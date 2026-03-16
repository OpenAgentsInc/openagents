use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use arc_client::{
    ArcCloseScorecardRequest, ArcCompatibilityServer, ArcCompatibilityServerConfig,
    ArcEnvironmentInfo, ArcOpenScorecardRequest, ArcRegisteredEnvironment, ArcRemoteArcade,
    ArcRemoteClient, ArcScorecardSummary, RemoteArcEnvironment,
};
use arc_core::{ArcAction, ArcGameState, ArcOperationMode, ArcTaskId};
use serde_json::json;
use tokio::sync::oneshot;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("engine")
        .join("fixtures")
        .join(name)
}

fn registered_environment(
    game_id: &str,
    title: &str,
    baseline_actions: Vec<u32>,
    fixture_name: &str,
) -> ArcRegisteredEnvironment {
    ArcRegisteredEnvironment::new(
        ArcEnvironmentInfo {
            game_id: ArcTaskId::new(game_id).expect("task id should validate"),
            title: Some(title.to_owned()),
            tags: vec!["benchmark".to_owned()],
            private_tags: Vec::new(),
            level_tags: Vec::new(),
            baseline_actions,
            class_name: Some(title.to_owned()),
            local_package_path: None,
        },
        fixture_path(fixture_name),
    )
}

fn registered_environment_bt11() -> ArcRegisteredEnvironment {
    registered_environment(
        "bt11-fd9df0622a1a",
        "BT11",
        vec![4],
        "upstream/bt11-fd9df0622a1a.json",
    )
}

fn registered_environment_bt33() -> ArcRegisteredEnvironment {
    registered_environment(
        "bt33-a7c3f9d18b4e",
        "BT33",
        vec![2],
        "upstream/bt33-a7c3f9d18b4e.json",
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
    spawn_server_with_config(
        vec![registered_environment_bt11()],
        ArcCompatibilityServerConfig::default(),
    )
}

fn spawn_server_with_config(
    environments: Vec<ArcRegisteredEnvironment>,
    config: ArcCompatibilityServerConfig,
) -> TestServerHandle {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should expose a bound address");
    listener
        .set_nonblocking(true)
        .expect("listener should become non-blocking");
    let server = ArcCompatibilityServer::new_with_config(environments, config);
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

#[test]
fn remote_arcade_reuses_default_scorecard_until_closed() {
    let server = spawn_server();
    let arcade = ArcRemoteArcade::new(
        ArcRemoteClient::new(server.base_url.clone(), "local-test-key")
            .expect("client should initialize"),
        ArcOperationMode::Online,
    )
    .with_default_open_request(ArcOpenScorecardRequest {
        source_url: Some("https://example.invalid/arc-default".to_owned()),
        tags: vec!["wrapper".to_owned()],
        opaque: Some(json!({ "mode": "default-scorecard" })),
        competition_mode: None,
    });

    let first = arcade
        .get_scorecard(None, None)
        .expect("default scorecard should open on first access");
    let second = arcade
        .get_scorecard(None, None)
        .expect("default scorecard should be reused");
    assert_eq!(first.card_id, second.card_id);
    assert_eq!(
        arcade.default_scorecard_id().expect("lock should succeed"),
        Some(first.card_id.clone())
    );

    let info = arcade
        .client()
        .list_games()
        .expect("game list should load")
        .into_iter()
        .next()
        .expect("fixture game should be listed");
    let mut environment = arcade
        .remote_environment(info, None)
        .expect("default scorecard should back environment creation");
    environment
        .reset()
        .expect("reset should start remote session");

    let after_reset = arcade
        .get_scorecard(None, None)
        .expect("default scorecard should remain readable");
    assert_eq!(after_reset.card_id, first.card_id);
    assert_eq!(after_reset.total_environments, Some(1));

    let closed = arcade
        .close_scorecard(None)
        .expect("closing the default scorecard should succeed")
        .expect("default scorecard should exist");
    assert_eq!(closed.card_id, first.card_id);
    assert_eq!(
        arcade.default_scorecard_id().expect("lock should succeed"),
        None
    );

    let reopened = arcade
        .get_scorecard(None, None)
        .expect("next default access should allocate a fresh scorecard");
    assert_ne!(reopened.card_id, first.card_id);
}

#[test]
fn compatibility_server_enforces_competition_mode_lifecycle_restrictions() {
    let server = spawn_server_with_config(
        vec![registered_environment_bt11(), registered_environment_bt33()],
        ArcCompatibilityServerConfig::default(),
    );
    let client = ArcRemoteClient::new(server.base_url.clone(), "competition-key")
        .expect("client should initialize");
    let games = client.list_games().expect("games should load");
    assert_eq!(games.len(), 2);

    let scorecard = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: Some("https://example.invalid/arc-competition".to_owned()),
            tags: vec!["competition".to_owned()],
            opaque: Some(json!({ "mode": "competition" })),
            competition_mode: Some(true),
        })
        .expect("competition scorecard should open");

    let duplicate = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: None,
            tags: vec!["competition".to_owned()],
            opaque: None,
            competition_mode: Some(true),
        })
        .expect_err("second competition scorecard should be refused");
    match duplicate {
        arc_client::ArcClientError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, reqwest::StatusCode::CONFLICT);
        }
        other => panic!("unexpected duplicate-open error: {other}"),
    }

    let inflight = client
        .get_scorecard(&scorecard.card_id, None)
        .expect_err("inflight competition scorecard should not be readable");
    match inflight {
        arc_client::ArcClientError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, reqwest::StatusCode::FORBIDDEN);
        }
        other => panic!("unexpected inflight-read error: {other}"),
    }

    let mut environment =
        RemoteArcEnvironment::new(client.clone(), games[0].clone(), scorecard.card_id.clone());
    environment
        .reset()
        .expect("first competition environment should open once");

    let mut duplicate_environment =
        RemoteArcEnvironment::new(client.clone(), games[0].clone(), scorecard.card_id.clone());
    let duplicate_reset = duplicate_environment
        .reset()
        .expect_err("same competition environment should not reopen");
    match duplicate_reset {
        arc_client::ArcClientError::UnexpectedStatus { status, .. } => {
            assert_eq!(status, reqwest::StatusCode::CONFLICT);
        }
        other => panic!("unexpected duplicate-reset error: {other}"),
    }

    let closed = client
        .close_scorecard(&ArcCloseScorecardRequest {
            card_id: scorecard.card_id.clone(),
        })
        .expect("closing competition scorecard should succeed");
    assert_eq!(closed.total_environments, Some(2));
    assert_eq!(closed.environments.len(), 2);
    assert_eq!(
        closed
            .environments
            .iter()
            .filter(|environment| environment.runs.is_empty())
            .count(),
        1
    );

    let archived = client
        .get_scorecard(&scorecard.card_id, None)
        .expect("closed competition scorecard should become readable");
    assert_eq!(archived.total_environments, Some(2));
}

