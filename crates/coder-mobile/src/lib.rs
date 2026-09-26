//! Coder's read-only mobile application. Rust owns synchronization and cached
//! evidence; the native shell owns controls, Keychain, and view lifetimes.
mod app;
mod cache;
mod ffi;
mod render;

pub use app::{App, Config, Packet, Request};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod connection_tests;
