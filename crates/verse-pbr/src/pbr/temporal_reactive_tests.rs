use super::*;
use wgpu::util::DeviceExt;

const WIDTH: u32 = 32;
const HEIGHT: u32 = 8;
const ORANGE: [f32; 4] = [1.0, 0.3, 0.05, 1.0];
const ROCK: [f32; 4] = [0.05, 0.03, 0.01, 1.0];

struct Fixture<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    temporal: Temporal,
    targets: Targets,
    scene: wgpu::Texture,
    scene_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    depth_pipeline: wgpu::RenderPipeline,
    depth_group: wgpu::BindGroup,
    lit_camera: wgpu::Buffer,
    current: Mat4,
    readback: wgpu::Buffer,
    previous: Mat4,
    sharpen: f32,
}

fn native_device() -> (wgpu::Device, wgpu::Queue) {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    pollster::block_on(adapter.request_device(&Default::default())).unwrap()
}

fn quad(x0: f32, y0: f32, x1: f32, y1: f32, depth: f32) -> Vec<super::super::LitVertex> {
    let point = |x: f32, y: f32| super::super::LitVertex {
        pos: [
            2.0 * x / WIDTH as f32 - 1.0,
            1.0 - 2.0 * y / HEIGHT as f32,
            depth,
        ],
        ..super::super::LitVertex::zeroed()
    };
    let [a, b, c, d] = [point(x0, y0), point(x1, y0), point(x1, y1), point(x0, y1)];
    vec![a, b, c, a, c, d]
}

impl<'a> Fixture<'a> {
    fn new(device: &'a wgpu::Device, queue: &'a wgpu::Queue, samples: u32) -> Self {
        let size = wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        };
        let scene = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("reactive test scene"),
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
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("reactive test physical depth"),
            size,
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let scene_view = scene.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let temporal = Temporal::new(device, wgpu::TextureFormat::Rgba16Float, samples);
        let targets = temporal.targets(device, &scene_view, &depth_view, [WIDTH, HEIGHT]);

