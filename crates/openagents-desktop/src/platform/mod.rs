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

/// macOS's notifications go through the notification center (#10061).
#[cfg(target_os = "macos")]
pub use crate::mac_notify::{listen_notifications, notify, notify_now};

/// Windows's notifications are toasts (#10062).
#[cfg(windows)]
pub use crate::win_notify::{listen_notifications, notify, notify_now};
