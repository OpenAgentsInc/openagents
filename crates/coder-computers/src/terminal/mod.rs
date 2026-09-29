//! The terminal screen a Computers client opens on a linked host (NIP-TERM).
//!
//! [`model::Model`] holds the session's phase and a `coder-vt` emulator.
//! [`project::view`] draws it as a Rust Native tree in the amber palette,
//! with an accessory key row. [`session::Session`], behind the `live`
//! feature, opens a shell with NIP-HOST `terminal.open` over the host's
//! current link and follows it with NIP-TERM attach, input, resize, and
//! close. [`exec::run`] runs one command in such a shell and collects
//! its output. The host checks the `terminal` right on every request; this
//! screen only avoids offering a control that cannot work.

pub mod model;
pub mod project;
#[cfg(feature = "live")]
pub mod exec;
#[cfg(feature = "live")]
pub mod screen;
#[cfg(feature = "live")]
pub mod session;

pub use model::{Model, Phase};
pub use project::{AccessoryKey, TerminalIntent, view};
