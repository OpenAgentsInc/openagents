//! A frame's water and the surfaces it moves: what both renderers' water
//! passes draw (`water.wgsl`).
//!
//! A zone puts two things in its meshes. Its world mesh carries a
//! [`WaterSurface`], the static grids of the water's rest shape (built by
//! hand or by [`super::bake`]), which a renderer uploads once. Each frame's
//! stage ([`crate::pbr::Neon::water`], or the imported renderer's
//! `water`) carries a [`Water`]: up to [`MAX_BODIES`] bodies, each with its
//! level, its Gerstner swell ([`Swell`], the terms of a
//! `physics::water::WaveSet`), and its optics, plus the clock, the fine
//! detail, the ripples of the moment, and the spells on the sea. Body 0 is
//! the sea when a stage has one: the physical renderer also tints and
//! lights what lies under it.
//!
//! The techniques are public ones, reimplemented here; `water.wgsl` names
//! them. [`Water::surface_height`] evaluates the sea's swell in `f32`
//! exactly as the shader does, so a floating body bobs on the surface the
//! player sees, and [`Swell`] holds the same terms `physics::water`
//! evaluates in `f64`.

use std::f32::consts::TAU;

use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};

pub use super::ripple::{MAX_SOURCES, Source, Wet};
pub use super::terms::{Swell, shoaling, wind_sea};
use super::{MAX_BODIES, preset::Preset};

/// Gravity's acceleration, m/s².
pub const GRAVITY: f32 = 9.81;
/// The most Gerstner terms a body carries (`physics::water::MAX_WAVES`).
pub const MAX_WAVES: usize = physics::water::MAX_WAVES;
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
/// The shore distance of a vertex nobody baked: far from any shore, m.
pub const OPEN_WATER: f32 = 1000.0;

/// One detail wave: it bends normals and never moves the surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wave {
    /// Unit direction of travel in the xz plane.
    pub dir: [f32; 2],
    /// Crest to crest, m.
    pub wavelength: f32,
    /// Crest height above the rest level, m.
    pub amplitude: f32,
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

/// The most boat hulls a frame masks the water out of.
pub const MAX_HULLS: usize = 4;

/// A boat's hull over the water, which keeps the water out of it: the
/// surface draws nothing inside its footprint under its gunwale.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hull {
    /// Its middle, x and z, m.
    pub center: [f32; 2],
    /// Unit direction toward the bow in the xz plane.
    pub dir: [f32; 2],
    /// Half its beam and half its length, m.
    pub half: [f32; 2],
    /// Its gunwale's height, m.
    pub top: f32,
}

/// One body of water as a frame draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// The level now, m.
    pub level: f32,
    /// The level its vertices were built at, m; a flood raises
    /// [`Self::level`] above it.
    pub rest: f32,
    /// Additional height change per meter along x and z, over the rest mesh.
    pub level_gradient: [f32; 2],
    /// The Gerstner terms ([`Swell::from_set`]).
    pub swell: Swell,
    /// The sea's wave spectrum, synthesized into cascades on body 0 only
    /// ([`super::ocean`]); its gameplay band is the one `physics::water`
    /// samples.
    pub spectrum: Option<physics::water::Spectrum>,
    /// Scales the swell's heights: above one in a storm or a flood.
    pub swell_gain: f32,
    /// Absorption per meter, linear rgb (Beer–Lambert).
    pub absorption: [f32; 3],
    /// The share of the light under the surface scattered back up toward
    /// the eye, linear rgb: the water's own color.
    pub scatter: [f32; 3],
    /// How much foam forms, 0 to 1.
    pub foam: f32,
    /// The calm surface's microfacet roughness.
    pub roughness: f32,
    /// How wide the shore's foam band is, m.
    pub shore_band: f32,
    /// How strongly light scatters through thin crests, 0 to 1.
    pub crest_scatter: f32,
    /// Whether the eye is in this body's water, which the zone knows: the
    /// surface then shades from below. The physical renderer finds it for
    /// the sea itself.
    pub eye_inside: bool,
}

