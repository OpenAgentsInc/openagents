//! Search every foreign conversation on the machine, as a `packet-v0`
//! guest plugin.
//!
//! The retrieval half the scanner and the reader left open
//! (OpenAgentsInc/openagents#46): given a query, find where any Claude
//! Code or Codex CLI session on this machine talked about it. The
//! `foreign-sessions` scanner enumerates candidate sessions, newest
//! first; each one is read back, its conversation text extracted the way
//! `read_conversation` extracts it (user and assistant text only —
//! thinking and tool payloads are not searched), and the query matched
//! case-insensitively against each turn. A hit reports its role and a
//! bounded window of surrounding context.
//!
//! The posture is the scanner's: read-only through the host's confined
//! capability imports, bounded everywhere, fail-soft on unreadable or
//! malformed files. A session file past the host's whole-file bound is
//! searched from its tail through the bounded range import and marked
//! `tail_only`; when the session budget cuts the candidate list short,
//! the output says `truncated` rather than pretending the search was
//! exhaustive.
//!
//! Searching is the whole capability. Nothing here resumes, continues,
//! or writes anything.

use foreign_sessions::{scan, Host, Input as ScanInput, Session};
use openagents_pdk::{
    list_mounted_dir, plugin_entry, read_mounted_file, read_mounted_file_range, MountDirListing,
    Refusal,
};
use serde::{Deserialize, Serialize};

/// Mirror of the host's per-read bound; a whole-file read past it refuses.
const WHOLE_READ_BOUND: u64 = 1_048_576;
/// How much of an oversized file's tail one read asks for.
const TAIL_BYTES: u32 = 1_048_576;
const DEFAULT_MAX_AGE_DAYS: f64 = 90.0;
const DEFAULT_MAX_SESSIONS: usize = 20;
const SESSION_CAP: usize = 40;
const DEFAULT_HITS_PER_SESSION: usize = 3;
const HITS_CAP: usize = 10;
const DEFAULT_CONTEXT_CHARS: usize = 240;
const CONTEXT_CAP: usize = 1_000;
const CONTEXT_FLOOR: usize = 40;

#[derive(Deserialize)]
pub struct Input {
    /// What to search for. Required; must be non-empty after trimming.
    #[serde(default)]
    pub query: String,
    /// Which stores to search; both when absent.
    #[serde(default)]
    pub sources: Option<Vec<String>>,
    /// Substring the session's working directory must contain.
    #[serde(default)]
    pub cwd_filter: Option<String>,
    /// Sessions older than this are not searched. Default 90.
    #[serde(default)]
    pub max_age_days: Option<f64>,
    /// Most sessions to search, newest first. Default 20, capped at 40.
    #[serde(default)]
    pub max_sessions: Option<usize>,
    /// Most hits reported per session. Default 3, capped at 10.
    #[serde(default)]
    pub max_hits_per_session: Option<usize>,
    /// Characters kept around each hit. Default 240, capped at 1000.
    #[serde(default)]
    pub context_chars: Option<usize>,
    /// Milliseconds since the Unix epoch, handed through to the scanner.
    #[serde(default)]
    pub now_ms: Option<i64>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Hit {
    /// The role whose turn held the match: `user` or `assistant`.
    pub role: String,
    /// The match with up to half the context budget on each side, elision
    /// markers where the turn continues beyond the window.
    pub context: String,
}

#[derive(Debug, Serialize)]
pub struct SessionMatch {
    pub source: String,
    pub session_id: String,
    /// Path relative to the source's mount root.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub mtime_ms: i64,
    /// True when the file exceeded the whole-read bound, so only its tail
    /// was searched and earlier turns went unseen.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub tail_only: bool,
    /// The first hits, in conversation order, up to the per-session cap.
    pub hits: Vec<Hit>,
    /// Every occurrence found in this session, the reported ones included.
    pub hits_total: usize,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub query: String,
    /// Sessions whose content was actually read and searched.
    pub sessions_searched: usize,
    pub sessions_matched: usize,
    /// Matching sessions, newest first.
    pub matches: Vec<SessionMatch>,
    /// True when the session budget or the scanner's own bounds cut the
    /// candidate list short; more sessions may exist than were searched.
    pub truncated: bool,
    /// Candidate sessions the host refused to read.
    pub skipped_unreadable: usize,
}

struct RealHost;

impl Host for RealHost {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        list_mounted_dir(mount_index, path)
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        read_mounted_file(path)
    }
    fn read_range(&self, path: &str, offset: u64, max_bytes: u32) -> Result<Vec<u8>, Refusal> {
        read_mounted_file_range(path, offset, max_bytes)
    }
}

