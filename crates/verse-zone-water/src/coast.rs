//! The coastal test scene (`docs/verse/water.md`, phase W10): a bay laid
//! out as the coast specification's (`docs/verse/coast.md`), for the
//! clipmap ocean's captures, its streaming walk, and its multiplayer
//! checks, and for the coast zone (C1) to start from.
//!
//! The frame is the coast's: mean sea level at y = 0, +x east, +z south,
//! land to the northeast and open sea to the south and west. Driftwood
//! Beach is a crescent from (−80, −180) to (240, 180) facing southwest whose
//! bed shelves at 1 in 30 to 5 m deep 150 m out, then to 20 m by 400 m and
//! 40 m at the playable edge. Dunes rise behind it to the arrival terrace,
//! 14 m up at (120, −120). In the northwest the headland stands 40 m high
//! with cliffs to the sea, and a breakwater runs east-southeast from it,
//! sheltering the harbor basin, 4 m deep, at (−130, −230). Gull Island rises
//! 18 m from the bay at (−100, 230).
//!
//! Everything here is a pure function of the zone's constants, so every
//! client builds the same ground ([`ground`]), the same gameplay water
//! ([`water_set`], an ocean of sea water with a seeded spectrum), and the
//! same baked field ([`field`]) without a download, and samples the same
//! surface at the same world tick ([`physics::water::tick_at`]).
//! Buoyant bodies ([`Buoys`]) are ordinary rigid bodies whose poses
//! replicate as any shared body's do; nothing about the water replicates.

use std::sync::Arc;

use glam::{DQuat, DVec2, DVec3, Vec2};
use physics::water::{SALT, WaterBody, WaterId, WaterSet, WaveSet};
use physics::{Body, BodyId, Collider, Filter, Material, Shape, Uniform, World};
use verse_pbr::water::field::{Field, Texel};
use verse_pbr::water::{Body as FrameBody, Ocean, Preset, SeaState, Water, WaterSurface};

use crate::floats::Kind;
use crate::terrain::{fbm, smoothstep};

/// The seed of the coast's sea.
pub const SEED: u64 = 0xC0A5_7;
/// The direction the waves travel, rad about +Y (0 is +z): from the
/// southwest toward the northeast, onto the beach.
pub const WIND: f64 = 2.45;
/// The depth the spectrum disperses over, m: the bay's.
pub const SEA_DEPTH: f64 = 20.0;
/// The playable square's half extent, m.
pub const HALF_EXTENT: f32 = 600.0;
/// The field's texel, m (`docs/verse/coast.md`, baked fields).
pub const TEXEL: f32 = 2.0;
/// The field's pages a side: 128 m pages over the playable square.
pub const PAGES: u32 = 10;
/// The beach's crescent: the center and radius of the circle its waterline
/// follows, m; the land lies inside.
pub const BEACH_CENTER: [f32; 2] = [400.0, -280.0];
pub const BEACH_RADIUS: f32 = 489.0;
/// The arrival terrace.
pub const TERRACE: [f32; 3] = [120.0, 14.0, -120.0];
/// The harbor basin's center and radius, m.
pub const HARBOR: [f32; 2] = [-130.0, -230.0];
pub const HARBOR_RADIUS: f32 = 75.0;
/// The breakwater, from the headland east-southeast, m.
pub const BREAKWATER: [[f32; 2]; 2] = [[-215.0, -175.0], [-70.0, -140.0]];
/// Gull Island's center and radius, m.
pub const GULL_ISLAND: [f32; 2] = [-100.0, 230.0];
pub const GULL_RADIUS: f32 = 60.0;
/// The world tick the captures and tests show: 2026-10-07 12:00 UTC.
pub const TICK: u64 = 1_791_374_400 * physics::water::TICK_HZ;

/// How far seaward of the beach's waterline (x, z) lies, m; negative
/// inland.
#[must_use]
pub fn seaward(x: f32, z: f32) -> f32 {
    Vec2::new(x, z).distance(Vec2::from(BEACH_CENTER)) - BEACH_RADIUS
}

/// The bed's depth below mean sea level `s` m seaward of the beach, m: 1 in
/// 30 to 5 m at 150 m, 20 m at 400 m, then down to 40 m.
#[must_use]
pub fn bathymetry(s: f32) -> f32 {
    if s <= 150.0 {
        s / 30.0
    } else if s <= 400.0 {
        5.0 + (s - 150.0) * 15.0 / 250.0
    } else {
        (20.0 + (s - 400.0) * 0.1).min(40.0)
    }
}

