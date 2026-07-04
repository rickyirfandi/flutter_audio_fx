import 'dart:math';
import 'package:flutter/material.dart';
import 'package:flutter_audio_fx/flutter_audio_fx.dart';
import '../theme/voxforge_theme.dart';

/// Combined visualizer panel showing animated waveform/spectrum.
/// Uses a fake animation when no real audio data is available.
class VisualizerPanel extends StatefulWidget {
  final AudioFxEngine engine;
  final bool isActive;

  const VisualizerPanel({
    super.key,
    required this.engine,
    this.isActive = false,
  });

  @override
  State<VisualizerPanel> createState() => _VisualizerPanelState();
}

class _VisualizerPanelState extends State<VisualizerPanel>
    with SingleTickerProviderStateMixin {
  late final AnimationController _animController;
  final _random = Random(42);

  @override
  void initState() {
    super.initState();
    _animController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 80),
    )..repeat();
  }

  @override
  void dispose() {
    _animController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 180,
      decoration: BoxDecoration(
        color: VoxForgeTheme.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: VoxForgeTheme.border, width: 0.5),
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(16),
        child: Stack(
          children: [
            // Background grid lines
            CustomPaint(
              size: const Size(double.infinity, 180),
              painter: _GridPainter(),
            ),
            // Animated bars
            AnimatedBuilder(
              animation: _animController,
              builder: (_, __) => CustomPaint(
                size: const Size(double.infinity, 180),
                painter: _BarPainter(
                  isActive: widget.isActive,
                  random: _random,
                  phase: _animController.value,
                ),
              ),
            ),
            // Center line
            Positioned(
              top: 90,
              left: 0,
              right: 0,
              child: Container(
                height: 0.5,
                color: VoxForgeTheme.primary.withOpacity(0.15),
              ),
            ),
            // Label
            Positioned(
              top: 10,
              left: 14,
              child: Row(
                children: [
                  Container(
                    width: 6,
                    height: 6,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: widget.isActive
                          ? VoxForgeTheme.danger
                          : VoxForgeTheme.textMuted,
                    ),
                  ),
                  const SizedBox(width: 6),
                  Text(
                    widget.isActive ? 'LIVE' : 'IDLE',
                    style: TextStyle(
                      fontSize: 9,
                      fontWeight: FontWeight.w700,
                      letterSpacing: 2,
                      color: widget.isActive
                          ? VoxForgeTheme.danger
                          : VoxForgeTheme.textMuted,
                    ),
                  ),
                ],
              ),
            ),
            // Frequency labels
            Positioned(
              bottom: 6,
              left: 14,
              child: Text(
                '50Hz',
                style: TextStyle(
                  fontSize: 8,
                  color: VoxForgeTheme.textMuted.withOpacity(0.5),
                ),
              ),
            ),
            Positioned(
              bottom: 6,
              right: 14,
              child: Text(
                '16kHz',
                style: TextStyle(
                  fontSize: 8,
                  color: VoxForgeTheme.textMuted.withOpacity(0.5),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _GridPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = VoxForgeTheme.border.withOpacity(0.3)
      ..strokeWidth = 0.5;

    // Horizontal lines
    for (var i = 1; i < 4; i++) {
      final y = size.height * i / 4;
      canvas.drawLine(Offset(0, y), Offset(size.width, y), paint);
    }

    // Vertical lines
    for (var i = 1; i < 8; i++) {
      final x = size.width * i / 8;
      canvas.drawLine(Offset(x, 0), Offset(x, size.height), paint);
    }
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

class _BarPainter extends CustomPainter {
  final bool isActive;
  final Random random;
  final double phase;

  _BarPainter({
    required this.isActive,
    required this.random,
    required this.phase,
  });

  @override
  void paint(Canvas canvas, Size size) {
    const barCount = 48;
    final barWidth = size.width / barCount - 1.5;
    final midY = size.height / 2;

    for (var i = 0; i < barCount; i++) {
      // Generate bar height with sinusoidal pattern + noise
      double amplitude;
      if (isActive) {
        final freqFactor = sin(i * 0.3 + phase * pi * 60) * 0.5;
        final noise = (sin(i * 7.31 + phase * 200) * 0.3);
        final bassCurve = exp(-i * 0.04); // more energy in low freq
        amplitude = (0.2 + freqFactor.abs() + noise.abs()) * bassCurve;
        amplitude = amplitude.clamp(0.05, 0.9);
      } else {
        // Idle: very subtle animation
        amplitude = 0.02 + sin(i * 0.5 + phase * pi * 10).abs() * 0.03;
      }

      final barHeight = amplitude * midY;
      final x = i * (barWidth + 1.5) + 0.75;

      // Gradient color: cyan at center, fades at edges
      final hue = 170 + (i / barCount * 30); // cyan → teal
      final color = HSLColor.fromAHSL(
        isActive ? 0.85 : 0.25,
        hue,
        isActive ? 0.8 : 0.3,
        isActive ? 0.55 : 0.25,
      ).toColor();

      final paint = Paint()..color = color;

      // Draw bar (mirrored around center)
      final rect = RRect.fromRectAndRadius(
        Rect.fromCenter(
          center: Offset(x + barWidth / 2, midY),
          width: barWidth,
          height: barHeight * 2,
        ),
        const Radius.circular(1.5),
      );
      canvas.drawRRect(rect, paint);
    }
  }

  @override
  bool shouldRepaint(covariant _BarPainter oldDelegate) => true;
}
