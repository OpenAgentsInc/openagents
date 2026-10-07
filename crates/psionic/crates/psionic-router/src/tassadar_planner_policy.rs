use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    TASSADAR_PLANNER_LANGUAGE_COMPUTE_POLICY_REPORT_REF, TassadarExecutorFixture,
    TassadarPlannerLanguageComputePolicyPublication, TassadarPlannerRouteFamily,
    TassadarWorkloadClass, TassadarWorkloadSupportPosture,
    tassadar_planner_language_compute_policy_publication,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TASSADAR_PLANNER_EXECUTOR_ROUTE_PRODUCT_ID;

const REPORT_SCHEMA_VERSION: u16 = 1;
const LANGUAGE_ONLY_PRODUCT_ID: &str = "psionic.text_generation";
const EXTERNAL_TOOL_PRODUCT_ID: &str = "psionic.sandbox_execution";
const TASSADAR_EXACTNESS_REFUSAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_exactness_refusal_report.json";

/// Stable hybrid-task intents used by the planner policy benchmark suite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerHybridTaskIntent {
    OpenEndedExplanation,
    ExactTransform,
    LongHorizonRobustExecution,
    ExactSearchCheck,
}

/// Admissibility posture for one candidate route in the planner policy report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerPolicyRouteAdmissibility {
    Admissible,
    RefusedByPolicy,
}

/// Typed refusal reason when the policy declines one route family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerPolicyRefusalReason {
    LanguageSufficient,
    EvidenceBurdenTooHigh,
    ExternalToolPreferredForRobustness,
    InternalExecutorWorkloadMismatch,
}

/// One scored route candidate for the planner policy benchmark.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerLanguageComputeRouteScore {
    /// Stable route family.
    pub route_family: TassadarPlannerRouteFamily,
    /// Product surface associated with the route family.
    pub product_id: String,
    /// Whether the route remains admissible after explicit policy checks.
    pub admissibility: TassadarPlannerPolicyRouteAdmissibility,
    /// Typed refusal reason when the route is declined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<TassadarPlannerPolicyRefusalReason>,
    /// Current expected-correctness prior in basis points.
    pub expected_correctness_bps: u32,
    /// Estimated route cost in milliunits.
    pub estimated_cost_milliunits: u32,
    /// Estimated cost converted into a penalty scale for scoring.
    pub estimated_cost_penalty_bps: u32,
    /// Estimated evidence burden in basis points.
    pub evidence_burden_bps: u32,
    /// Estimated refusal risk in basis points.
    pub refusal_risk_bps: u32,
    /// Workload-family fit in basis points.
    pub workload_fit_bps: u32,
    /// Weighted composite score used for route ranking.
    pub composite_score: i32,
    /// Primary evidence surface used by this route.
    pub evidence_surface: String,
    /// Plain-language route note.
    pub note: String,
}

/// One benchmarked hybrid case in the planner policy report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerLanguageComputeCaseEvaluation {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable task-family label.
    pub task_family: String,
    /// Task intent used to seed the benchmark label.
    pub task_intent: TassadarPlannerHybridTaskIntent,
    /// Workload class attached to the case.
    pub workload_class: TassadarWorkloadClass,
    /// Route family expected by the seeded benchmark label.
    pub expected_route_family: TassadarPlannerRouteFamily,
    /// Route family selected by the scored policy.
    pub selected_route_family: TassadarPlannerRouteFamily,
    /// Whether the selected route matches the seeded benchmark label.
    pub selection_matches_expected: bool,
    /// Whether the internal executor lane was explicitly refused by policy.
    pub executor_invocation_refused: bool,
    /// Ordered route scores for the case.
    pub route_scores: Vec<TassadarPlannerLanguageComputeRouteScore>,
    /// Plain-language case note.
    pub note: String,
}

