//! Contacts for convex polytopes (hulls and boxes against hulls) and for
//! rounded cores (spheres and capsules) against polytopes.
//!
//! GJK gives the gap, or EPA the depth, and the normal ([`super::gjk`]).
//! When that normal is a face normal of either shape, the contact is a
//! patch: the other shape's most opposed face is clipped against the side
//! planes of this reference face and its corners below the face become up
//! to four points, the reference/incident scheme our box–box contact
//! already uses (and Genesis's `multi_contact.py` face clipping; reimplemented,
//! not translated). Otherwise (edge on edge, corner on corner) the contact
//! is the single pair of witness points.

use glam::{DMat3, DQuat, DVec3};

use super::gjk::{self, Core, Proximity};
use super::hull::ConvexHull;
use super::{ContactPoint, push_distinct, reduce};

/// A normal this close to a face normal makes that face a reference face.
const FACE_COS: f64 = 0.999;

/// A polytope in the world frame.
#[derive(Clone, Debug)]
pub(crate) struct Poly {
    pub vertices: Vec<DVec3>,
    /// Outward normals and counterclockwise corner indices.
    pub faces: Vec<(DVec3, Vec<u32>)>,
}

impl Poly {
    pub(crate) fn hull(hull: &ConvexHull, pos: DVec3, rotation: DQuat) -> Self {
        let m = DMat3::from_quat(rotation);
        Self {
            vertices: hull.vertices().iter().map(|v| pos + m * *v).collect(),
            faces: hull
                .faces()
                .iter()
                .map(|f| (m * f.normal, f.vertices.clone()))
                .collect(),
        }
    }

    pub(crate) fn cuboid(center: DVec3, axes: DMat3, half: DVec3) -> Self {
        let vertices = (0..8)
            .map(|i| {
                let s = |bit: usize, k: usize| if i & bit == 0 { -half[k] } else { half[k] };
                center + axes.col(0) * s(1, 0) + axes.col(1) * s(2, 1) + axes.col(2) * s(4, 2)
            })
            .collect();
        // Corner index bits: 1 = +x, 2 = +y, 4 = +z.
        let faces = vec![
            (-axes.col(0), vec![0, 4, 6, 2]),
            (axes.col(0), vec![1, 3, 7, 5]),
            (-axes.col(1), vec![0, 1, 5, 4]),
            (axes.col(1), vec![2, 6, 7, 3]),
            (-axes.col(2), vec![0, 2, 3, 1]),
            (axes.col(2), vec![4, 5, 7, 6]),
        ];
        Self { vertices, faces }
    }

    fn core(&self) -> Core<'_> {
        Core::Points(&self.vertices)
    }

    /// The face whose normal is most along `n`, and that alignment.
    fn most_along(&self, n: DVec3) -> (usize, f64) {
        let mut best = (0, f64::NEG_INFINITY);
        for (i, (normal, _)) in self.faces.iter().enumerate() {
            let d = normal.dot(n);
            if d > best.1 {
                best = (i, d);
            }
        }
        best
    }

    fn corner(&self, face: usize, k: usize) -> DVec3 {
        self.vertices[self.faces[face].1[k] as usize]
    }

    fn polygon(&self, face: usize) -> Vec<DVec3> {
        self.faces[face]
            .1
            .iter()
            .map(|&k| self.vertices[k as usize])
            .collect()
    }

    fn radius(&self) -> f64 {
        let c = self.core().center();
        self.vertices
            .iter()
            .map(|v| v.distance(c))
            .fold(0.0, f64::max)
    }
}

/// How far two cores overlap along `n` (from `a` to `b`): positive when
/// their projections overlap.
fn overlap(a: &Core, b: &Core, n: DVec3, radii: f64) -> f64 {
    (a.support(n) - b.support(-n)).dot(n) + radii
}

/// The least overlap over `a`'s face normals and the reverse of `b`'s
/// (`b` may have none), as a proximity: an upper bound on the depth, exact
/// for face contacts.
fn face_axes(a: &Poly, b: &Core, b_faces: &[(DVec3, Vec<u32>)], radius: f64) -> Proximity {
    let core = a.core();
    let mut best = (f64::INFINITY, DVec3::Y);
    let axes = a
        .faces
        .iter()
        .map(|(n, _)| *n)
        .chain(b_faces.iter().map(|(n, _)| -*n));
    for n in axes {
        let o = overlap(&core, b, n, radius);
        if o < best.0 {
            best = (o, n);
        }
    }
    let (depth, n) = best;
    Proximity {
        normal: n,
        distance: radius - depth,
        pa: core.support(n),
        pb: b.support(-n),
    }
}

