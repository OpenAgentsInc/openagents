//! Feather Fall on physics.
//!
//! SRD 5.2.1: level 1 Transmutation, casting time Reaction (taken when you
//! or a creature you can see within 60 feet of you falls), range 60 feet,
//! components V and M, duration 1 minute. Up to five falling creatures in
//! range descend at 60 feet per round until the spell ends. A creature that
//! lands before then takes no damage from the fall, and the spell ends for
//! it.
//!
//! The spell does not remove gravity. After gravity, a vertical drag pulls a
//! warded creature's downward speed back to 60 feet per round (3.048 m/s)
//! and reports the energy and momentum it removes. The drag closes the
//! excess over [`SETTLE`] seconds, so the cap holds within [`CAP_WITHIN`] of
//! the cast without an instant snap. Horizontal velocity is never touched, so
//! a creature shoved sideways off a ledge glides far while it descends
//! slowly, and a Thunderwave or a gust mid-air still pushes it in full.
//!
//! In the chamber, the drag is the character's gravity override: each tick
//! sets the terminal descent speed the character's own step clamps to.
use super::FEET;
use crate::play::{Ability, Game};
use glam::{DVec3, Vec3};
use physics::character::GravityOverride;
use serde::{Deserialize, Serialize};

pub const NAME: &str = "Feather Fall";
/// Row-two action-bar slot (Shift+4).
pub const SLOT: u8 = 3;
/// One round, s.
const ROUND: f64 = super::ROUND as f64;
/// Range: 60 feet from the caster.
pub const RANGE: f64 = 60. * FEET;
/// Up to five falling creatures.
pub const MAX_TARGETS: usize = 5;
/// The slowed rate of descent: 60 feet per round, m/s.
pub const DESCENT_CAP: f64 = 60. * FEET / ROUND;
/// Duration: 1 minute, s.
pub const DURATION: f64 = 60.;
/// A creature is falling when it is unsupported and descends faster than
/// this, m/s.
pub const FALLING_SPEED: f64 = 1.;
/// The descent speed reaches the cap within this time of the cast, s.
pub const CAP_WITHIN: f64 = 0.2;
/// Design time over which the drag closes the excess speed, s. It is shorter
/// than [`CAP_WITHIN`] so a fixed step that does not divide it evenly still
/// reaches the cap in time.
pub const SETTLE: f64 = 0.15;
/// Fastest descent a character controller allows, m/s.
const TERMINAL: f64 = physics::character::TERMINAL_SPEED;
/// Chamber mana: MMO tuning, not tabletop rules.
pub const COST: i32 = 2;
/// One reaction per round.
pub const COOLDOWN: f32 = super::ROUND;
/// An agent reacts for itself once it has dropped this far below its arc's
/// peak, m: deeper than any jump, and the rest of the fall still hurts.
pub const AGENT_REACTION_DROP: f64 = 2.;
/// Most creatures the overlay follows.
const MAX_WATCH: usize = 16;

/// Whether a candidate is a creature. The SRD names creatures, so objects
/// are never eligible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Creature,
    Object,
}

/// Something the caster can see at the moment of casting.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: u64,
    pub kind: Kind,
    /// Center, m.
    pub position: DVec3,
    /// Velocity, m/s.
    pub velocity: DVec3,
    /// Standing on something.
    pub supported: bool,
}

impl Candidate {
    /// Unsupported and descending faster than [`FALLING_SPEED`].
    #[must_use]
    pub fn falling(&self) -> bool {
        !self.supported && self.velocity.y < -FALLING_SPEED
    }

    /// A falling creature within [`RANGE`] of `caster`.
    #[must_use]
    pub fn eligible(&self, caster: DVec3) -> bool {
        self.kind == Kind::Creature
            && self.falling()
            && self.position.distance_squared(caster) <= RANGE * RANGE
    }
}

/// Why a cast was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    /// No target was named.
    NoTarget,
    /// More than [`MAX_TARGETS`] targets.
    TooMany,
    /// A target is named twice.
    Duplicate(u64),
    /// A target is not among the candidates.
    Unknown(u64),
    /// A target is an object.
    NotCreature(u64),
    /// A target is not falling.
    NotFalling(u64),
    /// A target is farther than [`RANGE`].
    OutOfRange(u64),
}

/// Validates the caster's chosen targets at the moment of casting.
pub fn choose(caster: DVec3, candidates: &[Candidate], chosen: &[u64]) -> Result<(), Refusal> {
    if chosen.is_empty() {
        return Err(Refusal::NoTarget);
    }
    if chosen.len() > MAX_TARGETS {
        return Err(Refusal::TooMany);
    }
    for (i, id) in chosen.iter().enumerate() {
        if chosen[..i].contains(id) {
            return Err(Refusal::Duplicate(*id));
        }
        let candidate = candidates
            .iter()
            .find(|c| c.id == *id)
            .ok_or(Refusal::Unknown(*id))?;
        if candidate.kind != Kind::Creature {
            return Err(Refusal::NotCreature(*id));
        }
        if !candidate.falling() {
            return Err(Refusal::NotFalling(*id));
        }
        if candidate.position.distance_squared(caster) > RANGE * RANGE {
            return Err(Refusal::OutOfRange(*id));
        }
    }
    Ok(())
}

/// The controller's choice for an agent or NPC caster: the eligible
/// creatures nearest the caster, ties broken by ID, at most [`MAX_TARGETS`].
/// `None` when nobody is eligible, so the reaction stays available.
#[must_use]
pub fn decide(caster: DVec3, candidates: &[Candidate]) -> Option<Vec<u64>> {
    let mut eligible: Vec<&Candidate> = candidates.iter().filter(|c| c.eligible(caster)).collect();
    eligible.sort_by(|a, b| {
        a.position
            .distance_squared(caster)
            .total_cmp(&b.position.distance_squared(caster))
            .then(a.id.cmp(&b.id))
    });
    let chosen: Vec<u64> = eligible.iter().take(MAX_TARGETS).map(|c| c.id).collect();
    (!chosen.is_empty()).then_some(chosen)
}

/// The spell on one creature.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ward {
    pub target: u64,
    /// Rate at which the drag closes the excess descent speed, m/s².
    pub closing: f64,
    /// Kinetic energy the drag has removed, J.
    pub energy: f64,
    /// Upward impulse the drag has applied, N s.
    pub impulse: f64,
    /// Vertical speed at the last update, m/s.
    #[serde(default)]
    pub vertical_speed: f64,
    /// Time the descent first held at the cap, s.
    #[serde(default)]
    pub capped_at: Option<f64>,
}

/// The drag applied to one creature over one interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    /// Vertical speed after the drag, m/s.
    pub vertical_speed: f64,
    /// Kinetic energy removed, J.
    pub energy: f64,
    /// Upward impulse applied, N s.
    pub impulse: f64,
}

