//! Native texture and material-binding evidence for the portable mip recipes.
use super::*;
use sha2::{Digest, Sha256};
use verse_engine::mips::{Role, Variant};

fn read(
    renderer: &Renderer,
    texture: &wgpu::Texture,
    level: u32,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let row = (width * 4).div_ceil(256) * 256;
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mip verification readback"),
        size: u64::from(row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: level,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    renderer.queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
    renderer
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        })
        .unwrap();
    receive
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let bytes = buffer.slice(..).get_mapped_range();
    let result = bytes
        .chunks(row as usize)
        .flat_map(|row| row[..(width * 4) as usize].iter().copied())
        .collect();
    drop(bytes);
    buffer.unmap();
    result
}

fn probe(renderer: &Renderer, key: material_gpu::Key, mask: bool) -> Vec<u8> {
    let shader = renderer
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("material mip probe"),
            source: wgpu::ShaderSource::Wgsl(include_str!("mip_probe.wgsl").into()),
        });
    let layout = renderer
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&renderer.material_layout)],
            immediate_size: 0,
        });
    let pipeline = renderer
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("material mip probe"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(if mask { "mask" } else { "channels" }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
    let width = if mask { 64 } else { 320 };
    let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("material mip evidence"),
        size: wgpu::Extent3d {
            width,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
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
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &renderer.materials[&key], &[]);
        pass.draw(0..3, 0..1);
    }
    renderer.queue.submit([encoder.finish()]);
    read(renderer, &target, 0, width, 64)
}

#[test]
#[ignore = "requires a native GPU; uses generated assets and no display"]
fn uploaded_roles_and_material_bindings_match_linear_light_and_mask_recipes() {
    let assets = tempfile::tempdir().unwrap();
    let mut pack = verse_content::compiler::original::generate(assets.path()).unwrap();
    let rgba: Vec<u8> = (0..16)
        .flat_map(|i| {
            if (i % 4 + i / 4) % 2 == 0 {
                [255; 4]
            } else {
                [0; 4]
            }
        })
        .collect();
    let path = assets.path().join(&pack.textures[0].file);
    let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), 4, 4);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgba)
        .unwrap();
    pack.textures[0].width = 4;
    pack.textures[0].height = 4;
    pack.textures[0].sha256 = format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap()));
    let model = pack.models.get_mut("chamber").unwrap();
    let opaque = &mut model.surfaces[0];
    opaque.material.normal_texture = Some(0);
    opaque.material.metallic_roughness_texture = Some(0);
    opaque.material.occlusion_texture = Some(0);
    opaque.material.emissive_texture = Some(0);
    opaque.blend = 0;
    let opaque_key = material_gpu::Key::from_surface(opaque);
    let mut masked = opaque.clone();
    masked.blend = 1;
    masked.material.alpha_cutoff = 0.5;
    let mask_key = material_gpu::Key::from_surface(&masked);
    model.surfaces.push(masked);
    let atlas = Atlas::new(1.);
    let mut renderer = Renderer::new(pack, assets.path(), 320, 64, &atlas, &[]).unwrap();
    let mut levels = Vec::new();
    for role in [
        Role::Color,
        Role::Linear,
        Role::Normal,
        Role::masked(0.5, 1.).unwrap(),
    ] {
        let texture = &renderer.texture_variants[&Variant { texture: 0, role }].texture;
        let bytes = read(&renderer, texture, 1, 2, 2);
        match role {
            Role::Color => assert!(bytes.chunks_exact(4).all(|p| p == [188, 188, 188, 128])),
            Role::Linear => assert!(bytes.chunks_exact(4).all(|p| p == [128; 4])),
            Role::Normal => assert!(bytes.chunks_exact(4).all(|p| p == [128, 128, 255, 128])),
            Role::Mask { .. } => {
                assert_eq!(bytes.chunks_exact(4).filter(|p| p[3] >= 128).count(), 2);
            }
        }
        levels.push(serde_json::json!({"role": role, "mip1": bytes}));
    }
    let channels = probe(&renderer, opaque_key, false);
    let expected = [
        [128, 128, 128, 128],
        [128, 128, 255, 128],
        [128; 4],
        [128; 4],
        [128; 4],
    ];
    for (strip, expected) in expected.into_iter().enumerate() {
        let pixel = &channels[(32 * 320 + strip * 64 + 32) * 4..][..4];
        assert!(
            pixel.iter().zip(expected).all(|(&a, b)| a.abs_diff(b) <= 1),
            "strip {strip}: {pixel:?}"
        );
    }
    let mask = probe(&renderer, mask_key, true);
    let coverage = mask.chunks_exact(4).filter(|p| p[0] == 255).count() as f64 / 4096.;
    assert!((coverage - 0.5).abs() < 0.02, "coverage {coverage}");
    renderer
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(5)),
        })
        .unwrap();
    assets.close().unwrap();
    renderer.device.destroy();
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    while !renderer.recover_if_lost(&atlas).unwrap() {
        assert!(Instant::now() < deadline, "Device loss was not observed");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(renderer.device_recoveries, 1);
    assert_eq!(probe(&renderer, opaque_key, false), channels);
    assert_eq!(probe(&renderer, mask_key, true), mask);
    println!(
        "{}",
        serde_json::json!({ "device": renderer.device_profile, "mips": levels, "binding_samples": expected,
        "mask_coverage": coverage, "device_recoveries": renderer.device_recoveries, "sources_deleted_before_recovery": true, "channels_sha256": format!("{:x}", Sha256::digest(&channels)), "mask_sha256": format!("{:x}", Sha256::digest(&mask)) })
    );
}
