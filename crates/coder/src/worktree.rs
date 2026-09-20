//! Serialized Git worktree metadata changes, with concurrent delegate work.
//!
//! Git reads other worktrees' metadata when adding a checkout. Removing a
//! sibling then can make that read fail. The common Git directory holds an
//! advisory lock shared by Coder processes, including linked checkouts. Other
//! tools must cooperate with this lock to get the same guarantee.

use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use supervise::{Job, Limits};

const GIT_WALL: Duration = Duration::from_secs(30);
const LOCK_WALL: Duration = Duration::from_secs(30);
const LOCK_NAME: &str = "coder-worktrees.lock";

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
            let mut parent = repository.clone();
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
                if !parent.starts_with(&repository) {
                    return Err("worktree parent leaves the repository through a symlink".into());
                }
            }
            // Reserve an empty directory atomically. A recycled PID must
            // never cause failed creation to remove a previous run's checkout.
            let path = loop {
                let path = parent.join(unique_name());
                match std::fs::create_dir(&path) {
                    Ok(()) => break path,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(format!("cannot reserve worktree: {error}")),
                }
            };
            let worktree = Self {
                repository,
                path,
                active: AtomicBool::new(true),
            };
            mutate(&worktree.repository, &worktree.path, true)?;
            Ok(worktree)
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
            let result = mutate(&worktree.repository, &worktree.path, false);
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
                if let Err(error) = mutate(&repository, &path, false) {
                    eprintln!("worktree cleanup failed: {error}");
                }
            })
        {
            eprintln!("cannot start worktree cleanup: {error}");
        }
    }
}

/// Runs off the caller's async runtime, so cancellation cannot release the
/// metadata lock while the Git subprocess is still being terminated.
fn mutate(repository: &Path, path: &Path, create: bool) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot supervise Git: {error}"))?;
    runtime.block_on(async {
        let common = common_directory(repository).await?;
        let _held = acquire(&common.join(LOCK_NAME), LOCK_WALL)?;
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

fn acquire(path: &Path, wall: Duration) -> Result<File, String> {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
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

    #[tokio::test]
    async fn linked_checkouts_share_the_same_metadata_lock() {
        let directory = repository().await;
        let linked = Worktree::add(directory.path()).await.unwrap();
        let common = common_directory(directory.path()).await.unwrap();
        assert_eq!(common_directory(linked.path()).await.unwrap(), common);
        let held = acquire(&common.join(LOCK_NAME), Duration::ZERO).unwrap();
        let linked_common = common_directory(linked.path()).await.unwrap();
        assert!(acquire(&linked_common.join(LOCK_NAME), Duration::ZERO).is_err());
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
        assert_eq!(
            std::fs::read_dir(directory.path().join(crate::delegate::WORKTREE_DIR))
                .unwrap()
                .count(),
            0
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
        let path = directory.path().join(LOCK_NAME);
        let held = acquire(&path, Duration::ZERO).unwrap();
        let error = acquire(&path, Duration::from_millis(30)).unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        drop(held);
        let _next = acquire(&path, RELEASE_WAIT).unwrap();
    }

    #[test]
    fn a_different_process_cannot_take_the_metadata_lock() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(LOCK_NAME);
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
