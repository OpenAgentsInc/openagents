//! A MetaHuman casts Meteor Swarm at the CAD Lab rover, filmed in Verse:
//! her character, the meteors, the rover's break-up, fire and smoke, all
//! in one process and one renderer (Verse's imported renderer, with its
//! skin, eye, cornea, hair and cloth shading).
//!
//! - **The caster** is a static MetaHuman export (`scripts/unreal/dna_dump`
//!   and `scripts/blender/metahuman_import.py`) rigged with MetaHuman's own
//!   body skeleton and skin weights (`verse::imported::metahuman_rig`):
//!   she raises her arm, casts, points at the rover and turns her head and
//!   eyes to it; her strand groom fades between levels of detail
//!   (`verse::imported::groom`).
//! - **The rover** is `verse::zones::meteor_rover`: 51 rigid bodies,
//!   bolted and pinned joints that break under the blast, a Kinney-Graham
//!   blast per meteor ([`Rover::swarm`]: three round it, the fourth on it).
//!   It is simulated before rendering at 120 samples a second, so the film
//!   can slow time at the strike and hurry it while the wreck settles; the
//!   last shot holds on every piece at rest, and the film refuses to render
//!   if any piece ends floating, sunk or still falling.
//! - **Fire and smoke**: each meteor as a burning orb with a trail, each
//!   blast as a fireball, flash, fire column and smoke
//!   (`verse::imported::meteor_swarm`), the chassis's burning fragments
//!   trailing smoke, and fires left burning in the crater and on the wreck.
//! - **The set**: a meadow at dusk (grass, a few trees, boulders, wild
//!   flowers), a sky dome, the afterglow behind the rover.
//!
//! Every frame is the mean of `--subframes` renders spread over the shutter
//! (motion blur) and the lens (depth of field), with sub-pixel jitter.
//!
//! Usage: metahuman_meteor_film --model MH.glb --lower LOWER.glb
//! [--expression SMILE.glb] --skeleton DIR --pose POSE.txt --cache DIR
//! (--out OUT.mp4 | --still SECONDS OUT.png) [--stills DIR] [--size WxH]
//! [--subframes N] [--textures EDGE] [--paint KEY=RRGGBB]... [--hair R,G,B]
//! [--plain MINX,MINY,MINZ,MAXX,MAXY,MAXZ=RRGGBB]... [--sleeve FULL,NONE]
//! [--camera EX,EY,EZ,TX,TY,TZ] [--rest]
//!
//! - `--model`: the MetaHuman export (head, groom and clothes; a skinless
//!   glTF); `--lower` adds her lower body; `--expression` is a second
//!   export of the same character with another face (the last shot uses
//!   it).
//! - `--skeleton DIR`: `dna_dump BODY.dna DIR --joints POSE.txt --skin`'s
//!   `joints.txt`, `body_lod0_mesh.obj` and `body_lod0_mesh.skin.txt`;
//!   `--pose` is that `POSE.txt` (the export's own pose).
//! - `--hair` tints the groom (linear RGB multipliers); `--plain` gives
//!   the surfaces whose bounds are the box (within 1.5 cm; her frame:
//!   metres, feet at the origin, facing -Z) a plain color with no texture.
//! - `--stills DIR` also writes three stills (the cast, the strike, the
//!   reaction) beside the film.
//!
//! MetaHuman assets are private: this example names none and reads only
//! the paths it is given. Keep its outputs local.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::{Mat4, Quat, Vec2, Vec3};
use verse::imported::{
    Instance, Renderer, characters, flat, groom,
    lighting::{Light, Lighting},
    metahuman_rig::{BodySkin, Pose, Rig, Skeleton, parse_deltas},
    meteor_swarm, original, pack_cache,
};
use verse::render::View;
use verse::ui::UiBatch;
use verse::zones::meteor_rover::{Livery, Rover};
use verse_engine::assets::{Pack, Surface, Texture, Vertex};
use verse_engine::camera_rig::{CameraSample, fov_y, lens_sample, lens_view_projection};
use verse_engine::material::Shading;

/// Bump when the cached caster's layout or rig changes.
const CACHE_VERSION: u32 = 5;
const CASTER: &str = "caster";
const CASTER_EXPRESSION: &str = "caster-expression";
const STRANDS: [&str; 3] = ["caster-strands", "caster-strands-1", "caster-strands-2"];
const THIN: [usize; 3] = [1, 3, 8];
const BANDS: [(f32, f32); 2] = [(2.2, 3.2), (5.2, 6.8)];

/// The film: 30 frames a second.
const FPS: f32 = 30.0;
/// The rover's place, 20 m in front of her (she stands at the origin and
/// faces -Z), turned so she sees it three-quarters on.
const ROVER_AT: Vec3 = Vec3::new(1.5, 0.0, -20.0);
const ROVER_HEADING: f32 = 0.55;
/// When the first meteor lands (film seconds), and how fast the meteors
/// fall as seen (m/s; the physics strikes at the meteors' real speed).
const FIRST_LANDS: f32 = 5.9;
const FALL_SPEED: f32 = 60.0;
const FALL_SHOWN: f32 = 3.0;
/// Samples of the rover a second.
const RATE: f32 = 120.0;

// ------------------------------------------------------------- arguments

struct Args {
    model: PathBuf,
    lower: PathBuf,
    expression: Option<PathBuf>,
    skeleton: PathBuf,
    pose: PathBuf,
    cache: PathBuf,
    out: Option<PathBuf>,
    still: Option<(f32, PathBuf)>,
    stills: Option<PathBuf>,
    size: [u32; 2],
    subframes: usize,
    textures: u32,
    paint: Vec<String>,
    hair: Vec3,
    plain: Vec<(Vec3, Vec3, Vec3)>,
    /// A camera for checks: eye and target, overriding the shot's.
    camera: Option<(Vec3, Vec3)>,
    /// Her rest pose, for checks.
    rest: bool,
    /// How far from her arms' bones a garment follows them, cm.
    sleeve: Option<(f32, f32)>,
}

