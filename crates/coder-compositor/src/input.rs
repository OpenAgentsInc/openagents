//! Input from either backend: the keyboard, the pointer, and the wheel.
//!
//! The nested window reports the pointer as a position in the window, and
//! `libinput` reports a mouse as motion from where it was. Both end as a
//! point in the space every screen shares, held to the screens, so a
//! pointer crosses from one monitor to the next and stops at the outer
//! edges. A press runs through the bind table before a client sees it, and
//! on the hardware backend Ctrl+Alt with a function key switches the
//! virtual terminal before that. A mouse press runs through the table's
//! mouse rows the same way: Super with a button starts the drag
//! `crate::drag` holds.
//!
//! A tracked hand enters here too, once #9874 brings the reader back:
//! `crate::hands` moves the pointer with
//! [`pointer_to`], presses with [`button`], and sends Escape with
//! [`key_tap`], the same paths a device's events take.

use std::time::Instant;

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, ButtonState, Event, InputBackend, InputEvent, KeyState,
    KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
};
use smithay::input::keyboard::{FilterResult, Keycode, KeysymHandle, ModifiersState};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent, RelativeMotionEvent};
use smithay::utils::SERIAL_COUNTER;

use crate::binds::{self, Action, Chord, Mods};
use crate::keys;
use crate::state::Coder;

/// What a press the compositor keeps asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// A chord in the bind table.
    Chord(Action),
    /// A switch to a virtual terminal, 1 through 12.
    Vt(i32),
}

/// What one input event does.
pub fn on_input<B: InputBackend>(state: &mut Coder, event: InputEvent<B>) {
    // Somebody is at the keyboard or the pointer, which is what an
    // `ext-idle-notify` client waits to hear.
    if state.activity.report(Instant::now()) {
        let seat = state.seat.clone();
        state.idle_notifier.notify_activity(&seat);
    }
    match event {
        InputEvent::Keyboard { event } => on_key::<B>(state, event),
        InputEvent::PointerMotion { event } => {
            let delta = event.delta();
            let time = event.time_msec();
            let utime = event.time();
            // What the surface under the pointer asks of it. A client that
            // hides the cursor and steers by motion holds the pointer here,
            // so it cannot walk into the pane beside this one.
            let held = crate::constraints::held(state);
            if held == crate::constraints::Held::Locked {
                // The pointer does not move, so no absolute motion is sent
                // and the focus does not follow it. The client reads the
                // delta and nothing else.
                let under = state.surface_under();
                let pointer = state.pointer.clone();
                pointer.relative_motion(
                    state,
                    under,
                    &RelativeMotionEvent {
                        delta,
                        delta_unaccel: event.delta_unaccel(),
                        utime,
                    },
                );
                pointer.frame(state);
                return;
            }
            let wanted = state.pointer_at + delta;
            let (x, y) = state.screens.clamp(wanted.x, wanted.y);
            let mut at: smithay::utils::Point<f64, smithay::utils::Logical> = (x, y).into();
            if let crate::constraints::Held::Confined(bounds) = held {
                at = crate::constraints::clamp(at, bounds);
            }
            state.pointer_at = at;
            let under = move_pointer(state, time);
            let pointer = state.pointer.clone();
            pointer.relative_motion(
                state,
                under,
                &RelativeMotionEvent {
                    delta,
                    delta_unaccel: event.delta_unaccel(),
                    utime,
                },
            );
            pointer.frame(state);
        }
        InputEvent::PointerMotionAbsolute { event } => {
            // The nested window is one screen, and a position in the window
            // is a position on that screen.
            let Some(head) = state.screens.focused().cloned() else {
                return;
            };
            let size = head.logical();
            let at = event.position_transformed((size.width, size.height).into());
            state.pointer_at = (at.x + f64::from(head.at.0), at.y + f64::from(head.at.1)).into();
            move_pointer(state, event.time_msec());
            let pointer = state.pointer.clone();
            pointer.frame(state);
        }
        InputEvent::PointerButton { event } => {
            press_button(state, event.button_code(), event.state(), event.time_msec());
        }
        InputEvent::PointerAxis { event } => {
            let mut frame = AxisFrame::new(event.time_msec()).source(event.source());
            for axis in [Axis::Horizontal, Axis::Vertical] {
                if let Some(amount) = event.amount(axis) {
                    frame = frame.value(axis, amount);
                }
                if let Some(discrete) = event.amount_v120(axis) {
                    frame = frame.v120(axis, discrete as i32);
                }
                if event.amount(axis) == Some(0.0) {
                    frame = frame.stop(axis);
                }
            }
            let pointer = state.pointer.clone();
            pointer.axis(state, frame);
            pointer.frame(state);
        }
        _ => {}
    }
}

