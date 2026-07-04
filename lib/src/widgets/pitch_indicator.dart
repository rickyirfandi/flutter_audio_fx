import 'dart:async';
import 'package:flutter/material.dart';
import '../models/pitch_data.dart';

/// Pitch detection indicator widget.
///
/// Shows the detected note name, cents deviation, and a visual tuner gauge.
/// Connect to [AudioFxEngine.pitchStream].
///
/// ```dart
/// PitchIndicator(
///   stream: engine.pitchStream,
///   activeColor: Colors.green,
/// )
/// ```
class PitchIndicator extends StatefulWidget {
  final Stream<PitchData> stream;
  final Color activeColor;
  final Color inactiveColor;
  final Color sharpColor;
  final Color flatColor;
  final double height;
  final TextStyle? noteStyle;
  final TextStyle? centsStyle;

  const PitchIndicator({
    super.key,
    required this.stream,
    this.activeColor = Colors.green,
    this.inactiveColor = Colors.grey,
    this.sharpColor = Colors.orange,
    this.flatColor = Colors.blue,
    this.height = 80,
    this.noteStyle,
    this.centsStyle,
  });

  @override
  State<PitchIndicator> createState() => _PitchIndicatorState();
}

class _PitchIndicatorState extends State<PitchIndicator> {
  StreamSubscription<PitchData>? _subscription;
  PitchData _pitch = PitchData.silent;

  @override
  void initState() {
    super.initState();
    _subscription = widget.stream.listen((data) {
      if (mounted) setState(() => _pitch = data);
    });
  }

  @override
  void dispose() {
    _subscription?.cancel();
    super.dispose();
  }

  Color get _indicatorColor {
    if (!_pitch.isVoiced) return widget.inactiveColor;
    if (_pitch.isInTune) return widget.activeColor;
    return _pitch.centsOff > 0 ? widget.sharpColor : widget.flatColor;
  }

  String get _tuningLabel {
    if (!_pitch.isVoiced) return '';
    if (_pitch.isInTune) return '✓';
    final sign = _pitch.centsOff > 0 ? '+' : '';
    return '$sign${_pitch.centsOff.round()}¢';
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return SizedBox(
      height: widget.height,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          // Note name
          Text(
            _pitch.isVoiced ? _pitch.noteName : '—',
            style: widget.noteStyle ??
                theme.textTheme.headlineMedium?.copyWith(
                  fontWeight: FontWeight.bold,
                  color: _indicatorColor,
                  fontFamily: 'monospace',
                ),
          ),
          const SizedBox(height: 8),
          // Cents gauge
          SizedBox(
            width: 200,
            height: 12,
            child: CustomPaint(
              painter: _CentsGaugePainter(
                cents: _pitch.isVoiced ? _pitch.centsOff : 0,
                activeColor: _indicatorColor,
                inactiveColor: widget.inactiveColor.withValues(alpha: 0.3),
              ),
            ),
          ),
          const SizedBox(height: 4),
          // Cents label
          Text(
            _tuningLabel,
            style: widget.centsStyle ??
                theme.textTheme.bodySmall?.copyWith(
                  color: _indicatorColor,
                  fontFamily: 'monospace',
                ),
          ),
        ],
      ),
    );
  }
}

class _CentsGaugePainter extends CustomPainter {
  final double cents;
  final Color activeColor;
  final Color inactiveColor;

  _CentsGaugePainter({
    required this.cents,
    required this.activeColor,
    required this.inactiveColor,
  });

  @override
  void paint(Canvas canvas, Size size) {
    final midX = size.width / 2;
    final midY = size.height / 2;
    final radius = size.height / 2;

    // Background track
    final bgPaint = Paint()
      ..color = inactiveColor
      ..strokeWidth = 3
      ..strokeCap = StrokeCap.round;
    canvas.drawLine(Offset(0, midY), Offset(size.width, midY), bgPaint);

    // Center mark
    final centerPaint = Paint()
      ..color = activeColor.withValues(alpha: 0.5)
      ..strokeWidth = 2;
    canvas.drawLine(
      Offset(midX, midY - radius),
      Offset(midX, midY + radius),
      centerPaint,
    );

    // Indicator dot
    final normalizedCents = (cents / 50.0).clamp(-1.0, 1.0);
    final dotX = midX + normalizedCents * (size.width / 2 - radius);

    final dotPaint = Paint()..color = activeColor;
    canvas.drawCircle(Offset(dotX, midY), radius - 1, dotPaint);
  }

  @override
  bool shouldRepaint(covariant _CentsGaugePainter oldDelegate) =>
      oldDelegate.cents != cents || oldDelegate.activeColor != activeColor;
}
