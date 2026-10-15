//! Static signed-distance colliders: a grid of distances sampled offline
//! from a closed mesh, for non-convex fixed geometry (a bowl, a shell,
//! rubble terrain) that would be costly as many hulls.
//!
//! A dynamic shape meets a grid by sampling its own points against the
//! grid: a hull's or box's corners, edge midpoints, and face centers, a
//! sphere's center, or points along a capsule's core, each with its
//! radius. The grid answers the trilinear distance and its gradient, which
//! is the contact normal. This is the vertex-against-SDF contact of Genesis
//! (genesis-world `df30b61a`, Apache-2.0, `narrowphase.py`
//! `func_contact_vertex_sdf`, `utils/sdf.py`), reimplemented from the idea;
//! no Genesis code is translated.
//!
//! The grid is stored as `f32` and queried in `f64`, capped at
//! [`SdfGrid::MAX_AXIS`] samples along each axis, and read from a versioned
//! byte format ([`SdfGrid::from_bytes`]). [`SdfGrid::bake`] samples a
//! closed triangle mesh: the unsigned distance to the nearest triangle
//! (Ericson, Real-Time Collision Detection, 5.1.5), signed by the
//! generalized winding number (Jacobson et al., "Robust Inside-Outside
//! Segmentation using Generalized Winding Numbers", 2013) with the solid
//! angle of each triangle from Van Oosterom and Strackee (1983). Every
//! sample is computed independently and in a fixed order, so the same mesh
//! bakes to the same bytes.
//!
//! A grid collider is static only: it never moves, and two grids never
//! touch each other.

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ContactPoint;
use super::convex::Poly;

/// Why a grid could not be made or read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SdfError {
    /// A dimension is below 2 or above [`SdfGrid::MAX_AXIS`].
    Resolution,
    /// The spacing, origin, or a sample is not finite and positive.
    Invalid,
    /// The bytes are not a grid file, or not this version.
    Format(String),
    /// The mesh has no triangles or refers to a missing vertex.
    Mesh,
}

impl std::fmt::Display for SdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resolution => write!(
                f,
                "a distance grid has 2 to {} samples along each axis",
                SdfGrid::MAX_AXIS
            ),
            Self::Invalid => {
                f.write_str("a distance grid's spacing, origin, and samples are finite")
            }
            Self::Format(why) => write!(f, "not a distance grid file: {why}"),
            Self::Mesh => f.write_str("the mesh has no triangles or names a missing vertex"),
        }
    }
}

impl std::error::Error for SdfError {}

/// Signed distances on a regular grid in the collider frame, m: negative
/// inside. Sample `(i, j, k)` sits at `origin + cell * (i, j, k)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SdfGrid {
    origin: DVec3,
    cell: f64,
    dims: [u32; 3],
    values: Vec<f32>,
}

const MAGIC: &[u8; 8] = b"OASDFGRD";

impl SdfGrid {
    /// The most samples along one axis.
    pub const MAX_AXIS: u32 = 128;
    /// The file format version [`SdfGrid::to_bytes`] writes.
    pub const VERSION: u32 = 1;

    /// A grid from its samples, x fastest, then y, then z.
    ///
    /// # Errors
    ///
    /// The dimensions are out of range, the sample count does not match, or
    /// a number is not finite.
    pub fn new(
        origin: DVec3,
        cell: f64,
        dims: [u32; 3],
        values: Vec<f32>,
    ) -> Result<Self, SdfError> {
        if dims.iter().any(|&d| !(2..=Self::MAX_AXIS).contains(&d)) {
            return Err(SdfError::Resolution);
        }
        let count = dims.iter().map(|&d| d as usize).product::<usize>();
        if !origin.is_finite()
            || !cell.is_finite()
            || cell <= 0.0
            || values.len() != count
            || values.iter().any(|v| !v.is_finite())
        {
            return Err(SdfError::Invalid);
        }
        Ok(Self {
            origin,
            cell,
            dims,
            values,
        })
    }

    /// Lowest corner, m.
    #[must_use]
    pub fn origin(&self) -> DVec3 {
        self.origin
    }

    /// Sample spacing, m.
    #[must_use]
    pub fn cell(&self) -> f64 {
        self.cell
    }