fn segment_distance(p: Vec2, a: [f32; 2], b: [f32; 2]) -> f32 {
    let (a, b) = (Vec2::from(a), Vec2::from(b));
    let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
    p.distance(a + (b - a) * t)
}

/// The coast's ground height at (x, z), m: the land and the sea bed as
/// one heightfield.
#[must_use]
pub fn ground(x: f32, z: f32) -> f32 {
    if !x.is_finite() || !z.is_finite() {
        return 0.0;
    }
    let p = Vec2::new(x, z);
    let s = seaward(x, z);
    let mut h = if s > 0.0 {
        -bathymetry(s) + 0.4 * (fbm(x * 0.02, z * 0.02, 2) - 0.5) * smoothstep(20.0, 120.0, s)
    } else {
        // Sand to the dunes' foot at 2.5 m, then dunes to the terrace's
        // height, rolling inland.
        let d = -s;
        let sand = 0.05 * d.min(50.0);
        let dunes =
            smoothstep(40.0, 150.0, d) * (11.5 + 3.0 * (fbm(x * 0.015, z * 0.015, 3) - 0.5));
        sand + dunes + smoothstep(200.0, 500.0, d) * 10.0
    };
    // The headland: a plateau 35 to 45 m high in the northwest, with
    // cliffs to the sea.
    let headland = smoothstep(-170.0, -215.0, x) * smoothstep(-70.0, -115.0, z);
    if headland > 0.0 {
        let top = 40.0 + 5.0 * (fbm(x * 0.01, z * 0.01 + 4.0, 3) - 0.5) * 2.0;
        let k = smoothstep(0.0, 1.0, headland);
        h += (top - h) * k * k * (3.0 - 2.0 * k);
    }
    // The harbor basin, 4 m deep behind the breakwater.
    let harbor = p.distance(Vec2::from(HARBOR));
    if harbor < HARBOR_RADIUS + 25.0 {
        let k = smoothstep(HARBOR_RADIUS + 25.0, HARBOR_RADIUS, harbor);
        h = h + (h.min(-4.0) - h) * k;
    }
    // The breakwater: a mound of stone 2.5 m above the sea, 8 m wide.
    let wall = segment_distance(p, BREAKWATER[0], BREAKWATER[1]);
    if wall < 9.0 {
        h = h.max(2.5 - 0.6 * (wall - 4.0).max(0.0) * (wall - 4.0).max(0.0));
    }
    // Gull Island: rocky shores, a grassy top 18 m up.
    let island = p.distance(Vec2::from(GULL_ISLAND));
    if island < GULL_RADIUS + 40.0 {
        let k = smoothstep(GULL_RADIUS + 40.0, GULL_RADIUS * 0.3, island);
        let top = 18.0 + 2.0 * (fbm(x * 0.04, z * 0.04, 2) - 0.5);
        h = h.max(h + (top - h) * k);
    }
    h
}

/// The coast's ocean in sea state `state` (`assets/verse/water/seas/`):
/// sea water everywhere at mean sea level, its waves the seeded spectrum's
/// gameplay band.
///
/// # Errors
/// Names an unknown sea state.
pub fn ocean(state: &str) -> Result<WaterBody, String> {
    let sea = SeaState::named(state).ok_or_else(|| format!("No sea state {state}"))?;
    let waves = WaveSet::calm().with_spectrum(sea.spectrum(WIND, SEED, SEA_DEPTH))?;
    Ok(WaterBody::ocean(WaterId(0), 0.0)
        .with_density(SALT)
        .with_waves(waves))
}

/// The coast's gameplay water: what the host and every client sample.
///
/// # Errors
/// Names an unknown sea state.
pub fn water_set(state: &str) -> Result<WaterSet, String> {
    Ok(WaterSet::new(vec![ocean(state)?], 8.0))
}

/// The baked field: depth below the sea at rest from [`ground`], the shore
/// distance from the dry texels, and no current yet (the estuary's and
/// the bounds current are C1's), over the playable square.
///
/// # Errors
/// Passes on a refused bake.
pub fn field() -> Result<Field, String> {
    let origin = -(PAGES as f32) * TEXEL * verse_pbr::water::field::PAGE as f32 * 0.5;
    Field::bake(
        [origin, origin],
        TEXEL,
        [PAGES, PAGES],
        Texel::open(40.0),
        |x, z| (-ground(x, z), [0.0, 0.0]),
    )
}

