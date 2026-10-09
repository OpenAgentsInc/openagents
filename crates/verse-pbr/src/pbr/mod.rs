//! Physically lit geometry and a physical sky, for zones drawn in real units.
//!
//! The amber zones draw flat faces and lines in display colors. A zone that
//! wants photographic light instead emits [`LitVertex`] triangles, additive
//! [`GlowVertex`] quads, and one [`Sky`] per frame. When a frame carries a
//! [`Sky`], the renderer switches to the path in `gpu.rs`: a sun shadow map,
//! a floating-point scene in pre-exposed luminance, the Sun, Earth, Moon, and
//! catalogue stars drawn at infinity, bloom, exposure, the zone's color grade,
//! and tone mapping. The last four are `output.rs`, which the summoning
//! chamber shares.
//!
//! Units are photometric. Illuminance is in lux and luminance in candela per
//! square meter. Shaders multiply every luminance by the camera's exposure
//! before writing it, so half-precision targets never overflow.
//!
//! The techniques follow public sources: Karis, "Real Shading in Unreal Engine
//! 4" (SIGGRAPH 2013); Lagarde and de Rousiers, "Moving Frostbite to PBR"
//! (2014); Walter et al. (2007) and Heitz (2014) for the microfacet model;
//! Kulla and Conty (2017) for multiple scattering; Fernando (2005) for
//! percentage-closer soft shadows; Jimenez (2014) for bloom; and the Khronos
//! PBR Neutral tone mapper. See `docs/research/unreal/2026-09-27-lagrange-realism-audit.md`.
//!
//! Authored props with base-color images, alpha-masked foliage, and glass
//! draw through [`textured`] in the same frames.

pub mod bake;
pub mod baked_layers;
pub mod environment;
pub mod gpu;
pub mod instanced;
pub mod output;
pub mod relight;
pub mod screen;
pub mod sky;
pub mod textured;
pub mod textured_bake;
/// The water a frame draws, at its old path ([`crate::water::frame`]).
pub use crate::water::frame as water;

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Vec3};
pub use verse_engine::lighting::{Grade, HeightFog};

/// Solar illuminance at Sun–Earth L1, lux. The solar constant (1361 W/m²) at
/// 0.99 AU with a luminous efficacy of about 94 lm/W.
pub const SUN_ILLUMINANCE: f32 = 130_000.0;

/// One vertex of a physically lit triangle.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct LitVertex {
    /// World position in meters.
    pub pos: [f32; 3],
    /// Unit surface normal.
    pub normal: [f32; 3],
    /// Unit tangent, the brushing direction for anisotropic metal.
    pub tangent: [f32; 3],
    /// Object-space position, so procedural detail moves with its part.
    pub local: [f32; 3],
    /// Linear base color: albedo for dielectrics, F0 for metals.
    pub color: [f32; 3],
    /// Metallic, roughness, [`Material`] code, and baked ambient occlusion.
    pub params: [f32; 4],
}

/// One vertex of an additive, emissive quad (sunlit specks and glints).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct GlowVertex {
    /// World position in meters.
    pub pos: [f32; 3],
    /// Luminance in candela per square meter, before exposure.
    pub radiance: [f32; 3],
    /// Quad coordinate in [-1, 1] for a soft round falloff.
    pub uv: [f32; 2],
}

/// Surface families with their own shading lobes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// White thermal paint (Z-93 or S13G class): a rough dielectric.
    WhitePaint,
    /// Bare aluminium, brushed along the tangent.
    Aluminium,
    /// Aluminized Kapton multi-layer insulation: crinkled facets under an
    /// amber polyimide coat.
    Mli,
    /// A solar cell under cover glass with a magnesium fluoride coating.
    SolarCell,
    /// A gold-film visor under a polycarbonate coat.
    Visor,
    /// Beta cloth and the suit's outer fabric: rough with a sheen lobe.
    Fabric,
    /// A silvered-Teflon radiator: a mirror metal under a clear coat.
    Radiator,
    /// Safety-yellow paint.
    SafetyPaint,
    /// Dark anodized or composite hardware.
    Dark,
    /// Paint under a clear lacquer coat, as on a bowling ball: the base's own
    /// roughness beneath a thin glossy layer.
    Lacquer,
    /// A matte stage floor under a studio light. Its occlusion channel fades
    /// it into the stage's field, not only its bounce light, so a pool of
    /// light on it ends softly at the pool's rim.
    Stage,
}

