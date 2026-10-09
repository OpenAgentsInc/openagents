//! CPU light baking: a bounding-volume hierarchy over lit triangles, ambient
//! occlusion per vertex, sun visibility, and bounce-light probes.
//!
//! In vacuum at L1 the only light reaching a shadowed surface is sunlight
//! bounced off the station itself; Earthshine is about 5 × 10⁻⁶ of the Sun.
//! The probe bake gathers one diffuse bounce of direct sunlight into
//! order-one spherical harmonics (Ramamoorthi and Hanrahan 2001). Rays follow
//! Möller and Trumbore (1997); the hierarchy splits at the median centroid.

use glam::Vec3;

use super::{LitVertex, ProbeGrid};

/// A triangle with the data the bake needs at a hit.
#[derive(Clone, Copy, Debug)]
struct Triangle {
    a: Vec3,
    ab: Vec3,
    ac: Vec3,
    normal: Vec3,
    albedo: Vec3,
    /// Fraction of light the triangle stops: 1 for solid surfaces, less for
    /// alpha-tested cards and glass, which the textured bake treats as
    /// partial occluders.
    opacity: f32,
    /// The triangle's place in the list the hierarchy was built from, which
    /// building reorders.
    id: u32,
}

/// A triangle for [`Bvh::from_occluders`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Occluder {
    /// World-space corners, meters.
    pub corners: [Vec3; 3],
    /// The authored normal: it picks which side of the face is front, since
    /// winding is not a reliable side.
    pub normal: Vec3,
    /// Diffuse albedo the bake bounces light with.
    pub albedo: Vec3,
    /// Fraction of light the triangle stops, 0 to 1.
    pub opacity: f32,
}

/// Opacity at or above which an occluder counts as solid.
pub const SOLID: f32 = 0.999;

/// What one ray meets within its range, with the nearest triangle named by
/// its place in the list the hierarchy was built from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IndexedTrace {
    /// Fraction of light that passes every occluder along the ray.
    pub transmittance: f32,
    /// The distance to the nearest triangle crossed, and that triangle.
    pub nearest: Option<(f32, u32)>,
}

/// The most partial occluders one ray tells apart. Past this many, a
/// crossing counts without checking whether it repeats an earlier one.
const LAYERS: usize = 64;

/// The partial occluders a ray crosses, each surface once. A ray through
/// the edge two triangles share meets both at the same distance, and a ray
/// through a vertex meets every triangle around it, but light crosses the
/// surface only once. Crossings within [`Layers::gap`] of a counted one
/// are taken as the same surface.
#[derive(Clone, Copy, Debug)]
struct Layers {
    distances: [f32; LAYERS],
    len: usize,
    /// The fraction of light the counted crossings pass.
    passed: f32,
}

impl Default for Layers {
    fn default() -> Self {
        Self {
            distances: [0.0; LAYERS],
            len: 0,
            passed: 1.0,
        }
    }
}

impl Layers {
    /// How far apart two crossings must lie to count as two surfaces: a
    /// millionth of the distance, and at least 0.1 µm. It is a few rounding
    /// steps of the distance, so it joins the triangles around an edge or a
    /// vertex but never a separate surface just behind one.
    fn gap(t: f32) -> f32 {
        (t * 1e-6).max(1e-7)
    }

    /// Counts a crossing at distance `t` of a surface of `opacity`, unless
    /// it repeats one already counted.
    fn cross(&mut self, t: f32, opacity: f32) {
        let seen = &self.distances[..self.len];
        if seen.iter().any(|&d| (d - t).abs() < Self::gap(d.max(t))) {
            return;
        }
        if self.len < LAYERS {
            self.distances[self.len] = t;
            self.len += 1;
        }
        self.passed *= 1.0 - opacity.min(1.0);
    }
}

/// Everything one ray meets within its range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    /// Fraction of light that passes every occluder along the ray.
    pub transmittance: f32,
    /// The nearest triangle crossed, with its opacity.
    pub nearest: Option<(Hit, f32)>,
}

