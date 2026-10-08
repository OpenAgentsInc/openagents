//! Captures the production town clock and measures immutable sun blending.
//! Usage: baked_light_capture OUTPUT_DIR [PAIRS] [TIMELAPSE_FRAMES]
//! Requires VERSE_KIT_PACK and VERSE_KIT_BAKE for exactly the current scene.

use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones::{everglade::Everglade, everglade_pack},
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const WARMUP: usize = 16;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let pairs = number(args.next(), 128)?;
    let frames = number(args.next(), 1440)?;
    if pairs < 16 || pairs % 16 != 0 || frames < 2 {
        return Err(
            "Pairs must be a positive multiple of 16; timelapse frames must be at least 2".into(),
        );
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!("{}.vtp", everglade_pack::PACK_SHA256));
    let pack = everglade_pack::ZonePack::load_local(&path)?;
    let layers =
        everglade_pack::kit_bake::offered().ok_or("The capture requires offline light layers")?;
    let world = Everglade::world(&pack)?;
    let scene = world
        .mesh
        .textured
        .as_ref()
        .ok_or("Everglade has no textured scene")?;
    let merged = scene.merge()?;
    let digest =
        verse::pbr::baked_layers::hex(&verse::pbr::baked_layers::scene_digest(scene, &merged));
    if digest != layers.scene || merged.vertices.len() != layers.vertex_count() {
        return Err(format!(
            "Offline layers match scene {}, but the capture builds {digest}",
            layers.scene
        ));
    }
    layers.validate()?;
    drop(merged);
    drop(world);
    let mut runtime = WorldRuntime::new();
    runtime.set_town_clock(running_clock(12.0));
    runtime.install_everglade(&pack);
    runtime.settle_zone_light();
    if !runtime
        .everglade_zone_mut()
        .is_some_and(|z| z.uses_baked_light())
    {
        return Err("The capture requires verified offline layers".into());
    }
    runtime.set_spawn(glam::Vec3::new(-60.0, 0.0, 30.0), std::f32::consts::PI)?;
    runtime.apply(Action::Orbit { dx: 0.0, dy: 30.0 })?;
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();
    let air = verse::zones::atmosphere(runtime.zone);
    let mut on_renderer =
        verse::render::Offscreen::new(WIDTH, HEIGHT, &runtime.world.mesh, &atlas, air)?;
    let mut off_renderer =
        verse::render::Offscreen::new(WIDTH, HEIGHT, &runtime.world.mesh, &atlas, air)?;
    // Both renderers receive the same immutable bake. Initial delivery slots
    // are consumable, so replay them before warming the second renderer.
    let scene = runtime
        .world
        .mesh
        .textured
        .as_ref()
        .ok_or("Everglade has no textured scene")?
        .clone();
    let idle = InputState::default();
    runtime.tick(&idle, 1.0 / 60.0);
    let mut dynamic = runtime.dynamic_mesh();
    dynamic
        .neon
        .as_mut()
        .ok_or("Everglade has no light stage")?
        .temporal_aa = false;
    on_renderer.render(runtime.view(WIDTH as f32 / HEIGHT as f32), &dynamic, &ui)?;
    replay_layers(&scene, &layers);
    off_renderer.render(runtime.view(WIDTH as f32 / HEIGHT as f32), &dynamic, &ui)?;
    let adapter = on_renderer.adapter_info().clone();
    let phase_warmup = sky_warmup(on_renderer.quality());
    let mut captures = Vec::new();
    for (name, hour) in [
        ("dawn", 6.0),
        ("noon", 12.0),
        ("dusk", 18.0),
        ("night", 0.0),
        ("sun-08-before", 7.95),
        ("sun-08-after", 8.05),
        ("sun-12-before", 11.95),
        ("sun-12-after", 12.05),
        ("sun-1530-before", 15.45),
        ("sun-1530-after", 15.55),
        ("sun-1730-before", 17.45),
        ("sun-1730-after", 17.55),
    ] {
        runtime.set_town_clock(running_clock(hour));
        runtime.tick(&idle, 1.0 / 60.0);
        let mut dynamic = runtime.dynamic_mesh();
        dynamic
            .neon
            .as_mut()
            .ok_or("Everglade has no light stage")?
            .temporal_aa = false;
        let view = runtime.view(WIDTH as f32 / HEIGHT as f32);
        for _ in 0..phase_warmup {
            on_renderer.measure(view, &dynamic, &ui)?;
        }
        let pixels = on_renderer.render(view, &dynamic, &ui)?;
        let file = format!("{name}.png");
        write_png(&dir.join(&file), &pixels)?;
        captures.push(
            json!({"file":file,"requested_hour":hour,"sun":dynamic.neon.as_ref().unwrap().baked_sun,
            "sky_lux":dynamic.neon.as_ref().unwrap().baked_sky,"warmup_frames":phase_warmup}),
        );
    }
    let mut timelapse = Vec::new();
    let save_every = (frames / 48).max(1);
    for frame in 0..frames {
        let hour = 24.0 * frame as f64 / (frames - 1) as f64;
        runtime.set_town_clock(running_clock(hour));
        runtime.tick(&idle, 1.0 / 60.0);
        let mut dynamic = runtime.dynamic_mesh();
        dynamic
            .neon
            .as_mut()
            .ok_or("Everglade has no light stage")?
            .temporal_aa = false;
        let view = runtime.view(WIDTH as f32 / HEIGHT as f32);
        let file = if frame % save_every == 0 || frame + 1 == frames {
            let name = format!("clock-{frame:04}.png");
            let pixels = on_renderer.render(view, &dynamic, &ui)?;
            write_png(&dir.join(&name), &pixels)?;
            Some(name)
        } else {
            on_renderer.measure(view, &dynamic, &ui)?;
            None
        };
        timelapse.push(json!({"frame":frame,"requested_hour":hour,"file":file,"sun":dynamic.neon.as_ref().unwrap().baked_sun,
            "sky_lux":dynamic.neon.as_ref().unwrap().baked_sky}));
    }
    runtime.set_town_clock(running_clock(9.0));
    runtime.tick(&idle, 1.0 / 60.0);
    let mut on = runtime.dynamic_mesh();
    on.neon
        .as_mut()
        .ok_or("Everglade has no light stage")?
        .temporal_aa = false;
    let mut off = on.clone();
    off.neon.as_mut().unwrap().baked_sun = [0.0; 4];
    let view = runtime.view(WIDTH as f32 / HEIGHT as f32);
    // Give both variants the same first sky bake and exposure history;
    // the timelapse renderer's previous sky must not enter the comparison.
    on_renderer = verse::render::Offscreen::new(WIDTH, HEIGHT, &runtime.world.mesh, &atlas, air)?;
    off_renderer = verse::render::Offscreen::new(WIDTH, HEIGHT, &runtime.world.mesh, &atlas, air)?;
    replay_layers(&scene, &layers);
    on_renderer.render(view, &on, &ui)?;
    replay_layers(&scene, &layers);
    off_renderer.render(view, &off, &ui)?;
    let mut samples = Vec::new();
    for pair in 0..pairs + WARMUP {
        let mut record = [(0.0, 0.0, None); 2];
        for mode in [pair % 2, 1 - pair % 2] {
            let (renderer, dynamic) = if mode == 0 {
                (&mut off_renderer, &off)
            } else {
                (&mut on_renderer, &on)
            };
            renderer.render(view, dynamic, &ui)?;
            let (encode, completion) = renderer.last_timing();
            record[mode] = (encode, completion, renderer.last_gpu_ms());
        }
        if pair >= WARMUP {
            samples.push(json!({"pair":pair-WARMUP,"off_first":pair%2==0,
                "off_encode_ms":record[0].0,"off_completion_ms":record[0].1,"off_gpu_ms":record[0].2,
                "on_encode_ms":record[1].0,"on_completion_ms":record[1].1,"on_gpu_ms":record[1].2,
                "frame_completion_increment_ms":(record[1].0+record[1].1)-(record[0].0+record[0].1)}));
        }
    }
    let increments: Vec<_> = samples
        .iter()
        .map(|x| x["frame_completion_increment_ms"].as_f64().unwrap())
        .collect();
    let (mean, lower, upper) = mean_interval(&increments);
    let report = json!({"schema":"openagents.verse-baked-light-capture.v1",
        "resolution":[WIDTH,HEIGHT],"adapter":format!("{:?}",adapter),"quality":format!("{:?}",on_renderer.quality().tier),
        "scene":digest,"vertices":layers.vertex_count(),"bake_key":layers.bake_key,
        "inputs":{"kit":input_identity(everglade_pack::kit::LOCAL_ENV)?,"layers":input_identity(everglade_pack::kit_bake::LOCAL_ENV)?},
        "clock":"Unpinned production wall-clock adapter; solar weights, sky brightness and lamp fade use exact town time; sky shape keeps its scheduled cadence",
        "phase_warmup_frames":phase_warmup,
        "timelapse_method":{"frames":frames,"hours":24.0,"pixels":"Selected frames; all other frames complete rendering without pixel extraction",
            "scheduled_sky_frames_per_step":(frames-1) as f64/360.0,
            "scheduled_sky_warmup_frames":phase_warmup,
            "sky_bake_can_finish_between_steps":(frames-1) as f64/360.0 >= phase_warmup as f64,
            "limitation":"An accelerated timeline with too few frames per scheduled sky step can retain an older sky shape; exact brightness and sun weights still advance. Named phase images converge the sky bake."},
        "temporal_aa":false,"captures":captures,"timelapse":timelapse,
        "measurement":{"pairs":pairs,"warmup_pairs":WARMUP,"independent_renderers":true,"identical_bake_replayed":true,"fresh_same_state_renderers":true,
            "initial_seed_frames_per_variant":1,
            "off_first":pairs/2,"on_first":pairs/2,"readback":"Both variants read back every measured frame",
            "scope":"CPU fit, encode and submit plus serial completion wait, polling, mapping and pixel extraction; excludes PNG writing and simulation",
            "gpu_timestamps_supported":on_renderer.gpu_timestamps_available(),"gpu_timestamps_enabled":on_renderer.gpu_timestamps_enabled(),
            "invalid_gpu_durations":"null; wall completion is not GPU time","filtered_samples":0,
            "frame_completion_mean_increment_ms":mean,"approximate_95pct_block_interval_ms":[lower,upper],
            "interval_method":"Eight contiguous equal-sized batches with balanced render order; Student t df7; all measured samples retained"},
        "samples":samples});
    std::fs::write(
        dir.join("capture.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!(
        "Baked blend frame-completion mean {mean:.3} ms; approximate 95% interval {lower:.3} to {upper:.3} ms"
    );
    Ok(())
}

