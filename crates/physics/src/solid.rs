//! Placed solids for point and segment queries: the obstacles a rope
//! collides with and wraps around.
//!
//! A [`Solid`] is a box, capsule, or flat-ended cylinder already placed in
//! the world. It answers the signed distance from a point with the outward
//! normal there, and the closest pair between itself and a segment, which
//! is what a chain of rope particles needs to stay outside it.

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::collision::{Collider, Shape};
use crate::world::World;

/// A convex solid placed in the world, m.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "solid", rename_all = "snake_case")]
pub enum Solid {
    /// An oriented box.
    Cuboid {
        center: DVec3,
        rotation: DQuat,
        half: DVec3,
    },
    /// The segment from `a` to `b` swept by `radius`.
    Capsule { a: DVec3, b: DVec3, radius: f64 },
    /// A cylinder with flat ends at `a` and `b`.
    Cylinder { a: DVec3, b: DVec3, radius: f64 },
}

impl Solid {
    /// An axis-aligned box from its corners.
    #[must_use]
    pub fn aabb(min: DVec3, max: DVec3) -> Self {
        Self::Cuboid {
            center: (min + max) * 0.5,
            rotation: DQuat::IDENTITY,
            half: (max - min) * 0.5,
        }
    }

    /// A collider's shape where its body is now.
    #[must_use]
    pub fn of(collider: &Collider, world: &World) -> Self {
        let (pos, rotation) = collider.pose(world);
        match collider.shape {
            Shape::Sphere { radius } => Self::Capsule {
                a: pos,
                b: pos,
                radius,
            },
            Shape::Capsule {
                radius,
                half_length,
            } => {
                let axis = rotation * DVec3::Z * half_length;
                Self::Capsule {
                    a: pos - axis,
                    b: pos + axis,
                    radius,
                }
            }
            Shape::Cuboid { half } => Self::Cuboid {
                center: pos,
                rotation,
                half,
            },
        }
    }

    /// The corners of an axis-aligned box that encloses the solid.
    #[must_use]
    pub fn bounds(&self) -> (DVec3, DVec3) {
        match *self {
            Self::Cuboid {
                center,
                rotation,
                half,
            } => {
                let reach = (rotation * DVec3::X * half.x).abs()
                    + (rotation * DVec3::Y * half.y).abs()
                    + (rotation * DVec3::Z * half.z).abs();
                (center - reach, center + reach)
            }
            Self::Capsule { a, b, radius } => (a.min(b) - radius, a.max(b) + radius),
            Self::Cylinder { a, b, radius } => {
                // A disc of radius r about axis u reaches r sqrt(1 - u_i^2)
                // along each world axis.
                let u = (b - a).normalize_or(DVec3::Z);
                let reach = (DVec3::ONE - u * u).max(DVec3::ZERO).map(f64::sqrt) * radius;
                (a.min(b) - reach, a.max(b) + reach)
            }
        }
    }

    /// Signed distance from `x` to the surface, negative inside, and the
    /// outward unit normal of the nearest surface point.
    #[must_use]
    pub fn distance(&self, x: DVec3) -> (f64, DVec3) {
        match *self {
            Self::Cuboid {
                center,
                rotation,
                half,
            } => {
                let local = rotation.inverse() * (x - center);
                let q = local.abs() - half;
                if q.max_element() > 0.0 {
                    let off = local - local.clamp(-half, half);
                    let d = off.length();
                    (d, rotation * (off / d))
                } else {
                    // Inside: leave through the nearest face.
                    let (axis, depth) = if q.x >= q.y && q.x >= q.z {
                        (DVec3::X * local.x.signum(), q.x)
                    } else if q.y >= q.z {
                        (DVec3::Y * local.y.signum(), q.y)
                    } else {
                        (DVec3::Z * local.z.signum(), q.z)
                    };
                    (depth, rotation * axis)
                }
            }
            Self::Capsule { a, b, radius } => {
                let d = b - a;
                let t = if d.length_squared() > 0.0 {
                    ((x - a).dot(d) / d.length_squared()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let off = x - (a + d * t);
                let len = off.length();
                let normal = if len > 1e-12 {
                    off / len
                } else {
                    d.normalize_or(DVec3::Z).any_orthonormal_vector()
                };
                (len - radius, normal)
            }
            Self::Cylinder { a, b, radius } => {
                let axis = b - a;
                let length = axis.length();
                let u = if length > 0.0 {
                    axis / length
                } else {
                    DVec3::Z
                };
                let s = (x - a).dot(u);
                let radial = (x - a) - u * s;
                let rho = radial.length();
                let out = if rho > 1e-12 {
                    radial / rho
                } else {
                    u.any_orthonormal_vector()
                };
                let (dz, end) = if s < length * 0.5 {
                    (-s, -u)
                } else {
                    (s - length, u)
                };
                let dr = rho - radius;
                if dr <= 0.0 && dz <= 0.0 {
                    if dr > dz { (dr, out) } else { (dz, end) }
                } else if dr > 0.0 && dz > 0.0 {
                    let d = dr.hypot(dz);
                    (d, (out * dr + end * dz) / d)
                } else if dr > 0.0 {
                    (dr, out)
                } else {
                    (dz, end)
                }
            }
        }
    }

    /// The point of the solid nearest `x`: on its surface, or `x` itself
    /// when `x` is inside.
    #[must_use]
    pub fn closest_point(&self, x: DVec3) -> DVec3 {
        let (d, normal) = self.distance(x);
        if d > 0.0 { x - normal * d } else { x }
    }

    /// The point of segment `p`–`q` nearest the solid, as a fraction along
    /// it. Alternating projections between the two convex sets converge to
    /// their closest pair, or to a shared point when they intersect.
    #[must_use]
    pub fn nearest_on_segment(&self, p: DVec3, q: DVec3) -> f64 {
        let d = q - p;
        let len2 = d.length_squared();
        if len2 <= 0.0 {
            return 0.0;
        }
        let center = match *self {
            Self::Cuboid { center, .. } => center,
            Self::Capsule { a, b, .. } | Self::Cylinder { a, b, .. } => (a + b) * 0.5,
        };
        let mut t = ((center - p).dot(d) / len2).clamp(0.0, 1.0);
        for _ in 0..4 {
            let c = self.closest_point(p + d * t);
            t = ((c - p).dot(d) / len2).clamp(0.0, 1.0);
        }
        t
    }
}

/// A solid with its bounds computed once, for callers that pass the same
/// solids every step.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bounded {
    pub solid: Solid,
    pub min: DVec3,
    pub max: DVec3,
}

impl Bounded {
    #[must_use]
    pub fn new(solid: Solid) -> Self {
        let (min, max) = solid.bounds();
        Self { solid, min, max }
    }

