//! Standalone crypt lab, rendered through the zone's textured path.
//!
//! Usage: crypt_lab OUT_DIR [--models DIR] [--only NAME,...] [--no-assets]
//! [--no-scene] [--no-video]
//!
//! Loads the original models in `assets/verse/generated/chamber` (or
//! `--models DIR`) and writes `assets/<name>.png` for each model,
//! `scene.png` and one picture per station from inside the hall, and
//! `scene.mp4`, a walk from the door past the study, the storage, the
//! dissection slab, the brewing cauldrons, and the sarcophagus. The current
//! ritual chamber is not involved.
//!
//! The hall is closed and lit by its own sources: a lamp at every candle
//! cluster, cauldron, and brazier, which flickers, and moonlight through one
//! barred window in the far gable. Steam, bubbles, fire, dust motes, and low
//! fog run through the particle pipeline (`docs/verse/particles.md`). The
//! layout, lamps, and effects are the crypt zone's (`verse::zones::crypt`),
//! so the capture and `verse --crypt` show the same room.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use glam::{Mat4, Vec3};
use verse::fx::Particles;
use verse::mesh::Mesh;
use verse::pbr::textured::TexturedScene;
use verse::pbr::{Key, Lamp, LitVertex, Material, Neon};
use verse::render::{Offscreen, View};
use verse::zones::crypt::{self, Hall, MODELS};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const FPS: f32 = 24.0;

struct Args {
    out: PathBuf,
    models: PathBuf,
    only: Option<Vec<String>>,
    shots: Vec<(String, Vec3, Vec3)>,
    assets: bool,
    scene: bool,
    video: bool,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let out = PathBuf::from(it.next().ok_or("Expected an output directory")?);
    let mut args = Args {
        out,
        models: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/generated/chamber"),
        only: None,
        shots: Vec::new(),
        assets: true,
        scene: true,
        video: true,
    };
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--models" => {
                args.models = PathBuf::from(it.next().ok_or("--models needs a directory")?)
            }
            "--only" => {
                let list = it.next().ok_or("--only needs names")?;
                args.only = Some(list.split(',').map(str::to_owned).collect());
            }
            "--shot" => {
                let spec = it.next().ok_or("--shot needs NAME,EX,EY,EZ,TX,TY,TZ")?;
                let mut parts = spec.split(',');
                let name = parts.next().unwrap_or("shot").to_owned();
                let v: Vec<f32> = parts.filter_map(|p| p.parse().ok()).collect();
                if v.len() != 6 {
                    return Err(format!("Bad shot {spec}"));
                }
                args.shots.push((
                    name,
                    Vec3::new(v[0], v[1], v[2]),
                    Vec3::new(v[3], v[4], v[5]),
                ));
            }
            "--no-assets" => args.assets = false,
            "--no-scene" => args.scene = false,
            "--no-video" => args.video = false,
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    Ok(args)
}

fn main() -> Result<(), String> {
    let args = args()?;
    std::fs::create_dir_all(args.out.join("assets")).map_err(|e| e.to_string())?;
    if args.assets {
        render_assets(&args)?;
    }
    if !args.scene && !args.video {
        return Ok(());
    }
    let lab = Hall::from_dir(&args.models)?;
    let world = Mesh {
        lit: Vec::new(),
        textured: Some(Arc::new(lab.scene.clone())),
        ..Mesh::default()
    };
    let mut renderer = Offscreen::new(
        WIDTH,
        HEIGHT,
        &world,
        &verse::ui::Atlas::new(16.0),
        verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
    )?;
    let mut fx = lab.particles();
    // Let the steam, fog, and dust fill in before the first picture.
    let mut time = 0.0;
    while time < 10.0 {
        fx.tick(1.0 / FPS, |_, _| 0.0);
        time += 1.0 / FPS;
    }
    if args.scene {
        let custom: Vec<(&str, Vec3, Vec3)> = args
            .shots
            .iter()
            .map(|(n, e, t)| (n.as_str(), *e, *t))
            .collect();
        let shots: Vec<(&str, Vec3, Vec3)> = if custom.is_empty() {
            SHOTS.to_vec()
        } else {
            custom
        };
        for (name, eye, target) in &shots {
            let pixels = frame(&lab, &mut renderer, &fx, time, *eye, *target)?;
            write_png(&args.out.join(format!("{name}.png")), &pixels)?;
            eprintln!("{name}");
        }
    }
    if args.video {
        let path = args.out.join("scene.mp4");
        encode_walk(&path, &lab, &mut renderer, &mut fx, time)?;
        eprintln!("video {}", path.display());
    }
    Ok(())
}

