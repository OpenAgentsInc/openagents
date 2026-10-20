//! Account credentials in a separate module, loaded by start.js when Settings opens them.

#[cfg(target_family = "wasm")]
mod app;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(target_family = "wasm")]
thread_local! {
    static WINDOW: std::cell::RefCell<Option<(gpui::WeakEntity<app::KeysApp>, gpui::AsyncApp)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_family = "wasm")]
fn page_of(slug: &str) -> openagents_chat_view::settings_forms::Page {
    openagents_chat_view::settings_forms::Page::from_slug(slug)
        .filter(|page| openagents_chat_view::settings_forms::keys::is_page(*page))
        .unwrap_or(openagents_chat_view::settings_forms::Page::ApiKeys)
}

/// Start a credentials page in `#gpui-keys` with the chat's fonts.
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
    let signed_in = serde_json::from_str::<serde_json::Value>(&shared)
        .ok()
        .is_some_and(|value| value["signedIn"].as_bool() == Some(true));
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .with_http_client(Arc::new(FetchHttpClient::new()))
        .run_embedded(move |cx| {
            openagents_web_chat_view::install(cx);
            openagents_web_chat_view::composer::bind_keys(cx);
            let bounds = gpui::Bounds::centered(None, size(px(1280.), px(820.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                move |_, cx| cx.new(|cx| app::KeysApp::new(page, signed_in, cx)),
            );
            if let Ok(entity) = opened.and_then(|window| window.entity(cx)) {
                let async_cx = cx.to_async();
                WINDOW.with(|w| *w.borrow_mut() = Some((entity.downgrade(), async_cx)));
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// Show a credentials page again, forgetting any previous key.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(page: String, shared: String) {
    let page = page_of(&page);
    let signed_in = serde_json::from_str::<serde_json::Value>(&shared)
        .ok()
        .is_some_and(|value| value["signedIn"].as_bool() == Some(true));
    WINDOW.with(|w| {
        if let Some((entity, cx)) = w.borrow().as_ref() {
            let _ = entity.update(&mut cx.clone(), |app, cx| app.open(page, signed_in, cx));
        }
    });
}

/// Forget a newly created key when the page closes.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn hide() {
    WINDOW.with(|w| {
        if let Some((entity, cx)) = w.borrow().as_ref() {
            let _ = entity.update(&mut cx.clone(), |app, cx| app.clear(cx));
        }
    });
}
