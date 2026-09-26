import 'dart:math' as math;

/// Pitch detection result from the Rust engine.
/// Used for auto-tune visualization and tuner UI.
class PitchData {
  /// Detected pitch frequency in Hz (0.0 if unvoiced)
  final double pitchHz;

  /// Detection confidence (0.0 to 1.0)
  final double confidence;

  /// Nearest note name (e.g. "C4", "A#3")
  final String noteName;

  /// Cents deviation from nearest note (-50 to +50)
  /// Negative = flat, Positive = sharp
  final double centsOff;

  const PitchData({
    required this.pitchHz,
    required this.confidence,
    required this.noteName,
    required this.centsOff,
  });

  /// Whether a pitch was detected (voiced speech)
  bool get isVoiced => pitchHz > 0 && confidence > 0.3;

  /// Whether the pitch is in tune (within ±10 cents)
  bool get isInTune => centsOff.abs() < 10;

  static const silent = PitchData(
    pitchHz: 0,
    confidence: 0,
    noteName: '-',
    centsOff: 0,
  );

  static const _names = [
    'C',
    'C#',
    'D',
    'D#',
    'E',
    'F',
    'F#',
    'G',
    'G#',
    'A',
    'A#',
    'B',
  ];

  factory PitchData.fromFrequency(double freqHz, {double confidence = 1.0}) {
    if (freqHz <= 0 || confidence <= 0) return silent;
    final midi = 69.0 + 12.0 * (math.log(freqHz / 440.0) / math.ln2);
    final midiRound = midi.round();
    final cents = (midi - midiRound) * 100.0;
    final noteIdx = ((midiRound % 12) + 12) % 12;
    final octave = (midiRound ~/ 12) - 1;
    return PitchData(
      pitchHz: freqHz,
      confidence: confidence,
      noteName: '${_names[noteIdx]}$octave',
      centsOff: cents,
    );
  }
}
