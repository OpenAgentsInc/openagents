//! Reverse Gravity in the chamber authority and the spell playground.
//!
//! The cast adds a concentration [`SpellField`] over the cylinder: inside it
//! a prop feels twice standard gravity upward on top of the world's gravity,
//! so it falls upward at 9.81 m/s², and in the top [`HOVER_BAND`] the hover
//! spring of [`field_accel`] replaces both, so it settles on the top plane.
//! The field leaves when concentration ends (the minute runs out, the caster
//! casts it again to dismiss it, casts another concentration spell, or
//! dies), and everything falls.
//!
//! Characters are kinematic capsules, so [`step`] moves them itself: a
//! character inside the cylinder that is not holding a fixed object gets a
//! zero-gravity override and the exact upward or hover motion for the frame,
//! and the capsule sweep stops it against ceilings. Each creature makes its
//! Dexterity save when the cylinder first contains it; a success with a
//! fixed object within 5 feet of its center holds it down. A strike on a
//! ceiling deals SRD falling damage for the upward span, and landing after
//! the field leaves deals it for the drop, through the same
//! `Game::fall_damage` every fall uses. Props that strike a ceiling or land
//! take object damage by the same table.
//!
//! The chamber has walls but no modeled vault, so in the chamber the wall
//! top stands in for its ceiling ([`CHAMBER_ROOF`]): cultists strike it and
//! press against it until the spell ends.
use super::{
    Cylinder, FOOT, GRAB_REACH, GRAVITY, HEIGHT, HOVER_BAND, HOVER_DRAG, HOVER_OMEGA, RADIUS,
    RANGE, STRIKE_SPEED,
};
use crate::play::Game;
use crate::spells::{Area, CHARACTER_HEIGHT, SPELL_SAVE_DC, Save, SpellDef, SpellField};
use glam::{DVec3, Vec3};
use serde::{Deserialize, Serialize};

pub const NAME: &str = "Reverse Gravity";
/// Row-two slot: Shift+9.
pub const SLOT: u8 = 8;
/// Concentration, up to 1 minute, s.
pub const DURATION: f32 = 60.;
/// Chamber mana: MMO tuning, not tabletop rules.
pub const COST: i32 = 6;
/// Chamber cooldown, s: short, so a second press can dismiss the spell.
pub const COOLDOWN: f32 = 2.;
/// With no living target selected within range, the cylinder stands this
/// far ahead of the caster: 60 feet, so a caster on open ground is outside
/// its 50-foot radius.
pub const AIM_DISTANCE: f64 = 60. * FOOT;
/// The chamber's wall top, m, standing in for the vault it does not model.
pub const CHAMBER_ROOF: f64 = 12.;
/// The chamber's collision profile.
pub const CHAMBER_PROFILE: &str = "original-chamber-v1";
/// The player wizard's Dexterity modifier.
pub const WIZARD_DEXTERITY: i32 = 2;
/// How long falls are still tracked after the field leaves, s.
pub const SETTLE: f32 = 8.;
/// Height of a character's center above its feet, m.
const CENTER: f64 = 0.9;
/// A prop's reference point counts as hovering this close to the top, m.
const HOVERING: f64 = 0.2;
/// A character's feet hover this far above the top plane, m, so a fall from
/// the top measures the full 100 feet despite the capsule's contact skin.
const CLEARANCE: f64 = 0.01;
/// Feet this far below the cylinder's base still count as inside it, m: the
/// base stands on the target's feet, and the ground holds every creature a
/// contact skin above or below that.
const BASE_TOLERANCE: f64 = 0.1;

/// Whether the field holds a character's feet: inside the cylinder, with
/// the tolerances above.
pub fn holds(cylinder: &Cylinder, feet: DVec3) -> bool {
    cylinder.axis_distance(feet) <= cylinder.radius
        && (cylinder.base.y - BASE_TOLERANCE..=cylinder.top() + CLEARANCE).contains(&feet.y)
}

pub const SPELL: SpellDef = SpellDef {
    slot: SLOT,
    key: "reverse-gravity",
    label: NAME,
    icon: "reverse-gravity-icon",
    description: "Gravity reverses in a 50-ft-radius, 100-ft-high cylinder; cast again to end it",
    cost: COST,
    cooldown: COOLDOWN,
    cast,
};

/// Dexterity modifiers from SRD stat blocks; the Cultist has DEX 12.
pub fn dexterity_modifier(model: &str) -> i32 {
    match model {
        m if m.starts_with("cultist") => 1,
        "adventurer" => WIZARD_DEXTERITY,
        // Not an SRD creature: a straw training dummy has no agility.
        "dummy" => 0,
        // The ritual's boss has no SRD stat block; this is encounter tuning.
        "claude" => 0,
        _ => 0,
    }
}

/// The top plane of a Reverse Gravity field, if `field` is one.
pub fn hover_top(field: &SpellField) -> Option<f64> {
    match field.area {
        Area::Cylinder { base, height, .. } if field.spell == NAME => Some(base.y + height),
        _ => None,
    }
}

