use std::collections::BTreeMap;

use arc_benchmark::{
    ArcBenchmarkError, ArcBenchmarkRunKind, ArcBenchmarkRunSummary, ArcBenchmarkUsageTotals,
    ArcRepeatedRunAggregate, ArcRepeatedRunSpec, aggregate_repeated_runs,
};
use arc_core::{ArcInteractiveExecutionOutcome, ContractSerializationError, canonical_json_string};
use psionic_eval::{
    BenchmarkCase, BenchmarkExecutionMode, BenchmarkPackage, BenchmarkPackageKey, EvalArtifact,
    EvalExecutionStrategyFacts, EvalFinalStateCapture, EvalMetric, EvalRunContract, EvalRunMode,
    EvalRunState, EvalRuntimeError, EvalSampleRecord, EvalSampleStatus, EvalVerificationFacts,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    ArcInteractiveRunArtifacts, ArcInteractiveTrajectoryBundle, ArcInteractiveTrajectoryExport,
    ArcInteractiveTrajectoryExportError, arc_interactive_environment_package,
};

/// Shared ownership summary for the bounded ARC interactive eval bridge.
pub const INTERACTIVE_EVAL_BOUNDARY_SUMMARY: &str = "arc-solvers owns the bounded ARC-AGI-3 repeated-run eval bridge that launches typed interactive episodes, exports trajectory bundles, and assembles Psionic eval/benchmark sessions without replacing ARC-owned score or replay truth";

/// Configuration for one bounded repeated interactive eval program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveEvalConfig {
    /// Stable prefix used to derive round eval-run identifiers.
    pub eval_run_prefix: String,
    /// Packaged repeated benchmark contract.
    pub benchmark_package: BenchmarkPackage,
    /// Execution mode recorded in the shared Psionic benchmark session.
    pub execution_mode: BenchmarkExecutionMode,
    /// Stable experiment identifier forwarded into ARC repeated-run aggregation.
    pub experiment_id: String,
    /// Stable candidate identifier forwarded into ARC repeated-run aggregation.
    pub candidate_id: String,
    /// Output root forwarded into ARC repeated-run aggregation.
    pub output_root: String,
    /// Optional model reference for the eval contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_ref: Option<String>,
    /// Optional source reference for the eval contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    /// Optional policy revision reference for the eval contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_revision_id: Option<String>,
}

impl ArcInteractiveEvalConfig {
    /// Builds and validates one eval config.
    pub fn new(
        eval_run_prefix: impl Into<String>,
        benchmark_package: BenchmarkPackage,
        execution_mode: BenchmarkExecutionMode,
        experiment_id: impl Into<String>,
        candidate_id: impl Into<String>,
        output_root: impl Into<String>,
    ) -> Result<Self, ArcInteractiveEvalError> {
        let eval_run_prefix = normalize_field("eval_run_prefix", eval_run_prefix.into())?;
        let experiment_id = normalize_field("experiment_id", experiment_id.into())?;
        let candidate_id = normalize_field("candidate_id", candidate_id.into())?;
        let output_root = normalize_field("output_root", output_root.into())?;
        benchmark_package.validate()?;
        let expected_environment = arc_interactive_environment_package().key;
        if benchmark_package.environment != expected_environment {
            return Err(ArcInteractiveEvalError::BenchmarkEnvironmentMismatch {
                expected: expected_environment.storage_key(),
                actual: benchmark_package.environment.storage_key(),
            });
        }
        Ok(Self {
            eval_run_prefix,
            benchmark_package,
            execution_mode,
            experiment_id,
            candidate_id,
            output_root,
            model_ref: None,
            source_ref: None,
            policy_revision_id: None,
        })
    }
}

/// One case result inside one repeated interactive eval round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveEvalCaseResult {
    /// Benchmark case definition.
    pub case: BenchmarkCase,
    /// Eval sample recorded for the case.
    pub sample: EvalSampleRecord,
    /// ARC-owned trajectory bundle when the episode completed far enough to
    /// export one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trajectory: Option<ArcInteractiveTrajectoryBundle>,
}