fn args() -> Result<Args, String> {
    let mut a = Args {
        model: PathBuf::new(),
        lower: PathBuf::new(),
        expression: None,
        skeleton: PathBuf::new(),
        pose: PathBuf::new(),
        cache: PathBuf::new(),
        out: None,
        still: None,
        stills: None,
        size: [1920, 1080],
        subframes: 8,
        textures: 2048,
        paint: vec![],
        hair: Vec3::ONE,
        plain: vec![],
        camera: None,
        rest: false,
        sleeve: None,
    };
    let mut it = std::env::args().skip(1);
    let floats = |s: &str| -> Result<Vec<f32>, String> {
        s.split(',')
            .map(|x| {
                x.trim()
                    .parse::<f32>()
                    .map_err(|_| format!("bad number {x}"))
            })
            .collect()
    };
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--model" => a.model = value()?.into(),
            "--lower" => a.lower = value()?.into(),
            "--expression" => a.expression = Some(value()?.into()),
            "--skeleton" => a.skeleton = value()?.into(),
            "--pose" => a.pose = value()?.into(),
            "--cache" => a.cache = value()?.into(),
            "--out" => a.out = Some(value()?.into()),
            "--still" => {
                let t: f32 = value()?.parse().map_err(|_| "bad --still seconds")?;
                a.still = Some((t, value()?.into()));
            }
            "--stills" => a.stills = Some(value()?.into()),
            "--size" => {
                let v = value()?;
                let (w, h) = v.split_once('x').ok_or("--size is WxH")?;
                a.size = [
                    w.parse().map_err(|_| "bad width")?,
                    h.parse().map_err(|_| "bad height")?,
                ];
            }
            "--subframes" => a.subframes = value()?.parse().map_err(|_| "bad --subframes")?,
            "--textures" => a.textures = value()?.parse().map_err(|_| "bad --textures")?,
            "--paint" => a.paint.push(value()?),
            "--hair" => {
                let v = floats(&value()?)?;
                if v.len() != 3 {
                    return Err("--hair is R,G,B".into());
                }
                a.hair = Vec3::new(v[0], v[1], v[2]);
            }
            "--plain" => {
                let v = value()?;
                let (b, c) = v.split_once('=').ok_or("--plain is BOX=RRGGBB")?;
                let b = floats(b)?;
                let c = verse::zones::meteor_rover::linear(c).ok_or("bad --plain color")?;
                if b.len() != 6 {
                    return Err("--plain's box is MINX,MINY,MINZ,MAXX,MAXY,MAXZ".into());
                }
                a.plain.push((
                    Vec3::new(b[0], b[1], b[2]),
                    Vec3::new(b[3], b[4], b[5]),
                    Vec3::from(c),
                ));
            }
            "--camera" => {
                let v = floats(&value()?)?;
                if v.len() != 6 {
                    return Err("--camera is EX,EY,EZ,TX,TY,TZ".into());
                }
                a.camera = Some((Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5])));
            }
            "--rest" => a.rest = true,
            "--sleeve" => {
                let v = floats(&value()?)?;
                if v.len() != 2 {
                    return Err("--sleeve is FULL,NONE (cm)".into());
                }
                a.sleeve = Some((v[0], v[1]));
            }
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    for (path, what) in [
        (&a.model, "--model"),
        (&a.lower, "--lower"),
        (&a.skeleton, "--skeleton"),
        (&a.pose, "--pose"),
        (&a.cache, "--cache"),
    ] {
        if path.as_os_str().is_empty() {
            return Err(format!("{what} is required"));
        }
    }
    if a.out.is_none() && a.still.is_none() {
        return Err("Give --out OUT.mp4 or --still SECONDS OUT.png".into());
    }
    Ok(a)
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn bounds(points: impl Iterator<Item = Vec3>) -> (Vec3, Vec3) {
    points.fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(a, b), p| (a.min(p), b.max(p)),
    )
}

// ----------------------------------------------------------- the rover

/// The rover's break-up, simulated ahead: every body's pose at every
/// sample, its mesh, and the meteors.
struct Track {
    /// Per sample: (body, position, rotation).
    samples: Vec<Vec<(usize, Vec3, Quat)>>,
    /// Each body's mesh in its own frame, and its color.
    meshes: BTreeMap<usize, (Vec<Vec3>, Vec<Vec3>, Vec<u32>, [f32; 3])>,
    /// The fragments the deck's meteor broke off (they burn).
    burning: Vec<usize>,
    /// Each meteor: when it lands (sim seconds), where, which way it
    /// flies.
    meteors: Vec<(f32, Vec3, Vec3)>,
    /// When the last piece came to rest (sim seconds).
    settled: f32,
}

impl Track {
    fn simulate(paint: &[String], seconds: f32) -> Result<Self, String> {
        let livery = Livery::parse(paint)?;
        let mut rover = Rover::new(ROVER_AT, ROVER_HEADING, Vec3::ZERO);
        rover.repaint(|instance, color| livery.paint(instance, color));
        // Settle on the ground first.
        for _ in 0..60 {
            rover.step(1.0 / 60.0);
        }
        let meteors: Vec<(f32, Vec3, Vec3)> = rover
            .swarm()
            .into_iter()
            .map(|(delay, at, _)| (FIRST_LANDS + delay, at, rover.flight(at)))
            .collect();
        let radius: Vec<f32> = rover.swarm().into_iter().map(|s| s.2).collect();
        let mut track = Self {
            samples: vec![],
            meshes: BTreeMap::new(),
            burning: vec![],
            meteors,
            settled: 0.0,
        };
        let whole: Vec<usize> = rover.bodies().into_iter().map(|b| b.0).collect();
        let n = (seconds * RATE) as usize;
        let mut next = 0;
        let mut moving_until = 0.0f32;
        for i in 0..=n {
            let t = i as f32 / RATE;
            while next < track.meteors.len() && track.meteors[next].0 <= t {
                // The last lands on the deck, wherever the others left it.
                if next + 1 == track.meteors.len() {
                    let at = rover.deck();
                    track.meteors[next] = (track.meteors[next].0, at, rover.flight(at));
                }
                rover.blast(track.meteors[next].1, radius[next]);
                next += 1;
            }
            if i > 0 {
                rover.step(1.0 / RATE);
            }
            let mut frame = Vec::new();
            for (k, _, pos, rot) in rover.bodies() {
                if !track.meshes.contains_key(&k) {
                    let mesh = rover.body_mesh(k).ok_or("a body with no mesh")?;
                    track.meshes.insert(k, mesh);
                    if !whole.contains(&k) {
                        track.burning.push(k);
                    }
                }
                frame.push((k, pos.as_vec3(), rot.as_quat()));
            }
            if rover
                .resting()
                .iter()
                .any(|(_, _, _, speed, held)| *speed > 0.05 || !held)
            {
                moving_until = t;
            }
            track.samples.push(frame);
        }
        for (k, name, height, speed, held) in rover.resting() {
            if height < -verse::zones::meteor_rover::REST_CONTACT || !held || speed > 0.05 {
                return Err(format!(
                    "{name} ({k}) does not come to rest: {height:.3} m up, {speed:.3} m/s, held {held}"
                ));
            }
        }
        track.settled = moving_until;
        eprintln!(
            "film: the rover simulated for {seconds} s: {} bodies, {} burning, every piece at rest by {:.2} s",
            track.meshes.len(),
            track.burning.len(),
            moving_until
        );
        Ok(track)
    }

    /// Every body's pose at sim time `t`, between samples.
    fn at(&self, t: f32) -> Vec<(usize, Vec3, Quat)> {
        let x = (t * RATE).clamp(0.0, (self.samples.len() - 1) as f32);
        let i = x.floor() as usize;
        let f = x - i as f32;
        let j = (i + 1).min(self.samples.len() - 1);
        self.samples[i]
            .iter()
            .map(
                |&(k, p, q)| match self.samples[j].iter().find(|s| s.0 == k) {
                    Some(&(_, p2, q2)) => (k, p.lerp(p2, f), q.slerp(q2, f)),
                    None => (k, p, q),
                },
            )
            .collect()
    }

    fn position(&self, k: usize, t: f32) -> Option<Vec3> {
        self.at(t).into_iter().find(|b| b.0 == k).map(|b| b.1)
    }
}

// ----------------------------------------------------------- the timeline

/// Film time to simulation time: real time until the strike, a slow
/// strike (half speed), real time while the pieces fly, faster while the
/// wreck settles, then real time on the rest.
struct Clock {
    keys: Vec<(f32, f32)>,
}

impl Clock {
    fn new(settled: f32) -> Self {
        let strike = FIRST_LANDS + 0.9;
        let slow_from = strike - 0.35;
        let slow_to = strike + 1.25;
        let fly_to = 10.0;
        let sim_fly = slow_from + (slow_to - slow_from) * 0.5 + (fly_to - slow_to);
        // The debris shot ends half a second after the last piece rests.
        let settle_to = 12.3;
        let sim_settle = (settled + 0.5).max(sim_fly + (settle_to - fly_to));
        Self {
            keys: vec![
                (0.0, 0.0),
                (slow_from, slow_from),
                (slow_to, slow_from + (slow_to - slow_from) * 0.5),
                (fly_to, sim_fly),
                (settle_to, sim_settle),
                (30.0, sim_settle + (30.0 - settle_to)),
            ],
        }
    }

