use crate::graph::*;
use crate::impl_effect_meta;

pub struct Delay {
    pub enabled: AtomicEnabled,
    pub time_ms: AtomicF32,
    pub feedback: AtomicF32,
    pub mix: AtomicF32,
    buffer: Vec<f32>, write_pos: usize,
}

impl Delay {
    pub fn new(time_ms: f32, feedback: f32, mix: f32) -> Self {
        Self { enabled: AtomicEnabled::new(true),
            time_ms: AtomicF32::new(time_ms),
            feedback: AtomicF32::new(feedback.min(0.95)),
            mix: AtomicF32::new(mix),
            buffer: vec![0.0; 96000], write_pos: 0 }
    }
}

impl AudioEffect for Delay {
    impl_effect_meta!(self, EffectType::Delay, { "time_ms" => time_ms, "feedback" => feedback, "mix" => mix });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let ds = ((self.time_ms.get() * 0.001 * sample_rate as f32) as usize)
            .min(self.buffer.len() - 1).max(1);
        let fb = self.feedback.get().min(0.95);
        let mix = self.mix.get();
        for s in buffer.iter_mut() {
            let dry = *s;
            let rp = (self.write_pos + self.buffer.len() - ds) % self.buffer.len();
            let delayed = self.buffer[rp];
            self.buffer[self.write_pos] = dry + delayed * fb;
            self.write_pos = (self.write_pos + 1) % self.buffer.len();
            *s = dry * (1.0 - mix) + delayed * mix;
        }
    }
    fn reset(&mut self) { self.buffer.fill(0.0); }
}
