//! Read one foreign conversation back, as a `packet-v0` guest plugin.
//!
//! The content half of what `foreign-sessions` deliberately left out
//! (OpenAgentsInc/openagents#41): given a source and a session id — or just
//! "the newest session here" — locate the session through the scanner and
//! return its conversation as ordered turns, role and text, oldest first.
//!
//! The posture is the scanner's: read-only through the host's confined
//! capability imports, bounded everywhere, fail-soft on malformed records.
//! A file past the host's whole-file bound is read from its tail through
//! the bounded range import, and the output says so (`tail_only`), says how
//! many turns the ceilings dropped, and counts what it skipped (thinking,
//! tool activity, other records) — a truncated read that names what it left
//! out rather than pretending to be whole.
//!
//! Reading is the whole capability. Nothing here resumes, continues, or
//! writes anything.

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
const DEFAULT_MAX_TURNS: usize = 60;
const TURN_CAP: usize = 200;
const DEFAULT_MAX_CHARS: usize = 2_000;
const CHAR_CAP: usize = 8_000;

#[derive(Deserialize)]
pub struct Input {
    /// `claude` or `codex`; both are searched when absent.
    #[serde(default)]
    pub source: Option<String>,
    /// The session to read, by id or id prefix. Absent, the newest wins.
    #[serde(default)]
    pub session_id: Option<String>,
    /// Substring the session's working directory must contain.
    #[serde(default)]
    pub cwd_filter: Option<String>,
    /// Most turns to return, from the end of the conversation. Default 60.
    #[serde(default)]
    pub max_turns: Option<usize>,
    /// Character ceiling per turn; longer turns keep head and tail. Default 2000.
    #[serde(default)]
    pub max_chars: Option<usize>,
    /// Milliseconds since the Unix epoch, handed through to the scanner.
    #[serde(default)]
    pub now_ms: Option<i64>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Turn {
    pub role: String,
    pub text: String,
    /// True when the character ceiling elided the middle of this turn.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct Skipped {
    /// Reasoning blocks, which are thoughts rather than the conversation.
    pub thinking: usize,
    /// Tool calls and tool results, counted rather than replayed.
    pub tool_activity: usize,
    /// Records of any other kind, malformed lines included.
    pub other: usize,
}

#[derive(Debug, Serialize)]
pub struct Output {
    pub source: String,
    pub session_id: String,
    /// Path relative to the source's mount root.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub file_bytes: u64,
    pub bytes_read: usize,
    /// True when the file exceeded the whole-read bound, so only its tail
    /// was read and earlier turns are not merely dropped but unseen.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub tail_only: bool,
    /// JSONL records inspected in what was read.
    pub records_seen: usize,
    /// Conversation turns found in what was read, before the turn ceiling.
    pub turns_total: usize,
    /// Turns the ceiling dropped from the front of what was read.
    pub dropped_leading_turns: usize,
    pub turns: Vec<Turn>,
    pub skipped: Skipped,
}

/// The scanner's host plus the bounded range read this plugin adds.
pub trait RangeHost: Host {
    fn read_range(&self, path: &str, offset: u64, max_bytes: u32) -> Result<Vec<u8>, Refusal>;
}

struct RealHost;

impl Host for RealHost {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        list_mounted_dir(mount_index, path)
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        read_mounted_file(path)
    }
}

impl RangeHost for RealHost {
    fn read_range(&self, path: &str, offset: u64, max_bytes: u32) -> Result<Vec<u8>, Refusal> {
        read_mounted_file_range(path, offset, max_bytes)
    }
}

/// The whole read, over any [`RangeHost`].
pub fn read_conversation(host: &dyn RangeHost, input: &Input) -> Result<Output, Refusal> {
    let session = choose(host, input)?;
    let (bytes, tail_only) = load(host, &session)?;
    let bytes_read = bytes.len();

    let (raw_turns, records_seen, skipped) = match session.source {
        "claude" => claude_turns(&bytes),
        _ => codex_turns(&bytes),
    };

    let max_turns = input.max_turns.unwrap_or(DEFAULT_MAX_TURNS).clamp(1, TURN_CAP);
    let max_chars = input.max_chars.unwrap_or(DEFAULT_MAX_CHARS).clamp(200, CHAR_CAP);
    let turns_total = raw_turns.len();
    let dropped = turns_total.saturating_sub(max_turns);
    let turns = raw_turns
        .into_iter()
        .skip(dropped)
        .map(|(role, text)| bounded_turn(role, &text, max_chars))
        .collect();

    Ok(Output {
        source: session.source.to_string(),
        session_id: session.session_id,
        path: session.path,
        cwd: session.cwd,
        file_bytes: session.size_bytes,
        bytes_read,
        tail_only,
        records_seen,
        turns_total,
        dropped_leading_turns: dropped,
        turns,
        skipped,
    })
}

