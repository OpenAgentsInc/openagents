//! Temporal anti-aliasing for the physical path (issue #10936).
//!
//! A frame with [`Taa`] on renders through a projection nudged by a
//! sub-pixel jitter from the Halton (2, 3) sequence ([`jitter`]), eight
//! steps long. After the scene, `fs_resolve` in `taa.wgsl` reprojects each
//! pixel into the last frame's history through the depth prepass and the
//! last frame's camera, clamps the history in YCoCg to the current frame's
//! neighborhood, and blends the two; `fs_sharpen` writes the result back
//! into the scene, slightly sharpened, for bloom and the output transform.
//! Edges that crawled as the camera moved, the timber frames, roof lines,
//! and thin debris, settle into their average coverage, and bloom and
//! smoke stop flickering.
//!
//! Each pixel reprojects with the nearest depth of its 3 by 3 neighborhood,
//! so a silhouette moves with what stands in front whichever side of the
//! edge the jitter put the pixel's center; reprojecting with the sky one
//! frame and the roof the next was most of the crawl that remained.
//!
//! It runs on the high tier, which draws the depth prepass it reprojects
//! through ([`verse_engine::quality::Quality::screen_space`]), and
//! `VERSE_TAA=0` turns it off. The medium tier, the phones' and the web's,
//! keeps 4x MSAA: it has no prepass, and one would draw every caster again
//! and add two full-size histories to read and write each frame, the costs
//! a tile-based GPU feels most.
//!
//! Moving objects reproject by the camera's motion only. The resolve also
//! writes each pixel's depth (clip w) into a depth history, and where the
//! history at a pixel's last place holds another surface than the one the
//! pixel shows, a chunk that moved on its own or what it uncovered, it is
//! not that pixel's past: the pixel trusts the current frame, held to a
//! tight box. With the clamp and the history's weight falling with the
//! pixel's motion, fast debris trails at most about a frame. The ideas are Karis, "High Quality
//! Temporal Supersampling" (SIGGRAPH 2014), and Salvi's variance clipping
//! (GDC 2016), with Unreal's documentation of its temporal upsampler as a
//! reading of the same ideas; the code is our own
//! (`docs/research/unreal/AGENTS.md`).

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2};

/// How many jitter positions the sequence cycles through.
pub const PHASES: u32 = 8;
/// The current frame's weight in the blend when nothing moves, when the
/// pixel moves [`FAST`] pixels a frame or more, and that speed.
pub const STILL: f32 = 0.08;
pub const MOVING: f32 = 0.5;
pub const FAST: f32 = 24.0;
/// How far the clamp box reaches from the neighborhood's mean, in its
/// standard deviations, once the pixel moves a pixel a frame or more, and
/// while it holds still: a still edge's neighborhood shifts with the
/// jitter, and a tight box there pulls its history back and forth, which is
/// the crawl.
pub const CLIP: f32 = 1.5;
pub const CLIP_STILL: f32 = 2.5;
/// How far, as a fraction of its depth, the surface the history holds at a
/// pixel's last place may lie from where this pixel's surface was before
/// the history counts as another surface's: a moving chunk, or what it
/// uncovered. Such a pixel trusts the current frame.
pub const DEPTH_TOLERANCE: f32 = 0.04;
/// The sharpening pass's strength.
pub const SHARPEN: f32 = 0.2;

/// Element `index` (from 1) of the Halton sequence in `base`.
#[must_use]
pub fn halton(mut index: u32, base: u32) -> f32 {
    let mut f = 1.0;
    let mut r = 0.0;
    while index > 0 {
        f /= base as f32;
        r += f * (index % base) as f32;
        index /= base;
    }
    r
}

/// Frame `frame`'s sub-pixel jitter, in pixels, each axis within half a
/// pixel of the center.
#[must_use]
pub fn jitter(frame: u32) -> Vec2 {
    let i = frame % PHASES + 1;
    Vec2::new(halton(i, 2) - 0.5, halton(i, 3) - 0.5)
}

/// `view_proj` moved by `pixels` on a `size` pixel screen: clip space
/// shifts by two over the size for each pixel, and y runs up in clip space
/// but down in pixels.
#[must_use]
pub fn jittered(view_proj: Mat4, pixels: Vec2, size: [u32; 2]) -> Mat4 {
    let dx = 2.0 * pixels.x / size[0].max(1) as f32;
    let dy = -2.0 * pixels.y / size[1].max(1) as f32;
    Mat4::from_translation(glam::Vec3::new(dx, dy, 0.0)) * view_proj
}

/// The `Taa` uniform in `taa.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TaaUniform {
    inv_view_proj: [[f32; 4]; 4],
    prev_view_proj: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
    size: [f32; 4],
    blend: [f32; 4],
    sharpen: [f32; 4],
    jitter: [f32; 4],
}

