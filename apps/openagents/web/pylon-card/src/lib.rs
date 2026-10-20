//! The pylon card on openagents.com/pylons: one pylon's details, drawn by
//! GPUI with the shared chat views' theme in the page's `#gpui-mount` box.
//!
//! The page hands the card a pylon as JSON (`pylons_view::Shown`) through
//! [`show`] whenever it opens the card or the pylon changes, and once a
//! second while the card is open so its times stay current. The card draws
//! only the rows `pylons_view::rows` gives for that pylon: a figure the
//! events don't carry has no row.
//!
//! The page calls [`mount`] once the module is ready. `openagents-gpui-web`
//! dispatches `gpui-first-frame` from the canvas when the first frame at
//! the box's size is drawn; the card dispatches `gpui-failed` on the box
//! if it can't open its window.

use std::cell::{Cell, RefCell};

use pylons_view::Shown;

#[cfg(target_family = "wasm")]
mod app;

thread_local! {
    /// The newest pylon the page handed over, and a count of handovers.
    static LATEST: RefCell<Option<Shown>> = const { RefCell::new(None) };
    static VERSION: Cell<u64> = const { Cell::new(0) };
}

/// Takes the page's JSON. Anything that doesn't parse is ignored, and the
/// card keeps what it showed.
pub fn take(json: &str) {
    let Ok(shown) = serde_json::from_str::<Shown>(json) else {
        return;
    };
    LATEST.with(|latest| *latest.borrow_mut() = Some(shown));
    VERSION.with(|version| version.set(version.get() + 1));
}

/// The newest pylon and its handover count.
pub(crate) fn latest() -> (u64, Option<Shown>) {
    (
        VERSION.with(Cell::get),
        LATEST.with(|latest| latest.borrow().clone()),
    )
}

#[cfg(all(target_family = "wasm", feature = "export"))]
use wasm_bindgen::prelude::*;

/// Draws the card in `#gpui-mount`. `still` keeps it from moving, for
/// readers who ask for less.
#[cfg(all(target_family = "wasm", feature = "export"))]
#[wasm_bindgen]
pub fn mount(still: bool) {
    run(still);
}

/// Hands the card a pylon ([`take`]).
#[cfg(all(target_family = "wasm", feature = "export"))]
#[wasm_bindgen]
pub fn show(json: &str) {
    take(json);
}

/// [`mount`], for a build that exports it itself.
#[cfg(target_family = "wasm")]
pub fn run(still: bool) {
    use std::rc::Rc;

    use gpui::{
        AppContext, Application, WindowBackgroundAppearance, WindowBounds, WindowOptions, px, size,
    };
    use openagents_gpui_web::{WebFonts, WebPlatform};
    use openagents_web_chat_view::{assets, theme};

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
            let bounds = gpui::Bounds::centered(None, size(px(400.), px(560.)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                move |window, cx| cx.new(|cx| app::PylonCard::new(still, window, cx)),
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
/// card bundles them.
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

#[cfg(test)]
mod tests {
    #[test]
    fn the_card_takes_the_pages_json_and_ignores_anything_else() {
        let json = r#"{"pylon":{"id":"30200:ab:mac","name":"Mac","slug":"mac","npub":"npub1x",
"status":"online","family":"unified-memory","memory_gb":128,"busy":0,"total":2,"since":10,
"observed_at":20,"expires_at":260,"services":[],"free":true,"pools":[]},"now":25}"#;
        super::take("not json");
        assert_eq!(super::latest(), (0, None));
        super::take(json);
        let (version, shown) = super::latest();
        assert_eq!(version, 1);
        let shown = shown.unwrap();
        assert_eq!(shown.pylon.name, "Mac");
        assert!(!shown.gone);
        // The rows are the shared ones: no jobs, checks, or earnings here.
        let labels: Vec<_> = pylons_view::rows(&shown).iter().map(|r| r.label).collect();
        assert!(!labels.contains(&"Jobs, last 24 h"));
        assert!(!labels.contains(&"Checks"));
        assert!(!labels.contains(&"Model"));
    }
}
