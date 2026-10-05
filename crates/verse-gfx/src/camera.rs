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
//!
//! Where the world allows it, zooming in past the nearest orbit glides the
//! eye into the character's head for a first-person view, and zooming back
//! out glides it back to the nearest orbit.
//!
//! Every zone keeps the eye out of its solids the same way: each frame a
//! small sphere is cast from a pivot just over the character's head toward
//! the orbit's eye, over the zone's [`Sight`], and the eye stops just short
//! of the first hit ([`FollowCamera::frame`]). The pull-in is immediate,
//! so no frame, the first after a spawn included, puts the eye behind a
//! wall; [`FollowCamera::track`] eases it back out once the view clears.

use glam::{Mat4, Vec3};
use verse_world::social::sight::Sight;

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
/// Height of the first-person eye above the feet, in meters: just under
/// the top of the character's head.
pub const EYE_HEIGHT: f32 = verse_world::social::controller::AVATAR_HEIGHT - 0.1;
/// Seconds the eye takes to glide between the nearest orbit and first
/// person.
pub const TRANSITION_SECONDS: f32 = 0.25;
/// How far into the glide, from 0 to 1, the player's own avatar is hidden:
/// by then the eye is within half a meter of the head.
pub const HIDE_AVATAR_AT: f32 = 0.8;
/// Near clip plane in meters for the third-person orbit.
pub const NEAR: f32 = 0.1;
/// Near clip plane in meters while the eye is in or gliding toward the
/// head. The player's collision radius keeps walls far beyond the corners
/// of this plane, so a wall beside the player is not cut open.
pub const FIRST_PERSON_NEAR: f32 = 0.05;
/// Far clip plane in meters.
pub const FAR: f32 = 2000.0;
/// Radius of the sphere cast from the pivot toward the orbit's eye, m. It
/// keeps the corners of the near plane ([`FIRST_PERSON_NEAR`] while the eye
/// is pulled in) out of a wall the eye stops at.
pub const PROBE_RADIUS: f32 = 0.12;
/// Height above the feet the cast starts from, m: just over the head, so
/// the character never blocks its own view.
pub const PIVOT_HEIGHT: f32 = verse_world::social::controller::AVATAR_HEIGHT + 0.1;
/// Height the orbit's eye keeps over the ground, m.
pub const GROUND_CLEARANCE: f32 = 0.4;
/// A wall that pulls the eye nearer the pivot than this, m, would fill the
/// view with the character's back, so the avatar is hidden as in first
/// person.
pub const CLOSE: f32 = 1.0;
/// How fast a pulled-in eye eases back out once the view clears: the share
/// of the remaining distance per second, as an exponential rate, and the
/// least speed in meters per second, so the ease ends.
pub const EASE_OUT_RATE: f32 = 4.0;
pub const EASE_OUT_SPEED: f32 = 1.5;
/// A pivot that moves farther than this between tracked frames, m, was
/// carried there (a spawn, a portal, a teleport): the eye does not ease
/// out from where it was pulled in before.
const TELEPORT: f32 = 3.0;

/// Where this frame's eye is, after the zone's solids.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    /// The eye position.
    pub eye: Vec3,
    /// The eye is nearer the pivot than the orbit put it: a solid, or the
    /// ease back out after one, holds it in.
    pub limited: bool,
    /// The eye is within [`CLOSE`] of the pivot, so the player's own avatar
    /// is not drawn.
    pub close: bool,
}

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
    /// Progress of the glide into first person: 0 at the orbit, 1 at the
    /// head. [`Self::advance`] moves it toward `first_person`.
    blend: f32,
    /// How far from the pivot a solid pulled the eye in, m, while it eases
    /// back out; infinite when nothing holds it ([`Self::track`]).
    boom: f32,
    /// The pivot [`Self::track`] last saw.
    tracked: Option<Vec3>,
}

