//! Native output with prepared commands and off-callback resource reclamation.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use glam::Vec3;
use rtrb::{Consumer, Producer, PushError, RingBuffer};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use verse_engine::{
    audio::{Bus, Clip, Emitter, Mixer, Prepared, Retired},
    core::LifeId,
};

enum Command {
    Listener(Vec3, Vec3),
    Play(Prepared),
    StopLife(LifeId),
    StopBus(Bus),
}
#[derive(Default)]
struct Counters {
    frames: AtomicU64,
    errors: AtomicU64,
    refused: AtomicU64,
    nonzero: AtomicU64,
    peak: AtomicU64,
    capture_dropped: AtomicU64,
    callbacks: AtomicU64,
    callback_max_ns: AtomicU64,
    deadline_overruns: AtomicU64,
    stream_gaps: AtomicU64,
    virtual_voices: AtomicU64,
    retirement_blocked: AtomicU64,
}
struct Controls {
    suspended: AtomicBool,
    master: AtomicU64,
    buses: [AtomicU64; 4],
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            suspended: AtomicBool::new(false),
            master: AtomicU64::new(1f32.to_bits().into()),
            buses: std::array::from_fn(|_| AtomicU64::new(1f32.to_bits().into())),
        }
    }
}
/// Control-side locks serialize producers. The callback owns only consumers.
pub struct Output {
    stream: Option<cpal::Stream>,
    senders: Mutex<Option<(Producer<Command>, Producer<Command>)>>,
    workers: Mutex<Vec<std::thread::JoinHandle<()>>>,
    counters: Arc<Counters>,
    controls: Arc<Controls>,
    rate: u32,
}
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Stats {
    pub rendered_frames: u64,
    pub device_errors: u64,
    pub refused_commands: u64,
    pub nonzero_samples: u64,
    pub peak: f32,
    pub capture_dropped_blocks: u64,
    pub callbacks: u64,
    pub callback_max_ns: u64,
    pub deadline_overruns: u64,
    pub stream_gaps: u64,
    pub virtual_voices: u64,
    pub retirement_blocked: u64,
}
impl Output {
    pub fn open() -> Result<Self, String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No default audio output device")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let rate = supported.sample_rate();
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        if !(1..=8).contains(&config.channels) {
            return Err("Unsupported audio output channel count".into());
        }
        let (ordinary, ordinary_rx) = RingBuffer::new(128);
        let (critical, critical_rx) = RingBuffer::new(16);
        let (gc, mut retired) = RingBuffer::<Retired>::new(256);
        let reclaimer = std::thread::spawn(move || {
            loop {
                while let Ok(value) = retired.pop() {
                    drop(value);
                }
                if retired.is_abandoned() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            while let Ok(value) = retired.pop() {
                drop(value);
            }
        });
        let counters = Arc::new(Counters::default());
        let controls = Arc::new(Controls::default());
        let (capture, capture_worker) = capture(rate, counters.clone())?;
        let callback = Callback {
            mixer: Mixer::new(rate)?,
            ordinary: ordinary_rx,
            critical: critical_rx,
            gc,
            counters: counters.clone(),
            controls: controls.clone(),
            capture,
            rate,
        };
        let stream = match format {
            cpal::SampleFormat::F32 => build::<f32>(&device, &config, callback),
            cpal::SampleFormat::I16 => build::<i16>(&device, &config, callback),
            cpal::SampleFormat::U16 => build::<u16>(&device, &config, callback),
            _ => return Err("Unsupported audio output sample format".into()),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        let mut workers = vec![reclaimer];
        if let Some(worker) = capture_worker {
            workers.push(worker);
        }
        Ok(Self {
            stream: Some(stream),
            senders: Mutex::new(Some((ordinary, critical))),
            workers: Mutex::new(workers),
            counters,
            controls,
            rate,
        })
    }
    pub fn rate(&self) -> u32 {
        self.rate
    }
    pub fn stats(&self) -> Stats {
        let c = &self.counters;
        let read = |a: &AtomicU64| a.load(Ordering::Relaxed);
        Stats {
            rendered_frames: read(&c.frames),
            device_errors: read(&c.errors),
            refused_commands: read(&c.refused),
            nonzero_samples: read(&c.nonzero),
            peak: f32::from_bits(read(&c.peak) as u32),
            capture_dropped_blocks: read(&c.capture_dropped),
            callbacks: read(&c.callbacks),
            callback_max_ns: read(&c.callback_max_ns),
            deadline_overruns: read(&c.deadline_overruns),
            stream_gaps: read(&c.stream_gaps),
            virtual_voices: read(&c.virtual_voices),
            retirement_blocked: read(&c.retirement_blocked),
        }
    }
    fn send(&self, command: Command, protected: bool) -> Result<(), String> {
        let mut senders = self
            .senders
            .lock()
            .map_err(|_| "Audio control lock failed")?;
        let (ordinary, critical) = senders.as_mut().ok_or("Audio output is closed")?;
        let queue = if protected { critical } else { ordinary };
        if queue.is_abandoned() || queue.push(command).is_err() {
            self.counters.refused.fetch_add(1, Ordering::Relaxed);
            return Err("Audio command queue is full or disconnected".into());
        }
        Ok(())
    }
    pub fn listener(&self, position: Vec3, right: Vec3) -> Result<(), String> {
        // Validation occurs before the callback receives the pose.
        if !position.is_finite()
            || position.abs().max_element() > 1_000_000.
            || !right.is_finite()
            || !right.length_squared().is_finite()
            || right.length_squared() < 1e-8
        {
            return Err("Invalid audio listener pose".into());
        }
        self.send(Command::Listener(position, right), false)
    }
    pub fn play(&self, clip: &Clip, emitter: Emitter) -> Result<(), String> {
        self.play_prepared(Prepared::clip(clip.clone(), emitter, Bus::Effects, 0, 0)?)
    }
    pub fn play_prepared(&self, prepared: Prepared) -> Result<(), String> {
        let protected = prepared.priority() >= 128 || prepared.bus() != Bus::Effects;
        if prepared.bus() == Bus::Music {
            let (prepared, mut feeder) = prepared.into_streaming(8192)?;
            while feeder.pump() > 0 {}
            // Failed admission drops the stream on this control thread.
            self.send(Command::Play(prepared), protected)?;
            let mut workers = self
                .workers
                .lock()
                .map_err(|_| "Audio worker lock failed")?;
            let mut i = 0;
            while i < workers.len() {
                if workers[i].is_finished() {
                    let worker = workers.swap_remove(i);
                    let _ = worker.join();
                } else {
                    i += 1;
                }
            }
            workers.push(std::thread::spawn(move || {
                while !feeder.closed() {
                    feeder.pump();
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }));
            Ok(())
        } else {
            self.send(Command::Play(prepared), protected)
        }
    }
    pub fn stop_life(&self, life: LifeId) -> Result<(), String> {
        self.send(Command::StopLife(life), true)
    }
    pub fn stop_bus(&self, bus: Bus) -> Result<(), String> {
        self.send(Command::StopBus(bus), true)
    }
    pub fn suspend(&self, suspended: bool) {
        self.controls.suspended.store(suspended, Ordering::Release);
    }
    pub fn volume(&self, master: f32, buses: [f32; 4]) -> Result<(), String> {
        if std::iter::once(master)
            .chain(buses)
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        {
            return Err("Invalid audio volume".into());
        }
        self.controls
            .master
            .store(master.to_bits().into(), Ordering::Release);
        for (slot, value) in self.controls.buses.iter().zip(buses) {
            slot.store(value.to_bits().into(), Ordering::Release);
        }
        Ok(())
    }
}
struct Callback {
    mixer: Mixer,
    ordinary: Consumer<Command>,
    critical: Consumer<Command>,
    gc: Producer<Retired>,
    counters: Arc<Counters>,
    controls: Arc<Controls>,
    capture: Option<Producer<AudioBlock>>,
    rate: u32,
}
impl Callback {
    fn reclaim(&mut self) {
        for _ in 0..128 {
            let Some(retired) = self.mixer.pop_retired() else {
                break;
            };
            if let Err(PushError::Full(retired)) = self.gc.push(retired) {
                self.mixer.return_retired(retired);
                break;
            }
        }
    }
    fn commands(&mut self) {
        self.reclaim();
        self.mixer
            .suspend(self.controls.suspended.load(Ordering::Acquire));
        let _ = self.mixer.master(f32::from_bits(
            self.controls.master.load(Ordering::Acquire) as u32
        ));
        for (bus, value) in [Bus::Effects, Bus::Music, Bus::Dialogue, Bus::Interface]
            .into_iter()
            .zip(&self.controls.buses)
        {
            let _ = self
                .mixer
                .volume(bus, f32::from_bits(value.load(Ordering::Acquire) as u32));
        }
        for _ in 0..64 {
            // Reserve room for both a displaced source and a refused command.
            if self.mixer.retirement_slots() < 2 {
                break;
            }
            let command = self.critical.pop().or_else(|_| self.ordinary.pop());
            let Ok(command) = command else {
                break;
            };
            match command {
                Command::Listener(position, right) => {
                    let _ = self.mixer.set_listener(position, right);
                }
                Command::Play(prepared) => {
                    if let Err((_, prepared)) = self.mixer.play_rt(prepared) {
                        self.counters.refused.fetch_add(1, Ordering::Relaxed);
                        if self.mixer.retire_prepared(prepared).is_err() {
                            unreachable!("Reserved retirement capacity");
                        }
                    }
                }
                Command::StopLife(life) => self.mixer.stop_life(life),
                Command::StopBus(bus) => self.mixer.stop_bus(bus),
            }
        }
    }
    fn render<T: cpal::SizedSample + cpal::FromSample<f32>>(
        &mut self,
        data: &mut [T],
        channels: usize,
    ) {
        let started = std::time::Instant::now();
        self.commands();
        let mut scratch = [0.; 2048];
        for block in data.chunks_mut(channels * 1024) {
            let frames = block.len() / channels;
            let pcm = &mut scratch[..frames * 2];
            if self.mixer.render_rt(pcm).is_err() {
                pcm.fill(0.);
                self.counters.errors.fetch_add(1, Ordering::Relaxed);
            }
            let nonzero = pcm.iter().filter(|s| s.abs() > 0.00001).count();
            let peak = pcm.iter().fold(0f32, |p, s| p.max(s.abs()));
            self.counters
                .nonzero
                .fetch_add(nonzero as u64, Ordering::Relaxed);
            self.counters
                .peak
                .fetch_max(peak.to_bits().into(), Ordering::Relaxed);
            if let Some(sender) = &mut self.capture {
                let mut captured = AudioBlock {
                    samples: [0.; 2048],
                    len: pcm.len(),
                };
                captured.samples[..pcm.len()].copy_from_slice(pcm);
                if sender.push(captured).is_err() {
                    self.counters
                        .capture_dropped
                        .fetch_add(1, Ordering::Relaxed);
                }
            }
            for (frame, source) in block.chunks_exact_mut(channels).zip(pcm.chunks_exact(2)) {
                for (channel, sample) in frame.iter_mut().enumerate() {
                    *sample = T::from_sample(if channels == 1 {
                        (source[0] + source[1]) * 0.5
                    } else if channel < 2 {
                        source[channel]
                    } else {
                        0.
                    });
                }
            }
            // A malformed trailing partial frame must not expose old device data.
            for sample in &mut block[frames * channels..] {
                *sample = T::from_sample(0.);
            }
            self.counters
                .frames
                .fetch_add(frames as u64, Ordering::Relaxed);
            self.reclaim();
        }
        let ns = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        self.counters.callbacks.fetch_add(1, Ordering::Relaxed);
        self.counters
            .callback_max_ns
            .fetch_max(ns, Ordering::Relaxed);
        let budget = (data.len() / channels) as u64 * 1_000_000_000 / u64::from(self.rate);
        if ns > budget {
            self.counters
                .deadline_overruns
                .fetch_add(1, Ordering::Relaxed);
        }
        self.counters
            .stream_gaps
            .store(self.mixer.stats.stream_gaps, Ordering::Relaxed);
        self.counters
            .virtual_voices
            .store(self.mixer.stats.virtual_voices as u64, Ordering::Relaxed);
        self.counters
            .retirement_blocked
            .store(self.mixer.stats.retirement_blocked, Ordering::Relaxed);
    }
}
fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut callback: Callback,
) -> Result<cpal::Stream, String> {
    let channels = usize::from(config.channels);
    let errors = callback.counters.clone();
    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _| callback.render(data, channels),
            move |_| {
                errors.errors.fetch_add(1, Ordering::Relaxed);
            },
            Some(std::time::Duration::from_secs(2)),
        )
        .map_err(|e| e.to_string())
}
impl Drop for Output {
    fn drop(&mut self) {
        self.stream.take();
        self.senders
            .get_mut()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        for worker in self
            .workers
            .get_mut()
            .unwrap_or_else(|p| p.into_inner())
            .drain(..)
        {
            let _ = worker.join();
        }
    }
}
struct AudioBlock {
    samples: [f32; 2048],
    len: usize,
}
type Capture = (
    Option<Producer<AudioBlock>>,
    Option<std::thread::JoinHandle<()>>,
);
fn capture(rate: u32, counters: Arc<Counters>) -> Result<Capture, String> {
    use std::io::{Seek, SeekFrom, Write};
    let Some(path) = std::env::var_os("VERSE_AUDIO_CAPTURE") else {
        return Ok((None, None));
    };
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let header = move |bytes: u32| {
        let mut data = Vec::with_capacity(44);
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&(bytes + 36).to_le_bytes());
        data.extend_from_slice(b"WAVEfmt ");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&3u16.to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&rate.to_le_bytes());
        data.extend_from_slice(&(rate * 8).to_le_bytes());
        data.extend_from_slice(&8u16.to_le_bytes());
        data.extend_from_slice(&32u16.to_le_bytes());
        data.extend_from_slice(b"data");
        data.extend_from_slice(&bytes.to_le_bytes());
        data
    };
    file.write_all(&header(0)).map_err(|e| e.to_string())?;
    let (sender, mut receiver) = RingBuffer::<AudioBlock>::new(64);
    let worker = std::thread::spawn(move || {
        let result = (|| -> std::io::Result<()> {
            let mut bytes = 0u32;
            let limit = rate * 8 * 300;
            let mut encoded = [0u8; 8192];
            loop {
                let block = match receiver.pop() {
                    Ok(block) => block,
                    Err(_) if receiver.is_abandoned() => break,
                    Err(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        continue;
                    }
                };
                if bytes + block.len as u32 * 4 > limit {
                    continue;
                }
                for (sample, out) in block.samples[..block.len]
                    .iter()
                    .zip(encoded.chunks_exact_mut(4))
                {
                    out.copy_from_slice(&sample.to_le_bytes());
                }
                file.write_all(&encoded[..block.len * 4])?;
                bytes += block.len as u32 * 4;
            }
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&header(bytes))?;
            file.flush()
        })();
        if result.is_err() {
            counters.errors.fetch_add(1, Ordering::Relaxed);
        }
    });
    Ok((Some(sender), Some(worker)))
}
