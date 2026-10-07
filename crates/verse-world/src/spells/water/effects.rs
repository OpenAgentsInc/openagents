//! What the spells that don't freeze water do to it
//! (`docs/verse/water.md`, What the other spells do to water). SRD 5.2.1
//! gives the pushes, the vapor and flame rules, Resistance to Fire under
//! water, and Create or Destroy Water's amounts; the rest is ours.

use glam::{DVec2, DVec3};
use physics::water::{Effect, Settings, Water};
use physics::{BodyId, World};
use serde::{Deserialize, Serialize};

use super::{Basin, WaterSpells, ticks};
use crate::spells::{FEET, GRAVITY};

/// Gust of Wind's line, m: 60 feet long and 10 feet wide.
pub const GUST_LENGTH: f64 = 60.0 * FEET;
pub const GUST_WIDTH: f64 = 10.0 * FEET;
/// How far Gust of Wind drives a floating object every round, and pushes
/// a creature that fails its save, m: 15 feet.
pub const GUST_PUSH: f64 = 15.0 * FEET;
/// Thunderwave's cube and push, m.
pub const THUNDERWAVE_CUBE: f64 = 15.0 * FEET;
pub const THUNDERWAVE_PUSH: f64 = 10.0 * FEET;
/// Thunderwave's ring wave, m (ours).
pub const THUNDERWAVE_WAVE: f64 = 0.3;
/// Meteor Swarm's ring wave, m (ours).
pub const METEOR_WAVE: f64 = 1.0;
/// Create or Destroy Water's cube, m: 30 feet.
pub const RAIN_CUBE: f64 = 30.0 * FEET;
/// How long Create Water's rain falls, s (ours).
pub const RAIN_TIME: f64 = 6.0;
/// Destroy Water's dimple on open water and how fast it refills, m and s
/// (ours).
pub const DIMPLE: f64 = 0.5;
pub const REFILL: f64 = 2.0;
/// The most a dam raises the water behind it, and how far up the stream
/// it reaches, m (ours).
pub const DAM_RISE: f64 = 0.3;
pub const DAM_REACH: f64 = 6.0;
/// Water Walk's rise from under water, m/s: 60 feet a round (SRD 5.2.1).
pub const WALK_RISE: f64 = 60.0 * FEET / 6.0;
/// Water Walk lasts an hour and Water Breathing a day, s.
pub const WALK_TIME: f64 = 3600.0;
pub const BREATHING_TIME: f64 = 24.0 * 3600.0;
/// Each targets up to ten willing creatures.
pub const TARGETS: usize = 10;
/// Plants grow from beds no deeper than wading depth, m (ours).
pub const PLANT_DEPTH: f64 = 1.25;
/// Steam lasts this long where fire meets water, s (ours).
pub const STEAM_TIME: f64 = 4.0;

/// Fog, steam, or mist: what wind clears and walls stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VaporKind {
    Fog,
    Steam,
    Mist,
}

/// A bank of vapor over the water.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vapor {
    pub kind: VaporKind,
    pub center: DVec3,
    pub radius: f64,
    pub until: u64,
}

/// Create Water's rain over a cube.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rain {
    pub center: DVec3,
    pub half: f64,
    pub until: u64,
}

/// A flame something might put out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flame {
    pub at: DVec3,
    /// A magical flame, which water doesn't put out.
    pub magical: bool,
    /// Shielded, as a lantern's.
    pub protected: bool,
}

/// What fire did where it met water.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Steam {
    /// Points where steam rises.
    pub at: Vec<DVec3>,
    /// How many ice patches it melted into.
    pub melted: usize,
}

/// A ring wave and splash a spell throws on the water, for the client's
/// ripple field and effects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Splash {
    pub at: DVec3,
    pub radius: f64,
    /// The ring wave's height, m.
    pub wave: f64,
    /// Fire met the water: steam and a hiss.
    pub hiss: bool,
}

