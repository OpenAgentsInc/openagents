//! Gust of Wind on physics.
//!
//! SRD 5.2.1: level 2 Evocation, casting time Action, range Self, a Line
//! 60 feet long and 10 feet wide, concentration up to one minute. Each
//! creature in the Line makes a Strength saving throw or is pushed 15 feet
//! along it, again whenever it ends a turn there; moving closer to the
//! caster costs 2 feet of movement per foot; unprotected flames go out and
//! protected ones have a 50 percent chance to. A Bonus Action on a later
//! turn points the Line another way.
//!
//! The rule and field math lives in [`crate::gust`]. This module wires it
//! into the chamber: the cast and re-aim, the six-second save clocks, the
//! calibrated pushes, the drag impulses on props (recorded in the ledger as
//! `spell:gust_of_wind`), the drag on ordinary flights, the scene flames,
//! and the playground scenario.
use super::{PropKind, PropSpec, SPELL_SAVE_DC, Target, Track};
use crate::gust::{self, Creature, Event, Flame, Gust, SaveReason};
use crate::play::Game;
use glam::{DVec3, Vec3};
use serde::{Deserialize, Serialize};

pub const NAME: &str = "Gust of Wind";
/// The `--spell-playground` key and the evidence file stem.
pub const KEY: &str = "gust-of-wind";
/// Row-two slot 4: Shift+5.
pub const SLOT: u8 = 4;
/// Chamber mana for the cast and for each re-aim: MMO tuning.
pub const COST: i32 = 3;
/// The slot's cooldown is the Bonus Action re-aim's, one round.
pub const COOLDOWN: f32 = gust::REAIM_COOLDOWN as f32;
/// Ledger term for the wind's impulses on props.
pub const TERM: &str = "spell:gust_of_wind";
/// Speed of the ordinary arrows the playground's archer looses, m/s.
const ARROW_SPEED: f32 = 24.;
/// The chamber's collision profile, where ritual candles stand.
const CHAMBER_PROFILE: &str = "original-chamber-v1";
/// Seconds between walker speed readings on the overlay.
const WALKER_WINDOW: f32 = 0.5;

pub const DEF: super::SpellDef = super::SpellDef {
    slot: SLOT,
    key: KEY,
    label: NAME,
    icon: "gust-of-wind-icon",
    description: "A 60-foot Line of wind pushes creatures 15 feet and blows out candles; \
                  cast again to re-aim it",
    cost: COST,
    cooldown: COOLDOWN,
    cast,
};

/// Strength save modifiers from SRD stat blocks; the Cultist has STR 11.
pub fn strength_modifier(model: &str) -> i32 {
    match model {
        m if m.starts_with("cultist") => 0,
        // Not an SRD creature: a straw training dummy is as strong as a commoner.
        "dummy" => 0,
        // The ritual's boss has no SRD stat block; this is encounter tuning.
        "claude" => 5,
        _ => 0,
    }
}

/// Drag coefficient of a prop's box, by kind.
pub fn drag_coefficient(kind: PropKind) -> f64 {
    match kind {
        // A flat sheet presents its edge; its drag over that edge is a
        // flat plate's. It weighs 0.5 kg, so it flies.
        PropKind::Paper | PropKind::Basket | PropKind::Sheet => 1.2,
        // A rough sphere: 99 N on a 150 kg boulder, far below its friction.
        PropKind::Boulder => 0.5,
        PropKind::Barrel => 0.9,
        PropKind::TrainingDummy | PropKind::Anvil => 1.0,
        PropKind::Crate | PropKind::StoneBlock | PropKind::SpellBody => 1.05,
    }
}

/// One active Line and the cast that holds it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Active {
    pub cast: u64,
    pub gust: Gust,
}

/// Arrows a scenario's archer looses at `at`, one from each origin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Volley {
    pub at: f32,
    pub direction: DVec3,
    /// Label and release point of each arrow.
    pub arrows: Vec<(String, DVec3)>,
    /// Projectile IDs once loosed.
    pub loosed: Vec<u32>,
    /// Seconds after the release that the overlay reports the arrows.
    pub report_after: f32,
    pub reported: bool,
}

/// A creature whose speed toward the caster the overlay shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Walker {
    pub actor: u64,
    pub name: String,
    pub since: f32,
    pub from: DVec3,
    pub inside: bool,
}

/// Gust of Wind's share of the spell world, saved with every checkpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub lines: Vec<Active>,
    /// Scene lights tagged as flames: candles and torches (unprotected)
    /// and lanterns (protected).
    pub flames: Vec<Flame>,
    /// The chamber's ritual candles have been placed.
    pub chamber_lit: bool,
    pub volleys: Vec<Volley>,
    pub walkers: Vec<Walker>,
    /// Props the wind has started tracking, by index.
    pub tracked: Vec<usize>,
}

