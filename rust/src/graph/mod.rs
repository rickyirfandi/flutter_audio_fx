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

    /// Audio thread, RT-safe (no allocation): drop any buffered audio. Called
    /// when a disabled effect is re-enabled so latency-bearing effects don't
    /// replay stale samples captured before they were bypassed.
    fn flush(&mut self) {}

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
    /// Enable state seen by the previous callback (audio thread only), used to
    /// detect a disabled→enabled edge and flush stale buffered audio.
    was_enabled: UnsafeCell<bool>,
    /// AutoTune's pitch-detector mailbox, cached at construction so the
    /// runtime can rewire the detector worker without touching the effect.
    detect: Option<Arc<DetectShared>>,
}

// SAFETY: `cell`, `param_rx` and `was_enabled` are only ever accessed by the single audio
// thread; `enabled` is atomic; `param_tx` is behind a Mutex. See the contract.
unsafe impl Send for EffectSlot {}
unsafe impl Sync for EffectSlot {}

impl EffectSlot {
    pub fn new(fx: Box<dyn AudioEffect>) -> Self {
        Self::with_detect(fx, None)
    }

    pub fn with_detect(fx: Box<dyn AudioEffect>, detect: Option<Arc<DetectShared>>) -> Self {
        let initially_enabled = fx.is_enabled();
        let enabled = AtomicBool::new(initially_enabled);
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
            was_enabled: UnsafeCell::new(initially_enabled),
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
        self.apply_pending();
        let fx = &mut *self.cell.get();
        let enabled = self.enabled.load(Ordering::Relaxed);
        let was_enabled = &mut *self.was_enabled.get();
        if enabled && !*was_enabled { fx.flush(); }
        *was_enabled = enabled;
        if enabled { fx.process(buffer, sr); }
    }

    /// Apply queued settings before resetting an offline render, so smoothed
    /// parameters start at the requested values rather than the previous ones.
    /// SAFETY: caller must have exclusive access to the effect and queue.
    pub unsafe fn prepare_offline(&self) {
        self.apply_pending();
        (&mut *self.cell.get()).reset();
        *self.was_enabled.get() = self.is_enabled();
    }

    unsafe fn apply_pending(&self) {
        let fx = &mut *self.cell.get();
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
    }

    /// SAFETY: caller must guarantee they are not racing the audio thread
    /// (intended for offline file processing where no audio stream is live).
    #[inline]
    #[allow(clippy::mut_from_ref)] // sound per the contract above; callers uphold exclusivity
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

/// A parameter with an atomically-settable target and a per-sample smoothed
/// value. Eliminates zipper noise / clicks when the UI moves a knob: the
/// control thread `set`s the target, the audio thread `tick`s one sample at a
/// time toward it with a one-pole slew.
///
/// Threading: `set`/`get` are safe from any thread; `tick`/`snap`/`current`
/// require `&mut self` and are only called by the audio thread (which is the
/// sole holder of `&mut` to the effect — see `EffectSlot`).
#[derive(Debug)]
pub struct SmoothedParam {
    target: AtomicF32,
    current: f32,
}

impl SmoothedParam {
    pub fn new(v: f32) -> Self {
        Self { target: AtomicF32::new(v), current: v }
    }
    /// Control thread: set the target; the audio thread slews toward it.
    #[inline] pub fn set(&self, v: f32) { self.target.set(v); }
    /// The target value (not the smoothed instantaneous value).
    #[inline] pub fn get(&self) -> f32 { self.target.get() }
    /// Audio thread: advance one sample toward the target with the given
    /// one-pole coefficient (see [`smooth_coeff`]) and return the new value.
    #[inline]
    pub fn tick(&mut self, coeff: f32) -> f32 {
        let t = self.target.get();
        self.current = t + (self.current - t) * coeff;
        self.current
    }
    /// Audio thread / reset: jump instantly to the target.
    #[inline] pub fn snap(&mut self) { self.current = self.target.get(); }
    /// The smoothed instantaneous value (audio thread only).
    #[inline] pub fn current(&self) -> f32 { self.current }
}

/// One-pole smoothing coefficient for a time constant of `ms` milliseconds at
/// sample rate `sr`. `smoothed = target + (smoothed - target) * coeff`.
#[inline]
pub fn smooth_coeff(sr: f32, ms: f32) -> f32 {
    (-1.0 / (ms * 0.001 * sr).max(1.0)).exp()
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
    DeEsser, Exciter, Doubler,
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
