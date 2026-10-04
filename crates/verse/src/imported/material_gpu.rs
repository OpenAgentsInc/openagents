//! Per-surface material identity and uniform packing for the owned renderer.
use verse_engine::assets::Surface;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct Key {
    pub texture: usize,
    pub blend: u8,
    pub emissive: bool,
    pub maps: [Option<usize>; 4],
    values: [u32; 9],
}
impl Key {
    pub fn from_surface(surface: &Surface) -> Self {
        let m = &surface.material;
        Self {
            texture: surface.texture,
            blend: surface.blend,
            emissive: surface.emissive,
            maps: [
                m.normal_texture,
                m.metallic_roughness_texture,
                m.occlusion_texture,
                m.emissive_texture,
            ],
            values: [
                m.roughness,
                m.metallic,
                m.normal_scale,
                m.occlusion_strength,
                m.opacity,
                m.alpha_cutoff,
                m.emissive_factor[0],
                m.emissive_factor[1],
                m.emissive_factor[2],
            ]
            .map(f32::to_bits),
        }
    }
    /// Four aligned vec4 values, shared with the WGSL Material declaration.
    pub fn uniform(self) -> [[f32; 4]; 4] {
        let v = self.values.map(f32::from_bits);
        [
            [
                u8::from(self.emissive) as f32,
                self.blend as f32,
                v[0],
                v[1],
            ],
            [v[2], v[3], v[4], v[5]],
            [v[6], v[7], v[8], 0.],
            self.maps.map(|slot| u8::from(slot.is_some()) as f32),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn surface() -> Surface {
        Surface {
            vertices: vec![],
            indices: vec![],
            texture: 0,
            blend: 0,
            emissive: false,
            tint: [1.; 3],
            material: Default::default(),
        }
    }
    #[test]
    fn shared_base_color_does_not_merge_distinct_materials() {
        let original = surface();
        let key = Key::from_surface(&original);
        for channel in 0..5 {
            let mut changed = original.clone();
            match channel {
                0 => changed.material.roughness = 0.25,
                1 => changed.material.normal_texture = Some(1),
                2 => changed.material.metallic_roughness_texture = Some(1),
                3 => changed.material.occlusion_texture = Some(1),
                _ => changed.material.emissive_texture = Some(1),
            }
            assert_ne!(Key::from_surface(&changed), key);
        }
        let mut tinted = original.clone();
        tinted.tint = [0.5; 3];
        assert_eq!(Key::from_surface(&tinted), key);
    }
    #[test]
    fn packing_preserves_factors_and_presence_flags() {
        let mut surface = surface();
        surface.material.normal_texture = Some(1);
        surface.material.roughness = 0.25;
        surface.material.metallic = 0.75;
        surface.material.opacity = 0.5;
        let uniform = Key::from_surface(&surface).uniform();
        assert_eq!(std::mem::size_of_val(&uniform), 64);
        assert_eq!(uniform[0], [0., 0., 0.25, 0.75]);
        assert_eq!(uniform[1], [1., 1., 0.5, 0.5]);
        assert_eq!(uniform[3], [1., 0., 0., 0.]);
    }
}
