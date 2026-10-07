use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    build_tassadar_decompilation_fidelity_report, TassadarDecompilationFidelityReport,
};
use psionic_models::{
    tassadar_decompilable_executor_publication, TassadarDecompilableExecutorPublication,
    TassadarDecompilationFamily, TassadarDecompilationStabilityClass,
    TASSADAR_DECOMPILABLE_EXECUTOR_ARTIFACTS_REPORT_REF,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_DECOMPILABLE_EXECUTOR_ARTIFACTS_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_decompilable_executor_artifacts";
pub const TASSADAR_DECOMPILABLE_EXECUTOR_ARTIFACTS_TEST_COMMAND: &str =
    "cargo test -p psionic-research decompilable_executor_artifacts_report_matches_committed_truth -- --nocapture";

/// One provider-facing-ready artifact summary for the seeded decompilation lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecompilationArtifactSummary {
    /// Stable case identifier.
    pub case_id: String,
    /// Seeded symbolic-reference case identifier.
    pub source_case_id: String,
    /// Constrained learned family.
    pub family: TassadarDecompilationFamily,
    /// Candidate model identifier.
    pub candidate_model_id: String,
    /// Stable reference program digest.
    pub reference_program_digest: String,
    /// Number of seeded retrains compared for the case.
    pub retrain_count: u32,
    /// Number of distinct readable forms observed across retrains.
    pub distinct_readable_program_count: u32,
    /// Case-level stability class.
    pub stability_class: TassadarDecompilationStabilityClass,
    /// Stable benchmark refs anchoring the case.
    pub benchmark_refs: Vec<String>,
    /// Whether the case is receipt-ready for provider projection.
    pub receipt_ready: bool,
    /// Plain-language summary for the case.
    pub summary: String,
}

/// Committed research report for the decompilable learned executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecompilableExecutorArtifactsReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Repo-facing publication for the lane.
    pub publication: TassadarDecompilableExecutorPublication,
    /// Eval-facing fidelity report.
    pub fidelity_report: TassadarDecompilationFidelityReport,
    /// Ordered artifact summaries.
    pub artifact_summaries: Vec<TassadarDecompilationArtifactSummary>,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Plain-language report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarDecompilableExecutorArtifactsReport {
    fn new(
        publication: TassadarDecompilableExecutorPublication,
        fidelity_report: TassadarDecompilationFidelityReport,
        artifact_summaries: Vec<TassadarDecompilationArtifactSummary>,
    ) -> Self {
        let exact_form_count = artifact_summaries
            .iter()
            .filter(|summary| {
                summary.stability_class == TassadarDecompilationStabilityClass::StableExactForm
            })
            .count();
        let equivalent_form_count = artifact_summaries
            .iter()
            .filter(|summary| {
                summary.stability_class
                    == TassadarDecompilationStabilityClass::StableEquivalentForms
            })
            .count();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.decompilable_executor_artifacts.report.v1"),
            publication,
            fidelity_report,
            artifact_summaries,
            claim_boundary: String::from(
                "this report freezes one research-only artifact story for decompilable learned executor candidates over seeded bounded symbolic kernels; it proves readable decompilation receipts, compiled-reference comparison, and retrain-stability facts for the published families only, and does not imply broad learned exactness, arbitrary Wasm closure, or served promotion",
            ),
            summary: format!(
                "The decompilable learned executor lane now carries {} seeded receipt-ready cases: {} stable exact readable forms and {} stable equivalent renamed forms against compiled symbolic references.",
                exact_form_count + equivalent_form_count,
                exact_form_count,
                equivalent_form_count,
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_decompilable_executor_artifacts_report|",
            &report,
        );
        report
    }
}

