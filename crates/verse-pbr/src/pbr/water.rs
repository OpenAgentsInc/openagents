//! Water: seas, lakes, streams, and falls, drawn by the physical renderer's
//! water pass (`water.wgsl`) on a lit neon stage.
//!
//! A zone puts two things in its meshes. Its world mesh carries a
//! [`WaterSurface`], the static grids of the water's rest shape, which the
//! renderer uploads once. Each frame's stage ([`super::Neon::water`])
//! carries a [`Water`]: the level, the clock, the swell, the fine detail,
//! the ripples of the moment, and the water's optical properties.
//!
//! The techniques are public ones, reimplemented here:
//!
//! - The swell is a sum of Gerstner (trochoidal) waves, after Tessendorf,
//!   "Simulating Ocean Water" (SIGGRAPH course notes, 2001), and Finch,
//!   "Effective Water Simulation from Physical Models" (*GPU Gems*, ch. 1,
//!   2004), with the deep-water dispersion relation ω² = g·k. Toward the
//!   shore each wave's height falls with √tanh(k·d), the shallow-water
//!   factor of the full dispersion relation, so the swell flattens out over
//!   the beach instead of cutting through it.
//! - Fine detail is a second, shorter set of waves that only bends normals,
//!   faded out once a wave is shorter than a few pixels, with its slope
//!   variance moved into roughness (Toksvig 2005; Olano and Baker, "LEAN
//!   Mapping", 2010), so distant water keeps its sun glitter instead of
//!   aliasing.
//! - Streams advect their detail along a flow vector in two phases half a
//!   period apart, blended so neither phase's reset shows (Vlachos, "Water
//!   Flow in Portal 2", SIGGRAPH 2010).
//! - Reflection is Fresnel-weighted (Schlick 1994) sky from the stage's
//!   prefiltered sky, and the sun's glint is the renderer's GGX lobe.
//! - Light under the surface is attenuated by Beer–Lambert extinction along
//!   the refracted view path and the sun's path, plus single-scattered
//!   in-scatter; the seabed's own shader applies it, so a floating crate's
//!   submerged half fades into the blue the same way the sand does.
//! - Caustics on the seabed come from the area ratio of the refracted light
//!   field: where the surface's curvature focuses rays, the Jacobian of the
//!   map from surface to bed vanishes and the light gathers into bright
//!   lines (after Evan Wallace's WebGL Water, 2011, which measures the same
//!   ratio with screen-space derivatives; here it is the analytic Hessian of
//!   a few short waves).
//! - Ripples are analytic expanding rings, a damped wave packet whose front
//!   moves at a fixed speed.
//!
//! [`Water::surface_height`] evaluates the same swell on the CPU, so a
//! floating body bobs on exactly the surface the player sees.

use std::f32::consts::TAU;

use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};

/// Gravity's acceleration, m/s².
pub const GRAVITY: f32 = 9.81;
/// The most swell waves a stage carries.
pub const MAX_WAVES: usize = 8;
/// The most detail waves a stage carries.
pub const MAX_DETAIL: usize = 16;
/// The most ripples a frame carries.
pub const MAX_RIPPLES: usize = 24;
/// How long a ripple lasts, s.
pub const RIPPLE_LIFE: f32 = 4.0;
/// How fast a ripple's ring spreads, m/s.
pub const RIPPLE_SPEED: f32 = 1.1;
/// A quad whose four corners stand this far above the water (negative
/// depth) is dry ground, and no water is drawn there, m.
pub const DRY: f32 = 0.6;

/// One Gerstner wave.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wave {
    /// Unit direction of travel in the xz plane.
    pub dir: [f32; 2],
    /// Crest to crest, m.
    pub wavelength: f32,
    /// Crest height above the rest level, m.
    pub amplitude: f32,
    /// Gerstner steepness Q: 0 is a sine wave; higher sharpens the crests
    /// by moving water toward them.
    pub steepness: f32,
    /// Phase offset, radians.
    pub phase: f32,
}

impl Wave {
    /// The wavenumber, rad/m.
    #[must_use]
    pub fn k(&self) -> f32 {
        TAU / self.wavelength.max(1e-3)
    }

