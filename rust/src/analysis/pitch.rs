/// YIN pitch detector with all working buffers pre-allocated.
///
/// Designed to be invoked from a worker thread, not the audio callback —
/// the difference function is O(n²) and would blow the audio deadline.
pub struct YinDetector {
    window: usize,
    threshold: f32,
    yin_buf: Vec<f32>,
}

impl YinDetector {
    pub fn new(window: usize) -> Self {
        Self {
            window,
            threshold: 0.15,
            yin_buf: vec![0.0; window / 2],
        }
    }

    pub fn window(&self) -> usize { self.window }

    /// Returns (frequency in Hz, confidence in [0,1]) if a pitch is detected.
    pub fn detect(&mut self, signal: &[f32], sample_rate: u32) -> Option<(f32, f32)> {
        if signal.len() < self.window { return None; }
        let n = self.window;
        let half = n / 2;

        // Step 1: difference function.
        for tau in 0..half {
            let mut sum = 0.0f32;
            for j in 0..half {
                let d = signal[j] - signal[j + tau];
                sum += d * d;
            }
            self.yin_buf[tau] = sum;
        }

        // Step 2: cumulative mean normalised difference.
        self.yin_buf[0] = 1.0;
        let mut running = 0.0f32;
        for tau in 1..half {
            running += self.yin_buf[tau];
            self.yin_buf[tau] = if running > 0.0 {
                self.yin_buf[tau] * tau as f32 / running
            } else { 1.0 };
        }

        // Step 3: absolute threshold; find first dip below threshold and walk
        // forward to the local minimum.
        let min_tau = (sample_rate as f32 / 1000.0) as usize; // up to 1kHz
        let max_tau = ((sample_rate as f32 / 50.0) as usize).min(half - 1); // down to 50Hz

        let mut best_tau = 0usize;
        let mut best_val = 1.0f32;
        let mut tau = min_tau.max(2);
        while tau < max_tau {
            if self.yin_buf[tau] < self.threshold {
                while tau + 1 < max_tau && self.yin_buf[tau + 1] < self.yin_buf[tau] {
                    tau += 1;
                }
                best_tau = tau;
                best_val = self.yin_buf[tau];
                break;
            }
            tau += 1;
        }

        if best_tau == 0 || best_val >= 1.0 { return None; }

        // Step 4: parabolic interpolation around best_tau.
        let tau_f = if best_tau > 0 && best_tau < half - 1 {
            let s0 = self.yin_buf[best_tau - 1];
            let s1 = self.yin_buf[best_tau];
            let s2 = self.yin_buf[best_tau + 1];
            let denom = s0 - 2.0 * s1 + s2;
            if denom.abs() > 1e-10 { best_tau as f32 + (s0 - s2) / (2.0 * denom) }
            else { best_tau as f32 }
        } else { best_tau as f32 };

        Some((sample_rate as f32 / tau_f, 1.0 - best_val))
    }
}
