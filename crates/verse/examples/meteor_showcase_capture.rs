//! The Meteor Showcase as a film: two kit houses at golden hour, the
//! caster's cast, eight meteors arcing in, the impacts and the collapse,
//! and the smoke settling over the ruins (issue #10926).
//!
//! Usage: meteor_showcase_capture OUT_DIR [--video PATH] [--seconds N]
//! [--every K] [--no-video] [--readback-every-frame] [--live] [--no-flash-lights]
//! [--compare-flash-lights] [--impact-frame N] [--settle-light]
//! [--flash-repeats N] [--flash-every N]
//! [--compare-particles] [--no-particle-lighting] [--no-soft-particles]
//! [--particle-frame N] [--particle-repeats N] [--particle-every N]
//! [--smoke-frame N]
//! [--no-temporal-aa] [--compare-temporal-aa] [--camera director|pan|orbit]
//! [--static-houses] [--sequence FIRST:LAST]
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
//! Temporal comparison keeps separate on/off renderers, advancing both once
//! per simulation frame with alternating render order. Their histories stay
//! warm, and GPU timestamps exclude pixel readback. Matched images accompany
//! the usual captures and saved frames. Use `--camera pan --static-houses`
//! or `--camera orbit --static-houses` to inspect intact house edges; the
//! default destruction sequence exercises fast debris.
//! Without video, frames that save no artifact render and wait for completion
//! without reading pixels. Every frame advances scene history and exposure and
//! enters the timing report. `--readback-every-frame` restores all-frame pixel
//! readback; temporal comparison always retains it for both renderers.
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
    readback_every_frame: bool,
    seconds: f32,
    every: Option<usize>,
    sequence: Option<[usize; 2]>,
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
    no_temporal_aa: bool,
    compare_temporal_aa: bool,
    camera: CameraPath,
    static_houses: bool,
}

#[derive(Default)]
struct FrameSelection {
    establishing: bool,
    impact: bool,
    smoke: bool,
    aftermath: bool,
    numbered: bool,
    particle: bool,
}

impl FrameSelection {
    fn read_pixels(&self, args: &Args) -> bool {
        args.video.is_some()
            || args.readback_every_frame
            || args.compare_temporal_aa
            || self.establishing
            || self.impact
            || self.smoke
            || self.aftermath
            || self.numbered
            || self.particle
    }
}

impl Args {
    fn new(out: PathBuf) -> Self {
        Self {
            video: Some(out.join("meteor-swarm-v2.mp4")),
            out,
            readback_every_frame: false,
            seconds: 12.5,
            every: None,
            sequence: None,
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
            no_temporal_aa: false,
            compare_temporal_aa: false,
            camera: CameraPath::Director,
            static_houses: false,
        }
    }

    fn select_frame(
        &self,
        k: usize,
        frames: usize,
        fps: f32,
        first_impact: Option<usize>,
        impact_saved: bool,
        smoke_saved: bool,
    ) -> FrameSelection {
        FrameSelection {
            establishing: k == (2.0 * fps) as usize,
            impact: !impact_saved
                && self.impact_frame.map_or_else(
                    || first_impact.is_some_and(|f| k >= f + (1.1 * fps) as usize),
                    |frame| k == frame,
                ),
            smoke: !smoke_saved
                && self.smoke_frame.map_or_else(
                    || first_impact.is_some_and(|f| k >= f + (0.65 * fps) as usize),
                    |frame| k == frame,
                ),
            aftermath: k + fps as usize == frames,
            numbered: self.every.is_some_and(|every| every > 0 && k % every == 0)
                || self
                    .sequence
                    .is_some_and(|[first, last]| (first..=last).contains(&k)),
            particle: self.compare_particles && self.particle_frame == Some(k),
        }
    }
}

#[derive(Clone, Copy)]
enum CameraPath {
    Director,
    Pan,
    Orbit,
}

