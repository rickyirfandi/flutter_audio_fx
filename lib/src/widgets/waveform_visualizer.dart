import 'dart:async';
import 'dart:typed_data';
import 'package:flutter/material.dart';

/// Real-time amplitude-envelope visualizer.
///
/// Renders the live signal level as a scrolling, mirrored envelope. The engine
/// emits one amplitude point per visualisation frame (~60 fps) via
/// [AudioFxEngine.waveformStream]; this widget accumulates them into a rolling
/// window. (It is an envelope/meter, not a sample-accurate oscilloscope.)
///
/// ```dart
/// WaveformVisualizer(
///   stream: engine.waveformStream,
///   lineColor: Colors.white,
/// )
/// ```
class WaveformVisualizer extends StatefulWidget {
  final Stream<Float32List> stream;
  final Color lineColor;
  final Color backgroundColor;
  final double lineWidth;
  final double height;
  final int displaySamples;

  const WaveformVisualizer({
    super.key,
    required this.stream,
    this.lineColor = Colors.white,
    this.backgroundColor = Colors.transparent,
    this.lineWidth = 2.0,
    this.height = 100.0,
    this.displaySamples = 256,
  });

  @override
  State<WaveformVisualizer> createState() => _WaveformVisualizerState();
}

class _WaveformVisualizerState extends State<WaveformVisualizer> {
  StreamSubscription<Float32List>? _subscription;
  Float32List _samples = Float32List(0);

  @override
  void initState() {
    super.initState();
    _samples = Float32List(widget.displaySamples);
    _subscription = widget.stream.listen((data) {
      if (mounted) {
        setState(() {
          // Rolling buffer: shift old samples left, append new
          final newLen = data.length.clamp(0, widget.displaySamples);
          final keepLen = widget.displaySamples - newLen;
          final updated = Float32List(widget.displaySamples);
          for (var i = 0; i < keepLen; i++) {
            updated[i] = _samples[i + newLen];
          }
          for (var i = 0; i < newLen; i++) {
            updated[keepLen + i] = data[i];
          }
          _samples = updated;
        });
      }
    });
  }

  @override
  void dispose() {
    _subscription?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      height: widget.height,
      color: widget.backgroundColor,
      child: CustomPaint(
        size: Size.infinite,
        painter: _WaveformPainter(
          samples: _samples,
          lineColor: widget.lineColor,
          lineWidth: widget.lineWidth,
        ),
      ),
    );
  }
}

class _WaveformPainter extends CustomPainter {
  final Float32List samples;
  final Color lineColor;
  final double lineWidth;

  _WaveformPainter({
    required this.samples,
    required this.lineColor,
    required this.lineWidth,
  });

  @override
  void paint(Canvas canvas, Size size) {
    if (samples.length < 2) return;

    final midY = size.height / 2;
    final xStep = size.width / (samples.length - 1);

    // Build a symmetric envelope: top edge at midY - |s|, bottom at midY + |s|.
    final top = Path();
    final bottom = Path();
    double ampAt(int i) => samples[i].abs().clamp(0.0, 1.0) * midY;
    top.moveTo(0, midY - ampAt(0));
    bottom.moveTo(0, midY + ampAt(0));
    for (var i = 1; i < samples.length; i++) {
      final x = i * xStep;
      top.lineTo(x, midY - ampAt(i));
      bottom.lineTo(x, midY + ampAt(i));
    }

    // Filled body between the two envelope edges.
    final fill = Path.from(top);
    for (var i = samples.length - 1; i >= 0; i--) {
      fill.lineTo(i * xStep, midY + ampAt(i));
    }
    fill.close();
    canvas.drawPath(
      fill,
      Paint()
        ..color = lineColor.withValues(alpha: 0.25)
        ..style = PaintingStyle.fill,
    );

    final stroke = Paint()
      ..color = lineColor
      ..style = PaintingStyle.stroke
      ..strokeWidth = lineWidth
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    canvas.drawPath(top, stroke);
    canvas.drawPath(bottom, stroke);

    // Center line (zero crossing).
    canvas.drawLine(
      Offset(0, midY),
      Offset(size.width, midY),
      Paint()
        ..color = lineColor.withValues(alpha: 0.2)
        ..strokeWidth = 1,
    );
  }

  @override
  bool shouldRepaint(covariant _WaveformPainter oldDelegate) => true;
}
