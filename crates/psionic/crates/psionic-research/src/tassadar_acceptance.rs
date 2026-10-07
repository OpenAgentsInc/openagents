use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarCompiledExecutorCompatibilityReport, TassadarCompiledExecutorExactnessReport,
    TassadarExecutorHullBenchmarkReport, TassadarHungarianCompiledExecutorCompatibilityReport,
    TassadarHungarianCompiledExecutorExactnessReport, TassadarHungarianLaneClaimStatus,
    TassadarHungarianLaneStatusReport,
};
use psionic_runtime::{TassadarClaimClass, TassadarExecutorDecodeMode};
use psionic_train::{
    TassadarArticleLearnedBenchmarkReport, TassadarArticleLearnedFitReport,
    TassadarExecutorPromotionGateReport, TassadarExecutorReferenceRunBundle,
    TassadarExecutorSequenceFitReport, TassadarHungarianLearnedFitReport,
    TassadarHungarianLearnedLaneReport,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarCompiledExecutorRunBundle, TassadarExecutorAttentionPromotionRunBundle,
    TassadarExecutorAttentionRunBundle, TassadarHungarianCompiledExecutorRunBundle,
};

const RESEARCH_ONLY_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_attention_boundary_v9/run_bundle.json";
const LEARNED_BOUNDED_PROMOTION_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_promotion_v3/promotion_bundle.json";
const LEARNED_BOUNDED_PROMOTION_GATE_REPORT_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_promotion_v3/promotion_gate_report.json";
const FAST_PATH_REFERENCE_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_reference_run_v0/run_bundle.json";
const FAST_PATH_HULL_BENCHMARK_REPORT_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_reference_run_v0/neural_hull_benchmark_report.json";
const LEARNED_9X9_SEQUENCE_FIT_REPORT_REF: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_reference_run_v0/sequence_fit_report.json";
const LEARNED_HUNGARIAN_SEQUENCE_FIT_REPORT_REF: &str =
    "fixtures/tassadar/runs/hungarian_v0_learned_executor_v0/sequence_fit_report.json";
const LEARNED_HUNGARIAN_LANE_REPORT_REF: &str =
    "fixtures/tassadar/runs/hungarian_v0_learned_executor_v0/learned_lane_report.json";
const LEARNED_ARTICLE_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_v0_learned_article_executor_v0/run_bundle.json";
const LEARNED_ARTICLE_SEQUENCE_FIT_REPORT_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_v0_learned_article_executor_v0/sequence_fit_report.json";
const LEARNED_ARTICLE_BENCHMARK_REPORT_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_v0_learned_article_executor_v0/article_learned_benchmark_report.json";
const COMPILED_MILLION_STEP_BENCHMARK_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/million_step_loop_benchmark_v0/benchmark_bundle.json";
const COMPILED_SUDOKU_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0/run_bundle.json";
const COMPILED_SUDOKU_9X9_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/run_bundle.json";
const COMPILED_SUDOKU_EXACTNESS_REPORT_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0/compiled_executor_exactness_report.json";
const COMPILED_SUDOKU_COMPATIBILITY_REPORT_REF: &str = "fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0/compiled_executor_compatibility_report.json";
const COMPILED_HUNGARIAN_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/run_bundle.json";
const COMPILED_HUNGARIAN_EXACTNESS_REPORT_REF: &str = "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/compiled_executor_exactness_report.json";
const COMPILED_HUNGARIAN_COMPATIBILITY_REPORT_REF: &str = "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/compiled_executor_compatibility_report.json";
const COMPILED_HUNGARIAN_LANE_STATUS_REPORT_REF: &str =
    "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/hungarian_lane_status_report.json";
const COMPILED_HUNGARIAN_10X10_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/run_bundle.json";
const COMPILED_HUNGARIAN_10X10_EXACTNESS_REPORT_REF: &str = "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/compiled_executor_exactness_report.json";
const COMPILED_HUNGARIAN_10X10_COMPATIBILITY_REPORT_REF: &str = "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/compiled_executor_compatibility_report.json";
const COMPILED_HUNGARIAN_10X10_CLAIM_BOUNDARY_REPORT_REF: &str =
    "fixtures/tassadar/runs/hungarian_10x10_v0_compiled_executor_v0/claim_boundary_report.json";
const COMPILED_KERNEL_SUITE_RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0/run_bundle.json";
const COMPILED_KERNEL_SUITE_EXACTNESS_REPORT_REF: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_exactness_report.json";
const COMPILED_KERNEL_SUITE_COMPATIBILITY_REPORT_REF: &str = "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_compatibility_report.json";
const COMPILED_KERNEL_SUITE_SCALING_REPORT_REF: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0/compiled_kernel_suite_scaling_report.json";
const COMPILED_KERNEL_SUITE_CLAIM_BOUNDARY_REPORT_REF: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0/claim_boundary_report.json";

/// Canonical output directory for the Tassadar acceptance report.
pub const TASSADAR_ACCEPTANCE_OUTPUT_DIR: &str = "fixtures/tassadar/reports";
/// Canonical machine-readable acceptance report file.
pub const TASSADAR_ACCEPTANCE_REPORT_FILE: &str = "tassadar_acceptance_report.json";
/// Stable schema version for the machine-readable Tassadar acceptance report.
pub const TASSADAR_ACCEPTANCE_REPORT_SCHEMA_VERSION: u16 = 1;
/// Canonical checker command for the live acceptance report.
pub const TASSADAR_ACCEPTANCE_CHECKER_COMMAND: &str = "scripts/check-tassadar-acceptance.sh";
/// Canonical machine-readable learned long-horizon policy report file.
pub const TASSADAR_LEARNED_HORIZON_POLICY_REPORT_FILE: &str =
    "tassadar_learned_horizon_policy_report.json";
/// Stable schema version for the learned long-horizon policy report.
pub const TASSADAR_LEARNED_HORIZON_POLICY_REPORT_SCHEMA_VERSION: u16 = 1;

