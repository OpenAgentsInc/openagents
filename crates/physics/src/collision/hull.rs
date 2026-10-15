//! Convex hulls: a Quickhull build from points, polygon faces, and mass
//! properties.
//!
//! The build is the incremental Quickhull of Barber, Dobkin, and Huhdanpaa
//! ("The Quickhull Algorithm for Convex Hulls", ACM TOMS 1996): a starting
//! tetrahedron from extreme points, each point outside a face assigned to
//! that face, and the farthest outside point added in turn, replacing the
//! faces it sees with a cone to their horizon. Triangles that lie in one
//! plane are then merged into polygon faces, the faces a contact clips
//! against. Every loop runs in index order and every tie goes to the lower
//! index, so the same points always give the same hull, bit for bit.
//!
//! Mass properties sum signed tetrahedra from a fan of each face (the
//! covariance form of Blow and Binstock, "How to find the inertia tensor
//! (or other mass properties) of a 3D solid body represented by a triangle
//! mesh", 2004), and [`principal`] diagonalizes the tensor with a fixed
//! number of cyclic Jacobi sweeps.
//!
//! Hull colliders follow the shape of Genesis's (genesis-world `df30b61a`,
//! Apache-2.0, `genesis/engine/solvers/rigid/collider/`), reimplemented from
//! the published algorithms; no Genesis code is translated.
//!
//! **Degenerate input.** A hull needs a volume. [`ConvexHull::new`] refuses
//! non-finite points, fewer than four distinct points, more than
//! [`ConvexHull::MAX_POINTS`], and points that all lie within the build
//! tolerance of one plane (a flat, a line, or a point); the caller decides
//! what such a part should be (a thin box, say). Points within the
//! tolerance of a face count as on it and are dropped, so near-duplicates
//! and points on faces never make slivers.

use glam::{DMat3, DQuat, DVec3};
use serde::{Deserialize, Serialize};

/// Why a hull could not be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HullError {
    /// A coordinate was NaN or infinite.
    NonFinite,
    /// Fewer than four distinct points.
    TooFew,
    /// More than [`ConvexHull::MAX_POINTS`] points.
    TooMany,
    /// Every point lies within the tolerance of one plane: no volume.
    Flat,
}

impl std::fmt::Display for HullError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NonFinite => "a hull point is not finite",
            Self::TooFew => "a hull needs at least four distinct points",
            Self::TooMany => "too many hull points",
            Self::Flat => "the hull points lie in one plane and enclose no volume",
        })
    }
}

impl std::error::Error for HullError {}

/// A polygon face of a hull: its outward unit normal, its plane offset
/// (`normal . x = offset` on the face), and its corners, counterclockwise
/// seen from outside.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HullFace {
    pub normal: DVec3,
    pub offset: f64,
    pub vertices: Vec<u32>,
}

/// A convex polyhedron in its collider frame, m.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConvexHull {
    vertices: Vec<DVec3>,
    faces: Vec<HullFace>,
    /// Each edge once, lower index first.
    edges: Vec<[u32; 2]>,
    volume: f64,
    centroid: DVec3,
    /// Inertia tensor per unit density about the centroid, m^5.
    inertia: DMat3,
    /// Largest distance of a vertex from the frame origin, m.
    radius: f64,
}

static HULLS: crate::interned::Registry<ConvexHull> = crate::interned::Registry::new();

/// A `Copy` handle to a hull's shared data (see `interned`): what a
/// [`crate::Shape::Hull`] holds. It serializes as the hull itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HullRef {
    index: u32,
    radius: f64,
}

impl HullRef {
    /// Register `hull` (or find the identical one already registered).
    #[must_use]
    pub fn new(hull: ConvexHull) -> Self {
        let radius = hull.radius;
        Self {
            index: HULLS.intern(hull),
            radius,
        }
    }

    /// The hull's data.
    #[must_use]
    pub fn get(self) -> std::sync::Arc<ConvexHull> {
        HULLS.get(self.index)
    }

    /// [`ConvexHull::radius`], without a lookup.
    #[must_use]
    pub fn radius(self) -> f64 {
        self.radius
    }
}

impl Serialize for HullRef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.get().serialize(s)
    }
}

impl<'de> Deserialize<'de> for HullRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        ConvexHull::deserialize(d).map(Self::new)
    }
}

/// Mass, center of mass, and inertia tensor about it, in the hull frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassProperties {
    pub mass: f64,
    pub center: DVec3,
    pub inertia: DMat3,
}

