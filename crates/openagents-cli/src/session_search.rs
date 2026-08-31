//! Search past Coder sessions, and the Claude Code / Codex stores when
//! they are on this machine.
//!
//! The guest plugin of the same name (#46) only mounted `~/.claude` and
//! `~/.codex`. Live Coder history lives at
//! `~/.openagents/sessions/<urlencoded-cwd>/<id>/updates.jsonl`. This host
//! tool is that store, plus the foreign ones when their directories exist
//! (#318). It never resumes or writes.

use serde::Serialize;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const WHOLE_READ_BOUND: u64 = 1_048_576;
const TAIL_BYTES: u64 = 1_048_576;
const DEFAULT_MAX_AGE_DAYS: f64 = 90.0;
const DEFAULT_MAX_SESSIONS: usize = 20;
const SESSION_CAP: usize = 40;
const DEFAULT_HITS_PER_SESSION: usize = 3;
const HITS_CAP: usize = 10;
const DEFAULT_CONTEXT_CHARS: usize = 240;
const CONTEXT_CAP: usize = 1_000;
const CONTEXT_FLOOR: usize = 40;
const MS_PER_DAY: f64 = 86_400_000.0;

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub query: String,
    pub sources: Vec<String>,
    pub cwd_filter: Option<String>,
    pub max_age_days: f64,
    pub max_sessions: usize,
    pub max_hits_per_session: usize,
    pub context_chars: usize,
    pub now_ms: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Hit {
    pub role: String,
    pub context: String,
}

#[derive(Debug, Serialize)]
pub struct SessionMatch {
    pub source: String,
    pub session_id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub mtime_ms: i64,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub tail_only: bool,
    pub hits: Vec<Hit>,
    pub hits_total: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchOutput {
    pub query: String,
    pub sessions_searched: usize,
    pub sessions_matched: usize,
    pub matches: Vec<SessionMatch>,
    pub truncated: bool,
    pub skipped_unreadable: usize,
}

struct Candidate {
    source: &'static str,
    session_id: String,
    path: PathBuf,
    cwd: Option<String>,
    mtime_ms: i64,
    size_bytes: u64,
}

/// Parse tool arguments. An empty query is an error string.
pub fn request_from_arguments(
    arguments: &serde_json::Value,
    session_cwd: &Path,
    now_ms: i64,
) -> Result<SearchRequest, String> {
    let query = arguments
        .get("query")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if query.is_empty() {
        return Err(
            "the query is empty; pass a non-empty word or phrase in `query` to search for"
                .to_string(),
        );
    }
    let sources = match arguments.get("sources").and_then(|value| value.as_array()) {
        None => vec![
            "coder".to_string(),
            "claude".to_string(),
            "codex".to_string(),
        ],
        Some(named) => {
            let mut sources = Vec::new();
            for value in named {
                let Some(name) = value.as_str() else {
                    return Err("every `sources` entry must be a string".to_string());
                };
                match name {
                    "coder" | "claude" | "codex" => {
                        if !sources.iter().any(|seen| seen == name) {
                            sources.push(name.to_string());
                        }
                    }
                    other => {
                        return Err(format!(
                            "unknown source `{other}`; this searcher knows `coder`, `claude`, and `codex`"
                        ));
                    }
                }
            }
            sources
        }
    };
    // Absent cwd_filter defaults to this session's cwd. An explicit empty
    // string widens to every session on the machine.
    let cwd_filter = match arguments.get("cwd_filter") {
        None => Some(session_cwd.to_string_lossy().into_owned()),
        Some(serde_json::Value::String(filter)) if filter.is_empty() => None,
        Some(serde_json::Value::String(filter)) => Some(filter.clone()),
        Some(_) => {
            return Err("`cwd_filter` must be a string".to_string());
        }
    };
    let max_age_days = arguments
        .get("max_age_days")
        .and_then(|value| value.as_f64())
        .filter(|days| days.is_finite() && *days > 0.0)
        .unwrap_or(DEFAULT_MAX_AGE_DAYS);
    let max_sessions = arguments
        .get("max_sessions")
        .and_then(|value| value.as_u64())
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_MAX_SESSIONS)
        .clamp(1, SESSION_CAP);
    let max_hits_per_session = arguments
        .get("max_hits_per_session")
        .and_then(|value| value.as_u64())
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_HITS_PER_SESSION)
        .clamp(1, HITS_CAP);
    let context_chars = arguments
        .get("context_chars")
        .and_then(|value| value.as_u64())
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_CONTEXT_CHARS)
        .clamp(CONTEXT_FLOOR, CONTEXT_CAP);
    let now_ms = arguments
        .get("now_ms")
        .and_then(|value| value.as_i64())
        .unwrap_or(now_ms);
    Ok(SearchRequest {
        query,
        sources,
        cwd_filter,
        max_age_days,
        max_sessions,
        max_hits_per_session,
        context_chars,
        now_ms,
    })
}

