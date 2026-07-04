use crate::graph::*;
use crate::impl_effect_meta;

pub struct Limiter {
    pub enabled: AtomicEnabled,
    pub ceiling_db: AtomicF32,
    pub release_ms: AtomicF32,
    gain: f32,
}

impl Limiter {
    pub fn new(ceiling_db: f32, release_ms: f32) -> Self {
        Self { enabled: AtomicEnabled::new(true),
            ceiling_db: AtomicF32::new(ceiling_db),
            release_ms: AtomicF32::new(release_ms), gain: 1.0 }
    }
}

impl AudioEffect for Limiter {
    impl_effect_meta!(self, EffectType::Limiter, { "ceiling_db" => ceiling_db, "release_ms" => release_ms });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let ceil = 10.0_f32.powf(self.ceiling_db.get() / 20.0);
        let rel = (-1.0 / (self.release_ms.get() * 0.001 * sample_rate as f32)).exp();
        for s in buffer.iter_mut() {
            let abs = (*s * self.gain).abs();
            if abs > ceil { self.gain = ceil / s.abs().max(1e-10); }
            else { self.gain = rel * self.gain + (1.0 - rel); self.gain = self.gain.min(1.0); }
            *s *= self.gain;
        }
    }
    fn reset(&mut self) { self.gain = 1.0; }
}
