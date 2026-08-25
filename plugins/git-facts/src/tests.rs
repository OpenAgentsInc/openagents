//! The facts reader against a fake host: fixture `.git` trees and a
//! hand-built binary index, exercised without a WASM runtime. The same
//! shapes run through the real boundary in
//! `packages/openagents-cli/test/coder-plugin-git-facts.test.ts`.

use super::*;
use openagents_pdk::MountDirEntry;
use std::collections::BTreeMap as Map;

const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccc";
const SHA_D: &str = "dddddddddddddddddddddddddddddddddddddddd";

/// A fake host over one in-memory mount. Directories are keyed by listed
/// path; file bytes by mount-relative path.
#[derive(Default)]
struct FakeHost {
    dirs: Map<String, MountDirListing>,
    files: Map<String, Vec<u8>>,
    /// Paths the whole-file read answers with `file_too_large`; the range
    /// read still serves their bytes.
    oversized: Vec<String>,
}

impl FakeHost {
    fn dir(&mut self, path: &str, entries: Vec<MountDirEntry>) {
        self.dirs
            .insert(path.to_string(), MountDirListing { entries, truncated: false });
    }
    fn file(&mut self, path: &str, bytes: impl Into<Vec<u8>>) {
        self.files.insert(path.to_string(), bytes.into());
    }
}

impl Host for FakeHost {
    fn list(&self, _mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        self.dirs.get(path).cloned().ok_or_else(|| {
            Refusal::new(RefusalCode::FileUnreadable, "the mount has no such directory")
        })
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        if self.oversized.iter().any(|p| p == path) {
            return Err(Refusal::new(RefusalCode::FileTooLarge, "over the bound"));
        }
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| Refusal::new(RefusalCode::FileUnreadable, "no such file"))
    }
    fn read_range(&self, path: &str, offset: u64, max_bytes: u32) -> Result<Vec<u8>, Refusal> {
        let bytes = self
            .files
            .get(path)
            .ok_or_else(|| Refusal::new(RefusalCode::FileUnreadable, "no such file"))?;
        let start = (offset as usize).min(bytes.len());
        let end = (start + max_bytes as usize).min(bytes.len());
        Ok(bytes[start..end].to_vec())
    }
}

fn entry(name: &str, kind: &str, size: u64, mtime_ms: i64) -> MountDirEntry {
    MountDirEntry { name: name.to_string(), kind: kind.to_string(), size, mtime_ms }
}

fn input() -> Input {
    Input { facts: None, max_log: None, max_paths: None, max_walk: None }
}

/// Build a version-2 index from `(path, size, mtime_ms)` triples: the
/// 12-byte DIRC header, 62-byte fixed entries with 8-byte-aligned NUL
/// padding after each path, and a zeroed 20-byte trailing checksum.
fn build_index(entries: &[(&str, u32, i64)]) -> Vec<u8> {
    build_index_versioned(2, entries)
}

fn build_index_versioned(version: u32, entries: &[(&str, u32, i64)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"DIRC");
    bytes.extend_from_slice(&version.to_be_bytes());
    bytes.extend_from_slice(&(entries.len() as u32).to_be_bytes());
    for (path, size, mtime_ms) in entries {
        let start = bytes.len();
        bytes.extend_from_slice(&0u32.to_be_bytes()); // ctime sec
        bytes.extend_from_slice(&0u32.to_be_bytes()); // ctime nsec
        bytes.extend_from_slice(&((mtime_ms / 1000) as u32).to_be_bytes());
        bytes.extend_from_slice(&(((mtime_ms % 1000) * 1_000_000) as u32).to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes()); // dev
        bytes.extend_from_slice(&0u32.to_be_bytes()); // ino
        bytes.extend_from_slice(&0o100644u32.to_be_bytes()); // mode
        bytes.extend_from_slice(&0u32.to_be_bytes()); // uid
        bytes.extend_from_slice(&0u32.to_be_bytes()); // gid
        bytes.extend_from_slice(&size.to_be_bytes());
        bytes.extend_from_slice(&[0u8; 20]); // object id, unread here
        bytes.extend_from_slice(&(path.len() as u16).to_be_bytes()); // flags
        bytes.extend_from_slice(path.as_bytes());
        let entry_len = (INDEX_ENTRY_FIXED + path.len() + 8) & !7;
        bytes.resize(start + entry_len, 0);
    }
    bytes.extend_from_slice(&[0u8; 20]); // trailing checksum, unverified
    bytes
}

