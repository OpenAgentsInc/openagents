//! Native checks of production vertex lighting and sparse GPU deliveries.

use super::*;
use crate::pbr::baked_layers::{Layers, Reference, SunLayer, decode_lamp, encode_lamp};
use crate::pbr::textured::{
    DynamicInstance, InstancedFigure, LightPatch, Primitive, TexturedMesh, VertexLightStream,
};
use crate::pbr::textured_bake::decode;
use std::collections::BTreeMap;
use std::sync::Arc;

const SIZE: u32 = 4;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

struct Harness {
    device: wgpu::Device,
    queue: wgpu::Queue,
    photo: Photo,
    pipeline: wgpu::RenderPipeline,
    frame: wgpu::BindGroup,
    empty: wgpu::BindGroup,
}

impl Harness {
    fn new() -> Self {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
        let mut capability =
            Capability::probe(&adapter, &device, wgpu::TextureFormat::Rgba8Unorm, 1);
        capability.hdr = None;
        capability.samples = 1;
        capability.quality = Tier::Low.quality();
        let photo =
            Photo::new(&device, &queue, capability, wgpu::TextureFormat::Rgba8Unorm).unwrap();
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("baked test frame"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let mut uniform = Frame::zeroed();
        uniform.view_proj = Mat4::IDENTITY.to_cols_array_2d();
        uniform.baked_sun = [1.0, 2.0, 0.25, 0.8];
        uniform.lamp_params[3] = 1.25;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("baked test frame"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let frame = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("baked test frame"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        let empty_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("baked test empty"),
            entries: &[],
        });
        let empty = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("baked test empty"),
            layout: &empty_layout,
            entries: &[],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("baked test light"),
            bind_group_layouts: &[
                Some(&uniform_layout),
                Some(&empty_layout),
                Some(&empty_layout),
                Some(&photo.light_layout),
            ],
            immediate_size: 0,
        });
        // Use the production vertex entry point. The test fragment reports its
        // decoded outputs before exposure, materials, or postprocessing vary them.
        let source = format!(
            "{}\nstruct TestLight {{ @location(0) ambient: vec4<f32>, @location(1) lamp: vec4<f32> }}\n@fragment fn fs_test_light(i: TexturedOut) -> TestLight {{ return TestLight(i.ambient, vec4<f32>(i.lamp, 1.0)); }}",
            include_str!("../photo.wgsl")
        );
        let module = shader(&device, "baked test production vertex", &source);
        let targets = [0, 1].map(|_| {
            Some(wgpu::ColorTargetState {
                format: FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("baked test light"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_textured"),
                compilation_options: Default::default(),
                buffers: &textured_layout(),
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_test_light"),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            device,
            queue,
            photo,
            pipeline,
            frame,
            empty,
        }
    }

    fn pixels(&self, gpu: &TexturedGpu, indices: std::ops::Range<u32>) -> [[f32; 4]; 2] {
        let textures = [0, 1].map(|_| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("baked test result"),
                size: wgpu::Extent3d {
                    width: SIZE,
                    height: SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        });
        let views = textures
            .each_ref()
            .map(|texture| texture.create_view(&Default::default()));
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("baked test pixels"),
            size: u64::from(2 * SIZE * 256),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let attachments = views.each_ref().map(|view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &attachments,
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.frame, &[]);
            pass.set_bind_group(1, &self.empty, &[]);
            pass.set_bind_group(2, &self.empty, &[]);
            pass.set_bind_group(3, &gpu.light_group, &[]);
            pass.set_vertex_buffer(0, gpu.vertices.slice(..));
            pass.set_vertex_buffer(1, gpu.instances.slice(..));
            pass.set_index_buffer(gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(indices, 0, 0..1);
        }
        for (i, texture) in textures.iter().enumerate() {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: i as u64 * u64::from(SIZE * 256),
                        bytes_per_row: Some(256),
                        rows_per_image: Some(SIZE),
                    },
                },
                texture.size(),
            );
        }
        let submission = self.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                send.send(result).unwrap();
            });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .unwrap();
        receive.recv().unwrap().unwrap();
        let data = readback.slice(..).get_mapped_range();
        let result = [0, 1].map(|i| {
            let offset = i * SIZE as usize * 256 + 256 + 8;
            std::array::from_fn(|c| {
                half::f16::from_bits(u16::from_le_bytes([
                    data[offset + c * 2],
                    data[offset + c * 2 + 1],
                ]))
                .to_f32()
            })
        });
        drop(data);
        readback.unmap();
        result
    }

    fn deliveries(&self, gpu: &mut TexturedGpu, scene: &TexturedScene) {
        let slot = &scene.baked;
        let (lights, lamps, layers, mask, patches) = (
            slot.take(),
            slot.take_lamps(),
            slot.take_layers(),
            slot.take_mask(),
            slot.take_patches(),
        );
        if let Some(lights) = lights {
            gpu.write_baked(&self.queue, &lights);
        }
        if let Some(lamps) = lamps {
            self.photo
                .write_textured_lamps(&self.device, &self.queue, gpu, &lamps);
        }
        if let Some(layers) = layers {
            self.photo
                .write_textured_layers(&self.device, &self.queue, gpu, &layers);
        }
        self.photo
            .write_textured_patches(&self.queue, gpu, &patches);
        if let Some(mask) = mask {
            self.photo.write_textured_mask(&self.queue, gpu, &mask);
        }
    }
}

