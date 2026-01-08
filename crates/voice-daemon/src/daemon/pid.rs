//! PID file management for voice-daemon

use std::fs;
use std::path::PathBuf;

use super::runtime_dir;

/// Get the PID file path
fn pid_file() -> PathBuf {
    runtime_dir().join("voice-daemon.pid")
}

/// Write current process PID to file
pub fn write_pid() -> Result<(), String> {
    let pid = std::process::id();
    fs::write(pid_file(), pid.to_string())
        .map_err(|e| format!("Failed to write PID file: {}", e))
}

/// Read PID from file
pub fn read_pid() -> Result<Option<i32>, String> {
    let path = pid_file();

    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read PID file: {}", e))?;

    let pid: i32 = content
        .trim()
        .parse()
        .map_err(|e| format!("Invalid PID in file: {}", e))?;

    Ok(Some(pid))
}

/// Remove PID file
pub fn remove_pid() -> Result<(), String> {
    let path = pid_file();

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to remove PID file: {}", e))?;
    }

    Ok(())
}

/// Check if daemon process is running
pub fn is_running() -> Result<bool, String> {
    let pid = read_pid()?;

    match pid {
        None => Ok(false),
        Some(pid) => {
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;

                // Send signal 0 to check if process exists
                match kill(Pid::from_raw(pid), Signal::SIGCONT) {
                    Ok(()) => Ok(true),
                    Err(nix::errno::Errno::ESRCH) => {
                        // Process doesn't exist, clean up stale PID file
                        let _ = remove_pid();
                        Ok(false)
                    }
                    Err(nix::errno::Errno::EPERM) => {
                        // Process exists but we don't have permission
                        Ok(true)
                    }
                    Err(e) => Err(format!("Failed to check process: {}", e)),
                }
            }

            #[cfg(not(unix))]
            {
                // On non-Unix, just assume it's running if PID file exists
                Ok(true)
            }
        }
    }
}
