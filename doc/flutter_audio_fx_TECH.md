# flutter_audio_fx — Technical Documentation

**Version:** 0.1.0  
**Last Updated:** April 2026

---

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                   Flutter App (Dart)                      │
│                                                          │
│  AudioFxEngine ← setChain / updateParam / startMic      │
│       │                                                  │
│       │ Dart → Rust FFI via flutter_rust_bridge           │
│       ▼                                                  │
│  ┌──────────────────────────────────────────────────┐    │
│  │              Rust API Layer (api.rs)              │    │
│  │  engine_init / engine_set_chain / engine_start    │    │
│  │  engine_get_spectrum / engine_update_param        │    │
│  └───────────────────┬──────────────────────────────┘    │
│                      │                                   │
│  ┌───────────────────┴──────────────────────────────┐    │
│  │           AudioRuntime (engine/runtime.rs)        │    │
│  │                                                   │    │
│  │  ┌─────────┐   ring    ┌──────────┐   cpal       │    │
│  │  │ Mic     │──buffer──▶│ Process  │──stream──▶Speaker │
│  │  │ (cpal)  │           │ Chain    │              │    │
│  │  └─────────┘           └──────────┘              │    │
│  │       │                     │                     │    │
│  │       ▼                     ▼                     │    │
│  │  raw_rec_buf           proc_rec_buf               │    │
│  │  (recording)           (recording)                │    │
│  └──────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

The system has three layers: the Dart API (what developers use), the Rust API bridge (FFI boundary), and the Rust audio runtime (real-time processing).

---

## 2. Module Map

### 2.1 Rust Crate (`rust/src/`)

```
lib.rs                          Module root
├── api.rs                      FRB public API (23 functions)
├── graph/mod.rs                AudioEffect trait + AtomicF32 + impl_effect_meta! macro
├── effects/
│   ├── mod.rs                  Re-exports all effects
│   ├── noise_gate.rs           Envelope-following gate
│   ├── noise_suppress.rs       RNNoise neural network (nnnoiseless)
│   ├── pitch_shift.rs          Phase vocoder (STFT overlap-add)
│   ├── auto_tune.rs            YIN detection + scale quantizer + pitch shift
│   ├── equalizer.rs            N-band parametric biquad EQ
│   ├── compressor.rs           Soft-knee dynamic range compression
│   ├── limiter.rs              Brick-wall peak limiter
│   ├── reverb.rs               Freeverb (8 comb + 4 allpass)
│   ├── chorus.rs               LFO-modulated delay line
│   ├── delay.rs                Feedback delay
│   └── distortion.rs           Waveshaper (4 types)
├── engine/
│   ├── config.rs               EngineConfig, AudioFormat, AudioInput/Output enums
│   └── runtime.rs              cpal audio loop, recording, file processing
├── analysis/
│   └── spectrum.rs             FFT analyzer, SpectrumData, WaveformData, PitchData
├── io/
│   └── file_io.rs              WAV read/write (hound)
└── util/mod.rs                 db_to_linear, rms, midi_to_freq helpers
```

### 2.2 Dart Package (`lib/src/`)

```
flutter_audio_fx.dart           Package exports
├── engine/
│   ├── audio_fx_engine.dart    Main engine class (lifecycle, chain, viz streams)
│   ├── engine_config.dart      EngineConfig (sample_rate, buffer_size)
│   └── audio_format.dart       AudioFormat sealed class (WAV, MP3)
├── effects/
│   ├── effect.dart             Base AudioEffect abstract class
│   ├── noise_gate.dart         NoiseGate
│   ├── noise_suppress.dart     NoiseSuppress
│   ├── pitch_shift.dart        PitchShift
│   ├── auto_tune.dart          AutoTune + MusicalKey + MusicalScale enums
│   ├── equalizer.dart          Equalizer + EqBand
│   ├── compressor.dart         Compressor
│   ├── limiter.dart            Limiter
│   ├── reverb.dart             Reverb
│   ├── chorus.dart             Chorus
│   ├── delay_effect.dart       DelayEffect
│   └── distortion.dart         Distortion + DistortionType enum
├── models/
│   ├── spectrum_data.dart      SpectrumData (FFT magnitudes, RMS, dominant freq)
│   ├── pitch_data.dart         PitchData (Hz, note name, cents off)
│   ├── preset.dart             Preset model (serializable)
│   └── presets_builtin.dart    11 built-in presets
└── widgets/
    ├── spectrum_visualizer.dart SpectrumVisualizer (CustomPainter bars)
    ├── waveform_visualizer.dart WaveformVisualizer (CustomPainter line)
    └── pitch_indicator.dart     PitchIndicator (note + cents gauge)
```

---

## 3. Core Design Patterns

### 3.1 AudioEffect Trait (Rust)

Every effect implements this trait. The `impl_effect_meta!` macro generates boilerplate for `effect_type`, `set_enabled`, `is_enabled`, and `set_param`.

