//! Draconia's parity portraits: her character model in the dungeon set,
//! drawn by Verse's imported renderer with the character shading models
//! (skin, eye, cornea, hair, cloth), cinema lenses and true depth of field.
//!
//! Usage: draconia_portrait OUT.png [--model PATH.glb] [--set PATH.glb]
//! [--shot closeup|eyes|medium|wide|three|side|back] [--size WxH]
//! [--subframes N] [--fstop F] [--yaw DEGREES] [--turntable FRAMES]
//! [--light key|ref] [--exposure X] [--head YAW,PITCH] [--wind X,Y,Z]
//! [--video --seconds S] [--time T] [--stiffness K] [--gust T] [--gaze]
//! [--push SHARE] [--shot peacock_open] [--light peacock]
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

/// A shot aimed at her landmarks; `posed` carries a rest-pose head point to
/// where the playing clip holds it.
fn shot(
    name: &str,
    marks: &serde_json::Value,
    posed: &dyn Fn(Vec3) -> Vec3,
) -> Result<Shot, String> {
    let el = posed(world(&marks["eye_l"]));
    let er = posed(world(&marks["eye_r"]));
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
        // The owner's target frame (the photoreal reference at 0:54): a
        // close full face, head turned a little to camera left, placed right
        // of centre, the eyes a third down, crown to chin and collar.
        "fullface" => Shot {
            eye: face + Vec3::new(0.06, 0.0, -0.74),
            target: eyes + Vec3::new(0.022, -0.048, 0.0),
            focal_mm: 85.0,
            fstop: 2.4,
        },
        // The peacock guardian's opening (P3, ~114.5 s of the Draconia
        // film): an extreme close-up from the brows to the upper lip, ear to
        // ear, the head lowered and the eyes glaring up into the lens.
        "peacock_open" => Shot {
            eye: eyes + Vec3::new(0.0, -0.004, -0.41),
            target: eyes + Vec3::new(0.0, -0.006, 0.0),
            focal_mm: 100.0,
            fstop: 5.6,
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
        // The peacock's flash: a white core blazing behind her (the
        // backdrop) and rimming her hair, a soft front key from just above
        // the lens and a fill from below, the face fully readable.
        // The peacock's flash: a white core blazing behind and below her,
        // so the light comes up from the front-below (the lower face
        // brightest, the brow and forehead falling darker under the hair),
        // rims of blue-white on her hair, and a catchlight near the lens.
        "peacock" => {
            soft(
                &mut lights,
                face + Vec3::new(0.0, -0.35, -0.75),
                Vec3::new(1.0, 0.98, 0.96),
                3.0,
                0.3,
                3,
            );
            soft(
                &mut lights,
                face + Vec3::new(0.0, 0.5, -0.9),
                Vec3::new(0.85, 0.9, 1.0),
                0.45,
                0.4,
                3,
            );
            lights.push(Light {
                position: face + Vec3::new(-0.06, 0.03, -0.55),
                color: Vec3::new(1.0, 1.0, 1.0),
                intensity: 0.5,
                range: 4.0,
            });
            soft(
                &mut lights,
                face + Vec3::new(0.0, 0.5, 0.6),
                Vec3::new(0.85, 0.92, 1.0),
                2.5,
                0.6,
                6,
            );
            soft(
                &mut lights,
                face + Vec3::new(-0.5, 0.1, 0.5),
                Vec3::new(0.8, 0.9, 1.0),
                1.2,
                0.3,
                3,
            );
            soft(
                &mut lights,
                face + Vec3::new(0.5, 0.1, 0.5),
                Vec3::new(0.8, 0.9, 1.0),
                1.2,
                0.3,
                3,
            );
        }
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
            // Key: a soft, warm source high to screen left (world -X: the
            // camera looks along +Z, so +X is screen right), as the
            // reference's face light.
            soft(
                &mut lights,
                face + Vec3::new(-0.75, 0.45, -0.85),
                Vec3::new(1.0, 0.93, 0.86),
                1.6,
                0.35,
                6,
            );
            // Fill: cool and dim from screen right.
            soft(
                &mut lights,
                face + Vec3::new(0.9, 0.1, -1.0),
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
                face + Vec3::new(0.7, 0.25, 0.9),
                Vec3::new(0.35, 0.95, 0.85),
                1.1,
                0.3,
                3,
            );
            soft(
                &mut lights,
                face + Vec3::new(-0.75, 0.1, 0.9),
                Vec3::new(0.35, 0.95, 0.85),
                0.4,
                0.3,
                3,
            );
            for x in [-0.8f32, 0.0, 0.8, 1.6] {
                for y in [0.85f32, 1.3, 1.75] {
                    lights.push(Light {
                        position: Vec3::new(-x + 0.4, y + 0.1, 2.1),
                        color: Vec3::new(0.3, 0.95, 0.82),
                        intensity: 0.025,
                        range: 3.0,
                    });
                }
            }
        }
    }
    Lighting {
        ambient: Vec3::new(0.003, 0.007, 0.007),
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
    /// Head turn (yaw, pitch) in degrees.
    head: [f32; 2],
    wind: Vec3,
    seconds: f32,
    time: f32,
    video: bool,
    /// The hair chains' spring stiffness (1/s²): an updo's locks are held
    /// (70), loose long hair swings (25).
    stiffness: f32,
    gust: Option<f32>,
    /// Aim the eyes at the lens.
    gaze: bool,
    /// The share of the camera's distance it pushes in over a video.
    push: f32,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let build = PathBuf::from(user_home::home_string().map_err(|e| e.to_string())?)
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
        // The reference's head turns about 15 degrees to camera left,
        // chin a little down.
        head: [12.0, 3.0],
        wind: Vec3::new(0.25, 0.0, 0.1),
        seconds: 4.0,
        time: 1.0,
        video: false,
        stiffness: 70.0,
        gust: None,
        gaze: false,
        push: 0.0,
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
            "--head" => {
                let v = value()?;
                let (y, p) = v.split_once(',').ok_or("--head is YAW,PITCH")?;
                a.head = [
                    y.parse().map_err(|_| "bad yaw")?,
                    p.parse().map_err(|_| "bad pitch")?,
                ];
            }
            "--wind" => {
                let v: Vec<f32> = value()?.split(',').filter_map(|x| x.parse().ok()).collect();
                if v.len() != 3 {
                    return Err("--wind is X,Y,Z m/s".into());
                }
                a.wind = Vec3::new(v[0], v[1], v[2]);
            }
            "--seconds" => a.seconds = value()?.parse().map_err(|_| "bad seconds")?,
            "--time" => a.time = value()?.parse().map_err(|_| "bad time")?,
            "--video" => a.video = true,
            "--stiffness" => a.stiffness = value()?.parse().map_err(|_| "bad stiffness")?,
            "--gust" => a.gust = Some(value()?.parse().map_err(|_| "bad gust")?),
            "--gaze" => a.gaze = true,
            "--push" => a.push = value()?.parse().map_err(|_| "bad push")?,
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    Ok(a)
}

