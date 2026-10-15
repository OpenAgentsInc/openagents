//! The CAD Lab rover in the Meteor Showcase, as a film: the rover built by
//! talking in CAD Lab (#11627) stands on the lot between the two kit houses
//! at golden hour, the caster calls down the eight-meteor swarm, and the
//! meteor that lands on the rover shatters its deck while the blast tears
//! the dish, mast, wings, bay and wheels off at their joints and throws
//! them across the lot. Simulated, not animated
//! (`verse::zones::meteor_rover`: Kinney–Graham blast, Friedlander pulse
//! stepped at 21 µs, bolted joints).
//!
//! Usage: meteor_rover_capture OUT_DIR [--video PATH]
//!
//! Renders 1920 by 1080 at 30 frames a second. The scene is deterministic,
//! so each part of the film plays it again from the start:
//!
//! 1. the swarm coming down, up to the moment the meteor strikes the rover;
//! 2. the 0.35 s round the strike at 1/20 speed, low and close from the
//!    side, with the scene's fire and smoke hidden so every piece can be
//!    seen leaving (a note on screen says so);
//! 3. three of the largest pieces followed in flight at half speed (the
//!    dish and bay, a wing, the mast);
//! 4. an overhead shot of where the pieces came to rest, labelled.
//!
//! It writes the film to `PATH` (`OUT_DIR/verse-meteor-rover.mp4` by
//! default) through `ffmpeg`, the stills `verse-meteor-rover-{establishing,
//! impact,replay,tumble,aftermath}.png` into `OUT_DIR`, and `capture.json`
//! with the meteor's numbers, what it struck, every joint's capacity and
//! peak load, and each part's impulse and where it ended up.
//!
//! Run it with `VERSE_QUALITY=high`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use glam::Vec3;
use verse::{
    controller::InputState,
    render::View,
    runtime::WorldRuntime,
    ui::{Atlas, UiBatch},
    zones::{self, everglade_pack, meteor_showcase as showcase},
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: f32 = 30.0;
/// The scene's step before and outside the slow parts, s.
const DT: f32 = 1.0 / 60.0;
/// The replay: from this long before the strike to this long after, at
/// 1/`REPLAY_SLOW` speed.
const REPLAY_BEFORE: f32 = 0.05;
const REPLAY_AFTER: f32 = 0.3;
const REPLAY_SLOW: f32 = 20.0;
/// Following the pieces: from this long after the strike, for this long,
/// at 1/`FOLLOW_SLOW` speed.
const FOLLOW_FROM: f32 = 0.25;
const FOLLOW_FOR: f32 = 1.6;
const FOLLOW_SLOW: f32 = 2.0;
/// The overhead shot: from this long after the strike, for this long.
const REST_FROM: f32 = 6.0;
const REST_FOR: f32 = 4.0;

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(pixels))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The scene: the showcase installed, the rover on the lot, the cast
/// staged, and the light baked.
fn scene(pack: &everglade_pack::ZonePack) -> Result<WorldRuntime, String> {
    let mut runtime = WorldRuntime::new();
    runtime.install_meteor_showcase(pack);
    if runtime.zone != zones::ZoneId::MeteorShowcase {
        return Err("The Meteor Showcase did not install from the pinned pack".into());
    }
    runtime.set_spawn(Vec3::new(-46.0, 0.0, -80.0), 0.0)?;
    if !runtime.add_meteor_rover() || !runtime.stage_meteor_rover(showcase::DELAY) {
        return Err("The rover and the cast could not be staged".into());
    }
    eprintln!("baking the light");
    runtime.settle_zone_light();
    Ok(runtime)
}

/// Where the rover's body is, or where it stood.
fn rover(runtime: &WorldRuntime) -> Vec3 {
    runtime.meteor_rover().map_or_else(
        || {
            let [x, z] = showcase::rover_at();
            Vec3::new(x, 0.5, z)
        },
        verse::zones::meteor_rover::Rover::center,
    )
}

