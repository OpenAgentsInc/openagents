use arc_core::{ArcEpisodeStep, ArcLevelScore, ArcScorecardMetadata};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::interactive::ArcInteractiveRunArtifacts;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveRunnerParityReport {
    pub case_id: String,
    pub compared_turns: u32,
    pub local: ArcInteractiveRunArtifacts,
    pub remote: ArcInteractiveRunArtifacts,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expected_differences: Vec<ArcInteractiveRunnerExpectedDifference>,
    pub outcome: ArcInteractiveRunnerParityOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveRunnerExpectedDifference {
    pub field: ArcInteractiveRunnerExpectedDifferenceField,
    pub local: String,
    pub remote: String,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcInteractiveRunnerExpectedDifferenceField {
    EnvironmentKind,
    ReportOperationMode,
    RecordingOperationMode,
    ScorecardId,
    SessionGuid,
    LocalPackagePath,
    ScorecardSummary,
    RecordingDigest,
    CheckpointRecordingDigest,
    CompetitionModeCoverage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcInteractiveRunnerParityOutcome {
    Match,
    Mismatch(ArcInteractiveRunnerParityMismatch),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveRunnerParityMismatch {
    pub field: ArcInteractiveRunnerParityField,
    pub local: String,
    pub remote: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcInteractiveRunnerParityField {
    EnvironmentInfo,
    ReportSummary,
    ReportStepSummaries,
    RecordingBody,
    TurnResults,
    ExecutionOutcome,
    CheckpointHandoff,
    CheckpointBundle,
}

#[must_use]
pub fn compare_interactive_run_artifacts(
    case_id: impl Into<String>,
    local: &ArcInteractiveRunArtifacts,
    remote: &ArcInteractiveRunArtifacts,
    extra_expected_differences: &[ArcInteractiveRunnerExpectedDifference],
) -> ArcInteractiveRunnerParityReport {
    let expected_differences =
        collect_expected_differences(local, remote, extra_expected_differences);
    let outcome = find_first_mismatch(local, remote).map_or(
        ArcInteractiveRunnerParityOutcome::Match,
        ArcInteractiveRunnerParityOutcome::Mismatch,
    );

    ArcInteractiveRunnerParityReport {
        case_id: case_id.into(),
        compared_turns: u32::try_from(local.turn_results.len().min(remote.turn_results.len()))
            .unwrap_or(u32::MAX),
        local: local.clone(),
        remote: remote.clone(),
        expected_differences,
        outcome,
    }
}

fn collect_expected_differences(
    local: &ArcInteractiveRunArtifacts,
    remote: &ArcInteractiveRunArtifacts,
    extra_expected_differences: &[ArcInteractiveRunnerExpectedDifference],
) -> Vec<ArcInteractiveRunnerExpectedDifference> {
    let mut differences = Vec::new();
    push_difference(
        &mut differences,
        ArcInteractiveRunnerExpectedDifferenceField::EnvironmentKind,
        &local.environment_kind,
        &remote.environment_kind,
        "local runs use the offline wrapper while remote runs use the compatibility-server client surface",
    );
    push_difference(
        &mut differences,
        ArcInteractiveRunnerExpectedDifferenceField::ReportOperationMode,
        &local.report.operation_mode,
        &remote.report.operation_mode,
        "local recordings score under offline mode and remote recordings score under online mode",
    );
    push_difference(
        &mut differences,
        ArcInteractiveRunnerExpectedDifferenceField::RecordingOperationMode,
        &local.recording.operation_mode,
        &remote.recording.operation_mode,
        "recording envelopes preserve the environment operation mode instead of normalizing it away",
    );
    push_difference(
        &mut differences,
        ArcInteractiveRunnerExpectedDifferenceField::ScorecardId,
        &local.checkpoint_handoff.scorecard_id,
        &remote.checkpoint_handoff.scorecard_id,
        "local scorecard ids are runner-owned fixture ids while remote ids come from the scorecard service",
    );
    push_difference(
        &mut differences,
        ArcInteractiveRunnerExpectedDifferenceField::SessionGuid,
        &local.checkpoint_handoff.session_guid,
        &remote.checkpoint_handoff.session_guid,
        "local and remote sessions mint independent environment guids",
    );
    push_difference(
        &mut differences,
        ArcInteractiveRunnerExpectedDifferenceField::LocalPackagePath,
        &local.info.local_package_path,
        &remote.info.local_package_path,
        "local environments retain fixture package paths while remote game listings omit local file-system details",
    );
    push_difference(
        &mut differences,
        ArcInteractiveRunnerExpectedDifferenceField::ScorecardSummary,
        &local.scorecard_summary,
        &remote.scorecard_summary,
        "only the remote scorecard surface materializes a closeout summary",
    );
    push_difference(
        &mut differences,
        ArcInteractiveRunnerExpectedDifferenceField::RecordingDigest,
        &local.report.recording_digest,
        &remote.report.recording_digest,
        "recording digests differ because operation mode is part of the recording contract",
    );
    push_difference(
        &mut differences,
        ArcInteractiveRunnerExpectedDifferenceField::CheckpointRecordingDigest,
        &local.checkpoint_bundle.metadata.recording_digest,
        &remote.checkpoint_bundle.metadata.recording_digest,
        "checkpoint metadata preserves the recording digest from the environment-specific run contract",
    );
    differences.extend_from_slice(extra_expected_differences);
    differences
}

fn push_difference<T>(
    differences: &mut Vec<ArcInteractiveRunnerExpectedDifference>,
    field: ArcInteractiveRunnerExpectedDifferenceField,
    local: &T,
    remote: &T,
    reason: &str,
) where
    T: Serialize + PartialEq,
{
    if local == remote {
        return;
    }
    differences.push(ArcInteractiveRunnerExpectedDifference {
        field,
        local: serialize_debuggable(local),
        remote: serialize_debuggable(remote),
        reason: reason.to_owned(),
    });
}

fn find_first_mismatch(
    local: &ArcInteractiveRunArtifacts,
    remote: &ArcInteractiveRunArtifacts,
) -> Option<ArcInteractiveRunnerParityMismatch> {
    if comparable_environment_info(local) != comparable_environment_info(remote) {
        return Some(mismatch(
            ArcInteractiveRunnerParityField::EnvironmentInfo,
            &comparable_environment_info(local),
            &comparable_environment_info(remote),
        ));
    }
    if comparable_report_summary(local) != comparable_report_summary(remote) {
        return Some(mismatch(
            ArcInteractiveRunnerParityField::ReportSummary,
            &comparable_report_summary(local),
            &comparable_report_summary(remote),
        ));
    }
    if local.report.step_summaries != remote.report.step_summaries {
        return Some(mismatch(
            ArcInteractiveRunnerParityField::ReportStepSummaries,
            &local.report.step_summaries,
            &remote.report.step_summaries,
        ));
    }
    if comparable_recording(local) != comparable_recording(remote) {
        return Some(mismatch(
            ArcInteractiveRunnerParityField::RecordingBody,
            &comparable_recording(local),
            &comparable_recording(remote),
        ));
    }
    if local.turn_results != remote.turn_results {
        return Some(mismatch(
            ArcInteractiveRunnerParityField::TurnResults,
            &local.turn_results,
            &remote.turn_results,
        ));
    }
    if local.execution_outcome != remote.execution_outcome {
        return Some(mismatch(
            ArcInteractiveRunnerParityField::ExecutionOutcome,
            &local.execution_outcome,
            &remote.execution_outcome,
        ));
    }
    if comparable_checkpoint_handoff(local) != comparable_checkpoint_handoff(remote) {
        return Some(mismatch(
            ArcInteractiveRunnerParityField::CheckpointHandoff,
            &comparable_checkpoint_handoff(local),
            &comparable_checkpoint_handoff(remote),
        ));
    }
    if comparable_checkpoint_bundle(local) != comparable_checkpoint_bundle(remote) {
        return Some(mismatch(
            ArcInteractiveRunnerParityField::CheckpointBundle,
            &comparable_checkpoint_bundle(local),
            &comparable_checkpoint_bundle(remote),
        ));
    }
    None
}

#[derive(Serialize, PartialEq)]
struct ComparableEnvironmentInfo<'a> {
    game_id: &'a arc_core::ArcTaskId,
    title: &'a Option<String>,
    tags: &'a Vec<String>,
    private_tags: &'a Vec<String>,
    level_tags: &'a Vec<Vec<String>>,
    baseline_actions: &'a Vec<u32>,
    class_name: &'a Option<String>,
}

fn comparable_environment_info(
    artifacts: &ArcInteractiveRunArtifacts,
) -> ComparableEnvironmentInfo<'_> {
    ComparableEnvironmentInfo {
        game_id: &artifacts.info.game_id,
        title: &artifacts.info.title,
        tags: &artifacts.info.tags,
        private_tags: &artifacts.info.private_tags,
        level_tags: &artifacts.info.level_tags,
        baseline_actions: &artifacts.info.baseline_actions,
        class_name: &artifacts.info.class_name,
    }
}

#[derive(Serialize, PartialEq)]
struct ComparableReportSummary<'a> {
    total_actions: u32,
    resets: u32,
    levels_completed: u16,
    win_levels: u16,
    completed: bool,
    final_state: arc_core::ArcGameState,
    score_policy_id: arc_core::ArcScorePolicyId,
    metadata: &'a ArcScorecardMetadata,
    level_scores: &'a Vec<ArcLevelScore>,
    overall_score: f32,
}

fn comparable_report_summary(
    artifacts: &ArcInteractiveRunArtifacts,
) -> ComparableReportSummary<'_> {
    ComparableReportSummary {
        total_actions: artifacts.report.total_actions,
        resets: artifacts.report.resets,
        levels_completed: artifacts.report.levels_completed,
        win_levels: artifacts.report.win_levels,
        completed: artifacts.report.completed,
        final_state: artifacts.report.final_state,
        score_policy_id: artifacts.report.score_policy_id,
        metadata: &artifacts.report.scorecard.metadata,
        level_scores: &artifacts.report.scorecard.levels,
        overall_score: artifacts.report.scorecard.overall_score,
    }
}

#[derive(Serialize, PartialEq)]
struct ComparableRecording<'a> {
    benchmark: arc_core::ArcBenchmark,
    task_id: &'a arc_core::ArcTaskId,
    score_policy_id: &'a Option<arc_core::ArcScorePolicyId>,
    steps: &'a Vec<ArcEpisodeStep>,
}

