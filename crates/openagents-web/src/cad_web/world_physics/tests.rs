use super::*;
use verse::zones::everglade::cad_physics::sync::{Blast, Forces};
const ROVER: &[u8] = include_bytes!("../../../../../assets/verse/cad/rover.glb");
fn scope(name: &str) -> Scope {
    Scope::personal(&crate::chat_store::account_owner(name)).unwrap()
}
async fn placed(store: &Store, scope: &Scope, n: u64) {
    super::super::handoff::place(
        store,
        scope,
        &json!({"at":[0.,0.,-28.]}),
        "rover",
        n,
        Some(ROVER.to_vec()),
    )
    .await
    .unwrap();
}
async fn host(store: &Store, scope: &Scope) -> Host {
    let (entries, index) = store
        .read_key(&scope.key("world/index.json").unwrap())
        .await
        .unwrap()
        .unwrap();
    let entries: Vec<Value> = serde_json::from_slice(&entries).unwrap();
    make(
        models(store, scope, "verse-everglade", &entries)
            .await
            .unwrap(),
        &scope.actor,
        None,
        None,
        Some(index),
        "verse-everglade",
    )
    .unwrap()
}
fn command(host: &Host, seq: u64, inputs: Inputs) -> Request {
    Request {
        zone: "verse-everglade".into(),
        client: "a".repeat(32),
        sequence: seq,
        base: host.saved.snapshot.revision,
        bindings: bindings(&host.models),
        inputs,
    }
}
#[tokio::test]
async fn authority_transfers_push_gravity_blast_and_reconnect_without_client_physics() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::local(root.path().join("book"));
    let scope = scope("cad-sync-physics");
    placed(&store, &scope, 1).await;
    let mut authority = host(&store, &scope).await;
    let mut one = host(&store, &scope).await;
    let mut two = host(&store, &scope).await;
    let id = authority.models[0].placement.key().to_owned();
    let before = authority.site.cad_report(&id).unwrap().positions;
    let mut clock = authority.saved.millis;
    for seq in 1..=20 {
        let at = glam::Vec3::from_array(
            authority.site.cad_report(&id).unwrap().positions[0].map(|x| x as f32),
        );
        let inputs = Inputs {
            forces: Forces {
                player: at - glam::Vec3::Y * 0.5,
                push: glam::Vec3::X,
                ..Default::default()
            },
            blasts: vec![],
        };
        let request = command(&authority, seq, inputs);
        clock += 50;
        authority.advance(Some(&request), clock).unwrap();
    }
    assert!(
        authority
            .site
            .cad_report(&id)
            .unwrap()
            .positions
            .iter()
            .zip(&before)
            .any(|(p, q)| (p[0] - q[0]).abs() > 0.02),
        "the avatar push must move the model"
    );
    let after_push = authority.site.cad_report(&id).unwrap().positions[0][1];
    let base = glam::DVec3::new(0., 0., -28.);
    for seq in 21..=80 {
        let inputs=Inputs {forces:Forces {gravity:Some(serde_json::from_value(json!({"cylinder":{"base":base,"radius":15.24,"height":30.48},"active":true})).unwrap()),..Default::default()},blasts:vec![]};
        let request = command(&authority, seq, inputs);
        clock += 50;
        authority.advance(Some(&request), clock).unwrap();
    }
    assert!(authority.site.cad_report(&id).unwrap().positions[0][1] > after_push + 1.);
    let center = glam::Vec3::from_array(
        authority.site.cad_report(&id).unwrap().positions[0].map(|x| x as f32),
    );
    let request = command(
        &authority,
        81,
        Inputs {
            blasts: vec![Blast {
                center,
                radius: 8.,
                speed: 68.,
            }],
            ..Default::default()
        },
    );
    clock += 50;
    authority.advance(Some(&request), clock).unwrap();
    assert!(authority.site.cad_report(&id).unwrap().broken > 0);
    for replica in [&mut one, &mut two] {
        let states = authority.site.cad_states();
        replica.site.apply_cad_states(&states, true).unwrap();
        let received = replica.site.cad_states();
        replica.site.tick(0.1);
        for (id, state) in replica.site.cad_states() {
            assert_eq!(state.bodies, received[&id].bodies);
            assert_eq!(state.broken, received[&id].broken);
        }
        for (id, state) in &received {
            assert_eq!(state.bodies, states[id].bodies);
            assert_eq!(state.broken, states[id].broken);
        }
    }
    let mut fresh = host(&store, &scope).await;
    fresh
        .site
        .apply_cad_states(&authority.site.cad_states(), true)
        .unwrap();
    for (id, state) in fresh.site.cad_states() {
        assert_eq!(state.bodies, one.site.cad_states()[&id].bodies);
        assert_eq!(state.broken, one.site.cad_states()[&id].broken);
    }
    let mut bad = authority.site.cad_states();
    bad.get_mut(&id).unwrap().bodies[0].rotation = glam::DQuat::from_xyzw(0., 0., 0., 0.);
    let old = one.site.cad_states();
    assert!(one.site.apply_cad_states(&bad, true).is_err());
    assert_eq!(old, one.site.cad_states());
}
#[tokio::test]
async fn persistence_replacement_removal_foreign_accounts_and_old_updates_are_checked() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::local(root.path().join("book"));
    let alice = scope("cad-sync-alice");
    let bob = scope("cad-sync-bob");
    placed(&store, &alice, 1).await;
    let initial = exchange(&store, &alice, "verse-everglade", None)
        .await
        .unwrap();
    assert_eq!(initial.objects.len(), 1);
    let mut request = Request {
        zone: "verse-everglade".into(),
        client: "b".repeat(32),
        sequence: 2,
        base: initial.revision,
        bindings: initial.objects.iter().map(|o| o.binding.clone()).collect(),
        inputs: Inputs::default(),
    };
    let saved = exchange(&store, &alice, &request.zone, Some(request.clone()))
        .await
        .unwrap();
    let again = exchange(&store, &alice, &request.zone, Some(request.clone()))
        .await
        .unwrap();
    assert!(again.revision > saved.revision);
    request.sequence = 1;
    assert_eq!(
        exchange(&store, &alice, &request.zone, Some(request.clone()))
            .await
            .unwrap_err()
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        exchange(&store, &bob, &request.zone, Some(request.clone()))
            .await
            .unwrap_err()
            .0,
        StatusCode::CONFLICT
    );
    let foreign = exchange(&store, &bob, &request.zone, None).await.unwrap();
    assert!(foreign.objects.is_empty());
    assert_ne!(foreign.account, saved.account);
    let reopened = Store::local(root.path().join("book"));
    hosts().lock().unwrap().clear();
    let restored = exchange(&reopened, &alice, &request.zone, None)
        .await
        .unwrap();
    assert_eq!(restored.objects.len(), 1);
    placed(&store, &alice, 2).await;
    request.sequence = 3;
    assert_eq!(
        exchange(&store, &alice, &request.zone, Some(request.clone()))
            .await
            .unwrap_err()
            .0,
        StatusCode::CONFLICT
    );
    let next = exchange(&store, &alice, &request.zone, None).await.unwrap();
    assert_eq!(next.objects[0].binding.version, 2);
    assert_ne!(
        next.objects[0].binding.generation,
        initial.objects[0].binding.generation
    );
    super::super::handoff::place(&store, &alice, &json!({"remove":true}), "rover", 2, None)
        .await
        .unwrap();
    assert!(
        exchange(&store, &alice, &request.zone, None)
            .await
            .unwrap()
            .objects
            .is_empty()
    );
}