impl Material {
    /// The shader's material code.
    #[must_use]
    pub fn code(self) -> f32 {
        match self {
            Self::WhitePaint | Self::SafetyPaint | Self::Dark => 0.0,
            Self::Aluminium => 1.0,
            Self::Mli => 2.0,
            Self::SolarCell => 3.0,
            Self::Visor => 4.0,
            Self::Fabric => 5.0,
            // The shader gives code 6 a clear coat over the base layer.
            Self::Radiator | Self::Lacquer => 6.0,
            Self::Stage => 7.0,
        }
    }

    /// Linear base color, metallic, and perceptual roughness. Albedos follow
    /// measured solar absorptance (Gilmore, *Spacecraft Thermal Control
    /// Handbook*); conductor reflectance follows published F0 tables.
    #[must_use]
    pub fn parameters(self) -> ([f32; 3], f32, f32) {
        match self {
            Self::WhitePaint => ([0.82, 0.82, 0.80], 0.0, 0.85),
            Self::Aluminium => ([0.91, 0.92, 0.92], 1.0, 0.32),
            Self::Mli => ([0.91, 0.92, 0.92], 1.0, 0.14),
            Self::SolarCell => ([0.03, 0.04, 0.10], 0.0, 0.7),
            Self::Visor => ([1.0, 0.77, 0.34], 1.0, 0.05),
            Self::Fabric => ([0.75, 0.75, 0.73], 0.0, 0.9),
            Self::Radiator => ([0.97, 0.96, 0.92], 1.0, 0.12),
            Self::SafetyPaint => ([0.80, 0.50, 0.04], 0.0, 0.6),
            Self::Dark => ([0.06, 0.06, 0.07], 0.0, 0.55),
            Self::Lacquer => ([0.80, 0.80, 0.78], 0.0, 0.4),
            Self::Stage => ([0.07, 0.07, 0.07], 0.0, 0.8),
        }
    }

    /// Diffuse albedo used by the light bake: zero for metals, whose light
    /// leaves specularly and is approximated by their F0.
    #[must_use]
    pub fn bounce_albedo(self) -> [f32; 3] {
        let (color, metallic, _) = self.parameters();
        match self {
            // A crinkled metal scatters over a wide cone; treat half as diffuse.
            Self::Mli => [0.45, 0.33, 0.12],
            Self::Radiator => [0.3, 0.3, 0.3],
            _ if metallic > 0.5 => color.map(|c| c * 0.4),
            _ => color,
        }
    }
}

/// The camera's photographic settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Exposure value at ISO 100. Sunny 16 in vacuum is about 15.
    pub ev100: f32,
    /// Adapt exposure to the scene between `ev_min` and `ev_max`.
    pub auto_exposure: bool,
    pub ev_min: f32,
    pub ev_max: f32,
    /// Shadow lift toward mid-gray from 0 (none) to 1 (strong).
    pub local_exposure: f32,
    /// Fraction of bloom energy added back, 0.0 to 0.2.
    pub bloom: f32,
    /// Sensor noise strength in shadows.
    pub grain: f32,
    /// Natural vignetting strength.
    pub vignette: f32,
    /// Lateral chromatic aberration in pixels at the frame corner.
    pub fringe: f32,
    /// White balance in kelvin (sunlight is about 5,800 K).
    pub white_balance: f32,
    /// Multiplier for stars and the Milky Way, for a dark-adapted eye.
    pub star_gain: f32,
    /// Lens ghost strength.
    pub ghosts: f32,
}

impl Camera {
    /// A helmet camera at L1: sunny-16 exposure for sunlit subjects, with the
    /// clamped automatic exposure of a small action camera, which opens up
    /// when the frame is mostly shadow.
    #[must_use]
    pub fn helmet() -> Self {
        Self {
            ev100: 15.0,
            auto_exposure: true,
            ev_min: 10.0,
            ev_max: 15.5,
            local_exposure: 0.6,
            bloom: 0.012,
            grain: 0.1,
            vignette: 0.3,
            // Lateral chromatic aberration fringes every specular glint at
            // these contrasts; it stays available but off.
            fringe: 0.0,
            white_balance: 5_800.0,
            star_gain: 1.0,
            ghosts: 0.1,
        }
    }

    /// A readable art preset: brighter shadows and visible stars.
    #[must_use]
    pub fn art() -> Self {
        Self {
            local_exposure: 0.8,
            grain: 0.0,
            star_gain: 40_000.0,
            ..Self::helmet()
        }
    }

