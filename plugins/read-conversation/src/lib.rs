//! Read one foreign conversation back, as a `packet-v0` guest plugin.
//!
//! The content half of what `foreign-sessions` deliberately left out
//! (OpenAgentsInc/openagents#41): given a source and a session id — or just
//! "the newest session here" — locate the session through the scanner and
//! return its conversation as ordered turns, role and text, oldest first.
//! Besides the JSONL stores (claude, codex), the SQLite-backed stores are
//! readable on request (OpenAgentsInc/openagents#48): `opencode` sessions
//! out of `opencode.db`, and `devin` sessions out of Devin's per-session
//! ACP message databases — both through the scanner's page-at-a-time
//! SQLite reader, with the write-ahead log disclosed (`wal_unread`) rather
//! than parsed.
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

use foreign_sessions::sqlite::{self, MountedFile, Sqlite, Value};
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
/// Page reads allowed against one SQLite-backed store (opencode, devin).
const SQLITE_PAGE_BUDGET: usize = 20_000;
/// Payload cap for message rows and probe reads of part rows: enough for a
/// role and a `"type"` prefix; text parts that prove larger are re-read.
const ROW_PROBE_CAP: usize = 16_384;
/// Payload cap for a part row whose probe said it holds conversation text.
const TEXT_ROW_CAP: usize = 1_048_576;

#[derive(Deserialize)]
pub struct Input {
    /// `claude`, `codex`, `opencode`, or `devin`; the claude and codex
    /// stores are searched when absent.
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
    /// True when the store's SQLite write-ahead log held bytes: turns not
    /// yet checkpointed may be missing from this read, and the reader never
    /// parses the WAL — recent activity is disclosed as unread, not faked.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub wal_unread: bool,
    /// True when the SQLite page-read budget ran out before the whole
    /// conversation was examined; counts and turns describe what was read.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub read_budget_exhausted: bool,
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

/// The whole read, over any [`Host`] whose `read_range` answers — the
/// JSONL stores need it for oversized files, the SQLite stores for pages.
pub fn read_conversation(host: &dyn Host, input: &Input) -> Result<Output, Refusal> {
    let session = choose(host, input)?;
    match session.source {
        "opencode" => return opencode_read(host, session, input),
        "devin" => return devin_read(host, session, input),
        _ => {}
    }
    let (bytes, tail_only) = load(host, &session)?;
    let bytes_read = bytes.len();

    let (raw_turns, records_seen, skipped) = match session.source {
        "claude" => claude_turns(&bytes),
        _ => codex_turns(&bytes),
    };

    let (turns, turns_total, dropped) = apply_ceilings(raw_turns, input);

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
        wal_unread: session.wal_unread,
        read_budget_exhausted: false,
    })
}

/// The shared turn and character ceilings: keep the end of the
/// conversation, elide long turns in the middle, and say what was dropped.
fn apply_ceilings(
    raw_turns: Vec<(String, String)>,
    input: &Input,
) -> (Vec<Turn>, usize, usize) {
    let max_turns = input.max_turns.unwrap_or(DEFAULT_MAX_TURNS).clamp(1, TURN_CAP);
    let max_chars = input.max_chars.unwrap_or(DEFAULT_MAX_CHARS).clamp(200, CHAR_CAP);
    let turns_total = raw_turns.len();
    let dropped = turns_total.saturating_sub(max_turns);
    let turns = raw_turns
        .into_iter()
        .skip(dropped)
        .map(|(role, text)| bounded_turn(role, &text, max_chars))
        .collect();
    (turns, turns_total, dropped)
}

