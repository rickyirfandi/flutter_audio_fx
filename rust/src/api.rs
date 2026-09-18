//! C ABI surface for the flutter_audio_fx engine.
//!
//! The Dart side binds these symbols via dart:ffi. All buffers passed across
//! the boundary are explicitly sized; strings are null-terminated UTF-8.
//!
//! Threading model:
//!   * The audio thread is owned by cpal and runs `process` on the chain.
//!   * The Dart side calls these functions from its main isolate; chain edits
//!     are atomic via ArcSwap, parameter writes are atomic via AtomicF32.
//!   * Pitch detection and recording happen on dedicated worker threads.

use std::ffi::{c_char, CStr};
use std::os::raw::c_int;
use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::engine::runtime::AudioRuntime;
use crate::effects;
use crate::graph::{AudioEffect, EffectSlot};
use crate::effects::auto_tune::DetectShared;

static RUNTIME: OnceCell<AudioRuntime> = OnceCell::new();

#[inline]
fn rt() -> Option<&'static AudioRuntime> { RUNTIME.get() }

/// Like `rt()`, but records a message for `fx_last_error_message` when the
/// engine has not been initialized (for functions that return error codes).
#[inline]
fn rt_or_err() -> Option<&'static AudioRuntime> {
    let r = RUNTIME.get();
    if r.is_none() {
        set_last_error("engine not initialized — call fx_engine_init first");
    }
    r
}

unsafe fn cstr<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() { return None; }
    CStr::from_ptr(p).to_str().ok()
}

// ─── Last-error reporting ───
//
// Error details (device names, sample-rate mismatches, file I/O failures) are
// stored here so Dart can surface a useful message instead of a bare code.

static LAST_ERROR: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn set_last_error(msg: impl Into<String>) {
    if let Ok(mut e) = LAST_ERROR.lock() { *e = msg.into(); }
}

/// Copy the most recent error message into `out` (a buffer of `cap` bytes),
/// NUL-terminated. Returns the number of bytes written, excluding the NUL;
/// 0 if there is no message or `out` is null / zero-sized.
#[no_mangle]
pub unsafe extern "C" fn fx_last_error_message(out: *mut c_char, cap: u32) -> u32 {
    if out.is_null() || cap == 0 { return 0; }
    let msg = match LAST_ERROR.lock() { Ok(m) => m, Err(_) => return 0 };
    let bytes = msg.as_bytes();
    let n = bytes.len().min(cap as usize - 1);
    std::ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), out, n);
    *out.add(n) = 0;
    n as u32
}

// ─── Engine lifecycle ───

/// Initialize the engine, or verify an existing one matches the requested
/// config. The runtime is a process-wide singleton: the first call wins, and a
/// later call with a *different* sample rate / buffer size returns `false`
/// (with the reason available via `fx_last_error_message`) instead of
/// silently keeping the old config.
#[no_mangle]
pub extern "C" fn fx_engine_init(sample_rate: u32, buffer_size: u32) -> bool {
    init_logging();
    let rt = RUNTIME.get_or_init(|| AudioRuntime::new(sample_rate, buffer_size as usize));
    if rt.sample_rate() == sample_rate && rt.buffer_size() == buffer_size as usize {
        true
    } else {
        set_last_error(format!(
            "engine already initialized at {} Hz / {} frames; cannot re-init at {} Hz / {} frames",
            rt.sample_rate(), rt.buffer_size(), sample_rate, buffer_size));
        false
    }
}

#[no_mangle]
pub extern "C" fn fx_engine_is_running() -> bool {
    rt().map(|r| r.is_running()).unwrap_or(false)
}

