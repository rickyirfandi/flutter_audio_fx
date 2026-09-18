import 'effect.dart';

class Doubler extends AudioEffect {
  /// Level of the doubled voices, 0..1. The dry signal is crossfaded down
  /// as this rises, so the output never exceeds the input level.
  double mix;

  /// How far the two doubled voices wander in time (0 = fixed 25/35 ms
  /// delays, 1 = ±2 ms of slow modulation). The engine is mono, so this is
  /// not stereo width.
  double spread;

  Doubler({this.mix = 0.3, this.spread = 1.0, super.enabled});

  @override
  String get type => 'doubler';

  @override
  String get displayName => 'Doubler';

  @override
  Map<String, double> toParams() => {'mix': mix, 'spread': spread};

  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'mix':
        mix = value;
      case 'spread':
        spread = value;
    }
  }
}
