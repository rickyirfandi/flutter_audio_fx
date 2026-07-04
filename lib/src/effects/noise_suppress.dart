import 'effect.dart';

class NoiseSuppress extends AudioEffect {
  double strength;

  NoiseSuppress({this.strength = 0.8, super.enabled});

  @override
  String get type => 'noise_suppress';

  @override
  String get displayName => 'Noise Suppression';

  @override
  Map<String, double> toParams() => {'strength': strength};

  @override
  void updateParam(String name, double value) {
    if (name == 'strength') strength = value;
  }
}
