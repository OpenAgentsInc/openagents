//! Meteor Swarm in the demolition yard: the SRD's level 9 Evocation
//! (`verse_world::meteor_swarm`) aimed at a circle of ground and tuned
//! for buildings.
//!
//! `2` enters targeting: a pulsing ring of fire on the ground under the
//! cursor, as wide as the strike, never farther than [`RANGE`] from the
//! player. A click confirms it, and right click, `Esc`, or `2` again
//! cancels without spending anything. The cast takes [`CAST`] seconds, as
//! the combat model's level 9 tier does (`docs/verse/combat-model.md`);
//! moving or jumping interrupts it, and an interrupted cast spends
//! nothing. When it completes it spends [`COST`] mana and starts the
//! [`COOLDOWN`], and [`METEORS`] meteors (the SRD's four and two more for
//! the spectacle) fall one after another on points spread through the
//! circle, slanting in from the caster's side as the SRD solver's do.
//!
//! Each meteor detonates on the first piece it reaches, or on the ground.
//! Its explosion deals the spell's 20d6 Fire and 20d6 Bludgeoning, rolled
//! once per cast behind the scenes, to every piece within [`BLAST`] m,
//! falling off from all of it at the center to none at the edge, so it
//! breaks plaster outright near the center and cracks what it barely
//! reaches; the SRD's objects take damage once, but every explosion here
//! counts, a zone's tuning for a yard of buildings. What breaks flies
//! outward and up, and what loses its support falls through the site's
//! support graph. The explosion flashes, swells into a fireball, sends a
//! shockwave ring along the ground, throws sparks and dust, shakes the
//! camera, and leaves a scorch mark that fades.

use super::site::{Blow, Site};
use crate::controller::PlayerController;
use crate::mesh::{Mesh, Vertex};
use crate::pbr::GlowVertex;
use crate::zones::everglade::draw::shade;
use crate::zones::everglade::height;
use glam::Vec3;
use std::f32::consts::TAU;
use verse_world::meteor_swarm::{self as srd, Damage};

/// The mana pool the yard's caster has.
pub const MAX_MANA: f32 = 100.0;
/// Mana a cast spends, s of cooldown, and s of cast bar. The yard is a
/// playground, so casts are free and can follow each other at once; the
/// cast bar stays.
pub const COST: f32 = 0.0;
pub const COOLDOWN: f32 = 0.0;
pub const CAST: f32 = 2.5;
/// Mana regained a second once [`REST`] seconds pass without a cast.
pub const REGEN: f32 = 4.0;
pub const REST: f32 = 5.0;
/// Farthest the circle's center may be from the player, m. The SRD's
/// mile is far past the yard's trees.
pub const RANGE: f32 = 36.0;
/// The circle's radius, m: the strike's whole footprint.
pub const AREA: f32 = 6.0;
/// Each explosion's radius, m.
pub const BLAST: f32 = 4.0;
/// How many meteors fall.
pub const METEORS: usize = 6;
/// Fastest an explosion throws debris, m/s, at its center.
pub const THROW: f32 = 17.0;
/// The slot's one-sentence description, for the hotbar's tooltip.
pub const TOOLTIP: &str = "Calls down six blazing meteors on a circle of ground you choose, \
    blasting apart every wall and roof they reach.";

/// How far the caster may move before the cast is interrupted, m.
const DRIFT: f32 = 0.2;
/// Height a meteor falls from, s it takes, and s between meteors.
const HEIGHT: f32 = 30.0;
const FALL: f32 = 1.0;
const STAGGER: f32 = 0.13;
/// How far the meteors' heading turns from straight away from the caster,
/// radians.
const SHOULDER: f32 = 0.6;
/// A meteor's radius, m.
const METEOR_RADIUS: f32 = 0.55;
/// How long an explosion's fire lasts, and a scorch mark, s.
const BLAST_LIFE: f32 = 1.6;
const SCORCH_LIFE: f32 = 16.0;
/// Most smoke puffs alive at once.
const MAX_SMOKE: usize = 360;
/// Most sparks and scorch marks alive at once.
const MAX_SPARKS: usize = 700;
const MAX_SCORCHES: usize = 18;
/// How long the camera shakes after a blast, at most, s.
const SHAKE: f32 = 1.0;

/// Fire's colors, from white heat to a dull red, and the targeting
/// ring's.
const WHITE_HOT: [f32; 3] = [1.0, 0.92, 0.7];
const YELLOW: [f32; 3] = [1.0, 0.68, 0.22];
const ORANGE: [f32; 3] = [1.0, 0.4, 0.08];
const RED: [f32; 3] = [0.85, 0.16, 0.03];
const RING: [f32; 3] = [1.0, 0.45, 0.1];
const ROCK: [f32; 3] = [0.16, 0.07, 0.04];
const CHAR: [f32; 3] = [0.03, 0.025, 0.022];
const SMOKE: [f32; 3] = [0.16, 0.15, 0.14];
/// Luminance of each glow, cd/m² before exposure.
const RING_LUMINANCE: f32 = 7.0;
const FIRE_LUMINANCE: f32 = 40.0;
const FLASH_LUMINANCE: f32 = 160.0;