/// Presses or releases one pointer button where the pointer is, which is
/// what a device's button and a hand's pinch both come through.
fn press_button(state: &mut Coder, button: u32, button_state: ButtonState, time: u32) {
    let serial = state.serial();
    if button_state == ButtonState::Pressed {
        state.focus_follows_pointer();
        // The press goes to the surface under the pointer now, not to the
        // one the pointer found when it last moved.
        state.refresh_pointer();
        // A press that starts a Super drag takes the pointer with it, and
        // the client under the pointer hears nothing until it ends.
        if crate::drag::starts(state, button, serial) {
            return;
        }
    }
    let pointer = state.pointer.clone();
    pointer.button(
        state,
        &ButtonEvent {
            button,
            state: button_state,
            serial,
            time,
        },
    );
    pointer.frame(state);
}

/// The time an event the compositor makes itself carries: milliseconds
/// since it started, which is what its frame callbacks count in.
fn now(state: &Coder) -> u32 {
    state.started.elapsed().as_millis() as u32
}

/// Puts the pointer at a point in the shared space, held to the screens,
/// and tells the client under it, as a device's absolute motion does.
pub fn pointer_to(state: &mut Coder, at: (f64, f64)) {
    let (x, y) = state.screens.clamp(at.0, at.1);
    state.pointer_at = (x, y).into();
    let time = now(state);
    move_pointer(state, time);
    let pointer = state.pointer.clone();
    pointer.frame(state);
}

/// Presses or releases a button at the pointer, by its evdev code, as a
/// device's button does.
pub fn button(state: &mut Coder, code: u32, pressed: bool) {
    let button_state = if pressed {
        ButtonState::Pressed
    } else {
        ButtonState::Released
    };
    let time = now(state);
    press_button(state, code, button_state, time);
}

/// Presses and releases one key on the seat's keyboard, by its evdev
/// code, so the focused client reads it under the seat's keymap. The
/// bind table is not consulted: a hand sends no chord.
pub fn key_tap(state: &mut Coder, evdev: u32) {
    let code = Keycode::new(evdev + 8);
    for key_state in [KeyState::Pressed, KeyState::Released] {
        let time = now(state);
        if let Some(kept) = press(state, code, key_state, time) {
            log::debug!("a key from a hand asked for {kept:?}");
        }
    }
}

/// Tells the client under the pointer where it moved, then moves the focus
/// after it, and answers the surface under it. The pointer goes first: the
/// focus change restacks the space and reads the pointer's focus back,
/// which finds the surface the motion already entered and sends nothing
/// twice.
pub fn move_pointer(
    state: &mut Coder,
    time: u32,
) -> Option<(
    smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    smithay::utils::Point<f64, smithay::utils::Logical>,
)> {
    // A drag in progress moves or sizes its window here, before the
    // motion reaches the grab that holds the pointer: the redraw a step
    // asks for reads the pointer back, and the pointer is locked while a
    // grab method runs.
    crate::drag::step(state, state.pointer_at);
    let under = state.surface_under();
    let serial = state.serial();
    let location = state.pointer_at;
    let pointer = state.pointer.clone();
    pointer.motion(
        state,
        under.clone(),
        &MotionEvent {
            location,
            serial,
            time,
        },
    );
    state.focus_follows_pointer();
    under
}

/// Whether the pointer's focus has to move: the surface under the pointer
/// is not the one the pointer found when it last moved. A window that
/// mapped, closed, was raised, or was restacked under a pointer that held
/// still leaves the focus on the window that used to be there, and a press
/// sent then goes to that window.
pub fn pointer_moves<T: PartialEq>(focus: Option<&T>, under: Option<&T>) -> bool {
    focus != under
}

