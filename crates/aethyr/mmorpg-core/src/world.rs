use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const SOURCE_COMMIT: &str = "547884332ca650abe96264f7230702d36481b9bc";
pub const PROTOCOL: &str = "aethyrforge.mmorpg.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorldConfig {
    pub release: String,
    pub seed: u64,
    pub world_scale: f32,
    pub spawn: V3,
    pub yaw: f32,
    pub spawners: Option<Vec<SpawnSpec>>,
}
impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            release: "quick-mmorpg-v1".into(),
            seed: 1,
            world_scale: 1. / 6.,
            spawn: [6., 0., 20.],
            yaw: 0.,
            spawners: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnSpec {
    pub position: V3,
    pub class: Class,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Player,
    Npc,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActorSnapshot {
    pub id: u32,
    pub name: String,
    pub class: Class,
    pub kind: Kind,
    pub position: V3,
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub stats: Stats,
    pub state: Motion,
    pub action_time: f32,
    pub animation_time: f32,
    pub alive: bool,
    pub weapon: Option<Item>,
    pub attacks: u32,
    pub hits: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prediction: Option<Prediction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<[Option<Item>; 32]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory_version: Option<u64>,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: u64,
    pub tick: u64,
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    #[serde(rename = "type")]
    pub message_type: String,
    pub request_id: String,
    pub ok: bool,
    pub duplicate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory_version: Option<u64>,
}
#[derive(Clone)]
struct QueuedEvent {
    event: Event,
    position: V3,
    range: f32,
}
#[derive(Clone)]
struct Cached {
    id: String,
    fingerprint: String,
    receipt: Receipt,
}
#[derive(Clone)]
struct Entity {
    id: u32,
    name: String,
    class: Class,
    kind: Kind,
    motor: Prediction,
    stats: Stats,
    inventory: Inventory,
    input: Controls,
    ack_input: u32,
    input_sequence: u32,
    attack_latched: bool,
    dance_latched: bool,
    input_tick: u64,
    target: Option<u32>,
    scan: u16,
    spawner: Option<usize>,
    corpse: u32,
    attacks: u32,
    hits: u32,
    last_chat: Option<u64>,
    commands: VecDeque<Cached>,
}
impl Entity {
    fn snapshot(&self, local: bool) -> ActorSnapshot {
        ActorSnapshot {
            id: self.id,
            name: self.name.clone(),
            class: self.class,
            kind: self.kind,
            position: self.motor.position,
            yaw: self.motor.yaw,
            health: self.stats.health,
            max_health: self.stats.max_health,
            stats: self.stats.clone(),
            state: self.motor.state,
            action_time: self.motor.state_ticks as f32 * DT,
            animation_time: self.motor.state_ticks as f32 * DT,
            alive: self.stats.health > 0.,
            weapon: self.inventory.slots[24],
            attacks: self.attacks,
            hits: self.hits,
            prediction: local.then(|| self.motor.clone()),
            inventory: local.then_some(self.inventory.slots),
            inventory_version: local.then_some(self.inventory.version),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub tick: u64,
    pub player: ActorSnapshot,
    pub actors: Vec<ActorSnapshot>,
    pub despawned: Vec<u32>,
    pub events: Vec<Event>,
    pub ack_input: u32,
    pub last_event_id: u64,
    pub events_truncated: bool,
    pub population: Population,
}
#[derive(Clone, Debug, Serialize)]
pub struct Population {
    pub players: usize,
    pub npcs: usize,
    pub spawners: usize,
}

#[derive(Clone)]
struct Spawner {
    spec: SpawnSpec,
    actor: Option<u32>,
}
pub struct World {
    pub config: WorldConfig,
    pub tick: u64,
    entities: BTreeMap<u32, Entity>,
    spawners: Vec<Spawner>,
    next_id: u32,
    next_event: u64,
    events: VecDeque<QueuedEvent>,
}
fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:".contains(&c))
}
fn separated(a: V3, ah: f32, b: V3, bh: f32) -> bool {
    a[1] + ah <= b[1] || b[1] + bh <= a[1] || horizontal(a, b) >= 0.601
}
struct Occupancy<'a, T> {
    surface: &'a T,
    others: &'a [(u32, V3, f32, bool)],
    id: u32,
}
impl<T: Surface> Surface for Occupancy<'_, T> {
    fn height(&self, x: f32, z: f32) -> f32 {
        self.surface.height(x, z)
    }
    fn support(&self, p: V3, y: f32) -> f32 {
        self.surface.support(p, y)
    }
    fn step_support(&self, p: V3, y: f32) -> f32 {
        self.surface.step_support(p, y)
    }
    fn can_occupy(&self, p: V3, h: f32) -> bool {
        self.surface.can_occupy(p, h)
            && self.others.iter().all(|(id, pos, height, alive)| {
                *id == self.id || !alive || separated(p, h, *pos, *height)
            })
    }
    fn slide_down(&self, p: V3, h: f32) -> Option<V3> {
        self.surface.slide_down(p, h)
    }
    fn clear(&self, a: V3, b: V3) -> bool {
        self.surface.clear(a, b)
    }
}

