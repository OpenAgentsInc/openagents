//! Daemon infrastructure for voice-daemon
//!
//! Handles daemonization, PID files, and control socket.

mod pid;

use std::path::PathBuf;

/// Get the runtime directory for voice-daemon
pub fn runtime_dir() -> PathBuf {
    dirs::home_dir()
        .expect("No home directory")
        .join(".openagents")
        .join("voice")
}

/// Start the daemon (fork and daemonize)
pub fn start() -> Result<(), String> {
    let runtime = runtime_dir();
    std::fs::create_dir_all(&runtime)
        .map_err(|e| format!("Failed to create runtime directory: {}", e))?;

    // Check if already running
    if pid::is_running()? {
        return Err("Daemon is already running".to_string());
    }

    // Fork and daemonize
    #[cfg(unix)]
    {
        use nix::unistd::{fork, ForkResult, setsid};

        // First fork
        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => {
                // Parent exits
                return Ok(());
            }
            Ok(ForkResult::Child) => {
                // Child continues
            }
            Err(e) => {
                return Err(format!("Fork failed: {}", e));
            }
        }

        // Create new session
        setsid().map_err(|e| format!("setsid failed: {}", e))?;

        // Second fork to prevent terminal acquisition
        match unsafe { fork() } {
            Ok(ForkResult::Parent { .. }) => {
                // First child exits
                std::process::exit(0);
            }
            Ok(ForkResult::Child) => {
                // Grandchild continues as daemon
            }
            Err(e) => {
                return Err(format!("Second fork failed: {}", e));
            }
        }

        // Write PID file
        pid::write_pid()?;

        // Redirect stdio to /dev/null
        use std::fs::File;
        use std::os::unix::io::AsRawFd;
        let devnull = File::open("/dev/null").map_err(|e| e.to_string())?;
        let fd = devnull.as_raw_fd();
        unsafe {
            nix::libc::dup2(fd, 0); // stdin
            nix::libc::dup2(fd, 1); // stdout
            nix::libc::dup2(fd, 2); // stderr
        }

        // Run the actual daemon
        if let Err(e) = crate::app::run_foreground() {
            tracing::error!("Daemon error: {}", e);
        }

        // Clean up
        let _ = pid::remove_pid();
        std::process::exit(0);
    }

    #[cfg(not(unix))]
    {
        Err("Daemonization not supported on this platform".to_string())
    }
}

/// Stop the running daemon
pub fn stop() -> Result<(), String> {
    let pid = pid::read_pid()?;

    if let Some(pid) = pid {
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            kill(Pid::from_raw(pid), Signal::SIGTERM)
                .map_err(|e| format!("Failed to send signal: {}", e))?;

            // Wait a bit for graceful shutdown
            std::thread::sleep(std::time::Duration::from_millis(500));

            // Remove PID file
            pid::remove_pid()?;

            Ok(())
        }

        #[cfg(not(unix))]
        {
            Err("Stop not supported on this platform".to_string())
        }
    } else {
        Err("Daemon is not running".to_string())
    }
}

/// Check if daemon is running
pub fn status() -> Result<bool, String> {
    pid::is_running()
}
