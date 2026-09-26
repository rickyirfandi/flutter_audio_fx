import 'effect.dart';

/// Waveshaping curve used by [Distortion].
enum DistortionType {
  softClip,
  hardClip,
  tanh,
  bitcrush;

  String get label => switch (this) {
        softClip => 'Soft Clip',
        hardClip => 'Hard Clip',
        tanh => 'Tanh',
        bitcrush => 'Bitcrush',
      };
}

/// Waveshaping distortion with 4x oversampling.
///
/// The shaper runs at four times the stream rate and is filtered back
/// down, which keeps harmonics from aliasing into audible frequencies.
class Distortion extends AudioEffect {
  /// Amount of gain into the shaper, 0.0–1.0.
  double drive;

  /// Low-pass on the distorted signal, 0.0 (dark) – 1.0 (open).
  double tone;

  /// Wet/dry blend, 0.0 (dry) – 1.0 (fully wet).
  double mix;

  /// Which waveshaping curve to apply.
  DistortionType distType;

  Distortion({
    this.drive = 0.3,
    this.tone = 0.7,
    this.mix = 0.5,
    this.distType = DistortionType.softClip,
    super.enabled,
  });

  @override
  String get type => 'distortion';
  @override
  String get displayName => 'Distortion';
  @override
  Map<String, double> toParams() => {
        'drive': drive,
        'tone': tone,
        'mix': mix,
        'dist_type': distType.index.toDouble(),
      };
  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'drive':
        drive = value;
      case 'tone':
        tone = value;
      case 'mix':
        mix = value;
      case 'dist_type':
        distType = DistortionType.values[value.round().clamp(0, 3)];
    }
  }
}