        // Exercise the production lit vertex shader, rather than duplicate its projection.
        let shared = crate::shading::source(include_str!("photo.wgsl"));
        let source = verse_gfx::gles::wgsl(&shared, false);
        let module = naga::front::wgsl::parse_str(&source).unwrap();
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).unwrap();
        let frame = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some("Frame"))
            .unwrap()
            .0;
        let mut frame_bytes = vec![0_u8; layouter[frame].size as usize];
        frame_bytes[..64].copy_from_slice(bytemuck::bytes_of(&Mat4::IDENTITY.to_cols_array_2d()));
        let camera = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("reactive test lit camera"),
            contents: &frame_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let depth_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("reactive test production lit vertex"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let depth_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("reactive test opaque depth"), layout: None,
            vertex: wgpu::VertexState {
                module: &depth_shader, entry_point: Some("vs_lit"), compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<super::super::LitVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3, 4 => Float32x3, 5 => Float32x4],
                }],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float, depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual), stencil: Default::default(), bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
            fragment: None, multiview_mask: None, cache: None,
        });
        let depth_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &depth_pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(HEIGHT) * 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            device,
            queue,
            temporal,
            targets,
            scene,
            scene_view,
            depth_view,
            depth_pipeline,
            depth_group,
            lit_camera: camera,
            current: Mat4::IDENTITY,
            readback,
            previous: Mat4::IDENTITY,
            sharpen: 0.0,
        }
    }

    fn pixels(&self, encoder: wgpu::CommandEncoder) -> Vec<[f32; 4]> {
        let mut encoder = encoder;
        encoder.copy_texture_to_buffer(
            self.scene.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(HEIGHT),
                },
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        self.readback
            .map_async(wgpu::MapMode::Read, .., |r| r.unwrap());
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let data = self.readback.get_mapped_range(..);
        let result = data
            .chunks_exact(8)
            .map(|pixel| {
                std::array::from_fn(|i| {
                    half::f16::from_bits(u16::from_le_bytes([pixel[i * 2], pixel[i * 2 + 1]]))
                        .to_f32()
                })
            })
            .collect();
        drop(data);
        self.readback.unmap();
        result
    }

    fn render(
        &mut self,
        pixels: &[[f32; 4]],
        vertices: &[super::super::LitVertex],
        ranges: &[std::ops::Range<u32>],
        valid: bool,
    ) -> Vec<[f32; 4]> {
        let bits: Vec<u16> = pixels
            .iter()
            .flatten()
            .map(|&v| half::f16::from_f32(v).to_bits())
            .collect();
        self.queue.write_texture(
            self.scene.as_image_copy(),
            bytemuck::cast_slice(&bits),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WIDTH * 8),
                rows_per_image: Some(HEIGHT),
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        self.queue.write_buffer(
            &self.lit_camera,
            0,
            bytemuck::bytes_of(&self.current.to_cols_array_2d()),
        );
        let world = |vertex: &super::super::LitVertex| super::super::LitVertex {
            pos: self
                .current
                .inverse()
                .project_point3(Vec3::from(vertex.pos))
                .to_array(),
            ..*vertex
        };
        let vertices: Vec<_> = vertices.iter().map(world).collect();
        let mut geometry: Vec<_> = quad(0.0, 0.0, WIDTH as f32, HEIGHT as f32, 0.5)
            .iter()
            .map(world)
            .collect();
        geometry.extend_from_slice(&vertices);
        let opaque = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&geometry),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let reactive = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(if vertices.is_empty() {
                    &geometry[..1]
                } else {
                    &vertices
                }),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.depth_pipeline);
            pass.set_bind_group(0, &self.depth_group, &[]);
            pass.set_vertex_buffer(0, opaque.slice(..));
            pass.draw(0..geometry.len() as u32, 0..1);
        }
        self.targets.enabled = true;
        self.targets.camera.prepared = Uniform {
            current: self.current.to_cols_array_2d(),
            inverse: self.current.inverse().to_cols_array_2d(),
            previous: self.previous.to_cols_array_2d(),
            size: [
                WIDTH as f32,
                HEIGHT as f32,
                1.0 / WIDTH as f32,
                1.0 / HEIGHT as f32,
            ],
            settings: [f32::from(u8::from(valid)), 0.9, self.sharpen, 0.0],
        };
        self.temporal.encode(
            self.queue,
            &mut encoder,
            &self.scene_view,
            &mut self.targets,
            &[],
            Some(ReactiveLit {
                vertices: &reactive,
                count: vertices.len() as u32,
                ranges,
            }),
        );
        self.pixels(encoder)
    }

    fn marker(&self) -> Vec<[f32; 4]> {
        let source = "@group(0) @binding(0) var marker: texture_2d<f32>;
            @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                return vec4<f32>(f32((i << 1u) & 2u) * 2.0 - 1.0, f32(i & 2u) * 2.0 - 1.0, 0.0, 1.0);
            }
            @fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
                return vec4<f32>(textureLoad(marker, vec2<i32>(p.xy), 0).r, 0.0, 0.0, 1.0);
            }";
        let pipeline = flat_pipeline(self.device, source);
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.targets.reactive),
            }],
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = color_pass(&mut encoder, "read reactive coverage", &self.scene_view);
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.pixels(encoder)
    }

    fn history(&self) -> Vec<[f32; 4]> {
        let pipeline = flat_pipeline(
            self.device,
            "@group(0) @binding(0) var retained: texture_2d<f32>;
            @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                return vec4<f32>(f32((i << 1u) & 2u) * 2.0 - 1.0, f32(i & 2u) * 2.0 - 1.0, 0.0, 1.0);
            }
            @fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
                return textureLoad(retained, vec2<i32>(p.xy), 0);
            }",
        );
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &self.targets.history[self.targets.write ^ 1],
                ),
            }],
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = color_pass(
                &mut encoder,
                "read retained temporal history",
                &self.scene_view,
            );
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.pixels(encoder)
    }

    fn seed_history(&self, negative: [u32; 2]) {
        let pipeline = flat_pipeline(self.device, "
            @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                return vec4<f32>(f32((i << 1u) & 2u) * 2.0 - 1.0, f32(i & 2u) * 2.0 - 1.0, 0.0, 1.0);
            }
            @fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(0.25, 0.25, 0.25, -1.0); }");
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets.history[self.targets.write ^ 1],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.25,
                            g: 0.25,
                            b: 0.25,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&pipeline);
            pass.set_scissor_rect(negative[0], negative[1], 1, 1);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }
}

fn flat_pipeline(device: &wgpu::Device, source: &str) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba16Float.into())],
        }),
        multiview_mask: None,
        cache: None,
    })
}

