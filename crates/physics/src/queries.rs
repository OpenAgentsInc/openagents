//! Instance-scoped, read-only mesh and capsule queries in double-precision meters.
use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

mod profiling;
pub use profiling::{QueryMetrics, QueryProfile};
mod snapshot;
pub use snapshot::{GeometrySnapshot, SceneCache, SceneSnapshot, ShapeSnapshot};

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
    pub exclude: Option<ColliderKey>,
    pub usages: u8,
    pub limit: usize,
}
impl Filter {
    pub fn blocking(instance: u64) -> Self {
        Self {
            instance,
            layers: u32::MAX,
            ignore: None,
            exclude: None,
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
        self.admits_shape(collider.key, collider.layers, collider.usage)
    }
    fn admits_shape(self, key: ColliderKey, layers: u32, usage: Usage) -> bool {
        key.life.instance == self.instance
            && layers & self.layers != 0
            && usage.bit() & self.usages != 0
            && self.ignore != Some(key.life)
            && self.exclude != Some(key)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
    triangles: Arc<Vec<Triangle>>,
    nodes: Arc<Vec<Node>>,
    solid_box: Option<Bounds>,
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
        let mut mesh = Self::compile(
            faces
                .into_iter()
                .flat_map(|[a, b, c, d]| {
                    [
                        Triangle([corners[a], corners[b], corners[c]]),
                        Triangle([corners[a], corners[c], corners[d]]),
                    ]
                })
                .collect(),
        )?;
        mesh.solid_box = Some(Bounds { min, max });
        Ok(mesh)
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
            triangles: Arc::new(triangles),
            nodes: Arc::new(vec![]),
            solid_box: None,
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
        Arc::make_mut(&mut self.nodes).push(Node {
            bounds,
            children: None,
            triangles: vec![],
        });
        if indices.len() <= 8 {
            Arc::make_mut(&mut self.nodes)[index].triangles = indices;
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
        Arc::make_mut(&mut self.nodes)[index].children = Some([left, right]);
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
    /// Source triangle index, or `usize::MAX` for a primitive contact.
    pub triangle: usize,
    pub fraction: f64,
    pub distance: f64,
    pub position: DVec3,
    pub normal: DVec3,
    /// Surface normal; mesh plane normals differ from rounded edge contacts.
    pub surface_normal: DVec3,
    pub penetration: f64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub nodes: usize,
    pub triangles: usize,
    pub capsule_tests: usize,
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
    profiling: std::sync::Arc<profiling::Measurements>,
    poses: BTreeMap<ColliderKey, Pose>,
    colliders: BTreeMap<ColliderKey, MeshCollider>,
    capsules: BTreeMap<ColliderKey, CapsuleCollider>,
    world_capsules: BTreeMap<ColliderKey, CapsuleCollider>,
}
/// A local-space capsule retains its exact life and query usage.
#[derive(Clone, Copy, Debug)]
pub struct CapsuleCollider {
    pub key: ColliderKey,
    pub capsule: Capsule,
    pub layers: u32,
    pub usage: Usage,
}
/// A rigid pose preserves a compiled mesh hierarchy without rescaling it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub position: DVec3,
    pub rotation: DQuat,
}
impl Default for Pose {
    fn default() -> Self {
        Self {
            position: DVec3::ZERO,
            rotation: DQuat::IDENTITY,
        }
    }
}
impl Pose {
    pub fn point(self, local: DVec3) -> DVec3 {
        self.position + self.rotation * local
    }
    pub fn inverse_point(self, world: DVec3) -> DVec3 {
        self.rotation.conjugate() * (world - self.position)
    }
    fn capsule(self, world: Capsule) -> Capsule {
        Capsule {
            a: self.inverse_point(world.a),
            b: self.inverse_point(world.b),
            radius: world.radius,
        }
    }
}
impl Scene {
    /// Enables bounded measurements for this scene and its clones.
    pub fn enable_profiling(&self) {
        self.profiling.enable();
    }
    pub fn query_profile(&self) -> Option<QueryProfile> {
        self.profiling.snapshot()
    }
    /// Preserves an observation scope when the host replaces scene geometry.
    pub fn continue_profiling(&mut self, previous: &Self) {
        self.profiling = previous.profiling.clone();
    }

    pub fn capsule_keys(&self) -> impl Iterator<Item = ColliderKey> + '_ {
        self.capsules.keys().copied()
    }
    pub fn insert_capsule(&mut self, collider: CapsuleCollider) -> Result<(), String> {
        collider.capsule.validate()?;
        if self.colliders.contains_key(&collider.key) || self.capsules.contains_key(&collider.key) {
            return Err("Collision identity already exists".into());
        }
        if self.colliders.len() + self.capsules.len() >= 4096 {
            return Err("Collision scene budget exceeded".into());
        }
        self.world_capsules.insert(collider.key, collider);
        self.capsules.insert(collider.key, collider);
        Ok(())
    }
    pub fn remove_capsule(&mut self, key: ColliderKey) -> Option<CapsuleCollider> {
        if !self.capsules.contains_key(&key) {
            return None;
        }
        self.poses.remove(&key);
        self.world_capsules.remove(&key);
        self.capsules.remove(&key)
    }
    fn capsule_shapes(&self, filter: Filter) -> impl Iterator<Item = (ColliderKey, Capsule)> + '_ {
        self.world_capsules
            .values()
            .filter(move |c| filter.admits_shape(c.key, c.layers, c.usage))
            .map(|c| (c.key, c.capsule))
    }
    pub fn pose(&self, key: ColliderKey) -> Option<Pose> {
        (self.colliders.contains_key(&key) || self.capsules.contains_key(&key))
            .then(|| self.poses.get(&key).copied().unwrap_or_default())
    }
    pub fn set_pose(&mut self, key: ColliderKey, pose: Pose) -> Result<(), String> {
        valid_point(pose.position)?;
        if !pose.rotation.is_finite() || (pose.rotation.length_squared() - 1.).abs() > 1e-8 {
            return Err("Invalid collision pose".into());
        }
        if !self.colliders.contains_key(&key) && !self.capsules.contains_key(&key) {
            return Err("Collision identity does not exist".into());
        }
        if let Some(local) = self.capsules.get(&key) {
            let mut world = *local;
            world.capsule.a = pose.point(local.capsule.a);
            world.capsule.b = pose.point(local.capsule.b);
            self.world_capsules.insert(key, world);
        }
        self.poses.insert(key, pose);
        Ok(())
    }
    pub fn insert(&mut self, collider: MeshCollider) -> Result<(), String> {
        if self.colliders.contains_key(&collider.key) || self.capsules.contains_key(&collider.key) {
            return Err("Collision identity already exists".into());
        }
        if self.colliders.len() + self.capsules.len() >= 4096 {
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
        if self.colliders.contains_key(&key) {
            self.poses.remove(&key);
        }
        self.colliders.remove(&key)
    }
    pub fn ray(
        &self,
        origin: DVec3,
        direction: DVec3,
        distance: f64,
        filter: Filter,
    ) -> Result<Results, String> {
        let mut observation = self.profiling.observe(profiling::Kind::Ray);
        valid_point(origin)?;
        filter.validate()?;
        if !direction.is_finite()
            || (direction.length_squared() - 1.).abs() > 1e-8
            || !distance.is_finite()
            || !(0. ..=1_000_000.).contains(&distance)
        {
            return Err("Invalid spatial ray".into());
        }
        let mut out = Collector::new(filter.limit);
        let mut stats = Stats::default();
        for (key, target) in self.capsule_shapes(filter) {
            stats.nodes += 1;
            // A zero-radius point uses the same convex sweep as capsule queries.
            let point = Capsule {
                a: origin,
                b: origin,
                radius: 0.,
            };
            stats.capsule_tests += 1;
            if let Some(hit) = sweep_capsule(point, direction * distance, target, key)? {
                out.push(hit);
            }
        }
        for collider in self.colliders.values().filter(|c| filter.admits(c)) {
            let pose = self.pose(collider.key).unwrap();
            let origin = pose.inverse_point(origin);
            let direction = pose.rotation.conjugate() * direction;
            let end = origin + direction * distance;
            let bounds = Bounds {
                min: origin.min(end) - DVec3::splat(EPS),
                max: origin.max(end) + DVec3::splat(EPS),
            };
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
                        position: pose.point(point),
                        normal: pose.rotation * normal,
                        surface_normal: pose.rotation * normal,
                        penetration: 0.,
                    });
                }
                Ok(())
            })?;
        }
        observation.finish(stats, out.truncated);
        Ok(out.finish(stats))
    }
    pub fn overlap(&self, capsule: Capsule, filter: Filter) -> Result<Results, String> {
        let mut observation = self.profiling.observe(profiling::Kind::Overlap);
        capsule.validate()?;
        filter.validate()?;
        let mut out = Collector::new(filter.limit);
        let mut stats = Stats::default();
        let mut bounds = capsule.bounds();
        bounds.min -= DVec3::splat(EPS);
        bounds.max += DVec3::splat(EPS);
        for (key, target) in self.capsule_shapes(filter) {
            stats.nodes += 1;
            if !bounds.intersects(target.bounds()) {
                continue;
            }
            stats.capsule_tests += 1;
            let (axis, point) = segment_pair(capsule.a, capsule.b, target.a, target.b);
            let separation = axis.distance(point) - capsule.radius - target.radius;
            if separation <= EPS {
                let normal = capsule_normal(axis, point, capsule, target);
                out.push(Hit {
                    collider: key,
                    triangle: usize::MAX,
                    fraction: 0.,
                    distance: 0.,
                    position: point + normal * target.radius,
                    normal,
                    surface_normal: normal,
                    penetration: (-separation).max(0.),
                });
            }
        }
        for collider in self.colliders.values().filter(|c| filter.admits(c)) {
            let pose = self.pose(collider.key).unwrap();
            let capsule = pose.capsule(capsule);
            if let Some(bounds) = collider.mesh.solid_box {
                if let Some((normal, penetration, point)) = inside_box(capsule, bounds) {
                    out.push(Hit {
                        collider: collider.key,
                        triangle: usize::MAX,
                        fraction: 0.,
                        distance: 0.,
                        position: pose.point(point),
                        normal: pose.rotation * normal,
                        surface_normal: pose.rotation * normal,
                        penetration,
                    });
                    continue;
                }
            }
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
                            position: pose.point(point),
                            normal: pose.rotation * contact_normal(axis, point, triangle, capsule),
                            surface_normal: pose.rotation
                                * oriented_normal(
                                    triangle,
                                    contact_normal(axis, point, triangle, capsule),
                                ),
                            penetration: (capsule.radius - separation).max(0.),
                        });
                    }
                    Ok(())
                })?;
        }
        observation.finish(stats, out.truncated);
        Ok(out.finish(stats))
    }
    pub fn sweep(&self, capsule: Capsule, delta: DVec3, filter: Filter) -> Result<Results, String> {
        let mut observation = self.profiling.observe(profiling::Kind::Sweep);
        capsule.validate()?;
        filter.validate()?;
        if !delta.is_finite() || delta.length() > 1_000_000. {
            return Err("Invalid capsule displacement".into());
        }
        let mut out = Collector::new(filter.limit);
        let mut stats = Stats::default();
        let swept_bounds = capsule.bounds().union(capsule.translated(delta).bounds());
        for (key, target) in self.capsule_shapes(filter) {
            stats.nodes += 1;
            if !swept_bounds.intersects(target.bounds()) {
                continue;
            }
            stats.capsule_tests += 1;
            if let Some(hit) = sweep_capsule(capsule, delta, target, key)? {
                out.push(hit);
            }
        }
        for collider in self.colliders.values().filter(|c| filter.admits(c)) {
            let pose = self.pose(collider.key).unwrap();
            let capsule = pose.capsule(capsule);
            let delta = pose.rotation.conjugate() * delta;
            if let Some(bounds) = collider.mesh.solid_box {
                if let Some((normal, penetration, point)) = inside_box(capsule, bounds) {
                    out.push(Hit {
                        collider: collider.key,
                        triangle: usize::MAX,
                        fraction: 0.,
                        distance: 0.,
                        position: pose.point(point),
                        normal: pose.rotation * normal,
                        surface_normal: pose.rotation * normal,
                        penetration,
                    });
                    continue;
                }
            }
            let bounds = capsule.bounds().union(capsule.translated(delta).bounds());
            collider.mesh.visit(bounds, &mut stats, |index, triangle| {
                if let Some((fraction, point, normal, penetration)) =
                    sweep_triangle(capsule, delta, triangle)?
                {
                    out.push(Hit {
                        collider: collider.key,
                        triangle: index,
                        fraction,
                        distance: delta.length() * fraction,
                        position: pose.point(point),
                        normal: pose.rotation * normal,
                        surface_normal: pose.rotation * oriented_normal(triangle, normal),
                        penetration,
                    });
                }
                Ok(())
            })?;
        }
        observation.finish(stats, out.truncated);
        Ok(out.finish(stats))
    }
}
fn capsule_normal(axis: DVec3, point: DVec3, capsule: Capsule, target: Capsule) -> DVec3 {
    (axis - point)
        .try_normalize()
        .or_else(|| ((capsule.a + capsule.b) - (target.a + target.b)).try_normalize())
        .unwrap_or(DVec3::X)
}
fn sweep_capsule(
    capsule: Capsule,
    delta: DVec3,
    target: Capsule,
    key: ColliderKey,
) -> Result<Option<Hit>, String> {
    let mut time = 0.;
    for _ in 0..64 {
        let moved = capsule.translated(delta * time);
        let (axis, point) = segment_pair(moved.a, moved.b, target.a, target.b);
        let normal = capsule_normal(axis, point, moved, target);
        let separation = axis.distance(point) - capsule.radius - target.radius;
        let closing = -normal.dot(delta);
        if separation <= EPS {
            if time == 0. && separation >= -EPS && closing <= 1e-12 {
                return Ok(None);
            }
            return Ok(Some(Hit {
                collider: key,
                triangle: usize::MAX,
                fraction: time,
                distance: delta.length() * time,
                position: point + normal * target.radius,
                normal,
                surface_normal: normal,
                penetration: (-separation).max(0.),
            }));
        }
        if closing <= 1e-12 {
            return Ok(None);
        }
        let next = time + separation / closing;
        if next > 1. {
            return Ok(None);
        }
        if next <= time {
            return Err("Capsule contact failed to converge".into());
        }
        time = next;
    }
    Err("Capsule contact iteration budget exceeded".into())
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
    fn scene_clones_share_mesh_buffers_and_keep_collider_membership_independent() {
        let id = key(1, 1, 0);
        let mut original = Scene::default();
        original.insert(wall(1., id, Usage::Blocking, 1)).unwrap();
        let cloned = original.clone();
        let before = cloned
            .ray(DVec3::ZERO, DVec3::X, 5., Filter::blocking(1))
            .unwrap();
        let a = &original.colliders[&id].mesh;
        let b = &cloned.colliders[&id].mesh;
        assert!(Arc::ptr_eq(&a.triangles, &b.triangles));
        assert!(Arc::ptr_eq(&a.nodes, &b.nodes));
        original.remove(id).unwrap();
        assert!(
            original
                .ray(DVec3::ZERO, DVec3::X, 5., Filter::blocking(1))
                .unwrap()
                .hits
                .is_empty()
        );
        drop(original);
        assert_eq!(
            cloned
                .ray(DVec3::ZERO, DVec3::X, 5., Filter::blocking(1))
                .unwrap()
                .hits,
            before.hits
        );
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

fn oriented_normal(triangle: Triangle, contact: DVec3) -> DVec3 {
    let normal = triangle.normal();
    if normal.dot(contact) < 0. {
        -normal
    } else {
        normal
    }
}

// Filled occupancy is available only for explicitly compiled solid boxes.
// Arbitrary triangle meshes remain two-sided surfaces. Upright solid recovery
// chooses a horizontal or upward exit so a buried spawn does not cross its floor.
#[cfg(test)]
mod capsule_contact_tests {
    use super::*;
    #[test]
    fn world_capsule_cache_tracks_pose_removal_and_reinsertion_and_prunes_distant_tests() {
        let mut scene = Scene::default();
        let capsule = Capsule {
            a: DVec3::ZERO,
            b: DVec3::Y,
            radius: 0.35,
        };
        let key = |entity| ColliderKey {
            life: Life {
                instance: 1,
                entity,
                generation: 0,
            },
            shape: 0,
        };
        for entity in 1..=64 {
            scene
                .insert_capsule(CapsuleCollider {
                    key: key(entity),
                    capsule,
                    layers: 1,
                    usage: Usage::Blocking,
                })
                .unwrap();
            scene
                .set_pose(
                    key(entity),
                    Pose {
                        position: DVec3::X * ((entity - 1) as f64 * 4.),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        let filter = Filter::blocking(1);
        let result = scene.overlap(capsule, filter).unwrap();
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].collider, key(1));
        assert_eq!(result.stats.nodes, 64);
        assert_eq!(result.stats.capsule_tests, 1);
        let pose = Pose {
            position: DVec3::new(1., 1., 0.),
            rotation: DQuat::from_rotation_z(1.57),
        };
        scene.set_pose(key(1), pose).unwrap();
        let transformed = Capsule {
            a: pose.point(capsule.a),
            b: pose.point(capsule.b),
            ..capsule
        };
        let result = scene.overlap(transformed, filter).unwrap();
        assert_eq!(result.hits[0].collider, key(1));
        assert!((result.hits[0].penetration - 0.7).abs() < EPS);
        scene.remove_capsule(key(1)).unwrap();
        assert!(scene.overlap(transformed, filter).unwrap().hits.is_empty());
        scene
            .insert_capsule(CapsuleCollider {
                key: key(1),
                capsule,
                layers: 1,
                usage: Usage::Blocking,
            })
            .unwrap();
        assert!(
            scene
                .set_pose(
                    key(1),
                    Pose {
                        rotation: DQuat::from_array([0.; 4]),
                        ..pose
                    }
                )
                .is_err()
        );
        let result = scene.overlap(capsule, filter).unwrap();
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].collider, key(1));
    }
    #[test]
    fn overlap_bounds_keep_contacts_within_narrow_phase_tolerance() {
        let mut scene = Scene::default();
        let capsule = Capsule {
            a: DVec3::ZERO,
            b: DVec3::ZERO,
            radius: 0.35,
        };
        scene
            .insert_capsule(CapsuleCollider {
                key: ColliderKey {
                    life: Life {
                        instance: 1,
                        entity: 1,
                        generation: 0,
                    },
                    shape: 0,
                },
                capsule,
                layers: 1,
                usage: Usage::Blocking,
            })
            .unwrap();
        let near = capsule.translated(DVec3::X * (0.7 + EPS * 0.5));
        assert_eq!(
            scene.overlap(near, Filter::blocking(1)).unwrap().hits.len(),
            1
        );
        let far = capsule.translated(DVec3::X * (0.7 + EPS * 2.));
        assert!(
            scene
                .overlap(far, Filter::blocking(1))
                .unwrap()
                .hits
                .is_empty()
        );
    }
    fn fixture() -> (Scene, ColliderKey, Capsule) {
        let key = ColliderKey {
            life: Life {
                instance: 7,
                entity: 9,
                generation: 3,
            },
            shape: 0,
        };
        let capsule = Capsule {
            a: DVec3::Y * 0.35,
            b: DVec3::Y * 1.45,
            radius: 0.35,
        };
        let mut scene = Scene::default();
        scene
            .insert_capsule(CapsuleCollider {
                key,
                capsule,
                layers: 2,
                usage: Usage::Blocking,
            })
            .unwrap();
        (scene, key, capsule)
    }
    #[test]
    fn fast_capsules_stop_at_exact_contact_and_tangent_travel_is_free() {
        let (scene, key, capsule) = fixture();
        let start = capsule.translated(-DVec3::X * 10.);
        let hit = scene
            .sweep(start, DVec3::X * 100., Filter::blocking(7))
            .unwrap()
            .hits[0];
        assert_eq!(hit.collider, key);
        assert!((hit.fraction - 0.093).abs() < 1e-9);
        assert!(hit.normal.distance(-DVec3::X) < 1e-9);
        let touching = capsule.translated(-DVec3::X * 0.7);
        assert!(
            scene
                .sweep(touching, DVec3::Z, Filter::blocking(7))
                .unwrap()
                .hits
                .is_empty()
        );
        assert!(
            scene
                .sweep(touching, -DVec3::X, Filter::blocking(7))
                .unwrap()
                .hits
                .is_empty()
        );
        assert!(
            !scene
                .sweep(touching, DVec3::X, Filter::blocking(7))
                .unwrap()
                .hits
                .is_empty()
        );
    }
    #[test]
    fn capsule_queries_obey_pose_life_instance_layers_and_removal() {
        let (mut scene, key, capsule) = fixture();
        let pose = Pose {
            position: DVec3::new(3., 2., -1.),
            rotation: DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2),
        };
        scene.set_pose(key, pose).unwrap();
        let center = pose.point((capsule.a + capsule.b) * 0.5);
        let point = Capsule {
            a: center,
            b: center,
            radius: 0.1,
        };
        let filter = Filter::blocking(7);
        assert_eq!(scene.overlap(point, filter).unwrap().hits[0].collider, key);
        assert_eq!(
            scene
                .ray(center - DVec3::Z * 2., DVec3::Z, 4., filter)
                .unwrap()
                .hits[0]
                .collider,
            key
        );
        assert!(
            scene
                .overlap(point, Filter::blocking(8))
                .unwrap()
                .hits
                .is_empty()
        );
        let mut ignored = filter;
        ignored.ignore = Some(key.life);
        assert!(scene.overlap(point, ignored).unwrap().hits.is_empty());
        ignored.ignore = Some(Life {
            generation: 2,
            ..key.life
        });
        assert!(!scene.overlap(point, ignored).unwrap().hits.is_empty());
        ignored.layers = 1;
        assert!(scene.overlap(point, ignored).unwrap().hits.is_empty());
        assert!(scene.remove(key).is_none());
        assert_eq!(scene.pose(key).unwrap().position, pose.position);
        assert!(scene.remove_capsule(key).is_some());
        assert!(scene.pose(key).is_none());
        assert!(scene.overlap(point, filter).unwrap().hits.is_empty());
    }
}

