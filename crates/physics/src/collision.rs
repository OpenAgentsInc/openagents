//! Collision shapes, filters, and narrow-phase contact generation.
//!
//! Shapes are spheres, capsules, and boxes attached to bodies. Every pair
//! reduces to capsule–capsule, capsule–box, or box–box, since a sphere is a
//! capsule of zero length. Box–box uses the separating-axis test and clips
//! the incident face against the reference face, keeping at most four points
//! (the contact-patch scheme in Genesis `examples/collision/contact_manifold.py`).
//! Contacts are generated up to a margin before touching, so the solver can
//! stop a fast body at the surface instead of letting it tunnel through.

use glam::{DMat3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::world::{BodyId, World};

/// A collision shape in its collider frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum Shape {
    Sphere {
        radius: f64,
    },
    /// A segment along the collider's z axis, swept by `radius`.
    Capsule {
        radius: f64,
        half_length: f64,
    },
    Cuboid {
        half: DVec3,
    },
}

impl Shape {
    /// Radius of a sphere about the collider origin that encloses the shape.
    #[must_use]
    pub fn bound(&self) -> f64 {
        match *self {
            Self::Sphere { radius } => radius,
            Self::Capsule {
                radius,
                half_length,
            } => radius + half_length,
            Self::Cuboid { half } => half.length(),
        }
    }
}

/// Contact filtering by bitmasks, as MuJoCo's and Genesis's `contype` and
/// `conaffinity`: two colliders can touch when either one's `group` shares
/// a bit with the other's `mask`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    pub group: u32,
    pub mask: u32,
}

impl Filter {
    /// Collides with everything.
    pub const ALL: Self = Self {
        group: u32::MAX,
        mask: u32::MAX,
    };
    /// Collides with nothing.
    pub const NONE: Self = Self { group: 0, mask: 0 };

    #[must_use]
    pub const fn allows(self, other: Self) -> bool {
        self.group & other.mask != 0 || other.group & self.mask != 0
    }
}

/// Surface properties. Pairs combine friction and torsional friction by
/// geometric mean and restitution by maximum.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    /// Coulomb coefficient for sliding.
    pub friction: f64,
    /// Coulomb coefficient for spinning about the contact normal, m: the
    /// largest resisting torque is `torsional` times the normal force.
    pub torsional: f64,
    /// Fraction of the closing speed returned on impact.
    pub restitution: f64,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            friction: 0.5,
            torsional: 0.0,
            restitution: 0.2,
        }
    }
}

/// Index of a collider in its world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ColliderId(pub u32);

/// A shape attached to a body.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collider {
    pub body: BodyId,
    pub shape: Shape,
    /// Position in the body frame, m.
    pub offset: DVec3,
    /// Rotation in the body frame.
    pub rotation: DQuat,
    pub filter: Filter,
    pub material: Material,
}

impl Collider {
    /// A shape at the body's center of mass, colliding with everything.
    #[must_use]
    pub fn new(body: BodyId, shape: Shape) -> Self {
        Self {
            body,
            shape,
            offset: DVec3::ZERO,
            rotation: DQuat::IDENTITY,
            filter: Filter::ALL,
            material: Material::default(),
        }
    }

    #[must_use]
    pub fn at(mut self, offset: DVec3, rotation: DQuat) -> Self {
        self.offset = offset;
        self.rotation = rotation;
        self
    }

    #[must_use]
    pub fn with_filter(mut self, filter: Filter) -> Self {
        self.filter = filter;
        self
    }

    #[must_use]
    pub fn with_material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }

    /// The point of the shape nearest `x`: on its surface, or `x` itself
    /// when `x` is inside.
    #[must_use]
    pub fn closest_point(&self, world: &World, x: DVec3) -> DVec3 {
        let (pos, rotation) = self.pose(world);
        match place(self.shape, pos, rotation) {
            Placed::Capsule { p, q, radius } => {
                let d = q - p;
                let t = if d.length_squared() > 0.0 {
                    ((x - p).dot(d) / d.length_squared()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let core = p + d * t;
                let offset = x - core;
                if offset.length() <= radius {
                    x
                } else {
                    core + offset.normalize() * radius
                }
            }
            Placed::Cuboid { center, axes, half } => {
                let local = axes.transpose() * (x - center);
                center + axes * local.clamp(-half, half)
            }
        }
    }

    /// World pose of the collider frame.
    #[must_use]
    pub fn pose(&self, world: &World) -> (DVec3, DQuat) {
        let body = &world[self.body];
        (
            body.pos + body.orientation * self.offset,
            body.orientation * self.rotation,
        )
    }
}

/// One contact point. `normal` points from the first collider to the second;
/// `separation` is the gap along it, negative when the shapes overlap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactPoint {
    pub point: DVec3,
    pub normal: DVec3,
    pub separation: f64,
}