impl Default for Body {
    fn default() -> Self {
        Self::still(0.0, &Preset::default())
    }
}

impl Body {
    /// Still water at `level` with `preset`'s optics.
    #[must_use]
    pub fn still(level: f32, preset: &Preset) -> Self {
        Self {
            level,
            rest: level,
            level_gradient: [0.0; 2],
            swell: Swell::default(),
            spectrum: None,
            swell_gain: 1.0,
            absorption: preset.absorption,
            scatter: preset.scatter,
            foam: preset.foam,
            roughness: preset.roughness,
            shore_band: preset.shore_band,
            crest_scatter: preset.crest_scatter,
            eye_inside: false,
        }
    }

    /// `body`'s level, waves, and `preset`'s optics, at `level`.
    #[must_use]
    pub fn from_physics(body: &physics::water::WaterBody, preset: &Preset) -> Self {
        let level = body.level.at(0.0).0 as f32;
        Self {
            swell: Swell::from_set(&body.waves),
            spectrum: body.waves.spectrum,
            ..Self::still(level, preset)
        }
    }

    /// The swell's displacement of the rest point `p` (x, z) over water
    /// `depth` deep at `time`, the spectral band's included: x, height,
    /// and z, m.
    #[must_use]
    pub fn displacement(&self, p: Vec2, depth: f32, time: f32) -> Vec3 {
        let angles = self.swell.angles(f64::from(time));
        self.swell.displacement(&angles, p, depth, self.swell_gain)
            + self.spectral(p, depth, time).map_or(Vec3::ZERO, |s| s.0)
    }

    /// How fast the water at rest point `p` moves at `time`, m/s.
    #[must_use]
    pub fn velocity(&self, p: Vec2, depth: f32, time: f32) -> Vec3 {
        let angles = self.swell.angles(f64::from(time));
        self.swell.velocity(&angles, p, depth, self.swell_gain)
            + self.spectral(p, depth, time).map_or(Vec3::ZERO, |s| s.1)
    }

    /// The spectrum's gameplay band at rest point `p` over water `depth`
    /// deep at `time`, as the shader draws it: displacement and velocity,
    /// each scaled by the swell gain and the shoaling gain
    /// ([`super::ocean::gain`]).
    #[must_use]
    pub fn spectral(&self, p: Vec2, depth: f32, time: f32) -> Option<(Vec3, Vec3)> {
        let spectrum = self.spectrum.as_ref()?;
        let field = physics::water::spectrum::field(spectrum, spectrum.tick_at(f64::from(time)))?;
        let v = field.sample(p.as_dvec2());
        let significant = field.significant as f32 * self.swell_gain;
        let g = self.swell_gain * super::ocean::gain(field.shoal as f32, depth, significant);
        Some((
            Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32) * g,
            Vec3::new(v[3] as f32, v[4] as f32, v[5] as f32) * g,
        ))
    }

    fn valid(&self) -> bool {
        let finite = |v: f32| v.is_finite();
        finite(self.level)
            && finite(self.rest)
            && self.level_gradient.iter().all(|v| v.is_finite())
            && finite(self.swell_gain)
            && self.swell.count <= MAX_WAVES
            && self.swell.terms.iter().all(|t| {
                t.dir.iter().all(|v| v.is_finite())
                    && finite(t.k)
                    && finite(t.amplitude)
                    && finite(t.q)
                    && t.phase.is_finite()
            })
            && self.spectrum.is_none_or(|s| s.validate().is_ok())
            && self.absorption.iter().all(|v| finite(*v) && *v >= 0.0)
            && self.scatter.iter().all(|v| finite(*v) && *v >= 0.0)
            && (0.0..=1.0).contains(&self.foam)
            && (0.0..=1.0).contains(&self.roughness)
            && (0.0..=1.0).contains(&self.crest_scatter)
            && finite(self.shore_band)
    }
}

/// A sky and a sun for a renderer that has neither of its own (the
/// imported renderer's chamber), in its linear units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sky {
    pub zenith: [f32; 3],
    pub horizon: [f32; 3],
    /// Toward the sun, unit.
    pub sun_dir: [f32; 3],
    /// The sun's illuminance in the renderer's units; zero for none.
    pub sun_illuminance: f32,
    pub sun_color: [f32; 3],
}

