# VoxForge — Technical Documentation

**Version:** 0.1.0  
**Last Updated:** April 2026

---

## 1. Architecture

### 1.1 High-Level

```
┌─────────────────────────────────────────────────────┐
│                    VoxForge App                       │
│                                                      │
│  ┌──────────────────────────────────────────────┐    │
│  │            Presentation Layer                 │    │
│  │  HomeScreen │ EditorScreen │ ExportScreen │ Library │
│  │      │            │              │            │    │
│  │      └────────────┼──────────────┘            │    │
│  │                   ▼                           │    │
│  │           AppController                       │    │
│  │         (ChangeNotifier)                      │    │
│  └───────────────────┬──────────────────────────┘    │
│                      │                               │
│  ┌───────────────────┴──────────────────────────┐    │
│  │          flutter_audio_fx Package             │    │
│  │                                               │    │
│  │  AudioFxEngine → Rust API → AudioRuntime      │    │
│  │  11 Effects │ Presets │ Visualizer Widgets     │    │
│  └───────────────────────────────────────────────┘    │
│                                                      │
│  ┌──────────────────────────────────────────────┐    │
│  │           Platform Services                   │    │
│  │  permission_handler │ path_provider │ share_plus │
│  └──────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

### 1.2 Dependency Graph

```
VoxForge App
  ├── flutter_audio_fx (local path dependency)
  │     ├── flutter_rust_bridge ^2.9.0
  │     ├── ffi ^2.1.0
  │     ├── path_provider ^2.1.0
  │     └── permission_handler ^11.3.0
  ├── share_plus ^10.0.0
  ├── path_provider ^2.1.0
  └── permission_handler ^11.3.0
```

---

## 2. State Management

### 2.1 AppController

Single `ChangeNotifier` that owns all app state. Screens subscribe via `addListener()` and call `setState()` on change.

```
AppController
  │
  ├── AudioFxEngine engine          → owns Rust bridge lifecycle
  │
  ├── Recording State
  │   ├── RecordingMode _mode       → live | studio
  │   ├── bool _isRecording         → mic active
  │   ├── bool _isPlaying           → preview playback active
  │   ├── Duration _elapsed         → recording timer
  │   └── Timer? _timer             → 50ms periodic tick
  │
  ├── Project State
  │   ├── List<RecordingProject> _recordings
  │   ├── RecordingProject? _currentProject
  │   └── String _activePresetId
  │
  ├── Effect Chain
  │   └── List<AudioEffect> _chain  → current active chain
  │
  └── Export State
      ├── bool _isExporting
      └── double _exportProgress
```

### 2.2 Data Flow: Recording

```
User taps Record
  → AppController.startRecording()
    → Create RecordingProject with unique ID
    → Create directory: /recordings/{id}/
    → Start 50ms timer for elapsed counter
    → engine.setChain(current chain)
    → engine.startMicWithRecording(rawPath, procPath?)
      → Rust: open cpal streams, start ring buffer processing
    → notifyListeners()
      → HomeScreen rebuilds (shows timer, pulse animation)

User taps Stop
  → AppController.stopRecording()
    → Cancel timer
    → engine.stop()
      → Rust: drop cpal streams, save WAV files from buffers
    → Create final RecordingProject with duration + chain
    → Insert at recordings[0]
    → notifyListeners()
      → HomeScreen rebuilds, then navigates to EditorScreen
```

### 2.3 Data Flow: Effect Parameter Update

```
User drags slider in EditorScreen
  → EffectCard.onParamChanged(index, "room_size", 0.7)
    → AppController.updateEffectParam(index, "room_size", 0.7)
      → _chain[index].updateParam("room_size", 0.7)  // Dart model
      → engine.updateParam(index, "room_size", 0.7)   // → Rust
        → rust: engine_update_param()
          → runtime.update_param(idx, name, value)
            → chain[idx].set_param("room_size", 0.7)
              → AtomicF32.set(0.7)  // Lock-free, RT-safe
      → notifyListeners()
        → Slider rebuilds with new value

Total latency: <1ms (atomic write, no locks on audio thread)
```

### 2.4 Data Flow: A/B Compare

```
User taps A/B button (going DRY)
  → Save current enabled states: [true, true, false, true, ...]
  → AppController.setAllEffectsEnabled(false)
    → For each effect: engine.toggleEffect(i, false)
      → Rust: chain[i].set_enabled(false)  // AtomicBool
  → Audio immediately bypasses all effects

User taps A/B again (going WET)
  → Restore saved states
  → For each: if saved[i] != current[i], toggleEffect(i)
  → Audio restores to previous configuration
