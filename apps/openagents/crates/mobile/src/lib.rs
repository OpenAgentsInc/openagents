//! OpenAgents mobile core — the UniFFI surface shared by the iOS and Android apps.
//!
//! - [`client_ffi`]: account, workspace and session state (wraps `openagents-client`).
//! - [`layout`]: analytic transcript layout — markdown → measured display lists
//!   (wraps `openagents-markdown` + `openagents-text`).
//! - [`orb`]: the desktop's voice orb as paintable frames (wraps `openagents-orb`).
//! - [`caption`]: the voice caption's streaming veil (wraps `openagents-veil`).
//! - [`wallet`]: the Wallet screen over the shared Spark wallet
//!   (`crates/spark-wallet`), and the seed rules the identity reads.

#[cfg(all(feature = "account-fixture", not(target_abi = "sim")))]
compile_error!("The account fixture is only available for the iOS simulator.");

uniffi::setup_scaffolding!("openagents_core");

pub mod caption;
mod client_ffi;
pub mod layout;
pub mod orb;
mod push;
pub mod visual;
pub mod wallet;
pub mod wallpaper;

/// Version handshake: the Swift/Kotlin bindings must match the linked library.
#[uniffi::export]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}
