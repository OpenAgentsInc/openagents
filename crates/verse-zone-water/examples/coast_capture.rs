//! The coastal test scene through the physical renderer at every tier
//! (`docs/verse/water.md`, phase W10): the clipmap ocean over its streamed
//! field, with swell, surf, and buoyant bodies, in the bay the coast zone
//! is laid out as (`verse_zone_water::coast`).
//!
//! Usage: coast_capture OUTPUT_DIRECTORY
//!
//! Renders five views at Low, Medium, and High (the bay from the arrival
//! terrace, the surf along Driftwood Beach, the harbor behind its
//! breakwater, the bodies afloat off the beach, and the bay from high over
//! the water with the rings out to the horizon), streaming the field until
//! the pages around the eye are in place, and writes the PNGs and
//! `capture.json`: per picture its digest, the clipmap's triangles, the
//! field's resident bytes against its budget, and, at 1920 by 1080, the
//! frame's time with and without its water (the fastest of several
//! batches), whose difference is the water's GPU time.
//! `COAST_CAPTURE_TIERS` and `COAST_CAPTURE_VIEWS` (comma-separated lists)
//! narrow the run.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use glam::{Mat4, Quat, Vec3};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use verse_engine::{presentation::View, quality::Tier};
use verse_pbr::pbr::{
    Daylight, Key, LitVertex, Material, Neon,
    gpu::{Batches, Capability, Photo, Stage, WaterGpu},
};
use verse_zone_water::coast::{self, Buoys};
use verse_zone_water::floats::Kind;
use wgpu::util::DeviceExt;

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;
const BUDGET_SIZE: [u32; 2] = [1920, 1080];
const ROUNDS: usize = 7;
const BATCH: usize = 10;
const TIERS: [Tier; 3] = [Tier::Low, Tier::Medium, Tier::High];
/// The sea state every view shows.
const SEA: &str = "moderate";

struct ViewSpec {
    name: &'static str,
    eye: Vec3,
    target: Vec3,
    /// The Water Lab's cove rather than the coast: its sea, now on the
    /// clipmap, with its river, pool, and falls.
    lab: bool,
}

fn views() -> Vec<ViewSpec> {
    let all = vec![
        // From the arrival terrace over the dunes, the beach, and the bay
        // to Gull Island.
        ViewSpec {
            name: "bay",
            eye: Vec3::new(100.0, 45.0, -100.0),
            target: Vec3::new(-40.0, -20.0, 120.0),
            lab: false,
        },
        // Standing on the wet sand at the beach's middle, looking along it
        // at the surf.
        ViewSpec {
            name: "surf",
            eye: Vec3::new(31.0, 2.2, 29.0),
            target: Vec3::new(32.0, 0.0, 93.0),
            lab: false,
        },
        ViewSpec {
            name: "harbor",
            eye: Vec3::new(-80.0, 8.0, -235.0),
            target: Vec3::new(-170.0, 0.0, -200.0),
            lab: false,
        },
        ViewSpec {
            name: "buoys",
            eye: Vec3::new(-7.0, 2.6, 44.0),
            target: Vec3::new(-7.0, 0.0, 68.0),
            lab: false,
        },
        ViewSpec {
            name: "rings",
            eye: Vec3::new(40.0, 70.0, 120.0),
            target: Vec3::new(-120.0, 0.0, 330.0),
            lab: false,
        },
        // The Water Lab from its spawn on the beach, and over its cove.
        ViewSpec {
            name: "lab",
            eye: Vec3::new(4.0, 2.4, 16.0),
            target: Vec3::new(4.0, 0.0, -40.0),
            lab: true,
        },
        ViewSpec {
            name: "lab-cove",
            eye: Vec3::new(70.0, 28.0, 50.0),
            target: Vec3::new(-10.0, 0.0, -60.0),
            lab: true,
        },
    ];
    let only = std::env::var("COAST_CAPTURE_VIEWS").ok();
    all.into_iter()
        .filter(|v| {
            only.as_deref()
                .is_none_or(|o| o.split(',').any(|n| n == v.name))
        })
        .collect()
}

fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Low => "low",
        Tier::Medium => "medium",
        Tier::High => "high",
    }
}

/// The coast specification's fog end and draw distance per tier, m.
fn fog_end(tier: Tier) -> f32 {
    match tier {
        Tier::Low => 600.0,
        Tier::Medium => 1_200.0,
        Tier::High => 2_000.0,
    }
}

fn camera(spec: &ViewSpec, size: [u32; 2], tier: Tier) -> View {
    View {
        eye: spec.eye,
        view_proj: Mat4::perspective_rh(
            0.9,
            size[0] as f32 / size[1] as f32,
            0.3,
            fog_end(tier) + 200.0,
        ) * Mat4::look_at_rh(spec.eye, spec.target, Vec3::Y),
    }
}

// ---- The scene's ground and bodies.

const SAND: [f32; 3] = [0.62, 0.55, 0.42];
const WET_SAND: [f32; 3] = [0.42, 0.37, 0.28];
const GRASS: [f32; 3] = [0.24, 0.33, 0.15];
const ROCK: [f32; 3] = [0.36, 0.34, 0.31];
const WOOD: [f32; 3] = [0.45, 0.30, 0.17];

fn lit(p: Vec3, n: Vec3, color: [f32; 3], roughness: f32) -> LitVertex {
    LitVertex {
        pos: p.to_array(),
        normal: n.to_array(),
        tangent: n.any_orthonormal_vector().to_array(),
        local: p.to_array(),
        color,
        params: [0.0, roughness, Material::Stage.code(), 1.0],
    }
}

