use crate::graph::*;
use crate::impl_effect_meta;

/// Noise gate with hold, hysteresis and range.
///
/// * **Hysteresis**: opens at `threshold_db` and closes only once the level
///   falls `hysteresis_db` below it, so a signal hovering at the threshold
///   (breath, room tone, a decaying word) does not chatter the gate.
/// * **Hold**: stays open for `hold_ms` after the level drops, so the gate
///   does not clamp down between syllables and chop word endings.
/// * **Range**: attenuation when closed. At or below -96 dB (under 16-bit
///   resolution) the gate mutes completely; higher values act as a gentle
///   expander that keeps some room tone.
pub struct NoiseGate {
    pub enabled: AtomicEnabled,
    pub threshold_db: AtomicF32,
    pub attack_ms: AtomicF32,
    pub release_ms: AtomicF32,
    pub hold_ms: AtomicF32,
    pub hysteresis_db: AtomicF32,
    pub range_db: AtomicF32,
    envelope: f32,
    gate_gain: f32,
    open: bool,
    hold_left: u32,
}

impl NoiseGate {
    pub fn new(threshold_db: f32, attack_ms: f32, release_ms: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            threshold_db: AtomicF32::new(threshold_db),
            attack_ms: AtomicF32::new(attack_ms),
            release_ms: AtomicF32::new(release_ms),
            hold_ms: AtomicF32::new(50.0),
            hysteresis_db: AtomicF32::new(6.0),
            range_db: AtomicF32::new(-100.0),
            envelope: 0.0,
            gate_gain: 0.0,
            open: false,
            hold_left: 0,
        }
    }
}

impl AudioEffect for NoiseGate {
    impl_effect_meta!(self, EffectType::NoiseGate, {
        "threshold_db" => threshold_db,
        "attack_ms" => attack_ms,
        "release_ms" => release_ms,
        "hold_ms" => hold_ms,
        "hysteresis_db" => hysteresis_db,
        "range_db" => range_db,
    });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let sr = sample_rate as f32;
        let threshold_db = self.threshold_db.get().clamp(-96.0, 0.0);
        let open_at = 10.0_f32.powf(threshold_db / 20.0);
        let hyst = self.hysteresis_db.get().clamp(0.0, 24.0);
        let close_at = 10.0_f32.powf((threshold_db - hyst) / 20.0);
        let range_db = self.range_db.get().clamp(-120.0, 0.0);
        let floor = if range_db <= -96.0 { 0.0 } else { 10.0_f32.powf(range_db / 20.0) };
        let hold = (self.hold_ms.get().clamp(0.0, 2000.0) * 0.001 * sr) as u32;
        let att = (-1.0 / (self.attack_ms.get().clamp(0.05, 500.0) * 0.001 * sr).max(1.0)).exp();
        let rel = (-1.0 / (self.release_ms.get().clamp(1.0, 5000.0) * 0.001 * sr).max(1.0)).exp();
        for s in buffer.iter_mut() {
            let abs = s.abs();
            let coeff = if abs > self.envelope { att } else { rel };
            self.envelope = coeff * self.envelope + (1.0 - coeff) * abs;
            if self.envelope > open_at {
                self.open = true;
                self.hold_left = hold;
            } else if self.open && self.envelope < close_at {
                if self.hold_left > 0 { self.hold_left -= 1; } else { self.open = false; }
            }
            let target = if self.open { 1.0 } else { floor };
            let gc = if target > self.gate_gain { att } else { rel };
            self.gate_gain = gc * self.gate_gain + (1.0 - gc) * target;
            *s *= self.gate_gain;
        }
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.gate_gain = 0.0;
        self.open = false;
        self.hold_left = 0;
    }
}
