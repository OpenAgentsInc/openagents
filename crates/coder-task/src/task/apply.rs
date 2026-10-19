//! Bringing a local task's change into the checkout it came from
//! (`openagents chat apply`, #10343).
//!
//! A local run works in a detached worktree of its own ([`super::local`]),
//! so its change is not in the checkout the person ran the command from.
//! [`apply`] carries it over: the difference between the task's base and
//! everything in the worktree now (committed, staged, unstaged, and new
//! files), applied to the checkout as uncommitted changes with a three-way
//! merge. It refuses a checkout with changes of its own, so nothing the
//! person has there is mixed in, and never commits or pushes.

use std::path::{Path, PathBuf};

use super::local;

/// What [`apply`] carried over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    /// The checkout the change now sits in, uncommitted.
    pub checkout: PathBuf,
    /// The files it changes, relative to the checkout.
    pub files: Vec<String>,
}

/// Apply `task`'s change to the checkout it was made from.
///
/// # Errors
/// The task has no local worktree here, changed nothing, the checkout has
/// changes of its own, or Git cannot apply the change.
pub fn apply(store: &Path, task: &str) -> Result<Applied, String> {
    // A removed worktree comes back at its commit first (#10291).
    let record = super::retire::ensure(store, task)?
        .ok_or_else(|| "This task has no worktree on this computer to apply.".to_owned())?;
    let worktree = PathBuf::from(&record.worktree);
    let checkout = PathBuf::from(&record.checkout);
    if worktree == checkout {
        return Err("This task worked in the checkout itself; its change is already there.".into());
    }
    let scratch = store.join("apply");
    crate::private::create_dir_all(&scratch)
        .map_err(|_| format!("cannot create {}", scratch.display()))?;
    let index = scratch.join(format!("{task}.index"));
    let patch = change(&worktree, &record.base, &index);
    let _ = std::fs::remove_file(&index);
    let patch = patch?;
    if patch.trim().is_empty() {
        return Err("This task changed nothing to apply.".into());
    }
    let own = local::git_out(
        &checkout,
        &["status", "--porcelain", "--untracked-files=no"],
    )?;
    if !own.trim().is_empty() {
        return Err(format!(
            "{} has changes of its own. Commit or put them aside, then apply again.",
            checkout.display()
        ));
    }
    let file = scratch.join(format!("{task}.patch"));
    std::fs::write(&file, &patch).map_err(|error| error.to_string())?;
    let path = file.display().to_string();
    let applied = local::git_out(
        &checkout,
        &["apply", "--3way", "--whitespace=nowarn", &path],
    );
    let _ = std::fs::remove_file(&file);
    applied.map_err(|why| format!("Git could not apply the change here: {why}"))?;
    // A three-way apply stages what it applied, new files included.
    let files = local::git_out(&checkout, &["diff", "--cached", "--name-only", "HEAD"])
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    Ok(Applied { checkout, files })
}

/// The worktree's whole change from `base`, new files included, as a
/// binary patch, read through a private index so the worktree's own index
/// is left as it was.
fn change(worktree: &Path, base: &str, index: &Path) -> Result<String, String> {
    let with_index = |args: &[&str]| -> Result<String, String> {
        let output = local::git()
            .arg("-C")
            .arg(coder_boundary::plain_path(worktree))
            .env("GIT_INDEX_FILE", index)
            .args(args)
            .output()
            .map_err(|_| "cannot run git".to_owned())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    };
    with_index(&["read-tree", "HEAD"])?;
    with_index(&["add", "-A"])?;
    with_index(&["diff", "--cached", "--binary", base])
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
