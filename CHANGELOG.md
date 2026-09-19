# Changelog

## Unreleased

- Added opt-in Dart-to-Rust integration tests for worker-isolate exports,
  queued settings, offline pitch correction, and file-error recovery. CI runs
  them against the current Rust source.

- Export and preview apply queued effect settings, including reused preset
  slots, before rendering. Offline auto-tune now analyzes the source audio.
- Offline renders and stream lifecycle operations are serialized to prevent
  concurrent access to effect state.
- Recording file-creation errors fail startup; write/finalization failures are
  returned by `stop()`. The example reports these failures without adding a
  failed recording to its library.
- Waveform events no longer require a spectrum-stream subscriber.
- Retained the example's legacy reorder callback for Flutter 3.27 compatibility
  with a targeted deprecation suppression.

## 0.3.0

### Added

- **Split-band de-esser** with adjustable crossover, threshold, attenuation
  amount, and release.
- **Harmonic exciter** with high-band drive and a smoothed wet mix. Adds only
  harmonics: quiet material passes at unity, and the high band gains at most
  +6 dB even at full drive and mix.
- **Vocal doubler** with two independently modulated fractional-delay voices,
  crossfaded against the dry signal so it is never louder than its input.
- **4x distortion oversampling** (65-tap Kaiser polyphase, -55 dB at 28 kHz)
  to reduce nonlinear aliasing. The dry path is delayed to match the wet
  path, so dry/wet blends don't comb-filter; Distortion now reports 16
  samples of latency.
- **Compressor sidechain high-pass filter** to reduce bass-triggered pumping.

### Fixed (DSP quality)

- **Phase-vocoder output FIFO read bug.** The pitch shifter read the
  freshly-cleared end of the overlap-add accumulator instead of the fully
  accumulated start, so all shifted audio was ~27 dB too quiet with heavy
  windowing tremolo. Also corrected the OLA gain constant (Hann² at 75%
  overlap sums to 1.5, not 2.0). Unity reconstruction is now transparent
  (YIN reads 220.002 Hz at conf 0.99999 on a 220 Hz probe).
- **`formant_preserve` was dead code — now implemented.** The magnitude
  spectrum is whitened by its smoothed spectral envelope before the pitch
  remap and the *original* envelope is re-applied after, so formants stay
  put while harmonics move (no more chipmunk timbre on upward shifts).
- **`humanize` was dead code — now implemented** as a slowly-varying random
  detune (semitone amplitude = the param value, redrawn per detector
  estimate), per its documented semantics.
- **AutoTune retune speed no longer depends on buffer size.** The smoothing
  coefficient was applied once per `process()` call, so 128-sample buffers
  retuned 4x faster than 512. It is now normalized per-sample (calibrated to
  preserve the historical feel at 256 frames / 48 kHz).
- **Correction engage/disengage no longer clicks.** AutoTune's inner shifter
  used to be bypassed near unity, so the signal time-jumped through the
  vocoder FIFO every time correction kicked in. The vocoder now always runs
  (constant `fft_size - hop` latency); unity is near-transparent.
- **Limiter rewritten with 240-sample (5 ms @ 48 kHz) lookahead.** Required
  gain is tracked with a sliding-window minimum over the lookahead horizon,
  so gain ramps down *before* transients instead of clamping them after the
  fact (the old design behaved like a clipper). Reports its latency.
- **Parameter changes no longer zipper or click.** New `SmoothedParam`
  (atomic target + per-sample one-pole slew) applied to delay mix/feedback,
  reverb mix, chorus mix/depth, distortion drive/mix, and compressor makeup.
  Delay time changes glide the read pointer at ≤0.5 samples/sample with
  fractional interpolation (bounded tape-style bend instead of a click), and
  the delay buffer is now sized for 2 s at up to 192 kHz (it was hardcoded
  to 96000 samples ≈ 1 s at 96 kHz).