/// Aggregate summary for one repeated interactive eval round.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveEvalRoundSummary {
    /// One-based round index.
    pub round_index: u32,
    /// Total benchmark cases in the round.
    pub total_cases: u32,
    /// Cases that reached ARC completion.
    pub completed_cases: u32,
    /// Cases that ended in an explicit ARC refusal.
    pub refused_cases: u32,
    /// Cases that errored before a trajectory export could materialize.
    pub errored_cases: u32,
    /// Fraction of cases that surfaced replayable trajectory evidence.
    pub replay_coverage_bps: u32,
    /// Round-average ARC score across scored samples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_score_bps: Option<u32>,
    /// Round pass rate copied from the finalized eval summary.
    pub pass_rate_bps: u32,
    /// Total counted ARC actions across replayed cases.
    pub total_actions: u32,
    /// Mean counted ARC actions per replayed case when any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_actions_per_replayed_case: Option<f64>,
}

/// One repeated interactive eval round with typed ARC and Psionic artifacts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveEvalRound {
    /// One-based round index.
    pub round_index: u32,
    /// Finalized shared Psionic eval run.
    pub eval_run: EvalRunState,
    /// ARC-owned round summary.
    pub round_summary: ArcInteractiveEvalRoundSummary,
    /// Typed per-case results.
    pub cases: Vec<ArcInteractiveEvalCaseResult>,
}

/// Aggregate output for one bounded repeated interactive eval program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractiveEvalAggregate {
    /// Benchmark package used for the run.
    pub benchmark_package: BenchmarkPackage,
    /// Shared benchmark execution session summary.
    pub benchmark_summary: psionic_eval::BenchmarkAggregateSummary,
    /// ARC repeated-run aggregate view over the same rounds.
    pub repeated_run_aggregate: ArcRepeatedRunAggregate,
    /// Typed per-round artifacts.
    pub rounds: Vec<ArcInteractiveEvalRound>,
    /// Completed cases across all rounds.
    pub aggregate_completed_cases: u64,
    /// Refused cases across all rounds.
    pub aggregate_refused_cases: u64,
    /// Errored cases across all rounds.
    pub aggregate_errored_cases: u64,
    /// Replay coverage across all rounds in basis points.
    pub aggregate_replay_coverage_bps: u32,
    /// Counted ARC actions across all replayed cases.
    pub aggregate_total_actions: u64,
}

