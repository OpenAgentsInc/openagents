//! Bounded static-mesh contacts for the local walking preview.

use std::collections::{HashMap, HashSet};

type V3 = [f32; 3];
pub type Vertices = [V3; 3];
const CELL: f32 = 2.0;
const MAX_TRIANGLES: usize = 2_000_000;
const MAX_REFERENCES: usize = 32_000_000;
const MAX_BROAD: usize = 4096;
const EPS: f32 = 0.001;

fn add(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] - b[i])
}
fn mul(a: V3, s: f32) -> V3 {
    a.map(|v| v * s)
}
fn dot(a: V3, b: V3) -> f32 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn distance2(a: V3, b: V3) -> f32 {
    let d = sub(a, b);
    dot(d, d)
}
fn cell(v: f32) -> i32 {
    (v / CELL).floor() as i32
}

#[derive(Clone)]
struct Triangle {
    vertices: Vertices,
    min: V3,
    max: V3,
    normal: V3,
}
impl Triangle {
    fn new(vertices: Vertices) -> Result<Self, &'static str> {
        if vertices
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 100_000.0)
        {
            return Err("A collision surface has invalid coordinates.");
        }
        let raw = cross(sub(vertices[1], vertices[0]), sub(vertices[2], vertices[0]));
        let length = dot(raw, raw).sqrt();
        if length < 1e-7 {
            return Err("A collision surface has no area.");
        }
        Ok(Self {
            vertices,
            min: std::array::from_fn(|i| {
                vertices.iter().map(|v| v[i]).fold(f32::INFINITY, f32::min)
            }),
            max: std::array::from_fn(|i| {
                vertices
                    .iter()
                    .map(|v| v[i])
                    .fold(f32::NEG_INFINITY, f32::max)
            }),
            normal: mul(raw, 1.0 / length),
        })
    }
    fn floor(&self, x: f32, z: f32) -> Option<f32> {
        if self.normal[1].abs() < 0.65 {
            return None;
        }
        let a = self.vertices[0];
        let y = a[1] - (self.normal[0] * (x - a[0]) + self.normal[2] * (z - a[2])) / self.normal[1];
        let p = [x, y, z];
        let [a, b, c] = self.vertices;
        let ab = sub(b, a);
        let ac = sub(c, a);
        let ap = sub(p, a);
        let aa = dot(ab, ab);
        let bb = dot(ac, ac);
        let cc = dot(ab, ac);
        let den = aa * bb - cc * cc;
        if den <= 1e-12 {
            return None;
        }
        let u = (bb * dot(ap, ab) - cc * dot(ap, ac)) / den;
        let v = (aa * dot(ap, ac) - cc * dot(ap, ab)) / den;
        (u >= -1e-5 && v >= -1e-5 && u + v <= 1.0 + 1e-5).then_some(y)
    }
    /// Exact plane support avoids cancellation in long-edge sweep quadratics.
    fn horizontal_support(&self, x: f32, z: f32, radius: f32) -> Option<f32> {
        let vertices = self.vertices.map(|v| [v[0] as f64, v[2] as f64]);
        let point = [x as f64, z as f64];
        let edge = |a: [f64; 2], b: [f64; 2]| [b[0] - a[0], b[1] - a[1]];
        let cross = |a: [f64; 2], b: [f64; 2]| a[0] * b[1] - a[1] * b[0];
        let [a, b, c] = vertices;
        let ab = edge(a, b);
        let ac = edge(a, c);
        let ap = edge(a, point);
        let denominator = cross(ab, ac);
        let u = cross(ap, ac) / denominator;
        let v = cross(ab, ap) / denominator;
        if u >= 0.0 && v >= 0.0 && u + v <= 1.0 {
            return Some(self.min[1]);
        }
        // Outside the face, a sphere can still rest against an edge or vertex.
        let mut distance2 = f64::INFINITY;
        for index in 0..3 {
            let a = vertices[index];
            let ab = edge(a, vertices[(index + 1) % 3]);
            let ap = edge(a, point);
            let along =
                ((ap[0] * ab[0] + ap[1] * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1])).clamp(0.0, 1.0);
            let offset = [ap[0] - along * ab[0], ap[1] - along * ab[1]];
            distance2 = distance2.min(offset[0] * offset[0] + offset[1] * offset[1]);
        }
        let radius = radius as f64;
        let vertical2 = radius * radius - distance2;
        if vertical2 < 0.0 {
            return None;
        }
        let vertical = vertical2.sqrt();
        (vertical >= 0.65 * radius).then_some((self.min[1] as f64 + vertical - radius) as f32)
    }
}

