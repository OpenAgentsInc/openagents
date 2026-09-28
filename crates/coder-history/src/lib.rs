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
/// The `end` of a backward read that starts at the newest record: any offset
/// at or past the source's length works, and this one is the largest integer
/// that canonical JSON (RFC 8785) carries exactly.
pub const NEWEST: u64 = (1 << 53) - 1;
pub const MAX_READABLE_RECORD_BYTES: usize = 256 * 1024;

/// The bounds one read answers within. A reply that crosses a relay keeps
/// [`Limits::RELAY`]; a reply on a direct tailnet connection, which carries
/// one sealed reply of up to NIP-44's 256 KiB plaintext, can use
/// [`Limits::DIRECT`], so a chat's newest screen comes in one read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The most raw source bytes one transcript page asks for.
    pub page_bytes: u32,
    /// The most bytes one encoded page takes.
    pub response_bytes: usize,
    /// The most chats one catalog page lists.
    pub catalog_page: u16,
    /// The most record chunks one transcript page holds.
    pub chunks: usize,
}

impl Limits {
    pub const RELAY: Self = Self {
        page_bytes: MAX_PAGE_BYTES,
        response_bytes: MAX_RESPONSE_BYTES,
        catalog_page: MAX_CATALOG_PAGE,
        chunks: 128,
    };
    pub const DIRECT: Self = Self {
        page_bytes: 160 * 1024,
        response_bytes: 232 * 1024,
        catalog_page: 256,
        chunks: 2048,
    };
}

mod project;
pub use project::{readable_record, readable_record_full};

/// How Coder's engine marks a Claude Code, Codex, or OpenCode session it
/// starts for its own work (a delegated turn, an explorer handoff, a
/// terminal answer, a repository run), so the session's own store records
/// that it is not a chat the user typed.
///
/// The engine sets [`CODEX_VARIABLE`](engine::CODEX_VARIABLE) or
/// [`CLAUDE_VARIABLE`](engine::CLAUDE_VARIABLE) to [`MARK`](engine::MARK)
/// when it launches the CLI. Codex records the value as the session's
/// `originator` in its `session_meta` header; Claude Code records it as each
/// record's `entrypoint`. OpenCode records no caller in a session, so the
/// engine gives it a database of its own instead: it sets
/// [`OPENCODE_VARIABLE`](engine::OPENCODE_VARIABLE) to
/// [`opencode_database`](engine::opencode_database), and the session is
/// saved there, never in the owner's `opencode.db` that the catalog reads.
/// The catalog leaves such a session out: the task's own Coder transcript is
/// the chat.
pub mod engine {
    /// The value both CLIs record.
    pub const MARK: &str = "openagents-coder-engine";
    /// Codex's originator override, recorded as `session_meta.originator`.
    pub const CODEX_VARIABLE: &str = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";
    /// Claude Code's entry point, recorded as each record's `entrypoint`.
    pub const CLAUDE_VARIABLE: &str = "CLAUDE_CODE_ENTRYPOINT";
    /// OpenCode's database path (`OPENCODE_DB`, OpenCode 1.2 and later).
    pub const OPENCODE_VARIABLE: &str = "OPENCODE_DB";
    /// The engine's OpenCode database, relative to the home directory.
    pub const OPENCODE_DATABASE: &str = ".openagents/opencode/engine.db";

    /// The engine's OpenCode database under `home`: the file every
    /// engine-started OpenCode session is saved in.
    #[must_use]
    pub fn opencode_database(home: &std::path::Path) -> std::path::PathBuf {
        home.join(OPENCODE_DATABASE)
    }
}

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
    /// A Coder task attempt's transcript, `<task>.<attempt>.atif.jsonl`.
    Coder,
    /// An OpenCode session, as the host mirrors it from OpenCode's database
    /// (`opencode::mirror`, feature `opencode`).
    #[serde(rename = "opencode")]
    OpenCode,
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
    /// Read backward instead of from a cursor: the page of whole records
    /// that ends at or before this offset, [`NEWEST`] for the newest. It
    /// needs no cursor; the page's `previous` names the next one to ask for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<u64>,
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
    /// A backward read's start, when earlier records remain: send it as the
    /// next request's `end`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<u64>,
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
#[cfg(all(feature = "opencode", any(target_os = "linux", target_os = "macos")))]
pub mod opencode;
#[cfg(feature = "host")]
pub use host::{Config, History};
