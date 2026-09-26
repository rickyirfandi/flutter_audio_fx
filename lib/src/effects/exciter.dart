import 'effect.dart';

/// Harmonic exciter that adds presence and "air".
///
/// Harmonics are generated from the band above [frequencyHz] and mixed
/// back in. Quiet material passes through at unity, so this brightens
/// without acting as a plain treble boost.
class Exciter extends AudioEffect {
  /// Crossover above which harmonics are generated, in Hz,
  /// 1000.0–12000.0 (also capped below Nyquist at low sample rates).
  double frequencyHz;

  /// How hard the high band is driven, 0.0–1.0.
  double drive;

  /// Level of the added harmonics, 0.0–1.0.
  double mix;

  Exciter({
    this.frequencyHz = 3000.0,
    this.drive = 0.5,
    this.mix = 0.3,
    super.enabled,
  });

  @override
  String get type => 'exciter';

  @override
  String get displayName => 'Exciter';

  @override
  Map<String, double> toParams() => {
        'frequency_hz': frequencyHz,
        'drive': drive,
        'mix': mix,
      };

  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'frequency_hz':
        frequencyHz = value;
      case 'drive':
        drive = value;
      case 'mix':
        mix = value;
    }
  }
}