fn oriented(
    id: u32,
    corners: [Vec3; 3],
    authored: Vec3,
    albedo: Vec3,
    opacity: f32,
) -> Option<Triangle> {
    let a = corners[0];
    let ab = corners[1] - a;
    let ac = corners[2] - a;
    let face = ab.cross(ac).try_normalize()?;
    let authored = authored.try_normalize().unwrap_or(face);
    let normal = if face.dot(authored) < 0.0 {
        -face
    } else {
        face
    };
    Some(Triangle {
        a,
        ab,
        ac,
        normal,
        albedo,
        opacity: opacity.clamp(0.0, 1.0),
        id,
    })
}

#[derive(Clone, Copy, Debug)]
struct Node {
    min: Vec3,
    max: Vec3,
    /// Leaf: first triangle index; interior: right child index.
    index: u32,
    /// Leaf triangle count; zero for interior nodes.
    count: u32,
}

/// A ray-cast acceleration structure over a triangle list.
#[derive(Debug, Default)]
pub struct Bvh {
    triangles: Vec<Triangle>,
    nodes: Vec<Node>,
}

/// The nearest hit along a ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub distance: f32,
    pub normal: Vec3,
    pub albedo: Vec3,
}

/// The diffuse albedo the bake uses for a lit vertex.
#[must_use]
pub fn bounce_albedo(v: &LitVertex) -> Vec3 {
    let base = Vec3::from(v.color);
    let metallic = v.params[0];
    match v.params[2].round() as i32 {
        // Crinkled aluminized Kapton scatters over a wide, amber cone.
        2 => Vec3::new(0.45, 0.33, 0.12),
        6 => Vec3::splat(0.3),
        _ if metallic > 0.5 => base * 0.4,
        _ => base,
    }
}

impl Bvh {
    /// Builds a hierarchy over a triangle list of lit vertices.
    #[must_use]
    pub fn new(vertices: &[LitVertex]) -> Self {
        let triangles = vertices
            .chunks_exact(3)
            .enumerate()
            .filter_map(|(id, t)| {
                // Winding is not a reliable side; the authored normal is.
                let authored =
                    Vec3::from(t[0].normal) + Vec3::from(t[1].normal) + Vec3::from(t[2].normal);
                oriented(
                    id as u32,
                    [t[0].pos, t[1].pos, t[2].pos].map(Vec3::from),
                    authored,
                    bounce_albedo(&t[0]),
                    1.0,
                )
            })
            .collect();
        Self::from_triangles(triangles)
    }

    /// Builds a hierarchy over textured or other triangles that may be
    /// partial occluders. [`Self::trace_indexed`] names a triangle by its
    /// place in `occluders`.
    #[must_use]
    pub fn from_occluders(occluders: impl IntoIterator<Item = Occluder>) -> Self {
        let triangles = occluders
            .into_iter()
            .enumerate()
            .filter(|(_, o)| o.corners.iter().all(|c| c.is_finite()))
            .filter_map(|(id, o)| oriented(id as u32, o.corners, o.normal, o.albedo, o.opacity))
            .collect();
        Self::from_triangles(triangles)
    }

    fn from_triangles(mut triangles: Vec<Triangle>) -> Self {
        let mut nodes = Vec::with_capacity(triangles.len() / 2 + 1);
        if !triangles.is_empty() {
            let len = triangles.len();
            build(&mut triangles, &mut nodes, 0, len);
        }
        Self { triangles, nodes }
    }

