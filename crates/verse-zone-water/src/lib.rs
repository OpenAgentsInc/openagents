//! The Water Lab: a cove at golden hour to show off water (`docs/verse/water.md`,
//! phase W0, and `docs/verse/zones.md`, Water Lab).
//!
//! A sandy bay opens south between two rocky headlands. Its sea rolls in
//! as a Gerstner swell, clear over the sand and deep blue past the reef,
//! with caustics on the bed, foam along the shore, and the low sun's glitter
//! on the waves. In the west a river runs across a plateau, falls over a
//! cliff into a plunge pool, and crosses the beach to the sea. The renderer
//! draws all of it in its water pass ([`verse_pbr::pbr::water`]).
//!
//! The player walks Everglade's character in from the beach, wades, swims,
//! and with Water Breathing dives. Crates, barrels, and planks float as
//! rigid bodies in the shared `physics` crate ([`floats`]): `B` drops one
//! ahead, and it splashes, bobs, tilts, and drifts with the waves and the
//! current. The hotbar holds the water spells ([`spells`]), the Water Orb
//! ([`orb`]), and the Grove's Thunderbolt ([`bolt`]), and training dummies
//! stand on the beach and in the water ([`targets`]); `T` turns the hour
//! between golden hour and noon.
//!
//! [`terrain`] is the ground; [`sea`] the water's rest shape and light.
//! `verse` re-exports this crate as `zones::water`.

pub mod bolt;
pub mod floats;
pub mod hotbar;
pub mod orb;
pub mod sea;
pub mod spells;
pub mod targets;
pub mod terrain;
#[cfg(test)]
mod tests;

use std::sync::Arc;

use glam::{Vec2, Vec3};
use physics::BodyId;
use verse_core::fx::{Particles, Spawn};
use verse_core::world::World;
use verse_pbr::mesh::Mesh;
use verse_pbr::pbr::textured::Figure;
use verse_pbr::pbr::water::{Controls, Disc, Flow, Part, Water, Whirl};
use verse_world::spells::Dice;
use verse_zone_everglade::zones::everglade;
use verse_zone_everglade::zones::everglade::floaters::Floater;
use verse_zone_everglade::zones::everglade::layout::{Collision, Placement};
use verse_zone_everglade::zones::everglade_pack::ZonePack;
use verse_zone_grove::zones::grove::draw::{Model, Painter};

pub use floats::{Floats, Kind as FloatKind};
pub use sea::Hour;
pub use spells::{Mode, Slot, Spells};
pub use terrain::{LEVEL, ground};

/// The zone's walkable square's half extent, m.
pub const HALF_EXTENT: f32 = 150.0;
/// Where the player arrives: on the beach, facing the sea and the sun.
pub const SPAWN: Vec3 = Vec3::new(4.0, 0.0, 16.0);
pub const SPAWN_YAW: f32 = std::f32::consts::PI;
/// The way out: a lantern post at the head of the beach.
pub const EXIT: Vec3 = Vec3::new(10.0, 0.0, 22.0);
/// How near the post the player must stand to leave, m.
pub const EXIT_REACH: f32 = 2.2;

/// Whether `at` is in reach of the way out.
#[must_use]
pub fn near_exit(at: Vec3) -> bool {
    Vec2::new(at.x - EXIT.x, at.z - EXIT.z).length() < EXIT_REACH
}

/// The spawn on the ground.
#[must_use]
pub fn spawn() -> Vec3 {
    Vec3::new(SPAWN.x, ground(SPAWN.x, SPAWN.z), SPAWN.z)
}

