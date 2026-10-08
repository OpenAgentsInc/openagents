//! Matching original-flipbook and optical-fire captures through Verse's physical renderer.
//! Usage: fire_capture OUTPUT_DIRECTORY
use glam::{Mat4, Vec3};
use std::path::Path;
use verse_engine::{presentation::View, quality::Tier};
use verse_pbr::{
    fx::{Facing, Sprite, sheet, vertices},
    pbr::{
        self, Key, Lamp, LitVertex, Material, Neon,
        gpu::{Batches, Capability, Photo, Stage},
    },
};
use wgpu::util::DeviceExt;
const WIDTH: u32 = 1600;
const HEIGHT: u32 = 1000;
fn view() -> View {
    let eye = Vec3::new(0.0, 2.1, 6.5);
    View {
        eye,
        view_proj: Mat4::perspective_rh(0.7, WIDTH as f32 / HEIGHT as f32, 0.1, 100.0)
            * Mat4::look_at_rh(eye, Vec3::new(0.0, 1.2, 0.0), Vec3::Y),
    }
}
fn png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let mut encoder = png::Encoder::new(
        std::fs::File::create(path).map_err(|e| e.to_string())?,
        WIDTH,
        HEIGHT,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())
}
fn quad(out: &mut Vec<LitVertex>, corners: [Vec3; 4], normal: Vec3, color: [f32; 3]) {
    for i in [0, 1, 2, 0, 2, 3] {
        out.push(LitVertex {
            pos: corners[i].to_array(),
            normal: normal.to_array(),
            tangent: Vec3::X.to_array(),
            local: corners[i].to_array(),
            color,
            params: [0.0, 0.85, Material::Stage.code(), 1.0],
        });
    }
}
fn geometry() -> Vec<LitVertex> {
    let mut v = vec![];
    quad(
        &mut v,
        [
            Vec3::new(-5., 0., 4.),
            Vec3::new(5., 0., 4.),
            Vec3::new(5., 0., -3.),
            Vec3::new(-5., 0., -3.),
        ],
        Vec3::Y,
        [0.14, 0.13, 0.12],
    );
    quad(
        &mut v,
        [
            Vec3::new(-5., 0., -3.),
            Vec3::new(5., 0., -3.),
            Vec3::new(5., 5., -3.),
            Vec3::new(-5., 5., -3.),
        ],
        Vec3::Z,
        [0.08, 0.075, 0.07],
    );
    for x in [-1.8, 0., 1.8] {
        quad(
            &mut v,
            [
                Vec3::new(x - 0.42, 0., 0.1),
                Vec3::new(x + 0.42, 0., 0.1),
                Vec3::new(x + 0.42, 0.55, 0.1),
                Vec3::new(x - 0.42, 0.55, 0.1),
            ],
            Vec3::Z,
            [0.05, 0.04, 0.035],
        );
    }
    v
}
fn sprites(time: f32) -> Vec<Sprite> {
    let mut result = vec![];
    for (b, x) in [-1.8, 0.0, 1.8].into_iter().enumerate() {
        for i in 0..45 {
            let age = (i as f32 * 0.061 + time * 0.72 + b as f32 * 0.17).fract();
            let angle = i as f32 * 2.399963;
            let offset = Vec3::new(
                angle.cos() * 0.19 * (1.0 - age),
                0.62 + age * 1.2,
                angle.sin() * 0.12,
            );
            let s = sheet::find("fireball").unwrap();
            let frame = (age * 11.0) as u32 + 1;
            result.push(Sprite {
                at: Vec3::new(x, 0., 0.) + offset,
                half: (0.32 * (1.0 - age) + 0.07),
                angle: 0.12 * (time + i as f32).sin(),
                tail: Vec3::new(0., 0.1, 0.),
                facing: Facing::Camera,
                color: [6., 4., 2.],
                alpha: (1.0 - age) * 0.7,
                additive: 0.85,
                lit: false,
                scene_lit: false,
                density: 1.0,
                layer: 0,
                rect_a: s.rect(frame),
                rect_b: s.rect((frame + 1).min(15)),
                mix: (age * 11.).fract(),
                priority: 5,
            });
        }
        for i in 0..10 {
            let age = (i as f32 * 0.11 + time * 0.22).fract();
            let s = sheet::find("smoke").unwrap();
            let frame = (age * 15.) as u32;
            result.push(Sprite {
                at: Vec3::new(x + 0.2 * (time + age * 4.).sin(), 1.2 + age * 1.8, -0.06),
                half: 0.24 + age * 0.45,
                angle: age,
                tail: Vec3::ZERO,
                facing: Facing::Camera,
                color: [0.13, 0.12, 0.11],
                alpha: 0.2 * (1. - age),
                additive: 0.,
                lit: true,
                scene_lit: false,
                density: 1.0,
                layer: 1,
                rect_a: s.rect(frame),
                rect_b: s.rect((frame + 1).min(15)),
                mix: 0.0,
                priority: 2,
            });
        }
    }
    result
}
fn main() -> Result<(), String> {
    let directory = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Expected an output directory")?,
    );
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .map_err(|e| e.to_string())?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(|e| e.to_string())?;
    let mut photo = Photo::new(
        &device,
        &queue,
        Capability {
            hdr: Some(wgpu::TextureFormat::Rgba16Float),
            samples: 1,
            gles: false,
            quality: Tier::High.quality(),
        },
        wgpu::TextureFormat::Rgba8UnormSrgb,
    )?;
    let mut stage = Neon::neutral(0.0);
    stage.field = [0.012, 0.01, 0.008];
    stage.grade = verse_engine::lighting::Grade::NEUTRAL;
    stage.bloom = 0.012;
    stage.vignette = 0.0;
    stage.fog_start = 30.;
    stage.fog_end = 50.;
    stage.key = Some(Key {
        dir: Vec3::new(0.3, 0.8, 0.4).normalize(),
        illuminance: 30.,
        angular_radius: 0.06,
        rim_dir: Vec3::Y,
        rim_illuminance: 0.,
        rim_angular_radius: 0.1,
        sky: 0.8,
        ground: 0.1,
        ev100: 5.,
        shadow_center: Vec3::ZERO,
        shadow_half: 6.,
        shadow_distance: None,
        cache_far_shadows: false,
    });
    for (i, x) in [-1.8, 0.0, 1.8].into_iter().enumerate() {
        stage.lamps[i] = Lamp {
            position: Vec3::new(x, 0.8, 0.),
            color: [1.0, 0.4, 0.08],
            intensity: 110.,
            range: 5.,
        };
    }
    let geometry = geometry();
    let mut records = vec![];
    for (label, time) in [("early", 0.8), ("late", 1.7)] {
        stage.time = time;
        let mut quads = vec![];
        vertices(
            &sprites(time),
            view().eye,
            verse_pbr::fx::budget(Tier::High),
            &mut quads,
        );
        photo.sprites.write(&device, &queue, &quads);
        for (mode, enabled) in [("before", false), ("after", true)] {
            photo.fire_volumes = enabled;
            let pixels =
                physical_capture(&mut photo, &device, &queue, Stage::Neon(&stage), &geometry);
            let name = format!("{mode}-{label}.png");
            png(&directory.join(&name), &pixels)?;
            records.push(serde_json::json!({"image":name,"seconds":time,"fire_volumes":enabled,"sprite_vertices":quads.len(),"resolution":[WIDTH,HEIGHT],"adapter":format!("{:?}",adapter.get_info())}));
        }
    }
    imported_captures(&directory)?;
    std::fs::write(
        directory.join("capture.json"),
        serde_json::to_vec_pretty(&records).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
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
        tx.send(r).expect("Fire capture failed");
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(10)),
        })
        .expect("Fire capture failed");
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .expect("Fire capture failed")
        .expect("Fire capture failed");
    let bytes = readback.slice(..).get_mapped_range();
    bytes
        .chunks(row as usize)
        .flat_map(|row| row[..WIDTH as usize * 4].iter().copied())
        .collect()
}

