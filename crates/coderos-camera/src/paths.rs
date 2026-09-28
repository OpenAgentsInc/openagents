//! Where the daemon keeps its sockets.
//!
//! Both sockets sit under `$XDG_RUNTIME_DIR/coderos-camera/`, the
//! directory a sandboxed client can reach, the way the desk socket does.
//! A host that sets no runtime directory keeps them under
//! `~/.openagents/camera/`, and `CODEROS_CAMERA_DIR` names another
//! directory for a test.

use std::path::PathBuf;

/// The directory the sockets live in.
pub fn runtime_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CODEROS_CAMERA_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir).join("coderos-camera");
    }
    let home = std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(home).join(".openagents/camera")
}

/// The socket that takes the verbs.
pub fn control_socket() -> PathBuf {
    runtime_dir().join("control.sock")
}

/// The socket that carries one line of landmarks a frame.
pub fn hands_socket() -> PathBuf {
    runtime_dir().join("hands.sock")
}