/// Whether the learned long-horizon article guard is satisfied by an exact
/// benchmark or by an explicit refusal policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnedLongHorizonGuardStatus {
    /// An exact learned long-horizon benchmark bundle exists.
    ExactBenchmarkLanded,
    /// The repo explicitly refuses learned long-horizon article claims.
    ExplicitRefusalPolicy,
}

/// Current benchmark posture for the learned long-horizon lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnedLongHorizonBenchmarkStatus {
    /// An exact learned long-horizon benchmark exists.
    Exact,
    /// A benchmark exists but remains partial.
    Partial,
    /// No learned long-horizon benchmark exists yet.
    NotLanded,
}

/// Typed learned refusal kind for unsupported long-horizon requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnedLongHorizonRefusalKind {
    /// The requested learned horizon is outside the current supported bar.
    UnsupportedHorizon,
}

/// Typed reasons backing the current learned long-horizon refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLearnedLongHorizonRefusalReason {
    /// No committed learned million-step benchmark exists.
    NoLearnedMillionStepBenchmark,
    /// No validated learned article-class long-trace contract exists yet.
    NoValidatedLearnedArticleClassLongTraceContract,
    /// The 9x9 learned lane still operates under bounded incremental windows.
    Sudoku9x9StillBoundedToWindowedScope,
    /// The learned Hungarian lane remains research-only rather than promoted.
    HungarianV0StillResearchOnly,
}

/// Machine-readable guardrail for learned million-step and article-class
/// long-horizon claims.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnedLongHorizonPolicyReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable policy identifier.
    pub policy_id: String,
    /// Claim class guarded by this policy.
    pub enforced_claim_class: TassadarClaimClass,
    /// Minimum trace-step bar for the long-horizon article story.
    pub article_class_trace_step_floor: u32,
    /// Whether the guard is currently satisfied by a benchmark or refusal.
    pub guard_status: TassadarLearnedLongHorizonGuardStatus,
    /// Current learned benchmark posture for that long-horizon bar.
    pub benchmark_status: TassadarLearnedLongHorizonBenchmarkStatus,
    /// Learned benchmark artifact when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_artifact_ref: Option<String>,
    /// Compiled/runtime comparator artifact proving the million-step story is
    /// currently outside the learned lane.
    pub reference_million_step_artifact_ref: String,
    /// Typed refusal kind currently enforced by the repo, when the guard is
    /// still satisfied by refusal rather than an exact benchmark.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<TassadarLearnedLongHorizonRefusalKind>,
    /// Typed refusal reasons currently backing the policy.
    pub refusal_reasons: Vec<TassadarLearnedLongHorizonRefusalReason>,
    /// Human-readable refusal detail when the guard is still satisfied by
    /// explicit refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
    /// Whether learned article-class claims may bypass this guard.
    pub learned_article_class_bypass_allowed: bool,
    /// Green bounded learned artifact that stays below the article horizon.
    pub bounded_green_artifact_ref: String,
    /// 9x9 learned fit artifact currently bounding the long-horizon story.
    pub sudoku_9x9_fit_artifact_ref: String,
    /// 9x9 learned model identifier.
    pub sudoku_9x9_model_id: String,
    /// 9x9 learned context ceiling.
    pub sudoku_9x9_model_max_sequence_tokens: u32,
    /// 9x9 maximum full-sequence token count.
    pub sudoku_9x9_total_token_count_max: u32,
    /// Whether full 9x9 sequences fit model context.
    pub sudoku_9x9_full_sequence_fits_model_context: bool,
    /// Honest 9x9 scope statement.
    pub sudoku_9x9_scope_statement: String,
    /// Learned Hungarian fit artifact.
    pub hungarian_v0_fit_artifact_ref: String,
    /// Learned Hungarian lane artifact.
    pub hungarian_v0_lane_artifact_ref: String,
    /// Learned Hungarian model identifier.
    pub hungarian_v0_model_id: String,
    /// Learned Hungarian context ceiling.
    pub hungarian_v0_model_max_sequence_tokens: u32,
    /// Learned Hungarian maximum full-sequence token count.
    pub hungarian_v0_total_token_count_max: u32,
    /// Whether learned Hungarian full sequences fit model context.
    pub hungarian_v0_full_sequence_fits_model_context: bool,
    /// Learned Hungarian exact-trace case count.
    pub hungarian_v0_exact_trace_case_count: u32,
    /// Learned Hungarian exact final-output case count.
    pub hungarian_v0_final_output_exact_case_count: u32,
    /// Honest learned Hungarian verdict.
    pub hungarian_v0_verdict: String,
    /// Explicit follow-on requirement for replacing the refusal policy when the
    /// learned article-class benchmark is not yet landed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_requirement: Option<String>,
    /// Stable report digest.
    pub report_digest: String,
}

/// One artifact or report used as evidence for a Tassadar acceptance verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAcceptanceEvidenceRef {
    /// Short human-readable label.
    pub label: String,
    /// Repo-relative artifact path.
    pub artifact_ref: String,
    /// Stable digest when the artifact carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

impl TassadarAcceptanceEvidenceRef {
    fn new(
        label: impl Into<String>,
        artifact_ref: impl Into<String>,
        digest: Option<String>,
    ) -> Self {
        Self {
            label: label.into(),
            artifact_ref: artifact_ref.into(),
            digest,
        }
    }
}

/// One green or red acceptance verdict inside the Tassadar acceptance report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAcceptanceVerdict {
    /// Stable verdict identifier.
    pub verdict_id: String,
    /// Whether the current repo artifacts clear this bar.
    pub passed: bool,
    /// Human-readable detail that keeps the claim boundary honest.
    pub detail: String,
    /// Validation commands that justify the current verdict.
    pub validation_commands: Vec<String>,
    /// Evidence artifacts consulted for the verdict.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<TassadarAcceptanceEvidenceRef>,
    /// Explicit missing evidence for a red verdict.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_evidence: Vec<String>,
}

impl TassadarAcceptanceVerdict {
    fn new(
        verdict_id: impl Into<String>,
        passed: bool,
        detail: impl Into<String>,
        validation_commands: Vec<String>,
        evidence: Vec<TassadarAcceptanceEvidenceRef>,
        missing_evidence: Vec<String>,
    ) -> Self {
        Self {
            verdict_id: verdict_id.into(),
            passed,
            detail: detail.into(),
            validation_commands,
            evidence,
            missing_evidence,
        }
    }
}

