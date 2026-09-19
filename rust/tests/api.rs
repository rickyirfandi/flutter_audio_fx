//! Integration tests for the C ABI surface (`rust/src/api.rs`) — the exact
//! functions Dart binds via dart:ffi. Everything here runs without an audio
//! device: engine init, chain building, param plumbing, error reporting, and
//! offline file processing.
//!
//! NOTE: the runtime is a process-wide singleton (`fx_engine_init` is
//! first-call-wins), so the whole surface is exercised from ONE test function
//! to keep ordering deterministic under the parallel test harness.

use std::ffi::CString;

use flutter_audio_fx_core::api::*;
use flutter_audio_fx_core::io::file_io;

fn last_error() -> String {
    let mut buf = [0u8; 512];
    let n = unsafe { fx_last_error_message(buf.as_mut_ptr().cast(), buf.len() as u32) };
    String::from_utf8_lossy(&buf[..n as usize]).into_owned()
}

fn c(s: &str) -> CString { CString::new(s).unwrap() }

#[test]
fn ffi_surface_end_to_end() {
    // ── Init semantics ──
    assert!(fx_engine_init(48000, 256), "first init must succeed");
    assert!(fx_engine_init(48000, 256), "same-config re-init must succeed");
    assert!(!fx_engine_init(44100, 256), "conflicting re-init must fail");
    assert!(
        last_error().contains("already initialized"),
        "mismatch reason must be reported, got: {}",
        last_error(),
    );
    assert_eq!(fx_engine_sample_rate(), 48000);
    assert_eq!(fx_engine_buffer_size(), 256);
    assert!(!fx_engine_is_running());

    // ── Chain building ──
    fx_chain_begin();
    assert_eq!(unsafe { fx_chain_push_effect(c("delay").as_ptr(), true) }, 0);
    assert_eq!(unsafe { fx_chain_set_param(c("time_ms").as_ptr(), 100.0) }, 0);
    assert_eq!(unsafe { fx_chain_set_param(c("mix").as_ptr(), 0.5) }, 0);
    assert_eq!(
        unsafe { fx_chain_set_param(c("no_such_param").as_ptr(), 1.0) },
        -2,
        "unknown param must be rejected",
    );
    assert_eq!(unsafe { fx_chain_push_effect(c("reverb").as_ptr(), true) }, 0);
    assert_eq!(
        unsafe { fx_chain_push_effect(c("not_an_effect").as_ptr(), true) },
        -1,
        "unknown effect type must be rejected",
    );
    assert_eq!(fx_chain_commit(), 0);
    assert!(fx_engine_chain_latency_ms() >= 0.0);

    // Live edits on the committed chain.
    assert_eq!(fx_chain_toggle(0, false), 0);
    assert_eq!(fx_chain_toggle(0, true), 0);
    assert_eq!(unsafe { fx_chain_update_param(0, c("feedback").as_ptr(), 0.3) }, 0);

    // ── Visualization getters (no stream running → empty/zero, no crash) ──
    let mut spec = [0.0f32; 32];
    let n = unsafe {
        fx_get_spectrum(spec.as_mut_ptr(), 32,
            std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
    };
    assert_eq!(n, 0, "no spectrum without a live stream");
    unsafe { fx_get_pitch(std::ptr::null_mut(), std::ptr::null_mut()); }
    assert_eq!(fx_get_rms_level(), 0.0);

    // ── Offline processing: stereo in → processed mono out ──
    let dir = std::env::temp_dir();
    let in_path = dir.join("fx_api_test_in.wav");
    let out_path = dir.join("fx_api_test_out.wav");
    let in_c = c(in_path.to_str().unwrap());
    let out_c = c(out_path.to_str().unwrap());

    // 0.5 s interleaved stereo sine at 48 kHz.
    let frames = 24000;
    let mut interleaved = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let s = (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48000.0).sin() * 0.4;
        interleaved.push(s);
        interleaved.push(s * 0.5);
    }
    file_io::write_wav(in_path.to_str().unwrap(), &interleaved, 48000, 2)
        .expect("write test input");

    assert_eq!(
        unsafe { fx_engine_process_file(in_c.as_ptr(), out_c.as_ptr()) },
        0,
        "process_file failed: {}",
        last_error(),
    );
    assert!((fx_process_file_progress() - 1.0).abs() < 1e-6, "progress must end at 1.0");

    let (out_samples, out_meta) =
        file_io::read_wav(out_path.to_str().unwrap()).expect("read output");
    assert_eq!(out_meta.channels, 1, "processed output must be mono");
    assert_eq!(out_samples.len(), frames, "one output sample per input frame");
    assert!(out_samples.iter().all(|s| s.is_finite()));
    assert!(
        out_samples.iter().any(|s| s.abs() > 1e-4),
        "processed output should carry signal",
    );

    // An idle editor update must affect the very next export, including when
    // setChain reuses a slot and queues the new preset's parameters.
    fx_chain_begin();
    assert_eq!(unsafe { fx_chain_push_effect(c("noise_gate").as_ptr(), true) }, 0);
    assert_eq!(unsafe { fx_chain_set_param(c("threshold_db").as_ptr(), -80.0) }, 0);
    assert_eq!(fx_chain_commit(), 0);
    assert_eq!(unsafe { fx_chain_update_param(0, c("threshold_db").as_ptr(), 0.0) }, 0);
    assert_eq!(unsafe { fx_engine_process_file(in_c.as_ptr(), out_c.as_ptr()) }, 0);
    let (muted, _) = file_io::read_wav(out_path.to_str().unwrap()).unwrap();
    assert!(muted.iter().all(|s| s.abs() < 1e-4), "queued gate threshold must mute export");

    fx_chain_begin();
    assert_eq!(unsafe { fx_chain_push_effect(c("noise_gate").as_ptr(), true) }, 0);
    assert_eq!(unsafe { fx_chain_set_param(c("threshold_db").as_ptr(), -80.0) }, 0);
    assert_eq!(fx_chain_commit(), 0);
    assert_eq!(unsafe { fx_engine_process_file(in_c.as_ptr(), out_c.as_ptr()) }, 0);
    let (audible, _) = file_io::read_wav(out_path.to_str().unwrap()).unwrap();
    assert!(audible.iter().any(|s| s.abs() > 0.1), "reused slot must use new preset");

    // ── Error paths surface messages ──
    let missing = c(dir.join("fx_api_test_missing.wav").to_str().unwrap());
    assert_eq!(
        unsafe { fx_engine_process_file(missing.as_ptr(), out_c.as_ptr()) },
        -2,
    );
    assert!(!last_error().is_empty(), "file error must set last_error");

    assert_eq!(
        unsafe { fx_engine_process_file(std::ptr::null(), out_c.as_ptr()) },
        -3,
        "null path must be rejected",
    );

    // ── State preservation across commits: delay tail survives a re-commit ──
    // Prime the delay line by processing a burst, then commit an identical
    // chain; the reused slot must be the same live object, which we can only
    // observe end-to-end here via it still accepting live param updates.
    fx_chain_begin();
    assert_eq!(unsafe { fx_chain_push_effect(c("delay").as_ptr(), true) }, 0);
    assert_eq!(unsafe { fx_chain_push_effect(c("reverb").as_ptr(), true) }, 0);
    assert_eq!(fx_chain_commit(), 0);
    assert_eq!(unsafe { fx_chain_update_param(1, c("mix").as_ptr(), 0.7) }, 0);

    let _ = std::fs::remove_file(&in_path);
    let _ = std::fs::remove_file(&out_path);
}
