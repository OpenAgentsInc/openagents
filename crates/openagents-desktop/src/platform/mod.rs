//! The computer around the window, one module a system: the login agent
//! that runs `coder host serve`, where the host's control channel is, the
//! screen lock, the clipboard, the folder chooser, notifications, and
//! whether Codex and Claude Code are signed in. macOS is [`crate::mac`]; Linux and Windows
//! are here. Each exposes the same functions, and the rest of the binary
//! calls them through this module.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(any(target_os = "linux", windows)))]
pub use crate::mac::*;

/// Where the window reaches the host's control channel: the Unix socket
/// on macOS and Linux.
#[cfg(not(windows))]
pub fn control_path() -> Option<std::path::PathBuf> {
    openagents_desktop::control::socket_path()
}

/// Desktop notifications are Linux's for now (#10026); elsewhere a notice
/// is dropped.
#[cfg(not(target_os = "linux"))]
pub fn notify(_: openagents_desktop::notices::Notice) {}

/// See [`notify`]: no notification clicks to listen for here.
#[cfg(not(target_os = "linux"))]
pub fn listen_notifications() {}

/// See [`notify`]: no notification service here.
#[cfg(not(target_os = "linux"))]
pub fn notify_now(_: &openagents_desktop::notices::Notice) -> Option<&'static str> {
    None
}