/// What a frame's water looks like and how it moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Water {
    /// The bodies; the first [`Self::count`] are drawn. Body 0 is the sea
    /// on a stage that has one.
    pub bodies: [Body; MAX_BODIES],
    pub count: usize,
    /// Whether body 0 is a sea whose bed the physical renderer's lit
    /// surfaces tint and light themselves ([`Kind::Sea`] vertices), with
    /// its caustics, spells, and the view from under it.
    pub sea: bool,
    /// What spells are doing to the sea ([`Controls`]).
    pub controls: Controls,
    /// The water clock, s.
    pub time: f32,
    pub detail: [Wave; MAX_DETAIL],
    pub detail_count: usize,
    pub ripples: [Ripple; MAX_RIPPLES],
    /// How strongly the caustics focus sunlight on the sea's bed, 0 to 1.
    pub caustics: f32,
    /// The imported renderer's sky and sun, when set.
    pub sky: Option<Sky>,
    /// What writes into the ripple and foam field this frame
    /// ([`super::ripple`]): the first [`Self::source_count`] movers,
    /// impacts, and spells. The renderer keeps its tier's budget of them.
    pub sources: [Source; MAX_SOURCES],
    pub source_count: usize,
    /// Where the field's water is and how its current runs; wet and
    /// still everywhere without it.
    pub wet: Option<Wet>,
    /// The boats' hulls the water is masked out of; the first
    /// [`Self::hull_count`] apply.
    pub hulls: [Hull; MAX_HULLS],
    pub hull_count: usize,
    /// The surface over the eye when it stands in or near a body, from the
    /// zone's surface query: the physical renderer splits the view at it
    /// ([`super::under`]). It finds the sea's itself.
    pub eye: Option<super::under::EyeSurface>,
    /// The weather's rain on the water and the ground ([`super::rain`]).
    pub rain: super::rain::Rain,
    /// A baked harbor mask for the sea's swell.
    pub shelter: Option<super::shelter::Shelter>,
}

impl Default for Water {
    fn default() -> Self {
        Self::calm(0.0)
    }
}

impl Water {
    /// One body of still, clear water at `level` with only fine detail on it.
    #[must_use]
    pub fn calm(level: f32) -> Self {
        let mut water = Self {
            bodies: [Body::still(level, &Preset::default()); MAX_BODIES],
            count: 1,
            sea: false,
            controls: Controls::default(),
            time: 0.0,
            detail: [Wave::default(); MAX_DETAIL],
            detail_count: 0,
            ripples: [Ripple::default(); MAX_RIPPLES],
            caustics: 0.8,
            sky: None,
            sources: [Source::default(); MAX_SOURCES],
            source_count: 0,
            wet: None,
            hulls: [Hull::default(); MAX_HULLS],
            hull_count: 0,
            eye: None,
            rain: super::rain::Rain::default(),
            shelter: None,
        };
        water.set_detail(0.3, 0.12, 2.4, 0.016);
        water
    }

    /// A sea at `level` whose waves come from `spectrum`: a spectral sea
    /// with no Gerstner terms, and fine detail from the spectrum's wind.
    #[must_use]
    pub fn ocean(level: f32, spectrum: physics::water::Spectrum) -> Self {
        let mut water = Self::calm(level);
        water.sea = true;
        water.bodies[0].spectrum = Some(spectrum);
        let shortest = spectrum.patch() / physics::water::spectrum::RATIO / 31.0;
        water.set_detail(spectrum.wind as f32, 0.08, shortest as f32, 0.02);
        water
    }

    /// A sea at `level` with a wind sea's swell ([`wind_sea`]): waves from
    /// `wind` (radians about +Y, the direction they travel toward, as the
    /// controller's yaw: 0 is +z), the longest `longest` m, `height`
    /// setting the crest heights relative to wavelength.
    #[must_use]
    pub fn sea(level: f32, wind: f32, longest: f32, height: f32) -> Self {
        let mut water = Self::calm(level);
        water.sea = true;
        water.bodies[0].swell = Swell::from_set(&wind_sea(wind, longest, height, MAX_WAVES));
        water.set_detail(wind, 0.1, longest * 0.18, 0.02);
        water
    }

