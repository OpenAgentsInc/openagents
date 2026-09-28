//! Super and a mouse drag: the left button moves a window, the right one
//! resizes it.
//!
//! The two rows are the mouse rows of `coder_binds::BINDS`, the ones
//! `os/modules/coderos/desktop.nix` renders as the `bindm` lines Hyprland
//! reads, so the drag a person learned on the Hyprland session is the drag
//! here.
//!
//! A press that matches a row takes the pointer until the button comes up.
//! The pointer's motion then moves or sizes the window the press landed
//! on, the drag keeps that window when the pointer crosses another one,
//! and the client under the pointer gets no press, no motion, and no
//! release while the drag owns the pointer.
//!
//! A float moves and resizes as a float, by the pixels the pointer moved.
//! A tile resizes along the drag, which is the call the desk protocol's
//! `shape` already makes, and a tile does not move: the dwindle tree
//! places a tile, and Super+Shift with an arrow is what moves one inside
//! it.
//!
//! The window rules hold while a drag runs. A window whose rule keeps its
//! aspect ratio, the camera circle among them, keeps the ratio it started
//! the drag at, and a resize stops at the floor and at the ceiling rather
//! than turning the rectangle inside out.

use coder_wm::WinId;
use smithay::backend::input::ButtonState;
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, Focus, GestureHoldBeginEvent, GestureHoldEndEvent,
    GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
    GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData, MotionEvent, PointerGrab,
    PointerInnerHandle, RelativeMotionEvent,
};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Serial};

use crate::layout::{Fill, Placed};
use crate::state::Coder;

/// The smallest a drag leaves a window, in logical pixels: enough to see
/// it and to take hold of it again.
pub const FLOOR: i32 = 64;

/// What a drag does to the window it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Move the window, the left button's row.
    Move,
    /// Resize the window, the right button's row.
    Resize,
}

/// The sizes a resize holds a window between: the floor it stops
/// shrinking at, and the screen it stops growing past.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    /// The smallest width and height a drag leaves.
    pub floor: i32,
    /// The widest a drag grows to, which is the screen's width.
    pub width: i32,
    /// The tallest a drag grows to, which is the screen's height.
    pub height: i32,
}

/// A drag in progress.
#[derive(Clone, Copy, Debug)]
pub struct Drag {
    /// The window the press landed on, which the drag keeps until the
    /// button comes up.
    pub id: WinId,
    /// What the drag does to it.
    pub kind: Kind,
    /// The desk the window is on, which its pixels are measured against.
    pub desk: usize,
    /// Whether the window floated when the drag started.
    pub floating: bool,
    /// Where the pointer was when the button went down.
    pub origin: Point<f64, Logical>,
    /// Where the pointer was at the step before this one, which a tiled
    /// resize moves the split by.
    pub last: Point<f64, Logical>,
    /// The pixels the window filled when the button went down. Each step
    /// measures from here rather than from the step before, so a drag
    /// that crosses a bound and comes back lands where the pointer is.
    pub start: Placed,
    /// Whether the resize pulls the left edge, which pins the right one.
    pub from_left: bool,
    /// Whether the resize pulls the top edge, which pins the bottom one.
    pub from_top: bool,
    /// The ratio the window holds as it resizes, its width over its
    /// height, when its rule keeps one.
    pub aspect: Option<f64>,
}

/// Where a move puts a window: the corner the pointer carried. The layout
/// holds a float inside its screen, so the move needs no bound of its own.
pub fn moved(start: Placed, dx: f64, dy: f64) -> Placed {
    Placed {
        x: start.x + dx.round() as i32,
        y: start.y + dy.round() as i32,
        ..start
    }
}