/// Runs one bounded repeated interactive eval program over the shared Psionic
/// eval bridge while preserving ARC-owned episode and score semantics.
pub fn run_repeated_interactive_eval<F, E>(
    config: &ArcInteractiveEvalConfig,
    mut run_case: F,
) -> Result<ArcInteractiveEvalAggregate, ArcInteractiveEvalError>
where
    F: FnMut(u32, &BenchmarkCase) -> Result<ArcInteractiveRunArtifacts, E>,
    E: std::error::Error,
{
    let mut benchmark_session = config
        .benchmark_package
        .clone()
        .open_execution(config.execution_mode)?;
    let mut rounds = Vec::with_capacity(config.benchmark_package.repeat_count as usize);
    let mut round_summaries = Vec::with_capacity(config.benchmark_package.repeat_count as usize);

    for round_index in 1..=config.benchmark_package.repeat_count {
        let mut eval_run = EvalRunState::open(build_eval_run_contract(config, round_index))?;
        let started_at_ms = 1_000_u64.saturating_mul(u64::from(round_index));
        eval_run.start(started_at_ms)?;

        let mut case_results = Vec::with_capacity(config.benchmark_package.cases.len());
        for (ordinal, case) in config.benchmark_package.cases.iter().enumerate() {
            let case_result = match run_case(round_index, case) {
                Ok(run) => {
                    let trajectory = ArcInteractiveTrajectoryExport::from_run_artifacts(&run)?;
                    let sample = build_success_sample(case, ordinal as u64, &trajectory)?;
                    eval_run.append_sample(sample.clone())?;
                    ArcInteractiveEvalCaseResult {
                        case: case.clone(),
                        sample,
                        trajectory: Some(trajectory),
                    }
                }
                Err(error) => {
                    let sample =
                        build_error_sample(case, ordinal as u64, &error.to_string(), config);
                    eval_run.append_sample(sample.clone())?;
                    ArcInteractiveEvalCaseResult {
                        case: case.clone(),
                        sample,
                        trajectory: None,
                    }
                }
            };
            case_results.push(case_result);
        }

        let finalized_at_ms = started_at_ms
            .saturating_add(u64::try_from(case_results.len()).unwrap_or(u64::MAX))
            .saturating_add(1);
        eval_run.finalize(finalized_at_ms, Vec::new())?;
        benchmark_session.record_round(&eval_run)?;

        let round_summary = build_round_summary(round_index, &eval_run, &case_results)?;
        let run_summary = build_run_summary(config, &eval_run, &round_summary, &case_results);
        round_summaries.push(run_summary);
        rounds.push(ArcInteractiveEvalRound {
            round_index,
            eval_run,
            round_summary,
            cases: case_results,
        });
    }

    let benchmark_summary = benchmark_session.finalize()?;
    let repeated_run_aggregate = aggregate_repeated_runs(
        &ArcRepeatedRunSpec {
            benchmark: arc_core::ArcBenchmark::ArcAgi3,
            benchmark_package_key: BenchmarkPackageKey {
                benchmark_ref: config.benchmark_package.key.benchmark_ref.clone(),
                version: config.benchmark_package.key.version.clone(),
            },
            execution_mode: config.execution_mode,
            aggregation: config.benchmark_package.aggregation,
            experiment_id: config.experiment_id.clone(),
            candidate_id: config.candidate_id.clone(),
            output_root: config.output_root.clone(),
        },
        round_summaries,
    )?;

    let aggregate_completed_cases = rounds
        .iter()
        .map(|round| u64::from(round.round_summary.completed_cases))
        .sum::<u64>();
    let aggregate_refused_cases = rounds
        .iter()
        .map(|round| u64::from(round.round_summary.refused_cases))
        .sum::<u64>();
    let aggregate_errored_cases = rounds
        .iter()
        .map(|round| u64::from(round.round_summary.errored_cases))
        .sum::<u64>();
    let aggregate_total_actions = rounds
        .iter()
        .map(|round| u64::from(round.round_summary.total_actions))
        .sum::<u64>();
    let total_cases = rounds
        .iter()
        .map(|round| round.round_summary.total_cases)
        .sum::<u32>();
    let replayed_cases = rounds
        .iter()
        .map(|round| {
            round
                .cases
                .iter()
                .filter(|case| case.trajectory.is_some())
                .count() as u32
        })
        .sum::<u32>();

    Ok(ArcInteractiveEvalAggregate {
        benchmark_package: config.benchmark_package.clone(),
        benchmark_summary,
        repeated_run_aggregate,
        rounds,
        aggregate_completed_cases,
        aggregate_refused_cases,
        aggregate_errored_cases,
        aggregate_replay_coverage_bps: ratio_to_bps(replayed_cases, total_cases),
        aggregate_total_actions,
    })
}

