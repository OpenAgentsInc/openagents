//! Bounded read-only access to saved harness history.
//!
//! The default host feature reads explicitly selected desktop roots on macOS and Linux.
//! Disable it for portable request/page types. Reading history grants no relay
//! disclosure authority and never starts, resumes, or modifies a harness.

use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_RESPONSE_BYTES: usize = 112 * 1024;
pub const MAX_PAGE_BYTES: u32 = 32 * 1024;
pub const MAX_CHUNK_BYTES: usize = 8 * 1024;
pub const MAX_CATALOG_PAGE: u16 = 32;
pub const MAX_READABLE_RECORD_BYTES: usize = 256 * 1024;

mod project;
pub use project::{readable_record, readable_record_full};

/// Stable native record identity within one source-file incarnation. Clients
/// use this same formula to validate chunks before assembling a transcript.
pub fn record_id(source_id: &str, incarnation: &str, record_offset: u64) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(format!("{source_id}\0{incarnation}\0{record_offset}").as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Harness {
    Codex,
    Claude,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogRequest {
    pub cursor: Option<CatalogCursor>,
    pub limit: u16,
}

impl Default for CatalogRequest {
    fn default() -> Self {
        Self {
            cursor: None,
            limit: 16,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogCursor {
    pub snapshot: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogPage {
    pub snapshot: String,
    pub entries: Vec<Chat>,
    pub next: Option<CatalogCursor>,
    pub notices: Vec<Notice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chat {
    /// Stable native conversation identity, distinct from a source file.
    pub id: String,
    pub harness: Harness,
    pub native_id: Option<String>,
    pub title: String,
    pub title_truncated: bool,
    pub updated_at: Option<String>,
    pub archived: bool,
    pub subagent: bool,
    /// An opaque source selector; no path is accepted from a remote caller.
    pub source_id: Option<String>,
    pub status: SourceStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStatus {
    Available,
    Missing,
    Unreadable,
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notice {
    pub code: String,
    pub source_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptRequest {
    pub source_id: String,
    pub cursor: Option<TranscriptCursor>,
    pub max_bytes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptCursor {
    pub source_id: String,
    pub incarnation: String,
    pub offset: u64,
    pub record_offset: u64,
    pub record_index: u64,
    pub prefix_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptPage {
    pub source_id: String,
    pub incarnation: String,
    /// File length captured for this page; new bytes are read on a later poll.
    pub snapshot_bytes: u64,
    pub chunks: Vec<RecordChunk>,
    /// Retain this cursor even at EOF and use it when polling for new bytes.
    pub next: TranscriptCursor,
    pub has_more: bool,
    pub pending_line: bool,
    pub notices: Vec<Notice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordChunk {
    pub id: String,
    pub index: u64,
    pub record_offset: u64,
    pub offset: u64,
    pub end_offset: u64,
    /// Exact source bytes, including a terminating newline when present.
    pub raw_base64: String,
    /// True only when this chunk reaches the source record's newline.
    pub complete: bool,
    pub oversized: bool,
    pub readable: Option<Readable>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Readable {
    pub kind: String,
    pub native_id: Option<String>,
    pub role: Option<String>,
    pub timestamp: Option<String>,
    pub tool_name: Option<String>,
    pub call_id: Option<String>,
    /// A bounded convenience projection. Raw bytes remain authoritative.
    pub text: String,
    pub text_truncated: bool,
    pub unknown: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum Error {
    InvalidRequest,
    InvalidRoot,
    UnsupportedPlatform,
    SourceMissing,
    SourceUnreadable,
    SourceChanged,
    CursorStale,
    ResourceLimit,
    Encoding,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidRequest => "invalid history request",
            Self::InvalidRoot => "history root must be an explicitly selected existing directory",
            Self::UnsupportedPlatform => {
                "history host requires macOS or Linux descriptor confinement"
            }
            Self::SourceMissing => "history source is unavailable; refresh the catalog",
            Self::SourceUnreadable => "history source could not be read",
            Self::SourceChanged => "history source changed; restart from its current catalog entry",
            Self::CursorStale => "history catalog changed; restart catalog pagination",
            Self::ResourceLimit => "history exceeds a declared resource bound",
            Self::Encoding => "history page could not be encoded",
        })
    }
}
impl std::error::Error for Error {}

#[cfg(feature = "host")]
mod host;
#[cfg(feature = "host")]
pub use host::{Config, History};