impl TaaUniform {
    /// The uniform for a frame whose unjittered, reversed-depth camera is
    /// `view_proj`, drawn `jitter` pixels off, whose last frame's camera
    /// was `previous`; `reset` drops the history.
    #[must_use]
    pub fn new(view_proj: Mat4, previous: Mat4, size: [u32; 2], jitter: Vec2, reset: bool) -> Self {
        let (w, h) = (size[0].max(1) as f32, size[1].max(1) as f32);
        Self {
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            prev_view_proj: previous.to_cols_array_2d(),
            view_proj: view_proj.to_cols_array_2d(),
            size: [w, h, 1.0 / w, 1.0 / h],
            blend: [STILL, MOVING, FAST, f32::from(u8::from(reset))],
            sharpen: [SHARPEN, CLIP, CLIP_STILL, DEPTH_TOLERANCE],
            jitter: [jitter.x, jitter.y, 0.0, 0.0],
        }
    }
}

/// Whether temporal anti-aliasing runs where it can: on unless `VERSE_TAA`
/// is `0` or `off`.
#[must_use]
pub fn enabled() -> bool {
    !matches!(
        std::env::var("VERSE_TAA").as_deref(),
        Ok("0" | "off" | "false")
    )
}

/// The resolve and sharpen pipelines, their uniform, and the camera they
/// last ran with.
pub struct Taa {
    uniform: wgpu::Buffer,
    resolve_layout: wgpu::BindGroupLayout,
    sharpen_layout: wgpu::BindGroupLayout,
    resolve: wgpu::RenderPipeline,
    sharpen: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    /// Frames drawn, which picks the jitter.
    pub frame: u32,
    /// The last frame's unjittered camera and size.
    previous: Option<(Mat4, [u32; 2])>,
}

/// The format of the depth history: each pixel's clip-space w, its
/// distance along the view.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

/// The history textures for one size, and the bind groups that read them.
pub struct TaaTargets {
    histories: [wgpu::TextureView; 2],
    /// Each history pixel's depth, for telling a surface from another.
    depths: [wgpu::TextureView; 2],
    /// For each history slot written this frame: the resolve's group (the
    /// scene and the other slot) and the sharpen's (this slot).
    resolve_groups: [wgpu::BindGroup; 2],
    sharpen_groups: [wgpu::BindGroup; 2],
    /// The slot the next frame writes.
    next: usize,
    size: [u32; 2],
}

