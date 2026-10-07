use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_router::{
    TASSADAR_COUNTERFACTUAL_ROUTE_QUALITY_REPORT_REF, TassadarCounterfactualRouteFamily,
    TassadarCounterfactualRouteQualityReport, build_tassadar_counterfactual_route_quality_report,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_COUNTERFACTUAL_ROUTE_QUALITY_EVAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_counterfactual_route_quality_eval_report.json";

/// Eval-facing summary row for one realized-versus-counterfactual routing case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCounterfactualRouteQualityEvalRow {
    pub case_id: String,
    pub workload_family: String,
    pub realized_route_family: TassadarCounterfactualRouteFamily,
    pub best_available_route_family: TassadarCounterfactualRouteFamily,
    pub better_counterfactual_exists: bool,
    pub accepted_outcome_delta_bps: i32,
    pub success_delta_bps: i32,
    pub evidence_delta_bps: i32,
    pub cost_regret_milliunits: i32,
    pub latency_regret_ms: i32,
    pub note: String,
}

/// Eval-facing report for counterfactual route quality.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCounterfactualRouteQualityEvalReport {
    pub schema_version: u16,
    pub report_id: String,
    pub router_report: TassadarCounterfactualRouteQualityReport,
    pub eval_rows: Vec<TassadarCounterfactualRouteQualityEvalRow>,
    pub better_counterfactual_case_count: u32,
    pub accepted_outcome_better_alternative_case_count: u32,
    pub overuse_case_count: u32,
    pub underuse_case_count: u32,
    pub generated_from_refs: Vec<String>,
    pub nexus_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the committed counterfactual route-quality eval report.
#[must_use]
pub fn build_tassadar_counterfactual_route_quality_eval_report()
-> TassadarCounterfactualRouteQualityEvalReport {
    let router_report = build_tassadar_counterfactual_route_quality_report();
    let eval_rows = router_report
        .evaluated_cases
        .iter()
        .map(|case| TassadarCounterfactualRouteQualityEvalRow {
            case_id: case.case_id.clone(),
            workload_family: case.workload_family.clone(),
            realized_route_family: case.realized_route.route_family,
            best_available_route_family: case.best_available_route_family,
            better_counterfactual_exists: case.better_counterfactual_exists,
            accepted_outcome_delta_bps: case.accepted_outcome_delta_bps,
            success_delta_bps: case.success_delta_bps,
            evidence_delta_bps: case.evidence_delta_bps,
            cost_regret_milliunits: case.cost_regret_milliunits,
            latency_regret_ms: case.latency_regret_ms,
            note: case.note.clone(),
        })
        .collect::<Vec<_>>();
    let mut generated_from_refs = router_report.generated_from_refs.clone();
    generated_from_refs.push(String::from(
        TASSADAR_COUNTERFACTUAL_ROUTE_QUALITY_REPORT_REF,
    ));
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let mut report = TassadarCounterfactualRouteQualityEvalReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.counterfactual_route_quality.eval_report.v1"),
        better_counterfactual_case_count: router_report.better_counterfactual_case_count,
        accepted_outcome_better_alternative_case_count: router_report
            .accepted_outcome_regret_case_count,
        overuse_case_count: router_report.overuse_case_count,
        underuse_case_count: router_report.underuse_case_count,
        router_report,
        eval_rows,
        generated_from_refs,
        nexus_dependency_marker: String::from(
            "nexus remains the owner of accepted-outcome and settlement closure outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this eval report keeps realized-versus-counterfactual routing deltas explicit for planner learning and route auditing. It remains research-only analysis and does not treat better route ranking as accepted-outcome or settlement authority",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Counterfactual route-quality eval report covers {} cases with {} better-counterfactual cases, {} overuse cases, and {} underuse cases.",
        report.eval_rows.len(),
        report.better_counterfactual_case_count,
        report.overuse_case_count,
        report.underuse_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_counterfactual_route_quality_eval_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed counterfactual route-quality eval report.
#[must_use]
pub fn tassadar_counterfactual_route_quality_eval_report_path() -> PathBuf {
    repo_root().join(TASSADAR_COUNTERFACTUAL_ROUTE_QUALITY_EVAL_REPORT_REF)
}

/// Writes the committed counterfactual route-quality eval report.
pub fn write_tassadar_counterfactual_route_quality_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCounterfactualRouteQualityEvalReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_counterfactual_route_quality_eval_report();
    let json = serde_json::to_string_pretty(&report)
        .expect("counterfactual route-quality eval report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_counterfactual_route_quality_eval_report(
    path: impl AsRef<Path>,
) -> Result<TassadarCounterfactualRouteQualityEvalReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
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
        build_tassadar_counterfactual_route_quality_eval_report,
        load_tassadar_counterfactual_route_quality_eval_report,
        tassadar_counterfactual_route_quality_eval_report_path,
    };
    use psionic_router::TassadarCounterfactualRouteFamily;

    #[test]
    fn counterfactual_route_quality_eval_report_keeps_regret_explicit() {
        let report = build_tassadar_counterfactual_route_quality_eval_report();

        assert_eq!(report.eval_rows.len(), 4);
        assert!(report.eval_rows.iter().any(|row| {
            row.case_id == "case.patch_apply_internal_underuse"
                && row.best_available_route_family
                    == TassadarCounterfactualRouteFamily::HybridComposite
                && row.accepted_outcome_delta_bps == 10_000
        }));
        assert!(report.eval_rows.iter().any(|row| {
            row.case_id == "case.parity_hybrid_overuse"
                && row.best_available_route_family
                    == TassadarCounterfactualRouteFamily::CompiledExact
                && row.cost_regret_milliunits > 0
        }));
    }

    #[test]
    fn counterfactual_route_quality_eval_report_matches_committed_truth() {
        let expected = build_tassadar_counterfactual_route_quality_eval_report();
        let committed = load_tassadar_counterfactual_route_quality_eval_report(
            tassadar_counterfactual_route_quality_eval_report_path(),
        )
        .expect("committed counterfactual route-quality eval report");

        assert_eq!(committed, expected);
    }
}
