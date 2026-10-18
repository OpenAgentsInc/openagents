use axum::{
    Json, Router,
    extract::{Path, Query},
    http::HeaderMap,
    routing::{get, post, put},
};
use openagents_sdk::{
    CancellationToken, Client, Deployment, Error,
    runs::{RunFilter, RunHeartbeat, RunKind},
    sessions::SessionStatus,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};

fn beat() -> RunHeartbeat {
    RunHeartbeat {
        run: "run:123".into(),
        kind: RunKind::CloudWork,
        harness: "forked-harness".into(),
        harness_session: None,
        computer: "worker".into(),
        issues: vec![12053],
        status: SessionStatus::Working,
        last_output_at: 1000,
        last_progress_at: Some(900),
        note: Some("Built successfully".into()),
        agent: Some("b".repeat(64)),
    }
}
#[tokio::test]
async fn typed_methods_send_auth_paths_filters_and_reports() {
    let report = beat();
    let sent = report.clone();
    let router = Router::new()
        .route(
            "/v1/account",
            get(|| async {
                Json(json!({"account":{"id":"acct_runs","label":"Runs"},"workspaces":[]}))
            }),
        )
        .route(
            "/v1/runs/{id}/heartbeat",
            put(
                move |headers: HeaderMap, Path(id): Path<String>, Json(body): Json<Value>| {
                    let sent = sent.clone();
                    async move {
                        assert_eq!(id, sent.run);
                        assert_eq!(
                            headers["authorization"],
                            format!("Bearer oa_sk_{}", "a".repeat(64))
                        );
                        assert_eq!(body, serde_json::to_value(sent).unwrap());
                        Json(json!({"ok":true}))
                    }
                },
            ),
        )
        .route(
            "/v1/runs",
            get(move |Query(q): Query<BTreeMap<String, String>>| {
                let report = report.clone();
                async move {
                    assert_eq!(q.get("live").map(String::as_str), Some("true"));
                    assert_eq!(q.get("issue").map(String::as_str), Some("12053"));
                    Json(json!({"runs":[report]}))
                }
            }),
        )
        .route(
            "/v1/runs/{id}/end",
            post(|Path(id): Path<String>| async move {
                assert_eq!(id, "run:123");
                Json(json!({"ok":true}))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::with_timeout(
        Deployment::new(&format!("{base}/v1"), &base).unwrap(),
        Duration::from_secs(5),
    )
    .unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "a".repeat(64)))
        .await
        .unwrap();
    let elsewhere =
        Client::new(Deployment::new("http://127.0.0.1:9/v1", "http://127.0.0.1:9").unwrap())
            .unwrap();
    assert!(matches!(
        elsewhere.runs(&session),
        Err(Error::WrongDeployment)
    ));
    let runs = client.runs(&session).unwrap();
    let cancel = CancellationToken::new();
    runs.heartbeat(&beat(), &cancel).await.unwrap();
    assert_eq!(
        runs.list(
            RunFilter {
                live: Some(true),
                issue: Some(12053)
            },
            &cancel
        )
        .await
        .unwrap(),
        vec![beat()]
    );
    runs.end("run:123", &cancel).await.unwrap();
    let mut bad = beat();
    bad.run = "../account".into();
    assert!(matches!(
        runs.heartbeat(&bad, &cancel).await,
        Err(Error::InvalidReference)
    ));
    assert!(matches!(
        runs.end("..", &cancel).await,
        Err(Error::InvalidReference)
    ));
    assert!(matches!(
        runs.list(
            RunFilter {
                live: None,
                issue: Some(0)
            },
            &cancel
        )
        .await,
        Err(Error::InvalidReference)
    ));
    let stopped = CancellationToken::new();
    stopped.cancel();
    assert!(matches!(
        runs.heartbeat(&beat(), &stopped).await,
        Err(Error::Cancelled)
    ));
    assert!(matches!(
        runs.end("run:123", &stopped).await,
        Err(Error::Cancelled)
    ));
    task.abort();
}
#[test]
fn reports_round_trip_all_kinds_statuses_and_optional_fields() {
    for kind in [
        RunKind::AppChat,
        RunKind::Terminal,
        RunKind::CloudWork,
        RunKind::Landing,
    ] {
        for status in [
            SessionStatus::Idle,
            SessionStatus::Working,
            SessionStatus::AwaitingInput,
            SessionStatus::Errored,
        ] {
            let mut b = beat();
            b.kind = kind;
            b.status = status;
            assert!(b.valid());
            assert_eq!(
                serde_json::from_value::<RunHeartbeat>(serde_json::to_value(&b).unwrap()).unwrap(),
                b
            );
        }
    }
    let minimal = json!({"run":"r","kind":"terminal","harness":"custom","computer":"c","status":"idle","last_output_at":0});
    let b: RunHeartbeat = serde_json::from_value(minimal).unwrap();
    assert!(b.valid());
    assert!(b.agent.is_none());
}
