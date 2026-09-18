use crate::graph::*;
use crate::impl_effect_meta;
use std::f32::consts::PI;

pub struct DeEsser {
    pub enabled: AtomicEnabled,
    pub frequency_hz: SmoothedParam,
    pub threshold_db: AtomicF32,
    pub amount: SmoothedParam,
    pub release_ms: AtomicF32,
    low_state: f32,
    envelope: f32,
}

impl DeEsser {
    pub fn new(frequency_hz: f32, threshold_db: f32, amount: f32, release_ms: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            frequency_hz: SmoothedParam::new(frequency_hz),
            threshold_db: AtomicF32::new(threshold_db),
            amount: SmoothedParam::new(amount),
            release_ms: AtomicF32::new(release_ms),
            low_state: 0.0,
            envelope: 0.0,
        }
    }
}

impl AudioEffect for DeEsser {
    impl_effect_meta!(self, EffectType::DeEsser, {
        "frequency_hz" => frequency_hz,
        "threshold_db" => threshold_db,
        "amount" => amount,
        "release_ms" => release_ms,
    });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() {
            return;
        }
        let sr = sample_rate as f32;
        let param_coeff = smooth_coeff(sr, 15.0);
        let attack_coeff = (-1.0 / (0.001 * sr).max(1.0)).exp();
        let release_ms = self.release_ms.get().clamp(1.0, 1000.0);
        let release_coeff = (-1.0 / (release_ms * 0.001 * sr).max(1.0)).exp();
        let threshold = self.threshold_db.get().clamp(-96.0, 0.0);
        // Keep the crossover below Nyquist at low stream rates (e.g. 8 kHz).
        let max_frequency = (sr * 0.45).max(2000.0);

        for sample in buffer.iter_mut() {
            let frequency = self.frequency_hz.tick(param_coeff).clamp(2000.0, 12000.0).min(max_frequency);
            let amount = self.amount.tick(param_coeff).clamp(0.0, 1.0);
            let crossover = 1.0 - (-2.0 * PI * frequency / sr).exp();
            self.low_state += crossover * (*sample - self.low_state);
            let high = *sample - self.low_state;

            let level = high.abs();
            let env_coeff = if level > self.envelope {
                attack_coeff
            } else {
                release_coeff
            };
            self.envelope = env_coeff * self.envelope + (1.0 - env_coeff) * level;
            let envelope_db = 20.0 * self.envelope.max(1e-10).log10();
            let reduction_db = -((envelope_db - threshold).max(0.0) * amount).min(20.0 * amount);
            let high_gain = 10.0_f32.powf(reduction_db / 20.0);
            *sample = self.low_state + high * high_gain;
        }
    }

    fn reset(&mut self) {
        self.low_state = 0.0;
        self.envelope = 0.0;
        self.frequency_hz.snap();
        self.amount.snap();
    }
}
