//! The Grove: a druid training field on Everglade's pinned pack
//! (`docs/verse/druid-demo.md`, The build).
//!
//! A meadow about 60 m across, ringed by the nature kit's trees, bushes,
//! rocks, and flowers ([`layout`]), under Everglade's sky, haze, and light.
//! Seven training dummies stand in it ([`dummies`]). The player walks as
//! Everglade's character with Everglade's controls, and casts the druid's
//! spells from an icon hotbar ([`hotbar`]): Thunderwave, Gust of Wind,
//! Wind Wall, Wall of Stone, Reverse Gravity, Fire Bolt, Fireball, Misty
//! Step, and Web, with Long Rest to refill everything ([`kit`]).
//!
//! The runtime keeps an [`Everglade`] for the Grove too, built from these
//! placements: it moves the player over the heightfield and owns Wind
//! Wall, Wall of Stone, and Reverse Gravity, whose rules come from
//! `verse_world`. This module adds what acts on the dummies. Each spell's
//! dice, attack roll, and saving throw are rolled behind the scenes with
//! `verse_world`'s seeded dice, as the combat model says
//! (`docs/verse/combat-model.md`), and the dummies show the outcome:
//! floating numbers, "Miss", and "Resisted".

pub(crate) mod draw;
pub mod dummies;
pub mod hotbar;
pub mod kit;
pub mod layout;
#[cfg(test)]
mod tests;

use super::everglade::{self, Everglade, layout::Placement};
use super::everglade_pack::ZonePack;
use crate::controller::PlayerController;
use crate::mesh::Mesh;
use crate::pbr::textured::Figure;
use crate::world::World;
use dummies::Dummy;
use glam::{DVec3, Vec3};
use kit::{Delivery, Kit, Spell};
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
/// How long a web holds, s (two rounds).
const WEB_ROOT: f32 = 12.0;
/// Bolt speeds, m/s.
const BOLT_SPEED: f32 = 32.0;
const FIREBALL_SPEED: f32 = 24.0;
/// Gust of Wind's and Thunderwave's pushes, m.
const GUST_PUSH: f32 = 15.0 * 0.3048;
/// Wind Wall's lift on a dummy that fails its save, m/s.
const WIND_LIFT: f32 = 8.0;
/// How far Wall of Stone shoves a dummy past its face, m.
const SHOVE_CLEARANCE: f64 = 0.5;
/// Lines the combat log keeps.
const LOG: usize = 4;

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
    Everglade::with_solids(pack, at, solids)
}

/// The training field's live state.
pub(crate) struct Grove {
    pub dummies: Vec<Dummy>,
    pub kit: Kit,
    dice: Dice,
    time: f32,
    effects: Vec<draw::Effect>,
    floaters: Vec<draw::Floater>,
    /// The newest combat lines, oldest first.
    pub log: Vec<String>,
    model: draw::Model,
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
    /// pushes, lifts, or roots.
    Hit { dealt: i32, crit: bool, saved: bool },
}

impl Outcome {
    /// Whether the spell's push, lift, or root takes hold.
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
        let model = draw::Model::new(pack, glade.cast_figure().as_ref(), dummies.len())?;
        Ok(Self {
            dummies,
            kit: Kit::default(),
            dice: Dice::new(kit::DICE_SEED),
            time: 0.0,
            effects: Vec::new(),
            floaters: Vec::new(),
            log: Vec::new(),
            model,
        })
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

