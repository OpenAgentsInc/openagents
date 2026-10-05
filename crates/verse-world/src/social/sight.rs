//! What the third-person camera sees through: a zone's solid geometry as a
//! sphere sweep. The camera casts a small sphere from a pivot over the
//! character's head toward where its orbit wants the eye, and stops just
//! short of the first solid ([`Sight::sweep`]). Every zone answers the same
//! question over its own geometry: [`Solids`](super::solids::Solids) over
//! its blocks, roofs, columns, and ground; [`Footprints`] over a list of
//! footprints with their tops; [`Ground`] over a heightfield alone; and
//! [`Open`] where nothing blocks the view.
//!
//! The tests are cheap: a sweep looks only at the solids whose bounds meet
//! the swept segment, and walks a heightfield or a column grid in short
//! steps.

use glam::Vec3;

use super::controller::Footprint;

/// The longest step a sweep takes over a heightfield, m.
const GROUND_STEP: f32 = 0.5;

/// A zone's solid geometry, as the camera sees it.
pub trait Sight {
    /// How far along the segment from `from` to `to`, from 0 to 1, a sphere
    /// of `radius` meters travels before it touches a solid; 1 when it
    /// reaches `to`. A solid that already holds `from` does not count.
    fn sweep(&self, from: Vec3, to: Vec3, radius: f32) -> f32;

    /// The ground's height at `(x, z)`, m, or `None` where there is no
    /// ground, as in free flight.
    fn ground(&self, x: f32, z: f32) -> Option<f32> {
        let _ = (x, z);
        None
    }
}

/// Nothing blocks the view, and there is no ground.
#[derive(Clone, Copy, Debug, Default)]
pub struct Open;

impl Sight for Open {
    fn sweep(&self, _: Vec3, _: Vec3, _: f32) -> f32 {
        1.0
    }
}

/// A heightfield and nothing standing on it.
#[derive(Clone, Copy, Debug)]
pub struct Ground<F>(pub F);

impl<F: Fn(f32, f32) -> f32> Sight for Ground<F> {
    fn sweep(&self, from: Vec3, to: Vec3, radius: f32) -> f32 {
        ground_hit(from, to, radius, &self.0).unwrap_or(1.0)
    }

    fn ground(&self, x: f32, z: f32) -> Option<f32> {
        Some((self.0)(x, z))
    }
}

/// Footprints standing on flat ground, each up to its top: the plaza's,
/// the Grid's, and the Physics Lab's blockers.
#[derive(Clone, Copy, Debug)]
pub struct Footprints<'a> {
    /// What blocks walking.
    pub blocks: &'a [Footprint],
    /// Each block's top, m, by index; a block past the end is as tall as
    /// `default_top`.
    pub tops: &'a [f32],
    /// The top of a block `tops` does not list, m.
    pub default_top: f32,
    /// The flat ground's height, m.
    pub floor: f32,
}

impl Sight for Footprints<'_> {
    fn sweep(&self, from: Vec3, to: Vec3, radius: f32) -> f32 {
        let reach = Reach::of(from, to, radius);
        let mut t = plane_hit(from, to, radius, self.floor, false).unwrap_or(1.0);
        for (i, block) in self.blocks.iter().enumerate() {
            if !reach.meets(block) {
                continue;
            }
            let top = self.tops.get(i).copied().unwrap_or(self.default_top);
            if let Some(hit) = column_hit(from, to, radius, block, f32::NEG_INFINITY, top) {
                t = t.min(hit);
            }
        }
        t
    }

    fn ground(&self, _: f32, _: f32) -> Option<f32> {
        Some(self.floor)
    }
}

/// The ground rectangle a swept segment covers, to skip solids that cannot
/// meet it.
#[derive(Clone, Copy, Debug)]
pub struct Reach {
    min: [f32; 2],
    max: [f32; 2],
}

impl Reach {
    /// The rectangle around the segment from `from` to `to`, grown by
    /// `radius`.
    #[must_use]
    pub fn of(from: Vec3, to: Vec3, radius: f32) -> Self {
        Self {
            min: [from.x.min(to.x) - radius, from.z.min(to.z) - radius],
            max: [from.x.max(to.x) + radius, from.z.max(to.z) + radius],
        }
    }

    /// Whether `footprint` overlaps the rectangle.
    #[must_use]
    pub fn meets(&self, footprint: &Footprint) -> bool {
        footprint.min[0] <= self.max[0]
            && footprint.max[0] >= self.min[0]
            && footprint.min[1] <= self.max[1]
            && footprint.max[1] >= self.min[1]
    }
}

