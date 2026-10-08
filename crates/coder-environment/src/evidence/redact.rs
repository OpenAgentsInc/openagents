//! Selected-credential redaction that runs before any byte is spooled.
//!
//! A [`Redactor`] holds the exact values of the credentials a run selected
//! (and the fragments of credential documents such as engine login files).
//! It never prints or serializes them. [`Redactor::scrub`] works on raw
//! bytes with a rolling hold-back of `longest value - 1` bytes, so a value
//! split across chunk boundaries is still replaced, and a UTF-8 character
//! split across chunks passes through unchanged because nothing decodes it.

use super::EvidenceError;
use serde_json::Value;
use std::path::Path;

/// What replaces each selected credential value in retained evidence.
pub const REDACTION_MARKER: &[u8] = b"[redacted]";
/// Shortest explicitly selected value; shorter values match ordinary text.
pub const MIN_SELECTED_BYTES: usize = 8;
/// Longest selected value, which also bounds the per-stream hold-back.
pub const MAX_SELECTED_BYTES: usize = 1024 * 1024;
/// Most values one redactor holds.
pub const MAX_SELECTED_VALUES: usize = 4096;

/// Engine login files and environment variables. Their values never
/// enter evidence: [`Redactor::engine_logins`] selects every credential in
/// them. The Claude Code and Codex login rules live in `secret-screen`, the
/// one place every evidence path reads them from.
pub use secret_screen::{ENGINE_LOGIN_ENV, ENGINE_LOGIN_FILES};

#[derive(Clone, Default)]
pub struct Redactor {
    /// Longest first, deduplicated.
    values: Vec<Vec<u8>>,
}

impl std::fmt::Debug for Redactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Redactor({} values)", self.values.len())
    }
}

/// Scrubbed bytes ready to persist, and how many values they replaced.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Scrubbed {
    pub bytes: Vec<u8>,
    pub redactions: u64,
}

impl Redactor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Select one credential value. Its trimmed form is selected too, so a
    /// value read from a file with a trailing newline still matches.
    pub fn select(&mut self, value: &str) -> Result<(), EvidenceError> {
        if value.len() > MAX_SELECTED_BYTES || value.trim().len() < MIN_SELECTED_BYTES {
            return Err(EvidenceError::Credential(
                "A selected credential must be 8 bytes to 1 MiB.",
            ));
        }
        self.insert(value.as_bytes())?;
        self.insert(value.trim().as_bytes())
    }

    /// Select a credential document: the whole text, and when it is JSON,
    /// every string under a key that names a token, secret, key, or
    /// password (for example `accessToken`, `refresh_token`,
    /// `OPENAI_API_KEY`).
    pub fn select_document(&mut self, text: &str) -> Result<(), EvidenceError> {
        if text.trim().is_empty() {
            return Ok(());
        }
        if text.trim().len() >= MIN_SELECTED_BYTES {
            self.select(text)?;
        }
        for value in secret_screen::credential_fragments(text) {
            if value.trim().len() >= MIN_SELECTED_BYTES && value.len() <= MAX_SELECTED_BYTES {
                self.select(&value)?;
            }
        }
        Ok(())
    }

    /// Select every engine login on this host: the Claude Code and Codex
    /// login files under `home` and the login environment variables `env`
    /// returns. A missing file is fine; an unreadable one fails closed.
    pub fn engine_logins(
        home: &Path,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, EvidenceError> {
        let mut redactor = Self::new();
        redactor.add_engine_logins(home, env)?;
        Ok(redactor)
    }

    pub fn add_engine_logins(
        &mut self,
        home: &Path,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<(), EvidenceError> {
        for file in ENGINE_LOGIN_FILES {
            match std::fs::read(home.join(file)) {
                Ok(bytes) => self.select_document(&String::from_utf8_lossy(&bytes))?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    return Err(EvidenceError::Credential(
                        "An engine login file could not be read, so its values cannot be redacted.",
                    ));
                }
            }
        }
        for name in ENGINE_LOGIN_ENV {
            if let Some(value) = env(name).filter(|v| !v.trim().is_empty()) {
                self.select_document(&value)?;
            }
        }
        Ok(())
    }

    fn insert(&mut self, value: &[u8]) -> Result<(), EvidenceError> {
        if value.len() < MIN_SELECTED_BYTES || self.values.iter().any(|v| v == value) {
            return Ok(());
        }
        if self.values.len() >= MAX_SELECTED_VALUES {
            return Err(EvidenceError::Credential("Too many selected credentials."));
        }
        self.values.push(value.to_vec());
        self.values.sort_by_key(|v| std::cmp::Reverse(v.len()));
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Bytes a stream holds back between chunks.
    pub fn hold_back(&self) -> usize {
        self.values.first().map_or(0, |v| v.len() - 1)
    }

    /// Length of the longest selected value starting at `at`.
    fn match_at(&self, bytes: &[u8], at: usize) -> Option<usize> {
        let rest = &bytes[at..];
        self.values
            .iter()
            .find(|v| rest.starts_with(v))
            .map(|v| v.len())
    }

    /// Scrub the next `input` of a stream whose earlier undecided tail is
    /// `held`. Returns the bytes that are now safe to persist; the new
    /// undecided tail stays in `held` (memory only). With `last`, nothing is
    /// held back.
    pub fn scrub(&self, held: &mut Vec<u8>, input: &[u8], last: bool) -> Scrubbed {
        let mut buffer = std::mem::take(held);
        buffer.extend_from_slice(input);
        let keep = if last { 0 } else { self.hold_back() };
        let safe_end = buffer.len().saturating_sub(keep);
        let mut out = Scrubbed {
            bytes: Vec::with_capacity(safe_end),
            redactions: 0,
        };
        let mut at = 0;
        // Below `safe_end`, every selected value fits inside `buffer`, so a
        // match decision there never depends on bytes not yet received.
        while at < safe_end {
            if let Some(len) = self.match_at(&buffer, at) {
                out.bytes.extend_from_slice(REDACTION_MARKER);
                out.redactions += 1;
                at += len;
            } else {
                out.bytes.push(buffer[at]);
                at += 1;
            }
        }
        buffer.drain(..at.min(buffer.len()));
        *held = buffer;
        out
    }

    /// Scrub a whole text value.
    pub fn redact_text(&self, text: &str) -> (String, u64) {
        let mut held = Vec::new();
        let out = self.scrub(&mut held, text.as_bytes(), true);
        // Markers are ASCII and replace whole values, but a value that ends
        // inside a character could split it; keep the result valid.
        (
            String::from_utf8_lossy(&out.bytes).into_owned(),
            out.redactions,
        )
    }

    /// Scrub every string (and object key) in structured arguments or
    /// results in place; returns the number of values replaced.
    pub fn redact_json(&self, value: &mut Value) -> u64 {
        match value {
            Value::String(s) => {
                let (text, n) = self.redact_text(s);
                *s = text;
                n
            }
            Value::Array(items) => items.iter_mut().map(|v| self.redact_json(v)).sum(),
            Value::Object(fields) => {
                let mut n = 0;
                let taken = std::mem::take(fields);
                for (key, mut v) in taken {
                    let (key, k) = self.redact_text(&key);
                    n += k + self.redact_json(&mut v);
                    fields.insert(key, v);
                }
                n
            }
            _ => 0,
        }
    }
}
