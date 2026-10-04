//! Levitate in the chamber and the spell playground.
//!
//! SRD 5.2.1: level 2 Transmutation, casting time Action, range 60 feet,
//! concentration up to 10 minutes. One creature or loose object of up to
//! 500 pounds rises up to 20 feet; an unwilling creature makes a
//! Constitution save. The mechanics (admission, altitude hold, push-off,
//! capped descent) are in [`crate::levitate`]. This module picks the
//! target, runs the cast through the spell world, and applies the spell to
//! characters and props on every chamber tick.
//!
//! The cast aims along the caster's facing: it takes the creature or prop
//! nearest the facing within [`AIM_CONE`] and 60 feet that the caster can
//! see. Aiming at the current target, or at nothing while concentrating on
//! Levitate, spends the cast as the Magic action that changes the target's
//! altitude by 20 feet: down from the upper half of the band, up from the
//! lower half. Aiming at nothing otherwise lifts the caster.
//!
//! A levitated character does not walk. Its walking input, or a willing
//! creature's scripted input, becomes a push-off against a surface within
//! 5 feet, capped at climbing speed. Its gravity override is zero and the
//! hold sets its vertical speed toward the commanded height. When the spell
//! ends, gravity returns with Feather Fall's terminal speed, and landing
//! deals no falling damage.
use super::{CHARACTER_HEIGHT, CHARACTER_RADIUS, CONTACT_RESTITUTION, FEET, SPELL_SAVE_DC};
use super::{Target, Track};
use crate::levitate::{
    self as mechanics, ALTITUDE_STEP, AltitudeRefusal, CLIMB_SPEED, DURATION, FEATHER_FALL_SPEED,
    Levitation, MAX_RISE, Phase, RANGE, REACH, Subject,
};
use crate::play::Game;
use glam::{DVec3, Vec3};
use physics::character::{Character, GravityOverride, TERMINAL_SPEED};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub use crate::levitate::End;

pub const NAME: &str = "Levitate";
/// Row-two slot: Shift+3.
pub const SLOT: u8 = 2;
/// Half-angle of the cone along the caster's facing that picks a target,
/// rad.
pub const AIM_CONE: f64 = 20. * std::f64::consts::PI / 180.;
/// Chamber duration, s: MMO tuning that shortens the SRD's 10 minutes, so
/// a levitated cultist does not hang out of the fight for the whole
/// encounter. The playground uses the SRD duration.
pub const CHAMBER_DURATION: f64 = 30.;
/// The action-bar entry.
pub const DEF: super::SpellDef = super::SpellDef {
    slot: SLOT,
    key: "levitate",
    label: "Levitate",
    icon: "levitate-icon",
    description: "Lift a creature or loose object up to 20 ft; it moves only by pushing off surfaces",
    cost: 2,
    cooldown: 1.5,
    cast,
};
/// Smallest blocked speed that counts as a bounce, m/s.
const BOUNCE_MIN: f64 = 0.05;
/// Height of a character's center above its feet, m.
const CENTER: f64 = 0.9;
/// A descending prop this close to its resting height has landed, m.
const LANDED: f64 = 0.03;

/// One levitated target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Held {
    pub cast: u64,
    pub caster: u64,
    pub target: Target,
    pub label: String,
    pub spell: Levitation,
    /// When the spell ends on its own, scene seconds.
    pub until: f64,
    /// A creature's external motion after the last update, m/s; what the
    /// sweep removed since then is a collision.
    pub drift: DVec3,
    /// The creature pushed off in the last tick.
    pub pushing: bool,
    /// The phase the spell log last reported.
    pub reported: Phase,
}

/// A willing creature's own movement input over a time window: trusted
/// scenario setup, like a forced save.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Intent {
    pub actor: u64,
    pub from: f32,
    pub until: f32,
    /// Horizontal, length at most 1.
    pub input: DVec3,
}

/// Every active levitation, saved with the spell world.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Levitations {
    pub active: Vec<Held>,
    /// Creatures that accept the spell without a save (trusted setup).
    pub willing: BTreeSet<u64>,
    pub intents: Vec<Intent>,
    /// Admitted walking input of levitated characters this tick, by actor.
    pub input: BTreeMap<u64, DVec3>,
}

