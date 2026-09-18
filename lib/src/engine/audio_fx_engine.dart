import 'dart:async';
import 'dart:ffi';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import '../effects/effect.dart';
import '../ffi/bindings.dart' as native;
import '../models/pitch_data.dart';
import '../models/spectrum_data.dart';
import 'audio_format.dart';
import 'engine_config.dart';

enum ProcessingMode { idle, realtime, recording, offline, preview }

/// Runs the blocking native file-processing call. Top-level so [Isolate.run]
/// can send it to a worker isolate (the bindings re-open the shared library
/// there; the underlying Rust runtime is process-wide).
int _processFileNative(String inputPath, String outputPath) {
  final inPtr = inputPath.toNativeUtf8();
  final outPtr = outputPath.toNativeUtf8();
  try {
    return native.fxEngineProcessFile(inPtr, outPtr);
  } finally {
    calloc.free(inPtr);
    calloc.free(outPtr);
  }
}

/// High-level engine façade. Owns the lifecycle of the native runtime and
/// surfaces effect-chain edits, file I/O, and visualisation streams.
class AudioFxEngine {
  AudioFxEngine({this.config = const EngineConfig()});

  final EngineConfig config;
  final List<AudioEffect> _chain = [];
  ProcessingMode _mode = ProcessingMode.idle;
  bool _initialized = false;
  Timer? _vizTimer;

  // Pre-allocated FFI scratch buffers to avoid per-poll allocation.
  static const int _kSpectrumBins = 1024;
  late final Pointer<Float> _spectrumBuf =
      calloc.allocate<Float>(_kSpectrumBins * sizeOf<Float>());
  late final Pointer<Float> _scalarF1 = calloc.allocate<Float>(sizeOf<Float>());
  late final Pointer<Float> _scalarF2 = calloc.allocate<Float>(sizeOf<Float>());
  late final Pointer<Uint32> _scalarU =
      calloc.allocate<Uint32>(sizeOf<Uint32>());

  final _spectrumCtrl = StreamController<SpectrumData>.broadcast();
  final _waveformCtrl = StreamController<Float32List>.broadcast();
  final _pitchCtrl = StreamController<PitchData>.broadcast();
  final _levelCtrl = StreamController<double>.broadcast();
  final _previewDoneCtrl = StreamController<void>.broadcast();

  // ─── Lifecycle ───

  Future<void> init() async {
    _ensureInit();
  }

  void _ensureInit() {
    if (_initialized) return;
    final ok = native.fxEngineInit(config.sampleRate, config.bufferSize);
    if (!ok) {
      throw StateError('Native engine init failed: ${_lastNativeError()}');
    }
    // The native runtime is a process-wide singleton that survives Flutter
    // hot restart. If a previous Dart lifetime left a session running, stop
    // it now — otherwise every startMic() fails with "Already running" and
    // no public API can recover.
    if (native.fxEngineIsRunning()) {
      native.fxEngineStop();
    }
    _initialized = true;
  }

  /// Reads the most recent native error message (empty string if none).
  static String _lastNativeError() {
    final buf = calloc.allocate<Uint8>(512);
    try {
      final n = native.fxLastErrorMessage(buf.cast<Utf8>(), 512);
      return n == 0 ? '' : buf.cast<Utf8>().toDartString(length: n);
    } finally {
      calloc.free(buf);
    }
  }

  // ─── Chain management ───

  List<AudioEffect> get chain => List.unmodifiable(_chain);

  void setChain(List<AudioEffect> effects) {
    _ensureInit();
    _chain
      ..clear()
      ..addAll(effects);
    _commitChain();
  }

  void clearChain() {
    _chain.clear();
    _commitChain();
  }

  void toggleEffect(int index, bool enabled) {
    if (index < 0 || index >= _chain.length) return;
    _chain[index].enabled = enabled;
    native.fxChainToggle(index, enabled);
  }

