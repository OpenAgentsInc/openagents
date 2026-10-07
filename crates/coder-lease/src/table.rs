//! The lease table on disk: `table.json` under an exclusive lock on
//! `table.lock`, and one holder lock per lease under `held/`.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Error, Holder};

/// The table file's schema.
pub const TABLE_SCHEMA: &str = "openagents.lease.table.v1";

/// Whether a lease holds its resource or waits for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// The lease holds its resource.
    Held,
    /// The lease waits in the queue.
    Waiting,
}

/// How urgent a request is. Waiters are admitted most urgent first, then
/// in arrival order, and a waiter grows one level more urgent for each
/// aging step it waits ([`Entry::effective_priority`]). The declaration
/// order is the queue order: `Owner` sorts first.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// Work that blocks an owner request.
    Owner,
    /// A check before a push.
    Push,
    /// Everything else.
    #[default]
    Normal,
    /// Speculative work that can wait.
    Background,
}

/// The variable that sets the default priority of a session's requests,
/// such as `push`. A `--priority` flag wins over it.
pub const PRIORITY_VAR: &str = "OPENAGENTS_LEASE_PRIORITY";

impl Priority {
    /// Every priority, most urgent first.
    pub const ALL: [Priority; 4] = [
        Priority::Owner,
        Priority::Push,
        Priority::Normal,
        Priority::Background,
    ];

    /// Parses `owner`, `push`, `normal`, or `background`.
    ///
    /// # Errors
    /// A sentence naming the priorities when `name` is none of them.
    pub fn parse(name: &str) -> Result<Priority, String> {
        Priority::ALL
            .into_iter()
            .find(|priority| priority.as_str() == name.trim())
            .ok_or_else(|| {
                format!(
                    "`{name}` is not a priority; the priorities are owner, push, normal, and background"
                )
            })
    }

    /// The priority's name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::Owner => "owner",
            Priority::Push => "push",
            Priority::Normal => "normal",
            Priority::Background => "background",
        }
    }

    /// The priority [`PRIORITY_VAR`] names in this environment, or `None`
    /// when it's unset or empty.
    ///
    /// # Errors
    /// A sentence when the variable names no priority.
    pub fn from_env() -> Result<Option<Priority>, String> {
        Priority::from_var(std::env::var(PRIORITY_VAR).ok().as_deref())
    }

    /// The priority a value of [`PRIORITY_VAR`] names, or `None` when the
    /// value is absent or empty.
    ///
    /// # Errors
    /// A sentence when the value names no priority.
    pub fn from_var(value: Option<&str>) -> Result<Option<Priority>, String> {
        match value.map(str::trim).filter(|value| !value.is_empty()) {
            None => Ok(None),
            Some(value) => Priority::parse(value)
                .map(Some)
                .map_err(|message| format!("{PRIORITY_VAR}: {message}")),
        }
    }

    /// The more urgent of the two.
    #[must_use]
    pub fn most_urgent(self, other: Priority) -> Priority {
        self.min(other)
    }

    /// This priority raised `levels` levels, never above `Owner`.
    #[must_use]
    pub fn raised(self, levels: u64) -> Priority {
        let index = Priority::ALL
            .iter()
            .position(|priority| *priority == self)
            .unwrap_or(0);
        let raised = u64::try_from(index).unwrap_or(0).saturating_sub(levels);
        Priority::ALL[usize::try_from(raised).unwrap_or(0)]
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One lease, held or waiting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// The lease's identifier, also its holder lock's file name.
    pub id: String,
    /// The resource, such as `build` or `issue/10755`.
    pub resource: String,
    /// How much of a counted resource it takes; 1 for an exclusive one.
    pub amount: u64,
    /// Held or waiting.
    pub state: State,
    /// Who holds it.
    pub holder: Holder,
    /// How urgent the request is.
    pub priority: Priority,
    /// Its place in the queue: lower arrived first.
    pub seq: u64,
    /// When it was requested, in Unix milliseconds.
    pub requested_at_ms: u64,
    /// When it was admitted, in Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquired_at_ms: Option<u64>,
}

impl Entry {
    /// The priority a waiter competes at, at `now_ms`: its own, raised one
    /// level for each whole `aging` step it has waited, so nothing waits
    /// forever behind a stream of more urgent requests. `None` or a zero
    /// step turns aging off.
    #[must_use]
    pub fn effective_priority(&self, now_ms: u64, aging: Option<Duration>) -> Priority {
        let step = aging
            .map(|aging| u64::try_from(aging.as_millis()).unwrap_or(u64::MAX))
            .filter(|step| *step > 0);
        let Some(step) = step else {
            return self.priority;
        };
        self.priority
            .raised(now_ms.saturating_sub(self.requested_at_ms) / step)
    }

