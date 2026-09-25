use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use arc_swap::ArcSwap;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SampleRate, SizedSample, StreamConfig, SupportedStreamConfig};
use ringbuf::{HeapCons, HeapProd, HeapRb, traits::{Producer, Consumer, Split, Observer}};

#[allow(unused_imports)]
use crate::graph::{AudioEffect, EffectSlot};
use crate::analysis::spectrum::{SpectrumAnalyzer, SpectrumData};
use crate::analysis::pitch::YinDetector;
use crate::effects::auto_tune::DetectShared;
use crate::io::file_io;
use crate::util::{DenormalGuard, Dither16};
use super::resample::{self, Resampler};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode { Idle = 0, Realtime = 1, Recording = 2, Preview = 3 }
impl Mode {
    fn from_u8(v: u8) -> Self {
        match v { 1 => Self::Realtime, 2 => Self::Recording, 3 => Self::Preview, _ => Self::Idle }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle", Self::Realtime => "realtime",
            Self::Recording => "recording", Self::Preview => "preview",
        }
    }
}

pub(crate) struct AtomicF32W(AtomicU32);
impl AtomicF32W {
    pub(crate) const fn new(v: f32) -> Self { Self(AtomicU32::new(v.to_bits())) }
    pub(crate) fn get(&self) -> f32 { f32::from_bits(self.0.load(Ordering::Relaxed)) }
    pub(crate) fn set(&self, v: f32) { self.0.store(v.to_bits(), Ordering::Relaxed); }
}

pub(crate) struct SharedState {
    /// Slots are individually `Arc`'d so chain edits can carry live slots
    /// (with their DSP state) into the new chain instead of rebuilding them.
    pub chain: ArcSwap<Vec<Arc<EffectSlot>>>,
    pub spectrum: Mutex<Option<SpectrumData>>,
    pub rms_level: AtomicF32W,
    pub pitch_freq: AtomicF32W,
    pub pitch_conf: AtomicF32W,
    pub is_running: AtomicBool,
    pub detect_shareds: Mutex<Vec<Arc<DetectShared>>>,
}

/// Commands sent from the public API to the audio control thread.
enum Command {
    StartRealtime,
    StartRecording { raw: String, processed: Option<String> },
    PreviewFile(String),
    Stop,
    Shutdown,
}

/// Reply for commands that can fail.
type Reply = Result<(), String>;

pub struct AudioRuntime {
    sample_rate: u32,
    buffer_size: usize,
    mode: AtomicU8,
    pub(crate) shared: Arc<SharedState>,
    /// Progress of the current `process_file` call (0.0..=1.0), readable from
    /// other threads while the blocking call runs.
    file_progress: AtomicF32W,
    // Serializes offline access to EffectSlot's UnsafeCell with stream lifecycle.
    operation_lock: Arc<Mutex<()>>,

    cmd_tx: mpsc::Sender<(Command, mpsc::Sender<Reply>)>,
    audio_thread: Mutex<Option<JoinHandle<()>>>,
}

impl AudioRuntime {
    pub fn new(sample_rate: u32, buffer_size: usize) -> Self {
        let shared = Arc::new(SharedState {
            chain: ArcSwap::from_pointee(Vec::new()),
            spectrum: Mutex::new(None),
            rms_level: AtomicF32W::new(0.0),
            pitch_freq: AtomicF32W::new(0.0),
            pitch_conf: AtomicF32W::new(0.0),
            is_running: AtomicBool::new(false),
            detect_shareds: Mutex::new(Vec::new()),
        });

        let (cmd_tx, cmd_rx) = mpsc::channel::<(Command, mpsc::Sender<Reply>)>();
        let shared_at = Arc::clone(&shared);
        let operation_lock = Arc::new(Mutex::new(()));
        let control_lock = Arc::clone(&operation_lock);
        let audio_thread = thread::Builder::new()
            .name("flutter_audio_fx-audio".into())
            .spawn(move || audio_control_loop(sample_rate, shared_at, cmd_rx, control_lock))
            .expect("spawn audio control thread");

        Self {
            sample_rate, buffer_size,
            mode: AtomicU8::new(Mode::Idle as u8),
            shared,
            file_progress: AtomicF32W::new(0.0),
            operation_lock,
            cmd_tx,
            audio_thread: Mutex::new(Some(audio_thread)),
        }
    }

    fn send(&self, cmd: Command) -> Reply {
        let (tx, rx) = mpsc::channel();
        self.cmd_tx.send((cmd, tx)).map_err(|_| "audio thread gone".to_string())?;
        rx.recv().map_err(|_| "audio thread reply lost".to_string())?
    }

    pub fn is_running(&self) -> bool { self.shared.is_running.load(Ordering::Relaxed) }
    pub fn mode_string(&self) -> String {
        Mode::from_u8(self.mode.load(Ordering::Relaxed)).label().to_string()
    }
    pub fn sample_rate(&self) -> u32 { self.sample_rate }
    pub fn buffer_size(&self) -> usize { self.buffer_size }
    pub fn chain_latency_ms(&self) -> f32 {
        let chain = self.shared.chain.load();
        // Bypassed slots add no delay, so only count enabled ones.
        let s: usize = chain.iter()
            .filter(|slot| slot.is_enabled())
            .map(|slot| slot.latency_samples())
            .sum();
        s as f32 / self.sample_rate as f32 * 1000.0
    }

