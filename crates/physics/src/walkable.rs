//! Compiled multilayer walkable cells and bounded, instance-scoped routing.
//! Collision remains authoritative; a path supplies movement goals only.
use crate::{
    character::{Character, Settings},
    queries::{ColliderKey, Filter, Life, Mesh, MeshCollider, Scene, Usage},
};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap},
};

mod tiled;
pub use tiled::{Crowd, CrowdAgent, Scheduler, SearchScratch, Tile, Transition, TransitionKind};

const SKIN: f64 = 2e-5;
const MAX_CELLS: usize = 65_536;
const MAX_NODES: usize = 65_536;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Config {
    pub instance: u64,
    pub layers: u32,
    pub min: DVec3,
    pub max: DVec3,
    pub cell: f64,
    pub character: Settings,
    pub work_budget: usize,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct CompileStats {
    pub cells: usize,
    pub spans: usize,
    pub links: usize,
    pub work_units: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cell {
    pub feet: DVec3,
    /// Cardinal portals, including directed stair transitions.
    pub links: Vec<usize>,
}
/// Immutable walkable spans, eroded by the admitted capsule's clearance.
#[derive(Clone, Debug, Serialize)]
pub struct Navigation {
    config: Config,
    pub stats: CompileStats,
    #[serde(with = "tiled::cell_pairs")]
    cells: BTreeMap<(i32, i32), Vec<usize>>,
    nodes: Vec<Cell>,
    tiles: Vec<Tile>,
    node_tiles: Vec<usize>,
    tile_links: Vec<Vec<usize>>,
}
impl Navigation {
    /// Binds the same compiled local geometry to another world instance.
    pub fn bind_instance(&self, instance: u64) -> Self {
        let mut bound = self.clone();
        bound.config.instance = instance;
        bound
    }
    pub fn nodes(&self) -> &[Cell] {
        &self.nodes
    }
    pub fn compile(scene: &Scene, config: Config) -> Result<Self, String> {
        Self::compile_tiled(scene, config, 16, &[])
    }
    fn compile_tile(scene: &Scene, config: Config) -> Result<Self, String> {
        config.character.validate()?;
        if config.layers == 0
            || !config.min.is_finite()
            || !config.max.is_finite()
            || !config.min.cmplt(config.max).all()
            || !config.cell.is_finite()
            || !(0.1..=2.).contains(&config.cell)
            || config.work_budget == 0
            || config.work_budget > 100_000_000
            || config.min.abs().max_element() > 1_000_000.
            || config.max.abs().max_element() > 1_000_000.
        {
            return Err("Invalid navigation compilation configuration".into());
        }
        let width = ((config.max.x - config.min.x) / config.cell).ceil() as usize;
        let depth = ((config.max.z - config.min.z) / config.cell).ceil() as usize;
        if width
            .checked_mul(depth)
            .is_none_or(|count| count > MAX_CELLS)
        {
            return Err("Navigation cell budget exceeded".into());
        }
        let mut nav = Self {
            config,
            stats: CompileStats::default(),
            cells: BTreeMap::new(),
            nodes: vec![],
            tiles: vec![],
            node_tiles: vec![],
            tile_links: vec![],
        };
        let mut filter = Filter::blocking(config.instance);
        filter.limit = 128;
        filter.layers = config.layers;
        for x in 0..width {
            for z in 0..depth {
                nav.stats.cells += 1;
                nav.consume(1)?;
                let origin = DVec3::new(
                    config.min.x + (x as f64 + 0.5) * config.cell,
                    config.max.y + config.character.height,
                    config.min.z + (z as f64 + 0.5) * config.cell,
                );
                let hits = scene.ray(origin, -DVec3::Y, origin.y - config.min.y, filter)?;
                if hits.truncated {
                    return Err("Navigation span query budget exceeded".into());
                }
                let mut heights = vec![];
                for hit in hits.hits {
                    let height = hit.position.y;
                    if height < config.min.y
                        || height > config.max.y
                        || hit.surface_normal.y < config.character.slope_cos
                        || heights.iter().any(|old: &f64| (old - height).abs() < 1e-5)
                    {
                        continue;
                    }
                    heights.push(height);
                    let mut feet = DVec3::new(origin.x, height + SKIN, origin.z);
                    nav.consume(1)?;
                    let overlaps = scene.overlap(config.character.capsule(feet), filter)?;
                    if overlaps.truncated {
                        return Err("Navigation clearance query budget exceeded".into());
                    }
                    if overlaps.hits.iter().any(|hit| hit.penetration > SKIN) {
                        if overlaps
                            .hits
                            .iter()
                            .filter(|h| h.penetration > SKIN)
                            .any(|h| {
                                h.normal.y <= 0.
                                    || h.position.y > height + config.character.step_height + SKIN
                            })
                        {
                            continue;
                        }
                        // Rounded stair lips need an admitted standing pose, not
                        // wall erosion that removes every narrow stair tread.
                        nav.consume(64)?;
                        let mut standing = Character::new(feet);
                        standing.step(
                            scene,
                            filter,
                            config.character,
                            DVec3::ZERO,
                            false,
                            1. / 120.,
                        )?;
                        let change = standing.feet - feet;
                        if standing.support.is_none()
                            || change.y > config.character.step_height + SKIN
                            || DVec3::new(change.x, 0., change.z).length() > config.cell * 0.5
                        {
                            continue;
                        }
                        feet = standing.feet;
                    }
                    if nav.nodes.len() == MAX_NODES {
                        return Err("Navigation node budget exceeded".into());
                    }
                    let index = nav.nodes.len();
                    nav.nodes.push(Cell {
                        feet,
                        links: vec![],
                    });
                    nav.cells
                        .entry((x as i32, z as i32))
                        .or_default()
                        .push(index);
                    nav.stats.spans += 1;
                }
            }
        }
        for (coordinate, indices) in nav.cells.clone() {
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let neighbors = nav
                    .cells
                    .get(&(coordinate.0 + dx, coordinate.1 + dz))
                    .cloned()
                    .unwrap_or_default();
                for from in &indices {
                    for to in &neighbors {
                        let a = nav.nodes[*from].feet;
                        let b = nav.nodes[*to].feet;
                        if (a.y - b.y).abs() > config.character.step_height + SKIN {
                            continue;
                        }
                        let mut work = Work {
                            used: nav.stats.work_units,
                            limit: config.work_budget,
                        };
                        let admitted = traverse(scene, filter, config.character, a, b, &mut work)?;
                        nav.stats.work_units = work.used;
                        if admitted {
                            nav.nodes[*from].links.push(*to);
                            nav.stats.links += 1;
                        }
                    }
                }
            }
        }
        Ok(nav)
    }
    fn consume(&mut self, amount: usize) -> Result<(), String> {
        self.stats.work_units = self
            .stats
            .work_units
            .checked_add(amount)
            .ok_or("Navigation work counter exhausted")?;
        if self.stats.work_units > self.config.work_budget {
            return Err("Navigation compilation work budget exceeded".into());
        }
        Ok(())
    }
    fn coordinates(&self, p: DVec3) -> (i32, i32) {
        (
            ((p.x - self.config.min.x) / self.config.cell).floor() as i32,
            ((p.z - self.config.min.z) / self.config.cell).floor() as i32,
        )
    }
    fn nearest(
        &self,
        scene: &Scene,
        p: DVec3,
        filter: Filter,
        blockers: &Blockers,
        work: &mut Work,
    ) -> Result<Option<usize>, String> {
        work.charge(1)?;
        let overlaps = scene.overlap(self.config.character.capsule(p), filter)?;
        if overlaps.truncated || overlaps.hits.iter().any(|h| h.penetration > SKIN) {
            return Ok(None);
        }
        let (x, z) = self.coordinates(p);
        let mut candidates = vec![];
        for dx in -1..=1 {
            for dz in -1..=1 {
                for index in self.cells.get(&(x + dx, z + dz)).into_iter().flatten() {
                    let feet = self.nodes[*index].feet;
                    if (p.y - feet.y).abs() <= self.config.character.step_height + 0.05 {
                        candidates.push((*index, p.distance_squared(feet)));
                    }
                }
            }
        }
        candidates.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        for (index, _) in candidates {
            work.charge(blockers.work_size() + 1)?;
            if !blockers.clear(
                p,
                self.nodes[index].feet,
                self.config.character,
                filter.ignore,
            ) {
                continue;
            }
            if traverse(
                scene,
                filter,
                self.config.character,
                p,
                self.nodes[index].feet,
                work,
            )? {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }
    pub fn path(
        &self,
        scene: &Scene,
        blockers: &Blockers,
        instance: u64,
        start: DVec3,
        goal: DVec3,
        ignore: Option<Life>,
        budget: Budget,
    ) -> Result<Option<Path>, String> {
        self.path_with_scratch(
            scene,
            blockers,
            instance,
            start,
            goal,
            ignore,
            budget,
            &mut SearchScratch::default(),
        )
    }
    /// Reuses bounded search storage. Tile reachability rejects disconnected content first.
    pub fn path_with_scratch(
        &self,
        scene: &Scene,
        blockers: &Blockers,
        instance: u64,
        start: DVec3,
        goal: DVec3,
        ignore: Option<Life>,
        budget: Budget,
        scratch: &mut SearchScratch,
    ) -> Result<Option<Path>, String> {
        if instance != self.config.instance || instance != blockers.instance {
            return Err("Navigation query belongs to another instance".into());
        }
        if !start.is_finite()
            || !goal.is_finite()
            || start.abs().max_element() > 1_000_000.
            || goal.abs().max_element() > 1_000_000.
            || ignore.is_some_and(|life| life.instance != instance || life.entity == 0)
            || budget.nodes == 0
            || budget.nodes > MAX_NODES
            || budget.work_units == 0
            || budget.work_units > 100_000_000
        {
            return Err("Invalid navigation path query".into());
        }
        let mut work = Work {
            used: 0,
            limit: budget.work_units,
        };
        work.charge(2 * (blockers.work_size() + 1))?;
        let mut filter = Filter::blocking(instance);
        filter.ignore = ignore;
        filter.layers = self.config.layers;
        if !blockers.clear(start, start, self.config.character, ignore)
            || !blockers.clear(goal, goal, self.config.character, ignore)
        {
            return Ok(None);
        }
        let Some(source) = self.nearest(scene, start, filter, blockers, &mut work)? else {
            return Ok(None);
        };
        let Some(target) = self.nearest(scene, goal, filter, blockers, &mut work)? else {
            return Ok(None);
        };
        work.charge(blockers.work_size() + 1)?;
        if blockers.clear(start, goal, self.config.character, ignore)
            && shortcut(
                scene,
                filter,
                self.config.character,
                start,
                goal,
                &mut work,
                2048,
            )?
        {
            return Ok(Some(Path {
                points: vec![goal],
                expanded: 0,
                work_units: work.used,
                blocker_revision: blockers.revision,
            }));
        }
        let Some(corridor) = self.tile_corridor(source, target, &mut work)? else {
            return Ok(None);
        };
        scratch.begin(self.nodes.len());
        scratch.set(source, 0., usize::MAX);
        scratch.queue.push(Visit {
            tier: 0,
            estimate: 0.,
            cost: 0.,
            node: source,
        });
        let mut expanded = 0;
        while let Some(visit) = scratch.queue.pop() {
            if visit.cost > scratch.cost(visit.node) + 1e-10 {
                continue;
            }
            if expanded == budget.nodes {
                return Err("Navigation path work budget exceeded".into());
            }
            expanded += 1;
            if visit.node == target {
                let mut corridor = vec![goal];
                let mut node = target;
                while node != source {
                    if corridor.len() == 1024 {
                        return Err("Navigation waypoint budget exceeded".into());
                    }
                    corridor.push(self.nodes[node].feet);
                    node = scratch.parent[node];
                }
                corridor.push(self.nodes[source].feet);
                corridor.push(start);
                corridor.reverse();
                let mut points = vec![];
                let mut current = 0;
                let smoothing_end = work.used.saturating_add(4096).min(work.limit);
                // Shortcuts are independently admitted by the capsule controller.
                // Limit shortcut tests rather than searching every waypoint pair.
                while current + 1 < corridor.len() {
                    let mut next = current + 1;
                    for candidate in (current + 2..corridor.len().min(current + 18)).rev() {
                        if work.used.saturating_add(blockers.work_size() + 1) >= smoothing_end {
                            break;
                        }
                        let a = corridor[current];
                        let b = corridor[candidate];
                        work.charge(blockers.work_size() + 1)?;
                        if !blockers.clear(a, b, self.config.character, ignore) {
                            continue;
                        }
                        let quota = smoothing_end.saturating_sub(work.used);
                        if shortcut(scene, filter, self.config.character, a, b, &mut work, quota)? {
                            next = candidate;
                            break;
                        }
                    }
                    points.push(corridor[next]);
                    current = next;
                }
                return Ok(Some(Path {
                    points,
                    expanded,
                    work_units: work.used,
                    blocker_revision: blockers.revision,
                }));
            }
            let a = self.nodes[visit.node].feet;
            for next in &self.nodes[visit.node].links {
                let b = self.nodes[*next].feet;
                let cost = visit.cost + a.distance(b);
                if cost + 1e-10 < scratch.cost(*next) {
                    work.charge(blockers.work_size() + 1)?;
                    if !blockers.clear(a, b, self.config.character, ignore) {
                        continue;
                    }
                    scratch.set(*next, cost, visit.node);
                    scratch.queue.push(Visit {
                        tier: u8::from(!corridor[self.node_tiles[*next]]),
                        estimate: cost + b.distance(self.nodes[target].feet),
                        cost,
                        node: *next,
                    });
                }
            }
        }
        Ok(None)
    }
}

/// Expansion and collision work limits are independent.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub nodes: usize,
    pub work_units: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            nodes: 4096,
            work_units: 500_000,
        }
    }
}
struct Work {
    used: usize,
    limit: usize,
}
impl Work {
    fn charge(&mut self, amount: usize) -> Result<(), String> {
        let next = self
            .used
            .checked_add(amount)
            .ok_or("Navigation work counter exhausted")?;
        if next > self.limit {
            return Err("Navigation collision work budget exceeded".into());
        }
        self.used = next;
        Ok(())
    }
}