    /// Angular frequency from the deep-water dispersion relation, rad/s.
    #[must_use]
    pub fn omega(&self) -> f32 {
        (GRAVITY * self.k()).sqrt()
    }

    /// The share of this wave's height left over water `depth` deep:
    /// √tanh(k·d), one in deep water and zero on dry land.
    #[must_use]
    pub fn shoaling(&self, depth: f32) -> f32 {
        (self.k() * depth.max(0.0)).tanh().sqrt()
    }
}

/// An expanding ring on the surface, from a footstep, a bob, or a splash.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ripple {
    /// Where it started, x and z, m.
    pub at: [f32; 2],
    /// When it started, on the stage's water clock, s.
    pub start: f32,
    /// Its height at the start, m.
    pub strength: f32,
}

/// What a frame's water looks like and how it moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Water {
    /// The sea's level now, m. The seabed's shading takes the water's
    /// color from it; streams and pools at other heights carry their own.
    pub level: f32,
    /// The level the sea's vertices were built at, m; a flood raises
    /// [`Self::level`] above it.
    pub rest: f32,
    /// Scales the swell's heights: above one in a storm or a flood.
    pub swell_gain: f32,
    /// What spells are doing to the water ([`Controls`]).
    pub controls: Controls,
    /// The water clock, s.
    pub time: f32,
    pub waves: [Wave; MAX_WAVES],
    pub wave_count: usize,
    pub detail: [Wave; MAX_DETAIL],
    pub detail_count: usize,
    pub ripples: [Ripple; MAX_RIPPLES],
    /// Extinction (absorption and out-scattering) per meter, linear rgb.
    pub extinction: [f32; 3],
    /// The share of the light under the surface scattered back up toward
    /// the eye, linear rgb: the water body's own color.
    pub scatter: [f32; 3],
    /// How much foam forms on crests and along the shore, 0 to 1.
    pub foam: f32,
    /// How strongly the caustics focus sunlight on the bed, 0 to 1.
    pub caustics: f32,
    /// The calm surface's microfacet roughness.
    pub roughness: f32,
}

impl Default for Water {
    fn default() -> Self {
        Self::calm(0.0)
    }
}

impl Water {
    /// Still, clear water at `level` with only fine detail on it.
    #[must_use]
    pub fn calm(level: f32) -> Self {
        let mut water = Self {
            level,
            rest: level,
            swell_gain: 1.0,
            controls: Controls::default(),
            time: 0.0,
            waves: [Wave::default(); MAX_WAVES],
            wave_count: 0,
            detail: [Wave::default(); MAX_DETAIL],
            detail_count: 0,
            ripples: [Ripple::default(); MAX_RIPPLES],
            extinction: [0.42, 0.11, 0.085],
            scatter: [0.012, 0.055, 0.07],
            foam: 0.6,
            caustics: 0.8,
            roughness: 0.04,
        };
        water.set_detail(0.3, 0.12, 2.4, 0.016);
        water
    }

    /// A sea at `level` with a swell from `wind` (radians about +Y, the
    /// direction the waves travel toward, as the controller's yaw: 0 is +z)
    /// whose longest waves are `longest` m, `height` setting the crest
    /// heights relative to wavelength.
    #[must_use]
    pub fn sea(level: f32, wind: f32, longest: f32, height: f32) -> Self {
        let mut water = Self::calm(level);
        water.set_swell(wind, longest, height, MAX_WAVES);
        water.set_detail(wind, 0.1, longest * 0.18, 0.02);
        water
    }

    /// Replaces the swell with `count` waves from `wind`, the longest
    /// `longest` m, each shorter by a fixed ratio and turned a little off
    /// the wind, alternately to either side, as a directional spectrum's
    /// spread would. The steepness keeps the sum of Q·k·A under one, so no
    /// crest loops.
    pub fn set_swell(&mut self, wind: f32, longest: f32, height: f32, count: usize) {
        let count = count.min(MAX_WAVES);
        let mut wavelength = longest.max(0.5);
        for i in 0..count {
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let turn = wind + side * (0.18 + 0.11 * i as f32) * golden(i as u32 + 1);
            let amplitude = wavelength * 0.011 * height;
            let k = TAU / wavelength;
            self.waves[i] = Wave {
                dir: [turn.sin(), turn.cos()],
                wavelength,
                amplitude,
                steepness: (0.75 / (k * amplitude.max(1e-5) * count as f32)).min(1.0),
                phase: golden(i as u32 + 7) * TAU,
            };
            wavelength *= 0.71;
        }
        self.wave_count = count;
    }

