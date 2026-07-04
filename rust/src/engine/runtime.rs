use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use arc_swap::ArcSwap;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, StreamConfig, SupportedStreamConfig};
use ringbuf::{HeapRb, traits::{Producer, Consumer, Split, Observer}};

use crate::graph::{AudioEffect, EffectSlot};
use crate::analysis::spectrum::{SpectrumAnalyzer, SpectrumData};
use crate::analysis::pitch::YinDetector;
use crate::effects::auto_tune::DetectShared;
use crate::io::file_io;

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
    pub chain: ArcSwap<Vec<EffectSlot>>,
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
        let audio_thread = thread::Builder::new()
            .name("flutter_audio_fx-audio".into())
            .spawn(move || audio_control_loop(sample_rate, shared_at, cmd_rx))
            .expect("spawn audio control thread");

        Self {
            sample_rate, buffer_size,
            mode: AtomicU8::new(Mode::Idle as u8),
            shared,
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
        let s: usize = chain.iter().map(|slot| slot.latency_samples()).sum();
        s as f32 / self.sample_rate as f32 * 1000.0
    }

    pub fn set_chain(
        &self,
        effects: Vec<Box<dyn AudioEffect>>,
        detect_shareds: Vec<Arc<DetectShared>>,
    ) {
        let slots: Vec<EffectSlot> = effects.into_iter().map(EffectSlot::new).collect();
        self.shared.chain.store(Arc::new(slots));
        if let Ok(mut s) = self.shared.detect_shareds.lock() {
            *s = detect_shareds;
        }
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
    pub fn process_file(
        &self, input_path: &str, output_path: &str,
    ) -> Result<String, String> {
        if self.is_running() {
            return Err("Stop the live engine before processing files".into());
        }
        let (samples, meta) = file_io::read_wav(input_path)?;
        let chunk = 512;
        let mut output = Vec::with_capacity(samples.len());
        let mut pos = 0;
        let chain = self.shared.chain.load();
        while pos < samples.len() {
            let end = (pos + chunk).min(samples.len());
            let mut buf: Vec<f32> = samples[pos..end].to_vec();
            for slot in chain.iter() {
                if !slot.is_enabled() { continue; }
                // SAFETY: no audio stream is running.
                let fx = unsafe { slot.as_mut() };
                fx.process(&mut buf, meta.sample_rate);
            }
            output.extend_from_slice(&buf);
            pos = end;
        }
        file_io::write_wav(output_path, &output, meta.sample_rate, meta.channels)?;
        Ok(output_path.to_string())
    }

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

// ─── Audio control thread ───
//
// Owns all `cpal::Stream` instances (which are `!Send` on some platforms),
// processes commands serially, and handles worker thread lifecycles.

fn audio_control_loop(
    sample_rate: u32,
    shared: Arc<SharedState>,
    rx: mpsc::Receiver<(Command, mpsc::Sender<Reply>)>,
) {
    let mut state = AudioState::new();
    while let Ok((cmd, reply)) = rx.recv() {
        let r = match cmd {
            Command::StartRealtime =>
                state.start_realtime(sample_rate, &shared, false, None, None),
            Command::StartRecording { raw, processed } =>
                state.start_realtime(sample_rate, &shared, true, Some(raw), processed),
            Command::PreviewFile(path) => state.preview_file(&path, &shared),
            Command::Stop => { state.stop(&shared); Ok(()) }
            Command::Shutdown => { state.stop(&shared); break; }
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

/// Pick an input config that supports `desired_sr` (preferring the fewest
/// channels), falling back to the device default if it does not.
fn choose_input_config(dev: &cpal::Device, desired_sr: u32) -> Result<SupportedStreamConfig, String> {
    if let Ok(ranges) = dev.supported_input_configs() {
        let mut best: Option<SupportedStreamConfig> = None;
        for range in ranges {
            if range.min_sample_rate().0 <= desired_sr && desired_sr <= range.max_sample_rate().0 {
                let cfg = range.with_sample_rate(SampleRate(desired_sr));
                best = Some(match best {
                    Some(b) if b.channels() <= cfg.channels() => b,
                    _ => cfg,
                });
            }
        }
        if let Some(b) = best { return Ok(b); }
    }
    dev.default_input_config().map_err(|e| format!("input config: {e}"))
}

/// Pick an output config at exactly `sr` (preferring the fewest channels),
/// falling back to the device default if `sr` is unsupported.
fn choose_output_config(dev: &cpal::Device, sr: u32) -> Result<SupportedStreamConfig, String> {
    if let Ok(ranges) = dev.supported_output_configs() {
        let mut best: Option<SupportedStreamConfig> = None;
        for range in ranges {
            if range.min_sample_rate().0 <= sr && sr <= range.max_sample_rate().0 {
                let cfg = range.with_sample_rate(SampleRate(sr));
                best = Some(match best {
                    Some(b) if b.channels() <= cfg.channels() => b,
                    _ => cfg,
                });
            }
        }
        if let Some(b) = best { return Ok(b); }
    }
    dev.default_output_config().map_err(|e| format!("output config: {e}"))
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

        // Negotiate a configuration both devices actually support. We prefer the
        // engine's requested rate but accept the device's native rate; effects
        // are sample-rate-aware, so we run the whole chain at `sr` and avoid a
        // resampler. Input/output must agree on `sr` (no cross-rate resampling).
        let in_cfg  = choose_input_config(&in_dev, sample_rate)?;
        let sr = in_cfg.sample_rate().0;
        let out_cfg = choose_output_config(&out_dev, sr)?;
        if out_cfg.sample_rate().0 != sr {
            return Err(format!(
                "input/output sample-rate mismatch ({} vs {} Hz); resampling not supported",
                sr, out_cfg.sample_rate().0));
        }
        let in_channels  = in_cfg.channels() as usize;
        let out_channels = out_cfg.channels() as usize;
        let in_stream_cfg: StreamConfig  = in_cfg.config();
        let out_stream_cfg: StreamConfig = out_cfg.config();

        let mic_rb = HeapRb::<f32>::new((sr as usize) / 10);
        let (mut mic_prod, mut mic_cons) = mic_rb.split();
        let raw_rb = HeapRb::<f32>::new((sr as usize) * 2);
        let (mut raw_prod, raw_cons) = raw_rb.split();
        let proc_rb = HeapRb::<f32>::new((sr as usize) * 2);
        let (mut proc_prod, proc_cons) = proc_rb.split();
        let det_rb = HeapRb::<f32>::new((sr as usize) / 4);
        let (mut det_prod, det_cons) = det_rb.split();
        let spec_rb = HeapRb::<f32>::new((sr as usize) / 2);
        let (mut spec_prod, spec_cons) = spec_rb.split();

        let shared_in = Arc::clone(shared);
        let is_rec = recording;

        shared.is_running.store(true, Ordering::Release);

        let input_stream = in_dev.build_input_stream(
            &in_stream_cfg,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if !shared_in.is_running.load(Ordering::Relaxed) { return; }
                // Downmix interleaved input to mono.
                let mut i = 0;
                while i + in_channels <= data.len() {
                    let mut acc = 0.0f32;
                    for c in 0..in_channels { acc += data[i + c]; }
                    let s = acc / in_channels as f32;
                    let _ = mic_prod.try_push(s);
                    if is_rec { let _ = raw_prod.try_push(s); }
                    let _ = det_prod.try_push(s);
                    i += in_channels;
                }
            },
            |err| log::error!("Input error: {}", err),
            None,
        ).map_err(|e| format!("Input stream: {}", e))?;

        let shared_out = Arc::clone(shared);
        // Reusable mono scratch — generously sized so resize() never allocates
        // inside the callback for any realistic host buffer.
        let mut mono: Vec<f32> = Vec::with_capacity(16384);
        let output_stream = out_dev.build_output_stream(
            &out_stream_cfg,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let frames = data.len() / out_channels;
                mono.resize(frames, 0.0);
                for f in 0..frames {
                    mono[f] = mic_cons.try_pop().unwrap_or(0.0);
                }
                let chain = shared_out.chain.load();
                for slot in chain.iter() {
                    // SAFETY: cpal audio thread is the unique writer.
                    unsafe { slot.process_in_place(&mut mono[..frames], sr); }
                }
                // Upmix mono → interleaved output channels.
                for f in 0..frames {
                    let v = mono[f];
                    for c in 0..out_channels { data[f * out_channels + c] = v; }
                }
                if is_rec {
                    for f in 0..frames { let _ = proc_prod.try_push(mono[f]); }
                }
                for f in 0..frames { let _ = spec_prod.try_push(mono[f]); }
                shared_out.rms_level.set(crate::util::rms(&mono[..frames]));
            },
            |err| log::error!("Output error: {}", err),
            None,
        ).map_err(|e| format!("Output stream: {}", e))?;

        input_stream.play().map_err(|e| format!("Play input: {}", e))?;
        output_stream.play().map_err(|e| format!("Play output: {}", e))?;

        if is_rec {
            if let Some(p) = raw_path  { self.raw_writer  = Some(spawn_writer(p, sr, raw_cons)); }
            if let Some(p) = proc_path { self.proc_writer = Some(spawn_writer(p, sr, proc_cons)); }
        }
        self.detector = Some(spawn_detector(sr, det_cons, Arc::clone(shared)));
        self.spectrum = Some(spawn_spectrum(sr, spec_cons, Arc::clone(shared)));

        self.input = Some(input_stream);
        self.output = Some(output_stream);
        Ok(())
    }

    fn preview_file(&mut self, input_path: &str, shared: &Arc<SharedState>) -> Reply {
        if shared.is_running.load(Ordering::Relaxed) {
            return Err("Engine already running".into());
        }
        let (samples, meta) = file_io::read_wav(input_path)?;
        let chunk = 512;
        let mut processed = Vec::with_capacity(samples.len());
        let mut pos = 0;
        let chain = shared.chain.load();
        while pos < samples.len() {
            let end = (pos + chunk).min(samples.len());
            let mut buf: Vec<f32> = samples[pos..end].to_vec();
            for slot in chain.iter() {
                if !slot.is_enabled() { continue; }
                let fx = unsafe { slot.as_mut() };
                fx.process(&mut buf, meta.sample_rate);
            }
            processed.extend_from_slice(&buf);
            pos = end;
        }

        let host = cpal::default_host();
        let dev = host.default_output_device().ok_or("No output device")?;
        let out_cfg = choose_output_config(&dev, meta.sample_rate)?;
        if out_cfg.sample_rate().0 != meta.sample_rate {
            return Err(format!(
                "output device does not support {} Hz (resampling not supported)",
                meta.sample_rate));
        }
        let out_channels = out_cfg.channels() as usize;
        let stream_cfg: StreamConfig = out_cfg.config();
        let data = Arc::new(processed);
        let pos_idx = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let pp = Arc::clone(&pos_idx);
        let dd = Arc::clone(&data);
        let shared_p = Arc::clone(shared);
        shared.is_running.store(true, Ordering::Release);

        let stream = dev.build_output_stream(
            &stream_cfg,
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let frames = out.len() / out_channels;
                let mut p = pp.load(Ordering::Relaxed);
                for f in 0..frames {
                    let v = if p < dd.len() {
                        let x = dd[p]; p += 1; x
                    } else {
                        shared_p.is_running.store(false, Ordering::Release);
                        0.0
                    };
                    for c in 0..out_channels { out[f * out_channels + c] = v; }
                }
                pp.store(p, Ordering::Relaxed);
            },
            |e| log::error!("Preview error: {}", e),
            None,
        ).map_err(|e| format!("Preview stream: {}", e))?;
        stream.play().map_err(|e| format!("Preview play: {}", e))?;
        self.output = Some(stream);
        Ok(())
    }

    fn stop(&mut self, shared: &Arc<SharedState>) {
        shared.is_running.store(false, Ordering::Release);
        self.input.take();
        self.output.take();
        if let Some(h) = self.raw_writer.take()  { h.join_blocking(); }
        if let Some(h) = self.proc_writer.take() { h.join_blocking(); }
        if let Some(h) = self.detector.take()    { h.join_blocking(); }
        if let Some(h) = self.spectrum.take()    { h.join_blocking(); }
    }
}

