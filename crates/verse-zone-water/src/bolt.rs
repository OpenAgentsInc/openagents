//! The Thunderbolt in the Water Lab: the Grove's own spell
//! (`verse_zone_grove::zones::grove::kit::Spell::Thunderbolt`, not SRD):
//! one huge bolt from the clouds, 12d10 lightning rolled once behind the
//! scenes, a Dexterity save against the Grove's spell save DC for half.
//! Its jagged stroke and branches, its flash, and its strike effect are the
//! Grove's (ported from `everglade::demolition::meteor`), and it lights the
//! cove for a moment.
//!
//! `7` strikes what the pointer aims at, else the nearest orb ahead, else
//! a point ahead. Where it lands decides what it hurts:
//!
//! - **An orb.** A bolt that meets a Water Orb, or comes down over one,
//!   strikes the orb, and the whole orb electrifies. Everything inside it
//!   takes the bolt's damage; nothing outside it does. A dummy inside is
//!   Restrained by the water (the SRD's Engulf), so it makes the save with
//!   Disadvantage, and takes half on a success. Floating bodies inside jolt.
//! - **The sea or the river.** The strike's own 3.4 m blast hurts what
//!   stands in it (a save for half), and the water conducts under
//!   `docs/verse/water.md`'s rule (ours): every dummy wading in the same
//!   body within [`CONDUCTION`] m of the contact point makes the save,
//!   taking half the damage on a failure and none on a success; ice
//!   insulates. Floating bodies there jolt.
//! - **The ground.** The blast alone.

use glam::{Vec2, Vec3};
use verse_core::fx::Spawn;
use verse_pbr::mesh::Mesh;
use verse_pbr::pbr::Lamp;
use verse_zone_grove::zones::grove::kit::{Ability, Damage, SAVE_DC, Spell};

use crate::orb::{glow_blob, line};
use crate::{WaterLab, terrain};

/// How far lightning conducts through a body of water, m: 20 feet, the
/// water specification's rule.
pub const CONDUCTION: f32 = 6.0;
/// How far ahead a bolt with nothing aimed strikes, m.
pub const REACH: f32 = 14.0;
/// How far an orb may stand to be struck with nothing aimed, m.
const SEEK: f32 = 40.0;
/// How long the leader takes to come down, how long the bolt stays lit,
/// and how high it leaves the clouds, s and m.
const LEAD: f32 = 0.09;
pub const LIFE: f32 = 0.55;
const HEIGHT: f32 = 60.0;
/// How long a surge runs over the water, s.
const SURGE: f32 = 0.5;
/// How long struck floating bodies jolt, s.
pub const JOLT: f32 = 1.4;
const CORE: [f32; 3] = [1.7, 1.8, 2.0];
const GLOW: [f32; 3] = [0.45, 0.6, 1.0];
const LUMINANCE: f32 = 90.0;

/// A bolt from `from` in the clouds to `to`, `age` s after it left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bolt {
    pub from: Vec3,
    pub to: Vec3,
    pub age: f32,
    seed: u32,
}

/// What a bolt strikes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    /// The orb at this index, at this point of its skin.
    Orb(usize, Vec3),
    /// Open water, at the contact point on its surface; `river` for the
    /// river or the pool, else the sea.
    Water {
        at: Vec3,
        river: bool,
    },
    Ground(Vec3),
}

impl Hit {
    #[must_use]
    pub fn point(self) -> Vec3 {
        match self {
            Self::Orb(_, at) | Self::Water { at, .. } | Self::Ground(at) => at,
        }
    }
}