#[derive(Clone, Default)]
pub struct StaticGeometry {
    triangles: Vec<Triangle>,
    owners: Vec<u32>,
    owned: HashSet<u32>,
    disabled: HashSet<u32>,
    cells: HashMap<(i32, i32), Vec<usize>>,
    broad: Vec<usize>,
    references: usize,
}
impl StaticGeometry {
    pub fn len(&self) -> usize {
        self.triangles.len()
    }
    /// Validates the complete chunk before changing the spatial index.
    #[cfg(test)]
    pub fn append(&mut self, vertices: Vec<Vertices>) -> Result<(), &'static str> {
        self.append_owned(vertices, None)
    }
    pub fn append_owned(
        &mut self,
        vertices: Vec<Vertices>,
        owners: Option<Vec<u32>>,
    ) -> Result<(), &'static str> {
        if owners
            .as_ref()
            .is_some_and(|ids| ids.len() != vertices.len())
        {
            return Err("The collision pieces do not match their owners.");
        }
        if self.triangles.len() + vertices.len() > MAX_TRIANGLES {
            return Err("This scene has too many collision surfaces.");
        }
        let triangles = vertices
            .into_iter()
            .map(Triangle::new)
            .collect::<Result<Vec<_>, _>>()?;
        let mut references = 0usize;
        let mut broad = 0usize;
        for triangle in &triangles {
            let count = (cell(triangle.max[0]) - cell(triangle.min[0]) + 1) as u64
                * (cell(triangle.max[2]) - cell(triangle.min[2]) + 1) as u64;
            if count > 64 {
                broad += 1;
            } else {
                references += count as usize;
            }
        }
        if self.references + references > MAX_REFERENCES || self.broad.len() + broad > MAX_BROAD {
            return Err("The collision surfaces cover too much space.");
        }
        self.references += references;
        let owners = owners.unwrap_or_else(|| vec![u32::MAX; triangles.len()]);
        self.owned
            .extend(owners.iter().copied().filter(|id| *id != u32::MAX));
        self.owners.extend(owners);
        for triangle in triangles {
            let index = self.triangles.len();
            let (x0, x1, z0, z1) = (
                cell(triangle.min[0]),
                cell(triangle.max[0]),
                cell(triangle.min[2]),
                cell(triangle.max[2]),
            );
            if (x1 - x0 + 1) as u64 * (z1 - z0 + 1) as u64 > 64 {
                self.broad.push(index);
            } else {
                for x in x0..=x1 {
                    for z in z0..=z1 {
                        self.cells.entry((x, z)).or_default().push(index);
                    }
                }
            }
            self.triangles.push(triangle);
        }
        Ok(())
    }
    fn candidates_all(&self, min: V3, max: V3, active: bool) -> Vec<usize> {
        let mut found = HashSet::new();
        for &index in &self.broad {
            found.insert(index);
        }
        for x in cell(min[0])..=cell(max[0]) {
            for z in cell(min[2])..=cell(max[2]) {
                if let Some(indices) = self.cells.get(&(x, z)) {
                    found.extend(indices.iter().copied());
                }
            }
        }
        let mut indices: Vec<_> = found
            .into_iter()
            .filter(|&i| {
                let t = &self.triangles[i];
                (!active || !self.disabled.contains(&self.owners[i]))
                    && (0..3).all(|a| t.max[a] >= min[a] && t.min[a] <= max[a])
            })
            .collect();
        // Stable ordering keeps contacts deterministic across native and Wasm runs.
        indices.sort_unstable();
        indices
    }
    fn candidates(&self, min: V3, max: V3) -> Vec<usize> {
        self.candidates_all(min, max, true)
    }
    pub fn set_disabled(&mut self, ids: &std::collections::BTreeSet<u32>) {
        self.disabled.clear();
        self.disabled.extend(ids.iter().copied());
    }
    pub fn disabled_count(&self) -> usize {
        self.disabled.len()
    }
    pub fn has_owner(&self, id: u32) -> bool {
        self.owned.contains(&id)
    }
    pub fn clear_disabled(&mut self) {
        self.disabled.clear();
    }
    pub fn can_occupy(&self, feet: V3, radius: f32, height: f32) -> bool {
        self.can_occupy_inner(feet, radius, height, true)
    }
    pub fn can_occupy_all(&self, feet: V3, radius: f32, height: f32) -> bool {
        self.can_occupy_inner(feet, radius, height, false)
    }
    fn can_occupy_inner(&self, feet: V3, radius: f32, height: f32, active: bool) -> bool {
        if self.triangles.is_empty() {
            return true;
        }
        let a = add(feet, [0., radius, 0.]);
        let b = add(feet, [0., height - radius, 0.]);
        let min = [feet[0] - radius, feet[1], feet[2] - radius];
        let max = [feet[0] + radius, feet[1] + height, feet[2] + radius];
        let threshold = (radius - EPS).powi(2);
        self.candidates_all(min, max, active)
            .into_iter()
            .all(|i| segment_triangle_distance2(a, b, &self.triangles[i]) >= threshold)
    }
    pub fn support_height(&self, feet: V3, max_y: f32, radius: f32) -> Option<f32> {
        if self.triangles.is_empty() {
            return None;
        }
        let mut floor: Option<f32> = None;
        let min = [feet[0] - radius, max_y - 4., feet[2] - radius];
        let max = [feet[0] + radius, max_y + radius, feet[2] + radius];
        for i in self.candidates(min, max) {
            let triangle = &self.triangles[i];
            if triangle.min[1] == triangle.max[1] {
                if let Some(y) = triangle.horizontal_support(feet[0], feet[2], radius)
                    && y <= max_y + EPS
                {
                    floor = Some(floor.map_or(y, |old| old.max(y)));
                }
                continue;
            }
            // The bottom sphere rests above a sloping face, or beside a tread's
            // edge. Surface-height samples do not describe that contact height.
            let start = [
                feet[0],
                (triangle.max[1] + EPS).max(max_y) + radius,
                feet[2],
            ];
            let travel = start[1] - (max_y - 4.0 + radius);
            let along = sphere_triangle_sweep(start, [0.0, -travel, 0.0], radius, triangle);
            if along >= 1.0 {
                continue;
            }
            let center = add(start, [0.0, -travel * along, 0.0]);
            let y = center[1] - radius;
            if y > max_y + EPS {
                continue;
            }
            let offset = sub(center, closest_triangle(center, triangle.vertices));
            let length = dot(offset, offset).sqrt();
            // A wall's side contact cannot become a floor. Edge contacts use
            // their actual normal, even when the adjacent face is steeper.
            if length > 1e-6 && offset[1] / length >= 0.65 {
                floor = Some(floor.map_or(y, |old| old.max(y)));
            }
        }
        floor
    }
    /// A grounded step can reach the next tread before the bottom sphere rests on it.
    pub fn step_height(&self, feet: V3, max_y: f32, radius: f32) -> Option<f32> {
        let mut floor: Option<f32> = None;
        let r = radius * 0.95;
        let offsets = [
            [0., 0.],
            [r, 0.],
            [-r, 0.],
            [0., r],
            [0., -r],
            [r * 0.707, r * 0.707],
            [-r * 0.707, r * 0.707],
            [r * 0.707, -r * 0.707],
            [-r * 0.707, -r * 0.707],
        ];
        for i in self.candidates(
            [feet[0] - radius, max_y - 0.55, feet[2] - radius],
            [feet[0] + radius, max_y + EPS, feet[2] + radius],
        ) {
            if self.triangles[i].normal[1].abs() < 0.98 {
                continue;
            }
            for [x, z] in offsets {
                if let Some(y) = self.triangles[i].floor(feet[0] + x, feet[2] + z) {
                    if y <= max_y + EPS {
                        floor = Some(floor.map_or(y, |old| old.max(y)));
                    }
                }
            }
        }
        floor
    }
    /// Gravity slides the bottom sphere off an edge too steep to stand on.
    pub fn slide_down(&self, mut feet: V3, radius: f32, height: f32) -> Option<V3> {
        let mut moved = false;
        for _ in 0..3 {
            let center = add(feet, [0., radius, 0.]);
            let indices = self.candidates(
                [center[0] - radius, center[1] - radius, center[2] - radius],
                [center[0] + radius, center[1] + radius, center[2] + radius],
            );
            let mut pushed = false;
            for i in indices {
                let center = add(feet, [0., radius, 0.]);
                let offset = sub(center, closest_triangle(center, self.triangles[i].vertices));
                let distance = dot(offset, offset).sqrt();
                if distance <= 1e-6 || distance >= radius {
                    continue;
                }
                let normal = mul(offset, 1. / distance);
                if normal[1] <= 0. || normal[1] >= 0.65 {
                    continue;
                }
                feet = add(feet, mul(normal, radius - distance + EPS));
                pushed = true;
                moved = true;
            }
            if !pushed {
                break;
            }
        }
        (moved && self.can_occupy(feet, radius, height)).then_some(feet)
    }
    /// Sweeps a sphere against faces, edge capsules, and vertices.
    pub fn sweep(&self, from: V3, to: V3, radius: f32) -> f32 {
        self.sweep_contact(from, to, radius)
            .map_or(1., |(fraction, _)| fraction)
    }
    pub fn sweep_contact(&self, from: V3, to: V3, radius: f32) -> Option<(f32, V3)> {
        let delta = sub(to, from);
        let length = dot(delta, delta).sqrt();
        if length < 1e-6 || self.triangles.is_empty() {
            return None;
        }
        let min = std::array::from_fn(|a| from[a].min(to[a]) - radius);
        let max = std::array::from_fn(|a| from[a].max(to[a]) + radius);
        let mut first = 1.0f32;
        let mut normal = [0., 1., 0.];
        for index in self.candidates(min, max) {
            let fraction = sphere_triangle_sweep(from, delta, radius + EPS, &self.triangles[index]);
            if fraction < first {
                first = fraction;
                let t = &self.triangles[index];
                let center = add(from, mul(delta, fraction));
                let offset = sub(center, closest_triangle(center, t.vertices));
                let distance = dot(offset, offset).sqrt();
                normal = if distance > 1e-6 {
                    mul(offset, 1. / distance)
                } else if dot(t.normal, delta) > 0. {
                    mul(t.normal, -1.)
                } else {
                    t.normal
                };
            }
        }
        (first < 1.).then_some((first.clamp(0., 1.), normal))
    }
    pub fn raycast(&self, from: V3, direction: V3, max_distance: f32) -> Option<RayHit> {
        self.raycast_inner(from, direction, max_distance, false)
    }
    /// Finds permanent scenery without using breakable roofs as a landing floor.
    pub fn raycast_permanent(&self, from: V3, direction: V3, max_distance: f32) -> Option<RayHit> {
        self.raycast_inner(from, direction, max_distance, true)
    }
    fn raycast_inner(
        &self,
        from: V3,
        direction: V3,
        max_distance: f32,
        permanent: bool,
    ) -> Option<RayHit> {
        let end = add(from, mul(direction, max_distance));
        let min = std::array::from_fn(|i| from[i].min(end[i]) - EPS);
        let max = std::array::from_fn(|i| from[i].max(end[i]) + EPS);
        let mut best = max_distance;
        let mut result = None;
        for i in self.candidates(min, max) {
            if permanent && self.owners[i] != u32::MAX {
                continue;
            }
            let triangle = &self.triangles[i];
            let [a, b, c] = triangle.vertices;
            let edge1 = sub(b, a);
            let edge2 = sub(c, a);
            let p = cross(direction, edge2);
            let determinant = dot(edge1, p);
            if determinant.abs() < 1e-8 {
                continue;
            }
            let inverse = 1. / determinant;
            let t = sub(from, a);
            let u = dot(t, p) * inverse;
            if !(-EPS..=1. + EPS).contains(&u) {
                continue;
            }
            let q = cross(t, edge1);
            let v = dot(direction, q) * inverse;
            if v < -EPS || u + v > 1. + EPS {
                continue;
            }
            let distance = dot(edge2, q) * inverse;
            if distance >= 0. && distance <= best {
                best = distance;
                let normal = if dot(direction, triangle.normal) > 0. {
                    mul(triangle.normal, -1.)
                } else {
                    triangle.normal
                };
                result = Some(RayHit {
                    point: add(from, mul(direction, distance)),
                    normal,
                    distance,
                    owner: (self.owners[i] != u32::MAX).then_some(self.owners[i]),
                });
            }
        }
        result
    }
}

