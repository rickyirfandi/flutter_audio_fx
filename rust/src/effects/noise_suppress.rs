use crate::graph::*;
use crate::impl_effect_meta;
use nnnoiseless::DenoiseState;

const FRAME_SIZE: usize = 480; // RNNoise: 10ms @ 48kHz

/// RNNoise-based noise suppression.
///
/// RNNoise works on fixed 480-sample frames, but the audio callback hands us
/// arbitrary (usually smaller) buffer sizes. We accumulate input into a frame,
/// denoise it once full, and emit the result through a one-frame output delay.
/// This gives a constant `FRAME_SIZE` latency and keeps samples correctly
/// aligned regardless of how the host chunks the stream.
pub struct NoiseSuppression {
    pub enabled: AtomicEnabled,
    pub strength: AtomicF32,
    denoiser: Box<DenoiseState<'static>>,
    /// Current input frame being filled.
    in_buf: Vec<f32>,
    /// Previous frame's denoised output, drained as new input arrives.
    out_buf: Vec<f32>,
    /// Read/write cursor shared by in_buf (write) and out_buf (read); both
    /// advance in lockstep and reset together when a frame completes.
    pos: usize,
}

impl NoiseSuppression {
    pub fn new(strength: f32) -> Self {
        Self {
            enabled: AtomicEnabled::new(true),
            strength: AtomicF32::new(strength),
            denoiser: DenoiseState::new(),
            in_buf: vec![0.0; FRAME_SIZE],
            out_buf: vec![0.0; FRAME_SIZE], // primed with silence → 1-frame latency
            pos: 0,
        }
    }
}

impl AudioEffect for NoiseSuppression {
    impl_effect_meta!(self, EffectType::NoiseSuppression, {
        "strength" => strength,
    });

    fn process(&mut self, buffer: &mut [f32], sample_rate: u32) {
        if !self.enabled.get() { return; }
        // RNNoise's model is trained on 48 kHz / 480-sample frames; running it
        // at any other rate produces garbled "denoising". Pass through instead
        // — the runtime logs a warning at startup when the negotiated rate is
        // not 48 kHz (no logging here: this is the audio thread).
        if sample_rate != 48000 { return; }
        let strength = self.strength.get().clamp(0.0, 1.0);

        // Stack scratch — no heap allocation on the audio thread.
        let mut rnn_in = [0.0f32; FRAME_SIZE];
        let mut rnn_out = [0.0f32; FRAME_SIZE];

        for s in buffer.iter_mut() {
            // Emit the matching sample from the previously denoised frame.
            let dry = *s;
            *s = self.out_buf[self.pos];
            // Capture this input sample into the frame under construction.
            self.in_buf[self.pos] = dry;
            self.pos += 1;

            if self.pos == FRAME_SIZE {
                // RNNoise expects i16-scaled magnitudes.
                for i in 0..FRAME_SIZE {
                    rnn_in[i] = self.in_buf[i] * 32767.0;
                }
                let _vad = self.denoiser.process_frame(&mut rnn_out, &rnn_in);
                for i in 0..FRAME_SIZE {
                    let clean = rnn_out[i] / 32767.0;
                    self.out_buf[i] = self.in_buf[i] * (1.0 - strength) + clean * strength;
                }
                self.pos = 0;
            }
        }
    }

    fn latency_samples(&self) -> usize { FRAME_SIZE }

    /// Drop buffered audio without rebuilding the denoiser (that allocates).
    fn flush(&mut self) {
        self.in_buf.fill(0.0);
        self.out_buf.fill(0.0);
        self.pos = 0;
    }

    fn reset(&mut self) {
        self.denoiser = DenoiseState::new();
        self.in_buf.fill(0.0);
        self.out_buf.fill(0.0);
        self.pos = 0;
    }
}
