<p align="center">
  <img src="doc/images/banner.png" alt="flutter_audio_fx — real-time audio effects for Flutter" width="100%">
</p>

<h1 align="center">
  <img src="doc/images/icon.png" alt="" width="40" height="40" align="top">
  flutter_audio_fx
</h1>

<p align="center">
  <strong>Studio-grade, real-time voice effects for Flutter — powered by a Rust DSP core.</strong><br>
  Pitch shift, auto-tune, noise suppression, EQ, reverb and 14 effects total, with zero allocations on the audio thread.
</p>

<p align="center">
  <a href="https://pub.dev/packages/flutter_audio_fx"><img src="https://img.shields.io/pub/v/flutter_audio_fx.svg?label=pub&color=7c4dff" alt="pub version"></a>
  <a href="https://github.com/rickyirfandi/flutter_audio_fx/actions/workflows/build-native.yml"><img src="https://github.com/rickyirfandi/flutter_audio_fx/actions/workflows/build-native.yml/badge.svg" alt="build"></a>
  <img src="https://img.shields.io/badge/platforms-android%20%7C%20ios%20%7C%20macos%20%7C%20windows%20%7C%20linux-00bcd4" alt="platforms">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT license"></a>
</p>

<p align="center">
  <a href="#-quick-start">Quick start</a> ·
  <a href="#-effects">Effects</a> ·
  <a href="#-presets">Presets</a> ·
  <a href="#-platform-setup">Platform setup</a> ·
  <a href="#-performance">Performance</a> ·
  <a href="#-how-it-works">How it works</a>
</p>

---

## ✨ Why flutter_audio_fx?

- 🎛️ **14 production effects.** Auto-tune, pitch shift with formant
  preservation, RNNoise noise suppression, a 10-band EQ, dynamics, space,
  modulation and vocal polish, all chainable in any order.
- ⚡ **Real-time and glitch-free.** The audio callback never allocates, locks or
  does I/O. Knob changes are lock-free atomics that are smoothed per sample, so
  sliders don't produce zipper noise or clicks.
- 🦀 **Rust DSP core with no toolchain needed.** Precompiled binaries for every
  platform ship with the package. Add the dependency and run.
- 🎚️ **Live chain editing.** Reorder, toggle and retune effects while audio is
  running. Reverb tails keep ringing and the denoiser stays warmed up.
- 📁 **Offline rendering.** Process WAV files faster than real time on a worker
  isolate, with progress callbacks.
- 🎙️ **Tap recording.** Write the raw mic signal and/or the processed output to
  WAV while you monitor live.
- 📊 **Visualizer widgets.** Spectrum analyzer, waveform and a pitch/tuner
  indicator, ready to drop in.
- 🎭 **11 built-in presets.** T-Pain, Radio Host, Chipmunk, Podcast,
  Telephone, Robot and more.

## 📦 Installation

```yaml
dependencies:
  flutter_audio_fx: ^0.3.0
```

Or track the repository directly:

```yaml
dependencies:
  flutter_audio_fx:
    git:
      url: https://github.com/rickyirfandi/flutter_audio_fx.git
      ref: v0.3.0
```

