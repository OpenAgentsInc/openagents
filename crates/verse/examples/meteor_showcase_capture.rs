//! The Meteor Showcase as a film: two kit houses at golden hour, the
//! caster's cast, eight meteors arcing in, the impacts and the collapse,
//! and the smoke settling over the ruins (issue #10926).
//!
//! Usage: meteor_showcase_capture OUT_DIR [--video PATH] [--seconds N]
//! [--every K] [--no-video] [--live] [--no-flash-lights]
//! [--compare-flash-lights] [--impact-frame N] [--settle-light]
//! [--flash-repeats N] [--flash-every N]
//! [--compare-particles] [--no-particle-lighting] [--no-soft-particles]
//! [--particle-frame N] [--particle-repeats N] [--particle-every N]
//! [--smoke-frame N] [--restore-at SECONDS]
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
//! `--compare-flash-lights` measures each simulation snapshot with and
//! without its flash lights, alternates the order, and records the signed
//! render cost difference without pixel readback. It also writes
//! GPU timestamp durations when the adapter supports them, and matching
//! `impact-flash-off.png` and `impact-flash-on.png` frames. `--impact-frame`
//! selects their zero-based frame index and the `impact.png` frame.
//! `--settle-light` finishes the light bake before a live run starts.
//! `--flash-repeats` averages up to 256 interleaved off/on pairs per fixed
//! snapshot; `--flash-every` selects every Nth frame for comparison. Both
//! default to 1. The report keeps each individual render overhead's spread.
//! Particle comparison uses the same repeated pairs, toggling only scene
//! lighting and soft fade. It keeps the authored colors, density, and flash
//! lights fixed and writes matched impact, ground-smoke, and aftermath images.
//! `--particle-frame` measures only that snapshot; otherwise
//! `--particle-every` selects frames. Particle repeats and spacing default to 1.
//!
//! A film waits for the relighting of what the swarm broke
//! (`verse_pbr::pbr::relight`) before its aftermath frame, so the still
//! shows the settled light. `--restore-at` presses `R` at that time, as the
//! player rebuilds the houses, takes the aftermath half a second before it,
//! and writes `restored.png` a second after it with the light settled
//! again, so it can be compared with `establishing.png`. `capture.json` records what the last relight
//! recomputed.
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
const FLASH_POOL_ITERATIONS: usize = 4096;

struct Args {
    out: PathBuf,
    video: Option<PathBuf>,
    seconds: f32,
    every: Option<usize>,
    /// Plays as the owner does: 60 frames a second, one step a frame, the
    /// light baking while it plays, and the player's own cast.
    live: bool,
    settle_light: bool,
    no_flash_lights: bool,
    compare_flash_lights: bool,
    flash_repeats: usize,
    flash_every: usize,
    impact_frame: Option<usize>,
    smoke_frame: Option<usize>,
    no_particle_lighting: bool,
    no_soft_particles: bool,
    compare_particles: bool,
    particle_frame: Option<usize>,
    particle_repeats: usize,
    particle_every: usize,
    restore_at: Option<f32>,
}

/// One frame's costs, ms, and how much it drew.
#[derive(Clone, Copy, Default)]
struct Sample {
    index: usize,
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
    instanced: usize,
    detect: f32,
    solve: f32,
    awake: usize,
    contacts: usize,
    sprites: usize,
    ribbons: usize,
    ribbon_segments: usize,
    draws: u64,
    triangles: u64,
    flash_lights: usize,
    lit_alpha: usize,
    density_max: f32,
    sprite_area: f32,
    lit_alpha_area: f32,
}

impl Sample {
    /// The frame's time with the CPU and the GPU one after the other.
    fn frame(&self) -> f32 {
        self.tick + self.mesh + self.encode + self.gpu
    }
}

