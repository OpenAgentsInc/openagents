//! Core filesystem traits and types.

use crate::identity::PublicKey;
use crate::types::Timestamp;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

/// Filesystem result type.
pub type FsResult<T> = std::result::Result<T, FsError>;

/// Filesystem error types.
#[derive(Debug)]
pub enum FsError {
    /// Path not found.
    NotFound,
    /// Permission denied.
    PermissionDenied,
    /// Path already exists.
    AlreadyExists,
    /// Expected a directory.
    NotDirectory,
    /// Expected a file.
    IsDirectory,
    /// Invalid path.
    InvalidPath,
    /// Budget exceeded.
    BudgetExceeded,
    /// Underlying IO error.
    Io(std::io::Error),
    /// Other error with message.
    Other(String),
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsError::NotFound => write!(f, "not found"),
            FsError::PermissionDenied => write!(f, "permission denied"),
            FsError::AlreadyExists => write!(f, "already exists"),
            FsError::NotDirectory => write!(f, "not a directory"),
            FsError::IsDirectory => write!(f, "is a directory"),
            FsError::InvalidPath => write!(f, "invalid path"),
            FsError::BudgetExceeded => write!(f, "budget exceeded"),
            FsError::Io(err) => write!(f, "io error: {err}"),
            FsError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for FsError {}

impl From<std::io::Error> for FsError {
    fn from(err: std::io::Error) -> Self {
        FsError::Io(err)
    }
}

/// Flags for opening files.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpenFlags {
    /// Open for reading.
    pub read: bool,
    /// Open for writing.
    pub write: bool,
    /// Create if missing.
    pub create: bool,
    /// Truncate on open.
    pub truncate: bool,
    /// Append on write.
    pub append: bool,
}

impl OpenFlags {
    /// Open read-only.
    pub fn read() -> Self {
        Self {
            read: true,
            ..Default::default()
        }
    }

    /// Open write-only.
    pub fn write() -> Self {
        Self {
            write: true,
            ..Default::default()
        }
    }

    /// Open for read/write.
    pub fn read_write() -> Self {
        Self {
            read: true,
            write: true,
            ..Default::default()
        }
    }

    /// Open for write, creating if missing.
    pub fn create() -> Self {
        Self {
            write: true,
            create: true,
            ..Default::default()
        }
    }
}

/// Seek position.
#[derive(Clone, Copy, Debug)]
pub enum SeekFrom {
    /// Seek from start.
    Start(u64),
    /// Seek from end.
    End(i64),
    /// Seek from current position.
    Current(i64),
}

/// Directory entry metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirEntry {
    /// Entry name.
    pub name: String,
    /// Whether entry is a directory.
    pub is_dir: bool,
    /// Size in bytes.
    pub size: u64,
    /// Modified timestamp.
    pub modified: Option<Timestamp>,
}

impl DirEntry {
    /// Create a file entry.
    pub fn file(name: impl Into<String>, size: u64) -> Self {
        Self {
            name: name.into(),
            is_dir: false,
            size,
            modified: None,
        }
    }

    /// Create a directory entry.
    pub fn dir(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_dir: true,
            size: 0,
            modified: None,
        }
    }
}

/// File permissions.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Permissions {
    /// Read permission.
    pub read: bool,
    /// Write permission.
    pub write: bool,
    /// Execute permission.
    pub execute: bool,
}

impl Permissions {
    /// Read-only permissions.
    pub fn read_only() -> Self {
        Self {
            read: true,
            write: false,
            execute: false,
        }
    }

    /// Read/write permissions.
    pub fn read_write() -> Self {
        Self {
            read: true,
            write: true,
            execute: false,
        }
    }
}

/// File metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stat {
    /// Size in bytes.
    pub size: u64,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Created timestamp.
    pub created: Option<Timestamp>,
    /// Modified timestamp.
    pub modified: Option<Timestamp>,
    /// Permissions.
    pub permissions: Permissions,
}

impl Stat {
    /// Directory stat with defaults.
    pub fn dir() -> Self {
        Self {
            size: 0,
            is_dir: true,
            created: None,
            modified: None,
            permissions: Permissions::read_write(),
        }
    }

    /// File stat with size.
    pub fn file(size: u64) -> Self {
        Self {
            size,
            is_dir: false,
            created: None,
            modified: None,
            permissions: Permissions::read_only(),
        }
    }
}

/// Watch event types.
#[derive(Clone, Debug)]
pub enum WatchEvent {
    /// File modified.
    Modified {
        /// Path that changed.
        path: String,
    },
    /// File created.
    Created {
        /// Path that was created.
        path: String,
    },
    /// File deleted.
    Deleted {
        /// Path that was deleted.
        path: String,
    },
    /// Streaming data.
    Data(Vec<u8>),
}

/// Handle for watching file/directory changes.
pub trait WatchHandle: Send {
    /// Block until next change event (or timeout).
    fn next(&mut self, timeout: Option<Duration>) -> FsResult<Option<WatchEvent>>;

    /// Close the watch.
    fn close(&mut self) -> FsResult<()>;
}

/// An open file handle.
pub trait FileHandle: Send + Sync {
    /// Read bytes from current position.
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize>;

    /// Write bytes at current position.
    fn write(&mut self, buf: &[u8]) -> FsResult<usize>;

    /// Seek to position.
    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64>;

