//! Capability and resource admission before chamber allocation.
use verse_engine::quality::{Platform, Probe, Quality, Tier};

#[derive(Clone, Copy)]
pub(super) struct Admission {
    pub quality: Quality,
    pub samples: u32,
    pub scene: wgpu::TextureFormat,
    /// The adapter compiles shaders to GLSL ES (GLES 3.0 or WebGL2).
    pub gles: bool,
    /// Bones in one draw's uniform pose block: 256, or fewer where the
    /// largest uniform binding cannot hold 256 (16 KiB on GLES and WebGL2).
    pub pose_bones: u32,
}

/// Bytes of a pose block before its bones: the model matrix and parameters.
pub(super) const POSE_HEADER_BYTES: u32 = 64 + 16;
/// Bytes of one bone matrix.
const BONE_BYTES: u32 = 64;
/// The bones a pose block holds on a WebGPU-class device.
pub(super) const MAX_POSE_BONES: u32 = 256;
/// The uniform block size GLES 3.0 and WebGL2 guarantee
/// (`GL_MAX_UNIFORM_BLOCK_SIZE`).
pub(super) const GLES_UNIFORM_BLOCK: u64 = 16_384;

/// The bones a uniform pose block holds under `max_uniform_binding` bytes.
pub(super) fn pose_bones(max_uniform_binding: u64) -> u32 {
    let room =
        max_uniform_binding.saturating_sub(u64::from(POSE_HEADER_BYTES)) / u64::from(BONE_BYTES);
    room.min(u64::from(MAX_POSE_BONES)) as u32
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
        let gles = crate::gles::is_gles(adapter.get_info().backend);
        let probe = Probe {
            platform: Platform::current(),
            gles,
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
        let limits = adapter.limits();
        let admission = Self {
            quality,
            samples: samples.min(quality.sample_ceiling()),
            scene,
            gles,
            // GLES and WebGL2 guarantee a 16 KiB uniform block; every GLSL
            // ES device gets that layout, whatever larger limit it reports,
            // so one tested pose block serves them all.
            pose_bones: pose_bones(if gles {
                limits
                    .max_uniform_buffer_binding_size
                    .min(GLES_UNIFORM_BLOCK)
            } else {
                limits.max_uniform_buffer_binding_size
            }),
        };
        admission.check(&limits)?;
        Ok(admission)
    }
    /// Refuses limits this admission's layout does not fit.
    fn check(self, limits: &wgpu::Limits) -> Result<(), String> {
        if self.pose_bones == 0 {
            return Err(format!(
                "The pose block needs a uniform binding over {POSE_HEADER_BYTES} bytes; the \
                 adapter allows {}",
                limits.max_uniform_buffer_binding_size
            ));
        }
        // Local shadows sample one 2D depth array; nothing makes a cube or
        // cube-array view, so cube-array support is not required.
        if limits.max_texture_array_layers < self.shadow_texture_layers() {
            return Err(format!(
                "Local shadows need {} texture array layers; the adapter allows {}",
                self.shadow_texture_layers(),
                limits.max_texture_array_layers
            ));
        }
        Ok(())
    }
    /// Whether local lights cast shadows. GLES and WebGL2 cannot copy a depth
    /// texture, which the static shadow cache needs, so there local lights
    /// light without shadows.
    pub fn local_shadows(self) -> bool {
        !self.gles
    }
    /// Layers of the local shadow textures. wgpu's GLES backend makes a
    /// square texture whose layer count is a multiple of six a cube map or a
    /// cube-map array, which a `D2Array` view and a layered depth attachment
    /// cannot use, so on GLES one unused layer keeps it a 2D array.
    pub fn shadow_texture_layers(self) -> u32 {
        let views = self.quality.local_shadow_views();
        if self.gles && views.is_multiple_of(6) {
            views + 1
        } else {
            views
        }
    }
    /// Bytes of one draw's uniform pose block.
    pub fn pose_bytes(self) -> u64 {
        u64::from(POSE_HEADER_BYTES + self.pose_bones * BONE_BYTES)
    }
    /// The scene shader with its pose block cut to [`Self::pose_bones`].
    pub fn scene_shader(self, source: &str) -> String {
        scene_shader(source, self.pose_bones)
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
        // Static 32³ R8 turbulence and 512-entry RGBA16F spectral lookup.
        total += 32 * 32 * 32 + 512 * 8;
        total
            + u64::from(self.quality.local_shadow_size()).pow(2)
                * u64::from(self.shadow_texture_layers())
                * 4
                * 2
    }
}

/// The pose block's bone array as `scene.wgsl` declares it.
const POSE_BONES_DECLARATION: &str = "bones:array<mat4x4<f32>,256>";

/// `source` (the scene shader) with a pose block of `bones` bones.
pub(super) fn scene_shader(source: &str, bones: u32) -> String {
    assert!(
        source.contains(POSE_BONES_DECLARATION),
        "the scene shader declares {POSE_BONES_DECLARATION}"
    );
    if bones == MAX_POSE_BONES {
        return source.to_owned();
    }
    source.replace(
        POSE_BONES_DECLARATION,
        &format!("bones:array<mat4x4<f32>,{bones}>"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLES_UNIFORM_BINDING: u64 = GLES_UNIFORM_BLOCK;

    #[test]
    fn the_pose_block_fits_the_gles_uniform_limit() {
        assert_eq!(pose_bones(65_536), 256);
        let bones = pose_bones(GLES_UNIFORM_BINDING);
        assert_eq!(bones, 254);
        assert!(u64::from(POSE_HEADER_BYTES + bones * BONE_BYTES) <= GLES_UNIFORM_BINDING);
        assert_eq!(pose_bones(64), 0);
    }

    #[test]
    fn the_cut_scene_shader_declares_a_block_within_the_limit() {
        let source = &crate::shading::source(include_str!("scene.wgsl"));
        assert_eq!(scene_shader(source, 256), source.as_str());
        let text = scene_shader(source, 254);
        assert!(text.contains("bones:array<mat4x4<f32>,254>"));
        let module = naga::front::wgsl::parse_str(&text).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).unwrap();
        let pose = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some("Pose"))
            .map(|(handle, _)| handle)
            .unwrap();
        assert_eq!(u64::from(layouter[pose].size), 80 + 254 * 64);
        assert!(u64::from(layouter[pose].size) <= GLES_UNIFORM_BINDING);
    }
}