impl CameraPath {
    fn name(self) -> &'static str {
        match self {
            Self::Director => "director",
            Self::Pan => "pan",
            Self::Orbit => "orbit",
        }
    }
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
    rigid_instances: usize,
    awake_bodies: usize,
    sleeping_bodies: usize,
    merged_chunks: usize,
    contact_points: usize,
    warm_candidates: usize,
    physics_steps: u32,
    detect_ms: f32,
    solve_ms: f32,
    step_ms: f32,
    geometry_updates: usize,
    body_records: usize,
    collider_records: usize,
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
    off_gpu_ticks: Option<[u64; 4]>,
    on_gpu_ticks: Option<[u64; 4]>,
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

/// A paired mean and approximate 95% interval from contiguous batch means.
/// Batches retain nearby-frame noise together instead of assuming independent frames.
fn paired_mean_interval(values: &[f32], block_frames: usize) -> Option<(f64, f64, f64, usize)> {
    let blocks = values.len().div_ceil(block_frames.max(1));
    if blocks < 2 || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let mean = values.iter().map(|&v| f64::from(v)).sum::<f64>() / values.len() as f64;
    let mut weighted_variance = 0.0;
    let mut squared_weights = 0.0;
    for block in 0..blocks {
        // Balance lengths so the final partial second does not become a tiny batch.
        let from = block * values.len() / blocks;
        let to = (block + 1) * values.len() / blocks;
        let weight = (to - from) as f64 / values.len() as f64;
        let batch =
            values[from..to].iter().map(|&v| f64::from(v)).sum::<f64>() / (to - from) as f64;
        weighted_variance += weight * (batch - mean).powi(2);
        squared_weights += weight * weight;
    }
    let standard_error = (weighted_variance / (1.0 - squared_weights) * squared_weights).sqrt();
    let half = student_t_975(blocks - 1) * standard_error;
    Some((mean, mean - half, mean + half, blocks))
}

fn student_t_975(degrees: usize) -> f64 {
    const SMALL: [f64; 30] = [
        12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160,
        2.145, 2.131, 2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056,
        2.052, 2.048, 2.045, 2.042,
    ];
    if degrees <= SMALL.len() {
        return SMALL[degrees.saturating_sub(1)];
    }
    let z: f64 = 1.959963984540054;
    let n = degrees as f64;
    z + (z * z * z + z) / (4.0 * n)
        + (5.0 * z.powi(5) + 16.0 * z.powi(3) + 3.0 * z) / (96.0 * n * n)
        + (3.0 * z.powi(7) + 19.0 * z.powi(5) + 17.0 * z.powi(3) - 15.0 * z) / (384.0 * n * n * n)
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
                "physics_detect_ms": col(|s| s.detect_ms),
                "physics_solve_ms": col(|s| s.solve_ms),
                "physics_step_ms": col(|s| s.step_ms),
                "physics_steps_max": most(|s| s.physics_steps as u64),
                "geometry_updates_max": most(|s| s.geometry_updates as u64),
                "body_records_max": most(|s| s.body_records as u64),
                "collider_records_max": most(|s| s.collider_records as u64),
                "town_sync_ms": col(|s| s.sync),
                "town_pose_ms": col(|s| s.pose),
                "town_solids_ms": col(|s| s.solids),
                "chunks_max": most(|s| s.chunks as u64),
                "posed_vertices_max": most(|s| s.posed as u64),
                "rigid_instances_max": most(|s| s.rigid_instances as u64),
                "awake_bodies_max": most(|s| s.awake_bodies as u64),
                "sleeping_bodies_max": most(|s| s.sleeping_bodies as u64),
                "merged_chunks_max": most(|s| s.merged_chunks as u64),
                "contact_points_max": most(|s| s.contact_points as u64),
                "warm_candidates_max": most(|s| s.warm_candidates as u64),
                "debris_end": samples.last().map(|s| serde_json::json!({
                    "frame": s.index, "rigid_instances": s.rigid_instances,
                    "awake_bodies": s.awake_bodies, "sleeping_bodies": s.sleeping_bodies,
                    "merged_chunks": s.merged_chunks, "contact_points": s.contact_points,
                    "warm_candidates": s.warm_candidates,
                })),
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
    let mut args = Args::new(out);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--video" => args.video = Some(PathBuf::from(value()?)),
            "--no-video" => args.video = None,
            "--readback-every-frame" => args.readback_every_frame = true,
            "--live" => args.live = true,
            "--settle-light" => args.settle_light = true,
            "--no-flash-lights" => args.no_flash_lights = true,
            "--compare-flash-lights" => args.compare_flash_lights = true,
            "--no-particle-lighting" => args.no_particle_lighting = true,
            "--no-soft-particles" => args.no_soft_particles = true,
            "--compare-particles" => args.compare_particles = true,
            "--no-temporal-aa" => args.no_temporal_aa = true,
            "--compare-temporal-aa" => args.compare_temporal_aa = true,
            "--static-houses" => args.static_houses = true,
            "--sequence" => {
                let range = value()?;
                let (first, last) = range.split_once(':').ok_or("--sequence takes FIRST:LAST")?;
                let first = first
                    .parse::<usize>()
                    .map_err(|_| "--sequence takes whole frame numbers")?;
                let last = last
                    .parse::<usize>()
                    .map_err(|_| "--sequence takes whole frame numbers")?;
                if first > last || last - first >= 120 {
                    return Err("--sequence must contain 1 to 120 consecutive frames".into());
                }
                args.sequence = Some([first, last]);
            }
            "--camera" => {
                args.camera = match value()?.as_str() {
                    "director" => CameraPath::Director,
                    "pan" => CameraPath::Pan,
                    "orbit" => CameraPath::Orbit,
                    _ => return Err("--camera takes director, pan, or orbit".into()),
                }
            }
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
    if args.compare_temporal_aa
        && (args.no_temporal_aa || std::env::var("VERSE_TEMPORAL_AA").as_deref() == Ok("off"))
    {
        return Err("Temporal comparison requires temporal antialiasing enabled".into());
    }
    if args.compare_temporal_aa && (args.compare_flash_lights || args.compare_particles) {
        return Err("Run temporal comparison separately so each renderer advances once per simulation frame".into());
    }
    Ok(args)
}

