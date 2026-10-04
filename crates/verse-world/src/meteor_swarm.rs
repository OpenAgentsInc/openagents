//! Meteor Swarm (SRD 5.2.1) as four falling bodies.
//!
//! Level 9 Evocation. Casting time: Action. Range: 1 mile. Components: V, S.
//! Duration: Instantaneous. Blazing orbs of fire plummet to the ground at
//! four different points you can see within range. Each creature in a
//! 40-foot-radius Sphere centered on each point makes a Dexterity saving
//! throw, taking 20d6 Fire and 20d6 Bludgeoning damage on a failure, or half
//! as much on a success. A creature in the area of more than one Sphere is
//! affected only once. A nonmagical object that isn't worn or carried also
//! takes the damage if it's in the area, and it starts burning if it's
//! flammable.
//!
//! Each meteor is a dynamic body that spawns 120 m above its point, a
//! quarter second after the one before it, and falls under the world's
//! gravity from an initial 60 m/s along a 20 degree slant that comes from
//! the caster's side. Meteors collide with nothing in the contact solver:
//! after each step a swept test along the path since the last check finds
//! the first collider or creature capsule it crossed, and the meteor
//! detonates there. The SRD Sphere is centered on that detonation, which is
//! the SRD point only when the path is clear; [`Impact::obstructed`]
//! records when it is not.
//!
//! Creatures are resolved once each across all four Spheres and are never
//! moved, because the SRD has no displacement. Unattended objects take the
//! full damage once (objects make no saves), break into debris at 0 hit
//! points, receive a radial blast impulse from every Sphere they are in,
//! and ignite when flammable. The damage is rolled once for the spell, as
//! the SRD rolls damage once for every target of one effect.
//!
//! The mechanics act through a [`Host`], which owns the bodies: a bare
//! [`World`] in tests, the chamber's [`crate::spells::SpellWorld`] in play.

use std::collections::BTreeSet;

use glam::DVec3;
use physics::{Body, BodyId, BodyKind, Collider, Filter, Shape, World};
use serde::{Deserialize, Serialize};

/// One foot, m.
pub const FOOT: f64 = 0.3048;
/// Spell level.
pub const LEVEL: u8 = 9;
/// Range: 1 mile.
pub const RANGE: f64 = 5_280.0 * FOOT;
/// Radius of each Sphere: 40 feet.
pub const RADIUS: f64 = 40.0 * FOOT;
/// Number of meteors and points.
pub const METEORS: usize = 4;
/// Damage dice of each type: 20d6 Fire and 20d6 Bludgeoning.
pub const DICE: u32 = 20;
/// Die size.
pub const DIE: u32 = 6;
/// Smallest separation that makes two points different, m.
pub const DISTINCT: f64 = 1.0 * FOOT;
/// Meteor radius, m.
pub const METEOR_RADIUS: f64 = 0.6;
/// Meteor mass, kg.
pub const METEOR_MASS: f64 = 800.0;
/// Height of each spawn above its point, m.
pub const SPAWN_HEIGHT: f64 = 120.0;
/// Delay between successive meteors, s.
pub const STAGGER: f64 = 0.25;
/// Initial meteor speed, m/s.
pub const INITIAL_SPEED: f64 = 60.0;
/// Slant of the initial velocity from vertical, degrees.
pub const SLANT_DEGREES: f64 = 20.0;
/// Upward bias of the blast impulse above horizontal, degrees.
pub const BLAST_UPWARD_DEGREES: f64 = 30.0;
/// Blast impulse at the center of a Sphere, N s. It falls off linearly to
/// zero at the edge. Tuned so a 20 kg crate 3 m from the center leaves at
/// 13.0 m/s and 30 degrees, which carries it 15 m on flat ground.
pub const BLAST_IMPULSE: f64 = 346.0;
/// A detonation farther than this from its SRD point counts as obstructed, m.
pub const OBSTRUCTION_TOLERANCE: f64 = 1.0;
/// A meteor that has not detonated after this long is removed, s.
pub const MAX_FLIGHT: f64 = 10.0;
/// Fire damage a burning object deals to itself each interval.
pub const BURN_DAMAGE: i32 = 3;
/// Interval between burn damage, s.
pub const BURN_INTERVAL: f64 = 1.0;
/// How long an object burns, s. Fire spread is out of scope.
pub const BURN_DURATION: f64 = 30.0;
/// Debris chunks per broken object: the four quarters of its bounds,
/// split across its two horizontal axes.
pub const DEBRIS_CHUNKS: usize = 4;
/// Ledger term for every blast impulse.
pub const LEDGER_TERM: &str = "spell:meteor_swarm";
/// The overlay's SRD line.
pub const SRD_LINE: &str = "Level 9 Evocation | Range 1 mile | four 40-ft-radius Spheres | \
    DEX save | 20d6 Fire + 20d6 Bludgeoning, half on a save";