/// Averaged off/on renders of one simulation snapshot, without pixel readback.
#[derive(Clone, Default)]
struct RenderPair {
    index: usize,
    off: (f32, f32),
    on: (f32, f32),
    off_gpu: Option<f32>,
    on_gpu: Option<f32>,
    selected: usize,
    individual_render_overhead: Vec<f32>,
}

impl RenderPair {
    fn overhead(&self) -> f32 {
        self.on.0 + self.on.1 - self.off.0 - self.off.1
    }

    fn gpu_overhead(&self) -> Option<f32> {
        Some(self.on_gpu? - self.off_gpu?)
    }

    fn cpu_gpu_overhead(&self) -> Option<f32> {
        Some(self.on.0 - self.off.0 + self.gpu_overhead()?)
    }
}

enum Comparison {
    Flashes([verse::pbr::Lamp; verse::pbr::MAX_FLASH_CANDIDATES]),
    Particles,
}

fn measure_pair(
    renderer: &mut verse::render::Offscreen,
    view: verse::render::View,
    dynamic: &mut verse::mesh::Mesh,
    ui: &verse::ui::UiBatch,
    frame: usize,
    repeats: usize,
    comparison: Comparison,
) -> Result<RenderPair, String> {
    let original = dynamic
        .neon
        .as_ref()
        .map(|n| (n.flash_lamps, n.particle_lighting, n.soft_particles));
    let mut pair = RenderPair {
        index: frame,
        ..Default::default()
    };
    let mut gpu_total = (0.0, 0.0);
    let mut gpu_valid = true;
    for repeat in 0..repeats {
        let mut timings = [(0.0, 0.0); 2];
        let mut gpu = [None; 2];
        for enabled in if (frame + repeat) % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            if let Some(neon) = &mut dynamic.neon {
                match &comparison {
                    Comparison::Flashes(lamps) => {
                        neon.flash_lamps = *lamps;
                        if !enabled {
                            neon.flash_lamps.fill(verse::pbr::Lamp::OFF);
                        }
                    }
                    Comparison::Particles => {
                        neon.particle_lighting = enabled;
                        neon.soft_particles = enabled;
                    }
                }
            }
            let index = usize::from(enabled);
            timings[index] = renderer.measure(view, dynamic, ui)?;
            gpu[index] = renderer.last_gpu_ms();
            if enabled {
                pair.selected = match &comparison {
                    Comparison::Flashes(_) => renderer.selected_flash_lights(),
                    Comparison::Particles => dynamic
                        .sprites
                        .iter()
                        .filter(|s| s.scene_lit && s.additive < 1.0 && s.alpha > 1e-3)
                        .count(),
                };
            }
        }
        pair.off.0 += timings[0].0;
        pair.off.1 += timings[0].1;
        pair.on.0 += timings[1].0;
        pair.on.1 += timings[1].1;
        pair.individual_render_overhead
            .push(timings[1].0 + timings[1].1 - timings[0].0 - timings[0].1);
        if let [Some(off), Some(on)] = gpu {
            gpu_total.0 += off;
            gpu_total.1 += on;
        } else {
            gpu_valid = false;
        }
    }
    let repeats = repeats as f32;
    pair.off.0 /= repeats;
    pair.off.1 /= repeats;
    pair.on.0 /= repeats;
    pair.on.1 /= repeats;
    if gpu_valid {
        pair.off_gpu = Some(gpu_total.0 / repeats);
        pair.on_gpu = Some(gpu_total.1 / repeats);
    }
    if let (Some(neon), Some((lamps, lighting, soft))) = (&mut dynamic.neon, original) {
        neon.flash_lamps = lamps;
        neon.particle_lighting = lighting;
        neon.soft_particles = soft;
    }
    Ok(pair)
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

