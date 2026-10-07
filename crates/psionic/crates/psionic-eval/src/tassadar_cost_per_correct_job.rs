use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_COST_PER_CORRECT_JOB_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_cost_per_correct_job_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarEconomicRouteFamily {
    InternalExactCompute,
    ExternalDelegation,
    HybridComposite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRouteThresholdReason {
    CostPerCorrectTooHigh,
    EvidenceCompletenessTooLow,
    RefusalRateTooHigh,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLaneEconomicObservation {
    pub route_family: TassadarEconomicRouteFamily,
    pub cost_milliunits: u32,
    pub correctness_bps: u32,
    pub refusal_rate_bps: u32,
    pub evidence_completeness_bps: u32,
    pub cost_per_correct_job_milliunits: u32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRouteThresholdPolicy {
    pub threshold_id: String,
    pub max_internal_cost_per_correct_job_milliunits: u32,
    pub min_internal_evidence_completeness_bps: u32,
    pub max_internal_refusal_rate_bps: u32,
    pub fallback_route_family: TassadarEconomicRouteFamily,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCostPerCorrectJobCase {
    pub case_id: String,
    pub workload_family: String,
    pub accepted_outcome_required: bool,
    pub threshold_policy: TassadarRouteThresholdPolicy,
    pub lane_observations: Vec<TassadarLaneEconomicObservation>,
    pub selected_route_family: TassadarEconomicRouteFamily,
    pub threshold_crossed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub threshold_reasons: Vec<TassadarRouteThresholdReason>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCostPerCorrectJobReport {
    pub schema_version: u16,
    pub report_id: String,
    pub evaluated_cases: Vec<TassadarCostPerCorrectJobCase>,
    pub internal_lane_win_count: u32,
    pub external_lane_win_count: u32,
    pub hybrid_lane_win_count: u32,
    pub threshold_crossing_case_count: u32,
    pub average_internal_cost_per_correct_job_milliunits: u32,
    pub average_external_cost_per_correct_job_milliunits: u32,
    pub average_hybrid_cost_per_correct_job_milliunits: u32,
    pub generated_from_refs: Vec<String>,
    pub compute_market_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn build_tassadar_cost_per_correct_job_report() -> TassadarCostPerCorrectJobReport {
    let evaluated_cases = seeded_cases();
    let internal_lane_win_count = lane_win_count(
        &evaluated_cases,
        TassadarEconomicRouteFamily::InternalExactCompute,
    );
    let external_lane_win_count = lane_win_count(
        &evaluated_cases,
        TassadarEconomicRouteFamily::ExternalDelegation,
    );
    let hybrid_lane_win_count = lane_win_count(
        &evaluated_cases,
        TassadarEconomicRouteFamily::HybridComposite,
    );
    let threshold_crossing_case_count = evaluated_cases
        .iter()
        .filter(|case| case.threshold_crossed)
        .count() as u32;
    let mut generated_from_refs = vec![
        String::from("fixtures/tassadar/reports/tassadar_evidence_calibrated_routing_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_accepted_outcome_binding_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_composite_routing_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_exact_compute_market_report.json"),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let mut report = TassadarCostPerCorrectJobReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.cost_per_correct_job.report.v1"),
        average_internal_cost_per_correct_job_milliunits: average_cost_per_correct(
            &evaluated_cases,
            TassadarEconomicRouteFamily::InternalExactCompute,
        ),
        average_external_cost_per_correct_job_milliunits: average_cost_per_correct(
            &evaluated_cases,
            TassadarEconomicRouteFamily::ExternalDelegation,
        ),
        average_hybrid_cost_per_correct_job_milliunits: average_cost_per_correct(
            &evaluated_cases,
            TassadarEconomicRouteFamily::HybridComposite,
        ),
        evaluated_cases,
        internal_lane_win_count,
        external_lane_win_count,
        hybrid_lane_win_count,
        threshold_crossing_case_count,
        generated_from_refs,
        compute_market_dependency_marker: String::from(
            "compute-market remains the owner of canonical economic publication and route-to-product promotion outside standalone psionic",
        ),
        nexus_dependency_marker: String::from(
            "nexus remains the owner of accepted-outcome and settlement-qualified economic closure outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this eval report is benchmark-bound product research over matched workload families. It keeps cost per correct job, refusal rate, evidence completeness, and threshold crossings explicit without turning benchmark economics into pricing truth or settlement authority",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Cost-per-correct report covers {} matched cases with {} internal wins, {} hybrid wins, {} external wins, and {} threshold-crossing cases.",
        report.evaluated_cases.len(),
        report.internal_lane_win_count,
        report.hybrid_lane_win_count,
        report.external_lane_win_count,
        report.threshold_crossing_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_cost_per_correct_job_report|", &report);
    report
}

#[must_use]
pub fn tassadar_cost_per_correct_job_report_path() -> PathBuf {
    repo_root().join(TASSADAR_COST_PER_CORRECT_JOB_REPORT_REF)
}

pub fn write_tassadar_cost_per_correct_job_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCostPerCorrectJobReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_cost_per_correct_job_report();
    let json = serde_json::to_string_pretty(&report).expect("cost-per-correct report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_cost_per_correct_job_report(
    path: impl AsRef<Path>,
) -> Result<TassadarCostPerCorrectJobReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn seeded_cases() -> Vec<TassadarCostPerCorrectJobCase> {
    vec![
        case(
            "case.patch_apply",
            "patch_apply_internal_exact",
            true,
            threshold_policy(
                "threshold.patch_apply",
                4_500,
                9_000,
                500,
                TassadarEconomicRouteFamily::HybridComposite,
                "patch route thresholds stay strict because accepted-outcome readiness matters more than cheap but weak evidence",
            ),
            vec![
                lane(
                    TassadarEconomicRouteFamily::InternalExactCompute,
                    3_200,
                    10_000,
                    0,
                    9_600,
                    "internal exact patch lane is both correct and evidence-complete",
                ),
                lane(
                    TassadarEconomicRouteFamily::ExternalDelegation,
                    7_000,
                    9_800,
                    100,
                    9_000,
                    "external delegation is strong but more expensive",
                ),
                lane(
                    TassadarEconomicRouteFamily::HybridComposite,
                    4_100,
                    10_000,
                    0,
                    9_500,
                    "hybrid lane stays competitive but does not beat the internal lane economically here",
                ),
            ],
            TassadarEconomicRouteFamily::InternalExactCompute,
            false,
            Vec::new(),
            "patch apply is the benchmark case where internal exact-compute clearly wins on cost per correct accepted outcome",
        ),
        case(
            "case.long_loop",
            "long_loop_validator_heavy",
            true,
            threshold_policy(
                "threshold.long_loop",
                4_500,
                9_000,
                1_000,
                TassadarEconomicRouteFamily::HybridComposite,
                "long-loop route thresholds should escalate once refusal risk or evidence incompleteness becomes too large",
            ),
            vec![
                lane(
                    TassadarEconomicRouteFamily::InternalExactCompute,
                    3_600,
                    7_200,
                    1_800,
                    5_800,
                    "internal exact lane remains too brittle on long loops even when its raw cost looks attractive",
                ),
                lane(
                    TassadarEconomicRouteFamily::ExternalDelegation,
                    7_600,
                    9_800,
                    100,
                    9_200,
                    "external delegation remains robust but more expensive than the hybrid fallback",
                ),
                lane(
                    TassadarEconomicRouteFamily::HybridComposite,
                    6_900,
                    9_900,
                    200,
                    9_400,
                    "hybrid fallback keeps exact fast paths while staying below the external lane on cost per correct job",
                ),
            ],
            TassadarEconomicRouteFamily::HybridComposite,
            true,
            vec![
                TassadarRouteThresholdReason::EvidenceCompletenessTooLow,
                TassadarRouteThresholdReason::RefusalRateTooHigh,
            ],
            "long-loop work crosses the internal lane thresholds, so the hybrid route becomes the honest economic default",
        ),
        case(
            "case.validator_search",
            "served_search_validator_mount",
            true,
            threshold_policy(
                "threshold.validator_search",
                4_800,
                9_300,
                700,
                TassadarEconomicRouteFamily::ExternalDelegation,
                "validator-heavy search should route away from internal exact lanes once threshold posture fails",
            ),
            vec![
                lane(
                    TassadarEconomicRouteFamily::InternalExactCompute,
                    3_900,
                    8_600,
                    2_200,
                    6_400,
                    "internal lane underprices itself relative to its refusal and evidence deficits",
                ),
                lane(
                    TassadarEconomicRouteFamily::ExternalDelegation,
                    8_200,
                    9_950,
                    100,
                    9_800,
                    "external delegation is the robust validator-facing baseline",
                ),
                lane(
                    TassadarEconomicRouteFamily::HybridComposite,
                    7_700,
                    9_700,
                    400,
                    9_300,
                    "hybrid route is close, but external delegation still wins once validator-heavy thresholds are applied",
                ),
            ],
            TassadarEconomicRouteFamily::ExternalDelegation,
            true,
            vec![
                TassadarRouteThresholdReason::EvidenceCompletenessTooLow,
                TassadarRouteThresholdReason::RefusalRateTooHigh,
            ],
            "validator-heavy search is one of the cases where the executor lane should not be used economically",
        ),
        case(
            "case.parity_short",
            "parity_short_bounded",
            false,
            threshold_policy(
                "threshold.parity_short",
                2_000,
                8_500,
                200,
                TassadarEconomicRouteFamily::HybridComposite,
                "short bounded parity keeps a looser evidence threshold because accepted-outcome closure is not mandatory",
            ),
            vec![
                lane(
                    TassadarEconomicRouteFamily::InternalExactCompute,
                    900,
                    10_000,
                    0,
                    8_600,
                    "short bounded parity is the cheap bounded-execution win case",
                ),
                lane(
                    TassadarEconomicRouteFamily::ExternalDelegation,
                    2_500,
                    10_000,
                    0,
                    9_800,
                    "external delegation has stronger evidence but cannot justify the cost here",
                ),
                lane(
                    TassadarEconomicRouteFamily::HybridComposite,
                    1_800,
                    10_000,
                    0,
                    8_500,
                    "hybrid route remains available but adds unnecessary cost over the internal lane",
                ),
            ],
            TassadarEconomicRouteFamily::InternalExactCompute,
            false,
            Vec::new(),
            "bounded short parity remains the cheap internal-exact case even under explicit thresholding",
        ),
    ]
}

fn case(
    case_id: &str,
    workload_family: &str,
    accepted_outcome_required: bool,
    threshold_policy: TassadarRouteThresholdPolicy,
    lane_observations: Vec<TassadarLaneEconomicObservation>,
    selected_route_family: TassadarEconomicRouteFamily,
    threshold_crossed: bool,
    threshold_reasons: Vec<TassadarRouteThresholdReason>,
    note: &str,
) -> TassadarCostPerCorrectJobCase {
    TassadarCostPerCorrectJobCase {
        case_id: String::from(case_id),
        workload_family: String::from(workload_family),
        accepted_outcome_required,
        threshold_policy,
        lane_observations,
        selected_route_family,
        threshold_crossed,
        threshold_reasons,
        note: String::from(note),
    }
}

fn threshold_policy(
    threshold_id: &str,
    max_internal_cost_per_correct_job_milliunits: u32,
    min_internal_evidence_completeness_bps: u32,
    max_internal_refusal_rate_bps: u32,
    fallback_route_family: TassadarEconomicRouteFamily,
    note: &str,
) -> TassadarRouteThresholdPolicy {
    TassadarRouteThresholdPolicy {
        threshold_id: String::from(threshold_id),
        max_internal_cost_per_correct_job_milliunits,
        min_internal_evidence_completeness_bps,
        max_internal_refusal_rate_bps,
        fallback_route_family,
        note: String::from(note),
    }
}

fn lane(
    route_family: TassadarEconomicRouteFamily,
    cost_milliunits: u32,
    correctness_bps: u32,
    refusal_rate_bps: u32,
    evidence_completeness_bps: u32,
    note: &str,
) -> TassadarLaneEconomicObservation {
    TassadarLaneEconomicObservation {
        route_family,
        cost_milliunits,
        correctness_bps,
        refusal_rate_bps,
        evidence_completeness_bps,
        cost_per_correct_job_milliunits: (cost_milliunits * 10_000) / correctness_bps.max(1),
        note: String::from(note),
    }
}

fn lane_win_count(
    cases: &[TassadarCostPerCorrectJobCase],
    route_family: TassadarEconomicRouteFamily,
) -> u32 {
    cases
        .iter()
        .filter(|case| case.selected_route_family == route_family)
        .count() as u32
}

fn average_cost_per_correct(
    cases: &[TassadarCostPerCorrectJobCase],
    route_family: TassadarEconomicRouteFamily,
) -> u32 {
    let matching = cases
        .iter()
        .flat_map(|case| case.lane_observations.iter())
        .filter(|lane| lane.route_family == route_family)
        .collect::<Vec<_>>();
    let total = matching
        .iter()
        .map(|lane| lane.cost_per_correct_job_milliunits)
        .sum::<u32>();
    total / matching.len() as u32
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
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
        TassadarEconomicRouteFamily, build_tassadar_cost_per_correct_job_report,
        load_tassadar_cost_per_correct_job_report, tassadar_cost_per_correct_job_report_path,
    };

    #[test]
    fn cost_per_correct_job_report_keeps_threshold_crossings_and_losses_explicit() {
        let report = build_tassadar_cost_per_correct_job_report();

        assert_eq!(report.evaluated_cases.len(), 4);
        assert_eq!(report.internal_lane_win_count, 2);
        assert_eq!(report.hybrid_lane_win_count, 1);
        assert_eq!(report.external_lane_win_count, 1);
        assert_eq!(report.threshold_crossing_case_count, 2);
        assert!(report.evaluated_cases.iter().any(|case| {
            case.selected_route_family == TassadarEconomicRouteFamily::ExternalDelegation
                && case.note.contains("should not be used economically")
        }));
    }

    #[test]
    fn cost_per_correct_job_report_matches_committed_truth() {
        let expected = build_tassadar_cost_per_correct_job_report();
        let committed =
            load_tassadar_cost_per_correct_job_report(tassadar_cost_per_correct_job_report_path())
                .expect("committed cost-per-correct report");

        assert_eq!(committed, expected);
    }
}