/// Why a cast was refused before any effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// A point is farther than 1 mile from the caster.
    OutOfRange,
    /// Two points are not different points.
    SamePoint,
    /// The caster can't see a point.
    NotVisible,
    /// A point or the caster position is not finite.
    Invalid,
}

impl Refusal {
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::OutOfRange => "A point is beyond 1 mile",
            Self::SamePoint => "The four points must be different points",
            Self::NotVisible => "You can't see a point",
            Self::Invalid => "Invalid Meteor Swarm points",
        }
    }
}

/// What the mechanics need from whoever owns the bodies.
pub trait Host {
    fn world(&self) -> &World;
    /// Adds a meteor body at `start` moving at `velocity`. It must collide
    /// with nothing in the contact solver.
    ///
    /// # Errors
    ///
    /// Returns a message when the host can't add the body.
    fn spawn_meteor(&mut self, start: DVec3, velocity: DVec3) -> Result<BodyId, String>;
    /// Removes a meteor that detonated or expired.
    ///
    /// # Errors
    ///
    /// Returns a message when the host can't remove it.
    fn remove_meteor(&mut self, body: BodyId) -> Result<(), String>;
    /// Applies a blast impulse at `at`.
    ///
    /// # Errors
    ///
    /// Returns a message when the host refuses the impulse.
    fn impulse(&mut self, body: BodyId, impulse: DVec3, at: DVec3) -> Result<(), String>;
    /// Replaces a broken object with its debris, usually the [`debris`] of
    /// its body. Returns the debris bodies, which may be none.
    ///
    /// # Errors
    ///
    /// Returns a message when the host can't remove the object.
    fn shatter(&mut self, body: BodyId) -> Result<Vec<BodyId>, String>;
    /// One damage die with `sides` faces.
    fn damage_die(&mut self, sides: u32) -> u32;
    /// The d20 of a creature's Dexterity save.
    fn save_d20(&mut self, creature: &Creature) -> u32;
}

/// The spell's damage, rolled once for every target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Damage {
    pub fire: i32,
    pub bludgeoning: i32,
}

impl Damage {
    /// Roll 20d6 Fire and 20d6 Bludgeoning.
    pub fn roll(roll: &mut dyn FnMut(u32) -> u32) -> Self {
        let mut dice = || (0..DICE).map(|_| roll(DIE) as i32).sum::<i32>();
        let fire = dice();
        let bludgeoning = dice();
        Self { fire, bludgeoning }
    }

    /// Half of each type, rounded down, for a successful save.
    #[must_use]
    pub fn halved(self) -> Self {
        Self {
            fire: self.fire / 2,
            bludgeoning: self.bludgeoning / 2,
        }
    }

    #[must_use]
    pub fn total(self) -> i32 {
        self.fire + self.bludgeoning
    }
}

/// One Dexterity saving throw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Save {
    pub d20: i32,
    pub modifier: i32,
    pub dc: i32,
    pub success: bool,
}

impl Save {
    /// A save of `d20` plus `modifier` against `dc`; meeting the DC succeeds.
    #[must_use]
    pub fn new(d20: i32, modifier: i32, dc: i32) -> Self {
        Self {
            d20,
            modifier,
            dc,
            success: d20 + modifier >= dc,
        }
    }
}

/// A creature the Spheres can reach: an upright capsule with its feet at
/// `feet`. The spell only reads it; nothing here moves a creature.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub id: u64,
    pub feet: DVec3,
    pub radius: f64,
    pub height: f64,
    /// Dexterity modifier from its stat block.
    pub dexterity: i32,
}

