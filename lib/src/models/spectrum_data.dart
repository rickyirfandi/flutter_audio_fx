import 'dart:typed_data';

/// FFT spectrum data from the Rust analyzer.
/// Used by SpectrumVisualizer widget.
class SpectrumData {
  /// Magnitude of each frequency bin (0.0 to 1.0), normalized
  final Float32List magnitudes;

  /// Dominant (loudest) frequency in Hz
  final double dominantFreq;

  /// RMS volume level (0.0 to 1.0)
  final double rms;

  /// Number of frequency bins
  final int binCount;

  /// Frequency resolution (Hz per bin)
  final double freqResolution;

  const SpectrumData({
    required this.magnitudes,
    required this.dominantFreq,
    required this.rms,
    required this.binCount,
    required this.freqResolution,
  });

  /// Get the magnitude at a specific frequency
  double magnitudeAtFreq(double freqHz) {
    final bin = (freqHz / freqResolution).round();
    if (bin >= 0 && bin < binCount) {
      return magnitudes[bin];
    }
    return 0.0;
  }

  /// Get magnitudes grouped into N display bands (logarithmic spacing)
  List<double> toBands(int numBands) {
    if (magnitudes.isEmpty) return List.filled(numBands, 0.0);

    final bands = List<double>.filled(numBands, 0.0);
    for (var i = 0; i < numBands; i++) {
      // Logarithmic bin mapping
      final startBin =
          (binCount * (i / numBands).clamp(0, 1)).round();
      final endBin =
          (binCount * ((i + 1) / numBands).clamp(0, 1)).round();

      var max = 0.0;
      for (var b = startBin; b < endBin && b < binCount; b++) {
        if (magnitudes[b] > max) max = magnitudes[b];
      }
      bands[i] = max;
    }
    return bands;
  }
}
