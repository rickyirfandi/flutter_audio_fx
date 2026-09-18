.PHONY: all android ios macos linux windows test test-rust test-dart clean fmt

CARGO_DIR := rust
ANDROID_OUT := android/src/main/jniLibs
IOS_OUT := ios
MACOS_OUT := macos
LINUX_OUT := linux
WIN_OUT := windows

ANDROID_TARGETS := aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

all: android ios macos

android:
	cd $(CARGO_DIR) && cargo ndk \
		$(foreach t,$(ANDROID_TARGETS),-t $(t)) \
		-o ../$(ANDROID_OUT) build --release

# Device arm64 and simulator arm64 can't share a fat .a, so iOS ships as an
# xcframework, matching CI. `strip -S` drops the std debug info that Cargo's
# `strip` setting doesn't remove from static archives.
ios:
	cd $(CARGO_DIR) && cargo build --release --target aarch64-apple-ios
	cd $(CARGO_DIR) && cargo build --release --target aarch64-apple-ios-sim
	cd $(CARGO_DIR) && cargo build --release --target x86_64-apple-ios
	for t in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do \
	  strip -S $(CARGO_DIR)/target/$$t/release/libflutter_audio_fx_core.a; done
	mkdir -p build/ios-sim
	lipo -create \
	  $(CARGO_DIR)/target/aarch64-apple-ios-sim/release/libflutter_audio_fx_core.a \
	  $(CARGO_DIR)/target/x86_64-apple-ios/release/libflutter_audio_fx_core.a \
	  -output build/ios-sim/libflutter_audio_fx_core.a
	rm -rf $(IOS_OUT)/flutter_audio_fx_core.xcframework
	xcodebuild -create-xcframework \
	  -library $(CARGO_DIR)/target/aarch64-apple-ios/release/libflutter_audio_fx_core.a \
	  -library build/ios-sim/libflutter_audio_fx_core.a \
	  -output $(IOS_OUT)/flutter_audio_fx_core.xcframework

macos:
	cd $(CARGO_DIR) && cargo build --release --target aarch64-apple-darwin
	cd $(CARGO_DIR) && cargo build --release --target x86_64-apple-darwin
	lipo -create \
	  $(CARGO_DIR)/target/aarch64-apple-darwin/release/libflutter_audio_fx_core.a \
	  $(CARGO_DIR)/target/x86_64-apple-darwin/release/libflutter_audio_fx_core.a \
	  -output $(MACOS_OUT)/libflutter_audio_fx_core.a
	strip -S $(MACOS_OUT)/libflutter_audio_fx_core.a

linux:
	cd $(CARGO_DIR) && cargo build --release
	mkdir -p $(LINUX_OUT)
	cp $(CARGO_DIR)/target/release/libflutter_audio_fx_core.so $(LINUX_OUT)/

windows:
	cd $(CARGO_DIR) && cargo build --release
	mkdir -p $(WIN_OUT)
	cp $(CARGO_DIR)/target/release/flutter_audio_fx_core.dll $(WIN_OUT)/

test: test-rust test-dart

test-rust:
	cd $(CARGO_DIR) && cargo test --release

test-dart:
	flutter test

fmt:
	cd $(CARGO_DIR) && cargo fmt
	dart format lib test

clean:
	cd $(CARGO_DIR) && cargo clean
	flutter clean
	cd example && flutter clean
