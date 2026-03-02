use serde::{Deserialize, Serialize};

use crate::kernel_math::{Point2, Point3};

/// Sign of a geometric predicate result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sign {
    Negative,
    Zero,
    Positive,
}

impl Sign {
    #[inline]
    pub fn from_f64(value: f64) -> Self {
        if value > 0.0 {
            Self::Positive
        } else if value < 0.0 {
            Self::Negative
        } else {
            Self::Zero
        }
    }

    #[inline]
    pub fn is_positive(self) -> bool {
        matches!(self, Self::Positive)
    }

    #[inline]
    pub fn is_negative(self) -> bool {
        matches!(self, Self::Negative)
    }

    #[inline]
    pub fn is_zero(self) -> bool {
        matches!(self, Self::Zero)
    }
}

/// Robust 2D orientation predicate.
#[inline]
pub fn orient2d(a: &Point2, b: &Point2, c: &Point2) -> Sign {
    let result = robust::orient2d(
        robust::Coord { x: a.x, y: a.y },
        robust::Coord { x: b.x, y: b.y },
        robust::Coord { x: c.x, y: c.y },
    );
    Sign::from_f64(result)
}

/// Robust 2D in-circle predicate.
#[inline]
pub fn incircle(a: &Point2, b: &Point2, c: &Point2, d: &Point2) -> Sign {
    let result = robust::incircle(
        robust::Coord { x: a.x, y: a.y },
        robust::Coord { x: b.x, y: b.y },
        robust::Coord { x: c.x, y: c.y },
        robust::Coord { x: d.x, y: d.y },
    );
    Sign::from_f64(result)
}

/// Robust 3D orientation predicate.
#[inline]
pub fn orient3d(a: &Point3, b: &Point3, c: &Point3, d: &Point3) -> Sign {
    let result = robust::orient3d(
        robust::Coord3D {
            x: a.x,
            y: a.y,
            z: a.z,
        },
        robust::Coord3D {
            x: b.x,
            y: b.y,
            z: b.z,
        },
        robust::Coord3D {
            x: c.x,
            y: c.y,
            z: c.z,
        },
        robust::Coord3D {
            x: d.x,
            y: d.y,
            z: d.z,
        },
    );
    Sign::from_f64(result)
}

/// Robust 3D in-sphere predicate.
#[inline]
pub fn insphere(a: &Point3, b: &Point3, c: &Point3, d: &Point3, e: &Point3) -> Sign {
    let result = robust::insphere(
        robust::Coord3D {
            x: a.x,
            y: a.y,
            z: a.z,
        },
        robust::Coord3D {
            x: b.x,
            y: b.y,
            z: b.z,
        },
        robust::Coord3D {
            x: c.x,
            y: c.y,
            z: c.z,
        },
        robust::Coord3D {
            x: d.x,
            y: d.y,
            z: d.z,
        },
        robust::Coord3D {
            x: e.x,
            y: e.y,
            z: e.z,
        },
    );
    Sign::from_f64(result)
}

/// Test whether `p` is on segment `ab` in 2D (inclusive endpoints).
pub fn point_on_segment_2d(p: &Point2, a: &Point2, b: &Point2) -> bool {
    if !orient2d(a, b, p).is_zero() {
        return false;
    }
    let min_x = a.x.min(b.x);
    let max_x = a.x.max(b.x);
    let min_y = a.y.min(b.y);
    let max_y = a.y.max(b.y);
    p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y
}

/// Test whether `p` lies on the plane through `a`, `b`, `c`.
#[inline]
pub fn point_on_plane(p: &Point3, a: &Point3, b: &Point3, c: &Point3) -> bool {
    orient3d(a, b, c, p).is_zero()
}

/// Test whether four 3D points are coplanar.
#[inline]
pub fn are_coplanar(a: &Point3, b: &Point3, c: &Point3, d: &Point3) -> bool {
    orient3d(a, b, c, d).is_zero()
}

/// Test whether three 2D points are collinear.
#[inline]
pub fn are_collinear_2d(a: &Point2, b: &Point2, c: &Point2) -> bool {
    orient2d(a, b, c).is_zero()
}

