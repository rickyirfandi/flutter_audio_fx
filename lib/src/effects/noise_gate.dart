import 'effect.dart';

class NoiseGate extends AudioEffect {
  double thresholdDb;
  double attackMs;
  double releaseMs;

  /// How long the gate stays open after the level drops, so it does not
  /// clamp down between syllables and chop word endings.
  double holdMs;

  /// The gate opens at [thresholdDb] but only closes once the level falls
  /// this far below it, so a signal hovering at the threshold does not
  /// chatter.
  double hysteresisDb;

  /// Attenuation when closed. At or below -96 dB the gate mutes completely;
  /// values like -20 dB give a gentler "expander" that keeps some room tone.
  double rangeDb;

  NoiseGate({
    this.thresholdDb = -40.0,
    this.attackMs = 1.0,
    this.releaseMs = 50.0,
    this.holdMs = 50.0,
    this.hysteresisDb = 6.0,
    this.rangeDb = -100.0,
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
        'hold_ms': holdMs,
        'hysteresis_db': hysteresisDb,
        'range_db': rangeDb,
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
      case 'hold_ms':
        holdMs = value;
      case 'hysteresis_db':
        hysteresisDb = value;
      case 'range_db':
        rangeDb = value;
    }
  }
}
