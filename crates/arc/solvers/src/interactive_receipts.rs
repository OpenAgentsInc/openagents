use arc_client::ArcSessionFrame;
use arc_core::{
    ArcInteractiveActionResult, ArcTaskId, ContractSerializationError, canonical_json_string,
};
use psionic_environments::{
    EnvironmentExecutionEntrypoint, EnvironmentPackageContract, EnvironmentPackageFamily,
    EnvironmentPackageKey, EnvironmentPolicyKind, EnvironmentPolicyReference,
    EnvironmentRubricHook, EnvironmentRubricOutcome, EnvironmentRubricScoreKind,
    EnvironmentRuntimeError, EnvironmentRuntimeFamily, EnvironmentSessionSummary,
    EnvironmentStateMode, EnvironmentTurnInput, EnvironmentTurnReceipt, EnvironmentWorkloadClass,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::interactive::ArcInteractiveRunArtifacts;

/// Shared ownership summary for ARC-owned trajectory bundles exported through
/// generalized Psionic environment receipts.
pub const INTERACTIVE_RECEIPTS_BOUNDARY_SUMMARY: &str = "arc-solvers owns ARC-AGI-3 trajectory bundles, replay locators, and bounded adapters into Psionic environment/session receipts without normalizing away ARC-specific action, refusal, checkpoint, or score semantics";

/// Stable ABI version for ARC-owned trajectory bundles.
pub const ARC_INTERACTIVE_TRAJECTORY_ABI_VERSION: &str = "arc.interactive_trajectory.v1";

/// Stable package version for the ARC-to-Psionic environment export.
pub const ARC_INTERACTIVE_ENVIRONMENT_VERSION: &str = "2026.03.16";

/// Final replay locator for one exported trajectory bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveReplayLocator {
    /// Environment package key used by the Psionic export.
    pub environment_package: EnvironmentPackageKey,
    /// Environment session identifier.
    pub session_id: String,
    /// Environment task identifier copied into Psionic receipts.
    pub task_id: String,
    /// Stable digest over the environment session summary.
    pub session_digest: String,
    /// Final checkpoint identifier for resume.
    pub checkpoint_id: String,
    /// Stable ARC recording digest.
    pub recording_digest: String,
    /// Final recording step index when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_step_index: Option<u32>,
    /// Psionic turn identifier when this locator points at a turn-level export.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
}

/// ARC-owned score delta preserved per exported turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveScoreDelta {
    /// Levels completed before the step.
    pub previous_levels_completed: u16,
    /// Levels completed after the step.
    pub next_levels_completed: u16,
    /// Newly completed level when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_level_index: Option<u16>,
    /// Newly materialized level score in basis points when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_level_score_bps: Option<u32>,
}

/// ARC-owned turn-level trajectory export paired with one Psionic turn receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveTrajectoryTurn {
    /// Runner step index.
    pub step_index: u32,
    /// One-based Psionic turn index.
    pub turn_index: u32,
    /// Observation before the requested action.
    pub pre_observation: ArcSessionFrame,
    /// Requested ARC action.
    pub requested_action: arc_core::ArcAction,
    /// Typed action result or refusal.
    pub result: ArcInteractiveActionResult,
    /// Observation after the action when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_observation: Option<ArcSessionFrame>,
    /// ARC-owned score delta for the step.
    pub score_delta: ArcInteractiveScoreDelta,
    /// Turn receipt emitted through the generalized Psionic path.
    pub turn_receipt: EnvironmentTurnReceipt,
    /// Canonical input payload hashed by the Psionic receipt path.
    pub input_payload: serde_json::Value,
    /// Canonical output payload hashed by the Psionic receipt path.
    pub output_payload: serde_json::Value,
    /// Replay locator for this turn.
    pub replay_locator: ArcInteractiveReplayLocator,
}

