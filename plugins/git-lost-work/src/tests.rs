//! Unit tests for `git-lost-work` against a fake in-memory `.git` tree.

use super::*;
use openagents_pdk::{MountDirEntry, Refusal, RefusalCode};
use std::collections::BTreeMap;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccccccccccc";
const D: &str = "dddddddddddddddddddddddddddddddddddddddd";
const E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const ZERO: &str = "0000000000000000000000000000000000000000";

#[derive(Default)]
struct FakeHost {
    dirs: BTreeMap<(u32, String), MountDirListing>,
    files: BTreeMap<String, Vec<u8>>,
}

impl FakeHost {
    fn dir(&mut self, mount: u32, path: &str, entries: Vec<MountDirEntry>) {
        self.dirs.insert((mount, path.to_string()), MountDirListing { entries, truncated: false });
    }
    fn file(&mut self, path: &str, text: &str) {
        self.files.insert(path.to_string(), text.as_bytes().to_vec());
    }
    fn file_bytes(&mut self, path: &str, bytes: Vec<u8>) {
        self.files.insert(path.to_string(), bytes);
    }
}

impl Host for FakeHost {
    fn list(&self, mount_index: u32, path: &str) -> Result<MountDirListing, Refusal> {
        self.dirs
            .get(&(mount_index, path.to_string()))
            .cloned()
            .ok_or_else(|| Refusal::new(RefusalCode::FileUnreadable, "no such directory"))
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, Refusal> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| Refusal::new(RefusalCode::FileUnreadable, "no such file"))
    }
}

fn entry(name: &str, kind: &str, size: u64, mtime_ms: i64) -> MountDirEntry {
    MountDirEntry { name: name.to_string(), kind: kind.to_string(), size, mtime_ms }
}

fn zlib(text: &str) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(text.as_bytes(), 6)
}

fn commit_object(subject: &str, author: &str, timestamp: i64) -> String {
    format!(
        "commit {}\0tree ffffffffffffffffffffffffffffffffffffffff\nauthor {author} {timestamp} +0000\ncommitter {author} {timestamp} +0000\n\n{subject}\n",
        100 // placeholder size; parser only checks header prefix
    )
}

/// Standard repo with branch HEAD, packed-refs, loose and packed lost commits,
/// and one stash entry.
fn repo_with_lost() -> FakeHost {
    let mut h = FakeHost::default();

    // Root mount lists .git as a directory.
    h.dir(0, ".", vec![entry(".git", "dir", 0, 0)]);

    h.file(".git/HEAD", "ref: refs/heads/main\n");
    h.file(".git/refs/heads/main", &format!("{A}\n"));
    h.file(".git/packed-refs", &format!("{D} refs/heads/feature\n"));

    h.dir(0, ".git/refs/heads", vec![entry("main", "file", 40, 0)]);
    h.dir(0, ".git/logs/refs/heads", vec![entry("main", "file", 0, 0)]);

    h.file(
        ".git/logs/HEAD",
        &format!(
            "{ZERO} {A} Alice <a@b.com> 1700000000 +0000\tcommit (initial): init\n\
             {A} {B} Alice <a@b.com> 1700000100 +0000\tcommit: wip\n\
             {B} {A} Alice <a@b.com> 1700000200 +0000\treset: moving to main\n"
        ),
    );
    h.file(
        ".git/logs/refs/heads/main",
        &format!(
            "{ZERO} {A} Alice <a@b.com> 1700000000 +0000\tcommit (initial): init\n\
             {A} {C} Alice <a@b.com> 1700000050 +0000\tcherry-pick: c\n\
             {C} {A} Alice <a@b.com> 1700000060 +0000\treset: moving to a\n"
        ),
    );
    h.file(
        ".git/logs/refs/stash",
        &format!(
            "{ZERO} {E} Alice <a@b.com> 1700000300 +0000\tWIP on main: wip\n"
        ),
    );

    // Loose object for B (a real commit).
    let b_obj = commit_object("fix: bug", "Alice <a@b.com>", 1700000100);
    let b_obj_path = format!(".git/objects/{}/{}", &B[..2], &B[2..]);
    h.file_bytes(&b_obj_path, zlib(&b_obj));

    // C has no loose object; it is reported as packed.
    // E (stash) has no object; stashes do not need one.

    h
}

#[test]
fn finds_branch_head_and_reachable_tips() {
    let out = scan(&repo_with_lost(), &Input { max_lost_commits: None }).unwrap();
    assert_eq!(out.head.branch.as_deref(), Some("main"));
    assert_eq!(out.head.sha.as_deref(), Some(A));
}

#[test]
fn reports_lost_loose_and_packed_commits() {
    let out = scan(&repo_with_lost(), &Input { max_lost_commits: None }).unwrap();
    assert_eq!(out.lost_commits.len(), 2);

    let b = out.lost_commits.iter().find(|c| c.sha == B).unwrap();
    assert!(!b.packed);
    assert_eq!(b.subject.as_deref(), Some("fix: bug"));
    assert_eq!(b.author.as_deref(), Some("Alice <a@b.com>"));
    assert_eq!(b.author_date, Some(1700000100));
    assert_eq!(b.action.as_deref(), Some("reset: moving to main"));
    assert_eq!(b.timestamp, 1700000200);

    let c = out.lost_commits.iter().find(|c| c.sha == C).unwrap();
    assert!(c.packed);
    assert_eq!(c.subject, None);
    assert_eq!(c.author, None);
    assert_eq!(c.author_date, None);
    assert_eq!(c.action.as_deref(), Some("reset: moving to a"));
    assert_eq!(c.timestamp, 1700000060);
}