/// Deterministic benchmark report for planner-native language-vs-compute policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerLanguageComputePolicyReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Public publication that anchors the score vocabulary.
    pub policy_publication: TassadarPlannerLanguageComputePolicyPublication,
    /// Ordered benchmarked hybrid cases.
    pub evaluated_cases: Vec<TassadarPlannerLanguageComputeCaseEvaluation>,
    /// Ordered refs used to ground the report.
    pub generated_from_refs: Vec<String>,
    /// Lane-selection accuracy against the seeded benchmark labels.
    pub lane_selection_accuracy_bps: u32,
    /// Share of cases where the selected route remained policy-compliant.
    pub policy_compliance_rate_bps: u32,
    /// Average cost across correctly selected routes.
    pub cost_per_correct_job_milliunits: u32,
    /// Share of cases where the internal executor was explicitly declined.
    pub executor_invocation_refusal_rate_bps: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// One-line summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarPlannerLanguageComputePolicyReport {
    fn new(
        policy_publication: TassadarPlannerLanguageComputePolicyPublication,
        evaluated_cases: Vec<TassadarPlannerLanguageComputeCaseEvaluation>,
        generated_from_refs: Vec<String>,
    ) -> Self {
        let case_count = evaluated_cases.len() as u32;
        let correct_count = evaluated_cases
            .iter()
            .filter(|case| case.selection_matches_expected)
            .count() as u32;
        let compliant_count = evaluated_cases
            .iter()
            .filter(|case| {
                case.selection_matches_expected
                    && case
                        .route_scores
                        .iter()
                        .find(|score| score.route_family == case.selected_route_family)
                        .map(|score| {
                            score.admissibility
                                == TassadarPlannerPolicyRouteAdmissibility::Admissible
                        })
                        .unwrap_or(false)
            })
            .count() as u32;
        let executor_refusal_count = evaluated_cases
            .iter()
            .filter(|case| case.executor_invocation_refused)
            .count() as u32;
        let total_correct_cost = evaluated_cases
            .iter()
            .filter(|case| case.selection_matches_expected)
            .filter_map(|case| {
                case.route_scores
                    .iter()
                    .find(|score| score.route_family == case.selected_route_family)
                    .map(|score| score.estimated_cost_milliunits)
            })
            .sum::<u32>();
        let cost_per_correct_job_milliunits = if correct_count == 0 {
            0
        } else {
            total_correct_cost / correct_count
        };
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.planner_language_compute_policy.report.v1"),
            policy_publication,
            evaluated_cases,
            generated_from_refs,
            lane_selection_accuracy_bps: ratio_bps(correct_count, case_count),
            policy_compliance_rate_bps: ratio_bps(compliant_count, case_count),
            cost_per_correct_job_milliunits,
            executor_invocation_refusal_rate_bps: ratio_bps(executor_refusal_count, case_count),
            claim_boundary: String::from(
                "this report is a benchmark-bound routing surface over seeded hybrid cases. It compares language-only, internal exact-compute, and external-tool lanes without treating route ranking as authority closure, market settlement, or served capability widening",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Planner policy benchmark covers {} hybrid cases with selection accuracy {} bps, policy compliance {} bps, average cost-per-correct-job {} milliunits, and executor-invocation refusal {} bps.",
            report.evaluated_cases.len(),
            report.lane_selection_accuracy_bps,
            report.policy_compliance_rate_bps,
            report.cost_per_correct_job_milliunits,
            report.executor_invocation_refusal_rate_bps,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_planner_language_compute_policy_report|",
            &report,
        );
        report
    }
}

/// Planner-policy report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarPlannerLanguageComputePolicyReportError {
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read one committed artifact.
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode one committed artifact.
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed planner language-vs-compute policy report.
pub fn build_tassadar_planner_language_compute_policy_report() -> Result<
    TassadarPlannerLanguageComputePolicyReport,
    TassadarPlannerLanguageComputePolicyReportError,
