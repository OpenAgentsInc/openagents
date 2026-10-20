//! The signed-in OpenAgents chat in a browser: GPUI drawing into a canvas
//! through `openagents-gpui-web`, reading and writing the account's chats
//! through `openagents-web-client`.
//!
//! The page (`crates/openagents-web/src/pages/gpui_chat.rs`) fetches the
//! fonts beside the module and calls [`launch`] with them, so the module
//! carries no fonts and a return visit reads them from the cache.

mod attachment_chunks;
#[cfg(target_family = "wasm")]
mod attachments;
pub mod check_ins;
#[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
mod complete;
#[cfg(target_family = "wasm")]
mod compose;
pub mod model;
mod notifications;
pub mod palette;
pub mod route;

#[cfg(target_family = "wasm")]
mod app;
#[cfg(target_family = "wasm")]
#[cfg(target_family = "wasm")]
mod work;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(target_family = "wasm")]
thread_local! {
    static CHAT: std::cell::RefCell<Option<(gpui::WindowHandle<app::ChatApp>, gpui::AsyncApp)>> =
        const { std::cell::RefCell::new(None) };
}

/// A palette chord from another canvas, such as the file pane. The keymap
/// stays in Rust; the page only forwards a key the focused canvas did not use.
#[cfg(target_family = "wasm")]
#[wasm_bindgen(js_name = paletteShortcut)]
pub fn palette_shortcut(ctrl: bool, alt: bool, shift: bool, cmd: bool, key: String) -> bool {
    CHAT.with(|chat| {
        chat.borrow().as_ref().is_some_and(|(window, cx)| {
            window
                .update(&mut cx.clone(), |chat, window, cx| {
                    chat.palette_key(ctrl, alt, shift, cmd, &key, window, cx)
                })
                .unwrap_or(false)
        })
    })
}

/// Starts the chat with the page's fonts, one `Uint8Array` per face.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn launch(fonts: js_sys::Array) {
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
    // The browser owns the run loop: `run` would drop the app as soon as it
    // launched, so hold the embedded handle for the life of the page.
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .with_http_client(Arc::new(FetchHttpClient::new()))
        .run_embedded(|cx| {
            openagents_web_chat_view::install(cx);
            composer::bind_keys(cx);
            openagents_web_chat_view::selection::bind_keys(cx);
            let bounds = gpui::Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    |window, cx| cx.new(|cx| app::ChatApp::new(window, cx)),
                )
                .expect("the page has a canvas to draw into");
            CHAT.with(|chat| *chat.borrow_mut() = Some((window, cx.to_async())));
            cx.activate(true);
        });
    std::mem::forget(handle);
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_start_script_fetches_every_face_the_chat_draws() {
        let start = include_str!("../start.js");
        for face in openagents_web_chat_view::assets::FONT_FILES {
            assert!(start.contains(&format!("  \"{face}\",\n")), "{face}");
        }
    }

    #[test]
    fn the_start_script_names_no_upstream() {
        assert!(!include_str!("../start.js").to_lowercase().contains("zeron"));
    }
}
