#import <Flutter/Flutter.h>

// Empty plugin class — the package uses dart:ffi exclusively. This header
// keeps CocoaPods happy when integrating the plugin as a regular Flutter
// pod, and references the Rust symbols so the static library is not
// stripped during link.
@interface FlutterAudioFxPlugin : NSObject<FlutterPlugin>
@end