    /// Get current position.
    fn position(&self) -> u64;

    /// Flush any buffered writes.
    fn flush(&mut self) -> FsResult<()>;

    /// Close the handle.
    fn close(&mut self) -> FsResult<()>;
}

/// A capability exposed as a filesystem.
pub trait FileService: Send + Sync {
    /// Open a file or directory at the given path.
    fn open(&self, path: &str, flags: OpenFlags) -> FsResult<Box<dyn FileHandle>>;

    /// List directory contents.
    fn readdir(&self, path: &str) -> FsResult<Vec<DirEntry>>;

    /// Get file/directory metadata.
    fn stat(&self, path: &str) -> FsResult<Stat>;

    /// Create a directory.
    fn mkdir(&self, path: &str) -> FsResult<()>;

    /// Remove a file or directory.
    fn remove(&self, path: &str) -> FsResult<()>;

    /// Rename/move a file.
    fn rename(&self, from: &str, to: &str) -> FsResult<()>;

    /// Watch for changes (optional, returns None if not supported).
    fn watch(&self, path: &str) -> FsResult<Option<Box<dyn WatchHandle>>>;

    /// Service name for debugging.
    fn name(&self) -> &str;
}

/// Static budget policy for budgeted mounts (micro-USD).
#[derive(Clone, Debug)]
pub struct BudgetPolicy {
    /// Maximum spend per tick (micro-USD).
    pub per_tick_usd: u64,
    /// Maximum spend per day (micro-USD).
    pub per_day_usd: u64,
    /// Approval required above this amount (micro-USD).
    pub approval_threshold_usd: u64,
    /// Approvers allowed to authorize.
    pub approvers: Vec<PublicKey>,
}

/// Access level for mounted services.
#[derive(Clone, Debug)]
pub enum AccessLevel {
    /// Read-only access.
    ReadOnly,
    /// Read and write access.
    ReadWrite,
    /// Sign-only (for /identity - can sign, not extract keys).
    SignOnly,
    /// Budgeted access with spending limits.
    Budgeted(BudgetPolicy),
    /// Disabled (mount exists but access denied).
    Disabled,
}

/// A read-only in-memory file handle backed by bytes.
pub struct BytesHandle {
    data: Vec<u8>,
    pos: usize,
}

impl BytesHandle {
    /// Create a handle from bytes.
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, pos: 0 }
    }
}

impl FileHandle for BytesHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let remaining = self.data.len() - self.pos;
        let len = remaining.min(buf.len());
        buf[..len].copy_from_slice(&self.data[self.pos..self.pos + len]);
        self.pos += len;
        Ok(len)
    }

    fn write(&mut self, _buf: &[u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => self.data.len() as i64 + offset,
            SeekFrom::Current(offset) => self.pos as i64 + offset,
        };

        if new_pos < 0 {
            return Err(FsError::InvalidPath);
        }

        self.pos = new_pos as usize;
        Ok(self.pos as u64)
    }

    fn position(&self) -> u64 {
        self.pos as u64
    }

    fn flush(&mut self) -> FsResult<()> {
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

/// A read-only in-memory file handle backed by a string.
pub struct StringHandle {
    inner: BytesHandle,
}

impl StringHandle {
    /// Create a handle from a UTF-8 string.
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            inner: BytesHandle::new(data.into().into_bytes()),
        }
    }
}

impl FileHandle for StringHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        self.inner.read(buf)
    }

    fn write(&mut self, _buf: &[u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn seek(&mut self, pos: SeekFrom) -> FsResult<u64> {
        self.inner.seek(pos)
    }

    fn position(&self) -> u64 {
        self.inner.position()
    }

    fn flush(&mut self) -> FsResult<()> {
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}

/// A stream-backed file handle for streaming data.
pub struct StreamHandle {
    receiver: std::sync::Mutex<std::sync::mpsc::Receiver<Vec<u8>>>,
    pending: Option<Vec<u8>>,
    offset: usize,
}

impl StreamHandle {
    /// Create a stream handle from a receiver.
    pub fn new(receiver: std::sync::mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            receiver: std::sync::Mutex::new(receiver),
            pending: None,
            offset: 0,
        }
    }
}

impl FileHandle for StreamHandle {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if self.pending.is_none() {
            let receiver = self
                .receiver
                .lock()
                .map_err(|_| FsError::Other("receiver lock poisoned".into()))?;
            match receiver.recv() {
                Ok(data) => {
                    self.pending = Some(data);
                    self.offset = 0;
                }
                Err(_) => return Ok(0),
            }
        }

        if let Some(data) = &self.pending {
            let remaining = data.len() - self.offset;
            let len = remaining.min(buf.len());
            buf[..len].copy_from_slice(&data[self.offset..self.offset + len]);
            self.offset += len;

            if self.offset >= data.len() {
                self.pending = None;
            }
            Ok(len)
        } else {
            Ok(0)
        }
    }

    fn write(&mut self, _buf: &[u8]) -> FsResult<usize> {
        Err(FsError::PermissionDenied)
    }

    fn seek(&mut self, _pos: SeekFrom) -> FsResult<u64> {
        Err(FsError::InvalidPath)
    }

    fn position(&self) -> u64 {
        0
    }

    fn flush(&mut self) -> FsResult<()> {
        Ok(())
    }

    fn close(&mut self) -> FsResult<()> {
        Ok(())
    }
}