    pub fn set_chain(&self, slots: Vec<Arc<EffectSlot>>) {
        let shareds: Vec<Arc<DetectShared>> =
            slots.iter().filter_map(|s| s.detect_shared()).collect();
        // swap (not store) so this control thread usually holds the last
        // reference to the old chain and its deallocation happens here, not
        // inside the audio callback.
        let _old = self.shared.chain.swap(Arc::new(slots));
        if let Ok(mut s) = self.shared.detect_shareds.lock() {
            *s = shareds;
        }
    }

    /// Snapshot of the live chain, used by the API layer to reuse slots when
    /// rebuilding the chain.
    pub(crate) fn chain_snapshot(&self) -> Arc<Vec<Arc<EffectSlot>>> {
        self.shared.chain.load_full()
    }

    pub fn toggle_effect(&self, idx: usize, enabled: bool) {
        let chain = self.shared.chain.load();
        if idx < chain.len() { chain[idx].set_enabled(enabled); }
    }

    pub fn update_param(&self, idx: usize, name: &str, value: f32) {
        let chain = self.shared.chain.load();
        if idx < chain.len() { chain[idx].queue_param(name, value); }
    }

    pub fn start_realtime(&self) -> Result<(), String> {
        let r = self.send(Command::StartRealtime);
        if r.is_ok() { self.mode.store(Mode::Realtime as u8, Ordering::Release); }
        r
    }

    pub fn start_realtime_with_recording(
        &self, raw_path: &str, proc_path: Option<&str>,
    ) -> Result<(), String> {
        let r = self.send(Command::StartRecording {
            raw: raw_path.to_string(),
            processed: proc_path.map(|s| s.to_string()),
        });
        if r.is_ok() { self.mode.store(Mode::Recording as u8, Ordering::Release); }
        r
    }

    pub fn stop(&self) -> Result<(), String> {
        let r = self.send(Command::Stop);
        self.mode.store(Mode::Idle as u8, Ordering::Release);
        r
    }

    // ─── Offline file processing (control thread CPU only) ───
    //
    // The effect chain is mono, so multi-channel input is downmixed before
    // processing (running time-based effects over interleaved frames would
    // smear delays/reverbs across channels) and the output is written mono.
    pub fn process_file(
        &self, input_path: &str, output_path: &str,
    ) -> Result<String, String> {
        let _operation = self.operation_lock.lock().map_err(|e| e.to_string())?;
        if self.is_running() {
            return Err("Stop the live engine before processing files".into());
        }
        self.file_progress.set(0.0);
        let (samples, meta) = file_io::read_wav_mono(input_path)?;
        let chain = self.shared.chain.load();
        // SAFETY: operation_lock excludes other renders and stream startup;
        // the running check excludes an existing live callback.
        let output = unsafe { render_samples(&samples, meta.sample_rate, &chain,
            |p| self.file_progress.set(p)) };
        file_io::write_wav(output_path, &output, meta.sample_rate, 1)?;
        self.file_progress.set(1.0);
        Ok(output_path.to_string())
    }

    pub fn file_progress(&self) -> f32 { self.file_progress.get() }

    pub fn preview_file(&self, input_path: &str) -> Result<(), String> {
        let r = self.send(Command::PreviewFile(input_path.to_string()));
        if r.is_ok() { self.mode.store(Mode::Preview as u8, Ordering::Release); }
        r
    }

    pub fn take_spectrum(&self) -> Option<SpectrumData> {
        self.shared.spectrum.lock().ok().and_then(|mut d| d.take())
    }
    pub fn rms_level(&self) -> f32 { self.shared.rms_level.get() }
    pub fn pitch_data(&self) -> (f32, f32) {
        (self.shared.pitch_freq.get(), self.shared.pitch_conf.get())
    }
}

impl Drop for AudioRuntime {
    fn drop(&mut self) {
        let (tx, _rx) = mpsc::channel();
        let _ = self.cmd_tx.send((Command::Shutdown, tx));
        if let Some(t) = self.audio_thread.lock().unwrap().take() {
            let _ = t.join();
        }
    }
}

/// Shared export/preview renderer. Pitch analysis runs synchronously on this
/// non-audio thread, using source samples (as the live microphone detector does).
/// SAFETY: the caller must exclude every other renderer and live DSP callback.
unsafe fn render_samples(
    samples: &[f32], sample_rate: u32, chain: &[Arc<EffectSlot>],
    mut progress: impl FnMut(f32),
) -> Vec<f32> {
    let _ftz = DenormalGuard::new();
    let detectors: Vec<_> = chain.iter().filter_map(|s| s.detect_shared()).collect();
    let mut yin = YinDetector::new(2048);
    let mut window = vec![0.0; yin.window()];
    for slot in chain { slot.prepare_offline(); }
    let mut output = Vec::with_capacity(samples.len());
    for (index, chunk) in samples.chunks(512).enumerate() {
        let pos = index * 512;
        if !detectors.is_empty() {
            // Offline lookahead is available, including for the first block.
            window.fill(0.0);
            let end = (pos + window.len()).min(samples.len());
            window[..end - pos].copy_from_slice(&samples[pos..end]);
            let (freq, conf) = yin.detect(&window, sample_rate).unwrap_or((0.0, 0.0));
            for detector in &detectors {
                detector.freq.set(freq);
                detector.conf.set(conf);
                detector.gen.fetch_add(1, Ordering::Release);
            }
        }
        let mut buf = chunk.to_vec();
        for slot in chain { slot.process_in_place(&mut buf, sample_rate); }
        output.extend_from_slice(&buf);
        progress(output.len() as f32 / samples.len() as f32);
    }
    for detector in &detectors {
        detector.freq.set(0.0);
        detector.conf.set(0.0);
        detector.gen.fetch_add(1, Ordering::Release);
    }
    for slot in chain { slot.as_mut().reset(); }
    output
}