/// The pixels a resize puts a window at: the edge the drag pulls follows
/// the pointer and the edge opposite stays where it was.
///
/// The size stops at `bounds.floor` and at the screen. An edge dragged
/// past the one opposite stops at the floor rather than turning the
/// rectangle inside out, and a window that keeps a ratio keeps it at every
/// bound, because both sides scale together.
pub fn resized(
    start: Placed,
    dx: f64,
    dy: f64,
    from_left: bool,
    from_top: bool,
    aspect: Option<f64>,
    bounds: Bounds,
) -> Placed {
    let floor = f64::from(bounds.floor.max(1));
    let widest = f64::from(bounds.width.max(bounds.floor.max(1)));
    let tallest = f64::from(bounds.height.max(bounds.floor.max(1)));
    let pull = |side: i32, delta: f64, pulled: bool| {
        let grown = f64::from(side) + if pulled { -delta } else { delta };
        grown.max(1.0)
    };
    let mut width = pull(start.width, dx, from_left);
    let mut height = pull(start.height, dy, from_top);
    match aspect {
        Some(ratio) if ratio > 0.0 => {
            // The side the pointer moved further along drives, and the
            // other follows the ratio.
            if dx.abs() >= dy.abs() {
                height = width / ratio;
            } else {
                width = height * ratio;
            }
            // Both sides scale together, so a bound on either one moves
            // the other with it and the ratio survives the bound. The
            // floor is applied last, so a window with a ratio a screen
            // cannot hold stays visible.
            let scale = 1.0_f64
                .min(widest / width)
                .min(tallest / height)
                .max(floor / width)
                .max(floor / height);
            width *= scale;
            height *= scale;
        }
        _ => {
            width = width.max(floor).min(widest);
            height = height.max(floor).min(tallest);
        }
    }
    let width = width.round() as i32;
    let height = height.round() as i32;
    Placed {
        x: match from_left {
            true => start.x + start.width - width,
            false => start.x,
        },
        y: match from_top {
            true => start.y + start.height - height,
            false => start.y,
        },
        width,
        height,
    }
}

/// What a mouse button held with `mods` drags, or nothing when the shared
/// table holds no row for it.
///
/// The mouse rows are read from `crates/coder-binds` rather than joined
/// into the chord table in `crate::binds`: a button is not a keysym, and
/// the press that carries one never reaches the keyboard's filter.
pub fn bound(mods: coder_binds::Mods, button: u16) -> Option<Kind> {
    match coder_binds::find(
        coder_binds::Surface::Compositor,
        mods,
        coder_binds::Key::Mouse(button),
    ) {
        Some(coder_binds::Action::DragMove) => Some(Kind::Move),
        Some(coder_binds::Action::DragResize) => Some(Kind::Resize),
        _ => None,
    }
}

/// Starts a drag when a press matches a mouse row of the bind table, and
/// answers whether it did. A press that starts one never reaches the
/// client under the pointer.
pub fn starts(state: &mut Coder, button: u32, serial: Serial) -> bool {
    let Ok(code) = u16::try_from(button) else {
        return false;
    };
    let held = state.keyboard.modifier_state();
    let mods = coder_binds::Mods {
        super_key: held.logo,
        shift: held.shift,
        ctrl: held.ctrl,
        alt: held.alt,
    };
    let Some(kind) = bound(mods, code) else {
        return false;
    };
    let Some(id) = window_at(state) else {
        return false;
    };
    let Some((desk, tile)) = state
        .manager
        .all_tiles()
        .into_iter()
        .find(|(_, held)| held.id == id)
    else {
        return false;
    };
    let start = state.placed(desk, tile);
    let origin = state.pointer_at;
    let keeps_ratio = state.tile(id).is_some_and(|held| held.effects.keep_aspect)
        && start.width > 0
        && start.height > 0;
    state.drag = Some(Drag {
        id,
        kind,
        desk,
        floating: tile.floating,
        origin,
        last: origin,
        start,
        // A resize pulls the corner the press is nearest, the way
        // Hyprland's `resizewindow` does.
        from_left: origin.x < f64::from(start.x) + f64::from(start.width) / 2.0,
        from_top: origin.y < f64::from(start.y) + f64::from(start.height) / 2.0,
        aspect: match keeps_ratio {
            true => Some(f64::from(start.width) / f64::from(start.height)),
            false => None,
        },
    });
    log::debug!(
        "a drag takes the pointer: {kind:?} on {id:?} at {start:?}, ratio kept: {keeps_ratio}"
    );
    let pointer = state.pointer.clone();
    let focus = state.surface_under();
    pointer.set_grab(
        state,
        DragGrab {
            start_data: GrabStartData {
                focus,
                button,
                location: origin,
            },
        },
        serial,
        Focus::Clear,
    );
    true
}

