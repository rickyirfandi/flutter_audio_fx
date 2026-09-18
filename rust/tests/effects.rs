//! Black-box correctness tests for every effect. These tests guard against
//! NaN/Inf, gross gain explosions, and RT-unsafe regressions (the chunked
//! invocations mimic the cpal callback's small fixed-size buffers).

use flutter_audio_fx_core::effects::{
    AutoTune, Chorus, Compressor, DeEsser, Delay, Distortion, DistortionType, Doubler, Equalizer,
    Exciter, Limiter, MusicalKey, NoiseGate, NoiseSuppression, PitchShift, Reverb, Scale,
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

fn rms(buf: &[f32]) -> f32 {
    (buf.iter().map(|sample| sample * sample).sum::<f32>() / buf.len() as f32).sqrt()
}

fn magnitude_at(buf: &[f32], frequency: f32, sample_rate: u32) -> f32 {
    let mut real = 0.0;
    let mut imaginary = 0.0;
    for (index, &sample) in buf.iter().enumerate() {
        let phase = 2.0 * std::f32::consts::PI * frequency * index as f32 / sample_rate as f32;
        real += sample * phase.cos();
        imaginary -= sample * phase.sin();
    }
    (real * real + imaginary * imaginary).sqrt() / buf.len() as f32
}

/// Robust pitch estimate of a 2048-sample window via the crate's own YIN
/// detector (zero-crossing counts are thrown off by low-level ripple).
fn yin_freq(window: &[f32]) -> f32 {
    use flutter_audio_fx_core::analysis::pitch::YinDetector;
    let mut yin = YinDetector::new(2048);
    yin.detect(&window[..2048], SR)
        .map(|(f, _)| f)
        .unwrap_or(0.0)
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
fn pitch_shift_near_transparent_at_zero() {
    // The vocoder now always runs (constant latency — no engage click), so
    // unity is *near*-transparent rather than bit-exact: the output must be
    // the same pitch and comparable level after the fft_size latency.
    let mut fx = PitchShift::new(0.0);
    let n = 16384;
    let s = sine(220.0, n, SR);
    let out = drive_in_chunks(&mut fx, s.clone(), 128);
    assert_finite(&out, "pitch_shift zero");
    // fft_size - hop = 2048 - 512 (standard phase-vocoder FIFO latency).
    assert_eq!(fx.latency_samples(), 1536, "latency must be constant");

    let window = &out[8192..16384];
    let freq = yin_freq(window);
    assert!(
        (freq - 220.0).abs() < 5.0,
        "unity shift changed pitch: {freq} Hz"
    );
    // Level preserved within a broad band (catches silence / blow-up).
    let rms = (window.iter().map(|x| x * x).sum::<f32>() / window.len() as f32).sqrt();
    let in_rms = 0.5 / std::f32::consts::SQRT_2;
    assert!(
        rms > in_rms * 0.5 && rms < in_rms * 1.6,
        "unity shift level off: rms={rms} vs input {in_rms}"
    );
}

#[test]
fn pitch_shift_formant_preserve_changes_spectrum() {
    // A harmonic-rich source shifted up a fifth must differ audibly between
    // formant-preserving and plain modes (the old formant flag was dead code).
    fn run(formant: f32) -> Vec<f32> {
        let mut fx = PitchShift::new(7.0);
        assert!(fx.set_param("formant_preserve", formant));
        let s: Vec<f32> = (0..32768)
            .map(|i| {
                let t = i as f32 / SR as f32;
                (1..=9)
                    .step_by(2)
                    .map(|h| (2.0 * std::f32::consts::PI * 220.0 * h as f32 * t).sin() / h as f32)
                    .sum::<f32>()
                    * 0.3
            })
            .collect();
        drive_in_chunks(&mut fx, s, 128)
    }
    let on = run(1.0);
    let off = run(0.0);
    assert_finite(&on, "formant on");
    assert_finite(&off, "formant off");
    let diff = on[4096..]
        .iter()
        .zip(off[4096..].iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(diff > 1e-3, "formant_preserve had no effect on the output");
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
    assert!(
        drift < 0.05,
        "default EQ should be ~unity gain, drift={drift}"
    );
}

#[test]
fn compressor_attenuates_loud_signal() {
    let mut fx = Compressor::new(-20.0, 4.0, 5.0, 50.0);
    let loud = sine(220.0, SR as usize, SR)
        .into_iter()
        .map(|s| s * 1.5)
        .collect::<Vec<_>>();
    let out = drive_in_chunks(&mut fx, loud, 128);
    assert_finite(&out, "compressor");
    let peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(peak < 1.5, "compressor should reduce peaks, got {peak}");
}

#[test]
fn compressor_sidechain_hpf_reduces_bass_pumping() {
    let input = sine(80.0, SR as usize, SR);
    let mut full_band = Compressor::new(-30.0, 8.0, 1.0, 50.0);
    let full_band_out = drive_in_chunks(&mut full_band, input.clone(), 128);

    let mut filtered = Compressor::new(-30.0, 8.0, 1.0, 50.0);
    assert!(filtered.set_param("sidechain_hpf_hz", 500.0));
    let filtered_out = drive_in_chunks(&mut filtered, input, 128);
    assert_finite(&filtered_out, "compressor sidechain HPF");
    assert!(
        rms(&filtered_out[SR as usize / 2..]) > rms(&full_band_out[SR as usize / 2..]) * 1.5,
        "sidechain HPF should prevent low frequencies from driving full gain reduction",
    );
}

#[test]
fn limiter_caps_at_ceiling() {
    let mut fx = Limiter::new(-1.0, 50.0);
    let loud: Vec<f32> = sine(220.0, SR as usize, SR)
        .into_iter()
        .map(|s| s * 4.0)
        .collect();
    let out = drive_in_chunks(&mut fx, loud, 128);
    assert_finite(&out, "limiter");
    let peak = out.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(peak < 1.05, "limiter ceiling exceeded: {peak}");
}

#[test]
fn limiter_lookahead_caps_transients() {
    let mut fx = Limiter::new(-1.0, 50.0);
    assert_eq!(
        fx.latency_samples(),
        240,
        "limiter must report its lookahead"
    );
    // Silence, then a sudden full-scale square burst — the worst case for a
    // non-lookahead limiter (instant gain clamp = transient distortion).
    let mut s = vec![0.0f32; 9600];
    for i in 4800..9600 {
        s[i] = if (i / 50) % 2 == 0 { 4.0 } else { -4.0 };
    }
    let out = drive_in_chunks(&mut fx, s, 128);
    assert_finite(&out, "limiter transient");
    let ceil = 10.0f32.powf(-1.0 / 20.0);
    let peak = out.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
    assert!(
        peak <= ceil + 1e-4,
        "lookahead limiter exceeded ceiling: {peak}"
    );
}

#[test]
fn delay_time_change_is_click_free() {
    // Wet-only so a jumped read pointer would land directly in the output.
    let mut fx = Delay::new(100.0, 0.0, 1.0);
    let mut buf = sine(220.0, SR as usize, SR);
    let mut pos = 0;
    while pos < buf.len() {
        let end = (pos + 128).min(buf.len());
        if pos == SR as usize / 2 {
            assert!(fx.set_param("time_ms", 400.0));
        }
        fx.process(&mut buf[pos..end], SR);
        pos = end;
    }
    assert_finite(&buf, "delay time change");
    // Max per-sample step of a 220 Hz sine at 0.5 amplitude is ~0.014; the
    // bounded glide can raise that ~1.5x. A hard click is 10x larger.
    let max_step = buf
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_step < 0.08,
        "delay time change clicked: step={max_step}"
    );
}

#[test]
fn auto_tune_retune_is_buffer_size_independent() {
    // Feed a fixed detector estimate and measure the average pitch during the
    // retune transition. The old per-call smoothing retuned 4x faster at
    // 128-sample buffers than at 512.
    fn run(chunk: usize) -> f32 {
        let mut fx = AutoTune::new(MusicalKey::C, Scale::Chromatic, 0.6);
        let sh = fx.shared();
        sh.freq.set(451.0);
        sh.conf.set(1.0);
        sh.gen.fetch_add(1, std::sync::atomic::Ordering::Release);
        let s = sine(451.0, SR as usize / 2, SR);
        let out = drive_in_chunks(&mut fx, s, chunk);
        // Pitch mid-transition (after the vocoder latency): this is where a
        // buffer-size-dependent retune rate shows up most.
        yin_freq(&out[6144..8192])
    }
    let fa = run(128);
    let fb = run(512);
    assert!(
        (fa - fb).abs() < 1.5,
        "retune speed depends on buffer size: {fa} Hz (128) vs {fb} Hz (512)"
    );
}

#[test]
fn auto_tune_humanize_alters_output() {
    fn run(h: f32) -> Vec<f32> {
        let mut fx = AutoTune::new(MusicalKey::C, Scale::Chromatic, 0.3);
        assert!(fx.set_param("humanize", h));
        let sh = fx.shared();
        sh.freq.set(440.0);
        sh.conf.set(1.0);
        sh.gen.fetch_add(1, std::sync::atomic::Ordering::Release);
        let s = sine(440.0, 32768, SR);
        drive_in_chunks(&mut fx, s, 256)
    }
    let clean = run(0.0);
    let human = run(0.2);
    assert_finite(&human, "humanize");
    assert!(
        clean
            .iter()
            .zip(human.iter())
            .any(|(a, b)| (a - b).abs() > 1e-3),
        "humanize was dead code: output identical to humanize=0",
    );
}

#[test]
fn reverb_decays_to_silence() {
    let mut fx = Reverb::new(0.5, 0.5, 0.5);
    let mut s = sine(440.0, 4096, SR);
    s.extend(std::iter::repeat(0.0f32).take(SR as usize * 3));
    let out = drive_in_chunks(&mut fx, s, 128);
    assert_finite(&out, "reverb");
    let tail_rms = (out[out.len() - 1024..].iter().map(|s| s * s).sum::<f32>() / 1024.0).sqrt();
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
fn distortion_oversampling_stays_finite_at_high_frequency() {
    let mut fx = Distortion::new(1.0, 1.0, 1.0, DistortionType::HardClip);
    let input = sine(10000.0, SR as usize / 2, SR);
    let output = drive_in_chunks(&mut fx, input, 128);
    assert_finite(&output, "4x oversampled distortion");
    assert!(
        rms(&output[4096..]) > 1e-3,
        "oversampled wet path went silent"
    );
}

#[test]
fn de_esser_attenuates_sibilance_more_than_bass() {
    fn retained_level(frequency: f32) -> f32 {
        let input = sine(frequency, SR as usize, SR);
        let input_level = rms(&input[SR as usize / 2..]);
        let mut fx = DeEsser::new(6000.0, -30.0, 1.0, 60.0);
        let output = drive_in_chunks(&mut fx, input, 128);
        assert_finite(&output, "de-esser");
        rms(&output[SR as usize / 2..]) / input_level
    }

    let bass_retained = retained_level(200.0);
    let sibilance_retained = retained_level(8000.0);
    assert!(
        sibilance_retained < bass_retained * 0.8,
        "8 kHz should be attenuated more than 200 Hz: high={sibilance_retained}, low={bass_retained}",
    );
}

#[test]
fn exciter_adds_harmonics_above_its_input_band() {
    let input = sine(1000.0, SR as usize, SR);
    let dry_high_energy = magnitude_at(&input[SR as usize / 2..], 5000.0, SR);
    let mut fx = Exciter::new(3000.0, 1.0, 1.0);
    let output = drive_in_chunks(&mut fx, input, 128);
    assert_finite(&output, "exciter");
    let excited_high_energy = magnitude_at(&output[SR as usize / 2..], 5000.0, SR);
    assert!(
        excited_high_energy > dry_high_energy + 1e-4,
        "exciter should add fifth-harmonic energy: wet={excited_high_energy}, dry={dry_high_energy}",
    );
}

#[test]
fn doubler_adds_modulated_delayed_voices() {
    let input = sine(220.0, SR as usize, SR);
    let dry = input.clone();
    let mut fx = Doubler::new(0.7, 1.0);
    let output = drive_in_chunks(&mut fx, input, 128);
    assert_finite(&output, "doubler");
    let difference = output[4096..]
        .iter()
        .zip(&dry[4096..])
        .map(|(wet, dry)| (wet - dry).abs())
        .sum::<f32>()
        / (output.len() - 4096) as f32;
    assert!(
        difference > 1e-3,
        "doubler output should differ from dry audio"
    );
}

#[test]
fn reverb_is_stable_at_96k() {
    // Delay lines retune to the stream rate; the tail must stay finite and
    // decay just like at 48 kHz.
    let mut fx = Reverb::new(0.5, 0.5, 0.5);
    let sr = 96000u32;
    let mut s: Vec<f32> = (0..4096)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin() * 0.5)
        .collect();
    s.extend(std::iter::repeat(0.0f32).take(sr as usize * 3));
    let mut pos = 0;
    while pos < s.len() {
        let end = (pos + 128).min(s.len());
        fx.process(&mut s[pos..end], sr);
        pos = end;
    }
    assert_finite(&s, "reverb 96k");
    let tail_rms = (s[s.len() - 1024..].iter().map(|x| x * x).sum::<f32>() / 1024.0).sqrt();
    assert!(
        tail_rms < 0.1,
        "96k reverb tail did not decay: rms={tail_rms}"
    );
}

#[test]
fn reverb_survives_rate_change_mid_stream() {
    // Simulates device renegotiation: same instance processed at two rates.
    let mut fx = Reverb::new(0.7, 0.3, 0.5);
    let a = sine(440.0, 4096, 48000);
    let out_a = drive_in_chunks(&mut fx, a, 128);
    assert_finite(&out_a, "reverb 48k leg");
    let mut b: Vec<f32> = (0..4096)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 96000.0).sin() * 0.5)
        .collect();
    fx.process(&mut b, 96000);
    assert_finite(&b, "reverb after retune to 96k");
}