/// The pack's props around the cove: boulders on the headlands and in the
/// surf, rocks in the stream, pines and trees on the hills, bushes and
/// grass behind the beach, and the lantern post at the way out. Each is
/// lifted from Everglade's ground onto the cove's.
#[must_use]
pub fn placements() -> Vec<Placement> {
    let mut out = Vec::new();
    let mut put = |model: &'static str,
                   x: f32,
                   z: f32,
                   yaw: f32,
                   scale: f32,
                   sink: f32,
                   collision: Collision| {
        let lift = ground(x, z) - everglade::height(x, z) - sink;
        out.push(
            Placement::new(model, [x, z], yaw, collision)
                .lift(lift)
                .scale(scale),
        );
    };
    let rocks = [
        "nature/Rock_Medium_1",
        "nature/Rock_Medium_2",
        "nature/Rock_Medium_3",
    ];
    // Boulders along both headlands, and a few out in the surf.
    let mut n = 0u32;
    for side in [-1.0f32, 1.0] {
        for k in 0..11 {
            let x = side * (40.0 + k as f32 * 4.3 + terrain::noise(k as f32, side, 3) * 3.0);
            let z = terrain::shoreline(x) + 2.0 - terrain::noise(k as f32, side, 5) * 9.0;
            let scale = 2.2 + 2.8 * terrain::noise(k as f32 * 1.7, side, 7);
            put(
                rocks[(n % 3) as usize],
                x,
                z,
                n as f32 * 1.3,
                scale,
                0.5,
                Collision::Core(0.5),
            );
            n += 1;
        }
    }
    for (x, z, s) in [
        (-12.0, -14.0, 2.6),
        (21.0, -9.0, 1.8),
        (14.0, -27.5, 3.0),
        (-24.0, -26.0, 2.2),
    ] {
        put(
            rocks[(n % 3) as usize],
            x,
            z,
            n as f32 * 0.9,
            s,
            0.4,
            Collision::Core(0.5),
        );
        n += 1;
    }
    // Rocks in the stream and around the pool, which the water foams past.
    for &(x, z, s) in STREAM_ROCKS {
        put(
            rocks[(n % 3) as usize],
            x,
            z,
            n as f32 * 2.1,
            s,
            0.3,
            Collision::Core(0.45),
        );
        n += 1;
    }
    // Pines on the plateau and trees on the hills.
    for k in 0..60u32 {
        let x = -110.0 + terrain::noise(k as f32 * 3.1, 0.5, 11) * 220.0;
        let z = 40.0 + terrain::noise(k as f32 * 2.3, 1.5, 13) * 90.0;
        let h = ground(x, z);
        if h < 3.0 || terrain::fresh_water(Vec2::new(x, z)).is_some() {
            continue;
        }
        if terrain::nearest(&terrain::UPPER, Vec2::new(x, z)).is_some_and(|(d, _, _)| d < 6.0)
            || terrain::nearest(&terrain::LOWER, Vec2::new(x, z)).is_some_and(|(d, _, _)| d < 5.0)
            || Vec2::new(x, z).distance(Vec2::new(-46.0, 54.0)) < 10.0
        {
            continue;
        }
        let model = match k % 5 {
            0 | 1 => "nature/Pine_1",
            2 => "nature/Pine_2",
            3 => "nature/CommonTree_3",
            _ => "nature/CommonTree_4",
        };
        let scale = 1.0 + terrain::noise(k as f32, 9.0, 17) * 0.6;
        put(
            model,
            x,
            z,
            k as f32 * 0.7,
            scale,
            0.1,
            Collision::Core(0.3),
        );
    }
    // Bushes, ferns, and grass where the beach meets the hills.
    for k in 0..90u32 {
        let x = -70.0 + terrain::noise(k as f32 * 1.9, 2.5, 19) * 140.0;
        let s = 12.0 + terrain::noise(k as f32 * 2.7, 3.5, 23) * 30.0;
        let z = terrain::shoreline(x) + s;
        if ground(x, z) < 1.2 || terrain::fresh_water(Vec2::new(x, z)).is_some() {
            continue;
        }
        let model = match k % 6 {
            0 => "nature/Bush_Common",
            1 => "nature/Bush_Common_Flowers",
            2 => "nature/Fern_1",
            3 => "nature/Grass_Common_Tall",
            4 => "nature/Grass_Wispy_Short",
            _ => "nature/Plant_1",
        };
        put(model, x, z, k as f32 * 1.1, 1.0, 0.05, Collision::None);
    }
    put(
        "props/Lantern_Wall",
        EXIT.x,
        EXIT.z,
        0.0,
        1.4,
        0.0,
        Collision::None,
    );
    out
}

/// Rocks in the lower stream and at the pool's rim: x, z, and scale.
pub const STREAM_ROCKS: &[(f32, f32, f32)] = &[
    (-40.6, 37.0, 0.9),
    (-42.3, 39.5, 0.7),
    (-37.0, 24.5, 0.8),
    (-38.6, 27.4, 0.6),
    (-35.3, 14.5, 0.7),
    (-50.5, 49.0, 1.6),
    (-41.2, 53.0, 1.4),
];

/// The cove: the ground, the pack's props, and the water at rest.
///
/// # Errors
///
/// Returns a message when the pack lacks a placed model or the scene
/// exceeds the renderer's bounds.
pub fn world(pack: &ZonePack) -> Result<World, String> {
    let mut world = World::default();
    let (mut scene, blockers) = everglade::scene::build(pack, &placements())?;
    terrain::add_terrain(&mut scene);
    scene.validate()?;
    world.mesh.textured = Some(Arc::new(scene));
    let surface = sea::surface();
    surface.validate()?;
    world.mesh.water = Some(Arc::new(surface));
    world.blockers = blockers;
    Ok(world)
}

