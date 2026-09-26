# iOS plugin module

The Rust core ships as a prebuilt `flutter_audio_fx_core.xcframework` in this
directory, carrying both the device (arm64) and simulator (arm64 + x86_64)
slices. The podspec picks it up automatically.

To rebuild from source:

```bash
cd ../rust
cargo build --release --target aarch64-apple-ios       # device
cargo build --release --target aarch64-apple-ios-sim   # Apple Silicon sim
cargo build --release --target x86_64-apple-ios        # Intel sim

# The two simulator slices combine into one fat library; the device slice
# cannot join them (it is also arm64), so it stays a separate xcframework slice.
lipo -create \
  target/aarch64-apple-ios-sim/release/libflutter_audio_fx_core.a \
  target/x86_64-apple-ios/release/libflutter_audio_fx_core.a \
  -output /tmp/libflutter_audio_fx_core_sim.a

xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libflutter_audio_fx_core.a \
  -library /tmp/libflutter_audio_fx_core_sim.a \
  -output ../ios/flutter_audio_fx_core.xcframework
```

The provided `Makefile` at the repository root automates this (`make ios`).

You also need to add the `NSMicrophoneUsageDescription` key to the host app's
`Info.plist`.
