import 'package:flutter/material.dart';
import 'package:flutter_audio_fx/flutter_audio_fx.dart';
import '../controllers/app_controller.dart';
import '../models/recording_project.dart';
import '../theme/voxforge_theme.dart';
import '../widgets/effect_card.dart';
import '../widgets/visualizer_panel.dart';
import 'export_screen.dart';

class EditorScreen extends StatefulWidget {
  final AppController controller;
  final RecordingProject project;

  const EditorScreen({
    super.key,
    required this.controller,
    required this.project,
  });

  @override
  State<EditorScreen> createState() => _EditorScreenState();
}

class _EditorScreenState extends State<EditorScreen> {
  late final AppController _ctrl;
  bool _abCompare = false;
  List<bool>? _savedEnabledStates;

  @override
  void initState() {
    super.initState();
    _ctrl = widget.controller;
    _ctrl.addListener(_refresh);
  }

  void _refresh() {
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    _ctrl.removeListener(_refresh);
    super.dispose();
  }

  void _toggleAB() {
    setState(() => _abCompare = !_abCompare);
    if (_abCompare) {
      // Going DRY: save current states, disable all
      _savedEnabledStates = _ctrl.chain.map((e) => e.enabled).toList();
      _ctrl.setAllEffectsEnabled(false);
    } else {
      // Going WET: restore saved states
      if (_savedEnabledStates != null) {
        final chain = _ctrl.chain;
        for (var i = 0; i < chain.length && i < _savedEnabledStates!.length; i++) {
          if (chain[i].enabled != _savedEnabledStates![i]) {
            _ctrl.toggleEffect(i);
          }
        }
        _savedEnabledStates = null;
      } else {
        _ctrl.setAllEffectsEnabled(true);
      }
    }
  }

  // The controller is the source of truth for the play state: it resets
  // itself when preview playback reaches the end of the file.
  void _togglePlayback() {
    if (_ctrl.isPlaying) {
      _ctrl.stopPlayback();
    } else {
      _ctrl.startPlayback(widget.project);
    }
  }

  void _showAddEffectSheet() {
    showModalBottomSheet(
      context: context,
      builder: (_) => _AddEffectSheet(
        onAdd: (effect) {
          _ctrl.addEffect(effect);
          Navigator.pop(context);
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('EFFECT EDITOR'),
        leading: IconButton(
          icon: const Icon(Icons.arrow_back_ios, size: 18),
          onPressed: () => Navigator.pop(context),
        ),
        actions: [
          TextButton(
            onPressed: () {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (_) => ExportScreen(
                    controller: _ctrl,
                    project: widget.project,
                  ),
                ),
              );
            },
            child: const Text(
              'EXPORT',
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w700,
                letterSpacing: 1.5,
                color: VoxForgeTheme.accent,
              ),
            ),
          ),
        ],
      ),
      body: Column(
        children: [
          // ── Visualizer + playback controls ──
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: Spacing.md),
            child: VisualizerPanel(
              engine: _ctrl.engine,
              isActive: _ctrl.isPlaying,
            ),
          ),
          const SizedBox(height: Spacing.sm),

          // ── Playback bar ──
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: Spacing.md),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                // Play/stop
                _ControlButton(
                  icon: _ctrl.isPlaying ? Icons.stop : Icons.play_arrow,
                  label: _ctrl.isPlaying ? 'Stop' : 'Preview',
                  color: VoxForgeTheme.primary,
                  onTap: _togglePlayback,
                ),
                const SizedBox(width: 16),
                // A/B compare
                _ControlButton(
                  icon: Icons.compare_arrows,
                  label: _abCompare ? 'DRY' : 'WET',
                  color: _abCompare ? VoxForgeTheme.accent : VoxForgeTheme.textMuted,
                  onTap: _toggleAB,
                ),
                const SizedBox(width: 16),
                // Duration
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                  decoration: BoxDecoration(
                    color: VoxForgeTheme.bgCard,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    widget.project.formattedDuration,
                    style: const TextStyle(
                      fontSize: 13,
                      fontFamily: 'monospace',
                      color: VoxForgeTheme.textSecondary,
                    ),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: Spacing.md),

          // ── Section header ──
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: Spacing.md),
            child: Row(
              children: [
                const Text(
                  'EFFECT CHAIN',
                  style: TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w700,
                    letterSpacing: 2,
                    color: VoxForgeTheme.textMuted,
                  ),
                ),
                const Spacer(),
                Text(
                  '${_ctrl.chain.where((e) => e.enabled).length}/${_ctrl.chain.length} active',
                  style: const TextStyle(
                    fontSize: 10,
                    color: VoxForgeTheme.textMuted,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: Spacing.sm),

          // ── Effect chain list ──
          Expanded(
            child: _ctrl.chain.isEmpty
                ? _buildEmptyState()
                : ReorderableListView.builder(
                    padding: const EdgeInsets.symmetric(horizontal: Spacing.md),
                    itemCount: _ctrl.chain.length,
                    onReorder: (from, to) {
                      if (to > from) to--;
                      _ctrl.reorderEffect(from, to);
                    },
                    proxyDecorator: (child, index, animation) {
                      return Material(
                        color: Colors.transparent,
                        elevation: 4,
                        shadowColor: VoxForgeTheme.primary.withValues(alpha: 0.2),
                        borderRadius: BorderRadius.circular(14),
                        child: child,
                      );
                    },
                    itemBuilder: (context, index) {
                      return Padding(
                        key: ValueKey('effect_$index'),
                        padding: const EdgeInsets.only(bottom: 8),
                        child: EffectCard(
                          index: index,
                          effect: _ctrl.chain[index],
                          onToggle: _ctrl.toggleEffect,
                          onParamChanged: _ctrl.updateEffectParam,
                          onRemove: _ctrl.removeEffect,
                        ),
                      );
                    },
                  ),
          ),

          // ── Add effect button ──
          Padding(
            padding: const EdgeInsets.all(Spacing.md),
            child: SizedBox(
              width: double.infinity,
              child: OutlinedButton.icon(
                onPressed: _showAddEffectSheet,
                icon: const Icon(Icons.add, size: 16),
                label: const Text(
                  'ADD EFFECT',
                  style: TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w700,
                    letterSpacing: 1.5,
                  ),
                ),
                style: OutlinedButton.styleFrom(
                  foregroundColor: VoxForgeTheme.primary,
                  side: BorderSide(
                    color: VoxForgeTheme.primary.withValues(alpha: 0.3),
                  ),
                  padding: const EdgeInsets.symmetric(vertical: 14),
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(12),
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildEmptyState() {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.auto_fix_high,
              size: 48, color: VoxForgeTheme.textMuted.withValues(alpha: 0.3)),
          const SizedBox(height: 12),
          const Text(
            'No effects in chain',
            style: TextStyle(color: VoxForgeTheme.textMuted, fontSize: 13),
          ),
          const SizedBox(height: 4),
          const Text(
            'Tap "Add Effect" or select a preset',
            style: TextStyle(color: VoxForgeTheme.textMuted, fontSize: 11),
          ),
        ],
      ),
    );
  }
}