impl State {
    /// Concentration on `cast` ended: its Line is gone.
    pub fn end(&mut self, cast: u64) {
        self.lines.retain(|l| l.cast != cast);
    }
    pub fn validate(&self, casts: u64) -> Result<(), String> {
        if self.lines.len() > 8
            || self.flames.len() > 64
            || self.volleys.len() > 16
            || self.walkers.len() > 8
            || self.tracked.len() > super::MAX_PROPS
            || self.lines.iter().any(|l| {
                l.cast > casts || !l.gust.line.origin.is_finite() || !l.gust.until.is_finite()
            })
            || self.flames.iter().any(|f| !f.position.is_finite())
            || self.volleys.iter().any(|v| {
                v.arrows.len() > 4
                    || v.loosed.len() > 4
                    || !v.at.is_finite()
                    || !v.direction.is_finite()
                    || v.arrows.iter().any(|(_, p)| !p.is_finite())
            })
            || self.walkers.iter().any(|w| !w.from.is_finite())
        {
            return Err("Invalid Gust of Wind checkpoint".into());
        }
        Ok(())
    }
    /// A candle (unprotected) or a lantern (protected) at `position`.
    pub fn add_flame(&mut self, position: DVec3, protected: bool) -> Result<u64, String> {
        if self.flames.len() >= 64 || !position.is_finite() {
            return Err("Invalid flame".into());
        }
        let id = self.flames.len() as u64 + 1;
        self.flames.push(Flame {
            id,
            position,
            protected,
            lit: true,
        });
        Ok(id)
    }
}

fn flame_name(flame: &Flame) -> String {
    if flame.protected {
        format!("Lantern {}", flame.id)
    } else {
        format!("Candle {}", flame.id)
    }
}

/// Whether a lit flame dances in an active Line, for presentation.
pub fn flickers(state: &State, time: f32, flame: &Flame) -> bool {
    flame.lit
        && state
            .lines
            .iter()
            .any(|l| l.gust.flickers(f64::from(time), flame.position))
}

/// A walking creature's velocity with its approach to every active Line's
/// caster halved, as the movement cost requires.
pub fn approach(spells: &super::SpellWorld, time: f32, feet: DVec3, velocity: DVec3) -> DVec3 {
    spells
        .gust
        .lines
        .iter()
        .fold(velocity, |v, l| l.gust.approach(f64::from(time), feet, v))
}

fn facing(game: &Game) -> DVec3 {
    DVec3::new(-f64::from(game.yaw.sin()), 0., -f64::from(game.yaw.cos()))
}

/// Living hostile creatures: (scene actor, simulation ID, feet, Strength).
fn creatures(game: &Game) -> Vec<(u64, u32, DVec3, i32)> {
    let snapshot = game.snapshot();
    game.ids
        .iter()
        .filter_map(|(actor, id)| {
            snapshot
                .actors
                .iter()
                .find(|a| a.id == *id && a.alive && a.faction == "undead")
                .map(|a| {
                    let model = game
                        .scene
                        .actors
                        .iter()
                        .find(|s| s.id == *actor)
                        .map(|s| s.model.as_str())
                        .unwrap_or_default();
                    let feet = game
                        .npc_characters
                        .get(actor)
                        .filter(|c| c.feet.as_vec3() == Vec3::from(a.pos))
                        .map_or(Vec3::from(a.pos).as_dvec3(), |c| c.feet);
                    (*actor, *id, feet, strength_modifier(model))
                })
        })
        .collect()
}

fn as_creatures(list: &[(u64, u32, DVec3, i32)]) -> Vec<Creature> {
    list.iter()
        .map(|(actor, _, feet, strength)| Creature {
            id: *actor,
            feet: *feet,
            strength: *strength,
        })
        .collect()
}

/// Runs an admitted cast: a new Line from the caster's feet along their
/// facing, or, while the caster already holds one, the Bonus Action re-aim.
fn cast(game: &mut Game) -> Result<(), String> {
    light_chamber(game)?;
    let caster = game.player_actor();
    let time = f64::from(game.time);
    let aim = facing(game);
    let list = creatures(game);
    if let Some(active) = game
        .spells
        .gust
        .lines
        .iter_mut()
        .find(|l| l.gust.caster == caster && l.gust.active(time))
    {
        active.gust.reaim(aim, time)?;
        active.gust.sweep(&as_creatures(&list));
        let direction = active.gust.line.direction;
        game.spells.record(
            game.time,
            NAME,
            format!(
                "Bonus Action: the Line turns to ({:.0}, {:.0})",
                direction.x, direction.z
            ),
            None,
        );
    } else {
        let gust = Gust::cast(caster, game.player.as_dvec3(), aim, time)?;
        let cast = game.spells.begin_cast(caster, true)?;
        game.spells.gust.lines.push(Active { cast, gust });
        game.spells.record(
            game.time,
            NAME,
            "A 60 x 10 ft Line of 20 m/s wind blasts from the caster".into(),
            None,
        );
    }
    saves(game)?;
    flames(game);
    Ok(())
}

