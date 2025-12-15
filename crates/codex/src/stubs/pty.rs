//! Stub PTY handling
//!
//! This is a simplified stub for PTY operations.
//! For full PTY support, implement proper PTY handling.

use std::io;
use std::process::ExitStatus;

/// Stub for PTY command session
pub struct ExecCommandSession {
    _inner: (),
}

impl ExecCommandSession {
    /// Create a new stub session
    pub fn new() -> Self {
        Self { _inner: () }
    }

    /// Write to the session (stub)
    pub fn write(&mut self, _data: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "PTY not supported in stub implementation",
        ))
    }

    /// Read from the session (stub)
    pub fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "PTY not supported in stub implementation",
        ))
    }

    /// Close the session
    pub fn close(self) -> io::Result<ExitStatus> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "PTY not supported in stub implementation",
        ))
    }
}

impl Default for ExecCommandSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub for spawned PTY
pub struct SpawnedPty {
    _inner: (),
}

impl SpawnedPty {
    /// Create a new stub PTY
    pub fn new() -> Self {
        Self { _inner: () }
    }

    /// Spawn is not supported in stub
    pub fn spawn(_cmd: &str, _args: &[&str]) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "PTY not supported in stub implementation",
        ))
    }
}

impl Default for SpawnedPty {
    fn default() -> Self {
        Self::new()
    }
}
