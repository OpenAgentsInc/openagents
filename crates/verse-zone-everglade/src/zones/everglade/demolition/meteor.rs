//! Meteor Swarm in the demolition yard: the SRD's level 9 Evocation
//! (`verse_world::meteor_swarm`) aimed at a circle of ground and tuned
//! for buildings.
//!
//! `2` enters targeting: a pulsing ring of fire on the ground under the
//! cursor, as wide as the strike, never farther than [`RANGE`] from the
//! player. A click confirms it, and right click, `Esc`, or `2` again
//! cancels without spending anything. The cast takes [`CAST`] seconds, as
//! the combat model's level 9 tier does (`docs/verse/combat-model.md`);
//! the caster may walk, run, and jump through it, and the strike still
//! falls on the point chosen; right click or `Esc` stops it and spends
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
//!
//! The cursor's ray finds the first surface that stands in 3D
//! ([`surface_aim`]): the ground, a roof, or a wall, such as the side of the
//! Grove's concrete tower or, through a hole already blown in a near wall,
//! the inner face of the far one. On a wall within [`RANGE`] of the
//! caster's eyes the ring lies flat against it, facing the caster, and is
//! [`WALL_AREA`] wide; the meteors fly in on a slant from the sky in front
//! of the wall and burst on its face, spalling what breaks out of the wall.
//! When standing walls or a roof block that slant, as they do for a far
//! wall seen through a hole or a floor under a roof, the meteors and the
//! bolt come in along the targeting ray instead, through the opening the
//! caster looked through.
//!
//! The same targeting calls down the Grove's Thunderbolt
//! ([`Strike::Lightning`]): after a [`BOLT_CAST`] s cast, one thick jagged
//! bolt with branches leaves the clouds for the ring, flashes, and blasts
//! [`BOLT_BLAST`] m around the strike with [`BOLT_DICE`] (the
//! `thunderbolt_strike` effect). Everglade's dev bar also casts the Mega
//! Thunderbolt ([`Strike::MegaLightning`]): a [`MEGA_CAST`] s cast, five
//! times the damage ([`MEGA_DICE`]) over five times the volume
//! ([`MEGA_BLAST`]), a thicker, brighter bolt, and a harder shake.
//!
//! The fire, smoke, sparks, and shockwave are sprite particles: the effects
//! under `assets/verse/fx/effects/` (`meteor_head`, `meteor_trail`,
//! `meteor_explosion`, `cast_embers`, and `scorch_embers`), run by
//! [`crate::fx`].

use super::site::{Blow, Target};
use crate::controller::PlayerController;
use crate::fx::{Handle, Particles, Spawn};
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
/// mile is far past the yard's trees. A `dev-destruction` build, where the
/// owner tests destruction, targets anything in sight instead ([`SIGHT`]).
pub const RANGE: f32 = if LINE_OF_SIGHT { SIGHT } else { 36.0 };
/// Whether the strikes reach anything the player can see: only in a
/// `dev-destruction` build.
pub const LINE_OF_SIGHT: bool = cfg!(feature = "dev-destruction");
/// How far the player can see, m: well past where the fog
/// (`verse_pbr::fog::FOG_END`, 250 m) hides everything, so whatever is
/// drawn under the cursor is in reach.
pub const SIGHT: f32 = 600.0;
/// The circle's radius, m: the strike's whole footprint.
pub const AREA: f32 = 6.0;
/// Each explosion's radius, m.
pub const BLAST: f32 = 4.0;
/// How many meteors fall.
pub const METEORS: usize = 6;
/// Fastest an explosion throws debris, m/s, at its center.
pub const THROW: f32 = 17.0;
/// The slot's one-sentence description, for the hotbar's tooltip.
pub const TOOLTIP: &str = "Calls down six blazing meteors on a circle of ground or wall you choose, \
    blasting apart every wall and roof they reach.";
/// The targeting ring's radius on a wall, m, and how far through it the
/// meteors spread: a tower's side, not a field.
pub const WALL_AREA: f32 = 3.0;
/// Farthest a wall may be from the caster's eyes, m.
const EYE: f32 = 1.5;
/// The thunderbolt's cast bar, s; its ring's radius, m; its blast's
/// radius, m; and how fast its blast throws debris, m/s.
pub const BOLT_CAST: f32 = 1.2;
pub const BOLT_AREA: f32 = 2.2;
pub const BOLT_BLAST: f32 = 3.4;
pub const BOLT_THROW: f32 = 14.0;
/// The thunderbolt's damage to structures: 20d10 lightning, rolled once
/// per cast.
pub const BOLT_DICE: (u32, u32) = (20, 10);
/// How long the bolt takes to come down, s, and how long it stays lit.
const BOLT_LEAD: f32 = 0.09;
const BOLT_LIFE: f32 = 0.5;
/// How high above the target the bolt leaves the clouds, m.
const BOLT_HEIGHT: f32 = 60.0;
const BOLT_CORE: [f32; 3] = [0.92, 0.95, 1.0];
const BOLT_GLOW: [f32; 3] = [0.45, 0.6, 1.0];
const BOLT_LUMINANCE: f32 = 90.0;
/// The Mega Thunderbolt's cast bar, s, and its ring's radius, m.
pub const MEGA_CAST: f32 = 0.5;
pub const MEGA_AREA: f32 = 3.6;
/// Its blast's radius, m: five times the Thunderbolt's blast volume.
pub const MEGA_BLAST: f32 = BOLT_BLAST * 1.71;
/// How fast its blast throws debris, m/s.
pub const MEGA_THROW: f32 = 22.0;
/// Its damage to structures: 100d10, five times the Thunderbolt's.
pub const MEGA_DICE: (u32, u32) = (100, 10);

/// What the targeting calls down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Strike {
    /// Meteor Swarm's six meteors.
    #[default]
    Meteors,
    /// One huge bolt of lightning.
    Lightning,
    /// The Mega Thunderbolt: a faster cast and five times the damage.
    MegaLightning,
}

