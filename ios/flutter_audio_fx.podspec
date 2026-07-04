Pod::Spec.new do |s|
  s.name             = 'flutter_audio_fx'
  s.version          = '0.2.0'
  s.summary          = 'High-performance real-time audio DSP for Flutter (Rust core).'
  s.description      = <<-DESC
    Real-time audio effects (pitch shift, auto-tune, noise suppression,
    equalizer, reverb and more) implemented in Rust and exposed to Flutter
    via dart:ffi.
  DESC
  s.homepage         = 'https://github.com/flutter_audio_fx/flutter_audio_fx'
  s.license          = { :file => '../LICENSE' }
  s.author           = { 'flutter_audio_fx authors' => 'noreply@flutter_audio_fx.dev' }
  s.source           = { :path => '.' }
  s.source_files     = 'Classes/**/*'
  s.public_header_files = 'Classes/**/*.h'
  s.dependency 'Flutter'
  s.platform = :ios, '13.0'
  s.frameworks = 'AVFoundation', 'AudioToolbox'
  s.swift_version = '5.0'

  # Prefer the xcframework (proper device + simulator separation). Fall back to
  # the legacy lipo'd universal .a if only that is present in the package
  # directory. CI publishes both via the build-native workflow.
  if File.exist?(File.expand_path('flutter_audio_fx_core.xcframework', __dir__))
    s.vendored_frameworks = 'flutter_audio_fx_core.xcframework'
    s.pod_target_xcconfig = {
      'DEFINES_MODULE' => 'YES',
    }
  else
    s.vendored_libraries = 'libflutter_audio_fx_core.a'
    s.libraries = 'flutter_audio_fx_core', 'c++'
    s.pod_target_xcconfig = {
      'DEFINES_MODULE' => 'YES',
      'OTHER_LDFLAGS' => '-force_load $(PODS_TARGET_SRCROOT)/libflutter_audio_fx_core.a',
    }
  end
end
