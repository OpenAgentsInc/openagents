//! The third-person camera that follows the player.
//!
//! Mouse rules follow Ruins of Atlantis, which follows World of Warcraft:
//!
//! - Left drag orbits the camera around the character without turning it.
//! - Right drag is mouselook: the character snaps to the camera's heading
//!   and then turns with the mouse.
//! - The wheel zooms.
//! - While the character moves and the left button is up, the orbit
//!   swings back behind it.

use glam::{Mat4, Vec3};

/// Radians of turn per pixel of mouse travel.
pub const SENSITIVITY: f32 = 0.004;
/// Lowest pitch in radians, nearly straight up.
pub const MIN_PITCH: f32 = -1.45;
/// Highest pitch in radians, nearly straight down.
pub const MAX_PITCH: f32 = 1.45;
/// Nearest zoom in meters.
pub const MIN_DISTANCE: f32 = 2.5;
/// Farthest zoom in meters.
pub const MAX_DISTANCE: f32 = 40.0;
/// Height above the feet the camera looks at, in meters.
pub const FOCUS_HEIGHT: f32 = 1.6;
/// Vertical field of view in radians.
pub const FOV_Y: f32 = 1.0;
/// Further zoom, as the natural log of the distance ratio, that carries the
/// camera from the nearest orbit into first person, or back out. The
/// margin keeps a pinch's small reversals from flipping the view.
pub const FIRST_PERSON_PUSH: f32 = 0.15;

/// The orbit around the player, relative to the player's facing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowCamera {
    /// Orbit angle added to the player's yaw, in radians.
    pub yaw_offset: f32,
    /// Downward view angle in radians. Negative values look up.
    pub pitch: f32,
    /// Distance from the focus point, in meters. In first person it keeps
    /// the nearest orbit, which zooming out returns to.
    pub distance: f32,
    /// The eye is at the player's head and the avatar is hidden.
    pub first_person: bool,
    /// Zoom accumulated past the nearest orbit (or, in first person, back
    /// out), toward [`FIRST_PERSON_PUSH`].
    push: f32,
}

impl Default for FollowCamera {
    fn default() -> Self {
        Self {
            yaw_offset: 0.0,
            pitch: 0.28,
            distance: 9.0,
            first_person: false,
            push: 0.0,
        }
    }
}

