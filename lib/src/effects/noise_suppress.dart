import 'effect.dart';

/// Neural noise suppression (a pure-Rust RNNoise port).
///
/// Removes steady background noise such as fans and hiss while leaving
/// speech intact. Runs on 10 ms frames, so it adds 480 samples of
/// latency at 48 kHz.
class NoiseSuppress extends AudioEffect {
  /// How much of the denoised signal to use, 0.0 (bypass) – 1.0 (full).
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
