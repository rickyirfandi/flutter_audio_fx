# Changelog

## Unreleased

### Changed

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
