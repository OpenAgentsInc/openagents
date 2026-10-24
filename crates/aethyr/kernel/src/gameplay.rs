//! Bounded local destruction and the Druid's runtime abilities.
//!
//! Spell parameters and piece health follow OpenAgents Verse at revision
//! 83fe8b7f6b3cd437ae9b7af213cf9f04875dd793: Everglade's demolition meteor.rs,
//! kit.rs, and town.rs, and the Grove's kit.rs and cast.rs. This preview uses
//! its own cell support graph and physics.

use super::Collider;
use super::static_collision::{RayHit, StaticGeometry};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::Arc;

type V3 = [f32; 3];
pub const MAX_OBJECTS: usize = 65_536;
const MAX_DEBRIS: usize = 256;
const MAX_EFFECTS: usize = 64;
const MAX_PENDING: usize = 64;
const MAX_DAMAGE_EVENTS: usize = 64;
const MAX_DIRECT_DAMAGE: usize = 256;
const MAX_CASCADE_PER_TICK: usize = 256;
const MAX_RUBBLE: usize = 1024;
const CASTLE_WAVE_TICKS: u64 = 24;
const CELL: f32 = 8.0;
const DT: f32 = 1.0 / 60.0;
fn add(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] - b[i])
}
fn mul(a: V3, t: f32) -> V3 {
    a.map(|v| v * t)
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
fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}
fn unit(a: V3) -> V3 {
    let n = length(a);
    if n > 1e-6 {
        mul(a, 1. / n)
    } else {
        [0., 1., 0.]
    }
}
fn anchor_default() -> bool {
    true
}
fn finite(a: V3) -> bool {
    a.into_iter().all(|v| v.is_finite() && v.abs() <= 20_000.)
}
fn grid(v: f32) -> i32 {
    (v / CELL).floor() as i32
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Object {
    pub id: u32,
    pub source_id: u32,
    pub min: V3,
    pub max: V3,
    #[serde(default, alias = "health")]
    pub hp: Option<f32>,
    #[serde(default = "anchor_default")]
    pub anchored: bool,
    #[serde(default)]
    pub supports: Vec<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetGroups {
    #[serde(default)]
    castle: Vec<u32>,
}
impl Object {
    fn validate(&self) -> Result<(), &'static str> {
        if self.id == u32::MAX
            || self.source_id == u32::MAX
            || !finite(self.min)
            || !finite(self.max)
            || (0..3).any(|i| self.max[i] < self.min[i] || self.max[i] - self.min[i] > 512.)
            || length(sub(self.max, self.min)) < 0.001
            || self.supports.len() > 16
            || self.supports.contains(&self.id)
            || self
                .hp
                .is_some_and(|v| !v.is_finite() || v <= 0. || v > 100_000.)
        {
            return Err("A breakable piece has invalid bounds, health, or supports.");
        }
        let unique: HashSet<_> = self.supports.iter().collect();
        if unique.len() != self.supports.len() {
            return Err("A breakable piece repeats one of its supports.");
        }
        Ok(())
    }
    fn health(&self) -> f32 {
        self.hp.unwrap_or_else(|| {
            if sub(self.max, self.min).into_iter().fold(0.0, f32::max) < 1.6 {
                12.
            } else {
                27.
            }
        })
    }
}

#[derive(Clone, Default)]
pub struct Builder {
    objects: Vec<Object>,
    ids: HashSet<u32>,
    pub seed: u32,
}
impl Builder {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            ..Self::default()
        }
    }
    pub fn append(&mut self, objects: Vec<Object>) -> Result<(), &'static str> {
        if self.objects.len() + objects.len() > MAX_OBJECTS {
            return Err("This town has too many breakable pieces.");
        }
        let mut chunk = HashSet::new();
        for object in &objects {
            object.validate()?;
            if self.ids.contains(&object.id) || !chunk.insert(object.id) {
                return Err("A breakable piece repeats an identifier.");
            }
        }
        self.ids.extend(chunk);
        self.objects.extend(objects);
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.objects.len()
    }
    pub fn finish(&self, groups: TargetGroups) -> Result<Gameplay, &'static str> {
        Gameplay::new(self.objects.clone(), self.seed, groups)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Ability {
    MeteorShower,
    LightningBolt,
    Thunderbolt,
    DestroyCastle,
}
impl Ability {
    fn cast_ticks(self) -> u64 {
        match self {
            Self::MeteorShower => 150,
            Self::Thunderbolt => 72,
            Self::LightningBolt | Self::DestroyCastle => 0,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aim {
    pub point: V3,
    pub normal: V3,
    pub distance: f32,
    pub node_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enemy_id: Option<u32>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CastReceipt {
    pub cast_id: String,
    pub id: u32,
    pub ability: Ability,
    pub damage: u32,
    pub aim: Aim,
    pub duplicate: bool,
}
#[derive(Clone)]
struct CachedCast {
    fingerprint: Vec<u8>,
    fragment_masks: Vec<FragmentMask>,
    castle_fragments: Option<Vec<[u32; 2]>>,
    receipt: CastReceipt,
}
#[derive(Clone)]
struct PreparedCast {
    id: String,
    fingerprint: Vec<u8>,
    version: u64,
    aim: Option<Aim>,
    required: Arc<[u32]>,
    prepared_tick: u64,
    time_sensitive: bool,
    resumable: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FragmentMask {
    pub node_id: u32,
    pub mask: [u32; 2],
}
fn mask_bits([low, high]: [u32; 2]) -> u64 {
    u64::from(low) | (u64::from(high) << 32)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub id: u32,
    pub kind: &'static str,
    pub origin: V3,
    pub target: V3,
    pub age: f32,
    pub life: f32,
    pub radius: f32,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Debris {
    pub id: u32,
    pub node_id: u32,
    pub source_id: u32,
    pub chunk: u8,
    pub subdivisions: u8,
    pub source_min: V3,
    pub source_max: V3,
    pub position: V3,
    pub rotation: V3,
    pub half_extents: V3,
    pub velocity: V3,
    pub age: f32,
    pub life: f32,
    #[serde(skip)]
    angular: V3,
    #[serde(skip)]
    supported: bool,
    #[serde(skip)]
    quiet: u32,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DamageEvent {
    id: u32,
    node_id: u32,
    amount: f32,
    health: f32,
    max_health: f32,
    destroyed: bool,
    point: V3,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Casting {
    id: u32,
    ability: Ability,
    target: V3,
    age: f32,
    life: f32,
}
#[derive(Clone)]
struct Impact {
    at: u64,
    point: V3,
    normal: V3,
    radius: f32,
    damage: u32,
    throw: f32,
    castle_target: Option<usize>,
}

#[derive(Clone, Default)]
struct Castle {
    cells: BTreeSet<usize>,
    order: Vec<usize>,
    alive: BTreeSet<usize>,
    fragments: Option<Vec<[u32; 2]>>,
    target: Option<V3>,
    active: bool,
    cancelled: bool,
    waves: u32,
    impacts: u32,
    elapsed: u64,
    next_wave: u64,
    cursor: usize,
    pending: Vec<Impact>,
}

#[derive(Clone)]
struct Settling {
    at: u64,
    resolved: bool,
    body: Debris,
}
#[derive(Clone)]
enum Scheduled {
    Visual { at: u64, effect: Effect },
    Impact(Impact),
}
impl Scheduled {
    fn at(&self) -> u64 {
        match self {
            Self::Visual { at, .. } => *at,
            Self::Impact(i) => i.at,
        }
    }
}
#[derive(Clone)]
struct Cell {
    object: Object,
    hp: f32,
}

#[derive(Clone, Default)]
pub struct Gameplay {
    #[cfg(test)]
    pub(crate) forecast_starts: std::cell::Cell<usize>,
    pub enemies: super::enemies::Enemies,
    enemy_impacts: VecDeque<super::enemies::Impact>,
    prepared: VecDeque<PreparedCast>,
    cells: Vec<Cell>,
    index: HashMap<u32, usize>,
    reverse: Vec<Vec<usize>>,
    spatial: HashMap<(i32, i32), Vec<usize>>,
    broad: Vec<usize>,
    pub destroyed: BTreeSet<u32>,
    pub version: u64,
    tick: u64,
    seed: u32,
    initial_seed: u32,
    next_id: u32,
    cast_count: u32,
    debris: VecDeque<Debris>,
    rubble: Vec<Collider>,
    permanent: VecDeque<Debris>,
    settling: BTreeMap<(u32, u32, u8), Settling>,
    rubble_dirty: bool,
    next_landing: u64,
    fragment_masks: HashMap<usize, u64>,
    effects: VecDeque<Effect>,
    damage: VecDeque<DamageEvent>,
    pending: Vec<Scheduled>,
    active_cast: Option<Casting>,
    recent: VecDeque<CachedCast>,
    cascade: VecDeque<(usize, bool)>,
    queued: HashSet<usize>,
    last_impact: Option<V3>,
    last_cast: Option<CastReceipt>,
    castle: Castle,
}
impl Gameplay {
    fn new(
        mut objects: Vec<Object>,
        seed: u32,
        groups: TargetGroups,
    ) -> Result<Self, &'static str> {
        if objects.len() > MAX_OBJECTS {
            return Err("This town has too many breakable pieces.");
        }
        objects.sort_by_key(|o| o.id);
        let mut game = Self {
            seed: seed.max(1),
            initial_seed: seed.max(1),
            next_id: 1,
            ..Self::default()
        };
        for object in objects {
            object.validate()?;
            let at = game.cells.len();
            if game.index.insert(object.id, at).is_some() {
                return Err("A breakable piece repeats an identifier.");
            }
            let x0 = grid(object.min[0]);
            let x1 = grid(object.max[0]);
            let z0 = grid(object.min[2]);
            let z1 = grid(object.max[2]);
            if (x1 - x0 + 1) as u64 * (z1 - z0 + 1) as u64 > 64 {
                game.broad.push(at);
            } else {
                for x in x0..=x1 {
                    for z in z0..=z1 {
                        game.spatial.entry((x, z)).or_default().push(at);
                    }
                }
            }
            game.cells.push(Cell {
                hp: object.health(),
                object,
            });
        }
        if game.broad.len() > 4096 {
            return Err("Too many breakable pieces cover a large area.");
        }
        game.reverse = vec![vec![]; game.cells.len()];
        for (i, cell) in game.cells.iter().enumerate() {
            for support in &cell.object.supports {
                let Some(&below) = game.index.get(support) else {
                    return Err("A breakable piece refers to a missing support.");
                };
                game.reverse[below].push(i);
            }
        }
        if groups.castle.len() > MAX_OBJECTS {
            return Err("The castle has too many breakable pieces.");
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for id in groups.castle {
            let Some(&i) = game.index.get(&id) else {
                return Err("The castle refers to a missing breakable piece.");
            };
            if !game.castle.cells.insert(i) {
                return Err("The castle repeats a breakable piece.");
            }
            game.castle.order.push(i);
            for axis in 0..3 {
                min[axis] = min[axis].min(game.cells[i].object.min[axis]);
                max[axis] = max[axis].max(game.cells[i].object.max[axis]);
            }
        }
        if !game.castle.cells.is_empty() {
            game.castle.alive = game.castle.cells.clone();
            game.castle.target = Some(mul(add(min, max), 0.5));
        }
        Ok(game)
    }
    pub fn object_count(&self) -> usize {
        self.cells.len()
    }
    fn id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }
    fn random(&mut self) -> f32 {
        let mut x = self.seed.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / 16_777_216.
    }
    fn dice(&mut self, count: usize, sides: u32) -> u32 {
        (0..count)
            .map(|_| 1 + (self.random() * sides as f32) as u32)
            .sum()
    }
    fn effect(&mut self, kind: &'static str, origin: V3, target: V3, life: f32, radius: f32) {
        let id = self.id();
        self.effects.push_back(Effect {
            id,
            kind,
            origin,
            target,
            age: 0.,
            life,
            radius,
        });
        while self.effects.len() > MAX_EFFECTS {
            self.effects.pop_front();
        }
    }
    fn candidates(&self, point: V3, radius: f32) -> Vec<usize> {
        let mut found: HashSet<usize> = self.broad.iter().copied().collect();
        for x in grid(point[0] - radius)..=grid(point[0] + radius) {
            for z in grid(point[2] - radius)..=grid(point[2] + radius) {
                if let Some(v) = self.spatial.get(&(x, z)) {
                    found.extend(v.iter().copied());
                }
            }
        }
        let mut result: Vec<_> = found
            .into_iter()
            .filter(|&i| self.cells[i].hp > 0.)
            .collect();
        result.sort_unstable();
        result
    }
    pub fn aim(
        &self,
        origin: V3,
        direction: V3,
        max_distance: f32,
        static_hit: Option<RayHit>,
        ground: f32,
        geometry: &StaticGeometry,
    ) -> Option<Aim> {
        let ray = unit(direction);
        let mut best = static_hit.map(|hit| Aim {
            point: hit.point,
            normal: hit.normal,
            distance: hit.distance,
            node_id: hit.owner,
            enemy_id: None,
        });
        // Bounds also admit decorative pieces that have no walking collider.
        let end = add(origin, mul(ray, max_distance));
        let center = mul(add(origin, end), 0.5);
        for i in self.candidates(center, max_distance * 0.5 + 1.) {
            let cell = &self.cells[i];
            if geometry.has_owner(cell.object.id) {
                continue;
            }
            if let Some((distance, normal)) = ray_box(origin, ray, cell.object.min, cell.object.max)
            {
                if distance > 0.01
                    && distance <= max_distance
                    && best.as_ref().is_none_or(|hit| distance < hit.distance)
                {
                    best = Some(Aim {
                        point: add(origin, mul(ray, distance)),
                        normal,
                        distance,
                        node_id: Some(cell.object.id),
                        enemy_id: None,
                    });
                }
            }
        }
        if ray[1] < -1e-5 {
            let distance = (ground - origin[1]) / ray[1];
            if distance >= 0.
                && distance <= max_distance
                && best.as_ref().is_none_or(|hit| distance < hit.distance)
            {
                best = Some(Aim {
                    point: add(origin, mul(ray, distance)),
                    normal: [0., 1., 0.],
                    distance,
                    node_id: None,
                    enemy_id: None,
                });
            }
        }
        best
    }
    pub fn cast(
        &mut self,
        cast_id: String,
        ability: Ability,
        origin: V3,
        direction: V3,
        aim: Option<Aim>,
        expected_version: Option<u64>,
        geometry: &StaticGeometry,
        castle_fragments: Option<Vec<[u32; 2]>>,
        fragment_masks: Option<Vec<FragmentMask>>,
    ) -> Result<CastReceipt, &'static str> {
        let bound = self
            .prepared
            .iter()
            .find(|entry| entry.id == cast_id)
            .cloned();
        let duplicate = self
            .recent
            .iter()
            .any(|entry| entry.receipt.cast_id == cast_id);
        let aim = if duplicate {
            aim
        } else if let Some(bound) = bound {
            let fingerprint = serde_json::to_vec(&(ability, origin, direction))
                .map_err(|_| "The spell could not be read.")?;
            if bound.fingerprint != fingerprint {
                return Err("Use a new cast identifier for a different spell.");
            }
            if bound.version != self.version || expected_version.is_some_and(|v| v != bound.version)
            {
                return Err("The town changed. Aim and cast again.");
            }
            bound.aim
        } else {
            if expected_version.is_some() {
                return Err("Prepare this spell before casting.");
            }
            aim
        };
        let result = self.cast_inner(
            cast_id,
            ability,
            origin,
            direction,
            aim,
            expected_version,
            geometry,
            castle_fragments,
            fragment_masks,
            false,
        );
        if let Ok(receipt) = &result {
            self.prepared.retain(|entry| entry.id != receipt.cast_id);
        }
        result
    }
    fn cast_inner(
        &mut self,
        cast_id: String,
        ability: Ability,
        origin: V3,
        direction: V3,
        aim: Option<Aim>,
        expected_version: Option<u64>,
        geometry: &StaticGeometry,
        castle_fragments: Option<Vec<[u32; 2]>>,
        fragment_masks: Option<Vec<FragmentMask>>,
        forecast: bool,
    ) -> Result<CastReceipt, &'static str> {
        if cast_id.is_empty()
            || cast_id.len() > 128
            || !finite(origin)
            || !finite(direction)
            || length(direction) < 0.01
        {
            return Err("Choose a valid spell and aiming direction.");
        }
        if ability != Ability::DestroyCastle && castle_fragments.is_some() {
            return Err("Castle fragments belong to the castle barrage.");
        }
        let previous = self.recent.iter().find(|c| c.receipt.cast_id == cast_id);
        let mut provided = fragment_masks
            .unwrap_or_else(|| previous.map_or_else(Vec::new, |cast| cast.fragment_masks.clone()));
        if provided.len() > MAX_OBJECTS {
            return Err("Too many pieces were prepared for this spell.");
        }
        provided.sort_by_key(|entry| entry.node_id);
        let mut supplied = HashMap::with_capacity(provided.len());
        for entry in &provided {
            let Some(&index) = self.index.get(&entry.node_id) else {
                return Err("A prepared fragment refers to a missing piece.");
            };
            let bits = mask_bits(entry.mask);
            if supplied.insert(index, bits).is_some() {
                return Err("A piece repeats its prepared fragments.");
            }
        }
        let masks = if ability == Ability::DestroyCastle {
            castle_fragments
                .or_else(|| previous.and_then(|cast| cast.castle_fragments.clone()))
                .or_else(|| self.castle.fragments.clone())
                .or_else(|| {
                    self.castle
                        .order
                        .iter()
                        .map(|i| {
                            supplied
                                .get(i)
                                .or_else(|| self.fragment_masks.get(i))
                                .map(|bits| [*bits as u32, (*bits >> 32) as u32])
                        })
                        .collect::<Option<Vec<_>>>()
                })
        } else {
            None
        };
        if masks
            .as_ref()
            .is_some_and(|m| m.len() != self.castle.order.len() || !m.iter().any(|n| *n != [0, 0]))
        {
            return Err("Prepare the castle's visible fragments before casting.");
        }
        if let Some(masks) = &masks {
            for (&index, mask) in self.castle.order.iter().zip(masks) {
                let bits = mask_bits(*mask);
                if supplied.insert(index, bits).is_some_and(|old| old != bits) {
                    return Err("The prepared fragments do not match this world.");
                }
            }
        }
        let fingerprint = serde_json::to_vec(&(ability, origin, direction, &masks, &provided))
            .map_err(|_| "The spell could not be read.")?;
        if let Some(seen) = previous {
            if seen.fingerprint != fingerprint {
                return Err("Use a new cast identifier for a different spell.");
            }
            let mut receipt = seen.receipt.clone();
            receipt.duplicate = true;
            return Ok(receipt);
        }
        if expected_version.is_some_and(|v| v != self.version) {
            return Err("The town changed. Aim and cast again.");
        }
        if self.active_cast.is_some() {
            return Err("Finish casting before starting another spell.");
        }
        if self.castle.active {
            return Err("Stop the castle barrage before casting another spell.");
        }
        if ability == Ability::DestroyCastle && self.castle.alive.is_empty() {
            return Err(if self.castle.cells.is_empty() {
                "This world has no castle to destroy."
            } else {
                "The castle is already rubble."
            });
        }
        if self.pending.len() + 14 > MAX_PENDING {
            return Err("Wait for the falling meteors to land before casting again.");
        }
        if !forecast && ability == Ability::DestroyCastle && masks.is_none() {
            return Err("Prepare the castle's visible fragments before casting.");
        }
        let range = if ability == Ability::LightningBolt {
            30.48
        } else {
            80.
        };
        let aim = match aim {
            _ if ability == Ability::DestroyCastle => {
                let point = self.castle.target.expect("validated castle group");
                Aim {
                    point,
                    normal: [0., 1., 0.],
                    distance: length(sub(point, origin)),
                    node_id: None,
                    enemy_id: None,
                }
            }
            Some(a) => a,
            None if ability == Ability::LightningBolt => Aim {
                point: add(origin, mul(unit(direction), range)),
                normal: mul(unit(direction), -1.),
                distance: range,
                node_id: None,
                enemy_id: None,
            },
            None => return Err("Aim at the town or ground before casting."),
        };
        if !forecast {
            for (&index, &bits) in &supplied {
                if self
                    .fragment_masks
                    .get(&index)
                    .is_some_and(|known| *known != bits)
                {
                    return Err("The prepared fragments do not match this world.");
                }
            }
            let refresh = self.prepared_needs_refresh(&cast_id);
            if refresh
                && self
                    .prepared
                    .iter()
                    .any(|entry| entry.id == cast_id && entry.resumable)
            {
                return Err("The spell's timing changed. Prepare it again.");
            }
            if refresh || !self.prepared.iter().any(|entry| entry.id == cast_id) {
                self.prepare(
                    cast_id.clone(),
                    ability,
                    origin,
                    direction,
                    Some(aim.clone()),
                    geometry,
                )?;
            }
            let bound = self
                .prepared
                .iter()
                .find(|entry| entry.id == cast_id)
                .ok_or("Prepare this spell before casting.")?;
            let fingerprint = serde_json::to_vec(&(ability, origin, direction))
                .map_err(|_| "The spell could not be read.")?;
            if bound.version != self.version || bound.fingerprint != fingerprint {
                return Err("The town changed. Aim and cast again.");
            }
            if bound.required.iter().any(|id| {
                let index = self.index[id];
                !supplied.contains_key(&index) && !self.fragment_masks.contains_key(&index)
            }) {
                return Err("Prepare the visible fragments before casting.");
            }
        }
        self.fragment_masks.extend(supplied);
        if let Some(masks) = &masks {
            self.castle.fragments = Some(masks.clone());
        }
        let id = self.id();
        let damage = match ability {
            Ability::MeteorShower => self.dice(40, 6),
            Ability::LightningBolt => self.dice(8, 6),
            Ability::Thunderbolt => self.dice(20, 10),
            Ability::DestroyCastle => 0,
        };
        let receipt = CastReceipt {
            cast_id,
            id,
            ability,
            damage,
            aim: aim.clone(),
            duplicate: false,
        };
        match ability {
            Ability::LightningBolt => {
                self.effect("beam", origin, aim.point, 0.45, 0.762);
                if aim.distance < 30.48 - 0.001 || aim.node_id.is_some() {
                    self.impact(Impact {
                        at: self.tick,
                        point: aim.point,
                        normal: aim.normal,
                        radius: 1.8,
                        damage,
                        throw: 14.,
                        castle_target: None,
                    });
                }
            }
            Ability::Thunderbolt => {
                let cloud = add(aim.point, [0., 60., 0.]);
                let effect_id = self.id();
                self.pending.push(Scheduled::Visual {
                    at: self.tick + 72,
                    effect: Effect {
                        id: effect_id,
                        kind: "lightning",
                        origin: cloud,
                        target: aim.point,
                        age: 0.,
                        life: 0.59,
                        radius: 3.4,
                    },
                });
                self.pending.push(Scheduled::Impact(Impact {
                    at: self.tick + 78,
                    point: aim.point,
                    normal: aim.normal,
                    radius: 3.4,
                    damage,
                    throw: 14.,
                    castle_target: None,
                }));
            }
            Ability::MeteorShower => {
                let spread = if aim.normal[1].abs() > 0.65 { 6. } else { 3. };
                for meteor in 0..6 {
                    let angle = self.random() * std::f32::consts::TAU;
                    let radius = self.random().sqrt() * spread;
                    let tangent = if aim.normal[1].abs() > 0.65 {
                        [angle.cos() * radius, 0., angle.sin() * radius]
                    } else {
                        let right = unit(cross(aim.normal, [0., 1., 0.]));
                        let up = unit(cross(right, aim.normal));
                        add(
                            mul(right, angle.cos() * radius),
                            mul(up, angle.sin() * radius),
                        )
                    };
                    let scattered = add(aim.point, tangent);
                    let probe = if aim.normal[1].abs() > 0.65 {
                        [0., 1., 0.]
                    } else {
                        aim.normal
                    };
                    // Snap locally so another street cannot replace the aimed-at surface.
                    let snap = if aim.normal[1].abs() > 0.65 { 6. } else { 3. };
                    let surface = geometry.raycast(
                        add(scattered, mul(probe, snap)),
                        mul(probe, -1.),
                        snap * 2.,
                    );
                    let target = surface.as_ref().map_or(scattered, |hit| hit.point);
                    let normal = surface.as_ref().map_or(aim.normal, |hit| hit.normal);
                    let from = add(target, [-12., 80., -8.]);
                    let launch = self.tick + 150 + ((meteor as f32 * 0.13) / DT).round() as u64;
                    let effect_id = self.id();
                    self.pending.push(Scheduled::Visual {
                        at: launch,
                        effect: Effect {
                            id: effect_id,
                            kind: "meteor",
                            origin: from,
                            target,
                            age: 0.,
                            life: 1.,
                            radius: 1.,
                        },
                    });
                    self.pending.push(Scheduled::Impact(Impact {
                        at: launch + 60,
                        point: target,
                        normal,
                        radius: 4.,
                        damage,
                        throw: 17.,
                        castle_target: None,
                    }));
                }
            }
            Ability::DestroyCastle => {
                self.castle.active = true;
                self.castle.cancelled = false;
                self.castle.next_wave = self.tick + 1;
            }
        }
        if ability.cast_ticks() > 0 {
            self.active_cast = Some(Casting {
                id,
                ability,
                target: aim.point,
                age: 0.,
                life: ability.cast_ticks() as f32 * DT,
            });
        }
        self.cast_count = self.cast_count.saturating_add(1);
        self.version = self.version.saturating_add(1);
        self.last_cast = Some(receipt.clone());
        self.recent.push_back(CachedCast {
            fingerprint,
            fragment_masks: provided,
            castle_fragments: masks,
            receipt: receipt.clone(),
        });
        while self.recent.len() > 64 {
            self.recent.pop_front();
        }
        Ok(receipt)
    }
    fn impact(&mut self, impact: Impact) {
        self.enemy_impacts.push_back(super::enemies::Impact {
            point: impact.point,
            normal: impact.normal,
            radius: impact.radius,
            damage: impact.damage,
        });
        while self.enemy_impacts.len() > 64 {
            self.enemy_impacts.pop_front();
        }
        self.rubble.clear();
        self.last_impact = Some(impact.point);
        self.effect(
            "impact",
            impact.point,
            add(impact.point, impact.normal),
            0.9,
            if impact.castle_target.is_some() {
                20.
            } else {
                impact.radius
            },
        );
        let mut damaged = 0;
        let mut candidates = self.candidates(impact.point, impact.radius);
        if let Some(target) = impact.castle_target {
            // A dense cluster must not starve the cell this strike targets.
            candidates.retain(|i| *i != target && self.castle.cells.contains(i));
            if self.cells[target].hp > 0. {
                candidates.insert(0, target);
            }
        }
        for i in candidates {
            let object = &self.cells[i].object;
            let closest = std::array::from_fn(|axis| {
                impact.point[axis].clamp(object.min[axis], object.max[axis])
            });
            let distance = length(sub(impact.point, closest));
            let amount = (impact.damage as f32 * (1. - distance / impact.radius))
                .round()
                .max(0.);
            if amount <= 0. {
                continue;
            }
            let cell = &mut self.cells[i];
            cell.hp = (cell.hp - amount).max(0.);
            let (node_id, health, max_health) = (cell.object.id, cell.hp, cell.object.health());
            let event_id = self.id();
            self.damage.push_back(DamageEvent {
                id: event_id,
                node_id,
                amount,
                health,
                max_health,
                destroyed: health <= 0.,
                point: impact.point,
            });
            while self.damage.len() > MAX_DAMAGE_EVENTS {
                self.damage.pop_front();
            }
            if health <= 0. {
                self.break_cell(
                    i,
                    impact.point,
                    impact.throw,
                    impact.castle_target.is_some(),
                );
            }
            damaged += 1;
            if damaged
                >= if impact.castle_target.is_some() {
                    64
                } else {
                    MAX_DIRECT_DAMAGE
                }
            {
                break;
            }
        }
        self.find_unsupported(impact.castle_target.is_some());
        for body in &mut self.debris {
            if length(sub(body.position, impact.point)) <= 3. {
                body.quiet = 0;
                body.supported = false;
                body.velocity = add(
                    body.velocity,
                    mul(unit(sub(body.position, impact.point)), impact.throw),
                );
            }
        }
        self.version = self.version.saturating_add(1);
    }
    fn find_unsupported(&mut self, _fine: bool) {
        let mut supported = vec![false; self.cells.len()];
        let mut queue = VecDeque::new();
        for (i, cell) in self.cells.iter().enumerate() {
            if cell.hp > 0. && cell.object.anchored {
                supported[i] = true;
                queue.push_back(i);
            }
        }
        while let Some(i) = queue.pop_front() {
            for &above in &self.reverse[i] {
                if self.cells[above].hp > 0. && !supported[above] {
                    supported[above] = true;
                    queue.push_back(above);
                }
            }
        }
        for (i, is_supported) in supported.into_iter().enumerate() {
            if !is_supported && self.cells[i].hp > 0. && self.queued.insert(i) {
                self.cascade.push_back((i, true));
            }
        }
    }
    fn drain_cascade(&mut self, budget: usize, point: V3, throw: f32) {
        for _ in 0..budget {
            let Some((i, fine)) = self.cascade.pop_front() else {
                break;
            };
            self.queued.remove(&i);
            if self.cells[i].hp > 0. {
                self.cells[i].hp = 0.;
                self.break_cell(i, point, throw, fine);
                self.version = self.version.saturating_add(1);
            }
        }
    }
    fn break_cell(&mut self, index: usize, impact: V3, throw: f32, _fine: bool) {
        let object = self.cells[index].object.clone();
        if !self.destroyed.insert(object.id) {
            return;
        }
        self.castle.alive.remove(&index);
        let subdivisions = 4;
        let extent = sub(object.max, object.min);
        let half = mul(extent, 0.5 / subdivisions as f32);
        let mask = self.fragment_masks.get(&index).copied().unwrap_or(u64::MAX);
        for chunk in 0u8..64 {
            if mask & (1u64 << chunk) == 0 {
                continue;
            }
            let position = std::array::from_fn(|axis| {
                let digit = 2 * ((chunk >> axis) & 1) + ((chunk >> (axis + 3)) & 1);
                object.min[axis] + (digit as f32 + 0.5) * extent[axis] * 0.25
            });
            let direction = unit(add(sub(position, impact), [0., 1., 0.]));
            let velocity = add(
                mul(direction, throw * (0.6 + self.random() * 0.4)),
                [
                    (self.random() - 0.5) * 3.,
                    2. + self.random() * 4.,
                    (self.random() - 0.5) * 3.,
                ],
            );
            let angular = std::array::from_fn(|_| (self.random() - 0.5) * 12.);
            let id = self.id();
            self.debris.push_back(Debris {
                id,
                node_id: object.id,
                source_id: object.source_id,
                chunk,
                subdivisions,
                source_min: object.min,
                source_max: object.max,
                position,
                rotation: [0.; 3],
                half_extents: half,
                velocity,
                age: 0.,
                life: 20.,
                angular,
                supported: false,
                quiet: 0,
            });
        }
        while self.debris.len() > MAX_DEBRIS {
            if let Some(body) = self.debris.pop_front() {
                self.reserve_rubble(body);
            }
        }
    }
    fn reserve_rubble(&mut self, body: Debris) {
        if body.subdivisions != 4 || !finite(body.position) {
            return;
        }
        let mut priority = body.node_id.wrapping_mul(0x9e3779b9)
            ^ (body.chunk as u32).wrapping_mul(0x85ebca6b)
            ^ self.initial_seed;
        priority = (priority ^ (priority >> 16)).wrapping_mul(0x7feb352d);
        priority = (priority ^ (priority >> 15)).wrapping_mul(0x846ca68b);
        priority ^= priority >> 16;
        let key = (priority, body.node_id, body.chunk);
        if self.settling.len() >= MAX_RUBBLE
            && self
                .settling
                .last_key_value()
                .is_some_and(|(last, _)| key >= *last)
        {
            return;
        }
        self.settling.insert(
            key,
            Settling {
                at: self.tick,
                resolved: false,
                body,
            },
        );
        while self.settling.len() > MAX_RUBBLE {
            self.settling.pop_last();
        }
        self.rubble_dirty = true;
    }
    /// Retired fragments settle on actual permanent scenery with bounded queries.
    fn settle_rubble(&mut self, geometry: &StaticGeometry, ground: f32) {
        let pending: Vec<_> = self
            .settling
            .iter()
            .filter(|(_, s)| !s.resolved)
            .take(16)
            .map(|(key, _)| *key)
            .collect();
        for key in pending {
            let mut item = self.settling.remove(&key).expect("selected rubble");
            let body = &mut item.body;
            let fallback = ground;
            let initial_time =
                landing_seconds((body.position[1] - fallback).max(0.), body.velocity[1]);
            let travel = (1. - (-0.7 * initial_time).exp()) / 0.7;
            let mut horizontal = [body.velocity[0] * travel, 0., body.velocity[2] * travel];
            let distance = length(horizontal);
            if distance > 12. {
                horizontal = mul(horizontal, 12. / distance);
            }
            let original: V3 = std::array::from_fn(|axis| {
                let digit = 2 * ((body.chunk >> axis) & 1) + ((body.chunk >> (axis + 3)) & 1);
                body.source_min[axis]
                    + (digit as f32 + 0.5) * (body.source_max[axis] - body.source_min[axis]) * 0.25
            });
            let mut offset = sub(add(body.position, horizontal), original);
            offset[1] = 0.;
            let spread = length(offset);
            if spread > 48. {
                offset = mul(offset, 48. / spread);
            }
            let x = original[0] + offset[0];
            let z = original[2] + offset[2];
            let floor = geometry
                .raycast_permanent(
                    [x, body.position[1].max(fallback) + 512., z],
                    [0., -1., 0.],
                    1024.,
                )
                .map_or(fallback, |hit| hit.point[1]);
            let seconds = landing_seconds((body.position[1] - floor).max(0.), body.velocity[1]);
            body.rotation = add(
                body.rotation,
                mul(body.angular, (1. - (-0.5 * seconds).exp()) / 0.5),
            );
            body.position = [
                x,
                floor + rotated_extents(body.rotation, body.half_extents)[1],
                z,
            ];
            body.velocity = [0.; 3];
            body.angular = [0.; 3];
            body.age = 0.;
            body.supported = true;
            body.quiet = 90;
            item.at = item.at.saturating_add((seconds * 60.).ceil() as u64);
            item.resolved = true;
            self.settling.insert(key, item);
            self.rubble_dirty = true;
        }
        if !self.rubble_dirty && self.tick < self.next_landing {
            return;
        }
        if !self.rubble_dirty && self.next_landing == u64::MAX {
            return;
        }
        self.permanent.clear();
        self.next_landing = u64::MAX;
        for item in self.settling.values() {
            if !item.resolved {
                continue;
            }
            if item.at <= self.tick {
                self.permanent.push_back(item.body.clone());
            } else {
                self.next_landing = self.next_landing.min(item.at);
            }
        }
        self.rubble_dirty = false;
    }
    fn launch_castle_wave(&mut self) {
        self.castle.next_wave = self.tick + CASTLE_WAVE_TICKS;
        let mut reserved: HashSet<_> = self
            .castle
            .pending
            .iter()
            .filter_map(|p| p.castle_target)
            .collect();
        let mut launched = false;
        for strike in 0..6 {
            if self.castle.pending.len() + self.pending.len() >= MAX_PENDING {
                break;
            }
            let target = self
                .castle
                .alive
                .range(self.castle.cursor..)
                .chain(self.castle.alive.range(..self.castle.cursor))
                .find(|i| !reserved.contains(i))
                .copied();
            let Some(target) = target else { break };
            reserved.insert(target);
            self.castle.cursor = target + 1;
            let object = &self.cells[target].object;
            let point = mul(add(object.min, object.max), 0.5);
            let meteor = strike < 4;
            let from = add(
                point,
                if meteor {
                    [-24., 120., -16.]
                } else {
                    [0., 120., 0.]
                },
            );
            self.effect(
                if meteor { "meteor" } else { "lightning" },
                from,
                point,
                if meteor { 1. } else { 0.6 },
                if meteor { 24. } else { 32. },
            );
            let damage = if meteor {
                self.dice(40, 6)
            } else {
                self.dice(20, 10)
            };
            self.castle.pending.push(Impact {
                at: self.tick + if meteor { 60 } else { 6 },
                point,
                normal: [0., 1., 0.],
                radius: if meteor { 4. } else { 3.4 },
                damage,
                throw: if meteor { 17. } else { 14. },
                castle_target: Some(target),
            });
            launched = true;
        }
        if launched {
            self.castle.waves = self.castle.waves.saturating_add(1);
        }
    }
    /// Stops new waves; strikes already visible in the sky still land.
    pub fn cancel_cast(&mut self) {
        self.prepared.clear();
        if self.castle.active {
            self.castle.active = false;
            self.castle.cancelled = true;
            self.version = self.version.saturating_add(1);
        }
    }
    pub fn advance_events(&mut self) {
        let destroyed_before = self.destroyed.len();
        self.tick = self.tick.saturating_add(1);
        if self.castle.active || !self.castle.pending.is_empty() {
            self.castle.elapsed = self.castle.elapsed.saturating_add(1);
        }
        for effect in &mut self.effects {
            effect.age += DT;
        }
        self.effects.retain(|e| e.age < e.life);
        if let Some(cast) = &mut self.active_cast {
            cast.age += DT;
            if cast.age + 0.0001 >= cast.life {
                self.active_cast = None;
            }
        }
        let scheduled = std::mem::take(&mut self.pending);
        for event in scheduled {
            if event.at() > self.tick {
                self.pending.push(event);
                continue;
            }
            match event {
                Scheduled::Visual { effect, .. } => {
                    self.effects.push_back(effect);
                    while self.effects.len() > MAX_EFFECTS {
                        self.effects.pop_front();
                    }
                }
                Scheduled::Impact(impact) => self.impact(impact),
            }
        }
        for impact in std::mem::take(&mut self.castle.pending) {
            if impact.at > self.tick {
                self.castle.pending.push(impact);
            } else {
                self.castle.impacts = self.castle.impacts.saturating_add(1);
                self.impact(impact);
            }
        }
        if !self.cascade.is_empty() {
            self.drain_cascade(
                MAX_CASCADE_PER_TICK.saturating_sub(self.destroyed.len() - destroyed_before),
                self.last_impact.unwrap_or([0.; 3]),
                5.,
            );
        }
        if self.castle.alive.is_empty() {
            self.castle.active = false;
        } else if self.castle.active && self.tick >= self.castle.next_wave {
            self.launch_castle_wave();
        }
    }
    pub fn advance_debris(&mut self, geometry: &StaticGeometry, ground: f32, player: Option<V3>) {
        for body in &mut self.debris {
            body.age += DT;
            if body.age >= body.life {
                continue;
            }
            if body.supported && body.quiet >= 90 {
                continue;
            }
            body.velocity[1] -= 9.81 * DT;
            let speed = length(body.velocity);
            let cap = if body.age < 1.6 { 26. } else { 10. };
            if speed > cap {
                body.velocity = mul(body.velocity, cap / speed);
            }
            let old = body.position;
            let next = add(old, mul(body.velocity, DT));
            let radius = body
                .half_extents
                .into_iter()
                .fold(0.0, f32::max)
                .clamp(0.04, 2.);
            body.supported = false;
            if let Some((fraction, normal)) = geometry.sweep_contact(old, next, radius) {
                if fraction > 0.001 || body.age > 0.2 {
                    body.position = add(old, mul(sub(next, old), (fraction - 0.001).max(0.)));
                    let along = dot(body.velocity, normal);
                    if along < 0. {
                        body.velocity = sub(body.velocity, mul(normal, along * 1.25));
                    }
                    if normal[1] >= 0.65 {
                        body.supported = true;
                        body.velocity[0] *= 0.8;
                        body.velocity[2] *= 0.8;
                    }
                } else {
                    body.position = next;
                }
            } else {
                body.position = next;
            }
            let floor = ground;
            if body.position[1] < floor + radius {
                body.position[1] = floor + radius;
                body.supported = true;
                if body.velocity[1] < 0. {
                    body.velocity[1] *= -0.25;
                }
                body.velocity[0] *= 0.8;
                body.velocity[2] *= 0.8;
            }
            if body.supported && length(body.velocity) < 0.6 {
                body.quiet += 1;
                body.velocity = mul(body.velocity, 0.8);
                body.angular = mul(body.angular, 0.8);
            } else {
                body.quiet = 0;
            }
            body.rotation = add(body.rotation, mul(body.angular, DT));
            body.angular = mul(body.angular, if body.supported { 0.98 } else { 0.999 });
            if body.supported && body.quiet == 90 {
                body.position[1] += rotated_extents(body.rotation, body.half_extents)[1] - radius;
            }
        }
        let mut dynamic = VecDeque::new();
        for body in std::mem::take(&mut self.debris) {
            if !finite(body.position) {
                continue;
            }
            if body.subdivisions == 4
                && (body.age >= body.life || (body.supported && body.quiet >= 90))
            {
                self.reserve_rubble(body);
            } else if body.age < body.life {
                dynamic.push_back(body);
            }
        }
        self.debris = dynamic;
        self.settle_rubble(geometry, ground);
        self.refresh_rubble(player);
    }
    pub fn rubble_colliders(&self) -> &[Collider] {
        &self.rubble
    }
    /// A resting fragment becomes solid only after the player is clear of it.
    pub fn refresh_rubble(&mut self, player: Option<V3>) {
        self.rubble.clear();
        for body in self.debris.iter().chain(&self.permanent) {
            if !body.supported || body.quiet < 90 {
                continue;
            }
            let half = rotated_extents(body.rotation, body.half_extents);
            let collider = Collider {
                id: String::new(),
                min: sub(body.position, half),
                max: add(body.position, half),
            };
            if player
                .is_none_or(|position| super::can_occupy(position, std::slice::from_ref(&collider)))
            {
                self.rubble.push(collider);
            }
        }
    }
    pub fn start_forecast(
        &self,
        cast_id: String,
        ability: Ability,
        origin: V3,
        direction: V3,
        aim: Option<Aim>,
    ) -> Result<Forecast, &'static str> {
        if cast_id.is_empty()
            || cast_id.len() > 128
            || !finite(origin)
            || !finite(direction)
            || length(direction) < 0.01
        {
            return Err("Choose a valid spell and aiming direction.");
        }
        #[cfg(test)]
        self.forecast_starts.set(self.forecast_starts.get() + 1);
        let fingerprint = serde_json::to_vec(&(ability, origin, direction))
            .map_err(|_| "The spell could not be read.")?;
        let previous = self.prepared.iter().find(|entry| entry.id == cast_id);
        let aim = if let Some(previous) = previous {
            if previous.fingerprint != fingerprint {
                return Err("Use a new cast identifier for a different spell.");
            }
            if previous.version != self.version {
                return Err("The town changed. Aim and cast again.");
            }
            previous.aim.clone()
        } else {
            aim
        };
        Ok(Forecast {
            candidate: Box::new(self.clone()),
            binding: PreparedCast {
                id: cast_id,
                fingerprint,
                version: self.version,
                aim,
                required: Arc::from([]),
                prepared_tick: self.tick,
                // Without pre-existing impacts/collapse, live age/physics/enemy updates
                // consume no spell RNG/IDs or health; all new events shift together.
                time_sensitive: self
                    .pending
                    .iter()
                    .any(|event| matches!(event, Scheduled::Impact(_)))
                    || !self.castle.pending.is_empty()
                    || !self.cascade.is_empty(),
                resumable: false,
            },
            ability,
            origin,
            direction,
            receipt: None,
            phase: ForecastPhase::Cast,
            ticks: 0,
            complete: false,
            previously_prepared: previous.is_some(),
        })
    }
    pub fn accept_forecast(
        &mut self,
        forecast: &Forecast,
        resumable: bool,
    ) -> Result<(), &'static str> {
        if !forecast.complete || forecast.binding.version != self.version {
            return Err("The town changed. Aim and cast again.");
        }
        if let Some(previous) = self
            .prepared
            .iter_mut()
            .find(|entry| entry.id == forecast.binding.id)
        {
            if previous.fingerprint != forecast.binding.fingerprint
                || previous.version != forecast.binding.version
            {
                return Err("Use a new cast identifier for a different spell.");
            }
            previous.required = forecast.binding.required.clone();
            previous.prepared_tick = forecast.binding.prepared_tick;
            previous.time_sensitive = forecast.binding.time_sensitive;
            previous.resumable = resumable;
        } else {
            let mut binding = forecast.binding.clone();
            binding.resumable = resumable;
            self.prepared.push_back(binding);
            while self.prepared.len() > 16 {
                self.prepared.pop_front();
            }
        }
        Ok(())
    }
    pub fn prepared_needs_refresh(&self, cast_id: &str) -> bool {
        self.prepared
            .iter()
            .find(|entry| entry.id == cast_id)
            .is_some_and(|entry| entry.time_sensitive && entry.prepared_tick != self.tick)
    }
    pub fn is_prepared(&self, cast_id: &str) -> bool {
        self.prepared.iter().any(|entry| entry.id == cast_id)
    }
    pub fn discard_prepared(&mut self, cast_id: &str, version: u64) {
        self.prepared
            .retain(|entry| entry.id != cast_id || entry.version != version);
    }
    pub fn prepare(
        &mut self,
        cast_id: String,
        ability: Ability,
        origin: V3,
        direction: V3,
        aim: Option<Aim>,
        geometry: &StaticGeometry,
    ) -> Result<serde_json::Value, &'static str> {
        let mut forecast = self.start_forecast(cast_id, ability, origin, direction, aim)?;
        loop {
            if let Some(plan) = forecast.advance(geometry)? {
                self.accept_forecast(&forecast, false)?;
                return Ok(plan);
            }
        }
    }
    #[cfg(test)]
    pub(crate) fn reference_plan(
        &self,
        cast_id: String,
        ability: Ability,
        origin: V3,
        direction: V3,
        aim: Option<Aim>,
        geometry: &StaticGeometry,
    ) -> Result<serde_json::Value, &'static str> {
        // Preserve the original synchronous forecast as an independent ordering oracle.
        let aim = self
            .prepared
            .iter()
            .find(|entry| entry.id == cast_id)
            .map_or(aim, |entry| entry.aim.clone());
        let mut candidate = self.clone();
        if ability == Ability::DestroyCastle && candidate.castle.fragments.is_none() {
            candidate.castle.fragments = Some(vec![[u32::MAX; 2]; candidate.castle.order.len()]);
        }
        let receipt = candidate.cast_inner(
            cast_id, ability, origin, direction, aim, None, geometry, None, None, true,
        )?;
        if ability == Ability::DestroyCastle {
            for &index in &candidate.castle.cells {
                candidate.cells[index].hp = 0.;
                candidate.destroyed.insert(candidate.cells[index].object.id);
            }
            candidate.castle.alive.clear();
            candidate.castle.active = false;
            candidate.castle.pending.clear();
            candidate.find_unsupported(true);
        }
        for _ in 0..600 {
            if candidate.pending.is_empty()
                && candidate.castle.pending.is_empty()
                && candidate.cascade.is_empty()
                && candidate.active_cast.is_none()
            {
                break;
            }
            candidate.advance_events();
        }
        if !candidate.pending.is_empty()
            || !candidate.castle.pending.is_empty()
            || !candidate.cascade.is_empty()
        {
            return Err("This spell would change too much of the town at once.");
        }
        let sources: BTreeSet<_> = candidate
            .destroyed
            .iter()
            .filter_map(|id| {
                candidate
                    .index
                    .get(id)
                    .map(|&index| candidate.cells[index].object.source_id)
            })
            .collect();
        Ok(
            serde_json::json!({"version":self.version,"prospectiveDestroyed":candidate.destroyed,"sourceIds":sources,"cast":receipt,"aim":receipt.aim}),
        )
    }
    pub fn reset(&mut self) {
        self.enemies.reset();
        self.enemy_impacts.clear();
        self.prepared.clear();
        for cell in &mut self.cells {
            cell.hp = cell.object.health();
        }
        self.destroyed.clear();
        self.debris.clear();
        self.rubble.clear();
        self.effects.clear();
        self.damage.clear();
        self.pending.clear();
        self.active_cast = None;
        self.recent.clear();
        self.cascade.clear();
        self.queued.clear();
        self.last_impact = None;
        self.last_cast = None;
        self.castle.alive = self.castle.cells.clone();
        self.castle.active = false;
        self.castle.cancelled = false;
        self.castle.waves = 0;
        self.castle.impacts = 0;
        self.castle.elapsed = 0;
        self.castle.next_wave = 0;
        self.castle.cursor = 0;
        self.castle.pending.clear();
        self.permanent.clear();
        self.settling.clear();
        self.rubble_dirty = false;
        self.next_landing = u64::MAX;
        self.seed = self.initial_seed.max(1);
        self.tick = 0;
        self.cast_count = 0;
        self.next_id = 1;
        self.version = self.version.saturating_add(1);
    }
    pub fn snapshot(&self) -> serde_json::Value {
        let castle_rubble_count = self
            .permanent
            .iter()
            .filter(|body| {
                self.index
                    .get(&body.node_id)
                    .is_some_and(|i| self.castle.cells.contains(i))
            })
            .count();
        let castle = serde_json::json!({"total":self.castle.cells.len(),"remaining":self.castle.alive.len(),
            "destroyed":self.castle.cells.len()-self.castle.alive.len(),"active":self.castle.active,
            "complete":!self.castle.cells.is_empty()&&self.castle.alive.is_empty(),"cancelled":self.castle.cancelled,
            "waves":self.castle.waves,"impacts":self.castle.impacts,"elapsed":self.castle.elapsed as f64/60.,
            "target":self.castle.target,"pendingImpacts":self.castle.pending.len(),"rubbleCount":castle_rubble_count});
        serde_json::json!({"version":self.version,"tick":self.tick,"objectCount":self.object_count(),"castCount":self.cast_count,"enemies":self.enemies.snapshot(),
            "destroyed":self.destroyed,"destroyedCount":self.destroyed.len(),"debris":self.debris,"rubble":self.permanent,"restingCount":self.rubble.len(),"effects":self.effects,"damage":self.damage,
            "activeCast":self.active_cast,"pendingImpacts":self.pending.iter().filter(|p|matches!(p,Scheduled::Impact(_))).count()+self.castle.pending.len(),
            "pendingCollapse":self.cascade.len(),"lastImpact":self.last_impact,"lastCast":self.last_cast,
            "cooldowns":{"meteor_shower":0,"lightning_bolt":0,"thunderbolt":0,"destroy_castle":0},"castle":castle})
    }
    pub fn take_enemy_impacts(&mut self) -> VecDeque<super::enemies::Impact> {
        std::mem::take(&mut self.enemy_impacts)
    }
}

#[derive(Clone, Copy)]
enum ForecastPhase {
    Cast,
    Castle,
    Events,
}

/// An invisible copy uses the same spell and event operations as live gameplay.
#[derive(Clone)]
pub struct Forecast {
    candidate: Box<Gameplay>,
    binding: PreparedCast,
    ability: Ability,
    origin: V3,
    direction: V3,
    receipt: Option<CastReceipt>,
    phase: ForecastPhase,
    ticks: u16,
    complete: bool,
    previously_prepared: bool,
}
impl Forecast {
    pub fn phase(&self) -> &'static str {
        match self.phase {
            ForecastPhase::Cast => "cast",
            ForecastPhase::Castle => "castle",
            ForecastPhase::Events => "events",
        }
    }
    pub fn ticks(&self) -> u16 {
        self.ticks
    }
    pub fn previously_prepared(&self) -> bool {
        self.previously_prepared
    }
    fn pending(&self) -> bool {
        !self.candidate.pending.is_empty()
            || !self.candidate.castle.pending.is_empty()
            || !self.candidate.cascade.is_empty()
            || self.candidate.active_cast.is_some()
    }
    /// One call performs setup, castle reservation, or at most one event tick.
    pub fn advance(
        &mut self,
        geometry: &StaticGeometry,
    ) -> Result<Option<serde_json::Value>, &'static str> {
        if self.complete {
            return Err("This spell is already prepared.");
        }
        match self.phase {
            ForecastPhase::Cast => {
                if self.ability == Ability::DestroyCastle
                    && self.candidate.castle.fragments.is_none()
                {
                    self.candidate.castle.fragments =
                        Some(vec![[u32::MAX; 2]; self.candidate.castle.order.len()]);
                }
                self.receipt = Some(self.candidate.cast_inner(
                    self.binding.id.clone(),
                    self.ability,
                    self.origin,
                    self.direction,
                    self.binding.aim.clone(),
                    None,
                    geometry,
                    None,
                    None,
                    true,
                )?);
                self.phase = if self.ability == Ability::DestroyCastle {
                    ForecastPhase::Castle
                } else {
                    ForecastPhase::Events
                };
            }
            ForecastPhase::Castle => {
                for &index in &self.candidate.castle.cells {
                    self.candidate.cells[index].hp = 0.;
                    self.candidate
                        .destroyed
                        .insert(self.candidate.cells[index].object.id);
                }
                self.candidate.castle.alive.clear();
                self.candidate.castle.active = false;
                self.candidate.castle.pending.clear();
                self.candidate.find_unsupported(true);
                self.phase = ForecastPhase::Events;
            }
            ForecastPhase::Events => {
                if self.pending() && self.ticks < 600 {
                    self.candidate.advance_events();
                    self.ticks += 1;
                }
            }
        }
        if !matches!(self.phase, ForecastPhase::Events) || (self.pending() && self.ticks < 600) {
            return Ok(None);
        }
        if !self.candidate.pending.is_empty()
            || !self.candidate.castle.pending.is_empty()
            || !self.candidate.cascade.is_empty()
        {
            return Err("This spell would change too much of the town at once.");
        }
        let sources: BTreeSet<_> = self
            .candidate
            .destroyed
            .iter()
            .filter_map(|id| {
                self.candidate
                    .index
                    .get(id)
                    .map(|&index| self.candidate.cells[index].object.source_id)
            })
            .collect();
        self.binding.required = self
            .candidate
            .destroyed
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .into();
        self.complete = true;
        let receipt = self
            .receipt
            .as_ref()
            .expect("forecast cast precedes completion");
        Ok(Some(
            serde_json::json!({"version":self.binding.version,"prospectiveDestroyed":self.binding.required.as_ref(),"sourceIds":sources,"cast":receipt,"aim":receipt.aim}),
        ))
    }
}

