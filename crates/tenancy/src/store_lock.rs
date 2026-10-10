//! The writer lock the file-backed stores share.
//!
//! A store's writers serialize on a persistent, owner-private lock file
//! beside the store. The lock is the OS advisory lock on that file
//! (`File::try_lock`), not the file's existence, so the kernel releases it
//! when the holder exits for any reason — a crash, SIGKILL, or OOM kill
//! never leaves the store unwritable. The file itself is never removed:
//! removing a locked file would let a second writer create and lock a new
//! inode beside the first.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::private_fs;

/// The wait each attempt is worth: `attempts` of these bound how long a
/// writer waits for the holder.
const ATTEMPT: Duration = Duration::from_millis(10);
/// How often a waiting writer looks again. Short, so a waiter catches the
/// brief gap between one mutation releasing the lock and the next taking
/// it instead of starving behind a busy writer.
const POLL: Duration = Duration::from_millis(1);

/// Why a writer did not get the lock.
#[derive(Debug)]
pub(crate) enum LockFailure {
    /// A live writer still held it after every attempt.
    Held(PathBuf),
    /// The filesystem refused.
    Io(std::io::Error),
    /// The lock file is not an owner-private regular file, or it was
    /// replaced while the lock was being taken.
    Invalid(&'static str),
}

impl From<std::io::Error> for LockFailure {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<LockFailure> for crate::accounts::Trouble {
    fn from(failure: LockFailure) -> Self {
        match failure {
            LockFailure::Held(path) => Self::Locked(path.display().to_string()),
            LockFailure::Io(error) => Self::Io(error),
            LockFailure::Invalid(message) => Self::Invalid(message.into()),
        }
    }
}

/// A held store lock; dropping it (or the process dying) releases it.
#[derive(Debug)]
pub(crate) struct StoreLock {
    path: PathBuf,
    file: File,
}

impl StoreLock {
    /// Take the lock at `path`, waiting up to `attempts` times 10 ms for a
    /// live holder (one try when `attempts` is 0 or 1).
    pub(crate) fn acquire(path: &Path, attempts: u32) -> Result<Self, LockFailure> {
        let mut options = std::fs::OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        private_fs::mode(&mut options, 0o600)?;
        let file = private_fs::flags(&mut options, private_fs::O_NOFOLLOW | private_fs::O_CLOEXEC)?
            .open(path)?;
        let held = file.metadata()?;
        if !private(&held) {
            return Err(LockFailure::Invalid(
                "a store lock requires an owned private regular file",
            ));
        }
        let deadline = std::time::Instant::now() + ATTEMPT * attempts.saturating_sub(1);
        loop {
            match file.try_lock() {
                Ok(()) => {
                    let current = std::fs::symlink_metadata(path)?;
                    if !private(&current) || !private_fs::same_file(&current, &held) {
                        return Err(LockFailure::Invalid("a store lock changed while taking it"));
                    }
                    return Ok(Self {
                        path: path.to_path_buf(),
                        file,
                    });
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    if std::time::Instant::now() >= deadline {
                        return Err(LockFailure::Held(path.to_path_buf()));
                    }
                    std::thread::sleep(POLL);
                }
                Err(std::fs::TryLockError::Error(error)) => return Err(LockFailure::Io(error)),
            }
        }
    }

    /// The lock file's path.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// The open, locked file.
    pub(crate) fn file(&self) -> &File {
        &self.file
    }
}

fn private(meta: &std::fs::Metadata) -> bool {
    meta.is_file()
        && private_fs::nlink(meta) == 1
        && private_fs::owned(meta)
        && private_fs::mode_clear(meta, 0o077)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// The environment variable that turns the helper test below into the
    /// crashing lock holder.
    const HOLDER: &str = "TENANCY_STORE_LOCK_HOLDER";

    #[test]
    fn a_second_writer_waits_then_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.lock");
        let first = StoreLock::acquire(&path, 1).unwrap();
        assert!(matches!(
            StoreLock::acquire(&path, 3),
            Err(LockFailure::Held(_))
        ));
        drop(first);
        StoreLock::acquire(&path, 1).unwrap();
        assert!(path.exists(), "the lock file persists across holders");
    }

    #[test]
    fn a_leftover_lock_file_from_a_dead_writer_is_reclaimed() {
        // The old `create_new` locks left `pid N` files behind on a crash.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.lock");
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        private_fs::mode(&mut options, 0o600).unwrap();
        std::io::Write::write_all(&mut options.open(&path).unwrap(), b"pid 1\n").unwrap();
        StoreLock::acquire(&path, 1).unwrap();
    }

    #[test]
    fn a_symlinked_lock_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("elsewhere");
        std::fs::write(&target, b"").unwrap();
        let path = dir.path().join("store.lock");
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(StoreLock::acquire(&path, 1).is_err());
    }

    /// Run as a child process by the crash test: take the lock, signal
    /// readiness, and hang until killed.
    #[test]
    fn crash_holder_child() {
        let Some(path) = std::env::var_os(HOLDER) else {
            return;
        };
        let path = PathBuf::from(path);
        let _lock = StoreLock::acquire(&path, 1).unwrap();
        std::fs::write(path.with_extension("ready"), b"").unwrap();
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    }

    #[test]
    fn a_writer_killed_while_holding_the_lock_does_not_block_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("store.lock");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "store_lock::tests::crash_holder_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(HOLDER, &path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let ready = path.with_extension("ready");
        let started = std::time::Instant::now();
        while !ready.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the child never took the lock"
            );
            if let Some(status) = child.try_wait().unwrap() {
                panic!("the child exited early: {status}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(matches!(
            StoreLock::acquire(&path, 2),
            Err(LockFailure::Held(_))
        ));
        // SIGKILL: no destructor runs, exactly like a crash or OOM kill.
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(path.exists(), "the dead holder left its lock file behind");
        StoreLock::acquire(&path, 200).expect("the killed holder's lock is reclaimed");
    }
}