    /// Body 0, the sea on a stage that has one.
    #[must_use]
    pub fn sea_body(&self) -> &Body {
        &self.bodies[0]
    }

    /// Body 0, to change.
    pub fn sea_body_mut(&mut self) -> &mut Body {
        &mut self.bodies[0]
    }

    /// The sea's level now, m.
    #[must_use]
    pub fn level(&self) -> f32 {
        self.bodies[0].level
    }

    /// Adds a body and returns its index, or none when all are taken.
    pub fn add(&mut self, body: Body) -> Option<usize> {
        if self.count >= MAX_BODIES {
            return None;
        }
        self.bodies[self.count] = body;
        self.count += 1;
        Some(self.count - 1)
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
                phase: golden(i as u32 + 11) * TAU,
            };
            wavelength *= ratio;
        }
        self.detail_count = count;
    }

    /// The sea's displacement of the rest point `p` (x, z) over water
    /// `depth` deep: x, height, and z, m.
    #[must_use]
    pub fn displacement(&self, p: Vec2, depth: f32) -> Vec3 {
        self.bodies[0].displacement(p, depth, self.time)
    }

    /// How fast the sea's water at rest point `p` moves, m/s: the orbital
    /// velocity a floating body is carried by.
    #[must_use]
    pub fn velocity(&self, p: Vec2, depth: f32) -> Vec3 {
        self.bodies[0].velocity(p, depth, self.time)
    }

    /// The sea's height above the point (x, z), over water `depth` deep at
    /// rest, ripples and spells included, m. The swell moves water
    /// sideways, so this finds the rest point whose displaced position lies
    /// over (x, z) by a few fixed-point steps, which converge while the
    /// waves don't loop.
    #[must_use]
    pub fn surface_height(&self, x: f32, z: f32, depth: f32) -> f32 {
        let sea = &self.bodies[0];
        let target = Vec2::new(x, z);
        let depth = depth + sea.level - sea.rest;
        let calm = 1.0 - self.controls.ice_at(target);
        let angles = sea.swell.angles(f64::from(self.time));
        let at = |p: Vec2| {
            (sea.swell.displacement(&angles, p, depth, sea.swell_gain)
                + sea
                    .spectral(p, depth, self.time)
                    .map_or(Vec3::ZERO, |s| s.0))
                * calm
                * self.shelter.map_or(1.0, |mask| mask.sample(p)[0])
        };
        let mut rest = target;
        for _ in 0..4 {
            let d = at(rest);
            rest = target - Vec2::new(d.x, d.z);
        }
        sea.level + at(rest).y + self.ripple_height(target) * calm
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

    /// Lists `source` for the ripple field this frame; past
    /// [`MAX_SOURCES`] the weakest gives way.
    pub fn add_source(&mut self, source: Source) {
        if self.source_count < MAX_SOURCES {
            self.sources[self.source_count] = source;
            self.source_count += 1;
            return;
        }
        let weakest = self
            .sources
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.strength.abs().total_cmp(&b.1.strength.abs()))
            .map_or(0, |(i, _)| i);
        if self.sources[weakest].strength.abs() < source.strength.abs() {
            self.sources[weakest] = source;
        }
    }

    /// Masks the water out of `hull`; past [`MAX_HULLS`] it is ignored.
    pub fn add_hull(&mut self, hull: Hull) {
        if self.hull_count < MAX_HULLS {
            self.hulls[self.hull_count] = hull;
            self.hull_count += 1;
        }
    }

    /// This frame's sources.
    #[must_use]
    pub fn sources(&self) -> &[Source] {
        &self.sources[..self.source_count.min(MAX_SOURCES)]
    }

    /// The uniform both water passes read, each body's swell at the water
    /// clock.
    #[must_use]
    pub fn uniform(&self) -> WaterUniform {
        self.uniform_by(|swell| swell.angles(f64::from(self.time)))
    }

    /// As [`Self::uniform`], each body's swell at whole tick `tick` of its
    /// own clock, exactly as `physics::water` samples it there.
    #[must_use]
    pub fn uniform_at_tick(&self, tick: u64) -> WaterUniform {
        self.uniform_by(|swell| swell.angles_at(tick, 0.0))
    }

    fn uniform_by(&self, angles: impl Fn(&Swell) -> [f32; MAX_WAVES]) -> WaterUniform {
        let mut u = WaterUniform::zeroed();
        for (slot, body) in u.bodies.iter_mut().zip(&self.bodies[..self.count]) {
            let angles = angles(&body.swell);
            slot.waves = body.swell.rows(&angles);
            let a = body.absorption;
            slot.absorb = [a[0], a[1], a[2], body.level];
            let s = body.scatter;
            slot.scatter = [s[0], s[1], s[2], body.foam];
            slot.params = [
                body.swell.count as f32,
                body.roughness,
                body.swell_gain,
                if body.eye_inside { 1.0 } else { 0.0 },
            ];
            slot.rest = [body.rest, body.shore_band, body.crest_scatter, 0.0];
            slot.level_gradient = [body.level_gradient[0], body.level_gradient[1], 0.0, 0.0];
        }
        for (i, wave) in self.detail[..self.detail_count].iter().enumerate() {
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
        u.params = [
            self.time,
            self.detail_count as f32,
            ripples as f32,
            self.count as f32,
        ];
        u.look = [self.caustics, 0.0, self.rain.rain, 0.0];
        for (i, hull) in self.hulls[..self.hull_count.min(MAX_HULLS)]
            .iter()
            .enumerate()
        {
            u.hulls[i * 2] = [hull.center[0], hull.center[1], hull.dir[0], hull.dir[1]];
            u.hulls[i * 2 + 1] = [hull.half[0], hull.half[1], hull.top, 0.0];
        }
        if let Some(sky) = self.sky {
            let (z, h, d, c) = (sky.zenith, sky.horizon, sky.sun_dir, sky.sun_color);
            u.sky_zenith = [z[0], z[1], z[2], 1.0];
            u.sky_horizon = [h[0], h[1], h[2], 0.0];
            u.sun = [d[0], d[1], d[2], sky.sun_illuminance];
            u.sun_color = [c[0], c[1], c[2], 0.0];
        }
        u
    }

    /// Whether every value is finite and in range.
    #[must_use]
    pub fn valid(&self) -> bool {
        let finite = |v: f32| v.is_finite();
        (1..=MAX_BODIES).contains(&self.count)
            && self.bodies[..self.count].iter().all(Body::valid)
            && finite(self.time)
            && self.detail_count <= MAX_DETAIL
            && self.detail.iter().all(|w| {
                w.dir.iter().all(|v| v.is_finite())
                    && w.wavelength > 0.0
                    && finite(w.amplitude)
                    && finite(w.phase)
            })
            && (0.0..=1.0).contains(&self.caustics)
            && self.eye.is_none_or(|e| e.valid(self.count))
            && self.controls.valid()
            && self.rain.valid()
            && self.shelter.is_none_or(super::shelter::Shelter::valid)
            && self.source_count <= MAX_SOURCES
            && self.hull_count <= MAX_HULLS
            && self.hulls.iter().all(|h| {
                h.center
                    .iter()
                    .chain(&h.dir)
                    .chain(&h.half)
                    .all(|v| v.is_finite())
                    && h.top.is_finite()
            })
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

/// One body's rows of [`WaterUniform`], in `water.wgsl`'s `WaterBody`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct BodyUniform {
    /// Per term: direction, wavenumber, amplitude; then q, the angle's
    /// constant part now, ω, and 0.
    pub waves: [[f32; 4]; 2 * MAX_WAVES],
    /// rgb absorption; w the level now.
    pub absorb: [f32; 4],
    /// rgb in-scatter; w foam.
    pub scatter: [f32; 4],
    /// Term count, roughness, swell gain, and 1 when the eye is inside.
    pub params: [f32; 4],
    /// Rest level, shore band, crest scattering, and 0.
    pub rest: [f32; 4],
    /// Height change per meter along x and z; two unused components.
    pub level_gradient: [f32; 4],
}

/// What [`Water::uniform`] writes, in the layout `water.wgsl` reads.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct WaterUniform {
    pub bodies: [BodyUniform; MAX_BODIES],
    /// Per detail wave: direction, wavenumber, and amplitude; then angular
    /// frequency, phase, and wavelength.
    pub detail: [[f32; 4]; 2 * MAX_DETAIL],
    /// Per ripple: x, z, start, strength.
    pub ripples: [[f32; 4]; MAX_RIPPLES],
    /// Time, detail count, ripple count, and body count.
    pub params: [f32; 4],
    /// Caustics, the angle a pixel spans (set by the renderer), the rain
    /// ([`super::rain::Rain::rain`]), and one spare.
    pub look: [f32; 4],
    pub sky_zenith: [f32; 4],
    pub sky_horizon: [f32; 4],
    pub sun: [f32; 4],
    pub sun_color: [f32; 4],
    /// The spectral sea's rows ([`super::ocean::rows`]), which the
    /// renderer fills for its tier; zero without one.
    pub ocean: [[f32; 4]; super::ocean::ROWS],
    /// The ocean's clipmap ([`super::clipmap::rows`]), which the physical
    /// renderer fills from the eye; zero without one.
    pub clip: [[f32; 4]; super::clipmap::ROWS],
    /// The streamed field's shape ([`super::field::Stream::rows`]).
    pub field: [[f32; 4]; super::field::ROWS],
    /// The page each slot of the field's atlas holds.
    pub field_pages: [[f32; 4]; super::field::SLOT_ROWS],
    /// The ripple field's window ([`super::ripple::Ripples::window`]):
    /// its corner (x, z), side (m), and layer in `water_waves`; side 0
    /// draws none.
    pub ripple: [f32; 4],
    /// Shelter mask origin, side (0 when absent), and array layer.
    pub shelter: [f32; 4],
    /// Per hull ([`Hull`]): its middle and heading; then its half beam,
    /// half length, and gunwale. A half beam of 0 masks nothing.
    pub hulls: [[f32; 4]; 2 * MAX_HULLS],
}