/// The shot `a.shot`, aimed at her head where the portrait clip holds it
/// at `a.time`.
fn frame(
    model: &verse_engine::assets::Model,
    marks: &serde_json::Value,
    a: &Args,
) -> Result<Shot, String> {
    let placed = Mat4::from_quat(Quat::from_rotation_y(a.yaw.to_radians()))
        * verse::imported::chamber::basis();
    let head = model
        .skin
        .as_ref()
        .and_then(|s| s.names.iter().position(|n| n == "Head"))
        .ok_or("no Head")?;
    let clip = if model.clips.iter().any(|c| c.id == PORTRAIT_CLIP) {
        PORTRAIT_CLIP
    } else {
        PORTRAIT_CLIP - 1
    };
    let pose = verse_engine::animation::pose(model, clip, a.time);
    let posed = |w: Vec3| {
        placed.transform_point3(pose[head].transform_point3(placed.inverse().transform_point3(w)))
    };
    shot(&a.shot, marks, &posed)
}

/// The clip the portraits play: the Universal idle with her head turned,
/// plus the hair's motion in the wind.
const PORTRAIT_CLIP: u16 = 900;

/// Turns the head (and a third of it the neck) by `yaw` and `pitch`
/// radians in world terms, on every key of `clip`, as a new clip `id`.
fn turn_head(
    model: &mut verse_engine::assets::Model,
    clip: u16,
    id: u16,
    yaw: f32,
    pitch: f32,
) -> Result<(), String> {
    let skin = model.skin.as_ref().ok_or("no skin")?;
    let names = skin.names.clone();
    let rest = skin.rest.clone();
    let mut globals = vec![Mat4::IDENTITY; model.bones.len()];
    for (i, b) in model.bones.iter().enumerate() {
        let r = rest[i];
        let l = Mat4::from_scale_rotation_translation(
            r.scale.into(),
            Quat::from_array(r.rotation),
            r.translation.into(),
        );
        globals[i] = if b.parent >= 0 {
            globals[b.parent as usize] * l
        } else {
            l
        };
    }
    let mut out = model
        .clips
        .iter()
        .find(|c| c.id == clip)
        .ok_or("no source clip")?
        .clone();
    out.id = id;
    // Skeleton space is glTF's: +Y up, she faces +Z; camera-left is her
    // right, -X... a positive yaw turns her face toward +X.
    for (joint, share) in [("neck_01", 0.35f32), ("Head", 0.65)] {
        let j = names
            .iter()
            .position(|n| n == joint)
            .ok_or("no head joint")?;
        let parent = model.bones[j].parent;
        let p = if parent >= 0 {
            globals[parent as usize].to_scale_rotation_translation().1
        } else {
            Quat::IDENTITY
        };
        let e = Quat::from_rotation_y(yaw * share) * Quat::from_rotation_x(pitch * share);
        let fix = p.inverse() * e * p;
        let keys = match out.bones.iter_mut().find(|k| k.bone == j) {
            Some(k) => k,
            None => {
                out.bones.push(verse_engine::assets::BoneKeys {
                    bone: j,
                    translation: vec![],
                    rotation: vec![(0.0, rest[j].rotation)],
                    scale: vec![],
                });
                out.bones.last_mut().unwrap()
            }
        };
        if keys.rotation.is_empty() {
            keys.rotation.push((0.0, rest[j].rotation));
        }
        for k in &mut keys.rotation {
            k.1 = (fix * Quat::from_array(k.1)).normalize().to_array();
        }
    }
    model.clips.retain(|c| c.id != id);
    model.clips.push(out);
    Ok(())
}

