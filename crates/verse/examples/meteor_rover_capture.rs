//! The CAD Lab rover in the Meteor Showcase, as a film: the rover built by
//! talking in CAD Lab (#11627) stands on the lot between the two kit houses
//! at golden hour, the caster calls down the eight-meteor swarm, and the
//! meteor that lands on the rover shatters its deck and its blast tears the
//! dish, mast, wings, bay and wheels off at their joints and throws them
//! across the lot, with sparks and dust. Simulated, not animated
//! (`verse::zones::meteor_rover`: Kinney–Graham blast, Friedlander pulse,
//! bolted joints).
//!
//! Usage: meteor_rover_capture OUT_DIR [--video PATH] [--seconds N]
//!
//! Renders 1920 by 1080 at 30 frames a second through one offscreen
//! renderer: an establishing shot over the lot, a low shot beside the rover
//! as the meteors fall, a side shot tracking it as it is thrown, and a high
//! shot of the aftermath. Then it plays the whole scene again from the
//! start (it is deterministic), stops just before the hardest hit, and
//! films it a quarter as fast from close by: the slow-motion replay. It
//! writes the film to `PATH` (`OUT_DIR/verse-meteor-rover.mp4` by default)
//! through `ffmpeg`, the stills `verse-meteor-rover-{establishing,impact,
//! tumble,aftermath,replay}.png` into `OUT_DIR`, and `capture.json` with
//! the meteor's numbers, what it struck, every joint's capacity and peak
//! load, each part's blast impulse and where it ended up.
//!
//! Run it with `VERSE_QUALITY=high`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use glam::Vec3;
use verse::{
    controller::InputState,
    runtime::WorldRuntime,
    zones::{self, everglade_pack, meteor_showcase as showcase},
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const FPS: f32 = 30.0;
/// Simulation steps per frame at full speed.
const STEPS: usize = 2;
/// How much slower the replay plays.
const SLOW: usize = 4;
/// The replay starts this long before the hardest hit, and lasts this long
/// of the scene's time, s.
const LEAD: f32 = 0.2;
const REPLAY: f32 = 1.5;

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

/// Where the rover's parts are and how far they spread: the centre of
/// the parts within 25 m of where it stood (mass not weighed: a fragment
/// counts like a wheel), and the distance that holds four in five of them
/// (at least 1 m, at most 7 m). What the shots after the hit frame.
fn scatter(runtime: &WorldRuntime, stood: Vec3) -> (Vec3, f32) {
    let Some(rover) = runtime.meteor_rover() else {
        return (stood, 1.0);
    };
    let near: Vec<Vec3> = rover
        .poses()
        .iter()
        .map(|p| p.1.as_vec3())
        .filter(|p| p.distance(stood) < 25.0)
        .collect();
    if near.is_empty() {
        return (stood, 1.0);
    }
    let middle = near.iter().copied().sum::<Vec3>() / near.len() as f32;
    let mut d: Vec<f32> = near.iter().map(|p| p.distance(middle)).collect();
    d.sort_by(f32::total_cmp);
    let reach = d[(d.len() * 4 / 5).min(d.len() - 1)];
    (middle, reach.clamp(1.0, 7.0))
}

/// The direction the meteor that lands on the rover flies (away from the
/// caster, level), and the level direction across it.
fn throw_axes() -> (Vec3, Vec3) {
    let [x, z] = showcase::rover_at();
    let [cx, cz] = showcase::CASTER;
    let away = Vec3::new(x - cx, 0.0, z - cz).normalize_or(Vec3::X);
    (away, Vec3::new(-away.z, 0.0, away.x))
}

/// Where the rover's pieces are thrown (level), and where the cameras
/// after the hit look from: across the throw, from the open lawn, so the
/// fireball sits at one side of the frame and the pieces
/// fly across it. The scene is deterministic: in it the dish, the bay and
/// the right wing land about 18 m out at 0.27 along the meteor's flight and
/// 0.96 across it (`ROVER_TRACE=1` prints where every part is each second).
fn throw_view() -> (Vec3, Vec3) {
    let (away, across) = throw_axes();
    let throw = (away * 0.27 - across * 0.96).normalize();
    let side = Vec3::new(-throw.z, 0.0, throw.x);
    // The open lawn's side, away from the houses and their rubble.
    let (_, out) = lot_axes();
    let side = if side.dot(out) < 0.0 { -side } else { side };
    (throw, side)
}

/// Where the mast is (it comes off with the camera head and lands on the
/// open lawn), or where the rover stood.
fn dish(runtime: &WorldRuntime, stood: Vec3) -> Vec3 {
    runtime
        .meteor_rover()
        .and_then(|r| {
            r.poses()
                .into_iter()
                .find(|(name, _, _)| name == "mast")
                .map(|p| p.1.as_vec3())
        })
        .unwrap_or(stood)
}

/// The director's camera at film time `t`, given where the rover is, where
/// it stood, and where its parts lie: close on it with the houses behind,
/// low beside it as the meteors fall, above it (over the dust) as it is
/// thrown, looking down on the scattered parts, and a slow orbit round them
/// to end.
fn camera(t: f32, dish: Vec3, stood: Vec3, parts: (Vec3, f32)) -> (Vec3, Vec3) {
    let (along, out) = lot_axes();
    let ground = Vec3::new(stood.x, 0.0, stood.z);
    let _ = parts;
    let shots: [(f32, Box<dyn Fn(f32) -> (Vec3, Vec3)>); 6] = [
        (
            0.0,
            Box::new(move |u: f32| {
                // Close on the rover, the houses behind it.
                let eye =
                    ground + out * (6.0 - 1.5 * u) - along * (2.5 - u) + Vec3::Y * (1.6 - 0.3 * u);
                (eye, ground - out * 4.0 + Vec3::Y * (0.9 + 1.4 * u))
            }),
        ),
        (
            3.0,
            Box::new(move |u: f32| {
                let eye = ground + out * (3.2 - 0.4 * u) + along * 2.2 + Vec3::Y * 0.5;
                (eye, ground - out * 1.0 + Vec3::Y * (0.6 + 1.0 * u))
            }),
        ),
        (
            5.6,
            Box::new(move |u: f32| {
                // Across the throw, high, the fireball at the frame's side:
                // the pieces fly out of it across the frame.
                let (throw, side) = throw_view();
                let middle = ground + throw * 8.0;
                let eye = middle + side * (17.0 + 1.5 * u) + Vec3::Y * (5.0 + u);
                (eye, middle + Vec3::Y * 0.5)
            }),
        ),
        (
            7.0,
            Box::new(move |u: f32| {
                // Close on the mast and camera head, thrown off the deck.
                let (throw, side) = throw_view();
                let ground = Vec3::new(dish.x, 0.0, dish.z);
                let eye = ground + side * (5.5 + u) - throw * 1.0 + Vec3::Y * 4.8;
                (eye, ground + Vec3::Y * 0.3)
            }),
        ),
        (
            9.5,
            Box::new(move |u: f32| {
                // Looking down the line of pieces, from where it stood to
                // where the mast came down.
                let (_, side) = throw_view();
                let middle = (ground + Vec3::new(dish.x, 0.0, dish.z)) / 2.0;
                let span = ground.distance(Vec3::new(dish.x, 0.0, dish.z)).max(4.0);
                let eye = middle + side * (0.6 * span + 3.0 - u) + Vec3::Y * (0.45 * span + 2.0);
                (eye, middle)
            }),
        ),
        (
            11.5,
            Box::new(move |u: f32| {
                // A slow turn round the scattered parts.
                let (throw, side) = throw_view();
                let middle = (ground + Vec3::new(dish.x, 0.0, dish.z)) / 2.0;
                let span = ground.distance(Vec3::new(dish.x, 0.0, dish.z)).max(4.0);
                let angle = -0.4 + 0.8 * u;
                let round = side * angle.cos() + throw * angle.sin();
                (
                    middle + round * (0.6 * span + 3.0) + Vec3::Y * (0.35 * span + 1.5),
                    middle + Vec3::Y * 0.2,
                )
            }),
        ),
    ];
    let mut which = 0;
    for (k, (start, _)) in shots.iter().enumerate() {
        if t >= *start {
            which = k;
        }
    }
    let start = shots[which].0;
    let end = shots.get(which + 1).map_or(start + 3.0, |s| s.0);
    (shots[which].1)(ease((t - start) / (end - start)))
}

/// The replay's camera, circling slowly round the hit.
fn replay_camera(u: f32, at: Vec3) -> (Vec3, Vec3) {
    // Across the throw, the fireball at the frame's side.
    let (throw, side) = throw_view();
    let middle = Vec3::new(at.x, 0.0, at.z) + throw * (5.0 + 2.0 * u);
    let eye = middle + side * (8.5 + u) + Vec3::Y * 3.0;
    (eye, middle + Vec3::Y * 0.6)
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

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let mut video = out.join("verse-meteor-rover.mp4");
    let mut seconds = 14.5_f32;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--video" => video = PathBuf::from(args.next().ok_or("--video needs a path")?),
            "--seconds" => {
                seconds = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--seconds needs a number")?;
            }
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
    let mut runtime = scene(&pack)?;
    let stood = rover(&runtime);
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let (eye, target) = camera(0.0, stood, stood, (stood, 1.0));
    runtime.set_shot(Some((eye, target)));
    let mut renderer = verse::render::Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        &atlas,
        runtime.atmosphere(),
    )?;
    let main_film = out.join("main.mp4");
    let mut film = encoder(&main_film)?;
    let idle = InputState::default();
    let dt = 1.0 / (FPS * STEPS as f32);
    let frames = (seconds * FPS).round() as usize;
    let mut impact_still = false;
    let mut tumble_at: Option<usize> = None;
    for k in 0..frames {
        let t = k as f32 / FPS;
        if k > 0 {
            for _ in 0..STEPS {
                runtime.tick(&idle, dt);
            }
        }
        let parts = scatter(&runtime, stood);
        let at = parts.0;
        let (eye, target) = camera(t, dish(&runtime, stood), stood, parts);
        runtime.set_shot(Some((eye, target)));
        let dynamic = runtime.dynamic_mesh();
        let pixels = renderer.render(runtime.view(aspect), &dynamic, &ui)?;
        feed(&mut film, &pixels)?;
        if k == (1.5 * FPS) as usize {
            write_png(&out.join("verse-meteor-rover-establishing.png"), &pixels)?;
        }
        let hit = runtime
            .meteor_rover()
            .and_then(verse::zones::meteor_rover::Rover::hardest);
        if let Some((when, _)) = hit {
            if !impact_still && t as f64 >= when + 0.1 {
                write_png(&out.join("verse-meteor-rover-impact.png"), &pixels)?;
                impact_still = true;
                tumble_at = Some(k + (0.8 * FPS) as usize);
            }
            if tumble_at == Some(k) {
                write_png(&out.join("verse-meteor-rover-tumble.png"), &pixels)?;
            }
        }
        if k + 1 == frames {
            write_png(&out.join("verse-meteor-rover-aftermath.png"), &pixels)?;
        }
        if k % 30 == 0 {
            eprintln!("{t:.1} s, the rover at {at}");
            if std::env::var_os("ROVER_TRACE").is_some()
                && let Some(r) = runtime.meteor_rover()
            {
                let (away, across) = throw_axes();
                for (name, pos, _) in r.poses() {
                    let d = pos.as_vec3() - stood;
                    eprintln!(
                        "   {name:16} along {:6.1} across {:6.1} up {:5.1}",
                        d.dot(away),
                        d.dot(across),
                        d.y
                    );
                }
            }
        }
    }
    finish(film)?;
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
    // The replay: the same scene again, to just before the hardest hit,
    // then a quarter as fast from close by.
    let mut again = scene(&pack)?;
    let start = (hit_at as f32 - LEAD).max(0.0);
    let mut t = 0.0;
    while t + dt <= start {
        again.tick(&idle, dt);
        t += dt;
    }
    let replay_film = out.join("replay.mp4");
    let mut film = encoder(&replay_film)?;
    let mut renderer = verse::render::Offscreen::new(
        WIDTH,
        HEIGHT,
        &again.world.mesh,
        &atlas,
        again.atmosphere(),
    )?;
    let replay_frames = (REPLAY * FPS * SLOW as f32).round() as usize;
    let slow_dt = 1.0 / (FPS * SLOW as f32);
    let held = rover(&again);
    for k in 0..replay_frames {
        if k > 0 {
            again.tick(&idle, slow_dt);
        }
        let u = k as f32 / replay_frames as f32;
        let at = held;
        let (eye, target) = replay_camera(u, at);
        again.set_shot(Some((eye, target)));
        let dynamic = again.dynamic_mesh();
        let pixels = renderer.render(again.view(aspect), &dynamic, &ui)?;
        feed(&mut film, &pixels)?;
        if k == (LEAD * FPS * SLOW as f32) as usize + 8 {
            write_png(&out.join("verse-meteor-rover-replay.png"), &pixels)?;
        }
    }
    finish(film)?;
    // The film: the scene, then its replay.
    let list = out.join("films.txt");
    std::fs::write(
        &list,
        format!(
            "file '{}'\nfile '{}'\n",
            main_film.display(),
            replay_film.display()
        ),
    )
    .map_err(|e| e.to_string())?;
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
        "replay_from_s": start,
        "replay_slower": SLOW,
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
