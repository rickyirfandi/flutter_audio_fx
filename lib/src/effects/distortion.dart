import 'effect.dart';

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

class Distortion extends AudioEffect {
  double drive;
  double tone;
  double mix;
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
