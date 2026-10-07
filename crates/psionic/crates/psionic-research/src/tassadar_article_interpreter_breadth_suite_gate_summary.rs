use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleInterpreterBreadthSuiteGateReport,
    TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_interpreter_breadth_suite_gate_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthSuiteGateSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleInterpreterBreadthSuiteGateReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub blocked_issue_frontier: Option<String>,
    pub green_family_count: usize,
    pub required_family_count: usize,
    pub breadth_gate_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleInterpreterBreadthSuiteGateSummary {
    fn new(report: TassadarArticleInterpreterBreadthSuiteGateReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_interpreter_breadth_suite_gate.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            blocked_issue_frontier: report.acceptance_gate_tie.blocked_issue_ids.first().cloned(),
            green_family_count: report.green_family_count,
            required_family_count: report.suite_manifest.required_family_ids.len(),
            breadth_gate_green: report.breadth_gate_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-179A article interpreter breadth suite gate. It keeps the generic family coverage operator-readable while leaving benchmark-wide, single-run, and final article-equivalence closure explicitly open.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article interpreter breadth suite gate summary now records tied_requirement_satisfied={}, green_families={}/{}, breadth_gate_green={}, blocked_issue_frontier={:?}, and article_equivalence_green={}.",
            summary.tied_requirement_satisfied,
            summary.green_family_count,
            summary.required_family_count,
            summary.breadth_gate_green,
            summary.blocked_issue_frontier,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_interpreter_breadth_suite_gate_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleInterpreterBreadthSuiteGateSummaryError {
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

pub fn build_tassadar_article_interpreter_breadth_suite_gate_summary() -> Result<
    TassadarArticleInterpreterBreadthSuiteGateSummary,
    TassadarArticleInterpreterBreadthSuiteGateSummaryError,
> {
    let report: TassadarArticleInterpreterBreadthSuiteGateReport = read_repo_json(
        TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF,
        "article_interpreter_breadth_suite_gate_report",
    )?;
    Ok(TassadarArticleInterpreterBreadthSuiteGateSummary::new(
        report,
    ))
}

pub fn tassadar_article_interpreter_breadth_suite_gate_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_interpreter_breadth_suite_gate_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleInterpreterBreadthSuiteGateSummary,
    TassadarArticleInterpreterBreadthSuiteGateSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleInterpreterBreadthSuiteGateSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_interpreter_breadth_suite_gate_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleInterpreterBreadthSuiteGateSummaryError::Write {
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
) -> Result<T, TassadarArticleInterpreterBreadthSuiteGateSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleInterpreterBreadthSuiteGateSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleInterpreterBreadthSuiteGateSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_interpreter_breadth_suite_gate_summary, read_repo_json,
        tassadar_article_interpreter_breadth_suite_gate_summary_path,
        write_tassadar_article_interpreter_breadth_suite_gate_summary,
        TassadarArticleInterpreterBreadthSuiteGateSummary,
        TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_SUMMARY_REPORT_REF,
    };

    #[test]
    fn article_interpreter_breadth_suite_gate_summary_tracks_green_suite_gate() {
        let summary =
            build_tassadar_article_interpreter_breadth_suite_gate_summary().expect("summary");

        assert!(summary.tied_requirement_satisfied);
        assert_eq!(summary.blocked_issue_frontier, None);
        assert_eq!(summary.green_family_count, 8);
        assert_eq!(summary.required_family_count, 8);
        assert!(summary.breadth_gate_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn article_interpreter_breadth_suite_gate_summary_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated =
            build_tassadar_article_interpreter_breadth_suite_gate_summary().expect("summary");
        let committed: TassadarArticleInterpreterBreadthSuiteGateSummary = read_repo_json(
            TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_SUMMARY_REPORT_REF,
            "article_interpreter_breadth_suite_gate_summary",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_interpreter_breadth_suite_gate_summary_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_interpreter_breadth_suite_gate_summary.json");
        let written = write_tassadar_article_interpreter_breadth_suite_gate_summary(&output_path)?;
        let persisted: TassadarArticleInterpreterBreadthSuiteGateSummary =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_interpreter_breadth_suite_gate_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_interpreter_breadth_suite_gate_summary.json")
        );
        Ok(())
    }
}