/// How a vertex of a water surface is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// The sea: its body's swell at this scale (0 to 1) moves it, and on
    /// the physical renderer the bed beneath takes the sea's color.
    Sea(f32),
    /// A pond, lake, pool, or calm river: its body's swell at this scale
    /// (0 to 1) moves it, and the surface itself absorbs over the baked
    /// depth under it.
    Body(f32),
    /// A stream or pool at its own height: [`Kind::Body`] without swell,
    /// its detail carried along the vertex's flow.
    Stream,
    /// A falling sheet: aerated, streaked along its fall.
    Fall,
    /// A free body of water, such as an orb a spell holds in the air or the
    /// stream that feeds it: a closed surface around a center, wobbling by
    /// this much (0 to 1), refracting like a ball lens, and electrified
    /// when struck by lightning. [`WaterVertex::orb`] builds its vertices.
    /// The physical renderer alone draws it.
    Orb(f32),
}

impl Kind {
    /// The vertex's kind channel: the swell scale for the sea, 2 plus up to
    /// 0.98 for a body (2 for a stream), 3 for a fall, and 4 plus the
    /// wobble for an orb.
    #[must_use]
    pub fn code(self) -> f32 {
        match self {
            Self::Sea(scale) => scale.clamp(0.0, 1.0),
            Self::Body(scale) => 2.0 + 0.98 * scale.clamp(0.0, 1.0),
            Self::Stream => 2.0,
            Self::Fall => 3.0,
            Self::Orb(wobble) => 4.0 + wobble.clamp(0.0, 0.99),
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
    /// Distance to the shore, m ([`super::bake::shore_distance`]);
    /// [`OPEN_WATER`] when nobody baked it.
    pub shore: f32,
    /// The body whose terms move and color it ([`Water::bodies`]).
    pub body: f32,
}

/// The vertex attributes both water pipelines read.
pub const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
    0 => Float32x3, 1 => Float32, 2 => Float32x2, 3 => Float32, 4 => Float32,
    5 => Float32, 6 => Float32
];

impl WaterVertex {
    #[must_use]
    pub fn new(pos: Vec3, depth: f32, kind: Kind) -> Self {
        Self {
            pos: pos.to_array(),
            depth,
            flow: [0.0; 2],
            foam: 0.0,
            kind: kind.code(),
            shore: OPEN_WATER,
            body: 0.0,
        }
    }