```

---

## 3. Screen Implementations

### 3.1 HomeScreen

**Widget tree:**
```
Scaffold
  └── SafeArea → Column
        ├── TopBar (logo + library + tune buttons)
        ├── ModeToggle (animated segmented control)
        ├── VisualizerPanel (CustomPainter, 48 bars)
        ├── MiniTimer (monospace text)
        ├── ActiveEffectsBar (horizontal chips, Live only)
        ├── Spacer
        ├── RecordButton (Stack: outer ring + inner button + pulse)
        ├── PresetCarousel (horizontal ListView)
        └── ChainSummary (text in card)
```

**VisualizerPanel:** Uses `AnimationController` at 80ms period with `SingleTickerProviderStateMixin`. CustomPainter draws 48 bars using sinusoidal pattern + random noise when active, subtle breathing animation when idle. No real audio data needed for visual demo.

**RecordButton animation:** `AnimatedContainer` morphs between circle (radius 34) and rounded rectangle (radius 8) over 200ms. Outer pulse ring uses separate `AnimationController` that `repeat(reverse: true)` during recording.

### 3.2 EditorScreen

**Effect chain reordering:** Uses `ReorderableListView.builder` with custom `proxyDecorator` that adds elevation shadow during drag. Each `EffectCard` is keyed by index.

**Effect cards:** `StatefulWidget` with `_expanded` bool. Collapsed shows: drag handle + index badge + icon + name + switch + chevron. Expanded adds: param sliders + remove button. All params dynamically generated from `effect.toParams()` with range lookup table.

**Param range lookup:** Pattern-matched by `(effectType, paramName)` tuple to return `(min, max)`. Example: `('reverb', 'room_size') → (0.0, 1.0)`, `('compressor', 'threshold_db') → (-60.0, 0.0)`.

### 3.3 ExportScreen

**Export flow:**
1. User selects format (WAV/MP3) and quality
2. Taps Export → `setState(_exporting = true)`
3. `AppController.exportProject()` calls `engine.processFile()`
4. Progress updates via `onProgress` callback → `notifyListeners()` → progress bar rebuilds
5. On completion: done state with green checkmark
6. Share button → `Share.shareXFiles([XFile(path)])` via share_plus

**Actual sharing:** Uses `share_plus` package which invokes the native platform share sheet. Works with WhatsApp, TikTok, Instagram, Twitter, email, AirDrop, etc. without needing platform-specific SDKs.

### 3.4 LibraryScreen

**Dismissible for delete:** Wraps each `_RecordingTile` in `Dismissible` with `endToStart` direction. Red background with delete icon. `onDismissed` calls `AppController.deleteRecording(id)`.

**Mode badge:** Small colored chip showing "LIVE" (orange) or "STUDIO" (cyan) on each recording tile.

---

## 4. File System

### 4.1 Directory Structure

```
{getApplicationDocumentsDirectory()}/
├── recordings/
│   ├── rec_1713024000000/
│   │   ├── raw.wav              ← Original mic recording
│   │   └── processed.wav        ← With effects applied (Live mode)
│   ├── rec_1713024060000/
│   │   └── raw.wav
│   └── ...
└── exports/
    ├── rec_1713024000000.wav    ← Exported file
    └── rec_1713024060000.wav
```

### 4.2 File Lifecycle

| Event | Files Created |
|-------|--------------|
| Start recording (Studio) | `recordings/{id}/raw.wav` (written on stop) |
| Start recording (Live) | `raw.wav` + `processed.wav` (written on stop) |
| Export | `exports/{id}.wav` (or `.mp3`) |
| Delete recording | Removes entire `recordings/{id}/` directory |

---

## 5. Permission Handling

### 5.1 Flow

```
App launch
  → _PermissionGate checks Permission.microphone.isGranted
    → If granted → show HomeScreen
    → If not granted → show permission request screen
      → User taps "Enable Microphone"
        → Permission.microphone.request()
          → Granted → show HomeScreen
          → Denied → stay on request screen
          → PermanentlyDenied → show dialog → openAppSettings()
