//! Client glue: input state and a simple third‑person controller.
//!
//! Scaffolds added for replication and GPU upload coordination live in
//! `replication`, `upload`, and `systems` modules.

#![deny(warnings, clippy::all, clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::struct_excessive_bools
)]

pub mod input {
    /// Input snapshot for one frame of local player intent.
    ///
    /// This mirrors WoW-style semantics:
    /// - `turn_left/right` apply keyboard yaw when not in mouselook (RMB not held)
    /// - `strafe_left/right` apply lateral movement; also used when RMB is held and A/D are pressed
    /// - `forward`/`backward` are movement intents; `click_move_forward` reflects LMB+RMB chord
    /// - `mouse_look` indicates RMB (or pointer-locked look) is active
    #[derive(Default, Debug, Clone, Copy)]
    pub struct InputState {
        pub forward: bool,
        pub backward: bool,
        pub strafe_left: bool,
        pub strafe_right: bool,
        pub turn_left: bool,
        pub turn_right: bool,
        pub click_move_forward: bool,
        pub mouse_look: bool,
        /// One-shot jump press for this frame. The renderer should set this
        /// to true on key-press and clear it after passing the snapshot to
        /// `SceneInputs` so holding Space does not repeat-jump.
        pub jump_pressed: bool,
        // Legacy: not used by WoW controller (run is default); retained for compatibility
        pub run: bool,
    }
    impl InputState {
        pub fn clear(&mut self) {
            *self = Self::default();
        }
    }
}

pub mod controller {
    use super::input::InputState;
    use glam::Vec3;

    #[derive(Debug, Clone, Copy)]
    pub struct PlayerController {
        pub pos: Vec3,
        pub yaw: f32,
        // WoW-style toggles
        pub autorun: bool,
        pub walk_mode: bool,
        // Simple vertical motion state for jump/gravity
        vel_y: f32,
        ground_y: f32,
        airborne: bool,
    }
    impl PlayerController {
        #[must_use]
        pub fn new(initial_pos: Vec3) -> Self {
            Self {
                pos: initial_pos,
                yaw: 0.0,
                autorun: false,
                walk_mode: false,
                vel_y: 0.0,
                ground_y: initial_pos.y,
                airborne: false,
            }
        }
        /// Toggle autorun (behaves like holding `W` until canceled).
        pub fn toggle_autorun(&mut self) {
            self.autorun = !self.autorun;
        }
        /// Cancel autorun immediately (e.g., on `S`).
        pub fn cancel_autorun(&mut self) {
            self.autorun = false;
        }
        /// Toggle walk mode (reduces speed to walk).
        pub fn toggle_walk(&mut self) {
            self.walk_mode = !self.walk_mode;
        }

        /// True while the controller is off the ground (jumping/falling).
        #[must_use]
        pub fn airborne(&self) -> bool {
            self.airborne
        }

        /// Provide latest ground height under the controller (terrain or collider).
        /// - If grounded, snaps the character to this height and updates current ground.
        /// - If airborne, raises the ground plane only (landing on raised terrain works).
        pub fn set_ground_height(&mut self, h: f32) {
            if self.airborne {
                // Only allow the floor to rise underneath while jumping (e.g., over a slope)
                if h > self.ground_y {
                    self.ground_y = h;
                }
            } else {
                self.ground_y = h;
                if self.pos.y < h {
                    self.pos.y = h;
                }
            }
        }

