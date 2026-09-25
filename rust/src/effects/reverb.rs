use crate::graph::*;
use crate::impl_effect_meta;

/// Canonical Freeverb delay-line lengths are tuned at 44.1 kHz; active lengths
/// are rescaled to the actual stream rate so the room size and comb modes stay
/// put at 44.1/48/96/192 kHz. Buffers are allocated once for `MAX_SR`, and the
/// *active* length changes at runtime (no allocation on the audio thread).
const TUNING_SR: f32 = 44100.0;
const MAX_SR: f32 = 192000.0;

fn cap_for(base: usize) -> usize {
    (base as f32 * MAX_SR / TUNING_SR).ceil() as usize + 1
}
fn len_for(base: usize, sr: f32) -> usize {
    ((base as f32 * sr / TUNING_SR) as usize).max(1)
}

struct Comb {
    buf: Vec<f32>, base: usize, len: usize, pos: usize,
    feedback: f32, damp1: f32, damp2: f32, fstore: f32,
}
impl Comb {
    fn new(base: usize) -> Self {
        Self { buf: vec![0.0; cap_for(base)], base, len: len_for(base, 48000.0),
            pos: 0, feedback: 0.5, damp1: 0.5, damp2: 0.5, fstore: 0.0 }
    }
    fn set_damp(&mut self, v: f32) { self.damp1 = v; self.damp2 = 1.0 - v; }
    fn retune(&mut self, sr: f32) {
        let len = len_for(self.base, sr).min(self.buf.len());
        if len != self.len {
            self.len = len;
            if self.pos >= len { self.pos = 0; }
        }
    }
    #[inline] fn tick(&mut self, input: f32) -> f32 {
        let out = self.buf[self.pos];
        self.fstore = out * self.damp2 + self.fstore * self.damp1;
        self.buf[self.pos] = input + self.fstore * self.feedback;
        self.pos = (self.pos + 1) % self.len;
        out
    }
    fn reset(&mut self) { self.buf.fill(0.0); self.fstore = 0.0; }
}

struct Allpass { buf: Vec<f32>, base: usize, len: usize, pos: usize }
impl Allpass {
    fn new(base: usize) -> Self {
        Self { buf: vec![0.0; cap_for(base)], base, len: len_for(base, 48000.0), pos: 0 }
    }
    fn retune(&mut self, sr: f32) {
        let len = len_for(self.base, sr).min(self.buf.len());
        if len != self.len {
            self.len = len;
            if self.pos >= len { self.pos = 0; }
        }
    }
    #[inline] fn tick(&mut self, x: f32) -> f32 {
        let b = self.buf[self.pos];
        let out = -x + b;
        self.buf[self.pos] = x + b * 0.5;
        self.pos = (self.pos + 1) % self.len;
        out
    }
    fn reset(&mut self) { self.buf.fill(0.0); }
}

pub struct Reverb {
    pub enabled: AtomicEnabled,
    pub room_size: AtomicF32,
    pub damping: AtomicF32,
    pub mix: SmoothedParam,
    pub pre_delay_ms: AtomicF32,
    combs: [Comb; 8],
    allpasses: [Allpass; 4],
    delay_buf: Vec<f32>,
    delay_pos: usize,
    /// Audio-thread-only: last stream rate the delay lines were tuned for.
    tuned_sr: f32,
}

impl Reverb {
    pub fn new(room: f32, damp: f32, mix: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            room_size: AtomicF32::new(room), damping: AtomicF32::new(damp),
            mix: SmoothedParam::new(mix), pre_delay_ms: AtomicF32::new(0.0),
            combs: [Comb::new(1116),Comb::new(1188),Comb::new(1277),Comb::new(1356),
                    Comb::new(1422),Comb::new(1491),Comb::new(1557),Comb::new(1617)],
            allpasses: [Allpass::new(556),Allpass::new(441),Allpass::new(341),Allpass::new(225)],
            // 100 ms max pre-delay at MAX_SR.
            delay_buf: vec![0.0; (0.1 * MAX_SR) as usize], delay_pos: 0,
            tuned_sr: 48000.0,
        }
    }
}

impl AudioEffect for Reverb {
    impl_effect_meta!(self, EffectType::Reverb, {
        "room_size" => room_size, "damping" => damping,
        "mix" => mix, "pre_delay_ms" => pre_delay_ms,
    });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let sr = sample_rate as f32;
        if sr != self.tuned_sr {
            self.tuned_sr = sr;
            for c in &mut self.combs { c.retune(sr); }
            for a in &mut self.allpasses { a.retune(sr); }
        }
        let room = self.room_size.get().clamp(0.0, 1.0) * 0.28 + 0.7;
        // Canonical Freeverb damping scale (0.4). Unscaled, damping = 1 zeroed
        // the comb feedback path, leaving a short slapback and no tail.
        let damp = self.damping.get().clamp(0.0, 1.0) * 0.4;
        for c in &mut self.combs { c.feedback = room; c.set_damp(damp); }
        let mix_coeff = smooth_coeff(sr, 15.0);
        let pd_samples = ((self.pre_delay_ms.get().max(0.0) * 0.001 * sr) as usize)
            .min(self.delay_buf.len() - 1);

        for s in buffer.iter_mut() {
            let mix = self.mix.tick(mix_coeff).clamp(0.0, 1.0);
            let dry = *s;
            let delayed = if pd_samples > 0 {
                let rp = (self.delay_pos + self.delay_buf.len() - pd_samples) % self.delay_buf.len();
                let out = self.delay_buf[rp];
                self.delay_buf[self.delay_pos] = dry;
                self.delay_pos = (self.delay_pos + 1) % self.delay_buf.len();
                out
            } else { dry };

            let mut wet = 0.0f32;
            for c in &mut self.combs { wet += c.tick(delayed); }
            for ap in &mut self.allpasses { wet = ap.tick(wet); }
            *s = dry * (1.0 - mix) + wet * mix * 0.15;
        }
    }

    fn reset(&mut self) {
        for c in &mut self.combs { c.reset(); }
        for a in &mut self.allpasses { a.reset(); }
        self.delay_buf.fill(0.0); self.delay_pos = 0;
        self.mix.snap();
    }
}
