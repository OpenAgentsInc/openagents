//! A spare worktree per project, so a Coder start does not wait on Git
//! (#10115).
//!
//! A fresh `git worktree add` of a large repository writes every tracked
//! file: 7.6 s for this monorepo's 26,813 files. A start instead takes the
//! project's spare, a detached worktree of a recent commit prepared in the
//! background, and moves it to the exact commit the task needs with a
//! checkout that touches only the files that differ.
//!
//! - **One spare per project**, at `<worktrees>/<project>.spare-<key>`,
//!   `key` naming the checkout's top level. It is prepared at a temporary
//!   path and moved into place only when complete, under a lock file, so a
//!   start never takes a half-written spare.
//! - **Taken by a rename**: the first start to rename it owns it; Git's
//!   record of the worktree is repaired to the new path.
//! - **Never dirty**: a spare with any tracked change, untracked file, or
//!   ignored file is discarded, never handed to a task. A new spare is
//!   checked with a full `git status` and sealed: each of its folders'
//!   modification times is recorded beside it, so a start checks it by
//!   those times (any file or folder added or removed moves its parent's)
//!   and Git's tracked files, in a fraction of a full status.
//! - **Falls back**: no spare, a spare that cannot move to the commit, or
//!   any Git failure, and the start makes its worktree as before.
//! - **Observed ahead**: once its files are settled, the spare is
//!   observed into the store's snapshot digests, so the engine's first
//!   observation of the task's workspace reads only the files the start's
//!   checkout changed, not the whole tree.
//! - **Swept**: a spare whose repository is gone, and a preparation whose
//!   lock is older than [`STALE`], are removed.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::local::git_out;

/// A preparation lock older than this, or naming a process that is gone,
/// is from a preparation that died.
pub const STALE: Duration = Duration::from_secs(15 * 60);
/// How long a start waits for a spare another start is making before it
/// makes its own worktree.
const WAIT: Duration = Duration::from_secs(20);
/// How long a spare's files rest before it is observed: a digest is kept
/// only for a file whose times are settled (two seconds old).
const SETTLE: Duration = Duration::from_millis(2500);
/// How long after a start its replacement spare waits, so making it does
/// not compete with the engine's first steps for the disk.
pub const AFTER_START: Duration = Duration::from_secs(3);

/// Where the spares for the checkout at `top`, named `name`, live.
fn key(top: &Path) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(top.display().to_string().as_bytes())
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The ready spare's path for the checkout at `top`.
#[must_use]
pub fn path(worktrees: &Path, top: &Path, name: &str) -> PathBuf {
    worktrees.join(format!("{name}.spare-{}", key(top)))
}

fn lock_path(worktrees: &Path, top: &Path, name: &str) -> PathBuf {
    worktrees.join(format!(".{name}.spare-{}.lock", key(top)))
}

fn seal_path(worktrees: &Path, top: &Path, name: &str) -> PathBuf {
    worktrees.join(format!(".{name}.spare-{}.seal", key(top)))
}

fn tmp_prefix(top: &Path, name: &str) -> String {
    format!(".{name}.spare-{}.tmp-", key(top))
}

fn plain(path: &Path) -> String {
    coder_boundary::plain_path(path).display().to_string()
}

/// Take the spare for `top` as `target`, at `commit`. `None` when there is
/// no spare, or it could not be made clean at `commit`; the caller then
/// makes a worktree as before.
#[must_use]
pub fn take(
    worktrees: &Path,
    top: &Path,
    name: &str,
    commit: &str,
    target: &Path,
) -> Option<PathBuf> {
    let spare = path(worktrees, top, name);
    // A spare being made now is ready sooner than a fresh worktree would
    // be, and making both at once slows each.
    let lock = lock_path(worktrees, top, name);
    let waiting = std::time::Instant::now();
    while !spare.exists() && held(&lock) && waiting.elapsed() < WAIT {
        std::thread::sleep(Duration::from_millis(50));
    }
    // The rename is the claim: only one start can make it.
    std::fs::rename(&spare, target).ok()?;
    let seal = seal_path(worktrees, top, name);
    let sealed = std::fs::read(&seal).ok();
    let _ = std::fs::remove_file(&seal);
    let ready = (|| {
        git_out(top, &["worktree", "repair", &plain(target)]).ok()?;
        // A sealed spare is checked by its folders' times and Git's
        // tracked files, a tenth of a full status; an unsealed one by the
        // full status.
        let unchanged = match sealed {
            Some(bytes) => intact(target, &bytes) && tracked_clean(target),
            None => clean(target),
        };
        if !unchanged {
            return None;
        }
        let head = git_out(target, &["rev-parse", "HEAD"]).ok()?;
        if head.trim() != commit {
            git_out(target, &["checkout", "--detach", "--quiet", commit]).ok()?;
            let head = git_out(target, &["rev-parse", "HEAD"]).ok()?;
            if head.trim() != commit {
                return None;
            }
        }
        target.canonicalize().ok()
    })();
    if ready.is_none() {
        discard(top, target);
    }
    ready
}