// ─── Audio control thread ───
//
// Owns all `cpal::Stream` instances (which are `!Send` on some platforms),
// processes commands serially, and handles worker thread lifecycles.

fn audio_control_loop(
    sample_rate: u32,
    shared: Arc<SharedState>,
    rx: mpsc::Receiver<(Command, mpsc::Sender<Reply>)>,
    operation_lock: Arc<Mutex<()>>,
) {
    let mut state = AudioState::new();
    while let Ok((cmd, reply)) = rx.recv() {
        let _operation = operation_lock.lock().unwrap();
        let r = match cmd {
            Command::StartRealtime =>
                state.start_realtime(sample_rate, &shared, false, None, None),
            Command::StartRecording { raw, processed } =>
                state.start_realtime(sample_rate, &shared, true, Some(raw), processed),
            Command::PreviewFile(path) => state.preview_file(&path, &shared),
            Command::Stop => state.stop(&shared),
            Command::Shutdown => { let _ = state.stop(&shared); break; }
        };
        let _ = reply.send(r);
    }
}

#[derive(Default)]
struct AudioState {
    input: Option<cpal::Stream>,
    output: Option<cpal::Stream>,
    raw_writer: Option<WriterHandle>,
    proc_writer: Option<WriterHandle>,
    detector: Option<WorkerHandle>,
    spectrum: Option<WorkerHandle>,
}

/// Sample formats the live/preview paths can drive (converted to/from f32).
fn format_supported(f: SampleFormat) -> bool {
    matches!(f, SampleFormat::F32 | SampleFormat::I16 | SampleFormat::I32 | SampleFormat::U16)
}

/// Pick a stream config at `desired_sr`, preferring native f32, then the
/// fewest channels; fall back to the device default when `desired_sr` is not
/// offered (the resampler bridges the difference).
fn choose_config(
    ranges: Option<Vec<cpal::SupportedStreamConfigRange>>,
    default: Result<SupportedStreamConfig, String>,
    desired_sr: u32,
) -> Result<SupportedStreamConfig, String> {
    let best = ranges.unwrap_or_default().into_iter()
        .filter(|r| format_supported(r.sample_format())
            && r.min_sample_rate().0 <= desired_sr && desired_sr <= r.max_sample_rate().0)
        .min_by_key(|r| (r.sample_format() != SampleFormat::F32, r.channels()))
        .map(|r| r.with_sample_rate(SampleRate(desired_sr)));
    match best {
        Some(cfg) => Ok(cfg),
        None => {
            let cfg = default?;
            if format_supported(cfg.sample_format()) { Ok(cfg) }
            else { Err(format!("unsupported device sample format {:?}", cfg.sample_format())) }
        }
    }
}

fn choose_input_config(dev: &cpal::Device, desired_sr: u32) -> Result<SupportedStreamConfig, String> {
    choose_config(
        dev.supported_input_configs().ok().map(|r| r.collect()),
        dev.default_input_config().map_err(|e| format!("input config: {e}")),
        desired_sr)
}

fn choose_output_config(dev: &cpal::Device, desired_sr: u32) -> Result<SupportedStreamConfig, String> {
    choose_config(
        dev.supported_output_configs().ok().map(|r| r.collect()),
        dev.default_output_config().map_err(|e| format!("output config: {e}")),
        desired_sr)
}

/// Build an input stream in the device's native sample format.
macro_rules! build_input_as {
    ($dev:expr, $cfg:expr, $sink:ident, $($fmt:ident => $t:ty),*) => {{
        let sc: StreamConfig = $cfg.config();
        match $cfg.sample_format() {
            $(SampleFormat::$fmt => $dev.build_input_stream(
                &sc,
                move |d: &[$t], _: &cpal::InputCallbackInfo| $sink.push(d),
                |err| log::error!("Input error: {}", err),
                None,
            ).map_err(|e| format!("Input stream: {}", e)),)*
            f => Err(format!("unsupported input sample format {f:?}")),
        }
    }};
}

/// Build an output stream in the device's native sample format.
macro_rules! build_output_as {
    ($dev:expr, $cfg:expr, $renderer:ident, $label:literal, $($fmt:ident => $t:ty),*) => {{
        let sc: StreamConfig = $cfg.config();
        match $cfg.sample_format() {
            $(SampleFormat::$fmt => $dev.build_output_stream(
                &sc,
                move |d: &mut [$t], _: &cpal::OutputCallbackInfo| $renderer.render(d),
                |err| log::error!(concat!($label, " error: {}"), err),
                None,
            ).map_err(|e| format!(concat!($label, " stream: {}"), e)),)*
            f => Err(format!("unsupported output sample format {f:?}")),
        }
    }};
}

/// Input callback state: downmix to mono and fan out to the taps.
struct InputSink {
    mic: HeapProd<f32>,
    raw: Option<HeapProd<f32>>,
    det: HeapProd<f32>,
    channels: usize,
    shared: Arc<SharedState>,
    /// Frames per input callback, read by the output drift controller.
    block: Arc<AtomicUsize>,
}