/// The ground's color at a point of height `y` whose normal rises `up`:
/// rock on cliffs and the breakwater, wet sand at the water, dry sand,
/// then grass.
fn ground_color(y: f32, up: f32) -> [f32; 3] {
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| {
        let t = t.clamp(0.0, 1.0);
        [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
    };
    let land = mix(mix(WET_SAND, SAND, (y - 0.2) / 0.6), GRASS, (y - 2.5) / 1.5);
    mix(land, ROCK, (0.85 - up) / 0.15)
}

/// The ground `h` as triangles with smooth normals and colors, `step` m
/// apart over the square `half` m around `center`, and a bed `bed` m down
/// beyond it out past the clipmap's apron, so the water refracts a bed
/// everywhere.
fn ground(
    h: fn(f32, f32) -> f32,
    center: [f32; 2],
    half: f32,
    step: f32,
    bed: f32,
) -> Vec<LitVertex> {
    let n = (2.0 * half / step) as usize;
    let at = |c: usize, r: usize| {
        let x = center[0] - half + c as f32 * step;
        let z = center[1] - half + r as f32 * step;
        let e = 0.5;
        let normal = Vec3::new(
            h(x - e, z) - h(x + e, z),
            2.0 * e,
            h(x, z - e) - h(x, z + e),
        )
        .normalize();
        let y = h(x, z);
        lit(Vec3::new(x, y, z), normal, ground_color(y, normal.y), 0.9)
    };
    let mut out = Vec::with_capacity(n * n * 6 + 24);
    for r in 0..n {
        for c in 0..n {
            let quad = [at(c, r), at(c + 1, r), at(c + 1, r + 1), at(c, r + 1)];
            for i in [0, 2, 1, 0, 3, 2] {
                out.push(quad[i]);
            }
        }
    }
    // The open sea's bed from the square out to 5 km.
    let far = 5_000.0;
    let y = -bed;
    let (cx, cz) = (center[0], center[1]);
    let at = |(x, z): (f32, f32)| Vec3::new(cx + x, y, cz + z);
    for (a, b, c, d) in [
        ((-far, -far), (far, -far), (far, -half), (-far, -half)),
        ((-far, half), (far, half), (far, far), (-far, far)),
        ((-far, -half), (-half, -half), (-half, half), (-far, half)),
        ((half, -half), (far, -half), (far, half), (half, half)),
    ] {
        let q = [a, b, c, d].map(|p| lit(at(p), Vec3::Y, SAND, 0.9));
        for i in [0, 2, 1, 0, 3, 2] {
            out.push(q[i]);
        }
    }
    out
}

/// The buoyant bodies as boxes at their poses.
fn bodies(poses: &[(Kind, glam::DVec3, glam::DQuat)]) -> Vec<LitVertex> {
    let mut out = Vec::new();
    for (kind, pos, rot) in poses {
        let h = kind.half().as_vec3();
        let (p, q): (Vec3, Quat) = (pos.as_vec3(), rot.as_quat());
        for (axis, sign) in [
            (0, 1.0),
            (0, -1.0),
            (1, 1.0),
            (1, -1.0),
            (2, 1.0),
            (2, -1.0),
        ] {
            let mut n = Vec3::ZERO;
            n[axis] = sign;
            let (u, v) = match axis {
                0 => (Vec3::Y, Vec3::Z),
                1 => (Vec3::Z, Vec3::X),
                _ => (Vec3::X, Vec3::Y),
            };
            let corner = |a: f32, b: f32| p + q * ((n + u * a + v * b) * h);
            let c = [
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            ];
            let normal = q * n;
            let order = if sign > 0.0 {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            };
            for i in order {
                out.push(lit(c[i], normal, WOOD, 0.7));
            }
        }
    }
    out
}

fn stage(tier: Tier, water: Option<verse_pbr::water::Water>, time: f32) -> Neon {
    // Golden hour over the bay: the sun low in the southwest.
    let sun = Vec3::new(-0.55, 0.22, 0.8).normalize();
    let haze = [0.78, 0.64, 0.50];
    let color = [1.0, 0.66, 0.36];
    Neon {
        field: haze,
        fog_start: fog_end(tier) * 0.25,
        fog_end: fog_end(tier),
        line_gain: 1.0,
        line_width: 1.4,
        bloom: 0.06,
        vignette: 0.25,
        time,
        key: Some(Key {
            dir: sun,
            illuminance: 3_000.0,
            angular_radius: 0.03,
            rim_dir: Vec3::new(0.5, 0.45, -0.7).normalize(),
            rim_illuminance: 400.0,
            rim_angular_radius: 0.2,
            sky: 560.0,
            ground: 190.0,
            ev100: 10.0,
            shadow_center: Vec3::ZERO,
            shadow_half: 120.0,
            shadow_distance: Some(200.0),
            cache_far_shadows: false,
        }),
        daylight: Some(Daylight {
            zenith: [0.08, 0.17, 0.42],
            horizon: haze,
            sun: color,
            clouds: 0.25,
            ground: [0.3, 0.27, 0.2],
            glow: 0.5,
        }),
        key_color: color,
        rim_color: [0.55, 0.65, 1.0],
        water,
        ..Neon::plaza(time)
    }
}

// ---- The GPU.

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: String,
}

fn gpu() -> Result<Gpu, String> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .map_err(|e| e.to_string())?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(|e| e.to_string())?;
    let info = adapter.get_info();
    Ok(Gpu {
        device,
        queue,
        adapter: format!("{} ({:?})", info.name, info.backend),
    })
}

fn capability(tier: Tier) -> Capability {
    Capability {
        hdr: (tier != Tier::Low).then_some(wgpu::TextureFormat::Rgba16Float),
        samples: if tier == Tier::Low { 1 } else { 4 },
        gles: false,
        quality: tier.quality(),
    }
}