impl Default for FollowCamera {
    fn default() -> Self {
        Self {
            yaw_offset: 0.0,
            pitch: 0.28,
            distance: 9.0,
            first_person: false,
            push: 0.0,
            blend: 0.0,
            boom: f32::INFINITY,
            tracked: None,
        }
    }
}

impl FollowCamera {
    /// Left drag: orbit without turning the character.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw_offset = verse_world::social::controller::wrap(self.yaw_offset - dx * SENSITIVITY);
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

    /// Glides the eye toward first person or back out over `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let target = if self.first_person { 1.0 } else { 0.0 };
        let step = dt / TRANSITION_SECONDS;
        self.blend = if self.blend < target {
            (self.blend + step).min(target)
        } else {
            (self.blend - step).max(target)
        };
    }

    /// Progress of the glide into first person, from 0 at the orbit to 1
    /// at the head.
    #[must_use]
    pub fn blend(&self) -> f32 {
        self.blend
    }

    /// The eye is close enough to the head that the player's own avatar
    /// would fill the view, so it is not drawn.
    #[must_use]
    pub fn hides_avatar(&self) -> bool {
        self.blend >= HIDE_AVATAR_AT
    }

    /// The near clip plane for this frame.
    #[must_use]
    pub fn near(&self) -> f32 {
        if self.blend > 0.0 {
            FIRST_PERSON_NEAR
        } else {
            NEAR
        }
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
    pub fn unclamped_eye(&self, feet: Vec3, player_yaw: f32) -> Vec3 {
        let yaw = player_yaw + self.yaw_offset;
        let back = -verse_world::social::controller::forward(yaw) * self.pitch.cos();
        let orbit = focus(feet) + (back + Vec3::Y * self.pitch.sin()) * self.distance;
        if self.blend <= 0.0 {
            return orbit;
        }
        let t = self.blend.clamp(0.0, 1.0);
        orbit.lerp(head(feet), t * t * (3.0 - 2.0 * t))
    }

    /// The orbit's eye for a player at `feet` facing `player_yaw`, kept
    /// [`GROUND_CLEARANCE`] over the ground of `sight`, before its solids.
    #[must_use]
    pub fn desired(&self, feet: Vec3, player_yaw: f32, sight: &dyn Sight) -> Vec3 {
        let mut eye = self.unclamped_eye(feet, player_yaw);
        if let Some(ground) = sight.ground(eye.x, eye.z)
            && ground.is_finite()
        {
            eye.y = eye.y.max(ground + GROUND_CLEARANCE);
        }
        eye
    }

    /// This frame's eye: `desired` (see [`Self::desired`]), pulled in
    /// toward the pivot over the head of a player at `feet` so a sphere of
    /// [`PROBE_RADIUS`] there touches none of the solids of `sight`, and
    /// held in while [`Self::track`] eases it back out.
    #[must_use]
    pub fn frame(&self, feet: Vec3, desired: Vec3, sight: &dyn Sight) -> Framing {
        let open = Framing {
            eye: desired,
            limited: false,
            close: false,
        };
        if self.blend >= 1.0 || !feet.is_finite() || !desired.is_finite() {
            return open;
        }
        let (pivot, length, free) = self.reach(feet, desired, sight);
        let held = if self.boom < length {
            self.boom
        } else {
            length
        };
        let distance = free.min(held);
        if distance >= length - 1e-5 {
            return open;
        }
        let distance = distance.max(0.0);
        Framing {
            eye: pivot + (desired - pivot) / length * distance,
            limited: true,
            close: distance < CLOSE,
        }
    }

    /// Follows the solids of `sight` from one frame to the next: a solid
    /// that pulls the eye in holds it there at once, and once the view
    /// clears the eye eases back out over `dt` seconds toward the orbit the
    /// player chose.
    pub fn track(&mut self, feet: Vec3, desired: Vec3, sight: &dyn Sight, dt: f32) {
        if !feet.is_finite() || !desired.is_finite() {
            return;
        }
        let (pivot, length, free) = self.reach(feet, desired, sight);
        if self
            .tracked
            .is_none_or(|was| was.distance(pivot) > TELEPORT)
        {
            self.boom = f32::INFINITY;
        }
        self.tracked = Some(pivot);
        let target = free.min(length);
        if target < self.boom {
            self.boom = target;
        } else if self.boom.is_finite() {
            let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
            let gap = target - self.boom;
            let step = (gap * (1.0 - (-EASE_OUT_RATE * dt).exp())).max(EASE_OUT_SPEED * dt);
            self.boom += step.min(gap);
        }
        if self.boom >= length - 1e-3 {
            self.boom = f32::INFINITY;
        }
    }

    /// The pivot the cast starts from, the distance to `desired`, and how
    /// much of it is clear.
    fn reach(&self, feet: Vec3, desired: Vec3, sight: &dyn Sight) -> (Vec3, f32, f32) {
        // The pivot over the head, unless a low ceiling holds it lower.
        let focus = focus(feet);
        let over = feet + Vec3::Y * PIVOT_HEIGHT;
        let pivot = focus.lerp(over, sight.sweep(focus, over, PROBE_RADIUS));
        let length = pivot.distance(desired);
        if length < 1e-5 {
            return (pivot, length, length);
        }
        let free = sight.sweep(pivot, desired, PROBE_RADIUS).clamp(0.0, 1.0) * length;
        (pivot, length, free)
    }

    /// The combined projection and view matrix for `framing`'s eye. A
    /// pulled-in eye uses the first-person near plane, which the cast's
    /// sphere keeps clear of the wall it stopped at.
    #[must_use]
    pub fn view_proj_framed(&self, framing: Framing, player_yaw: f32, aspect: f32) -> Mat4 {
        let near = if framing.limited {
            FIRST_PERSON_NEAR
        } else {
            self.near()
        };
        self.view_proj_near(framing.eye, player_yaw, aspect, near)
    }

    /// The combined projection and view matrix.
    #[must_use]
    pub fn view_proj(&self, feet: Vec3, player_yaw: f32, aspect: f32) -> Mat4 {
        self.view_proj_from_eye(self.eye(feet, player_yaw), player_yaw, aspect)
    }

    /// Project from an eye whose clearance the active scene has already checked.
    pub fn view_proj_from_eye(&self, eye: Vec3, player_yaw: f32, aspect: f32) -> Mat4 {
        self.view_proj_near(eye, player_yaw, aspect, self.near())
    }

    fn view_proj_near(&self, eye: Vec3, player_yaw: f32, aspect: f32, near: f32) -> Mat4 {
        let direction = verse_world::social::controller::forward(player_yaw + self.yaw_offset)
            * self.pitch.cos()
            - Vec3::Y * self.pitch.sin();
        // Ground clearance changes the eye position, not the look angle.
        // Looking back at the shoulders after clamping the eye would prevent
        // looking up, especially when the camera is zoomed out.
        let view = Mat4::look_to_rh(eye, direction, Vec3::Y);
        let proj = Mat4::perspective_rh(FOV_Y, aspect.max(0.01), near, FAR);
        proj * view
    }
}

