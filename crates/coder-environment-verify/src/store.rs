//! Restart-safe verify job store: one private JSON file per job, replaced
//! atomically under a per-record lease, following
//! `coder_environment_setup::store`.

use crate::VerifyJob;
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
    Busy,
    Immutable,
    Corrupt(&'static str),
    Io(&'static str),
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidId => f.write_str("Invalid verify job ID."),
            Self::NotFound => f.write_str("No such verify job."),
            Self::Exists => f.write_str("The verify job already exists."),
            Self::Busy => f.write_str("Another process is changing this verify job."),
            Self::Immutable => f.write_str("Retained verification history cannot change."),
            Self::Corrupt(m) | Self::Io(m) => f.write_str(m),
        }
    }
}
impl std::error::Error for StoreError {}
pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}

/// Exclusive owner lease for one job, held across a whole advance.
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
    /// Read one job. No side effects.
    pub fn read(&self, id: &str) -> Result<VerifyJob> {
        read_record(&self.path(id)?)
    }
    /// Every retained job.
    pub fn list(&self) -> Result<Vec<VerifyJob>> {
        let entries = match fs::read_dir(&self.root) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(_) => return Err(StoreError::Io("Cannot list verify jobs.")),
        };
        let mut jobs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                jobs.push(read_record(&path)?);
            }
        }
        jobs.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(jobs)
    }
    pub fn lease(&self, id: &str) -> Result<Lease> {
        let path = self.path(id)?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
            .create(&self.root)
            .map_err(|_| StoreError::Io("Cannot create the verify store."))?;
        let lock = private_options()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path.with_extension("lock"))
            .map_err(|_| StoreError::Io("Cannot open the verify lock."))?;
        coder_environment::store::lock_waiting(&lock).map_err(|()| StoreError::Busy)?;
        Ok(Lease { path, _lock: lock })
    }
}

impl Lease {
    pub fn exists(&self) -> bool {
        fs::symlink_metadata(&self.path).is_ok()
    }
    pub fn read(&self) -> Result<VerifyJob> {
        read_record(&self.path)
    }
    /// Retain a new job; refuses to replace one.
    pub fn create(&self, job: &VerifyJob) -> Result<()> {
        if self.exists() {
            return Err(StoreError::Exists);
        }
        job.validate().map_err(StoreError::Corrupt)?;
        write_atomic(&self.path, job)
    }
    /// Change the job and retain it before returning it.
    pub fn update(&self, now_ms: u64, f: impl FnOnce(&mut VerifyJob)) -> Result<VerifyJob> {
        let current = self.read()?;
        let mut next = current.clone();
        f(&mut next);
        if next == current {
            return Ok(current);
        }
        next.revision = current.revision + 1;
        next.updated_ms = now_ms.max(current.updated_ms);
        if next.phase != current.phase {
            next.history.push(crate::Transition {
                phase: next.phase,
                at_ms: now_ms,
                reason: match &next.verdict {
                    Some(
                        crate::Verdict::Failed { reason }
                        | crate::Verdict::Incomplete { reason }
                        | crate::Verdict::Cancelled { reason },
                    ) if next.phase == crate::Phase::Cleanup => Some(reason.clone()),
                    _ => None,
                },
            });
        }
        next.validate().map_err(StoreError::Corrupt)?;
        if !current.preserves_history_of(&next) {
            return Err(StoreError::Immutable);
        }
        write_atomic(&self.path, &next)?;
        Ok(next)
    }
}

fn write_atomic(path: &Path, job: &VerifyJob) -> Result<()> {
    let bytes =
        serde_json::to_vec(job).map_err(|_| StoreError::Io("Cannot encode the verify job."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(StoreError::Io("The verify job exceeds its size limit."));
    }
    let temp = path.with_extension("writing");
    let mut file = private_options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&temp)
        .map_err(|_| StoreError::Io("Cannot write the verify job."))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| StoreError::Io("Cannot retain the verify job."))?;
    fs::rename(&temp, path).map_err(|_| StoreError::Io("Cannot replace the verify job."))?;
    File::open(path.parent().expect("record has a parent"))
        .and_then(|d| d.sync_all())
        .map_err(|_| StoreError::Io("Cannot sync the verify store."))
}

fn read_record(path: &Path) -> Result<VerifyJob> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StoreError::NotFound),
        Err(_) => return Err(StoreError::Io("Cannot read the verify job.")),
    };
    let mut bytes = Vec::new();
    Read::take(&mut file, MAX_RECORD_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| StoreError::Io("Cannot read the verify job."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(StoreError::Corrupt(
            "The verify job exceeds its size limit.",
        ));
    }
    let job: VerifyJob = serde_json::from_slice(&bytes)
        .map_err(|_| StoreError::Corrupt("The verify job does not decode."))?;
    job.validate().map_err(StoreError::Corrupt)?;
    Ok(job)
}
