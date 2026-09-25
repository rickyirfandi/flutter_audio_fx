use crate::graph::*;
use std::sync::atomic::{AtomicU32, Ordering};

const OVERSAMPLE: usize = 4;
/// Anti-imaging / anti-aliasing FIR length. `4L + 1` taps puts the combined
/// up + down group delay at exactly `L` base-rate samples (L = 16 here), so the
/// dry path can be delayed by an integer to stay phase-aligned with the wet.
const FIR_TAPS: usize = 65;
/// Taps per polyphase branch of the interpolator (ceil(FIR_TAPS / 4)).
const PHASE_TAPS: usize = FIR_TAPS.div_ceil(OVERSAMPLE);
/// Total wet-path delay (up + down), in base-rate samples.
const LATENCY: usize = (FIR_TAPS - 1) / OVERSAMPLE;
/// Kaiser-windowed sinc: cutoff as a fraction of the 4x rate (~22 kHz at a
/// 48 kHz base) and window beta. Per pass: -0.4 dB @ 18 kHz, -55 dB @ 28 kHz,
/// <= -70 dB above 30 kHz (48 kHz base).
const FIR_CUTOFF: f64 = 0.115;
const FIR_BETA: f64 = 6.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistortionType {
    SoftClip,
    HardClip,
    Tanh,
    Bitcrush,
}

impl DistortionType {
    pub fn from_index(i: u32) -> Self {
        match i {
            1 => Self::HardClip,
            2 => Self::Tanh,
            3 => Self::Bitcrush,
            _ => Self::SoftClip,
        }
    }
}

pub struct Distortion {
    pub enabled: AtomicEnabled,
    pub drive: SmoothedParam,
    pub tone: AtomicF32,
    pub mix: SmoothedParam,
    /// u32-encoded DistortionType so the control thread can retarget it live.
    dist_type: AtomicU32,
    filter_state: f32,
    fir_coeffs: [f32; FIR_TAPS],
    /// Interpolator split into its 4 polyphase branches (gain x4 folded in),
    /// so the zero-stuffed samples are never multiplied.
    poly: [[f32; PHASE_TAPS]; OVERSAMPLE],
    /// Histories are mirrored rings (each value stored at `pos` and
    /// `pos + LEN`) so `hist[pos..pos + LEN]` is always newest-first and
    /// contiguous: no modulo in the inner loops.
    up_hist: [f32; 2 * PHASE_TAPS],
    down_hist: [f32; 2 * FIR_TAPS],
    up_pos: usize,
    down_pos: usize,
    /// Delays the dry signal by `LATENCY` to match the wet path; without it
    /// the dry/wet blend comb-filters (first notch ~3 kHz at 48 kHz).
    dry_line: [f32; LATENCY],
    dry_pos: usize,
}

impl Distortion {
    pub fn new(drive: f32, tone: f32, mix: f32, dt: DistortionType) -> Self {
        let coeffs = Self::design_fir();
        let mut poly = [[0.0; PHASE_TAPS]; OVERSAMPLE];
        for (k, &c) in coeffs.iter().enumerate() {
            poly[k % OVERSAMPLE][k / OVERSAMPLE] = c * OVERSAMPLE as f32;
        }
        Self {
            enabled: AtomicEnabled::new(true),
            drive: SmoothedParam::new(drive),
            tone: AtomicF32::new(tone),
            mix: SmoothedParam::new(mix),
            dist_type: AtomicU32::new(dt as u32),
            filter_state: 0.0,
            fir_coeffs: coeffs,
            poly,
            up_hist: [0.0; 2 * PHASE_TAPS],
            down_hist: [0.0; 2 * FIR_TAPS],
            up_pos: 0,
            down_pos: 0,
            dry_line: [0.0; LATENCY],
            dry_pos: 0,
        }
    }
    pub fn dist_type(&self) -> DistortionType {
        DistortionType::from_index(self.dist_type.load(Ordering::Relaxed))
    }
    #[inline]
    fn shape(x: f32, gain: f32, levels: f32, dt: DistortionType) -> f32 {
        let g = x * gain;
        match dt {
            DistortionType::SoftClip => {
                if g.abs() < 1.0 {
                    g - g * g * g / 3.0
                } else {
                    g.signum() * 2.0 / 3.0
                }
            }
            DistortionType::HardClip => g.clamp(-1.0, 1.0),
            DistortionType::Tanh => g.tanh(),
            DistortionType::Bitcrush => {
                let clamped = g.clamp(-1.0, 1.0);
                (clamped * levels).round() / levels
            }
        }
    }

