//! Authored metallic/roughness materials, independent of GPU resource layouts.
use serde::{Deserialize, Serialize};

/// Effective alpha after texture, vertex, and authored opacity multiplication.
/// Opaque and retained masked fragments are fully covered; blended and additive
/// fragments have bounded coverage. Missing coverage means a discarded fragment.
#[must_use]
pub fn coverage(alpha: f32, blend: u8, cutoff: f32) -> Option<f32> {
    if !alpha.is_finite() {
        return None;
    }
    match blend {
        0 => Some(1.0),
        1 if alpha >= cutoff => Some(1.0),
        2 | 3 if alpha > 0.0 => Some(alpha.clamp(0.0, 1.0)),
        _ => None,
    }
}

/// Base color and emissive images use sRGB RGB with linear alpha. Normal,
/// occlusion, and metallic/roughness images use linear channels. Metallic is
/// sampled from B, roughness from G, and occlusion from R.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Material {
    pub roughness: f32,
    pub metallic: f32,
    pub normal_texture: Option<usize>,
    pub normal_scale: f32,
    pub metallic_roughness_texture: Option<usize>,
    pub occlusion_texture: Option<usize>,
    pub occlusion_strength: f32,
    pub emissive_texture: Option<usize>,
    pub emissive_factor: [f32; 3],
    pub opacity: f32,
    pub alpha_cutoff: f32,
    /// The shading model ([`Shading`]); standard metallic/roughness when 0.
    /// Omitted from serialized packs when standard, so existing packs and
    /// their digests are unchanged.
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub shading: u8,
    /// The shading model's parameters (see [`Shading`]); zero for standard.
    #[serde(skip_serializing_if = "is_zero4")]
    pub shading_params: [f32; 4],
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}
fn is_zero4(v: &[f32; 4]) -> bool {
    v.iter().all(|x| *x == 0.0)
}