Then follow [Platform setup](#-platform-setup) to enable microphone access.

## 🚀 Quick start

```dart
import 'package:flutter_audio_fx/flutter_audio_fx.dart';

final engine = AudioFxEngine();
await engine.init();

engine.setChain([
  NoiseSuppress(strength: 0.8),
  AutoTune(
    key: MusicalKey.C,
    scale: MusicalScale.major,
    correctionSpeed: 0.0, // hard snap: the classic T-Pain sound
  ),
  DeEsser(),
  Reverb(roomSize: 0.5, mix: 0.3),
  Limiter(ceilingDb: -1.0),
]);

await engine.startMic(); // mic → effects → speaker

// Tweak any parameter live; changes are smoothed, never clicky.
engine.updateParam(1, 'speed', 0.5);
engine.toggleEffect(3, false);

await engine.stop();
```

## 🎛️ Effects

| Category | Effect | Highlights |
|---|---|---|
| **Cleanup** | `NoiseGate` | Threshold, attack and release gating |
| | `NoiseSuppress` | RNNoise (pure-Rust port) neural denoiser |
| | `DeEsser` | Split-band sibilance control with an adjustable crossover |
| **Pitch** | `PitchShift` | Phase vocoder, ±12 semitones, formant preservation |
| | `AutoTune` | YIN detection, 12 keys, 7 scales, retune speed, humanize |
| **Tone** | `Equalizer` | 10-band parametric EQ |
| | `Exciter` | Adds high-frequency harmonics for presence and air |
| | `Distortion` | Soft, hard, tanh and bitcrush modes, 4x oversampled |
| **Dynamics** | `Compressor` | Soft knee, makeup gain, sidechain high-pass |
| | `Limiter` | 5 ms lookahead brick-wall limiter |
| **Space** | `Reverb` | Freeverb with pre-delay and damping |
| | `DelayEffect` | Up to 2 s with click-free time changes |
| **Modulation** | `Chorus` | Rate, depth and mix |
| | `Doubler` | Two modulated voices for double-tracked vocals |

Every effect is a plain Dart object. Construct it with named parameters, then
change it later with `engine.updateParam(index, name, value)`.

## 🎭 Presets

```dart
engine.setChain(BuiltInPresets.tpain);
```

| Preset | Sound |
|---|---|
| `tpain` | Hard auto-tune + compression |
| `gentleAutoTune` | Subtle pitch correction |
| `radioHost` | Clean broadcast voice |
| `podcast` | Professional voice cleanup |
| `chipmunk` | +1 octave pitch shift |
| `deepVoice` | Deep bass voice |
| `lofi` | Bitcrush + reverb aesthetic |
| `karaoke` | Reverb + delay for singing |
| `telephone` | Vintage phone filter |
| `robot` | Robotic vocoder effect |
| `echoChamber` | Heavy echo + reverb |

`BuiltInPresets.all` lists every preset with an id, name, emoji and
description, which is useful for building a picker UI.

## 🎙️ Recording

Tap the raw and/or processed signal to WAV while monitoring live. The files
are written on a worker thread, never on the audio thread.

```dart
await engine.startMicWithRecording(
  rawOutputPath: '/path/raw.wav',
  processedOutputPath: '/path/processed.wav', // optional
);
// ...
await engine.stop();
```

## 📁 Offline file processing

```dart
await engine.processFile(
  inputPath: 'recording.wav',
  outputPath: 'processed.wav',
  onProgress: (p) => print('${(p * 100).toStringAsFixed(0)}%'),
);
```

Rendering runs on a worker isolate, so the UI stays responsive. Multi-channel
input is downmixed to mono (the chain is mono) and written as mono WAV.
`previewFile(inputPath: ...)` plays a file through the chain in real time.

> MP3 export is reserved for a future release. `AudioFormat.mp3()` throws
> `UnsupportedError`.

## 📊 Visualizers

```dart
SpectrumVisualizer(stream: engine.spectrumStream, barCount: 32, height: 120)
WaveformVisualizer(stream: engine.waveformStream, height: 100)
PitchIndicator(stream: engine.pitchStream)
```

The raw data is also available as streams: `spectrumStream`, `waveformStream`,
`pitchStream` and `levelStream`.

## 🔧 Platform setup

| Platform | Minimum | Setup |
|---|---|---|
| **Android** | API 26 | `RECORD_AUDIO` is merged from the plugin manifest; request it at runtime (e.g. with [`permission_handler`](https://pub.dev/packages/permission_handler)). |
| **iOS** | 13.0 | Add `NSMicrophoneUsageDescription` to `ios/Runner/Info.plist`. |
| **macOS** | 10.14 | Add `NSMicrophoneUsageDescription` to `Info.plist` and the `com.apple.security.device.audio-input` entitlement. |
| **Windows** | 10 | None. |
| **Linux** | x64 | ALSA (`libasound2`) at runtime. |

## ⚡ Performance

| Metric | Target |
|---|---|
| Audio callback time | < 3 ms per buffer |
| CPU (all effects on, one channel) | < 20% of a mid-range core |
| Memory | < 50 MB total |
| Chain latency (non-FFT effects) | < 10 ms |

Some effects add their own delay. `engine.totalLatencyMs` reports the enabled
effects' total plus the audio buffer latency:

| Effect | Latency @ 48 kHz |
|---|---|
| Pitch Shift | 1536 samples (32 ms) |
| Auto-Tune | 3584 samples (~75 ms) |
| Noise Suppression | 480 samples (10 ms) |
| Limiter | 240 samples (5 ms) |
| Distortion | 16 samples (0.3 ms) |
| All others | 0 |

Pitch Shift and Auto-Tune keep their delay constant even at zero shift, so
correction engaging and disengaging never clicks.

## 🧠 How it works

```
Flutter (Dart)
  ├── AudioFxEngine            lifecycle, chain editing, visualizer polling
  ├── AudioEffect classes      typed, serializable effect configs
  └── Visualizer widgets
                │
                │  dart:ffi · flat C ABI (rust/src/api.rs)
                ▼
Rust DSP core
  ├── effects/                 14 effects implementing one AudioEffect trait
  ├── graph/                   EffectSlot, AtomicF32, SmoothedParam
  ├── analysis/                cached-FFT spectrum, YIN pitch detector
  ├── engine/runtime.rs        cpal streams, ArcSwap chain, worker threads
  └── io/file_io.rs            WAV reader/writer (hound)
```

On every buffer (≈ 2.7 ms at 128 samples / 48 kHz), the audio callback:

1. Loads the current chain with one atomic `ArcSwap` read.
2. Applies queued parameter changes from a lock-free SPSC queue.
3. Runs each effect in place over pre-allocated buffers.
4. Hands samples to worker threads (recording, pitch detection) via ring
   buffers.

No heap allocation, no blocking locks, no syscalls.

## 🛠️ Building from source

Package users don't need this: CI builds binaries for every platform on each
version tag. For contributors working on the Rust core in `rust/`:

```bash
make android   # → android/src/main/jniLibs/<abi>/libflutter_audio_fx_core.so
make ios       # → ios/flutter_audio_fx_core.xcframework (+ device .a)
make macos     # → macos/libflutter_audio_fx_core.a (universal)
make linux     # → linux/libflutter_audio_fx_core.so
make windows   # → windows/flutter_audio_fx_core.dll

cd rust && cargo test   # DSP test suite
flutter test            # Dart tests
```

Requires Rust ≥ 1.83, plus `cargo-ndk` and NDK r26+ for Android and Xcode 15+
for Apple platforms.

## 🤝 Contributing

Issues and pull requests are welcome at
[github.com/rickyirfandi/flutter_audio_fx](https://github.com/rickyirfandi/flutter_audio_fx/issues).
To add a new effect, see the checklist in [`CLAUDE.md`](CLAUDE.md#adding-a-new-effect).
Please keep all audio-thread code allocation-free and lock-free.

## 📄 License

MIT. See [LICENSE](LICENSE).
