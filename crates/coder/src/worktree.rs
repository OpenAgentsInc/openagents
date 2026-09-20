//! Serialized Git worktree metadata changes, with concurrent delegate work.
//!
//! Git reads other worktrees' metadata when adding a checkout. Removing a
//! sibling then can make that read fail. The common Git directory itself is
//! the advisory lock shared by Coder processes, including linked checkouts:
//! `flock` on the directory, so the lock leaves no file behind in a
//! workspace a read-only fan-out promised not to write. Other tools must
//! cooperate with this lock to get the same guarantee.
//!
//! The checkout parents under `.coder/worktrees` are created when the first
//! checkout needs them and removed, under the same lock, when the last
//! checkout leaves them empty.

use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use supervise::{Job, Limits};

const GIT_WALL: Duration = Duration::from_secs(30);
const LOCK_WALL: Duration = Duration::from_secs(30);

/// The scratch Git directory seeded inside every checkout, relative to it.
///
/// The common Git directory is sealed against a delegate, so a delegate
/// that commits does so here. The directory starts with one commit of the
/// checkout's tree, so the delegate's own commit diffs against the base
/// rather than holding every file, and a reviewer's
/// `git log -1 --stat` names only what the item changed.
pub const SCRATCH_GIT_DIR: &str = ".coder-git";

/// How long seeding the scratch directory may take: one `add -A` of the
/// whole checkout and one commit.
const SEED_WALL: Duration = Duration::from_secs(120);

/// A detached checkout owned by one delegation.
#[derive(Debug)]
pub struct Worktree {
    repository: PathBuf,
    path: PathBuf,
    active: AtomicBool,
}

impl Worktree {
    /// Creates a checkout without racing sibling creation or cleanup.
    pub(crate) async fn add(repository: &Path) -> Result<Self, String> {
        let repository = repository.to_path_buf();
        // A blocking task owns the entire transaction, including its runtime.
        // If the awaiting caller is cancelled, the transaction still finishes
        // and the returned Worktree is dropped and cleaned up. A cancelled Git
        // add cannot keep changing metadata after we release the lock.
        tokio::task::spawn_blocking(move || {
            let repository = repository
                .canonicalize()
                .map_err(|error| format!("cannot resolve repository: {error}"))?;
            let path = locked(&repository, |repository| {
                let mut parent = repository.to_path_buf();
                for component in Path::new(crate::delegate::WORKTREE_DIR) {
                    parent.push(component);
                    match std::fs::create_dir(&parent) {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(error) => {
                            return Err(format!("cannot create worktree parent: {error}"));
                        }
                    }
                    parent = parent
                        .canonicalize()
                        .map_err(|error| format!("cannot resolve worktree parent: {error}"))?;
                    if !parent.starts_with(repository) {
                        return Err(
                            "worktree parent leaves the repository through a symlink".into()
                        );
                    }
                }
                // Reserve an empty directory atomically. A recycled PID must
                // never cause failed creation to remove a previous run's
                // checkout.
                let path = loop {
                    let path = parent.join(unique_name());
                    match std::fs::create_dir(&path) {
                        Ok(()) => break path,
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                        Err(error) => return Err(format!("cannot reserve worktree: {error}")),
                    }
                };
                if let Err(error) = mutate(repository, &path, true) {
                    let _ = std::fs::remove_dir(&path);
                    prune_parents(repository);
                    return Err(error);
                }
                Ok(path)
            })?;
            let checkout = Self {
                repository,
                path,
                active: AtomicBool::new(true),
            };
            // Outside the metadata lock: the seed touches nothing shared.
            seed_scratch(&checkout.path)?;
            Ok(checkout)
        })
        .await
        .map_err(|error| format!("worktree creation stopped: {error}"))?
    }

    /// The checkout this delegation owns.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Keeps the checkout: it will not be removed when this `Worktree`
    /// drops, however it drops.
    ///
    /// A delegation that may have written owes a reviewer the files the
    /// executor left — answered, failed, timed out, or cancelled — so
    /// retention is set before the executor spawns rather than after it
    /// ends. The worktree stays registered with the repository, so
    /// `git worktree list` names it even when the caller walked away
    /// and no result came back; removing it or merging from it is the
    /// reviewer's decision now.
    pub(crate) fn retain(&self) -> PathBuf {
        self.active.store(false, Ordering::Release);
        self.path.clone()
    }