/// The lot's directions: along the houses' front line, and out in front
/// of it (toward the rover and the camera's side).
fn lot_axes() -> (Vec3, Vec3) {
    let (sin, cos) = showcase::FACING.sin_cos();
    (Vec3::new(cos, 0.0, -sin), Vec3::new(sin, 0.0, cos))
}

/// Where the largest solid of `instance` is.
fn part(runtime: &WorldRuntime, instance: &str) -> Option<Vec3> {
    runtime.meteor_rover().and_then(|r| {
        r.poses()
            .into_iter()
            .find(|(name, _, _)| name == instance)
            .map(|p| p.1.as_vec3())
    })
}

/// The pieces the film names: (solids, label).
const NAMED: [(&[&str], &str); 6] = [
    (&["antenna"], "dish"),
    (&["avionics_bay"], "avionics bay"),
    (&["solar_panel_l"], "left wing"),
    (&["solar_panel_r"], "right wing"),
    (&["mast", "camera_head"], "mast and camera"),
    (
        &[
            "wheel_lf", "wheel_lm", "wheel_lr", "wheel_rf", "wheel_rm", "wheel_rr",
        ],
        "wheels",
    ),
];

/// Where a named piece is: the middle of its solids.
fn named(runtime: &WorldRuntime, solids: &[&str]) -> Option<Vec3> {
    let found: Vec<Vec3> = solids.iter().filter_map(|s| part(runtime, s)).collect();
    (!found.is_empty()).then(|| found.iter().copied().sum::<Vec3>() / found.len() as f32)
}

/// The scene ticked from the start to scene time `until`, s.
fn run_until(pack: &everglade_pack::ZonePack, until: f32) -> Result<WorldRuntime, String> {
    let mut runtime = scene(pack)?;
    let idle = InputState::default();
    let mut t = 0.0;
    while t + DT <= until + 1e-4 {
        runtime.tick(&idle, DT);
        t += DT;
    }
    Ok(runtime)
}

/// The film's cameras before the strike: close on the rover with the
/// houses behind, then low beside it as the meteors fall.
fn opening(t: f32, stood: Vec3) -> (Vec3, Vec3) {
    let (along, out) = lot_axes();
    let ground = Vec3::new(stood.x, 0.0, stood.z);
    if t < 3.0 {
        let u = ease(t / 3.0);
        let eye = ground + out * (6.0 - 1.5 * u) - along * (2.5 - u) + Vec3::Y * (1.6 - 0.3 * u);
        (eye, ground - out * 4.0 + Vec3::Y * (0.9 + 1.4 * u))
    } else {
        let u = ease((t - 3.0) / 2.5);
        let eye = ground + out * (3.2 - 0.4 * u) + along * 2.2 + Vec3::Y * 0.5;
        (eye, ground - out * 1.0 + Vec3::Y * (0.6 + 1.0 * u))
    }
}

/// The way the pieces are thrown (level, from where it stood toward the
/// dish, the left wing and the mast at rest), and the level direction
/// across it on the open lawn's side: where the cameras after the strike
/// stand.
fn throw_axes(runtime: &WorldRuntime, stood: Vec3) -> (Vec3, Vec3) {
    let ground = Vec3::new(stood.x, 0.0, stood.z);
    let sum: Vec3 = ["antenna", "solar_panel_l", "mast"]
        .iter()
        .filter_map(|s| part(runtime, s))
        .map(|p| (Vec3::new(p.x, 0.0, p.z) - ground).normalize_or_zero())
        .sum();
    let (_, out) = lot_axes();
    let throw = sum.normalize_or(out);
    let side = Vec3::new(-throw.z, 0.0, throw.x);
    let side = if side.dot(out) < 0.0 { -side } else { side };
    (throw, side)
}

