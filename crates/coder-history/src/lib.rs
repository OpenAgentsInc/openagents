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

/// How Coder's engine marks a Claude Code, Codex, OpenCode, or Devin
/// session it starts for its own work (a delegated turn, an explorer
/// handoff, a terminal answer, a repository run), so the session's own store records
/// that it is not a chat the user typed.
///
/// The engine sets [`CODEX_VARIABLE`](engine::CODEX_VARIABLE) or
/// [`CLAUDE_VARIABLE`](engine::CLAUDE_VARIABLE) to [`MARK`](engine::MARK)
/// when it launches the CLI. Codex records the value as the session's
/// `originator` in its `session_meta` header; Claude Code records it as each
/// record's `entrypoint`. OpenCode records no caller in a session, so the
/// engine gives it a database of its own instead: it sets
/// [`OPENCODE_VARIABLE`](engine::OPENCODE_VARIABLE) to
/// [`OPENCODE_DATABASE`](engine::OPENCODE_DATABASE), a name OpenCode
/// resolves in its own data directory beside the owner's `opencode.db`, so
/// the session keeps OpenCode's logins but is saved in
/// `openagents-coder-engine.db`, never in the owner's `opencode.db`.
/// Devin keeps the `_meta` of the ACP `session/new` that started a session;
/// the engine sets [`DEVIN_META_KEY`](engine::DEVIN_META_KEY) there to
/// [`MARK`](engine::MARK), and a Devin mirror never writes such a session.
/// The catalog leaves such a session out: the task's own Coder transcript is
/// the chat, and the session's copy beside the task ([`delegate`]) lists as
/// the task's subagent.
pub mod engine {
    /// The value both CLIs record.
    pub const MARK: &str = "openagents-coder-engine";
    /// Codex's originator override, recorded as `session_meta.originator`.
    pub const CODEX_VARIABLE: &str = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";
    /// Claude Code's entry point, recorded as each record's `entrypoint`.
    pub const CLAUDE_VARIABLE: &str = "CLAUDE_CODE_ENTRYPOINT";
    /// OpenCode's database path (`OPENCODE_DB`, OpenCode 1.2 and later).
    pub const OPENCODE_VARIABLE: &str = "OPENCODE_DB";
    /// The engine's OpenCode database: a relative name, which OpenCode
    /// resolves in its data directory (`~/.local/share/opencode`).
    pub const OPENCODE_DATABASE: &str = "openagents-coder-engine.db";
    /// The `session/new` `_meta` key Devin keeps as the session's
    /// `metadata.client_meta`.
    pub const DEVIN_META_KEY: &str = "openagents.com/engine";

    /// The engine's OpenCode database in OpenCode's data directory `data`:
    /// the file every engine-started OpenCode session is saved in.
    #[must_use]
    pub fn opencode_database(data: &std::path::Path) -> std::path::PathBuf {
        data.join(OPENCODE_DATABASE)
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
    /// An OpenCode session: a delegate session's copy beside its Coder
    /// task ([`delegate`]), or a mirror of OpenCode's database
    /// (`opencode::mirror`, feature `opencode`).
    #[serde(rename = "opencode")]
    OpenCode,
    /// A Devin CLI session: a delegate session's copy beside its Coder task
    /// ([`delegate`]), or a mirror of Devin's session store
    /// (`devin::mirror`, feature `devin`).
    Devin,
}

/// Whether `id` is a Devin session ID the host mirrors: lowercase letters,
/// digits, and hyphens, at most 64 bytes, such as `serene-crayfish`.
#[must_use]
pub fn devin_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A delegate session's transcript, kept beside the Coder task that
/// delegated to it.
///
/// When a Coder task's turn hands its work to a whole coding agent over ACP
/// (an OpenCode or Devin route), the agent keeps its session in its own
/// SQLite store. At the end of each turn the engine copies that session,
/// in the same JSONL shape [`opencode`] and [`devin`] write, into the task
/// directory as `<task>.delegate.<agent>.<session>.jsonl`, and notes the
/// copy on the task's transcript under [`NOTE`]. The copy only grows while
/// the session only grows, so a reader's cursor stays valid across turns.
///
/// The host's Coder catalog lists such a file as a subagent chat of its
/// task: `harness` is the agent (`opencode` or `devin`), `native_id` is the
/// task's 64-hex ID (the same as the task's own chats), `subagent` is true,
/// and `title` names the agent and session. A device shows it inside the
/// Coder chat with that `native_id`, and reads it as any other transcript
/// by its `source_id`.
pub mod delegate {
    use crate::Harness;