    /// Replaces the detail with waves from `shortest` to `longest` m around
    /// `wind`, each `slope` steep (amplitude over wavelength), spread wider
    /// than the swell.
    pub fn set_detail(&mut self, wind: f32, shortest: f32, longest: f32, slope: f32) {
        let count = MAX_DETAIL;
        let ratio = (shortest.max(0.02) / longest.max(shortest)).powf(1.0 / (count - 1) as f32);
        let mut wavelength = longest.max(shortest);
        for i in 0..count {
            let turn = wind + (golden(i as u32 + 3) - 0.5) * 2.4;
            self.detail[i] = Wave {
                dir: [turn.sin(), turn.cos()],
                wavelength,
                amplitude: wavelength * slope,
                steepness: 0.0,
                phase: golden(i as u32 + 11) * TAU,
            };
            wavelength *= ratio;
        }
        self.detail_count = count;
    }

    /// The swell's displacement of the rest point `p` (x, z) over water
    /// `depth` deep: x, height, and z, m.
    #[must_use]
    pub fn displacement(&self, p: Vec2, depth: f32) -> Vec3 {
        let mut d = Vec3::ZERO;
        for wave in &self.waves[..self.wave_count] {
            let dir = Vec2::from(wave.dir);
            let amplitude = wave.amplitude * self.swell_gain * wave.shoaling(depth);
            let theta = wave.k() * dir.dot(p) - wave.omega() * self.time + wave.phase;
            let (s, c) = theta.sin_cos();
            d.x += wave.steepness * amplitude * dir.x * c;
            d.z += wave.steepness * amplitude * dir.y * c;
            d.y += amplitude * s;
        }
        d
    }

    /// How fast the water at rest point `p` moves, m/s: the time
    /// derivative of [`Self::displacement`], the orbital velocity a floating
    /// body is carried by.
    #[must_use]
    pub fn velocity(&self, p: Vec2, depth: f32) -> Vec3 {
        let mut v = Vec3::ZERO;
        for wave in &self.waves[..self.wave_count] {
            let dir = Vec2::from(wave.dir);
            let amplitude = wave.amplitude * self.swell_gain * wave.shoaling(depth);
            let omega = wave.omega();
            let theta = wave.k() * dir.dot(p) - omega * self.time + wave.phase;
            let (s, c) = theta.sin_cos();
            v.x += wave.steepness * amplitude * dir.x * omega * s;
            v.z += wave.steepness * amplitude * dir.y * omega * s;
            v.y -= amplitude * omega * c;
        }
        v
    }

    /// The surface's height above the point (x, z), over water `depth`
    /// deep at rest, ripples and spells included, m. The swell moves water
    /// sideways, so this finds the rest point whose displaced position lies
    /// over (x, z) by a few fixed-point steps, which converge while the
    /// waves don't loop.
    #[must_use]
    pub fn surface_height(&self, x: f32, z: f32, depth: f32) -> f32 {
        let target = Vec2::new(x, z);
        let depth = depth + self.level - self.rest;
        let calm = 1.0 - self.controls.ice_at(target);
        let mut rest = target;
        for _ in 0..4 {
            let d = self.displacement(rest, depth) * calm;
            rest = target - Vec2::new(d.x, d.z);
        }
        self.level + self.displacement(rest, depth).y * calm + self.ripple_height(target) * calm
            - self.controls.drop(target, depth)
    }

    /// The ripples' height at `p`, m.
    #[must_use]
    pub fn ripple_height(&self, p: Vec2) -> f32 {
        self.ripples
            .iter()
            .map(|ripple| ripple_profile(ripple, p, self.time).0)
            .sum()
    }

