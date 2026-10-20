//! The AgentGraph demo on openagents.com/lander7: a demo chat, drawn by the
//! shared GPUI chat views, at the center of a graph of the parts it uses.
//! As the scripted chat calls a tool, data moves along the link to that
//! part and an answer comes back; when it pays a Pylon for a decision, sats
//! move from the Spark wallet and its balance counts down. The chat
//! composer's text field sits at the bottom; it takes typing and sends
//! nothing. Everything is a demo: nothing is read, paid, or sent.
//!
//! The page calls [`mount`] once the module is ready; the canvas fills the
//! page's `#gpui-mount` box. `openagents-gpui-web` dispatches
//! `gpui-first-frame` from the canvas when the first frame at the box's size
//! is drawn, so the page can fade it in; this module dispatches
//! `gpui-failed` on the box if it can't open its window.

pub mod graph;
pub mod scene;
pub mod script;

mod app;
mod chat;

pub use app::GraphCard;

/// Now, in Unix milliseconds.
pub(crate) fn now_ms() -> i64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(all(target_family = "wasm", feature = "export"))]
use wasm_bindgen::prelude::*;

/// Draws the demo in `#gpui-mount`. `still` shows the finished chat and
/// graph with no motion, for readers who ask for less.
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
            app::bind_keys(cx);
            let bounds = gpui::Bounds::centered(None, size(px(1200.), px(760.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                move |window, cx| cx.new(|cx| app::GraphCard::new(still, window, cx)),
            );
            if opened.is_err() {
                signal("gpui-failed");
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// Tells the page something: an event on `#gpui-mount`.
#[cfg(target_family = "wasm")]
pub(crate) fn signal(name: &str) {
    let mount = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("gpui-mount"));
    if let (Some(mount), Ok(event)) = (mount, web_sys::Event::new(name)) {
        let _ = mount.dispatch_event(&event);
    }
}

/// Geist regular, medium, and semibold, and Geist Mono, as the
/// `/lander6` card bundles them.
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
/// spacing (as `/demo/gpui` and the `/lander6` card split replies).
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
        for beat in super::script::reply() {
            if let super::script::Beat::Text(body) = beat {
                let words = super::split_words(body);
                assert_eq!(words.iter().map(String::as_str).collect::<String>(), body);
            }
        }
    }
}
