//! Wind Wall in the chamber and the spell playground.
//!
//! SRD 5.2.1: level 3 Evocation, casting time Action, range 120 feet, a wall
//! of strong wind up to 50 feet long, 15 feet high, and 1 foot thick,
//! Concentration up to 1 minute. Each creature in the wall's area when it
//! appears makes a Strength saving throw: 4d8 Bludgeoning on a failure,
//! half on a success. Ordinary projectiles are deflected upward and miss;
//! siege projectiles and spells pass.
//!
//! The mechanics live in [`crate::wind_wall`]. This module casts the wall
//! through the spell world, steps its updraft and collider filters with
//! the props, hands its volume to the projectile simulation (which deflects
//! ordinary flights at their exact swept entry), and records arrow trails
//! for the overlay.
use super::{CHARACTER_HEIGHT, CHARACTER_RADIUS, FEET, SPELL_SAVE_DC, Target, Track};
use crate::play::Game;
use crate::wind_wall::{self as mechanics, Wall, WindWall};
use glam::{DVec2, DVec3, Vec3};
use physics::BodyId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const NAME: &str = "Wind Wall";
/// Row-two slot: Shift+6.
pub const SLOT: u8 = 5;
/// Length of the default straight wall: 30 feet.
pub const STRAIGHT_LENGTH: f64 = 30. * FEET;
/// The straight wall stands this far in front of the caster, m.
pub const STRAIGHT_OFFSET: f64 = 3.75;
/// Radius of the arc wall around the caster, m.
pub const ARC_RADIUS: f64 = 3.;
/// Arc the wall sweeps around the caster, rad: 288 degrees, centered on the
/// caster's facing, so its path stays within 50 feet.
pub const ARC_SWEEP: f64 = 1.6 * std::f64::consts::PI;
/// Legs of the L-shaped wall, m.
pub const L_LEGS: (f64, f64) = (20. * FEET, 20. * FEET);
/// Chamber mana and cooldown: MMO tuning, not tabletop rules.
pub const COST: i32 = 5;
pub const COOLDOWN: f32 = 8.;
/// Most arrow trails and points per trail the overlay keeps.
const TRAILS: usize = 48;
const TRAIL_POINTS: usize = 64;
/// A trail point is kept once the arrow has moved this far, m.
const TRAIL_STEP: f32 = 0.35;

pub const DEF: super::SpellDef = super::SpellDef {
    slot: SLOT,
    key: "wind-wall",
    label: NAME,
    icon: "wind-wall-icon",
    description: "A 15-ft wall of wind: arrows deflect upward, light things fly up, \
        Small fliers bounce off.",
    cost: COST,
    cooldown: COOLDOWN,
    cast,
};

/// The shape the next cast lays down, relative to the caster's facing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    /// 30 feet across the caster's facing, 3.75 m ahead.
    #[default]
    Straight,
    /// A 288-degree arc around the caster, open behind.
    Arc,
    /// Two 20-foot legs meeting ahead of the caster.
    L,
}

/// One standing wall and the cast that holds it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Active {
    pub cast: u64,
    pub caster: u64,
    pub spell: WindWall,
}

/// A siege projectile the wall must not touch, and the creature it was
/// aimed at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Siege {
    pub body: BodyId,
    pub target: u64,
    pub struck: bool,
}

/// An ordinary flight's path, with the point the wall turned it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trail {
    pub id: u32,
    pub points: Vec<Vec3>,
    /// Index into `points` of the deflection, if any.
    pub deflected: Option<usize>,
}

/// Every standing wind wall in a spell world.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Wind {
    pub walls: Vec<Active>,
    pub shape: Shape,
    pub siege: Vec<Siege>,
    pub trails: Vec<Trail>,
    /// Ordinary flights deflected so far.
    pub deflected: u32,
    /// Highest point each lightweight body reached while a wall stood, m.
    pub highest: BTreeMap<u32, f64>,
    /// Ordinary flights first seen while a wall stood.
    #[serde(default)]
    pub launched: u32,
    /// The creature whose health the running arrow count reports.
    #[serde(default)]
    pub watch: Option<u64>,
}

impl Wind {
    pub fn validate(&self, casts: u64) -> Result<(), String> {
        if self.walls.len() > 8
            || self.siege.len() > 16
            || self.trails.len() > TRAILS
            || self.highest.len() > super::MAX_PROPS
            || self.walls.iter().any(|a| {
                a.cast > casts
                    || a.spell.wall.path.len() > 64
                    || a.spell.wall.length() > mechanics::MAX_LENGTH + 1e-9
            })
            || self.trails.iter().any(|t| {
                t.points.len() > TRAIL_POINTS
                    || t.points.iter().any(|p| !p.is_finite())
                    || t.deflected.is_some_and(|i| i >= t.points.len())
            })
        {
            return Err("Invalid wind wall checkpoint".into());
        }
        Ok(())
    }

