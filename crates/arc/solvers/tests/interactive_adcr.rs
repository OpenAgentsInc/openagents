use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

use arc_client::{
    ArcCompatibilityServer, ArcEnvironmentInfo, ArcOpenScorecardRequest, ArcRegisteredEnvironment,
    ArcRemoteClient, LocalArcEnvironment, RemoteArcEnvironment,
};
use arc_core::{ArcAction, ArcGameState, ArcScorePolicyId, ArcTaskId};
use arc_solvers::{
    compare_interactive_run_artifacts, ArcAdcrBaselineAgent, ArcAdcrConfig, ArcAdcrHumanAction,
    ArcAdcrReplayProgram, ArcInteractivePromptPolicy, ArcInteractivePromptSection,
    ArcInteractiveRunner, ArcInteractiveRunnerConfig, ArcInteractiveRunnerParityOutcome,
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
        tags: vec!["interactive-adcr".to_owned()],
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

fn winning_demo_human_sequence() -> Vec<ArcAdcrHumanAction> {
    winning_demo_sequence()
        .into_iter()
        .map(|action| match action {
            ArcAction::Action1 => ArcAdcrHumanAction::MoveUp,
            ArcAction::Action2 => ArcAdcrHumanAction::MoveDown,
            ArcAction::Action3 => ArcAdcrHumanAction::MoveLeft,
            ArcAction::Action4 => ArcAdcrHumanAction::MoveRight,
            ArcAction::Action5 => ArcAdcrHumanAction::PerformAction,
            ArcAction::Action6 { x, y } => {
                ArcAdcrHumanAction::click_object(x, y).expect("coords should validate")
            }
            ArcAction::Action7 => ArcAdcrHumanAction::Undo,
            ArcAction::Reset => ArcAdcrHumanAction::Reset,
        })
        .collect()
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
    close_scorecard_on_finish: bool,
) -> ArcInteractiveRunnerConfig {
    let mut config = ArcInteractiveRunnerConfig::new(
        checkpoint_id,
        ArcScorePolicyId::ArcAgi3MethodologyV1,
        max_agent_actions,
    )
    .expect("runner config should validate");
    config.metadata.tags = vec!["interactive-adcr".to_owned()];
    config.checkpoint_timestamp_unix_s = 1_735_689_600;
    config.close_scorecard_on_finish = close_scorecard_on_finish;
    config.prompt_policy = ArcInteractivePromptPolicy::new(
        "adcr-fixture-prompt-v1",
        vec![
            ArcInteractivePromptSection::SessionProgress,
            ArcInteractivePromptSection::CurrentObservation,
            ArcInteractivePromptSection::SessionMemory,
        ],
    )
    .expect("prompt policy should validate");
    config
}

fn replay_agent(name: &str) -> ArcAdcrBaselineAgent {
    let program = ArcAdcrReplayProgram::new("demo-win", winning_demo_human_sequence())
        .expect("program should validate");
    ArcAdcrBaselineAgent::new(name, ArcAdcrConfig::replay(program)).expect("agent should build")
}

#[test]
fn adcr_baseline_replay_program_wins_locally_and_remotely() {
    let local_artifacts = {
        let environment = local_environment("adcr-local");
        let config = runner_config("adcr-baseline", 16, false);
        let mut runner = ArcInteractiveRunner::new(environment, config);
        let mut agent = replay_agent("adcr-baseline");
        runner
            .run_episode(&mut agent)
            .expect("local ADCR run should complete")
    };

    assert!(local_artifacts.report.completed);
    assert_eq!(local_artifacts.report.final_state, ArcGameState::Win);
    let Some(context_state) = local_artifacts.checkpoint_handoff.context_state.as_ref() else {
        panic!("ADCR run should retain context state");
    };
    assert!(!context_state.memory.entries.is_empty());
    assert!(context_state.memory.entries.iter().all(|entry| {
        entry
            .reasoning
            .as_ref()
            .is_some_and(|reasoning| reasoning.get("analysis").is_some())
    }));

    let server = spawn_server();
    let client = ArcRemoteClient::new(server.base_url.clone(), "interactive-adcr-key")
        .expect("remote client should initialize");
    let remote_info = client
        .list_games()
        .expect("game listing should succeed")
        .into_iter()
        .next()
        .expect("registered game should exist");
    let scorecard = client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: Some("https://example.invalid/interactive-adcr".to_owned()),
            tags: vec!["interactive-adcr".to_owned()],
            opaque: Some(json!({ "agent": "adcr-baseline" })),
            competition_mode: None,
        })
        .expect("scorecard should open");
    let environment = RemoteArcEnvironment::new(client, remote_info, scorecard.card_id);
    let config = runner_config("adcr-baseline", 16, true);
    let mut runner = ArcInteractiveRunner::new(environment, config);
    let mut agent = replay_agent("adcr-baseline");
    let remote_artifacts = runner
        .run_episode(&mut agent)
        .expect("remote ADCR run should complete");

    let parity = compare_interactive_run_artifacts(
        "adcr-baseline".to_owned(),
        &local_artifacts,
        &remote_artifacts,
        &[],
    );
    assert_eq!(parity.outcome, ArcInteractiveRunnerParityOutcome::Match);
    assert!(remote_artifacts
        .scorecard_summary
        .as_ref()
        .is_some_and(|summary| summary.published_at.is_some()));
}

#[test]
fn adcr_baseline_resume_restores_replay_cursor_and_memory() {
    let environment = local_environment("adcr-resume");
    let prefix_config = runner_config("adcr-resume-prefix", 4, false);
    let mut prefix_runner = ArcInteractiveRunner::new(environment, prefix_config);
    let mut prefix_agent = replay_agent("adcr-baseline");
    let prefix = prefix_runner
        .run_episode(&mut prefix_agent)
        .expect("prefix ADCR run should complete");

    let environment = prefix_runner.into_environment();
    let mut resume_config = runner_config("adcr-resume", 16, false);
    resume_config.resume_state = Some(prefix.checkpoint_handoff.clone());
    let mut resume_runner = ArcInteractiveRunner::new(environment, resume_config);
    let mut resume_agent = replay_agent("adcr-baseline");
    let resumed = resume_runner
        .run_episode(&mut resume_agent)
        .expect("resumed ADCR run should complete");

    assert!(resumed.report.completed);
    assert_eq!(resumed.report.final_state, ArcGameState::Win);
    let Some(context_state) = resumed.checkpoint_handoff.context_state.as_ref() else {
        panic!("resumed ADCR run should retain context state");
    };
    assert!(context_state.memory.entries.len() <= 8);
    assert!(context_state.memory.entries.iter().any(|entry| {
        entry.reasoning.as_ref().is_some_and(|reasoning| {
            reasoning
                .get("memory_notes")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|notes| !notes.is_empty())
        })
    }));
}
