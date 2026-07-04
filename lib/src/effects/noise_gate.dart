import 'effect.dart';

class NoiseGate extends AudioEffect {
  double thresholdDb;
  double attackMs;
  double releaseMs;

  NoiseGate({
    this.thresholdDb = -40.0,
    this.attackMs = 1.0,
    this.releaseMs = 50.0,
    super.enabled,
  });

  @override
  String get type => 'noise_gate';

  @override
  String get displayName => 'Noise Gate';

  @override
  Map<String, double> toParams() => {
        'threshold_db': thresholdDb,
        'attack_ms': attackMs,
        'release_ms': releaseMs,
      };

  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'threshold_db':
        thresholdDb = value;
      case 'attack_ms':
        attackMs = value;
      case 'release_ms':
        releaseMs = value;
    }
  }
}
