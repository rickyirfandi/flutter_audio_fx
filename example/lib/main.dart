import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'theme/app_theme.dart';
import 'screens/home_screen.dart';
import 'utils/permission_helper.dart';

// UNCOMMENT after `flutter_rust_bridge_codegen generate`:
// import 'package:flutter_audio_fx/src/rust/frb_generated.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // UNCOMMENT: await RustLib.init();
  SystemChrome.setPreferredOrientations([DeviceOrientation.portraitUp]);
  SystemChrome.setSystemUIOverlayStyle(const SystemUiOverlayStyle(
    statusBarColor: Colors.transparent,
    statusBarIconBrightness: Brightness.light,
    systemNavigationBarColor: Color(0xFF0A0A0F),
  ));
  runApp(const AudioFxExampleApp());
}

class AudioFxExampleApp extends StatelessWidget {
  const AudioFxExampleApp({super.key});
  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'flutter_audio_fx example',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.dark,
      home: const _PermissionGate(),
    );
  }
}

class _PermissionGate extends StatefulWidget {
  const _PermissionGate();
  @override
  State<_PermissionGate> createState() => _PermissionGateState();
}

class _PermissionGateState extends State<_PermissionGate> {
  bool _ok = false, _checking = true;

  @override
  void initState() {
    super.initState();
    _check();
  }

  Future<void> _check() async {
    final g = await PermissionHelper.hasMicPermission();
    if (mounted) {
      setState(() {
        _ok = g;
        _checking = false;
      });
    }
  }

  Future<void> _request() async {
    final g = await PermissionHelper.requestWithDialog(context);
    if (mounted) setState(() => _ok = g);
  }

  @override
  Widget build(BuildContext context) {
    if (_checking) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }
    if (_ok) return const HomeScreen();
    return Scaffold(
      body: Center(
          child: Padding(
        padding: const EdgeInsets.all(40),
        child: Column(mainAxisSize: MainAxisSize.min, children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 7),
            decoration: BoxDecoration(
                border: Border.all(color: AppTheme.primary, width: 1.5),
                borderRadius: BorderRadius.circular(6)),
            child: const Text('VOXFORGE',
                style: TextStyle(
                    fontSize: 18,
                    fontWeight: FontWeight.w900,
                    letterSpacing: 4,
                    color: AppTheme.primary)),
          ),
          const SizedBox(height: 32),
          Icon(Icons.mic_off,
              size: 56, color: AppTheme.textMuted.withValues(alpha: 0.4)),
          const SizedBox(height: 20),
          const Text('Microphone Access Required',
              style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600)),
          const SizedBox(height: 8),
          const Text('This demo needs mic access to record and process audio.',
              style: TextStyle(fontSize: 13, color: AppTheme.textSecondary),
              textAlign: TextAlign.center),
          const SizedBox(height: 28),
          SizedBox(
              width: double.infinity,
              child: ElevatedButton(
                onPressed: _request,
                style: ElevatedButton.styleFrom(
                    backgroundColor: AppTheme.primary,
                    foregroundColor: AppTheme.bg,
                    padding: const EdgeInsets.symmetric(vertical: 14),
                    shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(12))),
                child: const Text('ENABLE MICROPHONE',
                    style: TextStyle(
                        fontWeight: FontWeight.w800, letterSpacing: 1.5)),
              )),
        ]),
      )),
    );
  }
}
