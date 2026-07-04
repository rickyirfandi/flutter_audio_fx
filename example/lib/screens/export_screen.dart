import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter_audio_fx/flutter_audio_fx.dart';
import 'package:share_plus/share_plus.dart';
import '../controllers/app_controller.dart';
import '../models/recording_project.dart';
import '../theme/voxforge_theme.dart';

class ExportScreen extends StatefulWidget {
  final AppController controller;
  final RecordingProject project;
  const ExportScreen({super.key, required this.controller, required this.project});
  @override
  State<ExportScreen> createState() => _ExportScreenState();
}

class _ExportScreenState extends State<ExportScreen> {
  bool _isWav = false;
  int _mp3Bitrate = 192;
  late final TextEditingController _titleCtrl;
  bool _exporting = false;
  double _progress = 0;
  bool _done = false;
  String? _exportedPath;

  @override
  void initState() {
    super.initState();
    _titleCtrl = TextEditingController(text: widget.project.title);
    widget.controller.addListener(_refresh);
  }

  void _refresh() { if (mounted) setState(() {
    _exporting = widget.controller.isExporting;
    _progress = widget.controller.exportProgress;
  }); }

  @override
  void dispose() {
    widget.controller.removeListener(_refresh);
    _titleCtrl.dispose();
    super.dispose();
  }

  Future<void> _export() async {
    setState(() { _exporting = true; _progress = 0; _done = false; });
    final format = _isWav
        ? const AudioFormat.wav()
        : AudioFormat.mp3(bitrate: _mp3Bitrate);

    final result = await widget.controller.exportProject(widget.project, format);

    setState(() {
      _exporting = false;
      _done = result != null;
      _exportedPath = result;
    });
  }