impl Creature {
    /// The point of the capsule nearest `x`.
    #[must_use]
    pub fn closest_point(&self, x: DVec3) -> DVec3 {
        let low = self.feet + DVec3::Y * self.radius;
        let high = self.feet + DVec3::Y * (self.height - self.radius).max(self.radius);
        let core = DVec3::new(low.x, x.y.clamp(low.y, high.y), low.z);
        let offset = x - core;
        if offset.length() <= self.radius {
            x
        } else {
            core + offset.normalize() * self.radius
        }
    }
}

/// An unattended object: a dynamic or secured body nobody wears or carries.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Unattended {
    pub body: BodyId,
    /// SRD object hit points; `None` for debris and anything the spell can't
    /// break.
    pub hp: Option<i32>,
    pub flammable: bool,
    /// Tick the fire goes out, while burning.
    pub burning_until: Option<u64>,
    /// Tick of the next burn damage, while burning.
    pub next_burn: u64,
}

impl Unattended {
    #[must_use]
    pub fn new(body: BodyId, hp: Option<i32>, flammable: bool) -> Self {
        Self {
            body,
            hp,
            flammable,
            burning_until: None,
            next_burn: 0,
        }
    }

    #[must_use]
    pub fn burning(&self, tick: u64) -> bool {
        self.burning_until.is_some_and(|until| tick < until)
    }
}

/// A creature's single resolution across every Sphere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatureHit {
    pub id: u64,
    /// Which meteor's Sphere affected it.
    pub meteor: usize,
    pub save: Save,
    pub damage: Damage,
}

/// What a Sphere did to one object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObjectHit {
    pub body: BodyId,
    pub meteor: usize,
    /// Damage dealt by this Sphere: the full amount the first time, none
    /// after.
    pub damage: Option<Damage>,
    /// Blast impulse, N s.
    pub impulse: DVec3,
    pub ignited: bool,
    pub broke: bool,
    /// Debris bodies when it broke.
    pub debris: Vec<BodyId>,
}

/// One detonation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Impact {
    pub meteor: usize,
    pub tick: u64,
    /// The SRD point the caster chose.
    pub point: DVec3,
    /// Where the meteor detonated: the Sphere's center.
    pub center: DVec3,
    pub radius: f64,
    /// The meteor met something before its point.
    pub obstructed: bool,
    /// The body it struck, or `None` for a creature.
    pub struck: Option<BodyId>,
    pub creatures: Vec<CreatureHit>,
    pub objects: Vec<ObjectHit>,
}

/// A meteor's flight.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Meteor {
    pub point: DVec3,
    /// Tick it is due to appear.
    pub spawn_tick: u64,
    pub start: DVec3,
    pub velocity: DVec3,
    pub body: Option<BodyId>,
    /// Center at the last swept check; the next sweep starts here.
    pub last: DVec3,
    pub done: bool,
}

/// A cast in flight. Serializes with the world, so a checkpoint taken while
/// meteors fall replays identically.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeteorSwarm {
    pub caster: DVec3,
    pub dc: i32,
    pub damage: Damage,
    pub meteors: Vec<Meteor>,
    /// Creatures already affected; a creature is affected only once.
    pub affected: BTreeSet<u64>,
    /// Objects already damaged; they still feel every blast.
    pub damaged: BTreeSet<BodyId>,
    pub impacts: Vec<Impact>,
}

/// Steps in `seconds` at the world's step length.
#[must_use]
pub fn ticks(world: &World, seconds: f64) -> u64 {
    (seconds / world.dt).round() as u64
}

/// Check four chosen points against range, distinctness, and sight.
///
/// # Errors
///
/// Returns the first [`Refusal`] that applies.
pub fn validate(
    caster: DVec3,
    points: &[DVec3; METEORS],
    visible: impl Fn(DVec3) -> bool,
) -> Result<(), Refusal> {
    if !caster.is_finite() || points.iter().any(|p| !p.is_finite()) {
        return Err(Refusal::Invalid);
    }
    for (i, p) in points.iter().enumerate() {
        if p.distance(caster) > RANGE {
            return Err(Refusal::OutOfRange);
        }
        if points[..i].iter().any(|q| q.distance(*p) < DISTINCT) {
            return Err(Refusal::SamePoint);
        }
        if !visible(*p) {
            return Err(Refusal::NotVisible);
        }
    }
    Ok(())
}

