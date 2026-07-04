use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use std::sync::Arc;

/// Spectrum analysis data sent to Flutter for visualizers
#[derive(Debug, Clone)]
pub struct SpectrumData {
    /// Magnitude of each frequency bin (0.0 to 1.0)
    pub magnitudes: Vec<f32>,
    /// Dominant frequency in Hz
    pub dominant_freq: f32,
    /// RMS volume level (0.0 to 1.0)
    pub rms: f32,
    /// Number of frequency bins (= fft_size / 2)
    pub bin_count: usize,
    /// Frequency resolution (Hz per bin)
    pub freq_resolution: f32,
}

/// Real-time spectrum analyzer using a cached real FFT plan.
///
/// All buffers are allocated up front in `new()`. `feed`/`get_spectrum` only
/// touch pre-allocated memory.
pub struct SpectrumAnalyzer {
    fft_size: usize,
    buffer: Vec<f32>,
    write_pos: usize,
    window: Vec<f32>,
    fft: Arc<dyn RealToComplex<f32>>,
    fft_in: Vec<f32>,
    fft_out: Vec<Complex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    /// Hop between FFT computations (~60fps)
    hop_count: usize,
    sample_count: usize,
    smoothed: Vec<f32>,
    smooth_factor: f32,
    sample_rate: u32,
    /// Reusable normalized magnitude buffer (returned in SpectrumData)
    norm_mag: Vec<f32>,
}

impl SpectrumAnalyzer {
    pub fn new(fft_size: usize) -> Self {
        debug_assert!(fft_size.is_power_of_two());
        let half = fft_size / 2 + 1;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_scratch_len()];

        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / fft_size as f32).cos()))
            .collect();

        Self {
            fft_size,
            buffer: vec![0.0; fft_size],
            write_pos: 0,
            window,
            fft,
            fft_in: vec![0.0; fft_size],
            fft_out: vec![Complex::new(0.0, 0.0); half],
            fft_scratch,
            hop_count: 48000 / 60,
            sample_count: 0,
            smoothed: vec![0.0; fft_size / 2],
            smooth_factor: 0.7,
            sample_rate: 48000,
            norm_mag: vec![0.0; fft_size / 2],
        }
    }

    /// RT-safe: only writes into pre-allocated buffer.
    pub fn feed(&mut self, samples: &[f32]) {
        for &s in samples {
            self.buffer[self.write_pos] = s;
            self.write_pos = (self.write_pos + 1) % self.fft_size;
            self.sample_count += 1;
        }
    }

    /// RT-safe: writes into pre-allocated `norm_mag`. Caller may clone the
    /// returned slice off-thread if it wants an owned `Vec`.
    /// Returns None if not yet time for a new frame.
    pub fn get_spectrum(&mut self) -> Option<SpectrumData> {
        if self.sample_count < self.hop_count {
            return None;
        }
        self.sample_count = 0;

        let half = self.fft_size / 2;

        // Window into pre-allocated input buffer (handle circular buffer).
        for i in 0..self.fft_size {
            let idx = (self.write_pos + i) % self.fft_size;
            self.fft_in[i] = self.buffer[idx] * self.window[i];
        }

        // realfft normalizes the inverse, so forward returns un-normalized.
        // Scaling applied below.
        let _ = self.fft.process_with_scratch(
            &mut self.fft_in, &mut self.fft_out, &mut self.fft_scratch,
        );

        let norm = 2.0 / self.fft_size as f32;
        let mut max_mag = 0.0f32;
        let mut max_bin = 0usize;
        let mut rms_sum = 0.0f32;

        for i in 0..half {
            let re = self.fft_out[i].re;
            let im = self.fft_out[i].im;
            let mag = (re * re + im * im).sqrt() * norm;

            self.smoothed[i] = self.smoothed[i] * self.smooth_factor
                + mag * (1.0 - self.smooth_factor);

            if self.smoothed[i] > max_mag {
                max_mag = self.smoothed[i];
                max_bin = i;
            }
            rms_sum += mag * mag;
        }

        let rms = (rms_sum / half as f32).sqrt();
        let freq_resolution = self.sample_rate as f32 / self.fft_size as f32;
        let dominant_freq = max_bin as f32 * freq_resolution;

        // Normalize into pre-allocated buffer; clone for caller.
        if max_mag > 0.0 {
            let inv = 1.0 / max_mag;
            for i in 0..half { self.norm_mag[i] = self.smoothed[i] * inv; }
        } else {
            for i in 0..half { self.norm_mag[i] = 0.0; }
        }

        Some(SpectrumData {
            magnitudes: self.norm_mag.clone(),
            dominant_freq,
            rms: rms.min(1.0),
            bin_count: half,
            freq_resolution,
        })
    }

    pub fn set_sample_rate(&mut self, sr: u32) {
        self.sample_rate = sr;
        self.hop_count = sr as usize / 60;
    }
}

/// Pitch detection data (for auto-tune visualization)
#[derive(Debug, Clone)]
pub struct PitchData {
    pub pitch_hz: f32,
    pub confidence: f32,
    pub note_name: String,
    pub cents_off: f32,
}

impl PitchData {
    pub fn from_frequency(freq_hz: f32, confidence: f32) -> Self {
        if freq_hz <= 0.0 {
            return Self {
                pitch_hz: 0.0, confidence: 0.0,
                note_name: "-".to_string(), cents_off: 0.0,
            };
        }
        let midi = 69.0 + 12.0 * (freq_hz / 440.0).log2();
        let midi_round = midi.round() as i32;
        let cents = (midi - midi_round as f32) * 100.0;
        let names = ["C","C#","D","D#","E","F","F#","G","G#","A","A#","B"];
        let note_idx = ((midi_round % 12) + 12) % 12;
        let octave = (midi_round / 12) - 1;
        Self {
            pitch_hz: freq_hz, confidence,
            note_name: format!("{}{}", names[note_idx as usize], octave),
            cents_off: cents,
        }
    }
}
