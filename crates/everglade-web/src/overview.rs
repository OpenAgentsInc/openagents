//! `/home`'s camera before and as the visitor joins (#12406): the whole
//! Grid from about 45 degrees, circling slowly, then a glide down behind
//! the visitor's avatar into the third-person camera; and the zoom that
//! wheel turns, pinches, and the zoom keys feed in small steps over a few
//! frames instead of one jump. Plain math, so it is tested natively.

use glam::Vec3;

/// The point the overview looks at: the plaza's center, where the shared
/// terminal stands, a little toward the spawn area.
pub const CENTER: Vec3 = Vec3::new(0.0, 0.0, -4.0);
/// The overview's distance from [`CENTER`] along the ground, m. Equal to
/// [`HEIGHT`], so the camera looks down at 45 degrees.
pub const DISTANCE: f32 = 62.0;
/// The overview's height, m.
pub const HEIGHT: f32 = 62.0;
/// One full turn of the overview, s.
pub const PERIOD: f32 = 240.0;
/// How long the glide from the overview to the avatar takes, s.
pub const GLIDE_SECONDS: f32 = 1.6;
/// How fast pending zoom is taken up: about 95% within a quarter second.
const ZOOM_RATE: f32 = 12.0;
/// Wheel lines a second while a zoom key is held.
pub const ZOOM_KEY_RATE: f32 = 5.0;

/// An eye and the point it looks at.
pub type Shot = (Vec3, Vec3);

/// The overview `seconds` after the page opened.
#[must_use]
pub fn shot(seconds: f64) -> Shot {
    let turn = if seconds.is_finite() {
        ((seconds / f64::from(PERIOD)).fract() as f32) * std::f32::consts::TAU
    } else {
        0.0
    };
    // It starts behind the spawn area, looking north over the plaza.
    let eye = CENTER + Vec3::new(-turn.sin() * DISTANCE, HEIGHT, -turn.cos() * DISTANCE);
    (eye, CENTER)
}

/// How far a glide that began at `started` is at `now` (both seconds),
/// eased: 0 at the overview, 1 behind the avatar.
#[must_use]
pub fn glide(started: f64, now: f64) -> f32 {
    let t = ((now - started) as f32 / GLIDE_SECONDS).clamp(0.0, 1.0);
    if !t.is_finite() {
        return 1.0;
    }
    t * t * (3.0 - 2.0 * t)
}

/// The shot `t` of the way from `from` to `to`. The eye arcs: it stays
/// high early and comes down late, so the glide never cuts through the
/// ground or the terminal.
#[must_use]
pub fn between(from: Shot, to: Shot, t: f32) -> Shot {
    let t = t.clamp(0.0, 1.0);
    let mut eye = from.0.lerp(to.0, t);
    let drop = 1.0 - (1.0 - t) * (1.0 - t);
    eye.y = from.0.y + (to.0.y - from.0.y) * (t * 0.35 + drop * 0.65);
    (eye, from.1.lerp(to.1, t))
}

/// Zoom waiting to be applied, in wheel lines (positive moves closer).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SmoothZoom {
    pending: f32,
    /// A held zoom key: 1 in, -1 out, 0 none.
    pub held: f32,
}

impl SmoothZoom {
    /// Adds `lines` from the wheel or a key press.
    pub fn add(&mut self, lines: f32) {
        if lines.is_finite() {
            self.pending = (self.pending + lines).clamp(-20.0, 20.0);
        }
    }

    /// The lines to apply this frame, `dt` seconds long.
    pub fn take(&mut self, dt: f32) -> f32 {
        if !dt.is_finite() || dt <= 0.0 {
            return 0.0;
        }
        self.add(self.held * ZOOM_KEY_RATE * dt);
        let share = 1.0 - (-ZOOM_RATE * dt).exp();
        let step = if self.pending.abs() < 0.02 {
            self.pending
        } else {
            self.pending * share
        };
        self.pending -= step;
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overview_looks_down_at_45_degrees_and_circles() {
        for seconds in [0.0, 30.0, 117.0, 239.0] {
            let (eye, target) = shot(seconds);
            let d = eye - target;
            let ground = d.x.hypot(d.z);
            let pitch = d.y.atan2(ground).to_degrees();
            assert!((pitch - 45.0).abs() < 0.5, "{pitch}");
            // The pylons' outer ring (36 m) is in front of the eye.
            assert!(ground > 36.0);
        }
        assert_ne!(shot(0.0).0, shot(10.0).0);
        assert!((shot(0.0).0 - shot(240.0).0).length() < 1e-2);
        assert!(shot(f64::NAN).0.is_finite());
    }

    #[test]
    fn the_glide_eases_from_the_overview_to_the_avatar() {
        assert_eq!(glide(10.0, 10.0), 0.0);
        assert_eq!(glide(10.0, 10.0 + f64::from(GLIDE_SECONDS)), 1.0);
        assert_eq!(glide(10.0, 99.0), 1.0);
        let from = shot(0.0);
        let to = (Vec3::new(1.0, 3.0, -20.0), Vec3::new(1.0, 1.6, -11.0));
        assert_eq!(between(from, to, 0.0), from);
        let end = between(from, to, 1.0);
        assert!((end.0 - to.0).length() < 1e-4 && (end.1 - to.1).length() < 1e-4);
        // Smooth: no frame moves the eye more than a few meters at 60 Hz.
        let mut last = from.0;
        for i in 1..=96 {
            let t = glide(0.0, f64::from(i) / 60.0);
            let (eye, _) = between(from, to, t);
            assert!(eye.distance(last) < 4.0, "{i}");
            assert!(eye.y >= to.0.y - 1e-3);
            last = eye;
        }
    }

    #[test]
    fn zoom_arrives_over_a_few_frames_and_adds_up_exactly() {
        let mut zoom = SmoothZoom::default();
        zoom.add(3.0);
        let first = zoom.take(1.0 / 60.0);
        assert!(first > 0.0 && first < 1.0, "{first}");
        let mut total = first;
        for _ in 0..60 {
            total += zoom.take(1.0 / 60.0);
        }
        assert!((total - 3.0).abs() < 1e-4, "{total}");
        assert_eq!(zoom.take(1.0 / 60.0), 0.0);
        // A held key zooms steadily.
        zoom.held = -1.0;
        let mut out = 0.0;
        for _ in 0..120 {
            out += zoom.take(1.0 / 60.0);
        }
        assert!(out < -ZOOM_KEY_RATE * 1.5, "{out}");
        assert_eq!(zoom.take(f32::NAN), 0.0);
    }
}
