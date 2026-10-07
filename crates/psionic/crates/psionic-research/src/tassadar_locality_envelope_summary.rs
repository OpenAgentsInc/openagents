use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_eval::build_tassadar_locality_envelope_report;
use psionic_models::TASSADAR_LOCALITY_ENVELOPE_SUMMARY_REPORT_REF;

/// Research-facing summary over the computational locality envelopes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityEnvelopeSummary {
    pub summary_id: String,
    pub report_id: String,
    pub exact_winner_counts: BTreeMap<String, u32>,
    pub lowest_lookback_exact_winners: Vec<(String, String)>,
    pub refusal_sensitive_workloads: Vec<String>,
    pub degrade_sensitive_workloads: Vec<String>,
    pub claim_boundary: String,
    pub summary_digest: String,
}

/// Builds the committed research summary for the locality-envelope lane.
#[must_use]
pub fn build_tassadar_locality_envelope_summary() -> TassadarLocalityEnvelopeSummary {
    let report = build_tassadar_locality_envelope_report();
    let mut summary = TassadarLocalityEnvelopeSummary {
        summary_id: String::from("tassadar.locality_envelope.summary.v1"),
        report_id: report.report_id,
        exact_winner_counts: report.exact_winner_counts.clone(),
        lowest_lookback_exact_winners: report
            .workload_summaries
            .iter()
            .filter_map(|workload| {
                workload.lowest_lookback_exact_variant.map(|variant| {
                    (
                        workload.workload_family.as_str().to_string(),
                        variant.as_str().to_string(),
                    )
                })
            })
            .collect(),
        refusal_sensitive_workloads: report
            .workload_summaries
            .iter()
            .filter(|workload| workload.refused_variant_count > 0)
            .map(|workload| workload.workload_family.as_str().to_string())
            .collect(),
        degrade_sensitive_workloads: report
            .workload_summaries
            .iter()
            .filter(|workload| workload.degraded_variant_count >= workload.exact_variant_count)
            .map(|workload| workload.workload_family.as_str().to_string())
            .collect(),
        claim_boundary: String::from(
            "this summary remains a research-only locality analysis over shared workloads. It identifies where locality envelopes support exact fast paths, where downgrade remains bounded, and where the system must refuse rather than silently degrade",
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest =
        stable_digest(b"psionic_tassadar_locality_envelope_summary|", &summary);
    summary
}

/// Returns the canonical absolute path for the committed research summary.
#[must_use]
pub fn tassadar_locality_envelope_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_LOCALITY_ENVELOPE_SUMMARY_REPORT_REF)
}

/// Writes the committed research summary.
pub fn write_tassadar_locality_envelope_summary(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLocalityEnvelopeSummary, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let summary = build_tassadar_locality_envelope_summary();
    let json =
        serde_json::to_string_pretty(&summary).expect("locality-envelope summary serializes");
    fs::write(output_path, format!("{json}\n"))?;
    Ok(summary)
}

#[cfg(test)]
pub fn load_tassadar_locality_envelope_summary(
    path: impl AsRef<Path>,
) -> Result<TassadarLocalityEnvelopeSummary, Box<dyn std::error::Error>> {
    Ok(read_json(path)?)
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
        build_tassadar_locality_envelope_summary, load_tassadar_locality_envelope_summary,
        tassadar_locality_envelope_summary_path,
    };

    #[test]
    fn locality_envelope_summary_keeps_refusal_and_degrade_sensitive_workloads_explicit() {
        let summary = build_tassadar_locality_envelope_summary();

        assert!(
            summary
                .refusal_sensitive_workloads
                .contains(&String::from("sudoku_backtracking_search"))
        );
        assert!(
            summary
                .degrade_sensitive_workloads
                .contains(&String::from("module_scale_wasm_loop"))
        );
    }

    #[test]
    fn locality_envelope_summary_matches_committed_truth() {
        let expected = build_tassadar_locality_envelope_summary();
        let committed =
            load_tassadar_locality_envelope_summary(tassadar_locality_envelope_summary_path())
                .expect("committed locality-envelope summary");

        assert_eq!(committed, expected);
    }
}