/// Spawn position and initial velocity of a meteor that strikes `point`
/// from `caster`'s side under `gravity` (m/s^2, positive down).
#[must_use]
pub fn trajectory(caster: DVec3, point: DVec3, gravity: f64) -> (DVec3, DVec3) {
    let away = DVec3::new(point.x - caster.x, 0.0, point.z - caster.z)
        .try_normalize()
        .unwrap_or(DVec3::X);
    let slant = SLANT_DEGREES.to_radians();
    let down = INITIAL_SPEED * slant.cos();
    let across = INITIAL_SPEED * slant.sin();
    let time = fall_time(gravity);
    let target = point + DVec3::Y * METEOR_RADIUS;
    let start = target + DVec3::Y * SPAWN_HEIGHT - away * across * time;
    (start, away * across - DVec3::Y * down)
}

/// Seconds a meteor takes to fall [`SPAWN_HEIGHT`] under `gravity`.
#[must_use]
pub fn fall_time(gravity: f64) -> f64 {
    let down = INITIAL_SPEED * SLANT_DEGREES.to_radians().cos();
    if gravity > 0.0 {
        (-down + (down * down + 2.0 * gravity * SPAWN_HEIGHT).sqrt()) / gravity
    } else {
        SPAWN_HEIGHT / down
    }
}

/// Segments of the flight [`path_clear`] tests.
pub const PATH_SEGMENTS: u32 = 32;

/// Whether a meteor cast from `caster` reaches `point` without striking a
/// body that `blocks` says obstructs it, using the same swept test that
/// detonates meteors in flight. Bodies `blocks` rejects (props a meteor may
/// detonate on) and the ground at the point itself do not count.
#[must_use]
pub fn path_clear(
    world: &World,
    caster: DVec3,
    point: DVec3,
    gravity: f64,
    blocks: impl Fn(BodyId) -> bool,
) -> bool {
    let (start, velocity) = trajectory(caster, point, gravity);
    let time = fall_time(gravity);
    let at = |t: f64| start + velocity * t - DVec3::Y * (0.5 * gravity.max(0.0) * t * t);
    let mut from = start;
    for n in 1..=PATH_SEGMENTS {
        let to = at(time * f64::from(n) / f64::from(PATH_SEGMENTS));
        if let Some((center, struck)) = sweep(world, from, to, &[]) {
            return center.distance(point) <= OBSTRUCTION_TOLERANCE
                || struck.is_none_or(|body| !blocks(body));
        }
        from = to;
    }
    true
}

/// Blast impulse on a body at `at` from a Sphere centered on `center`.
#[must_use]
pub fn blast(center: DVec3, at: DVec3) -> DVec3 {
    let d = at.distance(center);
    if d >= RADIUS {
        return DVec3::ZERO;
    }
    let up = BLAST_UPWARD_DEGREES.to_radians();
    let direction = match DVec3::new(at.x - center.x, 0.0, at.z - center.z).try_normalize() {
        Some(out) => out * up.cos() + DVec3::Y * up.sin(),
        None => DVec3::Y,
    };
    direction * BLAST_IMPULSE * (1.0 - d / RADIUS)
}

impl MeteorSwarm {
    /// Cast at four points: validate them, roll the damage, and launch the
    /// first meteor now. The rest follow [`STAGGER`] apart.
    ///
    /// # Errors
    ///
    /// Returns a [`Refusal`] before any effect, or the host's message.
    pub fn cast(
        host: &mut impl Host,
        caster: DVec3,
        points: [DVec3; METEORS],
        gravity: f64,
        dc: i32,
        visible: impl Fn(DVec3) -> bool,
    ) -> Result<Self, String> {
        validate(caster, &points, visible).map_err(|r| r.message().to_string())?;
        let damage = Damage::roll(&mut |sides| host.damage_die(sides));
        let world = host.world();
        let stagger = ticks(world, STAGGER);
        let tick = world.tick;
        let meteors = points
            .iter()
            .enumerate()
            .map(|(i, &point)| {
                let (start, velocity) = trajectory(caster, point, gravity);
                Meteor {
                    point,
                    spawn_tick: tick + stagger * i as u64,
                    start,
                    velocity,
                    body: None,
                    last: start,
                    done: false,
                }
            })
            .collect();
        let mut swarm = Self {
            caster,
            dc,
            damage,
            meteors,
            affected: BTreeSet::new(),
            damaged: BTreeSet::new(),
            impacts: Vec::new(),
        };
        swarm.spawn_due(host)?;
        Ok(swarm)
    }

