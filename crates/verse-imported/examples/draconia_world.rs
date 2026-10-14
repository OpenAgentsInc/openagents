//! Draconia in the castle: a walkable hall with her standing in it, run
//! locally (`scripts/verse/draconia-world.sh`, `docs/verse/draconia/world.md`).
//!
//! The hall is the great crypt's vaulted nave (`scripts/blender/great_crypt.py`)
//! furnished as a potions hall from the crypt lab's props: cauldrons, jar
//! shelves, bookshelves, an alchemy bench, candelabra and braziers, lit by
//! their flames. Draconia is the parity pipeline's latest export, read at run
//! time from `~/work/film-out/draconia/metahuman/latest/mh.neutral.glb` (the
//! stable path each round overwrites; `build-rig-neutral/` before it exists)
//! (a private MetaHuman asset: never committed, never used for AI training),
//! with her lower body from `scripts/verse/draconia_world_body.py`.
//!
//! She is static geometry, so the world rigs her itself: every vertex gets
//! weights on seven joints (root, chest, head, both eyes, both upper lids)
//! from where it sits, and each frame poses them through
//! `Renderer::set_pose_override`: breathing, a slow weight shift, blinks,
//! and a head and eyes that turn to the player when near.
//!
//! Usage: `draconia_world [--model GLB] [--lower GLB] [--textures 2048]
//! [--size WxH] [--capture DIR] [--walk OUT.mp4] [--benchmark SECONDS]
//! [--lod-sheet OUT.png] [--check-walk] [--no-draconia]`
//!
//! Window keys: click to look with the mouse, W A S D to walk, Shift to
//! run, Space to jump, F12 for a screenshot in ~/Downloads, Escape to free
//! the mouse (again to quit).
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use glam::{DVec3, Mat4, Quat, Vec2, Vec3};
use verse_engine::assets::{Bone, Clip, Model, Pack, Placement, Vertex};
use verse_engine::material::Shading;
use verse_engine::presentation::View;
use verse_gfx::ui::UiBatch;
use verse_imported::imported::{
    Instance, Renderer, chamber, characters, crypt_fight, flat,
    lighting::{Light, Lighting},
    original,
};

/// Bump when the cached pack's layout or the rig's weights change.
const CACHE_VERSION: u32 = 10;
const DRACONIA: &str = "draconia";
/// Her strand groom's levels of detail, all cut from the one groom: level 0
/// is every clump; each further level keeps one clump in `THIN[level]`,
/// widened as many times, so the hairstyle keeps its outline, volume, color
/// and shading while each clump stays wider than a pixel (a 3.5 mm clump is
/// a third of a pixel at 10 m, and only sparkles). The flyaways (0.1 mm)
/// leave after level 0.
const STRANDS: [&str; 3] = [
    "draconia-strands",
    "draconia-strands-1",
    "draconia-strands-2",
];
const THIN: [usize; 3] = [1, 3, 8];
/// Where each level takes over from the one before, metres from her face:
/// the two dissolve into each other (alpha to coverage) across the band.
const BANDS: [(f32, f32); 2] = [(2.2, 3.2), (5.2, 6.8)];

/// Where she stands and which way she faces (+Z, toward the entrance).
const HER_AT: Vec3 = Vec3::new(0.0, 0.0, -2.5);
const HER_YAW: f32 = std::f32::consts::PI;
/// Where the player arrives: on the entrance landing, looking down the nave.
const SPAWN: Vec3 = Vec3::new(0.0, 1.5, 15.2);
const EYE_HEIGHT: f32 = 1.66;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

/// Her newest export: the parity pipeline's stable `latest/` path, or the
/// rig build it copies from when no round has been published there yet.
fn latest_model(film: &Path) -> PathBuf {
    let latest = film.join("metahuman/latest/mh.neutral.glb");
    if latest.exists() {
        latest
    } else {
        film.join("metahuman/build-rig-neutral/mh.neutral.glb")
    }
}

/// The round number the parity pipeline wrote beside her export, if any.
fn round(model: &Path) -> Option<String> {
    let text = std::fs::read_to_string(model.with_file_name("ROUND")).ok()?;
    Some(text.trim().to_string())
}

struct Args {
    model: PathBuf,
    lower: PathBuf,
    textures: u32,
    size: [u32; 2],
    capture: Option<PathBuf>,
    walk: Option<PathBuf>,
    lod_sheet: Option<PathBuf>,
    benchmark: Option<f32>,
    no_draconia: bool,
    check_walk: bool,
}

