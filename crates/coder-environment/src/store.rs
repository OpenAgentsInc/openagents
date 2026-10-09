//! Restart-safe environment store: one private JSON file per environment,
//! replaced atomically under a per-record lease and a record-revision fence.
//!
//! Reads open files read-only and never create directories, locks, or
//! markers.

use crate::{Applied, Command, Environment, ProjectLink, Refusal, VersionPin, apply, valid_id};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_RECORD_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    InvalidId,
    NotFound,
    Exists,
    /// Another writer holds this environment's lease.
    Busy,
    /// The retained revision differs from the caller's expectation.
    Fence {
        expected: u64,
        current: u64,
    },
    /// The new record would rewrite immutable history.
    Immutable,
    /// More than one live environment selects a version for one project.
    Ambiguous,
    Corrupt(&'static str),
    Io(&'static str),
    Refused(Refusal),
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId => f.write_str("Invalid environment ID."),
            Self::NotFound => f.write_str("No such environment."),
            Self::Exists => f.write_str("The environment already exists."),
            Self::Busy => f.write_str("Another process is changing this environment."),
            Self::Fence { expected, current } => write!(
                f,
                "The environment is at revision {current}, not the expected {expected}."
            ),
            Self::Immutable => f.write_str("Saved environment history cannot change."),
            Self::Ambiguous => f.write_str(
                "More than one environment selects a version for this project; retire one.",
            ),
            Self::Corrupt(m) | Self::Io(m) => f.write_str(m),
            Self::Refused(r) => r.fmt(f),
        }
    }
}
impl std::error::Error for StoreError {}
impl From<Refusal> for StoreError {
    fn from(r: Refusal) -> Self {
        Self::Refused(r)
    }
}
pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}

/// Exclusive writer lease for one environment.
pub struct Lease {
    path: PathBuf,
    _lock: File,
}

impl Store {
    pub fn under(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    fn path(&self, id: &str) -> Result<PathBuf> {
        if !valid_id(id) {
            return Err(StoreError::InvalidId);
        }
        Ok(self.root.join(format!("{id}.json")))
    }

    /// Read one environment. No side effects.
    pub fn read(&self, id: &str) -> Result<Environment> {
        read_record(&self.path(id)?, id)
    }
    /// Every retained environment, oldest first. No side effects.
    pub fn list(&self) -> Result<Vec<Environment>> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(_) => return Err(StoreError::Io("Cannot list environments.")),
        };
        let mut rows = vec![];
        for entry in entries {
            let path = entry
                .map_err(|_| StoreError::Io("Cannot read the environment directory."))?
                .path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            let Some(id) = path.file_stem().and_then(|v| v.to_str()) else {
                continue;
            };
            rows.push(read_record(&path, id)?);
        }
        rows.sort_by(|a, b| (a.created_ms, &a.id).cmp(&(b.created_ms, &b.id)));
        Ok(rows)
    }

    /// The exact version a job admitted now for `project` starts with, or
    /// `None` when no live environment selects one. Read once at job
    /// admission; the job keeps the returned pin. No side effects.
    pub fn selected(&self, project: &ProjectLink) -> Result<Option<VersionPin>> {
        let mut pins = self
            .list()?
            .into_iter()
            .filter(|e| &e.project == project)
            .filter_map(|e| e.pin());
        let pin = pins.next();
        if pins.next().is_some() {
            return Err(StoreError::Ambiguous);
        }
        Ok(pin)
    }

    pub fn lease(&self, id: &str) -> Result<Lease> {
        let path = self.path(id)?;
        fs::create_dir_all(&self.root)
            .map_err(|_| StoreError::Io("Cannot create the environment store."))?;
        protect_dir(&self.root)?;
        let lock = path.with_extension("lock");
        regular_or_missing(&lock)?;
        let file = private_options()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(lock)
            .map_err(|_| StoreError::Io("Cannot open the environment lock."))?;
        lock_waiting(&file).map_err(|()| StoreError::Busy)?;
        Ok(Lease { path, _lock: file })
    }

    /// Retain a new environment; refuses to replace an existing one.
    pub fn create(&self, env: &Environment) -> Result<()> {
        let lease = self.lease(&env.id)?;
        if fs::symlink_metadata(&lease.path).is_ok() {
            return Err(StoreError::Exists);
        }
        env.validate().map_err(StoreError::Corrupt)?;
        write_atomic(&lease.path, env)
    }

    /// Apply one command under the lease and retain the result before
    /// returning it. A replay or refusal writes nothing.
    pub fn apply(&self, id: &str, command: &Command, now_ms: u64) -> Result<Applied> {
        let lease = self.lease(id)?;
        let current = lease.read(id)?;
        let applied = apply(&current, command, now_ms)?;
        if let Applied::Changed(next, _) = &applied {
            lease.commit(id, current.revision, next)?;
        }
        Ok(applied)
    }
}

