# VoxForge — Product Requirements Document

**Version:** 0.1.0  
**Author:** Ricky  
**Date:** April 2026  
**Status:** MVP Development  
**Role:** Example app for flutter_audio_fx + standalone product

---

## 1. Overview

VoxForge is a Flutter audio recording app that lets users record their voice, apply real-time DSP effects (auto-tune, pitch shift, reverb, noise removal, etc.), and export/share the result. It serves as both the reference example for the `flutter_audio_fx` package and a standalone consumer product.

### 1.1 Problem Statement

Mobile voice recording apps are either:
- **Too simple** — basic recorder with no effects (Voice Memos, Samsung Recorder)
- **Too complex** — full DAWs ported to mobile (GarageBand, BandLab) with overwhelming UIs
- **Effect-only** — fun voice changers with no recording/export workflow (Voice Changer Plus)

No app offers: record → apply pro-grade effects → preview → tweak → export → share, with a focused voice-first UX and real-time monitoring.

### 1.2 Solution

VoxForge bridges the gap: a recording app with studio-quality DSP in a mobile-native UX. Two modes cover both use cases:

- **Live Mode** — hear effects applied in real-time while recording (content creators, karaoke)
- **Studio Mode** — record clean audio first, apply/tweak effects after (podcasters, musicians)

Both modes save raw audio, so effects are always non-destructive.

### 1.3 Target Audience

| Segment | Use Case | Key Feature |
|---------|----------|-------------|
| Content creators | TikTok/Reels audio | Live auto-tune + presets |
| Karaoke singers | Fun singing recordings | Reverb + pitch correction + share |
| Podcasters | Clean voice recordings | Noise suppress + EQ + compress |
| Musicians | Voice memos with effects | Studio mode + full editor |
| Casual users | Voice changer fun | Presets (chipmunk, deep, robot) |

---

## 2. Core User Flows

### 2.1 Quick Record (Happy Path)

```
Open app → Tap preset (T-Pain) → Tap record → Sing/talk
→ Tap stop → Auto-opens editor → Preview → Export → Share to WhatsApp
```

Time to first recording: <10 seconds.

### 2.2 Studio Recording

```
Open app → Toggle to STUDIO → Tap record → Record clean audio
→ Stop → Editor opens → Add effects → Tweak params → A/B compare
→ Satisfied → Export as MP3 → Share to TikTok
```

### 2.3 Browse & Re-Edit

```
Open app → Tap library icon → Browse recordings → Tap one
→ Editor opens → Change preset → Re-export with new effects
```

---

## 3. Screens & Features

### 3.1 Home Screen (Recording)

| Element | Behavior |
|---------|----------|
| VOXFORGE logo | Top-left branding |
| Library icon | Opens recording library |
| Tune icon | Opens effect editor |
| Mode toggle (LIVE / STUDIO) | Switches recording mode; disabled during recording |
| Visualizer panel | 48-bar FFT animation; shows LIVE/IDLE badge |
| Timer | MM:SS.ms elapsed time; monospace font |
| Active effects bar | (Live mode only) Horizontal chips showing active effects; tap to toggle |
| Record button | Circle with gradient; morphs to rounded square when recording; pulse animation |
| Preset carousel | Horizontal scroll of 11 preset cards with emoji + name |
| Chain summary | Single-line text showing current effect chain |

### 3.2 Effect Editor Screen

| Element | Behavior |
|---------|----------|
| Visualizer panel | Same as home screen; shows preview audio |
| Playback controls | Play/Stop button, A/B wet/dry compare, duration display |
| Effect chain list | Drag-to-reorder list of effect cards |
| Effect card | Expandable: shows icon, name, on/off toggle, index number; expanded shows param sliders + remove button |
| Add Effect button | Opens bottom sheet with all 11 effects as tappable grid |
| Export button | Top-right, navigates to export screen |

**A/B Compare:** Toggles all effects off (DRY) or restores previous enabled states (WET). Saves states so toggling back restores exact configuration.

### 3.3 Export Screen

| Element | Behavior |
|---------|----------|
| Recording info card | Title, duration, mode badge |
| Format selector | WAV (lossless) or MP3 (smaller) |
| MP3 quality | 128 / 192 / 320 kbps buttons (only shown for MP3) |
| Title field | Editable text input |
| Export button | Triggers offline processing; shows progress bar |
| Done state | Green checkmark card |
| Share button | Uses share_plus to open platform share sheet |

### 3.4 Library Screen

| Element | Behavior |
|---------|----------|
| Recording tiles | Card with waveform icon, title, duration, mode badge, date |
| Swipe to delete | Dismissible with red background |
| Tap to edit | Opens editor screen for that recording |
| Empty state | Folder icon + "No recordings yet" message |

---

## 4. Design System

### 4.1 Color Palette

| Token | Hex | Usage |
|-------|-----|-------|
| bg | #0A0A0F | Main background |
| bgCard | #13131A | Card surfaces |
| bgElevated | #1A1A24 | Elevated surfaces |
| bgSurface | #22222E | Input fields, track backgrounds |
| primary | #00E5CC | Cyan — actions, active states, chain text |
| accent | #FF6B35 | Warm orange — live mode, secondary actions |
| danger | #FF3B5C | Record button, destructive actions |
| success | #4ADE80 | Export complete |
| textPrimary | #F0F0F5 | Main text |
| textSecondary | #8888A0 | Supporting text |
| textMuted | #555570 | Disabled, labels |
| border | #2A2A38 | Card borders, dividers |

### 4.2 Typography

Monospace system font throughout. Letter-spacing used for labels and headers (1.5-3px). Tabular figures enabled on timer display.

### 4.3 Interaction Patterns

