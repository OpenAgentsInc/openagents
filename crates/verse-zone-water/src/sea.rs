//! The cove's water at rest (the sea, the river, the falls, and the pool)
//! and the light it is seen in.

use glam::{Vec2, Vec3};
use verse_pbr::pbr::water::{Body, Kind, Water, WaterPatch, WaterSurface, WaterVertex};
use verse_pbr::pbr::{Daylight, Grade, HeightFog, Key, Neon};
use verse_pbr::water::{Preset, SeaState};

use crate::terrain::{
    self, CENTER, FALL_HALF_WIDTH, FALL_HEADING, FALL_SPEED, LEVEL, LIP, LOWER, POOL, POOL_LEVEL,
    POOL_RADIUS, REACH, Reach, UPPER, ground,
};

/// The sea's body in the frame's water: salt water, the cove's look.
pub const SEA: usize = 0;
/// The river's, the pool's, and the falls' body: fresh water, a stream's
/// look.
pub const FRESH: usize = 1;

/// The cove's water surfaces.
#[must_use]
pub fn surface() -> WaterSurface {
    WaterSurface {
        patches: vec![
            sea(),
            reach(&UPPER, true),
            pool(),
            reach(&LOWER, false),
            fall(),
        ],
    }
}

/// The sea: a warped grid dense over the bay, out to the horizon.
fn sea() -> WaterPatch {
    let n = 241u32;
    let mut vertices = Vec::with_capacity((n * n) as usize);
    for j in 0..n {
        for i in 0..n {
            let u = i as f32 / (n - 1) as f32 * 2.0 - 1.0;
            let v = j as f32 / (n - 1) as f32 * 2.0 - 1.0;
            let x = CENTER[0] + terrain::warp(u, REACH);
            let z = CENTER[1] + terrain::warp(v, REACH);
            let depth = LEVEL - ground(x, z);
            vertices.push(WaterVertex::new(
                Vec3::new(x, LEVEL, z),
                depth,
                Kind::Sea(1.0),
            ));
        }
    }
    let mut patch = WaterPatch {
        cols: n,
        rows: n,
        vertices,
        decimate: true,
        // A flood rises over the beach, so the sea keeps quads up to its
        // height and more.
        dry: 6.0,
    };
    patch.bake_shore();
    patch
}

/// Resamples a reach's centerline every `step` m: points, surface heights,
/// and unit directions.
fn samples(reach: &Reach, step: f32) -> Vec<(Vec2, f32, Vec2)> {
    let mut out = Vec::new();
    for pair in reach.points.windows(2) {
        let a = Vec2::new(pair[0][0], pair[0][1]);
        let b = Vec2::new(pair[1][0], pair[1][1]);
        let n = ((b - a).length() / step).ceil().max(1.0) as usize;
        for k in 0..n {
            let t = k as f32 / n as f32;
            out.push((
                a.lerp(b, t),
                pair[0][2] + (pair[1][2] - pair[0][2]) * t,
                (b - a).normalize(),
            ));
        }
    }
    let last = reach.points[reach.points.len() - 1];
    let prev = reach.points[reach.points.len() - 2];
    out.push((
        Vec2::new(last[0], last[1]),
        last[2],
        (Vec2::new(last[0] - prev[0], last[1] - prev[1])).normalize(),
    ));
    // Smooth the directions so the strip doesn't kink at the corners.
    let dirs: Vec<Vec2> = (0..out.len())
        .map(|i| {
            let a = out[i.saturating_sub(2)].2;
            let b = out[(i + 2).min(out.len() - 1)].2;
            (a + out[i].2 * 2.0 + b).normalize()
        })
        .collect();
    for (o, d) in out.iter_mut().zip(dirs) {
        o.2 = d;
    }
    out
}

/// A river's surface: a strip along its centerline, wider than the channel
/// so its edges tuck under the banks. White water gathers toward the lip
/// on the plateau.
fn reach(reach: &Reach, to_lip: bool) -> WaterPatch {
    let mut rows = samples(reach, 0.8);
    if to_lip {
        // Stop just short of the lip, where the falls take over.
        rows.truncate(rows.len().saturating_sub(1));
    }
    let cols = 9u32;
    let half = reach.half_width + 1.6;
    let mut vertices = Vec::new();
    let end = rows.len() as f32;
    for (k, (c, y, dir)) in rows.iter().enumerate() {
        let side = Vec2::new(-dir.y, dir.x);
        for i in 0..cols {
            let a = i as f32 / (cols - 1) as f32 * 2.0 - 1.0;
            let p = *c + side * a * half;
            let depth = y - ground(p.x, p.y);
            let mut v =
                WaterVertex::new(Vec3::new(p.x, *y, p.y), depth, Kind::Stream).in_body(FRESH);
            // Faster in the middle of the channel.
            let speed = reach.speed * (1.0 - 0.5 * a * a);
            v.flow = (*dir * speed).to_array();
            let lip = if to_lip {
                ((k as f32 - end + 8.0) / 8.0).clamp(0.0, 1.0)
            } else {
                0.0
            };
            // Riffles where the bed is shallow.
            let riffle = (1.0 - depth / 0.35).clamp(0.0, 1.0) * 0.6;
            v.foam = lip.max(riffle);
            vertices.push(v);
        }
    }
    let mut patch = WaterPatch {
        cols,
        rows: rows.len() as u32,
        vertices,
        decimate: false,
        dry: 0.0,
    };
    patch.bake_shore();
    patch
}