fn on_key<B: InputBackend>(state: &mut Coder, event: B::KeyboardKeyEvent) {
    let code = event.key_code();
    let key_state = event.state();
    let time = event.time_msec();
    match press(state, code, key_state, time) {
        Some(Kept::Chord(action)) => state.run(action),
        Some(Kept::Vt(vt)) => state.switch_vt(vt),
        None => {}
    }
}

/// Presses or releases one key on the seat's keyboard and answers what the
/// press asks the compositor for, or nothing when the press belongs to the
/// client.
///
/// A key runs the same filter whichever side sent it: a backend read it
/// from the keyboard, or the desk protocol's `key` drove it.
pub fn press(state: &mut Coder, code: Keycode, key_state: KeyState, time: u32) -> Option<Kept> {
    let serial = SERIAL_COUNTER.next_serial();
    let keyboard = state.keyboard.clone();
    keyboard.input::<Kept, _>(
        state,
        code,
        key_state,
        serial,
        time,
        |state, modifiers, handle| {
            if key_state != KeyState::Pressed {
                return FilterResult::Forward;
            }
            match keep(&state.binds, modifiers, &handle) {
                Some(kept) => FilterResult::Intercept(kept),
                None => FilterResult::Forward,
            }
        },
    )
}

/// What one press asks the compositor for, or nothing when it belongs to
/// the client.
fn keep(
    rows: &[(Chord, Action)],
    modifiers: &ModifiersState,
    handle: &KeysymHandle<'_>,
) -> Option<Kept> {
    if let Some(vt) = keys::vt_of(handle.modified_sym()) {
        return Some(Kept::Vt(vt));
    }
    let mods = Mods {
        logo: modifiers.logo,
        shift: modifiers.shift,
        ctrl: modifiers.ctrl,
        alt: modifiers.alt,
    };
    if !binds::can_chord(mods) {
        return None;
    }
    let key = keys::key_of(handle)?;
    binds::action(rows, Chord { mods, key }).map(Kept::Chord)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stacking::{self, Layer};
    use coder_wm::WinId;

    /// A window and the pixels it covers.
    struct Placed {
        id: WinId,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    /// The window a point reaches, reading the stack front to back the way
    /// the space does.
    fn under(order: &[WinId], placed: &[Placed], x: i32, y: i32) -> Option<WinId> {
        order.iter().rev().copied().find(|id| {
            placed.iter().any(|window| {
                window.id == *id
                    && (window.x..window.x + window.width).contains(&x)
                    && (window.y..window.y + window.height).contains(&y)
            })
        })
    }

    #[test]
    fn a_press_on_a_pinned_float_over_a_tile_moves_the_pointer_onto_the_float() {
        let tile = WinId(1);
        let strip = WinId(2);
        let placed = [
            Placed {
                id: tile,
                x: 0,
                y: 0,
                width: 1280,
                height: 800,
            },
            Placed {
                id: strip,
                x: 700,
                y: 600,
                width: 560,
                height: 104,
            },
        ];
        // The pointer moved onto the tile, and then the strip mapped under
        // it, pinned over the tile, without the pointer moving.
        let focus = under(&stacking::order(&[(Layer::Tiled, tile)]), &placed, 900, 690);
        assert_eq!(focus, Some(tile));
        let order = stacking::order(&[(Layer::Tiled, tile), (Layer::Pinned, strip)]);
        let now = under(&order, &placed, 900, 690);
        assert_eq!(
            now,
            Some(strip),
            "the pinned float is what the pointer reaches"
        );
        assert!(
            pointer_moves(focus.as_ref(), now.as_ref()),
            "the press moves the pointer onto the float first"
        );
    }

    #[test]
    fn a_press_on_the_surface_the_pointer_holds_moves_nothing() {
        let strip = WinId(2);
        assert!(!pointer_moves(Some(&strip), Some(&strip)));
        assert!(!pointer_moves::<WinId>(None, None));
        assert!(
            pointer_moves(Some(&strip), None),
            "the window under the pointer closed"
        );
    }
}
