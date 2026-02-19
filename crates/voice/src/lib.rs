//! Voice transcription library using Whisper
//!
//! This crate provides voice recording and transcription functionality using
//! whisper.cpp via whisper-rs bindings.
//!
//! # Example
//!
//! ```no_run
//! use voice::{VoiceSession, VoiceEvent, VoiceConfig};
//!
//! let mut session = VoiceSession::new().expect("Failed to create session");
//!
//! session.on_event(|event| {
//!     match event {
//!         VoiceEvent::ModelReady => println!("Model ready!"),
//!         VoiceEvent::TranscriptionComplete { text } => println!("Transcribed: {}", text),
//!         _ => {}
//!     }
//! });
//!
//! // Start recording when user presses a key
//! session.start_recording().expect("Failed to start");
//!
//! // Stop recording when user releases the key
//! session.stop_recording().expect("Failed to stop");
//! ```

mod audio_capture;
mod error;
mod model_manager;
mod session;
mod transcriber;

// Re-export main types
pub use audio_capture::AudioCapture;
pub use error::VoiceError;
pub use model_manager::{available_models, ensure_model, model_size_mb};
pub use session::{VoiceConfig, VoiceEvent, VoiceSession};
pub use transcriber::Transcriber;