/// ARC-owned export that preserves trajectory truth alongside the generalized
/// Psionic environment/session receipt surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveTrajectoryExport {
    /// Stable ABI version for the bundle.
    pub abi_version: String,
    /// Full typed ARC run artifacts.
    pub run: ArcInteractiveRunArtifacts,
    /// Turn-level trajectory view aligned with Psionic receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trajectory: Vec<ArcInteractiveTrajectoryTurn>,
    /// Generalized Psionic environment package used for export.
    pub environment_package: EnvironmentPackageContract,
    /// Turn receipts emitted through the Psionic runtime session.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turn_receipts: Vec<EnvironmentTurnReceipt>,
    /// Final Psionic session summary.
    pub session_summary: EnvironmentSessionSummary,
    /// Final replay locator spanning ARC and Psionic evidence.
    pub replay_locator: ArcInteractiveReplayLocator,
}

/// Canonical ARC-owned trajectory bundle alias.
pub type ArcInteractiveTrajectoryBundle = ArcInteractiveTrajectoryExport;

impl ArcInteractiveTrajectoryExport {
    /// Builds an ARC-owned trajectory export plus a generalized Psionic
    /// environment/session receipt path from one interactive run.
    pub fn from_run_artifacts(
        run: &ArcInteractiveRunArtifacts,
    ) -> Result<Self, ArcInteractiveTrajectoryExportError> {
        let environment_package = arc_interactive_environment_package();
        let session_id = format!("{}-trajectory", run.checkpoint_handoff.checkpoint_id);
        let mut session = environment_package
            .clone()
            .open_session(session_id.clone(), run.report.task_id.to_string())?;
        let mut turn_receipts = Vec::with_capacity(run.turn_results.len());
        let mut trajectory = Vec::with_capacity(run.turn_results.len());
        let mut latest_observation = recording_step_to_session_frame(run, 0).ok_or_else(|| {
            ArcInteractiveTrajectoryExportError::MissingInitialRecordingStep {
                task_id: run.report.task_id.clone(),
            }
        })?;
        let mut executed_recording_index = 1_usize;
        let mut previous_levels_completed = 0_u16;

        for turn_result in &run.turn_results {
            let input_payload = serde_json::json!({
                "task_id": run.report.task_id,
                "step_index": turn_result.step_index,
                "environment_kind": run.environment_kind,
                "operation_mode": run.checkpoint_handoff.operation_mode,
                "pre_observation": latest_observation,
                "budget": turn_result.budget,
                "requested_action": turn_result.requested_action,
            });
            let score_delta =
                score_delta_for_step(run, previous_levels_completed, turn_result.step_index);
            previous_levels_completed = score_delta.next_levels_completed;
            session.begin_turn(EnvironmentTurnInput::new(canonical_json_string(
                &input_payload,
            )?))?;
            let output_payload = match &turn_result.result {
                ArcInteractiveActionResult::Executed { .. } => {
                    let recording_step = run
                        .recording
                        .steps
                        .get(executed_recording_index)
                        .ok_or_else(|| {
                            ArcInteractiveTrajectoryExportError::MissingRecordingStep {
                                task_id: run.report.task_id.clone(),
                                step_index: turn_result.step_index,
                                recording_index: executed_recording_index,
                            }
                        })?;
                    if recording_step.action != turn_result.requested_action {
                        return Err(
                            ArcInteractiveTrajectoryExportError::RecordingActionMismatch {
                                task_id: run.report.task_id.clone(),
                                step_index: turn_result.step_index,
                                expected: turn_result.requested_action.clone(),
                                actual: recording_step.action.clone(),
                            },
                        );
                    }
                    let post_observation =
                        recording_step_to_session_frame(run, executed_recording_index).ok_or_else(
                            || ArcInteractiveTrajectoryExportError::MissingRecordingStep {
                                task_id: run.report.task_id.clone(),
                                step_index: turn_result.step_index,
                                recording_index: executed_recording_index,
                            },
                        )?;
                    executed_recording_index = executed_recording_index.saturating_add(1);
                    let output_payload = serde_json::json!({
                        "result": turn_result.result,
                        "post_observation": post_observation,
                        "score_delta": score_delta,
                    });
                    let receipt = session.complete_turn(
                        canonical_json_string(&output_payload)?.as_str(),
                        turn_artifacts(
                            run,
                            turn_result.step_index,
                            Some(executed_recording_index.saturating_sub(1)),
                            &score_delta,
                            true,
                        )?,
                    )?;
                    let replay_locator = ArcInteractiveReplayLocator {
                        environment_package: environment_package.key.clone(),
                        session_id: session_id.clone(),
                        task_id: run.report.task_id.to_string(),
                        session_digest: String::new(),
                        checkpoint_id: run.checkpoint_handoff.checkpoint_id.clone(),
                        recording_digest: run.report.recording_digest.clone(),
                        recording_step_index: Some(
                            u32::try_from(executed_recording_index.saturating_sub(1))
                                .unwrap_or(u32::MAX),
                        ),
                        turn_id: Some(receipt.turn_id.clone()),
                    };
                    trajectory.push(ArcInteractiveTrajectoryTurn {
                        step_index: turn_result.step_index,
                        turn_index: receipt.turn_index,
                        pre_observation: latest_observation.clone(),
                        requested_action: turn_result.requested_action.clone(),
                        result: turn_result.result.clone(),
                        post_observation: Some(post_observation.clone()),
                        score_delta: score_delta.clone(),
                        turn_receipt: receipt.clone(),
                        input_payload: input_payload.clone(),
                        output_payload: output_payload.clone(),
                        replay_locator,
                    });
                    turn_receipts.push(receipt);
                    latest_observation = post_observation;
                    continue;
                }
                ArcInteractiveActionResult::Refused { refusal } => {
                    serde_json::json!({
                        "result": turn_result.result,
                        "refusal": refusal,
                        "score_delta": score_delta,
                    })
                }
            };

            let receipt = session.complete_turn(
                canonical_json_string(&output_payload)?.as_str(),
                turn_artifacts(run, turn_result.step_index, None, &score_delta, false)?,
            )?;
            let replay_locator = ArcInteractiveReplayLocator {
                environment_package: environment_package.key.clone(),
                session_id: session_id.clone(),
                task_id: run.report.task_id.to_string(),
                session_digest: String::new(),
                checkpoint_id: run.checkpoint_handoff.checkpoint_id.clone(),
                recording_digest: run.report.recording_digest.clone(),
                recording_step_index: executed_recording_index
                    .checked_sub(1)
                    .and_then(|index| u32::try_from(index).ok()),
                turn_id: Some(receipt.turn_id.clone()),
            };
            trajectory.push(ArcInteractiveTrajectoryTurn {
                step_index: turn_result.step_index,
                turn_index: receipt.turn_index,
                pre_observation: latest_observation.clone(),
                requested_action: turn_result.requested_action.clone(),
                result: turn_result.result.clone(),
                post_observation: None,
                score_delta,
                turn_receipt: receipt.clone(),
                input_payload,
                output_payload,
                replay_locator,
            });
            turn_receipts.push(receipt);
        }

        let session_summary = session.finalize(rubric_outcomes(run))?;
        for turn in &mut trajectory {
            turn.replay_locator.session_digest = session_summary.session_digest.clone();
        }

        let replay_locator = ArcInteractiveReplayLocator {
            environment_package: environment_package.key.clone(),
            session_id,
            task_id: run.report.task_id.to_string(),
            session_digest: session_summary.session_digest.clone(),
            checkpoint_id: run.checkpoint_handoff.checkpoint_id.clone(),
            recording_digest: run.report.recording_digest.clone(),
            recording_step_index: run
                .recording
                .steps
                .len()
                .checked_sub(1)
                .and_then(|index| u32::try_from(index).ok()),
            turn_id: turn_receipts.last().map(|receipt| receipt.turn_id.clone()),
        };

        Ok(Self {
            abi_version: String::from(ARC_INTERACTIVE_TRAJECTORY_ABI_VERSION),
            run: run.clone(),
            trajectory,
            environment_package,
            turn_receipts,
            session_summary,
            replay_locator,
        })
    }

