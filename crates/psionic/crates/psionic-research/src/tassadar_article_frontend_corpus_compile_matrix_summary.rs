use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF,
    TassadarArticleFrontendCorpusCompileMatrixReport,
};

pub const TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_frontend_corpus_compile_matrix_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCorpusCompileMatrixSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleFrontendCorpusCompileMatrixReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub compiled_case_count: usize,
    pub typed_refusal_case_count: usize,
    pub toolchain_failure_case_count: usize,
    pub lineage_green_count: usize,
    pub refusal_green_count: usize,
    pub toolchain_failure_green_count: usize,
    pub category_coverage_green: bool,
    pub envelope_alignment_green: bool,
    pub compile_matrix_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleFrontendCorpusCompileMatrixSummary {
    fn new(report: TassadarArticleFrontendCorpusCompileMatrixReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_frontend_corpus_compile_matrix.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            compiled_case_count: report.compiled_case_count,
            typed_refusal_case_count: report.typed_refusal_case_count,
            toolchain_failure_case_count: report.toolchain_failure_case_count,
            lineage_green_count: report.lineage_green_count,
            refusal_green_count: report.refusal_green_count,
            toolchain_failure_green_count: report.toolchain_failure_green_count,
            category_coverage_green: report.category_coverage_green,
            envelope_alignment_green: report.envelope_alignment_green,
            compile_matrix_green: report.compile_matrix_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the TAS-177 frontend corpus and compile-matrix tranche. It keeps the widened category coverage, typed refusal coverage, and toolchain-failure posture operator-readable without pretending that later article-demo parity or final article-equivalence closure are already green.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article frontend corpus compile matrix summary now records tied_requirement_satisfied={}, compiled_cases={}, typed_refusals={}, toolchain_failures={}, category_coverage_green={}, envelope_alignment_green={}, compile_matrix_green={}, and article_equivalence_green={}.",
            summary.tied_requirement_satisfied,
            summary.compiled_case_count,
            summary.typed_refusal_case_count,
            summary.toolchain_failure_case_count,
            summary.category_coverage_green,
            summary.envelope_alignment_green,
            summary.compile_matrix_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_frontend_corpus_compile_matrix_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleFrontendCorpusCompileMatrixSummaryError {
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

pub fn build_tassadar_article_frontend_corpus_compile_matrix_summary() -> Result<
    TassadarArticleFrontendCorpusCompileMatrixSummary,
    TassadarArticleFrontendCorpusCompileMatrixSummaryError,
> {
    let report: TassadarArticleFrontendCorpusCompileMatrixReport = read_repo_json(
        TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF,
        "article_frontend_corpus_compile_matrix_report",
    )?;
    Ok(TassadarArticleFrontendCorpusCompileMatrixSummary::new(report))
}

pub fn tassadar_article_frontend_corpus_compile_matrix_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_frontend_corpus_compile_matrix_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFrontendCorpusCompileMatrixSummary,
    TassadarArticleFrontendCorpusCompileMatrixSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFrontendCorpusCompileMatrixSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_article_frontend_corpus_compile_matrix_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFrontendCorpusCompileMatrixSummaryError::Write {
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
) -> Result<T, TassadarArticleFrontendCorpusCompileMatrixSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFrontendCorpusCompileMatrixSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFrontendCorpusCompileMatrixSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_SUMMARY_REPORT_REF,
        TassadarArticleFrontendCorpusCompileMatrixSummary,
        build_tassadar_article_frontend_corpus_compile_matrix_summary, read_repo_json,
        tassadar_article_frontend_corpus_compile_matrix_summary_path,
        write_tassadar_article_frontend_corpus_compile_matrix_summary,
    };

    #[test]
    fn frontend_corpus_compile_matrix_summary_tracks_broad_frontend_green_without_final_green() {
        let summary =
            build_tassadar_article_frontend_corpus_compile_matrix_summary().expect("summary");

        assert_eq!(summary.tied_requirement_id, "TAS-177");
        assert!(summary.tied_requirement_satisfied);
        assert_eq!(summary.compiled_case_count, 11);
        assert_eq!(summary.typed_refusal_case_count, 4);
        assert_eq!(summary.toolchain_failure_case_count, 1);
        assert_eq!(summary.lineage_green_count, 11);
        assert_eq!(summary.refusal_green_count, 4);
        assert_eq!(summary.toolchain_failure_green_count, 1);
        assert!(summary.category_coverage_green);
        assert!(summary.envelope_alignment_green);
        assert!(summary.compile_matrix_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn frontend_corpus_compile_matrix_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_frontend_corpus_compile_matrix_summary().expect("summary");
        let committed: TassadarArticleFrontendCorpusCompileMatrixSummary = read_repo_json(
            TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_SUMMARY_REPORT_REF,
            "summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_frontend_corpus_compile_matrix_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_frontend_corpus_compile_matrix_summary.json");
        let written = write_tassadar_article_frontend_corpus_compile_matrix_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleFrontendCorpusCompileMatrixSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_frontend_corpus_compile_matrix_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_frontend_corpus_compile_matrix_summary.json")
        );
    }
}