    /// Removes the checkout before a completed delegation reports.
    pub(crate) async fn close(self) -> Result<(), String> {
        tokio::task::spawn_blocking(move || {
            let worktree = self;
            let result = remove(&worktree.repository, &worktree.path);
            // A failed removal is reported with the retained path. Do not
            // silently retry after reporting that failure to the caller.
            worktree.active.store(false, Ordering::Release);
            result
        })
        .await
        .map_err(|error| format!("worktree cleanup stopped: {error}"))?
    }
}

impl Drop for Worktree {
    fn drop(&mut self) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }
        let repository = self.repository.clone();
        let path = self.path.clone();
        // This fallback also works after the caller's Tokio runtime shuts
        // down. Normal completion awaits close; cancellation has no caller
        // left to await cleanup. Never block a runtime worker on this lock.
        if let Err(error) = std::thread::Builder::new()
            .name("coder-worktree-cleanup".into())
            .spawn(move || {
                if let Err(error) = remove(&repository, &path) {
                    eprintln!("worktree cleanup failed: {error}");
                }
            })
        {
            eprintln!("cannot start worktree cleanup: {error}");
        }
    }
}

/// Removes one checkout under the metadata lock, then the checkout parents
/// when it was the last one.
fn remove(repository: &Path, path: &Path) -> Result<(), String> {
    locked(repository, |repository| {
        let result = mutate(repository, path, false);
        prune_parents(repository);
        result
    })
}

/// Removes `.coder/worktrees` and then `.coder` when each is empty. A
/// parent another checkout still uses is not empty, and stays.
fn prune_parents(repository: &Path) {
    let mut parent = repository.join(crate::delegate::WORKTREE_DIR);
    while parent != repository {
        if std::fs::remove_dir(&parent).is_err() {
            return;
        }
        let Some(next) = parent.parent() else { return };
        parent = next.to_path_buf();
    }
}

/// Runs one metadata transaction while holding the repository's lock. The
/// lock is taken off the caller's async runtime, so cancellation cannot
/// release it while a Git subprocess is still being terminated.
fn locked<T>(
    repository: &Path,
    transaction: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot supervise Git: {error}"))?;
    let common = runtime.block_on(common_directory(repository))?;
    let _held = acquire(&common, LOCK_WALL)?;
    transaction(repository)
}

/// Adds or removes one checkout. Call it with the metadata lock held.
fn mutate(repository: &Path, path: &Path, create: bool) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot supervise Git: {error}"))?;
    runtime.block_on(async {
        let command = if create {
            git(repository)
                .args(["worktree", "add", "--detach"])
                .arg(path)
                .arg("HEAD")
        } else {
            git(repository)
                .args(["worktree", "remove", "--force"])
                .arg(path)
        };
        let ended = command.run().await;
        if !ended.ending.success() {
            return Err(format!(
                "cannot {} worktree {}: {}: {}",
                if create { "create" } else { "remove" },
                path.display(),
                ended.ending,
                ended.stderr.marked().trim()
            ));
        }
        Ok(())
    })
}

/// Seeds the scratch Git directory in a fresh checkout: an empty
/// repository at [`SCRATCH_GIT_DIR`] whose one commit holds the
/// checkout's tree, with `.git` and the scratch directory itself
/// excluded so a delegate's `git add -A` never stages either.
fn seed_scratch(checkout: &Path) -> Result<(), String> {
    let git_dir = checkout.join(SCRATCH_GIT_DIR);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot supervise Git: {error}"))?;
    let base = runtime.block_on(async {
        let ended = git(checkout).args(["rev-parse", "HEAD"]).run().await;
        if !ended.ending.success() {
            return Err(format!(
                "cannot read the checkout's commit: {}: {}",
                ended.ending,
                ended.stderr.marked().trim()
            ));
        }
        Ok(ended.stdout.text.trim().to_string())
    })?;
    let scratch = |args: &[&str]| {
        let mut job = Job::new("git")
            .arg("--git-dir")
            .arg(&git_dir)
            .arg("--work-tree")
            .arg(checkout)
            .bounded(Limits::within(SEED_WALL).keeping(64 * 1024));
        for arg in args {
            job = job.arg(arg);
        }
        job
    };
    runtime.block_on(async {
        for args in [
            &["init", "--quiet"][..],
            &["config", "user.name", "coder"],
            &["config", "user.email", "coder@openagents.com"],
        ] {
            let ended = scratch(args).run().await;
            if !ended.ending.success() {
                return Err(format!(
                    "cannot seed scratch Git directory: git {}: {}: {}",
                    args.join(" "),
                    ended.ending,
                    ended.stderr.marked().trim()
                ));
            }
        }
        let exclude = git_dir.join("info");
        std::fs::create_dir_all(&exclude)
            .map_err(|error| format!("cannot write scratch exclude: {error}"))?;
        std::fs::write(
            exclude.join("exclude"),
            format!("{SCRATCH_GIT_DIR}\n.git\n"),
        )
        .map_err(|error| format!("cannot write scratch exclude: {error}"))?;
        let message = format!("Base {base}");
        for args in [
            &["add", "-A"][..],
            &["commit", "--quiet", "--allow-empty", "-m", &message],
        ] {
            let ended = scratch(args).run().await;
            if !ended.ending.success() {
                return Err(format!(
                    "cannot seed scratch Git directory: git {}: {}: {}",
                    args[0],
                    ended.ending,
                    ended.stderr.marked().trim()
                ));
            }
        }
        Ok(())
    })
}

