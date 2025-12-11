//! FileService trait and related types
//!
//! Plan 9-style file services that can be mounted into namespaces.

use crate::error::FsError;

/// Flags for opening files
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenFlags {
    /// Open for reading
    pub read: bool,
    /// Open for writing
    pub write: bool,
    /// Create if doesn't exist
    pub create: bool,
    /// Truncate existing file
    pub truncate: bool,
    /// Append to file
    pub append: bool,
}

impl OpenFlags {
    /// Read-only flags
    pub fn read_only() -> Self {
        Self {
            read: true,
            ..Default::default()
        }
    }

    /// Write-only flags
    pub fn write_only() -> Self {
        Self {
            write: true,
            ..Default::default()
        }
    }

    /// Read-write flags
    pub fn read_write() -> Self {
        Self {
            read: true,
            write: true,
            ..Default::default()
        }
    }
}

/// A directory entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Name of the entry
    pub name: String,
    /// Whether this is a directory
    pub is_dir: bool,
    /// Size in bytes (0 for directories)
    pub size: u64,
}

/// File metadata
#[derive(Debug, Clone)]
pub struct Metadata {
    /// Whether this is a directory
    pub is_dir: bool,
    /// Size in bytes
    pub size: u64,
    /// Last modified timestamp (Unix epoch seconds)
    pub modified: u64,
    /// Whether the file is read-only
    pub readonly: bool,
}

/// Handle to an open file
pub trait FileHandle: Send + Sync {
    /// Read bytes from the file
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError>;

    /// Write bytes to the file
    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError>;

    /// Seek to a position
    fn seek(&mut self, pos: u64) -> Result<(), FsError>;

    /// Get current position
    fn position(&self) -> u64;

    /// Flush any buffered writes
    fn flush(&mut self) -> Result<(), FsError>;
}

/// A Plan 9-style file service that can be mounted into a namespace
///
/// Services can represent:
/// - In-memory filesystems
/// - Task specifications
/// - Workspace files
/// - Capabilities (Nostr, WebSocket, payments)
/// - Logs and metrics
pub trait FileService: Send + Sync {
    /// Open a file at the given path
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError>;

    /// Read directory contents
    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError>;

    /// Get file metadata
    fn stat(&self, path: &str) -> Result<Metadata, FsError>;

    /// Create a directory (optional, default fails)
    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied("mkdir not supported".into()))
    }

    /// Remove a file (optional, default fails)
    fn remove(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied("remove not supported".into()))
    }

    /// Rename/move a file (optional, default fails)
    fn rename(&self, _from: &str, _to: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied("rename not supported".into()))
    }
}
