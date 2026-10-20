//! The file panel beside the GPUI web chat (#12271, #12272): the open
//! chat's files on the computer it runs on, over host RPC through the
//! device room (`openagents-web-client`'s `computer`).
//!
//! It is its own module so the chat's stays under its budget. The chat
//! page's start script loads it the first time the panel opens, puts a
//! `#gpui-mount` box on the right for its canvas, and calls [`mount`] with
//! the fonts the chat already fetched; when the open chat changes it calls
//! [`show`]. The panel's Close dispatches `oa-files-close` on the window.

pub mod model;

#[cfg(target_family = "wasm")]
mod app;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(target_family = "wasm")]
thread_local! {
    static PANEL: std::cell::RefCell<Option<(gpui::WeakEntity<app::FilesApp>, gpui::AsyncApp)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_family = "wasm")]
fn target(device: String, chat: String) -> Option<app::Target> {
    (!device.is_empty() && !chat.is_empty()).then_some(app::Target { device, chat })
}

/// Starts the panel in the page's `#gpui-mount` box for `device`'s chat
/// `chat`, with the page's fonts, one `Uint8Array` per face.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, device: String, chat: String) {
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
    let first = target(device, chat);
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
                move |window, cx| cx.new(|cx| app::FilesApp::new(first, window, cx)),
            );
            match opened.and_then(|window| window.entity(cx)) {
                Ok(entity) => {
                    let async_cx = cx.to_async();
                    PANEL.with(|panel| *panel.borrow_mut() = Some((entity.downgrade(), async_cx)));
                    app::signal("oa-files-ready");
                }
                Err(_) => app::signal("oa-files-failed"),
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// Shows `device`'s chat `chat` (empty strings: no chat on a computer).
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(device: String, chat: String) {
    let target = target(device, chat);
    PANEL.with(|panel| {
        if let Some((entity, cx)) = panel.borrow().as_ref() {
            let _ = entity.update(&mut cx.clone(), |files, cx| files.show(target, cx));
        }
    });
}

/// Open the file selected in the command palette, optionally at a 1-based line.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn open_file(device: String, chat: String, path: String, line: Option<u32>) {
    PANEL.with(|panel| {
        if let Some((entity, cx)) = panel.borrow().as_ref() {
            let _ = entity.update(&mut cx.clone(), |files, cx| {
                files.show(target(device, chat), cx);
                files.open_at(path, line.map(u64::from), cx);
            });
        }
    });
}
