//! The Grove: a druid training field on Everglade's pinned pack
//! (`docs/verse/druid-demo.md`, The build).
//!
//! A meadow about 60 m across, ringed by the nature kit's trees, bushes,
//! rocks, and flowers ([`layout`]), under Everglade's sky, haze, and light.
//! Seven training dummies stand in it ([`dummies`]). The player walks as
//! Everglade's character with Everglade's controls, and casts from the
//! Archdruid's four-row bar ([`slots`], [`hotbar`]): Wild Shape and the
//! druid's features, the cantrips and spells of levels 1 to 9, and the
//! chosen land's six spells, with Choose Land to swap lands and Long Rest
//! to stand the field back up ([`kit`]). Nothing gates a cast, so the
//! player can spam: every press casts, and a held key recasts six times a
//! second. The live effects, lasting areas, particles, and floating
//! numbers are capped, oldest first, so spam stays bounded in memory and
//! frame time.
//!
//! The runtime keeps an [`Everglade`] for the Grove too, built from these
//! placements: it moves the player over the heightfield and owns Wind
//! Wall, Wall of Stone, Reverse Gravity, and the eagle's flight, whose
//! rules come from `verse_world`. This module adds what acts on the
//! dummies ([`cast`], [`aura`]). Each spell's dice, attack roll, and saving
//! throw are rolled behind the scenes with `verse_world`'s seeded dice, as
//! the combat model says (`docs/verse/combat-model.md`), and the dummies
//! show the outcome: floating numbers, "Miss", "Resisted", and timed
//! conditions over their health bars, with each line in the combat log.
//!
//! Wild Shape turns the druid into a beast ([`shape`]), and Shapechange
//! into a dragon through a transformation of leaves and light ([`dragon`]).

pub(crate) mod aura;
mod cast;
pub mod dragon;
pub(crate) mod draw;
pub mod dummies;
pub mod hotbar;
pub mod kit;
pub mod layout;
pub mod shape;
pub mod slots;
#[cfg(test)]
mod tests;
pub(crate) mod thunder;

use super::Intent;
use super::everglade::{self, Everglade, layout::Placement, player::Beast};
use super::everglade_pack::ZonePack;
use crate::controller::PlayerController;
use crate::fx::{Particles, Spawn};
use crate::mesh::Mesh;
use crate::pbr::textured::Figure;
use crate::world::World;
use dummies::{Condition, Dummy};
use glam::Vec3;
use kit::{Damage, Def, Delivery, Land, Spell};
use shape::{Form, Shape};
use std::sync::Arc;
use verse_world::spells::Dice;

/// Where the player arrives, facing the field (+z).
pub const SPAWN: Vec3 = Vec3::new(0.0, 0.0, -15.0);
pub const SPAWN_YAW: f32 = 0.0;
/// The return arch, south of the spawn.
pub const RETURN_PORTAL: Vec3 = Vec3::new(0.0, 0.0, -31.0);
/// The meadow's radius, m: inside Everglade's flat clearing.
pub const MEADOW_RADIUS: f32 = 30.0;
/// Half the angle of the cone a spell finds its target in, radians.
const CONE: f32 = 0.7;
/// Lines the combat log keeps.
pub const LOG: usize = 8;
/// Live effects of every kind together, oldest dropped first; each kind
/// also has its own cap ([`draw::Effect::cap`]).
pub const MAX_EFFECTS: usize = 64;
/// Floating numbers at once, oldest dropped first.
pub const MAX_FLOATERS: usize = 48;
/// Poison's damage each second on a poisoned dummy.
const POISON_TICK: f32 = 3.0;

/// The Grove's placements as one list, for the static scene and the solids.
#[must_use]
pub fn placements() -> Vec<Placement> {
    layout::placements()
}

/// The meadow's static world: the textured placements and the grass, with
/// their blockers; no dirt, boards, or workshop.
///
/// # Errors
///
/// Returns a message when the pack lacks a placed model.
pub(crate) fn world(pack: &ZonePack) -> Result<World, String> {
    let mut world = World::default();
    let (mut scene, blockers) = everglade::scene::build(pack, &placements())?;
    everglade::draw::ground_with(&mut scene, false);
    scene.validate()?;
    world.mesh.textured = Some(Arc::new(scene));
    world.blockers = blockers;
    Ok(world)
}