/// A triangle under construction.
struct Tri {
    v: [usize; 3],
    normal: DVec3,
    offset: f64,
    outside: Vec<usize>,
    alive: bool,
}

impl ConvexHull {
    /// The most input points a build accepts.
    pub const MAX_POINTS: usize = 4096;

    /// The hull of `points`.
    ///
    /// # Errors
    ///
    /// See the module's degenerate-input policy.
    pub fn new(points: &[DVec3]) -> Result<Self, HullError> {
        if points.iter().any(|p| !p.is_finite()) {
            return Err(HullError::NonFinite);
        }
        if points.len() > Self::MAX_POINTS {
            return Err(HullError::TooMany);
        }
        if points.len() < 4 {
            return Err(HullError::TooFew);
        }
        let (lo, hi) = points.iter().fold((points[0], points[0]), |(lo, hi), p| {
            (lo.min(*p), hi.max(*p))
        });
        let scale = (hi - lo).length().max(hi.abs().max(lo.abs()).max_element());
        if scale == 0.0 {
            return Err(HullError::TooFew);
        }
        // Distances below this count as zero: far above rounding in the
        // plane tests, far below any feature a collider cares about.
        let eps = scale * 1e-9;
        let tris = quickhull(points, eps)?;
        Ok(Self::from_triangles(points, &tris, scale))
    }

    /// A box with half extents `half`, centered on the origin.
    ///
    /// # Errors
    ///
    /// A half extent is not positive and finite.
    pub fn cuboid(half: DVec3) -> Result<Self, HullError> {
        let corners: Vec<DVec3> = (0..8)
            .map(|i| {
                DVec3::new(
                    if i & 1 == 0 { -half.x } else { half.x },
                    if i & 2 == 0 { -half.y } else { half.y },
                    if i & 4 == 0 { -half.z } else { half.z },
                )
            })
            .collect();
        Self::new(&corners)
    }

