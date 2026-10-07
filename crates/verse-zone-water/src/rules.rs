//! The Water Lab on the shared spell rules (`docs/verse/water.md`, Spells
//! and water, phase W8): the lab's sea and pool as `physics::water` bodies
//! over its ground, and [`verse_world::spells::water`] deciding what every
//! spell does to them. The hotbar's Sleet Storm, Control Water, Create or
//! Destroy Water, and Water Walk run on these rules; [`Demo`] casts the
//! other spells that act on water (the freezing table, Gust of Wind,
//! Thunderwave, fire, Meteor Swarm, Reverse Gravity, and Fog Cloud) for
//! captures and tests.
//!
//! The renderer still draws one ice disc and one control area at a time
//! ([`verse_pbr::pbr::water::Controls`]), so the lab draws the strongest
//! ice patch; the rules hold every patch.

use std::sync::OnceLock;

use glam::{DVec2, DVec3, Vec2, Vec3};
use physics::water::{Kind, Level, Outline, WaterBody, WaterId, WaterSet, WaveSet};
use verse_core::fx::Spawn;
use verse_pbr::pbr::water::Disc;
use verse_pbr::water::Source;
use verse_world::spells::water::{
    Basin, Freeze, IceState, Shape, Wader, effects, ice::Bearing, ticks,
};

use crate::terrain::{self, LEVEL, POOL, POOL_LEVEL, POOL_RADIUS, ground};
use crate::{WaterLab, sea};

/// The sea's and the pool's body ids.
pub const SEA: WaterId = WaterId(0);
pub const POOL_BODY: WaterId = WaterId(1);
/// The player's creature id in the rules; dummies follow from
/// [`DUMMY_BASE`].
pub const PLAYER: u64 = 1;
pub const DUMMY_BASE: u64 = 100;
/// The lab's spell save DC: the Grove's.
pub const DC: i32 = verse_zone_grove::zones::grove::kit::SAVE_DC;

/// The lab's water for sea state `sea`: the bay seaward of the beach's
/// waterline, with the sea state's spectrum, and the plunge pool.
#[must_use]
pub fn water(sea: usize) -> &'static WaterSet {
    static SETS: OnceLock<Vec<WaterSet>> = OnceLock::new();
    let sets = SETS.get_or_init(|| (0..sea::SEAS.len()).map(build).collect());
    &sets[sea.min(sets.len() - 1)]
}

fn build(state: usize) -> WaterSet {
    let mut ring: Vec<DVec2> = (-30..=30)
        .map(|k| {
            let x = k as f32 * 5.0;
            DVec2::new(f64::from(x), f64::from(terrain::shoreline(x)))
        })
        .collect();
    ring.push(DVec2::new(150.0, -150.0));
    ring.push(DVec2::new(-150.0, -150.0));
    let waves = sea::spectrum(sea::SEAS[state])
        .and_then(|s| WaveSet::calm().with_spectrum(s).ok())
        .unwrap_or_default();
    let bay = WaterBody::new(
        SEA,
        Kind::Ocean,
        Outline::Polygon { points: ring },
        Level::Constant {
            height: f64::from(LEVEL),
        },
    )
    .with_density(physics::water::SALT)
    .with_waves(waves);
    let pool = WaterBody::pond(
        POOL_BODY,
        (0..16)
            .map(|k| {
                let a = k as f64 / 16.0 * std::f64::consts::TAU;
                DVec2::new(f64::from(POOL[0]), f64::from(POOL[1]))
                    + DVec2::new(a.cos(), a.sin()) * f64::from(POOL_RADIUS)
            })
            .collect(),
        f64::from(POOL_LEVEL),
    );
    WaterSet::new(vec![pool, bay], 8.0)
}

/// The lab's bed: its ground.
#[must_use]
pub fn bed(p: DVec2) -> f64 {
    f64::from(ground(p.x as f32, p.y as f32))
}

/// A spell the lab casts on its water outside the hotbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Demo {
    Freeze(Freeze),
    GustOfWind,
    Thunderwave,
    Fireball,
    MeteorSwarm,
    ReverseGravity,
    FogCloud,
}

