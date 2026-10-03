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
    removable_with(path, None)
}

/// [`removable`], where `published` is a commit that already holds the
/// worktree's whole content (a change published as a commit the worktree
/// itself never made, such as Coder's `git commit-tree` publication):
/// uncommitted changes then count as saved when the worktree's content,
/// tracked and untracked, is exactly that commit's tree and the commit is
/// on some remote. Ignored files and stashes are checked as always, and
/// the undo recreates the worktree at the published commit.
///
/// # Errors
/// Why it must stay.
pub fn removable_with(path: &Path, published: Option<&str>) -> Result<Undo, String> {
    let local = local_state(path, published, false)?;
    let tips: Vec<&str> = vec![local.head.as_str(), local.commit.as_str()];
    let unpushed = unpushed(path, &tips)?;
    if tips.iter().any(|tip| unpushed.contains(*tip)) {
        return Err("commits not on any remote".into());
    }
    let stashes = git(path, &["stash", "list", "--format=%P %gs"]).unwrap_or_default();
    finish(local, &stashes)
}

/// [`removable`] for many worktrees at once, in `paths`' order, for a
/// listing (#10304). The checks are the same; Git runs for several
/// worktrees at a time, and what a repository answers for all its
/// worktrees (which commits are on no remote, its stashes) is asked once
/// per repository instead of once per worktree.
#[must_use]
pub fn removable_all(paths: &[PathBuf]) -> Vec<Result<Undo, String>> {
    let locals = crate::pool::map(paths, |path| local_state(path, None, true));
    // One `rev-list` and one `stash list` per repository.
    let mut repos: std::collections::BTreeMap<PathBuf, (PathBuf, Vec<String>)> =
        std::collections::BTreeMap::new();
    for (path, local) in paths.iter().zip(&locals) {
        if let Ok(local) = local {
            repos
                .entry(local.common.clone())
                .or_insert_with(|| (path.clone(), Vec::new()))
                .1
                .push(local.head.clone());
        }
    }
    let repos: Vec<(PathBuf, PathBuf, Vec<String>)> = repos
        .into_iter()
        .map(|(common, (path, tips))| (common, path, tips))
        .collect();
    let facts = crate::pool::map(&repos, |(_, path, tips)| {
        let stashes = git(path, &["stash", "list", "--format=%P %gs"]).unwrap_or_default();
        let tips: Vec<&str> = tips.iter().map(String::as_str).collect();
        (unpushed(path, &tips), stashes)
    });
    let facts: std::collections::BTreeMap<&PathBuf, _> = repos
        .iter()
        .map(|(common, _, _)| common)
        .zip(facts)
        .collect();
    locals
        .into_iter()
        .map(|local| {
            let local = local?;
            let (unpushed, stashes) = &facts[&local.common];
            let unpushed = unpushed.as_ref().map_err(Clone::clone)?;
            if unpushed.contains(&local.head) {
                return Err("commits not on any remote".into());
            }
            finish(local, stashes)
        })
        .collect()
}

/// What one worktree says of itself: it passed the checks that need only
/// it (linked, clean or exactly its published commit, no ignored file
/// outside a cache).
struct Local {
    path: PathBuf,
    head: String,
    /// `HEAD`, or the published commit that holds its content.
    commit: String,
    branch: Option<String>,
    common: PathBuf,
}

