use crate::graph::*;
use crate::impl_effect_meta;

/// Lookahead in samples (5 ms at 48 kHz). Fixed in *samples*, not time, so
/// the value reported by `latency_samples` is constant across stream rates.
const LOOKAHEAD: usize = 240;
/// Window length: the emitted sample plus every sample still in the delay.
const WIN: usize = LOOKAHEAD + 1;
/// Queue ring capacity: one spare slot so a full queue (`WIN` live entries,
/// e.g. monotonically rising gains) is distinguishable from an empty one.
const CAP: usize = WIN + 1;

/// Lookahead brick-wall limiter.
///
/// The signal is delayed by `LOOKAHEAD` samples while the required gain
/// (`ceiling / |peak|`) is tracked over the same window with a sliding-window
/// minimum (monotonic queue, preallocated — RT-safe). Because the window
/// already contains the requirement of every sample about to be emitted, the
/// applied gain can ramp *down before a transient arrives* instead of
/// clamping it after the fact, which is what makes a limiter transparent.
/// A final clamp catches the sub-percent residue of the exponential attack.
pub struct Limiter {
    pub enabled: AtomicEnabled,
    pub ceiling_db: AtomicF32,
    pub release_ms: AtomicF32,
    delay: Vec<f32>,
    dpos: usize,
    /// Monotonic min-queue over the last `WIN` required gains.
    win_val: Vec<f32>,
    win_idx: Vec<u64>,
    head: usize,
    tail: usize,
    /// Running input-sample counter (window eviction key).
    n: u64,
    gain: f32,
}

impl Limiter {
    pub fn new(ceiling_db: f32, release_ms: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            ceiling_db: AtomicF32::new(ceiling_db),
            release_ms: AtomicF32::new(release_ms),
            delay: vec![0.0; LOOKAHEAD],
            dpos: 0,
            win_val: vec![0.0; CAP],
            win_idx: vec![0; CAP],
            head: 0,
            tail: 0,
            n: 0,
            gain: 1.0,
        }
    }
}

impl AudioEffect for Limiter {
    impl_effect_meta!(self, EffectType::Limiter, { "ceiling_db" => ceiling_db, "release_ms" => release_ms });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        let ceil = 10.0_f32.powf(self.ceiling_db.get().clamp(-30.0, 0.0) / 20.0);
        let rel = (-1.0 / (self.release_ms.get().clamp(1.0, 2000.0) * 0.001 * sample_rate as f32).max(1.0)).exp();
        // Attack reaches ~99.3% of a gain step within the lookahead window.
        let att = (-5.0 / LOOKAHEAD as f32).exp();
        for s in buffer.iter_mut() {
            let x = *s;
            // Required gain for this incoming sample.
            let req = (ceil / x.abs().max(1e-9)).min(1.0);
            // Monotonic queue push: drop entries that can never be the min.
            while self.tail != self.head {
                let last = (self.tail + CAP - 1) % CAP;
                if self.win_val[last] >= req { self.tail = last; } else { break; }
            }
            self.win_val[self.tail] = req;
            self.win_idx[self.tail] = self.n;
            self.tail = (self.tail + 1) % CAP;
            // Evict entries older than the window.
            while self.n >= LOOKAHEAD as u64
                && self.win_idx[self.head] < self.n - LOOKAHEAD as u64
            {
                self.head = (self.head + 1) % CAP;
            }
            let target = self.win_val[self.head];
            // Fast attack toward a lower target, param-set release upward.
            let c = if target < self.gain { att } else { rel };
            self.gain = target + (self.gain - target) * c;
            // Emit the delayed sample with the anticipated gain.
            let out = self.delay[self.dpos];
            self.delay[self.dpos] = x;
            self.dpos = (self.dpos + 1) % LOOKAHEAD;
            self.n += 1;
            *s = (out * self.gain).clamp(-ceil, ceil);
        }
    }

    fn latency_samples(&self) -> usize { LOOKAHEAD }

    fn flush(&mut self) { self.reset(); }

    fn reset(&mut self) {
        self.delay.fill(0.0);
        self.dpos = 0;
        self.head = 0;
        self.tail = 0;
        self.n = 0;
        self.gain = 1.0;
    }
}
