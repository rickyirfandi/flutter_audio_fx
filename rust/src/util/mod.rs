/// Utility functions for audio processing

/// Convert dB to linear amplitude
#[inline(always)]
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Convert linear amplitude to dB
#[inline(always)]
pub fn linear_to_db(linear: f32) -> f32 {
    if linear.abs() < 1e-10 {
        -96.0
    } else {
        20.0 * linear.abs().log10()
    }
}

/// Clamp a value between min and max
#[inline(always)]
pub fn clamp(val: f32, min: f32, max: f32) -> f32 {
    val.max(min).min(max)
}

/// Convert MIDI note number to frequency
#[inline]
pub fn midi_to_freq(midi: f32) -> f32 {
    440.0 * 2.0_f32.powf((midi - 69.0) / 12.0)
}

/// Convert frequency to MIDI note number
#[inline]
pub fn freq_to_midi(freq: f32) -> f32 {
    if freq <= 0.0 {
        return 0.0;
    }
    69.0 + 12.0 * (freq / 440.0).log2()
}

/// Convert semitones to frequency ratio
#[inline]
pub fn semitones_to_ratio(semitones: f32) -> f32 {
    2.0_f32.powf(semitones / 12.0)
}

/// Note names for display
pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"
];

/// Get note name and octave from MIDI note number
pub fn midi_to_note_name(midi: i32) -> String {
    let note = ((midi % 12) + 12) % 12;
    let octave = (midi / 12) - 1;
    format!("{}{}", NOTE_NAMES[note as usize], octave)
}

/// Calculate RMS of a buffer
pub fn rms(buffer: &[f32]) -> f32 {
    if buffer.is_empty() {
        return 0.0;
    }
    let sum: f32 = buffer.iter().map(|s| s * s).sum();
    (sum / buffer.len() as f32).sqrt()
}

/// Calculate peak level of a buffer
pub fn peak(buffer: &[f32]) -> f32 {
    buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max)
}
