//! Approach the shared computer to open the page's GPUI chat.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, HtmlElement};

use crate::terminal;
use crate::world_camera::WorldCamera;

/// The global the page's chat reads to offer DeepSeek V4.1 beside its
/// composer: `true` while a ready V4.1 group is online (#12421). The chat
/// draws the choice and sends it as `X-OpenAgents-Pylon-Model`.
const GROUP_READY_GLOBAL: &str = "openagentsPylonGroupReady";
const FLINT_READY_GLOBAL: &str = "openagentsPylonFlintReady";

pub struct Terminal {
    hint: HtmlElement,
    near: bool,
    /// Verse is in a zone, away from the Grid and its terminal.
    away: bool,
    group: bool,
    flint: bool,
}

impl Terminal {
    pub fn new(document: &Document) -> Option<Self> {
        let window = document.default_view()?;
        // Only on the Grid homepage: `/`, or `/home` before it moved (#12413).
        let path = window.location().pathname().ok()?;
        if !matches!(path.trim_end_matches('/'), "" | "/home") {
            return None;
        }
        let hint = document
            .create_element("p")
            .ok()?
            .dyn_into::<HtmlElement>()
            .ok()?;
        hint.set_id("grid-controls");
        hint.set_text_content(Some(
            "Click or WASD to walk · Drag to look · Scroll to zoom",
        ));
        hint.style().set_css_text("position:fixed;bottom:20px;left:50%;transform:translateX(-50%);z-index:3;margin:0;padding:8px 12px;background:#080808cc;border-radius:8px;font-size:12px;color:#aaa;white-space:nowrap;pointer-events:none");
        document.body()?.append_child(&hint).ok()?;
        Some(Self {
            hint,
            near: false,
            away: false,
            group: false,
            flint: false,
        })
    }

    /// Whether a ready V4.1 group is online now, for the chat to offer it.
    /// When it leaves, terminal turns go back to the default route.
    pub fn group(&mut self, ready: bool) {
        if ready == self.group {
            return;
        }
        self.group = ready;
        let _ = js_sys::Reflect::set(
            &js_sys::global(),
            &GROUP_READY_GLOBAL.into(),
            &JsValue::from_bool(ready),
        );
    }

    /// Whether a complete Flint model is present, including while its slot is busy.
    pub fn flint(&mut self, ready: bool) {
        if ready == self.flint {
            return;
        }
        self.flint = ready;
        let _ = js_sys::Reflect::set(
            &js_sys::global(),
            &FLINT_READY_GLOBAL.into(),
            &JsValue::from_bool(ready),
        );
    }

    /// Verse left the Grid for a zone (`true`) or came back: away, the
    /// walking hint hides and the chat closes.
    pub fn away(&mut self, away: bool) {
        if away == self.away {
            return;
        }
        self.away = away;
        if away {
            self.frame(None, None);
        }
        self.show_hint();
    }

    fn show_hint(&self) {
        let hidden = self.near || self.away;
        let _ = self
            .hint
            .style()
            .set_property("visibility", if hidden { "hidden" } else { "visible" });
    }

    pub fn frame(&mut self, camera: Option<WorldCamera>, player: Option<[f64; 3]>) {
        let near = !self.away
            && camera.is_some()
            && player.is_some_and(|p| terminal::in_reach(p, self.near));
        if near == self.near {
            return;
        }
        self.near = near;
        self.show_hint();
        if let Ok(callback) = js_sys::Reflect::get(&js_sys::global(), &"homeTerminal".into())
            && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
        {
            let _ = callback.call1(&JsValue::UNDEFINED, &JsValue::from_bool(near));
        }
    }
}
