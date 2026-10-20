//! The host the GPUI web app's account pages share (#12329, #12330,
//! #12331).
//!
//! A page module's `mount` calls [`mount`] with the fonts the chat fetched
//! and the page's path (such as `traces/ID`); the page's start script puts
//! the module's box over the chat. The host draws a header with Back to
//! chat (`oa-page-close`), the page's view from
//! `openagents_chat_view::settings_forms`, a notice line, and, while the
//! page asks for one ([`Prompt`]), a text field. Every activation reaches
//! the page as an [`Intent`]. When the page moves within itself it tells
//! the start script (`oa-page-path`), which keeps the address current.

#[cfg(target_family = "wasm")]
mod host;

#[cfg(target_family = "wasm")]
pub use host::{Host, Page, Prompt, hide, mount, show};

pub use openagents_chat_view::settings_forms::{Intent, Load, Paints};

/// A refusal or a failed read in the person's words.
pub fn plain(error: &anyhow::Error) -> String {
    let text = error.to_string();
    if text.is_empty() {
        "Something went wrong. Try again.".into()
    } else {
        text
    }
}
