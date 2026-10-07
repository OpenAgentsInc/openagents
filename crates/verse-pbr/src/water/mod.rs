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
//! - [`screen`]: what Medium and High copy and trace (W5): the scene's
//!   color and depth copies, refraction, the planar mirror, and
//!   screen-space reflection, with `screen.wgsl` as its shader half.
//! - [`clipmap`]: the ocean's geometry clipmap around the eye (W10).
//! - [`field`]: a zone's baked depth, shore distance, and current, whose
//!   pages stream into an atlas under `verse_engine::streaming` (W10).
//!
//! On Low each pass draws a surface in two halves inside the scene pass,
//! with no extra pass or render target: `fs_water_transmit` multiplies
//! what lies behind by the surface's per-channel transmittance, then
//! `fs_water` adds what it reflects, scatters, and foams. On Medium and
//! High the physical renderer draws its zone water once with
//! `fs_water_screen`, which reads what lies behind from the scene copy
//! instead ([`screen`]). The physical
//! renderer's pass also lights the Water Lab's sea bed and draws its spells,
//! falls, and orbs, which the imported renderer does not.

pub mod bake;
pub mod clipmap;
pub mod field;
pub mod frame;
pub mod ocean;
#[cfg(test)]
mod parity;
pub mod preset;
pub mod screen;
pub mod seas;
pub mod terms;
pub mod tile;

pub use frame::{
    Body, Controls, Kind, Ocean, Sky, Water, WaterPatch, WaterSurface, WaterUniform, WaterVertex,
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
    /// Where each body's surface lies at rest (seas, ponds, and streams;
    /// not falls or orbs), for choosing the body a planar mirror reflects.
    pub bounds: [Option<screen::Bounds>; MAX_BODIES],
    /// The ocean's clipmap, after the patches in both buffers.
    pub ocean: Option<OceanMesh>,
}

/// An ocean's clipmap on the GPU ([`clipmap`]) and its field's pages
/// streaming in ([`field::Stream`]).
pub struct OceanMesh {
    pub spec: clipmap::Spec,
    /// The clipmap's index ranges, offset into [`SurfaceGpu::indices`].
    pub mesh: clipmap::Mesh,
    pub sea: bool,
    /// Kept behind a lock so a frame that only borrows the surface can
    /// still stream.
    pub stream: Option<std::sync::Mutex<field::Stream>>,
}

/// The uniform rows an ocean fills each frame: the clipmap's, the field's
/// shape, and the field's slot table.
pub type OceanRows = (
    [[f32; 4]; clipmap::ROWS],
    [[f32; 4]; field::ROWS],
    [[f32; 4]; field::SLOT_ROWS],
);

impl OceanMesh {
    /// Streams the field's pages around `eye` into `atlas` and returns the
    /// uniform's rows.
    pub fn prepare(&self, queue: &wgpu::Queue, atlas: &field::Atlas, eye: glam::Vec3) -> OceanRows {
        let at = glam::Vec2::new(eye.x, eye.z);
        let clip = clipmap::rows(&self.spec, at, self.sea);
        let none = (clip, [[0.0; 4]; field::ROWS], [[-1.0; 4]; field::SLOT_ROWS]);
        let Some(Ok(mut stream)) = self.stream.as_ref().map(|s| s.lock()) else {
            return none;
        };
        // A refused view keeps the pages already in place.
        let _ = stream.update(at, &mut |slot, row, bytes| {
            atlas.write(queue, slot, row, bytes)
        });
        let (shape, slots) = stream.rows();
        (clip, shape, slots)
    }

    /// The field's residency so far, when it streams one.
    #[must_use]
    pub fn metrics(&self) -> Option<verse_engine::streaming::Metrics> {
        self.stream
            .as_ref()
            .and_then(|s| s.lock().ok().map(|s| s.metrics()))
    }
}

