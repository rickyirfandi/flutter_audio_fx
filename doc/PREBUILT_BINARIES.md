# Prebuilt native binaries

If you don't want to install the Rust toolchain locally, the
`build-native.yml` GitHub Actions workflow will build the native cores for
every platform and attach them as downloadable artifacts.

## Triggering a build

The workflow runs automatically on:

- every push to `main`
- every pull request to `main`
- every tag matching `v*` (e.g. `v0.2.0`) — also creates a GitHub Release
- manual dispatch from the **Actions** tab (use this for ad-hoc builds)

## What you get

| Artifact                       | Contents                                                                |
|--------------------------------|-------------------------------------------------------------------------|
| `flutter_audio_fx_android.zip` | `android/src/main/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/libflutter_audio_fx_core.so` |
| `flutter_audio_fx_ios.zip`     | `ios/flutter_audio_fx_core.xcframework/` and a legacy device-only `ios/libflutter_audio_fx_core.a` |
| `flutter_audio_fx_macos.zip`   | `macos/libflutter_audio_fx_core.a` (universal arm64 + x86_64)           |
| `flutter_audio_fx_linux.zip`   | `linux/libflutter_audio_fx_core.so`                                     |
| `flutter_audio_fx_windows.zip` | `windows/flutter_audio_fx_core.dll` (and import library)                |

When a tag is pushed the workflow also produces a single
`flutter_audio_fx_native_vX.Y.Z.zip` containing every platform laid out at
the project-root paths the Flutter ffiPlugin expects — unzip it at the repo
root and you're done.

## Drop-in usage

1. Open the run on the **Actions** tab of your fork on GitHub.
2. Scroll to the bottom of the run summary; the **Artifacts** section lists
   every per-platform zip plus the combined release bundle.
3. Download the ones you need.
4. Unzip them at the **repository root**. They already contain the correct
   directory structure (`android/...`, `ios/...`, etc.) — no manual file
   shuffling required.
5. Run `flutter pub get` and `flutter run` in `example/`.

The iOS podspec auto-detects whether you dropped in the xcframework or the
legacy lipo'd `.a` and configures itself accordingly.