/// Where, from 0 to 1, the segment from `from` to `to` first enters the box
/// from `min` to `max` grown by `radius` on every side, or `None` when it
/// misses it or starts inside it.
#[must_use]
pub fn box_hit(from: Vec3, to: Vec3, radius: f32, min: Vec3, max: Vec3) -> Option<f32> {
    let delta = to - from;
    let (min, max) = (min - Vec3::splat(radius), max + Vec3::splat(radius));
    let mut enter = f32::NEG_INFINITY;
    let mut leave = f32::INFINITY;
    for axis in 0..3 {
        let (o, d, lo, hi) = (from[axis], delta[axis], min[axis], max[axis]);
        if d.abs() < 1e-9 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo - o) / d, (hi - o) / d);
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    (enter <= leave && enter > 0.0 && enter <= 1.0).then_some(enter)
}

/// [`box_hit`] for a footprint standing from `bottom` to `top`, m.
#[must_use]
pub fn column_hit(
    from: Vec3,
    to: Vec3,
    radius: f32,
    footprint: &Footprint,
    bottom: f32,
    top: f32,
) -> Option<f32> {
    // A finite stand-in for an unbounded bottom or top keeps the slab test's
    // arithmetic finite.
    let bottom = bottom.max(-1.0e5);
    let top = top.min(1.0e5);
    box_hit(
        from,
        to,
        radius,
        Vec3::new(footprint.min[0], bottom, footprint.min[1]),
        Vec3::new(footprint.max[0], top, footprint.max[1]),
    )
}

/// Where the segment crosses the horizontal plane at `height`, kept
/// `radius` above it (or below it, for a `ceiling`), or `None` when it does
/// not, or starts on the wrong side.
#[must_use]
pub fn plane_hit(from: Vec3, to: Vec3, radius: f32, height: f32, ceiling: bool) -> Option<f32> {
    let (limit, start, end) = if ceiling {
        (height - radius, -from.y, -to.y)
    } else {
        (height + radius, from.y, to.y)
    };
    let limit = if ceiling { -limit } else { limit };
    (start >= limit && end < limit).then(|| (start - limit) / (start - end))
}

/// Where the segment first comes within `radius` above the heightfield
/// `ground`, found by stepping and then bisecting, or `None` when it stays
/// clear or starts too low.
pub fn ground_hit(
    from: Vec3,
    to: Vec3,
    radius: f32,
    ground: &impl Fn(f32, f32) -> f32,
) -> Option<f32> {
    let below = |t: f32| {
        let p = from.lerp(to, t);
        p.y < ground(p.x, p.z) + radius
    };
    if below(0.0) {
        return None;
    }
    let steps = ((from.distance(to) / GROUND_STEP).ceil() as usize).clamp(1, 128);
    let mut previous = 0.0;
    for k in 1..=steps {
        let t = k as f32 / steps as f32;
        if below(t) {
            let (mut lo, mut hi) = (previous, t);
            for _ in 0..8 {
                let mid = 0.5 * (lo + hi);
                if below(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return Some(lo);
        }
        previous = t;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_on_the_line_stops_the_sweep_short_of_its_face() {
        let hit = box_hit(
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            0.1,
            Vec3::new(4.0, -1.0, -1.0),
            Vec3::new(5.0, 1.0, 1.0),
        )
        .unwrap();
        assert!((hit - 0.39).abs() < 1e-5, "{hit}");
        // Off the line, behind the start, and around the start: no hit.
        for (min, max) in [
            (Vec3::new(4.0, 2.0, -1.0), Vec3::new(5.0, 3.0, 1.0)),
            (Vec3::new(-5.0, -1.0, -1.0), Vec3::new(-4.0, 1.0, 1.0)),
            (Vec3::splat(-1.0), Vec3::splat(1.0)),
        ] {
            assert!(box_hit(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 0.1, min, max).is_none());
        }
    }

    #[test]
    fn a_hill_between_stops_the_sweep_and_the_ground_keeps_it_above() {
        let hill = |x: f32, _: f32| if (4.0..6.0).contains(&x) { 3.0 } else { 0.0 };
        let t = ground_hit(
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(10.0, 2.0, 0.0),
            0.1,
            &hill,
        );
        let t = t.unwrap();
        assert!((t * 10.0 - 4.0).abs() < 0.1, "{t}");
        let flat = Ground(|_: f32, _: f32| 0.0);
        let t = flat.sweep(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, -2.0, 0.0), 0.1);
        assert!((t - 0.475).abs() < 1e-3, "{t}");
    }

    #[test]
    fn footprints_block_up_to_their_tops() {
        let blocks = [Footprint {
            min: [4.0, -1.0],
            max: [5.0, 1.0],
        }];
        let low = Footprints {
            blocks: &blocks,
            tops: &[1.2],
            default_top: f32::INFINITY,
            floor: 0.0,
        };
        // Over a low wall, the view is clear; through a tall one it is not.
        let (from, to) = (Vec3::new(0.0, 1.9, 0.0), Vec3::new(10.0, 4.0, 0.0));
        assert_eq!(low.sweep(from, to, 0.1), 1.0);
        let tall = Footprints { tops: &[], ..low };
        assert!(tall.sweep(from, to, 0.1) < 0.4);
    }
}
