# flutter_audio_fx

High-performance real-time audio DSP for Flutter, powered by Rust.

Pitch shift, auto-tune, noise suppression, EQ, reverb, and 14 effects total —
low-latency chains, zero allocations on the audio thread, lock-free
parameter updates.

## Features

- **14 audio effects**: Noise Gate, RNNoise-based Noise Suppression, Pitch
  Shift, Auto-Tune, 10-band Equalizer, Compressor (with sidechain HPF),
  lookahead Limiter, Reverb, Chorus, Delay, Distortion (soft/hard/tanh/bitcrush,
  4x oversampled), De-Esser, Exciter, Doubler.
- **Real-time mic → effects → speaker** with cpal on every desktop and mobile
  platform Flutter supports.
- **Offline file processing** (WAV in, WAV out) — faster than real-time.
- **Composable effect chains** — reorder, toggle, swap atomically.
- **Lock-free parameter updates** — every knob is an `AtomicF32`; writes from
  the UI never glitch the audio thread.
- **Built-in presets**: T-Pain, Radio Host, Chipmunk, Deep Voice, Lo-Fi,
  Karaoke, Podcast, Telephone, Robot, Echo Chamber, Gentle Auto-Tune.
- **Visualizer widgets**: spectrum analyzer, waveform, pitch indicator.
- **Tap recording** — write raw mic input and/or processed output to WAV
  while the live chain is running, on a worker thread.

## Installation

Precompiled native binaries for all five platforms are committed to this repo
by CI (built on every version tag), so **no Rust toolchain is needed** — add
the dependency, `flutter pub get`, and run:

```yaml
dependencies:
  flutter_audio_fx:
    git:
      url: https://github.com/rickyirfandi/flutter_audio_fx.git
      ref: master # or a version tag, e.g. v0.3.0
```

## Quick start

```dart
import 'package:flutter_audio_fx/flutter_audio_fx.dart';

final engine = AudioFxEngine();
await engine.init();

engine.setChain([
  NoiseSuppress(strength: 0.8),
  AutoTune(
    key: MusicalKey.C,
    scale: MusicalScale.major,
    correctionSpeed: 0.0, // T-Pain mode
  ),
  Reverb(roomSize: 0.5, mix: 0.3),
  Limiter(ceilingDb: -1.0),
]);

await engine.startMic();

// Update a param without glitches.
engine.updateParam(1, 'speed', 0.5);

await engine.stop();
```

### Presets

```dart
engine.setChain(BuiltInPresets.tpain);
engine.setChain(BuiltInPresets.podcast);
engine.setChain(BuiltInPresets.lofi);
```

### Recording

```dart
await engine.startMicWithRecording(
  rawOutputPath: '/path/raw.wav',
  processedOutputPath: '/path/processed.wav',
);
// ...
await engine.stop();
```

### Offline file processing

```dart
await engine.processFile(
  inputPath: 'recording.wav',
  outputPath: 'processed.wav',
  format: AudioFormat.wav(),
  onProgress: (p) => print('${(p * 100).toStringAsFixed(0)}%'),
);
```

Processing runs on a worker isolate, so the UI stays responsive; `onProgress`
reports real progress. Multi-channel input is downmixed to mono (the effect
chain is mono) and the output is written as mono WAV.

> MP3 export is reserved for a future release. Calling `processFile` with
> `AudioFormat.mp3()` throws `UnsupportedError`.

### Visualizers

```dart
SpectrumVisualizer(
  stream: engine.spectrumStream,
  barCount: 32,
  barColor: Colors.cyan,
  height: 120,
)
WaveformVisualizer(
  stream: engine.waveformStream,
  lineColor: Colors.white,
  height: 100,
)
PitchIndicator(
  stream: engine.pitchStream,
  activeColor: Colors.green,
)
```

## Building the native core

**Package consumers don't need any of this.** Precompiled binaries for every
platform (Android arm64-v8a / armeabi-v7a / x86_64, iOS xcframework, macOS
universal, Linux x64, Windows x64) are built by CI on every version tag and
committed into the repo — `flutter pub get` and run, no Rust toolchain
required.

For contributors working on the Rust core: the engine is a crate at `rust/`.
Use the Makefile to build the artefacts and place them where each platform
expects:

```bash
make android   # → android/src/main/jniLibs/<abi>/libflutter_audio_fx_core.so
make ios       # → ios/libflutter_audio_fx_core.a (universal)
make macos     # → macos/libflutter_audio_fx_core.a (universal)
make linux     # → linux/libflutter_audio_fx_core.so
make windows   # → windows/flutter_audio_fx_core.dll
```

You will need:

- The **Rust toolchain** (`rust-toolchain` ≥ 1.80) and the relevant cross
  targets (`cargo-ndk` for Android, the Apple SDKs for iOS/macOS).
- **Android NDK r26+**, `minSdk 26` (AAudio).
- **iOS 13+**, Xcode 15+. Add `NSMicrophoneUsageDescription` to your host
  app's `Info.plist`.

## Architecture

```
Flutter (Dart)
  ├── AudioFxEngine            (lifecycle, chain editing, viz polling)
  ├── AudioEffect classes      (Dart-side typed configs)
  └── Visualizer widgets
                │
                │  dart:ffi (flat C ABI — see rust/src/api.rs)
                │
Rust DSP core
  ├── effects/                 (NoiseGate, PitchShift, AutoTune, …)
  ├── graph/                   (AudioEffect trait, EffectSlot, AtomicF32)
  ├── analysis/                (cached-FFT spectrum, YIN pitch detector)
  ├── engine/runtime.rs        (cpal streams, ArcSwap chain, worker threads)
  └── io/file_io.rs            (WAV reader/writer via hound)
```

### Audio-thread invariants

The cpal callback (≈ 2.7 ms deadline at 128 samples / 48 kHz) does only:

1. Drain the mic ringbuf into the output buffer.
2. `ArcSwap::load` the current chain (one atomic read).
3. Call `process` on every effect — all working buffers are pre-allocated
   in `new()`.
4. Push processed samples into ringbufs consumed by writer threads (recording)
   and worker threads (pitch detection).
5. Try-lock the spectrum slot and store an RMS atomic.

No heap allocation, no mutex blocking, no syscalls.

## Performance targets

| Metric                          | Target |
|---------------------------------|--------|
| Full chain latency              | < 10 ms |
| Audio callback time             | < 3 ms per buffer |
| CPU (all effects on, 1 channel) | < 20% on a single mid-range core |
| Memory                          | < 50 MB total |

The latency target applies to chains without the FFT-based effects. Each
effect's own delay (reported via `fx_engine_chain_latency_ms`, counting only
enabled effects):

| Effect | Latency @ 48 kHz |
|---|---|
| Pitch Shift | 1536 samples (32 ms) — always, even at 0 semitones |
| Auto-Tune | 3584 samples (~75 ms) — always, even when not correcting |
| Noise Suppression | 480 samples (10 ms) |
| Limiter | 240 samples (5 ms) |
| Distortion | 16 samples (0.3 ms) |
| All others | 0 |

Pitch Shift and Auto-Tune keep their delay constant instead of bypassing at
unity, so correction engaging/disengaging never clicks.

## License

MIT.