  void updateParam(int effectIndex, String paramName, double value) {
    if (effectIndex < 0 || effectIndex >= _chain.length) return;
    _chain[effectIndex].updateParam(paramName, value);
    final namePtr = paramName.toNativeUtf8();
    try {
      native.fxChainUpdateParam(effectIndex, namePtr, value);
    } finally {
      calloc.free(namePtr);
    }
  }

  void _commitChain() {
    _ensureInit();
    native.fxChainBegin();
    for (final fx in _chain) {
      final tyPtr = fx.type.toNativeUtf8();
      try {
        final pushed = native.fxChainPushEffect(tyPtr, fx.enabled);
        if (pushed != 0) continue;
        for (final entry in fx.toParams().entries) {
          final namePtr = entry.key.toNativeUtf8();
          try {
            native.fxChainSetParam(namePtr, entry.value);
          } finally {
            calloc.free(namePtr);
          }
        }
      } finally {
        calloc.free(tyPtr);
      }
    }
    native.fxChainCommit();
  }

  // ─── Real-time ───

  Future<void> startMic() async {
    _guardIdle();
    _ensureInit();
    final rc = native.fxEngineStart();
    if (rc != 0) {
      throw StateError('startMic failed (code $rc): ${_lastNativeError()}');
    }
    _mode = ProcessingMode.realtime;
    _startVizPolling();
  }

  Future<void> startMicWithRecording({
    required String rawOutputPath,
    String? processedOutputPath,
  }) async {
    _guardIdle();
    _ensureInit();
    final rawPtr = rawOutputPath.toNativeUtf8();
    final procPtr =
        processedOutputPath?.toNativeUtf8() ?? Pointer<Utf8>.fromAddress(0);
    try {
      final rc = native.fxEngineStartRecording(rawPtr, procPtr);
      if (rc != 0) {
        throw StateError(
            'startMicWithRecording failed (code $rc): ${_lastNativeError()}');
      }
    } finally {
      calloc.free(rawPtr);
      if (processedOutputPath != null) calloc.free(procPtr);
    }
    _mode = ProcessingMode.recording;
    _startVizPolling();
  }

  Future<void> stop() async {
    // Don't gate on _mode: after a hot restart Dart state resets to idle
    // while the native session keeps running, and skipping the native stop
    // here would leave it wedged. fx_engine_stop is a no-op when idle.
    if (!_initialized) return;
    _stopVizPolling();
    final rc = native.fxEngineStop();
    _mode = ProcessingMode.idle;
    if (rc != 0) {
      throw StateError('stop failed (code $rc): ${_lastNativeError()}');
    }
  }

  // ─── File processing ───

  /// Process [inputPath] through the current chain into [outputPath].
  ///
  /// The native call is synchronous and can take seconds for long files, so
  /// it runs on a worker isolate; [onProgress] is fed from the main isolate
  /// by polling the native progress counter (~10 Hz).
  ///
  /// Multi-channel input is downmixed to mono (the chain is mono) and the
  /// output is written as mono WAV.
  Future<String> processFile({
    required String inputPath,
    required String outputPath,
    AudioFormat format = const AudioFormat.wav(),
    void Function(double)? onProgress,
  }) async {
    _guardIdle();
    _ensureInit();
    if (format is Mp3Format) {
      throw UnsupportedError(
          'MP3 export is not implemented yet. Use AudioFormat.wav() and '
          'transcode externally.');
    }
    _mode = ProcessingMode.offline;
    Timer? progressTimer;
    if (onProgress != null) {
      progressTimer = Timer.periodic(const Duration(milliseconds: 100),
          (_) => onProgress(native.fxProcessFileProgress()));
    }
    try {
      // The native runtime is a process-wide singleton, so the worker isolate
      // operates on the same engine the main isolate initialized.
      final rc = await Isolate.run(() => _processFileNative(inputPath, outputPath));
      if (rc != 0) {
        throw StateError('processFile failed (code $rc): ${_lastNativeError()}');
      }
      onProgress?.call(1.0);
      return outputPath;
    } finally {
      progressTimer?.cancel();
      _mode = ProcessingMode.idle;
    }
  }

