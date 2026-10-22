//! Game Lab's arch on the Grid homepage (#12459): when the visitor walks
//! under the GAME LAB arch, a prompt floats in it ("Press E to open Game
//! Lab", or "Tap here to open Game Lab" on a touch screen). E or a tap on
//! the prompt opens Game Lab's panel, which the page's GPUI chat draws, and
//! walking away closes it. Nothing on the page opens Game Lab any other
//! way, apart from the `?lab=game` address.
//!
//! The page's `homeGameLab(open)` sets `openagentsGameLabOpen`, which the
//! chat reads; the visit's Game Lab events come from the chat.

use std::cell::RefCell;

use glam::{Mat4, Vec3};
use verse::ui::{Atlas, UiBatch};
use verse::zones::game_place;
use wasm_bindgen::{JsCast, JsValue};

/// The page global that is `true` while Game Lab's panel is open or
/// opening.
const OPEN_GLOBAL: &str = "openagentsGameLabOpen";
/// The page's function that opens and closes Game Lab.
const OPEN_CALL: &str = "homeGameLab";
/// The key that opens it, as the help desk's.
const KEY: &str = "KeyE";
pub const PROMPT_KEYS: &str = "Press E to open Game Lab";
pub const PROMPT_TOUCH: &str = "Tap here to open Game Lab";
/// Where the prompt floats: in the arch's opening, under its sign, m.
const PROMPT_LIFT: f32 = 2.6;
/// The prompt's padding, CSS pixels.
const PAD: [f32; 2] = [10.0, 6.0];

#[derive(Default)]
struct Arch {
    near: bool,
    /// Where the prompt was last drawn, CSS pixels: x, y, width, height.
    prompt: Option<[f32; 4]>,
    touch: Option<bool>,
}

thread_local! {
    static ARCH: RefCell<Arch> = RefCell::default();
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

/// Follows the visitor at `player`; `plaza` is whether they are on the
/// Grid plaza rather than in a zone. Walking out of the arch's reach
/// closes Game Lab.
pub fn frame(player: Vec3, plaza: bool) {
    ARCH.with(|arch| {
        let mut arch = arch.borrow_mut();
        let near = plaza && game_place::in_reach(player, arch.near);
        if !near && arch.near && open_now() {
            call(false);
        }
        arch.near = near;
        if !near {
            arch.prompt = None;
        }
    });
}

/// Draws the prompt in the arch while the visitor is in reach and Game Lab
/// is closed.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4) {
    ARCH.with(|arch| {
        let mut arch = arch.borrow_mut();
        arch.prompt = None;
        if !arch.near || open_now() {
            return;
        }
        let touch = *arch.touch.get_or_insert_with(crate::help_desk::touch);
        let at = game_place::POSITION + Vec3::Y * PROMPT_LIFT;
        let Some([x, y]) = verse::hud::project(view_proj, size, at) else {
            return;
        };
        let text = if touch { PROMPT_TOUCH } else { PROMPT_KEYS };
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
        arch.prompt = Some([left, top, w, h]);
    });
}

/// A key the world would take: E in reach opens Game Lab. Returns whether
/// the arch used it.
pub fn key(code: &str) -> bool {
    let near = ARCH.with(|arch| arch.borrow().near);
    if near && code == KEY && !open_now() {
        call(true);
        return true;
    }
    false
}

/// A press at `at`, CSS pixels: a press on the prompt opens Game Lab.
/// Returns whether the arch used it.
pub fn press(at: [f32; 2]) -> bool {
    let hit = ARCH.with(|arch| {
        arch.borrow().prompt.is_some_and(|[x, y, w, h]| {
            at[0] >= x && at[0] <= x + w && at[1] >= y && at[1] <= y + h
        })
    });
    if hit && !open_now() {
        call(true);
        return true;
    }
    false
}
