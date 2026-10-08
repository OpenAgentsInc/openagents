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
    sampled_flat(device, "texture_2d<f32>", fragment)
}

fn sampled_flat(device: &wgpu::Device, image_type: &str, fragment: &str) -> wgpu::RenderPipeline {
    sampled_flat_reference(device, image_type, None, fragment)
}

fn sampled_flat_reference(
    device: &wgpu::Device,
    image_type: &str,
    reference_type: Option<&str>,
    fragment: &str,
) -> wgpu::RenderPipeline {
    let reference = reference_type.map_or(String::new(), |kind| {
        format!("@group(0) @binding(1) var reference: {kind};")
    });
    let source = format!("@group(0) @binding(0) var image: {image_type}; {reference}
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
    snapshot_reference(
        device, encoder, source, None, pipeline, target, readback, index,
    );
}

#[allow(clippy::too_many_arguments)]
fn snapshot_reference(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    source: &wgpu::TextureView,
    reference: Option<&wgpu::TextureView>,
    pipeline: &wgpu::RenderPipeline,
    target: &wgpu::Texture,
    readback: &wgpu::Buffer,
    index: u64,
) {
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: wgpu::BindingResource::TextureView(source),
    }];
    if let Some(reference) = reference {
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureView(reference),
        });
    }
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
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

#[test]
#[ignore = "Requires a native GPU; checks full High scene orchestration with a reactive rock and HDR ribbon"]
fn photo_encode_preserves_reactive_coverage_and_history_beside_an_hdr_ribbon() {
    full_photo_reactive_case(false);
}

#[test]
#[ignore = "Requires a native GPU; checks a moving perspective camera and reactive head over an opaque receiver and HDR ribbon"]
fn photo_encode_tracks_a_reactive_head_over_a_stationary_receiver_with_camera_motion() {
    full_photo_reactive_case(true);
}

