use std::{fs, path::Path};

use psionic_models::{
    TASSADAR_NEGATIVE_INVOCATION_BUNDLE_REF, TassadarNegativeInvocationEvidenceBundle,
    TassadarNegativeInvocationRouteOutcome, TassadarNegativeInvocationTrainingCase,
    TassadarPlannerRouteFamily, TassadarWorkloadClass, tassadar_negative_invocation_publication,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_NEGATIVE_INVOCATION_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_negative_invocation_v1";
pub const TASSADAR_NEGATIVE_INVOCATION_FILE: &str = "negative_invocation_evidence_bundle.json";

/// Errors while materializing the negative-invocation evidence bundle.
#[derive(Debug, Error)]
pub enum TassadarNegativeInvocationBundleError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write negative-invocation bundle `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Executes the committed negative-invocation suite and writes the train-side evidence bundle.
pub fn execute_tassadar_negative_invocation_bundle(
    output_dir: &Path,
) -> Result<TassadarNegativeInvocationEvidenceBundle, TassadarNegativeInvocationBundleError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarNegativeInvocationBundleError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let publication = tassadar_negative_invocation_publication();
    let cases = seeded_cases();
    let case_count = cases.len() as u32;
    let baseline_average_evidence_quality_bps = average_u32(
        cases
            .iter()
            .map(|case| route_outcome(case, case.baseline_route_family).evidence_quality_bps)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let preferred_average_evidence_quality_bps = average_u32(
        cases
            .iter()
            .map(|case| route_outcome(case, case.preferred_route_family).evidence_quality_bps)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let mut bundle = TassadarNegativeInvocationEvidenceBundle {
        publication,
        unnecessary_internal_invocation_rate_bps: ratio_bps(
            cases
                .iter()
                .filter(|case| case.unnecessary_internal_invocation)
                .count() as u32,
            case_count,
        ),
        baseline_fallback_churn_total: cases
            .iter()
            .map(|case| route_outcome(case, case.baseline_route_family).fallback_churn_count)
            .sum(),
        preferred_fallback_churn_total: cases
            .iter()
            .map(|case| route_outcome(case, case.preferred_route_family).fallback_churn_count)
            .sum(),
        baseline_average_evidence_quality_bps,
        preferred_average_evidence_quality_bps,
        cases,
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Negative-invocation bundle now freezes {} seeded planner cases from `{}` with unnecessary_internal_invocation={}bps, fallback_churn {}->{} and evidence_quality {}->{} bps.",
        bundle.cases.len(),
        TASSADAR_NEGATIVE_INVOCATION_BUNDLE_REF,
        bundle.unnecessary_internal_invocation_rate_bps,
        bundle.baseline_fallback_churn_total,
        bundle.preferred_fallback_churn_total,
        bundle.baseline_average_evidence_quality_bps,
        bundle.preferred_average_evidence_quality_bps,
    );
    bundle.bundle_digest = stable_digest(b"psionic_tassadar_negative_invocation_bundle|", &bundle);

    let output_path = output_dir.join(TASSADAR_NEGATIVE_INVOCATION_FILE);
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(&output_path, format!("{json}\n")).map_err(|error| {
        TassadarNegativeInvocationBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn seeded_cases() -> Vec<TassadarNegativeInvocationTrainingCase> {
    vec![
        training_case(
            "open_ended_article_math_explanation",
            TassadarWorkloadClass::ArithmeticMicroprogram,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::LanguageOnly,
            vec![
                route(
                    TassadarPlannerRouteFamily::LanguageOnly,
                    8_900,
                    600,
                    180,
                    9_400,
                    0,
                    false,
                    "language-only route stays cheaper and clearer for explanatory output",
                ),
                route(
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    9_850,
                    2_000,
                    430,
                    7_800,
                    1,
                    false,
                    "internal exact-compute call is unnecessary and adds retry churn for a task that should stay in language",
                ),
                route(
                    TassadarPlannerRouteFamily::ExternalTool,
                    9_650,
                    4_300,
                    740,
                    7_300,
                    0,
                    false,
                    "external tool route is exact but unnecessary for an explanatory answer",
                ),
            ],
            "baseline planner over-calls the executor for an explanatory task",
        ),
        training_case(
            "memory_lookup_result_narration",
            TassadarWorkloadClass::MemoryLookupMicroprogram,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::LanguageOnly,
            vec![
                route(
                    TassadarPlannerRouteFamily::LanguageOnly,
                    8_700,
                    640,
                    190,
                    9_100,
                    0,
                    false,
                    "language-only route is sufficient for narrating a known lookup result",
                ),
                route(
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    9_700,
                    1_950,
                    420,
                    7_600,
                    1,
                    false,
                    "executor call is unnecessary and adds avoidable cost and fallback churn",
                ),
                route(
                    TassadarPlannerRouteFamily::ExternalTool,
                    9_550,
                    4_100,
                    720,
                    7_200,
                    0,
                    false,
                    "external tool route is also unnecessary for this narrative task",
                ),
            ],
            "negative labels should teach the planner not to invoke compute just because it is available",
        ),
        training_case(
            "accepted_outcome_exact_patch",
            TassadarWorkloadClass::MemoryHeavyKernel,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::InternalExactCompute,
            vec![
                route(
                    TassadarPlannerRouteFamily::LanguageOnly,
                    5_400,
                    690,
                    170,
                    4_800,
                    0,
                    true,
                    "language-only route is under-specified for exact accepted-outcome work",
                ),
                route(
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    9_900,
                    2_500,
                    390,
                    9_300,
                    0,
                    false,
                    "internal exact-compute remains the honest winning lane here",
                ),
                route(
                    TassadarPlannerRouteFamily::ExternalTool,
                    9_820,
                    4_700,
                    690,
                    9_100,
                    0,
                    false,
                    "external tool route is viable but costlier than the current bounded internal lane",
                ),
            ],
            "negative-invocation training should not teach the planner to avoid the executor when it is actually the best lane",
        ),
        training_case(
            "long_loop_robust_execution_plan",
            TassadarWorkloadClass::LongLoopKernel,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::ExternalTool,
            vec![
                route(
                    TassadarPlannerRouteFamily::LanguageOnly,
                    3_900,
                    700,
                    210,
                    4_900,
                    0,
                    true,
                    "language-only route is too brittle for long-loop robust execution",
                ),
                route(
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    9_650,
                    3_650,
                    620,
                    7_200,
                    2,
                    true,
                    "internal exact-compute looks attractive but the retry-heavy path is costlier, slower, and still refusal-prone on this long-loop case",
                ),
                route(
                    TassadarPlannerRouteFamily::ExternalTool,
                    9_840,
                    4_350,
                    760,
                    9_200,
                    0,
                    false,
                    "external tool route remains the robust exact baseline",
                ),
            ],
            "counterfactual route outcomes should teach the planner to delegate externally here instead of churning through internal retries",
        ),
        training_case(
            "sudoku_exact_candidate_check",
            TassadarWorkloadClass::SudokuClass,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::InternalExactCompute,
            vec![
                route(
                    TassadarPlannerRouteFamily::LanguageOnly,
                    4_500,
                    710,
                    195,
                    5_100,
                    0,
                    true,
                    "language-only route is too weak for exact candidate checking",
                ),
                route(
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    9_700,
                    2_950,
                    410,
                    8_900,
                    0,
                    false,
                    "internal search lane remains the honest bounded winner on this case",
                ),
                route(
                    TassadarPlannerRouteFamily::ExternalTool,
                    9_620,
                    5_050,
                    790,
                    8_600,
                    0,
                    false,
                    "external tool route is correct but too expensive to prefer by default",
                ),
            ],
            "the penalty surface should preserve necessary internal exact-compute wins",
        ),
        training_case(
            "validator_heavy_search_mount",
            TassadarWorkloadClass::BranchHeavyKernel,
            TassadarPlannerRouteFamily::InternalExactCompute,
            TassadarPlannerRouteFamily::ExternalTool,
            vec![
                route(
                    TassadarPlannerRouteFamily::LanguageOnly,
                    4_100,
                    720,
                    220,
                    4_700,
                    0,
                    true,
                    "language-only route is not acceptable for validator-heavy exact work",
                ),
                route(
                    TassadarPlannerRouteFamily::InternalExactCompute,
                    9_680,
                    3_250,
                    580,
                    7_400,
                    1,
                    true,
                    "internal route still triggers avoidable fallback, extra cost, and slower delivery compared with the validator-backed external lane",
                ),
                route(
                    TassadarPlannerRouteFamily::ExternalTool,
                    9_780,
                    4_900,
                    770,
                    9_250,
                    0,
                    false,
                    "external tool route is slower but better on evidence quality and refusal posture",
                ),
            ],
            "negative invocation should steer the planner away from needless internal calls on validator-heavy work",
        ),
    ]
}

fn training_case(
    case_id: &str,
    workload_class: TassadarWorkloadClass,
    baseline_route_family: TassadarPlannerRouteFamily,
    preferred_route_family: TassadarPlannerRouteFamily,
    route_outcomes: Vec<TassadarNegativeInvocationRouteOutcome>,
    note: &str,
) -> TassadarNegativeInvocationTrainingCase {
    let baseline = route_outcome_from_slice(&route_outcomes, baseline_route_family);
    let preferred = route_outcome_from_slice(&route_outcomes, preferred_route_family);
    TassadarNegativeInvocationTrainingCase {
        case_id: String::from(case_id),
        workload_class,
        baseline_route_family,
        preferred_route_family,
        unnecessary_internal_invocation: baseline_route_family
            == TassadarPlannerRouteFamily::InternalExactCompute
            && preferred_route_family != TassadarPlannerRouteFamily::InternalExactCompute,
        evidence_quality_regression_bps: preferred.evidence_quality_bps as i32
            - baseline.evidence_quality_bps as i32,
        route_outcomes,
        note: String::from(note),
    }
}

fn route(
    route_family: TassadarPlannerRouteFamily,
    expected_correctness_bps: u32,
    estimated_cost_milliunits: u32,
    estimated_latency_millis: u32,
    evidence_quality_bps: u32,
    fallback_churn_count: u32,
    would_refuse_when_better_lane_exists: bool,
    note: &str,
) -> TassadarNegativeInvocationRouteOutcome {
    TassadarNegativeInvocationRouteOutcome {
        route_family,
        expected_correctness_bps,
        estimated_cost_milliunits,
        estimated_latency_millis,
        evidence_quality_bps,
        fallback_churn_count,
        would_refuse_when_better_lane_exists,
        note: String::from(note),
    }
}

fn route_outcome(
    case: &TassadarNegativeInvocationTrainingCase,
    route_family: TassadarPlannerRouteFamily,
) -> &TassadarNegativeInvocationRouteOutcome {
    route_outcome_from_slice(&case.route_outcomes, route_family)
}

fn route_outcome_from_slice(
    outcomes: &[TassadarNegativeInvocationRouteOutcome],
    route_family: TassadarPlannerRouteFamily,
) -> &TassadarNegativeInvocationRouteOutcome {
    outcomes
        .iter()
        .find(|outcome| outcome.route_family == route_family)
        .expect("route family should exist in seeded outcomes")
}

fn ratio_bps(numerator: u32, denominator: u32) -> u32 {
    if denominator == 0 {
        0
    } else {
        numerator.saturating_mul(10_000) / denominator
    }
}

fn average_u32(values: &[u32]) -> u32 {
    if values.is_empty() {
        0
    } else {
        values.iter().sum::<u32>() / values.len() as u32
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TASSADAR_NEGATIVE_INVOCATION_FILE, execute_tassadar_negative_invocation_bundle};

    #[test]
    fn negative_invocation_bundle_writes_machine_legible_training_artifact() {
        let directory = tempfile::tempdir().expect("tempdir");
        let bundle = execute_tassadar_negative_invocation_bundle(directory.path()).expect("bundle");

        assert_eq!(bundle.cases.len(), 6);
        assert!(bundle.unnecessary_internal_invocation_rate_bps >= 6_000);
        assert!(bundle.baseline_fallback_churn_total > bundle.preferred_fallback_churn_total);
        assert!(
            bundle.preferred_average_evidence_quality_bps
                > bundle.baseline_average_evidence_quality_bps
        );
        assert!(
            directory
                .path()
                .join(TASSADAR_NEGATIVE_INVOCATION_FILE)
                .exists()
        );
    }
}
