use crate::audio_capture::AudioCapture;
use crate::error::VoiceError;
use crate::model_manager::ensure_model;
use crate::transcriber::Transcriber;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

/// Configuration for voice session
#[derive(Clone, Debug)]
pub struct VoiceConfig {
    /// Model name (e.g., "base.en", "small.en")
    pub model: String,
    /// Language code (e.g., "en") or None for auto-detect
    pub language: Option<String>,
    /// Minimum hold time in milliseconds (default 200ms)
    pub min_hold_ms: u64,
    /// Minimum audio samples at 16kHz (default 8000 = 0.5s)
    pub min_audio_samples: usize,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            model: "base.en".to_string(),
            language: Some("en".to_string()),
            min_hold_ms: 200,
            min_audio_samples: 8000,
        }
    }
}

/// Events emitted by VoiceSession
#[derive(Debug, Clone)]
pub enum VoiceEvent {
    /// Model download/load started
    ModelLoading,
    /// Model is ready for use
    ModelReady,
    /// Model failed to load
    ModelError(String),
    /// Recording started
    RecordingStarted,
    /// Recording stopped, transcription started
    TranscriptionStarted,
    /// Recording was discarded (quick tap or no audio)
    RecordingDiscarded { reason: String },
    /// Transcription completed successfully
    TranscriptionComplete { text: String },
    /// Transcription failed
    TranscriptionError(String),
}

/// Internal state shared across threads
struct SessionState {
    transcriber: Option<Arc<Transcriber>>,
    loading: bool,
    error: Option<String>,
    transcribing: bool,
}

/// Voice recording and transcription session
pub struct VoiceSession {
    config: VoiceConfig,
    audio_capture: AudioCapture,
    state: Arc<Mutex<SessionState>>,
    recording: bool,
    press_time: Option<Instant>,
    event_callback: Option<Arc<dyn Fn(VoiceEvent) + Send + Sync + 'static>>,
}

