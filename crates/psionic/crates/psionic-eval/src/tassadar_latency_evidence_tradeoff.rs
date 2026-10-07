use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_router::{TassadarTradeoffRouteFamily, tassadar_validator_heavy_workload_pack};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_LATENCY_EVIDENCE_TRADEOFF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_latency_evidence_tradeoff_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTradeoffRouteObservation {
    pub route_family: TassadarTradeoffRouteFamily,
    pub latency_ms: u32,
    pub correctness_bps: u32,
    pub evidence_completeness_bps: u32,
    pub validator_cost_milliunits: u32,
    pub pareto_optimal: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTradeoffCaseReport {
    pub case_id: String,
    pub threshold_crossed: bool,
    pub preferred_route_family: TassadarTradeoffRouteFamily,
    pub route_observations: Vec<TassadarTradeoffRouteObservation>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLatencyEvidenceTradeoffReport {
    pub schema_version: u16,
    pub report_id: String,
    pub pack_id: String,
    pub case_reports: Vec<TassadarTradeoffCaseReport>,
    pub validator_heavy_case_count: u32,
    pub threshold_crossing_case_count: u32,
    pub average_challenge_rate_bps: u32,
    pub generated_from_refs: Vec<String>,
    pub compute_market_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn build_tassadar_latency_evidence_tradeoff_report() -> TassadarLatencyEvidenceTradeoffReport {
    let pack = tassadar_validator_heavy_workload_pack();
    let case_reports = vec![
        tradeoff_case(
            "validator_patch_fast",
            false,
            TassadarTradeoffRouteFamily::Compiled,
            vec![
                observation(
                    TassadarTradeoffRouteFamily::Compiled,
                    80,
                    10_000,
                    9_500,
                    700,
                    true,
                    "compiled route stays on the Pareto front for fast validator-bound patch work",
                ),
                observation(
                    TassadarTradeoffRouteFamily::Learned,
                    45,
                    9_200,
                    7_800,
                    200,
                    false,
                    "learned route is fast but falls below the validator-heavy evidence floor",
                ),
                observation(
                    TassadarTradeoffRouteFamily::External,
                    170,
                    9_900,
                    9_700,
                    1_200,
                    true,
                    "external route is slower but still Pareto-valid on evidence",
                ),
                observation(
                    TassadarTradeoffRouteFamily::Hybrid,
                    105,
                    10_000,
                    9_400,
                    900,
                    true,
                    "hybrid route remains competitive but does not beat compiled on latency",
                ),
            ],
            "patch case shows where lower latency and valid evidence can coexist without externalization",
        ),
        tradeoff_case(
            "challenge_search",
            true,
            TassadarTradeoffRouteFamily::External,
            vec![
                observation(
                    TassadarTradeoffRouteFamily::Compiled,
                    95,
                    8_900,
                    7_100,
                    600,
                    false,
                    "compiled route is too under-evidenced for the challenge-heavy search pack",
                ),
                observation(
                    TassadarTradeoffRouteFamily::Learned,
                    55,
                    8_400,
                    6_200,
                    300,
                    false,
                    "learned route is fastest but invalid once challenge and validator costs are counted",
                ),
                observation(
                    TassadarTradeoffRouteFamily::External,
                    210,
                    9_950,
                    9_900,
                    1_600,
                    true,
                    "external route owns the honest high-evidence frontier for challenge-heavy search",
                ),
                observation(
                    TassadarTradeoffRouteFamily::Hybrid,
                    170,
                    9_700,
                    9_400,
                    1_200,
                    true,
                    "hybrid route stays Pareto-valid but still loses to external on correctness and evidence",
                ),
            ],
            "challenge-heavy search is the core case where lower latency is not a win once evidence posture becomes invalid",
        ),
        tradeoff_case(
            "learned_trial_error",
            false,
            TassadarTradeoffRouteFamily::Hybrid,
            vec![
                observation(
                    TassadarTradeoffRouteFamily::Compiled,
                    140,
                    9_400,
                    8_900,
                    700,
                    false,
                    "compiled route remains stable but not Pareto-best on this medium-evidence trial-and-error pack",
                ),
                observation(
                    TassadarTradeoffRouteFamily::Learned,
                    70,
                    9_500,
                    8_900,
                    400,
                    true,
                    "learned route stays Pareto-valid when the evidence floor is moderate rather than validator-heavy",
                ),
                observation(
                    TassadarTradeoffRouteFamily::External,
                    200,
                    9_800,
                    9_500,
                    1_300,
                    true,
                    "external route stays on the frontier for maximum evidence",
                ),
                observation(
                    TassadarTradeoffRouteFamily::Hybrid,
                    110,
                    9_700,
                    9_200,
                    900,
                    true,
                    "hybrid route balances latency and evidence most cleanly here",
                ),
            ],
            "medium-evidence trial-and-error work is where learned and hybrid routes can honestly stay on the Pareto frontier together",
        ),
        tradeoff_case(
            "long_loop_validator",
            true,
            TassadarTradeoffRouteFamily::Hybrid,
            vec![
                observation(
                    TassadarTradeoffRouteFamily::Compiled,
                    120,
                    8_700,
                    7_400,
                    700,
                    false,
                    "compiled route breaks the long-loop validator thresholds",
                ),
                observation(
                    TassadarTradeoffRouteFamily::Learned,
                    85,
                    8_800,
                    7_000,
                    500,
                    false,
                    "learned route stays too brittle under long-loop validator pressure",
                ),
                observation(
                    TassadarTradeoffRouteFamily::External,
                    240,
                    9_900,
                    9_600,
                    1_700,
                    true,
                    "external route is the strongest evidence path but very slow",
                ),
                observation(
                    TassadarTradeoffRouteFamily::Hybrid,
                    180,
                    9_850,
                    9_500,
                    1_200,
                    true,
                    "hybrid route offers the best latency/evidence compromise under the current thresholds",
                ),
            ],
            "long-loop validator work is where the hybrid route becomes justified even though external delegation still has the strongest evidence posture",
        ),
    ];
    let mut generated_from_refs = vec![
        String::from("fixtures/tassadar/reports/tassadar_cost_per_correct_job_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_wedge_taxonomy_report.json"),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let validator_heavy_case_count = pack
        .cases
        .iter()
        .filter(|case| case.validator_heavy)
        .count() as u32;
    let average_challenge_rate_bps = pack
        .cases
        .iter()
        .map(|case| case.challenge_rate_bps)
        .sum::<u32>()
        / pack.cases.len() as u32;
    let threshold_crossing_case_count = case_reports
        .iter()
        .filter(|case| case.threshold_crossed)
        .count() as u32;
    let mut report = TassadarLatencyEvidenceTradeoffReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.latency_evidence_tradeoff.report.v1"),
        pack_id: pack.pack_id,
        case_reports,
        validator_heavy_case_count,
        threshold_crossing_case_count,
        average_challenge_rate_bps,
        generated_from_refs,
        compute_market_dependency_marker: String::from(
            "compute-market remains the owner of canonical route-to-product promotion outside standalone psionic",
        ),
        nexus_dependency_marker: String::from(
            "nexus remains the owner of accepted-outcome and settlement-qualified tradeoff closure outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this eval report is benchmark-bound product research over latency, correctness, and evidence tradeoffs. It keeps validator cost, challenge rate, and route thresholds explicit without treating the current Pareto fronts as product truth or authority closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Latency/evidence tradeoff report covers {} cases with {} validator-heavy cases and {} threshold-crossing cases.",
        report.case_reports.len(),
        report.validator_heavy_case_count,
        report.threshold_crossing_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_latency_evidence_tradeoff_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_latency_evidence_tradeoff_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LATENCY_EVIDENCE_TRADEOFF_REPORT_REF)
}

pub fn write_tassadar_latency_evidence_tradeoff_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLatencyEvidenceTradeoffReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_latency_evidence_tradeoff_report();
    let json = serde_json::to_string_pretty(&report).expect("tradeoff report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_latency_evidence_tradeoff_report(
    path: impl AsRef<Path>,
) -> Result<TassadarLatencyEvidenceTradeoffReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn tradeoff_case(
    case_id: &str,
    threshold_crossed: bool,
    preferred_route_family: TassadarTradeoffRouteFamily,
    route_observations: Vec<TassadarTradeoffRouteObservation>,
    note: &str,
) -> TassadarTradeoffCaseReport {
    TassadarTradeoffCaseReport {
        case_id: String::from(case_id),
        threshold_crossed,
        preferred_route_family,
        route_observations,
        note: String::from(note),
    }
}

fn observation(
    route_family: TassadarTradeoffRouteFamily,
    latency_ms: u32,
    correctness_bps: u32,
    evidence_completeness_bps: u32,
    validator_cost_milliunits: u32,
    pareto_optimal: bool,
    note: &str,
) -> TassadarTradeoffRouteObservation {
    TassadarTradeoffRouteObservation {
        route_family,
        latency_ms,
        correctness_bps,
        evidence_completeness_bps,
        validator_cost_milliunits,
        pareto_optimal,
        note: String::from(note),
    }
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
        TassadarTradeoffRouteFamily, build_tassadar_latency_evidence_tradeoff_report,
        load_tassadar_latency_evidence_tradeoff_report,
        tassadar_latency_evidence_tradeoff_report_path,
    };

    #[test]
    fn latency_evidence_tradeoff_report_keeps_thresholded_losses_explicit() {
        let report = build_tassadar_latency_evidence_tradeoff_report();

        assert_eq!(report.case_reports.len(), 4);
        assert_eq!(report.validator_heavy_case_count, 3);
        assert_eq!(report.threshold_crossing_case_count, 2);
        assert!(report.case_reports.iter().any(|case| {
            case.preferred_route_family == TassadarTradeoffRouteFamily::External
                && case.threshold_crossed
        }));
    }

    #[test]
    fn latency_evidence_tradeoff_report_matches_committed_truth() {
        let expected = build_tassadar_latency_evidence_tradeoff_report();
        let committed = load_tassadar_latency_evidence_tradeoff_report(
            tassadar_latency_evidence_tradeoff_report_path(),
        )
        .expect("committed tradeoff report");

        assert_eq!(committed, expected);
    }
}