impl FollowCamera {
    /// Left drag: orbit without turning the character.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw_offset = crate::controller::wrap(self.yaw_offset - dx * SENSITIVITY);
        self.tilt(dy);
    }

    /// Right drag: turn the character, which carries the camera with it.
    /// Returns the change to apply to the player's yaw.
    pub fn mouselook(&mut self, dx: f32, dy: f32) -> f32 {
        self.tilt(dy);
        -dx * SENSITIVITY
    }

    /// Right button pressed: hand the orbit angle to the character so it
    /// faces where the camera looks. Returns the change to the player's yaw.
    pub fn take_offset(&mut self) -> f32 {
        std::mem::take(&mut self.yaw_offset)
    }

    /// Wheel: positive `lines` zooms in.
    pub fn zoom(&mut self, lines: f32) {
        self.distance = (self.distance * 0.88f32.powf(lines)).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Pinch: a scale above one moves the camera closer.
    /// The caller validates the incremental scale before applying it.
    pub fn pinch(&mut self, scale: f32) {
        self.distance = (self.distance / scale).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Zooms by `factor`, where a factor above one moves the camera closer.
    /// When `allow_first_person` is set, zooming in past the nearest orbit
    /// enters first person, and zooming back out leaves it.
    pub fn zoom_by(&mut self, factor: f32, allow_first_person: bool) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let step = factor.ln();
        if self.first_person {
            if !allow_first_person {
                self.leave_first_person();
                self.zoom_by(factor, false);
                return;
            }
            // Only zooming out counts toward leaving first person.
            self.push = if step < 0.0 { self.push - step } else { 0.0 };
            if self.push >= FIRST_PERSON_PUSH {
                self.leave_first_person();
            }
            return;
        }
        let wanted = self.distance / factor;
        if allow_first_person && step > 0.0 && wanted < MIN_DISTANCE {
            // Only the part of this step beyond the nearest orbit counts.
            self.push += (MIN_DISTANCE / wanted).ln();
            self.distance = MIN_DISTANCE;
            if self.push >= FIRST_PERSON_PUSH {
                self.first_person = true;
                self.push = 0.0;
            }
        } else {
            self.push = 0.0;
            self.distance = wanted.clamp(MIN_DISTANCE, MAX_DISTANCE);
        }
    }

    /// Returns to the nearest third-person orbit.
    pub fn leave_first_person(&mut self) {
        if self.first_person {
            self.first_person = false;
            self.distance = MIN_DISTANCE;
        }
        self.push = 0.0;
    }

    /// Swings the orbit back behind a moving character.
    pub fn settle(&mut self, dt: f32) {
        self.yaw_offset *= 0.02f32.powf(dt);
    }

    fn tilt(&mut self, dy: f32) {
        self.pitch = (self.pitch + dy * SENSITIVITY).clamp(MIN_PITCH, MAX_PITCH);
    }

    /// The eye position for a player at `feet` facing `player_yaw`.
    #[must_use]
    pub fn eye(&self, feet: Vec3, player_yaw: f32) -> Vec3 {
        let mut eye = self.unclamped_eye(feet, player_yaw);
        eye.y = eye.y.max(0.4);
        eye
    }

    /// Orbit position before applying the active world's ground clearance.
    pub(crate) fn unclamped_eye(&self, feet: Vec3, player_yaw: f32) -> Vec3 {
        if self.first_person {
            return focus(feet);
        }
        let yaw = player_yaw + self.yaw_offset;
        let back = -crate::controller::forward(yaw) * self.pitch.cos();
        focus(feet) + (back + Vec3::Y * self.pitch.sin()) * self.distance
    }

    /// The combined projection and view matrix.
    #[must_use]
    pub fn view_proj(&self, feet: Vec3, player_yaw: f32, aspect: f32) -> Mat4 {
        self.view_proj_from_eye(self.eye(feet, player_yaw), player_yaw, aspect)
    }

    /// Project from an eye whose clearance the active scene has already checked.
    pub(crate) fn view_proj_from_eye(&self, eye: Vec3, player_yaw: f32, aspect: f32) -> Mat4 {
        let direction = crate::controller::forward(player_yaw + self.yaw_offset) * self.pitch.cos()
            - Vec3::Y * self.pitch.sin();
        // Ground clearance changes the eye position, not the look angle.
        // Looking back at the shoulders after clamping the eye would prevent
        // looking up, especially when the camera is zoomed out.
        let view = Mat4::look_to_rh(eye, direction, Vec3::Y);
        let proj = Mat4::perspective_rh(FOV_Y, aspect.max(0.01), 0.1, 2000.0);
        proj * view
    }
}

