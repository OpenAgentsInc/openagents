//! Coder's read-only mobile application. Rust owns synchronization and cached
//! evidence; the native shell owns controls, protected keys, and view lifetimes.
#[cfg(target_os = "android")]
mod android;
mod app;
mod chamber;
mod computer_hud;
mod ffi;
mod push;
mod render;
mod studio_panel;
mod verse_app;
mod verse_ffi;
pub mod verse_surface;

pub use app::{App, Config, Packet, Reply, Request};
pub use coder_computers::terminal::screen::TerminalPacket;
pub use push::{Push, PushConfig};
pub use verse_ffi::{BareGym, BarePresence, VerseHandle, blueprint_bytes as verse_blueprint};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod connection_tests;

#[cfg(test)]
mod copy_guard_tests;

#[cfg(test)]
mod computers_live_tests;

#[cfg(test)]
mod terminal_live_tests;