/// A cast under way.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Casting {
    at: Vec3,
    /// Where the player stood when it began.
    from: Vec3,
    elapsed: f32,
}

/// One meteor: from where it falls, where it lands, and how long it has
/// been falling (below zero while it waits its turn).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Meteor {
    start: Vec3,
    end: Vec3,
    t: f32,
}

impl Meteor {
    fn at(&self, t: f32) -> Vec3 {
        // It speeds up as it falls.
        let x = (t / FALL).clamp(0.0, 1.0);
        self.start + (self.end - self.start) * (0.55 * x + 0.45 * x * x)
    }
}

/// One explosion's fire.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Burst {
    at: Vec3,
    age: f32,
    seed: u32,
}

/// A spark or a fleck of burning debris.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Spark {
    at: Vec3,
    vel: Vec3,
    age: f32,
    life: f32,
    size: f32,
}

/// A scorch mark on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Scorch {
    at: Vec3,
    radius: f32,
    age: f32,
    seed: u32,
}

/// What the hotbar shows of the spell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Status {
    pub mana: f32,
    /// Whether a cast can start now.
    pub ready: bool,
    pub targeting: bool,
    /// The cast bar's fill, from 0 to 1, while casting.
    pub casting: Option<f32>,
    /// The fraction of the cooldown left.
    pub cooldown: f32,
}

/// The spell's state, its meteors in flight, and what they left behind.
pub struct Swarm {
    mana: f32,
    /// Seconds of cooldown left, and since the last cast.
    cooldown: f32,
    idle: f32,
    targeting: bool,
    /// The circle's center while targeting, once the cursor has found
    /// ground.
    aim: Option<Vec3>,
    casting: Option<Casting>,
    meteors: Vec<Meteor>,
    damage: Damage,
    bursts: Vec<Burst>,
    sparks: Vec<Spark>,
    /// Smoke from the meteors' trails and the explosions: puffs that
    /// rise, swell, and thin.
    smoke: Vec<Spark>,
    scorches: Vec<Scorch>,
    shake: f32,
    clock: f32,
    rng: u32,
}

impl Default for Swarm {
    fn default() -> Self {
        Self {
            mana: MAX_MANA,
            cooldown: 0.0,
            idle: REST,
            targeting: false,
            aim: None,
            casting: None,
            meteors: Vec::new(),
            damage: Damage {
                fire: 0,
                bludgeoning: 0,
            },
            bursts: Vec::new(),
            sparks: Vec::new(),
            smoke: Vec::new(),
            scorches: Vec::new(),
            shake: 0.0,
            clock: 0.0,
            rng: 0x3E7E_0125,
        }
    }
}

impl Swarm {
    /// Refills the mana, clears the cooldown, and ends every cast, meteor,
    /// and mark, as the yard's rebuild does.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Enters targeting, or leaves it when already in it.
    ///
    /// # Errors
    ///
    /// Returns why the spell can't be cast now.
    pub fn target(&mut self) -> Result<(), String> {
        if self.targeting {
            self.cancel();
            return Ok(());
        }
        if self.casting.is_some() {
            return Err("Already casting Meteor Swarm".into());
        }
        if self.cooldown > 0.0 {
            return Err(format!(
                "Meteor Swarm is ready in {} s",
                self.cooldown.ceil() as i32
            ));
        }
        if self.mana < COST {
            return Err(format!(
                "Meteor Swarm needs {} mana, you have {}",
                COST as i32,
                self.mana.floor() as i32
            ));
        }
        self.targeting = true;
        Ok(())
    }

    /// Whether the circle follows the cursor.
    #[must_use]
    pub fn targeting(&self) -> bool {
        self.targeting
    }

    /// Whether a cast is under way.
    #[must_use]
    pub fn casting(&self) -> bool {
        self.casting.is_some()
    }

    /// The circle's center while targeting.
    #[must_use]
    pub fn aim(&self) -> Option<Vec3> {
        self.aim.filter(|_| self.targeting)
    }

    /// Puts the circle at `ground`, drawn in no farther than [`RANGE`]
    /// from `player`, on the ground.
    pub fn aim_at(&mut self, ground: Vec3, player: &PlayerController) {
        if !self.targeting || !ground.is_finite() {
            return;
        }
        self.aim = Some(clamp_to_range(ground, player.pos));
    }

