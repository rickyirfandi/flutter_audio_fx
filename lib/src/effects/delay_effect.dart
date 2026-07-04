import 'effect.dart';

class DelayEffect extends AudioEffect {
  double timeMs;
  double feedback;
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
