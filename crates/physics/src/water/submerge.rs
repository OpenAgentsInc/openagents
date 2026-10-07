//! How much of a collider is under water: volume, center of buoyancy, and
//! wetted area.
//!
//! The water near a collider is replaced by its local plane. A sphere's
//! part below a plane is an exact spherical cap. A capsule is integrated
//! slice by slice along its axis, each slice a disc cut by the plane into a
//! circular segment; the hemispherical ends use Archimedes' hat-box
//! theorem for their wetted area. A cuboid splits into six tetrahedra
//! along its main diagonal and each is clipped by the plane exactly; its
//! faces are clipped for the wetted area. On rough water the cuboid's plane
//! is fitted to the surface under its four lowest corners, the box form of
//! Kerner's per-triangle clipping ("Water Interaction Model for Boats in
//! Video Games", *Game Developer*, 2015).

use std::f64::consts::PI;

use glam::{DQuat, DVec2, DVec3};

use super::body::Sample;
use super::set::Water;
use crate::collision::Shape;

/// A plane; the water is on the side its normal points away from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub point: DVec3,
    /// Unit, pointing out of the water.
    pub normal: DVec3,
}

impl Plane {
    /// Height of `p` above the plane along its normal, m.
    #[must_use]
    pub fn distance(&self, p: DVec3) -> f64 {
        (p - self.point).dot(self.normal)
    }
}

/// The part of a collider under water.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Submersion {
    /// m³.
    pub volume: f64,
    /// Center of buoyancy, world frame, m.
    pub centroid: DVec3,
    /// Wetted surface area, m².
    pub wetted_area: f64,
    /// The whole collider's volume, m³, and surface area, m².
    pub total_volume: f64,
    pub total_area: f64,
    /// The water at the collider, if any.
    pub water: Option<Sample>,
}

