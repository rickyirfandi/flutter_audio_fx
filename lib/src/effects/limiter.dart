import 'effect.dart';

/// Brick-wall limiter with 5 ms of lookahead.
///
/// Because it sees transients before they arrive, gain ramps down in
/// advance rather than clipping them. Adds 240 samples (5 ms at
/// 48 kHz) of latency. Put it last in a chain to guarantee headroom.
class Limiter extends AudioEffect {
  /// Maximum output level in dBFS, -30.0–0.0.
  double ceilingDb;

  /// Time to return to unity gain, in ms, 1.0–2000.0.
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
