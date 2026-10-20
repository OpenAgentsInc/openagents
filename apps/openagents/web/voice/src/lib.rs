//! The voice call stage as its own page module: the chat's Voice button
//! opens it (`oa-page-open` "voice"), each open starts a call, and ending
//! the call closes it (`oa-page-close`).

// The browser client (`openagents_web_client::web`) is wasm only.
#[cfg(target_family = "wasm")]
pub mod stage;

/// The page name the start script loads this module under.
pub const NAME: &str = "voice";

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Sends `name` to the page's start script.
#[cfg(target_family = "wasm")]
pub fn signal(name: &str, detail: &str) {
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&wasm_bindgen::JsValue::from_str(detail));
    if let Some(window) = web_sys::window()
        && let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(name, &init)
    {
        let _ = window.dispatch_event(&event);
    }
}

#[cfg(target_family = "wasm")]
thread_local! {
    static PAGE: std::cell::RefCell<Option<(gpui::WeakEntity<stage::Stage>, gpui::AsyncApp)>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, _path: String) {
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
                |_, cx| {
                    cx.new(|cx| {
                        let mut stage = stage::Stage::new(cx);
                        stage.start(None, cx);
                        stage
                    })
                },
            );
            if let Ok(entity) = opened.and_then(|window| window.entity(cx)) {
                let async_cx = cx.to_async();
                PAGE.with(|slot| *slot.borrow_mut() = Some((entity.downgrade(), async_cx)));
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// A later open: start a new call on the mounted stage.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(_path: String) {
    PAGE.with(|slot| {
        if let Some((entity, cx)) = slot.borrow().as_ref() {
            let _ = entity.update(&mut cx.clone(), |stage, cx| stage.start(None, cx));
        }
    });
}