/// What a Reverse Gravity field adds on top of `gravity` at `pos` for a body
/// moving at `vel`: `acceleration` below the hover band, and in the band the
/// critically damped hover spring toward `top` with light horizontal drag,
/// less the gravity it replaces.
pub fn field_accel(top: f64, pos: DVec3, vel: DVec3, gravity: DVec3, acceleration: DVec3) -> DVec3 {
    let below = top - pos.y;
    if below > HOVER_BAND {
        return acceleration;
    }
    let w = HOVER_OMEGA;
    DVec3::new(
        -HOVER_DRAG * vel.x,
        w * w * below - 2. * w * vel.y,
        -HOVER_DRAG * vel.z,
    ) - gravity
}

/// Exact motion of a reference point under the field for `dt`: upward at
/// standard gravity below the hover band, the critically damped spring
/// toward `top` inside it. Returns the new height and speed.
pub fn rise(y: f64, speed: f64, top: f64, dt: f64) -> (f64, f64) {
    let band = top - HOVER_BAND;
    if y < band {
        let next = y + speed * dt + 0.5 * GRAVITY * dt * dt;
        if next < band {
            return (next, speed + GRAVITY * dt);
        }
        // Enter the band partway through the frame, so the spring starts
        // from its lower edge and never overshoots the top.
        let t = ((speed * speed + 2. * GRAVITY * (band - y)).max(0.).sqrt() - speed) / GRAVITY;
        return rise(band, speed + GRAVITY * t, top, (dt - t).max(0.));
    }
    let w = HOVER_OMEGA;
    let x = y - top;
    let c = speed + w * x;
    let e = (-w * dt).exp();
    (top + (x + c * dt) * e, (speed - w * c * dt) * e)
}

/// A creature the field has seen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Creature {
    pub actor: u64,
    /// Its Dexterity save, made when the cylinder first contained it.
    pub save: Option<Save>,
    /// It made its save with a fixed object in reach, which it holds.
    pub held: Option<DVec3>,
    pub inside: bool,
    /// Whether the field controls its vertical motion.
    pub governed: bool,
    /// The vertical speed set for the last frame, and the true speed at its
    /// end, m/s.
    pub set_speed: f64,
    pub speed: f64,
    /// Lowest feet height since its last strike, m.
    pub low: f64,
    pub hovering: bool,
}

/// A prop's flight, for strikes and falling damage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropFall {
    pub index: usize,
    pub high: f64,
    pub low: f64,
    /// Vertical speed at the end of the last frame, m/s.
    pub speed: f64,
    pub hovering: bool,
}

/// One cast of Reverse Gravity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Active {
    pub cast: u64,
    pub caster: u64,
    /// The cylinder, its height clipped to a roof where there is one.
    pub cylinder: Cylinder,
    /// A ceiling plane the scene does not model, m.
    pub roof: Option<f64>,
    /// Scene time the field left, once it has.
    pub ended: Option<f32>,
    pub creatures: Vec<Creature>,
    pub props: Vec<PropFall>,
}

/// A scripted lateral impulse on a prop: what a playground scenario uses to
/// carry a hovering prop out of the cylinder, standing in for Gust of Wind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shove {
    /// Scene seconds.
    pub at: f32,
    pub prop: usize,
    /// Velocity change, m/s.
    pub velocity: DVec3,
}

/// Every Reverse Gravity cast in the spell world.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub active: Vec<Active>,
    pub shoves: Vec<Shove>,
}

impl State {
    pub fn validate(&self, props: usize) -> Result<(), String> {
        let finite = |c: &Cylinder| {
            c.base.is_finite()
                && c.radius > 0.
                && c.radius <= RADIUS
                && c.height > 0.
                && c.height <= HEIGHT
        };
        if self.active.len() > 16
            || self.shoves.len() > 64
            || self.active.iter().any(|a| {
                !finite(&a.cylinder)
                    || a.roof.is_some_and(|r| !r.is_finite())
                    || a.ended.is_some_and(|t| !t.is_finite())
                    || a.creatures.len() > 256
                    || a.props.len() > props
                    || a.props.iter().any(|p| {
                        p.index >= props
                            || !p.high.is_finite()
                            || !p.low.is_finite()
                            || !p.speed.is_finite()
                    })
                    || a.creatures.iter().any(|c| {
                        !c.set_speed.is_finite()
                            || !c.speed.is_finite()
                            || !c.low.is_finite()
                            || c.held.is_some_and(|p| !p.is_finite())
                    })
            })
            || self
                .shoves
                .iter()
                .any(|s| s.prop >= props || !s.at.is_finite() || !s.velocity.is_finite())
        {
            return Err("Invalid Reverse Gravity checkpoint".into());
        }
        Ok(())
    }
}

