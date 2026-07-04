import 'effect.dart';

class Chorus extends AudioEffect {
  double rateHz;
  double depth;
  double mix;

  Chorus({this.rateHz = 1.5, this.depth = 0.5, this.mix = 0.3, super.enabled});

  @override
  String get type => 'chorus';
  @override
  String get displayName => 'Chorus';
  @override
  Map<String, double> toParams() =>
      {'rate_hz': rateHz, 'depth': depth, 'mix': mix};
  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'rate_hz':
        rateHz = value;
      case 'depth':
        depth = value;
      case 'mix':
        mix = value;
    }
  }
}