fn inside_box(capsule: Capsule, bounds: Bounds) -> Option<(DVec3, f64, DVec3)> {
    let delta = capsule.b - capsule.a;
    let mut enter: f64 = 0.;
    let mut exit: f64 = 1.;
    for axis in 0..3 {
        if delta[axis].abs() < 1e-12 {
            if capsule.a[axis] <= bounds.min[axis] || capsule.a[axis] >= bounds.max[axis] {
                return None;
            }
        } else {
            let a = (bounds.min[axis] - capsule.a[axis]) / delta[axis];
            let b = (bounds.max[axis] - capsule.a[axis]) / delta[axis];
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
        }
    }
    if enter >= exit {
        return None;
    }
    let mut point = capsule.a + delta * ((enter + exit) * 0.5);
    let lo = capsule.a.min(capsule.b) - DVec3::splat(capsule.radius);
    let hi = capsule.a.max(capsule.b) + DVec3::splat(capsule.radius);
    let mut best = (DVec3::ZERO, f64::INFINITY);
    for axis in 0..3 {
        let mut normal = DVec3::ZERO;
        normal[axis] = -1.;
        let negative = hi[axis] - bounds.min[axis];
        if axis != 1 && negative < best.1 {
            best = (normal, negative);
        }
        normal[axis] = 1.;
        let positive = bounds.max[axis] - lo[axis];
        if positive < best.1 {
            best = (normal, positive);
        }
    }
    for axis in 0..3 {
        if best.0[axis] < 0. {
            point[axis] = bounds.min[axis];
        }
        if best.0[axis] > 0. {
            point[axis] = bounds.max[axis];
        }
    }
    Some((best.0, best.1, point))
}
