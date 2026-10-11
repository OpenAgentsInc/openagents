//! The Roblox-style camera: third person behind the avatar, orbited by a
//! right-drag, zoomed with the wheel (first person all the way in), Shift
//! Lock over the right shoulder with a crosshair, and pulled in when a
//! wall is behind the avatar (`docs/verse/games/the-ice-cream-man.md`,
//! Camera).

use glam::{Mat4, Vec3};
use icecream_rules::geom::{Block, V2, first_hit, v};

/// Closest and farthest zoom; under [`FIRST_PERSON`] the camera is in the
/// avatar's head.
pub const MIN_ZOOM: f32 = 0.5;
pub const MAX_ZOOM: f32 = 14.0;
pub const FIRST_PERSON: f32 = 0.9;
/// Where the camera looks: the avatar's head.
pub const HEAD: f32 = 1.6;
/// The camera stays under the ceiling.
pub const CEILING: f32 = 2.95;
/// Up and down limits, in radians below the horizon.
pub const MIN_PITCH: f32 = -0.6;
pub const MAX_PITCH: f32 = 1.2;
/// Shift Lock's shoulder offset, to the right.
pub const SHOULDER: f32 = 1.1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Turn about the vertical; 0 looks toward +z.
    pub yaw: f32,
    /// Radians below the horizon.
    pub pitch: f32,
    pub zoom: f32,
    pub shift_lock: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: std::f32::consts::PI,
            pitch: 0.35,
            zoom: 8.0,
            shift_lock: false,
        }
    }
}

/// Where the camera is this frame and what it looks at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub eye: Vec3,
    pub target: Vec3,
    pub first_person: bool,
}

impl Camera {
    /// The floor direction the camera faces.
    #[must_use]
    pub fn forward(&self) -> V2 {
        v(self.yaw.sin(), self.yaw.cos())
    }

    /// The floor direction to the camera's right.
    #[must_use]
    pub fn right(&self) -> V2 {
        let f = self.forward();
        v(-f.z, f.x)
    }

    /// A walk direction for keys held: forward and back, right and left.
    #[must_use]
    pub fn walk(&self, ahead: f32, side: f32) -> V2 {
        (self.forward() * ahead + self.right() * side).clamped(1.0)
    }

    /// Turns by a mouse or finger drag of `dx`, `dy` pixels.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.006;
        self.pitch = (self.pitch + dy * 0.005).clamp(MIN_PITCH, MAX_PITCH);
    }

    /// Zooms by a wheel's `delta` (positive zooms out).
    pub fn wheel(&mut self, delta: f32) {
        self.zoom = (self.zoom * 1.1_f32.powf(delta / 100.0)).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    #[must_use]
    pub fn first_person(&self) -> bool {
        self.zoom < FIRST_PERSON
    }

    /// Whether the crosshair shows and tools aim where the camera looks.
    #[must_use]
    pub fn aims(&self) -> bool {
        self.shift_lock || self.first_person()
    }

    /// The camera for an avatar standing at `at`, `feet` above the floor,
    /// pulled in by any wall between the avatar and the camera.
    #[must_use]
    pub fn view(&self, at: V2, feet: f32, blocks: &[Block]) -> View {
        let head = Vec3::new(at.x, feet + HEAD, at.z);
        let look = Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            -self.pitch.sin(),
            self.pitch.cos() * self.yaw.cos(),
        );
        if self.first_person() {
            return View {
                eye: head,
                target: head + look,
                first_person: true,
            };
        }
        let mut target = head;
        if self.shift_lock {
            let r = self.right();
            target += Vec3::new(r.x, 0.0, r.z) * SHOULDER * 0.6 + Vec3::Y * 0.15;
        }
        let mut zoom = self.zoom;
        let back = target - look * zoom;
        let from = v(target.x, target.z);
        let to = v(back.x, back.z);
        if let Some(t) = first_hit(blocks, from, to, 0.25, 1.0) {
            zoom = (zoom * t - 0.1).max(0.4);
        }
        let mut eye = target - look * zoom;
        eye.y = eye.y.clamp(0.3, CEILING);
        View {
            eye,
            target,
            first_person: false,
        }
    }

    /// The view and projection matrix for `view` at an aspect ratio.
    #[must_use]
    pub fn matrix(view: &View, aspect: f32) -> Mat4 {
        let fov = if aspect < 1.0 { 70.0_f32 } else { 60.0 };
        Mat4::perspective_rh_gl(fov.to_radians(), aspect, 0.08, 120.0)
            * Mat4::look_at_rh(view.eye, view.target, Vec3::Y)
    }
}

