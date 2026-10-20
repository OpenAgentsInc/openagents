//! Settings beside the GPUI web chat (#12288, #12289, #12290): the
//! desktop's pages (General, Appearance, Notifications, Shortcuts, Files,
//! Archived sessions, and Your ID) as the shared Rust Native forms.
//!
//! It is its own module so the chat's stays under its budget. The chat
//! page's start script loads it the first time Settings opens, puts a
//! `#gpui-settings` box over the chat for its canvas, and calls [`mount`]
//! with the fonts the chat already fetched, the page, and what the chat
//! knows (its computers and archived chats, as JSON); later openings call
//! [`show`]. The module tells the page what happened with window events:
//! `oa-settings-changed` (a choice was saved), `oa-settings-page` (another
//! page), `oa-settings-chat` (unarchive or delete a chat), and
//! `oa-settings-close`.

#[cfg(target_family = "wasm")]
mod app;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(target_family = "wasm")]
thread_local! {
    static WINDOW: std::cell::RefCell<Option<(gpui::WeakEntity<app::SettingsApp>, gpui::AsyncApp)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_family = "wasm")]
fn page_of(slug: &str) -> openagents_chat_view::settings_forms::Page {
    let page = openagents_chat_view::settings_forms::Page::from_slug(slug).unwrap_or_default();
    if app::shown(page) {
        page
    } else {
        Default::default()
    }
}

#[cfg(target_family = "wasm")]
fn shared_of(json: &str) -> app::Shared {
    serde_json::from_str(json).unwrap_or_default()
}

/// Starts Settings in the page's `#gpui-settings` box on `page` (a slug),
/// with the page's fonts, one `Uint8Array` per face, and the chat's
/// `shared` JSON.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, page: String, shared: String) {
    use std::borrow::Cow;
    use std::rc::Rc;
    use std::sync::Arc;

    use gpui::{AppContext, Application, WindowBounds, WindowOptions, px, size};
    use openagents_gpui_web::{FetchHttpClient, WebFonts, WebPlatform};
    use openagents_web_chat_view::{assets, theme};

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
    let page = page_of(&page);
    let shared = shared_of(&shared);
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .with_http_client(Arc::new(FetchHttpClient::new()))
        .run_embedded(move |cx| {
            openagents_web_chat_view::install(cx);
            let bounds = gpui::Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                move |_, cx| cx.new(|cx| app::SettingsApp::new(page, shared, cx)),
            );
            if let Ok(entity) = opened.and_then(|window| window.entity(cx)) {
                let async_cx = cx.to_async();
                WINDOW.with(|w| *w.borrow_mut() = Some((entity.downgrade(), async_cx)));
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// Shows `page` again with the chat's `shared` JSON.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(page: String, shared: String) {
    let page = page_of(&page);
    let shared = shared_of(&shared);
    WINDOW.with(|w| {
        if let Some((entity, cx)) = w.borrow().as_ref() {
            let _ = entity.update(&mut cx.clone(), |app, cx| app.open(page, shared, cx));
        }
    });
}