impl Demo {
    pub const ALL: [Self; 12] = [
        Self::Freeze(Freeze::RayOfFrost),
        Self::Freeze(Freeze::IceKnife),
        Self::Freeze(Freeze::SleetStorm),
        Self::Freeze(Freeze::IceStorm),
        Self::Freeze(Freeze::ConeOfCold),
        Self::Freeze(Freeze::StormOfVengeance),
        Self::GustOfWind,
        Self::Thunderwave,
        Self::Fireball,
        Self::MeteorSwarm,
        Self::ReverseGravity,
        Self::FogCloud,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Freeze(f) => f.name(),
            Self::GustOfWind => "Gust of Wind",
            Self::Thunderwave => "Thunderwave",
            Self::Fireball => "Fireball",
            Self::MeteorSwarm => "Meteor Swarm",
            Self::ReverseGravity => "Reverse Gravity",
            Self::FogCloud => "Fog Cloud",
        }
    }

    /// How far ahead of the caster it lands, m.
    #[must_use]
    pub fn reach(self) -> f32 {
        match self {
            Self::Freeze(Freeze::ConeOfCold) | Self::GustOfWind | Self::Thunderwave => 0.0,
            Self::Freeze(Freeze::RayOfFrost | Freeze::IceKnife) => 8.0,
            Self::MeteorSwarm | Self::Freeze(Freeze::StormOfVengeance) => 22.0,
            _ => 14.0,
        }
    }
}

/// A spell that keeps acting each step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Running {
    Gust {
        origin: DVec3,
        dir: DVec2,
        until: f32,
    },
    Lift {
        center: DVec3,
        until: f32,
    },
}

impl WaterLab {
    /// The world tick the rules run on: the lab's clock.
    #[must_use]
    pub fn rules_tick(&self) -> u64 {
        ticks(f64::from(self.time))
    }

