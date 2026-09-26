import 'effect.dart';

/// Short modulated delay that thickens a voice or instrument.
///
/// A single voice is detuned by a slow LFO and blended with the dry
/// signal. For a wider, double-tracked sound use [Doubler] instead.
class Chorus extends AudioEffect {
  /// LFO speed in Hz, 0.0–10.0.
  double rateHz;

  /// Modulation depth, 0.0–1.0.
  double depth;

  /// Wet/dry blend, 0.0 (dry) – 1.0 (fully wet).
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