    /// Returns a stable digest over the bundle.
    pub fn contract_digest(&self) -> Result<String, ContractSerializationError> {
        arc_core::canonical_sha256_hex(self)
    }
}

/// Export failure for ARC trajectory bundles and the generalized Psionic
/// receipt bridge.
#[derive(Debug, Error)]
pub enum ArcInteractiveTrajectoryExportError {
    /// The ARC bundle failed to serialize canonically.
    #[error(transparent)]
    Contract(#[from] ContractSerializationError),
    /// The Psionic environment runtime rejected the export.
    #[error(transparent)]
    EnvironmentRuntime(#[from] EnvironmentRuntimeError),
    /// The ARC recording omitted the initial frame.
    #[error("interactive ARC run `{task_id}` is missing the initial recording step")]
    MissingInitialRecordingStep {
        /// Task id that failed export.
        task_id: ArcTaskId,
    },
    /// An executed turn did not have a matching recording step.
    #[error(
        "interactive ARC run `{task_id}` is missing recording step {recording_index} for turn step {step_index}"
    )]
    MissingRecordingStep {
        /// Task id that failed export.
        task_id: ArcTaskId,
        /// Runner step index.
        step_index: u32,
        /// Expected recording step index.
        recording_index: usize,
    },
    /// The requested action drifted from the recording.
    #[error(
        "interactive ARC run `{task_id}` step {step_index} requested {expected:?} but recording captured {actual:?}"
    )]
    RecordingActionMismatch {
        /// Task id that failed export.
        task_id: ArcTaskId,
        /// Runner step index.
        step_index: u32,
        /// Requested action from the runner.
        expected: arc_core::ArcAction,
        /// Action recorded by the environment.
        actual: arc_core::ArcAction,
    },
}

