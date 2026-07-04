import 'package:flutter/material.dart';
import 'package:flutter_audio_fx/flutter_audio_fx.dart';
import '../theme/voxforge_theme.dart';

/// A single effect card in the editor chain list.
/// Shows effect name + icon, toggle switch, and expandable params.
class EffectCard extends StatefulWidget {
  final int index;
  final AudioEffect effect;
  final void Function(int index) onToggle;
  final void Function(int index, String param, double value) onParamChanged;
  final void Function(int index) onRemove;

  const EffectCard({
    super.key,
    required this.index,
    required this.effect,
    required this.onToggle,
    required this.onParamChanged,
    required this.onRemove,
  });

  @override
  State<EffectCard> createState() => _EffectCardState();
}

class _EffectCardState extends State<EffectCard> {
  bool _expanded = false;

  IconData get _icon => switch (widget.effect.type) {
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
        _ => Icons.auto_fix_high,
      };

  @override
  Widget build(BuildContext context) {
    final e = widget.effect;
    final isOn = e.enabled;

    return AnimatedContainer(
      duration: const Duration(milliseconds: 200),
      decoration: BoxDecoration(
        color: isOn ? VoxForgeTheme.bgCard : VoxForgeTheme.bgCard.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(14),
        border: Border.all(
          color: isOn
              ? VoxForgeTheme.primary.withValues(alpha: 0.2)
              : VoxForgeTheme.border,
          width: 0.5,
        ),
      ),
      child: Column(
        children: [
          // Header row
          GestureDetector(
            onTap: () => setState(() => _expanded = !_expanded),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
              child: Row(
                children: [
                  // Drag handle
                  const Icon(Icons.drag_indicator,
                      size: 16, color: VoxForgeTheme.textMuted),
                  const SizedBox(width: 10),
                  // Index
                  Container(
                    width: 22,
                    height: 22,
                    alignment: Alignment.center,
                    decoration: BoxDecoration(
                      color: VoxForgeTheme.bgSurface,
                      borderRadius: BorderRadius.circular(6),
                    ),
                    child: Text(
                      '${widget.index + 1}',
                      style: const TextStyle(
                        fontSize: 10,
                        fontWeight: FontWeight.w700,
                        color: VoxForgeTheme.textMuted,
                      ),
                    ),
                  ),
                  const SizedBox(width: 10),
                  // Icon + name
                  Icon(_icon,
                      size: 16,
                      color: isOn ? VoxForgeTheme.primary : VoxForgeTheme.textMuted),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      e.displayName,
                      style: TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w600,
                        color: isOn ? VoxForgeTheme.textPrimary : VoxForgeTheme.textMuted,
                      ),
                    ),
                  ),
                  // Toggle
                  SizedBox(
                    height: 24,
                    child: Switch(
                      value: isOn,
                      onChanged: (_) => widget.onToggle(widget.index),
                    ),
                  ),
                  // Expand arrow
                  Icon(
                    _expanded ? Icons.keyboard_arrow_up : Icons.keyboard_arrow_down,
                    size: 18,
                    color: VoxForgeTheme.textMuted,
                  ),
                ],
              ),
            ),
          ),

