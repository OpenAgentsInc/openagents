//! Approach the shared computer to open the page's GPUI chat.

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, HtmlElement};

use crate::{terminal, world_camera::WorldCamera};

pub struct Terminal {
    hint: HtmlElement,
    near: bool,
}

impl Terminal {
    pub fn new(document: &Document) -> Option<Self> {
        let window = document.default_view()?;
        if window.location().pathname().ok()?.trim_end_matches('/') != "/home" {
            return None;
        }
        let hint = document
            .create_element("p")
            .ok()?
            .dyn_into::<HtmlElement>()
            .ok()?;
        hint.set_id("grid-controls");
        hint.set_text_content(Some("WASD to walk · Drag to look"));
        hint.style().set_css_text("position:fixed;bottom:20px;left:50%;transform:translateX(-50%);z-index:3;margin:0;padding:8px 12px;background:#080808cc;border-radius:8px;font-size:12px;color:#aaa;white-space:nowrap;pointer-events:none");
        document.body()?.append_child(&hint).ok()?;
        Some(Self { hint, near: false })
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
        if let Ok(callback) = js_sys::Reflect::get(&js_sys::global(), &"homeTerminal".into())
            && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
        {
            let _ = callback.call1(&JsValue::UNDEFINED, &JsValue::from_bool(near));
        }
    }
}