fn snapshot(sample: Option<&Sample>) -> serde_json::Value {
    sample.map_or(serde_json::Value::Null, |s| {
        serde_json::json!({
            "frame": s.index, "sprites": s.sprites, "lit_alpha": s.lit_alpha,
            "density_max": s.density_max, "sprite_angular_area": s.sprite_area,
            "lit_alpha_angular_area": s.lit_alpha_area,
        })
    })
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
                "physics_detect_ms": col(|s| s.detect),
                "physics_solve_ms": col(|s| s.solve),
                "awake_bodies_max": most(|s| s.awake as u64),
                "contact_points_max": most(|s| s.contacts as u64),
                "town_sync_ms": col(|s| s.sync),
                "town_pose_ms": col(|s| s.pose),
                "town_solids_ms": col(|s| s.solids),
                "chunks_max": most(|s| s.chunks as u64),
                "posed_vertices_max": most(|s| s.posed as u64),
                "instanced_parts_max": most(|s| s.instanced as u64),
                "sprites_max": most(|s| s.sprites as u64),
                "ribbons_max": most(|s| s.ribbons as u64),
                "ribbon_segments_max": most(|s| s.ribbon_segments as u64),
                "draws_max": most(|s| s.draws),
                "triangles_max": most(|s| s.triangles),
                "flash_lights_selected_max": most(|s| s.flash_lights as u64),
                "lit_alpha_max": most(|s| s.lit_alpha as u64),
                "density_max": samples.iter().map(|s| s.density_max).fold(0.0_f32, f32::max),
                "most_sprites_snapshot": snapshot(samples.iter().max_by_key(|s| s.sprites)),
                "most_sprite_area_snapshot": snapshot(samples.iter().max_by(|a,b| a.sprite_area.total_cmp(&b.sprite_area))),
                "most_lit_alpha_area_snapshot": snapshot(samples.iter().max_by(|a,b| a.lit_alpha_area.total_cmp(&b.lit_alpha_area))),
            }),
        );
    }
    serde_json::Value::Object(out)
}

fn flash_report(phases: &[(&str, Vec<RenderPair>)], gpu_supported: bool) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for (name, pairs) in phases {
        let active: Vec<_> = pairs.iter().filter(|pair| pair.selected > 0).collect();
        let col = |f: fn(&RenderPair) -> f32| spread(active.iter().map(|pair| f(pair)).collect());
        let gpu_col = |f: fn(&RenderPair) -> Option<f32>| {
            spread(active.iter().filter_map(|pair| f(pair)).collect())
        };
        out.insert(
            (*name).to_owned(),
            serde_json::json!({
                "pairs": pairs.len(),
                "individual_pairs": pairs.iter().map(|p| p.individual_render_overhead.len()).sum::<usize>(),
                "individual_render_overhead_ms": spread(pairs.iter().flat_map(|p| p.individual_render_overhead.iter().copied()).collect()),
                "gpu_pairs": pairs.iter().filter(|p| p.gpu_overhead().is_some()).count(),
                "invalid_gpu_pairs": if gpu_supported {
                    pairs.iter().filter(|p| p.gpu_overhead().is_none()).count()
                } else { 0 },
                "pairs_with_flash_lights": active.len(),
                "active_individual_render_overhead_ms": spread(active.iter().flat_map(|p| p.individual_render_overhead.iter().copied()).collect()),
                "flash_lights_selected_max": pairs.iter().map(|p| p.selected).max().unwrap_or(0),
                "active_render_overhead_ms": col(RenderPair::overhead),
                "active_encode_overhead_ms": col(|p| p.on.0 - p.off.0),
                "active_completion_wait_overhead_ms": col(|p| p.on.1 - p.off.1),
                "active_off_render_ms": col(|p| p.off.0 + p.off.1),
                "active_on_render_ms": col(|p| p.on.0 + p.on.1),
                "active_gpu_pairs": active.iter().filter(|p| p.gpu_overhead().is_some()).count(),
                "active_invalid_gpu_pairs": if gpu_supported {
                    active.iter().filter(|p| p.gpu_overhead().is_none()).count()
                } else { 0 },
                "active_gpu_overhead_ms": gpu_col(RenderPair::gpu_overhead),
                "active_cpu_gpu_overhead_ms": gpu_col(RenderPair::cpu_gpu_overhead),
                "active_off_gpu_ms": gpu_col(|p| p.off_gpu),
                "active_on_gpu_ms": gpu_col(|p| p.on_gpu),
                "all_render_overhead_ms": spread(pairs.iter().map(RenderPair::overhead).collect()),
            }),
        );
    }
    serde_json::Value::Object(out)
}