/// What a warded creature's landing does.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Landing {
    pub target: u64,
    /// Falling damage dealt: always zero, since the spell cancels it.
    pub damage: i32,
    /// Kinetic energy the drag removed over the whole descent, J.
    pub energy: f64,
}

/// One cast of Feather Fall and the creatures it still holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeatherFall {
    pub caster: u64,
    /// Simulation time of the cast, s.
    pub cast_at: f64,
    pub wards: Vec<Ward>,
}

impl FeatherFall {
    /// Casts on `chosen` after [`choose`] admits them.
    pub fn cast(
        caster_id: u64,
        caster: DVec3,
        candidates: &[Candidate],
        chosen: &[u64],
        time: f64,
    ) -> Result<Self, Refusal> {
        choose(caster, candidates, chosen)?;
        Ok(Self {
            caster: caster_id,
            cast_at: time,
            wards: chosen
                .iter()
                .map(|id| {
                    let velocity = candidates
                        .iter()
                        .find(|c| c.id == *id)
                        .map_or(DVec3::ZERO, |c| c.velocity);
                    Ward {
                        target: *id,
                        closing: excess(velocity.y) / SETTLE,
                        energy: 0.,
                        impulse: 0.,
                        vertical_speed: velocity.y,
                        capped_at: None,
                    }
                })
                .collect(),
        })
    }

    /// Simulation time the spell ends, s.
    #[must_use]
    pub fn ends_at(&self) -> f64 {
        self.cast_at + DURATION
    }

    /// Whether `target` is still warded at `time`.
    #[must_use]
    pub fn holds(&self, target: u64, time: f64) -> bool {
        time < self.ends_at() && self.wards.iter().any(|w| w.target == target)
    }

    /// Whether the spell holds nobody, or its minute has run out.
    #[must_use]
    pub fn ended(&self, time: f64) -> bool {
        time >= self.ends_at() || self.wards.is_empty()
    }

    /// Drops every ward once the minute has run out, returning their targets.
    pub fn expire(&mut self, time: f64) -> Vec<u64> {
        if time >= self.ends_at() {
            self.wards.drain(..).map(|w| w.target).collect()
        } else {
            vec![]
        }
    }

    /// The terminal descent speed that holds `target` over the next `dt`,
    /// given its vertical speed `before` now. `None` when it is not warded.
    pub fn terminal(&mut self, target: u64, before: f64, dt: f64, time: f64) -> Option<f64> {
        if time >= self.ends_at() {
            return None;
        }
        let ward = self.wards.iter_mut().find(|w| w.target == target)?;
        let excess_before = excess(before);
        // A new downward shove mid-air widens the excess; the drag still
        // closes it within one settle time.
        ward.closing = ward.closing.max(excess_before / SETTLE);
        let allowed = (excess_before - ward.closing * dt).max(0.);
        if excess_before <= 1e-9 && ward.capped_at.is_none() {
            ward.capped_at = Some(time);
        }
        Some((DESCENT_CAP + allowed).min(TERMINAL))
    }

    /// Applies one interval of drag to `target`. `before` is the vertical
    /// speed at the start of the interval and `after` the speed once its
    /// gravity is applied. `None` when the target is not warded.
    pub fn drag(
        &mut self,
        target: u64,
        mass: f64,
        before: f64,
        after: f64,
        dt: f64,
        time: f64,
    ) -> Option<Drag> {
        let terminal = self.terminal(target, before, dt, time)?;
        let vertical_speed = after.max(-terminal);
        Some(self.account(target, mass, after, vertical_speed))
    }

    /// Records the drag that turned a free-fall speed `free` into `held`.
    pub fn account(&mut self, target: u64, mass: f64, free: f64, held: f64) -> Drag {
        let energy = 0.5 * mass * (free * free - held * held).max(0.);
        let impulse = mass * (held - free).max(0.);
        if let Some(ward) = self.wards.iter_mut().find(|w| w.target == target) {
            ward.energy += energy;
            ward.impulse += impulse;
            ward.vertical_speed = held;
        }
        Drag {
            vertical_speed: held,
            energy,
            impulse,
        }
    }

    /// Ends the spell for a warded creature that lands. The landing deals no
    /// falling damage. `None` when the target is not warded, so the caller
    /// applies normal falling damage.
    pub fn land(&mut self, target: u64, time: f64) -> Option<Landing> {
        if time >= self.ends_at() {
            return None;
        }
        let index = self.wards.iter().position(|w| w.target == target)?;
        let ward = self.wards.remove(index);
        Some(Landing {
            target,
            damage: 0,
            energy: ward.energy,
        })
    }

    /// Kinetic energy the drag has removed from every current target, J.
    #[must_use]
    pub fn energy(&self) -> f64 {
        self.wards.iter().map(|w| w.energy).sum()
    }

    /// Checks a restored checkpoint.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = Vec::with_capacity(self.wards.len());
        if !self.cast_at.is_finite()
            || self.cast_at < 0.
            || self.wards.len() > MAX_TARGETS
            || self.wards.iter().any(|w| {
                let duplicate = seen.contains(&w.target);
                seen.push(w.target);
                duplicate
                    || !w.closing.is_finite()
                    || w.closing < 0.
                    || !w.energy.is_finite()
                    || w.energy < 0.
                    || !w.impulse.is_finite()
                    || !w.vertical_speed.is_finite()
                    || w.capped_at.is_some_and(|t| !t.is_finite())
            })
        {
            return Err("Invalid Feather Fall checkpoint".into());
        }
        Ok(())
    }
}

/// How much faster than the cap a vertical speed descends, m/s.
fn excess(vertical_speed: f64) -> f64 {
    (-DESCENT_CAP - vertical_speed).max(0.)
}

/// A falling creature the overlay follows from the cast to its landing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Watch {
    pub actor: u64,
    pub label: String,
    pub warded: bool,
    /// Height fallen and the dice it cost, once it lands.
    pub landed: Option<(f64, u32)>,
    /// Descent speed at the moment of landing, m/s.
    pub impact: f64,
}

/// Feather Fall's part of the spell world, saved with every checkpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub casts: Vec<FeatherFall>,
    /// Creatures the caster has picked in the reaction prompt. When any of
    /// them is eligible, the cast takes only those.
    pub marks: Vec<u64>,
    /// Gravity overrides the spell replaced, restored when a ward ends.
    pub restore: Vec<(u64, Option<GravityOverride>)>,
    pub watch: Vec<Watch>,
}

