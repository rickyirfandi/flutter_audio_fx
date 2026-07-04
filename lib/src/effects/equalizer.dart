import 'effect.dart';

class EqBand {
  double freq;
  double gainDb;
  double q;

  EqBand({required this.freq, this.gainDb = 0.0, this.q = 1.414});

  Map<String, double> toJson() => {'freq': freq, 'gain_db': gainDb, 'q': q};

  factory EqBand.fromJson(Map<String, dynamic> json) => EqBand(
        freq: (json['freq'] as num).toDouble(),
        gainDb: (json['gain_db'] as num?)?.toDouble() ?? 0.0,
        q: (json['q'] as num?)?.toDouble() ?? 1.414,
      );
}

class Equalizer extends AudioEffect {
  List<EqBand> bands;

  /// Create default 10-band EQ
  Equalizer.defaultBands({super.enabled})
      : bands = [
          EqBand(freq: 31),
          EqBand(freq: 62),
          EqBand(freq: 125),
          EqBand(freq: 250),
          EqBand(freq: 500),
          EqBand(freq: 1000),
          EqBand(freq: 2000),
          EqBand(freq: 4000),
          EqBand(freq: 8000),
          EqBand(freq: 16000),
        ];

  /// Create with custom bands
  Equalizer({required this.bands, super.enabled});

  @override
  String get type => 'equalizer';

  @override
  String get displayName => 'Equalizer';

  @override
  Map<String, double> toParams() {
    final map = <String, double>{};
    for (var i = 0; i < bands.length; i++) {
      map['band_${i}_freq'] = bands[i].freq;
      map['band_${i}_gain'] = bands[i].gainDb;
      map['band_${i}_q'] = bands[i].q;
    }
    map['band_count'] = bands.length.toDouble();
    return map;
  }

  @override
  void updateParam(String name, double value) {
    final match = RegExp(r'band_(\d+)_(\w+)').firstMatch(name);
    if (match != null) {
      final idx = int.parse(match.group(1)!);
      final param = match.group(2)!;
      if (idx < bands.length) {
        switch (param) {
          case 'freq':
            bands[idx].freq = value;
          case 'gain':
            bands[idx].gainDb = value;
          case 'q':
            bands[idx].q = value;
        }
      }
    }
  }
}