    /// Number of triangles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.triangles.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// The nearest hit within `max` meters, both faces counted.
    #[must_use]
    pub fn hit(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<Hit> {
        let mut best: Option<(f32, usize)> = None;
        self.walk(origin, dir, max, |t, i| {
            if best.is_none_or(|(d, _)| t < d) {
                best = Some((t, i));
            }
            Some(false)
        });
        best.map(|(distance, i)| Hit {
            distance,
            normal: self.triangles[i].normal,
            albedo: self.triangles[i].albedo,
        })
    }

    /// Whether a front face lies within `max` meters along the ray. Back faces
    /// are skipped, so a ray that starts inside a crossing member escapes it.
    #[must_use]
    pub fn occluded_front(&self, origin: Vec3, dir: Vec3, max: f32) -> bool {
        let mut any = false;
        self.walk(origin, dir, max, |_, i| {
            (self.triangles[i].normal.dot(dir) < 0.0).then(|| {
                any = true;
                true
            })
        });
        any
    }

    /// The light that passes within `max` meters along the ray, and the
    /// nearest triangle it crosses. Partial occluders multiply the
    /// transmittance by one minus their opacity, once for each surface
    /// even where the ray meets it on an edge two triangles share; a solid
    /// one stops it. Of two faces at the same distance, the nearest is the
    /// one that faces the ray.
    #[must_use]
    pub fn trace(&self, origin: Vec3, dir: Vec3, max: f32) -> Trace {
        self.trace_masked(origin, dir, max, None)
    }

    /// [`Self::trace`] through the triangles `skip` leaves standing: a
    /// triangle whose place in the list the hierarchy was built from is
    /// `true` in `skip` is passed through, as if gone, such as a wall a
    /// meteor broke.
    #[must_use]
    pub fn trace_masked(&self, origin: Vec3, dir: Vec3, max: f32, skip: Option<&[bool]>) -> Trace {
        let (transmittance, nearest) = self.crossings(origin, dir, max, skip);
        Trace {
            transmittance,
            nearest: nearest.map(|(distance, i)| {
                let tri = &self.triangles[i];
                (
                    Hit {
                        distance,
                        normal: tri.normal,
                        albedo: tri.albedo,
                    },
                    tri.opacity,
                )
            }),
        }
    }

    /// [`Self::trace`], naming the nearest triangle by its place in the list
    /// the hierarchy was built from rather than by its surface.
    #[must_use]
    pub fn trace_indexed(&self, origin: Vec3, dir: Vec3, max: f32) -> IndexedTrace {
        let (transmittance, nearest) = self.crossings(origin, dir, max, None);
        IndexedTrace {
            transmittance,
            nearest: nearest.map(|(distance, i)| (distance, self.triangles[i].id)),
        }
    }

    /// The transmittance along a ray and the nearest crossing, as an
    /// internal triangle index.
    fn crossings(
        &self,
        origin: Vec3,
        dir: Vec3,
        max: f32,
        skip: Option<&[bool]>,
    ) -> (f32, Option<(f32, usize)>) {
        let mut layers = Layers::default();
        let mut solid = false;
        let mut nearest: Option<(f32, usize)> = None;
        let mut facing: Option<(f32, usize)> = None;
        self.walk(origin, dir, max, |t, i| {
            if self.skipped(i, skip) {
                return None;
            }
            if nearest.is_none_or(|(d, _)| t < d) {
                nearest = Some((t, i));
            }
            if self.triangles[i].normal.dot(dir) < 0.0 && facing.is_none_or(|(d, _)| t < d) {
                facing = Some((t, i));
            }
            let opacity = self.triangles[i].opacity;
            if opacity >= SOLID {
                solid = true;
                // Nothing beyond a solid face matters, but a nearer one may.
                Some(false)
            } else {
                layers.cross(t, opacity);
                None
            }
        });
        // Where two faces coincide, such as the top and bottom of a slab of
        // no thickness, the ray meets the one that faces it.
        let nearest = match (nearest, facing) {
            (Some((t, i)), Some(front))
                if self.triangles[i].normal.dot(dir) >= 0.0 && front.0 <= t + Layers::gap(t) =>
            {
                Some(front)
            }
            (nearest, _) => nearest,
        };
        (if solid { 0.0 } else { layers.passed }, nearest)
    }

    /// The fraction of light that passes within `max` meters along the ray,
    /// stopping early once almost nothing passes.
    #[must_use]
    pub fn transmittance(&self, origin: Vec3, dir: Vec3, max: f32) -> f32 {
        self.transmittance_masked(origin, dir, max, None)
    }

    /// [`Self::transmittance`] through the triangles `skip` leaves
    /// standing ([`Self::trace_masked`]).
    #[must_use]
    pub fn transmittance_masked(
        &self,
        origin: Vec3,
        dir: Vec3,
        max: f32,
        skip: Option<&[bool]>,
    ) -> f32 {
        let mut layers = Layers::default();
        let mut dark = false;
        self.walk(origin, dir, max, |t, i| {
            if self.skipped(i, skip) {
                return None;
            }
            let opacity = self.triangles[i].opacity;
            if opacity < SOLID {
                layers.cross(t, opacity);
            }
            dark = opacity >= SOLID || layers.passed < 1e-3;
            dark.then_some(true)
        });
        if dark { 0.0 } else { layers.passed }
    }

    /// Whether the internal triangle `i` is one `skip` passes through.
    fn skipped(&self, i: usize, skip: Option<&[bool]>) -> bool {
        skip.is_some_and(|skip| {
            skip.get(self.triangles[i].id as usize)
                .copied()
                .unwrap_or(false)
        })
    }

    /// Whether anything lies within `max` meters along the ray.
    #[must_use]
    pub fn occluded(&self, origin: Vec3, dir: Vec3, max: f32) -> bool {
        let mut any = false;
        self.walk(origin, dir, max, |_, _| {
            any = true;
            Some(true)
        });
        any
    }

    /// Visits hits nearer than the current limit. The callback returns `None`
    /// to ignore a hit, `Some(false)` to accept it and shrink the limit, or
    /// `Some(true)` to stop.
    fn walk(
        &self,
        origin: Vec3,
        dir: Vec3,
        max: f32,
        mut found: impl FnMut(f32, usize) -> Option<bool>,
    ) {
        if self.nodes.is_empty() {
            return;
        }
        let inv = dir.recip();
        let mut limit = max;
        let mut stack = [0u32; 64];
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let at = stack[top];
            let node = self.nodes[at as usize];
            if !slab(node.min, node.max, origin, inv, limit) {
                continue;
            }
            if node.count > 0 {
                for i in node.index..node.index + node.count {
                    let tri = &self.triangles[i as usize];
                    if let Some(t) = intersect(tri, origin, dir)
                        && t < limit
                    {
                        match found(t, i as usize) {
                            Some(true) => return,
                            // Faces that coincide with this one still count.
                            Some(false) => limit = t + Layers::gap(t),
                            None => {}
                        }
                    }
                }
            } else if top + 2 <= stack.len() {
                // The left child follows its parent; the parent stores the right.
                stack[top] = node.index;
                stack[top + 1] = at + 1;
                top += 2;
            }
        }
    }
}

