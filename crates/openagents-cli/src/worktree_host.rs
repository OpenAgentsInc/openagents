//! Host-owned git worktrees for Coder implement tabs (#319).
//!
//! AGENTS.md requires a fresh worktree per unit. The model used to shell
//! `git worktree add` against the shared checkout. These helpers create the
//! tree under a managed directory, point `CARGO_TARGET_DIR` outside it, and
//! never touch foreign WIP on the original cwd.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// A worktree this session created and may later finish.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManagedWorktree {
    pub path: PathBuf,
    pub branch: String,
    pub head: String,
    pub parent: PathBuf,
    pub cargo_target_dir: PathBuf,
}

/// Whether `cwd` is a git checkout with uncommitted changes (including
/// untracked files). Non-git directories are not dirty.
pub fn is_dirty(cwd: &Path) -> bool {
    let status = Command::new("git")
        .args(["-C", &cwd.display().to_string(), "status", "--porcelain"])
        .output();
    match status {
        Ok(out) if out.status.success() => !out.stdout.is_empty(),
        _ => false,
    }
}

/// Root for disposable Coder worktrees.
pub fn managed_root() -> PathBuf {
    crate::auth::home_directory()
        .join(".openagents")
        .join("worktrees")
}

/// Shared cargo cache outside any disposable tree.
pub fn cargo_target_dir() -> PathBuf {
    crate::auth::home_directory()
        .join(".openagents")
        .join("cargo-target")
}

/// Fetch `origin`/`openagents` `main` when a remote exists, then add a
/// detached worktree of that tip (HEAD if fetch is unavailable).
pub fn start(
    cwd: &Path,
    managed_root: &Path,
    cargo_target: &Path,
) -> Result<ManagedWorktree, String> {
    let repo = git_toplevel(cwd)?;
    let _ = fetch_main(&repo);
    let head = git_stdout(&repo, &["rev-parse", "HEAD"])?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let branch = format!("coder-wt-{stamp}");
    let path = managed_root.join(format!("{}-{stamp}", std::process::id()));
    fs::create_dir_all(managed_root).map_err(|e| format!("could not create worktree root: {e}"))?;
    fs::create_dir_all(cargo_target).map_err(|e| format!("could not create cargo cache: {e}"))?;
    let out = Command::new("git")
        .current_dir(&repo)
        .args(["worktree", "add", "--detach"])
        .arg(&path)
        .arg(&head)
        .output()
        .map_err(|e| format!("could not run git worktree add: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git worktree add refused: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    // A local branch name so finish can prune it; the tree itself is detached.
    let _ = Command::new("git")
        .current_dir(&path)
        .args(["checkout", "-B", &branch])
        .output();
    Ok(ManagedWorktree {
        path,
        branch,
        head,
        parent: repo,
        cargo_target_dir: cargo_target.to_path_buf(),
    })
}

/// Remove the worktree when `landed` is true (unit reached origin/main).
/// Otherwise leave it and return a checkpoint sentence naming the path.
pub fn finish(worktree: &ManagedWorktree, landed: bool) -> Result<String, String> {
    if !landed {
        return Ok(format!(
            "worktree left at {} (branch {}). It has not landed on main.",
            worktree.path.display(),
            worktree.branch
        ));
    }
    let remove = Command::new("git")
        .current_dir(&worktree.parent)
        .args(["worktree", "remove", "--force"])
        .arg(&worktree.path)
        .output()
        .map_err(|e| format!("could not run git worktree remove: {e}"))?;
    if !remove.status.success() {
        return Err(format!(
            "git worktree remove refused: {}",
            String::from_utf8_lossy(&remove.stderr).trim()
        ));
    }
    let _ = Command::new("git")
        .current_dir(&worktree.parent)
        .args(["branch", "-D", &worktree.branch])
        .output();
    Ok(format!(
        "removed worktree {} (branch {})",
        worktree.path.display(),
        worktree.branch
    ))
}

/// JSON the tool returns on start.
pub fn start_document(worktree: &ManagedWorktree) -> String {
    serde_json::json!({
        "ok": true,
        "path": worktree.path,
        "branch": worktree.branch,
        "head": worktree.head,
        "parent": worktree.parent,
        "cargo_target_dir": worktree.cargo_target_dir,
        "note": "session cwd is this path; CARGO_TARGET_DIR is outside the disposable tree"
    })
    .to_string()
}

fn git_toplevel(cwd: &Path) -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args([
            "-C",
            &cwd.display().to_string(),
            "rev-parse",
            "--show-toplevel",
        ])
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err("this directory is not a git checkout".to_string());
    }
    Ok(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

fn git_stdout(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} refused: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn fetch_main(repo: &Path) -> Result<(), String> {
    for remote in ["openagents", "origin"] {
        let out = Command::new("git")
            .current_dir(repo)
            .args(["fetch", remote, "main"])
            .output()
            .map_err(|e| format!("could not run git fetch: {e}"))?;
        if out.status.success() {
            return Ok(());
        }
    }
    Err("no remote answered git fetch main".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(repo: &Path, args: &[&str]) {
        let out = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn init_repo(root: &Path) -> PathBuf {
        let repo = root.join("repo");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-b", "main"]);
        git(&repo, &["config", "user.email", "dev@example.com"]);
        git(&repo, &["config", "user.name", "dev"]);
        fs::write(repo.join("README.md"), "hello\n").unwrap();
        git(&repo, &["add", "README.md"]);
        git(&repo, &["commit", "-m", "init"]);
        repo
    }

    #[test]
    fn a_dirty_checkout_is_reported_and_a_clean_one_is_not() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path());
        assert!(!is_dirty(&repo));
        fs::write(repo.join("foreign.txt"), "sibling wip\n").unwrap();
        assert!(is_dirty(&repo));
    }

    #[test]
    fn start_puts_writes_in_the_managed_tree_not_the_dirty_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path());
        fs::write(repo.join("foreign.txt"), "sibling wip\n").unwrap();
        let managed = tmp.path().join("managed");
        let cargo = tmp.path().join("cargo-target");
        let wt = start(&repo, &managed, &cargo).expect("start");
        assert!(wt.path.starts_with(&managed));
        assert!(wt.path.join("README.md").is_file());
        assert!(
            !wt.path.join("foreign.txt").exists(),
            "foreign WIP must not appear in the implement tree"
        );
        fs::write(wt.path.join("landed.rs"), "fn ok() {}\n").unwrap();
        assert!(
            !repo.join("landed.rs").exists(),
            "the first write must not land in the dirty parent"
        );
        let msg = finish(&wt, true).expect("finish");
        assert!(msg.contains("removed"));
        assert!(!wt.path.exists());
    }

    #[test]
    fn finish_without_landing_leaves_the_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = init_repo(tmp.path());
        let wt = start(
            &repo,
            &tmp.path().join("managed"),
            &tmp.path().join("cargo"),
        )
        .unwrap();
        let msg = finish(&wt, false).unwrap();
        assert!(msg.contains("left at"));
        assert!(wt.path.exists());
        let _ = finish(&wt, true);
    }
}