/// GJK's or EPA's answer if it is consistent, otherwise the face axes'.
/// The gap or depth reported must be the one along its normal: when two
/// shapes barely touch, GJK's closest-point vector is too short to give a
/// direction, and face-on-face contact is where EPA's polytope degenerates.
/// A depth must also be no deeper than the shallowest face axis. In both
/// failures the face axes are exact, since the contact is a face's.
fn checked(
    p: Proximity,
    a: &Poly,
    b: &Core,
    b_faces: &[(DVec3, Vec<u32>)],
    radius: f64,
    scale: f64,
) -> Proximity {
    // A wrong normal is wrong by far more than this; a right one from a
    // converged GJK or EPA is within rounding of it.
    let tol = scale.max(1e-9) * 1e-4;
    let gap = radius - overlap(&a.core(), b, p.normal, radius);
    if p.normal.is_finite() && (gap - p.distance).abs() <= tol && p.distance >= 0.0 {
        return p;
    }
    let faces = face_axes(a, b, b_faces, radius);
    if p.normal.is_finite() && (gap - p.distance).abs() <= tol && p.distance >= faces.distance - tol
    {
        p
    } else {
        faces
    }
}

/// Contacts between two polytopes within `margin`, normals from `a` to `b`.
pub(crate) fn poly_poly(a: &Poly, b: &Poly, margin: f64) -> Vec<ContactPoint> {
    let scale = a.radius() + b.radius();
    let Some(p) = gjk::proximity(&a.core(), &b.core(), margin, scale) else {
        return Vec::new();
    };
    if p.distance > margin {
        return Vec::new();
    }
    let p = checked(p, a, &b.core(), &b.faces, 0.0, scale);
    let n = p.normal;
    let (fa, align_a) = a.most_along(n);
    let (fb, align_b) = b.most_along(-n);
    if align_a.max(align_b) >= FACE_COS {
        // Prefer `a` as the reference unless `b`'s face is clearly better.
        let a_ref = align_a + 1e-6 >= align_b;
        let (reference, rf, incident) = if a_ref { (a, fa, b) } else { (b, fb, a) };
        let ref_normal = reference.faces[rf].0;
        let (fi, _) = incident.most_along(-ref_normal);
        let mut polygon = incident.polygon(fi);
        let corners = reference.polygon(rf);
        let count = corners.len();
        for k in 0..count {
            let (p0, p1) = (corners[k], corners[(k + 1) % count]);
            let side = (p1 - p0).cross(ref_normal).normalize_or_zero();
            if side == DVec3::ZERO {
                continue;
            }
            polygon = super::clip(&polygon, side, side.dot(p0));
            if polygon.is_empty() {
                break;
            }
        }
        let origin = reference.corner(rf, 0);
        let normal = if a_ref { ref_normal } else { -ref_normal };
        let mut points: Vec<ContactPoint> = polygon
            .into_iter()
            .filter_map(|x| {
                let s = (x - origin).dot(ref_normal);
                (s <= margin).then(|| ContactPoint {
                    point: x - ref_normal * (s * 0.5),
                    normal,
                    separation: s,
                })
            })
            .collect();
        reduce(&mut points);
        if !points.is_empty() {
            return points;
        }
    }
    vec![single(&p, 0.0, 0.0)]
}

/// The contact from a proximity of cores with radii `ra` and `rb`.
fn single(p: &Proximity, ra: f64, rb: f64) -> ContactPoint {
    let separation = p.distance - ra - rb;
    let on_a = p.pa + p.normal * ra;
    let on_b = p.pb - p.normal * rb;
    ContactPoint {
        point: (on_a + on_b) * 0.5,
        normal: p.normal,
        separation,
    }
}

/// Contacts between a polytope and a sphere (`p == q`) or capsule core
/// from `p` to `q` swept by `radius`, normals from the polytope.
pub(crate) fn poly_capsule(
    a: &Poly,
    p: DVec3,
    q: DVec3,
    radius: f64,
    margin: f64,
) -> Vec<ContactPoint> {
    let core = if p == q {
        Core::Point(p)
    } else {
        Core::Segment(p, q)
    };
    let scale = a.radius() + p.distance(q) + radius;
    let Some(prox) = gjk::proximity(&a.core(), &core, margin + radius, scale) else {
        return Vec::new();
    };
    let prox = checked(prox, a, &core, &[], 0.0, scale);
    let first = single(&prox, 0.0, radius);
    if first.separation > margin {
        return Vec::new();
    }
    let mut points = vec![first];
    let axis = q - p;
    if axis == DVec3::ZERO {
        return points;
    }
    // A capsule lying along a face rests on two points: its core clipped to
    // the face, each end pushed out by the radius.
    let (face, align) = a.most_along(prox.normal);
    let n = a.faces[face].0;
    if align < FACE_COS || axis.normalize().dot(n).abs() > 0.2 {
        return points;
    }
    let mut segment = vec![p, q];
    let corners = a.polygon(face);
    let count = corners.len();
    for k in 0..count {
        let (p0, p1) = (corners[k], corners[(k + 1) % count]);
        let side = (p1 - p0).cross(n).normalize_or_zero();
        if side == DVec3::ZERO {
            continue;
        }
        segment = clip_segment(&segment, side, side.dot(p0));
        if segment.is_empty() {
            return points;
        }
    }
    let origin = corners[0];
    let spacing = (p.distance(q) * 0.1).max(radius * 0.25);
    for x in segment {
        let s = (x - origin).dot(n) - radius;
        if s <= margin {
            push_distinct(
                &mut points,
                ContactPoint {
                    point: x - n * (radius + s * 0.5),
                    normal: n,
                    separation: s,
                },
                spacing,
            );
        }
    }
    points
}

