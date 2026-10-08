//! The Meteor Showcase as a film: two kit houses at golden hour, the
//! caster's cast, eight meteors arcing in, the impacts and the collapse,
//! and the smoke settling over the ruins (issue #10926).
//!
//! Usage: meteor_showcase_capture OUT_DIR [--video PATH] [--seconds N]
//! [--every K] [--no-video] [--live]
//!
//! Installs the showcase from the committed, pinned Everglade pack as
//! `verse --meteor-showcase` does, bakes its light, and renders it offscreen
//! at 1920 by 1080, 30 frames a second, through one renderer that keeps its
//! uploaded world. A director's camera pushes in on the houses, drifts
//! round them as the meteors fall, and eases back for the aftermath; the
//! blasts shake it. It writes `establishing.png`, `impact.png`, and
//! `aftermath.png` into `OUT_DIR`, every `K`th frame into `OUT_DIR/frames`
//! when `--every` is given, and the film through `ffmpeg` (`libx264`,
//! `yuv420p`, CRF 16) to `PATH`, `OUT_DIR/meteor-swarm-v2.mp4` by default.
//!
//! Every run writes `capture.json`: for the frames before the meteors set
//! out, the six seconds of the swarm, and after, the 50th and 99th
//! percentile and the longest of each frame's simulation step, dynamic
//! mesh, encoding, and wait for the GPU, the town's share of the step by
//! system, and the most chunks, posed vertices, sprites, draws, and
//! triangles. `--live` plays as a player does instead, for the frame
//! budget: 60 frames a second at one step each, the light still baking,
//! no staged caster, and the player's own Meteor Swarm at three seconds.
//!
//! Run it with `VERSE_QUALITY=high` for the high tier, and with
//! `VERSE_KIT_PACK` naming the licensed kit pack to draw it in place of its
//! committed proxies.

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
/// Simulation steps per frame.
const STEPS: usize = 2;
/// When the live run's player casts, s.
const LIVE_CAST: f32 = 3.0;
/// How long after the meteors set out the swarm phase lasts, s.
const SWARM: f32 = 6.0;

struct Args {
    out: PathBuf,
    video: Option<PathBuf>,
    seconds: f32,
    every: Option<usize>,
    /// Plays as the owner does: 60 frames a second, one step a frame, the
    /// light baking while it plays, and the player's own cast.
    live: bool,
}

/// One frame's costs, ms, and how much it drew.
#[derive(Clone, Copy, Default)]
struct Sample {
    tick: f32,
    mesh: f32,
    encode: f32,
    gpu: f32,
    swarm: f32,
    physics: f32,
    sync: f32,
    pose: f32,
    solids: f32,
    chunks: usize,
    posed: usize,
    sprites: usize,
    draws: u64,
    triangles: u64,
}

impl Sample {
    /// The frame's time with the CPU and the GPU one after the other.
    fn frame(&self) -> f32 {
        self.tick + self.mesh + self.encode + self.gpu
    }
}

/// The 50th and 99th percentiles and the largest of `values`.
fn spread(mut values: Vec<f32>) -> serde_json::Value {
    if values.is_empty() {
        return serde_json::Value::Null;
    }
    values.sort_by(f32::total_cmp);
    let at = |p: f32| values[((values.len() - 1) as f32 * p).round() as usize];
    serde_json::json!({"p50": at(0.5), "p99": at(0.99), "max": values[values.len() - 1]})
}

/// Each phase's frame costs, as `capture.json` holds them.
fn report(phases: &[(&str, Vec<Sample>)]) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for (name, samples) in phases {
        let col = |f: fn(&Sample) -> f32| spread(samples.iter().map(f).collect());
        let most = |f: fn(&Sample) -> u64| samples.iter().map(f).max().unwrap_or(0);
        out.insert(
            (*name).to_owned(),
            serde_json::json!({
                "frames": samples.len(),
                "frame_ms": col(Sample::frame),
                "tick_ms": col(|s| s.tick),
                "dynamic_mesh_ms": col(|s| s.mesh),
                "encode_ms": col(|s| s.encode),
                "gpu_wait_ms": col(|s| s.gpu),
                "town_swarm_ms": col(|s| s.swarm),
                "town_physics_ms": col(|s| s.physics),
                "town_sync_ms": col(|s| s.sync),
                "town_pose_ms": col(|s| s.pose),
                "town_solids_ms": col(|s| s.solids),
                "chunks_max": most(|s| s.chunks as u64),
                "posed_vertices_max": most(|s| s.posed as u64),
                "sprites_max": most(|s| s.sprites as u64),
                "draws_max": most(|s| s.draws),
                "triangles_max": most(|s| s.triangles),
            }),
        );
    }
    serde_json::Value::Object(out)
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let out = PathBuf::from(it.next().ok_or("Expected an output directory")?);
    let mut args = Args {
        video: Some(out.join("meteor-swarm-v2.mp4")),
        out,
        seconds: 12.5,
        every: None,
        live: false,
    };
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--video" => args.video = Some(PathBuf::from(value()?)),
            "--no-video" => args.video = None,
            "--live" => args.live = true,
            "--seconds" => {
                args.seconds = value()?
                    .parse()
                    .map_err(|_| "--seconds takes a number".to_owned())?;
            }
            "--every" => {
                args.every = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--every takes a whole number".to_owned())?,
                );
            }
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    Ok(args)
}

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