/// Strength saves that are due, and the pushes failed saves earn.
fn saves(game: &mut Game) -> Result<(), String> {
    let time = f64::from(game.time);
    let list = creatures(game);
    let all = as_creatures(&list);
    for index in 0..game.spells.gust.lines.len() {
        let due = game.spells.gust.lines[index].gust.due(time, &all);
        for (actor, reason) in due {
            let Some(&(_, id, feet, modifier)) = list.iter().find(|c| c.0 == actor) else {
                continue;
            };
            let name = game.actor_name(actor);
            let save = game
                .spells
                .dice
                .save(actor, "Strength", modifier, SPELL_SAVE_DC);
            let why = match reason {
                SaveReason::Cast => "",
                SaveReason::Entry => " (entered)",
                SaveReason::Turn => " (turn end)",
            };
            game.spells.record(
                game.time,
                NAME,
                format!(
                    "{name}{why}: STR save {} {:+} = {} vs DC {} {}{}",
                    save.roll,
                    save.modifier,
                    save.total,
                    save.dc,
                    if save.success { "succeeds" } else { "fails" },
                    if save.success { "" } else { ", pushed 15 ft" }
                ),
                Some(save.clone()),
            );
            game.spells.track(Track {
                label: name,
                target: Target::Actor(actor),
                spell: NAME.into(),
                at: game.time,
                start: feet,
                requested: if save.success {
                    0.
                } else {
                    gust::PUSH_DISTANCE
                },
            });
            if save.success {
                continue;
            }
            let Event::Push {
                direction,
                distance,
                ..
            } = game.spells.gust.lines[index].gust.push(actor)
            else {
                continue;
            };
            if game.colliders.is_empty() {
                // Scenes without collision keep the kinematic shove.
                let push = (direction * distance).as_vec3();
                game.controls.displace(id, push);
                game.simulation
                    .place_chamber_actor(id, (feet.as_vec3() + push).to_array(), 0.)?;
                continue;
            }
            let character = game
                .npc_characters
                .entry(actor)
                .or_insert_with(|| physics::character::Character::new(feet));
            if character.feet != feet {
                *character = physics::character::Character::new(feet);
            }
            character.add_velocity(direction * physics::character::Character::push_speed(distance));
            game.controls.displace(id, Vec3::ZERO);
        }
    }
    Ok(())
}

/// Candles in a Line go out; lanterns roll once on the seeded dice.
fn flames(game: &mut Game) {
    let time = f64::from(game.time);
    let mut events = vec![];
    {
        let spells = &mut game.spells;
        let state = &mut spells.gust;
        let dice = &mut spells.dice;
        for line in &mut state.lines {
            events.extend(
                line.gust
                    .flames(time, &mut state.flames, &mut || dice.roll(100)),
            );
        }
    }
    for event in events {
        let text = match event {
            Event::Extinguished { flame } => {
                let name = game
                    .spells
                    .gust
                    .flames
                    .iter()
                    .find(|f| f.id == flame)
                    .map(flame_name)
                    .unwrap_or_default();
                format!("{name} goes out")
            }
            Event::Gutter { flame, roll, out } => {
                let name = game
                    .spells
                    .gust
                    .flames
                    .iter()
                    .find(|f| f.id == flame)
                    .map(flame_name)
                    .unwrap_or_default();
                format!(
                    "{name}: d100 {roll} {} {}",
                    if out { "<=" } else { ">" },
                    if out {
                        "50, goes out"
                    } else {
                        "50, dances but stays lit"
                    }
                )
            }
            _ => continue,
        };
        game.spells.record(game.time, NAME, text, None);
    }
}

