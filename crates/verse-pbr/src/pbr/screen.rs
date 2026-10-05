//! High-tier screen-space detail: ambient occlusion and contact shadows.
//!
//! On the high quality tier ([`verse_engine::quality::Quality::screen_space`])
//! a physical frame draws its opaque and masked geometry into a
//! single-sample depth buffer before the scene ([`super::gpu`] owns that
//! prepass). This module's two passes then read that depth: `fs_trace` in
//! `screen.wgsl` traces a two-direction GTAO and the light's contact shadow
//! at half resolution, and `fs_resolve` blurs both back to full resolution
//! with weights that keep them on their own surface. The scene's lit and
//! textured shaders read the result through `screen_terms` in `photo.wgsl`:
//! the occlusion scales ambient light only, and the contact shadow scales
//! the sun's or key light's direct light only.
//!
//! Lower tiers create none of this. Their shaders read a constant white
//! texel instead, which multiplies their light by exactly one.
//!
//! [`verse_engine::render_graph::PhotoPlan`] declares the passes and their
//! order.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use super::gpu::DEPTH;

/// The trace and resolve targets: red ambient occlusion, green contact
/// shadow.
pub const OCCLUSION: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Unorm;

/// The occlusion search radius, m.
pub const AO_RADIUS: f32 = 0.6;
/// The largest screen radius the search covers, in full-resolution pixels,
/// so a surface close to the camera does not search half the screen.
pub const AO_MAX_PIXELS: f32 = 96.0;
/// The fraction of the radius over which a distant occluder fades out.
pub const AO_FALLOFF: f32 = 0.4;
/// The contact shadow ray's length, m: long enough to reach under a prop or
/// a foot, short enough to leave larger shadows to the shadow map.
pub const CONTACT_LENGTH: f32 = 0.3;
/// How far behind the depth buffer a ray sample may lie and still count as
/// blocked, m.
pub const CONTACT_THICKNESS: f32 = 0.08;
/// The ray starts this far along the surface normal, m.
pub const CONTACT_BIAS: f32 = 0.01;

/// The `Screen` uniform in `screen.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ScreenUniform {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    light: [f32; 4],
    size: [f32; 4],
    ao: [f32; 4],
    contact: [f32; 4],
}

impl ScreenUniform {
    /// The uniform for a frame drawn with reversed-depth `view_proj` from
    /// `eye` at `size` pixels. `light` points toward the light that casts
    /// contact shadows, when one does.
    pub fn new(view_proj: Mat4, eye: Vec3, light: Option<Vec3>, size: [u32; 2]) -> Self {
        let width = size[0].max(1) as f32;
        let height = size[1].max(1) as f32;
        let light = light
            .filter(|l| l.length_squared() > 0.0)
            .map_or([0.0; 4], |l| l.normalize().extend(1.0).to_array());
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            eye: eye.extend(1.0).to_array(),
            light,
            size: [width, height, 1.0 / width, 1.0 / height],
            ao: [AO_RADIUS, AO_MAX_PIXELS, AO_FALLOFF, 0.0],
            contact: [CONTACT_LENGTH, CONTACT_THICKNESS, CONTACT_BIAS, 0.0],
        }
    }
}

/// The trace and resolve pipelines and their uniform.
pub struct ScreenGpu {
    uniform: wgpu::Buffer,
    trace_layout: wgpu::BindGroupLayout,
    resolve_layout: wgpu::BindGroupLayout,
    trace: wgpu::RenderPipeline,
    resolve: wgpu::RenderPipeline,
}

/// Size-dependent screen-space targets.
pub struct ScreenTargets {
    /// The prepass depth: full resolution, single sample, readable.
    pub depth: wgpu::TextureView,
    traced: wgpu::TextureView,
    /// The resolved terms the scene reads, at full resolution.
    pub occlusion: wgpu::TextureView,
    trace_group: wgpu::BindGroup,
    resolve_group: wgpu::BindGroup,
}