fn build(triangles: &mut [Triangle], nodes: &mut Vec<Node>, start: usize, end: usize) -> usize {
    let index = nodes.len();
    let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for t in &triangles[start..end] {
        for p in [t.a, t.a + t.ab, t.a + t.ac] {
            min = min.min(p);
            max = max.max(p);
        }
    }
    nodes.push(Node {
        min,
        max,
        index: start as u32,
        count: (end - start) as u32,
    });
    if end - start <= 4 {
        return index;
    }
    let centroid = |t: &Triangle| t.a + (t.ab + t.ac) / 3.0;
    let extent = max - min;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    let mid = start + (end - start) / 2;
    triangles[start..end].select_nth_unstable_by(mid - start, |a, b| {
        centroid(a)[axis].total_cmp(&centroid(b)[axis])
    });
    let left = build(triangles, nodes, start, mid);
    let right = build(triangles, nodes, mid, end);
    debug_assert_eq!(left, index + 1);
    nodes[index].index = right as u32;
    nodes[index].count = 0;
    index
}

fn slab(min: Vec3, max: Vec3, origin: Vec3, inv: Vec3, limit: f32) -> bool {
    let t0 = (min - origin) * inv;
    let t1 = (max - origin) * inv;
    let near = t0.min(t1).max_element().max(0.0);
    let far = t0.max(t1).min_element().min(limit);
    near <= far
}

fn intersect(tri: &Triangle, origin: Vec3, dir: Vec3) -> Option<f32> {
    let p = dir.cross(tri.ac);
    let det = tri.ab.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = det.recip();
    let s = origin - tri.a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(tri.ab);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = tri.ac.dot(q) * inv;
    (t > 1e-4).then_some(t)
}

/// Deterministic unit directions spread evenly over the sphere.
#[must_use]
pub fn sphere_directions(n: usize) -> Vec<Vec3> {
    let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());
    (0..n)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let a = golden * i as f32;
            Vec3::new(r * a.cos(), y, r * a.sin())
        })
        .collect()
}