impl State {
    pub fn validate(&self) -> Result<(), String> {
        if self.casts.len() > 16
            || self.marks.len() > MAX_TARGETS * 4
            || self.restore.len() > MAX_TARGETS * 16
            || self.watch.len() > MAX_WATCH
            || self.watch.iter().any(|w| {
                w.label.len() > 64
                    || !w.impact.is_finite()
                    || w.landed.is_some_and(|(h, _)| !h.is_finite() || h < 0.)
            })
        {
            return Err("Invalid Feather Fall checkpoint".into());
        }
        self.casts.iter().try_for_each(FeatherFall::validate)
    }

    /// Whether any cast holds `target` at `time`.
    pub fn holds(&self, target: u64, time: f64) -> bool {
        self.casts.iter().any(|c| c.holds(target, time))
    }
}

/// The catalog line for the action bar.
pub const SPELL: super::SpellDef = super::SpellDef {
    slot: SLOT,
    key: "feather-fall",
    label: NAME,
    icon: "feather-fall-icon",
    description: "Reaction: up to five falling creatures descend at 60 ft per round",
    cost: COST,
    cooldown: COOLDOWN,
    cast,
};

/// The movement controller of a scene actor, mutably.
fn character_mut(game: &mut Game, actor: u64) -> Option<&mut physics::character::Character> {
    if actor == game.player_actor() {
        Some(&mut game.character)
    } else {
        game.npc_characters.get_mut(&actor)
    }
}

fn mass(game: &Game, actor: u64) -> f64 {
    let model = game
        .scene
        .actors
        .iter()
        .find(|a| a.id == actor)
        .map_or("", |a| a.model.as_str());
    super::model_size(model).creature_mass()
}

/// Every creature and prop the caster can see, as the SRD reaction sees
/// them.
fn candidates(game: &Game) -> Vec<Candidate> {
    let eye = game.player + Vec3::Y * 1.4;
    let snapshot = game.snapshot();
    let player = game.player_actor();
    let mut out = vec![];
    let mut creature = |actor: u64, feet: Vec3| {
        let (velocity, supported) = match game.actor_character(actor) {
            Some(c) => (
                DVec3::new(c.external.x, c.vertical_speed, c.external.z),
                !c.airborne(),
            ),
            None => (DVec3::ZERO, true),
        };
        let center = feet.as_dvec3() + DVec3::Y * 0.9;
        if actor == player || game.attack_clear(eye, center.as_vec3()) {
            out.push(Candidate {
                id: actor,
                kind: Kind::Creature,
                position: center,
                velocity,
                supported,
            });
        }
    };
    if snapshot.player.hp > 0 {
        creature(player, game.player);
    }
    for (actor, id) in &game.ids {
        if let Some(a) = snapshot.actors.iter().find(|a| a.id == *id && a.alive) {
            creature(*actor, Vec3::from(a.pos));
        }
    }
    for (i, prop) in game.spells.props.iter().enumerate() {
        if prop.removed {
            continue;
        }
        let body = &game.spells.world[prop.body];
        out.push(Candidate {
            id: super::PROP_ENTITY_BASE + i as u64,
            kind: Kind::Object,
            position: game.spells.prop_center(i),
            velocity: body.vel,
            supported: body.sleeping,
        });
    }
    out
}

/// Which creatures the caster catches: those picked in the reaction prompt
/// when any is eligible; otherwise the caster and the selected target when
/// they are falling; otherwise the nearest eligible creatures.
fn targets(game: &Game, caster: DVec3, candidates: &[Candidate]) -> Option<Vec<u64>> {
    let eligible = |id: &u64| candidates.iter().any(|c| c.id == *id && c.eligible(caster));
    let pick = |ids: Vec<u64>| {
        let mut ids: Vec<u64> = ids.into_iter().filter(|id| eligible(id)).collect();
        ids.dedup();
        ids.truncate(MAX_TARGETS);
        (!ids.is_empty()).then_some(ids)
    };
    pick(game.spells.feather_fall.marks.clone())
        .or_else(|| pick(vec![game.player_actor(), game.selected]))
        .or_else(|| decide(caster, candidates))
}

/// Runs an admitted cast from the action bar.
pub fn cast(game: &mut Game) -> Result<(), String> {
    let caster = game.player.as_dvec3() + DVec3::Y * 0.9;
    let candidates = candidates(game);
    let chosen =
        targets(game, caster, &candidates).ok_or("No falling creature within 60 feet to catch")?;
    let time = game.time as f64;
    let spell = FeatherFall::cast(game.player_actor(), caster, &candidates, &chosen, time)
        .map_err(|refusal| format!("Feather Fall refused: {refusal:?}"))?;
    game.spells.begin_cast(game.player_actor(), false)?;
    let mut names = vec![];
    let mut state = std::mem::take(&mut game.spells.feather_fall);
    // Follow every falling creature in range, caught or not, so the overlay
    // can compare them.
    for c in candidates.iter().filter(|c| c.eligible(caster)) {
        let label = game.actor_name(c.id);
        state.watch.retain(|w| w.actor != c.id);
        if state.watch.len() >= MAX_WATCH {
            state.watch.remove(0);
        }
        state.watch.push(Watch {
            actor: c.id,
            label: label.clone(),
            warded: chosen.contains(&c.id),
            landed: None,
            impact: 0.,
        });
        if chosen.contains(&c.id) {
            names.push(label);
        }
    }
    let mut spell = spell;
    for target in &chosen {
        let before = candidates
            .iter()
            .find(|c| c.id == *target)
            .map_or(0., |c| c.velocity.y);
        let terminal = spell.terminal(*target, before, 0., time);
        if let (Some(terminal), Some(character)) = (terminal, character_mut(game, *target)) {
            if !state.restore.iter().any(|(id, _)| id == target) {
                state.restore.push((*target, character.gravity));
            }
            let scale = character.gravity.map_or(1., |g| g.scale);
            character.gravity = Some(GravityOverride { scale, terminal });
        }
    }
    if state.casts.len() >= 16 {
        state.casts.remove(0);
    }
    state.casts.push(spell);
    state.marks.clear();
    game.spells.feather_fall = state;
    game.spells.record(
        game.time,
        NAME,
        format!(
            "Reaction: {} descend at 60 ft/round ({} of {} max)",
            names.join(", "),
            chosen.len(),
            MAX_TARGETS
        ),
        None,
    );
    Ok(())
}

/// Puts back the gravity a ward replaced.
fn release(game: &mut Game, state: &mut State, target: u64) {
    let Some(index) = state.restore.iter().position(|(id, _)| *id == target) else {
        return;
    };
    let (_, gravity) = state.restore.remove(index);
    if state
        .casts
        .iter()
        .any(|c| c.wards.iter().any(|w| w.target == target))
    {
        // Another cast still holds it.
        state.restore.push((target, gravity));
        return;
    }
    if let Some(character) = character_mut(game, target) {
        character.gravity = gravity;
    }
}

