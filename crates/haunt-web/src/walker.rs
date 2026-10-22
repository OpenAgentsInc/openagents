//! Doom walking: a fast, smooth first-person glide through boxy halls.
//! Eye at 1.6 m, walk 4.5 m/s and run 7 m/s with a quick ease in and out,
//! a small head bob, wall sliding, no jump, and pitch held to ±35°. Every
//! number is a starting value for playtests.

use glam::Vec3;
use haunt_rules::walk::{RADIUS, Walls};

/// The walker's numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// Eye height, m.
    pub eye: f32,
    /// Walking speed, m/s.
    pub walk: f32,
    /// Running speed, m/s.
    pub run: f32,
    /// How fast speed eases toward the target, 1/s.
    pub ease: f32,
    /// Head bob height, m.
    pub bob: f32,
    /// Metres per step of the bob.
    pub stride: f32,
    /// The most the view tilts up or down, degrees.
    pub pitch_limit: f32,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            eye: 1.6,
            walk: 4.5,
            run: 7.0,
            ease: 14.0,
            bob: 0.035,
            stride: 1.7,
            pitch_limit: 35.0,
        }
    }
}

/// What the player asks for this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Move {
    /// Forward (+) or back (-), -1 to 1.
    pub forward: f32,
    /// Right (+) or left (-), -1 to 1.
    pub strafe: f32,
    pub run: bool,
    /// Turn right by this many degrees.
    pub turn: f32,
    /// Look up by this many degrees.
    pub look: f32,
}

/// The player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walker {
    /// Feet on the ground, (x, z).
    pub pos: [f32; 2],
    /// Degrees; 0 faces north, 90 east.
    pub yaw: f32,
    /// Degrees; up is positive.
    pub pitch: f32,
    vel: [f32; 2],
    /// Distance walked, for the bob.
    walked: f32,
    pub tuning: Tuning,
}

impl Walker {
    #[must_use]
    pub fn new(pos: [f32; 2], yaw: f32) -> Self {
        Self {
            pos,
            yaw,
            pitch: 0.0,
            vel: [0.0; 2],
            walked: 0.0,
            tuning: Tuning::default(),
        }
    }

    /// Facing along the ground.
    #[must_use]
    pub fn heading(&self) -> [f32; 2] {
        let (s, c) = self.yaw.to_radians().sin_cos();
        [s, c]
    }

    /// The way the eye looks, with pitch.
    #[must_use]
    pub fn forward(&self) -> Vec3 {
        let (s, c) = self.yaw.to_radians().sin_cos();
        let (ps, pc) = self.pitch.to_radians().sin_cos();
        Vec3::new(s * pc, ps, c * pc)
    }

    /// The eye, with the head bob.
    #[must_use]
    pub fn eye(&self) -> Vec3 {
        let bob = (self.walked / self.tuning.stride * std::f32::consts::TAU)
            .sin()
            .abs()
            * self.tuning.bob
            * (self.speed() / self.tuning.walk).min(1.0);
        Vec3::new(self.pos[0], self.tuning.eye + bob, self.pos[1])
    }

    /// Ground speed, m/s.
    #[must_use]
    pub fn speed(&self) -> f32 {
        self.vel[0].hypot(self.vel[1])
    }

