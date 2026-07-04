use crate::graph::*;
use std::f32::consts::PI;

struct Biquad { b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, z1: f32, z2: f32 }
impl Biquad {
    fn new() -> Self { Self { b0:1.0,b1:0.0,b2:0.0,a1:0.0,a2:0.0,z1:0.0,z2:0.0 } }
    fn calc_peaking(&mut self, freq: f32, gain_db: f32, q: f32, sr: f32) {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq / sr;
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha / a;
        self.b0 = (1.0 + alpha * a) / a0;
        self.b1 = (-2.0 * w0.cos()) / a0;
        self.b2 = (1.0 - alpha * a) / a0;
        self.a1 = self.b1;
        self.a2 = (1.0 - alpha / a) / a0;
    }
    #[inline(always)]
    fn tick(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
    fn reset(&mut self) { self.z1 = 0.0; self.z2 = 0.0; }
}

pub struct EqBand {
    pub freq: AtomicF32,
    pub gain_db: AtomicF32,
    pub q: AtomicF32,
    filter: Biquad,
    prev_freq: f32, prev_gain: f32, prev_q: f32,
}

impl EqBand {
    pub fn new(freq: f32, gain_db: f32, q: f32) -> Self {
        Self {
            freq: AtomicF32::new(freq), gain_db: AtomicF32::new(gain_db),
            q: AtomicF32::new(q), filter: Biquad::new(),
            prev_freq: -1.0, prev_gain: -999.0, prev_q: -1.0,
        }
    }
    fn update_if_dirty(&mut self, sr: f32) {
        let f = self.freq.get(); let g = self.gain_db.get(); let q = self.q.get();
        if (f - self.prev_freq).abs() > 0.01 || (g - self.prev_gain).abs() > 0.01
            || (q - self.prev_q).abs() > 0.001 {
            self.filter.calc_peaking(f, g, q, sr);
            self.prev_freq = f; self.prev_gain = g; self.prev_q = q;
        }
    }
}

pub struct Equalizer {
    pub enabled: AtomicEnabled,
    pub bands: Vec<EqBand>,
}

impl Equalizer {
    pub fn new_default() -> Self {
        let freqs = [31.0,62.0,125.0,250.0,500.0,1000.0,2000.0,4000.0,8000.0,16000.0];
        Self {
            enabled: AtomicEnabled::new(true),
            bands: freqs.iter().map(|&f| EqBand::new(f, 0.0, 1.414)).collect(),
        }
    }
    pub fn new(bands: Vec<EqBand>) -> Self {
        Self { enabled: AtomicEnabled::new(true), bands }
    }
}

impl AudioEffect for Equalizer {
    fn effect_type(&self) -> EffectType { EffectType::Equalizer }
    fn set_enabled(&self, e: bool) { self.enabled.set(e); }
    fn is_enabled(&self) -> bool { self.enabled.get() }
    fn set_param(&self, name: &str, value: f32) -> bool {
        // Format: "band_N_freq", "band_N_gain", "band_N_q"
        if let Some(rest) = name.strip_prefix("band_") {
            if let Some(idx_end) = rest.find('_') {
                if let Ok(idx) = rest[..idx_end].parse::<usize>() {
                    if idx < self.bands.len() {
                        let param = &rest[idx_end+1..];
                        return match param {
                            "freq" => { self.bands[idx].freq.set(value); true }
                            "gain" => { self.bands[idx].gain_db.set(value); true }
                            "q" => { self.bands[idx].q.set(value); true }
                            _ => false,
                        };
                    }
                }
            }
        }
        false
    }

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let sr = sample_rate as f32;
        for band in &mut self.bands { band.update_if_dirty(sr); }
        for sample in buffer.iter_mut() {
            for band in &mut self.bands {
                *sample = band.filter.tick(*sample);
            }
        }
    }

    fn reset(&mut self) {
        for band in &mut self.bands { band.filter.reset(); }
    }
}