/// Machine-readable Tassadar acceptance report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAcceptanceReport {
    /// Stable report schema version.
    pub schema_version: u16,
    /// Checker command that regenerates and verifies the report.
    pub checker_command: String,
    /// Repo-relative output path for the canonical persisted report.
    pub report_ref: String,
    /// Canonical fixture root consulted by the checker.
    pub fixture_root: String,
    /// Current coarse claim classes that can be used honestly.
    pub allowed_claim_classes: Vec<TassadarClaimClass>,
    /// Current coarse claim classes that remain forbidden.
    pub disallowed_claim_classes: Vec<TassadarClaimClass>,
    /// Whether article-parity wording is currently allowed at all.
    pub article_parity_language_allowed: bool,
    /// Research-only lane truth.
    pub research_only: TassadarAcceptanceVerdict,
    /// Bounded compiled/proof-backed truth.
    pub compiled_exact: TassadarAcceptanceVerdict,
    /// Bounded learned-lane truth.
    pub learned_bounded: TassadarAcceptanceVerdict,
    /// Bounded fast-path truth on the declared workload window.
    pub fast_path_declared_workload_exact: TassadarAcceptanceVerdict,
    /// Compiled article-class closure.
    pub compiled_article_class: TassadarAcceptanceVerdict,
    /// Learned article-class closure.
    pub learned_article_class: TassadarAcceptanceVerdict,
    /// Final article-parity closeout.
    pub article_closure: TassadarAcceptanceVerdict,
    /// Whether the checker observed the exact current expected truth pattern.
    pub current_truth_holds: bool,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarAcceptanceReport {
    /// Returns whether the current report still matches the expected truth
    /// pattern for the live repo.
    #[must_use]
    pub const fn current_truth_holds(&self) -> bool {
        self.current_truth_holds
    }
}

/// Error while computing or persisting the Tassadar acceptance report.
#[derive(Debug, Error)]
pub enum TassadarAcceptanceError {
    /// Failed to read one required artifact.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// Artifact path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Failed to deserialize one required artifact.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Artifact path.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Failed to serialize the acceptance report.
    #[error("failed to serialize `{artifact_kind}`: {error}")]
    Serialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// Failed to write the acceptance report.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// Artifact path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
}

/// Returns the canonical repo-relative acceptance report path.
#[must_use]
pub fn tassadar_acceptance_report_ref() -> &'static str {
    "fixtures/tassadar/reports/tassadar_acceptance_report.json"
}

/// Returns the canonical absolute acceptance report path.
#[must_use]
pub fn tassadar_acceptance_report_path() -> PathBuf {
    repo_root().join(tassadar_acceptance_report_ref())
}

/// Returns the canonical repo-relative learned long-horizon policy report path.
#[must_use]
pub fn tassadar_learned_horizon_policy_report_ref() -> &'static str {
    "fixtures/tassadar/reports/tassadar_learned_horizon_policy_report.json"
}

/// Returns the canonical absolute learned long-horizon policy report path.
#[must_use]
pub fn tassadar_learned_horizon_policy_report_path() -> PathBuf {
    repo_root().join(tassadar_learned_horizon_policy_report_ref())
}

