//! Native device adapter for the owned engine mixer.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use glam::Vec3;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use verse_engine::{
    audio::{Clip, Emitter, Mixer},
    core::LifeId,
};

enum Command {
    Listener(Vec3, Vec3),
    Play(Clip, Emitter),
    StopLife(LifeId),
}
#[derive(Default)]
struct Counters {
    frames: AtomicU64,
    errors: AtomicU64,
    refused: AtomicU64,
    nonzero: AtomicU64,
    peak: AtomicU64,
    capture_dropped: AtomicU64,
}
/// Owning this value retains the stream. Dropping it closes device output.
pub struct Output {
    stream: Option<cpal::Stream>,
    capture_worker: Option<std::thread::JoinHandle<()>>,
    sender: SyncSender<Command>,
    counters: Arc<Counters>,
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
        let mixer = Mixer::new(rate)?;
        let (sender, receiver) = mpsc::sync_channel(128);
        let counters = Arc::new(Counters::default());
        let (capture, capture_worker) = capture(rate, counters.clone())?;
        let stream = match format {
            cpal::SampleFormat::F32 => {
                build::<f32>(&device, &config, mixer, receiver, counters.clone(), capture)
            }
            cpal::SampleFormat::I16 => {
                build::<i16>(&device, &config, mixer, receiver, counters.clone(), capture)
            }
            cpal::SampleFormat::U16 => {
                build::<u16>(&device, &config, mixer, receiver, counters.clone(), capture)
            }
            _ => return Err("Unsupported audio output sample format".into()),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            stream: Some(stream),
            capture_worker,
            sender,
            counters,
            rate,
        })
    }
    pub fn rate(&self) -> u32 {
        self.rate
    }
    pub fn stats(&self) -> Stats {
        Stats {
            rendered_frames: self.counters.frames.load(Ordering::Relaxed),
            device_errors: self.counters.errors.load(Ordering::Relaxed),
            refused_commands: self.counters.refused.load(Ordering::Relaxed),
            nonzero_samples: self.counters.nonzero.load(Ordering::Relaxed),
            peak: f32::from_bits(self.counters.peak.load(Ordering::Relaxed) as u32),
            capture_dropped_blocks: self.counters.capture_dropped.load(Ordering::Relaxed),
        }
    }
    fn send(&self, command: Command) -> Result<(), String> {
        self.sender.try_send(command).map_err(|_| {
            self.counters.refused.fetch_add(1, Ordering::Relaxed);
            "Audio command queue is full or disconnected".into()
        })
    }
    pub fn listener(&self, position: Vec3, right: Vec3) -> Result<(), String> {
        self.send(Command::Listener(position, right))
    }
    pub fn play(&self, clip: &Clip, emitter: Emitter) -> Result<(), String> {
        self.send(Command::Play(clip.clone(), emitter))
    }
    pub fn stop_life(&self, life: LifeId) -> Result<(), String> {
        self.send(Command::StopLife(life))
    }
}
fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut mixer: Mixer,
    receiver: Receiver<Command>,
    counters: Arc<Counters>,
    capture: Option<SyncSender<AudioBlock>>,
) -> Result<cpal::Stream, String> {
    let channels = usize::from(config.channels);
    let errors = counters.clone();
    let mut scratch = [0.; 2048];
    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _| {
                // Limit command work per callback. Queue storage and PCM are prepared elsewhere.
                for _ in 0..64 {
                    let Ok(command) = receiver.try_recv() else {
                        break;
                    };
                    let result = match command {
                        Command::Listener(position, right) => mixer.set_listener(position, right),
                        Command::Play(clip, emitter) => mixer.play(&clip, emitter).map(|_| ()),
                        Command::StopLife(life) => {
                            mixer.stop_life(life);
                            Ok(())
                        }
                    };
                    if result.is_err() {
                        counters.refused.fetch_add(1, Ordering::Relaxed);
                    }
                }
                for block in data.chunks_mut(channels * 1024) {
                    let frames = block.len() / channels;
                    let pcm = &mut scratch[..frames * 2];
                    if mixer.render(pcm).is_err() {
                        pcm.fill(0.);
                        counters.errors.fetch_add(1, Ordering::Relaxed);
                    }
                    let nonzero = pcm.iter().filter(|sample| sample.abs() > 0.00001).count();
                    let peak = pcm
                        .iter()
                        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
                    counters
                        .nonzero
                        .fetch_add(nonzero as u64, Ordering::Relaxed);
                    counters
                        .peak
                        .fetch_max(u64::from(peak.to_bits()), Ordering::Relaxed);
                    if let Some(sender) = &capture {
                        let mut block = AudioBlock {
                            samples: [0.; 2048],
                            len: pcm.len(),
                        };
                        block.samples[..pcm.len()].copy_from_slice(pcm);
                        if sender.try_send(block).is_err() {
                            counters.capture_dropped.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    for (frame, source) in block.chunks_exact_mut(channels).zip(pcm.chunks_exact(2))
                    {
                        for (channel, sample) in frame.iter_mut().enumerate() {
                            let value = if channels == 1 {
                                (source[0] + source[1]) * 0.5
                            } else if channel < 2 {
                                source[channel]
                            } else {
                                0.
                            };
                            *sample = T::from_sample(value);
                        }
                    }
                    counters.frames.fetch_add(frames as u64, Ordering::Relaxed);
                }
            },
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
        if let Some(worker) = self.capture_worker.take() {
            let _ = worker.join();
        }
    }
}
struct AudioBlock {
    samples: [f32; 2048],
    len: usize,
}
type Capture = (
    Option<SyncSender<AudioBlock>>,
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
    let (sender, receiver) = mpsc::sync_channel::<AudioBlock>(64);
    let worker = std::thread::spawn(move || {
        let result = (|| -> std::io::Result<()> {
            let mut bytes = 0u32;
            let limit = rate * 8 * 300;
            let mut encoded = [0u8; 8192];
            while let Ok(block) = receiver.recv() {
                if bytes + block.len as u32 * 4 > limit {
                    continue;
                }
                for (sample, output) in block.samples[..block.len]
                    .iter()
                    .zip(encoded.chunks_exact_mut(4))
                {
                    output.copy_from_slice(&sample.to_le_bytes());
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
