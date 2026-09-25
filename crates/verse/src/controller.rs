//! The player controller: WoW-style movement over a flat world.
//!
//! Reimplemented from the Ruins of Atlantis `client_core` controller
//! (`PlayerController` and its mouselook rules), not copied:
//!
//! - `W` runs forward, `S` backpedals at a slower speed and wins over `W`.
//! - `A` and `D` turn the character. While the right mouse button holds
//!   mouselook, they strafe instead. `Q` and `E` always strafe.
//! - Holding both mouse buttons runs forward.
//! - `Shift` sprints forward. `Space` jumps under gravity.
//!
//! Yaw is counterclockwise-positive around +Y. At yaw zero the character
//! faces +Z, and its right hand points at -X.

use glam::Vec3;

/// Run speed in meters per second (7 yards per second).
pub const RUN_SPEED: f32 = 6.4008;
/// Backpedal speed in meters per second (4.5 yards per second).
pub const BACKPEDAL_SPEED: f32 = 4.1148;
/// Forward speed multiplier while sprinting.
pub const SPRINT_MULT: f32 = 1.6;
/// Keyboard turn rate in radians per second.
pub const TURN_SPEED: f32 = std::f32::consts::PI;
/// Downward acceleration in meters per second squared.
pub const GRAVITY: f32 = 9.81 * 1.6;
/// Upward speed at the start of a jump, in meters per second.
pub const JUMP_VELOCITY: f32 = 6.2;
/// The character's collision radius in meters.
pub const RADIUS: f32 = 0.45;

/// The movement intent for one frame, resolved from raw keys and buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputState {
    /// `W`, or both mouse buttons.
    pub forward: bool,
    /// `S`.
    pub backward: bool,
    /// `A`: turn left, or strafe left under mouselook.
    pub left: bool,
    /// `D`: turn right, or strafe right under mouselook.
    pub right: bool,
    /// `Q`: always strafe left.
    pub strafe_left: bool,
    /// `E`: always strafe right.
    pub strafe_right: bool,
    /// Right mouse button held: the character faces the camera.
    pub mouse_look: bool,
    /// `Shift` held.
    pub sprint: bool,
    /// `Space` pressed this frame. One-shot: holding it does not repeat.
    pub jump: bool,
}

/// An axis-aligned footprint the character cannot walk through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Footprint {
    /// Minimum x and z, in meters.
    pub min: [f32; 2],
    /// Maximum x and z, in meters.
    pub max: [f32; 2],
}

impl Footprint {
    /// True when `(x, z)` lies inside this footprint grown by `margin`.
    #[must_use]
    pub fn contains(&self, x: f32, z: f32, margin: f32) -> bool {
        x > self.min[0] - margin
            && x < self.max[0] + margin
            && z > self.min[1] - margin
            && z < self.max[1] + margin
    }
}

/// The player character's position, facing, and vertical motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerController {
    /// Feet position in meters.
    pub pos: Vec3,
    /// Facing in radians, counterclockwise-positive around +Y.
    pub yaw: f32,
    vel_y: f32,
    /// Horizontal speed this frame, in meters per second, for animation.
    pub speed: f32,
}

impl PlayerController {
    /// A grounded character at `pos` facing `yaw`.
    #[must_use]
    pub fn new(pos: Vec3, yaw: f32) -> Self {
        Self {
            pos,
            yaw,
            vel_y: 0.0,
            speed: 0.0,
        }
    }

    /// The unit vector the character faces, on the ground plane.
    #[must_use]
    pub fn forward(&self) -> Vec3 {
        forward(self.yaw)
    }

    /// True while the character is off the ground.
    #[must_use]
    pub fn airborne(&self) -> bool {
        self.pos.y > 0.0 || self.vel_y > 0.0
    }

    /// Advances the character by `dt` seconds, then pushes it out of every
    /// footprint and back inside the square world of half-width `bound`.
    pub fn update(&mut self, input: &InputState, dt: f32, blockers: &[Footprint], bound: f32) {
        if !input.mouse_look {
            if input.left {
                self.yaw = wrap(self.yaw + TURN_SPEED * dt);
            }
            if input.right {
                self.yaw = wrap(self.yaw - TURN_SPEED * dt);
            }
        }

        let mut ahead = 0.0;
        if input.forward {
            ahead = 1.0;
        }
        if input.backward {
            ahead = -1.0;
        }
        let mut side = 0.0;
        if input.strafe_left || (input.mouse_look && input.left) {
            side -= 1.0;
        }
        if input.strafe_right || (input.mouse_look && input.right) {
            side += 1.0;
        }

        let speed = if ahead < 0.0 {
            BACKPEDAL_SPEED
        } else if input.sprint && ahead > 0.0 {
            RUN_SPEED * SPRINT_MULT
        } else {
            RUN_SPEED
        };
        let fwd = self.forward();
        let right = fwd.cross(Vec3::Y);
        let wish = fwd * ahead + right * side;
        let step = if wish.length_squared() > 0.0 {
            wish.normalize() * speed
        } else {
            Vec3::ZERO
        };
        self.speed = step.length();
        self.pos.x += step.x * dt;
        self.pos.z += step.z * dt;

        if input.jump && !self.airborne() {
            self.vel_y = JUMP_VELOCITY;
        }
        if self.airborne() {
            self.vel_y -= GRAVITY * dt;
            self.pos.y += self.vel_y * dt;
            if self.pos.y <= 0.0 {
                self.pos.y = 0.0;
                self.vel_y = 0.0;
            }
        }

        for block in blockers {
            self.push_out(block);
        }
        let limit = bound - RADIUS;
        self.pos.x = self.pos.x.clamp(-limit, limit);
        self.pos.z = self.pos.z.clamp(-limit, limit);
    }