/// The whole search, over any [`Host`].
pub fn search_sessions(host: &dyn Host, input: &Input) -> Result<Output, Refusal> {
    let query = input.query.trim();
    if query.is_empty() {
        return Err(Refusal::unsupported(
            "the query is empty; pass a non-empty word or phrase in `query` to search for",
        ));
    }
    let query_lower = query.to_lowercase();

    let max_sessions = input.max_sessions.unwrap_or(DEFAULT_MAX_SESSIONS).clamp(1, SESSION_CAP);
    let max_hits = input
        .max_hits_per_session
        .unwrap_or(DEFAULT_HITS_PER_SESSION)
        .clamp(1, HITS_CAP);
    let context_chars = input
        .context_chars
        .unwrap_or(DEFAULT_CONTEXT_CHARS)
        .clamp(CONTEXT_FLOOR, CONTEXT_CAP);
    let max_age_days = input
        .max_age_days
        .filter(|days| days.is_finite() && *days > 0.0)
        .unwrap_or(DEFAULT_MAX_AGE_DAYS);

    // One candidate past the budget, so a full page distinguishes "that was
    // everything" from "the budget cut the list short".
    let scan_input = ScanInput {
        sources: input.sources.clone(),
        cwd_filter: input.cwd_filter.clone(),
        max_age_days: Some(max_age_days),
        limit: Some(max_sessions + 1),
        now_ms: input.now_ms,
    };
    // `RangeHost: Host`, and dyn upcasting is stable, so the scanner takes
    // the same host value.
    let scanned = scan(host as &dyn Host, &scan_input)?;

    let mut truncated = scanned.scan_truncated
        || scanned.read_budget_exhausted
        || scanned.sessions.len() > max_sessions;
    let mut sessions_searched = 0usize;
    let mut skipped_unreadable = 0usize;
    let mut matches = Vec::new();

    for session in scanned.sessions.into_iter().take(max_sessions) {
        let (bytes, tail_only) = match load(host, &session) {
            Ok(loaded) => loaded,
            Err(_) => {
                skipped_unreadable += 1;
                continue;
            }
        };
        sessions_searched += 1;

        let turns = match session.source {
            "claude" => claude_turns(&bytes),
            _ => codex_turns(&bytes),
        };

        let mut hits = Vec::new();
        let mut hits_total = 0usize;
        for (role, text) in &turns {
            let found = occurrences(text, &query_lower);
            hits_total += found.len();
            for (start, len) in found {
                if hits.len() < max_hits {
                    hits.push(Hit {
                        role: role.clone(),
                        context: context_window(text, start, len, context_chars),
                    });
                }
            }
        }
        if hits_total > 0 {
            matches.push(SessionMatch {
                source: session.source.to_string(),
                session_id: session.session_id,
                path: session.path,
                cwd: session.cwd,
                mtime_ms: session.mtime_ms,
                tail_only,
                hits,
                hits_total,
            });
        }
    }

    // An unreadable candidate is also an incomplete search.
    if skipped_unreadable > 0 {
        truncated = true;
    }

    Ok(Output {
        query: query.to_string(),
        sessions_searched,
        sessions_matched: matches.len(),
        matches,
        truncated,
        skipped_unreadable,
    })
}

/// The session's bytes: whole when the file fits the host's bound, the tail
/// (aligned to the first whole line) when it does not.
fn load(host: &dyn Host, session: &Session) -> Result<(Vec<u8>, bool), Refusal> {
    if session.size_bytes <= WHOLE_READ_BOUND {
        return Ok((host.read(&session.path)?, false));
    }
    let offset = session.size_bytes - u64::from(TAIL_BYTES);
    let bytes = host.read_range(&session.path, offset, TAIL_BYTES)?;
    let aligned = match bytes.iter().position(|b| *b == b'\n') {
        Some(at) => bytes[at + 1..].to_vec(),
        None => Vec::new(),
    };
    Ok((aligned, true))
}

