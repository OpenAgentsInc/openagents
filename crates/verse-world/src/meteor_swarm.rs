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
//! Each meteor is a dynamic sphere that spawns 120 m above its point, a
//! quarter second after the one before it, and falls under the world's
//! gravity from an initial 60 m/s along a 20 degree slant that comes from
//! the caster's side. Meteors collide with nothing in the contact solver:
//! after each step a swept test along the step's path finds the first
//! collider or creature capsule it crossed, and the meteor detonates there.
//! The SRD Sphere is centered on that detonation, which is the SRD point
//! only when the path is clear; [`Impact::obstructed`] records when it is
//! not.
//!
//! Creatures are resolved once each across all four Spheres and are never
//! moved, because the SRD has no displacement. Unattended objects take the
//! full damage once (objects make no saves), break into debris at 0 hit
//! points, receive a radial blast impulse from every Sphere they are in,
//! and ignite when flammable. The damage is rolled once for the spell, as
//! the SRD rolls damage once for every target of one effect.

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
/// Flight timing; standard casts retain the issue's height, speed, and stagger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    pub height: f64,
    pub speed: f64,
    pub stagger: f64,
}
impl Flight {
    pub const STANDARD: Self = Self {
        height: SPAWN_HEIGHT,
        speed: INITIAL_SPEED,
        stagger: STAGGER,
    };
    pub const PLAYGROUND: Self = Self {
        height: 30.,
        speed: 8.,
        stagger: 0.5,
    };
    pub fn valid(self) -> bool {
        (1.0..=1000.0).contains(&self.height)
            && (0.1..=500.0).contains(&self.speed)
            && (0.0..=10.0).contains(&self.stagger)
    }
}

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
/// Debris chunks per broken object: the octants of its bounds.
pub const DEBRIS_CHUNKS: usize = 8;
/// Ledger term for every blast impulse.
pub const LEDGER_TERM: &str = "meteor_swarm";
/// The overlay's SRD line.
pub const SRD_LINE: &str = "Meteor Swarm - level 9 Evocation - range 1 mile - \
    four 40-ft-radius Spheres - Dexterity save - 20d6 Fire + 20d6 Bludgeoning";

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
    pub id: u32,
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
    pub id: u32,
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
    /// Tick it appears.
    pub spawn_tick: u64,
    pub start: DVec3,
    pub velocity: DVec3,
    pub body: Option<BodyId>,
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
    pub affected: BTreeSet<u32>,
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
    trajectory_with_flight(caster, point, gravity, Flight::STANDARD)
}

