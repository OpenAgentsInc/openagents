//! Exercises the standalone host executable with scratch content and credentials.
use std::{
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
use verse_world::service::{client::Client, host::Config};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn two_original_worlds_use_the_same_dedicated_host_and_authentication() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let key = secp256k1::Keypair::from_secret_key(
        &secp256k1::Secp256k1::new(),
        &secp256k1::SecretKey::from_byte_array([81; 32]).unwrap(),
    );
    let public = key.x_only_public_key().0.serialize();
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    std::fs::write(root.path().join("cert.der"), certificate.cert.der()).unwrap();
    std::fs::write(
        root.path().join("key.der"),
        certificate.signing_key.serialize_der(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.path().join("key.der"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    let mut roots = rustls::RootCertStore::empty();
    roots.add(certificate.cert.der().clone()).unwrap();
    let tls = Arc::new(
        rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth(),
    );
    let mut identities = Vec::new();
    for (index, recipe) in ["ritual", "observatory"].into_iter().enumerate() {
        let dir = root.path().join(recipe);
        let (pack, _, profile) = verse_content::compiler::worlds::compile(recipe, &dir).unwrap();
        assert!(pack.inventory.is_some());
        let address = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let instance = 8400 + index as u64;
        let mut json = serde_json::json!({"listen": address, "instance": instance,
            "scene": dir.join("scene.json"), "pack": dir.join("pack.json"),
            "certificate_der": root.path().join("cert.der"), "private_key_der": root.path().join("key.der"),
            "enrollments": [{"public_key": public.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), "role": {"type":"primary"}}],
            "social_profile": profile });
        if index == 0 {
            json["state_dir"] = serde_json::json!(dir.join("state"));
        }
        let path = dir.join("host.json");
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        let config = Config::from_json(&std::fs::read(&path).unwrap()).unwrap();
        let (gateway, _) = verse_host::prepare(&config).unwrap();
        use glam::{Mat4, Vec3};
        use verse_engine::{
            presentation::{Instance, View},
            render_world::RenderWorld,
            residency::Catalog,
        };
        let frame = gateway.game().frame();
        let instances: Vec<_> = frame
            .actors
            .iter()
            .filter(|actor| actor.visible)
            .map(|actor| Instance {
                mount: None,
                actor: actor.life,
                model: actor.actor.model.clone(),
                transform: Mat4::from_translation(actor.actor.position)
                    * Mat4::from_rotation_y(actor.actor.yaw)
                    * Mat4::from_scale(Vec3::splat(actor.actor.scale))
                    * verse_content::basis(),
                animation: actor.animation,
                time: actor.animation_time,
                animation_epoch: None,
                emission: Vec3::ONE,
            })
            .collect();
        let catalog = Catalog::new(&pack).unwrap();
        let lighting = verse_engine::lighting::Lighting::default();
        let render = RenderWorld::extract(
            &catalog,
            View {
                view_proj: Mat4::IDENTITY,
                eye: Vec3::ZERO,
            },
            &instances,
            &[],
            &lighting,
        )
        .unwrap();
        render.validate(&catalog).unwrap();
        assert!(!instances.is_empty());
        let identity = gateway.content().unwrap();
        identities.push(identity);
        let output = Command::new(env!("CARGO_BIN_EXE_verse-host"))
            .arg(&path)
            .args(["--check", "300"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let receipt: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(receipt["ticks"], 300);
        assert_eq!(receipt["content"], serde_json::json!(identity));
        println!(
            "{recipe} check: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        let log = std::fs::File::create(dir.join("host.log")).unwrap();
        let operations = dir.join("operations");
        let mut command = Command::new(env!("CARGO_BIN_EXE_verse-host"));
        command
            .arg(&path)
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log));
        #[cfg(unix)]
        if index == 0 {
            command.arg("--operations").arg(&operations);
        }
        let mut process = Process(command.spawn().unwrap());
        runtime.block_on(async {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut client = loop {
                assert!(
                    process.0.try_wait().unwrap().is_none(),
                    "Host exited before connection"
                );
                match Client::connect_with_content(
                    address,
                    rustls::pki_types::ServerName::try_from("localhost").unwrap(),
                    tls.clone(),
                    instance,
                    Some(identity),
                    &key,
                )
                .await
                {
                    Ok(client) => break client,
                    Err(error) => {
                        assert!(Instant::now() < deadline, "Host connection failed: {error}");
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                }
            };
            tokio::time::sleep(Duration::from_millis(125)).await;
            let state = client.snapshot().await.unwrap();
            assert!(client.tick() > 0);
            if recipe == "observatory" {
                let response = client
                    .social(verse_world::play::social::Action::Toggle { object: 2 })
                    .await
                    .unwrap();
                assert!(matches!(
                    response.body,
                    verse_world::service::wire::Reply::Accepted
                ));
                assert!(
                    client
                        .snapshot()
                        .await
                        .unwrap()
                        .social
                        .unwrap()
                        .switch_on(2)
                );
            }
            assert_eq!(client.instance(), instance);
            assert_eq!(state.social.is_some(), recipe == "observatory");
            println!(
                "{recipe} authenticated: instance {} with {} actors",
                instance,
                state.actors.len()
            );
            #[cfg(unix)]
            if index == 0 {
                let deadline = Instant::now() + Duration::from_secs(3);
                loop {
                    let output = Command::new(env!("CARGO_BIN_EXE_verse-host")).arg("--status").arg(&operations).output().unwrap();
                    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
                    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                    if status["snapshot"]["metrics"]["admission"]["active"].as_u64().unwrap() > 0 {
                        assert_eq!(status["availability"], "fresh");
                        assert_eq!(status["snapshot"]["build"]["wire_version"], serde_json::json!(verse_world::service::wire::VERSION));
                        println!("{}", serde_json::json!({"schema":"verse.host.operations.cli.live.v1", "status":status}));
                        break;
                    }
                    assert!(Instant::now() < deadline, "No live admitted client measurement");
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                let busy = Command::new(env!("CARGO_BIN_EXE_verse-host")).arg(&path).arg("--backup").arg(dir.join("busy-backup")).output().unwrap();
                assert!(!busy.status.success());
                assert!(String::from_utf8_lossy(&busy.stderr).contains("writer"));
                assert!(!dir.join("busy-backup").exists());
            }
            client.close().await.unwrap();
        });
        #[cfg(unix)]
        {
            if index == 0 {
                let output = Command::new(env!("CARGO_BIN_EXE_verse-host"))
                    .arg("--drain")
                    .arg(&operations)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(status["drain_requested"], true);
            } else {
                assert!(
                    Command::new("kill")
                        .args(["-TERM", &process.0.id().to_string()])
                        .status()
                        .unwrap()
                        .success()
                );
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(status) = process.0.try_wait().unwrap() {
                    assert!(status.success());
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "Host did not finish clean shutdown"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            println!("{}", std::fs::read_to_string(dir.join("host.log")).unwrap());
            if index == 0 {
                let status = Command::new(env!("CARGO_BIN_EXE_verse-host"))
                    .arg("--status")
                    .arg(&operations)
                    .output()
                    .unwrap();
                assert!(status.status.success());
                let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
                assert_eq!(status["availability"], "unavailable");
                assert_eq!(status["ready"], false);
                assert_eq!(status["snapshot"]["phase"], "stopped");
                let backup = dir.join("backup");
                let restored = dir.join("restored");
                for (mode, source, destination) in [
                    ("--backup", &backup, None),
                    ("--verify-backup", &backup, None),
                    ("--restore-backup", &backup, Some(&restored)),
                ] {
                    let mut command = Command::new(env!("CARGO_BIN_EXE_verse-host"));
                    command.arg(&path).arg(mode).arg(source);
                    if let Some(destination) = destination {
                        command.arg(destination);
                    }
                    let output = command.output().unwrap();
                    assert!(
                        output.status.success(),
                        "{mode}: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                    assert_eq!(report["instance"], instance);
                    println!(
                        "{}",
                        serde_json::json!({"schema":"verse.host.operations.cli.recovery.v1", "operation":mode, "report":report})
                    );
                }
                let prune = Command::new(env!("CARGO_BIN_EXE_verse-host"))
                    .arg(&path)
                    .arg("--prune-history")
                    .output()
                    .unwrap();
                assert!(
                    prune.status.success(),
                    "{}",
                    String::from_utf8_lossy(&prune.stderr)
                );
                let (source_gateway, _) = verse_host::prepare(&config).unwrap();
                let mut restore_config = config.clone();
                restore_config.state_dir = Some(restored);
                let (restored_gateway, _) = verse_host::prepare(&restore_config).unwrap();
                assert_eq!(
                    restored_gateway.game().authority_tick,
                    source_gateway.game().authority_tick
                );
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(
                        &restored_gateway.checkpoint().unwrap()
                    )
                    .unwrap(),
                    serde_json::from_slice::<serde_json::Value>(
                        &source_gateway.checkpoint().unwrap()
                    )
                    .unwrap()
                );
                println!(
                    "{}",
                    serde_json::json!({"schema":"verse.host.operations.cli.result.v1", "live_status":true, "active_writer_export_refused":true, "clean_drain":true, "offline_readiness_refused":true, "backup_verified":true, "restored_authority_tick":restored_gateway.game().authority_tick, "prune_passed":true})
                );
            }
        }
        let mut missing = serde_json::from_slice::<verse_engine::director::Scene>(
            &std::fs::read(&config.scene).unwrap(),
        )
        .unwrap();
        missing.actors[0].model = "missing-rig".into();
        assert!(
            verse_content::remote_content::admit(
                &pack,
                &missing,
                &dir,
                &config.outfits,
                &config.equipment
            )
            .is_err()
        );
        // Content admission must reject a changed runtime texture for both worlds.
        let texture = dir.join(&pack.textures[0].file);
        let mut bytes = std::fs::read(&texture).unwrap();
        bytes[0] ^= 1;
        std::fs::write(texture, bytes).unwrap();
        assert!(verse_host::prepare(&config).is_err());
    }
    assert_ne!(identities[0], identities[1]);
}

#[test]
fn an_authored_generation_loads_and_recovers_through_the_dedicated_host() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("ritual");
    verse_content::compiler::worlds::compile("ritual", &source).unwrap();
    let mut workspace = verse_content::authoring::Workspace::init(
        &source,
        &root.path().join("author"),
        "chamber-outpost".into(),
    )
    .unwrap();
    let tx = verse_content::authoring::parse::<verse_content::authoring::Transaction>(
        "outpost.transaction.json",
        include_bytes!("../../../assets/verse/authoring/chamber-outpost.transaction.json"),
        2 * 1024 * 1024,
    )
    .unwrap();
    workspace.transact(&tx).unwrap();
    let build = workspace.build().unwrap();
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(build.path.join("host-template.json")).unwrap())
            .unwrap();
    let key = secp256k1::Keypair::from_secret_key(
        &secp256k1::Secp256k1::new(),
        &secp256k1::SecretKey::from_byte_array([81; 32]).unwrap(),
    );
    json["enrollments"] = serde_json::json!([{"public_key":key.x_only_public_key().0.to_string(),"role":{"type":"primary"}}]);
    json["guests"] = serde_json::Value::Null;
    json["state_dir"] = serde_json::json!(root.path().join("state"));
    let config = Config::from_json(&serde_json::to_vec(&json).unwrap()).unwrap();
    let (mut gateway, mut store) = verse_host::prepare(&config).unwrap();
    assert_eq!(gateway.content(), Some(build.content));
    assert_eq!(gateway.quest_log(14)[0].giver, Some(100));
    let (id, challenge) = gateway.open(0).unwrap();
    let digest = challenge.signing_digest(key.x_only_public_key().0.serialize());
    let signature = secp256k1::Secp256k1::new()
        .sign_schnorr_no_aux_rand(&digest, &key)
        .to_byte_array();
    gateway
        .authenticate(id, 1, key.x_only_public_key().0.serialize(), signature)
        .unwrap();
    let admission = gateway.admission(id).unwrap();
    let giver = gateway.game().actor_life(100).unwrap();
    gateway
        .accept_quest(id, admission.actor(), admission.epoch(), 1, giver)
        .unwrap();
    for _ in 0..30 {
        gateway.tick(1. / 30.).unwrap();
    }
    assert!(gateway.quest_log(14)[0].accepted);
    store.as_mut().unwrap().commit(&mut gateway).unwrap();
    let mut expected: serde_json::Value =
        serde_json::from_slice(&gateway.checkpoint().unwrap()).unwrap();
    let mut world: serde_json::Value =
        serde_json::from_str(expected["world"].as_str().unwrap()).unwrap();
    world["world"]["admission"]["controller"] = serde_json::json!(0);
    world["world"]["admission"]["epoch"] = serde_json::json!(admission.epoch() + 1);
    expected["world"] = world;
    drop(store);
    let (recovered, store) = verse_host::prepare(&config).unwrap();
    assert_eq!(recovered.content(), Some(build.content));
    let mut actual: serde_json::Value =
        serde_json::from_slice(&recovered.checkpoint().unwrap()).unwrap();
    actual["world"] = serde_json::from_str(actual["world"].as_str().unwrap()).unwrap();
    assert_eq!(actual, expected);
    drop(store);
    let mut changed = config.clone();
    changed.progression.quests[0].experience += 1;
    assert!(verse_host::prepare(&changed).is_err());
    let path = root.path().join("host.json");
    json["state_dir"] = serde_json::Value::Null;
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_verse-host"))
        .arg(&path)
        .args(["--check", "60"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    println!("Authored host: {}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(admission.actor().actor, 14);
}
