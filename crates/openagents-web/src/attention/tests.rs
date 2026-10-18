use super::*;
use openagents_sdk::runs::RunHeartbeat;

const MIN: i64 = 60_000;

fn presence(run: &str, status: SessionStatus, output: i64, received: i64) -> RunPresence {
    RunPresence {
        beat: RunHeartbeat {
            run: run.into(),
            kind: RunKind::Terminal,
            harness: "any-harness".into(),
            harness_session: Some(format!("session:{run}")),
            computer: "scratch-host".into(),
            issues: vec![12055],
            status,
            last_output_at: output,
            last_progress_at: Some(output),
            note: Some("Which branch?".into()),
            agent: Some("a".repeat(64)),
        },
        received_at: received,
        ended_at: None,
    }
}

#[test]
fn heartbeats_without_new_output_raise_one_stall() {
    let start = 1_000_000_000;
    let mut log = Log::default();
    // Heartbeats keep arriving each minute, but output stays at `start`.
    for minute in 0..30 {
        let at = start + minute * MIN;
        let p = presence("run-a", SessionStatus::Working, start, at);
        log.sweep(&[p], &[], at);
    }
    let stalls: Vec<_> = log
        .events
        .iter()
        .filter(|e| matches!(e.attention, Attention::RunStalled { .. }))
        .collect();
    assert_eq!(stalls.len(), 1);
    assert_eq!(
        stalls[0].attention,
        Attention::RunStalled {
            run: "run-a".into(),
            since: start
        }
    );
    assert_eq!(stalls[0].at, start + 20 * MIN);
    assert_eq!(stalls[0].agent.as_deref(), Some("a".repeat(64).as_str()));
    // Output resumes, then stops again: a second, distinct stall.
    let later = start + 31 * MIN;
    log.sweep(
        &[presence("run-a", SessionStatus::Working, later, later)],
        &[],
        later,
    );
    let again = later + 20 * MIN;
    log.sweep(
        &[presence("run-a", SessionStatus::Working, later, again)],
        &[],
        again,
    );
    let count = log
        .events
        .iter()
        .filter(|e| matches!(e.attention, Attention::RunStalled { .. }))
        .count();
    assert_eq!(count, 2);
    // An ended run raises nothing.
    let mut ended = presence("run-b", SessionStatus::Working, start, start);
    ended.ended_at = Some(start);
    let mut fresh = Log::default();
    assert!(!fresh.sweep(&[ended], &[], start + 20 * MIN));
}

#[test]
fn sources_raise_ordered_events_once_and_resume_from_a_cursor() {
    let at = 2_000_000_000;
    let mut log = Log::default();
    let input = presence(
        "run-input",
        SessionStatus::AwaitingInput,
        at - 1000,
        at - 900,
    );
    let errored = presence("run-err", SessionStatus::Errored, at - 800, at - 800);
    let mut landing = presence("run-land", SessionStatus::Errored, at - 700, at - 700);
    landing.beat.kind = RunKind::Landing;
    let work = WorkFact {
        id: "work-1".into(),
        issue: 7,
        state: RunState::Failed,
        finished_at: at - 500,
        landing: Some("entry-9".into()),
        queued: true,
        why: Some("Tests failed".into()),
    };
    let old = WorkFact {
        id: "work-old".into(),
        finished_at: at - WINDOW_MS - 1,
        ..work.clone()
    };
    let runs = [errored, landing, input];
    assert!(log.sweep(&runs, &[work.clone(), old.clone()], at));
    let kinds: Vec<_> = log.events.iter().map(|e| e.attention.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            Attention::RunAwaitingInput {
                run: "run-input".into(),
                question: "Which branch?".into()
            },
            Attention::RunErrored {
                run: "run-err".into(),
                reason: "Which branch?".into()
            },
            Attention::LandingFailed {
                entry: "run-land".into(),
                issue: Some(12055),
                reason: "Which branch?".into()
            },
            Attention::LandingFailed {
                entry: "entry-9".into(),
                issue: Some(7),
                reason: "Tests failed".into()
            },
            Attention::WorkFinished {
                work: "work-1".into(),
                issue: 7,
                outcome: "failed".into()
            },
        ]
    );
    assert_eq!(
        log.events.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
    assert!(!log.sweep(&runs, &[work, old], at + 1000));
    assert_eq!(log.after(3).map(|e| e.seq).collect::<Vec<_>>(), vec![4, 5]);
    assert_eq!(log.after(5).count(), 0);
}

#[test]
fn claims_without_progress_raise_after_the_window_even_with_no_progress_time() {
    let at = 3_000_000_000;
    let mut log = Log::default();
    let mut p = presence("run-c", SessionStatus::Idle, at, at);
    p.beat.last_progress_at = None;
    log.sweep(std::slice::from_ref(&p), &[], at);
    p.received_at = at + 19 * MIN;
    p.beat.last_output_at = at + 19 * MIN;
    assert!(!log.sweep(std::slice::from_ref(&p), &[], at + 19 * MIN));
    p.received_at = at + 20 * MIN;
    assert!(log.sweep(std::slice::from_ref(&p), &[], at + 20 * MIN));
    assert_eq!(
        log.events[0].attention,
        Attention::ClaimNoProgress {
            issue: 12055,
            holder: "session:run-c".into(),
            since: at
        }
    );
}