impl Strike {
    /// The spell's name, for messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Meteors => "Meteor Swarm",
            Self::Lightning => "Thunderbolt",
            Self::MegaLightning => "Mega Thunderbolt",
        }
    }

    /// Whether it calls down a bolt of lightning rather than meteors.
    #[must_use]
    pub const fn lightning(self) -> bool {
        matches!(self, Self::Lightning | Self::MegaLightning)
    }

    /// The cast bar's length, s.
    #[must_use]
    pub const fn cast(self) -> f32 {
        match self {
            Self::Meteors => CAST,
            Self::Lightning => BOLT_CAST,
            Self::MegaLightning => MEGA_CAST,
        }
    }

    /// The targeting ring's radius on `wall` or on the ground, m.
    #[must_use]
    pub const fn area(self, wall: bool) -> f32 {
        match (self, wall) {
            (Self::Meteors, false) => AREA,
            (Self::Meteors, true) => WALL_AREA,
            (Self::Lightning, _) => BOLT_AREA,
            (Self::MegaLightning, _) => MEGA_AREA,
        }
    }
}

/// Where a strike is aimed: a point on a surface and the surface's
/// normal there, straight up on the ground or a roof and toward the caster
/// on a wall, whichever side of the wall that is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    pub at: Vec3,
    pub normal: Vec3,
    /// The unit direction of the targeting ray that found it, or zero for
    /// a point chosen without one: the way in a strike takes when the sky
    /// above the point is blocked.
    pub view: Vec3,
}

impl Aim {
    /// A point on the ground or a roof.
    #[must_use]
    pub const fn ground(at: Vec3) -> Self {
        Self {
            at,
            normal: Vec3::Y,
            view: Vec3::ZERO,
        }
    }

    /// Whether it is on a wall rather than the ground or a roof.
    #[must_use]
    pub fn wall(&self) -> bool {
        self.normal.y < 0.6
    }

    /// Two unit directions across the surface, at right angles: along a
    /// wall and up it, or east and south on the ground.
    #[must_use]
    pub fn across(&self) -> (Vec3, Vec3) {
        if !self.wall() {
            return (Vec3::X, Vec3::Z);
        }
        let u = Vec3::Y.cross(self.normal).normalize_or(Vec3::X);
        (u, self.normal.cross(u).normalize_or(Vec3::Y))
    }
}

/// Where a strike landed, for what it hurts besides buildings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub at: Vec3,
    pub normal: Vec3,
    pub strike: Strike,
    /// Its blast's radius, m.
    pub radius: f32,
}

/// Height a meteor falls from, s it takes, and s between meteors.
const HEIGHT: f32 = 30.0;
const FALL: f32 = 1.0;
const STAGGER: f32 = 0.13;
/// How far the meteors' heading turns from straight away from the caster,
/// radians.
const SHOULDER: f32 = 0.6;
/// A meteor's radius, m.
const METEOR_RADIUS: f32 = 0.55;
/// How long a scorch mark lasts, s.
const SCORCH_LIFE: f32 = 16.0;
/// Most scorch marks at once.
const MAX_SCORCHES: usize = 18;
/// How long the camera shakes after a blast, at most, s.
const SHAKE: f32 = 1.0;

/// Fire's colors, from yellow to a dull red, and the targeting ring's.
const YELLOW: [f32; 3] = [1.0, 0.68, 0.22];
const RED: [f32; 3] = [0.85, 0.16, 0.03];
const RING: [f32; 3] = [1.0, 0.45, 0.1];
const ROCK: [f32; 3] = [0.16, 0.07, 0.04];
const CHAR: [f32; 3] = [0.03, 0.025, 0.022];
/// Luminance of each glow, cd/m² before exposure.
const RING_LUMINANCE: f32 = 7.0;
const FIRE_LUMINANCE: f32 = 40.0;

/// A cast under way.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Casting {
    aim: Aim,
    /// Where the player stood when it began.
    from: Vec3,
    elapsed: f32,
    strike: Strike,
}

/// A thunderbolt: where it strikes, and how long since it left the
/// clouds (below [`BOLT_LEAD`] it is still coming down).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bolt {
    aim: Aim,
    /// Where it comes from: the clouds, or back along the targeting ray.
    sky: Vec3,
    age: f32,
    seed: u32,
    struck: bool,
    /// Whether it is the Mega Thunderbolt.
    mega: bool,
}

impl Bolt {
    /// Where it leaves the clouds for `aim`: high over the target, out in
    /// front of a wall.
    fn clouds(aim: Aim) -> Vec3 {
        let out = if aim.wall() {
            aim.normal * 14.0
        } else {
            Vec3::ZERO
        };
        aim.at + out + Vec3::Y * BOLT_HEIGHT
    }

    /// The jagged path from the clouds to the target, and its branches,
    /// each a list of points.
    fn paths(&self) -> Vec<Vec<Vec3>> {
        let (a, b) = (self.sky, self.aim.at);
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
        let main = jag(a, b, 16, 3.2, 0);
        let mut paths = vec![main.clone()];
        for k in 0..5u32 {
            let i = 3 + (noise(seed, 100 + k) * 11.0) as usize;
            let from = main[i.min(main.len() - 2)];
            let reach = 6.0 + 10.0 * noise(seed, 110 + k);
            let turn = side * (noise(seed, 120 + k) * 2.0 - 1.0)
                + other * (noise(seed, 130 + k) * 2.0 - 1.0);
            let to = from + (along + turn.normalize_or(side)).normalize() * reach;
            paths.push(jag(from, to, 6, 1.2, 200 + 20 * k));
        }
        paths
    }
}

/// One meteor: from where it falls, where it lands, and how long it has
/// been falling (below zero while it waits its turn).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Meteor {
    start: Vec3,
    end: Vec3,
    t: f32,
    /// Its burning head and its trail, once it is falling.
    fire: Option<[Handle; 2]>,
    /// The outward normal of the wall it was sent at, or zero.
    face: Vec3,
}

impl Meteor {
    fn at(&self, t: f32) -> Vec3 {
        // It speeds up as it falls.
        let x = (t / FALL).clamp(0.0, 1.0);
        self.start + (self.end - self.start) * (0.55 * x + 0.45 * x * x)
    }