/// The coast's water at rest: the ocean on the clipmap, over `field`.
#[must_use]
pub fn surface(field: Arc<Field>) -> WaterSurface {
    WaterSurface {
        patches: Vec::new(),
        ocean: Some(Ocean {
            body: 0,
            sea: true,
            field: Some(field),
        }),
    }
}

/// The frame's water at world tick `tick` in sea state `state`: the
/// ocean's spectrum with the `ocean` preset's optics, its clock folded on
/// the spectrum's loop so it stays precise at any tick.
///
/// # Errors
/// Names an unknown sea state.
pub fn frame_water(state: &str, tick: u64) -> Result<Water, String> {
    let body = ocean(state)?;
    let spectrum = body
        .waves
        .spectrum
        .ok_or("The coast's ocean has a spectrum")?;
    let preset = Preset::named("ocean").cloned().unwrap_or_default();
    let mut water = Water::ocean(0.0, spectrum);
    *water.sea_body_mut() = FrameBody {
        spectrum: Some(spectrum),
        ..FrameBody::still(0.0, &preset)
    };
    water.time = ((tick % spectrum.period) as f64 * spectrum.tick) as f32;
    Ok(water)
}

/// Bodies afloat off the beach, simulated on the gameplay surface: the
/// host's (or the owner's) side of a shared body.
pub struct Buoys {
    pub world: World,
    pub bodies: Vec<(Kind, BodyId)>,
    /// The world tick of the next step.
    pub tick: u64,
}

/// Where the scene's buoyant bodies start: crates, barrels, and planks
/// past the surf and in the harbor.
pub const BUOYS: [(Kind, [f32; 2]); 6] = [
    (Kind::Barrel, [-20.0, 60.0]),
    (Kind::Crate, [-8.0, 72.0]),
    (Kind::Plank, [6.0, 64.0]),
    (Kind::Barrel, [-120.0, -220.0]),
    (Kind::Crate, [-140.0, -235.0]),
    (Kind::Plank, [30.0, 90.0]),
];

impl Buoys {
    /// The scene's bodies, each dropped at its spot on the water at
    /// `tick`.
    #[must_use]
    pub fn new(tick: u64) -> Self {
        let mut world = World::new(1.0 / physics::water::TICK_HZ as f64);
        world.sleep.enabled = false;
        let mut bodies = Vec::new();
        for (i, (kind, [x, z])) in BUOYS.into_iter().enumerate() {
            let h = kind.half();
            let mut body = Body::new(
                kind.mass(),
                Body::box_inertia(kind.mass(), h * 2.0),
                DVec3::new(f64::from(x), 0.2, f64::from(z)),
            );
            body.orientation = DQuat::from_rotation_y(i as f64 * 1.3);
            body.prev_orientation = body.orientation;
            let id = world.add(body);
            world.add_collider(
                Collider::new(id, Shape::Cuboid { half: h })
                    .with_filter(Filter::ALL)
                    .with_material(Material {
                        friction: 0.4,
                        torsional: 0.0,
                        restitution: 0.1,
                    }),
            );
            bodies.push((kind, id));
        }
        Self {
            world,
            bodies,
            tick,
        }
    }

    /// Steps the bodies `ticks` times on `water`.
    pub fn step(&mut self, water: &WaterSet, ticks: u64) {
        let dt = 1.0 / physics::water::TICK_HZ as f64;
        for _ in 0..ticks {
            physics::water::apply(&mut self.world, water, self.tick, dt);
            self.world.step(&Uniform(DVec3::new(0.0, -9.81, 0.0)));
            self.tick += 1;
        }
    }

    /// Each body's kind, position, and orientation.
    #[must_use]
    pub fn poses(&self) -> Vec<(Kind, DVec3, DQuat)> {
        self.bodies
            .iter()
            .map(|&(kind, id)| {
                let b = &self.world.bodies()[id.0 as usize];
                (kind, b.pos, b.orientation)
            })
            .collect()
    }
}

/// The ground under (x, z) as `f64`, for the physics.
#[must_use]
pub fn ground_at(p: DVec2) -> f64 {
    f64::from(ground(p.x as f32, p.y as f32))
}