    fn sim(&self, t: f32) -> f32 {
        for w in self.keys.windows(2) {
            if t <= w[1].0 {
                let u = ((t - w[0].0) / (w[1].0 - w[0].0)).clamp(0.0, 1.0);
                return w[0].1 + (w[1].1 - w[0].1) * u;
            }
        }
        self.keys[self.keys.len() - 1].1
    }
}

/// The film's length, s.
const LENGTH: f32 = 14.4;

/// A shot: its span, the camera's eye and target at its start and end,
/// the lens (mm), and the f-stop.
struct Shot {
    from: f32,
    to: f32,
    eye: (Vec3, Vec3),
    target: (Vec3, Vec3),
    focal: f32,
    fstop: f32,
    /// Use the second expression for her face.
    expression: bool,
}

fn shots(track: &Track) -> Vec<Shot> {
    let face = Vec3::new(0.0, 1.6, 0.0);
    let rover = ROVER_AT + Vec3::Y * 0.6;
    let hit = track.meteors.last().map_or(ROVER_AT, |m| m.1);
    vec![
        // 1. Over her right shoulder: her hair, the rover waiting out on
        // the meadow; her hand comes up with the spell in it.
        Shot {
            from: 0.0,
            to: 3.2,
            eye: (Vec3::new(0.38, 1.8, 1.7), Vec3::new(0.34, 1.76, 1.35)),
            target: (
                rover + Vec3::new(0.0, 0.2, 0.0),
                rover + Vec3::new(0.0, 0.5, 0.0),
            ),
            focal: 40.0,
            fstop: 2.8,
            expression: false,
        },
        // 2. Her face, three-quarters from the front, lit by the fire in
        // her hand; her eyes go up to the meteors.
        Shot {
            from: 3.2,
            to: 5.6,
            eye: (Vec3::new(-0.75, 1.62, -1.25), Vec3::new(-0.68, 1.6, -1.12)),
            target: (
                face + Vec3::new(0.0, 0.05, 0.0),
                face + Vec3::new(0.0, 0.03, 0.0),
            ),
            focal: 65.0,
            fstop: 2.4,
            expression: false,
        },
        // 3. The strike: low, beside the rover, the meteors coming in
        // over her shoulder; slow at the hit; the pieces flying.
        Shot {
            from: 5.6,
            to: 10.0,
            eye: (
                ROVER_AT + Vec3::new(9.5, 1.1, 7.0),
                ROVER_AT + Vec3::new(11.5, 1.5, 9.5),
            ),
            target: (hit + Vec3::Y * 1.2, hit + Vec3::Y * 2.5),
            focal: 40.0,
            fstop: 8.0,
            expression: false,
        },
        // 4. The wreck settling: across the crater, pieces landing and
        // coming to rest, smoke and small fires.
        Shot {
            from: 10.0,
            to: 12.3,
            eye: (
                ROVER_AT + Vec3::new(-5.5, 1.3, 6.0),
                ROVER_AT + Vec3::new(-3.0, 1.1, 7.0),
            ),
            target: (
                hit + Vec3::new(1.0, 0.2, -2.0),
                hit + Vec3::new(1.5, 0.2, -3.0),
            ),
            focal: 30.0,
            fstop: 8.0,
            expression: false,
        },
        // 5. Her reaction: the firelight on her face, a small smile.
        Shot {
            from: 12.3,
            to: LENGTH,
            eye: (Vec3::new(0.62, 1.63, -1.05), Vec3::new(0.55, 1.62, -0.92)),
            target: (face, face),
            focal: 75.0,
            fstop: 2.2,
            expression: true,
        },
    ]
}

/// Her performance at film time `t`: the rig's pose, in the DNA's frame.
fn perform(rig: &Rig, to_dna: Mat4, t: f32) -> Pose {
    let mut pose = rig.rest();
    let rover = to_dna.transform_point3(ROVER_AT + Vec3::Y * 0.5);
    let sky = to_dna.transform_point3(Vec3::new(-5.0, 11.0, -16.0));
    let shoulder = pose.position(rig, "upperarm_r");
    let rest_hand = pose.position(rig, "hand_r");
    let reach = pose.position(rig, "lowerarm_r").distance(shoulder)
        + pose
            .position(rig, "hand_r")
            .distance(pose.position(rig, "lowerarm_r"));
    // Keys for her right hand (DNA frame: +Z ahead of her, -X her right).
    let ahead = (rover - shoulder).normalize();
    // The cast: her forearm comes up in front of her, palm out, then
    // the arm reaches toward the rover and holds.
    let raised = shoulder + Vec3::new(-2.0, 2.0, reach * 0.6);
    let point = shoulder + ahead * reach * 0.95;
    let keys = [
        (0.0, rest_hand),
        (0.9, rest_hand),
        (2.1, raised),
        (3.7, raised),
        (4.7, point),
        (11.6, point),
        (13.4, rest_hand),
    ];
    let mut hand = rest_hand;
    for w in keys.windows(2) {
        if t >= w[0].0 && t <= w[1].0 {
            hand = w[0].1.lerp(w[1].1, smooth(w[0].0, w[1].0, t));
        }
    }
    if t > keys[keys.len() - 1].0 {
        hand = rest_hand;
    }
    // Into the cast: the chest turns a little toward the arm and lifts.
    let cast = smooth(0.9, 2.2, t) * (1.0 - smooth(11.6, 13.4, t));
    pose.rotate(rig, "spine_03", Quat::from_rotation_y(-0.12 * cast));
    pose.rotate(rig, "spine_05", Quat::from_rotation_x(-0.05 * cast));
    // Her right arm, the elbow out and down.
    let shoulder = pose.position(rig, "upperarm_r");
    let pole = shoulder + Vec3::new(-60.0, -40.0, -20.0);
    pose.reach(rig, ["upperarm_r", "lowerarm_r", "hand_r"], hand, pole);
    // The hand opens as she casts: palm toward the rover.
    let open = smooth(1.6, 2.4, t) * (1.0 - smooth(11.8, 13.0, t));
    if open > 0.0 {
        let palm_to = (rover - pose.position(rig, "hand_r")).normalize();
        let before = pose.clone();
        pose.turn_palm(rig, "r", palm_to);
        // Blend the turn in (rotate is exact; blending poses by weight).
        if open < 1.0 {
            for (a, b) in pose.world.iter_mut().zip(&before.world) {
                *a = blend(*b, *a, open);
            }
        }
        pose.curl(rig, "r", -0.18 * open, 0.07 * open);
    }
    // Her left hand settles a little curled.
    pose.curl(rig, "l", 0.12, 0.0);
    // Where she looks: the rover, up to the sky as the meteors come, back
    // to the rover for the strike and after.
    let up = smooth(3.4, 4.0, t) * (1.0 - smooth(5.6, 6.3, t));
    let look = rover.lerp(sky, up);
    pose.aim(rig, "neck_01", look, 0.35, 0.5);
    pose.aim(rig, "head", look, 0.75, 0.7);
    pose.look(rig, look, 0.45);
    // Blinks.
    for b in [1.4, 4.9, 8.6, 12.9] {
        let u = (t - b) / 0.2;
        if (0.0..1.0).contains(&u) {
            pose.blink = if u < 0.35 {
                smooth(0.0, 0.35, u)
            } else {
                1.0 - smooth(0.35, 1.0, u)
            };
        }
    }
    pose
}

