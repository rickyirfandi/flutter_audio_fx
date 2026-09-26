import 'package:flutter/material.dart';
import '../controllers/app_controller.dart';
import '../models/recording_project.dart';
import '../theme/app_theme.dart';
import '../utils/permission_helper.dart';
import '../widgets/mode_toggle.dart';
import '../widgets/record_button.dart';
import '../widgets/preset_carousel.dart';
import '../widgets/visualizer_panel.dart';
import '../widgets/active_effects_bar.dart';
import '../widgets/mini_timer.dart';
import 'editor_screen.dart';
import 'library_screen.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});
  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> with TickerProviderStateMixin {
  late final AppController _ctrl;
  late final AnimationController _pulse;

  @override
  void initState() {
    super.initState();
    _ctrl = AppController()..addListener(_refresh);
    _pulse = AnimationController(
        vsync: this, duration: const Duration(milliseconds: 1200));
  }

  void _refresh() {
    if (!mounted) return;
    setState(() {});
    if (_ctrl.isRecording && !_pulse.isAnimating) {
      _pulse.repeat(reverse: true);
    } else if (!_ctrl.isRecording && _pulse.isAnimating) {
      _pulse.stop();
      _pulse.reset();
    }
    final error = _ctrl.lastError;
    if (error != null) {
      _ctrl.clearError();
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(error)));
    }
  }

  @override
  void dispose() {
    _ctrl.removeListener(_refresh);
    _ctrl.dispose();
    _pulse.dispose();
    super.dispose();
  }

  Future<void> _onRecordTap() async {
    if (_ctrl.isRecording) {
      await _ctrl.stopRecording();
      if (_ctrl.currentProject != null && mounted) {
        Navigator.push(
            context,
            MaterialPageRoute(
                builder: (_) => EditorScreen(
                    controller: _ctrl, project: _ctrl.currentProject!)));
      }
    } else {
      final ok = await PermissionHelper.requestWithDialog(context);
      if (ok) _ctrl.startRecording();
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        body: SafeArea(
            child: Column(children: [
      // Top bar
      Padding(
        padding: const EdgeInsets.symmetric(
            horizontal: Spacing.md, vertical: Spacing.sm),
        child: Row(children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
            decoration: BoxDecoration(
                border: Border.all(color: AppTheme.primary, width: 1.5),
                borderRadius: BorderRadius.circular(6)),
            child: const Text('VOXFORGE',
                style: TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w900,
                    letterSpacing: 3,
                    color: AppTheme.primary)),
          ),
          const Spacer(),
          IconButton(
              onPressed: () => Navigator.push(
                  context,
                  MaterialPageRoute(
                      builder: (_) => LibraryScreen(controller: _ctrl))),
              icon: const Icon(Icons.folder_outlined, size: 22),
              color: AppTheme.textSecondary),
          IconButton(
              onPressed: () {
                final p = _ctrl.currentProject ??
                    RecordingProject.create(mode: _ctrl.mode);
                Navigator.push(
                    context,
                    MaterialPageRoute(
                        builder: (_) =>
                            EditorScreen(controller: _ctrl, project: p)));
              },
              icon: const Icon(Icons.tune, size: 22),
              color: AppTheme.textSecondary),
        ]),
      ),
      const SizedBox(height: Spacing.md),

      ModeToggle(
          mode: _ctrl.mode,
          onChanged: _ctrl.setMode,
          enabled: !_ctrl.isRecording),
      const SizedBox(height: Spacing.lg),

      Padding(
          padding: const EdgeInsets.symmetric(horizontal: Spacing.md),
          child: VisualizerPanel(
              engine: _ctrl.engine, isActive: _ctrl.isRecording)),
      const SizedBox(height: Spacing.lg),

      MiniTimer(
          elapsed: _ctrl.elapsedFormatted, isRecording: _ctrl.isRecording),
      const SizedBox(height: Spacing.sm),

      if (_ctrl.mode == RecordingMode.live) ...[
        ActiveEffectsBar(chain: _ctrl.chain, onToggle: _ctrl.toggleEffect),
        const SizedBox(height: Spacing.md),
      ],

      const Spacer(),

      RecordButton(
          isRecording: _ctrl.isRecording,
          pulseAnimation: _pulse,
          onTap: _onRecordTap),
      const SizedBox(height: Spacing.lg),

      if (!_ctrl.isRecording) ...[
        const Padding(
            padding: EdgeInsets.symmetric(horizontal: Spacing.md),
            child: Row(children: [
              Text('PRESETS',
                  style: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w700,
                      letterSpacing: 2,
                      color: AppTheme.textMuted)),
              SizedBox(width: 8),
              Expanded(child: Divider(color: AppTheme.border, height: 1)),
            ])),
        const SizedBox(height: Spacing.sm),
        PresetCarousel(
            activePresetId: _ctrl.activePresetId,
            onPresetSelected: _ctrl.applyPreset),
        const SizedBox(height: Spacing.md),
        Padding(
            padding: const EdgeInsets.symmetric(horizontal: Spacing.md),
            child: Container(
              width: double.infinity,
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                  color: AppTheme.bgCard,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(color: AppTheme.border, width: 0.5)),
              child: Text(
                  _ctrl.chainSummary.isEmpty
                      ? 'No effects'
                      : _ctrl.chainSummary,
                  style: const TextStyle(
                      fontSize: 11,
                      color: AppTheme.primary,
                      letterSpacing: 0.5),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis),
            )),
      ],
      const SizedBox(height: Spacing.md),
    ])));
  }
}