    /// Starts a ripple at `at` (x, z) now, replacing the oldest.
    pub fn add_ripple(&mut self, at: Vec2, strength: f32) {
        let slot = self
            .ripples
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let age = |r: &Ripple| {
                    if r.strength == 0.0 {
                        f32::INFINITY
                    } else {
                        self.time - r.start
                    }
                };
                age(b).total_cmp(&age(a))
            })
            .map_or(0, |(i, _)| i);
        self.ripples[slot] = Ripple {
            at: at.to_array(),
            start: self.time,
            strength,
        };
    }

    /// The uniform the water pass reads.
    #[must_use]
    pub fn uniform(&self, detail_count: usize) -> WaterUniform {
        let mut u = WaterUniform::zeroed();
        for (i, wave) in self.waves[..self.wave_count].iter().enumerate() {
            u.waves[i * 2] = [wave.dir[0], wave.dir[1], wave.k(), wave.amplitude];
            u.waves[i * 2 + 1] = [wave.steepness, wave.omega(), wave.phase, 0.0];
        }
        let detail = detail_count.min(self.detail_count);
        // The longest detail waves matter most; a tier that draws fewer
        // keeps those.
        for (i, wave) in self.detail[..detail].iter().enumerate() {
            u.detail[i * 2] = [wave.dir[0], wave.dir[1], wave.k(), wave.amplitude];
            u.detail[i * 2 + 1] = [wave.omega(), wave.phase, wave.wavelength, 0.0];
        }
        let mut ripples = 0;
        for ripple in &self.ripples {
            let age = self.time - ripple.start;
            if ripple.strength != 0.0 && (0.0..RIPPLE_LIFE).contains(&age) {
                u.ripples[ripples] = [ripple.at[0], ripple.at[1], ripple.start, ripple.strength];
                ripples += 1;
            }
        }
        u.extinction = [
            self.extinction[0],
            self.extinction[1],
            self.extinction[2],
            self.level,
        ];
        u.scatter = [self.scatter[0], self.scatter[1], self.scatter[2], self.foam];
        u.params = [
            self.time,
            self.wave_count as f32,
            detail as f32,
            ripples as f32,
        ];
        u.look = [
            self.roughness,
            self.caustics,
            self.level - self.rest,
            self.swell_gain,
        ];
        u
    }

    /// Whether every value is finite and in range.
    #[must_use]
    pub fn valid(&self) -> bool {
        let finite = |v: f32| v.is_finite();
        finite(self.level)
            && finite(self.time)
            && self.wave_count <= MAX_WAVES
            && self.detail_count <= MAX_DETAIL
            && self.waves.iter().chain(&self.detail).all(|w| {
                w.dir.iter().all(|v| v.is_finite())
                    && w.wavelength > 0.0
                    && finite(w.amplitude)
                    && finite(w.steepness)
                    && finite(w.phase)
            })
            && self.extinction.iter().all(|v| finite(*v) && *v >= 0.0)
            && self.scatter.iter().all(|v| finite(*v) && *v >= 0.0)
            && (0.0..=1.0).contains(&self.foam)
            && (0.0..=1.0).contains(&self.caustics)
            && (0.0..=1.0).contains(&self.roughness)
            && finite(self.rest)
            && finite(self.swell_gain)
            && self.controls.valid()
    }

    /// The frame uniform's water controls, in `photo.wgsl`'s order:
    /// `water_part`, `water_part_size`, `water_flow`, `water_whirl`,
    /// `water_wet`, and `water_ice`.
    #[must_use]
    pub fn control_terms(&self) -> [[f32; 4]; 6] {
        let c = &self.controls;
        let mut out = [[0.0; 4]; 6];
        if let Some(part) = c.part {
            out[0] = [part.center[0], part.center[1], part.dir[0], part.dir[1]];
            out[1] = [part.half_length, part.half_width, part.amount, 0.0];
        }
        if let Some(flow) = c.flow {
            out[2] = [
                flow.center[0],
                flow.center[1],
                flow.velocity[0],
                flow.velocity[1],
            ];
            out[1][3] = flow.radius;
        }
        if let Some(whirl) = c.whirl {
            out[3] = [
                whirl.center[0],
                whirl.center[1],
                whirl.radius,
                whirl.strength,
            ];
        }
        if let Some(wet) = c.wet {
            out[4] = [wet.center[0], wet.center[1], wet.radius, wet.amount];
        }
        if let Some(ice) = c.ice {
            out[5] = [ice.center[0], ice.center[1], ice.radius, ice.amount];
        }
        out
    }
}