> {
    let policy_publication = tassadar_planner_language_compute_policy_publication();
    let evaluated_cases = seeded_cases()
        .iter()
        .map(build_case_evaluation)
        .collect::<Vec<_>>();
    let mut generated_from_refs = policy_publication.validation_refs.clone();
    generated_from_refs.push(String::from(TASSADAR_EXACTNESS_REFUSAL_REPORT_REF));
    generated_from_refs.sort();
    generated_from_refs.dedup();
    Ok(TassadarPlannerLanguageComputePolicyReport::new(
        policy_publication,
        evaluated_cases,
        generated_from_refs,
    ))
}

/// Returns the canonical absolute path for the committed planner policy report.
#[must_use]
pub fn tassadar_planner_language_compute_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PLANNER_LANGUAGE_COMPUTE_POLICY_REPORT_REF)
}

/// Writes the committed planner language-vs-compute policy report.
pub fn write_tassadar_planner_language_compute_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPlannerLanguageComputePolicyReport,
    TassadarPlannerLanguageComputePolicyReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPlannerLanguageComputePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_planner_language_compute_policy_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPlannerLanguageComputePolicyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[derive(Clone, Copy)]
struct SeededPlannerCase {
    case_id: &'static str,
    task_family: &'static str,
    task_intent: TassadarPlannerHybridTaskIntent,
    workload_class: TassadarWorkloadClass,
    note: &'static str,
}

fn seeded_cases() -> [SeededPlannerCase; 6] {
    [
        SeededPlannerCase {
            case_id: "open_ended_article_math_explanation",
            task_family: "open_ended_article_math_explanation",
            task_intent: TassadarPlannerHybridTaskIntent::OpenEndedExplanation,
            workload_class: TassadarWorkloadClass::ArithmeticMicroprogram,
            note: "Open-ended explanation task where exact internal compute exists but should stay unused because the planner can answer directly in language.",
        },
        SeededPlannerCase {
            case_id: "memory_lookup_result_narration",
            task_family: "memory_lookup_result_narration",
            task_intent: TassadarPlannerHybridTaskIntent::OpenEndedExplanation,
            workload_class: TassadarWorkloadClass::MemoryLookupMicroprogram,
            note: "Narrative result-reporting task over a bounded memory-lookup workload where language-only remains cheaper and sufficiently correct.",
        },
        SeededPlannerCase {
            case_id: "memory_heavy_exact_executor_patch",
            task_family: "memory_heavy_exact_executor_patch",
            task_intent: TassadarPlannerHybridTaskIntent::ExactTransform,
            workload_class: TassadarWorkloadClass::MemoryHeavyKernel,
            note: "Exact transform task over a memory-heavy kernel where the internal exact-compute lane should beat both language-only and external delegation.",
        },
        SeededPlannerCase {
            case_id: "branch_heavy_exact_control_repair",
            task_family: "branch_heavy_exact_control_repair",
            task_intent: TassadarPlannerHybridTaskIntent::ExactTransform,
            workload_class: TassadarWorkloadClass::BranchHeavyKernel,
            note: "Exact control-flow repair task where the planner should still route internally even if the runtime may need an explicit slower fallback path.",
        },
        SeededPlannerCase {
            case_id: "long_loop_robust_execution_plan",
            task_family: "long_loop_robust_execution_plan",
            task_intent: TassadarPlannerHybridTaskIntent::LongHorizonRobustExecution,
            workload_class: TassadarWorkloadClass::LongLoopKernel,
            note: "Long-horizon task where current internal exact-compute support is still too fragile or costly, so the policy should prefer an explicit external tool lane.",
        },
        SeededPlannerCase {
            case_id: "sudoku_exact_candidate_check",
            task_family: "sudoku_exact_candidate_check",
            task_intent: TassadarPlannerHybridTaskIntent::ExactSearchCheck,
            workload_class: TassadarWorkloadClass::SudokuClass,
            note: "Exact search-check task where the bounded internal search fixture should remain the planner's preferred exact route.",
        },
    ]
}

