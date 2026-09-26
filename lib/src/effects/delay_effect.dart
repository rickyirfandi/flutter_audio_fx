import 'effect.dart';

/// Feedback delay (echo) with click-free time changes.
///
/// Changing [timeMs] glides the read position, giving a short
/// tape-style pitch bend instead of a click.
class DelayEffect extends AudioEffect {
  /// Delay time in milliseconds, up to 2000.0.
  double timeMs;

  /// How much of the output is fed back, 0.0–0.95.
  /// Higher values give more repeats.
  double feedback;

  /// Wet/dry blend, 0.0 (dry) – 1.0 (fully wet).
  double mix;

  DelayEffect({
    this.timeMs = 250.0,
    this.feedback = 0.4,
    this.mix = 0.3,
    super.enabled,
  });

  @override
  String get type => 'delay';
  @override
  String get displayName => 'Delay';
  @override
  Map<String, double> toParams() =>
      {'time_ms': timeMs, 'feedback': feedback, 'mix': mix};
  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'time_ms':
        timeMs = value;
      case 'feedback':
        feedback = value;
      case 'mix':
        mix = value;
    }
  }
}
