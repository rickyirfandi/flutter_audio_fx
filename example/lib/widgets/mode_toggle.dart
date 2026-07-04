import 'package:flutter/material.dart';
import '../models/recording_project.dart';
import '../theme/voxforge_theme.dart';

class ModeToggle extends StatelessWidget {
  final RecordingMode mode;
  final ValueChanged<RecordingMode> onChanged;
  final bool enabled;

  const ModeToggle({
    super.key,
    required this.mode,
    required this.onChanged,
    this.enabled = true,
  });

  @override
  Widget build(BuildContext context) {
    return Opacity(
      opacity: enabled ? 1.0 : 0.5,
      child: Container(
        margin: const EdgeInsets.symmetric(horizontal: 60),
        padding: const EdgeInsets.all(3),
        decoration: BoxDecoration(
          color: VoxForgeTheme.bgCard,
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: VoxForgeTheme.border, width: 0.5),
        ),
        child: Row(
          children: [
            _buildTab(
              icon: Icons.bolt,
              label: 'LIVE',
              isActive: mode == RecordingMode.live,
              onTap: enabled ? () => onChanged(RecordingMode.live) : null,
            ),
            _buildTab(
              icon: Icons.tune,
              label: 'STUDIO',
              isActive: mode == RecordingMode.studio,
              onTap: enabled ? () => onChanged(RecordingMode.studio) : null,
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildTab({
    required IconData icon,
    required String label,
    required bool isActive,
    VoidCallback? onTap,
  }) {
    return Expanded(
      child: GestureDetector(
        onTap: onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 200),
          curve: Curves.easeOut,
          padding: const EdgeInsets.symmetric(vertical: 10),
          decoration: BoxDecoration(
            color: isActive ? VoxForgeTheme.primary.withOpacity(0.12) : Colors.transparent,
            borderRadius: BorderRadius.circular(9),
            border: isActive
                ? Border.all(color: VoxForgeTheme.primary.withOpacity(0.3), width: 0.5)
                : null,
          ),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                icon,
                size: 14,
                color: isActive ? VoxForgeTheme.primary : VoxForgeTheme.textMuted,
              ),
              const SizedBox(width: 6),
              Text(
                label,
                style: TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w700,
                  letterSpacing: 1.5,
                  color: isActive ? VoxForgeTheme.primary : VoxForgeTheme.textMuted,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
