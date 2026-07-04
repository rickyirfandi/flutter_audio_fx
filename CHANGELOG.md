# Changelog

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
