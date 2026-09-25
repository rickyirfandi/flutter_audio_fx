import 'dart:typed_data';

import 'package:flutter_audio_fx/flutter_audio_fx.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('AudioEffect serialization', () {
    test('NoiseGate roundtrips its params', () {
      final gate = NoiseGate(thresholdDb: -35, attackMs: 2, releaseMs: 60);
      final json = gate.toJson();
      expect(json['type'], 'noise_gate');
      expect(json['enabled'], true);
      expect(json['params']['threshold_db'], -35);
      expect(json['params']['attack_ms'], 2);
      expect(json['params']['release_ms'], 60);
      expect(json['params']['hold_ms'], 50);
      expect(json['params']['hysteresis_db'], 6);
      expect(json['params']['range_db'], -100);
      gate.updateParam('hold_ms', 120);
      expect(gate.holdMs, 120);
    });

    test('PitchShift updateParam mutates state', () {
      final ps = PitchShift(semitones: 0);
      ps.updateParam('semitones', 5);
      expect(ps.toParams()['semitones'], 5);
    });

    test('AutoTune key/scale roundtrip via numeric params', () {
      final at = AutoTune(
        key: MusicalKey.G,
        scale: MusicalScale.minor,
        correctionSpeed: 0.0,
      );
      final p = at.toParams();
      expect(p['key'], MusicalKey.G.index.toDouble());
      expect(p['scale'], MusicalScale.minor.index.toDouble());
      expect(p['speed'], 0.0);
    });

    test('Equalizer band params expose freq/gain/q', () {
      final eq = Equalizer.defaultBands();
      final p = eq.toParams();
      // Default 10-band EQ with gain=0 across the board.
      expect(p.length, greaterThanOrEqualTo(20));
    });

    test('Tier-2 vocal effects serialize their Rust parameter names', () {
      final deEsser = DeEsser(frequencyHz: 7000, amount: 0.8);
      expect(deEsser.toJson()['type'], 'de_esser');
      expect(deEsser.toParams()['frequency_hz'], 7000);
      expect(deEsser.toParams()['amount'], 0.8);

      final exciter = Exciter(drive: 0.7, mix: 0.4);
      expect(exciter.toJson()['type'], 'exciter');
      expect(exciter.toParams()['drive'], 0.7);

      final doubler = Doubler(mix: 0.6, spread: 0.5);
      expect(doubler.toJson()['type'], 'doubler');
      expect(doubler.toParams(), {'mix': 0.6, 'spread': 0.5});
    });

    test('Compressor serializes and updates its sidechain HPF', () {
      final compressor = Compressor(sidechainHpfHz: 120);
      expect(compressor.toParams()['sidechain_hpf_hz'], 120);
      compressor.updateParam('sidechain_hpf_hz', 250);
      expect(compressor.sidechainHpfHz, 250);
    });
  });

  group('PitchData', () {
    test('A4 maps to A4 with zero cents', () {
      final pd = PitchData.fromFrequency(440.0, confidence: 1.0);
      expect(pd.noteName, 'A4');
      expect(pd.centsOff.abs(), lessThan(1));
    });

    test('silent for non-positive frequency', () {
      expect(PitchData.fromFrequency(0).noteName, '-');
      expect(PitchData.fromFrequency(-10).pitchHz, 0);
    });

    test('isInTune within 10 cents', () {
      final close = PitchData.fromFrequency(442.0);
      expect(close.isInTune, isTrue);
      // 445 Hz is +20 cents from A4 — clearly out of tune.
      final far = PitchData.fromFrequency(445.0);
      expect(far.isInTune, isFalse);
    });
  });

  group('SpectrumData', () {
    test('toBands collapses into requested number of bins', () {
      final s = SpectrumData(
        magnitudes:
            Float32List.fromList(List<double>.generate(64, (i) => i / 64.0)),
        dominantFreq: 440,
        rms: 0.1,
        binCount: 64,
        freqResolution: 23.4,
      );
      final bands = s.toBands(8);
      expect(bands.length, 8);
      expect(bands.every((v) => v >= 0 && v <= 1), isTrue);
    });
  });

  group('BuiltInPresets', () {
    test('all entries return non-empty chains', () {
      for (final p in BuiltInPresets.all) {
        final chain = p.chain();
        expect(chain.isNotEmpty, true, reason: '${p.name} has empty chain');
        for (final fx in chain) {
          expect(fx.toJson()['type'], isA<String>());
          expect(fx.toJson()['params'], isA<Map>());
        }
      }
    });
  });
}
