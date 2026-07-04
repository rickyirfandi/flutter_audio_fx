use crate::graph::*;
use crate::impl_effect_meta;

#[derive(Debug, Clone, Copy)]
pub enum DistortionType { SoftClip, HardClip, Tanh, Bitcrush }

pub struct Distortion {
    pub enabled: AtomicEnabled,
    pub drive: AtomicF32,
    pub tone: AtomicF32,
    pub mix: AtomicF32,
    pub dist_type: DistortionType,
    filter_state: f32,
}

impl Distortion {
    pub fn new(drive: f32, tone: f32, mix: f32, dt: DistortionType) -> Self {
        Self { enabled: AtomicEnabled::new(true),
            drive: AtomicF32::new(drive), tone: AtomicF32::new(tone),
            mix: AtomicF32::new(mix), dist_type: dt, filter_state: 0.0 }
    }
    #[inline]
    fn shape(&self, x: f32, drive: f32) -> f32 {
        let g = x * (1.0 + drive * 20.0);
        match self.dist_type {
            DistortionType::SoftClip => {
                if g.abs() < 1.0 { g - g*g*g/3.0 } else { g.signum() * 2.0/3.0 }
            }
            DistortionType::HardClip => g.clamp(-1.0, 1.0),
            DistortionType::Tanh => g.tanh(),
            DistortionType::Bitcrush => {
                let bits = (16.0 - drive * 12.0).max(2.0);
                let levels = 2.0_f32.powf(bits);
                let clamped = g.clamp(-1.0, 1.0);
                (clamped * levels).round() / levels
            }
        }
    }
}

impl AudioEffect for Distortion {
    impl_effect_meta!(self, EffectType::Distortion, {
        "drive" => drive, "tone" => tone, "mix" => mix,
    });

    fn process(&mut self, buffer: &mut [f32], _sample_rate: u32) {
        if !self.enabled.get() { return; }
        let drive = self.drive.get();
        let tone_c = 0.01 + self.tone.get() * 0.99;
        let mix = self.mix.get();
        for s in buffer.iter_mut() {
            let dry = *s;
            let mut wet = self.shape(*s, drive);
            self.filter_state += tone_c * (wet - self.filter_state);
            wet = self.filter_state;
            wet *= 1.0 / (1.0 + drive * 2.0);
            *s = dry * (1.0 - mix) + wet * mix;
        }
    }
    fn reset(&mut self) { self.filter_state = 0.0; }
}
