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

/// Desktop notifications are Linux's and macOS's for now (#10026, #10061);
/// elsewhere a notice is dropped.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn notify(_: openagents_desktop::notices::Notice) {}

/// See [`notify`]: no notification clicks to listen for here.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn listen_notifications() {}

/// See [`notify`]: no notification service here.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn notify_now(_: &openagents_desktop::notices::Notice) -> Option<&'static str> {
    None
}