/// Builds the live learned long-horizon policy report from the committed fit
/// and lane artifacts.
pub fn build_tassadar_learned_horizon_policy_report()
-> Result<TassadarLearnedLongHorizonPolicyReport, TassadarAcceptanceError> {
    let sudoku_9x9_fit_report: TassadarExecutorSequenceFitReport = read_repo_json(
        LEARNED_9X9_SEQUENCE_FIT_REPORT_REF,
        "tassadar_executor_sequence_fit_report",
    )?;
    let hungarian_fit_report: TassadarHungarianLearnedFitReport = read_repo_json(
        LEARNED_HUNGARIAN_SEQUENCE_FIT_REPORT_REF,
        "tassadar_hungarian_learned_fit_report",
    )?;
    let hungarian_lane_report: TassadarHungarianLearnedLaneReport = read_repo_json(
        LEARNED_HUNGARIAN_LANE_REPORT_REF,
        "tassadar_hungarian_learned_lane_report",
    )?;
    let article_fit_report: TassadarArticleLearnedFitReport = read_repo_json(
        LEARNED_ARTICLE_SEQUENCE_FIT_REPORT_REF,
        "tassadar_article_learned_fit_report",
    )?;
    let article_benchmark_report: TassadarArticleLearnedBenchmarkReport = read_repo_json(
        LEARNED_ARTICLE_BENCHMARK_REPORT_REF,
        "tassadar_article_learned_benchmark_report",
    )?;

    let article_benchmark_landed =
        article_fit_report.full_sequence_fits_model_context && article_benchmark_report.passed;
    let (guard_status, benchmark_status, benchmark_artifact_ref, refusal_kind, refusal_reasons, refusal_detail, replacement_requirement, article_class_trace_step_floor) =
        if article_benchmark_landed {
            (
                TassadarLearnedLongHorizonGuardStatus::ExactBenchmarkLanded,
                TassadarLearnedLongHorizonBenchmarkStatus::Exact,
                Some(String::from(LEARNED_ARTICLE_BENCHMARK_REPORT_REF)),
                None,
                Vec::new(),
                None,
                None,
                article_fit_report.target_token_count_max,
            )
        } else {
            (
                TassadarLearnedLongHorizonGuardStatus::ExplicitRefusalPolicy,
                if article_benchmark_report.passed {
                    TassadarLearnedLongHorizonBenchmarkStatus::Partial
                } else {
                    TassadarLearnedLongHorizonBenchmarkStatus::NotLanded
                },
                Some(String::from(LEARNED_ARTICLE_BENCHMARK_REPORT_REF)),
                Some(TassadarLearnedLongHorizonRefusalKind::UnsupportedHorizon),
                vec![
                    TassadarLearnedLongHorizonRefusalReason::NoLearnedMillionStepBenchmark,
                    TassadarLearnedLongHorizonRefusalReason::NoValidatedLearnedArticleClassLongTraceContract,
                    TassadarLearnedLongHorizonRefusalReason::Sudoku9x9StillBoundedToWindowedScope,
                    TassadarLearnedLongHorizonRefusalReason::HungarianV0StillResearchOnly,
                ],
                Some(format!(
                    "learned million-step and broader learned long-horizon traces remain explicitly outside the supported bar until more than one exact learned long-horizon benchmark bundle exists; the landed learned Hungarian-10x10 benchmark is exact on the fixed article corpus with `validation_exact_traces={}` and `test_exact_traces={}`, while 9x9 remains bounded to `{:?}` with full-sequence overflow up to {} tokens and learned Hungarian-v0 remains research-only with `exact_traces={}` and `final_outputs={}`",
                    article_benchmark_report.validation_report.exact_trace_case_count,
                    article_benchmark_report.test_report.exact_trace_case_count,
                    sudoku_9x9_fit_report.long_trace_contract,
                    sudoku_9x9_fit_report.full_sequence_context_overflow_max,
                    hungarian_lane_report.exact_trace_case_count,
                    hungarian_lane_report.final_output_exact_case_count
                )),
                Some(String::from(
                    "replace the remaining explicit learned long-horizon refusal with additional exact learned benchmarks before widening beyond the fixed Hungarian-10x10 article corpus",
                )),
                article_fit_report.target_token_count_max,
            )
        };

    let mut report = TassadarLearnedLongHorizonPolicyReport {
        schema_version: TASSADAR_LEARNED_HORIZON_POLICY_REPORT_SCHEMA_VERSION,
        policy_id: String::from("tassadar.learned_long_horizon_policy.v0"),
        enforced_claim_class: TassadarClaimClass::LearnedArticleClass,
        article_class_trace_step_floor,
        guard_status,
        benchmark_status,
        benchmark_artifact_ref,
        reference_million_step_artifact_ref: String::from(COMPILED_MILLION_STEP_BENCHMARK_BUNDLE_REF),
        refusal_kind,
        refusal_reasons,
        refusal_detail,
        learned_article_class_bypass_allowed: false,
        bounded_green_artifact_ref: String::from(LEARNED_BOUNDED_PROMOTION_BUNDLE_REF),
        sudoku_9x9_fit_artifact_ref: String::from(LEARNED_9X9_SEQUENCE_FIT_REPORT_REF),
        sudoku_9x9_model_id: sudoku_9x9_fit_report.model_id.clone(),
        sudoku_9x9_model_max_sequence_tokens: sudoku_9x9_fit_report.model_max_sequence_tokens,
        sudoku_9x9_total_token_count_max: sudoku_9x9_fit_report.total_token_count_max,
        sudoku_9x9_full_sequence_fits_model_context: sudoku_9x9_fit_report
            .full_sequence_fits_model_context,
        sudoku_9x9_scope_statement: sudoku_9x9_fit_report.scope_statement.clone(),
        hungarian_v0_fit_artifact_ref: String::from(LEARNED_HUNGARIAN_SEQUENCE_FIT_REPORT_REF),
        hungarian_v0_lane_artifact_ref: String::from(LEARNED_HUNGARIAN_LANE_REPORT_REF),
        hungarian_v0_model_id: hungarian_fit_report.model_id.clone(),
        hungarian_v0_model_max_sequence_tokens: hungarian_fit_report.model_max_sequence_tokens,
        hungarian_v0_total_token_count_max: hungarian_fit_report.total_token_count_max,
        hungarian_v0_full_sequence_fits_model_context: hungarian_fit_report
            .full_sequence_fits_model_context,
        hungarian_v0_exact_trace_case_count: hungarian_lane_report.exact_trace_case_count,
        hungarian_v0_final_output_exact_case_count: hungarian_lane_report
            .final_output_exact_case_count,
        hungarian_v0_verdict: hungarian_lane_report.verdict.clone(),
        replacement_requirement,
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_tassadar_learned_horizon_policy_report|",
        &report,
    );
    Ok(report)
}

/// Writes the canonical learned long-horizon policy report.
pub fn write_tassadar_learned_horizon_policy_report(
    path: &Path,
) -> Result<TassadarLearnedLongHorizonPolicyReport, TassadarAcceptanceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarAcceptanceError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_tassadar_learned_horizon_policy_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).map_err(|error| TassadarAcceptanceError::Serialize {
            artifact_kind: String::from("tassadar_learned_horizon_policy_report"),
            error,
        })?;
    fs::write(path, bytes).map_err(|error| TassadarAcceptanceError::Write {
        path: path.display().to_string(),
        error,
    })?;
    Ok(report)
}