impl World {
    pub fn new(config: WorldConfig, surface: &impl Surface) -> Result<Self, &'static str> {
        if (config.world_scale - 1. / 6.).abs() > 0.0000001
            || !config.world_scale.is_finite()
            || !finite_position(config.spawn)
            || !config.yaw.is_finite()
            || config.release.is_empty()
            || config.release.len() > 128
        {
            return Err("Choose a valid world and starting position.");
        }
        let specs = if let Some(specs) = config.spawners.clone() {
            specs
        } else {
            // The original uses Math.random. A recorded seed makes the same 10%
            // source population reproducible across server restarts and tests.
            let mut seed = config.seed.max(1);
            let mut random = || {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed >> 32) as f32 / u32::MAX as f32
            };
            let mut specs = Vec::new();
            for x in -40..=40 {
                for z in -40..=40 {
                    if random() < 0.1 {
                        specs.push(SpawnSpec {
                            position: [
                                x as f32 * 75. * config.world_scale,
                                0.,
                                z as f32 * 75. * config.world_scale,
                            ],
                            class: if random() < 0.1 {
                                Class::Warrok
                            } else {
                                Class::Zombie
                            },
                        });
                    }
                }
            }
            specs
        };
        if specs.len() > MAX_NPCS
            || specs
                .iter()
                .any(|s| !finite_position(s.position) || s.class.is_player())
        {
            return Err("Choose up to 1,024 valid monster starting positions.");
        }
        let mut world = Self {
            config,
            tick: 0,
            entities: BTreeMap::new(),
            spawners: specs
                .into_iter()
                .map(|spec| Spawner { spec, actor: None })
                .collect(),
            next_id: 1,
            next_event: 1,
            events: VecDeque::new(),
        };
        world.spawn_missing(surface);
        Ok(world)
    }
    pub fn population(&self) -> Population {
        let players = self
            .entities
            .values()
            .filter(|e| e.kind == Kind::Player)
            .count();
        Population {
            players,
            npcs: self.entities.len() - players,
            spawners: self.spawners.len(),
        }
    }
    fn entity(
        &mut self,
        class: Class,
        name: String,
        kind: Kind,
        position: V3,
        yaw: f32,
    ) -> Result<u32, &'static str> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or("This world needs to restart before adding more characters.")?;
        let mut motor = Prediction::new(position, yaw);
        motor.grounded = true;
        self.entities.insert(
            id,
            Entity {
                id,
                name,
                class,
                kind,
                motor,
                stats: class.stats(),
                inventory: Inventory::new(class),
                input: Controls::default(),
                ack_input: 0,
                input_sequence: 0,
                attack_latched: false,
                dance_latched: false,
                input_tick: self.tick,
                target: None,
                scan: 0,
                spawner: None,
                corpse: 0,
                attacks: 0,
                hits: 0,
                last_chat: None,
                commands: VecDeque::new(),
            },
        );
        Ok(id)
    }
    fn spawn_missing(&mut self, surface: &impl Surface) {
        for index in 0..self.spawners.len() {
            if self.spawners[index].actor.is_some() {
                continue;
            }
            let spec = self.spawners[index].spec.clone();
            let mut p = spec.position;
            p[1] = surface.height(p[0], p[2]);
            if !surface.can_occupy(p, spec.class.height()) {
                continue;
            }
            if let Ok(id) = self.entity(spec.class, spec.class.name().into(), Kind::Npc, p, 0.) {
                self.entities.get_mut(&id).unwrap().spawner = Some(index);
                self.spawners[index].actor = Some(id);
            }
        }
    }
    fn spawn_position(
        &self,
        class: Class,
        surface: &impl Surface,
        skip: Option<u32>,
    ) -> Option<V3> {
        for index in 0..65 {
            let mut p = self.config.spawn;
            if index > 0 {
                let angle = index as f32 * 2.399963;
                p[0] += angle.sin() * (index as f32).sqrt() * 0.9;
                p[2] += angle.cos() * (index as f32).sqrt() * 0.9;
            }
            p[1] = surface.height(p[0], p[2]);
            if surface.can_occupy(p, class.height())
                && self.entities.values().all(|e| {
                    Some(e.id) == skip
                        || e.stats.health == 0.
                        || separated(p, class.height(), e.motor.position, e.class.height())
                })
            {
                return Some(p);
            }
        }
        None
    }
    pub fn join(
        &mut self,
        name: &str,
        class: Class,
        surface: &impl Surface,
    ) -> Result<u32, &'static str> {
        let name = name.trim();
        if name.is_empty()
            || name.chars().count() > 64
            || name.chars().any(char::is_control)
            || !class.is_player()
        {
            return Err("Choose a name and a player character.");
        }
        if self.population().players >= MAX_PLAYERS {
            return Err("This world is full. Try again shortly.");
        }
        let p = self
            .spawn_position(class, surface, None)
            .ok_or("The starting area is full. Try again shortly.")?;
        let id = self.entity(class, name.into(), Kind::Player, p, self.config.yaw)?;
        self.emit(
            "join",
            Some(id),
            None,
            None,
            Some(format!("{name} has entered the game")),
            Some(name.into()),
            p,
            50. * self.config.world_scale,
        );
        Ok(id)
    }
    pub fn leave(&mut self, id: u32) {
        if let Some(e) = self.entities.remove(&id) {
            self.emit(
                "leave",
                Some(id),
                None,
                None,
                Some(format!("{} left the game", e.name)),
                Some(e.name),
                e.motor.position,
                50. * self.config.world_scale,
            );
        }
    }
    pub fn input(&mut self, id: u32, sequence: u32, input: Controls) -> Result<(), &'static str> {
        if !input.valid() {
            return Err("Choose valid movement controls.");
        }
        let e = self
            .entities
            .get_mut(&id)
            .filter(|e| e.kind == Kind::Player)
            .ok_or("Join the world before moving.")?;
        if sequence > e.input_sequence {
            e.attack_latched |= input.attack && !e.input.attack;
            e.dance_latched |= input.dance && !e.input.dance;
            e.input = input;
            e.input_sequence = sequence;
            e.input_tick = self.tick;
        }
        Ok(())
    }
    fn emit(
        &mut self,
        kind: &'static str,
        actor: Option<u32>,
        target: Option<u32>,
        amount: Option<f32>,
        text: Option<String>,
        name: Option<String>,
        position: V3,
        range: f32,
    ) {
        let event = Event {
            id: self.next_event,
            tick: self.tick,
            kind,
            actor,
            target,
            amount,
            text,
            name,
        };
        self.next_event += 1;
        self.events.push_back(QueuedEvent {
            event,
            position,
            range,
        });
        if self.events.len() > 2048 {
            self.events.pop_front();
        }
    }
    pub fn step(&mut self, surface: &impl Surface) {
        self.tick += 1;
        let players: Vec<_> = self
            .entities
            .values()
            .filter(|e| e.kind == Kind::Player && e.stats.health > 0.)
            .map(|e| (e.id, e.motor.position))
            .collect();
        let mut occupied: Vec<_> = self
            .entities
            .values()
            .map(|e| {
                (
                    e.id,
                    e.motor.position,
                    e.class.height(),
                    e.stats.health > 0.,
                )
            })
            .collect();
        let mut attacks = Vec::new();
        let mut dead = Vec::new();
        let scale = self.config.world_scale;
        for (index, e) in self.entities.values_mut().enumerate() {
            if e.stats.health == 0. {
                e.motor.state_ticks = e.motor.state_ticks.saturating_add(1).min(216_000);
                if e.kind == Kind::Npc {
                    e.corpse += 1;
                    if e.corpse >= 1800 {
                        dead.push(e.id);
                    }
                }
                continue;
            }
            let view = Occupancy {
                surface,
                others: &occupied,
                id: e.id,
            };
            if e.kind == Kind::Player {
                let mut input = if self.tick.saturating_sub(e.input_tick) <= 15 {
                    e.input
                } else {
                    Controls::default()
                };
                input.attack |= e.attack_latched;
                input.dance |= e.dance_latched;
                e.attack_latched = false;
                e.dance_latched = false;
                if predict(&mut e.motor, input, e.class, scale, &view) {
                    e.attacks = e.attacks.saturating_add(1);
                    attacks.push((e.id, false));
                }
                e.ack_input = e.input_sequence;
            } else {
                e.motor.state_ticks = e.motor.state_ticks.saturating_add(1).min(216_000);
                e.motor.cooldown_ticks = e.motor.cooldown_ticks.saturating_sub(1);
                let target = e
                    .target
                    .and_then(|id| players.iter().find(|(other, _)| *other == id).copied());
                match e.motor.state {
                    Motion::Idle => {
                        e.scan += 1;
                        if e.scan >= 300 {
                            e.scan = 0;
                            e.target = players
                                .iter()
                                .find(|(_, p)| distance(e.motor.position, *p) <= 50. * scale)
                                .map(|(id, _)| *id);
                            if e.target.is_some() {
                                e.motor.set_state(Motion::Walk);
                            }
                        }
                    }
                    Motion::Walk => {
                        if let Some((_, p)) = target {
                            let d = distance(e.motor.position, p);
                            e.motor.yaw =
                                (p[0] - e.motor.position[0]).atan2(-(p[2] - e.motor.position[2]));
                            if d > 100. * scale {
                                e.target = None;
                                e.motor.set_state(Motion::Idle);
                            } else if d < 10. * scale && e.motor.cooldown_ticks == 0 {
                                e.motor.set_state(Motion::Attack);
                                e.motor.cooldown_ticks = e.class.attack_cooldown();
                                e.attacks = e.attacks.saturating_add(1);
                                attacks.push((e.id, false));
                            }
                        } else {
                            e.target = None;
                            e.motor.set_state(Motion::Idle);
                        }
                    }
                    Motion::Attack => {
                        if e.motor.state_ticks
                            >= e.class.attack_duration().max(e.class.attack_cooldown()) as u32
                        {
                            e.motor.set_state(Motion::Walk);
                        }
                    }
                    _ => {}
                }
                let mut delta = [0.; 3];
                if e.motor.state == Motion::Walk {
                    if let Some((_, p)) = target {
                        let dx = p[0] - e.motor.position[0];
                        let dz = p[2] - e.motor.position[2];
                        let d = dx.hypot(dz).max(0.001);
                        let mut direction = [dx / d, dz / d];
                        for (id, pos, _, alive) in &occupied {
                            if *id == e.id || !alive {
                                continue;
                            }
                            let dx = e.motor.position[0] - pos[0];
                            let dz = e.motor.position[2] - pos[2];
                            let d = dx.hypot(dz);
                            if d > 0.001 && d < 1.0 {
                                let force = (1. - d) * 2.;
                                direction[0] += dx / d * force;
                                direction[1] += dz / d * force;
                            }
                        }
                        let d = direction[0].hypot(direction[1]).max(0.001);
                        delta = [
                            direction[0] / d * 10. * scale * DT,
                            0.,
                            direction[1] / d * 10. * scale * DT,
                        ];
                    }
                }
                if e.motor.state != Motion::Idle || !e.motor.grounded {
                    move_body(&mut e.motor, delta, e.class.height(), &view);
                }
            }
            occupied[index].1 = e.motor.position;
            if e.motor.state == Motion::Attack && e.motor.state_ticks == e.class.attack_hit() as u32
            {
                attacks.push((e.id, true));
            }
        }
        for (id, fire) in attacks {
            if fire {
                self.fire_attack(id, surface);
            } else if let Some(e) = self.entities.get(&id) {
                self.emit(
                    "attack",
                    Some(id),
                    None,
                    None,
                    None,
                    None,
                    e.motor.position,
                    500. * scale,
                );
            }
        }
        for id in dead {
            if let Some(e) = self.entities.remove(&id) {
                if let Some(index) = e.spawner {
                    self.spawners[index].actor = None;
                }
            }
        }
        self.spawn_missing(surface);
    }
    fn fire_attack(&mut self, id: u32, surface: &impl Surface) {
        let Some(attacker) = self.entities.get(&id).filter(|e| e.stats.health > 0.) else {
            return;
        };
        let from = attacker.motor.position;
        let yaw = attacker.motor.yaw;
        let class = attacker.class;
        let damage = attacker.stats.damage(class, &attacker.inventory);
        let range = class.range() * self.config.world_scale;
        let mut origin = from;
        origin[1] += class.height() * 0.5;
        let targets: Vec<_> = self
            .entities
            .values()
            .filter(|e| e.id != id && e.stats.health > 0.)
            .filter(|e| {
                let to = e.motor.position;
                let d = distance(from, to);
                // The controller only aims around Y. A walkable slope must not
                // turn a correctly facing actor's attack away from its target.
                let dot = ((to[0] - from[0]) * yaw.sin() - (to[2] - from[2]) * yaw.cos())
                    / horizontal(from, to).max(0.00001);
                let mut end = to;
                end[1] += e.class.height() * 0.5;
                d <= range && dot >= 0.9 && surface.clear(origin, end)
            })
            .map(|e| e.id)
            .collect();
        for target in targets {
            let e = self.entities.get_mut(&target).unwrap();
            e.stats.health = (e.stats.health - damage).max(0.);
            let died = e.stats.health == 0.;
            let p = e.motor.position;
            if died {
                e.motor.set_state(Motion::Death);
                e.motor.velocity = 0.;
                e.target = None;
                e.input = Controls::default();
                e.corpse = 0;
            }
            self.entities.get_mut(&id).unwrap().hits = self.entities[&id].hits.saturating_add(1);
            self.emit(
                "damage",
                Some(id),
                Some(target),
                Some(damage),
                None,
                None,
                p,
                500. * self.config.world_scale,
            );
            if died {
                self.emit(
                    "death",
                    Some(target),
                    Some(id),
                    None,
                    None,
                    None,
                    p,
                    500. * self.config.world_scale,
                );
            }
        }
    }
    fn command(
        &mut self,
        id: u32,
        request: &str,
        fingerprint: String,
        run: impl FnOnce(&mut Self) -> Result<(), &'static str>,
    ) -> Receipt {
        let error = |message: &str| Receipt {
            message_type: "receipt".into(),
            request_id: request.into(),
            ok: false,
            duplicate: false,
            message: Some(message.into()),
            inventory_version: None,
        };
        if !token(request) {
            return error("Give this action a valid name.");
        }
        let Some(e) = self.entities.get(&id).filter(|e| e.kind == Kind::Player) else {
            return error("Join the world before using this action.");
        };
        if let Some(cached) = e.commands.iter().find(|c| c.id == request) {
            if cached.fingerprint != fingerprint {
                return error("That action name was already used for another request.");
            }
            let mut receipt = cached.receipt.clone();
            receipt.duplicate = true;
            return receipt;
        }
        let result = run(self);
        let receipt = Receipt {
            message_type: "receipt".into(),
            request_id: request.into(),
            ok: result.is_ok(),
            duplicate: false,
            message: result.err().map(Into::into),
            inventory_version: self.entities.get(&id).map(|e| e.inventory.version),
        };
        if let Some(e) = self.entities.get_mut(&id) {
            e.commands.push_back(Cached {
                id: request.into(),
                fingerprint,
                receipt: receipt.clone(),
            });
            if e.commands.len() > 128 {
                e.commands.pop_front();
            }
        }
        receipt
    }
    pub fn swap(&mut self, id: u32, request: &str, version: u64, from: u8, to: u8) -> Receipt {
        self.command(
            id,
            request,
            format!("swap:{version}:{from}:{to}"),
            |world| {
                let e = world.entities.get_mut(&id).unwrap();
                if e.stats.health == 0. {
                    return Err("Respawn before changing equipment.");
                }
                e.inventory.swap(version, from, to)?;
                let p = e.motor.position;
                world.emit(
                    "inventory",
                    Some(id),
                    None,
                    None,
                    None,
                    None,
                    p,
                    500. * world.config.world_scale,
                );
                Ok(())
            },
        )
    }
    pub fn chat(&mut self, id: u32, request: &str, text: &str) -> Receipt {
        self.command(id, request, format!("chat:{text}"), |world| {
            let text = text.trim();
            if text.is_empty() || text.chars().count() > 64 || text.chars().any(char::is_control) {
                return Err("Write a message of up to 64 characters.");
            }
            let e = world.entities.get_mut(&id).unwrap();
            if e.last_chat
                .is_some_and(|t| world.tick.saturating_sub(t) < 30)
            {
                return Err("Wait a moment before sending another message.");
            }
            e.last_chat = Some(world.tick);
            let p = e.motor.position;
            let name = e.name.clone();
            world.emit(
                "chat",
                Some(id),
                None,
                None,
                Some(text.into()),
                Some(name),
                p,
                50. * world.config.world_scale,
            );
            Ok(())
        })
    }
    pub fn respawn(&mut self, id: u32, request: &str, surface: &impl Surface) -> Receipt {
        self.command(id, request, "respawn".into(), |world| {
            let e = &world.entities[&id];
            let class = e.class;
            if e.stats.health > 0. {
                return Err("You can respawn after your character falls.");
            }
            let p = world
                .spawn_position(class, surface, Some(id))
                .ok_or("The starting area is full. Try again shortly.")?;
            let e = world.entities.get_mut(&id).unwrap();
            e.motor = Prediction::new(p, world.config.yaw);
            e.motor.grounded = true;
            e.stats.health = e.stats.max_health;
            e.input = Controls::default();
            e.target = None;
            world.emit(
                "respawn",
                Some(id),
                None,
                None,
                None,
                None,
                p,
                500. * world.config.world_scale,
            );
            Ok(())
        })
    }
    pub fn snapshot(
        &self,
        id: u32,
        known: &mut BTreeSet<u32>,
        after_event: u64,
    ) -> Option<Snapshot> {
        let player = self.entities.get(&id).filter(|e| e.kind == Kind::Player)?;
        let mut nearby: Vec<_> = self
            .entities
            .values()
            .filter(|e| {
                e.id != id
                    && distance(e.motor.position, player.motor.position)
                        <= 500. * self.config.world_scale
            })
            .collect();
        nearby.sort_by(|a, b| {
            distance(a.motor.position, player.motor.position)
                .total_cmp(&distance(b.motor.position, player.motor.position))
                .then(a.id.cmp(&b.id))
        });
        nearby.truncate(64);
        let current: BTreeSet<_> = nearby.iter().map(|e| e.id).collect();
        let despawned = known.difference(&current).copied().collect();
        *known = current;
        let events = self
            .events
            .iter()
            .filter(|e| {
                e.event.id > after_event && distance(e.position, player.motor.position) <= e.range
            })
            .map(|e| e.event.clone())
            .collect();
        Some(Snapshot {
            message_type: "snapshot",
            tick: self.tick,
            player: player.snapshot(true),
            actors: nearby.into_iter().map(|e| e.snapshot(false)).collect(),
            despawned,
            events,
            ack_input: player.ack_input,
            last_event_id: self.next_event - 1,
            events_truncated: self
                .events
                .front()
                .is_some_and(|e| after_event != 0 && after_event < e.event.id.saturating_sub(1)),
            population: self.population(),
        })
    }
}

