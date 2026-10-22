//! Approach the shared computer to open the page's GPUI chat.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::Document;

use crate::terminal;
use crate::world_camera::WorldCamera;

/// The global the page's chat reads to offer DeepSeek V4.1 beside its
/// composer: `true` while a ready V4.1 group is online (#12421). The chat
/// draws the choice and sends it as `X-OpenAgents-Pylon-Model`.
const GROUP_READY_GLOBAL: &str = "openagentsPylonGroupReady";
const FLINT_READY_GLOBAL: &str = "openagentsPylonFlintReady";

/// The walking hint is Verse's now, drawn on its canvas (#12459).
pub struct Terminal {
    near: bool,
    /// Verse is in a zone, away from the Grid and its terminal.
    away: bool,
    group: bool,
    flint: bool,
    /// Whether the player stands in the Pylon Bay, for the visit (#12439).
    in_bay: bool,
}

impl Terminal {
    pub fn new(document: &Document) -> Option<Self> {
        let window = document.default_view()?;
        // Only on the Grid homepage: `/`, or `/home` before it moved (#12413).
        let path = window.location().pathname().ok()?;
        if !matches!(path.trim_end_matches('/'), "" | "/home") {
            return None;
        }
        Some(Self {
            near: false,
            away: false,
            group: false,
            flint: false,
            in_bay: false,
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
    /// chat closes.
    pub fn away(&mut self, away: bool) {
        if away == self.away {
            return;
        }
        self.away = away;
        if away {
            self.frame(None, None);
        }
    }

    pub fn frame(&mut self, camera: Option<WorldCamera>, player: Option<[f64; 3]>) {
        let in_bay = !self.away && player.is_some_and(|p| crate::visit::in_bay(p, self.in_bay));
        if in_bay && !self.in_bay {
            crate::visit::send("pylon_bay_entered", "{}");
        }
        self.in_bay = in_bay;
        let near = !self.away
            && camera.is_some()
            && player.is_some_and(|p| terminal::in_reach(p, self.near));
        if near == self.near {
            return;
        }
        self.near = near;
        if let Ok(callback) = js_sys::Reflect::get(&js_sys::global(), &"homeTerminal".into())
            && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
        {
            let _ = callback.call1(&JsValue::UNDEFINED, &JsValue::from_bool(near));
        }
    }
}
