//! A checkout on its own new branch, for a background agent that commits
//! and pushes like a person would (#11163).
//!
//! It lives under `.coder/worktrees/NAME`, is created with `git worktree
//! add -b BRANCH` under the same metadata lock as a delegation's detached
//! checkout ([`crate::worktree`]), and `/.coder/` is added to the
//! repository's `info/exclude` so the parent checkout's status stays clean.
//! Nothing removes it on drop: [`remove`] does, and only when its caller
//! knows nobody works in it any more.

use std::path::{Path, PathBuf};

use crate::worktree::{common_directory, git, locked, prune_parents};

/// One branch checkout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchCheckout {
    /// The repository's top-level directory.
    pub repository: PathBuf,
    /// The new checkout.
    pub path: PathBuf,
    /// The new branch.
    pub branch: String,
    /// The commit both started from.
    pub base: String,
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot supervise Git: {error}"))
}

/// Creates `.coder/worktrees/NAME` on a new `branch` from `HEAD`.
///
/// # Errors
/// `repository` is not a Git checkout, `name` or `branch` is unusable, or
/// Git refuses (for example, the branch already exists).
pub async fn add(repository: &Path, name: &str, branch: &str) -> Result<BranchCheckout, String> {
    if name.is_empty()
        || name.len() > 96
        || name.starts_with('.')
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(format!("`{name}` is not a usable checkout name"));
    }
    if branch.is_empty() || branch.starts_with('-') || branch.contains("..") || branch.len() > 200 {
        return Err(format!("`{branch}` is not a usable branch name"));
    }
    let top = git(repository)
        .args(["rev-parse", "--show-toplevel"])
        .run()
        .await;
    if !top.ending.success() {
        return Err("This folder is not inside a Git repository.".into());
    }
    let top = PathBuf::from(top.stdout.text.trim());
    let head = git(&top).args(["rev-parse", "HEAD"]).run().await;
    if !head.ending.success() {
        return Err("The repository has no commit to start from yet.".into());
    }
    let base = head.stdout.text.trim().to_owned();
    let (name, branch) = (name.to_owned(), branch.to_owned());
    tokio::task::spawn_blocking(move || {
        let top = top
            .canonicalize()
            .map_err(|error| format!("cannot resolve repository: {error}"))?;
        locked(&top, |repository| {
            let mut parent = repository.to_path_buf();
            for component in Path::new(crate::delegate::WORKTREE_DIR) {
                parent.push(component);
                match std::fs::create_dir(&parent) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(format!("cannot create worktree parent: {error}")),
                }
                parent = parent
                    .canonicalize()
                    .map_err(|error| format!("cannot resolve worktree parent: {error}"))?;
                if !parent.starts_with(repository) {
                    return Err("worktree parent leaves the repository through a symlink".into());
                }
            }
            exclude_coder_dir(repository);
            let path = parent.join(&name);
            if path.exists() {
                return Err(format!("{} already exists", path.display()));
            }
            let ended = runtime()?.block_on(
                git(repository)
                    .args(["worktree", "add", "--quiet", "-b"])
                    .arg(&branch)
                    .arg(&path)
                    .arg(&base)
                    .run(),
            );
            if !ended.ending.success() {
                prune_parents(repository);
                return Err(format!(
                    "cannot create worktree {}: {}",
                    path.display(),
                    ended.stderr.marked().trim()
                ));
            }
            Ok(BranchCheckout {
                repository: repository.to_path_buf(),
                path,
                branch: branch.clone(),
                base: base.clone(),
            })
        })
    })
    .await
    .map_err(|error| format!("worktree creation stopped: {error}"))?
}