```

### 5.2 Runtime Permission Check

Before every `startRecording()`, the HomeScreen calls `PermissionHelper.requestWithDialog(context)`. This handles the edge case where the user revokes permission while the app is open.

---

## 6. Theme System

### 6.1 VoxForgeTheme

All colors, gradients, and decorations are defined as `static const` or `static get` in `VoxForgeTheme`. No hardcoded colors in widgets.

```dart
VoxForgeTheme.primary       // #00E5CC — cyan
VoxForgeTheme.accent        // #FF6B35 — orange
VoxForgeTheme.danger        // #FF3B5C — red
VoxForgeTheme.bgCard        // #13131A — card background
VoxForgeTheme.cardDecoration // BoxDecoration with border + radius
```

### 6.2 Spacing

```dart
Spacing.xs  = 4.0
Spacing.sm  = 8.0
Spacing.md  = 16.0
Spacing.lg  = 24.0
Spacing.xl  = 32.0
Spacing.xxl = 48.0
```

### 6.3 ThemeData

Dark theme with custom `SliderThemeData`, `SwitchThemeData`, `AppBarTheme`, `BottomSheetThemeData`. All configured in `VoxForgeTheme.dark` getter.

---

## 7. Widget Catalog

| Widget | Type | Props | Notes |
|--------|------|-------|-------|
| ModeToggle | Stateless | mode, onChanged, enabled | Animated segmented control |
| RecordButton | Stateless | isRecording, pulseAnimation, onTap | Circle→square morph + pulse ring |
| VisualizerPanel | Stateful | engine, isActive | CustomPainter, 48 bars, grid |
| PresetCarousel | Stateless | activePresetId, onPresetSelected | Horizontal ListView |
| ActiveEffectsBar | Stateless | chain, onToggle | Horizontal chips |
| EffectCard | Stateful | index, effect, onToggle, onParamChanged, onRemove | Expandable + sliders |
| MiniTimer | Stateless | elapsed, isRecording | Monospace, tabular figures |
| _AddEffectSheet | Stateless | onAdd | BottomSheet with 3-column grid |
| _RecordingTile | Stateless | project, onTap | Card with icon + info |

---

## 8. Error Handling

| Scenario | Handling |
|----------|---------|
| Mic permission denied | Permission gate prevents app access; dialog to settings |
| Audio device not found | try/catch in AppController; debugPrint; no crash |
| Recording start fails | Fallback to mic-only (no file recording) |
| File write fails | Silently skips file save; recording still in memory |
| Export fails | Returns null; UI stays on export screen with option to retry |
| Engine already running | StateError caught; UI prevents double-start via isRecording check |
| cpal stream drops | is_running AtomicBool prevents callbacks from processing dead data |

---

## 9. Build & Run

### 9.1 Prerequisites

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-linux-android armv7-linux-androideabi
cargo install cargo-ndk flutter_rust_bridge_codegen
```

### 9.2 First Run

```bash
cd flutter_audio_fx
make setup        # Install deps
make codegen      # Generate FRB bindings

# Enable bridge:
# 1. lib/src/engine/audio_fx_engine.dart → _kBridgeReady = true + uncomment imports
# 2. example/lib/main.dart → uncomment RustLib.init()

cd example
flutter run        # Must use physical device (mic needs real hardware)
```

### 9.3 Android Manifest

```xml
<uses-permission android:name="android.permission.RECORD_AUDIO" />
```

### 9.4 iOS Info.plist

```xml
<key>NSMicrophoneUsageDescription</key>
<string>VoxForge needs microphone access to record and process audio.</string>
```

---

## 10. Testing Strategy

### 10.1 Unit Tests

| Area | Tests |
|------|-------|
| RecordingProject model | Create, format duration, format date |
| Effect serialization | toJson/fromJson roundtrip for all 11 effects |
| Preset creation | All 11 presets create valid chains |
| EngineConfig | Latency calculation |
| EqBand | fromJson parsing |

### 10.2 Widget Tests

| Widget | Tests |
|--------|-------|
| ModeToggle | Renders both modes; tap switches; disabled state |
| RecordButton | Renders circle/square states; tap callback |
| PresetCarousel | Renders all presets; tap selects; active state |
| EffectCard | Renders collapsed/expanded; slider interaction; remove |
| MiniTimer | Renders elapsed time; isRecording styling |

### 10.3 Integration Tests

| Flow | Test |
|------|------|
| Record + Stop | Permission → record → timer ticks → stop → project created |
| Preset → Record | Select preset → record → editor opens with chain |
| Edit chain | Add effect → reorder → remove → chain state correct |
| Export | Record → stop → export → file exists |

### 10.4 Manual Testing Matrix

| Device | Android | iOS |
|--------|---------|-----|
| Low-end (Snapdragon 680) | Latency + CPU | — |
| Mid-range (Snapdragon 778) | Full test suite | — |
| High-end (Snapdragon 8 Gen 3) | Performance baseline | — |
| iPhone 13 | — | Full test suite |
| iPhone 15 Pro | — | Performance baseline |

---

## 11. Performance Budget

| Metric | Budget | Notes |
|--------|--------|-------|
| App cold start | <2s | Flutter engine + Rust init |
| Recording start | <300ms | cpal stream open |
| Preset switch | <50ms | Chain rebuild + sync to Rust |
| Slider response | <16ms | Atomic param update, no rebuild |
| Export (1 min WAV) | <6s | Offline processing |
| Memory (idle) | <80MB | Flutter + Rust engine |
| Memory (recording) | <120MB | + recording buffers |
| APK size | <30MB | Flutter + Rust .so libs |
