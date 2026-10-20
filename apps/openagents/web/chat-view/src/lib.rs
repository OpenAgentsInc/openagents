//! The chat views the web apps share, drawn with GPUI.
//!
//! The markdown is the desktop's own renderer from `openagents-chat-view`,
//! and [`desktop_transcript`] hosts the desktop's whole transcript, which
//! the chat, the demo, and the landers draw. [`labels`] holds the small
//! pieces the apps draw around it. `docs/audits/2026-10-19-gpui-web-chat.md`
//! records what still duplicates the desktop.

pub mod assets;
pub mod composer;
pub mod desktop_transcript;
pub mod labels;
pub mod markdown;
pub mod message_input;
pub mod selection;
pub mod theme;
mod tool_previews;
pub use tool_previews::read_image_bytes;

/// Install what the desktop's views read: the theme global and, in the
/// browser, the painter for their icons.
pub fn install(cx: &mut gpui::App) {
    theme::install(cx);
    // The transcript's column follows Settings → Appearance (#12289).
    openagents_chat_view::prefs::install_transcript(
        openagents_chat_view::prefs::TranscriptSettings {
            width: |_| labels::content_width(),
            compact_mode: |_| false,
            code_fences_generation: |_| 0,
            type_generation: |_| 0,
        },
        cx,
    );
    #[cfg(target_family = "wasm")]
    openagents_chat_view::icons::set_web_painter(icon::paint);
    #[cfg(target_family = "wasm")]
    openagents_chat_view::icons::set_web_glyph_painter(icon::paint_glyph);
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

#[cfg(all(target_family = "wasm", not(feature = "visuals")))]
pub mod lazy_visuals;
#[cfg(feature = "visuals")]
pub mod visuals;