fn build_case_evaluation(spec: &SeededPlannerCase) -> TassadarPlannerLanguageComputeCaseEvaluation {
    let mut route_scores = vec![
        language_route_score(spec),
        internal_route_score(spec),
        external_tool_route_score(spec),
    ];
    route_scores.sort_by_key(|score| score.route_family.as_str());
    let selected_route_family = select_route_family(&route_scores);
    let expected_route_family = expected_route_family(spec.task_intent);
    let executor_invocation_refused = route_scores
        .iter()
        .find(|score| score.route_family == TassadarPlannerRouteFamily::InternalExactCompute)
        .map(|score| {
            score.admissibility == TassadarPlannerPolicyRouteAdmissibility::RefusedByPolicy
        })
        .unwrap_or(false);
    TassadarPlannerLanguageComputeCaseEvaluation {
        case_id: String::from(spec.case_id),
        task_family: String::from(spec.task_family),
        task_intent: spec.task_intent,
        workload_class: spec.workload_class,
        expected_route_family,
        selected_route_family,
        selection_matches_expected: selected_route_family == expected_route_family,
        executor_invocation_refused,
        route_scores,
        note: String::from(spec.note),
    }
}

fn language_route_score(spec: &SeededPlannerCase) -> TassadarPlannerLanguageComputeRouteScore {
    let (
        expected_correctness_bps,
        estimated_cost_milliunits,
        evidence_burden_bps,
        refusal_risk_bps,
        workload_fit_bps,
        note,
    ) = match spec.task_intent {
        TassadarPlannerHybridTaskIntent::OpenEndedExplanation => (
            8_800,
            600,
            800,
            400,
            9_500,
            "language-only route stays cheap and sufficient for an explanatory answer",
        ),
        TassadarPlannerHybridTaskIntent::ExactTransform => (
            5_200,
            700,
            900,
            1_200,
            2_500,
            "language-only route cannot justify an exact transform even though it remains cheap",
        ),
        TassadarPlannerHybridTaskIntent::LongHorizonRobustExecution => (
            3_800,
            700,
            700,
            1_800,
            1_500,
            "language-only route is too brittle for long-horizon robust execution",
        ),
        TassadarPlannerHybridTaskIntent::ExactSearchCheck => (
            4_600,
            700,
            700,
            1_700,
            2_000,
            "language-only route is a poor fit for an exact search check",
        ),
    };
    route_score(
        TassadarPlannerRouteFamily::LanguageOnly,
        LANGUAGE_ONLY_PRODUCT_ID,
        TassadarPlannerPolicyRouteAdmissibility::Admissible,
        None,
        expected_correctness_bps,
        estimated_cost_milliunits,
        evidence_burden_bps,
        refusal_risk_bps,
        workload_fit_bps,
        "planner_summary",
        note,
    )
}

