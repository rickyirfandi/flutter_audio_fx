import 'package:flutter/material.dart';
import '../controllers/app_controller.dart';
import '../models/recording_project.dart';
import '../theme/voxforge_theme.dart';
import 'editor_screen.dart';

class LibraryScreen extends StatefulWidget {
  final AppController controller;

  const LibraryScreen({super.key, required this.controller});

  @override
  State<LibraryScreen> createState() => _LibraryScreenState();
}

class _LibraryScreenState extends State<LibraryScreen> {
  late final AppController _ctrl;

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

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('RECORDINGS'),
        leading: IconButton(
          icon: const Icon(Icons.arrow_back_ios, size: 18),
          onPressed: () => Navigator.pop(context),
        ),
        actions: [
          Padding(
            padding: const EdgeInsets.only(right: 16),
            child: Center(
              child: Text(
                '${_ctrl.recordings.length} files',
                style: const TextStyle(
                  fontSize: 11,
                  color: VoxForgeTheme.textMuted,
                ),
              ),
            ),
          ),
        ],
      ),
      body: _ctrl.recordings.isEmpty ? _buildEmpty() : _buildList(),
    );
  }

  Widget _buildEmpty() {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.folder_open,
              size: 56, color: VoxForgeTheme.textMuted.withOpacity(0.3)),
          const SizedBox(height: 16),
          const Text(
            'No recordings yet',
            style: TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.w500,
              color: VoxForgeTheme.textMuted,
            ),
          ),
          const SizedBox(height: 6),
          const Text(
            'Hit record on the home screen to get started',
            style: TextStyle(fontSize: 12, color: VoxForgeTheme.textMuted),
          ),
        ],
      ),
    );
  }

  Widget _buildList() {
    return ListView.separated(
      padding: const EdgeInsets.all(Spacing.md),
      itemCount: _ctrl.recordings.length,
      separatorBuilder: (_, __) => const SizedBox(height: 8),
      itemBuilder: (context, index) {
        final rec = _ctrl.recordings[index];
        return Dismissible(
          key: Key(rec.id),
          direction: DismissDirection.endToStart,
          onDismissed: (_) => _ctrl.deleteRecording(rec.id),
          background: Container(
            alignment: Alignment.centerRight,
            padding: const EdgeInsets.only(right: 20),
            decoration: BoxDecoration(
              color: VoxForgeTheme.danger.withOpacity(0.15),
              borderRadius: BorderRadius.circular(14),
            ),
            child: const Icon(Icons.delete_outline,
                color: VoxForgeTheme.danger, size: 22),
          ),
          child: _RecordingTile(
            project: rec,
            onTap: () {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (_) => EditorScreen(
                    controller: _ctrl,
                    project: rec,
                  ),
                ),
              );
            },
          ),
        );
      },
    );
  }
}

class _RecordingTile extends StatelessWidget {
  final RecordingProject project;
  final VoidCallback onTap;

  const _RecordingTile({required this.project, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.all(14),
        decoration: VoxForgeTheme.cardDecoration,
        child: Row(
          children: [
            // Waveform thumbnail placeholder
            Container(
              width: 48,
              height: 48,
              decoration: BoxDecoration(
                color: VoxForgeTheme.bgSurface,
                borderRadius: BorderRadius.circular(10),
              ),
              child: Icon(
                project.mode == RecordingMode.live
                    ? Icons.bolt
                    : Icons.tune,
                color: project.mode == RecordingMode.live
                    ? VoxForgeTheme.accent
                    : VoxForgeTheme.primary,
                size: 22,
              ),
            ),
            const SizedBox(width: 14),
            // Info
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    project.title,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w600,
                      color: VoxForgeTheme.textPrimary,
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 3),
                  Row(
                    children: [
                      Text(
                        project.formattedDuration,
                        style: const TextStyle(
                          fontSize: 11,
                          fontFamily: 'monospace',
                          color: VoxForgeTheme.textMuted,
                        ),
                      ),
                      const SizedBox(width: 8),
                      Container(
                        padding: const EdgeInsets.symmetric(
                            horizontal: 6, vertical: 2),
                        decoration: BoxDecoration(
                          color: (project.mode == RecordingMode.live
                                  ? VoxForgeTheme.accent
                                  : VoxForgeTheme.primary)
                              .withOpacity(0.1),
                          borderRadius: BorderRadius.circular(4),
                        ),
                        child: Text(
                          project.mode.name.toUpperCase(),
                          style: TextStyle(
                            fontSize: 8,
                            fontWeight: FontWeight.w700,
                            letterSpacing: 1,
                            color: project.mode == RecordingMode.live
                                ? VoxForgeTheme.accent
                                : VoxForgeTheme.primary,
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      Text(
                        project.formattedDate,
                        style: const TextStyle(
                          fontSize: 10,
                          color: VoxForgeTheme.textMuted,
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
            // Arrow
            const Icon(Icons.chevron_right,
                size: 18, color: VoxForgeTheme.textMuted),
          ],
        ),
      ),
    );
  }
}