fn comparable_recording(artifacts: &ArcInteractiveRunArtifacts) -> ComparableRecording<'_> {
    ComparableRecording {
        benchmark: artifacts.recording.benchmark,
        task_id: &artifacts.recording.task_id,
        score_policy_id: &artifacts.recording.score_policy_id,
        steps: &artifacts.recording.steps,
    }
}

#[derive(Serialize, PartialEq)]
struct ComparableCheckpointHandoff<'a> {
    next_step_index: u32,
    actions_taken: u32,
    terminal: bool,
    agent_name: &'a str,
    agent_state: &'a Option<Value>,
}

fn comparable_checkpoint_handoff(
    artifacts: &ArcInteractiveRunArtifacts,
) -> ComparableCheckpointHandoff<'_> {
    ComparableCheckpointHandoff {
        next_step_index: artifacts.checkpoint_handoff.next_step_index,
        actions_taken: artifacts.checkpoint_handoff.actions_taken,
        terminal: artifacts.checkpoint_handoff.terminal,
        agent_name: &artifacts.checkpoint_handoff.agent_name,
        agent_state: &artifacts.checkpoint_handoff.agent_state,
    }
}

#[derive(Serialize, PartialEq)]
struct ComparableCheckpointBundle<'a> {
    checkpoint_id: &'a str,
    score_policy_id: arc_core::ArcScorePolicyId,
    total_actions: u32,
    resets: u32,
    levels_completed: u16,
    win_levels: u16,
    final_state: arc_core::ArcGameState,
    step_count: u32,
    recording: ComparableRecording<'a>,
    level_scores: &'a Vec<ArcLevelScore>,
    overall_score: f32,
    step_summaries: &'a Vec<arc_benchmark::ArcInteractiveStepSummary>,
}

