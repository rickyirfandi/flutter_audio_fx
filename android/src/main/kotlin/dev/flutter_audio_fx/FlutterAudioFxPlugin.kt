package dev.flutter_audio_fx

import android.content.Context
import io.flutter.embedding.engine.plugins.FlutterPlugin

/**
 * Although the engine is driven entirely through `dart:ffi`, this plugin class
 * exists to bootstrap the Rust core's Android JNI requirements:
 *
 *  1. `System.loadLibrary` loads `libflutter_audio_fx_core.so` *through the
 *     JVM*, which fires the Rust `JNI_OnLoad` (a plain `dlopen` from Dart does
 *     not). That captures the `JavaVM`.
 *  2. `nativeAttachContext(applicationContext)` publishes the `JavaVM` +
 *     `Context` into `ndk_context`, which cpal's oboe backend needs before any
 *     device/stream query — otherwise those calls panic and abort the app.
 *
 * This runs at engine attach, well before any Dart `AudioFxEngine.init()`.
 */
class FlutterAudioFxPlugin : FlutterPlugin {
  companion object {
    init {
      System.loadLibrary("flutter_audio_fx_core")
    }

    @JvmStatic
    external fun nativeAttachContext(context: Context)
  }

  override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
    nativeAttachContext(binding.applicationContext)
  }

  override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {}
}
