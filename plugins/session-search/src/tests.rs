//! The searcher against a fake host: a seeded Claude tree and Codex tree,
//! whole-file and tail reads, hit caps, context windows, and honest
//! refusals — all without a WASM runtime, the reader's own testing pattern.

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

fn codex_session() -> &'static str {
    concat!(
        r#"{"type":"session_meta","payload":{"id":"bbb","cwd":"/Users/ada/work/other"}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"why does the flux capacitor overheat?"}]}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"reasoning","summary":[]}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"function_call","name":"shell"}}"#,
        "\n",
        r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"the Flux Capacitor needs coolant"}]}}"#,
        "\n",
    )
}

fn seeded(claude_body: &str) -> FakeHost {
    let mut host = FakeHost::default();
    host.dir(CLAUDE_MOUNT, "projects", vec![entry("-Users-ada-work-proj", "dir", 0, NOW)]);
    host.dir(
        CLAUDE_MOUNT,
        "projects/-Users-ada-work-proj",
        vec![entry("aaa.jsonl", "file", claude_body.len() as u64, NOW - DAY)],
    );
    host.file("projects/-Users-ada-work-proj/aaa.jsonl", claude_body);
    host.dir(CODEX_MOUNT, "sessions", vec![entry("2026", "dir", 0, NOW)]);
    host.dir(CODEX_MOUNT, "sessions/2026", vec![entry("08", "dir", 0, NOW)]);
    host.dir(CODEX_MOUNT, "sessions/2026/08", vec![entry("20", "dir", 0, NOW)]);
    host.dir(
        CODEX_MOUNT,
        "sessions/2026/08/20",
        vec![entry(
            "rollout-2026-08-20T10-00-00-bbb.jsonl",
            "file",
            codex_session().len() as u64,
            NOW - 2 * DAY,
        )],
    );
    host.file("sessions/2026/08/20/rollout-2026-08-20T10-00-00-bbb.jsonl", codex_session());
    host
}

fn input(query: &str) -> Input {
    Input {
        query: query.to_string(),
        sources: None,
        cwd_filter: None,
        max_age_days: None,
        max_sessions: None,
        max_hits_per_session: None,
        context_chars: None,
        now_ms: Some(NOW),
    }
}

fn claude_session() -> String {
    format!(
        "{}\n{}\n",
        claude_record("user", "the flux capacitor is rattling"),
        claude_record("assistant", "tighten its bolts")
    )
}

#[test]
fn finds_the_phrase_in_both_stores() {
    let host = seeded(&claude_session());
    let out = search_sessions(&host, &input("flux capacitor")).unwrap();
    assert_eq!(out.sessions_searched, 2);
    assert_eq!(out.sessions_matched, 2);
    assert_eq!(
        out.matches.iter().map(|m| (m.source.as_str(), m.session_id.as_str())).collect::<Vec<_>>(),
        vec![("claude", "aaa"), ("codex", "bbb")],
    );
    let claude = &out.matches[0];
    assert_eq!(claude.hits[0].role, "user");
    assert!(claude.hits[0].context.contains("flux capacitor"));
    assert_eq!(claude.cwd.as_deref(), Some("/Users/ada/work/proj"));
    // The Codex session matches twice: once per role, one differently cased.
    let codex = &out.matches[1];
    assert_eq!(codex.hits_total, 2);
    assert_eq!(codex.hits.iter().map(|h| h.role.as_str()).collect::<Vec<_>>(), vec![
        "user",
        "assistant"
    ]);
    assert!(!out.truncated);
    assert_eq!(out.skipped_unreadable, 0);
}

#[test]
fn the_search_is_case_insensitive_both_ways() {
    let host = seeded(&claude_session());
    let out = search_sessions(&host, &input("FLUX Capacitor")).unwrap();
    assert_eq!(out.sessions_matched, 2);
    // The reported context keeps the original casing.
    assert!(out.matches[1].hits[1].context.contains("Flux Capacitor"));
}

