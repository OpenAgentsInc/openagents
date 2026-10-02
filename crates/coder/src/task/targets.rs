//! Reusable Cargo build slots outside task worktrees.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use super::{Error, Store};

const SLOTS: usize = background::SLOTS;
const BUDGET: u64 = 64 * 1024 * 1024 * 1024;

fn root(store: &Path) -> PathBuf {
    store.parent().unwrap_or(store).join("targets")
}

/// A slot held for the entire run. The stable lock file is never removed.
pub struct Lease {
    pub path: PathBuf,
    lock: File,
    store: PathBuf,
}

impl Lease {
    /// Take a free slot for the repository's common Git directory.
    pub fn acquire(store: &Path, common: &Path) -> Result<Self, Error> {
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
        for slot in 0..SLOTS {
            let path = root.join(format!("{project}-slot-{slot}"));
            if let Some(lock) = lock(&path)? {
                if std::fs::symlink_metadata(&path).is_ok_and(|meta| !meta.is_dir()) {
                    return Err(Error::UnsafePath);
                }
                std::fs::create_dir_all(&path)?;
                return Ok(Self {
                    path,
                    lock,
                    store: store.to_owned(),
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

/// Remove known ended tasks' legacy builds and trim idle slots to 64 GiB.
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
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_file() {
        return Ok(meta.len());
    }
    if !meta.is_dir() {
        return Ok(0);
    }
    let mut bytes: u64 = 0;
    for entry in std::fs::read_dir(path)? {
        bytes = bytes.saturating_add(size(&entry?.path())?);
    }
    Ok(bytes)
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
