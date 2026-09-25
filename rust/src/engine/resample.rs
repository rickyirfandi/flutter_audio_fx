//! Band-limited variable-ratio resampler for the live and preview paths.
//!
//! Kaiser-windowed sinc, 48 taps (~65 dB stopband), 256 interpolated phases. The ratio can be
//! changed per call (RT-safe, no allocation), which is how the live path
//! absorbs clock drift between independent input/output devices and bridges
//! devices running at different nominal rates (e.g. a 16 kHz Bluetooth mic
//! feeding a 48 kHz output).
//!
//! The kernel is designed once at construction, off the audio thread. Its
//! cutoff is 0.9 × the lower of the two Nyquist rates, so it is flat to about
//! 20 kHz at 48 kHz, and it never aliases when downsampling or images when
//! upsampling. Because the same filter applies at every fractional position,
//! a slowly drifting ratio does not modulate the high end the way
//! linear or cubic interpolation would.

use std::f64::consts::PI;

const TAPS: usize = 48;
const HALF: usize = TAPS / 2;
const PHASES: usize = 256;
const BETA: f64 = 6.5;

/// Group delay in input samples.
pub const LATENCY: usize = HALF;

fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > 1e-12 * sum {
        term *= (x / (2.0 * k)) * (x / (2.0 * k));
        sum += term;
        k += 1.0;
    }
    sum
}

pub struct Resampler {
    /// `(PHASES + 1) * TAPS` coefficients; row `p` is the kernel for
    /// fractional position `p / PHASES`. Each row is normalized to unity DC.
    table: Vec<f32>,
    /// Input history, oldest first. The read point lies between
    /// `hist[HALF - 1]` and `hist[HALF]`.
    hist: [f32; TAPS],
    frac: f64,
    ratio: f64,
}

impl Resampler {
    pub fn new(in_rate: u32, out_rate: u32) -> Self {
        let base = in_rate as f64 / out_rate as f64;
        let cutoff = 0.9 * (1.0 / base).min(1.0); // fraction of input Nyquist
        let i0_beta = bessel_i0(BETA);
        let mut table = vec![0.0f32; (PHASES + 1) * TAPS];
        for p in 0..=PHASES {
            let t = p as f64 / PHASES as f64;
            let mut row = [0.0f64; TAPS];
            for (j, tap) in row.iter_mut().enumerate() {
                let x = j as f64 - (HALF - 1) as f64 - t;
                let sinc = if x.abs() < 1e-12 { cutoff } else { (PI * cutoff * x).sin() / (PI * x) };
                let r = x / HALF as f64;
                let w = if r.abs() >= 1.0 { 0.0 } else { bessel_i0(BETA * (1.0 - r * r).sqrt()) / i0_beta };
                *tap = sinc * w;
            }
            let sum: f64 = row.iter().sum();
            for (dst, v) in table[p * TAPS..(p + 1) * TAPS].iter_mut().zip(row) {
                *dst = (v / sum) as f32;
            }
        }
        Self { table, hist: [0.0; TAPS], frac: 0.0, ratio: base }
    }

    /// Input samples consumed per output sample.
    #[inline] pub fn set_ratio(&mut self, ratio: f64) { self.ratio = ratio; }

    /// Produce one output sample, calling `pull` for each input sample needed.
    #[inline]
    pub fn next(&mut self, mut pull: impl FnMut() -> f32) -> f32 {
        let pos = self.frac * PHASES as f64;
        let p = (pos as usize).min(PHASES - 1);
        let w = (pos - p as f64) as f32;
        let a = &self.table[p * TAPS..(p + 1) * TAPS];
        let b = &self.table[(p + 1) * TAPS..(p + 2) * TAPS];
        let mut y = 0.0f32;
        for j in 0..TAPS {
            y += self.hist[j] * (a[j] + (b[j] - a[j]) * w);
        }
        self.frac += self.ratio;
        while self.frac >= 1.0 {
            self.frac -= 1.0;
            self.hist.copy_within(1.., 0);
            self.hist[TAPS - 1] = pull();
        }
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone_gain(in_rate: u32, out_rate: u32, hz: f32) -> f32 {
        let mut rs = Resampler::new(in_rate, out_rate);
        let mut n = 0usize;
        let mut src = || {
            let s = (2.0 * std::f32::consts::PI * hz * n as f32 / in_rate as f32).sin();
            n += 1;
            s
        };
        let out: Vec<f32> = (0..out_rate as usize / 2).map(|_| rs.next(&mut src)).collect();
        let tail = &out[out.len() / 2..];
        let rms = (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt();
        rms * std::f32::consts::SQRT_2
    }

    #[test]
    fn passband_is_flat_at_unity_ratio() {
        for hz in [100.0, 1000.0, 8000.0, 16000.0] {
            let g = tone_gain(48000, 48000, hz);
            assert!((g - 1.0).abs() < 0.03, "{hz} Hz gain {g}");
        }
    }

    #[test]
    fn upsampling_bluetooth_mic_is_clean() {
        let g = tone_gain(16000, 48000, 1000.0);
        assert!((g - 1.0).abs() < 0.03, "gain {g}");
    }

    #[test]
    fn downsampling_rejects_content_above_new_nyquist() {
        // 30 kHz at 96 kHz in → 48 kHz out would alias to 18 kHz.
        let g = tone_gain(96000, 48000, 30000.0);
        assert!(g < 0.01, "alias leak {g}");
    }
}