fn args() -> Result<Args, String> {
    let film = home().join("work/film-out/draconia");
    let mut a = Args {
        model: latest_model(&film),
        lower: film.join("world/draconia_lower.glb"),
        textures: 2048,
        size: [1920, 1080],
        capture: None,
        walk: None,
        lod_sheet: None,
        benchmark: None,
        no_draconia: false,
        check_walk: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--model" => a.model = value()?.into(),
            "--lower" => a.lower = value()?.into(),
            "--textures" => a.textures = value()?.parse().map_err(|_| "bad --textures")?,
            "--size" => {
                let v = value()?;
                let (w, h) = v.split_once('x').ok_or("--size is WxH")?;
                a.size = [
                    w.parse().map_err(|_| "bad width")?,
                    h.parse().map_err(|_| "bad height")?,
                ];
            }
            "--capture" => a.capture = Some(value()?.into()),
            "--walk" => a.walk = Some(value()?.into()),
            "--lod-sheet" => a.lod_sheet = Some(value()?.into()),
            "--benchmark" => a.benchmark = Some(value()?.parse().map_err(|_| "bad seconds")?),
            "--no-draconia" => a.no_draconia = true,
            "--check-walk" => a.check_walk = true,
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    Ok(a)
}

// ---------------------------------------------------------------- the hall

/// Props the potions hall keeps from the crypt's layout; the cult's things
/// (the circle, bones, cages, chains, sarcophagi, the rug, ruins) stay out.
const KEEP: &[&str] = &[
    "great_crypt_hall",
    "candelabrum_tall",
    "candelabrum_short",
    "floor_candles",
    "brazier",
    "cauldron_green",
    "cauldron_red",
    "cauldron_amber",
    "bookshelf",
    "jar_shelf",
    "alchemy_bench",
    "writing_desk",
    "lectern",
    "crate",
    "barrel",
    "specimen_jar",
    "slab_table",
];

/// Models that hang on walls or lie flat, which nothing runs into.
const UNBLOCKING: &[&str] = &["cobweb", "ritual_rug"];

/// The potions class around her: name, x, y, z, yaw (world metres; a model
/// faces +Z at yaw 0).
const CLASS: &[(&str, f32, f32, f32, f32)] = &[
    // Her bench behind her, cauldrons either side, candles to light her.
    ("alchemy_bench", 0.0, 0.0, -4.4, 0.0),
    ("candelabrum_short", -0.5, 0.86, -4.5, 0.0),
    ("cauldron_green", -1.75, 0.0, -3.4, 0.4),
    ("cauldron_amber", 1.8, 0.0, -3.6, -0.5),
    // Her key's candles: a tall candelabrum at her front-right.
    ("candelabrum_tall", -1.15, 0.0, -1.35, 0.3),
    ("candelabrum_tall", 1.5, 0.0, -1.6, -0.2),
    // Two more benches down the nave, as a classroom.
    ("cauldron_red", -2.6, 0.0, 2.8, 0.9),
    ("writing_desk", 2.7, 0.0, 3.6, -0.3),
    ("lectern", -2.4, 0.0, -7.6, 0.6),
    ("jar_shelf", 3.4, 0.0, -7.8, -0.4),
    // The dais where the circle was: the master's bench.
    ("alchemy_bench", 0.0, 0.75, -17.8, 0.0),
    ("cauldron_green", -1.5, 0.75, -16.6, 0.0),
    ("cauldron_amber", 1.5, 0.75, -16.6, 0.0),
];

fn layout() -> Vec<(&'static str, f32, f32, f32, f32)> {
    let mut out: Vec<_> = verse_world::great_crypt::LAYOUT
        .iter()
        .copied()
        .filter(|(name, ..)| KEEP.contains(name) || *name == "cobweb")
        .collect();
    out.extend_from_slice(CLASS);
    out
}

fn placements(layout: &[(&str, f32, f32, f32, f32)]) -> Vec<Placement> {
    let basis = verse_content::basis();
    let inverse = basis.inverse();
    layout
        .iter()
        .map(|&(name, x, y, z, yaw)| {
            // The importer turns a model half a turn about Y; turn it back.
            let turn = inverse * Mat4::from_rotation_y(yaw - std::f32::consts::PI) * basis;
            Placement {
                model: verse_content::compiler::great_crypt::model_name(name),
                position: inverse.transform_point3(Vec3::new(x, y, z)).to_array(),
                rotation: Quat::from_mat4(&turn).normalize().to_array(),
                scale: 1.0,
            }
        })
        .collect()
}

#[derive(serde::Deserialize)]
struct FootprintFile {
    boxes: Vec<BoxSpec>,
}
#[derive(serde::Deserialize)]
struct BoxSpec {
    center: [f32; 3],
    half_extents: [f32; 3],
}

/// What the player runs into: the hall's collision boxes (walls, piers,
/// stairs, the dais), every standing prop's footprint, and Draconia.
fn collision(layout: &[(&str, f32, f32, f32, f32)]) -> Result<physics::queries::Scene, String> {
    use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Scene, Usage};
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/generated");
    let mut boxes: Vec<(Vec3, Vec3)> = Vec::new();
    for &(name, x, y, z, yaw) in layout {
        if UNBLOCKING.contains(&name) {
            continue;
        }
        let path = assets
            .join(verse_world::great_crypt::model_folder(name))
            .join(format!("{name}.footprint.json"));
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file: FootprintFile =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let turn = Mat4::from_translation(Vec3::new(x, y, z)) * Mat4::from_rotation_y(yaw);
        for spec in file.boxes {
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for corner in 0..8 {
                let s = |bit: usize| if corner & bit == 0 { -1.0 } else { 1.0 };
                let p = turn.transform_point3(Vec3::new(
                    spec.center[0] + s(1) * spec.half_extents[0],
                    spec.center[1] + s(2) * spec.half_extents[1],
                    spec.center[2] + s(4) * spec.half_extents[2],
                ));
                min = min.min(p);
                max = max.max(p);
            }
            boxes.push((min, max));
        }
    }
    // Draconia: her legs and robe, low, so the camera can still come close
    // to her face.
    boxes.push((
        HER_AT + Vec3::new(-0.24, 0.0, -0.2),
        HER_AT + Vec3::new(0.24, 1.2, 0.2),
    ));
    let mut scene = Scene::default();
    for (shape, (min, max)) in boxes.into_iter().enumerate() {
        scene.insert(MeshCollider {
            key: ColliderKey {
                life: Life {
                    instance: 0,
                    entity: 0,
                    generation: 0,
                },
                shape: shape as u32,
            },
            layers: 1,
            usage: Usage::Blocking,
            mesh: Mesh::from_box(min.as_dvec3(), max.as_dvec3())?,
        })?;
    }
    Ok(scene)
}

/// A light at each flame, colored cauldron glows, the moon through the far
/// window, and Draconia's own key, fill and rim, flickering at `t`.
fn lighting(layout: &[(&str, f32, f32, f32, f32)], t: f32) -> Lighting {
    let warm = Vec3::new(1.0, 0.56, 0.24);
    let fire = Vec3::new(1.0, 0.46, 0.16);
    // (position, color, intensity, range, flickers)
    let mut lights: Vec<(Vec3, Vec3, f32, f32, bool)> = Vec::new();
    let face = HER_AT + Vec3::new(0.0, 1.6, 0.0);
    // Her key: warm, high and well round to her right (screen left from
    // the landing), so her far cheek falls into shadow and the skin's
    // highlight sits off the nose; a cool soft fill from the other side; a
    // teal rim from the green cauldron behind her.
    // Her key sits on that candelabrum's flames (`CLASS`), a little above
    // her eyes; strong enough that she reads from across the hall.
    lights.push((Vec3::new(-1.15, 1.98, -1.35), warm, 8.5, 7.0, true));
    // A soft wash from above and in front: the candles' glow off the vault,
    // so her face and hair stay readable from the entrance.
    lights.push((face + Vec3::new(0.0, 1.5, 1.6), warm, 2.2, 7.0, false));
    lights.push((
        face + Vec3::new(1.4, 0.2, 1.6),
        Vec3::new(0.8, 0.74, 0.68),
        1.6,
        5.0,
        false,
    ));
    lights.push((
        face + Vec3::new(-0.7, 0.0, -0.9),
        Vec3::new(0.3, 0.95, 0.8),
        0.6,
        2.5,
        false,
    ));
    // A warm light high behind her (the bench's lamp), so she reads from
    // behind as well.
    lights.push((face + Vec3::new(0.5, 0.9, -1.1), warm, 2.4, 4.0, true));
    for &(name, x, y, z, _) in layout {
        let at = Vec3::new(x, y, z);
        let light = match name {
            "candelabrum_tall" => (at + Vec3::Y * 1.75, warm, 26.0, 7.0, true),
            "candelabrum_short" => (at + Vec3::Y * 0.45, warm, 10.0, 5.0, true),
            "floor_candles" => (at + Vec3::Y * 0.7, warm, 12.0, 5.0, true),
            "brazier" => (at + Vec3::Y * 1.45, fire, 60.0, 11.0, true),
            "cauldron_green" => (
                at + Vec3::Y * 1.3,
                Vec3::new(0.25, 1.0, 0.45),
                22.0,
                6.0,
                false,
            ),
            "cauldron_red" => (
                at + Vec3::Y * 1.3,
                Vec3::new(1.0, 0.25, 0.12),
                22.0,
                6.0,
                false,
            ),
            "cauldron_amber" => (
                at + Vec3::Y * 1.3,
                Vec3::new(1.0, 0.62, 0.2),
                20.0,
                6.0,
                false,
            ),
            _ => continue,
        };
        lights.push(light);
    }
    lights.push((
        Vec3::new(0.0, 6.2, -19.6),
        Vec3::new(0.42, 0.52, 0.9),
        55.0,
        20.0,
        false,
    ));
    for side in [-1.0, 1.0] {
        for z in verse_world::great_crypt::CHAPELS {
            lights.push((Vec3::new(side * 12.1, 1.8, z), warm, 26.0, 7.0, true));
        }
    }
    lights.truncate(verse_engine::lighting::MAX_LIGHTS);
    Lighting {
        ambient: Vec3::new(0.012, 0.010, 0.013),
        exposure: 1.0,
        fog: Vec3::new(0.011, 0.008, 0.012),
        density: 0.005,
        // A fixed clock: hair coverage is dithered by it, and a still
        // pattern reads as hair where a changing one fizzes. Flames flicker
        // below instead.
        time: 0.0,
        shadowed: 4,
        height_fog: Some(verse_engine::lighting::HeightFog {
            density: 0.035,
            base: 0.0,
            falloff: 0.5,
            start: 3.0,
            max_opacity: 0.5,
            sun_strength: 0.0,
            sun_exponent: 1.0,
        }),
        lights: lights
            .into_iter()
            .enumerate()
            .map(|(i, (position, color, intensity, range, flame))| {
                let k = i as f32 * 1.7;
                let flicker = if flame {
                    1.0 + 0.08 * (t * 7.3 + k).sin()
                        + 0.05 * (t * 13.1 + k * 2.3).sin()
                        + 0.04 * (t * 23.7 + k * 0.6).sin()
                } else {
                    1.0
                };
                Light {
                    position,
                    color,
                    intensity: intensity * flicker * 0.62,
                    range,
                }
            })
            .collect(),
    }
}