fn internal_route_score(spec: &SeededPlannerCase) -> TassadarPlannerLanguageComputeRouteScore {
    let fixture = internal_executor_fixture(spec.workload_class);
    let matrix = fixture.workload_capability_matrix();
    let row = matrix
        .row(spec.workload_class)
        .expect("fixture matrix should include seeded workload class");
    let (
        base_correctness_bps,
        base_cost_milliunits,
        base_evidence_burden_bps,
        base_refusal_risk_bps,
        workload_fit_bps,
    ): (u32, u32, u32, u32, u32) = match row.support_posture {
        TassadarWorkloadSupportPosture::Exact => (9_900, 1_900, 2_600, 500, 9_700),
        TassadarWorkloadSupportPosture::ExactFallbackOnly => (9_650, 2_900, 2_900, 2_300, 8_300),
        TassadarWorkloadSupportPosture::Partial => (6_500, 2_700, 3_000, 4_800, 5_300),
        TassadarWorkloadSupportPosture::ResearchOnly => (7_800, 2_800, 3_200, 3_800, 6_200),
        TassadarWorkloadSupportPosture::Unsupported => (3_200, 2_400, 2_400, 9_400, 1_000),
    };
    let (
        admissibility,
        refusal_reason,
        expected_correctness_bps,
        estimated_cost_milliunits,
        evidence_burden_bps,
        refusal_risk_bps,
        note,
    ) = match spec.task_intent {
        TassadarPlannerHybridTaskIntent::OpenEndedExplanation => (
            TassadarPlannerPolicyRouteAdmissibility::RefusedByPolicy,
            Some(TassadarPlannerPolicyRefusalReason::LanguageSufficient),
            base_correctness_bps,
            base_cost_milliunits,
            base_evidence_burden_bps,
            base_refusal_risk_bps.saturating_add(2_200),
            "internal executor stays explicit but is refused by policy because language-only is sufficient for this task",
        ),
        TassadarPlannerHybridTaskIntent::LongHorizonRobustExecution => (
            TassadarPlannerPolicyRouteAdmissibility::RefusedByPolicy,
            Some(TassadarPlannerPolicyRefusalReason::ExternalToolPreferredForRobustness),
            base_correctness_bps.saturating_sub(200),
            base_cost_milliunits.saturating_add(500),
            base_evidence_burden_bps,
            base_refusal_risk_bps.saturating_add(1_800),
            "internal executor remains benchmarked but is refused by policy on this long-horizon case because the explicit external tool lane is currently more robust",
        ),
        TassadarPlannerHybridTaskIntent::ExactTransform => (
            TassadarPlannerPolicyRouteAdmissibility::Admissible,
            None,
            base_correctness_bps,
            base_cost_milliunits,
            base_evidence_burden_bps,
            base_refusal_risk_bps,
            row.detail.as_str(),
        ),
        TassadarPlannerHybridTaskIntent::ExactSearchCheck => (
            if row.support_posture == TassadarWorkloadSupportPosture::Unsupported {
                TassadarPlannerPolicyRouteAdmissibility::RefusedByPolicy
            } else {
                TassadarPlannerPolicyRouteAdmissibility::Admissible
            },
            if row.support_posture == TassadarWorkloadSupportPosture::Unsupported {
                Some(TassadarPlannerPolicyRefusalReason::InternalExecutorWorkloadMismatch)
            } else {
                None
            },
            base_correctness_bps,
            base_cost_milliunits,
            base_evidence_burden_bps,
            base_refusal_risk_bps,
            row.detail.as_str(),
        ),
    };
    route_score(
        TassadarPlannerRouteFamily::InternalExactCompute,
        TASSADAR_PLANNER_EXECUTOR_ROUTE_PRODUCT_ID,
        admissibility,
        refusal_reason,
        expected_correctness_bps,
        estimated_cost_milliunits,
        evidence_burden_bps,
        refusal_risk_bps,
        workload_fit_bps,
        "executor_trace_and_proof_bundle",
        note,
    )
}

fn external_tool_route_score(spec: &SeededPlannerCase) -> TassadarPlannerLanguageComputeRouteScore {
    let (
        expected_correctness_bps,
        estimated_cost_milliunits,
        evidence_burden_bps,
        refusal_risk_bps,
        workload_fit_bps,
        note,
    ) = match spec.task_intent {
        TassadarPlannerHybridTaskIntent::OpenEndedExplanation => (
            8_400,
            4_200,
            3_000,
            900,
            3_200,
            "external tool route stays available but is too expensive and unnecessary for an explanatory answer",
        ),
        TassadarPlannerHybridTaskIntent::ExactTransform => (
            9_700,
            4_600,
            3_600,
            900,
            7_600,
            "external tool route is strong but costlier than the current internal exact-compute lane on this bounded transform",
        ),
        TassadarPlannerHybridTaskIntent::LongHorizonRobustExecution => (
            9_850,
            4_300,
            3_500,
            600,
            9_500,
            "external tool route is the current robust choice for this long-horizon case",
        ),
        TassadarPlannerHybridTaskIntent::ExactSearchCheck => (
            9_550,
            5_000,
            3_900,
            800,
            8_500,
            "external tool route remains a strong exact-search baseline but carries higher cost and evidence burden than the bounded internal search lane",
        ),
    };
    route_score(
        TassadarPlannerRouteFamily::ExternalTool,
        EXTERNAL_TOOL_PRODUCT_ID,
        TassadarPlannerPolicyRouteAdmissibility::Admissible,
        None,
        expected_correctness_bps,
        estimated_cost_milliunits,
        evidence_burden_bps,
        refusal_risk_bps,
        workload_fit_bps,
        "sandbox_execution_receipt",
        note,
    )
}