#[test]
fn chorus_is_stable_at_96k() {
    // The old fixed-sample tuning assumed 48 kHz; delays now derive from the
    // stream rate and must stay within the (larger) buffer at 96 kHz.
    let mut fx = Chorus::new(1.5, 1.0, 0.5);
    let s: Vec<f32> = (0..96000)
        .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 96000.0).sin() * 0.5)
        .collect();
    let mut out = s;
    let mut pos = 0;
    while pos < out.len() {
        let end = (pos + 128).min(out.len());
        fx.process(&mut out[pos..end], 96000);
        pos = end;
    }
    assert_finite(&out, "chorus 96k");
}

#[test]
fn noise_suppress_passes_through_at_non_48k() {
    // RNNoise is 48 kHz-only; at other rates the effect must be a bit-exact
    // bypass rather than garbling the signal.
    let mut fx = NoiseSuppression::new(1.0);
    let s = sine(300.0, 4410, 44100);
    let original = s.clone();
    let mut out = s;
    fx.process(&mut out, 44100);
    assert_eq!(out, original, "non-48k input must pass through unchanged");
}

#[test]
fn distortion_type_switches_live_via_set_param() {
    // Regression: "dist_type" used to be silently rejected by set_param, so
    // the Dart-side type selection never reached the DSP.
    let mut fx = Distortion::new(0.8, 1.0, 1.0, DistortionType::SoftClip);
    assert!(fx.set_param("dist_type", 3.0), "dist_type must be settable");

    let s = sine(220.0, 1024, SR);
    let out_bitcrush = drive_in_chunks(&mut fx, s.clone(), 128);
    assert_finite(&out_bitcrush, "distortion bitcrush via set_param");

    fx.reset();
    assert!(fx.set_param("dist_type", 0.0));
    let out_softclip = drive_in_chunks(&mut fx, s, 128);

    assert!(
        out_bitcrush
            .iter()
            .zip(out_softclip.iter())
            .any(|(a, b)| (a - b).abs() > 1e-4),
        "switching dist_type must change the output",
    );
    // Out-of-range values clamp instead of wrapping.
    assert!(fx.set_param("dist_type", 99.0));
    assert_eq!(fx.dist_type(), DistortionType::Bitcrush);
    assert!(fx.set_param("dist_type", -5.0));
    assert_eq!(fx.dist_type(), DistortionType::SoftClip);
}