/// Every contact point between two colliders.
#[derive(Clone, Debug, PartialEq)]
pub struct Manifold {
    pub a: ColliderId,
    pub b: ColliderId,
    pub points: Vec<ContactPoint>,
}

/// A shape placed in the world, reduced to the primitives the narrow phase
/// handles.
#[derive(Clone, Copy, Debug)]
enum Placed {
    /// Segment from `p` to `q`, swept by `radius`.
    Capsule { p: DVec3, q: DVec3, radius: f64 },
    Cuboid {
        center: DVec3,
        axes: DMat3,
        half: DVec3,
    },
}

fn place(shape: Shape, pos: DVec3, rotation: DQuat) -> Placed {
    match shape {
        Shape::Sphere { radius } => Placed::Capsule {
            p: pos,
            q: pos,
            radius,
        },
        Shape::Capsule {
            radius,
            half_length,
        } => {
            let axis = rotation * DVec3::Z * half_length;
            Placed::Capsule {
                p: pos - axis,
                q: pos + axis,
                radius,
            }
        }
        Shape::Cuboid { half } => Placed::Cuboid {
            center: pos,
            axes: DMat3::from_quat(rotation),
            half,
        },
    }
}

/// Contact points between two placed shapes within `margin` of touching.
fn contact(a: Placed, b: Placed, margin: f64) -> Vec<ContactPoint> {
    match (a, b) {
        (
            Placed::Capsule { p, q, radius },
            Placed::Capsule {
                p: p2,
                q: q2,
                radius: r2,
            },
        ) => capsule_capsule(p, q, radius, p2, q2, r2, margin),
        (Placed::Capsule { p, q, radius }, Placed::Cuboid { center, axes, half }) => {
            capsule_box(p, q, radius, center, axes, half, margin)
        }
        (Placed::Cuboid { .. }, Placed::Capsule { .. }) => flip(contact(b, a, margin)),
        (
            Placed::Cuboid { center, axes, half },
            Placed::Cuboid {
                center: c2,
                axes: a2,
                half: h2,
            },
        ) => box_box(center, axes, half, c2, a2, h2, margin),
    }
}

fn flip(points: Vec<ContactPoint>) -> Vec<ContactPoint> {
    points
        .into_iter()
        .map(|c| ContactPoint {
            normal: -c.normal,
            ..c
        })
        .collect()
}