    /// Whether every meteor has detonated or expired.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.meteors.iter().all(|m| m.done)
    }

    /// Bodies of the meteors still falling.
    pub fn falling(&self) -> impl Iterator<Item = BodyId> + '_ {
        self.meteors
            .iter()
            .filter(|m| !m.done)
            .filter_map(|m| m.body)
    }

    fn spawn_due(&mut self, host: &mut impl Host) -> Result<(), String> {
        let tick = host.world().tick;
        for meteor in &mut self.meteors {
            if meteor.body.is_none() && !meteor.done && tick >= meteor.spawn_tick {
                meteor.body = Some(host.spawn_meteor(meteor.start, meteor.velocity)?);
                meteor.last = meteor.start;
            }
        }
        Ok(())
    }

    /// Run after the world steps (once per step or once per several):
    /// detonate meteors whose path since the last check met a collider or a
    /// creature, apply each detonation, and launch the meteors now due.
    /// `creatures` are where creatures stand now; `objects` is the registry
    /// of unattended objects, which gains the debris of anything that
    /// breaks.
    ///
    /// # Errors
    ///
    /// Returns the host's message.
    pub fn after_step(
        &mut self,
        host: &mut impl Host,
        creatures: &[Creature],
        objects: &mut Vec<Unattended>,
    ) -> Result<Vec<Impact>, String> {
        let mut impacts = Vec::new();
        let max_flight = ticks(host.world(), MAX_FLIGHT);
        for index in 0..self.meteors.len() {
            let meteor = self.meteors[index];
            let Some(id) = meteor.body else { continue };
            if meteor.done {
                continue;
            }
            let now = host.world()[id].pos;
            if let Some((center, struck)) = sweep(host.world(), meteor.last, now, creatures) {
                host.remove_meteor(id)?;
                self.meteors[index].done = true;
                let impact = self.detonate(host, index, center, struck, creatures, objects)?;
                impacts.push(impact);
            } else if host.world().tick >= meteor.spawn_tick + max_flight {
                host.remove_meteor(id)?;
                self.meteors[index].done = true;
            } else {
                self.meteors[index].last = now;
            }
        }
        self.spawn_due(host)?;
        self.impacts.extend(impacts.iter().cloned());
        Ok(impacts)
    }

    fn detonate(
        &mut self,
        host: &mut impl Host,
        meteor: usize,
        center: DVec3,
        struck: Option<BodyId>,
        creatures: &[Creature],
        objects: &mut Vec<Unattended>,
    ) -> Result<Impact, String> {
        let point = self.meteors[meteor].point;
        let mut hits = Vec::new();
        for creature in creatures {
            if creature.closest_point(center).distance(center) > RADIUS
                || !self.affected.insert(creature.id)
            {
                continue;
            }
            let save = Save::new(host.save_d20(creature) as i32, creature.dexterity, self.dc);
            let damage = if save.success {
                self.damage.halved()
            } else {
                self.damage
            };
            hits.push(CreatureHit {
                id: creature.id,
                meteor,
                save,
                damage,
            });
        }
        let mut object_hits = Vec::new();
        // Debris added during this detonation is not blasted again.
        for index in 0..objects.len() {
            let object = objects[index];
            let body = host.world()[object.body];
            if body.removed || !in_sphere(host.world(), object.body, center) {
                continue;
            }
            let impulse = if body.kind == BodyKind::Dynamic {
                blast(center, body.pos)
            } else {
                DVec3::ZERO
            };
            if impulse != DVec3::ZERO {
                host.impulse(object.body, impulse, body.pos)?;
            }
            let mut hit = ObjectHit {
                body: object.body,
                meteor,
                damage: None,
                impulse,
                ignited: false,
                broke: false,
                debris: Vec::new(),
            };
            if self.damaged.insert(object.body) {
                hit.damage = Some(self.damage);
                let tick = host.world().tick;
                let (duration, interval) = (
                    ticks(host.world(), BURN_DURATION),
                    ticks(host.world(), BURN_INTERVAL),
                );
                let slot = &mut objects[index];
                if slot.flammable && slot.burning_until.is_none() {
                    slot.burning_until = Some(tick + duration);
                    slot.next_burn = tick + interval;
                    hit.ignited = true;
                }
                if let Some(hp) = slot.hp.as_mut() {
                    *hp = (*hp - self.damage.total()).max(0);
                    if *hp == 0 {
                        hit.broke = true;
                        hit.debris = shatter(host, objects, index)?;
                        // Debris is what is left of a damaged object.
                        self.damaged.extend(hit.debris.iter().copied());
                    }
                }
            }
            object_hits.push(hit);
        }
        Ok(Impact {
            meteor,
            tick: host.world().tick,
            point,
            center,
            radius: RADIUS,
            obstructed: center.distance(point) > OBSTRUCTION_TOLERANCE,
            struck,
            creatures: hits,
            objects: object_hits,
        })
    }
}