/// Whether `p` lies in the box of half extents `half` about `center`, in
/// (x, z).
fn in_square(p: DVec2, center: DVec2, half: f64) -> bool {
    let d = (p - center).abs();
    d.x <= half && d.y <= half
}

/// Whether segment `a`–`b` crosses segment `c`–`d` in (x, z).
#[must_use]
pub fn crosses(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> bool {
    let o = |p: DVec2, q: DVec2, r: DVec2| (q - p).perp_dot(r - p);
    let (d1, d2) = (o(c, d, a), o(c, d, b));
    let (d3, d4) = (o(a, b, c), o(a, b, d));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// The bodies in `world` floating in `water` at `tick`: touching it and
/// not resting on anything else.
#[must_use]
pub fn floating(world: &World, water: &impl Water, tick: u64) -> Vec<BodyId> {
    world
        .bodies()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == physics::BodyKind::Dynamic && !b.removed)
        .filter(|(_, b)| {
            water
                .sample(b.pos.x, b.pos.z, tick)
                .is_some_and(|s| b.pos.y < s.height + 0.6)
        })
        .map(|(i, _)| BodyId(i as u32))
        .collect()
}

/// Steps the floating bodies of `world` in `basin`'s water as the spells
/// leave it at `tick` (buoyancy, drag, the current), for `dt`. Bodies the
/// ice pins are held where they are.
pub fn float(world: &mut World, spells: &WaterSpells, basin: &Basin, tick: u64, dt: f64) {
    let water = spells.events.over(basin.water);
    physics::water::apply_with(world, &water, tick, dt, &Settings::default());
    for i in 0..world.bodies().len() {
        let b = &world.bodies()[i];
        if !b.responds() || b.removed {
            continue;
        }
        let p = DVec2::new(b.pos.x, b.pos.z);
        if spells.ice.pins(basin, p, tick) {
            let body = &mut world.bodies_mut()[i];
            body.vel = DVec3::ZERO;
            body.omega = DVec3::ZERO;
            body.force = DVec3::ZERO;
            body.torque = DVec3::ZERO;
        }
    }
}

/// Carries `body` along at `velocity`: the wind sets the part of its
/// horizontal velocity along the push, as Gust of Wind's field sets a
/// creature's (ours for floating objects).
fn drive(world: &mut World, body: BodyId, velocity: DVec2) {
    let b = &mut world[body];
    let speed = velocity.length();
    if speed <= 0.0 || b.inverse_mass() <= 0.0 {
        return;
    }
    let dir = velocity / speed;
    let along = b.vel.x * dir.x + b.vel.z * dir.y;
    if along < speed {
        let kick = dir * (speed - along);
        b.vel += DVec3::new(kick.x, 0.0, kick.y);
    }
}

impl WaterSpells {
    /// Gust of Wind's line from `origin` along `dir` for a step of `dt` at
    /// `tick`: floating objects in it drift 15 feet a round along it, and
    /// fog, steam, and mist in it clear (SRD for vapor; ours for floating
    /// objects). Returns the bodies it drove.
    pub fn gust(
        &mut self,
        world: &mut World,
        basin: &Basin,
        origin: DVec3,
        dir: DVec2,
        tick: u64,
    ) -> Vec<BodyId> {
        let dir = dir.normalize_or(DVec2::X);
        let o = DVec2::new(origin.x, origin.z);
        let inside = |p: DVec2| {
            let local = p - o;
            let along = local.dot(dir);
            (0.0..=GUST_LENGTH).contains(&along) && local.perp_dot(dir).abs() <= GUST_WIDTH * 0.5
        };
        self.vapors
            .retain(|v| !inside(DVec2::new(v.center.x, v.center.z)));
        let water = self.events.over(basin.water);
        let bodies: Vec<BodyId> = floating(world, &water, tick)
            .into_iter()
            .filter(|id| {
                let p = world[*id].pos;
                inside(DVec2::new(p.x, p.z))
            })
            .collect();
        let speed = GUST_PUSH / f64::from(crate::spells::ROUND);
        for id in &bodies {
            drive(world, *id, dir * speed);
        }
        bodies
    }

    /// Where Gust of Wind or Thunderwave pushes a swimmer at `feet` that
    /// failed its save: `distance` m along `dir`.
    #[must_use]
    pub fn shove(feet: DVec3, dir: DVec2, distance: f64) -> DVec3 {
        let d = dir.normalize_or(DVec2::X) * distance;
        DVec3::new(feet.x + d.x, feet.y, feet.z + d.y)
    }

    /// Thunderwave from a caster at `caster` facing `dir`: floating objects
    /// wholly in the 15-foot cube are pushed 10 feet away from the caster,
    /// and a 0.3 m ring wave runs out from the cube's center (ours).
    pub fn thunderwave(
        &mut self,
        world: &mut World,
        basin: &Basin,
        caster: DVec3,
        dir: DVec2,
        tick: u64,
    ) -> (Vec<BodyId>, Option<Splash>) {
        let dir = dir.normalize_or(DVec2::X);
        let c = DVec2::new(caster.x, caster.z);
        let center = c + dir * THUNDERWAVE_CUBE * 0.5;
        let water = self.events.over(basin.water);
        let bodies: Vec<BodyId> = floating(world, &water, tick)
            .into_iter()
            .filter(|id| {
                let p = world[*id].pos;
                in_square(DVec2::new(p.x, p.z), center, THUNDERWAVE_CUBE * 0.5)
            })
            .collect();
        for id in &bodies {
            let p = world[*id].pos;
            let away = (DVec2::new(p.x, p.z) - c).normalize_or(dir);
            // The speed that coasts 10 feet against linear drag in about a
            // round, as an impulse.
            let b = &mut world[*id];
            let kick = away * THUNDERWAVE_PUSH / 2.0;
            b.vel += DVec3::new(kick.x, 0.0, kick.y);
        }
        let splash = self.sample(basin, center, tick).map(|s| Splash {
            at: DVec3::new(center.x, s.height, center.y),
            radius: THUNDERWAVE_CUBE * 0.5,
            wave: THUNDERWAVE_WAVE,
            hiss: false,
        });
        (bodies, splash)
    }

    /// Whether Wind Wall from `a` to `b` stops something moving from
    /// `from` to `to`: fog, steam, and floating Small or smaller objects
    /// can't cross it; swimmers can (SRD for vapor and small objects, ours
    /// for applying it to floating ones).
    #[must_use]
    pub fn wind_wall_blocks(
        a: DVec2,
        b: DVec2,
        from: DVec2,
        to: DVec2,
        small_or_vapor: bool,
    ) -> bool {
        small_or_vapor && crosses(a, b, from, to)
    }

    /// Fire whose area is the disc at `center` meets the water at `tick`:
    /// steam where it touches water, and ice in it melts at once (ours).
    pub fn fire(&mut self, basin: &Basin, center: DVec3, radius: f64, tick: u64) -> Steam {
        let c = DVec2::new(center.x, center.z);
        let melted = self.ice.melt(c, radius, tick);
        let mut at = Vec::new();
        for k in 0..9 {
            let p = if k == 0 {
                c
            } else {
                let a = k as f64 / 8.0 * std::f64::consts::TAU;
                c + DVec2::new(a.cos(), a.sin()) * radius * 0.7
            };
            if let Some(s) = self.sample(basin, p, tick) {
                at.push(DVec3::new(p.x, s.height, p.y));
            }
        }
        if let Some(first) = at.first()
            && self.vapors.len() < 128
        {
            self.vapors.push(Vapor {
                kind: VaporKind::Steam,
                center: *first,
                radius: radius.min(12.0),
                until: tick + ticks(STEAM_TIME),
            });
        }
        Steam { at, melted }
    }

    /// A Meteor Swarm sphere of `radius` m at `center` meets the water: a
    /// crown splash, a 1 m ring wave, steam, and its ice melts; there is no
    /// ground fire on water (ours).
    pub fn meteor(
        &mut self,
        basin: &Basin,
        center: DVec3,
        radius: f64,
        tick: u64,
    ) -> Option<Splash> {
        let steam = self.fire(basin, center, radius, tick);
        let at = *steam.at.first()?;
        Some(Splash {
            at,
            radius,
            wave: METEOR_WAVE,
            hiss: true,
        })
    }

    /// Marks object `id` as in water at `tick`.
    pub fn soak(&mut self, id: u64, tick: u64) {
        if self.wet.len() < 1024 || self.wet.contains_key(&id) {
            self.wet.insert(id, tick);
        }
    }

    /// Whether fire can set object `id` alight at `tick`: not if it was in
    /// water in the last minute (ours).
    #[must_use]
    pub fn ignites(&self, id: u64, tick: u64) -> bool {
        self.wet
            .get(&id)
            .is_none_or(|at| tick >= at.saturating_add(ticks(60.0)))
    }

    /// Reverse Gravity's cylinder of `radius` m about `center` for a step:
    /// floating bodies in it fall upward, buoyancy or not (SRD 5.2.1). The
    /// water itself stays (ours).
    pub fn reverse_gravity(
        world: &mut World,
        center: DVec3,
        radius: f64,
        height: f64,
    ) -> Vec<BodyId> {
        let mut lifted = Vec::new();
        for i in 0..world.bodies().len() {
            let b = &mut world.bodies_mut()[i];
            if b.kind != physics::BodyKind::Dynamic || b.removed || b.inverse_mass() <= 0.0 {
                continue;
            }
            let d = DVec2::new(b.pos.x - center.x, b.pos.z - center.z).length();
            if d > radius || b.pos.y < center.y - 1.0 || b.pos.y > center.y + height {
                continue;
            }
            // Cancel gravity and apply it reversed.
            b.wake();
            let mass = 1.0 / b.inverse_mass();
            b.force += DVec3::Y * (2.0 * GRAVITY * mass);
            lifted.push(BodyId(i as u32));
        }
        lifted
    }

    /// Wall of Stone's panels from `a` to `b` stand on the bed; across
    /// water they dam it: the current turns along the wall, and the level
    /// behind it (to the wall's left, looking from `a` to `b`) rises at
    /// most 0.3 m (ours). Returns the dam's event, if it met water.
    pub fn dam(&mut self, basin: &Basin, cast: u64, a: DVec2, b: DVec2, tick: u64) -> Option<u64> {
        let body = (0..=8).find_map(|k| {
            let p = a.lerp(b, k as f64 / 8.0);
            basin.sample_base(p, tick).map(|s| s.body)
        })?;
        if self.dams.len() >= 32 {
            return None;
        }
        let id = self
            .start(
                body,
                tick,
                ticks(crate::spells::ROUND.into()),
                u64::MAX,
                Effect::Dam {
                    a,
                    b,
                    rise: DAM_RISE,
                    reach: DAM_REACH,
                },
            )
            .ok()?;
        self.dams.push((cast, id));
        Some(id)
    }

    /// Create Water's rain on the 30-foot cube at `center` for 6 s: it
    /// puts out the nonmagical, unprotected flames there (SRD 5.2.1).
    /// Returns which of `flames` went out.
    pub fn create_water(&mut self, center: DVec3, flames: &[Flame], tick: u64) -> Vec<usize> {
        if self.rains.len() < 32 {
            self.rains.push(Rain {
                center,
                half: RAIN_CUBE * 0.5,
                until: tick + ticks(RAIN_TIME),
            });
        }
        let c = DVec2::new(center.x, center.z);
        flames
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                !f.magical
                    && !f.protected
                    && in_square(DVec2::new(f.at.x, f.at.z), c, RAIN_CUBE * 0.5)
                    && (f.at.y - center.y).abs() <= RAIN_CUBE * 0.5
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Whether rain falls on `p` at `tick`.
    #[must_use]
    pub fn raining(&self, p: DVec2, tick: u64) -> bool {
        self.rains
            .iter()
            .any(|r| r.until > tick && in_square(p, DVec2::new(r.center.x, r.center.z), r.half))
    }

    /// Destroy Water on the 30-foot cube at `center`: fog, steam, and mist
    /// there clear (SRD 5.2.1), and on open water a 0.5 m dimple opens and
    /// refills over 2 s (ours). Returns how many banks it cleared.
    pub fn destroy_water(&mut self, basin: &Basin, center: DVec3, tick: u64) -> usize {
        let c = DVec2::new(center.x, center.z);
        let before = self.vapors.len();
        self.vapors
            .retain(|v| !in_square(DVec2::new(v.center.x, v.center.z), c, RAIN_CUBE * 0.5));
        if let Some(s) = basin.sample_base(c, tick) {
            let _ = self.start(
                s.body,
                tick,
                ticks(REFILL),
                0,
                Effect::Bowl {
                    center: c,
                    radius: RAIN_CUBE * 0.5,
                    depth: DIMPLE,
                },
            );
        }
        before - self.vapors.len()
    }

    /// Water Walk on up to ten `creatures` at `tick`, for an hour.
    pub fn water_walk(&mut self, creatures: &[u64], tick: u64) {
        for id in creatures.iter().take(TARGETS) {
            self.walkers.insert(*id, tick + ticks(WALK_TIME));
        }
    }

    /// Water Breathing on up to ten `creatures` at `tick`, for a day.
    pub fn water_breathing(&mut self, creatures: &[u64], tick: u64) {
        for id in creatures.iter().take(TARGETS) {
            self.breathers.insert(*id, tick + ticks(BREATHING_TIME));
        }
    }

    /// A water walker's feet at `feet` for a step of `dt` at `tick`: on the
    /// gameplay surface, riding the swell; from under water it rises at
    /// 60 feet a round. None when it isn't walking or there is no water.
    #[must_use]
    pub fn walk_step(
        &self,
        basin: &Basin,
        creature: u64,
        feet: DVec3,
        dt: f64,
        tick: u64,
    ) -> Option<DVec3> {
        if !self.walks(creature, tick) {
            return None;
        }
        let s = self.sample(basin, DVec2::new(feet.x, feet.z), tick)?;
        let y = if feet.y < s.height {
            (feet.y + WALK_RISE * dt).min(s.height)
        } else {
            s.height
        };
        Some(DVec3::new(feet.x, y, feet.z))
    }

    /// Fog Cloud over the water: a bank on the surface (ours: the water and
    /// what lies under it are unchanged).
    pub fn fog(&mut self, center: DVec3, radius: f64, until: u64) {
        if self.vapors.len() < 128 {
            self.vapors.push(Vapor {
                kind: VaporKind::Fog,
                center,
                radius,
                until,
            });
        }
    }
}

/// Whether Entangle, Spike Growth, or Wall of Thorns grows at `p`: from dry
/// ground, or a bed no deeper than wading depth (ours).
#[must_use]
pub fn plants_grow(basin: &Basin, p: DVec2) -> bool {
    basin.depth(p).is_none_or(|d| d <= PLANT_DEPTH)
}

/// Whether a Web at `p` holds: over open water nothing anchors it, so it
/// collapses (SRD 5.2.1), unless `anchored` between solid masses.
#[must_use]
pub fn web_holds(basin: &Basin, p: DVec2, anchored: bool) -> bool {
    anchored || basin.sample_base(p, 0).is_none()
}

/// Whether a caster teleported to `p` (Misty Step, Tree Stride) arrives
/// swimming: over water deeper than wading (ours).
#[must_use]
pub fn arrives_swimming(basin: &Basin, p: DVec2) -> bool {
    basin
        .depth(p)
        .is_some_and(|d| d > physics::water::medium::SWIM_DEPTH)
}

/// A fall's damage when it ends in water under Feather Fall: none (SRD
/// 5.2.1 Feather Fall takes no falling damage).
#[must_use]
pub const fn feather_fall_damage(_into_water: bool) -> u32 {
    0
}