    /// The waiter's place in its resource's queue at `now_ms`, lowest
    /// first: more urgent first, then earlier arrival.
    #[must_use]
    pub fn queue_key(&self, now_ms: u64, aging: Option<Duration>) -> (Priority, u64) {
        (self.effective_priority(now_ms, aging), self.seq)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Table {
    pub schema: String,
    pub next_seq: u64,
    pub leases: Vec<Entry>,
}

impl Default for Table {
    fn default() -> Self {
        Table {
            schema: TABLE_SCHEMA.to_owned(),
            next_seq: 1,
            leases: Vec::new(),
        }
    }
}

/// The table, read under its lock. Dropping the guard releases the lock.
pub(crate) struct Guard {
    _lock: File,
    root: PathBuf,
    pub table: Table,
}

/// Creates the root's directories, private to this user.
pub(crate) fn prepare(root: &Path) -> Result<(), Error> {
    for dir in [
        root.to_path_buf(),
        root.join("held"),
        root.join("receipts"),
        root.join("grants"),
    ] {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(&dir)?;
    }
    Ok(())
}

/// Opens a private file for reading and writing, never through a link.
pub(crate) fn private_file(path: &Path, create_new: bool) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    if create_new {
        options.create_new(true);
    } else {
        options.create(true).truncate(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}

/// Writes `bytes` to `path` through a temporary file and a rename.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut name = path.as_os_str().to_owned();
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    name.push(format!(".{}-{n}.tmp", std::process::id()));
    let temporary = PathBuf::from(name);
    let _ = std::fs::remove_file(&temporary);
    let mut file = private_file(&temporary, true)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, path)
}

pub(crate) fn held_lock(root: &Path, id: &str) -> PathBuf {
    root.join("held").join(format!("{id}.lock"))
}

impl Guard {
    /// Locks and reads the table, waiting for the lock.
    pub(crate) fn open(root: &Path) -> Result<Guard, Error> {
        prepare(root)?;
        let lock = private_file(&root.join("table.lock"), false)?;
        lock.lock()?;
        let path = root.join("table.json");
        let table = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Table>(&bytes)
                .map_err(|error| Error::Corrupt(format!("{}: {error}", path.display())))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Table::default(),
            Err(error) => return Err(error.into()),
        };
        if table.schema != TABLE_SCHEMA {
            return Err(Error::Corrupt(format!(
                "{} has schema {}, not {TABLE_SCHEMA}",
                path.display(),
                table.schema
            )));
        }
        Ok(Guard {
            _lock: lock,
            root: root.to_path_buf(),
            table,
        })
    }

    /// Writes the table back.
    pub(crate) fn save(&self) -> Result<(), Error> {
        let bytes = serde_json::to_vec_pretty(&self.table)
            .map_err(|error| Error::Corrupt(error.to_string()))?;
        write_atomic(&self.root.join("table.json"), &bytes)?;
        Ok(())
    }

    /// Drops every lease whose holder is gone, and holder locks no lease
    /// names that nobody holds. Returns how many leases it dropped.
    pub(crate) fn prune(&mut self) -> usize {
        let before = self.table.leases.len();
        let root = self.root.clone();
        self.table.leases.retain(|entry| alive(&root, &entry.id));
        let dropped = before - self.table.leases.len();
        // A holder that died between creating its lock and entering the
        // table leaves a lock file behind; a live one still holds it.
        if let Ok(files) = std::fs::read_dir(root.join("held")) {
            for file in files.flatten() {
                let name = file.file_name().to_string_lossy().into_owned();
                let Some(id) = name.strip_suffix(".lock") else {
                    // A staged lock its creator never renamed: gone once
                    // nobody holds it and it is a minute old.
                    if name.starts_with('.') && name.ends_with(".new") {
                        let old = file
                            .metadata()
                            .and_then(|meta| meta.modified())
                            .ok()
                            .and_then(|at| at.elapsed().ok())
                            .is_some_and(|age| age.as_secs() >= 60);
                        if old
                            && let Ok(handle) = OpenOptions::new().read(true).open(file.path())
                            && handle.try_lock().is_ok()
                        {
                            let _ = std::fs::remove_file(file.path());
                        }
                    }
                    continue;
                };
                if !self.table.leases.iter().any(|entry| entry.id == id) {
                    alive(&root, id);
                }
            }
        }
        dropped
    }

    /// The entry with `id`.
    pub(crate) fn find(&self, id: &str) -> Option<&Entry> {
        self.table.leases.iter().find(|entry| entry.id == id)
    }

    /// Removes the entry with `id`, returning it.
    pub(crate) fn remove(&mut self, id: &str) -> Option<Entry> {
        let index = self.table.leases.iter().position(|entry| entry.id == id)?;
        Some(self.table.leases.remove(index))
    }
}

/// Whether the holder of lease `id` still lives: its lock file exists and
/// another process holds the lock on it. A lock this process can take
/// belongs to nobody, so the file is removed.
fn alive(root: &Path, id: &str) -> bool {
    let path = held_lock(root, id);
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = match options.open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        // Something is there that can't be opened: keep the lease rather
        // than guess.
        Err(_) => return true,
    };
    match file.try_lock() {
        Ok(()) => {
            let _ = std::fs::remove_file(&path);
            false
        }
        Err(std::fs::TryLockError::WouldBlock) => true,
        Err(std::fs::TryLockError::Error(_)) => true,
    }
}