/// A route never grants permission to place an actor at its waypoints.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Path {
    pub points: Vec<DVec3>,
    pub expanded: usize,
    pub work_units: usize,
    pub blocker_revision: u64,
}
#[derive(Clone, Copy)]
struct Visit {
    tier: u8,
    estimate: f64,
    cost: f64,
    node: usize,
}
impl PartialEq for Visit {
    fn eq(&self, other: &Self) -> bool {
        self.tier == other.tier
            && self.estimate == other.estimate
            && self.node == other.node
            && self.cost == other.cost
    }
}
impl Eq for Visit {}
impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Visit {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .tier
            .cmp(&self.tier)
            .then_with(|| other.estimate.total_cmp(&self.estimate))
            .then(other.node.cmp(&self.node))
            .then(other.cost.total_cmp(&self.cost))
    }
}

/// Optional shortcut probes have their own soft quota. When it ends, retain
/// the compiled corridor; only exhausting the caller's hard quota fails a route.
fn shortcut(
    scene: &Scene,
    filter: Filter,
    settings: Settings,
    start: DVec3,
    goal: DVec3,
    work: &mut Work,
    quota: usize,
) -> Result<bool, String> {
    let hard_limit = work.limit;
    let soft_limit = work.used.saturating_add(quota).min(hard_limit);
    work.limit = soft_limit;
    let result = traverse(scene, filter, settings, start, goal, work);
    work.limit = hard_limit;
    match result {
        Err(error)
            if soft_limit < hard_limit && error == "Navigation collision work budget exceeded" =>
        {
            Ok(false)
        }
        other => other,
    }
}