/// Whether the checkout holds work: uncommitted changes, or commits past
/// the commit it started from.
///
/// # Errors
/// Git cannot read the checkout.
pub async fn changed(checkout: &BranchCheckout) -> Result<bool, String> {
    let status = git(&checkout.path)
        .args(["status", "--porcelain"])
        .run()
        .await;
    if !status.ending.success() {
        return Err(format!(
            "cannot read the checkout: {}",
            status.stderr.marked().trim()
        ));
    }
    if !status.stdout.text.trim().is_empty() {
        return Ok(true);
    }
    let head = git(&checkout.path).args(["rev-parse", "HEAD"]).run().await;
    if !head.ending.success() {
        return Err("cannot read the checkout's commit".into());
    }
    Ok(head.stdout.text.trim() != checkout.base)
}

/// Removes a branch checkout and, when `delete_branch`, its branch.
///
/// # Errors
/// Git refuses.
pub async fn remove(checkout: &BranchCheckout, delete_branch: bool) -> Result<(), String> {
    let checkout = checkout.clone();
    tokio::task::spawn_blocking(move || {
        crate::worktree::remove(&checkout.repository, &checkout.path)?;
        if delete_branch {
            let ended = runtime()?.block_on(
                git(&checkout.repository)
                    .args(["branch", "-D", "--quiet"])
                    .arg(&checkout.branch)
                    .run(),
            );
            if !ended.ending.success() {
                return Err(format!(
                    "cannot delete branch {}: {}",
                    checkout.branch,
                    ended.stderr.marked().trim()
                ));
            }
        }
        Ok(())
    })
    .await
    .map_err(|error| format!("worktree cleanup stopped: {error}"))?
}

/// Adds `/.coder/` to the repository's `info/exclude` once, so checkouts
/// under it never show in the parent's status. Best effort.
fn exclude_coder_dir(repository: &Path) {
    let Ok(runtime) = runtime() else { return };
    let Ok(common) = runtime.block_on(common_directory(repository)) else {
        return;
    };
    let info = common.join("info");
    let file = info.join("exclude");
    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    if existing
        .lines()
        .any(|line| matches!(line.trim(), "/.coder/" | ".coder/" | "/.coder" | ".coder"))
    {
        return;
    }
    let _ = std::fs::create_dir_all(&info);
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("/.coder/\n");
    let _ = std::fs::write(&file, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repository() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        for args in [
            vec!["init", "--quiet"],
            vec![
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "--allow-empty",
                "--quiet",
                "-m",
                "a checkout",
            ],
        ] {
            let result = git(directory.path()).args(args).run().await;
            assert!(result.ending.success(), "{}", result.stderr.text);
        }
        directory
    }

    #[tokio::test]
    async fn checkouts_are_isolated_report_changes_and_remove_cleanly() {
        let repository = repository().await;
        let one = add(repository.path(), "agent-one", "agent/one")
            .await
            .unwrap();
        let two = add(repository.path(), "agent-two", "agent/two")
            .await
            .unwrap();
        assert_ne!(one.path, two.path);
        assert!(!changed(&one).await.unwrap());
        std::fs::write(one.path.join("only-one.txt"), "x\n").unwrap();
        assert!(changed(&one).await.unwrap());
        assert!(!two.path.join("only-one.txt").exists());
        let status = git(repository.path())
            .args(["status", "--porcelain"])
            .run()
            .await;
        assert_eq!(status.stdout.text, "", "the parent checkout stays clean");
        let branch = git(&two.path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .run()
            .await;
        assert_eq!(branch.stdout.text.trim(), "agent/two");
        assert!(
            add(repository.path(), "agent-one", "agent/three")
                .await
                .is_err()
        );
        remove(&two, true).await.unwrap();
        assert!(!two.path.exists());
        let branches = git(repository.path()).args(["branch"]).run().await;
        assert!(!branches.stdout.text.contains("agent/two"));
        assert!(branches.stdout.text.contains("agent/one"));
    }

    #[tokio::test]
    async fn a_folder_outside_git_is_refused_in_plain_words() {
        let folder = tempfile::tempdir().unwrap();
        let error = add(folder.path(), "agent-x", "agent/x").await.unwrap_err();
        assert!(error.contains("not inside a Git repository"), "{error}");
    }
}