/// What the character stands on in the cove: the ground and the props.
///
/// # Errors
///
/// Returns a message when the pack lacks a placed model.
pub fn solids(pack: &ZonePack) -> Result<everglade::solids::Solids, String> {
    let mut solids = everglade::solids::Solids::over(ground);
    for placement in placements() {
        if placement.collision == Collision::None {
            continue;
        }
        let (blocks, _) = everglade::solids::of_placement(pack, &placement)?;
        for (footprint, top) in blocks {
            solids.add_block(footprint, top);
        }
    }
    Ok(solids)
}

/// What the lab asks of the character each frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Feet {
    /// The lowest the character's feet may be, m: the water under Water
    /// Walk, the swimming level, or ice. Below it the character rises.
    pub floor: Option<f32>,
    /// The character's movement speed multiplier.
    pub pace: f32,
}

/// The live lab.
pub struct WaterLab {
    pub time: f32,
    pub hour: Hour,
    pub water: Water,
    pub floats: Floats,
    pub spells: Spells,
    fx: Particles,
    next_kind: usize,
    stride: f32,
    bubbles: f32,
    /// Breath left, s.
    pub breath: f32,
    last: Option<Vec3>,
    /// The player's speed over the last frame, m/s.
    speed: f32,
    rain_wait: f32,
    /// The rain's and the sleet's running effects.
    rain_fx: Vec<verse_core::fx::Handle>,
    sleet_fx: Vec<verse_core::fx::Handle>,
    /// The last few things that happened, newest last.
    pub log: Vec<String>,
    seed: u32,
    /// What the character stands on this frame ([`Feet::floor`]), which
    /// the runtime applies after each movement step.
    pub floor: Option<f32>,
    /// The Water Orbs: the one forming in front of the caster, those in
    /// flight, and those set hovering ([`orb`]).
    pub orbs: Vec<orb::Orb>,
    /// The water burst orbs threw up, still in the air.
    spills: Vec<orb::Spill>,
    next_orb: u32,
    feed_wait: f32,
    /// The training dummies ([`targets`]).
    pub targets: Vec<targets::Target>,
    /// Numbers and words floating up from hits.
    pub floaters: Vec<Floater>,
    /// Thunderbolts striking now ([`bolt`]), and lightning running over
    /// the water from where one struck it, and when.
    pub bolts: Vec<bolt::Bolt>,
    surges: Vec<(Vec3, f32)>,
    /// How brightly lightning lights the sky, 0 to 1.
    pub flash: f32,
    /// The dice the rules roll behind the scenes.
    dice: Dice,
    /// The pointer's ray, origin and direction, while the app aims with it.
    aim: Option<(Vec3, Vec3)>,
    /// The caster's feet and facing at the last tick.
    caster: (Vec3, Vec3),
    /// Floating bodies lightning jolts, and until when.
    jolts: Vec<(BodyId, f32)>,
    /// The dummies' figure beside the character's, once the runtime sets
    /// it from the pack.
    model: Option<Model>,
}

/// Floating numbers at once, oldest dropped first.
pub const MAX_FLOATERS: usize = 48;

impl Default for WaterLab {
    fn default() -> Self {
        Self::new()
    }
}

/// The lab's own view of the water, for the floating bodies.
struct Medium<'a> {
    lab: &'a WaterLab,
    water: &'a Water,
}

impl floats::Medium for Medium<'_> {
    fn surface(&self, p: Vec2) -> Option<floats::Surface> {
        self.lab.surface_with(self.water, p)
    }

    fn ground(&self, p: Vec2) -> f32 {
        ground(p.x, p.y)
    }
}

