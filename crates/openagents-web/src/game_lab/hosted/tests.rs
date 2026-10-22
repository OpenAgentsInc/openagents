use super::*;
use lab_store::backend::Backend;

struct AccountsFixture {
    app: App,
    root: tempfile::TempDir,
    server: tokio::task::JoinHandle<()>,
    revoked: Arc<std::sync::atomic::AtomicBool>,
    endpoint: String,
}
impl Drop for AccountsFixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn token(account: &str) -> String {
    format!(
        "sess_{}",
        if account == "alice" { "a" } else { "b" }.repeat(64)
    )
}

impl AccountsFixture {
    async fn new(port: u16) -> Self {
        use std::sync::atomic::{AtomicBool, Ordering};
        let revoked = Arc::new(AtomicBool::new(false));
        let flag = revoked.clone();
        let auth=Router::new().route("/v1/{resource}",get(move |headers:HeaderMap,Path(resource):Path<String>| {
            let flag=flag.clone();
            async move {
                let auth=headers.get("authorization").and_then(|h|h.to_str().ok()).unwrap_or_default();
                let account=if auth==format!("Bearer {}",token("alice")) {"alice"} else if auth==format!("Bearer {}",token("bob")) {"bob"} else { return (StatusCode::UNAUTHORIZED,Json(json!({"error":{"code":"signed_out","message":"Sign in to continue."}}))).into_response(); };
                if flag.load(Ordering::SeqCst) { return (StatusCode::UNAUTHORIZED,Json(json!({"error":{"code":"signed_out","message":"Sign in to continue."}}))).into_response(); }
                Json(if resource=="session" {json!({"session":{"id":format!("game-session-{account}"),"kind":"user","state":"active","account":account,"expires_at":4_000_000_000u64}})} else {
                    json!({"account":{"id":account,"label":account,"admin":false},"workspaces":[]})
                }).into_response()
            }
        }));
        let auth = auth.route("/{*path}", post(scripted_model));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, auth).await.unwrap();
        });
        let root = tempfile::tempdir().unwrap();
        // The cloud session refuses paths through a symlink, and macOS's
        // temporary directory is under /var, a link to /private/var.
        let private = root.path().canonicalize().unwrap().join("private");
        std::fs::create_dir(&private).unwrap();
        let key = private.join("csrf");
        std::fs::write(&key, [7; 32]).unwrap();
        let config_path = private.join("cloud.json");
        std::fs::write(&config_path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":format!("http://127.0.0.1:{port}"),"account_service":endpoint,"csrf_secret":key})).unwrap()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
            for p in [&key, &config_path] {
                std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600)).unwrap();
            }
        }
        let mut config = crate::Config::development(root.path().join("tasks"));
        config.port = port;
        config.cloud = Some(Arc::new(
            crate::cloud::session::CloudSession::load(&config_path).unwrap(),
        ));
        Self {
            app: App(Arc::new(crate::Inner { config })),
            root,
            server,
            revoked,
            endpoint,
        }
    }
    fn headers(&self, account: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            "authorization",
            format!("Bearer {}", token(account)).parse().unwrap(),
        );
        h
    }
}

#[tokio::test]
async fn hosted_routes_require_fresh_sign_in_and_cookie_writes_require_csrf() {
    use tower::ServiceExt;
    let f = AccountsFixture::new(4300).await;
    let router = routes().with_state(f.app.clone());
    let request = |method: &str, path: &str, bearer: bool| {
        let mut request = axum::http::Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:4300")
            .header("content-type", "application/json");
        if bearer {
            request = request.header("authorization", format!("Bearer {}", token("alice")));
        } else {
            request = request.header("cookie", format!("oa_cloud_session={}", token("alice")));
        }
        request
            .body(axum::body::Body::from(
                json!({"game":ICE_CREAM_MAN}).to_string(),
            ))
            .unwrap()
    };
    assert_eq!(
        router
            .clone()
            .oneshot(request("POST", "/labs/game/tools/game_open", true))
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request("POST", "/labs/game/tools/game_open", false))
            .await
            .unwrap()
            .status(),
        403
    );
    let alice = Hosted::for_request(&f.app, &f.headers("alice"))
        .await
        .unwrap_or_else(|_| panic!("alice signs in"));
    assert_eq!(
        alice.run("game_edit", &edit(4.0)).await.unwrap()["version"],
        2
    );
    let bob = Hosted::for_request(&f.app, &f.headers("bob"))
        .await
        .unwrap_or_else(|_| panic!("bob signs in"));
    assert!(matches!(
        bob.version(Some(2)).await,
        Err(lab_store::Error::NotFound)
    ));
    // The Grid homepage's GPUI panel reads the card as JSON (#12459).
    let response = router
        .clone()
        .oneshot(request("GET", "/labs/game/card.json?v=2&a=1", true))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let view: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(view["version"], 2);
    assert_eq!(view["summary"], "Slower chase");
    assert_eq!(view["can_compare"], true);
    assert!(
        view["play"]
            .as_str()
            .unwrap()
            .starts_with("/games/the-ice-cream-man#")
    );
    assert_eq!(view["compare"]["a"]["version"], 1);
    assert!(
        view["compare"]["a"]["preview"]
            .as_str()
            .unwrap()
            .ends_with("&bot&speed=2")
    );
    assert!(
        view["compare"]["changes"][0]
            .as_str()
            .unwrap()
            .starts_with("man.man_chase: ")
    );
    for line in [&view["stats"], &view["compare"]["b"]["stats"]] {
        assert!(oa_copy::violations(line.as_str().unwrap(), &[]).is_empty());
    }
    f.revoked.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        router
            .oneshot(request("GET", "/labs/game/card?v=2", true))
            .await
            .unwrap()
            .status(),
        401
    );
}