/// Where a ray from `eye` along `dir` meets the plane `y = height`, if it
/// goes that way.
#[must_use]
pub fn ray_to_height(eye: Vec3, dir: Vec3, height: f32) -> Option<V2> {
    if dir.y.abs() < 1e-5 {
        return None;
    }
    let t = (height - eye.y) / dir.y;
    (t > 0.0).then(|| {
        let p = eye + dir * t;
        v(p.x, p.z)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall_behind() -> Vec<Block> {
        vec![Block {
            min: v(-10.0, -3.15),
            max: v(10.0, -2.85),
            height: 3.2,
        }]
    }

    #[test]
    fn keys_walk_where_the_camera_faces() {
        let camera = Camera {
            yaw: 0.0,
            ..Camera::default()
        };
        let ahead = camera.walk(1.0, 0.0);
        assert!((ahead.z - 1.0).abs() < 1e-5 && ahead.x.abs() < 1e-5);
        let right = camera.walk(0.0, 1.0);
        assert!((right.x + 1.0).abs() < 1e-5, "{right:?}");
        let both = camera.walk(1.0, 1.0);
        assert!(
            (both.length() - 1.0).abs() < 1e-5,
            "diagonals aren't faster"
        );
    }

    #[test]
    fn the_camera_sits_behind_and_above_and_pulls_in_at_walls() {
        let camera = Camera {
            yaw: 0.0,
            pitch: 0.3,
            zoom: 8.0,
            shift_lock: false,
        };
        let open = camera.view(V2::ZERO, 0.0, &[]);
        assert!(open.eye.z < -6.0, "behind: {:?}", open.eye);
        assert!(open.eye.y > HEAD && open.eye.y <= CEILING);
        let walled = camera.view(V2::ZERO, 0.0, &wall_behind());
        assert!(walled.eye.z > -2.9, "pulled in: {:?}", walled.eye);
    }

    #[test]
    fn zooming_all_the_way_in_is_first_person() {
        let mut camera = Camera::default();
        for _ in 0..100 {
            camera.wheel(-100.0);
        }
        assert_eq!(camera.zoom, MIN_ZOOM);
        assert!(camera.first_person() && camera.aims());
        let view = camera.view(v(2.0, 3.0), 0.0, &[]);
        assert!(view.first_person);
        assert_eq!(view.eye, Vec3::new(2.0, HEAD, 3.0));
        for _ in 0..100 {
            camera.wheel(100.0);
        }
        assert_eq!(camera.zoom, MAX_ZOOM);
    }

    #[test]
    fn shift_lock_sits_over_the_right_shoulder() {
        let camera = Camera {
            yaw: 0.0,
            shift_lock: true,
            ..Camera::default()
        };
        assert!(camera.aims());
        let view = camera.view(V2::ZERO, 0.0, &[]);
        assert!(view.target.x < -0.3, "to the right, which is -x here");
    }

    #[test]
    fn orbiting_turns_and_tilts_within_limits() {
        let mut camera = Camera::default();
        let yaw = camera.yaw;
        camera.orbit(100.0, 0.0);
        assert!(camera.yaw < yaw);
        camera.orbit(0.0, 10_000.0);
        assert_eq!(camera.pitch, MAX_PITCH);
        camera.orbit(0.0, -10_000.0);
        assert_eq!(camera.pitch, MIN_PITCH);
    }

    #[test]
    fn a_ray_meets_the_aim_height() {
        let hit = ray_to_height(Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, -1.0, 1.0), 1.0).unwrap();
        assert!((hit.z - 2.0).abs() < 1e-5);
        assert!(ray_to_height(Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, 1.0, 1.0), 1.0).is_none());
    }
}