/// Casts Reverse Gravity for the adventurer, or ends the cast it holds.
pub fn cast(game: &mut Game) -> Result<(), String> {
    let caster = game.player_actor();
    if let Some(active) = game
        .spells
        .reverse_gravity
        .active
        .iter()
        .find(|a| a.caster == caster && a.ended.is_none())
    {
        let cast = active.cast;
        game.spells.end_cast(cast)?;
        game.spells
            .record(game.time, NAME, "Dismissed: everything falls".into(), None);
        return Ok(());
    }
    let origin = game.player.as_dvec3();
    let facing = DVec3::new(-f64::from(game.yaw.sin()), 0., -f64::from(game.yaw.cos()));
    let selected = game.selected;
    // The adventurer can select itself; the spell then aims ahead.
    let target = (selected != caster)
        .then(|| game.frame().actors)
        .unwrap_or_default()
        .into_iter()
        .find(|a| a.actor.id == selected && a.health > 0)
        .and_then(|_| game.actor_position(selected))
        .map(Vec3::as_dvec3)
        .filter(|p| p.distance(origin) <= RANGE);
    let point = target.unwrap_or(origin + facing * AIM_DISTANCE);
    if point.distance(origin) > RANGE {
        return Err("The point is beyond 100 feet".into());
    }
    let roof =
        (game.scene.collision_profile.as_deref() == Some(CHAMBER_PROFILE)).then_some(CHAMBER_ROOF);
    let height = roof.map_or(HEIGHT, |r| (r - point.y).clamp(HOVER_BAND, HEIGHT));
    let cylinder = Cylinder {
        base: point,
        radius: RADIUS,
        height,
    };
    let cast = game.spells.begin_cast(caster, true)?;
    game.spells.add_field(SpellField {
        cast,
        spell: NAME.into(),
        owner: caster,
        area: Area::Cylinder {
            base: point,
            radius: RADIUS,
            height,
        },
        // Twice gravity upward on top of gravity: a net fall upward.
        acceleration: DVec3::Y * 2. * GRAVITY,
        expires: game.time + DURATION,
        concentration: true,
    })?;
    let mut props = vec![];
    for index in 0..game.spells.props.len() {
        let prop = &game.spells.props[index];
        if prop.removed || prop.spec.secured {
            continue;
        }
        let body = prop.body;
        game.spells.world.wake(body);
        let y = game.spells.world[body].pos.y;
        props.push(PropFall {
            index,
            high: y,
            low: y,
            speed: game.spells.world[body].vel.y,
            hovering: false,
        });
    }
    game.spells.record(
        game.time,
        NAME,
        format!(
            "Cast at ({:.1}, {:.1}): 50-ft-radius, {:.0}-ft-high cylinder{}",
            point.x,
            point.z,
            height / FOOT,
            if roof.is_some() {
                " under the chamber roof"
            } else {
                ""
            }
        ),
        None,
    );
    game.spells.reverse_gravity.active.push(Active {
        cast,
        caster,
        cylinder,
        roof,
        ended: None,
        creatures: vec![],
        props,
    });
    Ok(())
}

/// The nearest point on fixed geometry within [`GRAB_REACH`] of a creature's
/// center, ignoring what lies at or below its feet (the ground it stands on
/// is not an object to grab).
pub fn reachable_fixed(world: &physics::World, feet: DVec3) -> Option<DVec3> {
    let center = feet + DVec3::Y * CENTER;
    world
        .colliders()
        .iter()
        .filter(|c| {
            let body = &world[c.body];
            body.kind == physics::BodyKind::Static && !body.removed
        })
        .map(|c| c.closest_point(world, center))
        .filter(|p| p.y > feet.y + 0.1 && p.distance(center) <= GRAB_REACH)
        .min_by(|a, b| a.distance(center).total_cmp(&b.distance(center)))
}

/// What one frame did to a creature.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    /// It entered the cylinder and must make its save.
    pub entered: bool,
    /// Upward fall distance of a ceiling strike, m.
    pub strike: Option<f64>,
    /// It settled at the top.
    pub hovering: bool,
}

/// Moves one character for the next frame of `dt` seconds under the field,
/// after the frame its last setting produced. `live` is false once the field
/// has left; the character then gets its own gravity back.
pub fn govern(
    character: &mut physics::character::Character,
    creature: &mut Creature,
    active: &Active,
    live: bool,
    dt: f64,
) -> Outcome {
    let mut outcome = Outcome::default();
    let feet = character.feet;
    let inside = live && holds(&active.cylinder, feet);
    outcome.entered = inside && !creature.inside;
    creature.inside = inside;
    // The sweep zeroes the vertical speed when a ceiling stops the capsule.
    let stopped = creature.governed && character.vertical_speed != creature.set_speed;
    if creature.governed && stopped && creature.set_speed >= STRIKE_SPEED {
        outcome.strike = Some(feet.y - creature.low);
        creature.low = feet.y;
    }
    creature.low = creature.low.min(feet.y);
    if !inside || creature.held.is_some() || dt <= 0. {
        if creature.governed {
            character.gravity = None;
            creature.governed = false;
        }
        creature.set_speed = 0.;
        creature.speed = 0.;
        return outcome;
    }
    if !creature.governed {
        creature.speed = character.vertical_speed;
        creature.low = feet.y;
    } else if stopped {
        creature.speed = character.vertical_speed;
    }
    creature.governed = true;
    character.gravity = Some(physics::character::GravityOverride {
        scale: 0.,
        terminal: physics::character::TERMINAL_SPEED,
    });
    let top = active.cylinder.top() + CLEARANCE;
    let (mut next, mut speed) = rise(feet.y, creature.speed, top, dt);
    if let Some(roof) = active.roof {
        let limit = roof - CHARACTER_HEIGHT;
        if next >= limit {
            let free = (next - feet.y) / dt;
            if free >= STRIKE_SPEED && feet.y < limit {
                outcome.strike = Some(limit - creature.low);
                creature.low = limit;
            }
            next = limit;
            speed = 0.;
        }
    }
    let average = ((next - feet.y) / dt).clamp(-55., 100.);
    character.add_velocity(DVec3::Y * (average - character.vertical_speed));
    creature.set_speed = character.vertical_speed;
    creature.speed = speed;
    if !creature.hovering && (next - top).abs() < HOVERING && speed.abs() < 0.3 {
        creature.hovering = true;
        outcome.hovering = true;
    }
    outcome
}

