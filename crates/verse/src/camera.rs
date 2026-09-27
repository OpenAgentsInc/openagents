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

/// The orbit around the player, relative to the player's facing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowCamera {
    /// Orbit angle added to the player's yaw, in radians.
    pub yaw_offset: f32,
    /// Downward view angle in radians. Negative values look up.
    pub pitch: f32,
    /// Distance from the focus point, in meters.
    pub distance: f32,
}

impl Default for FollowCamera {
    fn default() -> Self {
        Self {
            yaw_offset: 0.0,
            pitch: 0.28,
            distance: 9.0,
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
        let yaw = player_yaw + self.yaw_offset;
        let back = -crate::controller::forward(yaw) * self.pitch.cos();
        let offset = (back + Vec3::Y * self.pitch.sin()) * self.distance;
        let mut eye = focus(feet) + offset;
        eye.y = eye.y.max(0.4);
        eye
    }

    /// The combined projection and view matrix.
    #[must_use]
    pub fn view_proj(&self, feet: Vec3, player_yaw: f32, aspect: f32) -> Mat4 {
        let eye = self.eye(feet, player_yaw);
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
    fn zoom_is_bounded() {
        let mut cam = FollowCamera::default();
        cam.zoom(100.0);
        assert_eq!(cam.distance, MIN_DISTANCE);
        cam.zoom(-100.0);
        assert_eq!(cam.distance, MAX_DISTANCE);
    }
}
