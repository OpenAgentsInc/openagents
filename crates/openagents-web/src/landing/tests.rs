use super::*;
use coder::task::land_queue::{Dir, Queue};
use openagents_sdk::land::{LandEntry, LandState, LandStatus};

async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (origin, task)
}

fn submission() -> LandSubmit {
    LandSubmit {
        repo: "example/project".into(),
        branch: "work/change".into(),
        issue: Some(12038),
        summary: Some("Use the landing queue".into()),
    }
}

async fn queue(root: &std::path::Path) -> (LandingQueue, tokio::task::JoinHandle<()>) {
    let (github, task) = serve(Router::new().route(
        "/repos/example/project/git/ref/heads/work/change",
        get(|| async { axum::Json(json!({"object":{"sha":"a".repeat(40)}})) }),
    ))
    .await;
    let mut queue = LandingQueue::new("example/project", root.to_str().unwrap()).unwrap();
    queue.github = url::Url::parse(&github).unwrap();
    (queue, task)
}

#[tokio::test]
async fn cli_and_api_share_entries_attempts_and_worker_state() {
    let root = tempfile::tempdir().unwrap();
    let (queue, task) = queue(root.path()).await;
    let submitted = queue
        .submit("alice", "request-one", submission())
        .await
        .unwrap_or_else(|_| panic!("submit"));
    let entry: LandEntry = serde_json::from_value(submitted.clone()).unwrap();
    let disk = Dir(root.path().into());
    let cli = Queue { store: &disk };
    let mut change = cli.entry(&entry.id).unwrap().unwrap();
    assert_eq!(change.branch, "work/change");
    assert_eq!(change.head, "a".repeat(40));
    assert_eq!(change.issue, Some(12038));
    assert!(change.close);
    cli.beat(&Heartbeat {
        machine: "integrator".into(),
        at: land_queue::now(),
        ..Heartbeat::default()
    })
    .unwrap();
    let status: LandStatus =
        serde_json::from_value(queue.status().await.unwrap_or_else(|_| panic!("status"))).unwrap();
    assert!(status.integrator.running);
    assert_eq!(status.waiting[0].id, entry.id);
    assert!(matches!(
        queue.withdraw("bob", false, &entry.id).await,
        Err(Failure::Denied)
    ));

    // The integrator rewrites the normal CLI Entry shape, dropping API metadata.
    change.state = EntryState::Landed;
    change.commit = Some("b".repeat(40));
    cli.put(&change).unwrap();
    cli.record(&Record {
        entry: entry.id.clone(),
        number: 1,
        outcome: "landed".into(),
        ..Record::default()
    })
    .unwrap();
    let replay: LandEntry = serde_json::from_value(
        queue
            .submit("alice", "request-one", submission())
            .await
            .unwrap_or_else(|_| panic!("retry")),
    )
    .unwrap();
    assert_eq!(replay.state, LandState::Landed);
    assert_eq!(cli.entries().unwrap().len(), 1);
    let shown: LandEntry = serde_json::from_value(
        queue
            .show(&entry.id)
            .await
            .unwrap_or_else(|_| panic!("show")),
    )
    .unwrap();
    assert_eq!(shown.records[0].outcome, "landed");
    assert!(matches!(
        queue.withdraw("alice", true, &entry.id).await,
        Err(Failure::Conflict)
    ));
    let mut changed = submission();
    changed.summary = Some("Different inputs".into());
    assert!(matches!(
        queue.submit("alice", "request-one", changed).await,
        Err(Failure::Conflict)
    ));
    task.abort();
}