/// Report build failures for the decompilable learned executor lane.
#[derive(Debug, Error)]
pub enum TassadarDecompilableExecutorArtifactsReportError {
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Downstream eval report construction failed.
    #[error(transparent)]
    Fidelity(#[from] psionic_eval::TassadarDecompilationFidelityReportError),
    /// JSON serialization or parsing failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed research report for the decompilable learned executor lane.
pub fn build_tassadar_decompilable_executor_artifacts_report() -> Result<
    TassadarDecompilableExecutorArtifactsReport,
    TassadarDecompilableExecutorArtifactsReportError,
> {
    let publication = tassadar_decompilable_executor_publication();
    let fidelity_report = build_tassadar_decompilation_fidelity_report()?;
    let cases = publication
        .cases
        .iter()
        .map(|case| (case.case_id.clone(), case))
        .collect::<BTreeMap<_, _>>();
    let artifact_summaries = fidelity_report
        .case_reports
        .iter()
        .map(|case_report| {
            let publication_case = cases
                .get(case_report.case_id.as_str())
                .expect("publication case should exist");
            TassadarDecompilationArtifactSummary {
                case_id: case_report.case_id.clone(),
                source_case_id: case_report.source_case_id.clone(),
                family: case_report.family,
                candidate_model_id: publication.model.model_id.clone(),
                reference_program_digest: case_report.reference_program_digest.clone(),
                retrain_count: case_report.retrain_count,
                distinct_readable_program_count: case_report.distinct_readable_program_count,
                stability_class: case_report.stability_class,
                benchmark_refs: publication_case.benchmark_refs.clone(),
                receipt_ready: case_report.semantic_equivalence_bps == 10_000
                    && case_report.readable_equivalence_bps == 10_000,
                summary: format!(
                    "case `{}` stays receipt-ready with {} retrains, {} distinct readable forms, and stability class `{}`",
                    case_report.source_case_id,
                    case_report.retrain_count,
                    case_report.distinct_readable_program_count,
                    stability_label(case_report.stability_class),
                ),
            }
        })
        .collect::<Vec<_>>();
    Ok(TassadarDecompilableExecutorArtifactsReport::new(
        publication,
        fidelity_report,
        artifact_summaries,
    ))
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_decompilable_executor_artifacts_report_path() -> PathBuf {
    repo_root().join(TASSADAR_DECOMPILABLE_EXECUTOR_ARTIFACTS_REPORT_REF)
}

/// Writes the committed research report for the decompilable learned executor lane.
pub fn write_tassadar_decompilable_executor_artifacts_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarDecompilableExecutorArtifactsReport,
    TassadarDecompilableExecutorArtifactsReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarDecompilableExecutorArtifactsReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_decompilable_executor_artifacts_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarDecompilableExecutorArtifactsReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn stability_label(class: TassadarDecompilationStabilityClass) -> &'static str {
    match class {
        TassadarDecompilationStabilityClass::StableExactForm => "stable_exact_form",
        TassadarDecompilationStabilityClass::StableEquivalentForms => "stable_equivalent_forms",
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    repo_relative_path: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    let path = repo_root().join(repo_relative_path);
    let bytes = std::fs::read(&path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_decompilable_executor_artifacts_report,
        tassadar_decompilable_executor_artifacts_report_path,
        write_tassadar_decompilable_executor_artifacts_report,
        TassadarDecompilableExecutorArtifactsReport,
        TASSADAR_DECOMPILABLE_EXECUTOR_ARTIFACTS_REPORT_REF,
    };

    #[test]
    fn decompilable_executor_artifacts_report_keeps_receipt_ready_cases(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_decompilable_executor_artifacts_report()?;
        assert_eq!(report.artifact_summaries.len(), 5);
        assert!(report
            .artifact_summaries
            .iter()
            .all(|summary| summary.receipt_ready));
        assert!(report
            .artifact_summaries
            .iter()
            .any(|summary| summary.distinct_readable_program_count > 1));
        Ok(())
    }

    #[test]
    fn decompilable_executor_artifacts_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_decompilable_executor_artifacts_report()?;
        let committed: TassadarDecompilableExecutorArtifactsReport =
            super::read_repo_json(TASSADAR_DECOMPILABLE_EXECUTOR_ARTIFACTS_REPORT_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_decompilable_executor_artifacts_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let output_path = temp_dir
            .path()
            .join("tassadar_decompilable_executor_artifacts_report.json");
        let written = write_tassadar_decompilable_executor_artifacts_report(&output_path)?;
        let bytes = std::fs::read(&output_path)?;
        let roundtrip: TassadarDecompilableExecutorArtifactsReport =
            serde_json::from_slice(&bytes)?;
        assert_eq!(written, roundtrip);
        assert_eq!(
            tassadar_decompilable_executor_artifacts_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_decompilable_executor_artifacts_report.json")
        );
        Ok(())
    }
}