fn particle_report(phases: &[(&str, Vec<RenderPair>)], gpu_supported: bool) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for (name, pairs) in phases {
        out.insert((*name).to_owned(), serde_json::json!({
            "snapshots": pairs.len(),
            "valid_gpu_snapshots": pairs.iter().filter(|p| p.gpu_overhead().is_some()).count(),
            "invalid_gpu_snapshots": if gpu_supported { pairs.iter().filter(|p| p.gpu_overhead().is_none()).count() } else { 0 },
            "render_overhead_ms": spread(pairs.iter().map(RenderPair::overhead).collect()),
            "individual_render_overhead_ms": spread(pairs.iter().flat_map(|p| p.individual_render_overhead.iter().copied()).collect()),
            "snapshot_results": pairs.iter().map(|p| serde_json::json!({
                "frame": p.index, "lit_alpha": p.selected,
                "render_overhead_ms": p.overhead(),
                "encode_overhead_ms": p.on.0 - p.off.0,
                "off_render_ms": p.off.0 + p.off.1,
                "on_render_ms": p.on.0 + p.on.1,
                "gpu_overhead_ms": p.gpu_overhead(),
                "cpu_gpu_overhead_ms": p.cpu_gpu_overhead(),
                "individual_render_overhead_ms": spread(p.individual_render_overhead.clone()),
            })).collect::<Vec<_>>(),
        }));
    }
    serde_json::Value::Object(out)
}

