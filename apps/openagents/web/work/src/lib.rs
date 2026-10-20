//! The work panel beside the GPUI web chat (#12225): the account's work
//! runs and the landing queue, its environments and their Claude Code runs,
//! and its Mac jobs, each drawn live from the site's own JSON with the
//! sign-in cookie.
//!
//! It is its own module so the chat's stays under its budget. The chat
//! page's start script loads it the first time the panel opens, puts a
//! `#gpui-work` box on the right for its canvas, and calls [`mount`] with
//! the fonts the chat already fetched. The panel's Close dispatches
//! `oa-work-close` on the window.

pub mod model;

#[cfg(target_family = "wasm")]
mod app;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Starts the panel with the page's fonts, one `Uint8Array` per face.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array) {
    use std::borrow::Cow;
    use std::rc::Rc;
    use std::sync::Arc;

    use gpui::{AppContext, Application, WindowBounds, WindowOptions, px, size};
    use openagents_gpui_web::{FetchHttpClient, WebFonts, WebPlatform};
    use openagents_web_chat_view::{assets, composer, theme};

    console_error_panic_hook::set_once();
    openagents_gpui_web::init_logging();
    let faces: Vec<Cow<'static, [u8]>> = fonts
        .iter()
        .map(|face| Cow::Owned(js_sys::Uint8Array::new(&face).to_vec()))
        .collect();
    let platform = WebPlatform::new(
        false,
        WebFonts {
            fallback_family: theme::SANS,
            faces,
        },
    );
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .with_http_client(Arc::new(FetchHttpClient::new()))
        .run_embedded(move |cx| {
            openagents_web_chat_view::install(cx);
            composer::bind_keys(cx);
            let bounds = gpui::Bounds::centered(None, size(px(480.), px(820.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| app::WorkApp::new(window, cx)),
            );
            if opened.is_err() {
                app::signal("oa-work-failed");
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}
