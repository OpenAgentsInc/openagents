//! Explicit offscreen comparisons for the declared lighting profiles.
use crate::pbr::{
    self,
    gpu::{Batches, Capability, Photo, Stage},
};
use glam::{Mat4, Vec3};
use verse_engine::{
    assets::{Pack, Surface, Topology},
    lighting::{FrameLighting, Light, Lighting},
    presentation::{Instance, View},
    quality::Tier,
};
use wgpu::util::DeviceExt;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;
fn view() -> View {
    let eye = Vec3::new(0.0, 1.6, 5.0);
    View {
        eye,
        view_proj: Mat4::perspective_rh(
            45f32.to_radians(),
            WIDTH as f32 / HEIGHT as f32,
            0.1,
            100.0,
        ) * Mat4::look_at_rh(eye, Vec3::new(0.0, 0.9, 0.0), Vec3::Y),
    }
}
fn instance(model: &str) -> Instance {
    Instance {
        model: model.into(),
        actor: None,
        mount: None,
        transform: verse_content::basis(),
        animation: 0.into(),
        time: 0.0,
        animation_epoch: None,
        emission: Vec3::ONE,
    }
}
fn plane(pack: &mut Pack, name: &str, corners: [Vec3; 4], color: [f32; 3], normal: Vec3) {
    let mut surface = crate::imported::flat::surface(0, color, Topology::Triangles);
    surface.unlit = false;
    crate::imported::flat::quad(&mut surface, corners);
    let inverse = verse_content::basis().inverse();
    for vertex in &mut surface.vertices {
        vertex.position = inverse
            .transform_point3(Vec3::from_array(vertex.position))
            .to_array();
        vertex.normal = inverse.transform_vector3(normal).normalize().to_array();
    }
    pack.models.insert(
        name.into(),
        crate::imported::flat::model(name, vec![surface], 6.0),
    );
}
fn png(path: &std::path::Path, pixels: &[u8]) {
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(pixels)
        .unwrap();
}
fn readability(body: &[u8], background: &[u8]) -> serde_json::Value {
    let mut differences = vec![];
    // Central character region, excluding its ground shadow.
    for y in 120..280 {
        for x in 275..365 {
            let i = (y * WIDTH as usize + x) * 4;
            let difference = (0..3)
                .map(|c| body[i + c].abs_diff(background[i + c]))
                .max()
                .unwrap();
            if difference > 4 {
                differences.push(difference);
            }
        }
    }
    differences.sort_unstable();
    assert!(
        differences.len() > 100,
        "Character must remain visible: {} pixels",
        differences.len()
    );
    let p95 = differences[differences.len() * 95 / 100];
    assert!(
        p95 > 12,
        "Character contrast must exceed 12 display code values: {p95}"
    );
    serde_json::json!({"changed_pixels":differences.len(),"difference_p95":p95,"region":[275,120,365,280],"metric":"maximum RGB code-value difference against identical frame without character"})
}
fn triangles(pack: &Pack, names: &[&str]) -> Vec<pbr::LitVertex> {
    let mut result = vec![];
    for name in names {
        let model = &pack.models[*name];
        let palette = verse_engine::animation::pose(model, 0, 0.0);
        for Surface {
            vertices,
            indices,
            tint,
            material,
            ..
        } in &model.surfaces
        {
            for &index in indices {
                let vertex = &vertices[index as usize];
                let mut skin = Mat4::ZERO;
                for i in 0..4 {
                    skin += palette
                        .get(vertex.joints[i] as usize)
                        .copied()
                        .unwrap_or(Mat4::IDENTITY)
                        * vertex.weights[i];
                }
                let transform = verse_content::basis() * skin;
                let position = transform.transform_point3(Vec3::from_array(vertex.position));
                let normal = transform
                    .transform_vector3(Vec3::from_array(vertex.normal))
                    .normalize();
                let axis = if normal.x.abs() > 0.9 {
                    Vec3::Z
                } else {
                    Vec3::X
                };
                result.push(pbr::LitVertex {
                    pos: position.to_array(),
                    normal: normal.to_array(),
                    tangent: normal.cross(axis).normalize().to_array(),
                    local: position.to_array(),
                    color: *tint,
                    params: [material.metallic, material.roughness, 0.0, 1.0],
                });
            }
        }
    }
    result
}
fn physical_capture(
    photo: &mut Photo,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    stage: Stage<'_>,
    geometry: &[pbr::LitVertex],
) -> Vec<u8> {
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Lighting fixture geometry"),
        contents: bytemuck::cast_slice(geometry),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let mut targets = photo.targets(device, WIDTH, HEIGHT);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Lighting fixture capture"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let row = (WIDTH * 4).div_ceil(256) * 256;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * HEIGHT),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let output = texture.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    photo.encode(
        device,
        queue,
        &mut encoder,
        &output,
        &mut targets,
        view(),
        stage,
        Batches {
            streamed: None,
            lit: (&buffer, geometry.len() as u32),
            faces: [(&buffer, 0); 2],
            lines: [(&buffer, 0); 2],
            textured: None,
            figure: None,
            instances: [None, None],
            water: None,
        },
        None,
    );
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
                bytes_per_row: Some(row),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        tx.send(r).unwrap();
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        })
        .unwrap();
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .unwrap()
        .unwrap();
    let bytes = readback.slice(..).get_mapped_range();
    bytes
        .chunks(row as usize)
        .flat_map(|row| row[..WIDTH as usize * 4].iter().copied())
        .collect()
}
#[test]
#[ignore = "Explicit native GPU and private scratch output; run once per VERSE_QUALITY tier"]
fn indoor_and_outdoor_lighting_keep_the_character_readable() {
    let output = std::path::PathBuf::from(
        std::env::var_os("VERSE_LIGHTING_EVIDENCE").expect("Explicit evidence directory"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let tier = Tier::parse(&std::env::var("VERSE_QUALITY").expect("Explicit tier")).unwrap();
    let assets = tempfile::tempdir().unwrap();
    let mut pack = verse_content::compiler::original::generate(assets.path()).unwrap();
    plane(
        &mut pack,
        "fixture-floor",
        [
            Vec3::new(-8., 0., -8.),
            Vec3::new(8., 0., -8.),
            Vec3::new(8., 0., 8.),
            Vec3::new(-8., 0., 8.),
        ],
        [0.08, 0.09, 0.08],
        Vec3::Y,
    );
    plane(
        &mut pack,
        "fixture-back",
        [
            Vec3::new(-8., 0., -2.),
            Vec3::new(8., 0., -2.),
            Vec3::new(8., 6., -2.),
            Vec3::new(-8., 6., -2.),
        ],
        [0.035, 0.06, 0.035],
        Vec3::Z,
    );
    for (name, x, tint) in [
        ("gray-card", -1.8, [0.18; 3]),
        ("metal-card", 1.8, [0.65; 3]),
    ] {
        plane(
            &mut pack,
            name,
            [
                Vec3::new(x - 0.3, 0.6, 0.0),
                Vec3::new(x + 0.3, 0.6, 0.0),
                Vec3::new(x + 0.3, 1.2, 0.0),
                Vec3::new(x - 0.3, 1.2, 0.0),
            ],
            tint,
            Vec3::Z,
        );
        if name == "metal-card" {
            let material = &mut pack.models.get_mut(name).unwrap().surfaces[0].material;
            material.metallic = 1.0;
            material.roughness = 0.2;
        }
    }
    let background = [
        instance("fixture-floor"),
        instance("fixture-back"),
        instance("gray-card"),
        instance("metal-card"),
    ];
    let mut full = background.to_vec();
    full.push(instance("adventurer"));
    let content_digest = {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(serde_json::to_vec(&pack).unwrap()))
    };
    let mut renderer = crate::imported::Renderer::new(
        pack.clone(),
        assets.path(),
        WIDTH,
        HEIGHT,
        &crate::ui::Atlas::new(1.0),
        &[],
    )
    .unwrap();
    let mut records = vec![];
    for name in ["torch", "spell"] {
        let mut lighting = Lighting {
            density: 0.0,
            ambient: Vec3::splat(0.08),
            ..Default::default()
        };
        lighting.lights = (0..32)
            .map(|i| Light {
                position: Vec3::new(0., 2., 2.),
                color: if name == "torch" {
                    Vec3::new(1., 0.6, 0.25)
                } else {
                    Vec3::new(0.3, 0.6, 1.)
                },
                intensity: if i == 31 { 4.0 } else { 0.02 },
                range: 8.0,
            })
            .collect();
        let no_body = renderer
            .draw(view(), &background, &Default::default(), &lighting)
            .unwrap();
        let body = renderer
            .draw(view(), &full, &Default::default(), &lighting)
            .unwrap();
        let diagnostics = &renderer.last_lighting;
        assert!(diagnostics.shadow_views as u32 <= tier.quality().local_shadow_views());
        assert!(diagnostics.shadowed_points.contains(&31));
        png(&output.join(format!("{name}-{}.png", tier.name())), &body);
        records.push(serde_json::json!({"scene":name,"tier":tier.name(),"lighting":diagnostics,"readability":readability(&body,&no_body),"timings":renderer.last_timings,"device":renderer.device_profile}));
    }
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    let capability = Capability {
        hdr: Some(wgpu::TextureFormat::Rgba16Float),
        samples: 1,
        gles: false,
        quality: tier.quality(),
    };
    let mut photo = Photo::new(
        &device,
        &queue,
        capability,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    )
    .unwrap();
    let without = triangles(
        &pack,
        &["fixture-floor", "fixture-back", "gray-card", "metal-card"],
    );
    let with = triangles(
        &pack,
        &[
            "fixture-floor",
            "fixture-back",
            "gray-card",
            "metal-card",
            "adventurer",
        ],
    );
    for name in ["forest", "station"] {
        let mut stage = pbr::Neon::neutral(0.0);
        stage.bloom = 0.0;
        stage.vignette = 0.0;
        stage.fog_start = 100.0;
        stage.fog_end = 200.0;
        stage.grade = verse_engine::lighting::Grade::NEUTRAL;
        stage.key = Some(pbr::Key {
            dir: Vec3::new(0.3, 0.6, 1.0).normalize(),
            illuminance: if name == "forest" {
                20_000.0
            } else {
                130_000.0
            },
            angular_radius: 0.01,
            rim_dir: Vec3::new(-1., 1., 0.).normalize(),
            rim_illuminance: 1_000.0,
            rim_angular_radius: 0.1,
            sky: 4_000.0,
            ground: 1_000.0,
            ev100: if name == "forest" { 12.0 } else { 15.0 },
            shadow_center: Vec3::ZERO,
            shadow_half: 10.0,
            shadow_distance: Some(30.0),
            cache_far_shadows: false,
        });
        if name == "forest" {
            stage.daylight = Some(pbr::Daylight {
                zenith: [0.1, 0.3, 0.73],
                horizon: [0.45, 0.55, 0.6],
                sun: [1., 0.8, 0.54],
                clouds: 0.38,
                ground: [0.1, 0.11, 0.07],
                glow: 0.0,
            });
        }
        for i in 0..32 {
            stage.lamps[i] = pbr::Lamp {
                position: Vec3::new(0., 2., 2.),
                color: [1., 0.6, 0.3],
                intensity: if i == 31 { 100.0 } else { 0.1 },
                range: 8.0,
            };
        }
        let key = stage.key.unwrap();
        let body_at = |dir| pbr::Body {
            dir,
            angular_radius: 0.001,
            distance: 1_000_000.0,
            axes: glam::Mat3::IDENTITY,
        };
        let sky = pbr::Sky {
            sun_dir: key.dir,
            sun_illuminance: key.illuminance,
            sun_angular_radius: 0.004,
            sun_visible: 0.0,
            earth: body_at(-Vec3::Y),
            moon: body_at(-Vec3::Y),
            celestial: glam::Mat3::IDENTITY,
            shadow_center: Vec3::ZERO,
            shadow_half: 10.0,
            camera: pbr::Camera {
                ev100: 15.0,
                auto_exposure: false,
                bloom: 0.0,
                grain: 0.0,
                vignette: 0.0,
                local_exposure: 0.0,
                ghosts: 0.0,
                ..pbr::Camera::helmet()
            },
            probes: None,
            time: 0.0,
        };
        if name == "station" {
            photo.prepare_space(&device, &queue).unwrap();
        }
        let stage_for_frame = || {
            if name == "station" {
                Stage::Space(&sky)
            } else {
                Stage::Neon(&stage)
            }
        };
        let no_body = physical_capture(&mut photo, &device, &queue, stage_for_frame(), &without);
        let body = physical_capture(&mut photo, &device, &queue, stage_for_frame(), &with);
        let diagnostics: &FrameLighting = &photo.last_lighting;
        assert!(diagnostics.shadow_views <= tier.quality().cascades as usize);
        if name == "forest" {
            assert_eq!(diagnostics.selected_points[0], 31);
        } else {
            assert!(diagnostics.selected_points.is_empty());
            assert_eq!(diagnostics.shadow_views, 1);
        }
        assert!(diagnostics.selected_points.len() <= pbr::gpu::lamp_budget(tier));
        png(&output.join(format!("{name}-{}.png", tier.name())), &body);
        records.push(serde_json::json!({"scene":name,"tier":tier.name(),"lighting":diagnostics,"readability":readability(&body,&no_body),"adapter":format!("{:?}",adapter.get_info()),"profile":"procedural character and backdrop; forest uses daylight stage, station uses physical space sky; not the full zone"}));
    }
    std::fs::write(output.join(format!("{}.json",tier.name())),serde_json::to_vec_pretty(&serde_json::json!({"schema":"verse.lighting.acceptance.v1","content_sha256":content_digest,"viewport":[WIDTH,HEIGHT],"records":records})).unwrap()).unwrap();
}
