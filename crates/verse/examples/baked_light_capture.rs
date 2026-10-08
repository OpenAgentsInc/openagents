//! Captures the production town clock and measures immutable sun blending.
//! Usage: baked_light_capture OUTPUT_DIR [PAIRS] [TIMELAPSE_FRAMES] [--preflight-only]
//! Requires VERSE_KIT_PACK and VERSE_KIT_BAKE for exactly the current scene.

use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::Read;
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
const REPAIR_HOLD_FRAMES: usize = 3600;
const REPAIR_HOLD_SECONDS: u64 = 180;

type RepairDiagnostics = verse::zones::everglade::demolition::town::BakedRepairDiagnostics;

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let preflight_only = args.iter().any(|arg| arg == "--preflight-only");
    let mut args = args.into_iter().filter(|arg| arg != "--preflight-only");
    let dir = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let pairs = number(args.next(), 128)?;
    let frames = number(args.next(), 1440)?;
    if let Some(argument) = args.next() {
        return Err(format!("Unexpected argument: {argument}"));
    }
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
    let inputs = json!({"base_pack":path_identity(&path)?,
        "kit":input_identity(everglade_pack::kit::LOCAL_ENV)?,
        "layers":input_identity(everglade_pack::kit_bake::LOCAL_ENV)?});
    write_json(&dir.join("inputs.json"), &inputs)?;
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
    let preflight = json!({"schema":"openagents.verse-baked-light-preflight.v1",
        "verified":true,"scene":digest,"vertices":layers.vertex_count(),"bake_key":layers.bake_key,
        "inputs":inputs,"captured_before_simulation":true});
    write_json(&dir.join("preflight.json"), &preflight)?;
    if preflight_only {
        println!(
            "Verified baked scene {digest}; {} vertices",
            layers.vertex_count()
        );
        return Ok(());
    }
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
    verify_repair_health(&dir, &repair_diagnostics(&mut runtime)?)?;
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
    let paired_warmup = phase_warmup.next_multiple_of(2);
    let mut samples = Vec::new();
    for pair in 0..pairs + paired_warmup {
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
        if pair >= paired_warmup {
            samples.push(json!({"pair":pair-paired_warmup,"off_first":pair%2==0,
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
    let destruction = destruction_capture(&mut runtime, &mut on_renderer, &ui, &dir)?;
    let report = json!({"schema":"openagents.verse-baked-light-capture.v1",
        "resolution":[WIDTH,HEIGHT],"adapter":format!("{:?}",adapter),"quality":format!("{:?}",on_renderer.quality().tier),
        "scene":digest,"vertices":layers.vertex_count(),"bake_key":layers.bake_key,
        "inputs":inputs,"inputs_hashed_before_simulation":true,
        "clock":"Unpinned production wall-clock adapter; solar weights, sky brightness and lamp fade use exact town time; sky shape keeps its scheduled cadence",
        "phase_warmup_frames":phase_warmup,
        "timelapse_method":{"frames":frames,"hours":24.0,"pixels":"Selected frames; all other frames complete rendering without pixel extraction",
            "scheduled_sky_frames_per_step":(frames-1) as f64/360.0,
            "scheduled_sky_warmup_frames":phase_warmup,
            "sky_bake_can_finish_between_steps":(frames-1) as f64/360.0 >= phase_warmup as f64,
            "limitation":"An accelerated timeline with too few frames per scheduled sky step can retain an older sky shape; exact brightness and sun weights still advance. Named phase images converge the sky bake."},
        "temporal_aa":false,"captures":captures,"timelapse":timelapse,
        "destruction":destruction,
        "measurement":{"pairs":pairs,"warmup_pairs":paired_warmup,"independent_renderers":true,"identical_bake_replayed":true,"fresh_same_state_renderers":true,
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

fn destruction_capture(
    runtime: &mut WorldRuntime,
    renderer: &mut verse::render::Offscreen,
    ui: &verse::ui::UiBatch,
    dir: &Path,
) -> Result<serde_json::Value, String> {
    use glam::Vec3;
    use verse::zones::everglade::demolition::meteor::Volley;
    runtime.set_town_clock(town_clock::Clock::DAYTIME.pinned(Some(12.0)));
    let building = runtime
        .everglade_zone_mut()
        .and_then(|z| z.town())
        .ok_or("The baked town has no demolition state")?
        .buildings()
        .iter()
        .filter(|b| b.destructible() && !b.is_carved())
        .min_by(|a, b| {
            let distance = |b: &verse::zones::everglade::demolition::town::Building| {
                (b.rect.0[0] + 42.0).powi(2) + (b.rect.0[1] - 16.0).powi(2)
            };
            distance(a).total_cmp(&distance(b))
        })
        .cloned()
        .ok_or("The baked town has no destructible kit building")?;
    let ([x, z], [hx, hz]) = building.rect;
    let aim = Vec3::new(x, building.base, z);
    let eye = Vec3::new(x - hx - 22.0, building.base + 8.0, z + hz + 24.0);
    runtime.set_spawn(eye.with_y(building.base), 0.0)?;
    runtime.set_shot(Some((eye, aim + Vec3::Y * 4.0)));
    let idle = InputState::default();
    runtime.tick(&idle, 1.0 / 60.0);
    let repair_before = repair_diagnostics(runtime)?;
    verify_repair_health(dir, &repair_before)?;
    let mut dynamic = runtime.dynamic_mesh();
    dynamic.neon.as_mut().unwrap().temporal_aa = false;
    let view = runtime.view(WIDTH as f32 / HEIGHT as f32);
    for _ in 0..sky_warmup(renderer.quality()) {
        renderer.measure(view, &dynamic, ui)?;
    }
    write_png(
        &dir.join("destruction-pristine.png"),
        &renderer.render(view, &dynamic, ui)?,
    )?;
    let targets = vec![
        Vec3::new(x, building.top - 0.5, z),
        Vec3::new(x - hx, building.base + 1.5, z + hz),
        Vec3::new(x + hx, building.base + 1.5, z + hz),
        Vec3::new(x, building.base + 2.0, z + hz),
    ];
    runtime
        .everglade_zone_mut()
        .and_then(|zone| zone.town_mut())
        .ok_or("The baked town disappeared")?
        .start_showcase(
            eye.with_y(building.base),
            aim,
            targets,
            Volley::SHOWCASE,
            0.5,
            600.0,
        );
    let mut captures = Vec::new();
    let mut relit_max = 0;
    let mut hidden_max = 0;
    for frame in 0..960 {
        runtime.tick(&idle, 1.0 / 60.0);
        let mut dynamic = runtime.dynamic_mesh();
        dynamic.neon.as_mut().unwrap().temporal_aa = false;
        let view = runtime.view(WIDTH as f32 / HEIGHT as f32);
        let town = runtime
            .everglade_zone_mut()
            .and_then(|zone| zone.town())
            .unwrap();
        let relit = town
            .site()
            .pieces()
            .iter()
            .filter(|piece| piece.relight)
            .count();
        let repair = town.baked_repair_diagnostics();
        verify_repair_health(dir, &repair)?;
        relit_max = relit_max.max(relit);
        hidden_max = hidden_max.max(town.hidden());
        if [360, 480, 600, 900].contains(&frame) {
            let file = format!("destruction-{frame:04}.png");
            write_png(&dir.join(&file), &renderer.render(view, &dynamic, ui)?)?;
            captures.push(json!({"frame":frame,"file":file,"relit_pieces":relit,"hidden_placements":town.hidden(),"chunks":town.profile().chunks,"repair":repair}));
        } else {
            renderer.measure(view, &dynamic, ui)?;
        }
    }
    if relit_max == 0 || hidden_max == 0 {
        return Err("The meteor capture did not break a baked kit building".into());
    }
    let after_swarm = repair_diagnostics(runtime)?;
    let (noon_repair, noon_hold) = drain_repair(runtime, renderer, ui, dir, &repair_before)?;
    if noon_repair.completed_geometry_generations <= repair_before.completed_geometry_generations {
        write_json(
            &dir.join("repair-verification.json"),
            &json!({"verified":false,"reason":"No selective geometry generation completed",
                "before":repair_before,"noon":noon_repair,"noon_hold":noon_hold}),
        )?;
        return Err("The destruction capture completed no selective geometry repair".into());
    }
    let mut dynamic = runtime.dynamic_mesh();
    dynamic.neon.as_mut().unwrap().temporal_aa = false;
    write_png(
        &dir.join("destruction-repaired-noon.png"),
        &renderer.render(runtime.view(WIDTH as f32 / HEIGHT as f32), &dynamic, ui)?,
    )?;

    runtime.set_town_clock(town_clock::Clock::DAYTIME.pinned(Some(0.0)));
    let (night_repair, night_hold) = drain_repair(runtime, renderer, ui, dir, &noon_repair)?;
    if night_repair.geometry_epoch != noon_repair.geometry_epoch
        || night_repair.completed_clock_generations <= noon_repair.completed_clock_generations
        || night_repair.clock_minute == noon_repair.clock_minute
        || night_repair.sun_direction == noon_repair.sun_direction
        || night_repair.sun_lux >= noon_repair.sun_lux
        || night_repair.sky_lux >= noon_repair.sky_lux
    {
        write_json(
            &dir.join("repair-verification.json"),
            &json!({"verified":false,"reason":"Clock repair did not reuse geometry and follow midnight light",
                "noon":noon_repair,"night":night_repair,"noon_hold":noon_hold,"night_hold":night_hold}),
        )?;
        return Err(
            "The destruction capture did not repair the same geometry under midnight light".into(),
        );
    }
    let mut dynamic = runtime.dynamic_mesh();
    dynamic.neon.as_mut().unwrap().temporal_aa = false;
    let view = runtime.view(WIDTH as f32 / HEIGHT as f32);
    let phase_warmup = sky_warmup(renderer.quality());
    for _ in 0..phase_warmup {
        renderer.measure(view, &dynamic, ui)?;
    }
    write_png(
        &dir.join("destruction-repaired-night.png"),
        &renderer.render(view, &dynamic, ui)?,
    )?;

    runtime.set_town_clock(town_clock::Clock::DAYTIME.pinned(Some(12.0)));
    runtime.zone_intent(verse::zones::Intent::Rebuild)?;
    let town = runtime
        .everglade_zone_mut()
        .and_then(|zone| zone.town())
        .unwrap();
    let pristine = town.hidden() == 0 && town.site().pieces().iter().all(|piece| !piece.relight);
    if !pristine {
        return Err("Restore did not reset the town's fallback state".into());
    }
    let repair_restored = town.baked_repair_diagnostics();
    if repair_restored.active
        || repair_restored.current_targets != 0
        || repair_restored.current_backlog != 0
    {
        return Err("Restore did not invalidate selective repair work".into());
    }
    let mut dynamic = runtime.dynamic_mesh();
    dynamic.neon.as_mut().unwrap().temporal_aa = false;
    let view = runtime.view(WIDTH as f32 / HEIGHT as f32);
    for _ in 0..phase_warmup {
        poll_held_zone(runtime)?;
        dynamic = runtime.dynamic_mesh();
        dynamic.neon.as_mut().unwrap().temporal_aa = false;
        renderer.measure(view, &dynamic, ui)?;
    }
    let repair_restored_after_poll = repair_diagnostics(runtime)?;
    verify_repair_health(dir, &repair_restored_after_poll)?;
    let town = runtime
        .everglade_zone_mut()
        .and_then(|zone| zone.town())
        .unwrap();
    if repair_restored_after_poll.current_targets != 0
        || repair_restored_after_poll.current_backlog != 0
        || town.hidden() != 0
        || town.site().pieces().iter().any(|piece| piece.relight)
    {
        return Err(
            "Restore warmup revived selective repair targets or changed pristine geometry".into(),
        );
    }
    write_png(
        &dir.join("destruction-restored.png"),
        &renderer.render(view, &dynamic, ui)?,
    )?;
    let verification = json!({"schema":"openagents.verse-baked-repair-verification.v1","verified":true,
        "before":repair_before,"after_swarm":after_swarm,"noon":noon_repair,"night":night_repair,
        "restored_before_poll":repair_restored,"restored_after_poll":repair_restored_after_poll,
        "noon_hold":noon_hold,"night_hold":night_hold,"night_and_restore_sky_warmup_frames_each":phase_warmup,
        "restore_warmup_simulation_dt":0.0,
        "noon_image":"destruction-repaired-noon.png","night_image":"destruction-repaired-night.png"});
    write_json(&dir.join("repair-verification.json"), &verification)?;
    Ok(
        json!({"building_center":[x,z],"building_is_kit":true,"frames":960,"fps":60,
        "pristine":"destruction-pristine.png","restored":"destruction-restored.png",
        "relit_pieces_max":relit_max,"hidden_placements_max":hidden_max,"restore_fallback_reset":pristine,
        "selective_repair":verification,
        "captures":captures,"timing":"Diagnostic serial rendering; separate blend comparison supplies frame cost"}),
    )
}

fn repair_diagnostics(runtime: &mut WorldRuntime) -> Result<RepairDiagnostics, String> {
    runtime
        .everglade_zone_mut()
        .and_then(|zone| zone.town())
        .map(|town| town.baked_repair_diagnostics())
        .ok_or("The baked town disappeared".into())
}

fn poll_held_zone(runtime: &mut WorldRuntime) -> Result<(), String> {
    // Runtime tick ignores dt=0. The zone tick polls one worker batch
    // while physics time and the pinned town clock remain fixed.
    let player = runtime.player;
    runtime
        .everglade_zone_mut()
        .ok_or("The baked zone disappeared")?
        .tick(0.0, &player, &[]);
    Ok(())
}

fn repair_healthy(repair: &RepairDiagnostics) -> bool {
    repair.enabled
        && repair.error.is_none()
        && repair.error_count == 0
        && repair.rejected_chunk_vertices == 0
}

fn verify_repair_health(dir: &Path, repair: &RepairDiagnostics) -> Result<(), String> {
    if repair_healthy(repair) {
        return Ok(());
    }
    write_json(
        &dir.join("repair-verification.json"),
        &json!({"verified":false,"repair":repair}),
    )?;
    Err("Selective repair is disabled, has a worker error, or rejected a chunk patch".into())
}

fn repair_ready(before: &RepairDiagnostics, repair: &RepairDiagnostics) -> bool {
    repair_healthy(repair)
        && repair.active
        && repair.generation > before.generation
        && repair.current_targets > 0
        && repair.current_complete
        && repair.current_backlog == 0
        && repair.current_processed == repair.current_targets
        && repair.current_skipped == 0
        && repair.current_applied_vertices == repair.current_targets
        && repair.last_completed_generation == repair.generation
        && repair.completed_generations > before.completed_generations
        && repair.applied_batches > before.applied_batches
        && repair.delivered_vertices > before.delivered_vertices
        && repair.applied_static_vertices > before.applied_static_vertices
        && repair.applied_chunk_vertices > before.applied_chunk_vertices
}

fn drain_repair(
    runtime: &mut WorldRuntime,
    renderer: &mut verse::render::Offscreen,
    ui: &verse::ui::UiBatch,
    dir: &Path,
    before: &RepairDiagnostics,
) -> Result<(RepairDiagnostics, serde_json::Value), String> {
    let started = std::time::Instant::now();
    let mut frames = 0;
    let mut progress = Vec::new();
    let mut repair = repair_diagnostics(runtime)?;
    verify_repair_health(dir, &repair)?;
    while !repair_ready(before, &repair)
        && frames < REPAIR_HOLD_FRAMES
        && started.elapsed().as_secs() < REPAIR_HOLD_SECONDS
    {
        poll_held_zone(runtime)?;
        let mut dynamic = runtime.dynamic_mesh();
        dynamic.neon.as_mut().unwrap().temporal_aa = false;
        renderer.measure(runtime.view(WIDTH as f32 / HEIGHT as f32), &dynamic, ui)?;
        frames += 1;
        repair = repair_diagnostics(runtime)?;
        verify_repair_health(dir, &repair)?;
        if frames % 30 == 0 {
            progress.push(json!({"hold_frame":frames,"wall_seconds":started.elapsed().as_secs_f64(),"repair":repair}));
        }
    }
    let hold = json!({"frames":frames,"wall_seconds":started.elapsed().as_secs_f64(),
        "frame_limit":REPAIR_HOLD_FRAMES,"wall_seconds_limit":REPAIR_HOLD_SECONDS,
        "simulation_dt":0.0,"clock":"Pinned; no physics time advances",
        "method":"Production zone tick polls at most one selective batch per hold frame; each frame completes rendering without pixel extraction",
        "progress":progress});
    if !repair_ready(before, &repair) {
        write_json(
            &dir.join("repair-verification.json"),
            &json!({"verified":false,"reason":"Selective repair did not finish within the bounded hold",
            "before":before,"repair":repair,"hold":hold}),
        )?;
        return Err(format!(
            "Selective repair remains incomplete: {} of {} targets processed after {frames} hold frames",
            repair.current_processed, repair.current_targets
        ));
    }
    Ok((repair, hold))
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
    path_identity(Path::new(&path))
}

fn path_identity(path: &Path) -> Result<serde_json::Value, String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let mut file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0; 32 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        bytes += count as u64;
    }
    Ok(json!({"path":path,"bytes":bytes,"sha256":format!("{:x}",hash.finalize())}))
}

fn write_json(path: &Path, report: &serde_json::Value) -> Result<(), String> {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
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
    fn selective_capture_requires_current_worker_completion_and_both_receiver_types() {
        let before = RepairDiagnostics {
            enabled: true,
            generation: 7,
            completed_generations: 2,
            applied_batches: 3,
            delivered_vertices: 8,
            applied_static_vertices: 4,
            applied_chunk_vertices: 4,
            ..Default::default()
        };
        let complete = RepairDiagnostics {
            active: true,
            generation: 8,
            current_targets: 5,
            current_processed: 5,
            current_applied_vertices: 5,
            current_complete: true,
            last_completed_generation: 8,
            completed_generations: 3,
            applied_batches: 5,
            delivered_vertices: 13,
            applied_static_vertices: 6,
            applied_chunk_vertices: 7,
            ..before.clone()
        };
        assert!(repair_ready(&before, &complete));
        let mut skipped_target = complete.clone();
        skipped_target.current_skipped = 1;
        skipped_target.current_applied_vertices -= 1;
        assert!(!repair_ready(&before, &skipped_target));
        let mut missing_application = complete.clone();
        missing_application.current_applied_vertices -= 1;
        assert!(!repair_ready(&before, &missing_application));
        let mut fallback_only = complete.clone();
        fallback_only.current_targets = 0;
        fallback_only.current_processed = 0;
        fallback_only.current_applied_vertices = 0;
        fallback_only.current_skipped = 0;
        assert!(!repair_ready(&before, &fallback_only));
        let mut old_completion = complete.clone();
        old_completion.generation += 1;
        assert!(!repair_ready(&before, &old_completion));
        let mut unfinished = complete.clone();
        unfinished.current_complete = false;
        unfinished.current_backlog = 1;
        assert!(!repair_ready(&before, &unfinished));
        let mut missing_chunks = complete.clone();
        missing_chunks.applied_chunk_vertices = before.applied_chunk_vertices;
        assert!(!repair_ready(&before, &missing_chunks));
        let mut missing_ground = complete.clone();
        missing_ground.applied_static_vertices = before.applied_static_vertices;
        assert!(!repair_ready(&before, &missing_ground));
        let mut lost_error = complete.clone();
        lost_error.error_count = 1;
        assert!(!repair_ready(&before, &lost_error));
        let mut rejected = complete;
        rejected.rejected_chunk_vertices = 1;
        assert!(!repair_ready(&before, &rejected));
        assert!(!repair_ready(&before, &RepairDiagnostics::default()));
    }

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