#[derive(Clone, Debug)]
pub struct RayHit {
    pub point: V3,
    pub normal: V3,
    pub distance: f32,
    pub owner: Option<u32>,
}

fn sphere_triangle_sweep(from: V3, delta: V3, radius: f32, triangle: &Triangle) -> f32 {
    if distance2(from, closest_triangle(from, triangle.vertices)) <= radius * radius {
        return 0.;
    }
    let length = dot(delta, delta).sqrt();
    let ray = mul(delta, 1. / length);
    let mut first = length;
    let distance = dot(sub(from, triangle.vertices[0]), triangle.normal);
    let change = dot(ray, triangle.normal);
    if change.abs() > 1e-8 {
        for side in [-1., 1.] {
            let at = (side * radius - distance) / change;
            if at >= 0. && at < first {
                let center = add(from, mul(ray, at));
                let contact = sub(center, mul(triangle.normal, side * radius));
                if distance2(contact, closest_triangle(contact, triangle.vertices)) < EPS * EPS {
                    first = at;
                }
            }
        }
    }
    for i in 0..3 {
        let a = triangle.vertices[i];
        let b = triangle.vertices[(i + 1) % 3];
        let edge = sub(b, a);
        let offset = sub(from, a);
        let ee = dot(edge, edge);
        let er = dot(edge, ray);
        let eo = dot(edge, offset);
        let ro = dot(ray, offset);
        let oo = dot(offset, offset);
        let qa = ee - er * er;
        let qb = ee * ro - eo * er;
        let qc = ee * oo - eo * eo - radius * radius * ee;
        let determinant = qb * qb - qa * qc;
        if qa > 1e-10 && determinant >= 0. {
            let at = (-qb - determinant.sqrt()) / qa;
            let along = eo + at * er;
            if at >= 0. && at < first && along >= 0. && along <= ee {
                first = at;
            }
        }
        let determinant = ro * ro - (oo - radius * radius);
        if determinant >= 0. {
            let at = -ro - determinant.sqrt();
            if at >= 0. && at < first {
                first = at;
            }
        }
    }
    first / length
}