/// Bakes ambient occlusion into each vertex's `params[3]`: the cosine-weighted
/// fraction of the hemisphere open within `radius` meters.
pub fn bake_occlusion(vertices: &mut [LitVertex], bvh: &Bvh, radius: f32, rays: usize) {
    let directions = sphere_directions(rays * 2);
    let mut cache = std::collections::HashMap::new();
    for v in vertices.iter_mut() {
        let key = (
            v.pos.map(|x| (x * 256.0).round() as i32),
            v.normal.map(|x| (x * 64.0).round() as i32),
        );
        let ao = *cache.entry(key).or_insert_with(|| {
            let n = Vec3::from(v.normal);
            let origin = Vec3::from(v.pos) + n * 0.01;
            let (mut open, mut total) = (0.0, 0.0);
            for &d in &directions {
                let c = d.dot(n);
                if c <= 0.0 {
                    continue;
                }
                total += c;
                if !bvh.occluded_front(origin, d, radius) {
                    open += c;
                }
            }
            if total > 0.0 { open / total } else { 1.0 }
        });
        v.params[3] = ao;
    }
}

/// Fraction of `samples` points across the solar disc visible from `origin`.
#[must_use]
pub fn sun_visibility(bvh: &Bvh, origin: Vec3, sun_dir: Vec3, angular_radius: f32) -> f32 {
    let (u, v) = basis(sun_dir);
    let offsets = [(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)];
    let open = offsets
        .iter()
        .filter(|(a, b)| {
            let d = (sun_dir + (u * *a + v * *b) * angular_radius * 0.7).normalize();
            !bvh.occluded(origin, d, 1.0e4)
        })
        .count();
    open as f32 / offsets.len() as f32
}

fn basis(axis: Vec3) -> (Vec3, Vec3) {
    let a = axis.normalize();
    let helper = if a.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let u = a.cross(helper).normalize();
    (u, a.cross(u))
}

/// Settings for a probe bake.
#[derive(Clone, Copy, Debug)]
pub struct ProbeSettings {
    pub min: Vec3,
    pub max: Vec3,
    pub cell: f32,
    pub rays: usize,
    pub sun_dir: Vec3,
    pub sun_illuminance: f32,
    pub version: u64,
}

/// Bakes one bounce of sunlight off the geometry into an irradiance grid.
#[must_use]
pub fn bake_probes(bvh: &Bvh, s: &ProbeSettings) -> ProbeGrid {
    let size = (s.max - s.min) / s.cell;
    let dims = [
        size.x.ceil() as u32 + 1,
        size.y.ceil() as u32 + 1,
        size.z.ceil() as u32 + 1,
    ]
    .map(|d| d.clamp(2, 64));
    let directions = sphere_directions(s.rays);
    let weight = 4.0 * std::f32::consts::PI / directions.len() as f32;
    let (y0, y1) = (0.282_095_f32, 0.488_603_f32);
    let (a0, a1) = (std::f32::consts::PI, 2.0 * std::f32::consts::PI / 3.0);
    let mut data = Vec::with_capacity((dims[0] * dims[1] * dims[2]) as usize);
    let mut valid = Vec::with_capacity(data.capacity());
    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                let p = s.min + Vec3::new(x as f32, y as f32, z as f32) * s.cell;
                let mut l0 = Vec3::ZERO;
                let mut l1 = [Vec3::ZERO; 3];
                let mut backfaces = 0;
                for &d in &directions {
                    let Some(hit) = bvh.hit(p, d, 80.0) else {
                        continue;
                    };
                    // Thin panels are lit on either side; face the probe.
                    let mut n = hit.normal;
                    if n.dot(d) > 0.0 {
                        n = -n;
                        backfaces += 1;
                    }
                    let point = p + d * hit.distance + n * 0.02;
                    let cos = n.dot(s.sun_dir);
                    if cos <= 0.0 || bvh.occluded(point, s.sun_dir, 1.0e4) {
                        continue;
                    }
                    let radiance = hit.albedo * (s.sun_illuminance * cos / std::f32::consts::PI);
                    l0 += radiance * (y0 * weight);
                    for (k, axis) in l1.iter_mut().enumerate() {
                        *axis += radiance * (y1 * d[k] * weight);
                    }
                }
                let mut probe = [0.0; 12];
                for c in 0..3 {
                    probe[c * 4] = a0 * y0 * l0[c];
                    for k in 0..3 {
                        probe[c * 4 + 1 + k] = a1 * y1 * l1[k][c];
                    }
                }
                valid.push(backfaces * 3 < directions.len());
                data.push(probe);
            }
        }
    }
    dilate(&mut data, &valid, dims);
    ProbeGrid {
        origin: s.min,
        cell: s.cell,
        dims,
        data,
        version: s.version,
    }
}

