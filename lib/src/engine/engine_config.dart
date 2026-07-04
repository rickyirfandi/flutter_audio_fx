/// Configuration for the audio engine.
class EngineConfig {
  /// Sample rate in Hz (default: 48000)
  final int sampleRate;

  /// Buffer size in samples (lower = less latency, more CPU)
  /// Recommended: 128 for real-time, 512 for offline processing
  final int bufferSize;

  /// Number of audio channels (1 = mono, 2 = stereo)
  /// Use mono (1) for voice processing.
  final int channels;

  const EngineConfig({
    this.sampleRate = 48000,
    this.bufferSize = 256,
    this.channels = 1,
  });

  /// Latency introduced by the buffer size alone (in ms)
  double get bufferLatencyMs => bufferSize / sampleRate * 1000;

  Map<String, dynamic> toJson() => {
        'sample_rate': sampleRate,
        'buffer_size': bufferSize,
        'channels': channels,
      };
}
