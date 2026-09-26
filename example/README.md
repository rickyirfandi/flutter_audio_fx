# flutter_audio_fx example

A demo app for the [`flutter_audio_fx`](https://pub.dev/packages/flutter_audio_fx)
package: record your voice through a live effect chain, tweak every parameter
while it runs, and export the result.

## What it shows

- Building an effect chain and editing it live (reorder, toggle, retune).
- Loading the built-in presets, such as T-Pain, Podcast and Telephone.
- Driving the `SpectrumVisualizer`, `WaveformVisualizer` and `PitchIndicator`
  widgets from the engine's streams.
- Recording the raw and processed signals to WAV, then exporting.
- Requesting microphone permission before starting the engine.

## Run it

```bash
cd example
flutter run            # or: flutter run -d windows / macos / linux
```

The package ships precompiled native binaries, so no Rust toolchain is needed.

## Where to look

| Path | Contents |
|---|---|
| `lib/controllers/app_controller.dart` | Engine lifecycle, chain edits, recording |
| `lib/screens/editor_screen.dart` | Effect list, parameter sliders, add-effect sheet |
| `lib/widgets/visualizer_panel.dart` | Spectrum, waveform and pitch widgets |
| `lib/screens/export_screen.dart` | Offline file processing with progress |