/// Part Water's trench: the surface pulled down into two walls with dry
/// ground between them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Part {
    pub center: [f32; 2],
    /// Unit direction along the trench.
    pub dir: [f32; 2],
    pub half_length: f32,
    pub half_width: f32,
    /// How far the walls have opened, 0 to 1.
    pub amount: f32,
}

/// Redirect Flow: the water within `radius` moving at `velocity`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flow {
    pub center: [f32; 2],
    pub velocity: [f32; 2],
    pub radius: f32,
}

/// A whirlpool: a funnel `radius` wide whose `strength` (0 to 1) sets
/// how deep it sinks and how fast it turns.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Whirl {
    pub center: [f32; 2],
    pub radius: f32,
    pub strength: f32,
}

/// A disc of something on the water or ground: rain's wetness, or ice.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Disc {
    pub center: [f32; 2],
    pub radius: f32,
    /// How far it has set in, 0 to 1.
    pub amount: f32,
}

/// What spells are doing to the water, all optional.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    pub part: Option<Part>,
    pub flow: Option<Flow>,
    pub whirl: Option<Whirl>,
    /// Rain's wetness and puddles on the ground ([`Disc`]).
    pub wet: Option<Disc>,
    /// Ice on the water ([`Disc`]).
    pub ice: Option<Disc>,
}

/// How deep a whirlpool's funnel sinks at full strength, m.
pub const WHIRL_DEPTH: f32 = 3.2;
/// How far beyond a trench's half width its walls slope, m.
pub const PART_SLOPE: f32 = 2.5;

impl Controls {
    /// How far the spells pull the surface down at `p` over water `depth`
    /// deep: into a trench or a whirlpool's funnel, m.
    #[must_use]
    pub fn drop(&self, p: Vec2, depth: f32) -> f32 {
        let mut drop = 0.0;
        if self.part.is_some() {
            drop += self.trench(p, PART_SLOPE) * (depth.max(0.0) + 1.0);
        }
        if let Some(whirl) = self.whirl {
            let r = p.distance(Vec2::from(whirl.center)) / whirl.radius.max(0.1);
            drop += whirl.strength * WHIRL_DEPTH * (-(r * 2.2) * (r * 2.2)).exp();
        }
        drop
    }

    /// How much of `p` lies in Part Water's trench, 0 to 1, its walls
    /// sloping over `slope` m.
    #[must_use]
    pub fn trench(&self, p: Vec2, slope: f32) -> f32 {
        let Some(part) = self.part else {
            return 0.0;
        };
        let local = p - Vec2::from(part.center);
        let dir = Vec2::from(part.dir);
        let along = local.dot(dir).abs();
        let across = local.perp_dot(dir).abs();
        let ends = 1.0 - smooth(part.half_length - slope, part.half_length, along);
        let width = part.half_width * part.amount;
        let sides = 1.0 - smooth(width, width + slope * part.amount.max(0.05), across);
        ends * sides * part.amount.min(1.0)
    }

    /// The spells' current at `p`, m/s: Redirect Flow's and the
    /// whirlpool's swirl, which also pulls down near its eye (y).
    #[must_use]
    pub fn flow_at(&self, p: Vec2) -> Vec3 {
        let mut v = Vec3::ZERO;
        if let Some(flow) = self.flow {
            let r = p.distance(Vec2::from(flow.center));
            let k = 1.0 - smooth(0.7 * flow.radius, flow.radius, r);
            v += Vec3::new(flow.velocity[0], 0.0, flow.velocity[1]) * k;
        }
        if let Some(whirl) = self.whirl {
            let to = Vec2::from(whirl.center) - p;
            let r = to.length().max(0.3);
            let k = whirl.strength * (1.0 - smooth(0.6 * whirl.radius, 1.6 * whirl.radius, r));
            let swirl = Vec2::new(-to.y, to.x) / r * 3.2;
            let inward = to / r * 0.9;
            let flat = (swirl + inward) * k;
            let down = -2.4 * whirl.strength * (-(r / whirl.radius * 3.0).powi(2)).exp();
            v += Vec3::new(flat.x, down, flat.y);
        }
        v
    }