impl WaterLab {
    /// The lab with a few bodies already afloat in the bay and the falls'
    /// spray running.
    #[must_use]
    pub fn new() -> Self {
        let mut lab = Self {
            time: 0.0,
            hour: Hour::Golden,
            water: sea::water(),
            floats: Floats::new(),
            spells: Spells::default(),
            fx: Particles::new(7),
            next_kind: 0,
            stride: 0.0,
            bubbles: 0.0,
            breath: spells::breath_limit(spells::CON_MODIFIER),
            last: None,
            speed: 0.0,
            rain_wait: 0.0,
            rain_fx: Vec::new(),
            sleet_fx: Vec::new(),
            log: Vec::new(),
            seed: 1,
            floor: None,
            orbs: Vec::new(),
            spills: Vec::new(),
            next_orb: 1,
            feed_wait: 0.0,
            targets: targets::Target::field(),
            floaters: Vec::new(),
            bolts: Vec::new(),
            surges: Vec::new(),
            flash: 0.0,
            dice: Dice::new(0x0057_A7E2),
            aim: None,
            caster: (SPAWN, Vec3::NEG_Z),
            jolts: Vec::new(),
            model: None,
        };
        lab.fx.start("water_falls_spray", Spawn::at(sea::landing()));
        for (k, (x, z)) in [
            (2.0, -6.0),
            (-5.0, -10.0),
            (8.0, -14.0),
            (-1.0, -18.0),
            (12.0, -4.0),
        ]
        .into_iter()
        .enumerate()
        {
            let kind = FloatKind::ALL[k % 3];
            lab.floats
                .spawn(kind, Vec3::new(x, 0.3, z), k as f32 * 1.1, Vec3::ZERO);
        }
        for _ in 0..90 {
            lab.tick_world(1.0 / 30.0);
        }
        lab
    }

    /// The water as it is this frame: the swell, the ripples, and what the
    /// spells do to it.
    #[must_use]
    pub fn frame_water(&self) -> Water {
        let mut water = self.water;
        water.time = self.time;
        let s = &self.spells;
        water.level = LEVEL + s.rise;
        water.rest = LEVEL;
        water.swell_gain = s.swell;
        let mut controls = Controls::default();
        if s.part > 0.001 {
            let c = s
                .control
                .filter(|c| c.mode == Mode::Part)
                .or(s.control)
                .map_or((Vec2::ZERO, Vec2::Y), |c| (c.center, c.dir));
            controls.part = Some(Part {
                center: c.0.to_array(),
                dir: c.1.to_array(),
                half_length: spells::CONTROL_SIDE * 0.5,
                half_width: 2.4,
                amount: s.part,
            });
        }
        if let Some(c) = s.control.filter(|c| c.mode == Mode::Redirect) {
            let ramp = ((self.time - c.started) / 2.0).clamp(0.0, 1.0);
            controls.flow = Some(Flow {
                center: c.center.to_array(),
                velocity: (c.dir * 2.6 * ramp).to_array(),
                radius: spells::CONTROL_SIDE * 0.5,
            });
        }
        if s.whirl > 0.001 {
            let at = s
                .control
                .filter(|c| c.mode == Mode::Whirlpool)
                .map(|c| c.center)
                .or(s.drain.map(|d| d.center))
                .unwrap_or(Vec2::ZERO);
            controls.whirl = Some(Whirl {
                center: at.to_array(),
                radius: spells::WHIRL_RADIUS,
                strength: s.whirl,
            });
        }
        if let Some((at, amount)) = s.wet {
            controls.wet = Some(Disc {
                center: at.to_array(),
                radius: if s.wet_radius > 0.0 {
                    s.wet_radius
                } else {
                    spells::RAIN_SIDE * 0.5
                },
                amount,
            });
        }
        if let Some((at, amount)) = s.ice {
            controls.ice = Some(Disc {
                center: at.to_array(),
                radius: spells::SLEET_RADIUS,
                amount,
            });
        }
        water.controls = controls;
        water
    }

    /// The water's surface and velocity over `p`, if water is there: the
    /// river or the pool, else the sea (none in Part Water's trench).
    #[must_use]
    pub fn surface_at(&self, p: Vec2) -> Option<floats::Surface> {
        self.surface_with(&self.frame_water(), p)
    }

    fn surface_with(&self, water: &Water, p: Vec2) -> Option<floats::Surface> {
        if let Some((height, flow)) = terrain::fresh_water(p) {
            return Some(floats::Surface {
                height,
                velocity: Vec3::new(flow.x, 0.0, flow.y),
            });
        }
        let bed = ground(p.x, p.y);
        let depth = LEVEL - bed;
        let height = water.surface_height(p.x, p.y, depth);
        if height <= bed + 0.02 || water.controls.trench(p, 0.6) > 0.5 {
            return None;
        }
        let ice = water.controls.ice_at(p);
        let velocity = (water.velocity(p, depth + water.level - water.rest)
            + water.controls.flow_at(p))
            * (1.0 - ice);
        Some(floats::Surface { height, velocity })
    }