/// Advances every Reverse Gravity cast one frame of `dt` seconds, after the
/// spell world stepped its props.
pub(crate) fn step(game: &mut Game, dt: f64) -> Result<(), String> {
    let due: Vec<Shove> = game
        .spells
        .reverse_gravity
        .shoves
        .iter()
        .filter(|s| s.at <= game.time)
        .cloned()
        .collect();
    game.spells
        .reverse_gravity
        .shoves
        .retain(|s| s.at > game.time);
    for shove in due {
        let Some(prop) = game.spells.props.get(shove.prop) else {
            continue;
        };
        let body = prop.body;
        let name = prop.name.clone();
        let mass = game.spells.world[body].mass;
        let at = game.spells.world[body].pos;
        game.spells
            .impulse_prop(shove.prop, shove.velocity * mass, at, "scripted shove")?;
        game.spells.record(
            game.time,
            NAME,
            format!(
                "{name} shoved sideways at {:.1} m/s",
                shove.velocity.length()
            ),
            None,
        );
    }
    let mut actives = std::mem::take(&mut game.spells.reverse_gravity.active);
    let result = actives
        .iter_mut()
        .try_for_each(|active| step_active(game, active, dt));
    let time = game.time;
    actives.retain(|a| a.ended.is_none_or(|t| time < t + SETTLE));
    game.spells.reverse_gravity.active = actives;
    result
}

fn step_active(game: &mut Game, active: &mut Active, dt: f64) -> Result<(), String> {
    let live = game.spells.fields.iter().any(|f| f.cast == active.cast);
    if !live && active.ended.is_none() {
        active.ended = Some(game.time);
        for fall in &active.props {
            if let Some(prop) = game.spells.props.get(fall.index) {
                if !prop.removed {
                    let body = prop.body;
                    game.spells.world.wake(body);
                }
            }
        }
        game.spells.record(
            game.time,
            NAME,
            "The spell ends: everything falls".into(),
            None,
        );
    }
    let player = game.player_actor();
    let mut actors: Vec<(u64, String)> = vec![];
    if game.snapshot().player.hp > 0 {
        actors.push((player, "adventurer".into()));
    }
    let additional: Vec<u64> = game
        .additional_characters()
        .into_iter()
        .map(|(a, _)| a)
        .collect();
    actors.extend(additional.iter().map(|a| (*a, "adventurer".to_string())));
    let snapshot = game.snapshot();
    for (actor, id) in &game.ids {
        if *actor == player || additional.contains(actor) {
            continue;
        }
        if snapshot
            .actors
            .iter()
            .any(|a| a.id == *id && a.alive && a.faction != "player")
        {
            let model = game
                .scene
                .actors
                .iter()
                .find(|a| a.id == *actor)
                .map_or_else(String::new, |a| a.model.clone());
            actors.push((*actor, model));
        }
    }
    // A creature that died while the field held it gets its gravity back.
    let living: Vec<u64> = actors.iter().map(|(a, _)| *a).collect();
    for creature in &mut active.creatures {
        if creature.governed && !living.contains(&creature.actor) {
            creature.governed = false;
            creature.inside = false;
            if let Some(character) = game.npc_characters.get_mut(&creature.actor) {
                character.gravity = None;
            }
        }
    }
    for (actor, model) in actors {
        let Some(mut character) = character_of(game, actor) else {
            continue;
        };
        let index = match active.creatures.iter().position(|c| c.actor == actor) {
            Some(i) => i,
            None => {
                active.creatures.push(Creature {
                    actor,
                    save: None,
                    held: None,
                    inside: false,
                    governed: false,
                    set_speed: 0.,
                    speed: 0.,
                    low: character.feet.y,
                    hovering: false,
                });
                active.creatures.len() - 1
            }
        };
        let mut creature = active.creatures[index].clone();
        if !live {
            creature.held = None;
        }
        let entering = live && holds(&active.cylinder, character.feet) && !creature.inside;
        if entering && creature.held.is_none() {
            let save = game.spells.dice.save(
                actor,
                "Dexterity",
                dexterity_modifier(&model),
                SPELL_SAVE_DC,
            );
            let reach = if save.success {
                reachable_fixed(&game.spells.world, character.feet)
            } else {
                None
            };
            let name = game.actor_name(actor);
            game.spells.record(
                game.time,
                NAME,
                format!(
                    "{name}: DEX save {} {:+} = {} vs DC {} {}",
                    save.roll,
                    save.modifier,
                    save.total,
                    save.dc,
                    match (save.success, reach) {
                        (true, Some(_)) => "succeeds; grabs a fixed object and stays down",
                        (true, None) => "succeeds, but nothing is in reach; falls upward",
                        (false, _) => "fails; falls upward",
                    }
                ),
                Some(save.clone()),
            );
            creature.held = reach;
            creature.save = Some(save);
        }
        let outcome = govern(&mut character, &mut creature, active, live, dt);
        active.creatures[index] = creature;
        set_character(game, actor, character);
        let name = game.actor_name(actor);
        if outcome.hovering {
            game.spells.record(
                game.time,
                NAME,
                format!(
                    "{name} hovers at the top, {:.0} ft up",
                    (active.cylinder.top() - active.cylinder.base.y) / FOOT
                ),
                None,
            );
        }
        if let Some(span) = outcome.strike {
            game.spells.record(
                game.time,
                NAME,
                format!(
                    "{name} slams into the ceiling after {:.0} ft up",
                    span / FOOT
                ),
                None,
            );
            game.fall_damage((actor != player).then_some(actor), span)?;
        }
    }
    props(game, active, live)
}