    /// How frozen the water at `p` is, 0 to 1.
    #[must_use]
    pub fn ice_at(&self, p: Vec2) -> f32 {
        self.ice.map_or(0.0, |ice| {
            let r = p.distance(Vec2::from(ice.center));
            ice.amount * (1.0 - smooth(ice.radius - 1.5, ice.radius, r))
        })
    }

    fn valid(&self) -> bool {
        let ok = |v: &[f32]| v.iter().all(|x| x.is_finite());
        self.part.is_none_or(|p| {
            ok(&[
                p.center[0],
                p.center[1],
                p.dir[0],
                p.dir[1],
                p.half_length,
                p.half_width,
                p.amount,
            ])
        }) && self.flow.is_none_or(|f| {
            ok(&[
                f.center[0],
                f.center[1],
                f.velocity[0],
                f.velocity[1],
                f.radius,
            ])
        }) && self
            .whirl
            .is_none_or(|w| ok(&[w.center[0], w.center[1], w.radius, w.strength]))
            && [self.wet, self.ice]
                .iter()
                .flatten()
                .all(|d| ok(&[d.center[0], d.center[1], d.radius, d.amount]))
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A ripple's height and radial slope at `p` at time `time`: a packet of
/// short waves riding an expanding ring, fading as it spreads and ages.
#[must_use]
pub fn ripple_profile(ripple: &Ripple, p: Vec2, time: f32) -> (f32, f32) {
    let age = time - ripple.start;
    if ripple.strength == 0.0 || !(0.0..RIPPLE_LIFE).contains(&age) {
        return (0.0, 0.0);
    }
    let r = p.distance(Vec2::from(ripple.at));
    let front = RIPPLE_SPEED * age;
    let x = r - front;
    let k = TAU / 0.45;
    let envelope = (-x * x / 0.18).exp() * (-age * 1.1).exp() / (1.0 + 1.5 * r);
    let (s, c) = (k * x).sin_cos();
    let a = ripple.strength * envelope;
    (a * s, a * k * c)
}

/// A low-discrepancy fraction for index `i`, from the golden ratio.
fn golden(i: u32) -> f32 {
    (i as f32 * 0.618_034).fract()
}

/// What [`Water::uniform`] writes, in the layout `water.wgsl` reads.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct WaterUniform {
    /// Per swell wave: direction, wavenumber, and amplitude; then
    /// steepness, angular frequency, and phase.
    pub waves: [[f32; 4]; 2 * MAX_WAVES],
    /// Per detail wave: direction, wavenumber, and amplitude; then angular
    /// frequency, phase, and wavelength.
    pub detail: [[f32; 4]; 2 * MAX_DETAIL],
    /// Per ripple: x, z, start, strength.
    pub ripples: [[f32; 4]; MAX_RIPPLES],
    /// rgb extinction, 1/m; w the sea's level.
    pub extinction: [f32; 4],
    /// rgb in-scatter; w foam.
    pub scatter: [f32; 4],
    /// Time, swell count, detail count, ripple count.
    pub params: [f32; 4],
    /// Roughness, caustics, the flood's rise over the rest level, and the
    /// swell's gain.
    pub look: [f32; 4],
}

/// How a vertex of a water surface is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// The sea: the swell at this scale (0 to 1) moves it, and the bed
    /// beneath takes the sea's color from [`Water::level`].
    Sea(f32),
    /// A stream or pool at its own height: flat, its detail carried along
    /// the vertex's flow, and its own depth tint drawn by the surface.
    Stream,
    /// A falling sheet: aerated, streaked along its fall.
    Fall,
}

impl Kind {
    /// The vertex's kind channel: the swell scale for the sea, 2 for a
    /// stream, and 3 for a fall.
    #[must_use]
    pub fn code(self) -> f32 {
        match self {
            Self::Sea(scale) => scale.clamp(0.0, 1.0),
            Self::Stream => 2.0,
            Self::Fall => 3.0,
        }
    }
}

/// One vertex of a water surface at rest.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct WaterVertex {
    /// Rest position, m.
    pub pos: [f32; 3],
    /// Water depth under the vertex, m; negative over dry ground.
    pub depth: f32,
    /// Flow velocity in the xz plane, m/s. A fall's points along its
    /// horizontal heading.
    pub flow: [f32; 2],
    /// Extra foam, 0 to 1: white water around a rock or under a fall.
    pub foam: f32,
    /// [`Kind::code`].
    pub kind: f32,
}

