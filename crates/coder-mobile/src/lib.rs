//! Coder's read-only mobile application. Rust owns synchronization and cached
//! evidence; the native shell owns controls, protected keys, and view lifetimes.
#[cfg(target_os = "android")]
mod android;
mod app;
mod cache;
mod computer_hud;
mod ffi;
mod push;
mod render;
mod terminal;
mod verse_app;
mod verse_ffi;

pub use app::{App, Config, Packet, Request};
pub use push::PushConfig;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod connection_tests;

#[cfg(test)]
mod computers_live_tests;