#[cfg(test)]
mod combat_tests {
    use super::*;
    fn fixture(specs: Vec<SpawnSpec>) -> World {
        World::new(
            WorldConfig {
                spawn: [0.; 3],
                spawners: Some(specs),
                ..Default::default()
            },
            &FlatSurface(0.),
        )
        .unwrap()
    }
    fn attack(world: &mut World, id: u32, sequence: u32, ticks: usize) {
        world
            .input(
                id,
                sequence,
                Controls {
                    attack: true,
                    ..Default::default()
                },
            )
            .unwrap();
        for _ in 0..ticks {
            world.step(&FlatSurface(0.));
        }
    }
    #[test]
    fn shared_pvp_hits_once_at_source_time_and_dead_players_cannot_move_or_attack() {
        let mut world = fixture(vec![]);
        let a = world
            .join("Paladin", Class::Paladin, &FlatSurface(0.))
            .unwrap();
        let b = world
            .join("Sorceror", Class::Sorceror, &FlatSurface(0.))
            .unwrap();
        world.entities.get_mut(&b).unwrap().motor.position = [0., 0., -1.2];
        attack(&mut world, a, 1, 20);
        assert_eq!(world.entities[&b].stats.health, 100.);
        world.step(&FlatSurface(0.));
        assert_eq!(world.entities[&b].stats.health, 0.);
        let pose = world.entities[&b].motor.clone();
        world
            .input(
                b,
                1,
                Controls {
                    forward: 1.,
                    turn: 1.,
                    attack: true,
                    ..Default::default()
                },
            )
            .unwrap();
        for _ in 0..120 {
            world.step(&FlatSurface(0.));
        }
        assert_eq!(world.entities[&b].motor.position, pose.position);
        assert_eq!(world.entities[&b].motor.yaw, pose.yaw);
        assert_eq!(world.entities[&a].stats.health, 200.);
        assert_eq!(
            world
                .events
                .iter()
                .filter(|e| e.event.kind == "damage")
                .count(),
            1
        );
        assert!(world.respawn(b, "respawn-1", &FlatSurface(0.)).ok);
        assert_eq!(world.entities[&b].stats.health, 100.);
        assert_eq!(world.entities[&a].stats.health, 200.);
    }
    #[test]
    fn unequipping_changes_damage_and_monster_corpses_respawn_after_thirty_seconds() {
        let mut world = fixture(vec![SpawnSpec {
            position: [0., 0., -1.2],
            class: Class::Zombie,
        }]);
        let npc = world.spawners[0].actor.unwrap();
        let a = world.join("A", Class::Paladin, &FlatSurface(0.)).unwrap();
        assert!(world.swap(a, "unequip", 0, 24, 2).ok);
        attack(&mut world, a, 1, 21);
        assert_eq!(world.entities[&npc].stats.health, 10.);
        world.input(a, 2, Controls::default()).unwrap();
        for _ in 0..100 {
            world.step(&FlatSurface(0.));
        }
        attack(&mut world, a, 3, 21);
        assert_eq!(world.entities[&npc].stats.health, 0.);
        world.input(a, 4, Controls::default()).unwrap();
        for _ in 0..1799 {
            world.step(&FlatSurface(0.));
        }
        assert!(world.entities.contains_key(&npc));
        world.step(&FlatSurface(0.));
        let replacement = world.spawners[0].actor.unwrap();
        assert_ne!(replacement, npc);
        assert!(!world.entities.contains_key(&npc));
        assert_eq!(world.entities[&replacement].stats.health, 20.);
        assert_eq!(world.entities[&a].stats.experience, 0);
    }
    #[test]
    fn sorceror_warrok_and_facing_cover_use_their_actual_damage_rules() {
        let mut world = fixture(vec![]);
        let a = world
            .join("Sorceror", Class::Sorceror, &FlatSurface(0.))
            .unwrap();
        let b = world
            .join("Paladin", Class::Paladin, &FlatSurface(0.))
            .unwrap();
        world.entities.get_mut(&b).unwrap().motor.position = [0., 0., -4.];
        attack(&mut world, a, 1, 59);
        assert_eq!(world.entities[&b].stats.health, 200.);
        world.step(&FlatSurface(0.));
        assert_eq!(world.entities[&b].stats.health, 180.);
        let mut world = fixture(vec![SpawnSpec {
            position: [0., 0., -1.2],
            class: Class::Warrok,
        }]);
        let npc = world.spawners[0].actor.unwrap();
        let a = world.join("A", Class::Paladin, &FlatSurface(0.)).unwrap();
        {
            let e = world.entities.get_mut(&npc).unwrap();
            e.target = Some(a);
            e.motor.set_state(Motion::Walk);
        }
        for _ in 0..90 {
            world.step(&FlatSurface(0.));
        }
        assert_eq!(world.entities[&a].stats.health, 200.);
        world.step(&FlatSurface(0.));
        assert_eq!(world.entities[&a].stats.health, 160.);
        struct Covered;
        impl Surface for Covered {
            fn height(&self, _: f32, _: f32) -> f32 {
                0.
            }
            fn clear(&self, _: V3, _: V3) -> bool {
                false
            }
        }
        world.fire_attack(npc, &Covered);
        assert_eq!(world.entities[&a].stats.health, 160.);
    }