    #[must_use]
    pub fn dims(&self) -> [u32; 3] {
        self.dims
    }

    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Highest corner, m.
    #[must_use]
    pub fn max(&self) -> DVec3 {
        self.origin
            + DVec3::new(
                f64::from(self.dims[0] - 1),
                f64::from(self.dims[1] - 1),
                f64::from(self.dims[2] - 1),
            ) * self.cell
    }

    /// Largest distance of a grid corner from the collider origin, m.
    #[must_use]
    pub fn radius(&self) -> f64 {
        let (lo, hi) = (self.origin, self.max());
        lo.abs().max(hi.abs()).length()
    }

    fn at(&self, i: u32, j: u32, k: u32) -> f64 {
        let [nx, ny, _] = self.dims;
        f64::from(self.values[(i + nx * (j + ny * k)) as usize])
    }

    /// Signed distance at `p` (collider frame) and its gradient. Inside the
    /// grid, trilinear; outside, the distance at the nearest grid point
    /// plus the distance to it, which never overstates how near the
    /// surface is.
    #[must_use]
    pub fn sample(&self, p: DVec3) -> (f64, DVec3) {
        let hi = self.max();
        let clamped = p.clamp(self.origin, hi);
        let outside = p - clamped;
        let g = (clamped - self.origin) / self.cell;
        let mut base = [0_u32; 3];
        let mut frac = [0.0_f64; 3];
        for a in 0..3 {
            let top = self.dims[a] - 2;
            let f = g[a].floor().clamp(0.0, f64::from(top));
            base[a] = f as u32;
            frac[a] = (g[a] - f).clamp(0.0, 1.0);
        }
        let [i, j, k] = base;
        let [fx, fy, fz] = frac;
        let c = |a: u32, b: u32, d: u32| self.at(i + a, j + b, k + d);
        let (c000, c100, c010, c110) = (c(0, 0, 0), c(1, 0, 0), c(0, 1, 0), c(1, 1, 0));
        let (c001, c101, c011, c111) = (c(0, 0, 1), c(1, 0, 1), c(0, 1, 1), c(1, 1, 1));
        let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let x00 = lerp(c000, c100, fx);
        let x10 = lerp(c010, c110, fx);
        let x01 = lerp(c001, c101, fx);
        let x11 = lerp(c011, c111, fx);
        let y0 = lerp(x00, x10, fy);
        let y1 = lerp(x01, x11, fy);
        let d = lerp(y0, y1, fz);
        let gx = lerp(
            lerp(c100 - c000, c110 - c010, fy),
            lerp(c101 - c001, c111 - c011, fy),
            fz,
        );
        let gy = lerp(x10 - x00, x11 - x01, fz);
        let gz = y1 - y0;
        let mut gradient = DVec3::new(gx, gy, gz) / self.cell;
        let away = outside.length();
        if away > 0.0 {
            gradient = outside / away;
        }
        (d + away, gradient.normalize_or_zero())
    }

    /// The first point of the ray `origin + t dir` (`dir` unit, grid
    /// frame), `t` in [0, `max`], where the distance reaches zero, by
    /// sphere tracing, with the outward normal there.
    #[must_use]
    pub fn ray(&self, origin: DVec3, dir: DVec3, max: f64) -> Option<(f64, DVec3)> {
        let floor = self.cell * 0.05;
        let mut t = 0.0;
        for _ in 0..512 {
            let (d, n) = self.sample(origin + dir * t);
            if d <= floor {
                return Some((t, n));
            }
            t += d.max(floor);
            if t > max {
                return None;
            }
        }
        None
    }

