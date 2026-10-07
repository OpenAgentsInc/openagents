use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleSingleRunNoSpillClosureReport,
    TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_single_run_no_spill_closure_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleSingleRunNoSpillClosureSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleSingleRunNoSpillClosureReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub blocked_issue_frontier: Option<String>,
    pub deterministic_exactness_green: bool,
    pub step_consistency_green: bool,
    pub context_sensitivity_green: bool,
    pub perturbation_negative_control_green: bool,
    pub stochastic_mode_robustness_green: bool,
    pub single_run_no_spill_closure_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleSingleRunNoSpillClosureSummary {
    fn new(report: TassadarArticleSingleRunNoSpillClosureReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_single_run_no_spill_closure.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            blocked_issue_frontier: report.acceptance_gate_tie.blocked_issue_ids.first().cloned(),
            deterministic_exactness_green: report.horizon_review.deterministic_exactness_green,
            step_consistency_green: report.step_consistency_review.consistency_green,
            context_sensitivity_green: report.context_sensitivity_review.context_sensitivity_green,
            perturbation_negative_control_green: report
                .boundary_perturbation_review
                .perturbation_negative_control_green,
            stochastic_mode_robustness_green: report
                .stochastic_mode_review
                .stochastic_mode_robustness_green,
            single_run_no_spill_closure_green: report.single_run_no_spill_closure_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-183 single-run no-spill closure gate. It keeps the no-resume operator envelope, long-horizon exactness, context-sensitivity bound, and continuation negative controls operator-readable without pretending that clean-room weight causality, route minimality, or final article-equivalence green status are already true.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article single-run no-spill closure summary now records tied_requirement_satisfied={}, deterministic_exactness_green={}, step_consistency_green={}, context_sensitivity_green={}, perturbation_negative_control_green={}, stochastic_mode_robustness_green={}, gate_green={}, blocked_issue_frontier={:?}, and article_equivalence_green={}.",
            summary.tied_requirement_satisfied,
            summary.deterministic_exactness_green,
            summary.step_consistency_green,
            summary.context_sensitivity_green,
            summary.perturbation_negative_control_green,
            summary.stochastic_mode_robustness_green,
            summary.single_run_no_spill_closure_green,
            summary.blocked_issue_frontier,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_single_run_no_spill_closure_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleSingleRunNoSpillClosureSummaryError {
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

pub fn build_tassadar_article_single_run_no_spill_closure_summary() -> Result<
    TassadarArticleSingleRunNoSpillClosureSummary,
    TassadarArticleSingleRunNoSpillClosureSummaryError,
> {
    let report: TassadarArticleSingleRunNoSpillClosureReport = read_repo_json(
        TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
        "article_single_run_no_spill_closure_report",
    )?;
    Ok(TassadarArticleSingleRunNoSpillClosureSummary::new(report))
}

pub fn tassadar_article_single_run_no_spill_closure_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_single_run_no_spill_closure_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleSingleRunNoSpillClosureSummary,
    TassadarArticleSingleRunNoSpillClosureSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleSingleRunNoSpillClosureSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_single_run_no_spill_closure_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleSingleRunNoSpillClosureSummaryError::Write {
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
) -> Result<T, TassadarArticleSingleRunNoSpillClosureSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleSingleRunNoSpillClosureSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleSingleRunNoSpillClosureSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_single_run_no_spill_closure_summary, read_repo_json,
        tassadar_article_single_run_no_spill_closure_summary_path,
        write_tassadar_article_single_run_no_spill_closure_summary,
        TassadarArticleSingleRunNoSpillClosureSummary,
        TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_SUMMARY_REPORT_REF,
    };

    #[test]
    fn article_single_run_no_spill_closure_summary_tracks_green_gate() {
        let summary =
            build_tassadar_article_single_run_no_spill_closure_summary().expect("summary");

        assert!(summary.tied_requirement_satisfied);
        assert!(summary.deterministic_exactness_green);
        assert!(summary.step_consistency_green);
        assert!(summary.context_sensitivity_green);
        assert!(summary.perturbation_negative_control_green);
        assert!(summary.stochastic_mode_robustness_green);
        assert!(summary.single_run_no_spill_closure_green);
        assert_eq!(summary.blocked_issue_frontier, None);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn article_single_run_no_spill_closure_summary_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_single_run_no_spill_closure_summary()?;
        let committed: TassadarArticleSingleRunNoSpillClosureSummary = read_repo_json(
            TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_SUMMARY_REPORT_REF,
            "article_single_run_no_spill_closure_summary",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_single_run_no_spill_closure_summary_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_single_run_no_spill_closure_summary.json");
        let written = write_tassadar_article_single_run_no_spill_closure_summary(&output_path)?;
        let persisted: TassadarArticleSingleRunNoSpillClosureSummary =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_single_run_no_spill_closure_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_single_run_no_spill_closure_summary.json")
        );
        Ok(())
    }
}
