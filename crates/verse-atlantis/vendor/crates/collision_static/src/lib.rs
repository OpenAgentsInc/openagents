//! collision_static: simple capsule-vs-static colliders (cylinderY/OBB) + slide resolve.

use glam::{Mat3, Vec3};
use smallvec::SmallVec;

#[derive(Clone, Copy, Debug)]
pub enum ShapeKind {
    CylinderY,
    OBB,
}

#[derive(Clone, Copy, Debug)]
pub struct CylinderY {
    pub center: Vec3,
    pub radius: f32,
    pub half_height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct OBB {
    pub center: Vec3,
    pub half_extents: Vec3,
    pub rot3x3: Mat3,
}

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub enum ShapeRef {
    Cyl(CylinderY),
    Box(OBB),
}

#[derive(Clone, Copy, Debug)]
pub struct StaticCollider {
    pub aabb: Aabb,
    pub shape: ShapeRef,
}

#[derive(Clone, Debug, Default)]
pub struct StaticIndex {
    pub colliders: Vec<StaticCollider>,
}

#[derive(Clone, Copy, Debug)]
pub struct Capsule {
    pub p0: Vec3,
    pub p1: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Hit {
    pub normal: Vec3,
    pub depth: f32,
}

pub fn capsule_vs_static_overlap(
    cap: &Capsule,
    idx: &StaticIndex,
    query_aabb: Aabb,
    out: &mut SmallVec<[Hit; 8]>,
) {
    for c in &idx.colliders {
        if !aabb_overlap(&c.aabb, &query_aabb) {
            continue;
        }
        match c.shape {
            ShapeRef::Cyl(cyl) => {
                if let Some(h) = capsule_vs_cyl_y(cap, &cyl) {
                    out.push(h);
                }
            }
            ShapeRef::Box(obb) => {
                if let Some(h) = capsule_vs_obb(cap, &obb) {
                    out.push(h);
                }
            }
        }
    }
}

pub fn resolve_slide(
    _pos_old: Vec3,
    pos_try: Vec3,
    cap: &Capsule,
    idx: &StaticIndex,
    step_height: f32,
    max_iters: u32,
) -> Vec3 {
    let mut pos = pos_try;
    let mut cap_cur = *cap;
    let mut it = 0;
    while it < max_iters {
        it += 1;
        let aabb = expand_aabb(&capsule_aabb(&cap_cur), 0.01);
        let mut hits: SmallVec<[Hit; 8]> = SmallVec::new();
        capsule_vs_static_overlap(&cap_cur, idx, aabb, &mut hits);
        if hits.is_empty() {
            break;
        }
        // Accumulate normals; push out along the most penetrating
        let mut best = Hit::default();
        for h in hits {
            if h.depth > best.depth {
                best = h;
            }
        }
        if best.depth <= 1e-4 {
            break;
        }
        // Slide: remove normal component from displacement
        pos += best.normal * best.depth;
        cap_cur.p0 += best.normal * best.depth;
        cap_cur.p1 += best.normal * best.depth;
        // Optional simple step-up: if the normal is mostly horizontal, try lifting by step_height
        if best.normal.y.abs() < 0.4 && step_height > 0.0 {
            pos.y += step_height;
            cap_cur.p0.y += step_height;
            cap_cur.p1.y += step_height;
        }
    }
    // Prevent tunneling: if still penetrating, push out along up
    let aabb = expand_aabb(&capsule_aabb(&cap_cur), 0.01);
    let mut hits: SmallVec<[Hit; 8]> = SmallVec::new();
    capsule_vs_static_overlap(&cap_cur, idx, aabb, &mut hits);
    if !hits.is_empty() {
        pos.y += 0.02;
    }
    pos
}

fn aabb_overlap(a: &Aabb, b: &Aabb) -> bool {
    !(a.max.x < b.min.x
        || a.min.x > b.max.x
        || a.max.y < b.min.y
        || a.min.y > b.max.y
        || a.max.z < b.min.z
        || a.min.z > b.max.z)
}

fn expand_aabb(a: &Aabb, eps: f32) -> Aabb {
    Aabb {
        min: a.min - Vec3::splat(eps),
        max: a.max + Vec3::splat(eps),
    }
}

fn capsule_aabb(c: &Capsule) -> Aabb {
    let min = c.p0.min(c.p1) - Vec3::splat(c.radius);
    let max = c.p0.max(c.p1) + Vec3::splat(c.radius);
    Aabb { min, max }
}

fn capsule_vs_cyl_y(cap: &Capsule, cyl: &CylinderY) -> Option<Hit> {
    // Project capsule segment to XZ for lateral separation; clamp Y within cylinder top/bottom
    let cy = cyl.center.y;
    let top = cy + cyl.half_height;
    let bot = cy - cyl.half_height;
    // Choose the closest point on the capsule segment in Y to the cylinder center Y
    let mut y_closest = cyl.center.y;
    if y_closest < cap.p0.y {
        y_closest = cap.p0.y;
    }
    if y_closest > cap.p1.y {
        y_closest = cap.p1.y;
    }
    // Lateral vector from cylinder axis to capsule axis
    let cap_xz = Vec3::new(cap.p0.x, 0.0, cap.p0.z);
    let cyl_xz = Vec3::new(cyl.center.x, 0.0, cyl.center.z);
    let d = cap_xz - cyl_xz;
    let dist = (d.x * d.x + d.z * d.z).sqrt();
    let allowed = cyl.radius + cap.radius;
    let depth = allowed - dist;
    let y_penetrates = y_closest >= bot - cap.radius && y_closest <= top + cap.radius;
    if depth > 0.0 && y_penetrates {
        let normal = if dist > 1e-6 {
            Vec3::new(d.x / dist, 0.0, d.z / dist)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        return Some(Hit { normal, depth });
    }
    None
}

fn capsule_vs_obb(_cap: &Capsule, _obb: &OBB) -> Option<Hit> {
    // Phase 1: not used; return None.
    None
}

pub mod chunks;

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn capsule_cylinder_pushes_out() {
        let cap = Capsule {
            p0: Vec3::new(0.0, 0.4, 0.0),
            p1: Vec3::new(0.0, 1.8, 0.0),
            radius: 0.4,
        };
        let cyl = CylinderY {
            center: Vec3::new(0.6, 1.0, 0.0),
            radius: 0.5,
            half_height: 2.5,
        };
        let idx = StaticIndex {
            colliders: vec![StaticCollider {
                aabb: Aabb {
                    min: Vec3::new(0.0, -2.0, -1.0),
                    max: Vec3::new(1.5, 4.0, 1.0),
                },
                shape: ShapeRef::Cyl(cyl),
            }],
        };
        let pos_old = Vec3::ZERO;
        let pos_try = Vec3::ZERO;
        let out = resolve_slide(pos_old, pos_try, &cap, &idx, 0.2, 3);
        assert!(out.x < 0.0); // pushed left away from cylinder at +x
        assert_abs_diff_eq!(out.z, 0.0, epsilon = 1e-3);
    }
}