    fn tick_world(&mut self, dt: f32) {
        self.time += dt;
        self.spells.tick(dt, self.time);
        self.water.time = self.time;
        // The orbs move first and take in what they touch, so the bodies
        // they hold follow them this step.
        self.tick_orbs(dt);
        let water = self.frame_water();
        let ice = water.controls;
        // The medium's current carries the bodies, the whirlpool's pull
        // included; ice holds what it froze around.
        let push = |_: Vec3| Vec3::ZERO;
        let mut floats = std::mem::take(&mut self.floats);
        let medium = Medium {
            lab: &*self,
            water: &water,
        };
        let events = floats.tick(dt, &medium, &push);
        for f in &floats.floats {
            let body = &mut floats.world.bodies_mut()[f.id.0 as usize];
            let at = body.pos.as_vec3();
            if ice.ice_at(Vec2::new(at.x, at.z)) > 0.6 {
                body.vel *= 0.8;
                body.omega *= 0.8;
            }
        }
        self.floats = floats;
        self.tick_jolts();
        self.tick_targets(dt);
        self.tick_bolts(dt);
        let now = self.time;
        self.floaters
            .retain(|f| now - f.start < verse_zone_everglade::zones::everglade::floaters::FLOAT);
        for event in events {
            match event {
                floats::Event::Splash { at, speed, size } => {
                    let scale = (speed / 5.0 * size / 0.4).clamp(0.4, 1.8);
                    self.fx.start("water_splash", Spawn::at(at).scaled(scale));
                    self.water
                        .add_ripple(Vec2::new(at.x, at.z), (0.02 * speed * size).min(0.08));
                }
                floats::Event::Ripple { at, strength } => {
                    self.water.add_ripple(at, strength);
                }
            }
        }
        // Rain and sleet stop with their spells.
        if self.spells.rain.is_none() {
            for handle in self.rain_fx.drain(..) {
                self.fx.stop(handle);
            }
        }
        if self.spells.sleet.is_none() {
            for handle in self.sleet_fx.drain(..) {
                self.fx.stop(handle);
            }
        }
        self.fx.tick(dt, |x, z| ground(x, z).max(LEVEL - 0.05));
    }

    fn random(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / 16_777_216.0
    }

    /// Advances the lab by `dt` seconds with the character at `feet`
    /// facing `forward`, and says what the character stands on.
    pub fn tick(&mut self, dt: f32, feet: Vec3, forward: Vec3) -> Feet {
        if !dt.is_finite() || dt <= 0.0 {
            return Feet {
                pace: 1.0,
                ..Feet::default()
            };
        }
        self.caster = (feet, forward);
        self.tick_world(dt);
        let moved = self.last.map_or(0.0, |last| {
            Vec2::new(feet.x - last.x, feet.z - last.z).length()
        });
        self.last = Some(feet);
        self.speed = self.speed + (moved / dt - self.speed) * (dt * 8.0).min(1.0);
        // Rain dimples the water it falls on.
        if let Some(rain) = self.spells.rain {
            self.rain_wait -= dt;
            while self.rain_wait <= 0.0 {
                self.rain_wait += 0.06;
                let half = spells::RAIN_SIDE * 0.5;
                let at = rain.center
                    + Vec2::new(self.random() * 2.0 - 1.0, self.random() * 2.0 - 1.0) * half;
                if self.surface_at(at).is_some() {
                    self.water.add_ripple(at, 0.006);
                }
            }
        }
        // The body pushes floating things aside.
        self.floats.shove(feet + Vec3::Y * 0.5, 1.2, 3.0);
        let p = Vec2::new(feet.x, feet.z);
        let water = self.frame_water();
        let mut out = Feet {
            pace: 1.0,
            ..Feet::default()
        };
        let surface = self.surface_with(&water, p);
        let ice = water.controls.ice_at(p);
        if ice > 0.5 && feet.y > water.level - 0.5 && self.surface_with(&water, p).is_some() {
            // Ice is ground, slick and hard going.
            out.floor = Some(water.level + 0.04);
            out.pace = 0.55;
        } else if let Some(s) = surface {
            let depth = s.height - feet.y;
            if self.spells.water_walk && depth < 0.45 {
                out.floor = Some(s.height + 0.02);
                if self.speed > 0.5 {
                    self.wake(p, s.height, forward, 0.05);
                }
            } else if depth > 0.05 {
                let swim = s.height - 1.25;
                let bed = ground(feet.x, feet.z);
                if !self.spells.breathing && bed < swim - 0.05 {
                    // Deep water: the character swims with its head up.
                    out.floor = Some(swim);
                    out.pace = 0.6;
                    if self.speed > 0.4 {
                        self.wake(p, s.height, forward, 0.035);
                    }
                } else {
                    // Wading slows the character by depth.
                    out.pace = (1.0 - depth.min(1.2) * 0.35).max(0.55);
                    if self.speed > 0.4 && depth < 1.4 {
                        self.stride -= dt;
                        if self.stride <= 0.0 {
                            self.stride = 0.34;
                            self.water.add_ripple(p, 0.025);
                            self.fx.start(
                                "water_wade",
                                Spawn::at(Vec3::new(feet.x, s.height, feet.z))
                                    .moving(forward * self.speed),
                            );
                        }
                    } else {
                        self.stride -= dt;
                        if self.stride <= -1.2 {
                            self.stride = 0.0;
                            self.water.add_ripple(p, 0.01);
                        }
                    }
                }
            }
        }
        // A held breath under the surface; Water Breathing needs none.
        let head = feet.y + 1.6;
        let under = surface.is_some_and(|s| head < s.height);
        if under && !self.spells.breathing {
            self.breath = (self.breath - dt).max(0.0);
            if self.breath <= 0.0 {
                out.floor = surface.map(|s| s.height - 1.25);
            }
        } else {
            self.breath = (self.breath + dt * 6.0).min(spells::breath_limit(spells::CON_MODIFIER));
        }
        if under {
            self.bubbles -= dt;
            if self.bubbles <= 0.0 {
                self.bubbles = 2.2 + self.random();
                self.fx
                    .start("water_bubbles", Spawn::at(Vec3::new(feet.x, head, feet.z)));
            }
        }
        out
    }