/// Returns the generalized Psionic environment package used for ARC
/// interactive trajectory exports.
#[must_use]
pub fn arc_interactive_environment_package() -> EnvironmentPackageContract {
    EnvironmentPackageContract::new(
        EnvironmentPackageKey::new(
            "env.openagents.arc.arc_agi3.interactive_trajectory",
            ARC_INTERACTIVE_ENVIRONMENT_VERSION,
        ),
        EnvironmentPackageFamily::Agentic,
        "ARC-AGI-3 interactive trajectory export",
        EnvironmentExecutionEntrypoint {
            runtime_family: EnvironmentRuntimeFamily::MultiTurnDialog,
            entrypoint: String::from("arc_solvers::interactive_receipts::export"),
            args: Vec::new(),
            sandbox_profile_ref: None,
            max_turns: 10_000,
            state_mode: EnvironmentStateMode::SessionPersistent,
            time_budget_ms: None,
        },
    )
    .with_supported_workloads(vec![
        EnvironmentWorkloadClass::Rl,
        EnvironmentWorkloadClass::OfflineEval,
        EnvironmentWorkloadClass::ValidatorBenchmark,
    ])
    .with_rubric_hooks(vec![
        EnvironmentRubricHook {
            rubric_ref: String::from("arc.completed"),
            hook_name: String::from("arc_interactive_completion"),
            score_kind: EnvironmentRubricScoreKind::Binary,
            pass_threshold: Some(1),
        },
        EnvironmentRubricHook {
            rubric_ref: String::from("arc.score_bps"),
            hook_name: String::from("arc_interactive_score_bps"),
            score_kind: EnvironmentRubricScoreKind::Scalar,
            pass_threshold: Some(10_000),
        },
        EnvironmentRubricHook {
            rubric_ref: String::from("arc.replay_coverage_bps"),
            hook_name: String::from("arc_interactive_replay_coverage"),
            score_kind: EnvironmentRubricScoreKind::Scalar,
            pass_threshold: Some(10_000),
        },
    ])
    .with_policy_references(vec![
        EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Benchmark,
            policy_ref: String::from("arc.interactive.scorecard.v1"),
            required: true,
        },
        EnvironmentPolicyReference {
            kind: EnvironmentPolicyKind::Verification,
            policy_ref: String::from("arc.interactive.replay.v1"),
            required: true,
        },
    ])
}

fn rubric_outcomes(run: &ArcInteractiveRunArtifacts) -> Vec<EnvironmentRubricOutcome> {
    vec![
        EnvironmentRubricOutcome {
            rubric_ref: String::from("arc.completed"),
            score_value: i32::from(run.report.completed),
            passed: run.report.completed,
        },
        EnvironmentRubricOutcome {
            rubric_ref: String::from("arc.score_bps"),
            score_value: i32::try_from(score_to_bps(run.report.scorecard.overall_score))
                .unwrap_or(i32::MAX),
            passed: run.report.completed,
        },
        EnvironmentRubricOutcome {
            rubric_ref: String::from("arc.replay_coverage_bps"),
            score_value: 10_000,
            passed: true,
        },
    ]
}