fn focus(feet: Vec3) -> Vec3 {
    feet + Vec3::Y * FOCUS_HEIGHT
}

/// The first-person eye of a player standing at `feet`.
#[must_use]
pub fn head(feet: Vec3) -> Vec3 {
    feet + Vec3::Y * EYE_HEIGHT
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
        // The eye glides in rather than jumping, and the avatar hides
        // once the eye is close to the head.
        let orbit = cam.eye(feet, 0.4);
        assert!(orbit.distance(focus(feet)) > MIN_DISTANCE - 1e-4);
        assert!(!cam.hides_avatar());
        cam.advance(TRANSITION_SECONDS * 0.5);
        let midway = cam.eye(feet, 0.4);
        assert!(midway.distance(head(feet)) < orbit.distance(head(feet)));
        assert!(midway.distance(head(feet)) > 0.1);
        assert!(!cam.hides_avatar());
        cam.advance(TRANSITION_SECONDS);
        assert!(cam.hides_avatar());
        assert!(cam.eye(feet, 0.4).distance(head(feet)) < 1e-6);
        assert_eq!(cam.near(), FIRST_PERSON_NEAR);
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
        // Zooming out leaves it at the nearest orbit, gliding back out,
        // then keeps going.
        cam.zoom_by(0.9, true);
        assert!(!cam.first_person);
        assert_eq!(cam.distance, MIN_DISTANCE);
        assert!(cam.hides_avatar(), "the avatar returns as the eye leaves");
        cam.advance(TRANSITION_SECONDS * 0.5);
        assert!(!cam.hides_avatar());
        cam.advance(TRANSITION_SECONDS);
        assert_eq!(cam.blend(), 0.0);
        assert_eq!(cam.near(), NEAR);
        assert!(cam.eye(feet, 0.4).distance(focus(feet)) > MIN_DISTANCE - 1e-4);
        cam.zoom_by(0.5, true);
        assert!((cam.distance - MIN_DISTANCE * 2.0).abs() < 1e-4);
        // A world that no longer allows it drops first person at once.
        cam.first_person = true;
        cam.zoom_by(1.1, false);
        assert!(!cam.first_person && cam.distance == MIN_DISTANCE);
    }

    #[test]
    fn the_first_person_near_plane_stays_clear_of_walls_at_arms_length() {
        // A wall can come no nearer the eye than the player's collision
        // radius. The near plane's farthest corner, at the widest aspect a
        // window or phone allows, must sit well inside that.
        let aspect = 3.0;
        let half_h = (FOV_Y * 0.5).tan() * FIRST_PERSON_NEAR;
        let corner = Vec3::new(half_h * aspect, half_h, FIRST_PERSON_NEAR).length();
        assert!(
            corner < verse_world::social::controller::RADIUS * 0.5,
            "{corner} m reaches toward a wall {} m away",
            verse_world::social::controller::RADIUS
        );
        assert!(
            EYE_HEIGHT < verse_world::social::controller::AVATAR_HEIGHT
                && EYE_HEIGHT > FOCUS_HEIGHT
        );
    }

    use verse_world::social::controller::Footprint;
    use verse_world::social::sight::{Footprints, Ground, Open};

    /// A pillar a meter square, 4 m behind a player at the origin facing +Z.
    const PILLAR: [Footprint; 1] = [Footprint {
        min: [-0.5, -4.5],
        max: [0.5, -3.5],
    }];

    fn pillar() -> Footprints<'static> {
        Footprints {
            blocks: &PILLAR,
            tops: &[],
            default_top: f32::INFINITY,
            floor: 0.0,
        }
    }

    #[test]
    fn an_open_view_leaves_the_orbit_alone() {
        let cam = FollowCamera::default();
        let desired = cam.desired(Vec3::ZERO, 0.0, &Open);
        let framing = cam.frame(Vec3::ZERO, desired, &Open);
        assert_eq!(framing.eye, cam.unclamped_eye(Vec3::ZERO, 0.0));
        assert!(!framing.limited && !framing.close);
        // Over flat ground the orbit keeps the old clearance.
        let ground = Ground(|_: f32, _: f32| 0.0);
        assert_eq!(
            cam.desired(Vec3::ZERO, 0.0, &ground),
            cam.eye(Vec3::ZERO, 0.0)
        );
    }

    #[test]
    fn a_pillar_between_pulls_the_eye_in_at_once_and_it_eases_back_out() {
        let mut cam = FollowCamera::default();
        let sight = pillar();
        let feet = Vec3::ZERO;
        let desired = cam.desired(feet, 0.0, &sight);
        // The very first frame, before any tracking, is in front of it.
        let framing = cam.frame(feet, desired, &sight);
        assert!(framing.limited);
        assert!(
            framing.eye.z > -3.5 + PROBE_RADIUS - 1e-3,
            "{}",
            framing.eye
        );
        assert!(framing.eye.z < -2.0, "{}", framing.eye);
        cam.track(feet, desired, &sight, 1.0 / 60.0);
        assert_eq!(cam.frame(feet, desired, &sight), framing);
        // The pillar goes away: the eye eases back over several frames
        // rather than popping, and ends at the chosen orbit.
        let mut last = framing.eye.distance(desired);
        let mut frames = 0;
        while cam.frame(feet, desired, &Open).limited {
            cam.track(feet, desired, &Open, 1.0 / 60.0);
            let gap = cam.frame(feet, desired, &Open).eye.distance(desired);
            assert!(gap < last, "eases out monotonically");
            assert!(last - gap < 0.5, "no pop: {last} to {gap}");
            last = gap;
            frames += 1;
            assert!(frames < 600);
        }
        assert!(frames > 5, "{frames} frames");
        assert_eq!(cam.frame(feet, desired, &Open).eye, desired);
        // The pillar back pulls the eye in again at once.
        assert!(cam.frame(feet, desired, &sight).limited);
        // The user's zoom is kept: nearer than the pillar, nothing moves.
        cam.distance = MIN_DISTANCE;
        let near = cam.desired(feet, 0.0, &sight);
        assert_eq!(cam.frame(feet, near, &sight).eye, near);
    }

    #[test]
    fn a_wall_at_the_back_hides_the_avatar_and_a_teleport_does_not_ease() {
        let wall = [Footprint {
            min: [-5.0, -1.2],
            max: [5.0, -0.5],
        }];
        let sight = Footprints {
            blocks: &wall,
            ..pillar()
        };
        let mut cam = FollowCamera::default();
        let desired = cam.desired(Vec3::ZERO, 0.0, &sight);
        let framing = cam.frame(Vec3::ZERO, desired, &sight);
        assert!(framing.close && framing.limited);
        assert!(
            framing.eye.z > -0.5 + PROBE_RADIUS - 1e-3,
            "{}",
            framing.eye
        );
        cam.track(Vec3::ZERO, desired, &sight, 0.016);
        // Carried far away, the eye is at the open orbit at once.
        let far = Vec3::new(100.0, 0.0, 100.0);
        let there = cam.desired(far, 0.0, &sight);
        cam.track(far, there, &sight, 0.016);
        assert_eq!(cam.frame(far, there, &sight).eye, there);
    }

    #[test]
    fn the_eye_never_goes_under_the_terrain() {
        let hill = Ground(|x: f32, z: f32| 2.0 + 0.3 * (x * 0.7).sin() - 0.25 * z);
        for pitch in [MIN_PITCH, -0.6, 0.0, 0.28, 1.0] {
            for yaw in [0.0, 1.0, 2.5, -2.0] {
                let mut cam = FollowCamera {
                    pitch,
                    distance: 20.0,
                    ..Default::default()
                };
                let feet = Vec3::new(1.0, (hill.0)(1.0, 2.0), 2.0);
                for _ in 0..3 {
                    let desired = cam.desired(feet, yaw, &hill);
                    cam.track(feet, desired, &hill, 0.016);
                    let eye = cam.frame(feet, desired, &hill).eye;
                    assert!(
                        eye.y > (hill.0)(eye.x, eye.z) + 0.05,
                        "{eye} at {pitch} {yaw}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_pulled_in_near_plane_stays_inside_the_probe() {
        let aspect = 3.0;
        let half_h = (FOV_Y * 0.5).tan() * FIRST_PERSON_NEAR;
        let corner = Vec3::new(half_h * aspect, half_h, FIRST_PERSON_NEAR).length();
        assert!(corner < PROBE_RADIUS, "{corner}");
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
