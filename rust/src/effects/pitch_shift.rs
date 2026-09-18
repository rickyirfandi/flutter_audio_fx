use crate::graph::*;
use realfft::{RealFftPlanner, RealToComplex, ComplexToReal};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use std::sync::Arc;

/// Phase-vocoder pitch shifter.
///
/// All working buffers (FFT input/output, magnitude/frequency arrays, FIFOs)
/// are pre-allocated in `new()`. `process` is RT-safe.
pub struct PitchShift {
    pub enabled: AtomicEnabled,
    pub semitones: AtomicF32,
    pub cents: AtomicF32,
    pub formant_preserve: AtomicEnabled,

    fft_size: usize,
    hop_size: usize,
    window: Vec<f32>,

    fft_fwd: Arc<dyn RealToComplex<f32>>,
    fft_inv: Arc<dyn ComplexToReal<f32>>,
    fft_scratch: Vec<Complex<f32>>,

    last_phase: Vec<f32>,
    sum_phase: Vec<f32>,

    fft_in: Vec<f32>,
    fft_out: Vec<Complex<f32>>,
    ifft_in: Vec<Complex<f32>>,
    ifft_out: Vec<f32>,

    mags: Vec<f32>,
    freqs: Vec<f32>,
    syn_mag: Vec<f32>,
    syn_freq: Vec<f32>,
    /// Spectral envelope of the analysis frame (formant preservation).
    env: Vec<f32>,
    env_tmp: Vec<f32>,

    in_fifo: Vec<f32>,
    out_fifo: Vec<f32>,
    fifo_pos: usize,
    initialized: bool,
}

impl PitchShift {
    pub fn new(semitones: f32) -> Self {
        let fft_size = 2048;
        let hop_size = fft_size / 4;
        let half = fft_size / 2 + 1;

        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / fft_size as f32).cos()))
            .collect();

        let mut planner = RealFftPlanner::<f32>::new();
        let fft_fwd = planner.plan_fft_forward(fft_size);
        let fft_inv = planner.plan_fft_inverse(fft_size);
        let scratch_len = fft_fwd.get_scratch_len().max(fft_inv.get_scratch_len());
        let fft_scratch = vec![Complex::new(0.0, 0.0); scratch_len];

        Self {
            enabled: AtomicEnabled::new(true),
            semitones: AtomicF32::new(semitones),
            cents: AtomicF32::new(0.0),
            formant_preserve: AtomicEnabled::new(true),
            fft_size, hop_size, window,
            fft_fwd, fft_inv, fft_scratch,
            last_phase: vec![0.0; half],
            sum_phase: vec![0.0; half],
            fft_in: vec![0.0; fft_size],
            fft_out: vec![Complex::new(0.0, 0.0); half],
            ifft_in: vec![Complex::new(0.0, 0.0); half],
            ifft_out: vec![0.0; fft_size],
            mags: vec![0.0; half],
            freqs: vec![0.0; half],
            syn_mag: vec![0.0; half],
            syn_freq: vec![0.0; half],
            env: vec![0.0; half],
            env_tmp: vec![0.0; half],
            in_fifo: vec![0.0; fft_size],
            out_fifo: vec![0.0; fft_size],
            fifo_pos: fft_size,
            initialized: false,
        }
    }

    fn pitch_ratio(&self) -> f32 {
        2.0_f32.powf((self.semitones.get() + self.cents.get() / 100.0) / 12.0)
    }
}

/// Edge-clamped moving average over `2*w+1` bins (running sum, O(n), no
/// allocation). Two passes approximate a Gaussian well enough for a spectral
/// envelope.
fn box_smooth(src: &[f32], dst: &mut [f32], w: usize) {
    let n = src.len();
    let mut sum: f32 = src[..(w + 1).min(n)].iter().sum();
    let mut count = (w + 1).min(n);
    for i in 0..n {
        dst[i] = sum / count as f32;
        if i + w + 1 < n { sum += src[i + w + 1]; count += 1; }
        if i >= w { sum -= src[i - w]; count -= 1; }
    }
}

impl AudioEffect for PitchShift {
    fn effect_type(&self) -> EffectType { EffectType::PitchShift }
    fn set_enabled(&self, e: bool) { self.enabled.set(e); }
    fn is_enabled(&self) -> bool { self.enabled.get() }
    fn set_param(&self, name: &str, value: f32) -> bool {
        match name {
            "semitones" => { self.semitones.set(value); true }
            "cents" => { self.cents.set(value); true }
            "formant_preserve" => { self.formant_preserve.set(value > 0.5); true }
            _ => false,
        }
    }

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let ratio = self.pitch_ratio();
        // NOTE: no bypass at ratio == 1. Bypassing would drop the fft_size
        // FIFO latency, so toggling in/out of unity (AutoTune does this
        // constantly) would time-jump the signal and click. The vocoder is
        // near-transparent at unity; latency stays constant instead.

