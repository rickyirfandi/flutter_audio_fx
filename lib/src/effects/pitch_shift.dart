import 'effect.dart';

class PitchShift extends AudioEffect {
  /// Shift in semitones (-12.0 to +12.0)
  double semitones;

  /// Fine tuning in cents (-50 to +50)
  double cents;

  /// Preserve vocal formants when shifting
  bool formantPreserve;

  PitchShift({
    this.semitones = 0.0,
    this.cents = 0.0,
    this.formantPreserve = true,
    super.enabled,
  });

  @override
  String get type => 'pitch_shift';

  @override
  String get displayName => 'Pitch Shift';

  @override
  Map<String, double> toParams() => {
        'semitones': semitones,
        'cents': cents,
        'formant_preserve': formantPreserve ? 1.0 : 0.0,
      };

  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'semitones':
        semitones = value;
      case 'cents':
        cents = value;
      case 'formant_preserve':
        formantPreserve = value > 0.5;
    }
  }
}