    struct SourceHill;
    impl Surface for SourceHill {
        fn height(&self, x: f32, z: f32) -> f32 {
            terrain::height(x as f64, z as f64) as f32
        }
        fn clear(&self, from: V3, to: V3) -> bool {
            height_line_clear(from, to, |x, z| self.height(x, z))
        }
    }

    fn hill_duel() -> (World, u32, u32) {
        // Actual browser failure: two mutually facing actors on source terrain,
        // with 0.602 m horizontal separation and 0.393 m elevation difference.
        let player = [48.9838, 13.9971, -12.4595];
        let monster = [49.5856, 14.3901, -12.4533];
        for p in [player, monster] {
            assert!((SourceHill.height(p[0], p[2]) - p[1]).abs() < 0.002);
        }
        let mut world = World::new(
            WorldConfig {
                spawn: player,
                yaw: 1.5293,
                spawners: Some(vec![SpawnSpec {
                    position: monster,
                    class: Class::Warrok,
                }]),
                ..Default::default()
            },
            &SourceHill,
        )
        .unwrap();
        let npc = world.spawners[0].actor.unwrap();
        let player = world
            .join("Hill duel", Class::Paladin, &SourceHill)
            .unwrap();
        let monster = world.entities.get_mut(&npc).unwrap();
        monster.motor.yaw = -1.5617;
        monster.motor.set_state(Motion::Walk);
        monster.target = Some(player);
        (world, player, npc)
    }