/// A matrix between `a` and `b`: translations lerped, rotations slerped.
fn blend(a: Mat4, b: Mat4, w: f32) -> Mat4 {
    let (sa, ra, ta) = a.to_scale_rotation_translation();
    let (sb, rb, tb) = b.to_scale_rotation_translation();
    Mat4::from_scale_rotation_translation(sa.lerp(sb, w), ra.slerp(rb, w), ta.lerp(tb, w))
}

// ----------------------------------------------------------- the content

fn stamp(p: &Path) -> Result<String, String> {
    let m = std::fs::metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let t = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    Ok(format!("{}:{}:{}", p.display(), m.len(), t))
}

/// The DNA's frame (cm, her +Z ahead) into the importer's model space.
fn dna_frame() -> Mat4 {
    verse_content::basis().inverse()
        * Mat4::from_rotation_y(std::f32::consts::PI)
        * Mat4::from_scale(Vec3::splat(0.01))
}

fn rig_inputs(a: &Args) -> Result<(Skeleton, HashMap<String, Vec3>), String> {
    let read =
        |p: PathBuf| std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()));
    let skeleton = Skeleton::parse(&read(a.skeleton.join("joints.txt"))?)?;
    let deltas = parse_deltas(&read(a.pose.clone())?)?;
    Ok((skeleton, deltas))
}

/// Her models (rigged, with her groom's levels), from the cache when her
/// sources have not changed.
fn caster(a: &Args, dir: &Path) -> Result<Pack, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut key = format!("v{CACHE_VERSION} tex{} sleeve{:?} ", a.textures, a.sleeve);
    for p in [&a.model, &a.lower, &a.pose]
        .into_iter()
        .chain(a.expression.as_ref())
    {
        key += &stamp(p)?;
        key.push(' ');
    }
    for f in [
        "joints.txt",
        "body_lod0_mesh.obj",
        "body_lod0_mesh.skin.txt",
    ] {
        key += &stamp(&a.skeleton.join(f))?;
    }
    let key_path = dir.join("key.txt");
    if std::fs::read_to_string(&key_path).ok().as_deref() == Some(key.as_str())
        && let Ok(pack) = pack_cache::read_pack(dir)
    {
        eprintln!("film: the caster from the cache");
        return Ok(pack);
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let started = Instant::now();
    let mut pack = Pack {
        inventory: None,
        version: 1,
        source_revision: "metahuman-meteor-film".into(),
        models: Default::default(),
        textures: vec![],
        placements: vec![],
    };
    let read =
        |p: PathBuf| std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()));
    let body = BodySkin::parse(
        &read(a.skeleton.join("body_lod0_mesh.obj"))?,
        &read(a.skeleton.join("body_lod0_mesh.skin.txt"))?,
    )?;
    let options = characters::ImportOptions {
        max_texture_edge: a.textures,
    };
    let lower = characters::import_with(&mut pack, dir, &a.lower, options)?;
    let mut faces = vec![(CASTER, a.model.clone())];
    if let Some(e) = &a.expression {
        faces.push((CASTER_EXPRESSION, e.clone()));
    }
    for (name, path) in faces {
        let mut her = characters::import_with(&mut pack, dir, &path, options)?;
        her.surfaces.extend(lower.surfaces.clone());
        let (skeleton, deltas) = rig_inputs(a)?;
        let mut rig = Rig::new(&her, skeleton, &deltas, dna_frame())?;
        if let Some(sleeve) = a.sleeve {
            rig.sleeve = sleeve;
        }
        rig.skin(&mut her, &body)?;
        let strands = groom::split_strands(&mut her, &pack, dir, 1.0)?;
        if name == CASTER {
            for (level, strand) in STRANDS.iter().enumerate() {
                let model = if level == 0 {
                    strands.clone()
                } else {
                    groom::thinned(&strands, THIN[level])?
                };
                pack.models.insert((*strand).into(), model);
            }
        }
        pack.models.insert(name.into(), her);
    }
    pack.validate()?;
    pack_cache::write_pack(dir, &pack)?;
    std::fs::write(&key_path, &key).map_err(|e| e.to_string())?;
    eprintln!(
        "film: the caster imported and rigged in {:.1} s",
        started.elapsed().as_secs_f32()
    );
    Ok(pack)
}