/// The wind's drag on every unsecured prop in a Line, as one impulse over
/// the frame's `dt` at the middle of the prop's part inside the Line.
fn props(game: &mut Game, dt: f64) -> Result<(), String> {
    let time = f64::from(game.time);
    for index in 0..game.spells.props.len() {
        let prop = &game.spells.props[index];
        if prop.removed {
            continue;
        }
        let spec = prop.spec.clone();
        let name = prop.name.clone();
        let body = *game.spells.body(prop.body);
        let center = game.spells.prop_center(index);
        for l in 0..game.spells.gust.lines.len() {
            let Some((impulse, at)) = game.spells.gust.lines[l].gust.wind_on(
                time,
                center,
                body.orientation,
                spec.dimensions * 0.5,
                body.vel,
                spec.mass,
                drag_coefficient(spec.kind),
                dt,
            ) else {
                continue;
            };
            if !game.spells.gust.tracked.contains(&index) {
                game.spells.gust.tracked.push(index);
                game.spells.track(Track {
                    label: name.clone(),
                    target: Target::Prop(index),
                    spell: NAME.into(),
                    at: game.time,
                    start: center,
                    requested: 0.,
                });
            }
            game.spells.impulse_prop(index, impulse, at, TERM)?;
        }
    }
    Ok(())
}

/// Drag on ordinary flights crossing a Line; spell flights fly through.
fn flights(game: &mut Game, dt: f64) {
    let time = f64::from(game.time);
    let lines = &game.spells.gust.lines;
    if lines.is_empty() {
        return;
    }
    game.simulation.bend_flights(|tag, position, velocity| {
        if tag != crate::rules::FlightTag::Ordinary {
            return Vec3::ZERO;
        }
        lines
            .iter()
            .map(|l| {
                l.gust
                    .arrow_drag(time, position.as_dvec3(), velocity.as_dvec3(), dt)
            })
            .sum::<DVec3>()
            .as_vec3()
    });
}

/// Looses due volleys and reports them once they have flown.
fn volleys(game: &mut Game) -> Result<(), String> {
    for index in 0..game.spells.gust.volleys.len() {
        let volley = game.spells.gust.volleys[index].clone();
        if volley.loosed.is_empty() && game.time >= volley.at {
            let mut ids = vec![];
            for (_, origin) in &volley.arrows {
                game.simulation.launch_bow_at(
                    0,
                    origin.as_vec3().to_array(),
                    volley.direction.as_vec3().to_array(),
                )?;
                let id = game
                    .snapshot()
                    .projectiles
                    .iter()
                    .map(|p| p.id)
                    .max()
                    .ok_or("The arrow was not loosed")?;
                ids.push(id);
            }
            game.spells.record(
                game.time,
                NAME,
                format!("Archer looses {} arrows at {ARROW_SPEED:.0} m/s", ids.len()),
                None,
            );
            game.spells.gust.volleys[index].loosed = ids;
        } else if !volley.loosed.is_empty()
            && !volley.reported
            && game.time >= volley.at + volley.report_after
        {
            let projectiles = game.snapshot().projectiles;
            let parts: Vec<String> = volley
                .arrows
                .iter()
                .zip(&volley.loosed)
                .map(|((label, origin), id)| {
                    projectiles.iter().find(|p| p.id == *id).map_or_else(
                        || format!("{label}: landed"),
                        |p| {
                            format!(
                                "{label} {:.1} m/s, {:.1} m",
                                Vec3::from(p.vel).length(),
                                Vec3::from(p.pos).distance(origin.as_vec3())
                            )
                        },
                    )
                })
                .collect();
            game.spells.record(
                game.time,
                NAME,
                format!("After {:.1} s: {}", volley.report_after, parts.join("; ")),
                None,
            );
            game.spells.gust.volleys[index].reported = true;
        }
    }
    Ok(())
}

/// Shows each walker's speed in its overlay label, and logs when a Line
/// starts or stops slowing its approach.
fn walkers(game: &mut Game) {
    let time = f64::from(game.time);
    for index in 0..game.spells.gust.walkers.len() {
        let walker = game.spells.gust.walkers[index].clone();
        let Some(feet) = game.actor_position(walker.actor).map(|p| p.as_dvec3()) else {
            continue;
        };
        if game.time - walker.since < WALKER_WINDOW {
            continue;
        }
        let moved = DVec3::new(feet.x - walker.from.x, 0., feet.z - walker.from.z).length();
        let speed = moved / f64::from(game.time - walker.since);
        let inside = game
            .spells
            .gust
            .lines
            .iter()
            .any(|l| l.gust.active(time) && l.gust.line.contains(feet));
        if inside != walker.inside {
            game.spells.record(
                game.time,
                NAME,
                format!(
                    "{} {} the Line: approach {}",
                    walker.name,
                    if inside { "enters" } else { "leaves" },
                    if inside {
                        "costs double, speed halved"
                    } else {
                        "at full speed"
                    }
                ),
                None,
            );
        }
        let label = format!(
            "{} {speed:.2} m/s{}",
            walker.name,
            if inside { " (in Line)" } else { "" }
        );
        // Keep the walker's line last so the overlay always shows it.
        if let Some(at) = game
            .spells
            .tracks
            .iter()
            .position(|t| t.target == Target::Actor(walker.actor))
        {
            let mut track = game.spells.tracks.remove(at);
            track.label = label;
            game.spells.tracks.push(track);
        }
        let w = &mut game.spells.gust.walkers[index];
        w.since = game.time;
        w.from = feet;
        w.inside = inside;
    }
}

