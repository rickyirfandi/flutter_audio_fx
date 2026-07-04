import 'dart:async';
import 'package:flutter/material.dart';
import '../models/spectrum_data.dart';

/// Real-time FFT spectrum visualizer widget.
///
/// Displays frequency magnitudes as animated bars.
/// Connect to [AudioFxEngine.spectrumStream].
///
/// ```dart
/// SpectrumVisualizer(
///   stream: engine.spectrumStream,
///   barCount: 32,
///   barColor: Colors.cyan,
/// )
/// ```
class SpectrumVisualizer extends StatefulWidget {
  final Stream<SpectrumData> stream;
  final int barCount;
  final Color barColor;
  final Color backgroundColor;
  final double barWidth;
  final double barSpacing;
  final double height;
  final BorderRadius? barBorderRadius;

  const SpectrumVisualizer({
    super.key,
    required this.stream,
    this.barCount = 32,
    this.barColor = Colors.cyan,
    this.backgroundColor = Colors.transparent,
    this.barWidth = 4.0,
    this.barSpacing = 2.0,
    this.height = 120.0,
    this.barBorderRadius,
  });

  @override
  State<SpectrumVisualizer> createState() => _SpectrumVisualizerState();
}

class _SpectrumVisualizerState extends State<SpectrumVisualizer> {
  StreamSubscription<SpectrumData>? _subscription;
  List<double> _bands = [];

  @override
  void initState() {
    super.initState();
    _bands = List.filled(widget.barCount, 0.0);
    _subscription = widget.stream.listen((data) {
      if (mounted) {
        setState(() {
          _bands = data.toBands(widget.barCount);
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
        painter: _SpectrumPainter(
          bands: _bands,
          barColor: widget.barColor,
          barWidth: widget.barWidth,
          barSpacing: widget.barSpacing,
          barBorderRadius: widget.barBorderRadius ?? BorderRadius.circular(2),
        ),
      ),
    );
  }
}

class _SpectrumPainter extends CustomPainter {
  final List<double> bands;
  final Color barColor;
  final double barWidth;
  final double barSpacing;
  final BorderRadius barBorderRadius;

  _SpectrumPainter({
    required this.bands,
    required this.barColor,
    required this.barWidth,
    required this.barSpacing,
    required this.barBorderRadius,
  });

  @override
  void paint(Canvas canvas, Size size) {
    if (bands.isEmpty) return;

    final paint = Paint()..color = barColor;
    final totalBarWidth = barWidth + barSpacing;
    final startX = (size.width - totalBarWidth * bands.length) / 2;

    for (var i = 0; i < bands.length; i++) {
      final x = startX + i * totalBarWidth;
      final barHeight = bands[i] * size.height;
      final y = size.height - barHeight;

      final rect = RRect.fromRectAndCorners(
        Rect.fromLTWH(x, y, barWidth, barHeight),
        topLeft: barBorderRadius.topLeft,
        topRight: barBorderRadius.topRight,
        bottomLeft: barBorderRadius.bottomLeft,
        bottomRight: barBorderRadius.bottomRight,
      );

      // Gradient from bottom to top
      paint.shader = LinearGradient(
        begin: Alignment.bottomCenter,
        end: Alignment.topCenter,
        colors: [
          barColor.withValues(alpha: 0.6),
          barColor,
        ],
      ).createShader(Rect.fromLTWH(x, y, barWidth, barHeight));

      canvas.drawRRect(rect, paint);
    }
  }

  @override
  bool shouldRepaint(covariant _SpectrumPainter oldDelegate) =>
      oldDelegate.bands != bands;
}