/// Shading models beyond standard metallic/roughness, after the layered
/// character materials of film-grade digital humans
/// (docs/verse/draconia/metahuman-study.md). Parameters per model, in
/// [`Material::shading_params`] order:
///
/// - `Skin`: subsurface strength, specular lobe mix, specular weight,
///   unused. A
///   wrapped, colour-shifted diffuse (light bleeds red past the
///   terminator), a softened normal for the red channel, and two GGX
///   lobes at 0.75 and 1.3 times the roughness.
/// - `Eye`: iris depth in metres below the eyeball's surface at the pupil,
///   the iris radius in UV units, unused, unused. The iris is seen through
///   a refractive cornea, so its texture is offset by the refracted view
///   ray's travel to the iris plane.
/// - `Cornea`: specular only (drawn additively over the eye), a tear-film
///   gloss.
/// - `Hair`: primary highlight shift, secondary shift (radians, along the
///   strand), transmission strength, unused. Kajiya-Kay style strand
///   lighting with two shifted lobes (Marschner R and TRT) along the
///   strand direction, which is the surface's +v (root to tip), and light
///   transmitted through backlit hair (TT). Cutout coverage is stochastic,
///   so averaged sub-frames give soft strand edges.
/// - `Cloth`: sheen strength, sheen roughness, unused, unused. A Charlie
///   sheen lobe over a softly wrapped diffuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Shading {
    Standard = 0,
    Skin = 1,
    Eye = 2,
    Cornea = 3,
    Hair = 4,
    Cloth = 5,
}
impl Shading {
    /// The model named in authored assets (glTF material extras'
    /// `verse_shading`), an exact enum value.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "standard" => Self::Standard,
            "skin" => Self::Skin,
            "eye" => Self::Eye,
            "cornea" => Self::Cornea,
            "hair" => Self::Hair,
            "cloth" => Self::Cloth,
            _ => return None,
        })
    }
    /// Default parameters for each model.
    #[must_use]
    pub fn defaults(self) -> [f32; 4] {
        match self {
            Self::Standard | Self::Cornea => [0.0; 4],
            Self::Skin => [1.0, 0.85, 0.5, 0.0],
            Self::Eye => [0.0016, 0.0826, 0.0, 0.0],
            Self::Hair => [0.06, -0.10, 0.6, 0.0],
            Self::Cloth => [0.5, 0.5, 0.0, 0.0],
        }
    }
}
impl Default for Material {
    fn default() -> Self {
        Self {
            roughness: 1.,
            metallic: 0.,
            normal_texture: None,
            normal_scale: 1.,
            metallic_roughness_texture: None,
            occlusion_texture: None,
            occlusion_strength: 1.,
            emissive_texture: None,
            emissive_factor: [0.; 3],
            opacity: 1.,
            alpha_cutoff: 0.5,
            shading: 0,
            shading_params: [0.; 4],
        }
    }
}
impl Material {
    /// Texture slots remain logical pack references, never filesystem paths.
    pub fn textures(&self) -> impl Iterator<Item = usize> + '_ {
        [
            self.normal_texture,
            self.metallic_roughness_texture,
            self.occlusion_texture,
            self.emissive_texture,
        ]
        .into_iter()
        .flatten()
    }
    pub fn validate(&self, texture_count: usize) -> Result<(), String> {
        if [
            self.roughness,
            self.metallic,
            self.occlusion_strength,
            self.opacity,
            self.alpha_cutoff,
        ]
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || !self.normal_scale.is_finite()
            || !(0.0..=16.0).contains(&self.normal_scale)
            || self
                .emissive_factor
                .iter()
                .any(|v| !v.is_finite() || *v < 0. || *v > 65504.)
            || self.textures().any(|slot| slot >= texture_count)
            || self.shading > Shading::Cloth as u8
            || self
                .shading_params
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 64.)
        {
            return Err("Invalid authored material channels or texture references".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_materials_have_explicit_defaults_and_new_channels_round_trip() {
        let mut material: Material = serde_json::from_str("{}").unwrap();
        assert_eq!(material, Material::default());
        material.normal_texture = Some(0);
        material.metallic_roughness_texture = Some(1);
        material.occlusion_texture = Some(1);
        material.emissive_texture = Some(2);
        material.emissive_factor = [2., 0.5, 0.];
        assert!(material.validate(3).is_ok());
        assert!(material.validate(2).is_err());
        assert_eq!(material.textures().collect::<Vec<_>>(), [0, 1, 1, 2]);
        let json = serde_json::to_string(&material).unwrap();
        assert_eq!(serde_json::from_str::<Material>(&json).unwrap(), material);
        assert!(serde_json::from_str::<Material>(r#"{"rougness":0.5}"#).is_err());
    }
    #[test]
    fn standard_materials_serialize_without_shading_and_models_round_trip() {
        let standard = serde_json::to_string(&Material::default()).unwrap();
        assert!(!standard.contains("shading"));
        let skin = Material {
            shading: Shading::Skin as u8,
            shading_params: Shading::Skin.defaults(),
            ..Default::default()
        };
        assert!(skin.validate(0).is_ok());
        let json = serde_json::to_string(&skin).unwrap();
        assert_eq!(serde_json::from_str::<Material>(&json).unwrap(), skin);
        assert_eq!(Shading::from_name("hair"), Some(Shading::Hair));
        assert_eq!(Shading::from_name("Hair"), None);
        let mut bad = skin.clone();
        bad.shading = 6;
        assert!(bad.validate(0).is_err());
        bad.shading = 1;
        bad.shading_params[0] = f32::NAN;
        assert!(bad.validate(0).is_err());
    }
    #[test]
    fn rejects_nonfinite_out_of_range_and_invalid_dependencies() {
        for value in [-0.01, 1.01, f32::NAN, f32::INFINITY] {
            for field in 0..5 {
                let mut material = Material::default();
                match field {
                    0 => material.roughness = value,
                    1 => material.metallic = value,
                    2 => material.occlusion_strength = value,
                    3 => material.opacity = value,
                    _ => material.alpha_cutoff = value,
                }
                assert!(material.validate(0).is_err());
            }
        }
        let mut material = Material::default();
        material.normal_texture = Some(0);
        assert!(material.validate(0).is_err());
        material.normal_texture = None;
        material.normal_scale = f32::INFINITY;
        assert!(material.validate(0).is_err());
        material.normal_scale = 1.;
        material.emissive_factor[0] = -1.;
        assert!(material.validate(0).is_err());
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    #[test]
    fn mask_survivors_are_opaque_and_transparency_is_bounded() {
        assert_eq!(coverage(0.0, 0, 0.5), Some(1.0));
        assert_eq!(coverage(0.5, 1, 0.5), Some(1.0));
        assert_eq!(coverage(0.49, 1, 0.5), None);
        assert_eq!(coverage(2.0, 2, 0.0), Some(1.0));
        assert_eq!(coverage(0.25, 3, 0.0), Some(0.25));
        assert_eq!(coverage(f32::NAN, 2, 0.0), None);
    }
}