/// Apply burn damage to burning objects whose interval is due, breaking any
/// that reach 0 hit points. Run after the world steps. Returns the objects
/// that broke.
///
/// # Errors
///
/// Returns the host's message.
pub fn burn(host: &mut impl Host, objects: &mut Vec<Unattended>) -> Result<Vec<BodyId>, String> {
    let mut broken = Vec::new();
    let interval = ticks(host.world(), BURN_INTERVAL);
    let tick = host.world().tick;
    for index in 0..objects.len() {
        let object = objects[index];
        if host.world()[object.body].removed || !object.burning(tick) || tick < object.next_burn {
            continue;
        }
        let slot = &mut objects[index];
        slot.next_burn += interval;
        if let Some(hp) = slot.hp.as_mut() {
            *hp = (*hp - BURN_DAMAGE).max(0);
            if *hp == 0 {
                broken.push(object.body);
                shatter(host, objects, index)?;
            }
        }
    }
    Ok(broken)
}

/// Whether any collider of `body` reaches within the Sphere at `center`.
fn in_sphere(world: &World, body: BodyId, center: DVec3) -> bool {
    world
        .colliders()
        .iter()
        .filter(|c| c.body == body)
        .any(|c| c.closest_point(world, center).distance(center) <= RADIUS)
}

/// Half extents of the box that bounds a shape in its collider frame.
fn bounds(shape: Shape) -> DVec3 {
    match shape {
        Shape::Sphere { radius } => DVec3::splat(radius),
        Shape::Capsule {
            radius,
            half_length,
        } => DVec3::new(radius, radius, radius + half_length),
        Shape::Cuboid { half } => half,
    }
}

/// One piece of a broken object: a box body and its collider's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chunk {
    pub body: Body,
    pub half: DVec3,
    pub filter: Filter,
    pub material: physics::Material,
}

/// The debris of `body`: the four quarters of its first collider's bounds
/// across the collider's x and z axes. Each chunk carries a quarter of the
/// mass and the rigid velocity at its center, so linear momentum is
/// preserved exactly, and angular momentum too for a box, since the
/// quarters tile it.
#[must_use]
pub fn debris(world: &World, body: BodyId) -> Vec<Chunk> {
    let parent = world[body];
    let Some(collider) = world.colliders().iter().find(|c| c.body == body).copied() else {
        return Vec::new();
    };
    let full = bounds(collider.shape);
    let half = DVec3::new(full.x / 2.0, full.y, full.z / 2.0);
    let mass = parent.mass / DEBRIS_CHUNKS as f64;
    let omega = parent.omega_world();
    let rotation = parent.orientation * collider.rotation;
    let center = parent.pos + parent.orientation * collider.offset;
    (0..DEBRIS_CHUNKS)
        .map(|corner| {
            let sign = |bit: usize| if corner & bit == 0 { -1.0 } else { 1.0 };
            let local = DVec3::new(sign(1) * half.x, 0.0, sign(2) * half.z);
            let at = center + rotation * local;
            let mut chunk = Body::new(mass, Body::box_inertia(mass, half * 2.0), at);
            chunk.orientation = rotation;
            chunk.prev_orientation = rotation;
            chunk.vel = parent.vel + omega.cross(at - parent.pos);
            chunk.omega = rotation.inverse() * omega;
            Chunk {
                body: chunk,
                half,
                filter: collider.filter,
                material: collider.material,
            }
        })
        .collect()
}