impl InputSink {
    fn push<T: SizedSample>(&mut self, data: &[T]) where f32: FromSample<T> {
        if !self.shared.is_running.load(Ordering::Relaxed) { return; }
        let ch = self.channels;
        self.block.store(data.len() / ch, Ordering::Relaxed);
        let scale = 1.0 / ch as f32;
        for frame in data.chunks_exact(ch) {
            let mut acc = 0.0f32;
            for &x in frame { acc += <f32 as FromSample<T>>::from_sample_(x); }
            let s = acc * scale;
            let _ = self.mic.try_push(s);
            if let Some(raw) = self.raw.as_mut() { let _ = raw.try_push(s); }
            let _ = self.det.try_push(s);
        }
    }
}

/// Largest host buffer rendered in one pass; bigger callbacks are chunked so
/// the scratch never reallocates on the audio thread.
const MAX_BLOCK: usize = 8192;

/// Live output callback state.
///
/// Input and output devices run on independent clocks (and possibly different
/// nominal rates). The mic ring is read through a band-limited resampler whose
/// ratio is trimmed by a slow proportional controller holding the ring fill
/// near the smallest safe level: latency stays minimal and constant, and drift
/// is absorbed as an inaudible (<= 8.6 cent, typically < 0.5 cent) rate trim
/// instead of dropped or repeated samples. The integral term removes the
/// steady-state fill error a proportional-only loop would leave (which would
/// eat into the underrun cushion). Underruns widen the cushion automatically.
struct OutputRenderer {
    mic: HeapCons<f32>,
    rs: Resampler,
    base_ratio: f64,
    in_block: Arc<AtomicUsize>,
    in_rate: f32,
    sr: u32,
    avg_fill: f32,
    fill_init: bool,
    /// Integral of the relative fill error (PI controller state).
    drift_integral: f64,
    /// Jitter-buffer prefill: play silence until the ring first reaches its
    /// target, so start-up is not a burst of underruns.
    primed: bool,
    margin: usize,
    max_margin: usize,
    last_in: f32,
    mono: Vec<f32>,
    channels: usize,
    shared: Arc<SharedState>,
    proc: Option<HeapProd<f32>>,
    spec: HeapProd<f32>,
}

impl OutputRenderer {
    fn render<T: SizedSample + FromSample<f32>>(&mut self, data: &mut [T]) {
        let _ftz = DenormalGuard::new();
        let frames = data.len() / self.channels;
        if !self.update_drift(frames) {
            for s in data.iter_mut() { *s = T::from_sample(0.0f32); }
            self.shared.rms_level.set(0.0);
            return;
        }
        let chain = self.shared.chain.load();
        let mut sum_sq = 0.0f32;
        for block in data.chunks_mut(MAX_BLOCK * self.channels) {
            let n = block.len() / self.channels;
            let mono = &mut self.mono[..n];
            let mic = &mut self.mic;
            let last = &mut self.last_in;
            let mut starved = false;
            for s in mono.iter_mut() {
                *s = self.rs.next(|| match mic.try_pop() {
                    Some(x) => { *last = x; x }
                    None => { starved = true; *last *= 0.995; *last }
                });
            }
            if starved {
                // Grow the cushion by 1 ms per underrun (bounded).
                self.margin = (self.margin + (self.in_rate * 0.001) as usize).min(self.max_margin);
            }
            for slot in chain.iter() {
                // SAFETY: the cpal output callback is the unique DSP thread.
                unsafe { slot.process_in_place(mono, self.sr); }
            }
            for (frame, &v) in block.chunks_exact_mut(self.channels).zip(mono.iter()) {
                let out = T::from_sample(v);
                for c in frame { *c = out; }
            }
            if let Some(p) = self.proc.as_mut() {
                for &v in mono.iter() { let _ = p.try_push(v); }
            }
            for &v in mono.iter() {
                let _ = self.spec.try_push(v);
                sum_sq += v * v;
            }
        }
        if frames > 0 {
            self.shared.rms_level.set((sum_sq / frames as f32).sqrt());
        }
    }

    /// Trim the resampler ratio toward the target fill. Returns false while
    /// still prefilling (the caller outputs silence).
    fn update_drift(&mut self, frames: usize) -> bool {
        let fill = self.mic.occupied_len();
        let need = (frames as f64 * self.base_ratio).ceil() as usize;
        let in_block = self.in_block.load(Ordering::Relaxed);
        let target = (need + in_block + self.margin).max(1) as f32;
        if !self.primed {
            if (fill as f32) < target { return false; }
            self.primed = true;
        }
        if !self.fill_init {
            self.avg_fill = fill as f32;
            self.fill_init = true;
        }
        // ~0.5 s averaging smooths the sawtooth of block-wise arrivals.
        let alpha = (frames as f32 / (self.sr as f32 * 0.5)).min(1.0);
        self.avg_fill += (fill as f32 - self.avg_fill) * alpha;
        // Far over target (startup burst, app resumed from background):
        // discard the backlog once rather than trimming it out over seconds.
        let hard = target as usize + (self.in_rate * 0.03) as usize;
        if fill > hard {
            self.mic.skip(fill - target as usize);
            self.avg_fill = target;
        }
        let err = ((self.avg_fill - target) / target).clamp(-1.0, 1.0) as f64;
        // PI: Kp = 2000 ppm per 100 % fill error; integral time ~10 s.
        const KP: f64 = 0.002;
        const TI: f64 = 10.0;
        let dt = frames as f64 / self.sr as f64;
        self.drift_integral = (self.drift_integral + err * dt).clamp(-TI * 1.5, TI * 1.5);
        let trim = (KP * (err + self.drift_integral / TI)).clamp(-0.005, 0.005);
        self.rs.set_ratio(self.base_ratio * (1.0 + trim));
        true
    }
}