// ------------------------------------------------------------- her rig

/// Joint indices of the world's rig.
const ROOT: u32 = 0;
const CHEST: u32 = 1;
const HEAD: u32 = 2;
const EYE: [u32; 2] = [3, 4];
const LID: [u32; 2] = [5, 6];
const JOINTS: usize = 7;

/// Her frame ("body space"): metres, Y up, she faces -Z, feet at the
/// origin. Model space is the importer's; `basis` maps one to the other.
fn to_body(p: [f32; 3]) -> Vec3 {
    verse_content::basis().transform_point3(Vec3::from(p))
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What the rig needs to know about her, found from her geometry.
#[derive(Clone, Copy, Debug)]
struct Landmarks {
    eyes: [Vec3; 2],
    neck: Vec3,
}

/// Small eye-shaded or cornea-shaded surfaces are the eyeballs; their
/// centres are the eyes (left is +X in her own frame, so -X in body space).
fn landmarks(model: &Model) -> Result<Landmarks, String> {
    let mut eyes = [Vec3::ZERO; 2];
    let mut found = [false; 2];
    for s in &model.surfaces {
        if s.material.shading != Shading::Eye as u8 || s.vertices.is_empty() {
            continue;
        }
        let (min, max) = bounds(s.vertices.iter().map(|v| to_body(v.position)));
        if max.x - min.x > 0.045 {
            continue;
        }
        let c = (min + max) * 0.5;
        let side = usize::from(c.x > 0.0);
        eyes[side] = c;
        found[side] = true;
    }
    if !found[0] || !found[1] {
        return Err("Draconia's model has no eyeballs (eye-shaded surfaces)".into());
    }
    let mid = (eyes[0] + eyes[1]) * 0.5;
    Ok(Landmarks {
        eyes,
        neck: Vec3::new(mid.x, mid.y - 0.1, mid.z + 0.07),
    })
}

fn bounds(points: impl Iterator<Item = Vec3>) -> (Vec3, Vec3) {
    points.fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(a, b), p| (a.min(p), b.max(p)),
    )
}

/// Weights every vertex on the world rig by where it sits.
fn rig(model: &mut Model) -> Result<Landmarks, String> {
    let marks = landmarks(model)?;
    for s in &mut model.surfaces {
        let shading = s.material.shading;
        // Her skin's sheen is tuned for the film's broad soft lights; under
        // a candle's point source it reads oily, so the world halves it.
        if shading == Shading::Skin as u8 {
            s.material.shading_params[2] *= 0.55;
        }
        let (min, max) = bounds(s.vertices.iter().map(|v| to_body(v.position)));
        let small_eye = (shading == Shading::Eye as u8 || shading == Shading::Cornea as u8)
            && max.x - min.x < 0.045;
        let cloth = shading == Shading::Cloth as u8;
        let hair = shading == Shading::Hair as u8;
        for v in &mut s.vertices {
            let b = to_body(v.position);
            let chest = smooth(0.95, 1.2, b.y);
            let mut joints = [ROOT, CHEST, HEAD, 0];
            let mut weights;
            if small_eye {
                let side = usize::from((min.x + max.x) > 0.0);
                joints = [EYE[side], 0, 0, 0];
                weights = [1.0, 0.0, 0.0, 0.0];
            } else {
                let head = if cloth {
                    0.0
                } else if hair {
                    smooth(1.36, 1.46, b.y)
                } else {
                    smooth(1.43, 1.53, b.y)
                };
                weights = [
                    (1.0 - chest) * (1.0 - head),
                    chest * (1.0 - head),
                    head,
                    0.0,
                ];
                // The upper lids (skin and lashes) over the front of each eye.
                if !cloth && head > 0.99 {
                    for side in 0..2 {
                        let c = marks.eyes[side];
                        let d = b.distance(c);
                        let lid = (1.0 - smooth(0.0145, 0.021, d))
                            * smooth(-0.002, 0.003, b.y - c.y)
                            * smooth(0.004, -0.004, b.z - c.z);
                        if lid > 0.0 {
                            joints[3] = LID[side];
                            weights = [0.0, 0.0, 1.0 - lid, lid];
                        }
                    }
                }
            }
            v.joints = joints;
            v.weights = weights;
        }
    }
    model.skin = None;
    model.bones = (0..JOINTS)
        .map(|i| Bone {
            parent: match i {
                0 => -1,
                1 => 0,
                2 => 1,
                _ => 2,
            },
            pivot: [0.0; 3],
        })
        .collect();
    model.clips = vec![Clip {
        id: 0,
        duration: 1.0,
        bones: vec![],
    }];
    Ok(marks)
}

/// Splits her strand groom (every hair-shaded surface on the strand
/// texture: the body, the flyaways, the face-framing locks) into its own
/// model, which fades with distance, and makes the rest of her hair agree
/// with it:
///
/// - the hair cards are tinted so their mean color (by coverage) is the
///   strands', so the hairstyle keeps its color when the strands dissolve;
/// - the brows and lashes are lightened and thinned a little: dark,
///   full-opacity cards read heavy under candlelight.
fn groom(her: &mut Model, pack: &Pack, dir: &Path) -> Result<Model, String> {
    let hair = Shading::Hair as u8;
    let strand_texture = her
        .surfaces
        .iter()
        .filter(|s| s.material.shading == hair)
        .max_by_key(|s| s.vertices.len())
        .map(|s| s.texture)
        .ok_or("Draconia has no hair")?;
    let mut strands = her.clone();
    strands
        .surfaces
        .retain(|s| s.material.shading == hair && s.texture == strand_texture);
    for s in &mut strands.surfaces {
        s.tint = (Vec3::from(s.tint) * HAIR_GRADE).to_array();
    }
    her.surfaces
        .retain(|s| !(s.material.shading == hair && s.texture == strand_texture));
    let strand_color = mean_color(pack, dir, strand_texture)?;
    let marks = landmarks(her)?;
    let eyes_y = (marks.eyes[0].y + marks.eyes[1].y) * 0.5;
    for s in &mut her.surfaces {
        if s.material.shading != hair || s.vertices.is_empty() {
            continue;
        }
        let (min, max) = bounds(s.vertices.iter().map(|v| to_body(v.position)));
        if max.y - min.y > 0.15 {
            // The cards: the strands' color, card for strand.
            let card = mean_color(pack, dir, s.texture)?;
            let ratio = (strand_color / card.max(Vec3::splat(1e-3)))
                .clamp(Vec3::splat(0.5), Vec3::splat(1.6));
            s.tint = (Vec3::from(s.tint) * ratio * HAIR_GRADE).to_array();
        } else if s.texture == 0 {
            // The painted brow (a solid brow-shaped shell, no map): a soft
            // fair tone under the brow hairs, not a dark block.
            s.tint = (Vec3::from(s.tint) * 1.4).min(Vec3::ONE).to_array();
            s.material.opacity *= 0.35;
        } else {
            // Brow hairs and lashes: their cards' own hard cutout (alpha to
            // coverage turns a lash card's soft alpha into a smudge), the
            // brows a lighter brown.
            s.material.shading = Shading::Standard as u8;
            if (min.y + max.y) * 0.5 > eyes_y + 0.012 {
                s.tint = (Vec3::from(s.tint) * 1.5).min(Vec3::ONE).to_array();
            }
        }
    }
    eprintln!("draconia-world: strand color {strand_color:.3}");
    Ok(strands)
}

