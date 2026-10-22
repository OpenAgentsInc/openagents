//! The walking hint on the Grid homepage (#12459): "Click or WASD to walk
//! · Drag to look · Scroll to zoom" ("Tap to walk · Drag to look" on a
//! touch screen), drawn on the canvas at the bottom of the plaza view. It
//! steps aside at the shared terminal, where the chat's composer takes its
//! place, and while the chat or Game Lab is open.
//!
//! The page repeats it in a visually hidden line the canvas names as its
//! description, for screen readers.

use std::cell::RefCell;

use glam::Vec3;
use verse::ui::{Atlas, UiBatch};

pub const KEYS: &str = "Click or WASD to walk \u{b7} Drag to look \u{b7} Scroll to zoom";
pub const TOUCH: &str = "Tap to walk \u{b7} Drag to look";
/// The page globals that are `true` while the chat or Game Lab is open.
const OPEN_GLOBALS: [&str; 2] = ["openagentsChatOpen", "openagentsGameLabOpen"];
/// The hint's padding and its gap from the bottom, CSS pixels.
const PAD: [f32; 2] = [12.0, 8.0];
const BOTTOM: f32 = 20.0;

#[derive(Default)]
struct Hint {
    shown: bool,
    near: bool,
    touch: Option<bool>,
}

thread_local! {
    static HINT: RefCell<Hint> = RefCell::default();
}

fn open_now() -> bool {
    OPEN_GLOBALS.iter().any(|name| {
        js_sys::Reflect::get(&js_sys::global(), &(*name).into())
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    })
}

/// Follows the visitor at `player`; `plaza` is whether they are on the
/// Grid plaza rather than in a zone.
pub fn frame(player: Vec3, plaza: bool) {
    HINT.with(|hint| {
        let mut hint = hint.borrow_mut();
        let at = player.as_dvec3().to_array();
        hint.near = pylons_view::terminal::in_reach(at, hint.near);
        hint.shown = plaza && !hint.near && !open_now();
    });
}

/// Draws the hint at the bottom middle of a `size` canvas, CSS pixels.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2]) {
    HINT.with(|hint| {
        let mut hint = hint.borrow_mut();
        if !hint.shown {
            return;
        }
        let touch = *hint.touch.get_or_insert_with(crate::help_desk::touch);
        let text = if touch { TOUCH } else { KEYS };
        let w = atlas.measure(text) + PAD[0] * 2.0;
        let h = atlas.line + PAD[1] * 2.0;
        // Too narrow for the line: it says nothing rather than spilling.
        if w > size[0] - 16.0 {
            return;
        }
        let (left, top) = ((size[0] - w) / 2.0, size[1] - BOTTOM - h);
        ui.rect(
            atlas,
            left,
            top,
            w,
            h,
            crate::theme::linear(coder_ui::coder_noir::SURFACE, 0.8),
        );
        ui.text(
            atlas,
            left + PAD[0],
            top + PAD[1],
            text,
            crate::theme::linear(coder_ui::coder_noir::CONTENT_SECONDARY, 1.0),
        );
    });
}