/// Search under `home` (`$HOME`). `session_cwd` is only used when building
/// the default filter, already applied in [`request_from_arguments`].
pub fn search(home: &Path, request: &SearchRequest) -> Result<SearchOutput, String> {
    let query_lower = request.query.to_lowercase();
    let cutoff = request.now_ms - (request.max_age_days * MS_PER_DAY) as i64;
    let mut candidates = Vec::new();
    let mut truncated = false;
    for source in &request.sources {
        match source.as_str() {
            "coder" => collect_coder(home, request.cwd_filter.as_deref(), cutoff, &mut candidates),
            "claude" => {
                collect_claude(home, request.cwd_filter.as_deref(), cutoff, &mut candidates)
            }
            "codex" => collect_codex(home, request.cwd_filter.as_deref(), cutoff, &mut candidates),
            _ => {}
        }
    }
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.mtime_ms));
    if candidates.len() > request.max_sessions {
        truncated = true;
        candidates.truncate(request.max_sessions);
    }

    let mut sessions_searched = 0usize;
    let mut skipped_unreadable = 0usize;
    let mut matches = Vec::new();
    for candidate in candidates {
        let (bytes, tail_only) = match load_bytes(&candidate.path, candidate.size_bytes) {
            Ok(loaded) => loaded,
            Err(_) => {
                skipped_unreadable += 1;
                truncated = true;
                continue;
            }
        };
        sessions_searched += 1;
        let turns = match candidate.source {
            "coder" => coder_turns(&bytes),
            "claude" => claude_turns(&bytes),
            _ => codex_turns(&bytes),
        };
        let mut hits = Vec::new();
        let mut hits_total = 0usize;
        for (role, text) in &turns {
            let found = occurrences(text, &query_lower);
            hits_total += found.len();
            for (start, len) in found {
                if hits.len() < request.max_hits_per_session {
                    hits.push(Hit {
                        role: role.clone(),
                        context: context_window(text, start, len, request.context_chars),
                    });
                }
            }
        }
        if hits_total > 0 {
            matches.push(SessionMatch {
                source: candidate.source.to_string(),
                session_id: candidate.session_id,
                path: candidate.path.display().to_string(),
                cwd: candidate.cwd,
                mtime_ms: candidate.mtime_ms,
                tail_only,
                hits,
                hits_total,
            });
        }
    }
    Ok(SearchOutput {
        query: request.query.clone(),
        sessions_searched,
        sessions_matched: matches.len(),
        matches,
        truncated,
        skipped_unreadable,
    })
}