/// Validates a continuous grounded crossing, including intermediate support.
fn traverse(
    scene: &Scene,
    filter: Filter,
    settings: Settings,
    start: DVec3,
    goal: DVec3,
    work: &mut Work,
) -> Result<bool, String> {
    let horizontal = DVec3::new(goal.x - start.x, 0., goal.z - start.z);
    let length = horizontal.length();
    if length > 20. {
        return Ok(false);
    }
    if (start.y - goal.y).abs() <= SKIN * 1.5 && length > 1e-6 {
        work.charge(1)?;
        let crossing = scene.sweep(settings.capsule(start), goal - start, filter)?;
        if !crossing.hits.is_empty() {
            return Ok(false);
        }
        let steps = (length / 0.2).ceil() as usize;
        let mut ground_filter = filter;
        ground_filter.limit = 1;
        for i in 0..=steps {
            work.charge(1)?;
            let sample = start.lerp(goal, i as f64 / steps as f64);
            let support = scene.ray(sample + DVec3::Y * 0.05, -DVec3::Y, 0.051, ground_filter)?;
            if !support
                .hits
                .first()
                .is_some_and(|h| h.surface_normal.y >= settings.slope_cos)
            {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    // A route probe cannot recover a compiled waypoint through another actor.
    // Reject occupied starts before the motor's bounded recovery loop.
    work.charge(1)?;
    let overlaps = scene.overlap(settings.capsule(start), filter)?;
    if overlaps.truncated || overlaps.hits.iter().any(|hit| hit.penetration > SKIN) {
        return Ok(false);
    }
    let mut character = Character::new(start);
    let steps = (length / 0.05).ceil().max(1.) as usize;
    let velocity = horizontal * (120. / steps as f64);
    for _ in 0..steps {
        // A controller step has bounded sweeps and overlaps; charge its worst case.
        work.charge(64)?;
        let previous = character.feet;
        if matches!(
            character.step_contained(scene, filter, settings, velocity, false, 1. / 120.)?,
            crate::character::Step::BlockedRecovery { .. }
        ) {
            return Ok(false);
        }
        let progress = character.feet - previous;
        let horizontal_progress =
            DVec3::new(progress.x, 0., progress.z).dot(horizontal.normalize_or_zero());
        if length > 1e-6 && horizontal_progress < length / steps as f64 * 0.2 {
            return Ok(false);
        }
        if character.support.is_none() {
            return Ok(false);
        }
    }
    Ok((character.feet - goal).length() < 0.08)
}

/// Blocker boxes (props and corpses) use their own shape identity. A corpse
/// keeps its actor's life, so sharing the living capsule's shape 0 would let a
/// character supported by one be carried by the other's unrelated pose frame.
pub const BLOCKER_SHAPE: u32 = 1;
pub fn blocker_key(life: Life) -> ColliderKey {
    ColliderKey {
        life,
        shape: BLOCKER_SHAPE,
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry {
    life: Life,
    min: DVec3,
    max: DVec3,
    alive: bool,
}
/// A bounded life book retains tombstones so an old update cannot revive a prop.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Blockers {
    pub instance: u64,
    pub revision: u64,
    entries: BTreeMap<u64, Entry>,
    #[serde(skip)]
    obstacles: Vec<crate::kinematic::Aabb>,
}
impl Blockers {
    pub fn new(instance: u64) -> Self {
        Self {
            instance,
            revision: 0,
            entries: BTreeMap::new(),
            obstacles: vec![],
        }
    }
    /// Adds transient hard geometry to a route query. These bounds do not alter
    /// the durable life book, its revision, or the shared cooked graph.
    pub fn with_obstacles(
        &self,
        obstacles: impl IntoIterator<Item = crate::kinematic::Aabb>,
    ) -> Result<Self, String> {
        let mut query = self.clone();
        query.obstacles.clear();
        for bounds in obstacles {
            if query.obstacles.len() == 1024
                || !bounds.min.is_finite()
                || !bounds.max.is_finite()
                || !bounds.min.cmplt(bounds.max).all()
                || bounds.min.abs().max_element() > 1_000_000.
                || bounds.max.abs().max_element() > 1_000_000.
            {
                return Err("Invalid navigation obstruction or obstruction budget exceeded".into());
            }
            query.obstacles.push(bounds);
        }
        Ok(query)
    }
    fn work_size(&self) -> usize {
        self.entries.len() + self.obstacles.len()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.entries.len() > 256
            || self.entries.iter().any(|(id, e)| {
                *id != e.life.entity
                    || e.life.instance != self.instance
                    || *id == 0
                    || !e.min.is_finite()
                    || !e.max.is_finite()
                    || !e.min.cmplt(e.max).all()
                    || e.min.abs().max_element() > 1_000_000.
                    || e.max.abs().max_element() > 1_000_000.
            })
        {
            return Err("Invalid navigation blocker state".into());
        }
        Ok(())
    }
    pub fn upsert(&mut self, life: Life, min: DVec3, max: DVec3) -> Result<(), String> {
        if life.instance != self.instance || life.entity == 0 {
            return Err("Invalid navigation blocker life".into());
        }
        Mesh::from_box(min, max)?;
        if let Some(old) = self.entries.get(&life.entity) {
            if old.life.generation > life.generation
                || (old.life.generation == life.generation && !old.alive)
            {
                return Err("Navigation blocker life is stale".into());
            }
            if old.life == life && old.min == min && old.max == max && old.alive {
                return Ok(());
            }
        } else if self.entries.len() == 256 {
            return Err("Navigation blocker budget exceeded".into());
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Navigation blocker revision exhausted")?;
        self.entries.insert(
            life.entity,
            Entry {
                life,
                min,
                max,
                alive: true,
            },
        );
        self.revision = revision;
        Ok(())
    }
    pub fn remove(&mut self, life: Life) -> Result<bool, String> {
        if life.instance != self.instance {
            return Err("Navigation blocker belongs to another instance".into());
        }
        if !self
            .entries
            .get(&life.entity)
            .is_some_and(|e| e.life == life && e.alive)
        {
            return Ok(false);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Navigation blocker revision exhausted")?;
        self.entries.get_mut(&life.entity).unwrap().alive = false;
        self.revision = revision;
        Ok(true)
    }
    pub fn active_bounds(&self) -> impl Iterator<Item = (Life, DVec3, DVec3)> + '_ {
        self.entries
            .values()
            .filter(|e| e.alive)
            .map(|e| (e.life, e.min, e.max))
    }
    pub fn colliders(&self) -> Result<Vec<MeshCollider>, String> {
        self.validate()?;
        self.entries
            .values()
            .filter(|e| e.alive)
            .map(|e| {
                Ok(MeshCollider {
                    key: blocker_key(e.life),
                    layers: 1,
                    usage: Usage::Blocking,
                    mesh: Mesh::from_box(e.min, e.max)?,
                })
            })
            .collect()
    }
    fn clear(&self, start: DVec3, goal: DVec3, settings: Settings, ignore: Option<Life>) -> bool {
        let center = start + DVec3::Y * (settings.height * 0.5);
        let half = DVec3::new(settings.radius, settings.height * 0.5, settings.radius);
        self.entries
            .values()
            .filter(|e| e.alive && Some(e.life) != ignore)
            .map(|e| crate::kinematic::Aabb {
                min: e.min,
                max: e.max,
            })
            .chain(self.obstacles.iter().copied())
            .all(|bounds| {
                crate::kinematic::sweep_box(center, half, goal - start, &[bounds])
                    .is_ok_and(|hit| hit.is_none())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn add(scene: &mut Scene, id: u32, min: DVec3, max: DVec3) {
        scene
            .insert(MeshCollider {
                key: ColliderKey {
                    life: Life {
                        instance: 7,
                        entity: 0,
                        generation: 0,
                    },
                    shape: id,
                },
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::from_box(min, max).unwrap(),
            })
            .unwrap();
    }
    pub(super) fn fixture() -> (Scene, Config) {
        let mut scene = Scene::default();
        add(
            &mut scene,
            0,
            DVec3::new(-5., -1., -5.),
            DVec3::new(5., 0., 5.),
        );
        let config = Config {
            instance: 7,
            layers: 1,
            min: DVec3::new(-4., -0.1, -4.),
            max: DVec3::new(4., 2., 4.),
            cell: 0.5,
            character: Settings::default(),
            work_budget: 10_000_000,
        };
        (scene, config)
    }
    #[test]
    fn column_route_and_no_path_are_distinct_from_budget_refusal() {
        let (mut scene, config) = fixture();
        add(
            &mut scene,
            1,
            DVec3::new(-0.6, 0., -0.6),
            DVec3::new(0.6, 4., 0.6),
        );
        let nav = Navigation::compile(&scene, config).unwrap();
        let blockers = Blockers::new(7);
        let start = DVec3::new(-2., SKIN, 0.);
        let goal = DVec3::new(2., SKIN, 0.);
        let path = nav
            .path(&scene, &blockers, 7, start, goal, None, Budget::default())
            .unwrap()
            .unwrap();
        assert!(path.points.iter().any(|p| p.z.abs() > 0.95));
        assert!(
            nav.path(
                &scene,
                &blockers,
                7,
                start,
                goal,
                None,
                Budget {
                    nodes: 1,
                    ..Budget::default()
                }
            )
            .is_err()
        );
        assert!(
            nav.path(
                &scene,
                &blockers,
                7,
                start,
                goal,
                None,
                Budget {
                    work_units: 1,
                    ..Budget::default()
                }
            )
            .is_err()
        );
        assert!(
            nav.path(
                &scene,
                &blockers,
                7,
                DVec3::splat(1e100),
                goal,
                None,
                Budget::default()
            )
            .is_err()
        );
        assert!(
            nav.path(&scene, &blockers, 8, start, goal, None, Budget::default())
                .is_err()
        );
        assert!(
            nav.path(
                &scene,
                &blockers,
                7,
                start,
                DVec3::new(0., SKIN, 0.),
                None,
                Budget::default()
            )
            .unwrap()
            .is_none()
        );
        let again = nav
            .path(&scene, &blockers, 7, start, goal, None, Budget::default())
            .unwrap()
            .unwrap();
        assert_eq!(path.points, again.points);
    }
    #[test]
    fn stairs_and_low_clearance() {
        let (mut scene, config) = fixture();
        for i in 0..3 {
            add(
                &mut scene,
                i + 1,
                DVec3::new(i as f64 * 0.9, 0., -1.),
                DVec3::new((i + 1) as f64 * 0.9, (i + 1) as f64 * 0.25, 1.),
            );
        }
        let nav = Navigation::compile(&scene, config).unwrap();
        let path = nav
            .path(
                &scene,
                &Blockers::new(7),
                7,
                DVec3::new(-1., SKIN, 0.),
                DVec3::new(2.25, 0.75 + SKIN, 0.),
                None,
                Budget::default(),
            )
            .unwrap()
            .unwrap();
        assert!(path.points.iter().any(|p| p.y > 0.7));
        add(
            &mut scene,
            8,
            DVec3::new(-4., 1.5, -4.),
            DVec3::new(4., 1.6, 4.),
        );
        let covered = Navigation::compile(&scene, config).unwrap();
        assert!(covered.nodes.iter().all(|node| node.feet.y > 1.5));
        assert!(
            covered
                .path(
                    &scene,
                    &Blockers::new(7),
                    7,
                    DVec3::new(-1., SKIN, 0.),
                    DVec3::new(2.25, 0.75 + SKIN, 0.),
                    None,
                    Budget::default()
                )
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn slope_admission_uses_the_character_profile() {
        use crate::queries::Triangle;
        for (angle, admitted) in [(20_f64.to_radians(), true), (60_f64.to_radians(), false)] {
            let (mut scene, config) = fixture();
            let height = 4. * angle.tan();
            let a = DVec3::new(0., 0., -2.);
            let b = DVec3::new(4., height, -2.);
            let c = DVec3::new(4., height, 2.);
            let d = DVec3::new(0., 0., 2.);
            scene
                .insert(MeshCollider {
                    key: ColliderKey {
                        life: Life {
                            instance: 7,
                            entity: 1,
                            generation: 0,
                        },
                        shape: 0,
                    },
                    layers: 1,
                    usage: Usage::Blocking,
                    mesh: Mesh::compile(vec![Triangle([a, b, c]), Triangle([a, c, d])]).unwrap(),
                })
                .unwrap();
            let nav = Navigation::compile(
                &scene,
                Config {
                    max: DVec3::new(4., 8., 4.),
                    ..config
                },
            )
            .unwrap();
            let offset = config.character.radius * (1. / angle.cos() - 1.);
            let path = nav
                .path(
                    &scene,
                    &Blockers::new(7),
                    7,
                    DVec3::new(-1., SKIN, 0.),
                    DVec3::new(2., 2. * angle.tan() + offset + SKIN, 0.),
                    None,
                    Budget::default(),
                )
                .unwrap();
            assert_eq!(path.is_some(), admitted);
        }
    }
    #[test]
    fn disconnected_layers_do_not_create_a_vertical_shortcut() {
        let (mut scene, mut config) = fixture();
        config.max.y = 4.;
        add(
            &mut scene,
            1,
            DVec3::new(-2., 3., -2.),
            DVec3::new(2., 3.1, 2.),
        );
        let nav = Navigation::compile(&scene, config).unwrap();
        assert!(
            nav.path(
                &scene,
                &Blockers::new(7),
                7,
                DVec3::new(0., SKIN, 0.),
                DVec3::new(0., 3.1 + SKIN, 0.),
                None,
                Budget::default()
            )
            .unwrap()
            .is_none()
        );
        assert!(nav.nodes.iter().any(|n| n.feet.y > 3.));
        assert!(nav.nodes.iter().any(|n| n.feet.y < 0.01));
    }
    #[test]
    fn blockers_replan_and_fence_reused_lives() {
        let (scene, config) = fixture();
        let nav = Navigation::compile(&scene, config).unwrap();
        let mut blockers = Blockers::new(7);
        let life = Life {
            instance: 7,
            entity: 99,
            generation: 0,
        };
        let start = DVec3::new(-2., SKIN, 0.);
        let goal = DVec3::new(2., SKIN, 0.);
        let direct = nav
            .path(&scene, &blockers, 7, start, goal, None, Budget::default())
            .unwrap()
            .unwrap();
        blockers
            .upsert(life, DVec3::new(-0.6, 0., -0.6), DVec3::new(0.6, 3., 0.6))
            .unwrap();
        let detour = nav
            .path(&scene, &blockers, 7, start, goal, None, Budget::default())
            .unwrap()
            .unwrap();
        assert!(detour.points.iter().any(|p| p.z.abs() > 0.95));
        assert!(detour.blocker_revision > direct.blocker_revision);
        assert!(blockers.remove(life).unwrap());
        assert!(blockers.upsert(life, DVec3::ZERO, DVec3::ONE).is_err());
        let next = Life {
            generation: 1,
            ..life
        };
        blockers
            .upsert(next, DVec3::new(-0.6, 0., -0.6), DVec3::new(0.6, 3., 0.6))
            .unwrap();
        assert!(!blockers.remove(life).unwrap());
        let old_ignore = nav
            .path(
                &scene,
                &blockers,
                7,
                start,
                goal,
                Some(life),
                Budget::default(),
            )
            .unwrap()
            .unwrap();
        assert!(old_ignore.points.iter().any(|p| p.z.abs() > 0.95));
        let own_ignore = nav
            .path(
                &scene,
                &blockers,
                7,
                start,
                goal,
                Some(next),
                Budget::default(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(own_ignore.points, direct.points);
        assert!(blockers.remove(next).unwrap());
        let restored: Blockers =
            serde_json::from_slice(&serde_json::to_vec(&blockers).unwrap()).unwrap();
        restored.validate().unwrap();
        let clear = nav
            .path(&scene, &restored, 7, start, goal, None, Budget::default())
            .unwrap()
            .unwrap();
        assert_eq!(direct.points, clear.points);
    }
    #[test]
    fn configuration_and_compilation_work_are_bounded() {
        let (scene, config) = fixture();
        assert!(
            Navigation::compile(
                &scene,
                Config {
                    work_budget: 1,
                    ..config
                }
            )
            .is_err()
        );
        assert!(
            Navigation::compile(
                &scene,
                Config {
                    cell: f64::NAN,
                    ..config
                }
            )
            .is_err()
        );
        assert!(
            Navigation::compile(
                &scene,
                Config {
                    max: DVec3::splat(1_000_000.),
                    ..config
                }
            )
            .is_err()
        );
    }
}