    /// Starts the cast at the circle. Returns whether it started.
    pub fn confirm(&mut self, player: &PlayerController) -> bool {
        let Some(at) = self.aim() else {
            return false;
        };
        self.targeting = false;
        self.aim = None;
        self.casting = Some(Casting {
            at,
            from: player.pos,
            elapsed: 0.0,
        });
        true
    }

    /// Leaves targeting, or stops the cast, spending nothing.
    pub fn cancel(&mut self) {
        self.targeting = false;
        self.aim = None;
        self.casting = None;
    }

    #[must_use]
    pub fn status(&self) -> Status {
        Status {
            mana: self.mana,
            ready: self.cooldown <= 0.0 && self.mana >= COST && self.casting.is_none(),
            targeting: self.targeting,
            casting: self.casting.map(|c| (c.elapsed / CAST).clamp(0.0, 1.0)),
            cooldown: if COOLDOWN > 0.0 {
                (self.cooldown / COOLDOWN).clamp(0.0, 1.0)
            } else {
                0.0
            },
        }
    }

    /// The camera's offset this frame while a blast shakes it, m.
    #[must_use]
    pub fn shake(&self) -> Vec3 {
        let s = self.shake * self.shake * 0.28;
        let t = self.clock;
        Vec3::new(
            (t * 47.0).sin() + 0.5 * (t * 83.0).sin(),
            (t * 53.0 + 1.0).sin(),
            (t * 61.0 + 2.0).sin() + 0.5 * (t * 97.0).sin(),
        ) * s
    }

    /// Advances the cast, the meteors, and their fire for `player`, and
    /// explodes what lands on `site`. Returns each explosion's blows.
    pub fn tick(&mut self, dt: f32, player: &PlayerController, site: &mut Site) -> Vec<Blow> {
        self.clock += dt;
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.idle += dt;
        if self.idle >= REST && self.casting.is_none() {
            self.mana = (self.mana + REGEN * dt).min(MAX_MANA);
        }
        self.shake = (self.shake - dt / SHAKE).max(0.0);
        if let Some(cast) = &mut self.casting {
            let moved = Vec3::new(player.pos.x - cast.from.x, 0.0, player.pos.z - cast.from.z);
            if moved.length() > DRIFT || player.airborne() {
                self.casting = None;
            } else {
                cast.elapsed += dt;
                if cast.elapsed >= CAST {
                    let (at, from) = (cast.at, cast.from);
                    self.casting = None;
                    self.release(at, from, site);
                }
            }
        }
        // Sparks the casting hands throw up, and a meteor's embers.
        if let Some(cast) = self.casting {
            let k = cast.elapsed / CAST;
            for _ in 0..2 {
                let angle = self.unit() * TAU;
                let at = player.pos
                    + Vec3::new(
                        angle.cos() * 0.6,
                        1.0 + self.unit() * 0.8,
                        angle.sin() * 0.6,
                    );
                let vel = Vec3::new(-angle.sin(), 1.2 + 1.5 * k, angle.cos()) * 1.2;
                let life = 0.5 + 0.4 * self.unit();
                self.spark(at, vel, life, 0.05 + 0.07 * k);
            }
        }
        let mut blows = Vec::new();
        let mut landed = Vec::new();
        for (index, meteor) in self.meteors.iter_mut().enumerate() {
            let before = meteor.t;
            meteor.t += dt;
            if meteor.t <= 0.0 {
                continue;
            }
            let (a, b) = (meteor.at(before.max(0.0)), meteor.at(meteor.t));
            // Sampled finely enough that a fast meteor can't pass through
            // a wall or the roof between frames.
            let steps = ((b - a).length() / 0.25).ceil().max(1.0) as usize;
            let hit = (1..=steps)
                .map(|i| a + (b - a) * (i as f32 / steps as f32))
                .find(|p| {
                    p.y <= height(p.x, p.z) + METEOR_RADIUS || site.touches(*p, METEOR_RADIUS)
                });
            match hit {
                Some(p) => landed.push((index, p)),
                None if meteor.t >= FALL => landed.push((index, b)),
                None => {}
            }
        }
        for &(_, at) in &landed {
            blows.extend(self.explode(at, site));
        }
        let gone: Vec<usize> = landed.iter().map(|&(i, _)| i).collect();
        let mut index = 0;
        self.meteors.retain(|_| {
            index += 1;
            !gone.contains(&(index - 1))
        });
        let trail: Vec<(Vec3, Vec3)> = self
            .meteors
            .iter()
            .filter(|m| m.t > 0.0)
            .map(|m| (m.at(m.t), (m.end - m.start).normalize_or(Vec3::NEG_Y)))
            .collect();
        for (at, along) in trail {
            for _ in 0..3 {
                let vel = -along * 6.0 + Vec3::new(self.unit(), self.unit(), self.unit()) * 2.5;
                let life = 0.35 + 0.3 * self.unit().abs();
                self.spark(at, vel, life, 0.18);
            }
        }
        for burst in &mut self.bursts {
            burst.age += dt;
        }
        self.bursts.retain(|b| b.age < BLAST_LIFE);
        for spark in &mut self.sparks {
            spark.age += dt;
            spark.vel.y -= 6.0 * dt;
            spark.vel *= (1.0 - 0.9 * dt).max(0.0);
            spark.at += spark.vel * dt;
            let floor = height(spark.at.x, spark.at.z) + 0.05;
            if spark.at.y < floor {
                spark.at.y = floor;
                spark.vel = Vec3::new(spark.vel.x * 0.4, -spark.vel.y * 0.25, spark.vel.z * 0.4);
            }
        }
        self.sparks.retain(|s| s.age < s.life);
        for puff in &mut self.smoke {
            puff.age += dt;
            puff.vel *= (1.0 - 1.2 * dt).max(0.0);
            puff.vel.y += 0.6 * dt;
            puff.at += puff.vel * dt;
        }
        self.smoke.retain(|s| s.age < s.life);
        for scorch in &mut self.scorches {
            scorch.age += dt;
        }
        self.scorches.retain(|s| s.age < SCORCH_LIFE);
        blows
    }

