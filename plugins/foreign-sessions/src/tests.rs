//! The scanner against a fake host: fixture trees with good, malformed,
//! oversized, and symlinked entries, exercised without a WASM runtime. The
//! same shapes run through the real boundary in
//! `packages/openagents-cli/test/coder-plugin-foreign-sessions.test.ts`.

use super::*;
use openagents_pdk::MountDirEntry;
use std::collections::BTreeMap;

/// A fake host over two in-memory mount trees. Directories are keyed by
/// `(mount, path)`; file bytes are keyed by mount-relative path the way
/// the real host's mount-order read loop would find them.
#[derive(Default)]
struct FakeHost {
    dirs: BTreeMap<(u32, String), MountDirListing>,
    files: BTreeMap<String, Vec<u8>>,
    /// Paths the read import answers with `file_too_large`.
    oversized: Vec<String>,
    /// Paths the read import answers with `file_unreadable`.
    unreadable: Vec<String>,
}

impl FakeHost {
    fn dir(&mut self, mount: u32, path: &str, entries: Vec<MountDirEntry>) {
        self.dirs.insert(
            (mount, path.to_string()),
            MountDirListing { entries, truncated: false },
        );
    }
    fn file(&mut self, path: &str, bytes: &str) {
        self.files.insert(path.to_string(), bytes.as_bytes().to_vec());
    }
    fn bytes(&mut self, path: &str, bytes: Vec<u8>) {
        self.files.insert(path.to_string(), bytes);
    }
}

impl Host for FakeHost {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        self.dirs
            .get(&(mount_index, path.to_string()))
            .cloned()
            .ok_or_else(|| {
                Refusal::new(RefusalCode::FileUnreadable, "the mount has no such directory")
            })
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        if self.oversized.iter().any(|p| p == path) {
            return Err(Refusal::new(RefusalCode::FileTooLarge, "over the bound"));
        }
        if self.unreadable.iter().any(|p| p == path) {
            return Err(Refusal::new(RefusalCode::FileUnreadable, "io failure"));
        }
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| Refusal::new(RefusalCode::MountDenied, "no declared mount contains the path"))
    }
    fn read_range(&self, path: &str, offset: u64, max_bytes: u32) -> Result<Vec<u8>, Refusal> {
        if self.unreadable.iter().any(|p| p == path) {
            return Err(Refusal::new(RefusalCode::FileUnreadable, "io failure"));
        }
        let bytes = self.files.get(path).ok_or_else(|| {
            Refusal::new(RefusalCode::MountDenied, "no declared mount contains the path")
        })?;
        let start = (offset as usize).min(bytes.len());
        let end = start.saturating_add(max_bytes as usize).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }
}

fn entry(name: &str, kind: &str, size: u64, mtime_ms: i64) -> MountDirEntry {
    MountDirEntry { name: name.to_string(), kind: kind.to_string(), size, mtime_ms }
}

fn claude_line(cwd: &str, session_id: &str) -> String {
    format!(
        r#"{{"type":"user","cwd":"{cwd}","sessionId":"{session_id}","message":{{"role":"user","content":"hi"}}}}"#
    )
}

fn codex_line(cwd: &str, id: &str) -> String {
    format!(
        r#"{{"timestamp":"2026-08-20T10:00:00.000Z","type":"session_meta","payload":{{"id":"{id}","cwd":"{cwd}"}}}}"#
    )
}

const DAY: i64 = 86_400_000;
const NOW: i64 = 1_756_000_000_000;

