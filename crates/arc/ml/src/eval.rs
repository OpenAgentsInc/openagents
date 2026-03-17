use std::collections::BTreeSet;

use arc_benchmark::{ArcBenchmarkError, ArcInteractiveRunReport, score_interactive_recording};
use arc_core::{ArcBenchmark, ArcRecording, ArcScorecardMetadata, ArcTaskId, canonical_sha256_hex};
use psionic_eval::BenchmarkPackageKey;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable boundary summary for evaluator-first ARC-ML work.
pub const ARC_ML_BOUNDARY_SUMMARY: &str = "arc-ml owns ARC-specific evaluator wrappers, pass@k aggregation, ARC losses/metrics, and future model/training glue over arc-benchmark truth and Psionic eval contracts";

/// Explicit provenance for ARC-ML practice corpora.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcMlDataProvenance {
    /// Synthetic ARC-AGI-3-style practice corpus that uses owned fixtures and
    /// benchmark contracts instead of real ARC Prize held-out data.
    SyntheticArcAgi3Practice,
}

/// One candidate model/sample attempt for one interactive ARC case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractivePracticeAttempt {
    pub attempt_id: String,
    pub recording: ArcRecording,
}

/// One practice case containing multiple sampled attempts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractivePracticeCase {
    pub case_id: String,
    pub task_id: ArcTaskId,
    pub metadata: ArcScorecardMetadata,
    pub baseline_actions: Vec<u32>,
    pub attempts: Vec<ArcInteractivePracticeAttempt>,
}

/// One evaluator-first ARC-ML practice suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractivePracticeSuite {
    pub suite_id: String,
    pub bounded_scope: String,
    pub package_key: BenchmarkPackageKey,
    pub data_provenance: ArcMlDataProvenance,
    pub k_values: Vec<u32>,
    pub cases: Vec<ArcInteractivePracticeCase>,
}

/// One evaluated attempt summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractivePracticeAttemptReport {
    pub attempt_id: String,
    pub recording_digest: String,
    pub report_digest: String,
    pub score_bps: u32,
    pub completed: bool,
    pub final_state: arc_core::ArcGameState,
    pub total_actions: u32,
    pub levels_completed: u16,
    pub win_levels: u16,
    pub report: ArcInteractiveRunReport,
}

/// One pass@k estimate for one case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcPassAtKCaseEstimate {
    pub requested_k: u32,
    pub effective_k: u32,
    pub pass_rate: f64,
    pub pass_rate_bps: u32,
}

/// Aggregate pass@k across all evaluated cases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcAggregatePassAtK {
    pub requested_k: u32,
    pub mean_pass_rate: f64,
    pub mean_pass_rate_bps: u32,
}

/// One evaluated case summary with all scored attempts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractivePracticeCaseReport {
    pub case_id: String,
    pub task_id: ArcTaskId,
    pub attempt_count: u32,
    pub successful_attempts: u32,
    pub best_score_bps: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub best_attempt_id: Option<String>,
    pub pass_at_k: Vec<ArcPassAtKCaseEstimate>,
    pub attempts: Vec<ArcInteractivePracticeAttemptReport>,
}

/// Full evaluator-first practice report for one synthetic suite.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArcInteractivePracticeReport {
    pub suite_id: String,
    pub bounded_scope: String,
    pub package_key: BenchmarkPackageKey,
    pub data_provenance: ArcMlDataProvenance,
    pub total_cases: u32,
    pub total_attempts: u32,
    pub successful_attempts: u32,
    pub mean_best_score_bps: u32,
    pub pass_at_k: Vec<ArcAggregatePassAtK>,
    pub summary_digest: String,
    pub cases: Vec<ArcInteractivePracticeCaseReport>,
}

