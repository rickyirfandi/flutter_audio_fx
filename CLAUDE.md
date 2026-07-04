# CLAUDE.md — flutter_audio_fx Development Guide

## Project Overview
A Flutter package providing real-time audio DSP powered by Rust. The Dart ↔ Rust
boundary is a hand-written flat C ABI (`rust/src/api.rs`) bound via `dart:ffi`
(`lib/src/ffi/bindings.dart`) — NOT flutter_rust_bridge.

## Architecture

### Dart Layer (`lib/`)
- `AudioFxEngine` — main entry point, manages lifecycle + chain
- `AudioEffect` subclasses — Dart models with params, serializable to JSON
- Widget classes — `SpectrumVisualizer`, `WaveformVisualizer`, `PitchIndicator`
- `BuiltInPresets` — ready-to-use effect chain presets

### Rust Layer (`rust/src/`)
- `effects/` — each effect implements `AudioEffect` trait
- `engine/audio_engine.rs` — manages cpal I/O + effect chain processing
- `graph/mod.rs` — `AudioEffect` trait, `AtomicF32`, `SmoothedParam`
- `analysis/spectrum.rs` — FFT analyzer for UI visualizers
- `io/` — mic input, speaker output, file I/O via cpal + hound

### Key Design Decisions
1. **Lock-free audio thread**: Audio callback must NEVER allocate, lock, or do I/O
2. **AtomicF32 for params**: UI thread sets atomics, audio thread reads them — zero glitches
3. **nnnoiseless** over rnnoise-c: Pure Rust port, no C cross-compilation headaches
4. **Phase vocoder** for pitch shift: FFT-based, supports formant preservation
5. **YIN algorithm** for pitch detection: Robust, well-studied, good for voice
6. **Freeverb** for reverb: Classic Schroeder-Moorer, tuned for 48kHz

## Build Commands

The Dart bindings are hand-written (`lib/src/ffi/bindings.dart`); there is no
codegen step. When you add/change a C ABI export in `rust/src/api.rs`, update the
matching `lookupFunction` binding by hand and keep the signatures in sync.

```bash
# Build native cores for every platform (see Makefile)
make android   # cargo-ndk → android/src/main/jniLibs/<abi>/
make ios       # → ios/ (xcframework / universal .a)
make macos     # → macos/libflutter_audio_fx_core.a
make linux     # → linux/libflutter_audio_fx_core.so
make windows   # → windows/flutter_audio_fx_core.dll

# Run example app
cd example && flutter run
```

> On Android the Kotlin `FlutterAudioFxPlugin` loads the core via
> `System.loadLibrary` and calls `nativeAttachContext` so cpal's oboe backend
> can reach the JavaVM/Context (`rust/src/android.rs`). Don't remove it.

## Adding a New Effect

### Rust side:
1. Create `rust/src/effects/my_effect.rs`
2. Implement `AudioEffect` trait (must be `Send + Sync`)
3. Add to `effects/mod.rs` exports
4. Add `EffectType::MyEffect` variant to `graph/mod.rs`
5. Add builder case to `engine/audio_engine.rs::create_effect()`

### Dart side:
1. Create `lib/src/effects/my_effect.dart`
2. Extend `AudioEffect`, implement `toParams()` and `updateParam()`
3. Export from `lib/flutter_audio_fx.dart`
4. Add to any relevant presets in `models/presets_builtin.dart`

## Audio Thread Rules (CRITICAL)
The audio callback runs with a hard deadline of ~2.67ms (128 samples @ 48kHz).

**NEVER do these on the audio thread:**
- `Vec::push()`, `Box::new()`, `String::new()` — heap allocation
- `Mutex::lock()`, `RwLock::write()` — blocking
- `File::open()`, `println!()` — I/O
- Any syscall that could block

**ALWAYS use:**
- `AtomicF32::get()` — for reading params
- `try_lock()` — if you must lock, with instant fallback
- Pre-allocated buffers — sized at init, reused forever
- `ringbuf` — for lock-free data passing between threads

## Testing
```bash
# Rust unit tests
cd rust && cargo test

# Dart unit tests
flutter test

# Integration test (requires device)
cd example && flutter test integration_test/
```

## Performance Targets
| Metric | Target |
|--------|--------|
| Full chain latency | <10ms |
| Audio callback time | <3ms per buffer |
| CPU usage (all effects on) | <20% single core |
| Memory | <50MB total |
