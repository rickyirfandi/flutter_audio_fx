import '../effects/effect.dart';
import '../effects/noise_gate.dart';
import '../effects/noise_suppress.dart';
import '../effects/pitch_shift.dart';
import '../effects/auto_tune.dart';
import '../effects/equalizer.dart';
import '../effects/compressor.dart';
import '../effects/limiter.dart';
import '../effects/reverb.dart';
import '../effects/chorus.dart';
import '../effects/delay_effect.dart';
import '../effects/distortion.dart';

/// Built-in effect chain presets.
///
/// Usage:
/// ```dart
/// engine.setChain(BuiltInPresets.tpain);
/// ```
abstract final class BuiltInPresets {
  /// T-Pain style hard auto-tune with compression
  static List<AudioEffect> get tpain => [
        NoiseSuppress(strength: 0.7),
        AutoTune(
          key: MusicalKey.C,
          scale: MusicalScale.major,
          correctionSpeed: 0.0, // instant snap
        ),
        Compressor(thresholdDb: -18, ratio: 6.0, attackMs: 5, releaseMs: 80),
        Reverb(roomSize: 0.3, damping: 0.5, mix: 0.15),
        Limiter(ceilingDb: -1.0),
      ];

  /// Radio host / broadcaster voice
  static List<AudioEffect> get radioHost => [
        NoiseGate(thresholdDb: -40),
        NoiseSuppress(strength: 0.8),
        Equalizer(bands: [
          EqBand(freq: 80, gainDb: -6), // cut rumble
          EqBand(freq: 200, gainDb: 2), // warmth
          EqBand(freq: 3000, gainDb: 4), // presence
          EqBand(freq: 5000, gainDb: 3), // clarity
          EqBand(freq: 10000, gainDb: -2), // de-ess
        ]),
        Compressor(thresholdDb: -20, ratio: 4.0, attackMs: 10, releaseMs: 100),
        Limiter(ceilingDb: -1.0),
      ];

  /// Chipmunk voice (+1 octave)
  static List<AudioEffect> get chipmunk => [
        PitchShift(semitones: 12),
        Limiter(ceilingDb: -1.0),
      ];

  /// Deep/bass voice (-7 semitones)
  static List<AudioEffect> get deepVoice => [
        NoiseSuppress(strength: 0.5),
        PitchShift(semitones: -7, formantPreserve: true),
        Equalizer(bands: [
          EqBand(freq: 80, gainDb: 5), // bass boost
          EqBand(freq: 200, gainDb: 3), // body
          EqBand(freq: 800, gainDb: -1), // reduce mud
          EqBand(freq: 3000, gainDb: 1), // presence
          EqBand(freq: 8000, gainDb: -3), // tame highs
        ]),
        Compressor(thresholdDb: -15, ratio: 3.0, attackMs: 10, releaseMs: 150),
        Limiter(ceilingDb: -1.0),
      ];

  /// Lo-fi aesthetic (bitcrush + reverb)
  static List<AudioEffect> get lofi => [
        Distortion(
          drive: 0.15,
          tone: 0.4,
          mix: 0.3,
          distType: DistortionType.bitcrush,
        ),
        Reverb(roomSize: 0.7, damping: 0.7, mix: 0.4),
        Chorus(rateHz: 0.3, depth: 0.3, mix: 0.2),
        Limiter(ceilingDb: -2.0),
      ];

  /// Karaoke (reverb + delay for fullness)
  static List<AudioEffect> get karaoke => [
        NoiseSuppress(strength: 0.5),
        Reverb(roomSize: 0.6, damping: 0.4, mix: 0.35),
        DelayEffect(timeMs: 120, feedback: 0.2, mix: 0.15),
        Compressor(thresholdDb: -18, ratio: 3.0, attackMs: 10, releaseMs: 100),
        Limiter(ceilingDb: -1.0),
      ];

  /// Podcast voice (clean, professional)
  static List<AudioEffect> get podcast => [
        NoiseGate(thresholdDb: -45, releaseMs: 80),
        NoiseSuppress(strength: 0.9),
        Equalizer.defaultBands(),
        Compressor(
          thresholdDb: -22,
          ratio: 3.0,
          attackMs: 15,
          releaseMs: 120,
          makeupGainDb: 4,
        ),
        Limiter(ceilingDb: -1.5),
      ];

