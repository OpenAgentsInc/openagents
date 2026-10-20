//! The chat views the web apps share, drawn with GPUI.
//!
//! The markdown is the desktop's own renderer from `openagents-chat-view`;
//! the transcript and composer are still ports of the desktop's views in
//! `apps/openagents/crates/ui`. `docs/audits/2026-10-19-gpui-web-chat.md`
//! records what still duplicates the desktop and the split that ends it.

pub mod assets;
pub mod composer;
pub mod markdown;
pub mod selection;
pub mod theme;
pub mod transcript;

/// Install what the desktop's views read: the theme global and, in the
/// browser, the painter for their icons.
pub fn install(cx: &mut gpui::App) {
    theme::install(cx);
    #[cfg(target_family = "wasm")]
    openagents_chat_view::icons::set_web_painter(icon::paint);
}

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

pub mod visuals;
