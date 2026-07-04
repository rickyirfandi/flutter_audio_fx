import 'effect.dart';

class Reverb extends AudioEffect {
  double roomSize;
  double damping;
  double mix;
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