fn character_of(game: &mut Game, actor: u64) -> Option<physics::character::Character> {
    if actor == game.player_actor() {
        return Some(game.character);
    }
    if let Some((_, c)) = game
        .additional_characters()
        .into_iter()
        .find(|(a, _)| *a == actor)
    {
        return Some(*c);
    }
    let feet = game.actor_position(actor)?.as_dvec3();
    let character = game
        .npc_characters
        .entry(actor)
        .or_insert_with(|| physics::character::Character::new(feet));
    if character.feet.as_vec3() != feet.as_vec3() {
        *character = physics::character::Character::new(feet);
    }
    Some(*character)
}

fn set_character(game: &mut Game, actor: u64, character: physics::character::Character) {
    if actor == game.player_actor() {
        game.character = character;
        return;
    }
    if let Some((_, c)) = game
        .additional_characters()
        .into_iter()
        .find(|(a, _)| *a == actor)
    {
        *c = character;
        return;
    }
    game.npc_characters.insert(actor, character);
}

/// Tracks each prop's flight and applies object damage when one strikes a
/// ceiling or lands. The props stepped the whole frame already, so a strike
/// shows as a vertical speed that collapsed outside the hover band, where
/// nothing but a contact stops a body that fast.
fn props(game: &mut Game, active: &mut Active, live: bool) -> Result<(), String> {
    let top = active.cylinder.top();
    for fall in &mut active.props {
        let Some(prop) = game.spells.props.get(fall.index) else {
            continue;
        };
        if prop.removed {
            continue;
        }
        let name = prop.name.clone();
        let body = &game.spells.world[prop.body];
        let (pos, speed) = (body.pos, body.vel.y);
        let before = fall.speed;
        fall.speed = speed;
        fall.high = fall.high.max(pos.y);
        fall.low = fall.low.min(pos.y);
        let in_band = live && active.cylinder.contains(pos) && pos.y >= top - HOVER_BAND - 0.3;
        if live && !fall.hovering && (pos.y - top).abs() < HOVERING && speed.abs() < 0.3 {
            fall.hovering = true;
        }
        if in_band {
            continue;
        }
        let upward = before >= STRIKE_SPEED && speed < before * 0.5;
        let downward = before <= -STRIKE_SPEED && speed > before * 0.5;
        if !upward && !downward {
            continue;
        }
        let span = if upward {
            fall.high - fall.low
        } else {
            fall.high - fall.low.min(pos.y)
        };
        fall.high = pos.y;
        fall.low = pos.y;
        let dice = crate::spells::fall_dice(span);
        if dice == 0 {
            continue;
        }
        let damage = game.spells.dice.sum(dice, 6) as i32;
        let index = fall.index;
        let remaining = game.spells.props[index].hit_points.map(|hp| hp - damage);
        game.spells.props[index].hit_points = remaining.map(|hp| hp.max(0));
        game.spells.record(
            game.time,
            NAME,
            format!(
                "{name} {} after {:.0} ft: {dice}d6 = {damage} bludgeoning{}",
                if upward {
                    "cracks against the ceiling"
                } else {
                    "crashes down"
                },
                span / FOOT,
                if remaining.is_some_and(|hp| hp <= 0) {
                    "; it breaks"
                } else {
                    ""
                }
            ),
            None,
        );
        if remaining.is_some_and(|hp| hp <= 0) {
            game.spells.remove_prop(index)?;
        }
    }
    Ok(())
}

