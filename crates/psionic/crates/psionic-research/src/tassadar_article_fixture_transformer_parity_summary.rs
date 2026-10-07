use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleFixtureTransformerParityReport,
    TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fixture_transformer_parity_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFixtureTransformerParitySummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleFixtureTransformerParityReport,
    pub case_count: usize,
    pub routeable_case_count: usize,
    pub exact_trace_case_count: usize,
    pub exact_output_case_count: usize,
    pub context_window_fit_case_count: usize,
    pub forward_binding_case_count: usize,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub replacement_certified: bool,
    pub replacement_publication_allowed: bool,
    pub article_equivalence_green: bool,
    pub mismatch_case_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleFixtureTransformerParitySummary {
    fn new(report: TassadarArticleFixtureTransformerParityReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_fixture_transformer_parity.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF),
            case_count: report.supported_case_count,
            routeable_case_count: report.routeable_case_count,
            exact_trace_case_count: report.exact_trace_case_count,
            exact_output_case_count: report.exact_output_case_count,
            context_window_fit_case_count: report.context_window_fit_case_count,
            forward_binding_case_count: report.forward_binding_case_count,
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            acceptance_status: format!("{:?}", report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            replacement_certified: report.replacement_certified,
            replacement_publication_allowed: report.replacement_publication_allowed,
            article_equivalence_green: report.article_equivalence_green,
            mismatch_case_ids: report.mismatch_case_ids.clone(),
            report,
            claim_boundary: String::from(
                "this summary mirrors only the bounded fixture-to-Transformer parity certificate. It keeps the replacement counts operator-readable without widening the claim beyond the underlying parity report.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article fixture-to-Transformer parity summary now records case_count={}, routeable_case_count={}, exact_trace_case_count={}, exact_output_case_count={}, context_window_fit_case_count={}, forward_binding_case_count={}, replacement_certified={}, and article_equivalence_green={}.",
            summary.case_count,
            summary.routeable_case_count,
            summary.exact_trace_case_count,
            summary.exact_output_case_count,
            summary.context_window_fit_case_count,
            summary.forward_binding_case_count,
            summary.replacement_certified,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_fixture_transformer_parity_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleFixtureTransformerParitySummaryError {
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

pub fn build_tassadar_article_fixture_transformer_parity_summary() -> Result<
    TassadarArticleFixtureTransformerParitySummary,
    TassadarArticleFixtureTransformerParitySummaryError,
> {
    let report: TassadarArticleFixtureTransformerParityReport = read_repo_json(
        TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF,
        "article_fixture_transformer_parity_report",
    )?;
    Ok(TassadarArticleFixtureTransformerParitySummary::new(report))
}

pub fn tassadar_article_fixture_transformer_parity_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_fixture_transformer_parity_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFixtureTransformerParitySummary,
    TassadarArticleFixtureTransformerParitySummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFixtureTransformerParitySummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_fixture_transformer_parity_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFixtureTransformerParitySummaryError::Write {
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
) -> Result<T, TassadarArticleFixtureTransformerParitySummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFixtureTransformerParitySummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFixtureTransformerParitySummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_fixture_transformer_parity_summary, read_repo_json,
        tassadar_article_fixture_transformer_parity_summary_path,
        write_tassadar_article_fixture_transformer_parity_summary,
        TassadarArticleFixtureTransformerParitySummary,
        TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_SUMMARY_REPORT_REF,
    };

    #[test]
    fn article_fixture_transformer_parity_summary_tracks_replacement_without_final_green() {
        let report = build_tassadar_article_fixture_transformer_parity_summary().expect("summary");

        assert!(report.tied_requirement_satisfied);
        assert_eq!(report.acceptance_status, "green");
        assert!(report.replacement_certified);
        assert!(report.replacement_publication_allowed);
        assert!(report.article_equivalence_green);
        assert_eq!(report.case_count, 13);
        assert_eq!(report.routeable_case_count, 13);
        assert_eq!(report.exact_trace_case_count, 13);
        assert_eq!(report.exact_output_case_count, 13);
        assert_eq!(report.context_window_fit_case_count, 4);
        assert_eq!(report.forward_binding_case_count, 4);
        assert!(report.mismatch_case_ids.is_empty());
    }

    #[test]
    fn article_fixture_transformer_parity_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_fixture_transformer_parity_summary().expect("summary");
        let committed: TassadarArticleFixtureTransformerParitySummary = read_repo_json(
            TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_SUMMARY_REPORT_REF,
            "article_fixture_transformer_parity_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_fixture_transformer_parity_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_fixture_transformer_parity_summary.json");
        let written = write_tassadar_article_fixture_transformer_parity_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleFixtureTransformerParitySummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_fixture_transformer_parity_summary_path(),
            super::repo_root().join(TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_SUMMARY_REPORT_REF)
        );
    }
}