        /// Update WoW-style character controller: turn/strafe/autorun.
        pub fn update(&mut self, input: &InputState, dt: f32, _cam_forward: Vec3) {
            // Constants converted from yards/sec
            const RUN_SPEED: f32 = 6.4008; // 7.0 yd/s
            const WALK_SPEED: f32 = 2.2860; // 2.5 yd/s
            const BACKPEDAL_SPEED: f32 = 4.1148; // 4.5 yd/s
            const TURN_SPEED_DEG: f32 = 180.0; // keyboard turn
            const SPRINT_MULT: f32 = 1.3; // forward-only speed boost when Shift is held
            const GRAVITY: f32 = 9.81; // m/s^2
            const JUMP_VELOCITY: f32 = 4.6; // m/s
            let turn_speed = TURN_SPEED_DEG.to_radians();

            // 1) Modes & modifiers
            let mut fwd_intent = input.forward || input.click_move_forward || self.autorun;
            if input.backward {
                fwd_intent = false; /* backpedal wins */
            }

            // 2) Intent axes (camera-relative)
            let mut fwd = 0.0;
            if fwd_intent {
                fwd += 1.0;
            }
            if input.backward {
                fwd = -1.0;
            }
            let mut strafe = 0.0;
            if input.strafe_left {
                strafe -= 1.0;
            }
            if input.strafe_right {
                strafe += 1.0;
            }

            // 3) Choose speed bucket
            let base_speed = if input.backward && fwd <= 0.0 {
                BACKPEDAL_SPEED
            } else if self.walk_mode {
                WALK_SPEED
            } else if input.run {
                RUN_SPEED * SPRINT_MULT
            } else {
                RUN_SPEED
            };

            // 4) Orientation rules (CCW-positive yaw)
            if !input.mouse_look {
                // Keyboard turn: A turns left (increase yaw), D turns right (decrease yaw)
                if input.turn_left {
                    self.yaw = wrap_angle(self.yaw + turn_speed * dt);
                }
                if input.turn_right {
                    self.yaw = wrap_angle(self.yaw - turn_speed * dt);
                }
            }

            // 5) Build world-space velocity using yaw basis (avoids sign mismatches)
            let mut mx = 0.0f32; // right(+)/left(-)
            let mut mz = 0.0f32; // forward(+)/back(-)
            mz += if fwd > 0.0 {
                1.0
            } else if fwd < 0.0 {
                -1.0
            } else {
                0.0
            };
            mx += if strafe > 0.0 {
                1.0
            } else if strafe < 0.0 {
                -1.0
            } else {
                0.0
            };
            if mx != 0.0 || mz != 0.0 {
                let (s, c) = self.yaw.sin_cos();
                let basis_fwd = glam::Vec3::new(s, 0.0, c);
                let basis_right = glam::Vec3::new(c, 0.0, -s);
                let mut v = basis_right * mx + basis_fwd * mz;
                if v.length_squared() > 1.0 {
                    v = v.normalize();
                }
                self.pos += v * base_speed * dt;
            }

            // 6) Jump/gravity vertical integration
            if self.airborne {
                // Apply gravity and integrate vertical position
                self.vel_y -= GRAVITY * dt;
                self.pos.y += self.vel_y * dt;
                // Land on ground plane
                if self.pos.y <= self.ground_y {
                    self.pos.y = self.ground_y;
                    self.vel_y = 0.0;
                    self.airborne = false;
                }
            } else {
                // Snap fully to the current ground height
                self.pos.y = self.ground_y;
                // Start jump if requested
                if input.jump_pressed {
                    self.airborne = true;
                    self.vel_y = JUMP_VELOCITY;
                    self.pos.y += self.vel_y * dt; // immediate lift so it feels responsive
                }
            }
        }
    }
    fn wrap_angle(a: f32) -> f32 {
        let mut x = a;
        while x > std::f32::consts::PI {
            x -= std::f32::consts::TAU;
        }
        while x < -std::f32::consts::PI {
            x += std::f32::consts::TAU;
        }
        x
    }
}

/// Replication apply scaffolding and buffers.
pub mod replication;
/// Placeholder for client-side systems (prediction/lag-comp/etc.).
pub mod systems;
pub mod facade {
    pub mod controller;
}
/// Mesh upload interface used by renderer or client runtime integration.
pub mod upload;
/// Client-side zone snapshot loader (CPU presentation for renderer).
pub mod zone_client;