impl Levitations {
    pub fn validate(&self, props: usize, casts: u64) -> Result<(), String> {
        let unit = |v: &DVec3| v.is_finite() && v.length() <= 1. + 1e-6;
        if self.active.len() > 64
            || self.willing.len() > 256
            || self.intents.len() > 64
            || self.input.len() > 64
            || self.active.iter().any(|h| {
                h.cast > casts
                    || matches!(h.target, Target::Prop(i) if i >= props)
                    || !h.spell.base.is_finite()
                    || !(0. ..=MAX_RISE).contains(&h.spell.rise)
                    || !h.spell.cast_at.is_finite()
                    || !h.until.is_finite()
                    || !h.drift.is_finite()
                    || h.label.len() > 64
            })
            || self
                .intents
                .iter()
                .any(|i| !unit(&i.input) || !i.from.is_finite() || !i.until.is_finite())
            || !self.input.values().all(unit)
        {
            return Err("Invalid levitation checkpoint".into());
        }
        Ok(())
    }

    /// Whether the spell holds `actor` aloft or is lowering it. Such a
    /// character neither walks nor follows its controller.
    pub fn holds(&self, actor: u64) -> bool {
        self.active
            .iter()
            .any(|h| h.target == Target::Actor(actor) && !matches!(h.spell.phase, Phase::Done(_)))
    }

    /// A cast ended (concentration moved on or was lost): its targets float
    /// down.
    pub fn end_cast(&mut self, cast: u64) {
        for held in self.active.iter_mut().filter(|h| h.cast == cast) {
            held.spell.end(End::Concentration);
        }
    }

    /// Applies the hold and the drift damping to levitated props for one
    /// fixed step, recording both in `ledger`.
    pub fn drive(
        &self,
        world: &mut physics::World,
        ledger: &mut physics::Ledger,
        props: &[super::Prop],
    ) {
        for held in &self.active {
            if let Target::Prop(index) = held.target
                && let Some(prop) = props.get(index)
                && !prop.removed
                && !prop.spec.secured
            {
                held.spell.drive(world, prop.body, super::GRAVITY, ledger);
            }
        }
    }

    fn intent(&self, actor: u64, now: f32) -> Option<DVec3> {
        self.intents
            .iter()
            .find(|i| i.actor == actor && (i.from..i.until).contains(&now))
            .map(|i| i.input)
    }
}

fn reason(end: End) -> &'static str {
    match end {
        End::Concentration => "concentration ends",
        End::Duration => "the duration ends",
        End::OutOfRange => "out of the 60-ft range",
        End::TargetGone => "the target is gone",
    }
}

/// Living hostile creatures and their feet.
fn creatures(game: &Game) -> Vec<(u64, DVec3)> {
    let snapshot = game.snapshot();
    game.ids
        .iter()
        .filter_map(|(actor, id)| {
            snapshot
                .actors
                .iter()
                .find(|a| a.id == *id && a.alive && a.faction == "undead")
                .map(|a| (*actor, Vec3::from(a.pos).as_dvec3()))
        })
        .collect()
}

/// A target's reference point: a creature's center or a prop's box center.
fn position(game: &Game, target: Target) -> Option<DVec3> {
    match target {
        Target::Actor(actor) => game
            .actor_position(actor)
            .map(|p| p.as_dvec3() + DVec3::Y * CENTER),
        Target::Prop(index) => {
            let prop = game.spells.props.get(index)?;
            (!prop.removed).then(|| game.spells.prop_center(index))
        }
    }
}

fn label(game: &Game, target: Target) -> String {
    match target {
        Target::Actor(actor) if actor == game.player_actor() => "Wizard (self)".into(),
        Target::Actor(actor) => game.actor_name(actor),
        Target::Prop(index) => game.spells.props[index].name.clone(),
    }
}

fn visible(game: &Game, eye: DVec3, point: DVec3) -> bool {
    physics::kinematic::sweep_box(eye, DVec3::splat(0.12), point - eye, &game.colliders)
        .is_ok_and(|hit| hit.is_none())
}

