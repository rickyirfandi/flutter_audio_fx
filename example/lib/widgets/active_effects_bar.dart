import 'package:flutter/material.dart';
import 'package:flutter_audio_fx/flutter_audio_fx.dart';
import '../theme/app_theme.dart';

class ActiveEffectsBar extends StatelessWidget {
  final List<AudioEffect> chain;
  final void Function(int index) onToggle;

  const ActiveEffectsBar({
    super.key,
    required this.chain,
    required this.onToggle,
  });

  IconData _iconFor(String type) => switch (type) {
        'noise_gate' => Icons.security,
        'noise_suppress' => Icons.noise_aware,
        'pitch_shift' => Icons.swap_vert,
        'auto_tune' => Icons.music_note,
        'equalizer' => Icons.equalizer,
        'compressor' => Icons.compress,
        'limiter' => Icons.horizontal_rule,
        'reverb' => Icons.waves,
        'chorus' => Icons.multiline_chart,
        'delay' => Icons.timer,
        'distortion' => Icons.electric_bolt,
        'de_esser' => Icons.graphic_eq,
        'exciter' => Icons.auto_awesome,
        'doubler' => Icons.people_alt,
        _ => Icons.auto_fix_high,
      };

  @override
  Widget build(BuildContext context) {
    if (chain.isEmpty) return const SizedBox.shrink();

    return SizedBox(
      height: 36,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.symmetric(horizontal: Spacing.md),
        itemCount: chain.length,
        separatorBuilder: (_, __) => const SizedBox(width: 6),
        itemBuilder: (context, index) {
          final effect = chain[index];
          final isOn = effect.enabled;

          return GestureDetector(
            onTap: () => onToggle(index),
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 150),
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
              decoration: BoxDecoration(
                color: isOn
                    ? AppTheme.primary.withValues(alpha: 0.12)
                    : AppTheme.bgCard,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(
                  color: isOn
                      ? AppTheme.primary.withValues(alpha: 0.4)
                      : AppTheme.border,
                  width: 0.5,
                ),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    _iconFor(effect.type),
                    size: 12,
                    color: isOn ? AppTheme.primary : AppTheme.textMuted,
                  ),
                  const SizedBox(width: 5),
                  Text(
                    effect.displayName,
                    style: TextStyle(
                      fontSize: 10,
                      fontWeight: FontWeight.w600,
                      color: isOn ? AppTheme.primary : AppTheme.textMuted,
                    ),
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