fn turn_artifacts(
    run: &ArcInteractiveRunArtifacts,
    step_index: u32,
    recording_step_index: Option<usize>,
    score_delta: &ArcInteractiveScoreDelta,
    executed: bool,
) -> Result<Vec<psionic_environments::EnvironmentArtifactOutput>, ContractSerializationError> {
    let mut artifacts = Vec::new();
    artifacts.push(psionic_environments::EnvironmentArtifactOutput::new(
        "arc.interactive.score_delta",
        format!("arc://{}/score_delta/{step_index}", run.report.task_id),
        canonical_json_string(score_delta)?.as_bytes(),
    ));
    if let Some(recording_step_index) = recording_step_index {
        if let Some(step) = run.recording.steps.get(recording_step_index) {
            artifacts.push(psionic_environments::EnvironmentArtifactOutput::new(
                "arc.interactive.recording_step",
                format!(
                    "arc://{}/recording_step/{recording_step_index}",
                    run.report.task_id
                ),
                canonical_json_string(step)?.as_bytes(),
            ));
        }
    }
    if executed {
        artifacts.push(psionic_environments::EnvironmentArtifactOutput::new(
            "arc.interactive.checkpoint_handoff",
            format!(
                "arc://{}/checkpoint_handoff/{}",
                run.report.task_id, step_index
            ),
            canonical_json_string(&run.checkpoint_handoff)?.as_bytes(),
        ));
    }
    Ok(artifacts)
}

fn score_delta_for_step(
    run: &ArcInteractiveRunArtifacts,
    previous_levels_completed: u16,
    step_index: u32,
) -> ArcInteractiveScoreDelta {
    let Some(step_summary) = run
        .report
        .step_summaries
        .iter()
        .find(|summary| summary.step_index == step_index)
    else {
        return ArcInteractiveScoreDelta {
            previous_levels_completed,
            next_levels_completed: previous_levels_completed,
            completed_level_index: None,
            completed_level_score_bps: None,
        };
    };
    ArcInteractiveScoreDelta {
        previous_levels_completed,
        next_levels_completed: step_summary.levels_completed,
        completed_level_index: step_summary.completed_level_index,
        completed_level_score_bps: step_summary.completed_level_score.map(score_to_bps),
    }
}

fn recording_step_to_session_frame(
    run: &ArcInteractiveRunArtifacts,
    recording_index: usize,
) -> Option<ArcSessionFrame> {
    let step = run.recording.steps.get(recording_index)?;
    Some(ArcSessionFrame {
        game_id: run.report.task_id.clone(),
        guid: run
            .checkpoint_handoff
            .session_guid
            .clone()
            .unwrap_or_else(|| format!("{}-recording", run.report.task_id)),
        frames: vec![step.observation.frame.clone()],
        game_state: step.observation.game_state,
        levels_completed: step.levels_completed,
        win_levels: step.win_levels,
        action: step.action.clone(),
        available_actions: step.observation.available_actions.clone(),
        full_reset: step.full_reset,
    })
}