/// Writes `rgba` (`w` × `h`) as a PNG texture in `dir` and adds it.
fn texture(
    pack: &mut Pack,
    dir: &Path,
    file: &str,
    w: u32,
    h: u32,
    rgba: &[u8],
) -> Result<usize, String> {
    use sha2::{Digest, Sha256};
    let mut bytes = Vec::new();
    {
        let mut e = png::Encoder::new(&mut bytes, w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(rgba)
            .map_err(|e| e.to_string())?;
    }
    std::fs::write(dir.join(file), &bytes).map_err(|e| e.to_string())?;
    pack.textures.push(Texture {
        file: file.into(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        width: w,
        height: h,
    });
    Ok(pack.textures.len() - 1)
}

fn vertex(p: Vec3, n: Vec3, uv: [f32; 2]) -> Vertex {
    Vertex {
        position: p.to_array(),
        normal: n.to_array(),
        uv,
        joints: [0; 4],
        weights: [1.0, 0.0, 0.0, 0.0],
    }
}

fn surface(texture: usize, tint: [f32; 3], vertices: Vec<Vertex>, indices: Vec<u32>) -> Surface {
    let mut s = flat::surface(texture, tint, Default::default());
    s.unlit = false;
    s.vertices = vertices;
    s.indices = indices;
    s
}

/// The set: the meadow, the sky, trees, stones and flowers, and the
/// rover's bodies; returns the dressing's placements (model, transform).
fn set(
    pack: &mut Pack,
    dir: &Path,
    track: &Track,
    assets: &Path,
) -> Result<Vec<(String, Mat4)>, String> {
    let white = flat::white_texture(pack, dir)?;
    particle_textures(dir, assets)?;
    verse::imported::chamber::add_effect_models(pack, dir)?;
    // More smoke sprites, each a different puff from the sheet, so a
    // column of puffs doesn't repeat one shape.
    // Smoke from burning metal and grass at dusk: dark.
    if let Some(m) = pack.models.get_mut("particle-smoke") {
        for s in &mut m.surfaces {
            s.tint = [0.11, 0.095, 0.085];
        }
    }
    let smoke = pack
        .models
        .get("particle-smoke")
        .cloned()
        .ok_or("no smoke model")?;
    for k in 1..SMOKE_VARIANTS {
        let file = format!("film-smoke-{k}.png");
        let slot = pack.textures.len();
        let bytes = std::fs::read(dir.join(&file)).map_err(|e| e.to_string())?;
        use sha2::Digest;
        pack.textures.push(Texture {
            file,
            sha256: format!("{:x}", sha2::Sha256::digest(&bytes)),
            width: SMOKE_CELL,
            height: SMOKE_CELL,
        });
        let mut m = smoke.clone();
        m.source = format!("verse/particles/film-smoke-{k}");
        for s in &mut m.surfaces {
            s.texture = slot;
        }
        pack.models.insert(format!("film-smoke-{k}"), m);
    }
    // The meadow: a tileable texture of short grass and earth, made here
    // (value noise over a few octaves, wrapping at the edges), one tile
    // every 2.5 m.
    let size = 512usize;
    let lattice = |cells: usize, seed: u32| -> Vec<f32> {
        (0..cells * cells)
            .map(|i| {
                let h = (i as u32 ^ seed.wrapping_mul(0x9E37_79B9)).wrapping_mul(2_654_435_761);
                let h = (h ^ (h >> 15)).wrapping_mul(2_246_822_519);
                (h >> 8) as f32 / (1u32 << 24) as f32
            })
            .collect()
    };
    let octaves: Vec<(usize, Vec<f32>, f32)> = [(4, 0.5), (16, 0.25), (64, 0.15), (128, 0.1)]
        .iter()
        .enumerate()
        .map(|(k, &(cells, weight))| (cells, lattice(cells, k as u32 + 3), weight))
        .collect();
    let sample = |x: f32, y: f32| -> f32 {
        let mut v = 0.0;
        for (cells, values, weight) in &octaves {
            let (fx, fy) = (x * *cells as f32, y * *cells as f32);
            let (ix, iy) = (fx.floor() as usize, fy.floor() as usize);
            let (tx, ty) = (smooth(0.0, 1.0, fx.fract()), smooth(0.0, 1.0, fy.fract()));
            let at = |a: usize, b: usize| values[(b % cells) * cells + a % cells];
            let top = at(ix, iy) + (at(ix + 1, iy) - at(ix, iy)) * tx;
            let bottom = at(ix, iy + 1) + (at(ix + 1, iy + 1) - at(ix, iy + 1)) * tx;
            v += (top + (bottom - top) * ty) * weight;
        }
        v
    };
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f32 / size as f32, y as f32 / size as f32);
            let n = sample(u, v);
            let blades = sample(u * 3.0 + 0.37, v * 3.0 + 0.71);
            let green = Vec3::new(0.10, 0.17, 0.05);
            let dry = Vec3::new(0.22, 0.2, 0.09);
            let earth = Vec3::new(0.09, 0.07, 0.045);
            let c = green
                .lerp(dry, smooth(0.45, 0.75, n))
                .lerp(earth, smooth(0.62, 0.8, blades) * 0.6)
                * (0.8 + 0.4 * blades);
            let enc = |c: f32| (c.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
            rgba.extend([enc(c.x), enc(c.y), enc(c.z), 255]);
        }
    }
    let grass = texture(
        pack,
        dir,
        "film-meadow.png",
        size as u32,
        size as u32,
        &rgba,
    )?;
    // One quad per 2.5 m tile, each mapping the whole texture: the
    // renderer's texture coordinates stay within 0..1.
    let mut verts = Vec::new();
    let mut idx = Vec::new();
    let (half, tile) = (300.0f32, 2.5f32);
    let cells = (2.0 * half / tile) as usize;
    for i in 0..cells {
        for j in 0..cells {
            let (x, z) = (-half + i as f32 * tile, -half + j as f32 * tile);
            let base = verts.len() as u32;
            let uv = |u: f32, v: f32| [u, v];
            verts.push(vertex(Vec3::new(x, 0.0, z), Vec3::Y, uv(0.0, 0.0)));
            verts.push(vertex(Vec3::new(x + tile, 0.0, z), Vec3::Y, uv(1.0, 0.0)));
            verts.push(vertex(
                Vec3::new(x + tile, 0.0, z + tile),
                Vec3::Y,
                uv(1.0, 1.0),
            ));
            verts.push(vertex(Vec3::new(x, 0.0, z + tile), Vec3::Y, uv(0.0, 1.0)));
            idx.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    let mut ground = surface(grass, [1.0; 3], verts, idx);
    ground.material.roughness = 0.95;
    pack.models.insert(
        "film-ground".into(),
        flat::model("film/ground", vec![ground], 0.0),
    );
    // The sky: a dome, dusk overhead to the afterglow at the horizon
    // (strongest behind the rover, toward -Z).
    let (w, h) = (256u32, 128u32);
    let mut sky = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let elevation = 1.0 - y as f32 / (h - 1) as f32; // 0 horizon, 1 zenith
            let azimuth = x as f32 / w as f32 * std::f32::consts::TAU;
            let toward = ((azimuth - std::f32::consts::PI).cos() * 0.5 + 0.5).powf(3.0);
            let zenith = Vec3::new(0.012, 0.02, 0.06);
            let mid = Vec3::new(0.06, 0.075, 0.16);
            let glow = Vec3::new(0.75, 0.32, 0.12);
            let e = elevation.max(0.0);
            let base = mid.lerp(zenith, smooth(0.03, 0.5, e));
            let c = base.lerp(glow, (1.0 - smooth(0.0, 0.09, e)) * (0.1 + 0.9 * toward));
            let enc = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
            sky.extend([enc(c.x), enc(c.y), enc(c.z), 255]);
        }
    }
    let sky = texture(pack, dir, "film-sky.png", w, h, &sky)?;
    let mut verts = Vec::new();
    let mut idx = Vec::new();
    let (rows, cols) = (24u32, 64u32);
    for r in 0..=rows {
        for c in 0..=cols {
            let e = r as f32 / rows as f32 * std::f32::consts::FRAC_PI_2 * 1.1 - 0.1;
            let az = c as f32 / cols as f32 * std::f32::consts::TAU;
            // The texture's middle column (azimuth PI) faces -Z; u runs
            // 0..1 round the dome with no wrap inside a triangle.
            let dir3 = Vec3::new(e.cos() * az.sin(), e.sin(), e.cos() * az.cos());
            let u = c as f32 / cols as f32;
            let v = 1.0 - (e.max(0.0) / std::f32::consts::FRAC_PI_2).min(1.0);
            verts.push(vertex(dir3 * 700.0, -dir3, [u, v]));
        }
    }
    for r in 0..rows {
        for c in 0..cols {
            let a = r * (cols + 1) + c;
            let b = a + cols + 1;
            idx.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    let mut dome = surface(sky, [1.0; 3], verts, idx);
    dome.unlit = true;
    dome.emissive = true;
    pack.models
        .insert("film-sky".into(), flat::model("film/sky", vec![dome], 0.0));
    // The rover's bodies.
    for (k, (positions, normals, indices, color)) in &track.meshes {
        let verts = positions
            .iter()
            .zip(normals)
            .map(|(p, n)| vertex(*p, *n, [0.5, 0.5]))
            .collect();
        let mut s = surface(white, *color, verts, indices.clone());
        s.material.roughness = 0.5;
        s.material.metallic = 0.15;
        pack.models.insert(
            format!("rover-{k}"),
            flat::model("film/rover", vec![s], 1.0),
        );
    }
    // Dressing: trees beyond the rover, stones, grass and flowers.
    let options = characters::ImportOptions {
        max_texture_edge: 1024,
    };
    for name in [
        "oak_old",
        "linden_broad",
        "birch_trio",
        "boulder_cluster",
        "standing_stone",
        "grass_tall",
        "wildflower_clump",
        "fern_clump",
        "shrub_hazel",
    ] {
        let path = assets.join(format!("everglade/foliage/{name}.gltf"));
        let model = characters::import_with(pack, dir, &path, options)?;
        pack.models.insert(format!("dress-{name}"), model);
    }
    let place = |name: &str, x: f32, z: f32, yaw: f32, scale: f32| {
        (
            format!("dress-{name}"),
            Mat4::from_translation(Vec3::new(x, 0.0, z))
                * Mat4::from_rotation_y(yaw)
                * Mat4::from_scale(Vec3::splat(scale))
                * verse_content::basis(),
        )
    };
    let mut out = vec![
        place("oak_old", -26.0, -52.0, 0.4, 1.2),
        place("linden_broad", 44.0, -72.0, 1.9, 1.3),
        place("birch_trio", 30.0, -44.0, 0.2, 1.1),
        place("oak_old", -48.0, -30.0, 2.5, 1.1),
        place("linden_broad", 42.0, -18.0, 0.8, 1.0),
        place("birch_trio", -36.0, 6.0, 1.1, 1.0),
        place("oak_old", 34.0, 18.0, 2.0, 1.15),
        place("boulder_cluster", -30.0, -44.0, 0.3, 2.2),
        place("boulder_cluster", 19.0, -31.0, 2.2, 1.8),
        place("standing_stone", -4.5, -6.5, 0.5, 0.9),
        place("shrub_hazel", 5.0, -3.5, 0.9, 1.0),
        place("shrub_hazel", -16.0, -19.0, 2.4, 1.2),
    ];
    // Grass, ferns and flowers in loose drifts round her and the rover,
    // kept off where she stands and where the rover's pieces land.
    let mut seed = 17u32;
    let mut random = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1u32 << 24) as f32
    };
    let mut n = 0;
    while n < 70 {
        let x = -22.0 + 44.0 * random();
        let z = -34.0 + 40.0 * random();
        let p = Vec3::new(x, 0.0, z);
        if p.length() < 1.6 || (p - ROVER_AT).length() < 6.0 {
            continue;
        }
        let pick = random();
        let name = if pick < 0.55 {
            "grass_tall"
        } else if pick < 0.8 {
            "wildflower_clump"
        } else {
            "fern_clump"
        };
        out.push(place(name, x, z, random() * 6.28, 0.8 + 0.5 * random()));
        n += 1;
    }
    Ok(out)
}

