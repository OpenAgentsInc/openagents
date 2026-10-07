use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_eval::build_tassadar_pointer_memory_scratchpad_report;
use psionic_models::TASSADAR_POINTER_MEMORY_SCRATCHPAD_SUMMARY_REF;

/// Research-facing summary over the separation study.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPointerMemoryScratchpadSummary {
    pub summary_id: String,
    pub report_id: String,
    pub workloads_by_primary_limit: Vec<(String, Vec<String>)>,
    pub strongest_axis_by_workload: Vec<(String, String)>,
    pub claim_boundary: String,
    pub summary_digest: String,
}

/// Builds the committed research summary for the separation study.
#[must_use]
pub fn build_tassadar_pointer_memory_scratchpad_summary() -> TassadarPointerMemoryScratchpadSummary
{
    let report = build_tassadar_pointer_memory_scratchpad_report();
    let workloads_by_primary_limit = grouped_workloads(&report);
    let strongest_axis_by_workload = report
        .workload_summaries
        .iter()
        .map(|summary| {
            (
                summary.workload_family.clone(),
                summary.strongest_axis.clone(),
            )
        })
        .collect::<Vec<_>>();
    let mut summary = TassadarPointerMemoryScratchpadSummary {
        summary_id: String::from("tassadar.pointer_memory_scratchpad.summary.v1"),
        report_id: report.report_id,
        workloads_by_primary_limit,
        strongest_axis_by_workload,
        claim_boundary: String::from(
            "this summary remains a research-only analytic separation over shared workloads. It keeps address-selection, memory, and representation limits explicit and does not widen served capability or broad learned-compute claims by itself",
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_pointer_memory_scratchpad_summary|",
        &summary,
    );
    summary
}

/// Returns the canonical absolute path for the committed research summary.
#[must_use]
pub fn tassadar_pointer_memory_scratchpad_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POINTER_MEMORY_SCRATCHPAD_SUMMARY_REF)
}

/// Writes the committed research summary.
pub fn write_tassadar_pointer_memory_scratchpad_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarPointerMemoryScratchpadSummary, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let summary = build_tassadar_pointer_memory_scratchpad_summary();
    let json = serde_json::to_string_pretty(&summary)
        .expect("pointer/memory/scratchpad summary serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(summary)
}

#[cfg(test)]
pub fn load_tassadar_pointer_memory_scratchpad_summary(
    path: impl AsRef<Path>,
) -> Result<TassadarPointerMemoryScratchpadSummary, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn grouped_workloads(
    report: &psionic_eval::TassadarPointerMemoryScratchpadReport,
) -> Vec<(String, Vec<String>)> {
    let mut groups = BTreeMap::<String, Vec<String>>::new();
    for summary in &report.workload_summaries {
        groups
            .entry(summary.primary_limit.as_str().to_string())
            .or_default()
            .push(summary.workload_family.clone());
    }
    groups.into_iter().collect()
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
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
        build_tassadar_pointer_memory_scratchpad_summary,
        load_tassadar_pointer_memory_scratchpad_summary,
        tassadar_pointer_memory_scratchpad_summary_path,
    };

    #[test]
    fn pointer_memory_scratchpad_summary_keeps_limit_groups_explicit() {
        let summary = build_tassadar_pointer_memory_scratchpad_summary();

        assert!(summary.strongest_axis_by_workload.contains(&(
            String::from("arithmetic_multi_operand"),
            String::from("scratchpad_local_reasoning")
        )));
        assert!(
            summary
                .workloads_by_primary_limit
                .iter()
                .any(|(limit, workloads)| {
                    limit == "memory_limit"
                        && workloads.contains(&String::from("module_scale_wasm_loop"))
                })
        );
    }

    #[test]
    fn pointer_memory_scratchpad_summary_matches_committed_truth() {
        let expected = build_tassadar_pointer_memory_scratchpad_summary();
        let committed = load_tassadar_pointer_memory_scratchpad_summary(
            tassadar_pointer_memory_scratchpad_summary_path(),
        )
        .expect("committed separation-study summary");

        assert_eq!(committed, expected);
    }
}
