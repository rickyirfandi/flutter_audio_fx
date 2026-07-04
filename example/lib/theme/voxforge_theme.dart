import 'package:flutter/material.dart';

/// VoxForge design system.
/// Dark studio aesthetic — deep blacks, cyan primary, warm orange accent.
abstract final class VoxForgeTheme {
  // ── Colors ──
  static const bg = Color(0xFF0A0A0F);
  static const bgCard = Color(0xFF13131A);
  static const bgElevated = Color(0xFF1A1A24);
  static const bgSurface = Color(0xFF22222E);

  static const primary = Color(0xFF00E5CC); // cyan
  static const accent = Color(0xFFFF6B35); // warm orange
  static const danger = Color(0xFFFF3B5C); // record red
  static const success = Color(0xFF4ADE80);
  static const warning = Color(0xFFFFBB33);

  static const textPrimary = Color(0xFFF0F0F5);
  static const textSecondary = Color(0xFF8888A0);
  static const textMuted = Color(0xFF555570);

  static const border = Color(0xFF2A2A38);
  static const borderActive = Color(0xFF00E5CC);

  // ── Gradients ──
  static const recordGradient = LinearGradient(
    colors: [Color(0xFFFF3B5C), Color(0xFFFF6B35)],
  );

  static const primaryGradient = LinearGradient(
    colors: [Color(0xFF00E5CC), Color(0xFF00B4D8)],
  );

  // ── Typography ──
  static const fontFamily = 'monospace';

  // ── Shared decorations ──
  static BoxDecoration get cardDecoration => BoxDecoration(
        color: bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: border, width: 0.5),
      );

  static BoxDecoration get elevatedCardDecoration => BoxDecoration(
        color: bgElevated,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: border, width: 0.5),
      );

  // ── ThemeData ──
  static ThemeData get dark => ThemeData(
        brightness: Brightness.dark,
        scaffoldBackgroundColor: bg,
        fontFamily: fontFamily,
        colorScheme: const ColorScheme.dark(
          primary: primary,
          secondary: accent,
          surface: bgCard,
          error: danger,
          onPrimary: bg,
          onSecondary: bg,
          onSurface: textPrimary,
        ),
        appBarTheme: const AppBarTheme(
          backgroundColor: bg,
          elevation: 0,
          centerTitle: true,
          titleTextStyle: TextStyle(
            fontFamily: fontFamily,
            fontSize: 16,
            fontWeight: FontWeight.w700,
            letterSpacing: 1.5,
            color: textPrimary,
          ),
          iconTheme: IconThemeData(color: textSecondary),
        ),
        sliderTheme: SliderThemeData(
          activeTrackColor: primary,
          inactiveTrackColor: bgSurface,
          thumbColor: primary,
          overlayColor: primary.withValues(alpha: 0.1),
          trackHeight: 3,
          thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 7),
        ),
        switchTheme: SwitchThemeData(
          thumbColor: WidgetStateProperty.resolveWith((states) {
            if (states.contains(WidgetState.selected)) return primary;
            return textMuted;
          }),
          trackColor: WidgetStateProperty.resolveWith((states) {
            if (states.contains(WidgetState.selected)) {
              return primary.withValues(alpha: 0.3);
            }
            return bgSurface;
          }),
        ),
        dividerColor: border,
        bottomSheetTheme: const BottomSheetThemeData(
          backgroundColor: bgCard,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
          ),
        ),
      );
}

/// Shared spacing constants
abstract final class Spacing {
  static const xs = 4.0;
  static const sm = 8.0;
  static const md = 16.0;
  static const lg = 24.0;
  static const xl = 32.0;
  static const xxl = 48.0;
}