fn route_score(
    route_family: TassadarPlannerRouteFamily,
    product_id: &str,
    admissibility: TassadarPlannerPolicyRouteAdmissibility,
    refusal_reason: Option<TassadarPlannerPolicyRefusalReason>,
    expected_correctness_bps: u32,
    estimated_cost_milliunits: u32,
    evidence_burden_bps: u32,
    refusal_risk_bps: u32,
    workload_fit_bps: u32,
    evidence_surface: &str,
    note: &str,
) -> TassadarPlannerLanguageComputeRouteScore {
    let estimated_cost_penalty_bps = estimated_cost_milliunits.min(10_000);
    let admissibility_penalty =
        if admissibility == TassadarPlannerPolicyRouteAdmissibility::RefusedByPolicy {
            4_000
        } else {
            0
        };
    let positive =
        (expected_correctness_bps as i32 * 36 / 100) + (workload_fit_bps as i32 * 18 / 100);
    let penalty = (estimated_cost_penalty_bps as i32 * 15 / 100)
        + (evidence_burden_bps as i32 * 18 / 100)
        + (refusal_risk_bps as i32 * 13 / 100)
        + admissibility_penalty;
    TassadarPlannerLanguageComputeRouteScore {
        route_family,
        product_id: String::from(product_id),
        admissibility,
        refusal_reason,
        expected_correctness_bps,
        estimated_cost_milliunits,
        estimated_cost_penalty_bps,
        evidence_burden_bps,
        refusal_risk_bps,
        workload_fit_bps,
        composite_score: positive - penalty,
        evidence_surface: String::from(evidence_surface),
        note: String::from(note),
    }
}

fn select_route_family(
    route_scores: &[TassadarPlannerLanguageComputeRouteScore],
) -> TassadarPlannerRouteFamily {
    route_scores
        .iter()
        .filter(|score| score.admissibility == TassadarPlannerPolicyRouteAdmissibility::Admissible)
        .max_by(|left, right| {
            left.composite_score
                .cmp(&right.composite_score)
                .then_with(|| {
                    left.expected_correctness_bps
                        .cmp(&right.expected_correctness_bps)
                })
                .then_with(|| {
                    right
                        .estimated_cost_milliunits
                        .cmp(&left.estimated_cost_milliunits)
                })
                .then_with(|| left.route_family.as_str().cmp(right.route_family.as_str()))
        })
        .map(|score| score.route_family)
        .expect("seeded route scores should always leave one admissible route")
}

fn expected_route_family(intent: TassadarPlannerHybridTaskIntent) -> TassadarPlannerRouteFamily {
    match intent {
        TassadarPlannerHybridTaskIntent::OpenEndedExplanation => {
            TassadarPlannerRouteFamily::LanguageOnly
        }
        TassadarPlannerHybridTaskIntent::ExactTransform
        | TassadarPlannerHybridTaskIntent::ExactSearchCheck => {
            TassadarPlannerRouteFamily::InternalExactCompute
        }
        TassadarPlannerHybridTaskIntent::LongHorizonRobustExecution => {
            TassadarPlannerRouteFamily::ExternalTool
        }
    }
}

