# flutter_audio_fx — Product Requirements Document

**Version:** 0.1.0  
**Author:** Ricky  
**Date:** April 2026  
**Status:** MVP Development

---

## 1. Overview

flutter_audio_fx is an open-source Flutter package that provides real-time audio DSP (Digital Signal Processing) powered by a Rust native core. It enables Flutter developers to add professional-grade audio effects — pitch shifting, auto-tune, noise suppression, equalization, reverb, and more — to their apps with sub-5ms latency and zero garbage collection interference.

### 1.1 Problem Statement

Flutter lacks a performant audio processing solution. Existing options fall into three categories, all inadequate:

- **Pure Dart packages** — too slow for real-time processing; GC pauses cause audio glitches at 30fps+
- **Platform channel wrappers** — fragmented implementations across iOS/Android; inconsistent APIs; limited to platform-native capabilities
- **C/C++ FFI packages** — complex build setup; cross-compilation headaches; memory-unsafe

Dart is fundamentally incapable of real-time audio DSP due to its garbage collector, lack of SIMD access, and inability to share memory across isolates. Audio callbacks run on dedicated threads with hard latency deadlines (2-5ms). Any GC pause kills the audio stream.

### 1.2 Solution

A Rust-powered audio engine accessed via flutter_rust_bridge (FFI). Rust provides:

- Zero GC pauses (no garbage collector)
- Deterministic sub-millisecond processing
- Memory safety without runtime overhead
- Cross-compilation to Android (NDK) and iOS
- Lock-free atomic parameter updates

The package exposes a clean Dart API that hides all Rust complexity. Developers work with familiar Flutter patterns — no Rust knowledge required.

### 1.3 Target Users

- Flutter developers building voice/audio recording apps
- Content creation apps (TikTok-style, karaoke, podcasting)
- VoIP / WebRTC apps needing audio preprocessing
- Music production / DJ apps on mobile
- Accessibility apps needing real-time voice modification

---

## 2. Product Goals

### 2.1 Primary Goals

| Goal | Metric | Target |
|------|--------|--------|
| Real-time processing latency | Mic-to-speaker round-trip | <15ms |
| Audio callback budget | Per-buffer processing time | <3ms (128 samples @ 48kHz) |
| Effect count | Usable effects in chain | 11 simultaneous |
| Platform coverage | Supported platforms | Android (API 26+) + iOS (15+) |
| Developer experience | Lines to add an effect | <5 lines of Dart |

### 2.2 Non-Goals (MVP)

- Desktop support (macOS, Windows, Linux) — future release
- MP3 encoding (requires LAME C library; WAV export for MVP)
- Web support (WebAssembly Rust compilation is unstable for audio)
- MIDI input/output
- Multi-track recording

---

## 3. Features

### 3.1 Audio Effects (11 total)

| # | Effect | Description | Key Parameters |
|---|--------|-------------|---------------|
| 1 | Noise Gate | Silences audio below threshold | threshold_db, attack_ms, release_ms |
| 2 | Noise Suppression | Neural-network denoising (RNNoise) | strength (0-1) |
| 3 | Pitch Shift | Phase vocoder pitch modification | semitones (-12 to +12), cents, formant_preserve |
| 4 | Auto-Tune | Real-time pitch correction | key, scale, correction_speed (0=T-Pain, 1=off), humanize |
| 5 | Equalizer | 10-band parametric EQ | per-band: freq, gain_db, Q |
| 6 | Compressor | Dynamic range compression | threshold, ratio, attack, release, makeup_gain, knee |
| 7 | Limiter | Brick-wall peak limiter | ceiling_db, release_ms |
| 8 | Reverb | Freeverb algorithm | room_size, damping, mix, pre_delay_ms |
| 9 | Chorus | LFO-modulated delay | rate_hz, depth, mix |
| 10 | Delay | Feedback echo | time_ms, feedback, mix |
| 11 | Distortion | 4 waveshaper types | drive, tone, mix, type (soft/hard/tanh/bitcrush) |

### 3.2 Audio Engine

| Feature | Description |
|---------|-------------|
| Real-time mic processing | Mic → Effect Chain → Speaker |
| Real-time with recording | Mic → Chain → Speaker + save raw + save processed |
| Offline file processing | WAV file → Chain → WAV file (faster than real-time) |
| File preview | Load file → Chain → Speaker playback |
| Lock-free param updates | Change any knob in real-time without glitches |
| Composable chains | Stack any effects in any order; drag to reorder |

### 3.3 Visualization