/// A level of detail of the strand groom: one clump in `k` (a hashed
/// choice, so no pattern shows), each widened `k` times about its centre
/// line, so the hair's coverage stays what it was. The groom is ribbons:
/// vertices in left/right pairs, quads joining each pair to the next along
/// a clump. Surfaces of hair finer than 0.2 mm (the flyaways) are left out.
fn thinned(strands: &Model, k: usize) -> Result<Model, String> {
    let mut out = strands.clone();
    out.surfaces.clear();
    for s in &strands.surfaces {
        let v = &s.vertices;
        if v.len() % 2 != 0 || v.is_empty() {
            return Err("The strand groom is not left/right ribbon pairs".into());
        }
        let pairs = v.len() / 2;
        let width =
            |p: usize| Vec3::from(v[2 * p].position).distance(Vec3::from(v[2 * p + 1].position));
        let mut widths: Vec<f32> = (0..pairs).step_by(97).map(width).collect();
        widths.sort_by(f32::total_cmp);
        // Model units are yards (`basis`); 0.2 mm.
        if widths[widths.len() / 2] < 0.0002 / 0.9144 {
            continue;
        }
        let mut link = vec![false; pairs];
        let mut odd = 0usize;
        for t in s.indices.chunks_exact(3) {
            let ps = [t[0] as usize / 2, t[1] as usize / 2, t[2] as usize / 2];
            let lo = *ps.iter().min().unwrap_or(&0);
            let hi = *ps.iter().max().unwrap_or(&0);
            if hi == lo + 1 {
                link[lo] = true;
            } else if hi != lo {
                odd += 1;
            }
        }
        if odd * 20 > s.indices.len() / 3 {
            return Err("The strand groom's triangles do not join ribbon pairs".into());
        }
        let mut keep = vec![false; pairs];
        let mut strand = 0u32;
        for p in 0..pairs {
            if p > 0 && !link[p - 1] {
                strand += 1;
            }
            let hash = strand.wrapping_mul(2_654_435_761) >> 7;
            keep[p] = hash as usize % k == 0;
        }
        let mut map = vec![u32::MAX; pairs];
        let mut vertices = Vec::new();
        for p in 0..pairs {
            if !keep[p] {
                continue;
            }
            map[p] = (vertices.len() / 2) as u32;
            let (l, r) = (
                Vec3::from(v[2 * p].position),
                Vec3::from(v[2 * p + 1].position),
            );
            let (c, half) = ((l + r) * 0.5, (r - l) * 0.5 * k as f32);
            let mut a = v[2 * p].clone();
            let mut b = v[2 * p + 1].clone();
            a.position = (c - half).to_array();
            b.position = (c + half).to_array();
            vertices.push(a);
            vertices.push(b);
        }
        let mut indices = Vec::new();
        for t in s.indices.chunks_exact(3) {
            if t.iter().all(|&i| keep[i as usize / 2]) {
                indices.extend(t.iter().map(|&i| map[i as usize / 2] * 2 + i % 2));
            }
        }
        let mut surface = s.clone();
        surface.vertices = vertices;
        surface.indices = indices;
        out.surfaces.push(surface);
    }
    Ok(out)
}

/// How much of each strand level to draw `distance` metres from her face:
/// weights that sum to one, crossing over in the `BANDS`.
fn strand_levels(distance: f32) -> [f32; 3] {
    let a = smooth(BANDS[0].0, BANDS[0].1, distance);
    let b = smooth(BANDS[1].0, BANDS[1].1, distance);
    [1.0 - a, a * (1.0 - b), b]
}

/// Her platinum under candlelight: the film renders' key is a broad soft
/// source well to the side, the world's are candles close above, which
/// light the crown of the updo near white; the world grades the hair down.
const HAIR_GRADE: f32 = 0.72;

/// A texture's mean linear color, weighted by its alpha.
fn mean_color(pack: &Pack, dir: &Path, texture: usize) -> Result<Vec3, String> {
    let file = &pack.textures.get(texture).ok_or("No such texture")?.file;
    let path = dir.join(file);
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?,
    ));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let channels = info.color_type.samples();
    if info.bit_depth != png::BitDepth::Eight || channels < 3 {
        return Err(format!("{}: expected 8-bit RGB(A)", path.display()));
    }
    let linear = |c: u8| {
        let c = f32::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let mut sum = Vec3::ZERO;
    let mut weight = 0.0;
    for px in buf[..info.buffer_size()].chunks_exact(channels).step_by(7) {
        let a = if channels == 4 {
            f32::from(px[3]) / 255.0
        } else {
            1.0
        };
        sum += Vec3::new(linear(px[0]), linear(px[1]), linear(px[2])) * a;
        weight += a;
    }
    Ok(sum / weight.max(1e-6))
}

/// Her idle: breathing, a weight shift, blinks, and her gaze.
struct Idle {
    head: Vec2,
    eyes: Vec2,
    next_blink: f32,
    blink_at: f32,
    seed: u32,
}

impl Idle {
    fn new() -> Self {
        Self {
            head: Vec2::ZERO,
            eyes: Vec2::ZERO,
            next_blink: 1.5,
            blink_at: -10.0,
            seed: 7,
        }
    }