const SMOKE_VARIANTS: usize = 4;
const SMOKE_CELL: u32 = 128;

/// The particle sprites the effect models read: soft round falloffs, and
/// smoke puffs cut from the Blender-rendered smoke sheet
/// (`assets/verse/fx/smoke.png`, 4 × 4 cells, premultiplied), one per
/// smoke model.
fn particle_textures(dir: &Path, assets: &Path) -> Result<(), String> {
    let encode = |name: &str, size: u32, rgba: &[u8]| -> Result<(), String> {
        let mut bytes = Vec::new();
        {
            let mut e = png::Encoder::new(&mut bytes, size, size);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header()
                .map_err(|e| e.to_string())?
                .write_image_data(rgba)
                .map_err(|e| e.to_string())?;
        }
        std::fs::write(dir.join(format!("{name}.png")), bytes).map_err(|e| e.to_string())
    };
    for name in [
        "particle-fire",
        "particle-arcane",
        "particle-web",
        "particle-shadow",
        "particle-spark",
        "particle-ribbon",
        "particle-rune",
    ] {
        let mut px = Vec::new();
        for y in 0..64 {
            for x in 0..64 {
                let r = Vec2::new((x as f32 - 31.5) / 31.5, (y as f32 - 31.5) / 31.5).length();
                px.extend([255, 255, 255, ((1.0 - r).max(0.0).powi(2) * 255.0) as u8]);
            }
        }
        encode(name, 64, &px)?;
    }
    let sheet = std::fs::read(assets.join("fx/smoke.png")).map_err(|e| e.to_string())?;
    let mut reader = png::Decoder::new(std::io::Cursor::new(sheet))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("smoke sheet too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba || info.width != SMOKE_CELL * 4 {
        return Err("the smoke sheet is not 4 x 4 RGBA cells".into());
    }
    let w = info.width as usize;
    let cell = SMOKE_CELL as usize;
    for k in 0..SMOKE_VARIANTS {
        // Cells from the sheet's middle: full-grown puffs.
        let (cx, cy) = ((5 + k * 2) % 4, (5 + k * 2) / 4);
        let mut px = Vec::with_capacity(cell * cell * 4);
        for y in 0..cell {
            for x in 0..cell {
                let i = ((cy * cell + y) * w + cx * cell + x) * 4;
                let a = f32::from(buf[i + 3]) / 255.0;
                // Premultiplied: the puff's own shading is rgb / alpha.
                let shade = if a > 0.01 {
                    ((f32::from(buf[i]) + f32::from(buf[i + 1]) + f32::from(buf[i + 2]))
                        / (3.0 * 255.0 * a))
                        .clamp(0.35, 1.0)
                } else {
                    1.0
                };
                let v = (shade * 255.0) as u8;
                px.extend([v, v, v, buf[i + 3]]);
            }
        }
        let name = if k == 0 {
            "particle-smoke".to_string()
        } else {
            format!("film-smoke-{k}")
        };
        encode(&name, SMOKE_CELL, &px)?;
    }
    Ok(())
}

// ----------------------------------------------------------- one frame

/// A burning fragment's smoke puffs at sim time `s`: one every 0.06 s of
/// its flight, each rising, growing and thinning over 2.4 s.
fn trail(track: &Track, k: usize, born: f32, s: f32, time: f32) -> Vec<Instance> {
    let mut out = Vec::new();
    let life = 2.4;
    let mut t0 = (s - life).max(born);
    t0 = (t0 / 0.06).ceil() * 0.06;
    let mut t = t0;
    while t <= s {
        if let Some(p) = track.position(k, t) {
            let age = s - t;
            let u = age / life;
            out.push(particle(
                smoke_model((t * 16.7) as usize + k),
                p + Vec3::Y * (0.3 + age * 1.1),
                0.35 + 1.6 * u,
                0.75 * (1.0 - u) * smooth(0.0, 0.08, age),
                time,
            ));
        }
        t += 0.06;
    }
    out
}

fn smoke_model(k: usize) -> &'static str {
    [
        "particle-smoke",
        "film-smoke-1",
        "film-smoke-2",
        "film-smoke-3",
    ][k % SMOKE_VARIANTS]
}

fn particle(model: &str, center: Vec3, radius: f32, opacity: f32, time: f32) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform: Mat4::from_translation(center) * Mat4::from_scale(Vec3::splat(radius)),
        animation: 0.into(),
        time,
        animation_epoch: None,
        emission: Vec3::splat(opacity.clamp(0.0, 1.0)),
    }
}

fn instance(model: &str, transform: Mat4) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform,
        animation: 0.into(),
        time: 0.0,
        animation_epoch: None,
        emission: Vec3::ONE,
    }
}

/// A fire burning at `at` from `since` (sim s): flickering flames and a
/// rising column of smoke.
fn fire(
    at: Vec3,
    since: f32,
    s: f32,
    size: f32,
    time: f32,
    out: &mut Vec<Instance>,
    lights: &mut Vec<Light>,
) {
    let age = s - since;
    if age < 0.0 {
        return;
    }
    let grow = smooth(0.0, 0.6, age);
    for n in 0..4 {
        let k = n as f32;
        let flicker = 1.0 + 0.18 * (time * (9.0 + k * 3.1) + k * 1.7).sin();
        let lift = ((time * 1.7 + k * 0.37).fract()) * 1.2 * size;
        out.push(particle(
            "effect-fire",
            at + Vec3::new(
                (k - 1.5) * 0.25 * size,
                0.25 * size + lift,
                ((k * 2.3).sin()) * 0.2 * size,
            ),
            size * 0.55 * flicker * grow * (1.0 - lift / (1.6 * size)),
            0.55,
            time,
        ));
    }
    let life = 4.0;
    let mut t = ((s - life).max(since) / 0.12).ceil() * 0.12;
    while t <= s {
        let a = s - t;
        let u = a / life;
        let drift = Vec3::new(0.6, 0.0, 0.25) * a;
        out.push(particle(
            smoke_model((t * 8.3) as usize + (at.x * 7.0) as usize),
            at + drift + Vec3::Y * (size + a * 1.6),
            size * (0.5 + 2.2 * u),
            0.7 * (1.0 - u) * smooth(0.0, 0.2, a),
            time,
        ));
        t += 0.12;
    }
    lights.push(Light {
        position: at + Vec3::Y * (0.6 * size),
        color: Vec3::new(1.0, 0.45, 0.12),
        intensity: 35.0 * size * grow * (1.0 + 0.15 * (time * 13.0).sin()),
        range: 9.0 * size.max(0.6),
    });
}