    /// One frame: turn, ease the velocity toward what's asked, and move,
    /// sliding along walls.
    pub fn step(&mut self, walls: &Walls, m: Move, dt: f32) {
        let t = self.tuning;
        self.yaw = (self.yaw + m.turn).rem_euclid(360.0);
        self.pitch = (self.pitch + m.look).clamp(-t.pitch_limit, t.pitch_limit);
        let [hx, hz] = self.heading();
        // Right of north (0, 1) is east (1, 0).
        let (rx, rz) = (hz, -hx);
        let mut want = [
            hx * m.forward + rx * m.strafe,
            hz * m.forward + rz * m.strafe,
        ];
        let len = want[0].hypot(want[1]);
        if len > 1.0 {
            want = [want[0] / len, want[1] / len];
        }
        let top = if m.run { t.run } else { t.walk };
        let target = [want[0] * top, want[1] * top];
        let k = 1.0 - (-t.ease * dt).exp();
        self.vel[0] += (target[0] - self.vel[0]) * k;
        self.vel[1] += (target[1] - self.vel[1]) * k;
        let before = self.pos;
        self.pos = walls.slide(self.pos, [self.vel[0] * dt, self.vel[1] * dt], RADIUS);
        let moved = [self.pos[0] - before[0], self.pos[1] - before[1]];
        // Running into a wall loses the speed into it.
        if dt > 0.0 {
            self.vel = [moved[0] / dt, moved[1] / dt];
        }
        self.walked += moved[0].hypot(moved[1]);
    }

    /// Turns toward a point at up to `rate` degrees a second; true once
    /// facing it.
    pub fn face(&mut self, at: [f32; 2], rate: f32, dt: f32) -> bool {
        let want = (at[0] - self.pos[0])
            .atan2(at[1] - self.pos[1])
            .to_degrees();
        let off = (want - self.yaw + 540.0).rem_euclid(360.0) - 180.0;
        let turn = off.clamp(-rate * dt, rate * dt);
        self.yaw = (self.yaw + turn).rem_euclid(360.0);
        (off - turn).abs() < 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walking_eases_up_to_speed_and_stops_quickly() {
        let house = haunt_rules::v1();
        let walls = Walls::of(&house, haunt_assets::catalog());
        let mut w = Walker::new([8.0, 4.0], 180.0);
        let go = Move {
            forward: 1.0,
            ..Move::default()
        };
        w.step(&walls, go, 1.0 / 60.0);
        assert!(w.speed() < 1.5, "eases in: {}", w.speed());
        for _ in 0..20 {
            w.step(&walls, go, 1.0 / 60.0);
        }
        assert!((w.speed() - 4.5).abs() < 0.3, "{}", w.speed());
        assert!(w.pos[1] < 4.0, "walked south");
        for _ in 0..15 {
            w.step(&walls, Move::default(), 1.0 / 60.0);
        }
        assert!(w.speed() < 0.2, "stops: {}", w.speed());
        let run = Move {
            forward: 1.0,
            run: true,
            ..Move::default()
        };
        w.yaw = 0.0;
        for _ in 0..30 {
            w.step(&walls, run, 1.0 / 60.0);
        }
        assert!((w.speed() - 7.0).abs() < 0.4, "{}", w.speed());
    }

    #[test]
    fn the_view_tilts_only_so_far_and_the_eye_bobs_a_little() {
        let house = haunt_rules::v1();
        let walls = Walls::of(&house, haunt_assets::catalog());
        let mut w = Walker::new([8.0, 8.0], 0.0);
        w.step(
            &walls,
            Move {
                look: 80.0,
                ..Move::default()
            },
            0.016,
        );
        assert_eq!(w.pitch, 35.0);
        let mut lowest = f32::MAX;
        let mut highest = f32::MIN;
        for _ in 0..90 {
            w.step(
                &walls,
                Move {
                    forward: 1.0,
                    ..Move::default()
                },
                1.0 / 60.0,
            );
            lowest = lowest.min(w.eye().y);
            highest = highest.max(w.eye().y);
        }
        assert!(
            lowest >= 1.6 - 1e-4 && highest <= 1.64,
            "{lowest} {highest}"
        );
        assert!(highest - lowest > 0.02);
    }

    #[test]
    fn turning_right_faces_east() {
        let mut w = Walker::new([0.0, 0.0], 0.0);
        assert!(w.face([5.0, 0.0], 720.0, 1.0));
        assert!((w.yaw - 90.0).abs() < 1e-3);
        let f = w.forward();
        assert!(f.x > 0.99);
    }
}