    /// The versioned file: magic, version, dimensions, origin, spacing, the
    /// `f32` samples, all little-endian.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(52 + self.values.len() * 4);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&Self::VERSION.to_le_bytes());
        for d in self.dims {
            out.extend_from_slice(&d.to_le_bytes());
        }
        for x in self.origin.to_array() {
            out.extend_from_slice(&x.to_le_bytes());
        }
        out.extend_from_slice(&self.cell.to_le_bytes());
        for v in &self.values {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// A grid from [`SdfGrid::to_bytes`].
    ///
    /// # Errors
    ///
    /// The bytes are another format or version, short, or out of range.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SdfError> {
        let fail = |why: &str| SdfError::Format(why.to_owned());
        if bytes.len() < 52 || &bytes[..8] != MAGIC {
            return Err(fail("no grid header"));
        }
        let u32_at =
            |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        let f64_at = |o: usize| {
            let mut b = [0_u8; 8];
            b.copy_from_slice(&bytes[o..o + 8]);
            f64::from_le_bytes(b)
        };
        let version = u32_at(8);
        if version != Self::VERSION {
            return Err(fail(&format!(
                "version {version}, expected {}",
                Self::VERSION
            )));
        }
        let dims = [u32_at(12), u32_at(16), u32_at(20)];
        if dims.iter().any(|&d| !(2..=Self::MAX_AXIS).contains(&d)) {
            return Err(SdfError::Resolution);
        }
        let origin = DVec3::new(f64_at(24), f64_at(32), f64_at(40));
        let cell = f64_at(48);
        let count = dims.iter().map(|&d| d as usize).product::<usize>();
        if bytes.len() != 56 + count * 4 {
            return Err(fail("wrong length for its dimensions"));
        }
        let values = bytes[56..]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Self::new(origin, cell, dims, values)
    }

    /// SHA-256 of [`SdfGrid::to_bytes`], hex.
    #[must_use]
    pub fn hash(&self) -> String {
        Sha256::digest(self.to_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Sample the closed mesh (`vertices`, m, and counterclockwise
    /// `triangles`) at spacing `cell` over its bounds grown by `pad`.
    ///
    /// # Errors
    ///
    /// The mesh is empty or invalid, or the grid would be finer than
    /// [`SdfGrid::MAX_AXIS`] along an axis.
    pub fn bake(
        vertices: &[DVec3],
        triangles: &[[u32; 3]],
        cell: f64,
        pad: f64,
    ) -> Result<Self, SdfError> {
        if triangles.is_empty()
            || triangles
                .iter()
                .any(|t| t.iter().any(|&i| i as usize >= vertices.len()))
        {
            return Err(SdfError::Mesh);
        }
        if !cell.is_finite() || cell <= 0.0 || !pad.is_finite() || pad < 0.0 {
            return Err(SdfError::Invalid);
        }
        let (lo, hi) = vertices
            .iter()
            .fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(*v), hi.max(*v))
            });
        let lo = lo - DVec3::splat(pad);
        let hi = hi + DVec3::splat(pad);
        let span = hi - lo;
        let mut dims = [0_u32; 3];
        for a in 0..3 {
            let n = (span[a] / cell).ceil() + 1.0;
            if !(2.0..=f64::from(Self::MAX_AXIS)).contains(&n) {
                return Err(SdfError::Resolution);
            }
            dims[a] = n as u32;
        }
        let tris: Vec<[DVec3; 3]> = triangles
            .iter()
            .map(|t| t.map(|i| vertices[i as usize]))
            .collect();
        let [nx, ny, nz] = dims;
        let points: Vec<DVec3> = (0..nz)
            .flat_map(|k| (0..ny).flat_map(move |j| (0..nx).map(move |i| (i, j, k))))
            .map(|(i, j, k)| lo + DVec3::new(f64::from(i), f64::from(j), f64::from(k)) * cell)
            .collect();
        let values = crate::parallel::map(&points, 256, |&p| {
            let mut nearest = f64::INFINITY;
            let mut winding = 0.0;
            for t in &tris {
                nearest = nearest.min(closest_on_triangle(p, t).distance_squared(p));
                winding += solid_angle(p, t);
            }
            let distance = nearest.sqrt();
            let inside = winding / (4.0 * std::f64::consts::PI) > 0.5;
            (if inside { -distance } else { distance }) as f32
        });
        Self::new(lo, cell, dims, values)
    }
}

/// The closest point of a triangle to `p` (Ericson 5.1.5).
fn closest_on_triangle(p: DVec3, [a, b, c]: &[DVec3; 3]) -> DVec3 {
    let (ab, ac, ap) = (*b - *a, *c - *a, p - *a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return *a;
    }
    let bp = p - *b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return *b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return *a + ab * (d1 / (d1 - d3));
    }
    let cp = p - *c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return *c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return *a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return *b + (*c - *b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = va + vb + vc;
    if denom.abs() <= f64::MIN_POSITIVE {
        return *a;
    }
    *a + ab * (vb / denom) + ac * (vc / denom)
}