fn internal_executor_fixture(workload_class: TassadarWorkloadClass) -> TassadarExecutorFixture {
    match workload_class {
        TassadarWorkloadClass::ArithmeticMicroprogram
        | TassadarWorkloadClass::ClrsShortestPath
        | TassadarWorkloadClass::MemoryLookupMicroprogram
        | TassadarWorkloadClass::BranchControlFlowMicroprogram => TassadarExecutorFixture::new(),
        TassadarWorkloadClass::MicroWasmKernel
        | TassadarWorkloadClass::BranchHeavyKernel
        | TassadarWorkloadClass::MemoryHeavyKernel
        | TassadarWorkloadClass::LongLoopKernel => {
            TassadarExecutorFixture::article_i32_compute_v1()
        }
        TassadarWorkloadClass::SudokuClass => TassadarExecutorFixture::sudoku_v0_search_v1(),
        TassadarWorkloadClass::HungarianMatching => {
            TassadarExecutorFixture::hungarian_v0_matching_v1()
        }
    }
}

fn ratio_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        0
    } else {
        numerator.saturating_mul(10_000) / denominator
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-router crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPlannerLanguageComputePolicyReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(
            |error| TassadarPlannerLanguageComputePolicyReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPlannerLanguageComputePolicyReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarPlannerLanguageComputePolicyReport, TassadarPlannerPolicyRefusalReason,
        TassadarPlannerPolicyRouteAdmissibility,
        build_tassadar_planner_language_compute_policy_report, read_repo_json,
        tassadar_planner_language_compute_policy_report_path,
        write_tassadar_planner_language_compute_policy_report,
    };
    use psionic_models::{
        TASSADAR_PLANNER_LANGUAGE_COMPUTE_POLICY_REPORT_REF, TassadarPlannerRouteFamily,
    };

    #[test]
    fn planner_language_compute_policy_report_keeps_all_three_route_families_explicit() {
        let report =
            build_tassadar_planner_language_compute_policy_report().expect("planner report");

        assert_eq!(report.evaluated_cases.len(), 6);
        let language_case = report
            .evaluated_cases
            .iter()
            .find(|case| case.case_id == "open_ended_article_math_explanation")
            .expect("language case");
        assert_eq!(
            language_case.selected_route_family,
            TassadarPlannerRouteFamily::LanguageOnly
        );
        let internal_score = language_case
            .route_scores
            .iter()
            .find(|score| score.route_family == TassadarPlannerRouteFamily::InternalExactCompute)
            .expect("internal score");
        assert_eq!(
            internal_score.admissibility,
            TassadarPlannerPolicyRouteAdmissibility::RefusedByPolicy
        );
        assert_eq!(
            internal_score.refusal_reason,
            Some(TassadarPlannerPolicyRefusalReason::LanguageSufficient)
        );

        let exact_case = report
            .evaluated_cases
            .iter()
            .find(|case| case.case_id == "memory_heavy_exact_executor_patch")
            .expect("exact case");
        assert_eq!(
            exact_case.selected_route_family,
            TassadarPlannerRouteFamily::InternalExactCompute
        );

        let external_case = report
            .evaluated_cases
            .iter()
            .find(|case| case.case_id == "long_loop_robust_execution_plan")
            .expect("external case");
        assert_eq!(
            external_case.selected_route_family,
            TassadarPlannerRouteFamily::ExternalTool
        );
        assert!(report.executor_invocation_refusal_rate_bps > 0);
    }

    #[test]
    fn planner_language_compute_policy_report_matches_committed_truth() {
        let generated =
            build_tassadar_planner_language_compute_policy_report().expect("planner report");
        let committed: TassadarPlannerLanguageComputePolicyReport =
            read_repo_json(TASSADAR_PLANNER_LANGUAGE_COMPUTE_POLICY_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_planner_language_compute_policy_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_planner_language_compute_policy_report.json");
        let written = write_tassadar_planner_language_compute_policy_report(&output_path)
            .expect("write report");
        let persisted: TassadarPlannerLanguageComputePolicyReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            output_path.file_name().and_then(|value| value.to_str()),
            Some("tassadar_planner_language_compute_policy_report.json")
        );
        assert!(
            tassadar_planner_language_compute_policy_report_path().ends_with(
                "fixtures/tassadar/reports/tassadar_planner_language_compute_policy_report.json"
            )
        );
    }
}