```rust
pub trait AudioEffect: Send + Sync {
    fn process(&mut self, buffer: &mut [f32], sample_rate: u32);
    fn effect_type(&self) -> EffectType;
    fn latency_samples(&self) -> usize { 0 }
    fn reset(&mut self);
    fn set_enabled(&self, enabled: bool);
    fn is_enabled(&self) -> bool;
    fn set_param(&self, name: &str, value: f32) -> bool;
}
```

All mutable state that the UI thread needs to modify (parameters, enabled flag) uses `AtomicF32` / `AtomicEnabled` — lock-free, wait-free, RT-safe.

### 3.2 Global Singleton (Rust → Dart Bridge)

```rust
static RUNTIME: Mutex<Option<AudioRuntime>> = Mutex::new(None);
```

Uses `Mutex<Option<>>` (not `OnceLock`) so `engine_init()` can be called multiple times to re-initialize with different configs. Helper functions `with_rt()` and `with_rt_mut()` handle locking and error propagation.

### 3.3 Audio Callback Threading

```
┌──────────────┐   lock-free    ┌────────────────────┐
│  UI Thread   │◄──ring buffer─►│  Audio Thread (RT)  │
│  (Dart)      │   + atomics    │  (Rust/cpal)        │
│              │                │                     │
│  slider →    │  AtomicF32     │  reads param        │
│  toggle →    │  AtomicBool    │  reads enabled      │
│  reorder →   │  Mutex         │  try_lock (skip if  │
│              │  (rare)        │  contended)         │
└──────────────┘                └────────────────────┘
```

The audio callback in `runtime.rs` uses `chain.try_lock()` — if the Dart side is rebuilding the chain (rare), the audio thread skips processing for that buffer rather than blocking.

### 3.4 Visualization Data Flow

```
Audio Thread                    UI Thread
─────────                       ─────────
process(buffer)
  → analyzer.feed(buffer)
  → spectrum_data = Arc<Mutex<Option<SpectrumData>>>
                                Timer(16ms) polls:
                                  engine_get_spectrum()
                                    → takes spectrum from Mutex
                                  → StreamController<SpectrumData>
                                    → SpectrumVisualizer widget rebuilds
```

Polling at 60fps from Dart. The Rust side overwrites the spectrum data each time — Dart gets the latest frame, not a backlog.

---

## 4. Effect Implementations

### 4.1 Pitch Shift — Phase Vocoder

Algorithm: STFT (Short-Time Fourier Transform) with overlap-add.

```
Input → Window (Hann, 2048pt, 75% overlap)
      → FFT
      → Phase Analysis (instantaneous frequency per bin)
      → Bin shifting (multiply bin index by pitch ratio)
      → Phase Reconstruction (accumulate shifted phases)
      → iFFT
      → Overlap-Add → Output
```

Latency: 2048 samples (42.7ms @ 48kHz). Uses `rustfft` (pure Rust, SIMD-optimized). The FIFO-based implementation avoids the circular buffer complexity of traditional phase vocoders.

### 4.2 Auto-Tune — YIN + Scale Quantizer

Three-stage pipeline:

1. **YIN Pitch Detection** — Computes autocorrelation difference function, applies cumulative mean normalization, finds first dip below threshold (0.15), walks forward to local minimum, parabolic interpolation for sub-sample accuracy. Detects 50-500Hz (human voice range).

2. **Scale Quantizer** — Maps detected frequency to MIDI note number, finds nearest note in the configured key+scale (7 scales × 12 keys = 84 combinations), computes target frequency.

3. **Correction** — Calculates semitone shift needed, smooths via `correction_speed` parameter (0.0 = instant T-Pain snap, 0.5 = fast natural, 1.0 = bypass). Feeds into internal PitchShift instance.

### 4.3 Noise Suppression — RNNoise

