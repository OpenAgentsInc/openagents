use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleInterpreterOwnershipGateReport, TassadarArticleInterpreterOwnershipLocalityKind,
    TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_interpreter_ownership_gate_summary.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterOwnershipGateSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleInterpreterOwnershipGateReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub blocked_issue_frontier: Option<String>,
    pub generic_direct_proof_suite_green: bool,
    pub generic_direct_proof_case_count: usize,
    pub breadth_conformance_matrix_green: bool,
    pub green_family_count: usize,
    pub route_purity_green: bool,
    pub mapping_stable_across_runs: bool,
    pub perturbation_sensitivity_green: bool,
    pub locality_characterization: TassadarArticleInterpreterOwnershipLocalityKind,
    pub interpreter_ownership_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleInterpreterOwnershipGateSummary {
    fn new(report: TassadarArticleInterpreterOwnershipGateReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_interpreter_ownership_gate.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            blocked_issue_frontier: report.acceptance_gate_tie.blocked_issue_ids.first().cloned(),
            generic_direct_proof_suite_green: report
                .generic_direct_proof_review
                .generic_direct_proof_suite_green,
            generic_direct_proof_case_count: report.generic_direct_proof_review.case_rows.len(),
            breadth_conformance_matrix_green: report
                .breadth_conformance_matrix
                .conformance_matrix_green,
            green_family_count: report.breadth_conformance_matrix.green_family_count,
            route_purity_green: report.route_purity_review.route_purity_green,
            mapping_stable_across_runs: report.computation_mapping_report.stable_across_runs,
            perturbation_sensitivity_green: report
                .weight_perturbation_review
                .all_interventions_show_sensitivity,
            locality_characterization: report
                .weight_perturbation_review
                .locality_characterization,
            interpreter_ownership_green: report.interpreter_ownership_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-184 interpreter-ownership gate. It keeps the widened generic direct-proof suite, breadth conformance, route-purity audit, computation mapping, and weight-perturbation sensitivity operator-readable without pretending that the later KV-cache and activation-state discipline verdict, cross-machine reproducibility, route minimality, or final article-equivalence green status are already true.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article interpreter-ownership gate summary now records tied_requirement_satisfied={}, generic_direct_proof_suite_green={}, generic_direct_proof_case_count={}, breadth_conformance_matrix_green={}, green_family_count={}, route_purity_green={}, mapping_stable_across_runs={}, perturbation_sensitivity_green={}, locality_characterization={:?}, blocked_issue_frontier={:?}, ownership_gate_green={}, and article_equivalence_green={}.",
            summary.tied_requirement_satisfied,
            summary.generic_direct_proof_suite_green,
            summary.generic_direct_proof_case_count,
            summary.breadth_conformance_matrix_green,
            summary.green_family_count,
            summary.route_purity_green,
            summary.mapping_stable_across_runs,
            summary.perturbation_sensitivity_green,
            summary.locality_characterization,
            summary.blocked_issue_frontier,
            summary.interpreter_ownership_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_interpreter_ownership_gate_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleInterpreterOwnershipGateSummaryError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_interpreter_ownership_gate_summary() -> Result<
    TassadarArticleInterpreterOwnershipGateSummary,
    TassadarArticleInterpreterOwnershipGateSummaryError,
> {
    let report: TassadarArticleInterpreterOwnershipGateReport = read_repo_json(
        TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
        "article_interpreter_ownership_gate_report",
    )?;
    Ok(TassadarArticleInterpreterOwnershipGateSummary::new(report))
}

pub fn tassadar_article_interpreter_ownership_gate_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_interpreter_ownership_gate_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleInterpreterOwnershipGateSummary,
    TassadarArticleInterpreterOwnershipGateSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleInterpreterOwnershipGateSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_interpreter_ownership_gate_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(summary)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-research should live under <repo>/crates/psionic-research")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleInterpreterOwnershipGateSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleInterpreterOwnershipGateSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_interpreter_ownership_gate_summary, read_repo_json,
        tassadar_article_interpreter_ownership_gate_summary_path,
        write_tassadar_article_interpreter_ownership_gate_summary,
        TassadarArticleInterpreterOwnershipGateSummary,
        TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_SUMMARY_REPORT_REF,
    };

    #[test]
    fn article_interpreter_ownership_gate_summary_tracks_green_gate() {
        let summary = build_tassadar_article_interpreter_ownership_gate_summary().expect("summary");

        assert!(summary.tied_requirement_satisfied);
        assert!(summary.generic_direct_proof_suite_green);
        assert_eq!(summary.generic_direct_proof_case_count, 6);
        assert!(summary.breadth_conformance_matrix_green);
        assert_eq!(summary.green_family_count, 8);
        assert!(summary.route_purity_green);
        assert!(summary.mapping_stable_across_runs);
        assert!(summary.perturbation_sensitivity_green);
        assert_eq!(summary.blocked_issue_frontier, None);
        assert!(summary.interpreter_ownership_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn article_interpreter_ownership_gate_summary_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_interpreter_ownership_gate_summary()?;
        let committed: TassadarArticleInterpreterOwnershipGateSummary = read_repo_json(
            TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_SUMMARY_REPORT_REF,
            "article_interpreter_ownership_gate_summary",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_interpreter_ownership_gate_summary_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_interpreter_ownership_gate_summary.json");
        let written = write_tassadar_article_interpreter_ownership_gate_summary(&output_path)?;
        let persisted: TassadarArticleInterpreterOwnershipGateSummary =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_interpreter_ownership_gate_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_interpreter_ownership_gate_summary.json")
        );
        Ok(())
    }
}
