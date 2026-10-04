//! Keyboard, mouse, and touch state, mapped to Verse's shared controller as
//! the desktop maps it.
//!
//! - `W`/`S` or the up and down arrows walk; `A`/`D` or the left and right
//!   arrows turn (strafe while looking); `Q`/`E` strafe; `Shift` runs;
//!   `Space` jumps.
//! - A left drag orbits the camera, a right drag turns the character, both
//!   buttons walk forward, and the wheel zooms.
//! - One finger turns the character and tilts the camera. Two fingers walk
//!   forward, and pinching them zooms.
use std::collections::BTreeMap;
use verse::controller::InputState;
use verse::runtime::Action;

/// Held keys, buttons, and touches.
#[derive(Default)]
pub struct Input {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    strafe_left: bool,
    strafe_right: bool,
    sprint: bool,
    jump: bool,
    /// The left mouse button: orbit.
    pub orbit: bool,
    /// The right mouse button: mouselook.
    pub look: bool,
    /// Active touches by pointer ID, at their last position in CSS pixels.
    touches: BTreeMap<i32, [f32; 2]>,
}

impl Input {
    /// Records a key by its `KeyboardEvent.code`. Returns whether the key is
    /// one Everglade uses, so the page does not also scroll on it.
    pub fn key(&mut self, code: &str, down: bool) -> bool {
        let held = match code {
            "KeyW" | "ArrowUp" => &mut self.forward,
            "KeyS" | "ArrowDown" => &mut self.backward,
            "KeyA" | "ArrowLeft" => &mut self.left,
            "KeyD" | "ArrowRight" => &mut self.right,
            "KeyQ" => &mut self.strafe_left,
            "KeyE" => &mut self.strafe_right,
            "ShiftLeft" | "ShiftRight" => &mut self.sprint,
            "Space" => {
                // One-shot: the frame that reads it clears it.
                self.jump |= down;
                return true;
            }
            _ => return false,
        };
        *held = down;
        true
    }

    /// Releases everything, as when the page loses focus.
    pub fn release(&mut self) {
        *self = Self::default();
    }

    /// This frame's controller input. A jump is reported once.
    pub fn take(&mut self) -> InputState {
        let walking = (self.orbit && self.look) || self.touches.len() >= 2;
        let state = InputState {
            forward: self.forward || walking,
            backward: self.backward,
            left: self.left,
            right: self.right,
            strafe_left: self.strafe_left,
            strafe_right: self.strafe_right,
            mouse_look: self.look,
            sprint: self.sprint,
            jump: self.jump,
        };
        self.jump = false;
        state
    }

    /// A mouse drag of `dx`, `dy` CSS pixels.
    pub fn drag(&self, dx: f32, dy: f32) -> Option<Action> {
        if self.look {
            Some(Action::Look { dx, dy })
        } else if self.orbit {
            Some(Action::Orbit { dx, dy })
        } else {
            None
        }
    }

    /// A touch went down at `at`.
    pub fn touch_start(&mut self, id: i32, at: [f32; 2]) {
        // Two fingers are enough; a third is ignored.
        if self.touches.len() < 2 {
            self.touches.insert(id, at);
        }
    }

    /// A touch moved to `at`: one finger turns, two pinch.
    pub fn touch_move(&mut self, id: i32, at: [f32; 2]) -> Option<Action> {
        let before = self.spread();
        let last = self.touches.get_mut(&id)?;
        let (dx, dy) = (at[0] - last[0], at[1] - last[1]);
        *last = at;
        match (before, self.spread()) {
            (Some(before), Some(after)) if before > 1.0 => Some(Action::PinchZoom {
                scale: after / before,
            }),
            (None, None) => Some(Action::Look { dx, dy }),
            _ => None,
        }
    }

    /// A touch ended or was canceled.
    pub fn touch_end(&mut self, id: i32) {
        self.touches.remove(&id);
    }

    /// The distance between two touches.
    fn spread(&self) -> Option<f32> {
        let mut points = self.touches.values();
        let (a, b) = (points.next()?, points.next()?);
        Some((a[0] - b[0]).hypot(a[1] - b[1]))
    }
}