/// The playground recording, in one 14-second run under a single cast:
/// crates, barrels, a plank pile, and four dummies rise and hover at
/// 100 feet; a fifth dummy makes its save and clings to the west pillar
/// beside a secured crate that stays put; two dummies and two props under a
/// stone slab 40 feet above the dummies' heads slam into it; at 5 s a
/// scripted shove (standing in for Gust of Wind) carries two hovering crates
/// over the cylinder's edge and they plummet; at 10 s the wizard dismisses
/// the spell and everything crashes down. The rise replays at 0.25×.
pub fn scenario() -> crate::playground::Scenario {
    use crate::playground::{Cue, Scenario, Shot, Step, creature};
    use crate::spells::{PropKind, PropSpec};
    Scenario {
        key: "reverse-gravity",
        title: NAME,
        srd: "Level 7 Transmutation | Range 100 ft | 50-ft-radius, 100-ft-high Cylinder | \
              DEX save to grab a fixed object | Concentration, 1 minute",
        seed: 460,
        live: 14.,
        replay: (0.9, 3.4),
        setup: |scene, _| {
            for (id, name, x, z) in [
                (101, "Dummy A", CENTER_X, CENTER_Z),
                (102, "Dummy B", -13., -3.),
                (103, "Dummy C", -7., -9.),
                (104, "Dummy D (saves, no grip)", -13., -9.),
                (105, "Dummy E (clings)", -4.6, -12.),
                (106, "Dummy F (indoor)", -11., 3.),
                (107, "Dummy G (indoor)", -9., 4.),
            ] {
                scene.actors.push(creature(
                    id,
                    name,
                    "dummy",
                    Vec3::new(x as f32, 0., z as f32),
                    0.,
                    100,
                ));
            }
            Ok(())
        },
        populate: |game, _| {
            let crate_spec = PropSpec::reference(PropKind::Crate);
            let barrel = PropSpec::reference(PropKind::Barrel);
            let mut plank = PropSpec::reference(PropKind::Crate);
            plank.dimensions = DVec3::new(2.4, 0.1, 0.3);
            plank.mass = 6.;
            for (name, x, z) in [("Crate 1", -8., -4.), ("Crate 2", -11.5, -7.5)] {
                game.spawn_prop(name, crate_spec.clone(), Vec3::new(x, 0.3, z), 0.3)?;
            }
            for (name, x, z) in [("Barrel 1", -9., -8.), ("Barrel 2", -12., -5.)] {
                game.spawn_prop(name, barrel.clone(), Vec3::new(x, 0.45, z), 0.)?;
            }
            for i in 0..5 {
                game.spawn_prop(
                    &format!("Plank {}", i + 1),
                    plank.clone(),
                    Vec3::new(-7.5, 0.05 + 0.102 * i as f32, -1.5 + 0.08 * i as f32),
                    0.15 * i as f32,
                )?;
            }
            game.spawn_prop(
                "Secured crate",
                crate_spec.clone().secured(),
                Vec3::new(-5.6, 0.3, -10.8),
                0.,
            )?;
            // The indoor variant: a stone slab whose underside sits 40 feet
            // (plus a hair) above the dummies' heads.
            let mut slab = PropSpec::reference(PropKind::StoneBlock).secured();
            slab.dimensions = DVec3::new(6., 0.6, 6.);
            game.spawn_prop(
                "Ceiling slab",
                slab,
                Vec3::new(-10., (SLAB_UNDERSIDE + 0.3) as f32, 3.),
                0.,
            )?;
            game.spawn_prop(
                "Crate (indoor)",
                crate_spec.clone(),
                Vec3::new(-10.5, 0.3, 1.2),
                0.,
            )?;
            game.spawn_prop("Barrel (indoor)", barrel, Vec3::new(-8., 0.45, 1.5), 0.)?;
            let center = DVec3::new(CENTER_X, 0., CENTER_Z);
            for (name, x, z) in [("Edge crate 1", -3., 4.), ("Edge crate 2", -1.5, 3.)] {
                let index = game.spawn_prop(name, crate_spec.clone(), Vec3::new(x, 0.3, z), 0.)?;
                let out = (DVec3::new(x as f64, 0., z as f64) - center).normalize();
                game.spells.reverse_gravity.shoves.push(Shove {
                    at: 5.,
                    prop: index,
                    velocity: out * 3.5,
                });
            }
            // Fails for A, B, C, F, and G; D saves with nothing in reach;
            // E saves beside the pillar.
            for (dummy, roll) in [
                (101, 3),
                (102, 5),
                (103, 7),
                (104, 19),
                (105, 17),
                (106, 2),
                (107, 4),
            ] {
                game.spells.dice.force_save(dummy, roll)?;
            }
            Ok(())
        },
        script: || {
            vec![
                // Face the cylinder's center; with no other target selected
                // the cylinder stands 60 feet ahead, on Dummy A.
                Cue {
                    at: 0.4,
                    step: Step::Face(PLAYGROUND_YAW),
                },
                Cue {
                    at: 1.0,
                    step: Step::Cast(crate::play::Ability::Spell(SLOT)),
                },
                Cue {
                    at: 10.0,
                    step: Step::Cast(crate::play::Ability::Spell(SLOT)),
                },
            ]
        },
        camera: || {
            let start = (Vec3::new(9.5, 3.0, 6.0), Vec3::new(-9., 2.0, -4.));
            let wide = (Vec3::new(21., 16., 21.), Vec3::new(-13., 14., -3.));
            let pillar = (Vec3::new(0.5, 3.5, -5.5), Vec3::new(-5.2, 1.4, -12.));
            [
                (0., start),
                (0.9, start),
                (1.6, wide),
                (3.8, wide),
                (4.3, pillar),
                (4.9, pillar),
                (5.4, wide),
                (14., wide),
            ]
            .into_iter()
            .map(|(at, (eye, target))| Shot { at, eye, target })
            .collect()
        },
        replay_camera: (Vec3::new(20., 8., 20.), Vec3::new(-11., 9., -3.)),
        check: check_scenario,
    }
}

/// The wizard's yaw toward the playground cylinder, rad.
const PLAYGROUND_YAW: f32 = 1.208;
/// Center of the playground cylinder, m: [`AIM_DISTANCE`] ahead of the
/// caster spawn at (5.75, 0, 0) along [`PLAYGROUND_YAW`]. Dummy A stands
/// there.
const CENTER_X: f64 = -11.34;
const CENTER_Z: f64 = -6.51;
/// The indoor slab's underside: 40 feet above a dummy's head, m.
const SLAB_UNDERSIDE: f64 = CHARACTER_HEIGHT + 40. * FOOT + 0.05;