fn closest_triangle(p: V3, [a, b, c]: Vertices) -> V3 {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0. && d2 <= 0. {
        return a;
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0. && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0. && d1 >= 0. && d3 <= 0. {
        return add(a, mul(ab, d1 / (d1 - d3)));
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0. && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0. && d2 >= 0. && d6 <= 0. {
        return add(a, mul(ac, d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0. && d4 - d3 >= 0. && d5 - d6 >= 0. {
        return add(b, mul(sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))));
    }
    let inverse = 1. / (va + vb + vc);
    add(a, add(mul(ab, vb * inverse), mul(ac, vc * inverse)))
}
fn segment_segment_distance2(p1: V3, q1: V3, p2: V3, q2: V3) -> f32 {
    let d1 = sub(q1, p1);
    let d2 = sub(q2, p2);
    let r = sub(p1, p2);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);
    let (mut s, mut t);
    if a <= 1e-12 && e <= 1e-12 {
        return distance2(p1, p2);
    }
    if a <= 1e-12 {
        s = 0.;
        t = (f / e).clamp(0., 1.);
    } else {
        let c = dot(d1, r);
        if e <= 1e-12 {
            t = 0.;
            s = (-c / a).clamp(0., 1.);
        } else {
            let b = dot(d1, d2);
            let den = a * e - b * b;
            s = if den > 1e-12 {
                ((b * f - c * e) / den).clamp(0., 1.)
            } else {
                0.
            };
            t = (b * s + f) / e;
            if t < 0. {
                t = 0.;
                s = (-c / a).clamp(0., 1.);
            } else if t > 1. {
                t = 1.;
                s = ((b - c) / a).clamp(0., 1.);
            }
        }
    }
    distance2(add(p1, mul(d1, s)), add(p2, mul(d2, t)))
}
fn segment_triangle_distance2(a: V3, b: V3, t: &Triangle) -> f32 {
    let delta = sub(b, a);
    let den = dot(t.normal, delta);
    if den.abs() > 1e-9 {
        let along = dot(t.normal, sub(t.vertices[0], a)) / den;
        if (0. ..=1.).contains(&along) {
            let p = add(a, mul(delta, along));
            if distance2(p, closest_triangle(p, t.vertices)) < 1e-10 {
                return 0.;
            }
        }
    }
    let mut best = distance2(a, closest_triangle(a, t.vertices))
        .min(distance2(b, closest_triangle(b, t.vertices)));
    for edge in 0..3 {
        best = best.min(segment_segment_distance2(
            a,
            b,
            t.vertices[edge],
            t.vertices[(edge + 1) % 3],
        ));
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    fn large_floor() -> Vec<Vertices> {
        vec![
            [
                [-1024., 0., -1024.],
                [-1024., 0., 1024.],
                [1024., 0., 1024.],
            ],
            [
                [-1024., 0., -1024.],
                [1024., 0., 1024.],
                [1024., 0., -1024.],
            ],
        ]
    }
    #[test]
    fn large_flat_floor_has_exact_support_at_the_reported_coasting_position() {
        let mut geometry = StaticGeometry::default();
        geometry.append(large_floor()).unwrap();
        for z in [5.150237, 6., 6.2, -800., 800.] {
            assert_eq!(
                geometry.support_height([6., 0., z], 0.25, 0.3),
                Some(0.),
                "z={z}"
            );
            assert!(geometry.can_occupy([6., 0., z], 0.3, 1.8), "z={z}");
        }
    }
    #[test]
    fn large_flat_floor_keeps_idle_and_coasting_feet_at_ground_level() {
        use crate::{CameraMode, DT, Engine, Input, Request};
        let mut engine = Engine::default();
        engine.dispatch(Request::SetStaticCollision {
            triangles: large_floor(),
            owners: None,
            append: false,
        });
        engine.dispatch(Request::SetCamera {
            position: [6., 0., 5.150237],
            yaw: 0.,
            pitch: 0.,
            mode: CameraMode::Walk,
        });
        engine.dispatch(Request::SetCharacterController {
            controller: Some(crate::quick_mmorpg::Configuration {
                profile: "quick-mmorpg-v1".into(),
                world_scale: 1. / 6.,
            }),
        });
        for frame in 0..300 {
            engine.step(
                DT,
                Input {
                    forward: if (120..180).contains(&frame) { 1. } else { 0. },
                    ..Input::default()
                },
            );
            assert_eq!(engine.camera.position[1], 0., "frame={frame}");
            assert!(engine.grounded, "frame={frame}");
        }
        assert!(engine.camera.position[2] < 2.);
    }
    #[test]
    fn horizontal_edges_and_vertices_keep_their_sphere_support_in_both_windings() {
        let vertices = [[0., 1., 0.], [1024., 1., 0.], [0., 1., 1024.]];
        for face in [vertices, [vertices[0], vertices[2], vertices[1]]] {
            let mut geometry = StaticGeometry::default();
            geometry.append(vec![face]).unwrap();
            assert_eq!(geometry.support_height([10., 1., 10.], 1.01, 0.3), Some(1.));
            let edge = geometry
                .support_height([10., 1., -0.18], 1.01, 0.3)
                .unwrap();
            assert!((edge - 0.94).abs() < 0.000001, "{edge}");
            let vertex = geometry
                .support_height([-0.1, 1., -0.1], 1.01, 0.3)
                .unwrap();
            assert!(
                (vertex - (0.7 + 0.07f32.sqrt())).abs() < 0.000001,
                "{vertex}"
            );
            assert_eq!(geometry.support_height([10., 1., -0.25], 1.01, 0.3), None);
            assert_eq!(geometry.support_height([10., 1., -0.31], 1.01, 0.3), None);
        }
    }
    fn wall() -> StaticGeometry {
        let mut g = StaticGeometry::default();
        g.append(vec![
            [[-4., 0., -2.], [4., 0., -2.], [4., 4., -2.]],
            [[-4., 0., -2.], [4., 4., -2.], [-4., 4., -2.]],
        ])
        .unwrap();
        g
    }
    #[test]
    fn capsule_contacts_triangle_faces_edges_and_reversed_winding() {
        let g = wall();
        assert!(g.can_occupy([0., 0., -1.65], 0.3, 1.8));
        assert!(!g.can_occupy([0., 0., -1.75], 0.3, 1.8));
        assert!(!g.can_occupy([4.15, 0., -2.], 0.3, 1.8));
        assert!(g.can_occupy([4.31, 0., -2.], 0.3, 1.8));
        assert!(!g.can_occupy([0., 0., -2.25], 0.3, 1.8));
    }
    #[test]
    fn camera_sphere_stops_before_wall_even_on_first_frame() {
        let fraction = wall().sweep([0., 1.9, 0.], [0., 2.3, -9.], 0.12);
        assert!(fraction > 0.20 && fraction < 0.21, "{fraction}");
        assert_eq!(wall().sweep([0., 1.9, 0.], [0., 2.3, 9.], 0.12), 1.);
    }
    #[test]
    fn camera_sphere_hits_edges_and_vertices_but_misses_a_parallel_near_edge() {
        let g = wall();
        let edge = g.sweep([4.05, 2., 0.], [4.05, 2., -4.], 0.12);
        assert!(edge > 0.47 && edge < 0.48, "{edge}");
        let vertex = g.sweep([4.05, 4.05, 0.], [4.05, 4.05, -4.], 0.12);
        assert!(vertex > 0.47 && vertex < 0.49, "{vertex}");
        assert_eq!(g.sweep([4.13, 2., 0.], [4.13, 2., -30.], 0.12), 1.);
    }
    #[test]
    fn floor_projection_is_two_sided_and_rejects_steep_walls() {
        let mut g = wall();
        g.append(vec![[[-3., 1., -3.], [3., 1., -3.], [0., 1., 3.]]])
            .unwrap();
        assert!((g.support_height([0., 1., 0.], 1.01, 0.3).unwrap() - 1.).abs() < 0.001);
        assert_eq!(g.support_height([0., 0., 0.], 0.25, 0.3), None);
        assert!(g.can_occupy([0., 1., 0.], 0.3, 1.8));
        assert!(!g.can_occupy([0., 0.8, 0.], 0.3, 1.8));
    }
    #[test]
    fn permanent_landing_ray_ignores_owned_roofs_above_real_terrain() {
        let mut g = StaticGeometry::default();
        g.append_owned(
            vec![
                [[-4., 2., -4.], [4., 2., -4.], [0., 2., 4.]],
                [[-4., 5., -4.], [4., 5., -4.], [0., 5., 4.]],
            ],
            Some(vec![u32::MAX, 7]),
        )
        .unwrap();
        let origin = [0., 10., 0.];
        let direction = [0., -1., 0.];
        assert_eq!(g.raycast(origin, direction, 20.).unwrap().point[1], 5.);
        assert_eq!(
            g.raycast_permanent(origin, direction, 20.).unwrap().point[1],
            2.
        );
        g.set_disabled(&[7].into_iter().collect());
        assert_eq!(
            g.raycast_permanent(origin, direction, 20.).unwrap().point[1],
            2.
        );
    }
    #[test]
    fn invalid_chunk_is_atomic_and_large_triangles_have_bounded_index() {
        let mut g = wall();
        let before = g.len();
        assert!(g.append(vec![[[0., 0., 0.]; 3]]).is_err());
        assert_eq!(g.len(), before);
        g.append(vec![[
            [-200., -1., -200.],
            [200., -1., -200.],
            [0., -1., 200.],
        ]])
        .unwrap();
        assert_eq!(g.broad.len(), 1);
        assert!((g.support_height([0., 0., 0.], 0., 0.3).unwrap() + 1.).abs() < 0.001);
        g.append(vec![[
            [-100_000., -2., -100_000.],
            [100_000., -2., -100_000.],
            [0., -2., 100_000.],
        ]])
        .unwrap();
        assert_eq!(g.broad.len(), 2);
    }
}