/// The glade machinery the Grove walks with: movement, the three glade
/// spells, and the character, over the meadow's solids.
///
/// # Errors
///
/// Returns a message when the pack lacks a model or its character cannot
/// play.
pub(crate) fn glade(pack: &ZonePack, at: &PlayerController) -> Result<Everglade, String> {
    let solids = everglade::solids::build_with(pack, &placements(), &[])?;
    let mut glade = Everglade::with_solids(pack, at, solids)?;
    glade.set_free_casting();
    Ok(glade)
}

/// The training field's live state.
pub(crate) struct Grove {
    pub dummies: Vec<Dummy>,
    dice: Dice,
    time: f32,
    /// When each held hotbar slot next recasts.
    held: [Option<f32>; slots::COUNT],
    effects: Vec<draw::Effect>,
    floaters: Vec<draw::Floater>,
    /// The newest combat lines, oldest first.
    pub log: Vec<String>,
    model: draw::Model,
    /// Each form the pack carries, in [`Form::ALL`] order.
    beasts: Vec<Option<Beast>>,
    /// The beast's shape the druid wears, if any.
    shape: Option<Shape>,
    /// The Circle of the Land's chosen land.
    land: Land,
    /// The lasting areas: Moonbeam, the walls, the storms, and the clouds.
    auras: Vec<aura::Aura>,
    /// The spells' particles.
    fx: Particles,
    /// The outlined dummies' faerie light, by dummy.
    glows: Vec<(usize, crate::fx::Handle)>,
    /// When poison next bites, s.
    next_poison: f32,
    /// A change of shape under way: Shapechange or its return.
    morph: Option<dragon::Morph>,
    /// The dragon's breath of fire under way.
    breath: Option<dragon::Breath>,
    /// The burning dummies' flames, by dummy.
    burns: Vec<(usize, crate::fx::Handle)>,
    /// When burning next bites, s.
    next_burn: f32,
    /// How many times its usual distance the camera stands back.
    pull: f32,
    /// When the dragon last roared, for the camera's jolt, s.
    roared: f32,
    /// The player's height a frame ago, to tell a climb, m.
    last_y: f32,
    /// Where the player stood at the last tick.
    feet: Vec3,
}

/// What one roll did to one dummy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The attack missed.
    Miss,
    /// It saved against a spell that does nothing on a success.
    Resisted,
    /// It landed: the damage dealt, whether it was a critical hit, and
    /// whether a save halved it. Only a landing that no save halved
    /// pushes, lifts, roots, or leaves a condition.
    Hit { dealt: i32, crit: bool, saved: bool },
}

impl Outcome {
    /// Whether the spell's push, lift, root, or condition takes hold.
    #[must_use]
    pub const fn takes_hold(self) -> bool {
        matches!(self, Self::Hit { saved: false, .. })
    }
}

impl Grove {
    /// The field at rest, drawn beside `glade`'s character.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack has no dummy.
    pub fn new(pack: &ZonePack, glade: &Everglade) -> Result<Self, String> {
        let dummies = Dummy::field();
        let beasts = shape::beasts(pack)?;
        let forms: Vec<Option<Figure>> = beasts
            .iter()
            .map(|beast| beast.as_ref().map(Beast::figure))
            .collect();
        let model = draw::Model::new(pack, glade.cast_figure().as_ref(), &forms, dummies.len())?;
        Ok(Self {
            dummies,
            dice: Dice::new(kit::DICE_SEED),
            time: 0.0,
            held: [None; slots::COUNT],
            effects: Vec::new(),
            floaters: Vec::new(),
            log: Vec::new(),
            model,
            beasts,
            shape: None,
            land: Land::Arid,
            auras: Vec::new(),
            fx: Particles::new(0x6720_F00D),
            glows: Vec::new(),
            next_poison: 0.0,
            morph: None,
            breath: None,
            burns: Vec::new(),
            next_burn: 0.0,
            pull: 1.0,
            roared: f32::NEG_INFINITY,
            last_y: 0.0,
            feet: SPAWN,
        })
    }