fn wait(gpu: &Gpu) -> Result<(), String> {
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The ground and the bodies afloat, and the frame's water.
struct Scene {
    lit: wgpu::Buffer,
    count: u32,
    water: verse_pbr::water::Water,
    time: f32,
}

/// One view at `size`: its pixels at the capture size, else the fastest
/// batch's frame time, ms.
#[allow(clippy::too_many_arguments)]
fn render(
    gpu: &Gpu,
    photo: &mut Photo,
    tier: Tier,
    scene: &Scene,
    water: Option<&WaterGpu>,
    spec: &ViewSpec,
    size: [u32; 2],
) -> Result<(Vec<u8>, f64), String> {
    let neon = if spec.lab {
        let mut neon =
            verse_zone_water::sea::stage(scene.time, verse_zone_water::Hour::Golden, scene.water);
        if water.is_none() {
            neon.water = None;
        }
        neon
    } else {
        stage(tier, water.map(|_| scene.water), scene.time)
    };
    let [width, height] = size;
    let mut targets = photo.targets(&gpu.device, width, height);
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("coast capture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let output = texture.create_view(&Default::default());
    photo.ocean.exact = true;
    let view = camera(spec, size, tier);
    let mut frame = |photo: &mut Photo| {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        photo.encode(
            &gpu.device,
            &gpu.queue,
            &mut encoder,
            &output,
            &mut targets,
            view,
            Stage::Neon(&neon),
            Batches {
                streamed: None,
                lit: (&scene.lit, scene.count),
                faces: [(&scene.lit, 0); 2],
                lines: [(&scene.lit, 0); 2],
                textured: None,
                figure: None,
                water,
            },
            None,
        );
        encoder
    };
    // Stream the field's pages in, as frames would while the player
    // stands here.
    for _ in 0..48 {
        gpu.queue.submit([frame(photo).finish()]);
    }
    wait(gpu)?;
    if size == [WIDTH, HEIGHT] {
        return Ok((read(gpu, &texture, frame(photo))?, 0.0));
    }
    let mut fastest = f64::INFINITY;
    for round in 0..=ROUNDS {
        let started = Instant::now();
        for _ in 0..BATCH {
            gpu.queue.submit([frame(photo).finish()]);
        }
        wait(gpu)?;
        if round > 0 {
            fastest = fastest.min(started.elapsed().as_secs_f64() * 1e3 / BATCH as f64);
        }
    }
    Ok((Vec::new(), fastest))
}

fn read(
    gpu: &Gpu,
    texture: &wgpu::Texture,
    mut encoder: wgpu::CommandEncoder,
) -> Result<Vec<u8>, String> {
    let row = (WIDTH * 4).div_ceil(256) * 256;
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * HEIGHT),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
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
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r)));
    wait(gpu)?;
    rx.recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let bytes = readback.slice(..).get_mapped_range();
    Ok(bytes
        .chunks(row as usize)
        .flat_map(|row| row[..WIDTH as usize * 4].iter().copied())
        .collect())
}