/// Closest points between segments `p1 q1` and `p2 q2` (Ericson, Real-Time
/// Collision Detection, 5.1.9), as segment parameters in [0, 1].
fn closest_segments(p1: DVec3, q1: DVec3, p2: DVec3, q2: DVec3) -> (f64, f64) {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.length_squared();
    let e = d2.length_squared();
    let f = d2.dot(r);
    const EPS: f64 = 1e-12;
    if a <= EPS && e <= EPS {
        return (0.0, 0.0);
    }
    if a <= EPS {
        return (0.0, (f / e).clamp(0.0, 1.0));
    }
    let c = d1.dot(r);
    if e <= EPS {
        return ((-c / a).clamp(0.0, 1.0), 0.0);
    }
    let b = d1.dot(d2);
    let denom = a * e - b * b;
    let mut s = if denom > EPS {
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

/// A contact between points `x` on the first shape's core and `y` on the
/// second's, with the shapes' radii, or none beyond the margin.
fn sphere_pair(
    x: DVec3,
    y: DVec3,
    ra: f64,
    rb: f64,
    fallback: DVec3,
    margin: f64,
) -> Option<ContactPoint> {
    let offset = y - x;
    let distance = offset.length();
    let separation = distance - ra - rb;
    if separation > margin {
        return None;
    }
    let normal = if distance > 1e-12 {
        offset / distance
    } else {
        fallback
    };
    Some(ContactPoint {
        point: x + normal * (ra + separation * 0.5),
        normal,
        separation,
    })
}

fn capsule_capsule(
    p1: DVec3,
    q1: DVec3,
    r1: f64,
    p2: DVec3,
    q2: DVec3,
    r2: f64,
    margin: f64,
) -> Vec<ContactPoint> {
    let (s, t) = closest_segments(p1, q1, p2, q2);
    let x = p1.lerp(q1, s);
    let y = p2.lerp(q2, t);
    let mut points: Vec<ContactPoint> = sphere_pair(x, y, r1, r2, DVec3::Y, margin)
        .into_iter()
        .collect();
    // Parallel capsules lying along each other touch along a line: add the
    // overlap's ends so they rest on two points instead of rocking on one.
    let (d1, d2) = (q1 - p1, q2 - p2);
    if d1.length_squared() > 1e-12
        && d2.length_squared() > 1e-12
        && d1.normalize().cross(d2.normalize()).length() < 1e-3
    {
        for end in [p1, q1] {
            let t = ((end - p2).dot(d2) / d2.length_squared()).clamp(0.0, 1.0);
            let y = p2 + d2 * t;
            if let Some(c) = sphere_pair(end, y, r1, r2, DVec3::Y, margin) {
                push_distinct(&mut points, c, (r1 + r2) * 0.25);
            }
        }
    }
    points
}

fn push_distinct(points: &mut Vec<ContactPoint>, c: ContactPoint, spacing: f64) {
    if points.iter().all(|p| p.point.distance(c.point) > spacing) {
        points.push(c);
    }
}

/// Signed distance from `x` to a box and the outward direction there.
fn box_distance(x: DVec3, center: DVec3, axes: DMat3, half: DVec3) -> (f64, DVec3) {
    let local = axes.transpose() * (x - center);
    let outside = (local.abs() - half).max(DVec3::ZERO);
    if outside.length_squared() > 0.0 {
        let clamped = local.clamp(-half, half);
        let direction = axes * (local - clamped);
        let distance = direction.length();
        return (distance, direction / distance);
    }
    // Inside: leave through the nearest face.
    let depth = half - local.abs();
    let (i, d) = if depth.x <= depth.y && depth.x <= depth.z {
        (0, depth.x)
    } else if depth.y <= depth.z {
        (1, depth.y)
    } else {
        (2, depth.z)
    };
    let sign = if local[i] < 0.0 { -1.0 } else { 1.0 };
    (-d, axes.col(i) * sign)
}

fn capsule_box(
    p: DVec3,
    q: DVec3,
    radius: f64,
    center: DVec3,
    axes: DMat3,
    half: DVec3,
    margin: f64,
) -> Vec<ContactPoint> {
    let at = |t: f64| p.lerp(q, t);
    let distance = |t: f64| box_distance(at(t), center, axes, half).0;
    // The signed distance to a convex box is convex along a line; a golden
    // section search finds the deepest point on the segment.
    let mut t = 0.0;
    if p != q {
        let ratio = (5.0_f64.sqrt() - 1.0) / 2.0;
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..48 {
            let m1 = hi - ratio * (hi - lo);
            let m2 = lo + ratio * (hi - lo);
            if distance(m1) < distance(m2) {
                hi = m2;
            } else {
                lo = m1;
            }
        }
        t = (lo + hi) / 2.0;
    }
    let mut points = Vec::new();
    let length = p.distance(q);
    let spacing = (length * 0.1).max(radius * 0.25);
    for t in [t, 0.0, 1.0] {
        let x = at(t);
        let (d, outward) = box_distance(x, center, axes, half);
        let separation = d - radius;
        if separation > margin {
            continue;
        }
        let normal = -outward;
        push_distinct(
            &mut points,
            ContactPoint {
                point: x + normal * (radius + separation * 0.5),
                normal,
                separation,
            },
            spacing,
        );
        if p == q {
            break;
        }
    }
    points
}

/// Separating-axis box–box contact with face clipping.
fn box_box(
    ca: DVec3,
    aa: DMat3,
    ha: DVec3,
    cb: DVec3,
    ab: DMat3,
    hb: DVec3,
    margin: f64,
) -> Vec<ContactPoint> {
    let d = cb - ca;
    let radius = |axes: DMat3, half: DVec3, l: DVec3| {
        half.x * axes.x_axis.dot(l).abs()
            + half.y * axes.y_axis.dot(l).abs()
            + half.z * axes.z_axis.dot(l).abs()
    };
    let separation = |l: DVec3| d.dot(l).abs() - radius(aa, ha, l) - radius(ab, hb, l);
    // Best face axis: 0..3 on A, 3..6 on B.
    let mut face = (f64::NEG_INFINITY, 0usize, DVec3::ZERO);
    for i in 0..6 {
        let l = if i < 3 { aa.col(i) } else { ab.col(i - 3) };
        let s = separation(l);
        if s > margin {
            return Vec::new();
        }
        if s > face.0 {
            face = (s, i, l);
        }
    }
    let mut edge = (f64::NEG_INFINITY, 0usize, 0usize, DVec3::ZERO);
    for i in 0..3 {
        for j in 0..3 {
            let l = aa.col(i).cross(ab.col(j));
            if l.length_squared() < 1e-10 {
                continue;
            }
            let l = l.normalize();
            let s = separation(l);
            if s > margin {
                return Vec::new();
            }
            if s > edge.0 {
                edge = (s, i, j, l);
            }
        }
    }
    // Prefer a face unless an edge axis is clearly better: face contacts give
    // stable multi-point manifolds.
    if edge.0 > face.0 + 1e-4 * (1.0 + face.0.abs()) {
        let (s, i, j, mut n) = edge;
        if d.dot(n) < 0.0 {
            n = -n;
        }
        let edge_point = |c: DVec3, axes: DMat3, half: DVec3, skip: usize, toward: DVec3| {
            (0..3).filter(|k| *k != skip).fold(c, |p, k| {
                let axis = axes.col(k);
                p + axis * (half[k] * axis.dot(toward).signum())
            })
        };
        let pa = edge_point(ca, aa, ha, i, n);
        let pb = edge_point(cb, ab, hb, j, -n);
        let (ua, ub) = (aa.col(i) * ha[i], ab.col(j) * hb[j]);
        let (sa, sb) = closest_segments(pa - ua, pa + ua, pb - ub, pb + ub);
        let x = (pa - ua).lerp(pa + ua, sa);
        let y = (pb - ub).lerp(pb + ub, sb);
        return vec![ContactPoint {
            point: (x + y) * 0.5,
            normal: n,
            separation: s,
        }];
    }
    let (_, index, axis) = face;
    let a_is_reference = index < 3;
    let (rc, ra, rh, ic, ia, ih) = if a_is_reference {
        (ca, aa, ha, cb, ab, hb)
    } else {
        (cb, ab, hb, ca, aa, ha)
    };
    let j = index % 3;
    // Reference normal points from the reference box toward the incident one.
    let toward = ic - rc;
    let ref_normal = if axis.dot(toward) < 0.0 { -axis } else { axis };
    // Incident face: the incident box's face most opposed to the normal.
    let (mut best, mut fi) = (f64::NEG_INFINITY, 0usize);
    for k in 0..3 {
        let dot = ia.col(k).dot(ref_normal).abs();
        if dot > best {
            best = dot;
            fi = k;
        }
    }
    let inc_normal = ia.col(fi) * -ia.col(fi).dot(ref_normal).signum();
    let inc_center = ic + inc_normal * ih[fi];
    let (u, v) = ((fi + 1) % 3, (fi + 2) % 3);
    let (eu, ev) = (ia.col(u) * ih[u], ia.col(v) * ih[v]);
    let mut polygon = vec![
        inc_center + eu + ev,
        inc_center - eu + ev,
        inc_center - eu - ev,
        inc_center + eu - ev,
    ];
    // Clip against the reference face's four side planes.
    for k in [(j + 1) % 3, (j + 2) % 3] {
        let side = ra.col(k);
        for sign in [1.0, -1.0] {
            let n = side * sign;
            let limit = rc.dot(n) + rh[k];
            polygon = clip(&polygon, n, limit);
        }
    }
    let face_point = rc + ref_normal * rh[j];
    let normal = if a_is_reference {
        ref_normal
    } else {
        -ref_normal
    };
    let mut points: Vec<ContactPoint> = polygon
        .into_iter()
        .filter_map(|p| {
            let s = (p - face_point).dot(ref_normal);
            (s <= margin).then(|| ContactPoint {
                point: p - ref_normal * (s * 0.5),
                normal,
                separation: s,
            })
        })
        .collect();
    reduce(&mut points);
    points
}

/// Sutherland–Hodgman: keep the part of `polygon` where `x . n <= limit`.
fn clip(polygon: &[DVec3], n: DVec3, limit: f64) -> Vec<DVec3> {
    let mut out = Vec::with_capacity(polygon.len() + 4);
    for (i, &a) in polygon.iter().enumerate() {
        let b = polygon[(i + 1) % polygon.len()];
        let (da, db) = (a.dot(n) - limit, b.dot(n) - limit);
        if da <= 0.0 {
            out.push(a);
        }
        if (da <= 0.0) != (db <= 0.0) {
            out.push(a + (b - a) * (da / (da - db)));
        }
    }
    out
}

/// Keep at most four points that span the patch: the deepest, the one
/// farthest from it, and the farthest on each side of the line between them.
fn reduce(points: &mut Vec<ContactPoint>) {
    // Clipping repeats a vertex that lies on a plane; drop exact repeats.
    points.dedup_by(|a, b| a.point.distance(b.point) < 1e-9);
    if points.len() > 1 && points[0].point.distance(points[points.len() - 1].point) < 1e-9 {
        points.pop();
    }
    if points.len() <= 4 {
        return;
    }
    let pick = |score: &dyn Fn(&ContactPoint) -> f64, points: &[ContactPoint]| {
        points
            .iter()
            .enumerate()
            .max_by(|a, b| score(a.1).total_cmp(&score(b.1)))
            .map_or(0, |(i, _)| i)
    };
    let first = points[pick(&|c| -c.separation, points)];
    let second = points[pick(&|c| c.point.distance_squared(first.point), points)];
    let line = second.point - first.point;
    let side = |c: &ContactPoint| line.cross(c.point - first.point).dot(first.normal);
    let third = points[pick(&|c| side(c), points)];
    let fourth = points[pick(&|c| -side(c), points)];
    *points = vec![first, second, third, fourth];
}

/// Work performed by rigid contact detection; timings live in `StepStats`.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct DetectionStats {
    pub colliders: usize,
    pub scene_nodes: usize,
    pub candidate_pairs: usize,
    pub filtered_pairs: usize,
    pub bound_tests: usize,
    pub narrow_phase: usize,
    pub index_updates: usize,
    pub wake_scene_nodes: usize,
    pub wake_candidate_pairs: usize,
    pub wake_index_updates: usize,
}

impl World {
    /// Contact manifolds between colliders whose filters allow it and whose
    /// bodies can respond (at least one dynamic), within `margin` of
    /// touching. Colliders on the same body never collide. An arbitrary callback
    /// requires exhaustive pair enumeration; use `detect_bounded` for a constant
    /// margin. `World::step` uses indexed per-collider motion bounds.
    #[must_use]
    pub fn detect(&self, margin: &dyn Fn(&Collider, &Collider) -> f64) -> Vec<Manifold> {
        self.detect_profiled(margin).0
    }
    /// Exhaustive reference for arbitrary pair-specific margins.
    pub fn detect_profiled(
        &self,
        margin: &dyn Fn(&Collider, &Collider) -> f64,
    ) -> (Vec<Manifold>, DetectionStats) {
        let mut stats = DetectionStats {
            colliders: self.colliders().len(),
            ..Default::default()
        };
        let placed: Vec<(Placed, DVec3, f64)> = self
            .colliders()
            .iter()
            .map(|c| {
                let (pos, rotation) = c.pose(self);
                (place(c.shape, pos, rotation), pos, c.shape.bound())
            })
            .collect();
        let mut manifolds = Vec::new();
        let colliders = self.colliders();
        for i in 0..colliders.len() {
            for j in (i + 1)..colliders.len() {
                stats.candidate_pairs += 1;
                let (a, b) = (&colliders[i], &colliders[j]);
                if a.body == b.body || !a.filter.allows(b.filter) {
                    continue;
                }
                // Two bodies that cannot respond (fixed, kinematic, or asleep)
                // need no contacts.
                if !self[a.body].responds() && !self[b.body].responds() {
                    continue;
                }
                stats.filtered_pairs += 1;
                let m = margin(a, b);
                stats.bound_tests += 1;
                let (pa, ca, ra) = placed[i];
                let (pb, cb, rb) = placed[j];
                if ca.distance(cb) > ra + rb + m {
                    continue;
                }
                stats.narrow_phase += 1;
                let points = contact(pa, pb, m);
                if !points.is_empty() {
                    manifolds.push(Manifold {
                        a: ColliderId(i as u32),
                        b: ColliderId(j as u32),
                        points,
                    });
                }
            }
        }
        (manifolds, stats)
    }
    /// Indexed detection with a constant nonnegative margin.
    pub fn detect_bounded(
        &mut self,
        margin: f64,
    ) -> Result<(Vec<Manifold>, DetectionStats), String> {
        self.detect_motion_profiled(margin, &vec![0.; self.colliders().len()])
    }
    /// Per-collider motion reaches bound the pair margin: `base + reach[a] + reach[b]`.
    pub fn detect_motion_profiled(
        &mut self,
        base: f64,
        reaches: &[f64],
    ) -> Result<(Vec<Manifold>, DetectionStats), String> {
        self.detect_motion_with_margin(base, reaches, &|i, j| base + reaches[i] + reaches[j])
    }
    pub(crate) fn detect_motion_with_margin(
        &mut self,
        base: f64,
        reaches: &[f64],
        margin: &dyn Fn(usize, usize) -> f64,
    ) -> Result<(Vec<Manifold>, DetectionStats), String> {
        #[cfg(test)]
        if self.exhaustive_detection {
            return Err("Exhaustive test reference".into());
        }
        if !base.is_finite()
            || base < 0.
            || reaches.len() != self.colliders().len()
            || reaches.iter().any(|v| !v.is_finite() || *v < 0.)
        {
            return Err("Invalid rigid broadphase margin or motion reaches".into());
        }
        let mut stats = DetectionStats {
            colliders: self.colliders().len(),
            ..Default::default()
        };
        let placed: Vec<_> = self
            .colliders()
            .iter()
            .map(|c| {
                let (pos, rotation) = c.pose(self);
                (place(c.shape, pos, rotation), pos, c.shape.bound())
            })
            .collect();
        if placed
            .iter()
            .any(|(_, pos, radius)| !pos.is_finite() || !radius.is_finite() || *radius < 0.)
        {
            return Err("Invalid rigid broadphase geometry".into());
        }
        let bounds: Vec<_> = placed
            .iter()
            .enumerate()
            .map(|(i, (_, pos, radius))| {
                let extent = DVec3::splat(radius + base * 0.5 + reaches[i]);
                crate::broadphase::Bounds {
                    min: *pos - extent,
                    max: *pos + extent,
                }
            })
            .collect();
        for (i, &bounds) in bounds.iter().enumerate() {
            let collider = &self.colliders()[i];
            if collider.filter != Filter::NONE {
                stats.index_updates += usize::from(self.collision_index.set(i, bounds));
            } else {
                stats.index_updates += usize::from(self.collision_index.remove(i));
            }
        }
        // Only colliders that respond look for partners, in the tree of
        // every present collider: a pair needs one that responds, so a
        // fixed, kinematic, or sleeping collider is found by its partner
        // and never queries itself. A rubble pile frozen at rest so costs
        // no queries. Pairs are then visited in the exhaustive order, lower
        // collider first, so the contacts are the same as checking every
        // pair.
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for (i, &bounds) in bounds.iter().enumerate() {
            let a = &self.colliders()[i];
            if a.filter == Filter::NONE || !self[a.body].responds() {
                continue;
            }
            for j in self.collision_index.query(bounds, &mut stats.scene_nodes) {
                if j == i {
                    continue;
                }
                let other_responds = self[self.colliders()[j].body].responds();
                // Two that respond: the lower one's query keeps the pair.
                if other_responds && j < i {
                    continue;
                }
                pairs.push((i.min(j), i.max(j)));
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        let mut manifolds = Vec::new();
        for (i, j) in pairs {
            let a = &self.colliders()[i];
            {
                stats.candidate_pairs += 1;
                let b = &self.colliders()[j];
                if a.body == b.body
                    || !a.filter.allows(b.filter)
                    || (!self[a.body].responds() && !self[b.body].responds())
                {
                    continue;
                }
                stats.filtered_pairs += 1;
                let m = margin(i, j);
                stats.bound_tests += 1;
                let (pa, ca, ra) = placed[i];
                let (pb, cb, rb) = placed[j];
                if ca.distance(cb) > ra + rb + m {
                    continue;
                }
                stats.narrow_phase += 1;
                let points = contact(pa, pb, m);
                if !points.is_empty() {
                    manifolds.push(Manifold {
                        a: ColliderId(i as u32),
                        b: ColliderId(j as u32),
                        points,
                    });
                }
            }
        }
        Ok((manifolds, stats))
    }
}

#[cfg(test)]
mod index_tests;