#[test]
fn the_log_keeps_its_latest_events() {
    let mut log = Log::default();
    let at = 4_000_000_000;
    let work: Vec<_> = (0..MAX_EVENTS as u64 + 5)
        .map(|n| WorkFact {
            id: format!("w{n:05}"),
            issue: n + 1,
            state: RunState::Done,
            finished_at: at,
            landing: None,
            queued: false,
            why: None,
        })
        .collect();
    log.sweep(&[], &work, at);
    assert_eq!(log.events.len(), MAX_EVENTS);
    assert_eq!(log.events[0].seq, 6);
    assert_eq!(log.next, MAX_EVENTS as u64 + 5);
}

#[cfg(unix)]
#[tokio::test]
async fn route_streams_in_order_resumes_and_hides_other_accounts() {
    use axum::{Json, response::IntoResponse};
    use openagents_sdk::{CancellationToken, Client, Deployment};
    use serde_json::json;
    use std::{os::unix::fs::PermissionsExt, sync::Arc};
    async fn auth(headers: HeaderMap) -> Response {
        let token = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        let id = if token == format!("Bearer sess_{}", "a".repeat(64)) {
            "attention-alice"
        } else if token == format!("Bearer sess_{}", "b".repeat(64)) {
            "attention-bob"
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
    // Alice's log already holds two events; a sweep has just run.
    let alice = crate::chat_store::account_owner("attention-alice");
    let mut log = Log::default();
    for (n, run) in ["run-1", "run-2"].iter().enumerate() {
        log.raise(
            format!("x:{n}"),
            n as i64,
            None,
            Attention::RunErrored {
                run: (*run).into(),
                reason: String::new(),
            },
        );
    }
    log.swept_at = i64::MAX / 2;
    store
        .write_key(
            &Store::owner_key(&alice, KEY).unwrap(),
            serde_json::to_vec(&log).unwrap(),
            None,
        )
        .await
        .unwrap();
    let mut config = crate::Config::development(path.join("tasks"));
    config.chat_store = store.clone();
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&config_path).unwrap(),
    ));
    let app = App(Arc::new(crate::Inner { config }));
    let (base, task) = serve(routes().with_state(app).route("/v1/account", get(auth))).await;
    let client = Client::with_timeout(
        Deployment::new(&format!("{base}/v1"), &base).unwrap(),
        Duration::from_secs(5),
    )
    .unwrap();
    let a = client
        .authenticate(&format!("sess_{}", "a".repeat(64)))
        .await
        .unwrap();
    let b = client
        .authenticate(&format!("sess_{}", "b".repeat(64)))
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    let mut stream = client.attention(&a, 0, &cancel).await.unwrap();
    let first = stream.next().await.unwrap().unwrap();
    let second = stream.next().await.unwrap().unwrap();
    assert_eq!((first.seq, second.seq), (1, 2));
    assert_eq!(
        second.attention,
        Attention::RunErrored {
            run: "run-2".into(),
            reason: String::new()
        }
    );
    assert_eq!(stream.cursor(), 2);
    drop(stream);
    // Resume from the first event's cursor.
    let mut resumed = client.attention(&a, 1, &cancel).await.unwrap();
    assert_eq!(resumed.next().await.unwrap().unwrap().seq, 2);
    // The raw route answers an event stream with ids, and a Last-Event-ID resumes.
    let raw = reqwest::Client::new()
        .get(format!("{base}/v1/attention"))
        .bearer_auth(format!("sess_{}", "a".repeat(64)))
        .header("last-event-id", "1")
        .send()
        .await
        .unwrap();
    assert_eq!(raw.status(), StatusCode::OK);
    assert!(
        raw.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    let mut raw = raw;
    let chunk = raw.chunk().await.unwrap().unwrap();
    let text = String::from_utf8_lossy(&chunk);
    assert!(text.contains("event: attention"), "{text}");
    assert!(text.contains("id: 2"), "{text}");
    assert!(!text.contains("id: 1\n"), "{text}");
    // Bob sees nothing of Alice's.
    let mut other = client.attention(&b, 0, &cancel).await.unwrap();
    let nothing = tokio::time::timeout(Duration::from_millis(300), other.next()).await;
    assert!(nothing.is_err(), "another account must see no events");
    let bad = reqwest::Client::new()
        .get(format!("{base}/v1/attention?after=x"))
        .bearer_auth(format!("sess_{}", "a".repeat(64)))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
    let signed_out = reqwest::Client::new()
        .get(format!("{base}/v1/attention"))
        .send()
        .await
        .unwrap();
    assert!(signed_out.status().is_client_error());
    task.abort();
    auth_task.abort();
}

#[test]
fn openapi_declares_the_stream() {
    let doc = crate::app_openapi::document();
    let op = &doc["paths"]["/v1/attention"]["get"];
    assert_eq!(op["security"], serde_json::json!([{"appBearer":[]}]));
    assert_eq!(op["parameters"][0]["name"], "after");
    let validator =
        jsonschema::validator_for(&doc["components"]["schemas"]["AttentionEvent"]).unwrap();
    let event = AttentionEvent {
        seq: 1,
        at: 5,
        agent: Some("a".repeat(64)),
        attention: Attention::RunStalled {
            run: "r".into(),
            since: 1,
        },
    };
    assert!(validator.is_valid(&serde_json::to_value(&event).unwrap()));
    let mut named = serde_json::to_value(&event).unwrap();
    named["agent"] = serde_json::json!("agent-name");
    assert!(!validator.is_valid(&named));
}