    /// Its velocity at `t`, m/s.
    fn velocity(&self, t: f32) -> Vec3 {
        let x = (t / FALL).clamp(0.0, 1.0);
        (self.end - self.start) * (0.55 + 0.9 * x) / FALL
    }
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
    /// What it aims or casts.
    pub strike: Strike,
}

/// The spell's state, its meteors in flight, and what they left behind.
pub struct Swarm {
    mana: f32,
    /// Seconds of cooldown left, and since the last cast.
    cooldown: f32,
    idle: f32,
    targeting: bool,
    /// Where the circle lies while targeting, once the cursor has found a
    /// surface.
    aim: Option<Aim>,
    /// What the targeting calls down.
    strike: Strike,
    casting: Option<Casting>,
    meteors: Vec<Meteor>,
    bolts: Vec<Bolt>,
    /// Where strikes landed since the last [`Swarm::take_impacts`].
    impacts: Vec<Impact>,
    damage: Damage,
    /// The fire, smoke, sparks, and shockwaves.
    fx: Particles,
    /// The embers around the caster while the cast runs.
    gathering: Option<Handle>,
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
            strike: Strike::Meteors,
            casting: None,
            meteors: Vec::new(),
            bolts: Vec::new(),
            impacts: Vec::new(),
            damage: Damage {
                fire: 0,
                bludgeoning: 0,
            },
            fx: Particles::new(0x3E7E_0125),
            gathering: None,
            scorches: Vec::new(),
            shake: 0.0,
            clock: 0.0,
            rng: 0x3E7E_0125,
        }
    }
}

