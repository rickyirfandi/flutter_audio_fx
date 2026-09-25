use crate::graph::*;
use crate::impl_effect_meta;

/// Buffer sized for the maximum supported stream rate so the advertised
/// maximum delay time holds at any rate (2 s even at 192 kHz). Allocated once
/// in `new()`; never touched structurally on the audio thread.
const MAX_SR: usize = 192_000;
const MAX_DELAY_S: usize = 2;

pub struct Delay {
    pub enabled: AtomicEnabled,
    pub time_ms: AtomicF32,
    pub feedback: SmoothedParam,
    pub mix: SmoothedParam,
    buffer: Vec<f32>,
    write_pos: usize,
    /// Audio-thread state: smoothed delay length in samples. Slewing the read
    /// position (with fractional interpolation) turns a `time_ms` change into
    /// a brief tape-style pitch bend instead of a hard click.
    cur_delay: f32,
}

impl Delay {
    pub fn new(time_ms: f32, feedback: f32, mix: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            time_ms: AtomicF32::new(time_ms),
            feedback: SmoothedParam::new(feedback.min(0.95)),
            mix: SmoothedParam::new(mix),
            buffer: vec![0.0; MAX_SR * MAX_DELAY_S],
            write_pos: 0,
            cur_delay: -1.0, // sentinel: snap to target on first process
        }
    }
}

impl AudioEffect for Delay {
    impl_effect_meta!(self, EffectType::Delay, { "time_ms" => time_ms, "feedback" => feedback, "mix" => mix });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let sr = sample_rate as f32;
        let len = self.buffer.len();
        let target = (self.time_ms.get() * 0.001 * sr).clamp(1.0, (len - 2) as f32);
        if self.cur_delay < 0.0 { self.cur_delay = target; }
        // Glide the read pointer at most half a sample per sample: bounded
        // tape-style pitch bend (~0.5x–1.5x) no matter how far the time jumps.
        const MAX_GLIDE: f32 = 0.5;
        let gain_coeff = smooth_coeff(sr, 15.0);
        for s in buffer.iter_mut() {
            self.cur_delay += (target - self.cur_delay).clamp(-MAX_GLIDE, MAX_GLIDE);
            let fb = self.feedback.tick(gain_coeff).clamp(0.0, 0.95);
            let mix = self.mix.tick(gain_coeff).clamp(0.0, 1.0);
            let dry = *s;
            // Fractional read at write_pos - cur_delay (linear interpolation).
            // Split into integer + fraction so precision doesn't depend on the
            // absolute buffer index (f32 ulp near 384k is ~1/32 sample).
            let d_int = self.cur_delay as usize;
            let d_frac = self.cur_delay - d_int as f32;
            let i0 = (self.write_pos + len - d_int) % len;
            let i1 = (i0 + len - 1) % len; // one sample older
            let delayed = self.buffer[i0] * (1.0 - d_frac) + self.buffer[i1] * d_frac;
            self.buffer[self.write_pos] = dry + delayed * fb;
            self.write_pos = (self.write_pos + 1) % len;
            *s = dry * (1.0 - mix) + delayed * mix;
        }
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
        self.cur_delay = -1.0;
        self.feedback.snap();
        self.mix.snap();
    }
}