    /// The multiplier from luminance to the display-referred signal.
    #[must_use]
    pub fn exposure(&self) -> f32 {
        exposure(self.ev100)
    }
}

/// Saturation-based sensor model with a lens attenuation of 0.78 (Lagarde and
/// de Rousiers 2014): exposure = 1 / (1.2 × 2^EV100).
#[must_use]
pub fn exposure(ev100: f32) -> f32 {
    verse_engine::lighting::exposure(ev100)
}

/// Irradiance probes for bounce light, as linear functions of the normal.
///
/// Each probe stores, per color channel, `e0 + e1 · n`: the order-one
/// spherical-harmonic irradiance of the light bounced by nearby surfaces, in
/// lux. The grid is axis aligned, `dims[0] × dims[1] × dims[2]` probes spaced
/// `cell` meters apart from `origin`.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeGrid {
    pub origin: Vec3,
    pub cell: f32,
    pub dims: [u32; 3],
    /// Per probe: red (e0, e1.x, e1.y, e1.z), then green, then blue.
    pub data: Vec<[f32; 12]>,
    /// Changes whenever the contents change, so the GPU copy can follow.
    pub version: u64,
}

/// A body drawn at infinity: its direction, angular radius, and body frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// Unit direction from the camera to the body center, scene axes.
    pub dir: Vec3,
    /// Angular radius in radians.
    pub angular_radius: f32,
    /// Distance in meters.
    pub distance: f64,
    /// Columns are the body's x, y, and z axes in scene coordinates. The z axis
    /// is the north pole; x points at longitude zero on the equator.
    pub axes: Mat3,
}

/// A neon stage: the amber plaza drawn through the physical path. Lines are
/// emissive in their palette colors, fog fades toward the field, and
/// a hue-preserving tone curve keeps every amber step on its own hue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neon {
    /// The near-black field: background, fog, and faces.
    pub field: [f32; 3],
    /// Distance where fog starts and where it is total, meters.
    pub fog_start: f32,
    pub fog_end: f32,
    /// Emission multiplier for lines, so their cores bloom.
    pub line_gain: f32,
    /// Line width in pixels.
    pub line_width: f32,
    /// Fraction of bloom energy added back.
    pub bloom: f32,
    /// Natural vignetting strength.
    pub vignette: f32,
    /// Seconds, for animated effects.
    pub time: f32,
    /// Studio light for physically lit geometry on the stage. Without it the
    /// stage draws no lit geometry.
    pub key: Option<Key>,
    /// A daytime sky drawn behind the stage instead of the flat field. Fog
    /// then fades toward the sky's color along each view ray, and with a key
    /// light the sky also lights the stage ([`environment`]). Without it the
    /// stage clears to its field.
    pub daylight: Option<Daylight>,
    /// Fog that thins with height and glows toward the Sun, in place of the
    /// linear ramp from `fog_start` to `fog_end`. The ramp's end stays the
    /// distance where fog is total. Without it the stage keeps the amber
    /// world's ramp.
    pub height_fog: Option<HeightFog>,
    /// Local point lights, such as candles and braziers, that light the
    /// stage's lit and textured geometry beside the key. They cast no
    /// shadows. Lamps with zero intensity are off.
    pub lamps: [Lamp; MAX_LAMPS],
    /// Transient impact and spell lights, selected before permanent lamps
    /// within the tier's shared lamp budget. They cast no shadows.
    pub flash_lamps: [Lamp; MAX_FLASH_CANDIDATES],
    /// Whether scene-lit particles take the scene's light. When disabled,
    /// they keep their legacy surface colors for capture comparisons.
    pub particle_lighting: bool,
    /// Whether particles fade into the opaque depth on Medium and High.
    pub soft_particles: bool,
    /// How brightly a textured scene's baked lamp layer burns, 0 for off to 1
    /// for as baked ([`baked_layers`]), such as from dusk to dawn.
    pub baked_lamps: f32,
    /// The output pass's grade.
    pub grade: Grade,
    /// The key and rim lights' linear colors, multiplied with their lux:
    /// white for a studio, a warm low Sun against a cool sky at dusk.
    pub key_color: [f32; 3],
    pub rim_color: [f32; 3],
    /// How brightly lightning lights the daylight sky this frame, 0 to 1.
    /// The sky pass alone draws it; the sky light keeps its bake.
    pub sky_flash: f32,
    /// Whether the sky changes only gradually, as a running town clock
    /// moves it. A gradual change rebakes the sky light over several frames
    /// ([`environment::SkyLightGpu`]), so the light trails the sky by a
    /// fraction of a second instead of the frame stalling; any other change
    /// rebakes at once.
    pub sky_gradual: bool,
    /// The water this frame: the swell, the ripples, and the water's
    /// color, drawn over the world mesh's [`water::WaterSurface`] on a lit
    /// stage, and the sea's tint on whatever lies under its level.
    pub water: Option<water::Water>,
}

