//! Desktop controls translated into the shared player and camera intents.
use rust_native_desktop::input::NativeInput;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use verse::controller::InputState;
use verse::runtime::Action;

#[derive(Default)]
pub struct Controls {
    keys: BTreeSet<String>,
    pub focused: bool,
    pub left: bool,
    pub right: bool,
    jump: bool,
    press: Option<([f32; 2], Instant, f32)>,
}

pub enum Effect {
    Camera(Action),
    Click([f32; 2]),
    Escape,
}

impl Controls {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn capture(&self) -> bool {
        self.focused && (self.left || self.right)
    }
    pub fn input(&mut self) -> InputState {
        if !self.focused {
            return InputState::default();
        }
        InputState {
            forward: self.keys.contains("KeyW") || (self.left && self.right),
            backward: self.keys.contains("KeyS"),
            strafe_left: self.keys.contains("KeyA") || self.keys.contains("KeyQ"),
            strafe_right: self.keys.contains("KeyD") || self.keys.contains("KeyE"),
            mouse_look: self.right,
            sprint: self.keys.contains("ShiftLeft") || self.keys.contains("ShiftRight"),
            jump: std::mem::take(&mut self.jump),
            ..InputState::default()
        }
    }

    pub fn event(
        &mut self,
        event: NativeInput<'_>,
        inside: bool,
        now: Instant,
    ) -> (bool, Option<Effect>) {
        match event {
            NativeInput::Key {
                code: "Tab",
                pressed: true,
                ..
            } => {
                self.clear();
                (false, None)
            }
            NativeInput::Focus(false) | NativeInput::Cancel => {
                self.clear();
                (false, None)
            }
            NativeInput::Key {
                code: "Escape",
                pressed: true,
                ..
            } if self.focused => {
                self.clear();
                (true, Some(Effect::Escape))
            }
            NativeInput::Key {
                code,
                pressed,
                repeat,
                command,
                alt,
            } => {
                let known = matches!(
                    code,
                    "KeyW"
                        | "KeyA"
                        | "KeyS"
                        | "KeyD"
                        | "KeyQ"
                        | "KeyE"
                        | "ShiftLeft"
                        | "ShiftRight"
                        | "Space"
                );
                if !known || !self.focused {
                    return (false, None);
                }
                if !pressed {
                    self.keys.remove(code);
                } else if command || alt {
                    self.clear();
                    return (false, None);
                } else if !repeat && self.keys.insert(code.to_owned()) && code == "Space" {
                    self.jump = true;
                }
                (true, None)
            }
            NativeInput::Button {
                button,
                pressed,
                x,
                y,
            } if button <= 1 => {
                if pressed && !inside {
                    self.clear();
                    return (false, None);
                }
                if !inside && !self.capture() {
                    return (false, None);
                }
                self.focused = true;
                if button == 0 {
                    self.left = pressed;
                    if pressed {
                        self.press = Some(([x, y], now, 0.0));
                    } else if let Some((point, start, travel)) = self.press.take()
                        && inside
                        && travel <= 8.0
                        && now.saturating_duration_since(start) <= Duration::from_millis(300)
                        && (point[0] - x).hypot(point[1] - y) <= 8.0
                        && !self.right
                    {
                        return (true, Some(Effect::Click(point)));
                    }
                } else {
                    self.right = pressed;
                    self.press = None;
                    if pressed {
                        return (true, Some(Effect::Camera(Action::FaceCamera)));
                    }
                }
                (true, None)
            }
            NativeInput::Motion { dx, dy } if self.capture() => {
                if let Some((_, _, travel)) = &mut self.press {
                    *travel += dx.hypot(dy);
                }
                let action = if self.right {
                    Action::Look { dx, dy }
                } else {
                    Action::Orbit { dx, dy }
                };
                (true, Some(Effect::Camera(action)))
            }
            NativeInput::Cursor { .. } => (self.capture(), None),
            NativeInput::Wheel { lines, .. } if inside => {
                (true, Some(Effect::Camera(Action::Zoom { lines })))
            }
            _ => (false, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_chords_use_normalized_diagonals_backpedaling_and_sprint() {
        let travel = |keys: &[&str]| {
            let mut controls = Controls {
                focused: true,
                ..Controls::default()
            };
            let mut world = verse::runtime::WorldRuntime::bare();
            world.set_spawn([-30.0, 0.0, -30.0].into(), 0.0).unwrap();
            let start = world.player.pos;
            for code in keys {
                controls.event(key(code, true, false), true, Instant::now());
            }
            for _ in 0..60 {
                world.tick(&controls.input(), 1.0 / 60.0);
            }
            world.player.pos.distance(start)
        };
        let forward = travel(&["KeyW"]);
        let diagonal = travel(&["KeyW", "KeyD"]);
        assert!((forward - diagonal).abs() < 0.02);
        assert!(travel(&["KeyS"]) < forward * 0.8);
        assert!(travel(&["KeyW", "ShiftLeft"]) > forward * 1.2);
    }
    fn key(code: &str, pressed: bool, repeat: bool) -> NativeInput<'_> {
        NativeInput::Key {
            code,
            pressed,
            repeat,
            command: false,
            alt: false,
        }
    }

    #[test]
    fn releases_and_repeat_preserve_simultaneous_movement_and_one_jump() {
        let now = Instant::now();
        let mut controls = Controls {
            focused: true,
            ..Controls::default()
        };
        for code in ["KeyW", "KeyD", "Space"] {
            assert!(controls.event(key(code, true, false), true, now).0);
        }
        let input = controls.input();
        assert!(input.forward && input.strafe_right && input.jump);
        controls.event(key("Space", true, true), true, now);
        assert!(!controls.input().jump);
        controls.event(key("KeyW", false, false), true, now);
        let input = controls.input();
        assert!(!input.forward && input.strafe_right);
        controls.event(NativeInput::Focus(false), false, now);
        assert_eq!(controls.input(), InputState::default());
        assert!(!controls.capture());
    }

    #[test]
    fn an_orbit_drag_back_to_its_origin_never_opens_a_board() {
        let now = Instant::now();
        let mut controls = Controls::default();
        controls.event(
            NativeInput::Button {
                button: 0,
                pressed: true,
                x: 20.0,
                y: 30.0,
            },
            true,
            now,
        );
        assert!(controls.capture());
        controls.event(NativeInput::Motion { dx: 10.0, dy: 0.0 }, true, now);
        controls.event(NativeInput::Motion { dx: -10.0, dy: 0.0 }, true, now);
        let (_, effect) = controls.event(
            NativeInput::Button {
                button: 0,
                pressed: false,
                x: 20.0,
                y: 30.0,
            },
            true,
            now,
        );
        assert!(effect.is_none());
        assert!(!controls.capture());
        controls.event(
            NativeInput::Button {
                button: 1,
                pressed: true,
                x: 20.0,
                y: 30.0,
            },
            true,
            now,
        );
        controls.event(key("Escape", true, false), true, now);
        assert!(!controls.capture());
    }
}
