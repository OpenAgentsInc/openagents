//! Instance-scoped, read-only triangle mesh queries in double-precision meters.
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const EPS: f64 = 1e-7;
const MAX_TRIANGLES: usize = 1_000_000;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Life {
    pub instance: u64,
    pub entity: u64,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ColliderKey {
    pub life: Life,
    pub shape: u32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Usage {
    Blocking,
    Trigger,
    Selection,
    Damage,
}
impl Usage {
    pub fn bit(self) -> u8 {
        match self {
            Self::Blocking => 1,
            Self::Trigger => 2,
            Self::Selection => 4,
            Self::Damage => 8,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Filter {
    pub instance: u64,
    pub layers: u32,
    pub ignore: Option<Life>,
    pub usages: u8,
    pub limit: usize,
}
impl Filter {
    pub fn blocking(instance: u64) -> Self {
        Self {
            instance,
            layers: u32::MAX,
            ignore: None,
            usages: Usage::Blocking.bit(),
            limit: 64,
        }
    }
    fn validate(self) -> Result<(), String> {
        if self.limit == 0 || self.limit > 4096 || self.usages & !15 != 0 {
            return Err("Invalid spatial query budget or usage".into());
        }
        Ok(())
    }
    fn admits(self, collider: &MeshCollider) -> bool {
        collider.key.life.instance == self.instance
            && collider.layers & self.layers != 0
            && collider.usage.bit() & self.usages != 0
            && self.ignore != Some(collider.key.life)
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Triangle(pub [DVec3; 3]);
impl Triangle {
    fn normal(self) -> DVec3 {
        (self.0[1] - self.0[0])
            .cross(self.0[2] - self.0[0])
            .normalize()
    }
    fn bounds(self) -> Bounds {
        Bounds {
            min: self.0[0].min(self.0[1]).min(self.0[2]),
            max: self.0[0].max(self.0[1]).max(self.0[2]),
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: DVec3,
    max: DVec3,
}
impl Bounds {
    fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
    fn intersects(self, other: Self) -> bool {
        self.min.cmple(other.max).all() && other.min.cmple(self.max).all()
    }
}
#[derive(Clone, Debug)]
struct Node {
    bounds: Bounds,
    children: Option<[usize; 2]>,
    triangles: Vec<usize>,
}
/// Validated source triangles and a deterministic median-split bounding hierarchy.
#[derive(Clone, Debug)]
pub struct Mesh {
    triangles: Vec<Triangle>,
    nodes: Vec<Node>,
}
impl Mesh {
    /// Compiles the twelve boundary triangles of a nondegenerate box.
    pub fn from_box(min: DVec3, max: DVec3) -> Result<Self, String> {
        valid_point(min)?;
        valid_point(max)?;
        if !min.cmplt(max).all() {
            return Err("Invalid collision box".into());
        }
        let corners: Vec<_> = (0..8)
            .map(|i| {
                DVec3::new(
                    if i & 1 == 0 { min.x } else { max.x },
                    if i & 2 == 0 { min.y } else { max.y },
                    if i & 4 == 0 { min.z } else { max.z },
                )
            })
            .collect();
        let faces = [
            [0, 2, 6, 4],
            [1, 5, 7, 3],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 1, 3, 2],
            [4, 6, 7, 5],
        ];
        Self::compile(
            faces
                .into_iter()
                .flat_map(|[a, b, c, d]| {
                    [
                        Triangle([corners[a], corners[b], corners[c]]),
                        Triangle([corners[a], corners[c], corners[d]]),
                    ]
                })
                .collect(),
        )
    }
    pub fn compile(triangles: Vec<Triangle>) -> Result<Self, String> {
        if triangles.is_empty() || triangles.len() > MAX_TRIANGLES {
            return Err("Invalid collision mesh triangle budget".into());
        }
        for triangle in &triangles {
            for vertex in triangle.0 {
                valid_point(vertex)?;
            }
            if (triangle.0[1] - triangle.0[0])
                .cross(triangle.0[2] - triangle.0[0])
                .length_squared()
                < 1e-20
            {
                return Err("Degenerate collision triangle".into());
            }
        }
        let mut mesh = Self {
            triangles,
            nodes: vec![],
        };
        mesh.build((0..mesh.triangles.len()).collect());
        Ok(mesh)
    }
    fn build(&mut self, mut indices: Vec<usize>) -> usize {
        let bounds = indices
            .iter()
            .map(|i| self.triangles[*i].bounds())
            .reduce(Bounds::union)
            .unwrap();
        let index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            children: None,
            triangles: vec![],
        });
        if indices.len() <= 8 {
            self.nodes[index].triangles = indices;
            return index;
        }
        let extent = bounds.max - bounds.min;
        let axis = if extent.x >= extent.y && extent.x >= extent.z {
            0
        } else if extent.y >= extent.z {
            1
        } else {
            2
        };
        indices.sort_by(|a, b| {
            let center = |i: usize| self.triangles[i].0.iter().map(|p| p[axis]).sum::<f64>();
            center(*a).total_cmp(&center(*b)).then(a.cmp(b))
        });
        let right = indices.split_off(indices.len() / 2);
        let left = self.build(indices);
        let right = self.build(right);
        self.nodes[index].children = Some([left, right]);
        index
    }
    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }
    fn visit(
        &self,
        bounds: Bounds,
        stats: &mut Stats,
        mut visit: impl FnMut(usize, Triangle) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            stats.nodes += 1;
            let node = &self.nodes[index];
            if !node.bounds.intersects(bounds) {
                continue;
            }
            if let Some([left, right]) = node.children {
                stack.push(right);
                stack.push(left);
            }
            for triangle in &node.triangles {
                stats.triangles += 1;
                visit(*triangle, self.triangles[*triangle])?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct MeshCollider {
    pub key: ColliderKey,
    pub layers: u32,
    pub usage: Usage,
    pub mesh: Mesh,
}
/// Endpoints describe the centerline, including sphere-shaped degenerate capsules.
#[derive(Clone, Copy, Debug)]
pub struct Capsule {
    pub a: DVec3,
    pub b: DVec3,
    pub radius: f64,
}
impl Capsule {
    fn validate(self) -> Result<(), String> {
        valid_point(self.a)?;
        valid_point(self.b)?;
        if !self.radius.is_finite() || self.radius <= 0. || self.radius > 10_000. {
            return Err("Invalid query capsule".into());
        }
        Ok(())
    }
    fn bounds(self) -> Bounds {
        Bounds {
            min: self.a.min(self.b) - DVec3::splat(self.radius + EPS),
            max: self.a.max(self.b) + DVec3::splat(self.radius + EPS),
        }
    }
    fn translated(self, delta: DVec3) -> Self {
        Self {
            a: self.a + delta,
            b: self.b + delta,
            ..self
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub collider: ColliderKey,
    pub triangle: usize,
    pub fraction: f64,
    pub distance: f64,
    pub position: DVec3,
    pub normal: DVec3,
    pub penetration: f64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub nodes: usize,
    pub triangles: usize,
}
#[derive(Clone, Debug)]
pub struct Results {
    pub hits: Vec<Hit>,
    pub truncated: bool,
    pub stats: Stats,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct HitKey(u64, ColliderKey, usize);
struct Collector {
    hits: BTreeMap<HitKey, Hit>,
    limit: usize,
    truncated: bool,
}
impl Collector {
    fn new(limit: usize) -> Self {
        Self {
            hits: BTreeMap::new(),
            limit,
            truncated: false,
        }
    }
    fn push(&mut self, hit: Hit) {
        let key = HitKey(hit.distance.max(0.).to_bits(), hit.collider, hit.triangle);
        if self.hits.len() == self.limit {
            self.truncated = true;
            if self
                .hits
                .last_key_value()
                .is_some_and(|(old, _)| key >= *old)
            {
                return;
            }
            self.hits.pop_last();
        }
        self.hits.insert(key, hit);
    }
    fn finish(self, stats: Stats) -> Results {
        Results {
            hits: self.hits.into_values().collect(),
            truncated: self.truncated,
            stats,
        }
    }
}
/// Collider identities determine iteration order; queries never update geometry.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    colliders: BTreeMap<ColliderKey, MeshCollider>,
}
impl Scene {
    pub fn insert(&mut self, collider: MeshCollider) -> Result<(), String> {
        if self.colliders.contains_key(&collider.key) {
            return Err("Collision identity already exists".into());
        }
        if self.colliders.len() >= 4096 {
            return Err("Collision scene budget exceeded".into());
        }
        if self
            .colliders
            .values()
            .map(|c| c.mesh.triangles.len())
            .sum::<usize>()
            + collider.mesh.triangles.len()
            > MAX_TRIANGLES
        {
            return Err("Collision scene triangle budget exceeded".into());
        }
        self.colliders.insert(collider.key, collider);
        Ok(())
    }
    pub fn remove(&mut self, key: ColliderKey) -> Option<MeshCollider> {
        self.colliders.remove(&key)
    }
    pub fn ray(
        &self,
        origin: DVec3,
        direction: DVec3,
        distance: f64,
        filter: Filter,
    ) -> Result<Results, String> {
        valid_point(origin)?;
        filter.validate()?;
        if !direction.is_finite()
            || (direction.length_squared() - 1.).abs() > 1e-8
            || !distance.is_finite()
            || !(0. ..=1_000_000.).contains(&distance)
        {
            return Err("Invalid spatial ray".into());
        }
        let end = origin + direction * distance;
        let bounds = Bounds {
            min: origin.min(end) - DVec3::splat(EPS),
            max: origin.max(end) + DVec3::splat(EPS),
        };
        let mut out = Collector::new(filter.limit);
        let mut stats = Stats::default();
        for collider in self.colliders.values().filter(|c| filter.admits(c)) {
            collider.mesh.visit(bounds, &mut stats, |index, triangle| {
                if let Some((at, point)) = ray_triangle(origin, direction, distance, triangle) {
                    let mut normal = triangle.normal();
                    if normal.dot(direction) > 0. {
                        normal = -normal;
                    }
                    out.push(Hit {
                        collider: collider.key,
                        triangle: index,
                        fraction: if distance > 0. { at / distance } else { 0. },
                        distance: at,
                        position: point,
                        normal,
                        penetration: 0.,
                    });
                }
                Ok(())
            })?;
        }
        Ok(out.finish(stats))
    }
    pub fn overlap(&self, capsule: Capsule, filter: Filter) -> Result<Results, String> {
        capsule.validate()?;
        filter.validate()?;
        let mut out = Collector::new(filter.limit);
        let mut stats = Stats::default();
        for collider in self.colliders.values().filter(|c| filter.admits(c)) {
            collider
                .mesh
                .visit(capsule.bounds(), &mut stats, |index, triangle| {
                    let (axis, point) = closest_segment_triangle(capsule.a, capsule.b, triangle);
                    let separation = axis.distance(point);
                    if separation <= capsule.radius + EPS {
                        out.push(Hit {
                            collider: collider.key,
                            triangle: index,
                            fraction: 0.,
                            distance: 0.,
                            position: point,
                            normal: contact_normal(axis, point, triangle, capsule),
                            penetration: (capsule.radius - separation).max(0.),
                        });
                    }
                    Ok(())
                })?;
        }
        Ok(out.finish(stats))
    }
    pub fn sweep(&self, capsule: Capsule, delta: DVec3, filter: Filter) -> Result<Results, String> {
        capsule.validate()?;
        filter.validate()?;
        if !delta.is_finite() || delta.length() > 1_000_000. {
            return Err("Invalid capsule displacement".into());
        }
        let bounds = capsule.bounds().union(capsule.translated(delta).bounds());
        let mut out = Collector::new(filter.limit);
        let mut stats = Stats::default();
        for collider in self.colliders.values().filter(|c| filter.admits(c)) {
            collider.mesh.visit(bounds, &mut stats, |index, triangle| {
                if let Some((fraction, point, normal, penetration)) =
                    sweep_triangle(capsule, delta, triangle)?
                {
                    out.push(Hit {
                        collider: collider.key,
                        triangle: index,
                        fraction,
                        distance: delta.length() * fraction,
                        position: point,
                        normal,
                        penetration,
                    });
                }
                Ok(())
            })?;
        }
        Ok(out.finish(stats))
    }
}
fn valid_point(point: DVec3) -> Result<(), String> {
    if !point.is_finite() || point.abs().max_element() > 1_000_000. {
        Err("Invalid spatial coordinate".into())
    } else {
        Ok(())
    }
}
fn point_segment(point: DVec3, a: DVec3, b: DVec3) -> DVec3 {
    let edge = b - a;
    a + edge
        * if edge.length_squared() > 0. {
            ((point - a).dot(edge) / edge.length_squared()).clamp(0., 1.)
        } else {
            0.
        }
}
fn inside(point: DVec3, triangle: Triangle) -> bool {
    let [a, b, c] = triangle.0;
    let normal = triangle.normal();
    [(a, b), (b, c), (c, a)].iter().all(|(start, end)| {
        (*end - *start).cross(point - *start).dot(normal) >= -EPS * (*end - *start).length()
    })
}
fn point_triangle(point: DVec3, triangle: Triangle) -> DVec3 {
    let [a, b, c] = triangle.0;
    let normal = triangle.normal();
    let projected = point - normal * (point - a).dot(normal);
    if inside(projected, triangle) {
        return projected;
    }
    [(a, b), (b, c), (c, a)]
        .into_iter()
        .map(|(a, b)| point_segment(point, a, b))
        .min_by(|a, b| {
            a.distance_squared(point)
                .total_cmp(&b.distance_squared(point))
        })
        .unwrap()
}
fn ray_triangle(
    origin: DVec3,
    direction: DVec3,
    max: f64,
    triangle: Triangle,
) -> Option<(f64, DVec3)> {
    let normal = triangle.normal();
    let denominator = direction.dot(normal);
    if denominator.abs() < 1e-14 {
        return None;
    }
    let at = (triangle.0[0] - origin).dot(normal) / denominator;
    if !(0. ..=max).contains(&at) {
        return None;
    }
    let point = origin + direction * at;
    inside(point, triangle).then_some((at, point))
}
fn segment_pair(a: DVec3, b: DVec3, c: DVec3, d: DVec3) -> (DVec3, DVec3) {
    let u = b - a;
    let v = d - c;
    let w = a - c;
    let uu = u.length_squared();
    let vv = v.length_squared();
    let uv = u.dot(v);
    let mut candidates = vec![
        (a, point_segment(a, c, d)),
        (b, point_segment(b, c, d)),
        (point_segment(c, a, b), c),
        (point_segment(d, a, b), d),
    ];
    let determinant = uu * vv - uv * uv;
    if determinant > 1e-14 * uu * vv {
        let s = (uv * v.dot(w) - vv * u.dot(w)) / determinant;
        let t = (uu * v.dot(w) - uv * u.dot(w)) / determinant;
        if (0. ..=1.).contains(&s) && (0. ..=1.).contains(&t) {
            candidates.push((a + u * s, c + v * t));
        }
    }
    candidates
        .into_iter()
        .min_by(|(a, b), (c, d)| a.distance_squared(*b).total_cmp(&c.distance_squared(*d)))
        .unwrap()
}
fn closest_segment_triangle(a: DVec3, b: DVec3, triangle: Triangle) -> (DVec3, DVec3) {
    let delta = b - a;
    let length = delta.length();
    if length > 0. {
        if let Some((_, point)) = ray_triangle(a, delta / length, length, triangle) {
            return (point, point);
        }
    }
    let [x, y, z] = triangle.0;
    let mut pairs = vec![
        (a, point_triangle(a, triangle)),
        (b, point_triangle(b, triangle)),
    ];
    for (c, d) in [(x, y), (y, z), (z, x)] {
        pairs.push(segment_pair(a, b, c, d));
    }
    pairs
        .into_iter()
        .min_by(|(a, b), (c, d)| a.distance_squared(*b).total_cmp(&c.distance_squared(*d)))
        .unwrap()
}
fn contact_normal(axis: DVec3, point: DVec3, triangle: Triangle, capsule: Capsule) -> DVec3 {
    let offset = axis - point;
    if offset.length_squared() > 1e-24 {
        return offset.normalize();
    }
    let normal = triangle.normal();
    if ((capsule.a + capsule.b) * 0.5 - triangle.0[0]).dot(normal) < 0. {
        -normal
    } else {
        normal
    }
}
fn sweep_triangle(
    capsule: Capsule,
    delta: DVec3,
    triangle: Triangle,
) -> Result<Option<(f64, DVec3, DVec3, f64)>, String> {
    let plane = triangle.normal();
    let a = (capsule.a - triangle.0[0]).dot(plane);
    let b = (capsule.b - triangle.0[0]).dot(plane);
    // A separating support plane also rejects tangent travel across mesh seams.
    if a * b >= 0.
        && a.abs().min(b.abs()) >= capsule.radius - EPS
        && a.signum() * delta.dot(plane) >= -1e-12
    {
        return Ok(None);
    }
    let mut time = 0.;
    for _ in 0..64 {
        let moved = capsule.translated(delta * time);
        let (axis, point) = closest_segment_triangle(moved.a, moved.b, triangle);
        let distance = axis.distance(point);
        let normal = contact_normal(axis, point, triangle, moved);
        let separation = distance - capsule.radius;
        let closing = -normal.dot(delta);
        if separation <= EPS {
            if time == 0. && separation >= -EPS && closing <= 1e-12 {
                return Ok(None);
            }
            return Ok(Some((time, point, normal, (-separation).max(0.))));
        }
        // Distance to translated convex geometry is convex. Its current tangent
        // bounds the earliest possible contact, so this advance cannot tunnel.
        if closing <= 1e-12 {
            return Ok(None);
        }
        let next = time + separation / closing;
        if next > 1. {
            return Ok(None);
        }
        if next <= time {
            return Err("Capsule sweep failed to converge".into());
        }
        time = next;
    }
    Err("Capsule sweep iteration budget exceeded".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(instance: u64, entity: u64, generation: u64) -> ColliderKey {
        ColliderKey {
            life: Life {
                instance,
                entity,
                generation,
            },
            shape: 0,
        }
    }
    fn wall(x: f64, key: ColliderKey, usage: Usage, layers: u32) -> MeshCollider {
        let a = DVec3::new(x, -10., -10.);
        let b = DVec3::new(x, 10., -10.);
        let c = DVec3::new(x, 10., 10.);
        let d = DVec3::new(x, -10., 10.);
        MeshCollider {
            key,
            layers,
            usage,
            mesh: Mesh::compile(vec![Triangle([a, b, c]), Triangle([a, c, d])]).unwrap(),
        }
    }
    fn capsule() -> Capsule {
        Capsule {
            a: DVec3::new(0., 0.35, 0.),
            b: DVec3::new(0., 1.45, 0.),
            radius: 0.35,
        }
    }
    #[test]
    fn capsule_stops_at_a_thin_triangle_wall_at_the_correct_time() {
        let mut scene = Scene::default();
        scene
            .insert(wall(1., key(1, 1, 0), Usage::Blocking, 1))
            .unwrap();
        let hits = scene
            .sweep(capsule(), DVec3::X * 100., Filter::blocking(1))
            .unwrap();
        let hit = hits.hits[0];
        assert!((hit.fraction - 0.0065).abs() < 1e-9);
        assert!((hit.normal + DVec3::X).length() < 1e-9);
        assert_eq!(hit.position.x, 1.);
        let touching = capsule().translated(DVec3::X * 0.65);
        assert!(
            scene
                .sweep(touching, DVec3::Z, Filter::blocking(1))
                .unwrap()
                .hits
                .is_empty()
        );
        assert!(
            scene
                .sweep(touching, -DVec3::X, Filter::blocking(1))
                .unwrap()
                .hits
                .is_empty()
        );
        assert_eq!(
            scene
                .sweep(touching, DVec3::X, Filter::blocking(1))
                .unwrap()
                .hits[0]
                .fraction,
            0.
        );
    }
    #[test]
    fn rounded_edge_contacts_do_not_use_an_expanded_box_approximation() {
        let triangle = Triangle([DVec3::ZERO, DVec3::X * 3., DVec3::Z * 3.]);
        let mut scene = Scene::default();
        scene
            .insert(MeshCollider {
                key: key(1, 1, 0),
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::compile(vec![triangle]).unwrap(),
            })
            .unwrap();
        let point = DVec3::new(-0.2, 0.2, 1.);
        let shape = Capsule {
            a: point,
            b: point,
            radius: 0.25,
        };
        assert!(
            scene
                .overlap(shape, Filter::blocking(1))
                .unwrap()
                .hits
                .is_empty()
        );
        let hit = scene
            .sweep(shape, -DVec3::Y * 0.2, Filter::blocking(1))
            .unwrap()
            .hits[0];
        assert!((hit.fraction - 0.25).abs() < 1e-6);
        assert!((hit.normal - DVec3::new(-0.8, 0.6, 0.)).length() < 1e-6);
        let crossing = Capsule {
            a: DVec3::new(-1., 0.3, 1.),
            b: DVec3::new(4., 0.3, 1.),
            radius: 0.35,
        };
        let hit = scene.overlap(crossing, Filter::blocking(1)).unwrap().hits[0];
        assert!((hit.penetration - 0.05).abs() < 1e-9);
        assert_eq!(hit.normal, DVec3::Y);
    }
    #[test]
    fn life_instance_layer_and_usage_filters_are_independent() {
        let mut scene = Scene::default();
        for collider in [
            wall(1., key(1, 1, 1), Usage::Blocking, 2),
            wall(2., key(1, 1, 2), Usage::Blocking, 2),
            wall(0.5, key(2, 1, 1), Usage::Blocking, 2),
            wall(0.25, key(1, 3, 0), Usage::Trigger, 2),
        ] {
            scene.insert(collider).unwrap();
        }
        let mut filter = Filter::blocking(1);
        filter.ignore = Some(key(1, 1, 1).life);
        filter.layers = 2;
        let hit = scene.ray(DVec3::ZERO, DVec3::X, 5., filter).unwrap().hits[0];
        assert_eq!(hit.collider, key(1, 1, 2));
        filter.usages = Usage::Trigger.bit();
        assert_eq!(
            scene.ray(DVec3::ZERO, DVec3::X, 5., filter).unwrap().hits[0].collider,
            key(1, 3, 0)
        );
        filter.instance = 2;
        filter.usages = Usage::Blocking.bit();
        assert_eq!(
            scene.ray(DVec3::ZERO, DVec3::X, 5., filter).unwrap().hits[0].collider,
            key(2, 1, 1)
        );
    }
    #[test]
    fn nearest_results_are_bounded_and_independent_of_collider_insertion_order() {
        let mut first = Scene::default();
        let mut second = Scene::default();
        for i in 1..=20 {
            first
                .insert(wall(i as f64, key(1, i, 0), Usage::Blocking, 1))
                .unwrap();
        }
        for i in (1..=20).rev() {
            second
                .insert(wall(i as f64, key(1, i, 0), Usage::Blocking, 1))
                .unwrap();
        }
        let mut filter = Filter::blocking(1);
        filter.limit = 3;
        let a = first.ray(DVec3::ZERO, DVec3::X, 30., filter).unwrap();
        let b = second.ray(DVec3::ZERO, DVec3::X, 30., filter).unwrap();
        assert_eq!(a.hits, b.hits);
        assert_eq!(a.hits.len(), 3);
        assert!(a.truncated && b.truncated);
        assert_eq!(a.hits[0].distance, 1.);
        assert_eq!(a.hits[2].distance, 2.);
        assert!(
            first
                .insert(wall(50., key(1, 1, 0), Usage::Blocking, 1))
                .is_err()
        );
        first.remove(key(1, 1, 0));
        assert_eq!(
            first.ray(DVec3::ZERO, DVec3::X, 30., filter).unwrap().hits[0].distance,
            2.
        );
    }
    #[test]
    fn mesh_hierarchy_prunes_distant_geometry_and_preserves_source_triangle_ids() {
        let mut triangles = vec![];
        for x in 0..1000 {
            let p = DVec3::X * (x as f64 * 10.);
            triangles.push(Triangle([p, p + DVec3::Z, p + DVec3::X]));
        }
        let mut scene = Scene::default();
        scene
            .insert(MeshCollider {
                key: key(1, 1, 0),
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::compile(triangles).unwrap(),
            })
            .unwrap();
        let hit = scene
            .ray(
                DVec3::new(5000.2, 2., 0.2),
                -DVec3::Y,
                4.,
                Filter::blocking(1),
            )
            .unwrap();
        assert_eq!(hit.hits[0].triangle, 500);
        assert!(hit.stats.triangles < 16);
        assert!(hit.stats.nodes < 40);
    }
    #[test]
    fn malformed_geometry_and_nonfinite_queries_are_refused() {
        assert!(Mesh::compile(vec![]).is_err());
        assert!(Mesh::compile(vec![Triangle([DVec3::ZERO; 3])]).is_err());
        assert!(Mesh::compile(vec![Triangle([DVec3::NAN, DVec3::X, DVec3::Y])]).is_err());
        let scene = Scene::default();
        let filter = Filter::blocking(1);
        assert!(scene.ray(DVec3::ZERO, DVec3::X * 2., 10., filter).is_err());
        assert!(scene.sweep(capsule(), DVec3::NAN, filter).is_err());
        let mut bad = filter;
        bad.limit = 0;
        assert!(scene.overlap(capsule(), bad).is_err());
        let mut shape = capsule();
        shape.radius = -1.;
        assert!(scene.overlap(shape, filter).is_err());
    }
    #[test]
    fn ramp_and_stair_queries_return_ground_height_and_surface_normals() {
        let ramp = MeshCollider {
            key: key(1, 1, 0),
            layers: 1,
            usage: Usage::Blocking,
            mesh: Mesh::compile(vec![Triangle([
                DVec3::ZERO,
                DVec3::new(4., 2., 0.),
                DVec3::Z * 4.,
            ])])
            .unwrap(),
        };
        let mut scene = Scene::default();
        scene.insert(ramp).unwrap();
        let shape = Capsule {
            a: DVec3::new(1., 2., 1.),
            b: DVec3::new(1., 3., 1.),
            radius: 0.35,
        };
        let hit = scene
            .sweep(shape, -DVec3::Y * 4., Filter::blocking(1))
            .unwrap()
            .hits[0];
        let normal = DVec3::new(-0.5, 1., 0.).normalize();
        assert!((hit.normal - normal).length() < 1e-9);
        assert!((hit.fraction - (1.5 - 0.35 / normal.y) / 4.).abs() < 1e-7);
        let mut scene = Scene::default();
        scene
            .insert(MeshCollider {
                key: key(1, 1, 0),
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::from_box(DVec3::new(1., 0., -2.), DVec3::new(3., 0.25, 2.)).unwrap(),
            })
            .unwrap();
        let hit = scene
            .sweep(capsule(), DVec3::X * 2., Filter::blocking(1))
            .unwrap()
            .hits[0];
        assert!(hit.normal.x < -0.9 && hit.normal.y > 0.25);
        let ground = scene
            .ray(DVec3::new(2., 1., 0.), -DVec3::Y, 2., Filter::blocking(1))
            .unwrap()
            .hits[0];
        assert_eq!(ground.position.y, 0.25);
        assert_eq!(ground.normal, DVec3::Y);
        assert!(Mesh::from_box(DVec3::ZERO, DVec3::ZERO).is_err());
    }
}
