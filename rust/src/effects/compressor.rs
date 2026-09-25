use crate::graph::*;
use crate::impl_effect_meta;

pub struct Compressor {
    pub enabled: AtomicEnabled,
    pub threshold_db: AtomicF32,
    pub ratio: AtomicF32,
    pub attack_ms: AtomicF32,
    pub release_ms: AtomicF32,
    pub makeup_gain_db: SmoothedParam,
    pub knee_db: AtomicF32,
    pub sidechain_hpf_hz: AtomicF32,
    /// Linear-domain peak envelope of the detector signal.
    envelope: f32,
    sidechain_low_state: f32,
}

impl Compressor {
    pub fn new(threshold_db: f32, ratio: f32, attack_ms: f32, release_ms: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            threshold_db: AtomicF32::new(threshold_db),
            ratio: AtomicF32::new(ratio),
            attack_ms: AtomicF32::new(attack_ms),
            release_ms: AtomicF32::new(release_ms),
            makeup_gain_db: SmoothedParam::new(0.0),
            knee_db: AtomicF32::new(3.0),
            sidechain_hpf_hz: AtomicF32::new(0.0),
            envelope: 0.0,
            sidechain_low_state: 0.0,
        }
    }
    #[inline]
    fn to_db(x: f32) -> f32 {
        if x.abs() < 1e-10 {
            -96.0
        } else {
            20.0 * x.abs().log10()
        }
    }
    #[inline]
    fn from_db(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }
}

impl AudioEffect for Compressor {
    impl_effect_meta!(self, EffectType::Compressor, {
        "threshold_db" => threshold_db, "ratio" => ratio,
        "attack_ms" => attack_ms, "release_ms" => release_ms,
        "makeup_gain_db" => makeup_gain_db, "knee_db" => knee_db,
        "sidechain_hpf_hz" => sidechain_hpf_hz,
    });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() {
            return;
        }
        let sr = sample_rate as f32;
        let att = (-1.0 / (self.attack_ms.get().clamp(0.05, 500.0) * 0.001 * sr).max(1.0)).exp();
        let rel = (-1.0 / (self.release_ms.get().clamp(1.0, 5000.0) * 0.001 * sr).max(1.0)).exp();
        let threshold = self.threshold_db.get().clamp(-80.0, 0.0);
        // ratio < 1 would expand and ratio = 0 divides by zero.
        let ratio = self.ratio.get().clamp(1.0, 100.0);
        let knee = self.knee_db.get().clamp(0.0, 24.0);
        let sidechain_hpf_hz = self.sidechain_hpf_hz.get().clamp(0.0, 500.0);
        let sidechain_coeff = 1.0 - (-2.0 * std::f32::consts::PI * sidechain_hpf_hz / sr).exp();
        let makeup_coeff = smooth_coeff(sr, 15.0);

        for s in buffer.iter_mut() {
            let makeup = self.makeup_gain_db.tick(makeup_coeff);
            let detector = if sidechain_hpf_hz > 0.0 {
                self.sidechain_low_state += sidechain_coeff * (*s - self.sidechain_low_state);
                *s - self.sidechain_low_state
            } else {
                self.sidechain_low_state = 0.0;
                *s
            };
            // Branching peak detector in the linear domain, then to dB. A
            // log-domain follower fed per-sample |x| dives toward -96 dB at
            // every zero crossing, which ripples the gain at twice the input
            // frequency and distorts low voices.
            let level = detector.abs();
            let c = if level > self.envelope { att } else { rel };
            self.envelope = c * self.envelope + (1.0 - c) * level;
            let envelope_db = Self::to_db(self.envelope);

            // Gain computation with soft knee
            let gain = if knee <= 0.01 {
                if envelope_db <= threshold {
                    0.0
                } else {
                    let over = envelope_db - threshold;
                    (over / ratio) - over
                }
            } else {
                let hk = knee / 2.0;
                if envelope_db < threshold - hk {
                    0.0
                } else if envelope_db > threshold + hk {
                    let over = envelope_db - threshold;
                    (over / ratio) - over
                } else {
                    let x = envelope_db - threshold + hk;
                    (1.0 / ratio - 1.0) * x * x / (2.0 * knee)
                }
            };
            *s *= Self::from_db(gain + makeup.clamp(-24.0, 36.0));
        }
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.sidechain_low_state = 0.0;
        self.makeup_gain_db.snap();
    }
}