class _ControlButton extends StatelessWidget {
  final IconData icon;
  final String label;
  final Color color;
  final VoidCallback onTap;

  const _ControlButton({
    required this.icon,
    required this.label,
    required this.color,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.1),
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: color.withValues(alpha: 0.3), width: 0.5),
        ),
        child: Row(
          children: [
            Icon(icon, size: 16, color: color),
            const SizedBox(width: 6),
            Text(
              label,
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w700,
                letterSpacing: 0.5,
                color: color,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Bottom sheet for adding a new effect to the chain
class _AddEffectSheet extends StatelessWidget {
  final void Function(AudioEffect effect) onAdd;

  const _AddEffectSheet({required this.onAdd});

  @override
  Widget build(BuildContext context) {
    final effects = <(String, String, IconData, AudioEffect Function())>[
      ('Noise Gate', 'Remove silence', Icons.security, () => NoiseGate()),
      ('Noise Suppress', 'Clean up noise', Icons.noise_aware, () => NoiseSuppress()),
      ('Pitch Shift', 'Change pitch', Icons.swap_vert, () => PitchShift()),
      ('Auto-Tune', 'Pitch correction', Icons.music_note, () => AutoTune()),
      ('Equalizer', '10-band EQ', Icons.equalizer, () => Equalizer.defaultBands()),
      ('Compressor', 'Dynamic range', Icons.compress, () => Compressor()),
      ('Limiter', 'Prevent clipping', Icons.horizontal_rule, () => Limiter()),
      ('Reverb', 'Room ambience', Icons.waves, () => Reverb()),
      ('Chorus', 'Thicken sound', Icons.multiline_chart, () => Chorus()),
      ('Delay', 'Echo effect', Icons.timer, () => DelayEffect()),
      ('Distortion', 'Saturation', Icons.electric_bolt, () => Distortion()),
    ];

    return Container(
      padding: const EdgeInsets.all(Spacing.md),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Handle
          Center(
            child: Container(
              width: 36,
              height: 4,
              decoration: BoxDecoration(
                color: VoxForgeTheme.border,
                borderRadius: BorderRadius.circular(2),
              ),
            ),
          ),
          const SizedBox(height: Spacing.md),
          const Text(
            'ADD EFFECT',
            style: TextStyle(
              fontSize: 12,
              fontWeight: FontWeight.w700,
              letterSpacing: 2,
              color: VoxForgeTheme.textMuted,
            ),
          ),
          const SizedBox(height: Spacing.md),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: effects.map((e) {
              return GestureDetector(
                onTap: () => onAdd(e.$4()),
                child: Container(
                  width: (MediaQuery.of(context).size.width - 48 - 16) / 3,
                  padding: const EdgeInsets.symmetric(vertical: 14),
                  decoration: BoxDecoration(
                    color: VoxForgeTheme.bgElevated,
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(color: VoxForgeTheme.border, width: 0.5),
                  ),
                  child: Column(
                    children: [
                      Icon(e.$3, size: 20, color: VoxForgeTheme.primary),
                      const SizedBox(height: 6),
                      Text(
                        e.$1,
                        style: const TextStyle(
                          fontSize: 10,
                          fontWeight: FontWeight.w600,
                          color: VoxForgeTheme.textPrimary,
                        ),
                        textAlign: TextAlign.center,
                      ),
                      Text(
                        e.$2,
                        style: const TextStyle(
                          fontSize: 8,
                          color: VoxForgeTheme.textMuted,
                        ),
                        textAlign: TextAlign.center,
                      ),
                    ],
                  ),
                ),
              );
            }).toList(),
          ),
          const SizedBox(height: Spacing.lg),
        ],
      ),
    );
  }
}