impl Taa {
    /// The pipelines for a scene in `format`.
    #[must_use]
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse taa"),
            size: std::mem::size_of::<TaaUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let fragment = wgpu::ShaderStages::FRAGMENT;
        let uniform_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: fragment,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture = |binding, sample_type| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: fragment,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let color = wgpu::TextureSampleType::Float { filterable: true };
        let resolve_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse taa resolve"),
            entries: &[
                uniform_entry,
                texture(1, wgpu::TextureSampleType::Depth),
                texture(2, color),
                texture(3, color),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: fragment,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture(5, wgpu::TextureSampleType::Float { filterable: false }),
            ],
        });
        let sharpen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse taa sharpen"),
            entries: &[uniform_entry, texture(3, color)],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("verse taa"),
            source: wgpu::ShaderSource::Wgsl(include_str!("taa.wgsl").into()),
        });
        let pipeline = |layout: &wgpu::BindGroupLayout, label: &str, fs: &str, depth: bool| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_fullscreen"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[
                        Some(wgpu::ColorTargetState {
                            format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                        depth.then_some(wgpu::ColorTargetState {
                            format: DEPTH_FORMAT,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }),
                    ][..1 + usize::from(depth)],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let resolve = pipeline(&resolve_layout, "verse taa resolve", "fs_resolve", true);
        let sharpen = pipeline(&sharpen_layout, "verse taa sharpen", "fs_sharpen", false);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("verse taa history"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            uniform,
            resolve_layout,
            sharpen_layout,
            resolve,
            sharpen,
            sampler,
            frame: 0,
            previous: None,
        }
    }

    /// The histories for a `size` frame in `format` reading `scene` and the
    /// prepass `depth`.
    #[must_use]
    pub fn targets(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        scene: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        size: [u32; 2],
    ) -> TaaTargets {
        let history = |format| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("verse taa history"),
                    size: wgpu::Extent3d {
                        width: size[0].max(1),
                        height: size[1].max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let histories = [history(format), history(format)];
        let depths = [history(DEPTH_FORMAT), history(DEPTH_FORMAT)];
        let view = |binding, view| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        };
        let uniform = || wgpu::BindGroupEntry {
            binding: 0,
            resource: self.uniform.as_entire_binding(),
        };
        let resolve_groups = [0, 1].map(|slot| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("verse taa resolve"),
                layout: &self.resolve_layout,
                entries: &[
                    uniform(),
                    view(1, depth),
                    view(2, scene),
                    view(3, &histories[1 - slot]),
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    view(5, &depths[1 - slot]),
                ],
            })
        });
        let sharpen_groups = [0, 1].map(|slot| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("verse taa sharpen"),
                layout: &self.sharpen_layout,
                entries: &[uniform(), view(3, &histories[slot])],
            })
        });
        TaaTargets {
            histories,
            depths,
            resolve_groups,
            sharpen_groups,
            next: 0,
            size,
        }
    }

    /// This frame's jittered camera for unjittered, reversed-depth
    /// `view_proj` at `size`, and the jitter in pixels.
    #[must_use]
    pub fn jitter(&self, view_proj: Mat4, size: [u32; 2]) -> (Mat4, Vec2) {
        let pixels = jitter(self.frame);
        (jittered(view_proj, pixels, size), pixels)
    }

    /// Resolves this frame's `scene` into the history and writes it back,
    /// sharpened, into `scene`. `view_proj` is the frame's unjittered,
    /// reversed-depth camera; it drew [`Self::jitter`]'s jitter off.
    pub fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        targets: &mut TaaTargets,
        scene: &wgpu::TextureView,
        view_proj: Mat4,
    ) {
        let size = targets.size;
        let reset = self.previous.is_none_or(|(_, s)| s != size);
        let previous = self.previous.map_or(view_proj, |(m, _)| m);
        let uniform = TaaUniform::new(view_proj, previous, size, jitter(self.frame), reset);
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        let slot = targets.next;
        for (label, view, depth, pipeline, group) in [
            (
                "verse taa resolve",
                &targets.histories[slot],
                Some(&targets.depths[slot]),
                &self.resolve,
                &targets.resolve_groups[slot],
            ),
            (
                "verse taa sharpen",
                scene,
                None,
                &self.sharpen,
                &targets.sharpen_groups[slot],
            ),
        ] {
            let attachment = |view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })
            };
            let attachments = [attachment(view), depth.and_then(attachment)];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &attachments[..1 + usize::from(depth.is_some())],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
        targets.next = 1 - slot;
        self.previous = Some((view_proj, size));
        self.frame = self.frame.wrapping_add(1);
    }

    /// Forgets the history, as after a cut to another place.
    pub fn reset(&mut self) {
        self.previous = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Vec3, Vec4};

    #[test]
    fn the_jitter_covers_the_pixel_without_repeating_within_its_cycle() {
        let points: Vec<Vec2> = (0..PHASES).map(jitter).collect();
        for (i, p) in points.iter().enumerate() {
            assert!(p.x.abs() < 0.5 && p.y.abs() < 0.5, "{p}");
            for q in &points[i + 1..] {
                assert!(p.distance(*q) > 0.05, "{p} repeats {q}");
            }
        }
        // Centered on the pixel, near enough.
        let mean = points.iter().copied().sum::<Vec2>() / PHASES as f32;
        assert!(mean.length() < 0.1, "{mean}");
        assert_eq!(jitter(3), jitter(3 + PHASES));
        assert!((halton(1, 2) - 0.5).abs() < 1e-6 && (halton(2, 3) - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn a_jittered_camera_moves_points_by_the_jitter_in_pixels() {
        let view_proj = Mat4::perspective_rh(1.0, 16.0 / 9.0, 0.1, 100.0)
            * Mat4::look_at_rh(Vec3::new(0.0, 2.0, 5.0), Vec3::ZERO, Vec3::Y);
        let size = [1920, 1080];
        let pixel = |m: Mat4, p: Vec3| {
            let c = m * Vec4::new(p.x, p.y, p.z, 1.0);
            let ndc = c.truncate() / c.w;
            Vec2::new(
                (ndc.x * 0.5 + 0.5) * size[0] as f32,
                (0.5 - ndc.y * 0.5) * size[1] as f32,
            )
        };
        let shift = Vec2::new(0.25, -0.375);
        let moved = jittered(view_proj, shift, size);
        for p in [
            Vec3::ZERO,
            Vec3::new(1.0, 0.5, -3.0),
            Vec3::new(-2.0, 0.0, 1.0),
        ] {
            let d = pixel(moved, p) - pixel(view_proj, p);
            assert!((d - shift).length() < 1e-3, "{d}");
        }
    }

    /// `taa.wgsl` validates and translates to Metal, and its uniform has the
    /// Rust struct's size.
    #[test]
    fn the_taa_shader_validates() {
        let source = include_str!("taa.wgsl");
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("taa.wgsl: {}", e.emit_to_string(source)));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("taa.wgsl: {}", e.emit_to_string(source)));
        naga::back::msl::write_string(
            &module,
            &info,
            &naga::back::msl::Options {
                lang_version: (2, 3),
                ..Default::default()
            },
            &naga::back::msl::PipelineOptions::default(),
        )
        .unwrap_or_else(|e| panic!("taa.wgsl: {e}"));
        let taa = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Taa"))
            .expect("the Taa struct");
        assert_eq!(
            taa.1.inner.size(module.to_ctx()) as usize,
            std::mem::size_of::<TaaUniform>()
        );
    }
}