#[test]
#[ignore = "Requires a native GPU; checks moving heads and exact visible MSAA samples"]
fn moving_reactive_geometry_cannot_leave_history_in_a_bright_trail() {
    let (device, queue) = native_device();
    for samples in [1, 4] {
        for marked in [false, true] {
            let mut fixture = Fixture::new(&device, &queue, samples);
            for (frame, rock) in [Some(6), Some(10), Some(14), None, None]
                .into_iter()
                .enumerate()
            {
                let mut pixels = vec![ORANGE; (WIDTH * HEIGHT) as usize];
                // Dark ribbon detail keeps the previous silhouette inside the clamp's neighborhood.
                pixels[(4 * WIDTH) as usize..(5 * WIDTH) as usize].fill(ROCK);
                let vertices = rock.map_or_else(Vec::new, |x| {
                    pixels[(3 * WIDTH + x) as usize] = ROCK;
                    quad(x as f32, 3.0, x as f32 + 1.0, 4.0, 0.5)
                });
                let ranges = if marked && rock.is_some() {
                    vec![0..6]
                } else {
                    vec![]
                };
                let rendered = fixture.render(&pixels, &vertices, &ranges, frame > 0);
                if frame > 0 {
                    let old = rendered[(3 * WIDTH + 6) as usize][0];
                    if marked {
                        assert!(
                            (old - ORANGE[0]).abs() < 0.002,
                            "{samples}x frame {frame}: old head retained {old}"
                        );
                    } else if frame <= 2 {
                        assert!(
                            old < 0.4,
                            "control must expose the older silhouette: {samples}x frame {frame} {old}"
                        );
                    }
                }
            }
        }
        let mut fixture = Fixture::new(&device, &queue, samples);
        let eye = Vec3::new(3.0, 2.0, 6.0);
        fixture.current = super::super::gpu::reversed_depth()
            * Mat4::perspective_rh(0.9, 4.0, 0.3, 200.0)
            * Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
        for (vertices, ranges, expected) in [
            (quad(10.0, 3.0, 11.0, 4.0, 0.6), vec![0..6], true),
            (quad(10.0, 3.0, 11.0, 4.0, 0.49), vec![0..6], false),
            (quad(-20.0, 3.0, -19.0, 4.0, 0.6), vec![0..6], false),
            (quad(10.0, 3.0, 11.0, 4.0, 0.6), vec![1..6, 0..9], false),
            // The pixel center is outside; only actual off-center MSAA samples may mark it.
            (quad(10.6, 3.0, 10.9, 4.0, 0.6), vec![0..6], samples == 4),
        ] {
            fixture.render(
                &vec![ORANGE; (WIDTH * HEIGHT) as usize],
                &vertices,
                &ranges,
                false,
            );
            let marker = fixture.marker();
            let coverage = marker[(3 * WIDTH + 10) as usize][0];
            assert_eq!(
                coverage > 0.0,
                expected,
                "{samples}x coverage {coverage}, ranges {ranges:?}"
            );
            if expected && samples == 4 && vertices[0].pos[0] > -0.34 {
                assert!(
                    coverage < 1.0,
                    "partial MSAA coverage must resolve fractionally"
                );
            }
        }
    }
}

#[test]
#[ignore = "Requires a native GPU; checks nonzero bilinear history taps"]
fn reactive_history_rejection_matches_the_actual_bilinear_footprint() {
    let (device, queue) = native_device();
    for samples in [1, 4] {
        let mut fixture = Fixture::new(&device, &queue, samples);
        let pixels: Vec<_> = (0..HEIGHT)
            .flat_map(|y| {
                (0..WIDTH).map(move |x| {
                    let value = ((x + y) % 2) as f32;
                    [value, value, value, 1.0]
                })
            })
            .collect();
        // Leave a real current marker behind, then verify empty streams ignore it.
        fixture.render(&pixels, &quad(10.0, 3.0, 11.0, 4.0, 0.6), &[0..6], false);
        for [dx, dy] in [
            [0.25_f32, 0.25],
            [-0.25, 0.25],
            [0.25, -0.25],
            [-0.25, -0.25],
            [0.0, 0.0],
        ] {
            let negative = [if dx < 0.0 { 9 } else { 11 }, if dy < 0.0 { 2 } else { 4 }];
            fixture.seed_history(negative);
            fixture.previous = Mat4::from_translation(Vec3::new(
                2.0 * dx / WIDTH as f32,
                -2.0 * dy / HEIGHT as f32,
                0.0,
            ));
            let rendered = fixture.render(&pixels, &[], &[], true);
            let value = rendered[(3 * WIDTH + 10) as usize][0];
            if dx == 0.0 && dy == 0.0 {
                assert!(
                    (value - 0.325).abs() < 0.002,
                    "zero-weight negative tap rejected valid history: {value}"
                );
            } else {
                assert!(
                    (value - 1.0).abs() < 0.002,
                    "{samples}x corner [{dx},{dy}] leaked reactive history: {value}"
                );
            }
        }
    }
}