/// The target the caster aims at, if any.
fn aim(game: &Game, feet: DVec3, facing: DVec3) -> Option<Target> {
    let eye = feet + DVec3::Y * 1.4;
    let center = feet + DVec3::Y * CENTER;
    let caster = game.player_actor();
    let mut candidates: Vec<(Target, DVec3)> = creatures(game)
        .into_iter()
        .filter(|(actor, _)| *actor != caster)
        .map(|(actor, feet)| (Target::Actor(actor), feet + DVec3::Y * CENTER))
        .collect();
    for (index, prop) in game.spells.props.iter().enumerate() {
        if !prop.removed {
            candidates.push((Target::Prop(index), game.spells.prop_center(index)));
        }
    }
    candidates
        .into_iter()
        .filter_map(|(target, point)| {
            let d = point - eye;
            let flat = DVec3::new(d.x, 0., d.z);
            if flat.length() < 1e-6 {
                return None;
            }
            let angle = flat.angle_between(facing);
            (angle <= AIM_CONE && point.distance(center) <= RANGE && visible(game, eye, point))
                .then_some((target, angle, flat.length()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map(|(target, ..)| target)
}

/// Runs an admitted Levitate from the caster's place and facing.
pub fn cast(game: &mut Game) -> Result<(), String> {
    if game.colliders.is_empty() {
        return Err("Levitate needs a scene with a collision profile".into());
    }
    let caster = game.player_actor();
    let feet = game.player.as_dvec3();
    let yaw = f64::from(game.yaw);
    let facing = DVec3::new(-yaw.sin(), 0., -yaw.cos());
    let now = f64::from(game.time);
    let aimed = aim(game, feet, facing);
    let current = game
        .spells
        .concentration
        .get(&caster)
        .copied()
        .and_then(|cast| {
            game.spells
                .levitations
                .active
                .iter()
                .position(|h| h.cast == cast && h.spell.holding())
        });
    match (aimed, current) {
        (Some(target), Some(index)) if game.spells.levitations.active[index].target == target => {
            command(game, index, now)
        }
        (None, Some(index)) => command(game, index, now),
        (Some(target), _) => lift(game, caster, target, now),
        (None, None) => lift(game, caster, Target::Actor(caster), now),
    }
}

/// The Magic action: move the current target 20 feet down or up.
fn command(game: &mut Game, index: usize, now: f64) -> Result<(), String> {
    let held = game.spells.levitations.active[index].clone();
    let caster = game.player.as_dvec3() + DVec3::Y * CENTER;
    let distance = position(game, held.target).map_or(f64::INFINITY, |p| p.distance(caster));
    if distance > RANGE {
        return Err(format!("{} is out of range", held.label));
    }
    let delta = if held.spell.rise > MAX_RISE * 0.5 {
        -ALTITUDE_STEP
    } else {
        ALTITUDE_STEP
    };
    let result = game.spells.levitations.active[index]
        .spell
        .command(delta, now);
    match result {
        Ok(rise) => {
            game.spells.record(
                game.time,
                NAME,
                format!(
                    "{}: altitude {:+.0} ft, now {:.0} ft above the ground",
                    held.label,
                    delta / FEET,
                    rise / FEET
                ),
                None,
            );
            Ok(())
        }
        Err(AltitudeRefusal::ThisTurn { wait }) => Err(format!(
            "{}'s altitude changes once per turn; {wait:.1} s left",
            held.label
        )),
        Err(AltitudeRefusal::TooFar) => Err("An altitude change is at most 20 ft".into()),
        Err(AltitudeRefusal::NotHolding) => Err(format!("{} is not levitating", held.label)),
    }
}

/// The movement controller of a creature target, made consistent with its
/// admitted feet.
fn character_mut(game: &mut Game, actor: u64, feet: DVec3) -> &mut Character {
    if actor == game.player_actor() {
        return &mut game.character;
    }
    let character = game
        .npc_characters
        .entry(actor)
        .or_insert_with(|| Character::new(feet));
    if character.feet.as_vec3() != feet.as_vec3() {
        *character = Character::new(feet);
    }
    character
}

/// A new levitation on `target`.
fn lift(game: &mut Game, caster: u64, target: Target, now: f64) -> Result<(), String> {
    let caster_center = game.player.as_dvec3() + DVec3::Y * CENTER;
    let center = position(game, target).ok_or("The target is gone")?;
    let distance = center.distance(caster_center);
    let label = label(game, target);
    // Range refusal costs nothing: the spell has no target to take.
    if distance > RANGE {
        return Err(format!(
            "{label} is out of range ({:.0} ft)",
            distance / FEET
        ));
    }
    let (subject, actor) = match target {
        Target::Prop(index) => {
            let spec = &game.spells.props[index].spec;
            (
                Subject::Object {
                    mass: spec.mass,
                    secured: spec.secured,
                },
                0,
            )
        }
        Target::Actor(actor) => {
            let model = game
                .scene
                .actors
                .iter()
                .find(|a| a.id == actor)
                .map(|a| a.model.clone())
                .unwrap_or_default();
            (
                Subject::Creature {
                    willing: actor == caster || game.spells.levitations.willing.contains(&actor),
                    constitution: super::constitution_modifier(&model),
                },
                actor,
            )
        }
    };
    let mut save = None;
    let admitted = {
        let dice = &mut game.spells.dice;
        mechanics::admit(subject, distance, || {
            let modifier = match subject {
                Subject::Creature { constitution, .. } => constitution,
                Subject::Object { .. } => 0,
            };
            let rolled = dice.save(actor, "Constitution", modifier, SPELL_SAVE_DC);
            let roll = rolled.roll as i32;
            save = Some(rolled);
            roll
        })
    };
    let save_text = save.as_ref().map_or(String::new(), |s| {
        format!(
            "CON save {} {:+} = {} vs DC {} {}; ",
            s.roll,
            s.modifier,
            s.total,
            s.dc,
            if s.success { "succeeds" } else { "fails" }
        )
    });
    if let Err(refusal) = admitted {
        // The spell takes no hold, but the cast is spent, as any failed spell.
        let text = match refusal {
            mechanics::Refusal::Saved(_) => format!("{label}: {save_text}unaffected"),
            refusal => format!("{label}: refused, {}", refusal.reason()),
        };
        game.spells.record(game.time, NAME, text, save);
        return Ok(());
    }
    let cast = game.spells.begin_cast(caster, true)?;
    let duration = if game.scene.collision_profile.as_deref() == Some(crate::playground::PROFILE) {
        DURATION
    } else {
        CHAMBER_DURATION
    };
    let (base, start) = match target {
        Target::Actor(actor) => {
            let feet = center - DVec3::Y * CENTER;
            let world = &game.spells.world;
            let ground = world
                .raycast(feet + DVec3::Y * 0.05, -DVec3::Y, 1000., &|c| {
                    !world[c.body].removed
                })
                .map_or(feet.y, |hit| hit.point.y.min(feet.y));
            let character = character_mut(game, actor, feet);
            character.gravity = Some(GravityOverride {
                scale: 0.,
                terminal: TERMINAL_SPEED,
            });
            character.peak = Some(character.feet.y);
            (ground, feet)
        }
        Target::Prop(index) => {
            let body = game.spells.props[index].body;
            let rest = mechanics::resting_height(&game.spells.world, body)
                .unwrap_or(game.spells.world[body].pos.y)
                .min(game.spells.world[body].pos.y);
            game.spells.world.wake(body);
            (rest, center)
        }
    };
    let self_target = target == Target::Actor(caster);
    let levitations = &mut game.spells.levitations;
    levitations.active.retain(|h| h.target != target);
    levitations.active.push(Held {
        cast,
        caster,
        target,
        label: label.clone(),
        spell: Levitation::new(base, MAX_RISE, self_target, now),
        until: now + duration,
        drift: DVec3::ZERO,
        pushing: false,
        reported: Phase::Holding,
    });
    game.spells.track(Track {
        label: label.clone(),
        target,
        spell: NAME.into(),
        at: game.time,
        start,
        requested: 0.,
    });
    game.spells.record(
        game.time,
        NAME,
        format!("{label}: {save_text}rises up to 20 ft"),
        save,
    );
    Ok(())
}

impl Game {
    /// A levitated character's walking input becomes push-off input for the
    /// spell's next update; the character itself does not walk.
    pub(crate) fn levitated_walk(&mut self, actor: u64, velocity: DVec3) -> DVec3 {
        if !self.spells.levitations.holds(actor) {
            return velocity;
        }
        let input =
            (DVec3::new(velocity.x, 0., velocity.z) / mechanics::WALK_SPEED).clamp_length_max(1.);
        if input.length_squared() > 1e-12 {
            self.spells.levitations.input.insert(actor, input);
        }
        DVec3::ZERO
    }

    /// Applies every levitation after the tick's character and prop motion:
    /// range and duration, bounces, push-offs, the hold, and landings.
    pub(crate) fn levitate_tick(&mut self, steps: usize) -> Result<(), String> {
        let inputs = std::mem::take(&mut self.spells.levitations.input);
        if self.spells.levitations.active.is_empty() {
            return Ok(());
        }
        let dt = steps as f64 * self.physics_clock.dt;
        let player = self.player_actor();
        let player_alive = self.snapshot().player.hp > 0;
        let living: BTreeSet<u64> = creatures(self).into_iter().map(|(a, _)| a).collect();
        let mut lines = vec![];
        let mut released = vec![];
        for index in 0..self.spells.levitations.active.len() {
            let mut held = self.spells.levitations.active[index].clone();
            let caster = self
                .actor_position(held.caster)
                .map(|p| p.as_dvec3() + DVec3::Y * CENTER);
            let was_holding = held.spell.holding();
            match held.target {
                Target::Prop(prop) => self.levitate_prop(&mut held, prop, caster, &mut lines),
                Target::Actor(actor) => {
                    let alive = if actor == player {
                        player_alive
                    } else {
                        living.contains(&actor)
                    };
                    let input = inputs
                        .get(&actor)
                        .copied()
                        .or_else(|| self.spells.levitations.intent(actor, self.time));
                    self.levitate_creature(&mut held, actor, alive, caster, input, dt, &mut lines)?;
                }
            }
            if held.spell.phase != held.reported {
                if let Phase::Descending(end) = held.spell.phase {
                    lines.push(format!(
                        "{}: {}; floats down at {:.0} ft/s or less",
                        held.label,
                        reason(end),
                        FEATHER_FALL_SPEED / FEET
                    ));
                }
                held.reported = held.spell.phase;
            }
            if was_holding
                && !held.spell.holding()
                && !matches!(
                    held.spell.phase,
                    Phase::Descending(End::Concentration) | Phase::Done(End::Concentration)
                )
            {
                released.push(held.cast);
            }
            self.spells.levitations.active[index] = held;
        }
        self.spells
            .levitations
            .active
            .retain(|h| !matches!(h.spell.phase, Phase::Done(_)));
        self.spells
            .concentration
            .retain(|_, cast| !released.contains(cast));
        for text in lines {
            self.spells.record(self.time, NAME, text, None);
        }
        Ok(())
    }

    fn levitate_prop(
        &mut self,
        held: &mut Held,
        index: usize,
        caster: Option<DVec3>,
        lines: &mut Vec<String>,
    ) {
        let Some(prop) = self.spells.props.get(index) else {
            held.spell.end(End::TargetGone);
            held.spell.land();
            return;
        };
        if prop.removed {
            held.spell.end(End::TargetGone);
            held.spell.land();
            return;
        }
        let body = prop.body;
        let now = f64::from(self.time);
        if held.spell.holding() {
            let center = self.spells.prop_center(index);
            held.spell
                .update(now, caster.map_or(f64::INFINITY, |c| c.distance(center)));
            if now >= held.until {
                held.spell.end(End::Duration);
            }
        }
        if let Phase::Descending(_) = held.spell.phase
            && let Some(rest) = mechanics::resting_height(&self.spells.world, body)
            && self.spells.world[body].pos.y - rest < LANDED
        {
            held.spell.land();
            lines.push(format!("{}: settles gently on the ground", held.label));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn levitate_creature(
        &mut self,
        held: &mut Held,
        actor: u64,
        alive: bool,
        caster: Option<DVec3>,
        input: Option<DVec3>,
        dt: f64,
        lines: &mut Vec<String>,
    ) -> Result<(), String> {
        let now = f64::from(self.time);
        let player = self.player_actor();
        let mass = super::model_size(
            self.scene
                .actors
                .iter()
                .find(|a| a.id == actor)
                .map_or("", |a| a.model.as_str()),
        )
        .creature_mass();
        let character = if actor == player {
            Some(&mut self.character)
        } else {
            self.npc_characters.get_mut(&actor)
        };
        let Some(character) = character else {
            held.spell.end(End::TargetGone);
            held.spell.land();
            return Ok(());
        };
        if !alive {
            held.spell.end(End::TargetGone);
        }
        if held.spell.holding() {
            let center = character.feet + DVec3::Y * CENTER;
            held.spell
                .update(now, caster.map_or(f64::INFINITY, |c| c.distance(center)));
            if now >= held.until {
                held.spell.end(End::Duration);
            }
        }
        let capsule = |feet: DVec3| {
            (
                feet + DVec3::Y * CHARACTER_RADIUS,
                feet + DVec3::Y * (CHARACTER_HEIGHT - CHARACTER_RADIUS),
            )
        };
        match held.spell.phase {
            Phase::Holding => {
                // What the sweep removed from the drift since the last update
                // is a collision: the target bounces back with the larger of
                // the surface's restitution and a character contact's.
                let blocked = held.drift - character.external;
                if blocked.length() > BOUNCE_MIN && blocked.dot(held.drift) > 0. {
                    let (a, b) = capsule(character.feet);
                    let surface = mechanics::capsule_surface(
                        &self.spells.world,
                        a,
                        b,
                        CHARACTER_RADIUS,
                        REACH,
                        &|_| true,
                    );
                    let restitution = surface
                        .map_or(0., |s| {
                            self.spells.world.colliders()[s.collider.0 as usize]
                                .material
                                .restitution
                        })
                        .max(CONTACT_RESTITUTION);
                    character.external -= blocked * restitution;
                    character.external.y = 0.;
                    lines.push(format!(
                        "{}: bounces, {:.1} -> {:.1} ft/s (restitution {restitution:.2})",
                        held.label,
                        held.drift.length() / FEET,
                        character.external.length() / FEET
                    ));
                }
                character.external *= (1. - mechanics::DRIFT_DAMPING * dt).max(0.);
                let mut pushed = false;
                if let Some(input) = input {
                    let (a, b) = capsule(character.feet);
                    let surface = mechanics::capsule_surface(
                        &self.spells.world,
                        a,
                        b,
                        CHARACTER_RADIUS,
                        REACH,
                        &|_| true,
                    );
                    let change = mechanics::push_off(input, character.external, surface.as_ref());
                    if change != DVec3::ZERO {
                        character.add_velocity(change);
                        pushed = true;
                        if let Some(surface) = surface
                            && let Some(prop) = self
                                .spells
                                .props
                                .iter()
                                .position(|p| p.body == surface.body)
                        {
                            // A loose prop takes the reaction.
                            self.spells.impulse_prop(
                                prop,
                                -change * mass,
                                surface.point,
                                "levitate push-off",
                            )?;
                        }
                        if !held.pushing {
                            lines.push(format!(
                                "{}: pushes off at {:.1} ft/s (climbing cap {:.1} ft/s)",
                                held.label,
                                character.external.length() / FEET,
                                CLIMB_SPEED / FEET
                            ));
                        }
                    }
                }
                held.pushing = pushed;
                character.gravity = Some(GravityOverride {
                    scale: 0.,
                    terminal: TERMINAL_SPEED,
                });
                character.vertical_speed = held.spell.hold_speed(character.feet.y);
                // Moving under the spell is never a fall.
                character.peak = Some(character.feet.y);
                held.drift = character.external;
            }
            Phase::Descending(_) => {
                if character.support.is_some() {
                    held.spell.land();
                    character.gravity = None;
                    lines.push(format!("{}: lands gently, no falling damage", held.label));
                } else {
                    character.gravity = Some(GravityOverride {
                        scale: 1.,
                        terminal: FEATHER_FALL_SPEED,
                    });
                    character.peak = Some(character.feet.y);
                }
            }
            Phase::Done(_) => character.gravity = None,
        }
        Ok(())
    }
}

/// The playground recording, in the hall's east half. A willing dummy
/// rises beside the east pillar, pushes off, drifts across the hall, and
/// bounces off the east wall. Levitating a crate ends the dummy's spell, so
/// it floats down from 20 ft without damage. The crate drops 20 ft and
/// rises 20 ft on consecutive turns, then a Thunderwave blows it across
/// the hall while it floats. An anvil with a load (300 kg) is refused, and
/// an unwilling dummy makes its save.
pub fn scenario() -> crate::playground::Scenario {
    use crate::play::Ability;
    use crate::playground::{Cue, Scenario, Shot, Step, creature};
    use std::f32::consts::{FRAC_PI_2, PI};
    const CASTER: Vec3 = Vec3::new(16., 0., -4.);
    const DUMMY_A: Vec3 = Vec3::new(7.35, 0., -12.);
    const DUMMY_B: Vec3 = Vec3::new(13., 0., 1.5);
    const CRATE: Vec3 = Vec3::new(16., 0.3, -1.5);
    const ANVIL: Vec3 = Vec3::new(12.5, 0.2, -2.);
    /// Yaw that faces `to` from the caster.
    fn toward(to: Vec3) -> f32 {
        let d = to - CASTER;
        (-d.x).atan2(-d.z)
    }
    Scenario {
        key: "levitate",
        title: NAME,
        srd: "Level 2 Transmutation | Range 60 ft | Concentration, 10 min | CON save if unwilling | rise 20 ft, 500 lb max",
        seed: 454,
        live: 21.5,
        replay: (1.6, 2.4),
        setup: |scene, _| {
            scene.actors[0].position = CASTER;
            scene.actors.push(creature(
                101,
                "Dummy A (willing)",
                "dummy",
                DUMMY_A,
                -FRAC_PI_2,
                100,
            ));
            scene.actors.push(creature(
                102,
                "Dummy B (unwilling)",
                "dummy",
                DUMMY_B,
                -FRAC_PI_2,
                100,
            ));
            Ok(())
        },
        populate: |game, _| {
            use super::{PropKind, PropSpec};
            game.spawn_prop("Crate", PropSpec::reference(PropKind::Crate), CRATE, 0.)?;
            let mut anvil = PropSpec::reference(PropKind::Anvil);
            anvil.mass = 300.;
            game.spawn_prop("Anvil + load", anvil, ANVIL, 0.)?;
            let levitations = &mut game.spells.levitations;
            levitations.willing.insert(101);
            // Dummy A pushes away from the east pillar's east face.
            levitations.intents.push(Intent {
                actor: 101,
                from: 1.8,
                until: 2.0,
                input: DVec3::X,
            });
            game.spells.dice.force_save(102, 18)?;
            Ok(())
        },
        script: || {
            let cue = |at, step| Cue { at, step };
            vec![
                cue(0.3, Step::Face(toward(DUMMY_A))),
                cue(0.5, Step::Cast(Ability::Spell(SLOT))),
                cue(8.7, Step::Face(PI)),
                // A new target ends the dummy's spell: it floats down.
                cue(9.0, Step::Cast(Ability::Spell(SLOT))),
                // Recasting at the crate changes its altitude: down 20 ft,
                // then up 20 ft on the next turn.
                cue(12.0, Step::Cast(Ability::Spell(SLOT))),
                cue(13.3, Step::Face(toward(ANVIL))),
                cue(13.7, Step::Cast(Ability::Spell(SLOT))),
                cue(15.0, Step::Face(toward(DUMMY_B))),
                cue(15.4, Step::Cast(Ability::Spell(SLOT))),
                cue(17.7, Step::Face(PI)),
                cue(18.1, Step::Cast(Ability::Spell(SLOT))),
                cue(18.9, Step::Cast(Ability::Thunderwave)),
            ]
        },
        camera: || {
            let hall = (Vec3::new(20., 7., 0.), Vec3::new(14., 3., -12.));
            let caster = (Vec3::new(21., 4., -10.), Vec3::new(14.5, 1.5, 1.));
            [(0., hall), (11.0, hall), (12.0, caster), (21.5, caster)]
                .into_iter()
                .map(|(at, (eye, target))| Shot { at, eye, target })
                .collect()
        },
        replay_camera: (Vec3::new(11., 3., -16.), Vec3::new(7.6, 1.6, -12.)),
        check: |game| {
            let logged = |needle: &str| {
                game.spells
                    .log
                    .iter()
                    .any(|r| r.spell == NAME && r.text.contains(needle))
            };
            for needle in [
                "Dummy A (willing): rises up to 20 ft",
                "Dummy A (willing): pushes off",
                "Dummy A (willing): bounces",
                "Dummy A (willing): concentration ends",
                "Dummy A (willing): lands gently",
                "Crate: altitude -20 ft",
                "Crate: altitude +20 ft",
                "Anvil + load: refused, too heavy",
                "Dummy B (unwilling): CON save 18",
            ] {
                if !logged(needle) {
                    return Err(format!("The spell log has no \"{needle}\""));
                }
            }
            if game.spells.log.iter().any(|r| r.spell == "Falling") {
                return Err("A levitated target took falling damage".into());
            }
            let crate_moved = game
                .spells
                .tracks
                .iter()
                .find(|t| t.label == "Crate")
                .and_then(|t| crate::playground::measure(game, t))
                .map_or(0., |(_, d)| d);
            if crate_moved < 10. * FEET {
                return Err(format!("The crate drifted only {crate_moved:.2} m"));
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
mod tests;
