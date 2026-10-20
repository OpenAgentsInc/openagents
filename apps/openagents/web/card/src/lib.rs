//! The agent demo card on openagents.com/lander6: the desktop demo's
//! scripted reply, streamed through the shared GPUI chat views into the
//! page's `#gpui-mount` box, over and over.
//!
//! The page calls [`mount`] once the box is ready. `openagents-gpui-web`
//! dispatches `gpui-first-frame` from the canvas when the card's first frame
//! at the box's size is drawn, so the page can fade it in; the card
//! dispatches `gpui-failed` on the box if it can't open its window.

// The demo's files compile for the browser only: their own tests need the
// desktop's visual engine, which the web workspace doesn't build.
#[cfg(target_family = "wasm")]
mod demo;

#[cfg(target_family = "wasm")]
mod app;

#[cfg(target_family = "wasm")]
pub(crate) use openagents_web_chat_view::now_ms;

/// The two `openagents-client` items the demo transcripts name.
#[cfg(target_family = "wasm")]
pub(crate) mod attachments {
    /// `openagents_client::attachments::with_attachments`; the card's
    /// script attaches nothing.
    pub fn with_attachments(text: &str, _paths: &[String]) -> String {
        text.to_owned()
    }
}

#[cfg(all(target_family = "wasm", feature = "export"))]
use wasm_bindgen::prelude::*;

/// Draws the card in `#gpui-mount`. `still` shows the finished reply with
/// no motion, for readers who ask for less.
#[cfg(all(target_family = "wasm", feature = "export"))]
#[wasm_bindgen]
pub fn mount(still: bool) {
    run(still);
}

/// [`mount`], for a build that exports it itself.
#[cfg(target_family = "wasm")]
pub fn run(still: bool) {
    use std::rc::Rc;

    use gpui::{
        AppContext, Application, WindowBackgroundAppearance, WindowBounds, WindowOptions, px, size,
    };
    use openagents_gpui_web::{WebFonts, WebPlatform};
    use openagents_web_chat_view::{assets, theme};

    console_error_panic_hook::set_once();
    openagents_gpui_web::init_logging();
    let platform = WebPlatform::new(
        false,
        WebFonts {
            fallback_family: theme::SANS,
            faces: font_faces(),
        },
    );
    // The browser owns the run loop: hold the embedded handle for the life
    // of the page.
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .run_embedded(move |cx| {
            openagents_web_chat_view::install(cx);
            let bounds = gpui::Bounds::centered(None, size(px(640.), px(420.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                move |window, cx| cx.new(|cx| app::CardApp::new(still, window, cx)),
            );
            if opened.is_err() {
                app::signal("gpui-failed");
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// Geist regular, medium, and semibold, and Geist Mono: the faces the card
/// draws. The italics and bolds fall back to these.
#[cfg(target_family = "wasm")]
fn font_faces() -> Vec<std::borrow::Cow<'static, [u8]>> {
    let faces: [&'static [u8]; 4] = [
        include_bytes!("../../../crates/ui/assets/fonts/Geist.ttf"),
        include_bytes!("../../../crates/ui/assets/fonts/Geist-Medium.ttf"),
        include_bytes!("../../../crates/ui/assets/fonts/Geist-SemiBold.ttf"),
        include_bytes!("../../../crates/ui/assets/fonts/GeistMono.ttf"),
    ];
    faces.into_iter().map(std::borrow::Cow::Borrowed).collect()
}

/// Words with the spaces before them, so each streamed append keeps the
/// spacing (as the demo at /demo/gpui splits its replies).
pub fn split_words(body: &str) -> std::collections::VecDeque<String> {
    let mut out = std::collections::VecDeque::new();
    let mut current = String::new();
    let mut in_space = false;
    for ch in body.chars() {
        let space = ch.is_whitespace();
        if !space && in_space {
            out.push_back(std::mem::take(&mut current));
        }
        in_space = space;
        current.push(ch);
    }
    if !current.is_empty() {
        out.push_back(current);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn streamed_words_rebuild_the_reply_exactly() {
        let body = "Here's how it renders:\n\n- one  two\n```rust\nconst A: u8 = 1;\n```";
        let words = super::split_words(body);
        assert_eq!(words.iter().map(String::as_str).collect::<String>(), body);
        assert_eq!(words[0], "Here's ");
        assert!(
            words
                .iter()
                .skip(1)
                .all(|word| !word.starts_with(char::is_whitespace))
        );
    }
}