/// Whether the worktree at `dir` has no change at all: nothing modified,
/// staged, untracked, or ignored. A folder of untracked or ignored files
/// shows as the folder, which is as dirty and half as slow to find.
fn clean(dir: &Path) -> bool {
    git_out(
        dir,
        &[
            "status",
            "--porcelain",
            "--untracked-files=normal",
            "--ignored=traditional",
        ],
    )
    .is_ok_and(|out| out.trim().is_empty())
}

/// Whether no tracked file in the worktree at `dir` changed, staged or not.
fn tracked_clean(dir: &Path) -> bool {
    git_out(dir, &["status", "--porcelain", "--untracked-files=no"])
        .is_ok_and(|out| out.trim().is_empty())
}

/// Every folder in the worktree at `dir`, relative, with its modification
/// time: what a file or folder added, removed, or renamed anywhere in it
/// moves. Git's own folder (a file in a linked worktree) is not one.
fn folders(dir: &Path) -> Option<Vec<(String, i64, i64)>> {
    fn time(meta: &std::fs::Metadata) -> Option<(i64, i64)> {
        let at = meta
            .modified()
            .ok()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?;
        Some((
            i64::try_from(at.as_secs()).ok()?,
            i64::from(at.subsec_nanos()),
        ))
    }
    let mut out = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        let here = dir.join(&relative);
        let (secs, nanos) = time(&std::fs::symlink_metadata(&here).ok()?)?;
        out.push((relative.to_str()?.to_owned(), secs, nanos));
        for entry in std::fs::read_dir(&here).ok()? {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                pending.push(relative.join(entry.file_name()));
            }
        }
    }
    Some(out)
}

/// Record the clean spare at `dir`'s folders in `seal`.
fn seal_up(dir: &Path, seal: &Path) -> bool {
    folders(dir)
        .and_then(|folders| serde_json::to_vec(&folders).ok())
        .is_some_and(|bytes| std::fs::write(seal, bytes).is_ok())
}

/// Whether every folder the seal `bytes` names is still there with the
/// same modification time under `dir`. A new folder moves its parent's.
fn intact(dir: &Path, bytes: &[u8]) -> bool {
    let Ok(folders) = serde_json::from_slice::<Vec<(String, i64, i64)>>(bytes) else {
        return false;
    };
    !folders.is_empty()
        && folders.iter().all(|(relative, secs, nanos)| {
            std::fs::symlink_metadata(dir.join(relative))
                .ok()
                .filter(std::fs::Metadata::is_dir)
                .and_then(|meta| meta.modified().ok())
                .and_then(|at| at.duration_since(SystemTime::UNIX_EPOCH).ok())
                .is_some_and(|at| {
                    i64::try_from(at.as_secs()).ok() == Some(*secs)
                        && i64::from(at.subsec_nanos()) == *nanos
                })
        })
}

/// Remove the worktree at `dir` of the repository at `top`.
pub fn discard(top: &Path, dir: &Path) {
    let _ = git_out(top, &["worktree", "remove", "--force", &plain(dir)]);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
        let _ = git_out(top, &["worktree", "prune"]);
    }
}

