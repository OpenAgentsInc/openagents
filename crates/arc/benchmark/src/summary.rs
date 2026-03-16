use arc_core::{ArcBenchmark, ArcTaskId, canonical_sha256_hex};
use psionic_eval::{
    BenchmarkAggregateSummary, BenchmarkAggregationKind, BenchmarkExecutionMode,
    BenchmarkPackageKey,
};
use psionic_research::{
    CandidateMutation, ExperimentArtifactKind, ExperimentArtifactRef, ExperimentBudget,
    ExperimentFamily, ExperimentFamilyKind, ExperimentMetric, ExperimentReceiptKind,
    ExperimentReceiptRef, ExperimentResult, ExperimentRunStatus, ExperimentRuntimeProfile,
    ExperimentScore, ExperimentScoreContract, ExperimentScoreEvaluation, ExperimentSpec,
    ScoreDirection, ScoreMetricSpec, ValidatorPolicySpec,
};
use serde::{Deserialize, Serialize};

use crate::{
    ArcBenchmarkError, ArcBenchmarkUsageTotals, ArcExactMatchBenchmarkSummary,
    ArcInteractiveRunReport,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcBenchmarkRunKind {
    ExactMatch,
    Interactive,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcBenchmarkRunSummary {
    pub run_id: String,
    pub benchmark: ArcBenchmark,
    pub run_kind: ArcBenchmarkRunKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<ArcTaskId>,
    pub total_samples: u32,
    pub passed_samples: u32,
    pub score_bps: u32,
    pub pass_rate_bps: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_actions: Option<u32>,
    pub duration_ms: u64,
    pub costs: ArcBenchmarkUsageTotals,
    pub report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcRepeatedRunSpec {
    pub benchmark: ArcBenchmark,
    pub benchmark_package_key: BenchmarkPackageKey,
    pub execution_mode: BenchmarkExecutionMode,
    pub aggregation: BenchmarkAggregationKind,
    pub experiment_id: String,
    pub candidate_id: String,
    pub output_root: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcRepeatedRunAggregate {
    pub benchmark: ArcBenchmark,
    pub round_count: u32,
    pub aggregate_score_bps: u32,
    pub aggregate_pass_rate_bps: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_actions: Option<f64>,
    pub total_duration_ms: u64,
    pub total_cost_usd: f64,
    pub eval_summary: BenchmarkAggregateSummary,
    pub research_spec: ExperimentSpec,
    pub research_result: ExperimentResult,
    pub research_evaluation: ExperimentScoreEvaluation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rounds: Vec<ArcBenchmarkRunSummary>,
}

pub fn summarize_exact_match_run(
    run_id: impl Into<String>,
    summary: &ArcExactMatchBenchmarkSummary,
    costs: ArcBenchmarkUsageTotals,
    duration_ms: u64,
) -> Result<ArcBenchmarkRunSummary, ArcBenchmarkError> {
    let score_bps = score_to_bps(summary.mean_task_score);
    let pass_rate_bps = ratio_to_bps(summary.exact_match_tasks, summary.total_tasks);
    Ok(ArcBenchmarkRunSummary {
        run_id: run_id.into(),
        benchmark: summary.benchmark,
        run_kind: ArcBenchmarkRunKind::ExactMatch,
        task_id: None,
        total_samples: summary.total_tasks,
        passed_samples: summary.exact_match_tasks,
        score_bps,
        pass_rate_bps,
        total_actions: None,
        duration_ms,
        costs,
        report_digest: canonical_sha256_hex(summary)?,
    })
}

pub fn summarize_interactive_run(
    run_id: impl Into<String>,
    report: &ArcInteractiveRunReport,
    costs: ArcBenchmarkUsageTotals,
    duration_ms: u64,
) -> Result<ArcBenchmarkRunSummary, ArcBenchmarkError> {
    Ok(ArcBenchmarkRunSummary {
        run_id: run_id.into(),
        benchmark: report.benchmark,
        run_kind: ArcBenchmarkRunKind::Interactive,
        task_id: Some(report.task_id.clone()),
        total_samples: 1,
        passed_samples: u32::from(report.completed),
        score_bps: score_to_bps(report.scorecard.overall_score),
        pass_rate_bps: if report.completed { 10_000 } else { 0 },
        total_actions: Some(report.total_actions),
        duration_ms,
        costs,
        report_digest: canonical_sha256_hex(report)?,
    })
}

pub fn aggregate_repeated_runs(
    spec: &ArcRepeatedRunSpec,
    rounds: Vec<ArcBenchmarkRunSummary>,
) -> Result<ArcRepeatedRunAggregate, ArcBenchmarkError> {
    if rounds.is_empty() {
        return Err(ArcBenchmarkError::RepeatedRunsMissing);
    }
    for round in &rounds {
        if round.benchmark != spec.benchmark {
            return Err(ArcBenchmarkError::RepeatedRunBenchmarkMismatch {
                expected: spec.benchmark,
                actual: round.benchmark,
                run_id: round.run_id.clone(),
            });
        }
    }

    let per_round_scores_bps = rounds
        .iter()
        .map(|round| round.score_bps)
        .collect::<Vec<_>>();
    let per_round_pass_rates_bps = rounds
        .iter()
        .map(|round| round.pass_rate_bps)
        .collect::<Vec<_>>();
    let aggregate_score_bps = aggregate_u32(spec.aggregation, per_round_scores_bps.as_slice());
    let aggregate_pass_rate_bps =
        aggregate_u32(spec.aggregation, per_round_pass_rates_bps.as_slice());
    let total_duration_ms = rounds.iter().map(|round| round.duration_ms).sum::<u64>();
    let total_cost_usd = rounds
        .iter()
        .map(|round| round.costs.total_cost_usd)
        .sum::<f64>();
    let total_tokens_input = rounds
        .iter()
        .map(|round| round.costs.total_tokens_input)
        .sum::<u64>();
    let total_tokens_output = rounds
        .iter()
        .map(|round| round.costs.total_tokens_output)
        .sum::<u64>();
    let mean_actions = mean_optional_actions(rounds.as_slice());

    let eval_summary = BenchmarkAggregateSummary {
        package_key: spec.benchmark_package_key.clone(),
        execution_mode: spec.execution_mode,
        aggregation: spec.aggregation,
        round_count: u32::try_from(rounds.len()).unwrap_or(u32::MAX),
        aggregate_score_bps: Some(aggregate_score_bps),
        aggregate_pass_rate_bps,
        per_round_scores_bps,
        per_round_pass_rates_bps,
        summary_digest: repeated_run_digest(
            spec,
            rounds.as_slice(),
            aggregate_score_bps,
            aggregate_pass_rate_bps,
        )?,
    };

    let research_spec = build_research_spec(spec, &eval_summary)?;
    let research_result = build_research_result(
        spec,
        &research_spec,
        &eval_summary,
        total_duration_ms,
        total_cost_usd,
        total_tokens_input,
        total_tokens_output,
        mean_actions,
    );
    let research_evaluation = research_spec
        .score_contract
        .evaluate_result(&research_result)?;

    Ok(ArcRepeatedRunAggregate {
        benchmark: spec.benchmark,
        round_count: u32::try_from(rounds.len()).unwrap_or(u32::MAX),
        aggregate_score_bps,
        aggregate_pass_rate_bps,
        mean_actions,
        total_duration_ms,
        total_cost_usd,
        eval_summary,
        research_spec,
        research_result,
        research_evaluation,
        rounds,
    })
}

fn build_research_spec(
    spec: &ArcRepeatedRunSpec,
    eval_summary: &BenchmarkAggregateSummary,
) -> Result<ExperimentSpec, ArcBenchmarkError> {
    let package_ref = spec.benchmark_package_key.storage_key();
    let package_digest = canonical_sha256_hex(&package_ref)?;
    Ok(ExperimentSpec::new(
        spec.experiment_id.clone(),
        spec.candidate_id.clone(),
        ExperimentFamily::ValidatorPolicy {
            validator_suite_ref: package_ref.clone(),
            policy: ValidatorPolicySpec::new(
                format!("arc.benchmark.{}", spec.experiment_id),
                10_000,
                1,
                "arc_benchmark_summary",
                10_000,
            ),
        },
        vec![ExperimentArtifactRef::new(
            ExperimentArtifactKind::BenchmarkSuite,
            package_ref,
            package_digest,
        )],
        CandidateMutation::new(
            format!("arc-summary-{}", spec.candidate_id),
            None,
            ExperimentFamilyKind::ValidatorPolicy,
            vec![
                String::from("arc_benchmark_summary"),
                format!("aggregation:{:?}", spec.aggregation),
            ],
        ),
        ExperimentRuntimeProfile::new(eval_summary.summary_digest.clone()),
        ExperimentBudget::new(
            u64::from(eval_summary.round_count).saturating_mul(1_000),
            spec.output_root.clone(),
        ),
        ExperimentScoreContract::new(
            "arc.benchmark.aggregate.v1",
            ExperimentFamilyKind::ValidatorPolicy,
            vec![
                ScoreMetricSpec::new(
                    "benchmark_score_bps",
                    "basis_points",
                    ScoreDirection::Maximize,
                    8_000,
                ),
                ScoreMetricSpec::new(
                    "pass_rate_bps",
                    "basis_points",
                    ScoreDirection::Maximize,
                    2_000,
                ),
            ],
        ),
    ))
}

fn build_research_result(
    spec: &ArcRepeatedRunSpec,
    research_spec: &ExperimentSpec,
    eval_summary: &BenchmarkAggregateSummary,
    total_duration_ms: u64,
    total_cost_usd: f64,
    total_tokens_input: u64,
    total_tokens_output: u64,
    mean_actions: Option<f64>,
) -> ExperimentResult {
    let mut metrics = vec![
        ExperimentMetric::new("round_count", "count", i64::from(eval_summary.round_count)),
        ExperimentMetric::new(
            "total_cost_usd_micros",
            "micro_usd",
            usd_to_micros(total_cost_usd),
        ),
        ExperimentMetric::new(
            "total_tokens_input",
            "count",
            i64::try_from(total_tokens_input).unwrap_or(i64::MAX),
        ),
        ExperimentMetric::new(
            "total_tokens_output",
            "count",
            i64::try_from(total_tokens_output).unwrap_or(i64::MAX),
        ),
        ExperimentMetric::new(
            "total_duration_ms",
            "milliseconds",
            i64::try_from(total_duration_ms).unwrap_or(i64::MAX),
        ),
    ];
    if let Some(mean_actions) = mean_actions {
        metrics.push(ExperimentMetric::new(
            "mean_actions_millis",
            "count_millis",
            (mean_actions * 1_000.0).round() as i64,
        ));
    }

    ExperimentResult::new(
        format!("{}-aggregate", spec.experiment_id),
        research_spec,
        0,
        total_duration_ms,
        ExperimentRunStatus::Succeeded,
        vec![
            ExperimentScore::new(
                "benchmark_score_bps",
                "basis_points",
                i64::from(eval_summary.aggregate_score_bps.unwrap_or(0)),
            ),
            ExperimentScore::new(
                "pass_rate_bps",
                "basis_points",
                i64::from(eval_summary.aggregate_pass_rate_bps),
            ),
        ],
        metrics,
        vec![ExperimentReceiptRef::new(
            ExperimentReceiptKind::EvalRun,
            spec.benchmark_package_key.storage_key(),
            eval_summary.summary_digest.clone(),
        )],
        Vec::new(),
        stable_placeholder_digest("stdout"),
        stable_placeholder_digest("stderr"),
    )
}

fn repeated_run_digest(
    spec: &ArcRepeatedRunSpec,
    rounds: &[ArcBenchmarkRunSummary],
    aggregate_score_bps: u32,
    aggregate_pass_rate_bps: u32,
) -> Result<String, ArcBenchmarkError> {
    #[derive(Serialize)]
    struct DigestSeed<'a> {
        spec: &'a ArcRepeatedRunSpec,
        round_ids: Vec<&'a str>,
        round_digests: Vec<&'a str>,
        aggregate_score_bps: u32,
        aggregate_pass_rate_bps: u32,
    }

    canonical_sha256_hex(&DigestSeed {
        spec,
        round_ids: rounds.iter().map(|round| round.run_id.as_str()).collect(),
        round_digests: rounds
            .iter()
            .map(|round| round.report_digest.as_str())
            .collect(),
        aggregate_score_bps,
        aggregate_pass_rate_bps,
    })
    .map_err(Into::into)
}

fn stable_placeholder_digest(label: &str) -> String {
    canonical_sha256_hex(&label).unwrap_or_else(|_| String::from("arc-benchmark"))
}

fn aggregate_u32(aggregation: BenchmarkAggregationKind, values: &[u32]) -> u32 {
    match aggregation {
        BenchmarkAggregationKind::MeanScore => {
            (values.iter().copied().map(u64::from).sum::<u64>() / values.len() as u64) as u32
        }
        BenchmarkAggregationKind::MedianScore => robust_median(values),
    }
}

fn robust_median(values: &[u32]) -> u32 {
    let mut values = values.to_vec();
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        ((u64::from(values[middle - 1]) + u64::from(values[middle])) / 2) as u32
    }
}

fn mean_optional_actions(rounds: &[ArcBenchmarkRunSummary]) -> Option<f64> {
    let values = rounds
        .iter()
        .filter_map(|round| round.total_actions.map(f64::from))
        .collect::<Vec<_>>();
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
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

fn usd_to_micros(total_cost_usd: f64) -> i64 {
    (total_cost_usd * 1_000_000.0).round() as i64
}