/// The plunge pool: a square grid over the pool, its foam thickest where
/// the falls land, its water drifting out toward the stream.
fn pool() -> WaterPatch {
    let n = 33u32;
    let half = POOL_RADIUS + 1.2;
    let land = landing();
    let out = Vec2::new(LOWER.points[0][0], LOWER.points[0][1]);
    let mut vertices = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let x = POOL[0] - half + 2.0 * half * i as f32 / (n - 1) as f32;
            let z = POOL[1] - half + 2.0 * half * j as f32 / (n - 1) as f32;
            let p = Vec2::new(x, z);
            let depth = POOL_LEVEL - ground(x, z);
            let mut v =
                WaterVertex::new(Vec3::new(x, POOL_LEVEL, z), depth, Kind::Stream).in_body(FRESH);
            let from = p - Vec2::new(land.x, land.z);
            let spread = from.normalize_or_zero() * 0.6 * (-from.length() / 3.0).exp();
            v.flow = (spread + (out - p).normalize_or_zero() * 0.25).to_array();
            v.foam = (1.0 - from.length() / 3.2).clamp(0.0, 1.0);
            vertices.push(v);
        }
    }
    let mut patch = WaterPatch {
        cols: n,
        rows: n,
        vertices,
        decimate: false,
        dry: 0.0,
    };
    patch.bake_shore();
    patch
}

/// Where the falls meet the pool.
#[must_use]
pub fn landing() -> Vec3 {
    let t = fall_time();
    let h = FALL_HEADING.normalize();
    let p = Vec2::new(LIP.x, LIP.z) + h * FALL_SPEED * t;
    Vec3::new(p.x, POOL_LEVEL, p.y)
}

/// How long the water takes to fall from the lip to the pool, s.
fn fall_time() -> f32 {
    (2.0 * (LIP.y - POOL_LEVEL) / verse_pbr::pbr::water::GRAVITY).sqrt()
}

/// The falls: a sheet along the water's ballistic path from the lip to the
/// pool, widening a little as it falls.
fn fall() -> WaterPatch {
    let cols = 9u32;
    let rows = 24u32;
    let heading = FALL_HEADING.normalize();
    let side = Vec2::new(-heading.y, heading.x);
    let total = fall_time();
    let mut vertices = Vec::new();
    for j in 0..rows {
        let s = j as f32 / (rows - 1) as f32;
        // Rows bunch toward the lip, where the sheet curves.
        let t = total * s * s.sqrt();
        let c = Vec2::new(LIP.x, LIP.z) + heading * FALL_SPEED * t;
        let y = LIP.y - 0.5 * verse_pbr::pbr::water::GRAVITY * t * t;
        let half = FALL_HALF_WIDTH * (1.0 + 0.25 * s);
        for i in 0..cols {
            let a = i as f32 / (cols - 1) as f32 * 2.0 - 1.0;
            let p = c + side * a * half;
            let mut v =
                WaterVertex::new(Vec3::new(p.x, y + 0.02, p.y), 1.0, Kind::Fall).in_body(FRESH);
            v.flow = heading.to_array();
            // Ragged, thinner edges; foamier as it falls.
            v.foam = (s * 0.8 + 0.2 - a.abs().powi(4) * 0.6).clamp(0.0, 1.0);
            vertices.push(v);
        }
    }
    WaterPatch {
        cols,
        rows,
        vertices,
        decimate: false,
        dry: 0.0,
    }
}

/// The time of day the lab shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Hour {
    /// A low sun over the sea: long shadows and a path of glitter.
    #[default]
    Golden,
    /// The sun high: clear turquoise shallows and bright caustics.
    Noon,
}

impl Hour {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Golden => Self::Noon,
            Self::Noon => Self::Golden,
        }
    }

    fn sun(self) -> Vec3 {
        match self {
            Self::Golden => Vec3::new(-0.38, 0.17, -0.91).normalize(),
            Self::Noon => Vec3::new(-0.3, 0.86, -0.42).normalize(),
        }
    }
}