#[test]
fn stale_scorecards_auto_close_and_refuse_follow_on_actions() {
    let server = spawn_server_with_config(
        vec![registered_environment_bt11()],
        ArcCompatibilityServerConfig {
            stale_after: Some(Duration::from_millis(10)),
        },
    );
    let client = ArcRemoteClient::new(server.base_url.clone(), "stale-key")
        .expect("client should initialize");
    let game = client
        .list_games()
        .expect("games should load")
        .into_iter()
        .next()
        .expect("fixture game should exist");

    let scorecard = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: None,
            tags: vec!["stale".to_owned()],
            opaque: None,
            competition_mode: None,
        })
        .expect("scorecard should open");
    let mut environment =
        RemoteArcEnvironment::new(client.clone(), game, scorecard.card_id.clone());
    environment.reset().expect("reset should start the session");

    std::thread::sleep(Duration::from_millis(25));

    let error = environment
        .step(ArcAction::Action3)
        .expect_err("stale auto-close should refuse further actions");
    match error {
        arc_client::ArcClientError::UnexpectedStatus { status, body, .. } => {
            assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);
            assert!(body.contains("closed"), "{body}");
        }
        other => panic!("unexpected stale-close error: {other}"),
    }

    let archived = client
        .get_scorecard(&scorecard.card_id, None)
        .expect("auto-closed scorecard should remain readable");
    assert!(archived.published_at.is_some());
}

fn assert_local_summary_shape(summary: &ArcScorecardSummary) {
    assert_eq!(summary.score, 0);
    assert!(summary.open_at.is_some());
    assert!(summary.last_update.is_some());
    assert!(summary.tags_scores.is_empty());
}
