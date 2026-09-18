use crate::graph::*;
use crate::impl_effect_meta;
use std::f32::consts::PI;

pub struct Exciter {
    pub enabled: AtomicEnabled,
    pub frequency_hz: SmoothedParam,
    pub drive: SmoothedParam,
    pub mix: SmoothedParam,
    low_state: f32,
}

impl Exciter {
    pub fn new(frequency_hz: f32, drive: f32, mix: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            frequency_hz: SmoothedParam::new(frequency_hz),
            drive: SmoothedParam::new(drive),
            mix: SmoothedParam::new(mix),
            low_state: 0.0,
        }
    }
}

impl AudioEffect for Exciter {
    impl_effect_meta!(self, EffectType::Exciter, {
        "frequency_hz" => frequency_hz,
        "drive" => drive,
        "mix" => mix,
    });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() {
            return;
        }
        let sr = sample_rate as f32;
        let coeff = smooth_coeff(sr, 15.0);
        // Keep the crossover below Nyquist at low stream rates (e.g. 8 kHz).
        let max_frequency = (sr * 0.45).max(1000.0);

        for sample in buffer.iter_mut() {
            let frequency = self.frequency_hz.tick(coeff).clamp(1000.0, 12000.0).min(max_frequency);
            let drive = self.drive.tick(coeff).clamp(0.0, 1.0);
            let mix = self.mix.tick(coeff).clamp(0.0, 1.0);
            let highpass = 1.0 - (-2.0 * PI * frequency / sr).exp();
            self.low_state += highpass * (*sample - self.low_state);
            let high = *sample - self.low_state;

            // tanh(g·h)/g has unity small-signal slope, so the residual
            // h - tanh(g·h)/g ≈ g²h³/3 is purely nonlinear (odd harmonics) —
            // no linear treble boost on quiet material. |residual| ≤ |h|, so
            // even flat-out (drive = mix = 1) the high band gains ≤ +6 dB.
            let gain = 1.0 + drive * 8.0;
            let residual = high - (high * gain).tanh() / gain;
            *sample += residual * mix;
        }
    }

    fn reset(&mut self) {
        self.low_state = 0.0;
        self.frequency_hz.snap();
        self.drive.snap();
        self.mix.snap();
    }
}