        let fft_size = self.fft_size;
        let hop = self.hop_size;
        let half = fft_size / 2 + 1;
        let sr = sample_rate as f32;
        let freq_per_bin = sr / fft_size as f32;
        let expected = 2.0 * PI * hop as f32 / fft_size as f32;
        // Overlap-add gain: Hann² at N/hop overlap sums to (N/hop) * 3/8.
        let norm = 1.0 / (fft_size as f32 / hop as f32 * 0.375);

        if !self.initialized {
            self.fifo_pos = fft_size - hop;
            for v in &mut self.in_fifo  { *v = 0.0; }
            for v in &mut self.out_fifo { *v = 0.0; }
            self.initialized = true;
        }

        for sample in buffer.iter_mut() {
            // New input lands in the newest `hop` region of the analysis FIFO;
            // output is read from the *oldest* `hop` of the OLA accumulator
            // (the only region every overlapping frame has contributed to).
            self.in_fifo[self.fifo_pos] = *sample;
            *sample = self.out_fifo[self.fifo_pos - (fft_size - hop)];
            self.fifo_pos += 1;

            if self.fifo_pos >= fft_size {
                self.fifo_pos = fft_size - hop;

                // Window into pre-allocated input.
                for i in 0..fft_size {
                    self.fft_in[i] = self.in_fifo[i] * self.window[i];
                }
                // Forward FFT (real-to-complex).
                let _ = self.fft_fwd.process_with_scratch(
                    &mut self.fft_in, &mut self.fft_out, &mut self.fft_scratch,
                );

                // Analysis.
                for k in 0..half {
                    let re = self.fft_out[k].re;
                    let im = self.fft_out[k].im;
                    self.mags[k] = (re * re + im * im).sqrt();
                    let phase = im.atan2(re);
                    let mut dp = phase - self.last_phase[k];
                    self.last_phase[k] = phase;
                    dp -= k as f32 * expected;
                    dp = dp - (dp / (2.0 * PI)).round() * 2.0 * PI;
                    self.freqs[k] = k as f32 * freq_per_bin + dp * freq_per_bin / expected;
                }

                // Formant preservation: estimate the spectral envelope (the
                // formant shape), divide it out so only the excitation
                // (harmonics) is shifted, and re-apply the *original*
                // envelope afterwards. Envelope = double box-smoothing of the
                // magnitude spectrum over a ~500 Hz-wide window.
                let formant = self.formant_preserve.get();
                if formant {
                    let w = ((500.0 / freq_per_bin) as usize).clamp(4, half / 8);
                    box_smooth(&self.mags, &mut self.env_tmp, w);
                    box_smooth(&self.env_tmp, &mut self.env, w);
                    // Floor relative to the frame peak so near-silent bins
                    // don't get boosted by orders of magnitude.
                    let peak = self.env.iter().fold(0.0f32, |a, &b| a.max(b));
                    let floor = (peak * 1e-3).max(1e-9);
                    for k in 0..half {
                        let e = self.env[k].max(floor);
                        self.env[k] = e;
                        self.mags[k] /= e; // whiten
                    }
                }

                // Pitch shift via bin remap.
                for v in &mut self.syn_mag  { *v = 0.0; }
                for v in &mut self.syn_freq { *v = 0.0; }
                for k in 0..half {
                    let new_bin = (k as f32 * ratio) as usize;
                    if new_bin < half {
                        self.syn_mag[new_bin]  += self.mags[k];
                        self.syn_freq[new_bin]  = self.freqs[k] * ratio;
                    }
                }
                if formant {
                    for k in 0..half { self.syn_mag[k] *= self.env[k]; }
                }

                // Phase reconstruction.
                for k in 0..half {
                    let dp = self.syn_freq[k] / freq_per_bin - k as f32;
                    let advance = dp * expected + k as f32 * expected;
                    self.sum_phase[k] += advance;
                    self.ifft_in[k] = Complex::new(
                        self.syn_mag[k] * self.sum_phase[k].cos(),
                        self.syn_mag[k] * self.sum_phase[k].sin(),
                    );
                }
                // Inverse FFT (complex-to-real, normalized by realfft).
                let _ = self.fft_inv.process_with_scratch(
                    &mut self.ifft_in, &mut self.ifft_out, &mut self.fft_scratch,
                );

                // Shift the FIFO left by `hop` and overlap-add into output.
                for i in 0..fft_size - hop {
                    self.in_fifo[i]  = self.in_fifo[i + hop];
                    self.out_fifo[i] = self.out_fifo[i + hop];
                }
                for i in fft_size - hop..fft_size {
                    self.out_fifo[i] = 0.0;
                }
                let scale = norm / fft_size as f32;
                for i in 0..fft_size {
                    self.out_fifo[i] += self.ifft_out[i] * self.window[i] * scale;
                }
            }
        }
    }

    fn latency_samples(&self) -> usize { self.fft_size - self.hop_size }

    fn flush(&mut self) { self.reset(); }

    fn reset(&mut self) {
        for v in &mut self.in_fifo    { *v = 0.0; }
        for v in &mut self.out_fifo   { *v = 0.0; }
        for v in &mut self.last_phase { *v = 0.0; }
        for v in &mut self.sum_phase  { *v = 0.0; }
        self.fifo_pos = self.fft_size - self.hop_size;
        self.initialized = false;
    }
}
