import 'package:flutter/material.dart';
import 'package:flutter_audio_fx/flutter_audio_fx.dart';
import '../theme/app_theme.dart';

class PresetCarousel extends StatelessWidget {
  final String activePresetId;
  final ValueChanged<String> onPresetSelected;

  const PresetCarousel({
    super.key,
    required this.activePresetId,
    required this.onPresetSelected,
  });

  @override
  Widget build(BuildContext context) {
    final presets = BuiltInPresets.all;

    return SizedBox(
      height: 88,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: Spacing.md),
        itemCount: presets.length,
        separatorBuilder: (_, __) => const SizedBox(width: 10),
        itemBuilder: (context, index) {
          final preset = presets[index];
          final isActive = activePresetId == preset.id;

          return GestureDetector(
            onTap: () => onPresetSelected(preset.id),
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 200),
              width: 78,
              decoration: BoxDecoration(
                color: isActive
                    ? AppTheme.primary.withValues(alpha: 0.1)
                    : AppTheme.bgCard,
                borderRadius: BorderRadius.circular(14),
                border: Border.all(
                  color: isActive ? AppTheme.primary : AppTheme.border,
                  width: isActive ? 1.5 : 0.5,
                ),
              ),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(preset.emoji, style: const TextStyle(fontSize: 26)),
                  const SizedBox(height: 6),
                  Text(
                    preset.name,
                    style: TextStyle(
                      fontSize: 10,
                      fontWeight: FontWeight.w600,
                      letterSpacing: 0.5,
                      color:
                          isActive ? AppTheme.primary : AppTheme.textSecondary,
                    ),
                    textAlign: TextAlign.center,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
              ),
            ),
          );
        },
      ),
    );
  }
}
