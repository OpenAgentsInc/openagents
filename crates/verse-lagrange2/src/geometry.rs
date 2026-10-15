//! Clearance checks: oriented boxes and capsules. The factory uses them to
//! refuse a move that would put an arm through a part, a part through the
//! ship, or anything through the deck.

use glam::{DMat3, DQuat, DVec3};

/// An oriented box: center, orientation, and half extents along its axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obb {
    pub center: DVec3,
    pub orientation: DQuat,
    pub half: DVec3,
}

impl Obb {
    fn axes(&self) -> [DVec3; 3] {
        let m = DMat3::from_quat(self.orientation);
        [m.x_axis, m.y_axis, m.z_axis]
    }

    /// The box shrunk by `margin` on every face (never below zero).
    #[must_use]
    pub fn shrunk(mut self, margin: f64) -> Self {
        self.half = (self.half - DVec3::splat(margin)).max(DVec3::ZERO);
        self
    }

    /// Distance from `p` to the box, zero inside.
    #[must_use]
    pub fn distance(&self, p: DVec3) -> f64 {
        let local = self.orientation.inverse() * (p - self.center);
        let outside = (local.abs() - self.half).max(DVec3::ZERO);
        outside.length()
    }

    /// Whether two boxes overlap (separating-axis test over 15 axes).
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        let a = self.axes();
        let b = other.axes();
        let t = other.center - self.center;
        let mut axes: Vec<DVec3> = Vec::with_capacity(15);
        axes.extend_from_slice(&a);
        axes.extend_from_slice(&b);
        for u in a {
            for v in b {
                let c = u.cross(v);
                if c.length_squared() > 1e-12 {
                    axes.push(c.normalize());
                }
            }
        }
        let radius = |axes: &[DVec3; 3], half: DVec3, n: DVec3| {
            half.x * axes[0].dot(n).abs()
                + half.y * axes[1].dot(n).abs()
                + half.z * axes[2].dot(n).abs()
        };
        axes.into_iter()
            .all(|n| t.dot(n).abs() <= radius(&a, self.half, n) + radius(&b, other.half, n))
    }
}

/// Whether a capsule from `a` to `b` with `radius` touches the box. The
/// segment is sampled at most every quarter radius, which is exact enough
/// for a clearance check with a margin.
#[must_use]
pub fn capsule_hits(a: DVec3, b: DVec3, radius: f64, obb: &Obb) -> bool {
    let length = a.distance(b);
    let samples = ((length / (radius * 0.25)).ceil() as usize).max(1);
    (0..=samples).any(|i| obb.distance(a.lerp(b, i as f64 / samples as f64)) < radius)
}

/// The shortest distance between two segments.
#[must_use]
pub fn segment_distance(p1: DVec3, q1: DVec3, p2: DVec3, q2: DVec3) -> f64 {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.length_squared();
    let e = d2.length_squared();
    let f = d2.dot(r);
    let (s, t) = if a <= 1e-12 && e <= 1e-12 {
        (0.0, 0.0)
    } else if a <= 1e-12 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= 1e-12 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s = if denom > 1e-12 {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    (p1 + d1 * s).distance(p2 + d2 * t)
}
