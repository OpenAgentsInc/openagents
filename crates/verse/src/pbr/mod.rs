//! Physically lit geometry and a physical sky, for zones drawn in real units.
//!
//! The amber zones draw flat faces and lines in display colors. A zone that
//! wants photographic light instead emits [`LitVertex`] triangles, additive
//! [`GlowVertex`] quads, and one [`Sky`] per frame. When a frame carries a
//! [`Sky`], the renderer switches to the path in `gpu.rs`: a sun shadow map,
//! a floating-point scene in pre-exposed luminance, the Sun, Earth, Moon, and
//! catalogue stars drawn at infinity, bloom, exposure, and tone mapping.
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

pub mod bake;
pub(crate) mod gpu;
pub mod sky;

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Vec3};

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
            Self::Radiator => 6.0,
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
            Self::SolarCell => ([0.03, 0.04, 0.10], 0.0, 0.35),
            Self::Visor => ([1.0, 0.77, 0.34], 1.0, 0.05),
            Self::Fabric => ([0.75, 0.75, 0.73], 0.0, 0.9),
            Self::Radiator => ([0.97, 0.96, 0.92], 1.0, 0.12),
            Self::SafetyPaint => ([0.80, 0.50, 0.04], 0.0, 0.6),
            Self::Dark => ([0.06, 0.06, 0.07], 0.0, 0.55),
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
    1.0 / (1.2 * 2f32.powf(ev100))
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