    /// The walls the projectile simulation deflects against.
    pub fn standing(&self) -> Vec<Wall> {
        self.walls
            .iter()
            .filter(|a| !a.spell.ended)
            .map(|a| a.spell.wall.clone())
            .collect()
    }

    /// Runs before each rigid step: the updraft, and which props the walls
    /// stop. Siege props never meet a wall.
    pub fn before_step(
        &mut self,
        world: &mut physics::World,
        props: &[super::Prop],
        ledger: &mut physics::Ledger,
    ) {
        if self.walls.is_empty() {
            return;
        }
        let affected: Vec<mechanics::Prop> = props
            .iter()
            .filter(|p| {
                !p.removed && !p.spec.secured && !self.siege.iter().any(|s| s.body == p.body)
            })
            .map(|p| mechanics::Prop {
                body: p.body,
                size: size(p.spec.size),
                radius: p.spec.dimensions.x.max(p.spec.dimensions.z) * 0.5,
            })
            .collect();
        for siege in &self.siege {
            mechanics::set_blocked(world, siege.body, false);
        }
        for active in &mut self.walls {
            active.spell.before_step(world, &affected, ledger);
            for prop in &affected {
                if world[prop.body].mass <= mechanics::LIGHTWEIGHT_MASS
                    && active.spell.inside.contains(&prop.body)
                {
                    let y = world[prop.body].pos.y;
                    let peak = self.highest.entry(prop.body.0).or_insert(y);
                    *peak = peak.max(y);
                }
            }
        }
        self.walls.retain(|a| !a.spell.ended);
    }

    /// Ends the wall a cast holds: its colliders leave and its field stops.
    pub fn end_cast(&mut self, cast: u64, world: &mut physics::World, props: &[super::Prop]) {
        let affected: Vec<mechanics::Prop> = props
            .iter()
            .filter(|p| !p.removed && !p.spec.secured)
            .map(|p| mechanics::Prop {
                body: p.body,
                size: size(p.spec.size),
                radius: 0.,
            })
            .collect();
        for active in self.walls.iter_mut().filter(|a| a.cast == cast) {
            active.spell.end(world, &affected);
        }
        self.walls.retain(|a| a.cast != cast);
    }
}

fn size(size: super::Size) -> mechanics::Size {
    match size {
        super::Size::Tiny => mechanics::Size::Tiny,
        super::Size::Small => mechanics::Size::Small,
        super::Size::Medium => mechanics::Size::Medium,
        super::Size::Large => mechanics::Size::Large,
        super::Size::Huge => mechanics::Size::Huge,
    }
}

/// Strength modifiers from SRD stat blocks; the Cultist has STR 11.
pub fn strength_modifier(model: &str) -> i32 {
    match model {
        m if m.starts_with("cultist") => 0,
        // Not an SRD creature: a straw training dummy is as strong as a commoner.
        "dummy" => 0,
        // The ritual's boss has no SRD stat block; this is encounter tuning.
        "claude" => 4,
        _ => 0,
    }
}

/// Height of the static floor under `p`, from the profile's solid geometry
/// (never a creature or a prop), searched from 1 m above `feet` down.
fn ground(scene: &physics::queries::Scene, instance: u64, feet: f64, p: DVec2) -> Option<f64> {
    let filter = physics::queries::Filter::blocking(instance);
    let reach = 1. + mechanics::GROUND_TOLERANCE + 1.;
    scene
        .ray(DVec3::new(p.x, feet + 1., p.y), DVec3::NEG_Y, reach, filter)
        .ok()?
        .hits
        .into_iter()
        .filter(|h| h.collider.life.entity == 0)
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
        .map(|h| h.position.y)
}

/// The path the current shape lays down for a caster at `feet` facing
/// `facing` (a horizontal unit vector, x and z).
pub fn path(shape: Shape, feet: DVec3, facing: DVec2) -> Vec<DVec2> {
    let center = DVec2::new(feet.x, feet.z);
    match shape {
        Shape::Straight => Wall::straight(
            center + facing * STRAIGHT_OFFSET,
            facing.perp(),
            STRAIGHT_LENGTH,
        ),
        Shape::Arc => {
            let heading = facing.y.atan2(facing.x);
            Wall::arc(center, ARC_RADIUS, heading - ARC_SWEEP / 2., ARC_SWEEP)
        }
        Shape::L => Wall::l_shape(
            center + facing * STRAIGHT_OFFSET,
            facing.perp(),
            L_LEGS.0,
            -facing,
            L_LEGS.1,
        ),
    }
}

