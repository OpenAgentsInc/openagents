use std::{fs, path::Path};

use psionic_eval::{
    build_tassadar_verifier_guided_search_evaluation_report,
    TassadarVerifierGuidedSearchEvaluationReport,
    TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_REF,
};
use psionic_runtime::TassadarVerifierGuidedSearchWorkloadFamily;
#[cfg(test)]
use serde::de::DeserializeOwned;
#[cfg(test)]
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_OUTPUT_DIR: &str =
    "fixtures/tassadar/reports";
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_FILE: &str =
    "tassadar_verifier_guided_search_architecture_report.json";
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_verifier_guided_search_architecture_report.json";
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_verifier_guided_search_architecture_report";
pub const TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_TEST_COMMAND: &str =
    "cargo test -p psionic-research verifier_guided_search_architecture_report_matches_committed_truth -- --nocapture";

/// Family-level summary in the verifier-guided search architecture report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchArchitectureFamilySummary {
    /// Search workload family.
    pub workload_family: TassadarVerifierGuidedSearchWorkloadFamily,
    /// Human-readable trainability summary.
    pub trainability_summary: String,
    /// Explicit claim boundary for the family.
    pub claim_boundary: String,
}

/// Repo-facing research summary for the verifier-guided search lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchArchitectureReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable repo-relative report reference.
    pub report_ref: String,
    /// Regeneration commands for the report.
    pub regeneration_commands: Vec<String>,
    /// Canonical source report ref.
    pub source_report_ref: String,
    /// Stable source report digest.
    pub source_report_digest: String,
    /// Family-level summaries.
    pub family_summaries: Vec<TassadarVerifierGuidedSearchArchitectureFamilySummary>,
    /// Plain-language top-level claim boundary.
    pub claim_boundary: String,
    /// Plain-language top-level summary.
    pub summary: String,
    /// Stable report digest.
    pub report_digest: String,
}

/// Verifier-guided search architecture report failure.
#[derive(Debug, Error)]
pub enum TassadarVerifierGuidedSearchArchitectureReportError {
    /// Eval report build failed.
    #[error(transparent)]
    Eval(#[from] psionic_eval::TassadarVerifierGuidedSearchEvaluationReportError),
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

/// Builds the verifier-guided search architecture summary from the committed eval report.
pub fn build_tassadar_verifier_guided_search_architecture_report(
) -> Result<TassadarVerifierGuidedSearchArchitectureReport, TassadarVerifierGuidedSearchArchitectureReportError>
{
    let evaluation_report = build_tassadar_verifier_guided_search_evaluation_report()?;
    Ok(build_from_evaluation_report(&evaluation_report))
}

/// Writes the verifier-guided search architecture summary to the requested output directory.
pub fn run_tassadar_verifier_guided_search_architecture_report(
    output_dir: &Path,
) -> Result<TassadarVerifierGuidedSearchArchitectureReport, TassadarVerifierGuidedSearchArchitectureReportError>
{
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarVerifierGuidedSearchArchitectureReportError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    let report = build_tassadar_verifier_guided_search_architecture_report()?;
    let output_path = output_dir.join(TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_FILE);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarVerifierGuidedSearchArchitectureReportError::Write {
            path: output_dir.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_from_evaluation_report(
    evaluation_report: &TassadarVerifierGuidedSearchEvaluationReport,
) -> TassadarVerifierGuidedSearchArchitectureReport {
    let mut family_summaries = evaluation_report
        .family_summaries
        .iter()
        .map(|summary| TassadarVerifierGuidedSearchArchitectureFamilySummary {
            workload_family: summary.workload_family,
            trainability_summary: format!(
                "family={:?}, mean_guesses={}, mean_backtracks={}, verifier_accuracy={}bps, backtrack_exactness={}bps, recovery_quality={}bps",
                summary.workload_family,
                summary.mean_guess_count,
                summary.mean_backtrack_count,
                summary.verifier_certificate_accuracy_bps,
                summary.backtrack_exactness_bps,
                summary.recovery_quality_bps
            ),
            claim_boundary: String::from(
                "research-only verifier-guided search trace family; explicit guess, contradiction, and recovery steps are surfaced for trainability study and do not imply compiled correctness or a general solver",
            ),
        })
        .collect::<Vec<_>>();
    family_summaries.sort_by_key(|summary| summary.workload_family);

    let mut report = TassadarVerifierGuidedSearchArchitectureReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.verifier_guided_search.architecture_report.v1"),
        report_ref: String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_REPORT_REF),
        regeneration_commands: vec![
            String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_EXAMPLE_COMMAND),
            String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_TEST_COMMAND),
        ],
        source_report_ref: String::from(TASSADAR_VERIFIER_GUIDED_SEARCH_REPORT_REF),
        source_report_digest: evaluation_report.report_digest.clone(),
        family_summaries,
        claim_boundary: String::from(
            "this report freezes one research-only verifier-guided search architecture summary over seeded Sudoku backtracking and bounded search-kernel traces. It does not imply compiled correctness, general combinatorial closure, or served promotion",
        ),
        summary: String::from(
            "Verifier-guided search architecture summary now freezes the seeded Sudoku backtracking and search-kernel families under explicit guess, verify, contradiction, and backtrack semantics. The lane remains research-only and separate from deterministic compiled executor claims",
        ),
        report_digest: String::new(),
    };
    report.report_digest =
        stable_digest(b"psionic_tassadar_verifier_guided_search_architecture_report|", &report);
    report
}

#[cfg(test)]
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("verifier-guided search architecture report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_verifier_guided_search_architecture_report,
        read_repo_json,
        run_tassadar_verifier_guided_search_architecture_report,
        TassadarVerifierGuidedSearchArchitectureReport,
        TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_FILE,
        TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_OUTPUT_DIR,
        TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_REPORT_REF,
    };

    #[test]
    fn verifier_guided_search_architecture_report_is_machine_legible(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_verifier_guided_search_architecture_report()?;
        assert_eq!(report.family_summaries.len(), 2);
        assert!(!report.report_digest.is_empty());
        Ok(())
    }

    #[test]
    fn verifier_guided_search_architecture_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_verifier_guided_search_architecture_report()?;
        let persisted: TassadarVerifierGuidedSearchArchitectureReport =
            read_repo_json(TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_REPORT_REF)?;
        assert_eq!(persisted, report);
        Ok(())
    }

    #[test]
    fn verifier_guided_search_architecture_report_writes_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output_dir = tempdir()?;
        let report = run_tassadar_verifier_guided_search_architecture_report(output_dir.path())?;
        let persisted: TassadarVerifierGuidedSearchArchitectureReport =
            serde_json::from_slice(&std::fs::read(
                output_dir
                    .path()
                    .join(TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_FILE),
            )?)?;
        assert_eq!(persisted, report);
        assert!(TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_OUTPUT_DIR.contains(
            "fixtures/tassadar/reports"
        ));
        Ok(())
    }
}

#[cfg(test)]
fn read_repo_json<T>(repo_relative_path: &str) -> Result<T, Box<dyn std::error::Error>>
where
    T: DeserializeOwned,
{
    let path = repo_root().join(repo_relative_path);
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}