    /// This vertex in body `body`.
    #[must_use]
    pub fn in_body(mut self, body: usize) -> Self {
        self.body = body.min(MAX_BODIES - 1) as f32;
        self
    }

    /// A vertex of an orb ([`Kind::Orb`]) centered at `center`, `offset`
    /// from it at rest, wobbling by `wobble` and electrified by `charge`
    /// (each 0 to 1). The surface's center rides in the depth and flow
    /// channels and the charge in the foam channel, so the vertex keeps
    /// the water pass's one layout; the pass moves it by the wobble.
    #[must_use]
    pub fn orb(center: Vec3, offset: Vec3, wobble: f32, charge: f32) -> Self {
        Self {
            pos: offset.to_array(),
            depth: center.y,
            flow: [center.x, center.z],
            foam: charge.clamp(0.0, 1.0),
            kind: Kind::Orb(wobble).code(),
            shore: OPEN_WATER,
            body: 0.0,
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

impl WaterPatch {
    /// Bakes each vertex's distance to the shore from the grid's own wet
    /// and dry vertices ([`super::bake::shore_distance`]), with the grid's
    /// spacing measured at each vertex, so a warped grid's distances stay
    /// close to meters.
    pub fn bake_shore(&mut self) {
        let (cols, rows) = (self.cols as usize, self.rows as usize);
        if cols < 2 || rows < 2 || self.vertices.len() != cols * rows {
            return;
        }
        let wet: Vec<bool> = self.vertices.iter().map(|v| v.depth > 0.0).collect();
        let cells = super::bake::shore_distance(cols, rows, &wet, 1.0);
        let at = |c: usize, r: usize| Vec3::from(self.vertices[r * cols + c].pos);
        let spacing: Vec<f32> = (0..rows)
            .flat_map(|r| (0..cols).map(move |c| (c, r)))
            .map(|(c, r)| {
                // Central differences span two cells, one-sided ones one.
                let (c0, c1) = (c.saturating_sub(1), (c + 1).min(cols - 1));
                let (r0, r1) = (r.saturating_sub(1), (r + 1).min(rows - 1));
                let dx = at(c1, r).distance(at(c0, r)) / (c1 - c0) as f32;
                let dz = at(c, r1).distance(at(c, r0)) / (r1 - r0) as f32;
                0.5 * (dx + dz)
            })
            .collect();
        for ((v, d), s) in self.vertices.iter_mut().zip(cells).zip(spacing) {
            v.shore = (d * s).min(OPEN_WATER);
        }
    }
}

/// An unbounded body drawn on a clipmap around the eye
/// ([`super::clipmap`]) instead of a patch: a zone's ocean.
#[derive(Clone, Debug, PartialEq)]
pub struct Ocean {
    /// The body whose terms move and color it ([`Water::bodies`]).
    pub body: usize,
    /// Whether it draws as [`Kind::Sea`] (the physical renderer tints the
    /// bed under it) rather than [`Kind::Body`].
    pub sea: bool,
    /// Its depth, shore distance, and current, streamed by page
    /// ([`super::field`]); open water everywhere without one.
    pub field: Option<std::sync::Arc<super::field::Field>>,
}

/// The water of a zone at rest: one or more patches, and an ocean.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WaterSurface {
    pub patches: Vec<WaterPatch>,
    /// The ocean, which the physical renderer draws on its clipmap before
    /// the patches. The imported renderer draws patches only.
    pub ocean: Option<Ocean>,
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
        self.ranges(stride).0
    }

    /// As [`Self::indices`], with each patch's range of them, so a
    /// renderer can draw the patches in order, each through both halves of
    /// its water pass.
    #[must_use]
    pub fn ranges(&self, stride: u32) -> (Vec<u32>, Vec<std::ops::Range<u32>>) {
        let mut out = Vec::new();
        let mut ranges = Vec::new();
        let mut base = 0u32;
        for patch in &self.patches {
            let start = out.len() as u32;
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
            ranges.push(start..out.len() as u32);
            base += patch.vertices.len() as u32;
        }
        (out, ranges)
    }

    /// Checks that every patch is a whole grid of finite vertices.
    ///
    /// # Errors
    /// Names the first patch that is not.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ocean) = &self.ocean
            && ocean.body >= MAX_BODIES
        {
            return Err(format!("The ocean's body {} is not a body", ocean.body));
        }
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
                    && v.shore.is_finite()
                    && (0.0..MAX_BODIES as f32).contains(&v.body)
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
        assert_eq!(water.uniform().params[2] as usize, 4);
    }