    /// An irregular rock: the hull of `count` points on an ellipsoid of
    /// half extents `half`, each pulled in by up to a third of its radius,
    /// from a seeded generator, so the same seed is the same rock.
    ///
    /// # Errors
    ///
    /// As [`ConvexHull::new`] (for example, a zero half extent).
    pub fn rock(seed: u64, half: DVec3, count: usize) -> Result<Self, HullError> {
        let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1_u64 << 53) as f64
        };
        let points: Vec<DVec3> = (0..count.max(4))
            .map(|_| {
                let z = 2.0 * next() - 1.0;
                let angle = std::f64::consts::TAU * next();
                let r = (1.0 - z * z).max(0.0).sqrt();
                let dent = 1.0 - next() / 3.0;
                DVec3::new(r * angle.cos(), z, r * angle.sin()) * half * dent
            })
            .collect();
        Self::new(&points)
    }

    /// Merge coplanar triangles into polygons and measure the result.
    fn from_triangles(points: &[DVec3], tris: &[[usize; 3]], scale: f64) -> Self {
        let plane_eps = scale * 1e-7;
        let normal_of = |t: &[usize; 3]| {
            (points[t[1]] - points[t[0]])
                .cross(points[t[2]] - points[t[0]])
                .normalize_or_zero()
        };
        let mut grouped = vec![false; tris.len()];
        // (normal, member triangles)
        let mut groups: Vec<(DVec3, Vec<usize>)> = Vec::new();
        for i in 0..tris.len() {
            if grouped[i] {
                continue;
            }
            let n = normal_of(&tris[i]);
            let d = n.dot(points[tris[i][0]]);
            let mut members = vec![i];
            grouped[i] = true;
            for j in (i + 1)..tris.len() {
                if grouped[j] {
                    continue;
                }
                let m = normal_of(&tris[j]);
                if m.dot(n) > 1.0 - 1e-9
                    && tris[j]
                        .iter()
                        .all(|&k| (n.dot(points[k]) - d).abs() <= plane_eps)
                {
                    grouped[j] = true;
                    members.push(j);
                }
            }
            groups.push((n, members));
        }
        // Each group becomes one polygon: the 2D hull of its corners.
        let mut polygons: Vec<(DVec3, Vec<usize>)> = Vec::new();
        for (n, members) in &groups {
            // An area-weighted normal of the merged triangles.
            let mut normal = DVec3::ZERO;
            let mut corners: Vec<usize> = Vec::new();
            for &t in members {
                let [a, b, c] = tris[t];
                normal += (points[b] - points[a]).cross(points[c] - points[a]);
                corners.extend([a, b, c]);
            }
            let normal = normal.normalize_or(*n);
            corners.sort_unstable();
            corners.dedup();
            let polygon = convex_polygon(points, &corners, normal, plane_eps);
            if polygon.len() >= 3 {
                polygons.push((normal, polygon));
            }
        }
        // Keep the vertices some face uses, in input order.
        let mut used: Vec<usize> = polygons
            .iter()
            .flat_map(|(_, p)| p.iter().copied())
            .collect();
        used.sort_unstable();
        used.dedup();
        let index_of = |k: usize| used.binary_search(&k).map_or(0, |i| i as u32);
        let vertices: Vec<DVec3> = used.iter().map(|&k| points[k]).collect();
        let faces: Vec<HullFace> = polygons
            .into_iter()
            .map(|(normal, polygon)| {
                let vertices_of: Vec<u32> = polygon.iter().map(|&k| index_of(k)).collect();
                let offset = polygon
                    .iter()
                    .map(|&k| normal.dot(points[k]))
                    .fold(f64::NEG_INFINITY, f64::max);
                HullFace {
                    normal,
                    offset,
                    vertices: vertices_of,
                }
            })
            .collect();
        let mut edges: Vec<[u32; 2]> = faces
            .iter()
            .flat_map(|f| {
                let n = f.vertices.len();
                (0..n).map(move |i| {
                    let (a, b) = (f.vertices[i], f.vertices[(i + 1) % n]);
                    [a.min(b), a.max(b)]
                })
            })
            .collect();
        edges.sort_unstable();
        edges.dedup();
        let radius = vertices.iter().map(|v| v.length()).fold(0.0, f64::max);
        let mut hull = Self {
            vertices,
            faces,
            edges,
            volume: 0.0,
            centroid: DVec3::ZERO,
            inertia: DMat3::ZERO,
            radius,
        };
        hull.measure();
        hull
    }

    /// Volume, centroid, and the inertia tensor per unit density.
    fn measure(&mut self) {
        let reference = self.vertices.iter().copied().sum::<DVec3>() / self.vertices.len() as f64;
        let canonical = DMat3::from_cols_array(&[2.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 2.0]);
        let mut volume = 0.0;
        let mut moment = DVec3::ZERO;
        let mut covariance = DMat3::ZERO;
        for face in &self.faces {
            let v0 = self.vertices[face.vertices[0] as usize] - reference;
            for k in 1..face.vertices.len() - 1 {
                let a = v0;
                let b = self.vertices[face.vertices[k] as usize] - reference;
                let c = self.vertices[face.vertices[k + 1] as usize] - reference;
                let m = DMat3::from_cols(a, b, c);
                let det = m.determinant();
                let v = det / 6.0;
                volume += v;
                moment += (a + b + c) * (v / 4.0);
                covariance += m * canonical * m.transpose() * (det / 120.0);
            }
        }
        let centroid = moment / volume;
        // Shift the covariance from the reference point to the centroid.
        let shifted = covariance - outer(centroid, centroid) * volume;
        let trace = shifted.x_axis.x + shifted.y_axis.y + shifted.z_axis.z;
        self.volume = volume;
        self.centroid = reference + centroid;
        self.inertia = DMat3::from_diagonal(DVec3::splat(trace)) - shifted;
    }

    /// Corners, m.
    #[must_use]
    pub fn vertices(&self) -> &[DVec3] {
        &self.vertices
    }

    #[must_use]
    pub fn faces(&self) -> &[HullFace] {
        &self.faces
    }

    #[must_use]
    pub fn edges(&self) -> &[[u32; 2]] {
        &self.edges
    }

    /// Volume, m^3.
    #[must_use]
    pub fn volume(&self) -> f64 {
        self.volume
    }

    /// Center of volume in the hull frame, m.
    #[must_use]
    pub fn centroid(&self) -> DVec3 {
        self.centroid
    }

    /// Largest distance of a corner from the hull frame's origin, m.
    #[must_use]
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// Mass properties at uniform `density`, kg/m^3.
    #[must_use]
    pub fn mass_properties(&self, density: f64) -> MassProperties {
        MassProperties {
            mass: self.volume * density,
            center: self.centroid,
            inertia: self.inertia * density,
        }
    }

    /// The corner farthest along `direction` (the lowest index on a tie).
    #[must_use]
    pub fn support(&self, direction: DVec3) -> DVec3 {
        let mut best = self.vertices[0];
        let mut far = best.dot(direction);
        for v in &self.vertices[1..] {
            let d = v.dot(direction);
            if d > far {
                far = d;
                best = *v;
            }
        }
        best
    }

    /// Surface area, m^2.
    #[must_use]
    pub fn area(&self) -> f64 {
        self.faces
            .iter()
            .map(|f| polygon_area(&self.corners(f), f.normal))
            .sum()
    }

    fn corners(&self, face: &HullFace) -> Vec<DVec3> {
        face.vertices
            .iter()
            .map(|&k| self.vertices[k as usize])
            .collect()
    }

    /// The first entry of the ray `origin + t dir` (`dir` unit, hull
    /// frame) for `t` in [0, `max`], with the outward normal there. A ray
    /// starting inside hits at 0 facing back along itself.
    #[must_use]
    pub fn ray(&self, origin: DVec3, dir: DVec3, max: f64) -> Option<(f64, DVec3)> {
        let (mut near, mut far) = (0.0_f64, max);
        let mut normal = None;
        for f in &self.faces {
            let denom = f.normal.dot(dir);
            let dist = f.offset - f.normal.dot(origin);
            if denom.abs() < 1e-15 {
                if dist < 0.0 {
                    return None;
                }
                continue;
            }
            let t = dist / denom;
            if denom < 0.0 {
                if t > near {
                    near = t;
                    normal = Some(f.normal);
                }
            } else {
                far = far.min(t);
            }
            if near > far {
                return None;
            }
        }
        Some((near, normal.unwrap_or(-dir)))
    }

    /// The part of the hull (placed at `pos`, `rotation`) on the side of
    /// the plane through `point` opposite its unit `normal`: volume, its
    /// centroid, and the area of the hull's surface there. Exact: each face
    /// is clipped by the plane and the clipped faces are summed as signed
    /// tetrahedra from a point on the plane, where the cut itself adds no
    /// volume.
    #[must_use]
    pub fn below(
        &self,
        pos: DVec3,
        rotation: DQuat,
        point: DVec3,
        normal: DVec3,
    ) -> (f64, DVec3, f64) {
        let m = DMat3::from_quat(rotation);
        let world: Vec<DVec3> = self.vertices.iter().map(|v| pos + m * *v).collect();
        let apex = point;
        let (mut volume, mut moment, mut area) = (0.0, DVec3::ZERO, 0.0);
        for f in &self.faces {
            let polygon: Vec<DVec3> = f.vertices.iter().map(|&k| world[k as usize]).collect();
            let kept = clip_below(&polygon, normal, normal.dot(point));
            if kept.len() < 3 {
                continue;
            }
            area += polygon_area(&kept, m * f.normal);
            for k in 1..kept.len() - 1 {
                let (a, b, c) = (kept[0] - apex, kept[k] - apex, kept[k + 1] - apex);
                let v = a.dot(b.cross(c)) / 6.0;
                volume += v;
                moment += (a + b + c) * (v / 4.0);
            }
        }
        if volume <= 0.0 {
            return (0.0, pos + m * self.centroid, area);
        }
        (volume, apex + moment / volume, area)
    }

    /// Signed distance from `x` to the nearest face plane: negative inside,
    /// exact inside the hull, a lower bound on the distance outside.
    #[must_use]
    pub fn plane_distance(&self, x: DVec3) -> (f64, usize) {
        let mut best = (f64::NEG_INFINITY, 0);
        for (i, f) in self.faces.iter().enumerate() {
            let d = f.normal.dot(x) - f.offset;
            if d > best.0 {
                best = (d, i);
            }
        }
        best
    }
}