fn comparable_checkpoint_bundle(
    artifacts: &ArcInteractiveRunArtifacts,
) -> ComparableCheckpointBundle<'_> {
    ComparableCheckpointBundle {
        checkpoint_id: &artifacts.checkpoint_bundle.metadata.checkpoint_id,
        score_policy_id: artifacts.checkpoint_bundle.metadata.score_policy_id,
        total_actions: artifacts.checkpoint_bundle.metadata.total_actions,
        resets: artifacts.checkpoint_bundle.metadata.resets,
        levels_completed: artifacts.checkpoint_bundle.metadata.levels_completed,
        win_levels: artifacts.checkpoint_bundle.metadata.win_levels,
        final_state: artifacts.checkpoint_bundle.metadata.final_state,
        step_count: artifacts.checkpoint_bundle.metadata.step_count,
        recording: ComparableRecording {
            benchmark: artifacts.checkpoint_bundle.recording.benchmark,
            task_id: &artifacts.checkpoint_bundle.recording.task_id,
            score_policy_id: &artifacts.checkpoint_bundle.recording.score_policy_id,
            steps: &artifacts.checkpoint_bundle.recording.steps,
        },
        level_scores: &artifacts.checkpoint_bundle.scorecard.levels,
        overall_score: artifacts.checkpoint_bundle.scorecard.overall_score,
        step_summaries: &artifacts.checkpoint_bundle.step_summaries,
    }
}

