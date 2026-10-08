//! Temporal antialiasing from subpixel camera samples and reprojected history.
//!
//! The sampling and history conditioning follow the public presentations by
//! [Karis (2014)](https://www.advances.realtimerendering.com/s2014/index.html)
//! and [Salvi (2016)](https://developer.download.nvidia.com/gameworks/events/GDC2016/msalvi_temporal_supersampling.pdf),
//! reimplemented here without engine code.
//! History stays linear, before bloom and tone mapping. Moving objects supply
//! their current and previous transforms through [`MotionDraw`].

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3, Vec4};
use verse_engine::{
    presentation::View,
    quality::{Platform, Tier},
};

/// A moving object's draw, with the textured renderer's 28-byte vertices and
/// 128-byte instance records: current affine rows at byte 0, previous at 48.
pub struct MotionDraw<'a> {
    pub vertices: &'a wgpu::Buffer,
    pub indices: &'a wgpu::Buffer,
    pub instances: &'a wgpu::Buffer,
    pub draw: MotionCommand<'a>,
}

/// Current lit triangles whose color must not enter temporal history.
/// The buffer is the dynamic lit stream already drawn by the color pass.
#[derive(Clone, Copy)]
pub struct ReactiveLit<'a> {
    pub vertices: &'a wgpu::Buffer,
    pub count: u32,
    pub ranges: &'a [std::ops::Range<u32>],
}

impl ReactiveLit<'_> {
    fn admitted_ranges(&self) -> impl Iterator<Item = std::ops::Range<u32>> + '_ {
        self.ranges
            .iter()
            .filter(|range| {
                range.start < range.end
                    && range.end <= self.count
                    && range.start % 3 == 0
                    && range.end % 3 == 0
            })
            .cloned()
    }
}

/// Direct draws are portable; native devices can share one indirect command stream.
pub enum MotionCommand<'a> {
    Indexed {
        draw: super::instanced::Draw,
        double_sided: bool,
        /// Current world bounds; absent for callers without rigid batch bounds.
        bounds: Option<[Vec3; 2]>,
    },
    Indirect {
        buffer: &'a wgpu::Buffer,
        first: u32,
        count: u32,
        double_sided: bool,
    },
}

impl MotionCommand<'_> {
    fn double_sided(&self) -> bool {
        match self {
            Self::Indexed { double_sided, .. } | Self::Indirect { double_sided, .. } => {
                *double_sided
            }
        }
    }

    fn has_vertices(&self) -> bool {
        match self {
            Self::Indexed { draw, .. } => draw.count > 0 && draw.instances.count > 0,
            Self::Indirect { count, .. } => *count > 0,
        }
    }

    fn admitted(&self, view_proj: Mat4) -> bool {
        self.has_vertices()
            && match self {
                Self::Indexed {
                    bounds: Some([min, max]),
                    ..
                } => super::textured::in_frustum_conservative(*min, *max, view_proj),
                _ => true,
            }
    }
}

pub(crate) struct MotionRun {
    pub first: u32,
    pub count: u32,
    pub double_sided: bool,
}

/// Keeps draw order while grouping adjacent commands with the same culling.
pub(crate) fn indirect_motion(
    draws: &[super::instanced::Draw],
    double_sided: impl Fn(usize) -> bool,
) -> (Vec<wgpu::util::DrawIndexedIndirectArgs>, Vec<MotionRun>) {
    let mut commands = Vec::with_capacity(draws.len());
    let mut runs: Vec<MotionRun> = Vec::new();
    for draw in draws {
        let sided = double_sided(draw.item);
        if runs.last().is_none_or(|run| run.double_sided != sided) {
            runs.push(MotionRun {
                first: commands.len() as u32,
                count: 0,
                double_sided: sided,
            });
        }
        runs.last_mut().expect("motion run").count += 1;
        commands.push(draw.indirect());
    }
    (commands, runs)
}

/// Whether the physical renderer can keep temporal history on this platform.
#[must_use]
pub fn supported(platform: Platform, tier: Tier, hdr: bool, gles: bool) -> bool {
    platform == Platform::Desktop && tier != Tier::Low && hdr && !gles
}

/// One eight-frame Halton sample, in pixels, centered within a pixel.
#[must_use]
pub fn jitter(frame: u64) -> Vec2 {
    fn radical(mut index: u32, base: u32) -> f32 {
        let mut value = 0.0;
        let mut weight = 1.0 / base as f32;
        while index > 0 {
            value += (index % base) as f32 * weight;
            index /= base;
            weight /= base as f32;
        }
        value
    }
    let index = (frame % 8) as u32 + 1;
    Vec2::new(radical(index, 2), radical(index, 3)) - Vec2::splat(0.5)
}

