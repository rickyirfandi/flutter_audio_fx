import 'dart:io';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter_audio_fx/flutter_audio_fx.dart';
import 'package:flutter_test/flutter_test.dart';

// Opt in after building rust/ and adding target/debug to the platform's
// library search path. These tests use real FFI and the export worker isolate;
// they never open a microphone or output device.
void main() {
  group('Native engine', () {
    late Directory temporary;
    late AudioFxEngine engine;
    late String input;
    late String output;

    setUpAll(() async {
      temporary =
          await Directory.systemTemp.createTemp('flutter_audio_fx_ffi_');
      input = '${temporary.path}/input.wav';
      output = '${temporary.path}/output.wav';
      engine = AudioFxEngine();
      await engine.init();
    });

    tearDownAll(() async {
      await engine.stop();
      engine.dispose();
      await temporary.delete(recursive: true);
    });

    test('queued settings and reused presets affect worker-isolate exports',
        () async {
      await File(input).writeAsBytes(_wav(List.filled(4800, 0.25)));
      engine.setChain([NoiseGate(thresholdDb: -80)]);
      engine.updateParam(0, 'threshold_db', 0);
      final progress = <double>[];
      expect(
          await engine.processFile(
            inputPath: input,
            outputPath: output,
            onProgress: progress.add,
          ),
          output);
      expect(_samples(await File(output).readAsBytes()), everyElement(0));
      expect(progress.last, 1);
      expect(engine.mode, ProcessingMode.idle);

      engine.setChain([NoiseGate(thresholdDb: -80)]);
      await engine.processFile(inputPath: input, outputPath: output);
      expect(_samples(await File(output).readAsBytes()).last,
          closeTo(0.25, 0.001));
    });

    test('native file errors reach Dart and permit a subsequent export',
        () async {
      engine.clearChain();
      await expectLater(
        engine.processFile(
            inputPath: '${temporary.path}/missing.wav', outputPath: output),
        throwsA(isA<StateError>().having(
          (e) => e.message.toString(),
          'native reason',
          contains('Failed to open WAV'),
        )),
      );
      expect(engine.mode, ProcessingMode.idle);
      await File(input).writeAsBytes(_wav(List.filled(1024, 0.25)));
      await engine.processFile(inputPath: input, outputPath: output);
      expect(_samples(await File(output).readAsBytes()), hasLength(1024));
    });

    test('offline auto-tune corrects successive notes through Dart FFI',
        () async {
      final signal = List.generate(48000, (i) {
        final hz = i < 24000 ? 225.0 : 450.0;
        return math.sin(2 * math.pi * hz * i / 48000) * 0.4;
      });
      await File(input).writeAsBytes(_wav(signal));
      engine.setChain(
          [AutoTune(scale: MusicalScale.chromatic, correctionSpeed: 0)]);
      await engine.processFile(inputPath: input, outputPath: output);
      final samples = _samples(await File(output).readAsBytes());
      expect(_frequency(samples.sublist(10000, 18000)), closeTo(220, 2));
      expect(_frequency(samples.sublist(34000, 42000)), closeTo(440, 2));
    });
  },
      skip: Platform.environment['FLUTTER_AUDIO_FX_NATIVE_TESTS'] != '1'
          ? 'Requires a locally built native core; see test/README.md.'
          : false);
}

Uint8List _wav(List<double> samples) {
  final data = ByteData(44 + samples.length * 2);
  void text(int offset, String value) {
    for (var i = 0; i < value.length; i++) {
      data.setUint8(offset + i, value.codeUnitAt(i));
    }
  }

  text(0, 'RIFF');
  data.setUint32(4, data.lengthInBytes - 8, Endian.little);
  text(8, 'WAVEfmt ');
  data.setUint32(16, 16, Endian.little);
  data.setUint16(20, 1, Endian.little);
  data.setUint16(22, 1, Endian.little);
  data.setUint32(24, 48000, Endian.little);
  data.setUint32(28, 96000, Endian.little);
  data.setUint16(32, 2, Endian.little);
  data.setUint16(34, 16, Endian.little);
  text(36, 'data');
  data.setUint32(40, samples.length * 2, Endian.little);
  for (var i = 0; i < samples.length; i++) {
    data.setInt16(44 + i * 2, (samples[i] * 32767).round(), Endian.little);
  }
  return data.buffer.asUint8List();
}

List<double> _samples(Uint8List bytes) {
  final data = ByteData.sublistView(bytes);
  for (var offset = 12; offset + 8 <= bytes.length;) {
    final length = data.getUint32(offset + 4, Endian.little);
    if (String.fromCharCodes(bytes.sublist(offset, offset + 4)) == 'data') {
      return List.generate(length ~/ 2,
          (i) => data.getInt16(offset + 8 + i * 2, Endian.little) / 32768);
    }
    offset += 8 + length + (length % 2);
  }
  throw StateError('Output WAV contains no data chunk');
}

double _frequency(List<double> samples) {
  final crossings = <double>[];
  for (var i = 1; i < samples.length; i++) {
    if (samples[i - 1] <= 0 && samples[i] > 0) {
      crossings.add(i - 1 - samples[i - 1] / (samples[i] - samples[i - 1]));
    }
  }
  expect(crossings.length, greaterThan(2));
  return 48000 * (crossings.length - 1) / (crossings.last - crossings.first);
}
