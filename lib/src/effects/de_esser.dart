import 'effect.dart';

class DeEsser extends AudioEffect {
  double frequencyHz;
  double thresholdDb;
  double amount;
  double releaseMs;

  DeEsser({
    this.frequencyHz = 6000.0,
    this.thresholdDb = -30.0,
    this.amount = 1.0,
    this.releaseMs = 60.0,
    super.enabled,
  });

  @override
  String get type => 'de_esser';

  @override
  String get displayName => 'De-Esser';

  @override
  Map<String, double> toParams() => {
        'frequency_hz': frequencyHz,
        'threshold_db': thresholdDb,
        'amount': amount,
        'release_ms': releaseMs,
      };

  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'frequency_hz':
        frequencyHz = value;
      case 'threshold_db':
        thresholdDb = value;
      case 'amount':
        amount = value;
      case 'release_ms':
        releaseMs = value;
    }
  }
}
