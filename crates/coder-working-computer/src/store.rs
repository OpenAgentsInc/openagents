//! Restart-safe computer store: one private JSON file per computer,
//! replaced atomically under a per-record lease and a revision fence,
//! following `coder_environment::store`.

use crate::{Applied, Command, Computer, Refusal, apply};
use coder_environment::valid_id;
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
    /// Another owner holds this computer's lease.
    Busy,
    Fence {
        expected: u64,
        current: u64,
    },
    /// The new record would rewrite retained history.
    Immutable,
    Corrupt(&'static str),
    Io(&'static str),
    Refused(Refusal),
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId => f.write_str("Invalid computer ID."),
            Self::NotFound => f.write_str("No such computer."),
            Self::Exists => f.write_str("The computer already exists."),
            Self::Busy => f.write_str("Another process is changing this computer."),
            Self::Fence { expected, current } => write!(
                f,
                "The computer is at revision {current}, not the expected {expected}."
            ),
            Self::Immutable => f.write_str("Retained computer history cannot change."),
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

/// Exclusive owner lease for one computer, held across its provider effects.
pub struct Lease {
    path: PathBuf,
    id: String,
    _lock: File,
}

impl Store {
    pub fn under(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    fn path(&self, id: &str) -> Result<PathBuf> {
        if !valid_id(id) {
            return Err(StoreError::InvalidId);
        }
        Ok(self.root.join(format!("{id}.json")))
    }
    /// Read one computer. No side effects.
    pub fn read(&self, id: &str) -> Result<Computer> {
        read_record(&self.path(id)?, id)
    }
    pub fn lease(&self, id: &str) -> Result<Lease> {
        let path = self.path(id)?;
        fs::create_dir_all(&self.root)
            .map_err(|_| StoreError::Io("Cannot create the computer store."))?;
        protect_dir(&self.root)?;
        let lock = path.with_extension("lock");
        regular_or_missing(&lock)?;
        let file = private_options()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(lock)
            .map_err(|_| StoreError::Io("Cannot open the computer lock."))?;
        coder_environment::store::lock_waiting(&file).map_err(|()| StoreError::Busy)?;
        Ok(Lease {
            path,
            id: id.into(),
            _lock: file,
        })
    }
    /// Retain a new computer; refuses to replace an existing one.
    pub fn create(&self, computer: &Computer) -> Result<()> {
        let lease = self.lease(&computer.id)?;
        if fs::symlink_metadata(&lease.path).is_ok() {
            return Err(StoreError::Exists);
        }
        computer.validate().map_err(StoreError::Corrupt)?;
        write_atomic(&lease.path, computer)
    }
}

impl Lease {
    pub fn read(&self) -> Result<Computer> {
        read_record(&self.path, &self.id)
    }
    /// Apply one command and retain the result before returning it.
    pub fn apply(&self, command: &Command, now_ms: u64) -> Result<Computer> {
        let current = self.read()?;
        match apply(&current, command, now_ms)? {
            Applied::Changed(next) => {
                self.commit(current.revision, &next)?;
                Ok(*next)
            }
            Applied::Unchanged => Ok(current),
        }
    }
    pub fn commit(&self, expected_revision: u64, next: &Computer) -> Result<()> {
        let current = self.read()?;
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

fn write_atomic(path: &Path, c: &Computer) -> Result<()> {
    let bytes = serde_json::to_vec(c).map_err(|_| StoreError::Io("Cannot encode the computer."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(StoreError::Io("The computer exceeds its size limit."));
    }
    regular_or_missing(path)?;
    let temp = path.with_extension("writing");
    regular_or_missing(&temp)?;
    let mut file = private_options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temp)
        .map_err(|_| StoreError::Io("Cannot write the computer."))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| StoreError::Io("Cannot retain the computer."))?;
    fs::rename(&temp, path).map_err(|_| StoreError::Io("Cannot replace the computer."))?;
    File::open(path.parent().expect("record has a parent"))
        .and_then(|d| d.sync_all())
        .map_err(|_| StoreError::Io("Cannot sync the computer store."))
}

fn read_record(path: &Path, id: &str) -> Result<Computer> {
    let file = match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => File::open(path),
        Ok(_) => return Err(StoreError::Corrupt("Computer files must be regular files.")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StoreError::NotFound),
        Err(_) => return Err(StoreError::Io("Cannot inspect the computer.")),
    };
    let mut bytes = vec![];
    file.and_then(|f| f.take(MAX_RECORD_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(|_| StoreError::Io("Cannot read the computer."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(StoreError::Corrupt("The computer exceeds its size limit."));
    }
    let c: Computer = serde_json::from_slice(&bytes)
        .map_err(|_| StoreError::Corrupt("Cannot decode the computer."))?;
    if c.id != id {
        return Err(StoreError::Corrupt("Computer identity mismatch."));
    }
    c.validate().map_err(StoreError::Corrupt)?;
    Ok(c)
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
            .map_err(|_| StoreError::Io("Cannot protect the computer store."))?;
    }
    Ok(())
}
fn regular_or_missing(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Ok(()),
        Ok(_) => Err(StoreError::Corrupt("Computer files must be regular files.")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(StoreError::Io("Cannot inspect a computer file.")),
    }
}
