use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_data::{
    TASSADAR_SEARCH_NATIVE_EXECUTOR_EVIDENCE_BUNDLE_REF,
    TASSADAR_SEARCH_NATIVE_EXECUTOR_REPORT_REF, TassadarSearchNativeExecutorContract,
    tassadar_search_native_executor_contract,
};
use psionic_models::{
    TassadarSearchNativeExecutorPublication, tassadar_search_native_executor_publication,
};
use psionic_runtime::{
    TASSADAR_SEARCH_NATIVE_EXECUTOR_RUNTIME_REPORT_REF, TassadarSearchNativeRuntimeReport,
    TassadarSearchNativeRuntimeStatus, build_tassadar_search_native_executor_runtime_report,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

/// One same-task comparison row in the search-native executor report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSearchNativeComparisonCell {
    pub workload_family: String,
    pub guess_count: u32,
    pub backtrack_count: u32,
    pub recovery_quality_bps: u32,
    pub search_native_exactness_bps: u32,
    pub straight_trace_exactness_bps: u32,
    pub verifier_guided_exactness_bps: u32,
    pub search_native_status: TassadarSearchNativeRuntimeStatus,
    pub preferred_lane: String,
    pub refusal_preferred: bool,
    pub note: String,
}

/// Eval-facing report for the search-native executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSearchNativeExecutorReport {
    pub schema_version: u16,
    pub report_id: String,
    pub contract: TassadarSearchNativeExecutorContract,
    pub publication: TassadarSearchNativeExecutorPublication,
    pub runtime_report: TassadarSearchNativeRuntimeReport,
    pub evidence_bundle_ref: String,
    pub comparison_cells: Vec<TassadarSearchNativeComparisonCell>,
    pub search_native_preferred_case_count: u32,
    pub verifier_guided_preferred_case_count: u32,
    pub straight_trace_preferred_case_count: u32,
    pub refusal_case_count: u32,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the committed search-native executor eval report.
#[must_use]
pub fn build_tassadar_search_native_executor_report() -> TassadarSearchNativeExecutorReport {
    let contract = tassadar_search_native_executor_contract();
    let publication = tassadar_search_native_executor_publication();
    let runtime_report = build_tassadar_search_native_executor_runtime_report();
    let comparison_cells = runtime_report
        .workload_receipts
        .iter()
        .map(|receipt| {
            let preferred_lane = preferred_lane(receipt);
            TassadarSearchNativeComparisonCell {
                workload_family: receipt.workload_family.clone(),
                guess_count: receipt.guess_count,
                backtrack_count: receipt.backtrack_count,
                recovery_quality_bps: receipt.recovery_quality_bps,
                search_native_exactness_bps: receipt.exactness_bps,
                straight_trace_exactness_bps: receipt.straight_trace_baseline_exactness_bps,
                verifier_guided_exactness_bps: receipt.verifier_guided_baseline_exactness_bps,
                search_native_status: receipt.status,
                refusal_preferred: receipt.status
                    == TassadarSearchNativeRuntimeStatus::BudgetExhaustedRefusal,
                preferred_lane: String::from(preferred_lane),
                note: receipt.note.clone(),
            }
        })
        .collect::<Vec<_>>();
    let search_native_preferred_case_count = comparison_cells
        .iter()
        .filter(|cell| cell.preferred_lane == "search_native_executor")
        .count() as u32;
    let verifier_guided_preferred_case_count = comparison_cells
        .iter()
        .filter(|cell| cell.preferred_lane == "verifier_guided_search")
        .count() as u32;
    let straight_trace_preferred_case_count = comparison_cells
        .iter()
        .filter(|cell| cell.preferred_lane == "straight_trace_executor")
        .count() as u32;
    let refusal_case_count = comparison_cells
        .iter()
        .filter(|cell| cell.refusal_preferred)
        .count() as u32;
    let mut generated_from_refs = vec![
        String::from(TASSADAR_SEARCH_NATIVE_EXECUTOR_EVIDENCE_BUNDLE_REF),
        String::from(TASSADAR_SEARCH_NATIVE_EXECUTOR_RUNTIME_REPORT_REF),
    ];
    for workload_row in &contract.workload_rows {
        generated_from_refs.extend(workload_row.baseline_refs.iter().cloned());
    }
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let mut report = TassadarSearchNativeExecutorReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.search_native_executor.report.v1"),
        contract,
        publication,
        runtime_report,
        evidence_bundle_ref: String::from(TASSADAR_SEARCH_NATIVE_EXECUTOR_EVIDENCE_BUNDLE_REF),
        comparison_cells,
        search_native_preferred_case_count,
        verifier_guided_preferred_case_count,
        straight_trace_preferred_case_count,
        refusal_case_count,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report compares search-native, straight-trace, and verifier-guided baselines on seeded search-heavy workloads. It remains benchmark-bound and refusal-bounded, and it does not widen served capability or imply broad learned-compute closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Search-native eval report covers {} workload rows with {} search-native-preferred rows, {} verifier-guided-preferred rows, and {} refusal rows.",
        report.comparison_cells.len(),
        report.search_native_preferred_case_count,
        report.verifier_guided_preferred_case_count,
        report.refusal_case_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_search_native_executor_report|", &report);
    report
}

