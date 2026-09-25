use crate::graph::*;
use std::f32::consts::PI;

struct Biquad { b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, z1: f32, z2: f32, dirty: bool }
impl Biquad {
    fn new() -> Self { Self { b0:1.0,b1:0.0,b2:0.0,a1:0.0,a2:0.0,z1:0.0,z2:0.0,dirty:true } }
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
    /// Audio-thread glide state (log-frequency, dB, Q) and the stream rate the
    /// coefficients were computed for.
    cur_log_f: f32, cur_gain: f32, cur_q: f32,
    coeff_sr: f32,
    primed: bool,
}

impl EqBand {
    pub fn new(freq: f32, gain_db: f32, q: f32) -> Self {
        Self {
            freq: AtomicF32::new(freq), gain_db: AtomicF32::new(gain_db),
            q: AtomicF32::new(q), filter: Biquad::new(),
            cur_log_f: 0.0, cur_gain: 0.0, cur_q: 1.0, coeff_sr: 0.0, primed: false,
        }
    }

    /// Clamp targets to a range that is always stable at `sr`.
    fn targets(&self, sr: f32) -> (f32, f32, f32) {
        let f = self.freq.get().clamp(10.0, sr * 0.45);
        let g = self.gain_db.get().clamp(-24.0, 24.0);
        let q = self.q.get().clamp(0.1, 20.0);
        (f.ln(), g, q)
    }

    /// Glide toward the targets once per buffer (`glide` = one-pole weight)
    /// and recompute coefficients only while moving or after a rate change.
    fn update(&mut self, sr: f32, glide: f32) {
        let (lf, g, q) = self.targets(sr);
        let rate_changed = sr != self.coeff_sr;
        if !self.primed {
            self.cur_log_f = lf; self.cur_gain = g; self.cur_q = q;
            self.primed = true;
        } else {
            self.cur_log_f += (lf - self.cur_log_f) * glide;
            self.cur_gain += (g - self.cur_gain) * glide;
            self.cur_q += (q - self.cur_q) * glide;
            // Snap the last hair so settled bands stop recomputing.
            if (lf - self.cur_log_f).abs() < 1e-4 { self.cur_log_f = lf; }
            if (g - self.cur_gain).abs() < 1e-3 { self.cur_gain = g; }
            if (q - self.cur_q).abs() < 1e-4 { self.cur_q = q; }
        }
        let moving = self.cur_log_f != lf || self.cur_gain != g || self.cur_q != q;
        if rate_changed || moving || self.filter.dirty {
            self.filter.calc_peaking(self.cur_log_f.exp(), self.cur_gain, self.cur_q, sr);
            self.filter.dirty = moving;
            self.coeff_sr = sr;
        }
    }

    fn snap(&mut self) { self.primed = false; }
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
        // ~20 ms glide so dragging a band never steps the coefficients.
        let glide = 1.0 - (-(buffer.len() as f32) / (0.02 * sr)).exp();
        for band in &mut self.bands { band.update(sr, glide); }
        for sample in buffer.iter_mut() {
            for band in &mut self.bands {
                *sample = band.filter.tick(*sample);
            }
        }
    }

    fn reset(&mut self) {
        for band in &mut self.bands { band.filter.reset(); band.snap(); }
    }
}