/// The session to read: the id match when one is asked for, the newest
/// session the scanner reports otherwise.
fn choose(host: &dyn Host, input: &Input) -> Result<Session, Refusal> {
    let scan_input = ScanInput {
        sources: input.source.as_ref().map(|s| vec![s.clone()]),
        cwd_filter: input.cwd_filter.clone(),
        max_age_days: Some(365.0),
        limit: Some(50),
        now_ms: input.now_ms,
    };
    let scanned = scan(host, &scan_input)?;

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

/// What one opencode part row turned out to hold.
enum PartSort {
    /// Conversation text; `None` when the probe cap cut it and the row
    /// must be re-read with the text cap.
    Text(Option<String>),
    /// A non-text part kind (`tool`, `reasoning`, `step-start`, …).
    Kind(String),
    Unknown,
}

/// Map an `Err` from the SQLite reader to honest truncation when the
/// page-read budget ran out, and a real refusal otherwise.
fn soft_budget(
    outcome: Result<(), Refusal>,
    reader: &Sqlite,
    budget_out: &mut bool,
) -> Result<(), Refusal> {
    match outcome {
        Ok(()) => Ok(()),
        Err(_) if reader.budget_exhausted => {
            *budget_out = true;
            Ok(())
        }
        Err(refusal) => Err(refusal),
    }
}

/// One opencode conversation out of `opencode.db`: the session's `message`
/// rows (roles) joined with their `part` rows (text), ordered oldest first.
/// Indexes are used when the schema has them — the real store is far past
/// any full-scan budget — with a bounded full scan as the fallback for
/// small or differently-migrated databases.
fn opencode_read(host: &dyn Host, session: Session, input: &Input) -> Result<Output, Refusal> {
    let file = MountedFile { host, path: "opencode.db".to_string() };
    let mut reader = Sqlite::open(&file, SQLITE_PAGE_BUDGET)?;
    let master = reader.master()?;
    let table = |name: &str| master.iter().find(|e| e.kind == "table" && e.name == name);
    let message = table("message")
        .ok_or_else(|| Refusal::unsupported("the opencode database has no message table"))?;
    let part = table("part")
        .ok_or_else(|| Refusal::unsupported("the opencode database has no part table"))?;
    let position = |columns: &[String], name: &str| columns.iter().position(|c| c == name);
    let (Some(m_sess), Some(m_id), Some(m_time), Some(m_data)) = (
        position(&message.columns, "session_id"),
        position(&message.columns, "id"),
        position(&message.columns, "time_created"),
        position(&message.columns, "data"),
    ) else {
        return Err(Refusal::unsupported(
            "the opencode message table is missing expected columns",
        ));
    };
    let (Some(p_msg), Some(p_id), Some(p_sess), Some(p_data)) = (
        position(&part.columns, "message_id"),
        position(&part.columns, "id"),
        position(&part.columns, "session_id"),
        position(&part.columns, "data"),
    ) else {
        return Err(Refusal::unsupported("the opencode part table is missing expected columns"));
    };
    let index_on = |tbl: &str, first: &str| {
        master.iter().find(|e| {
            e.kind == "index"
                && e.tbl_name == tbl
                && e.columns.first().map(String::as_str) == Some(first)
        })
    };

    let sid = session.session_id.clone();
    let mut budget_out = false;

    // The session's messages: `(time_created, message id, rowid)`.
    let mut messages: Vec<(i64, String, i64)> = Vec::new();
    let outcome = match index_on("message", "session_id") {
        Some(index) => {
            let time_at = position(&index.columns, "time_created");
            let id_at = position(&index.columns, "id");
            reader.index_scan_eq(index.rootpage, &sid, 4_096, &mut |values| {
                let Some(Value::Int(rowid)) = values.last() else {
                    return true;
                };
                let time =
                    time_at.and_then(|at| values.get(at)).and_then(Value::as_int).unwrap_or(0);
                let id = id_at
                    .and_then(|at| values.get(at))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                messages.push((time, id, *rowid));
                true
            })
        }
        None => reader.scan_table(message.rootpage, ROW_PROBE_CAP, &mut |rowid, values| {
            if values.get(m_sess).and_then(Value::as_str) == Some(sid.as_str()) {
                let time = values.get(m_time).and_then(Value::as_int).unwrap_or(0);
                let id =
                    values.get(m_id).and_then(Value::as_str).unwrap_or("").to_string();
                messages.push((time, id, rowid));
            }
            true
        }),
    };
    soft_budget(outcome, &reader, &mut budget_out)?;
    messages.sort();

    // Without a per-message index, one bounded scan of the part table
    // groups the session's parts by message.
    let part_index = index_on("part", "message_id");
    let mut grouped: Option<std::collections::BTreeMap<String, Vec<(String, i64)>>> = None;
    if part_index.is_none() && !budget_out {
        let mut map = std::collections::BTreeMap::<String, Vec<(String, i64)>>::new();
        let outcome = reader.scan_table(part.rootpage, 512, &mut |rowid, values| {
            if values.get(p_sess).and_then(Value::as_str) == Some(sid.as_str()) {
                let mid = values.get(p_msg).and_then(Value::as_str).unwrap_or("").to_string();
                let pid = values.get(p_id).and_then(Value::as_str).unwrap_or("").to_string();
                map.entry(mid).or_default().push((pid, rowid));
            }
            true
        });
        soft_budget(outcome, &reader, &mut budget_out)?;
        grouped = Some(map);
    }

    let mut raw_turns: Vec<(String, String)> = Vec::new();
    let mut skipped = Skipped::default();
    let mut records = 0usize;
    'messages: for (_time, mid, rowid) in &messages {
        if budget_out {
            break;
        }
        let row = match reader.find_by_rowid(message.rootpage, *rowid, ROW_PROBE_CAP) {
            Ok(Some(values)) => values,
            Ok(None) => {
                skipped.other += 1;
                continue;
            }
            Err(refusal) => {
                soft_budget(Err(refusal), &reader, &mut budget_out)?;
                break;
            }
        };
        records += 1;
        let mid = if mid.is_empty() {
            row.get(m_id).and_then(Value::as_str).unwrap_or("").to_string()
        } else {
            mid.clone()
        };
        let role = json_str_of(row.get(m_data), "role").unwrap_or_else(|| "unknown".to_string());

        let mut part_rows: Vec<(String, i64)> = Vec::new();
        if let Some(map) = &grouped {
            part_rows = map.get(&mid).cloned().unwrap_or_default();
        } else if let Some(index) = part_index {
            let id_at = position(&index.columns, "id");
            let outcome = reader.index_scan_eq(index.rootpage, &mid, 4_096, &mut |values| {
                let Some(Value::Int(rowid)) = values.last() else {
                    return true;
                };
                let pid = id_at
                    .and_then(|at| values.get(at))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                part_rows.push((pid, *rowid));
                true
            });
            soft_budget(outcome, &reader, &mut budget_out)?;
        }
        part_rows.sort();

        let mut texts: Vec<String> = Vec::new();
        for (_pid, prowid) in &part_rows {
            if budget_out {
                break 'messages;
            }
            let part_row = match reader.find_by_rowid(part.rootpage, *prowid, ROW_PROBE_CAP) {
                Ok(Some(values)) => values,
                Ok(None) => {
                    skipped.other += 1;
                    continue;
                }
                Err(refusal) => {
                    soft_budget(Err(refusal), &reader, &mut budget_out)?;
                    break 'messages;
                }
            };
            records += 1;
            let sort = match part_row.get(p_data) {
                Some(Value::Text(json)) => match serde_json::from_str::<serde_json::Value>(json) {
                    Ok(value) => {
                        let kind =
                            value.get("type").and_then(|t| t.as_str()).unwrap_or("").to_string();
                        if kind == "text" {
                            PartSort::Text(
                                value.get("text").and_then(|t| t.as_str()).map(str::to_string),
                            )
                        } else {
                            PartSort::Kind(kind)
                        }
                    }
                    Err(_) => PartSort::Unknown,
                },
                Some(Value::Truncated(prefix)) => match sqlite::sniff_json_str(prefix, "type") {
                    Some(kind) if kind == "text" => PartSort::Text(None),
                    Some(kind) => PartSort::Kind(kind),
                    None => PartSort::Unknown,
                },
                _ => PartSort::Unknown,
            };
            match sort {
                PartSort::Text(Some(text)) => texts.push(text),
                // The probe cap cut a text part; one re-read with the text
                // cap recovers it (overflow pages and all).
                PartSort::Text(None) => {
                    match reader.find_by_rowid(part.rootpage, *prowid, TEXT_ROW_CAP) {
                        Ok(Some(full)) => {
                            let text = full
                                .get(p_data)
                                .and_then(Value::as_str)
                                .and_then(|json| {
                                    serde_json::from_str::<serde_json::Value>(json).ok()
                                })
                                .and_then(|v| {
                                    v.get("text").and_then(|t| t.as_str()).map(str::to_string)
                                });
                            match text {
                                Some(text) => texts.push(text),
                                None => skipped.other += 1,
                            }
                        }
                        Ok(None) => skipped.other += 1,
                        Err(refusal) => {
                            soft_budget(Err(refusal), &reader, &mut budget_out)?;
                            break 'messages;
                        }
                    }
                }
                PartSort::Kind(kind) => match kind.as_str() {
                    "reasoning" => skipped.thinking += 1,
                    "tool" | "step-start" | "step-finish" | "patch" => {
                        skipped.tool_activity += 1
                    }
                    _ => skipped.other += 1,
                },
                PartSort::Unknown => skipped.other += 1,
            }
        }
        let joined = texts.join("\n").trim().to_string();
        if !joined.is_empty() {
            raw_turns.push((role, joined));
        }
    }

    let (turns, turns_total, dropped) = apply_ceilings(raw_turns, input);
    Ok(Output {
        source: "opencode".to_string(),
        session_id: session.session_id,
        path: session.path,
        cwd: session.cwd,
        file_bytes: session.size_bytes,
        bytes_read: usize::try_from(reader.bytes_read).unwrap_or(usize::MAX),
        tail_only: false,
        records_seen: records,
        turns_total,
        dropped_leading_turns: dropped,
        turns,
        skipped,
        wal_unread: session.wal_unread,
        read_budget_exhausted: budget_out,
    })
}