| Widget | Data Source | Refresh Rate |
|--------|-----------|-------------|
| SpectrumVisualizer | FFT magnitude bins | 60fps |
| WaveformVisualizer | Raw sample stream | 60fps |
| PitchIndicator | YIN pitch detection | 60fps |
| Level meter | RMS audio level | 60fps |

### 3.4 Presets (11 built-in)

T-Pain, Radio Host, Chipmunk, Deep Voice, Lo-Fi, Karaoke, Podcast, Telephone, Robot, Echo Chamber, Gentle Auto-Tune. Each is a pre-configured effect chain with tuned parameters.

---

## 4. User Stories

### Developer Stories

| ID | Story | Priority |
|----|-------|----------|
| D1 | As a developer, I can add pitch shifting to my app with 3 lines of Dart | P0 |
| D2 | As a developer, I can compose an effect chain and reorder it at runtime | P0 |
| D3 | As a developer, I can update effect parameters in real-time with no audio glitches | P0 |
| D4 | As a developer, I can process a WAV file offline and get a processed WAV back | P0 |
| D5 | As a developer, I can display real-time spectrum/waveform visualizations | P1 |
| D6 | As a developer, I can use built-in presets without configuring individual effects | P1 |
| D7 | As a developer, I can serialize/deserialize effect chains to/from JSON | P1 |
| D8 | As a developer, I can get pitch detection data for tuner/karaoke UI | P2 |

---

## 5. Technical Constraints

### 5.1 Audio Thread Rules

The audio callback thread has a hard deadline. Violations cause audible clicks/pops.

**NEVER on the audio thread:**
- Heap allocation (Vec::push, Box::new, String::new)
- Mutex lock (blocking)
- File I/O, network, logging
- Any syscall that could block

**ALWAYS:**
- AtomicF32 for parameter reads
- try_lock() with instant fallback
- Pre-allocated buffers reused forever
- Ring buffers for inter-thread data

### 5.2 Platform Requirements

| Platform | Min Version | Audio Backend | Build Tool |
|----------|------------|---------------|------------|
| Android | API 26 | AAudio via cpal | cargo-ndk |
| iOS | 15.0 | CoreAudio via cpal | Xcode |

### 5.3 Dependencies

| Crate | Version | Purpose | Why Not Alternatives |
|-------|---------|---------|---------------------|
| cpal | 0.15 | Cross-platform audio I/O | Only mature option for Rust → Android/iOS |
| rustfft | 6.2 | FFT for pitch shift/analysis | Pure Rust, SIMD-optimized, no C deps |
| nnnoiseless | 0.5 | Noise suppression (RNNoise port) | Pure Rust — avoids C cross-compilation |
| ringbuf | 0.3 | Lock-free ring buffer | RT-safe, zero-alloc |
| hound | 3.5 | WAV read/write | Lightweight, stable |

---

## 6. Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Full chain latency | <15ms | Loopback test with all 11 effects |
| CPU usage (full chain) | <20% | Single core, Snapdragon 680 |
| Memory usage | <50MB | Engine + all buffers |
| APK size increase | <8MB | Rust .so libraries |
| Cold start | <500ms | Engine initialization |
| Offline render speed | >10x realtime | 1 min file in <6 seconds |
| Pub.dev score | ≥130/160 | Package analysis |

---

## 7. Risks & Mitigations

| Risk | Impact | Likelihood | Mitigation |
|------|--------|-----------|------------|
| cpal Android support flaky | Can't capture mic | Medium | Fallback to OpenSL ES; test on 10+ devices |
| nnnoiseless model accuracy | Poor noise removal | Low | Ship with custom-trained model if needed |
| Phase vocoder artifacts | Chipmunk sounds robotic | Medium | Add formant preservation; tune window size |
| flutter_rust_bridge breaking changes | Build breaks | Low | Pin exact version (=2.9.0) |
| APK size too large | Users won't install | Low | LTO + strip in release; split ABIs |

---

## 8. Roadmap

### v0.1.0 — MVP (Current)
- 11 effects with full parameter control
- Real-time mic processing + file processing
- WAV export
- 11 built-in presets
- 3 visualizer widgets

### v0.2.0 — Polish
- MP3 export via LAME
- Custom preset save/load
- Formant-preserving pitch shift
- Desktop support (macOS, Windows)

### v0.3.0 — Advanced
- WebRTC integration (PCM stream output)
- Multi-track recording
- Beat/tempo detection
- Custom effect plugin API

### v1.0.0 — Stable
- Full test coverage
- Performance benchmarks published
- Pub.dev publication
- Documentation site
