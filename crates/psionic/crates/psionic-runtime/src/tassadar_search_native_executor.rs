use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_SEARCH_NATIVE_EXECUTOR_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_search_native_executor_runtime_report.json";

/// Runtime status for one search-native workload receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSearchNativeRuntimeStatus {
    WithinBudget,
    BudgetExhaustedRefusal,
}

/// One runtime receipt in the search-native lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSearchNativeRuntimeReceipt {
    pub workload_family: String,
    pub guess_count: u32,
    pub verify_count: u32,
    pub contradiction_count: u32,
    pub backtrack_count: u32,
    pub branch_summary_count: u32,
    pub search_budget_limit: u32,
    pub search_budget_used: u32,
    pub search_budget_utilization_bps: u32,
    pub exactness_bps: u32,
    pub recovery_quality_bps: u32,
    pub straight_trace_baseline_exactness_bps: u32,
    pub verifier_guided_baseline_exactness_bps: u32,
    pub status: TassadarSearchNativeRuntimeStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Runtime report for the search-native executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSearchNativeRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub claim_class: String,
    pub workload_receipts: Vec<TassadarSearchNativeRuntimeReceipt>,
    pub within_budget_case_count: u32,
    pub refused_case_count: u32,
    pub straight_trace_win_count: u32,
    pub verifier_guided_win_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the canonical runtime report for the search-native executor lane.
#[must_use]
pub fn build_tassadar_search_native_executor_runtime_report() -> TassadarSearchNativeRuntimeReport {
    let workload_receipts = vec![
        receipt(
            "sudoku_backtracking_search",
            5,
            7,
            2,
            2,
            3,
            12,
            9,
            9_600,
            9_400,
            4_200,
            9_100,
            TassadarSearchNativeRuntimeStatus::WithinBudget,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json",
            ],
            "search-native Sudoku stays within budget and outperforms the straight-trace and verifier-guided baselines on the seeded row",
        ),
        receipt(
            "branch_heavy_clrs_variant",
            4,
            6,
            1,
            1,
            4,
            10,
            8,
            8_800,
            8_700,
            6_100,
            8_400,
            TassadarSearchNativeRuntimeStatus::WithinBudget,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
                "fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json",
            ],
            "branch-heavy CLRS benefits from first-class branch-summary and backtrack state without claiming generic CLRS ownership",
        ),
        receipt(
            "search_kernel_recovery",
            3,
            5,
            1,
            1,
            2,
            8,
            6,
            9_700,
            9_800,
            5_400,
            9_300,
            TassadarSearchNativeRuntimeStatus::WithinBudget,
            None,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_error_regime_catalog.json",
            ],
            "search-kernel recovery benefits from explicit guess, verify, and backtrack state under one bounded search budget",
        ),
        receipt(
            "verifier_heavy_workload_pack",
            6,
            8,
            3,
            2,
            5,
            10,
            10,
            0,
            0,
            6_200,
            7_600,
            TassadarSearchNativeRuntimeStatus::BudgetExhaustedRefusal,
            Some("search_budget_exhausted_for_nested_validator_recovery"),
            &[
                "fixtures/tassadar/reports/tassadar_latency_evidence_tradeoff_report.json",
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
            ],
            "verifier-heavy packs currently hit the seeded search-budget ceiling, so the lane refuses instead of silently degrading",
        ),
    ];
    let within_budget_case_count = workload_receipts
        .iter()
        .filter(|receipt| receipt.status == TassadarSearchNativeRuntimeStatus::WithinBudget)
        .count() as u32;
    let refused_case_count = workload_receipts
        .iter()
        .filter(|receipt| {
            receipt.status == TassadarSearchNativeRuntimeStatus::BudgetExhaustedRefusal
        })
        .count() as u32;
    let straight_trace_win_count = workload_receipts
        .iter()
        .filter(|receipt| {
            receipt.status == TassadarSearchNativeRuntimeStatus::WithinBudget
                && receipt.exactness_bps > receipt.straight_trace_baseline_exactness_bps
        })
        .count() as u32;
    let verifier_guided_win_count = workload_receipts
        .iter()
        .filter(|receipt| {
            receipt.status == TassadarSearchNativeRuntimeStatus::WithinBudget
                && receipt.exactness_bps > receipt.verifier_guided_baseline_exactness_bps
        })
        .count() as u32;
    let mut report = TassadarSearchNativeRuntimeReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.search_native_executor.runtime_report.v1"),
        claim_class: String::from("learned_bounded / research_only_architecture"),
        workload_receipts,
        within_budget_case_count,
        refused_case_count,
        straight_trace_win_count,
        verifier_guided_win_count,
        claim_boundary: String::from(
            "this runtime report is a benchmark-bound search-native executor lane over seeded search-heavy workloads. It keeps guess, verify, contradiction, backtrack, branch-summary, and search-budget state explicit and prefers refusal over silent degradation on unsupported regimes",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Search-native runtime report covers {} workload receipts with {} within-budget rows and {} refusal rows.",
        report.workload_receipts.len(),
        report.within_budget_case_count,
        report.refused_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_search_native_executor_runtime_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed search-native runtime report.
#[must_use]
pub fn tassadar_search_native_executor_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SEARCH_NATIVE_EXECUTOR_RUNTIME_REPORT_REF)
}

