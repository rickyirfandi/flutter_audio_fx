import 'effect.dart';

/// Soft-knee dynamic range compressor.
///
/// Level is detected in the linear domain and gain is applied with a
/// smoothed makeup stage, so fast settings stay clean on low notes.
class Compressor extends AudioEffect {
  /// Level above which compression starts, in dBFS, -80.0–0.0.
  double thresholdDb;

  /// Compression ratio, 1.0 (off) – 100.0 (limiting).
  double ratio;

  /// Time to clamp down on a loud signal, in ms, 0.05–500.0.
  double attackMs;

  /// Time to recover after the signal drops, in ms, 1.0–5000.0.
  double releaseMs;

  /// Output gain applied after compression, in dB, -24.0–36.0.
  double makeupGainDb;

  /// Width of the soft knee around the threshold, in dB, 0.0–24.0.
  /// 0 gives a hard knee.
  double kneeDb;

  /// High-pass applied to the detector only, in Hz, 0.0–500.0.
  /// Stops bass energy from pumping the whole signal; 0 disables it.
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