impl crate::World {
    /// A dynamic body of uniform `density`, kg/m^3, shaped as `hull`, with
    /// the hull's frame at `pos` and `rotation`. The body's frame is the
    /// hull's principal frame at its centroid; the collider sits in it.
    pub fn add_hull_body(
        &mut self,
        hull: HullRef,
        density: f64,
        pos: DVec3,
        rotation: DQuat,
    ) -> (crate::BodyId, crate::ColliderId) {
        let m = hull.get().mass_properties(density);
        let (moments, frame) = principal(m.inertia);
        let mut body = crate::Body::new(m.mass, moments, pos + rotation * m.center);
        body.orientation = (rotation * frame).normalize();
        body.prev_orientation = body.orientation;
        let id = self.add(body);
        let inverse = frame.inverse();
        let collider = self.add_collider(
            crate::Collider::new(id, crate::Shape::Hull { hull }).at(inverse * -m.center, inverse),
        );
        (id, collider)
    }

    /// One dynamic body of uniform `density` made of several hulls that
    /// share a frame (a convex decomposition of a non-convex part), with
    /// that frame at `pos` and `rotation`. Mass properties are summed over
    /// the pieces (parallel axis theorem), and each piece is a collider.
    ///
    /// # Panics
    ///
    /// `hulls` is empty.
    pub fn add_compound_body(
        &mut self,
        hulls: &[HullRef],
        density: f64,
        pos: DVec3,
        rotation: DQuat,
    ) -> (crate::BodyId, Vec<crate::ColliderId>) {
        assert!(!hulls.is_empty(), "a compound body needs a hull");
        let parts: Vec<MassProperties> = hulls
            .iter()
            .map(|h| h.get().mass_properties(density))
            .collect();
        let mass: f64 = parts.iter().map(|p| p.mass).sum();
        let center = parts.iter().map(|p| p.center * p.mass).sum::<DVec3>() / mass;
        let mut inertia = DMat3::ZERO;
        for p in &parts {
            let d = p.center - center;
            inertia += p.inertia
                + (DMat3::from_diagonal(DVec3::splat(d.length_squared())) - outer(d, d)) * p.mass;
        }
        let (moments, frame) = principal(inertia);
        let mut body = crate::Body::new(mass, moments, pos + rotation * center);
        body.orientation = (rotation * frame).normalize();
        body.prev_orientation = body.orientation;
        let id = self.add(body);
        let inverse = frame.inverse();
        let colliders = hulls
            .iter()
            .map(|&hull| {
                self.add_collider(
                    crate::Collider::new(id, crate::Shape::Hull { hull })
                        .at(inverse * -center, inverse),
                )
            })
            .collect();
        (id, colliders)
    }
}