    /// Ripples and foam behind a body moving across the surface.
    fn wake(&mut self, p: Vec2, height: f32, forward: Vec3, strength: f32) {
        self.stride -= 1.0 / 60.0;
        if self.stride <= 0.0 {
            self.stride = 0.22;
            let behind = p - Vec2::new(forward.x, forward.z) * 0.4;
            self.water.add_ripple(behind, strength);
            if self.random() < 0.5 {
                self.fx.start(
                    "water_wade",
                    Spawn::at(Vec3::new(p.x, height, p.y)).scaled(0.7),
                );
            }
        }
    }

    /// Drops the next kind of float `reach` m ahead of `at`, from above.
    pub fn drop_float(&mut self, at: Vec3, forward: Vec3, yaw: f32) -> String {
        let kind = FloatKind::ALL[self.next_kind % 3];
        self.next_kind += 1;
        let spot = at + forward * 4.0;
        let high = ground(spot.x, spot.z).max(LEVEL) + 3.0;
        self.floats
            .spawn(kind, Vec3::new(spot.x, high, spot.z), yaw, forward * 2.0);
        let name = match kind {
            FloatKind::Crate => "A crate",
            FloatKind::Barrel => "A barrel",
            FloatKind::Plank => "A plank",
        };
        self.say(format!("{name} drops into the water"))
    }

    fn say(&mut self, line: String) -> String {
        self.log.push(line.clone());
        if self.log.len() > 4 {
            self.log.remove(0);
        }
        line
    }

