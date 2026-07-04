import Cocoa
import FlutterMacOS

// dart:ffi-only plugin — this class exists to satisfy FlutterPlugin
// registration and to keep the Rust symbols from being stripped.
public class FlutterAudioFxPlugin: NSObject, FlutterPlugin {
  public static func register(with registrar: FlutterPluginRegistrar) {
    _ = fx_engine_init
    _ = fx_engine_start
    _ = fx_engine_stop
  }
}

// Forward declarations of Rust symbols.
@_silgen_name("fx_engine_init")
func fx_engine_init(_ sr: UInt32, _ buf: UInt32) -> Bool
@_silgen_name("fx_engine_start")
func fx_engine_start() -> Int32
@_silgen_name("fx_engine_stop")
func fx_engine_stop() -> Int32
