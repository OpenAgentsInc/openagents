//! The Git checks a worktree passes before it is removed, the removal
//! itself, and its undo.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

/// What recreates a removed worktree: its repository, branch, and commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Undo {
    pub repo: PathBuf,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub commit: String,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Whether Git ignores `path` inside the checkout `top`.
#[must_use]
pub fn ignored(top: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(top) else {
        return false;
    };
    Command::new("git")
        .arg("-C")
        .arg(top)
        .args(["check-ignore", "-q", "--no-index"])
        .arg(relative)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Directory and file names that only ever hold rebuildable output: build
/// directories, package installs, and tool caches.
const DISPOSABLE: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    "build",
    ".build",
    ".next",
    ".turbo",
    ".cache",
    ".parcel-cache",
    ".svelte-kit",
    ".gradle",
    "DerivedData",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".DS_Store",
];

/// Whether an ignored path (as `git status --ignored=matching` lists it) lies in
/// a disposable cache: some component is a [`DISPOSABLE`] name or a
/// `.cargo-target*` directory. Anything else may be a person's data.
#[must_use]
pub fn disposable(entry: &str) -> bool {
    entry
        .split('/')
        .any(|part| DISPOSABLE.contains(&part) || part.starts_with(".cargo-target"))
}

/// Check that the worktree at `path` holds nothing that is not saved
/// elsewhere: it is a linked worktree (its `.git` is a file), `git status`
/// is clean (untracked files count), it holds no ignored file outside a
/// disposable cache (`git worktree remove` would delete it, and undo only
/// recreates the checkout), every commit is on some remote, and
/// no stash was made on it. Returns what recreates it.
///
/// # Errors
/// Why it must stay.
pub fn removable(path: &Path) -> Result<Undo, String> {
    let dot = path.join(".git");
    match std::fs::symlink_metadata(&dot) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Err("a full checkout, not a worktree".into()),
        Err(_) => return Err("not a Git worktree".into()),
    }
    // `--ignored=matching` names an ignored directory once (`target/`)
    // without walking it, and every ignored file outside one.
    let status = git(
        path,
        &[
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--ignored=matching",
            "-z",
        ],
    )?;
    let mut ignored = Vec::new();
    for entry in status.split('\0').filter(|entry| !entry.is_empty()) {
        match entry.strip_prefix("!! ") {
            Some(path) => ignored.push(path),
            None => return Err("uncommitted changes".into()),
        }
    }
    let kept: Vec<&str> = ignored
        .into_iter()
        .filter(|entry| !disposable(entry))
        .collect();
    if !kept.is_empty() {
        let mut why = kept.iter().take(3).copied().collect::<Vec<_>>().join(", ");
        if kept.len() > 3 {
            why.push_str(&format!(" and {} more", kept.len() - 3));
        }
        return Err(format!("holds ignored files: {why}"));
    }
    let commit = git(path, &["rev-parse", "HEAD"])?;
    let unpushed = git(path, &["rev-list", "HEAD", "--not", "--remotes"])?;
    if !unpushed.is_empty() {
        return Err("commits not on any remote".into());
    }
    let branch = git(path, &["symbolic-ref", "-q", "--short", "HEAD"])
        .ok()
        .filter(|branch| !branch.is_empty());
    // A stash is the repository's, not the worktree's: keep the worktree
    // when any stash was made on its branch (or, detached, on its commit).
    let stashes = git(path, &["stash", "list", "--format=%P %gs"]).unwrap_or_default();
    for line in stashes.lines() {
        let base = line.split(' ').next().unwrap_or_default();
        let made_here = match &branch {
            Some(branch) => line
                .to_lowercase()
                .contains(&format!("on {branch}:").to_lowercase()),
            None => base == commit && line.contains("(no branch)"),
        };
        if made_here {
            return Err("a stash was made on it".into());
        }
    }
    let common = git(
        path,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let common = PathBuf::from(common);
    let repo = if common.file_name().is_some_and(|name| name == ".git") {
        common.parent().map(Path::to_owned).unwrap_or(common)
    } else {
        common
    };
    Ok(Undo {
        repo,
        path: path.to_owned(),
        branch,
        commit,
    })
}

/// Remove the worktree with `git worktree remove` (which refuses a dirty
/// one), then prune the repository's worktree list.
///
/// # Errors
/// Git refused.
pub fn remove(undo: &Undo) -> Result<(), String> {
    let path = undo.path.to_string_lossy().into_owned();
    git(&undo.repo, &["worktree", "remove", &path])?;
    let _ = git(&undo.repo, &["worktree", "prune"]);
    Ok(())
}

/// Recreate a removed worktree at its path, on its branch when the branch
/// still exists, else on a new branch of that name at the commit, else
/// detached at the commit.
///
/// # Errors
/// Git refused, or the path is taken.
pub fn restore(undo: &Undo) -> Result<(), String> {
    if undo.path.exists() {
        return Err(format!("{} already exists", undo.path.display()));
    }
    let path = undo.path.to_string_lossy().into_owned();
    let _ = git(&undo.repo, &["worktree", "prune"]);
    match &undo.branch {
        Some(branch)
            if git(
                &undo.repo,
                &[
                    "rev-parse",
                    "--verify",
                    "-q",
                    &format!("refs/heads/{branch}"),
                ],
            )
            .is_ok() =>
        {
            git(&undo.repo, &["worktree", "add", &path, branch]).map(drop)
        }
        Some(branch) => git(
            &undo.repo,
            &["worktree", "add", "-b", branch, &path, &undo.commit],
        )
        .map(drop),
        None => git(
            &undo.repo,
            &["worktree", "add", "--detach", &path, &undo.commit],
        )
        .map(drop),
    }
}