/// The most permanent lamps a stage carries and total lamps its shader shades.
pub const MAX_LAMPS: usize = 32;

/// The most transient flash lights the high tier shades.
pub const MAX_FLASH_LIGHTS: usize = 8;

/// The most transient candidates a stage carries before camera-view selection.
pub const MAX_FLASH_CANDIDATES: usize = 32;

/// A point light on a neon stage, unshadowed.
///
/// Its light falls off with the inverse square of distance and is windowed
/// to zero at `range` (Karis 2013), so a lamp lights only what is near it.
/// The renderer scales its candela by the key's exposure, as it does the
/// key's lux, so lamps need a [`Key`] to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lamp {
    /// World position in meters.
    pub position: Vec3,
    /// Linear color, multiplied with `intensity`.
    pub color: [f32; 3],
    /// Luminous intensity, candela.
    pub intensity: f32,
    /// Distance where the light reaches zero, meters.
    pub range: f32,
}

impl Lamp {
    /// A lamp that gives no light.
    pub const OFF: Self = Self {
        position: Vec3::ZERO,
        color: [0.0; 3],
        intensity: 0.0,
        range: 1.0,
    };

    /// Whether the lamp gives light and every value is finite.
    #[must_use]
    pub fn lit(&self) -> bool {
        self.position.is_finite()
            && self.color.iter().all(|c| c.is_finite() && *c >= 0.0)
            && self.intensity.is_finite()
            && self.intensity > 0.0
            && self.range.is_finite()
            && self.range > 0.0
    }

    /// This lamp with a candle's flicker at `time` seconds: two slow
    /// swells and a faster gutter, out of phase for each `seed`, between
    /// about 0.75 and 1.1 of its intensity.
    #[must_use]
    pub fn flickering(self, time: f32, seed: u32) -> Self {
        let phase = seed as f32 * 1.618;
        let swell = (time * 2.3 + phase).sin() * 0.06 + (time * 3.7 + phase * 2.1).sin() * 0.05;
        let gutter = ((time * 11.0 + phase * 3.3).sin() * (time * 17.0 + phase).sin()).max(0.0);
        Self {
            intensity: self.intensity * (1.0 + swell - gutter * 0.18),
            ..self
        }
    }
}

/// A stylized daytime sky for a neon stage, in the stage's display-linear
/// colors (values the hue-preserving curve leaves alone stay below 0.76).
///
/// The model follows the structure of a physical sky without its cost: a
/// gradient from a hazy horizon to the zenith stands in for Rayleigh
/// scattering's optical depth, a forward lobe around the Sun stands in for
/// Mie scattering (Cornette and Shanks 1992), and fog takes the sky's color
/// along the view ray as a single-scattering aerial perspective, so distant
/// ground meets the horizon without a seam (Hillaire, "A Scalable and
/// Production Ready Sky and Atmosphere Rendering Technique", EGSR 2020).
/// Clouds are value-noise octaves on a plane overhead. The Sun sits in the
/// stage key's direction, so the disc agrees with the shadows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Daylight {
    /// Sky color straight up.
    pub zenith: [f32; 3],
    /// Haze color at the horizon, which is also the fog color.
    pub horizon: [f32; 3],
    /// The Sun's tint: its disc, its glow, and sunlit cloud tops.
    pub sun: [f32; 3],
    /// Cloud cover from 0 (clear) to 1 (overcast).
    pub clouds: f32,
    /// The ground's diffuse albedo. The sky light treats everything below
    /// the horizon as this ground, lit by the key and the sky.
    pub ground: [f32; 3],
    /// Dusk from 0 (day) to 1: a wider haze band, a broad glow and a band
    /// of fire on the horizon under a low Sun, violet cloud undersides with
    /// fiery edges, and crepuscular rays fanning from the Sun.
    pub glow: f32,
}