impl WaterVertex {
    #[must_use]
    pub fn new(pos: Vec3, depth: f32, kind: Kind) -> Self {
        Self {
            pos: pos.to_array(),
            depth,
            flow: [0.0; 2],
            foam: 0.0,
            kind: kind.code(),
        }
    }
}

/// A regular grid of water vertices, row by row (`cols` a row). The grid
/// may be warped in space; it only has to be a grid in its indices.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterPatch {
    pub cols: u32,
    pub rows: u32,
    pub vertices: Vec<WaterVertex>,
    /// Whether a low tier may draw every other row and column.
    pub decimate: bool,
    /// How high above its rest level a quad's corners may stand and still
    /// be drawn, m: [`DRY`], or more where a flood may rise.
    pub dry: f32,
}

/// The water of a zone at rest: one or more patches.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterSurface {
    pub patches: Vec<WaterPatch>,
}

impl WaterSurface {
    /// Every patch's vertices, in order.
    #[must_use]
    pub fn vertices(&self) -> Vec<WaterVertex> {
        self.patches
            .iter()
            .flat_map(|p| p.vertices.iter().copied())
            .collect()
    }

    /// The triangles that cover water: every quad of every patch with a
    /// corner under water, stepping `stride` rows and columns at a time in
    /// patches that may be decimated.
    #[must_use]
    pub fn indices(&self, stride: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut base = 0u32;
        for patch in &self.patches {
            let step = if patch.decimate { stride.max(1) } else { 1 };
            let at = |c: u32, r: u32| base + r * patch.cols + c;
            let dry = if patch.dry > 0.0 { patch.dry } else { DRY };
            let wet = |i: u32| patch.vertices[(i - base) as usize].depth > -dry;
            let mut r = 0;
            while r + 1 < patch.rows {
                let r2 = (r + step).min(patch.rows - 1);
                let mut c = 0;
                while c + 1 < patch.cols {
                    let c2 = (c + step).min(patch.cols - 1);
                    let quad = [at(c, r), at(c2, r), at(c2, r2), at(c, r2)];
                    if quad.iter().any(|&i| wet(i)) {
                        out.extend_from_slice(&[
                            quad[0], quad[2], quad[1], quad[0], quad[3], quad[2],
                        ]);
                    }
                    c = c2;
                }
                r = r2;
            }
            base += patch.vertices.len() as u32;
        }
        out
    }