#[derive(Debug, Error)]
pub enum ArcMlEvalError {
    #[error("ARC-ML practice suite id must not be empty")]
    EmptySuiteId,
    #[error("ARC-ML practice suite bounded scope must not be empty")]
    EmptyBoundedScope,
    #[error("ARC-ML practice suite must define at least one pass@k request")]
    MissingKValues,
    #[error("ARC-ML practice suite pass@k requests must be positive")]
    NonPositiveK,
    #[error("ARC-ML practice suite contains duplicate pass@k request {k}")]
    DuplicateK { k: u32 },
    #[error("ARC-ML practice suite must include at least one case")]
    MissingCases,
    #[error("ARC-ML practice case `{case_id}` must include at least one baseline action")]
    MissingBaselineActions { case_id: String },
    #[error("ARC-ML practice case `{case_id}` must include at least one attempt")]
    MissingAttempts { case_id: String },
    #[error("ARC-ML practice case id must not be empty")]
    EmptyCaseId,
    #[error("ARC-ML practice attempt id must not be empty for case `{case_id}`")]
    EmptyAttemptId { case_id: String },
    #[error("ARC-ML practice case `{case_id}` contains duplicate attempt id `{attempt_id}`")]
    DuplicateAttemptId { case_id: String, attempt_id: String },
    #[error(
        "ARC-ML practice attempt `{attempt_id}` in case `{case_id}` targets task `{actual}` but case expects `{expected}`"
    )]
    AttemptTaskMismatch {
        case_id: String,
        attempt_id: String,
        expected: ArcTaskId,
        actual: ArcTaskId,
    },
    #[error(
        "ARC-ML practice attempt `{attempt_id}` in case `{case_id}` must use the ARC-AGI-3 benchmark contract, got `{benchmark:?}`"
    )]
    UnsupportedBenchmark {
        case_id: String,
        attempt_id: String,
        benchmark: ArcBenchmark,
    },
    #[error("ARC-ML pass@k estimation requires at least one attempt")]
    MissingAttemptSamples,
    #[error("ARC-ML pass@k successful-attempt count {successful} exceeds total attempts {total}")]
    InvalidSuccessCount { total: u32, successful: u32 },
    #[error(transparent)]
    Benchmark(#[from] ArcBenchmarkError),
    #[error(transparent)]
    Serialization(#[from] arc_core::ContractSerializationError),
}

/// Evaluates one synthetic interactive-practice suite over benchmark-owned ARC
/// recording scoring and aggregates `pass@k` estimates per case and suite.
pub fn evaluate_interactive_practice_suite(
    suite: &ArcInteractivePracticeSuite,
) -> Result<ArcInteractivePracticeReport, ArcMlEvalError> {
    validate_suite(suite)?;

    let mut case_reports = Vec::with_capacity(suite.cases.len());
    for case in &suite.cases {
        case_reports.push(evaluate_case(case, suite.k_values.as_slice())?);
    }

    let total_cases = saturating_usize_to_u32(case_reports.len());
    let total_attempts = case_reports
        .iter()
        .map(|case| case.attempt_count)
        .sum::<u32>();
    let successful_attempts = case_reports
        .iter()
        .map(|case| case.successful_attempts)
        .sum::<u32>();
    let best_score_sum = case_reports
        .iter()
        .map(|case| u64::from(case.best_score_bps))
        .sum::<u64>();
    let mean_best_score_bps = if case_reports.is_empty() {
        0
    } else {
        ((best_score_sum as f64) / (case_reports.len() as f64)).round() as u32
    };
    let pass_at_k = aggregate_pass_at_k(case_reports.as_slice(), suite.k_values.as_slice());

    let summary_digest = practice_report_digest(
        suite,
        total_cases,
        total_attempts,
        successful_attempts,
        mean_best_score_bps,
        pass_at_k.as_slice(),
        case_reports.as_slice(),
    )?;

    Ok(ArcInteractivePracticeReport {
        suite_id: suite.suite_id.clone(),
        bounded_scope: suite.bounded_scope.clone(),
        package_key: suite.package_key.clone(),
        data_provenance: suite.data_provenance,
        total_cases,
        total_attempts,
        successful_attempts,
        mean_best_score_bps,
        pass_at_k,
        summary_digest,
        cases: case_reports,
    })
}

/// Standard `pass@k` estimator using bounded sampling without replacement.
pub fn estimate_pass_at_k(
    total_attempts: u32,
    successful_attempts: u32,
    requested_k: u32,
) -> Result<f64, ArcMlEvalError> {
    if total_attempts == 0 {
        return Err(ArcMlEvalError::MissingAttemptSamples);
    }
    if successful_attempts > total_attempts {
        return Err(ArcMlEvalError::InvalidSuccessCount {
            total: total_attempts,
            successful: successful_attempts,
        });
    }
    if requested_k == 0 {
        return Err(ArcMlEvalError::NonPositiveK);
    }

    let effective_k = requested_k.min(total_attempts);
    let failures = total_attempts.saturating_sub(successful_attempts);
    if failures < effective_k {
        return Ok(1.0);
    }

    let mut failure_only_probability = 1.0f64;
    for offset in 0..effective_k {
        let numerator = f64::from(failures.saturating_sub(offset));
        let denominator = f64::from(total_attempts.saturating_sub(offset));
        failure_only_probability *= numerator / denominator;
    }
    Ok(1.0 - failure_only_probability)
}

fn validate_suite(suite: &ArcInteractivePracticeSuite) -> Result<(), ArcMlEvalError> {
    if suite.suite_id.trim().is_empty() {
        return Err(ArcMlEvalError::EmptySuiteId);
    }
    if suite.bounded_scope.trim().is_empty() {
        return Err(ArcMlEvalError::EmptyBoundedScope);
    }
    if suite.k_values.is_empty() {
        return Err(ArcMlEvalError::MissingKValues);
    }
    let mut seen_k = BTreeSet::new();
    for requested_k in &suite.k_values {
        if *requested_k == 0 {
            return Err(ArcMlEvalError::NonPositiveK);
        }
        if !seen_k.insert(*requested_k) {
            return Err(ArcMlEvalError::DuplicateK { k: *requested_k });
        }
    }
    if suite.cases.is_empty() {
        return Err(ArcMlEvalError::MissingCases);
    }
    for case in &suite.cases {
        validate_case(case)?;
    }
    Ok(())
}

fn validate_case(case: &ArcInteractivePracticeCase) -> Result<(), ArcMlEvalError> {
    if case.case_id.trim().is_empty() {
        return Err(ArcMlEvalError::EmptyCaseId);
    }
    if case.baseline_actions.is_empty() {
        return Err(ArcMlEvalError::MissingBaselineActions {
            case_id: case.case_id.clone(),
        });
    }
    if case.attempts.is_empty() {
        return Err(ArcMlEvalError::MissingAttempts {
            case_id: case.case_id.clone(),
        });
    }
    let mut attempt_ids = BTreeSet::new();
    for attempt in &case.attempts {
        if attempt.attempt_id.trim().is_empty() {
            return Err(ArcMlEvalError::EmptyAttemptId {
                case_id: case.case_id.clone(),
            });
        }
        if !attempt_ids.insert(attempt.attempt_id.clone()) {
            return Err(ArcMlEvalError::DuplicateAttemptId {
                case_id: case.case_id.clone(),
                attempt_id: attempt.attempt_id.clone(),
            });
        }
        if attempt.recording.task_id != case.task_id {
            return Err(ArcMlEvalError::AttemptTaskMismatch {
                case_id: case.case_id.clone(),
                attempt_id: attempt.attempt_id.clone(),
                expected: case.task_id.clone(),
                actual: attempt.recording.task_id.clone(),
            });
        }
        if attempt.recording.benchmark != ArcBenchmark::ArcAgi3 {
            return Err(ArcMlEvalError::UnsupportedBenchmark {
                case_id: case.case_id.clone(),
                attempt_id: attempt.attempt_id.clone(),
                benchmark: attempt.recording.benchmark,
            });
        }
    }
    Ok(())
}

fn evaluate_case(
    case: &ArcInteractivePracticeCase,
    k_values: &[u32],
) -> Result<ArcInteractivePracticeCaseReport, ArcMlEvalError> {
    let mut attempt_reports = Vec::with_capacity(case.attempts.len());
    let mut best_score_bps = 0u32;
    let mut best_attempt_id = None;
    let mut successful_attempts = 0u32;

    for attempt in &case.attempts {
        let report = score_interactive_recording(
            &attempt.recording,
            case.metadata.clone(),
            case.baseline_actions.as_slice(),
        )?;
        let score_bps = score_to_bps(report.scorecard.overall_score);
        if report.completed {
            successful_attempts = successful_attempts.saturating_add(1);
        }
        if score_bps > best_score_bps {
            best_score_bps = score_bps;
            best_attempt_id = Some(attempt.attempt_id.clone());
        }
        attempt_reports.push(ArcInteractivePracticeAttemptReport {
            attempt_id: attempt.attempt_id.clone(),
            recording_digest: attempt.recording.contract_digest()?,
            report_digest: canonical_sha256_hex(&report)?,
            score_bps,
            completed: report.completed,
            final_state: report.final_state,
            total_actions: report.total_actions,
            levels_completed: report.levels_completed,
            win_levels: report.win_levels,
            report,
        });
    }

    let attempt_count = saturating_usize_to_u32(attempt_reports.len());
    let pass_at_k = k_values
        .iter()
        .map(|requested_k| {
            let pass_rate = estimate_pass_at_k(attempt_count, successful_attempts, *requested_k)?;
            Ok(ArcPassAtKCaseEstimate {
                requested_k: *requested_k,
                effective_k: (*requested_k).min(attempt_count),
                pass_rate,
                pass_rate_bps: ratio_to_bps(pass_rate),
            })
        })
        .collect::<Result<Vec<_>, ArcMlEvalError>>()?;

    Ok(ArcInteractivePracticeCaseReport {
        case_id: case.case_id.clone(),
        task_id: case.task_id.clone(),
        attempt_count,
        successful_attempts,
        best_score_bps,
        best_attempt_id,
        pass_at_k,
        attempts: attempt_reports,
    })
}

fn aggregate_pass_at_k(
    case_reports: &[ArcInteractivePracticeCaseReport],
    k_values: &[u32],
) -> Vec<ArcAggregatePassAtK> {
    k_values
        .iter()
        .map(|requested_k| {
            let mean_pass_rate = if case_reports.is_empty() {
                0.0
            } else {
                case_reports
                    .iter()
                    .map(|case| {
                        case.pass_at_k
                            .iter()
                            .find(|estimate| estimate.requested_k == *requested_k)
                            .map_or(0.0, |estimate| estimate.pass_rate)
                    })
                    .sum::<f64>()
                    / (case_reports.len() as f64)
            };
            ArcAggregatePassAtK {
                requested_k: *requested_k,
                mean_pass_rate,
                mean_pass_rate_bps: ratio_to_bps(mean_pass_rate),
            }
        })
        .collect()
}

fn practice_report_digest(
    suite: &ArcInteractivePracticeSuite,
    total_cases: u32,
    total_attempts: u32,
    successful_attempts: u32,
    mean_best_score_bps: u32,
    pass_at_k: &[ArcAggregatePassAtK],
    cases: &[ArcInteractivePracticeCaseReport],
) -> Result<String, arc_core::ContractSerializationError> {
    canonical_sha256_hex(&ArcInteractivePracticeReportDigestWire {
        suite_id: suite.suite_id.clone(),
        bounded_scope: suite.bounded_scope.clone(),
        package_key: suite.package_key.clone(),
        data_provenance: suite.data_provenance,
        total_cases,
        total_attempts,
        successful_attempts,
        mean_best_score_bps,
        pass_at_k: pass_at_k.to_vec(),
        cases: cases.to_vec(),
    })
}

fn score_to_bps(score: f32) -> u32 {
    let bounded = score.clamp(0.0, 1.0);
    (f64::from(bounded) * 10_000.0).round() as u32
}

fn ratio_to_bps(value: f64) -> u32 {
    let bounded = value.clamp(0.0, 1.0);
    (bounded * 10_000.0).round() as u32
}

fn saturating_usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct ArcInteractivePracticeReportDigestWire {
    suite_id: String,
    bounded_scope: String,
    package_key: BenchmarkPackageKey,
    data_provenance: ArcMlDataProvenance,
    total_cases: u32,
    total_attempts: u32,
    successful_attempts: u32,
    mean_best_score_bps: u32,
    pass_at_k: Vec<ArcAggregatePassAtK>,
    cases: Vec<ArcInteractivePracticeCaseReport>,
}