    /// The cast completes: the mana and cooldown are spent, the damage is
    /// rolled, and the meteors set out for points through the circle at
    /// `at`, from the side of `from`.
    fn release(&mut self, at: Vec3, from: Vec3, site: &mut Site) {
        self.mana -= COST;
        self.cooldown = COOLDOWN;
        self.idle = 0.0;
        self.damage = Damage::roll(&mut |sides| site.roll(sides) as u32);
        // From the caster's side, over a shoulder, so the trails cross the
        // view instead of running straight away from it.
        let away = glam::Quat::from_rotation_y(SHOULDER)
            * Vec3::new(at.x - from.x, 0.0, at.z - from.z).normalize_or(Vec3::Z);
        let slant = (srd::SLANT_DEGREES as f32).to_radians().tan() * HEIGHT;
        let turn = self.unit() * TAU;
        let mut points: Vec<Vec3> = (0..METEORS)
            .map(|i| {
                let (angle, r) = if i == 0 {
                    (self.unit() * TAU, 0.6 * self.unit().abs())
                } else {
                    (
                        turn + TAU * (i - 1) as f32 / (METEORS - 1) as f32 + 0.35 * self.unit(),
                        AREA * (0.42 + 0.14 * self.unit()),
                    )
                };
                let x = at.x + angle.cos() * r;
                let z = at.z + angle.sin() * r;
                Vec3::new(x, height(x, z), z)
            })
            .collect();
        // A shuffled order, the center last.
        for i in (2..points.len()).rev() {
            let j = 1 + (self.next() as usize) % i;
            points.swap(i, j);
        }
        points.rotate_left(1);
        self.meteors = points
            .into_iter()
            .enumerate()
            .map(|(i, end)| Meteor {
                start: end + Vec3::Y * HEIGHT - away * slant,
                end,
                t: -(i as f32) * STAGGER,
            })
            .collect();
    }

    /// One meteor detonates at `at`: the site takes the blast, and the
    /// fire, sparks, scorch, and shake begin.
    fn explode(&mut self, at: Vec3, site: &mut Site) -> Vec<Blow> {
        let ground = height(at.x, at.z);
        let center = Vec3::new(at.x, at.y.max(ground + 0.4), at.z);
        let blows = site.explode(center, BLAST, self.damage.total(), THROW);
        let seed = self.next();
        self.bursts.push(Burst {
            at: center,
            age: 0.0,
            seed,
        });
        for _ in 0..46 {
            let up = 0.25 + 0.75 * self.unit().abs();
            let angle = self.unit() * TAU;
            let speed = 5.0 + 12.0 * self.unit().abs();
            let vel = Vec3::new(
                angle.cos() * (1.0 - up * 0.5),
                up,
                angle.sin() * (1.0 - up * 0.5),
            ) * speed;
            let life = 0.8 + 1.0 * self.unit().abs();
            let size = 0.12 + 0.14 * self.unit().abs();
            self.spark(center, vel, life, size);
        }
        // A dark pall that climbs out of the fireball.
        for _ in 0..7 {
            let angle = self.unit() * TAU;
            let out = 1.0 + 2.0 * self.unit().abs();
            let vel = Vec3::new(
                angle.cos() * out,
                2.0 + 2.5 * self.unit().abs(),
                angle.sin() * out,
            );
            let life = 3.5 + 2.0 * self.unit().abs();
            let size = 1.4 + 1.0 * self.unit().abs();
            self.puff(center + Vec3::Y * 0.8, vel, life, size);
        }
        if self.scorches.len() >= MAX_SCORCHES {
            self.scorches.remove(0);
        }
        let radius = BLAST * (0.75 + 0.15 * self.unit());
        self.scorches.push(Scorch {
            at: Vec3::new(center.x, ground, center.z),
            radius,
            age: 0.0,
            seed,
        });
        self.shake = (self.shake + 0.55).min(1.0);
        blows
    }

