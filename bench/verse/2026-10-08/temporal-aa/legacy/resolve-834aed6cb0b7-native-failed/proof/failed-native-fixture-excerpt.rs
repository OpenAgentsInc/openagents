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