/// Non-overlapping case-insensitive occurrences of the lowercased query,
/// as `(char_start, char_len)` positions in the original text. Lowercasing
/// can change a character's length, so a byte-offset map carries each
/// match back to original character positions, keeping every window cut
/// char-boundary safe.
fn occurrences(text: &str, query_lower: &str) -> Vec<(usize, usize)> {
    let mut lowered = String::with_capacity(text.len());
    // (byte offset in `lowered`, character index in `text`), ascending.
    let mut origins: Vec<(usize, usize)> = Vec::new();
    for (char_index, ch) in text.chars().enumerate() {
        origins.push((lowered.len(), char_index));
        for lower in ch.to_lowercase() {
            lowered.push(lower);
        }
    }
    let total_chars = origins.len();

    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = lowered[from..].find(query_lower) {
        let start_byte = from + at;
        let end_byte = start_byte + query_lower.len();
        let start_char = match origins.binary_search_by_key(&start_byte, |(byte, _)| *byte) {
            Ok(index) => origins[index].1,
            Err(index) => origins[index.saturating_sub(1)].1,
        };
        let end_char = match origins.binary_search_by_key(&end_byte, |(byte, _)| *byte) {
            Ok(index) => origins[index].1,
            Err(index) if index < origins.len() => origins[index].1,
            Err(_) => total_chars,
        };
        found.push((start_char, end_char.saturating_sub(start_char).max(1)));
        from = end_byte.max(start_byte + 1);
    }
    found
}

/// The hit with up to half the context budget of characters on each side,
/// elision markers where the turn continues beyond the window.
fn context_window(text: &str, start_char: usize, hit_chars: usize, context_chars: usize) -> String {
    let total = text.chars().count();
    let half = context_chars / 2;
    let from = start_char.saturating_sub(half);
    let to = (start_char + hit_chars + half).min(total);
    let window: String = text.chars().skip(from).take(to - from).collect();
    let mut out = String::new();
    if from > 0 {
        out.push('…');
    }
    out.push_str(&window);
    if to < total {
        out.push('…');
    }
    out
}

/// Claude Code records: `user` / `assistant` with `message.content` as a
/// string or a block list. Text blocks are the conversation; thinking and
/// tool payloads are not searched.
fn claude_turns(bytes: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(bytes);
    let mut turns = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "user" && kind != "assistant" {
            continue;
        }
        let message = value.get("message").unwrap_or(&serde_json::Value::Null);
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or(kind)
            .to_string();
        let mut parts: Vec<String> = Vec::new();
        match message.get("content") {
            Some(serde_json::Value::String(content)) => parts.push(content.clone()),
            Some(serde_json::Value::Array(blocks)) => {
                for block in blocks {
                    if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                        if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                            parts.push(t.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
        let joined = parts.join("\n").trim().to_string();
        if !joined.is_empty() {
            turns.push((role, joined));
        }
    }
    turns
}

/// Codex rollout records: `response_item` messages carry the conversation;
/// reasoning and tool items are not searched.
fn codex_turns(bytes: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(bytes);
    let mut turns = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("type").and_then(|v| v.as_str()) != Some("response_item") {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };
        match payload.get("type").and_then(|v| v.as_str()) {
            Some("message") => {
                let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
                if role != "user" && role != "assistant" {
                    continue;
                }
                let joined = block_text(payload.get("content"));
                if !joined.is_empty() {
                    turns.push((role.to_string(), joined));
                }
            }
            Some("agent_message") => {
                let joined = block_text(payload.get("content"));
                if !joined.is_empty() {
                    turns.push(("assistant".to_string(), joined));
                }
            }
            _ => {}
        }
    }
    turns
}

/// The text of a Codex content value: a plain string, or the joined text of
/// its `input_text` / `output_text` blocks.
fn block_text(content: Option<&serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(text)) => text.trim().to_string(),
        Some(serde_json::Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(|v| v.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

fn handle(input: Input) -> Result<Output, Refusal> {
    search_sessions(&RealHost, &input)
}

plugin_entry!(handle);

#[cfg(test)]
mod tests;