/// Resolves an admitted Wind Wall from the caster's place and facing.
pub fn cast(game: &mut Game) -> Result<(), String> {
    let caster = game.player_actor();
    let feet = game.player.as_dvec3();
    let facing = DVec2::new(-f64::from(game.yaw.sin()), -f64::from(game.yaw.cos()));
    let points = path(game.spells.wind.shape, feet, facing);
    let wall = if game.colliders.is_empty() {
        Wall::new(points, feet, |_| Some(feet.y))
    } else {
        let instance = game.admission.actor().instance;
        let scene = &game.query_scene;
        Wall::new(points, feet, |p| ground(scene, instance, feet.y, p))
    }
    .map_err(|refusal| format!("Wind Wall refused: {refusal:?}"))?;
    // Creatures in the area when it appears.
    let snapshot = game.snapshot();
    let creatures: Vec<(u64, u32, Vec3)> = game
        .ids
        .iter()
        .filter_map(|(actor, id)| {
            snapshot
                .actors
                .iter()
                .find(|a| a.id == *id && a.alive && a.faction == "undead")
                .map(|a| (*actor, *id, Vec3::from(a.pos)))
        })
        .filter(|(_, _, at)| wall.in_area(at.as_dvec3(), CHARACTER_RADIUS, CHARACTER_HEIGHT))
        .collect();
    let cast = game.spells.begin_cast(caster, true)?;
    let seed = u64::from(game.spells.dice.roll(u32::MAX));
    let now = game.spells.world.time();
    let length = wall.length();
    let spell = WindWall::raise(&mut game.spells.world, wall, now, seed);
    game.spells.wind.walls.push(Active {
        cast,
        caster,
        spell,
    });
    game.spells.owned.push(super::Owned {
        cast,
        spell: NAME.into(),
        caster,
        joints: vec![],
        expires: Some(game.time + mechanics::DURATION as f32),
    });
    game.spells.record(
        game.time,
        NAME,
        format!(
            "{:?} wall {:.0} ft long, 15 ft high, 1 ft thick",
            game.spells.wind.shape,
            length / FEET
        ),
        None,
    );
    for (actor, id, _) in creatures {
        let name = game.actor_name(actor);
        let modifier = strength_modifier(
            &game
                .scene
                .actors
                .iter()
                .find(|a| a.id == actor)
                .map(|a| a.model.clone())
                .unwrap_or_default(),
        );
        let save = game
            .spells
            .dice
            .save(actor, "Strength", modifier, SPELL_SAVE_DC);
        let dice = &mut game.spells.dice;
        let (rolls, damage) = mechanics::appearance_damage(save.success, || dice.roll(8) as i32);
        game.simulation.bow_impact(id, damage)?;
        game.spells.record(
            game.time,
            NAME,
            format!(
                "{name}: STR save {} {:+} = {} vs DC {} {}; 4d8 {rolls:?} -> {damage} bludgeoning",
                save.roll,
                save.modifier,
                save.total,
                save.dc,
                if save.success {
                    "succeeds, half"
                } else {
                    "fails"
                },
            ),
            Some(save),
        );
    }
    Ok(())
}

/// Hands the standing walls to the projectile simulation before it steps.
pub(crate) fn before_flights(game: &mut Game) {
    game.simulation.wind_walls = game.spells.wind.standing();
}

/// Logs the flights the walls deflected, extends the arrow trails, and
/// notes when a siege projectile reaches its target.
pub(crate) fn after_flights(game: &mut Game) -> Result<(), String> {
    let deflections = std::mem::take(&mut game.simulation.deflections);
    let standing = !game.simulation.wind_walls.is_empty();
    let wind = &mut game.spells.wind;
    for (id, point) in &deflections {
        wind.deflected += 1;
        let trail = trail(wind, *id);
        trail.points.push(*point);
        trail.deflected = Some(trail.points.len() - 1);
    }
    if standing || !game.spells.wind.trails.is_empty() {
        let projectiles = game.simulation.snapshot().projectiles;
        let wind = &mut game.spells.wind;
        for p in projectiles
            .iter()
            .filter(|p| p.kind.tag() == crate::rules::FlightTag::Ordinary)
        {
            if !standing && !wind.trails.iter().any(|t| t.id == p.id) {
                continue;
            }
            let at = Vec3::from(p.pos);
            let trail = trail(wind, p.id);
            if trail.points.is_empty() {
                trail.points.push(at);
            }
            if trail.points.len() < TRAIL_POINTS
                && trail
                    .points
                    .last()
                    .is_none_or(|last| last.distance(at) >= TRAIL_STEP)
            {
                trail.points.push(at);
            }
        }
    }
    for index in 0..game.spells.wind.siege.len() {
        let siege = game.spells.wind.siege[index].clone();
        if siege.struck {
            continue;
        }
        let Some(feet) = game.actor_position(siege.target) else {
            continue;
        };
        let rock = game.spells.world[siege.body].pos.as_vec3();
        let flat = Vec3::new(rock.x - feet.x, 0., rock.z - feet.z).length();
        if flat <= CHARACTER_RADIUS as f32 + 0.45
            && (feet.y..=feet.y + CHARACTER_HEIGHT as f32).contains(&rock.y)
        {
            game.spells.wind.siege[index].struck = true;
            let name = game.actor_name(siege.target);
            game.spells.record(
                game.time,
                NAME,
                format!("Siege boulder passed the wall unaffected and struck {name}"),
                None,
            );
        }
    }
    if !deflections.is_empty() {
        status(game);
    }
    Ok(())
}