/// Run the tool: JSON on success, error string on a refused query.
pub fn run(home: &Path, session_cwd: &Path, arguments: &serde_json::Value) -> (String, bool) {
    let now_ms = now_ms();
    match request_from_arguments(arguments, session_cwd, now_ms) {
        Err(why) => (why, true),
        Ok(request) => match search(home, &request) {
            Ok(output) => (
                serde_json::to_string(&output).unwrap_or_else(|err| err.to_string()),
                false,
            ),
            Err(why) => (why, true),
        },
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn mtime_ms(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn cwd_matches(filter: Option<&str>, cwd: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    cwd.is_some_and(|cwd| cwd.contains(filter))
}

fn collect_coder(home: &Path, cwd_filter: Option<&str>, cutoff: i64, out: &mut Vec<Candidate>) {
    let sessions_root = home.join(".openagents").join("sessions");
    let Ok(encoded_dirs) = fs::read_dir(&sessions_root) else {
        return;
    };
    for encoded in encoded_dirs.flatten() {
        if !encoded.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let Ok(ids) = fs::read_dir(encoded.path()) else {
            continue;
        };
        for id_entry in ids.flatten() {
            if !id_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let dir = id_entry.path();
            let summary_path = dir.join("summary.json");
            let updates = dir.join("updates.jsonl");
            let Ok(meta) = fs::metadata(&updates) else {
                continue;
            };
            let mtime = mtime_ms(&meta);
            if mtime < cutoff {
                continue;
            }
            let summary = fs::read_to_string(&summary_path)
                .ok()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
            let cwd = summary
                .as_ref()
                .and_then(|value| value.get("cwd"))
                .and_then(|value| value.as_str())
                .map(str::to_string);
            let session_id = summary
                .as_ref()
                .and_then(|value| value.get("id"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| id_entry.file_name().to_string_lossy().into_owned());
            if !cwd_matches(cwd_filter, cwd.as_deref()) {
                continue;
            }
            out.push(Candidate {
                source: "coder",
                session_id,
                path: updates,
                cwd,
                mtime_ms: mtime,
                size_bytes: meta.len(),
            });
        }
    }
}

fn collect_claude(home: &Path, cwd_filter: Option<&str>, cutoff: i64, out: &mut Vec<Candidate>) {
    let projects = home.join(".claude").join("projects");
    let Ok(project_dirs) = fs::read_dir(&projects) else {
        return;
    };
    for project in project_dirs.flatten() {
        if !project.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let Ok(files) = fs::read_dir(project.path()) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name();
            let name = name.to_string_lossy();
            if !name.ends_with(".jsonl") {
                continue;
            }
            let path = file.path();
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            let mtime = mtime_ms(&meta);
            if mtime < cutoff {
                continue;
            }
            let bytes = peek_head(&path, 64 * 1024).unwrap_or_default();
            let (cwd, session_id) = claude_meta(&bytes);
            if !cwd_matches(cwd_filter, cwd.as_deref()) {
                continue;
            }
            out.push(Candidate {
                source: "claude",
                session_id: session_id
                    .unwrap_or_else(|| name.trim_end_matches(".jsonl").to_string()),
                path,
                cwd,
                mtime_ms: mtime,
                size_bytes: meta.len(),
            });
        }
    }
}

fn collect_codex(home: &Path, cwd_filter: Option<&str>, cutoff: i64, out: &mut Vec<Candidate>) {
    let sessions = home.join(".codex").join("sessions");
    walk_jsonl_tree(&sessions, cutoff, |path, meta, mtime| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("rollout-") || !name.ends_with(".jsonl") {
            return;
        }
        let bytes = peek_head(path, 64 * 1024).unwrap_or_default();
        let Some((cwd, session_id)) = codex_meta(&bytes) else {
            return;
        };
        if !cwd_matches(cwd_filter, cwd.as_deref()) {
            return;
        }
        out.push(Candidate {
            source: "codex",
            session_id: session_id.unwrap_or_else(|| name.to_string()),
            path: path.to_path_buf(),
            cwd,
            mtime_ms: mtime,
            size_bytes: meta.len(),
        });
    });
}

fn walk_jsonl_tree(root: &Path, cutoff: i64, mut visit: impl FnMut(&Path, &fs::Metadata, i64)) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = fs::metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                stack.push(path);
                continue;
            }
            if !meta.is_file() {
                continue;
            }
            let mtime = mtime_ms(&meta);
            if mtime < cutoff {
                continue;
            }
            visit(&path, &meta, mtime);
        }
    }
}