#[no_mangle]
pub extern "C" fn fx_engine_sample_rate() -> u32 {
    rt().map(|r| r.sample_rate()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn fx_engine_buffer_size() -> u32 {
    rt().map(|r| r.buffer_size() as u32).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn fx_engine_chain_latency_ms() -> f32 {
    rt().map(|r| r.chain_latency_ms()).unwrap_or(0.0)
}

#[no_mangle]
pub extern "C" fn fx_engine_start() -> c_int {
    let Some(r) = rt_or_err() else { return -1; };
    match r.start_realtime() { Ok(_) => 0, Err(e) => { set_last_error(e); -2 } }
}

/// `proc_path` may be null.
#[no_mangle]
pub unsafe extern "C" fn fx_engine_start_recording(
    raw_path: *const c_char,
    proc_path: *const c_char,
) -> c_int {
    let Some(r) = rt_or_err() else { return -1; };
    let Some(raw) = cstr(raw_path) else {
        set_last_error("raw_path is null or not valid UTF-8");
        return -3;
    };
    let proc_opt = cstr(proc_path);
    match r.start_realtime_with_recording(raw, proc_opt) {
        Ok(_) => 0, Err(e) => { set_last_error(e); -2 }
    }
}

#[no_mangle]
pub extern "C" fn fx_engine_stop() -> c_int {
    let Some(r) = rt_or_err() else { return -1; };
    match r.stop() { Ok(_) => 0, Err(e) => { set_last_error(e); -2 } }
}

#[no_mangle]
pub unsafe extern "C" fn fx_engine_process_file(
    input: *const c_char, output: *const c_char,
) -> c_int {
    let Some(r) = rt_or_err() else { return -1; };
    let (Some(i), Some(o)) = (cstr(input), cstr(output)) else {
        set_last_error("input/output path is null or not valid UTF-8");
        return -3;
    };
    match r.process_file(i, o) { Ok(_) => 0, Err(e) => { set_last_error(e); -2 } }
}

/// Progress of the current `fx_engine_process_file` call, 0.0..=1.0. Readable
/// from any thread while another thread runs the (blocking) processing call.
#[no_mangle]
pub extern "C" fn fx_process_file_progress() -> f32 {
    rt().map(|r| r.file_progress()).unwrap_or(0.0)
}

#[no_mangle]
pub unsafe extern "C" fn fx_engine_preview_file(input: *const c_char) -> c_int {
    let Some(r) = rt_or_err() else { return -1; };
    let Some(i) = cstr(input) else {
        set_last_error("input path is null or not valid UTF-8");
        return -3;
    };
    match r.preview_file(i) { Ok(_) => 0, Err(e) => { set_last_error(e); -2 } }
}

// ─── Chain editing ───
//
// The chain is set as a sequence of (effect_type, params) calls bracketed by
// `fx_chain_begin` / `fx_chain_commit`. This avoids any cross-FFI Vec marshal
// and keeps the per-effect parameter list flexible.

use std::sync::Mutex;
struct PendingChain {
    effects: Vec<PendingEffect>,
    current: Option<PendingEffect>,
}
struct PendingEffect {
    fx: Box<dyn AudioEffect>,
    detect: Option<Arc<DetectShared>>,
    enabled: bool,
    /// Params recorded at build time so they can be re-queued onto a reused
    /// live slot (whose effect we must not touch directly — see EffectSlot).
    params: Vec<(String, f32)>,
}

static PENDING: OnceCell<Mutex<PendingChain>> = OnceCell::new();
fn pending() -> &'static Mutex<PendingChain> {
    PENDING.get_or_init(|| Mutex::new(PendingChain {
        effects: Vec::new(), current: None,
    }))
}

#[no_mangle]
pub extern "C" fn fx_chain_begin() {
    let mut p = pending().lock().unwrap();
    p.effects.clear();
    p.current = None;
}

/// Push an effect onto the pending chain. Subsequent `fx_chain_set_param`
/// calls apply to this effect until the next push or commit.
/// Returns 0 on success, -1 if effect type is unknown.
#[no_mangle]
pub unsafe extern "C" fn fx_chain_push_effect(
    effect_type: *const c_char, enabled: bool,
) -> c_int {
    let Some(ty) = cstr(effect_type) else { return -1; };
    let mut p = pending().lock().unwrap();
    // Flush previous current.
    if let Some(prev) = p.current.take() {
        p.effects.push(prev);
    }
    let (fx, detect): (Box<dyn AudioEffect>, Option<Arc<DetectShared>>) = match ty {
        "noise_gate"     => (Box::new(effects::NoiseGate::new(-40.0, 1.0, 50.0)), None),
        "noise_suppress" => (Box::new(effects::NoiseSuppression::new(0.8)), None),
        "pitch_shift"    => (Box::new(effects::PitchShift::new(0.0)), None),
        "auto_tune"      => {
            let at = effects::AutoTune::new(
                effects::MusicalKey::C, effects::Scale::Major, 0.3);
            let d = at.shared();
            (Box::new(at), Some(d))
        }
        "equalizer"      => (Box::new(effects::Equalizer::new_default()), None),
        "compressor"     => (Box::new(effects::Compressor::new(-20.0, 4.0, 10.0, 100.0)), None),
        "limiter"        => (Box::new(effects::Limiter::new(-1.0, 50.0)), None),
        "reverb"         => (Box::new(effects::Reverb::new(0.5, 0.5, 0.3)), None),
        "chorus"         => (Box::new(effects::Chorus::new(1.5, 0.5, 0.3)), None),
        "delay"          => (Box::new(effects::Delay::new(250.0, 0.4, 0.3)), None),
        "distortion"     => (Box::new(effects::Distortion::new(
            0.3, 0.7, 0.5, effects::DistortionType::SoftClip)), None),
        "de_esser"       => (Box::new(effects::DeEsser::new(6000.0, -30.0, 1.0, 60.0)), None),
        "exciter"        => (Box::new(effects::Exciter::new(3000.0, 0.5, 0.3)), None),
        "doubler"        => (Box::new(effects::Doubler::new(0.3, 1.0)), None),
        _ => return -1,
    };
    fx.set_enabled(enabled);
    p.current = Some(PendingEffect { fx, detect, enabled, params: Vec::new() });
    0
}

#[no_mangle]
pub unsafe extern "C" fn fx_chain_set_param(
    name: *const c_char, value: f32,
) -> c_int {
    let Some(n) = cstr(name) else { return -1; };
    let mut p = pending().lock().unwrap();
    match &mut p.current {
        Some(cur) => {
            if cur.fx.set_param(n, value) {
                cur.params.push((n.to_string(), value));
                0
            } else {
                -2
            }
        }
        None => -3,
    }
}

/// Build the new chain from pending descriptors, reusing live slots whose
/// effect type matches (first unused match, in order). Reuse preserves DSP
/// state across chain edits — reverb tails keep ringing, the denoiser stays
/// warmed up — instead of every edit resetting every effect. Params for a
/// reused slot are pushed through its lock-free queue and applied by the
/// audio thread at the top of the next callback.
fn rebuild_chain(
    old: &[Arc<EffectSlot>], pending: Vec<PendingEffect>,
) -> Vec<Arc<EffectSlot>> {
    let mut used = vec![false; old.len()];
    let mut slots = Vec::with_capacity(pending.len());
    for pe in pending {
        let ty = pe.fx.effect_type();
        let hit = (0..old.len()).find(|&i| !used[i] && old[i].effect_type() == ty);
        let slot = match hit {
            Some(i) => {
                used[i] = true;
                let slot = Arc::clone(&old[i]);
                for (name, v) in &pe.params { slot.queue_param(name, *v); }
                slot
            }
            None => {
                // Fresh effect: its SmoothedParams were constructed at the
                // defaults and the preset params only moved their targets.
                // reset() snaps them so the first callback doesn't glide from
                // default to preset value (e.g. a mix=0 delay leaking wet).
                let mut fx = pe.fx;
                fx.reset();
                Arc::new(EffectSlot::with_detect(fx, pe.detect))
            }
        };
        slot.set_enabled(pe.enabled);
        slots.push(slot);
    }
    slots
}

#[no_mangle]
pub extern "C" fn fx_chain_commit() -> c_int {
    let Some(r) = rt_or_err() else { return -1; };
    let mut p = pending().lock().unwrap();
    if let Some(prev) = p.current.take() {
        p.effects.push(prev);
    }
    let pending_effects = std::mem::take(&mut p.effects);
    drop(p);
    let old = r.chain_snapshot();
    r.set_chain(rebuild_chain(&old, pending_effects));
    0
}

#[no_mangle]
pub extern "C" fn fx_chain_toggle(index: u32, enabled: bool) -> c_int {
    let Some(r) = rt() else { return -1; };
    r.toggle_effect(index as usize, enabled);
    0
}

/// Update a param on the live chain.
#[no_mangle]
pub unsafe extern "C" fn fx_chain_update_param(
    index: u32, name: *const c_char, value: f32,
) -> c_int {
    let Some(r) = rt() else { return -1; };
    let Some(n) = cstr(name) else { return -2; };
    r.update_param(index as usize, n, value);
    0
}

// ─── Visualization ───

/// Copy the current spectrum magnitudes into `out` (length `out_len` floats).
/// Returns the number of bins copied; 0 if no new spectrum is available.
/// Also writes dominant_freq/rms/bin_count via out-pointers (each may be null).
#[no_mangle]
pub unsafe extern "C" fn fx_get_spectrum(
    out: *mut f32, out_len: u32,
    dominant_freq: *mut f32, rms: *mut f32, bin_count: *mut u32,
) -> u32 {
    let Some(r) = rt() else { return 0; };
    let Some(s) = r.take_spectrum() else { return 0; };
    let n = (out_len as usize).min(s.magnitudes.len());
    if !out.is_null() && n > 0 {
        std::ptr::copy_nonoverlapping(s.magnitudes.as_ptr(), out, n);
    }
    if !dominant_freq.is_null() { *dominant_freq = s.dominant_freq; }
    if !rms.is_null()           { *rms = s.rms; }
    if !bin_count.is_null()     { *bin_count = s.bin_count as u32; }
    n as u32
}

#[no_mangle]
pub extern "C" fn fx_get_rms_level() -> f32 {
    rt().map(|r| r.rms_level()).unwrap_or(0.0)
}

/// Writes pitch_hz and confidence via out-pointers.
#[no_mangle]
pub unsafe extern "C" fn fx_get_pitch(pitch_hz: *mut f32, confidence: *mut f32) {
    let Some(r) = rt() else { return; };
    let (hz, conf) = r.pitch_data();
    if !pitch_hz.is_null()   { *pitch_hz = hz; }
    if !confidence.is_null() { *confidence = conf; }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pe(fx: Box<dyn AudioEffect>) -> PendingEffect {
        PendingEffect { fx, detect: None, enabled: true, params: Vec::new() }
    }

    #[test]
    fn rebuild_reuses_matching_slots_and_creates_new_ones() {
        let old = vec![
            Arc::new(EffectSlot::new(Box::new(effects::Reverb::new(0.5, 0.5, 0.3)))),
            Arc::new(EffectSlot::new(Box::new(effects::Delay::new(250.0, 0.4, 0.3)))),
        ];
        // Reordered + one addition: both existing slots must be reused (state
        // preserved), the chorus must be freshly created.
        let pending = vec![
            pe(Box::new(effects::Delay::new(100.0, 0.2, 0.5))),
            pe(Box::new(effects::Reverb::new(0.1, 0.1, 0.1))),
            pe(Box::new(effects::Chorus::new(1.5, 0.5, 0.3))),
        ];
        let new = rebuild_chain(&old, pending);
        assert_eq!(new.len(), 3);
        assert!(Arc::ptr_eq(&new[0], &old[1]), "delay slot must be reused");
        assert!(Arc::ptr_eq(&new[1], &old[0]), "reverb slot must be reused");
        assert!(
            !old.iter().any(|o| Arc::ptr_eq(&new[2], o)),
            "chorus has no old match and must be new",
        );
    }

    #[test]
    fn fresh_slot_starts_at_its_preset_values() {
        // Regression: a new effect's SmoothedParams started at the constructor
        // defaults and glided to the preset values over ~15 ms.
        let fx = effects::Doubler::new(0.3, 1.0);
        assert!(fx.set_param("mix", 0.0));
        let new = rebuild_chain(&[], vec![pe(Box::new(fx))]);
        let mut buf = vec![0.5f32; 64];
        unsafe { new[0].process_in_place(&mut buf, 48_000) };
        assert!(buf.iter().all(|&s| s == 0.5), "mix=0 must be dry from the first sample");
    }

    #[test]
    fn rebuild_does_not_reuse_one_slot_twice() {
        let old = vec![
            Arc::new(EffectSlot::new(Box::new(effects::Reverb::new(0.5, 0.5, 0.3)))),
        ];
        let pending = vec![
            pe(Box::new(effects::Reverb::new(0.1, 0.1, 0.1))),
            pe(Box::new(effects::Reverb::new(0.2, 0.2, 0.2))),
        ];
        let new = rebuild_chain(&old, pending);
        assert_eq!(new.len(), 2);
        assert!(Arc::ptr_eq(&new[0], &old[0]));
        assert!(!Arc::ptr_eq(&new[1], &old[0]));
    }
}

// ─── Logging init ───

fn init_logging() {
    #[cfg(target_os = "android")]
    {
        let _ = android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("flutter_audio_fx"),
        );
    }
    #[cfg(target_os = "ios")]
    {
        let _ = oslog::OsLogger::new("dev.flutter_audio_fx")
            .level_filter(log::LevelFilter::Info)
            .init();
    }
}
