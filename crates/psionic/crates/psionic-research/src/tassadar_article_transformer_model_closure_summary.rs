use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleTransformerModelClosureReport,
    TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_model_closure_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerModelClosureSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleTransformerModelClosureReport,
    pub case_count: usize,
    pub passed_case_count: usize,
    pub boundary_review_passed: bool,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub article_transformer_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleTransformerModelClosureSummary {
    fn new(report: TassadarArticleTransformerModelClosureReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_transformer_model_closure.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF),
            case_count: report.case_rows.len(),
            passed_case_count: report.case_rows.iter().filter(|row| row.passed).count(),
            boundary_review_passed: report.boundary_review.passed,
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            acceptance_status: format!("{:?}", report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            article_transformer_contract_green: report.article_transformer_contract_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors the paper-faithful article-Transformer model closure only. It keeps the canonical stack and wrapper facts operator-readable without widening the current article-equivalence claim boundary.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article Transformer model summary now records case_count={}, passed_case_count={}, boundary_review_passed={}, tied_requirement_satisfied={}, article_transformer_contract_green={}, and article_equivalence_green={}.",
            summary.case_count,
            summary.passed_case_count,
            summary.boundary_review_passed,
            summary.tied_requirement_satisfied,
            summary.article_transformer_contract_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_transformer_model_closure_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerModelClosureSummaryError {
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

pub fn build_tassadar_article_transformer_model_closure_summary(
) -> Result<
    TassadarArticleTransformerModelClosureSummary,
    TassadarArticleTransformerModelClosureSummaryError,
> {
    let report: TassadarArticleTransformerModelClosureReport = read_repo_json(
        TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF,
        "article_transformer_model_closure",
    )?;
    Ok(TassadarArticleTransformerModelClosureSummary::new(report))
}

pub fn tassadar_article_transformer_model_closure_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_transformer_model_closure_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerModelClosureSummary,
    TassadarArticleTransformerModelClosureSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerModelClosureSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_model_closure_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerModelClosureSummaryError::Write {
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
) -> Result<T, TassadarArticleTransformerModelClosureSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleTransformerModelClosureSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerModelClosureSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_transformer_model_closure_summary, read_repo_json,
        tassadar_article_transformer_model_closure_summary_path,
        write_tassadar_article_transformer_model_closure_summary,
        TassadarArticleTransformerModelClosureSummary,
        TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_SUMMARY_REPORT_REF,
    };

    #[test]
    fn article_transformer_model_closure_summary_tracks_gate_tie_without_final_green() {
        let report =
            build_tassadar_article_transformer_model_closure_summary().expect("summary");

        assert!(report.tied_requirement_satisfied);
        assert_eq!(report.acceptance_status, "green");
        assert!(report.boundary_review_passed);
        assert!(report.article_transformer_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.case_count, 6);
        assert_eq!(report.passed_case_count, 6);
    }

    #[test]
    fn article_transformer_model_closure_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_model_closure_summary().expect("summary");
        let committed: TassadarArticleTransformerModelClosureSummary = read_repo_json(
            TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_SUMMARY_REPORT_REF,
            "article_transformer_model_closure_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_transformer_model_closure_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_model_closure_summary.json");
        let written = write_tassadar_article_transformer_model_closure_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleTransformerModelClosureSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_model_closure_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_model_closure_summary.json")
        );
    }
}