          // Expanded params
          if (_expanded) ...[
            const Divider(height: 1, color: VoxForgeTheme.border),
            Padding(
              padding: const EdgeInsets.fromLTRB(14, 10, 14, 10),
              child: Column(
                children: [
                  ..._buildParams(),
                  const SizedBox(height: 6),
                  // Remove button
                  Align(
                    alignment: Alignment.centerRight,
                    child: TextButton.icon(
                      onPressed: () => widget.onRemove(widget.index),
                      icon: const Icon(Icons.delete_outline, size: 14),
                      label: const Text('Remove', style: TextStyle(fontSize: 11)),
                      style: TextButton.styleFrom(
                        foregroundColor: VoxForgeTheme.danger,
                        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }

  List<Widget> _buildParams() {
    final params = widget.effect.toParams();
    return params.entries.map((entry) {
      final range = _paramRange(widget.effect.type, entry.key);
      return _ParamSlider(
        label: _formatParamName(entry.key),
        value: entry.value,
        min: range.$1,
        max: range.$2,
        suffix: _paramSuffix(entry.key),
        onChanged: (v) => widget.onParamChanged(widget.index, entry.key, v),
      );
    }).toList();
  }

  String _formatParamName(String name) {
    return name
        .replaceAll('_', ' ')
        .replaceAll('db', 'dB')
        .replaceAll('ms', 'ms')
        .replaceAll('hz', 'Hz')
        .split(' ')
        .map((w) => w.isEmpty ? w : '${w[0].toUpperCase()}${w.substring(1)}')
        .join(' ');
  }

  String _paramSuffix(String name) {
    if (name.contains('db')) return ' dB';
    if (name.contains('ms')) return ' ms';
    if (name.contains('hz')) return ' Hz';
    if (name == 'semitones') return ' st';
    if (name == 'cents') return ' ¢';
    if (name == 'ratio') return ':1';
    return '';
  }

  (double, double) _paramRange(String effectType, String param) {
    return switch ((effectType, param)) {
      (_, 'threshold_db') => (-60.0, 0.0),
      (_, 'ceiling_db') => (-12.0, 0.0),
      (_, 'attack_ms') => (0.1, 100.0),
      (_, 'release_ms') => (10.0, 1000.0),
      (_, 'ratio') => (1.0, 20.0),
      (_, 'makeup_gain_db') => (0.0, 24.0),
      (_, 'knee_db') => (0.0, 12.0),
      (_, 'semitones') => (-12.0, 12.0),
      (_, 'cents') => (-50.0, 50.0),
      (_, 'strength') => (0.0, 1.0),
      (_, 'mix') => (0.0, 1.0),
      (_, 'room_size') => (0.0, 1.0),
      (_, 'damping') => (0.0, 1.0),
      (_, 'pre_delay_ms') => (0.0, 100.0),
      (_, 'rate_hz') => (0.1, 5.0),
      (_, 'depth') => (0.0, 1.0),
      (_, 'time_ms') => (10.0, 2000.0),
      (_, 'feedback') => (0.0, 0.95),
      (_, 'drive') => (0.0, 1.0),
      (_, 'tone') => (0.0, 1.0),
      (_, 'speed') => (0.0, 1.0),
      (_, 'humanize') => (0.0, 0.2),
      (_, 'retune_threshold') => (0.0, 1.0),
      (_, 'key') => (0.0, 11.0),
      (_, 'scale') => (0.0, 6.0),
      _ => (0.0, 1.0),
    };
  }
}

class _ParamSlider extends StatelessWidget {
  final String label;
  final double value;
  final double min;
  final double max;
  final String suffix;
  final ValueChanged<double> onChanged;

  const _ParamSlider({
    required this.label,
    required this.value,
    required this.min,
    required this.max,
    required this.suffix,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Row(
        children: [
          SizedBox(
            width: 100,
            child: Text(
              label,
              style: const TextStyle(
                fontSize: 11,
                color: VoxForgeTheme.textSecondary,
              ),
            ),
          ),
          Expanded(
            child: SliderTheme(
              data: SliderTheme.of(context).copyWith(trackHeight: 2),
              child: Slider(
                value: value.clamp(min, max),
                min: min,
                max: max,
                onChanged: onChanged,
              ),
            ),
          ),
          SizedBox(
            width: 60,
            child: Text(
              '${value.toStringAsFixed(1)}$suffix',
              style: const TextStyle(
                fontSize: 10,
                fontFamily: 'monospace',
                color: VoxForgeTheme.textSecondary,
              ),
              textAlign: TextAlign.right,
            ),
          ),
        ],
      ),
    );
  }
}
