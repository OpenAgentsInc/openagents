use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_eval::build_tassadar_supervision_density_report;

pub const TASSADAR_SUPERVISION_DENSITY_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_supervision_density_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSupervisionDensitySummary {
    pub summary_id: String,
    pub report_id: String,
    pub full_trace_required_workloads: Vec<String>,
    pub strongest_regime_by_workload: Vec<(String, String)>,
    pub claim_boundary: String,
    pub summary_digest: String,
}

#[must_use]
pub fn build_tassadar_supervision_density_summary() -> TassadarSupervisionDensitySummary {
    let report = build_tassadar_supervision_density_report();
    let mut summary = TassadarSupervisionDensitySummary {
        summary_id: String::from("tassadar.supervision_density.summary.v1"),
        report_id: report.report_id,
        full_trace_required_workloads: vec![
            String::from("clrs_shortest_path"),
            String::from("sudoku_backtracking_search"),
        ],
        strongest_regime_by_workload: vec![
            (
                String::from("clrs_shortest_path"),
                String::from("full_trace"),
            ),
            (
                String::from("arithmetic_multi_operand"),
                String::from("partial_state"),
            ),
            (
                String::from("sudoku_backtracking_search"),
                String::from("full_trace"),
            ),
            (
                String::from("module_scale_wasm_loop"),
                String::from("mixed"),
            ),
        ],
        claim_boundary: String::from(
            "this summary remains a research-only supervision-density frontier and does not widen served capability or broad learned-compute claims",
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest =
        stable_digest(b"psionic_tassadar_supervision_density_summary|", &summary);
    summary
}

#[must_use]
pub fn tassadar_supervision_density_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_SUPERVISION_DENSITY_SUMMARY_REF)
}

pub fn write_tassadar_supervision_density_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSupervisionDensitySummary, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let summary = build_tassadar_supervision_density_summary();
    let json =
        serde_json::to_string_pretty(&summary).expect("supervision-density summary serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(summary)
}

#[cfg(test)]
pub fn load_tassadar_supervision_density_summary(
    path: impl AsRef<Path>,
) -> Result<TassadarSupervisionDensitySummary, Box<dyn std::error::Error>> {
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
        build_tassadar_supervision_density_summary, load_tassadar_supervision_density_summary,
        tassadar_supervision_density_summary_path,
    };

    #[test]
    fn supervision_density_summary_keeps_best_regimes_explicit() {
        let summary = build_tassadar_supervision_density_summary();

        assert_eq!(summary.strongest_regime_by_workload.len(), 4);
        assert_eq!(summary.full_trace_required_workloads.len(), 2);
    }

    #[test]
    fn supervision_density_summary_matches_committed_truth() {
        let expected = build_tassadar_supervision_density_summary();
        let committed =
            load_tassadar_supervision_density_summary(tassadar_supervision_density_summary_path())
                .expect("committed supervision-density summary");

        assert_eq!(committed, expected);
    }
}