fn write_temporal_pair(
    out: &Path,
    name: &str,
    on: &[u8],
    off: Option<&[u8]>,
) -> Result<(), String> {
    if let Some(off) = off {
        write_png(&out.join(format!("{name}-taa-off.png")), off)?;
        write_png(&out.join(format!("{name}-taa-on.png")), on)?;
    }
    Ok(())
}

fn paired_world(
    world: &verse::mesh::Mesh,
) -> (
    verse::mesh::Mesh,
    Option<verse::pbr::textured::BakedVertices>,
) {
    let mut copy = world.clone();
    let Some(original) = &world.textured else {
        return (copy, None);
    };
    let mut scene = original.as_ref().clone();
    scene.baked = verse::pbr::textured::BakedVertices::default();
    let receiver = scene.baked.clone();
    copy.textured = Some(std::sync::Arc::new(scene));
    (copy, Some(receiver))
}

fn forward_bake(
    source: &verse::pbr::textured::BakedVertices,
    receivers: &[verse::pbr::textured::BakedVertices],
) {
    let lights = source.take();
    let lamps = source.take_lamps();
    for receiver in receivers {
        if let Some(lights) = &lights {
            receiver.deliver_lights(lights.clone());
        }
        if let Some(lamps) = &lamps {
            receiver.deliver_lamps(lamps.clone());
        }
    }
}

