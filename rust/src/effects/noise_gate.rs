use crate::graph::*;
use crate::impl_effect_meta;

pub struct NoiseGate {
    pub enabled: AtomicEnabled,
    pub threshold_db: AtomicF32,
    pub attack_ms: AtomicF32,
    pub release_ms: AtomicF32,
    envelope: f32,
    gate_gain: f32,
}

impl NoiseGate {
    pub fn new(threshold_db: f32, attack_ms: f32, release_ms: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            threshold_db: AtomicF32::new(threshold_db),
            attack_ms: AtomicF32::new(attack_ms),
            release_ms: AtomicF32::new(release_ms),
            envelope: 0.0,
            gate_gain: 0.0,
        }
    }
}

impl AudioEffect for NoiseGate {
    impl_effect_meta!(self, EffectType::NoiseGate, {
        "threshold_db" => threshold_db,
        "attack_ms" => attack_ms,
        "release_ms" => release_ms,
    });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let threshold = 10.0_f32.powf(self.threshold_db.get() / 20.0);
        let sr = sample_rate as f32;
        let att = (-1.0 / (self.attack_ms.get() * 0.001 * sr)).exp();
        let rel = (-1.0 / (self.release_ms.get() * 0.001 * sr)).exp();
        for s in buffer.iter_mut() {
            let abs = s.abs();
            let coeff = if abs > self.envelope { att } else { rel };
            self.envelope = coeff * self.envelope + (1.0 - coeff) * abs;
            let target = if self.envelope > threshold { 1.0 } else { 0.0 };
            let gc = if target > self.gate_gain { att } else { rel };
            self.gate_gain = gc * self.gate_gain + (1.0 - gc) * target;
            *s *= self.gate_gain;
        }
    }

    fn reset(&mut self) { self.envelope = 0.0; self.gate_gain = 0.0; }
}
