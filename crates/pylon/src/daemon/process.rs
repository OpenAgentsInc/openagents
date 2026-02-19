//! Process daemonization for Pylon
//!
//! Implements Unix-style fork()/setsid() daemonization.

use nix::sys::stat::Mode;
use nix::unistd::{self, ForkResult};
use std::fs::File;
use std::io;
use std::os::unix::io::AsRawFd;

use super::{PidFile, pid_path};

/// Check if the daemon is already running
pub fn is_daemon_running() -> bool {
    match pid_path() {
        Ok(path) => PidFile::new(path).is_running(),
        Err(_) => false,
    }
}

/// Daemonize the current process
///
/// This performs the classic Unix double-fork:
/// 1. Fork and exit parent
/// 2. Create new session with setsid()
/// 3. Fork again and exit first child
/// 4. Change directory to /
/// 5. Close/redirect file descriptors
/// 6. Write PID file
///
/// Returns Ok(true) in the daemon process, Ok(false) in the parent
pub fn daemonize() -> anyhow::Result<bool> {
    // First fork
    match unsafe { unistd::fork() }? {
        ForkResult::Parent { .. } => {
            // Parent exits immediately
            return Ok(false);
        }
        ForkResult::Child => {
            // Child continues
        }
    }

    // Create new session (detach from terminal)
    unistd::setsid()?;

    // Ignore SIGHUP that would be sent when session leader exits
    unsafe {
        nix::libc::signal(nix::libc::SIGHUP, nix::libc::SIG_IGN);
    }

    // Second fork (prevent reacquiring a terminal)
    match unsafe { unistd::fork() }? {
        ForkResult::Parent { .. } => {
            // First child exits
            std::process::exit(0);
        }
        ForkResult::Child => {
            // Grandchild continues as daemon
        }
    }

    // Change to root directory so we don't hold any mount point
    unistd::chdir("/")?;

    // Set file mode mask
    nix::sys::stat::umask(Mode::empty());

    // Close standard file descriptors and redirect to /dev/null
    redirect_stdio()?;

    // Write PID file
    let pid_file = PidFile::new(pid_path()?);
    pid_file.write()?;

    Ok(true)
}

/// Redirect stdin, stdout, stderr to /dev/null
fn redirect_stdio() -> io::Result<()> {
    let dev_null = File::options().read(true).write(true).open("/dev/null")?;

    let null_fd = dev_null.as_raw_fd();

    // Redirect stdin (fd 0)
    unistd::dup2(null_fd, 0)?;
    // Redirect stdout (fd 1)
    unistd::dup2(null_fd, 1)?;
    // Redirect stderr (fd 2)
    unistd::dup2(null_fd, 2)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_daemon_running_when_not_running() {
        // Clean up any existing PID file first
        if let Ok(path) = pid_path() {
            let _ = std::fs::remove_file(path);
        }
        assert!(!is_daemon_running());
    }
}