fn sky_warmup(quality: verse_engine::quality::Quality) -> usize {
    let (size, samples) = quality.sky_cube();
    let cost = verse_engine::environment::Prefilter::new(size, samples).cost();
    (cost.div_ceil(verse::pbr::environment::GRADUAL_BUDGET) as usize + 1).max(WARMUP)
}

fn number(value: Option<String>, default: usize) -> Result<usize, String> {
    value
        .map(|v| {
            v.parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())
        })
        .transpose()
        .map(|x| x.unwrap_or(default))
}

fn replay_layers(
    scene: &verse::pbr::textured::TexturedScene,
    layers: &Arc<verse::pbr::baked_layers::Layers>,
) {
    scene.baked.deliver_lights(layers.sky.clone());
    scene.baked.deliver_lamps(layers.lamp_texels());
    scene.baked.deliver_layers(layers.clone());
}

fn running_clock(hour: f64) -> town_clock::Clock {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    town_clock::Clock {
        epoch_unix: unix - (hour.rem_euclid(24.0) * 3600.0).round() as i64,
        mode: town_clock::Mode::WallClock {
            utc_offset_minutes: 0,
        },
        pinned_second: None,
    }
}

fn input_identity(name: &str) -> Result<serde_json::Value, String> {
    let path = std::env::var_os(name).ok_or_else(|| format!("Missing {name}"))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(
        json!({"path":PathBuf::from(path),"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))}),
    )
}

fn mean_interval(values: &[f64]) -> (f64, f64, f64) {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let batches: Vec<_> = values
        .chunks_exact(values.len() / 8)
        .map(|b| b.iter().sum::<f64>() / b.len() as f64)
        .collect();
    let variance = batches.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / 7.0;
    let half = 2.364624 * (variance / 8.0).sqrt();
    (mean, mean - half, mean + half)
}

fn write_png(path: &Path, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paired_interval_retains_order_noise_and_uses_all_balanced_batches() {
        let values: Vec<_> = (0..128)
            .map(|i| 0.25 + if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        let (mean, lo, hi) = mean_interval(&values);
        assert!((mean - 0.25).abs() < 1e-12);
        assert_eq!(lo, mean);
        assert_eq!(hi, mean);
        let noisy: Vec<_> = (0..128).map(|i| 0.25 + (i / 16) as f64 * 0.1).collect();
        let (mean, lo, hi) = mean_interval(&noisy);
        assert!(lo < mean && hi > mean);
    }
}