#[test]
#[ignore = "Requires a native GPU; checks HDR head-dependent conditioning and retained history"]
fn a_reactive_head_cannot_seed_neighbor_history_through_color_conditioning() {
    let (device, queue) = native_device();
    const DIM_RIBBON: [f32; 4] = [2.0, 0.6, 0.1, 1.0];
    const BRIGHT_RIBBON: [f32; 4] = [8.0, 2.4, 0.4, 1.0];
    let center = (3 * WIDTH + 10) as usize;
    for samples in [1, 4] {
        let mut without_head = Fixture::new(&device, &queue, samples);
        without_head.sharpen = 0.15;
        without_head.render(
            &vec![DIM_RIBBON; (WIDTH * HEIGHT) as usize],
            &[],
            &[],
            false,
        );
        without_head.render(
            &vec![BRIGHT_RIBBON; (WIDTH * HEIGHT) as usize],
            &[],
            &[],
            true,
        );
        let narrow = without_head.history()[center];
        assert!(
            (narrow[0] - BRIGHT_RIBBON[0]).abs() < 0.01 && narrow[3] > 0.0,
            "{samples}x uniform current must condition the darker history: {narrow:?}"
        );

        for marked in [false, true] {
            let mut fixture = Fixture::new(&device, &queue, samples);
            fixture.sharpen = 0.15;
            fixture.render(
                &vec![DIM_RIBBON; (WIDTH * HEIGHT) as usize],
                &[],
                &[],
                false,
            );

            // The center is bright ribbon. Only its neighboring head changes the color bounds.
            let mut pixels = vec![BRIGHT_RIBBON; (WIDTH * HEIGHT) as usize];
            pixels[(3 * WIDTH + 11) as usize] = ROCK;
            let ranges = if marked { vec![0..6] } else { vec![] };
            fixture.render(&pixels, &quad(11.0, 3.0, 12.0, 4.0, 0.6), &ranges, true);
            let history = fixture.history()[center];
            let marker = fixture.marker();
            assert_eq!(
                marker[center][0], 0.0,
                "the observed ribbon is not head geometry"
            );
            assert_eq!(marker[(3 * WIDTH + 11) as usize][0] > 0.0, marked);
            if marked {
                assert!(
                    history[3] < 0.0,
                    "{samples}x head-dependent ribbon seeded retainable history: {history:?}"
                );
                // Reject the blend in this frame as well as later history.
                // Marking the conditioned halo negative cannot undo its visible RGB.
                for channel in 0..3 {
                    let expected = half::f16::from_f32(BRIGHT_RIBBON[channel]).to_f32();
                    assert_eq!(
                        history[channel], expected,
                        "{samples}x current ribbon retained a head-conditioned halo in channel {channel}: {history:?}"
                    );
                }
            } else {
                assert!(
                    history[3] > 0.0 && history[0] < narrow[0] - 4.0,
                    "{samples}x control must expose conditioning from the neighboring head: {history:?}"
                );
            }

            // The head moves away; darker ribbon detail keeps the old contour within the bounds.
            pixels = vec![BRIGHT_RIBBON; (WIDTH * HEIGHT) as usize];
            pixels[(4 * WIDTH) as usize..(5 * WIDTH) as usize].fill(ROCK);
            pixels[(3 * WIDTH + 14) as usize] = ROCK;
            let rendered = fixture.render(&pixels, &quad(14.0, 3.0, 15.0, 4.0, 0.6), &ranges, true);
            if marked {
                assert!(
                    (rendered[center][0] - BRIGHT_RIBBON[0]).abs() < 0.01,
                    "{samples}x displaced head left a ribbon contour after sharpening: {:?}",
                    rendered[center]
                );
            } else {
                assert!(
                    rendered[center][0] < BRIGHT_RIBBON[0] - 3.0,
                    "{samples}x control must retain the conditioned contour: {:?}",
                    rendered[center]
                );
            }
        }
    }
}