/// The scene pictures: a wide view from inside the door, then one per
/// station.
const SHOTS: &[(&str, Vec3, Vec3)] = &[
    (
        "scene",
        Vec3::new(2.5, 1.75, 5.0),
        Vec3::new(-1.9, 1.05, -4.2),
    ),
    (
        "station_brewing",
        Vec3::new(0.2, 1.8, -1.4),
        Vec3::new(-3.6, 0.8, -5.6),
    ),
    (
        "station_dissection",
        Vec3::new(0.6, 1.8, -0.4),
        Vec3::new(4.4, 0.8, -4.6),
    ),
    (
        "station_study",
        Vec3::new(0.2, 1.75, 5.4),
        Vec3::new(-4.4, 0.9, 1.2),
    ),
    (
        "station_storage",
        Vec3::new(0.6, 1.75, 2.2),
        Vec3::new(5.0, 0.7, 6.0),
    ),
    (
        "station_sarcophagus",
        Vec3::new(0.7, 1.6, -3.4),
        Vec3::new(-0.1, 1.0, -7.8),
    ),
];

/// The walk-through: the camera's keys, eye then target, at even times:
/// in at the door, the study, the storage, the dissection slab, the
/// cauldrons, and up to the sarcophagus.
const WALK: &[(Vec3, Vec3)] = &[
    (Vec3::new(0.0, 1.75, 8.0), Vec3::new(0.0, 1.3, 0.0)),
    (Vec3::new(-0.4, 1.75, 5.8), Vec3::new(-4.2, 1.0, 2.6)),
    (Vec3::new(-1.0, 1.7, 3.8), Vec3::new(-4.8, 0.9, 0.8)),
    (Vec3::new(0.2, 1.7, 3.4), Vec3::new(4.6, 0.8, 5.0)),
    (Vec3::new(1.2, 1.7, 1.6), Vec3::new(5.2, 0.8, 2.6)),
    (Vec3::new(0.7, 1.75, -0.6), Vec3::new(4.4, 0.9, -3.8)),
    (Vec3::new(0.8, 1.7, -2.4), Vec3::new(4.6, 0.9, -5.8)),
    (Vec3::new(0.3, 1.7, -2.6), Vec3::new(-3.6, 0.8, -4.4)),
    (Vec3::new(-0.6, 1.7, -3.2), Vec3::new(-3.8, 0.9, -7.2)),
    (Vec3::new(-0.3, 1.8, -3.8), Vec3::new(0.0, 1.2, -8.0)),
    (Vec3::new(0.0, 1.9, -4.6), Vec3::new(0.0, 1.0, -7.8)),
];
const WALK_SECONDS: f32 = 26.0;

/// One frame of the hall from `eye` toward `target` at `time`, s: the
/// zone's stage, moonbeam, and effects ([`verse::zones::crypt`]).
fn frame(
    lab: &Hall,
    renderer: &mut Offscreen,
    fx: &Particles,
    time: f32,
    eye: Vec3,
    target: Vec3,
) -> Result<Vec<u8>, String> {
    let mut dynamic = Mesh {
        neon: Some(lab.stage(time)),
        glow: crypt::moonbeam(eye),
        ..Mesh::default()
    };
    fx.draw(&mut dynamic.sprites);
    renderer.render(view(eye, target), &dynamic, &verse::ui::UiBatch::default())
}

fn view(eye: Vec3, target: Vec3) -> View {
    View {
        view_proj: Mat4::perspective_rh(0.9, WIDTH as f32 / HEIGHT as f32, 0.08, 120.0)
            * Mat4::look_at_rh(eye, target, Vec3::Y),
        eye,
    }
}

/// One picture per model, on a dark stage under a studio key and a warm
/// lamp, with its flames and glowing liquids lit.
fn render_assets(args: &Args) -> Result<(), String> {
    for name in MODELS {
        if args
            .only
            .as_ref()
            .is_some_and(|only| !only.iter().any(|n| n == name))
        {
            continue;
        }
        let mut scene = TexturedScene::default();
        let mesh = scene.import_gltf(&args.models.join(format!("{name}.glb")))?;
        let (min, max) = bounds(&scene, mesh);
        let lift = if *name == "crypt_hall" { 0.0 } else { -min.y };
        scene.place(mesh, Mat4::from_translation(Vec3::new(0.0, lift, 0.0)));
        let center = (min + max) * 0.5 + Vec3::new(0.0, lift, 0.0);
        let reach = ((max - min).length() * 0.8).max(0.5);
        let (eye, target) = if *name == "crypt_hall" {
            (Vec3::new(-3.8, 2.6, 7.6), Vec3::new(1.2, 2.2, -5.0))
        } else {
            (
                center + Vec3::new(reach * 0.95, reach * 0.62, reach * 1.45),
                center - Vec3::Y * reach * 0.04,
            )
        };
        let world = Mesh {
            lit: stage(reach * 2.0),
            textured: Some(Arc::new(scene)),
            ..Mesh::default()
        };
        let mut renderer = Offscreen::new(
            WIDTH,
            HEIGHT,
            &world,
            &verse::ui::Atlas::new(16.0),
            verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
        )?;
        let hall = *name == "crypt_hall";
        let dynamic = Mesh {
            neon: Some(studio(center, reach, hall)),
            ..Mesh::default()
        };
        let pixels = renderer.render(
            View {
                view_proj: Mat4::perspective_rh(
                    if hall { 1.05 } else { 0.72 },
                    WIDTH as f32 / HEIGHT as f32,
                    0.05,
                    200.0,
                ) * Mat4::look_at_rh(eye, target, Vec3::Y),
                eye,
            },
            &dynamic,
            &verse::ui::UiBatch::default(),
        )?;
        write_png(
            &args.out.join("assets").join(format!("{name}.png")),
            &pixels,
        )?;
        eprintln!("asset {name}");
    }
    Ok(())
}

