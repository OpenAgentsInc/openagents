use std::collections::{BTreeMap, BTreeSet};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

use arc_client::{
    ArcCompatibilityServer, ArcEnvironmentInfo, ArcOpenScorecardRequest, ArcRegisteredEnvironment,
    ArcRemoteClient, LocalArcEnvironment, RemoteArcEnvironment,
};
use arc_core::{ArcAction, ArcScorePolicyId, ArcTaskId};
use arc_solvers::{
    compare_interactive_run_artifacts, ArcInteractiveAgent, ArcInteractiveAgentError,
    ArcInteractiveGameStep, ArcInteractiveRunArtifacts, ArcInteractiveRunner,
    ArcInteractiveRunnerConfig, ArcInteractiveRunnerExpectedDifference,
    ArcInteractiveRunnerExpectedDifferenceField, ArcInteractiveRunnerParityOutcome,
    ArcInteractiveSessionContext,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::oneshot;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

#[derive(Debug, Deserialize)]
struct ParityManifest {
    schema_version: u16,
    bounded_scope: String,
    documented_expected_differences: Vec<DocumentedExpectedDifference>,
    #[serde(default)]
    extra_expected_differences: Vec<ArcInteractiveRunnerExpectedDifference>,
    cases: Vec<ParityCase>,
}

#[derive(Debug, Deserialize)]
struct DocumentedExpectedDifference {
    field: ArcInteractiveRunnerExpectedDifferenceField,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct ParityCase {
    id: String,
    game_id: String,
    title: Option<String>,
    baseline_actions: Vec<u32>,
    package_path: String,
    checkpoint_id: String,
    checkpoint_timestamp_unix_s: u64,
    max_agent_actions: u32,
    #[serde(default)]
    resume_after_actions: Option<u32>,
    actions: Vec<ArcAction>,
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

fn spawn_server(cases: &[ParityCase]) -> TestServerHandle {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should expose a bound address");
    listener
        .set_nonblocking(true)
        .expect("listener should become non-blocking");

    let environments = cases
        .iter()
        .map(|case| {
            ArcRegisteredEnvironment::new(
                ArcEnvironmentInfo {
                    game_id: ArcTaskId::new(case.game_id.clone()).expect("task id should validate"),
                    title: case.title.clone(),
                    tags: vec!["interactive-parity".to_owned()],
                    private_tags: Vec::new(),
                    level_tags: Vec::new(),
                    baseline_actions: case.baseline_actions.clone(),
                    class_name: case.title.clone(),
                    local_package_path: None,
                },
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&case.package_path),
            )
        })
        .collect::<Vec<_>>();
    let server = ArcCompatibilityServer::new(environments);
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
fn interactive_runner_parity_manifest_covers_one_shot_and_resume_flows() {
    let manifest: ParityManifest = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("interactive_runner_parity_manifest.json"))
            .expect("interactive parity manifest should load"),
    )
    .expect("interactive parity manifest should deserialize");

    assert_eq!(manifest.schema_version, 1);
    assert!(manifest.bounded_scope.contains("checkpoint resume"));
    assert!(!manifest
        .documented_expected_differences
        .iter()
        .any(|difference| {
            difference.field == ArcInteractiveRunnerExpectedDifferenceField::CompetitionModeCoverage
        }));
    assert!(manifest
        .extra_expected_differences
        .iter()
        .any(|difference| {
            difference.field == ArcInteractiveRunnerExpectedDifferenceField::CompetitionModeCoverage
        }));

    let documented_reasons = manifest
        .documented_expected_differences
        .iter()
        .map(|difference| (serialize_field(difference.field), difference.reason.clone()))
        .collect::<BTreeMap<_, _>>();
    let documented_fields = documented_reasons.keys().cloned().collect::<BTreeSet<_>>();
    let extra_fields = manifest
        .extra_expected_differences
        .iter()
        .map(|difference| serialize_field(difference.field))
        .collect::<BTreeSet<_>>();

    let server = spawn_server(&manifest.cases);
    let client = ArcRemoteClient::new(server.base_url.clone(), "interactive-parity-test-key")
        .expect("client should initialize");
    let remote_infos = client
        .list_games()
        .expect("game list should load")
        .into_iter()
        .map(|info| (info.game_id.to_string(), info))
        .collect::<BTreeMap<_, _>>();

    for case in &manifest.cases {
        let remote_info = remote_infos
            .get(&case.game_id)
            .cloned()
            .expect("registered environment should exist");

        let one_shot_local = run_one_shot_local(case);
        let one_shot_remote = run_one_shot_remote(case, &client, remote_info.clone());
        let parity = compare_interactive_run_artifacts(
            case.id.clone(),
            &one_shot_local,
            &one_shot_remote,
            &manifest.extra_expected_differences,
        );

        assert_eq!(
            parity.outcome,
            ArcInteractiveRunnerParityOutcome::Match,
            "{}",
            serde_json::to_string_pretty(&parity).expect("parity report should serialize")
        );

        let observed_fields = parity
            .expected_differences
            .iter()
            .map(|difference| serialize_field(difference.field))
            .collect::<BTreeSet<_>>();
        let observed_documented_fields = observed_fields
            .difference(&extra_fields)
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(observed_documented_fields, documented_fields, "{}", case.id);

        for difference in &parity.expected_differences {
            if let Some(reason) = documented_reasons.get(&serialize_field(difference.field)) {
                assert!(!reason.trim().is_empty(), "{}", case.id);
            }
        }
        assert_remote_closeout_matches_report(&one_shot_remote);
        assert!(one_shot_local.scorecard_summary.is_none());

        if case.resume_after_actions.is_some() {
            let resumed_local = run_resumed_local(case);
            let resumed_remote = run_resumed_remote(case, &client, remote_info.clone());
            let resumed_parity = compare_interactive_run_artifacts(
                format!("{}::resume", case.id),
                &resumed_local.1,
                &resumed_remote.1,
                &manifest.extra_expected_differences,
            );
            assert_eq!(
                resumed_parity.outcome,
                ArcInteractiveRunnerParityOutcome::Match,
                "{}",
                serde_json::to_string_pretty(&resumed_parity)
                    .expect("resumed parity report should serialize")
            );
            assert_resume_equivalence(&one_shot_local, &resumed_local.1);
            assert_resume_equivalence(&one_shot_remote, &resumed_remote.1);
            let Some(prefix_summary) = resumed_remote.0.scorecard_summary.as_ref() else {
                panic!("prefix remote run should expose an inflight scorecard summary");
            };
            assert!(prefix_summary.published_at.is_none());
            assert_remote_closeout_matches_report(&resumed_remote.1);
        }
    }
}

fn serialize_field(field: ArcInteractiveRunnerExpectedDifferenceField) -> String {
    serde_json::to_string(&field).expect("field should serialize")
}

fn run_one_shot_local(case: &ParityCase) -> ArcInteractiveRunArtifacts {
    let environment = local_environment(case, format!("local-{}", case.id));
    let config = runner_config(
        &case.checkpoint_id,
        case.max_agent_actions,
        case.checkpoint_timestamp_unix_s,
        false,
        None,
        case.id.as_str(),
    );
    let mut runner = ArcInteractiveRunner::new(environment, config);
    let mut agent = FixedSequenceAgent::new("parity-agent", case.actions.clone());
    runner
        .run_episode(&mut agent)
        .expect("local parity run should complete")
}

fn run_one_shot_remote(
    case: &ParityCase,
    client: &ArcRemoteClient,
    remote_info: ArcEnvironmentInfo,
) -> ArcInteractiveRunArtifacts {
    let scorecard = open_scorecard(client, case, false);
    let environment = RemoteArcEnvironment::new(client.clone(), remote_info, scorecard.card_id);
    let config = runner_config(
        &case.checkpoint_id,
        case.max_agent_actions,
        case.checkpoint_timestamp_unix_s,
        true,
        None,
        case.id.as_str(),
    );
    let mut runner = ArcInteractiveRunner::new(environment, config);
    let mut agent = FixedSequenceAgent::new("parity-agent", case.actions.clone());
    runner
        .run_episode(&mut agent)
        .expect("remote parity run should complete")
}

fn run_resumed_local(
    case: &ParityCase,
) -> (ArcInteractiveRunArtifacts, ArcInteractiveRunArtifacts) {
    let resume_after = case
        .resume_after_actions
        .expect("resume case should define resume_after_actions");
    let environment = local_environment(case, format!("local-resume-{}", case.id));
    let prefix_config = runner_config(
        format!("{}-prefix", case.checkpoint_id),
        resume_after,
        case.checkpoint_timestamp_unix_s,
        false,
        None,
        case.id.as_str(),
    );
    let mut prefix_runner = ArcInteractiveRunner::new(environment, prefix_config);
    let mut prefix_agent = FixedSequenceAgent::new("parity-agent", case.actions.clone());
    let prefix = prefix_runner
        .run_episode(&mut prefix_agent)
        .expect("local prefix run should complete");
    let environment = prefix_runner.into_environment();
    let resume_config = runner_config(
        &case.checkpoint_id,
        case.max_agent_actions,
        case.checkpoint_timestamp_unix_s,
        false,
        Some(prefix.checkpoint_handoff.clone()),
        case.id.as_str(),
    );
    let mut resume_runner = ArcInteractiveRunner::new(environment, resume_config);
    let mut resume_agent = FixedSequenceAgent::new("parity-agent", case.actions.clone());
    let resumed = resume_runner
        .run_episode(&mut resume_agent)
        .expect("local resumed run should complete");
    (prefix, resumed)
}

fn run_resumed_remote(
    case: &ParityCase,
    client: &ArcRemoteClient,
    remote_info: ArcEnvironmentInfo,
) -> (ArcInteractiveRunArtifacts, ArcInteractiveRunArtifacts) {
    let resume_after = case
        .resume_after_actions
        .expect("resume case should define resume_after_actions");
    let scorecard = open_scorecard(client, case, false);
    let environment = RemoteArcEnvironment::new(client.clone(), remote_info, scorecard.card_id);
    let prefix_config = runner_config(
        format!("{}-prefix", case.checkpoint_id),
        resume_after,
        case.checkpoint_timestamp_unix_s,
        false,
        None,
        case.id.as_str(),
    );
    let mut prefix_runner = ArcInteractiveRunner::new(environment, prefix_config);
    let mut prefix_agent = FixedSequenceAgent::new("parity-agent", case.actions.clone());
    let prefix = prefix_runner
        .run_episode(&mut prefix_agent)
        .expect("remote prefix run should complete");
    let environment = prefix_runner.into_environment();
    let resume_config = runner_config(
        &case.checkpoint_id,
        case.max_agent_actions,
        case.checkpoint_timestamp_unix_s,
        true,
        Some(prefix.checkpoint_handoff.clone()),
        case.id.as_str(),
    );
    let mut resume_runner = ArcInteractiveRunner::new(environment, resume_config);
    let mut resume_agent = FixedSequenceAgent::new("parity-agent", case.actions.clone());
    let resumed = resume_runner
        .run_episode(&mut resume_agent)
        .expect("remote resumed run should complete");
    (prefix, resumed)
}

fn local_environment(case: &ParityCase, scorecard_id: String) -> LocalArcEnvironment {
    let info = ArcEnvironmentInfo {
        game_id: ArcTaskId::new(case.game_id.clone()).expect("task id should validate"),
        title: case.title.clone(),
        tags: vec!["interactive-parity".to_owned()],
        private_tags: Vec::new(),
        level_tags: Vec::new(),
        baseline_actions: case.baseline_actions.clone(),
        class_name: case.title.clone(),
        local_package_path: None,
    };
    LocalArcEnvironment::load_from_path(
        info,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&case.package_path),
        scorecard_id,
    )
    .expect("local environment should initialize")
}

