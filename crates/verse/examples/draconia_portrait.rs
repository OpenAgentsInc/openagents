//! Draconia's parity portraits: her character model in the dungeon set,
//! drawn by Verse's imported renderer with the character shading models
//! (skin, eye, cornea, hair, cloth), cinema lenses and true depth of field.
//!
//! Usage: draconia_portrait OUT.png [--model PATH.glb] [--set PATH.glb]
//! [--shot closeup|eyes|medium|wide|three|side|back] [--size WxH]
//! [--subframes N] [--fstop F] [--yaw DEGREES] [--turntable FRAMES]
//! [--light key|ref] [--exposure X]
//!
//! The model and set default to `~/work/film-out/draconia/build/`
//! (`draconia.neutral.glb`, `draconia_set.glb`, built by
//! `scripts/blender/draconia.py` and `draconia_set.py`); the shots aim at
//! the face from `draconia.landmarks.json`, which the build writes beside
//! the model. Each output frame is the mean, in linear light, of
//! `subframes` renders, each through a different point on the lens (Vogel
//! spiral; [`verse_engine::camera_rig::lens_view_projection`]) with a
//! sub-pixel jitter, so it has depth of field and anti-aliasing; the hair's
//! stochastic coverage also averages out over them. `--turntable N` writes
//! an MP4 of N frames turning her a full circle instead.
//!
//! The lighting is the reference's: a dim stone room lit by teal specimen
//! jars behind her and a warm candle key in front, with a soft cool fill
//! (see docs/verse/draconia/parity.md).
use std::io::Write;
use std::path::{Path, PathBuf};

use glam::{Mat4, Quat, Vec2, Vec3};
use verse::imported::{
    Instance, Renderer, characters, flat,
    lighting::{Light, Lighting},
    original,
};
use verse::render::View;
use verse::ui::UiBatch;
use verse_engine::assets::Pack;
use verse_engine::camera_rig::{CameraSample, lens_sample, lens_view_projection};

struct Shot {
    eye: Vec3,
    target: Vec3,
    focal_mm: f32,
    fstop: f32,
}