#[tokio::test]
async fn retries_recover_the_prepared_entry_and_status_includes_failed_changes() {
    let root = tempfile::tempdir().unwrap();
    let (queue, task) = queue(root.path()).await;
    let first = queue
        .submit("alice", "request-one", submission())
        .await
        .unwrap_or_else(|_| panic!("submit"));
    let id = first["id"].as_str().unwrap();
    std::fs::remove_file(
        root.path()
            .join(entry_key(id).unwrap_or_else(|_| panic!("valid fixture id"))),
    )
    .unwrap();
    assert_eq!(
        first,
        queue
            .submit("alice", "request-one", submission())
            .await
            .unwrap_or_else(|_| panic!("recover"))
    );
    let disk = Dir(root.path().into());
    let cli = Queue { store: &disk };
    for (id, state) in [
        ("waiting", EntryState::Queued),
        ("active", EntryState::Landing),
        ("failed", EntryState::Bounced),
        ("done", EntryState::Landed),
    ] {
        cli.submit(&Entry {
            id: id.into(),
            state,
            ..Entry::default()
        })
        .unwrap();
    }
    cli.beat(&Heartbeat {
        machine: "stopped".into(),
        at: 1,
        ..Heartbeat::default()
    })
    .unwrap();
    let status: LandStatus =
        serde_json::from_value(queue.status().await.unwrap_or_else(|_| panic!("status"))).unwrap();
    assert!(!status.integrator.running);
    assert_eq!(status.waiting.len(), 2);
    assert_eq!(status.landing[0].id, "active");
    assert_eq!(status.failed[0].id, "failed");
    queue
        .withdraw("alice", false, first["id"].as_str().unwrap())
        .await
        .unwrap_or_else(|_| panic!("withdraw"));
    queue
        .withdraw("alice", false, first["id"].as_str().unwrap())
        .await
        .unwrap_or_else(|_| panic!("retry withdraw"));
    assert_eq!(
        cli.entry(first["id"].as_str().unwrap())
            .unwrap()
            .unwrap()
            .state,
        EntryState::Withdrawn
    );
    for branch in ["../main", "-bad", "a//b", "a.lock", "a?b", "a#b"] {
        assert!(!valid_branch(branch), "{branch}");
    }
    let mut wrong_repo = submission();
    wrong_repo.repo = "other/repository".into();
    assert!(matches!(
        queue.submit("alice", "request-two", wrong_repo).await,
        Err(Failure::Invalid)
    ));
    assert!(entry_key("../worker").is_err());
    task.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn sdk_drives_authenticated_routes_against_the_fixture_queue() {
    use openagents_sdk::{CancellationToken, Client, Deployment};
    use std::os::unix::fs::PermissionsExt;
    async fn auth(headers: HeaderMap) -> Response {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        let id = if token == format!("Bearer sess_{}", "a".repeat(64)) {
            "alice"
        } else if token == format!("Bearer sess_{}", "b".repeat(64)) {
            "bob"
        } else if token == format!("Bearer sess_{}", "c".repeat(64)) {
            "carol"
        } else {
            return StatusCode::UNAUTHORIZED.into_response();
        };
        axum::Json(json!({"account":{"id":id,"label":id,"admin":false},"workspaces":[]}))
            .into_response()
    }
    async fn session(headers: HeaderMap) -> Response {
        let response = auth(headers).await;
        if response.status() != StatusCode::OK {
            return response;
        }
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        axum::Json(json!({"session":{"id":"fixture-session","account":value["account"]["id"],"kind":"user","state":"active","expires_at":land_queue::now()+3600}})).into_response()
    }
    let (accounts, auth_task) = serve(
        Router::new()
            .route("/v1/account", get(auth))
            .route("/v1/session", get(session)),
    )
    .await;
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path().canonicalize().unwrap();
    std::fs::set_permissions(&root_path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let key = root_path.join("csrf");
    std::fs::write(&key, [42; 32]).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let cloud = root_path.join("cloud.json");
    std::fs::write(&cloud, serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":base,"account_service":accounts,"csrf_secret":key})).unwrap()).unwrap();
    std::fs::set_permissions(&cloud, std::fs::Permissions::from_mode(0o600)).unwrap();
    let (queue, branch_task) = queue(&root_path.join("queue")).await;
    let mut config = crate::Config::development(root_path.join("tasks"));
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&cloud).unwrap(),
    ));
    config.agent_accounts = vec!["alice".into(), "bob".into()];
    config.landing = Some(Arc::new(queue));
    let app = App(Arc::new(crate::Inner { config }));
    let router = routes().route("/v1/account", get(auth)).with_state(app);
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{base}/v1"), &base).unwrap()).unwrap();
    let alice = client
        .authenticate(&format!("sess_{}", "a".repeat(64)))
        .await
        .unwrap();
    let bob = client
        .authenticate(&format!("sess_{}", "b".repeat(64)))
        .await
        .unwrap();
    let carol = client
        .authenticate(&format!("sess_{}", "c".repeat(64)))
        .await
        .unwrap();
    let land = client.land(&alice).unwrap();
    let cancel = CancellationToken::new();
    let entry = land
        .submit("request-one", submission(), &cancel)
        .await
        .unwrap();
    assert_eq!(
        entry,
        land.submit("request-one", submission(), &cancel)
            .await
            .unwrap()
    );
    assert_eq!(land.status(&cancel).await.unwrap().waiting[0].id, entry.id);
    assert_eq!(land.show(&entry.id, &cancel).await.unwrap(), entry);
    assert!(
        client
            .land(&bob)
            .unwrap()
            .withdraw(&entry.id, &cancel)
            .await
            .is_err()
    );
    assert!(client.land(&carol).unwrap().status(&cancel).await.is_err());
    land.withdraw(&entry.id, &cancel).await.unwrap();
    assert_eq!(
        land.show(&entry.id, &cancel).await.unwrap().state,
        LandState::Withdrawn
    );
    let http = reqwest::Client::new();
    assert!(crate::upstream::owned("/v1/land"));
    assert!(crate::upstream::owned("/api/v1/land/entry"));
    assert!(!owns("/v1/land-other"));
    assert_eq!(
        http.get(format!("{base}/api/v1/land"))
            .header(header::HOST, "api.example.test")
            .bearer_auth(alice.bearer_token())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        http.get(format!("{base}/v1/land"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        http.post(format!("{base}/v1/land"))
            .bearer_auth(alice.bearer_token())
            .json(&submission())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    server.abort();
    auth_task.abort();
    branch_task.abort();
}
