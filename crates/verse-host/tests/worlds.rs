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
        let json = serde_json::json!({"listen": address, "instance": instance,
            "scene": dir.join("scene.json"), "pack": dir.join("pack.json"),
            "certificate_der": root.path().join("cert.der"), "private_key_der": root.path().join("key.der"),
            "enrollments": [{"public_key": public.iter().map(|byte| format!("{byte:02x}")).collect::<String>(), "role": {"type":"primary"}}],
            "social_profile": profile });
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
        let mut process = Process(
            Command::new(env!("CARGO_BIN_EXE_verse-host"))
                .arg(&path)
                .stdout(Stdio::from(log.try_clone().unwrap()))
                .stderr(Stdio::from(log))
                .spawn()
                .unwrap(),
        );
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
            client.close().await.unwrap();
        });
        #[cfg(unix)]
        {
            assert!(
                Command::new("kill")
                    .args(["-TERM", &process.0.id().to_string()])
                    .status()
                    .unwrap()
                    .success()
            );
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