    /// Moves the character out of `block` along the shallowest axis.
    fn push_out(&mut self, block: &Footprint) {
        let (x, z) = (self.pos.x, self.pos.z);
        if !block.contains(x, z, RADIUS) {
            return;
        }
        let exits = [
            (block.min[0] - RADIUS - x, 0.0),
            (block.max[0] + RADIUS - x, 0.0),
            (0.0, block.min[1] - RADIUS - z),
            (0.0, block.max[1] + RADIUS - z),
        ];
        let (dx, dz) = exits
            .into_iter()
            .min_by(|a, b| (a.0.abs() + a.1.abs()).total_cmp(&(b.0.abs() + b.1.abs())))
            .unwrap_or((0.0, 0.0));
        self.pos.x += dx;
        self.pos.z += dz;
    }
}

/// The ground-plane unit vector for `yaw`.
#[must_use]
pub fn forward(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

/// Wraps an angle into `(-PI, PI]`.
#[must_use]
pub fn wrap(angle: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let a = (angle + PI).rem_euclid(TAU) - PI;
    if a <= -PI { a + TAU } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPEN: f32 = 1000.0;

    fn run(input: InputState, seconds: f32) -> PlayerController {
        let mut pc = PlayerController::new(Vec3::ZERO, 0.0);
        let dt = 1.0 / 60.0;
        for _ in 0..(seconds / dt) as usize {
            pc.update(&input, dt, &[], OPEN);
        }
        pc
    }

    #[test]
    fn w_runs_forward_along_positive_z() {
        let pc = run(
            InputState {
                forward: true,
                ..Default::default()
            },
            1.0,
        );
        assert!((pc.pos.z - RUN_SPEED).abs() < 0.2, "z = {}", pc.pos.z);
        assert!(pc.pos.x.abs() < 1e-4);
    }

    #[test]
    fn s_backpedals_slower_and_wins_over_w() {
        let pc = run(
            InputState {
                forward: true,
                backward: true,
                ..Default::default()
            },
            1.0,
        );
        assert!((pc.pos.z + BACKPEDAL_SPEED).abs() < 0.2, "z = {}", pc.pos.z);
    }

    #[test]
    fn a_turns_without_mouselook_and_strafes_with_it() {
        let turned = run(
            InputState {
                left: true,
                ..Default::default()
            },
            0.5,
        );
        assert!(turned.pos.length() < 1e-4);
        assert!(turned.yaw > 1.0);

        let strafed = run(
            InputState {
                left: true,
                mouse_look: true,
                ..Default::default()
            },
            1.0,
        );
        assert_eq!(strafed.yaw, 0.0);
        assert!(strafed.pos.x > RUN_SPEED - 0.2, "left of +Z is +X");
    }

    #[test]
    fn a_jump_leaves_and_returns_to_the_ground() {
        let mut pc = PlayerController::new(Vec3::ZERO, 0.0);
        let jump = InputState {
            jump: true,
            ..Default::default()
        };
        pc.update(&jump, 1.0 / 60.0, &[], OPEN);
        assert!(pc.airborne());
        let mut peak: f32 = 0.0;
        for _ in 0..240 {
            pc.update(&InputState::default(), 1.0 / 60.0, &[], OPEN);
            peak = peak.max(pc.pos.y);
        }
        assert!(peak > 1.0, "peak = {peak}");
        assert!(!pc.airborne());
        assert_eq!(pc.pos.y, 0.0);
    }

    #[test]
    fn a_footprint_stops_the_character() {
        let wall = Footprint {
            min: [-5.0, 3.0],
            max: [5.0, 8.0],
        };
        let mut pc = PlayerController::new(Vec3::ZERO, 0.0);
        let input = InputState {
            forward: true,
            ..Default::default()
        };
        for _ in 0..120 {
            pc.update(&input, 1.0 / 60.0, &[wall], OPEN);
        }
        assert!(pc.pos.z <= 3.0 - RADIUS + 1e-3, "z = {}", pc.pos.z);
    }

    #[test]
    fn the_world_edge_holds() {
        let mut pc = PlayerController::new(Vec3::ZERO, 0.0);
        let input = InputState {
            forward: true,
            ..Default::default()
        };
        for _ in 0..600 {
            pc.update(&input, 1.0 / 60.0, &[], 10.0);
        }
        assert!(pc.pos.z <= 10.0 - RADIUS + 1e-4);
    }

    #[test]
    fn wrap_keeps_angles_in_range() {
        use std::f32::consts::PI;
        assert!((wrap(3.0 * PI) - PI).abs() < 1e-5);
        assert!((wrap(-PI / 2.0) + PI / 2.0).abs() < 1e-6);
        assert!(wrap(-PI) > 0.0);
    }
}
