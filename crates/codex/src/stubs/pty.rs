//! Stub PTY handling
//!
//! This is a simplified stub for PTY operations.
//! For full PTY support, implement proper PTY handling.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::ExitStatus;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// Stub for PTY command session
#[derive(Debug)]
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

    /// Get the underlying file descriptor (stub)
    pub fn get_file_descriptor(&self) -> Option<i32> {
        None
    }

    /// Kill the process
    pub fn kill(&self) -> io::Result<()> {
        Ok(())
    }

    /// Terminate the session
    pub fn terminate(&self) -> io::Result<()> {
        Ok(())
    }

    /// Get writer sender (stub - returns None)
    pub fn writer_sender(&self) -> Option<mpsc::Sender<Vec<u8>>> {
        None
    }

    /// Get output receiver (stub - returns a closed receiver)
    pub fn output_receiver(&self) -> mpsc::Receiver<Vec<u8>> {
        let (_tx, rx) = mpsc::channel(1);
        rx
    }

    /// Check if process has exited
    pub fn has_exited(&self) -> bool {
        true
    }

    /// Get exit code
    pub fn exit_code(&self) -> Option<i32> {
        Some(0)
    }
}

impl Default for ExecCommandSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub for spawned PTY with channels for output and exit notification
pub struct SpawnedPty {
    pub session: ExecCommandSession,
    pub output_rx: mpsc::Receiver<Vec<u8>>,
    pub exit_rx: oneshot::Receiver<Option<ExitStatus>>,
}

impl SpawnedPty {
    /// Create a new stub PTY
    pub fn new() -> Self {
        let (_output_tx, output_rx) = mpsc::channel(1);
        let (_exit_tx, exit_rx) = oneshot::channel();
        Self {
            session: ExecCommandSession::new(),
            output_rx,
            exit_rx,
        }
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

/// Spawn a PTY process (stub - returns error)
pub async fn spawn_pty_process(
    _program: &str,
    _args: &[String],
    _cwd: &Path,
    _env: &HashMap<String, String>,
    _arg0: &Option<String>,
) -> io::Result<SpawnedPty> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "PTY not supported in stub implementation - use non-PTY execution path",
    ))
}
