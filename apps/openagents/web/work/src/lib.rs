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

#[cfg(target_family = "wasm")]
thread_local! {
    static PANEL: std::cell::RefCell<Option<(gpui::WeakEntity<app::WorkApp>, gpui::AsyncApp)>> =
        const { std::cell::RefCell::new(None) };
}

/// Starts the panel with the page's fonts, one `Uint8Array` per face, open
/// at `target` when an address names an item (#12332; see
/// [`app::WorkApp::focus`]).
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, target: Option<String>) {
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
                move |window, cx| {
                    cx.new(|cx| {
                        let mut panel = app::WorkApp::new(window, cx);
                        if let Some(target) = &target {
                            panel.focus(target, cx);
                        }
                        panel
                    })
                },
            );
            match opened.and_then(|window| window.entity(cx)) {
                Ok(entity) => {
                    let async_cx = cx.to_async();
                    PANEL.with(|p| *p.borrow_mut() = Some((entity.downgrade(), async_cx)));
                }
                Err(_) => app::signal("oa-work-failed"),
            }
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// Opens the item `target` names in the mounted panel (#12332).
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn focus(target: String) {
    PANEL.with(|p| {
        if let Some((entity, cx)) = p.borrow().as_ref() {
            let _ = entity.update(&mut cx.clone(), |panel, cx| panel.focus(&target, cx));
        }
    });
}
