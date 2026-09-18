import 'effect.dart';

class Compressor extends AudioEffect {
  double thresholdDb;
  double ratio;
  double attackMs;
  double releaseMs;
  double makeupGainDb;
  double kneeDb;
  double sidechainHpfHz;

  Compressor({
    this.thresholdDb = -20.0,
    this.ratio = 4.0,
    this.attackMs = 10.0,
    this.releaseMs = 100.0,
    this.makeupGainDb = 0.0,
    this.kneeDb = 3.0,
    this.sidechainHpfHz = 0.0,
    super.enabled,
  });

  @override
  String get type => 'compressor';

  @override
  String get displayName => 'Compressor';

  @override
  Map<String, double> toParams() => {
        'threshold_db': thresholdDb,
        'ratio': ratio,
        'attack_ms': attackMs,
        'release_ms': releaseMs,
        'makeup_gain_db': makeupGainDb,
        'knee_db': kneeDb,
        'sidechain_hpf_hz': sidechainHpfHz,
      };

  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'threshold_db':
        thresholdDb = value;
      case 'ratio':
        ratio = value;
      case 'attack_ms':
        attackMs = value;
      case 'release_ms':
        releaseMs = value;
      case 'makeup_gain_db':
        makeupGainDb = value;
      case 'knee_db':
        kneeDb = value;
      case 'sidechain_hpf_hz':
        sidechainHpfHz = value;
    }
  }
}