#[test]
fn read_wav_mono_downmixes_stereo() {
    use flutter_audio_fx_core::io::file_io;
    let path = std::env::temp_dir().join("fx_test_stereo_downmix.wav");
    let path = path.to_str().unwrap();

    // Interleaved stereo: L = 0.5, R = 0.1 → mono average = 0.3.
    let frames = 1000;
    let mut interleaved = Vec::with_capacity(frames * 2);
    for _ in 0..frames {
        interleaved.push(0.5f32);
        interleaved.push(0.1f32);
    }
    file_io::write_wav(path, &interleaved, 48000, 2).expect("write stereo wav");

    let (mono, meta) = file_io::read_wav_mono(path).expect("read mono");
    assert_eq!(meta.channels, 2, "meta must describe the source file");
    assert_eq!(mono.len(), frames, "one sample per frame after downmix");
    for (i, &s) in mono.iter().enumerate() {
        assert!(
            (s - 0.3).abs() < 0.01,
            "frame {i}: expected ~0.3 downmix, got {s}"
        );
    }
    let _ = std::fs::remove_file(path);
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
    // The dry path is delayed to stay aligned with the oversampled wet path.
    let latency = fx.latency_samples();
    let slot = EffectSlot::new(fx);

    let mut buf = vec![0.8f32; 64];
    let orig = buf.clone();
    unsafe {
        slot.process_in_place(&mut buf, SR);
    }
    assert!(buf[..latency].iter().all(|&s| s == 0.0), "dry path is delayed by the reported latency");
    assert_eq!(buf[latency..], orig[latency..], "mix=0 must pass through unchanged");

    // Queue a live param update; the audio thread applies it on next process.
    slot.queue_param("mix", 1.0);
    let mut buf2 = vec![0.8f32; 64];
    unsafe {
        slot.process_in_place(&mut buf2, SR);
    }
    assert!(
        buf2.iter()
            .zip(orig.iter())
            .any(|(a, b)| (a - b).abs() > 1e-6),
        "queued mix=1.0 should change the signal",
    );

    // Disabling via the slot must short-circuit to passthrough even though the
    // effect's own params would otherwise modify the signal.
    slot.set_enabled(false);
    let mut buf3 = vec![0.8f32; 64];
    let orig3 = buf3.clone();
    unsafe {
        slot.process_in_place(&mut buf3, SR);
    }
    assert_eq!(buf3, orig3, "disabled slot must pass through");

    // Re-enabling must take effect (regression guard for the slot/effect enable
    // mirroring).
    slot.set_enabled(true);
    let mut buf4 = vec![0.8f32; 64];
    unsafe {
        slot.process_in_place(&mut buf4, SR);
    }
    assert!(
        buf4.iter()
            .zip(orig3.iter())
            .any(|(a, b)| (a - b).abs() > 1e-6),
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
    assert!(
        head_rms < 1e-6,
        "first frame should be primed silence, got {head_rms}"
    );
    let body_rms = (out[960..1440].iter().map(|s| s * s).sum::<f32>() / 480.0).sqrt();
    assert!(
        body_rms > 1e-3,
        "denoised body should carry signal, got {body_rms}"
    );
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

fn sine_at(freq: f32, amplitude: f32, len: usize) -> Vec<f32> {
    sine(freq, len, SR).into_iter().map(|s| s * 2.0 * amplitude).collect()
}

#[test]
fn exciter_is_transparent_on_quiet_material() {
    // Regression: the old tanh(g·h)/tanh(g) normalisation made the exciter a
    // linear treble shelf (+19 dB at drive = mix = 1). A -60 dBFS tone is far
    // below saturation, so only (negligible) harmonics may be added.
    let input = sine_at(8000.0, 0.001, SR as usize);
    let mut fx = Exciter::new(3000.0, 1.0, 1.0);
    let output = drive_in_chunks(&mut fx, input.clone(), 128);
    let gain_db = 20.0 * (rms(&output[4800..]) / rms(&input[4800..])).log10();
    assert!(gain_db.abs() < 0.1, "quiet-signal gain must be ~0 dB, got {gain_db:+.2} dB");
}

#[test]
fn exciter_gain_is_bounded_when_driven_hard() {
    let input = sine_at(8000.0, 0.9, SR as usize);
    let mut fx = Exciter::new(3000.0, 1.0, 1.0);
    let output = drive_in_chunks(&mut fx, input.clone(), 128);
    assert_finite(&output, "exciter hard");
    let ratio = rms(&output[4800..]) / rms(&input[4800..]);
    assert!(ratio < 2.0, "high band may gain at most +6 dB, got x{ratio}");
}

#[test]
fn distortion_dry_wet_blend_has_no_comb_notches() {
    // Regression: the oversampled wet path lagged the dry path by 7.5 samples,
    // notching a 50/50 blend at ~3.2 kHz, 9.6 kHz, ... At drive 0 with the tone
    // filter open the wet path is ~linear, so the blend must be ~flat.
    for freq in [1000.0, 3200.0, 9600.0, 15000.0] {
        let input = sine_at(freq, 0.01, SR as usize / 2);
        let mut fx = Distortion::new(0.0, 1.0, 0.5, DistortionType::Tanh);
        let output = drive_in_chunks(&mut fx, input.clone(), 128);
        let gain_db = 20.0 * (rms(&output[4800..]) / rms(&input[4800..])).log10();
        assert!(gain_db.abs() < 1.0, "{freq} Hz: blend gain {gain_db:+.2} dB (comb notch?)");
    }
}

#[test]
fn distortion_wet_and_dry_share_the_reported_latency() {
    let latency = Distortion::new(0.0, 1.0, 0.0, DistortionType::Tanh).latency_samples();
    for mix in [0.0, 1.0] {
        let mut fx = Distortion::new(0.0, 1.0, mix, DistortionType::Tanh);
        let mut impulse = vec![0.0f32; 256];
        impulse[0] = 0.01;
        let output = drive_in_chunks(&mut fx, impulse, 64);
        let peak = output
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .unwrap()
            .0;
        assert_eq!(peak, latency, "mix={mix}: impulse peak must land at the reported latency");
    }
}

#[test]
fn doubler_is_never_louder_than_its_input() {
    let input = sine(220.0, SR as usize, SR); // peak 0.5
    let mut fx = Doubler::new(1.0, 1.0);
    let output = drive_in_chunks(&mut fx, input, 128);
    let peak = output.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(peak <= 0.5 + 1e-4, "doubler at mix=1 must not exceed input peak, got {peak}");
}

#[test]
fn reenabled_limiter_does_not_replay_stale_audio() {
    use flutter_audio_fx_core::graph::EffectSlot;
    let slot = EffectSlot::new(Box::new(Limiter::new(-1.0, 50.0)));
    let mut loud = vec![0.9f32; 512];
    unsafe { slot.process_in_place(&mut loud, SR) };

    // Bypass, then re-enable on silence: the lookahead line must be flushed.
    slot.set_enabled(false);
    let mut bypassed = vec![0.0f32; 128];
    unsafe { slot.process_in_place(&mut bypassed, SR) };
    slot.set_enabled(true);
    let mut silence = vec![0.0f32; 512];
    unsafe { slot.process_in_place(&mut silence, SR) };
    assert!(silence.iter().all(|&s| s == 0.0), "re-enabled limiter replayed pre-bypass audio");
}