impl Submersion {
    /// The fraction of the surface that is wet.
    #[must_use]
    pub fn wetted_fraction(&self) -> f64 {
        if self.total_area > 0.0 {
            (self.wetted_area / self.total_area).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// The fraction of the volume under water.
    #[must_use]
    pub fn volume_fraction(&self) -> f64 {
        if self.total_volume > 0.0 {
            (self.volume / self.total_volume).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// A shape's volume, m³.
#[must_use]
pub fn volume(shape: &Shape) -> f64 {
    match *shape {
        Shape::Sphere { radius } => 4.0 / 3.0 * PI * radius.powi(3),
        Shape::Capsule {
            radius,
            half_length,
        } => PI * radius * radius * 2.0 * half_length + 4.0 / 3.0 * PI * radius.powi(3),
        Shape::Cuboid { half } => 8.0 * half.x * half.y * half.z,
    }
}

/// A shape's surface area, m².
#[must_use]
pub fn area(shape: &Shape) -> f64 {
    match *shape {
        Shape::Sphere { radius } => 4.0 * PI * radius * radius,
        Shape::Capsule {
            radius,
            half_length,
        } => 2.0 * PI * radius * 2.0 * half_length + 4.0 * PI * radius * radius,
        Shape::Cuboid { half } => 8.0 * (half.x * half.y + half.y * half.z + half.x * half.z),
    }
}

/// The part of `shape` at `pose` below `plane`: volume, centroid, and
/// wetted area.
#[must_use]
pub fn below(shape: &Shape, pose: (DVec3, DQuat), plane: Plane) -> (f64, DVec3, f64) {
    let (center, rotation) = pose;
    match *shape {
        Shape::Sphere { radius } => sphere(center, radius, plane),
        Shape::Capsule {
            radius,
            half_length,
        } => capsule(center, rotation * DVec3::Z, radius, half_length, plane),
        Shape::Cuboid { half } => cuboid(center, rotation, half, plane),
    }
}

/// The exact spherical cap below the plane.
fn sphere(center: DVec3, r: f64, plane: Plane) -> (f64, DVec3, f64) {
    let h = (r - plane.distance(center)).clamp(0.0, 2.0 * r);
    if h <= 0.0 {
        return (0.0, center, 0.0);
    }
    let volume = PI * h * h * (3.0 * r - h) / 3.0;
    // The cap's centroid lies 3 (2r − h)² / (4 (3r − h)) from the center.
    let depth = 3.0 * (2.0 * r - h).powi(2) / (4.0 * (3.0 * r - h));
    (volume, center - plane.normal * depth, 2.0 * PI * r * h)
}

/// Slices along the capsule's axis on each side of a split.
const SLICES: usize = 16;

fn capsule(center: DVec3, axis: DVec3, r: f64, half: f64, plane: Plane) -> (f64, DVec3, f64) {
    let n = plane.normal;
    let along = axis.dot(n);
    let across = n - axis * along;
    let s = across.length();
    let side = if s > 1e-12 { across / s } else { DVec3::ZERO };
    let dc = plane.distance(center);
    let radius = |z: f64| {
        let e = z.abs() - half;
        if e <= 0.0 {
            r
        } else {
            (r * r - e * e).max(0.0).sqrt()
        }
    };
    let mut volume = 0.0;
    let mut moment = DVec3::ZERO;
    let mut wetted = 0.0;
    let mut slice = |z: f64, dz: f64| {
        let rz = radius(z);
        let a = center + axis * z;
        let d0 = dc + along * z;
        let (area, offset, arc) = if s <= 1e-12 || rz <= 0.0 {
            if d0 < 0.0 {
                (PI * rz * rz, 0.0, 1.0)
            } else {
                (0.0, 0.0, 0.0)
            }
        } else {
            let delta = d0 / s;
            if delta >= rz {
                (0.0, 0.0, 0.0)
            } else if delta <= -rz {
                (PI * rz * rz, 0.0, 1.0)
            } else {
                let c = (rz * rz - delta * delta).sqrt();
                let theta = (delta / rz).acos();
                let area = rz * rz * theta - delta * c;
                let offset = if area > 0.0 {
                    -2.0 / 3.0 * c * c * c / area
                } else {
                    0.0
                };
                (area, offset, theta / PI)
            }
        };
        volume += area * dz;
        moment += (a + side * offset) * (area * dz);
        wetted += 2.0 * PI * r * arc * dz;
    };
    // Integrate each part, split where the axis crosses the plane so the
    // step there falls between slices.
    let cross = if along.abs() > 1e-12 {
        Some(-dc / along)
    } else {
        None
    };
    for (lo, hi) in [(-half - r, -half), (-half, half), (half, half + r)] {
        if hi <= lo {
            continue;
        }
        let parts = match cross {
            Some(z) if z > lo && z < hi => [(lo, z), (z, hi)],
            _ => [(lo, hi), (hi, hi)],
        };
        for (a, b) in parts {
            if b <= a {
                continue;
            }
            let dz = (b - a) / SLICES as f64;
            for k in 0..SLICES {
                slice(a + (k as f64 + 0.5) * dz, dz);
            }
        }
    }
    let centroid = if volume > 0.0 {
        moment / volume
    } else {
        center
    };
    (volume, centroid, wetted)
}

fn cuboid(center: DVec3, rotation: DQuat, half: DVec3, plane: Plane) -> (f64, DVec3, f64) {
    let corners = corners(center, rotation, half);
    let d: [f64; 8] = corners.map(|p| plane.distance(p));
    if d.iter().all(|&x| x >= 0.0) {
        return (0.0, center, 0.0);
    }
    if d.iter().all(|&x| x < 0.0) {
        return (
            8.0 * half.x * half.y * half.z,
            center,
            area(&Shape::Cuboid { half }),
        );
    }
    // Six tetrahedra about the diagonal from corner 0 to corner 7.
    const TETS: [[usize; 4]; 6] = [
        [0, 1, 3, 7],
        [0, 1, 5, 7],
        [0, 2, 3, 7],
        [0, 2, 6, 7],
        [0, 4, 5, 7],
        [0, 4, 6, 7],
    ];
    let mut volume = 0.0;
    let mut moment = DVec3::ZERO;
    for tet in TETS {
        let (v, m) = tet_below(tet.map(|i| corners[i]), tet.map(|i| d[i]));
        volume += v;
        moment += m;
    }
    // Faces as corner loops: −x, +x, −y, +y, −z, +z.
    const FACES: [[usize; 4]; 6] = [
        [0, 2, 6, 4],
        [1, 5, 7, 3],
        [0, 4, 5, 1],
        [2, 3, 7, 6],
        [0, 1, 3, 2],
        [4, 6, 7, 5],
    ];
    let mut wetted = 0.0;
    for face in FACES {
        wetted += clipped_area(face.map(|i| corners[i]), face.map(|i| d[i]));
    }
    let centroid = if volume > 0.0 {
        moment / volume
    } else {
        center
    };
    (volume, centroid, wetted)
}

/// Corner `i` has its x, y, and z signs in bits 0, 1, and 2.
fn corners(center: DVec3, rotation: DQuat, half: DVec3) -> [DVec3; 8] {
    std::array::from_fn(|i| {
        let sign = DVec3::new(
            if i & 1 == 0 { -1.0 } else { 1.0 },
            if i & 2 == 0 { -1.0 } else { 1.0 },
            if i & 4 == 0 { -1.0 } else { 1.0 },
        );
        center + rotation * (half * sign)
    })
}

fn tet(p: [DVec3; 4]) -> (f64, DVec3) {
    let v = (p[1] - p[0]).dot((p[2] - p[0]).cross(p[3] - p[0])).abs() / 6.0;
    (v, (p[0] + p[1] + p[2] + p[3]) * (0.25 * v))
}

/// Volume and first moment of the part of a tetrahedron below the plane,
/// given each vertex's signed distance.
fn tet_below(p: [DVec3; 4], d: [f64; 4]) -> (f64, DVec3) {
    let cut = |i: usize, j: usize| p[i] + (p[j] - p[i]) * (d[i] / (d[i] - d[j]));
    let under: Vec<usize> = (0..4).filter(|&i| d[i] < 0.0).collect();
    let over: Vec<usize> = (0..4).filter(|&i| d[i] >= 0.0).collect();
    match under.len() {
        0 => (0.0, DVec3::ZERO),
        4 => tet(p),
        1 => {
            let i = under[0];
            let (o0, o1, o2) = (over[0], over[1], over[2]);
            tet([p[i], cut(i, o0), cut(i, o1), cut(i, o2)])
        }
        3 => {
            let j = over[0];
            let (whole, wm) = tet(p);
            let (top, tm) = tet([p[j], cut(j, under[0]), cut(j, under[1]), cut(j, under[2])]);
            (whole - top, wm - tm)
        }
        _ => {
            let (i, k) = (under[0], under[1]);
            let (j, l) = (over[0], over[1]);
            let a = [p[i], cut(i, j), cut(i, l)];
            let b = [p[k], cut(k, j), cut(k, l)];
            let (v0, m0) = tet([a[0], a[1], a[2], b[0]]);
            let (v1, m1) = tet([a[1], a[2], b[0], b[1]]);
            let (v2, m2) = tet([a[2], b[0], b[1], b[2]]);
            (v0 + v1 + v2, m0 + m1 + m2)
        }
    }
}

/// Area of the part of a planar quad below the plane (Sutherland–Hodgman).
fn clipped_area(p: [DVec3; 4], d: [f64; 4]) -> f64 {
    let mut poly: Vec<DVec3> = Vec::with_capacity(8);
    for i in 0..4 {
        let j = (i + 1) % 4;
        if d[i] <= 0.0 {
            poly.push(p[i]);
        }
        if (d[i] < 0.0) != (d[j] < 0.0) && d[i] != d[j] {
            poly.push(p[i] + (p[j] - p[i]) * (d[i] / (d[i] - d[j])));
        }
    }
    if poly.len() < 3 {
        return 0.0;
    }
    let mut sum = DVec3::ZERO;
    for i in 1..poly.len() - 1 {
        sum += (poly[i] - poly[0]).cross(poly[i + 1] - poly[0]);
    }
    0.5 * sum.length()
}

/// The surface near `center` as a plane through the sample's point.
fn tangent_plane(sample: &Sample, at: DVec2) -> Plane {
    Plane {
        point: DVec3::new(at.x, sample.height, at.y),
        normal: sample.normal,
    }
}

/// A plane fitted by least squares to surface heights at `points`, or none
/// when they do not span an area.
fn fitted_plane(points: &[(DVec2, f64)]) -> Option<Plane> {
    let n = points.len() as f64;
    let mean = points.iter().fold(DVec2::ZERO, |s, (p, _)| s + *p) / n;
    let height = points.iter().map(|(_, h)| h).sum::<f64>() / n;
    let (mut sxx, mut sxz, mut szz, mut sxh, mut szh) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (p, h) in points {
        let q = *p - mean;
        let e = h - height;
        sxx += q.x * q.x;
        sxz += q.x * q.y;
        szz += q.y * q.y;
        sxh += q.x * e;
        szh += q.y * e;
    }
    let det = sxx * szz - sxz * sxz;
    let scale = (sxx + szz).powi(2);
    if scale <= 0.0 || det <= scale * 1e-6 {
        return None;
    }
    let gx = (sxh * szz - szh * sxz) / det;
    let gz = (szh * sxx - sxh * sxz) / det;
    Some(Plane {
        point: DVec3::new(mean.x, height, mean.y),
        normal: DVec3::new(-gx, 1.0, -gz).normalize(),
    })
}

/// The part of `shape` at `pose` under `water` at `tick`. The water is the
/// plane tangent to its surface over the collider's center; on moving
/// water a capsule tilts that plane to the heights under its ends and a
/// cuboid fits it to the heights under its four lowest corners.
#[must_use]
pub fn submerged<W: Water + ?Sized>(
    shape: &Shape,
    pose: (DVec3, DQuat),
    water: &W,
    tick: u64,
) -> Submersion {
    let (center, rotation) = pose;
    let mut out = Submersion {
        volume: 0.0,
        centroid: center,
        wetted_area: 0.0,
        total_volume: volume(shape),
        total_area: area(shape),
        water: None,
    };
    let xz = DVec2::new(center.x, center.z);
    let Some(sample) = water.sample(xz.x, xz.y, tick) else {
        return out;
    };
    out.water = Some(sample);
    let bound = shape.bound();
    // Far above or far below the surface: nothing to clip.
    if center.y - bound > sample.height + bound {
        return out;
    }
    if center.y + bound < sample.height - bound {
        out.volume = out.total_volume;
        out.wetted_area = out.total_area;
        return out;
    }
    let flat = sample.normal == DVec3::Y && sample.surface_velocity == DVec3::ZERO;
    let mut plane = tangent_plane(&sample, xz);
    match *shape {
        Shape::Sphere { .. } => {}
        Shape::Capsule { half_length, .. } if !flat => {
            let axis = rotation * DVec3::Z * half_length;
            let w = DVec2::new(axis.x, axis.z);
            let reach = w.length();
            if reach > 1e-9 {
                let ends = [xz - w, xz + w];
                if let (Some(a), Some(b)) = (
                    water.sample(ends[0].x, ends[0].y, tick),
                    water.sample(ends[1].x, ends[1].y, tick),
                ) {
                    let dir = w / reach;
                    let slope = (b.height - a.height) / (2.0 * reach);
                    let g = DVec2::new(-sample.normal.x, -sample.normal.z) / sample.normal.y;
                    let perp = DVec2::new(-dir.y, dir.x);
                    let grad = dir * slope + perp * g.dot(perp);
                    plane.normal = DVec3::new(-grad.x, 1.0, -grad.y).normalize();
                }
            }
        }
        Shape::Cuboid { half } if !flat => {
            let mut c = corners(center, rotation, half);
            c.sort_by(|a, b| a.y.total_cmp(&b.y));
            let heights: Vec<(DVec2, f64)> = c[..4]
                .iter()
                .filter_map(|p| {
                    water
                        .sample(p.x, p.z, tick)
                        .map(|s| (DVec2::new(p.x, p.z), s.height))
                })
                .collect();
            if heights.len() >= 3
                && let Some(fit) = fitted_plane(&heights)
            {
                plane = fit;
            }
        }
        _ => {}
    }
    let (v, centroid, wetted) = below(shape, pose, plane);
    out.volume = v;
    out.centroid = centroid;
    out.wetted_area = wetted;
    out
}