impl ScreenGpu {
    pub fn new(device: &wgpu::Device) -> Self {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verse screen"),
            size: std::mem::size_of::<ScreenUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_entry = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let depth_entry = wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let traced_entry = wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let trace_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse screen trace"),
            entries: &[uniform_entry, depth_entry],
        });
        let resolve_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("verse screen resolve"),
            entries: &[uniform_entry, depth_entry, traced_entry],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("verse screen"),
            source: wgpu::ShaderSource::Wgsl(include_str!("screen.wgsl").into()),
        });
        let pipeline = |layout: &wgpu::BindGroupLayout, label: &str, fs: &str| {
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
                    targets: &[Some(wgpu::ColorTargetState {
                        format: OCCLUSION,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let trace = pipeline(&trace_layout, "verse screen trace", "fs_trace");
        let resolve = pipeline(&resolve_layout, "verse screen resolve", "fs_resolve");
        Self {
            uniform,
            trace_layout,
            resolve_layout,
            trace,
            resolve,
        }
    }

    /// The prepass depth, the half-resolution trace, and the resolved
    /// terms for a `width` by `height` frame.
    pub fn targets(&self, device: &wgpu::Device, width: u32, height: u32) -> ScreenTargets {
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let texture = |label, format, w: u32, h: u32| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: w.max(1),
                        height: h.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let depth = texture("verse depth prepass", DEPTH, width, height);
        let traced = texture(
            "verse screen trace",
            OCCLUSION,
            width.div_ceil(2),
            height.div_ceil(2),
        );
        let occlusion = texture("verse screen occlusion", OCCLUSION, width, height);
        let entry = |binding, view| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        };
        let uniform = wgpu::BindGroupEntry {
            binding: 0,
            resource: self.uniform.as_entire_binding(),
        };
        let trace_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse screen trace"),
            layout: &self.trace_layout,
            entries: &[uniform.clone(), entry(1, &depth)],
        });
        let resolve_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("verse screen resolve"),
            layout: &self.resolve_layout,
            entries: &[uniform, entry(1, &depth), entry(2, &traced)],
        });
        ScreenTargets {
            depth,
            traced,
            occlusion,
            trace_group,
            resolve_group,
        }
    }

    /// Traces and resolves from the prepass depth, which must be drawn.
    pub fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        targets: &ScreenTargets,
        uniform: &ScreenUniform,
    ) {
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(uniform));
        for (label, view, pipeline, group) in [
            (
                "verse screen trace",
                &targets.traced,
                &self.trace,
                &targets.trace_group,
            ),
            (
                "verse screen resolve",
                &targets.occlusion,
                &self.resolve,
                &targets.resolve_group,
            ),
        ] {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{IVec2, Vec2, Vec4};

    /// Samples on each side of each of the two directions, and along the
    /// contact ray: the loop counts `screen.wgsl` fixes.
    const AO_STEPS: i32 = 6;
    const CONTACT_STEPS: i32 = 12;

    /// `screen.wgsl` validates and translates to Metal, its loop counts match
    /// the CPU form below, and its uniform has the Rust struct's size.
    #[test]
    fn the_screen_shader_validates_and_matches_its_uniform() {
        let source = include_str!("screen.wgsl");
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("screen.wgsl: {}", e.emit_to_string(source)));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("screen.wgsl: {}", e.emit_to_string(source)));
        naga::back::msl::write_string(
            &module,
            &info,
            &naga::back::msl::Options {
                lang_version: (2, 3),
                ..Default::default()
            },
            &naga::back::msl::PipelineOptions::default(),
        )
        .unwrap_or_else(|e| panic!("screen.wgsl: {e}"));
        let entries: Vec<_> = module
            .entry_points
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(entries, ["vs_fullscreen", "fs_trace", "fs_resolve"]);
        assert!(source.contains(&format!("const AO_STEPS: i32 = {AO_STEPS};")));
        assert!(source.contains(&format!("const CONTACT_STEPS: i32 = {CONTACT_STEPS};")));
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).unwrap();
        let (screen, _) = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some("Screen"))
            .expect("screen.wgsl declares Screen");
        assert_eq!(
            layouter[screen].size as usize,
            std::mem::size_of::<ScreenUniform>()
        );
    }

    /// A synthetic depth buffer, rendered from planes by ray casting, and
    /// the CPU form of `fs_trace`'s two terms at full resolution.
    struct Depth {
        width: i32,
        height: i32,
        values: Vec<f32>,
        view_proj: Mat4,
        inv: Mat4,
        eye: Vec3,
    }

    impl Depth {
        /// Planes `n · x = c`, seen from `eye` toward `target`.
        fn render(planes: &[(Vec3, f32)], eye: Vec3, target: Vec3) -> Self {
            let (width, height) = (128, 96);
            let projection = Mat4::perspective_rh(1.0, width as f32 / height as f32, 0.1, 100.0);
            let view_proj = super::super::gpu::reversed_depth()
                * projection
                * Mat4::look_at_rh(eye, target, Vec3::Y);
            let mut depth = Self {
                width,
                height,
                values: vec![0.0; (width * height) as usize],
                view_proj,
                inv: view_proj.inverse(),
                eye,
            };
            for y in 0..height {
                for x in 0..width {
                    let far = depth.world_at(Vec2::new(x as f32, y as f32), 0.0);
                    let ray = (far - eye).normalize();
                    let hit = planes
                        .iter()
                        .filter_map(|&(n, c)| {
                            let den = n.dot(ray);
                            (den.abs() > 1e-9)
                                .then(|| (c - n.dot(eye)) / den)
                                .filter(|&t| t > 0.0)
                        })
                        .reduce(f32::min);
                    if let Some(t) = hit {
                        let clip = view_proj * (eye + ray * t).extend(1.0);
                        depth.values[(y * width + x) as usize] = clip.z / clip.w;
                    }
                }
            }
            depth
        }

        fn load(&self, p: IVec2) -> f32 {
            let x = p.x.clamp(0, self.width - 1);
            let y = p.y.clamp(0, self.height - 1);
            self.values[(y * self.width + x) as usize]
        }

        fn inside(&self, p: IVec2) -> bool {
            p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height
        }

        fn world_at(&self, p: Vec2, d: f32) -> Vec3 {
            let uv = (p + 0.5) / Vec2::new(self.width as f32, self.height as f32);
            let w = self.inv * Vec4::new(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
            w.truncate() / w.w
        }

        fn project(&self, world: Vec3) -> Vec3 {
            let c = self.view_proj * world.extend(1.0);
            if c.w <= 1e-6 {
                return Vec3::new(-1.0, -1.0, 0.0);
            }
            let ndc = c.truncate() / c.w;
            Vec3::new(
                (ndc.x * 0.5 + 0.5) * self.width as f32,
                (0.5 - ndc.y * 0.5) * self.height as f32,
                ndc.z,
            )
        }

        fn toward(&self, p: IVec2, offset: IVec2, center: Vec3) -> Vec3 {
            let q = p + offset;
            if !self.inside(q) || self.load(q) <= 0.0 {
                return Vec3::splat(1e9);
            }
            self.world_at(q.as_vec2(), self.load(q)) - center
        }

        fn normal_at(&self, p: IVec2, center: Vec3) -> Vec3 {
            let right = self.toward(p, IVec2::new(1, 0), center);
            let left = -self.toward(p, IVec2::new(-1, 0), center);
            let down = self.toward(p, IVec2::new(0, 1), center);
            let up = -self.toward(p, IVec2::new(0, -1), center);
            let dx = if right.dot(right) <= left.dot(left) {
                right
            } else {
                left
            };
            let dy = if down.dot(down) <= up.dot(up) {
                down
            } else {
                up
            };
            let to_eye = self.eye - center;
            let n = dy.cross(dx);
            if n.dot(n) < 1e-20 {
                return to_eye.normalize();
            }
            let unit = n.normalize();
            if unit.dot(to_eye) >= 0.0 { unit } else { -unit }
        }

        fn occlusion(&self, p: IVec2, noise: f32) -> f32 {
            use std::f32::consts::FRAC_PI_2;
            let arc = |h: f32, n: f32| 0.25 * (n.cos() + 2.0 * h * n.sin() - (2.0 * h - n).cos());
            let d = self.load(p);
            if d <= 0.0 {
                return 1.0;
            }
            let center = self.world_at(p.as_vec2(), d);
            let n = self.normal_at(p, center);
            let v = (self.eye - center).normalize();
            let side_axis = if v.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
            let across = v.cross(side_axis).normalize();
            let radius_px = (self.project(center + across * AO_RADIUS).truncate()
                - self.project(center).truncate())
            .length()
            .min(AO_MAX_PIXELS);
            if radius_px < 1.0 {
                return 1.0;
            }
            let stride = radius_px / AO_STEPS as f32;
            let size = Vec2::new(self.width as f32, self.height as f32);
            let (mut seen, mut open) = (0.0, 0.0);
            for slice in 0..2 {
                let phi = (slice as f32 + noise) * FRAC_PI_2;
                let dir = Vec2::new(phi.cos(), phi.sin());
                let along = self.world_at(p.as_vec2() + dir * 4.0, d) - center;
                let along = (along - v * along.dot(v)).normalize();
                let axis = along.cross(v).normalize();
                let projected = n - axis * n.dot(axis);
                let len = projected.length();
                if len < 1e-4 {
                    continue;
                }
                let cos_n = (projected.dot(v) / len).clamp(-1.0, 1.0);
                let angle = (if along.dot(projected) >= 0.0 {
                    1.0
                } else {
                    -1.0
                }) * cos_n.acos();
                let mut horizons = [0.0; 2];
                for (side, horizon_out) in horizons.iter_mut().enumerate() {
                    let heading = 1.0 - 2.0 * side as f32;
                    let low = (angle + heading * FRAC_PI_2).cos();
                    let mut horizon = low;
                    for k in 0..AO_STEPS {
                        let t = 1.0 + (k as f32 + noise) * stride;
                        let q = (p.as_vec2() + 0.5 + dir * heading * t).floor();
                        if q.x < 0.0 || q.y < 0.0 || q.x >= size.x || q.y >= size.y {
                            break;
                        }
                        let qd = self.load(q.as_ivec2());
                        if qd <= 0.0 {
                            continue;
                        }
                        let delta = self.world_at(q, qd) - center;
                        let dist = delta.length();
                        if dist < 1e-4 {
                            continue;
                        }
                        let weight =
                            ((AO_RADIUS - dist) / (AO_RADIUS * AO_FALLOFF)).clamp(0.0, 1.0);
                        let cos = delta.dot(v) / dist;
                        horizon = horizon.max(low + (cos - low) * weight);
                    }
                    *horizon_out = horizon;
                }
                let h1 = angle + (horizons[0].clamp(-1.0, 1.0).acos() - angle).min(FRAC_PI_2);
                let h0 = angle + (-horizons[1].clamp(-1.0, 1.0).acos() - angle).max(-FRAC_PI_2);
                seen += len * (arc(h0, angle) + arc(h1, angle));
                open += len * (arc(angle - FRAC_PI_2, angle) + arc(angle + FRAC_PI_2, angle));
            }
            if open <= 1e-6 {
                return 1.0;
            }
            (seen / open).clamp(0.0, 1.0)
        }

        fn range_at(&self, q: IVec2, fallback: f32) -> f32 {
            if !self.inside(q) || self.load(q) <= 0.0 {
                return fallback;
            }
            self.world_at(q.as_vec2(), self.load(q)).distance(self.eye)
        }

        fn contact(&self, p: IVec2, light: Vec3, noise: f32) -> f32 {
            let d = self.load(p);
            if d <= 0.0 {
                return 1.0;
            }
            let center = self.world_at(p.as_vec2(), d);
            let n = self.normal_at(p, center);
            let l = light.normalize();
            if n.dot(l) <= 0.0 {
                return 1.0;
            }
            let start = center + n * CONTACT_BIAS;
            for k in 0..CONTACT_STEPS {
                let t = (k as f32 + noise) / CONTACT_STEPS as f32 * CONTACT_LENGTH;
                let marched = start + l * t;
                let q = self.project(marched);
                if q.x < 0.0
                    || q.y < 0.0
                    || q.x >= self.width as f32
                    || q.y >= self.height as f32
                    || q.z <= 0.0
                {
                    break;
                }
                let qi = q.truncate().floor().as_ivec2();
                let qd = self.load(qi);
                if qd <= q.z {
                    continue;
                }
                let r0 = self.world_at(qi.as_vec2(), qd).distance(self.eye);
                let gap = marched.distance(self.eye) - r0;
                let slope = |o: IVec2| {
                    (self.range_at(qi + o, r0) - r0)
                        .abs()
                        .min((self.range_at(qi - o, r0) - r0).abs())
                };
                let slope = slope(IVec2::X).max(slope(IVec2::Y));
                if gap > slope.max(0.001) && gap < CONTACT_THICKNESS {
                    return ((t / CONTACT_LENGTH - 0.75) * 4.0).clamp(0.0, 1.0);
                }
            }
            1.0
        }

        fn pixel(&self, world: Vec3) -> IVec2 {
            self.project(world).truncate().floor().as_ivec2()
        }
    }

    const EYE: Vec3 = Vec3::new(0.0, 1.5, 3.0);
    const TARGET: Vec3 = Vec3::new(0.0, 0.0, -0.9);
    const NOISE: [f32; 4] = [0.0, 0.25, 0.5, 0.75];
    const FLOOR: (Vec3, f32) = (Vec3::Y, 0.0);
    /// A wall across the floor at z = −1, facing the camera.
    const WALL: (Vec3, f32) = (Vec3::Z, -1.0);

    #[test]
    fn a_flat_floor_is_unoccluded_and_casts_no_contact_shadow() {
        let depth = Depth::render(&[FLOOR], EYE, TARGET);
        let lights = [
            Vec3::new(0.0, 0.5, -1.0),
            Vec3::new(0.3, 1.0, 0.2),
            Vec3::new(1.0, 0.2, 0.0),
            Vec3::new(0.0, 0.3, 1.0),
            Vec3::new(-1.0, 0.1, 0.3),
        ];
        let mut floor_pixels = 0;
        for y in (0..depth.height).step_by(3) {
            for x in (0..depth.width).step_by(5) {
                let p = IVec2::new(x, y);
                if depth.load(p) <= 0.0 {
                    continue;
                }
                floor_pixels += 1;
                let ao = depth.occlusion(p, 0.37);
                assert!(ao > 0.95, "{p}: {ao}");
                for light in lights {
                    assert_eq!(depth.contact(p, light, 0.37), 1.0, "{p} toward {light}");
                }
            }
        }
        assert!(floor_pixels > 300, "{floor_pixels}");
        let open = depth.pixel(Vec3::new(0.0, 0.0, 0.5));
        for noise in NOISE {
            assert!(depth.occlusion(open, noise) > 0.97);
        }
    }

    #[test]
    fn a_corner_darkens_the_floor_at_the_wall_base() {
        let depth = Depth::render(&[FLOOR, WALL], EYE, TARGET);
        let base = depth.pixel(Vec3::new(0.0, 0.0, -0.9));
        let center = depth.world_at(base.as_vec2(), depth.load(base));
        // The pixel is floor beside the wall, and its normal is the floor's.
        assert!(
            center.y.abs() < 1e-3 && center.z > -1.0 && center.z < -0.8,
            "{center}"
        );
        assert!(depth.normal_at(base, center).y > 0.99);
        let ao: f32 = NOISE.iter().map(|&n| depth.occlusion(base, n)).sum::<f32>() / 4.0;
        assert!(ao < 0.9, "{ao}");
        // The same floor point without the wall is open.
        let open = Depth::render(&[FLOOR], EYE, TARGET);
        let clear: f32 = NOISE.iter().map(|&n| open.occlusion(base, n)).sum::<f32>() / 4.0;
        assert!(clear > 0.97 && ao < clear - 0.08, "{ao} {clear}");
        // A low light behind the wall: the floor at its base is in contact
        // shadow; with the light behind the camera it is not.
        let behind_wall = Vec3::new(0.0, 0.5, -1.0);
        let shadow: f32 = NOISE
            .iter()
            .map(|&n| depth.contact(base, behind_wall, n))
            .sum::<f32>()
            / 4.0;
        assert!(shadow < 0.5, "{shadow}");
        for noise in NOISE {
            assert_eq!(depth.contact(base, Vec3::new(0.0, 0.5, 1.0), noise), 1.0);
        }
    }

    #[test]
    fn the_uniform_carries_the_light_only_when_one_casts() {
        let view_proj = Mat4::IDENTITY;
        let lit = ScreenUniform::new(
            view_proj,
            Vec3::ZERO,
            Some(Vec3::new(0.0, 2.0, 0.0)),
            [4, 2],
        );
        assert_eq!(lit.light, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(lit.size, [4.0, 2.0, 0.25, 0.5]);
        let unlit = ScreenUniform::new(view_proj, Vec3::ZERO, None, [4, 2]);
        assert_eq!(unlit.light[3], 0.0);
        let zero = ScreenUniform::new(view_proj, Vec3::ZERO, Some(Vec3::ZERO), [0, 0]);
        assert_eq!(zero.light[3], 0.0);
        assert_eq!(zero.size, [1.0, 1.0, 1.0, 1.0]);
    }
}
