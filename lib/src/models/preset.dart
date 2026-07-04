import '../effects/effect.dart';

/// A named preset containing a full effect chain configuration.
class Preset {
  final String id;
  final String name;
  final String description;
  final String emoji;
  final bool isBuiltIn;
  final List<Map<String, dynamic>> effectConfigs;

  const Preset({
    required this.id,
    required this.name,
    required this.description,
    required this.emoji,
    this.isBuiltIn = false,
    required this.effectConfigs,
  });

  /// Create a preset from a current effect chain
  factory Preset.fromChain({
    required String id,
    required String name,
    required String description,
    required String emoji,
    required List<AudioEffect> chain,
  }) {
    return Preset(
      id: id,
      name: name,
      description: description,
      emoji: emoji,
      effectConfigs: chain.map((e) => e.toJson()).toList(),
    );
  }

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'description': description,
        'emoji': emoji,
        'is_built_in': isBuiltIn,
        'effects': effectConfigs,
      };

  factory Preset.fromJson(Map<String, dynamic> json) => Preset(
        id: json['id'] as String,
        name: json['name'] as String,
        description: json['description'] as String? ?? '',
        emoji: json['emoji'] as String? ?? '🎵',
        isBuiltIn: json['is_built_in'] as bool? ?? false,
        effectConfigs: (json['effects'] as List)
            .map((e) => Map<String, dynamic>.from(e))
            .toList(),
      );
}
