//! Unix socket IPC for Pylon daemon control
//!
//! Provides communication between CLI commands and the running daemon
//! via ~/.openagents/pylon/control.sock

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

/// Commands that can be sent to the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonCommand {
    /// Request status information
    Status,
    /// Request graceful shutdown
    Shutdown,
    /// Ping to check if daemon is responsive
    Ping,
}

/// Responses from the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    /// Status response with daemon info
    Status {
        running: bool,
        uptime_secs: u64,
        provider_active: bool,
        host_active: bool,
        jobs_completed: u64,
        earnings_msats: u64,
    },
    /// Acknowledgement
    Ok,
    /// Pong response to ping
    Pong,
    /// Error response
    Error(String),
}

/// Unix socket server for daemon control
pub struct ControlSocket {
    listener: UnixListener,
    path: PathBuf,
}

impl ControlSocket {
    /// Create a new control socket at the given path
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        // Remove existing socket file if it exists
        if path.exists() {
            std::fs::remove_file(&path)?;
        }

        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;

        Ok(Self { listener, path })
    }

    /// Accept a connection (non-blocking)
    pub fn try_accept(&self) -> Option<ControlConnection> {
        match self.listener.accept() {
            Ok((stream, _)) => Some(ControlConnection { stream }),
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(_) => None,
        }
    }

    /// Get the socket path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for ControlSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A connection to the control socket
pub struct ControlConnection {
    stream: UnixStream,
}

impl ControlConnection {
    /// Read a command from the connection
    pub fn read_command(&mut self) -> anyhow::Result<DaemonCommand> {
        let mut reader = BufReader::new(&mut self.stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let cmd: DaemonCommand = serde_json::from_str(&line)?;
        Ok(cmd)
    }

    /// Write a response to the connection
    pub fn write_response(&mut self, response: &DaemonResponse) -> anyhow::Result<()> {
        let json = serde_json::to_string(response)?;
        writeln!(self.stream, "{}", json)?;
        self.stream.flush()?;
        Ok(())
    }
}

/// Client for connecting to the daemon's control socket
pub struct ControlClient {
    path: PathBuf,
}

impl ControlClient {
    /// Create a new control client
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Send a command and receive a response
    pub fn send(&self, command: DaemonCommand) -> anyhow::Result<DaemonResponse> {
        let mut stream = UnixStream::connect(&self.path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        // Send command
        let json = serde_json::to_string(&command)?;
        writeln!(stream, "{}", json)?;
        stream.flush()?;

        // Read response
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let response: DaemonResponse = serde_json::from_str(&line)?;

        Ok(response)
    }

    /// Check if the daemon is running and responsive
    pub fn ping(&self) -> bool {
        match self.send(DaemonCommand::Ping) {
            Ok(DaemonResponse::Pong) => true,
            _ => false,
        }
    }

    /// Request daemon status
    pub fn status(&self) -> anyhow::Result<DaemonResponse> {
        self.send(DaemonCommand::Status)
    }

    /// Request graceful shutdown
    pub fn shutdown(&self) -> anyhow::Result<DaemonResponse> {
        self.send(DaemonCommand::Shutdown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_command_serialization() {
        let cmd = DaemonCommand::Status;
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: DaemonCommand = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, DaemonCommand::Status));
    }

    #[test]
    fn test_response_serialization() {
        let resp = DaemonResponse::Status {
            running: true,
            uptime_secs: 3600,
            provider_active: true,
            host_active: false,
            jobs_completed: 42,
            earnings_msats: 100000,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: DaemonResponse = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, DaemonResponse::Status { .. }));
    }

    #[test]
    fn test_socket_creation_and_cleanup() {
        let tmp = TempDir::new().unwrap();
        let socket_path = tmp.path().join("test.sock");

        {
            let _socket = ControlSocket::new(socket_path.clone()).unwrap();
            assert!(socket_path.exists());
        }

        // Socket file should be removed on drop
        assert!(!socket_path.exists());
    }
}