impl Daylight {
    /// Whether every color is a finite display color and the cover a
    /// fraction.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.zenith
            .iter()
            .chain(&self.horizon)
            .chain(&self.sun)
            .chain(&self.ground)
            .all(|c| c.is_finite() && (0.0..=1.0).contains(c))
            && (0.0..=1.0).contains(&self.clouds)
            && (0.0..=1.0).contains(&self.glow)
    }
}

/// Studio light for lit geometry on a neon stage: a shadowed key light, an
/// unshadowed rim light, and an ambient sky, each a small source in lux.
///
/// The stage's lines are display colors at unit exposure, so the renderer
/// scales these lux by [`exposure`]`(ev100)` before shading: a white surface
/// facing the key then reads near display white beside the lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    /// Unit direction toward the key light, which casts shadows.
    pub dir: Vec3,
    /// Key illuminance on a surface facing it, lux.
    pub illuminance: f32,
    /// The key's angular radius, radians: it sets penumbra and highlight size.
    pub angular_radius: f32,
    /// Unit direction toward the rim light, which casts no shadow.
    pub rim_dir: Vec3,
    pub rim_illuminance: f32,
    pub rim_angular_radius: f32,
    /// Ambient irradiance on surfaces facing straight up and straight down,
    /// lux. Under a [`Daylight`] sky the sky's own light replaces both: its
    /// colors, gradient, and lit ground set the ambient's shape, and `sky`
    /// only sets its level on surfaces facing up. The ambient light bake
    /// still reads both as the open sky's irradiance.
    pub sky: f32,
    pub ground: f32,
    /// Exposure value at ISO 100 that carries these lux onto the stage.
    pub ev100: f32,
    /// Center and half extent of the region that casts and receives shadows,
    /// when the shadow does not follow the camera.
    pub shadow_center: Vec3,
    pub shadow_half: f32,
    /// How far from the camera the key's shadow reaches, m. When set, the
    /// shadow follows the camera in cascades, as many as the quality tier
    /// draws, and `shadow_center` and `shadow_half` only center the ambient
    /// probes. Without it, one map covers the fixed region.
    pub shadow_distance: Option<f32>,
    /// Whether every caster the cascades after the first need is in the
    /// zone's world mesh, so those cascades can hold static casters only and
    /// be redrawn only when the camera crosses a cell. A zone that draws
    /// destructible or far-moving casters in its frame mesh turns it off,
    /// and every cascade then draws every caster each frame.
    pub cache_far_shadows: bool,
}

impl Key {
    /// Uniform ambient probes for this light, pre-exposed: the sky from
    /// above and the ground from below, as the linear function the lit
    /// shader reads. The version changes only when the light does.
    #[must_use]
    pub fn probes(&self) -> ProbeGrid {
        let exposure = exposure(self.ev100);
        let e0 = (self.sky + self.ground) * 0.5 * exposure;
        let e1 = (self.sky - self.ground) * 0.5 * exposure;
        let channel = [e0, 0.0, e1, 0.0];
        let mut probe = [0.0; 12];
        for c in 0..3 {
            probe[c * 4..c * 4 + 4].copy_from_slice(&channel);
        }
        // Tagged apart from baked grids, whose versions count up from one.
        let version = (1 << 63)
            ^ (u64::from(self.sky.to_bits()) << 32)
            ^ u64::from(self.ground.to_bits())
            ^ (u64::from(self.ev100.to_bits()) << 16);
        ProbeGrid {
            origin: self.shadow_center - Vec3::splat(500.0),
            cell: 1000.0,
            dims: [2, 2, 2],
            data: vec![probe; 8],
            version,
        }
    }
}

impl Neon {
    /// The plaza's stage, with the amber world's own field and fog.
    #[must_use]
    pub fn plaza(time: f32) -> Self {
        Self {
            field: verse_gfx::palette::field(),
            fog_start: crate::fog::FOG_START,
            fog_end: crate::fog::FOG_END,
            line_gain: 1.8,
            line_width: 1.6,
            bloom: 0.07,
            vignette: 0.2,
            time,
            key: None,
            daylight: None,
            height_fog: None,
            lamps: [Lamp::OFF; MAX_LAMPS],
            flash_lamps: [Lamp::OFF; MAX_FLASH_CANDIDATES],
            particle_lighting: true,
            soft_particles: true,
            baked_lamps: 0.0,
            grade: Grade::STAGE,
            key_color: [1.0; 3],
            rim_color: [1.0; 3],
            sky_flash: 0.0,
            sky_gradual: false,
            water: None,
        }
    }

