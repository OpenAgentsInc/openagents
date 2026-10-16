use super::*;
use axum::{Router, response::IntoResponse, routing::any};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;

#[tokio::test]
async fn withdrawing_a_live_repository_grant_refuses_before_new_effects() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::AtomicBool;
    let (dir, app, run) = fixture().await;
    let revoked = Arc::new(AtomicBool::new(false));
    let standing = revoked.clone();
    let broker = Router::new().route(
        oa_auth::repos::broker::PATH,
        axum::routing::post(move |body: Bytes| {
            let revoked = standing.clone();
            async move {
                let form: BTreeMap<String, String> = url::form_urlencoded::parse(&body)
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect();
                assert_eq!(form["ticket"], "fixture-private-ticket");
                assert_eq!(form["path"], "fixture/repo.git");
                if revoked.load(Ordering::SeqCst) {
                    (StatusCode::FORBIDDEN, "error=revoked\n").into_response()
                } else {
                    (
                        StatusCode::OK,
                        "username=x-access-token\npassword=fixture-github-secret\n",
                    )
                        .into_response()
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let auth = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, broker).await.unwrap() });
    let key = dir.path().join("csrf");
    std::fs::write(&key, [9u8; 32]).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let path = dir.path().join("cloud.json");
    std::fs::write(&path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":auth,"csrf_secret":key})).unwrap()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut config = app.config.clone();
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&path).unwrap(),
    ));
    let app = App(Arc::new(crate::Inner { config }));
    let run = claim_for_backend(
        &app.config.chat_store,
        None,
        "shared",
        Some(&[Engine::Solver]),
        true,
    )
    .await
    .unwrap()
    .unwrap()
    .run;
    assert_eq!(&run.id[..3], "wrc");
    let secret = token(&app, &run);
    let create = run.computer.as_ref().unwrap().create.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let effects = calls.clone();
    let router = Router::new()
        .fallback(any(move || {
            effects.fetch_add(1, Ordering::SeqCst);
            async { axum::Json(json!({"id":"cmp_0123456789abcdef0123"})).into_response() }
        }))
        .layer(axum::middleware::from_fn_with_state(app.clone(), guard));
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/computers")
                .header(header::AUTHORIZATION, format!("Bearer {secret}"))
                .body(Body::from(serde_json::to_vec(&create).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let run = load_plain(&app.config.chat_store, &run.id)
        .await
        .unwrap()
        .unwrap();
    keep_token(&app, &run, "fixture-current-google-token")
        .await
        .unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", credential(&app, &run, "scv_"))
            .parse()
            .unwrap(),
    );
    let response = read_token(State(app.clone()), headers.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        &*axum::body::to_bytes(response.into_body(), 8192)
            .await
            .unwrap(),
        b"fixture-current-google-token"
    );
    revoked.store(true, Ordering::SeqCst);
    assert_eq!(
        read_token(State(app.clone()), headers).await.status(),
        StatusCode::FORBIDDEN
    );
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/computers/cmp_0123456789abcdef0123/wake")
                .header(header::AUTHORIZATION, format!("Bearer {secret}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let response = router
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/v1/computers/cmp_0123456789abcdef0123?kind=cloud")
                .header(header::AUTHORIZATION, format!("Bearer {secret}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    server.abort();
}

fn binding() -> Binding {
    let digest = "a".repeat(64);
    let pin:VersionPin=serde_json::from_value(json!({
        "environment":"env-test","version_id":"v1","number":1,"selection_revision":1,"recipe_revision":1,
        "recipe_digest":digest,"source":{"repository":"fixture/repo","revision":"b".repeat(40),"digest":digest},
        "base":{"provider":"boat","image_id":"fixture-image","digest":digest},
        "runtime":{"revision":"fixture-runtime","digest":digest},
        "image":{"provider":"boat","image_id":"fixture-image","manifest_digest":digest},
        "plan_digest":digest,"evidence_digest":digest
    })).unwrap();
    Binding {
        schema: "openagents.solver.computer.v1".into(),
        project: ProjectLink {
            workspace: "ws-test".into(),
            project: "project-test".into(),
        },
        branch: "main".into(),
        permission: format!("{:x}", Sha256::digest(b"fixture-private-ticket")),
        environment: pin,
        payer: "acct_test".into(),
        image: format!("fixture@sha256:{digest}"),
        create: json!({"environment":"env-test","version":"1","size":"large"}),
        computer: None,
        commands: BTreeMap::new(),
        command_ids: Vec::new(),
        cleanup_observed: false,
        vertex: None,
    }
}
async fn fixture() -> (tempfile::TempDir, App, WorkRun) {
    let dir = tempfile::tempdir().unwrap();
    let mut config = crate::Config::development(dir.path().join("tasks"));
    config.chat_store = Store::local(dir.path().join("chats")).into();
    config.agent_accounts = vec!["acct_test".into()];
    config.solver_keys = Some(Arc::new(
        oa_seal::Keyring::scratch("solver-test").unwrap().0,
    ));
    let app = App(std::sync::Arc::new(crate::Inner { config }));
    let requester = Requester {
        account: "acct_test".into(),
        workspace: "ws-test".into(),
        members_epoch: 7,
    };
    let ask = Ask {
        repo: "fixture/repo".into(),
        issue: 1,
        land: Land::Pr,
        engine: Engine::Solver,
        source: "api".into(),
        title: None,
        github: Some(Ticket::new("fixture-private-ticket".into())),
    };
    let run = submit_bound(
        &app.config.chat_store,
        &account_owner("acct_test"),
        requester,
        ask,
        Some(binding()),
    )
    .await
    .unwrap()
    .unwrap();
    (dir, app, run)
}

#[tokio::test]
async fn legacy_claims_cannot_take_public_jobs_and_claims_keep_repository_authority() {
    let (_dir, app, run) = fixture().await;
    assert!(
        claim_for(
            &app.config.chat_store,
            None,
            "legacy",
            Some(&[Engine::Solver])
        )
        .await
        .unwrap()
        .is_none()
    );
    let taken = claim_for_backend(
        &app.config.chat_store,
        None,
        "shared",
        Some(&[Engine::Solver]),
        true,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(taken.run.id, run.id);
    let saved = load_plain(&app.config.chat_store, &run.id)
        .await
        .unwrap()
        .unwrap();
    assert!(saved.github.is_some());
    assert_eq!(
        saved.computer.unwrap().create["idempotency_key"],
        format!("solver-{}", run.id)
    );
}

#[tokio::test]
async fn job_credentials_refuse_other_resources_and_allow_cleanup_after_revocation() {
    let (_dir, app, run) = fixture().await;
    claim_for_backend(
        &app.config.chat_store,
        None,
        "shared",
        Some(&[Engine::Solver]),
        true,
    )
    .await
    .unwrap();
    update_run(&app.config.chat_store, &run.id, |run| {
        let binding = run.computer.as_mut().unwrap();
        binding.computer = Some("cmp_0123456789abcdef0123".into());
        binding.command_ids.push("trn_0123456789abcdef0123".into());
        run.github = None;
        run.cancel = true;
        (true, ())
    })
    .await
    .unwrap();
    let run = load_plain(&app.config.chat_store, &run.id)
        .await
        .unwrap()
        .unwrap();
    let credential = token(&app, &run);
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let router = Router::new()
        .fallback(any(move || {
            seen.fetch_add(1, Ordering::SeqCst);
            async { StatusCode::NO_CONTENT.into_response() }
        }))
        .layer(axum::middleware::from_fn_with_state(app.clone(), guard));
    for (method, path, status) in [
        ("GET", "/v1/account", 403),
        ("GET", "/v1/computers", 403),
        ("GET", "/v1/computers/cmp_aaaaaaaaaaaaaaaaaaaa", 403),
        ("PUT", "/v1/computers/cmp_0123456789abcdef0123/files", 403),
        ("POST", "/v1/computers/cmp_0123456789abcdef0123/wake", 403),
        ("GET", "/v1/computers/cmp_0123456789abcdef0123", 204),
        (
            "POST",
            "/v1/computers/cmp_0123456789abcdef0123/commands/trn_0123456789abcdef0123/cancel",
            204,
        ),
        ("DELETE", "/v1/computers/cmp_0123456789abcdef0123", 403),
        (
            "DELETE",
            "/v1/computers/cmp_0123456789abcdef0123?kind=enrolled",
            403,
        ),
        (
            "DELETE",
            "/v1/computers/cmp_0123456789abcdef0123?kind=cloud",
            204,
        ),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, format!("Bearer {credential}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            router
                .clone()
                .oneshot(request)
                .await
                .unwrap()
                .status()
                .as_u16(),
            status,
            "{method} {path}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    let mut changed = run.clone();
    changed.requester.account = "acct_foreign".into();
    assert_ne!(credential, token(&app, &changed));
    changed = run.clone();
    changed.host = Some("another-host".into());
    assert_ne!(credential, token(&app, &changed));
    let shown = view(&run, 0).to_string();
    assert!(!shown.contains(&credential));
    assert!(!shown.contains("fixture-private-ticket"));
    assert_eq!(view(&run, 0)["computer"]["usage"]["status"], "unknown");
}

#[tokio::test]
async fn cancelling_an_unclaimed_job_needs_no_provider_and_leaves_no_cleanup_debt() {
    let (_dir, app, run) = fixture().await;
    cancel(&app.config.chat_store, &run.owner, &run.id)
        .await
        .unwrap();
    sweep(&app).await.unwrap();
    let saved = load_plain(&app.config.chat_store, &run.id)
        .await
        .unwrap()
        .unwrap();
    assert!(saved.computer.unwrap().cleanup_observed);
    let (bytes, _) = app
        .config
        .chat_store
        .read_key(PUBLIC_QUEUE_KEY)
        .await
        .unwrap()
        .unwrap();
    let queue: Queue = serde_json::from_slice(&bytes).unwrap();
    assert!(!queue.computer_jobs.contains(&run.id));
}

#[tokio::test]
async fn vertex_tokens_are_sealed_bound_to_the_job_and_removed_when_it_ends() {
    let (_dir, app, run) = fixture().await;
    let run = claim_for_backend(
        &app.config.chat_store,
        None,
        "shared",
        Some(&[Engine::Solver]),
        true,
    )
    .await
    .unwrap()
    .unwrap()
    .run;
    keep_token(&app, &run, "fixture-vertex-secret")
        .await
        .unwrap();
    let saved = load_plain(&app.config.chat_store, &run.id)
        .await
        .unwrap()
        .unwrap();
    let held = saved.computer.as_ref().unwrap().vertex.as_ref().unwrap();
    let keys = app.config.solver_keys.as_ref().unwrap();
    assert_eq!(
        &**keys.open(&token_aad(&saved), &held.value).unwrap(),
        b"fixture-vertex-secret"
    );
    let mut foreign = saved.clone();
    foreign.requester.account = "someone-else".into();
    assert!(keys.open(&token_aad(&foreign), &held.value).is_err());
    let (bytes, _) = app
        .config
        .chat_store
        .read_key(&run_key(&run.id))
        .await
        .unwrap()
        .unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("fixture-vertex-secret"));
    assert!(
        !claim(&app, &saved)
            .unwrap()
            .to_string()
            .contains("ciphertext")
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {}", credential(&app, &saved, "scv_"))
            .parse()
            .unwrap(),
    );
    assert!(resolve(&app, &headers).await.is_err());
    assert!(resolve_scope(&app, &headers, "scv_").await.is_ok());
    let mut ended = saved;
    ended.end(RunState::Done, None);
    assert!(ended.computer.unwrap().vertex.is_none());
}
#[test]
fn jobs_only_allow_their_saved_computer_and_commands() {
    let create = json!({"environment":"env-test","version":"1","idempotency_key":"solver-test","size":"large"});
    let commands = BTreeMap::new();
    let check = |method: &str, path: &str, body: &Value| {
        policy(
            &create,
            Some("cmp_0123456789abcdef0123"),
            &commands,
            &[],
            method,
            path,
            body,
        )
    };
    assert_eq!(
        check(
            "DELETE",
            "/v1/computers/cmp_0123456789abcdef0123",
            &Value::Null
        ),
        Ok(false)
    );
    for path in [
        "/v1/account",
        "/v1/computers",
        "/v1/computers/cmp_aaaaaaaaaaaaaaaaaaaa",
        "/v1/computers/cmp_0123456789abcdef0123/files",
    ] {
        assert!(check("GET", path, &Value::Null).is_err());
    }
    assert_eq!(check("POST", "/v1/computers", &create), Ok(true));
    let mut changed = create.clone();
    changed["size"] = json!("default");
    assert!(check("POST", "/v1/computers", &changed).is_err());
}