fn pack(
    dir: &Path,
    model: &Path,
    set: Option<&Path>,
    a: &Args,
) -> Result<(Pack, verse_engine::assets::Model), String> {
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
    let universal =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/characters/quaternius");
    let mut draconia = characters::film_character(&mut pack, dir, model, &universal, options)?;
    turn_head(
        &mut draconia,
        0,
        PORTRAIT_CLIP - 1,
        a.head[0].to_radians(),
        a.head[1].to_radians(),
    )?;
    if !verse_engine::hair_chains::chains(&draconia).is_empty() {
        let spheres = verse_engine::hair_chains::humanoid_spheres(&draconia);
        let wind = a.wind;
        let gust = a.gust;
        let clip = verse_engine::hair_chains::bake(
            &draconia,
            PORTRAIT_CLIP - 1,
            PORTRAIT_CLIP,
            a.seconds.max(2.0),
            30.0,
            2.0,
            // An updo's locks are held: stiffer than loose hair.
            verse_engine::hair_chains::Spring {
                stiffness: a.stiffness,
                ..Default::default()
            },
            &spheres,
            |t| {
                // An upward blast from `--gust T` seconds: peaks 0.25 s
                // later and fades over 1.15 s (the peacock's flash).
                let k = gust.map_or(0.0, |g| {
                    let u = t - g;
                    if u < 0.0 {
                        0.0
                    } else if u < 0.25 {
                        u / 0.25
                    } else {
                        (1.0 - (u - 0.25) / 1.15).max(0.0)
                    }
                });
                verse_engine::hair_chains::Wind {
                    velocity: wind + Vec3::new(0.0, 14.0, -8.0) * k,
                    gust: 0.6,
                    gust_rate: 0.3,
                    turbulence: 0.25 + wind.length() * 0.15 + 1.5 * k,
                    outward: 6.0 * k,
                    center: Vec3::ZERO,
                }
            },
        )?;
        draconia.clips.push(clip);
    } else {
        let mut c = draconia
            .clips
            .iter()
            .find(|c| c.id == PORTRAIT_CLIP - 1)
            .unwrap()
            .clone();
        c.id = PORTRAIT_CLIP;
        draconia.clips.push(c);
    }
    if a.gaze {
        // Lock the eyes on the lens: the shot is framed on the posed head,
        // then the eye joints aim at the camera in skeleton space.
        let marks = landmarks(model)?;
        let s = frame(&draconia, &marks, a)?;
        let placed = Mat4::from_quat(Quat::from_rotation_y(a.yaw.to_radians()))
            * verse::imported::chamber::basis();
        let skin = draconia.skin.as_ref().ok_or("no skin")?;
        let to_skeleton = (placed * Mat4::from_cols_array(&skin.basis)).inverse();
        let eyes: Vec<usize> = ["eye_l", "eye_r"]
            .iter()
            .filter_map(|n| skin.names.iter().position(|m| m == n))
            .collect();
        verse_engine::hair_chains::aim(
            &mut draconia,
            PORTRAIT_CLIP,
            &eyes,
            to_skeleton.transform_point3(s.eye),
            30.0,
            0.8,
        )?;
    }
    pack.models.insert("draconia".into(), draconia.clone());
    if let Some(set) = set {
        let room = characters::import_with(&mut pack, dir, set, options)?;
        pack.models.insert("draconia-set".into(), room);
    }
    pack.validate()?;
    Ok((pack, draconia))
}

