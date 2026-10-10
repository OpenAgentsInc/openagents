//! OpenAgents for Mac, Linux, and Windows.
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
//! - The host, `coder host serve --keychain --iroh --control`, runs as a
//!   login agent the app registers with `SMAppService`, and reads its keys
//!   from the keychain itself. Adopting an old-style setup runs the same
//!   `coder` as a child, `coder host adopt` ([`migrate`]), so the keychain
//!   items are written and read by one program and the window never
//!   touches a key.
//!
//! The modules, in the order a reader meets them:
//!
//! - `backdrop`: the Grid, watched live on the Verse page (the `app`
//!   feature; not on Windows).
//! - [`control`]: the control protocol client and the [`control::HostControl`] seam.
//! - [`codes`]: when a code shows, rotates, and is cancelled.
//! - [`model`]: the window's state, clicks, and requests.
//! - [`screens`]: the screens as Rust Native views.
//! - [`settings`]: the Settings pages and the preferences they show.
//! - [`typeface`]: the web's fonts, from the token stacks.
//! - [`words`]: the words no screen may show.
//! - [`preview`]: the screens only a preview build shows.
//! - [`qr`]: the code's QR modules.
//! - [`fake`]: an in-process host for tests and `--fake-host`.
//! - [`folder`]: choosing a folder, and the order Linux tries choosers in.
//! - [`migrate`]: adopting an old-style setup through `coder host adopt`.
//! - [`notices`]: when Coder's work is worth a desktop notification.
//! - `update`: the signed-manifest updater (the `app` feature).

/// Whether this binary is the packaged release. The packaging scripts
/// (`scripts/desktop/package-macos.sh`, `scripts/desktop/package-linux.sh`)
/// build it with `OPENAGENTS_DESKTOP_RELEASE=1` in the environment; every
/// other build, `cargo build --release` included, is a dev build. A dev
/// build keeps its own keychain items (`grid::store::SERVICE`) and never
/// asks `coder` to adopt the keychain, so it can't prompt for, read, or
/// move the signed app's secrets (#10096).
pub const RELEASE: bool = release_flag(option_env!("OPENAGENTS_DESKTOP_RELEASE"));

/// Whether a build-time `OPENAGENTS_DESKTOP_RELEASE` marks the release:
/// exactly `1`.
pub const fn release_flag(value: Option<&str>) -> bool {
    matches!(value, Some(value) if value.len() == 1 && value.as_bytes()[0] == b'1')
}

#[cfg(all(feature = "app", not(windows)))]
pub mod backdrop;
pub mod background_pane;
#[cfg(all(feature = "app", not(windows)))]
pub mod chat_gym;
pub mod chrome;
pub mod codes;
pub mod control;
pub mod fake;
#[cfg(feature = "app")]
pub mod feedback;
pub mod folder;
#[cfg(all(feature = "app", not(windows)))]
pub mod grid;
pub mod map_action;
pub mod migrate;
pub mod model;
pub mod notices;
pub mod preview;
pub mod qr;
#[cfg(feature = "app")]
pub mod route_chat;
#[cfg(feature = "app")]
pub mod route_future;
#[cfg(feature = "app")]
pub mod route_live;
#[cfg(feature = "app")]
pub mod route_map;
#[cfg(feature = "app")]
pub mod route_plugin;
pub mod screens;
pub mod settings;
#[cfg(feature = "app")]
pub mod slide_embeds;
#[cfg(feature = "app")]
pub mod slides;
pub mod terminal_action;
#[cfg(feature = "app")]
pub mod terminal_pane;
pub mod typeface;
#[cfg(feature = "app")]
pub mod update;
pub mod words;

#[cfg(test)]
mod copy_guard_tests;
#[cfg(test)]
mod tests;

#[cfg(feature = "app")]
pub mod chat;
pub mod chat_action;

#[cfg(feature = "app")]
mod chat_images;