  Future<void> previewFile({required String inputPath}) async {
    _guardIdle();
    _ensureInit();
    final ptr = inputPath.toNativeUtf8();
    try {
      final rc = native.fxEnginePreviewFile(ptr);
      if (rc != 0) {
        throw StateError('previewFile failed (code $rc): ${_lastNativeError()}');
      }
    } finally {
      calloc.free(ptr);
    }
    _mode = ProcessingMode.preview;
    _startVizPolling();
  }

  Future<void> stopPreview() => stop();

  // ─── Visualization streams ───

  Stream<SpectrumData> get spectrumStream => _spectrumCtrl.stream;
  Stream<Float32List> get waveformStream => _waveformCtrl.stream;
  Stream<PitchData> get pitchStream => _pitchCtrl.stream;
  Stream<double> get levelStream => _levelCtrl.stream;

  /// Fires once each time a [previewFile] playback reaches the end of the
  /// file. The engine returns to [ProcessingMode.idle] automatically; no
  /// [stop] call is needed for a preview that ran to completion.
  Stream<void> get previewCompleteStream => _previewDoneCtrl.stream;

  void _startVizPolling() {
    _stopVizPolling();
    _vizTimer = Timer.periodic(const Duration(milliseconds: 16), (_) => _pollViz());
  }

  void _stopVizPolling() {
    _vizTimer?.cancel();
    _vizTimer = null;
  }

  void _pollViz() {
    // Preview playback flips the native running flag off at end-of-file, but
    // the Dart mode used to stay `preview`, wedging the engine ("Engine
    // busy") until an explicit stop(). Detect completion and return to idle.
    if (_mode == ProcessingMode.preview && !native.fxEngineIsRunning()) {
      _stopVizPolling();
      native.fxEngineStop(); // release the finished output stream
      _mode = ProcessingMode.idle;
      _previewDoneCtrl.add(null);
      return;
    }
    final n = native.fxGetSpectrum(
        _spectrumBuf, _kSpectrumBins, _scalarF1, _scalarF2, _scalarU);
    if (n > 0 && _spectrumCtrl.hasListener) {
      final mags = Float32List(n);
      for (var i = 0; i < n; i++) {
        mags[i] = _spectrumBuf[i];
      }
      _spectrumCtrl.add(SpectrumData(
        magnitudes: mags,
        dominantFreq: _scalarF1.value,
        rms: _scalarF2.value,
        binCount: _scalarU.value,
        freqResolution: config.sampleRate / (_scalarU.value * 2.0),
      ));
      // Amplitude-envelope point for WaveformVisualizer (one level per frame;
      // this is a meter/envelope, not sample-accurate audio).
      _waveformCtrl.add(Float32List.fromList(<double>[_scalarF2.value]));
    }
    if (_pitchCtrl.hasListener) {
      native.fxGetPitch(_scalarF1, _scalarF2);
      final hz = _scalarF1.value;
      final conf = _scalarF2.value;
      _pitchCtrl.add(PitchData.fromFrequency(hz, confidence: conf));
    }
    if (_levelCtrl.hasListener) {
      _levelCtrl.add(native.fxGetRmsLevel());
    }
  }

  // ─── State ───

  ProcessingMode get mode => _mode;
  bool get isRunning => _mode != ProcessingMode.idle;

  double get totalLatencyMs =>
      native.fxEngineChainLatencyMs() + config.bufferLatencyMs;

  List<Map<String, dynamic>> chainToJson() =>
      _chain.map((e) => e.toJson()).toList();

  String get chainSummary =>
      _chain.where((e) => e.enabled).map((e) => e.displayName).join(' → ');

  void _guardIdle() {
    if (_mode != ProcessingMode.idle) {
      throw StateError('Engine busy (${_mode.name}). Call stop() first.');
    }
  }

  void dispose() {
    stop();
    _stopVizPolling();
    _spectrumCtrl.close();
    _waveformCtrl.close();
    _pitchCtrl.close();
    _levelCtrl.close();
    _previewDoneCtrl.close();
    calloc.free(_spectrumBuf);
    calloc.free(_scalarF1);
    calloc.free(_scalarF2);
    calloc.free(_scalarU);
  }
}
