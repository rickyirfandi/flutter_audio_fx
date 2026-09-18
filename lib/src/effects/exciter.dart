import 'effect.dart';

class Exciter extends AudioEffect {
  double frequencyHz;
  double drive;
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