/// Called for every landing before falling damage. Returns true when
/// Feather Fall cancels the damage; that landing also ends the spell for the
/// creature.
pub(crate) fn cushion(game: &mut Game, actor: Option<u64>, height: f64) -> Result<bool, String> {
    if height <= 0. {
        return Ok(false);
    }
    let target = actor.unwrap_or_else(|| game.player_actor());
    let time = game.time as f64;
    let mut state = std::mem::take(&mut game.spells.feather_fall);
    let impact = state
        .casts
        .iter()
        .flat_map(|c| c.wards.iter())
        .find(|w| w.target == target)
        .map(|w| -w.vertical_speed);
    let landing = state.casts.iter_mut().find_map(|c| c.land(target, time));
    if let Some(watch) = state.watch.iter_mut().find(|w| w.actor == target) {
        if watch.landed.is_none() {
            watch.landed = Some((height, super::fall_dice(height)));
            watch.impact = impact.unwrap_or_else(|| {
                // Free fall from the arc's peak under the controller's gravity.
                (2. * physics::character::Settings::default().gravity * height).sqrt()
            });
        }
    }
    if landing.is_some() {
        release(game, &mut state, target);
    }
    state.casts.retain(|c| !c.ended(time));
    game.spells.feather_fall = state;
    let Some(landing) = landing else {
        return Ok(false);
    };
    let name = game.actor_name(target);
    game.spells.record(
        game.time,
        NAME,
        format!(
            "{name} lands softly after {:.0} ft: {} damage (drag removed {:.1} kJ)",
            height / FEET,
            landing.damage,
            landing.energy / 1000.
        ),
        None,
    );
    Ok(true)
}

/// Advances every ward after the characters have stepped `dt` seconds: it
/// accounts the drag, sets the next interval's terminal speed, ends wards
/// whose minute has run out, and offers the reaction to a human player.
pub(crate) fn step(game: &mut Game, dt: f64) -> Result<(), String> {
    let time = game.time as f64;
    let mut state = std::mem::take(&mut game.spells.feather_fall);
    let gravity = physics::character::Settings::default().gravity;
    let mut ended = vec![];
    let mut landed = vec![];
    for i in 0..state.casts.len() {
        ended.extend(state.casts[i].expire(time));
        let targets: Vec<u64> = state.casts[i].wards.iter().map(|w| w.target).collect();
        for target in targets {
            let weight = mass(game, target);
            let Some(character) = character_mut(game, target) else {
                landed.extend(state.casts[i].land(target, time).map(|l| l.target));
                continue;
            };
            let now = character.vertical_speed;
            let airborne = character.airborne();
            let cast = &mut state.casts[i];
            if !airborne {
                // A landing too short to measure still ends the spell for it.
                landed.extend(cast.land(target, time).map(|l| l.target));
                continue;
            }
            let last = cast
                .wards
                .iter()
                .find(|w| w.target == target)
                .map_or(now, |w| w.vertical_speed);
            let free = (last - gravity * dt).max(-TERMINAL);
            cast.account(target, weight, free, now);
            if let Some(terminal) = cast.terminal(target, now, dt, time) {
                if let Some(character) = character_mut(game, target) {
                    let scale = character.gravity.map_or(1., |g| g.scale);
                    character.gravity = Some(GravityOverride { scale, terminal });
                }
            }
        }
    }
    for (target, why) in ended
        .into_iter()
        .map(|t| (t, "the minute runs out"))
        .chain(landed.into_iter().map(|t| (t, "lands: 0 damage")))
    {
        release(game, &mut state, target);
        let name = game.actor_name(target);
        game.spells
            .record(game.time, NAME, format!("{name} {why}"), None);
    }
    state.casts.retain(|c| !c.ended(time));
    game.spells.feather_fall = state;
    let someone_falls = std::iter::once(&game.character)
        .chain(game.npc_characters.values())
        .any(|c| c.airborne() && c.vertical_speed < -FALLING_SPEED);
    if someone_falls && !game.agent_controlled && ready(game) {
        let caster = game.player.as_dvec3() + DVec3::Y * 0.9;
        if candidates(game)
            .iter()
            .any(|c| c.eligible(caster) && !game.spells.feather_fall.holds(c.id, time))
        {
            game.message = "Reaction: Feather Fall (Shift+4) catches falling creatures".into();
        }
    }
    Ok(())
}

/// Whether the adventurer's reaction is available: the slot is off its
/// cooldown, there is mana for it, and the adventurer is alive.
fn ready(game: &Game) -> bool {
    let player = game.snapshot().player;
    player.hp > 0
        && player.mana >= COST
        && game
            .spells
            .ready
            .get(&SLOT)
            .is_none_or(|at| *at <= game.time)
}

/// The agent's reaction: when the adventurer itself has dropped
/// [`AGENT_REACTION_DROP`] below its arc's peak and is still falling, it
/// casts Feather Fall. Returns true when it cast.
pub(crate) fn react(game: &mut Game) -> Result<bool, String> {
    if !ready(game) || game.casting.is_some() {
        return Ok(false);
    }
    let c = game.character;
    let dropped = c.peak.map_or(0., |peak| peak - c.feet.y);
    if !c.airborne()
        || c.vertical_speed >= -FALLING_SPEED
        || dropped < AGENT_REACTION_DROP
        || game
            .spells
            .feather_fall
            .holds(game.player_actor(), game.time as f64)
    {
        return Ok(false);
    }
    Ok(game.activate(Ability::Spell(SLOT)).is_ok())
}

/// Overlay lines: each followed creature's descent speed, then how it
/// landed.
pub fn overlay(game: &Game) -> Vec<String> {
    let mut lines = vec![];
    for w in &game.spells.feather_fall.watch {
        let tag = if w.warded {
            "feather fall"
        } else {
            "free fall"
        };
        let line = match w.landed {
            Some((height, dice)) => format!(
                "{:<10} landed at {:4.1} m/s after {:.0} ft: {}",
                w.label,
                w.impact,
                height / FEET,
                if w.warded {
                    "0 damage".to_string()
                } else {
                    format!("{dice}d6")
                }
            ),
            None => {
                let speed = game
                    .actor_character(w.actor)
                    .map_or(0., |c| -c.vertical_speed.min(0.));
                format!(
                    "{:<10} descent {:4.1} m/s ({:3.0} ft/round) {tag}",
                    w.label,
                    speed,
                    speed / FEET * ROUND
                )
            }
        };
        lines.push(line);
    }
    lines
}

