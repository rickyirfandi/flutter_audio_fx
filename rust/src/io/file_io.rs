use hound;
use std::path::Path;

/// Audio file metadata
#[derive(Debug, Clone)]
pub struct AudioFileMeta {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub duration_ms: u64,
    pub total_samples: usize,
}

/// Read a WAV file and return samples as f32 (normalized to [-1.0, 1.0])
pub fn read_wav(path: &str) -> Result<(Vec<f32>, AudioFileMeta), String> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV: {}", e))?;

    let spec = reader.spec();
    let duration_samples = reader.duration() as usize;
    let duration_ms = (duration_samples as u64 * 1000) / spec.sample_rate as u64;

    let meta = AudioFileMeta {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        bits_per_sample: spec.bits_per_sample,
        duration_ms,
        total_samples: duration_samples,
    };

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            match spec.bits_per_sample {
                16 => reader.into_samples::<i16>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f32 / 32768.0)
                    .collect(),
                24 => reader.into_samples::<i32>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f32 / 8388608.0)
                    .collect(),
                32 => reader.into_samples::<i32>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f32 / 2147483648.0)
                    .collect(),
                _ => return Err(format!("Unsupported bit depth: {}", spec.bits_per_sample)),
            }
        }
        hound::SampleFormat::Float => {
            reader.into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect()
        }
    };

    Ok((samples, meta))
}

/// Read a WAV file downmixed to mono (channels averaged per frame).
///
/// The returned `AudioFileMeta` describes the *source* file, so `channels`
/// reflects the original channel count even though the samples are mono.
pub fn read_wav_mono(path: &str) -> Result<(Vec<f32>, AudioFileMeta), String> {
    let (samples, meta) = read_wav(path)?;
    if meta.channels <= 1 {
        return Ok((samples, meta));
    }
    let ch = meta.channels as usize;
    let frames = samples.len() / ch;
    let mut mono = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut acc = 0.0f32;
        for c in 0..ch { acc += samples[f * ch + c]; }
        mono.push(acc / ch as f32);
    }
    Ok((mono, meta))
}

/// Write samples to a WAV file (16-bit PCM)
pub fn write_wav(path: &str, samples: &[f32], sample_rate: u32, channels: u16) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create WAV: {}", e))?;

    for &sample in samples {
        let val = (sample * 32767.0).max(-32768.0).min(32767.0) as i16;
        writer.write_sample(val)
            .map_err(|e| format!("WAV write error: {}", e))?;
    }

    writer.finalize()
        .map_err(|e| format!("WAV finalize error: {}", e))?;

    Ok(())
}

/// Write samples to a 32-bit float WAV file (higher quality)
pub fn write_wav_float(path: &str, samples: &[f32], sample_rate: u32, channels: u16) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| format!("Failed to create WAV: {}", e))?;

    for &sample in samples {
        writer.write_sample(sample)
            .map_err(|e| format!("WAV write error: {}", e))?;
    }

    writer.finalize()
        .map_err(|e| format!("WAV finalize error: {}", e))?;

    Ok(())
}

/// Get audio file info without reading all samples
pub fn get_file_info(path: &str) -> Result<AudioFileMeta, String> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open: {}", e))?;

    let spec = reader.spec();
    let duration_samples = reader.duration() as usize;

    Ok(AudioFileMeta {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        bits_per_sample: spec.bits_per_sample,
        duration_ms: (duration_samples as u64 * 1000) / spec.sample_rate as u64,
        total_samples: duration_samples,
    })
}

/// Check if a file path is a supported audio format
pub fn is_supported_format(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    matches!(ext.as_str(), "wav" | "wave")
    // MP3 decoding will be added later
}