- Record button: circle → rounded square morph (200ms ease)
- Cards: AnimatedContainer (200ms) on selection state changes
- Preset carousel: horizontal ListView with snap
- Effect chain: ReorderableListView with elevation proxy
- Sliders: 3px track, 7px thumb, cyan active color

---

## 5. App Architecture

### 5.1 State Management

```
AppController (ChangeNotifier)
  ├── engine: AudioFxEngine         ← Rust bridge
  ├── _mode: RecordingMode          ← live / studio
  ├── _isRecording: bool
  ├── _isPlaying: bool
  ├── _elapsed: Duration
  ├── _chain: List<AudioEffect>     ← current effect chain
  ├── _activePresetId: String
  ├── _recordings: List<RecordingProject>
  ├── _currentProject: RecordingProject?
  ├── _isExporting: bool
  └── _exportProgress: double
```

Screens listen via `addListener()`. Single controller owns all state — clean, testable, easy to migrate to Riverpod/Bloc later.

### 5.2 File Structure

```
example/lib/
├── main.dart                    Entry + permission gate
├── controllers/
│   └── app_controller.dart      All app state + engine bridge
├── models/
│   └── recording_project.dart   Recording metadata model
├── screens/
│   ├── home_screen.dart         Main recording screen
│   ├── editor_screen.dart       Effect chain editor
│   ├── export_screen.dart       Export + share
│   └── library_screen.dart      Recording browser
├── widgets/
│   ├── mode_toggle.dart         LIVE / STUDIO switch
│   ├── record_button.dart       Animated record button
│   ├── visualizer_panel.dart    FFT bar animation
│   ├── preset_carousel.dart     Horizontal preset list
│   ├── active_effects_bar.dart  Quick-toggle effect chips
│   ├── effect_card.dart         Expandable effect with sliders
│   └── mini_timer.dart          Large monospace timer
├── theme/
│   └── voxforge_theme.dart      Colors, typography, decorations
└── utils/
    └── permission_helper.dart   Mic permission handling
```

### 5.3 Data Model

```dart
RecordingProject {
  id: String              // "rec_1713024000000"
  title: String           // "Recording 14:30"
  createdAt: DateTime
  duration: Duration
  mode: RecordingMode     // live / studio
  rawPath: String         // /recordings/{id}/raw.wav
  processedPath: String?  // /recordings/{id}/processed.wav
  effectChain: List<AudioEffect>
}
```

Recordings stored in `getApplicationDocumentsDirectory()/recordings/`. Each recording gets its own directory with raw.wav + optional processed.wav.

---

## 6. User Stories

| ID | Story | Screen | Priority |
|----|-------|--------|----------|
| U1 | As a user, I can record audio with one tap | Home | P0 |
| U2 | As a user, I can hear effects in real-time while recording (Live mode) | Home | P0 |
| U3 | As a user, I can record clean audio and add effects later (Studio mode) | Home + Editor | P0 |
| U4 | As a user, I can pick from preset effect chains (T-Pain, Robot, etc.) | Home | P0 |
| U5 | As a user, I can adjust individual effect parameters with sliders | Editor | P0 |
| U6 | As a user, I can reorder effects in the chain by dragging | Editor | P1 |
| U7 | As a user, I can A/B compare with and without effects | Editor | P1 |
| U8 | As a user, I can export my recording as WAV or MP3 | Export | P0 |
| U9 | As a user, I can share my recording to WhatsApp, TikTok, IG | Export | P0 |
| U10 | As a user, I can browse and re-edit past recordings | Library | P1 |
| U11 | As a user, I can delete recordings I don't want | Library | P1 |
| U12 | As a user, I see a spectrum visualizer while recording | Home | P2 |
| U13 | As a user, the app asks for mic permission with a clear explanation | Permission Gate | P0 |

---

## 7. Technical Requirements

| Requirement | Spec |
|-------------|------|
| Min Android | API 26 (Android 8.0) |
| Min iOS | 15.0 |
| Permissions | RECORD_AUDIO (Android), NSMicrophoneUsageDescription (iOS) |
| Storage | App documents directory; no external storage required |
| Network | None required (fully offline) |
| Sharing | share_plus for platform share sheet |
| Audio format | WAV (native); MP3 (planned) |
| Sample rate | 48kHz mono |
| Max recording | Limited by device storage (~10MB/min WAV) |

---

## 8. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Audio latency too high on cheap Android devices | Use cpal BufferSize::Default; document minimum specs |
| Users confused by Live vs Studio | Default to Studio; add onboarding tooltip |
| Large WAV files fill storage | Add file size estimate before recording; auto-cleanup old exports |
| Share to TikTok/IG requires SDK | MVP uses generic share sheet; add native SDKs in v0.2 |
| Permission denied permanently | Show dialog directing to system settings |

---

## 9. Success Metrics

| Metric | Target |
|--------|--------|
| Time to first recording | <10 seconds |
| Preset selection → audible effect | <200ms |
| Export time (1 min recording) | <6 seconds |
| App cold start | <2 seconds |
| Crash rate | <0.1% sessions |

---

## 10. Roadmap

### v0.1 — MVP (Current)
- Dual mode recording (Live/Studio)
- 11 effects with full editor
- 11 built-in presets
- WAV export + platform share sheet
- Recording library with swipe-to-delete

### v0.2 — Polish
- MP3 export
- Custom preset save/load
- Waveform scrubbing in editor
- Recording rename
- Storage usage indicator

### v0.3 — Growth
- In-app audio trimming
- Waveform thumbnail generation for library
- Preset sharing (import/export JSON)
- Background audio recording
- Widget for quick recording from home screen

### v1.0 — Launch
- App Store / Play Store submission
- Onboarding flow
- Analytics + crash reporting
- Localization (EN, ID, ES, PT)
- Rate/review prompt