/// One Devin conversation out of its per-session ACP message database:
/// `messages(position, kind, payload)` in position order. `user_message`
/// and `agent_message` rows become turns; thoughts and tool calls are
/// counted, not replayed.
fn devin_read(host: &dyn Host, session: Session, input: &Input) -> Result<Output, Refusal> {
    let file = MountedFile { host, path: session.path.clone() };
    let mut reader = Sqlite::open(&file, SQLITE_PAGE_BUDGET)?;
    let master = reader.master()?;
    let table = master
        .iter()
        .find(|e| e.kind == "table" && e.name == "messages")
        .ok_or_else(|| Refusal::unsupported("the devin database has no messages table"))?;
    let position = |name: &str| table.columns.iter().position(|c| c == name);
    let (Some(kind_at), Some(payload_at)) = (position("kind"), position("payload")) else {
        return Err(Refusal::unsupported(
            "the devin messages table is missing expected columns",
        ));
    };

    let mut budget_out = false;
    // `(rowid, kind, payload)` in position order — `position` is the
    // table's INTEGER PRIMARY KEY, so rowid order is message order. Only
    // message kinds keep their payload; the rest are counted by kind.
    let mut rows: Vec<(i64, String, Value)> = Vec::new();
    let outcome = reader.scan_table(table.rootpage, ROW_PROBE_CAP, &mut |rowid, values| {
        let kind = values.get(kind_at).and_then(Value::as_str).unwrap_or("").to_string();
        let payload = if kind == "user_message" || kind == "agent_message" {
            values.get(payload_at).cloned().unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        rows.push((rowid, kind, payload));
        true
    });
    soft_budget(outcome, &reader, &mut budget_out)?;

    let mut raw_turns: Vec<(String, String)> = Vec::new();
    let mut skipped = Skipped::default();
    let records = rows.len();
    for (rowid, kind, payload) in rows {
        match kind.as_str() {
            "user_message" | "agent_message" => {
                let role = if kind == "user_message" { "user" } else { "assistant" };
                let json = match payload {
                    Value::Text(json) => Some(json),
                    // The probe cap cut the payload; re-read with the text cap.
                    Value::Truncated(_) => {
                        match reader.find_by_rowid(table.rootpage, rowid, TEXT_ROW_CAP) {
                            Ok(Some(full)) => full
                                .get(payload_at)
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            Ok(None) => None,
                            Err(refusal) => {
                                soft_budget(Err(refusal), &reader, &mut budget_out)?;
                                break;
                            }
                        }
                    }
                    _ => None,
                };
                match json.as_deref().and_then(devin_text) {
                    Some(text) if !text.is_empty() => raw_turns.push((role.to_string(), text)),
                    Some(_) => {}
                    None => skipped.other += 1,
                }
            }
            "agent_thought" => skipped.thinking += 1,
            "tool_call" => skipped.tool_activity += 1,
            _ => skipped.other += 1,
        }
    }

    let (turns, turns_total, dropped) = apply_ceilings(raw_turns, input);
    Ok(Output {
        source: "devin".to_string(),
        session_id: session.session_id,
        path: session.path,
        cwd: session.cwd,
        file_bytes: session.size_bytes,
        bytes_read: usize::try_from(reader.bytes_read).unwrap_or(usize::MAX),
        tail_only: false,
        records_seen: records,
        turns_total,
        dropped_leading_turns: dropped,
        turns,
        skipped,
        wal_unread: session.wal_unread,
        read_budget_exhausted: budget_out,
    })
}

/// A named string field out of a JSON column value — parsed when the value
/// is whole, sniffed from the prefix when the payload cap cut it.
fn json_str_of(value: Option<&Value>, field: &str) -> Option<String> {
    match value {
        Some(Value::Text(json)) => serde_json::from_str::<serde_json::Value>(json)
            .ok()
            .and_then(|v| v.get(field).and_then(|f| f.as_str()).map(str::to_string)),
        Some(Value::Truncated(prefix)) => sqlite::sniff_json_str(prefix, field),
        _ => None,
    }
}

/// The joined text blocks of a Devin ACP payload: `content[]` entries whose
/// inner `content` is `{"type":"text","text":…}`.
fn devin_text(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let content = value.get("content")?.as_array()?;
    let mut parts: Vec<&str> = Vec::new();
    for block in content {
        let Some(inner) = block.get("content") else {
            continue;
        };
        if inner.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(text) = inner.get("text").and_then(|t| t.as_str()) {
                parts.push(text);
            }
        }
    }
    Some(parts.join("\n").trim().to_string())
}

fn handle(input: Input) -> Result<Output, Refusal> {
    read_conversation(&RealHost, &input)
}

plugin_entry!(handle);

#[cfg(test)]
mod tests;