pub fn trajectory_with_flight(
    caster: DVec3,
    point: DVec3,
    gravity: f64,
    flight: Flight,
) -> (DVec3, DVec3) {
    let away = DVec3::new(point.x - caster.x, 0.0, point.z - caster.z)
        .try_normalize()
        .unwrap_or(DVec3::X);
    let slant = SLANT_DEGREES.to_radians();
    let down = flight.speed * slant.cos();
    let across = flight.speed * slant.sin();
    // Time to fall from the configured height under gravity.
    let time = if gravity > 0.0 {
        (-down + (down * down + 2.0 * gravity * flight.height).sqrt()) / gravity
    } else {
        flight.height / down
    };
    let target = point + DVec3::Y * METEOR_RADIUS;
    let start = target + DVec3::Y * flight.height - away * across * time;
    (start, away * across - DVec3::Y * down)
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
    /// first meteor this tick. The rest follow [`STAGGER`] apart.
    ///
    /// # Errors
    ///
    /// Returns a [`Refusal`] before any effect.
    pub fn cast(
        world: &mut World,
        caster: DVec3,
        points: [DVec3; METEORS],
        gravity: f64,
        dc: i32,
        visible: impl Fn(DVec3) -> bool,
        roll: &mut dyn FnMut(u32) -> u32,
    ) -> Result<Self, Refusal> {
        Self::cast_with_flight(
            world,
            caster,
            points,
            gravity,
            Flight::STANDARD,
            dc,
            visible,
            roll,
        )
    }

    /// Cast with an explicit recording profile; stored trajectories replay without it.
    pub fn cast_with_flight(
        world: &mut World,
        caster: DVec3,
        points: [DVec3; METEORS],
        gravity: f64,
        flight: Flight,
        dc: i32,
        visible: impl Fn(DVec3) -> bool,
        roll: &mut dyn FnMut(u32) -> u32,
    ) -> Result<Self, Refusal> {
        if !flight.valid() {
            return Err(Refusal::Invalid);
        }
        validate(caster, &points, visible)?;
        let damage = Damage::roll(roll);
        let stagger = ticks(world, flight.stagger);
        let meteors = points
            .iter()
            .enumerate()
            .map(|(i, &point)| {
                let (start, velocity) = trajectory_with_flight(caster, point, gravity, flight);
                Meteor {
                    point,
                    spawn_tick: world.tick + stagger * i as u64,
                    start,
                    velocity,
                    body: None,
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
        swarm.spawn_due(world);
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

    fn spawn_due(&mut self, world: &mut World) {
        for meteor in &mut self.meteors {
            if meteor.body.is_none() && !meteor.done && world.tick >= meteor.spawn_tick {
                let inertia = DVec3::splat(0.4 * METEOR_MASS * METEOR_RADIUS * METEOR_RADIUS);
                let mut body = Body::new(METEOR_MASS, inertia, meteor.start);
                body.vel = meteor.velocity;
                let id = world.add(body);
                // Meteors never touch anything in the solver; the swept test
                // in `after_step` decides where they detonate.
                world.add_collider(
                    Collider::new(
                        id,
                        Shape::Sphere {
                            radius: METEOR_RADIUS,
                        },
                    )
                    .with_filter(Filter::NONE),
                );
                meteor.body = Some(id);
            }
        }
    }

    /// Run after every world step: detonate meteors whose path this step
    /// met a collider or a creature, apply each detonation, and launch the
    /// meteors now due. `creatures` are where creatures stand this tick;
    /// `objects` is the caller's registry of unattended objects, which
    /// gains the debris of anything that breaks.
    pub fn after_step(
        &mut self,
        world: &mut World,
        creatures: &[Creature],
        objects: &mut Vec<Unattended>,
        roll: &mut dyn FnMut(u32) -> u32,
    ) -> Vec<Impact> {
        let mut impacts = Vec::new();
        let max_flight = ticks(world, MAX_FLIGHT);
        for index in 0..self.meteors.len() {
            let meteor = self.meteors[index];
            let Some(id) = meteor.body else { continue };
            if meteor.done {
                continue;
            }
            let body = world[id];
            if let Some((center, struck)) = sweep(world, body.prev_pos, body.pos, creatures) {
                world.remove_body(id);
                self.meteors[index].done = true;
                let impact = self.detonate(world, index, center, struck, creatures, objects, roll);
                impacts.push(impact);
            } else if world.tick >= meteor.spawn_tick + max_flight {
                world.remove_body(id);
                self.meteors[index].done = true;
            }
        }
        self.spawn_due(world);
        self.impacts.extend(impacts.iter().cloned());
        impacts
    }

    #[allow(clippy::too_many_arguments)]
    fn detonate(
        &mut self,
        world: &mut World,
        meteor: usize,
        center: DVec3,
        struck: Option<BodyId>,
        creatures: &[Creature],
        objects: &mut Vec<Unattended>,
        roll: &mut dyn FnMut(u32) -> u32,
    ) -> Impact {
        let point = self.meteors[meteor].point;
        let mut hits = Vec::new();
        for creature in creatures {
            if creature.closest_point(center).distance(center) > RADIUS
                || !self.affected.insert(creature.id)
            {
                continue;
            }
            let save = Save::new(roll(20) as i32, creature.dexterity, self.dc);
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
        for index in 0..objects.len() {
            let object = objects[index];
            let body = world[object.body];
            if body.removed || !in_sphere(world, object.body, center) {
                continue;
            }
            let impulse = if body.kind == BodyKind::Dynamic {
                blast(center, body.pos)
            } else {
                DVec3::ZERO
            };
            if impulse != DVec3::ZERO {
                world[object.body].apply_impulse_at(impulse, body.pos);
            }
            let mut hit = ObjectHit {
                body: object.body,
                meteor,
                damage: None,
                impulse,
                ignited: false,
                debris: Vec::new(),
            };
            if self.damaged.insert(object.body) {
                hit.damage = Some(self.damage);
                let slot = &mut objects[index];
                if slot.flammable && slot.burning_until.is_none() {
                    slot.burning_until = Some(world.tick + ticks(world, BURN_DURATION));
                    slot.next_burn = world.tick + ticks(world, BURN_INTERVAL);
                    hit.ignited = true;
                }
                if let Some(hp) = slot.hp.as_mut() {
                    *hp = (*hp - self.damage.total()).max(0);
                    if *hp == 0 {
                        hit.debris = shatter(world, objects, index);
                        // Debris is what is left of a damaged object.
                        self.damaged.extend(hit.debris.iter().copied());
                    }
                }
            }
            object_hits.push(hit);
        }
        Impact {
            meteor,
            tick: world.tick,
            point,
            center,
            radius: RADIUS,
            obstructed: center.distance(point) > OBSTRUCTION_TOLERANCE,
            struck,
            creatures: hits,
            objects: object_hits,
        }
    }
}

/// Apply burn damage to burning objects whose interval is due, breaking any
/// that reach 0 hit points. Run after every world step. Returns the objects
/// that broke.
pub fn burn(world: &mut World, objects: &mut Vec<Unattended>) -> Vec<BodyId> {
    let mut broken = Vec::new();
    let interval = ticks(world, BURN_INTERVAL);
    for index in 0..objects.len() {
        let object = objects[index];
        if world[object.body].removed
            || !object.burning(world.tick)
            || world.tick < object.next_burn
        {
            continue;
        }
        let slot = &mut objects[index];
        slot.next_burn += interval;
        if let Some(hp) = slot.hp.as_mut() {
            *hp = (*hp - BURN_DAMAGE).max(0);
            if *hp == 0 {
                broken.push(object.body);
                shatter(world, objects, index);
            }
        }
    }
    broken
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

/// Break the object at `index` into the eight octants of its first
/// collider's bounds. Each chunk carries an eighth of the mass and the
/// rigid velocity at its center, so linear momentum is preserved exactly
/// (and angular momentum too, for a box). Chunks inherit the object's
/// filter, material, and fire, and have no hit points.
pub(crate) fn shatter(
    world: &mut World,
    objects: &mut Vec<Unattended>,
    index: usize,
) -> Vec<BodyId> {
    let object = objects[index];
    let body = world[object.body];
    let Some(collider) = world
        .colliders()
        .iter()
        .find(|c| c.body == object.body)
        .copied()
    else {
        world.remove_body(object.body);
        return Vec::new();
    };
    objects[index].hp = Some(0);
    world.remove_body(object.body);
    let half = bounds(collider.shape) / 2.0;
    let mass = body.mass / DEBRIS_CHUNKS as f64;
    let omega = body.omega_world();
    let mut debris = Vec::new();
    for corner in 0..DEBRIS_CHUNKS {
        let sign = |bit: usize| if corner & bit == 0 { -1.0 } else { 1.0 };
        let local = DVec3::new(sign(1), sign(2), sign(4)) * half;
        let rotation = body.orientation * collider.rotation;
        let at = body.pos + body.orientation * collider.offset + rotation * local;
        let mut chunk = Body::new(mass, Body::box_inertia(mass, half * 2.0), at);
        chunk.orientation = rotation;
        chunk.prev_orientation = rotation;
        chunk.vel = body.vel + omega.cross(at - body.pos);
        chunk.omega = rotation.inverse() * omega;
        if body.kind != BodyKind::Dynamic {
            chunk.kind = BodyKind::Dynamic;
        }
        let id = world.add(chunk);
        world.add_collider(
            Collider::new(id, Shape::Cuboid { half })
                .with_filter(collider.filter)
                .with_material(collider.material),
        );
        objects.push(Unattended {
            body: id,
            hp: None,
            flammable: object.flammable,
            burning_until: objects[index].burning_until,
            next_burn: objects[index].next_burn,
        });
        debris.push(id);
    }
    debris
}

/// The first thing a meteor moving from `from` to `to` this step meets:
/// the detonation point and the body struck (`None` for a creature).
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
        if (0.0..=length).contains(&travel.max(0.0)) && best.is_none_or(|b| travel < b.0) {
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

/// Deterministic dice for the chamber until the shared simulation carries
/// its own: SplitMix64 from a seed, saved with the checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dice {
    pub state: u64,
}

impl Dice {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// One die of `sides` faces, 1 to `sides`.
    pub fn roll(&mut self, sides: u32) -> u32 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z % u64::from(sides.max(1))) as u32 + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physics::trace::{Tolerance, Trace};
    use physics::{Ledger, Momentum, Uniform};

    const DT: f64 = 1.0 / 120.0;
    const G: f64 = 9.81;
    const DOWN: DVec3 = DVec3::new(0.0, -G, 0.0);
    const DC: i32 = 15;

    fn static_box(world: &mut World, center: DVec3, half: DVec3) -> BodyId {
        let id = world.add(Body::new(1.0, DVec3::ONE, center).with_kind(BodyKind::Static));
        world.add_collider(Collider::new(id, Shape::Cuboid { half }));
        id
    }

    fn floor(world: &mut World) -> BodyId {
        static_box(
            world,
            DVec3::new(0.0, -0.5, 0.0),
            DVec3::new(200.0, 0.5, 200.0),
        )
    }

    fn block(world: &mut World, mass: f64, half: DVec3, at: DVec3) -> BodyId {
        let id = world.add(Body::new(mass, Body::box_inertia(mass, half * 2.0), at));
        world.add_collider(Collider::new(id, Shape::Cuboid { half }));
        id
    }

    fn crate_at(world: &mut World, x: f64, z: f64) -> BodyId {
        block(world, 20.0, DVec3::splat(0.25), DVec3::new(x, 0.25, z))
    }

    fn dummy(id: u32, x: f64, z: f64) -> Creature {
        Creature {
            id,
            feet: DVec3::new(x, 0.0, z),
            radius: 0.35,
            height: 1.8,
            dexterity: 0,
        }
    }

    /// Four points far apart on a line 40 m from the caster.
    fn points() -> [DVec3; METEORS] {
        [-45.0, -15.0, 15.0, 45.0].map(|x| DVec3::new(x, 0.0, 40.0))
    }

    struct Scene {
        world: World,
        swarm: MeteorSwarm,
        creatures: Vec<Creature>,
        objects: Vec<Unattended>,
        dice: Dice,
    }

    impl Scene {
        fn cast(mut world: World, points: [DVec3; METEORS], dice: Dice) -> Self {
            let mut dice = dice;
            let swarm =
                MeteorSwarm::cast(&mut world, DVec3::ZERO, points, G, DC, |_| true, &mut |s| {
                    dice.roll(s)
                })
                .unwrap();
            Self {
                world,
                swarm,
                creatures: Vec::new(),
                objects: Vec::new(),
                dice,
            }
        }

        fn step(&mut self) -> Vec<Impact> {
            self.world.step(&Uniform(DOWN));
            let dice = &mut self.dice;
            let impacts = self.swarm.after_step(
                &mut self.world,
                &self.creatures,
                &mut self.objects,
                &mut |s| dice.roll(s),
            );
            burn(&mut self.world, &mut self.objects);
            impacts
        }

        fn run(&mut self, seconds: f64) -> Vec<Impact> {
            let mut impacts = Vec::new();
            for _ in 0..ticks(&self.world, seconds) {
                impacts.extend(self.step());
            }
            impacts
        }
    }

    fn flat() -> World {
        let mut world = World::new(DT);
        floor(&mut world);
        world
    }

    #[test]
    fn four_meteors_fall_a_quarter_second_apart_and_strike_their_points() {
        let mut scene = Scene::cast(flat(), points(), Dice::new(1));
        assert_eq!(scene.swarm.falling().count(), 1);
        let spawns: Vec<u64> = scene.swarm.meteors.iter().map(|m| m.spawn_tick).collect();
        assert_eq!(spawns, vec![0, 30, 60, 90]);
        for meteor in &scene.swarm.meteors {
            assert!((meteor.start.y - meteor.point.y - SPAWN_HEIGHT - METEOR_RADIUS).abs() < 1e-9);
            assert!((meteor.velocity.length() - INITIAL_SPEED).abs() < 1e-9);
            let slant = meteor.velocity.angle_between(-DVec3::Y).to_degrees();
            assert!((slant - SLANT_DEGREES).abs() < 1e-9);
            // From the caster's side: it moves away from the caster.
            let away = meteor.point - DVec3::ZERO;
            assert!(meteor.velocity.x * away.x + meteor.velocity.z * away.z > 0.0);
        }
        let impacts = scene.run(4.0);
        assert!(scene.swarm.finished());
        assert_eq!(impacts.len(), METEORS);
        for (i, impact) in impacts.iter().enumerate() {
            assert_eq!(impact.meteor, i);
            assert_eq!(impact.radius, RADIUS);
            assert!((impact.radius - 12.192).abs() < 1e-9);
            assert!(!impact.obstructed, "{impact:?}");
            assert!(impact.center.distance(impact.point) < 0.6, "{impact:?}");
        }
        // Staggered starts and equal flights give staggered impacts.
        for pair in impacts.windows(2) {
            assert_eq!(pair[1].tick - pair[0].tick, 30);
        }
    }

    #[test]
    fn slow_recording_flight_lands_on_all_four_points() {
        let mut world = flat();
        let mut dice = Dice::new(1);
        let swarm = MeteorSwarm::cast_with_flight(
            &mut world,
            DVec3::ZERO,
            points(),
            G,
            Flight::PLAYGROUND,
            DC,
            |_| true,
            &mut |s| dice.roll(s),
        )
        .unwrap();
        assert_eq!(
            swarm
                .meteors
                .iter()
                .map(|m| m.spawn_tick)
                .collect::<Vec<_>>(),
            [0, 60, 120, 180]
        );
        let mut scene = Scene {
            world,
            swarm,
            creatures: vec![],
            objects: vec![],
            dice,
        };
        let impacts = scene.run(5.);
        assert_eq!(impacts.len(), 4);
        assert!(
            impacts
                .iter()
                .all(|i| !i.obstructed && i.center.distance(i.point) < 0.6)
        );
    }

    #[test]
    fn the_sphere_reaches_exactly_forty_feet() {
        let mut scene = Scene::cast(flat(), points(), Dice::new(2));
        let p = points()[0];
        scene.creatures = vec![
            dummy(1, p.x + RADIUS - 1.0, p.z),
            dummy(2, p.x + RADIUS + 1.2, p.z),
        ];
        let impacts = scene.run(4.0);
        let ids: Vec<u32> = impacts
            .iter()
            .flat_map(|i| i.creatures.iter().map(|c| c.id))
            .collect();
        assert_eq!(ids, vec![1]);
    }

    #[test]
    fn a_creature_in_two_spheres_is_damaged_once() {
        // The first two points are 10 m apart; the dummy between them is in
        // both Spheres.
        let points = [
            DVec3::new(-5.0, 0.0, 40.0),
            DVec3::new(5.0, 0.0, 40.0),
            DVec3::new(60.0, 0.0, 40.0),
            DVec3::new(-60.0, 0.0, 40.0),
        ];
        let mut scene = Scene::cast(flat(), points, Dice::new(3));
        scene.creatures = vec![dummy(7, 0.0, 41.0), dummy(8, 9.0, 40.0)];
        let impacts = scene.run(4.0);
        let hits: Vec<&CreatureHit> = impacts.iter().flat_map(|i| &i.creatures).collect();
        assert_eq!(hits.iter().filter(|h| h.id == 7).count(), 1);
        assert_eq!(hits.iter().filter(|h| h.id == 8).count(), 1);
        let first = hits.iter().find(|h| h.id == 7).unwrap();
        assert_eq!(first.meteor, 0);
        // The second Sphere also covers dummy 7 but adds nothing.
        assert!(impacts[1].creatures.iter().all(|h| h.id != 7));
        assert_eq!(scene.swarm.affected.len(), 2);
    }

    #[test]
    fn damage_is_twenty_d6_of_each_type_or_half_on_a_save() {
        let mut sixes = |_| 6;
        let damage = Damage::roll(&mut sixes);
        assert_eq!(
            damage,
            Damage {
                fire: 120,
                bludgeoning: 120
            }
        );
        // Alternate ones and twos: every Fire die is odd-indexed.
        let mut n = 0;
        let mut alternating = |_| {
            n += 1;
            if n <= 20 { 1 } else { 3 }
        };
        let damage = Damage::roll(&mut alternating);
        assert_eq!(
            damage,
            Damage {
                fire: 20,
                bludgeoning: 60
            }
        );
        assert_eq!(
            damage.halved(),
            Damage {
                fire: 10,
                bludgeoning: 30
            }
        );
        // Odd totals round down per type.
        let odd = Damage {
            fire: 71,
            bludgeoning: 69,
        };
        assert_eq!(
            odd.halved(),
            Damage {
                fire: 35,
                bludgeoning: 34
            }
        );
        // Rolled dice stay in range.
        let mut dice = Dice::new(9);
        for _ in 0..200 {
            let d = Damage::roll(&mut |s| dice.roll(s));
            assert!((20..=120).contains(&d.fire) && (20..=120).contains(&d.bludgeoning));
        }

        // In a cast: a failing and a succeeding dummy, fixed dice.
        let mut world = flat();
        let mut fixed = |s: u32| if s == 20 { 1 } else { 4 };
        let mut swarm = MeteorSwarm::cast(
            &mut world,
            DVec3::ZERO,
            points(),
            G,
            DC,
            |_| true,
            &mut fixed,
        )
        .unwrap();
        assert_eq!(
            swarm.damage,
            Damage {
                fire: 80,
                bludgeoning: 80
            }
        );
        let creatures = [
            dummy(1, -45.0, 42.0),
            Creature {
                dexterity: 20,
                ..dummy(2, -43.0, 40.0)
            },
        ];
        let mut hits = Vec::new();
        for _ in 0..ticks(&world, 3.0) {
            world.step(&Uniform(DOWN));
            for impact in swarm.after_step(&mut world, &creatures, &mut Vec::new(), &mut fixed) {
                hits.extend(impact.creatures);
            }
        }
        assert_eq!(hits.len(), 2);
        assert!(!hits[0].save.success);
        assert_eq!(
            hits[0].damage,
            Damage {
                fire: 80,
                bludgeoning: 80
            }
        );
        assert!(hits[1].save.success);
        assert_eq!(
            hits[1].damage,
            Damage {
                fire: 40,
                bludgeoning: 40
            }
        );
    }

    #[test]
    fn creatures_are_not_displaced() {
        let mut scene = Scene::cast(flat(), points(), Dice::new(4));
        let before = vec![dummy(1, -45.0, 41.0), dummy(2, -44.0, 38.0)];
        scene.creatures = before.clone();
        let mut world_before = scene.world.bodies().len();
        let impacts = scene.run(4.0);
        assert_eq!(impacts[0].creatures.len(), 2);
        // The spell reads creatures and adds no body for them; the only
        // bodies it creates are the three meteors spawned since the cast.
        assert_eq!(scene.creatures, before);
        world_before += 3;
        assert_eq!(scene.world.bodies().len(), world_before);
    }

    #[test]
    fn a_crate_three_meters_out_flies_about_fifteen_meters() {
        let mut world = flat();
        let crate_ = crate_at(&mut world, 3.0, 0.0);
        for _ in 0..60 {
            world.step(&Uniform(DOWN));
        }
        let start = world[crate_].pos;
        let impulse = blast(DVec3::ZERO, start);
        let launch = impulse / 20.0;
        assert!((launch.length() - 13.0).abs() < 0.1, "{launch}");
        let elevation = launch.angle_between(DVec3::new(launch.x, 0.0, launch.z));
        assert!((elevation.to_degrees() - BLAST_UPWARD_DEGREES).abs() < 1e-9);
        world[crate_].apply_impulse_at(impulse, start);
        let mut airborne = false;
        let mut landed = None;
        for _ in 0..ticks(&world, 4.0) {
            world.step(&Uniform(DOWN));
            let body = world[crate_];
            airborne |= body.pos.y > start.y + 0.5;
            if airborne && body.vel.y <= 0.0 && body.pos.y <= start.y + 0.05 {
                landed = Some(body.pos);
                break;
            }
        }
        let landed = landed.expect("the crate lands");
        let carried = (landed - start).with_y(0.0).length();
        assert!((carried - 15.0).abs() < 1.0, "carried {carried} m");
    }

    #[test]
    fn unattended_objects_take_full_damage_break_and_feel_the_blast() {
        let p = points()[0];
        let mut world = flat();
        let crate_ = crate_at(&mut world, p.x + 3.0, p.z);
        let stone = block(
            &mut world,
            1_000.0,
            DVec3::splat(0.5),
            DVec3::new(p.x - 4.0, 0.5, p.z),
        );
        let far = crate_at(&mut world, p.x, p.z + RADIUS + 2.0);
        for _ in 0..60 {
            world.step(&Uniform(DOWN));
        }
        let mut scene = Scene::cast(world, points(), Dice::new(5));
        scene.objects = vec![
            Unattended::new(crate_, Some(10), false),
            Unattended::new(stone, None, false),
            Unattended::new(far, Some(10), false),
        ];
        let mut impacts = Vec::new();
        let mut stone_after = None;
        while impacts.is_empty() {
            impacts.extend(scene.step());
            if !impacts.is_empty() {
                stone_after = Some(scene.world[stone].vel);
            }
        }
        let impact = &impacts[0];
        let damage = scene.swarm.damage;
        let crate_hit = impact.objects.iter().find(|h| h.body == crate_).unwrap();
        assert_eq!(crate_hit.damage, Some(damage), "full damage, no save");
        assert_eq!(crate_hit.debris.len(), DEBRIS_CHUNKS);
        assert!(scene.world[crate_].removed);
        // Debris carries the crate's mass and its momentum after the blast.
        let chunks: Vec<Body> = crate_hit.debris.iter().map(|&d| scene.world[d]).collect();
        let mass: f64 = chunks.iter().map(|c| c.mass).sum();
        assert!((mass - 20.0).abs() < 1e-9);
        let momentum: DVec3 = chunks.iter().map(Body::momentum).sum();
        assert!((momentum - crate_hit.impulse).length() < 1e-6, "{momentum}");
        // A 1,000 kg block survives and is shoved by the blast.
        let stone_hit = impact.objects.iter().find(|h| h.body == stone).unwrap();
        assert!(stone_hit.debris.is_empty());
        let expected = blast(impact.center, scene.world[stone].prev_pos) / 1_000.0;
        assert!(stone_hit.impulse.length() > 0.0);
        assert!(
            (stone_after.unwrap() - expected).length() < 0.05,
            "{expected}"
        );
        // Outside the Sphere: nothing.
        assert!(impact.objects.iter().all(|h| h.body != far));
        assert_eq!(scene.objects[2].hp, Some(10));
    }

    #[test]
    fn overlapping_spheres_damage_an_object_once_but_blast_it_twice() {
        let points = [
            DVec3::new(-4.0, 0.0, 40.0),
            DVec3::new(4.0, 0.0, 40.0),
            DVec3::new(60.0, 0.0, 40.0),
            DVec3::new(-60.0, 0.0, 40.0),
        ];
        let mut world = flat();
        let stone = block(
            &mut world,
            1_000.0,
            DVec3::splat(0.5),
            DVec3::new(0.0, 0.5, 46.0),
        );
        let mut scene = Scene::cast(world, points, Dice::new(6));
        scene.objects = vec![Unattended::new(stone, Some(500), false)];
        let impacts = scene.run(4.0);
        let hits: Vec<&ObjectHit> = impacts
            .iter()
            .flat_map(|i| &i.objects)
            .filter(|h| h.body == stone)
            .collect();
        assert_eq!(hits.len(), 2);
        assert!(hits[0].damage.is_some());
        assert!(hits[1].damage.is_none());
        assert!(hits.iter().all(|h| h.impulse.length() > 0.0));
        assert_eq!(scene.objects[0].hp, Some(500 - scene.swarm.damage.total()));
    }

    #[test]
    fn flammable_objects_ignite_and_burn_only_themselves() {
        let p = points()[0];
        let mut world = flat();
        let barrel = block(
            &mut world,
            60.0,
            DVec3::new(0.3, 0.45, 0.3),
            DVec3::new(p.x + 6.0, 0.45, p.z),
        );
        let stone = block(
            &mut world,
            1_000.0,
            DVec3::splat(0.5),
            DVec3::new(p.x - 6.0, 0.5, p.z),
        );
        let mut world_scene = world;
        for _ in 0..60 {
            world_scene.step(&Uniform(DOWN));
        }
        // All damage dice show 1: 20 Fire + 20 Bludgeoning.
        let mut ones = |_| 1;
        let mut swarm = MeteorSwarm::cast(
            &mut world_scene,
            DVec3::ZERO,
            points(),
            G,
            DC,
            |_| true,
            &mut ones,
        )
        .unwrap();
        let mut objects = vec![
            Unattended::new(barrel, Some(50), true),
            Unattended::new(stone, Some(500), false),
        ];
        let mut ignited = Vec::new();
        let mut broken = Vec::new();
        for _ in 0..ticks(&world_scene, 10.0) {
            world_scene.step(&Uniform(DOWN));
            for impact in swarm.after_step(&mut world_scene, &[], &mut objects, &mut ones) {
                ignited.extend(impact.objects.iter().filter(|h| h.ignited).map(|h| h.body));
            }
            broken.extend(burn(&mut world_scene, &mut objects));
        }
        assert_eq!(ignited, vec![barrel]);
        assert!(!objects[1].burning(world_scene.tick));
        assert_eq!(objects[1].hp, Some(460));
        // 50 - 40 = 10 left, then 3 per second: broken after the fourth burn.
        assert_eq!(broken, vec![barrel]);
        assert_eq!(objects[0].hp, Some(0));
        let chunks: Vec<&Unattended> = objects[2..].iter().collect();
        assert_eq!(chunks.len(), DEBRIS_CHUNKS);
        assert!(chunks.iter().all(|c| c.burning(world_scene.tick)));
    }

    #[test]
    fn an_obstructed_meteor_detonates_at_the_obstruction() {
        let mut world = flat();
        let p = points()[1];
        // An overhang 5 m up over the second point, wide enough to catch
        // the slanted path.
        let overhang = static_box(
            &mut world,
            DVec3::new(p.x, 5.25, p.z),
            DVec3::new(4.0, 0.25, 4.0),
        );
        let mut scene = Scene::cast(world, points(), Dice::new(7));
        scene.creatures = vec![dummy(3, p.x, p.z)];
        let impacts = scene.run(4.0);
        let blocked = impacts.iter().find(|i| i.meteor == 1).unwrap();
        assert!(blocked.obstructed);
        assert_eq!(blocked.struck, Some(overhang));
        assert!(
            (blocked.center.y - 5.5).abs() < 0.05,
            "{:?}",
            blocked.center
        );
        // The Sphere is centered on the overhang, so the dummy below it is
        // still inside.
        assert_eq!(blocked.creatures.len(), 1);
        assert!(
            impacts
                .iter()
                .filter(|i| i.meteor != 1)
                .all(|i| !i.obstructed)
        );
    }

    #[test]
    fn a_meteor_detonates_on_a_creature_in_its_path() {
        let mut scene = Scene::cast(flat(), points(), Dice::new(8));
        let p = points()[0];
        // A tall creature standing at the point: the meteor strikes it.
        scene.creatures = vec![Creature {
            height: 6.0,
            radius: 1.5,
            ..dummy(4, p.x, p.z)
        }];
        let impacts = scene.run(3.0);
        assert_eq!(impacts[0].struck, None);
        assert!(impacts[0].center.y > 2.0);
        assert_eq!(impacts[0].creatures.len(), 1);
    }

    #[test]
    fn placement_is_validated() {
        let ok = points();
        assert_eq!(validate(DVec3::ZERO, &ok, |_| true), Ok(()));
        let mut far = ok;
        far[2] = DVec3::new(RANGE + 1.0, 0.0, 0.0);
        assert_eq!(
            validate(DVec3::ZERO, &far, |_| true),
            Err(Refusal::OutOfRange)
        );
        let mut same = ok;
        same[3] = same[0] + DVec3::X * 0.1;
        assert_eq!(
            validate(DVec3::ZERO, &same, |_| true),
            Err(Refusal::SamePoint)
        );
        assert_eq!(
            validate(DVec3::ZERO, &ok, |p| p.x < 40.0),
            Err(Refusal::NotVisible)
        );
        let mut bad = ok;
        bad[0] = DVec3::NAN;
        assert_eq!(validate(DVec3::ZERO, &bad, |_| true), Err(Refusal::Invalid));
        let mut world = flat();
        let bodies = world.bodies().len();
        let mut dice = Dice::new(1);
        assert!(
            MeteorSwarm::cast(&mut world, DVec3::ZERO, far, G, DC, |_| true, &mut |s| {
                dice.roll(s)
            })
            .is_err()
        );
        assert_eq!(world.bodies().len(), bodies);
        assert_eq!(dice, Dice::new(1));
    }

    #[test]
    fn blast_impulses_balance_the_ledger() {
        let p = points()[0];
        let mut world = World::new(DT);
        world.sleep.enabled = false;
        let a = block(
            &mut world,
            20.0,
            DVec3::splat(0.25),
            p + DVec3::new(3.0, 0.25, 0.0),
        );
        let b = block(
            &mut world,
            60.0,
            DVec3::splat(0.3),
            p + DVec3::new(-2.0, 0.3, 4.0),
        );
        let mut objects = vec![
            Unattended::new(a, None, false),
            Unattended::new(b, None, false),
        ];
        let mut swarm = MeteorSwarm {
            caster: DVec3::ZERO,
            dc: DC,
            damage: Damage {
                fire: 1,
                bludgeoning: 1,
            },
            meteors: vec![Meteor {
                point: p,
                spawn_tick: 0,
                start: p,
                velocity: DVec3::ZERO,
                body: None,
                done: true,
            }],
            affected: BTreeSet::new(),
            damaged: BTreeSet::new(),
            impacts: Vec::new(),
        };
        let origin = DVec3::ZERO;
        let mut ledger = Ledger::new(origin, world.momentum(origin));
        let impact = swarm.detonate(&mut world, 0, p, None, &[], &mut objects, &mut |_| 1);
        for hit in &impact.objects {
            ledger.add_impulse(LEDGER_TERM, hit.impulse, world[hit.body].pos);
        }
        let now: Momentum = world.momentum(origin);
        let error = ledger.error(now);
        assert!(error.linear < 1e-12 && error.angular < 1e-12, "{error:?}");
    }

    #[test]
    fn a_checkpoint_mid_fall_replays_identically() {
        let p = points()[0];
        let mut world = flat();
        let mut ids = Vec::new();
        for (i, x) in [2.0, 4.0, 6.0].into_iter().enumerate() {
            ids.push(crate_at(&mut world, p.x + x, p.z + i as f64));
        }
        let mut scene = Scene::cast(world, points(), Dice::new(11));
        scene.objects = ids
            .iter()
            .map(|&id| Unattended::new(id, Some(10), true))
            .collect();
        scene.creatures = vec![dummy(1, p.x, p.z + 3.0), dummy(2, points()[1].x, 40.0)];
        // Mid-fall: three meteors in the air, none detonated.
        scene.run(0.7);
        assert_eq!(scene.swarm.falling().count(), 3);
        assert!(scene.swarm.impacts.is_empty());
        let saved =
            serde_json::to_string(&(&scene.world, &scene.swarm, &scene.objects, &scene.dice))
                .unwrap();
        let (world, swarm, objects, dice): (World, MeteorSwarm, Vec<Unattended>, Dice) =
            serde_json::from_str(&saved).unwrap();
        let mut restored = Scene {
            world,
            swarm,
            creatures: scene.creatures.clone(),
            objects,
            dice,
        };
        let (mut a, mut b) = (Trace::default(), Trace::default());
        let (mut ia, mut ib) = (Vec::new(), Vec::new());
        for _ in 0..ticks(&scene.world, 5.0) {
            ia.extend(scene.step());
            ib.extend(restored.step());
            a.record(&scene.world);
            b.record(&restored.world);
        }
        a.compare(&b, Tolerance::EXACT).unwrap();
        assert_eq!(ia.len(), METEORS);
        assert_eq!(ia, ib);
        assert_eq!(scene.swarm, restored.swarm);
        assert_eq!(scene.objects, restored.objects);
    }
}