/// The signed solid angle a triangle subtends at `p`, positive when its
/// counterclockwise side faces away from `p` (Van Oosterom and Strackee).
fn solid_angle(p: DVec3, [a, b, c]: &[DVec3; 3]) -> f64 {
    let (x, y, z) = (*a - p, *b - p, *c - p);
    let (lx, ly, lz) = (x.length(), y.length(), z.length());
    let numerator = x.dot(y.cross(z));
    let denominator = lx * ly * lz + x.dot(y) * lz + y.dot(z) * lx + z.dot(x) * ly;
    2.0 * numerator.atan2(denominator)
}

static GRIDS: crate::interned::Registry<SdfGrid> = crate::interned::Registry::new();

/// A `Copy` handle to a grid's shared data, what a
/// [`crate::Shape::Sdf`] holds. It serializes as the grid itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SdfRef {
    index: u32,
    radius: f64,
}

impl SdfRef {
    #[must_use]
    pub fn new(grid: SdfGrid) -> Self {
        let radius = grid.radius();
        Self {
            index: GRIDS.intern(grid),
            radius,
        }
    }

    #[must_use]
    pub fn get(self) -> std::sync::Arc<SdfGrid> {
        GRIDS.get(self.index)
    }

    /// [`SdfGrid::radius`], without a lookup.
    #[must_use]
    pub fn radius(self) -> f64 {
        self.radius
    }
}

impl Serialize for SdfRef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.get().serialize(s)
    }
}

impl<'de> Deserialize<'de> for SdfRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let grid = SdfGrid::deserialize(d)?;
        SdfGrid::new(grid.origin, grid.cell, grid.dims, grid.values)
            .map(Self::new)
            .map_err(serde::de::Error::custom)
    }
}

/// A grid placed in the world.
#[derive(Clone, Debug)]
pub(crate) struct PlacedGrid {
    pub grid: std::sync::Arc<SdfGrid>,
    pub pos: DVec3,
    pub rotation: DQuat,
}

impl PlacedGrid {
    /// Signed distance and outward world normal at world point `x`.
    pub(crate) fn sample(&self, x: DVec3) -> (f64, DVec3) {
        let (d, n) = self.grid.sample(self.rotation.inverse() * (x - self.pos));
        (d, self.rotation * n)
    }

    /// A contact for a point `x` of the other shape swept by `radius`.
    fn point(&self, x: DVec3, radius: f64, margin: f64) -> Option<ContactPoint> {
        let (d, n) = self.sample(x);
        let separation = d - radius;
        if separation > margin || n == DVec3::ZERO {
            return None;
        }
        Some(ContactPoint {
            point: x - n * (radius + separation * 0.5),
            normal: n,
            separation,
        })
    }
}

/// Thin a grid contact set: points are grouped by normal (a shape in a
/// corner touches two walls), each group keeps at most four that span it
/// (`reduce`), and at most three groups are kept, the deepest first.
fn reduce_spread(points: &mut Vec<ContactPoint>) {
    if points.len() <= 4 {
        return;
    }
    let mut groups: Vec<Vec<ContactPoint>> = Vec::new();
    for c in points.drain(..) {
        match groups.iter_mut().find(|g| g[0].normal.dot(c.normal) > 0.95) {
            Some(g) => g.push(c),
            None => groups.push(vec![c]),
        }
    }
    let deepest =
        |g: &Vec<ContactPoint>| g.iter().map(|c| c.separation).fold(f64::INFINITY, f64::min);
    groups.sort_by(|a, b| deepest(a).total_cmp(&deepest(b)));
    for mut g in groups.into_iter().take(3) {
        super::reduce(&mut g);
        points.extend(g);
    }
}

/// Contacts of a grid with a polytope: its corners, edge midpoints, and
/// face centers against the grid. Normals point out of the grid.
pub(crate) fn grid_poly(grid: &PlacedGrid, poly: &Poly, margin: f64) -> Vec<ContactPoint> {
    let mut samples: Vec<DVec3> = poly.vertices.clone();
    for (_, face) in &poly.faces {
        let n = face.len();
        let mut center = DVec3::ZERO;
        for i in 0..n {
            let (a, b) = (face[i], face[(i + 1) % n]);
            let (pa, pb) = (poly.vertices[a as usize], poly.vertices[b as usize]);
            center += pa;
            // Each edge once, from the face that has it as a < b.
            if a < b {
                samples.push((pa + pb) * 0.5);
            }
        }
        samples.push(center / n as f64);
    }
    let mut points: Vec<ContactPoint> = samples
        .into_iter()
        .filter_map(|x| grid.point(x, 0.0, margin))
        .collect();
    reduce_spread(&mut points);
    points
}