fn png(path: &Path, pixels: &[u8]) -> Result<String, String> {
    let mut encoder = png::Encoder::new(
        std::fs::File::create(path).map_err(|e| e.to_string())?,
        WIDTH,
        HEIGHT,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())?;
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// `docs/verse/water.md`'s budget for water's GPU time at 1080p, ms.
fn budget_ms(tier: Tier) -> f64 {
    match tier {
        Tier::Low => 1.5,
        Tier::Medium => 2.5,
        Tier::High => 4.0,
    }
}

fn main() -> Result<(), String> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Expected an output directory")?,
    );
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let gpu = gpu()?;
    let field = Arc::new(coast::field()?);
    let surface = coast::surface(field.clone());
    surface.validate()?;
    let water = coast::frame_water(SEA, coast::TICK)?;
    // The bodies dropped five seconds before the picture, simulated on the
    // gameplay surface.
    let set = coast::water_set(SEA)?;
    let mut buoys = Buoys::new(coast::TICK - 600);
    buoys.step(&set, 600);
    for (kind, p, _) in buoys.poses() {
        println!("{kind:?} afloat at ({:.1}, {:.2}, {:.1})", p.x, p.y, p.z);
    }
    let half = coast::HALF_EXTENT + 40.0;
    let mut geometry = ground(coast::ground, [0.0, 0.0], half, 4.0, 40.0);
    geometry.extend(bodies(&buoys.poses()));
    let scene = Scene {
        lit: gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("coast ground"),
                contents: bytemuck::cast_slice(&geometry),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        count: geometry.len() as u32,
        time: water.time,
        water,
    };
    // The Water Lab's cove: its ground and its water as the lab draws them.
    let lab_surface = verse_zone_water::sea::surface();
    lab_surface.validate()?;
    let mut lab_water = verse_zone_water::sea::water();
    lab_water.time = 41.25;
    let lab_ground = ground(
        verse_zone_water::ground,
        verse_zone_water::terrain::CENTER,
        verse_zone_water::terrain::REACH,
        2.0,
        verse_zone_water::sea::SEA_DEPTH as f32,
    );
    let lab = Scene {
        lit: gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("lab ground"),
                contents: bytemuck::cast_slice(&lab_ground),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        count: lab_ground.len() as u32,
        time: lab_water.time,
        water: lab_water,
    };
    let only = std::env::var("COAST_CAPTURE_TIERS").ok();
    let mut records: Vec<Value> = Vec::new();
    for tier in TIERS {
        if only
            .as_deref()
            .is_some_and(|o| !o.split(',').any(|n| n == tier_name(tier)))
        {
            continue;
        }
        let mut photo = Photo::new(
            &gpu.device,
            &gpu.queue,
            capability(tier),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )?;
        for spec in views() {
            let (scene, surface) = if spec.lab {
                (&lab, &lab_surface)
            } else {
                (&scene, &surface)
            };
            let water_gpu = photo.upload_water(&gpu.device, surface);
            let (pixels, _) = render(
                &gpu,
                &mut photo,
                tier,
                scene,
                Some(&water_gpu),
                &spec,
                [WIDTH, HEIGHT],
            )?;
            let name = format!("{}-{}.png", spec.name, tier_name(tier));
            let digest = png(&directory.join(&name), &pixels)?;
            let ocean = water_gpu.0.ocean.as_ref().ok_or("the coast has an ocean")?;
            let metrics = ocean.metrics().unwrap_or_default();
            let (resident, wanted) = ocean
                .stream
                .as_ref()
                .and_then(|s| s.lock().ok().map(|s| (s.last.resident, s.last.wanted)))
                .unwrap_or_default();
            let budget = verse_pbr::water::field::budget(tier);
            let (_, wet) = render(
                &gpu,
                &mut photo,
                tier,
                scene,
                Some(&water_gpu),
                &spec,
                BUDGET_SIZE,
            )?;
            let (_, dry) = render(&gpu, &mut photo, tier, scene, None, &spec, BUDGET_SIZE)?;
            let water_ms = (wet - dry).max(0.0);
            println!(
                "{name}: {wet:.2} ms with water, {dry:.2} without, water {water_ms:.2} ms \
                 (budget {}), pages {resident}/{wanted}",
                budget_ms(tier)
            );
            records.push(json!({
                "picture": name,
                "sha256": digest,
                "tier": tier_name(tier),
                "view": spec.name,
                "eye": spec.eye.to_array(),
                "target": spec.target.to_array(),
                "rings": ocean.spec.levels,
                "clipmap_triangles": ocean.spec.triangles(),
                "field_pages_in_place": resident,
                "field_pages_wanted": wanted,
                "field_gpu_bytes": metrics.gpu_bytes,
                "field_gpu_high_water": metrics.gpu_high_water,
                "field_gpu_budget": budget.gpu_bytes,
                "field_atlas_bytes": photo.water_field.bytes(),
                "surface_bytes": water_gpu.bytes(),
                "frame_ms_1080p_with_water": wet,
                "frame_ms_1080p_without_water": dry,
                "water_ms_1080p": water_ms,
                "water_ms_budget": budget_ms(tier),
            }));
        }
    }
    let out = json!({
        "example": "cargo run --release -p verse-zone-water --example coast_capture",
        "adapter": gpu.adapter,
        "sea_state": SEA,
        "tick": coast::TICK,
        "size": [WIDTH, HEIGHT],
        "timed_size": BUDGET_SIZE,
        "timing": "the fastest of 7 batches of 10 frames, submitted back to back",
        "records": records,
    });
    std::fs::write(
        directory.join("capture.json"),
        serde_json::to_vec_pretty(&out).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