fn full_photo_reactive_case(receiver: bool) {
    assert_ne!(
        std::env::var("VERSE_TEMPORAL_AA").ok().as_deref(),
        Some("off"),
        "this diagnostic requires temporal AA"
    );
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: adapter.features() & wgpu::Features::TIMESTAMP_QUERY,
        ..Default::default()
    }))
    .unwrap();
    let copy = flat(&device, "return textureLoad(image, vec2<i32>(p.xy), 0);");
    let mask = flat(
        &device,
        "return vec4<f32>(textureLoad(image, vec2<i32>(p.xy), 0).rg, 0.0, 1.0);",
    );
    // Only opaque geometry writes physical depth. The clear-depth case counts
    // nonzero samples; the receiver case compares each sample to the absent
    // head's matching depth. Neither reference reads the reactive marker.
    let coverage = if receiver {
        sampled_flat_reference(
            &device,
            "texture_depth_multisampled_2d",
            Some("texture_depth_multisampled_2d"),
            "var covered = 0.0; var nearest = 0.0;
             for (var sample = 0; sample < 4; sample++) {
                 let d = textureLoad(image, vec2<i32>(p.xy), sample);
                 let behind = textureLoad(reference, vec2<i32>(p.xy), sample);
                 covered += select(0.0, 0.25, d > behind);
                 nearest = max(nearest, d);
             }
             return vec4<f32>(covered, nearest, 0.0, 1.0);",
        )
    } else {
        sampled_flat(
            &device,
            "texture_depth_multisampled_2d",
            "var covered = 0.0; var nearest = 0.0;
         for (var sample = 0; sample < 4; sample++) {
             let d = textureLoad(image, vec2<i32>(p.xy), sample);
             covered += select(0.0, 0.25, d > 0.0);
             nearest = max(nearest, d);
         }
         return vec4<f32>(covered, nearest, 0.0, 1.0);",
        )
    };
    let texture = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("full photo reactive diagnostic"),
            size: wgpu::Extent3d {
                width: SIZE[0],
                height: SIZE[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let target = texture(
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let output = texture(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    )
    .create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("full photo reactive diagnostic"),
        size: 4 * u64::from(ROW * SIZE[1]),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut photo = Photo::new(
        &device,
        &queue,
        Capability {
            hdr: Some(wgpu::TextureFormat::Rgba16Float),
            samples: 4,
            gles: false,
            quality: Tier::High.quality(),
        },
        wgpu::TextureFormat::Rgba8Unorm,
    )
    .unwrap();
    let mut targets = std::array::from_fn::<_, 3, _>(|_| photo.targets(&device, SIZE[0], SIZE[1]));
    assert!(photo.prepass.is_some() && photo.water_screen.is_some());
    let eye = Vec3::new(143.2, 17.4, 201.8);
    let center = Vec3::new(139.7, 12.6, 150.3);
    let toward = (eye - center).normalize();
    let right = (center - eye).cross(Vec3::Y).normalize();
    let up = toward.cross(right).normalize();
    let receiver_center = center - toward * 15.0;
    let corners = [(-70.0, -45.0), (70.0, -45.0), (70.0, 45.0), (-70.0, 45.0)]
        .map(|(x, y)| receiver_center + right * x + up * y);
    let receiver_vertices: Vec<_> = [0, 1, 2, 0, 2, 3]
        .map(|k| super::super::LitVertex {
            pos: corners[k].to_array(),
            normal: toward.to_array(),
            tangent: right.to_array(),
            local: (corners[k] - receiver_center).to_array(),
            color: [0.16, 0.10, 0.065],
            params: [0.0, 0.9, 0.0, 1.0],
        })
        .into();
    let view = verse_engine::presentation::View {
        eye,
        view_proj: Mat4::perspective_rh(0.9, SIZE[0] as f32 / SIZE[1] as f32, 0.1, 1000.0)
            * Mat4::look_at_rh(eye, center, Vec3::Y),
    };
    // A clipped static mesh keeps the identical key-light path active in the
    // no-rock control. Nonzero prefix and suffix vertices test range offsets.
    let outside = boulder(center + right * 100.0, 0.0);
    let static_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("clipped static light admission"),
        contents: bytemuck::cast_slice(&outside),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let mut partial = false;
    let mut hdr = false;
    let mut dependent_background = false;
    let mut camera_moved = false;
    let mut fx_covered = false;
    let mut preceding_footprint = vec![false; (SIZE[0] * SIZE[1]) as usize];
    for (frame, (offset, present, occluded)) in [
        (0.0, false, false),
        (0.0, false, false),
        (-10.0, true, false),
        (-4.0, true, false),
        (2.0, true, false),
        (8.0, true, false),
        (14.0, true, false),
        (60.0, true, false),
        (0.0, true, true),
        (0.0, false, false),
        (0.0, false, false),
    ]
    .into_iter()
    .enumerate()
    {
        let view = if receiver {
            let eye = eye
                + right * (frame as f32 * 0.7).sin() * 1.1
                + up * (frame as f32 * 0.5).sin() * 0.5;
            camera_moved |= eye != view.eye;
            verse_engine::presentation::View {
                eye,
                view_proj: Mat4::perspective_rh(0.9, SIZE[0] as f32 / SIZE[1] as f32, 0.1, 1000.0)
                    * Mat4::look_at_rh(eye, center, up),
            }
        } else {
            view
        };
        let head = boulder(center + right * offset, frame as f32 * 0.41);
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
        let mut world_vertices = if receiver {
            receiver_vertices.clone()
        } else {
            outside.clone()
        };
        if occluded {
            world_vertices.extend_from_slice(&blocker);
        }
        let world_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("full photo opaque control"),
            contents: bytemuck::cast_slice(&world_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ribbon_shift = if receiver && present { offset } else { 0.0 };
        let ribbon = crate::fx::Ribbon {
            points: [-24.0, -12.0, 0.0, 12.0, 24.0]
                .into_iter()
                .map(|offset| crate::fx::RibbonPoint {
                    at: center + right * (offset + ribbon_shift) - toward * 5.0,
                    half: 7.0,
                    color: [12.0, 5.0, 0.8],
                    alpha: 1.0,
                })
                .collect(),
            layer: crate::fx::sheet::layer("fireball").unwrap(),
            rect: crate::fx::sheet::find("fireball").unwrap().rect(6),
            priority: 240,
        };
        let mut sprites = Vec::new();
        crate::fx::vertices_with_ribbons(&[], &[ribbon], view.eye, 1024, &mut sprites);
        photo.sprites.write(&device, &queue, &sprites);
        assert!(photo.sprites.count > 0, "the split particle path must run");
        let mut neon = Neon::plaza(frame as f32 / 60.0);
        neon.field = [0.005, 0.004, 0.003];
        neon.fog_start = 2000.0;
        neon.fog_end = 3000.0;
        neon.key = Some(super::super::Key {
            dir: Vec3::new(0.3, 0.8, 0.4).normalize(),
            illuminance: 5.0,
            angular_radius: 0.01,
            rim_dir: -Vec3::Y,
            rim_illuminance: 0.0,
            rim_angular_radius: 0.01,
            sky: 1.0,
            ground: 0.2,
            ev100: 0.0,
            shadow_center: center,
            shadow_half: 80.0,
            shadow_distance: None,
            cache_far_shadows: false,
        });
        let mut frame_images: [Vec<[f32; 4]>; 3] = std::array::from_fn(|_| Vec::new());
        // The absent-head depth supplies a same-camera, same-sample receiver
        // reference. It renders first before either head variant reads it.
        let order = if receiver { [2, 1, 0] } else { [0, 1, 2] };
        for variant in order {
            let reference_depth = receiver.then(|| targets[2].depth.clone());
            let targets = &mut targets[variant];
            let mut vertices = outside.clone();
            let first = vertices.len() as u32;
            if present && variant != 2 {
                vertices.extend_from_slice(&head);
            }
            let end = vertices.len() as u32;
            vertices.extend_from_slice(&outside);
            photo.dynamic_lit.write(&device, &queue, &vertices);
            let dynamic = photo.dynamic_lit.buffer.clone();
            let dynamic_count = photo.dynamic_lit.count;
            let ranges = if variant == 0 && first < end {
                vec![first..end]
            } else {
                vec![]
            };
            let mut encoder = device.create_command_encoder(&Default::default());
            photo.encode(
                &device,
                &queue,
                &mut encoder,
                &output,
                targets,
                view,
                Stage::Neon(&neon),
                Batches {
                    motion: &[],
                    reactive_lit: Some(super::super::temporal::ReactiveLit {
                        vertices: &dynamic,
                        count: dynamic_count,
                        ranges: &ranges,
                    }),
                    #[cfg(not(target_arch = "wasm32"))]
                    streamed: None,
                    lit: (&world_buffer, world_vertices.len() as u32),
                    faces: [(&static_buffer, 0); 2],
                    lines: [(&static_buffer, 0); 2],
                    textured: None,
                    figure: None,
                    instances: None,
                    water: None,
                },
                None,
            );
            let temporal = targets.temporal.as_ref().unwrap();
            assert!(temporal.enabled, "full Photo path must enable temporal AA");
            for (index, (source, pipeline)) in [
                (temporal.reactive_view(), &mask),
                (&targets.depth, &coverage),
                (temporal.history_view(), &copy),
                (&targets.scene, &copy),
            ]
            .into_iter()
            .enumerate()
            {
                snapshot_reference(
                    &device,
                    &mut encoder,
                    source,
                    if index == 1 {
                        reference_depth.as_ref()
                    } else {
                        None
                    },
                    pipeline,
                    &target,
                    &readback,
                    index as u64,
                );
            }
            // Each variant submits separately: Photo's frame uniform must not
            // be overwritten by another encode before these draws execute.
            queue.submit([encoder.finish()]);
            photo.submitted();
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
            let [marker, depth, history, scene] =
                std::array::from_fn(|k| &pixels[k * n..(k + 1) * n]);
            assert!(pixels.iter().flatten().all(|v| v.is_finite()));
            if receiver {
                let middle = (SIZE[1] / 2 * SIZE[0] + SIZE[0] / 2) as usize;
                assert!(
                    depth[middle][1] > 0.0,
                    "the receiver must write physical depth"
                );
                if variant == 2 {
                    assert!(
                        depth.iter().all(|p| p[0] == 0.0),
                        "the same-sample receiver reference cannot occlude itself"
                    );
                }
            }
            hdr |= scene.iter().any(|p| p[0] > 1.0 && p[1] > 0.1);
            fx_covered |= marker.iter().any(|p| p[1] > 0.0);
            if variant == 0 && present {
                let mut visible = 0;
                let mut dependent = 0;
                for p in 0..n {
                    if occluded {
                        assert_eq!(marker[p][0], 0.0, "nearer geometry must hide the marker");
                        continue;
                    }
                    assert!(
                        (depth[p][0] - marker[p][0]).abs() < 0.004,
                        "frame {frame} pixel {p}: physical coverage {} reactive {}",
                        depth[p][0],
                        marker[p][0]
                    );
                    partial |= marker[p][0] > 0.0 && marker[p][0] < 1.0;
                    visible += usize::from(marker[p][0] > 0.0);
                    let x = p as i32 % SIZE[0] as i32;
                    let y = p as i32 / SIZE[0] as i32;
                    let touches_head = (-1..=1).any(|dy| {
                        (-1..=1).any(|dx| {
                            let x = (x + dx).clamp(0, SIZE[0] as i32 - 1);
                            let y = (y + dy).clamp(0, SIZE[1] as i32 - 1);
                            marker[(y * SIZE[0] as i32 + x) as usize][0] > 0.0
                        })
                    });
                    if touches_head {
                        dependent += 1;
                        assert!(
                            history[p][3] < 0.0,
                            "frame {frame} pixel {p}: actual Photo history retained head-dependent color at alpha {}",
                            history[p][3]
                        );
                        dependent_background |= depth[p][0] == 0.0
                            && (!receiver || depth[p][1] > 0.0)
                            && scene[p][0] > 0.1;
                    }
                }
                eprintln!(
                    "full Photo frame {frame}: visible={visible}, dependent={dependent}, occluded={occluded}"
                );
                if offset < 60.0 && !occluded {
                    assert!(
                        visible > 0,
                        "visible rock must populate the actual marker R channel"
                    );
                }
            } else {
                assert!(
                    marker.iter().all(|p| p[0] == 0.0),
                    "unmarked and no-rock controls must not write body R"
                );
            }
            // The independent body-depth oracle remains in R. All variants
            // also draw additive ribbons, so history uses the union with G.
            for p in 0..n {
                let x = p as i32 % SIZE[0] as i32;
                let y = p as i32 / SIZE[0] as i32;
                let touches = (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| {
                        let x = (x + dx).clamp(0, SIZE[0] as i32 - 1);
                        let y = (y + dy).clamp(0, SIZE[1] as i32 - 1);
                        let value = marker[(y * SIZE[0] as i32 + x) as usize];
                        value[0] > 0.0 || value[1] > 0.0
                    })
                });
                assert_eq!(
                    history[p][3] < 0.0,
                    touches,
                    "frame {frame} variant {variant} pixel {p}: history must match the actual R/G conditioning footprint"
                );
            }
            frame_images[variant] = pixels;
            drop(bytes);
            readback.unmap();
        }
        let n = (SIZE[0] * SIZE[1]) as usize;
        let marked = &frame_images[0];
        let unmarked = &frame_images[1];
        let absent = &frame_images[2];
        let mut marked_error = 0.0_f32;
        let mut unmarked_error = 0.0_f32;
        let mut prior_background = 0;
        for p in 0..n {
            if preceding_footprint[p] && marked[n + p][0] == 0.0 {
                prior_background += 1;
                for channel in 0..3 {
                    marked_error = marked_error
                        .max((marked[2 * n + p][channel] - absent[2 * n + p][channel]).abs());
                    unmarked_error = unmarked_error
                        .max((unmarked[2 * n + p][channel] - absent[2 * n + p][channel]).abs());
                }
            }
            preceding_footprint[p] = marked[2 * n + p][3] < 0.0;
        }
        // Keep the control differences visible even when coverage/metadata
        // pass: this distinguishes a later color leak from a lost marker.
        eprintln!(
            "full Photo frame {frame}: preceding background={prior_background}, retained HDR max error marked={marked_error}, unmarked={unmarked_error}"
        );
    }
    assert!(
        partial,
        "full High rendering must exercise partial MSAA edge samples"
    );
    assert!(
        hdr,
        "the actual atlas-backed ribbon must reach HDR scene color"
    );
    assert!(
        dependent_background,
        "the reactive conditioning footprint must touch ribbon beyond the head"
    );
    assert!(
        fx_covered,
        "the full Photo ribbon must survive the later body marker pass in G"
    );
    if receiver {
        assert!(
            camera_moved,
            "the receiver case must move the perspective camera"
        );
    }
}

#[test]
#[ignore = "Requires a native GPU; compares actual sprite MRT with the original color draw and checks visible additive coverage"]
fn photo_sprite_mrt_matches_color_and_rejects_hidden_or_covered_emission() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let copy = flat(&device, "return textureLoad(image, vec2<i32>(p.xy), 0);");
    let mask = flat(
        &device,
        "return vec4<f32>(textureLoad(image, vec2<i32>(p.xy), 0).rg, 0.0, 1.0);",
    );
    let eye = Vec3::new(0.0, 0.0, 12.0);
    let view = verse_engine::presentation::View {
        eye,
        view_proj: Mat4::perspective_rh(1.1, SIZE[0] as f32 / SIZE[1] as f32, 0.1, 100.0)
            * Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y),
    };
    let corners = [
        Vec3::new(-20.0, -20.0, 0.0),
        Vec3::new(20.0, -20.0, 0.0),
        Vec3::new(20.0, 20.0, 0.0),
        Vec3::new(-20.0, 20.0, 0.0),
    ];
    let plane: Vec<_> = [0, 1, 2, 0, 2, 3]
        .map(|k| super::super::LitVertex {
            pos: corners[k].to_array(),
            normal: Vec3::Z.to_array(),
            tangent: Vec3::X.to_array(),
            local: corners[k].to_array(),
            color: [0.1; 3],
            params: [0.0, 0.9, 0.0, 1.0],
        })
        .into();
    let world = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("MRT receiver"),
        contents: bytemuck::cast_slice(&plane),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let mut neon = Neon::plaza(0.0);
    neon.field = [0.0; 3];
    neon.fog_start = 2000.0;
    neon.fog_end = 3000.0;
    neon.key = Some(super::super::Key {
        dir: Vec3::Z,
        illuminance: 1.0,
        angular_radius: 0.01,
        rim_dir: Vec3::Y,
        rim_illuminance: 0.0,
        rim_angular_radius: 0.01,
        sky: 1.0,
        ground: 1.0,
        ev100: 0.0,
        shadow_center: Vec3::ZERO,
        shadow_half: 30.0,
        shadow_distance: None,
        cache_far_shadows: false,
    });
    let texture = |samples, format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MRT independent color reference"),
            size: wgpu::Extent3d {
                width: SIZE[0],
                height: SIZE[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let target = texture(
        1,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let output = texture(
        1,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    )
    .create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("MRT integration readback"),
        size: 4 * u64::from(ROW * SIZE[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    for (samples, tier) in [(1, Tier::Medium), (4, Tier::High)] {
        let mut photo = Photo::new(
            &device,
            &queue,
            Capability {
                hdr: Some(wgpu::TextureFormat::Rgba16Float),
                samples,
                gles: false,
                quality: tier.quality(),
            },
            wgpu::TextureFormat::Rgba8Unorm,
        )
        .unwrap();
        assert!(photo.pipelines.sprites_reactive.is_some());
        // Constant authored alpha exposes the volume's narrower, possibly
        // zero-radiance support. A separate opaque layer makes smoke erasure exact.
        let sheets = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MRT coverage controls"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 5,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        queue.write_texture(
            sheets.as_image_copy(),
            &[255; 20],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 5,
            },
        );
        photo.fx_parts.sheets = sheets.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let reference = texture(
            1,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let reference_view = reference.create_view(&Default::default());
        let reference_msaa = (samples > 1).then(|| {
            texture(
                samples,
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
            .create_view(&Default::default())
        });
        for (name, color, z, additive, alpha, smoke, expected) in [
            ("volume", [12.0, 5.0, 0.8], 1.0, 1.0, 1.0, false, true),
            ("zero emission", [0.0; 3], 1.0, 1.0, 1.0, false, false),
            ("zero alpha", [12.0, 5.0, 0.8], 1.0, 1.0, 0.0, false, false),
            ("occluded", [12.0, 5.0, 0.8], -1.0, 1.0, 1.0, false, false),
            ("alpha share", [12.0, 5.0, 0.8], 1.0, 0.0, 1.0, false, false),
            ("opaque smoke", [12.0, 5.0, 0.8], 1.0, 1.0, 1.0, true, false),
        ] {
            let fire = crate::fx::Sprite {
                at: Vec3::new(0.0, 0.0, z),
                half: 3.0,
                angle: 0.0,
                tail: Vec3::ZERO,
                facing: crate::fx::Facing::Camera,
                color,
                alpha: 1.0,
                additive,
                lit: false,
                scene_lit: false,
                density: 1.0,
                layer: 0,
                rect_a: [0.0, 0.0, 1.0, 1.0],
                rect_b: [0.0, 0.0, 1.0, 1.0],
                mix: 0.35,
                priority: 240,
            };
            let mut vertices = Vec::new();
            crate::fx::vertices(&[fire], eye, 32, &mut vertices);
            for vertex in &mut vertices {
                vertex.color[3] = alpha;
            }
            if smoke {
                let mut foreground = Vec::new();
                crate::fx::vertices(
                    &[crate::fx::Sprite {
                        at: Vec3::new(0.0, 0.0, 2.0),
                        half: 7.0,
                        color: [0.2; 3],
                        layer: 1,
                        additive: 0.0,
                        lit: true,
                        ..fire
                    }],
                    eye,
                    32,
                    &mut foreground,
                );
                vertices.extend(foreground);
            }
            photo.sprites.write(&device, &queue, &vertices);
            let mut images: [Vec<[f32; 4]>; 2] = std::array::from_fn(|_| Vec::new());
            for mrt in [false, true] {
                let mut targets = photo.targets(&device, SIZE[0], SIZE[1]);
                let pipeline = (!mrt).then(|| photo.pipelines.sprites_reactive.take().unwrap());
                let mut encoder = device.create_command_encoder(&Default::default());
                photo.encode(
                    &device,
                    &queue,
                    &mut encoder,
                    &output,
                    &mut targets,
                    view,
                    Stage::Neon(&neon),
                    Batches {
                        lit: (&world, plane.len() as u32),
                        faces: [(&world, 0); 2],
                        lines: [(&world, 0); 2],
                        textured: None,
                        figure: None,
                        instances: None,
                        water: None,
                        motion: &[],
                        reactive_lit: None,
                        #[cfg(not(target_arch = "wasm32"))]
                        streamed: None,
                    },
                    None,
                );
                if let Some(pipeline) = pipeline {
                    photo.pipelines.sprites_reactive = Some(pipeline);
                }
                // The original single-color fragment draws on transparent HDR,
                // against the same physical samples and current frame bindings.
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("original sprite color oracle"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: reference_msaa.as_ref().unwrap_or(&reference_view),
                            resolve_target: reference_msaa.as_ref().map(|_| &reference_view),
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &targets.depth,
                            depth_ops: None,
                            stencil_ops: None,
                        }),
                        ..Default::default()
                    });
                    pass.set_pipeline(&photo.pipelines.sprites);
                    pass.set_bind_group(0, &photo.scene_group, &[]);
                    pass.set_bind_group(1, &targets.guide_groups[0], &[]);
                    pass.set_bind_group(2, &photo.empty_group, &[]);
                    pass.set_bind_group(3, &targets.fx_group, &[]);
                    pass.set_vertex_buffer(0, photo.sprites.buffer.slice(..));
                    pass.draw(0..photo.sprites.count, 0..1);
                }
                let temporal = targets.temporal.as_ref().unwrap();
                for (index, (view, pipeline)) in [
                    (temporal.reactive_view(), &mask),
                    (temporal.history_view(), &copy),
                    (&targets.scene, &copy),
                    (&reference_view, &copy),
                ]
                .into_iter()
                .enumerate()
                {
                    snapshot(
                        &device,
                        &mut encoder,
                        view,
                        pipeline,
                        &target,
                        &readback,
                        index as u64,
                    );
                }
                queue.submit([encoder.finish()]);
                photo.submitted();
                readback.map_async(wgpu::MapMode::Read, .., |result| result.unwrap());
                device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
                let bytes = readback.get_mapped_range(..);
                let pixels: Vec<[f32; 4]> = bytes
                    .chunks_exact(8)
                    .map(|p| {
                        std::array::from_fn(|k| {
                            half::f16::from_bits(u16::from_le_bytes([p[k * 2], p[k * 2 + 1]]))
                                .to_f32()
                        })
                    })
                    .collect();
                let n = (SIZE[0] * SIZE[1]) as usize;
                if mrt {
                    let marker = &pixels[..n];
                    assert!(
                        marker.iter().all(|p| p[0] == 0.0),
                        "nonreactive receiver cannot write body R"
                    );
                    assert_eq!(
                        marker.iter().any(|p| p[1] > 0.0),
                        expected,
                        "{samples}x {name}: actual G visibility"
                    );
                    for p in 0..n {
                        let x = p as i32 % SIZE[0] as i32;
                        let y = p as i32 / SIZE[0] as i32;
                        let touches = (-1..=1).any(|dy| {
                            (-1..=1).any(|dx| {
                                let x = (x + dx).clamp(0, SIZE[0] as i32 - 1);
                                let y = (y + dy).clamp(0, SIZE[1] as i32 - 1);
                                marker[(y * SIZE[0] as i32 + x) as usize][1] > 0.0
                            })
                        });
                        assert_eq!(
                            pixels[n + p][3] < 0.0,
                            touches,
                            "{samples}x {name} pixel {p}: history must follow visible FX, not its authored proxy"
                        );
                    }
                    if expected {
                        // This lies inside the fully opaque authored quad but
                        // outside the volume's radial support in its repeated cell.
                        let middle = (SIZE[1] / 2 * SIZE[0] + SIZE[0] / 2) as usize;
                        assert_eq!(marker[middle][1], 0.0);
                        assert!(
                            pixels[3 * n + middle][..3].iter().all(|&v| v == 0.0),
                            "the independent fragment must confirm zero volume emission at the authored center"
                        );
                        for p in 0..n {
                            let emitted = pixels[3 * n + p][..3].iter().any(|&v| v > 0.0);
                            assert_eq!(
                                marker[p][1] > 0.0,
                                emitted,
                                "{samples}x {name} pixel {p}: G must match original fragment emission"
                            );
                        }
                    }
                }
                images[usize::from(mrt)] = pixels;
                drop(bytes);
                readback.unmap();
            }
            let n = (SIZE[0] * SIZE[1]) as usize;
            for p in 0..n {
                assert_eq!(
                    images[0][2 * n + p],
                    images[1][2 * n + p],
                    "{samples}x {name} pixel {p}: MRT must preserve the original scene color on a fresh history"
                );
            }
        }
    }
}