/// Area of a planar polygon with unit normal `normal`.
fn polygon_area(polygon: &[DVec3], normal: DVec3) -> f64 {
    let mut twice = DVec3::ZERO;
    for k in 1..polygon.len().saturating_sub(1) {
        twice += (polygon[k] - polygon[0]).cross(polygon[k + 1] - polygon[0]);
    }
    0.5 * twice.dot(normal).abs()
}

/// The part of a convex polygon where `x . n <= limit`.
fn clip_below(polygon: &[DVec3], n: DVec3, limit: f64) -> Vec<DVec3> {
    let mut out = Vec::with_capacity(polygon.len() + 2);
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

/// `a b^T`.
fn outer(a: DVec3, b: DVec3) -> DMat3 {
    DMat3::from_cols(a * b.x, a * b.y, a * b.z)
}

/// Principal moments and the rotation from the principal frame to the
/// tensor's frame: `inertia = R diag(moments) R^T`, with `R` a proper
/// rotation. Cyclic Jacobi with a fixed sweep limit, so it is
/// deterministic.
#[must_use]
pub fn principal(inertia: DMat3) -> (DVec3, DQuat) {
    let mut a = [
        [inertia.x_axis.x, inertia.y_axis.x, inertia.z_axis.x],
        [inertia.x_axis.y, inertia.y_axis.y, inertia.z_axis.y],
        [inertia.x_axis.z, inertia.y_axis.z, inertia.z_axis.z],
    ];
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let scale = (0..3)
        .map(|i| a[i][i].abs())
        .fold(0.0, f64::max)
        .max(1e-300);
    for _ in 0..32 {
        let off = a[0][1].abs() + a[0][2].abs() + a[1][2].abs();
        if off <= scale * 1e-15 {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q].abs() <= scale * 1e-18 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let t = if theta == 0.0 { 1.0 } else { t };
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for k in 0..3 {
                let (akp, akq) = (a[k][p], a[k][q]);
                a[k][p] = c * akp - s * akq;
                a[k][q] = s * akp + c * akq;
            }
            for k in 0..3 {
                let (apk, aqk) = (a[p][k], a[q][k]);
                a[p][k] = c * apk - s * aqk;
                a[q][k] = s * apk + c * aqk;
            }
            for row in &mut v {
                let (vp, vq) = (row[p], row[q]);
                row[p] = c * vp - s * vq;
                row[q] = s * vp + c * vq;
            }
        }
    }
    let mut r = DMat3::from_cols(
        DVec3::new(v[0][0], v[1][0], v[2][0]),
        DVec3::new(v[0][1], v[1][1], v[2][1]),
        DVec3::new(v[0][2], v[1][2], v[2][2]),
    );
    if r.determinant() < 0.0 {
        r.z_axis = -r.z_axis;
    }
    (
        DVec3::new(a[0][0], a[1][1], a[2][2]),
        DQuat::from_mat3(&r).normalize(),
    )
}

