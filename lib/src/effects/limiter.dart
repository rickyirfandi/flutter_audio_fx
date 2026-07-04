import 'effect.dart';

class Limiter extends AudioEffect {
  double ceilingDb;
  double releaseMs;

  Limiter({this.ceilingDb = -1.0, this.releaseMs = 50.0, super.enabled});

  @override
  String get type => 'limiter';
  @override
  String get displayName => 'Limiter';
  @override
  Map<String, double> toParams() =>
      {'ceiling_db': ceilingDb, 'release_ms': releaseMs};
  @override
  void updateParam(String name, double value) {
    if (name == 'ceiling_db') ceilingDb = value;
    if (name == 'release_ms') releaseMs = value;
  }
}
