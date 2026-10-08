use super::*;

const SIZE: [u32; 2] = [128, 72];
const ROW: u32 = SIZE[0] * 8;
const ORANGE: [f64; 4] = [12.0, 5.0, 0.8, 0.0];

// Use the meteor's twice-subdivided icosahedron topology. Its world-space
// facets vary in depth; a screen-aligned quad cannot check equal-depth replay.
fn boulder(center: Vec3, turn: f32) -> Vec<super::super::LitVertex> {
    let t = (1.0 + 5.0_f32.sqrt()) * 0.5;
    let mut points: Vec<Vec3> = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .into_iter()
    .map(|p| Vec3::from_array(p).normalize())
    .collect();
    let mut triangles = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..2 {
        let mut middles = std::collections::BTreeMap::new();
        let mut middle = |a: usize, b: usize, points: &mut Vec<Vec3>| {
            *middles.entry((a.min(b), a.max(b))).or_insert_with(|| {
                points.push((points[a] + points[b]).normalize());
                points.len() - 1
            })
        };
        let mut next = Vec::new();
        for [a, b, c] in triangles {
            let ab = middle(a, b, &mut points);
            let bc = middle(b, c, &mut points);
            let ca = middle(c, a, &mut points);
            next.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        triangles = next;
    }
    let rotation = glam::Quat::from_axis_angle(Vec3::new(0.3, 0.8, -0.2).normalize(), turn);
    let placed: Vec<_> = points
        .iter()
        .map(|p| {
            let lump = 1.0 + 0.2 * (p.dot(Vec3::new(2.2, 3.6, 6.5)) + 0.4).sin();
            center + rotation * (*p * Vec3::new(1.0, 0.86, 1.08) * lump * 3.5)
        })
        .collect();
    let mut vertices = Vec::new();
    for [a, b, c] in triangles {
        let normal = (placed[b] - placed[a])
            .cross(placed[c] - placed[a])
            .normalize();
        let tangent = (placed[b] - placed[a]).normalize();
        for k in [a, b, c] {
            vertices.push(super::super::LitVertex {
                pos: placed[k].to_array(),
                normal: normal.to_array(),
                tangent: tangent.to_array(),
                local: (points[k] * 3.5).to_array(),
                color: [0.018, 0.011, 0.006],
                params: [0.0, 0.92, 0.0, 1.0],
            });
        }
    }
    vertices
}

fn flat(device: &wgpu::Device, fragment: &str) -> wgpu::RenderPipeline {
    let source = format!("@group(0) @binding(0) var image: texture_2d<f32>;
        @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {{
            return vec4<f32>(f32((i << 1u) & 2u) * 2.0 - 1.0, f32(i & 2u) * 2.0 - 1.0, 0.0, 1.0);
        }}
        @fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {{ {fragment} }}");
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

fn snapshot(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    source: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    target: &wgpu::Texture,
    readback: &wgpu::Buffer,
    index: u64,
) {
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(source),
        }],
    });
    let view = target.create_view(&Default::default());
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
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: index * u64::from(ROW * SIZE[1]),
                bytes_per_row: Some(ROW),
                rows_per_image: Some(SIZE[1]),
            },
        },
        wgpu::Extent3d {
            width: SIZE[0],
            height: SIZE[1],
            depth_or_array_layers: 1,
        },
    );
}

