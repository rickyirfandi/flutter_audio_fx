# iOS plugin module

The Rust core ships as a fat static library `libflutter_audio_fx_core.a` placed
in this directory.

To rebuild from source:

```bash
cd ../rust
cargo build --release --target aarch64-apple-ios
cargo build --release --target aarch64-apple-ios-sim   # Apple Silicon sim
cargo build --release --target x86_64-apple-ios        # Intel sim
lipo -create \
  target/aarch64-apple-ios/release/libflutter_audio_fx_core.a \
  target/aarch64-apple-ios-sim/release/libflutter_audio_fx_core.a \
  -output ../ios/libflutter_audio_fx_core.a
```

The provided `Makefile` at the repository root automates this.

You also need to add the `NSMicrophoneUsageDescription` key to the host app's
`Info.plist`.
