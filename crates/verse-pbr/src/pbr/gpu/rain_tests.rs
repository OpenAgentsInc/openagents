//! Offscreen regression through the real rain depth pass and exposure shader.
use super::*;

#[test]
fn a_roof_blocks_rain_wetness_and_a_removed_roof_opens_the_same_column() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .expect("offscreen GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    for tier in [Tier::Low, Tier::Medium, Tier::High] {
        let mut photo = Photo::new(
            &device,
            &queue,
            Capability {
                hdr: None,
                samples: 1,
                gles: false,
                quality: tier.quality(),
            },
            wgpu::TextureFormat::Rgba8Unorm,
        )
        .unwrap();
        let roof: Vec<LitVertex> = [
            (-4.0, -4.0),
            (4.0, -4.0),
            (4.0, 4.0),
            (-4.0, -4.0),
            (4.0, 4.0),
            (-4.0, 4.0),
        ]
        .into_iter()
        .map(|(x, z)| LitVertex {
            pos: [x, 5.0, z],
            normal: [0.0, 1.0, 0.0],
            tangent: [1.0, 0.0, 0.0],
            local: [0.0; 3],
            color: [1.0; 3],
            params: [0.0, 0.8, 0.0, 1.0],
        })
        .collect();
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("known rain roof"),
            contents: bytemuck::cast_slice(&roof),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let source = format!(
            "{}\n{}",
            verse_gfx::gles::wgsl(
                &crate::shading::source(include_str!("../photo.wgsl")),
                false
            ),
            r"
@vertex fn vs_rain_test(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}
@fragment fn fs_rain_test(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    var p = vec3<f32>(0.0, 1.0, 0.0);
    if pixel.x > 1.0 { p = vec3<f32>(12.0, 1.0, 0.0); }
    if pixel.x > 2.0 { p = vec3<f32>(0.0, 5.1, 0.0); }
    let open = rain_open(p);
    return vec4<f32>(open, open, open, 1.0);
}"
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rain exposure regression"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[Some(&photo.scene_layout)],
                    immediate_size: 0,
                }),
            ),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_rain_test"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_rain_test"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 3,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        for count in [6, 0] {
            let mut frame = Frame::zeroed();
            let world = Batches {
                streamed: None,
                lit: (&buffer, count),
                faces: [(&buffer, 0); 2],
                lines: [(&buffer, 0); 2],
                textured: None,
                figure: None,
                instances: [None, None],
                water: None,
            };
            let mut encoder = device.create_command_encoder(&Default::default());
            photo.encode_rain(&queue, &mut encoder, &mut frame, &world);
            queue.write_buffer(&photo.frame, 0, bytemuck::bytes_of(&frame));
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &photo.scene_group, &[]);
                pass.draw(0..3, 0..1);
            }
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                texture.size(),
            );
            queue.submit([encoder.finish()]);
            let (tx, rx) = std::sync::mpsc::channel();
            readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                tx.send(r).unwrap();
            });
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let bytes = readback.slice(..).get_mapped_range();
            assert_eq!(
                bytes[0],
                if count > 0 { 0 } else { 255 },
                "{tier:?}: beneath roof"
            );
            assert_eq!(bytes[4], 255, "{tier:?}: outdoors unchanged");
            assert_eq!(bytes[8], 255, "{tier:?}: exposed roof stays wet");
            drop(bytes);
            readback.unmap();
        }
    }
}