fn temporal_report(
    phases: &[(&str, Vec<RenderPair>)],
    gpu_supported: bool,
    fps: f32,
) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for (name, pairs) in phases {
        let costs: Vec<_> = pairs.iter().map(RenderPair::overhead).collect();
        let block_frames = fps.round().max(1.0) as usize;
        let interval = paired_mean_interval(&costs, block_frames);
        let order_mean = |off_first: bool| {
            let values: Vec<_> = pairs
                .iter()
                .filter(|p| (p.index % 2 == 0) == off_first)
                .map(RenderPair::overhead)
                .collect();
            (!values.is_empty()).then(|| values.iter().sum::<f32>() / values.len() as f32)
        };
        out.insert((*name).to_owned(), serde_json::json!({
            "frames": pairs.len(),
            "valid_gpu_frames": pairs.iter().filter(|p| p.gpu_overhead().is_some()).count(),
            "invalid_gpu_frames": if gpu_supported { pairs.iter().filter(|p| p.gpu_overhead().is_none()).count() } else { 0 },
            "gpu_increment_ms": spread(pairs.iter().filter_map(RenderPair::gpu_overhead).collect()),
            "mean_gpu_increment_ms": if !pairs.is_empty() && pairs.iter().all(|p| p.gpu_overhead().is_some()) { Some(pairs.iter().filter_map(RenderPair::gpu_overhead).sum::<f32>() / pairs.len() as f32) } else { None },
            "wall_render_completion_increment_ms": spread(pairs.iter().map(RenderPair::overhead).collect()),
            "mean_wall_render_completion_increment_ms": if pairs.is_empty() { None } else { Some(pairs.iter().map(RenderPair::overhead).sum::<f32>() / pairs.len() as f32) },
            "wall_render_completion_mean_95pct_ci_ms": interval.map(|(mean, lower, upper, blocks)| serde_json::json!({"mean": mean, "lower": lower, "upper": upper, "samples": costs.len(), "blocks": blocks, "nominal_block_frames": block_frames})),
            "wall_mean_upper_95pct_under_1ms": interval.map(|(_, _, upper, _)| upper < 1.0),
            "off_first_frames": pairs.iter().filter(|p| p.index % 2 == 0).count(),
            "on_first_frames": pairs.iter().filter(|p| p.index % 2 != 0).count(),
            "off_first_mean_wall_increment_ms": order_mean(true),
            "on_first_mean_wall_increment_ms": order_mean(false),
            "encode_increment_ms": spread(pairs.iter().map(|p| p.on.0 - p.off.0).collect()),
            "cpu_gpu_increment_ms": spread(pairs.iter().filter_map(RenderPair::cpu_gpu_overhead).collect()),
            "gpu_increment_under_1ms_p99": if pairs.iter().all(|p| p.gpu_overhead().is_some()) && !pairs.is_empty() {
                let mut costs: Vec<_> = pairs.iter().filter_map(RenderPair::gpu_overhead).collect();
                costs.sort_by(f32::total_cmp);
                Some(costs[((costs.len() - 1) as f32 * 0.99).round() as usize] < 1.0)
            } else { None },
            "all_gpu_increments_under_1ms": if !pairs.is_empty() && pairs.iter().all(|p| p.gpu_overhead().is_some()) { Some(pairs.iter().filter_map(RenderPair::gpu_overhead).all(|ms| ms < 1.0)) } else { None },
            "frame_results": pairs.iter().map(|p| serde_json::json!({"frame": p.index, "off_gpu_ms": p.off_gpu, "on_gpu_ms": p.on_gpu, "gpu_increment_ms": p.gpu_overhead(), "off_gpu_ticks": p.off_gpu_ticks, "on_gpu_ticks": p.on_gpu_ticks, "wall_render_completion_increment_ms": p.overhead()})).collect::<Vec<_>>(),
        }));
    }
    serde_json::Value::Object(out)
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

fn shot(path: CameraPath, t: f32) -> (Vec3, Vec3) {
    match path {
        CameraPath::Director => camera(t),
        CameraPath::Pan => {
            let (eye, gaze) = camera(0.0);
            let offset = Vec3::new(t * 2.0, 0.0, 0.0);
            (eye + offset, gaze + offset)
        }
        CameraPath::Orbit => {
            let [cx, cz] = showcase::LOT;
            let angle = 2.8 + t * 0.08;
            (
                Vec3::new(cx + angle.sin() * 58.0, 8.0, cz + angle.cos() * 58.0),
                Vec3::new(cx, 6.0, cz),
            )
        }
    }
}

