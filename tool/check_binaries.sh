#!/usr/bin/env bash
# Pre-publish guard: fail if any precompiled native binary is missing.
# Without them Android (System.loadLibrary) and iOS/macOS (vendored_libraries)
# consumers get a package that cannot load. Run before `flutter pub publish`:
#
#   bash tool/check_binaries.sh && flutter pub publish
set -euo pipefail
cd "$(dirname "$0")/.."

required=(
  android/src/main/jniLibs/arm64-v8a/libflutter_audio_fx_core.so
  android/src/main/jniLibs/armeabi-v7a/libflutter_audio_fx_core.so
  android/src/main/jniLibs/x86_64/libflutter_audio_fx_core.so
  ios/flutter_audio_fx_core.xcframework/Info.plist
  ios/libflutter_audio_fx_core.a
  macos/libflutter_audio_fx_core.a
  linux/libflutter_audio_fx_core.so
  windows/flutter_audio_fx_core.dll
)

missing=0
for f in "${required[@]}"; do
  if [[ ! -s "$f" ]]; then
    echo "MISSING: $f"
    missing=1
  fi
done

if [[ $missing -ne 0 ]]; then
  echo "Native binaries are missing. Run the 'Build native cores' workflow" >&2
  echo "(tag v* or workflow_dispatch) and pull its commit before publishing." >&2
  exit 1
fi
echo "All native binaries present."
