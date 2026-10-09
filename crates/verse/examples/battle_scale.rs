//! Isolated original-world capacity checks with the actual authority and TLS worker.
//! Usage: battle_scale authority|network|combined OUTPUT.json SECONDS
//! VERSE_BATTLE_RESOLUTION=2560x1440 selects the offscreen dimensions.
//! VERSE_BATTLE_PACK selects a local licensed pack; its files stay outside Git.
#[path = "common/battle_acceptance.rs"]
mod battle_acceptance;
#[path = "common/battle_native_player.rs"]
mod battle_native_player;
use glam::Vec3;
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
    imported::{Renderer, chamber, original},
    profiling::FrameProfile,
};
use verse_engine::{assets::Pack, director::Scene};
use verse_world::{
    Command, Intent,
    play::{Ability, Game},
    service::{
        Chamber,
        auth::Gateway,
        client::Client,
        net::{self, Phases, Timing},
        persistence::Store,
    },
};
const INSTANCE: u64 = 9300;
const PLAYERS: usize = 20;
const HOSTILES: usize = 40;
const WARMUP: u64 = 120;

const SEGMENT_RECORD_BYTES: usize = 16 * 1024 * 1024;
struct SegmentRecord {
    path: std::path::PathBuf,
    bytes: usize,
    digest: [u8; 32],
}
impl SegmentRecord {
    fn write(path: std::path::PathBuf, value: serde_json::Value) -> Result<Self, String> {
        use sha2::{Digest, Sha256};
        use std::io::Write;
        let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
        if bytes.len() > SEGMENT_RECORD_BYTES {
            return Err("Battle segment measurement record exceeds byte budget".into());
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            bytes: bytes.len(),
            digest: Sha256::digest(&bytes).into(),
        })
    }
    fn read(self) -> Result<serde_json::Value, String> {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let file = std::fs::File::open(&self.path).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take((SEGMENT_RECORD_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() != self.bytes || <[u8; 32]>::from(Sha256::digest(&bytes)) != self.digest {
            return Err("Battle segment measurement record changed before collection".into());
        }
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }
}
struct PlayerRecords {
    player: usize,
    error: Option<String>,
    segments: Vec<SegmentRecord>,
}
impl PlayerRecords {
    fn collect(self) -> Result<serde_json::Value, String> {
        let segments = self
            .segments
            .into_iter()
            .map(SegmentRecord::read)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(
            serde_json::json!({"player":self.player,"status":if self.error.is_none(){"complete"}else{"failed"},"error":self.error,"connections":segments.len(),"segments":segments}),
        )
    }
}

fn timing(value: &Timing) -> serde_json::Value {
    serde_json::json!({"count":value.count,"maximum_ms":value.maximum_seconds*1000.,
        "p95_upper_bound_ms":value.percentile(0.95).map(|v|v*1000.),
        "p99_upper_bound_ms":value.percentile(0.99).map(|v|v*1000.)})
}
fn phases(value: &Phases) -> serde_json::Value {
    serde_json::json!({"startup":timing(&value.startup),"steady":timing(&value.steady)})
}
fn hostile_count(game: &Game) -> usize {
    game.frame()
        .actors
        .iter()
        .filter(|a| a.health > 0 && !a.actor.friendly && a.actor.model != "adventurer")
        .count()
}
fn make_world(
    pack: &Pack,
    directory: &std::path::Path,
) -> Result<(Scene, Gateway, Vec<Keypair>), String> {
    let mut scene = Scene::from_json(include_bytes!("../../../assets/verse/original/ritual.json"))?;
    let mut cultists: Vec<_> = scene
        .actors
        .iter()
        .filter(|a| a.model == "cultist")
        .cloned()
        .collect();
    while cultists.len() < HOSTILES - 1 {
        let mut actor = cultists[0].clone();
        actor.id = 200 + (cultists.len() - 12) as u64;
        scene.actors.push(actor.clone());
        cultists.push(actor);
    }
    for (index, source) in cultists.iter().enumerate() {
        let actor = scene.actors.iter_mut().find(|a| a.id == source.id).unwrap();
        actor.position = Vec3::new(
            -4.5 + 3. * (index % 4) as f32,
            0.,
            -9. + 2. * (index / 4) as f32,
        );
        actor.health = 20000;
    }
    if pack.source_revision == "verse-bestiary-ritual-v1" {
        let claude = scene
            .actors
            .iter_mut()
            .find(|actor| actor.model == "claude")
            .unwrap();
        claude.scale = 6. / (pack.models["claude"].height * 0.9144);
    }
    scene.validate()?;
    let content = verse_content::remote_content::identity(pack, &scene, directory)?;
    let mut game = Game::combat_authored_in(scene.clone(), false, INSTANCE)?;
    verse_content::collision::admit_collision(pack, &mut game)?;
    game.enable_query_profiling();
    let mut gateway = Gateway::new(Chamber::new(game)?)?.with_content(content)?;
    let mut keys = Vec::new();
    let mut lives = Vec::new();
    for index in 0..PLAYERS {
        let key = Keypair::new(&Secp256k1::new(), &mut secp256k1::rand::rng());
        let public = key.x_only_public_key().0.serialize();
        let life = if index == 0 {
            gateway.enroll_primary(public)?;
            gateway.game().player_life()
        } else {
            let position = if index == PLAYERS - 1 {
                // Keep one participant exposed to hostile pursuit so the workload
                // exercises an actual defeat and the normal respawn request.
                Vec3::new(-6., 0., -8.)
            } else {
                Vec3::new(
                    -6. + 3. * ((index - 1) % 5) as f32,
                    0.,
                    -21. + 2. * ((index - 1) / 5) as f32,
                )
            };
            gateway.enroll_player(public, position)?
        };
        lives.push(life);
        keys.push(key);
    }
    use verse_world::service::{
        equipment, items, progression,
        rewards::{Entry, Transaction},
    };
    gateway = gateway
        .with_equipment(equipment::Catalog {
            version: 1,
            gear: vec![equipment::Gear {
                id: 501,
                name: "Capacity fixture hat".into(),
                slot: equipment::Slot::Head,
                model: "gear-hat".into(),
                offset: if pack.source_revision == "verse-bestiary-ritual-v1" {
                    [0; 3]
                } else {
                    [0, 0, 230]
                },
                health: 100,
                mana: 20,
            }],
        })?
        .with_items(items::Catalog {
            version: 1,
            items: vec![items::Item {
                id: 502,
                name: "Capacity fixture recovery".into(),
                health: 100,
                mana: 20,
            }],
        })?
        .with_progression(progression::Config {
            version: 1,
            levels: vec![0, 100, 1000],
            quests: vec![progression::Quest {
                repeatable: false,
                dialogue: None,
                giver: None,
                prerequisites: vec![],
                id: 1,
                name: "Capacity fixture objective".into(),
                objective: 1,
                goal: 1,
                experience: 100,
                items: vec![],
            }],
        })?;
    for (index, life) in lives.iter().enumerate() {
        let mut source = [0; 32];
        source[..8].copy_from_slice(b"VBENCH01");
        source[8..16].copy_from_slice(&(index as u64 + 1).to_be_bytes());
        gateway.grant_reward(Transaction {
            acceptance: None,
            instance: INSTANCE,
            actor: life.actor,
            source,
            experience: 1,
            items: vec![
                Entry { id: 501, count: 1 },
                Entry {
                    id: 502,
                    count: 1000,
                },
            ],
            quests: vec![Entry { id: 1, count: 1 }],
            spent: vec![],
            outfit: None,
            equipment: None,
        })?;
    }
    verse_content::remote_content::admit(
        pack,
        &scene,
        directory,
        gateway.outfits(),
        gateway.equipment(),
    )?;
    Ok((scene, gateway, keys))
}
struct Process(std::process::Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn rss_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/status").ok()?;
    text.lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")
                .and_then(|s| s.split_whitespace().next()?.parse::<u64>().ok())
        })
        .map(|kb| kb * 1024)
}
/// Bytes the allocator holds for live allocations, separate from resident pages
/// it keeps after frees (high water and fragmentation).
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn heap_in_use_bytes() -> Option<u64> {
    // SAFETY: mallinfo2 only reads allocator statistics.
    let info = unsafe { libc::mallinfo2() };
    Some((info.uordblks + info.hblkhd) as u64)
}
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn heap_in_use_bytes() -> Option<u64> {
    None
}
/// Twenty clients share this process's allocator, which a real client never
/// does: one that reconnects returns its freed session to its own process.
/// Without this, glibc keeps the freed pages of all twenty sessions resident
/// when they reconnect together at the midpoint, a 26 MiB step in RSS while
/// the allocator's in-use bytes stay flat (#10559).
fn trim_allocator() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: malloc_trim only returns free allocator pages to the system.
    unsafe {
        libc::malloc_trim(0);
    }
}
async fn connect(
    address: std::net::SocketAddr,
    tls: Arc<ClientConfig>,
    content: [u8; 32],
    key: &Keypair,
) -> Result<Client, String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match Client::connect_with_content(
            address,
            ServerName::try_from("localhost").unwrap(),
            tls.clone(),
            INSTANCE,
            Some(content),
            key,
        )
        .await
        {
            Ok(client) => return Ok(client),
            Err(error)
                if error.starts_with("Chamber storage is busy") && Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
            Err(error) => return Err(error),
        }
    }
}
fn fixture_pack(
    directory: &std::path::Path,
    report: &mut serde_json::Value,
) -> Result<Pack, String> {
    use sha2::{Digest, Sha256};
    let mut pack = original::generate(directory)?;
    let Some(path) = std::env::var_os("VERSE_BATTLE_PACK") else {
        report["asset_profile"] = serde_json::json!({"kind":"procedural"});
        return Ok(pack);
    };
    let path = std::path::PathBuf::from(path);
    let mut loaded = Pack::read(&path)?;
    let root = path
        .parent()
        .ok_or("Asset pack requires a parent directory")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let offset = pack.textures.len();
    let mut copied = 0u64;
    for (index, texture) in loaded.textures.iter_mut().enumerate() {
        let source = root
            .join(&texture.file)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !source.starts_with(&root) {
            return Err("Fixture texture escapes the selected asset directory".into());
        }
        let bytes = source.metadata().map_err(|e| e.to_string())?.len();
        copied = copied
            .checked_add(bytes)
            .ok_or("Fixture texture size overflow")?;
        if bytes > 64 * 1024 * 1024 || copied > 512 * 1024 * 1024 {
            return Err("Fixture texture copy exceeds its byte budget".into());
        }
        let name = format!("battle-licensed-{index}.png");
        std::fs::copy(&source, directory.join(&name)).map_err(|e| e.to_string())?;
        texture.file = name;
    }
    let mut derived_head_sockets = 0;
    for model in loaded.models.values_mut() {
        if !model
            .attachments
            .iter()
            .any(|attachment| attachment.id == 5)
        {
            if let Some(skin) = &model.skin {
                if let Some(bone) = skin.names.iter().position(|name| name == "Head") {
                    // Older compiled rigs predate the equipment socket. Derive
                    // it from retained rest transforms, as the current compiler does.
                    let mut globals = Vec::<glam::Mat4>::with_capacity(model.bones.len());
                    for (index, node) in model.bones.iter().enumerate() {
                        let rest = skin.rest[index];
                        let local = glam::Mat4::from_scale_rotation_translation(
                            rest.scale.into(),
                            glam::Quat::from_array(rest.rotation),
                            rest.translation.into(),
                        );
                        globals.push(if node.parent < 0 {
                            local
                        } else {
                            globals[node.parent as usize] * local
                        });
                    }
                    let position = (glam::Mat4::from_cols_array(&skin.basis) * globals[bone])
                        .transform_point3(Vec3::ZERO);
                    model.attachments.push(verse_engine::assets::Attachment {
                        id: 5,
                        bone,
                        position: position.to_array(),
                    });
                    derived_head_sockets += 1;
                }
            }
        }
        for surface in &mut model.surfaces {
            surface.texture += offset;
            for slot in [
                &mut surface.material.normal_texture,
                &mut surface.material.metallic_roughness_texture,
                &mut surface.material.occlusion_texture,
                &mut surface.material.emissive_texture,
            ] {
                if let Some(index) = slot {
                    *index += offset;
                }
            }
        }
    }
    report["asset_profile"] = serde_json::json!({
        "kind":"licensed_with_procedural_battle_equipment",
        "source_revision":loaded.source_revision,
        "manifest_sha256":format!("{:x}",Sha256::digest(std::fs::read(&path).map_err(|e|e.to_string())?)),
        "loaded_models":loaded.models.len(),"loaded_textures":loaded.textures.len(),
        "loaded_placements":loaded.placements.len(),"copied_texture_bytes":copied,
        "derived_head_sockets":derived_head_sockets
    });
    pack.source_revision = loaded.source_revision;
    pack.models.extend(loaded.models);
    pack.textures.extend(loaded.textures);
    pack.placements = loaded.placements;
    // The authority fixture supplies its own equipment catalog, including the
    // generated hat. The source pack's authoring inventory covers only its models.
    pack.inventory = None;
    pack.validate()?;
    Ok(pack)
}
async fn run(
    mode: &str,
    seconds: u32,
    resolution: (u32, u32),
    report: &mut serde_json::Value,
) -> Result<(), String> {
    let assets = tempfile::tempdir().map_err(|e| e.to_string())?;
    let state = tempfile::tempdir().map_err(|e| e.to_string())?;
    let pack = fixture_pack(assets.path(), report)?;
    let (scene, mut gateway, keys) = make_world(&pack, assets.path())?;
    let content = gateway.content().ok_or("Missing fixture content")?;
    report["content"] = serde_json::json!(content);
    report["phase"] = serde_json::json!("authority_setup");
    if mode == "authority" {
        let mut connections = Vec::new();
        for key in &keys {
            let public = key.x_only_public_key().0.serialize();
            let (id, challenge) = gateway.open(0)?;
            gateway.authenticate(
                id,
                0,
                public,
                Secp256k1::new()
                    .sign_schnorr_no_aux_rand(&challenge.signing_digest(public), key)
                    .to_byte_array(),
            )?;
            connections.push(id);
        }
        let mut profile = FrameProfile::new(WARMUP);
        let mut min_hostiles = HOSTILES;
        let mut accepted = 0u64;
        let mut refused = 0u64;
        let mut completed_ticks = 0u64;
        let mut battle_moves = vec![0u64; PLAYERS];
        let mut respawns = 0u64;
        let mut respawn_refusals = 0u64;
        let mut next_respawn = vec![0u64; PLAYERS];
        let workload = (|| {
            for tick in 1..=u64::from(seconds) * 30 {
                let started = Instant::now();
                for (index, id) in connections.iter().enumerate() {
                    let mut admission = gateway.admission(*id)?;
                    if gateway.game().player_snapshot(admission.actor())?.player.hp == 0 {
                        if tick < next_respawn[index] {
                            continue;
                        }
                        let request = verse_world::service::wire::Request {
                            version: verse_world::service::wire::VERSION,
                            request_id: tick * PLAYERS as u64 + index as u64 + 1,
                            body: verse_world::service::wire::Body::Respawn {
                                life: admission.actor().into(),
                            },
                        };
                        let bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
                        let response: verse_world::service::wire::Response =
                            serde_json::from_slice(&gateway.dispatch_json(
                                *id,
                                tick * 1000 / 30,
                                &bytes,
                            )?)
                            .map_err(|e| e.to_string())?;
                        match response.body {
                            verse_world::service::wire::Reply::Accepted => respawns += 1,
                            verse_world::service::wire::Reply::Refused { code, message }
                                if code == "command"
                                    && matches!(
                                        message.as_str(),
                                        "Character teleport endpoint is obstructed"
                                            | "Teleport destination is occupied"
                                    ) =>
                            {
                                respawn_refusals += 1;
                                next_respawn[index] = tick + 30;
                                continue;
                            }
                            verse_world::service::wire::Reply::Refused { message, .. } => {
                                return Err(message);
                            }
                            _ => return Err("Unexpected authority respawn outcome".into()),
                        }
                        admission = gateway.admission(*id)?;
                    }
                    let intent = if tick % 60 == index as u64 % 60 {
                        Intent::Cast {
                            ability: Ability::Shield,
                            target: None,
                            aim: [0., 0., 1.],
                        }
                    } else {
                        Intent::Move {
                            axes: [
                                if (tick / 120 + index as u64) % 2 == 0 {
                                    0.25
                                } else {
                                    -0.25
                                },
                                0.,
                            ],
                            yaw: std::f32::consts::PI,
                        }
                    };
                    let moving = matches!(intent, Intent::Move { .. });
                    let command = Command {
                        actor: admission.actor(),
                        epoch: admission.epoch(),
                        sequence: admission.accepted_sequence() + 1,
                        tick: gateway.game().authority_tick,
                        intent,
                    };
                    match gateway.submit(*id, command) {
                        Ok(()) => {
                            accepted += 1;
                            if moving && gateway.game().time >= 20. {
                                battle_moves[index] += 1;
                            }
                        }
                        Err(_) => refused += 1,
                    };
                }
                profile.record(
                    tick,
                    "command_dispatch_cpu_ms",
                    started.elapsed().as_secs_f64() * 1000.,
                );
                let started = Instant::now();
                gateway.tick(1. / 30.)?;
                profile.record(
                    tick,
                    "simulation_cpu_ms",
                    started.elapsed().as_secs_f64() * 1000.,
                );
                completed_ticks += 1;
                min_hostiles = min_hostiles.min(hostile_count(gateway.game()));
            }
            Ok::<_, String>(())
        })();
        report["authority"] = serde_json::json!({"ticks":completed_ticks,"accepted_battle_movement_per_player":battle_moves,"respawns":respawns,"respawn_refusals":respawn_refusals,"accepted_inputs":accepted,"refused_inputs":refused,"minimum_live_hostiles":min_hostiles,"measurements":profile.summary(),"query_profile":gateway.game().query_profile(),"motor_recovery":gateway.game().motor_recovery.blocks});
        return workload;
    }
    let content = gateway.content().unwrap();
    let certificate =
        rcgen::generate_simple_self_signed(vec!["localhost".into()]).map_err(|e| e.to_string())?;
    let der = certificate.cert.der().clone();
    let private = PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der());
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
    let destination = listener.local_addr().map_err(|e| e.to_string())?;
    let proxy_ready = state.path().join("proxy-ready.json");
    let proxy_receipt = state.path().join("proxy-receipt.json");
    let proxy_start = Instant::now();
    let mut proxy = Process(
        std::process::Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/bench/verse-delayed-route.py"
            ))
            .args([
                "--destination-port",
                &destination.port().to_string(),
                "--connections",
                "32",
                "--delay-profile",
                "pipeline",
                "--delay-ms",
                "40",
                "--jitter-ms",
                "20",
                "--seconds",
                &(seconds + 30).to_string(),
            ])
            .arg("--ready")
            .arg(&proxy_ready)
            .arg("--receipt")
            .arg(&proxy_receipt)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?,
    );
    while !proxy_ready.is_file() {
        if proxy.0.try_wait().map_err(|e| e.to_string())?.is_some()
            || proxy_start.elapsed() > Duration::from_secs(5)
        {
            return Err("Scratch delayed route did not start".into());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let route: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&proxy_ready).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let address: std::net::SocketAddr = route["address"]
        .as_str()
        .ok_or("Missing delayed route address")?
        .parse()
        .map_err(|_| "Invalid delayed route address")?;
    let mut store = Store::open(&state.path().join("world"), content, INSTANCE)?;
    // Initial storage preparation also attaches the bounded history before clients start.
    store.commit(&mut gateway)?;
    let (stop, stopping) = tokio::sync::oneshot::channel();
    let workload_window = Arc::new(std::sync::Mutex::new(None::<(Instant, Instant)>));
    let workload_ticks = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let tick_window = workload_window.clone();
    let tick_count = workload_ticks.clone();
    let tick: net::Tick = Box::new(move |_, _| {
        let now = Instant::now();
        if tick_window
            .lock()
            .unwrap()
            .is_some_and(|(start, end)| now >= start && now < end)
        {
            tick_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    });
    let server = tokio::spawn(net::serve_ticked(
        listener,
        Arc::new(tls),
        gateway,
        Some(store),
        tick,
        async {
            let _ = stopping.await;
        },
    ));
    let (tap, mut snapshots) = tokio::sync::mpsc::channel::<battle_native_player::Tap>(1);
    let atlas = Arc::new(if mode == "combined" {
        chamber::original_portrait_atlas(assets.path(), &pack)?
    } else {
        original::atlas()?
    });
    let mut renderer = if mode == "combined" {
        Some(Renderer::new(
            pack.clone(),
            assets.path(),
            resolution.0,
            resolution.1,
            &atlas,
            &chamber::static_instances(&pack, scene.origin.into()),
        )?)
    } else {
        None
    };
    let mut native_frame = None;
    let mut projection_ms = 0.;
    let mut profile = FrameProfile::new(WARMUP);
    let start_offset = proxy_start.elapsed().as_secs_f64();
    let began = tokio::time::Instant::now();
    let end = began + Duration::from_secs(u64::from(seconds));
    *workload_window.lock().unwrap() = Some((began.into_std(), end.into_std()));
    let segment_directory = state.path().join("measurement-segments");
    std::fs::create_dir(&segment_directory).map_err(|e| e.to_string())?;
    report["diagnostic_retention"] = serde_json::json!({"completed_segments":"scratch files collected after workload memory sampling", "segment_record_bytes":SEGMENT_RECORD_BYTES, "maximum_segment_records":PLAYERS*2, "integrity":"SHA-256 and exact byte length"});
    let mut tasks = Vec::new();
    for (index, key) in keys.into_iter().enumerate() {
        let tls = client_tls.clone();
        let segment_directory = segment_directory.clone();
        let scene = scene.clone();
        let pack = (index == 0).then(|| pack.clone());
        let tap = if index == 0 { Some(tap.clone()) } else { None };
        let atlas = if index == 0 {
            Some(atlas.clone())
        } else {
            None
        };
        tasks.push(tokio::spawn(async move {
            let mut segments = Vec::new();
            let midpoint = began + Duration::from_secs(u64::from(seconds) / 2);
            let mut error = None;
            for deadline in [midpoint, end] {
                tokio::time::sleep(Duration::from_millis(index as u64 * 125)).await;
                let client = match connect(address, tls.clone(), content, &key).await {
                    Ok(client) => client,
                    Err(message) => {
                        error = Some(message);
                        break;
                    }
                };
                let segment = battle_native_player::player(
                    client,
                    index,
                    deadline,
                    scene.clone(),
                    pack.clone(),
                    tap.clone(),
                    atlas.clone(),
                )
                .await?;
                let path = segment_directory
                    .join(format!("player-{index}-segment-{}.json", segments.len()));
                let written = SegmentRecord::write(path, segment);
                trim_allocator();
                match written {
                    Ok(record) => segments.push(record),
                    Err(message) => {
                        error = Some(message);
                        break;
                    }
                }
            }
            Ok::<_, String>(PlayerRecords {
                player: index,
                error,
                segments,
            })
        }));
    }
    drop(tap);
    let mut frame = 0u64;
    let mut verified = None;
    let mut rss = Vec::new();
    let mut window = FrameProfile::new(0);
    let mut windows = Vec::new();
    let mut window_frames = 0u64;
    report["phase"] = serde_json::json!("mixed_workload");
    let workload=async {
        let mut clock=tokio::time::interval(Duration::from_secs_f64(1./60.));clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        while tokio::time::Instant::now()<end {
            clock.tick().await;frame+=1;window_frames+=1;
            if window_frames>1800 {windows.push(serde_json::json!({"end_seconds":began.elapsed().as_secs_f64(),"measurements":window.summary()}));window=FrameProfile::new(0);window_frames=1;}
            while let Ok(snapshot)=snapshots.try_recv() {native_frame=Some(snapshot.frame);projection_ms=snapshot.projection_ms;verified=Some(snapshot.applied_at);}
            if frame%300==0 {rss.push(serde_json::json!({"seconds":began.elapsed().as_secs_f64(),"bytes":rss_bytes(),"heap_in_use_bytes":heap_in_use_bytes()}));}
            if let Some(at)=verified {let age=at.elapsed().as_secs_f64()*1000.;profile.record(frame,"applied_snapshot_age_ms",age);window.record(frame,"applied_snapshot_age_ms",age);}
            if let Some(renderer)=renderer.as_mut() {
                if let Some(scene)=native_frame.as_ref() {
                    renderer.draw_live(scene.view,&scene.instances,&scene.ui,&scene.lighting)?;
                    let timings=renderer.last_timings;
                    let roots=scene.instances.iter().filter(|i|i.actor.is_some()).count();
                    let mounts=scene.instances.iter().filter(|i|i.mount.is_some()).count();
                    if timings.actor_roots!=roots || timings.mounts!=mounts {
                        return Err("Required replicated visual was omitted".into());
                    }
                    for (name,value) in [("frame_cpu_ms",timings.total_ms+projection_ms),("draw_cpu_ms",timings.total_ms),("command_finish_cpu_ms",timings.command_finish_ms),("actor_roots",timings.actor_roots as f64),("mounts",timings.mounts as f64),("managed_geometry_bytes",renderer.resources.geometry_bytes as f64)] {profile.record(timings.frame,name,value);window.record(timings.frame,name,value);}
                    for sample in timings.gpu_samples.into_iter().flatten() {profile.record(sample.frame,"gpu_scene_ms",sample.total_ms);window.record(sample.frame,"gpu_scene_ms",sample.total_ms);}
                }
            }
        }
        Ok::<_,String>(())
    }.await;
    if window_frames > 0 {
        windows.push(serde_json::json!({"end_seconds":began.elapsed().as_secs_f64(),"measurements":window.summary()}));
    }
    report["render"] = serde_json::json!({"frames":frame,"rss":rss,"windows":windows,"measurements":profile.summary(),"failure":workload.as_ref().err()});
    if let Some(renderer) = renderer {
        report["device"] = serde_json::json!(renderer.device_profile);
        report["resources"] = serde_json::json!(renderer.resources);
        report["gpu_health"] = serde_json::json!(renderer.last_timings.gpu_health);
    }
    let mut players = Vec::new();
    for task in tasks {
        players.push(match task.await {
            Ok(Ok(record)) => record
                .collect()
                .unwrap_or_else(|error| serde_json::json!({"status":"failed","error":error})),
            Ok(Err(error)) => serde_json::json!({"status":"failed","error":error}),
            Err(error) => serde_json::json!({"status":"failed","error":error.to_string()}),
        });
    }
    report["players"] = serde_json::json!(players);
    let _ = stop.send(());
    let exit = server.await.map_err(|e| e.to_string())?;
    report["server"] = serde_json::json!({"failure":exit.failure,"ticks":exit.stats.ticks,"workload_ticks":workload_ticks.load(std::sync::atomic::Ordering::Relaxed),"dropped_seconds":exit.stats.dropped_seconds,"simulation":phases(&exit.stats.simulation_phases),"admission":exit.stats.admission,"movement_expiry":exit.gateway.game().movement_expiry,"capture":phases(&exit.stats.capture_phases),"commits":phases(&exit.stats.commit_phases),"commit_preparation":timing(&exit.stats.commit_preparation),"history_sync":timing(&exit.stats.history_sync),"history_slow_syncs":verse_world::service::persistence::slow_history_syncs(),"journal_encoding":timing(&exit.stats.journal_encoding),"journal_write":timing(&exit.stats.journal_write),"journal_sync":timing(&exit.stats.journal_sync),"snapshot_compaction":timing(&exit.stats.snapshot_compaction),"checkpoint_copy":timing(&exit.stats.checkpoint_copy),"read_projection":timing(&exit.stats.read_projection),"movement_queue_wait":timing(&exit.stats.movement_queue_wait),"read_queue_wait":timing(&exit.stats.read_queue_wait),"checkpoint_bytes":exit.stats.checkpoint_bytes,"checkpoint_commits":exit.stats.checkpoint_commits,"writer_queue_peak":exit.stats.writer_queue_peak,"request_queue_peak":exit.stats.request_queue_peak,"held_reply_bytes_peak":exit.stats.held_reply_bytes_peak,"storage_refusals":exit.stats.storage_refusals,"storage_paused_seconds":exit.stats.storage_paused_seconds,"requests":exit.stats.requests,"replication":exit.stats.replication,"motor_recovery_blocks":exit.gateway.game().motor_recovery.blocks,"motor_recovery_diagnostic":exit.gateway.game().motor_recovery.last_diagnostic,"minimum_final_live_hostiles":hostile_count(exit.gateway.game()),"navigation_plans":exit.gateway.game().navigation_plans,"navigation_budget_refusals":exit.gateway.game().navigation_budget_refusals,"navigation_work":exit.gateway.game().navigation_work(),"query_profile":exit.gateway.game().query_profile()});
    report["phase"] = serde_json::json!("durable_recovery");
    if !std::process::Command::new("kill")
        .args(["-TERM", &proxy.0.id().to_string()])
        .status()
        .map_err(|e| e.to_string())?
        .success()
    {
        return Err("Could not stop scratch delayed route".into());
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while proxy.0.try_wait().map_err(|e| e.to_string())?.is_none() {
        if Instant::now() > deadline {
            return Err("Scratch delayed route did not stop".into());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    report["route"] =
        serde_json::from_slice(&std::fs::read(&proxy_receipt).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    report["disconnect_windows_seconds"] = serde_json::json!([
        start_offset + f64::from(seconds) / 2.,
        start_offset + f64::from(seconds)
    ]);
    let checkpoint = exit.gateway.checkpoint()?;
    let recovered = Store::open(&state.path().join("world"), content, INSTANCE)?
        .recover()
        .ok_or("Missing final durable recovery")?;
    let saved: serde_json::Value =
        serde_json::from_slice(&checkpoint).map_err(|e| e.to_string())?;
    let mut characters = 0usize;
    for actor in exit
        .gateway
        .game()
        .frame()
        .actors
        .iter()
        .filter(|a| a.actor.model == "adventurer")
    {
        let life = actor.life.ok_or("Player has no durable life")?;
        if exit.gateway.character_rewards(life.actor) != recovered.character_rewards(life.actor) {
            return Err("Durable recovery changed owned character rewards".into());
        }
        characters += 1;
    }
    report["recovery"] = serde_json::json!({"characters_checked":characters,"active_receipts":saved["ledger"]["receipts"].as_array().map(|r|r.len()),"ledger_revision":saved["ledger"]["revision"],"retained_events":exit.gateway.game().events.len(),"checkpoint_bytes":checkpoint.len(),"restored_checkpoint_bytes":recovered.checkpoint()?.len(),"live_hostiles":hostile_count(recovered.game()),"actor_count":recovered.game().frame().actors.len()});
    workload?;
    if let Some(error) = exit.failure {
        return Err(error);
    }
    Ok(())
}
fn executable_digest() -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(std::env::current_exe().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut buffer = [0; 65536];
    let mut digest = Sha256::new();
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn resolution() -> Result<(u32, u32), String> {
    let value = std::env::var("VERSE_BATTLE_RESOLUTION").unwrap_or_else(|_| "1280x720".into());
    let (width, height) = value
        .split_once('x')
        .ok_or("Expected WIDTHxHEIGHT resolution")?;
    let width: u32 = width.parse().map_err(|_| "Invalid render width")?;
    let height: u32 = height.parse().map_err(|_| "Invalid render height")?;
    if !(64..=4096).contains(&width) || !(64..=4096).contains(&height) {
        return Err("Render dimensions must be between 64 and 4096".into());
    }
    Ok((width, height))
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Expected MODE OUTPUT.json SECONDS".into());
    }
    let seconds: u32 = args[2].parse().map_err(|_| "Invalid duration")?;
    if !matches!(args[0].as_str(), "authority" | "network" | "combined")
        || !(10..=3600).contains(&seconds)
    {
        return Err("Invalid capacity mode or duration".into());
    }
    let mut report = serde_json::json!({"schema":"verse.battle.capacity.v1","mode":args[0],"seconds":seconds,"players":PLAYERS,"native_sessions":if args[0]=="authority" {0} else {PLAYERS},"headless_workers":0,"unrendered_native_sessions":if args[0]=="authority" {0} else if args[0]=="combined" {PLAYERS-1} else {PLAYERS},"rendered_native_sessions":usize::from(args[0]=="combined"),"hostile_npcs":HOSTILES,"cultist_health":20000,"frontline_player":19,"frontline_initial_position":[-6.,0.,-8.],"frontline_initial_life_recipe":"approach the nearest hostile and cast only Fire Bolt; defer inventory, healing, crowd control, Shield, and Misty Step until the first actual respawn","delay_profile":"pipeline","delay_ms_per_chunk":40,"jitter_ms_per_chunk":20,"wire_version":verse_world::service::wire::VERSION,"debug_assertions":cfg!(debug_assertions),"status":"running","budgets":{"movement_expiries":0,"simulation_p99_upper_bound_ms":1000./30.,"render_cpu_p95_ms":1000./60.,"render_gpu_p95_ms":1000./60.,"snapshot_age_p95_ms":400.,"minimum_live_hostiles":HOSTILES,"minimum_authority_tick_fraction":0.98,"battle_framed_snapshot_fraction":0.8,"prediction_correction_p95_meters":0.25,"prediction_correction_maximum_meters":1.,"checkpoint_bytes":1048576,"request_queue_peak":128,"writer_queue_peak":2,"held_reply_bytes_peak":33554432,"steady_rss_growth_bytes":67108864},"limits":["Scratch authority and clients share this process and machine.","Offscreen rendering has no display surface, capture, or physical input-to-display measurement.","Authority mode reports simulated duration and in-process authenticated command dispatch; network modes use actual wall time, TLS workers, and durable storage.","GPU timestamp observations exclude display and final pending slots; bounded omissions are retained."]});
    let resolution = resolution()?;
    report["render_resolution"] = serde_json::json!({"width":resolution.0,"height":resolution.1});
    use sha2::{Digest, Sha256};
    report["implementation"] = serde_json::json!({
        "base_revision":option_env!("VERSE_BENCH_SOURCE_REVISION").unwrap_or("unrecorded"),
        "executable_sha256":executable_digest()?,
        "harness_sha256":format!("{:x}",Sha256::digest(include_bytes!("battle_scale.rs"))),
        "native_driver_sha256":format!("{:x}",Sha256::digest(include_bytes!("common/battle_native_player.rs"))),
        "acceptance_sha256":format!("{:x}",Sha256::digest(include_bytes!("common/battle_acceptance.rs"))),
        "world_net_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-world/src/service/net.rs"))),
        "worker_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-world/src/service/worker.rs"))),
        "wire_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-world/src/service/wire.rs"))),
        "prediction_local_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-world/src/prediction/local.rs"))),
        "prediction_frames_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-world/src/prediction/local_frames.rs"))),
        "authority_movement_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-world/src/play/framed_movement.rs"))),
        "authority_clock_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-world/src/movement/frames.rs"))),
        "authority_game_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-world/src/play.rs"))),
        "hostile_combat_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-world/src/combat.rs"))),
        "persistence_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-world/src/service/persistence.rs"))),
        "native_session_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../verse-imported/src/imported/chamber_session.rs"))),
        "cargo_lock_sha256":format!("{:x}",Sha256::digest(include_bytes!("../../../Cargo.lock"))),
        "delayed_route_sha256":format!("{:x}",Sha256::digest(std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),"/../../scripts/bench/verse-delayed-route.py")).map_err(|e|e.to_string())?))
    });
    report["machine"] = serde_json::json!({"architecture":std::env::consts::ARCH,"os":std::env::consts::OS,"cpu":std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|text|text.lines().find_map(|line|line.strip_prefix("model name").and_then(|s|s.split_once(':').map(|(_,s)|s.trim().to_string())))),"load_average_at_start":std::fs::read_to_string("/proc/loadavg").ok().map(|s|s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))});
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let result = runtime.block_on(run(&args[0], seconds, resolution, &mut report));
    report["status"] = serde_json::json!(if result.is_ok() { "complete" } else { "failed" });
    report["error"] = serde_json::json!(result.as_ref().err());
    let failures = battle_acceptance::failures(&report);
    report["acceptance"] = serde_json::json!(if failures.is_empty() {
        "passed"
    } else {
        "failed"
    });
    report["acceptance_failures"] = serde_json::json!(failures);
    std::fs::write(
        &args[1],
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    result?;
    if !failures.is_empty() {
        return Err("Capacity budgets failed; the report retains observations".into());
    }
    Ok(())
}
