use std::collections::BTreeMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

use arc_client::{
    ArcCompatibilityServer, ArcEnvironmentInfo, ArcOpenScorecardRequest, ArcRegisteredEnvironment,
    ArcRemoteClient, LocalArcEnvironment, RemoteArcEnvironment,
};
use arc_core::{ArcAction, ArcScorePolicyId, ArcTaskId};
use arc_solvers::{
    ArcInteractiveRunner, ArcInteractiveRunnerConfig, ArcRandomBaselineAgent,
    ArcRandomBaselineConfig, ArcScriptedBaselineAgent, ArcScriptedBaselineProgram,
    compare_interactive_run_artifacts,
};
use serde_json::json;
use tokio::sync::oneshot;

fn demo_package_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("engine")
        .join("fixtures")
        .join("demo_game.json")
}

fn demo_environment_info() -> ArcEnvironmentInfo {
    ArcEnvironmentInfo {
        game_id: ArcTaskId::new("arc-engine-demo").expect("task id should validate"),
        title: Some("Demo ARC".to_owned()),
        tags: vec!["interactive-baselines".to_owned()],
        private_tags: Vec::new(),
        level_tags: Vec::new(),
        baseline_actions: vec![7, 5],
        class_name: Some("DemoArcGame".to_owned()),
        local_package_path: None,
    }
}

fn winning_demo_sequence() -> Vec<ArcAction> {
    vec![
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::action6(22, 22).expect("coords should validate"),
        ArcAction::Action4,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action2,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action5,
    ]
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
    let environment = ArcRegisteredEnvironment::new(demo_environment_info(), demo_package_path());
    let server = ArcCompatibilityServer::new(vec![environment]);
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

fn local_environment(scorecard_id: &str) -> LocalArcEnvironment {
    LocalArcEnvironment::load_from_path(demo_environment_info(), demo_package_path(), scorecard_id)
        .expect("local environment should initialize")
}

fn runner_config(
    checkpoint_id: &str,
    max_agent_actions: u32,
    checkpoint_timestamp_unix_s: u64,
    close_scorecard_on_finish: bool,
) -> ArcInteractiveRunnerConfig {
    let mut config = ArcInteractiveRunnerConfig::new(
        checkpoint_id,
        ArcScorePolicyId::ArcAgi3MethodologyV1,
        max_agent_actions,
    )
    .expect("runner config should validate");
    config.metadata.source_url =
        Some("https://example.invalid/arc-interactive-baselines".to_owned());
    config.metadata.tags = vec!["interactive-baselines".to_owned()];
    config.metadata.opaque = Some(json!({ "checkpoint_id": checkpoint_id }));
    config.checkpoint_timestamp_unix_s = checkpoint_timestamp_unix_s;
    config.close_scorecard_on_finish = close_scorecard_on_finish;
    config
}

#[test]
fn seeded_random_baseline_replays_repeatably_on_local_fixture() {
    let random_config = ArcRandomBaselineConfig::new(17).expect("random config should validate");

    let artifacts_a = {
        let environment = local_environment("random-baseline");
        let runner_config = runner_config("random-baseline", 4, 1_735_689_600, false);
        let mut runner = ArcInteractiveRunner::new(environment, runner_config);
        let mut agent = ArcRandomBaselineAgent::new("random-baseline", random_config.clone())
            .expect("agent should build");
        runner
            .run_episode(&mut agent)
            .expect("first random baseline run should complete")
    };

    let artifacts_b = {
        let environment = local_environment("random-baseline");
        let runner_config = runner_config("random-baseline", 4, 1_735_689_600, false);
        let mut runner = ArcInteractiveRunner::new(environment, runner_config);
        let mut agent = ArcRandomBaselineAgent::new("random-baseline", random_config)
            .expect("agent should build");
        runner
            .run_episode(&mut agent)
            .expect("second random baseline run should complete")
    };

    assert_eq!(artifacts_a.report, artifacts_b.report);
    assert_eq!(artifacts_a.recording, artifacts_b.recording);
    assert_eq!(artifacts_a.turn_results, artifacts_b.turn_results);
    assert_eq!(artifacts_a.execution_outcome, artifacts_b.execution_outcome);
    assert_eq!(artifacts_a.checkpoint_bundle, artifacts_b.checkpoint_bundle);
    assert_eq!(
        artifacts_a.checkpoint_handoff.actions_taken,
        artifacts_b.checkpoint_handoff.actions_taken
    );
    assert_eq!(
        artifacts_a.checkpoint_handoff.next_step_index,
        artifacts_b.checkpoint_handoff.next_step_index
    );
    assert_eq!(
        artifacts_a.checkpoint_handoff.agent_state,
        artifacts_b.checkpoint_handoff.agent_state
    );
}

#[test]
fn seeded_random_baseline_closes_remote_scorecards_cleanly() {
    let server = spawn_server();
    let client = ArcRemoteClient::new(server.base_url.clone(), "random-baseline-remote-key")
        .expect("remote client should initialize");
    let remote_info = client
        .list_games()
        .expect("game listing should succeed")
        .into_iter()
        .next()
        .expect("registered game should exist");
    let scorecard = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: Some("https://example.invalid/random-baseline".to_owned()),
            tags: vec!["interactive-baselines".to_owned()],
            opaque: Some(json!({ "agent": "random-baseline" })),
            competition_mode: None,
        })
        .expect("scorecard should open");
    let environment = RemoteArcEnvironment::new(client, remote_info, scorecard.card_id);
    let config = runner_config("random-baseline-remote", 4, 1_735_689_600, true);
    let mut runner = ArcInteractiveRunner::new(environment, config);
    let mut agent = ArcRandomBaselineAgent::new(
        "random-baseline",
        ArcRandomBaselineConfig::new(17).expect("random config should validate"),
    )
    .expect("agent should build");
    let artifacts = runner
        .run_episode(&mut agent)
        .expect("remote random baseline run should complete");

    let Some(summary) = artifacts.scorecard_summary else {
        panic!("remote baseline runs should materialize a scorecard summary");
    };
    assert!(summary.published_at.is_some());
    assert_eq!(summary.total_actions, Some(artifacts.report.total_actions));
    assert_eq!(summary.environments.len(), 1);
}

