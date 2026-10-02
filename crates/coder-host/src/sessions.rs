//! Claude Code and Codex sessions on this computer, imported as threads
//! (#10151).
//!
//! Claude Code keeps each session as JSON lines under
//! `~/.claude/projects/<project>/<session>.jsonl`, and Codex under
//! `~/.codex/sessions/<year>/<month>/<day>/rollout-…jsonl`. The host reads
//! those files and nothing else there: it opens them read-only, never
//! writes, moves, or locks anything under `~/.claude` or `~/.codex`, and
//! follows no link out of them.
//!
//! Each session becomes one thread in the host's own store, a copy: the
//! person's messages and the agent's text replies, in order, with their
//! times. Tool calls, their output, thinking, and the context the agent
//! injected are left out. The thread's ID is derived from the session's
//! own, so importing again takes in only sessions not imported before and
//! leaves the threads already there as they are. Every app that reads the
//! host's threads (the desktop app, a paired phone, `openagents chat`, and
//! OpenAgents Terminal) shows them.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use openagents_chat::basic_chats::{BasicChats, Summary};
use openagents_chat::basic_coder::{Role, Turn};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The most text one imported turn keeps; the rest is cut.
pub const MAX_TURN_CHARS: usize = 16 * 1024;
/// The most turns one imported thread keeps, the newest.
pub const MAX_TURNS: usize = 200;
/// The most characters of a title.
const MAX_TITLE_CHARS: usize = 80;
/// A session file larger than this is skipped.
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// Which program kept a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    ClaudeCode,
    Codex,
}

impl Source {
    /// The program's name, as a thread's title starts.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Source::ClaudeCode => "Claude Code",
            Source::Codex => "Codex",
        }
    }

    const fn word(self) -> &'static str {
        match self {
            Source::ClaudeCode => "claude-code",
            Source::Codex => "codex",
        }
    }
}

/// One session read from its file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub source: Source,
    /// The session's own ID.
    pub id: String,
    pub title: String,
    pub turns: Vec<Turn>,
    pub started: u64,
    pub updated: u64,
}

impl Session {
    /// The thread this session becomes: the same ID every time.
    #[must_use]
    pub fn thread(&self) -> String {
        thread_id(self.source, &self.id)
    }
}

/// The thread ID of `source`'s session `id`: 32 lowercase hex characters,
/// as the chat service admits, the same on every import.
#[must_use]
pub fn thread_id(source: Source, id: &str) -> String {
    let digest = Sha256::digest(format!("openagents.import.{}:{id}", source.word()).as_bytes());
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// What an import did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Sessions taken in as new threads now.
    pub imported: usize,
    /// Sessions the store already held, left as they were.
    pub present: usize,
    /// Session files that could not be read or held no conversation.
    pub skipped: usize,
}

/// This user's home folder: `HOME`, or `USERPROFILE` on Windows.
#[must_use]
pub fn user_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// Every session file under `home`'s `.claude` and `.codex`, with the
/// program that kept it, oldest path first.
#[must_use]
pub fn find(home: &Path) -> Vec<(Source, PathBuf)> {
    let mut found = Vec::new();
    // Only `~/.claude` and `~/.codex` are read. One that is a link into a
    // folder macOS guards with a privacy prompt is not followed there:
    // reading it would make macOS ask the person about Coder.
    let readable = |dir: &Path| {
        dir.canonicalize()
            .is_ok_and(|real| !coder_boundary::privacy::is_protected(&real, home))
    };
    if !readable(&home.join(".claude")) {
        return codex_sessions(home, &readable);
    }
    for project in dirs(&home.join(".claude").join("projects")) {
        for file in files(&project) {
            found.push((Source::ClaudeCode, file));
        }
    }
    found.extend(codex_sessions(home, &readable));
    found
}

fn codex_sessions(home: &Path, readable: &dyn Fn(&Path) -> bool) -> Vec<(Source, PathBuf)> {
    let mut codex = Vec::new();
    if readable(&home.join(".codex")) {
        walk(&home.join(".codex").join("sessions"), 3, &mut codex);
    }
    codex
        .into_iter()
        .map(|file| (Source::Codex, file))
        .collect()
}