/// Where `p` is on screen, px, if in front of the camera.
fn project(view: &View, p: Vec3) -> Option<[f32; 2]> {
    let clip = view.view_proj * p.extend(1.0);
    if clip.w <= 0.01 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some([
        (ndc.x + 1.0) / 2.0 * WIDTH as f32,
        (1.0 - ndc.y) / 2.0 * HEIGHT as f32,
    ])
}

/// A line of text on a dark band, at `x`, `y` (its top left), px.
fn note(ui: &mut UiBatch, atlas: &Atlas, x: f32, y: f32, text: &str) {
    let w = atlas.measure(text);
    let h = atlas.ascent * 1.6;
    ui.rect(
        atlas,
        x - 12.0,
        y - 8.0,
        w + 24.0,
        h + 12.0,
        [0.0, 0.0, 0.0, 0.55],
    );
    ui.text(atlas, x, y, text, [1.0, 1.0, 1.0, 0.95]);
}

/// The film's note in the lower left.
fn caption(ui: &mut UiBatch, atlas: &Atlas, text: &str) {
    note(ui, atlas, 48.0, HEIGHT as f32 - 96.0, text);
}

/// Labels on the named pieces in `view`: a dot on each and its name above,
/// merged with a nearer label within 160 px.
fn labels(ui: &mut UiBatch, atlas: &Atlas, runtime: &WorldRuntime, view: &View) {
    let mut placed: Vec<([f32; 2], String)> = Vec::new();
    for (solids, label) in NAMED {
        let Some(at) = named(runtime, solids).and_then(|p| project(view, p)) else {
            continue;
        };
        if let Some(near) = placed
            .iter_mut()
            .find(|(p, _)| (p[0] - at[0]).hypot(p[1] - at[1]) < 160.0)
        {
            near.1 = format!("{}, {label}", near.1);
            continue;
        }
        placed.push((at, label.to_string()));
    }
    for (at, text) in placed {
        ui.disc(atlas, at[0], at[1], 7.0, [1.0, 0.85, 0.3, 1.0]);
        ui.line(atlas, at, [at[0], at[1] - 40.0], 2.0, [1.0, 0.85, 0.3, 0.9]);
        let w = atlas.measure(&text);
        note(
            ui,
            atlas,
            at[0] - w / 2.0,
            at[1] - 40.0 - atlas.ascent * 1.6 - 6.0,
            &text,
        );
    }
}

/// The scene with its fire, smoke and glow left out.
fn without_fire(mut mesh: verse::mesh::Mesh) -> verse::mesh::Mesh {
    mesh.sprites.clear();
    mesh.ribbons.clear();
    mesh.glow.clear();
    mesh
}

fn encoder(path: &Path) -> Result<std::process::Child, String> {
    Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
        ])
        .arg(format!("{WIDTH}x{HEIGHT}"))
        .arg("-r")
        .arg(FPS.to_string())
        .args(["-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .args(["-crf", "16", "-movflags", "+faststart"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start ffmpeg: {e}"))
}

fn feed(child: &mut std::process::Child, pixels: &[u8]) -> Result<(), String> {
    child
        .stdin
        .as_mut()
        .ok_or("ffmpeg has no input")?
        .write_all(pixels)
        .map_err(|e| e.to_string())
}

fn finish(mut child: std::process::Child) -> Result<(), String> {
    drop(child.stdin.take());
    let status = child.wait().map_err(|e| e.to_string())?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "ffmpeg failed".into())
}

