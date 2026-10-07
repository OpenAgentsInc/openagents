use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleEquivalenceBlockerMatrixReport,
    TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
};

pub const TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_equivalence_blocker_matrix_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEquivalenceBlockerMatrixSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub blocker_matrix_report_ref: String,
    pub blocker_matrix_report: TassadarArticleEquivalenceBlockerMatrixReport,
    pub blocker_count: usize,
    pub open_blocker_count: usize,
    pub open_blocker_ids: Vec<String>,
    pub required_later_issue_count: usize,
    pub matrix_contract_green: bool,
    pub article_equivalence_green: bool,
    pub current_truth_boundary: String,
    pub non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleEquivalenceBlockerMatrixSummary {
    fn new(blocker_matrix_report: TassadarArticleEquivalenceBlockerMatrixReport) -> Self {
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_equivalence.blocker_matrix.summary.v1"),
            blocker_matrix_report_ref: String::from(
                TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
            ),
            blocker_count: blocker_matrix_report.blocker_count,
            open_blocker_count: blocker_matrix_report.open_blocker_count,
            open_blocker_ids: blocker_matrix_report.open_blocker_ids.clone(),
            required_later_issue_count: blocker_matrix_report.required_later_issue_count,
            matrix_contract_green: blocker_matrix_report.matrix_contract_green,
            article_equivalence_green: blocker_matrix_report.article_equivalence_green,
            current_truth_boundary: blocker_matrix_report.current_truth_boundary.clone(),
            non_implications: blocker_matrix_report.non_implications.clone(),
            blocker_matrix_report,
            claim_boundary: String::from(
                "this summary mirrors the TAS-157 blocker matrix only. It keeps the blocker contract operator-readable, but it does not add evidence or widen the current public claim boundary beyond the underlying matrix",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Article-equivalence blocker matrix summary now records blocker_count={}, open_blocker_count={}, matrix_contract_green={}, and article_equivalence_green={}.",
            report.blocker_count,
            report.open_blocker_count,
            report.matrix_contract_green,
            report.article_equivalence_green,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_article_equivalence_blocker_matrix_summary|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleEquivalenceBlockerMatrixSummaryError {
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

pub fn build_tassadar_article_equivalence_blocker_matrix_summary() -> Result<
    TassadarArticleEquivalenceBlockerMatrixSummary,
    TassadarArticleEquivalenceBlockerMatrixSummaryError,
> {
    let blocker_matrix_report: TassadarArticleEquivalenceBlockerMatrixReport = read_repo_json(
        TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_REPORT_REF,
        "article_equivalence_blocker_matrix",
    )?;
    Ok(TassadarArticleEquivalenceBlockerMatrixSummary::new(
        blocker_matrix_report,
    ))
}

pub fn tassadar_article_equivalence_blocker_matrix_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_equivalence_blocker_matrix_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleEquivalenceBlockerMatrixSummary,
    TassadarArticleEquivalenceBlockerMatrixSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleEquivalenceBlockerMatrixSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_equivalence_blocker_matrix_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleEquivalenceBlockerMatrixSummaryError::Write {
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
) -> Result<T, TassadarArticleEquivalenceBlockerMatrixSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleEquivalenceBlockerMatrixSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleEquivalenceBlockerMatrixSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_equivalence_blocker_matrix_summary, read_repo_json,
        tassadar_article_equivalence_blocker_matrix_summary_path,
        write_tassadar_article_equivalence_blocker_matrix_summary,
        TassadarArticleEquivalenceBlockerMatrixSummary,
        TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_SUMMARY_REPORT_REF,
    };

    #[test]
    fn article_equivalence_blocker_matrix_summary_is_green_only_as_a_contract() {
        let report = build_tassadar_article_equivalence_blocker_matrix_summary().expect("summary");

        assert!(report.matrix_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.blocker_count, 7);
        assert_eq!(report.open_blocker_count, 0);
    }

    #[test]
    fn article_equivalence_blocker_matrix_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_equivalence_blocker_matrix_summary().expect("summary");
        let committed: TassadarArticleEquivalenceBlockerMatrixSummary = read_repo_json(
            TASSADAR_ARTICLE_EQUIVALENCE_BLOCKER_MATRIX_SUMMARY_REPORT_REF,
            "article_equivalence_blocker_matrix_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_equivalence_blocker_matrix_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_equivalence_blocker_matrix_summary.json");
        let written = write_tassadar_article_equivalence_blocker_matrix_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleEquivalenceBlockerMatrixSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_equivalence_blocker_matrix_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_equivalence_blocker_matrix_summary.json")
        );
    }
}
