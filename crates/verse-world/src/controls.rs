//! Classic default keyboard movement and independent camera/character mouse modes.
use glam::Vec3;

#[derive(Default, Clone, Copy)]
pub struct Held {
    pub forward: bool,
    pub backward: bool,
    pub turn_left: bool,
    pub turn_right: bool,
    pub strafe_left: bool,
    pub strafe_right: bool,
}
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target_distance: f32,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: std::f32::consts::PI,
            pitch: 0.2,
            distance: 5.5,
            target_distance: 5.5,
        }
    }
}
impl Camera {
    pub fn direction(&self) -> Vec3 {
        Vec3::new(
            -self.yaw.sin() * self.pitch.cos(),
            -self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }
    pub fn zoom(&mut self, steps: f32) {
        if steps.is_finite() {
            self.target_distance =
                (self.target_distance - steps * 0.9144).clamp(0.0, 15.0 * 0.9144);
        }
    }
}
#[derive(Default)]
pub struct ClassicControls {
    pub left: bool,
    pub right: bool,
    pub autorun: bool,
    last_command: u16,
    previous_held: Held,
    follow: Option<(f32, f32, f32)>,
}
impl ClassicControls {
    /// Entering right mouselook faces the character along the current camera yaw.
    pub fn button(&mut self, right: bool, down: bool, yaw: &mut f32, camera: &Camera) {
        let was_both = self.left && self.right;
        if right {
            self.right = down;
            if down {
                *yaw = camera.yaw;
            }
        } else {
            self.left = down;
        }
        if !was_both && self.left && self.right {
            self.autorun = false;
        }
    }
    pub fn looking(&self) -> bool {
        self.left || self.right
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn motion(&self, delta: [f64; 2], yaw: &mut f32, camera: &mut Camera) {
        if !self.looking() || delta.iter().any(|d| !d.is_finite()) {
            return;
        }
        // Relative pointer units avoid viewport-size-dependent steering and edge stops.
        camera.yaw = (camera.yaw - delta[0] as f32 * (180.0f32 / 800.0).to_radians())
            .rem_euclid(std::f32::consts::TAU);
        camera.pitch = (camera.pitch + delta[1] as f32 * (90.0f32 / 600.0).to_radians())
            .clamp(-89.0f32.to_radians(), 89.0f32.to_radians());
        if self.right {
            *yaw = camera.yaw;
        }
    }
    pub fn step(&mut self, held: Held, dt: f32, yaw: &mut f32, camera: &mut Camera) -> [f32; 2] {
        let axis = |positive: bool, negative: bool| positive as u8 as f32 - negative as u8 as f32;
        if (held.forward && !self.previous_held.forward)
            || (held.backward && !self.previous_held.backward)
        {
            self.autorun = false;
        }
        self.previous_held = held;
        let strafe = (axis(held.strafe_right, held.strafe_left)
            + if self.right {
                axis(held.turn_right, held.turn_left)
            } else {
                0.0
            })
        .clamp(-1.0, 1.0);
        let forward = axis(
            held.forward || self.autorun || (self.left && self.right),
            held.backward,
        );
        if !self.right {
            let moving = strafe != 0.0 || forward != 0.0;
            let turn = axis(held.turn_left, held.turn_right)
                * std::f32::consts::PI
                * dt
                * if moving { 0.75 } else { 1.0 };
            *yaw = (*yaw + turn).rem_euclid(std::f32::consts::TAU);
            camera.yaw = (camera.yaw + turn).rem_euclid(std::f32::consts::TAU);
        }
        if self.right {
            *yaw = camera.yaw;
        }
        let flags = [
            held.forward,
            held.backward,
            held.turn_left,
            held.turn_right,
            held.strafe_left,
            held.strafe_right,
            self.left,
            self.right,
            self.autorun,
        ];
        let command = flags
            .iter()
            .enumerate()
            .fold(0u16, |bits, (i, on)| bits | ((*on as u16) << i));
        let driven = strafe != 0.0 || forward != 0.0 || held.turn_left || held.turn_right;
        if self.looking() || !driven {
            self.follow = None;
        } else if command != self.last_command {
            let offset = (camera.yaw - *yaw + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            self.follow = Some((
                offset,
                0.0,
                (offset.abs() / std::f32::consts::PI).clamp(0.1, 2.0),
            ));
        }
        self.last_command = command;
        if let Some((offset, elapsed, duration)) = &mut self.follow {
            *elapsed += dt;
            let progress = (*elapsed / *duration).min(1.0);
            camera.yaw = *yaw + *offset * (1.0 + (std::f32::consts::PI * progress).cos()) * 0.5;
            if progress == 1.0 {
                self.follow = None;
            }
        }
        let difference = camera.target_distance - camera.distance;
        camera.distance += difference.clamp(-8.33 * 0.9144 * dt, 8.33 * 0.9144 * dt);
        [strafe, forward]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn turn_keys_become_strafe_only_during_right_mouselook() {
        let mut controls = ClassicControls::default();
        let mut camera = Camera::default();
        let mut yaw = camera.yaw;
        let d = Held {
            turn_right: true,
            ..Held::default()
        };
        assert_eq!(controls.step(d, 0.1, &mut yaw, &mut camera), [0.0; 2]);
        assert!(yaw < std::f32::consts::PI);
        controls.button(true, true, &mut yaw, &camera);
        let before = yaw;
        assert_eq!(controls.step(d, 0.1, &mut yaw, &mut camera), [1.0, 0.0]);
        assert_eq!(yaw, before);
        controls.button(true, false, &mut yaw, &camera);
        assert_eq!(
            controls.step(
                Held {
                    strafe_left: true,
                    forward: true,
                    ..Held::default()
                },
                0.1,
                &mut yaw,
                &mut camera
            ),
            [-1.0, 1.0]
        );
    }
    #[test]
    fn orbit_preserves_facing_steering_aligns_and_both_buttons_run() {
        let mut controls = ClassicControls::default();
        let mut camera = Camera::default();
        let mut yaw = camera.yaw;
        controls.button(false, true, &mut yaw, &camera);
        controls.motion([200.0, 100.0], &mut yaw, &mut camera);
        assert_eq!(yaw, std::f32::consts::PI);
        assert_ne!(camera.yaw, yaw);
        assert!(camera.pitch > 0.2);
        controls.button(true, true, &mut yaw, &camera);
        assert_eq!(yaw, camera.yaw);
        assert_eq!(
            controls.step(Held::default(), 0.1, &mut yaw, &mut camera),
            [0.0, 1.0]
        );
        controls.motion([100.0, -100.0], &mut yaw, &mut camera);
        assert_eq!(yaw, camera.yaw);
        controls.button(false, false, &mut yaw, &camera);
        assert_eq!(
            controls.step(Held::default(), 0.1, &mut yaw, &mut camera),
            [0.0; 2]
        );
        controls.clear();
        let before = yaw;
        controls.motion([500.0, 0.0], &mut yaw, &mut camera);
        assert_eq!(yaw, before);
    }
    #[test]
    fn opposing_keys_net_and_autorun_is_cancelled_by_edges() {
        let mut c = ClassicControls::default();
        let mut camera = Camera::default();
        let mut yaw = camera.yaw;
        let back = Held {
            backward: true,
            ..Held::default()
        };
        c.step(back, 0.1, &mut yaw, &mut camera);
        c.autorun = true;
        assert_eq!(c.step(back, 0.1, &mut yaw, &mut camera), [0.0; 2]);
        c.step(Held::default(), 0.1, &mut yaw, &mut camera);
        assert_eq!(c.step(back, 0.1, &mut yaw, &mut camera), [0.0, -1.0]);
        assert!(!c.autorun);
        c.button(true, true, &mut yaw, &camera);
        c.autorun = true;
        c.button(false, true, &mut yaw, &camera);
        assert!(!c.autorun);
        assert_eq!(
            c.step(
                Held {
                    strafe_left: true,
                    strafe_right: true,
                    turn_left: true,
                    ..Held::default()
                },
                0.1,
                &mut yaw,
                &mut camera
            ),
            [-1.0, 1.0]
        );
        assert_eq!(
            c.step(
                Held {
                    forward: true,
                    backward: true,
                    ..Held::default()
                },
                0.1,
                &mut yaw,
                &mut camera
            ),
            [0.0; 2]
        );
    }
    #[test]
    fn reference_rates_and_camera_return_are_frame_independent() {
        let mut c = ClassicControls::default();
        let mut camera = Camera::default();
        let mut yaw = camera.yaw;
        c.button(false, true, &mut yaw, &camera);
        c.motion([800.0, 600.0], &mut yaw, &mut camera);
        assert!((camera.yaw - 0.0).abs() < 0.001);
        assert_eq!(camera.pitch, 89.0f32.to_radians());
        c.button(false, false, &mut yaw, &camera);
        let moving = Held {
            forward: true,
            turn_left: true,
            ..Held::default()
        };
        let before = yaw;
        c.step(moving, 0.1, &mut yaw, &mut camera);
        assert!((yaw - before - std::f32::consts::PI * 0.075).abs() < 0.001);
        camera.zoom(1.0);
        let before = camera.distance;
        c.step(moving, 0.1, &mut yaw, &mut camera);
        assert!((before - camera.distance - 8.33 * 0.9144 * 0.1).abs() < 0.001);
        let mut a = ClassicControls::default();
        let mut b = ClassicControls::default();
        let mut ca = Camera::default();
        let mut cb = ca;
        let mut ya = ca.yaw;
        let mut yb = ya;
        ca.yaw += 1.0;
        cb.yaw += 1.0;
        let forward = Held {
            forward: true,
            ..Held::default()
        };
        for _ in 0..10 {
            a.step(forward, 0.02, &mut ya, &mut ca);
        }
        for _ in 0..20 {
            b.step(forward, 0.01, &mut yb, &mut cb);
        }
        assert!((ca.yaw - cb.yaw).abs() < 0.001);
    }
    #[test]
    fn zoom_pitch_limits_and_focus_reset() {
        let mut c = ClassicControls {
            left: true,
            right: true,
            autorun: true,
            ..Default::default()
        };
        let mut camera = Camera::default();
        let mut yaw = camera.yaw;
        camera.zoom(1000.0);
        assert_eq!(camera.target_distance, 0.0);
        camera.zoom(-1000.0);
        assert_eq!(camera.target_distance, 15.0 * 0.9144);
        c.motion([0.0, 10000.0], &mut yaw, &mut camera);
        assert_eq!(camera.pitch, 89.0f32.to_radians());
        c.motion([f64::NAN, 0.0], &mut yaw, &mut camera);
        assert!(camera.yaw.is_finite());
        c.clear();
        assert!(!c.looking());
        assert!(!c.autorun);
    }
}

/// A platform control's meaning; mappings carry no world authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Forward,
    Backward,
    TurnLeft,
    TurnRight,
    StrafeLeft,
    StrafeRight,
    Jump,
    Cast(u8),
    Target,
    Respawn,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub control: String,
    pub action: Action,
}
/// Bounded mappings and held physical controls, shared by platform adapters.
/// A focus clear fences held controls until their release; repeating a down
/// event cannot cast twice or restart movement after focus recovery.
pub struct InputMap {
    bindings: std::collections::BTreeMap<String, Action>,
    pressed: std::collections::BTreeSet<String>,
    blocked: std::collections::BTreeSet<String>,
}
impl InputMap {
    pub fn new(bindings: Vec<Binding>) -> Result<Self, String> {
        if bindings.len() > 64 {
            return Err("Input mapping exceeds 64 controls".into());
        }
        let mut map = std::collections::BTreeMap::new();
        for binding in bindings {
            if binding.control.is_empty()
                || binding.control.len() > 32
                || !binding.control.is_ascii()
                || binding.control.chars().any(char::is_control)
                || matches!(binding.action, Action::Cast(slot) if slot >= 10)
                || map.insert(binding.control, binding.action).is_some()
            {
                return Err("Input mapping has an invalid or duplicate control".into());
            }
        }
        Ok(Self {
            bindings: map,
            pressed: Default::default(),
            blocked: Default::default(),
        })
    }
    /// Changes bindings atomically and fences controls already held until release.
    pub fn rebind(&mut self, bindings: Vec<Binding>) -> Result<(), String> {
        let mut next = Self::new(bindings)?;
        for control in self.pressed.iter().chain(&self.blocked) {
            if next.bindings.contains_key(control) {
                next.blocked.insert(control.clone());
            }
        }
        *self = next;
        Ok(())
    }
    /// Returns a discrete action only on a fresh press. Movement is read by `held`.
    pub fn change(&mut self, control: &str, down: bool) -> Option<Action> {
        let action = *self.bindings.get(control)?;
        if !down {
            self.pressed.remove(control);
            self.blocked.remove(control);
            return None;
        }
        if self.blocked.contains(control) || !self.pressed.insert(control.into()) {
            return None;
        }
        matches!(
            action,
            Action::Jump | Action::Cast(_) | Action::Target | Action::Respawn
        )
        .then_some(action)
    }
    pub fn held(&self) -> Held {
        let active = |action| {
            self.pressed
                .iter()
                .any(|control| self.bindings[control] == action)
        };
        Held {
            forward: active(Action::Forward),
            backward: active(Action::Backward),
            turn_left: active(Action::TurnLeft),
            turn_right: active(Action::TurnRight),
            strafe_left: active(Action::StrafeLeft),
            strafe_right: active(Action::StrafeRight),
        }
    }
    pub fn clear(&mut self) {
        self.blocked.append(&mut self.pressed);
    }
}
impl Default for InputMap {
    fn default() -> Self {
        let mut bindings = vec![];
        for (control, action) in [
            ("KeyW", Action::Forward),
            ("KeyS", Action::Backward),
            ("KeyA", Action::TurnLeft),
            ("KeyD", Action::TurnRight),
            ("KeyQ", Action::StrafeLeft),
            ("KeyE", Action::StrafeRight),
            ("Space", Action::Jump),
            ("Tab", Action::Target),
            ("KeyR", Action::Respawn),
            ("TouchForward", Action::Forward),
            ("TouchBackward", Action::Backward),
            ("TouchLeft", Action::StrafeLeft),
            ("TouchRight", Action::StrafeRight),
            ("TouchTurnLeft", Action::TurnLeft),
            ("TouchTurnRight", Action::TurnRight),
            ("PadForward", Action::Forward),
            ("PadBackward", Action::Backward),
            ("PadLeft", Action::StrafeLeft),
            ("PadRight", Action::StrafeRight),
            ("Pad0", Action::Cast(0)),
            ("Pad1", Action::Jump),
        ] {
            bindings.push(Binding {
                control: control.into(),
                action,
            });
        }
        for slot in 0..10 {
            bindings.push(Binding {
                control: format!("Digit{}", (slot + 1) % 10),
                action: Action::Cast(slot),
            });
        }
        Self::new(bindings).unwrap()
    }
}
#[cfg(test)]
mod platform_input_tests {
    use super::*;
    #[test]
    fn remapping_fences_repeats_and_focus_and_keeps_independent_touches() {
        let mut input = InputMap::new(vec![
            Binding {
                control: "TouchLeft".into(),
                action: Action::Forward,
            },
            Binding {
                control: "TouchRight".into(),
                action: Action::Forward,
            },
            Binding {
                control: "Pad0".into(),
                action: Action::Cast(2),
            },
        ])
        .unwrap();
        assert_eq!(input.change("Pad0", true), Some(Action::Cast(2)));
        assert_eq!(input.change("Pad0", true), None);
        input.change("TouchLeft", true);
        input.change("TouchRight", true);
        input.change("TouchLeft", false);
        assert!(input.held().forward);
        input.clear();
        assert!(!input.held().forward);
        input.change("TouchRight", true);
        assert!(!input.held().forward);
        assert_eq!(input.change("Pad0", true), None);
        input.change("Pad0", false);
        assert_eq!(input.change("Pad0", true), Some(Action::Cast(2)));
        input.change("TouchRight", false);
        input.change("TouchRight", true);
        assert!(input.held().forward);
    }
    #[test]
    fn remapping_a_held_control_requires_release_before_its_new_action() {
        let mut input = InputMap::default();
        input.change("TouchForward", true);
        input
            .rebind(vec![Binding {
                control: "TouchForward".into(),
                action: Action::Cast(1),
            }])
            .unwrap();
        assert_eq!(input.change("TouchForward", true), None);
        input.change("TouchForward", false);
        assert_eq!(input.change("TouchForward", true), Some(Action::Cast(1)));
    }
    #[test]
    fn rejects_duplicate_controls_and_foreign_spell_slots() {
        let b = Binding {
            control: "Pad0".into(),
            action: Action::Jump,
        };
        assert!(InputMap::new(vec![b.clone(), b]).is_err());
        assert!(
            InputMap::new(vec![Binding {
                control: "Pad0".into(),
                action: Action::Cast(10)
            }])
            .is_err()
        );
        assert!(
            InputMap::new(vec![Binding {
                control: "\n".into(),
                action: Action::Jump
            }])
            .is_err()
        );
    }
}
