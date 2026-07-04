use crate::graph::*;
use crate::effects::pitch_shift::PitchShift;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MusicalKey {
    C, Db, D, Eb, E, F, Gb, G, Ab, A, Bb, B,
}

impl MusicalKey {
    pub fn semitone_offset(&self) -> i32 { *self as i32 }
    pub fn from_index(i: u8) -> Self {
        match i % 12 {
            0 => Self::C, 1 => Self::Db, 2 => Self::D, 3 => Self::Eb,
            4 => Self::E, 5 => Self::F, 6 => Self::Gb, 7 => Self::G,
            8 => Self::Ab, 9 => Self::A, 10 => Self::Bb, _ => Self::B,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    Chromatic, Major, Minor, Pentatonic, Blues, Dorian, Mixolydian,
}

impl Scale {
    pub fn intervals(&self) -> &[i32] {
        match self {
            Self::Chromatic  => &[0,1,2,3,4,5,6,7,8,9,10,11],
            Self::Major      => &[0,2,4,5,7,9,11],
            Self::Minor      => &[0,2,3,5,7,8,10],
            Self::Pentatonic => &[0,2,4,7,9],
            Self::Blues      => &[0,3,5,6,7,10],
            Self::Dorian     => &[0,2,3,5,7,9,10],
            Self::Mixolydian => &[0,2,4,5,7,9,10],
        }
    }
    pub fn from_index(i: u8) -> Self {
        match i {
            0 => Self::Chromatic, 1 => Self::Major, 2 => Self::Minor,
            3 => Self::Pentatonic, 4 => Self::Blues, 5 => Self::Dorian,
            6 => Self::Mixolydian, _ => Self::Major,
        }
    }
}

/// Shared between AutoTune (audio thread) and the pitch detector worker.
pub struct DetectShared {
    pub freq: AtomicF32,
    pub conf: AtomicF32,
    /// Generation counter: detector increments on each new estimate so the
    /// audio thread can tell results apart without a mutex.
    pub gen: AtomicU32,
}

impl DetectShared {
    pub fn new() -> Self {
        Self {
            freq: AtomicF32::new(0.0),
            conf: AtomicF32::new(0.0),
            gen: AtomicU32::new(0),
        }
    }
}

pub struct AutoTune {
    pub enabled: AtomicEnabled,
    pub correction_speed: AtomicF32,
    pub retune_threshold: AtomicF32,
    pub humanize: AtomicF32,
    pub key: AtomicU32,    // u32-encoded MusicalKey
    pub scale: AtomicU32,  // u32-encoded Scale

    pub shared: Arc<DetectShared>,
    last_seen_gen: u32,
    current_shift: f32,
    target_shift: f32,
    shifting: bool,
    pitch_shifter: PitchShift,
}

impl AutoTune {
    pub fn new(key: MusicalKey, scale: Scale, speed: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            correction_speed: AtomicF32::new(speed),
            retune_threshold: AtomicF32::new(0.3),
            humanize: AtomicF32::new(0.0),
            key: AtomicU32::new(key as u32),
            scale: AtomicU32::new(scale as u32),
            shared: Arc::new(DetectShared::new()),
            last_seen_gen: 0,
            current_shift: 0.0,
            target_shift: 0.0,
            shifting: false,
            pitch_shifter: PitchShift::new(0.0),
        }
    }

    /// Detector worker uses this to publish new estimates.
    pub fn shared(&self) -> Arc<DetectShared> { Arc::clone(&self.shared) }

    fn current_key(&self) -> MusicalKey {
        MusicalKey::from_index(self.key.load(Ordering::Relaxed) as u8)
    }
    fn current_scale(&self) -> Scale {
        Scale::from_index(self.scale.load(Ordering::Relaxed) as u8)
    }

    fn quantize_to_scale(&self, freq: f32) -> f32 {
        if freq <= 0.0 { return freq; }
        let midi = 69.0 + 12.0 * (freq / 440.0).log2();
        let key_off = self.current_key().semitone_offset();
        let scale = self.current_scale();
        let intervals = scale.intervals();
        let midi_round = midi.round() as i32;
        let in_octave = ((midi_round - key_off) % 12 + 12) % 12;

        let mut min_dist = 12i32;
        let mut best = 0i32;
        for &iv in intervals {
            let d = ((in_octave - iv) % 12 + 12) % 12;
            let d = d.min(12 - d);
            if d < min_dist { min_dist = d; best = iv; }
        }
        let octave = (midi_round - key_off).div_euclid(12);
        let target_midi = key_off + octave * 12 + best;
        440.0 * 2.0_f32.powf((target_midi as f32 - 69.0) / 12.0)
    }
}

impl AudioEffect for AutoTune {
    fn effect_type(&self) -> EffectType { EffectType::AutoTune }
    fn set_enabled(&self, e: bool) { self.enabled.set(e); }
    fn is_enabled(&self) -> bool { self.enabled.get() }
    fn set_param(&self, name: &str, value: f32) -> bool {
        match name {
            "speed" => { self.correction_speed.set(value); true }
            "retune_threshold" => { self.retune_threshold.set(value); true }
            "humanize" => { self.humanize.set(value); true }
            "key" => {
                self.key.store((value as u32) % 12, Ordering::Relaxed);
                true
            }
            "scale" => {
                self.scale.store((value as u32) % 7, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let speed = self.correction_speed.get();
        if speed >= 0.999 { return; }

        // Pick up the most recent detector estimate (lock-free).
        let gen = self.shared.gen.load(Ordering::Acquire);
        if gen != self.last_seen_gen {
            self.last_seen_gen = gen;
            let freq = self.shared.freq.get();
            let conf = self.shared.conf.get();
            let thresh = self.retune_threshold.get();
            if conf >= thresh && freq > 50.0 && freq < 1500.0 {
                let target = self.quantize_to_scale(freq);
                self.target_shift = 12.0 * (target / freq).log2();
            } else {
                self.target_shift = 0.0;
            }
        }

        // Smooth correction.
        let coeff = speed.powf(0.1);
        self.current_shift = self.current_shift * coeff + self.target_shift * (1.0 - coeff);

        if self.current_shift.abs() > 0.01 {
            self.pitch_shifter.semitones.set(self.current_shift);
            self.pitch_shifter.process(buffer, sample_rate);
            self.shifting = true;
        } else if self.shifting {
            // Correction just crossed back to ~unity: clear the phase-vocoder
            // FIFO so it doesn't resume later with stale, glitchy state.
            self.pitch_shifter.reset();
            self.shifting = false;
        }
    }

    fn latency_samples(&self) -> usize {
        self.pitch_shifter.latency_samples() + 2048
    }

    fn reset(&mut self) {
        self.current_shift = 0.0;
        self.target_shift = 0.0;
        self.shifting = false;
        self.pitch_shifter.reset();
    }
}