fn focus(feet: Vec3) -> Vec3 {
    feet + Vec3::Y * FOCUS_HEIGHT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_starts_behind_and_above() {
        let cam = FollowCamera::default();
        let eye = cam.eye(Vec3::ZERO, 0.0);
        assert!(eye.z < -5.0, "behind a +Z-facing player, got {eye}");
        assert!(eye.y > FOCUS_HEIGHT);
        assert!(eye.x.abs() < 1e-4);
    }

    #[test]
    fn pitch_is_clamped() {
        let mut cam = FollowCamera::default();
        cam.orbit(0.0, 1e6);
        assert_eq!(cam.pitch, MAX_PITCH);
        cam.orbit(0.0, -1e6);
        assert_eq!(cam.pitch, MIN_PITCH);
    }

    #[test]
    fn the_eye_never_sinks_below_the_ground() {
        let mut cam = FollowCamera::default();
        cam.orbit(0.0, -1e6);
        cam.distance = MAX_DISTANCE;
        assert!(cam.eye(Vec3::ZERO, 0.0).y >= 0.4);
    }

    #[test]
    fn upward_view_keeps_its_pitch_when_ground_clearance_lifts_the_eye() {
        for distance in [MIN_DISTANCE, 9.0, MAX_DISTANCE] {
            for pitch in [-0.6, -1.2, MIN_PITCH] {
                let cam = FollowCamera {
                    pitch,
                    distance,
                    yaw_offset: 0.3,
                    ..Default::default()
                };
                let eye = cam.eye(Vec3::ZERO, 0.4);
                assert!(eye.y >= 0.4);
                let inverse = cam.view_proj(Vec3::ZERO, 0.4, 0.7).inverse();
                let near = inverse.project_point3(Vec3::ZERO);
                let far = inverse.project_point3(Vec3::Z * 0.9);
                let direction = (far - near).normalize();
                let actual_pitch = -direction.y.asin();
                assert!(
                    (actual_pitch - pitch).abs() < 1e-4,
                    "{actual_pitch} != {pitch}"
                );
                let heading = direction.x.atan2(direction.z);
                assert!((heading - 0.7).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn ordinary_orbit_views_still_point_at_the_player() {
        for pitch in [0.0, 0.28, 0.8, MAX_PITCH] {
            let cam = FollowCamera {
                pitch,
                ..Default::default()
            };
            let projected = cam
                .view_proj(Vec3::ZERO, 0.0, 1.5)
                .project_point3(focus(Vec3::ZERO));
            assert!(projected.x.abs() < 1e-5 && projected.y.abs() < 1e-5);
        }
    }

    #[test]
    fn mouselook_takes_the_orbit_angle() {
        let mut cam = FollowCamera::default();
        cam.orbit(-200.0, 0.0);
        let before = cam.eye(Vec3::ZERO, 0.0);
        let turn = cam.take_offset();
        assert!(turn > 0.0);
        assert_eq!(cam.yaw_offset, 0.0);
        let after = cam.eye(Vec3::ZERO, turn);
        assert!(before.distance(after) < 1e-4, "the view does not jump");
    }

    #[test]
    fn settle_swings_back_behind() {
        let mut cam = FollowCamera {
            yaw_offset: 1.0,
            ..Default::default()
        };
        for _ in 0..120 {
            cam.settle(1.0 / 60.0);
        }
        assert!(cam.yaw_offset < 0.05);
    }

    #[test]
    fn zooming_past_the_nearest_orbit_enters_first_person_and_back() {
        let mut cam = FollowCamera::default();
        // Without permission the nearest orbit is a hard stop.
        for _ in 0..50 {
            cam.zoom_by(1.1, false);
        }
        assert!(!cam.first_person);
        assert_eq!(cam.distance, MIN_DISTANCE);
        // A small overshoot, then a reversal, stays in third person.
        cam.zoom_by(1.05, true);
        assert!(!cam.first_person);
        cam.zoom_by(0.99, true);
        assert!(!cam.first_person && cam.distance > MIN_DISTANCE);
        // Pinching on carries the eye to the player's head.
        for _ in 0..4 {
            cam.zoom_by(1.06, true);
        }
        assert!(cam.first_person);
        let feet = Vec3::new(3.0, 0.0, -2.0);
        assert!(cam.eye(feet, 0.4).distance(focus(feet)) < 1e-6);
        // It looks where the orbit looked: along the heading, at its pitch.
        let inverse = cam.view_proj(feet, 0.4, 0.6).inverse();
        let near = inverse.project_point3(Vec3::ZERO);
        let far = inverse.project_point3(Vec3::Z * 0.9);
        let direction = (far - near).normalize();
        assert!((-direction.y.asin() - cam.pitch).abs() < 1e-4);
        assert!((direction.x.atan2(direction.z) - 0.4).abs() < 1e-4);
        // More zooming in and a small reversal keep first person.
        cam.zoom_by(2.0, true);
        cam.zoom_by(0.95, true);
        assert!(cam.first_person);
        // Zooming out leaves it at the nearest orbit, then keeps going.
        cam.zoom_by(0.9, true);
        assert!(!cam.first_person);
        assert_eq!(cam.distance, MIN_DISTANCE);
        cam.zoom_by(0.5, true);
        assert!((cam.distance - MIN_DISTANCE * 2.0).abs() < 1e-4);
        // A world that no longer allows it drops first person at once.
        cam.first_person = true;
        cam.zoom_by(1.1, false);
        assert!(!cam.first_person && cam.distance == MIN_DISTANCE);
    }

    #[test]
    fn zoom_is_bounded() {
        let mut cam = FollowCamera::default();
        cam.zoom(100.0);
        assert_eq!(cam.distance, MIN_DISTANCE);
        cam.zoom(-100.0);
        assert_eq!(cam.distance, MAX_DISTANCE);
    }
}