impl Swarm {
    /// How many meteors are on their way.
    pub fn meteors_left(&self) -> usize {
        self.meteors.len()
    }

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
        self.target_with(Strike::Meteors)
    }

    /// Enters targeting for `strike`, switches to it while targeting
    /// another, or leaves targeting when already aiming it.
    ///
    /// # Errors
    ///
    /// Returns why the spell can't be cast now.
    pub fn target_with(&mut self, strike: Strike) -> Result<(), String> {
        if self.targeting {
            if self.strike == strike {
                self.cancel();
            } else {
                self.strike = strike;
            }
            return Ok(());
        }
        if self.casting.is_some() {
            return Err(format!(
                "Already casting {}",
                self.casting.map_or(strike, |c| c.strike).name()
            ));
        }
        self.strike = strike;
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
        self.aimed().map(|a| a.at)
    }

    /// Where the circle lies while targeting, with the surface's normal.
    #[must_use]
    pub fn aimed(&self) -> Option<Aim> {
        self.aim.filter(|_| self.targeting)
    }

    /// What the targeting calls down.
    #[must_use]
    pub fn strike(&self) -> Strike {
        self.strike
    }

    /// Puts the circle at `ground`, drawn in no farther than [`RANGE`]
    /// from `player`, on the ground.
    pub fn aim_at(&mut self, ground: Vec3, player: &PlayerController) {
        if !self.targeting || !ground.is_finite() {
            return;
        }
        self.aim = Some(Aim::ground(clamp_to_range(ground, player.pos)));
    }

    /// Puts the circle on the surface `aim` found: flat against a wall
    /// within [`RANGE`] of `player`'s eyes; on the ground, a roof, or a
    /// floor within [`RANGE`] of `player` across the ground; or, past
    /// range, on the ground drawn in to range as [`Self::aim_at`] does.
    pub fn aim_on(&mut self, aim: Aim, player: &PlayerController) {
        if !self.targeting || !aim.at.is_finite() || !aim.normal.is_finite() {
            return;
        }
        let eye = player.pos + Vec3::Y * EYE;
        let across = Vec3::new(aim.at.x - player.pos.x, 0.0, aim.at.z - player.pos.z).length();
        if aim.wall() && aim.at.distance(eye) <= RANGE {
            self.aim = Some(aim);
        } else if !aim.wall() && across <= RANGE {
            self.aim = Some(Aim {
                normal: Vec3::Y,
                ..aim
            });
        } else {
            self.aim_at(aim.at, player);
        }
    }

    /// Puts the circle `ahead` meters in front of `player`, where a touch
    /// screen without a cursor first shows it.
    pub fn aim_ahead(&mut self, player: &PlayerController, ahead: f32) {
        self.aim_at(player.pos + player.forward() * ahead, player);
    }

    /// Starts the cast at the circle. Returns whether it started.
    pub fn confirm(&mut self, player: &PlayerController) -> bool {
        let Some(aim) = self.aimed() else {
            return false;
        };
        self.targeting = false;
        self.aim = None;
        self.casting = Some(Casting {
            aim,
            from: player.pos,
            elapsed: 0.0,
            strike: self.strike,
        });
        true
    }

    /// Where strikes landed since the last call.
    pub fn take_impacts(&mut self) -> Vec<Impact> {
        std::mem::take(&mut self.impacts)
    }

    /// Adds a jolt of `size`, from 0 to 1, to the camera's shake, as a
    /// toppled tower striking the ground does.
    pub fn quake(&mut self, size: f32) {
        self.shake = (self.shake + size.clamp(0.0, 1.0)).min(1.0);
    }

    /// Starts `effect` at `at` among the spell's particles, as a crash's
    /// dust does.
    pub fn burst(&mut self, effect: &str, at: Vec3) {
        self.fx.start(effect, Spawn::at(at));
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
            casting: self
                .casting
                .map(|c| (c.elapsed / c.strike.cast()).clamp(0.0, 1.0)),
            strike: self.casting.map_or(self.strike, |c| c.strike),
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
    pub fn tick(&mut self, dt: f32, player: &PlayerController, site: &mut dyn Target) -> Vec<Blow> {
        self.clock += dt;
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.idle += dt;
        if self.idle >= REST && self.casting.is_none() {
            self.mana = (self.mana + REGEN * dt).min(MAX_MANA);
        }
        self.shake = (self.shake - dt / SHAKE).max(0.0);
        // Moving never interrupts the cast: the strike falls where it was
        // aimed, wherever the caster has gone.
        if let Some(cast) = &mut self.casting {
            cast.elapsed += dt;
            if cast.elapsed >= cast.strike.cast() {
                let (aim, from, strike) = (cast.aim, cast.from, cast.strike);
                self.casting = None;
                match strike {
                    Strike::Meteors => self.release(aim, from, site),
                    Strike::Lightning => self.call_bolt(aim, site, false),
                    Strike::MegaLightning => self.call_bolt(aim, site, true),
                }
            }
        }
        let mut blows = self.tick_bolts(dt, site);
        // Embers swirl up around the caster while the cast runs.
        let hands = player.pos + Vec3::Y;
        match (self.casting.is_some(), self.gathering) {
            (true, None) => self.gathering = self.fx.start("cast_embers", Spawn::at(hands)),
            (true, Some(embers)) => self.fx.place(embers, hands, Vec3::ZERO),
            (false, Some(embers)) => {
                self.fx.stop(embers);
                self.gathering = None;
            }
            (false, None) => {}
        }
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
            // Its fire follows it, trailing back along its flight.
            let velocity = meteor.velocity(meteor.t);
            let back = -velocity.normalize_or(Vec3::NEG_Y);
            match meteor.fire {
                None => {
                    let spawn = Spawn::at(b).moving(velocity).along(back);
                    meteor.fire = self
                        .fx
                        .start("meteor_head", spawn)
                        .zip(self.fx.start("meteor_trail", spawn))
                        .map(|(head, trail)| [head, trail]);
                }
                Some(fire) => {
                    for handle in fire {
                        self.fx.place(handle, b, velocity);
                    }
                }
            }
        }
        for &(index, _) in &landed {
            for handle in self.meteors[index].fire.into_iter().flatten() {
                self.fx.stop(handle);
            }
        }
        for &(index, at) in &landed {
            let face = self.meteors[index].face;
            blows.extend(self.explode(at, face, site));
        }
        let gone: Vec<usize> = landed.iter().map(|&(i, _)| i).collect();
        let mut index = 0;
        self.meteors.retain(|_| {
            index += 1;
            !gone.contains(&(index - 1))
        });
        for scorch in &mut self.scorches {
            scorch.age += dt;
        }
        self.scorches.retain(|s| s.age < SCORCH_LIFE);
        self.fx.tick(dt, height);
        blows
    }

    /// The cast completes: the mana and cooldown are spent, the damage is
    /// rolled, and the meteors set out for points through the circle at
    /// `aim`, from the side of `from`.
    fn release(&mut self, aim: Aim, from: Vec3, site: &mut dyn Target) {
        self.mana -= COST;
        self.cooldown = COOLDOWN;
        self.idle = 0.0;
        self.damage = Damage::roll(&mut |sides| site.roll(sides) as u32);
        if aim.wall() {
            self.release_on_wall(aim, site);
            return;
        }
        let at = aim.at;
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
                Vec3::new(at.x + angle.cos() * r, at.y, at.z + angle.sin() * r)
            })
            .collect();
        // A shuffled order, the center last.
        for i in (2..points.len()).rev() {
            let j = 1 + (self.next() as usize) % i;
            points.swap(i, j);
        }
        points.rotate_left(1);
        let sky = Vec3::Y * HEIGHT - away * slant;
        let paths: Vec<(Vec3, Vec3)> = if blocked(&*site, aim, at + sky, at) {
            // A floor under a roof: in along the targeting ray.
            points
                .iter()
                .map(|&p| along_view(&*site, aim, p, aim.view * 0.3))
                .collect()
        } else {
            // Each meteor falls on the ground under its point and bursts on
            // the first roof or wall in its way.
            points
                .iter()
                .map(|&p| {
                    let end = Vec3::new(p.x, height(p.x, p.z), p.z);
                    (end + sky, end)
                })
                .collect()
        };
        self.launch(&paths, Vec3::ZERO);
    }

    /// The meteors set out for points across a wall at `aim`, flying in
    /// from the sky in front of it on a slant and bursting on its face, or,
    /// when that slant is blocked, along the targeting ray.
    fn release_on_wall(&mut self, aim: Aim, site: &mut dyn Target) {
        let (u, v) = aim.across();
        let turn = self.unit() * TAU;
        let mut points: Vec<Vec3> = (0..METEORS)
            .map(|i| {
                let (angle, r) = if i == 0 {
                    (self.unit() * TAU, 0.3 * self.unit().abs())
                } else {
                    (
                        turn + TAU * (i - 1) as f32 / (METEORS - 1) as f32 + 0.35 * self.unit(),
                        WALL_AREA * (0.45 + 0.15 * self.unit()),
                    )
                };
                aim.at + (u * angle.cos() + v * angle.sin()) * r
            })
            .collect();
        for i in (2..points.len()).rev() {
            let j = 1 + (self.next() as usize) % i;
            points.swap(i, j);
        }
        points.rotate_left(1);
        // Into the wall a little, so a meteor bursts on its face.
        let into = -aim.normal * 0.6;
        let front = aim.normal * 0.85 * HEIGHT + Vec3::Y * HEIGHT * 0.75;
        let through = blocked(&*site, aim, aim.at + into + front, aim.at + into);
        let mut paths = Vec::with_capacity(points.len());
        for &p in &points {
            let side = u * (0.25 * self.unit());
            paths.push(if through {
                along_view(&*site, aim, p, aim.view * 0.6)
            } else {
                let end = p + into;
                (end + front + side * HEIGHT, end)
            });
        }
        self.launch(&paths, aim.normal);
    }

    /// Sends a meteor down each of `paths`, a start and an end, one after
    /// another; `face` is the outward normal of the wall they were sent at,
    /// or zero.
    fn launch(&mut self, paths: &[(Vec3, Vec3)], face: Vec3) {
        self.meteors = paths
            .iter()
            .enumerate()
            .map(|(i, &(start, end))| Meteor {
                start,
                end,
                t: -(i as f32) * STAGGER,
                fire: None,
                face,
            })
            .collect();
    }

    /// The thunderbolt's cast completes: its damage is rolled and the bolt
    /// leaves the clouds for `aim`.
    fn call_bolt(&mut self, aim: Aim, site: &mut dyn Target, mega: bool) {
        self.idle = 0.0;
        let (count, sides) = if mega { MEGA_DICE } else { BOLT_DICE };
        let total: i32 = (0..count).map(|_| site.roll(sides)).sum();
        self.damage = Damage {
            fire: 0,
            bludgeoning: total.max(0),
        };
        let seed = self.next();
        if self.bolts.len() >= 4 {
            self.bolts.remove(0);
        }
        // Down from the clouds, or, when walls or a roof stand in that
        // way, back along the targeting ray through the opening the caster
        // looked through.
        let clouds = Bolt::clouds(aim);
        let sky = if aim.view != Vec3::ZERO && !reaches(&*site, clouds, aim.at) {
            way_back(&*site, aim.at, aim.view, BOLT_HEIGHT)
        } else {
            clouds
        };
        self.bolts.push(Bolt {
            aim,
            sky,
            age: 0.0,
            seed,
            struck: false,
            mega,
        });
    }

    /// Advances the thunderbolts: each strikes once its leader reaches the
    /// target, and fades after. Returns its blast's blows.
    fn tick_bolts(&mut self, dt: f32, site: &mut dyn Target) -> Vec<Blow> {
        let mut blows = Vec::new();
        for index in 0..self.bolts.len() {
            self.bolts[index].age += dt;
            let bolt = self.bolts[index];
            if bolt.struck || bolt.age < BOLT_LEAD {
                continue;
            }
            self.bolts[index].struck = true;
            let Aim { at, normal, .. } = bolt.aim;
            let face = if bolt.aim.wall() { normal } else { Vec3::ZERO };
            // Its heart sits just off the face, as a meteor's does.
            let center = at + normal * 0.5;
            let (blast, throw, strike) = if bolt.mega {
                (MEGA_BLAST, MEGA_THROW, Strike::MegaLightning)
            } else {
                (BOLT_BLAST, BOLT_THROW, Strike::Lightning)
            };
            blows.extend(site.explode_facing(center, blast, self.damage.total(), throw, face));
            self.fx.start("thunderbolt_strike", Spawn::at(center));
            if bolt.mega {
                // A second burst and a fireball's shockwave around it.
                self.fx
                    .start("thunderbolt_strike", Spawn::at(center + Vec3::Y));
                self.fx.start("meteor_explosion", Spawn::at(center));
            }
            let ground = height(at.x, at.z);
            if at.y - ground < 1.5 {
                if self.scorches.len() >= MAX_SCORCHES {
                    self.scorches.remove(0);
                }
                self.scorches.push(Scorch {
                    at: Vec3::new(at.x, ground, at.z),
                    radius: blast * 0.7,
                    age: 0.0,
                    seed: bolt.seed,
                });
            }
            self.impacts.push(Impact {
                at: center,
                normal,
                strike,
                radius: blast,
            });
            self.shake = (self.shake + if bolt.mega { 1.0 } else { 0.7 }).min(1.0);
        }
        self.bolts.retain(|b| b.age < BOLT_LIFE);
        blows
    }

    /// One meteor detonates at `at`: the site takes the blast, and the
    /// fire, sparks, scorch, and shake begin.
    fn explode(&mut self, at: Vec3, face: Vec3, site: &mut dyn Target) -> Vec<Blow> {
        let ground = height(at.x, at.z);
        // On a wall the blast's heart sits just off its face.
        let at = at + face * 0.5;
        let center = Vec3::new(at.x, at.y.max(ground + 0.4), at.z);
        let blows = site.explode_facing(center, BLAST, self.damage.total(), THROW, face);
        let seed = self.next();
        self.fx.start("meteor_explosion", Spawn::at(center));
        self.impacts.push(Impact {
            at: center,
            normal: if face == Vec3::ZERO { Vec3::Y } else { face },
            strike: Strike::Meteors,
            radius: BLAST,
        });
        self.shake = (self.shake + 0.55).min(1.0);
        if center.y - ground > 1.5 {
            // High on a wall: embers where it burst, no mark on the ground.
            self.fx.start("scorch_embers", Spawn::at(center));
            return blows;
        }
        let floor = Vec3::new(center.x, ground, center.z);
        self.fx
            .start("scorch_embers", Spawn::at(floor + Vec3::Y * 0.08));
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
        blows
    }

    /// The targeting circle, the cast's gathering fire, the meteors, their
    /// explosions, sparks, and scorch marks, seen from `eye`.
    pub fn draw(&self, mesh: &mut Mesh, eye: Vec3) {
        self.draw_over(mesh, eye, &height);
    }

    /// [`Self::draw`], with the circle laid over `surface`, the height of
    /// whatever is highest at a point: the ground, a roof, or a stage.
    pub fn draw_over(&self, mesh: &mut Mesh, eye: Vec3, surface: &dyn Fn(f32, f32) -> f32) {
        let glow = &mut mesh.glow;
        let pulse = 0.75 + 0.25 * (self.clock * 6.0).sin();
        if let Some(aim) = self.aimed() {
            let area = self.strike.area(aim.wall());
            ring(glow, aim, area, pulse, self.clock, self.strike, surface);
        }
        if let Some(cast) = self.casting {
            let k = (cast.elapsed / cast.strike.cast()).clamp(0.0, 1.0);
            let quick = 0.7 + 0.3 * (self.clock * (8.0 + 14.0 * k)).sin();
            ring(
                glow,
                cast.aim,
                cast.strike.area(cast.aim.wall()),
                quick * (1.0 + 1.5 * k),
                self.clock,
                cast.strike,
                surface,
            );
        }
        for bolt in &self.bolts {
            draw_bolt(&mut mesh.glow, bolt, eye);
        }
        for scorch in &self.scorches {
            scorch_mark(mesh, scorch);
        }
        self.fx.draw(&mut mesh.sprites);
        for meteor in self.meteors.iter().filter(|m| m.t > 0.0) {
            rock(mesh, meteor.at(meteor.t), self.clock);
        }
        if let Some(cast) = self.casting
            && cast.strike.lightning()
        {
            // The storm gathers over the target while the cast runs: a
            // dim cloud glow and sparks crawling in it.
            let k = (cast.elapsed / cast.strike.cast()).clamp(0.0, 1.0);
            let glow = &mut mesh.glow;
            let above = cast.aim.at + Vec3::Y * 22.0;
            blob(glow, above, 3.0 + 6.0 * k, tint(BOLT_GLOW, 4.0 * k), eye);
            for i in 0..5 {
                let angle = TAU * i as f32 / 5.0 + self.clock * 3.0;
                let at = above
                    + Vec3::new(
                        angle.cos(),
                        0.2 * (self.clock * 9.0 + i as f32).sin(),
                        angle.sin(),
                    ) * (4.0 - 2.5 * k);
                let on = (noise(i, (self.clock * 20.0) as u32) > 0.5) as u32 as f32;
                blob(
                    glow,
                    at,
                    0.4 + 0.4 * k,
                    tint(BOLT_CORE, BOLT_LUMINANCE * 0.3 * k * on),
                    eye,
                );
            }
        } else if let Some(cast) = self.casting {
            // The meteors gather high over the circle while the cast runs.
            let k = (cast.elapsed / CAST).clamp(0.0, 1.0);
            let glow = &mut mesh.glow;
            let above = cast.aim.at + Vec3::Y * 12.0;
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

/// How far a targeting ray looks for a surface, m: past [`RANGE`].
const LOOK: f32 = if LINE_OF_SIGHT { SIGHT } else { 90.0 };
/// How long one leg of a targeting ray is, m: the length every surface
/// test was tuned at.
const LEG: f32 = 90.0;
/// How far short of its end a meteor's or a bolt's path may first meet
/// something and still count as reaching it, m.
const SLACK: f32 = 2.2;
/// How far back along the targeting ray a strike that takes it starts, m,
/// at most.
const BACK: f32 = 40.0;

/// Where a ray from `origin` along `direction` first meets a surface, as
/// `cast` finds it: `cast(from, to)` is how far, from 0 to 1, the segment
/// from `from` to `to` goes before it meets something standing or the
/// ground, or `None` when it stays clear. On the ground, a roof, or a
/// floor, the point facing up; on a wall, the point and the wall's normal
/// turned toward the ray, so the inner face of a far wall seen through a
/// hole in the near one faces the caster too. A ray that meets nothing
/// lands on the ground under its heading, [`RANGE`] away.
#[must_use]
pub fn surface_aim(
    origin: Vec3,
    direction: Vec3,
    cast: &dyn Fn(Vec3, Vec3) -> Option<f32>,
) -> Option<Aim> {
    if !origin.is_finite() || !direction.is_finite() || direction.length_squared() < 1e-6 {
        return None;
    }
    let direction = direction.normalize();
    // In legs of [`LEG`] m, so a long look meets thin walls as a short one
    // does.
    let legs = (LOOK / LEG).ceil() as usize;
    let hit = (0..legs).find_map(|k| {
        let (a, b) = (k as f32 * LEG, ((k + 1) as f32 * LEG).min(LOOK));
        cast(origin + direction * a, origin + direction * b).map(|f| a + f * (b - a))
    });
    let Some(t) = hit else {
        let flat = Vec3::new(direction.x, 0.0, direction.z).try_normalize()?;
        let p = origin + flat * RANGE;
        return Some(Aim {
            view: direction,
            ..Aim::ground(Vec3::new(p.x, height(p.x, p.z), p.z))
        });
    };
    let at = origin + direction * t;
    // A top surface (the ground, a floor, a flat roof, a stair tread)
    // met by a ray coming down holds a short ray dropped onto it a little
    // nearer the caster: the ring lies flat there. Beside a wall the same
    // ray falls past the hit. (Pitched roofs are left to the normal.)
    let back = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero() * 0.3;
    let near = at - back;
    let (top, bottom) = (near + Vec3::Y * 0.6, near - Vec3::Y * 0.6);
    if direction.y < -0.02
        && let Some(f) = cast(top, bottom)
    {
        let y = top.y + (bottom.y - top.y) * f;
        if f > 0.0 && (y - at.y).abs() < 0.2 {
            return Some(Aim {
                view: direction,
                ..Aim::ground(Vec3::new(at.x, y, at.z))
            });
        }
    }
    let normal = face_normal(at, direction, t, cast);
    // The town's roofs are pitched up to about 55 degrees: still roofs.
    if normal.y >= 0.45 {
        return Some(Aim {
            view: direction,
            ..Aim::ground(at)
        });
    }
    // A wall's ring stands upright: its normal lies across the ground.
    let back = Vec3::new(-direction.x, 0.0, -direction.z).normalize_or(Vec3::X);
    let normal = Vec3::new(normal.x, 0.0, normal.z).normalize_or(back);
    Some(Aim {
        at,
        normal,
        view: direction,
    })
}

/// The normal of the surface the ray along `direction` met at `at`, `t` m
/// from its origin, turned toward the ray: the plane through the points
/// that rays beside it meet, a little to either side and above and below.
/// A ray beside it that misses the surface, through a hole's edge or past
/// a corner, is left out, and narrower ones are tried when too many miss.
fn face_normal(
    at: Vec3,
    direction: Vec3,
    t: f32,
    cast: &dyn Fn(Vec3, Vec3) -> Option<f32>,
) -> Vec3 {
    let (u, v) = beside(direction);
    let depth = 1.5_f32;
    let probe = |offset: Vec3| -> Option<Vec3> {
        let from = at + offset - direction * depth.min(t * 0.9);
        let to = at + offset + direction * depth;
        cast(from, to).map(|f| from.lerp(to, f))
    };
    let edge = |a: Option<Vec3>, b: Option<Vec3>| match (a, b) {
        (Some(a), Some(b)) => Some(a - b),
        (Some(a), None) => Some(a - at),
        (None, Some(b)) => Some(at - b),
        (None, None) => None,
    };
    for spread in [0.5, 0.25, 0.1] {
        let across = edge(probe(u * spread), probe(-u * spread));
        let up = edge(probe(v * spread), probe(-v * spread));
        if let (Some(across), Some(up)) = (across, up)
            && let Some(normal) = across.cross(up).try_normalize()
        {
            return if normal.dot(direction) > 0.0 {
                -normal
            } else {
                normal
            };
        }
    }
    -direction
}

/// Two unit directions at right angles to `direction` and each other.
fn beside(direction: Vec3) -> (Vec3, Vec3) {
    let u = direction.any_orthonormal_vector();
    (u, direction.cross(u))
}

/// Whether a strike from `start` to `end` for `aim` must take the
/// targeting ray instead: it has one, and something stands in the way.
fn blocked(site: &dyn Target, aim: Aim, start: Vec3, end: Vec3) -> bool {
    aim.view != Vec3::ZERO && !reaches(site, start, end)
}

/// Whether the path from `start` to `end` meets nothing until it is within
/// [`SLACK`] of `end`.
fn reaches(site: &dyn Target, start: Vec3, end: Vec3) -> bool {
    let beyond = end + (end - start).normalize_or_zero();
    site.ray(start, beyond)
        .is_none_or(|f| start.lerp(beyond, f).distance(end) <= SLACK)
}

/// A meteor's path to `point` on `aim`'s surface along the targeting ray,
/// ending `into` past the surface it meets there: the line through `point`
/// along the ray when a meteor's width passes along all of it, and else the
/// ray itself to `aim`'s point, which the caster saw, from short of
/// whatever stands behind the caster.
fn along_view(site: &dyn Target, aim: Aim, point: Vec3, into: Vec3) -> (Vec3, Vec3) {
    let view = aim.view;
    let land = |start: Vec3, p: Vec3| -> Option<(Vec3, Vec3)> {
        let beyond = p + view * 3.0;
        let hit = start.lerp(beyond, site.ray(start, beyond)?);
        (hit.distance(p) <= SLACK).then_some((start, hit + into))
    };
    // A meteor's sides must pass too, or it bursts on a hole's edge.
    let (u, v) = beside(view);
    let start = point - view * BACK;
    let clear = [u, -u, v, -v].iter().all(|&side| {
        let offset = side * METEOR_RADIUS * 0.8;
        reaches(site, start + offset, point + offset)
    });
    if clear && let Some(path) = land(start, point) {
        return path;
    }
    let start = way_back(site, aim.at, view, BACK);
    land(start, aim.at).unwrap_or((start, aim.at + into))
}

/// The point up to `back` m back from `point` against `view` that a strike
/// coming along `view` starts from: short of the first thing behind the
/// caster, so it never starts inside a building there, and high enough
/// over the ground that a meteor doesn't burst where it starts.
fn way_back(site: &dyn Target, point: Vec3, view: Vec3, back: f32) -> Vec3 {
    // From a little before the point, so the surface it is on doesn't
    // count.
    let near = point - view;
    let far = point - view * back;
    let mut start = match site.ray(near, far) {
        Some(f) => near.lerp(far, f) + view * 1.2,
        None => far,
    };
    let clearance = 3.0 * METEOR_RADIUS;
    let mut left = start.distance(near);
    while left > 0.5 && start.y < height(start.x, start.z) + clearance {
        start += view * 0.5;
        left -= 0.5;
    }
    start
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
    surface_hit(origin, direction, &height)
}

/// [`ground_hit`] against `surface`, the height of whatever is highest at
/// a point, so a ray at a roof meets the roof; the point is still on the
/// ground under it, where meteors aim.
#[must_use]
pub fn surface_hit(
    origin: Vec3,
    direction: Vec3,
    surface: &dyn Fn(f32, f32) -> f32,
) -> Option<Vec3> {
    if !origin.is_finite() || !direction.is_finite() || direction.length_squared() < 1e-6 {
        return None;
    }
    let direction = direction.normalize();
    let under = |p: Vec3| p.y <= surface(p.x, p.z);
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

/// The targeting ring of `radius` at `aim`: over `surface` on the ground,
/// or flat against a wall, in the colors of `strike`.
fn ring(
    out: &mut Vec<GlowVertex>,
    aim: Aim,
    radius: f32,
    level: f32,
    clock: f32,
    strike: Strike,
    surface: &dyn Fn(f32, f32) -> f32,
) {
    let start = out.len();
    if aim.wall() {
        wall_circle(out, aim, radius, level, clock);
    } else {
        // A ring a ray put on a floor under a roof stays on the floor.
        let under = (aim.view != Vec3::ZERO).then_some(aim.at.y);
        circle(out, aim.at, radius, level, clock, &|x, z| {
            let y = surface(x, z);
            match under {
                Some(floor) if y > floor + 2.5 => floor,
                _ => y,
            }
        });
    }
    if strike.lightning() {
        // Blue-white instead of fire.
        for v in &mut out[start..] {
            let l = v.radiance[0].max(v.radiance[1]).max(v.radiance[2]);
            v.radiance = [0.55 * l, 0.75 * l, 1.0 * l];
        }
    }
}

/// [`circle`] standing flat against a wall at `aim`, a little off its
/// face.
fn wall_circle(out: &mut Vec<GlowVertex>, aim: Aim, radius: f32, level: f32, clock: f32) {
    let (u, v) = aim.across();
    let center = aim.at + aim.normal * 0.08;
    let on = |angle: f32, r: f32| center + (u * angle.cos() + v * angle.sin()) * r;
    let flat = |out: &mut Vec<GlowVertex>, p: Vec3, half: f32, radiance: [f32; 3]| {
        quad(out, p, u * half, v * half, radiance);
    };
    let rim = 48;
    for i in 0..rim {
        let angle = TAU * i as f32 / rim as f32;
        flat(
            out,
            on(angle, radius),
            0.3,
            tint(RING, RING_LUMINANCE * level),
        );
    }
    let inner = 30;
    for i in 0..inner {
        let angle = TAU * i as f32 / inner as f32 - clock * 0.4;
        flat(
            out,
            on(angle, radius * 0.62),
            0.17,
            tint(YELLOW, RING_LUMINANCE * 0.5 * level),
        );
    }
    for i in 0..6 {
        let angle = TAU * i as f32 / 6.0 + clock * 0.6;
        for k in 0..4 {
            flat(
                out,
                on(angle, radius * (0.68 + 0.08 * k as f32)),
                0.2,
                tint(YELLOW, RING_LUMINANCE * 0.7 * level),
            );
        }
    }
    flat(
        out,
        center,
        radius,
        tint(RING, RING_LUMINANCE * 0.08 * level),
    );
    flat(out, center, 0.4, tint(YELLOW, RING_LUMINANCE * 0.6 * level));
}

/// A thunderbolt from the clouds to its target: a white core and a blue
/// glow along each jagged path, thinner on the branches, and a blinding
/// flash where it strikes, all flickering out over [`BOLT_LIFE`].
fn draw_bolt(out: &mut Vec<GlowVertex>, bolt: &Bolt, eye: Vec3) {
    let fade = 1.0 - (bolt.age / BOLT_LIFE).clamp(0.0, 1.0);
    let flicker = if bolt.age < 0.12 {
        1.0
    } else {
        0.35 + 0.65 * noise(bolt.seed, (bolt.age / 0.03) as u32)
    };
    let light = fade * fade * flicker;
    if light <= 0.0 {
        return;
    }
    // While the leader comes down, only the part above it shows.
    let reach = (bolt.age / BOLT_LEAD).clamp(0.0, 1.0);
    // The Mega Thunderbolt is twice as thick and brighter.
    let (thick, bright) = if bolt.mega { (2.2, 1.8) } else { (1.0, 1.0) };
    let light = light * bright;
    for (k, path) in bolt.paths().iter().enumerate() {
        let (core, wide) = if k == 0 { (0.32, 1.4) } else { (0.12, 0.6) };
        let (core, wide) = (core * thick, wide * thick);
        let shown = if k == 0 {
            ((path.len() - 1) as f32 * reach).ceil() as usize
        } else if reach < 1.0 {
            0
        } else {
            path.len() - 1
        };
        // Each glow is a round spot, so a line of them, overlapping, draws
        // the bolt's stroke.
        for pair in path.windows(2).take(shown) {
            let (a, b) = (pair[0], pair[1]);
            for (half, radiance) in [
                (wide, tint(BOLT_GLOW, BOLT_LUMINANCE * 0.12 * light)),
                (core, tint(BOLT_CORE, BOLT_LUMINANCE * 0.6 * light)),
            ] {
                let spots = ((a.distance(b) / (half * 0.9)).ceil() as usize).clamp(1, 64);
                for i in 0..spots {
                    let at = a.lerp(b, (i as f32 + 0.5) / spots as f32);
                    let toward = (eye - at).normalize_or(Vec3::Z);
                    let right = Vec3::Y.cross(toward).normalize_or(Vec3::X);
                    let up = toward.cross(right);
                    quad(out, at, right * half, up * half, radiance);
                }
            }
        }
    }
    if reach >= 1.0 {
        let flash = (1.0 - bolt.age / 0.25).clamp(0.0, 1.0);
        blob(
            out,
            bolt.aim.at + bolt.aim.normal * 0.5,
            (2.0 + 5.0 * flash) * thick,
            tint(BOLT_CORE, BOLT_LUMINANCE * 1.5 * flash * bright),
            eye,
        );
        blob(out, bolt.sky, 12.0, tint(BOLT_GLOW, 6.0 * light), eye);
    }
}

/// The targeting ring of `radius` at `at`, `level` times its plain glow,
/// laid over `surface`: a bright rim, a fainter inner ring, a soft fill,
/// and turning marks.
fn circle(
    out: &mut Vec<GlowVertex>,
    at: Vec3,
    radius: f32,
    level: f32,
    clock: f32,
    surface: &dyn Fn(f32, f32) -> f32,
) {
    let ground = |x: f32, z: f32| Vec3::new(x, surface(x, z) + 0.06, z);
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

/// Something of a strike that gives off light now, for a zone that lights
/// its stage with it (the Grove).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Glow {
    /// The fire or charge gathering over the aim while the cast runs, from
    /// 0 to 1 of the cast.
    Gathering {
        at: Vec3,
        strike: Strike,
        progress: f32,
    },
    /// A meteor's burning head.
    Meteor { at: Vec3 },
    /// A thunderbolt, `age` s after it left the clouds at `sky` for `at`.
    Bolt { at: Vec3, sky: Vec3, age: f32 },
}

impl Swarm {
    /// What gives off light now: the gathering cast, the falling meteors,
    /// and the bolts.
    #[must_use]
    pub fn glows(&self) -> Vec<Glow> {
        let mut out = Vec::new();
        if let Some(casting) = &self.casting {
            out.push(Glow::Gathering {
                at: casting.aim.at + casting.aim.normal * 1.5,
                strike: casting.strike,
                progress: (casting.elapsed / casting.strike.cast()).clamp(0.0, 1.0),
            });
        }
        for meteor in self.meteors.iter().filter(|m| (0.0..FALL).contains(&m.t)) {
            out.push(Glow::Meteor {
                at: meteor.at(meteor.t),
            });
        }
        for bolt in &self.bolts {
            out.push(Glow::Bolt {
                at: bolt.aim.at + bolt.aim.normal * 0.5,
                sky: bolt.sky,
                age: bolt.age,
            });
        }
        out
    }

    /// How long a bolt stays lit, s.
    #[must_use]
    pub const fn bolt_life() -> f32 {
        BOLT_LIFE
    }
}

#[cfg(test)]
impl Swarm {
    /// Seconds of cooldown left.
    pub fn cooldown_left(&self) -> f32 {
        self.cooldown
    }
}