/// Keeps one running line in the spell log for the arrows the walls
/// turned, in place of a line per arrow.
fn status(game: &mut Game) {
    const PREFIX: &str = "Arrows deflected:";
    let wind = &game.spells.wind;
    let mut text = format!("{PREFIX} {} of {}", wind.deflected, wind.launched);
    if let Some(actor) = wind.watch {
        let health = game
            .frame()
            .actors
            .iter()
            .find(|a| a.actor.id == actor)
            .map(|a| (a.health, a.actor.health));
        if let Some((now, full)) = health {
            let name = game.actor_name(actor);
            text += &format!(
                "; {name} {} ({now}/{full} HP)",
                if now == full { "unhurt" } else { "HIT" }
            );
        }
    }
    game.spells
        .log
        .retain(|r| !(r.spell == NAME && r.text.starts_with(PREFIX)));
    game.spells.record(game.time, NAME, text, None);
}

fn trail(wind: &mut Wind, id: u32) -> &mut Trail {
    if let Some(index) = wind.trails.iter().position(|t| t.id == id) {
        return &mut wind.trails[index];
    }
    wind.launched += 1;
    if wind.trails.len() >= TRAILS {
        wind.trails.remove(0);
    }
    wind.trails.push(Trail {
        id,
        points: vec![],
        deflected: None,
    });
    wind.trails.last_mut().unwrap()
}

/// World-space guide lines for the overlay: each wall's outline, rising
/// wind streaks, and arrow trails (amber before the wall, cyan after).
pub fn guide_lines(game: &Game) -> Vec<(Vec3, Vec3, [f32; 4])> {
    let mut out = vec![];
    let outline = [0.55, 0.9, 1.0, 0.85];
    let streak = [0.85, 0.97, 1.0, 0.55];
    for active in &game.spells.wind.walls {
        let wall = &active.spell.wall;
        let (base, top) = (wall.base as f32, wall.top() as f32);
        let at = |p: DVec2, y: f32| Vec3::new(p.x as f32, y, p.y as f32);
        for pair in wall.path.windows(2) {
            let normal = (pair[1] - pair[0]).normalize().perp() * (mechanics::THICKNESS * 0.5);
            for side in [-1., 1.] {
                let (a, b) = (pair[0] + normal * side, pair[1] + normal * side);
                out.push((at(a, base + 0.02), at(b, base + 0.02), outline));
                out.push((at(a, top), at(b, top), outline));
            }
            let length = pair[0].distance(pair[1]);
            let count = (length / 0.5).ceil().max(1.) as usize;
            for i in 0..count {
                let p = pair[0].lerp(pair[1], (i as f64 + 0.5) / count as f64);
                let phase = (i as f32 * 0.618).fract();
                let height = top - base;
                let y = base + ((game.time * 2.2 + phase) * height / 2.).rem_euclid(height);
                let end = (y + 0.9).min(top);
                out.push((at(p, y), at(p, end), streak));
            }
        }
        for p in &wall.path {
            out.push((at(*p, base), at(*p, top), outline));
        }
    }
    for trail in &game.spells.wind.trails {
        for (i, pair) in trail.points.windows(2).enumerate() {
            let after = trail.deflected.is_some_and(|d| i >= d);
            let color = if after {
                [0.3, 0.95, 1.0, 0.95]
            } else {
                [1.0, 0.7, 0.2, 0.95]
            };
            out.push((pair[0], pair[1], color));
        }
    }
    out
}

/// Index of the prop named `name`.
fn prop(game: &Game, name: &str) -> Result<usize, String> {
    game.spells
        .props
        .iter()
        .position(|p| p.name == name)
        .ok_or_else(|| format!("No prop named {name}"))
}