fn triangle() -> Vec<TexturedVertex> {
    [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(3.0, -1.0, 0.0),
        Vec3::new(-1.0, 3.0, 0.0),
    ]
    .into_iter()
    .map(|pos| TexturedVertex::new(pos, Vec3::Z, [0.0; 2]))
    .collect()
}

fn assert_texels(actual: [[f32; 4]; 2], sky: [u8; 4], suns: Option<[[u8; 4]; 2]>, lamp: [u8; 4]) {
    let (mut ambient, open) = decode(sky);
    if let Some(suns) = suns {
        ambient += decode(suns[0]).0.lerp(decode(suns[1]).0, 0.25) * 0.8;
    }
    let expected = [
        (ambient * 1.25).extend(open).to_array(),
        decode_lamp(lamp).extend(1.0).to_array(),
    ];
    for (actual, expected) in actual.into_iter().zip(expected) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            // Half-float output rounds once; allow two output ULPs for
            // interpolation and the shader's multiply/add evaluation.
            let rounded = half::f16::from_f32(expected);
            let next = half::f16::from_bits(rounded.to_bits().saturating_add(1));
            let tolerance = (next.to_f32() - rounded.to_f32()).abs() * 2.0 + 1e-6;
            assert!(
                (actual - expected).abs() <= tolerance,
                "{actual} versus {expected}, tolerance {tolerance}"
            );
        }
    }
}

#[test]
#[ignore = "Requires a native GPU; checks sunlight, repair masks, and pristine restoration"]
fn static_light_patches_cross_rows_and_layers_and_survive_late_bakes_until_restore() {
    let h = Harness::new();
    let validation = h.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut scene = TexturedScene::default();
    scene.add_material(TexturedMaterial::default());
    let first = 4094u32;
    let count = first as usize + 3;
    let sky = [83, 107, 139, 219];
    let sun = [[64, 80, 96, 255], [96, 64, 48, 255]];
    let lamp = encode_lamp(Vec3::new(3.0, 5.0, 7.0));
    let mut prepared = Prepared {
        vertices: vec![
            GpuVertex::pack(&TexturedVertex::new(Vec3::splat(10.0), Vec3::Z, [0.0; 2]));
            count
        ],
        indices: vec![first, first + 1, first + 2],
        instances: vec![Instance::MERGED],
        lights: vec![sky; count],
        ..Default::default()
    };
    for (target, vertex) in prepared.vertices[first as usize..]
        .iter_mut()
        .zip(triangle())
    {
        *target = GpuVertex::pack(&vertex);
    }
    let layers = Arc::new(Layers {
        bake_key: "native-test".into(),
        scene: "native-test".into(),
        reference: Reference {
            sun: 1.0,
            sky: 1.0,
            ground: 1.0,
        },
        sky: vec![sky; count],
        sky_probes: vec![[0.0; 12]],
        suns: sun
            .map(|texel| SunLayer {
                dir: Vec3::Y.to_array(),
                vertices: vec![texel; count],
                probes: vec![[0.0; 12]],
            })
            .to_vec(),
        lamps: (first..first + 3).map(|index| (index, lamp)).collect(),
        probe_origin: [0.0; 3],
        probe_cell: 1.0,
        probe_dims: [1; 3],
    });
    let mut gpu = h
        .photo
        .upload_textured(&h.device, &h.queue, &scene, &prepared);
    assert_eq!(gpu.light_rows, 2);
    scene.baked.deliver_lights(layers.sky.clone());
    scene.baked.deliver_lamps(layers.lamp_texels());
    scene.baked.deliver_layers(layers.clone());
    h.deliveries(&mut gpu, &scene);
    assert_texels(h.pixels(&gpu, 0..3), sky, Some(sun), lamp);
    let repaired = [17, 29, 41, 177];
    scene.baked.deliver_patches([LightPatch {
        first,
        lights: vec![repaired; 3],
        dynamic: true,
    }]);
    h.deliveries(&mut gpu, &scene);
    assert_texels(h.pixels(&gpu, 0..3), repaired, None, [0; 4]);
    let later_sky = [121, 93, 71, 239];
    let mut later = (*layers).clone();
    later.sky.fill(later_sky);
    later.suns.reverse();
    let later = Arc::new(later);
    scene.baked.deliver_lights(later.sky.clone());
    scene.baked.deliver_lamps(later.lamp_texels());
    scene.baked.deliver_layers(later);
    h.deliveries(&mut gpu, &scene);
    assert_texels(h.pixels(&gpu, 0..3), repaired, None, [0; 4]);
    // A queued bright repair must not run after restoration of pristine planes.
    scene.baked.deliver_patches([LightPatch {
        first,
        lights: vec![[200; 4]; 3],
        dynamic: true,
    }]);
    scene.baked.clear_repairs();
    scene.baked.set_fallback(Vec::new());
    h.deliveries(&mut gpu, &scene);
    assert_texels(
        h.pixels(&gpu, 0..3),
        later_sky,
        Some([sun[1], sun[0]]),
        lamp,
    );
    scene
        .baked
        .set_fallback((first..first + 3).map(|i| (i, repaired)).collect());
    scene.baked.deliver_patches([LightPatch {
        first,
        lights: vec![later_sky; 3],
        dynamic: false,
    }]);
    h.deliveries(&mut gpu, &scene);
    assert_texels(h.pixels(&gpu, 0..3), repaired, None, [0; 4]);
    scene.baked.set_fallback(Vec::new());
    h.deliveries(&mut gpu, &scene);
    assert_texels(
        h.pixels(&gpu, 0..3),
        later_sky,
        Some([sun[1], sun[0]]),
        lamp,
    );
    assert!(pollster::block_on(validation.pop()).is_none());
}

