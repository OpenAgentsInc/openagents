use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

use arc_client::{
    ArcCompatibilityServer, ArcEnvironmentInfo, ArcOpenScorecardRequest, ArcRegisteredEnvironment,
    ArcRemoteClient, LocalArcEnvironment, RemoteArcEnvironment,
};
use arc_core::{
    ArcAction, ArcGameState, ArcInteractiveActionResult, ArcInteractiveExecutionOutcome,
    ArcInteractiveRefusalCode, ArcInteractiveResetKind, ArcScorePolicyId, ArcTaskId,
};
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

struct RefusingAgent {
    name: String,
    message: String,
}

impl RefusingAgent {
    fn new(name: &str, message: &str) -> Self {
        Self {
            name: name.to_owned(),
            message: message.to_owned(),
        }
    }
}

impl ArcInteractiveAgent for RefusingAgent {
    fn agent_name(&self) -> &str {
        &self.name
    }

    fn step(
        &mut self,
        _context: &ArcInteractiveSessionContext,
    ) -> Result<ArcInteractiveGameStep, ArcInteractiveAgentError> {
        Err(ArcInteractiveAgentError::message(self.message.clone()))
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

fn local_environment(scorecard_id: &str) -> LocalArcEnvironment {
    LocalArcEnvironment::load_from_path(demo_environment_info(), demo_package_path(), scorecard_id)
        .expect("local environment should initialize")
}

fn runner_config(checkpoint_id: &str, max_agent_actions: u32) -> ArcInteractiveRunnerConfig {
    ArcInteractiveRunnerConfig::new(
        checkpoint_id,
        ArcScorePolicyId::ArcAgi3MethodologyV1,
        max_agent_actions,
    )
    .expect("runner config should validate")
}

#[test]
fn interactive_runner_executes_a_bounded_local_episode_end_to_end() {
    let environment = local_environment("local-card");
    let mut config = runner_config("local-demo-checkpoint", 16);
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
    assert_eq!(artifacts.report.final_state, ArcGameState::Win);
    assert_eq!(artifacts.recording.steps.len(), 13);
    assert!(artifacts.recording.steps[0].full_reset);
    assert_eq!(artifacts.checkpoint_bundle.metadata.step_count, 13);
    assert_eq!(artifacts.checkpoint_handoff.agent_name, "fixed-local");
    assert_eq!(artifacts.checkpoint_handoff.actions_taken, 12);
    assert!(artifacts.checkpoint_handoff.terminal);
    assert_eq!(artifacts.turn_results.len(), 12);
    assert!(matches!(
        artifacts.execution_outcome,
        ArcInteractiveExecutionOutcome::Completed {
            final_state: ArcGameState::Win,
            ..
        }
    ));
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

    let mut config = runner_config("remote-demo-checkpoint", 16);
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
    assert_eq!(artifacts.report.final_state, ArcGameState::Win);
    assert_eq!(artifacts.turn_results.len(), 12);
    assert!(matches!(
        artifacts.execution_outcome,
        ArcInteractiveExecutionOutcome::Completed {
            final_state: ArcGameState::Win,
            ..
        }
    ));
    assert_eq!(artifacts.checkpoint_handoff.agent_name, "fixed-remote");
    assert!(scorecard_summary.published_at.is_some());
    assert_eq!(scorecard_summary.total_actions, Some(12));
    assert_eq!(scorecard_summary.environments.len(), 1);
    assert_eq!(scorecard_summary.environments[0].runs.len(), 1);
    assert_eq!(scorecard_summary.environments[0].runs[0].actions, 12);
    assert!(scorecard_summary.environments[0].runs[0].completed);
}

#[test]
fn interactive_runner_marks_budget_exhaustion_as_an_explicit_refusal() {
    let environment = local_environment("budget-card");
    let mut runner = ArcInteractiveRunner::new(environment, runner_config("budget-checkpoint", 1));
    let mut agent = FixedSequenceAgent::new("budget-agent", vec![ArcAction::Action4]);
    let artifacts = runner
        .run_episode(&mut agent)
        .expect("budget-limited episode should run");

    assert_eq!(artifacts.report.total_actions, 1);
    assert_eq!(artifacts.turn_results.len(), 1);
    assert!(matches!(
        artifacts.turn_results[0].result,
        ArcInteractiveActionResult::Executed {
            terminal: false,
            reset: None,
            ..
        }
    ));
    assert!(matches!(
        artifacts.execution_outcome,
        ArcInteractiveExecutionOutcome::Refused {
            refusal: arc_core::ArcInteractiveRefusal {
                code: ArcInteractiveRefusalCode::BudgetExhausted,
                ..
            }
        }
    ));
}

#[test]
fn interactive_runner_refuses_invalid_actions_before_the_environment_drifts() {
    let environment = local_environment("invalid-card");
    let mut runner = ArcInteractiveRunner::new(environment, runner_config("invalid-checkpoint", 2));
    let mut agent = FixedSequenceAgent::new("invalid-agent", vec![ArcAction::Action7]);
    let artifacts = runner
        .run_episode(&mut agent)
        .expect("invalid-action episode should still materialize artifacts");

    assert_eq!(artifacts.report.total_actions, 0);
    assert_eq!(artifacts.recording.steps.len(), 1);
    assert_eq!(artifacts.turn_results.len(), 1);
    match &artifacts.turn_results[0].result {
        ArcInteractiveActionResult::Refused { refusal } => {
            assert_eq!(refusal.code, ArcInteractiveRefusalCode::InvalidAction);
            assert_eq!(refusal.action, Some(ArcAction::Action7));
        }
        other => panic!("expected invalid-action refusal, got {other:?}"),
    }
    assert!(matches!(
        artifacts.execution_outcome,
        ArcInteractiveExecutionOutcome::Refused {
            refusal: arc_core::ArcInteractiveRefusal {
                code: ArcInteractiveRefusalCode::InvalidAction,
                ..
            }
        }
    ));
}

#[test]
fn interactive_runner_tracks_full_and_level_reset_semantics() {
    let environment = local_environment("reset-card");
    let mut runner = ArcInteractiveRunner::new(environment, runner_config("reset-checkpoint", 2));
    let mut agent =
        FixedSequenceAgent::new("reset-agent", vec![ArcAction::Action4, ArcAction::Reset]);
    let artifacts = runner
        .run_episode(&mut agent)
        .expect("reset episode should run");

    assert!(artifacts.recording.steps[0].full_reset);
    assert_eq!(artifacts.turn_results.len(), 2);
    match &artifacts.turn_results[1].result {
        ArcInteractiveActionResult::Executed { reset, .. } => {
            assert_eq!(reset, &Some(ArcInteractiveResetKind::LevelOnly));
        }
        other => panic!("expected executed reset result, got {other:?}"),
    }
    assert!(matches!(
        artifacts.execution_outcome,
        ArcInteractiveExecutionOutcome::Refused {
            refusal: arc_core::ArcInteractiveRefusal {
                code: ArcInteractiveRefusalCode::BudgetExhausted,
                ..
            }
        }
    ));
}

#[test]
fn interactive_runner_refuses_when_the_episode_is_already_terminal() {
    let mut environment = local_environment("terminal-card");
    environment
        .reset()
        .expect("reset should initialize the session");
    for action in winning_demo_sequence() {
        environment
            .step(action)
            .expect("winning sequence should replay");
    }
    let mut runner =
        ArcInteractiveRunner::new(environment, runner_config("terminal-checkpoint", 16));
    let mut agent = FixedSequenceAgent::new("terminal-agent", vec![ArcAction::Action4]);
    let artifacts = runner
        .run_episode(&mut agent)
        .expect("terminal episodes should still materialize artifacts");

    assert!(artifacts.report.completed);
    assert_eq!(artifacts.report.final_state, ArcGameState::Win);
    assert!(artifacts.turn_results.is_empty());
    assert!(matches!(
        artifacts.execution_outcome,
        ArcInteractiveExecutionOutcome::Refused {
            refusal: arc_core::ArcInteractiveRefusal {
                code: ArcInteractiveRefusalCode::TerminalState,
                ..
            }
        }
    ));
}

#[test]
fn interactive_runner_maps_agent_refusals_to_policy_refusals() {
    let environment = local_environment("policy-card");
    let mut runner = ArcInteractiveRunner::new(environment, runner_config("policy-checkpoint", 4));
    let mut agent = RefusingAgent::new("policy-agent", "prompt policy denied the move");
    let artifacts = runner
        .run_episode(&mut agent)
        .expect("agent refusals should still materialize artifacts");

    assert_eq!(artifacts.report.total_actions, 0);
    assert!(artifacts.turn_results.is_empty());
    assert!(matches!(
        artifacts.execution_outcome,
        ArcInteractiveExecutionOutcome::Refused {
            refusal: arc_core::ArcInteractiveRefusal {
                code: ArcInteractiveRefusalCode::PolicyRefusal,
                ..
            }
        }
    ));
}

#[test]
fn interactive_runner_maps_closed_scorecards_to_explicit_refusals() {
    let server = spawn_server();
    let client = ArcRemoteClient::new(server.base_url.clone(), "closed-scorecard-test-key")
        .expect("remote client should initialize");
    let scorecard = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: Some("https://example.invalid/arc-closed-scorecard".to_owned()),
            tags: vec![
                "interactive-runner".to_owned(),
                "closed-scorecard".to_owned(),
            ],
            opaque: Some(json!({ "kind": "closed-scorecard-test" })),
            competition_mode: None,
        })
        .expect("scorecard should open");
    let games = client.list_games().expect("game listing should succeed");
    let mut environment =
        RemoteArcEnvironment::new(client.clone(), games[0].clone(), scorecard.card_id.clone());
    environment
        .reset()
        .expect("reset should create the initial session frame");
    client
        .close_scorecard(&arc_client::ArcCloseScorecardRequest {
            card_id: scorecard.card_id,
        })
        .expect("scorecard should close cleanly");

    let mut runner =
        ArcInteractiveRunner::new(environment, runner_config("closed-scorecard-checkpoint", 4));
    let mut agent = FixedSequenceAgent::new("closed-scorecard-agent", vec![ArcAction::Action4]);
    let artifacts = runner
        .run_episode(&mut agent)
        .expect("closed scorecard runs should still materialize artifacts");

    assert_eq!(artifacts.report.total_actions, 0);
    assert_eq!(artifacts.recording.steps.len(), 1);
    assert_eq!(artifacts.turn_results.len(), 1);
    match &artifacts.turn_results[0].result {
        ArcInteractiveActionResult::Refused { refusal } => {
            assert_eq!(refusal.code, ArcInteractiveRefusalCode::ClosedScorecard);
        }
        other => panic!("expected closed-scorecard refusal, got {other:?}"),
    }
    assert!(matches!(
        artifacts.execution_outcome,
        ArcInteractiveExecutionOutcome::Refused {
            refusal: arc_core::ArcInteractiveRefusal {
                code: ArcInteractiveRefusalCode::ClosedScorecard,
                ..
            }
        }
    ));
}
