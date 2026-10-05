//! Capability and resource admission before chamber allocation.
use verse_engine::quality::{Platform, Probe, Quality, Tier};

#[derive(Clone, Copy)]
pub(super) struct Admission {
    pub quality: Quality,
    pub samples: u32,
    pub scene: wgpu::TextureFormat,
}
impl Admission {
    pub fn probe(adapter: &wgpu::Adapter) -> Result<Self, String> {
        let float = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba16Float);
        let floating = float.allowed_usages.contains(
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        ) && float.flags.contains(
            wgpu::TextureFormatFeatureFlags::FILTERABLE
                | wgpu::TextureFormatFeatureFlags::BLENDABLE,
        );
        let scene = if floating {
            wgpu::TextureFormat::Rgba16Float
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let color = adapter.get_texture_format_features(scene);
        let depth = adapter.get_texture_format_features(wgpu::TextureFormat::Depth32Float);
        let samples =
            if color.flags.sample_count_supported(4) && depth.flags.sample_count_supported(4) {
                4
            } else {
                1
            };
        let capabilities = adapter.get_downlevel_capabilities();
        if !capabilities
            .flags
            .contains(wgpu::DownlevelFlags::CUBE_ARRAY_TEXTURES)
            || adapter.limits().max_texture_array_layers < 24
        {
            return Err(
                "Chamber requires cube-array shadow textures with 24 supported layers".into(),
            );
        }
        let probe = Probe {
            platform: Platform::current(),
            gles: crate::gles::is_gles(adapter.get_info().backend),
            float_target: floating,
            compute: capabilities
                .flags
                .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
                && adapter.limits().max_storage_buffers_per_shader_stage >= 2,
            samples,
        };
        let quality = probe
            .select(
                std::env::var("VERSE_QUALITY")
                    .ok()
                    .and_then(|v| Tier::parse(&v)),
            )
            .quality();
        Ok(Self {
            quality,
            samples: samples.min(quality.sample_ceiling()),
            scene,
        })
    }
    /// Declared texel payload including both shadow arrays, bloom, LUT, and capture staging.
    /// This excludes driver padding, retained assets, uniforms, and asynchronous capture copies.
    pub fn target_bytes(self, width: u32, height: u32) -> u64 {
        let bytes = if self.scene == wgpu::TextureFormat::Rgba16Float {
            8
        } else {
            4
        };
        let pixels = u64::from(width) * u64::from(height);
        // The single-sample scene and display, world multisample color/depth, and readback.
        let mut total = pixels * (bytes + 4 + u64::from(self.samples) * (bytes + 4));
        total += u64::from((width * 4).div_ceil(256) * 256) * u64::from(height);
        let mut w = (width / 2).max(1);
        let mut h = (height / 2).max(1);
        let levels = super::super::pbr::output::BLOOM_LEVELS
            .min(32 - w.min(h).leading_zeros())
            .max(1);
        for _ in 0..levels {
            total += u64::from(w) * u64::from(h) * bytes;
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        total += 2 * bytes + (verse_engine::lighting::GRADE_LUT_SIZE as u64).pow(3) * 8;
        total
            + u64::from(self.quality.local_shadow_size()).pow(2)
                * u64::from(self.quality.local_shadow_views())
                * 4
                * 2
    }
}