/// Break the object at `index`: the host replaces it with debris, which
/// joins the registry with the object's fire and no hit points.
fn shatter(
    host: &mut impl Host,
    objects: &mut Vec<Unattended>,
    index: usize,
) -> Result<Vec<BodyId>, String> {
    let object = objects[index];
    objects[index].hp = Some(0);
    let pieces = host.shatter(object.body)?;
    for &id in &pieces {
        objects.push(Unattended {
            body: id,
            hp: None,
            flammable: object.flammable,
            burning_until: object.burning_until,
            next_burn: object.next_burn,
        });
    }
    Ok(pieces)
}

/// The first thing a meteor moving from `from` to `to` meets: the
/// detonation point and the body struck (`None` for a creature).
fn sweep(
    world: &World,
    from: DVec3,
    to: DVec3,
    creatures: &[Creature],
) -> Option<(DVec3, Option<BodyId>)> {
    let path = to - from;
    let length = path.length();
    let direction = path.try_normalize()?;
    let accept = |c: &Collider| c.filter != Filter::NONE;
    // A center ray reaches one radius ahead; four rim rays at 0.7 radius
    // reach the sphere's surface ahead of them, about 0.71 radius.
    let rim = 0.7 * METEOR_RADIUS;
    let rim_reach = (METEOR_RADIUS * METEOR_RADIUS - rim * rim).sqrt();
    let side = direction.any_orthonormal_vector();
    let other = direction.cross(side);
    let mut best: Option<(f64, DVec3, Option<BodyId>)> = None;
    let mut consider = |travel: f64, point: DVec3, struck: Option<BodyId>| {
        if travel <= length && best.is_none_or(|b| travel < b.0) {
            best = Some((travel, point, struck));
        }
    };
    let rays = [
        (DVec3::ZERO, METEOR_RADIUS),
        (side * rim, rim_reach),
        (-side * rim, rim_reach),
        (other * rim, rim_reach),
        (-other * rim, rim_reach),
    ];
    for (offset, reach) in rays {
        if let Some(hit) = world.raycast(from + offset, direction, length + reach, &accept) {
            consider(hit.distance - reach, hit.point, Some(hit.body));
        }
    }
    for creature in creatures {
        if let Ok(Some(t)) = physics::continuous::sphere_capsule(
            from,
            to,
            METEOR_RADIUS,
            creature.feet,
            creature.feet,
            creature.radius,
            creature.height,
        ) {
            let center = from + path * t;
            consider(length * t, creature.closest_point(center), None);
        }
    }
    best.map(|(_, point, struck)| (point, struck))
}

/// A bare world as a host, with seeded dice: meteors are spheres that
/// collide with nothing, and debris replaces broken bodies directly.
pub struct Bench {
    pub world: World,
    pub dice: crate::spells::Dice,
}

impl Host for Bench {
    fn world(&self) -> &World {
        &self.world
    }

    fn spawn_meteor(&mut self, start: DVec3, velocity: DVec3) -> Result<BodyId, String> {
        let inertia = DVec3::splat(0.4 * METEOR_MASS * METEOR_RADIUS * METEOR_RADIUS);
        let mut body = Body::new(METEOR_MASS, inertia, start);
        body.vel = velocity;
        let id = self.world.add(body);
        self.world.add_collider(
            Collider::new(
                id,
                Shape::Sphere {
                    radius: METEOR_RADIUS,
                },
            )
            .with_filter(Filter::NONE),
        );
        Ok(id)
    }

    fn remove_meteor(&mut self, body: BodyId) -> Result<(), String> {
        self.world.remove_body(body);
        Ok(())
    }

    fn impulse(&mut self, body: BodyId, impulse: DVec3, at: DVec3) -> Result<(), String> {
        self.world[body].apply_impulse_at(impulse, at);
        Ok(())
    }

    fn shatter(&mut self, body: BodyId) -> Result<Vec<BodyId>, String> {
        let chunks = debris(&self.world, body);
        self.world.remove_body(body);
        Ok(chunks
            .into_iter()
            .map(|chunk| {
                let id = self.world.add(chunk.body);
                self.world.add_collider(
                    Collider::new(id, Shape::Cuboid { half: chunk.half })
                        .with_filter(chunk.filter)
                        .with_material(chunk.material),
                );
                id
            })
            .collect())
    }

    fn damage_die(&mut self, sides: u32) -> u32 {
        self.dice.roll(sides)
    }

    fn save_d20(&mut self, creature: &Creature) -> u32 {
        self.dice.save(creature.id, "Dexterity", 0, 0).roll
    }
}

#[cfg(test)]
mod tests;