struct Frame {
    instances: Vec<Instance>,
    lighting: Lighting,
    palette: Vec<Mat4>,
    expression: bool,
}

#[allow(clippy::too_many_arguments)]
fn frame(
    rig: &Rig,
    track: &Track,
    clock: &Clock,
    shots: &[Shot],
    dressing: &[(String, Mat4)],
    t: f32,
) -> Frame {
    let s = clock.sim(t);
    let world = Mat4::IDENTITY; // she stands at the origin facing -Z
    let to_dna = (world * verse_content::basis() * dna_frame()).inverse();
    let pose = perform(rig, to_dna, t);
    let palette = rig.palette(&pose);
    // The spell gathering in her hand: a small fire that swells as she
    // raises it and is thrown when she points.
    let hand = to_dna.inverse().transform_point3(
        pose.position(rig, "middle_01_r") * 0.6 + pose.position(rig, "hand_r") * 0.4,
    );
    let gather = smooth(1.4, 3.2, t) * (1.0 - smooth(4.5, 4.9, t));
    let mut instances = vec![
        instance("film-sky", Mat4::IDENTITY),
        instance("film-ground", Mat4::IDENTITY),
    ];
    for (model, m) in dressing {
        instances.push(instance(model, *m));
    }
    for (k, p, q) in track.at(s) {
        instances.push(instance(
            &format!("rover-{k}"),
            Mat4::from_rotation_translation(q, p),
        ));
    }
    let mut lights = Vec::new();
    if gather > 0.01 {
        let flicker = 1.0 + 0.12 * (t * 17.0).sin() + 0.08 * (t * 29.0).sin();
        instances.push(particle(
            "effect-fire",
            hand,
            (0.05 + 0.05 * gather) * flicker,
            0.5 * gather,
            t,
        ));
        instances.push(particle(
            "effect-impact",
            hand,
            0.16 * gather,
            0.1 * gather,
            t,
        ));
        lights.push(Light {
            position: hand + Vec3::new(0.0, 0.05, -0.08),
            color: Vec3::new(1.0, 0.5, 0.18),
            intensity: 2.2 * gather * flicker,
            range: 3.5,
        });
    }
    // The meteors, falling and landing.
    for (i, &(lands, at, flight)) in track.meteors.iter().enumerate() {
        let before = lands - s;
        if before > 0.0 && before < FALL_SHOWN {
            let p = at - flight * FALL_SPEED * before + Vec3::Y * 0.6;
            instances.extend(meteor_swarm::meteor(p, flight * FALL_SPEED, t));
            lights.push(meteor_swarm::meteor_light(p));
        } else if before <= 0.0 {
            // The spell's blast is drawn for a game camera tens of metres
            // off; at film distances it burns to white, so it is drawn
            // at half strength here.
            instances.extend(
                meteor_swarm::blast(at, -before, i as f32 * 1.3, t)
                    .into_iter()
                    .map(|mut b| {
                        b.emission *= 0.45;
                        let c = b.transform.w_axis.truncate();
                        b.transform = Mat4::from_translation(at + (c - at) * 0.7)
                            * Mat4::from_scale(Vec3::splat(0.7))
                            * Mat4::from_translation(-c)
                            * b.transform;
                        b
                    }),
            );
            if let Some(mut l) = meteor_swarm::blast_light(at, -before) {
                l.intensity *= 0.2;
                lights.push(l);
            }
            instances.push(meteor_swarm::scorch(at, t));
            // What the blast leaves burning in its crater.
            let size = if i + 1 == track.meteors.len() {
                1.3
            } else {
                0.8
            };
            fire(at, lands + 0.5, s, size, t, &mut instances, &mut lights);
        }
    }
    // The chassis's fragments burn, trailing smoke, and burn on where
    // they land.
    let strike = track.meteors.last().map_or(0.0, |m| m.0);
    for &k in &track.burning {
        if s < strike {
            continue;
        }
        instances.extend(trail(track, k, strike, s, t));
        if let Some(p) = track.position(k, s) {
            fire(p, strike + 0.05, s, 0.45, t, &mut instances, &mut lights);
        }
    }
    // Light: the dusk sky and the afterglow behind the rover, a soft key
    // on her from the glow, a cool fill, and every fire.
    let face = Vec3::new(0.0, 1.6, 0.0);
    let mut key = vec![
        Light {
            position: Vec3::new(-30.0, 60.0, 40.0),
            color: Vec3::new(0.55, 0.62, 0.95),
            intensity: 2600.0,
            range: 260.0,
        },
        // The afterglow: far and high, so it lights the meadow evenly
        // (a point light near the ground's edge lit a band there).
        Light {
            position: Vec3::new(0.0, 600.0, -900.0),
            color: Vec3::new(1.0, 0.55, 0.28),
            intensity: 830_000.0,
            range: 2000.0,
        },
        // Her key: the afterglow's warmth, soft, high in front of her.
        Light {
            position: face + Vec3::new(-1.6, 1.4, -2.6),
            color: Vec3::new(1.0, 0.72, 0.5),
            intensity: 6.0,
            range: 9.0,
        },
        // A cool fill from her right, and a rim from behind.
        Light {
            position: face + Vec3::new(2.2, 0.4, -1.2),
            color: Vec3::new(0.55, 0.65, 0.95),
            intensity: 2.2,
            range: 7.0,
        },
        Light {
            position: face + Vec3::new(0.8, 0.9, 1.8),
            color: Vec3::new(0.75, 0.8, 1.0),
            intensity: 3.0,
            range: 6.0,
        },
    ];
    // The burst of light on her as the strike flares.
    let flare = (1.0 - ((s - strike).abs() / 1.6)).max(0.0) * f32::from(s >= strike - 0.2);
    key.push(Light {
        position: face + Vec3::new(0.0, 1.0, -3.0),
        color: Vec3::new(1.0, 0.5, 0.18),
        intensity: 10.0 * flare + 2.5 * smooth(strike, strike + 1.5, s),
        range: 8.0,
    });
    key.extend(lights);
    key.truncate(verse_engine::lighting::MAX_LIGHTS);
    let lighting = Lighting {
        ambient: Vec3::new(0.05, 0.055, 0.08),
        exposure: 1.0,
        fog: Vec3::new(0.16, 0.13, 0.17),
        density: 0.004,
        time: 0.0,
        shadowed: 4,
        height_fog: Some(verse_engine::lighting::HeightFog {
            density: 0.006,
            base: 0.0,
            falloff: 0.08,
            start: 25.0,
            max_opacity: 0.55,
            sun_strength: 0.0,
            sun_exponent: 1.0,
        }),
        lights: key,
    };
    Frame {
        instances,
        lighting,
        palette,
        expression: shots
            .iter()
            .find(|x| t >= x.from && t < x.to)
            .is_some_and(|x| x.expression),
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

fn write_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut png = png::Encoder::new(file, w, h);
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(rgba)
        .map_err(|e| e.to_string())
}

struct Film {
    renderer: Renderer,
    rig: Rig,
    track: Track,
    clock: Clock,
    shots: Vec<Shot>,
    dressing: Vec<(String, Mat4)>,
    subframes: usize,
    camera: Option<(Vec3, Vec3)>,
    rest: bool,
}

impl Film {
    fn load(a: &Args) -> Result<Self, String> {
        let started = Instant::now();
        let track = Track::simulate(&a.paint, 32.0)?;
        let clock = Clock::new(track.settled);
        let dir = a.cache.clone();
        let mut pack = caster(a, &dir)?;
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse");
        let dressing = set(&mut pack, &dir, &track, &root)?;
        // Her hair's color, and plain surfaces.
        let white = flat::white_texture(&mut pack, &dir)?;
        for name in [CASTER, CASTER_EXPRESSION].into_iter().chain(STRANDS) {
            let Some(model) = pack.models.get_mut(name) else {
                continue;
            };
            for s in &mut model.surfaces {
                if s.material.shading == Shading::Hair as u8 {
                    s.tint = (Vec3::from(s.tint) * a.hair).to_array();
                }
                if s.vertices.is_empty() {
                    continue;
                }
                let (min, max) = bounds(
                    s.vertices
                        .iter()
                        .map(|v| verse_content::basis().transform_point3(Vec3::from(v.position))),
                );
                for &(lo, hi, color) in &a.plain {
                    if (min - lo).abs().max_element() < 0.015
                        && (max - hi).abs().max_element() < 0.015
                    {
                        s.texture = white;
                        s.tint = color.to_array();
                    }
                }
            }
        }
        let model = pack.models.get(CASTER).ok_or("no caster")?;
        let (skeleton, deltas) = rig_inputs(a)?;
        let rig = Rig::new(model, skeleton, &deltas, dna_frame())?;
        let atlas = original::atlas()?;
        let budget = verse_engine::loading::Budget {
            manifest_bytes: 1024 * 1024 * 1024,
            encoded_file_bytes: 256 * 1024 * 1024,
            encoded_total_bytes: 1024 * 1024 * 1024,
            rgba_total_bytes: 1024 * 1024 * 1024,
            decoder_workspace_bytes: 256 * 1024 * 1024,
        };
        let prepared = verse_engine::loading::Prepared::load(pack, &dir, budget)?;
        let mut renderer = Renderer::from_prepared(prepared, a.size[0], a.size[1], &atlas, &[])?;
        renderer.hair_coverage = true;
        let shots = shots(&track);
        eprintln!("film: ready in {:.1} s", started.elapsed().as_secs_f32());
        Ok(Self {
            renderer,
            rig,
            track,
            clock,
            shots,
            dressing,
            subframes: a.subframes.max(1),
            camera: a.camera,
            rest: a.rest,
        })
    }

    /// Frame `t` (film seconds), averaged over its sub-frames: the shutter
    /// open for half the frame, the lens, and sub-pixel jitter.
    fn render(&mut self, t: f32) -> Result<Vec<u8>, String> {
        let [w, h] = self.renderer.dimensions();
        let aspect = w as f32 / h as f32;
        let n = self.subframes;
        let mut acc = vec![0.0f32; (w * h * 3) as usize];
        let ui = UiBatch::default();
        for k in 0..n {
            let tk = t + (k as f32 + 0.5) / n as f32 * 0.5 / FPS;
            let mut f = frame(
                &self.rig,
                &self.track,
                &self.clock,
                &self.shots,
                &self.dressing,
                tk,
            );
            if self.rest {
                f.palette = self.rig.palette(&self.rig.rest());
            }
            let shot = self
                .shots
                .iter()
                .find(|x| tk >= x.from && tk < x.to)
                .unwrap_or(&self.shots[self.shots.len() - 1]);
            let u = smooth(shot.from, shot.to, tk);
            let (eye, target) = self.camera.unwrap_or((
                shot.eye.0.lerp(shot.eye.1, u),
                shot.target.0.lerp(shot.target.1, u),
            ));
            let camera = CameraSample {
                eye,
                target,
                fov_y: fov_y(shot.focal),
                focus: (target - eye).length(),
                aperture: shot.focal / 1000.0 / (2.0 * shot.fstop),
            };
            let (view_proj, eye) =
                lens_view_projection(&camera, aspect, 0.05, 900.0, lens_sample(k, n, 0.0));
            let jitter = Vec2::new(halton(k + 1, 2) - 0.5, halton(k + 1, 3) - 0.5)
                * Vec2::new(2.0 / w as f32, 2.0 / h as f32);
            let view_proj = Mat4::from_translation(jitter.extend(0.0)) * view_proj;
            // Her face's distance picks the groom's levels.
            let distance = eye.distance(Vec3::new(0.0, 1.6, 0.0));
            let her = if f.expression {
                CASTER_EXPRESSION
            } else {
                CASTER
            };
            let her = if self.renderer.pack().models.contains_key(her) {
                her
            } else {
                CASTER
            };
            let mut instances = f.instances;
            let placed = verse_content::basis();
            for (model, weight) in std::iter::once((her, 1.0)).chain(
                groom::strand_levels(distance, BANDS)
                    .into_iter()
                    .enumerate()
                    .map(|(l, w)| (STRANDS[l], w)),
            ) {
                if weight <= 0.01 {
                    continue;
                }
                if model != her {
                    self.renderer.set_hair_fade(model, Some(weight.min(1.0)))?;
                }
                self.renderer
                    .set_pose_override(model, Some(f.palette.clone()))?;
                instances.push(instance(model, placed));
            }
            let mut light = f.lighting;
            light.time = k as f32 * 1.37 + t * 0.7;
            let pixels = self
                .renderer
                .draw(View { view_proj, eye }, &instances, &ui, &light)?;
            for (i, px) in pixels.chunks_exact(4).enumerate() {
                for c in 0..3 {
                    acc[i * 3 + c] += to_linear(px[c]);
                }
            }
        }
        Ok(acc
            .chunks_exact(3)
            .flat_map(|c| {
                [
                    to_srgb(c[0] / n as f32),
                    to_srgb(c[1] / n as f32),
                    to_srgb(c[2] / n as f32),
                    255,
                ]
            })
            .collect())
    }
}

fn main() -> Result<(), String> {
    let a = args()?;
    let mut film = Film::load(&a)?;
    let [w, h] = film.renderer.dimensions();
    if let Some((t, out)) = &a.still {
        let rgba = film.render(*t)?;
        write_png(out, w, h, &rgba)?;
        println!("wrote {}", out.display());
        return Ok(());
    }
    let out = a.out.clone().ok_or("no --out")?;
    let mut encoder = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
        ])
        .arg(format!("{w}x{h}"))
        .args([
            "-r", "30", "-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-preset", "slow",
            "-crf", "14",
        ])
        .arg(&out)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    let frames = (LENGTH * FPS).round() as usize;
    let strike = film.track.meteors.last().map_or(6.8, |m| m.0);
    let stills = [
        (4.4, "1-cast"),
        (strike + 0.6, "2-hit"),
        (13.6, "3-reaction"),
    ];
    let started = Instant::now();
    for i in 0..frames {
        let t = i as f32 / FPS;
        let rgba = film.render(t)?;
        if let Some(dir) = &a.stills {
            for (at, name) in stills {
                if (t - at).abs() < 0.5 / FPS {
                    let stem = out
                        .file_stem()
                        .map_or("film".into(), |s| s.to_string_lossy().to_string());
                    write_png(&dir.join(format!("{stem}-{name}.png")), w, h, &rgba)?;
                }
            }
        }
        encoder
            .stdin
            .as_mut()
            .ok_or("ffmpeg input")?
            .write_all(&rgba)
            .map_err(|e| e.to_string())?;
        if i % 30 == 0 {
            eprintln!(
                "film: frame {i}/{frames} ({:.0} s)",
                started.elapsed().as_secs_f32()
            );
        }
    }
    drop(encoder.stdin.take());
    encoder.wait().map_err(|e| e.to_string())?;
    println!("wrote {}", out.display());
    Ok(())
}