    /// Casts `spell` for `player`, who turns to face its target, through
    /// `glade` for the glade's spells.
    ///
    /// # Errors
    ///
    /// Returns why the cast was refused: a cooldown, mana, no target, or a
    /// glade spell's own rule. A refused cast spends nothing.
    pub fn cast(
        &mut self,
        spell: Spell,
        player: &mut PlayerController,
        glade: &mut Everglade,
    ) -> Result<(), String> {
        let now = self.time;
        if spell == Spell::LongRest {
            self.long_rest(player, glade);
            return Ok(());
        }
        // Pressing a live concentration spell's slot ends it, freely.
        if let Some(glade_spell) = spell.glade()
            && glade.spell_active(glade_spell)
        {
            return glade.cast_spell(glade_spell, player);
        }
        self.kit.admit(spell)?;
        let def = spell.def();
        let target = self.target(player, def.range.max(6.0));
        if spell.needs_target() && target.is_none() {
            return Err(format!(
                "{} needs a dummy in front within {:.0} m",
                def.label, def.range
            ));
        }
        if let Some(i) = target {
            let to = self.dummies[i].pos - player.pos;
            player.yaw = to.x.atan2(to.z);
        }
        let feet = player.pos;
        let forward = player.forward();
        let hand = feet + Vec3::Y * 1.4 + forward * 0.3;
        match spell {
            Spell::FireBolt | Spell::Fireball => {
                let i = target.ok_or("no target")?;
                let fireball = spell == Spell::Fireball;
                let distance = hand.distance(self.dummies[i].center());
                let speed = if fireball { FIREBALL_SPEED } else { BOLT_SPEED };
                self.effects.push(draw::Effect::Bolt {
                    from: hand,
                    target: i,
                    start: now,
                    flight: distance / speed,
                    fireball,
                });
            }
            Spell::Thunderwave => {
                let cube = verse_world::spells::thunderwave::Cube::new(
                    feet.as_dvec3(),
                    forward.as_dvec3(),
                );
                for i in 0..self.dummies.len() {
                    let d = &self.dummies[i];
                    let probe = d.pos.as_dvec3() + DVec3::Y * 0.9;
                    if d.down() || !cube.contains(probe) {
                        continue;
                    }
                    let away = cube.away(probe).as_vec3();
                    if self.strike(spell, i).takes_hold() {
                        let push = verse_world::spells::thunderwave::PUSH as f32;
                        self.dummies[i].push(away, push, now);
                    }
                }
                self.effects.push(draw::Effect::Wave {
                    origin: cube.origin.as_vec3(),
                    forward: cube.forward.as_vec3(),
                    start: now,
                });
            }
            Spell::GustOfWind => {
                let line = verse_world::gust::Line::new(feet.as_dvec3(), forward.as_dvec3())
                    .ok_or("Gust of Wind needs a direction")?;
                for i in 0..self.dummies.len() {
                    let d = &self.dummies[i];
                    let radius = f64::from(0.35 * d.kind.scale());
                    if d.down() || !line.touches_sphere(d.center().as_dvec3(), radius) {
                        continue;
                    }
                    if self.strike(spell, i).takes_hold() {
                        self.dummies[i].push(forward, GUST_PUSH, now);
                    }
                }
                self.effects.push(draw::Effect::Gust {
                    origin: feet,
                    forward,
                    start: now,
                });
            }
            Spell::WindWall | Spell::WallOfStone => {
                let ahead = target.map_or(f64::from(everglade::spells::AHEAD), |i| {
                    let to = self.dummies[i].pos - feet;
                    f64::from(to.x.hypot(to.z))
                });
                let glade_spell = spell.glade().ok_or("not a glade spell")?;
                glade.cast_spell_ahead(glade_spell, player, ahead)?;
                if spell == Spell::WindWall {
                    self.wind_wall(glade);
                } else {
                    self.shove(glade, feet);
                }
            }
            Spell::ReverseGravity => {
                glade.cast_spell(everglade::spells::Spell::ReverseGravity, player)?;
                self.say("Reverse Gravity: everything nearby falls upward".into());
            }
            Spell::MistyStep => {
                let reach = def.range;
                let step = target.map_or(reach, |i| {
                    let to = self.dummies[i].pos - feet;
                    (to.x.hypot(to.z) - 2.0).clamp(0.0, reach)
                });
                let mut to = feet + forward * step;
                let r = to.x.hypot(to.z);
                if r > MEADOW_RADIUS {
                    to.x *= MEADOW_RADIUS / r;
                    to.z *= MEADOW_RADIUS / r;
                }
                to.y = everglade::height(to.x, to.z);
                self.effects.push(draw::Effect::Mist {
                    at: feet,
                    start: now,
                });
                self.effects.push(draw::Effect::Mist { at: to, start: now });
                player.pos = to;
                player.set_surface_height(to.y);
                player.set_vertical_speed(0.0);
            }
            Spell::Web => {
                let i = target.ok_or("no target")?;
                let at = self.dummies[i].pos;
                // A 20-foot cube centered on the target.
                let half = 10.0 * 0.3048;
                for j in 0..self.dummies.len() {
                    let d = &self.dummies[j];
                    let off = d.pos - at;
                    if d.down() || off.x.abs() > half || off.z.abs() > half || off.y.abs() > half {
                        continue;
                    }
                    if self.strike(spell, j).takes_hold() {
                        let held = self.dummies[j].root(WEB_ROOT, now);
                        let name = self.dummies[j].kind.name();
                        if held > 0.0 {
                            self.say(format!("Web roots {name} for {held:.0} s"));
                        } else {
                            self.float(j, "Immune".into(), [0.8, 0.7, 1.0]);
                            self.say(format!("{name} is immune to Web for now"));
                        }
                    }
                }
                self.effects.push(draw::Effect::Web {
                    at,
                    start: now,
                    until: now + WEB_ROOT,
                });
            }
            Spell::LongRest => {}
        }
        self.kit.spend(spell);
        Ok(())
    }