fn check_scenario(game: &Game) -> Result<(), String> {
    let log: Vec<&str> = game.spells.log.iter().map(|r| r.text.as_str()).collect();
    let has = |needle: &str| log.iter().any(|t| t.contains(needle));
    for needle in [
        "Dummy E (clings): DEX save 17",
        "grabs a fixed object",
        "nothing is in reach",
        "Dummy F (indoor) slams into the ceiling after 40 ft",
        "Dummy G (indoor) slams into the ceiling after 40 ft",
        "Crate (indoor) cracks against the ceiling",
        "Dummy A hovers at the top, 100 ft up",
        "Edge crate 1 crashes down",
        "Dismissed",
        "The spell ends",
    ] {
        if !has(needle) {
            return Err(format!("The log never says \"{needle}\": {log:?}"));
        }
    }
    if !log
        .iter()
        .any(|t| t.starts_with("Dummy F (indoor) fell 40 ft: 4d6"))
    {
        return Err(format!("Dummy F took no 4d6 ceiling strike: {log:?}"));
    }
    if !log
        .iter()
        .any(|t| t.starts_with("Dummy A fell 100 ft: 10d6"))
    {
        return Err(format!("Dummy A did not fall 100 ft for 10d6: {log:?}"));
    }
    for actor in [101, 102, 103, 104, 105, 106, 107] {
        let feet = game
            .actor_position(actor)
            .ok_or(format!("Actor {actor} is gone"))?;
        if feet.y > 0.05 {
            return Err(format!("Actor {actor} is still {:.2} m up", feet.y));
        }
    }
    let clinging = game.actor_position(105).unwrap();
    if clinging.distance(Vec3::new(-4.6, 0., -12.)) > 0.05 {
        return Err(format!("Dummy E moved to {clinging}"));
    }
    let center = DVec3::new(CENTER_X, 0., CENTER_Z);
    for (index, prop) in game.spells.props.iter().enumerate() {
        if prop.removed {
            continue;
        }
        let at = game.spells.prop_center(index);
        if prop.name.starts_with("Edge crate") {
            let d = DVec3::new(at.x - center.x, 0., at.z - center.z).length();
            if d <= RADIUS || at.y > 1. {
                return Err(format!("{} rests at {at}, inside the cylinder", prop.name));
            }
        } else if !prop.spec.secured && at.y > 1.5 {
            return Err(format!("{} is still {:.2} m up", prop.name, at.y));
        }
    }
    let secured = game
        .spells
        .props
        .iter()
        .position(|p| p.name == "Secured crate")
        .ok_or("No secured crate")?;
    if game
        .spells
        .prop_center(secured)
        .distance(DVec3::new(-5.6, 0.3, -10.8))
        > 1e-6
    {
        return Err("The secured crate moved".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spells::{LEDGER_TOLERANCE, PROP_ENTITY_BASE, PropKind, PropSpec, SpellWorld};
    use physics::kinematic::Aabb;

    fn floor() -> SpellWorld {
        SpellWorld::new(
            &[Aabb {
                min: DVec3::new(-40., -0.6, -40.),
                max: DVec3::new(40., 0., 40.),
            }],
            1,
        )
    }

    fn life(n: u64) -> physics::queries::Life {
        physics::queries::Life {
            instance: 0,
            entity: PROP_ENTITY_BASE + n,
            generation: 0,
        }
    }

    fn field(cast: u64) -> SpellField {
        SpellField {
            cast,
            spell: NAME.into(),
            owner: 1,
            area: Area::Cylinder {
                base: DVec3::ZERO,
                radius: RADIUS,
                height: HEIGHT,
            },
            acceleration: DVec3::Y * 2. * GRAVITY,
            expires: 60.,
            concentration: true,
        }
    }

    fn frames(world: &mut SpellWorld, seconds: f32) {
        for _ in 0..(seconds * 30.) as u32 {
            world.begin_tick();
            world.step(4, 1.).unwrap();
        }
    }

    #[test]
    fn props_rise_and_hover_at_the_top_with_a_balanced_ledger() {
        let mut world = floor();
        let spec = PropSpec::reference(PropKind::Crate);
        let crate_ = world
            .add_prop(life(0), "crate", spec.clone(), DVec3::Y * 0.3, 0., None)
            .unwrap();
        let outside = world
            .add_prop(
                life(1),
                "outside",
                spec.clone(),
                DVec3::new(16., 0.3, 0.),
                0.,
                None,
            )
            .unwrap();
        let secured = world
            .add_prop(
                life(2),
                "secured",
                spec.secured(),
                DVec3::new(3., 0.3, 0.),
                0.,
                None,
            )
            .unwrap();
        frames(&mut world, 1.);
        let cast = world.begin_cast(1, true).unwrap();
        world.add_field(field(cast)).unwrap();
        let body = world.props[crate_].body;
        world.world.wake(body);
        let mut highest = f64::MIN;
        for _ in 0..300 {
            world.begin_tick();
            world.step(4, 1.).unwrap();
            highest = highest.max(world.prop_center(crate_).y);
        }
        assert!(highest <= HEIGHT + 0.2, "{highest}");
        assert!((world.prop_center(crate_).y - HEIGHT).abs() < 0.2);
        assert!((world.prop_center(outside).y - 0.3).abs() < 0.01);
        assert!(world.prop_center(secured).distance(DVec3::new(3., 0.3, 0.)) < 1e-12);
        let error = world.ledger_error();
        assert!(error.linear < LEDGER_TOLERANCE, "{error:?}");
        // Ending the cast drops it.
        world.end_concentration(1).unwrap();
        world.world.wake(body);
        frames(&mut world, 4.);
        assert!((world.prop_center(crate_).y - 0.3).abs() < 0.05);
    }

    #[test]
    fn the_exact_rise_hovers_without_overshooting() {
        let (mut y, mut v) = (0., 0.);
        let mut highest = f64::MIN;
        for _ in 0..600 {
            (y, v) = rise(y, v, HEIGHT, 1. / 30.);
            highest = highest.max(y);
        }
        assert!(highest <= HEIGHT + 1e-9, "{highest}");
        assert!((y - HEIGHT).abs() < 1e-6 && v.abs() < 1e-6);
    }

    #[test]
    fn a_governed_character_strikes_a_roof_once_and_presses_against_it() {
        let active = Active {
            cast: 1,
            caster: 1,
            cylinder: Cylinder {
                base: DVec3::ZERO,
                radius: RADIUS,
                height: CHAMBER_ROOF,
            },
            roof: Some(CHAMBER_ROOF),
            ended: None,
            creatures: vec![],
            props: vec![],
        };
        let mut character = physics::character::Character::new(DVec3::ZERO);
        let mut creature = Creature {
            actor: 2,
            save: None,
            held: None,
            inside: false,
            governed: false,
            set_speed: 0.,
            speed: 0.,
            low: 0.,
            hovering: false,
        };
        let dt = 1. / 30.;
        let mut strikes = vec![];
        for _ in 0..120 {
            let outcome = govern(&mut character, &mut creature, &active, true, dt);
            strikes.extend(outcome.strike);
            // What the capsule step does with no obstacle in the way.
            character.feet.y += character.vertical_speed * dt;
        }
        assert_eq!(strikes.len(), 1, "{strikes:?}");
        let limit = CHAMBER_ROOF - CHARACTER_HEIGHT;
        assert!((strikes[0] - limit).abs() < 1e-9);
        assert!((character.feet.y - limit).abs() < 1e-9);
        assert_eq!(crate::spells::fall_dice(strikes[0]), 3);
        // The spell ends: the character's own gravity returns.
        govern(&mut character, &mut creature, &active, false, dt);
        assert!(character.gravity.is_none() && !creature.governed);
    }

    #[test]
    fn grabs_need_a_fixed_object_above_the_feet_within_five_feet() {
        let world = SpellWorld::new(
            &[
                Aabb {
                    min: DVec3::new(-40., -0.6, -40.),
                    max: DVec3::new(40., 0., 40.),
                },
                Aabb {
                    min: DVec3::new(-0.7, 0., -0.7),
                    max: DVec3::new(0.7, 10., 0.7),
                },
            ],
            1,
        );
        assert!(reachable_fixed(&world.world, DVec3::new(1.4, 0., 0.)).is_some());
        assert!(reachable_fixed(&world.world, DVec3::new(2.3, 0., 0.)).is_none());
        assert!(reachable_fixed(&world.world, DVec3::new(10., 0., 0.)).is_none());
    }

    #[test]
    fn the_playground_scenario_plays_replays_and_passes_its_check() {
        let mut run =
            crate::playground::Run::new(crate::playground::scenario("reverse-gravity").unwrap())
                .unwrap();
        while !run.done() {
            run.advance().unwrap();
        }
        (run.scenario.check)(run.result()).unwrap();
        assert_eq!(run.replay_identical, Some(true));
    }

    #[test]
    fn chamber_cultists_strike_the_roof_and_drop_when_the_caster_dies() {
        let scene = verse_engine::director::Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut game = Game::new(scene).unwrap();
        game.time = 21.;
        for _ in 0..10 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
        // The first cultist, 13.8 m from the adventurer.
        let target = game.selected;
        game.spells.dice.force_save(target, 1).unwrap();
        game.activate(crate::play::Ability::Spell(SLOT)).unwrap();
        let mut highest = 0f32;
        for _ in 0..90 {
            game.tick(1. / 30., [0.; 2]).unwrap();
            highest = highest.max(game.actor_position(target).map_or(0., |p| p.y));
        }
        let limit = (CHAMBER_ROOF - CHARACTER_HEIGHT) as f32;
        assert!((highest - limit).abs() < 0.05, "{highest}");
        assert!(
            game.spells
                .log
                .iter()
                .any(|r| r.text.contains("slams into the ceiling")),
            "{:?}",
            game.spells.log
        );
        let saved = game.checkpoint().unwrap();
        crate::play::Game::restore(&saved).unwrap();
        // Concentration ends with the caster, as on death: every creature
        // gets its own gravity back and falls.
        let caster = game.player_actor();
        game.spells.end_concentration(caster).unwrap();
        for _ in 0..60 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert!(game.npc_characters.values().all(|c| c.gravity.is_none()));
        assert!(game.character.gravity.is_none());
        assert!(
            game.spells
                .log
                .iter()
                .any(|r| r.text.contains("The spell ends"))
        );
    }
}