fn git(repository: &Path) -> Job {
    Job::new("git")
        .arg("-C")
        .arg(repository)
        .bounded(Limits::within(GIT_WALL).keeping(64 * 1024))
}

/// The repository's common Git directory, resolved to a canonical
/// absolute path. For a linked checkout this is the main checkout's
/// `.git`, which is the directory a delegate's boundary seals — writing
/// there from a worktree would corrupt the shared object store and the
/// metadata of every sibling.
pub(crate) async fn common_directory(repository: &Path) -> Result<PathBuf, String> {
    let ended = git(repository)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .run()
        .await;
    if !ended.ending.success() || ended.stdout.truncated {
        return Err(format!(
            "cannot resolve Git common directory: {}: {}",
            ended.ending,
            ended.stderr.marked()
        ));
    }
    let path = PathBuf::from(ended.stdout.text.trim_end_matches('\n'));
    if !path.is_absolute() {
        return Err("Git common directory is not absolute".into());
    }
    path.canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", path.display()))
}

/// Takes an exclusive `flock` on a directory that already exists — the
/// common Git directory — so no lock file is written into the workspace.
fn acquire(path: &Path, wall: Duration) -> Result<File, String> {
    let file = File::open(path)
        .map_err(|error| format!("cannot open worktree lock {}: {error}", path.display()))?;
    let started = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if started.elapsed() < wall => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(format!(
                    "worktree metadata lock timed out: {}",
                    path.display()
                ));
            }
            Err(TryLockError::Error(error)) => {
                return Err(format!("cannot lock worktree metadata: {error}"));
            }
        }
    }
}