impl VoiceSession {
    /// Create a new voice session with default config
    pub fn new() -> Result<Self, VoiceError> {
        Self::with_config(VoiceConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(config: VoiceConfig) -> Result<Self, VoiceError> {
        let audio_capture = AudioCapture::new().map_err(VoiceError::AudioInit)?;

        let state = Arc::new(Mutex::new(SessionState {
            transcriber: None,
            loading: true,
            error: None,
            transcribing: false,
        }));

        let mut session = Self {
            config,
            audio_capture,
            state,
            recording: false,
            press_time: None,
            event_callback: None,
        };

        // Start background model loading
        session.start_model_loading();

        Ok(session)
    }

    /// Set callback for voice events
    pub fn on_event<F>(&mut self, callback: F)
    where
        F: Fn(VoiceEvent) + Send + Sync + 'static,
    {
        self.event_callback = Some(Arc::new(callback));
    }

    /// Emit an event through the callback
    fn emit(&self, event: VoiceEvent) {
        if let Some(ref callback) = self.event_callback {
            callback(event);
        }
    }

    /// Start loading the model in background
    fn start_model_loading(&mut self) {
        let state_clone = Arc::clone(&self.state);
        let model = self.config.model.clone();
        let callback = self.event_callback.clone();

        // Emit loading event
        self.emit(VoiceEvent::ModelLoading);

        thread::spawn(move || {
            tracing::info!("Background: Starting Whisper model download/load...");

            let emit = |event: VoiceEvent| {
                if let Some(ref cb) = callback {
                    cb(event);
                }
            };

            match ensure_model(&model) {
                Ok(model_path) => {
                    tracing::info!("Background: Model downloaded, loading...");
                    match Transcriber::new(&model_path) {
                        Ok(transcriber) => {
                            if let Ok(mut s) = state_clone.lock() {
                                s.transcriber = Some(Arc::new(transcriber));
                                s.loading = false;
                                tracing::info!("Background: Whisper model ready!");
                            }
                            emit(VoiceEvent::ModelReady);
                        }
                        Err(e) => {
                            let msg = format!("Failed to load model: {}", e);
                            if let Ok(mut s) = state_clone.lock() {
                                s.error = Some(msg.clone());
                                s.loading = false;
                            }
                            tracing::error!("Background: {}", msg);
                            emit(VoiceEvent::ModelError(msg));
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("Failed to download model: {}", e);
                    if let Ok(mut s) = state_clone.lock() {
                        s.error = Some(msg.clone());
                        s.loading = false;
                    }
                    tracing::error!("Background: {}", msg);
                    emit(VoiceEvent::ModelError(msg));
                }
            }
        });
    }

    /// Check if model is ready
    pub fn is_ready(&self) -> bool {
        self.state
            .lock()
            .map(|s| s.transcriber.is_some())
            .unwrap_or(false)
    }

    /// Check if model is still loading
    pub fn is_loading(&self) -> bool {
        self.state.lock().map(|s| s.loading).unwrap_or(false)
    }

    /// Check if transcription is in progress
    pub fn is_transcribing(&self) -> bool {
        self.state.lock().map(|s| s.transcribing).unwrap_or(false)
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Get current status message (for UI display)
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

    /// Start recording audio
    pub fn start_recording(&mut self) -> Result<(), VoiceError> {
        // Recovery: if we think we're recording but audio capture disagrees, reset state
        if self.recording && !self.audio_capture.is_recording() {
            tracing::warn!("Recording state was stuck, resetting");
            self.recording = false;
        }

        if self.recording {
            return Err(VoiceError::AlreadyRecording);
        }

        // Check if model is ready
        if !self.is_ready() {
            if self.is_loading() {
                return Err(VoiceError::ModelLoading);
            } else {
                return Err(VoiceError::ModelNotLoaded);
            }
        }

        tracing::info!("Starting audio capture...");

        // Start audio capture
        self.audio_capture
            .start()
            .map_err(VoiceError::RecordingStart)?;

        self.recording = true;
        self.press_time = Some(Instant::now());

        self.emit(VoiceEvent::RecordingStarted);
        tracing::info!("Recording started");

        Ok(())
    }

    /// Stop recording and start transcription
    pub fn stop_recording(&mut self) -> Result<(), VoiceError> {
        tracing::info!("stop_recording called, recording={}", self.recording);

        if !self.recording {
            return Err(VoiceError::NotRecording);
        }

        self.recording = false;

        // Check hold duration
        let held_ms = self
            .press_time
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        tracing::info!("Recording held for {}ms", held_ms);

        let audio = self.audio_capture.stop();
        tracing::info!(
            "Audio capture stopped, got {} samples",
            audio.as_ref().map(|a| a.len()).unwrap_or(0)
        );

        // Quick tap - discard
        if held_ms < self.config.min_hold_ms {
            let reason = format!(
                "Quick tap ({}ms < {}ms minimum)",
                held_ms, self.config.min_hold_ms
            );
            tracing::info!("{}", reason);
            self.emit(VoiceEvent::RecordingDiscarded { reason });
            return Ok(());
        }

        // Get audio data
        let audio = match audio {
            Some(a) if !a.is_empty() => a,
            Some(_) => {
                let reason = "Audio buffer was empty".to_string();
                self.emit(VoiceEvent::RecordingDiscarded { reason });
                return Ok(());
            }
            None => {
                let reason = "No audio data captured".to_string();
                self.emit(VoiceEvent::RecordingDiscarded { reason });
                return Ok(());
            }
        };

        // Validate audio
        if let Err(reason) = self.validate_audio(&audio) {
            self.emit(VoiceEvent::RecordingDiscarded { reason });
            return Ok(());
        }

        // Get transcriber reference
        let transcriber = {
            let s = self
                .state
                .lock()
                .map_err(|e| VoiceError::Internal(e.to_string()))?;
            match s.transcriber.clone() {
                Some(t) => t,
                None => return Err(VoiceError::ModelNotLoaded),
            }
        };

        // Mark as transcribing
        {
            let mut s = self
                .state
                .lock()
                .map_err(|e| VoiceError::Internal(e.to_string()))?;
            s.transcribing = true;
        }

        self.emit(VoiceEvent::TranscriptionStarted);

        // Start background transcription
        let state_clone = Arc::clone(&self.state);
        let callback = self.event_callback.clone();
        let language = self.config.language.clone();

        thread::spawn(move || {
            tracing::info!(
                "Background: Starting transcription with {} samples...",
                audio.len()
            );

            let result = match &language {
                Some(lang) => transcriber.transcribe_with_language(&audio, Some(lang)),
                None => transcriber.transcribe_with_language(&audio, None),
            };

            tracing::info!("Background: Transcription finished");

            if let Ok(mut s) = state_clone.lock() {
                s.transcribing = false;
            }

            if let Some(cb) = callback {
                match result {
                    Ok(text) => cb(VoiceEvent::TranscriptionComplete { text }),
                    Err(e) => cb(VoiceEvent::TranscriptionError(e)),
                }
            }
        });

        Ok(())
    }

    /// Cancel recording without transcribing
    pub fn cancel_recording(&mut self) {
        if self.recording {
            self.recording = false;
            let _ = self.audio_capture.stop();
            self.emit(VoiceEvent::RecordingDiscarded {
                reason: "Cancelled".to_string(),
            });
        }
    }

    /// Validate audio data before transcription
    fn validate_audio(&self, audio: &[f32]) -> Result<(), String> {
        // Minimum duration
        if audio.len() < self.config.min_audio_samples {
            return Err(format!(
                "Audio too short: {} samples (minimum {})",
                audio.len(),
                self.config.min_audio_samples
            ));
        }

        // Check for silence (RMS level)
        let rms: f32 = (audio.iter().map(|x| x * x).sum::<f32>() / audio.len() as f32).sqrt();
        if rms < 1e-6 {
            return Err(format!("Audio too quiet: RMS = {}", rms));
        }

        // Check for NaN/inf
        if audio.iter().any(|x| x.is_nan() || x.is_infinite()) {
            return Err("Audio contains invalid values".to_string());
        }

        Ok(())
    }
}
