//! The chat views the web apps share, drawn with GPUI.
//!
//! These are ports of the desktop's views in `apps/openagents/crates/ui`,
//! which links native-only crates; `docs/audits/2026-10-19-gpui-web-chat.md`
//! records what still duplicates the desktop and the split that ends it.

pub mod assets;
pub mod composer;
pub mod markdown;
pub mod theme;
pub mod transcript;

/// Milliseconds since the Unix epoch, from the browser's clock on the web.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(target_family = "wasm")]
mod icon;
#[cfg(not(target_family = "wasm"))]
pub use gpui::svg;
#[cfg(target_family = "wasm")]
pub use icon::svg;