    /// The beast the druid is now, if any.
    #[must_use]
    pub fn form(&self) -> Option<Form> {
        self.shape.map(|s| s.form)
    }

    /// The chosen land.
    #[must_use]
    pub fn land(&self) -> Land {
        self.land
    }

    /// What hotbar slot `index` casts now.
    #[must_use]
    pub fn slot_spell(&self, index: usize) -> Option<Spell> {
        slots::spell(index, self.form(), self.land)
    }

    /// The hotbar slot `intent` presses: a slot itself, or the slot a
    /// named spell's intent sits on now.
    #[must_use]
    pub fn slot_of(&self, intent: Intent) -> Option<usize> {
        match intent {
            Intent::GroveSlot(index) => {
                (usize::from(index) < slots::COUNT).then_some(usize::from(index))
            }
            _ => slots::slot_of(Spell::of(intent)?, self.form(), self.land),
        }
    }

    /// The spell `intent` casts now: what its slot holds, or a named
    /// spell.
    #[must_use]
    pub fn resolve(&self, intent: Intent) -> Option<Spell> {
        match intent {
            Intent::GroveSlot(index) => self.slot_spell(usize::from(index)),
            _ => Spell::of(intent),
        }
    }

    /// The dummy a spell of `range` m aims at from `player`: the standing
    /// one nearest the facing within the cone, by angle and then distance.
    #[must_use]
    pub fn target(&self, player: &PlayerController, range: f32) -> Option<usize> {
        let forward = player.forward();
        self.dummies
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.down())
            .filter_map(|(i, d)| {
                let to = d.center() - player.pos;
                let flat = Vec3::new(to.x, 0.0, to.z);
                let distance = flat.length();
                if distance > range || distance < 1e-3 {
                    return None;
                }
                let angle = forward.angle_between(flat);
                (angle <= CONE).then_some((i, angle + distance * 0.004))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Rolls `spell` against dummy `i` behind the scenes, deals its damage
    /// or healing, puts its condition on a landing no save halved, and
    /// shows the outcome over the dummy and in the log.
    fn strike(&mut self, spell: Spell, i: usize) -> Outcome {
        self.strike_def(&spell.def(), i)
    }

    /// As [`Self::strike`], with the numbers in `def`, such as one round
    /// of Storm of Vengeance.
    fn strike_def(&mut self, def: &Def, i: usize) -> Outcome {
        let now = self.time;
        let kind = self.dummies[i].kind;
        let (count, sides) = def.dice;
        if def.kind == Damage::Healing {
            let amount = (self.dice.sum(count, sides) as i32 + def.bonus).max(0) as f32;
            let healed = self.dummies[i].heal(amount, now);
            self.float(i, format!("+{healed}"), Damage::Healing.color());
            let name = kind.name();
            self.say(format!("{} heals {name} for {healed}", def.label));
            return Outcome::Hit {
                dealt: healed,
                crit: false,
                saved: false,
            };
        }
        // The dice of the spell's damage and of its second damage, doubled
        // on a crit, with the flat bonus on the first.
        let roll_damage = |dice: &mut Dice, crit: bool| -> (f32, f32) {
            let times = if crit { 2 } else { 1 };
            let first = (dice.sum(count * times, sides) as i32 + def.bonus).max(0) as f32;
            let second = def
                .extra
                .map_or(0.0, |(n, s, _)| dice.sum(n * times, s) as f32);
            (first, second)
        };
        let mut second_dealt = 0;
        let outcome = match def.delivery {
            Delivery::Attack { bonus } => {
                let roll = self.dice.roll(20);
                let total = roll as i32 + bonus;
                if roll == 1 || (roll != 20 && total < kind.armor_class()) {
                    Outcome::Miss
                } else {
                    let crit = roll == 20;
                    let (damage, second) = roll_damage(&mut self.dice, crit);
                    let dealt = self.dummies[i].damage(damage, def.kind, now);
                    if let Some((_, _, other)) = def.extra {
                        second_dealt = self.dummies[i].damage(second, other, now);
                    }
                    Outcome::Hit {
                        dealt,
                        crit,
                        saved: false,
                    }
                }
            }
            Delivery::Save { ability, half } => {
                let save =
                    self.dice
                        .save(i as u64, ability.name(), kind.save(ability), kit::SAVE_DC);
                let (damage, second) = roll_damage(&mut self.dice, false);
                if save.success && !half {
                    self.dummies[i].damage(0.0, def.kind, now);
                    Outcome::Resisted
                } else {
                    let halve = |d: f32| if save.success { (d / 2.0).floor() } else { d };
                    let dealt = self.dummies[i].damage(halve(damage), def.kind, now);
                    if let Some((_, _, other)) = def.extra {
                        second_dealt = self.dummies[i].damage(halve(second), other, now);
                    }
                    Outcome::Hit {
                        dealt,
                        crit: false,
                        saved: save.success,
                    }
                }
            }
            Delivery::Automatic => {
                let (damage, second) = roll_damage(&mut self.dice, false);
                let dealt = self.dummies[i].damage(damage, def.kind, now);
                if let Some((_, _, other)) = def.extra {
                    second_dealt = self.dummies[i].damage(second, other, now);
                }
                Outcome::Hit {
                    dealt,
                    crit: false,
                    saved: false,
                }
            }
        };
        let name = kind.name();
        match outcome {
            Outcome::Miss => {
                self.float(i, "Miss".into(), [0.85, 0.85, 0.95]);
                self.say(format!("{} misses {name}", def.label));
            }
            Outcome::Resisted => {
                self.float(i, "Resisted".into(), [0.75, 0.9, 1.0]);
                self.say(format!("{name} resists {}", def.label));
            }
            Outcome::Hit { dealt, crit, saved } => {
                if count > 0 {
                    let text = if crit {
                        format!("Crit {dealt}")
                    } else {
                        dealt.to_string()
                    };
                    self.float(i, text, def.kind.color());
                    let how = if crit {
                        " (crit)"
                    } else if saved {
                        " (saved, half)"
                    } else {
                        ""
                    };
                    let also = match def.extra {
                        Some((_, _, other)) => {
                            self.float(i, second_dealt.to_string(), other.color());
                            // Above the first number rather than over it.
                            if let Some(floater) = self.floaters.last_mut() {
                                floater.at.y += 0.45;
                            }
                            format!(" and {second_dealt} {}", other.word())
                        }
                        None => String::new(),
                    };
                    let resisted = kind.multiplier(def.kind) < 1.0;
                    let resist = if resisted { ", resisted" } else { "" };
                    self.say(format!(
                        "{} hits {name}: {dealt} {}{also}{how}{resist}",
                        def.label,
                        def.kind.word()
                    ));
                }
                if !saved && let Some((condition, seconds)) = def.rider {
                    self.afflict(i, condition, seconds, def.label);
                }
            }
        }
        outcome
    }

    /// Puts `condition` on dummy `i` for `seconds`, as `source` does, and
    /// logs it, or that diminishing returns made it immune.
    fn afflict(&mut self, i: usize, condition: Condition, seconds: f32, source: &str) {
        let now = self.time;
        let applied = self.dummies[i].afflict(condition, seconds, now);
        let name = self.dummies[i].kind.name();
        if applied > 0.0 {
            // Short repeats (a cloud's each second) don't fill the log.
            if seconds >= 1.5 {
                self.say(format!(
                    "{source}: {name} is {} for {applied:.1} s",
                    condition.name()
                ));
            }
        } else {
            self.float(i, "Immune".into(), [0.8, 0.7, 1.0]);
            self.say(format!(
                "{name} is immune to being {} for now",
                condition.name()
            ));
        }
    }

    /// Takes `form`'s shape: the player's character gives way to the
    /// beast, at its pace. The eagle takes to the air. Taking a shape ends
    /// any other.
    fn take_shape(
        &mut self,
        form: Form,
        player: &mut PlayerController,
        glade: &mut Everglade,
    ) -> Result<(), String> {
        if self.beasts.get(form.index()).is_none_or(Option::is_none) {
            return Err(format!("The pack has no {}", form.name()));
        }
        self.end_shape(player, glade);
        self.wear(Some(form), player, glade);
        player.set_pace(form.pace());
        self.add(draw::Effect::Shift {
            at: player.pos,
            start: self.time,
        });
        self.say(format!("Wild Shape: you become a {}", form.name()));
        Ok(())
    }

    /// Ends the beast's shape, if any, the pace it set, and the eagle's
    /// flight.
    fn end_shape(&mut self, player: &mut PlayerController, glade: &mut Everglade) {
        self.morph = None;
        self.stop_breath();
        if let Some(shape) = self.shape.take()
            && shape.form.flies()
            && glade.levitating
        {
            glade.toggle_levitate(player);
        }
        glade.set_lift(1.0);
        player.set_pace(1.0);
    }

    /// Starts the beast's attack clip.
    fn attack(&mut self) {
        if let Some(shape) = &mut self.shape {
            shape.attack = Some(self.time);
        }
    }

    /// Stands the field back up and ends the glade's spells, the lasting
    /// areas, and the beast's shape.
    fn long_rest(&mut self, player: &mut PlayerController, glade: &mut Everglade) {
        glade.long_rest();
        self.end_shape(player, glade);
        for dummy in &mut self.dummies {
            dummy.reset();
        }
        self.effects.clear();
        self.floaters.clear();
        self.end_auras(|_| true);
        self.fx.clear();
        self.glows.clear();
        self.burns.clear();
        self.add(draw::Effect::Rest {
            at: player.pos,
            start: self.time,
        });
        self.say("Long Rest: spells ended and dummies refilled".into());
    }

    /// Adds `effect`, first dropping the oldest of its kind past the kind's
    /// cap and the oldest of all past [`MAX_EFFECTS`]. A dropped bolt's
    /// trail stops.
    fn add(&mut self, effect: draw::Effect) {
        let kind = std::mem::discriminant(&effect);
        let same = self
            .effects
            .iter()
            .filter(|e| std::mem::discriminant(*e) == kind)
            .count();
        if same >= effect.cap()
            && let Some(oldest) = self
                .effects
                .iter()
                .position(|e| std::mem::discriminant(e) == kind)
        {
            let dropped = self.effects.remove(oldest);
            self.stop_trail(&dropped);
        }
        if self.effects.len() >= MAX_EFFECTS {
            let dropped = self.effects.remove(0);
            self.stop_trail(&dropped);
        }
        self.effects.push(effect);
    }

    fn stop_trail(&mut self, effect: &draw::Effect) {
        if let draw::Effect::Bolt {
            trail: Some(trail), ..
        } = effect
        {
            self.fx.stop(*trail);
        }
    }

    /// Starts the particle effect `name` at `at`; a full system skips it.
    fn burst(&mut self, name: &str, at: Vec3) {
        let _ = self.fx.start(name, Spawn::at(at));
    }

    /// The live effects, oldest first.
    #[cfg(test)]
    #[must_use]
    pub fn effects(&self) -> &[draw::Effect] {
        &self.effects
    }

    /// The lasting areas, oldest first.
    #[cfg(test)]
    #[must_use]
    pub fn auras(&self) -> &[aura::Aura] {
        &self.auras
    }

    /// Particles alive now.
    #[cfg(test)]
    #[must_use]
    pub fn particles(&self) -> usize {
        self.fx.len()
    }

    /// Holds or lets go of hotbar slot `index`. While held, the slot
    /// recasts what it holds every [`kit::REPEAT`] seconds after the
    /// press's own cast; a demo control or a change of shape never repeats.
    pub fn hold(&mut self, index: usize, down: bool) {
        let repeats = self.slot_spell(index).is_some_and(Spell::repeats);
        if let Some(held) = self.held.get_mut(index) {
            *held = (down && repeats).then_some(self.time + kit::REPEAT);
        }
    }

    /// Lets go of every held key, as when the window loses focus.
    pub fn release(&mut self) {
        self.held = [None; slots::COUNT];
    }

    /// The held slots due to recast now, each at most once a frame; a long
    /// frame skips the missed repeats rather than bursting them.
    pub fn due(&mut self) -> Vec<usize> {
        let now = self.time;
        let mut due = Vec::new();
        for (index, next) in self.held.iter_mut().enumerate() {
            if let Some(at) = next
                && *at <= now
            {
                due.push(index);
                *at = (*at + kit::REPEAT).max(now + kit::REPEAT * 0.5);
            }
        }
        due
    }

    /// The camera's shake from the newest Thunderwave, m.
    #[must_use]
    pub fn shake(&self) -> Vec3 {
        let now = self.time;
        self.effects
            .iter()
            .rev()
            .find_map(|effect| match *effect {
                draw::Effect::Wave { start, .. } => Some(draw::shake(now - start)),
                _ => None,
            })
            .unwrap_or(Vec3::ZERO)
            + self.roar_shake()
    }

    fn float(&mut self, i: usize, text: String, color: [f32; 3]) {
        let d = &self.dummies[i];
        if self.floaters.len() >= MAX_FLOATERS {
            self.floaters.remove(0);
        }
        self.floaters.push(draw::Floater {
            at: d.pos + Vec3::Y * (d.top() - d.pos.y + 0.75),
            text,
            color,
            start: self.time,
        });
    }

    fn say(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > LOG {
            self.log.remove(0);
        }
    }

    /// Advances the field `dt` seconds: bolts land, the lasting areas act,
    /// poison bites, dummies move and reset, the dummies block walking
    /// where they stand, the particles move, and the beast the druid wears
    /// poses where `player` stands.
    pub fn tick(&mut self, dt: f32, glade: &mut Everglade, player: &PlayerController) {
        self.time += dt.max(0.0);
        let now = self.time;
        self.tick_morph(glade, player);
        let special = self.pose_special(dt, glade, player);
        if !special
            && let Some(shape) = &mut self.shape
            && let Some(Some(beast)) = self.beasts.get_mut(shape.form.index())
        {
            let length = beast.attack_length().unwrap_or(0.0);
            let attack = shape
                .attack
                .map(|at| now - at)
                .filter(|&t| (0.0..length).contains(&t));
            if attack.is_none() {
                shape.attack = None;
            }
            // A flying form beats its wings whenever it is off the ground,
            // hovering at a held altitude included.
            let aloft = shape.form.flies() && (glade.levitating || player.airborne());
            beast.advance(player, shape.form.scale(), attack, aloft, dt);
        }
        self.land_bolts();
        let gravity = glade.spells().reverse_gravity().cloned();
        for dummy in &mut self.dummies {
            dummy.tick(dt, now, gravity.as_ref());
        }
        self.tick_auras();
        if now >= self.next_poison {
            self.next_poison = now + 1.0;
            for i in 0..self.dummies.len() {
                if self.dummies[i].has(Condition::Poisoned, now) && !self.dummies[i].down() {
                    let dealt = self.dummies[i].damage(POISON_TICK, Damage::Poison, now);
                    self.float(i, dealt.to_string(), Damage::Poison.color());
                }
            }
        }
        self.tick_glows();
        self.tick_breath(player);
        self.tick_burning();
        self.tick_camera(dt);
        self.last_y = player.pos.y;
        self.feet = player.pos;
        self.fx.tick(dt, everglade::height);
        self.floaters.retain(|f| now - f.start < draw::FLOAT);
        glade.set_extra_blocks(self.dummies.iter().map(Dummy::block).collect());
    }

    /// Keeps faerie light on each outlined dummy and puts it out when the
    /// outline ends.
    fn tick_glows(&mut self) {
        let now = self.time;
        let mut glows = std::mem::take(&mut self.glows);
        glows.retain(|&(i, handle)| {
            let lit = self
                .dummies
                .get(i)
                .is_some_and(|d| d.has(Condition::Outlined, now) && !d.down());
            if lit {
                let at = self.dummies[i].center();
                self.fx.place(handle, at, Vec3::ZERO);
            } else {
                self.fx.stop(handle);
            }
            lit
        });
        for i in 0..self.dummies.len() {
            if self.dummies[i].has(Condition::Outlined, now)
                && !glows.iter().any(|(g, _)| *g == i)
                && let Some(handle) = self
                    .fx
                    .start("grove_faerie", Spawn::at(self.dummies[i].center()))
            {
                glows.push((i, handle));
            }
        }
        self.glows = glows;
    }

    /// The player's character, or the beast the druid wears, and the
    /// dummies, posed for this frame and lit by `glade`'s probes.
    #[must_use]
    pub fn figure(&self, glade: &Everglade) -> Figure {
        // In a beast's shape the beast draws and the druid doesn't.
        let worn = self.shape.and_then(|shape| {
            let beast = self.beasts.get(shape.form.index())?.as_ref()?;
            Some((shape.form.index(), beast.vertices()))
        });
        let mut figure =
            self.model
                .figure(glade.cast_figure().as_ref(), worn, &self.dummies, self.time);
        let mut vertices = figure.vertices.as_ref().clone();
        // The druid dissolving into Shapechange's vortex, or forming again.
        if let Some((k, spin)) = self.druid_morph() {
            Self::dissolve(&mut vertices, self.model.cast_count(), self.feet, k, spin);
        }
        glade.shade(&mut vertices);
        figure.vertices = Arc::new(vertices);
        figure
    }

    /// The bars, conditions, numbers, post, target ring, lasting areas,
    /// effects, and particles, seen from `eye` by `player`.
    #[must_use]
    pub fn mesh(&self, eye: Vec3, player: &PlayerController) -> Mesh {
        let now = self.time;
        let mut painter = draw::Painter::new(eye);
        let target = self.target(player, Spell::Fireball.def().range);
        for (i, dummy) in self.dummies.iter().enumerate() {
            if dummy.anchored() {
                painter.post(dummy);
            }
            let rooted = dummy.rooted(now);
            if rooted {
                painter.strands(dummy, now);
            }
            if dummy.has(Condition::Outlined, now) {
                painter.outline(dummy, now);
            }
            painter.bar(dummy, &dummy.conditions(now), target == Some(i));
        }
        if let Some(i) = target {
            painter.target(&self.dummies[i], now);
        }
        // Newest first, so a renderer that runs out of glow quads under spam
        // drops the oldest blasts.
        for effect in self.effects.iter().rev() {
            let at = match effect {
                draw::Effect::Bolt { target, .. } => self.dummies.get(*target).map(Dummy::center),
                _ => None,
            };
            painter.effect(effect, now, at);
        }
        for floater in &self.floaters {
            painter.floater(floater, now);
        }
        let mut mesh = painter.mesh;
        self.fx.draw(&mut mesh.sprites);
        mesh
    }

    /// The hotbar: each slot's ability, lit when it has what it needs to
    /// cast. No slot ever cools down.
    #[must_use]
    pub fn bar(&self, player: &PlayerController, glade: &Everglade) -> hotbar::Bar {
        let form = self.form();
        let spells: [Option<Spell>; slots::COUNT] =
            std::array::from_fn(|i| slots::spell(i, form, self.land));
        let slots = spells.map(|spell| {
            let Some(spell) = spell else {
                return everglade::hotbar::Slot::default();
            };
            let active = spell.glade().is_some_and(|g| glade.spell_active(g))
                || Form::of(spell).is_some_and(|f| Some(f) == form)
                || self.auras.iter().any(|a| a.spell == spell);
            let targeted =
                !spell.needs_target() || self.target(player, spell.def().range).is_some();
            let ready = match spell {
                Spell::ReturnToForm => form.is_some(),
                _ => true,
            };
            everglade::hotbar::Slot {
                enabled: (active || targeted) && ready,
                active,
                cooldown: 0.0,
            }
        });
        hotbar::Bar { spells, slots }
    }

    /// The combat log's heading: the land, and the shape the druid wears.
    #[must_use]
    pub fn status(&self) -> String {
        let form = self.form().map_or("Druid", Form::name);
        format!("Land: {} · Form: {form}", self.land.name())
    }

    /// The zone caption: the soft target and the newest combat line.
    #[must_use]
    pub fn caption(&self, player: &PlayerController) -> String {
        let mut caption = String::from("Grove");
        match self.target(player, Spell::Fireball.def().range) {
            Some(i) => {
                let d = &self.dummies[i];
                caption.push_str(&format!(
                    "\nTarget: {} · {:.0}/{:.0}",
                    d.kind.name(),
                    d.hp.ceil(),
                    d.kind.max_hp()
                ));
            }
            None => caption.push_str("\nFace a training dummy"),
        }
        if let Some(line) = self.log.last() {
            caption.push('\n');
            caption.push_str(line);
        }
        caption
    }
}