fn main() -> Result<(), String> {
    let a = args()?;
    let marks = landmarks(&a.model)?;
    let dir = std::env::temp_dir().join(format!("verse-draconia-portrait-{}", std::process::id()));
    let (pack, model) = pack(&dir, &a.model, a.set.as_deref(), &a)?;
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
    let base = verse::imported::chamber::basis();
    let placed = Mat4::from_quat(Quat::from_rotation_y(a.yaw.to_radians())) * base;
    let head = model
        .skin
        .as_ref()
        .and_then(|s| s.names.iter().position(|n| n == "Head"))
        .ok_or("no Head")?;
    let pose = verse_engine::animation::pose(&model, PORTRAIT_CLIP, a.time);
    let posed = |w: Vec3| {
        placed.transform_point3(pose[head].transform_point3(placed.inverse().transform_point3(w)))
    };
    let mut s = frame(&model, &marks, &a)?;
    if let Some(f) = a.fstop {
        s.fstop = f;
    }
    let face = posed((world(&marks["eye_l"]) + world(&marks["eye_r"])) * 0.5);
    let instance = |model: &str, yaw: f32, time: f32| Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform: Mat4::from_quat(Quat::from_rotation_y(yaw)) * base,
        animation: PORTRAIT_CLIP.into(),
        time,
        animation_epoch: None,
        emission: Vec3::ONE,
    };
    let ui = UiBatch::default();
    let aspect = a.width as f32 / a.height as f32;
    let fov_y = 2.0 * (12.0 / s.focal_mm).atan();
    let frames = if a.turntable > 0 {
        a.turntable
    } else if a.video {
        (a.seconds * 24.0).round() as usize
    } else {
        1
    };
    let mut encoder = if frames > 1 {
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
        let spin = if a.turntable > 0 {
            frame as f32 / frames as f32 * std::f32::consts::TAU
        } else {
            0.0
        };
        let yaw = a.yaw.to_radians() + spin;
        let time = a.time + if a.video { frame as f32 / 24.0 } else { 0.0 };
        let mut instances = vec![instance("draconia", yaw, time)];
        if a.set.is_some() {
            instances.push(instance("draconia-set", 0.0, 0.0));
        }
        let mut acc = vec![0.0f32; (a.width * a.height * 3) as usize];
        let n = a.subframes.max(1);
        let u = if frames > 1 {
            frame as f32 / (frames - 1) as f32
        } else {
            0.0
        };
        let ease = u * u * (3.0 - 2.0 * u);
        let eye_at = s.eye + (s.target - s.eye) * a.push * ease;
        for k in 0..n {
            let camera = CameraSample {
                eye: eye_at,
                target: s.target,
                fov_y,
                // The landmarks are the eyeballs' centres; focus on the
                // corneas, 12 mm nearer the lens.
                focus: (s.target - eye_at).length() - 0.012,
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