    /// Rolls `spell` against dummy `i` behind the scenes, deals its damage,
    /// and shows the outcome over the dummy and in the log.
    fn strike(&mut self, spell: Spell, i: usize) -> Outcome {
        let def = spell.def();
        let now = self.time;
        let kind = self.dummies[i].kind;
        let (count, sides) = def.dice;
        let outcome = match def.delivery {
            Delivery::Attack => {
                let roll = self.dice.roll(20);
                let total = roll as i32 + kit::ATTACK_BONUS;
                if roll == 1 || (roll != 20 && total < kind.armor_class()) {
                    Outcome::Miss
                } else {
                    let crit = roll == 20;
                    let damage = self.dice.sum(if crit { count * 2 } else { count }, sides);
                    let dealt = self.dummies[i].damage(damage as f32, def.kind, now);
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
                let damage = self.dice.sum(count, sides) as f32;
                if save.success && !half {
                    self.dummies[i].damage(0.0, def.kind, now);
                    Outcome::Resisted
                } else {
                    let damage = if save.success {
                        (damage / 2.0).floor()
                    } else {
                        damage
                    };
                    let dealt = self.dummies[i].damage(damage, def.kind, now);
                    Outcome::Hit {
                        dealt,
                        crit: false,
                        saved: save.success,
                    }
                }
            }
            Delivery::Automatic => {
                let dealt =
                    self.dummies[i].damage(self.dice.sum(count, sides) as f32, def.kind, now);
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
                    self.say(format!(
                        "{} hits {name}: {dealt} {}{how}",
                        def.label,
                        def.kind.word()
                    ));
                }
            }
        }
        outcome
    }

    /// Wind Wall's damage and lift on every dummy the new wall stands in.
    fn wind_wall(&mut self, glade: &Everglade) {
        let Some(wall) = glade.spells().wind_wall().cloned() else {
            return;
        };
        for i in 0..self.dummies.len() {
            let d = &self.dummies[i];
            let scale = f64::from(d.kind.scale());
            if d.down() || !wall.in_area(d.pos.as_dvec3(), 0.35 * scale, 1.8 * scale) {
                continue;
            }
            if self.strike(Spell::WindWall, i).takes_hold() {
                self.dummies[i].lift(WIND_LIFT, self.time);
            }
        }
    }

    /// Wall of Stone's shove: a dummy where a panel rose moves out past
    /// the panel's far face, away from the caster at `from`.
    fn shove(&mut self, glade: &Everglade, from: Vec3) {
        let Some(panels) = glade.spells().stone_panels().map(<[_]>::to_vec) else {
            return;
        };
        let now = self.time;
        for i in 0..self.dummies.len() {
            let d = &self.dummies[i];
            let radius = 0.35 * f64::from(d.kind.scale());
            let center = d.pos.as_dvec3();
            let Some((normal, depth)) = panels.iter().find_map(|panel| {
                let half = panel.half();
                let local = panel.orientation.inverse() * (center - panel.center);
                (local.x.abs() <= half.x + radius && local.z.abs() <= half.z + radius).then(|| {
                    let normal = panel.normal();
                    let side = (center - from.as_dvec3()).dot(normal).signum();
                    let side = if side == 0.0 { 1.0 } else { side };
                    let past = half.z + radius + SHOVE_CLEARANCE - local.z * side;
                    (normal * side, past)
                })
            }) else {
                continue;
            };
            if self.dummies[i].push(normal.as_vec3(), depth as f32, now) {
                let name = self.dummies[i].kind.name();
                self.say(format!("Wall of Stone shoves {name} aside"));
            }
        }
    }

