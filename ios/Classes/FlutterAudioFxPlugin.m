#import "FlutterAudioFxPlugin.h"

// Forward declarations of Rust symbols so the linker keeps libflutter_audio_fx_core.a
// in the final binary even though Dart calls them dynamically via dart:ffi.
extern bool fx_engine_init(uint32_t, uint32_t);
extern int  fx_engine_start(void);
extern int  fx_engine_stop(void);

@implementation FlutterAudioFxPlugin

+ (void)registerWithRegistrar:(NSObject<FlutterPluginRegistrar>*)registrar {
  // No method channel — Dart talks to Rust directly via FFI. This class
  // exists solely to satisfy the FlutterPlugin protocol and to retain the
  // Rust symbols.
  (void)&fx_engine_init;
  (void)&fx_engine_start;
  (void)&fx_engine_stop;
}

@end