    /// The bare world's stage: the plaza's, over the neutral field, with the
    /// bare world's nearer fog, so the distant grid fades toward the horizon.
    #[must_use]
    pub fn neutral(time: f32) -> Self {
        Self {
            field: verse_gfx::palette::neutral(verse_gfx::palette::field()),
            fog_start: crate::fog::BARE_FOG_START,
            fog_end: crate::fog::BARE_FOG_END,
            ..Self::plaza(time)
        }
    }
}

/// Everything the physical renderer needs about light and sky for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Sky {
    /// Unit direction toward the Sun, scene axes.
    pub sun_dir: Vec3,
    /// Solar illuminance at the station, lux.
    pub sun_illuminance: f32,
    /// Apparent angular radius of the Sun, radians.
    pub sun_angular_radius: f32,
    /// How much of the Sun is unobstructed from the camera, 0 to 1.
    pub sun_visible: f32,
    pub earth: Body,
    pub moon: Body,
    /// Columns map equatorial J2000 unit vectors into scene axes.
    pub celestial: Mat3,
    /// Center and half extent of the region that casts and receives shadows.
    pub shadow_center: Vec3,
    pub shadow_half: f32,
    pub camera: Camera,
    pub probes: Option<Arc<ProbeGrid>>,
    /// Seconds, for grain and other animated effects.
    pub time: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lamp gives light only when it is finite, bright, and in range, and
    /// its flicker stays a gentle swell around its intensity.
    #[test]
    fn lamps_light_only_when_valid_and_flicker_within_bounds() {
        assert!(!Lamp::OFF.lit());
        assert!(Neon::plaza(0.0).lamps.iter().all(|lamp| !lamp.lit()));
        let candle = Lamp {
            position: Vec3::new(1.0, 1.0, 0.0),
            color: [1.0, 0.6, 0.3],
            intensity: 4.0,
            range: 5.0,
        };
        assert!(candle.lit());
        assert!(
            !Lamp {
                range: 0.0,
                ..candle
            }
            .lit()
        );
        assert!(
            !Lamp {
                intensity: f32::NAN,
                ..candle
            }
            .lit()
        );
        assert!(
            !Lamp {
                color: [-1.0, 0.0, 0.0],
                ..candle
            }
            .lit()
        );
        let mut seen = (f32::INFINITY, 0.0f32);
        for step in 0..2_000 {
            let lit = candle.flickering(step as f32 * 0.013, 3);
            assert_eq!(lit.position, candle.position);
            seen = (seen.0.min(lit.intensity), seen.1.max(lit.intensity));
        }
        assert!(
            seen.0 >= candle.intensity * 0.7 && seen.1 <= candle.intensity * 1.15,
            "{seen:?}"
        );
        assert!(
            seen.1 - seen.0 > candle.intensity * 0.1,
            "the flame barely moves: {seen:?}"
        );
        // Two lamps flicker out of step.
        assert_ne!(
            candle.flickering(1.0, 0).intensity,
            candle.flickering(1.0, 1).intensity
        );
    }

    /// Only zones that opt in draw a daylight sky; the plaza and the bare
    /// world keep their flat field.
    #[test]
    fn stages_draw_daylight_only_when_asked() {
        assert!(Neon::plaza(0.0).daylight.is_none());
        assert!(Neon::neutral(0.0).daylight.is_none());
        // The amber stages keep their linear fog ramp.
        assert!(Neon::plaza(0.0).height_fog.is_none());
        assert!(Neon::neutral(0.0).height_fog.is_none());
        let day = Daylight {
            zenith: [0.1, 0.3, 0.7],
            horizon: [0.7, 0.65, 0.5],
            sun: [1.0, 0.9, 0.6],
            clouds: 0.4,
            ground: [0.1, 0.11, 0.07],
            glow: 0.0,
        };
        assert!(day.valid());
        assert!(Daylight { glow: 1.0, ..day }.valid());
        assert!(!Daylight { glow: 1.5, ..day }.valid());
        assert!(!Daylight { clouds: 1.5, ..day }.valid());
        assert!(
            !Daylight {
                zenith: [f32::NAN, 0.0, 0.0],
                ..day
            }
            .valid()
        );
    }
}