fn studio(center: Vec3, reach: f32, hall: bool) -> Neon {
    let mut neon = Neon::neutral(0.0);
    neon.field = [0.012, 0.011, 0.01];
    neon.fog_start = 200.0;
    neon.fog_end = 400.0;
    neon.bloom = 0.06;
    neon.vignette = 0.3;
    neon.key = Some(Key {
        dir: Vec3::new(0.45, 0.75, 0.5).normalize(),
        illuminance: if hall { 400.0 } else { 2_200.0 },
        angular_radius: 0.04,
        rim_dir: Vec3::new(-0.5, 0.35, -0.7).normalize(),
        rim_illuminance: if hall { 120.0 } else { 900.0 },
        rim_angular_radius: 0.12,
        sky: if hall { 160.0 } else { 260.0 },
        ground: 40.0,
        ev100: 8.0,
        shadow_center: center,
        shadow_half: reach * 2.0,
        shadow_distance: None,
        cache_far_shadows: false,
    });
    if hall {
        // Work lights along the empty hall.
        for (i, z) in [-6.0, -2.0, 2.0, 6.0].into_iter().enumerate() {
            neon.lamps[i] = Lamp {
                position: Vec3::new(if i % 2 == 0 { -3.0 } else { 3.0 }, 2.0, z),
                color: [1.0, 0.62, 0.32],
                intensity: 3_000.0,
                range: 12.0,
            };
        }
    } else {
        neon.lamps[0] = Lamp {
            position: center + Vec3::new(-reach * 0.7, reach * 0.5, reach * 0.6),
            color: [1.0, 0.62, 0.32],
            intensity: 60.0 * reach * reach,
            range: reach * 4.0,
        };
    }
    neon
}

fn bounds(scene: &TexturedScene, mesh: usize) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for primitive in &scene.meshes[mesh].primitives {
        for vertex in &primitive.vertices {
            let p = Vec3::from(vertex.pos);
            min = min.min(p);
            max = max.max(p);
        }
    }
    (min, max)
}

fn encode_walk(
    path: &Path,
    lab: &Hall,
    renderer: &mut Offscreen,
    fx: &mut Particles,
    start: f32,
) -> Result<(), String> {
    let frames = (WALK_SECONDS * FPS) as u32;
    let mut encoder = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &format!("{WIDTH}x{HEIGHT}"),
            "-framerate",
            &format!("{FPS}"),
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            "20",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    let mut pipe = encoder.stdin.take().ok_or("ffmpeg has no stdin")?;
    let mut time = start;
    for i in 0..frames {
        let t = i as f32 / (frames - 1) as f32;
        let (eye, target) = walk(t);
        let pixels = frame(lab, renderer, fx, time, eye, target)?;
        use std::io::Write;
        pipe.write_all(&pixels).map_err(|e| e.to_string())?;
        fx.tick(1.0 / FPS, |_, _| 0.0);
        time += 1.0 / FPS;
        if i % 48 == 0 {
            eprintln!("frame {i}/{frames}");
        }
    }
    drop(pipe);
    let status = encoder.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg exited {status}"))
    }
}

/// The camera at `t` from 0 to 1 along [`WALK`]: Catmull-Rom through the
/// keys, eased at both ends.
fn walk(t: f32) -> (Vec3, Vec3) {
    let t = t * t * (3.0 - 2.0 * t);
    let n = WALK.len() - 1;
    let x = t * n as f32;
    let i = (x.floor() as usize).min(n - 1);
    let f = x - i as f32;
    let key = |k: isize| WALK[k.clamp(0, n as isize) as usize];
    let i = i as isize;
    let [a, b, c, d] = [key(i - 1), key(i), key(i + 1), key(i + 2)];
    let spline = |p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3| {
        0.5 * ((2.0 * p1)
            + (p2 - p0) * f
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * f * f
            + (3.0 * p1 - p0 - 3.0 * p2 + p3) * f * f * f)
    };
    (spline(a.0, b.0, c.0, d.0), spline(a.1, b.1, c.1, d.1))
}

fn stage(half: f32) -> Vec<LitVertex> {
    let color = [0.09, 0.085, 0.08];
    let corner = |x: f32, z: f32| LitVertex {
        pos: [x * half, -0.002, z * half],
        normal: [0.0, 1.0, 0.0],
        tangent: [1.0, 0.0, 0.0],
        local: [x * half, 0.0, z * half],
        color,
        params: [0.0, 0.9, Material::WhitePaint.code(), 1.0],
    };
    let [a, b, c, d] = [
        corner(-1.0, -1.0),
        corner(-1.0, 1.0),
        corner(1.0, 1.0),
        corner(1.0, -1.0),
    ];
    vec![a, b, c, a, c, d]
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels))
        .map_err(|e| format!("{}: {e}", path.display()))
}