// ─── Workers ───

struct WriterHandle { stop: Arc<AtomicBool>, thread: Option<JoinHandle<()>> }

/// Generic stop+join handle for the detector / spectrum worker threads.
struct WorkerHandle { stop: Arc<AtomicBool>, thread: Option<JoinHandle<()>> }
impl WorkerHandle {
    fn join_blocking(mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() { let _ = t.join(); }
    }
}

fn spawn_writer(
    path: String, sample_rate: u32, mut cons: ringbuf::HeapCons<f32>,
) -> WriterHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = Arc::clone(&stop);
    let thread = thread::spawn(move || {
        let spec = hound::WavSpec {
            channels: 1, sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = match hound::WavWriter::create(&path, spec) {
            Ok(w) => w,
            Err(e) => { log::error!("WAV create {}: {}", path, e); return; }
        };
        let mut scratch = [0.0f32; 1024];
        loop {
            let n = cons.pop_slice(&mut scratch);
            if n > 0 {
                for &s in &scratch[..n] {
                    let v = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
                    let _ = writer.write_sample(v);
                }
            } else if stop_t.load(Ordering::Acquire) {
                while cons.occupied_len() > 0 {
                    if let Some(s) = cons.try_pop() {
                        let v = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
                        let _ = writer.write_sample(v);
                    } else { break; }
                }
                break;
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }
        if let Err(e) = writer.finalize() {
            log::error!("WAV finalize: {}", e);
        }
    });
    WriterHandle { stop, thread: Some(thread) }
}

impl WriterHandle {
    fn join_blocking(mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() { let _ = t.join(); }
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
