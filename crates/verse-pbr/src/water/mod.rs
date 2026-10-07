//! Water for both Verse renderers (`docs/verse/water.md`, phase W2): one
//! shared shader, `water.wgsl`, spliced by [`crate::shading::source`] into
//! the physical renderer's pass (`photo.wgsl` here, after `pbr/photo.wgsl`)
//! and the imported renderer's (`imported.wgsl` here, after
//! `imported/scene.wgsl`), and the data both draw from.
//!
//! - [`frame`]: a frame's [`Water`] (bodies, clock, detail, ripples, and
//!   spells) and the [`WaterSurface`] meshes it moves.
//! - [`terms`]: the Gerstner terms of a `physics::water::WaveSet` as the
//!   shader reads them, with an `f32` mirror of its displacement.
//! - [`bake`]: a mesh per `physics::water::WaterBody`, spaced by tier, with
//!   baked depth, shore distance, and current.
//! - [`preset`]: looks from `assets/verse/water/presets/`, with absorption
//!   from Jerlov's water types.
//! - [`tile`]: the baked looping normal tile the low tier reads.
//! - [`ocean`]: the spectral sea's cascades (phase W4), synthesized on a
//!   worker thread and uploaded as the `water_waves` array texture.
//! - [`seas`]: sea states from `assets/verse/water/seas/`.
//! - [`control`]: the frame's `water_control` vector for a quality tier.
//!
//! Each pass draws a surface in two halves inside the scene pass, with no
//! extra pass or render target on any tier: `fs_water_transmit` multiplies
//! what lies behind by the surface's per-channel transmittance, then
//! `fs_water` adds what it reflects, scatters, and foams. The physical
//! renderer's pass also lights the Water Lab's sea bed and draws its spells,
//! falls, and orbs, which the imported renderer does not.

pub mod bake;
pub mod frame;
pub mod ocean;
#[cfg(test)]
mod parity;
pub mod preset;
pub mod seas;
pub mod terms;
pub mod tile;

pub use frame::{
    Body, Controls, Kind, Sky, Water, WaterPatch, WaterSurface, WaterUniform, WaterVertex,
};
pub use ocean::OceanGpu;
pub use preset::{Jerlov, Preset};
pub use seas::SeaState;
pub use terms::Swell;

use verse_engine::quality::Tier;

/// The most bodies a frame's water carries.
pub const MAX_BODIES: usize = 8;

/// The shared water shader.
pub const SHARED: &str = include_str!("water.wgsl");
/// The physical renderer's water pass, after [`SHARED`].
pub const PHOTO: &str = include_str!("photo.wgsl");
/// The imported renderer's water pass, after [`SHARED`].
pub const IMPORTED: &str = include_str!("imported.wgsl");

/// What a tier's water draws, as the frame uniform's `water_control`:
///
/// - x: analytic detail waves, 0 for the baked tile (Low), 10 (Medium), or
///   16 (High);
/// - y: the distance over which sun glints fade, m;
/// - z: octaves of foam noise;
/// - w: 1, water drawn.
///
/// Every tier draws the same surface, Fresnel, sky, glints, crest
/// scattering, absorption, and foam; Low draws them with no extra pass and
/// no floating-point target.
#[must_use]
pub fn control(tier: Tier) -> [f32; 4] {
    match tier {
        Tier::Low => [0.0, 120.0, 1.0, 1.0],
        Tier::Medium => [10.0, 250.0, 2.0, 1.0],
        Tier::High => [16.0, 400.0, 2.0, 1.0],
    }
}

/// The angle one pixel spans at the eye, rad, at the middle of a view
/// `height` pixels tall: what the water's footprint grows by per meter
/// (`look.y` of the uniform, which each renderer sets).
#[must_use]
pub fn pixel_angle(view_proj: glam::Mat4, eye: glam::Vec3, height: u32) -> f32 {
    let inverse = view_proj.inverse();
    let at =
        |y: f32| (inverse.project_point3(glam::Vec3::new(0.0, y, 0.5)) - eye).normalize_or_zero();
    let step = 2.0 / height.max(1) as f32;
    let angle = at(-0.5 * step).angle_between(at(0.5 * step));
    if angle.is_finite() { angle } else { 0.0 }
}

/// The blend of the transmitted half: the scene times the source color.
pub const TRANSMIT_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::Src,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

/// The blend of the emitted half: added to the scene.
pub const EMIT_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Zero,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

/// The uniform buffer's layout entry, at `binding`, for both stages.
#[must_use]
pub fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// The tile's and its sampler's layout entries at `binding` and the next.
/// The vertex stage samples the spectral cascades with the same sampler.
#[must_use]
pub fn tile_entries(binding: u32) -> [wgpu::BindGroupLayoutEntry; 2] {
    [
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: binding + 1,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ]
}

/// A zone's water surface uploaded once, with each patch's index range,
/// for either renderer.
pub struct SurfaceGpu {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub ranges: Vec<std::ops::Range<u32>>,
}

impl SurfaceGpu {
    /// Uploads `surface`, drawing every other row and column of the
    /// patches that allow it on Low.
    #[must_use]
    pub fn upload(device: &wgpu::Device, surface: &WaterSurface, tier: Tier) -> Self {
        use wgpu::util::DeviceExt;
        let stride = if tier == Tier::Low { 2 } else { 1 };
        let vertices = surface.vertices();
        let (indices, ranges) = surface.ranges(stride);
        let make = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        Self {
            vertices: make(
                "verse water vertices",
                bytemuck::cast_slice(&vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            // An empty buffer still needs a size; one index draws nothing.
            indices: make(
                "verse water indices",
                bytemuck::cast_slice(if indices.is_empty() {
                    &[0u32]
                } else {
                    &indices
                }),
                wgpu::BufferUsages::INDEX,
            ),
            ranges,
        }
    }

    /// Triangles' indices in all.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.ranges.iter().map(|r| r.end - r.start).sum()
    }

    /// The bytes the surface holds on the GPU.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.vertices.size() + self.indices.size()
    }

    /// Draws each patch through both halves of a water pass: `transmit`
    /// then `emit`, patch by patch, so nearer water lies over farther water
    /// in the order the zone lists it.
    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        transmit: &'a wgpu::RenderPipeline,
        emit: &'a wgpu::RenderPipeline,
    ) {
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        for range in &self.ranges {
            if range.is_empty() {
                continue;
            }
            pass.set_pipeline(transmit);
            pass.draw_indexed(range.clone(), 0, 0..1);
            pass.set_pipeline(emit);
            pass.draw_indexed(range.clone(), 0, 0..1);
        }
    }
}