    /// Presses a hotbar slot with the character at `at` facing `forward`.
    /// `alternate` (Shift) ends Control Water or casts Destroy Water.
    pub fn press(
        &mut self,
        slot: Slot,
        alternate: bool,
        at: Vec3,
        forward: Vec3,
        yaw: f32,
    ) -> String {
        let now = self.time;
        let flat = Vec2::new(forward.x, forward.z);
        let line = match slot {
            Slot::WaterWalk => {
                self.spells.water_walk = !self.spells.water_walk;
                if self.spells.water_walk {
                    "Water Walk: the surface holds you for an hour".into()
                } else {
                    "Water Walk ends".into()
                }
            }
            Slot::ControlWater => {
                if alternate {
                    self.spells.dismiss_control()
                } else {
                    let reach = if self
                        .spells
                        .control
                        .map_or(self.spells.mode, |c| c.mode.next())
                        == Mode::Part
                    {
                        spells::CONTROL_SIDE * 0.5 - 2.0
                    } else {
                        14.0
                    };
                    let center = Spells::aim(at, forward, reach);
                    self.spells.control_water(center, flat, now)
                }
            }
            Slot::CreateWater => {
                let center = Spells::aim(at, forward, 7.0);
                if alternate {
                    self.fx.start(
                        "water_mist",
                        Spawn::at(Vec3::new(center.x, LEVEL + 0.5, center.y)),
                    );
                    self.spells.destroy(center, now)
                } else {
                    let top = ground(center.x, center.y).max(LEVEL) + spells::RAIN_SIDE;
                    for handle in self.rain_fx.drain(..) {
                        self.fx.stop(handle);
                    }
                    self.rain_fx.extend(
                        self.fx
                            .start("water_rain", Spawn::at(Vec3::new(center.x, top, center.y))),
                    );
                    self.spells.create(center, now)
                }
            }
            Slot::SleetStorm => {
                let center = Spells::aim(at, forward, 14.0);
                let line = self.spells.sleet_storm(center, now);
                if self.spells.sleet.is_some() {
                    let top = LEVEL + spells::SLEET_HEIGHT;
                    self.sleet_fx.extend(
                        self.fx
                            .start("water_sleet", Spawn::at(Vec3::new(center.x, top, center.y))),
                    );
                    self.sleet_fx.extend(self.fx.start(
                        "water_steam",
                        Spawn::at(Vec3::new(center.x, LEVEL + 0.2, center.y)),
                    ));
                }
                line
            }
            Slot::WaterBreathing => {
                self.spells.breathing = !self.spells.breathing;
                if self.spells.breathing {
                    "Water Breathing: you can breathe under water for 24 hours".into()
                } else {
                    "Water Breathing ends".into()
                }
            }
            Slot::WaterOrb => self.begin_orb(at, forward),
            Slot::Thunderbolt => return self.thunderbolt(at, forward),
            Slot::Drop => return self.drop_float(at, forward, yaw),
        };
        self.say(line)
    }

    /// Lets go of a hotbar slot with the character at `at` facing
    /// `forward`: the Water Orb's key throws the orb it formed, or with
    /// `alternate` (Shift) sets it hovering. Other slots do nothing.
    pub fn release(
        &mut self,
        slot: Slot,
        alternate: bool,
        at: Vec3,
        forward: Vec3,
    ) -> Option<String> {
        match slot {
            Slot::WaterOrb => self.release_orb(alternate, at, forward),
            _ => None,
        }
    }

    /// Aims with the pointer's ray from `origin` along `direction`, or, with
    /// `None`, ahead of the caster.
    pub fn set_aim(&mut self, ray: Option<(Vec3, Vec3)>) {
        self.aim = ray.filter(|(o, d)| o.is_finite() && d.is_finite() && d.length_squared() > 1e-6);
    }

    /// Adds a floating number, dropping the oldest past [`MAX_FLOATERS`].
    fn float(&mut self, floater: Floater) {
        if self.floaters.len() >= MAX_FLOATERS {
            self.floaters.remove(0);
        }
        self.floaters.push(floater);
    }

    /// Jolts the floating bodies lightning struck: they shudder and spin.
    fn tick_jolts(&mut self) {
        let now = self.time;
        self.jolts.retain(|(_, until)| now < *until);
        let jolts = self.jolts.clone();
        for (id, until) in jolts {
            let k = ((until - now) / bolt::JOLT).clamp(0.0, 1.0);
            let kick = Vec3::new(
                self.random() - 0.5,
                self.random() - 0.5,
                self.random() - 0.5,
            ) * 1.8
                * k;
            let spin = Vec3::new(
                self.random() - 0.5,
                self.random() - 0.5,
                self.random() - 0.5,
            ) * 9.0
                * k;
            if let Some(body) = self.floats.world.bodies_mut().get_mut(id.0 as usize) {
                body.vel += kick.as_dvec3();
                body.omega += spin.as_dvec3();
            }
        }
    }

    /// Moves the dummies: those an orb holds with it, the rest falling and
    /// sliding, and each untouched one back up in time.
    fn tick_targets(&mut self, dt: f32) {
        let now = self.time;
        for (k, target) in self.targets.iter_mut().enumerate() {
            let hold = self
                .orbs
                .iter()
                .find(|o| o.dummies.contains(&k))
                .map(|o| (o.center, o.velocity));
            target.tick(dt, now, hold);
        }
    }

    /// Sets the dummies' figure: the pack's dummy beside the character's
    /// figure `cast`.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack has no dummy.
    pub fn set_model(&mut self, pack: &ZonePack, cast: Option<&Figure>) -> Result<(), String> {
        self.model = Some(Model::new(pack, cast, &[], self.targets.len())?);
        Ok(())
    }

