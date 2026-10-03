//! A chat run's base, freshened before its worktree is made (#10298).
//!
//! A chat run starts from the checkout's `HEAD`. When `HEAD` is a branch
//! that tracks a remote branch, that one branch is fetched first, bounded by
//! a short timeout, into its own remote-tracking ref. The fetch names its
//! refspec on the command line, so nothing is added to `remote.<name>.fetch`
//! (a refspec left there breaks later fetches once the branch is deleted on
//! the remote; Paseo's diagnose-5556). It takes the repository's fetch lock
//! like the landing does (#10233), and gives up when the lock or the remote
//! takes too long: the cached ref is still usable.
//!
//! The run then starts at the fetched tip only when the local branch is
//! behind it with nothing of its own; a branch with local commits, or one
//! that has moved past the remote, keeps its `HEAD`, so a run never drops
//! the person's unpushed work.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use super::{landing, local};

/// How long the fetch, lock included, may take before the cached ref is used.
pub const TIMEOUT: Duration = Duration::from_secs(8);

/// What freshening found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fresh {
    /// `HEAD` is detached or tracks no remote branch: nothing was fetched.
    Untracked,
    /// The tracked branch was fetched (or the fetch failed and the cached
    /// ref stands); the run starts at `base`.
    Tracked {
        /// The remote-tracking ref, such as `refs/remotes/origin/main`.
        upstream: String,
        /// Whether the fetch finished in time.
        fetched: bool,
        /// The commit the run starts at.
        base: String,
    },
}

/// Freshen the branch `HEAD` tracks in the checkout at `top`, whose `HEAD`
/// is the commit `head`, waiting at most `timeout`.
#[must_use]
pub fn base(top: &Path, head: &str, timeout: Duration) -> Fresh {
    let Some((remote, merge, upstream)) = tracked(top) else {
        return Fresh::Untracked;
    };
    let fetched = fetch(top, &remote, &merge, &upstream, timeout);
    let tip = local::git_out(
        top,
        &["rev-parse", "--verify", &format!("{upstream}^{{commit}}")],
    )
    .map(|out| out.trim().to_owned())
    .unwrap_or_default();
    let behind = !tip.is_empty()
        && tip != head
        && local::git_out(top, &["merge-base", "--is-ancestor", head, &tip]).is_ok();
    Fresh::Tracked {
        upstream,
        fetched,
        base: if behind { tip } else { head.to_owned() },
    }
}

/// `HEAD`'s branch's remote, the remote branch it merges, and its
/// remote-tracking ref; `None` when `HEAD` is detached or tracks nothing on
/// a remote.
fn tracked(top: &Path) -> Option<(String, String, String)> {
    let read = |args: &[&str]| {
        local::git_out(top, args)
            .ok()
            .map(|out| out.trim().to_owned())
            .filter(|out| !out.is_empty())
    };
    let branch = read(&["symbolic-ref", "-q", "--short", "HEAD"])?;
    let remote = read(&["config", "--get", &format!("branch.{branch}.remote")])?;
    // `.` is the local repository: a branch tracking a local branch.
    if remote == "." {
        return None;
    }
    let merge = read(&["config", "--get", &format!("branch.{branch}.merge")])?;
    merge.strip_prefix("refs/heads/")?;
    let upstream = read(&["rev-parse", "--symbolic-full-name", "@{upstream}"])?;
    upstream
        .starts_with(&format!("refs/remotes/{remote}/"))
        .then_some((remote, merge, upstream))
}

/// Fetch `merge` from `remote` into `upstream` under the repository's fetch
/// lock; `false` when the lock, the remote, or Git took longer than
/// `timeout` or refused.
fn fetch(top: &Path, remote: &str, merge: &str, upstream: &str, timeout: Duration) -> bool {
    let started = Instant::now();
    let Ok(lock) = landing::fetch_lock(top) else {
        return false;
    };
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return false,
        }
    }
    let ran = run(
        top,
        &[
            "fetch",
            "-q",
            "--no-tags",
            remote,
            &format!("+{merge}:{upstream}"),
        ],
        timeout.saturating_sub(started.elapsed()),
    );
    // Unlock explicitly so a child forked meanwhile cannot keep the lock.
    let _ = lock.unlock();
    ran
}

/// Run Git in `top` with `args`, never prompting, killed after `timeout`.
fn run(top: &Path, args: &[&str], timeout: Duration) -> bool {
    let child = local::git()
        .arg("-C")
        .arg(coder_boundary::plain_path(top))
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return false;
    };
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(test)]
#[path = "freshen_tests.rs"]
mod tests;