    fn design_fir() -> [f32; FIR_TAPS] {
        // Modified Bessel function of the first kind, order 0 (power series).
        fn bessel_i0(x: f64) -> f64 {
            let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
            while term > 1e-12 * sum {
                term *= (x / (2.0 * k)) * (x / (2.0 * k));
                sum += term;
                k += 1.0;
            }
            sum
        }
        let center = (FIR_TAPS - 1) as f64 * 0.5;
        let mut taps = [0.0f64; FIR_TAPS];
        for (index, tap) in taps.iter_mut().enumerate() {
            let offset = index as f64 - center;
            let sinc = if offset == 0.0 {
                2.0 * FIR_CUTOFF
            } else {
                (2.0 * std::f64::consts::PI * FIR_CUTOFF * offset).sin()
                    / (std::f64::consts::PI * offset)
            };
            let r = offset / center;
            let window =
                bessel_i0(FIR_BETA * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(FIR_BETA);
            *tap = sinc * window;
        }
        let sum: f64 = taps.iter().sum();
        let mut coeffs = [0.0f32; FIR_TAPS];
        for (c, t) in coeffs.iter_mut().zip(taps) {
            *c = (t / sum) as f32;
        }
        coeffs
    }

    /// Push `x` into a mirrored ring of length `len`; afterwards
    /// `hist[*pos..*pos + len]` is newest-first.
    #[inline]
    fn push(hist: &mut [f32], pos: &mut usize, len: usize, x: f32) {
        *pos = if *pos == 0 { len - 1 } else { *pos - 1 };
        hist[*pos] = x;
        hist[*pos + len] = x;
    }

    #[inline]
    fn dot(coeffs: &[f32], hist: &[f32]) -> f32 {
        coeffs.iter().zip(hist).map(|(c, h)| c * h).sum()
    }
}

impl AudioEffect for Distortion {
    fn effect_type(&self) -> EffectType {
        EffectType::Distortion
    }
    fn set_enabled(&self, e: bool) {
        self.enabled.set(e);
    }
    fn is_enabled(&self) -> bool {
        self.enabled.get()
    }
    fn set_param(&self, name: &str, value: f32) -> bool {
        match name {
            "drive" => {
                self.drive.set(value);
                true
            }
            "tone" => {
                self.tone.set(value);
                true
            }
            "mix" => {
                self.mix.set(value);
                true
            }
            "dist_type" => {
                self.dist_type
                    .store((value.max(0.0) as u32).min(3), Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() {
            return;
        }
        let coeff = smooth_coeff(sample_rate as f32, 15.0);
        let tone_c = 0.01 + self.tone.get().clamp(0.0, 1.0) * 0.99;
        let dt = self.dist_type();
        for s in buffer.iter_mut() {
            let drive = self.drive.tick(coeff).clamp(0.0, 1.0);
            let mix = self.mix.tick(coeff).clamp(0.0, 1.0);
            let gain = 1.0 + drive * 20.0;
            // Only Bitcrush uses it; computed once per base sample, not per phase.
            let levels = if dt == DistortionType::Bitcrush {
                2.0_f32.powf((16.0 - drive * 12.0).max(2.0))
            } else {
                1.0
            };
            let input = *s;

            // 4x polyphase interpolate -> shape -> anti-alias filter. Only the
            // phase-0 output survives decimation, so it is the only one
            // computed; it lands exactly LATENCY base samples behind the input.
            Self::push(&mut self.up_hist, &mut self.up_pos, PHASE_TAPS, input);
            let mut wet = 0.0;
            for phase in 0..OVERSAMPLE {
                let up = self.up_pos;
                let upsampled =
                    Self::dot(&self.poly[phase], &self.up_hist[up..up + PHASE_TAPS]);
                let shaped = Self::shape(upsampled, gain, levels, dt);
                Self::push(&mut self.down_hist, &mut self.down_pos, FIR_TAPS, shaped);
                if phase == 0 {
                    let down = self.down_pos;
                    wet = Self::dot(&self.fir_coeffs, &self.down_hist[down..down + FIR_TAPS]);
                }
            }
            self.filter_state += tone_c * (wet - self.filter_state);
            wet = self.filter_state / (1.0 + drive * 2.0);

            let dry = self.dry_line[self.dry_pos];
            self.dry_line[self.dry_pos] = input;
            self.dry_pos = (self.dry_pos + 1) % LATENCY;
            *s = dry * (1.0 - mix) + wet * mix;
        }
    }

    fn latency_samples(&self) -> usize {
        LATENCY
    }

    fn flush(&mut self) {
        self.reset();
    }

    fn reset(&mut self) {
        self.filter_state = 0.0;
        self.up_hist.fill(0.0);
        self.down_hist.fill(0.0);
        self.up_pos = 0;
        self.down_pos = 0;
        self.dry_line.fill(0.0);
        self.dry_pos = 0;
        self.drive.snap();
        self.mix.snap();
    }
}