fn reflog_line(old: &str, new: &str, timestamp: i64, message: &str) -> String {
    format!("{old} {new} Ada Lovelace <ada@example.com> {timestamp} -0600\t{message}\n")
}

/// A repo on branch `main` with one loose ref, one packed ref, a two-entry
/// reflog, a two-file index, and a workdir where one tracked file matches,
/// one drifted, one is missing, and one file is untracked.
fn seeded() -> FakeHost {
    let mut host = FakeHost::default();
    host.file(".git/HEAD", "ref: refs/heads/main\n");
    host.dir(".git/refs/heads", vec![entry("main", "file", 41, 0)]);
    host.file(".git/refs/heads/main", format!("{SHA_A}\n"));
    host.file(
        ".git/packed-refs",
        format!(
            "# pack-refs with: peeled fully-peeled sorted \n{SHA_B} refs/heads/packed-branch\n^{SHA_C}\n{SHA_C} refs/tags/v1\n"
        ),
    );
    host.file(
        ".git/logs/HEAD",
        format!(
            "{}{}",
            reflog_line(&"0".repeat(40), SHA_B, 1_700_000_000, "commit (initial): begin"),
            reflog_line(SHA_B, SHA_A, 1_700_000_100, "commit: keep\tgoing"),
        ),
    );
    host.file(
        ".git/index",
        build_index(&[
            ("src/kept.rs", 4, 1_000_000),
            ("src/drifted.rs", 4, 1_000_000),
            ("src/gone.rs", 9, 1_000_000),
        ]),
    );
    host.dir(
        "",
        vec![entry(".git", "dir", 0, 0), entry("new.txt", "file", 2, 5_000), entry("src", "dir", 0, 0)],
    );
    host.dir(
        "src",
        vec![
            entry("drifted.rs", "file", 7, 2_000_000),
            entry("kept.rs", "file", 4, 1_000_000),
        ],
    );
    host
}

#[test]
fn head_on_a_branch_names_the_branch() {
    let out = git_facts(&seeded(), &input()).unwrap();
    assert_eq!(
        out.head,
        Some(Head { branch: Some("main".to_string()), detached: None })
    );
}

#[test]
fn a_detached_head_reports_the_commit_id() {
    let mut host = seeded();
    host.file(".git/HEAD", format!("{SHA_D}\n"));
    let out = git_facts(&host, &input()).unwrap();
    assert_eq!(
        out.head,
        Some(Head { branch: None, detached: Some(SHA_D.to_string()) })
    );
}

#[test]
fn branches_merge_loose_and_packed_refs_and_loose_wins() {
    let mut host = seeded();
    // The packed twin of `main` is stale; the loose ref must shadow it.
    host.file(
        ".git/packed-refs",
        format!("{SHA_B} refs/heads/packed-branch\n{SHA_C} refs/heads/main\n"),
    );
    let out = git_facts(&host, &input()).unwrap();
    assert_eq!(
        out.branches,
        Some(vec![
            Branch { name: "main".to_string(), id: SHA_A.to_string() },
            Branch { name: "packed-branch".to_string(), id: SHA_B.to_string() },
        ])
    );
}

#[test]
fn packed_refs_only_is_named_in_notes() {
    let mut host = seeded();
    host.dirs.remove(".git/refs/heads");
    host.files.remove(".git/refs/heads/main");
    let out = git_facts(&host, &input()).unwrap();
    assert_eq!(
        out.branches,
        Some(vec![Branch { name: "packed-branch".to_string(), id: SHA_B.to_string() }])
    );
    assert!(out.notes.iter().any(|n| n.contains("packed-refs only")), "{:?}", out.notes);
}

#[test]
fn the_log_is_the_reflog_newest_first_with_tabs_kept_in_messages() {
    let out = git_facts(&seeded(), &input()).unwrap();
    let log = out.log.unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].id, SHA_A);
    assert_eq!(log[0].at_ms, 1_700_000_100_000);
    // Only the first tab splits header from message; later tabs belong
    // to the message.
    assert_eq!(log[0].message, "commit: keep\tgoing");
    assert_eq!(log[1].id, SHA_B);
    assert_eq!(log[1].message, "commit (initial): begin");
}

