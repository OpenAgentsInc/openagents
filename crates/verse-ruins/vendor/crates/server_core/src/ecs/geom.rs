//! Small geometry helpers reusable by systems.

use glam::{Vec2, Vec3};

#[inline]
pub fn segment_hits_circle_xz(p0: Vec3, p1: Vec3, center: Vec3, radius: f32) -> bool {
    let a = Vec2::new(p0.x, p0.z);
    let b = Vec2::new(p1.x, p1.z);
    let c = Vec2::new(center.x, center.z);
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 <= 1e-12 {
        return (a - c).length_squared() <= radius * radius;
    }
    let t = ((c - a).dot(ab) / len2).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (closest - c).length_squared() <= radius * radius
}

/// Compute the parametric `t` at which a segment `[p0, p1]` first enters an axis-aligned AABB.
/// Returns `None` when no intersection occurs. `t` is in `[0, 1]`.
#[inline]
pub fn segment_aabb_enter_t(p0: Vec3, p1: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let d = p1 - p0;
    let mut tmin = 0.0f32;
    let mut tmax = 1.0f32;
    for i in 0..3 {
        let s = p0[i];
        let dir = d[i];
        let minb = min[i];
        let maxb = max[i];
        if dir.abs() < 1e-6 {
            if s < minb || s > maxb {
                return None;
            }
        } else {
            let inv = 1.0 / dir;
            let mut t0 = (minb - s) * inv;
            let mut t1 = (maxb - s) * inv;
            if t0 > t1 {
                core::mem::swap(&mut t0, &mut t1);
            }
            tmin = tmin.max(t0);
            tmax = tmax.min(t1);
            if tmin > tmax {
                return None;
            }
        }
    }
    Some(tmin)
}