impl Bolt {
    /// The jagged path from the clouds to the target, and its branches.
    fn paths(&self) -> Vec<Vec<Vec3>> {
        let (a, b) = (self.from, self.to);
        let along = (b - a).normalize_or(Vec3::NEG_Y);
        let side = along.cross(Vec3::X).normalize_or(Vec3::Z);
        let other = along.cross(side).normalize_or(Vec3::X);
        // A fresh jag every few hundredths of a second, so it flickers.
        let flicker = (self.age / 0.05) as u32;
        let seed = self.seed ^ flicker.wrapping_mul(0x9E37);
        let jag = |from: Vec3, to: Vec3, parts: u32, wide: f32, salt: u32| -> Vec<Vec3> {
            (0..=parts)
                .map(|i| {
                    let t = i as f32 / parts as f32;
                    let taper = (t * (1.0 - t) * 4.0).sqrt();
                    let x = noise(seed, salt + 2 * i) * 2.0 - 1.0;
                    let y = noise(seed, salt + 2 * i + 1) * 2.0 - 1.0;
                    from.lerp(to, t) + (side * x + other * y) * wide * taper
                })
                .collect()
        };
        let main = jag(a, b, 18, 3.2, 0);
        let mut paths = vec![main.clone()];
        for k in 0..5u32 {
            let i = 3 + (noise(seed, 100 + k) * 12.0) as usize;
            let from = main[i.min(main.len() - 2)];
            let reach = 6.0 + 10.0 * noise(seed, 110 + k);
            let turn = side * (noise(seed, 120 + k) * 2.0 - 1.0)
                + other * (noise(seed, 130 + k) * 2.0 - 1.0);
            let to = from + (along + turn.normalize_or(side)).normalize() * reach;
            paths.push(jag(from, to, 6, 1.2, 200 + 20 * k));
        }
        paths
    }

    /// How brightly it shines now, 0 to 1, flickering out.
    #[must_use]
    pub fn light(&self) -> f32 {
        let fade = 1.0 - (self.age / LIFE).clamp(0.0, 1.0);
        let flicker = if self.age < 0.12 {
            1.0
        } else {
            0.35 + 0.65 * noise(self.seed, (self.age / 0.03) as u32)
        };
        fade * fade * flicker
    }

    /// Its white core as lines and its blue halo as glow, the part above
    /// the leader only while it comes down, and a flash where it strikes.
    fn draw(&self, mesh: &mut Mesh, eye: Vec3) {
        let light = self.light();
        if light <= 0.0 {
            return;
        }
        let reach = (self.age / LEAD).clamp(0.0, 1.0);
        for (k, path) in self.paths().iter().enumerate() {
            let shown = if k == 0 {
                ((path.len() - 1) as f32 * reach).ceil() as usize
            } else if reach < 1.0 {
                0
            } else {
                path.len() - 1
            };
            let core = CORE.map(|c| c * (0.4 + 0.6 * light));
            let halo = if k == 0 { 1.6 } else { 0.7 };
            for pair in path.windows(2).take(shown) {
                let (a, b) = (pair[0], pair[1]);
                // A thick core: three strands side by side.
                let across = (b - a).cross(eye - a).normalize_or(Vec3::X) * 0.06;
                for o in [-1.0, 0.0, 1.0] {
                    if k > 0 && o != 0.0 {
                        continue;
                    }
                    line(&mut mesh.lines, a + across * o, b + across * o, core);
                }
                let spots = ((a.distance(b) / (halo * 1.1)).ceil() as usize).clamp(1, 12);
                for i in 0..spots {
                    let at = a.lerp(b, (i as f32 + 0.5) / spots as f32);
                    glow_blob(
                        &mut mesh.glow,
                        at,
                        halo,
                        GLOW,
                        LUMINANCE * 0.12 * light,
                        eye,
                    );
                }
            }
        }
        if reach >= 1.0 {
            let flash = (1.0 - self.age / 0.25).clamp(0.0, 1.0);
            glow_blob(
                &mut mesh.glow,
                self.to,
                1.5 + 3.0 * flash,
                [0.92, 0.95, 1.0],
                LUMINANCE * 0.8 * flash,
                eye,
            );
            glow_blob(&mut mesh.glow, self.from, 12.0, GLOW, 6.0 * light, eye);
        }
    }
}

