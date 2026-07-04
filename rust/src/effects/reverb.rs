use crate::graph::*;
use crate::impl_effect_meta;

const SR_SCALE: f32 = 48000.0 / 44100.0;
fn sc(s: usize) -> usize { (s as f32 * SR_SCALE) as usize }

struct Comb { buf: Vec<f32>, pos: usize, feedback: f32, damp1: f32, damp2: f32, fstore: f32 }
impl Comb {
    fn new(s: usize) -> Self { Self { buf: vec![0.0;s], pos:0, feedback:0.5, damp1:0.5, damp2:0.5, fstore:0.0 } }
    fn set_damp(&mut self, v: f32) { self.damp1 = v; self.damp2 = 1.0 - v; }
    #[inline] fn tick(&mut self, input: f32) -> f32 {
        let out = self.buf[self.pos];
        self.fstore = out * self.damp2 + self.fstore * self.damp1;
        self.buf[self.pos] = input + self.fstore * self.feedback;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
    fn reset(&mut self) { self.buf.fill(0.0); self.fstore = 0.0; }
}

struct Allpass { buf: Vec<f32>, pos: usize }
impl Allpass {
    fn new(s: usize) -> Self { Self { buf: vec![0.0;s], pos:0 } }
    #[inline] fn tick(&mut self, x: f32) -> f32 {
        let b = self.buf[self.pos];
        let out = -x + b;
        self.buf[self.pos] = x + b * 0.5;
        self.pos = (self.pos + 1) % self.buf.len();
        out
    }
    fn reset(&mut self) { self.buf.fill(0.0); }
}

pub struct Reverb {
    pub enabled: AtomicEnabled,
    pub room_size: AtomicF32,
    pub damping: AtomicF32,
    pub mix: AtomicF32,
    pub pre_delay_ms: AtomicF32,
    combs: [Comb; 8],
    allpasses: [Allpass; 4],
    delay_buf: Vec<f32>,
    delay_pos: usize,
}

impl Reverb {
    pub fn new(room: f32, damp: f32, mix: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            room_size: AtomicF32::new(room), damping: AtomicF32::new(damp),
            mix: AtomicF32::new(mix), pre_delay_ms: AtomicF32::new(0.0),
            combs: [Comb::new(sc(1116)),Comb::new(sc(1188)),Comb::new(sc(1277)),Comb::new(sc(1356)),
                    Comb::new(sc(1422)),Comb::new(sc(1491)),Comb::new(sc(1557)),Comb::new(sc(1617))],
            allpasses: [Allpass::new(sc(556)),Allpass::new(sc(441)),Allpass::new(sc(341)),Allpass::new(sc(225))],
            delay_buf: vec![0.0; 4800], delay_pos: 0,
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
        let room = self.room_size.get() * 0.28 + 0.7;
        let damp = self.damping.get();
        for c in &mut self.combs { c.feedback = room; c.set_damp(damp); }
        let mix = self.mix.get();
        let pd_samples = ((self.pre_delay_ms.get() * 0.001 * sample_rate as f32) as usize)
            .min(self.delay_buf.len() - 1);

        for s in buffer.iter_mut() {
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
    }
}
