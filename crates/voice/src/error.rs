use thiserror::Error;

/// Errors that can occur during voice operations
#[derive(Error, Debug)]
pub enum VoiceError {
    #[error("No audio input device available")]
    NoInputDevice,

    #[error("Failed to initialize audio: {0}")]
    AudioInit(String),

    #[error("Failed to start recording: {0}")]
    RecordingStart(String),

    #[error("Failed to stop recording: {0}")]
    RecordingStop(String),

    #[error("Not currently recording")]
    NotRecording,

    #[error("Already recording")]
    AlreadyRecording,

    #[error("Model not loaded")]
    ModelNotLoaded,

    #[error("Model is still loading")]
    ModelLoading,

    #[error("Failed to load model: {0}")]
    ModelLoad(String),

    #[error("Failed to download model: {0}")]
    ModelDownload(String),

    #[error("Transcription failed: {0}")]
    Transcription(String),

    #[error("Audio too short (minimum 0.5 seconds)")]
    AudioTooShort,

    #[error("Audio too quiet")]
    AudioTooQuiet,

    #[error("Invalid audio data")]
    InvalidAudio,

    #[error("Internal error: {0}")]
    Internal(String),
}