fn peek_head(path: &Path, max: u64) -> std::io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; max as usize];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

fn load_bytes(path: &Path, size: u64) -> std::io::Result<(Vec<u8>, bool)> {
    if size <= WHOLE_READ_BOUND {
        return Ok((fs::read(path)?, false));
    }
    let mut file = fs::File::open(path)?;
    let offset = size.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let aligned = match buf.iter().position(|b| *b == b'\n') {
        Some(at) => buf[at + 1..].to_vec(),
        None => Vec::new(),
    };
    Ok((aligned, true))
}

fn coder_turns(bytes: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(bytes);
    let mut turns = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let event_type = value
            .get("event_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let role = match event_type {
            "turn.user" => "user",
            "turn.assistant" => "assistant",
            "turn.checkpoint" => "checkpoint",
            _ => continue,
        };
        let payload = value.get("payload").unwrap_or(&serde_json::Value::Null);
        let Some(text) = payload.get("text").and_then(|v| v.as_str()) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        turns.push((role.to_string(), text.to_string()));
    }
    turns
}

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
                    if block.get("type").and_then(|v| v.as_str()) == Some("text")
                        && let Some(text) = block.get("text").and_then(|v| v.as_str())
                    {
                        parts.push(text.to_string());
                    }
                }
            }
            _ => {}
        }
        let joined = parts.join("\n");
        if !joined.is_empty() {
            turns.push((role, joined));
        }
    }
    turns
}

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
        let payload = value.get("payload").unwrap_or(&serde_json::Value::Null);
        if payload.get("type").and_then(|v| v.as_str()) != Some("message") {
            continue;
        }
        let role = payload
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if role != "user" && role != "assistant" {
            continue;
        }
        let mut parts = Vec::new();
        if let Some(serde_json::Value::Array(blocks)) = payload.get("content") {
            for block in blocks {
                let kind = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                if (kind == "input_text" || kind == "output_text")
                    && let Some(text) = block.get("text").and_then(|v| v.as_str())
                {
                    parts.push(text.to_string());
                }
            }
        }
        let joined = parts.join("\n");
        if !joined.is_empty() {
            turns.push((role, joined));
        }
    }
    turns
}

fn claude_meta(bytes: &[u8]) -> (Option<String>, Option<String>) {
    let text = String::from_utf8_lossy(bytes);
    let mut cwd = None;
    let mut session_id = None;
    for line in text.lines().take(20) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if cwd.is_none()
            && let Some(dir) = value.get("cwd").and_then(|v| v.as_str())
        {
            cwd = Some(dir.to_string());
        }
        if session_id.is_none()
            && let Some(id) = value.get("sessionId").and_then(|v| v.as_str())
        {
            session_id = Some(id.to_string());
        }
        if cwd.is_some() && session_id.is_some() {
            break;
        }
    }
    (cwd, session_id)
}

fn codex_meta(bytes: &[u8]) -> Option<(Option<String>, Option<String>)> {
    let text = String::from_utf8_lossy(bytes);
    let first = text.lines().next()?;
    let value = serde_json::from_str::<serde_json::Value>(first).ok()?;
    if value.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
        return None;
    }
    let payload = value.get("payload")?;
    let cwd = payload
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Some((cwd, id))
}