/// Preview callback state: plays a pre-rendered buffer, resampled to the
/// device rate when the file's rate is not offered.
struct PreviewRenderer {
    data: Arc<Vec<f32>>,
    pos: usize,
    rs: Resampler,
    channels: usize,
    shared: Arc<SharedState>,
}

impl PreviewRenderer {
    fn render<T: SizedSample + FromSample<f32>>(&mut self, out: &mut [T]) {
        // Play past the end by the resampler delay so the tail is not cut.
        let end = self.data.len() + resample::LATENCY;
        for frame in out.chunks_exact_mut(self.channels) {
            let data = &self.data;
            let pos = &mut self.pos;
            let v = self.rs.next(|| {
                let x = data.get(*pos).copied().unwrap_or(0.0);
                *pos += 1;
                x
            });
            let o = T::from_sample(v);
            for c in frame { *c = o; }
        }
        if self.pos >= end {
            self.shared.is_running.store(false, Ordering::Release);
        }
    }
}

impl AudioState {
    fn new() -> Self { Self::default() }

    fn start_realtime(
        &mut self,
        sample_rate: u32,
        shared: &Arc<SharedState>,
        recording: bool,
        raw_path: Option<String>,
        proc_path: Option<String>,
    ) -> Reply {
        if shared.is_running.load(Ordering::Relaxed) {
            return Err("Already running".into());
        }
        let host = cpal::default_host();
        let in_dev  = host.default_input_device().ok_or("No input device")?;
        let out_dev = host.default_output_device().ok_or("No output device")?;

        // Each side negotiates independently, preferring the engine rate. The
        // chain runs at the *output* rate; the resampler bridges any mismatch
        // (e.g. a 16 kHz Bluetooth mic into a 48 kHz output).
        let in_cfg  = choose_input_config(&in_dev, sample_rate)?;
        let out_cfg = choose_output_config(&out_dev, sample_rate)?;
        let in_sr  = in_cfg.sample_rate().0;
        let sr     = out_cfg.sample_rate().0;
        let in_channels  = in_cfg.channels() as usize;
        let out_channels = out_cfg.channels() as usize;
        log::info!("live: in {in_sr} Hz {in_channels}ch {:?}, out {sr} Hz {out_channels}ch {:?}",
            in_cfg.sample_format(), out_cfg.sample_format());

        if sr != 48000 {
            log::warn!(
                "negotiated {sr} Hz: noise suppression (RNNoise) requires 48 kHz \
                 and will pass audio through unchanged at this rate");
        }

        let (mic_prod, mic_cons) = HeapRb::<f32>::new((in_sr as usize) / 5).split();
        let (raw_prod, raw_cons) = HeapRb::<f32>::new((in_sr as usize) * 2).split();
        let (proc_prod, proc_cons) = HeapRb::<f32>::new((sr as usize) * 2).split();
        let (det_prod, det_cons) = HeapRb::<f32>::new((in_sr as usize) / 4).split();
        let (spec_prod, spec_cons) = HeapRb::<f32>::new((sr as usize) / 2).split();

        let in_block = Arc::new(AtomicUsize::new(0));
        let mut sink = InputSink {
            mic: mic_prod,
            raw: if recording { Some(raw_prod) } else { None },
            det: det_prod,
            channels: in_channels,
            shared: Arc::clone(shared),
            block: Arc::clone(&in_block),
        };
        let input_stream = build_input_as!(in_dev, in_cfg, sink,
            F32 => f32, I16 => i16, I32 => i32, U16 => u16)?;

        let mut renderer = OutputRenderer {
            mic: mic_cons,
            rs: Resampler::new(in_sr, sr),
            base_ratio: in_sr as f64 / sr as f64,
            in_block,
            in_rate: in_sr as f32,
            sr,
            avg_fill: 0.0,
            fill_init: false,
            drift_integral: 0.0,
            primed: false,
            margin: (in_sr as usize) / 1000, // 1 ms initial cushion
            max_margin: (in_sr as usize) / 50, // 20 ms
            last_in: 0.0,
            mono: vec![0.0; MAX_BLOCK],
            channels: out_channels,
            shared: Arc::clone(shared),
            proc: if recording { Some(proc_prod) } else { None },
            spec: spec_prod,
        };
        let output_stream = build_output_as!(out_dev, out_cfg, renderer, "Output",
            F32 => f32, I16 => i16, I32 => i32, U16 => u16)?;

        // Open both files before starting capture. Failure must reach the caller,
        // not disappear inside a writer thread after start() reported success.
        // The raw take is at the mic rate, the processed take at the chain rate.
        let raw_file = if recording { raw_path.as_deref().map(|p| create_writer(p, in_sr)).transpose()? } else { None };
        let proc_file = if recording { proc_path.as_deref().map(|p| create_writer(p, sr)).transpose()? } else { None };

        input_stream.play().map_err(|e| format!("Play input: {}", e))?;
        output_stream.play().map_err(|e| format!("Play output: {}", e))?;

        // Only mark running once both streams are live: setting it earlier
        // left the engine stuck in "Already running" forever when stream
        // construction failed. (The callbacks tolerate the brief gap — input
        // no-ops and output plays silence until this flips.)
        shared.is_running.store(true, Ordering::Release);

        if let Some(writer) = raw_file { self.raw_writer = Some(spawn_writer(writer, raw_cons)); }
        if let Some(writer) = proc_file { self.proc_writer = Some(spawn_writer(writer, proc_cons)); }
        self.detector = Some(spawn_detector(in_sr, det_cons, Arc::clone(shared)));
        self.spectrum = Some(spawn_spectrum(sr, spec_cons, Arc::clone(shared)));

        self.input = Some(input_stream);
        self.output = Some(output_stream);
        Ok(())
    }

