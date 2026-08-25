//! The reader against a fake host: a seeded Claude tree and Codex tree,
//! whole-file and tail reads, ceilings, and honest refusals — all without a
//! WASM runtime, the scanner's own testing pattern.

use super::*;
use openagents_pdk::{MountDirEntry, RefusalCode};
use std::collections::BTreeMap;

const CLAUDE_MOUNT: u32 = 0;
const CODEX_MOUNT: u32 = 1;
const DAY: i64 = 86_400_000;
const NOW: i64 = 1_756_000_000_000;

#[derive(Default)]
struct FakeHost {
    dirs: BTreeMap<(u32, String), MountDirListing>,
    files: BTreeMap<String, Vec<u8>>,
}

impl FakeHost {
    fn dir(&mut self, mount: u32, path: &str, entries: Vec<MountDirEntry>) {
        self.dirs
            .insert((mount, path.to_string()), MountDirListing { entries, truncated: false });
    }
    fn file(&mut self, path: &str, bytes: &str) {
        self.files.insert(path.to_string(), bytes.as_bytes().to_vec());
    }
}

impl Host for FakeHost {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        self.dirs.get(&(mount_index, path.to_string())).cloned().ok_or_else(|| {
            Refusal::new(RefusalCode::FileUnreadable, "the mount has no such directory")
        })
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        let bytes = self
            .files
            .get(path)
            .ok_or_else(|| Refusal::new(RefusalCode::MountDenied, "no mount holds the path"))?;
        if bytes.len() as u64 > WHOLE_READ_BOUND {
            return Err(Refusal::new(RefusalCode::FileTooLarge, "over the bound"));
        }
        Ok(bytes.clone())
    }
    fn read_range(&self, path: &str, offset: u64, max_bytes: u32) -> Result<Vec<u8>, Refusal> {
        let bytes = self
            .files
            .get(path)
            .ok_or_else(|| Refusal::new(RefusalCode::MountDenied, "no mount holds the path"))?;
        let start = (offset as usize).min(bytes.len());
        let end = (start + max_bytes as usize).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }
}

fn entry(name: &str, kind: &str, size: u64, mtime_ms: i64) -> MountDirEntry {
    MountDirEntry { name: name.to_string(), kind: kind.to_string(), size, mtime_ms }
}

fn claude_record(role: &str, content: &str) -> String {
    format!(
        r#"{{"type":"{role}","cwd":"/Users/ada/work/proj","sessionId":"aaa","message":{{"role":"{role}","content":"{content}"}}}}"#
    )
}

fn seeded(session_body: &str) -> FakeHost {
    let mut host = FakeHost::default();
    host.dir(CLAUDE_MOUNT, "projects", vec![entry("-Users-ada-work-proj", "dir", 0, NOW)]);
    host.dir(
        CLAUDE_MOUNT,
        "projects/-Users-ada-work-proj",
        vec![entry("aaa.jsonl", "file", session_body.len() as u64, NOW - DAY)],
    );
    host.file("projects/-Users-ada-work-proj/aaa.jsonl", session_body);
    host.dir(CODEX_MOUNT, "sessions", vec![entry("2026", "dir", 0, NOW)]);
    host.dir(CODEX_MOUNT, "sessions/2026", vec![entry("08", "dir", 0, NOW)]);
    host.dir(CODEX_MOUNT, "sessions/2026/08", vec![entry("20", "dir", 0, NOW)]);
    host.dir(
        CODEX_MOUNT,
        "sessions/2026/08/20",
        vec![entry("rollout-2026-08-20T10-00-00-bbb.jsonl", "file", 400, NOW - 2 * DAY)],
    );
    host.file(
        "sessions/2026/08/20/rollout-2026-08-20T10-00-00-bbb.jsonl",
        concat!(
            r#"{"type":"session_meta","payload":{"id":"bbb","cwd":"/Users/ada/work/other"}}"#,
            "\n",
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"fix the test"}]}}"#,
            "\n",
            r#"{"type":"response_item","payload":{"type":"reasoning","summary":[]}}"#,
            "\n",
            r#"{"type":"response_item","payload":{"type":"function_call","name":"shell"}}"#,
            "\n",
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"done, it passes"}]}}"#,
            "\n",
        ),
    );
    host
}