    /// A grid's quads over dry ground draw nothing, and a decimated patch
    /// draws a quarter as many triangles; each patch's range is its own.
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
        patch.bake_shore();
        // The column next to the dry ground is a meter from it.
        assert!((patch.vertices[3].shore - 1.0).abs() < 1e-4);
        assert!((patch.vertices[0].shore - 4.0).abs() < 1e-4);
        let surface = WaterSurface {
            patches: vec![patch.clone(), patch],
            ..WaterSurface::default()
        };
        surface.validate().unwrap();
        // Columns 0 to 4 have a wet corner: four quads across, eight down.
        assert_eq!(surface.indices(1).len(), 2 * 4 * 8 * 6);
        let (_, ranges) = surface.ranges(2);
        assert_eq!(ranges, vec![0..2 * 4 * 6, 2 * 4 * 6..2 * 2 * 4 * 6]);
    }

    /// The uniform's layout is the one `water.wgsl` declares, in both
    /// renderers.
    #[test]
    fn the_water_uniform_matches_the_shader() {
        for shader in [
            include_str!("../pbr/photo.wgsl"),
            include_str!("../imported/scene.wgsl"),
        ] {
            for gles in [false, true] {
                let shared = crate::shading::source(shader);
                let source = verse_gfx::gles::wgsl(&shared, gles);
                let module = naga::front::wgsl::parse_str(&source).unwrap();
                let mut layouter = naga::proc::Layouter::default();
                layouter.update(module.to_ctx()).unwrap();
                let size = |name: &str| {
                    let (ty, _) = module
                        .types
                        .iter()
                        .find(|(_, ty)| ty.name.as_deref() == Some(name))
                        .expect("water.wgsl declares it");
                    layouter[ty].size as usize
                };
                assert_eq!(size("WaterUniform"), std::mem::size_of::<WaterUniform>());
                assert_eq!(size("WaterBody"), std::mem::size_of::<BodyUniform>());
                // GLES and WebGL2 guarantee 16 KiB a uniform block.
                assert!(size("WaterUniform") <= 16_384);
            }
        }
    }

    /// The sea's CPU surface and velocity are the shader's terms: at the
    /// same angles they agree with `Swell` itself.
    #[test]
    fn the_sea_reads_its_body_s_swell() {
        let mut sea = Water::sea(1.5, 0.25, 22.0, 1.0);
        sea.time = 12.25;
        let body = sea.sea_body();
        let angles = body.swell.angles(12.25);
        let p = Vec2::new(4.0, -3.0);
        assert_eq!(
            sea.displacement(p, 30.0),
            body.swell.displacement(&angles, p, 30.0, 1.0)
        );
        assert!(sea.valid());
        let u = sea.uniform();
        assert_eq!(u.params[3], 1.0);
        assert_eq!(u.bodies[0].absorb[3], 1.5);
        assert_eq!(u.bodies[0].params[0], 8.0);
    }
}