#[test]
fn max_log_keeps_the_newest_entries() {
    let mut host = seeded();
    let mut lines = String::new();
    for i in 0..10 {
        lines.push_str(&reflog_line(SHA_B, SHA_A, 1_700_000_000 + i, &format!("commit: {i}")));
    }
    host.file(".git/logs/HEAD", lines);
    let mut asked = input();
    asked.max_log = Some(3);
    let log = git_facts(&host, &asked).unwrap().log.unwrap();
    assert_eq!(
        log.iter().map(|e| e.message.as_str()).collect::<Vec<_>>(),
        vec!["commit: 9", "commit: 8", "commit: 7"]
    );
}

#[test]
fn a_missing_reflog_is_a_note_not_an_error() {
    let mut host = seeded();
    host.files.remove(".git/logs/HEAD");
    let out = git_facts(&host, &input()).unwrap();
    assert_eq!(out.log, Some(vec![]));
    assert!(out.notes.iter().any(|n| n.contains("no reflog")), "{:?}", out.notes);
}

#[test]
fn the_hand_built_index_parses_to_the_right_paths_and_sizes() {
    let bytes = build_index(&[("a.txt", 5, 12_345), ("dir/b.txt", 700, 999_000)]);
    let mut notes = Vec::new();
    let parsed = parse_index(&bytes, false, &mut notes);
    assert!(parsed.complete);
    assert_eq!(parsed.declared, 2);
    assert_eq!(
        parsed.entries,
        vec![
            IndexEntry { path: "a.txt".to_string(), size: 5, mtime_ms: 12_345 },
            IndexEntry { path: "dir/b.txt".to_string(), size: 700, mtime_ms: 999_000 },
        ]
    );
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn status_classifies_changed_untracked_and_missing() {
    let out = git_facts(&seeded(), &input()).unwrap();
    let status = out.status.unwrap();
    assert_eq!(status.tracked, 3);
    assert_eq!(status.changed_candidates.paths, vec!["src/drifted.rs"]);
    assert_eq!(status.changed_candidates.count, 1);
    assert_eq!(status.untracked.paths, vec!["new.txt"]);
    assert_eq!(status.missing.paths, vec!["src/gone.rs"]);
    assert!(!status.walk_truncated);
    assert_eq!(status.comparison, "size_and_mtime_only");
}

#[test]
fn a_same_size_mtime_drift_is_a_changed_candidate() {
    let mut host = seeded();
    // Same size as the index records, different mtime.
    host.dir(
        "src",
        vec![
            entry("drifted.rs", "file", 4, 3_000_000),
            entry("kept.rs", "file", 4, 1_000_000),
        ],
    );
    let status = git_facts(&host, &input()).unwrap().status.unwrap();
    assert_eq!(status.changed_candidates.paths, vec!["src/drifted.rs"]);
}

#[test]
fn no_git_head_is_a_refusal_about_the_workspace() {
    let mut host = FakeHost::default();
    host.dir("", vec![entry("README.md", "file", 10, 0)]);
    let refusal = git_facts(&host, &input()).unwrap_err();
    assert_eq!(refusal.code, RefusalCode::Unsupported);
    assert!(refusal.reason.contains("no git repository"), "{}", refusal.reason);
}

#[test]
fn a_missing_index_means_everything_walked_is_untracked() {
    let mut host = seeded();
    host.files.remove(".git/index");
    let out = git_facts(&host, &input()).unwrap();
    let status = out.status.unwrap();
    assert_eq!(status.tracked, 0);
    assert_eq!(status.untracked.count, 3);
    assert_eq!(status.missing.count, 0);
    assert!(out.notes.iter().any(|n| n.contains("no .git/index")), "{:?}", out.notes);
}

#[test]
fn an_index_version_above_2_is_named_and_never_misparsed() {
    let mut host = seeded();
    host.file(".git/index", build_index_versioned(3, &[("a.txt", 1, 1000)]));
    let out = git_facts(&host, &input()).unwrap();
    let status = out.status.unwrap();
    // The header's count is still honest; the per-entry facts are not
    // guessed, and the comparison is refused rather than calling every
    // tracked file untracked.
    assert_eq!(status.tracked, 1);
    assert_eq!(status.changed_candidates.count, 0);
    assert_eq!(status.untracked.count, 0);
    assert_eq!(status.missing.count, 0);
    assert!(
        out.notes.iter().any(|n| n.contains("index_version_unsupported") && n.contains("version 3")),
        "{:?}",
        out.notes
    );
    assert!(out.notes.iter().any(|n| n.contains("not computed")), "{:?}", out.notes);
}

#[test]
fn an_oversized_index_is_read_from_the_front_whole_entries_only() {
    let mut host = seeded();
    let full = build_index(&[("a.txt", 5, 1_000_000), ("b.txt", 4, 1_000_000)]);
    // Serve a prefix that cuts the second entry in half; the range read
    // returns what exists and the parser must keep only whole entries.
    let cut = 12 + ((INDEX_ENTRY_FIXED + "a.txt".len() + 8) & !7) + 10;
    host.file(".git/index", full[..cut].to_vec());
    host.oversized.push(".git/index".to_string());
    host.dir(
        "",
        vec![
            entry(".git", "dir", 0, 0),
            entry("a.txt", "file", 5, 1_000_000),
            entry("b.txt", "file", 4, 1_000_000),
        ],
    );
    let out = git_facts(&host, &input()).unwrap();
    let status = out.status.unwrap();
    assert_eq!(status.tracked, 2);
    // `a.txt` parsed and matches; `b.txt` fell past the cut, so the
    // comparison sees it as untracked — and a note owns that.
    assert_eq!(status.changed_candidates.count, 0);
    assert_eq!(status.untracked.paths, vec!["b.txt"]);
    assert_eq!(status.missing.count, 0);
    assert!(out.notes.iter().any(|n| n.contains("index_truncated")), "{:?}", out.notes);
    assert!(out.notes.iter().any(|n| n.contains("truncation point")), "{:?}", out.notes);
}

#[test]
fn the_walk_budget_truncates_and_says_so() {
    let mut asked = input();
    asked.max_walk = Some(1);
    let out = git_facts(&seeded(), &asked).unwrap();
    let status = out.status.unwrap();
    assert!(status.walk_truncated);
    assert!(out.notes.iter().any(|n| n.contains("walk stopped")), "{:?}", out.notes);
}

#[test]
fn max_paths_bounds_the_listing_but_not_the_count() {
    let mut host = seeded();
    host.files.remove(".git/index");
    let mut root = vec![entry(".git", "dir", 0, 0)];
    for i in 0..30 {
        root.push(entry(&format!("file-{i:02}.txt"), "file", 1, 0));
    }
    host.dir("", root);
    let mut asked = input();
    asked.max_paths = Some(5);
    let status = git_facts(&host, &asked).unwrap().status.unwrap();
    assert_eq!(status.untracked.count, 30);
    assert_eq!(status.untracked.paths.len(), 5);
    assert_eq!(status.untracked.paths[0], "file-00.txt");
}

#[test]
fn facts_selection_reports_only_what_was_asked() {
    let mut asked = input();
    asked.facts = Some(vec!["head".to_string(), "weather".to_string()]);
    let out = git_facts(&seeded(), &asked).unwrap();
    assert!(out.head.is_some());
    assert!(out.branches.is_none());
    assert!(out.log.is_none());
    assert!(out.status.is_none());
    assert!(out.notes.iter().any(|n| n.contains("weather")), "{:?}", out.notes);
}

#[test]
fn a_long_name_with_the_0xfff_flag_is_read_to_its_nul() {
    // Build one entry whose flags claim the 0xFFF overflow length; the
    // parser must fall back to scanning for the NUL terminator.
    let long = "d/".repeat(30) + "x.txt";
    let mut bytes = build_index(&[(long.as_str(), 3, 1000)]);
    // Overwrite the flags field with 0xFFF.
    let flags_at = 12 + 60;
    bytes[flags_at] = 0x0F;
    bytes[flags_at + 1] = 0xFF;
    let mut notes = Vec::new();
    let parsed = parse_index(&bytes, false, &mut notes);
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].path, long);
}
