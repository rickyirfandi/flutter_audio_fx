import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:flutter_audio_fx/flutter_audio_fx.dart';
import 'package:path_provider/path_provider.dart';
import 'dart:io';
import '../models/recording_project.dart';

class AppController extends ChangeNotifier {
  final AudioFxEngine engine = AudioFxEngine();

  RecordingMode _mode = RecordingMode.studio;
  bool _isRecording = false;
  bool _isPlaying = false;
  Duration _elapsed = Duration.zero;
  Timer? _timer;

  final List<RecordingProject> _recordings = [];
  RecordingProject? _currentProject;
  List<AudioEffect> _chain = [];
  String _activePresetId = 'tpain';
  bool _isExporting = false;
  double _exportProgress = 0.0;

  AppController() {
    _init();
  }

  Future<void> _init() async {
    await engine.init();
    _applyPreset('tpain');
  }

  // ─── Getters ───
  RecordingMode get mode => _mode;
  bool get isRecording => _isRecording;
  bool get isPlaying => _isPlaying;
  Duration get elapsed => _elapsed;
  List<RecordingProject> get recordings => List.unmodifiable(_recordings);
  RecordingProject? get currentProject => _currentProject;
  List<AudioEffect> get chain => List.unmodifiable(_chain);
  String get activePresetId => _activePresetId;
  bool get isExporting => _isExporting;
  double get exportProgress => _exportProgress;

  String get elapsedFormatted {
    final m = _elapsed.inMinutes.remainder(60).toString().padLeft(2, '0');
    final s = _elapsed.inSeconds.remainder(60).toString().padLeft(2, '0');
    final ms = (_elapsed.inMilliseconds.remainder(1000) ~/ 10).toString().padLeft(2, '0');
    return '$m:$s.$ms';
  }

  String get chainSummary =>
      _chain.where((e) => e.enabled).map((e) => e.displayName).join(' → ');

  // ─── Mode ───
  void setMode(RecordingMode mode) {
    if (_isRecording) return;
    _mode = mode;
    notifyListeners();
  }

  // ─── Recording ───
  Future<void> startRecording() async {
    if (_isRecording) return;

    // Get recordings directory
    final dir = await getApplicationDocumentsDirectory();
    final id = 'rec_${DateTime.now().millisecondsSinceEpoch}';
    final recDir = Directory('${dir.path}/recordings/$id');
    await recDir.create(recursive: true);

    _currentProject = RecordingProject(
      id: id,
      title: 'Recording ${DateTime.now().hour}:${DateTime.now().minute.toString().padLeft(2, '0')}',
      createdAt: DateTime.now(),
      duration: Duration.zero,
      mode: _mode,
      rawPath: '${recDir.path}/raw.wav',
    );

    _isRecording = true;
    _elapsed = Duration.zero;
    _timer = Timer.periodic(const Duration(milliseconds: 50), (_) {
      _elapsed += const Duration(milliseconds: 50);
      notifyListeners();
    });

    try {
      engine.setChain(List.from(_chain));
      if (_mode == RecordingMode.live) {
        await engine.startMicWithRecording(
          rawOutputPath: _currentProject!.rawPath,
          processedOutputPath: '${recDir.path}/processed.wav',
        );
      } else {
        await engine.startMicWithRecording(
          rawOutputPath: _currentProject!.rawPath,
        );
      }
    } catch (e) {
      debugPrint('[VoxForge] Recording start error: $e');
      // Fallback to mic-only (no file recording)
      try { await engine.startMic(); } catch (_) {}
    }

    notifyListeners();
  }

  Future<void> stopRecording() async {
    if (!_isRecording) return;
    _timer?.cancel();
    _timer = null;
    _isRecording = false;

    try { await engine.stop(); } catch (e) {
      debugPrint('[VoxForge] Stop error: $e');
    }

    if (_currentProject != null) {
      _currentProject = RecordingProject(
        id: _currentProject!.id,
        title: _currentProject!.title,
        createdAt: _currentProject!.createdAt,
        duration: _elapsed,
        mode: _currentProject!.mode,
        rawPath: _currentProject!.rawPath,
        effectChain: List.from(_chain),
      );
      _recordings.insert(0, _currentProject!);
    }
    notifyListeners();
  }

