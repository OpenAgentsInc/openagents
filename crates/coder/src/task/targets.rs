//! Reusable Cargo build slots outside task worktrees.

use background::volume::{Statvfs, Volumes};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::{Error, Store};

const SLOTS: usize = background::SLOTS;
const BUDGET: u64 = 64 * 1024 * 1024 * 1024;

/// Slot limits, in decimal gigabytes. Environment overrides apply on admission.
#[derive(Clone, Copy, Debug)]
struct Policy {
    cap: u64,
    floor: u64,
    keep: usize,
}

impl Policy {
    /// The environment first, then `coder.slot_cap_gb` and
    /// `coder.slot_free_gb` from this computer's settings, then the defaults.
    fn from_env() -> Result<Self, Error> {
        let coder = super::settings::load().map(|s| s.coder).unwrap_or_default();
        Self::read(&|name| {
            std::env::var(name).ok().or_else(|| {
                match name {
                    "OPENAGENTS_SLOT_CAP_GB" => coder.slot_cap_gb,
                    "OPENAGENTS_SLOT_FREE_GB" => coder.slot_free_gb,
                    _ => None,
                }
                .map(|gb| gb.to_string())
            })
        })
    }

    fn read(env: &impl Fn(&str) -> Option<String>) -> Result<Self, Error> {
        let number = |name, default| -> Result<u64, Error> {
            env(name).map_or(Ok(default), |value| {
                value.parse().map_err(|_| {
                    Error::InvalidCommand("slot limits must be nonnegative whole numbers")
                })
            })
        };
        let gb = |name, default| {
            number(name, default)?
                .checked_mul(1_000_000_000)
                .ok_or(Error::InvalidCommand("slot limit is too large"))
        };
        let keep = number("OPENAGENTS_SLOT_KEEP_BUILDS", 3)?;
        if !(1..=100).contains(&keep) {
            return Err(Error::InvalidCommand(
                "slot retention must be between 1 and 100 builds",
            ));
        }
        Ok(Self {
            cap: gb("OPENAGENTS_SLOT_CAP_GB", 25)?,
            floor: gb("OPENAGENTS_SLOT_FREE_GB", 10)?,
            keep: keep as usize,
        })
    }
}

fn root(store: &Path) -> PathBuf {
    background::task_targets(store)
}

/// A slot held for the entire run. The stable lock file is never removed.
pub struct Lease {
    pub path: PathBuf,
    lock: File,
    store: PathBuf,
    policy: Policy,
    cutoff: SystemTime,
}

impl Lease {
    /// Take a free slot for the repository's common Git directory.
    pub fn acquire(store: &Path, common: &Path) -> Result<Self, Error> {
        Self::acquire_with(store, common, Policy::from_env()?, &Statvfs)
    }

    fn acquire_with(
        store: &Path,
        common: &Path,
        policy: Policy,
        volumes: &dyn Volumes,
    ) -> Result<Self, Error> {
        let common = common.canonicalize()?;
        let project = common
            .parent()
            .and_then(Path::file_name)
            .unwrap_or_else(|| std::ffi::OsStr::new("project"))
            .to_string_lossy();
        let digest = nostr::contracts::digest_bytes(common.as_os_str().as_encoded_bytes());
        let tag = &digest.trim_start_matches("sha256:")[..12];
        // The path digest keeps unrelated repositories with the same name apart.
        let project = format!("{project}-{tag}");
        let root = root(store);
        std::fs::create_dir_all(&root)?;
        let free = volumes.space(&root)?.free;
        if free < policy.floor {
            // Reclaim idle caches before deciding to wait. Never build elsewhere
            // to bypass this refusal.
            maintain(&root, policy, volumes)?;
            let free = volumes.space(&root)?.free;
            if free < policy.floor {
                return Err(Error::BuildDiskLow {
                    free,
                    floor: policy.floor,
                });
            }
        }
        for slot in 0..SLOTS {
            let path = root.join(format!("{project}-slot-{slot}"));
            if let Some(mut lock) = lock(&path)? {
                if std::fs::symlink_metadata(&path).is_ok_and(|meta| !meta.is_dir()) {
                    return Err(Error::UnsafePath);
                }
                std::fs::create_dir_all(&path)?;
                let cutoff = remember_build(&mut lock, policy.keep)?;
                return Ok(Self {
                    path,
                    lock,
                    store: store.to_owned(),
                    policy,
                    cutoff,
                });
            }
        }
        Err(Error::Busy)
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        // Touch the stable lock to record last use, including interrupted runs.
        let _ = self.lock.set_modified(std::time::SystemTime::now());
        // Keep the slot exclusively held until pruning ends, so the next run
        // cannot compile beside deletion.
        if let Err(error) = prune(&self.path, self.policy, self.cutoff, &Statvfs) {
            eprintln!(
                "coder: cannot prune build slot {}: {error}",
                self.path.display()
            );
        }
        let _ = self.lock.unlock();
        let _ = cleanup(&self.store);
    }
}

fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

fn lock(path: &Path) -> Result<Option<File>, Error> {
    let path = lock_path(path);
    if std::fs::symlink_metadata(&path).is_ok_and(|meta| !meta.is_file()) {
        return Err(Error::UnsafePath);
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

fn legacy(store: &Path, workspace: &Path) -> PathBuf {
    let name = workspace
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("workspace"))
        .to_string_lossy();
    let digest = nostr::contracts::digest_bytes(workspace.as_os_str().as_encoded_bytes());
    root(store).join(format!(
        "{name}-{}",
        &digest.trim_start_matches("sha256:")[..12]
    ))
}

/// Remove ended legacy builds, prune oversized idle slots, and trim the pool to 64 GiB.
/// Unknown or live tasks, symlinks, and locked slots are never deleted.
pub fn cleanup(store: &Path) -> Result<(), Error> {
    let tasks = Store::open(store)?.list()?;
    let active: std::collections::BTreeSet<_> = tasks
        .iter()
        .filter(|task| !ended(task))
        .map(|task| legacy(store, Path::new(&task.intent.workspace.path)))
        .collect();
    for task in tasks.iter().filter(|task| ended(task)) {
        let path = legacy(store, Path::new(&task.intent.workspace.path));
        if !active.contains(&path) && real_dir(&path) {
            std::fs::remove_dir_all(path)?;
        }
    }
    maintain(&root(store), Policy::from_env()?, &Statvfs)?;
    trim(&root(store), BUDGET)
}

fn ended(task: &super::Task) -> bool {
    task.ended()
}

/// What the background disk monitor needs from the task store: each
/// task's worktree, its per-task build directory, and whether it ended.
///
/// # Errors
/// The store cannot be read.
pub fn facts(store: &Path) -> Result<Vec<background::TaskFact>, String> {
    if !super::present(store) {
        return Ok(Vec::new());
    }
    let tasks = Store::open_waiting(store, std::time::Duration::from_secs(30))
        .and_then(|store| store.list())
        .map_err(|error| error.to_string())?;
    Ok(tasks
        .iter()
        .map(|task| {
            let workspace = Path::new(&task.intent.workspace.path);
            background::TaskFact {
                id: task.task_id.clone(),
                worktree: workspace.to_owned(),
                target: legacy(store, workspace),
                ended: ended(task),
                failed: task.execution == super::Execution::Failed
                    || task.checks == super::Checks::Failed,
                cancelled: task.status == super::Status::Cancelled,
                running: matches!(
                    task.status,
                    super::Status::Queued | super::Status::Running | super::Status::CancelRequested
                ),
            }
        })
        .collect())
}

fn real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

fn size(path: &Path) -> Result<u64, Error> {
    Ok(background::view::slot_bytes(path)?)
}

// The stable lock also keeps the starts of the last N builds. A Cargo cache
// hit does not update artifact mtimes; retaining several starts keeps recent
// build variants warm while older artifacts can be rebuilt on demand.
fn history(lock: &mut File) -> Result<Vec<u64>, Error> {
    lock.rewind()?;
    let mut text = String::new();
    lock.take(16 * 1024).read_to_string(&mut text)?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

fn remember_build(lock: &mut File, keep: usize) -> Result<SystemTime, Error> {
    let mut builds = history(lock)?;
    builds.push(background::paths::now());
    let skip = builds.len().saturating_sub(keep);
    builds.drain(..skip);
    lock.rewind()?;
    lock.set_len(0)?;
    lock.write_all(&serde_json::to_vec(&builds).unwrap())?;
    Ok(std::time::UNIX_EPOCH + std::time::Duration::from_secs(builds[0]))
}

fn maintain(root: &Path, policy: Policy, volumes: &dyn Volumes) -> Result<(), Error> {
    for slot in background::view::slots(root, false) {
        let Some(mut held) = lock(&slot.path)? else {
            continue;
        };
        let builds = history(&mut held)?;
        let cutoff = std::time::UNIX_EPOCH
            + std::time::Duration::from_secs(
                builds
                    .first()
                    .copied()
                    .unwrap_or_else(background::paths::now),
            );
        prune(&slot.path, policy, cutoff, volumes)?;
    }
    Ok(())
}

fn prune(
    path: &Path,
    policy: Policy,
    cutoff: SystemTime,
    volumes: &dyn Volumes,
) -> Result<(), Error> {
    if !real_dir(path) {
        return Ok(());
    }
    if size(path)? <= policy.cap && volumes.space(path)?.free >= policy.floor {
        return Ok(());
    }
    // Cargo supports custom profiles and cross-compilation target subfolders.
    // Walk only real directories and never follow links into another tree.
    let mut stack = vec![path.to_owned()];
    while let Some(dir) = stack.pop() {
        let cargo_lock = dir.join(".cargo-lock");
        let _cargo = if cargo_lock.exists() {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&cargo_lock)?;
            if file.try_lock().is_err() {
                continue;
            }
            Some(file)
        } else {
            None
        };
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let child = entry.path();
            if entry.file_name() == "incremental" {
                std::fs::remove_dir_all(child)?;
            } else if entry.file_name() == "deps" {
                for artifact in std::fs::read_dir(child)? {
                    let artifact = artifact?;
                    let meta = std::fs::symlink_metadata(artifact.path())?;
                    if meta.is_file() && meta.modified()? < cutoff {
                        std::fs::remove_file(artifact.path())?;
                    }
                }
            } else if !matches!(entry.file_name().to_str(), Some("build" | ".fingerprint")) {
                stack.push(child);
            }
        }
    }
    Ok(())
}