/// The playground recording: five dummies on a 60-foot tower, a Thunderwave
/// that knocks all five off, and the wizard's reaction catching three. Two
/// land unharmed, the two left falling take 6d6, and the third caught dummy,
/// shoved hardest, glides past the others.
pub fn scenario() -> crate::playground::Scenario {
    use crate::playground::{Cue, Scenario, Shot, Step, creature};
    use std::f32::consts::FRAC_PI_2;
    Scenario {
        key: "feather-fall",
        title: NAME,
        srd: "Level 1 Transmutation | Reaction | Range 60 ft | up to 5 falling creatures | 1 minute",
        seed: 455,
        live: 12.,
        replay: (1.7, 3.2),
        setup: |scene, _| {
            // The wizard stands on the tower's top, behind the dummies.
            scene.actors[0].position = CASTER;
            scene.actors[0].yaw = -FRAC_PI_2;
            for (id, name, place) in DUMMIES {
                scene
                    .actors
                    .push(creature(id, name, "dummy", place, FRAC_PI_2, 100));
            }
            Ok(())
        },
        populate: |game, _| {
            let spec = super::PropSpec {
                dimensions: DVec3::new(8., TOWER_HEIGHT as f64, 10.),
                ..super::PropSpec::reference(super::PropKind::StoneBlock).secured()
            };
            game.spawn_prop(
                "Tower",
                spec,
                Vec3::new(TOWER_X, TOWER_HEIGHT * 0.5, TOWER_Z),
                0.,
            )?;
            // Every dummy fails its save against the wave.
            for (id, _, _) in DUMMIES {
                game.spells.dice.force_save(id, 1)?;
            }
            // The reaction prompt: the wizard picks three of the five.
            game.spells.feather_fall.marks = vec![102, 103, 104];
            Ok(())
        },
        script: || {
            vec![
                Cue {
                    at: 0.3,
                    step: Step::Face(-FRAC_PI_2),
                },
                Cue {
                    at: 1.5,
                    step: Step::Cast(Ability::Thunderwave),
                },
                Cue {
                    at: 2.3,
                    step: Step::Cast(Ability::Spell(SLOT)),
                },
            ]
        },
        camera: || {
            // Close on the tower's top from the south-east, then wide from
            // the south so the falls read against the floor.
            let top = (Vec3::new(-6.5, 21.5, -7.), Vec3::new(-13.5, 17.2, 4.));
            // The eye sits between the pillars' sight lines to the tower.
            let wide = (Vec3::new(3., 9.5, -20.5), Vec3::new(-1., 7.5, 4.));
            [(0., top), (2.6, top), (4.2, wide), (12., wide)]
                .into_iter()
                .map(|(at, (eye, target))| Shot { at, eye, target })
                .collect()
        },
        replay_camera: (Vec3::new(-6.5, 21.5, -7.), Vec3::new(-13.5, 17.2, 4.)),
        check: |game| {
            let state = &game.spells.feather_fall;
            let cast = game
                .spells
                .log
                .iter()
                .find(|r| r.spell == NAME && r.text.starts_with("Reaction"))
                .ok_or("Feather Fall was never cast")?;
            for id in [102, 103, 104] {
                let name = game.actor_name(id);
                if !cast.text.contains(&name) {
                    return Err(format!("{name} was not caught: {}", cast.text));
                }
                let watch = state
                    .watch
                    .iter()
                    .find(|w| w.actor == id)
                    .ok_or(format!("{name} was not followed"))?;
                if !watch.warded || watch.landed.is_none() || watch.impact > DESCENT_CAP + 0.01 {
                    return Err(format!("{name} did not land slowly: {watch:?}"));
                }
                if !game
                    .spells
                    .log
                    .iter()
                    .any(|r| r.spell == NAME && r.text.starts_with(&format!("{name} lands softly")))
                {
                    return Err(format!("{name}'s landing was not cushioned"));
                }
                if game
                    .spells
                    .log
                    .iter()
                    .any(|r| r.spell == "Falling" && r.text.starts_with(&name))
                {
                    return Err(format!("{name} took falling damage"));
                }
            }
            for id in [101, 105] {
                let name = game.actor_name(id);
                let fall = game
                    .spells
                    .log
                    .iter()
                    .find(|r| r.spell == "Falling" && r.text.starts_with(&name))
                    .ok_or(format!("{name} took no falling damage"))?;
                if !fall.text.contains("6d6") {
                    return Err(format!("{name}: {}", fall.text));
                }
            }
            let reach = |id: u64| {
                game.actor_position(id)
                    .map(|p| p.x - TOWER_EDGE)
                    .ok_or(format!("{} is gone", game.actor_name(id)))
            };
            let glider = reach(103)?;
            for id in [101, 102, 104, 105] {
                if glider < reach(id)? + 5. {
                    return Err(format!(
                        "The glider reached {glider:.1} m, {} reached {:.1} m",
                        game.actor_name(id),
                        reach(id)?
                    ));
                }
            }
            if state.holds(103, game.time as f64) || !state.casts.is_empty() {
                return Err("Feather Fall outlived its last landing".into());
            }
            Ok(())
        },
    }
}