    #[test]
    fn actual_source_hill_duel_hits_uphill_and_downhill_once_at_each_windup() {
        let (mut world, player, npc) = hill_duel();
        let p = world.entities[&player].motor.position;
        let n = world.entities[&npc].motor.position;
        // The previous normalization rejected both actors despite correct yaw.
        assert!(horizontal(p, n) / distance(p, n) < 0.9);
        let mut from = p;
        from[1] += Class::Paladin.height() * 0.5;
        let mut to = n;
        to[1] += Class::Warrok.height() * 0.5;
        assert!(SourceHill.clear(from, to));
        assert!(SourceHill.clear(to, from));
        world
            .input(
                player,
                1,
                Controls {
                    attack: true,
                    ..Default::default()
                },
            )
            .unwrap();
        for _ in 0..20 {
            world.step(&SourceHill);
        }
        assert_eq!(world.entities[&npc].stats.health, 1000.);
        world.step(&SourceHill);
        assert_eq!(world.entities[&npc].stats.health, 700.);
        for _ in 21..90 {
            world.step(&SourceHill);
        }
        assert_eq!(world.entities[&player].stats.health, 200.);
        world.step(&SourceHill);
        assert_eq!(world.entities[&player].stats.health, 160.);
        for _ in 0..20 {
            world.step(&SourceHill);
        }
        assert_eq!(world.entities[&npc].stats.health, 700.);
        assert_eq!(world.entities[&player].stats.health, 160.);
        assert_eq!(world.entities[&player].hits, 1);
        assert_eq!(world.entities[&npc].hits, 1);
        assert_eq!(
            world
                .events
                .iter()
                .filter(|e| e.event.kind == "damage")
                .count(),
            2
        );
    }

    #[test]
    fn hill_facing_fix_keeps_three_dimensional_reach_back_misses_and_solid_cover() {
        let (mut world, player, npc) = hill_duel();
        let original = world.entities[&npc].motor.position;
        world.entities.get_mut(&player).unwrap().motor.yaw += std::f32::consts::PI;
        world.fire_attack(player, &SourceHill);
        assert_eq!(world.entities[&npc].stats.health, 1000.);
        world.entities.get_mut(&player).unwrap().motor.yaw = 1.5293;
        world.entities.get_mut(&npc).unwrap().motor.position[1] += 3.;
        world.fire_attack(player, &SourceHill);
        assert_eq!(world.entities[&npc].stats.health, 1000.);
        world.entities.get_mut(&npc).unwrap().motor.position = original;
        struct Wall;
        impl Surface for Wall {
            fn height(&self, x: f32, z: f32) -> f32 {
                SourceHill.height(x, z)
            }
            fn clear(&self, _: V3, _: V3) -> bool {
                false
            }
        }
        world.fire_attack(player, &Wall);
        assert_eq!(world.entities[&npc].stats.health, 1000.);
        world.fire_attack(player, &SourceHill);
        assert_eq!(world.entities[&npc].stats.health, 700.);
    }
}