    /// Checks that every patch is a whole grid of finite vertices.
    ///
    /// # Errors
    /// Names the first patch that is not.
    pub fn validate(&self) -> Result<(), String> {
        for (i, patch) in self.patches.iter().enumerate() {
            if patch.cols < 2 || patch.rows < 2 {
                return Err(format!("Water patch {i} is smaller than one quad"));
            }
            if patch.vertices.len() != (patch.cols * patch.rows) as usize {
                return Err(format!(
                    "Water patch {i} has {} vertices for a {}×{} grid",
                    patch.vertices.len(),
                    patch.cols,
                    patch.rows
                ));
            }
            if !patch.vertices.iter().all(|v| {
                v.pos.iter().chain(&v.flow).all(|c| c.is_finite())
                    && v.depth.is_finite()
                    && v.foam.is_finite()
                    && v.kind.is_finite()
            }) {
                return Err(format!("Water patch {i} has a vertex that is not finite"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A long wave's speed is the deep-water phase speed √(g·L / 2π).
    #[test]
    fn waves_follow_the_deep_water_dispersion_relation() {
        let wave = Wave {
            wavelength: 20.0,
            ..Wave::default()
        };
        let speed = wave.omega() / wave.k();
        let expected = (GRAVITY * 20.0 / TAU).sqrt();
        assert!((speed - expected).abs() < 1e-4);
        // A wave dies out over dry land and keeps its height in deep water.
        assert_eq!(wave.shoaling(0.0), 0.0);
        assert!(wave.shoaling(100.0) > 0.999);
    }

    /// The CPU height finds the displaced surface over a point: the rest
    /// point it settles on is displaced back onto that point.
    #[test]
    fn the_surface_height_lies_on_the_displaced_surface() {
        let mut sea = Water::sea(0.0, 0.4, 18.0, 1.0);
        sea.time = 7.3;
        for &(x, z) in &[(0.0, 0.0), (3.2, -7.5), (-12.0, 4.4)] {
            let target = Vec2::new(x, z);
            let mut rest = target;
            for _ in 0..12 {
                let d = sea.displacement(rest, 50.0);
                rest = target - Vec2::new(d.x, d.z);
            }
            let d = sea.displacement(rest, 50.0);
            assert!((rest + Vec2::new(d.x, d.z) - target).length() < 1e-3);
            let h = sea.surface_height(x, z, 50.0);
            assert!((h - d.y).abs() < 0.02, "{h} vs {}", d.y);
        }
    }

    /// No crest loops: the swell's summed Q·k·A stays under one.
    #[test]
    fn the_swell_never_loops() {
        let sea = Water::sea(0.0, 1.0, 30.0, 2.0);
        let sum: f32 = sea.waves[..sea.wave_count]
            .iter()
            .map(|w| w.steepness * w.k() * w.amplitude)
            .sum();
        assert!(sum <= 1.0, "{sum}");
        assert!(sea.valid());
    }

    /// A ripple spreads, then fades out; a new one replaces the oldest.
    #[test]
    fn ripples_spread_and_fade() {
        let mut water = Water::calm(0.0);
        water.add_ripple(Vec2::ZERO, 0.05);
        water.time = 1.0;
        let ring = water
            .ripple_height(Vec2::new(RIPPLE_SPEED + 0.11, 0.0))
            .abs();
        assert!(ring > 1e-3, "{ring}");
        water.time = RIPPLE_LIFE + 0.1;
        assert_eq!(water.ripple_height(Vec2::new(RIPPLE_SPEED, 0.0)), 0.0);
        for i in 0..MAX_RIPPLES + 3 {
            water.time = i as f32;
            water.add_ripple(Vec2::new(i as f32, 0.0), 0.01);
        }
        let newest = water
            .ripples
            .iter()
            .map(|r| r.start)
            .fold(f32::MIN, f32::max);
        assert_eq!(newest, (MAX_RIPPLES + 2) as f32);
        assert_eq!(water.uniform(MAX_DETAIL).params[3] as usize, 4);
    }

    /// A grid's quads over dry ground draw nothing, and a decimated patch
    /// draws a quarter as many triangles.
    #[test]
    fn dry_quads_are_skipped_and_low_tiers_decimate() {
        let mut patch = WaterPatch {
            cols: 9,
            rows: 9,
            decimate: true,
            ..WaterPatch::default()
        };
        for r in 0..9 {
            for c in 0..9 {
                let depth = if c < 4 { 2.0 } else { -5.0 };
                patch.vertices.push(WaterVertex::new(
                    Vec3::new(c as f32, 0.0, r as f32),
                    depth,
                    Kind::Sea(1.0),
                ));
            }
        }
        let surface = WaterSurface {
            patches: vec![patch],
        };
        surface.validate().unwrap();
        // Columns 0 to 4 have a wet corner: four quads across, eight down.
        assert_eq!(surface.indices(1).len(), 4 * 8 * 6);
        assert_eq!(surface.indices(2).len(), 2 * 4 * 6);
    }

    /// The uniform's layout is the one `water.wgsl` declares.
    #[test]
    fn the_water_uniform_matches_the_shader() {
        for gles in [false, true] {
            let shared = crate::shading::source(include_str!("photo.wgsl"));
            let source = verse_gfx::gles::wgsl(&shared, gles);
            let module = naga::front::wgsl::parse_str(&source).unwrap();
            let mut layouter = naga::proc::Layouter::default();
            layouter.update(module.to_ctx()).unwrap();
            let (ty, _) = module
                .types
                .iter()
                .find(|(_, ty)| ty.name.as_deref() == Some("WaterUniform"))
                .expect("water.wgsl declares WaterUniform");
            assert_eq!(
                layouter[ty].size as usize,
                std::mem::size_of::<WaterUniform>()
            );
        }
    }
}
