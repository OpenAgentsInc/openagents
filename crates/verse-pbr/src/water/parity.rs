//! Surface parity: the shader's Gerstner displacement, evaluated on the GPU
//! by the same `water_gerstner` both water passes call, against
//! `physics::water` in `f64`.

use glam::DVec2;
use physics::water::{WaterBody, WaterId, WaveSet};

use super::frame::{Body, Water, WaterUniform};
use super::preset::Preset;

/// Points per draw: one pixel each.
const POINTS: usize = 16;

/// The parity shader: the shared water shader with stub hooks, and a
/// fragment that writes the displacement of point `x` into pixel `x`.
fn shader() -> String {
    format!(
        "{}\n{}",
        super::SHARED,
        r"
@group(0) @binding(0) var<uniform> water: WaterUniform;
@group(0) @binding(1) var<uniform> points: array<vec4<f32>, 16>;
@group(0) @binding(2) var water_tile: texture_2d<f32>;
@group(0) @binding(3) var water_tile_sampler: sampler;
fn water_host_control() -> vec4<f32> { return vec4<f32>(0.0, 1.0, 1.0, 1.0); }
fn water_host_sky(dir: vec3<f32>, level: f32) -> vec3<f32> { return vec3<f32>(0.0); }
fn water_host_sun() -> vec4<f32> { return vec4<f32>(0.0); }
fn water_host_sun_light() -> vec3<f32> { return vec3<f32>(0.0); }
fn water_host_light(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> vec3<f32> { return vec3<f32>(0.0); }
fn water_host_shadow(world: vec3<f32>, n: vec3<f32>, pixel: vec2<f32>) -> f32 { return 1.0; }
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}
@fragment fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let p0 = points[u32(at.x)].xy;
    // Deep water: the shallow-water factor is exactly one.
    let wave = water_gerstner(0u, p0, 1.0e6, 1.0);
    return vec4<f32>(wave.xyz, 1.0);
}
"
    )
}

/// The displacements the GPU computes for `points` under `water`, or none
/// without an adapter.
fn gpu_displacements(uniform: &WaterUniform, points: &[[f32; 4]; POINTS]) -> Option<Vec<[f32; 4]>> {
    use wgpu::util::DeviceExt;
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("water parity"),
        source: wgpu::ShaderSource::Wgsl(shader().into()),
    });
    let buffer = |label, bytes: &[u8]| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        })
    };
    let water_buffer = buffer("water parity terms", bytemuck::bytes_of(uniform));
    let point_buffer = buffer("water parity points", bytemuck::cast_slice(points));
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[super::uniform_entry(0), super::uniform_entry(1)],
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: water_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: point_buffer.as_entire_binding(),
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let format = wgpu::TextureFormat::Rgba32Float;
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("water parity"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(format.into())],
        }),
        multiview_mask: None,
        cache: None,
    });
    let size = wgpu::Extent3d {
        width: POINTS as u32,
        height: 1,
        depth_or_array_layers: 1,
    };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("water parity"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let row = 256u32;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(1),
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r)));
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        })
        .ok()?;
    rx.recv().ok()?.ok()?;
    let bytes = readback.slice(..).get_mapped_range();
    Some(bytemuck::cast_slice::<u8, [f32; 4]>(&bytes).to_vec())
}

/// The GPU's displacement of sixteen points, at ticks near and far along a
/// long-running clock, lands on `physics::water`'s exact displacement and
/// on `Surface::sample`'s height within a millimeter.
#[test]
fn the_shader_displacement_matches_the_physics_surface() {
    let set = WaveSet::seeded(
        11,
        1.0 / 120.0,
        120 * 600,
        8,
        DVec2::new(-0.4, 1.0),
        14.0,
        0.3,
        0.5,
    )
    .unwrap();
    let body = WaterBody::ocean(WaterId(0), 0.0).with_waves(set.clone());
    let mut points = [[0.0f32; 4]; POINTS];
    for (i, p) in points.iter_mut().enumerate() {
        let a = i as f32 * 2.399_963;
        let r = 3.0 + 9.0 * i as f32;
        *p = [r * a.cos(), r * a.sin(), 0.0, 0.0];
    }
    let mut worst = (0.0f64, 0.0f64);
    let mut highest = 0.0f64;
    let mut checked = 0;
    let mut water = Water::calm(0.0);
    water.bodies[0] = Body::from_physics(&body, &Preset::default());
    for tick in [0u64, 1, 4_321, 120 * 600 - 1, 37_000_017] {
        let Some(gpu) = gpu_displacements(&water.uniform_at_tick(tick), &points) else {
            eprintln!("No GPU adapter; the parity check did not run");
            return;
        };
        let phases = set.phases(tick);
        for (p, g) in points.iter().zip(&gpu) {
            let p0 = DVec2::new(f64::from(p[0]), f64::from(p[1]));
            let exact = set.at_rest(p0, &phases);
            let horizontal =
                (DVec2::new(f64::from(g[0]), f64::from(g[2])) - exact.horizontal).length();
            let height = (f64::from(g[1]) - exact.height).abs();
            let at = p0 + DVec2::new(f64::from(g[0]), f64::from(g[2]));
            let sample = body.surface().sample(at.x, at.y, tick).unwrap();
            let sampled = (f64::from(g[1]) - sample.height).abs();
            worst.0 = worst.0.max(horizontal.max(height));
            worst.1 = worst.1.max(sampled);
            highest = highest.max(exact.height.abs());
            assert!(g.iter().all(|v| v.is_finite()), "{g:?}");
            checked += 1;
        }
    }
    eprintln!(
        "water parity: {checked} points with crests up to {highest:.3} m, worst {:.2} µm against \
         at_rest and {:.2} µm against Surface::sample",
        worst.0 * 1e6,
        worst.1 * 1e6
    );
    assert!(highest > 0.1, "the waves move the surface");
    assert!(worst.0 < 1e-3 && worst.1 < 1e-3, "{worst:?}");
}
