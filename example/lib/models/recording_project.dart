import 'package:flutter_audio_fx/flutter_audio_fx.dart';

/// Recording mode
enum RecordingMode { live, studio }

/// A recording project containing raw audio, effect config, and exports
class RecordingProject {
  final String id;
  final String title;
  final DateTime createdAt;
  final Duration duration;
  final RecordingMode mode;
  final String rawPath;
  String? processedPath;
  List<AudioEffect> effectChain;

  RecordingProject({
    required this.id,
    required this.title,
    required this.createdAt,
    required this.duration,
    required this.mode,
    required this.rawPath,
    this.processedPath,
    List<AudioEffect>? effectChain,
  }) : effectChain = effectChain ?? [];

  /// Create a new project
  factory RecordingProject.create({
    required RecordingMode mode,
    String? title,
  }) {
    final now = DateTime.now();
    final id = 'rec_${now.millisecondsSinceEpoch}';
    return RecordingProject(
      id: id,
      title: title ?? 'Recording ${now.hour}:${now.minute.toString().padLeft(2, '0')}',
      createdAt: now,
      duration: Duration.zero,
      mode: mode,
      rawPath: '/recordings/$id/raw.wav',
    );
  }

  String get formattedDuration {
    final m = duration.inMinutes.remainder(60).toString().padLeft(2, '0');
    final s = duration.inSeconds.remainder(60).toString().padLeft(2, '0');
    return '$m:$s';
  }

  String get formattedDate {
    final d = createdAt;
    return '${d.day}/${d.month}/${d.year}';
  }
}