/// `quick` first asks Git about tracked files alone, which does not walk
/// the folders: a tracked change already decides "uncommitted changes"
/// (with no published commit, the answer the full check gives first), so
/// the walk for untracked and ignored files is skipped.
fn local_state(path: &Path, published: Option<&str>, quick: bool) -> Result<Local, String> {
    let dot = path.join(".git");
    match std::fs::symlink_metadata(&dot) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Err("a full checkout, not a worktree".into()),
        Err(_) => return Err("not a Git worktree".into()),
    }
    if quick
        && published.is_none()
        && !git(
            path,
            &["status", "--porcelain", "--untracked-files=no", "-z"],
        )?
        .is_empty()
    {
        return Err("uncommitted changes".into());
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
    let mut changed = false;
    for entry in status.split('\0').filter(|entry| !entry.is_empty()) {
        match entry.strip_prefix("!! ") {
            Some(path) => ignored.push(path),
            None => changed = true,
        }
    }
    let saved_at = match (changed, published) {
        (false, _) => None,
        (true, None) => return Err("uncommitted changes".into()),
        (true, Some(commit)) => {
            let want = git(
                path,
                &["rev-parse", "--verify", &format!("{commit}^{{tree}}")],
            )
            .map_err(|_| "its published commit is not in this repository".to_owned())?;
            if content_tree(path)? != want {
                return Err("uncommitted changes since it was published".into());
            }
            Some(commit)
        }
    };
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
    // The common directory, `HEAD`, and its branch in one call.
    let facts = git(
        path,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
            "HEAD",
            "--symbolic-full-name",
            "HEAD",
        ],
    )?;
    let mut lines = facts.lines();
    let (Some(common), Some(head), symbolic) = (lines.next(), lines.next(), lines.next()) else {
        return Err("git rev-parse: unexpected output".into());
    };
    let branch = symbolic
        .and_then(|name| name.strip_prefix("refs/heads/"))
        .filter(|branch| !branch.is_empty())
        .map(str::to_owned);
    let head = head.to_owned();
    Ok(Local {
        path: path.to_owned(),
        commit: saved_at.map_or_else(|| head.clone(), str::to_owned),
        head,
        branch,
        common: PathBuf::from(common),
    })
}

/// The commits of `tips` (and their history) on no remote. A tip is on no
/// remote exactly when it is in this set: a tip some remote has brings its
/// whole history with it.
fn unpushed(path: &Path, tips: &[&str]) -> Result<std::collections::HashSet<String>, String> {
    let mut args = vec!["rev-list"];
    args.extend(tips.iter().copied());
    args.extend(["--not", "--remotes"]);
    Ok(git(path, &args)?.lines().map(str::to_owned).collect())
}

/// The checks after a worktree's own: no stash was made on it. Returns
/// what recreates it.
fn finish(local: Local, stashes: &str) -> Result<Undo, String> {
    // A stash is the repository's, not the worktree's: keep the worktree
    // when any stash was made on its branch (or, detached, on its commit).
    for line in stashes.lines() {
        let base = line.split(' ').next().unwrap_or_default();
        let made_here = match &local.branch {
            Some(branch) => line
                .to_lowercase()
                .contains(&format!("on {branch}:").to_lowercase()),
            None => base == local.commit && line.contains("(no branch)"),
        };
        if made_here {
            return Err("a stash was made on it".into());
        }
    }
    let common = local.common;
    let repo = if common.file_name().is_some_and(|name| name == ".git") {
        common.parent().map(Path::to_owned).unwrap_or(common)
    } else {
        common
    };
    Ok(Undo {
        repo,
        path: local.path,
        branch: local.branch,
        commit: local.commit,
    })
}

/// The tree of the worktree's whole content, tracked and untracked (not
/// ignored), written through a private index so the worktree's own index
/// is untouched.
fn content_tree(path: &Path) -> Result<String, String> {
    let index = std::env::temp_dir().join(format!(
        "openagents-removable-{}-{}.index",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    ));
    let with_index = |args: &[&str]| -> Result<String, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .env("GIT_INDEX_FILE", &index)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|error| format!("git: {error}"))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        } else {
            Err(format!(
                "git {}: {}",
                args.first().copied().unwrap_or_default(),
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    };
    let tree = with_index(&["read-tree", "HEAD"])
        .and_then(|_| with_index(&["add", "-A", "--", "."]))
        .and_then(|_| with_index(&["write-tree"]));
    let _ = std::fs::remove_file(&index);
    let _ = std::fs::remove_file(index.with_extension("index.lock"));
    tree
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

/// [`remove`] for a worktree [`removable_with`] passed on a published
/// commit: its uncommitted changes are exactly that commit's tree, so the
/// removal is forced past them.
///
/// # Errors
/// Git refused.
pub fn remove_published(undo: &Undo) -> Result<(), String> {
    let path = undo.path.to_string_lossy().into_owned();
    git(&undo.repo, &["worktree", "remove", "--force", &path])?;
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