/// Eases `x` from 0 to 1 with zero slope at both ends.
fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The value at `t` of a path through `keys`, each a time and a value,
/// eased between neighbors.
fn track(keys: &[(f32, f32)], t: f32) -> f32 {
    let Some(&(first, start)) = keys.first() else {
        return 0.0;
    };
    if t <= first {
        return start;
    }
    for pair in keys.windows(2) {
        let ((t0, a), (t1, b)) = (pair[0], pair[1]);
        if t <= t1 {
            return a + (b - a) * ease((t - t0) / (t1 - t0));
        }
    }
    keys.last().map_or(start, |&(_, v)| v)
}

/// The director's camera at film time `t`: its eye and the point it looks
/// at. It pushes in from the south-south-west for two seconds, keeps
/// drifting in and round toward the west while the cast gathers and the
/// meteors fall, lifts its gaze to the sky they come from, and eases back
/// and round for the aftermath.
fn camera(t: f32) -> (Vec3, Vec3) {
    let [cx, cz] = showcase::LOT;
    // Round the lot from the south-south-west toward the west, into the
    // low Sun's side, so the light rakes across the houses' fronts.
    let angle = track(&[(0.0, 2.80), (3.0, 2.70), (6.5, 2.58), (12.5, 2.47)], t);
    let reach = track(
        &[
            (0.0, 64.0),
            (2.0, 54.0),
            (3.5, 46.0),
            (6.5, 42.0),
            (12.5, 52.0),
        ],
        t,
    );
    let rise = track(
        &[(0.0, 8.5), (2.0, 7.0), (3.5, 6.0), (6.5, 5.5), (12.5, 10.0)],
        t,
    );
    let gaze = track(
        &[
            (0.0, 7.5),
            (2.2, 10.0),
            (3.8, 12.0),
            (6.0, 8.5),
            (12.5, 4.5),
        ],
        t,
    );
    let eye = Vec3::new(
        cx + angle.sin() * reach,
        rise,
        cz - 4.0 + angle.cos() * reach,
    );
    let target = Vec3::new(cx + 1.0, gaze, cz - 2.0);
    (eye, target)
}

