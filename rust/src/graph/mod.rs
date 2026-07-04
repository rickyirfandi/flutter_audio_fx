use std::cell::UnsafeCell;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use ringbuf::{HeapRb, HeapProd, HeapCons};
use ringbuf::traits::{Producer, Consumer, Split};

use crate::effects::auto_tune::DetectShared;

/// Trait that all audio effects must implement.
/// `process` is called on the real-time audio thread — it MUST NOT allocate,
/// lock, or perform syscalls.
pub trait AudioEffect: Send + Sync {
    fn process(&mut self, buffer: &mut [f32], sample_rate: u32);
    fn effect_type(&self) -> EffectType;
    fn latency_samples(&self) -> usize { 0 }
    fn reset(&mut self);

    /// RT-safe enable/disable via atomics
    fn set_enabled(&self, enabled: bool);
    fn is_enabled(&self) -> bool;

    /// RT-safe param update via atomics. Returns true if the param was found.
    fn set_param(&self, name: &str, value: f32) -> bool;
}

/// Max length of a parameter name passed across the control→audio queue.
/// All current param names are well under this (longest is ~16 chars).
const PARAM_NAME_CAP: usize = 32;

/// A single queued parameter update (control thread → audio thread).
#[derive(Clone, Copy)]
struct ParamMsg {
    name: [u8; PARAM_NAME_CAP],
    len: u8,
    value: f32,
}

/// A slot holding a single effect.
///
/// SOUNDNESS CONTRACT — the audio thread is the *only* thread that ever forms a
/// reference (`&` or `&mut`) to the boxed effect:
///   * The audio callback calls `process_in_place`, which takes `&mut` to the
///     effect and is the sole accessor while a stream is live.
///   * The control plane NEVER touches the effect directly. Enable/disable goes
///     through `self.enabled` (a slot-owned atomic); parameter changes are
///     pushed onto a lock-free SPSC queue and applied by the audio thread at
///     the top of `process_in_place`. Static metadata (`effect_type`,
///     `latency`) is cached at construction.
///   * Chain edits replace the whole `Arc<Vec<Arc<EffectSlot>>>` via
///     `ArcSwap`, so the audio thread never sees a partially-mutated chain.
///     Individual slots may be carried over (`Arc`-cloned) into the new chain
///     to preserve DSP state; that is sound because the control plane still
///     only touches the slot's atomics and param queue — never the effect.
/// This removes the `&mut`/`&` cross-thread aliasing that a direct `view()`
/// would create.
pub struct EffectSlot {
    cell: UnsafeCell<Box<dyn AudioEffect>>,
    enabled: AtomicBool,
    effect_type: EffectType,
    latency: usize,
    /// Control thread is the sole producer (guarded by the Mutex, only ever
    /// contended by control-plane callers, never the audio thread).
    param_tx: Mutex<HeapProd<ParamMsg>>,
    /// Audio thread is the sole consumer.
    param_rx: UnsafeCell<HeapCons<ParamMsg>>,
    /// AutoTune's pitch-detector mailbox, cached at construction so the
    /// runtime can rewire the detector worker without touching the effect.
    detect: Option<Arc<DetectShared>>,
}

// SAFETY: `cell` and `param_rx` are only ever accessed by the single audio
// thread; `enabled` is atomic; `param_tx` is behind a Mutex. See the contract.
unsafe impl Send for EffectSlot {}
unsafe impl Sync for EffectSlot {}

impl EffectSlot {
    pub fn new(fx: Box<dyn AudioEffect>) -> Self {
        Self::with_detect(fx, None)
    }

    pub fn with_detect(fx: Box<dyn AudioEffect>, detect: Option<Arc<DetectShared>>) -> Self {
        let enabled = AtomicBool::new(fx.is_enabled());
        let effect_type = fx.effect_type();
        let latency = fx.latency_samples();
        let (tx, rx) = HeapRb::<ParamMsg>::new(256).split();
        Self {
            cell: UnsafeCell::new(fx),
            enabled,
            effect_type,
            latency,
            param_tx: Mutex::new(tx),
            param_rx: UnsafeCell::new(rx),
            detect,
        }
    }