fn unique_name() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How long a test waits for a dropped lock to come free. A sibling
    /// test that is between fork and exec holds a copy of every open
    /// descriptor for that moment, and a `flock` lasts until the last
    /// copy closes.
    const RELEASE_WAIT: Duration = Duration::from_secs(5);

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
    async fn a_parent_symlink_cannot_create_a_checkout_outside_the_repository() {
        let repository = repository().await;
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), repository.path().join(".coder")).unwrap();
        let error = Worktree::add(repository.path()).await.unwrap_err();
        assert!(error.contains("symlink"), "{error}");
        assert!(!outside.path().join("worktrees").exists());
    }

    /// A fresh checkout carries a scratch Git directory whose one commit
    /// is the checkout's tree, so a delegate's commit there holds only
    /// what the delegate changed.
    #[tokio::test]
    async fn a_checkout_is_seeded_with_a_scratch_git_directory_at_its_base() {
        let directory = repository().await;
        std::fs::write(directory.path().join("a.txt"), "a\n").unwrap();
        std::fs::write(directory.path().join("b.txt"), "b\n").unwrap();
        for args in [
            vec!["add", "a.txt", "b.txt"],
            vec![
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "two files",
            ],
        ] {
            let result = git(directory.path()).args(args).run().await;
            assert!(result.ending.success(), "{}", result.stderr.text);
        }
        let checkout = Worktree::add(directory.path()).await.unwrap();
        let scratch = |args: &[&str]| {
            Job::new("git")
                .arg("--git-dir")
                .arg(checkout.path().join(SCRATCH_GIT_DIR))
                .arg("--work-tree")
                .arg(checkout.path())
                .args(args.iter().copied())
                .bounded(Limits::within(GIT_WALL).keeping(64 * 1024))
                .run()
        };

        let clean = scratch(&["status", "--porcelain"]).await;
        assert!(clean.ending.success(), "{}", clean.stderr.text);
        assert_eq!(clean.stdout.text, "", "the seed commit covers the tree");
        let base = scratch(&["log", "--format=%s"]).await;
        assert_eq!(base.stdout.text.lines().count(), 1);
        assert!(
            base.stdout.text.starts_with("Base "),
            "{}",
            base.stdout.text
        );

        std::fs::write(checkout.path().join("b.txt"), "changed\n").unwrap();
        for args in [&["add", "-A"][..], &["commit", "--quiet", "-m", "the item"]] {
            let result = scratch(args).await;
            assert!(result.ending.success(), "{}", result.stderr.text);
        }
        let stat = scratch(&["show", "--stat", "--format=", "HEAD"]).await;
        assert!(stat.stdout.text.contains("b.txt"), "{}", stat.stdout.text);
        assert!(!stat.stdout.text.contains("a.txt"), "{}", stat.stdout.text);
        assert!(
            !stat.stdout.text.contains(".git"),
            "neither Git directory is staged: {}",
            stat.stdout.text
        );
        checkout.close().await.unwrap();
    }

    #[tokio::test]
    async fn linked_checkouts_share_the_same_metadata_lock() {
        let directory = repository().await;
        let linked = Worktree::add(directory.path()).await.unwrap();
        let common = common_directory(directory.path()).await.unwrap();
        assert_eq!(common_directory(linked.path()).await.unwrap(), common);
        let held = acquire(&common, Duration::ZERO).unwrap();
        let linked_common = common_directory(linked.path()).await.unwrap();
        assert!(acquire(&linked_common, Duration::ZERO).is_err());
        drop(held);
        linked.close().await.unwrap();
    }

    #[tokio::test]
    async fn simultaneous_additions_and_removals_keep_all_answers() {
        let directory = repository().await;
        // Some jobs finish while later jobs are still creating checkouts.
        // The lock covers metadata changes, not the sleep standing in for
        // executor work. Every created checkout must survive until its owner
        // ends it, and cleanup must finish before this test reports.
        let mut jobs = tokio::task::JoinSet::new();
        for n in 0..18 {
            let root = directory.path().to_path_buf();
            jobs.spawn(async move {
                let checkout = Worktree::add(&root).await.unwrap();
                tokio::time::sleep(Duration::from_millis((n % 3) * 20)).await;
                assert!(checkout.path().join(".git").is_file());
                checkout.close().await.unwrap();
            });
        }
        let mut completed = 0;
        while let Some(result) = jobs.join_next().await {
            result.unwrap();
            completed += 1;
        }
        assert_eq!(completed, 18);
        // The last checkout out takes the empty parents with it, so a
        // read-only fan-out leaves the workspace as it found it.
        assert!(!directory.path().join(".coder").exists());
        assert!(
            !directory
                .path()
                .join(".git")
                .join("coder-worktrees.lock")
                .exists()
        );
        let listed = git(directory.path())
            .args(["worktree", "list", "--porcelain"])
            .run()
            .await;
        assert!(listed.ending.success());
        assert_eq!(
            listed
                .stdout
                .text
                .lines()
                .filter(|line| line.starts_with("worktree "))
                .count(),
            1
        );
    }

    #[test]
    fn a_contended_metadata_lock_times_out_and_then_recovers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().to_path_buf();
        let held = acquire(&path, Duration::ZERO).unwrap();
        let error = acquire(&path, Duration::from_millis(30)).unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        drop(held);
        let _next = acquire(&path, RELEASE_WAIT).unwrap();
    }

    #[test]
    fn a_different_process_cannot_take_the_metadata_lock() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().to_path_buf();
        let held = acquire(&path, Duration::ZERO).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "worktree::tests::child_attempts_the_metadata_lock",
                "--nocapture",
            ])
            .env("CODER_TEST_WORKTREE_LOCK", &path)
            .status()
            .unwrap();
        assert!(status.success());
        drop(held);
        let _next = acquire(&path, RELEASE_WAIT).unwrap();
    }

    #[test]
    fn child_attempts_the_metadata_lock() {
        let Some(path) = std::env::var_os("CODER_TEST_WORKTREE_LOCK") else {
            return;
        };
        let error = acquire(Path::new(&path), Duration::ZERO).unwrap_err();
        assert!(error.contains("timed out"), "{error}");
    }
}