    fn random(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// The model-space palette at `t` for a viewer at `viewer` (body space).
    fn pose(&mut self, marks: &Landmarks, t: f32, dt: f32, viewer: Vec3) -> Vec<Mat4> {
        let mid = (marks.eyes[0] + marks.eyes[1]) * 0.5;
        // Gaze: yaw about +Y and pitch about +X, from her face to the
        // viewer, when the viewer is near and in front of her.
        let d = viewer - mid;
        let yaw = (-d.x).atan2(-d.z);
        let pitch = d.y.atan2(Vec2::new(d.x, d.z).length());
        let engaged = d.length() < 5.5 && yaw.abs() < 1.75;
        let (head_goal, eye_goal) = if engaged {
            let head = Vec2::new(
                (yaw * 0.65).clamp(-0.6, 0.6),
                (pitch * 0.55).clamp(-0.22, 0.18),
            );
            let eyes = Vec2::new(
                (yaw - head.x).clamp(-0.42, 0.42),
                (pitch - head.y).clamp(-0.3, 0.25),
            );
            (head, eyes)
        } else {
            // At rest she looks a little down and to her left, idly.
            (
                Vec2::new(0.06 * (t * 0.13).sin(), -0.05),
                Vec2::new(0.08 * (t * 0.31).sin(), -0.04),
            )
        };
        self.head += (head_goal - self.head) * (1.0 - (-2.6 * dt).exp());
        self.eyes += (eye_goal - self.eyes) * (1.0 - (-16.0 * dt).exp());
        // Blinks every 2.5-6 s, and when her gaze first finds the viewer.
        if t >= self.next_blink {
            self.blink_at = t;
            self.next_blink = t + 2.5 + 3.5 * self.random();
        }
        let u = (t - self.blink_at) / 0.2;
        let close = if (0.0..1.0).contains(&u) {
            if u < 0.35 {
                smooth(0.0, 0.35, u)
            } else {
                1.0 - smooth(0.35, 1.0, u)
            }
        } else {
            0.0
        };
        // Breathing (4.4 s) and the weight shift (9 s), small.
        let breath = (t * std::f32::consts::TAU / 4.4).sin();
        let sway = (t * std::f32::consts::TAU / 9.0).sin();
        let around = |pivot: Vec3, q: Quat| {
            Mat4::from_translation(pivot) * Mat4::from_quat(q) * Mat4::from_translation(-pivot)
        };
        let root = around(Vec3::ZERO, Quat::from_rotation_z(0.006 * sway));
        let chest = Mat4::from_translation(Vec3::new(0.0, 0.0016 * breath, 0.0))
            * around(
                Vec3::new(0.0, 1.0, 0.0),
                Quat::from_rotation_z(-0.004 * sway) * Quat::from_rotation_x(0.004 * breath),
            );
        let head_q = Quat::from_rotation_y(self.head.x) * Quat::from_rotation_x(self.head.y);
        let head = around(marks.neck, head_q);
        let eye_q = Quat::from_rotation_y(self.eyes.x) * Quat::from_rotation_x(self.eyes.y);
        let world = [
            root,
            root * chest,
            root * chest * head,
            root * chest * head * around(marks.eyes[0], eye_q),
            root * chest * head * around(marks.eyes[1], eye_q),
            root * chest * head * around(marks.eyes[0], Quat::from_rotation_x(-1.0 * close)),
            root * chest * head * around(marks.eyes[1], Quat::from_rotation_x(-1.0 * close)),
        ];
        let basis = verse_content::basis();
        let inverse = basis.inverse();
        world.iter().map(|w| inverse * *w * basis).collect()
    }
}

// ------------------------------------------------------- content and cache

/// The pack (the hall, its props, and her), from the cache when her
/// sources have not changed since it was written.
fn content(a: &Args, dir: &Path) -> Result<(Pack, Option<Landmarks>), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let layout = layout();
    let stamp = |p: &Path| -> Result<String, String> {
        let m = std::fs::metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let t = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        Ok(format!("{}:{}:{}", p.display(), m.len(), t))
    };
    let key = if a.no_draconia {
        format!("v{CACHE_VERSION} castle only")
    } else {
        format!(
            "v{CACHE_VERSION} {} {} tex{}",
            stamp(&a.model)?,
            stamp(&a.lower)?,
            a.textures
        )
    };
    let key_path = dir.join("key.txt");
    if std::fs::read_to_string(&key_path).ok().as_deref() == Some(key.as_str())
        && let Ok(pack) = read_pack(dir)
    {
        let marks = match pack.models.get(DRACONIA) {
            Some(m) => Some(landmarks(m)?),
            None => None,
        };
        eprintln!(
            "draconia-world: her model from the cache ({})",
            a.model.display()
        );
        return Ok((pack, marks));
    }
    // Rebuild: clear the old textures first.
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let started = Instant::now();
    let mut pack = Pack {
        inventory: None,
        version: 1,
        source_revision: "draconia-world".into(),
        models: Default::default(),
        textures: vec![],
        placements: vec![],
    };
    flat::white_texture(&mut pack, dir)?;
    verse_content::compiler::great_crypt::install(&mut pack, dir, crypt_fight::model)?;
    pack.placements = placements(&layout);
    let mut marks = None;
    if !a.no_draconia {
        eprintln!(
            "draconia-world: importing her latest export (once per round): {}",
            a.model.display()
        );
        let options = characters::ImportOptions {
            max_texture_edge: a.textures,
        };
        let mut her = characters::import_with(&mut pack, dir, &a.model, options)?;
        let lower = characters::import_with(&mut pack, dir, &a.lower, options)?;
        her.surfaces.extend(lower.surfaces);
        marks = Some(rig(&mut her)?);
        let strands = groom(&mut her, &pack, dir)?;
        for (level, name) in STRANDS.iter().enumerate() {
            let model = if level == 0 {
                strands.clone()
            } else {
                thinned(&strands, THIN[level])?
            };
            pack.models.insert((*name).into(), model);
        }
        pack.models.insert(DRACONIA.into(), her);
    }
    pack.validate()?;
    write_pack(dir, &pack)?;
    std::fs::write(&key_path, &key).map_err(|e| e.to_string())?;
    eprintln!(
        "draconia-world: built in {:.1} s, cached in {}",
        started.elapsed().as_secs_f32(),
        dir.display()
    );
    Ok((pack, marks))
}

/// The pack as JSON without its geometry, and the geometry as raw
/// little-endian arrays beside it (JSON of three million vertices would
/// take a minute to read).
fn write_pack(dir: &Path, pack: &Pack) -> Result<(), String> {
    let mut light = pack.clone();
    let mut bin = std::io::BufWriter::new(
        std::fs::File::create(dir.join("geometry.bin")).map_err(|e| e.to_string())?,
    );
    for model in light.models.values_mut() {
        for s in &mut model.surfaces {
            let mut put = |x: u32| bin.write_all(&x.to_le_bytes()).map_err(|e| e.to_string());
            put(s.vertices.len() as u32)?;
            put(s.indices.len() as u32)?;
            for v in &s.vertices {
                for x in v.position.iter().chain(&v.normal).chain(&v.uv) {
                    put(x.to_bits())?;
                }
                for j in v.joints {
                    put(j)?;
                }
                for w in v.weights {
                    put(w.to_bits())?;
                }
            }
            for &i in &s.indices {
                put(i)?;
            }
            s.vertices.clear();
            s.indices.clear();
        }
    }
    bin.flush().map_err(|e| e.to_string())?;
    let json = serde_json::to_vec(&light).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("pack.json"), json).map_err(|e| e.to_string())
}