    /// The character's figure `cast` with the dummies posed after it, and
    /// where the dummies' vertices begin, for the probes' light; `None`
    /// before [`Self::set_model`].
    #[must_use]
    pub fn figure(&self, cast: Option<&Figure>) -> Option<(Figure, usize)> {
        let model = self.model.as_ref()?;
        let dummies: Vec<_> = self.targets.iter().map(|t| t.dummy.clone()).collect();
        Some((
            model.figure(cast, None, &dummies, self.time),
            model.cast_count(),
        ))
    }

    /// Whether each slot's spell is running, for the hotbar.
    #[must_use]
    pub fn active(&self, slot: Slot) -> bool {
        match slot {
            Slot::WaterWalk => self.spells.water_walk,
            Slot::ControlWater => self.spells.control.is_some(),
            Slot::CreateWater => self.spells.rain.is_some() || self.spells.drain.is_some(),
            Slot::SleetStorm => self.spells.sleet.is_some(),
            Slot::WaterBreathing => self.spells.breathing,
            Slot::WaterOrb => self.forming().is_some(),
            Slot::Thunderbolt => !self.bolts.is_empty(),
            Slot::Drop => false,
        }
    }

    /// The stage this frame: the hour's light and the water.
    #[must_use]
    pub fn stage(&self) -> verse_pbr::pbr::Neon {
        let mut stage = sea::stage(self.time, self.hour, self.frame_water());
        // Lightning lights the cove and the sky for a moment.
        for (slot, lamp) in stage.lamps.iter_mut().zip(self.lamps()) {
            *slot = lamp;
        }
        stage.sky_flash = stage.sky_flash.max(self.flash * 0.5);
        stage
    }

    /// This frame's lit stage, floating bodies, orbs, lightning, dummies'
    /// bars and numbers, and particles, seen from `eye`.
    #[must_use]
    pub fn mesh(&self, eye: Vec3) -> Mesh {
        let mut mesh = Mesh {
            neon: Some(self.stage()),
            lit: self.floats.draw(),
            ..Mesh::default()
        };
        self.orb_liquid(&mut mesh.liquid);
        self.orb_marks(&mut mesh, eye);
        self.bolt_marks(&mut mesh, eye);
        let mut painter = Painter::new(eye);
        let now = self.time;
        for target in &self.targets {
            painter.bar(&target.dummy, &target.dummy.conditions(now), false);
        }
        for floater in &self.floaters {
            painter.floater(floater, now);
        }
        mesh.extend(&painter.mesh);
        self.fx.draw(&mut mesh.sprites);
        mesh
    }

    /// The HUD's caption.
    #[must_use]
    pub fn caption(&self, at: Vec3) -> String {
        let mut lines = vec!["Water Lab".to_owned()];
        let mut live = Vec::new();
        if self.spells.water_walk {
            live.push("Water Walk".to_owned());
        }
        if let Some(c) = self.spells.control {
            live.push(c.mode.name().to_owned());
        }
        if self.spells.rain.is_some() {
            live.push("Rain".into());
        }
        if self.spells.sleet.is_some() {
            live.push("Sleet Storm".into());
        }
        if self.spells.breathing {
            live.push("Water Breathing".into());
        }
        let hovering = self.orbs.iter().filter(|o| o.hovering()).count();
        if hovering > 0 {
            let plural = if hovering == 1 { "" } else { "s" };
            live.push(format!("{hovering} hovering orb{plural}"));
        }
        if !live.is_empty() {
            lines.push(live.join(", "));
        }
        if let Some(orb) = self.forming() {
            let full = if orb.radius >= orb::MAX_RADIUS - 1e-3 {
                " (largest)"
            } else {
                ""
            };
            lines.push(format!(
                "Water Orb {:.1} m across{full}, {}; let go to throw, Shift to hold",
                orb.radius * 2.0,
                orb.source.name()
            ));
        }
        let limit = spells::breath_limit(spells::CON_MODIFIER);
        if self.breath < limit - 0.5 {
            lines.push(format!("Breath {:.0} s", self.breath));
        }
        if near_exit(at) {
            lines.push("F at the lantern leaves for the plaza".into());
        } else if let Some(line) = self.log.last() {
            lines.push(line.clone());
        } else {
            lines.push(
                "1 to 5 cast, hold 6 for a Water Orb, 7 Thunderbolt, B drops a float, T turns the hour, G leaves"
                    .into(),
            );
        }
        lines.join("\n")
    }

    /// Turns the hour.
    pub fn turn_hour(&mut self) -> String {
        self.hour = self.hour.next();
        let line = match self.hour {
            Hour::Golden => "Golden hour",
            Hour::Noon => "Noon",
        };
        self.say(line.into())
    }
}
