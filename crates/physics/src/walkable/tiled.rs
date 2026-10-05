//! Content-addressed walkable tiles, reusable search, and bounded crowd work.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, VecDeque};

/// Tile identity covers local collision sources, controller settings, and spans.
/// A door or prop invalidates routes locally without changing immutable cooked content.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tile {
    pub coordinate: [i32; 2],
    pub min: DVec3,
    pub max: DVec3,
    pub digest: [u8; 32],
    pub spans: usize,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TransitionKind {
    /// A continuous supported crossing, independently tested by the capsule motor.
    Grounded,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Transition {
    pub id: u64,
    pub from: DVec3,
    pub to: DVec3,
    pub bidirectional: bool,
    pub kind: TransitionKind,
}
impl Navigation {
    /// Cooks aligned tiles and connects directed seam portals. The global graph
    /// retains every admitted span; the tile graph supplies a reachability hierarchy.
    pub fn compile_tiled(
        scene: &Scene,
        config: Config,
        tile_cells: usize,
        transitions: &[Transition],
    ) -> Result<Self, String> {
        config.character.validate()?;
        if tile_cells == 0
            || tile_cells > 256
            || transitions.len() > 4096
            || config.layers == 0
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
            return Err("Invalid tiled navigation configuration".into());
        }
        let width = ((config.max.x - config.min.x) / config.cell).ceil() as usize;
        let depth = ((config.max.z - config.min.z) / config.cell).ceil() as usize;
        let count = width
            .div_ceil(tile_cells)
            .checked_mul(depth.div_ceil(tile_cells));
        if count.is_none_or(|n| n > 4096) || width.checked_mul(depth).is_none_or(|n| n > 1_048_576)
        {
            return Err("Navigation tile budget exceeded".into());
        }
        let snapshot = scene.snapshot(config.instance)?;
        let mut nav = Self {
            config,
            stats: CompileStats::default(),
            cells: BTreeMap::new(),
            nodes: vec![],
            tiles: vec![],
            node_tiles: vec![],
            tile_links: vec![],
        };
        for x in (0..width).step_by(tile_cells) {
            for z in (0..depth).step_by(tile_cells) {
                let min = DVec3::new(
                    config.min.x + x as f64 * config.cell,
                    config.min.y,
                    config.min.z + z as f64 * config.cell,
                );
                let max = DVec3::new(
                    (min.x + tile_cells as f64 * config.cell).min(config.max.x),
                    config.max.y,
                    (min.z + tile_cells as f64 * config.cell).min(config.max.z),
                );
                let local = Self::compile_tile(
                    scene,
                    Config {
                        min,
                        max,
                        work_budget: config
                            .work_budget
                            .saturating_sub(nav.stats.work_units)
                            .max(1),
                        ..config
                    },
                )?;
                if nav.nodes.len() + local.nodes.len() > 1_048_576 {
                    return Err("Navigation tiled span budget exceeded".into());
                }
                let digest = tile_digest(&snapshot, &local)?;
                let offset = nav.nodes.len();
                let tile = nav.tiles.len();
                nav.tiles.push(Tile {
                    coordinate: [(x / tile_cells) as i32, (z / tile_cells) as i32],
                    min,
                    max,
                    digest,
                    spans: local.nodes.len(),
                });
                nav.tile_links.push(vec![]);
                nav.stats.cells += local.stats.cells;
                nav.stats.spans += local.stats.spans;
                nav.stats.links += local.stats.links;
                nav.consume(local.stats.work_units)?;
                nav.node_tiles
                    .extend(std::iter::repeat_n(tile, local.nodes.len()));
                nav.nodes.extend(local.nodes.into_iter().map(|mut node| {
                    for link in &mut node.links {
                        *link += offset;
                    }
                    node
                }));
                for ((cx, cz), nodes) in local.cells {
                    nav.cells.insert(
                        (cx + x as i32, cz + z as i32),
                        nodes.into_iter().map(|n| n + offset).collect(),
                    );
                }
            }
        }
        let filter = Filter {
            layers: config.layers,
            ..Filter::blocking(config.instance)
        };
        // Only boundary cells need seam tests. Preserve the same directed motor rule.
        for (coordinate, indices) in nav.cells.clone() {
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let neighbors = nav
                    .cells
                    .get(&(coordinate.0 + dx, coordinate.1 + dz))
                    .cloned()
                    .unwrap_or_default();
                for from in &indices {
                    for to in &neighbors {
                        if nav.node_tiles[*from] == nav.node_tiles[*to] {
                            continue;
                        }
                        if (nav.nodes[*from].feet.y - nav.nodes[*to].feet.y).abs()
                            > config.character.step_height + SKIN
                        {
                            continue;
                        }
                        nav.admit_link(scene, filter, *from, *to)?;
                    }
                }
            }
        }
        let mut ids = BTreeSet::new();
        let blockers = Blockers::new(config.instance);
        for transition in transitions {
            if transition.id == 0
                || !ids.insert(transition.id)
                || !transition.from.is_finite()
                || !transition.to.is_finite()
                || transition.from.abs().max_element() > 1_000_000.
                || transition.to.abs().max_element() > 1_000_000.
            {
                return Err("Invalid navigation transition identity or endpoints".into());
            }
            let mut work = Work {
                used: nav.stats.work_units,
                limit: config.work_budget,
            };
            let from = nav.nearest(scene, transition.from, filter, &blockers, &mut work)?;
            let to = nav.nearest(scene, transition.to, filter, &blockers, &mut work)?;
            nav.stats.work_units = work.used;
            let (Some(from), Some(to)) = (from, to) else {
                return Err("Navigation transition has no admitted endpoints".into());
            };
            if !nav.admit_link(scene, filter, from, to)? {
                return Err("Navigation transition fails collision admission".into());
            }
            if transition.bidirectional && !nav.admit_link(scene, filter, to, from)? {
                return Err("Reverse navigation transition fails collision admission".into());
            }
        }
        for links in &mut nav.tile_links {
            links.sort_unstable();
            links.dedup();
        }
        for node in &mut nav.nodes {
            node.links.sort_unstable();
            node.links.dedup();
        }
        // Seal seam and authored transition identities into each affected tile.
        let mut span_offset = 0;
        for tile in 0..nav.tiles.len() {
            let mut hash = Sha256::new();
            hash.update(b"verse.navigation.portals.v1\0");
            hash.update(nav.tiles[tile].digest);
            for i in span_offset..span_offset + nav.tiles[tile].spans {
                let node = &nav.nodes[i];
                for next in &node.links {
                    if nav.node_tiles[i] != nav.node_tiles[*next] {
                        for value in node
                            .feet
                            .to_array()
                            .into_iter()
                            .chain(nav.nodes[*next].feet.to_array())
                        {
                            hash.update(value.to_le_bytes());
                        }
                    }
                }
            }
            for transition in transitions {
                if nav
                    .affected_tiles(
                        transition.from.min(transition.to),
                        transition.from.max(transition.to),
                    )?
                    .contains(&tile)
                {
                    hash.update(serde_json::to_vec(transition).map_err(|e| e.to_string())?);
                }
            }
            nav.tiles[tile].digest = hash.finalize().into();
            span_offset += nav.tiles[tile].spans;
        }
        Ok(nav)
    }
    fn admit_link(
        &mut self,
        scene: &Scene,
        filter: Filter,
        from: usize,
        to: usize,
    ) -> Result<bool, String> {
        let mut work = Work {
            used: self.stats.work_units,
            limit: self.config.work_budget,
        };
        let admitted = traverse(
            scene,
            filter,
            self.config.character,
            self.nodes[from].feet,
            self.nodes[to].feet,
            &mut work,
        )?;
        self.stats.work_units = work.used;
        if admitted && !self.nodes[from].links.contains(&to) {
            self.nodes[from].links.push(to);
            self.stats.links += 1;
            let (a, b) = (self.node_tiles[from], self.node_tiles[to]);
            if a != b {
                self.tile_links[a].push(b);
            }
        }
        Ok(admitted)
    }
    /// Encodes an immutable navigation manifest. Its digest is a content pin,
    /// not a signature or permission to execute an authored transition.
    pub fn cooked(&self) -> Result<(Vec<u8>, [u8; 32]), String> {
        let mut portable = self.clone();
        portable.config.instance = 0;
        let mut bytes = b"VNT1".to_vec();
        bytes.extend(serde_json::to_vec(&portable).map_err(|e| e.to_string())?);
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("Navigation cooked byte budget exceeded".into());
        }
        let digest = Sha256::digest(&bytes).into();
        Ok((bytes, digest))
    }
    /// Loads bytes pinned by a trusted content manifest and binds the local graph
    /// to the requested instance. Checks graph bounds before a route can read it.
    pub fn from_cooked(bytes: &[u8], expected: [u8; 32], instance: u64) -> Result<Self, String> {
        if bytes.len() > 64 * 1024 * 1024
            || !bytes.starts_with(b"VNT1")
            || <[u8; 32]>::from(Sha256::digest(bytes)) != expected
        {
            return Err("Navigation cooked identity or byte budget mismatch".into());
        }
        let graph: CookedGraph = serde_json::from_slice(&bytes[4..])
            .map_err(|e| format!("Invalid cooked navigation: {e}"))?;
        let mut nav = Self {
            config: graph.config,
            stats: graph.stats,
            cells: graph.cells,
            nodes: graph.nodes,
            tiles: graph.tiles,
            node_tiles: graph.node_tiles,
            tile_links: graph.tile_links,
        };
        nav.config.character.validate()?;
        if nav.tiles.is_empty()
            || nav.tiles.len() > 4096
            || nav.nodes.len() > 1_048_576
            || nav.cells.len() > 1_048_576
            || nav.node_tiles.len() != nav.nodes.len()
            || nav.tile_links.len() != nav.tiles.len()
            || nav.node_tiles.iter().any(|t| *t >= nav.tiles.len())
            || nav.nodes.iter().any(|n| {
                !n.feet.is_finite()
                    || n.feet.abs().max_element() > 1_000_000.
                    || n.links.len() > 4096
                    || n.links.iter().any(|i| *i >= nav.nodes.len())
            })
            || nav
                .tile_links
                .iter()
                .any(|links| links.len() > 4096 || links.iter().any(|i| *i >= nav.tiles.len()))
            || nav
                .cells
                .values()
                .any(|links| links.len() > 128 || links.iter().any(|i| *i >= nav.nodes.len()))
            || nav
                .tiles
                .iter()
                .any(|t| !t.min.is_finite() || !t.max.is_finite() || !t.min.cmplt(t.max).all())
            || !nav.config.min.is_finite()
            || !nav.config.max.is_finite()
            || !nav.config.min.cmplt(nav.config.max).all()
            || nav.config.min.abs().max_element() > 1_000_000.
            || nav.config.max.abs().max_element() > 1_000_000.
            || !nav.config.cell.is_finite()
            || !(0.1..=2.).contains(&nav.config.cell)
            || nav.config.layers == 0
        {
            return Err("Invalid cooked navigation graph bounds".into());
        }
        nav.config.instance = instance;
        Ok(nav)
    }
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }
    /// Prefers a coarse directed tile corridor. Fine search can leave it when
    /// height layers or dynamic obstructions invalidate a coarse connection.
    pub(super) fn tile_corridor(
        &self,
        source: usize,
        target: usize,
        work: &mut Work,
    ) -> Result<Option<Vec<bool>>, String> {
        let (from, to) = (self.node_tiles[source], self.node_tiles[target]);
        let mut parent = vec![usize::MAX; self.tiles.len()];
        let mut queue = VecDeque::from([from]);
        parent[from] = from;
        while let Some(tile) = queue.pop_front() {
            work.charge(1 + self.tile_links[tile].len())?;
            if tile == to {
                let mut corridor = vec![false; self.tiles.len()];
                let mut cursor = to;
                loop {
                    corridor[cursor] = true;
                    if cursor == from {
                        break;
                    }
                    cursor = parent[cursor];
                }
                return Ok(Some(corridor));
            }
            for next in &self.tile_links[tile] {
                if parent[*next] == usize::MAX {
                    parent[*next] = tile;
                    queue.push_back(*next);
                }
            }
        }
        Ok(None)
    }
    /// Returns the local immutable tiles whose capsule clearance overlaps a change.
    pub fn affected_tiles(&self, min: DVec3, max: DVec3) -> Result<Vec<usize>, String> {
        if !min.is_finite() || !max.is_finite() || !min.cmple(max).all() {
            return Err("Invalid navigation invalidation bounds".into());
        }
        let margin = DVec3::new(
            self.config.character.radius + self.config.cell,
            self.config.character.height + self.config.character.step_height,
            self.config.character.radius + self.config.cell,
        );
        Ok(self
            .tiles
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                (t.min.cmple(max + margin).all() && t.max.cmpge(min - margin).all()).then_some(i)
            })
            .collect())
    }
    /// Includes every tile touched by a swept route segment, including start and goal.
    pub fn route_tiles(&self, start: DVec3, points: &[DVec3]) -> Result<Vec<usize>, String> {
        let mut touched = BTreeSet::new();
        let mut previous = start;
        for point in points {
            touched.extend(self.affected_tiles(previous.min(*point), previous.max(*point))?);
            previous = *point;
        }
        if points.is_empty() {
            touched.extend(self.affected_tiles(start, start)?);
        }
        Ok(touched.into_iter().collect())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CookedGraph {
    config: Config,
    stats: CompileStats,
    #[serde(with = "cell_pairs")]
    cells: BTreeMap<(i32, i32), Vec<usize>>,
    nodes: Vec<Cell>,
    tiles: Vec<Tile>,
    node_tiles: Vec<usize>,
    tile_links: Vec<Vec<usize>>,
}
fn tile_digest(
    snapshot: &crate::queries::SceneSnapshot,
    nav: &Navigation,
) -> Result<[u8; 32], String> {
    let mut local = snapshot.clone();
    local.instance = 0;
    let min = nav.config.min - DVec3::splat(nav.config.character.height + nav.config.cell);
    let max = nav.config.max + DVec3::splat(nav.config.character.height + nav.config.cell);
    local.colliders.retain_mut(|shape| {
        let (lo, hi) = match &shape.geometry {
            crate::queries::GeometrySnapshot::Box { min, max } => {
                let corners = (0..8).map(|i| {
                    shape.pose.point(DVec3::new(
                        if i & 1 == 0 { min.x } else { max.x },
                        if i & 2 == 0 { min.y } else { max.y },
                        if i & 4 == 0 { min.z } else { max.z },
                    ))
                });
                corners.fold(
                    (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
                    |(a, b), p| (a.min(p), b.max(p)),
                )
            }
            crate::queries::GeometrySnapshot::Triangles { triangles } => triangles
                .iter()
                .flat_map(|t| t.0)
                .map(|p| shape.pose.point(p))
                .fold(
                    (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
                    |(a, b), p| (a.min(p), b.max(p)),
                ),
            crate::queries::GeometrySnapshot::Capsule { a, b, radius } => {
                let a = shape.pose.point(*a);
                let b = shape.pose.point(*b);
                (
                    a.min(b) - DVec3::splat(*radius),
                    a.max(b) + DVec3::splat(*radius),
                )
            }
        };
        shape.key.life.instance = 0;
        shape.usage == Usage::Blocking
            && shape.layers & nav.config.layers != 0
            && lo.cmple(max).all()
            && hi.cmpge(min).all()
    });
    let mut config = nav.config;
    config.instance = 0;
    config.work_budget = 0;
    let mut hash = Sha256::new();
    hash.update(b"verse.navigation.tile.v1\0");
    hash.update(serde_json::to_vec(&(config, local)).map_err(|e| e.to_string())?);
    for node in &nav.nodes {
        for component in node.feet.to_array() {
            hash.update(component.to_le_bytes());
        }
        hash.update((node.links.len() as u64).to_le_bytes());
        for link in &node.links {
            hash.update((*link as u64).to_le_bytes());
        }
    }
    Ok(hash.finalize().into())
}
/// Epoch stamps avoid clearing every span when a route uses a small part of a zone.
#[derive(Clone, Default)]
pub struct SearchScratch {
    distance: Vec<f64>,
    pub(super) parent: Vec<usize>,
    stamps: Vec<u64>,
    epoch: u64,
    pub(super) queue: BinaryHeap<Visit>,
}
impl SearchScratch {
    pub(super) fn begin(&mut self, nodes: usize) {
        self.distance.resize(nodes, f64::INFINITY);
        self.parent.resize(nodes, usize::MAX);
        self.stamps.resize(nodes, 0);
        self.epoch = self.epoch.wrapping_add(1);
        if self.epoch == 0 {
            self.stamps.fill(0);
            self.epoch = 1;
        }
        self.queue.clear();
    }
    pub(super) fn cost(&self, node: usize) -> f64 {
        if self.stamps[node] == self.epoch {
            self.distance[node]
        } else {
            f64::INFINITY
        }
    }
    pub(super) fn set(&mut self, node: usize, cost: f64, parent: usize) {
        self.stamps[node] = self.epoch;
        self.distance[node] = cost;
        self.parent[node] = parent;
    }
    pub fn span_capacity(&self) -> usize {
        self.distance.capacity()
    }
}
/// FIFO actor admission reserves worst-case work before a search starts.
/// Failed searches consume the reservation; deferred searches consume no collision work.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scheduler {
    pending: VecDeque<Life>,
    #[serde(with = "life_pairs")]
    seen: BTreeMap<Life, u64>,
    tick: Option<u64>,
    plans: usize,
    work: usize,
    nodes: usize,
}
impl Default for Scheduler {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            seen: BTreeMap::new(),
            tick: None,
            plans: 0,
            work: 0,
            nodes: 0,
        }
    }
}
impl Scheduler {
    pub fn begin_tick(&mut self, tick: u64) {
        if self.tick != Some(tick) {
            self.pending.retain(|life| {
                self.seen
                    .get(life)
                    .is_some_and(|last| tick.saturating_sub(*last) <= 2)
            });
            self.seen.retain(|life, _| self.pending.contains(life));
            self.tick = Some(tick);
            self.plans = 0;
            self.work = 0;
            self.nodes = 0;
        }
    }
    pub fn validate(&self, instance: u64) -> Result<(), String> {
        if self.pending.len() > 256
            || self.seen.len() != self.pending.len()
            || self.seen.keys().any(|l| !self.pending.contains(l))
            || self
                .seen
                .values()
                .any(|last| self.tick.is_none_or(|tick| *last > tick))
            || self.plans > 4
            || self.work > 2_000_000
            || self.nodes > 65_536
            || self
                .pending
                .iter()
                .any(|l| l.instance != instance || l.entity == 0)
            || self.pending.iter().copied().collect::<BTreeSet<_>>().len() != self.pending.len()
        {
            return Err("Invalid navigation scheduler checkpoint".into());
        }
        Ok(())
    }
    pub fn request(&mut self, life: Life, budget: Budget) -> Result<bool, String> {
        if budget.nodes == 0
            || budget.nodes > 65_536
            || budget.work_units == 0
            || budget.work_units > 2_000_000
            || life.entity == 0
            || self.tick.is_none()
        {
            return Err("Invalid scheduled navigation request".into());
        }
        self.pending
            .retain(|l| l.entity != life.entity || *l == life);
        if !self.pending.contains(&life) {
            if self.pending.len() == 256 {
                return Err("Navigation pending route budget exceeded".into());
            }
            self.pending.push_back(life);
        }
        self.seen.retain(|l, _| self.pending.contains(l));
        self.seen.insert(life, self.tick.unwrap_or(0));
        if self.pending.front() != Some(&life)
            || self.plans == 4
            || self.work + budget.work_units > 2_000_000
            || self.nodes + budget.nodes > 65_536
        {
            return Ok(false);
        }
        self.pending.pop_front();
        self.seen.remove(&life);
        self.plans += 1;
        self.work += budget.work_units;
        self.nodes += budget.nodes;
        Ok(true)
    }
    pub fn cancel(&mut self, life: Life) {
        self.pending.retain(|l| *l != life);
        self.seen.remove(&life);
    }
    pub fn retain(&mut self, mut alive: impl FnMut(Life) -> bool) {
        self.pending.retain(|l| alive(*l));
        self.seen.retain(|l, _| self.pending.contains(l));
    }
    pub fn used(&self) -> (usize, usize, usize) {
        (self.plans, self.nodes, self.work)
    }
    pub fn tick(&self) -> Option<u64> {
        self.tick
    }
    pub fn pending_lives(&self) -> impl Iterator<Item = Life> + '_ {
        self.pending.iter().copied()
    }
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CrowdAgent {
    pub life: Life,
    pub feet: DVec3,
    pub radius: f64,
}
/// One-meter buckets and at most 16 selected actors bound each steering decision.
#[derive(Clone, Default)]
pub struct Crowd {
    cells: BTreeMap<(i32, i32), Vec<CrowdAgent>>,
}
impl Crowd {
    pub fn rebuild(&mut self, agents: impl IntoIterator<Item = CrowdAgent>) -> Result<(), String> {
        self.cells.clear();
        let mut count = 0;
        let mut lives = BTreeSet::new();
        for a in agents {
            count += 1;
            if count > 4096
                || !a.feet.is_finite()
                || a.feet.abs().max_element() > 1_000_000.
                || !a.radius.is_finite()
                || !(0.01..=0.5).contains(&a.radius)
                || !lives.insert(a.life)
            {
                self.cells.clear();
                return Err("Invalid crowd agent or crowd budget exceeded".into());
            }
            self.cells
                .entry((a.feet.x.floor() as i32, a.feet.z.floor() as i32))
                .or_default()
                .push(a);
        }
        for cell in self.cells.values_mut() {
            cell.sort_by_key(|a| a.life);
        }
        Ok(())
    }
    /// Lower actor identities retain priority. The caller admits the final velocity
    /// through collision; this advisory never changes a pose or grants a teleport.
    pub fn steer(&self, actor: CrowdAgent, desired: DVec3) -> Result<DVec3, String> {
        if !actor.feet.is_finite()
            || actor.feet.abs().max_element() > 1_000_000.
            || !desired.is_finite()
            || desired.length() > 1.000001
            || !actor.radius.is_finite()
            || !(0.01..=0.5).contains(&actor.radius)
        {
            return Err("Invalid crowd steering request".into());
        }
        let key = (actor.feet.x.floor() as i32, actor.feet.z.floor() as i32);
        let mut near = vec![];
        let reach = (desired.length() + actor.radius + 0.55).ceil() as i32;
        for x in key.0 - reach..=key.0 + reach {
            for z in key.1 - reach..=key.1 + reach {
                for a in self.cells.get(&(x, z)).into_iter().flatten() {
                    if a.life == actor.life
                        || a.life.instance != actor.life.instance
                        || (a.feet.y - actor.feet.y).abs() > 0.6
                    {
                        continue;
                    }
                    near.push((actor.feet.distance_squared(a.feet), *a));
                }
            }
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.life.cmp(&b.1.life)));
        near.truncate(16);
        // Sample a fixed set of advisory velocities. Actor priority widens the
        // clearance of yielding actors; every candidate still avoids physical overlap.
        for angle in [
            0_f64,
            0.5235987755982988,
            -0.5235987755982988,
            1.0471975511965976,
            -1.0471975511965976,
            1.5707963267948966,
            -1.5707963267948966,
        ] {
            let (sin, cos) = angle.sin_cos();
            let motion = DVec3::new(
                desired.x * cos - desired.z * sin,
                desired.y,
                desired.x * sin + desired.z * cos,
            );
            let safe = near.iter().all(|(_, other)| {
                let delta =
                    DVec3::new(other.feet.x - actor.feet.x, 0., other.feet.z - actor.feet.z);
                let projected = if motion.length_squared() > 1e-12 {
                    (delta.dot(motion) / motion.length_squared()).clamp(0., 1.)
                } else {
                    0.
                };
                let closest = (delta - motion * projected).length();
                let clearance = actor.radius
                    + other.radius
                    + if actor.life > other.life { 0.03 } else { 0.001 };
                closest >= clearance
                    || (delta.length() < clearance
                        && delta.dot(motion) <= 0.
                        && (delta - motion).length() > delta.length())
            });
            if safe {
                return Ok(motion);
            }
        }
        Ok(DVec3::ZERO)
    }
}
#[cfg(test)]
mod tests;

pub(super) mod cell_pairs {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        cells: &BTreeMap<(i32, i32), Vec<usize>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        cells.iter().collect::<Vec<_>>().serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<(i32, i32), Vec<usize>>, D::Error> {
        let pairs = Vec::<((i32, i32), Vec<usize>)>::deserialize(deserializer)?;
        let count = pairs.len();
        let map: BTreeMap<_, _> = pairs.into_iter().collect();
        if map.len() != count {
            return Err(serde::de::Error::custom("Duplicate navigation cell"));
        }
        Ok(map)
    }
}

mod life_pairs {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        seen: &BTreeMap<Life, u64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        seen.iter().collect::<Vec<_>>().serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<Life, u64>, D::Error> {
        let pairs = Vec::<(Life, u64)>::deserialize(deserializer)?;
        let count = pairs.len();
        let map: BTreeMap<_, _> = pairs.into_iter().collect();
        if map.len() != count {
            return Err(serde::de::Error::custom(
                "Duplicate navigation scheduler life",
            ));
        }
        Ok(map)
    }
}
