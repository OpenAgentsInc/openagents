//! PID file management for Pylon daemon
//!
//! Manages ~/.openagents/pylon/pylon.pid for tracking the running daemon process.

use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

/// Manages a PID file for daemon lifecycle tracking
pub struct PidFile {
    path: PathBuf,
}

impl PidFile {
    /// Create a new PID file manager
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Write the current process PID to the file
    pub fn write(&self) -> io::Result<()> {
        let pid = std::process::id();
        let mut file = fs::File::create(&self.path)?;
        writeln!(file, "{}", pid)?;
        file.sync_all()?;
        Ok(())
    }

    /// Read the PID from the file
    pub fn read(&self) -> io::Result<u32> {
        let content = fs::read_to_string(&self.path)?;
        let pid: u32 = content
            .trim()
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(pid)
    }

    /// Check if a process with the stored PID is running
    pub fn is_running(&self) -> bool {
        match self.read() {
            Ok(pid) => process_exists(pid),
            Err(_) => false,
        }
    }

    /// Remove the PID file
    pub fn remove(&self) -> io::Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    /// Check if the PID file exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }

    /// Get the path to the PID file
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Send a signal to the process
    pub fn send_signal(&self, sig: Signal) -> anyhow::Result<()> {
        let pid = self.read()?;
        signal::kill(Pid::from_raw(pid as i32), sig)?;
        Ok(())
    }

    /// Send SIGTERM for graceful shutdown
    pub fn terminate(&self) -> anyhow::Result<()> {
        self.send_signal(Signal::SIGTERM)
    }

    /// Send SIGKILL for forced shutdown
    pub fn kill(&self) -> anyhow::Result<()> {
        self.send_signal(Signal::SIGKILL)
    }
}

/// Check if a process with the given PID exists
fn process_exists(pid: u32) -> bool {
    // Sending signal 0 checks if process exists without actually sending a signal
    match signal::kill(Pid::from_raw(pid as i32), None) {
        Ok(()) => true,
        Err(nix::errno::Errno::ESRCH) => false, // No such process
        Err(nix::errno::Errno::EPERM) => true,  // Process exists but we can't signal it
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_pid_file_write_read() {
        let tmp = TempDir::new().unwrap();
        let pid_path = tmp.path().join("test.pid");
        let pid_file = PidFile::new(pid_path);

        pid_file.write().unwrap();
        let pid = pid_file.read().unwrap();
        assert_eq!(pid, std::process::id());
    }

    #[test]
    fn test_pid_file_is_running() {
        let tmp = TempDir::new().unwrap();
        let pid_path = tmp.path().join("test.pid");
        let pid_file = PidFile::new(pid_path);

        pid_file.write().unwrap();
        assert!(pid_file.is_running());
    }

    #[test]
    fn test_pid_file_remove() {
        let tmp = TempDir::new().unwrap();
        let pid_path = tmp.path().join("test.pid");
        let pid_file = PidFile::new(pid_path.clone());

        pid_file.write().unwrap();
        assert!(pid_path.exists());

        pid_file.remove().unwrap();
        assert!(!pid_path.exists());
    }

    #[test]
    fn test_nonexistent_pid_not_running() {
        let tmp = TempDir::new().unwrap();
        let pid_path = tmp.path().join("nonexistent.pid");
        let pid_file = PidFile::new(pid_path);

        assert!(!pid_file.is_running());
    }
}