/// Builds the live Tassadar acceptance report from the canonical committed
/// fixture roots.
pub fn build_tassadar_acceptance_report()
-> Result<TassadarAcceptanceReport, TassadarAcceptanceError> {
    let research_only = build_research_only_verdict()?;
    let compiled_exact = build_compiled_exact_verdict()?;
    let learned_bounded = build_learned_bounded_verdict()?;
    let fast_path_declared_workload_exact = build_fast_path_verdict()?;
    let compiled_article_class = build_compiled_article_class_verdict()?;
    let learned_article_class = build_learned_article_class_verdict()?;
    let article_closure = build_article_closure_verdict(
        &compiled_article_class,
        &fast_path_declared_workload_exact,
        &learned_article_class,
    );

    let mut allowed_claim_classes = Vec::new();
    if research_only.passed {
        allowed_claim_classes.push(TassadarClaimClass::ResearchOnly);
    }
    if compiled_exact.passed {
        allowed_claim_classes.push(TassadarClaimClass::CompiledExact);
    }
    if learned_bounded.passed {
        allowed_claim_classes.push(TassadarClaimClass::LearnedBounded);
    }
    if compiled_article_class.passed {
        allowed_claim_classes.push(TassadarClaimClass::CompiledArticleClass);
    }
    if learned_article_class.passed {
        allowed_claim_classes.push(TassadarClaimClass::LearnedArticleClass);
    }

    let disallowed_claim_classes = [
        TassadarClaimClass::ResearchOnly,
        TassadarClaimClass::CompiledExact,
        TassadarClaimClass::LearnedBounded,
        TassadarClaimClass::CompiledArticleClass,
        TassadarClaimClass::LearnedArticleClass,
    ]
    .into_iter()
    .filter(|claim_class| !allowed_claim_classes.contains(claim_class))
    .collect::<Vec<_>>();

    let current_truth_holds = research_only.passed
        && compiled_exact.passed
        && learned_bounded.passed
        && fast_path_declared_workload_exact.passed
        && compiled_article_class.passed
        && learned_article_class.passed
        && article_closure.passed;

    let mut report = TassadarAcceptanceReport {
        schema_version: TASSADAR_ACCEPTANCE_REPORT_SCHEMA_VERSION,
        checker_command: String::from(TASSADAR_ACCEPTANCE_CHECKER_COMMAND),
        report_ref: String::from(tassadar_acceptance_report_ref()),
        fixture_root: String::from("fixtures/tassadar/runs"),
        allowed_claim_classes,
        disallowed_claim_classes,
        article_parity_language_allowed: article_closure.passed,
        research_only,
        compiled_exact,
        learned_bounded,
        fast_path_declared_workload_exact,
        compiled_article_class,
        learned_article_class,
        article_closure,
        current_truth_holds,
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(b"psionic_tassadar_acceptance_report|", &report);
    Ok(report)
}

/// Writes the canonical Tassadar acceptance report.
pub fn write_tassadar_acceptance_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarAcceptanceReport, TassadarAcceptanceError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarAcceptanceError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_tassadar_acceptance_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).map_err(|error| TassadarAcceptanceError::Serialize {
            artifact_kind: String::from("tassadar_acceptance_report"),
            error,
        })?;
    fs::write(output_path, bytes).map_err(|error| TassadarAcceptanceError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(report)
}

fn build_research_only_verdict() -> Result<TassadarAcceptanceVerdict, TassadarAcceptanceError> {
    let run_bundle: TassadarExecutorAttentionRunBundle = read_repo_json(
        RESEARCH_ONLY_RUN_BUNDLE_REF,
        "tassadar_executor_attention_run_bundle",
    )?;
    let passed = run_bundle.claim_class == TassadarClaimClass::ResearchOnly;
    Ok(TassadarAcceptanceVerdict::new(
        "research_only",
        passed,
        if passed {
            "The preserved Sudoku-v0 attention boundary bundle remains explicitly research-only and is not promoted to a learned or compiled executor claim."
        } else {
            "The preserved attention boundary bundle no longer carries the required `research_only` claim class."
        },
        vec![String::from(
            "cargo test -p psionic-research attention_training_reduces_loss_and_writes_bundle",
        )],
        vec![TassadarAcceptanceEvidenceRef::new(
            "preserved research-only attention run bundle",
            RESEARCH_ONLY_RUN_BUNDLE_REF,
            Some(run_bundle.bundle_digest),
        )],
        Vec::new(),
    ))
}