    /// The lab's water as the rules read it.
    #[must_use]
    pub fn basin(&self) -> Basin<'static> {
        Basin {
            water: water(self.sea),
            bed: &bed,
            seed: sea::SEED,
        }
    }

    /// The player and the dummies, where they stand.
    #[must_use]
    pub fn waders(&self) -> Vec<Wader> {
        let mut out = vec![Wader::new(PLAYER, self.caster.0.as_dvec3())];
        for (k, t) in self.targets.iter().enumerate() {
            out.push(Wader::new(DUMMY_BASE + k as u64, t.dummy.pos.as_dvec3()));
        }
        out
    }

    pub(crate) fn next_cast(&mut self) -> u64 {
        self.casts += 1;
        self.casts
    }

    /// Freezes the water under `spell` at `center` (Cone of Cold from the
    /// caster along `dir`) and starts its look.
    pub(crate) fn freeze(&mut self, spell: Freeze, center: Vec2, dir: Vec2, cast: u64) -> String {
        let basin = self.basin();
        let waders = self.waders();
        let tick = self.rules_tick();
        let result = self.rules.freeze(
            &basin,
            spell,
            center.as_dvec2(),
            dir.as_dvec2(),
            cast,
            DC,
            &waders,
            tick,
        );
        match result {
            Ok(_) => {
                let state = spell.row().stages[0].0;
                let at = Vec3::new(center.x, LEVEL, center.y);
                let effect = match spell {
                    Freeze::IceStorm => "grove_hail",
                    Freeze::ConeOfCold => "grove_cold_cone",
                    Freeze::SleetStorm | Freeze::StormOfVengeance => "grove_sleet",
                    Freeze::RayOfFrost | Freeze::IceKnife => "grove_ice_shatter",
                };
                let spawn = if spell == Freeze::ConeOfCold {
                    let d = dir.normalize_or(Vec2::NEG_Y);
                    Spawn::at(at + Vec3::Y * 1.2).moving(Vec3::new(d.x, 0.0, d.y) * 6.0)
                } else {
                    Spawn::at(at + Vec3::Y * 0.6)
                };
                self.fx.start(effect, spawn);
                self.sources.push(Source::impact(
                    center,
                    spell.row().radius as f32 * 0.5,
                    0.02,
                ));
                let held = self.rules.ice.held.len();
                let mut line = format!("{}: the water turns to {}", spell.name(), state.name());
                if spell.row().delay > 0.0 {
                    line = format!(
                        "{}: freezing rain glazes the water from round 5",
                        spell.name()
                    );
                }
                if held > 0 {
                    line.push_str(&format!("; {held} caught at the surface"));
                }
                line
            }
            Err(e) => format!("{}: {e}", spell.name()),
        }
    }

    /// Casts `spell` with the caster at `at` facing `forward`.
    pub fn cast_demo(&mut self, spell: Demo, at: Vec3, forward: Vec3) -> String {
        let flat = Vec2::new(forward.x, forward.z).normalize_or(Vec2::NEG_Y);
        let center = Vec2::new(at.x, at.z) + flat * spell.reach();
        let cast = self.next_cast();
        let basin = self.basin();
        let tick = self.rules_tick();
        let now = self.time;
        let line = match spell {
            Demo::Freeze(f) => self.freeze(f, center, flat, cast),
            Demo::GustOfWind => {
                let origin = at.as_dvec3() + DVec3::Y;
                self.running.push(Running::Gust {
                    origin,
                    dir: flat.as_dvec2(),
                    until: now + 6.0,
                });
                // Vapor in the line clears at once.
                self.rules.gust(
                    &mut self.floats.world,
                    &basin,
                    origin,
                    flat.as_dvec2(),
                    tick,
                );
                "Gust of Wind: a line of wind drives what floats 15 feet a round".into()
            }
            Demo::Thunderwave => {
                let (pushed, ring) = self.rules.thunderwave(
                    &mut self.floats.world,
                    &basin,
                    at.as_dvec3(),
                    flat.as_dvec2(),
                    tick,
                );
                if let Some(ring) = ring {
                    let c = Vec2::new(ring.at.x as f32, ring.at.z as f32);
                    self.sources.push(Source::impact(
                        c,
                        ring.radius as f32,
                        ring.wave as f32 * 0.25,
                    ));
                    self.water.add_ripple(c, ring.wave as f32 * 0.3);
                    self.fx
                        .start("water_splash", Spawn::at(ring.at.as_vec3()).scaled(1.6));
                }
                format!(
                    "Thunderwave: a ring wave shoves {} floating bodies",
                    pushed.len()
                )
            }
            Demo::Fireball => {
                let c = Vec3::new(center.x, LEVEL, center.y);
                let steam = self.rules.fire(&basin, c.as_dvec3(), 20.0 * 0.3048, tick);
                self.fx
                    .start("grove_flame_hit", Spawn::at(c + Vec3::Y).scaled(2.0));
                for p in &steam.at {
                    self.fx.start("water_steam", Spawn::at(p.as_vec3()));
                }
                format!(
                    "Fireball: steam where it meets the water; {} ice melted",
                    steam.melted
                )
            }
            Demo::MeteorSwarm => {
                let c = Vec3::new(center.x, LEVEL, center.y);
                let splash = self.rules.meteor(&basin, c.as_dvec3(), 12.0, tick);
                self.fx.start("meteor_explosion", Spawn::at(c));
                if let Some(s) = splash {
                    let p = Vec2::new(s.at.x as f32, s.at.z as f32);
                    self.sources
                        .push(Source::impact(p, 4.0, s.wave as f32 * 0.2));
                    self.water.add_ripple(p, 0.2);
                    self.fx
                        .start("water_splash", Spawn::at(s.at.as_vec3()).scaled(4.0));
                    for k in 0..6 {
                        let a = k as f32 / 6.0 * std::f32::consts::TAU;
                        let q = s.at.as_vec3() + Vec3::new(a.cos(), 0.0, a.sin()) * 5.0;
                        self.fx.start("water_steam", Spawn::at(q));
                    }
                }
                "Meteor Swarm: a crown splash, a ring wave, and steam; no fire on the water".into()
            }
            Demo::ReverseGravity => {
                self.running.push(Running::Lift {
                    center: DVec3::new(
                        f64::from(center.x),
                        f64::from(LEVEL) - 4.0,
                        f64::from(center.y),
                    ),
                    until: now + 4.0,
                });
                self.fx.start(
                    "water_crest_spray",
                    Spawn::at(Vec3::new(center.x, LEVEL, center.y)).scaled(3.0),
                );
                "Reverse Gravity: what floats falls upward; the water stays".into()
            }
            Demo::FogCloud => {
                let c = Vec3::new(center.x, LEVEL + 1.0, center.y);
                self.rules
                    .fog(c.as_dvec3(), 20.0 * 0.3048, tick + ticks(3600.0));
                self.fx.start("grove_fog", Spawn::at(c).scaled(2.0));
                "Fog Cloud: a fog bank on the water".into()
            }
        };
        self.say(line)
    }

    /// Advances the rules and the spells still acting.
    pub(crate) fn tick_rules(&mut self, dt: f32) {
        let tick = self.rules_tick();
        let basin = self.basin();
        self.rules.tick(tick);
        let now = self.time;
        self.running.retain(|r| match r {
            Running::Gust { until, .. } | Running::Lift { until, .. } => *until > now,
        });
        for r in self.running.clone() {
            match r {
                Running::Gust { origin, dir, .. } => {
                    let driven = self
                        .rules
                        .gust(&mut self.floats.world, &basin, origin, dir, tick);
                    if self.random() < 0.25 {
                        let along = self.random() * effects::GUST_LENGTH as f32;
                        let p =
                            origin.as_vec3() + Vec3::new(dir.x as f32, 0.0, dir.y as f32) * along;
                        let c = Vec2::new(p.x, p.z);
                        if self.surface_at(c).is_some() {
                            self.water.add_ripple(c, 0.02);
                            self.fx.start(
                                "water_crest_spray",
                                Spawn::at(Vec3::new(p.x, LEVEL, p.z))
                                    .moving(Vec3::new(dir.x as f32, 0.3, dir.y as f32) * 4.0),
                            );
                        }
                    }
                    let _ = driven;
                }
                Running::Lift { center, .. } => {
                    // The floats step several times a frame and forces
                    // last one step, so the reversed fall goes in as the
                    // frame's change of velocity.
                    let lifted = verse_world::spells::water::WaterSpells::reverse_gravity(
                        &mut self.floats.world,
                        center,
                        15.0,
                        30.0,
                    );
                    for id in lifted {
                        let b = &mut self.floats.world[id];
                        b.force = DVec3::ZERO;
                        b.vel.y += 2.0 * verse_world::spells::GRAVITY * f64::from(dt);
                    }
                }
            }
        }
    }

    /// The push the running gusts give the water at a point, for the
    /// floats.
    pub(crate) fn gust_push(&self) -> impl Fn(Vec3) -> Vec3 + use<> {
        let gusts: Vec<(Vec3, Vec2)> = self
            .running
            .iter()
            .filter_map(|r| match r {
                Running::Gust { origin, dir, .. } => Some((origin.as_vec3(), dir.as_vec2())),
                Running::Lift { .. } => None,
            })
            .collect();
        let speed = (effects::GUST_PUSH / f64::from(verse_world::spells::ROUND)) as f32;
        move |p: Vec3| {
            for (o, d) in &gusts {
                let local = Vec2::new(p.x - o.x, p.z - o.z);
                let along = local.dot(*d);
                if (0.0..=effects::GUST_LENGTH as f32).contains(&along)
                    && local.perp_dot(*d).abs() <= effects::GUST_WIDTH as f32 * 0.5
                {
                    return Vec3::new(d.x, 0.0, d.y) * speed;
                }
            }
            Vec3::ZERO
        }
    }

    /// The strongest ice the rules hold, as the renderer's disc.
    #[must_use]
    pub fn ice_disc(&self) -> Option<Disc> {
        let tick = self.rules_tick();
        let basin = self.basin();
        let mut best: Option<(f64, Disc)> = None;
        for patch in &self.rules.ice.patches {
            let (center, radius) = match patch.shape {
                Shape::Disc { center, radius } => (center, radius),
                Shape::Cone { apex, dir, length } => (
                    apex + dir.normalize_or(DVec2::X) * length * 0.55,
                    length * 0.45,
                ),
            };
            let mut amount = patch.amount(tick);
            if let Some(state) = self.rules.ice.state(&basin, center, tick)
                && state == IceState::Floes
            {
                amount = amount.min(0.6);
            }
            if amount <= 0.0 {
                continue;
            }
            if best.is_none_or(|(a, _)| amount > a) {
                best = Some((
                    amount,
                    Disc {
                        center: [center.x as f32, center.y as f32],
                        radius: radius as f32,
                        amount: amount as f32,
                    },
                ));
            }
        }
        best.map(|(_, d)| d)
    }

    /// What the ice at `p` does under the player: holds, or not.
    pub(crate) fn ice_holds(&mut self, p: Vec2) -> bool {
        let basin = self.basin();
        let tick = self.rules_tick();
        match self.rules.ice.bear(&basin, p.as_dvec2(), 75.0, tick) {
            Bearing::Holds => true,
            Bearing::Breaks => {
                self.say("The thin ice breaks under you".into());
                false
            }
            Bearing::Open | Bearing::Sinks => false,
        }
    }

    /// Whether the rules' ice pins a floating body at `p`.
    #[must_use]
    pub fn ice_pins(&self, p: Vec2) -> bool {
        self.rules
            .ice
            .pins(&self.basin(), p.as_dvec2(), self.rules_tick())
    }
}
