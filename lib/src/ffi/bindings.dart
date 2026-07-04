// Direct dart:ffi bindings for the flutter_audio_fx Rust core.
//
// The Rust side exposes a flat C ABI (see rust/src/api.rs). On Android the
// native lib is loaded as `libflutter_audio_fx_core.so`; on iOS it is
// statically linked into the app, so `DynamicLibrary.process()` is used.

import 'dart:ffi';
import 'dart:io' show Platform;

import 'package:ffi/ffi.dart';

const String _kLibName = 'flutter_audio_fx_core';

DynamicLibrary _open() {
  if (Platform.isAndroid || Platform.isLinux) {
    return DynamicLibrary.open('lib$_kLibName.so');
  }
  if (Platform.isMacOS || Platform.isIOS) {
    return DynamicLibrary.process();
  }
  if (Platform.isWindows) {
    return DynamicLibrary.open('$_kLibName.dll');
  }
  throw UnsupportedError('Unsupported platform: ${Platform.operatingSystem}');
}

final DynamicLibrary _lib = _open();

// ─── Lifecycle ───

final bool Function(int sampleRate, int bufferSize) fxEngineInit = _lib
    .lookupFunction<Bool Function(Uint32, Uint32), bool Function(int, int)>(
        'fx_engine_init',
        isLeaf: true);

final bool Function() fxEngineIsRunning = _lib
    .lookupFunction<Bool Function(), bool Function()>('fx_engine_is_running',
        isLeaf: true);

final int Function() fxEngineSampleRate = _lib
    .lookupFunction<Uint32 Function(), int Function()>('fx_engine_sample_rate',
        isLeaf: true);

final int Function() fxEngineBufferSize = _lib
    .lookupFunction<Uint32 Function(), int Function()>('fx_engine_buffer_size',
        isLeaf: true);

final double Function() fxEngineChainLatencyMs = _lib
    .lookupFunction<Float Function(), double Function()>(
        'fx_engine_chain_latency_ms',
        isLeaf: true);

final int Function() fxEngineStart = _lib
    .lookupFunction<Int32 Function(), int Function()>('fx_engine_start');

final int Function(Pointer<Utf8>, Pointer<Utf8>) fxEngineStartRecording = _lib
    .lookupFunction<Int32 Function(Pointer<Utf8>, Pointer<Utf8>),
        int Function(Pointer<Utf8>, Pointer<Utf8>)>('fx_engine_start_recording');

final int Function() fxEngineStop = _lib
    .lookupFunction<Int32 Function(), int Function()>('fx_engine_stop');

final int Function(Pointer<Utf8>, Pointer<Utf8>) fxEngineProcessFile = _lib
    .lookupFunction<Int32 Function(Pointer<Utf8>, Pointer<Utf8>),
        int Function(Pointer<Utf8>, Pointer<Utf8>)>('fx_engine_process_file');

final int Function(Pointer<Utf8>) fxEnginePreviewFile = _lib
    .lookupFunction<Int32 Function(Pointer<Utf8>),
        int Function(Pointer<Utf8>)>('fx_engine_preview_file');

// ─── Chain editing ───

final void Function() fxChainBegin = _lib
    .lookupFunction<Void Function(), void Function()>('fx_chain_begin');

final int Function(Pointer<Utf8>, bool) fxChainPushEffect = _lib
    .lookupFunction<Int32 Function(Pointer<Utf8>, Bool),
        int Function(Pointer<Utf8>, bool)>('fx_chain_push_effect');

final int Function(Pointer<Utf8>, double) fxChainSetParam = _lib
    .lookupFunction<Int32 Function(Pointer<Utf8>, Float),
        int Function(Pointer<Utf8>, double)>('fx_chain_set_param');

final int Function() fxChainCommit = _lib
    .lookupFunction<Int32 Function(), int Function()>('fx_chain_commit');

final int Function(int, bool) fxChainToggle = _lib
    .lookupFunction<Int32 Function(Uint32, Bool), int Function(int, bool)>(
        'fx_chain_toggle',
        isLeaf: true);

final int Function(int, Pointer<Utf8>, double) fxChainUpdateParam = _lib
    .lookupFunction<Int32 Function(Uint32, Pointer<Utf8>, Float),
        int Function(int, Pointer<Utf8>, double)>('fx_chain_update_param',
        isLeaf: true);

// ─── Visualization ───

final int Function(Pointer<Float>, int, Pointer<Float>, Pointer<Float>,
    Pointer<Uint32>) fxGetSpectrum = _lib.lookupFunction<
    Uint32 Function(Pointer<Float>, Uint32, Pointer<Float>, Pointer<Float>,
        Pointer<Uint32>),
    int Function(Pointer<Float>, int, Pointer<Float>, Pointer<Float>,
        Pointer<Uint32>)>('fx_get_spectrum', isLeaf: true);

final double Function() fxGetRmsLevel = _lib
    .lookupFunction<Float Function(), double Function()>('fx_get_rms_level',
        isLeaf: true);

final void Function(Pointer<Float>, Pointer<Float>) fxGetPitch = _lib
    .lookupFunction<Void Function(Pointer<Float>, Pointer<Float>),
        void Function(Pointer<Float>, Pointer<Float>)>('fx_get_pitch',
        isLeaf: true);