/// Returns the canonical absolute path for the committed search-native eval report.
#[must_use]
pub fn tassadar_search_native_executor_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SEARCH_NATIVE_EXECUTOR_REPORT_REF)
}

/// Writes the committed search-native eval report.
pub fn write_tassadar_search_native_executor_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSearchNativeExecutorReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_search_native_executor_report();
    let json = serde_json::to_string_pretty(&report).expect("search-native eval report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_search_native_executor_report(
    path: impl AsRef<Path>,
) -> Result<TassadarSearchNativeExecutorReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn preferred_lane(receipt: &psionic_runtime::TassadarSearchNativeRuntimeReceipt) -> &'static str {
    if receipt.status == TassadarSearchNativeRuntimeStatus::BudgetExhaustedRefusal {
        if receipt.verifier_guided_baseline_exactness_bps
            >= receipt.straight_trace_baseline_exactness_bps
        {
            "verifier_guided_search"
        } else {
            "straight_trace_executor"
        }
    } else if receipt.exactness_bps >= receipt.verifier_guided_baseline_exactness_bps
        && receipt.exactness_bps >= receipt.straight_trace_baseline_exactness_bps
    {
        "search_native_executor"
    } else if receipt.verifier_guided_baseline_exactness_bps
        >= receipt.straight_trace_baseline_exactness_bps
    {
        "verifier_guided_search"
    } else {
        "straight_trace_executor"
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
        build_tassadar_search_native_executor_report, load_tassadar_search_native_executor_report,
        tassadar_search_native_executor_report_path,
    };
    use psionic_runtime::TassadarSearchNativeRuntimeStatus;

    #[test]
    fn search_native_executor_report_keeps_route_wins_and_refusal_explicit() {
        let report = build_tassadar_search_native_executor_report();

        assert_eq!(report.comparison_cells.len(), 4);
        assert!(report.comparison_cells.iter().any(|cell| {
            cell.workload_family == "sudoku_backtracking_search"
                && cell.preferred_lane == "search_native_executor"
                && cell.backtrack_count == 2
        }));
        assert!(report.comparison_cells.iter().any(|cell| {
            cell.workload_family == "verifier_heavy_workload_pack"
                && cell.search_native_status
                    == TassadarSearchNativeRuntimeStatus::BudgetExhaustedRefusal
                && cell.preferred_lane == "verifier_guided_search"
        }));
    }

    #[test]
    fn search_native_executor_report_matches_committed_truth() {
        let expected = build_tassadar_search_native_executor_report();
        let committed = load_tassadar_search_native_executor_report(
            tassadar_search_native_executor_report_path(),
        )
        .expect("committed search-native eval report");

        assert_eq!(committed, expected);
    }
}
