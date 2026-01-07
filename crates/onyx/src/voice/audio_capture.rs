use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleRate, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Whisper's expected sample rate
const WHISPER_SAMPLE_RATE: u32 = 16000;

/// Audio capture using cpal
pub struct AudioCapture {
    device: Device,
    config: StreamConfig,
    buffer: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<AtomicBool>,
    stream: Option<Stream>,
    device_sample_rate: u32,
}

impl AudioCapture {
    /// Create a new AudioCapture instance
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();

        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        // Get supported config - use device's preferred config
        let supported_config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {}", e))?;

        tracing::info!(
            "Audio device: {:?}, sample rate: {}, channels: {}",
            device.name().unwrap_or_default(),
            supported_config.sample_rate().0,
            supported_config.channels()
        );

        let device_sample_rate = supported_config.sample_rate().0;

        // Use device's native config (mono if possible, otherwise we'll convert)
        let config = StreamConfig {
            channels: 1, // Request mono
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        Ok(Self {
            device,
            config,
            buffer: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(AtomicBool::new(false)),
            stream: None,
            device_sample_rate,
        })
    }

    /// Start recording audio
    pub fn start(&mut self) -> Result<(), String> {
        let already_recording = self.is_recording.load(Ordering::SeqCst);
        tracing::info!("AudioCapture::start() called, already_recording={}", already_recording);

        if already_recording {
            tracing::debug!("Already recording, returning early");
            return Ok(());
        }

        // Clear buffer
        if let Ok(mut buffer) = self.buffer.lock() {
            let old_len = buffer.len();
            buffer.clear();
            tracing::debug!("Cleared audio buffer (had {} samples)", old_len);
        } else {
            tracing::error!("Failed to lock buffer for clearing");
            return Err("Failed to lock audio buffer".to_string());
        }

        let buffer = Arc::clone(&self.buffer);
        let is_recording = Arc::clone(&self.is_recording);

        let err_fn = |err| {
            tracing::error!("Audio capture error: {}", err);
        };

        // Try mono first, fall back to stereo if needed
        let stream = self
            .device
            .build_input_stream(&self.config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if is_recording.load(Ordering::SeqCst) {
                        if let Ok(mut buffer) = buffer.lock() {
                            buffer.extend_from_slice(data);
                        }
                    }
                },
                err_fn,
                None
            )
            .or_else(|_| {
                // Try stereo config
                tracing::info!("Mono not supported, trying stereo...");
                let stereo_config = StreamConfig {
                    channels: 2,
                    sample_rate: SampleRate(self.device_sample_rate),
                    buffer_size: cpal::BufferSize::Default,
                };

                let buffer = Arc::clone(&self.buffer);
                let is_recording = Arc::clone(&self.is_recording);

                self.device.build_input_stream(&stereo_config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if is_recording.load(Ordering::SeqCst) {
                            if let Ok(mut buffer) = buffer.lock() {
                                // Convert stereo to mono by averaging channels
                                for chunk in data.chunks(2) {
                                    if chunk.len() == 2 {
                                        buffer.push((chunk[0] + chunk[1]) / 2.0);
                                    }
                                }
                            }
                        }
                    },
                    err_fn,
                    None
                )
            })
            .map_err(|e| format!("Failed to build input stream: {}", e))?;

        tracing::info!("Built audio stream, attempting to play...");

        stream
            .play()
            .map_err(|e| format!("Failed to play stream: {}", e))?;

        self.is_recording.store(true, Ordering::SeqCst);
        self.stream = Some(stream);

        tracing::info!("Voice recording started at {}Hz, is_recording={}",
            self.device_sample_rate,
            self.is_recording.load(Ordering::SeqCst));
        Ok(())
    }

    /// Stop recording and return the audio data (resampled to 16kHz for Whisper)
    pub fn stop(&mut self) -> Option<Vec<f32>> {
        let was_recording = self.is_recording.load(Ordering::SeqCst);
        tracing::info!("AudioCapture::stop() called, was_recording={}", was_recording);

        if !was_recording {
            tracing::warn!("AudioCapture::stop() called but not recording");
            return None;
        }

        self.is_recording.store(false, Ordering::SeqCst);
        tracing::debug!("Set is_recording to false");

        // Drop stream to stop recording
        let had_stream = self.stream.is_some();
        self.stream = None;
        tracing::debug!("Dropped stream (had_stream={})", had_stream);

        // Get audio data
        let audio = match self.buffer.lock() {
            Ok(b) => {
                let data = b.clone();
                tracing::info!("Got {} samples from buffer", data.len());
                data
            }
            Err(e) => {
                tracing::error!("Failed to lock buffer in stop(): {}", e);
                return None;
            }
        };

        if audio.is_empty() {
            tracing::warn!("No audio data captured (buffer was empty)");
            return None;
        }

        tracing::info!(
            "Voice recording stopped: {} samples at {}Hz ({:.2}s of audio)",
            audio.len(),
            self.device_sample_rate,
            audio.len() as f32 / self.device_sample_rate as f32
        );

        // Resample to 16kHz if needed
        if self.device_sample_rate != WHISPER_SAMPLE_RATE {
            let resampled = resample(&audio, self.device_sample_rate, WHISPER_SAMPLE_RATE);
            tracing::info!("Resampled to {} samples at 16kHz", resampled.len());
            Some(resampled)
        } else {
            Some(audio)
        }
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }
}

/// Simple linear interpolation resampler
fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return input.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (input.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_idx = i as f64 * ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(input.len() - 1);
        let frac = src_idx - idx_floor as f64;

        let sample = if idx_floor < input.len() {
            input[idx_floor] * (1.0 - frac as f32) + input[idx_ceil] * frac as f32
        } else {
            0.0
        };
        output.push(sample);
    }

    output
}
