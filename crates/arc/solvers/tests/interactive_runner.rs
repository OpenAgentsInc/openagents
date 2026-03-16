use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

use arc_client::{
    ArcCompatibilityServer, ArcEnvironmentInfo, ArcOpenScorecardRequest, ArcRegisteredEnvironment,
    ArcRemoteClient, LocalArcEnvironment, RemoteArcEnvironment,
};
use arc_core::{ArcAction, ArcScorePolicyId, ArcTaskId};
use arc_solvers::{
    ArcInteractiveAgent, ArcInteractiveAgentError, ArcInteractiveGameStep, ArcInteractiveRunner,
    ArcInteractiveRunnerConfig, ArcInteractiveSessionContext,
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
        tags: vec!["interactive-runner".to_owned()],
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

struct FixedSequenceAgent {
    name: String,
    actions: Vec<ArcAction>,
    cursor: usize,
}

impl FixedSequenceAgent {
    fn new(name: &str, actions: Vec<ArcAction>) -> Self {
        Self {
            name: name.to_owned(),
            actions,
            cursor: 0,
        }
    }
}

impl ArcInteractiveAgent for FixedSequenceAgent {
    fn agent_name(&self) -> &str {
        &self.name
    }

    fn step(
        &mut self,
        _context: &ArcInteractiveSessionContext,
    ) -> Result<ArcInteractiveGameStep, ArcInteractiveAgentError> {
        let action = self
            .actions
            .get(self.cursor)
            .cloned()
            .ok_or_else(|| ArcInteractiveAgentError::message("fixed sequence exhausted"))?;
        self.cursor = self.cursor.saturating_add(1);
        Ok(ArcInteractiveGameStep::new(action).with_reasoning(json!({
            "agent": self.name,
            "cursor": self.cursor,
        })))
    }

    fn checkpoint_state(&self) -> Result<Option<serde_json::Value>, ArcInteractiveAgentError> {
        Ok(Some(json!({ "cursor": self.cursor })))
    }

    fn restore_checkpoint_state(
        &mut self,
        state: Option<&serde_json::Value>,
    ) -> Result<(), ArcInteractiveAgentError> {
        let cursor = state
            .and_then(|state| state.get("cursor"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        self.cursor = usize::try_from(cursor).unwrap_or(usize::MAX);
        Ok(())
    }
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

#[test]
fn interactive_runner_executes_a_bounded_local_episode_end_to_end() {
    let environment = LocalArcEnvironment::load_from_path(
        demo_environment_info(),
        demo_package_path(),
        "local-card",
    )
    .expect("local environment should initialize");
    let mut config = ArcInteractiveRunnerConfig::new(
        "local-demo-checkpoint",
        ArcScorePolicyId::ArcAgi3MethodologyV1,
        16,
    )
    .expect("runner config should validate");
    config.metadata.tags.push("local".to_owned());

    let mut runner = ArcInteractiveRunner::new(environment, config);
    let mut agent = FixedSequenceAgent::new("fixed-local", winning_demo_sequence());
    let artifacts = runner
        .run_episode(&mut agent)
        .expect("local episode should run");

    assert_eq!(
        artifacts.environment_kind,
        arc_solvers::ArcInteractiveEnvironmentKind::Local
    );
    assert_eq!(artifacts.report.total_actions, 12);
    assert!(artifacts.report.completed);
    assert_eq!(artifacts.report.final_state, arc_core::ArcGameState::Win);
    assert_eq!(artifacts.recording.steps.len(), 13);
    assert_eq!(artifacts.checkpoint_bundle.metadata.step_count, 13);
    assert_eq!(artifacts.checkpoint_handoff.agent_name, "fixed-local");
    assert_eq!(artifacts.checkpoint_handoff.actions_taken, 12);
    assert!(artifacts.checkpoint_handoff.terminal);
    assert!(artifacts.scorecard_summary.is_none());
    artifacts
        .checkpoint_bundle
        .validate()
        .expect("checkpoint bundle should remain valid");
}

#[test]
fn interactive_runner_executes_a_bounded_remote_episode_end_to_end() {
    let server = spawn_server();
    let client = ArcRemoteClient::new(server.base_url.clone(), "interactive-runner-test-key")
        .expect("remote client should initialize");
    let scorecard = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: Some("https://example.invalid/arc-interactive-runner".to_owned()),
            tags: vec!["interactive-runner".to_owned()],
            opaque: Some(json!({ "kind": "remote-test" })),
            competition_mode: None,
        })
        .expect("scorecard should open");
    let games = client.list_games().expect("game listing should succeed");
    let environment = RemoteArcEnvironment::new(client, games[0].clone(), scorecard.card_id);

    let mut config = ArcInteractiveRunnerConfig::new(
        "remote-demo-checkpoint",
        ArcScorePolicyId::ArcAgi3MethodologyV1,
        16,
    )
    .expect("runner config should validate");
    config.metadata.tags.push("remote".to_owned());
    config.close_scorecard_on_finish = true;

    let mut runner = ArcInteractiveRunner::new(environment, config);
    let mut agent = FixedSequenceAgent::new("fixed-remote", winning_demo_sequence());
    let artifacts = runner
        .run_episode(&mut agent)
        .expect("remote episode should run");

    let Some(scorecard_summary) = artifacts.scorecard_summary else {
        panic!("remote runs should materialize a scorecard summary");
    };
    assert_eq!(
        artifacts.environment_kind,
        arc_solvers::ArcInteractiveEnvironmentKind::Remote
    );
    assert_eq!(artifacts.report.total_actions, 12);
    assert!(artifacts.report.completed);
    assert_eq!(artifacts.report.final_state, arc_core::ArcGameState::Win);
    assert_eq!(artifacts.checkpoint_handoff.agent_name, "fixed-remote");
    assert!(scorecard_summary.published_at.is_some());
    assert_eq!(scorecard_summary.total_actions, Some(12));
    assert_eq!(scorecard_summary.environments.len(), 1);
    assert_eq!(scorecard_summary.environments[0].runs.len(), 1);
    assert_eq!(scorecard_summary.environments[0].runs[0].actions, 12);
    assert!(scorecard_summary.environments[0].runs[0].completed);
}
