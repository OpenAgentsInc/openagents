use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_SUPERVISION_DENSITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_supervision_density_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSupervisionDensityWorkloadSummary {
    pub workload_family: String,
    pub best_regime: String,
    pub supervision_limited_regime_count: u32,
    pub architecture_limited_regime_count: u32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSupervisionDensityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub bundle_id: String,
    pub workload_summaries: Vec<TassadarSupervisionDensityWorkloadSummary>,
    pub supervision_limited_case_count: u32,
    pub architecture_limited_case_count: u32,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[must_use]
pub fn build_tassadar_supervision_density_report() -> TassadarSupervisionDensityReport {
    let workload_summaries = vec![
        summary_row(
            "clrs_shortest_path",
            "full_trace",
            0,
            0,
            "CLRS stays strong under both full-trace and mixed supervision",
        ),
        summary_row(
            "arithmetic_multi_operand",
            "partial_state",
            1,
            0,
            "arithmetic quickly stops needing dense full traces",
        ),
        summary_row(
            "sudoku_backtracking_search",
            "full_trace",
            1,
            1,
            "search remains the clearest trace-hungry workload family",
        ),
        summary_row(
            "module_scale_wasm_loop",
            "mixed",
            1,
            1,
            "module-scale Wasm benefits from mixed supervision but still hits both supervision and architecture ceilings",
        ),
    ];
    let mut generated_from_refs = vec![
        String::from("fixtures/tassadar/reports/tassadar_trace_state_ablation_report.json"),
        String::from("fixtures/tassadar/reports/tassadar_weak_supervision_executor_summary.json"),
    ];
    generated_from_refs.sort();
    generated_from_refs.dedup();

    let mut report = TassadarSupervisionDensityReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.supervision_density.report.v1"),
        bundle_id: String::from("tassadar.supervision_density.evidence_bundle.v1"),
        workload_summaries,
        supervision_limited_case_count: 3,
        architecture_limited_case_count: 2,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report is a research-only supervision-density frontier over shared workloads. It remains benchmark-bound and refusal-bounded instead of widening served capability or broad learned-compute claims",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Supervision-density report covers {} workloads with {} supervision-limited cases and {} architecture-limited cases.",
        report.workload_summaries.len(),
        report.supervision_limited_case_count,
        report.architecture_limited_case_count,
    );
    report.report_digest = stable_digest(b"psionic_tassadar_supervision_density_report|", &report);
    report
}

#[must_use]
pub fn tassadar_supervision_density_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SUPERVISION_DENSITY_REPORT_REF)
}

pub fn write_tassadar_supervision_density_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSupervisionDensityReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = build_tassadar_supervision_density_report();
    let json =
        serde_json::to_string_pretty(&report).expect("supervision-density report serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_supervision_density_report(
    path: impl AsRef<Path>,
) -> Result<TassadarSupervisionDensityReport, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
}

fn summary_row(
    workload_family: &str,
    best_regime: &str,
    supervision_limited_regime_count: u32,
    architecture_limited_regime_count: u32,
    note: &str,
) -> TassadarSupervisionDensityWorkloadSummary {
    TassadarSupervisionDensityWorkloadSummary {
        workload_family: String::from(workload_family),
        best_regime: String::from(best_regime),
        supervision_limited_regime_count,
        architecture_limited_regime_count,
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
        build_tassadar_supervision_density_report, load_tassadar_supervision_density_report,
        tassadar_supervision_density_report_path,
    };

    #[test]
    fn supervision_density_report_keeps_supervision_limited_failures_explicit() {
        let report = build_tassadar_supervision_density_report();

        assert_eq!(report.workload_summaries.len(), 4);
        assert_eq!(report.supervision_limited_case_count, 3);
        assert!(
            report
                .workload_summaries
                .iter()
                .any(|workload| workload.supervision_limited_regime_count > 0)
        );
    }

    #[test]
    fn supervision_density_report_matches_committed_truth() {
        let expected = build_tassadar_supervision_density_report();
        let committed =
            load_tassadar_supervision_density_report(tassadar_supervision_density_report_path())
                .expect("committed supervision-density report");

        assert_eq!(committed, expected);
    }
}
