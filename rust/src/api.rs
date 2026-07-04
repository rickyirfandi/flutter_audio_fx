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
use crate::graph::AudioEffect;
use crate::effects::auto_tune::DetectShared;

static RUNTIME: OnceCell<AudioRuntime> = OnceCell::new();

#[inline]
fn rt() -> Option<&'static AudioRuntime> { RUNTIME.get() }

unsafe fn cstr<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() { return None; }
    CStr::from_ptr(p).to_str().ok()
}

// ─── Engine lifecycle ───

#[no_mangle]
pub extern "C" fn fx_engine_init(sample_rate: u32, buffer_size: u32) -> bool {
    init_logging();
    RUNTIME.set(AudioRuntime::new(sample_rate, buffer_size as usize)).is_ok()
        || RUNTIME.get().is_some()
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
    let Some(r) = rt() else { return -1; };
    match r.start_realtime() { Ok(_) => 0, Err(_) => -2 }
}

/// `proc_path` may be null.
#[no_mangle]
pub unsafe extern "C" fn fx_engine_start_recording(
    raw_path: *const c_char,
    proc_path: *const c_char,
) -> c_int {
    let Some(r) = rt() else { return -1; };
    let Some(raw) = cstr(raw_path) else { return -3; };
    let proc_opt = cstr(proc_path);
    match r.start_realtime_with_recording(raw, proc_opt) {
        Ok(_) => 0, Err(_) => -2,
    }
}

#[no_mangle]
pub extern "C" fn fx_engine_stop() -> c_int {
    let Some(r) = rt() else { return -1; };
    match r.stop() { Ok(_) => 0, Err(_) => -2 }
}

#[no_mangle]
pub unsafe extern "C" fn fx_engine_process_file(
    input: *const c_char, output: *const c_char,
) -> c_int {
    let Some(r) = rt() else { return -1; };
    let Some(i) = cstr(input)  else { return -3; };
    let Some(o) = cstr(output) else { return -3; };
    match r.process_file(i, o) { Ok(_) => 0, Err(_) => -2 }
}

#[no_mangle]
pub unsafe extern "C" fn fx_engine_preview_file(input: *const c_char) -> c_int {
    let Some(r) = rt() else { return -1; };
    let Some(i) = cstr(input) else { return -3; };
    match r.preview_file(i) { Ok(_) => 0, Err(_) => -2 }
}

// ─── Chain editing ───
//
// The chain is set as a sequence of (effect_type, params) calls bracketed by
// `fx_chain_begin` / `fx_chain_commit`. This avoids any cross-FFI Vec marshal
// and keeps the per-effect parameter list flexible.

use std::sync::Mutex;
struct PendingChain {
    effects: Vec<Box<dyn AudioEffect>>,
    detect_shareds: Vec<Arc<DetectShared>>,
    current: Option<PendingEffect>,
}
struct PendingEffect {
    fx: Box<dyn AudioEffect>,
    detect: Option<Arc<DetectShared>>,
}

static PENDING: OnceCell<Mutex<PendingChain>> = OnceCell::new();
fn pending() -> &'static Mutex<PendingChain> {
    PENDING.get_or_init(|| Mutex::new(PendingChain {
        effects: Vec::new(), detect_shareds: Vec::new(), current: None,
    }))
}

#[no_mangle]
pub extern "C" fn fx_chain_begin() {
    let mut p = pending().lock().unwrap();
    p.effects.clear();
    p.detect_shareds.clear();
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
        if let Some(d) = prev.detect { p.detect_shareds.push(d); }
        p.effects.push(prev.fx);
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
        _ => return -1,
    };
    fx.set_enabled(enabled);
    p.current = Some(PendingEffect { fx, detect });
    0
}

#[no_mangle]
pub unsafe extern "C" fn fx_chain_set_param(
    name: *const c_char, value: f32,
) -> c_int {
    let Some(n) = cstr(name) else { return -1; };
    let p = pending().lock().unwrap();
    match &p.current {
        Some(cur) => if cur.fx.set_param(n, value) { 0 } else { -2 },
        None => -3,
    }
}

#[no_mangle]
pub extern "C" fn fx_chain_commit() -> c_int {
    let Some(r) = rt() else { return -1; };
    let mut p = pending().lock().unwrap();
    if let Some(prev) = p.current.take() {
        if let Some(d) = prev.detect { p.detect_shareds.push(d); }
        p.effects.push(prev.fx);
    }
    let effects = std::mem::take(&mut p.effects);
    let shareds = std::mem::take(&mut p.detect_shareds);
    r.set_chain(effects, shareds);
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
