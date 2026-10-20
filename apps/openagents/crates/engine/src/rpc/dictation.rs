//! Dictation from a paired browser (#12292): the `Dictation*` methods. The
//! browser sends a recording in chunks; this computer keeps it in memory
//! and transcribes it with the Parakeet model the desktop's dictation
//! downloaded (`openagents-voice`), then drops the audio. Nothing is
//! written down.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use base64::Engine as _;
use openagents_rpc::dictation::{
    Append, DICTATION_APPEND, DICTATION_CANCEL, DICTATION_FINISH, DICTATION_STATUS,
    MAX_CHUNK_BYTES, MAX_SECONDS, RATE, Recording,
};
use openagents_rpc::{RpcError, RpcReply, parse_params};
use serde_json::{Value, json};

pub(super) use openagents_rpc::dictation::handles;

/// At most this many recordings wait at once.
const MAX_RECORDINGS: usize = 4;
/// A recording nobody added to for this long is dropped.
const IDLE: Duration = Duration::from_secs(120);
/// The longest recording, in samples.
const MAX_SAMPLES: usize = RATE as usize * MAX_SECONDS;

/// Where the desktop's dictation keeps its model, under the data dir.
pub(crate) fn model_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("models/parakeet-tdt-0.6b-v3-int8")
}

struct Buffer {
    samples: Vec<i16>,
    next: u32,
    touched: Instant,
}

/// The recordings in memory, by the browser's name for each.
#[derive(Default)]
pub(crate) struct Recordings(HashMap<String, Buffer>);