/// The hall layout the playground scenario uses.
mod hall {
    use glam::Vec3;
    /// The caster's spawn; the wizard faces west (-x) from here.
    pub const CASTER: Vec3 = Vec3::new(5.75, 0., 0.);
    /// The dummy behind the straight wall that the turret shoots at.
    pub const TARGET: Vec3 = Vec3::new(4.2, 0., 1.5);
    /// The dummy the catapult boulder is aimed at.
    pub const SIEGE_TARGET: Vec3 = Vec3::new(4.2, 0., -1.);
    /// Two dummies standing where the arc wall rises.
    pub const ARC_FAILS: Vec3 = Vec3::new(6.78, 0., -2.82);
    pub const ARC_SAVES: Vec3 = Vec3::new(4.25, 0., 2.6);
    /// The arrow turret's muzzle.
    pub const TURRET: Vec3 = Vec3::new(-5., 1.4, 0.);
    /// Where the boulder rests before the catapult throws it.
    pub const BOULDER: Vec3 = Vec3::new(-7., 0.45, -1.);
    /// Flight time the catapult aims for, s.
    pub const BOULDER_FLIGHT: f32 = 1.2;
}

const TARGET_ID: u64 = 101;
const SIEGE_ID: u64 = 102;
const FAILS_ID: u64 = 103;
const SAVES_ID: u64 = 104;
const VOLLEY: usize = 20;
/// Paper and leaf sheets in the pile.
const SHEETS: usize = 12;

fn turret_arrow(game: &mut Game) -> Result<(), String> {
    // The turret walks its aim across the dummy's body, arrow by arrow.
    let fired = game.spells.wind.trails.len();
    let spread = ((fired % 5) as f32 - 2.) * 0.12;
    let aim = hall::TARGET + Vec3::new(0., 1.1 + spread, spread);
    let direction = (aim - hall::TURRET).normalize();
    game.simulation
        .launch_bow(hall::TURRET.to_array(), direction.to_array())
}

fn catapult(game: &mut Game) -> Result<(), String> {
    let index = prop(game, "Catapult boulder")?;
    let start = game.spells.prop_center(index).as_vec3();
    let t = hall::BOULDER_FLIGHT;
    let aim = hall::SIEGE_TARGET + Vec3::Y * 1.0;
    let gravity = Vec3::Y * -(super::GRAVITY as f32);
    let velocity = (aim - start - gravity * (0.5 * t * t)) / t;
    let body = game.spells.props[index].body;
    let mass = game.spells.world[body].mass;
    let at = game.spells.world[body].pos;
    if let Some(feet) = game.actor_position(SIEGE_ID) {
        game.spells.track(Track {
            label: "Siege dummy".into(),
            target: Target::Actor(SIEGE_ID),
            spell: NAME.into(),
            at: game.time,
            start: feet.as_dvec3(),
            requested: 0.,
        });
    }
    game.spells.record(
        game.time,
        NAME,
        "Catapult hurls a 150 kg boulder (siege): the wall does not touch it".into(),
        None,
    );
    game.spells
        .impulse_prop(index, velocity.as_dvec3() * mass, at, "catapult")
}

fn bellows(game: &mut Game) -> Result<(), String> {
    // A scripted gust sweeps the pile toward the wall.
    for index in 0..game.spells.props.len() {
        if game.spells.props[index].spec.kind != super::PropKind::Sheet {
            continue;
        }
        let body = game.spells.props[index].body;
        let mass = game.spells.world[body].mass;
        let at = game.spells.world[body].pos;
        game.spells
            .impulse_prop(index, DVec3::X * 3.2 * mass, at, "bellows")?;
    }
    Ok(())
}

fn throw_crate(game: &mut Game) -> Result<(), String> {
    let index = prop(game, "Thrown crate")?;
    let body = game.spells.props[index].body;
    let mass = game.spells.world[body].mass;
    let at = game.spells.world[body].pos;
    game.spells.track(Track {
        label: "Thrown crate".into(),
        target: Target::Prop(index),
        spell: NAME.into(),
        at: game.time,
        start: game.spells.prop_center(index),
        requested: 0.,
    });
    game.spells
        .impulse_prop(index, DVec3::new(6.5, 3.5, 0.) * mass, at, "throw")?;
    game.spells.record(
        game.time,
        NAME,
        "A 20 kg crate is thrown at the wall (airborne, Small)".into(),
        None,
    );
    Ok(())
}

fn shove_crate(game: &mut Game) -> Result<(), String> {
    let index = prop(game, "Pushed crate")?;
    game.spells.track(Track {
        label: "Pushed crate".into(),
        target: Target::Prop(index),
        spell: NAME.into(),
        at: game.time,
        start: game.spells.prop_center(index),
        requested: 10. * FEET,
    });
    game.spells
        .push_prop(index, DVec3::X, 10. * FEET, "shove")?;
    game.spells.record(
        game.time,
        NAME,
        "A 20 kg crate is shoved along the floor (grounded)".into(),
        None,
    );
    Ok(())
}

fn choose_arc(game: &mut Game) -> Result<(), String> {
    game.spells.wind.shape = Shape::Arc;
    Ok(())
}

