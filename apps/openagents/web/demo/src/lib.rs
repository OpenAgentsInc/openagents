//! The OpenAgents desktop chat UI in a browser: GPUI drawing into a canvas
//! through `openagents-gpui-web`, showing the desktop's offline demo chats.

mod demo;
mod shims;

#[cfg(target_family = "wasm")]
mod app;

pub(crate) use shims::{attachments, client, config, now_ms, rpc};

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(target_family = "wasm")]
#[wasm_bindgen(start)]
pub fn start() {
    use std::rc::Rc;

    use gpui::{AppContext, Application, WindowBounds, WindowOptions, px, size};
    use openagents_gpui_web::{WebFonts, WebPlatform};
    use openagents_web_chat_view::{assets, composer, theme};

    console_error_panic_hook::set_once();
    openagents_gpui_web::init_logging();
    let platform = WebPlatform::new(
        false,
        WebFonts {
            fallback_family: theme::SANS,
            faces: font_faces(),
        },
    );
    // The browser owns the run loop: `run` would drop the app as soon as it
    // launched, so hold the embedded handle for the life of the page.
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .run_embedded(|cx| {
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

/// The shared Geist faces, plus Noto subsets for the CJK and emoji in the
/// demo chats.
#[cfg(target_family = "wasm")]
fn font_faces() -> Vec<std::borrow::Cow<'static, [u8]>> {
    let mut faces = openagents_web_chat_view::assets::bundled_font_faces();
    faces.push(std::borrow::Cow::Borrowed(include_bytes!(
        "../assets/fonts/NotoSansCJKjp-Regular-demo.otf"
    )));
    faces.push(std::borrow::Cow::Borrowed(include_bytes!(
        "../assets/fonts/NotoColorEmoji-demo.ttf"
    )));
    faces
}