- **New effects no longer glide in from defaults.** Freshly built effects
  started at their constructor defaults and slewed to the preset values over
  ~15 ms (e.g. a mix=0 delay briefly played wet). They now start at the
  configured values.
- **Re-enabling a latency-bearing effect no longer replays stale audio.**
  Limiter, Pitch Shift, Auto-Tune, Noise Suppression and Distortion flush
  their internal buffers when switched back on (RT-safe, no allocation).
- **Auto-Tune humanize is smooth at fast retune speeds.** The random detune
  shared the correction's smoothing, so at speed 0 it jumped to a new value on
  every pitch estimate; it now glides with its own ~150 ms time constant.
- **Reported chain latency ignores disabled effects.**
- **De-esser and exciter crossovers are clamped below Nyquist**, so they keep
  working at low stream rates such as 8 kHz.

### Changed

- **Minimum supported Rust version is now 1.83** (it already relied on
  `const f32::to_bits`).
- **Dropped unused package dependencies.** `permission_handler` and
  `path_provider` were declared by the package but never used by its code
  (only the example app needs them); consumers no longer inherit those
  plugins and their platform setup. The example declares them directly.
- **Hot restart no longer wedges the engine.** The native runtime survives a
  Flutter hot restart, so a session left running kept rejecting `startMic()`
  with "Already running" while `stop()` no-op'd (Dart state had reset to
  idle). `init()` now stops a stale native session, and `stop()` always
  reaches the native engine regardless of Dart-side mode.
- **Precompiled binaries are committed to the repo by CI.** The
  `build-native` workflow now builds all five platforms on version tags (and
  manual dispatch), validates the package, and auto-commits the binaries to
  the default branch — consumers `pub get` and run with no Rust toolchain.
  Pushes/PRs run the Rust + Dart test jobs. (The old workflow's triggers
  pointed at a `main` branch that doesn't exist and only uploaded release
  zips.)
- Minimum Flutter is now 3.27 (Dart 3.6) — the widgets use
  `Color.withValues`, which older Flutters don't have; the old 3.19
  constraint could never have compiled.

- **Chain edits preserve effect state.** Slots are now `Arc`-shared and reused
  across `fx_chain_commit` when the effect type matches, so adding/removing/
  reordering effects no longer cuts reverb tails, resets delay lines, or
  re-warms the denoiser for the effects that stayed. Offline renders
  explicitly reset the chain before and after processing so exports start
  clean and leave nothing behind.
- **Input/output clock drift is compensated.** The two devices run on
  independent clocks; the output callback now sheds one sample per callback
  above a 50 ms fill watermark and holds the last sample (with decay) on
  starvation, preventing the periodic dropouts / creeping latency that
  accumulated over long sessions.
- **Reverb and chorus are sample-rate-adaptive.** Freeverb delay lines retune
  to the negotiated stream rate (44.1 k–192 k) and chorus timing is specified
  in milliseconds, so the sound is consistent at any rate. Noise suppression
  (RNNoise, inherently 48 kHz) now passes through bit-exact at other rates
  and the runtime logs a startup warning instead of producing garbled output.
- **FFI integration tests.** New `rust/tests/api.rs` exercises the exact C
  ABI Dart binds: init semantics, chain build/commit/live-edit, error-message
  reporting, and offline file processing end-to-end (no audio device needed).

### Fixed

- **Distortion `dist_type` now works.** The Dart-side type selection
  (`hardClip`, `tanh`, `bitcrush`) was silently rejected by the Rust core,
  which always ran SoftClip. `dist_type` is now a live-settable parameter.
- **`processFile` no longer blocks the UI isolate.** The blocking native call
  runs on a worker isolate via `Isolate.run`, and `onProgress` now reports
  real progress (polled ~10 Hz from a native progress counter) instead of
  firing once with `1.0` at the end.
- **Multi-channel WAVs are downmixed to mono before offline processing.**
  Previously the mono effect chain ran over interleaved stereo frames,
  smearing time-based effects (delay, reverb, pitch) across channels.
  Processed output is now written as mono WAV. `previewFile` downmixes too.
