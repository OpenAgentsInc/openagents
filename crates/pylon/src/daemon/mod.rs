//! Pylon daemon infrastructure
//!
//! Provides self-daemonizing capabilities for Pylon:
//! - PID file management at ~/.openagents/pylon/pylon.pid
//! - fork()/setsid() process daemonization
//! - Unix socket IPC for daemon control

mod control;
mod pid;
mod process;

pub use control::{ControlClient, ControlSocket, DaemonCommand, DaemonResponse};
pub use pid::PidFile;
pub use process::{daemonize, is_daemon_running};

use crate::config::PylonConfig;
use std::path::PathBuf;

/// Get the pylon runtime directory (~/.openagents/pylon)
pub fn runtime_dir() -> anyhow::Result<PathBuf> {
    let dir = PylonConfig::pylon_dir()?;
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
