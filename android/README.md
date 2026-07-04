# Android plugin module

The Rust audio core ships as a prebuilt `.so` per ABI under
`android/src/main/jniLibs/<abi>/libflutter_audio_fx_core.so`.

To rebuild from source (requires the Android NDK and `cargo-ndk`):

```bash
cd ../rust
cargo ndk -t aarch64-linux-android \
          -t armv7-linux-androideabi \
          -t x86_64-linux-android \
          -o ../android/src/main/jniLibs build --release
```

The provided `Makefile` at the repository root wraps this command.