fn score_to_bps(score: f32) -> u32 {
    let bounded = score.clamp(0.0, 1.0);
    (bounded * 10_000.0).round() as u32
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::thread;

    use arc_client::{
        ArcCompatibilityServer, ArcEnvironmentInfo, ArcOpenScorecardRequest,
        ArcRegisteredEnvironment, ArcRemoteClient, LocalArcEnvironment, RemoteArcEnvironment,
    };
    use arc_core::{ArcAction, ArcGameState, ArcScorePolicyId, ArcTaskId};
    use serde_json::json;
    use tokio::sync::oneshot;

    use crate::{
        ArcInteractiveAgent, ArcInteractiveAgentError, ArcInteractiveGameStep,
        ArcInteractiveRunner, ArcInteractiveRunnerConfig, ArcInteractiveSessionContext,
    };

    use super::{
        ArcInteractiveTrajectoryExport, ArcInteractiveTrajectoryExportError,
        arc_interactive_environment_package,
    };

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
            tags: vec!["interactive-receipts".to_owned()],
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
        let environment =
            ArcRegisteredEnvironment::new(demo_environment_info(), demo_package_path());
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

    fn local_run_artifacts() -> crate::ArcInteractiveRunArtifacts {
        let environment = LocalArcEnvironment::load_from_path(
            demo_environment_info(),
            demo_package_path(),
            "local-card",
        )
        .expect("local environment should initialize");
        let config = ArcInteractiveRunnerConfig::new(
            "trajectory-checkpoint",
            ArcScorePolicyId::ArcAgi3MethodologyV1,
            16,
        )
        .expect("runner config should validate");
        let mut runner = ArcInteractiveRunner::new(environment, config);
        let mut agent = FixedSequenceAgent::new("fixed-local", winning_demo_sequence());
        runner
            .run_episode(&mut agent)
            .expect("local trajectory run should succeed")
    }

    #[test]
    fn trajectory_export_materializes_psionic_receipts_for_local_runs() {
        let export = ArcInteractiveTrajectoryExport::from_run_artifacts(&local_run_artifacts())
            .expect("trajectory export should succeed");

        assert_eq!(export.abi_version, "arc.interactive_trajectory.v1");
        assert_eq!(
            export.environment_package.key,
            arc_interactive_environment_package().key
        );
        assert_eq!(export.turn_receipts.len(), export.run.turn_results.len());
        assert_eq!(export.session_summary.turn_count, 12);
        assert_eq!(export.session_summary.tool_invocation_count, 0);
        assert_eq!(export.trajectory.len(), 12);
        assert_eq!(export.trajectory[0].turn_receipt.turn_index, 1);
        assert_eq!(export.trajectory[0].requested_action, ArcAction::Action4);
        assert_eq!(
            export
                .trajectory
                .last()
                .and_then(|turn| turn.post_observation.as_ref())
                .map(|frame| frame.game_state),
            Some(ArcGameState::Win)
        );
        assert_eq!(
            export.replay_locator.session_digest,
            export.session_summary.session_digest
        );
        assert!(export.contract_digest().is_ok());
    }

    #[test]
    fn trajectory_export_preserves_refusal_turns_and_remote_resume_locators() {
        let server = spawn_server();
        let client = ArcRemoteClient::new(server.base_url.clone(), "trajectory-export-test-key")
            .expect("remote client should initialize");
        let scorecard = client
            .open_scorecard(&ArcOpenScorecardRequest {
                source_url: Some("https://example.invalid/arc-trajectory-export".to_owned()),
                tags: vec!["interactive-receipts".to_owned()],
                opaque: Some(json!({ "kind": "refusal-export" })),
                competition_mode: None,
            })
            .expect("scorecard should open");
        let games = client.list_games().expect("game listing should succeed");
        let environment = RemoteArcEnvironment::new(client, games[0].clone(), scorecard.card_id);
        let config = ArcInteractiveRunnerConfig::new(
            "trajectory-refusal",
            ArcScorePolicyId::ArcAgi3MethodologyV1,
            1,
        )
        .expect("runner config should validate");
        let mut runner = ArcInteractiveRunner::new(environment, config);
        let mut agent = FixedSequenceAgent::new("invalid-agent", vec![ArcAction::Action7]);
        let run = runner
            .run_episode(&mut agent)
            .expect("invalid action should still materialize artifacts");
        let export = ArcInteractiveTrajectoryExport::from_run_artifacts(&run)
            .expect("trajectory export should preserve refusal turns");

        assert_eq!(export.trajectory.len(), 1);
        assert!(matches!(
            export.trajectory[0].result,
            arc_core::ArcInteractiveActionResult::Refused { .. }
        ));
        assert_eq!(
            export.trajectory[0].replay_locator.recording_step_index,
            Some(0)
        );
        assert_eq!(export.turn_receipts[0].turn_index, 1);
        assert_eq!(export.session_summary.turn_count, 1);
    }

    #[test]
    fn trajectory_export_rejects_drift_between_turn_results_and_recording() {
        let mut run = local_run_artifacts();
        run.recording.steps[1].action = ArcAction::Action5;
        let error = ArcInteractiveTrajectoryExport::from_run_artifacts(&run)
            .expect_err("mismatched recording actions should refuse export");
        assert!(matches!(
            error,
            ArcInteractiveTrajectoryExportError::RecordingActionMismatch { .. }
        ));
    }
}
