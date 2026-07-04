use crate::graph::*;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistortionType { SoftClip, HardClip, Tanh, Bitcrush }

impl DistortionType {
    pub fn from_index(i: u32) -> Self {
        match i {
            1 => Self::HardClip,
            2 => Self::Tanh,
            3 => Self::Bitcrush,
            _ => Self::SoftClip,
        }
    }
}

pub struct Distortion {
    pub enabled: AtomicEnabled,
    pub drive: AtomicF32,
    pub tone: AtomicF32,
    pub mix: AtomicF32,
    /// u32-encoded DistortionType so the control thread can retarget it live.
    dist_type: AtomicU32,
    filter_state: f32,
}

impl Distortion {
    pub fn new(drive: f32, tone: f32, mix: f32, dt: DistortionType) -> Self {
        Self { enabled: AtomicEnabled::new(true),
            drive: AtomicF32::new(drive), tone: AtomicF32::new(tone),
            mix: AtomicF32::new(mix), dist_type: AtomicU32::new(dt as u32),
            filter_state: 0.0 }
    }
    pub fn dist_type(&self) -> DistortionType {
        DistortionType::from_index(self.dist_type.load(Ordering::Relaxed))
    }
    #[inline]
    fn shape(x: f32, drive: f32, dt: DistortionType) -> f32 {
        let g = x * (1.0 + drive * 20.0);
        match dt {
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
    fn effect_type(&self) -> EffectType { EffectType::Distortion }
    fn set_enabled(&self, e: bool) { self.enabled.set(e); }
    fn is_enabled(&self) -> bool { self.enabled.get() }
    fn set_param(&self, name: &str, value: f32) -> bool {
        match name {
            "drive" => { self.drive.set(value); true }
            "tone" => { self.tone.set(value); true }
            "mix" => { self.mix.set(value); true }
            "dist_type" => {
                self.dist_type.store((value.max(0.0) as u32).min(3), Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    fn process(&mut self, buffer: &mut [f32], _sample_rate: u32) {
        if !self.enabled.get() { return; }
        let drive = self.drive.get();
        let tone_c = 0.01 + self.tone.get() * 0.99;
        let mix = self.mix.get();
        let dt = self.dist_type();
        for s in buffer.iter_mut() {
            let dry = *s;
            let mut wet = Self::shape(*s, drive, dt);
            self.filter_state += tone_c * (wet - self.filter_state);
            wet = self.filter_state;
            wet *= 1.0 / (1.0 + drive * 2.0);
            *s = dry * (1.0 - mix) + wet * mix;
        }
    }
    fn reset(&mut self) { self.filter_state = 0.0; }
}