#[test]
fn reports_stash_entries() {
    let out = scan(&repo_with_lost(), &Input { max_lost_commits: None }).unwrap();
    assert_eq!(out.stash_entries.len(), 1);
    assert_eq!(out.stash_entries[0].selector, "stash@{0}");
    assert_eq!(out.stash_entries[0].sha, E);
    assert_eq!(out.stash_entries[0].message.as_deref(), Some("WIP on main: wip"));
    assert_eq!(out.stash_entries[0].timestamp, 1700000300);
}

#[test]
fn summary_counts_are_correct() {
    let out = scan(&repo_with_lost(), &Input { max_lost_commits: None }).unwrap();
    assert_eq!(out.summary.total_lost_candidates, 2);
    assert_eq!(out.summary.stashes_count, 1);
}

#[test]
fn max_lost_commits_caps_and_defaults_to_fifty() {
    let out = scan(&repo_with_lost(), &Input { max_lost_commits: Some(1) }).unwrap();
    assert_eq!(out.lost_commits.len(), 1);
    let out = scan(&repo_with_lost(), &Input { max_lost_commits: Some(999) }).unwrap();
    assert_eq!(out.lost_commits.len(), 2);
    let out = scan(&repo_with_lost(), &Input { max_lost_commits: None }).unwrap();
    assert_eq!(out.lost_commits.len(), 2);
}

#[test]
fn lost_commits_are_sorted_by_timestamp_desc_then_sha_asc() {
    let out = scan(&repo_with_lost(), &Input { max_lost_commits: None }).unwrap();
    let shas: Vec<&str> = out.lost_commits.iter().map(|c| c.sha.as_str()).collect();
    assert_eq!(shas, vec![B, C]);
    assert!(out.lost_commits[0].timestamp >= out.lost_commits[1].timestamp);
}

#[test]
fn detached_head_is_reported() {
    let mut h = FakeHost::default();
    h.dir(0, ".", vec![entry(".git", "dir", 0, 0)]);
    h.file(".git/HEAD", &format!("{B}\n"));
    h.file(".git/logs/HEAD", &format!("{ZERO} {A} Alice <a@b.com> 1700000000 +0000\tcommit (initial): init\n{A} {B} Alice <a@b.com> 1700000100 +0000\tcheckout: moving to b\n"));
    h.file(".git/refs/heads/main", &format!("{A}\n"));
    h.dir(0, ".git/refs/heads", vec![entry("main", "file", 40, 0)]);
    h.dir(0, ".git/logs/refs/heads", vec![]);

    let out = scan(&h, &Input { max_lost_commits: None }).unwrap();
    assert_eq!(out.head.branch, None);
    assert_eq!(out.head.sha.as_deref(), Some(B));
    // A is still a current ref tip (main), so it is not lost.
    assert!(out.lost_commits.is_empty());
}

#[test]
fn missing_git_directory_returns_empty_output() {
    let h = FakeHost::default();
    let out = scan(&h, &Input { max_lost_commits: None }).unwrap();
    assert_eq!(out, Output::default());
}

#[test]
fn malformed_reflog_lines_and_corrupt_objects_do_not_panic() {
    let mut h = repo_with_lost();
    // Append a malformed line to HEAD reflog.
    let mut head_log = String::from_utf8(h.files[".git/logs/HEAD"].clone()).unwrap();
    head_log.push_str("this is not a valid reflog line\n");
    h.file(".git/logs/HEAD", &head_log);

    // Overwrite B's object with corrupt zlib-looking bytes.
    h.file_bytes(&format!(".git/objects/{}/{}", &B[..2], &B[2..]), b"not zlib".to_vec());

    let out = scan(&h, &Input { max_lost_commits: None }).unwrap();
    // B is now reported as packed because the object could not be parsed.
    let b = out.lost_commits.iter().find(|c| c.sha == B).unwrap();
    assert!(b.packed);
    assert_eq!(b.subject, None);
}

#[test]
fn fallback_stash_ref_is_used_when_log_is_missing() {
    let mut h = repo_with_lost();
    h.files.remove(".git/logs/refs/stash");
    h.file(".git/refs/stash", &format!("{E}\n"));
    let out = scan(&h, &Input { max_lost_commits: None }).unwrap();
    assert_eq!(out.stash_entries.len(), 1);
    assert_eq!(out.stash_entries[0].selector, "stash@{0}");
    assert_eq!(out.stash_entries[0].sha, E);
    assert_eq!(out.stash_entries[0].message, None);
    assert_eq!(out.stash_entries[0].timestamp, 0);
}

#[test]
fn zlib_roundtrip_for_commit_object() {
    let text = commit_object("fix: bug", "Alice <a@b.com>", 1700000100);
    let compressed = zlib(&text);
    let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib(&compressed).unwrap();
    let back = String::from_utf8_lossy(&decompressed);
    eprintln!("text={text:?}");
    eprintln!("back={back:?}");
    assert_eq!(text, back.as_ref());
}

#[test]
fn commit_object_parses_directly() {
    let text = commit_object("fix: bug", "Alice <a@b.com>", 1700000100);
    let compressed = zlib(&text);
    let result = crate::parse_commit_object(&compressed);
    eprintln!("result={result:?}");
    let (subject, author, date) = result.unwrap();
    assert_eq!(subject, "fix: bug");
    assert_eq!(author, "Alice <a@b.com>");
    assert_eq!(date, 1700000100);
}

#[test]
fn output_is_deterministic_across_runs() {
    let a = scan(&repo_with_lost(), &Input { max_lost_commits: None }).unwrap();
    let b = scan(&repo_with_lost(), &Input { max_lost_commits: None }).unwrap();
    assert_eq!(a, b);
}