- **Re-initializing with a different config now fails loudly.**
  `fx_engine_init` used to return `true` while silently keeping the first
  config; it now returns `false` with a descriptive error when the requested
  sample rate / buffer size doesn't match the live engine.
- **Native error messages reach Dart.** New `fx_last_error_message` export;
  engine `StateError`s now include the underlying reason (device errors,
  sample-rate mismatches, file I/O failures) instead of a bare error code.
- **Preview completion no longer wedges the engine.** When `previewFile`
  playback reached the end of the file, the engine stayed in `preview` mode
  and every subsequent start threw "Engine busy" until an explicit `stop()`.
  The engine now detects completion, returns to idle automatically, and fires
  the new `AudioFxEngine.previewCompleteStream`.
- **Failed stream startup no longer bricks the engine.** `is_running` was set
  before cpal stream construction, so a failure (e.g. no input device) left
  the native engine permanently reporting "Already running". The flag is now
  set only once streams are actually live.
- Dev builds now set `panic = "abort"` (release already did), so a panic in a
  debug build of the cdylib can no longer unwind across the C ABI (UB).

### Example app

- Recording start failures are surfaced in the UI (SnackBar) instead of
  silently falling back to mic-only and saving a phantom project with no
  audio file; the elapsed timer no longer runs after a failed start.
- Play/stop state follows `previewCompleteStream`, so the Preview button
  resets when playback finishes on its own.
- Export failures show the native error message; export progress is now real
  (driven by the fixed `onProgress`).
- `flutter analyze` is clean (fixed 34 deprecation/lint issues).

## 0.2.0

Major reliability and performance overhaul. Earlier 0.1.0 was non-functional —
the Dart engine was stubbed out and bridge code was never generated. 0.2.0 is
the first working release.

### Highlights

- **Direct `dart:ffi` bridge** — replaces `flutter_rust_bridge`. Lower overhead,
  no codegen step, and a stable C ABI under `rust/src/api.rs`.
- **Lock-free audio path** — chain stored in `ArcSwap`; the audio callback now
  performs zero allocations, no mutex contention, and no inline FFT plan
  construction.
- **Pitch detection moved off the audio thread** — YIN now runs on a dedicated
  worker fed by a `ringbuf`; results are published through atomics and consumed
  by AutoTune via a generation counter.
- **Recording moved off the audio thread** — both raw and processed taps push
  into ring buffers consumed by writer threads. No more unbounded `Vec` growth
  in the cpal callback.
- **Cached real-FFT plans** — pitch shift and the spectrum analyzer use
  `realfft` plans built once in `new()`.
- **Sound `Send`/`Sync` story** — removed the unsound blanket `unsafe impl`s on
  `AudioRuntime`. Effects use a documented `EffectSlot` (`UnsafeCell`) that
  upholds the single-writer audio-thread invariant.
- **Platform plugin scaffolding** — `android/`, `ios/`, `macos/` (and Linux /
  Windows builds) with a Makefile that produces the artefacts at the locations
  the Flutter ffiPlugin expects.
- **Tests** — `cargo test` runs effect black-box checks (silence, sine,
  finite-sample asserts) and YIN/spectrum sanity tests. `flutter test` covers
  effect serialization and preset construction.

### Breaking changes

- The Dart entrypoint no longer requires `flutter_rust_bridge_codegen`.
- `AudioFormat.mp3()` now throws `UnsupportedError` until a pure-Rust MP3
  encoder is integrated.
- `engine_add_effect` / `engine_remove_effect` / `engine_reorder_effect` are
  removed from the C ABI in favour of full-chain replacement via
  `fx_chain_begin` / `fx_chain_push_effect` / `fx_chain_commit`. The Dart API
  (`AudioFxEngine.setChain`) is unchanged.

## 0.1.0

Initial scaffolding (non-functional).
