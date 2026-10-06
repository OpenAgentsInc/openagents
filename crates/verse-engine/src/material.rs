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