/// Ritual candles around the chamber's summoning circle, and two lanterns,
/// placed on the first cast so presentation without the spell is unchanged.
fn light_chamber(game: &mut Game) -> Result<(), String> {
    if game.spells.gust.chamber_lit
        || game.scene.collision_profile.as_deref() != Some(CHAMBER_PROFILE)
    {
        return Ok(());
    }
    game.spells.gust.chamber_lit = true;
    for i in 0..16 {
        let angle = f64::from(i) * std::f64::consts::TAU / 16.;
        game.spells.gust.add_flame(
            DVec3::new(5.5 * angle.sin(), 0.9, -5.5 * angle.cos()),
            false,
        )?;
    }
    for x in [-2.2, 2.2] {
        game.spells.gust.add_flame(DVec3::new(x, 1.2, -6.4), true)?;
    }
    Ok(())
}

/// One frame of Gust of Wind, before the spell world steps: the Line
/// follows its caster, expires with concentration, rolls due saves, puts
/// out flames, and blows on props and arrows.
pub(crate) fn step(game: &mut Game, dt: f64) -> Result<(), String> {
    volleys(game)?;
    walkers(game);
    if game.spells.gust.lines.is_empty() {
        return Ok(());
    }
    let time = f64::from(game.time);
    let ended: Vec<u64> = game
        .spells
        .gust
        .lines
        .iter()
        .filter(|l| !l.gust.active(time))
        .map(|l| l.cast)
        .collect();
    for cast in ended {
        game.spells.end_cast(cast)?;
        game.spells
            .record(game.time, NAME, "The wind dies down".into(), None);
    }
    let caster = game.player_actor();
    let feet = game.player.as_dvec3();
    for line in &mut game.spells.gust.lines {
        if line.gust.caster == caster {
            line.gust.follow(feet);
        }
    }
    saves(game)?;
    flames(game);
    props(game, dt)?;
    flights(game, dt);
    Ok(())
}