  // ─── Playback ───
  Future<void> startPlayback(RecordingProject project) async {
    if (_isRecording) return;
    _currentProject = project;
    _isPlaying = true;
    engine.setChain(project.effectChain.isNotEmpty
        ? List.from(project.effectChain)
        : List.from(_chain));
    try {
      await engine.previewFile(inputPath: project.rawPath);
    } catch (e) {
      debugPrint('[VoxForge] Playback error: $e');
    }
    notifyListeners();
  }

  Future<void> stopPlayback() async {
    _isPlaying = false;
    try { await engine.stop(); } catch (_) {}
    notifyListeners();
  }

  // ─── Effect chain ───
  void applyPreset(String presetId) => _applyPreset(presetId);

  void _applyPreset(String presetId) {
    _activePresetId = presetId;
    final preset = BuiltInPresets.all.firstWhere(
      (p) => p.id == presetId, orElse: () => BuiltInPresets.all.first);
    _chain = preset.chain();
    engine.setChain(List.from(_chain));
    notifyListeners();
  }

  void setCustomChain(List<AudioEffect> chain) {
    _activePresetId = 'custom';
    _chain = chain;
    engine.setChain(List.from(_chain));
    notifyListeners();
  }

  void toggleEffect(int index) {
    if (index >= 0 && index < _chain.length) {
      _chain[index].enabled = !_chain[index].enabled;
      engine.toggleEffect(index, _chain[index].enabled);
      notifyListeners();
    }
  }

  /// Set enabled state for all effects at once (for A/B compare)
  void setAllEffectsEnabled(bool enabled) {
    for (var i = 0; i < _chain.length; i++) {
      _chain[i].enabled = enabled;
      engine.toggleEffect(i, enabled);
    }
    notifyListeners();
  }

  void updateEffectParam(int index, String param, double value) {
    if (index >= 0 && index < _chain.length) {
      _chain[index].updateParam(param, value);
      engine.updateParam(index, param, value);
      notifyListeners();
    }
  }

  void removeEffect(int index) {
    if (index >= 0 && index < _chain.length) {
      _chain.removeAt(index);
      _activePresetId = 'custom';
      engine.setChain(List.from(_chain));
      notifyListeners();
    }
  }

  void reorderEffect(int from, int to) {
    if (from < 0 || from >= _chain.length || to < 0 || to >= _chain.length) return;
    final e = _chain.removeAt(from);
    _chain.insert(to, e);
    _activePresetId = 'custom';
    engine.setChain(List.from(_chain));
    notifyListeners();
  }

  void addEffect(AudioEffect effect) {
    final limIdx = _chain.indexWhere((e) => e.type == 'limiter');
    if (limIdx >= 0) { _chain.insert(limIdx, effect); }
    else { _chain.add(effect); }
    _activePresetId = 'custom';
    engine.setChain(List.from(_chain));
    notifyListeners();
  }

  // ─── Export ───
  Future<String?> exportProject(RecordingProject project, AudioFormat format) async {
    _isExporting = true; _exportProgress = 0.0; notifyListeners();
    try {
      final dir = await getApplicationDocumentsDirectory();
      final ext = format is Mp3Format ? 'mp3' : 'wav';
      final outPath = '${dir.path}/exports/${project.id}.$ext';
      await Directory('${dir.path}/exports').create(recursive: true);

      engine.setChain(project.effectChain.isNotEmpty
          ? List.from(project.effectChain) : List.from(_chain));

      final result = await engine.processFile(
        inputPath: project.rawPath, outputPath: outPath, format: format,
        onProgress: (p) { _exportProgress = p; notifyListeners(); },
      );
      _exportProgress = 1.0; _isExporting = false; notifyListeners();
      return result;
    } catch (e) {
      debugPrint('[VoxForge] Export error: $e');
      _isExporting = false; notifyListeners();
      return null;
    }
  }

  void deleteRecording(String id) {
    _recordings.removeWhere((r) => r.id == id);
    notifyListeners();
  }

  @override
  void dispose() { _timer?.cancel(); engine.dispose(); super.dispose(); }
}
