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
}
/// Owning this value retains the stream. Dropping it closes device output.
pub struct Output {
    _stream: cpal::Stream,
    sender: SyncSender<Command>,
    counters: Arc<Counters>,
    rate: u32,
}
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Stats {
    pub rendered_frames: u64,
    pub device_errors: u64,
    pub refused_commands: u64,
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
        let stream = match format {
            cpal::SampleFormat::F32 => {
                build::<f32>(&device, &config, mixer, receiver, counters.clone())
            }
            cpal::SampleFormat::I16 => {
                build::<i16>(&device, &config, mixer, receiver, counters.clone())
            }
            cpal::SampleFormat::U16 => {
                build::<u16>(&device, &config, mixer, receiver, counters.clone())
            }
            _ => return Err("Unsupported audio output sample format".into()),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self {
            _stream: stream,
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
