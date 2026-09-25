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

/// RAII guard that enables flush-to-zero / denormals-are-zero for the current
/// thread and restores the previous FP control state on drop.
///
/// Recursive DSP (reverb combs, delay feedback, IIR filters, envelopes) decays
/// into subnormal floats when the input goes silent. On x86 every subnormal
/// operation takes a ~100-cycle microcode assist, which turns a silent tail
/// into a CPU spike that can blow the audio deadline. Install this at the top
/// of every audio callback / offline render. A no-op on other architectures.
pub struct DenormalGuard {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    prev: u32,
    #[cfg(target_arch = "aarch64")]
    prev: u64,
}

impl DenormalGuard {
    #[inline]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            let mut prev: u32 = 0;
            // SAFETY: stmxcsr/ldmxcsr only touch this thread's SSE control
            // register; FTZ (bit 15) + DAZ (bit 6) change rounding of
            // subnormals only.
            unsafe {
                std::arch::asm!("stmxcsr [{}]", in(reg) &mut prev, options(nostack));
                let next = prev | 0x8040;
                std::arch::asm!("ldmxcsr [{}]", in(reg) &next, options(nostack, readonly));
            }
            Self { prev }
        }
        #[cfg(target_arch = "aarch64")]
        {
            let prev: u64;
            // SAFETY: FPCR is per-thread; bit 24 (FZ) flushes subnormals.
            unsafe {
                std::arch::asm!("mrs {}, fpcr", out(reg) prev, options(nomem, nostack));
                std::arch::asm!("msr fpcr, {}", in(reg) prev | (1 << 24), options(nomem, nostack));
            }
            Self { prev }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
        { Self {} }
    }
}

impl Drop for DenormalGuard {
    #[inline]
    fn drop(&mut self) {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        // SAFETY: restores the value read in `new`.
        unsafe { std::arch::asm!("ldmxcsr [{}]", in(reg) &self.prev, options(nostack, readonly)); }
        #[cfg(target_arch = "aarch64")]
        // SAFETY: restores the value read in `new`.
        unsafe { std::arch::asm!("msr fpcr, {}", in(reg) self.prev, options(nomem, nostack)); }
    }
}

/// Float → 16-bit PCM with TPDF dither (±1 LSB triangular noise).
///
/// Plain truncation to 16 bits turns quantization error into distortion that
/// is correlated with the signal (audible as grit on fades and reverb tails).
/// TPDF dither decorrelates it into a constant, benign noise floor at about
/// -93 dBFS. Below -120 dBFS (1/30 LSB, no information at 16 bits) the
/// dither is blanked so digital silence stays exactly zero. xorshift RNG: no
/// allocation, no syscalls, RT-safe.
pub struct Dither16 {
    rng: u32,
}

impl Dither16 {
    pub fn new(seed: u32) -> Self { Self { rng: seed | 1 } }

    #[inline]
    fn next_uniform(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32
    }

    #[inline]
    pub fn quantize(&mut self, s: f32) -> i16 {
        if s.abs() < 1e-6 { return 0; }
        let tpdf = self.next_uniform() - self.next_uniform(); // triangular, ±1 LSB
        (s * 32767.0 + tpdf).round().clamp(-32768.0, 32767.0) as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64"))]
    fn denormal_guard_flushes_subnormals_and_restores() {
        let tiny = black_box(f32::MIN_POSITIVE);
        assert!(black_box(tiny * 0.5) != 0.0, "subnormals should exist without the guard");
        {
            let _g = DenormalGuard::new();
            assert_eq!(black_box(tiny * 0.5), 0.0, "guard must flush subnormal results");
        }
        assert!(black_box(tiny * 0.5) != 0.0, "guard must restore FP state on drop");
    }
}
