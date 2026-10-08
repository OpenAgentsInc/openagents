//! Surface parity: the shader's Gerstner displacement and the spectral
//! sea's gameplay band, evaluated on the GPU by the same `water_gerstner`
//! and `water_ocean_move` both water passes call, against `physics::water`
//! in `f64`.

use glam::DVec2;
use physics::water::{Spectrum, WaterBody, WaterId, WaveSet};
use verse_engine::quality::Tier;

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
@group(0) @binding(4) var water_waves: texture_2d_array<f32>;
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
@fragment fn fs_ocean(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let p0 = points[u32(at.x)].xy;
    let wave = water_ocean_move(0u, p0, 1.0e6, 1.0, 0.0);
    return vec4<f32>(wave.xyz, 1.0);
}
@fragment fn fs_shelter(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = points[min(u32(pos.x), 15u)].xy;
    return vec4<f32>(water_shelter(0u, p), 1.0);
}

"
    )
}

/// The displacements the GPU computes for `points` under `water`, or none
/// without an adapter: the Gerstner terms', or with `ocean` the spectral
/// sea's gameplay band at that tick, synthesized and uploaded as the
/// renderers do.
fn gpu_displacements(
    uniform: &WaterUniform,
    points: &[[f32; 4]; POINTS],
    ocean: Option<(&Spectrum, u64, Tier)>,
    shelter: Option<(super::shelter::Shelter, Tier)>,
) -> Option<Vec<[f32; 4]>> {
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
    let tier = shelter.map_or_else(|| ocean.map_or(Tier::Medium, |o| o.2), |(_, tier)| tier);
    let mut cascades = super::OceanGpu::new(&device, tier);
    cascades.exact = true;
    let mut uniform = *uniform;
    uniform.shelter = cascades.prepare_shelter(&queue, shelter.map(|(mask, _)| mask));
    if let Some((spectrum, tick, _)) = ocean {
        uniform.ocean = cascades.prepare(&queue, Some(spectrum), tick as f64 * spectrum.tick, 1.0);
        // Cascade 0 alone: the band `physics::water` samples.
        uniform.ocean[3][1] = 1.0;
    }
    let uniform = &uniform;
    let sampler = super::tile::sampler(&device);
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
        entries: &[
            super::uniform_entry(0),
            super::uniform_entry(1),
            super::tile_entries(2)[1],
            super::ocean::entry(4),
        ],
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
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(cascades.view()),
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
            entry_point: Some(if shelter.is_some() {
                "fs_shelter"
            } else if ocean.is_some() {
                "fs_ocean"
            } else {
                "fs"
            }),
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
        let Some(gpu) = gpu_displacements(&water.uniform_at_tick(tick), &points, None, None) else {
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

/// The GPU's spectral displacement of sixteen points, from the cascades
/// the worker synthesizes and the shader samples, lands on the gameplay
/// band `physics::water` samples within a centimeter (half floats and the
/// texture filter's precision), for a calm and a storm sea, on Medium's
/// grid and on High's, where cascade 0 is the same grid doubled.
#[test]
fn the_shader_spectral_band_matches_the_physics_band() {
    let mut points = [[0.0f32; 4]; POINTS];
    for (i, p) in points.iter_mut().enumerate() {
        let a = i as f32 * 2.399_963;
        let r = 5.0 + 23.0 * i as f32;
        *p = [r * a.cos(), r * a.sin(), 0.0, 0.0];
    }
    let mut worst = 0.0f64;
    let mut highest = 0.0f64;
    for (name, tick, tier) in [
        ("calm", 9_001u64, Tier::Medium),
        ("storm", 123_457, Tier::Medium),
        ("storm", 77_777, Tier::High),
    ] {
        let spectrum = super::SeaState::named(name)
            .unwrap()
            .spectrum(0.3, 0x5EA, 30.0);
        let Some(gpu) = gpu_displacements(
            &Water::calm(0.0).uniform(),
            &points,
            Some((&spectrum, tick, tier)),
            None,
        ) else {
            eprintln!("No GPU adapter; the spectral parity check did not run");
            return;
        };
        let field = physics::water::spectrum::field(&spectrum, tick).unwrap();
        for (p, g) in points.iter().zip(&gpu) {
            let exact = field.sample(DVec2::new(f64::from(p[0]), f64::from(p[1])));
            for k in 0..3 {
                worst = worst.max((f64::from(g[k]) - exact[k]).abs());
            }
            highest = highest.max(exact[1].abs());
        }
    }
    eprintln!(
        "spectral parity: crests up to {highest:.3} m, worst {:.2} mm",
        worst * 1e3
    );
    assert!(highest > 0.5, "the storm moves the surface");
    assert!(worst < 0.01, "{worst}");
}

/// The uploaded mask keeps the CPU's sheltered center, open-water border,
/// and transition gradient. Run this GPU check on a remote GPU host.
#[test]
fn the_shader_shelter_mask_matches_the_cpu_field() {
    let mask = super::shelter::Shelter {
        center: [-130.0, -230.0],
        inner: 55.0,
        outer: 95.0,
        gain: 0.1,
    };
    let points =
        std::array::from_fn(|i| [mask.center[0] + i as f32 * 8.0, mask.center[1], 0.0, 0.0]);
    for tier in [Tier::Low, Tier::Medium, Tier::High] {
        let gpu = gpu_displacements(
            &Water::calm(0.0).uniform(),
            &points,
            None,
            Some((mask, tier)),
        )
        .expect("the shelter qualification requires a GPU adapter");
        let mut worst = [0.0_f32; 3];
        for (point, sampled) in points.iter().zip(gpu) {
            let expected = mask.sample(glam::Vec2::new(point[0], point[1]));
            for axis in 0..3 {
                worst[axis] = worst[axis].max((sampled[axis] - expected[axis]).abs());
            }
        }
        // A 64² mask spans 198 m: bilinear filtering across the cubic
        // transition differs from the analytic curve, especially at its
        // flat-to-curved boundary. These bounds include half-float rounding.
        eprintln!(
            "{tier:?} shelter error: gain {}, gradient {:?}",
            worst[0],
            &worst[1..]
        );
        assert!(worst[0] < 0.004, "{tier:?}: {worst:?}");
        assert!(
            worst[1] < 0.0012 && worst[2] < 0.0012,
            "{tier:?}: {worst:?}"
        );
    }
}