/// The corners of the planar point set `ids` in counterclockwise order about
/// `normal`, collinear points dropped (Andrew's monotone chain).
fn convex_polygon(points: &[DVec3], ids: &[usize], normal: DVec3, eps: f64) -> Vec<usize> {
    let u = normal.any_orthonormal_vector();
    let v = normal.cross(u);
    let mut pts: Vec<(f64, f64, usize)> = ids
        .iter()
        .map(|&k| (points[k].dot(u), points[k].dot(v), k))
        .collect();
    pts.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then(a.1.total_cmp(&b.1))
            .then(a.2.cmp(&b.2))
    });
    let cross = |o: (f64, f64, usize), a: (f64, f64, usize), b: (f64, f64, usize)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    // Twice the area of a sliver below this is a straight line.
    let tol = eps * eps.sqrt().max(1e-6);
    let mut lower: Vec<(f64, f64, usize)> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= tol {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<(f64, f64, usize)> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= tol {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower.into_iter().map(|p| p.2).collect()
}

/// Quickhull: the hull's triangles, counterclockwise from outside.
fn quickhull(points: &[DVec3], eps: f64) -> Result<Vec<[usize; 3]>, HullError> {
    let n = points.len();
    // Extreme points along the axes; the widest pair starts the simplex.
    let mut extremes = [0usize; 6];
    for (i, p) in points.iter().enumerate() {
        for axis in 0..3 {
            if p[axis] < points[extremes[axis * 2]][axis] {
                extremes[axis * 2] = i;
            }
            if p[axis] > points[extremes[axis * 2 + 1]][axis] {
                extremes[axis * 2 + 1] = i;
            }
        }
    }
    let (mut a, mut b, mut widest) = (0, 0, -1.0);
    for axis in 0..3 {
        let (i, j) = (extremes[axis * 2], extremes[axis * 2 + 1]);
        let d = points[i].distance_squared(points[j]);
        if d > widest {
            widest = d;
            a = i;
            b = j;
        }
    }
    if widest.sqrt() <= eps {
        return Err(HullError::TooFew);
    }
    // The point farthest from the line ab.
    let ab = (points[b] - points[a]).normalize();
    let (mut c, mut far) = (0, -1.0);
    for (i, p) in points.iter().enumerate() {
        let r = *p - points[a];
        let d = (r - ab * r.dot(ab)).length_squared();
        if d > far {
            far = d;
            c = i;
        }
    }
    if far.sqrt() <= eps {
        return Err(HullError::Flat);
    }
    // The point farthest from the plane abc.
    let plane = (points[b] - points[a])
        .cross(points[c] - points[a])
        .normalize();
    let (mut d, mut far) = (0, -1.0);
    for (i, p) in points.iter().enumerate() {
        let h = (*p - points[a]).dot(plane).abs();
        if h > far {
            far = h;
            d = i;
        }
    }
    if far <= eps {
        return Err(HullError::Flat);
    }
    let make = |v: [usize; 3]| {
        let normal = (points[v[1]] - points[v[0]])
            .cross(points[v[2]] - points[v[0]])
            .normalize_or_zero();
        Tri {
            v,
            normal,
            offset: normal.dot(points[v[0]]),
            outside: Vec::new(),
            alive: true,
        }
    };
    // Orient the tetrahedron's faces outward.
    let inside = (points[a] + points[b] + points[c] + points[d]) * 0.25;
    let mut tris: Vec<Tri> = Vec::new();
    for v in [[a, b, c], [a, d, b], [b, d, c], [c, d, a]] {
        let mut t = make(v);
        if t.normal.dot(inside) - t.offset > 0.0 {
            t = make([v[0], v[2], v[1]]);
        }
        tris.push(t);
    }
    let simplex = [a, b, c, d];
    for i in 0..n {
        if simplex.contains(&i) {
            continue;
        }
        assign(&mut tris, 0, i, points, eps);
    }
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 4 * n + 16 {
            break;
        }
        // The first face with outside points, and its farthest point.
        let Some(f) = tris.iter().position(|t| t.alive && !t.outside.is_empty()) else {
            break;
        };
        let (mut eye, mut far) = (tris[f].outside[0], f64::NEG_INFINITY);
        for &p in &tris[f].outside {
            let h = tris[f].normal.dot(points[p]) - tris[f].offset;
            if h > far {
                far = h;
                eye = p;
            }
        }
        // Every face the eye sees.
        let visible: Vec<usize> = (0..tris.len())
            .filter(|&k| tris[k].alive && tris[k].normal.dot(points[eye]) - tris[k].offset > eps)
            .collect();
        // Horizon: edges of visible faces whose twin is on a hidden face.
        let mut horizon: Vec<[usize; 2]> = Vec::new();
        for &k in &visible {
            let v = tris[k].v;
            for e in 0..3 {
                let (p, q) = (v[e], v[(e + 1) % 3]);
                let twin_visible = visible.iter().any(|&o| {
                    let w = tris[o].v;
                    (0..3).any(|g| w[g] == q && w[(g + 1) % 3] == p)
                });
                if !twin_visible {
                    horizon.push([p, q]);
                }
            }
        }
        let mut orphans: Vec<usize> = Vec::new();
        for &k in &visible {
            tris[k].alive = false;
            orphans.append(&mut tris[k].outside);
        }
        let first = tris.len();
        for [p, q] in horizon {
            tris.push(make([p, q, eye]));
        }
        orphans.sort_unstable();
        for p in orphans {
            if p != eye {
                assign(&mut tris, first, p, points, eps);
            }
        }
    }
    Ok(tris
        .into_iter()
        .filter(|t| t.alive && t.normal != DVec3::ZERO)
        .map(|t| t.v)
        .collect())
}