/// The scenario's tower: 60 feet tall, a hair over so the contact skin on
/// the floor and the tower top still measures a full 60-foot fall.
const TOWER_HEIGHT: f32 = 18.31;
const TOWER_X: f32 = -16.;
const TOWER_Z: f32 = 4.;
/// The tower's east face, which the dummies go over.
const TOWER_EDGE: f32 = -12.;
const CASTER: Vec3 = Vec3::new(-18.1, TOWER_HEIGHT, 4.);
/// Five dummies ahead of the wizard, inside the Thunderwave Cube. Each
/// stands where the 10-foot push carries it over the edge: the outer four at
/// about 2.5 m/s, the middle one, 0.4 m nearer the edge, at about 4.5 m/s.
const DUMMIES: [(u64, &str, Vec3); 5] = [
    (101, "Dummy 1", Vec3::new(-14.37, TOWER_HEIGHT, 2.4)),
    (102, "Dummy 2", Vec3::new(-14.53, TOWER_HEIGHT, 3.2)),
    (103, "Dummy 3", Vec3::new(-14.17, TOWER_HEIGHT, 4.)),
    (104, "Dummy 4", Vec3::new(-14.53, TOWER_HEIGHT, 4.8)),
    (105, "Dummy 5", Vec3::new(-14.37, TOWER_HEIGHT, 5.6)),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spells::fall_dice;
    use physics::body::Body;
    use physics::collision::{Collider, Shape};
    use physics::ledger::Ledger;
    use physics::world::{BodyId, Uniform, World};

    const G: f64 = 9.81;
    const DT: f64 = 1. / 120.;
    const TOWER: f64 = 60. * FEET;
    const RADIUS: f64 = 0.4;
    const DUMMY: f64 = 75.;

    fn candidate(id: u64, position: DVec3, velocity: DVec3) -> Candidate {
        Candidate {
            id,
            kind: Kind::Creature,
            position,
            velocity,
            supported: false,
        }
    }

    /// A 60 ft tower beside a floor, five sphere dummies at its edge, and a
    /// caster on the tower.
    struct Scene {
        world: World,
        dummies: Vec<BodyId>,
        caster: DVec3,
    }

    fn scene() -> Scene {
        let mut world = World::new(DT);
        let floor = world.add(
            Body::new(1., DVec3::ONE, DVec3::new(0., -0.5, 0.))
                .with_kind(physics::body::BodyKind::Static),
        );
        world.add_collider(Collider::new(
            floor,
            Shape::Cuboid {
                half: DVec3::new(100., 0.5, 100.),
            },
        ));
        let tower = world.add(
            Body::new(1., DVec3::ONE, DVec3::new(-5., TOWER / 2., 0.))
                .with_kind(physics::body::BodyKind::Static),
        );
        world.add_collider(Collider::new(
            tower,
            Shape::Cuboid {
                half: DVec3::new(5., TOWER / 2., 10.),
            },
        ));
        let dummies = (0..5)
            .map(|i| {
                let id = world.add(Body::new(
                    DUMMY,
                    DVec3::splat(0.4 * DUMMY * RADIUS * RADIUS),
                    DVec3::new(-0.6, TOWER + RADIUS, -6. + 3. * i as f64),
                ));
                world.add_collider(Collider::new(id, Shape::Sphere { radius: RADIUS }));
                id
            })
            .collect();
        Scene {
            world,
            dummies,
            caster: DVec3::new(-3., TOWER + 0.9, 0.),
        }
    }

    /// The fall a creature has made since its highest unsupported point.
    #[derive(Default, Clone, Copy)]
    struct Fall {
        apex: Option<f64>,
        landed: Option<f64>,
        /// Distance from the tower's edge at the landing, m.
        reach: f64,
        /// Time in the air, s.
        airtime: f64,
        left_at: Option<f64>,
    }

    /// Runs the scene: a Thunderwave-like shove sends every dummy off the
    /// edge, the caster reacts once the warded ones are falling, and each
    /// landing is scored.
    fn run(shove: [f64; 5], warded: &[usize]) -> (Vec<Fall>, FeatherFall, Ledger, f64) {
        let mut s = scene();
        for (i, id) in s.dummies.iter().enumerate() {
            s.world[*id].vel = DVec3::new(shove[i], 0., 0.);
        }
        let mut ledger = Ledger::new(DVec3::ZERO, s.world.momentum(DVec3::ZERO));
        let mut falls = vec![Fall::default(); 5];
        let mut spell: Option<FeatherFall> = None;
        let mut cast_at = 0.;
        for _ in 0..(120 * 30) {
            let time = s.world.time();
            let candidates: Vec<Candidate> = s
                .dummies
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    let body = &s.world[*id];
                    Candidate {
                        id: i as u64,
                        kind: Kind::Creature,
                        position: body.pos,
                        velocity: body.vel,
                        supported: body.pos.y - RADIUS < 0.02
                            || (body.pos.x < 0. && body.pos.y - RADIUS > TOWER - 0.02),
                    }
                })
                .collect();
            if spell.is_none() && warded.iter().all(|i| candidates[*i].falling()) {
                let chosen: Vec<u64> = warded.iter().map(|i| *i as u64).collect();
                spell = Some(FeatherFall::cast(0, s.caster, &candidates, &chosen, time).unwrap());
                cast_at = time;
            }
            // The drag acts as a force, so the world's own integration and
            // contacts see it and the ledger can name it.
            for (i, id) in s.dummies.iter().enumerate() {
                let body = s.world[*id];
                if let Some(spell) = spell.as_mut() {
                    let before = body.vel.y;
                    let after = before - G * DT;
                    if let Some(drag) = spell.drag(i as u64, DUMMY, before, after, DT, time) {
                        if drag.impulse != 0. {
                            s.world[*id].apply_force(DVec3::Y * drag.impulse / DT);
                            ledger.add_impulse("feather_fall", DVec3::Y * drag.impulse, body.pos);
                        }
                    }
                }
            }
            s.world.step(&Uniform(DVec3::new(0., -G, 0.)));
            for (i, id) in s.dummies.iter().enumerate() {
                let body = &s.world[*id];
                let height = body.pos.y - RADIUS;
                let fall = &mut falls[i];
                if fall.landed.is_some() {
                    continue;
                }
                if body.pos.x > 0. || height < TOWER - 0.02 {
                    fall.apex = Some(fall.apex.map_or(height, |a: f64| a.max(height)));
                    fall.left_at.get_or_insert(s.world.time());
                }
                if fall.apex.is_some() && height < 0.02 && body.vel.y > -0.5 {
                    // The landing point is the floor's surface; the contact
                    // margin leaves the sphere a few millimetres above it.
                    fall.landed = Some(fall.apex.unwrap());
                    fall.reach = body.pos.x;
                    fall.airtime = s.world.time() - fall.left_at.unwrap();
                    if let Some(spell) = spell.as_mut() {
                        if let Some(landing) = spell.land(i as u64, s.world.time()) {
                            assert_eq!(landing.damage, 0);
                        }
                    }
                }
            }
        }
        (falls, spell.unwrap(), ledger, cast_at)
    }

    #[test]
    fn at_most_five_targets_and_only_falling_creatures_in_range() {
        let caster = DVec3::ZERO;
        let fall = DVec3::new(0., -4., 0.);
        let six: Vec<Candidate> = (0..6)
            .map(|i| candidate(i, DVec3::new(i as f64, 5., 0.), fall))
            .collect();
        assert_eq!(
            choose(caster, &six, &[0, 1, 2, 3, 4, 5]),
            Err(Refusal::TooMany)
        );
        assert_eq!(choose(caster, &six, &[0, 1, 2, 3, 4]), Ok(()));
        assert_eq!(decide(caster, &six).unwrap(), vec![0, 1, 2, 3, 4]);
        assert_eq!(choose(caster, &six, &[]), Err(Refusal::NoTarget));
        assert_eq!(choose(caster, &six, &[1, 1]), Err(Refusal::Duplicate(1)));

        // Range is exactly 60 feet.
        let edge = candidate(7, DVec3::new(RANGE - 1e-6, 0., 0.), fall);
        let beyond = candidate(8, DVec3::new(RANGE + 1e-3, 0., 0.), fall);
        assert!((RANGE - 18.288).abs() < 1e-12);
        assert_eq!(choose(caster, &[edge], &[7]), Ok(()));
        assert_eq!(choose(caster, &[beyond], &[8]), Err(Refusal::OutOfRange(8)));

        // Objects are never eligible, even falling in range.
        let crate_ = Candidate {
            kind: Kind::Object,
            ..candidate(9, DVec3::X, fall)
        };
        assert_eq!(
            choose(caster, &[crate_], &[9]),
            Err(Refusal::NotCreature(9))
        );
        assert_eq!(decide(caster, &[crate_]), None);

        // Standing, rising, or drifting slowly down is not falling.
        let standing = Candidate {
            supported: true,
            ..candidate(10, DVec3::X, fall)
        };
        let slow = candidate(11, DVec3::X, DVec3::new(0., -0.9, 0.));
        assert_eq!(
            choose(caster, &[standing], &[10]),
            Err(Refusal::NotFalling(10))
        );
        assert_eq!(choose(caster, &[slow], &[11]), Err(Refusal::NotFalling(11)));
        assert_eq!(decide(caster, &[standing, slow, beyond]), None);
    }

    #[test]
    fn drag_reaches_the_cap_within_two_tenths_of_a_second_without_a_snap() {
        for start in [-3.5, -12., -40., -55.] {
            let c = candidate(1, DVec3::ZERO, DVec3::new(2., start, 0.));
            let mut spell = FeatherFall::cast(0, DVec3::ZERO, &[c], &[1], 0.).unwrap();
            let mut v = start;
            let mut steps = 0;
            let mut first = None;
            while v < -DESCENT_CAP - 1e-9 {
                let after = v - G * DT;
                let next = spell
                    .drag(1, DUMMY, v, after, DT, steps as f64 * DT)
                    .unwrap()
                    .vertical_speed;
                first.get_or_insert(next);
                v = next;
                steps += 1;
            }
            assert!(steps as f64 * DT <= CAP_WITHIN, "{start}: {steps} steps");
            // Not an instant snap: the first step removes a small fraction.
            if start < -10. {
                assert!(first.unwrap() < -DESCENT_CAP - 1., "{start}");
            }
            // Gravity cannot push it past the cap once there.
            for i in 0..600 {
                let after = v - G * DT;
                v = spell
                    .drag(1, DUMMY, v, after, DT, 1. + i as f64 * DT)
                    .unwrap()
                    .vertical_speed;
                assert!(v >= -DESCENT_CAP - 1e-12);
            }
            assert!(spell.energy() > 0.);
        }
    }

    #[test]
    fn the_chamber_tick_reaches_the_cap_within_two_tenths_of_a_second() {
        // The chamber sets the terminal speed once per 1/30 s tick and the
        // character's own step clamps to it.
        let tick = 1. / 30.;
        let gravity = physics::character::Settings::default().gravity;
        for start in [-4., -15., -55.] {
            let c = candidate(1, DVec3::ZERO, DVec3::new(0., start, 0.));
            let mut spell = FeatherFall::cast(0, DVec3::ZERO, &[c], &[1], 0.).unwrap();
            let mut v: f64 = start;
            let mut time = 0.;
            let mut terminal = spell.terminal(1, v, 0., time).unwrap();
            while v < -DESCENT_CAP - 1e-9 {
                v = (v - gravity * tick).max(-terminal);
                time += tick;
                terminal = spell.terminal(1, v, tick, time).unwrap();
            }
            assert!(time <= CAP_WITHIN + 1e-9, "{start}: {time}");
        }
    }

    #[test]
    fn warded_dummies_land_slowly_unharmed_and_the_others_take_six_d6() {
        let (falls, spell, ledger, cast_at) = run([3., 3., 6., 3., 3.], &[0, 1, 2]);
        for (i, fall) in falls.iter().enumerate() {
            let height = fall.landed.expect("every dummy lands");
            assert!(
                (height - TOWER).abs() < 0.05,
                "dummy {i} fell {height} m, apex {:?}",
                fall.apex
            );
        }
        // The landing ended the spell for each warded dummy.
        assert!(spell.wards.is_empty());
        assert!(spell.ended(cast_at + 1.));
        // The two unwarded dummies fell 60 feet: 6d6.
        assert_eq!(fall_dice(falls[3].landed.unwrap()), 6);
        assert_eq!(fall_dice(falls[4].landed.unwrap()), 6);
        // Falling at 3.048 m/s, 60 feet takes about six seconds; free fall
        // takes two, plus the roll off the edge and the bounce.
        for fall in &falls[..3] {
            assert!(fall.airtime > 5.5, "{}", fall.airtime);
        }
        for fall in &falls[3..] {
            assert!(fall.airtime < 3.5, "{}", fall.airtime);
        }
        // Horizontal motion is untouched, so the warded dummies glide far
        // and the one shoved hardest glides past the others.
        let x = |i: usize| falls[i].reach;
        assert!(x(2) > x(0) + 10., "{} vs {}", x(2), x(0));
        assert!(x(0) > x(3) + 5., "{} vs {}", x(0), x(3));
        assert!(ledger.external["feather_fall"].linear.y > 0.);
    }

    #[test]
    fn descent_is_capped_within_two_tenths_and_horizontal_velocity_is_preserved() {
        let mut s = scene();
        let id = s.dummies[0];
        s.world[id].vel = DVec3::new(5., 0., 1.);
        let mut spell = None;
        let mut checked = false;
        for _ in 0..(120 * 4) {
            let time = s.world.time();
            let body = s.world[id];
            let c = candidate(0, body.pos, body.vel);
            if spell.is_none() && body.pos.x > 0.5 && c.falling() {
                spell = Some((
                    FeatherFall::cast(9, s.caster, &[c], &[0], time).unwrap(),
                    time,
                ));
            }
            if let Some((spell, cast_at)) = spell.as_mut() {
                let horizontal = DVec3::new(body.vel.x, 0., body.vel.z);
                let after = body.vel.y - G * DT;
                let drag = spell.drag(0, DUMMY, body.vel.y, after, DT, time).unwrap();
                s.world[id].apply_force(DVec3::Y * drag.impulse / DT);
                s.world.step(&Uniform(DVec3::new(0., -G, 0.)));
                let now = s.world[id].vel;
                assert!((DVec3::new(now.x, 0., now.z) - horizontal).length() < 1e-9);
                if s.world.time() - *cast_at >= CAP_WITHIN - 1e-9 {
                    assert!(now.y >= -DESCENT_CAP - 1e-9, "{now}");
                    checked = true;
                }
            } else {
                s.world.step(&Uniform(DVec3::new(0., -G, 0.)));
            }
        }
        assert!(checked);
    }

    #[test]
    fn the_minute_runs_out() {
        // A 300 m drop outlasts the spell: the drag stops at 60 s and the
        // body then falls freely again.
        let mut world = World::new(DT);
        let id = world.add(Body::new(DUMMY, DVec3::ONE, DVec3::new(0., 300., 0.)));
        world[id].vel = DVec3::new(0., -5., 0.);
        let c = candidate(0, world[id].pos, world[id].vel);
        let mut spell = FeatherFall::cast(1, DVec3::new(0., 290., 0.), &[c], &[0], 0.).unwrap();
        while world.time() < DURATION + 1. {
            let time = world.time();
            let v = world[id].vel.y;
            if let Some(drag) = spell.drag(0, DUMMY, v, v - G * DT, DT, time) {
                world[id].apply_force(DVec3::Y * drag.impulse / DT);
            }
            world.step(&Uniform(DVec3::new(0., -G, 0.)));
            if time + DT < DURATION - 1e-9 && time > CAP_WITHIN {
                assert!(world[id].vel.y >= -DESCENT_CAP - 1e-9);
            }
        }
        assert!(world[id].pos.y > 300. - 60. * DESCENT_CAP - 10.);
        assert!(world[id].vel.y < -DESCENT_CAP - 5., "{}", world[id].vel.y);
        assert!(!spell.holds(0, DURATION));
        assert!(spell.holds(0, DURATION - 1e-6));
        assert_eq!(spell.land(0, DURATION + 1.), None);
        assert_eq!(spell.expire(DURATION), vec![0]);
        assert!(spell.wards.is_empty());
    }

    #[test]
    fn an_unwarded_creature_lands_with_normal_damage() {
        let c = candidate(1, DVec3::ZERO, DVec3::new(0., -5., 0.));
        let mut spell = FeatherFall::cast(0, DVec3::ZERO, &[c], &[1], 0.).unwrap();
        assert_eq!(spell.land(2, 1.), None);
        let landing = spell.land(1, 1.).unwrap();
        assert_eq!((landing.target, landing.damage), (1, 0));
        // The landing ended it for that creature: a second landing is normal.
        assert_eq!(spell.land(1, 2.), None);
        assert_eq!(fall_dice(10. * FEET), 1);
        assert_eq!(fall_dice(60. * FEET), 6);
    }

    #[test]
    fn checkpoints_round_trip_and_reject_corruption() {
        let c = candidate(1, DVec3::ZERO, DVec3::new(0., -5., 0.));
        let mut spell = FeatherFall::cast(0, DVec3::ZERO, &[c], &[1], 2.).unwrap();
        spell.drag(1, DUMMY, -5., -5.1, DT, 2.);
        let json = serde_json::to_string(&spell).unwrap();
        let restored: FeatherFall = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, spell);
        restored.validate().unwrap();
        let mut bad = spell.clone();
        bad.wards.push(bad.wards[0]);
        assert!(bad.validate().is_err());
    }

    fn settle(game: &mut Game, seconds: f32) {
        for _ in 0..(seconds * 30.) as u32 {
            game.tick(1. / 30., [0.; 2]).unwrap();
        }
    }

    #[test]
    fn the_playground_catches_three_and_two_take_six_d6() {
        let mut run = crate::playground::Run::new(scenario()).unwrap();
        while run.game.time < 2.35 {
            run.advance().unwrap();
        }
        let game = &run.game;
        let cast = game
            .spells
            .log
            .iter()
            .find(|r| r.spell == NAME)
            .expect("the reaction was cast");
        assert!(cast.text.contains("3 of 5"), "{}", cast.text);
        assert_eq!(game.spells.feather_fall.casts[0].wards.len(), 3);
        // Two-tenths of a second later every caught dummy holds at the cap,
        // and every dummy keeps its horizontal shove.
        while run.game.time < 2.35 + CAP_WITHIN as f32 + 0.05 {
            run.advance().unwrap();
        }
        for id in [102, 103, 104] {
            let c = run.game.actor_character(id).unwrap();
            assert!(c.airborne());
            assert!(c.vertical_speed >= -DESCENT_CAP - 1e-9, "{id}: {c:?}");
            assert!(c.external.x > 1., "{id}: {c:?}");
        }
        for id in [101, 105] {
            let c = run.game.actor_character(id).unwrap();
            assert!(c.vertical_speed < -DESCENT_CAP * 2., "{id}: {c:?}");
        }
        let ward = run.game.spells.feather_fall.casts[0].wards[0];
        assert!(
            ward.capped_at.unwrap() - run.game.spells.feather_fall.casts[0].cast_at
                <= CAP_WITHIN + 1e-6
        );
        while !run.done() {
            run.advance().unwrap();
        }
        (run.scenario.check)(run.result()).unwrap();
        assert_eq!(run.replay_identical, Some(true));
    }

    #[test]
    fn checkpoints_replay_identically_mid_fall() {
        let mut run = crate::playground::Run::new(scenario()).unwrap();
        while run.game.time < 3. {
            run.advance().unwrap();
        }
        assert!(!run.game.spells.feather_fall.casts.is_empty());
        let saved = run.game.checkpoint().unwrap();
        let mut restored = Game::restore(&saved).unwrap();
        assert_eq!(restored.spells.feather_fall, run.game.spells.feather_fall);
        for _ in 0..90 {
            run.game.tick(1. / 30., [0.; 2]).unwrap();
            restored.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert_eq!(
            run.game.checkpoint().unwrap(),
            restored.checkpoint().unwrap()
        );
    }

    #[test]
    fn a_cast_with_nobody_falling_is_refused_and_costs_nothing() {
        let mut run = crate::playground::Run::new(scenario()).unwrap();
        settle(&mut run.game, 0.5);
        let mana = run.game.snapshot().player.mana;
        assert!(run.game.activate(Ability::Spell(SLOT)).is_err());
        assert_eq!(run.game.snapshot().player.mana, mana);
        assert!(run.game.spells.feather_fall.casts.is_empty());
    }

    #[test]
    fn the_adventurer_catches_itself_walking_off_the_tower() {
        let mut run = crate::playground::Run::new(scenario()).unwrap();
        let game = &mut run.game;
        game.spells.feather_fall.marks.clear();
        settle(game, 0.3);
        // Walk north off the tower's edge, clear of the dummies, then react
        // once falling.
        game.face(std::f32::consts::PI).unwrap();
        let hp = game.snapshot().player.hp;
        let mut reacted = false;
        for _ in 0..240 {
            let falling = game.character.airborne() && game.character.vertical_speed < -2.;
            if falling && !reacted {
                game.activate(Ability::Spell(SLOT)).unwrap();
                reacted = true;
            }
            game.tick(1. / 30., if reacted { [0.; 2] } else { [0., 1.] })
                .unwrap();
            if reacted && !game.character.airborne() {
                break;
            }
        }
        assert!(reacted);
        settle(game, 0.2);
        assert!(!game.character.airborne());
        assert!(game.player.y < 0.1, "{}", game.player);
        assert_eq!(game.snapshot().player.hp, hp);
        assert!(
            game.spells
                .log
                .iter()
                .any(|r| r.spell == NAME && r.text.contains("lands softly")),
        );
        assert!(!game.spells.log.iter().any(|r| r.spell == "Falling"));
        assert_eq!(game.character.gravity, None);
    }
}
