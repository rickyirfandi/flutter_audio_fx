use crate::graph::*;
use crate::impl_effect_meta;
use std::f32::consts::PI;

/// Chorus timing in seconds (15 ms base delay, ±7 ms sweep at full depth) so
/// the voicing is identical at any stream rate. The buffer is sized for the
/// maximum supported rate (192 kHz → 15+7 ms ≈ 4224 samples).
const BASE_DELAY_S: f32 = 0.015;
const SWEEP_S: f32 = 0.007;
const BUF_LEN: usize = 8192;

pub struct Chorus {
    pub enabled: AtomicEnabled,
    pub rate_hz: AtomicF32,
    pub depth: SmoothedParam,
    pub mix: SmoothedParam,
    buffer: Vec<f32>, write_pos: usize, lfo_phase: f32,
}

impl Chorus {
    pub fn new(rate: f32, depth: f32, mix: f32) -> Self {
        Self { enabled: AtomicEnabled::new(true),
            rate_hz: AtomicF32::new(rate), depth: SmoothedParam::new(depth),
            mix: SmoothedParam::new(mix), buffer: vec![0.0; BUF_LEN],
            write_pos: 0, lfo_phase: 0.0 }
    }
    #[inline] fn read_interp(&self, delay: f32) -> f32 {
        let len = self.buffer.len() as f32;
        let pos = ((self.write_pos as f32 - delay) % len + len) % len;
        let i0 = pos as usize; let i1 = (i0 + 1) % self.buffer.len();
        let frac = pos - i0 as f32;
        self.buffer[i0] * (1.0 - frac) + self.buffer[i1] * frac
    }
}

impl AudioEffect for Chorus {
    impl_effect_meta!(self, EffectType::Chorus, { "rate_hz" => rate_hz, "depth" => depth, "mix" => mix });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let sr = sample_rate as f32;
        let rate = self.rate_hz.get().clamp(0.0, 10.0);
        let coeff = smooth_coeff(sr, 15.0);
        let inc = rate / sr;
        let sweep = SWEEP_S * sr;
        let base = (BASE_DELAY_S * sr).min((BUF_LEN - 2) as f32 - sweep);
        for s in buffer.iter_mut() {
            let depth = self.depth.tick(coeff).clamp(0.0, 1.0);
            let mix = self.mix.tick(coeff).clamp(0.0, 1.0);
            let dry = *s;
            self.buffer[self.write_pos] = dry;
            self.write_pos = (self.write_pos + 1) % self.buffer.len();
            let lfo = (2.0 * PI * self.lfo_phase).sin();
            self.lfo_phase += inc;
            if self.lfo_phase >= 1.0 { self.lfo_phase -= 1.0; }
            let wet = self.read_interp(base + lfo * sweep * depth);
            *s = dry * (1.0 - mix) + wet * mix;
        }
    }
    fn reset(&mut self) {
        self.buffer.fill(0.0); self.lfo_phase = 0.0;
        self.depth.snap(); self.mix.snap();
    }
}