/// Read `path` as a session `source` kept. `None` when it cannot be read
/// or holds no message.
#[must_use]
pub fn read(source: Source, path: &Path) -> Option<Session> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    let lines = BufReader::new(File::open(path).ok()?)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok());
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let mut session = match source {
        Source::ClaudeCode => claude(lines, stem),
        Source::Codex => codex(lines.collect(), stem),
    }?;
    if session.turns.len() > MAX_TURNS {
        session.turns.drain(..session.turns.len() - MAX_TURNS);
    }
    Some(session)
}

/// Take every session under `home` into `chats` that it does not hold yet.
///
/// # Errors
/// The store refused a write; what was imported before stays.
pub fn import(home: &Path, chats: &mut BasicChats) -> Result<Report, String> {
    let mut report = Report::default();
    for (source, path) in find(home) {
        let Some(session) = read(source, &path) else {
            report.skipped += 1;
            continue;
        };
        let summary = Summary {
            id: session.thread(),
            title: session.title.clone(),
            started: session.started,
            updated: session.updated,
            coder: None,
            archived: false,
            pinned: false,
            named: false,
        };
        if chats.adopt(summary, session.turns, None)? {
            report.imported += 1;
        } else {
            report.present += 1;
        }
    }
    Ok(report)
}

/// The turns of a conversation as it is read: consecutive messages of one
/// role join into one turn.
#[derive(Default)]
struct Builder {
    turns: Vec<Turn>,
    started: Option<u64>,
    updated: u64,
}

impl Builder {
    fn push(&mut self, role: Role, text: &str, at: Option<u64>) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if let Some(at) = at {
            self.started.get_or_insert(at);
            self.updated = self.updated.max(at);
        }
        match self.turns.last_mut() {
            Some(last) if last.role == role => {
                last.text = bounded(&format!("{}\n\n{text}", last.text));
                if at.is_some() {
                    last.at = at;
                }
            }
            _ => {
                let mut turn = match role {
                    Role::User => Turn::user(bounded(text)),
                    Role::Assistant => Turn::assistant(bounded(text), None),
                };
                turn.at = at;
                self.turns.push(turn);
            }
        }
    }

    fn finish(self, source: Source, id: String, title: Option<String>) -> Option<Session> {
        if !self.turns.iter().any(|turn| turn.role == Role::User) {
            return None;
        }
        let title = title
            .filter(|title| !title.trim().is_empty())
            .or_else(|| {
                self.turns
                    .iter()
                    .find(|turn| turn.role == Role::User)
                    .and_then(|turn| turn.text.lines().next().map(str::to_owned))
            })
            .unwrap_or_default();
        let title: String = title.trim().chars().take(MAX_TITLE_CHARS).collect();
        Some(Session {
            source,
            id,
            title: format!("{} · {title}", source.name()),
            started: self.started.unwrap_or(self.updated),
            updated: self.updated,
            turns: self.turns,
        })
    }
}

/// A Claude Code session: `user` lines whose content is the person's text
/// (tool results and meta lines are not), `assistant` lines' `text` parts,
/// and the session's own title when it has one.
fn claude(lines: impl Iterator<Item = Value>, stem: String) -> Option<Session> {
    let mut built = Builder::default();
    let mut id = None;
    let mut title = None;
    for line in lines {
        if line["isSidechain"].as_bool() == Some(true) || line["isMeta"].as_bool() == Some(true) {
            continue;
        }
        if id.is_none() {
            id = line["sessionId"].as_str().map(str::to_owned);
        }
        let at = line["timestamp"].as_str().and_then(unix);
        match line["type"].as_str() {
            Some("user") => {
                let content = &line["message"]["content"];
                let text = match content {
                    Value::String(text) => text.clone(),
                    _ => texts(content, &["text"]),
                };
                // A slash command's echo and its local output are not the
                // person's words.
                if !text.trim_start().starts_with('<') {
                    built.push(Role::User, &text, at);
                }
            }
            Some("assistant") => {
                built.push(
                    Role::Assistant,
                    &texts(&line["message"]["content"], &["text"]),
                    at,
                );
            }
            Some("ai-title") => title = line["aiTitle"].as_str().map(str::to_owned),
            Some("summary") if title.is_none() => {
                title = line["summary"].as_str().map(str::to_owned);
            }
            _ => {}
        }
    }
    built.finish(Source::ClaudeCode, id.unwrap_or(stem), title)
}

