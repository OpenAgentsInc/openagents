//! Scratch TLS clients with independent offscreen GPU, CPU, and capture measurements.
//! Usage: frame_profile OUTPUT.json CLIENTS FRAMES CAPTURE_EVERY
use glam::{Mat4, Vec3};
use rustls::{
    ClientConfig, RootCertStore, ServerConfig,
    pki_types::{PrivatePkcs8KeyDer, ServerName},
};
use secp256k1::{Keypair, Secp256k1};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use verse::{
    imported::{Renderer, chamber, original, remote_content},
    profiling::FrameProfile,
};
use verse_world::{
    Intent,
    play::Game,
    service::{
        Chamber,
        auth::Gateway,
        client::Client,
        event_cursor::Cursor,
        net::{self, Phases, Timing},
        persistence::Store,
        view::{Camera, View},
        worker::{self, Input, Observer, Update},
    },
};

const INSTANCE: u64 = 9100;
const WARMUP: u64 = 120;
struct Slot {
    renderer: Renderer,
    view: View,
    input: tokio::sync::mpsc::Sender<Input>,
    output: tokio::sync::mpsc::Receiver<Update>,
    observer: Observer,
    profile: FrameProfile,
    capture_profile: FrameProfile,
    stop: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<Result<(), String>>,
    snapshot_at: Option<Instant>,
    steady_at: Option<Instant>,
    snapshots: u64,
    outcomes: u64,
    refusals: u64,
}
fn timing(value: &Timing) -> serde_json::Value {
    serde_json::json!({"count":value.count,"total_ms":value.total_seconds * 1000.,
        "maximum_ms":value.maximum_seconds * 1000.,
        "p95_upper_bound_ms":value.percentile(0.95).map(|v|v * 1000.),
        "p99_upper_bound_ms":value.percentile(0.99).map(|v|v * 1000.)})
}
fn phases(value: &Phases) -> serde_json::Value {
    serde_json::json!({"warmup_observations":Phases::WARMUP_OBSERVATIONS,
        "startup":timing(&value.startup),"steady":timing(&value.steady)})
}
fn consume(slot: &mut Slot, update: Update) -> Result<(), String> {
    match update {
        Update::Snapshot(response) => {
            slot.view.push_snapshot(&response)?;
            slot.snapshot_at = Some(Instant::now());
            slot.snapshots += 1;
        }
        Update::Inventory(response) => {
            slot.view.push_inventory(&response)?;
        }
        Update::Events { delivery, .. } => {
            slot.view.push_events(&delivery)?;
        }
        Update::Outcome(response) => {
            slot.outcomes += 1;
            if matches!(
                response.body,
                verse_world::service::wire::Reply::Refused { .. }
            ) {
                slot.refusals += 1;
            }
        }
        _ => {}
    }
    Ok(())
}
async fn run() -> Result<(), String> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 4 {
        return Err("Expected OUTPUT.json CLIENTS FRAMES CAPTURE_EVERY".into());
    }
    let clients: usize = arguments[1].parse().map_err(|_| "Invalid client count")?;
    let frames: u64 = arguments[2].parse().map_err(|_| "Invalid frame count")?;
    let capture_every: u64 = arguments[3]
        .parse()
        .map_err(|_| "Invalid capture cadence")?;
    if !(1..=3).contains(&clients) || !(180..=1200).contains(&frames) {
        return Err("Profile supports 1–3 clients and 180–1200 frames".into());
    }
    let assets = tempfile::tempdir().map_err(|e| e.to_string())?;
    let state = tempfile::tempdir().map_err(|e| e.to_string())?;
    let startup = Instant::now();
    let atlas = original::atlas()?;
    let pack = original::generate(assets.path())?;
    let mut scene = verse_engine::director::Scene::from_json(include_bytes!(
        "../../../assets/verse/original/ritual.json"
    ))?;
    scene.cut_at = 0.;
    let mut game = Game::combat_authored_in(scene.clone(), false, INSTANCE)?;
    game.tick(0., [0.; 2])?;
    let content = remote_content::identity(&pack, &scene, assets.path())?;
    let mut gateway = Gateway::new(Chamber::new(game)?)?.with_content(content)?;
    let secp = Secp256k1::new();
    let keys: Vec<_> = (0..clients)
        .map(|_| Keypair::new(&secp, &mut secp256k1::rand::rng()))
        .collect();
    for (index, key) in keys.iter().enumerate() {
        let public = key.x_only_public_key().0.serialize();
        if index == 0 {
            gateway.enroll_primary(public)?;
        } else {
            gateway.enroll_player(public, Vec3::new(index as f32 * 3., 0., -22.))?;
        }
    }
    let cert =
        rcgen::generate_simple_self_signed(vec!["localhost".into()]).map_err(|e| e.to_string())?;
    let der = cert.cert.der().clone();
    let private = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls = ServerConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_single_cert(vec![der.clone()], private.into())
        .map_err(|e| e.to_string())?;
    let mut roots = RootCertStore::empty();
    roots.add(der).map_err(|e| e.to_string())?;
    let client_tls = Arc::new(
        ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_root_certificates(roots)
            .with_no_client_auth(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let address = listener.local_addr().map_err(|e| e.to_string())?;
    let store = Store::open(&state.path().join("world"), content, INSTANCE)?;
    let (stop, stopping) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(net::serve_durable(
        listener,
        Arc::new(tls),
        gateway,
        store,
        async {
            let _ = stopping.await;
        },
    ));
    let static_instances = chamber::static_instances(&pack, scene.origin.into());
    let mut slots = Vec::new();
    for key in &keys {
        let deadline = Instant::now() + Duration::from_secs(5);
        let client = loop {
            match Client::connect_with_content(
                address,
                ServerName::try_from("localhost").unwrap(),
                client_tls.clone(),
                INSTANCE,
                Some(content),
                key,
            )
            .await
            {
                Ok(client) => break client,
                Err(message)
                    if message.starts_with("Chamber storage is busy")
                        && Instant::now() < deadline =>
                {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(message) => return Err(format!("Profile authentication failed: {message}")),
            }
        };
        let (input, inputs, updates, output) = worker::channels();
        let (stop, stopping) = tokio::sync::oneshot::channel();
        let observer = Observer::default();
        let task = tokio::spawn(worker::run_profiled(
            client,
            Cursor::new(INSTANCE),
            worker::NATIVE_CADENCE,
            inputs,
            updates,
            stopping,
            observer.clone(),
        ));
        let creation = Instant::now();
        let renderer = Renderer::new(
            pack.clone(),
            assets.path(),
            1280,
            720,
            &atlas,
            &static_instances,
        )?;
        let mut profile = FrameProfile::new(WARMUP);
        profile.record(
            0,
            "renderer_construction_cpu_ms",
            creation.elapsed().as_secs_f64() * 1000.,
        );
        slots.push(Slot {
            renderer,
            view: View::new(INSTANCE, 10., 0)?,
            input,
            output,
            observer,
            profile,
            capture_profile: FrameProfile::new(WARMUP),
            stop,
            task,
            snapshot_at: None,
            steady_at: None,
            snapshots: 0,
            outcomes: 0,
            refusals: 0,
        });
    }
    let startup_ms = startup.elapsed().as_secs_f64() * 1000.;
    for slot in &mut slots {
        while slot.view.replica().latest().is_none() {
            let update = tokio::time::timeout(Duration::from_secs(10), slot.output.recv())
                .await
                .map_err(|_| "Initial profile snapshot timed out")?
                .ok_or("Profile worker closed")?;
            consume(slot, update)?;
        }
    }
    let started = Instant::now();
    for frame in 1..=frames {
        let cycle_started = Instant::now();
        for (index, slot) in slots.iter_mut().enumerate() {
            let prepare = Instant::now();
            while let Ok(update) = slot.output.try_recv() {
                consume(slot, update)?;
            }
            if frame > WARMUP && slot.steady_at.is_none() {
                slot.steady_at = Some(Instant::now());
            }
            if frame % 4 == 0 {
                let axes = if (frame / 120) % 2 == 0 {
                    [0.25, 0.]
                } else {
                    [-0.25, 0.]
                };
                let _ = slot.input.try_send(Input::Command(Intent::Move {
                    axes,
                    yaw: std::f32::consts::PI,
                }));
            }
            let camera = Camera {
                eye: Vec3::new(8. + index as f32, 6., -10.),
                target: Vec3::new(0., 1., -24.),
                fov: 60.,
            };
            let rendered = chamber::remote_scene(
                &pack,
                &slot.view,
                1.,
                camera,
                scene.origin.into(),
                false,
                camera.target,
            )?
            .ok_or("Profile snapshot produced no scene")?;
            slot.profile.record(
                frame,
                "client_projection_cpu_ms",
                prepare.elapsed().as_secs_f64() * 1000.,
            );
            let view = verse::render::View {
                view_proj: Mat4::perspective_rh(
                    rendered.frame.fov.to_radians(),
                    16. / 9.,
                    0.1,
                    600.,
                ) * Mat4::look_at_rh(rendered.frame.eye, rendered.frame.target, Vec3::Y),
                eye: rendered.frame.eye,
            };
            slot.renderer.draw_live(
                view,
                &rendered.instances,
                &verse::ui::UiBatch::default(),
                &rendered.lighting,
            )?;
            let timing = slot.renderer.last_timings;
            if let Some(value) = timing.gpu_query_poll_cpu_ms {
                slot.profile.record(frame, "gpu_query_poll_cpu_ms", value);
            }
            for (name, value) in [
                ("renderer_prepare_cpu_ms", timing.prepare_ms),
                ("shadow_encode_cpu_ms", timing.shadow_encode_ms),
                ("world_encode_cpu_ms", timing.world_encode_ms),
                ("overlay_encode_cpu_ms", timing.overlay_encode_ms),
                ("command_finish_cpu_ms", timing.command_finish_ms),
                ("queue_submit_cpu_ms", timing.queue_submit_ms),
                ("scene_draw_cpu_ms", timing.total_ms),
                ("instances", timing.instances as f64),
                ("world_draws", timing.world_draws as f64),
                ("shadow_draws", timing.shadow_draws as f64),
            ] {
                slot.profile.record(frame, name, value);
            }
            for sample in timing.gpu_samples.into_iter().flatten() {
                for (name, value) in [
                    ("gpu_shadow_ms", sample.shadow_ms),
                    ("gpu_world_ms", sample.world_ms),
                    ("gpu_overlay_ms", sample.overlay_ms),
                    ("gpu_scene_ms", sample.total_ms),
                ] {
                    slot.profile.record(sample.frame, name, value);
                }
            }
            let observations = slot.observer.drain();
            for observed in observations.samples {
                let phase = if slot.steady_at.is_none_or(|end| observed.started_at < end) {
                    0
                } else {
                    frame
                };
                slot.profile
                    .record(phase, "request_turnaround_ms", observed.turnaround_ms);
                slot.profile.record(
                    frame,
                    "verified_to_consumer_ms",
                    observed.verified_at.elapsed().as_secs_f64() * 1000.,
                );
                slot.profile
                    .record(frame, "pending_requests", observed.pending_requests as f64);
                slot.profile
                    .record(frame, "input_channel_depth", observed.queued_inputs as f64);
                slot.profile.record(
                    frame,
                    "update_channel_depth",
                    observed.queued_updates as f64,
                );
            }
            if let Some(at) = observations.snapshot_verified_at {
                slot.profile.record(
                    frame,
                    "sdk_snapshot_age_ms",
                    at.elapsed().as_secs_f64() * 1000.,
                );
            }
            if let Some(at) = slot.snapshot_at {
                slot.profile.record(
                    frame,
                    "applied_snapshot_age_ms",
                    at.elapsed().as_secs_f64() * 1000.,
                );
            }
            if capture_every != 0 && frame % capture_every == 0 {
                let (pixels, timing) = slot.renderer.capture_submitted().finish_profiled()?;
                if pixels.len() != 1280 * 720 * 4 {
                    return Err("Profile capture size differs".into());
                }
                for (name, value) in [
                    ("copy_submit_cpu_ms", timing.copy_submit_cpu_ms),
                    ("fence_wait_cpu_ms", timing.fence_wait_cpu_ms),
                    ("row_copy_cpu_ms", timing.row_copy_cpu_ms),
                ] {
                    slot.capture_profile.record(timing.frame, name, value);
                }
            }
        }
        if let Some(delay) = Duration::from_secs_f64(1. / 60.).checked_sub(cycle_started.elapsed())
        {
            tokio::time::sleep(delay).await;
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    let mut records = Vec::new();
    for slot in slots {
        let _ = slot.stop.send(());
        slot.task
            .await
            .map_err(|e| e.to_string())?
            .map_err(|message| format!("Profile replication worker failed: {message}"))?;
        records.push(serde_json::json!({"device":slot.renderer.device_profile,"frames":frames,
            "snapshots":slot.snapshots,"command_outcomes":slot.outcomes,"command_refusals":slot.refusals,
            "gpu_health":slot.renderer.last_timings.gpu_health,"frame_measurements":slot.profile.summary(),
            "capture_measurements":slot.capture_profile.summary(),"telemetry_omitted":slot.observer.drain().omitted}));
    }
    let _ = stop.send(());
    let exit = server.await.map_err(|e| e.to_string())?;
    let report = serde_json::json!({"schema":"verse.frame-attribution.workload.v1","package_version":env!("CARGO_PKG_VERSION"),
        "debug_assertions":cfg!(debug_assertions),"wire_version":verse_world::service::wire::VERSION,
        "clients":clients,"resolution":[1280,720],"startup_ms":startup_ms,"workload_seconds":seconds,
        "pack_revision":pack.source_revision,"content":content,"capture_every":capture_every,
        "workload":"Original procedural chamber; concurrent TLS player workers and GPU devices share one process and adapter; no video encoder or display surface.",
        "client_records":records,"server_failure":exit.failure,"server":{"ticks":exit.stats.ticks,"dropped_seconds":exit.stats.dropped_seconds,
            "simulation":phases(&exit.stats.simulation_phases),"save_capture":phases(&exit.stats.capture_phases),"commits":phases(&exit.stats.commit_phases),
            "writer_queue_peak":exit.stats.writer_queue_peak,"storage_paused_seconds":exit.stats.storage_paused_seconds,
            "requests":exit.stats.requests,"replication":exit.stats.replication},
        "unavailable":{"surface_acquire_ms":null,"display_completion_ms":null,"input_to_display_ms":null,"isolated_network_rtt_ms":null,"video_encoder_cpu_ms":null},
        "limits":["Debug fixture and shared-process clients establish attribution, not AAA population or hardware performance acceptance.",
            "GPU pass timestamps exclude presentation and capture copies. Final pending timestamp slots are not synchronously drained.",
            "Request turnaround includes local queues, transport, server work, and validation. Snapshot ages begin at local verification or application, not the server clock.",
            "Server percentiles are histogram upper bounds with independent warm-up counters per stage."]});
    std::fs::write(
        &arguments[0],
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if exit.failure.is_some() {
        return Err("Scratch profile host failed; retained report contains the failure".into());
    }
    println!(
        "Profiled {clients} clients for {frames} frames; report {}",
        arguments[0]
    );
    Ok(())
}
fn main() -> Result<(), String> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?
        .block_on(run())
}