/// A host with one recent Claude session, one recent Codex session, and one
/// stale Claude session (90 days old).
fn seeded() -> FakeHost {
    let mut host = FakeHost::default();
    host.dir(
        CLAUDE_MOUNT,
        "projects",
        vec![entry("-Users-ada-work-proj", "dir", 0, NOW)],
    );
    host.dir(
        CLAUDE_MOUNT,
        "projects/-Users-ada-work-proj",
        vec![
            entry("aaa.jsonl", "file", 100, NOW - DAY),
            entry("old.jsonl", "file", 90, NOW - 90 * DAY),
        ],
    );
    host.file(
        "projects/-Users-ada-work-proj/aaa.jsonl",
        &format!("{}\n{}\n", claude_line("/Users/ada/work/proj", "aaa"), claude_line("/Users/ada/work/proj", "aaa")),
    );
    host.file(
        "projects/-Users-ada-work-proj/old.jsonl",
        &claude_line("/Users/ada/work/proj", "old"),
    );
    host.dir(CODEX_MOUNT, "sessions", vec![entry("2026", "dir", 0, NOW)]);
    host.dir(CODEX_MOUNT, "sessions/2026", vec![entry("08", "dir", 0, NOW)]);
    host.dir(CODEX_MOUNT, "sessions/2026/08", vec![entry("20", "dir", 0, NOW)]);
    host.dir(
        CODEX_MOUNT,
        "sessions/2026/08/20",
        vec![entry("rollout-2026-08-20T10-00-00-bbb.jsonl", "file", 200, NOW - 2 * DAY)],
    );
    host.file(
        "sessions/2026/08/20/rollout-2026-08-20T10-00-00-bbb.jsonl",
        &format!("{}\n", codex_line("/Users/ada/work/other", "bbb")),
    );
    host
}

fn input() -> Input {
    Input { sources: None, cwd_filter: None, max_age_days: None, limit: None, now_ms: Some(NOW) }
}

#[test]
fn recent_sessions_from_both_sources_come_back_newest_first() {
    let out = scan(&seeded(), &input()).unwrap();
    assert_eq!(
        out.sessions.iter().map(|s| s.session_id.as_str()).collect::<Vec<_>>(),
        vec!["aaa", "bbb"],
    );
    let claude = &out.sessions[0];
    assert_eq!(claude.source, "claude");
    assert_eq!(claude.cwd.as_deref(), Some("/Users/ada/work/proj"));
    assert_eq!(claude.record_count, Some(2));
    assert_eq!(claude.project_dir.as_deref(), Some("-Users-ada-work-proj"));
    assert!(!claude.metadata_truncated);
    let codex = &out.sessions[1];
    assert_eq!(codex.source, "codex");
    assert_eq!(codex.cwd.as_deref(), Some("/Users/ada/work/other"));
    assert_eq!(out.skipped, Skipped::default());
}

#[test]
fn the_age_cutoff_drops_stale_sessions() {
    let out = scan(&seeded(), &input()).unwrap();
    assert!(out.sessions.iter().all(|s| s.session_id != "old"));
    let out = scan(&seeded(), &Input { max_age_days: Some(365.0), ..input() }).unwrap();
    assert!(out.sessions.iter().any(|s| s.session_id == "old"));
}

#[test]
fn without_a_clock_the_newest_mtime_stands_in_for_now() {
    let out = scan(&seeded(), &Input { now_ms: None, ..input() }).unwrap();
    // Newest is aaa at NOW - DAY; old at NOW - 90*DAY is outside 30 days of it.
    assert_eq!(out.sessions.len(), 2);
}

#[test]
fn a_cwd_filter_narrows_by_working_directory() {
    let out = scan(&seeded(), &Input { cwd_filter: Some("work/proj".into()), ..input() }).unwrap();
    assert_eq!(out.sessions.len(), 1);
    assert_eq!(out.sessions[0].session_id, "aaa");
}

#[test]
fn an_unknown_source_is_refused_not_guessed() {
    let refusal =
        scan(&seeded(), &Input { sources: Some(vec!["cursor".into()]), ..input() }).unwrap_err();
    assert_eq!(refusal.code, RefusalCode::Unsupported);
}

#[test]
fn a_missing_store_is_reported_not_fatal() {
    let mut host = seeded();
    host.dirs.remove(&(CODEX_MOUNT, "sessions".to_string()));
    let out = scan(&host, &input()).unwrap();
    assert_eq!(out.missing_sources, vec!["codex"]);
    assert_eq!(out.sessions.len(), 1);
}

#[test]
fn malformed_files_are_skipped_and_counted() {
    let mut host = seeded();
    host.dir(
        CLAUDE_MOUNT,
        "projects/-Users-ada-work-proj",
        vec![
            entry("aaa.jsonl", "file", 100, NOW - DAY),
            entry("bad.jsonl", "file", 50, NOW - DAY),
        ],
    );
    host.file("projects/-Users-ada-work-proj/bad.jsonl", "not json at all\n{}\n");
    let out = scan(&host, &input()).unwrap();
    assert_eq!(out.skipped.malformed, 1);
    assert!(out.sessions.iter().any(|s| s.session_id == "aaa"));
}