/// Contacts of a grid with a sphere (`p == q`) or a capsule's core swept
/// by `radius`, sampled every radius along the core.
pub(crate) fn grid_capsule(
    grid: &PlacedGrid,
    p: DVec3,
    q: DVec3,
    radius: f64,
    margin: f64,
) -> Vec<ContactPoint> {
    let length = p.distance(q);
    let steps = if length > 0.0 {
        ((length / radius.max(1e-3)).ceil() as usize).clamp(1, 16)
    } else {
        0
    };
    let mut points: Vec<ContactPoint> = (0..=steps)
        .filter_map(|s| {
            let t = if steps == 0 {
                0.0
            } else {
                s as f64 / steps as f64
            };
            grid.point(p.lerp(q, t), radius, margin)
        })
        .collect();
    reduce_spread(&mut points);
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A closed box mesh, counterclockwise from outside.
    pub(crate) fn box_mesh(half: DVec3) -> (Vec<DVec3>, Vec<[u32; 3]>) {
        let v: Vec<DVec3> = (0..8)
            .map(|i| {
                DVec3::new(
                    if i & 1 == 0 { -half.x } else { half.x },
                    if i & 2 == 0 { -half.y } else { half.y },
                    if i & 4 == 0 { -half.z } else { half.z },
                )
            })
            .collect();
        let quads = [
            [0, 4, 6, 2],
            [1, 3, 7, 5],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 2, 3, 1],
            [4, 5, 7, 6],
        ];
        let t = quads
            .iter()
            .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
            .collect();
        (v, t)
    }

    #[test]
    fn a_baked_box_reads_back_its_distances() {
        let (v, t) = box_mesh(DVec3::new(0.5, 0.3, 0.4));
        let grid = SdfGrid::bake(&v, &t, 0.05, 0.2).unwrap();
        for (p, expect) in [
            (DVec3::ZERO, -0.3),
            (DVec3::new(0.0, 0.45, 0.0), 0.15),
            (DVec3::new(0.6, 0.0, 0.0), 0.1),
            (DVec3::new(0.0, 0.0, -0.35), -0.05),
        ] {
            let (d, _) = grid.sample(p);
            assert!((d - expect).abs() < 0.01, "{p}: {d} vs {expect}");
        }
        let (_, n) = grid.sample(DVec3::new(0.0, 0.42, 0.01));
        assert!((n - DVec3::Y).length() < 0.05, "{n}");
        // Far outside the grid the distance grows with the gap.
        let (d, n) = grid.sample(DVec3::new(5.0, 0.0, 0.0));
        assert!(d > 4.0 && (n - DVec3::X).length() < 1e-9);
    }

    #[test]
    fn grid_files_round_trip_and_refuse_other_versions() {
        let (v, t) = box_mesh(DVec3::splat(0.25));
        let grid = SdfGrid::bake(&v, &t, 0.1, 0.1).unwrap();
        let bytes = grid.to_bytes();
        assert_eq!(SdfGrid::from_bytes(&bytes).unwrap(), grid);
        let mut other = bytes.clone();
        other[8] = 2;
        assert!(matches!(
            SdfGrid::from_bytes(&other),
            Err(SdfError::Format(_))
        ));
        assert!(SdfGrid::from_bytes(&bytes[..40]).is_err());
        // Baking again gives the same file.
        assert_eq!(SdfGrid::bake(&v, &t, 0.1, 0.1).unwrap().hash(), grid.hash());
    }

    #[test]
    fn grids_finer_than_the_cap_are_refused() {
        let (v, t) = box_mesh(DVec3::splat(1.0));
        assert_eq!(SdfGrid::bake(&v, &t, 0.001, 0.0), Err(SdfError::Resolution));
        assert_eq!(SdfGrid::bake(&v, &[], 0.1, 0.0), Err(SdfError::Mesh));
    }
}