/// Eval bridge configuration or runtime failure.
#[derive(Debug, Error)]
pub enum ArcInteractiveEvalError {
    /// One required config field was empty.
    #[error("interactive ARC eval config field `{field}` must not be empty")]
    EmptyField {
        /// Human-readable field name.
        field: &'static str,
    },
    /// The benchmark package points at the wrong environment package.
    #[error("interactive ARC benchmark package must target `{expected}`, got `{actual}`")]
    BenchmarkEnvironmentMismatch {
        /// Expected canonical environment package.
        expected: String,
        /// Actual package on the benchmark.
        actual: String,
    },
    /// ARC trajectory export failed.
    #[error(transparent)]
    TrajectoryExport(#[from] ArcInteractiveTrajectoryExportError),
    /// ARC benchmark aggregation failed.
    #[error(transparent)]
    Benchmark(#[from] ArcBenchmarkError),
    /// ARC contracts failed to serialize canonically.
    #[error(transparent)]
    Contract(#[from] ContractSerializationError),
    /// Psionic eval runtime rejected the program.
    #[error(transparent)]
    Eval(#[from] EvalRuntimeError),
}

fn build_eval_run_contract(config: &ArcInteractiveEvalConfig, round_index: u32) -> EvalRunContract {
    let mut contract = EvalRunContract::new(
        format!("{}-round-{round_index}", config.eval_run_prefix),
        EvalRunMode::Benchmark,
        config.benchmark_package.environment.clone(),
    )
    .with_benchmark_package(config.benchmark_package.key.clone())
    .with_expected_sample_count(
        u64::try_from(config.benchmark_package.cases.len()).unwrap_or(u64::MAX),
    );
    contract.model_ref = config.model_ref.clone();
    contract.source_ref = config.source_ref.clone();
    contract.policy_revision_id = config.policy_revision_id.clone();
    contract.metadata.insert(
        String::from("arc_benchmark"),
        Value::String(String::from("arc_agi3")),
    );
    contract.metadata.insert(
        String::from("round_index"),
        Value::from(u64::from(round_index)),
    );
    contract
}

fn build_success_sample(
    case: &BenchmarkCase,
    ordinal: u64,
    trajectory: &ArcInteractiveTrajectoryBundle,
) -> Result<EvalSampleRecord, ArcInteractiveEvalError> {
    let score_bps = score_to_bps(trajectory.run.report.scorecard.overall_score);
    let replay_digest = trajectory.contract_digest()?;
    let mut sample = EvalSampleRecord {
        sample_id: case.case_id.clone(),
        ordinal: Some(ordinal),
        environment: trajectory.environment_package.key.clone(),
        status: if trajectory.run.report.completed {
            EvalSampleStatus::Passed
        } else {
            EvalSampleStatus::Failed
        },
        input_ref: case.input_ref.clone(),
        output_ref: Some(format!(
            "arc://{}/trajectory/{}",
            trajectory.run.report.task_id, replay_digest
        )),
        expected_output_ref: case.expected_output_ref.clone(),
        score_bps: Some(score_bps),
        metrics: build_sample_metrics(trajectory, score_bps),
        artifacts: build_sample_artifacts(trajectory, replay_digest.as_str())?,
        error_reason: None,
        verification: Some(EvalVerificationFacts {
            timer_integrity: None,
            token_accounting: None,
            final_state: Some(EvalFinalStateCapture {
                session_digest: trajectory.session_summary.session_digest.clone(),
                output_digest: trajectory
                    .turn_receipts
                    .last()
                    .map(|receipt| receipt.output_digest.clone()),
                artifact_digests: trajectory
                    .session_summary
                    .artifacts
                    .iter()
                    .map(|artifact| artifact.artifact_digest.clone())
                    .collect(),
            }),
            execution_strategy: Some(EvalExecutionStrategyFacts {
                strategy_label: trajectory.run.checkpoint_handoff.agent_name.clone(),
                runtime_family: Some(String::from("arc_interactive_runner")),
                scheduler_posture: Some(match trajectory.run.environment_kind {
                    crate::ArcInteractiveEnvironmentKind::Local => String::from("local"),
                    crate::ArcInteractiveEnvironmentKind::Remote => String::from("remote"),
                }),
            }),
        }),
        session_digest: Some(trajectory.session_summary.session_digest.clone()),
        metadata: BTreeMap::new(),
    };
    sample.metadata.insert(
        String::from("task_id"),
        Value::String(trajectory.run.report.task_id.to_string()),
    );
    sample.metadata.insert(
        String::from("checkpoint_id"),
        Value::String(trajectory.run.checkpoint_handoff.checkpoint_id.clone()),
    );
    sample.metadata.insert(
        String::from("recording_digest"),
        Value::String(trajectory.run.report.recording_digest.clone()),
    );
    sample.metadata.insert(
        String::from("trajectory_digest"),
        Value::String(replay_digest),
    );
    sample.metadata.insert(
        String::from("execution_outcome"),
        Value::String(canonical_json_string(&trajectory.run.execution_outcome)?),
    );
    sample.metadata.insert(
        String::from("replay_locator"),
        Value::String(canonical_json_string(&trajectory.replay_locator)?),
    );
    if !case.metadata.is_null() {
        sample
            .metadata
            .insert(String::from("case_metadata"), case.metadata.clone());
    }
    if let ArcInteractiveExecutionOutcome::Refused { refusal } = &trajectory.run.execution_outcome {
        sample.metadata.insert(
            String::from("refusal_code"),
            Value::String(format!("{:?}", refusal.code)),
        );
    }
    Ok(sample)
}

fn build_error_sample(
    case: &BenchmarkCase,
    ordinal: u64,
    error: &str,
    config: &ArcInteractiveEvalConfig,
) -> EvalSampleRecord {
    let mut sample = EvalSampleRecord::errored(
        case.case_id.clone(),
        config.benchmark_package.environment.clone(),
        error.to_owned(),
    );
    sample.ordinal = Some(ordinal);
    sample.input_ref = case.input_ref.clone();
    sample.expected_output_ref = case.expected_output_ref.clone();
    if !case.metadata.is_null() {
        sample
            .metadata
            .insert(String::from("case_metadata"), case.metadata.clone());
    }
    sample
}

fn build_round_summary(
    round_index: u32,
    eval_run: &EvalRunState,
    cases: &[ArcInteractiveEvalCaseResult],
) -> Result<ArcInteractiveEvalRoundSummary, ArcInteractiveEvalError> {
    let summary = eval_run
        .summary
        .as_ref()
        .ok_or(EvalRuntimeError::SummaryMissing)?;
    let total_cases = u32::try_from(cases.len()).unwrap_or(u32::MAX);
    let completed_cases = u32::try_from(
        cases
            .iter()
            .filter(|case| {
                case.trajectory
                    .as_ref()
                    .is_some_and(|bundle| bundle.run.report.completed)
            })
            .count(),
    )
    .unwrap_or(u32::MAX);
    let refused_cases = u32::try_from(
        cases
            .iter()
            .filter(|case| {
                case.trajectory.as_ref().is_some_and(|bundle| {
                    matches!(
                        bundle.run.execution_outcome,
                        ArcInteractiveExecutionOutcome::Refused { .. }
                    )
                })
            })
            .count(),
    )
    .unwrap_or(u32::MAX);
    let errored_cases = u32::try_from(
        cases
            .iter()
            .filter(|case| case.sample.status == EvalSampleStatus::Errored)
            .count(),
    )
    .unwrap_or(u32::MAX);
    let replayed_cases = u32::try_from(
        cases
            .iter()
            .filter(|case| case.trajectory.is_some())
            .count(),
    )
    .unwrap_or(u32::MAX);
    let total_actions = cases
        .iter()
        .filter_map(|case| {
            case.trajectory
                .as_ref()
                .map(|bundle| bundle.run.report.total_actions)
        })
        .sum::<u32>();
    let mean_actions_per_replayed_case = if replayed_cases == 0 {
        None
    } else {
        Some(f64::from(total_actions) / f64::from(replayed_cases))
    };
    Ok(ArcInteractiveEvalRoundSummary {
        round_index,
        total_cases,
        completed_cases,
        refused_cases,
        errored_cases,
        replay_coverage_bps: ratio_to_bps(replayed_cases, total_cases),
        average_score_bps: summary.average_score_bps,
        pass_rate_bps: summary.pass_rate_bps,
        total_actions,
        mean_actions_per_replayed_case,
    })
}

fn build_run_summary(
    config: &ArcInteractiveEvalConfig,
    eval_run: &EvalRunState,
    round_summary: &ArcInteractiveEvalRoundSummary,
    cases: &[ArcInteractiveEvalCaseResult],
) -> ArcBenchmarkRunSummary {
    ArcBenchmarkRunSummary {
        run_id: eval_run.contract.eval_run_id.clone(),
        benchmark: arc_core::ArcBenchmark::ArcAgi3,
        run_kind: ArcBenchmarkRunKind::Interactive,
        task_id: if cases.len() == 1 {
            cases[0]
                .trajectory
                .as_ref()
                .map(|bundle| bundle.run.report.task_id.clone())
        } else {
            None
        },
        total_samples: round_summary.total_cases,
        passed_samples: round_summary.completed_cases,
        score_bps: round_summary.average_score_bps.unwrap_or(0),
        pass_rate_bps: round_summary.pass_rate_bps,
        total_actions: Some(round_summary.total_actions),
        duration_ms: eval_run
            .finalized_at_ms
            .unwrap_or_default()
            .saturating_sub(eval_run.started_at_ms.unwrap_or_default()),
        costs: ArcBenchmarkUsageTotals::default(),
        report_digest: eval_run
            .summary
            .as_ref()
            .map(|summary| summary.summary_digest.clone())
            .unwrap_or_else(|| config.benchmark_package.stable_digest()),
    }
}

fn build_sample_metrics(
    trajectory: &ArcInteractiveTrajectoryBundle,
    score_bps: u32,
) -> Vec<EvalMetric> {
    let mut metrics = vec![
        EvalMetric::new("arc_score_bps", f64::from(score_bps)).with_unit("basis_points"),
        EvalMetric::new(
            "arc_total_actions",
            f64::from(trajectory.run.report.total_actions),
        )
        .with_unit("count"),
        EvalMetric::new("arc_resets", f64::from(trajectory.run.report.resets)).with_unit("count"),
        EvalMetric::new(
            "arc_replay_coverage_bps",
            f64::from(
                if trajectory.trajectory.is_empty()
                    && matches!(
                        trajectory.run.execution_outcome,
                        ArcInteractiveExecutionOutcome::Refused { .. }
                    )
                {
                    10_000
                } else {
                    10_000
                },
            ),
        )
        .with_unit("basis_points"),
    ];
    if matches!(
        trajectory.run.execution_outcome,
        ArcInteractiveExecutionOutcome::Refused { .. }
    ) {
        metrics.push(EvalMetric::new("arc_refused", 1.0).with_unit("boolean"));
    }
    metrics
}

fn build_sample_artifacts(
    trajectory: &ArcInteractiveTrajectoryBundle,
    replay_digest: &str,
) -> Result<Vec<EvalArtifact>, ArcInteractiveEvalError> {
    let mut artifacts = vec![
        EvalArtifact::new(
            "arc_interactive_trajectory_bundle",
            format!("arc://{}/trajectory_bundle", trajectory.run.report.task_id),
            canonical_json_string(trajectory)?.as_bytes(),
        ),
        EvalArtifact::new(
            "arc_recording",
            format!("arc://{}/recording", trajectory.run.report.task_id),
            trajectory.run.recording.canonical_json()?.as_bytes(),
        ),
        EvalArtifact::new(
            "arc_scorecard",
            format!("arc://{}/scorecard", trajectory.run.report.task_id),
            canonical_json_string(&trajectory.run.report.scorecard)?.as_bytes(),
        ),
        EvalArtifact::new(
            "arc_checkpoint_bundle",
            format!(
                "arc://{}/checkpoint_bundle/{}",
                trajectory.run.report.task_id, trajectory.run.checkpoint_handoff.checkpoint_id
            ),
            canonical_json_string(&trajectory.run.checkpoint_bundle)?.as_bytes(),
        ),
        EvalArtifact::new(
            "arc_environment_session",
            format!(
                "arc://{}/environment_session",
                trajectory.run.report.task_id
            ),
            canonical_json_string(&trajectory.session_summary)?.as_bytes(),
        ),
        EvalArtifact::new(
            "arc_trajectory_digest",
            format!("arc://{}/trajectory_digest", trajectory.run.report.task_id),
            replay_digest.as_bytes(),
        ),
    ];
    if let Some(scorecard_summary) = &trajectory.run.scorecard_summary {
        artifacts.push(EvalArtifact::new(
            "arc_scorecard_summary",
            format!(
                "arc://{}/scorecard_summary/{}",
                trajectory.run.report.task_id, scorecard_summary.card_id
            ),
            canonical_json_string(scorecard_summary)?.as_bytes(),
        ));
    }
    Ok(artifacts)
}

fn normalize_field(field: &'static str, value: String) -> Result<String, ArcInteractiveEvalError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ArcInteractiveEvalError::EmptyField { field });
    }
    Ok(trimmed.to_owned())
}

fn ratio_to_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        return 0;
    }
    (((f64::from(numerator) / f64::from(denominator)) * 10_000.0).round()) as u32
}

fn score_to_bps(score: f32) -> u32 {
    (score.clamp(0.0, 1.0) * 10_000.0).round() as u32
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::PathBuf;

    use arc_client::{ArcEnvironmentInfo, LocalArcEnvironment};
    use arc_core::{ArcAction, ArcScorePolicyId, ArcTaskId};
    use psionic_eval::{
        BenchmarkAggregationKind, BenchmarkCase, BenchmarkExecutionMode, BenchmarkPackage,
        EvalSampleStatus,
    };
    use serde_json::json;

    use crate::{
        ArcInteractiveAgent, ArcInteractiveAgentError, ArcInteractiveEvalConfig,
        ArcInteractiveGameStep, ArcInteractiveRunner, ArcInteractiveRunnerConfig,
        ArcInteractiveSessionContext, arc_interactive_environment_package,
        run_repeated_interactive_eval,
    };

    #[derive(Debug)]
    struct DemoEvalError(String);

    impl std::fmt::Display for DemoEvalError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(&self.0)
        }
    }

    impl Error for DemoEvalError {}

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
            tags: vec!["interactive-eval".to_owned()],
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

    fn local_run(
        actions: Vec<ArcAction>,
        checkpoint_id: &str,
    ) -> crate::ArcInteractiveRunArtifacts {
        let environment = LocalArcEnvironment::load_from_path(
            demo_environment_info(),
            demo_package_path(),
            "eval-card",
        )
        .expect("local environment should initialize");
        let config = ArcInteractiveRunnerConfig::new(
            checkpoint_id,
            ArcScorePolicyId::ArcAgi3MethodologyV1,
            16,
        )
        .expect("runner config should validate");
        let mut runner = ArcInteractiveRunner::new(environment, config);
        let mut agent = FixedSequenceAgent::new(checkpoint_id, actions);
        runner
            .run_episode(&mut agent)
            .expect("interactive run should succeed")
    }

    #[test]
    fn repeated_interactive_eval_aggregates_replayed_rounds_over_psionic_eval() {
        let environment = arc_interactive_environment_package().key;
        let config = ArcInteractiveEvalConfig::new(
            "arc-eval-demo",
            BenchmarkPackage::new(
                psionic_eval::BenchmarkPackageKey::new(
                    "benchmark://openagents/arc/demo",
                    "2026.03.16",
                ),
                "ARC Interactive Demo",
                environment,
                2,
                BenchmarkAggregationKind::MeanScore,
            )
            .with_cases(vec![BenchmarkCase::new("case-win")]),
            BenchmarkExecutionMode::OperatorSimulation,
            "arc-406-demo",
            "candidate-a",
            "runs/arc-406-demo",
        )
        .expect("config should validate");

        let aggregate = run_repeated_interactive_eval(&config, |round_index, case| {
            let checkpoint = format!("{}-{round_index}", case.case_id);
            Ok::<_, DemoEvalError>(local_run(winning_demo_sequence(), &checkpoint))
        })
        .expect("repeated interactive eval should succeed");

        assert_eq!(aggregate.benchmark_summary.round_count, 2);
        assert_eq!(
            aggregate.benchmark_summary.aggregate_score_bps,
            Some(10_000)
        );
        assert_eq!(aggregate.repeated_run_aggregate.round_count, 2);
        assert_eq!(aggregate.aggregate_replay_coverage_bps, 10_000);
        assert_eq!(aggregate.aggregate_completed_cases, 2);
        assert_eq!(aggregate.aggregate_refused_cases, 0);
        assert_eq!(aggregate.aggregate_errored_cases, 0);
        assert_eq!(aggregate.rounds[0].cases.len(), 1);
        assert!(aggregate.rounds[0].cases[0].trajectory.is_some());
    }

    #[test]
    fn repeated_interactive_eval_surfaces_refusal_and_error_coverage() {
        let environment = arc_interactive_environment_package().key;
        let config = ArcInteractiveEvalConfig::new(
            "arc-eval-failures",
            BenchmarkPackage::new(
                psionic_eval::BenchmarkPackageKey::new(
                    "benchmark://openagents/arc/failures",
                    "2026.03.16",
                ),
                "ARC Interactive Failure Coverage",
                environment,
                1,
                BenchmarkAggregationKind::MeanScore,
            )
            .with_cases(vec![
                BenchmarkCase::new("case-win"),
                BenchmarkCase::new("case-refuse"),
                BenchmarkCase::new("case-error"),
            ]),
            BenchmarkExecutionMode::OperatorSimulation,
            "arc-406-failures",
            "candidate-b",
            "runs/arc-406-failures",
        )
        .expect("config should validate");

        let aggregate = run_repeated_interactive_eval(&config, |_round_index, case| {
            match case.case_id.as_str() {
                "case-win" => {
                    Ok::<_, DemoEvalError>(local_run(winning_demo_sequence(), "win-checkpoint"))
                }
                "case-refuse" => {
                    Ok::<_, DemoEvalError>(local_run(vec![ArcAction::Action7], "refuse-checkpoint"))
                }
                "case-error" => Err(DemoEvalError(String::from("fixture launch failed"))),
                other => Err(DemoEvalError(format!("unexpected case {other}"))),
            }
        })
        .expect("failure coverage run should still aggregate");

        let round = &aggregate.rounds[0];
        assert_eq!(round.round_summary.completed_cases, 1);
        assert_eq!(round.round_summary.refused_cases, 1);
        assert_eq!(round.round_summary.errored_cases, 1);
        assert_eq!(round.round_summary.replay_coverage_bps, 6_667);
        assert_eq!(aggregate.aggregate_refused_cases, 1);
        assert_eq!(aggregate.aggregate_errored_cases, 1);
        assert_eq!(round.cases[0].sample.status, EvalSampleStatus::Passed);
        assert_eq!(round.cases[1].sample.status, EvalSampleStatus::Failed);
        assert_eq!(round.cases[2].sample.status, EvalSampleStatus::Errored);
        assert!(round.cases[0].trajectory.is_some());
        assert!(round.cases[1].trajectory.is_some());
        assert!(round.cases[2].trajectory.is_none());
    }
}