fn jittered(view: View, size: [u32; 2], sample: Vec2) -> View {
    let offset = Vec2::new(
        2.0 * sample.x / size[0] as f32,
        -2.0 * sample.y / size[1] as f32,
    );
    let mut shift = Mat4::IDENTITY;
    shift.w_axis.x = offset.x;
    shift.w_axis.y = offset.y;
    View {
        view_proj: shift * view.view_proj,
        ..view
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniform {
    current: [[f32; 4]; 4],
    inverse: [[f32; 4]; 4],
    previous: [[f32; 4]; 4],
    size: [f32; 4],
    settings: [f32; 4],
}

/// Device resources for the resolve, sharpening, and moving-object motion pass.
pub(super) struct Temporal {
    resolve_layout: wgpu::BindGroupLayout,
    sharpen_layout: wgpu::BindGroupLayout,
    motion_layout: wgpu::BindGroupLayout,
    reactive_layout: wgpu::BindGroupLayout,
    samples: u32,
    resolve: wgpu::RenderPipeline,
    sharpen: wgpu::RenderPipeline,
    motion: [wgpu::RenderPipeline; 2],
    reactive: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

/// History belongs to one view size and is discarded with that view's targets.
pub(super) struct Targets {
    history: [wgpu::TextureView; 2],
    motion: wgpu::TextureView,
    reactive: wgpu::TextureView,
    reactive_msaa: Option<wgpu::TextureView>,
    reactive_group: wgpu::BindGroup,
    depth: wgpu::TextureView,
    resolve: [wgpu::BindGroup; 2],
    sharpen: [wgpu::BindGroup; 2],
    motion_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    size: [u32; 2],
    write: usize,
    camera: CameraHistory,
    pub enabled: bool,
}

#[derive(Default)]
struct CameraHistory {
    frame: u64,
    previous: Option<(View, f32)>,
    prepared: Uniform,
}

impl Default for Uniform {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl CameraHistory {
    fn prepare(&mut self, view: View, size: [u32; 2], time: f32, enabled: bool) -> View {
        if !enabled || !usable_camera(view) {
            self.previous = None;
            self.frame = 0;
            return view;
        }
        let sample = jitter(self.frame);
        let camera = jittered(view, size, sample);
        let valid = self.previous.is_some_and(|(old, at)| {
            let focal = |matrix: Mat4| matrix.transpose().x_axis.truncate().length();
            let zoom = focal(view.view_proj) / focal(old.view_proj);
            time >= at
                && time - at <= 0.5
                && old.eye.distance(view.eye) < 8.0
                && direction(old).dot(direction(view)) > 0.8
                && (0.67..=1.5).contains(&zoom)
        });
        let current = super::gpu::reversed_depth() * camera.view_proj;
        let previous = self
            .previous
            .filter(|_| valid)
            .map_or(current, |(old, _)| old.view_proj);
        self.prepared = Uniform {
            current: current.to_cols_array_2d(),
            inverse: current.inverse().to_cols_array_2d(),
            previous: previous.to_cols_array_2d(),
            size: [
                size[0] as f32,
                size[1] as f32,
                1.0 / size[0] as f32,
                1.0 / size[1] as f32,
            ],
            settings: [f32::from(u8::from(valid)), 0.9, 0.15, 0.0],
        };
        self.previous = Some((
            View {
                view_proj: current,
                ..camera
            },
            time,
        ));
        self.frame += 1;
        camera
    }
}

fn usable_camera(view: View) -> bool {
    let determinant = view.view_proj.determinant();
    view.view_proj.is_finite()
        && view.eye.is_finite()
        && determinant.is_finite()
        && determinant.abs() > 1e-12
}

fn direction(view: View) -> Vec3 {
    let center = view.view_proj.inverse() * Vec4::new(0.0, 0.0, 0.5, 1.0);
    (center.truncate() / center.w - view.eye).normalize_or_zero()
}

impl Targets {
    pub fn reset(&mut self) {
        self.camera = CameraHistory::default();
        self.enabled = false;
    }

    pub fn prepare(&mut self, view: View, time: f32, enabled: bool) -> View {
        self.enabled = enabled && usable_camera(view);
        self.camera.prepare(view, self.size, time, self.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_runs_keep_winding_policy_and_instance_order() {
        use super::super::instanced::{Draw, Run};
        let draws: Vec<_> = (0..4)
            .map(|item| Draw {
                item,
                first: item as u32 * 3,
                count: 3,
                instances: Run {
                    first: item as u32 + 1,
                    count: 1,
                },
            })
            .collect();
        let (commands, runs) = indirect_motion(&draws, |item| item == 2);
        assert_eq!(
            runs.iter()
                .map(|run| (run.first, run.count, run.double_sided))
                .collect::<Vec<_>>(),
            [(0, 2, false), (2, 1, true), (3, 1, false)]
        );
        assert_eq!(
            commands
                .iter()
                .map(|command| command.first_instance)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert_eq!(
            commands
                .iter()
                .map(|command| command.first_index)
                .collect::<Vec<_>>(),
            [0, 3, 6, 9]
        );
        assert!(indirect_motion(&[], |_| false).1.is_empty());
        let empty = MotionCommand::Indexed {
            draw: Draw {
                count: 0,
                ..draws[0]
            },
            double_sided: false,
            bounds: None,
        };
        assert!(!empty.has_vertices());
    }
    use glam::{Vec3, Vec4};

    fn view(eye: Vec3) -> View {
        View {
            view_proj: Mat4::perspective_rh(1.0, 16.0 / 9.0, 0.1, 1000.0)
                * Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y),
            eye,
        }
    }

    fn close(a: Vec4, b: Vec4) {
        assert!((a - b).abs().max_element() < 1e-4, "{a:?} != {b:?}");
    }

    #[test]
    fn halton_samples_cover_a_pixel_and_repeat_after_eight_frames() {
        let mut samples = Vec::new();
        for frame in 0..8 {
            let sample = jitter(frame);
            assert!(sample.abs().max_element() <= 0.5);
            assert!(!samples.contains(&sample));
            assert_eq!(sample, jitter(frame + 8));
            samples.push(sample);
        }
        assert!((jitter(0) - Vec2::new(0.0, -1.0 / 6.0)).abs().max_element() < 1e-6);
        assert!(
            (jitter(1) - Vec2::new(-0.25, 1.0 / 6.0))
                .abs()
                .max_element()
                < 1e-6
        );
    }

    #[test]
    fn projection_jitter_moves_pixels_without_changing_depth() {
        let original = view(Vec3::new(0.0, 2.0, 10.0));
        let size = [1920, 1080];
        let sample = Vec2::new(0.25, -0.25);
        let camera = jittered(original, size, sample);
        let a = original.view_proj * Vec4::new(2.0, 0.0, 0.0, 1.0);
        let b = camera.view_proj * Vec4::new(2.0, 0.0, 0.0, 1.0);
        let pixel_shift = Vec2::new(
            (b.x / b.w - a.x / a.w) * 960.0,
            (a.y / a.w - b.y / b.w) * 540.0,
        );
        assert!((pixel_shift - sample).abs().max_element() < 1e-4);
        assert_eq!((a.z, a.w), (b.z, b.w));
        assert_eq!(camera.eye, original.eye);
    }

    #[test]
    fn motion_admission_uses_the_rendered_jitter_and_preserves_uncertain_streams() {
        let point = Vec3::new(1.000_2, 0.0, 0.5);
        let mut command = MotionCommand::Indexed {
            draw: super::super::instanced::Draw {
                item: 0,
                first: 9,
                count: 6,
                instances: super::super::instanced::Run { first: 7, count: 2 },
            },
            double_sided: false,
            bounds: Some([point, point]),
        };
        assert!(!command.admitted(Mat4::IDENTITY));
        let camera = jittered(
            View {
                view_proj: Mat4::IDENTITY,
                eye: Vec3::ZERO,
            },
            [1920, 1080],
            jitter(1),
        );
        assert!(command.admitted(camera.view_proj));
        if let MotionCommand::Indexed { bounds, .. } = &mut command {
            *bounds = Some([Vec3::NAN, Vec3::ONE]);
        }
        assert!(command.admitted(Mat4::IDENTITY));
        if let MotionCommand::Indexed { bounds, .. } = &mut command {
            *bounds = None;
        }
        assert!(command.admitted(Mat4::IDENTITY));
        if let MotionCommand::Indexed { draw, .. } = command {
            assert_eq!(
                (
                    draw.first,
                    draw.count,
                    draw.instances.first,
                    draw.instances.count
                ),
                (9, 6, 7, 2)
            );
        }
    }

    #[test]
    fn camera_reprojection_uses_the_previous_jittered_camera() {
        let mut history = CameraHistory::default();
        let first = history.prepare(view(Vec3::new(0.0, 2.0, 10.0)), [1920, 1080], 1.0, true);
        assert_eq!(history.prepared.settings[0], 0.0);
        history.prepare(
            view(Vec3::new(0.1, 2.0, 10.0)),
            [1920, 1080],
            1.0 + 1.0 / 60.0,
            true,
        );
        assert_eq!(history.prepared.settings[0], 1.0);
        let uniform = history.prepared;
        let point = Vec4::new(2.0, 1.0, -3.0, 1.0);
        let current = Mat4::from_cols_array_2d(&uniform.current) * point;
        let ndc = current / current.w;
        let reconstructed = Mat4::from_cols_array_2d(&uniform.inverse) * ndc;
        let reconstructed = reconstructed / reconstructed.w;
        close(reconstructed, point);
        let reprojected = Mat4::from_cols_array_2d(&uniform.previous) * reconstructed;
        close(
            reprojected,
            super::super::gpu::reversed_depth() * first.view_proj * point,
        );
    }

    #[test]
    fn disable_time_rewind_pause_and_camera_cut_discard_history() {
        let mut history = CameraHistory::default();
        let camera = view(Vec3::new(0.0, 2.0, 10.0));
        history.prepare(camera, [1920, 1080], 1.0, true);
        let unchanged = history.prepare(camera, [1920, 1080], 1.1, false);
        assert_eq!(unchanged.view_proj, camera.view_proj);
        assert_eq!(history.frame, 0);
        assert!(history.previous.is_none());
        history.prepare(camera, [1920, 1080], 1.2, true);
        assert_eq!(history.prepared.settings[0], 0.0);
        history.prepare(camera, [1920, 1080], 1.0, true);
        assert_eq!(history.prepared.settings[0], 0.0);
        history.prepare(camera, [1920, 1080], 2.0, true);
        assert_eq!(history.prepared.settings[0], 0.0);
        history.prepare(view(Vec3::new(10.0, 2.0, 10.0)), [1920, 1080], 2.1, true);
        assert_eq!(history.prepared.settings[0], 0.0);
        let turned = View {
            view_proj: Mat4::perspective_rh(1.0, 16.0 / 9.0, 0.1, 1000.0)
                * Mat4::look_at_rh(camera.eye, camera.eye + Vec3::X, Vec3::Y),
            ..camera
        };
        history.prepare(camera, [1920, 1080], 2.15, true);
        history.prepare(turned, [1920, 1080], 2.2, true);
        assert_eq!(history.prepared.settings[0], 0.0);
        history.prepare(camera, [1920, 1080], 2.25, true);
        let zoomed = View {
            view_proj: Mat4::from_scale(Vec3::new(2.0, 2.0, 1.0)) * camera.view_proj,
            ..camera
        };
        history.prepare(zoomed, [1920, 1080], 2.26, true);
        assert_eq!(history.prepared.settings[0], 0.0);
        let singular = View {
            view_proj: Mat4::ZERO,
            ..camera
        };
        history.prepare(singular, [1920, 1080], 2.3, true);
        assert!(history.previous.is_none());
    }

    #[test]
    fn temporal_history_respects_platform_and_tier_ceilings() {
        for tier in [Tier::Medium, Tier::High] {
            assert!(supported(Platform::Desktop, tier, true, false));
            assert!(!supported(Platform::Desktop, tier, false, false));
            assert!(!supported(Platform::Desktop, tier, true, true));
            for platform in [Platform::Mobile, Platform::Web] {
                assert!(!supported(platform, tier, true, false));
            }
        }
        assert!(!supported(Platform::Desktop, Tier::Low, true, false));
        assert_eq!(bytes(1920, 1080, 1), 51_840_000);
        assert_eq!(bytes(1920, 1080, 4), 60_134_400);
    }

    #[test]
    fn temporal_shaders_validate_for_one_and_four_sample_depth() {
        for samples in [1, 4] {
            for source in [
                include_str!("temporal.wgsl"),
                include_str!("temporal_motion.wgsl"),
                include_str!("temporal_reactive.wgsl"),
                include_str!("temporal_sharpen.wgsl"),
            ] {
                let source = depth_source(source, samples);
                let module = naga::front::wgsl::parse_str(&source).unwrap();
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::empty(),
                )
                .validate(&module)
                .unwrap();
                let mut layouter = naga::proc::Layouter::default();
                layouter.update(module.to_ctx()).unwrap();
                let camera = module
                    .types
                    .iter()
                    .find(|(_, ty)| ty.name.as_deref() == Some("Camera"))
                    .unwrap()
                    .0;
                assert_eq!(layouter[camera].size, std::mem::size_of::<Uniform>() as u32);
                let images = module
                    .global_variables
                    .iter()
                    .filter(|(_, variable)| {
                        matches!(
                            module.types[variable.ty].inner,
                            naga::TypeInner::Image { .. }
                        )
                    })
                    .count();
                assert!(
                    images <= 5,
                    "temporal pass exceeds its separate texture budget"
                );
                let naga::TypeInner::Struct { members, .. } = &module.types[camera].inner else {
                    panic!("camera is a struct")
                };
                assert_eq!(
                    members
                        .iter()
                        .map(|member| member.offset)
                        .collect::<Vec<_>>(),
                    vec![0, 64, 128, 192, 208]
                );
            }
        }
    }

    #[test]
    #[ignore = "Requires a native GPU; checks empty and clipped motion with retained object data"]
    fn empty_motion_keeps_camera_reprojection_with_stale_object_data() {
        for clipped_draw in [false, true] {
            camera_reprojection_with_stale_object_data(clipped_draw);
        }
    }

    fn camera_reprojection_with_stale_object_data(clipped_draw: bool) {
        use wgpu::util::DeviceExt;
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let size = wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        };
        let scene = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("empty motion scene"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let pixels: Vec<u16> = (0..4)
            .flat_map(|y| {
                (0..4).flat_map(move |x| {
                    let value = half::f16::from_f32(((x + y) % 2) as f32).to_bits();
                    [value, value, value, half::f16::ONE.to_bits()]
                })
            })
            .collect();
        queue.write_texture(
            scene.as_image_copy(),
            bytemuck::cast_slice(&pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(32),
                rows_per_image: Some(4),
            },
            size,
        );
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("empty motion depth"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let scene_view = scene.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let temporal = Temporal::new(&device, wgpu::TextureFormat::Rgba16Float, 1);
        let mut targets = temporal.targets(&device, &scene_view, &depth_view, [4, 4]);
        targets.enabled = true;
        targets.camera.prepared = Uniform {
            current: Mat4::IDENTITY.to_cols_array_2d(),
            inverse: Mat4::IDENTITY.to_cols_array_2d(),
            previous: Mat4::IDENTITY.to_cols_array_2d(),
            size: [4.0, 4.0, 0.25, 0.25],
            settings: [1.0, 0.9, 0.0, 1.0],
        };
        let readback = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &[0; 1024],
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.002),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
        }
        for (view, color) in [
            (
                &targets.history[1],
                wgpu::Color {
                    r: 0.25,
                    g: 0.25,
                    b: 0.25,
                    a: 1.0,
                },
            ),
            // If read, this old object's depth rejects the valid camera history.
            (
                &targets.motion,
                wgpu::Color {
                    r: 0.25,
                    g: 0.0,
                    b: 20.0,
                    a: 1.0,
                },
            ),
        ] {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        let vertices =
            [Vec3::ZERO, Vec3::X, Vec3::Y].map(|position| super::super::instanced::GpuVertex {
                pos: (position + Vec3::Z * 0.002).to_array(),
                normal: [0; 2],
                uv: [0.0; 2],
                color: [255; 4],
            });
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[0_u32, 1, 2]),
            usage: wgpu::BufferUsages::INDEX,
        });
        let records = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&super::super::instanced::Instance::new(
                Mat4::from_translation(Vec3::X * 10.0),
                0,
            )),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let motions = [MotionDraw {
            vertices: &vertices,
            indices: &indices,
            instances: &records,
            draw: MotionCommand::Indexed {
                draw: super::super::instanced::Draw {
                    item: 0,
                    first: 0,
                    count: 3,
                    instances: super::super::instanced::Run { first: 0, count: 1 },
                },
                double_sided: false,
                bounds: Some([Vec3::new(10.0, 0.0, 0.002), Vec3::new(11.0, 1.0, 0.002)]),
            },
        }];
        assert!(!motions[0].draw.admitted(Mat4::IDENTITY));
        temporal.encode(
            &queue,
            &mut encoder,
            &scene_view,
            &mut targets,
            if clipped_draw { &motions } else { &[] },
            None,
        );
        encoder.copy_texture_to_buffer(
            scene.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(4),
                },
            },
            size,
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).unwrap();
            });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
        rx.recv().unwrap().unwrap();
        let data = readback.slice(..).get_mapped_range();
        let color =
            half::f16::from_bits(u16::from_le_bytes(data[264..266].try_into().unwrap())).to_f32();
        assert!(
            (color - 0.225).abs() < 0.001,
            "empty or clipped motion must retain camera history: {color}, clipped={clipped_draw}"
        );
        drop(data);
        readback.unmap();
    }

    #[test]
    #[ignore = "Requires a native GPU; checks visible and occluded moving surfaces"]
    fn hidden_motion_cannot_overwrite_the_visible_surface() {
        use wgpu::util::DeviceExt;
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let indirect = adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION)
            && adapter
                .features()
                .contains(wgpu::Features::INDIRECT_FIRST_INSTANCE);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: if indirect {
                wgpu::Features::INDIRECT_FIRST_INSTANCE
            } else {
                wgpu::Features::empty()
            },
            ..Default::default()
        }))
        .unwrap();
        let size = wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        };
        for samples in [1, 4] {
            let temporal = Temporal::new(&device, wgpu::TextureFormat::Rgba16Float, samples);
            let depth_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("motion test masked scene"),
                source: wgpu::ShaderSource::Wgsl(
                    "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                        let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
                        return vec4<f32>(positions[i], 0.002, 1.0);
                    }
                    @fragment fn fs() -> @builtin(frag_depth) f32 { return 0.002; }"
                        .into(),
                ),
            });
            let depth_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("motion test masked scene"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &depth_shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &depth_shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[],
                }),
                multiview_mask: None,
                cache: None,
            });
            let depth = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("motion test scene depth"),
                size,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let output = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("motion test output"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let uniform = Uniform {
                current: Mat4::IDENTITY.to_cols_array_2d(),
                previous: Mat4::IDENTITY.to_cols_array_2d(),
                inverse: Mat4::IDENTITY.to_cols_array_2d(),
                size: [4.0, 4.0, 0.25, 0.25],
                settings: [0.0; 4],
            };
            let camera = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let depth_view = depth.create_view(&Default::default());
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &temporal.motion_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&depth_view),
                    },
                ],
            });
            let vertices =
                [[-1.0, -1.0, 0.002], [3.0, -1.0, 0.002], [-1.0, 3.0, 0.002]].map(|pos| {
                    super::super::instanced::GpuVertex {
                        pos,
                        normal: [0; 2],
                        uv: [0.0; 2],
                        color: [255; 4],
                    }
                });
            let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let indices = [[0_u32, 1, 2], [0_u32, 2, 1]].map(|indices| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                })
            });
            let mut records = Vec::new();
            for (current, previous) in [
                (Mat4::IDENTITY, Mat4::IDENTITY),
                (
                    Mat4::IDENTITY,
                    Mat4::from_translation(Vec3::new(0.25, 0.0, 0.0)),
                ),
                (
                    Mat4::from_translation(Vec3::new(0.0, 0.0, -0.000005)),
                    Mat4::from_translation(Vec3::new(-0.25, 0.0, 0.0)),
                ),
                (
                    Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
                    Mat4::from_translation(Vec3::new(9.0, 0.0, 0.0)),
                ),
            ] {
                for matrix in [current, previous] {
                    let matrix = matrix.transpose();
                    for row in [matrix.x_axis, matrix.y_axis, matrix.z_axis] {
                        records.extend(row.to_array());
                    }
                }
                records.extend([0.0_f32; 8]);
            }
            let records = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&records),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let commands = [1, 2, 3].map(|first_instance| wgpu::util::DrawIndexedIndirectArgs {
                index_count: 3,
                instance_count: 1,
                first_index: 0,
                base_vertex: 0,
                first_instance,
            });
            let commands = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&commands),
                usage: wgpu::BufferUsages::INDIRECT,
            });
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 1024,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let output_view = output.create_view(&Default::default());
            for use_indirect in [false, true].into_iter().filter(|&mode| !mode || indirect) {
                for reversed in [false, true] {
                    for double_sided in [false, true] {
                        let mut unfiltered = None;
                        for cull in [false, true] {
                            let mut encoder = device.create_command_encoder(&Default::default());
                            {
                                let mut pass =
                                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                        label: None,
                                        color_attachments: &[],
                                        depth_stencil_attachment: Some(
                                            wgpu::RenderPassDepthStencilAttachment {
                                                view: &depth_view,
                                                depth_ops: Some(wgpu::Operations {
                                                    load: wgpu::LoadOp::Clear(0.001),
                                                    store: wgpu::StoreOp::Store,
                                                }),
                                                stencil_ops: None,
                                            },
                                        ),
                                        ..Default::default()
                                    });
                                // The unfilled right half represents holes in a masked surface.
                                pass.set_pipeline(&depth_pipeline);
                                pass.set_scissor_rect(0, 0, 2, 4);
                                pass.draw(0..3, 0..1);
                            }
                            {
                                let mut pass = color_pass(
                                    &mut encoder,
                                    "motion visibility regression",
                                    &output_view,
                                );
                                pass.set_pipeline(&temporal.motion[usize::from(double_sided)]);
                                pass.set_bind_group(0, &group, &[]);
                                pass.set_vertex_buffer(0, vertices.slice(..));
                                pass.set_vertex_buffer(1, records.slice(..));
                                pass.set_index_buffer(
                                    indices[usize::from(reversed)].slice(..),
                                    wgpu::IndexFormat::Uint32,
                                );
                                if use_indirect {
                                    pass.multi_draw_indexed_indirect(&commands, 0, 3);
                                } else {
                                    let mut issued = 0;
                                    for first in [1, 2, 3] {
                                        let x = if first == 3 { 10.0 } else { 0.0 };
                                        let z = if first == 2 { 0.001_995 } else { 0.002 };
                                        let command = MotionCommand::Indexed {
                                            draw: super::super::instanced::Draw {
                                                item: 0,
                                                first: 0,
                                                count: 3,
                                                instances: super::super::instanced::Run {
                                                    first,
                                                    count: 1,
                                                },
                                            },
                                            double_sided,
                                            bounds: Some([
                                                Vec3::new(x - 1.0, -1.0, z),
                                                Vec3::new(x + 3.0, 3.0, z),
                                            ]),
                                        };
                                        if !cull || command.admitted(Mat4::IDENTITY) {
                                            pass.draw_indexed(0..3, 0, first..first + 1);
                                            issued += 1;
                                        }
                                    }
                                    assert_eq!(issued, if cull { 2 } else { 3 });
                                }
                            }
                            encoder.copy_texture_to_buffer(
                                wgpu::TexelCopyTextureInfo {
                                    texture: &output,
                                    mip_level: 0,
                                    origin: wgpu::Origin3d::ZERO,
                                    aspect: wgpu::TextureAspect::All,
                                },
                                wgpu::TexelCopyBufferInfo {
                                    buffer: &readback,
                                    layout: wgpu::TexelCopyBufferLayout {
                                        offset: 0,
                                        bytes_per_row: Some(256),
                                        rows_per_image: Some(4),
                                    },
                                },
                                size,
                            );
                            queue.submit([encoder.finish()]);
                            let (tx, rx) = std::sync::mpsc::channel();
                            readback
                                .slice(..)
                                .map_async(wgpu::MapMode::Read, move |result| {
                                    tx.send(result).unwrap();
                                });
                            device
                                .poll(wgpu::PollType::Wait {
                                    submission_index: None,
                                    timeout: None,
                                })
                                .unwrap();
                            rx.recv().unwrap().unwrap();
                            let data = readback.slice(..).get_mapped_range();
                            if cull {
                                assert_eq!(
                                    &*data,
                                    unfiltered.as_deref().unwrap(),
                                    "frustum admission must preserve every motion pixel"
                                );
                            } else {
                                unfiltered = Some(data.to_vec());
                            }
                            let texel = &data[256 + 8..256 + 16];
                            let x = half::f16::from_bits(u16::from_le_bytes(
                                texel[..2].try_into().unwrap(),
                            ))
                            .to_f32();
                            let valid = half::f16::from_bits(u16::from_le_bytes(
                                texel[6..8].try_into().unwrap(),
                            ))
                            .to_f32();
                            if reversed && !double_sided {
                                assert_eq!(
                                    valid, 0.0,
                                    "single-sided back faces must not write motion"
                                );
                            } else {
                                assert!(
                                    (x - 0.125).abs() < 1e-4,
                                    "{samples}x depth, indirect={use_indirect}, reversed={reversed}, double_sided={double_sided}, accepted hidden motion: {x}"
                                );
                                assert_eq!(valid, 1.0);
                            }
                            let hole = &data[256 + 3 * 8..256 + 4 * 8];
                            let hole_valid = half::f16::from_bits(u16::from_le_bytes(
                                hole[6..8].try_into().unwrap(),
                            ))
                            .to_f32();
                            assert_eq!(
                                hole_valid, 0.0,
                                "masked holes must retain camera reprojection"
                            );
                            drop(data);
                            readback.unmap();
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn bytes(width: u32, height: u32, samples: u32) -> u64 {
    // Two RGBA16 histories, RGBA16 motion, and resolved/sample-matched R8 markers.
    u64::from(width) * u64::from(height) * (25 + if samples > 1 { u64::from(samples) } else { 0 })
}

fn depth_source(source: &str, samples: u32) -> String {
    let (kind, load) = if samples > 1 {
        (
            "texture_depth_multisampled_2d",
            "var depth = 0.0; for (var k = 0; k < 4; k++) { depth = max(depth, textureLoad(scene_depth, p, k)); } return depth;",
        )
    } else {
        ("texture_depth_2d", "return textureLoad(scene_depth, p, 0);")
    };
    source
        .replace("// DEPTH_TYPE", &format!("alias DepthTexture = {kind};"))
        .replace("// DEPTH_LOAD", load)
        .replace(
            "// MOTION_FOOTPRINT",
            if samples > 1 {
                "const MOTION_FOOTPRINT = 0.375;"
            } else {
                "const MOTION_FOOTPRINT = 0.0;"
            },
        )
}

impl Temporal {
    pub fn new(device: &wgpu::Device, scene_format: wgpu::TextureFormat, samples: u32) -> Self {
        let uniform = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture = |binding, sample_type, multisampled| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled,
            },
            count: None,
        };
        let float = wgpu::TextureSampleType::Float { filterable: true };
        let resolve_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse temporal resolve"),
            entries: &[
                uniform,
                texture(1, float, false),
                texture(2, float, false),
                texture(3, wgpu::TextureSampleType::Depth, samples > 1),
                texture(4, float, false),
                texture(6, float, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sharpen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse temporal sharpen"),
            entries: &[uniform, texture(1, float, false)],
        });
        let motion_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse object motion"),
            entries: &[
                uniform,
                texture(1, wgpu::TextureSampleType::Depth, samples > 1),
            ],
        });
        let reactive_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse reactive lit geometry"),
            entries: &[uniform],
        });
        let make = |label,
                    source: String,
                    layout,
                    format: wgpu::TextureFormat,
                    vs,
                    fs,
                    buffers: &[wgpu::VertexBufferLayout<'_>],
                    cull_mode| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let resolve = make(
            "verse temporal resolve",
            depth_source(include_str!("temporal.wgsl"), samples),
            &resolve_layout,
            wgpu::TextureFormat::Rgba16Float,
            "vs",
            "fs",
            &[],
            None,
        );
        let sharpen = make(
            "verse temporal sharpen",
            include_str!("temporal_sharpen.wgsl").into(),
            &sharpen_layout,
            scene_format,
            "vs",
            "fs",
            &[],
            None,
        );
        const POSITION: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
        const AFFINE: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
        let motion = [false, true].map(|double_sided| {
            make(
                "verse object motion",
                depth_source(include_str!("temporal_motion.wgsl"), samples),
                &motion_layout,
                wgpu::TextureFormat::Rgba16Float,
                "vs",
                "fs",
                &[
                    wgpu::VertexBufferLayout {
                        array_stride: 28,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &POSITION,
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: 128,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &AFFINE,
                    },
                ],
                (!double_sided).then_some(wgpu::Face::Back),
            )
        });
        let reactive_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("verse reactive lit geometry"),
            source: wgpu::ShaderSource::Wgsl(include_str!("temporal_reactive.wgsl").into()),
        });
        let reactive_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("verse reactive lit geometry"),
                bind_group_layouts: &[Some(&reactive_layout)],
                immediate_size: 0,
            });
        let reactive = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("verse reactive lit geometry"),
            layout: Some(&reactive_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &reactive_shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<super::LitVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &POSITION,
                }],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Equal),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &reactive_shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::TextureFormat::R8Unorm.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("verse temporal history"),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            resolve_layout,
            sharpen_layout,
            motion_layout,
            reactive_layout,
            samples,
            resolve,
            sharpen,
            motion,
            reactive,
            sampler,
        }
    }

    pub fn targets(
        &self,
        device: &wgpu::Device,
        scene: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        size: [u32; 2],
    ) -> Targets {
        let texture = |label| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba16Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let history = [
            texture("verse temporal history A"),
            texture("verse temporal history B"),
        ];
        let motion = texture("verse temporal object motion");
        let marker = |samples, label| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let reactive = marker(1, "verse reactive lit visibility");
        let reactive_msaa =
            (self.samples > 1).then(|| marker(self.samples, "verse reactive lit samples"));
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse temporal camera"),
            size: std::mem::size_of::<Uniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view = |binding, texture| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(texture),
        };
        let camera = || wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        };
        let resolve = [0, 1].map(|write| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("verse temporal resolve"),
                layout: &self.resolve_layout,
                entries: &[
                    camera(),
                    view(1, scene),
                    view(2, &history[write ^ 1]),
                    view(3, depth),
                    view(4, &motion),
                    view(6, &reactive),
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            })
        });
        let sharpen = [0, 1].map(|write| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("verse temporal sharpen"),
                layout: &self.sharpen_layout,
                entries: &[camera(), view(1, &history[write])],
            })
        });
        let motion_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse object motion"),
            layout: &self.motion_layout,
            entries: &[camera(), view(1, depth)],
        });
        let reactive_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse reactive lit geometry"),
            layout: &self.reactive_layout,
            entries: &[camera()],
        });
        Targets {
            history,
            motion,
            reactive,
            reactive_msaa,
            reactive_group,
            depth: depth.clone(),
            resolve,
            sharpen,
            motion_group,
            uniform,
            size,
            write: 0,
            camera: CameraHistory::default(),
            enabled: false,
        }
    }

    pub fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        targets: &mut Targets,
        motions: &[MotionDraw<'_>],
        reactive: Option<ReactiveLit<'_>>,
    ) {
        if !targets.enabled {
            return;
        }
        let has_motion = motions.iter().any(|motion| motion.draw.has_vertices());
        let has_reactive = reactive.is_some_and(|lit| lit.admitted_ranges().next().is_some());
        let mut uniform = targets.camera.prepared;
        let camera = Mat4::from_cols_array_2d(&uniform.current);
        uniform.settings[3] = f32::from(u8::from(has_motion) + 2 * u8::from(has_reactive));
        queue.write_buffer(&targets.uniform, 0, bytemuck::bytes_of(&uniform));
        if has_motion {
            // An entirely clipped stream still clears the preceding frame's motion.
            let mut pass = color_pass(encoder, "verse object motion", &targets.motion);
            pass.set_bind_group(0, &targets.motion_group, &[]);
            let mut buffers = None;
            let mut sides = None;
            for motion in motions {
                if !motion.draw.admitted(camera) {
                    continue;
                }
                let double_sided = motion.draw.double_sided();
                if sides != Some(double_sided) {
                    pass.set_pipeline(&self.motion[usize::from(double_sided)]);
                    sides = Some(double_sided);
                }
                let current = (motion.vertices, motion.instances, motion.indices);
                if buffers != Some(current) {
                    pass.set_vertex_buffer(0, motion.vertices.slice(..));
                    pass.set_vertex_buffer(1, motion.instances.slice(..));
                    pass.set_index_buffer(motion.indices.slice(..), wgpu::IndexFormat::Uint32);
                    buffers = Some(current);
                }
                match &motion.draw {
                    MotionCommand::Indexed { draw, .. } => pass.draw_indexed(
                        draw.first..draw.first + draw.count,
                        0,
                        draw.instances.first..draw.instances.first + draw.instances.count,
                    ),
                    MotionCommand::Indirect {
                        buffer,
                        first,
                        count,
                        ..
                    } => {
                        pass.multi_draw_indexed_indirect(buffer, u64::from(*first) * 20, *count);
                    }
                }
            }
        }
        if let Some(lit) = reactive.filter(|_| has_reactive) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("verse reactive lit visibility"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: targets.reactive_msaa.as_ref().unwrap_or(&targets.reactive),
                    resolve_target: targets.reactive_msaa.as_ref().map(|_| &targets.reactive),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: if targets.reactive_msaa.is_some() {
                            wgpu::StoreOp::Discard
                        } else {
                            wgpu::StoreOp::Store
                        },
                    },
                })],
                // Read the exact samples written by the opaque color geometry.
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.reactive);
            pass.set_bind_group(0, &targets.reactive_group, &[]);
            pass.set_vertex_buffer(0, lit.vertices.slice(..));
            for range in lit.admitted_ranges() {
                pass.draw(range, 0..1);
            }
        }
        let write = targets.write;
        {
            let mut pass = color_pass(encoder, "verse temporal resolve", &targets.history[write]);
            pass.set_pipeline(&self.resolve);
            pass.set_bind_group(0, &targets.resolve[write], &[]);
            pass.draw(0..3, 0..1);
        }
        {
            let mut pass = color_pass(encoder, "verse temporal sharpen", scene);
            pass.set_pipeline(&self.sharpen);
            pass.set_bind_group(0, &targets.sharpen[write], &[]);
            pass.draw(0..3, 0..1);
        }
        targets.write ^= 1;
    }
}

fn color_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    label: &str,
    view: &wgpu::TextureView,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

#[cfg(test)]
#[path = "temporal_reactive_tests.rs"]
mod reactive_tests;