Uses `nnnoiseless` (pure Rust port of Mozilla's RNNoise). Processes in 480-sample frames (10ms @ 48kHz). Pre-trained recurrent neural network runs at ~2% CPU on mid-range devices.

Input is scaled to [-32768, 32767] (RNNoise expects 16-bit range), processed, scaled back. `strength` parameter cross-fades between clean and original signal.

### 4.4 Reverb — Freeverb

Schroeder-Moorer reverberator: 8 parallel comb filters (with lowpass damping) → 4 series allpass filters. Comb filter sizes tuned for 48kHz (scaled from original 44.1kHz Freeverb constants). Pre-delay implemented as simple circular buffer.

### 4.5 Equalizer — Biquad Filters

N-band parametric EQ using cascaded biquad (2nd-order IIR) filters in Direct Form II Transposed. Peaking EQ coefficients recalculated only when parameters change (dirty flag optimization). Each band stores previous freq/gain/Q and skips coefficient computation if unchanged.

---

## 5. Data Flow

### 5.1 Real-Time Mode

```
Mic (cpal input callback)
  → Ring buffer (HeapRb, 50ms, lock-free)
    → Output callback reads from ring buffer
      → Effect chain: for each effect, process(buffer, sample_rate)
        → Only if effect.is_enabled()
        → Skipped if chain.try_lock() fails (rare)
      → Spectrum analyzer: feed(buffer)
      → RMS level calculation
      → If recording: extend raw_rec_buf + proc_rec_buf
    → cpal output → Speaker
```

### 5.2 Recording Mode

Identical to real-time, plus:
- Input callback also writes raw mic data to `raw_rec_buf`
- Output callback writes processed data to `proc_rec_buf`
- On `stop()`, both buffers are written to WAV files via `file_io::write_wav()`

### 5.3 Offline File Processing

```
file_io::read_wav(input)
  → Vec<f32> samples
  → Process in 512-sample chunks through chain
  → file_io::write_wav(output)
```

Runs on calling thread (not audio thread), faster than real-time because no I/O latency.

---

## 6. FFI Bridge (api.rs)

### 6.1 Function Inventory

| Function | Sync? | Returns | Purpose |
|----------|-------|---------|---------|
| `engine_init` | sync | bool | Initialize/reinitialize engine |
| `engine_get_state` | sync | EngineStateDto | Query running state |
| `engine_set_chain` | sync | bool | Rebuild entire effect chain |
| `engine_toggle_effect` | sync | bool | Enable/disable one effect |
| `engine_update_param` | sync | bool | Update one atomic parameter |
| `engine_add_effect` | sync | bool | Append effect to chain |
| `engine_remove_effect` | sync | bool | Remove effect by index |
| `engine_reorder_effect` | sync | bool | Move effect in chain |
| `engine_start_realtime` | async | Result | Start mic → speaker |
| `engine_start_realtime_with_recording` | async | Result | Start mic → speaker + files |
| `engine_stop` | async | Result | Stop all audio |
| `engine_process_file` | async | Result<String> | Offline file processing |
| `engine_preview_file` | async | Result | Play processed file |
| `engine_get_spectrum` | sync | Option<SpectrumDto> | Poll FFT data |
| `engine_get_pitch` | sync | PitchDto | Poll pitch detection |
| `engine_get_level` | sync | f32 | Poll RMS level |

### 6.2 DTOs

```rust
EffectConfigDto { effect_type: String, enabled: bool, params: Vec<ParamDto> }
ParamDto { name: String, value: f64 }
SpectrumDto { magnitudes: Vec<f32>, dominant_freq: f32, rms: f32, bin_count: i32 }
PitchDto { pitch_hz: f32, confidence: f32, note_name: String, cents_off: f32 }
EngineStateDto { is_running: bool, mode: String, chain_latency_ms: f32, ... }
```

### 6.3 Effect Factory

`build_one_effect(cfg)` maps `effect_type` string to Rust struct constructor, applies all params from the DTO, and sets the enabled flag. Supports all 11 effect types.

---

## 7. Build System

### 7.1 Rust Cross-Compilation

```
flutter_rust_bridge_codegen generate
  → Generates Dart bindings (lib/src/rust/frb_generated.dart)
  → Generates Rust glue (rust/src/frb_generated.rs)
  → Configures Cargokit for Android/iOS builds
```

Cargokit (bundled with flutter_rust_bridge) handles:
- Android: `cargo ndk` → `.so` for aarch64, armv7, x86_64
- iOS: `cargo build --target aarch64-apple-ios` → `.a` static lib

### 7.2 Release Profile

```toml
[profile.release]
opt-level = 3       # Maximum optimization
lto = "fat"         # Full link-time optimization
codegen-units = 1   # Better optimization at cost of compile time
strip = true        # Remove debug symbols
panic = "abort"     # No unwinding overhead
```

---

## 8. Performance Characteristics

| Operation | Budget | Actual (est.) | Notes |
|-----------|--------|--------------|-------|
| NoiseGate | <0.01ms | ~0.005ms | Simple envelope + multiply |
| NoiseSuppression | <0.5ms | ~0.3ms | RNNoise neural net, 10ms frames |
| PitchShift | <2ms | ~1.5ms | 2048pt FFT + phase vocoder |
| AutoTune | <3ms | ~2ms | YIN (2048pt) + PitchShift |
| Equalizer (10 band) | <0.1ms | ~0.05ms | 10× biquad, SIMD-friendly |
| Compressor | <0.01ms | ~0.005ms | Simple envelope + gain |
| Reverb | <0.5ms | ~0.3ms | 8 comb + 4 allpass |
| Full chain (all 11) | <5ms | ~4ms | Summed |
| Spectrum FFT | <0.5ms | ~0.2ms | 2048pt, non-blocking |

Memory per effect: 8-400KB (reverb largest due to comb filter buffers). Total engine memory: ~20-50MB including all buffers.

---

## 9. Error Handling Strategy

| Layer | Strategy | Example |
|-------|----------|---------|
| Audio callback | `try_lock()` — skip frame if contended | Chain rebuild during playback |
| Rust API | `Result<T, String>` — all errors propagated | Device not found, file not readable |
| Ring buffer | `try_push()` / `try_pop()` — drop if full/empty | Mic faster than output |
| Dart engine | try/catch with debugPrint fallback | Start fails, falls back to mic-only |
| Recording | Saves whatever was captured; empty = no file | Short recording, no crash |

No `unwrap()` calls anywhere in the Rust codebase. No panics possible from normal operation.
