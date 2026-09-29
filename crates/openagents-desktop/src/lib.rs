//! OpenAgents for Mac.
//!
//! The desktop app a person installs so their phone can connect to this
//! computer by scanning a QR code
//! ([spec](../../../docs/coder/design/2026-09-29-auto-pairing.md)). Its
//! window shows the code (`DSK-01`), the phone that just connected
//! (`DSK-02`), and home: status, phones, and what Coder is doing
//! (`DSK-03`).
//!
//! Two processes, and only one of them ever holds a secret key:
//!
//! - The window (this crate's binary) draws Rust Native views through
//!   `rust-native-desktop` and talks to the host only over the local
//!   control socket ([`control`]). It asks for codes, lists and removes
//!   phones, and sets the project and auto-start policy.
//! - The host, `coder host serve`, runs as a login agent the app registers
//!   with `SMAppService`, and reads its keys from the keychain
//!   ([`keychain`], behind the `keychain` feature). The window never calls
//!   it; the adoption helper ([`migrate`]) runs in its own process.
//!
//! The modules, in the order a reader meets them:
//!
//! - [`control`]: the control protocol client and the [`control::HostControl`] seam.
//! - [`codes`]: when a code shows, rotates, and is cancelled.
//! - [`model`]: the window's state, clicks, and requests.
//! - [`screens`]: the screens as Rust Native views.
//! - [`words`]: the words no screen may show.
//! - [`qr`]: the code's QR modules.
//! - [`fake`]: an in-process host for tests and `--fake-host`.
//! - [`keychain`] and [`migrate`]: the host's keys, and adopting an
//!   old-style setup.

pub mod codes;
pub mod control;
pub mod fake;
#[cfg(feature = "keychain")]
pub mod keychain;
#[cfg(feature = "keychain")]
pub mod migrate;
pub mod model;
pub mod qr;
pub mod screens;
pub mod words;

#[cfg(test)]
mod tests;