fn build_compiled_exact_verdict() -> Result<TassadarAcceptanceVerdict, TassadarAcceptanceError> {
    let sudoku_bundle: TassadarCompiledExecutorRunBundle = read_repo_json(
        COMPILED_SUDOKU_RUN_BUNDLE_REF,
        "tassadar_compiled_executor_run_bundle",
    )?;
    let sudoku_exactness: TassadarCompiledExecutorExactnessReport = read_repo_json(
        COMPILED_SUDOKU_EXACTNESS_REPORT_REF,
        "tassadar_compiled_executor_exactness_report",
    )?;
    let sudoku_compatibility: TassadarCompiledExecutorCompatibilityReport = read_repo_json(
        COMPILED_SUDOKU_COMPATIBILITY_REPORT_REF,
        "tassadar_compiled_executor_compatibility_report",
    )?;
    let hungarian_bundle: TassadarHungarianCompiledExecutorRunBundle = read_repo_json(
        COMPILED_HUNGARIAN_RUN_BUNDLE_REF,
        "tassadar_hungarian_compiled_executor_run_bundle",
    )?;
    let hungarian_exactness: TassadarHungarianCompiledExecutorExactnessReport = read_repo_json(
        COMPILED_HUNGARIAN_EXACTNESS_REPORT_REF,
        "tassadar_hungarian_compiled_executor_exactness_report",
    )?;
    let hungarian_compatibility: TassadarHungarianCompiledExecutorCompatibilityReport =
        read_repo_json(
            COMPILED_HUNGARIAN_COMPATIBILITY_REPORT_REF,
            "tassadar_hungarian_compiled_executor_compatibility_report",
        )?;
    let hungarian_lane_status: TassadarHungarianLaneStatusReport = read_repo_json(
        COMPILED_HUNGARIAN_LANE_STATUS_REPORT_REF,
        "tassadar_hungarian_lane_status_report",
    )?;

    let passed = sudoku_bundle.claim_class == TassadarClaimClass::CompiledExact
        && hungarian_bundle.claim_class == TassadarClaimClass::CompiledExact
        && sudoku_exactness.exact_trace_rate_bps == 10_000
        && sudoku_exactness.exact_trace_case_count == sudoku_exactness.total_case_count
        && sudoku_compatibility.matched_refusal_rate_bps == 10_000
        && sudoku_compatibility.matched_refusal_check_count
            == sudoku_compatibility.total_check_count
        && hungarian_exactness.exact_trace_rate_bps == 10_000
        && hungarian_exactness.exact_trace_case_count == hungarian_exactness.total_case_count
        && hungarian_compatibility.matched_refusal_rate_bps == 10_000
        && hungarian_compatibility.matched_refusal_check_count
            == hungarian_compatibility.total_check_count
        && hungarian_lane_status.compiled_lane_status == TassadarHungarianLaneClaimStatus::Exact;

    Ok(TassadarAcceptanceVerdict::new(
        "compiled_exact",
        passed,
        if passed {
            "The bounded compiled/proof-backed Sudoku-v0 and Hungarian-v0 lanes are exact on their matched corpora and preserve exact refusal truth; any learned Hungarian posture remains a separate lane-status fact rather than part of this compiled exactness claim."
        } else {
            "One of the bounded compiled/proof-backed corpora no longer clears exactness or refusal truth on the committed reports."
        },
        vec![
            String::from(
                "cargo test -p psionic-research compiled_executor_bundle_writes_reports_and_deployments",
            ),
            String::from(
                "cargo test -p psionic-research compiled_hungarian_executor_bundle_writes_reports_and_deployments",
            ),
        ],
        vec![
            TassadarAcceptanceEvidenceRef::new(
                "bounded compiled Sudoku run bundle",
                COMPILED_SUDOKU_RUN_BUNDLE_REF,
                Some(sudoku_bundle.bundle_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "bounded compiled Sudoku exactness report",
                COMPILED_SUDOKU_EXACTNESS_REPORT_REF,
                Some(sudoku_exactness.report_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "bounded compiled Sudoku compatibility report",
                COMPILED_SUDOKU_COMPATIBILITY_REPORT_REF,
                Some(sudoku_compatibility.report_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "bounded compiled Hungarian run bundle",
                COMPILED_HUNGARIAN_RUN_BUNDLE_REF,
                Some(hungarian_bundle.bundle_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "bounded compiled Hungarian exactness report",
                COMPILED_HUNGARIAN_EXACTNESS_REPORT_REF,
                Some(hungarian_exactness.report_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "bounded compiled Hungarian compatibility report",
                COMPILED_HUNGARIAN_COMPATIBILITY_REPORT_REF,
                Some(hungarian_compatibility.report_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "compiled-vs-learned Hungarian lane status",
                COMPILED_HUNGARIAN_LANE_STATUS_REPORT_REF,
                Some(hungarian_lane_status.report_digest),
            ),
        ],
        Vec::new(),
    ))
}

fn build_learned_bounded_verdict() -> Result<TassadarAcceptanceVerdict, TassadarAcceptanceError> {
    let promotion_bundle: TassadarExecutorAttentionPromotionRunBundle = read_repo_json(
        LEARNED_BOUNDED_PROMOTION_BUNDLE_REF,
        "tassadar_executor_attention_promotion_bundle",
    )?;
    let promotion_gate: TassadarExecutorPromotionGateReport = read_repo_json(
        LEARNED_BOUNDED_PROMOTION_GATE_REPORT_REF,
        "tassadar_executor_promotion_gate_report",
    )?;
    let passed = promotion_bundle.claim_class == TassadarClaimClass::LearnedBounded
        && promotion_gate.passed
        && promotion_gate.first_target_exactness_bps
            >= promotion_gate.required_first_target_exactness_bps
        && promotion_gate.first_32_token_exactness_bps
            > promotion_gate.required_first_32_token_exactness_bps_strictly_greater_than
        && promotion_gate.exact_trace_case_count >= promotion_gate.required_exact_trace_case_count;

    Ok(TassadarAcceptanceVerdict::new(
        "learned_bounded",
        passed,
        if passed {
            "The learned Sudoku-v0 promotion bundle is green on its explicit 4x4 boundary and remains honestly bounded rather than article-class."
        } else {
            "The learned promotion bundle no longer clears the explicit bounded promotion gate."
        },
        vec![
            String::from(
                "cargo test -p psionic-train promotion_gate_checker_matches_committed_green_attention_bundle",
            ),
            String::from("scripts/check-tassadar-4x4-promotion-gate.sh"),
        ],
        vec![
            TassadarAcceptanceEvidenceRef::new(
                "learned promotion bundle",
                LEARNED_BOUNDED_PROMOTION_BUNDLE_REF,
                Some(promotion_bundle.bundle_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "learned promotion gate report",
                LEARNED_BOUNDED_PROMOTION_GATE_REPORT_REF,
                Some(promotion_gate.report_digest),
            ),
        ],
        Vec::new(),
    ))
}

fn build_fast_path_verdict() -> Result<TassadarAcceptanceVerdict, TassadarAcceptanceError> {
    let reference_run_bundle: TassadarExecutorReferenceRunBundle = read_repo_json(
        FAST_PATH_REFERENCE_RUN_BUNDLE_REF,
        "tassadar_executor_reference_run_bundle",
    )?;
    let hull_report: TassadarExecutorHullBenchmarkReport = read_repo_json(
        FAST_PATH_HULL_BENCHMARK_REPORT_REF,
        "tassadar_executor_hull_benchmark_report",
    )?;
    let expected_case_count = hull_report.case_reports.len() as u32;
    let direct_hull_case_reports = hull_report.case_reports.iter().all(|case| {
        case.hull_decode_selection.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
            && case.hull_decode_selection.effective_decode_mode
                == Some(TassadarExecutorDecodeMode::HullCache)
            && case.hull_matches_linear_prefix
    });
    let passed = hull_report.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
        && hull_report.direct_hull_case_count == expected_case_count
        && hull_report.hull_fallback_case_count == 0
        && hull_report.hull_refusal_case_count == 0
        && hull_report.hull_matches_linear_case_count == expected_case_count
        && direct_hull_case_reports;

    Ok(TassadarAcceptanceVerdict::new(
        "fast_path_declared_workload_exact",
        passed,
        if passed {
            "The committed hull-cache fast path runs directly with no fallback or refusal and matches the bounded reference-linear decode on all declared Sudoku-v0 benchmark windows; this is bounded fast-path equivalence, not full article-class task exactness."
        } else {
            "The committed hull-cache benchmark no longer proves direct no-fallback equivalence to the bounded reference-linear path on its declared workload window."
        },
        vec![String::from(
            "cargo test -p psionic-eval neural_hull_benchmark_reports_direct_hull_selection_and_window_cap",
        )],
        vec![
            TassadarAcceptanceEvidenceRef::new(
                "reference run bundle with hull benchmark digest",
                FAST_PATH_REFERENCE_RUN_BUNDLE_REF,
                Some(reference_run_bundle.bundle_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "neural hull benchmark report",
                FAST_PATH_HULL_BENCHMARK_REPORT_REF,
                reference_run_bundle.neural_hull_benchmark_report_digest,
            ),
            TassadarAcceptanceEvidenceRef::new(
                "neural hull benchmark report digest",
                FAST_PATH_HULL_BENCHMARK_REPORT_REF,
                Some(stable_digest(
                    b"psionic_tassadar_executor_neural_hull_benchmark_report|",
                    &hull_report,
                )),
            ),
        ],
        Vec::new(),
    ))
}

fn build_compiled_article_class_verdict()
-> Result<TassadarAcceptanceVerdict, TassadarAcceptanceError> {
    let sudoku_9x9_bundle: Value = read_repo_json(
        COMPILED_SUDOKU_9X9_RUN_BUNDLE_REF,
        "tassadar_sudoku_9x9_compiled_executor_run_bundle",
    )?;
    let hungarian_10x10_bundle: Value = read_repo_json(
        COMPILED_HUNGARIAN_10X10_RUN_BUNDLE_REF,
        "tassadar_hungarian_10x10_compiled_executor_run_bundle",
    )?;
    let kernel_suite_bundle: Value = read_repo_json(
        COMPILED_KERNEL_SUITE_RUN_BUNDLE_REF,
        "tassadar_compiled_kernel_suite_run_bundle",
    )?;
    let closure_report: crate::TassadarCompiledArticleClosureReport = read_repo_json(
        crate::TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF,
        "tassadar_compiled_article_closure_report",
    )?;
    let passed = closure_report.passed;

    Ok(TassadarAcceptanceVerdict::new(
        "compiled_article_class",
        passed,
        closure_report.detail.as_str(),
        vec![String::from(
            crate::TASSADAR_COMPILED_ARTICLE_CLOSURE_CHECKER_COMMAND,
        )],
        vec![
            TassadarAcceptanceEvidenceRef::new(
                "exact compiled Sudoku-9x9 run bundle",
                COMPILED_SUDOKU_9X9_RUN_BUNDLE_REF,
                sudoku_9x9_bundle["bundle_digest"]
                    .as_str()
                    .map(std::string::ToString::to_string),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "article-sized compiled Hungarian-10x10 run bundle",
                COMPILED_HUNGARIAN_10X10_RUN_BUNDLE_REF,
                hungarian_10x10_bundle["bundle_digest"]
                    .as_str()
                    .map(std::string::ToString::to_string),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "article-sized compiled Hungarian-10x10 exactness report",
                COMPILED_HUNGARIAN_10X10_EXACTNESS_REPORT_REF,
                None,
            ),
            TassadarAcceptanceEvidenceRef::new(
                "article-sized compiled Hungarian-10x10 compatibility report",
                COMPILED_HUNGARIAN_10X10_COMPATIBILITY_REPORT_REF,
                None,
            ),
            TassadarAcceptanceEvidenceRef::new(
                "article-sized compiled Hungarian-10x10 claim-boundary report",
                COMPILED_HUNGARIAN_10X10_CLAIM_BOUNDARY_REPORT_REF,
                None,
            ),
            TassadarAcceptanceEvidenceRef::new(
                "generic compiled kernel-suite run bundle",
                COMPILED_KERNEL_SUITE_RUN_BUNDLE_REF,
                kernel_suite_bundle["bundle_digest"]
                    .as_str()
                    .map(std::string::ToString::to_string),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "generic compiled kernel-suite exactness report",
                COMPILED_KERNEL_SUITE_EXACTNESS_REPORT_REF,
                None,
            ),
            TassadarAcceptanceEvidenceRef::new(
                "generic compiled kernel-suite compatibility report",
                COMPILED_KERNEL_SUITE_COMPATIBILITY_REPORT_REF,
                None,
            ),
            TassadarAcceptanceEvidenceRef::new(
                "generic compiled kernel-suite scaling report",
                COMPILED_KERNEL_SUITE_SCALING_REPORT_REF,
                None,
            ),
            TassadarAcceptanceEvidenceRef::new(
                "generic compiled kernel-suite claim-boundary report",
                COMPILED_KERNEL_SUITE_CLAIM_BOUNDARY_REPORT_REF,
                None,
            ),
            TassadarAcceptanceEvidenceRef::new(
                "compiled article-closure checker report",
                crate::TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF,
                Some(closure_report.report_digest),
            ),
        ],
        closure_report.missing_requirements,
    ))
}

fn build_learned_article_class_verdict()
-> Result<TassadarAcceptanceVerdict, TassadarAcceptanceError> {
    let article_run_bundle: TassadarExecutorReferenceRunBundle = read_repo_json(
        LEARNED_ARTICLE_RUN_BUNDLE_REF,
        "tassadar_executor_reference_run_bundle",
    )?;
    let fit_report: TassadarArticleLearnedFitReport = read_repo_json(
        LEARNED_ARTICLE_SEQUENCE_FIT_REPORT_REF,
        "tassadar_article_learned_fit_report",
    )?;
    let benchmark_report: TassadarArticleLearnedBenchmarkReport = read_repo_json(
        LEARNED_ARTICLE_BENCHMARK_REPORT_REF,
        "tassadar_article_learned_benchmark_report",
    )?;
    let horizon_policy = build_tassadar_learned_horizon_policy_report()?;
    let passed = article_run_bundle.claim_class == TassadarClaimClass::LearnedArticleClass
        && fit_report.full_sequence_fits_model_context
        && benchmark_report.passed
        && horizon_policy.guard_status
            == TassadarLearnedLongHorizonGuardStatus::ExactBenchmarkLanded
        && horizon_policy.benchmark_status == TassadarLearnedLongHorizonBenchmarkStatus::Exact
        && !horizon_policy.learned_article_class_bypass_allowed;

    Ok(TassadarAcceptanceVerdict::new(
        "learned_article_class",
        passed,
        if passed {
            "The learned lane now clears the article-class bar on the fixed Hungarian-10x10 benchmark corpus with exact validation and test traces."
        } else {
            "Learned article-class closure is still red: the fixed-corpus Hungarian-10x10 learned article benchmark is not yet exact, or the learned-horizon policy still keeps article-class language red."
        },
        vec![String::from(TASSADAR_ACCEPTANCE_CHECKER_COMMAND)],
        vec![
            TassadarAcceptanceEvidenceRef::new(
                "learned article run bundle",
                LEARNED_ARTICLE_RUN_BUNDLE_REF,
                Some(article_run_bundle.bundle_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "learned article fit report",
                LEARNED_ARTICLE_SEQUENCE_FIT_REPORT_REF,
                Some(fit_report.report_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "learned article benchmark report",
                LEARNED_ARTICLE_BENCHMARK_REPORT_REF,
                Some(benchmark_report.report_digest),
            ),
            TassadarAcceptanceEvidenceRef::new(
                "learned long-horizon policy report",
                tassadar_learned_horizon_policy_report_ref(),
                Some(horizon_policy.report_digest),
            ),
        ],
        if passed {
            Vec::new()
        } else {
            vec![
                String::from("land one exact learned article benchmark bundle"),
                String::from("turn the learned-horizon policy green on that exact benchmark"),
                String::from("keep the benchmark-corpus exactness scope statement explicit"),
            ]
        },
    ))
}

fn build_article_closure_verdict(
    compiled_article_class: &TassadarAcceptanceVerdict,
    fast_path_declared_workload_exact: &TassadarAcceptanceVerdict,
    learned_article_class: &TassadarAcceptanceVerdict,
) -> TassadarAcceptanceVerdict {
    let passed = compiled_article_class.passed
        && fast_path_declared_workload_exact.passed
        && learned_article_class.passed;
    TassadarAcceptanceVerdict::new(
        "article_closure",
        passed,
        if passed {
            "The repo can now reproduce the article-shaped Wasm compute claim from local artifacts and commands: compiled article workloads are exact, the declared fast path is exact on its workload class, and the learned Hungarian-10x10 article benchmark is exact on the fixed benchmark corpus."
        } else if compiled_article_class.passed {
            "Final article-parity closure remains red: the compiled article-class bar is green, but the learned article benchmark has not yet turned green."
        } else {
            "Final article-parity closure remains red because the compiled article-class bar is not closed, even though bounded fast-path and bounded learned facts are separately recorded."
        },
        vec![String::from(TASSADAR_ACCEPTANCE_CHECKER_COMMAND)],
        Vec::new(),
        if passed {
            Vec::new()
        } else if compiled_article_class.passed {
            vec![
                String::from("learned-lane article-class workloads must turn green"),
                String::from("fast decode truth must stay explicit on its exact workload class"),
            ]
        } else {
            vec![
                String::from("compiled/proof-backed article-class workloads must turn green"),
                String::from("fast decode truth must stay explicit on its exact workload class"),
                String::from("learned-lane article-class workloads must turn green"),
            ]
        },
    )
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn read_repo_json<T>(
    repo_relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarAcceptanceError>
where
    T: DeserializeOwned,
{
    let path = repo_root().join(repo_relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarAcceptanceError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarAcceptanceError::Deserialize {
        artifact_kind: artifact_kind.to_string(),
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let bytes = serde_json::to_vec(value)
        .expect("tassadar acceptance artifacts should serialize for stable digests");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes.as_slice());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        LEARNED_ARTICLE_BENCHMARK_REPORT_REF,
        TassadarAcceptanceReport, TassadarLearnedLongHorizonBenchmarkStatus,
        TassadarLearnedLongHorizonGuardStatus, TassadarLearnedLongHorizonPolicyReport,
        build_tassadar_acceptance_report, build_tassadar_learned_horizon_policy_report,
        read_repo_json, tassadar_acceptance_report_ref, tassadar_learned_horizon_policy_report_ref,
        write_tassadar_acceptance_report, write_tassadar_learned_horizon_policy_report,
    };

    #[test]
    fn acceptance_report_matches_committed_tassadar_truth() -> Result<(), Box<dyn std::error::Error>>
    {
        let report = build_tassadar_acceptance_report()?;
        assert!(report.current_truth_holds());
        assert!(report.research_only.passed);
        assert!(report.compiled_exact.passed);
        assert!(report.learned_bounded.passed);
        assert!(report.fast_path_declared_workload_exact.passed);
        assert!(report.compiled_article_class.passed);
        assert!(report.learned_article_class.passed);
        assert!(report.article_closure.passed);

        let persisted: TassadarAcceptanceReport = read_repo_json(
            tassadar_acceptance_report_ref(),
            "tassadar_acceptance_report",
        )?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn write_tassadar_acceptance_report_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let report_path = temp_dir.path().join("tassadar_acceptance_report.json");
        let report = write_tassadar_acceptance_report(&report_path)?;
        let bytes = std::fs::read(&report_path)?;
        let persisted: TassadarAcceptanceReport = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, report);
        assert!(persisted.current_truth_holds());
        Ok(())
    }

    #[test]
    fn learned_horizon_policy_report_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_learned_horizon_policy_report()?;
        assert_eq!(
            report.guard_status,
            TassadarLearnedLongHorizonGuardStatus::ExactBenchmarkLanded
        );
        assert_eq!(
            report.benchmark_status,
            TassadarLearnedLongHorizonBenchmarkStatus::Exact
        );
        assert_eq!(
            report.benchmark_artifact_ref.as_deref(),
            Some(LEARNED_ARTICLE_BENCHMARK_REPORT_REF)
        );
        assert!(report.refusal_kind.is_none());
        assert!(!report.learned_article_class_bypass_allowed);

        let persisted: TassadarLearnedLongHorizonPolicyReport = read_repo_json(
            tassadar_learned_horizon_policy_report_ref(),
            "tassadar_learned_horizon_policy_report",
        )?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn write_learned_horizon_policy_report_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let report_path = temp_dir
            .path()
            .join("tassadar_learned_horizon_policy_report.json");
        let report = write_tassadar_learned_horizon_policy_report(&report_path)?;
        let bytes = std::fs::read(&report_path)?;
        let persisted: TassadarLearnedLongHorizonPolicyReport = serde_json::from_slice(&bytes)?;
        assert_eq!(persisted, report);
        assert!(!persisted.learned_article_class_bypass_allowed);
        assert!(persisted.refusal_kind.is_none());
        Ok(())
    }
}
