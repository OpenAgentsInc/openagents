use super::*;
use openagents_sdk::{runs::RunKind, sessions::SessionStatus};

fn beat() -> RunHeartbeat {
    RunHeartbeat {
        run: "scratch-run".into(),
        kind: RunKind::Terminal,
        harness: "any-harness".into(),
        harness_session: Some("session:123".into()),
        computer: "scratch-host".into(),
        issues: vec![12053],
        status: SessionStatus::Working,
        last_output_at: 1000,
        last_progress_at: Some(900),
        note: Some("Tests passed".into()),
        agent: Some("a".repeat(64)),
    }
}
#[test]
fn fake_clock_separates_output_progress_and_liveness() {
    let mut book = Book::default();
    let b = beat();
    assert_eq!(book.heartbeat(&b, 1000), Ok(true));
    let live = RunFilter {
        live: Some(true),
        issue: Some(12053),
    };
    assert_eq!(book.list(live, 1000 + LIVE_WINDOW_MS - 1), vec![b.clone()]);
    assert!(book.list(live, 1000 + LIVE_WINDOW_MS).is_empty());
    assert_eq!(
        book.list(
            RunFilter {
                live: Some(false),
                issue: None
            },
            1000 + LIVE_WINDOW_MS
        ),
        vec![b.clone()]
    );
    assert!(
        book.list(
            RunFilter {
                live: None,
                issue: Some(7)
            },
            1000
        )
        .is_empty()
    );
    // A live but silent harness can continue to report unchanged output times.
    assert_eq!(book.heartbeat(&b, 61_000), Ok(true));
    assert_eq!(book.list(live, 61_000), vec![b.clone()]);
    assert_eq!(book.end(&b.run, 62_000), Ok(true));
    assert_eq!(book.end(&b.run, 63_000), Ok(false));
    assert!(book.list(live, 63_000).is_empty());
    assert_eq!(book.heartbeat(&b, 64_000), Err(Refusal::Ended));
    assert_eq!(book.end("missing", 64_000), Err(Refusal::Missing));
    assert!(
        book.list(RunFilter::default(), 62_001 + RETENTION_MS)
            .is_empty()
    );
}
#[test]
fn retries_and_status_changes_obey_rolling_limits() {
    let mut book = Book::default();
    let mut b = beat();
    assert_eq!(book.heartbeat(&b, 1000), Ok(true));
    assert_eq!(book.heartbeat(&b, 2000), Ok(false));
    assert_eq!(book.runs[&b.run].received_at, 1000);
    b.last_output_at = 2000;
    assert_eq!(book.heartbeat(&b, 2000), Err(Refusal::Limited(59_000)));
    b.status = SessionStatus::Idle;
    assert_eq!(book.heartbeat(&b, 2000), Ok(true));
    for n in 2..12 {
        b.status = if n % 2 == 0 {
            SessionStatus::Working
        } else {
            SessionStatus::Idle
        };
        assert_eq!(book.heartbeat(&b, 2000 + n), Ok(true));
    }
    b.status = SessionStatus::Working;
    assert!(matches!(book.heartbeat(&b, 3000), Err(Refusal::Limited(_))));
    assert_eq!(book.heartbeat(&b, 63_000), Ok(true));
    let response = Refusal::Limited(1001).response();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()[header::RETRY_AFTER], "2");
}
#[test]
fn validates_times_identity_and_monotonic_progress() {
    for invalid in ["", "..", "a/b", "a?b", "a\n"] {
        assert!(!valid_id(invalid));
    }
    let mut book = Book::default();
    let b = beat();
    book.heartbeat(&b, 1000).unwrap();
    let mut bad = b.clone();
    bad.status = SessionStatus::Idle;
    assert_eq!(book.heartbeat(&bad, 999), Err(Refusal::Conflict));
    bad = b.clone();
    bad.last_output_at = 62_001;
    assert_eq!(book.heartbeat(&bad, 1000), Err(Refusal::Invalid));
    bad = b.clone();
    bad.last_output_at = 999;
    assert_eq!(book.heartbeat(&bad, 61_000), Err(Refusal::Conflict));
    bad = b.clone();
    bad.last_progress_at = None;
    assert_eq!(book.heartbeat(&bad, 61_000), Err(Refusal::Conflict));
    bad = b.clone();
    bad.agent = Some("b".repeat(64));
    assert_eq!(book.heartbeat(&bad, 61_000), Err(Refusal::Conflict));
    for note in [
        "n".repeat(200),
        "line\nbreak".into(),
        "line\u{2028}break".into(),
        "é".repeat(100),
    ] {
        bad = b.clone();
        bad.note = Some(note);
        assert!(!bad.valid());
    }
    bad = b.clone();
    bad.issues = vec![1, 1];
    assert!(!bad.valid());
    bad = b.clone();
    bad.status = SessionStatus::Other("unknown".into());
    assert!(!bad.valid());
    bad = b;
    bad.agent = Some("agent-name".into());
    assert!(!bad.valid());
}
#[test]
fn account_quota_and_storage_are_bounded_and_expire() {
    let mut book = Book::default();
    for n in 0..120 {
        let mut b = beat();
        b.run = format!("r-{n}");
        book.heartbeat(&b, 1000).unwrap();
    }
    let mut b = beat();
    b.run = "next".into();
    assert!(matches!(book.heartbeat(&b, 1000), Err(Refusal::Limited(_))));
    for n in 120..MAX_RUNS {
        b.run = format!("r-{n}");
        book.heartbeat(&b, 61_000 * (n as i64 / 100 + 1)).unwrap();
    }
    b.run = "full".into();
    assert_eq!(book.heartbeat(&b, 1_000_000), Err(Refusal::Full));
    assert_eq!(book.heartbeat(&b, RETENTION_MS + 1_000_001), Ok(true));
    assert_eq!(book.runs.len(), 1);
}
#[tokio::test]
async fn disk_restart_preserves_end_and_rate_limits_and_accounts_are_separate() {
    let root = tempfile::tempdir().unwrap();
    let owner = crate::chat_store::account_owner("acct_runs");
    let other = crate::chat_store::account_owner("acct_other");
    let b = beat();
    {
        let store = Store::local(root.path().to_path_buf());
        assert_eq!(
            change(&store, &owner, 1000, |book, t| book.heartbeat(&b, t))
                .await
                .unwrap(),
            Ok(())
        );
    }
    let store = Store::local(root.path().to_path_buf());
    assert!(
        load(&store, &other)
            .await
            .unwrap()
            .0
            .list(RunFilter::default(), 1000)
            .is_empty()
    );
    let mut next = b.clone();
    next.last_output_at += 1;
    assert!(matches!(
        change(&store, &owner, 2000, |book, t| book.heartbeat(&next, t))
            .await
            .unwrap(),
        Err(Refusal::Limited(_))
    ));
    assert_eq!(
        change(&store, &owner, 3000, |book, t| book.end(&b.run, t))
            .await
            .unwrap(),
        Ok(())
    );
    let store = Store::local(root.path().to_path_buf());
    assert_eq!(
        change(&store, &owner, 4000, |book, t| book.heartbeat(&b, t))
            .await
            .unwrap(),
        Err(Refusal::Ended)
    );
}
#[tokio::test]
async fn concurrent_heartbeats_do_not_lose_runs() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::local(root.path().to_path_buf());
    let owner = crate::chat_store::account_owner("acct_runs");
    let a = beat();
    let mut b = beat();
    b.run = "another".into();
    let (one, two) = tokio::join!(
        change(&store, &owner, 1000, |book, t| book.heartbeat(&a, t)),
        change(&store, &owner, 1000, |book, t| book.heartbeat(&b, t))
    );
    assert_eq!(one.unwrap(), Ok(()));
    assert_eq!(two.unwrap(), Ok(()));
    assert_eq!(load(&store, &owner).await.unwrap().0.runs.len(), 2);
}

