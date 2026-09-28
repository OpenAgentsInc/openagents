//! Nonmutating change collection: what the working tree changed, read
//! without touching the index.
//!
//! Collection used to run `git add -N .`, which rewrites the index of the
//! workspace it observes, a real change on a Git recovery task. This module
//! reads tracked differences with `git diff --binary <base>` and captures
//! each untracked file on its own with `git diff --no-index`, under
//! explicit bounds. Every Git call runs with optional locks off, so Git
//! never refreshes the index as a side effect.
//!
//! A [`Collection`] is tagged with the workspace [`revision`] before and
//! after it read. When the two differ, something wrote during the read,
//! and the collection is stale rather than an observation of one
//! consistent candidate.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::ops::hex;

/// How much one collection may capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    /// The most untracked files captured.
    pub files: usize,
    /// The largest untracked file captured, in bytes.
    pub file_bytes: u64,
    /// The most untracked bytes captured in all.
    pub total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            files: 200,
            file_bytes: 1024 * 1024,
            total_bytes: 8 * 1024 * 1024,
        }
    }
}

/// What happened to one untracked file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kept {
    Captured,
    OverFileLimit,
    OverTotalLimit,
    OverCountLimit,
    Unreadable,
}

/// One untracked file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Untracked {
    pub path: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub kept: Kept,
}

/// The working tree's change against a base commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub base: String,
    /// The workspace revision before and after the read.
    pub revision_before: Option<String>,
    pub revision_after: Option<String>,
    /// The tracked diff and every captured untracked file, as one patch.
    #[serde(skip)]
    pub patch: String,
    pub tracked_bytes: u64,
    pub untracked: Vec<Untracked>,
    pub limits: Limits,
}

impl Collection {
    /// Whether the workspace changed while it was read.
    #[must_use]
    pub fn stale(&self) -> bool {
        self.revision_before != self.revision_after
    }

    /// Whether every untracked file was captured.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.untracked
            .iter()
            .all(|file| file.kept == Kept::Captured)
    }

    /// The record the bundle keeps beside the patch.
    #[must_use]
    pub fn record(&self) -> Value {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        value["schema"] = "openagents.coder-one.collection.v1".into();
        value["stale"] = self.stale().into();
        value["complete"] = self.complete().into();
        value["patch_sha256"] = hex(&Sha256::digest(self.patch.as_bytes())).into();
        value["patch_bytes"] = self.patch.len().into();
        value
    }
}

/// Runs Git read-only in `workdir`: optional locks off, no prompt, no
/// pager. `None` when it cannot run; `accept` lists the exit codes that
/// count as an answer.
fn git(workdir: &Path, args: &[&str], accept: &[i32]) -> Option<Vec<u8>> {
    let mut command = Command::new("git");
    command
        .args(crate::ops::READ_ONLY_GIT)
        .args(args)
        .current_dir(workdir);
    crate::ops::quiet_environment(&mut command);
    let output = command.output().ok()?;
    let code = output.status.code()?;
    accept.contains(&code).then_some(output.stdout)
}

/// The workspace revision: a digest of HEAD, Git's porcelain status with
/// every untracked file, and each changed path's size and modification
/// time. Two reads that return the same revision saw the same tree, as far
/// as Git's own change detection can tell. `None` outside a work tree.
#[must_use]
pub fn revision(workdir: &Path) -> Option<String> {
    let head = git(workdir, &["rev-parse", "--verify", "-q", "HEAD"], &[0, 1])?;
    let status = git(
        workdir,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        &[0],
    )?;
    let mut hasher = Sha256::new();
    hasher.update(&head);
    hasher.update(&status);
    for entry in status.split(|&b| b == 0) {
        let Some(path) = entry.get(3..) else { continue };
        let path = String::from_utf8_lossy(path);
        if let Ok(meta) = std::fs::symlink_metadata(workdir.join(path.as_ref())) {
            let modified = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |elapsed| elapsed.as_nanos());
            hasher.update(format!("{path}\0{}\0{modified}\n", meta.len()));
        }
    }
    Some(hex(&hasher.finalize()))
}

