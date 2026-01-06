mod audio_capture;
mod model_manager;
mod transcriber;

pub use audio_capture::AudioCapture;
pub use model_manager::ensure_model;
pub use transcriber::Transcriber;

use std::sync::{Arc, Mutex};
use std::thread;

/// Shared state for background model loading and transcription
struct BackgroundState {
    transcriber: Option<Arc<Transcriber>>,
    loading: bool,
    error: Option<String>,
    // Transcription result from background thread
    transcription_result: Option<Result<String, String>>,
    transcribing: bool,
}

/// Voice recording and transcription state
pub struct VoiceState {
    pub recording: bool,
    pub press_time: Option<web_time::Instant>,
    pub audio_capture: AudioCapture,
    state: Arc<Mutex<BackgroundState>>,
}

impl VoiceState {
    /// Create a new VoiceState and start loading model in background
    pub fn new() -> Result<Self, String> {
        let audio_capture = AudioCapture::new().map_err(|e| e.to_string())?;

        let state = Arc::new(Mutex::new(BackgroundState {
            transcriber: None,
            loading: true,
            error: None,
            transcription_result: None,
            transcribing: false,
        }));

        // Start background model loading
        let state_clone = Arc::clone(&state);
        thread::spawn(move || {
            tracing::info!("Background: Starting Whisper model download/load...");

            match ensure_model("base.en") {
                Ok(model_path) => {
                    tracing::info!("Background: Model downloaded, loading...");
                    match Transcriber::new(&model_path) {
                        Ok(transcriber) => {
                            if let Ok(mut s) = state_clone.lock() {
                                s.transcriber = Some(Arc::new(transcriber));
                                s.loading = false;
                                tracing::info!("Background: Whisper model ready!");
                            }
                        }
                        Err(e) => {
                            if let Ok(mut s) = state_clone.lock() {
                                s.error = Some(format!("Failed to load model: {}", e));
                                s.loading = false;
                            }
                            tracing::error!("Background: Failed to load Whisper model: {}", e);
                        }
                    }
                }
                Err(e) => {
                    if let Ok(mut s) = state_clone.lock() {
                        s.error = Some(format!("Failed to download model: {}", e));
                        s.loading = false;
                    }
                    tracing::error!("Background: Failed to download Whisper model: {}", e);
                }
            }
        });

        Ok(Self {
            recording: false,
            press_time: None,
            audio_capture,
            state,
        })
    }

    /// Check if transcriber is ready
    pub fn is_ready(&self) -> bool {
        self.state
            .lock()
            .map(|s| s.transcriber.is_some())
            .unwrap_or(false)
    }

    /// Check if model is still loading
    pub fn is_loading(&self) -> bool {
        self.state
            .lock()
            .map(|s| s.loading)
            .unwrap_or(false)
    }

    /// Check if transcription is in progress
    pub fn is_transcribing(&self) -> bool {
        self.state
            .lock()
            .map(|s| s.transcribing)
            .unwrap_or(false)
    }

    /// Get loading status message
    pub fn status_message(&self) -> Option<String> {
        if let Ok(s) = self.state.lock() {
            if s.loading {
                return Some("Loading voice model...".to_string());
            }
            if s.transcribing {
                return Some("Transcribing...".to_string());
            }
            if let Some(ref err) = s.error {
                return Some(format!("Voice error: {}", err));
            }
            if s.transcriber.is_some() {
                return None; // Ready, no message needed
            }
        }
        None
    }

    /// Check for and take transcription result (call this from main loop)
    pub fn take_transcription_result(&mut self) -> Option<Result<String, String>> {
        if let Ok(mut s) = self.state.lock() {
            s.transcription_result.take()
        } else {
            None
        }
    }

    /// Start recording audio
    pub fn start_recording(&mut self) -> Result<(), String> {
        if self.recording {
            return Ok(());
        }

        // Check if model is ready
        if !self.is_ready() {
            if self.is_loading() {
                return Err("Voice model still loading...".to_string());
            } else {
                return Err("Voice model not available".to_string());
            }
        }

        self.recording = true;
        self.press_time = Some(web_time::Instant::now());
        self.audio_capture.start()
    }

    /// Stop recording and start background transcription
    /// Returns Ok(true) if transcription started, Ok(false) if discarded (quick tap)
    pub fn stop_recording(&mut self) -> Result<bool, String> {
        if !self.recording {
            return Ok(false);
        }

        self.recording = false;

        // Check hold duration
        let held_ms = self
            .press_time
            .map(|t| t.elapsed().as_millis())
            .unwrap_or(0);

        let audio = self.audio_capture.stop();

        // Quick tap (<200ms) - discard
        if held_ms < 200 {
            tracing::debug!("Quick tap ({}ms) - discarding", held_ms);
            return Ok(false);
        }

        // Get audio data
        let audio = match audio {
            Some(a) if !a.is_empty() => a,
            _ => {
                tracing::debug!("No audio data captured");
                return Ok(false);
            }
        };

        tracing::info!(
            "Captured {} samples ({:.1}s)",
            audio.len(),
            audio.len() as f32 / 16000.0
        );

        // Validate audio
        if !Self::validate_audio(&audio) {
            tracing::debug!("Audio validation failed");
            return Ok(false);
        }

        // Get transcriber reference
        let transcriber = {
            let s = self.state.lock().map_err(|e| e.to_string())?;
            s.transcriber.clone().ok_or("Transcriber not ready")?
        };

        // Mark as transcribing
        {
            let mut s = self.state.lock().map_err(|e| e.to_string())?;
            s.transcribing = true;
        }

        // Start background transcription
        let state_clone = Arc::clone(&self.state);
        thread::spawn(move || {
            tracing::info!("Background: Starting transcription...");
            let result = transcriber.transcribe(&audio);

            if let Ok(mut s) = state_clone.lock() {
                s.transcribing = false;
                s.transcription_result = Some(result);
            }
            tracing::info!("Background: Transcription complete");
        });

        Ok(true)
    }

    /// Validate audio data before transcription
    fn validate_audio(audio: &[f32]) -> bool {
        // Minimum duration: 0.5 seconds at 16kHz
        if audio.len() < 8000 {
            tracing::debug!("Audio too short: {} samples", audio.len());
            return false;
        }

        // Check for silence (RMS level)
        let rms: f32 = (audio.iter().map(|x| x * x).sum::<f32>() / audio.len() as f32).sqrt();
        if rms < 1e-6 {
            tracing::debug!("Audio too quiet: RMS = {}", rms);
            return false;
        }

        // Check for NaN/inf
        if audio.iter().any(|x| x.is_nan() || x.is_infinite()) {
            tracing::debug!("Audio contains NaN/inf values");
            return false;
        }

        true
    }
}

impl Default for VoiceState {
    fn default() -> Self {
        Self::new().expect("Failed to initialize voice state")
    }
}
