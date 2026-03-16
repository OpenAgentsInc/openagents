use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

use arc_client::{
    ArcCloseScorecardRequest, ArcCompatibilityServer, ArcEnvironmentInfo, ArcOpenScorecardRequest,
    ArcRegisteredEnvironment, ArcRemoteClient, ArcScorecardSummary, RemoteArcEnvironment,
};
use arc_core::{ArcAction, ArcGameState, ArcTaskId};
use serde_json::json;
use tokio::sync::oneshot;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("engine")
        .join("fixtures")
        .join(name)
}

fn registered_environment() -> ArcRegisteredEnvironment {
    ArcRegisteredEnvironment::new(
        ArcEnvironmentInfo {
            game_id: ArcTaskId::new("bt11-fd9df0622a1a").expect("task id should validate"),
            title: Some("BT11".to_owned()),
            tags: vec!["benchmark".to_owned()],
            private_tags: Vec::new(),
            level_tags: Vec::new(),
            baseline_actions: vec![4],
            class_name: Some("BT11".to_owned()),
            local_package_path: None,
        },
        fixture_path("upstream/bt11-fd9df0622a1a.json"),
    )
}

struct TestServerHandle {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for TestServerHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().expect("server thread should join cleanly");
        }
    }
}

fn spawn_server() -> TestServerHandle {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should expose a bound address");
    listener
        .set_nonblocking(true)
        .expect("listener should become non-blocking");
    let server = ArcCompatibilityServer::new(vec![registered_environment()]);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let thread = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime should initialize");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener)
                .expect("tokio listener should initialize");
            axum::serve(listener, server.router())
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("server should exit cleanly");
        });
    });

    TestServerHandle {
        base_url: format!("http://{addr}"),
        shutdown: Some(shutdown_tx),
        thread: Some(thread),
    }
}

#[test]
fn compatibility_server_supports_local_docs_flow_without_authoritative_scoring() {
    let server = spawn_server();
    let raw_http = reqwest::blocking::Client::builder()
        .cookie_store(true)
        .build()
        .expect("raw HTTP client should initialize");

    let health = raw_http
        .get(format!("{}/api/healthcheck", server.base_url))
        .send()
        .expect("healthcheck should respond");
    assert_eq!(health.status(), reqwest::StatusCode::OK);
    assert_eq!(health.text().expect("health body should decode"), "okay");

    let games_response = raw_http
        .get(format!("{}/api/games", server.base_url))
        .send()
        .expect("local game listing should not require an API key");
    assert_eq!(games_response.status(), reqwest::StatusCode::OK);
    let games = games_response
        .json::<Vec<ArcEnvironmentInfo>>()
        .expect("game list should decode");
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].game_id.as_str(), "bt11-fd9df0622a1a");

    let game_info = raw_http
        .get(format!(
            "{}/api/games/{}",
            server.base_url, games[0].game_id
        ))
        .send()
        .expect("game lookup should respond")
        .json::<ArcEnvironmentInfo>()
        .expect("game lookup should decode");
    assert_eq!(game_info.title.as_deref(), Some("BT11"));
    assert!(game_info.local_package_path.is_none());

    let client = ArcRemoteClient::new(server.base_url.clone(), "local-test-key")
        .expect("client should initialize");
    let scorecard = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: Some("https://example.invalid/arc-local".to_owned()),
            tags: vec!["compat".to_owned()],
            opaque: Some(json!({
                "mode": "local-compatibility",
                "origin": "arc-client-tests"
            })),
            competition_mode: None,
        })
        .expect("scorecard should open");

    let empty_summary = client
        .get_scorecard(&scorecard.card_id, None)
        .expect("fresh scorecard should be readable");
    assert_eq!(
        empty_summary.opaque,
        Some(json!({
            "mode": "local-compatibility",
            "origin": "arc-client-tests"
        }))
    );
    assert_eq!(empty_summary.total_actions, Some(0));
    assert_eq!(empty_summary.total_environments, Some(0));
    assert!(empty_summary.environments.is_empty());

    let mut environment =
        RemoteArcEnvironment::new(client.clone(), games[0].clone(), scorecard.card_id.clone());
    let reset = environment
        .reset()
        .expect("reset should start a local session");
    assert_eq!(reset.game_state, ArcGameState::NotFinished);
    assert!(reset.full_reset);

    let after_reset = client
        .get_scorecard(&scorecard.card_id, None)
        .expect("scorecard should update after reset");
    assert_eq!(after_reset.score, 0);
    assert_eq!(after_reset.total_environments, Some(1));
    assert_eq!(after_reset.total_actions, Some(0));
    assert_eq!(after_reset.environments.len(), 1);
    assert_eq!(after_reset.environments[0].runs.len(), 1);
    assert_eq!(
        after_reset.environments[0].runs[0].level_baseline_actions,
        vec![4]
    );

    let step = environment
        .step(ArcAction::Action3)
        .expect("action should execute through the compatibility server");
    assert_eq!(step.action, ArcAction::Action3);

    let per_game_summary = client
        .get_scorecard(&scorecard.card_id, Some(&games[0].game_id))
        .expect("per-game scorecard view should be readable");
    assert_local_summary_shape(&per_game_summary);
    assert_eq!(per_game_summary.environments.len(), 1);
    assert_eq!(per_game_summary.environments[0].runs[0].actions, 1);
    assert_eq!(per_game_summary.environments[0].runs[0].resets, 0);
    assert_eq!(per_game_summary.total_actions, Some(1));

    let closed_summary = client
        .close_scorecard(&ArcCloseScorecardRequest {
            card_id: scorecard.card_id.clone(),
        })
        .expect("closing the scorecard should succeed");
    assert_local_summary_shape(&closed_summary);
    assert!(closed_summary.published_at.is_some());

    let post_close_error = environment
        .step(ArcAction::Action3)
        .expect_err("closed scorecard should reject new actions");
    match post_close_error {
        arc_client::ArcClientError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);
        }
        other => panic!("unexpected post-close error: {other}"),
    }

    let archived_summary = client
        .get_scorecard(&scorecard.card_id, None)
        .expect("closed scorecards should remain readable");
    assert_eq!(archived_summary.published_at, closed_summary.published_at);
    assert_eq!(archived_summary.total_actions, Some(1));
}

fn assert_local_summary_shape(summary: &ArcScorecardSummary) {
    assert_eq!(summary.score, 0);
    assert!(summary.open_at.is_some());
    assert!(summary.last_update.is_some());
    assert!(summary.tags_scores.is_empty());
}