impl SurfaceGpu {
    /// Uploads `surface`, drawing every other row and column of the
    /// patches that allow it on Low.
    #[must_use]
    pub fn upload(device: &wgpu::Device, surface: &WaterSurface, tier: Tier) -> Self {
        use wgpu::util::DeviceExt;
        let stride = if tier == Tier::Low { 2 } else { 1 };
        let mut vertices = surface.vertices();
        let (mut indices, ranges) = surface.ranges(stride);
        let bounds = body_bounds(&vertices);
        let ocean = surface.ocean.as_ref().map(|ocean| {
            let spec = clipmap::Spec::of(tier);
            let mut mesh = clipmap::mesh(&spec, ocean.body);
            let (base, first) = (vertices.len() as u32, indices.len() as u32);
            vertices.append(&mut mesh.vertices);
            indices.extend(mesh.indices.drain(..).map(|i| i + base));
            let shift = |r: &std::ops::Range<u32>| r.start + first..r.end + first;
            for level in &mut mesh.levels {
                *level = std::array::from_fn(|k| shift(&level[k]));
            }
            mesh.apron = shift(&mesh.apron);
            OceanMesh {
                spec,
                mesh,
                sea: ocean.sea,
                stream: ocean.field.clone().and_then(|f| {
                    field::Stream::new(f, tier)
                        .inspect_err(|e| eprintln!("verse: the water field does not stream: {e}"))
                        .ok()
                        .map(std::sync::Mutex::new)
                }),
            }
        });
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
            bounds,
            ocean,
        }
    }

    /// Triangles' indices in all, an ocean's one frame's.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.ranges.iter().map(|r| r.end - r.start).sum::<u32>()
            + self.ocean.as_ref().map_or(0, |o| 3 * o.spec.triangles())
    }

    /// The bytes the surface holds on the GPU.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.vertices.size() + self.indices.size()
    }

    /// The ocean's index ranges for an eye at `eye`, or none.
    #[must_use]
    pub fn ocean_ranges(&self, eye: glam::Vec3) -> Vec<std::ops::Range<u32>> {
        self.ocean.as_ref().map_or_else(Vec::new, |o| {
            o.mesh.draw(&o.spec, glam::Vec2::new(eye.x, eye.z))
        })
    }

    /// Draws the ocean's clipmap for an eye at `eye` through `transmit`
    /// and `emit`, or once through `emit` alone when `transmit` is none.
    pub fn draw_ocean<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        eye: glam::Vec3,
        transmit: Option<&'a wgpu::RenderPipeline>,
        emit: &'a wgpu::RenderPipeline,
    ) {
        let ranges = self.ocean_ranges(eye);
        if ranges.is_empty() {
            return;
        }
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        for range in ranges.into_iter().filter(|r| !r.is_empty()) {
            if let Some(transmit) = transmit {
                pass.set_pipeline(transmit);
                pass.draw_indexed(range.clone(), 0, 0..1);
            }
            pass.set_pipeline(emit);
            pass.draw_indexed(range, 0, 0..1);
        }
    }

    /// Draws each patch once through `pipeline`, which reads what lies
    /// behind the surface from the scene copy ([`screen`]).
    pub fn draw_once<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        pipeline: &'a wgpu::RenderPipeline,
    ) {
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_pipeline(pipeline);
        for range in &self.ranges {
            if !range.is_empty() {
                pass.draw_indexed(range.clone(), 0, 0..1);
            }
        }
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

/// Where each body's surface lies at rest, from the vertices of seas,
/// ponds, and streams.
#[must_use]
pub fn body_bounds(vertices: &[WaterVertex]) -> [Option<screen::Bounds>; MAX_BODIES] {
    let mut out: [Option<screen::Bounds>; MAX_BODIES] = [None; MAX_BODIES];
    for v in vertices {
        // Sea and body codes lie below 2.99; falls are 3 and orbs above 4.
        if !(v.kind < 2.99) || !v.body.is_finite() {
            continue;
        }
        let p = glam::Vec3::from_array(v.pos);
        if !p.is_finite() {
            continue;
        }
        let slot = &mut out[(v.body.max(0.0) as usize).min(MAX_BODIES - 1)];
        *slot = Some(
            slot.map_or(screen::Bounds { min: p, max: p }, |b| screen::Bounds {
                min: b.min.min(p),
                max: b.max.max(p),
            }),
        );
    }
    out
}