    /// Whether the bounds meet the box from `lo` to `hi`.
    #[must_use]
    pub fn meets(&self, lo: DVec3, hi: DVec3) -> bool {
        self.min.cmple(hi).all() && self.max.cmpge(lo).all()
    }
}

impl From<Solid> for Bounded {
    fn from(solid: Solid) -> Self {
        Self::new(solid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_and_normals_point_out_of_each_solid() {
        let cube = Solid::aabb(DVec3::splat(-1.0), DVec3::splat(1.0));
        assert_eq!(cube.distance(DVec3::new(3.0, 0.0, 0.0)), (2.0, DVec3::X));
        assert_eq!(
            cube.distance(DVec3::new(0.0, -0.75, 0.0)),
            (-0.25, -DVec3::Y)
        );
        let (d, n) = cube.distance(DVec3::new(2.0, 2.0, 0.0));
        assert!((d - 2f64.sqrt()).abs() < 1e-12);
        assert!((n - DVec3::new(1.0, 1.0, 0.0).normalize()).length() < 1e-12);
        let turned = Solid::Cuboid {
            center: DVec3::ZERO,
            rotation: DQuat::from_rotation_z(std::f64::consts::FRAC_PI_4),
            half: DVec3::ONE,
        };
        let (d, n) = turned.distance(DVec3::new(3.0, 0.0, 0.0));
        assert!((d - (3.0 - 2f64.sqrt())).abs() < 1e-12 && (n - DVec3::X).length() < 1e-12);
        let capsule = Solid::Capsule {
            a: DVec3::ZERO,
            b: DVec3::Z * 4.0,
            radius: 0.5,
        };
        assert_eq!(capsule.distance(DVec3::new(0.0, 2.0, 2.0)), (1.5, DVec3::Y));
        assert_eq!(capsule.distance(DVec3::Z * 5.0), (0.5, DVec3::Z));
        let cylinder = Solid::Cylinder {
            a: DVec3::ZERO,
            b: DVec3::Z * 4.0,
            radius: 1.0,
        };
        // Flat ends, unlike the capsule, and the rim between side and end.
        assert_eq!(
            cylinder.distance(DVec3::new(0.9, 0.0, 4.5)),
            (0.5, DVec3::Z)
        );
        assert_eq!(
            cylinder.distance(DVec3::new(0.0, 0.5, 2.0)),
            (-0.5, DVec3::Y)
        );
        assert_eq!(
            cylinder.distance(DVec3::new(0.0, 0.5, 0.2)),
            (-0.2, -DVec3::Z)
        );
        let (d, n) = cylinder.distance(DVec3::new(2.0, 0.0, 5.0));
        assert!((d - 2f64.sqrt()).abs() < 1e-12);
        assert!((n - DVec3::new(1.0, 0.0, 1.0).normalize()).length() < 1e-12);
    }

    #[test]
    fn bounds_enclose_each_solid() {
        let cylinder = Solid::Cylinder {
            a: DVec3::new(-1.0, 0.0, 0.0),
            b: DVec3::new(1.0, 0.0, 0.0),
            radius: 0.5,
        };
        assert_eq!(
            cylinder.bounds(),
            (DVec3::new(-1.0, -0.5, -0.5), DVec3::new(1.0, 0.5, 0.5))
        );
        let turned = Solid::Cuboid {
            center: DVec3::ZERO,
            rotation: DQuat::from_rotation_z(std::f64::consts::FRAC_PI_4),
            half: DVec3::ONE,
        };
        let (lo, hi) = turned.bounds();
        assert!((hi.x - 2f64.sqrt()).abs() < 1e-12 && (hi.z - 1.0).abs() < 1e-12);
        assert_eq!(lo, -hi);
    }

    #[test]
    fn a_segment_finds_its_point_nearest_a_solid() {
        let cube = Solid::aabb(DVec3::splat(-1.0), DVec3::splat(1.0));
        // A segment passing above the cube's edge meets it nearest over x = 0.
        let t = cube.nearest_on_segment(DVec3::new(-5.0, 1.5, 0.0), DVec3::new(3.0, 1.5, 0.0));
        let x = -5.0 + 8.0 * t;
        assert!((-1.0..=1.0).contains(&x), "{x}");
        // One through it lands inside.
        let t = cube.nearest_on_segment(DVec3::new(-5.0, 0.2, 0.0), DVec3::new(5.0, 0.2, 0.0));
        assert!(cube.distance(DVec3::new(-5.0 + 10.0 * t, 0.2, 0.0)).0 < 0.0);
    }
}
