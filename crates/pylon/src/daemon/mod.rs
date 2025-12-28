//! Pylon daemon infrastructure
//!
//! Provides self-daemonizing capabilities for Pylon:
//! - PID file management at ~/.pylon/pylon.pid
//! - fork()/setsid() process daemonization
//! - Unix socket IPC for daemon control

mod control;
mod pid;
mod process;

pub use control::{ControlClient, ControlSocket, DaemonCommand, DaemonResponse};
pub use pid::PidFile;
pub use process::{daemonize, is_daemon_running};

use std::path::PathBuf;

/// Get the pylon runtime directory (~/.pylon)
pub fn runtime_dir() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let dir = home.join(".pylon");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Get the PID file path
pub fn pid_path() -> anyhow::Result<PathBuf> {
    Ok(runtime_dir()?.join("pylon.pid"))
}

/// Get the control socket path
pub fn socket_path() -> anyhow::Result<PathBuf> {
    Ok(runtime_dir()?.join("control.sock"))
}

/// Get the database path
pub fn db_path() -> anyhow::Result<PathBuf> {
    Ok(runtime_dir()?.join("pylon.db"))
}