/// A Codex session: the `user_message` and `agent_message` events when the
/// file has them, else the `message` items, leaving out the context Codex
/// injects into the first user message (instructions and environment).
fn codex(lines: Vec<Value>, stem: String) -> Option<Session> {
    let id = lines
        .iter()
        .find(|line| line["type"] == "session_meta")
        .and_then(|line| line["payload"]["id"].as_str())
        .map_or(stem, str::to_owned);
    let events = lines
        .iter()
        .any(|line| line["type"] == "event_msg" && line["payload"]["type"] == "user_message");
    let mut built = Builder::default();
    for line in &lines {
        let at = line["timestamp"].as_str().and_then(unix);
        let payload = &line["payload"];
        if events {
            if line["type"] != "event_msg" {
                continue;
            }
            match payload["type"].as_str() {
                Some("user_message") => {
                    built.push(Role::User, payload["message"].as_str().unwrap_or(""), at);
                }
                Some("agent_message") => {
                    built.push(
                        Role::Assistant,
                        payload["message"].as_str().unwrap_or(""),
                        at,
                    );
                }
                _ => {}
            }
            continue;
        }
        if line["type"] != "response_item" || payload["type"] != "message" {
            continue;
        }
        match payload["role"].as_str() {
            Some("user") => {
                let text = payload["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|part| part["type"] == "input_text")
                    .filter_map(|part| part["text"].as_str())
                    .filter(|text| !injected(text))
                    .collect::<Vec<_>>()
                    .join("\n\n");
                built.push(Role::User, &text, at);
            }
            Some("assistant") => {
                built.push(
                    Role::Assistant,
                    &texts(&payload["content"], &["output_text"]),
                    at,
                );
            }
            _ => {}
        }
    }
    built.finish(Source::Codex, id, None)
}

/// Context Codex puts in a user message that the person did not type: its
/// instructions files and environment blocks, each a tagged block or the
/// instructions heading.
fn injected(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with('<') || text.starts_with("# AGENTS.md instructions")
}

/// The `text` of each content part whose `type` is one of `kinds`, joined.
fn texts(content: &Value, kinds: &[&str]) -> String {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|part| {
            part["type"]
                .as_str()
                .is_some_and(|kind| kinds.contains(&kind))
        })
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// `text` cut to [`MAX_TURN_CHARS`], saying so.
fn bounded(text: &str) -> String {
    match text.char_indices().nth(MAX_TURN_CHARS) {
        Some((at, _)) => format!("{}\n\n[cut on import]", &text[..at]),
        None => text.to_owned(),
    }
}

/// The directories in `dir`, not following links, in name order.
fn dirs(dir: &Path) -> Vec<PathBuf> {
    entries(dir)
        .into_iter()
        .filter(|path| std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir()))
        .collect()
}

/// The `.jsonl` files in `dir`, not following links, in name order.
fn files(dir: &Path) -> Vec<PathBuf> {
    entries(dir)
        .into_iter()
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "jsonl")
                && std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file())
        })
        .collect()
}