/// Runs the real web router with isolated, synthetic accounts for browser checks.
#[tokio::test]
#[ignore]
async fn browser_preview() {
    let assets = std::env::var("OA_GAME_PREVIEW_ASSETS").expect("set OA_GAME_PREVIEW_ASSETS");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut fixture = AccountsFixture::new(port).await;
    // This ignored check runs alone in its own process, with a scripted model.
    // All account and model requests stay on the loopback fixtures.
    unsafe {
        std::env::set_var("VERTEX_ACCESS_TOKEN", "local-game-preview");
        std::env::set_var("VERTEX_BASE_URL", &fixture.endpoint);
    }
    let config = &mut Arc::get_mut(&mut fixture.app.0).unwrap().config;
    config.everglade = Some(std::path::Path::new(&assets).join("everglade"));
    config.icecream = Some(std::path::Path::new(&assets).join("icecream"));
    config.pylons = Some(std::path::Path::new(&assets).join("pylons"));
    config.lander5 = Some(std::path::Path::new(&assets).join("lander5"));
    config.lander8 = Some(std::path::Path::new(&assets).join("lander8"));
    config.chat = Arc::new(NeverRemote);
    // The live pylons, for the Grid's Pylon Bay: read only (#12459).
    if let Ok(relay) = std::env::var("OA_GAME_PREVIEW_RELAY") {
        config.pylons_relay = Some(relay);
    }
    let app = crate::router(config.clone());
    eprintln!("Game Lab preview: http://127.0.0.1:{port}/?lab=game");
    eprintln!("Isolated data: {}", fixture.root.path().display());
    axum::serve(listener, app).await.unwrap();
}

fn hosted(root: &std::path::Path, account: &str) -> Hosted {
    let files = Arc::new(crate::chat_store::Store::local(root.join("chats")));
    let backend = Arc::new(storage::ChatBackend(files.clone()));
    Hosted {
        store: Store::new(
            backend,
            &SpaceRef::Personal {
                account: account.into(),
            },
        )
        .unwrap(),
        files,
        account: account.into(),
        owner: crate::chat_store::account_owner(account),
        admin: false,
        analytics: Arc::new(crate::analytics::Analytics::default()),
        count_events: true,
    }
}

fn edit(value: f64) -> Value {
    json!({"game":ICE_CREAM_MAN,"summary":"Slower chase","patch":[{"op":"replace","path":"/man/man_chase","value":value}]})
}