    fn puff(&mut self, at: Vec3, vel: Vec3, life: f32, size: f32) {
        if self.smoke.len() >= MAX_SMOKE {
            self.smoke.remove(0);
        }
        self.smoke.push(Spark {
            at,
            vel,
            age: 0.0,
            life,
            size,
        });
    }

    fn spark(&mut self, at: Vec3, vel: Vec3, life: f32, size: f32) {
        if self.sparks.len() >= MAX_SPARKS {
            self.sparks.remove(0);
        }
        self.sparks.push(Spark {
            at,
            vel,
            age: 0.0,
            life: life.max(0.05),
            size,
        });
    }

    /// The targeting circle, the cast's gathering fire, the meteors, their
    /// explosions, sparks, and scorch marks, seen from `eye`.
    pub fn draw(&self, mesh: &mut Mesh, eye: Vec3) {
        let glow = &mut mesh.glow;
        let pulse = 0.75 + 0.25 * (self.clock * 6.0).sin();
        if let Some(at) = self.aim() {
            circle(glow, at, AREA, pulse, self.clock);
        }
        if let Some(cast) = self.casting {
            let k = (cast.elapsed / CAST).clamp(0.0, 1.0);
            let quick = 0.7 + 0.3 * (self.clock * (8.0 + 14.0 * k)).sin();
            circle(glow, cast.at, AREA, quick * (1.0 + 1.5 * k), self.clock);
        }
        for scorch in &self.scorches {
            scorch_mark(mesh, scorch);
        }
        for puff in &self.smoke {
            let x = puff.age / puff.life;
            // It swells, then thins away.
            let size = puff.size * (0.5 + 1.4 * x.sqrt()) * (1.0 - x * x * x);
            // Hot at first, then a sooty grey.
            let color = mix([0.3, 0.12, 0.05], SMOKE, (x * 4.0).min(1.0));
            billow(mesh, puff.at, size, color, (puff.at.x * 13.0) as u32);
        }
        let glow = &mut mesh.glow;
        for scorch in &self.scorches {
            // Embers in the scorch for its first few seconds.
            let fade = (1.0 - scorch.age / 5.0).max(0.0);
            if fade <= 0.0 {
                continue;
            }
            for i in 0..9 {
                let angle = noise(scorch.seed, i) * TAU;
                let r = scorch.radius * 0.8 * noise(scorch.seed, i + 20).sqrt();
                let at = scorch.at + Vec3::new(angle.cos() * r, 0.08, angle.sin() * r);
                let flicker = 0.6 + 0.4 * (self.clock * 9.0 + i as f32).sin();
                flat(
                    glow,
                    at,
                    0.5,
                    tint(ORANGE, FIRE_LUMINANCE * 0.12 * fade * flicker),
                );
            }
        }
        for meteor in self.meteors.iter().filter(|m| m.t > 0.0) {
            let at = meteor.at(meteor.t);
            let along = (meteor.end - meteor.start).normalize_or(Vec3::NEG_Y);
            // The tail, as long as the meteor has fallen.
            let length = (at - meteor.start).length().min(16.0);
            let count = 28;
            for i in (0..count).rev() {
                let x = i as f32 / count as f32;
                let color = mix(YELLOW, RED, x);
                let half = METEOR_RADIUS * (2.2 - 1.8 * x);
                blob(
                    glow,
                    at - along * length * x,
                    half,
                    tint(color, FIRE_LUMINANCE * (1.0 - x) * 0.45),
                    eye,
                );
            }
            blob(
                glow,
                at,
                METEOR_RADIUS * 3.6,
                tint(ORANGE, FIRE_LUMINANCE * 0.3),
                eye,
            );
            blob(
                glow,
                at,
                METEOR_RADIUS * 1.8,
                tint(WHITE_HOT, FIRE_LUMINANCE * 1.4),
                eye,
            );
        }
        for burst in &self.bursts {
            fire(glow, burst, eye);
        }
        for spark in &self.sparks {
            let x = spark.age / spark.life;
            let color = mix(YELLOW, RED, x);
            blob(
                glow,
                spark.at,
                spark.size * (1.0 - 0.5 * x),
                tint(color, FIRE_LUMINANCE * 0.9 * (1.0 - x)),
                eye,
            );
        }
        for meteor in self.meteors.iter().filter(|m| m.t > 0.0) {
            rock(mesh, meteor.at(meteor.t), self.clock);
        }
        if let Some(cast) = self.casting {
            // The meteors gather high over the circle while the cast runs.
            let k = (cast.elapsed / CAST).clamp(0.0, 1.0);
            let glow = &mut mesh.glow;
            let above = cast.at + Vec3::Y * 12.0;
            // One ember for each meteor, circling closer as the cast
            // completes, under a faint halo.
            blob(
                glow,
                above,
                1.0 + 3.0 * k,
                tint(RED, FIRE_LUMINANCE * 0.15 * k),
                eye,
            );
            for i in 0..METEORS {
                let angle = TAU * i as f32 / METEORS as f32 + self.clock * (1.0 + 2.0 * k);
                let r = 3.5 - 2.0 * k;
                let at = above + Vec3::new(angle.cos() * r, 0.0, angle.sin() * r);
                blob(
                    glow,
                    at,
                    0.25 + 0.5 * k,
                    tint(YELLOW, FIRE_LUMINANCE * 0.8 * k),
                    eye,
                );
            }
        }
    }
}