fn open_scorecard(
    client: &ArcRemoteClient,
    case: &ParityCase,
    competition_mode: bool,
) -> arc_client::ArcOpenScorecardResponse {
    client
        .open_scorecard(&ArcOpenScorecardRequest {
            source_url: Some("https://example.invalid/arc-interactive-parity".to_owned()),
            tags: vec!["interactive-parity".to_owned(), case.id.clone()],
            opaque: Some(json!({ "case_id": case.id })),
            competition_mode: Some(competition_mode),
        })
        .expect("scorecard should open")
}

fn runner_config(
    checkpoint_id: impl Into<String>,
    max_agent_actions: u32,
    checkpoint_timestamp_unix_s: u64,
    close_scorecard_on_finish: bool,
    resume_state: Option<arc_solvers::ArcInteractiveCheckpointHandoff>,
    case_id: &str,
) -> ArcInteractiveRunnerConfig {
    let mut config = ArcInteractiveRunnerConfig::new(
        checkpoint_id,
        ArcScorePolicyId::ArcAgi3MethodologyV1,
        max_agent_actions,
    )
    .expect("runner config should validate");
    config.metadata.source_url = Some("https://example.invalid/arc-interactive-parity".to_owned());
    config.metadata.tags = vec!["interactive-parity".to_owned(), case_id.to_owned()];
    config.metadata.opaque = Some(json!({ "case_id": case_id }));
    config.checkpoint_timestamp_unix_s = checkpoint_timestamp_unix_s;
    config.close_scorecard_on_finish = close_scorecard_on_finish;
    config.resume_state = resume_state;
    config
}