/// One shot of the film: from a fresh run of the scene ticked to `from`,
/// `frames` frames each advancing the scene `dt`, with the camera and
/// overlay for each frame (frame index, scene), and whether the scene's
/// fire shows. Returns the frames' stills wanted by index.
#[allow(clippy::too_many_arguments)]
fn shoot(
    pack: &everglade_pack::ZonePack,
    atlas: &Atlas,
    path: &Path,
    from: f32,
    frames: usize,
    dt: f32,
    fire: bool,
    camera: &dyn Fn(usize, &WorldRuntime) -> (Vec3, Vec3),
    overlay: &dyn Fn(usize, &WorldRuntime, &View, &mut UiBatch),
    stills: &[(usize, PathBuf)],
) -> Result<WorldRuntime, String> {
    let mut runtime = run_until(pack, from)?;
    let (eye, target) = camera(0, &runtime);
    runtime.set_shot(Some((eye, target)));
    let mut renderer = verse::render::Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        atlas,
        runtime.atmosphere(),
    )?;
    let idle = InputState::default();
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let mut film = encoder(path)?;
    for k in 0..frames {
        if k > 0 {
            runtime.tick(&idle, dt);
        }
        let (eye, target) = camera(k, &runtime);
        runtime.set_shot(Some((eye, target)));
        let view = runtime.view(aspect);
        let mut ui = UiBatch::default();
        overlay(k, &runtime, &view, &mut ui);
        let dynamic = runtime.dynamic_mesh();
        let dynamic = if fire { dynamic } else { without_fire(dynamic) };
        let pixels = renderer.render(view, &dynamic, &ui)?;
        feed(&mut film, &pixels)?;
        for (at, still) in stills {
            if *at == k {
                write_png(still, &pixels)?;
            }
        }
    }
    finish(film)?;
    eprintln!("wrote {}", path.display());
    Ok(runtime)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let mut video = out.join("verse-meteor-rover.mp4");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--video" => video = PathBuf::from(args.next().ok_or("--video needs a path")?),
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;
    let atlas = Atlas::new(30.0);
    let idle = InputState::default();

    // A first run without filming: when the meteor strikes the rover, and
    // which way the pieces are thrown.
    eprintln!("finding the strike");
    let mut runtime = scene(&pack)?;
    let stood = rover(&runtime);
    let mut t = 0.0f32;
    let hit = loop {
        runtime.tick(&idle, DT);
        t += DT;
        if let Some(r) = runtime.meteor_rover()
            && !r.struck().is_empty()
        {
            break r.hardest().map_or(t, |h| h.0 as f32);
        }
        if t > 30.0 {
            return Err("Nothing struck the rover".into());
        }
    };
    while t < hit + 3.0 {
        runtime.tick(&idle, DT);
        t += DT;
    }
    let (throw, side) = throw_axes(&runtime, stood);
    eprintln!("struck at {hit:.2} s; pieces thrown toward {throw}");
    let ground = Vec3::new(stood.x, 0.0, stood.z);
    let mut clips = Vec::new();

    // 1. The swarm coming down, to the strike.
    let path = out.join("1-opening.mp4");
    let frames = ((hit - REPLAY_BEFORE) * FPS).floor() as usize;
    shoot(
        &pack,
        &atlas,
        &path,
        0.0,
        frames,
        DT * 2.0,
        true,
        &|k, _| opening(k as f32 / FPS, stood),
        &|_, _, _, _| {},
        &[(45, out.join("verse-meteor-rover-establishing.png"))],
    )?;
    clips.push(path);

    // 2. The strike at 1/20 speed, low and close, fire and smoke hidden.
    let path = out.join("2-replay.mp4");
    let span = REPLAY_BEFORE + REPLAY_AFTER;
    let frames = (span * REPLAY_SLOW * FPS).round() as usize;
    let at = |seconds_after: f32| ((REPLAY_BEFORE + seconds_after) * REPLAY_SLOW * FPS) as usize;
    shoot(
        &pack,
        &atlas,
        &path,
        hit - REPLAY_BEFORE,
        frames,
        1.0 / (REPLAY_SLOW * FPS),
        false,
        &|k, _| {
            let u = k as f32 / frames as f32;
            let eye = ground + side * (2.6 + 0.5 * u) - throw * (0.6 - 1.2 * u) + Vec3::Y * 0.6;
            (eye, ground + throw * (0.4 + 0.8 * u) + Vec3::Y * 0.6)
        },
        &|_, _, _, ui| caption(ui, &atlas, "Replay at 1/20 speed: fire and smoke hidden"),
        &[
            (at(0.02), out.join("verse-meteor-rover-impact.png")),
            (at(0.12), out.join("verse-meteor-rover-replay.png")),
        ],
    )?;
    clips.push(path);

    // 3. Three pieces followed in flight at half speed.
    let followed: [(&str, &str); 3] = [
        ("antenna", "Following the dish and the avionics bay"),
        ("solar_panel_l", "Following the left wing"),
        ("mast", "Following the mast and camera head"),
    ];
    for (n, (solid, title)) in followed.iter().enumerate() {
        let path = out.join(format!("3-follow-{n}.mp4"));
        let frames = (FOLLOW_FOR * FOLLOW_SLOW * FPS).round() as usize;
        let text = format!("{title}: half speed, fire and smoke hidden");
        let still = if n == 0 {
            vec![(frames / 3, out.join("verse-meteor-rover-tumble.png"))]
        } else {
            Vec::new()
        };
        shoot(
            &pack,
            &atlas,
            &path,
            hit + FOLLOW_FROM,
            frames,
            1.0 / (FOLLOW_SLOW * FPS),
            false,
            &|_, runtime| {
                let p = part(runtime, solid).unwrap_or(ground);
                let eye = p + side * 2.2 - throw * 1.0 + Vec3::Y * 0.9;
                (eye, p)
            },
            &|_, runtime, view, ui| {
                caption(ui, &atlas, &text);
                labels(ui, &atlas, runtime, view);
            },
            &still,
        )?;
        clips.push(path);
    }

    // 4. Where the pieces came to rest, from overhead, labelled.
    let path = out.join("4-rest.mp4");
    let frames = (REST_FOR * FPS).round() as usize;
    let rest = run_until(&pack, hit + REST_FROM)?;
    let spots: Vec<Vec3> = NAMED
        .iter()
        .filter_map(|(solids, _)| named(&rest, solids))
        .chain(std::iter::once(ground))
        .collect();
    let middle = spots.iter().copied().sum::<Vec3>() / spots.len() as f32;
    let middle = Vec3::new(middle.x, 0.0, middle.z);
    let reach = spots
        .iter()
        .map(|p| Vec3::new(p.x, 0.0, p.z).distance(middle))
        .fold(3.0, f32::max);
    drop(rest);
    let runtime = shoot(
        &pack,
        &atlas,
        &path,
        hit + REST_FROM,
        frames,
        DT * 2.0,
        false,
        &|k, _| {
            let u = k as f32 / frames as f32;
            let height = reach * 1.35 + 3.0;
            let eye = middle + side * (0.35 * height) - throw * (0.15 * height * (1.0 - u))
                + Vec3::Y * height;
            (eye, middle)
        },
        &|_, runtime, view, ui| {
            caption(ui, &atlas, "Where the pieces came to rest: smoke hidden");
            labels(ui, &atlas, runtime, view);
        },
        &[(frames - 1, out.join("verse-meteor-rover-aftermath.png"))],
    )?;
    clips.push(path);

    // The film, in order.
    let list = out.join("films.txt");
    let lines: String = clips
        .iter()
        .map(|c| format!("file '{}'\n", c.display()))
        .collect();
    std::fs::write(&list, lines).map_err(|e| e.to_string())?;
    let status = Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
        ])
        .arg(&list)
        .args(["-c", "copy", "-movflags", "+faststart"])
        .arg(&video)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("ffmpeg could not join the films".into());
    }

    // The report, from the last run (the scene at rest).
    let (hit_at, knock) = runtime
        .meteor_rover()
        .and_then(verse::zones::meteor_rover::Rover::hardest)
        .ok_or("Nothing struck the rover")?;
    let broken: Vec<String> = runtime
        .meteor_rover()
        .map(verse::zones::meteor_rover::Rover::broken)
        .unwrap_or_default();
    let joints: Vec<serde_json::Value> = runtime
        .meteor_rover()
        .map(|r| {
            r.joints()
                .iter()
                .map(|l| {
                    let capacity = match l.capacity {
                        verse::zones::meteor_rover::Capacity::Pin { force, torque } => serde_json::json!({
                            "pin": {"shear_n": (force * l.scale).round(), "bending_n_m": (torque * l.scale).round()}}),
                        verse::zones::meteor_rover::Capacity::Bolted { bolts, size, shear, tension, bending, twist, .. } => serde_json::json!({
                            "bolts": format!("{bolts} x {size}"),
                            "shear_n": (shear * l.scale).round(), "tension_n": (tension * l.scale).round(),
                            "bending_n_m": (bending * l.scale).round(), "twist_n_m": (twist * l.scale).round()}),
                    };
                    serde_json::json!({
                        "joint": l.name, "kind": format!("{:?}", l.kind),
                        "capacity": capacity,
                        "design_scale": (l.scale * 100.0).round() / 100.0,
                        "peak_n": l.peak_force.round(), "peak_n_m": (l.peak_torque * 10.0).round() / 10.0,
                        "peak_share_of_capacity": (l.peak_use * 1000.0).round() / 1000.0,
                        "broke_s": l.broke.map(|t| (t * 100.0).round() / 100.0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    eprintln!("hardest hit at {hit_at:.2} s ({knock:.1} m/s); broke off: {broken:?}");
    let charge = verse::zones::meteor_rover::blast::charge_for(5.2);
    let energy = charge * verse::zones::meteor_rover::blast::TNT
        / verse::zones::meteor_rover::blast::SURFACE;
    let speed = verse::zones::meteor_rover::IMPACT_SPEED;
    let meteor = serde_json::json!({
        "damage_radius_m": 5.2,
        "overpressure_at_radius_kpa": verse::zones::meteor_rover::blast::DAMAGE_OVERPRESSURE / 1000.0,
        "free_air_tnt_kg": (charge * 1000.0).round() / 1000.0,
        "energy_mj": (energy / 1e4).round() / 100.0,
        "speed_m_s": speed,
        "mass_kg": (2.0 * energy / (speed * speed) * 1000.0).round() / 1000.0,
    });
    let (struck, fragmented, parts) = runtime.meteor_rover().map_or_else(Default::default, |r| {
        let struck: Vec<serde_json::Value> = r
            .struck()
            .iter()
            .map(|(name, p, e)| serde_json::json!({"solid": name, "momentum_n_s": p.round(), "energy_kj": (e / 1000.0).round()}))
            .collect();
        let mut seen = std::collections::BTreeMap::new();
        for (name, pos, _) in r.poses() {
            let d = pos.as_vec3().distance(stood);
            let e = seen.entry(name.clone()).or_insert(0.0f32);
            *e = e.max(d);
        }
        let parts: Vec<serde_json::Value> = seen
            .into_iter()
            .map(|(name, d)| {
                serde_json::json!({
                    "solid": name,
                    "blast_and_strike_impulse_n_s": (r.received(&name).length() * 10.0).round() / 10.0,
                    "farthest_from_where_it_stood_m": (d * 10.0).round() / 10.0,
                })
            })
            .collect();
        (struck, r.fragmented().to_vec(), parts)
    });
    let report = serde_json::json!({
        "meteor": meteor,
        "struck": struck,
        "fragmented": fragmented,
        "parts": parts,
        "hardest_hit_s": hit_at,
        "hardest_knock_m_s": knock,
        "broke_off": broken,
        "joints": joints,
        "replay_slower": REPLAY_SLOW,
        "video": video,
    });
    std::fs::write(
        out.join("capture.json"),
        serde_json::to_string_pretty(&report).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    eprintln!("wrote {}", video.display());
    Ok(())
}