    fn preview_file(&mut self, input_path: &str, shared: &Arc<SharedState>) -> Reply {
        if shared.is_running.load(Ordering::Relaxed) {
            return Err("Engine already running".into());
        }
        // Mono chain — downmix multi-channel input (playback upmixes again).
        let (samples, meta) = file_io::read_wav_mono(input_path)?;
        let chain = shared.chain.load();
        // SAFETY: the control loop holds operation_lock and no live DSP runs.
        let processed = unsafe { render_samples(&samples, meta.sample_rate, &chain, |_| {}) };

        let host = cpal::default_host();
        let dev = host.default_output_device().ok_or("No output device")?;
        let out_cfg = choose_output_config(&dev, meta.sample_rate)?;
        let mut renderer = PreviewRenderer {
            data: Arc::new(processed),
            pos: 0,
            rs: Resampler::new(meta.sample_rate, out_cfg.sample_rate().0),
            channels: out_cfg.channels() as usize,
            shared: Arc::clone(shared),
        };
        let stream = build_output_as!(dev, out_cfg, renderer, "Preview",
            F32 => f32, I16 => i16, I32 => i32, U16 => u16)?;
        // Set before play(): the callback clears this flag at end-of-file, so
        // storing it afterwards could race a very short file and wedge the
        // engine in "running". A failed build/play leaves it false.
        shared.is_running.store(true, Ordering::Release);
        if let Err(e) = stream.play() {
            shared.is_running.store(false, Ordering::Release);
            return Err(format!("Preview play: {}", e));
        }
        self.output = Some(stream);
        Ok(())
    }

    fn stop(&mut self, shared: &Arc<SharedState>) -> Reply {
        shared.is_running.store(false, Ordering::Release);
        self.input.take();
        self.output.take();
        let mut errors = Vec::new();
        if let Some(h) = self.raw_writer.take() {
            if let Err(e) = h.join_blocking() { errors.push(format!("raw recording: {e}")); }
        }
        if let Some(h) = self.proc_writer.take() {
            if let Err(e) = h.join_blocking() { errors.push(format!("processed recording: {e}")); }
        }
        if let Some(h) = self.detector.take()    { h.join_blocking(); }
        if let Some(h) = self.spectrum.take()    { h.join_blocking(); }
        if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
    }
}

// ─── Workers ───

struct WriterHandle { stop: Arc<AtomicBool>, thread: Option<JoinHandle<Reply>> }

/// Generic stop+join handle for the detector / spectrum worker threads.
struct WorkerHandle { stop: Arc<AtomicBool>, thread: Option<JoinHandle<()>> }
impl WorkerHandle {
    fn join_blocking(mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() { let _ = t.join(); }
    }
}

fn create_writer(path: &str, sample_rate: u32)
    -> Result<hound::WavWriter<std::io::BufWriter<std::fs::File>>, String>
{
    let spec = hound::WavSpec {
        channels: 1, sample_rate, bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    hound::WavWriter::create(path, spec).map_err(|e| format!("WAV create {path}: {e}"))
}

fn spawn_writer<W: std::io::Write + std::io::Seek + Send + 'static>(
    mut writer: hound::WavWriter<W>, mut cons: ringbuf::HeapCons<f32>,
) -> WriterHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = Arc::clone(&stop);
    let thread = thread::spawn(move || {
        let mut scratch = [0.0f32; 1024];
        let mut dither = Dither16::new(0x2545_F491);
        loop {
            let n = cons.pop_slice(&mut scratch);
            if n > 0 {
                for &s in &scratch[..n] {
                    writer.write_sample(dither.quantize(s)).map_err(|e| format!("WAV write: {e}"))?;
                }
            } else if stop_t.load(Ordering::Acquire) {
                while cons.occupied_len() > 0 {
                    if let Some(s) = cons.try_pop() {
                        writer.write_sample(dither.quantize(s)).map_err(|e| format!("WAV write: {e}"))?;
                    } else { break; }
                }
                break;
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }
        writer.finalize().map_err(|e| format!("WAV finalize: {e}"))
    });
    WriterHandle { stop, thread: Some(thread) }
}

impl WriterHandle {
    fn join_blocking(mut self) -> Reply {
        self.stop.store(true, Ordering::Release);
        match self.thread.take() {
            Some(t) => t.join().map_err(|_| "recording writer panicked".to_string())?,
            None => Ok(()),
        }
    }
}