#[tokio::test]
async fn creators_get_separate_versions_and_persist_after_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let alice = hosted(dir.path(), "game-alice");
    let bob = hosted(dir.path(), "game-bob");
    assert_eq!(
        alice
            .run("game_open", &json!({"game":ICE_CREAM_MAN}))
            .await
            .unwrap()["version"],
        1
    );
    assert!(
        matches!(
            alice.store.document(ICE_CREAM_MAN).await,
            Err(lab_store::Error::NotFound)
        ),
        "reading the default creates nothing"
    );
    assert_eq!(
        alice.run("game_edit", &edit(4.0)).await.unwrap()["version"],
        2
    );
    assert_eq!(
        bob.run("game_edit", &edit(3.5)).await.unwrap()["version"],
        2
    );
    let alice = hosted(dir.path(), "game-alice");
    let a = alice.version(None).await.unwrap();
    let b = bob.version(None).await.unwrap();
    assert_eq!(a.content["man"]["man_chase"], 4.0);
    assert_eq!(b.content["man"]["man_chase"], 3.5);
    assert_ne!(a.content_sha256, b.content_sha256);
    assert_eq!(alice.versions().await.unwrap().len(), 2);
    let compare = alice
        .run("game_compare", &json!({"game":ICE_CREAM_MAN,"a":1}))
        .await
        .unwrap();
    assert_eq!(compare["card"]["compare"]["b"]["version"], 2);
    assert_eq!(compare["card"]["compare"]["a"]["version"], 1);
    assert_eq!(compare["card"]["compare"]["changes"][0][0], "man.man_chase");
    assert!(
        alice
            .run("game_open", &json!({"game":"../game-bob"}))
            .await
            .is_err()
    );
    assert!(alice.version(Some(3)).await.is_err());
    assert!(
        alice
            .run(
                "game_open",
                &json!({"game":ICE_CREAM_MAN,"version":"other"})
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn bad_edits_do_not_save_and_account_limits_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let alice = hosted(dir.path(), "game-limits");
    let bad = json!({"game":ICE_CREAM_MAN,"summary":"Lock door","patch":[{"op":"replace","path":"/rules/front_door","value":"locked"}]});
    assert!(alice.run("game_edit", &bad).await.is_err());
    assert_eq!(alice.versions().await.unwrap().len(), 1);
    assert!(
        alice
            .analytics
            .snapshot()
            .iter()
            .any(|r| r.name == "lab_edit_refused")
    );
    for _ in 0..RUNS_PER_HOUR {
        alice.allowance().await.unwrap();
    }
    let again = hosted(dir.path(), "game-limits");
    assert!(matches!(
        again.allowance().await,
        Err(lab_store::Error::RateLimited)
    ));
    assert!(hosted(dir.path(), "game-other").allowance().await.is_ok());
    let request = json!({"game":ICE_CREAM_MAN,"request":"Add a second floor"});
    let recorded = alice.run("game_propose_code", &request).await.unwrap();
    assert_eq!(recorded["saved"], false);
    assert_eq!(recorded["text"], "Recorded for the team.");
    let mut owner = hosted(dir.path(), "game-owner");
    owner.admin = true;
    assert_eq!(
        owner.run("game_propose_code", &request).await.unwrap()["saved"],
        true
    );
    assert_eq!(owner.versions().await.unwrap().len(), 1);
}

#[tokio::test]
async fn five_creators_can_check_edits_without_a_worker_or_compiler() {
    let dir = tempfile::tempdir().unwrap();
    let started = std::time::Instant::now();
    let mut tasks = vec![];
    for n in 0..5 {
        let h = hosted(dir.path(), &format!("game-load-{n}"));
        tasks.push(tokio::spawn(async move {
            h.run("game_edit", &edit(3.0 + n as f64 * 0.1)).await
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().unwrap()["version"], 2);
    }
    assert!(started.elapsed() < WAIT + DEADLINE);
    eprintln!(
        "Five Game Lab edits finished in {:.2}s",
        started.elapsed().as_secs_f64()
    );
}

#[tokio::test]
async fn storage_rejects_lost_updates_and_cross_space_objects() {
    let dir = tempfile::tempdir().unwrap();
    let b = storage::ChatBackend(Arc::new(crate::chat_store::Store::local(dir.path().into())));
    let a = "a".repeat(64);
    let other = "b".repeat(64);
    assert!(b.compare_and_swap(&a, 0, b"one".to_vec()).await.unwrap());
    assert!(!b.compare_and_swap(&a, 0, b"lost".to_vec()).await.unwrap());
    assert_eq!(b.load(&a).await.unwrap(), (1, Some(b"one".to_vec())));
    let bytes = b"private version".to_vec();
    let hash = crate::chat_store::digest(&bytes);
    b.put(&a, &hash, bytes.clone()).await.unwrap();
    assert!(matches!(
        b.get(&other, &hash, None).await,
        Err(lab_store::Error::NotFound)
    ));
    assert_eq!(b.get(&a, &hash, Some(0..7)).await.unwrap(), b"private");
}

struct NeverRemote;
impl crate::ask::Chat for NeverRemote {
    fn door(
        &self,
        _: secp256k1::SecretKey,
    ) -> Result<Box<dyn openagents_chat::basic_coder::Door>, String> {
        Ok(Box::new(Self))
    }
}
impl openagents_chat::basic_coder::Door for NeverRemote {
    fn ask(
        &self,
        _: Vec<openagents_chat::basic_coder::Turn>,
        _: openagents_chat::router::Context,
        reply: Arc<std::sync::Mutex<openagents_chat::basic_coder::Reply>>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        Box::pin(async move {
            openagents_chat::basic_coder::lock(&reply).failure =
                Some(openagents_chat::basic_coder::Failure::Transport(
                    "The local test model did not handle this turn.".into(),
                ));
        })
    }
}

/// Scripted tool choices for the browser acceptance check, never production.
async fn scripted_model(Json(body): Json<Value>) -> Json<Value> {
    let parts = body["contents"]
        .as_array()
        .and_then(|c| c.last())
        .and_then(|c| c["parts"].as_array())
        .cloned()
        .unwrap_or_default();
    let part = if parts.iter().any(|p| p["functionResponse"].is_object()) {
        json!({"text":"We checked your game. The result is ready to play and compare."})
    } else {
        let text = parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let (name, args) = if text.contains("slower") {
            ("game_edit", edit(4.0))
        } else if text.contains("compare") {
            ("game_compare", json!({"game":ICE_CREAM_MAN,"a":1}))
        } else if text.contains("play") {
            ("game_play", json!({"game":ICE_CREAM_MAN}))
        } else {
            ("game_open", json!({"game":ICE_CREAM_MAN}))
        };
        json!({"functionCall":{"name":name,"args":args}})
    };
    Json(json!({"candidates":[{"content":{"role":"model","parts":[part]},"finishReason":"STOP"}]}))
}

#[tokio::test]
async fn lab_events_respect_do_not_track_and_global_privacy_control() {
    let f = AccountsFixture::new(4300).await;
    for signal in ["dnt", "sec-gpc"] {
        let mut headers = f.headers("alice");
        headers.insert(signal, "1".parse().unwrap());
        let h = Hosted::for_request(&f.app, &headers)
            .await
            .unwrap_or_else(|_| panic!("synthetic sign-in"));
        assert!(!h.count_events);
        let before = h.analytics.snapshot().len();
        assert!(
            h.run("game_edit", &json!({"game":ICE_CREAM_MAN}))
                .await
                .is_err()
        );
        assert_eq!(h.analytics.snapshot().len(), before);
    }
}

#[tokio::test]
async fn a_tester_makes_one_change_to_their_own_house_and_an_admin_has_no_limit() {
    let dir = tempfile::tempdir().unwrap();
    let alice = hosted(dir.path(), "haunt-alice");
    let bob = hosted(dir.path(), "haunt-bob");
    let described = alice.run("haunt_describe", &json!({})).await.unwrap();
    assert_eq!(described["version"], 1);
    assert!(described["text"].as_str().unwrap().contains("long-hall"));
    // A refused edit costs nothing.
    let refused = alice
        .run(
            "haunt_place",
            &json!({"area": "long-hall", "asset": "dragon", "summary": "x"}),
        )
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("dragon"), "{refused}");
    // "Add a werewolf in the long hall that howls when you get near."
    let saved = alice
        .run(
            "haunt_place",
            &json!({"area": "long-hall", "asset": "werewolf", "summary": "A werewolf in the long hall that howls when you get near"}),
        )
        .await
        .unwrap();
    assert_eq!(saved["version"], 2);
    assert_eq!(saved["changes_left"], 0);
    assert_eq!(saved["card"]["doc"], game_lab::haunt::HAUNTED_HOUSE);
    let house = alice.haunt_version(None).await.unwrap().content;
    let hall = house["areas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "long-hall")
        .unwrap();
    assert!(
        hall["props"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["asset"] == "werewolf")
    );
    assert!(hall["triggers"].to_string().contains("howl-wolf"));
    // The Grid's panel asks for the newest card without naming a game: the
    // house, in a frame that follows it.
    let card = alice.card_of(None, None, None).await.unwrap();
    assert_eq!(card["doc"], game_lab::haunt::HAUNTED_HOUSE);
    let view = crate::game_lab::card_view(&card);
    assert_eq!(view["preview"], game_lab::haunt::PLAY_PATH);
    assert!(view["stats"].as_str().unwrap().contains("werewolf"));
    // Bob changed nothing, so his panel shows The Ice Cream Man.
    assert_eq!(
        bob.card_of(None, None, None).await.unwrap()["doc"],
        ICE_CREAM_MAN
    );
    // A second change is refused in a plain sentence.
    let spent = alice
        .run(
            "haunt_audio",
            &json!({"music": {"by": -0.25}, "summary": "Music down"}),
        )
        .await
        .unwrap_err();
    assert_eq!(spent.to_string(), super::haunt::SPENT);
    // Bob's house is his own.
    assert_eq!(bob.haunt_version(None).await.unwrap().number, 1);
    // An admin has no limit.
    let mut owner = hosted(dir.path(), "haunt-owner");
    owner.admin = true;
    for (i, by) in [-0.2, -0.1, 0.1].iter().enumerate() {
        let saved = owner
            .run(
                "haunt_audio",
                &json!({"music": {"by": by}, "summary": "Music"}),
            )
            .await
            .unwrap();
        assert_eq!(saved["version"], i as u64 + 2);
        assert_eq!(saved["changes_left"], Value::Null);
    }
}
