use crate::graph::*;
use crate::impl_effect_meta;

pub struct Compressor {
    pub enabled: AtomicEnabled,
    pub threshold_db: AtomicF32,
    pub ratio: AtomicF32,
    pub attack_ms: AtomicF32,
    pub release_ms: AtomicF32,
    pub makeup_gain_db: AtomicF32,
    pub knee_db: AtomicF32,
    envelope_db: f32,
}

impl Compressor {
    pub fn new(threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            threshold_db: AtomicF32::new(threshold_db),
            ratio: AtomicF32::new(ratio),
            attack_ms: AtomicF32::new(attack_ms),
            release_ms: AtomicF32::new(release_ms),
            makeup_gain_db: AtomicF32::new(0.0),
            knee_db: AtomicF32::new(3.0),
            envelope_db: -96.0,
        }
    }
    #[inline] fn to_db(x: f32) -> f32 { if x.abs() < 1e-10 { -96.0 } else { 20.0 * x.abs().log10() } }
    #[inline] fn from_db(db: f32) -> f32 { 10.0_f32.powf(db / 20.0) }
}

impl AudioEffect for Compressor {
    impl_effect_meta!(self, EffectType::Compressor, {
        "threshold_db" => threshold_db, "ratio" => ratio,
        "attack_ms" => attack_ms, "release_ms" => release_ms,
        "makeup_gain_db" => makeup_gain_db, "knee_db" => knee_db,
    });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let sr = sample_rate as f32;
        let att = (-1.0 / (self.attack_ms.get() * 0.001 * sr)).exp();
        let rel = (-1.0 / (self.release_ms.get() * 0.001 * sr)).exp();
        let threshold = self.threshold_db.get();
        let ratio = self.ratio.get();
        let knee = self.knee_db.get();
        let makeup = self.makeup_gain_db.get();

        for s in buffer.iter_mut() {
            let db = Self::to_db(*s);
            let c = if db > self.envelope_db { att } else { rel };
            self.envelope_db = c * self.envelope_db + (1.0 - c) * db;

            // Gain computation with soft knee
            let gain = if knee <= 0.01 {
                if self.envelope_db <= threshold { 0.0 }
                else { let over = self.envelope_db - threshold; (over / ratio) - over }
            } else {
                let hk = knee / 2.0;
                if self.envelope_db < threshold - hk { 0.0 }
                else if self.envelope_db > threshold + hk {
                    let over = self.envelope_db - threshold; (over / ratio) - over
                } else {
                    let x = self.envelope_db - threshold + hk;
                    (1.0 / ratio - 1.0) * x * x / (2.0 * knee)
                }
            };
            *s *= Self::from_db(gain + makeup);
        }
    }

    fn reset(&mut self) { self.envelope_db = -96.0; }
}