/// Spectrum analysis worker: consumes the processed output stream and computes
/// FFT frames off the audio thread (the FFT + the owned-Vec result would
/// otherwise allocate on the RT callback).
fn spawn_spectrum(
    sample_rate: u32, mut cons: ringbuf::HeapCons<f32>, shared: Arc<SharedState>,
) -> WorkerHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = Arc::clone(&stop);
    let thread = thread::spawn(move || {
        let mut az = SpectrumAnalyzer::new(2048);
        az.set_sample_rate(sample_rate);
        let mut scratch = [0.0f32; 1024];
        loop {
            let n = cons.pop_slice(&mut scratch);
            if n > 0 {
                az.feed(&scratch[..n]);
                if let Some(spec) = az.get_spectrum() {
                    if let Ok(mut out) = shared.spectrum.lock() { *out = Some(spec); }
                }
            } else if stop_t.load(Ordering::Acquire) {
                break;
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }
    });
    WorkerHandle { stop, thread: Some(thread) }
}

fn spawn_detector(
    sample_rate: u32, mut cons: ringbuf::HeapCons<f32>, shared: Arc<SharedState>,
) -> WorkerHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = Arc::clone(&stop);
    let thread = thread::spawn(move || {
        let mut yin = YinDetector::new(2048);
        let win = yin.window();
        let mut buf = vec![0.0f32; win];
        let mut filled = 0usize;
        loop {
            let needed = win - filled;
            let n = cons.pop_slice(&mut buf[filled..filled + needed]);
            filled += n;
            if filled >= win {
                if let Some((freq, conf)) = yin.detect(&buf, sample_rate) {
                    shared.pitch_freq.set(freq);
                    shared.pitch_conf.set(conf);
                    if let Ok(shareds) = shared.detect_shareds.lock() {
                        for s in shareds.iter() {
                            s.freq.set(freq);
                            s.conf.set(conf);
                            s.gen.fetch_add(1, Ordering::Release);
                        }
                    }
                } else {
                    shared.pitch_conf.set(0.0);
                    if let Ok(shareds) = shared.detect_shareds.lock() {
                        for s in shareds.iter() {
                            s.conf.set(0.0);
                            s.gen.fetch_add(1, Ordering::Release);
                        }
                    }
                }
                let half = win / 2;
                buf.copy_within(half.., 0);
                filled = half;
            } else if stop_t.load(Ordering::Acquire) {
                break;
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }
    });
    WorkerHandle { stop, thread: Some(thread) }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    use crate::effects::{AutoTune, MusicalKey, Scale, NoiseGate};
    use std::io::{self, Cursor, Seek, SeekFrom, Write};

    #[test]
    fn file_renderer_applies_queued_parameters_and_enabled_state() {
        let slot = Arc::new(EffectSlot::new(Box::new(NoiseGate::new(-80.0, 1.0, 1.0))));
        let input = vec![0.25; 4096];
        slot.queue_param("threshold_db", 0.0);
        let chain = vec![slot.clone()];
        let muted = unsafe { render_samples(&input, 48000, &chain, |_| {}) };
        assert!(muted[2048..].iter().all(|s| s.abs() < 1e-4));
        slot.queue_param("threshold_db", -80.0);
        let audible = unsafe { render_samples(&input, 48000, &chain, |_| {}) };
        assert!(audible[2048..].iter().all(|s| *s > 0.24));
        slot.set_enabled(false);
        assert_eq!(unsafe { render_samples(&input, 48000, &chain, |_| {}) }, input);
        slot.set_enabled(true);
        slot.queue_param("threshold_db", 0.0);
        let muted_again = unsafe { render_samples(&input, 48000, &chain, |_| {}) };
        assert!(muted_again[2048..].iter().all(|s| s.abs() < 1e-4));
    }

    #[test]
    fn file_autotune_tracks_changing_notes_and_repeated_renders() {
        let tune = AutoTune::new(MusicalKey::C, Scale::Chromatic, 0.0);
        let detector = tune.shared();
        let chain = vec![Arc::new(EffectSlot::with_detect(Box::new(tune), Some(detector)))];
        let input: Vec<f32> = (0..38400).map(|i| {
            let hz = if i < 19200 { 225.0 } else { 450.0 };
            (2.0 * std::f32::consts::PI * hz * i as f32 / 48000.0).sin() * 0.4
        }).collect();
        for _ in 0..2 {
            let output = unsafe { render_samples(&input, 48000, &chain, |_| {}) };
            let mut yin = YinDetector::new(2048);
            for (start, expected) in [(10000, 220.0), (30000, 440.0)] {
                let (hz, confidence) = yin.detect(&output[start..start + 2048], 48000).unwrap();
                assert!(confidence > 0.9);
                assert!((hz - expected).abs() < 2.0, "expected {expected}, got {hz}");
            }
        }
    }

    /// Simulate independent input/output clocks through the live renderer.
    /// Returns (final ring fill, final safety margin, output tail).
    fn run_drift(ppm: f64, in_rate: u32, out_rate: u32) -> (usize, usize, Vec<f32>) {
        let runtime = AudioRuntime::new(out_rate, 256);
        let (mut prod, cons) = HeapRb::<f32>::new(in_rate as usize / 5).split();
        let (spec_prod, _spec_cons) = HeapRb::<f32>::new(1024).split();
        let in_block = 192usize;
        let block = Arc::new(AtomicUsize::new(in_block));
        let mut r = OutputRenderer {
            mic: cons,
            rs: Resampler::new(in_rate, out_rate),
            base_ratio: in_rate as f64 / out_rate as f64,
            in_block: block,
            in_rate: in_rate as f32,
            sr: out_rate,
            avg_fill: 0.0,
            fill_init: false,
            drift_integral: 0.0,
            primed: false,
            margin: in_rate as usize / 1000,
            max_margin: in_rate as usize / 50,
            last_in: 0.0,
            mono: vec![0.0; MAX_BLOCK],
            channels: 1,
            shared: Arc::clone(&runtime.shared),
            proc: None,
            spec: spec_prod,
        };
        let in_period = in_block as f64 / (in_rate as f64 * (1.0 + ppm * 1e-6));
        let out_period = 256.0 / out_rate as f64;
        let (mut t_in, mut t_out, mut n) = (0.0f64, 0.0f64, 0u64);
        let mut out = vec![0.0f32; 256];
        let mut tail = Vec::new();
        let seconds = 60.0;
        while t_out < seconds {
            if t_in <= t_out {
                for _ in 0..in_block {
                    let ph = 2.0 * std::f64::consts::PI * 1000.0 * n as f64 / in_rate as f64;
                    let _ = prod.try_push((ph.sin() * 0.5) as f32);
                    n += 1;
                }
                t_in += in_period;
            } else {
                r.render(&mut out[..]);
                if t_out > seconds - 1.0 { tail.extend_from_slice(&out); }
                t_out += out_period;
            }
        }
        (r.mic.occupied_len(), r.margin, tail)
    }

    #[test]
    fn live_path_absorbs_clock_drift_cleanly() {
        for ppm in [-300.0, 0.0, 300.0] {
            let (fill, margin, tail) = run_drift(ppm, 48000, 48000);
            // Latency stays small and bounded (<= ~10 ms of buffered input).
            assert!(fill < 480, "{ppm} ppm: ring fill grew to {fill}");
            // At most a couple of start-up underruns widened the cushion.
            assert!(margin <= 48 * 3, "{ppm} ppm: repeated underruns, margin {margin}");
            // The tone is reproduced cleanly (no dropped/held samples). The
            // window is short (85 ms) because the controller's residual
            // +/-60 ppm trim wobble (0.1 cent) would otherwise smear a
            // fixed-frequency correlation over a whole second.
            let tail = &tail[tail.len() - 4096..];
            let f = 1000.0 * (1.0 + ppm as f32 * 1e-6);
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, &s) in tail.iter().enumerate() {
                let p = 2.0 * std::f64::consts::PI * f as f64 * i as f64 / 48000.0;
                re += s as f64 * p.cos();
                im -= s as f64 * p.sin();
            }
            let amp = 2.0 * (re * re + im * im).sqrt() / tail.len() as f64;
            let energy: f64 = tail.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / tail.len() as f64;
            let purity = amp * amp / 2.0 / energy;
            assert!(purity > 0.999, "{ppm} ppm: purity {purity}");
        }
    }

    #[test]
    fn live_path_bridges_bluetooth_mic_rate() {
        // 16 kHz headset mic into a 48 kHz output: previously a start error.
        let (fill, margin, tail) = run_drift(0.0, 16000, 48000);
        assert!(fill < 16000 / 50, "ring fill {fill}");
        assert!(margin <= 16 * 3, "margin {margin}");
        let energy: f32 = tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32;
        assert!((energy.sqrt() - 0.5 / std::f32::consts::SQRT_2).abs() < 0.02, "level {}", energy.sqrt());
    }

    struct FailingSink {
        data: Cursor<Vec<u8>>,
        fail_write: Arc<AtomicBool>,
        fail_seek: Arc<AtomicBool>,
    }
    impl Write for FailingSink {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            if self.fail_write.load(Ordering::Relaxed) { return Err(io::Error::other("disk full")); }
            self.data.write(data)
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    impl Seek for FailingSink {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            if self.fail_seek.load(Ordering::Relaxed) { return Err(io::Error::other("header seek failed")); }
            self.data.seek(pos)
        }
    }

    fn failing_writer(finalize: bool) -> WriterHandle {
        let fail_write = Arc::new(AtomicBool::new(false));
        let fail_seek = Arc::new(AtomicBool::new(false));
        let sink = FailingSink { data: Cursor::new(Vec::new()), fail_write: fail_write.clone(), fail_seek: fail_seek.clone() };
        let writer = hound::WavWriter::new(sink, hound::WavSpec {
            channels: 1, sample_rate: 48000, bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        }).unwrap();
        let (mut producer, consumer) = HeapRb::<f32>::new(16).split();
        if finalize { fail_seek.store(true, Ordering::Relaxed); }
        else {
            fail_write.store(true, Ordering::Relaxed);
            producer.try_push(0.5).unwrap();
        }
        spawn_writer(writer, consumer)
    }

    #[test]
    fn recording_creation_error_is_returned() {
        let err = create_writer(std::env::temp_dir().to_str().unwrap(), 48000).err().unwrap();
        assert!(err.contains("WAV create"));
    }

    #[test]
    fn recording_write_and_finalize_errors_reach_stop_and_all_workers_join() {
        let runtime = AudioRuntime::new(48000, 256);
        let mut state = AudioState::new();
        state.raw_writer = Some(failing_writer(false));
        state.proc_writer = Some(failing_writer(true));
        let error = state.stop(&runtime.shared).unwrap_err();
        assert!(error.contains("raw recording: WAV write: disk full"), "{error}");
        assert!(error.contains("processed recording: WAV finalize:"), "{error}");
        assert!(state.raw_writer.is_none() && state.proc_writer.is_none());
        assert!(state.stop(&runtime.shared).is_ok());
    }
}
