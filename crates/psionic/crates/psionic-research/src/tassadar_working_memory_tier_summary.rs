use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TASSADAR_WORKING_MEMORY_TIER_EVAL_REPORT_REF, TassadarWorkingMemoryTierEvalReport,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_WORKING_MEMORY_TIER_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_working_memory_tier_summary.json";

/// Research summary for the working-memory tier lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarWorkingMemoryTierSummaryReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Eval report consumed by the summary.
    pub eval_report: TassadarWorkingMemoryTierEvalReport,
    /// Workload families that currently look like bounded widening cases.
    pub widening_workload_families: Vec<String>,
    /// Workload families that currently look like trace reshaping only.
    pub trace_shaping_only_workload_families: Vec<String>,
    /// Widening families that also carry explicit refusal on nearby cases.
    pub fragile_widening_workload_families: Vec<String>,
    /// Refused bounded case identifiers.
    pub refused_case_ids: Vec<String>,
    /// Explicit next unknowns for the research lane.
    pub follow_on_unknowns: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Summary failure while building or writing the working-memory tier report.
#[derive(Debug, Error)]
pub enum TassadarWorkingMemoryTierSummaryError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the machine-readable working-memory tier summary report.
pub fn build_tassadar_working_memory_tier_summary_report()
-> Result<TassadarWorkingMemoryTierSummaryReport, TassadarWorkingMemoryTierSummaryError> {
    let eval_report: TassadarWorkingMemoryTierEvalReport =
        read_repo_json(TASSADAR_WORKING_MEMORY_TIER_EVAL_REPORT_REF)?;
    let widening_workload_families = eval_report
        .family_summaries
        .iter()
        .filter(|summary| summary.widening_case_count > 0)
        .map(|summary| String::from(summary.workload_family.as_str()))
        .collect::<Vec<_>>();
    let trace_shaping_only_workload_families = eval_report
        .family_summaries
        .iter()
        .filter(|summary| {
            summary.trace_shaping_only_case_count > 0 && summary.widening_case_count == 0
        })
        .map(|summary| String::from(summary.workload_family.as_str()))
        .collect::<Vec<_>>();
    let fragile_widening_workload_families = eval_report
        .family_summaries
        .iter()
        .filter(|summary| summary.widening_case_count > 0 && summary.refused_case_count > 0)
        .map(|summary| String::from(summary.workload_family.as_str()))
        .collect::<Vec<_>>();
    let refused_case_ids = eval_report.refused_case_ids.clone();
    let mut report = TassadarWorkingMemoryTierSummaryReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.working_memory_tier.summary.v1"),
        eval_report,
        widening_workload_families,
        trace_shaping_only_workload_families,
        fragile_widening_workload_families,
        refused_case_ids,
        follow_on_unknowns: vec![
            String::from(
                "whether bounded working-memory publication stays exact on wider sort and graph-locality families instead of only shrinking trace volume",
            ),
            String::from(
                "whether associative recall can widen beyond the current bounded key-space without collapsing into opaque external lookup semantics",
            ),
            String::from(
                "whether write-heavy working-memory tiers preserve enough replay and lineage truth to justify anything beyond research-only posture",
            ),
        ],
        claim_boundary: String::from(
            "this summary keeps the working-memory tier as a research-only Psionic-owned architecture surface. Widening families, trace-shaping-only families, and fragile refusal boundaries stay explicit, and none of them widen served capability or imply arbitrary-memory closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Working-memory tier summary now marks {} widening workload families, {} trace-shaping-only workload families, {} fragile widening families, and {} refused bounded cases.",
        report.widening_workload_families.len(),
        report.trace_shaping_only_workload_families.len(),
        report.fragile_widening_workload_families.len(),
        report.refused_case_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_working_memory_tier_summary_report|",
        &report,
    );
    Ok(report)
}

/// Returns the canonical absolute path for the committed summary report.
#[must_use]
pub fn tassadar_working_memory_tier_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WORKING_MEMORY_TIER_SUMMARY_REPORT_REF)
}

/// Writes the committed working-memory tier summary report.
pub fn write_tassadar_working_memory_tier_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarWorkingMemoryTierSummaryReport, TassadarWorkingMemoryTierSummaryError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarWorkingMemoryTierSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_working_memory_tier_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarWorkingMemoryTierSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarWorkingMemoryTierSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarWorkingMemoryTierSummaryError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarWorkingMemoryTierSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
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
        TASSADAR_WORKING_MEMORY_TIER_SUMMARY_REPORT_REF, TassadarWorkingMemoryTierSummaryReport,
        build_tassadar_working_memory_tier_summary_report, read_repo_json,
        tassadar_working_memory_tier_summary_report_path,
        write_tassadar_working_memory_tier_summary_report,
    };

    #[test]
    fn working_memory_summary_marks_widening_fragility_and_trace_shaping() {
        let report = build_tassadar_working_memory_tier_summary_report().expect("summary");

        assert!(
            report
                .widening_workload_families
                .contains(&String::from("copy_window"))
        );
        assert!(
            report
                .trace_shaping_only_workload_families
                .contains(&String::from("stable_sort"))
        );
        assert!(
            report
                .fragile_widening_workload_families
                .contains(&String::from("associative_recall"))
        );
        assert!(
            report
                .refused_case_ids
                .contains(&String::from("associative_recall_overflow"))
        );
    }

    #[test]
    fn working_memory_summary_matches_committed_truth() {
        let generated = build_tassadar_working_memory_tier_summary_report().expect("summary");
        let committed: TassadarWorkingMemoryTierSummaryReport =
            read_repo_json(TASSADAR_WORKING_MEMORY_TIER_SUMMARY_REPORT_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_working_memory_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_working_memory_tier_summary.json");
        let written =
            write_tassadar_working_memory_tier_summary_report(&output_path).expect("write");
        let persisted: TassadarWorkingMemoryTierSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_working_memory_tier_summary_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_working_memory_tier_summary.json")
        );
    }
}