/// Put point `p` in the outside set of the first live face, from `from` on
/// and then the older ones, that it is above by more than `eps`.
fn assign(tris: &mut [Tri], from: usize, p: usize, points: &[DVec3], eps: f64) {
    let (old, new) = tris.split_at_mut(from);
    for t in new.iter_mut().chain(old.iter_mut()) {
        if t.alive && t.normal.dot(points[p]) - t.offset > eps {
            t.outside.push(p);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> ConvexHull {
        ConvexHull::cuboid(DVec3::new(0.5, 1.0, 1.5)).unwrap()
    }

    #[test]
    fn a_box_hull_has_six_quads_and_exact_mass_properties() {
        let hull = cube();
        assert_eq!(hull.vertices().len(), 8);
        assert_eq!(hull.faces().len(), 6);
        assert_eq!(hull.edges().len(), 12);
        assert!(hull.faces().iter().all(|f| f.vertices.len() == 4));
        assert!((hull.volume() - 6.0).abs() < 1e-12);
        assert!(hull.centroid().length() < 1e-12);
        // A box's inertia per unit density: V (b^2 + c^2) / 12 with full sizes.
        let m = hull.mass_properties(2.0);
        let expect = crate::Body::box_inertia(12.0, DVec3::new(1.0, 2.0, 3.0));
        let got = DVec3::new(m.inertia.x_axis.x, m.inertia.y_axis.y, m.inertia.z_axis.z);
        assert!((got - expect).length() < 1e-9, "{got} vs {expect}");
        assert!(m.inertia.x_axis.y.abs() < 1e-12);
        for f in hull.faces() {
            // Counterclockwise from outside: the polygon's own normal agrees.
            let v: Vec<DVec3> = f
                .vertices
                .iter()
                .map(|&k| hull.vertices()[k as usize])
                .collect();
            let own = (v[1] - v[0]).cross(v[2] - v[0]);
            assert!(own.dot(f.normal) > 0.0);
            for p in hull.vertices() {
                assert!(f.normal.dot(*p) - f.offset < 1e-12);
            }
        }
    }

    #[test]
    fn interior_and_duplicate_points_do_not_change_the_hull() {
        let base = cube();
        let mut points: Vec<DVec3> = base.vertices().to_vec();
        points.push(DVec3::ZERO);
        points.push(DVec3::new(0.1, 0.2, -0.3));
        points.push(points[0]);
        // A point on a face and one on an edge.
        points.push(DVec3::new(0.5, 0.0, 0.0));
        points.push(DVec3::new(0.5, 1.0, 0.0));
        let hull = ConvexHull::new(&points).unwrap();
        assert_eq!(hull.vertices().len(), 8);
        assert_eq!(hull.faces().len(), 6);
        assert!((hull.volume() - base.volume()).abs() < 1e-12);
    }

    #[test]
    fn degenerate_input_is_refused() {
        let flat: Vec<DVec3> = (0..10)
            .map(|i| DVec3::new(f64::from(i).sin(), f64::from(i).cos(), 0.0))
            .collect();
        assert_eq!(ConvexHull::new(&flat), Err(HullError::Flat));
        let line: Vec<DVec3> = (0..5).map(|i| DVec3::X * f64::from(i)).collect();
        assert_eq!(ConvexHull::new(&line), Err(HullError::Flat));
        assert_eq!(ConvexHull::new(&[DVec3::ONE; 6]), Err(HullError::TooFew));
        assert_eq!(ConvexHull::new(&flat[..3]), Err(HullError::TooFew));
        let mut bad = cube().vertices().to_vec();
        bad.push(DVec3::new(f64::NAN, 0.0, 0.0));
        assert_eq!(ConvexHull::new(&bad), Err(HullError::NonFinite));
        // Nearly flat but with real thickness builds.
        let mut slab = flat.clone();
        slab.push(DVec3::new(0.0, 0.0, 1e-3));
        assert!(ConvexHull::new(&slab).is_ok());
    }

    /// Points on a sphere: every one is a corner, the hull is closed (each
    /// edge in exactly two faces), the volume approaches the sphere's, and
    /// the inertia is that of a ball.
    #[test]
    fn a_sphere_cloud_hull_is_closed_and_measures_like_a_ball() {
        let n = 400;
        let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
        let points: Vec<DVec3> = (0..n)
            .map(|i| {
                let y = 1.0 - 2.0 * (f64::from(i) + 0.5) / f64::from(n);
                let r = (1.0 - y * y).sqrt();
                let t = golden * f64::from(i);
                DVec3::new(r * t.cos(), y, r * t.sin()) + DVec3::new(3.0, -1.0, 2.0)
            })
            .collect();
        let hull = ConvexHull::new(&points).unwrap();
        assert_eq!(hull.vertices().len(), n as usize);
        let mut uses = std::collections::BTreeMap::new();
        for f in hull.faces() {
            let k = f.vertices.len();
            for i in 0..k {
                let (a, b) = (f.vertices[i], f.vertices[(i + 1) % k]);
                *uses.entry([a.min(b), a.max(b)]).or_insert(0) += 1;
            }
        }
        assert!(uses.values().all(|&u| u == 2));
        // Euler: V - E + F = 2.
        let (v, e, f) = (
            hull.vertices().len(),
            hull.edges().len(),
            hull.faces().len(),
        );
        assert_eq!(v + f, e + 2);
        let ball = 4.0 / 3.0 * std::f64::consts::PI;
        assert!(hull.volume() < ball && hull.volume() > 0.98 * ball);
        assert!((hull.centroid() - DVec3::new(3.0, -1.0, 2.0)).length() < 1e-3);
        let (moments, _) = principal(hull.mass_properties(1.0).inertia);
        let expect = 0.4 * hull.volume();
        for k in 0..3 {
            assert!((moments[k] - expect).abs() < 0.02 * expect, "{moments}");
        }
    }

    #[test]
    fn principal_axes_diagonalize_a_rotated_box() {
        let rotation = DQuat::from_euler(glam::EulerRot::XYZ, 0.3, -0.7, 1.1);
        let points: Vec<DVec3> = cube().vertices().iter().map(|v| rotation * *v).collect();
        let hull = ConvexHull::new(&points).unwrap();
        let inertia = hull.mass_properties(1.0).inertia;
        let (moments, frame) = principal(inertia);
        let r = DMat3::from_quat(frame);
        let back = r * DMat3::from_diagonal(moments) * r.transpose();
        for k in 0..3 {
            assert!((back.col(k) - inertia.col(k)).length() < 1e-9);
        }
        let mut sorted = moments.to_array();
        sorted.sort_by(f64::total_cmp);
        let mut expect = crate::Body::box_inertia(6.0, DVec3::new(1.0, 2.0, 3.0)).to_array();
        expect.sort_by(f64::total_cmp);
        for k in 0..3 {
            assert!((sorted[k] - expect[k]).abs() < 1e-9);
        }
    }

    #[test]
    fn the_same_points_give_the_same_hull_bit_for_bit() {
        let mut s = 7_u64;
        let mut next = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        let points: Vec<DVec3> = (0..60)
            .map(|_| DVec3::new(next(), next(), next()))
            .collect();
        let a = ConvexHull::new(&points).unwrap();
        let b = ConvexHull::new(&points).unwrap();
        assert_eq!(a, b);
        let json = serde_json::to_string(&a).unwrap();
        let c: ConvexHull = serde_json::from_str(&json).unwrap();
        assert_eq!(a, c);
    }
}