fn landmarks(model: &Path) -> Result<serde_json::Value, String> {
    let path = model.with_file_name("draconia.landmarks.json");
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// A Blender-frame point (Z up, she faces -Y) in Verse's world after the
/// importer's basis and the instance transform: Y up, she faces -Z.
fn world(v: &serde_json::Value) -> Vec3 {
    let f = |i: usize| v[i].as_f64().unwrap_or(0.0) as f32;
    Vec3::new(-f(0), f(2), f(1))
}

fn shot(name: &str, marks: &serde_json::Value) -> Result<Shot, String> {
    let el = world(&marks["eye_l"]);
    let er = world(&marks["eye_r"]);
    let eyes = (el + er) * 0.5;
    let face = eyes + Vec3::new(0.0, -0.035, 0.0);
    let chest = world(&marks["chest"]);
    Ok(match name {
        // The reference's close-up (t 24 s): head and shoulders, the eyes a
        // third down the frame, an 85 mm lens a metre away, nearly frontal.
        "closeup" => Shot {
            eye: face + Vec3::new(0.06, -0.01, -0.86),
            target: face + Vec3::new(-0.025, -0.008, 0.0),
            focal_mm: 75.0,
            fstop: 2.0,
        },
        // The extreme close-up of the eyes (t 82 s): a macro framing the
        // eyes and the top of the nose.
        "eyes" => Shot {
            eye: eyes + Vec3::new(0.0, -0.005, -0.30),
            target: eyes + Vec3::new(0.0, -0.012, 0.0),
            focal_mm: 100.0,
            fstop: 4.0,
        },
        // The medium (t 12 s): chest up, three-quarter from her right.
        "medium" => Shot {
            eye: (eyes + chest) * 0.5 + Vec3::new(0.5, 0.0, -1.15),
            target: (eyes * 0.6 + chest * 0.4) + Vec3::new(-0.08, -0.02, 0.0),
            focal_mm: 50.0,
            fstop: 2.4,
        },
        "wide" => Shot {
            eye: Vec3::new(0.9, 1.5, -3.6),
            target: chest + Vec3::new(-0.3, 0.0, 0.0),
            focal_mm: 32.0,
            fstop: 2.8,
        },
        "three" => Shot {
            eye: face + Vec3::new(0.45, 0.02, -0.75),
            target: face,
            focal_mm: 85.0,
            fstop: 2.8,
        },
        "side" => Shot {
            eye: face + Vec3::new(0.85, 0.02, -0.02),
            target: face + Vec3::new(0.0, 0.0, 0.03),
            focal_mm: 85.0,
            fstop: 4.0,
        },
        "back" => Shot {
            eye: face + Vec3::new(0.45, 0.12, 0.75),
            target: face + Vec3::new(0.0, 0.02, 0.06),
            focal_mm: 70.0,
            fstop: 4.0,
        },
        other => return Err(format!("Unknown shot {other}")),
    })
}

/// The reference's light: teal from the jars behind her, a warm candle
/// below-left in front, a soft cool fill from front-right, low teal
/// ambient. Point lights only; a cluster stands in for a soft source.
fn lighting(face: Vec3, kind: &str) -> Lighting {
    let mut lights = vec![];
    let soft =
        |lights: &mut Vec<Light>, at: Vec3, color: Vec3, intensity: f32, spread: f32, n: usize| {
            for i in 0..n {
                let s = lens_sample(i, n, 0.3) * spread;
                lights.push(Light {
                    position: at + Vec3::new(s.x, s.y, s.x * 0.3),
                    color,
                    intensity: intensity / n as f32,
                    range: 12.0,
                });
            }
        };
    match kind {
        "key" => {
            soft(
                &mut lights,
                face + Vec3::new(0.6, 0.5, -1.2),
                Vec3::new(1.0, 0.95, 0.9),
                6.0,
                0.3,
                4,
            );
        }
        _ => {
            // Key: a soft, slightly warm source high to camera left (her
            // right), as the reference's face light.
            soft(
                &mut lights,
                face + Vec3::new(0.55, 0.45, -1.1),
                Vec3::new(1.0, 0.94, 0.88),
                1.5,
                0.35,
                6,
            );
            // Fill: cool and dim from camera right.
            soft(
                &mut lights,
                face + Vec3::new(-0.9, 0.1, -1.0),
                Vec3::new(0.62, 0.82, 0.86),
                0.3,
                0.4,
                3,
            );
            // The candle on the table, low and warm.
            lights.push(Light {
                position: Vec3::new(0.55, 1.0, -0.95),
                color: Vec3::new(1.0, 0.55, 0.22),
                intensity: 0.5,
                range: 6.0,
            });
            // Teal rims from the jars behind, both sides, and the shelves'
            // glow.
            soft(
                &mut lights,
                face + Vec3::new(-0.7, 0.25, 0.9),
                Vec3::new(0.35, 0.95, 0.85),
                1.1,
                0.3,
                3,
            );
            soft(
                &mut lights,
                face + Vec3::new(0.75, 0.1, 0.9),
                Vec3::new(0.35, 0.95, 0.85),
                0.8,
                0.3,
                3,
            );
            for x in [-0.8f32, 0.0, 0.8, 1.6] {
                for y in [0.85f32, 1.3, 1.75] {
                    lights.push(Light {
                        position: Vec3::new(-x + 0.4, y + 0.1, 2.1),
                        color: Vec3::new(0.3, 0.95, 0.82),
                        intensity: 0.12,
                        range: 3.0,
                    });
                }
            }
        }
    }
    Lighting {
        ambient: Vec3::new(0.006, 0.012, 0.012),
        exposure: 1.0,
        fog: Vec3::new(0.01, 0.02, 0.02),
        density: 0.0,
        time: 0.0,
        shadowed: 4,
        lights,
        height_fog: None,
    }
}

fn to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
fn to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

struct Args {
    out: PathBuf,
    model: PathBuf,
    set: Option<PathBuf>,
    shot: String,
    width: u32,
    height: u32,
    subframes: usize,
    fstop: Option<f32>,
    yaw: f32,
    turntable: usize,
    light: String,
    exposure: f32,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let build = PathBuf::from(std::env::var("HOME").map_err(|e| e.to_string())?)
        .join("work/film-out/draconia/build");
    let mut a = Args {
        out: PathBuf::from(it.next().ok_or("Expected OUT.png")?),
        model: build.join("draconia.neutral.glb"),
        set: Some(build.join("draconia_set.glb")),
        shot: "closeup".into(),
        width: 1920,
        height: 1080,
        subframes: 48,
        fstop: None,
        yaw: 0.0,
        turntable: 0,
        light: "ref".into(),
        exposure: 0.6,
    };
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--model" => a.model = PathBuf::from(value()?),
            "--set" => {
                let v = value()?;
                a.set = (v != "none").then(|| PathBuf::from(v));
            }
            "--shot" => a.shot = value()?,
            "--size" => {
                let v = value()?;
                let (w, h) = v.split_once('x').ok_or("--size is WxH")?;
                a.width = w.parse().map_err(|_| "bad width")?;
                a.height = h.parse().map_err(|_| "bad height")?;
            }
            "--subframes" => a.subframes = value()?.parse().map_err(|_| "bad subframes")?,
            "--fstop" => a.fstop = Some(value()?.parse().map_err(|_| "bad fstop")?),
            "--yaw" => a.yaw = value()?.parse().map_err(|_| "bad yaw")?,
            "--turntable" => a.turntable = value()?.parse().map_err(|_| "bad frames")?,
            "--light" => a.light = value()?,
            "--exposure" => a.exposure = value()?.parse().map_err(|_| "bad exposure")?,
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    Ok(a)
}

fn pack(dir: &Path, model: &Path, set: Option<&Path>) -> Result<Pack, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut pack = Pack {
        inventory: None,
        version: 1,
        source_revision: "draconia-parity".into(),
        models: Default::default(),
        textures: vec![],
        placements: vec![],
    };
    // Texture 0 is white: untextured surfaces sample it.
    flat::white_texture(&mut pack, dir)?;
    let options = characters::ImportOptions {
        max_texture_edge: 4096,
    };
    let draconia = characters::import_with(&mut pack, dir, model, options)?;
    pack.models.insert("draconia".into(), draconia);
    if let Some(set) = set {
        let room = characters::import_with(&mut pack, dir, set, options)?;
        pack.models.insert("draconia-set".into(), room);
    }
    pack.validate()?;
    Ok(pack)
}