#[cfg(unix)]
#[tokio::test]
async fn sdk_and_routes_cover_presence_filters_end_auth_and_rate_limits() {
    use axum::{Json, response::IntoResponse};
    use openagents_sdk::{CancellationToken, Client, Deployment, Error as SdkError};
    use std::{os::unix::fs::PermissionsExt, sync::Arc};
    async fn auth(headers: HeaderMap) -> Response {
        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        let id = if token == format!("Bearer sess_{}", "a".repeat(64)) {
            "run-api-alice"
        } else if token == format!("Bearer sess_{}", "b".repeat(64)) {
            "run-api-bob"
        } else {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error":{"code":"signed_out"}})),
            )
                .into_response();
        };
        Json(json!({"account":{"id":id,"label":id,"admin":false},"workspaces":[]})).into_response()
    }
    async fn session(headers: HeaderMap) -> Response {
        let response = auth(headers).await;
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        if status != StatusCode::OK {
            return (status, Json(value)).into_response();
        }
        Json(json!({"session":{"id":"fixture-session","account":value["account"]["id"],"kind":"user","state":"active","expires_at":crate::cloud::session::now()+3600}})).into_response()
    }
    async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        (origin, task)
    }
    let (auth_base, auth_task) = serve(
        Router::new()
            .route("/v1/account", get(auth))
            .route("/v1/session", get(session)),
    )
    .await;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let key = path.join("csrf");
    std::fs::write(&key, [42; 32]).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let config_path = path.join("cloud.json");
    std::fs::write(&config_path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":auth_base,"csrf_secret":key})).unwrap()).unwrap();
    std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let store = Arc::new(Store::local(path.join("chats")));
    let mut config = crate::Config::development(path.join("tasks"));
    config.chat_store = store.clone();
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&config_path).unwrap(),
    ));
    let app = App(Arc::new(crate::Inner { config }));
    let (base, task) = serve(routes().with_state(app).route("/v1/account", get(auth))).await;
    let client = Client::with_timeout(
        Deployment::new(&format!("{base}/v1"), &base).unwrap(),
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    let a_token = format!("sess_{}", "a".repeat(64));
    let a = client.authenticate(&a_token).await.unwrap();
    let b = client
        .authenticate(&format!("sess_{}", "b".repeat(64)))
        .await
        .unwrap();
    let runs = client.runs(&a).unwrap();
    let other = client.runs(&b).unwrap();
    let cancel = CancellationToken::new();
    let mut report = beat();
    runs.heartbeat(&report, &cancel).await.unwrap();
    runs.heartbeat(&report, &cancel).await.unwrap();
    let live = RunFilter {
        live: Some(true),
        issue: Some(12053),
    };
    assert_eq!(
        runs.list(live, &cancel).await.unwrap(),
        vec![report.clone()]
    );
    assert!(
        other
            .list(RunFilter::default(), &cancel)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        other.end(&report.run, &cancel).await,
        Err(SdkError::Refused { status: 404, .. })
    ));
    assert!(
        runs.list(
            RunFilter {
                live: Some(true),
                issue: Some(1)
            },
            &cancel
        )
        .await
        .unwrap()
        .is_empty()
    );
    report.last_output_at += 1;
    assert!(matches!(
        runs.heartbeat(&report, &cancel).await,
        Err(SdkError::RateLimited {
            retry_after: Some(_)
        })
    ));
    report.status = SessionStatus::AwaitingInput;
    runs.heartbeat(&report, &cancel).await.unwrap();
    let raw = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    let bad = raw
        .put(format!("{base}/v1/runs/wrong/heartbeat"))
        .bearer_auth(&a_token)
        .json(&report)
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status().as_u16(), 400);
    assert!(
        bad.headers()[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
    assert_eq!(
        raw.get(format!("{base}/v1/runs"))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        401
    );
    assert_eq!(
        raw.get(format!("{base}/v1/runs?live=maybe"))
            .bearer_auth(&a_token)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        400
    );
    assert_eq!(
        raw.put(format!("{base}/v1/runs/{}/heartbeat", report.run))
            .bearer_auth(&a_token)
            .body("x".repeat(MAX_BODY + 1))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        413
    );
    // A dead process cannot refresh its report. Expire receipt time on disk,
    // then verify the public route applies the same live filter after restart.
    let owner = crate::chat_store::account_owner("run-api-alice");
    let (mut book, generation) = load(&store, &owner).await.unwrap();
    book.runs.get_mut(&report.run).unwrap().received_at = now() - LIVE_WINDOW_MS;
    store
        .write_key(
            &Store::owner_key(&owner, KEY).unwrap(),
            serde_json::to_vec(&book).unwrap(),
            generation.as_deref(),
        )
        .await
        .unwrap();
    assert!(runs.list(live, &cancel).await.unwrap().is_empty());
    assert_eq!(
        runs.list(
            RunFilter {
                live: Some(false),
                issue: None
            },
            &cancel
        )
        .await
        .unwrap(),
        vec![report.clone()]
    );
    runs.end(&report.run, &cancel).await.unwrap();
    runs.end(&report.run, &cancel).await.unwrap();
    assert!(matches!(
        runs.heartbeat(&report, &cancel).await,
        Err(SdkError::Refused { status: 409, .. })
    ));
    // Another owner can use the same run ID without changing Alice's record.
    other.heartbeat(&report, &cancel).await.unwrap();
    assert_eq!(
        other.list(live, &cancel).await.unwrap(),
        vec![report.clone()]
    );
    assert!(runs.list(live, &cancel).await.unwrap().is_empty());
    let stopped = CancellationToken::new();
    stopped.cancel();
    assert!(matches!(
        runs.list(RunFilter::default(), &stopped).await,
        Err(SdkError::Cancelled)
    ));
    task.abort();
    auth_task.abort();
}

#[test]
fn openapi_declares_routes_security_filters_and_heartbeat_shape() {
    let doc = crate::app_openapi::document();
    let validator =
        jsonschema::validator_for(&doc["components"]["schemas"]["RunHeartbeat"]).unwrap();
    let report = serde_json::to_value(beat()).unwrap();
    assert!(validator.is_valid(&report));
    let mut invalid = report.clone();
    invalid["run"] = json!("..");
    assert!(!validator.is_valid(&invalid));
    invalid = report;
    invalid["agent"] = json!("agent-name");
    assert!(!validator.is_valid(&invalid));
    for (path, method) in [
        ("/v1/runs", "get"),
        ("/v1/runs/{id}/heartbeat", "put"),
        ("/v1/runs/{id}/end", "post"),
    ] {
        assert_eq!(
            doc["paths"][path][method]["security"],
            json!([{"appBearer":[]} ])
        );
    }
    assert_eq!(
        doc["paths"]["/v1/runs"]["get"]["parameters"][0]["name"],
        "live"
    );
    assert_eq!(
        doc["paths"]["/v1/runs"]["get"]["parameters"][1]["name"],
        "issue"
    );
    assert_eq!(
        doc["components"]["schemas"]["RunHeartbeat"]["additionalProperties"],
        false
    );
}
