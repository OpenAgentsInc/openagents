//! The Grove's lasting areas: spells that stay where they were cast and act
//! over time, from Moonbeam's column to Storm of Vengeance.
//!
//! An aura is a zone (a radius at a point) or a wall (a line across the
//! druid's facing). It acts when it rises, then each second, each 3 or 6
//! seconds, or as a dummy is moved through it, as its spell says, and ends
//! after its duration. A concentration aura ends the druid's other
//! concentration auras, as the combat model's one maintained effect does;
//! casting the same one again moves it. Each aura runs continuous
//! particles that stop when it ends, and the field keeps at most
//! [`MAX_AURAS`], oldest dropped first.

use super::kit::{Area, Damage, Def, Delivery, FT};
use super::*;
use crate::fx::{Handle, Spawn};

/// Lasting areas at once.
pub const MAX_AURAS: usize = 12;
/// How far a dummy must be moved through thorns to be hurt again, m.
const STRIDE: f32 = 5.0 * FT;
/// A wall's half thickness, m: the 1-foot wall and a dummy's body.
const WALL_HALF: f32 = 1.0;
/// Particle effects along a wall, one each this many meters.
const WALL_STEP: f32 = 2.0;

/// One lasting area.
#[derive(Clone, Debug, PartialEq)]
pub struct Aura {
    pub spell: Spell,
    /// Its center on the ground, m.
    pub center: Vec3,
    /// Along a wall, horizontal and unit length.
    pub across: Vec3,
    /// A zone's radius, or half a wall's length, m.
    pub reach: f32,
    pub wall: bool,
    pub start: f32,
    pub until: f32,
    /// When it next acts, s.
    next: f32,
    /// Storm of Vengeance's round.
    round: u32,
    /// Each dummy's distance moved through it since it last hurt, m, and
    /// where it last stood, for thorns.
    moved: Vec<(f32, Vec3)>,
    fx: Vec<Handle>,
}

impl Aura {
    /// Whether dummy at `at` (its feet) stands inside it.
    #[must_use]
    pub fn contains(&self, at: Vec3) -> bool {
        // Most areas hug the ground; Moonbeam's column is 40 feet tall,
        // and the storms reach the sky.
        let tall = match self.spell {
            Spell::Moonbeam => 40.0 * FT,
            Spell::CallLightning | Spell::StormOfVengeance => 30.0,
            _ => 4.0,
        };
        if at.y - self.center.y > tall {
            return false;
        }
        let off = Vec3::new(at.x - self.center.x, 0.0, at.z - self.center.z);
        if self.wall {
            let along = off.dot(self.across);
            let normal = (off - self.across * along).length();
            along.abs() <= self.reach + 0.3 && normal <= WALL_HALF
        } else {
            off.length() <= self.reach + 0.35
        }
    }

    /// Its duration, s.
    const fn length(spell: Spell) -> f32 {
        match spell {
            Spell::Entangle => 6.0,
            Spell::CallLightning | Spell::StormOfVengeance => 30.0,
            _ => 10.0,
        }
    }

    /// How often it acts, s; zero acts only as dummies move through it.
    const fn period(spell: Spell) -> f32 {
        match spell {
            Spell::CallLightning | Spell::StormOfVengeance => 6.0,
            Spell::SleetStorm => 3.0,
            Spell::SpikeGrowth | Spell::WallOfThorns | Spell::Entangle => 0.0,
            _ => 1.0,
        }
    }
}

/// The continuous particles an aura runs, and how high above its center.
fn looks(spell: Spell) -> &'static [(&'static str, f32)] {
    match spell {
        Spell::Moonbeam => &[("grove_moonbeam", 0.0)],
        Spell::SpikeGrowth => &[("grove_thorns", 0.0)],
        Spell::CallLightning => &[("grove_storm_cloud", 10.0)],
        Spell::StormOfVengeance => &[("grove_storm_cloud", 10.0), ("grove_sleet", 8.0)],
        Spell::Entangle => &[("grove_vines", 0.0)],
        Spell::FogCloud => &[("grove_fog", 0.0)],
        Spell::SleetStorm => &[("grove_sleet", 8.0)],
        Spell::StinkingCloud => &[("grove_stink_cloud", 0.0)],
        Spell::InsectPlague => &[("grove_swarm", 0.0)],
        _ => &[],
    }
}