fn mismatch<T>(
    field: ArcInteractiveRunnerParityField,
    local: &T,
    remote: &T,
) -> ArcInteractiveRunnerParityMismatch
where
    T: Serialize,
{
    ArcInteractiveRunnerParityMismatch {
        field,
        local: serialize_debuggable(local),
        remote: serialize_debuggable(remote),
    }
}

fn serialize_debuggable<T>(value: &T) -> String
where
    T: Serialize,
{
    serde_json::to_string(value).unwrap_or_else(|_| "<serialization-error>".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interactive::ArcInteractiveEnvironmentKind;
    use arc_core::ArcOperationMode;

    #[test]
    fn expected_difference_collection_marks_operation_mode_and_summary_boundaries() {
        let mut local = base_artifacts();
        let mut remote = base_artifacts();
        local.environment_kind = ArcInteractiveEnvironmentKind::Local;
        remote.environment_kind = ArcInteractiveEnvironmentKind::Remote;
        local.report.operation_mode = Some(ArcOperationMode::Offline);
        remote.report.operation_mode = Some(ArcOperationMode::Online);
        local.recording.operation_mode = Some(ArcOperationMode::Offline);
        remote.recording.operation_mode = Some(ArcOperationMode::Online);
        local.checkpoint_handoff.scorecard_id = "local-card".to_owned();
        remote.checkpoint_handoff.scorecard_id = "remote-card".to_owned();
        remote.scorecard_summary = Some(arc_client::ArcScorecardSummary {
            card_id: "remote-card".to_owned(),
            score: 0,
            source_url: None,
            tags: Vec::new(),
            opaque: None,
            user_name: None,
            user_id: None,
            published_at: None,
            open_at: None,
            last_update: None,
            total_environments_completed: None,
            total_environments: None,
            total_levels_completed: None,
            total_levels: None,
            total_actions: Some(0),
            environments: Vec::new(),
            tags_scores: Vec::new(),
        });
        let report =
            compare_interactive_run_artifacts("expected-differences", &local, &remote, &[]);
        let fields = report
            .expected_differences
            .iter()
            .map(|difference| difference.field)
            .collect::<Vec<_>>();
        assert!(fields.contains(&ArcInteractiveRunnerExpectedDifferenceField::EnvironmentKind));
        assert!(fields.contains(&ArcInteractiveRunnerExpectedDifferenceField::ReportOperationMode));
        assert!(
            fields.contains(&ArcInteractiveRunnerExpectedDifferenceField::RecordingOperationMode)
        );
        assert!(fields.contains(&ArcInteractiveRunnerExpectedDifferenceField::ScorecardId));
        assert!(fields.contains(&ArcInteractiveRunnerExpectedDifferenceField::ScorecardSummary));
    }

    fn base_artifacts() -> ArcInteractiveRunArtifacts {
        let task_id = arc_core::ArcTaskId::new("parity-fixture").expect("task id should validate");
        let frame =
            arc_core::ArcFrameData::new(1, 1, vec![0]).expect("frame fixture should validate");
        let recording = arc_core::ArcRecording::new(
            arc_core::ArcBenchmark::ArcAgi3,
            task_id.clone(),
            vec![arc_core::ArcEpisodeStep {
                step_index: 0,
                action: arc_core::ArcAction::Reset,
                observation: arc_core::ArcObservation {
                    frame: frame.clone(),
                    available_actions: vec![arc_core::ArcActionKind::Action1],
                    game_state: arc_core::ArcGameState::NotFinished,
                },
                levels_completed: 0,
                win_levels: 1,
                terminal: false,
                full_reset: true,
            }],
        )
        .expect("recording should validate");
        let scorecard = arc_core::ArcScorecard {
            benchmark: arc_core::ArcBenchmark::ArcAgi3,
            task_id: task_id.clone(),
            overall_score: 0.0,
            operation_mode: Some(ArcOperationMode::Offline),
            score_policy_id: Some(arc_core::ArcScorePolicyId::ArcAgi3MethodologyV1),
            recording_envelope_id: None,
            metadata: ArcScorecardMetadata {
                source_url: None,
                tags: vec!["parity".to_owned()],
                opaque: None,
            },
            levels: vec![arc_core::ArcLevelScore {
                level_index: 1,
                action_count: 0,
                score: 0.0,
            }],
        };
        let report = arc_benchmark::ArcInteractiveRunReport {
            benchmark: arc_core::ArcBenchmark::ArcAgi3,
            task_id: task_id.clone(),
            operation_mode: Some(ArcOperationMode::Offline),
            score_policy_id: arc_core::ArcScorePolicyId::ArcAgi3MethodologyV1,
            recording_digest: recording.contract_digest().expect("digest should compute"),
            total_actions: 0,
            resets: 0,
            levels_completed: 0,
            win_levels: 1,
            completed: false,
            final_state: arc_core::ArcGameState::NotFinished,
            scorecard: scorecard.clone(),
            step_summaries: vec![arc_benchmark::ArcInteractiveStepSummary {
                step_index: 0,
                action: arc_core::ArcAction::Reset,
                total_actions: 0,
                resets: 0,
                levels_completed: 0,
                win_levels: 1,
                game_state: arc_core::ArcGameState::NotFinished,
                active_level_index: Some(1),
                current_level_action_count: 0,
                completed_level_index: None,
                completed_level_action_count: None,
                completed_level_score: None,
                full_reset: true,
                terminal: false,
            }],
        };
        let checkpoint_bundle = arc_benchmark::ArcInteractiveCheckpointBundle::from_run_report(
            "parity-checkpoint",
            &report,
            arc_benchmark::ArcBenchmarkUsageTotals::default(),
            1_735_689_600,
            recording.clone(),
        )
        .expect("checkpoint bundle should validate");
        ArcInteractiveRunArtifacts {
            environment_kind: ArcInteractiveEnvironmentKind::Local,
            info: arc_client::ArcEnvironmentInfo {
                game_id: task_id,
                title: Some("Parity Fixture".to_owned()),
                tags: vec!["parity".to_owned()],
                private_tags: Vec::new(),
                level_tags: Vec::new(),
                baseline_actions: vec![1],
                class_name: Some("ParityFixture".to_owned()),
                local_package_path: None,
            },
            report,
            recording,
            checkpoint_bundle,
            checkpoint_handoff: crate::interactive::ArcInteractiveCheckpointHandoff {
                checkpoint_id: "parity-checkpoint".to_owned(),
                game_id: arc_core::ArcTaskId::new("parity-fixture")
                    .expect("task id should validate"),
                scorecard_id: "local-card".to_owned(),
                environment_kind: ArcInteractiveEnvironmentKind::Local,
                operation_mode: ArcOperationMode::Offline,
                session_guid: Some("local-guid".to_owned()),
                next_step_index: 1,
                actions_taken: 0,
                terminal: false,
                agent_name: "parity-agent".to_owned(),
                agent_state: Some(serde_json::json!({ "cursor": 0 })),
            },
            turn_results: Vec::new(),
            execution_outcome: arc_core::ArcInteractiveExecutionOutcome::Refused {
                refusal: arc_core::ArcInteractiveRefusal {
                    code: arc_core::ArcInteractiveRefusalCode::BudgetExhausted,
                    step_index: 1,
                    action: None,
                    detail: "fixture".to_owned(),
                },
            },
            scorecard_summary: None,
        }
    }
}