#[test]
fn context_is_bounded_and_elided_on_both_sides() {
    let body = format!(
        "{}\n",
        claude_record("user", &format!("{}needle{}", "a".repeat(600), "b".repeat(600)))
    );
    let host = seeded(&body);
    let out = search_sessions(
        &host,
        &Input { context_chars: Some(100), ..input("NEEDLE") },
    )
    .unwrap();
    let context = &out.matches[0].hits[0].context;
    assert!(context.starts_with('…'));
    assert!(context.ends_with('…'));
    assert!(context.contains("needle"));
    // The hit plus half the budget on each side, plus two markers.
    assert!(context.chars().count() <= 100 + "needle".len() + 2);
}

#[test]
fn hits_per_session_are_capped_but_counted_in_full() {
    let mut body = String::new();
    for at in 0..7 {
        body.push_str(&claude_record("user", &format!("echo number {at}")));
        body.push('\n');
    }
    let host = seeded(&body);
    let out = search_sessions(
        &host,
        &Input { max_hits_per_session: Some(2), sources: Some(vec!["claude".into()]), ..input("echo") },
    )
    .unwrap();
    let hit = &out.matches[0];
    assert_eq!(hit.hits.len(), 2);
    assert_eq!(hit.hits_total, 7);
    assert_eq!(hit.hits[0].context, "echo number 0");
}

#[test]
fn an_oversized_session_is_searched_from_its_tail_and_says_so() {
    // One line of padding pushes the file over the whole-read bound; the
    // conversation sits at the end, where a tail read finds it.
    let padding = format!(
        r#"{{"type":"padding","filler":"{}"}}"#,
        "p".repeat(WHOLE_READ_BOUND as usize)
    );
    let body = format!("{padding}\n{}", claude_session());
    let host = seeded(&body);
    let out = search_sessions(
        &host,
        &Input { sources: Some(vec!["claude".into()]), ..input("rattling") },
    )
    .unwrap();
    assert_eq!(out.sessions_matched, 1);
    assert!(out.matches[0].tail_only);
    assert!(out.matches[0].hits[0].context.contains("rattling"));
}

#[test]
fn no_match_is_an_empty_result_not_a_refusal() {
    let host = seeded(&claude_session());
    let out = search_sessions(&host, &input("perpetual motion")).unwrap();
    assert_eq!(out.sessions_searched, 2);
    assert_eq!(out.sessions_matched, 0);
    assert!(out.matches.is_empty());
}

#[test]
fn a_blank_query_refuses_and_names_the_fix() {
    let host = seeded(&claude_session());
    let refusal = search_sessions(&host, &input("   ")).unwrap_err();
    assert!(refusal.reason.contains("query"));
}

#[test]
fn thinking_and_tool_payloads_are_not_searched() {
    let body = format!(
        "{}\n",
        r#"{"type":"assistant","cwd":"/Users/ada/work/proj","sessionId":"aaa","message":{"role":"assistant","content":[{"type":"thinking","thinking":"secret plan"},{"type":"tool_use","input":{"cmd":"secret cmd"}},{"type":"text","text":"visible answer"}]}}"#
    );
    let host = seeded(&body);
    let secret = search_sessions(
        &host,
        &Input { sources: Some(vec!["claude".into()]), ..input("secret") },
    )
    .unwrap();
    assert!(secret.matches.is_empty());
    let visible = search_sessions(
        &host,
        &Input { sources: Some(vec!["claude".into()]), ..input("visible") },
    )
    .unwrap();
    assert_eq!(visible.sessions_matched, 1);
}

#[test]
fn the_session_budget_reports_truncation() {
    let host = seeded(&claude_session());
    let out = search_sessions(&host, &Input { max_sessions: Some(1), ..input("flux") }).unwrap();
    assert_eq!(out.sessions_searched, 1);
    assert!(out.truncated);
}
