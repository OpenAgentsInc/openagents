//! Approach the shared computer to open the page's GPUI chat.

use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, HtmlElement};

use crate::terminal::{self, GROUP_MODEL, PICK_OFF, PICK_ON};
use crate::world_camera::WorldCamera;

/// The global the page's chat reads before it sends a terminal turn: when it
/// names a model, the chat sends it as `X-OpenAgents-Pylon-Model` (#12416).
const MODEL_GLOBAL: &str = "openagentsPylonModel";

pub struct Terminal {
    hint: HtmlElement,
    /// Shown at the terminal while a ready V4.1 group is online.
    pick: HtmlElement,
    near: bool,
    group: bool,
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
        let pick = document
            .create_element("button")
            .ok()?
            .dyn_into::<HtmlElement>()
            .ok()?;
        pick.set_id("grid-pick-model");
        let _ = pick.set_attribute("type", "button");
        let _ = pick.set_attribute("aria-pressed", "false");
        pick.set_text_content(Some(PICK_OFF));
        pick.style().set_css_text("position:fixed;bottom:20px;left:50%;transform:translateX(-50%);z-index:4;margin:0;padding:8px 12px;background:#080808e6;border:1px solid #444;border-radius:8px;font:inherit;font-size:12px;color:#ddd;white-space:nowrap;cursor:pointer;visibility:hidden");
        {
            let button = pick.clone();
            let toggle = Closure::<dyn FnMut()>::new(move || {
                let on = button.get_attribute("aria-pressed").as_deref() != Some("true");
                choose(&button, on);
            });
            pick.set_onclick(Some(toggle.as_ref().unchecked_ref()));
            toggle.forget();
        }
        document.body()?.append_child(&pick).ok()?;
        Some(Self {
            hint,
            pick,
            near: false,
            group: false,
        })
    }

    /// Whether a ready V4.1 group is online now. When it leaves, terminal
    /// turns go back to the default route.
    pub fn group(&mut self, ready: bool) {
        if ready == self.group {
            return;
        }
        self.group = ready;
        if !ready {
            choose(&self.pick, false);
        }
        self.show_pick();
    }

    fn show_pick(&self) {
        let _ = self.pick.style().set_property(
            "visibility",
            if self.near && self.group {
                "visible"
            } else {
                "hidden"
            },
        );
    }

    pub fn frame(&mut self, camera: Option<WorldCamera>, player: Option<[f64; 3]>) {
        let near = camera.is_some() && player.is_some_and(|p| terminal::in_reach(p, self.near));
        if near == self.near {
            return;
        }
        self.near = near;
        let _ = self
            .hint
            .style()
            .set_property("visibility", if near { "hidden" } else { "visible" });
        self.show_pick();
        if let Ok(callback) = js_sys::Reflect::get(&js_sys::global(), &"homeTerminal".into())
            && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
        {
            let _ = callback.call1(&JsValue::UNDEFINED, &JsValue::from_bool(near));
        }
    }
}

/// Turn asking for the V4.1 group on or off for the next terminal turns.
fn choose(button: &HtmlElement, on: bool) {
    let _ = button.set_attribute("aria-pressed", if on { "true" } else { "false" });
    button.set_text_content(Some(if on { PICK_ON } else { PICK_OFF }));
    let value = if on {
        JsValue::from_str(GROUP_MODEL)
    } else {
        JsValue::UNDEFINED
    };
    let _ = js_sys::Reflect::set(&js_sys::global(), &MODEL_GLOBAL.into(), &value);
}