// Budget eviction removes the whole slot, so hold all profile locks, not
// just the slot lease. A manually launched Cargo build may hold only its
// profile lock.
fn cargo_locks(path: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut paths = Vec::new();
    let mut stack = vec![path.to_owned()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_name() == ".cargo-lock" {
                paths.push(entry.path());
            } else if entry.file_type()?.is_dir()
                && !matches!(
                    entry.file_name().to_str(),
                    Some("deps" | "build" | "incremental" | ".fingerprint")
                )
            {
                stack.push(entry.path());
            }
        }
    }
    Ok(paths)
}

fn trim(root: &Path, budget: u64) -> Result<(), Error> {
    if !real_dir(root) {
        return Ok(());
    }
    let mut slots = Vec::new();
    let mut total: u64 = 0;
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name
            .rsplit_once("-slot-")
            .is_some_and(|(_, slot)| slot.parse::<usize>().is_ok())
            || !real_dir(&path)
        {
            continue;
        }
        let bytes = size(&path)?;
        total = total.saturating_add(bytes);
        let used = std::fs::metadata(lock_path(&path))
            .and_then(|meta| meta.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        slots.push((used, path));
    }
    slots.sort();
    for (_, path) in slots {
        if total <= budget {
            break;
        }
        let Some(_lock) = lock(&path)? else {
            continue;
        };
        if !real_dir(&path) {
            continue;
        }
        let locks = cargo_locks(&path)?;
        let Ok(_cargo) = background::inuse::Held::take(&locks) else {
            continue;
        };
        // Preserve compiled dependencies first; if that is insufficient, evict
        // the idle slot. Live builds may temporarily exceed the budget.
        let before = size(&path)?;
        let incremental = path.join("debug/incremental");
        if real_dir(&path.join("debug")) && real_dir(&incremental) {
            std::fs::remove_dir_all(&incremental)?;
        }
        total = total.saturating_sub(before.saturating_sub(size(&path)?));
        if total > budget {
            let bytes = size(&path)?;
            std::fs::remove_dir_all(&path)?;
            total = total.saturating_sub(bytes);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Free(u64);
    impl Volumes for Free {
        fn space(&self, _: &Path) -> std::io::Result<background::volume::Space> {
            Ok(background::volume::Space {
                device: 1,
                free: self.0,
                total: 1000,
            })
        }
    }

    fn tree(path: &Path) {
        for profile in ["debug", "release", "aarch64-test/custom"] {
            let profile = path.join(profile);
            std::fs::create_dir_all(profile.join("deps")).unwrap();
            std::fs::create_dir_all(profile.join("incremental")).unwrap();
            std::fs::write(profile.join("incremental/cache"), [0; 20]).unwrap();
            std::fs::write(profile.join("deps/stale.rlib"), [0; 10]).unwrap();
            File::options()
                .write(true)
                .open(profile.join("deps/stale.rlib"))
                .unwrap()
                .set_modified(std::time::UNIX_EPOCH)
                .unwrap();
            std::fs::write(profile.join("deps/recent.rlib"), [0; 10]).unwrap();
        }
    }

    #[test]
    fn budget_eviction_keeps_slots_with_cargo_locks_in_any_profile() {
        for profile in ["debug", "release", "aarch64-test/custom"] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("targets");
            let slot = root.join("project-slot-0");
            tree(&slot);
            let cargo = File::create(slot.join(profile).join(".cargo-lock")).unwrap();
            cargo.lock().unwrap();
            trim(&root, 0).unwrap();
            assert!(slot.join(profile).join("incremental/cache").exists());
            drop(cargo);
            trim(&root, 0).unwrap();
            assert!(!slot.exists());
        }
    }

    #[test]
    fn policy_defaults_overrides_and_invalid_values() {
        let defaults = Policy::read(&|_| None).unwrap();
        assert_eq!(
            (defaults.cap, defaults.floor, defaults.keep),
            (25_000_000_000, 10_000_000_000, 3)
        );
        let custom = Policy::read(&|name| {
            Some(
                match name {
                    "OPENAGENTS_SLOT_CAP_GB" => "7",
                    "OPENAGENTS_SLOT_FREE_GB" => "2",
                    _ => "5",
                }
                .into(),
            )
        })
        .unwrap();
        assert_eq!(
            (custom.cap, custom.floor, custom.keep),
            (7_000_000_000, 2_000_000_000, 5)
        );
        for value in ["bad", "-1", "18446744073709551615"] {
            assert!(Policy::read(&|_| Some(value.into())).is_err());
        }
        assert!(
            Policy::read(&|name| (name == "OPENAGENTS_SLOT_KEEP_BUILDS").then(|| "0".into()))
                .is_err()
        );
    }

    #[test]
    fn pressure_prunes_incremental_and_old_deps_in_all_profiles_but_keeps_recent_deps() {
        for (cap, free) in [(1, 100), (1000, 0)] {
            let dir = tempfile::tempdir().unwrap();
            tree(dir.path());
            let cutoff = SystemTime::now() - std::time::Duration::from_secs(60);
            prune(
                dir.path(),
                Policy {
                    cap,
                    floor: 10,
                    keep: 3,
                },
                cutoff,
                &Free(free),
            )
            .unwrap();
            for profile in ["debug", "release", "aarch64-test/custom"] {
                let profile = dir.path().join(profile);
                assert!(!profile.join("incremental").exists());
                assert!(!profile.join("deps/stale.rlib").exists());
                assert!(profile.join("deps/recent.rlib").exists());
            }
        }
    }

    #[test]
    fn below_limits_keeps_the_entire_build_warm() {
        let dir = tempfile::tempdir().unwrap();
        tree(dir.path());
        let before = size(dir.path()).unwrap();
        prune(
            dir.path(),
            Policy {
                cap: 1000,
                floor: 10,
                keep: 3,
            },
            SystemTime::now(),
            &Free(100),
        )
        .unwrap();
        assert_eq!(size(dir.path()).unwrap(), before);
        assert!(dir.path().join("debug/incremental/cache").exists());
        assert!(dir.path().join("debug/deps/stale.rlib").exists());
    }

    #[test]
    fn releasing_an_oversized_lease_prunes_before_reuse() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let common = dir.path().join("project/.git");
        std::fs::create_dir_all(&common).unwrap();
        let policy = Policy {
            cap: 1,
            floor: 0,
            keep: 3,
        };
        let lease = Lease::acquire_with(&store, &common, policy, &Free(100)).unwrap();
        let path = lease.path.clone();
        tree(&path);
        drop(lease);
        assert!(!path.join("debug/incremental").exists());
        assert!(!path.join("debug/deps/stale.rlib").exists());
        assert!(path.join("debug/deps/recent.rlib").exists());
        let lease = Lease::acquire_with(&store, &common, policy, &Free(100)).unwrap();
        assert_eq!(lease.path, path);
    }

    #[test]
    fn low_disk_does_not_start_even_a_warm_slot_or_overflow_slot_and_retries_when_free() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let common = dir.path().join("project/.git");
        std::fs::create_dir_all(&common).unwrap();
        let policy = Policy {
            cap: 1000,
            floor: 10,
            keep: 3,
        };
        assert!(matches!(
            Lease::acquire_with(&store, &common, policy, &Free(9)),
            Err(Error::BuildDiskLow { free: 9, floor: 10 })
        ));
        assert!(background::view::slots(&root(&store), false).is_empty());
        let lease = Lease::acquire_with(&store, &common, policy, &Free(10)).unwrap();
        tree(&lease.path);
        let live = lease.path.clone();
        assert!(matches!(
            Lease::acquire_with(&store, &common, policy, &Free(9)),
            Err(Error::BuildDiskLow { .. })
        ));
        assert!(live.join("debug/incremental/cache").exists());
        drop(lease);
        assert!(Lease::acquire_with(&store, &common, policy, &Free(10)).is_ok());
    }

    #[test]
    fn retention_keeps_only_the_last_n_build_starts_in_the_stable_lock() {
        let dir = tempfile::tempdir().unwrap();
        let mut lock = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(dir.path().join("lock"))
            .unwrap();
        lock.write_all(b"[1,2,3]").unwrap();
        assert_eq!(
            remember_build(&mut lock, 3).unwrap(),
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(2)
        );
        let builds = history(&mut lock).unwrap();
        assert_eq!(builds.len(), 3);
        assert_eq!(&builds[..2], &[2, 3]);
    }

    #[test]
    fn pruning_never_follows_links_or_touches_a_cargo_locked_profile() {
        let dir = tempfile::tempdir().unwrap();
        let slot = dir.path().join("slot");
        tree(&slot);
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("keep"), [0; 20]).unwrap();
        std::os::unix::fs::symlink(&outside, slot.join("linked-profile")).unwrap();
        std::os::unix::fs::symlink(outside.join("keep"), slot.join("release/deps/link")).unwrap();
        let cargo = File::create(slot.join("debug/.cargo-lock")).unwrap();
        cargo.lock().unwrap();
        prune(
            &slot,
            Policy {
                cap: 0,
                floor: 0,
                keep: 3,
            },
            SystemTime::now(),
            &Free(100),
        )
        .unwrap();
        assert!(slot.join("debug/incremental/cache").exists());
        assert!(slot.join("debug/deps/stale.rlib").exists());
        assert!(!slot.join("release/deps/stale.rlib").exists());
        assert!(outside.join("keep").exists());
        assert!(slot.join("release/deps/link").is_symlink());
    }

    #[test]
    fn sequential_tasks_reuse_and_concurrent_tasks_take_distinct_slots() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let common = dir.path().join("project.with.dots/.git");
        std::fs::create_dir_all(&common).unwrap();
        let first = Lease::acquire(&store, &common).unwrap();
        let path = first.path.clone();
        std::fs::write(path.join("warm"), "cached dependency").unwrap();
        let second = Lease::acquire(&store, &common).unwrap();
        assert_ne!(path, second.path);
        assert!(!path.starts_with(common.parent().unwrap()));
        drop(first);
        let third = Lease::acquire(&store, &common).unwrap();
        assert_eq!(path, third.path);
        assert!(third.path.join("warm").exists());
        let fourth = Lease::acquire(&store, &common).unwrap();
        let fifth = Lease::acquire(&store, &common).unwrap();
        assert!(matches!(Lease::acquire(&store, &common), Err(Error::Busy)));
        drop((second, third, fourth, fifth));
    }

    #[test]
    fn ended_legacy_removed_but_live_and_unknown_directories_remain() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let workspace = dir.path().join("project-task");
        let mut inbox = Store::open(&store).unwrap();
        let submit = serde_json::json!({"schema": super::super::COMMAND_SCHEMA, "command_id":"submit", "task_id":"task", "expected_revision":null, "action":{"type":"submit", "intent":{"title":"test", "prompt":"test", "workspace":{"path":workspace,"source_revision":null}, "configuration":{"adapter":"test", "model":null}}}});
        inbox.apply(&serde_json::to_vec(&submit).unwrap()).unwrap();
        drop(inbox);
        let old = legacy(&store, &workspace);
        std::fs::create_dir_all(&old).unwrap();
        let unknown = root(&store).join("unrecognized-task-cache");
        std::fs::create_dir_all(&unknown).unwrap();
        cleanup(&store).unwrap();
        assert!(old.exists());
        let mut inbox = Store::open(&store).unwrap();
        let cancel = serde_json::json!({"schema": super::super::COMMAND_SCHEMA, "command_id":"cancel", "task_id":"task", "expected_revision":1, "action":{"type":"cancel", "reason":"done"}});
        inbox.apply(&serde_json::to_vec(&cancel).unwrap()).unwrap();
        drop(inbox);
        let _host = super::super::remote::Inbox::new(&store, Default::default());
        assert!(!old.exists());
        assert!(unknown.exists());
    }

    #[test]
    fn facts_name_each_tasks_worktree_target_and_end() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        assert!(facts(&store).unwrap().is_empty());
        let workspace = dir.path().join("project-task");
        let mut inbox = Store::open(&store).unwrap();
        let submit = serde_json::json!({"schema": super::super::COMMAND_SCHEMA, "command_id":"submit", "task_id":"task", "expected_revision":null, "action":{"type":"submit", "intent":{"title":"test", "prompt":"test", "workspace":{"path":workspace,"source_revision":null}, "configuration":{"adapter":"test", "model":null}}}});
        inbox.apply(&serde_json::to_vec(&submit).unwrap()).unwrap();
        drop(inbox);
        let found = facts(&store).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].worktree, workspace);
        assert_eq!(found[0].target, legacy(&store, &workspace));
        assert!(!found[0].ended);
        let mut inbox = Store::open(&store).unwrap();
        let cancel = serde_json::json!({"schema": super::super::COMMAND_SCHEMA, "command_id":"cancel", "task_id":"task", "expected_revision":1, "action":{"type":"cancel", "reason":"done"}});
        inbox.apply(&serde_json::to_vec(&cancel).unwrap()).unwrap();
        drop(inbox);
        assert!(facts(&store).unwrap()[0].ended);
    }

    #[test]
    fn same_named_projects_have_separate_pools() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let a = dir.path().join("a/project/.git");
        let b = dir.path().join("b/project/.git");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let first = Lease::acquire(&store, &a).unwrap();
        let second = Lease::acquire(&store, &b).unwrap();
        assert_ne!(first.path, second.path);
    }

    #[test]
    fn releasing_a_run_removes_ended_legacy_builds() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let workspace = dir.path().join("project-task");
        let common = dir.path().join("project/.git");
        std::fs::create_dir_all(&common).unwrap();
        let lease = Lease::acquire(&store, &common).unwrap();
        let mut inbox = Store::open(&store).unwrap();
        let submit = serde_json::json!({"schema": super::super::COMMAND_SCHEMA, "command_id":"submit", "task_id":"task", "expected_revision":null, "action":{"type":"submit", "intent":{"title":"test", "prompt":"test", "workspace":{"path":workspace,"source_revision":null}, "configuration":{"adapter":"test", "model":null}}}});
        inbox.apply(&serde_json::to_vec(&submit).unwrap()).unwrap();
        let cancel = serde_json::json!({"schema": super::super::COMMAND_SCHEMA, "command_id":"cancel", "task_id":"task", "expected_revision":1, "action":{"type":"cancel", "reason":"done"}});
        inbox.apply(&serde_json::to_vec(&cancel).unwrap()).unwrap();
        drop(inbox);
        let old = legacy(&store, &workspace);
        std::fs::create_dir_all(&old).unwrap();
        drop(lease);
        assert!(!old.exists());
    }

    #[cfg(unix)]
    #[test]
    fn trimming_does_not_follow_incremental_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let common = dir.path().join("project/.git");
        std::fs::create_dir_all(&common).unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("keep"), [0; 10]).unwrap();
        let lease = Lease::acquire(&store, &common).unwrap();
        std::fs::create_dir_all(lease.path.join("debug")).unwrap();
        std::os::unix::fs::symlink(&outside, lease.path.join("debug/incremental")).unwrap();
        std::fs::write(lease.path.join("artifact"), [0; 10]).unwrap();
        drop(lease);
        trim(&root(&store), 0).unwrap();
        assert!(outside.join("keep").exists());
    }

    #[test]
    fn budget_evicts_oldest_idle_slot_when_incremental_is_insufficient() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let common = dir.path().join("project/.git");
        std::fs::create_dir_all(&common).unwrap();
        let first = Lease::acquire(&store, &common).unwrap();
        let second = Lease::acquire(&store, &common).unwrap();
        let older = first.path.clone();
        let newer = second.path.clone();
        std::fs::write(older.join("dependency"), [0; 10]).unwrap();
        std::fs::write(newer.join("dependency"), [0; 10]).unwrap();
        drop((first, second));
        OpenOptions::new()
            .write(true)
            .open(lock_path(&older))
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH)
            .unwrap();
        trim(&root(&store), 10).unwrap();
        assert!(!older.exists());
        assert!(newer.join("dependency").exists());
    }

    #[test]
    fn budget_trims_idle_incremental_but_never_a_locked_slot() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let common = dir.path().join("project/.git");
        std::fs::create_dir_all(&common).unwrap();
        let live = Lease::acquire(&store, &common).unwrap();
        std::fs::write(live.path.join("artifact"), [0; 10]).unwrap();
        let idle = Lease::acquire(&store, &common).unwrap();
        let path = idle.path.clone();
        std::fs::create_dir_all(path.join("debug/incremental")).unwrap();
        std::fs::write(path.join("debug/incremental/cache"), [0; 20]).unwrap();
        drop(idle);
        trim(&root(&store), 10).unwrap();
        assert!(live.path.join("artifact").exists());
        assert!(path.exists());
        assert!(!path.join("debug/incremental").exists());
        trim(&root(&store), 0).unwrap();
        assert!(live.path.exists());
    }
}