    /// Refills the druid and the field, and ends the glade's spells.
    fn long_rest(&mut self, player: &PlayerController, glade: &mut Everglade) {
        self.kit.rest();
        glade.long_rest();
        for dummy in &mut self.dummies {
            dummy.reset();
        }
        self.effects.clear();
        self.floaters.clear();
        self.effects.push(draw::Effect::Rest {
            at: player.pos,
            start: self.time,
        });
        self.say("Long Rest: mana, cooldowns, and dummies refilled".into());
    }

    fn float(&mut self, i: usize, text: String, color: [f32; 3]) {
        let d = &self.dummies[i];
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

    /// Advances the field `dt` seconds: bolts land, dummies move and reset,
    /// mana regenerates, and the dummies block walking where they stand.
    pub fn tick(&mut self, dt: f32, glade: &mut Everglade) {
        self.time += dt.max(0.0);
        let now = self.time;
        self.kit.tick(dt);
        let landed: Vec<(usize, bool)> = self
            .effects
            .iter()
            .filter_map(|effect| match *effect {
                draw::Effect::Bolt {
                    target,
                    start,
                    flight,
                    fireball,
                    ..
                } if now - start >= flight => Some((target, fireball)),
                _ => None,
            })
            .collect();
        self.effects.retain(|e| !e.done(now));
        for (target, fireball) in landed {
            if target >= self.dummies.len() {
                continue;
            }
            if fireball {
                let at = self.dummies[target].center();
                let radius = 20.0 * 0.3048;
                for i in 0..self.dummies.len() {
                    if self.dummies[i].center().distance(at) <= radius + 0.35 {
                        self.strike(Spell::Fireball, i);
                    }
                }
                self.effects.push(draw::Effect::Burst {
                    at,
                    radius,
                    start: now,
                });
            } else {
                self.strike(Spell::FireBolt, target);
            }
        }
        let gravity = glade.spells().reverse_gravity().cloned();
        for dummy in &mut self.dummies {
            dummy.tick(dt, now, gravity.as_ref());
        }
        self.floaters.retain(|f| now - f.start < draw::FLOAT);
        glade.set_extra_blocks(self.dummies.iter().map(Dummy::block).collect());
    }

    /// The player's character and the dummies, posed for this frame and lit
    /// by `glade`'s probes.
    #[must_use]
    pub fn figure(&self, glade: &Everglade) -> Figure {
        let mut figure = self
            .model
            .figure(glade.cast_figure().as_ref(), &self.dummies, self.time);
        let mut vertices = figure.vertices.as_ref().clone();
        glade.shade(&mut vertices);
        figure.vertices = Arc::new(vertices);
        figure
    }

    /// The bars, numbers, post, target ring, and effects, seen from `eye`
    /// by `player`.
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
            painter.bar(dummy, rooted, target == Some(i));
        }
        if let Some(i) = target {
            painter.target(&self.dummies[i], now);
        }
        for effect in &self.effects {
            let at = match effect {
                draw::Effect::Bolt { target, .. } => self.dummies.get(*target).map(Dummy::center),
                _ => None,
            };
            painter.effect(effect, now, at);
        }
        for floater in &self.floaters {
            painter.floater(floater, now);
        }
        painter.mesh
    }

    /// The hotbar: each slot's readiness and cooldown, and the mana.
    #[must_use]
    pub fn bar(&self, player: &PlayerController, glade: &Everglade) -> hotbar::Bar {
        let slots = Spell::ALL.map(|spell| {
            let active = spell.glade().is_some_and(|g| glade.spell_active(g));
            let targeted =
                !spell.needs_target() || self.target(player, spell.def().range).is_some();
            everglade::hotbar::Slot {
                enabled: active || (self.kit.admit(spell).is_ok() && targeted),
                active,
                cooldown: self.kit.cooldown_fraction(spell),
            }
        });
        hotbar::Bar {
            slots,
            mana: self.kit.mana,
        }
    }

    /// The zone caption: the soft target and the newest combat lines.
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