fn imported_captures(directory: &Path) -> Result<(), String> {
    use verse_engine::{assets::Topology, lighting::Lighting, presentation::Instance};
    use verse_pbr::{
        imported::{Renderer, flat},
        ui::{Atlas, UiBatch},
    };
    let assets = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut pack = verse_content::compiler::original::generate(assets.path())?;
    let texture = flat::white_texture(&mut pack, assets.path())?;
    let mut surface = flat::surface(texture, [0.12, 0.08, 0.05], Topology::Triangles);
    flat::quad(
        &mut surface,
        [
            Vec3::new(-5., 0., -3.),
            Vec3::new(5., 0., -3.),
            Vec3::new(5., 0., 3.),
            Vec3::new(-5., 0., 3.),
        ],
    );
    pack.models.insert(
        "fixture-floor".into(),
        flat::model("verse/fixture/floor", vec![surface], 1.),
    );
    let floor = Instance {
        model: "fixture-floor".into(),
        actor: None,
        mount: None,
        transform: Mat4::IDENTITY,
        animation: 0.into(),
        time: 0.,
        animation_epoch: None,
        emission: Vec3::ONE,
    };
    let mut renderer = Renderer::new(
        pack,
        assets.path(),
        WIDTH,
        HEIGHT,
        &Atlas::new(1.0),
        &[floor],
    )?;
    for (label, time) in [("early", 0.8), ("late", 1.7)] {
        let particles: Vec<_> = sprites(time)
            .into_iter()
            .filter(|s| s.layer == 0)
            .map(|s| Instance {
                model: "effect-fire".into(),
                actor: None,
                mount: None,
                transform: Mat4::from_scale_rotation_translation(
                    Vec3::splat(s.half),
                    glam::Quat::IDENTITY,
                    s.at,
                ),
                animation: 0.into(),
                time,
                animation_epoch: None,
                emission: Vec3::new(s.alpha, 0., 0.),
            })
            .collect();
        let lighting = Lighting {
            time,
            density: 0.,
            ..Default::default()
        };
        for (mode, enabled) in [("before", false), ("after", true)] {
            renderer.fire_volumes = enabled;
            let pixels = renderer.draw(view(), &particles, &UiBatch::default(), &lighting)?;
            png(
                &directory.join(format!("battle-{mode}-{label}.png")),
                &pixels,
            )?;
        }
    }
    Ok(())
}