#[test]
#[ignore = "Requires a native GPU; checks the production lit fragment pipeline and sloped MSAA coverage"]
fn production_lit_projection_marks_every_visible_faceted_sample() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let copy = flat(&device, "return textureLoad(image, vec2<i32>(p.xy), 0);");
    let mask = flat(
        &device,
        "return vec4<f32>(textureLoad(image, vec2<i32>(p.xy), 0).r, 0.0, 0.0, 1.0);",
    );
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: SIZE[0],
            height: SIZE[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 4 * u64::from(ROW * SIZE[1]),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let eye = Vec3::new(143.2, 17.4, 201.8);
    let center = Vec3::new(139.7, 12.6, 150.3);
    let right = (center - eye).cross(Vec3::Y).normalize();
    let view = verse_engine::presentation::View {
        eye,
        view_proj: Mat4::perspective_rh(0.9, SIZE[0] as f32 / SIZE[1] as f32, 0.1, 1000.0)
            * Mat4::look_at_rh(eye, center, Vec3::Y),
    };
    for samples in [1, 4] {
        let photo = Photo::new(
            &device,
            &queue,
            Capability {
                hdr: Some(wgpu::TextureFormat::Rgba16Float),
                samples,
                gles: false,
                quality: Tier::High.quality(),
            },
            wgpu::TextureFormat::Rgba8Unorm,
        )
        .unwrap();
        let mut targets = photo.targets(&device, SIZE[0], SIZE[1]);
        let temporal = photo.temporal.as_ref().expect("native High temporal path");
        let mut partial = false;
        // Different subpixel projections, rotating facets, an offscreen head,
        // an occluded head, and a deliberately unmarked visible control.
        for (frame, (offset, occluded, marked)) in [
            (-10.0, false, true),
            (-4.0, false, true),
            (2.0, false, true),
            (8.0, false, true),
            (60.0, false, true),
            (0.0, true, true),
            (0.0, false, false),
        ]
        .into_iter()
        .enumerate()
        {
            let rendered =
                targets
                    .temporal
                    .as_mut()
                    .unwrap()
                    .prepare(view, frame as f32 / 60.0, true);
            let current = reversed_depth() * rendered.view_proj;
            let mut uniform = Frame::zeroed();
            uniform.view_proj = current.to_cols_array_2d();
            uniform.inv_view_proj = current.inverse().to_cols_array_2d();
            uniform.eye = eye.extend(1.0).to_array();
            uniform.sun = Vec3::new(0.3, 0.8, 0.4).normalize().extend(5.0).to_array();
            uniform.earth = [0.0, 1.0, 0.0, 0.0];
            uniform.probe_dims = [1.0, 1.0, 1.0, 0.0];
            uniform.viewport = [
                SIZE[0] as f32,
                SIZE[1] as f32,
                1.0 / SIZE[0] as f32,
                1.0 / SIZE[1] as f32,
            ];
            uniform.metering = [0.18, 1.0, 1.0, 0.0];
            queue.write_buffer(&photo.frame, 0, bytemuck::bytes_of(&uniform));
            let vertices = boulder(center + right * offset, frame as f32 * 0.41);
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let blocker_center = eye.lerp(center, 0.75);
            let blocker: Vec<_> = boulder(blocker_center, 0.2)
                .into_iter()
                .map(|mut vertex| {
                    vertex.pos = (blocker_center
                        + (Vec3::from_array(vertex.pos) - blocker_center) * 2.0)
                        .to_array();
                    vertex
                })
                .collect();
            let blocker = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&blocker),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = scene_pass(
                    &mut encoder,
                    "production faceted lit test",
                    targets.msaa.as_ref().unwrap_or(&targets.scene),
                    targets.msaa.as_ref().map(|_| &targets.scene),
                    &targets.depth,
                    wgpu::LoadOp::Clear(wgpu::Color {
                        r: ORANGE[0],
                        g: ORANGE[1],
                        b: ORANGE[2],
                        a: ORANGE[3],
                    }),
                    wgpu::LoadOp::Clear(0.0),
                    wgpu::StoreOp::Store,
                );
                pass.set_pipeline(&photo.pipelines.lit);
                pass.set_bind_group(0, &photo.scene_group, &[]);
                pass.set_bind_group(1, &targets.guide_groups[targets.parity()], &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..vertices.len() as u32, 0..1);
                if occluded {
                    pass.set_vertex_buffer(0, blocker.slice(..));
                    pass.draw(0..vertices.len() as u32, 0..1);
                }
            }
            snapshot(
                &device,
                &mut encoder,
                &targets.scene,
                &copy,
                &target,
                &readback,
                0,
            );
            let ranges = if marked {
                vec![0..vertices.len() as u32]
            } else {
                vec![]
            };
            temporal.encode(
                &queue,
                &mut encoder,
                &targets.scene,
                targets.temporal.as_mut().unwrap(),
                &[],
                Some(super::super::temporal::ReactiveLit {
                    vertices: &buffer,
                    count: vertices.len() as u32,
                    ranges: &ranges,
                }),
            );
            let temporal_targets = targets.temporal.as_ref().unwrap();
            snapshot(
                &device,
                &mut encoder,
                &targets.scene,
                &copy,
                &target,
                &readback,
                1,
            );
            snapshot(
                &device,
                &mut encoder,
                temporal_targets.reactive_view(),
                &mask,
                &target,
                &readback,
                2,
            );
            snapshot(
                &device,
                &mut encoder,
                temporal_targets.history_view(),
                &copy,
                &target,
                &readback,
                3,
            );
            queue.submit([encoder.finish()]);
            readback.map_async(wgpu::MapMode::Read, .., |result| result.unwrap());
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            let bytes = readback.get_mapped_range(..);
            let pixels: Vec<[f32; 4]> = bytes
                .chunks_exact(8)
                .map(|p| {
                    std::array::from_fn(|k| {
                        half::f16::from_bits(u16::from_le_bytes([p[k * 2], p[k * 2 + 1]])).to_f32()
                    })
                })
                .collect();
            let n = (SIZE[0] * SIZE[1]) as usize;
            let [raw, sharpened, coverage, history] =
                std::array::from_fn(|k| &pixels[k * n..(k + 1) * n]);
            assert!(
                pixels.iter().flatten().all(|v| v.is_finite()),
                "{samples}x frame {frame}: nonfinite production color"
            );
            if marked && !occluded {
                for (p, (raw, mask)) in raw.iter().zip(coverage).enumerate() {
                    assert!(
                        (raw[3] - mask[0]).abs() < 0.004,
                        "{samples}x frame {frame} pixel {p}: visible alpha {} marker {}",
                        raw[3],
                        mask[0]
                    );
                    if raw[3] > 0.0 && raw[3] < 1.0 {
                        partial = true;
                    }
                    if raw[3] > 0.0 {
                        assert!(history[p][3] < 0.0, "visible head seeded valid history");
                    }
                }
            } else if occluded {
                assert!(
                    coverage.iter().all(|p| p[0] == 0.0),
                    "hidden facets became reactive"
                );
            } else {
                assert!(
                    raw.iter()
                        .zip(history)
                        .any(|(p, h)| p[3] > 0.0 && h[3] > 0.0),
                    "unmarked control must retain ordinary depth"
                );
            }
            assert!(
                sharpened.iter().all(|p| p[3] == 1.0),
                "production sharpen ran"
            );
            drop(bytes);
            readback.unmap();
        }
        if samples == 4 {
            assert!(partial, "the fixture must exercise resolved edge samples");
        }
    }
}
