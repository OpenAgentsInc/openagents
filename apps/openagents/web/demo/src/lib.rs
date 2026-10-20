//! The OpenAgents desktop chat UI in a browser: GPUI drawing into a canvas
//! through `openagents-gpui-web`, showing the desktop's offline demo chats.

mod demo;
mod shims;

#[cfg(target_family = "wasm")]
mod app;
#[cfg(target_family = "wasm")]
mod approval;

pub(crate) use shims::{attachments, client, config, now_ms, rpc};

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Starts the demo with the page's fonts: the shared Geist and Geist Mono
/// files (`openagents_web_chat_view::assets::FONT_FILES`, one `Uint8Array`
/// each, in that order), which the page fetches from `/gpui-fonts/`.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn launch(fonts: js_sys::Array) {
    use std::rc::Rc;

    use gpui::{AppContext, Application, WindowBounds, WindowOptions, px, size};
    use openagents_gpui_web::WebPlatform;
    use openagents_web_chat_view::{assets, composer, theme};

    console_error_panic_hook::set_once();
    openagents_gpui_web::init_logging();
    let platform = WebPlatform::new(
        false,
        WebFonts {
            fallback_family: theme::SANS,
            faces: font_faces(&fonts),
        },
    );
    // The browser owns the run loop: `run` would drop the app as soon as it
    // launched, so hold the embedded handle for the life of the page.
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .run_embedded(|cx| {
            theme::install(cx);
            composer::bind_keys(cx);
            let bounds = gpui::Bounds::centered(None, size(px(1280.), px(820.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| app::ChatApp::new(window, cx)),
            )
            .expect("the page has a canvas to draw into");
            cx.activate(true);
        });
    std::mem::forget(handle);
}

/// The page's shared Geist faces, plus Noto subsets for the CJK and emoji in
/// the demo chats, which only the demo draws.
#[cfg(target_family = "wasm")]
use openagents_gpui_web::WebFonts;

#[cfg(target_family = "wasm")]
fn font_faces(fonts: &js_sys::Array) -> Vec<std::borrow::Cow<'static, [u8]>> {
    WebFonts::from_js(openagents_web_chat_view::theme::SANS, fonts).faces
}