/// The playground recording: a row of props, three dummies, a walker, a
/// row of candles and a lantern, and an archer down the Line; then a re-aim
/// through 90 degrees across a second row.
pub fn scenario() -> crate::playground::Scenario {
    use crate::playground::{Cue, Scenario, Shot, Step, creature};
    use std::f32::consts::FRAC_PI_2;
    Scenario {
        key: KEY,
        title: NAME,
        srd: "Level 2 Evocation | Range Self (60 x 10 ft Line) | STR save | push 15 ft | Conc. 1 min",
        seed: 456,
        live: 12.,
        replay: (0.8, 3.3),
        setup: |scene, _| {
            let wizard = scene
                .actors
                .iter_mut()
                .find(|a| a.model == "adventurer")
                .ok_or("The hall has no wizard")?;
            wizard.position = CASTER;
            for (id, name, z) in [
                (101, "Dummy A", 5.0),
                (102, "Dummy B", 6.0),
                (103, "Dummy C", 7.0),
            ] {
                scene.actors.push(creature(
                    id,
                    name,
                    "dummy",
                    Vec3::new(-1., 0., z),
                    FRAC_PI_2,
                    100,
                ));
            }
            scene.actors.push(creature(
                104,
                "Walker",
                "dummy",
                Vec3::new(15., 0., 5.),
                FRAC_PI_2,
                100,
            ));
            scene.actors.push(creature(
                105,
                "Archer",
                "dummy",
                Vec3::new(17.5, 0., 8.6),
                FRAC_PI_2,
                100,
            ));
            Ok(())
        },
        populate: |game, _| {
            let paper = PropSpec::reference(PropKind::Paper);
            let basket = PropSpec::reference(PropKind::Basket);
            let reference = PropSpec::reference(PropKind::Crate);
            // An empty crate, larger than the reference, whose drag beats
            // its friction.
            let mut crate_ = reference.clone();
            crate_.dimensions = DVec3::splat(0.7);
            // An empty barrel standing on end; a full one (60 kg) holds.
            let mut barrel = PropSpec::reference(PropKind::Barrel);
            barrel.dimensions = DVec3::new(0.45, 1., 0.45);
            barrel.mass = 15.;
            let anvil = PropSpec::reference(PropKind::Anvil);
            let half = |s: &PropSpec| s.dimensions.y as f32 * 0.5;
            for (name, spec, x, z, yaw) in [
                ("Crate (empty)", crate_, 4.5, 6.0, 0.),
                ("Barrel (empty)", barrel, 6.0, 7.0, 0.),
                ("Anvil", anvil, 6.5, 5.0, 0.),
                ("Basket", basket.clone(), 7.6, 6.3, 0.),
                ("Paper A", paper.clone(), 8.2, 6.7, FRAC_PI_2),
                ("Paper B", paper.clone(), 8.6, 7.25, FRAC_PI_2),
            ] {
                let y = half(&spec);
                game.spawn_prop(name, spec, Vec3::new(x, y, z), yaw)?;
            }
            // The second row, across the re-aimed Line.
            let dummy = PropSpec::reference(PropKind::TrainingDummy);
            for (name, spec, x, z) in [
                ("Paper C", paper.clone(), -6.8, 3.9),
                ("Paper D", paper, -5.2, 4.1),
                ("Basket B", basket, -6.0, 2.8),
                ("Crate (reference)", reference.clone(), -5.2, 1.3),
                ("Secured crate", reference.secured(), -6.8, 1.3),
                ("Training dummy", dummy, -6.0, -0.3),
            ] {
                let y = half(&spec);
                game.spawn_prop(name, spec, Vec3::new(x, y, z), 0.)?;
            }
            let state = &mut game.spells.gust;
            for x in [9.0, 9.5, 10.0, 10.5] {
                state.add_flame(DVec3::new(x, 1., 7.35), false)?;
            }
            state.add_flame(DVec3::new(11.0, 1.2, 7.3), true)?;
            state.volleys.push(Volley {
                at: 4.0,
                direction: DVec3::NEG_X,
                arrows: vec![
                    ("Into the wind".into(), DVec3::new(17., 2.4, 7.3)),
                    ("Calm air".into(), DVec3::new(17., 2.4, 9.3)),
                ],
                loosed: vec![],
                report_after: 1.0,
                reported: false,
            });
            // A and B fail and are pushed; C succeeds. Everyone passes the
            // end-of-turn save, so the measured pushes are one each.
            for (dummy, rolls) in [
                (101, [3, 20]),
                (102, [5, 20]),
                (103, [18, 20]),
                (104, [20, 20]),
            ] {
                for roll in rolls {
                    game.spells.dice.force_save(dummy, roll)?;
                }
            }
            let life = game.actor_life(104).ok_or("The walker has no life")?;
            game.direct_npc_navigation(life, Vec3::new(9., 0., 5.), 1.2)?;
            game.spells.track(Track {
                label: "Walker".into(),
                target: Target::Actor(104),
                spell: NAME.into(),
                at: 0.,
                start: DVec3::new(15., 0., 5.),
                requested: 0.,
            });
            game.spells.gust.walkers.push(Walker {
                actor: 104,
                name: "Walker".into(),
                since: game.time,
                from: DVec3::new(15., 0., 5.),
                inside: false,
            });
            Ok(())
        },
        script: || {
            let spell = crate::play::Ability::Spell(SLOT);
            vec![
                Cue {
                    at: 0.3,
                    step: Step::Face(-FRAC_PI_2),
                },
                Cue {
                    at: 1.0,
                    step: Step::Cast(spell),
                },
                Cue {
                    at: 7.3,
                    step: Step::Face(0.),
                },
                Cue {
                    at: 7.6,
                    step: Step::Cast(spell),
                },
            ]
        },
        camera: || {
            let down_the_line = (Vec3::new(3., 12., 20.), Vec3::new(3., 4., 0.));
            let across = (Vec3::new(8., 13., -2.), Vec3::new(-8., 5., -1.5));
            [
                (0., down_the_line),
                (7.0, down_the_line),
                (7.6, across),
                (12., across),
            ]
            .into_iter()
            .map(|(at, (eye, target))| Shot { at, eye, target })
            .collect()
        },
        replay_camera: (Vec3::new(3.5, 9.5, 17.5), Vec3::new(3.5, 3., 0.5)),
        check: |game| {
            let moved = |label: &str| {
                game.spells
                    .tracks
                    .iter()
                    .find(|t| t.label == label)
                    .and_then(|t| crate::playground::measure(game, t))
                    .map(|(_, d)| d)
                    .ok_or(format!("{label} was not in the Line"))
            };
            let push = gust::PUSH_DISTANCE;
            for dummy in ["Dummy A", "Dummy B"] {
                let got = moved(dummy)?;
                if (got - push).abs() > push * 0.05 {
                    return Err(format!("{dummy} moved {got:.3} m, expected {push:.3} m"));
                }
            }
            if moved("Dummy C")? > 0.02 {
                return Err("Dummy C made its save but moved".into());
            }
            if moved("Anvil")? > 0.01 {
                return Err("The anvil moved".into());
            }
            for (label, least) in [("Paper A", 5.), ("Basket", 3.), ("Crate (empty)", 0.3)] {
                if moved(label)? < least {
                    return Err(format!("{label} moved less than {least} m"));
                }
            }
            if moved("Secured crate")? > 1e-6 {
                return Err("The secured crate moved".into());
            }
            let tilted = |name: &str| {
                game.spells
                    .props
                    .iter()
                    .find(|p| p.name == name)
                    .map(|p| {
                        game.spells.world[p.body]
                            .orientation
                            .angle_between(glam::DQuat::IDENTITY)
                    })
                    .unwrap_or(0.)
            };
            for name in ["Barrel (empty)", "Training dummy"] {
                if tilted(name) < 0.5 {
                    return Err(format!("{name} did not tip"));
                }
            }
            if game
                .spells
                .gust
                .flames
                .iter()
                .any(|f| !f.protected && f.lit)
            {
                return Err("A candle in the Line stayed lit".into());
            }
            if !game.spells.log.iter().any(|r| r.text.contains("Lantern")) {
                return Err("The lantern never rolled".into());
            }
            if !game
                .spells
                .log
                .iter()
                .any(|r| r.text.contains("Walker enters the Line"))
            {
                return Err("The walker never entered the Line".into());
            }
            let error = game.spells.ledger_error();
            if error.linear > super::LEDGER_TOLERANCE || error.angular > super::LEDGER_TOLERANCE {
                return Err(format!("Ledger residual {error:?}"));
            }
            Ok(())
        },
    }
}

