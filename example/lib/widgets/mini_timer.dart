import 'package:flutter/material.dart';
import '../theme/voxforge_theme.dart';

class MiniTimer extends StatelessWidget {
  final String elapsed;
  final bool isRecording;

  const MiniTimer({
    super.key,
    required this.elapsed,
    this.isRecording = false,
  });

  @override
  Widget build(BuildContext context) {
    return Text(
      elapsed,
      style: TextStyle(
        fontSize: 42,
        fontWeight: FontWeight.w300,
        letterSpacing: 4,
        color: isRecording ? VoxForgeTheme.textPrimary : VoxForgeTheme.textMuted,
        fontFeatures: const [FontFeature.tabularFigures()],
      ),
    );
  }
}