/// The `.jsonl` files `depth` directories below `dir`.
fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        out.extend(files(dir));
        return;
    }
    for child in dirs(dir) {
        walk(&child, depth - 1, out);
    }
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|read| {
            read.filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

/// Unix seconds of an RFC 3339 UTC time such as
/// `2026-09-30T18:43:31.123Z`.
fn unix(text: &str) -> Option<u64> {
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if text.get(4..5) != Some("-") || text.get(10..11) != Some("T") || !(1..=12).contains(&month) {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hour * 3_600 + minute * 60 + second).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write(path: &Path, lines: &[Value]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
        std::fs::write(path, text).unwrap();
    }

    /// A temporary home with one Claude Code and one Codex session.
    fn home() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        write(
            &home.path().join(".claude/projects/-work-demo/c1.jsonl"),
            &[
                json!({"type": "permission-mode", "sessionId": "c1"}),
                json!({"type": "user", "sessionId": "c1", "timestamp": "2026-09-30T10:00:00.000Z",
                       "message": {"role": "user", "content": "fix the flaky test"}}),
                json!({"type": "user", "isMeta": true, "sessionId": "c1",
                       "message": {"role": "user", "content": "Caveat: injected"}}),
                json!({"type": "assistant", "sessionId": "c1", "timestamp": "2026-09-30T10:00:05.000Z",
                       "message": {"role": "assistant", "content": [{"type": "thinking", "thinking": "hmm"}]}}),
                json!({"type": "assistant", "sessionId": "c1", "timestamp": "2026-09-30T10:00:06.000Z",
                       "message": {"role": "assistant", "content": [{"type": "text", "text": "Looking."},
                                                                     {"type": "tool_use", "name": "Bash"}]}}),
                json!({"type": "user", "sessionId": "c1", "timestamp": "2026-09-30T10:00:07.000Z",
                       "message": {"role": "user", "content": [{"type": "tool_result", "content": "ok"}]}}),
                json!({"type": "assistant", "sessionId": "c1", "timestamp": "2026-09-30T10:00:09.000Z",
                       "message": {"role": "assistant", "content": [{"type": "text", "text": "Fixed it."}]}}),
                json!({"type": "assistant", "isSidechain": true, "sessionId": "c1",
                       "message": {"role": "assistant", "content": [{"type": "text", "text": "subagent"}]}}),
                json!({"type": "ai-title", "aiTitle": "Fix flaky test", "sessionId": "c1"}),
                json!("not an object"),
            ],
        );
        std::fs::write(
            home.path().join(".claude/projects/-work-demo/broken.jsonl"),
            "{nope",
        )
        .unwrap();
        write(
            &home
                .path()
                .join(".codex/sessions/2026/09/30/rollout-2026-09-30T18-43-31-x1.jsonl"),
            &[
                json!({"type": "session_meta", "timestamp": "2026-09-30T18:43:31.000Z",
                       "payload": {"id": "x1", "cwd": "/work/demo"}}),
                json!({"type": "response_item", "timestamp": "2026-09-30T18:43:32.000Z",
                       "payload": {"type": "message", "role": "developer",
                                   "content": [{"type": "input_text", "text": "rules"}]}}),
                json!({"type": "response_item", "timestamp": "2026-09-30T18:43:32.000Z",
                       "payload": {"type": "message", "role": "user",
                                   "content": [{"type": "input_text", "text": "# AGENTS.md instructions for /work"},
                                               {"type": "input_text", "text": "<environment_context>x</environment_context>"},
                                               {"type": "input_text", "text": "add a readme"}]}}),
                json!({"type": "response_item", "payload": {"type": "custom_tool_call", "name": "apply_patch"}}),
                json!({"type": "response_item", "timestamp": "2026-09-30T18:44:00.000Z",
                       "payload": {"type": "message", "role": "assistant",
                                   "content": [{"type": "output_text", "text": "Added README.md."}]}}),
            ],
        );
        home
    }

    #[test]
    fn a_claude_code_session_keeps_the_conversation_only() {
        let home = home();
        let session = read(
            Source::ClaudeCode,
            &home.path().join(".claude/projects/-work-demo/c1.jsonl"),
        )
        .unwrap();
        assert_eq!(session.id, "c1");
        assert_eq!(session.title, "Claude Code · Fix flaky test");
        let turns: Vec<(Role, &str)> = session
            .turns
            .iter()
            .map(|turn| (turn.role, turn.text.as_str()))
            .collect();
        assert_eq!(
            turns,
            [
                (Role::User, "fix the flaky test"),
                (Role::Assistant, "Looking.\n\nFixed it."),
            ]
        );
        assert_eq!(session.started, unix("2026-09-30T10:00:00Z").unwrap());
        assert_eq!(session.updated, session.started + 9);
    }

    #[test]
    fn a_codex_session_leaves_out_what_codex_injected() {
        let home = home();
        let path = home
            .path()
            .join(".codex/sessions/2026/09/30/rollout-2026-09-30T18-43-31-x1.jsonl");
        let session = read(Source::Codex, &path).unwrap();
        assert_eq!(session.id, "x1");
        assert_eq!(session.title, "Codex · add a readme");
        let turns: Vec<&str> = session
            .turns
            .iter()
            .map(|turn| turn.text.as_str())
            .collect();
        assert_eq!(turns, ["add a readme", "Added README.md."]);
        // The older form, with message events, reads those instead.
        let older = home.path().join("older.jsonl");
        write(
            &older,
            &[
                json!({"type": "event_msg", "payload": {"type": "user_message", "message": "hi"}}),
                json!({"type": "response_item", "payload": {"type": "message", "role": "user",
                       "content": [{"type": "input_text", "text": "hi"}]}}),
                json!({"type": "event_msg", "payload": {"type": "agent_message", "message": "hello"}}),
            ],
        );
        let turns: Vec<String> = read(Source::Codex, &older)
            .unwrap()
            .turns
            .into_iter()
            .map(|turn| turn.text)
            .collect();
        assert_eq!(turns, ["hi", "hello"]);
    }

    /// Importing copies each session once into the store and never
    /// changes the session files.
    /// A `.codex` that is a link into a privacy-protected folder is not
    /// followed there, so importing never makes macOS ask about Coder.
    #[cfg(target_os = "macos")]
    #[test]
    fn sessions_behind_a_link_into_a_protected_folder_are_not_read() {
        let dir = home();
        let home = dir.path().canonicalize().unwrap();
        let before = find(&home).len();
        let kept = home.join("Documents/codex");
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        std::fs::rename(home.join(".codex"), &kept).unwrap();
        std::os::unix::fs::symlink(&kept, home.join(".codex")).unwrap();
        let found = find(&home);
        assert_eq!(found.len(), before - 1, "{found:?}");
        assert!(
            found
                .iter()
                .all(|(source, _)| *source == Source::ClaudeCode)
        );
    }

    #[test]
    fn importing_is_a_copy_and_happens_once() {
        let home = home();
        let before: Vec<(PathBuf, Vec<u8>)> = find(home.path())
            .into_iter()
            .map(|(_, path)| {
                let bytes = std::fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect();
        assert_eq!(before.len(), 3);
        let mut chats = BasicChats::empty();
        let report = import(home.path(), &mut chats).unwrap();
        assert_eq!(
            report,
            Report {
                imported: 2,
                present: 0,
                skipped: 1
            }
        );
        let claude = thread_id(Source::ClaudeCode, "c1");
        assert_eq!(claude.len(), 32);
        assert_eq!(
            chats.get(&claude).unwrap().title,
            "Claude Code · Fix flaky test"
        );
        assert_eq!(chats.turns(&claude).len(), 2);
        assert!(chats.get(&thread_id(Source::Codex, "x1")).is_some());
        let again = import(home.path(), &mut chats).unwrap();
        assert_eq!((again.imported, again.present), (0, 2));
        for (path, bytes) in before {
            assert_eq!(std::fs::read(&path).unwrap(), bytes, "{}", path.display());
        }
    }

    #[test]
    fn long_turns_are_cut_and_times_parse() {
        let long = "x".repeat(MAX_TURN_CHARS + 10);
        assert!(bounded(&long).ends_with("[cut on import]"));
        assert_eq!(unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix("2026-10-02T00:00:00.5Z"), Some(1_790_899_200));
        assert_eq!(unix("garbage"), None);
    }
}