/// `ground` drawn in to [`RANGE`] of `player` across the ground, and set on
/// the ground.
#[must_use]
pub fn clamp_to_range(ground: Vec3, player: Vec3) -> Vec3 {
    let offset = Vec3::new(ground.x - player.x, 0.0, ground.z - player.z);
    let flat = Vec3::new(player.x, 0.0, player.z) + offset.clamp_length_max(RANGE);
    Vec3::new(flat.x, height(flat.x, flat.z), flat.z)
}

/// Where a ray from `origin` along `direction` meets the ground, or, for a
/// ray that never comes down, the ground under its horizontal heading
/// [`RANGE`] away.
#[must_use]
pub fn ground_hit(origin: Vec3, direction: Vec3) -> Option<Vec3> {
    if !origin.is_finite() || !direction.is_finite() || direction.length_squared() < 1e-6 {
        return None;
    }
    let direction = direction.normalize();
    let under = |p: Vec3| p.y <= height(p.x, p.z);
    let step = 0.5;
    let mut t = 0.0;
    while t < 400.0 {
        let next = t + step;
        if under(origin + direction * next) {
            // Narrow down the crossing.
            let (mut lo, mut hi) = (t, next);
            for _ in 0..16 {
                let mid = 0.5 * (lo + hi);
                if under(origin + direction * mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let p = origin + direction * hi;
            return Some(Vec3::new(p.x, height(p.x, p.z), p.z));
        }
        t = next;
    }
    let flat = Vec3::new(direction.x, 0.0, direction.z).try_normalize()?;
    let p = origin + flat * RANGE;
    Some(Vec3::new(p.x, height(p.x, p.z), p.z))
}

/// The targeting ring of `radius` at `at`, `level` times its plain glow:
/// a bright rim, a fainter inner ring, a soft fill, and turning marks.
fn circle(out: &mut Vec<GlowVertex>, at: Vec3, radius: f32, level: f32, clock: f32) {
    let ground = |x: f32, z: f32| Vec3::new(x, height(x, z) + 0.06, z);
    let rim = 64;
    for i in 0..rim {
        let angle = TAU * i as f32 / rim as f32;
        let p = ground(at.x + angle.cos() * radius, at.z + angle.sin() * radius);
        flat(out, p, 0.42, tint(RING, RING_LUMINANCE * level));
    }
    let inner = 40;
    for i in 0..inner {
        let angle = TAU * i as f32 / inner as f32 - clock * 0.4;
        let r = radius * 0.62;
        let p = ground(at.x + angle.cos() * r, at.z + angle.sin() * r);
        flat(out, p, 0.22, tint(YELLOW, RING_LUMINANCE * 0.5 * level));
    }
    // Six marks that turn with the clock, pointing in.
    for i in 0..6 {
        let angle = TAU * i as f32 / 6.0 + clock * 0.6;
        for k in 0..4 {
            let r = radius * (0.68 + 0.08 * k as f32);
            let p = ground(at.x + angle.cos() * r, at.z + angle.sin() * r);
            flat(out, p, 0.25, tint(YELLOW, RING_LUMINANCE * 0.7 * level));
        }
    }
    flat(
        out,
        ground(at.x, at.z),
        radius,
        tint(RING, RING_LUMINANCE * 0.08 * level),
    );
    flat(
        out,
        ground(at.x, at.z),
        0.5,
        tint(YELLOW, RING_LUMINANCE * 0.6 * level),
    );
}

/// One explosion's flash, fireball, rising column, and shockwave ring.
fn fire(out: &mut Vec<GlowVertex>, burst: &Burst, eye: Vec3) {
    let a = burst.age;
    let at = burst.at;
    // The flash.
    if a < 0.22 {
        let k = 1.0 - a / 0.22;
        blob(
            out,
            at,
            3.0 + 14.0 * a,
            tint(WHITE_HOT, FLASH_LUMINANCE * k * k),
            eye,
        );
    }
    // The fireball swells fast, then cools and rises.
    let grow = (a / 0.3).min(1.0);
    let size = BLAST * (0.35 + 0.75 * (1.0 - (1.0 - grow) * (1.0 - grow)));
    let cool = (a / BLAST_LIFE).clamp(0.0, 1.0);
    let color = if cool < 0.3 {
        mix(WHITE_HOT, YELLOW, cool / 0.3)
    } else {
        mix(ORANGE, RED, (cool - 0.3) / 0.7)
    };
    let fade = (1.0 - cool) * (1.0 - cool);
    for i in 0..9u32 {
        let angle = noise(burst.seed, i) * TAU;
        let r = size * 0.45 * noise(burst.seed, i + 10);
        let lift = a * (1.5 + 2.5 * noise(burst.seed, i + 30));
        let p = at + Vec3::new(angle.cos() * r, 0.4 + lift + r * 0.4, angle.sin() * r);
        let half = size * (0.55 + 0.35 * noise(burst.seed, i + 40));
        blob(out, p, half, tint(color, FIRE_LUMINANCE * 0.5 * fade), eye);
    }
    // A column of fire that climbs.
    for i in 0..6 {
        let x = i as f32 / 6.0;
        let p = at + Vec3::Y * (a * 7.0 * (0.3 + x));
        blob(
            out,
            p,
            BLAST * 0.45 * (1.0 - 0.5 * x),
            tint(mix(ORANGE, RED, x), FIRE_LUMINANCE * 0.35 * fade),
            eye,
        );
    }
    // The shockwave along the ground.
    if a < 0.55 {
        let k = a / 0.55;
        let r = BLAST * 2.3 * (1.0 - (1.0 - k) * (1.0 - k));
        let strength = (1.0 - k) * (1.0 - k);
        let count = 48;
        for i in 0..count {
            let angle = TAU * i as f32 / count as f32;
            let x = at.x + angle.cos() * r;
            let z = at.z + angle.sin() * r;
            flat(
                out,
                Vec3::new(x, height(x, z) + 0.15, z),
                0.5 + 0.6 * k,
                tint(WHITE_HOT, FIRE_LUMINANCE * 0.5 * strength),
            );
        }
    }
}

/// A dark ragged scorch on the ground, shrinking away at the end of its
/// life.
fn scorch_mark(mesh: &mut Mesh, scorch: &Scorch) {
    let left = SCORCH_LIFE - scorch.age;
    let k = (left / 4.0).clamp(0.0, 1.0);
    let radius = scorch.radius * k.sqrt();
    if radius <= 0.01 {
        return;
    }
    let ground = |x: f32, z: f32| Vec3::new(x, height(x, z) + 0.03, z);
    let center = ground(scorch.at.x, scorch.at.z);
    let sides = 18;
    let edge: Vec<Vec3> = (0..sides)
        .map(|i| {
            let angle = TAU * i as f32 / sides as f32;
            let r = radius * (0.7 + 0.3 * noise(scorch.seed, 100 + i));
            ground(scorch.at.x + angle.cos() * r, scorch.at.z + angle.sin() * r)
        })
        .collect();
    let color = mix(CHAR, [0.11, 0.08, 0.05], 1.0 - k);
    for i in 0..sides as usize {
        let (a, b) = (edge[i], edge[(i + 1) % edge.len()]);
        for p in [center, b, a] {
            mesh.faces.push(Vertex {
                pos: p.to_array(),
                color,
                fog: 1.0,
            });
        }
    }
}

/// A billow of smoke of about `size` m at `at`: three overlapping
/// squat spheres, the same on every frame for one `seed`.
fn billow(mesh: &mut Mesh, at: Vec3, size: f32, color: [f32; 3], seed: u32) {
    if size <= 0.02 {
        return;
    }
    for lobe in 0..3 {
        let angle = TAU * (lobe as f32 / 3.0 + noise(seed, lobe));
        let offset = Vec3::new(angle.cos(), 0.15 * lobe as f32, angle.sin()) * size * 0.45;
        let r = size * (0.55 + 0.25 * noise(seed, lobe + 7));
        let shade_k = 0.85 + 0.15 * noise(seed, lobe + 3);
        sphere(mesh, at + offset, r, color.map(|c| c * shade_k));
    }
}

/// A squat low-poly sphere of radius `r` at `at`.
fn sphere(mesh: &mut Mesh, at: Vec3, r: f32, color: [f32; 3]) {
    const AROUND: usize = 7;
    const UP: usize = 4;
    let point = |i: usize, j: usize| {
        let theta = TAU * i as f32 / AROUND as f32;
        let phi = std::f32::consts::PI * j as f32 / UP as f32;
        at + Vec3::new(
            theta.cos() * phi.sin(),
            -phi.cos() * 0.8,
            theta.sin() * phi.sin(),
        ) * r
    };
    for j in 0..UP {
        for i in 0..AROUND {
            let (a, b, c, d) = (
                point(i, j),
                point(i + 1, j),
                point(i + 1, j + 1),
                point(i, j + 1),
            );
            for tri in [[a, c, b], [a, d, c]] {
                if (tri[1] - tri[0]).cross(tri[2] - tri[0]).length_squared() < 1e-10 {
                    continue;
                }
                let color = shade(color, tri[0], tri[1], tri[2]);
                for p in tri {
                    mesh.faces.push(Vertex {
                        pos: p.to_array(),
                        color,
                        fog: 1.0,
                    });
                }
            }
        }
    }
}

/// The meteor's rock: a rough tumbling octahedron.
fn rock(mesh: &mut Mesh, at: Vec3, clock: f32) {
    let (s, c) = (clock * 5.0).sin_cos();
    let x = Vec3::new(c, s * 0.3, s) * METEOR_RADIUS;
    let y = Vec3::new(-s * 0.3, 1.0, 0.2).normalize() * METEOR_RADIUS * 0.9;
    let z = x.cross(y).normalize() * METEOR_RADIUS * 1.1;
    let points = [at + x, at - x, at + y, at - y, at + z, at - z];
    for (a, b, cc) in [
        (0, 2, 4),
        (4, 2, 1),
        (1, 2, 5),
        (5, 2, 0),
        (0, 4, 3),
        (4, 1, 3),
        (1, 5, 3),
        (5, 0, 3),
    ] {
        let color = shade(ROCK, points[a], points[b], points[cc]);
        for p in [points[a], points[b], points[cc]] {
            mesh.faces.push(Vertex {
                pos: p.to_array(),
                color,
                fog: 1.0,
            });
        }
    }
}

/// `color` at `luminance`.
fn tint(color: [f32; 3], luminance: f32) -> [f32; 3] {
    color.map(|c| c * luminance.max(0.0))
}

fn mix(a: [f32; 3], b: [f32; 3], x: f32) -> [f32; 3] {
    let x = x.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * x)
}

/// A round glow of half size `half` at `at`, turned toward `eye`.
fn blob(out: &mut Vec<GlowVertex>, at: Vec3, half: f32, radiance: [f32; 3], eye: Vec3) {
    // A glow brushing the camera would fill the view; it fades out instead.
    let near = ((eye.distance(at) - half * 0.5 - 0.5) / 2.0).clamp(0.0, 1.0);
    if near <= 0.0 || radiance.iter().all(|c| *c <= 0.0) {
        return;
    }
    let toward = (eye - at).normalize_or(Vec3::Z);
    let right = Vec3::Y.cross(toward).normalize_or(Vec3::X);
    let up = toward.cross(right);
    quad(out, at, right * half, up * half, radiance.map(|c| c * near));
}

/// A round glow of half size `half` lying on the ground at `at`.
fn flat(out: &mut Vec<GlowVertex>, at: Vec3, half: f32, radiance: [f32; 3]) {
    quad(out, at, Vec3::X * half, Vec3::Z * half, radiance);
}

/// A glow quad centered at `at` spanning `right` and `up` each way.
fn quad(out: &mut Vec<GlowVertex>, at: Vec3, right: Vec3, up: Vec3, radiance: [f32; 3]) {
    for (x, y) in [
        (-1.0, -1.0),
        (1.0, -1.0),
        (1.0, 1.0),
        (-1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
    ] {
        out.push(GlowVertex {
            pos: (at + right * x + up * y).to_array(),
            radiance,
            uv: [x, y],
        });
    }
}

/// A value in `0..1` for `n` in stream `seed`.
fn noise(seed: u32, n: u32) -> f32 {
    super::noise(n, seed)
}

impl Swarm {
    /// A value in `-1..1`.
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    }

    /// xorshift32.
    fn next(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }
}

#[cfg(test)]
impl Swarm {
    /// Seconds of cooldown left.
    pub fn cooldown_left(&self) -> f32 {
        self.cooldown
    }

    /// How many meteors are on their way.
    pub fn meteors_left(&self) -> usize {
        self.meteors.len()
    }
}