/// Moves or sizes the window a drag holds, from where the pointer is now.
///
/// This runs from `crate::input::move_pointer`, before the motion reaches
/// the grab, because the redraw a step asks for reads the pointer back and
/// the pointer is locked while a grab method runs.
pub fn step(state: &mut Coder, at: Point<f64, Logical>) {
    let Some(drag) = state.drag else {
        return;
    };
    let dx = at.x - drag.origin.x;
    let dy = at.y - drag.origin.y;
    if drag.floating {
        let target = match drag.kind {
            Kind::Move => moved(drag.start, dx, dy),
            Kind::Resize => resized(
                drag.start,
                dx,
                dy,
                drag.from_left,
                drag.from_top,
                drag.aspect,
                room(state, &drag),
            ),
        };
        let rect = state.normalized(drag.desk, target);
        if state.manager.place_float(drag.id, rect) {
            state.after_layout();
        }
    } else if drag.kind == Kind::Resize {
        // A tile resizes along the drag, by the step the pointer just
        // took: the layout moves the split its window sits on, which is
        // a ratio and not a rectangle, so each step adds to the last.
        // A tiled move does nothing. The dwindle tree decides where a
        // tile sits, and Super+Shift with an arrow is what moves one.
        let screen = state.home_size(drag.desk);
        let step_x = (at.x - drag.last.x) as f32 / screen.width.max(1) as f32;
        let step_y = (at.y - drag.last.y) as f32 / screen.height.max(1) as f32;
        if step_x != 0.0 || step_y != 0.0 {
            state.manager.focus_id(drag.id);
            state
                .manager
                .resize_grab(step_x, step_y, drag.from_left, drag.from_top);
            state.after_layout();
        }
    }
    if let Some(held) = state.drag.as_mut() {
        held.last = at;
    }
}

/// The sizes a resize holds its window between: the floor, and the room
/// the window has in the direction the drag grows it.
///
/// The layout holds a float inside the area it lays out in. A resize that
/// asked for more would have the side that reached the edge cut back to
/// fit, and a window that keeps a ratio would lose it at the edge, so the
/// ceiling is the room rather than the screen.
fn room(state: &Coder, drag: &Drag) -> Bounds {
    let area = area_of(state, drag.desk);
    Bounds {
        floor: FLOOR,
        width: match drag.from_left {
            true => drag.start.x + drag.start.width - area.x,
            false => area.x + area.width - drag.start.x,
        },
        height: match drag.from_top {
            true => drag.start.y + drag.start.height - area.y,
            false => area.y + area.height - drag.start.y,
        },
    }
}

/// The pixels one desk's layout fills, which is the room its floats have.
fn area_of(state: &Coder, desk: usize) -> Placed {
    let whole = coder_wm::Rect {
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
    };
    let screen = state.home_size(desk);
    match state.home(desk) {
        Some(head) => head.place(whole, Fill::Tiled),
        None => crate::layout::place(whole, screen, crate::layout::whole(screen), Fill::Tiled),
    }
}

/// The window under the pointer, which is the one a drag takes hold of.
fn window_at(state: &Coder) -> Option<WinId> {
    let (window, _) = state.window_under(state.pointer_at)?;
    state.tile_of(&window).map(|tile| tile.id)
}

/// The pointer grab a drag holds.
///
/// While it is set no client is under the pointer, so the motion, the
/// press, and the release reach none of them, and the window the drag
/// took hold of is the window it moves however far the pointer travels.
/// The drag ends when the button that started it comes up.
struct DragGrab {
    start_data: GrabStartData<Coder>,
}

impl PointerGrab<Coder> for DragGrab {
    fn motion(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        // `crate::drag::step` has already moved the window. The motion
        // goes nowhere: the drag owns the pointer.
        handle.motion(data, None, event);
    }

    fn relative_motion(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, None, event);
    }

    fn button(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        event: &ButtonEvent,
    ) {
        // No client is under the pointer, so this reaches none of them.
        handle.button(data, event);
        if event.state == ButtonState::Released && event.button == self.start_data.button {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        details: AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn frame(&mut self, data: &mut Coder, handle: &mut PointerInnerHandle<'_, Coder>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut Coder,
        handle: &mut PointerInnerHandle<'_, Coder>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn start_data(&self) -> &GrabStartData<Coder> {
        &self.start_data
    }

    fn unset(&mut self, data: &mut Coder) {
        // The grab ends here whether the button came up or the compositor
        // took the pointer away, which is why the drag is dropped here and
        // not in the release.
        data.drag = None;
    }
}

#[cfg(test)]
#[path = "drag_tests.rs"]
mod tests;