fn main() -> Result<(), String> {
    let a = args()?;
    let marks = landmarks(&a.model)?;
    let dir = std::env::temp_dir().join(format!("verse-draconia-portrait-{}", std::process::id()));
    let pack = pack(&dir, &a.model, a.set.as_deref())?;
    let atlas = original::atlas()?;
    // A film character is far past a game pack's budgets: a million
    // triangles of JSON and 4K maps. The loader's limits allow up to 1 GiB.
    let budget = verse_engine::loading::Budget {
        manifest_bytes: 1024 * 1024 * 1024,
        encoded_file_bytes: 256 * 1024 * 1024,
        encoded_total_bytes: 768 * 1024 * 1024,
        rgba_total_bytes: 1024 * 1024 * 1024,
        decoder_workspace_bytes: 256 * 1024 * 1024,
    };
    let prepared = verse_engine::loading::Prepared::load(pack, &dir, budget)?;
    let mut renderer = Renderer::from_prepared(prepared, a.width, a.height, &atlas, &[])?;
    let mut s = shot(&a.shot, &marks)?;
    if let Some(f) = a.fstop {
        s.fstop = f;
    }
    let face = (world(&marks["eye_l"]) + world(&marks["eye_r"])) * 0.5;
    let base = verse::imported::chamber::basis();
    let instance = |model: &str, yaw: f32| Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform: Mat4::from_quat(Quat::from_rotation_y(yaw)) * base,
        animation: 0.into(),
        time: 0.0,
        animation_epoch: None,
        emission: Vec3::ONE,
    };
    let ui = UiBatch::default();
    let aspect = a.width as f32 / a.height as f32;
    let fov_y = 2.0 * (12.0 / s.focal_mm).atan();
    let frames = a.turntable.max(1);
    let mut encoder = if a.turntable > 0 {
        Some(
            std::process::Command::new("ffmpeg")
                .args([
                    "-y",
                    "-loglevel",
                    "error",
                    "-f",
                    "rawvideo",
                    "-pixel_format",
                    "rgba",
                    "-video_size",
                ])
                .arg(format!("{}x{}", a.width, a.height))
                .args([
                    "-framerate",
                    "24",
                    "-i",
                    "pipe:0",
                    "-an",
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    "-crf",
                    "14",
                ])
                .arg(&a.out)
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    for frame in 0..frames {
        let yaw = a.yaw.to_radians() + frame as f32 / frames as f32 * std::f32::consts::TAU;
        let mut instances = vec![instance("draconia", yaw)];
        if a.set.is_some() {
            instances.push(instance("draconia-set", 0.0));
        }
        let mut acc = vec![0.0f32; (a.width * a.height * 3) as usize];
        let n = a.subframes.max(1);
        for k in 0..n {
            let camera = CameraSample {
                eye: s.eye,
                target: s.target,
                fov_y,
                focus: (s.target - s.eye).length(),
                aperture: s.focal_mm / 1000.0 / (2.0 * s.fstop),
            };
            let (view_proj, eye) =
                lens_view_projection(&camera, aspect, 0.02, 60.0, lens_sample(k, n, 0.0));
            // Sub-pixel jitter (a 2,3 Halton step) for anti-aliasing.
            let jitter = Vec2::new(halton(k + 1, 2) - 0.5, halton(k + 1, 3) - 0.5)
                * Vec2::new(2.0 / a.width as f32, 2.0 / a.height as f32);
            let view_proj = Mat4::from_translation(jitter.extend(0.0)) * view_proj;
            let mut light = lighting(face, &a.light);
            light.time = 3.0 + k as f32 * 1.37 + frame as f32 * 0.04;
            light.exposure = a.exposure;
            let pixels = renderer.draw(View { view_proj, eye }, &instances, &ui, &light)?;
            for (i, px) in pixels.chunks_exact(4).enumerate() {
                for c in 0..3 {
                    acc[i * 3 + c] += to_linear(px[c]);
                }
            }
        }
        let rgba: Vec<u8> = acc
            .chunks_exact(3)
            .flat_map(|c| {
                [
                    to_srgb(c[0] / n as f32),
                    to_srgb(c[1] / n as f32),
                    to_srgb(c[2] / n as f32),
                    255,
                ]
            })
            .collect();
        if let Some(enc) = encoder.as_mut() {
            enc.stdin
                .as_mut()
                .ok_or("ffmpeg input")?
                .write_all(&rgba)
                .map_err(|e| e.to_string())?;
            eprintln!("frame {}/{}", frame + 1, frames);
        } else {
            let file = std::fs::File::create(&a.out).map_err(|e| e.to_string())?;
            let mut png = png::Encoder::new(file, a.width, a.height);
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header()
                .map_err(|e| e.to_string())?
                .write_image_data(&rgba)
                .map_err(|e| e.to_string())?;
            println!("wrote {}", a.out.display());
        }
    }
    if let Some(mut enc) = encoder {
        drop(enc.stdin.take());
        enc.wait().map_err(|e| e.to_string())?;
        println!("wrote {}", a.out.display());
    }
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}

fn halton(mut i: usize, base: usize) -> f32 {
    let mut f = 1.0;
    let mut r = 0.0;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}