  /// Telephone / walkie-talkie effect
  static List<AudioEffect> get telephone => [
        Equalizer(bands: [
          EqBand(freq: 100, gainDb: -12), // cut lows
          EqBand(freq: 200, gainDb: -6), // roll off
          EqBand(freq: 800, gainDb: 2), // midrange
          EqBand(freq: 2000, gainDb: 3), // telephone presence
          EqBand(freq: 4000, gainDb: -6), // roll off highs
          EqBand(freq: 8000, gainDb: -12), // cut highs
        ]),
        Distortion(drive: 0.1, tone: 0.5, mix: 0.4),
        Limiter(ceilingDb: -1.0),
      ];

  /// Robot voice (chromatic auto-tune + chorus)
  static List<AudioEffect> get robot => [
        AutoTune(
          key: MusicalKey.C,
          scale: MusicalScale.chromatic,
          correctionSpeed: 0.0,
        ),
        Chorus(rateHz: 5.0, depth: 0.8, mix: 0.5),
        Distortion(
          drive: 0.2,
          tone: 0.6,
          mix: 0.3,
          distType: DistortionType.hardClip,
        ),
        Reverb(roomSize: 0.2, damping: 0.3, mix: 0.2),
        Limiter(ceilingDb: -1.0),
      ];

  /// Echo chamber (heavy delay + reverb)
  static List<AudioEffect> get echoChamber => [
        DelayEffect(timeMs: 350, feedback: 0.6, mix: 0.5),
        Reverb(roomSize: 0.8, damping: 0.3, mix: 0.4),
        Limiter(ceilingDb: -1.0),
      ];

  /// Gentle auto-tune (natural pitch correction)
  static List<AudioEffect> get gentleAutoTune => [
        NoiseSuppress(strength: 0.6),
        AutoTune(
          key: MusicalKey.C,
          scale: MusicalScale.major,
          correctionSpeed: 0.5,
          humanize: 0.1,
        ),
        Compressor(thresholdDb: -20, ratio: 3.0),
        Limiter(ceilingDb: -1.0),
      ];

  /// All available built-in presets
  static List<
      ({
        String id,
        String name,
        String emoji,
        String desc,
        List<AudioEffect> Function() chain
      })> get all => [
        (
          id: 'tpain',
          name: 'T-Pain',
          emoji: '🎤',
          desc: 'Hard auto-tune + compression',
          chain: () => tpain
        ),
        (
          id: 'radio',
          name: 'Radio Host',
          emoji: '📻',
          desc: 'Clean broadcast voice',
          chain: () => radioHost
        ),
        (
          id: 'chipmunk',
          name: 'Chipmunk',
          emoji: '🐿️',
          desc: '+1 octave pitch shift',
          chain: () => chipmunk
        ),
        (
          id: 'deep',
          name: 'Deep Voice',
          emoji: '🗿',
          desc: 'Deep bass voice',
          chain: () => deepVoice
        ),
        (
          id: 'lofi',
          name: 'Lo-Fi',
          emoji: '📼',
          desc: 'Bitcrush + reverb aesthetic',
          chain: () => lofi
        ),
        (
          id: 'karaoke',
          name: 'Karaoke',
          emoji: '🎶',
          desc: 'Reverb + delay for singing',
          chain: () => karaoke
        ),
        (
          id: 'podcast',
          name: 'Podcast',
          emoji: '🎙️',
          desc: 'Professional voice cleanup',
          chain: () => podcast
        ),
        (
          id: 'telephone',
          name: 'Telephone',
          emoji: '📞',
          desc: 'Vintage phone filter',
          chain: () => telephone
        ),
        (
          id: 'robot',
          name: 'Robot',
          emoji: '🤖',
          desc: 'Robotic vocoder effect',
          chain: () => robot
        ),
        (
          id: 'echo',
          name: 'Echo Chamber',
          emoji: '🏔️',
          desc: 'Heavy echo + reverb',
          chain: () => echoChamber
        ),
        (
          id: 'gentle_autotune',
          name: 'Gentle Tune',
          emoji: '🎵',
          desc: 'Subtle pitch correction',
          chain: () => gentleAutoTune
        ),
      ];
}