impl Lease {
    pub fn read(&self, id: &str) -> Result<Environment> {
        read_record(&self.path, id)
    }
    /// Replace the record if it is still at `expected_revision` and `next`
    /// preserves its immutable history.
    pub fn commit(&self, id: &str, expected_revision: u64, next: &Environment) -> Result<()> {
        let current = self.read(id)?;
        if current.revision != expected_revision {
            return Err(StoreError::Fence {
                expected: expected_revision,
                current: current.revision,
            });
        }
        next.validate().map_err(StoreError::Corrupt)?;
        if !current.preserves_history_of(next) {
            return Err(StoreError::Immutable);
        }
        write_atomic(&self.path, next)
    }
}

fn write_atomic(path: &Path, env: &Environment) -> Result<()> {
    let bytes =
        serde_json::to_vec(env).map_err(|_| StoreError::Io("Cannot encode the environment."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(StoreError::Io("The environment exceeds its size limit."));
    }
    regular_or_missing(path)?;
    let temp = path.with_extension("writing");
    regular_or_missing(&temp)?;
    let mut file = private_options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temp)
        .map_err(|_| StoreError::Io("Cannot write the environment."))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| StoreError::Io("Cannot retain the environment."))?;
    fs::rename(&temp, path).map_err(|_| StoreError::Io("Cannot replace the environment."))?;
    File::open(path.parent().expect("record has a parent"))
        .and_then(|d| d.sync_all())
        .map_err(|_| StoreError::Io("Cannot sync the environment store."))
}

fn read_record(path: &Path, id: &str) -> Result<Environment> {
    let file = match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => File::open(path),
        Ok(_) => {
            return Err(StoreError::Corrupt(
                "Environment files must be regular files.",
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StoreError::NotFound),
        Err(_) => return Err(StoreError::Io("Cannot inspect the environment.")),
    };
    let mut bytes = vec![];
    file.and_then(|f| f.take(MAX_RECORD_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(|_| StoreError::Io("Cannot read the environment."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(StoreError::Corrupt(
            "The environment exceeds its size limit.",
        ));
    }
    let env: Environment = serde_json::from_slice(&bytes)
        .map_err(|_| StoreError::Corrupt("Cannot decode the environment."))?;
    if env.id != id {
        return Err(StoreError::Corrupt("Environment identity mismatch."));
    }
    env.validate().map_err(StoreError::Corrupt)?;
    Ok(env)
}

/// How long a lease waits for a held lock before refusing as busy.
pub const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// Take `file`'s exclusive lock, waiting up to [`LOCK_WAIT`] for a holder.
///
/// A lease holds its lock only for one short local operation, but the lock
/// can also look held after its holder dropped it: `flock` belongs to the
/// open file description, and a thread that forks a child (a PTY, or any
/// spawn with a `pre_exec` hook) shares every open descriptor with that
/// child until it execs. Standard-library files are opened close-on-exec, so
/// the exec drops them, but the window before it can be long under load.
/// Retrying briefly absorbs that window; a genuinely concurrent holder that
/// keeps the lock past the wait is still refused. The environment, setup,
/// build, verify, and working-computer stores all lock through this.
pub fn lock_waiting(file: &File) -> std::result::Result<(), ()> {
    let deadline = std::time::Instant::now() + LOCK_WAIT;
    let mut pause = std::time::Duration::from_millis(1);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(fs::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(pause);
                pause = (pause * 2).min(std::time::Duration::from_millis(25));
            }
            Err(_) => return Err(()),
        }
    }
}

fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}
fn protect_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| StoreError::Io("Cannot protect the environment store."))?;
    }
    Ok(())
}
fn regular_or_missing(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Ok(()),
        Ok(_) => Err(StoreError::Corrupt(
            "Environment files must be regular files.",
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(StoreError::Io("Cannot inspect an environment file.")),
    }
}