/// The wizard's place in the playground hall.
const CASTER: Vec3 = Vec3::new(-6., 0., 6.);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::Ability;
    use crate::playground::{creature, hall};

    const SPELL: Ability = Ability::Spell(SLOT);

    /// The playground hall with dummies at `dummies` and the wizard at
    /// `caster`, facing +z.
    fn hall_game(caster: Vec3, dummies: &[(u64, Vec3)]) -> Game {
        let hall = hall().unwrap();
        let mut scene = hall.scene.clone();
        scene.actors[0].position = caster;
        for (id, at) in dummies {
            scene
                .actors
                .push(creature(*id, "Dummy", "dummy", *at, 0., 100));
        }
        let mut game = Game::new(scene).unwrap();
        game.face(std::f32::consts::PI).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game
    }

    fn settle(game: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 30.) as u32 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
    }

    #[test]
    fn the_spell_is_on_the_second_action_bar_row() {
        let def = Ability::Spell(SLOT).catalog().unwrap();
        assert_eq!(def.key, KEY);
        assert_eq!(def.icon, "gust-of-wind-icon");
        assert!(crate::playground::scenario(KEY).is_some());
    }

    #[test]
    fn a_failed_save_pushes_fifteen_feet_and_a_pass_stays() {
        let caster = Vec3::new(-4., 0., 0.);
        let (fails, passes) = (
            caster + Vec3::new(-1., 0., 3.),
            caster + Vec3::new(1., 0., 3.),
        );
        let mut game = hall_game(caster, &[(2, fails), (3, passes)]);
        game.spells.dice.force_save(2, 1).unwrap();
        game.spells.dice.force_save(3, 20).unwrap();
        game.activate(SPELL).unwrap();
        settle(&mut game, 3.);
        let moved = game.actor_position(2).unwrap().distance(fails) as f64;
        assert!(
            (moved - gust::PUSH_DISTANCE).abs() < gust::PUSH_DISTANCE * 0.05,
            "{moved}"
        );
        assert!(game.actor_position(3).unwrap().distance(passes) < 1e-3);
        let pushed = game.actor_position(2).unwrap() - fails;
        assert!(pushed.z > 0. && pushed.x.abs() < 0.05, "along the Line");
    }

    #[test]
    fn saves_repeat_every_six_seconds_inside() {
        let caster = Vec3::new(-4., 0., 0.);
        let dummy = caster + Vec3::new(0., 0., 4.);
        let mut game = hall_game(caster, &[(2, dummy)]);
        for _ in 0..4 {
            game.spells.dice.force_save(2, 20).unwrap();
        }
        game.activate(SPELL).unwrap();
        settle(&mut game, 13.);
        let at: Vec<f32> = game
            .spells
            .log
            .iter()
            .filter(|r| r.save.is_some())
            .map(|r| r.at)
            .collect();
        assert_eq!(at.len(), 3, "{at:?}");
        assert!((at[1] - at[0] - 6.).abs() < 0.05 && (at[2] - at[1] - 6.).abs() < 0.05);
    }

    #[test]
    fn re_aim_waits_for_its_cooldown_and_concentration_end_removes_the_line() {
        let caster = Vec3::new(-4., 0., 0.);
        let mut game = hall_game(caster, &[(2, Vec3::new(20., 0., -20.))]);
        let spec = PropSpec::reference(PropKind::Basket);
        let basket = game
            .spawn_prop("Basket", spec, caster + Vec3::new(0., 0.2, 3.), 0.)
            .unwrap();
        game.activate(SPELL).unwrap();
        let start = game.time;
        settle(&mut game, 3.);
        assert!(game.activate(SPELL).is_err(), "cooling down");
        settle(&mut game, 3.1);
        game.face(-std::f32::consts::FRAC_PI_2).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        game.activate(SPELL).unwrap();
        let line = game.spells.gust.lines[0].gust.line;
        assert!((line.direction - DVec3::X).length() < 1e-6, "{line:?}");
        assert!(game.time - start >= COOLDOWN);
        assert!(game.spells.ledger.external.contains_key(TERM));
        game.spells.end_concentration(game.player_actor()).unwrap();
        assert!(game.spells.gust.lines.is_empty());
        settle(&mut game, 2.);
        let resting = game.spells.prop_center(basket);
        settle(&mut game, 1.);
        assert!(game.spells.prop_center(basket).distance(resting) < 1e-3);
        let error = game.spells.ledger_error();
        assert!(error.linear < super::super::LEDGER_TOLERANCE, "{error:?}");
    }

    #[test]
    fn the_wind_moves_paper_but_not_the_anvil() {
        let caster = Vec3::new(-4., 0., 0.);
        let mut game = hall_game(caster, &[(2, Vec3::new(20., 0., -20.))]);
        let paper = game
            .spawn_prop(
                "Paper",
                PropSpec::reference(PropKind::Paper),
                caster + Vec3::new(-1., 0.025, 4.),
                0.,
            )
            .unwrap();
        let anvil = game
            .spawn_prop(
                "Anvil",
                PropSpec::reference(PropKind::Anvil),
                caster + Vec3::new(1., 0.2, 4.),
                std::f32::consts::FRAC_PI_2,
            )
            .unwrap();
        settle(&mut game, 0.5);
        let before = [
            game.spells.prop_center(paper),
            game.spells.prop_center(anvil),
        ];
        game.activate(SPELL).unwrap();
        settle(&mut game, 2.);
        assert!(game.spells.prop_center(paper).distance(before[0]) > 5.);
        assert!(game.spells.prop_center(anvil).distance(before[1]) < 1e-3);
    }

    #[test]
    fn checkpoints_replay_identically_with_the_wind_blowing() {
        let mut run = crate::playground::Run::new(scenario()).unwrap();
        while run.game.time < 1.8 {
            run.advance().unwrap();
        }
        assert!(!run.game.spells.gust.lines.is_empty());
        let saved = run.game.checkpoint().unwrap();
        let mut restored = Game::restore(&saved).unwrap();
        for _ in 0..60 {
            run.game.tick(1. / 30., [0.; 2]).unwrap();
            restored.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert_eq!(
            run.game.checkpoint().unwrap(),
            restored.checkpoint().unwrap()
        );
    }

    #[test]
    fn the_chamber_wind_blows_out_ritual_candles_and_pushes_a_cultist_back() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = Game::new(scene).unwrap();
        for _ in 0..201 {
            game.tick(0.1, [0.; 2]).unwrap();
        }
        assert!(game.spells.gust.flames.is_empty());
        let cultist = game
            .scene
            .actors
            .iter()
            .find(|a| a.model.starts_with("cultist"))
            .map(|a| a.id)
            .unwrap();
        let near = game.actor_position(cultist).unwrap();
        game.player = Vec3::new(near.x, 0., near.z - 3.);
        game.face(std::f32::consts::PI).unwrap();
        game.tick(1. / 30., [0.; 2]).unwrap();
        let near = game.actor_position(cultist).unwrap();
        game.spells.dice.force_save(cultist, 1).unwrap();
        game.activate(SPELL).unwrap();
        settle(&mut game, 2.);
        assert!(game.actor_position(cultist).unwrap().distance(near) > 2.);
        let flames = &game.spells.gust.flames;
        assert_eq!(flames.len(), 18);
        assert!(
            flames.iter().any(|f| !f.lit),
            "a ritual candle in the Line went out"
        );
        assert!(flames.iter().any(|f| f.lit), "candles outside it burn on");
    }
}
