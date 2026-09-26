import 'effect.dart';

/// Split-band de-esser that tames harsh "s" sounds.
///
/// Only the band above [frequencyHz] is attenuated, so the body of the
/// voice is left alone.
class DeEsser extends AudioEffect {
  /// Crossover above which sibilance is detected, in Hz, 2000.0–12000.0
  /// (also capped below Nyquist at low sample rates).
  double frequencyHz;

  /// Level at which attenuation begins, in dBFS, -96.0–0.0.
  double thresholdDb;

  /// Strength of the attenuation, 0.0 (off) – 1.0 (up to 20 dB).
  double amount;

  /// Recovery time after a sibilant, in ms, 1.0–1000.0.
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
