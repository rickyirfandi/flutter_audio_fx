import 'effect.dart';

// ignore_for_file: constant_identifier_names
enum MusicalKey {
  C, Db, D, Eb, E, F, Gb, G, Ab, A, Bb, B;

  String get label => switch (this) {
        C => 'C',
        Db => 'Db',
        D => 'D',
        Eb => 'Eb',
        E => 'E',
        F => 'F',
        Gb => 'Gb',
        G => 'G',
        Ab => 'Ab',
        A => 'A',
        Bb => 'Bb',
        B => 'B',
      };
}

enum MusicalScale {
  chromatic,
  major,
  minor,
  pentatonic,
  blues,
  dorian,
  mixolydian;

  String get label => switch (this) {
        chromatic => 'Chromatic',
        major => 'Major',
        minor => 'Minor',
        pentatonic => 'Pentatonic',
        blues => 'Blues',
        dorian => 'Dorian',
        mixolydian => 'Mixolydian',
      };
}

class AutoTune extends AudioEffect {
  MusicalKey key;
  MusicalScale scale;

  /// 0.0 = instant snap (T-Pain), 1.0 = no correction
  double correctionSpeed;

  /// Minimum confidence to apply correction (0.0 to 1.0)
  double retuneThreshold;

  /// Slight random detune for natural feel (0.0 to 0.2)
  double humanize;

  AutoTune({
    this.key = MusicalKey.C,
    this.scale = MusicalScale.major,
    this.correctionSpeed = 0.3,
    this.retuneThreshold = 0.3,
    this.humanize = 0.0,
    super.enabled,
  });

  @override
  String get type => 'auto_tune';

  @override
  String get displayName => 'Auto-Tune';

  @override
  Map<String, double> toParams() => {
        'key': key.index.toDouble(),
        'scale': scale.index.toDouble(),
        'speed': correctionSpeed,
        'retune_threshold': retuneThreshold,
        'humanize': humanize,
      };

  @override
  void updateParam(String name, double value) {
    switch (name) {
      case 'key':
        key = MusicalKey.values[value.round().clamp(0, 11)];
      case 'scale':
        scale = MusicalScale.values[value.round().clamp(0, 6)];
      case 'speed':
        correctionSpeed = value;
      case 'retune_threshold':
        retuneThreshold = value;
      case 'humanize':
        humanize = value;
    }
  }
}