    /// The step extension on a Coder transcript that names a delegate
    /// session's copy: `{"agent", "session", "file"}`.
    pub const NOTE: &str = "delegate_transcript";

    fn word(agent: Harness) -> Option<&'static str> {
        match agent {
            Harness::OpenCode => Some("opencode"),
            Harness::Devin => Some("devin"),
            Harness::Codex | Harness::Claude | Harness::Coder => None,
        }
    }

    fn session_id(agent: Harness, session: &str) -> bool {
        match agent {
            Harness::OpenCode => {
                session.starts_with("ses_")
                    && session.len() <= 64
                    && session[4..].bytes().all(|b| b.is_ascii_alphanumeric())
            }
            Harness::Devin => crate::devin_session_id(session),
            _ => false,
        }
    }

    fn task_id(task: &str) -> bool {
        task.len() == 64 && task.bytes().all(|b| b.is_ascii_hexdigit())
    }

    /// The file name of `agent`'s session `session` delegated by the task
    /// `task`, or `None` when an ID is not one this shape admits.
    #[must_use]
    pub fn file_name(task: &str, agent: Harness, session: &str) -> Option<String> {
        let word = word(agent)?;
        (task_id(task) && session_id(agent, session))
            .then(|| format!("{task}.delegate.{word}.{session}.jsonl"))
    }

    /// The task, agent, and session a delegate file's name names.
    #[must_use]
    pub fn parse(name: &str) -> Option<(String, Harness, String)> {
        let rest = name.strip_suffix(".jsonl")?;
        let (task, rest) = rest.split_once(".delegate.")?;
        let (agent, session) = rest.split_once('.')?;
        let agent = match agent {
            "opencode" => Harness::OpenCode,
            "devin" => Harness::Devin,
            _ => return None,
        };
        (file_name(task, agent, session).as_deref() == Some(name))
            .then(|| (task.to_owned(), agent, session.to_owned()))
    }

    /// Settle a freshly written copy at `fresh` onto `path`: nothing when
    /// they are the same, an append when `path` holds a prefix of it, else a
    /// replacement. Returns whether `path` changed.
    #[cfg(any(feature = "opencode", feature = "devin"))]
    pub(crate) fn settle(fresh: &std::path::Path, path: &std::path::Path) -> Result<bool, String> {
        use std::io::Write as _;
        let new =
            std::fs::read(fresh).map_err(|e| format!("cannot read {}: {e}", fresh.display()))?;
        let old = std::fs::read(path).unwrap_or_default();
        if !old.is_empty() && new == old {
            let _ = std::fs::remove_file(fresh);
            return Ok(false);
        }
        if !old.is_empty() && new.starts_with(&old) {
            let appended = std::fs::OpenOptions::new()
                .append(true)
                .open(path)
                .and_then(|mut file| file.write_all(&new[old.len()..]));
            if appended.is_ok() {
                let _ = std::fs::remove_file(fresh);
                return Ok(true);
            }
        }
        std::fs::rename(fresh, path).map_err(|e| {
            let _ = std::fs::remove_file(fresh);
            format!("cannot write {}: {e}", path.display())
        })?;
        Ok(true)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_delegate_file_name_round_trips_and_refuses_other_shapes() {
            let task = "a".repeat(64);
            let name = file_name(&task, Harness::OpenCode, "ses_0Abc").unwrap();
            assert_eq!(name, format!("{task}.delegate.opencode.ses_0Abc.jsonl"));
            assert_eq!(
                parse(&name),
                Some((task.clone(), Harness::OpenCode, "ses_0Abc".into()))
            );
            let devin = file_name(&task, Harness::Devin, "serene-crayfish").unwrap();
            assert_eq!(parse(&devin).unwrap().1, Harness::Devin);
            assert_eq!(file_name(&task, Harness::Claude, "ses_0"), None);
            assert_eq!(file_name(&task, Harness::OpenCode, "../x"), None);
            assert_eq!(file_name("abc", Harness::Devin, "otter"), None);
            assert_eq!(parse(&format!("{task}.1.atif.jsonl")), None);
            assert_eq!(
                parse(&format!("{task}.delegate.opencode.ses_a.b.jsonl")),
                None
            );
            assert_eq!(parse(&format!("{task}.delegate.devin.Otter.jsonl")), None);
        }
    }
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

#[cfg(all(feature = "devin", any(target_os = "linux", target_os = "macos")))]
pub mod devin;
#[cfg(feature = "host")]
mod host;
#[cfg(all(feature = "opencode", any(target_os = "linux", target_os = "macos")))]
pub mod opencode;
#[cfg(feature = "host")]
pub use host::{Config, History};
