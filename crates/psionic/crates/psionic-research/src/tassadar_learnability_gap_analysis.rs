use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    build_tassadar_learnability_gap_report, TassadarLearnabilityGapClass,
    TassadarLearnabilityGapReport, TassadarLearnabilityGapReportError,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_LEARNABILITY_GAP_ANALYSIS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_learnability_gap_analysis_report.json";
pub const TASSADAR_LEARNABILITY_GAP_ANALYSIS_EXAMPLE_COMMAND: &str =
    "cargo run -p psionic-research --example tassadar_learnability_gap_analysis";
pub const TASSADAR_LEARNABILITY_GAP_ANALYSIS_TEST_COMMAND: &str =
    "cargo test -p psionic-research learnability_gap_analysis_report_matches_committed_truth -- --nocapture";

const REPORT_SCHEMA_VERSION: u16 = 1;

/// One recommended next action attached to the learnability-gap report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnabilityGapAction {
    /// Stable gap class the action addresses.
    pub gap_class: TassadarLearnabilityGapClass,
    /// Plain-language action summary.
    pub action: String,
}

/// Committed research summary for learnability-gap analysis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLearnabilityGapAnalysisReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Eval-facing learnability-gap report.
    pub eval_report: TassadarLearnabilityGapReport,
    /// Ordered next actions.
    pub recommended_actions: Vec<TassadarLearnabilityGapAction>,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarLearnabilityGapAnalysisReport {
    fn new(
        eval_report: TassadarLearnabilityGapReport,
        recommended_actions: Vec<TassadarLearnabilityGapAction>,
    ) -> Self {
        let materially_actionable_case_count = eval_report
            .case_reports
            .iter()
            .filter(|case| case.materially_actionable)
            .count();
        let recommended_action_count = recommended_actions.len();
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.learnability_gap.analysis.report.v1"),
            eval_report,
            recommended_actions,
            claim_boundary: String::from(
                "this report summarizes bounded learnability-gap evidence across seeded executor families only; it recommends next actions for the current gap classes without promoting any learned lane, extrapolating beyond the measured families, or treating the classification itself as closure",
            ),
            summary: format!(
                "The learnability-gap tranche now freezes {} materially actionable cases and {} gap-class-specific next actions across kernel, Sudoku, Hungarian, and CLRS-to-Wasm evidence.",
                materially_actionable_case_count,
                recommended_action_count,
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_learnability_gap_analysis_report|",
            &report,
        );
        report
    }
}

/// Learnability-gap analysis report build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarLearnabilityGapAnalysisReportError {
    /// Eval report construction failed.
    #[error(transparent)]
    Eval(#[from] TassadarLearnabilityGapReportError),
    /// Failed to create an output directory.
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the report.
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed learnability-gap research summary.
pub fn build_tassadar_learnability_gap_analysis_report(
) -> Result<TassadarLearnabilityGapAnalysisReport, TassadarLearnabilityGapAnalysisReportError> {
    let eval_report = build_tassadar_learnability_gap_report()?;
    let recommended_actions = eval_report
        .summary_rows
        .iter()
        .map(|row| TassadarLearnabilityGapAction {
            gap_class: row.gap_class,
            action: match row.gap_class {
                TassadarLearnabilityGapClass::PositionalSchemeMiss => String::from(
                    "widen the scratchpad and position-scheme matrix before treating kernel-format failures as architecture limits",
                ),
                TassadarLearnabilityGapClass::SupervisionMiss => String::from(
                    "prioritize weaker-versus-structured supervision comparisons on the same workload before widening learned capability claims",
                ),
                TassadarLearnabilityGapClass::TraceFormatMiss => String::from(
                    "keep alternate target families explicit and benchmarked so context-fit gains do not get mistaken for full authority traces",
                ),
                TassadarLearnabilityGapClass::ArchitectureCapacityLimit => String::from(
                    "treat remaining fit ceilings as architecture-budget blockers and refuse extrapolation until scaling evidence changes them",
                ),
            },
        })
        .collect::<Vec<_>>();
    Ok(TassadarLearnabilityGapAnalysisReport::new(
        eval_report,
        recommended_actions,
    ))
}

/// Returns the canonical absolute path for the committed report.
#[must_use]
pub fn tassadar_learnability_gap_analysis_report_path() -> PathBuf {
    repo_root().join(TASSADAR_LEARNABILITY_GAP_ANALYSIS_REPORT_REF)
}

/// Writes the committed learnability-gap research summary.
pub fn write_tassadar_learnability_gap_analysis_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarLearnabilityGapAnalysisReport, TassadarLearnabilityGapAnalysisReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarLearnabilityGapAnalysisReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_learnability_gap_analysis_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarLearnabilityGapAnalysisReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
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
        build_tassadar_learnability_gap_analysis_report,
        tassadar_learnability_gap_analysis_report_path,
        write_tassadar_learnability_gap_analysis_report, TassadarLearnabilityGapAnalysisReport,
        TASSADAR_LEARNABILITY_GAP_ANALYSIS_REPORT_REF,
    };

    #[test]
    fn learnability_gap_analysis_report_recommends_actions_for_each_gap_class(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_learnability_gap_analysis_report()?;
        assert_eq!(
            report.recommended_actions.len(),
            report.eval_report.summary_rows.len()
        );
        assert!(report
            .recommended_actions
            .iter()
            .all(|action| !action.action.is_empty()));
        Ok(())
    }

    #[test]
    fn learnability_gap_analysis_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_learnability_gap_analysis_report()?;
        let committed: TassadarLearnabilityGapAnalysisReport =
            super::read_repo_json(TASSADAR_LEARNABILITY_GAP_ANALYSIS_REPORT_REF)?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_learnability_gap_analysis_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        let output_path = temp_dir
            .path()
            .join("tassadar_learnability_gap_analysis_report.json");
        let written = write_tassadar_learnability_gap_analysis_report(&output_path)?;
        let bytes = std::fs::read(&output_path)?;
        let roundtrip: TassadarLearnabilityGapAnalysisReport = serde_json::from_slice(&bytes)?;
        assert_eq!(written, roundtrip);
        assert_eq!(
            tassadar_learnability_gap_analysis_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_learnability_gap_analysis_report.json")
        );
        Ok(())
    }
}
