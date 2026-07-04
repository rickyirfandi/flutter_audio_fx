import 'package:flutter/material.dart';
import 'package:permission_handler/permission_handler.dart';

class PermissionHelper {
  static Future<bool> hasMicPermission() => Permission.microphone.isGranted;

  static Future<bool> requestMicPermission() async =>
      (await Permission.microphone.request()).isGranted;

  static Future<bool> requestWithDialog(BuildContext context) async {
    if (await hasMicPermission()) return true;
    if (await Permission.microphone.isPermanentlyDenied) {
      if (!context.mounted) return false;
      final go = await showDialog<bool>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: const Text('Microphone Access'),
          content: const Text('VoxForge needs mic access. Enable in settings.'),
          actions: [
            TextButton(onPressed: () => Navigator.pop(ctx, false), child: const Text('Cancel')),
            TextButton(onPressed: () => Navigator.pop(ctx, true), child: const Text('Settings')),
          ],
        ),
      );
      if (go == true) await openAppSettings();
      return false;
    }
    return requestMicPermission();
  }
}