#[test]
fn an_oversized_file_keeps_listing_metadata_and_is_marked_truncated() {
    let mut host = seeded();
    host.dir(
        CLAUDE_MOUNT,
        "projects/-Users-ada-work-proj",
        vec![entry("huge.jsonl", "file", 5_000_000, NOW)],
    );
    host.oversized.push("projects/-Users-ada-work-proj/huge.jsonl".to_string());
    let out = scan(&host, &input()).unwrap();
    let huge = out.sessions.iter().find(|s| s.session_id == "huge").unwrap();
    assert!(huge.metadata_truncated);
    assert_eq!(huge.cwd, None);
    assert_eq!(huge.record_count, None);
    assert_eq!(huge.size_bytes, 5_000_000);
    assert_eq!(out.oversized, 1);
}

#[test]
fn symlinked_entries_are_skipped_and_counted() {
    let mut host = seeded();
    host.dir(
        CLAUDE_MOUNT,
        "projects/-Users-ada-work-proj",
        vec![
            entry("aaa.jsonl", "file", 100, NOW - DAY),
            entry("sneaky.jsonl", "symlink", 0, NOW),
        ],
    );
    let out = scan(&host, &input()).unwrap();
    assert_eq!(out.skipped.symlinked, 1);
    assert!(out.sessions.iter().all(|s| s.session_id != "sneaky"));
}

#[test]
fn unreadable_files_are_skipped_and_counted() {
    let mut host = seeded();
    host.unreadable.push("projects/-Users-ada-work-proj/aaa.jsonl".to_string());
    let out = scan(&host, &input()).unwrap();
    assert_eq!(out.skipped.unreadable, 1);
    assert!(out.sessions.iter().all(|s| s.session_id != "aaa"));
}

#[test]
fn the_limit_caps_results_and_never_exceeds_fifty() {
    let mut host = seeded();
    let entries: Vec<MountDirEntry> = (0..80)
        .map(|i| entry(&format!("s{i:02}.jsonl"), "file", 10, NOW - i * 1000))
        .collect();
    host.dir(CLAUDE_MOUNT, "projects/-Users-ada-work-proj", entries);
    for i in 0..80 {
        host.file(
            &format!("projects/-Users-ada-work-proj/s{i:02}.jsonl"),
            &claude_line("/Users/ada/work/proj", &format!("s{i:02}")),
        );
    }
    let out = scan(&host, &Input { limit: Some(3), ..input() }).unwrap();
    assert_eq!(out.sessions.len(), 3);
    assert_eq!(out.sessions[0].session_id, "s00");
    let out = scan(&host, &Input { limit: Some(10_000), ..input() }).unwrap();
    assert_eq!(out.sessions.len(), 50);
}

#[test]
fn a_truncated_listing_marks_the_scan_truncated() {
    let mut host = seeded();
    host.dirs
        .get_mut(&(CLAUDE_MOUNT, "projects".to_string()))
        .unwrap()
        .truncated = true;
    let out = scan(&host, &input()).unwrap();
    assert!(out.scan_truncated);
}

#[test]
fn dashing_matches_claudes_project_directory_encoding() {
    assert_eq!(dashed("/Users/ada/work/openagents.com"), "-Users-ada-work-openagents-com");
    assert_eq!(dashed("openagents.com"), "openagents-com");
}

#[test]
fn claude_meta_scans_past_leading_records_without_a_cwd() {
    let bytes = format!(
        "{}\n{}\n{}\n",
        r#"{"type":"mode","mode":"normal","sessionId":"abc"}"#,
        r#"{"type":"file-history-snapshot","messageId":"m1"}"#,
        claude_line("/Users/ada/work/proj", "abc"),
    );
    let (cwd, session_id, records) = claude_meta(bytes.as_bytes()).unwrap();
    assert_eq!(cwd, "/Users/ada/work/proj");
    assert_eq!(session_id.as_deref(), Some("abc"));
    assert_eq!(records, 3);
}

#[test]
fn codex_meta_requires_a_session_meta_first_line() {
    assert!(codex_meta(br#"{"type":"other"}"#).is_none());
    assert!(codex_meta(b"garbage").is_none());
    let (cwd, id, _) = codex_meta(codex_line("/tmp/x", "id1").as_bytes()).unwrap();
    assert_eq!(cwd, "/tmp/x");
    assert_eq!(id.as_deref(), Some("id1"));
}