impl Grove {
    /// Raises `spell`'s lasting area at `point`, across `forward` for a
    /// wall, and acts once as it rises.
    pub(super) fn raise(&mut self, spell: Spell, point: Vec3, forward: Vec3) {
        let def = spell.def();
        let now = self.time;
        // Call Lightning cast again under its storm calls a bolt there.
        if spell == Spell::CallLightning
            && let Some(storm) = self.auras.iter().position(|a| a.spell == spell)
        {
            let reach = self.auras[storm].reach;
            if self.auras[storm].center.distance(point) <= reach {
                self.lightning(point);
                return;
            }
        }
        if def.concentration {
            self.end_auras(|a| a.spell.def().concentration);
        }
        while self.auras.len() >= MAX_AURAS {
            self.end_aura(0);
        }
        let (reach, wall) = match def.area {
            Area::Wall(length) => (length / 2.0, true),
            Area::Zone(radius) => (radius, false),
            _ => (3.0, false),
        };
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::Z);
        let across = Vec3::new(-flat.z, 0.0, flat.x);
        let mut fx = Vec::new();
        if wall {
            // Fire burns from the ground; the brambles stand about the
            // height of a dummy's chest.
            let (name, lift) = if spell == Spell::WallOfFire {
                ("grove_fire_wall", 0.0)
            } else {
                ("grove_thorn_wall", 0.9)
            };
            let steps = (reach * 2.0 / WALL_STEP).round() as i32;
            for k in 0..=steps {
                let s = -reach + k as f32 * WALL_STEP;
                let mut at = point + across * s;
                at.y = everglade::height(at.x, at.z) + lift;
                fx.extend(self.fx.start(name, Spawn::at(at)));
            }
        } else {
            for &(name, height) in looks(spell) {
                fx.extend(self.fx.start(name, Spawn::at(point + Vec3::Y * height)));
            }
        }
        let period = Aura::period(spell);
        let aura = Aura {
            spell,
            center: point,
            across,
            reach,
            wall,
            start: now,
            until: now + Aura::length(spell),
            next: now + period,
            round: 0,
            moved: self.dummies.iter().map(|d| (0.0, d.pos)).collect(),
            fx,
        };
        self.auras.push(aura);
        let index = self.auras.len() - 1;
        let inside = self.inside(index);
        match spell {
            Spell::CallLightning => self.lightning(point),
            Spell::StormOfVengeance => {
                for i in inside {
                    self.strike(spell, i);
                }
                self.decal(point, reach);
                self.say("Storm of Vengeance: thunder rolls over the field".into());
            }
            // Walls rise through what stands in them; zones that act each
            // second act now; thorns wait for movement.
            Spell::WallOfFire | Spell::WallOfThorns | Spell::Entangle => {
                for i in inside {
                    self.strike(spell, i);
                }
            }
            Spell::SpikeGrowth => {}
            _ => self.act(index),
        }
    }

    /// The standing dummies inside aura `index`.
    fn inside(&self, index: usize) -> Vec<usize> {
        let aura = &self.auras[index];
        (0..self.dummies.len())
            .filter(|&i| !self.dummies[i].down() && aura.contains(self.dummies[i].pos))
            .collect()
    }

    /// One action of aura `index`: its tick damage, condition, or round.
    fn act(&mut self, index: usize) {
        let spell = self.auras[index].spell;
        let inside = self.inside(index);
        match spell {
            Spell::CallLightning => {
                // The bolt finds the dummy nearest the storm's heart.
                let center = self.auras[index].center;
                let nearest = inside.into_iter().min_by(|&a, &b| {
                    let da = self.dummies[a].pos.distance(center);
                    let db = self.dummies[b].pos.distance(center);
                    da.total_cmp(&db)
                });
                if let Some(i) = nearest {
                    let at = self.dummies[i].pos;
                    self.lightning(at);
                }
            }
            Spell::StormOfVengeance => self.storm_round(index, &inside),
            Spell::WallOfFire => {
                // Inside the wall, 5d8 a second with no save.
                let def = Def {
                    delivery: Delivery::Automatic,
                    ..spell.def()
                };
                for i in inside {
                    self.strike_def(&def, i);
                }
            }
            _ => {
                for i in inside {
                    self.strike(spell, i);
                }
            }
        }
    }

    /// A bolt of Call Lightning at `at`: 3d10 lightning within 5 feet, half
    /// on a Dexterity save.
    fn lightning(&mut self, at: Vec3) {
        let now = self.time;
        let sky = at + Vec3::Y * 10.0;
        self.add(draw::Effect::Strike {
            from: sky,
            to: at,
            start: now,
        });
        self.burst("grove_lightning", at + Vec3::Y * 0.2);
        for i in self.within(at + Vec3::Y * 0.9, 5.0 * FT) {
            self.strike(Spell::CallLightning, i);
        }
    }

    /// Storm of Vengeance's next round over the dummies `inside`: acid
    /// rain, then lightning, then hail.
    fn storm_round(&mut self, index: usize, inside: &[usize]) {
        let aura = &mut self.auras[index];
        aura.round += 1;
        let round = aura.round;
        let center = aura.center;
        let base = Spell::StormOfVengeance.def();
        let (def, what) = match round {
            1 => (
                Def {
                    dice: (4, 6),
                    kind: Damage::Acid,
                    delivery: Delivery::Automatic,
                    ..base
                },
                "acid rain falls",
            ),
            2 => (
                Def {
                    dice: (10, 6),
                    kind: Damage::Lightning,
                    delivery: Delivery::Save {
                        ability: kit::Ability::Dexterity,
                        half: true,
                    },
                    ..base
                },
                "lightning strikes",
            ),
            _ => (
                Def {
                    dice: (2, 6),
                    kind: Damage::Bludgeoning,
                    delivery: Delivery::Automatic,
                    ..base
                },
                "hail pounds the field",
            ),
        };
        self.say(format!("Storm of Vengeance: {what}"));
        for (k, &i) in inside.iter().enumerate() {
            // Six bolts at most, as the SRD's storm calls.
            if round == 2 && k >= 6 {
                break;
            }
            if round == 2 {
                let at = self.dummies[i].pos;
                self.add(draw::Effect::Strike {
                    from: at + Vec3::Y * 10.0,
                    to: at,
                    start: self.time,
                });
            }
            self.strike_def(&def, i);
        }
        if round == 1 {
            self.burst("grove_acid", center + Vec3::Y * 0.5);
        } else if round >= 3 {
            self.burst("grove_hail", center + Vec3::Y * 8.0);
        }
    }

    /// Ends every aura `which` picks, stopping its particles.
    pub(super) fn end_auras(&mut self, which: impl Fn(&Aura) -> bool) {
        let mut i = 0;
        while i < self.auras.len() {
            if which(&self.auras[i]) {
                self.end_aura(i);
            } else {
                i += 1;
            }
        }
    }

    fn end_aura(&mut self, index: usize) {
        let aura = self.auras.remove(index);
        for handle in aura.fx {
            self.fx.stop(handle);
        }
    }

    /// Acts each aura whose time has come, hurts dummies moved through
    /// thorns, and ends the auras whose time is up.
    pub(super) fn tick_auras(&mut self) {
        let now = self.time;
        self.end_auras(|a| now >= a.until);
        for index in 0..self.auras.len() {
            let spell = self.auras[index].spell;
            let period = Aura::period(spell);
            if period > 0.0 && now >= self.auras[index].next {
                self.auras[index].next = now + period;
                self.act(index);
            }
            if matches!(spell, Spell::SpikeGrowth | Spell::WallOfThorns) {
                self.thorns(index);
            }
        }
    }

    /// Thorns: every 5 feet a dummy is moved inside the aura costs it the
    /// spell's damage, with no save.
    fn thorns(&mut self, index: usize) {
        let spell = self.auras[index].spell;
        let def = Def {
            delivery: Delivery::Automatic,
            ..spell.def()
        };
        let mut hurt = Vec::new();
        {
            let aura = &mut self.auras[index];
            if aura.moved.len() != self.dummies.len() {
                aura.moved = self.dummies.iter().map(|d| (0.0, d.pos)).collect();
            }
            for (i, d) in self.dummies.iter().enumerate() {
                let inside = aura.contains(d.pos);
                let (walked, last) = &mut aura.moved[i];
                let step = Vec3::new(d.pos.x - last.x, 0.0, d.pos.z - last.z).length();
                *last = d.pos;
                if d.down() || !inside {
                    *walked = 0.0;
                    continue;
                }
                *walked += step;
                while *walked >= STRIDE {
                    *walked -= STRIDE;
                    hurt.push(i);
                }
            }
        }
        for i in hurt {
            self.strike_def(&def, i);
        }
    }
}
