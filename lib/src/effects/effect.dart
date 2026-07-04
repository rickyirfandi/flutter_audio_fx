/// Base class for all audio effects.
///
/// Each effect has:
/// - A type identifier
/// - Enable/disable toggle
/// - Parameters as a Map (for serialization)
/// - Real-time parameter update support
abstract class AudioEffect {
  /// Effect type identifier (matches Rust side)
  String get type;

  /// Display name for UI
  String get displayName;

  /// Whether this effect is active
  bool enabled;

  AudioEffect({this.enabled = true});

  /// Get all parameters as a serializable map
  Map<String, double> toParams();

  /// Update a single parameter by name.
  /// This will be sent to the Rust engine via atomic update (no audio glitch).
  void updateParam(String name, double value);

  /// Serialize to JSON-compatible map
  Map<String, dynamic> toJson() => {
        'type': type,
        'enabled': enabled,
        'params': toParams(),
      };

  /// Create effect from JSON
  static AudioEffect? fromJson(Map<String, dynamic> json) {
    // Import specific effect files and create instances
    // This is handled by EffectFactory
    return null;
  }
}
