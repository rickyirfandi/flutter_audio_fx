# Tests

`flutter test` runs the Dart model tests without a native library. Run
`cargo test --locked --manifest-path rust/Cargo.toml` for DSP and native API tests.

The opt-in native engine tests exercise the actual Dart bindings, export worker
isolate, queued settings, pitch correction, and error recovery without audio
hardware. Build the current source locally; packaged binaries may be older.

Windows (PowerShell, from the repository root):

```powershell
cargo build --locked --manifest-path rust/Cargo.toml
$env:PATH = "$((Resolve-Path rust/target/debug).Path);$env:PATH"
$env:FLUTTER_AUDIO_FX_NATIVE_TESTS = '1'
flutter test test/native_engine_test.dart
```

Linux (requires ALSA development headers):

```bash
cargo build --locked --manifest-path rust/Cargo.toml
LD_LIBRARY_PATH="$PWD/rust/target/debug${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  FLUTTER_AUDIO_FX_NATIVE_TESTS=1 flutter test test/native_engine_test.dart
```

This standalone test setup supports Windows and Linux. Apple platforms resolve
symbols from the host process and need a linked app/test host instead.
