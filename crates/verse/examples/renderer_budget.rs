//! Reproducible original-content GPU crowd and quality-budget measurements.
//! Usage: renderer_budget OUTPUT.json FRAMES
use glam::{Mat4, Vec3};
use std::time::{Duration, Instant};
use verse::{
    imported::{Renderer, chamber, original},
    profiling::FrameProfile,
    ui::UiBatch,
};
use verse_engine::{
    core::LifeId,
    motion::State,
    presentation::{Instance, Mount, View},
};
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Expected OUTPUT.json FRAMES".into());
    }
    let frames: u64 = args[1].parse().map_err(|_| "Invalid frame count")?;
    if !(180..=1200).contains(&frames) {
        return Err("Expected 180–1200 frames".into());
    }
    let assets = tempfile::tempdir().map_err(|e| e.to_string())?;
    let pack = original::generate(assets.path())?;
    let atlas = original::atlas()?;
    let static_instances = chamber::static_instances(&pack, Vec3::ZERO);
    let startup = Instant::now();
    let mut renderer = Renderer::new(pack, assets.path(), 1280, 720, &atlas, &static_instances)?;
    let mut profile = FrameProfile::new(120);
    profile.record(
        0,
        "construction_cpu_ms",
        startup.elapsed().as_secs_f64() * 1000.,
    );
    let mut source = Vec::new();
    for actor in 0..60 {
        let player = actor < 20;
        let life = LifeId {
            instance: 9200,
            actor: actor + 1,
            generation: 0,
        };
        source.push(Instance {
            actor: Some(life),
            mount: None,
            model: if player { "adventurer" } else { "cultist" }.into(),
            transform: Mat4::from_translation(Vec3::new(
                (actor % 10) as f32 * 2. - 9.,
                0.,
                -12. - (actor / 10) as f32 * 2.,
            )) * chamber::basis(),
            animation: State::Walk.into(),
            time: 0.,
            animation_epoch: None,
            emission: Vec3::ONE,
        });
        if player {
            source.push(Instance {
                actor: None,
                mount: Some(Mount {
                    parent: life,
                    parent_model: "adventurer".into(),
                    socket: 2,
                    local: Mat4::IDENTITY,
                    pose: verse_engine::presentation::MountPose::Socket,
                }),
                model: "bow".into(),
                transform: Mat4::IDENTITY,
                animation: 0.into(),
                time: 0.,
                animation_epoch: None,
                emission: Vec3::ONE,
            });
        }
    }
    for index in 0..2000 {
        source.push(Instance {
            actor: None,
            mount: None,
            model: "particle-spark".into(),
            transform: Mat4::from_translation(Vec3::new(
                (index % 40) as f32 * 0.5 - 10.,
                1.,
                -12. - (index / 40) as f32 * 0.2,
            )) * Mat4::from_scale(Vec3::splat(0.1)),
            animation: 0.into(),
            time: 0.,
            animation_epoch: None,
            emission: Vec3::ONE,
        });
    }
    let eye = Vec3::new(12., 10., 8.);
    let view = View {
        view_proj: Mat4::perspective_rh(60f32.to_radians(), 16. / 9., 0.1, 600.)
            * Mat4::look_at_rh(eye, Vec3::new(0., 1., -17.), Vec3::Y),
        eye,
    };
    let mut lighting = chamber::playground_lighting();
    let ui = UiBatch::default();
    let started = Instant::now();
    for frame in 1..=frames {
        let cycle = Instant::now();
        for instance in &mut source {
            instance.time = frame as f32 / 60.;
        }
        lighting.time = frame as f32 / 60.;
        renderer.draw_live(view, &source, &ui, &lighting)?;
        let t = renderer.last_timings;
        if t.actor_roots != 60 || t.mounts != 20 {
            return Err("Quality fallback omitted required actor or mount".into());
        }
        for (name, value) in [
            ("draw_cpu_ms", t.total_ms),
            ("prepare_cpu_ms", t.prepare_ms),
            ("shadow_encode_cpu_ms", t.shadow_encode_ms),
            ("world_encode_cpu_ms", t.world_encode_ms),
            ("instances", t.instances as f64),
            ("actor_roots", t.actor_roots as f64),
            ("mounts", t.mounts as f64),
            ("retained_effects", t.optional_effects as f64),
            ("dropped_effects", t.dropped_effects as f64),
            ("surface_batches", t.surface_batches as f64),
            ("shadow_views", t.shadow_views as f64),
            ("upload_bytes", t.upload_bytes as f64),
            ("world_draws", t.world_draws as f64),
            ("shadow_draws", t.shadow_draws as f64),
        ] {
            profile.record(t.frame, name, value);
        }
        for sample in t.gpu_samples.into_iter().flatten() {
            profile.record(sample.frame, "gpu_scene_ms", sample.total_ms);
        }
        if let Some(health) = t.gpu_health {
            profile.record(t.frame, "timestamp_busy_slots", health.busy_frames as f64);
        }
        if let Some(remaining) = Duration::from_secs_f64(1. / 60.).checked_sub(cycle.elapsed()) {
            std::thread::sleep(remaining);
        }
    }
    let report = serde_json::json!({"schema":"verse.renderer-budget.v1","fixture":"20 synthetic player roots, 40 synthetic NPC roots, 20 required bow mounts, 2000 optional spark instances; no network or gameplay simulation","frames":frames,"resolution":[1280,720],"device":renderer.device_profile,"resources":renderer.resources,"resource_accounting":"Logical payload and conservative buffer reservations; driver padding, swapchain, shader-private resources, and transient capture copies excluded","measurements":profile.summary(),"wall_seconds":started.elapsed().as_secs_f64(),"profile":"debug","actual_input_to_display_ms":null,"all_required_visuals_preserved":true,"timestamp_health":renderer.last_timings.gpu_health});
    std::fs::write(
        &args[0],
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{}", serde_json::to_string(&report["device"]).unwrap());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