  Future<void> _share() async {
    if (_exportedPath == null) return;
    final file = XFile(_exportedPath!);
    await Share.shareXFiles([file], text: _titleCtrl.text);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('EXPORT'),
        leading: IconButton(
          icon: const Icon(Icons.arrow_back_ios, size: 18),
          onPressed: () => Navigator.pop(context)),
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(Spacing.md),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          // Recording info
          Container(
            width: double.infinity, padding: const EdgeInsets.all(16),
            decoration: VoxForgeTheme.cardDecoration,
            child: Row(children: [
              Container(width: 48, height: 48,
                decoration: BoxDecoration(gradient: VoxForgeTheme.primaryGradient,
                  borderRadius: BorderRadius.circular(12)),
                child: const Icon(Icons.audio_file, color: VoxForgeTheme.bg, size: 24)),
              const SizedBox(width: 14),
              Expanded(child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                Text(widget.project.title, style: const TextStyle(fontSize: 15,
                  fontWeight: FontWeight.w600, color: VoxForgeTheme.textPrimary)),
                const SizedBox(height: 2),
                Text('${widget.project.formattedDuration} · ${widget.project.mode.name.toUpperCase()}',
                  style: const TextStyle(fontSize: 11, color: VoxForgeTheme.textMuted)),
              ])),
            ]),
          ),
          const SizedBox(height: Spacing.lg),

          // Format
          _header('FORMAT'),
          const SizedBox(height: Spacing.sm),
          Row(children: [
            _fmtBtn('MP3', 'Smaller', Icons.compress, !_isWav, () => setState(() => _isWav = false)),
            const SizedBox(width: 10),
            _fmtBtn('WAV', 'Lossless', Icons.high_quality, _isWav, () => setState(() => _isWav = true)),
          ]),
          const SizedBox(height: Spacing.md),

          if (!_isWav) ...[
            _header('QUALITY'), const SizedBox(height: Spacing.sm),
            Row(children: [128, 192, 320].map((br) {
              final sel = _mp3Bitrate == br;
              return Expanded(child: GestureDetector(
                onTap: () => setState(() => _mp3Bitrate = br),
                child: Container(
                  margin: EdgeInsets.only(right: br == 320 ? 0 : 8),
                  padding: const EdgeInsets.symmetric(vertical: 12),
                  decoration: BoxDecoration(
                    color: sel ? VoxForgeTheme.primary.withOpacity(0.1) : VoxForgeTheme.bgCard,
                    borderRadius: BorderRadius.circular(10),
                    border: Border.all(color: sel ? VoxForgeTheme.primary : VoxForgeTheme.border,
                      width: sel ? 1.5 : 0.5)),
                  alignment: Alignment.center,
                  child: Text('${br}kbps', style: TextStyle(fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: sel ? VoxForgeTheme.primary : VoxForgeTheme.textSecondary)),
                ),
              ));
            }).toList()),
            const SizedBox(height: Spacing.md),
          ],

          // Title
          _header('TITLE'), const SizedBox(height: Spacing.sm),
          TextField(controller: _titleCtrl,
            style: const TextStyle(fontSize: 13, color: VoxForgeTheme.textPrimary),
            decoration: InputDecoration(
              filled: true, fillColor: VoxForgeTheme.bgCard,
              border: OutlineInputBorder(borderRadius: BorderRadius.circular(10),
                borderSide: const BorderSide(color: VoxForgeTheme.border, width: 0.5)),
              enabledBorder: OutlineInputBorder(borderRadius: BorderRadius.circular(10),
                borderSide: const BorderSide(color: VoxForgeTheme.border, width: 0.5)),
              focusedBorder: OutlineInputBorder(borderRadius: BorderRadius.circular(10),
                borderSide: const BorderSide(color: VoxForgeTheme.primary, width: 1)),
              contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12))),
          const SizedBox(height: Spacing.xl),

          // Export / progress / done
          if (_done) _doneWidget()
          else if (_exporting) _progressWidget()
          else _exportButton(),

          // Share button (after export)
          if (_done) ...[
            const SizedBox(height: Spacing.md),
            SizedBox(width: double.infinity, child: OutlinedButton.icon(
              onPressed: _share,
              icon: const Icon(Icons.share, size: 16),
              label: const Text('SHARE', style: TextStyle(fontWeight: FontWeight.w700, letterSpacing: 1.5)),
              style: OutlinedButton.styleFrom(
                foregroundColor: VoxForgeTheme.primary,
                side: BorderSide(color: VoxForgeTheme.primary.withOpacity(0.3)),
                padding: const EdgeInsets.symmetric(vertical: 14),
                shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12))),
            )),
          ],
          const SizedBox(height: Spacing.lg),
        ]),
      ),
    );
  }

  Widget _header(String t) => Text(t, style: const TextStyle(fontSize: 11,
    fontWeight: FontWeight.w700, letterSpacing: 2, color: VoxForgeTheme.textMuted));

  Widget _fmtBtn(String label, String desc, IconData icon, bool sel, VoidCallback onTap) {
    return Expanded(child: GestureDetector(onTap: onTap, child: AnimatedContainer(
      duration: const Duration(milliseconds: 200), padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: sel ? VoxForgeTheme.primary.withOpacity(0.08) : VoxForgeTheme.bgCard,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: sel ? VoxForgeTheme.primary : VoxForgeTheme.border,
          width: sel ? 1.5 : 0.5)),
      child: Column(children: [
        Icon(icon, size: 28, color: sel ? VoxForgeTheme.primary : VoxForgeTheme.textMuted),
        const SizedBox(height: 8),
        Text(label, style: TextStyle(fontSize: 14, fontWeight: FontWeight.w700,
          color: sel ? VoxForgeTheme.primary : VoxForgeTheme.textPrimary)),
        Text(desc, style: const TextStyle(fontSize: 10, color: VoxForgeTheme.textMuted)),
      ]),
    )));
  }

  Widget _exportButton() => SizedBox(width: double.infinity, child: ElevatedButton(
    onPressed: _export,
    style: ElevatedButton.styleFrom(
      backgroundColor: VoxForgeTheme.primary, foregroundColor: VoxForgeTheme.bg,
      padding: const EdgeInsets.symmetric(vertical: 16),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)), elevation: 0),
    child: const Text('EXPORT', style: TextStyle(fontSize: 13, fontWeight: FontWeight.w800, letterSpacing: 2)),
  ));

  Widget _progressWidget() => Column(children: [
    ClipRRect(borderRadius: BorderRadius.circular(4),
      child: LinearProgressIndicator(value: _progress, minHeight: 6,
        backgroundColor: VoxForgeTheme.bgSurface,
        valueColor: const AlwaysStoppedAnimation(VoxForgeTheme.primary))),
    const SizedBox(height: 8),
    Text('Exporting... ${(_progress * 100).toInt()}%',
      style: const TextStyle(fontSize: 12, color: VoxForgeTheme.textSecondary)),
  ]);

  Widget _doneWidget() => Container(
    width: double.infinity, padding: const EdgeInsets.all(20),
    decoration: BoxDecoration(
      color: VoxForgeTheme.success.withOpacity(0.08), borderRadius: BorderRadius.circular(14),
      border: Border.all(color: VoxForgeTheme.success.withOpacity(0.3))),
    child: Column(children: [
      const Icon(Icons.check_circle, color: VoxForgeTheme.success, size: 40),
      const SizedBox(height: 10),
      Text('Exported ${_isWav ? "WAV" : "MP3 ${_mp3Bitrate}kbps"}',
        style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600, color: VoxForgeTheme.success)),
      const SizedBox(height: 4),
      const Text('Ready to share', style: TextStyle(fontSize: 11, color: VoxForgeTheme.textMuted)),
    ]),
  );
}