#[test]
fn scripted_baseline_reproduces_a_stable_demo_win_locally_and_remotely() {
    let program = ArcScriptedBaselineProgram::new("demo-win", winning_demo_sequence())
        .expect("program should validate");
    let local_artifacts = {
        let environment = local_environment("scripted-demo");
        let config = runner_config("scripted-demo", 16, 1_735_689_600, false);
        let mut runner = ArcInteractiveRunner::new(environment, config);
        let mut agent = ArcScriptedBaselineAgent::new("scripted-demo", program.clone())
            .expect("agent should build");
        runner
            .run_episode(&mut agent)
            .expect("local scripted baseline should complete")
    };
    assert!(local_artifacts.report.completed);
    assert_eq!(local_artifacts.report.total_actions, 12);
    assert_eq!(local_artifacts.turn_results.len(), 12);

    let server = spawn_server();
    let client = ArcRemoteClient::new(server.base_url.clone(), "scripted-baseline-remote-key")
        .expect("remote client should initialize");
    let remote_infos = client
        .list_games()
        .expect("game listing should succeed")
        .into_iter()
        .map(|info| (info.game_id.to_string(), info))
        .collect::<BTreeMap<_, _>>();
    let scorecard = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: Some("https://example.invalid/scripted-baseline".to_owned()),
            tags: vec!["interactive-baselines".to_owned()],
            opaque: Some(json!({ "agent": "scripted-demo" })),
            competition_mode: None,
        })
        .expect("scorecard should open");
    let environment = RemoteArcEnvironment::new(
        client,
        remote_infos
            .get("arc-engine-demo")
            .cloned()
            .expect("demo environment should exist"),
        scorecard.card_id,
    );
    let config = runner_config("scripted-demo", 16, 1_735_689_600, true);
    let mut runner = ArcInteractiveRunner::new(environment, config);
    let mut agent =
        ArcScriptedBaselineAgent::new("scripted-demo", program).expect("agent should build");
    let remote_artifacts = runner
        .run_episode(&mut agent)
        .expect("remote scripted baseline should complete");

    assert!(remote_artifacts.report.completed);
    let parity = compare_interactive_run_artifacts(
        "scripted-demo",
        &local_artifacts,
        &remote_artifacts,
        &[],
    );
    assert_eq!(
        parity.outcome,
        arc_solvers::ArcInteractiveRunnerParityOutcome::Match,
        "{}",
        serde_json::to_string_pretty(&parity).expect("parity report should serialize")
    );
    let Some(summary) = remote_artifacts.scorecard_summary else {
        panic!("remote scripted baseline should close the scorecard");
    };
    assert!(summary.published_at.is_some());
    assert_eq!(summary.total_actions, Some(12));
}
