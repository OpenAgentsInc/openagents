use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_models::{
    TASSADAR_POINTER_MEMORY_SCRATCHPAD_REPORT_REF, TassadarPointerMemoryScratchpadPublication,
    tassadar_pointer_memory_scratchpad_publication,
};
use psionic_runtime::{
    TASSADAR_POINTER_MEMORY_SCRATCHPAD_RUNTIME_REPORT_REF, TassadarFailureLimitKind,
    TassadarPointerMemoryScratchpadRuntimeReport,
    build_tassadar_pointer_memory_scratchpad_runtime_report,
};

const TASSADAR_POINTER_MEMORY_SCRATCHPAD_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_pointer_memory_scratchpad_study_v1/pointer_memory_scratchpad_ablation_bundle.json";
const REPORT_SCHEMA_VERSION: u16 = 1;

/// Eval-facing workload summary in the separation study.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPointerMemoryScratchpadWorkloadSummary {
    pub workload_family: String,
    pub pointer_quality_bps: u32,
    pub memory_access_quality_bps: u32,
    pub scratchpad_local_reasoning_quality_bps: u32,
    pub locality_score_bps: u32,
    pub primary_limit: TassadarFailureLimitKind,
    pub strongest_axis: String,
    pub note: String,
}

/// Eval-facing report for the separation study.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPointerMemoryScratchpadReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication: TassadarPointerMemoryScratchpadPublication,
    pub runtime_report: TassadarPointerMemoryScratchpadRuntimeReport,
    pub budget_bundle_ref: String,
    pub workload_summaries: Vec<TassadarPointerMemoryScratchpadWorkloadSummary>,
    pub limit_counts: BTreeMap<String, u32>,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

/// Builds the committed separation-study report.
#[must_use]
pub fn build_tassadar_pointer_memory_scratchpad_report() -> TassadarPointerMemoryScratchpadReport {
    let publication = tassadar_pointer_memory_scratchpad_publication();
    let runtime_report = build_tassadar_pointer_memory_scratchpad_runtime_report();
    let workload_summaries = runtime_report
        .workload_receipts
        .iter()
        .map(|receipt| TassadarPointerMemoryScratchpadWorkloadSummary {
            workload_family: receipt.workload_family.clone(),
            pointer_quality_bps: receipt.pointer_quality_bps,
            memory_access_quality_bps: receipt.memory_access_quality_bps,
            scratchpad_local_reasoning_quality_bps: receipt.scratchpad_local_reasoning_quality_bps,
            locality_score_bps: receipt.locality_score_bps,
            primary_limit: receipt.primary_limit,
            strongest_axis: strongest_axis(receipt),
            note: receipt.note.clone(),
        })
        .collect::<Vec<_>>();
    let limit_counts = count_limits(&workload_summaries);
    let mut generated_from_refs = vec![
        String::from(TASSADAR_POINTER_MEMORY_SCRATCHPAD_BUNDLE_REF),
        String::from(TASSADAR_POINTER_MEMORY_SCRATCHPAD_RUNTIME_REPORT_REF),
    ];
    generated_from_refs.extend(
        runtime_report
            .workload_receipts
            .iter()
            .flat_map(|receipt| receipt.benchmark_refs.iter().cloned()),
    );
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let mut report = TassadarPointerMemoryScratchpadReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.pointer_memory_scratchpad.report.v1"),
        publication,
        runtime_report,
        budget_bundle_ref: String::from(TASSADAR_POINTER_MEMORY_SCRATCHPAD_BUNDLE_REF),
        workload_summaries,
        limit_counts,
        generated_from_refs,
        claim_boundary: String::from(
            "this report is a benchmark-bound separation study over pointer prediction, mutable memory access, and scratchpad-local reasoning on shared workloads. It keeps address-selection, memory, and representation limits explicit instead of treating them as one blended learned-executor loss curve",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Pointer/memory/scratchpad report covers {} workloads with primary limit counts {:?}.",
        report.workload_summaries.len(),
        report.limit_counts,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_pointer_memory_scratchpad_report|",
        &report,
    );
    report
}

/// Returns the canonical absolute path for the committed separation-study report.
#[must_use]
pub fn tassadar_pointer_memory_scratchpad_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POINTER_MEMORY_SCRATCHPAD_REPORT_REF)
}

/// Writes the committed separation-study report.
pub fn write_tassadar_pointer_memory_scratchpad_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarPointerMemoryScratchpadReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_pointer_memory_scratchpad_report();
    let json =
        serde_json::to_string_pretty(&report).expect("pointer/memory/scratchpad report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_pointer_memory_scratchpad_report(
    path: impl AsRef<Path>,
) -> Result<TassadarPointerMemoryScratchpadReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn strongest_axis(receipt: &psionic_runtime::TassadarPointerMemoryScratchpadReceipt) -> String {
    let candidates = [
        ("pointer_prediction", receipt.pointer_quality_bps),
        ("mutable_memory_access", receipt.memory_access_quality_bps),
        (
            "scratchpad_local_reasoning",
            receipt.scratchpad_local_reasoning_quality_bps,
        ),
    ];
    String::from(
        candidates
            .into_iter()
            .max_by_key(|(_, score)| *score)
            .expect("study should have at least one mechanism")
            .0,
    )
}

fn count_limits(
    summaries: &[TassadarPointerMemoryScratchpadWorkloadSummary],
) -> BTreeMap<String, u32> {
    let mut counts = BTreeMap::new();
    for summary in summaries {
        *counts
            .entry(summary.primary_limit.as_str().to_string())
            .or_insert(0) += 1;
    }
    counts
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
        build_tassadar_pointer_memory_scratchpad_report,
        load_tassadar_pointer_memory_scratchpad_report,
        tassadar_pointer_memory_scratchpad_report_path,
    };
    use psionic_runtime::TassadarFailureLimitKind;

    #[test]
    fn pointer_memory_scratchpad_report_keeps_primary_limits_explicit() {
        let report = build_tassadar_pointer_memory_scratchpad_report();

        assert_eq!(report.workload_summaries.len(), 4);
        assert_eq!(report.limit_counts.get("memory_limit"), Some(&2));
        assert!(report.workload_summaries.iter().any(|summary| {
            summary.workload_family == "arithmetic_multi_operand"
                && summary.primary_limit == TassadarFailureLimitKind::RepresentationLimit
                && summary.strongest_axis == "scratchpad_local_reasoning"
        }));
    }

    #[test]
    fn pointer_memory_scratchpad_report_matches_committed_truth() {
        let expected = build_tassadar_pointer_memory_scratchpad_report();
        let committed = load_tassadar_pointer_memory_scratchpad_report(
            tassadar_pointer_memory_scratchpad_report_path(),
        )
        .expect("committed separation-study report");

        assert_eq!(committed, expected);
    }
}