fn read_pack(dir: &Path) -> Result<Pack, String> {
    let json = std::fs::read(dir.join("pack.json")).map_err(|e| e.to_string())?;
    let mut pack: Pack = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    std::fs::File::open(dir.join("geometry.bin"))
        .and_then(|mut f| f.read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<&[u32], String> {
        let s = words.get(at..at + n).ok_or("The geometry cache is short")?;
        at += n;
        Ok(s)
    };
    let f = |x: u32| f32::from_bits(x);
    for model in pack.models.values_mut() {
        for s in &mut model.surfaces {
            let head = take(2)?;
            let (nv, ni) = (head[0] as usize, head[1] as usize);
            let raw = take(nv * 16)?;
            s.vertices = raw
                .chunks_exact(16)
                .map(|w| Vertex {
                    position: [f(w[0]), f(w[1]), f(w[2])],
                    normal: [f(w[3]), f(w[4]), f(w[5])],
                    uv: [f(w[6]), f(w[7])],
                    joints: [w[8], w[9], w[10], w[11]],
                    weights: [f(w[12]), f(w[13]), f(w[14]), f(w[15])],
                })
                .collect();
            s.indices = take(ni)?.to_vec();
        }
    }
    Ok(pack)
}

// ------------------------------------------------------------- the world

struct World {
    renderer: Renderer,
    layout: Vec<(&'static str, f32, f32, f32, f32)>,
    marks: Option<Landmarks>,
    idle: Idle,
    scene: physics::queries::Scene,
    walker: physics::character::Character,
    yaw: f32,
    pitch: f32,
    clock: f32,
}

fn placed() -> Mat4 {
    Mat4::from_translation(HER_AT) * Mat4::from_rotation_y(HER_YAW)
}

const WALKER: physics::character::Settings = physics::character::Settings {
    radius: 0.22,
    height: 1.75,
    step_height: 0.42,
    ground_snap: 0.15,
    slope_cos: std::f64::consts::FRAC_1_SQRT_2,
    gravity: 19.62,
    jump_speed: 5.0,
};

impl World {
    fn load(a: &Args, width: u32, height: u32) -> Result<Self, String> {
        let started = Instant::now();
        if !a.no_draconia {
            for (path, what) in [
                (
                    &a.model,
                    "her MetaHuman export (the parity pipeline's latest round)",
                ),
                (
                    &a.lower,
                    "her lower body (scripts/verse/draconia_world_body.py)",
                ),
            ] {
                if !path.exists() {
                    return Err(format!(
                        "Draconia's private assets are not on this computer: {what} is missing at {}.\n\
                         They are built by the private MetaHuman pipeline (docs/verse/draconia/parity.md) \
                         and never committed. Run with --no-draconia to walk the castle alone.",
                        path.display()
                    ));
                }
            }
        }
        let dir = home().join("work/film-out/draconia/world/cache");
        let (pack, marks) = content(a, &dir)?;
        let layout = layout();
        let statics = chamber::static_instances(&pack, Vec3::ZERO)
            .into_iter()
            .map(|mut i| {
                i.emission = Vec3::ONE;
                i
            })
            .collect::<Vec<_>>();
        let atlas = original::atlas()?;
        let budget = verse_engine::loading::Budget {
            manifest_bytes: 1024 * 1024 * 1024,
            encoded_file_bytes: 256 * 1024 * 1024,
            encoded_total_bytes: 1024 * 1024 * 1024,
            rgba_total_bytes: 1024 * 1024 * 1024,
            decoder_workspace_bytes: 256 * 1024 * 1024,
        };
        let prepared = verse_engine::loading::Prepared::load(pack, &dir, budget)?;
        let mut renderer = Renderer::from_prepared(prepared, width, height, &atlas, &statics)?;
        renderer.hair_coverage = std::env::var_os("DRACONIA_WORLD_DITHER").is_none();
        if !a.no_draconia {
            eprintln!(
                "draconia-world: Draconia {} from {}",
                round(&a.model).map_or("(round not stamped)".into(), |r| format!("round {r}")),
                a.model.display()
            );
        }
        eprintln!(
            "draconia-world: ready in {:.1} s",
            started.elapsed().as_secs_f32()
        );
        Ok(Self {
            renderer,
            scene: collision(&layout)?,
            layout,
            marks,
            idle: Idle::new(),
            walker: physics::character::Character::new(SPAWN.as_dvec3()),
            yaw: 0.0,
            pitch: -0.06,
            clock: 0.0,
        })
    }

    fn eye(&self) -> Vec3 {
        self.walker.feet.as_vec3() + Vec3::Y * EYE_HEIGHT
    }

    fn forward(&self) -> Vec3 {
        Vec3::new(
            -self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }

    /// Walks the player one step of `dt` seconds: `wish` is (strafe right,
    /// forward) in -1..1.
    fn walk(&mut self, wish: Vec2, run: bool, jump: bool, dt: f32) -> Result<(), String> {
        let speed = if run { 4.2 } else { 1.9 };
        let forward = Vec3::new(-self.yaw.sin(), 0.0, -self.yaw.cos());
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        let v = (forward * wish.y + right * wish.x).clamp_length_max(1.0) * speed;
        let mut left = dt;
        while left > 1e-4 {
            let step = left.min(1.0 / 60.0);
            self.walker.step(
                &self.scene,
                physics::queries::Filter::blocking(0),
                WALKER,
                DVec3::new(f64::from(v.x), 0.0, f64::from(v.z)),
                jump,
                f64::from(step),
            )?;
            left -= step;
        }
        Ok(())
    }

    /// Draws one frame from `eye` looking at `target`; `live` keeps it on
    /// the GPU, else returns its pixels.
    fn frame(&mut self, eye: Vec3, target: Vec3, dt: f32, live: bool) -> Result<Vec<u8>, String> {
        self.clock += dt;
        let t = self.clock;
        let [w, h] = self.renderer.dimensions();
        let aspect = w as f32 / h.max(1) as f32;
        let view_proj = Mat4::perspective_rh(55f32.to_radians(), aspect, 0.03, 90.0)
            * Mat4::look_at_rh(eye, target, Vec3::Y);
        let mut instances = Vec::new();
        if let Some(marks) = self.marks {
            let viewer = placed().inverse().transform_point3(eye);
            let palette = self.idle.pose(&marks, t, dt, viewer);
            // Neighbouring strand levels dissolve into each other over a
            // band of distance (alpha to coverage), never a switch.
            let distance = eye.distance(HER_AT + Vec3::Y * 1.6);
            let mut models = vec![DRACONIA];
            for (level, weight) in strand_levels(distance).into_iter().enumerate() {
                if weight > 0.01 {
                    models.push(STRANDS[level]);
                    self.renderer
                        .set_hair_fade(STRANDS[level], Some(weight.min(1.0)))?;
                }
            }
            for model in models {
                if std::env::var_os("DRACONIA_WORLD_REST").is_none() {
                    self.renderer
                        .set_pose_override(model, Some(palette.clone()))?;
                }
                instances.push(Instance {
                    mount: None,
                    actor: None,
                    model: model.into(),
                    transform: placed() * verse_content::basis(),
                    animation: 0.into(),
                    time: 0.0,
                    animation_epoch: None,
                    emission: Vec3::ONE,
                });
            }
        }
        let ui = UiBatch::default();
        let light = lighting(&self.layout, t);
        let view = View { view_proj, eye };
        if live {
            self.renderer.draw_live(view, &instances, &ui, &light)?;
            Ok(Vec::new())
        } else {
            self.renderer.draw(view, &instances, &ui, &light)
        }
    }
}

// ----------------------------------------------------- captures and clips

/// A camera path: positions and targets over `seconds`, eased between keys.
struct Path3 {
    keys: Vec<(f32, Vec3, Vec3)>,
}

impl Path3 {
    fn at(&self, t: f32) -> (Vec3, Vec3) {
        let k = &self.keys;
        if t <= k[0].0 {
            return (k[0].1, k[0].2);
        }
        for w in k.windows(2) {
            if t <= w[1].0 {
                let u = smooth(w[0].0, w[1].0, t);
                return (w[0].1.lerp(w[1].1, u), w[0].2.lerp(w[1].2, u));
            }
        }
        let last = k[k.len() - 1];
        (last.1, last.2)
    }
    fn seconds(&self) -> f32 {
        self.keys[self.keys.len() - 1].0
    }
}

/// The walk: down from the landing, up to her, close to her face, round
/// her to a three-quarter view, and back.
fn walk_path() -> Path3 {
    let face = HER_AT + Vec3::new(0.0, 1.6, 0.0);
    let e = EYE_HEIGHT;
    Path3 {
        keys: vec![
            (
                0.0,
                Vec3::new(0.0, 1.5 + e, 15.2),
                face + Vec3::new(0.0, -0.3, 0.0),
            ),
            (
                2.5,
                Vec3::new(0.3, 1.5 + e, 12.6),
                face + Vec3::new(0.0, -0.2, 0.0),
            ),
            (5.5, Vec3::new(0.6, e, 6.5), face),
            (9.0, Vec3::new(0.2, e, 1.1), face),
            (11.5, face + Vec3::new(0.05, 0.02, 0.55), face),
            (14.0, face + Vec3::new(0.55, 0.0, 0.5), face),
            (
                16.5,
                face + Vec3::new(1.4, 0.05, 0.3),
                face + Vec3::new(0.0, -0.2, 0.0),
            ),
            (
                19.0,
                Vec3::new(-1.6, e, -0.8),
                face + Vec3::new(0.0, -0.3, 0.0),
            ),
            (
                22.0,
                Vec3::new(-3.0, e, 4.5),
                face + Vec3::new(0.0, -0.5, 0.0),
            ),
        ],
    }
}

fn write_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut png = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(rgba)
        .map_err(|e| e.to_string())?;
    println!("wrote {}", path.display());
    Ok(())
}

/// The evidence stills: the hall wide, walking up to her, her face close,
/// and a three-quarter view.
fn capture(a: &Args, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let [w, h] = a.size;
    let mut world = World::load(a, w, h)?;
    let face = HER_AT + Vec3::new(0.0, 1.6, 0.0);
    let shots = [
        (
            "wide",
            Vec3::new(-2.6, 2.6, 8.5),
            face + Vec3::new(0.0, -0.5, -3.0),
        ),
        (
            "approach",
            Vec3::new(0.7, EYE_HEIGHT, 3.2),
            face + Vec3::new(0.0, -0.25, 0.0),
        ),
        (
            "face",
            face + Vec3::new(0.04, 0.0, 0.5),
            face + Vec3::new(0.0, -0.02, 0.0),
        ),
        (
            "three-quarter",
            face + Vec3::new(0.75, 0.02, 0.8),
            face + Vec3::new(0.0, -0.08, 0.0),
        ),
        (
            "full",
            HER_AT + Vec3::new(0.45, 1.35, 2.6),
            HER_AT + Vec3::new(0.0, 0.92, 0.0),
        ),
    ];
    for (name, eye, target) in shots {
        // Settle her gaze on the camera first.
        for _ in 0..90 {
            world.frame(eye, target, 1.0 / 60.0, true)?;
        }
        let rgba = world.frame(eye, target, 1.0 / 60.0, false)?;
        write_png(&dir.join(format!("draconia-world-{name}.png")), w, h, &rgba)?;
    }
    // A check of the rig: the face shot mid-blink.
    let (eye, target) = (
        face + Vec3::new(0.04, 0.0, 0.5),
        face + Vec3::new(0.0, -0.02, 0.0),
    );
    world.idle.blink_at = world.clock + 1.0 / 60.0 - 0.07;
    world.idle.next_blink = world.clock + 5.0;
    let rgba = world.frame(eye, target, 1.0 / 60.0, false)?;
    write_png(&dir.join("check-blink.png"), w, h, &rgba)?;
    Ok(())
}

/// Her hair at every distance: from 1 m to 15 m in 1 m steps, each frame
/// rendered as the player sees it (1920x1080, the walking lens), cropped
/// round her head and scaled to one tile, in a 5x3 sheet read row by row.
fn lod_sheet(a: &Args, out: &Path) -> Result<(), String> {
    let [w, h] = a.size;
    let mut world = World::load(a, w, h)?;
    let face = HER_AT + Vec3::new(0.0, 1.62, 0.0);
    let tile = 360usize;
    let (cols, rows) = (5usize, 3usize);
    let mut sheet = vec![0u8; cols * tile * rows * tile * 4];
    for i in 0..15 {
        let d = (i + 1) as f32;
        let dir = Vec3::new(0.3, 0.0, 1.0).normalize();
        let eye = face + dir * d + Vec3::Y * 0.04;
        for _ in 0..40 {
            world.frame(eye, face, 1.0 / 60.0, true)?;
        }
        let rgba = world.frame(eye, face, 1.0 / 60.0, false)?;
        // Her head is about 0.55 m of crop at every distance.
        let half_fov = 27.5f32.to_radians();
        let px_per_m = h as f32 / (2.0 * d * half_fov.tan());
        let side = (0.55 * px_per_m).clamp(24.0, h as f32);
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let (x0, y0) = (cx - side * 0.5, cy - side * 0.5);
        let (tx, ty) = ((i % cols) * tile, (i / cols) * tile);
        for y in 0..tile {
            for x in 0..tile {
                // Bilinear sample of the crop.
                let sx =
                    (x0 + (x as f32 + 0.5) / tile as f32 * side - 0.5).clamp(0.0, w as f32 - 2.0);
                let sy =
                    (y0 + (y as f32 + 0.5) / tile as f32 * side - 0.5).clamp(0.0, h as f32 - 2.0);
                let (ix, iy) = (sx as usize, sy as usize);
                let (fx, fy) = (sx - ix as f32, sy - iy as f32);
                let at =
                    |x: usize, y: usize, c: usize| f32::from(rgba[(y * w as usize + x) * 4 + c]);
                let o = ((ty + y) * cols * tile + tx + x) * 4;
                for c in 0..3 {
                    let v = at(ix, iy, c) * (1.0 - fx) * (1.0 - fy)
                        + at(ix + 1, iy, c) * fx * (1.0 - fy)
                        + at(ix, iy + 1, c) * (1.0 - fx) * fy
                        + at(ix + 1, iy + 1, c) * fx * fy;
                    sheet[o + c] = v.round() as u8;
                }
                sheet[o + 3] = 255;
            }
        }
        eprintln!("lod sheet: {d} m");
    }
    write_png(out, (cols * tile) as u32, (rows * tile) as u32, &sheet)
}

/// The walk-around clip, 60 frames a second, through ffmpeg.
fn walk_clip(a: &Args, out: &Path) -> Result<(), String> {
    let [w, h] = a.size;
    let mut world = World::load(a, w, h)?;
    let path = walk_path();
    let fps = 60.0;
    let frames = (path.seconds() * fps) as usize;
    let mut ffmpeg = std::process::Command::new("ffmpeg")
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
        .arg(format!("{w}x{h}"))
        .args([
            "-framerate",
            "60",
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-crf",
            "17",
        ])
        .arg(out)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    let stdin = ffmpeg.stdin.as_mut().ok_or("ffmpeg input")?;
    for i in 0..frames {
        let (eye, target) = path.at(i as f32 / fps);
        let rgba = world.frame(eye, target, 1.0 / fps, false)?;
        stdin.write_all(&rgba).map_err(|e| e.to_string())?;
    }
    drop(ffmpeg.stdin.take());
    ffmpeg.wait().map_err(|e| e.to_string())?;
    println!("wrote {} ({frames} frames)", out.display());
    Ok(())
}

// ------------------------------------------------------------- the window

mod window {
    use super::*;
    use winit::{
        application::ApplicationHandler,
        dpi::PhysicalSize,
        event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        keyboard::{KeyCode, PhysicalKey},
        window::{CursorGrabMode, Window, WindowId},
    };

    pub fn run(a: Args) -> Result<(), String> {
        let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
        event_loop.set_control_flow(ControlFlow::Poll);
        let mut app = App {
            args: a,
            window: None,
            world: None,
            presenter: None,
            keys: HashSet::new(),
            captured: false,
            last: Instant::now(),
            error: None,
            frames: Vec::new(),
            near: Vec::new(),
            title_at: Instant::now(),
            shots: 0,
            bench_started: None,
        };
        event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
        if let Some(report) = app.report() {
            println!("{report}");
        }
        app.error.map_or(Ok(()), Err)
    }

    struct App {
        args: Args,
        window: Option<Arc<Window>>,
        world: Option<World>,
        presenter: Option<verse_imported::imported::WindowPresenter>,
        keys: HashSet<KeyCode>,
        captured: bool,
        last: Instant,
        error: Option<String>,
        /// Frame intervals, seconds, and whether the camera was within 3 m
        /// of her face (full-detail hair) for that frame.
        frames: Vec<f32>,
        near: Vec<bool>,
        title_at: Instant,
        shots: u32,
        bench_started: Option<Instant>,
    }

    impl App {
        fn capture_pointer(&mut self, on: bool) {
            let Some(window) = &self.window else { return };
            self.captured = on;
            if on {
                let _ = window
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
                window.set_cursor_visible(false);
            } else {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(true);
            }
        }

        fn report(&self) -> Option<String> {
            // Skip the first second (shader warm-up and the first uploads).
            let mut skip = 0.0;
            let kept: Vec<(f32, bool)> = self
                .frames
                .iter()
                .copied()
                .zip(self.near.iter().copied())
                .filter(|&(f, _)| {
                    skip += f;
                    skip > 1.0
                })
                .collect();
            let frames: Vec<f32> = kept.iter().map(|k| k.0).collect();
            if frames.len() < 10 {
                return None;
            }
            let close: Vec<f32> = kept.iter().filter(|k| k.1).map(|k| k.0).collect();
            let close_fps = close.len() as f32 / close.iter().sum::<f32>().max(1e-6);
            let total: f32 = frames.iter().sum();
            let mut sorted = frames.clone();
            sorted.sort_by(f32::total_cmp);
            let p = |q: f32| sorted[((sorted.len() - 1) as f32 * q) as usize] * 1000.0;
            let size = self
                .world
                .as_ref()
                .map(|w| w.renderer.dimensions())
                .unwrap_or([0, 0]);
            Some(format!(
                "draconia-world: {} frames at {}x{} in {:.1} s: {:.1} fps average ({:.1} fps within 3 m of her face, {} frames); frame time median {:.1} ms, 95th percentile {:.1} ms, 99th {:.1} ms",
                frames.len(),
                size[0],
                size[1],
                total,
                frames.len() as f32 / total,
                close_fps,
                close.len(),
                p(0.5),
                p(0.95),
                p(0.99)
            ))
        }

        fn step_and_draw(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
            let now = Instant::now();
            let dt = (now - self.last).as_secs_f32().min(0.1);
            self.last = now;
            let window = self.window.clone().ok_or("No window")?;
            let world = self.world.as_mut().ok_or("No world")?;
            let size = window.inner_size();
            if size.width == 0 || size.height == 0 {
                return Ok(());
            }
            world.renderer.resize(size.width, size.height)?;
            let (eye, target) = if let Some(started) = self.bench_started {
                let t = started.elapsed().as_secs_f32();
                if t > self.args.benchmark.unwrap_or(0.0) {
                    event_loop.exit();
                }
                walk_path().at(t % walk_path().seconds())
            } else {
                let key = |k| self.keys.contains(&k);
                let axis = |p: bool, n: bool| f32::from(u8::from(p)) - f32::from(u8::from(n));
                let wish = Vec2::new(
                    axis(key(KeyCode::KeyD), key(KeyCode::KeyA)),
                    axis(key(KeyCode::KeyW), key(KeyCode::KeyS)),
                );
                let run = key(KeyCode::ShiftLeft) || key(KeyCode::ShiftRight);
                world.walk(wish, run, key(KeyCode::Space), dt)?;
                let eye = world.eye();
                (eye, eye + world.forward())
            };
            world.frame(eye, target, dt, true)?;
            let presenter = self.presenter.as_mut().ok_or("No presenter")?;
            world
                .renderer
                .present_window(presenter, [size.width, size.height])?;
            self.frames.push(dt);
            self.near.push(eye.distance(HER_AT + Vec3::Y * 1.6) < 3.0);
            if self.title_at.elapsed().as_secs_f32() > 1.0 {
                let recent: Vec<f32> = self.frames.iter().rev().take(60).copied().collect();
                let fps = recent.len() as f32 / recent.iter().sum::<f32>().max(1e-6);
                window.set_title(&format!(
                    "Draconia — the castle · {fps:.0} fps · click to look, WASD, Shift to run, F12 screenshot, Esc"
                ));
                self.title_at = Instant::now();
            }
            Ok(())
        }

        fn screenshot(&mut self) -> Result<(), String> {
            let world = self.world.as_mut().ok_or("No world")?;
            let eye = world.eye();
            let target = eye + world.forward();
            let [w, h] = world.renderer.dimensions();
            let rgba = world.frame(eye, target, 0.0, false)?;
            self.shots += 1;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            write_png(
                &home().join(format!(
                    "Downloads/draconia-world-shot-{stamp}-{}.png",
                    self.shots
                )),
                w,
                h,
                &rgba,
            )
        }
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }
            let result = (|| -> Result<(), String> {
                let [w, h] = self.args.size;
                let attributes = Window::default_attributes()
                    .with_title("Draconia — the castle (loading)")
                    .with_inner_size(PhysicalSize::new(w, h));
                let window = Arc::new(
                    event_loop
                        .create_window(attributes)
                        .map_err(|e| e.to_string())?,
                );
                let size = window.inner_size();
                let world = World::load(&self.args, size.width, size.height)?;
                let presenter = world.renderer.attach_window(window.clone())?;
                self.window = Some(window);
                self.world = Some(world);
                self.presenter = Some(presenter);
                self.last = Instant::now();
                if self.args.benchmark.is_some() {
                    self.bench_started = Some(Instant::now());
                }
                Ok(())
            })();
            if let Err(e) = result {
                self.error = Some(e);
                event_loop.exit();
            }
        }

        fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
            if let (true, DeviceEvent::MouseMotion { delta }, Some(world)) =
                (self.captured, event, self.world.as_mut())
            {
                world.yaw -= delta.0 as f32 * 0.0022;
                world.pitch = (world.pitch - delta.1 as f32 * 0.0022).clamp(-1.45, 1.45);
            }
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Focused(false) => {
                    self.keys.clear();
                    self.capture_pointer(false);
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => self.capture_pointer(true),
                WindowEvent::KeyboardInput { event, .. } => {
                    let PhysicalKey::Code(key) = event.physical_key else {
                        return;
                    };
                    if event.state != ElementState::Pressed {
                        self.keys.remove(&key);
                        return;
                    }
                    self.keys.insert(key);
                    if event.repeat {
                        return;
                    }
                    match key {
                        KeyCode::Escape if self.captured => self.capture_pointer(false),
                        KeyCode::Escape => event_loop.exit(),
                        KeyCode::F12 => {
                            if let Err(e) = self.screenshot() {
                                eprintln!("draconia-world: screenshot failed: {e}");
                            }
                        }
                        _ => {}
                    }
                }
                WindowEvent::RedrawRequested => {
                    if event_loop.exiting() {
                        return;
                    }
                    if let Err(e) = self.step_and_draw(event_loop) {
                        self.error = Some(e);
                        event_loop.exit();
                    }
                }
                _ => {}
            }
        }

        fn about_to_wait(&mut self, _: &ActiveEventLoop) {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}

/// Walks into Draconia, then into the west wall, and reports where the
/// walker stopped: a check that collisions hold.
fn check_walk() -> Result<(), String> {
    let scene = collision(&layout())?;
    let mut walker = physics::character::Character::new(SPAWN.as_dvec3());
    let filter = physics::queries::Filter::blocking(0);
    for (name, v, seconds) in [
        ("toward her", DVec3::new(0.0, 0.0, -4.2), 8.0),
        ("west into the aisle wall", DVec3::new(-4.2, 0.0, 0.0), 6.0),
    ] {
        for _ in 0..(seconds * 60.0) as usize {
            walker.step(&scene, filter, WALKER, v, false, 1.0 / 60.0)?;
        }
        println!("{name}: stopped at {:.2?}", walker.feet);
    }
    Ok(())
}

fn main() {
    let result = args().and_then(|a| {
        if a.check_walk {
            check_walk()
        } else if let Some(out) = a.lod_sheet.clone() {
            lod_sheet(&a, &out)
        } else if let Some(dir) = a.capture.clone() {
            capture(&a, &dir)
        } else if let Some(out) = a.walk.clone() {
            walk_clip(&a, &out)
        } else {
            window::run(a)
        }
    });
    if let Err(e) = result {
        eprintln!("draconia-world: {e}");
        std::process::exit(1);
    }
}
