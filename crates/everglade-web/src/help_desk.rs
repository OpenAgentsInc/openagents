//! The help desk on the Grid homepage (#12443): when the visitor walks up
//! to the counter, a prompt floats over it ("Press E to ask", or "Tap here
//! to ask" on a touch screen). E or a tap on the prompt opens the page's
//! chat with our agent, anchored over the world as the terminal's is, and
//! walking away closes it.
//!
//! The desk itself is plain data in `crates/grid-help-desk`, and the page's
//! chat is the terminal's GPUI chat: the page's `homeDesk(open)` sets
//! `openagentsChatDesk`, which points the chat at the site's free hosted
//! agent rather than the pylons. The visit records the approach here; the
//! page records the open, and the chat each message, without its text.

use std::cell::RefCell;

use glam::{Mat4, Vec3};
use verse::ui::{Atlas, UiBatch};
use wasm_bindgen::{JsCast, JsValue};

/// The page global that is `true` while the chat is open at the desk.
const OPEN_GLOBAL: &str = "openagentsChatDesk";
/// The page's function that opens and closes the chat at the desk.
const OPEN_CALL: &str = "homeDesk";
/// The prompt's padding, CSS pixels.
const PAD: [f32; 2] = [10.0, 6.0];

#[derive(Default)]
struct Desk {
    near: bool,
    /// Where the prompt was last drawn, CSS pixels: x, y, width, height.
    prompt: Option<[f32; 4]>,
    touch: Option<bool>,
}

thread_local! {
    static DESK: RefCell<Desk> = RefCell::default();
}

fn open_now() -> bool {
    js_sys::Reflect::get(&js_sys::global(), &OPEN_GLOBAL.into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn call(open: bool) {
    if let Ok(callback) = js_sys::Reflect::get(&js_sys::global(), &OPEN_CALL.into())
        && let Some(callback) = callback.dyn_ref::<js_sys::Function>()
    {
        let _ = callback.call1(&JsValue::UNDEFINED, &JsValue::from_bool(open));
    }
}

/// Whether the page's pointer is a finger.
fn touch() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(query) = js_sys::Reflect::get(&window, &"matchMedia".into()) else {
        return false;
    };
    let Some(query) = query.dyn_ref::<js_sys::Function>() else {
        return false;
    };
    query
        .call1(&window, &"(pointer: coarse)".into())
        .ok()
        .and_then(|list| js_sys::Reflect::get(&list, &"matches".into()).ok())
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Follows the visitor at `player`; `plaza` is whether they are on the
/// Grid plaza rather than in a zone.
pub fn frame(player: Vec3, plaza: bool) {
    DESK.with(|desk| {
        let mut desk = desk.borrow_mut();
        let at = player.as_dvec3().to_array();
        let near = plaza && grid_help_desk::in_reach(at, desk.near);
        if near && !desk.near {
            crate::visit::send([("help_desk_reached", serde_json::json!({}))]);
        }
        if !near && desk.near && open_now() {
            call(false);
        }
        desk.near = near;
        if !near {
            desk.prompt = None;
        }
    });
}

/// Draws the prompt over the counter while the visitor is in reach and the
/// chat is closed.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4) {
    DESK.with(|desk| {
        let mut desk = desk.borrow_mut();
        desk.prompt = None;
        if !desk.near || open_now() {
            return;
        }
        let touch = *desk.touch.get_or_insert_with(touch);
        let at = Vec3::from_array(grid_help_desk::prompt_at().map(|v| v as f32));
        let Some([x, y]) = verse::hud::project(view_proj, size, at) else {
            return;
        };
        let text = if touch {
            grid_help_desk::PROMPT_TOUCH
        } else {
            grid_help_desk::PROMPT_KEYS
        };
        let w = atlas.measure(text) + PAD[0] * 2.0;
        let h = atlas.line + PAD[1] * 2.0;
        let (left, top) = (x - w / 2.0, y - h);
        ui.rect(
            atlas,
            left,
            top,
            w,
            h,
            crate::theme::linear(coder_ui::coder_noir::SURFACE, 0.92),
        );
        ui.frame(
            atlas,
            left,
            top,
            w,
            h,
            1.0,
            crate::theme::linear(coder_ui::coder_noir::STROKE, 1.0),
        );
        ui.text(
            atlas,
            left + PAD[0],
            top + PAD[1],
            text,
            crate::theme::linear(coder_ui::coder_noir::CONTENT, 1.0),
        );
        desk.prompt = Some([left, top, w, h]);
    });
}

/// A key the world would take: E in reach opens the chat. Returns whether
/// the desk used it.
pub fn key(code: &str) -> bool {
    let near = DESK.with(|desk| desk.borrow().near);
    if near && code == grid_help_desk::KEY && !open_now() {
        call(true);
        return true;
    }
    false
}

/// A press at `at`, CSS pixels: a press on the prompt opens the chat.
/// Returns whether the desk used it.
pub fn press(at: [f32; 2]) -> bool {
    let hit = DESK.with(|desk| {
        desk.borrow().prompt.is_some_and(|[x, y, w, h]| {
            at[0] >= x && at[0] <= x + w && at[1] >= y && at[1] <= y + h
        })
    });
    if hit && !open_now() {
        call(true);
        return true;
    }
    false
}
