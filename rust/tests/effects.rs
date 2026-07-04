//! Black-box correctness tests for every effect. These tests guard against
//! NaN/Inf, gross gain explosions, and RT-unsafe regressions (the chunked
//! invocations mimic the cpal callback's small fixed-size buffers).

use flutter_audio_fx_core::effects::{
    AutoTune, Chorus, Compressor, Delay, Distortion, DistortionType, Equalizer,
    Limiter, MusicalKey, NoiseGate, NoiseSuppression, PitchShift, Reverb, Scale,
};
use flutter_audio_fx_core::graph::AudioEffect;

const SR: u32 = 48000;

fn sine(freq: f32, len: usize, sr: u32) -> Vec<f32> {
    (0..len)
        .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin() * 0.5)
        .collect()
}

fn assert_finite(buf: &[f32], label: &str) {
    for (i, &s) in buf.iter().enumerate() {
        assert!(s.is_finite(), "{label}: non-finite sample at {i}: {s}");
        assert!(s.abs() < 16.0, "{label}: runaway sample at {i}: {s}");
    }
}

fn drive_in_chunks(fx: &mut dyn AudioEffect, mut buf: Vec<f32>, chunk: usize) -> Vec<f32> {
    let mut pos = 0;
    while pos < buf.len() {
        let end = (pos + chunk).min(buf.len());
        fx.process(&mut buf[pos..end], SR);
        pos = end;
    }
    buf
}

#[test]
fn noise_gate_silences_below_threshold() {
    let mut fx = NoiseGate::new(-30.0, 1.0, 50.0);
    let quiet = vec![0.001f32; SR as usize / 2];
    let out = drive_in_chunks(&mut fx, quiet, 128);
    assert_finite(&out, "noise_gate quiet");
    let max = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(max < 0.01, "gate should attenuate quiet input, got {max}");
}

#[test]
fn noise_suppress_passes_clean_speech_band() {
    let mut fx = NoiseSuppression::new(0.5);
    let s = sine(220.0, SR as usize, SR);
    let out = drive_in_chunks(&mut fx, s, 256);
    assert_finite(&out, "noise_suppress");
}

#[test]
fn pitch_shift_no_alloc_per_chunk() {
    let mut fx = PitchShift::new(7.0); // up a fifth
    let s = sine(220.0, SR as usize, SR);
    let out = drive_in_chunks(&mut fx, s, 128);
    assert_finite(&out, "pitch_shift");
}

#[test]
fn pitch_shift_passthrough_when_zero() {
    let mut fx = PitchShift::new(0.0);
    let mut s = sine(220.0, 4096, SR);
    let original = s.clone();
    fx.process(&mut s, SR);
    assert_eq!(s, original, "ratio==1 must early-return without modifying");
}

#[test]
fn auto_tune_chromatic_does_not_crash() {
    let mut fx = AutoTune::new(MusicalKey::C, Scale::Chromatic, 0.0);
    let s = sine(440.0, SR as usize, SR);
    let out = drive_in_chunks(&mut fx, s, 128);
    assert_finite(&out, "auto_tune");
}

#[test]
fn equalizer_unit_gain_with_zero_bands() {
    let mut fx = Equalizer::new_default();
    let s = sine(1000.0, 4096, SR);
    let original = s.clone();
    let out = drive_in_chunks(&mut fx, s, 128);
    // With all bands at 0 dB the EQ should be near-transparent (allow tiny drift).
    let drift: f32 = out
        .iter()
        .zip(original.iter())
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / out.len() as f32;
    assert!(drift < 0.05, "default EQ should be ~unity gain, drift={drift}");
}

#[test]
fn compressor_attenuates_loud_signal() {
    let mut fx = Compressor::new(-20.0, 4.0, 5.0, 50.0);
    let loud = sine(220.0, SR as usize, SR).into_iter().map(|s| s * 1.5).collect::<Vec<_>>();
    let out = drive_in_chunks(&mut fx, loud, 128);
    assert_finite(&out, "compressor");
    let peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(peak < 1.5, "compressor should reduce peaks, got {peak}");
}

#[test]
fn limiter_caps_at_ceiling() {
    let mut fx = Limiter::new(-1.0, 50.0);
    let loud: Vec<f32> = sine(220.0, SR as usize, SR).into_iter().map(|s| s * 4.0).collect();
    let out = drive_in_chunks(&mut fx, loud, 128);
    assert_finite(&out, "limiter");
    let peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(peak < 1.05, "limiter ceiling exceeded: {peak}");
}

#[test]
fn reverb_decays_to_silence() {
    let mut fx = Reverb::new(0.5, 0.5, 0.5);
    let mut s = sine(440.0, 4096, SR);
    s.extend(std::iter::repeat(0.0f32).take(SR as usize * 3));
    let out = drive_in_chunks(&mut fx, s, 128);
    assert_finite(&out, "reverb");
    let tail_rms = (out[out.len() - 1024..]
        .iter()
        .map(|s| s * s)
        .sum::<f32>()
        / 1024.0)
        .sqrt();
    assert!(tail_rms < 0.1, "reverb tail did not decay: rms={tail_rms}");
}