/// Runs the actual website router with a local account-service stand-in.
async fn serve_fixture(
    root: &std::path::Path,
) -> (
    String,
    tokio::task::JoinHandle<()>,
    tokio::task::JoinHandle<()>,
) {
    use std::os::unix::fs::PermissionsExt;
    async fn auth(headers: HeaderMap) -> Response {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|s| s.to_str().ok())
            .unwrap_or("");
        let account = if token == format!("Bearer sess_{}", "a".repeat(64)) {
            "fixture-alice"
        } else if token == format!("Bearer sess_{}", "b".repeat(64)) {
            "fixture-bob"
        } else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        Json(json!({"session":{"id":format!("session-{account}"),"kind":"user","account":account,"created_at":now()/1000-1,"expires_at":now()/1000+3600,"state":"active"}})).into_response()
    }
    async fn account(headers: HeaderMap) -> Response {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|s| s.to_str().ok())
            .unwrap_or("");
        let account = if token == format!("Bearer sess_{}", "a".repeat(64)) {
            "fixture-alice"
        } else {
            "fixture-bob"
        };
        Json(json!({"account":{"id":account,"label":account},"workspaces":[]})).into_response()
    }
    std::fs::create_dir_all(root).unwrap();
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let auth_socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", auth_socket.local_addr().unwrap());
    let auth_server = tokio::spawn(async move {
        axum::serve(
            auth_socket,
            Router::new()
                .route("/v1/session", get(auth))
                .route("/v1/account", get(account)),
        )
        .await
        .unwrap();
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let secret = root.join("csrf.key");
    std::fs::write(&secret, [11; 32]).unwrap();
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).unwrap();
    let config_file = root.join("cloud.json");
    std::fs::write(&config_file,json!({"schema":"openagents.cloud.web-config.v1","public_origin":url,"account_service":endpoint,"csrf_secret":secret}).to_string()).unwrap();
    std::fs::set_permissions(&config_file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut config = crate::Config::development(root.join("tasks"));
    config.port = listener.local_addr().unwrap().port();
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&config_file).unwrap(),
    ));
    config.chat_store = Arc::new(Store::local(root.join("store")));
    if let Some(build) = std::env::var_os("CAD_SYNC_BROWSER_BUILD") {
        config.everglade = Some(PathBuf::from(build));
    }
    placed(&config.chat_store, &scope("fixture-alice"), 1).await;
    let server = tokio::spawn(async move {
        axum::serve(listener, crate::router(config)).await.unwrap();
    });
    (url, server, auth_server)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_requires_a_checked_account_and_does_not_open_another_accounts_objects() {
    let root = tempfile::tempdir().unwrap();
    let (url, server, auth) = serve_fixture(root.path()).await;
    let client = reqwest::Client::new();
    let endpoint = format!("{url}/cad-world/physics?zone=verse-everglade");
    assert_eq!(
        client.get(&endpoint).send().await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let alice = client
        .get(&endpoint)
        .header(
            header::COOKIE,
            format!("oa_cloud_session=sess_{}", "a".repeat(64)),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(alice.status(), StatusCode::OK);
    assert_eq!(alice.headers()[header::CACHE_CONTROL], "private, no-store");
    let snapshot: Snapshot = alice.json().await.unwrap();
    assert_eq!(snapshot.objects.len(), 1);
    let bob = client
        .get(&endpoint)
        .header(
            header::COOKIE,
            format!("oa_cloud_session=sess_{}", "b".repeat(64)),
        )
        .send()
        .await
        .unwrap();
    assert!(bob.json::<Snapshot>().await.unwrap().objects.is_empty());
    let request = Request {
        zone: "verse-everglade".into(),
        client: "d".repeat(32),
        sequence: 1,
        base: snapshot.revision,
        bindings: snapshot.objects.into_iter().map(|o| o.binding).collect(),
        inputs: Inputs::default(),
    };
    assert_eq!(
        client
            .post(format!("{url}/cad-world/physics"))
            .header(
                header::COOKIE,
                format!("oa_cloud_session=sess_{}", "b".repeat(64))
            )
            .json(&request)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    server.abort();
    auth.abort();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "isolated browser fixture; set CAD_SYNC_FIXTURE_DIR and CAD_SYNC_BROWSER_BUILD"]
async fn isolated_browser_fixture() {
    let root = PathBuf::from(std::env::var_os("CAD_SYNC_FIXTURE_DIR").expect("fixture folder"));
    let (url, server, auth) = serve_fixture(&root).await;
    std::fs::write(root.join("server.json"),json!({"url":url,"alice_cookie":format!("oa_cloud_session=sess_{}","a".repeat(64)),"bob_cookie":format!("oa_cloud_session=sess_{}","b".repeat(64))}).to_string()).unwrap();
    for _ in 0..3600 {
        if root.join("stop").exists() {
            break;
        }
        let control = root.join("control.json");
        if let Ok(bytes) = std::fs::read(&control) {
            let command: Value = serde_json::from_slice(&bytes).unwrap();
            let store = Store::local(root.join("store"));
            if command["remove"] == true {
                super::super::handoff::place(
                    &store,
                    &scope("fixture-alice"),
                    &json!({"remove":true}),
                    "rover",
                    2,
                    None,
                )
                .await
                .unwrap();
            } else {
                placed(
                    &store,
                    &scope("fixture-alice"),
                    command["version"].as_u64().unwrap_or(2),
                )
                .await;
            }
            std::fs::remove_file(control).unwrap();
            std::fs::write(root.join("control-done"), "done").unwrap();
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    server.abort();
    auth.abort();
}

#[tokio::test]
async fn an_acknowledged_blast_survives_restart_before_its_first_step() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::local(root.path().join("book"));
    let scope = scope("cad-blast-restart");
    placed(&store, &scope, 1).await;
    let mut original = host(&store, &scope).await;
    let time = original.saved.millis;
    let center = glam::Vec3::from_array(
        original
            .site
            .cad_report(original.models[0].placement.key())
            .unwrap()
            .positions[0]
            .map(|v| v as f32),
    );
    let request = command(
        &original,
        1,
        Inputs {
            blasts: vec![Blast {
                center,
                radius: 8.,
                speed: 68.,
            }],
            ..Default::default()
        },
    );
    original.advance(Some(&request), time).unwrap();
    let saved: Saved =
        serde_json::from_slice(&serde_json::to_vec(&original.saved).unwrap()).unwrap();
    let next = host(&store, &scope).await;
    let mut restored = make(
        next.models,
        &scope.actor,
        Some(saved),
        None,
        None,
        "verse-everglade",
    )
    .unwrap();
    restored.advance(None, time + 100).unwrap();
    assert!(
        restored
            .site
            .cad_report(restored.models[0].placement.key())
            .unwrap()
            .broken
            > 0
    );
    let mut changed = ROVER.to_vec();
    changed[0] = 0;
    assert!(
        super::super::handoff::place(&store, &scope, &json!({}), "rover", 1, Some(changed))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_slow_observer_does_not_cancel_another_clients_spell() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::local(root.path().join("book"));
    let scope = scope("cad-slow-observer");
    placed(&store, &scope, 1).await;
    let mut world = host(&store, &scope).await;
    let start = world.saved.millis;
    let inputs=Inputs {forces:Forces {gravity:Some(serde_json::from_value(json!({"cylinder":{"base":[0.,0.,-28.],"radius":15.24,"height":30.48},"active":true})).unwrap()),..Default::default()},..Default::default()};
    let caster = command(&world, 1, inputs);
    world.advance(Some(&caster), start + 50).unwrap();
    let mut observer = command(&world, 1, Inputs::default());
    observer.client = "e".repeat(32);
    world.advance(Some(&observer), start + 2050).unwrap();
    assert!(
        world
            .site
            .cad_report(world.models[0].placement.key())
            .unwrap()
            .velocities[0][1]
            > 1.
    );
}