/// Collects the change against `base` without writing to the workspace.
/// `None` outside a Git work tree or when the tracked diff cannot be read.
#[must_use]
pub fn collect(workdir: &Path, base: &str, limits: Limits) -> Option<Collection> {
    let revision_before = revision(workdir);
    let tracked = git(workdir, &["diff", "--binary", base, "--"], &[0])?;
    let mut patch = String::from_utf8_lossy(&tracked).into_owned();
    let listed = git(
        workdir,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        &[0],
    )
    .unwrap_or_default();
    let mut paths: Vec<String> = listed
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect();
    paths.sort();
    let mut untracked = Vec::new();
    let mut total: u64 = 0;
    let mut captured = 0usize;
    for path in paths {
        let full = workdir.join(&path);
        let bytes = std::fs::symlink_metadata(&full).map_or(0, |m| m.len());
        let kept = if captured >= limits.files {
            Kept::OverCountLimit
        } else if bytes > limits.file_bytes {
            Kept::OverFileLimit
        } else if total + bytes > limits.total_bytes {
            Kept::OverTotalLimit
        } else {
            Kept::Captured
        };
        let mut sha256 = None;
        let mut kept = kept;
        if kept == Kept::Captured {
            match git(
                workdir,
                &["diff", "--no-index", "--binary", "--", "/dev/null", &path],
                &[0, 1],
            ) {
                Some(diff) => {
                    captured += 1;
                    total += bytes;
                    sha256 = std::fs::read(&full)
                        .ok()
                        .map(|content| hex(&Sha256::digest(&content)));
                    patch.push_str(&String::from_utf8_lossy(&diff));
                }
                None => kept = Kept::Unreadable,
            }
        }
        untracked.push(Untracked {
            path,
            bytes,
            sha256,
            kept,
        });
    }
    Some(Collection {
        base: base.to_string(),
        revision_before,
        revision_after: revision(workdir),
        tracked_bytes: tracked.len() as u64,
        patch,
        untracked,
        limits,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::process::Command;

    /// A fixture repository with a commit, a modified tracked file, a
    /// tracked file whose stat changed but content did not (so a plain
    /// `git status` would rewrite the index), a stash, a branch, and
    /// untracked files.
    pub(crate) fn fixture() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(root)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@example.com")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@example.com")
                .output()
                .unwrap();
            assert!(status.status.success(), "git {args:?}: {status:?}");
            String::from_utf8_lossy(&status.stdout).trim().to_string()
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        std::fs::write(root.join("b.txt"), "two\n").unwrap();
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::write(root.join("data/log.txt"), "2025-08-10 ERROR x\n").unwrap();
        std::fs::write(root.join("README.md"), "# fixture\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "first"]);
        git(&["branch", "side"]);
        std::fs::write(root.join("a.txt"), "stashed\n").unwrap();
        git(&["stash", "-q"]);
        let base = git(&["rev-parse", "HEAD"]);
        std::fs::write(root.join("a.txt"), "one\nchanged\n").unwrap();
        // Same content, new modification time: stat-dirty in the index.
        std::fs::write(root.join("b.txt"), "two\n").unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(root.join("b.txt"))
            .unwrap()
            .set_modified(later)
            .unwrap();
        std::fs::write(root.join("new.txt"), "untracked\n").unwrap();
        std::fs::write(root.join("big.bin"), vec![7u8; 4096]).unwrap();
        (dir, base)
    }

    #[test]
    fn collection_captures_tracked_and_untracked_changes() {
        let (dir, base) = fixture();
        let collection = collect(dir.path(), &base, Limits::default()).unwrap();
        assert!(collection.patch.contains("+changed"));
        assert!(collection.patch.contains("new file mode"));
        assert!(collection.patch.contains("b/new.txt"));
        assert!(!collection.stale());
        assert!(collection.complete());
        assert_eq!(collection.untracked.len(), 2);
    }

    #[test]
    fn bounds_leave_files_named_but_uncaptured() {
        let (dir, base) = fixture();
        let limits = Limits {
            files: 10,
            file_bytes: 100,
            total_bytes: 1_000,
        };
        let collection = collect(dir.path(), &base, limits).unwrap();
        let big = collection
            .untracked
            .iter()
            .find(|file| file.path == "big.bin")
            .unwrap();
        assert_eq!(big.kept, Kept::OverFileLimit);
        assert!(!collection.complete());
        assert!(!collection.patch.contains("big.bin"));
    }

    #[test]
    fn a_write_between_reads_changes_the_revision() {
        let (dir, _) = fixture();
        let before = revision(dir.path()).unwrap();
        assert_eq!(revision(dir.path()).unwrap(), before);
        std::fs::write(dir.path().join("new.txt"), "rewritten, longer\n").unwrap();
        assert_ne!(revision(dir.path()).unwrap(), before);
    }
}