/// A full pool's mean tick and projection cost, excluding spawn and aggregation.
fn measure_flash_pool(eye: Vec3) -> f32 {
    let mut pool = verse_core::flash_light::FlashLights::default();
    for index in 0..verse::pbr::MAX_FLASH_CANDIDATES {
        // Ascending brightness makes each candidate shift the selected array.
        pool.spawn(
            eye + Vec3::X,
            [1.0, 0.9, 0.7],
            (index + 1) as f32 * 1000.0,
            20.0,
        );
    }
    let started = std::time::Instant::now();
    for _ in 0..FLASH_POOL_ITERATIONS {
        pool.tick(1.0 / 65536.0);
        std::hint::black_box(pool.lamps(eye));
    }
    started.elapsed().as_secs_f32() * 1000.0 / FLASH_POOL_ITERATIONS as f32
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
        settle_light: false,
        no_flash_lights: false,
        compare_flash_lights: false,
        flash_repeats: 1,
        flash_every: 1,
        impact_frame: None,
        smoke_frame: None,
        no_particle_lighting: false,
        no_soft_particles: false,
        compare_particles: false,
        particle_frame: None,
        particle_repeats: 1,
        particle_every: 1,
        restore_at: None,
    };
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--video" => args.video = Some(PathBuf::from(value()?)),
            "--no-video" => args.video = None,
            "--live" => args.live = true,
            "--settle-light" => args.settle_light = true,
            "--no-flash-lights" => args.no_flash_lights = true,
            "--compare-flash-lights" => args.compare_flash_lights = true,
            "--no-particle-lighting" => args.no_particle_lighting = true,
            "--no-soft-particles" => args.no_soft_particles = true,
            "--compare-particles" => args.compare_particles = true,
            "--particle-repeats" => {
                args.particle_repeats = value()?
                    .parse()
                    .map_err(|_| "--particle-repeats takes a whole number".to_owned())?
            }
            "--particle-every" => {
                args.particle_every = value()?
                    .parse()
                    .map_err(|_| "--particle-every takes a whole number".to_owned())?
            }
            "--particle-frame" => {
                args.particle_frame = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--particle-frame takes a whole number".to_owned())?,
                )
            }
            "--smoke-frame" => {
                args.smoke_frame = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--smoke-frame takes a whole number".to_owned())?,
                )
            }
            "--flash-repeats" => {
                args.flash_repeats = value()?
                    .parse()
                    .map_err(|_| "--flash-repeats takes a whole number".to_owned())?;
            }
            "--flash-every" => {
                args.flash_every = value()?
                    .parse()
                    .map_err(|_| "--flash-every takes a whole number".to_owned())?;
            }
            "--impact-frame" => {
                args.impact_frame = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--impact-frame takes a whole number".to_owned())?,
                );
            }
            "--restore-at" => {
                args.restore_at = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--restore-at takes a number".to_owned())?,
                );
            }
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
    if !(1..=256).contains(&args.flash_repeats) {
        return Err("--flash-repeats must be between 1 and 256".into());
    }
    if args.flash_every == 0 {
        return Err("--flash-every must be positive".into());
    }
    if !(1..=256).contains(&args.particle_repeats) {
        return Err("--particle-repeats must be between 1 and 256".into());
    }
    if args.particle_every == 0 {
        return Err("--particle-every must be positive".into());
    }
    if args.particle_frame.is_some_and(|frame| frame < 3) {
        return Err("--particle-frame must follow the first three warm-up frames".into());
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

fn capture_particle_pair(
    renderer: &mut verse::render::Offscreen,
    view: verse::render::View,
    dynamic: &mut verse::mesh::Mesh,
    ui: &verse::ui::UiBatch,
    out: &Path,
    name: &str,
) -> Result<(), String> {
    let original = dynamic
        .neon
        .as_ref()
        .map(|n| (n.particle_lighting, n.soft_particles));
    for enabled in [false, true] {
        if let Some(neon) = &mut dynamic.neon {
            neon.particle_lighting = enabled;
            neon.soft_particles = enabled;
        }
        let pixels = renderer.render(view, dynamic, ui)?;
        write_png(
            &out.join(format!(
                "{name}-particles-{}.png",
                if enabled { "on" } else { "off" }
            )),
            &pixels,
        )?;
    }
    if let (Some(neon), Some((lighting, soft))) = (&mut dynamic.neon, original) {
        neon.particle_lighting = lighting;
        neon.soft_particles = soft;
    }
    Ok(())
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
    let frames = (args.seconds * fps).round() as usize;
    for (flag, frame) in [
        ("--impact-frame", args.impact_frame),
        ("--smoke-frame", args.smoke_frame),
        ("--particle-frame", args.particle_frame),
    ] {
        if frame.is_some_and(|frame| frame >= frames) {
            return Err(format!("{flag} must be within the captured frame range"));
        }
    }
    let cast_at = if args.live {
        if args.settle_light {
            eprintln!("baking the light");
            runtime.settle_zone_light();
        }
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
    let mut flash_phases: Vec<(&str, Vec<RenderPair>)> = vec![
        ("before", Vec::new()),
        ("swarm", Vec::new()),
        ("after", Vec::new()),
    ];
    let mut particle_phases: Vec<(&str, Vec<RenderPair>)> = vec![
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
                .arg("-r")
                .arg(fps.to_string())
                .args(["-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv420p"])
                .args(["-crf", "16", "-movflags", "+faststart"])
                .arg(path)
                .stdin(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Cannot start ffmpeg: {e}"))?,
        ),
        None => None,
    };
    let dt = 1.0 / (fps * steps as f32);
    let idle = InputState::default();
    let mut first_impact: Option<usize> = None;
    let mut impact_shot = false;
    let mut impact_frame = None;
    let mut smoke_frame = None;
    let mut slowest = 0.0_f64;
    let mut most_sprites = 0;
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let mut cast = false;
    let mut restored = false;
    let mut relit = [None; 2];
    // The aftermath still comes before any restore, half a second ahead.
    let aftermath = args
        .restore_at
        .map_or(frames.saturating_sub(fps as usize), |at| {
            (((at - 0.5) * fps).round().max(0.0) as usize).min(frames.saturating_sub(fps as usize))
        });
    for k in 0..frames {
        let t = k as f32 / fps;
        let mut sample = Sample {
            index: k,
            ..Default::default()
        };
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
        if !restored && args.restore_at.is_some_and(|at| t >= at) {
            // Key R: the houses stand whole again.
            restored = true;
            runtime.zone_intent(zones::Intent::Rebuild)?;
        }
        // The film's stills show the light settled after what broke.
        let restored_still = restored
            && args
                .restore_at
                .is_some_and(|at| k == ((at + 1.0) * fps).round() as usize);
        if !args.live && (k == aftermath || restored_still) {
            runtime.settle_zone_light();
            relit[usize::from(restored_still)] = runtime.everglade_relit();
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
                    sample.instanced = town.instances;
                    sample.detect += town.steps.detect_ms;
                    sample.solve += town.steps.solve_ms;
                    sample.awake = sample.awake.max(town.steps.awake);
                    sample.contacts = sample.contacts.max(town.steps.contacts);
                }
            }
        }
        runtime.set_shot(Some(camera(t)));
        let started = std::time::Instant::now();
        let mut dynamic = runtime.dynamic_mesh();
        sample.mesh = started.elapsed().as_secs_f32() * 1000.0;
        let flash_lamps = dynamic.neon.as_ref().map(|neon| neon.flash_lamps);
        if args.no_flash_lights
            && let Some(neon) = &mut dynamic.neon
        {
            neon.flash_lamps.fill(verse::pbr::Lamp::OFF);
        }
        if let Some(neon) = &mut dynamic.neon {
            neon.particle_lighting = !args.no_particle_lighting;
            neon.soft_particles = !args.no_soft_particles;
        }
        let sprites = dynamic.sprites.len();
        sample.sprites = sprites;
        let view = runtime.view(aspect);
        for sprite in &dynamic.sprites {
            if sprite.alpha <= 1e-3 {
                continue;
            }
            let area = sprite.half * (sprite.half + sprite.tail.length() * 0.5)
                / view.eye.distance_squared(sprite.at).max(0.01);
            sample.sprite_area += area;
            if sprite.scene_lit && sprite.additive < 1.0 {
                sample.lit_alpha += 1;
                sample.lit_alpha_area += area;
                sample.density_max = sample.density_max.max(sprite.density);
            }
        }
        sample.ribbons = dynamic.ribbons.len();
        sample.ribbon_segments = dynamic
            .ribbons
            .iter()
            .map(|r| r.points.len().saturating_sub(1))
            .sum();
        most_sprites = most_sprites.max(sprites);
        let pixels = renderer.render(runtime.view(aspect), &dynamic, &ui)?;
        (sample.encode, sample.gpu) = renderer.last_timing();
        sample.flash_lights = renderer.selected_flash_lights();
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
        // Keep the simulation, camera, and geometry fixed for both renders.
        // Exclude startup warm-up and alternate order to reduce order bias.
        if args.compare_flash_lights && k >= 3 && k % args.flash_every == 0 {
            let lamps =
                flash_lamps.unwrap_or([verse::pbr::Lamp::OFF; verse::pbr::MAX_FLASH_CANDIDATES]);
            flash_phases[phase].1.push(measure_pair(
                &mut renderer,
                view,
                &mut dynamic,
                &ui,
                k,
                args.flash_repeats,
                Comparison::Flashes(lamps),
            )?);
        }
        if args.compare_particles
            && k >= 3
            && args
                .particle_frame
                .map_or(k % args.particle_every == 0, |frame| k == frame)
        {
            particle_phases[phase].1.push(measure_pair(
                &mut renderer,
                view,
                &mut dynamic,
                &ui,
                k,
                args.particle_repeats,
                Comparison::Particles,
            )?);
        }
        let wreck = runtime.everglade_wreckage().unwrap_or_default();
        let landed = runtime.zone_snapshot(1.0).caption;
        if first_impact.is_none() && wreck[1] > 0 && wreck[2] > 0 {
            first_impact = Some(k);
        }
        if k == (2.0 * fps) as usize {
            write_png(&args.out.join("establishing.png"), &pixels)?;
        }
        let capture_impact = args.impact_frame.map_or_else(
            || first_impact.is_some_and(|f| k >= f + (1.1 * fps) as usize),
            |frame| k == frame,
        );
        if !impact_shot && capture_impact {
            write_png(&args.out.join("impact.png"), &pixels)?;
            impact_shot = true;
            impact_frame = Some(k);
            if args.compare_flash_lights {
                let (current, other) = if args.no_flash_lights {
                    ("impact-flash-off.png", "impact-flash-on.png")
                } else {
                    ("impact-flash-on.png", "impact-flash-off.png")
                };
                write_png(&args.out.join(current), &pixels)?;
                if let (Some(neon), Some(lamps)) = (&mut dynamic.neon, flash_lamps) {
                    neon.flash_lamps = lamps;
                    if !args.no_flash_lights {
                        neon.flash_lamps.fill(verse::pbr::Lamp::OFF);
                    }
                }
                let other_pixels = renderer.render(runtime.view(aspect), &dynamic, &ui)?;
                write_png(&args.out.join(other), &other_pixels)?;
                if let (Some(neon), Some(lamps)) = (&mut dynamic.neon, flash_lamps) {
                    neon.flash_lamps = lamps;
                    if args.no_flash_lights {
                        neon.flash_lamps.fill(verse::pbr::Lamp::OFF);
                    }
                }
            }
            if args.compare_particles {
                capture_particle_pair(&mut renderer, view, &mut dynamic, &ui, &args.out, "impact")?;
            }
        }
        let capture_smoke = args.smoke_frame.map_or_else(
            || first_impact.is_some_and(|frame| k >= frame + (0.65 * fps) as usize),
            |frame| k == frame,
        );
        if smoke_frame.is_none() && capture_smoke {
            smoke_frame = Some(k);
            write_png(&args.out.join("ground-smoke.png"), &pixels)?;
            if args.compare_particles {
                capture_particle_pair(
                    &mut renderer,
                    view,
                    &mut dynamic,
                    &ui,
                    &args.out,
                    "ground-smoke",
                )?;
            }
        }
        if args.compare_particles && args.particle_frame == Some(k) {
            capture_particle_pair(
                &mut renderer,
                view,
                &mut dynamic,
                &ui,
                &args.out,
                "selected-frame",
            )?;
        }
        if restored_still {
            write_png(&args.out.join("restored.png"), &pixels)?;
        }
        if k == aftermath {
            write_png(&args.out.join("aftermath.png"), &pixels)?;
            if args.compare_particles {
                capture_particle_pair(
                    &mut renderer,
                    view,
                    &mut dynamic,
                    &ui,
                    &args.out,
                    "aftermath",
                )?;
            }
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
    let flash_pool_step_ms = args
        .compare_flash_lights
        .then(|| measure_flash_pool(runtime.view(aspect).eye));
    let relit_report: Vec<serde_json::Value> = ["aftermath", "restored"]
        .iter()
        .zip(relit)
        .map(|(still, relit)| {
            serde_json::json!({
                "still": still,
                "relight": relit.map(|r: verse::pbr::relight::RelightStats| serde_json::json!({
                    "hidden_triangles": r.hidden, "vertices": r.vertices, "probes": r.probes,
                    "worker_ms": r.ms,
                })),
            })
        })
        .collect();
    let summary = serde_json::json!({
        "mode": if args.live { "live" } else { "film" },
        "width": WIDTH,
        "height": HEIGHT,
        "fps": fps,
        "steps_per_frame": steps,
        "quality": std::env::var("VERSE_QUALITY").unwrap_or_default(),
        "effective_quality": renderer.quality().tier.name(),
        "particle_budget": verse::fx::budget(renderer.quality().tier),
        "frame_budget_ms": 1000.0/fps,
        "impact_layers": ["flash", "stretched_sparks_one_bounce", "cooling_fireball", "ground_dust_ring", "projected_scorch", "smolder", "distance_shake"],
        "trail": "Connected camera-facing ribbon widening and cooling behind the rock, with retained smoke and embers.",
        "adapter": {
            "name": renderer.adapter_info().name,
            "vendor": renderer.adapter_info().vendor,
            "device": renderer.adapter_info().device,
            "backend": format!("{:?}", renderer.adapter_info().backend),
            "device_type": format!("{:?}", renderer.adapter_info().device_type),
        },
        "light_settled_before_capture": !args.live || args.settle_light,
        "gpu_timestamp_features_supported": renderer.gpu_timestamps_available(),
        "gpu_timestamps_available": flash_phases.iter().chain(&particle_phases).flat_map(|(_, pairs)| pairs).any(|p| p.gpu_overhead().is_some()),
        "flash_lights_enabled": !args.no_flash_lights,
        "impact_frame": impact_frame,
        "relit": relit_report,
        "restore_at": args.restore_at,
        "smoke_frame": smoke_frame,
        "particle_lighting_enabled": !args.no_particle_lighting,
        "soft_particles_enabled": !args.no_soft_particles,
        "sprite_area_method": "Sum of half-width times (half-width plus half-tail) over squared camera distance, before GPU clipping; an angular overdraw proxy, not pixel coverage.",
        "particle_comparison": if args.compare_particles { serde_json::json!({
            "repeats_per_snapshot": args.particle_repeats,
            "every_frames": args.particle_every,
            "selected_frame": args.particle_frame,
            "method": "Repeated interleaved renders of a fixed simulation snapshot, alternating order. Only scene particle lighting and soft fade change; authored colors, density, flash lights, geometry, and camera remain fixed. Reports mean completion increments with individual tail noise; excludes simulation and startup costs.",
            "timing_limit": "Wall-clock completion includes submission and polling overhead. Invalid or unsupported GPU timestamps remain null.",
            "phases": particle_report(&particle_phases, renderer.gpu_timestamps_available()),
        }) } else { serde_json::Value::Null },
        "flash_light_comparison": if args.compare_flash_lights {
            serde_json::json!({
                "repeats_per_snapshot": args.flash_repeats,
                "every_frames": args.flash_every,
                "flash_pool_step_ms": flash_pool_step_ms,
                "flash_pool_iterations": FLASH_POOL_ITERATIONS,
                "flash_pool_candidates": verse::pbr::MAX_FLASH_CANDIDATES,
                "flash_pool_method": "Mean tick plus camera-ranked projection for a full pool, with ascending input brightness to shift every candidate. Excludes impact spawn and aggregation across sources; no lights expire during the sample.",
                "method": "Each selected simulation snapshot averages repeated interleaved off/on render pairs, alternating order after three warm-up frames. Encode, submit, and completion wait omit pixel readback; individual render overhead spreads retain tail noise. Excludes simulation and first-frame warm-up costs. GPU averages require every repeat to have valid off/on timestamps.",
                "timing_limit": "Wall-clock render completion includes submission and polling overhead; it is not a GPU timestamp measurement.",
                "gpu_method": "Optional encoder timestamps around scene commands, excluding buffer uploads, query resolve, and readback. Includes one 8-byte marker copy in each variant. Timestamp command ordering depends on the backend and driver; nonpositive durations are invalid.",
                "phases": flash_report(&flash_phases, renderer.gpu_timestamps_available()),
            })
        } else {
            serde_json::Value::Null
        },
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