impl Recordings {
    /// Add chunk `append` to its recording, starting it at chunk 0.
    pub(crate) fn append(&mut self, append: &Append, now: Instant) -> Result<(), String> {
        self.0.retain(|_, b| now.duration_since(b.touched) < IDLE);
        if append.recording.is_empty() || append.recording.len() > 64 {
            return Err("Name the recording.".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&append.pcm)
            .map_err(|_| "The recording didn't arrive whole.".to_string())?;
        if bytes.len() > MAX_CHUNK_BYTES || bytes.len() % 2 != 0 {
            return Err("The recording didn't arrive whole.".into());
        }
        if append.seq == 0 {
            if !self.0.contains_key(&append.recording) && self.0.len() >= MAX_RECORDINGS {
                return Err("Too many recordings at once. Try again in a moment.".into());
            }
            self.0.insert(
                append.recording.clone(),
                Buffer {
                    samples: Vec::new(),
                    next: 0,
                    touched: now,
                },
            );
        }
        let buffer = self
            .0
            .get_mut(&append.recording)
            .filter(|b| b.next == append.seq)
            .ok_or_else(|| "The recording didn't arrive whole.".to_string())?;
        if buffer.samples.len() + bytes.len() / 2 > MAX_SAMPLES {
            return Err("Recording exceeds one minute".into());
        }
        buffer.samples.extend(
            bytes
                .chunks_exact(2)
                .map(|pair| i16::from_le_bytes([pair[0], pair[1]])),
        );
        buffer.next += 1;
        buffer.touched = now;
        Ok(())
    }

    /// Take a recording's samples out, as the model reads them.
    pub(crate) fn take(&mut self, recording: &str) -> Option<Vec<f32>> {
        let buffer = self.0.remove(recording)?;
        Some(
            buffer
                .samples
                .into_iter()
                .map(|s| f32::from(s) / 32_768.0)
                .collect(),
        )
    }

    pub(crate) fn cancel(&mut self, recording: &str) {
        self.0.remove(recording);
    }
}

static RECORDINGS: Mutex<Option<Recordings>> = Mutex::new(None);
/// The model, loaded on the first recording and kept.
static RECOGNIZER: Mutex<Option<openagents_voice::Recognizer>> = Mutex::new(None);

fn recordings<T>(f: impl FnOnce(&mut Recordings) -> T) -> T {
    let mut held = RECORDINGS.lock().unwrap_or_else(PoisonError::into_inner);
    f(held.get_or_insert_with(Recordings::default))
}

pub(super) async fn handle(
    data_dir: &Path,
    method: &str,
    params: Value,
) -> Result<RpcReply, RpcError> {
    let dir = model_dir(data_dir);
    match method {
        DICTATION_STATUS => {
            RpcReply::value(&json!({ "ready": openagents_voice::installed(&dir) }))
        }
        DICTATION_APPEND => {
            let append: Append = parse_params(params)?;
            recordings(|r| r.append(&append, Instant::now())).map_err(RpcError::Failed)?;
            RpcReply::value(&json!({}))
        }
        DICTATION_CANCEL => {
            let p: Recording = parse_params(params)?;
            recordings(|r| r.cancel(&p.recording));
            RpcReply::value(&json!({}))
        }
        DICTATION_FINISH => {
            let p: Recording = parse_params(params)?;
            let samples = recordings(|r| r.take(&p.recording))
                .ok_or_else(|| RpcError::Failed("That recording isn't here.".into()))?;
            if !openagents_voice::installed(&dir) {
                return Err(RpcError::Failed(
                    "Dictation isn't set up on this computer. Download its model in the OpenAgents app's Voice settings.".into(),
                ));
            }
            let text = tokio::task::spawn_blocking(move || {
                let mut held = RECOGNIZER.lock().unwrap_or_else(PoisonError::into_inner);
                if held.is_none() {
                    *held = Some(openagents_voice::Recognizer::load(&dir)?);
                }
                held.as_mut().map_or_else(
                    || Ok(String::new()),
                    |model| model.transcribe(samples, RATE),
                )
            })
            .await
            .map_err(|_| RpcError::Failed("Could not transcribe this recording".into()))?
            .map_err(|e| RpcError::Failed(e.to_string()))?;
            RpcReply::value(&json!({ "text": text.trim() }))
        }
        _ => Err(RpcError::Failed("Choose a dictation step.".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(recording: &str, seq: u32, samples: &[i16]) -> Append {
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        Append {
            recording: recording.into(),
            seq,
            pcm: base64::engine::general_purpose::STANDARD.encode(bytes),
        }
    }

    #[test]
    fn a_recording_arrives_in_order_and_leaves_once() {
        let mut r = Recordings::default();
        let now = Instant::now();
        r.append(&chunk("a", 0, &[0, 16_384]), now).unwrap();
        r.append(&chunk("a", 1, &[-32_768]), now).unwrap();
        // A chunk out of order or for no recording is refused.
        assert!(r.append(&chunk("a", 3, &[1]), now).is_err());
        assert!(r.append(&chunk("b", 1, &[1]), now).is_err());
        assert_eq!(r.take("a"), Some(vec![0.0, 0.5, -1.0]));
        assert_eq!(r.take("a"), None);
    }

    #[test]
    fn recordings_are_bounded() {
        let mut r = Recordings::default();
        let now = Instant::now();
        for name in ["a", "b", "c", "d"] {
            r.append(&chunk(name, 0, &[1]), now).unwrap();
        }
        assert!(r.append(&chunk("e", 0, &[1]), now).is_err());
        // An idle one makes room.
        let later = now + IDLE + Duration::from_secs(1);
        r.append(&chunk("e", 0, &[1]), later).unwrap();
        // No more than a minute.
        let minute = vec![0i16; MAX_CHUNK_BYTES / 2];
        let mut seq = 1;
        let result = loop {
            match r.append(&chunk("e", seq, &minute), later) {
                Ok(()) => seq += 1,
                Err(e) => break e,
            }
        };
        assert_eq!(result, "Recording exceeds one minute");
        r.cancel("e");
        assert_eq!(r.take("e"), None);
        // A chunk that isn't whole samples is refused.
        let odd = Append {
            recording: "f".into(),
            seq: 0,
            pcm: base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3]),
        };
        assert!(r.append(&odd, later).is_err());
    }
}