/// The session to read: the id match when one is asked for, the newest
/// session the scanner reports otherwise.
fn choose(host: &dyn RangeHost, input: &Input) -> Result<Session, Refusal> {
    let scan_input = ScanInput {
        sources: input.source.as_ref().map(|s| vec![s.clone()]),
        cwd_filter: input.cwd_filter.clone(),
        max_age_days: Some(365.0),
        limit: Some(50),
        now_ms: input.now_ms,
    };
    // `RangeHost: Host`, and dyn upcasting is stable, so the scanner
    // takes the same host value.
    let scanned = scan(host as &dyn Host, &scan_input)?;

    match &input.session_id {
        Some(want) => scanned
            .sessions
            .into_iter()
            .find(|s| s.session_id.starts_with(want.as_str()) || s.path.contains(want.as_str()))
            .ok_or_else(|| {
                Refusal::unsupported(format!(
                    "no recent session matches `{want}`; ask foreign_sessions what exists"
                ))
            }),
        None => scanned.sessions.into_iter().next().ok_or_else(|| {
            Refusal::unsupported(
                "no recent foreign sessions were found; nothing to read".to_string(),
            )
        }),
    }
}

/// The session's bytes: whole when the file fits the host's bound, the tail
/// (aligned to the first whole line) when it does not.
fn load(host: &dyn RangeHost, session: &Session) -> Result<(Vec<u8>, bool), Refusal> {
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

/// One turn, elided in the middle when it is past the ceiling.
fn bounded_turn(role: String, text: &str, max_chars: usize) -> Turn {
    let count = text.chars().count();
    if count <= max_chars {
        return Turn { role, text: text.to_string(), truncated: false };
    }
    let half = max_chars / 2;
    let head: String = text.chars().take(half).collect();
    let tail: String = text
        .chars()
        .skip(count - half)
        .collect();
    Turn {
        role,
        text: format!("{head}\n…[{} characters elided]…\n{tail}", count - half * 2),
        truncated: true,
    }
}

/// Claude Code records: `user` / `assistant` with `message.content` as a
/// string or a block list. Text blocks are the conversation; thinking and
/// tool blocks are counted, not replayed.
pub fn claude_turns(bytes: &[u8]) -> (Vec<(String, String)>, usize, Skipped) {
    let text = String::from_utf8_lossy(bytes);
    let mut turns = Vec::new();
    let mut skipped = Skipped::default();
    let mut records = 0usize;

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        records += 1;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            skipped.other += 1;
            continue;
        };
        let kind = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "user" && kind != "assistant" {
            skipped.other += 1;
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
                    match block.get("type").and_then(|v| v.as_str()) {
                        Some("text") => {
                            if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                                parts.push(t.to_string());
                            }
                        }
                        Some("thinking") => skipped.thinking += 1,
                        Some("tool_use") | Some("tool_result") => skipped.tool_activity += 1,
                        _ => skipped.other += 1,
                    }
                }
            }
            _ => skipped.other += 1,
        }
        let joined = parts.join("\n").trim().to_string();
        if !joined.is_empty() {
            turns.push((role, joined));
        }
    }
    (turns, records, skipped)
}

/// Codex rollout records: `response_item` messages carry the conversation;
/// reasoning and tool items are counted, everything else is other.
pub fn codex_turns(bytes: &[u8]) -> (Vec<(String, String)>, usize, Skipped) {
    let text = String::from_utf8_lossy(bytes);
    let mut turns = Vec::new();
    let mut skipped = Skipped::default();
    let mut records = 0usize;

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        records += 1;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            skipped.other += 1;
            continue;
        };
        if value.get("type").and_then(|v| v.as_str()) != Some("response_item") {
            skipped.other += 1;
            continue;
        }
        let Some(payload) = value.get("payload") else {
            skipped.other += 1;
            continue;
        };
        match payload.get("type").and_then(|v| v.as_str()) {
            Some("message") => {
                let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
                if role != "user" && role != "assistant" {
                    skipped.other += 1;
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
            Some("reasoning") => skipped.thinking += 1,
            // `function_call`, `custom_tool_call`, and their `_output` twins.
            Some(kind) if kind.contains("call") => skipped.tool_activity += 1,
            _ => skipped.other += 1,
        }
    }
    (turns, records, skipped)
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
    read_conversation(&RealHost, &input)
}

plugin_entry!(handle);

#[cfg(test)]
mod tests;