/// Arrows from the west, north, and south at the wizard inside the arc.
fn three_sides(game: &mut Game) -> Result<(), String> {
    let chest = hall::CASTER + Vec3::Y * 1.3;
    for from in [
        hall::CASTER + Vec3::new(-8., 1.4, 0.),
        hall::CASTER + Vec3::new(0., 1.4, 8.),
        hall::CASTER + Vec3::new(0.6, 1.4, -8.),
    ] {
        game.simulation
            .launch_bow(from.to_array(), (chest - from).normalize().to_array())?;
    }
    Ok(())
}

/// The playground recording: a turret's 20 arrows at a dummy behind a
/// straight wall, a catapult boulder that passes, paper and leaves that
/// spiral out of the top, a thrown crate that bounces beside a pushed crate
/// that slides through, then an arc around the caster that deflects arrows
/// from three sides and damages the two dummies standing in it.
pub fn scenario() -> crate::playground::Scenario {
    use crate::play::Ability;
    use crate::playground::{Cue, Device, Scenario, Shot, Step, creature};
    use std::f32::consts::FRAC_PI_2;
    Scenario {
        key: "wind-wall",
        title: NAME,
        srd: "Level 3 Evocation | Range 120 ft | Wall 50 x 15 x 1 ft | STR save | 4d8 Bludgeoning, half on save",
        seed: 457,
        live: 17.5,
        replay: (2.0, 3.0),
        setup: |scene, _| {
            for (id, name, at) in [
                (TARGET_ID, "Target dummy", hall::TARGET),
                (SIEGE_ID, "Siege dummy", hall::SIEGE_TARGET),
                (FAILS_ID, "Dummy D", hall::ARC_FAILS),
                (SAVES_ID, "Dummy E", hall::ARC_SAVES),
            ] {
                scene
                    .actors
                    .push(creature(id, name, "dummy", at, -FRAC_PI_2, 100));
            }
            Ok(())
        },
        populate: |game, _| {
            use super::{PropKind, PropSpec};
            let crate_spec = PropSpec::reference(PropKind::Crate);
            // The turret: two secured crates under the muzzle.
            for (name, y) in [("Arrow turret", 0.3), ("Arrow turret top", 0.9)] {
                game.spawn_prop(
                    name,
                    crate_spec.clone().secured(),
                    Vec3::new(hall::TURRET.x - 0.4, y, hall::TURRET.z),
                    0.,
                )?;
            }
            let index = game.spawn_prop(
                "Catapult boulder",
                PropSpec::reference(PropKind::Boulder),
                hall::BOULDER,
                0.,
            )?;
            let body = game.spells.props[index].body;
            game.spells.wind.siege.push(Siege {
                body,
                target: SIEGE_ID,
                struck: false,
            });
            game.spells.wind.watch = Some(TARGET_ID);
            // Paper and leaves: sheets of 2 kg or less, so the wall lifts
            // them. A row on the floor and a second layer on every other one.
            for i in 0..SHEETS {
                let mut sheet = PropSpec::reference(PropKind::Sheet);
                sheet.mass = 0.3 + 0.15 * i as f64;
                let z = 0.4 + 0.5 * (i % 8) as f32;
                let y = if i < 8 { 0.025 } else { 0.075 };
                let z = if i < 8 { z } else { 0.4 + 1.0 * (i - 8) as f32 };
                game.spawn_prop(&format!("Sheet {}", i + 1), sheet, Vec3::new(0.9, y, z), 0.)?;
            }
            game.spawn_prop(
                "Thrown crate",
                crate_spec.clone(),
                Vec3::new(0.2, 0.3, -2.6),
                0.,
            )?;
            game.spawn_prop("Pushed crate", crate_spec, Vec3::new(0.6, 0.3, -3.8), 0.)?;
            // Dummy D fails its Strength save and Dummy E succeeds.
            game.spells.dice.force_save(FAILS_ID, 3)?;
            game.spells.dice.force_save(SAVES_ID, 18)?;
            Ok(())
        },
        script: || {
            let mut cues = vec![
                Cue {
                    at: 0.4,
                    step: Step::Face(FRAC_PI_2),
                },
                Cue {
                    at: 0.8,
                    step: Step::Cast(Ability::Spell(SLOT)),
                },
                Cue {
                    at: 6.8,
                    step: Step::Device(Device(catapult)),
                },
                Cue {
                    at: 9.6,
                    step: Step::Device(Device(bellows)),
                },
                Cue {
                    at: 12.4,
                    step: Step::Device(Device(throw_crate)),
                },
                Cue {
                    at: 12.8,
                    step: Step::Device(Device(shove_crate)),
                },
                Cue {
                    at: 13.9,
                    step: Step::Device(Device(choose_arc)),
                },
                Cue {
                    at: 14.1,
                    step: Step::Cast(Ability::Spell(SLOT)),
                },
            ];
            for i in 0..VOLLEY {
                cues.push(Cue {
                    at: 1.5 + 0.25 * i as f32,
                    step: Step::Device(Device(turret_arrow)),
                });
            }
            for i in 0..2 {
                cues.push(Cue {
                    at: 15.0 + 0.8 * i as f32,
                    step: Step::Device(Device(three_sides)),
                });
            }
            cues
        },
        camera: || {
            // From the north (+z) the turret is on the left, the wall on the
            // right, and the arrows kick up below the overlay panel.
            let volley = (Vec3::new(-1.4, 2.2, 6.6), Vec3::new(-0.4, 2.5, -0.4));
            // The boulder from the south, followed across the wall.
            let launch = (Vec3::new(-6.5, 2.2, -7.5), Vec3::new(-6.0, 1.8, -1.0));
            let crossing = (Vec3::new(-1.5, 2.6, -8.0), Vec3::new(-1.0, 2.4, -1.0));
            let strike = (Vec3::new(3.0, 2.2, -7.5), Vec3::new(3.6, 1.8, -1.0));
            // Square to the wall's face, the pile mid-frame.
            let pile = (Vec3::new(-6.5, 2.6, 0.8), Vec3::new(2.0, 4.2, 0.8));
            let crates = (Vec3::new(0.9, 1.9, -7.4), Vec3::new(1.4, 1.6, -3.0));
            let arc = (Vec3::new(1.2, 8.5, -7.5), Vec3::new(5.0, 0.8, 0.5));
            [
                (0., volley),
                (6.3, volley),
                (6.6, launch),
                (6.9, launch),
                (7.4, crossing),
                (8.0, strike),
                (8.9, strike),
                (9.3, pile),
                (11.9, pile),
                (12.2, crates),
                (13.7, crates),
                (14.0, arc),
                (17.5, arc),
            ]
            .into_iter()
            .map(|(at, (eye, target))| Shot { at, eye, target })
            .collect()
        },
        replay_camera: (Vec3::new(-0.2, 1.9, 4.6), Vec3::new(1.4, 2.1, 0.6)),
        check: |game| {
            let health = |actor: u64| {
                game.frame()
                    .actors
                    .iter()
                    .find(|a| a.actor.id == actor)
                    .map_or(0, |a| a.health)
            };
            let wind = &game.spells.wind;
            if wind.deflected < (VOLLEY + 6) as u32 {
                return Err(format!("Only {} arrows were deflected", wind.deflected));
            }
            if health(TARGET_ID) != 100 {
                return Err(format!(
                    "The target dummy took damage: {}",
                    health(TARGET_ID)
                ));
            }
            if !wind.siege.iter().all(|s| s.struck) {
                return Err("The siege boulder never reached its dummy".into());
            }
            for (i, p) in game.spells.props.iter().enumerate() {
                if p.spec.kind != super::PropKind::Sheet {
                    continue;
                }
                let peak = wind.highest.get(&p.body.0).copied().unwrap_or(0.);
                if peak < mechanics::HEIGHT {
                    return Err(format!("{} (#{i}) only rose to {peak:.2} m", p.name));
                }
            }
            let x = |name: &str| -> Result<f64, String> {
                Ok(game.spells.prop_center(prop(game, name)?).x)
            };
            if x("Thrown crate")? > 1.85 {
                return Err("The thrown crate crossed the wall".into());
            }
            if x("Pushed crate")? < 2.3 {
                return Err("The pushed crate did not slide through the wall".into());
            }
            let (fails, saves) = (100 - health(FAILS_ID), 100 - health(SAVES_ID));
            if !(4..=32).contains(&fails) || !(2..=16).contains(&saves) {
                return Err(format!(
                    "Appearance damage {fails} (failed) and {saves} (saved) is outside 4d8 and half"
                ));
            }
            let error = game.spells.ledger_error();
            if error.linear > super::LEDGER_TOLERANCE || error.angular > super::LEDGER_TOLERANCE {
                return Err(format!("Ledger residual {error:?}"));
            }
            Ok(())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::Ability;
    use crate::playground::{creature, hall};

    fn hall_game(dummies: &[(u64, Vec3)]) -> Game {
        let hall = hall().unwrap();
        let mut scene = hall.scene.clone();
        for (id, at) in dummies {
            scene
                .actors
                .push(creature(*id, "Dummy", "dummy", *at, 0., 100));
        }
        let mut game = Game::new(scene).unwrap();
        game.face(std::f32::consts::FRAC_PI_2).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game
    }

    fn settle(game: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 30.) as u32 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
    }

    fn health(game: &Game, actor: u64) -> u32 {
        game.frame()
            .actors
            .iter()
            .find(|a| a.actor.id == actor)
            .unwrap()
            .health
    }

    #[test]
    fn the_spell_is_on_the_second_action_bar_row() {
        let def = super::super::spell_in_slot(SLOT).unwrap();
        assert_eq!(def.key, "wind-wall");
        assert_eq!(def.icon, "wind-wall-icon");
        assert_eq!(Ability::Spell(SLOT).label(), NAME);
    }

    #[test]
    fn player_arrows_deflect_and_a_dummy_behind_the_wall_takes_no_hits() {
        // The wall stands at x = 2; the dummy is behind it.
        let mut game = hall_game(&[(101, Vec3::new(3.5, 0., 0.))]);
        game.activate(Ability::Spell(SLOT)).unwrap();
        assert_eq!(game.spells.wind.walls.len(), 1);
        for i in 0..10 {
            let from = Vec3::new(-6., 1.4, -0.5 + 0.1 * i as f32);
            let aim = (Vec3::new(3.5, 1.1, 0.) - from).normalize();
            game.simulation
                .launch_bow(from.to_array(), aim.to_array())
                .unwrap();
            settle(&mut game, 0.2);
        }
        settle(&mut game, 1.);
        assert_eq!(game.spells.wind.deflected, 10);
        assert_eq!(health(&game, 101), 100);
        // A spell bolt passes the same wall.
        let before = game.spells.wind.deflected;
        let from = Vec3::new(-6., 1.4, 0.);
        game.simulation
            .cast(
                crate::rules::Spell::Firebolt,
                from.to_array(),
                (Vec3::new(3.5, 1.1, 0.) - from).normalize().to_array(),
            )
            .unwrap();
        settle(&mut game, 1.);
        assert_eq!(game.spells.wind.deflected, before);
        assert!(health(&game, 101) < 100, "the firebolt hits");
    }

    #[test]
    fn creatures_in_the_footprint_take_four_d8_or_half() {
        // The default wall stands 3.75 m west of the caster at (5.75, 0, 0).
        let mut game = hall_game(&[
            (101, Vec3::new(2.0, 0., 1.)),
            (102, Vec3::new(2.0, 0., -1.)),
        ]);
        game.spells.dice.force_save(101, 2).unwrap();
        game.spells.dice.force_save(102, 19).unwrap();
        game.activate(Ability::Spell(SLOT)).unwrap();
        settle(&mut game, 0.2);
        let failed = 100 - health(&game, 101);
        let saved = 100 - health(&game, 102);
        assert!((4..=32).contains(&failed), "{failed}");
        assert!((2..=16).contains(&saved), "{saved}");
        let lines: Vec<_> = game.spells.log.iter().map(|r| r.text.clone()).collect();
        assert!(lines.iter().any(|l| l.contains("fails")), "{lines:?}");
        assert!(
            lines.iter().any(|l| l.contains("succeeds, half")),
            "{lines:?}"
        );
    }

    #[test]
    fn a_second_cast_ends_the_first_and_concentration_removes_the_wall() {
        let mut game = hall_game(&[(101, Vec3::new(-6., 0., 4.))]);
        game.activate(Ability::Spell(SLOT)).unwrap();
        let first: Vec<_> = game.spells.wind.walls[0].spell.bodies.clone();
        settle(&mut game, COOLDOWN + 0.5);
        game.spells.wind.shape = Shape::Arc;
        game.activate(Ability::Spell(SLOT)).unwrap();
        assert_eq!(game.spells.wind.walls.len(), 1);
        assert!(first.iter().all(|b| game.spells.world[*b].removed));
        let player = game.player_actor();
        game.spells.end_concentration(player).unwrap();
        assert!(game.spells.wind.walls.is_empty());
        before_flights(&mut game);
        assert!(game.simulation.wind_walls.is_empty());
    }

    #[test]
    fn a_checkpoint_with_a_deflected_arrow_in_flight_replays_identically() {
        let mut game = hall_game(&[(101, Vec3::new(3.5, 0., 0.))]);
        game.activate(Ability::Spell(SLOT)).unwrap();
        let from = Vec3::new(-6., 1.4, 0.);
        game.simulation
            .launch_bow(
                from.to_array(),
                (Vec3::new(3.5, 1.1, 0.) - from).normalize().to_array(),
            )
            .unwrap();
        settle(&mut game, 0.5);
        assert_eq!(game.spells.wind.deflected, 1);
        assert!(!game.snapshot().projectiles.is_empty(), "still in flight");
        let saved = game.checkpoint().unwrap();
        let mut restored = Game::restore(&saved).unwrap();
        settle(&mut game, 1.);
        settle(&mut restored, 1.);
        assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
    }

    #[test]
    fn the_playground_scenario_meets_every_beat() {
        let mut run = crate::playground::Run::new(scenario()).unwrap();
        while !run.done() {
            run.advance().unwrap();
        }
        (run.scenario.check)(run.result()).unwrap();
        assert_eq!(run.replay_identical, Some(true));
    }
}
