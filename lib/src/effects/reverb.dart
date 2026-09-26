import 'effect.dart';

/// Freeverb-style reverb (Schroeder-Moorer), tuned for voice.
class Reverb extends AudioEffect {
  /// Size of the simulated space, 0.0 (tight) – 1.0 (large hall).
  double roomSize;

  /// How quickly high frequencies decay, 0.0 (bright) – 1.0 (dark).
  double damping;

  /// Wet/dry blend, 0.0 (dry) – 1.0 (fully wet).
  double mix;

  /// Gap before the reverb starts, in milliseconds. A short pre-delay
  /// keeps the dry voice clear in front of the tail.
  double preDelayMs;

  Reverb({
    this.roomSize = 0.5,
    this.damping = 0.5,
    this.mix = 0.3,
    this.preDelayMs = 0.0,
    super.enabled,
  });

  @override
  String get type => 'reverb';
  @override
  String get displayName => 'Reverb';
  @override
  Map<String, double> toParams() => {
        'room_size': roomSize,
        'damping': damping,
        'mix': mix,
        'pre_delay_ms': preDelayMs,
      };
  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'room_size':
        roomSize = value;
      case 'damping':
        damping = value;
      case 'mix':
        mix = value;
      case 'pre_delay_ms':
        preDelayMs = value;
    }
  }
}
