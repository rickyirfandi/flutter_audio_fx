/// Audio engine configuration
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub sample_rate: u32,
    pub buffer_size: usize,
    pub channels: u16,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            buffer_size: 256,
            channels: 1, // mono for voice processing
        }
    }
}

/// Audio format for file export
#[derive(Debug, Clone)]
pub enum AudioFormat {
    Wav,
    Mp3 { bitrate: u32 },
}

/// Input source
#[derive(Debug, Clone)]
pub enum AudioInput {
    Microphone,
    File { path: String },
}

/// Output destination
#[derive(Debug, Clone)]
pub enum AudioOutput {
    Speaker,
    File { path: String, format: AudioFormat },
    PcmStream, // Raw samples back to Dart
}

/// Effect chain configuration (serializable to/from JSON)
#[derive(Debug, Clone)]
pub struct EffectConfig {
    pub effect_type: String,
    pub enabled: bool,
    pub params: std::collections::HashMap<String, f64>,
}

/// Recording project metadata
#[derive(Debug, Clone)]
pub struct ProjectMeta {
    pub id: String,
    pub created_at: String,
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub mode: String,
    pub title: String,
}
