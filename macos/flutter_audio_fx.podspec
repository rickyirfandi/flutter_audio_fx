Pod::Spec.new do |s|
  s.name             = 'flutter_audio_fx'
  s.version          = '0.3.0'
  s.summary          = 'High-performance real-time audio DSP for Flutter (Rust core).'
  s.description      = 'Real-time audio effects implemented in Rust, exposed via dart:ffi.'
  s.homepage         = 'https://github.com/rickyirfandi/flutter_audio_fx'
  s.license          = { :file => '../LICENSE' }
  s.author           = { 'flutter_audio_fx authors' => 'noreply@flutter_audio_fx.dev' }
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.dependency 'FlutterMacOS'
  s.platform = :osx, '10.14'
  s.vendored_libraries = 'libflutter_audio_fx_core.a'
  s.libraries = 'flutter_audio_fx_core', 'c++'
  s.frameworks = 'CoreAudio', 'AudioUnit'
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'OTHER_LDFLAGS' => '-force_load $(PODS_TARGET_SRCROOT)/libflutter_audio_fx_core.a',
  }
  s.swift_version = '5.0'
end
