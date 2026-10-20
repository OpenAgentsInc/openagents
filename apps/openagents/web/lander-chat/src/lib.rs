//! The chat on openagents.com/lander8: the composer at the bottom of the
//! page, a short welcome line over it before the first message, and the
//! conversation in a card that grows as it goes, drawn by the shared GPUI
//! chat views over the page's Grid.
//!
//! It is a client of the site's chat API (`openagents-web-client`): a
//! message starts or continues a chat with `POST /v1/threads` or
//! `/v1/threads/{id}/messages`, its answer streams in from
//! `/v1/threads/{id}/events`, and Stop is `/v1/threads/{id}/stop`, all
//! signed in by the site's cookie. Signed out, the site answers as it does
//! for the apps' signed-out visitors until it asks the person to sign in;
//! the chat then keeps what they typed and comes back here after. The open
//! chat's address is `/lander8/chat/{id}`, so a reload shows it again.
//!
//! The page calls [`mount`] once the module is ready; the canvas fills the
//! page's `#gpui-mount` box. `openagents-gpui-web` dispatches
//! `gpui-first-frame` from the canvas when the first frame at the box's size
//! is drawn, so the page can fade it in; this module dispatches
//! `gpui-failed` on the box if it can't open its window.

pub mod model;

#[cfg(target_family = "wasm")]
mod app;

#[cfg(all(target_family = "wasm", feature = "export"))]
use wasm_bindgen::prelude::*;

/// Draws the chat in `#gpui-mount`. `still` leaves out the text's fade-in,
/// for readers who ask for less motion.
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
    // The browser owns the run loop: hold the embedded handle for the life
    // of the page.
    let handle = Application::with_platform(Rc::new(platform))
        .with_assets(assets::Assets)
        .run_embedded(move |cx| {
            openagents_web_chat_view::install(cx);
            composer::bind_keys(cx);
            let bounds = gpui::Bounds::centered(None, size(px(1200.), px(760.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                move |window, cx| cx.new(|cx| app::LanderChat::new(still, window, cx)),
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

/// Geist regular, medium, and semibold, and Geist Mono, as the `/lander6`
/// and `/lander7` cards bundle them.
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
