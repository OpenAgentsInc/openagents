//! Dictation from a browser (#12292): a paired browser records the
//! microphone and this computer transcribes it with its own Parakeet model,
//! the one the desktop's dictation uses. Audio goes to this computer only,
//! in chunks through its device room, and lives in memory until the
//! recording is transcribed or dropped; nothing is written down.
//!
//! A recording is 16 kHz mono 16-bit samples, little-endian, base64 in
//! [`Append::pcm`], at most [`MAX_SECONDS`] long and [`MAX_CHUNK_BYTES`] a
//! chunk.

use serde::{Deserialize, Serialize};

/// Whether this computer can transcribe: `{ready}`, false until its
/// dictation model is downloaded in the app's Voice settings.
pub const DICTATION_STATUS: &str = "DictationStatus";
/// Add a chunk to a recording, with [`Append`] params. Returns `{}`.
pub const DICTATION_APPEND: &str = "DictationAppend";
/// Transcribe a recording and drop its audio, with [`Recording`] params.
/// Returns `{text}`.
pub const DICTATION_FINISH: &str = "DictationFinish";
/// Drop a recording without transcribing it, with [`Recording`] params.
pub const DICTATION_CANCEL: &str = "DictationCancel";

/// Every method here.
pub const ALL: &[&str] = &[
    DICTATION_STATUS,
    DICTATION_APPEND,
    DICTATION_FINISH,
    DICTATION_CANCEL,
];

/// The sample rate a recording is sent at.
pub const RATE: u32 = 16_000;
/// The longest recording, as on the desktop.
pub const MAX_SECONDS: usize = 60;
/// The most audio one chunk carries, before base64.
pub const MAX_CHUNK_BYTES: usize = 256 * 1024;

/// [`DICTATION_APPEND`]'s params.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Append {
    /// The browser's name for the recording.
    pub recording: String,
    /// The chunk's place, from 0.
    pub seq: u32,
    /// Base64 16-bit little-endian samples at [`RATE`].
    pub pcm: String,
}

/// [`DICTATION_FINISH`]'s and [`DICTATION_CANCEL`]'s params.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recording {
    pub recording: String,
}

/// Whether `method` is one of these.
pub fn handles(method: &str) -> bool {
    ALL.contains(&method)
}