/// Replaces probes buried inside geometry with the mean of valid neighbors,
/// so light does not leak from inside solid parts.
pub fn dilate(data: &mut [[f32; 12]], valid: &[bool], dims: [u32; 3]) {
    let index =
        |x: i64, y: i64, z: i64| (x + y * dims[0] as i64 + z * (dims[0] * dims[1]) as i64) as usize;
    let mut ok = valid.to_vec();
    for _ in 0..3 {
        let snapshot = data.to_vec();
        let known = ok.clone();
        for z in 0..dims[2] as i64 {
            for y in 0..dims[1] as i64 {
                for x in 0..dims[0] as i64 {
                    let i = index(x, y, z);
                    if known[i] {
                        continue;
                    }
                    let mut sum = [0.0; 12];
                    let mut n = 0.0;
                    for (dx, dy, dz) in [
                        (1, 0, 0),
                        (-1, 0, 0),
                        (0, 1, 0),
                        (0, -1, 0),
                        (0, 0, 1),
                        (0, 0, -1),
                    ] {
                        let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                        if nx < 0
                            || ny < 0
                            || nz < 0
                            || nx >= dims[0] as i64
                            || ny >= dims[1] as i64
                            || nz >= dims[2] as i64
                        {
                            continue;
                        }
                        let j = index(nx, ny, nz);
                        if known[j] {
                            for (s, v) in sum.iter_mut().zip(snapshot[j]) {
                                *s += v;
                            }
                            n += 1.0;
                        }
                    }
                    if n > 0.0 {
                        data[i] = sum.map(|s| s / n);
                        ok[i] = true;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(center: Vec3, half: f32, normal: Vec3, color: [f32; 3]) -> Vec<LitVertex> {
        let (u, v) = basis(normal);
        let c = [
            center + (-u - v) * half,
            center + (u - v) * half,
            center + (u + v) * half,
            center + (-u + v) * half,
        ];
        [0, 1, 2, 0, 2, 3]
            .map(|i| LitVertex {
                pos: c[i].to_array(),
                normal: normal.to_array(),
                tangent: u.to_array(),
                local: c[i].to_array(),
                color,
                params: [0.0, 0.8, 0.0, 1.0],
            })
            .to_vec()
    }

    #[test]
    fn a_ray_through_a_shared_edge_crosses_the_surface_once() {
        // Two halves of a glass pane at y = 1 that stops 30 percent; the
        // ray runs down their shared diagonal.
        let half = |corners: [Vec3; 3]| Occluder {
            corners,
            normal: Vec3::Y,
            albedo: Vec3::splat(0.5),
            opacity: 0.3,
        };
        let bvh = Bvh::from_occluders([
            half([
                Vec3::new(-1.0, 1.0, -1.0),
                Vec3::new(1.0, 1.0, -1.0),
                Vec3::new(1.0, 1.0, 1.0),
            ]),
            half([
                Vec3::new(-1.0, 1.0, -1.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(-1.0, 1.0, 1.0),
            ]),
        ]);
        let origin = Vec3::new(0.25, 3.0, 0.25);
        let trace = bvh.trace(origin, -Vec3::Y, 10.0);
        assert!((trace.transmittance - 0.7).abs() < 1e-6, "{trace:?}");
        assert!((bvh.transmittance(origin, -Vec3::Y, 10.0) - 0.7).abs() < 1e-6);
        // Off the edge, still one crossing.
        let off = Vec3::new(0.5, 3.0, -0.25);
        assert!((bvh.transmittance(off, -Vec3::Y, 10.0) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn of_two_coincident_faces_a_ray_meets_the_one_facing_it() {
        // A slab of no thickness: a dark face down and a white face up.
        let face = |normal: Vec3, albedo: f32| Occluder {
            corners: [
                Vec3::new(-1.0, 1.0, -1.0),
                Vec3::new(1.0, 1.0, -1.0),
                Vec3::new(0.0, 1.0, 1.0),
            ],
            normal,
            albedo: Vec3::splat(albedo),
            opacity: 1.0,
        };
        for order in [[0, 1], [1, 0]] {
            let faces = [face(-Vec3::Y, 0.1), face(Vec3::Y, 0.9)];
            let bvh = Bvh::from_occluders(order.map(|k| faces[k]));
            let down = bvh.trace(Vec3::new(0.0, 3.0, 0.0), -Vec3::Y, 10.0);
            assert_eq!(down.nearest.unwrap().0.albedo, Vec3::splat(0.9));
            let up = bvh.trace(Vec3::ZERO, Vec3::Y, 10.0);
            assert_eq!(up.nearest.unwrap().0.albedo, Vec3::splat(0.1));
        }
    }

    #[test]
    fn rays_hit_the_nearest_face_and_miss_open_space() {
        let mut v = quad(Vec3::new(0.0, 0.0, 5.0), 1.0, -Vec3::Z, [0.5; 3]);
        v.extend(quad(Vec3::new(0.0, 0.0, 9.0), 1.0, -Vec3::Z, [0.5; 3]));
        for i in 0..40 {
            v.extend(quad(
                Vec3::new(i as f32 * 3.0 + 10.0, 0.0, 0.0),
                0.5,
                Vec3::Y,
                [0.5; 3],
            ));
        }
        let bvh = Bvh::new(&v);
        let hit = bvh.hit(Vec3::ZERO, Vec3::Z, 100.0).unwrap();
        assert!((hit.distance - 5.0).abs() < 1e-4);
        assert!(bvh.hit(Vec3::ZERO, -Vec3::Z, 100.0).is_none());
        assert!(bvh.occluded(Vec3::ZERO, Vec3::Z, 6.0));
        assert!(!bvh.occluded(Vec3::ZERO, Vec3::Z, 4.0));
        let far = bvh.hit(Vec3::new(40.0, 5.0, 0.0), -Vec3::Y, 100.0).unwrap();
        assert!((far.distance - 5.0).abs() < 1e-4);
    }

    #[test]
    fn a_corner_is_occluded_and_open_ground_is_not() {
        let mut v = quad(Vec3::ZERO, 5.0, Vec3::Y, [0.5; 3]);
        v.extend(quad(Vec3::new(0.0, 5.0, -0.01), 5.0, Vec3::Z, [0.5; 3]));
        let bvh = Bvh::new(&v);
        let mut probe = [
            LitVertex {
                pos: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                ..v[0]
            },
            LitVertex {
                pos: [0.0, 0.0, 4.9],
                normal: [0.0, 1.0, 0.0],
                ..v[0]
            },
        ];
        bake_occlusion(&mut probe, &bvh, 3.0, 64);
        assert!(probe[0].params[3] < 0.8, "{}", probe[0].params[3]);
        assert!(probe[1].params[3] > 0.95, "{}", probe[1].params[3]);
    }

    #[test]
    fn a_sunlit_white_floor_lights_a_probe_from_below() {
        // A white floor lit from above, and a probe 1 m over it.
        let v = quad(Vec3::ZERO, 20.0, Vec3::Y, [0.8; 3]);
        let bvh = Bvh::new(&v);
        let grid = bake_probes(
            &bvh,
            &ProbeSettings {
                min: Vec3::new(-1.0, 1.0, -1.0),
                max: Vec3::new(1.0, 1.0, 1.0),
                cell: 2.0,
                rays: 512,
                sun_dir: Vec3::Y,
                sun_illuminance: 100_000.0,
                version: 1,
            },
        );
        let p = grid.data[0];
        let down = p[0] - p[2];
        let up = p[0] + p[2];
        // A large Lambertian floor of albedo 0.8 under 100 klx returns
        // about 80 klx to a downward-facing sensor, and nothing upward.
        assert!((down - 80_000.0).abs() < 8_000.0, "{down}");
        assert!(up.abs() < 8_000.0, "{up}");
    }
}