/// The part of a segment (two points, or none) where `x . n <= limit`.
fn clip_segment(segment: &[DVec3], n: DVec3, limit: f64) -> Vec<DVec3> {
    let [a, b] = [segment[0], segment[1]];
    let (da, db) = (a.dot(n) - limit, b.dot(n) - limit);
    match (da <= 0.0, db <= 0.0) {
        (true, true) => vec![a, b],
        (false, false) => Vec::new(),
        (true, false) => vec![a, a + (b - a) * (da / (da - db))],
        (false, true) => vec![a + (b - a) * (da / (da - db)), b],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_hull() -> ConvexHull {
        ConvexHull::cuboid(DVec3::splat(0.5)).unwrap()
    }

    #[test]
    fn a_hull_resting_on_a_box_touches_at_four_corners() {
        let floor = Poly::cuboid(
            DVec3::new(0.0, -0.5, 0.0),
            DMat3::IDENTITY,
            DVec3::new(5.0, 0.5, 5.0),
        );
        let hull = unit_hull();
        let top = Poly::hull(
            &hull,
            DVec3::new(0.2, 0.499, -0.1),
            DQuat::from_rotation_y(0.4),
        );
        let points = poly_poly(&floor, &top, 0.01);
        assert_eq!(points.len(), 4, "{points:?}");
        for c in &points {
            assert!((c.normal - DVec3::Y).length() < 1e-9);
            assert!((c.separation + 0.001).abs() < 1e-9, "{c:?}");
        }
        // Flipped order: normals flip.
        let points = poly_poly(&top, &floor, 0.01);
        assert_eq!(points.len(), 4);
        assert!(points.iter().all(|c| (c.normal + DVec3::Y).length() < 1e-9));
    }

    #[test]
    fn a_tipped_hull_touches_at_a_corner_or_an_edge() {
        let floor = Poly::cuboid(
            DVec3::new(0.0, -0.5, 0.0),
            DMat3::IDENTITY,
            DVec3::new(5.0, 0.5, 5.0),
        );
        let hull = unit_hull();
        // On an edge: rotated 45 degrees about z, the lowest edge at y = 0.
        let h = 0.5 * 2.0_f64.sqrt();
        let edge = Poly::hull(
            &hull,
            DVec3::new(0.0, h - 0.002, 0.0),
            DQuat::from_rotation_z(std::f64::consts::FRAC_PI_4),
        );
        let points = poly_poly(&floor, &edge, 0.01);
        assert_eq!(points.len(), 2, "{points:?}");
        assert!(points.iter().all(|c| (c.separation + 0.002).abs() < 1e-9));
        // On a corner.
        let tilt =
            DQuat::from_rotation_z(std::f64::consts::FRAC_PI_4) * DQuat::from_rotation_x(0.6155);
        let corner = Poly::hull(&hull, DVec3::new(0.0, 0.8, 0.0), tilt);
        let low = corner
            .vertices
            .iter()
            .map(|v| v.y)
            .fold(f64::INFINITY, f64::min);
        let corner = Poly::hull(&hull, DVec3::new(0.0, 0.8 - low - 0.003, 0.0), tilt);
        let points = poly_poly(&floor, &corner, 0.01);
        assert!(!points.is_empty());
        let deepest = points
            .iter()
            .map(|c| c.separation)
            .fold(f64::INFINITY, f64::min);
        assert!((deepest + 0.003).abs() < 1e-6, "{points:?}");
    }

    #[test]
    fn spheres_and_capsules_meet_hulls_where_geometry_says() {
        let hull = unit_hull();
        let block = Poly::hull(&hull, DVec3::ZERO, DQuat::IDENTITY);
        let ball = DVec3::new(0.0, 0.75, 0.0);
        let points = poly_capsule(&block, ball, ball, 0.3, 0.01);
        assert_eq!(points.len(), 1);
        assert!((points[0].separation + 0.05).abs() < 1e-9);
        assert!((points[0].normal - DVec3::Y).length() < 1e-9);
        // A capsule lying on the top face rests on its ends (and the
        // nearest point between them).
        let (p, q) = (DVec3::new(-0.3, 0.69, 0.0), DVec3::new(0.3, 0.69, 0.1));
        let points = poly_capsule(&block, p, q, 0.2, 0.01);
        assert!(points.len() >= 2, "{points:?}");
        for end in [p, q] {
            assert!(points.iter().any(|c| (c.point.x - end.x).abs() < 1e-9));
        }
        assert!(points.iter().all(|c| (c.separation + 0.01).abs() < 1e-9));
        // A ball deep inside leaves through the nearest face.
        let inside = DVec3::new(0.4, 0.0, 0.0);
        let points = poly_capsule(&block, inside, inside, 0.05, 0.01);
        assert!((points[0].separation + 0.15).abs() < 1e-9, "{points:?}");
        assert!((points[0].normal - DVec3::X).length() < 1e-9);
    }
}