fn main() -> Result<(), String> {
    let args = args()?;
    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    if let Some(every) = args.every
        && every > 0
    {
        std::fs::create_dir_all(args.out.join("frames")).map_err(|e| e.to_string())?;
    }
    if std::env::var("VERSE_QUALITY").as_deref() != Ok("high") {
        eprintln!("meteor_showcase_capture: set VERSE_QUALITY=high for the high tier");
    }
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;
    eprintln!(
        "kit: {}",
        if everglade_pack::kit::installed(&pack) {
            "licensed"
        } else {
            "committed proxies"
        }
    );
    let mut runtime = WorldRuntime::new();
    runtime.install_meteor_showcase(&pack);
    if runtime.zone != zones::ZoneId::MeteorShowcase {
        return Err("The Meteor Showcase did not install from the pinned pack".into());
    }
    // The film stands the player out of shot and stages the caster; a live
    // run casts as the player, from the spawn, with the light still baking.
    let fps = if args.live { 60.0 } else { FPS };
    let steps = if args.live { 1 } else { STEPS };
    let cast_at = if args.live {
        LIVE_CAST
    } else {
        runtime.set_spawn(glam::Vec3::new(-46.0, 0.0, -80.0), 0.0)?;
        if !runtime.stage_meteor_showcase(showcase::DELAY) {
            return Err("The showcase's caster could not be staged".into());
        }
        eprintln!("baking the light");
        runtime.settle_zone_light();
        showcase::DELAY
    };
    let release = cast_at + zones::everglade::demolition::meteor::CAST;
    let mut phases: Vec<(&str, Vec<Sample>)> = vec![
        ("before", Vec::new()),
        ("swarm", Vec::new()),
        ("after", Vec::new()),
    ];
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    runtime.set_shot(Some(camera(0.0)));
    let mut renderer = verse::render::Offscreen::new(
        WIDTH,
        HEIGHT,
        &runtime.world.mesh,
        &atlas,
        runtime.atmosphere(),
    )?;
    let mut encoder = match &args.video {
        Some(path) => Some(
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
                .args([
                    "-r", "30", "-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p",
                ])
                .args(["-crf", "16", "-movflags", "+faststart"])
                .arg(path)
                .stdin(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Cannot start ffmpeg: {e}"))?,
        ),
        None => None,
    };
    let frames = (args.seconds * fps).round() as usize;
    let dt = 1.0 / (fps * steps as f32);
    let idle = InputState::default();
    let mut first_impact: Option<usize> = None;
    let mut impact_shot = false;
    let mut slowest = 0.0_f64;
    let mut most_sprites = 0;
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let mut cast = false;
    for k in 0..frames {
        let t = k as f32 / fps;
        let mut sample = Sample::default();
        runtime.set_shot(Some(camera(t)));
        if args.live && !cast && t >= cast_at {
            // Key 1, the ring on the ground between the houses, a click.
            cast = true;
            runtime.zone_intent(zones::Intent::MeteorSwarm)?;
            let clip = runtime.view(aspect).view_proj * showcase::aim().extend(1.0);
            let (x, y) = (0.5 + 0.5 * clip.x / clip.w, 0.5 - 0.5 * clip.y / clip.w);
            if !(runtime.demolition_aim(aspect, x, y) && runtime.demolition_confirm()) {
                return Err("The player's Meteor Swarm did not start".into());
            }
        }
        if k > 0 {
            for _ in 0..steps {
                let ticked = std::time::Instant::now();
                runtime.tick(&idle, dt);
                sample.tick += ticked.elapsed().as_secs_f32() * 1000.0;
                if let Some(town) = runtime.everglade_town_profile() {
                    sample.swarm += town.swarm_ms;
                    sample.physics += town.physics_ms;
                    sample.sync += town.sync_ms;
                    sample.pose += town.pose_ms;
                    sample.solids += town.solids_ms;
                    sample.chunks = town.chunks;
                    sample.posed = town.posed_vertices;
                }
            }
        }
        runtime.set_shot(Some(camera(t)));
        let started = std::time::Instant::now();
        let dynamic = runtime.dynamic_mesh();
        sample.mesh = started.elapsed().as_secs_f32() * 1000.0;
        let sprites = dynamic.sprites.len();
        sample.sprites = sprites;
        most_sprites = most_sprites.max(sprites);
        let pixels = renderer.render(runtime.view(aspect), &dynamic, &ui)?;
        (sample.encode, sample.gpu) = renderer.last_timing();
        if let Some(stats) = renderer.draw_stats() {
            sample.draws = stats.draws;
            sample.triangles = stats.triangles;
        }
        slowest = slowest.max(started.elapsed().as_secs_f64());
        let phase = if t < release {
            0
        } else if t < release + SWARM {
            1
        } else {
            2
        };
        phases[phase].1.push(sample);
        let wreck = runtime.everglade_wreckage().unwrap_or_default();
        let landed = runtime.zone_snapshot(1.0).caption;
        if first_impact.is_none() && wreck[1] > 0 && wreck[2] > 0 {
            first_impact = Some(k);
        }
        if k == (2.0 * fps) as usize {
            write_png(&args.out.join("establishing.png"), &pixels)?;
        }
        if !impact_shot && first_impact.is_some_and(|f| k >= f + (1.1 * fps) as usize) {
            write_png(&args.out.join("impact.png"), &pixels)?;
            impact_shot = true;
        }
        if k + fps as usize == frames {
            write_png(&args.out.join("aftermath.png"), &pixels)?;
        }
        if let Some(every) = args.every
            && every > 0
            && k % every == 0
        {
            write_png(
                &args.out.join("frames").join(format!("{k:04}.png")),
                &pixels,
            )?;
        }
        if let Some(child) = &mut encoder {
            child
                .stdin
                .as_mut()
                .ok_or("ffmpeg has no input")?
                .write_all(&pixels)
                .map_err(|e| format!("ffmpeg: {e}"))?;
        }
        if k % (fps as usize) == 0 {
            eprintln!(
                "t {t:.1} s · frame {:.1} ms (tick {:.1}, mesh {:.1}, encode {:.1}, gpu {:.1}) · raised {} · pieces {} · chunks {} · sprites {sprites} · {}",
                sample.frame(),
                sample.tick,
                sample.mesh,
                sample.encode,
                sample.gpu,
                wreck[0],
                wreck[1],
                wreck[2],
                landed.lines().next().unwrap_or("")
            );
        }
    }
    if let Some(mut child) = encoder {
        drop(child.stdin.take());
        let status = child.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("ffmpeg failed: {status}"));
        }
    }
    let summary = serde_json::json!({
        "mode": if args.live { "live" } else { "film" },
        "width": WIDTH,
        "height": HEIGHT,
        "fps": fps,
        "steps_per_frame": steps,
        "quality": std::env::var("VERSE_QUALITY").unwrap_or_default(),
        "phases": report(&phases),
    });
    let json = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
    std::fs::write(args.out.join("capture.json"), &json).map_err(|e| e.to_string())?;
    eprintln!("{json}");
    eprintln!(
        "slowest frame to render: {:.0} ms; most sprites in a frame: {most_sprites}",
        slowest * 1e3
    );
    if let Some(path) = &args.video {
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}