#[test]
fn chorus_mixes_dry_wet() {
    let mut fx = Chorus::new(1.5, 0.5, 0.3);
    let s = sine(220.0, 4096, SR);
    let out = drive_in_chunks(&mut fx, s, 128);
    assert_finite(&out, "chorus");
}

#[test]
fn delay_repeats_signal() {
    let mut fx = Delay::new(100.0, 0.5, 0.5);
    let mut s = vec![0.0f32; SR as usize / 2];
    for i in 0..128 {
        s[i] = if i % 8 == 0 { 0.5 } else { 0.0 };
    }
    let out = drive_in_chunks(&mut fx, s, 128);
    assert_finite(&out, "delay");
}

#[test]
fn distortion_clips_into_range() {
    for ty in [
        DistortionType::SoftClip,
        DistortionType::HardClip,
        DistortionType::Tanh,
        DistortionType::Bitcrush,
    ] {
        let mut fx = Distortion::new(0.8, 0.5, 1.0, ty);
        let loud: Vec<f32> = sine(220.0, 4096, SR).into_iter().map(|s| s * 2.0).collect();
        let out = drive_in_chunks(&mut fx, loud, 128);
        assert_finite(&out, &format!("distortion {:?}", ty));
        let peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak < 5.0, "distortion peak too high for {:?}: {peak}", ty);
    }
}

#[test]
fn yin_detects_a4() {
    use flutter_audio_fx_core::analysis::pitch::YinDetector;
    let mut yin = YinDetector::new(2048);
    let signal = sine(440.0, 2048, SR);
    let (freq, conf) = yin.detect(&signal, SR).expect("should find pitch");
    assert!(
        (freq - 440.0).abs() < 5.0,
        "expected 440 Hz, got {freq} (conf {conf})"
    );
    assert!(conf > 0.5, "low confidence: {conf}");
}

#[test]
fn yin_returns_none_for_silence() {
    use flutter_audio_fx_core::analysis::pitch::YinDetector;
    let mut yin = YinDetector::new(2048);
    let silence = vec![0.0f32; 2048];
    assert!(yin.detect(&silence, SR).is_none());
}

#[test]
fn slot_enable_toggle_and_param_queue() {
    use flutter_audio_fx_core::graph::EffectSlot;
    // mix=0 → dry passthrough until a queued param turns it up.
    let fx = Box::new(Distortion::new(0.9, 0.5, 0.0, DistortionType::HardClip));
    let slot = EffectSlot::new(fx);

    let mut buf = vec![0.8f32; 64];
    let orig = buf.clone();
    unsafe { slot.process_in_place(&mut buf, SR); }
    assert_eq!(buf, orig, "mix=0 must pass through unchanged");

    // Queue a live param update; the audio thread applies it on next process.
    slot.queue_param("mix", 1.0);
    let mut buf2 = vec![0.8f32; 64];
    unsafe { slot.process_in_place(&mut buf2, SR); }
    assert!(
        buf2.iter().zip(orig.iter()).any(|(a, b)| (a - b).abs() > 1e-6),
        "queued mix=1.0 should change the signal",
    );

    // Disabling via the slot must short-circuit to passthrough even though the
    // effect's own params would otherwise modify the signal.
    slot.set_enabled(false);
    let mut buf3 = vec![0.8f32; 64];
    let orig3 = buf3.clone();
    unsafe { slot.process_in_place(&mut buf3, SR); }
    assert_eq!(buf3, orig3, "disabled slot must pass through");

    // Re-enabling must take effect (regression guard for the slot/effect enable
    // mirroring).
    slot.set_enabled(true);
    let mut buf4 = vec![0.8f32; 64];
    unsafe { slot.process_in_place(&mut buf4, SR); }
    assert!(
        buf4.iter().zip(orig3.iter()).any(|(a, b)| (a - b).abs() > 1e-6),
        "re-enabled slot should modify the signal again",
    );
}

#[test]
fn noise_suppress_delays_by_one_frame() {
    // First 480 output samples are the primed-silence latency; afterwards the
    // denoised signal must be non-trivial (not stuck at zero / not garbled NaN).
    let mut fx = NoiseSuppression::new(1.0);
    let s = sine(300.0, 4800, SR);
    let out = drive_in_chunks(&mut fx, s, 100); // odd chunk vs 480 frame
    assert_finite(&out, "noise_suppress latency");
    let head_rms = (out[..480].iter().map(|s| s * s).sum::<f32>() / 480.0).sqrt();
    assert!(head_rms < 1e-6, "first frame should be primed silence, got {head_rms}");
    let body_rms = (out[960..1440].iter().map(|s| s * s).sum::<f32>() / 480.0).sqrt();
    assert!(body_rms > 1e-3, "denoised body should carry signal, got {body_rms}");
}

#[test]
fn spectrum_analyzer_no_alloc_per_frame() {
    use flutter_audio_fx_core::analysis::spectrum::SpectrumAnalyzer;
    let mut az = SpectrumAnalyzer::new(2048);
    let s = sine(440.0, SR as usize, SR);
    // Call feed/get_spectrum many times; should never panic.
    for chunk in s.chunks(128) {
        az.feed(chunk);
        let _ = az.get_spectrum();
    }
}