/// A value in `0..1` for `n` in stream `seed`.
fn noise(seed: u32, n: u32) -> f32 {
    let mut h = seed ^ n.wrapping_mul(0x27d4_eb2d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    (h >> 8) as f32 / 16_777_216.0
}

impl WaterLab {
    /// Casts the Thunderbolt from `at` facing `forward`: at the pointer's
    /// aim, else the nearest orb ahead, else [`REACH`] m ahead.
    pub fn thunderbolt(&mut self, at: Vec3, forward: Vec3) -> String {
        let aimed = self
            .aim
            .and_then(|(origin, direction)| self.ray_hit(origin, direction, None));
        let (point, orb) = match aimed {
            Some(hit) => hit,
            None => {
                let flat = Vec2::new(forward.x, forward.z).normalize_or(Vec2::NEG_Y);
                let ahead = self
                    .orbs
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| {
                        let to = Vec2::new(o.center.x - at.x, o.center.z - at.z);
                        to.length() < SEEK && to.normalize_or_zero().dot(flat) > 0.7
                    })
                    .min_by(|a, b| a.1.center.distance(at).total_cmp(&b.1.center.distance(at)))
                    .map(|(i, o)| (o.center + Vec3::Y * o.radius, Some(i)));
                ahead.unwrap_or_else(|| {
                    let (x, z) = (at.x + flat.x * REACH, at.z + flat.y * REACH);
                    (Vec3::new(x, self.top_at(x, z), z), None)
                })
            }
        };
        self.strike(self.resolve(point, orb))
    }

    /// What a bolt aimed at `point` (on orb `orb`, if the aim met one)
    /// strikes: an orb it meets or comes down over, else open water, else
    /// the ground.
    #[must_use]
    pub fn resolve(&self, point: Vec3, orb: Option<usize>) -> Hit {
        if let Some(i) = orb.filter(|i| *i < self.orbs.len()) {
            return Hit::Orb(i, point);
        }
        // Coming down from the clouds, it meets the highest orb over the
        // point first.
        let over = self
            .orbs
            .iter()
            .enumerate()
            .filter_map(|(i, o)| {
                let h = Vec2::new(point.x - o.center.x, point.z - o.center.z).length();
                (h < o.radius * 0.95 && o.center.y > point.y - o.radius).then(|| {
                    let top = o.center.y + (o.radius * o.radius - h * h).max(0.0).sqrt();
                    (i, Vec3::new(point.x, top, point.z))
                })
            })
            .max_by(|a, b| a.1.y.total_cmp(&b.1.y));
        if let Some((i, at)) = over {
            return Hit::Orb(i, at);
        }
        let p = Vec2::new(point.x, point.z);
        if let Some(s) = self.surface_at(p)
            && point.y <= s.height + 0.6
        {
            return Hit::Water {
                at: Vec3::new(point.x, s.height, point.z),
                river: terrain::fresh_water(p).is_some(),
            };
        }
        Hit::Ground(point)
    }

    /// The bolt strikes `hit`: the damage, rolled once, lands by where it
    /// struck, and the stroke, flash, and light begin.
    pub fn strike(&mut self, hit: Hit) -> String {
        let def = Spell::Thunderbolt.def();
        let total = self.dice.sum(def.dice.0, def.dice.1) as f32 + def.bonus as f32;
        let to = hit.point();
        let seed = (self.random() * 16_777_216.0) as u32;
        let from = to
            + Vec3::new(
                self.random() * 10.0 - 5.0,
                HEIGHT,
                self.random() * 10.0 - 5.0,
            );
        self.bolts.push(Bolt {
            from,
            to,
            age: 0.0,
            seed,
        });
        if self.bolts.len() > 6 {
            self.bolts.remove(0);
        }
        // The Grove's strike, with its grit and smoke, on the ground and the
        // water; on an orb, sparks alone.
        let effect = if matches!(hit, Hit::Orb(..)) {
            "grove_lightning"
        } else {
            "thunderbolt_strike"
        };
        self.fx.start(effect, Spawn::at(to));
        self.flash = 1.0;
        let line = match hit {
            Hit::Orb(i, _) => self.electrify(i, total),
            Hit::Water { at, river } => {
                let direct = self.blast(at, total);
                self.water.add_ripple(Vec2::new(at.x, at.z), 0.05);
                self.fx
                    .start("water_steam", Spawn::at(at + Vec3::Y * 0.2).scaled(0.8));
                let conducted = self.conduct(at, river, total, &direct);
                let body = if river { "river" } else { "sea" };
                format!(
                    "Thunderbolt strikes the {body}: {} in the blast, {} through the water",
                    direct.len(),
                    conducted
                )
            }
            Hit::Ground(at) => {
                let direct = self.blast(at, total);
                format!("Thunderbolt: {} in the blast", direct.len())
            }
        };
        self.say(line)
    }

    /// Dummy `k`'s Dexterity save against the spell save DC, with
    /// Disadvantage when water restrains it.
    fn save(&mut self, k: usize, disadvantage: bool) -> bool {
        let modifier = self.targets[k].dummy.kind.save(Ability::Dexterity);
        let first = self.dice.save(k as u64, "Dexterity", modifier, SAVE_DC);
        if disadvantage {
            let second = self.dice.save(k as u64, "Dexterity", modifier, SAVE_DC);
            first.success && second.success
        } else {
            first.success
        }
    }

    /// Deals `amount` lightning to dummy `k` and floats the number.
    fn shock(&mut self, k: usize, amount: f32) -> i32 {
        let now = self.time;
        let target = &mut self.targets[k];
        target.touched = now;
        let dealt = target.dummy.damage(amount, Damage::Lightning, now);
        let (top, feet) = (target.dummy.top(), target.dummy.pos);
        let text = if dealt > 0 {
            dealt.to_string()
        } else {
            "Saved".into()
        };
        self.float(crate::targets::floater(
            top,
            feet,
            &text,
            Damage::Lightning.color(),
            now,
        ));
        dealt
    }

    /// The bolt's own blast at `at`: every dummy within it not in an orb
    /// saves for half, and floating bodies are thrown. Returns the dummies
    /// it reached.
    fn blast(&mut self, at: Vec3, total: f32) -> Vec<usize> {
        let radius = crate::bolt::blast_radius();
        let held = self.held_dummies();
        let reached: Vec<usize> = (0..self.targets.len())
            .filter(|k| !held.contains(k))
            .filter(|k| {
                let d = &self.targets[*k].dummy;
                let near = Vec2::new(d.pos.x - at.x, d.pos.z - at.z).length();
                near < radius && at.y > d.pos.y - 1.0 && at.y < d.top() + 1.5
            })
            .collect();
        for &k in &reached {
            let amount = if self.save(k, false) {
                (total / 2.0).floor()
            } else {
                total
            };
            self.shock(k, amount);
        }
        self.floats.blast(at, radius * 1.6, 7.0);
        reached
    }

    /// Lightning at `at` on the river (`river`) or the sea conducts to the
    /// dummies wading in the same water within [`CONDUCTION`] m, other
    /// than those in `direct`: each saves, taking half on a failure and
    /// none on a success. Ice insulates. Returns how many it reached.
    fn conduct(&mut self, at: Vec3, river: bool, total: f32, direct: &[usize]) -> usize {
        let p = Vec2::new(at.x, at.z);
        let water = self.frame_water();
        if water.controls.ice_at(p) > 0.5 {
            return 0;
        }
        self.surges.push((at, self.time));
        let same = |q: Vec2| terrain::fresh_water(q).is_some() == river;
        let held = self.held_dummies();
        let reached: Vec<usize> = (0..self.targets.len())
            .filter(|k| !direct.contains(k) && !held.contains(k))
            .filter(|k| {
                let d = &self.targets[*k].dummy;
                let q = Vec2::new(d.pos.x, d.pos.z);
                q.distance(p) <= CONDUCTION
                    && same(q)
                    && water.controls.ice_at(q) < 0.5
                    && self
                        .surface_at(q)
                        .is_some_and(|s| d.pos.y < s.height - 0.05)
            })
            .collect();
        for &k in &reached {
            let amount = if self.save(k, false) {
                0.0
            } else {
                (total / 2.0).floor()
            };
            self.shock(k, amount);
        }
        // Floating bodies in the same water jolt.
        let until = self.time + JOLT;
        let ids: Vec<_> = self
            .floats
            .floats
            .iter()
            .filter(|f| self.floats.held(f.id).is_none())
            .filter_map(|f| {
                let b = &self.floats.world.bodies()[f.id.0 as usize];
                let q = Vec2::new(b.pos.x as f32, b.pos.z as f32);
                (q.distance(p) <= CONDUCTION && same(q) && f.wet).then_some(f.id)
            })
            .collect();
        for id in ids {
            self.jolts.push((id, until));
        }
        reached.len()
    }

    /// The bolt electrifies orb `i`: everything inside takes the bolt,
    /// each dummy saving with Disadvantage for half, every floating body
    /// jolting, and nothing outside it touched.
    fn electrify(&mut self, i: usize, total: f32) -> String {
        let now = self.time;
        let orb = &mut self.orbs[i];
        orb.charge = 1.0;
        orb.wobble = 1.0;
        let (center, radius) = (orb.center, orb.radius);
        let dummies = orb.dummies.clone();
        let floats = orb.floats.clone();
        self.fx.start(
            "water_mist",
            Spawn::at(center + Vec3::Y * radius * 0.9).scaled(0.4 + 0.15 * radius),
        );
        self.fx.start(
            "water_splash",
            Spawn::at(center + Vec3::Y * radius * 0.9).scaled(0.5 + 0.2 * radius),
        );
        let mut dealt = Vec::new();
        for k in dummies {
            let amount = if self.save(k, true) {
                (total / 2.0).floor()
            } else {
                total
            };
            dealt.push(self.shock(k, amount));
        }
        for id in &floats {
            self.jolts.push((*id, now + crate::orb::CHARGE_TIME));
        }
        let size = radius * 2.0;
        if dealt.is_empty() && floats.is_empty() {
            format!("Thunderbolt: the {size:.1} m orb electrifies")
        } else {
            let numbers: Vec<String> = dealt.iter().map(ToString::to_string).collect();
            let things = dealt.len() + floats.len();
            if numbers.is_empty() {
                format!("Thunderbolt: the {size:.1} m orb electrifies {things} inside")
            } else {
                format!(
                    "Thunderbolt: the {size:.1} m orb electrifies {things} inside: {} lightning",
                    numbers.join(", ")
                )
            }
        }
    }

    /// The dummies an orb holds.
    fn held_dummies(&self) -> Vec<usize> {
        self.orbs
            .iter()
            .flat_map(|o| o.dummies.iter().copied())
            .collect()
    }

    /// Ages the bolts and the surges.
    pub(crate) fn tick_bolts(&mut self, dt: f32) {
        for bolt in &mut self.bolts {
            bolt.age += dt;
        }
        self.bolts.retain(|b| b.age < LIFE);
        let now = self.time;
        self.surges.retain(|(_, t)| now - t < SURGE);
        self.flash = (self.flash - dt * 3.0).max(0.0);
    }

    /// The bolts, and the surges running over the water, seen from `eye`.
    pub(crate) fn bolt_marks(&self, mesh: &mut Mesh, eye: Vec3) {
        for bolt in &self.bolts {
            bolt.draw(mesh, eye);
        }
        for (k, (at, start)) in self.surges.iter().enumerate() {
            let age = self.time - start;
            let reach = CONDUCTION * (age / 0.15).min(1.0);
            let fade = 1.0 - age / SURGE;
            let color = [0.8 * fade + 0.4, 0.9 * fade + 0.4, 1.6];
            let flick = (self.time * 30.0) as u32;
            for arm in 0..9u32 {
                let a =
                    arm as f32 / 9.0 * std::f32::consts::TAU + noise(k as u32 + flick, arm) * 0.6;
                let mut last = *at + Vec3::Y * 0.05;
                for s in 1..=6u32 {
                    let r = reach * s as f32 / 6.0;
                    let wiggle = (noise(flick ^ arm, s) - 0.5) * 0.8;
                    let x = at.x + (a + wiggle * 0.3).cos() * r;
                    let z = at.z + (a + wiggle * 0.3).sin() * r;
                    let y = self.top_at(x, z) + 0.05;
                    let p = Vec3::new(x, y, z);
                    line(&mut mesh.lines, last, p, color);
                    last = p;
                }
            }
            glow_blob(
                &mut mesh.glow,
                *at + Vec3::Y * 0.3,
                2.5,
                GLOW,
                12.0 * fade,
                eye,
            );
        }
    }

    /// The lights lightning makes: each bolt at its strike, each charged
    /// orb at its heart.
    #[must_use]
    pub fn lamps(&self) -> Vec<Lamp> {
        let mut out = Vec::new();
        for bolt in &self.bolts {
            out.push(Lamp {
                position: bolt.to + Vec3::Y * 2.0,
                color: [0.72, 0.82, 1.0],
                intensity: 160_000.0 * bolt.light(),
                range: 30.0,
            });
        }
        for orb in &self.orbs {
            if orb.charge > 0.0 {
                let flick = 0.6 + 0.4 * noise(orb.id, (self.time * 25.0) as u32);
                out.push(Lamp {
                    position: orb.center,
                    color: [0.5, 0.7, 1.0],
                    intensity: 20_000.0 * orb.charge * flick * (0.5 + orb.radius * 0.5),
                    range: orb.radius * 3.0 + 8.0,
                });
            }
        }
        out
    }
}

/// The Thunderbolt's blast radius, m: the Grove's.
#[must_use]
pub fn blast_radius() -> f32 {
    match Spell::Thunderbolt.def().area {
        verse_zone_grove::zones::grove::kit::Area::Burst(r) => r,
        _ => 3.4,
    }
}
