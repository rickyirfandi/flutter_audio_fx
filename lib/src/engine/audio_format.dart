/// Audio export format.
///
/// Currently only WAV is supported by the native engine. MP3 is reserved for
/// a future release; passing `AudioFormat.mp3()` to `AudioFxEngine.processFile`
/// throws `UnsupportedError`.
sealed class AudioFormat {
  const AudioFormat();

  /// Lossless WAV (PCM 16-bit).
  const factory AudioFormat.wav() = WavFormat;

  /// MP3 — not yet implemented; reserved for a future release.
  const factory AudioFormat.mp3({int bitrate}) = Mp3Format;
}

class WavFormat extends AudioFormat {
  const WavFormat();
  @override
  String toString() => 'WAV (lossless)';
}

class Mp3Format extends AudioFormat {
  /// Target bitrate in kbps (128/192/256/320). Currently unused.
  final int bitrate;
  const Mp3Format({this.bitrate = 192});
  @override
  String toString() => 'MP3 ${bitrate}kbps (not yet supported)';
}