#[test]
#[ignore = "Requires a native GPU; checks wrapped rigid ranges and light texture rebinding"]
fn rigid_light_ranges_wrap_max_grow_rebind_and_restore_direct_ambient() {
    let h = Harness::new();
    let validation = h.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut source = TexturedScene::default();
    source.add_material(TexturedMaterial::default());
    for _ in 0..2 {
        source.add_mesh(TexturedMesh {
            primitives: vec![Primitive {
                vertices: triangle(),
                indices: vec![0, 1, 2],
                material: 0,
            }],
        });
    }
    let direct = [101, 90, 73, 214];
    let repaired = [17, 29, 41, 177];
    let mut frame = InstancedFigure {
        scene: Arc::new(source),
        motion_epoch: Arc::new(()),
        instances: Arc::new(vec![DynamicInstance {
            id: 7,
            mesh: 1,
            current: Mat4::IDENTITY,
            previous: Mat4::IDENTITY,
            color: [1.0; 4],
            light: direct,
            settled: false,
        }]),
        vertex_lights: Some(VertexLightStream {
            texels: Arc::new(vec![repaired; 5]),
            ranges: Arc::new(BTreeMap::from([(7, 2)])),
        }),
    };
    frame.validate().unwrap();
    let mut gpu = h.photo.upload_instances(&h.device, &h.queue, &frame);
    let (records, _) = instanced::rigid_frame_lit(
        &gpu.rigid_meshes,
        &frame.instances,
        &gpu.rigid_vertex_offsets,
        frame.vertex_lights.as_ref(),
    );
    assert_eq!(records[0].light, u32::MAX);
    assert_eq!(records[0].ambient, 1);
    assert_texels(h.pixels(&gpu, 3..6), repaired, None, [0; 4]);
    let grown = [61, 53, 47, 193];
    let relocated = [109, 97, 83, 211];
    let mut texels = vec![[0; 4]; 4100];
    texels[4094..4097].fill(grown);
    texels[10..13].fill(relocated);
    frame.vertex_lights = Some(VertexLightStream {
        texels: Arc::new(texels),
        ranges: Arc::new(BTreeMap::from([(7, 4094)])),
    });
    h.photo
        .write_instance_lights(&h.device, &h.queue, &mut gpu, frame.vertex_lights.as_ref());
    gpu.write_instances(&h.device, &h.queue, &frame);
    assert_eq!(gpu.light_rows, 2);
    assert_texels(h.pixels(&gpu, 3..6), grown, None, [0; 4]);
    // Keep the texels Arc and change only the range: transform records must
    // still change even though the texture upload cache has a hit.
    frame.vertex_lights.as_mut().unwrap().ranges = Arc::new(BTreeMap::from([(7, 10)]));
    h.photo
        .write_instance_lights(&h.device, &h.queue, &mut gpu, frame.vertex_lights.as_ref());
    gpu.write_instances(&h.device, &h.queue, &frame);
    assert_texels(h.pixels(&gpu, 3..6), relocated, None, [0; 4]);
    frame.vertex_lights = None;
    h.photo
        .write_instance_lights(&h.device, &h.queue, &mut gpu, None);
    gpu.write_instances(&h.device, &h.queue, &frame);
    assert_texels(h.pixels(&gpu, 3..6), direct, None, [0; 4]);
    assert!(pollster::block_on(validation.pop()).is_none());
}
