use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_POINTER_MEMORY_SCRATCHPAD_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_pointer_memory_scratchpad_runtime_report.json";

/// Dominant failure limit separated by the study.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFailureLimitKind {
    AddressSelectionLimit,
    MemoryLimit,
    RepresentationLimit,
}

impl TassadarFailureLimitKind {
    /// Returns the stable limit label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AddressSelectionLimit => "address_selection_limit",
            Self::MemoryLimit => "memory_limit",
            Self::RepresentationLimit => "representation_limit",
        }
    }
}

/// One workload receipt in the separation study.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPointerMemoryScratchpadReceipt {
    pub workload_family: String,
    pub pointer_quality_bps: u32,
    pub memory_access_quality_bps: u32,
    pub scratchpad_local_reasoning_quality_bps: u32,
    pub locality_score_bps: u32,
    pub address_selection_error_count: u32,
    pub memory_error_count: u32,
    pub representation_error_count: u32,
    pub primary_limit: TassadarFailureLimitKind,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

/// Runtime report for the separation study.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPointerMemoryScratchpadRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub claim_class: String,
    pub workload_receipts: Vec<TassadarPointerMemoryScratchpadReceipt>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the canonical runtime report for the separation study.
#[must_use]
pub fn build_tassadar_pointer_memory_scratchpad_runtime_report()
-> TassadarPointerMemoryScratchpadRuntimeReport {
    let workload_receipts = vec![
        receipt(
            "clrs_shortest_path",
            7_100,
            7_900,
            6_800,
            7_500,
            9,
            5,
            7,
            TassadarFailureLimitKind::AddressSelectionLimit,
            &[
                "fixtures/tassadar/reports/tassadar_conditional_masking_report.json",
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
            ],
            "graph frontier updates remain dominated by address-selection limits before memory or representation limits",
        ),
        receipt(
            "arithmetic_multi_operand",
            4_200,
            5_100,
            9_300,
            9_400,
            3,
            2,
            11,
            TassadarFailureLimitKind::RepresentationLimit,
            &[
                "fixtures/tassadar/reports/tassadar_locality_scratchpad_report.json",
                "fixtures/tassadar/reports/tassadar_trace_state_ablation_report.json",
            ],
            "arithmetic failures are mostly representation-limited once scratchpad-local reasoning is isolated",
        ),
        receipt(
            "sudoku_backtracking_search",
            7_600,
            5_800,
            6_400,
            7_000,
            6,
            10,
            7,
            TassadarFailureLimitKind::MemoryLimit,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_working_memory_tier_eval_report.json",
            ],
            "search-heavy Sudoku still loses backtrack and candidate state before it loses pointer prediction",
        ),
        receipt(
            "module_scale_wasm_loop",
            6_900,
            5_400,
            7_200,
            6_800,
            5,
            12,
            6,
            TassadarFailureLimitKind::MemoryLimit,
            &[
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
                "fixtures/tassadar/reports/tassadar_working_memory_tier_eval_report.json",
            ],
            "module-scale Wasm remains dominated by mutable-memory pressure even after scratchpad formatting gains",
        ),
    ];
    let mut report = TassadarPointerMemoryScratchpadRuntimeReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.pointer_memory_scratchpad.runtime_report.v1"),
        claim_class: String::from("research_only_architecture / learned_bounded_success"),
        workload_receipts,
        claim_boundary: String::from(
            "this runtime report is a benchmark-bound separation study over pointer prediction, mutable memory access, and scratchpad-local reasoning. It keeps address-selection, memory, and representation limits explicit instead of flattening them into one executor failure mode",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Pointer/memory/scratchpad runtime report covers {} workload receipts with explicit address-selection, memory, and representation limits.",
        report.workload_receipts.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_pointer_memory_scratchpad_runtime_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed runtime report.
#[must_use]
pub fn tassadar_pointer_memory_scratchpad_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POINTER_MEMORY_SCRATCHPAD_RUNTIME_REPORT_REF)
}

/// Writes the committed runtime report.
pub fn write_tassadar_pointer_memory_scratchpad_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarPointerMemoryScratchpadRuntimeReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_pointer_memory_scratchpad_runtime_report();
    let json = serde_json::to_string_pretty(&report)
        .expect("pointer/memory/scratchpad runtime report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_pointer_memory_scratchpad_runtime_report(
    path: impl AsRef<Path>,
) -> Result<TassadarPointerMemoryScratchpadRuntimeReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

#[allow(clippy::too_many_arguments)]
fn receipt(
    workload_family: &str,
    pointer_quality_bps: u32,
    memory_access_quality_bps: u32,
    scratchpad_local_reasoning_quality_bps: u32,
    locality_score_bps: u32,
    address_selection_error_count: u32,
    memory_error_count: u32,
    representation_error_count: u32,
    primary_limit: TassadarFailureLimitKind,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarPointerMemoryScratchpadReceipt {
    TassadarPointerMemoryScratchpadReceipt {
        workload_family: String::from(workload_family),
        pointer_quality_bps,
        memory_access_quality_bps,
        scratchpad_local_reasoning_quality_bps,
        locality_score_bps,
        address_selection_error_count,
        memory_error_count,
        representation_error_count,
        primary_limit,
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
        TassadarFailureLimitKind, build_tassadar_pointer_memory_scratchpad_runtime_report,
        load_tassadar_pointer_memory_scratchpad_runtime_report,
        tassadar_pointer_memory_scratchpad_runtime_report_path,
        write_tassadar_pointer_memory_scratchpad_runtime_report,
    };

    #[test]
    fn pointer_memory_scratchpad_runtime_report_keeps_primary_limits_explicit() {
        let report = build_tassadar_pointer_memory_scratchpad_runtime_report();

        assert_eq!(report.workload_receipts.len(), 4);
        assert!(report.workload_receipts.iter().any(|receipt| {
            receipt.workload_family == "arithmetic_multi_operand"
                && receipt.primary_limit == TassadarFailureLimitKind::RepresentationLimit
        }));
        assert!(report.workload_receipts.iter().any(|receipt| {
            receipt.workload_family == "module_scale_wasm_loop"
                && receipt.primary_limit == TassadarFailureLimitKind::MemoryLimit
        }));
    }

    #[test]
    fn pointer_memory_scratchpad_runtime_report_matches_committed_truth() {
        let expected = build_tassadar_pointer_memory_scratchpad_runtime_report();
        let committed = load_tassadar_pointer_memory_scratchpad_runtime_report(
            tassadar_pointer_memory_scratchpad_runtime_report_path(),
        )
        .expect("committed runtime report");

        assert_eq!(committed, expected);
    }

    #[test]
    fn write_pointer_memory_scratchpad_runtime_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_pointer_memory_scratchpad_runtime_report.json");
        let written = write_tassadar_pointer_memory_scratchpad_runtime_report(&output_path)
            .expect("write report");
        let persisted = load_tassadar_pointer_memory_scratchpad_runtime_report(&output_path)
            .expect("persisted runtime report");

        assert_eq!(written, persisted);
    }
}
