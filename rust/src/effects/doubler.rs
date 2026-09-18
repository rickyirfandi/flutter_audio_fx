use crate::graph::*;
use crate::impl_effect_meta;
use std::f32::consts::PI;

const BUFFER_LEN: usize = 8192;
const VOICE_ONE_DELAY_S: f32 = 0.025;
const VOICE_TWO_DELAY_S: f32 = 0.035;
const MODULATION_S: f32 = 0.002;

pub struct Doubler {
    pub enabled: AtomicEnabled,
    pub mix: SmoothedParam,
    pub spread: SmoothedParam,
    /// One shared history: both voices are taps into the same delay line.
    history: Vec<f32>,
    write_pos: usize,
    phase_one: f32,
    phase_two: f32,
}

impl Doubler {
    pub fn new(mix: f32, spread: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            mix: SmoothedParam::new(mix),
            spread: SmoothedParam::new(spread),
            history: vec![0.0; BUFFER_LEN],
            write_pos: 0,
            phase_one: 0.0,
            phase_two: 0.25,
        }
    }

    #[inline]
    fn read_interp(delay_line: &[f32], write_pos: usize, delay: f32) -> f32 {
        let len = delay_line.len() as f32;
        let position = ((write_pos as f32 - delay) % len + len) % len;
        let first = position as usize;
        let second = (first + 1) % delay_line.len();
        let fraction = position - first as f32;
        delay_line[first] * (1.0 - fraction) + delay_line[second] * fraction
    }
}

impl AudioEffect for Doubler {
    impl_effect_meta!(self, EffectType::Doubler, {
        "mix" => mix,
        "spread" => spread,
    });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() {
            return;
        }
        let sr = sample_rate as f32;
        let coeff = smooth_coeff(sr, 15.0);
        let max_delay = (BUFFER_LEN - 2) as f32;
        let base_one = (VOICE_ONE_DELAY_S * sr).min(max_delay);
        let base_two = (VOICE_TWO_DELAY_S * sr).min(max_delay);
        let max_depth = MODULATION_S * sr;

        for sample in buffer.iter_mut() {
            let mix = self.mix.tick(coeff).clamp(0.0, 1.0);
            let spread = self.spread.tick(coeff).clamp(0.0, 1.0);
            let dry = *sample;
            self.history[self.write_pos] = dry;
            self.write_pos = (self.write_pos + 1) % BUFFER_LEN;

            let depth_one = max_depth.min((base_one - 1.0).max(0.0));
            let depth_two = max_depth.min((base_two - 1.0).max(0.0));
            let delay_one = (base_one + (2.0 * PI * self.phase_one).sin() * depth_one * spread)
                .clamp(1.0, max_delay);
            let delay_two = (base_two + (2.0 * PI * self.phase_two).sin() * depth_two * spread)
                .clamp(1.0, max_delay);
            let doubled_one = Self::read_interp(&self.history, self.write_pos, delay_one);
            let doubled_two = Self::read_interp(&self.history, self.write_pos, delay_two);

            self.phase_one += 0.4 / sr;
            self.phase_two += 0.6 / sr;
            if self.phase_one >= 1.0 {
                self.phase_one -= 1.0;
            }
            if self.phase_two >= 1.0 {
                self.phase_two -= 1.0;
            }
            // Crossfade so gains always sum to 1 (never louder than the
            // input): mix = 1 is dry and the doubled pair at equal level.
            *sample = dry * (1.0 - 0.5 * mix) + 0.25 * mix * (doubled_one + doubled_two);
        }
    }

    fn reset(&mut self) {
        self.history.fill(0.0);
        self.write_pos = 0;
        self.phase_one = 0.0;
        self.phase_two = 0.25;
        self.mix.snap();
        self.spread.snap();
    }
}