fn occurrences(text: &str, query_lower: &str) -> Vec<(usize, usize)> {
    let mut lowered = String::with_capacity(text.len());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_coder_session(home: &Path, cwd: &str, id: &str, user: &str) {
        let encoded: String = url::form_urlencoded::byte_serialize(cwd.as_bytes()).collect();
        let dir = home
            .join(".openagents")
            .join("sessions")
            .join(encoded)
            .join(id);
        fs::create_dir_all(&dir).unwrap();
        let summary = serde_json::json!({
            "format_version": 1,
            "id": id,
            "cwd": cwd,
            "created_at_ms": 1,
            "updated_at_ms": 2,
            "lane": "flash",
            "cloud_history": false
        });
        fs::write(
            dir.join("summary.json"),
            serde_json::to_vec_pretty(&summary).unwrap(),
        )
        .unwrap();
        let mut updates = fs::File::create(dir.join("updates.jsonl")).unwrap();
        writeln!(
            updates,
            r#"{{"format_version":1,"sequence":1,"at_ms":1700000000000,"event_type":"turn.user","payload":{{"text":"{user}"}}}}"#
        )
        .unwrap();
        writeln!(
            updates,
            r#"{{"format_version":1,"sequence":2,"at_ms":1700000001000,"event_type":"tool.ran","payload":{{"tool":"bash","output":"should not match flux capacitor in tools"}}}}"#
        )
        .unwrap();
        writeln!(
            updates,
            r#"{{"format_version":1,"sequence":3,"at_ms":1700000002000,"event_type":"turn.assistant","payload":{{"text":"noted"}}}}"#
        )
        .unwrap();
    }

    fn write_claude(home: &Path, body: &str) {
        let dir = home
            .join(".claude")
            .join("projects")
            .join("-Users-ada-work-proj");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("aaa.jsonl"), body).unwrap();
    }

    #[test]
    fn finds_a_coder_user_sentence_and_skips_tool_payloads() {
        let home = tempfile::tempdir().unwrap();
        let cwd = "/Users/ada/work/openagents";
        write_coder_session(home.path(), cwd, "sess-a", "the flux capacitor is rattling");
        let request = request_from_arguments(
            &serde_json::json!({"query": "flux capacitor", "sources": ["coder"]}),
            Path::new(cwd),
            now_ms(),
        )
        .unwrap();
        let out = search(home.path(), &request).unwrap();
        assert_eq!(out.sessions_matched, 1);
        assert_eq!(out.matches[0].source, "coder");
        assert_eq!(out.matches[0].session_id, "sess-a");
        assert_eq!(out.matches[0].hits[0].role, "user");
        assert!(out.matches[0].hits[0].context.contains("flux capacitor"));
        assert_eq!(
            out.matches[0].hits_total, 1,
            "tool.ran must not be searched"
        );
    }

    #[test]
    fn default_cwd_filter_hides_a_foreign_working_directory() {
        let home = tempfile::tempdir().unwrap();
        write_coder_session(
            home.path(),
            "/Users/ada/work/openagents",
            "here",
            "unique-phrase-xyz",
        );
        write_coder_session(
            home.path(),
            "/Users/ada/work/other",
            "there",
            "unique-phrase-xyz",
        );
        let request = request_from_arguments(
            &serde_json::json!({"query": "unique-phrase-xyz", "sources": ["coder"]}),
            Path::new("/Users/ada/work/openagents"),
            now_ms(),
        )
        .unwrap();
        let out = search(home.path(), &request).unwrap();
        assert_eq!(out.sessions_matched, 1);
        assert_eq!(out.matches[0].session_id, "here");
    }

    #[test]
    fn claude_store_still_matches_when_present() {
        let home = tempfile::tempdir().unwrap();
        write_claude(
            home.path(),
            r#"{"type":"user","cwd":"/Users/ada/work/proj","sessionId":"aaa","message":{"role":"user","content":"why does the flux capacitor overheat?"}}
"#,
        );
        let request = request_from_arguments(
            &serde_json::json!({
                "query": "flux capacitor",
                "sources": ["claude"],
                "cwd_filter": ""
            }),
            Path::new("/tmp"),
            now_ms(),
        )
        .unwrap();
        let out = search(home.path(), &request).unwrap();
        assert_eq!(out.sessions_matched, 1);
        assert_eq!(out.matches[0].source, "claude");
        assert_eq!(out.matches[0].session_id, "aaa");
    }

    #[test]
    fn empty_query_is_refused() {
        let err = request_from_arguments(&serde_json::json!({}), Path::new("/tmp"), 0).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }
}