fn input() -> Input {
    Input {
        source: None,
        session_id: None,
        cwd_filter: None,
        max_turns: None,
        max_chars: None,
        now_ms: Some(NOW),
    }
}

fn claude_session() -> String {
    format!("{}\n{}\n", claude_record("user", "hello there"), claude_record("assistant", "hi!"))
}

#[test]
fn reads_the_newest_session_when_none_is_named() {
    let host = seeded(&claude_session());
    let out = read_conversation(&host, &input()).unwrap();
    assert_eq!(out.source, "claude");
    assert_eq!(out.session_id, "aaa");
    assert_eq!(
        out.turns.iter().map(|t| (t.role.as_str(), t.text.as_str())).collect::<Vec<_>>(),
        vec![("user", "hello there"), ("assistant", "hi!")],
    );
    assert!(!out.tail_only);
    assert_eq!(out.dropped_leading_turns, 0);
}

#[test]
fn reads_a_codex_session_by_id_and_counts_what_it_skips() {
    let host = seeded(&claude_session());
    let out = read_conversation(
        &host,
        &Input { source: Some("codex".into()), session_id: Some("bbb".into()), ..input() },
    )
    .unwrap();
    assert_eq!(out.source, "codex");
    assert_eq!(
        out.turns.iter().map(|t| (t.role.as_str(), t.text.as_str())).collect::<Vec<_>>(),
        vec![("user", "fix the test"), ("assistant", "done, it passes")],
    );
    assert_eq!(out.skipped.thinking, 1);
    assert_eq!(out.skipped.tool_activity, 1);
    // session_meta is a record, not a turn.
    assert!(out.skipped.other >= 1);
}

#[test]
fn an_unknown_session_id_is_a_refusal_naming_the_scanner() {
    let host = seeded(&claude_session());
    let refusal = read_conversation(
        &host,
        &Input { session_id: Some("zzz".into()), ..input() },
    )
    .unwrap_err();
    assert!(refusal.reason.contains("zzz"));
    assert!(refusal.reason.contains("foreign_sessions"));
}

#[test]
fn the_turn_ceiling_keeps_the_end_and_says_what_it_dropped() {
    let mut body = String::new();
    for at in 0..10 {
        body.push_str(&claude_record("user", &format!("question {at}")));
        body.push('\n');
    }
    let host = seeded(&body);
    let out = read_conversation(&host, &Input { max_turns: Some(3), ..input() }).unwrap();
    assert_eq!(out.turns_total, 10);
    assert_eq!(out.dropped_leading_turns, 7);
    assert_eq!(out.turns.last().unwrap().text, "question 9");
}

#[test]
fn a_long_turn_is_elided_in_the_middle_and_marked() {
    let long = "x".repeat(5_000);
    let host = seeded(&format!("{}\n", claude_record("user", &long)));
    let out = read_conversation(&host, &Input { max_chars: Some(400), ..input() }).unwrap();
    let turn = &out.turns[0];
    assert!(turn.truncated);
    assert!(turn.text.contains("characters elided"));
    assert!(turn.text.chars().count() < 500);
}

#[test]
fn an_oversized_file_is_read_from_its_tail_and_says_so() {
    // One line of padding pushes the file over the whole-read bound; the
    // conversation sits at the end, where a tail read finds it.
    let padding = format!(
        r#"{{"type":"padding","filler":"{}"}}"#,
        "p".repeat(WHOLE_READ_BOUND as usize)
    );
    let body = format!("{padding}\n{}", claude_session());
    let host = seeded(&body);
    let out = read_conversation(&host, &input()).unwrap();
    assert!(out.tail_only);
    assert_eq!(out.turns.len(), 2);
    assert_eq!(out.turns[1].text, "hi!");
    assert!(out.bytes_read <= TAIL_BYTES as usize);
}

#[test]
fn thinking_blocks_are_counted_not_replayed() {
    let body = format!(
        "{}\n",
        r#"{"type":"assistant","cwd":"/Users/ada/work/proj","sessionId":"aaa","message":{"role":"assistant","content":[{"type":"thinking","thinking":"secret"},{"type":"text","text":"the answer"}]}}"#
    );
    let host = seeded(&body);
    let out = read_conversation(&host, &input()).unwrap();
    assert_eq!(out.turns[0].text, "the answer");
    assert_eq!(out.skipped.thinking, 1);
    assert!(!out.turns[0].text.contains("secret"));
}