fn landing_seconds(height: f32, vertical_speed: f32) -> f32 {
    let upward = vertical_speed.max(0.);
    let rise_time = upward / 9.81;
    let distance = height + upward * upward / (2. * 9.81);
    let down_speed = (-vertical_speed).clamp(0., 10.);
    let accelerating = (100. - down_speed * down_speed) / (2. * 9.81);
    let fall = if distance <= accelerating {
        ((down_speed * down_speed + 2. * 9.81 * distance).sqrt() - down_speed) / 9.81
    } else {
        (10. - down_speed) / 9.81 + (distance - accelerating) / 10.
    };
    (rise_time + fall).clamp(0., 20.)
}

fn rotated_extents([x, y, z]: V3, [hx, hy, hz]: V3) -> V3 {
    let (sx, cx) = x.sin_cos();
    let (sy, cy) = y.sin_cos();
    let (sz, cz) = z.sin_cos();
    [
        (cz * cy).abs() * hx
            + (cz * sy * sx - sz * cx).abs() * hy
            + (cz * sy * cx + sz * sx).abs() * hz,
        (sz * cy).abs() * hx
            + (sz * sy * sx + cz * cx).abs() * hy
            + (sz * sy * cx - cz * sx).abs() * hz,
        sy.abs() * hx + (cy * sx).abs() * hy + (cy * cx).abs() * hz,
    ]
}

pub(crate) fn ray_box(origin: V3, ray: V3, min: V3, max: V3) -> Option<(f32, V3)> {
    let mut near = 0.0f32;
    let mut far = f32::INFINITY;
    let mut normal = [0.; 3];
    for axis in 0..3 {
        if ray[axis].abs() < 1e-8 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
        } else {
            let a = (min[axis] - origin[axis]) / ray[axis];
            let b = (max[axis] - origin[axis]) / ray[axis];
            let enter = a.min(b);
            if enter > near {
                near = enter;
                normal = [0.; 3];
                normal[axis] = if ray[axis] > 0. { -1. } else { 1. };
            }
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    (far >= 0.).then_some((near, normal))
}