/// Return the side of line `ab` point `p` is on, or None if on the line.
#[inline]
pub fn point_side_of_line(p: &Point2, a: &Point2, b: &Point2) -> Option<Sign> {
    let sign = orient2d(a, b, p);
    if sign.is_zero() { None } else { Some(sign) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orient2d_detects_ccw_cw_and_near_collinear() {
        let a = Point2::new(0.0, 0.0);
        let b = Point2::new(1.0, 0.0);
        assert_eq!(orient2d(&a, &b, &Point2::new(0.5, 1.0)), Sign::Positive);
        assert_eq!(orient2d(&a, &b, &Point2::new(0.5, -1.0)), Sign::Negative);
        assert_eq!(orient2d(&a, &b, &Point2::new(0.5, 0.0)), Sign::Zero);
        assert_eq!(orient2d(&a, &b, &Point2::new(0.5, 1e-15)), Sign::Positive);
    }

    #[test]
    fn orient3d_detects_plane_side_and_near_coplanar() {
        let a = Point3::new(0.0, 0.0, 0.0);
        let b = Point3::new(1.0, 0.0, 0.0);
        let c = Point3::new(0.0, 1.0, 0.0);
        assert_eq!(
            orient3d(&a, &b, &c, &Point3::new(0.0, 0.0, 1.0)),
            Sign::Negative
        );
        assert_eq!(
            orient3d(&a, &b, &c, &Point3::new(0.0, 0.0, -1.0)),
            Sign::Positive
        );
        assert_eq!(
            orient3d(&a, &b, &c, &Point3::new(0.5, 0.5, 0.0)),
            Sign::Zero
        );
        assert_eq!(
            orient3d(&a, &b, &c, &Point3::new(0.5, 0.5, 1e-15)),
            Sign::Negative
        );
    }

    #[test]
    fn incircle_and_insphere_classify_inside_outside() {
        let a = Point2::new(0.0, 0.0);
        let b = Point2::new(1.0, 0.0);
        let c = Point2::new(0.5, 0.866_025_403_784);
        assert_eq!(incircle(&a, &b, &c, &Point2::new(0.5, 0.3)), Sign::Positive);
        assert_eq!(incircle(&a, &b, &c, &Point2::new(2.0, 2.0)), Sign::Negative);

        let p0 = Point3::new(1.0, 1.0, 1.0);
        let p1 = Point3::new(1.0, -1.0, -1.0);
        let p2 = Point3::new(-1.0, 1.0, -1.0);
        let p3 = Point3::new(-1.0, -1.0, 1.0);
        assert_eq!(
            insphere(&p0, &p1, &p2, &p3, &Point3::new(0.0, 0.0, 0.0)),
            Sign::Positive
        );
        assert_eq!(
            insphere(&p0, &p1, &p2, &p3, &Point3::new(10.0, 10.0, 10.0)),
            Sign::Negative
        );
    }

    #[test]
    fn derived_predicates_follow_expected_contracts() {
        let a = Point2::new(0.0, 0.0);
        let b = Point2::new(2.0, 0.0);
        assert!(point_on_segment_2d(&Point2::new(1.0, 0.0), &a, &b));
        assert!(!point_on_segment_2d(&Point2::new(3.0, 0.0), &a, &b));
        assert!(are_collinear_2d(&a, &b, &Point2::new(1.0, 0.0)));
        assert_eq!(
            point_side_of_line(&Point2::new(0.2, 1.0), &a, &b),
            Some(Sign::Positive)
        );

        let p0 = Point3::new(0.0, 0.0, 0.0);
        let p1 = Point3::new(1.0, 0.0, 0.0);
        let p2 = Point3::new(0.0, 1.0, 0.0);
        assert!(point_on_plane(&Point3::new(0.2, 0.2, 0.0), &p0, &p1, &p2));
        assert!(!point_on_plane(&Point3::new(0.2, 0.2, 0.1), &p0, &p1, &p2));
        assert!(are_coplanar(&p0, &p1, &p2, &Point3::new(1.0, 1.0, 0.0)));
    }
}