fn assert_remote_closeout_matches_report(artifacts: &ArcInteractiveRunArtifacts) {
    let Some(summary) = artifacts.scorecard_summary.as_ref() else {
        panic!("remote parity runs should close scorecards");
    };
    assert!(summary.published_at.is_some());
    assert_eq!(summary.total_actions, Some(artifacts.report.total_actions));
    assert_eq!(summary.environments.len(), 1);
    assert_eq!(summary.environments[0].runs.len(), 1);
    assert_eq!(
        summary.environments[0].runs[0].actions,
        artifacts.report.total_actions
    );
    assert_eq!(
        summary.environments[0].runs[0].completed,
        artifacts.report.completed
    );
}

fn assert_resume_equivalence(
    baseline: &ArcInteractiveRunArtifacts,
    resumed: &ArcInteractiveRunArtifacts,
) {
    assert_eq!(baseline.report, resumed.report);
    assert_eq!(baseline.recording, resumed.recording);
    match (&baseline.execution_outcome, &resumed.execution_outcome) {
        (
            arc_core::ArcInteractiveExecutionOutcome::Completed {
                final_state: baseline_state,
                ..
            },
            arc_core::ArcInteractiveExecutionOutcome::Completed {
                final_state: resumed_state,
                ..
            },
        ) => assert_eq!(baseline_state, resumed_state),
        (
            arc_core::ArcInteractiveExecutionOutcome::Refused {
                refusal: baseline_refusal,
            },
            arc_core::ArcInteractiveExecutionOutcome::Refused {
                refusal: resumed_refusal,
            },
        ) => assert_eq!(baseline_refusal.code, resumed_refusal.code),
        (baseline_outcome, resumed_outcome) => {
            panic!(
                "resume changed terminal outcome kind: baseline={baseline_outcome:?}, resumed={resumed_outcome:?}"
            );
        }
    }
    assert_eq!(baseline.checkpoint_bundle, resumed.checkpoint_bundle);
    assert_eq!(
        baseline.checkpoint_handoff.next_step_index,
        resumed.checkpoint_handoff.next_step_index
    );
    assert_eq!(
        baseline.checkpoint_handoff.actions_taken,
        resumed.checkpoint_handoff.actions_taken
    );
    assert_eq!(
        baseline.checkpoint_handoff.terminal,
        resumed.checkpoint_handoff.terminal
    );
    assert_eq!(
        baseline.checkpoint_handoff.agent_name,
        resumed.checkpoint_handoff.agent_name
    );
    assert_eq!(
        baseline.checkpoint_handoff.agent_state,
        resumed.checkpoint_handoff.agent_state
    );
}
