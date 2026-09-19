use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use arc_swap::ArcSwap;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleRate, StreamConfig, SupportedStreamConfig};
use ringbuf::{HeapRb, traits::{Producer, Consumer, Split, Observer}};

#[allow(unused_imports)]
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

        if sr != 48000 {
            log::warn!(
                "negotiated {sr} Hz: noise suppression (RNNoise) requires 48 kHz \
                 and will pass audio through unchanged at this rate");
        }

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
        // Input and output devices run on independent clocks, so their rates
        // differ by up to a few hundred ppm even at the same nominal Hz. Left
        // uncompensated, the mic ring slowly fills (input faster → dropped
        // chunks + creeping latency) or starves (output faster → zero-fill
        // clicks). Watermark correction: above `drift_high`, shed one sample
        // per callback; on starvation, hold the last sample with a fast decay
        // instead of a hard zero. One sample per callback absorbs ~4000 ppm —
        // far beyond real-world drift.
        let drift_high = (sr as usize) / 20; // 50 ms
        let mut last_in = 0.0f32;
        let output_stream = out_dev.build_output_stream(
            &out_stream_cfg,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let frames = data.len() / out_channels;
                mono.resize(frames, 0.0);
                if mic_cons.occupied_len() > drift_high {
                    let _ = mic_cons.try_pop();
                }
                for f in 0..frames {
                    match mic_cons.try_pop() {
                        Some(s) => { last_in = s; mono[f] = s; }
                        None => { last_in *= 0.995; mono[f] = last_in; }
                    }
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

        // Open both files before starting capture. Failure must reach the caller,
        // not disappear inside a writer thread after start() reported success.
        let raw_file = if recording { raw_path.as_deref().map(|p| create_writer(p, sr)).transpose()? } else { None };
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
        // Mono chain — downmix multi-channel input (playback upmixes again).
        let (samples, meta) = file_io::read_wav_mono(input_path)?;
        let chain = shared.chain.load();
        // SAFETY: the control loop holds operation_lock and no live DSP runs.
        let processed = unsafe { render_samples(&samples, meta.sample_rate, &chain, |_| {}) };

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
        loop {
            let n = cons.pop_slice(&mut scratch);
            if n > 0 {
                for &s in &scratch[..n] {
                    let v = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
                    writer.write_sample(v).map_err(|e| format!("WAV write: {e}"))?;
                }
            } else if stop_t.load(Ordering::Acquire) {
                while cons.occupied_len() > 0 {
                    if let Some(s) = cons.try_pop() {
                        let v = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
                        writer.write_sample(v).map_err(|e| format!("WAV write: {e}"))?;
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