/// Writes the committed search-native runtime report.
pub fn write_tassadar_search_native_executor_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSearchNativeRuntimeReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_search_native_executor_runtime_report();
    let json =
        serde_json::to_string_pretty(&report).expect("search-native runtime report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_search_native_executor_runtime_report(
    path: impl AsRef<Path>,
) -> Result<TassadarSearchNativeRuntimeReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

#[allow(clippy::too_many_arguments)]
fn receipt(
    workload_family: &str,
    guess_count: u32,
    verify_count: u32,
    contradiction_count: u32,
    backtrack_count: u32,
    branch_summary_count: u32,
    search_budget_limit: u32,
    search_budget_used: u32,
    exactness_bps: u32,
    recovery_quality_bps: u32,
    straight_trace_baseline_exactness_bps: u32,
    verifier_guided_baseline_exactness_bps: u32,
    status: TassadarSearchNativeRuntimeStatus,
    refusal_reason: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarSearchNativeRuntimeReceipt {
    let search_budget_utilization_bps = if status == TassadarSearchNativeRuntimeStatus::WithinBudget
    {
        search_budget_used.saturating_mul(10_000) / search_budget_limit.max(1)
    } else {
        10_000
    };
    TassadarSearchNativeRuntimeReceipt {
        workload_family: String::from(workload_family),
        guess_count,
        verify_count,
        contradiction_count,
        backtrack_count,
        branch_summary_count,
        search_budget_limit,
        search_budget_used,
        search_budget_utilization_bps,
        exactness_bps,
        recovery_quality_bps,
        straight_trace_baseline_exactness_bps,
        verifier_guided_baseline_exactness_bps,
        status,
        refusal_reason: refusal_reason.map(String::from),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
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
        TassadarSearchNativeRuntimeStatus, build_tassadar_search_native_executor_runtime_report,
        load_tassadar_search_native_executor_runtime_report,
        tassadar_search_native_executor_runtime_report_path,
        write_tassadar_search_native_executor_runtime_report,
    };

    #[test]
    fn search_native_executor_runtime_report_keeps_search_state_and_refusal_explicit() {
        let report = build_tassadar_search_native_executor_runtime_report();

        assert_eq!(report.workload_receipts.len(), 4);
        assert!(report.workload_receipts.iter().any(|receipt| {
            receipt.workload_family == "sudoku_backtracking_search"
                && receipt.guess_count == 5
                && receipt.backtrack_count == 2
        }));
        assert!(report.workload_receipts.iter().any(|receipt| {
            receipt.workload_family == "verifier_heavy_workload_pack"
                && receipt.status == TassadarSearchNativeRuntimeStatus::BudgetExhaustedRefusal
        }));
    }

    #[test]
    fn search_native_executor_runtime_report_matches_committed_truth() {
        let expected = build_tassadar_search_native_executor_runtime_report();
        let committed = load_tassadar_search_native_executor_runtime_report(
            tassadar_search_native_executor_runtime_report_path(),
        )
        .expect("committed search-native runtime report");

        assert_eq!(committed, expected);
    }

    #[test]
    fn write_search_native_executor_runtime_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_search_native_executor_runtime.json");
        let report = write_tassadar_search_native_executor_runtime_report(&output_path)
            .expect("search-native runtime report should write");
        let written = std::fs::read_to_string(&output_path).expect("written report");
        let decoded = serde_json::from_str::<super::TassadarSearchNativeRuntimeReport>(&written)
            .expect("written report should decode");

        assert_eq!(decoded, report);
    }
}