/// Make the spare for `top` at `commit`, unless one is ready or being
/// made, and, with `digests`, observe it into that snapshot digest file
/// ([`super::adapter::SNAPSHOT_DIGESTS`]). Blocks for as long as Git and
/// the observation take.
///
/// # Errors
/// Why Git could not make it.
pub fn prepare(
    worktrees: &Path,
    top: &Path,
    name: &str,
    commit: &str,
    digests: Option<&Path>,
) -> Result<(), String> {
    if !top.exists() {
        return Err(format!("{} is gone", top.display()));
    }
    crate::private::create_dir_all(worktrees)
        .map_err(|_| format!("cannot create {}", worktrees.display()))?;
    sweep(worktrees, top, name);
    let spare = path(worktrees, top, name);
    if spare.exists() {
        return Ok(());
    }
    let lock = lock_path(worktrees, top, name);
    let Ok(mut file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
    else {
        // Another start is making it.
        return Ok(());
    };
    {
        use std::io::Write as _;
        let _ = write!(file, "{}", std::process::id());
    }
    let seal = seal_path(worktrees, top, name);
    let nonce: [u8; 6] = secp256k1::rand::random();
    let tmp = worktrees.join(format!(
        "{}{}",
        tmp_prefix(top, name),
        nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    let made = git_out(
        top,
        &[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            &plain(&tmp),
            commit,
        ],
    )
    .and_then(|_| {
        if spare.exists() {
            return Err("a spare appeared".into());
        }
        // Checked whole once, and sealed, before any start can see it: a
        // folder's time does not move when its parent is renamed.
        if !clean(&tmp) || !seal_up(&tmp, &seal) {
            let _ = std::fs::remove_file(&seal);
            return Err("the new worktree was not clean".into());
        }
        // A rename and a repair rather than `git worktree move`, which
        // refuses a repository with submodules.
        std::fs::rename(&tmp, &spare).map_err(|e| {
            let _ = std::fs::remove_file(&seal);
            e.to_string()
        })?;
        git_out(top, &["worktree", "repair", &plain(&spare)])
            .map(|_| ())
            .inspect_err(|_| {
                let _ = std::fs::remove_file(&seal);
                discard(top, &spare);
            })
    });
    if made.is_err() && tmp.exists() {
        discard(top, &tmp);
    }
    let _ = std::fs::remove_file(&lock);
    made.map_err(|why| format!("Git could not make a spare worktree: {why}"))?;
    if let Some(digests) = digests {
        observe(&spare, digests);
    }
    Ok(())
}

/// Observe the spare at `dir` into the digest file `digests`, once its
/// files are old enough for their digests to be kept.
#[cfg(unix)]
fn observe(dir: &Path, digests: &Path) {
    use coder_boundary::Snapshot;
    std::thread::sleep(SETTLE);
    if !dir.exists() {
        return;
    }
    Snapshot::recall_digests(digests);
    let _ = Snapshot::observe(dir);
    let _ = Snapshot::remember_digests(digests);
}

#[cfg(not(unix))]
fn observe(_dir: &Path, _digests: &Path) {}

/// [`prepare`] on a thread of its own, after `delay`; the caller does not
/// wait.
pub fn prepare_in_background(
    worktrees: PathBuf,
    top: PathBuf,
    name: String,
    commit: String,
    digests: Option<PathBuf>,
    delay: Duration,
) {
    let _ = std::thread::Builder::new()
        .name("coder-spare".into())
        .spawn(move || {
            std::thread::sleep(delay);
            let _ = prepare(&worktrees, &top, &name, &commit, digests.as_deref());
        });
}

/// Return `dir`, taken for a start that was then refused, as the spare
/// for `top`, or remove it when a spare is already back or it is not
/// clean.
pub fn give_back(worktrees: &Path, top: &Path, name: &str, dir: &Path) {
    let spare = path(worktrees, top, name);
    let seal = seal_path(worktrees, top, name);
    let mut left = dir.to_path_buf();
    if !spare.exists() && clean(dir) && seal_up(dir, &seal) {
        if std::fs::rename(dir, &spare).is_ok() {
            if git_out(top, &["worktree", "repair", &plain(&spare)]).is_ok() {
                return;
            }
            left = spare;
        }
        let _ = std::fs::remove_file(&seal);
    }
    let top = top.to_path_buf();
    let _ = std::thread::Builder::new()
        .name("coder-spare".into())
        .spawn(move || discard(&top, &left));
}

/// Remove what a dead preparation left for `top` (its lock is older than
/// [`STALE`]), and every spare in `worktrees` whose repository is gone.
fn sweep(worktrees: &Path, top: &Path, name: &str) {
    let lock = lock_path(worktrees, top, name);
    let stale = lock.exists() && !held(&lock);
    let Ok(entries) = std::fs::read_dir(worktrees) else {
        return;
    };
    let prefix = tmp_prefix(top, name);
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let dir = entry.path();
        if stale && file.starts_with(&prefix) {
            discard(top, &dir);
        } else if file.contains(".spare-") && !file.starts_with('.') && orphaned(&dir) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
    if stale {
        let _ = std::fs::remove_file(&lock);
    }
}

/// Whether a preparation holds `lock` now: the file names a process that
/// is still alive, and is younger than [`STALE`].
fn held(lock: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(lock) else {
        return false;
    };
    let young = meta
        .modified()
        .ok()
        .and_then(|at| SystemTime::now().duration_since(at).ok())
        .is_none_or(|age| age <= STALE);
    young && std::fs::read_to_string(lock).is_ok_and(|pid| alive(pid.trim()))
}

/// Whether the process `pid` names is alive. An empty one is (a
/// preparation that has not written its number yet).
fn alive(pid: &str) -> bool {
    pid.parse::<u32>()
        .map_or(pid.is_empty(), crate::activity::alive)
}

/// Whether the worktree at `dir` belongs to a repository that is gone: its
/// `.git` file names an administrative folder that no longer exists.
fn orphaned(dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join(".git")) else {
        return false;
    };
    text.trim()
        .strip_prefix("gitdir:")
        .is_some_and(|admin| !Path::new(admin.trim()).exists())
}
