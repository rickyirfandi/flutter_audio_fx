import 'package:flutter/material.dart';
import '../theme/voxforge_theme.dart';

class RecordButton extends StatelessWidget {
  final bool isRecording;
  final AnimationController pulseAnimation;
  final VoidCallback onTap;

  const RecordButton({
    super.key,
    required this.isRecording,
    required this.pulseAnimation,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: SizedBox(
        width: 100,
        height: 100,
        child: Stack(
          alignment: Alignment.center,
          children: [
            // Outer pulse ring
            if (isRecording)
              AnimatedBuilder(
                animation: pulseAnimation,
                builder: (_, __) => Container(
                  width: 100 + pulseAnimation.value * 20,
                  height: 100 + pulseAnimation.value * 20,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    border: Border.all(
                      color: VoxForgeTheme.danger
                          .withOpacity(0.3 - pulseAnimation.value * 0.25),
                      width: 2,
                    ),
                  ),
                ),
              ),
            // Outer ring
            Container(
              width: 88,
              height: 88,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                border: Border.all(
                  color: isRecording
                      ? VoxForgeTheme.danger.withOpacity(0.4)
                      : VoxForgeTheme.border,
                  width: 3,
                ),
              ),
            ),
            // Inner button
            AnimatedContainer(
              duration: const Duration(milliseconds: 200),
              width: isRecording ? 36 : 68,
              height: isRecording ? 36 : 68,
              decoration: BoxDecoration(
                gradient: VoxForgeTheme.recordGradient,
                borderRadius:
                    BorderRadius.circular(isRecording ? 8 : 34),
                boxShadow: [
                  BoxShadow(
                    color: VoxForgeTheme.danger.withOpacity(isRecording ? 0.5 : 0.2),
                    blurRadius: isRecording ? 24 : 8,
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
