use std::path::Path;
use std::sync::Mutex;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

/// Whisper transcriber using whisper-rs (whisper.cpp bindings)
pub struct Transcriber {
    ctx: WhisperContext,
    // Cache the state to avoid re-initializing GPU buffers on each transcription
    state: Mutex<Option<WhisperState>>,
}

impl Transcriber {
    /// Create a new transcriber with the given model path
    pub fn new(model_path: &Path) -> Result<Self, String> {
        tracing::info!("Loading Whisper model from: {:?}", model_path);

        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or("Invalid model path")?,
            params,
        )
        .map_err(|e| format!("Failed to load Whisper model: {}", e))?;

        tracing::info!("Whisper model loaded successfully");

        Ok(Self {
            ctx,
            state: Mutex::new(None),
        })
    }

    /// Transcribe audio data to text
    ///
    /// # Arguments
    /// * `audio` - Audio samples as f32, 16kHz mono
    ///
    /// # Returns
    /// Transcribed text
    pub fn transcribe(&self, audio: &[f32]) -> Result<String, String> {
        self.transcribe_with_language(audio, Some("en"))
    }

    /// Transcribe audio data to text with specified language
    ///
    /// # Arguments
    /// * `audio` - Audio samples as f32, 16kHz mono
    /// * `language` - Language code (e.g., "en") or None for auto-detect
    ///
    /// # Returns
    /// Transcribed text
    pub fn transcribe_with_language(&self, audio: &[f32], language: Option<&str>) -> Result<String, String> {
        // Get or create state (first call will be slow due to GPU init)
        let mut state_guard = self.state.lock().map_err(|e| format!("Lock error: {}", e))?;

        let state = if let Some(ref mut existing_state) = *state_guard {
            existing_state
        } else {
            tracing::info!("Creating whisper state (first transcription, may take a moment)...");
            let new_state = self.ctx.create_state()
                .map_err(|e| format!("Failed to create state: {}", e))?;
            *state_guard = Some(new_state);
            tracing::info!("Whisper state ready");
            state_guard.as_mut().unwrap()
        };

        // Configure transcription parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Set language
        params.set_language(language);

        // Disable translation (we want transcription)
        params.set_translate(false);

        // Single segment mode for faster processing
        params.set_single_segment(false);

        // Don't print progress
        params.set_print_progress(false);
        params.set_print_special(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Run transcription
        state
            .full(params, audio)
            .map_err(|e| format!("Transcription failed: {}", e))?;

        // Collect segments
        let num_segments = state
            .full_n_segments()
            .map_err(|e| format!("Failed to get segments: {}", e))?;

        let mut result = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                if !result.is_empty() {
                    result.push(' ');
                }
                result.push_str(&segment);
            }
        }

        let result = result.trim().to_string();
        tracing::info!("Transcription result: {}", result);
        Ok(result)
    }
}