fn main() -> Result<(), String> {
    let args = args()?;
    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    if args.every.is_some_and(|every| every > 0) || args.sequence.is_some() {
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
    if args.sequence.is_some_and(|[_, last]| last >= frames) {
        return Err("--sequence must be within the captured frame range".into());
    }
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
        if !args.static_houses && !runtime.stage_meteor_showcase(showcase::DELAY) {
            return Err("The showcase's caster could not be staged".into());
        }
        eprintln!("baking the light");
        runtime.settle_zone_light();
        showcase::DELAY
    };
    let release = if args.static_houses {
        args.seconds + 1.0
    } else {
        cast_at + zones::everglade::demolition::meteor::CAST
    };
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
    let mut temporal_phases: Vec<(&str, Vec<RenderPair>)> = vec![
        ("before", Vec::new()),
        ("swarm", Vec::new()),
        ("after", Vec::new()),
    ];
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    runtime.set_shot(Some(shot(args.camera, 0.0)));
    // Independent delivery slots let each GPU receive the same bake. A late
    // background delivery waits until the next paired simulation snapshot.
    let (capture_world, on_bake) = if args.compare_temporal_aa {
        paired_world(&runtime.world.mesh)
    } else {
        (runtime.world.mesh.clone(), None)
    };
    let mut renderer =
        verse::render::Offscreen::new(WIDTH, HEIGHT, &capture_world, &atlas, runtime.atmosphere())?;
    let mut bake_receivers: Vec<_> = on_bake.into_iter().collect();
    let mut temporal_baseline = if args.compare_temporal_aa {
        let (baseline_world, off_bake) = paired_world(&runtime.world.mesh);
        bake_receivers.extend(off_bake);
        Some(verse::render::Offscreen::new(
            WIDTH,
            HEIGHT,
            &baseline_world,
            &atlas,
            runtime.atmosphere(),
        )?)
    } else {
        None
    };
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
    let mut primary_readback_frames = Vec::new();
    let mut completion_only_frames = Vec::new();
    let mut artifact_readback_frames = Vec::new();
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let mut cast = false;
    for k in 0..frames {
        let t = k as f32 / fps;
        let mut sample = Sample {
            index: k,
            ..Default::default()
        };
        runtime.set_shot(Some(shot(args.camera, t)));
        if args.live && !args.static_houses && !cast && t >= cast_at {
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
                    sample.rigid_instances = town.rigid_instances;
                    sample.awake_bodies = town.awake_bodies;
                    sample.sleeping_bodies = town.sleeping_bodies;
                    sample.merged_chunks = town.merged_chunks;
                    sample.contact_points = town.contact_points;
                    sample.warm_candidates = town.warm_candidates;
                    sample.physics_steps += town.physics_timings.steps;
                    sample.detect_ms += town.physics_timings.detect_ms;
                    sample.solve_ms += town.physics_timings.solve_ms;
                    sample.step_ms += town.physics_timings.step_ms;
                    sample.geometry_updates += town.physics_timings.geometry_updates;
                    sample.body_records = town.physics_timings.body_records;
                    sample.collider_records = town.physics_timings.collider_records;
                }
            }
        }
        runtime.set_shot(Some(shot(args.camera, t)));
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
            neon.temporal_aa =
                !args.no_temporal_aa && !args.compare_flash_lights && !args.compare_particles;
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
        let wreck = runtime.everglade_wreckage().unwrap_or_default();
        if first_impact.is_none() && wreck[1] > 0 && wreck[2] > 0 {
            first_impact = Some(k);
        }
        let selection = args.select_frame(
            k,
            frames,
            fps,
            first_impact,
            impact_shot,
            smoke_frame.is_some(),
        );
        let read_pixels = selection.read_pixels(&args);
        if args.compare_temporal_aa
            && let Some(scene) = &runtime.world.mesh.textured
        {
            forward_bake(&scene.baked, &bake_receivers);
        }
        let mut temporal_pair = RenderPair {
            index: k,
            ..Default::default()
        };
        let mut off_pixels = None;
        if let Some(baseline) = &mut temporal_baseline
            && k % 2 == 0
        {
            if let Some(neon) = &mut dynamic.neon {
                neon.temporal_aa = false;
            }
            off_pixels = Some(baseline.render(view, &dynamic, &ui)?);
            temporal_pair.off = baseline.last_timing();
            temporal_pair.off_gpu = baseline.last_gpu_ms();
            temporal_pair.off_gpu_ticks = baseline.last_gpu_ticks();
            if let Some(neon) = &mut dynamic.neon {
                neon.temporal_aa = true;
            }
        }
        let pixels = if read_pixels {
            primary_readback_frames.push(k);
            renderer.render(view, &dynamic, &ui)?
        } else {
            completion_only_frames.push(k);
            renderer.measure(view, &dynamic, &ui)?;
            Vec::new()
        };
        temporal_pair.on = renderer.last_timing();
        temporal_pair.on_gpu = renderer.last_gpu_ms();
        temporal_pair.on_gpu_ticks = renderer.last_gpu_ticks();
        if let Some(baseline) = &mut temporal_baseline
            && k % 2 != 0
        {
            if let Some(neon) = &mut dynamic.neon {
                neon.temporal_aa = false;
            }
            off_pixels = Some(baseline.render(view, &dynamic, &ui)?);
            temporal_pair.off = baseline.last_timing();
            temporal_pair.off_gpu = baseline.last_gpu_ms();
            temporal_pair.off_gpu_ticks = baseline.last_gpu_ticks();
            if let Some(neon) = &mut dynamic.neon {
                neon.temporal_aa = true;
            }
        }
        if args.compare_temporal_aa && !renderer.temporal_aa_available() {
            return Err(
                "Temporal comparison requires a desktop Medium or High HDR renderer".into(),
            );
        }
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
        if args.compare_temporal_aa && k >= 8 {
            temporal_phases[phase].1.push(temporal_pair);
        }
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
        let landed = runtime.zone_snapshot(1.0).caption;
        if selection.establishing {
            write_png(&args.out.join("establishing.png"), &pixels)?;
            write_temporal_pair(&args.out, "establishing", &pixels, off_pixels.as_deref())?;
        }
        if selection.impact {
            write_png(&args.out.join("impact.png"), &pixels)?;
            impact_shot = true;
            impact_frame = Some(k);
            write_temporal_pair(&args.out, "impact", &pixels, off_pixels.as_deref())?;
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
                artifact_readback_frames.push(k);
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
                artifact_readback_frames.extend([k, k]);
            }
        }
        if selection.smoke {
            smoke_frame = Some(k);
            write_png(&args.out.join("ground-smoke.png"), &pixels)?;
            write_temporal_pair(&args.out, "ground-smoke", &pixels, off_pixels.as_deref())?;
            if args.compare_particles {
                capture_particle_pair(
                    &mut renderer,
                    view,
                    &mut dynamic,
                    &ui,
                    &args.out,
                    "ground-smoke",
                )?;
                artifact_readback_frames.extend([k, k]);
            }
        }
        if selection.particle {
            capture_particle_pair(
                &mut renderer,
                view,
                &mut dynamic,
                &ui,
                &args.out,
                "selected-frame",
            )?;
            artifact_readback_frames.extend([k, k]);
        }
        if selection.aftermath {
            write_png(&args.out.join("aftermath.png"), &pixels)?;
            write_temporal_pair(&args.out, "aftermath", &pixels, off_pixels.as_deref())?;
            if args.compare_particles {
                capture_particle_pair(
                    &mut renderer,
                    view,
                    &mut dynamic,
                    &ui,
                    &args.out,
                    "aftermath",
                )?;
                artifact_readback_frames.extend([k, k]);
            }
        }
        if selection.numbered {
            write_png(
                &args.out.join("frames").join(format!("{k:04}.png")),
                &pixels,
            )?;
            write_temporal_pair(
                &args.out.join("frames"),
                &format!("{k:04}"),
                &pixels,
                off_pixels.as_deref(),
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
    let summary = serde_json::json!({
        "mode": if args.live { "live" } else { "film" },
        "camera_path": args.camera.name(),
        "static_houses": args.static_houses,
        "sequence_frames": args.sequence,
        "readback_every_frame": args.readback_every_frame,
        "pixel_readback": {
            "policy": if args.compare_temporal_aa { "all_frames_temporal_comparison" } else if args.readback_every_frame { "all_frames_legacy_override" } else if args.video.is_some() { "all_frames_video" } else { "selected_artifact_frames" },
            "primary_readback_count": primary_readback_frames.len(),
            "primary_readback_frame_indices": primary_readback_frames,
            "completion_only_count": completion_only_frames.len(),
            "completion_only_frame_indices": completion_only_frames,
            "temporal_baseline_readback_count": if args.compare_temporal_aa { frames } else { 0 },
            "temporal_baseline_readback_frame_indices": if args.compare_temporal_aa { Some((0..frames).collect::<Vec<_>>()) } else { None },
            "additional_artifact_readback_count": artifact_readback_frames.len(),
            "additional_artifact_readback_frame_indices": artifact_readback_frames,
            "timing_scope": "Every simulation frame renders the full scene once through the primary renderer, advances history and exposure, waits for completion, and enters phase statistics without outlier filtering. Encode timing includes frame validation, fitting, buffer uploads, command encoding, submission, and timestamp instrumentation. Completion timing includes polling and optional timestamp readback. Selected primary frames also include pixel copy, mapping, and CPU pixel extraction; other primary frames use Offscreen::measure and omit those pixel operations. PNG/video writes, repeated comparison renders, and additional artifact readbacks occur after the primary timing sample and are excluded. Temporal comparisons retain pixel readback on every primary and baseline frame and preserve their existing warmed-frame paired intervals.",
        },
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
        "gpu_timestamps_available": flash_phases.iter().chain(&particle_phases).chain(&temporal_phases).flat_map(|(_, pairs)| pairs).any(|p| p.gpu_overhead().is_some()),
        "flash_lights_enabled": !args.no_flash_lights,
        "impact_frame": impact_frame,
        "smoke_frame": smoke_frame,
        "particle_lighting_enabled": !args.no_particle_lighting,
        "soft_particles_enabled": !args.no_soft_particles,
        "temporal_aa_available": renderer.temporal_aa_available(),
        "temporal_aa_enabled": renderer.temporal_aa_available() && !args.no_temporal_aa && !args.compare_flash_lights && !args.compare_particles && std::env::var("VERSE_TEMPORAL_AA").as_deref() != Ok("off"),
        "temporal_comparison": if args.compare_temporal_aa { serde_json::json!({
            "warmup_frames": 8,
            "history_method": "Separate on/off renderers retain their own history and exposure, advance once per identical simulation frame, and alternate order. Each has its own baked-light delivery slot; runtime light and lamp deliveries are forwarded identically before either render. Deliveries arriving mid-pair wait until the next frame. No repeated snapshot renders or temporal toggle resets enter the sample.",
            "gpu_method": "Render-pass boundary timestamps bracket scene commands with two real one-pixel marker draws. The ending marker reads scene output, so it depends on completed output work. Both variants include the same marker work; query resolve, overlay, and pixel readback follow the measured span. Four raw ticks retain before/after marker boundaries. Zero, error-sentinel, decreasing, nonpositive, or unsupported results remain null.",
            "wall_method": "Signed paired wall render-completion increments include CPU encode, submission, mapping, pixel readback, and polling. Every warmed frame enters the mean and spread. The approximate 95% mean interval uses contiguous, balanced batches of about one capture second and Student t bounds, retaining nearby-frame noise together; fewer than two batches yield no interval. Separate order counts and means expose order bias. The upper mean bound tests the 1 ms frame-time budget. These completion measurements do not measure GPU time.",
            "acceptance_frame_time_increment_ms": 1.0,
            "phases": temporal_report(&temporal_phases, renderer.gpu_timestamps_available(), fps),
        }) } else { serde_json::Value::Null },
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
                "gpu_method": "Optional render-pass boundary timestamps around scene commands, including identical one-pixel marker draws in each variant. The final marker samples scene output. Excludes buffer uploads, query resolve, and readback. Zero, error-sentinel, decreasing, and nonpositive durations are invalid.",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selective_readback_covers_every_saved_frame_and_retires_automatic_shots() {
        let mut args = Args::new(PathBuf::new());
        args.video = None;
        args.every = Some(100);
        args.sequence = Some([17, 19]);
        args.compare_particles = true;
        args.particle_frame = Some(23);
        let mut impact_saved = false;
        let mut smoke_saved = false;
        let mut readback = Vec::new();
        for k in 0..240 {
            let selection = args.select_frame(k, 240, 60.0, Some(100), impact_saved, smoke_saved);
            if selection.read_pixels(&args) {
                readback.push(k);
            }
            impact_saved |= selection.impact;
            smoke_saved |= selection.smoke;
        }
        assert_eq!(readback, [0, 17, 18, 19, 23, 100, 120, 139, 166, 180, 200]);
        assert!(impact_saved && smoke_saved);

        args.every = Some(0);
        args.sequence = None;
        args.impact_frame = Some(5);
        args.smoke_frame = Some(3);
        assert!(args.select_frame(5, 240, 60.0, None, false, false).impact);
        assert!(args.select_frame(3, 240, 60.0, None, false, false).smoke);
        assert!(
            !args
                .select_frame(6, 240, 60.0, Some(0), true, true)
                .read_pixels(&args)
        );
    }

    #[test]
    fn video_legacy_and_temporal_policies_read_every_frame() {
        let mut args = Args::new(PathBuf::new());
        let no_artifact = args.select_frame(7, 240, 60.0, None, false, false);
        assert!(no_artifact.read_pixels(&args), "video needs every frame");
        args.video = None;
        assert!(!no_artifact.read_pixels(&args));
        args.readback_every_frame = true;
        assert!(
            no_artifact.read_pixels(&args),
            "legacy policy restores readback"
        );
        args.readback_every_frame = false;
        args.compare_temporal_aa = true;
        assert!(
            no_artifact.read_pixels(&args),
            "paired temporal timing keeps its readback scope"
        );
    }

    #[test]
    fn paired_mean_interval_retains_correlated_noise_and_every_frame() {
        let mut values = vec![0.2; 60];
        values.extend([0.4; 60]);
        values.extend([0.6; 60]);
        let (mean, lower, upper, blocks) = paired_mean_interval(&values, 60).unwrap();
        assert!((mean - 0.4).abs() < 1e-6);
        assert_eq!(blocks, 3);
        assert!(
            upper > 0.8 && lower < 0.0,
            "batch correlation retains uncertainty"
        );
        values.push(2.0);
        let expected = values.iter().map(|&v| f64::from(v)).sum::<f64>() / 181.0;
        assert_eq!(paired_mean_interval(&values, 60).unwrap().0, expected);
        assert!(paired_mean_interval(&values[..60], 60).is_none());
        assert!(paired_mean_interval(&[f32::NAN, 1.0], 1).is_none());
    }

    #[test]
    fn completion_budget_does_not_turn_invalid_gpu_queries_into_gpu_costs() {
        let pairs = (8..128)
            .map(|index| RenderPair {
                index,
                off: (1.0, 3.0),
                on: (1.1, 3.2),
                ..Default::default()
            })
            .collect();
        let report = temporal_report(&[("swarm", pairs)], true, 60.0);
        let phase = &report["swarm"];
        assert_eq!(phase["valid_gpu_frames"], 0);
        assert_eq!(phase["invalid_gpu_frames"], 120);
        assert!(phase["mean_gpu_increment_ms"].is_null());
        assert!(phase["gpu_increment_under_1ms_p99"].is_null());
        assert_eq!(phase["wall_mean_upper_95pct_under_1ms"], true);
        assert_eq!(
            phase["wall_render_completion_mean_95pct_ci_ms"]["samples"],
            120
        );
        assert_eq!(phase["off_first_frames"], 60);
        assert_eq!(phase["on_first_frames"], 60);
    }

    #[test]
    fn paired_bake_deliveries_are_independent_and_match_each_revision() {
        let source = verse::pbr::textured::TexturedScene::default();
        let world = verse::mesh::Mesh {
            textured: Some(std::sync::Arc::new(source)),
            ..Default::default()
        };
        let (on_world, on) = paired_world(&world);
        let (off_world, off) = paired_world(&world);
        let receivers = [on.unwrap(), off.unwrap()];
        let source = &world.textured.as_ref().unwrap().baked;
        for revision in [1, 2] {
            let lights = vec![[revision; 4]; 3];
            let lamps = vec![[revision + 1; 4]; 3];
            source.deliver_lights(lights.clone());
            source.deliver_lamps(lamps.clone());
            forward_bake(source, &receivers);
            assert_eq!(
                on_world.textured.as_ref().unwrap().baked.take(),
                Some(lights.clone())
            );
            assert_eq!(
                off_world.textured.as_ref().unwrap().baked.take(),
                Some(lights)
            );
            assert_eq!(receivers[0].take_lamps(), Some(lamps.clone()));
            assert_eq!(receivers[1].take_lamps(), Some(lamps));
            assert!(source.take().is_none());
        }
    }
}
