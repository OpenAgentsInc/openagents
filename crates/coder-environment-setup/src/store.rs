//! Restart-safe setup session store: one private JSON file per session,
//! replaced atomically under a per-record lease and a revision fence,
//! following `coder_environment::store`.

use crate::{Op, Refusal, SetupSession, apply};
use coder_environment::valid_id;
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    InvalidId,
    NotFound,
    Exists,
    Busy,
    Fence { expected: u64, current: u64 },
    Immutable,
    Corrupt(&'static str),
    Io(&'static str),
    Refused(Refusal),
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId => f.write_str("Invalid setup session ID."),
            Self::NotFound => f.write_str("No such setup session."),
            Self::Exists => f.write_str("The setup session already exists."),
            Self::Busy => f.write_str("Another process is changing this setup session."),
            Self::Fence { expected, current } => write!(
                f,
                "The setup session is at revision {current}, not the expected {expected}."
            ),
            Self::Immutable => f.write_str("Retained setup history cannot change."),
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

/// Exclusive owner lease for one session, held across a whole tool call.
pub struct Lease {
    path: PathBuf,
    _lock: File,
}

fn private_options() -> OpenOptions {
    let mut o = OpenOptions::new();
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    o
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
    /// Read one session. No side effects.
    pub fn read(&self, id: &str) -> Result<SetupSession> {
        read_record(&self.path(id)?)
    }
    /// Every retained session, oldest first. No side effects: a missing
    /// store is empty and nothing is created.
    pub fn list(&self) -> Result<Vec<SetupSession>> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(_) => return Err(StoreError::Io("Cannot list setup sessions.")),
        };
        let mut rows = vec![];
        for entry in entries {
            let path = entry
                .map_err(|_| StoreError::Io("Cannot read the setup store."))?
                .path();
            if path.extension().and_then(|v| v.to_str()) == Some("json") {
                rows.push(read_record(&path)?);
            }
        }
        rows.sort_by(|a, b| (a.created_ms, &a.id).cmp(&(b.created_ms, &b.id)));
        Ok(rows)
    }
    pub fn lease(&self, id: &str) -> Result<Lease> {
        let path = self.path(id)?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
            .create(&self.root)
            .map_err(|_| StoreError::Io("Cannot create the setup store."))?;
        let lock = private_options()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path.with_extension("lock"))
            .map_err(|_| StoreError::Io("Cannot open the setup lock."))?;
        coder_environment::store::lock_waiting(&lock).map_err(|()| StoreError::Busy)?;
        Ok(Lease { path, _lock: lock })
    }
}

impl Lease {
    pub fn exists(&self) -> bool {
        fs::symlink_metadata(&self.path).is_ok()
    }
    pub fn read(&self) -> Result<SetupSession> {
        read_record(&self.path)
    }
    /// Retain a new session; refuses to replace one.
    pub fn create(&self, session: &SetupSession) -> Result<()> {
        if self.exists() {
            return Err(StoreError::Exists);
        }
        session.validate().map_err(StoreError::Corrupt)?;
        write_atomic(&self.path, session)
    }
    /// Apply one operation and retain the result before returning it.
    pub fn apply(&self, op: &Op, now_ms: u64) -> Result<SetupSession> {
        let current = self.read()?;
        match apply(&current, op, now_ms)? {
            Some(next) => {
                self.commit(current.revision, &next)?;
                Ok(next)
            }
            None => Ok(current),
        }
    }
    pub fn commit(&self, expected_revision: u64, next: &SetupSession) -> Result<()> {
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

fn write_atomic(path: &Path, session: &SetupSession) -> Result<()> {
    let bytes = serde_json::to_vec(session)
        .map_err(|_| StoreError::Io("Cannot encode the setup session."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(StoreError::Io("The setup session exceeds its size limit."));
    }
    let temp = path.with_extension("writing");
    let mut file = private_options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temp)
        .map_err(|_| StoreError::Io("Cannot write the setup session."))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| StoreError::Io("Cannot retain the setup session."))?;
    fs::rename(&temp, path).map_err(|_| StoreError::Io("Cannot replace the setup session."))?;
    File::open(path.parent().expect("record has a parent"))
        .and_then(|d| d.sync_all())
        .map_err(|_| StoreError::Io("Cannot sync the setup store."))
}

fn read_record(path: &Path) -> Result<SetupSession> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StoreError::NotFound),
        Err(_) => return Err(StoreError::Io("Cannot read the setup session.")),
    };
    let mut bytes = Vec::new();
    Read::take(&mut file, MAX_RECORD_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| StoreError::Io("Cannot read the setup session."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(StoreError::Corrupt(
            "The setup session exceeds its size limit.",
        ));
    }
    let session: SetupSession = serde_json::from_slice(&bytes)
        .map_err(|_| StoreError::Corrupt("The setup session does not decode."))?;
    session.validate().map_err(StoreError::Corrupt)?;
    Ok(session)
}