    pub fn detect_shared(&self) -> Option<Arc<DetectShared>> { self.detect.clone() }

    #[inline] pub fn is_enabled(&self) -> bool { self.enabled.load(Ordering::Relaxed) }
    #[inline] pub fn set_enabled(&self, enabled: bool) { self.enabled.store(enabled, Ordering::Relaxed); }
    #[inline] pub fn effect_type(&self) -> EffectType { self.effect_type }
    #[inline] pub fn latency_samples(&self) -> usize { self.latency }

    /// Queue a parameter change to be applied by the audio thread. Lock-free on
    /// the audio side; the control side takes an uncontended Mutex. A full
    /// queue (256 pending updates) drops the oldest-unread update silently.
    pub fn queue_param(&self, name: &str, value: f32) {
        let bytes = name.as_bytes();
        if bytes.len() > PARAM_NAME_CAP { return; }
        let mut msg = ParamMsg { name: [0; PARAM_NAME_CAP], len: bytes.len() as u8, value };
        msg.name[..bytes.len()].copy_from_slice(bytes);
        if let Ok(mut tx) = self.param_tx.lock() {
            let _ = tx.try_push(msg);
        }
    }

    /// SAFETY: caller must be the audio thread (sole writer / sole consumer).
    #[inline]
    pub unsafe fn process_in_place(&self, buffer: &mut [f32], sr: u32) {
        let fx: &mut Box<dyn AudioEffect> = &mut *self.cell.get();
        // Drain pending parameter updates (sole consumer) — even while
        // disabled, so values are current when re-enabled.
        let rx: &mut HeapCons<ParamMsg> = &mut *self.param_rx.get();
        while let Some(msg) = rx.try_pop() {
            if let Ok(name) = std::str::from_utf8(&msg.name[..msg.len as usize]) {
                fx.set_param(name, msg.value);
            }
        }
        // Mirror the slot's enable state into the effect's own gate (the audio
        // thread is the sole writer of the effect), then run if enabled.
        let enabled = self.enabled.load(Ordering::Relaxed);
        fx.set_enabled(enabled);
        if enabled { fx.process(buffer, sr); }
    }

    /// SAFETY: caller must guarantee they are not racing the audio thread
    /// (intended for offline file processing where no audio stream is live).
    #[inline]
    pub unsafe fn as_mut(&self) -> &mut Box<dyn AudioEffect> {
        &mut *self.cell.get()
    }
}

/// Lock-free atomic f32 for RT-safe parameter passing.
#[derive(Debug)]
pub struct AtomicF32 {
    bits: AtomicU32,
}

impl AtomicF32 {
    pub const fn new(val: f32) -> Self {
        Self { bits: AtomicU32::new(val.to_bits()) }
    }
    #[inline] pub fn get(&self) -> f32 { f32::from_bits(self.bits.load(Ordering::Relaxed)) }
    #[inline] pub fn set(&self, val: f32) { self.bits.store(val.to_bits(), Ordering::Relaxed); }
}

#[derive(Debug)]
pub struct AtomicEnabled {
    inner: AtomicBool,
}

impl AtomicEnabled {
    pub const fn new(enabled: bool) -> Self {
        Self { inner: AtomicBool::new(enabled) }
    }
    #[inline] pub fn get(&self) -> bool { self.inner.load(Ordering::Relaxed) }
    #[inline] pub fn set(&self, enabled: bool) { self.inner.store(enabled, Ordering::Relaxed); }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectType {
    NoiseGate, NoiseSuppression, PitchShift, AutoTune,
    Equalizer, Compressor, Limiter, Reverb, Chorus, Delay, Distortion,
}

#[macro_export]
macro_rules! impl_effect_meta {
    ($self:ident, $ty:expr, { $($name:literal => $field:ident),* $(,)? }) => {
        fn effect_type(&$self) -> $crate::graph::EffectType { $ty }
        fn set_enabled(&$self, enabled: bool) { $self.enabled.set(enabled); }
        fn is_enabled(&$self) -> bool { $self.enabled.get() }
        fn set_param(&$self, name: &str, value: f32) -> bool {
            match name {
                $( $name => { $self.$field.set(value); true } )*
                _ => false,
            }
        }
    };
}