/// The haze at the horizon and the fog's color.
pub const HAZE: [f32; 3] = [0.74, 0.62, 0.50];
pub const NOON_HAZE: [f32; 3] = [0.66, 0.76, 0.84];
pub const FOG_START: f32 = 60.0;
pub const FOG_END: f32 = 280.0;

/// Low sea haze that glows toward the sun.
pub const HEIGHT_FOG: HeightFog = HeightFog {
    density: 0.0016,
    base: 0.0,
    falloff: 0.08,
    start: 70.0,
    max_opacity: 0.85,
    sun_strength: 0.8,
    sun_exponent: 6.0,
};

/// The sea states the lab turns through (`assets/verse/water/seas/`).
pub const SEAS: [&str; 3] = ["calm", "moderate", "storm"];
/// The sea state the lab opens with, in [`SEAS`].
pub const DEFAULT_SEA: usize = 1;
/// The direction the waves travel, rad about +Y: toward the beach (+z), a
/// little east.
pub const WIND: f64 = 0.25;
/// The bay's deepest water, m: the depth the spectrum disperses over.
pub const SEA_DEPTH: f64 = 16.0;
/// The seed of the lab's sea.
pub const SEED: u64 = 0x5EA_C0FE;

/// The spectrum of sea state `name` rolling into the bay.
#[must_use]
pub fn spectrum(name: &str) -> Option<physics::water::Spectrum> {
    SeaState::named(name).map(|s| s.spectrum(WIND, SEED, SEA_DEPTH))
}

/// The sea's waves and color for the lab: a wind sea from the south-west
/// rolling into the bay ([`DEFAULT_SEA`] of [`SEAS`]), clear water over sand
/// (the `cove` preset), and the fresh water of the river, pool, and falls
/// (`river`). The waves are a `physics::water::Spectrum` whose gameplay
/// band the shader, the floats, and the physics share; the finer cascades
/// only draw.
#[must_use]
pub fn water() -> Water {
    let preset = |name| Preset::named(name).cloned().unwrap_or_default();
    let spectrum = spectrum(SEAS[DEFAULT_SEA]).expect("the lab's sea states are built in");
    let mut water = Water::ocean(LEVEL, spectrum);
    *water.sea_body_mut() = Body {
        spectrum: Some(spectrum),
        ..Body::still(LEVEL, &preset("cove"))
    };
    water.caustics = 0.9;
    let fresh = water.add(Body::still(POOL_LEVEL, &preset("river")));
    debug_assert_eq!(fresh, Some(FRESH));
    water
}

/// The lit stage at `hour`.
#[must_use]
pub fn stage(time: f32, hour: Hour, water: Water) -> Neon {
    let sun = hour.sun();
    let (haze, glow, zenith, sun_color, illuminance, sky) = match hour {
        Hour::Golden => (
            HAZE,
            0.55,
            [0.07, 0.16, 0.42],
            [1.0, 0.64, 0.34],
            2_800.0,
            520.0,
        ),
        Hour::Noon => (
            NOON_HAZE,
            0.0,
            [0.10, 0.32, 0.78],
            [1.0, 0.95, 0.86],
            5_200.0,
            1_500.0,
        ),
    };
    Neon {
        field: haze,
        fog_start: FOG_START,
        fog_end: FOG_END,
        line_gain: 1.0,
        line_width: 1.4,
        bloom: 0.08,
        vignette: 0.32,
        time,
        key: Some(Key {
            dir: sun,
            illuminance,
            angular_radius: 0.03,
            rim_dir: Vec3::new(0.5, 0.45, 0.7).normalize(),
            rim_illuminance: illuminance * 0.18,
            rim_angular_radius: 0.2,
            sky,
            ground: sky * 0.35,
            ev100: if hour == Hour::Golden { 10.0 } else { 11.0 },
            shadow_center: Vec3::ZERO,
            shadow_half: 90.0,
            shadow_distance: Some(140.0),
            // The floating bodies move; every cascade draws them.
            cache_far_shadows: false,
        }),
        daylight: Some(Daylight {
            zenith,
            horizon: haze,
            sun: sun_color,
            clouds: 0.3,
            ground: [0.3, 0.27, 0.2],
            glow,
        }),
        height_fog: Some(HEIGHT_FOG),
        grade: Grade {
            exposure: if hour == Hour::Golden { 0.1 } else { 0.0 },
            saturation: 1.08,
            contrast: 1.12,
            shadows: Vec3::new(0.92, 1.0, 1.1),
            highlights: Vec3::new(1.06, 1.0, 0.9),
            ..Grade::STAGE
        },
        key_color: sun_color,
        rim_color: [0.55, 0.65, 1.0],
        water: Some(water),
        ..Neon::plaza(time)
    }
}
