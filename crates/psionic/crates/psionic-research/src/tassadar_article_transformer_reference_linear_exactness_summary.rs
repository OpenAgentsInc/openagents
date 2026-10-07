use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleTransformerReferenceLinearExactnessGateReport,
    TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_reference_linear_exactness_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerReferenceLinearExactnessSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleTransformerReferenceLinearExactnessGateReport,
    pub case_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub refused_case_count: usize,
    pub within_transformer_context_window_case_count: usize,
    pub direct_model_weight_proof_case_count: usize,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub reference_linear_exactness_green: bool,
    pub article_equivalence_green: bool,
    pub mismatch_case_ids: Vec<String>,
    pub refused_case_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleTransformerReferenceLinearExactnessSummary {
    fn new(report: TassadarArticleTransformerReferenceLinearExactnessGateReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from(
                "tassadar.article_transformer.reference_linear_exactness.summary.v1",
            ),
            report_ref: String::from(
                TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF,
            ),
            case_count: report.declared_case_count,
            exact_case_count: report.exact_case_count,
            mismatch_case_count: report.mismatch_case_count,
            refused_case_count: report.refused_case_count,
            within_transformer_context_window_case_count: report
                .within_transformer_context_window_case_count,
            direct_model_weight_proof_case_count: report.direct_model_weight_proof_case_count,
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            acceptance_status: format!("{:?}", report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            reference_linear_exactness_green: report.reference_linear_exactness_green,
            article_equivalence_green: report.article_equivalence_green,
            mismatch_case_ids: report.mismatch_case_ids.clone(),
            refused_case_ids: report.refused_case_ids.clone(),
            report,
            claim_boundary: String::from(
                "this summary mirrors only the bounded Transformer-backed reference-linear exactness gate. It keeps the full-family exactness counts operator-readable without widening the claim beyond the underlying gate report.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Transformer-backed reference-linear exactness summary now records case_count={}, exact_case_count={}, mismatch_case_count={}, refused_case_count={}, direct_model_weight_proof_case_count={}, reference_linear_exactness_green={}, and article_equivalence_green={}.",
            summary.case_count,
            summary.exact_case_count,
            summary.mismatch_case_count,
            summary.refused_case_count,
            summary.direct_model_weight_proof_case_count,
            summary.reference_linear_exactness_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_transformer_reference_linear_exactness_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerReferenceLinearExactnessSummaryError {
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

pub fn build_tassadar_article_transformer_reference_linear_exactness_summary() -> Result<
    TassadarArticleTransformerReferenceLinearExactnessSummary,
    TassadarArticleTransformerReferenceLinearExactnessSummaryError,
> {
    let report: TassadarArticleTransformerReferenceLinearExactnessGateReport = read_repo_json(
        TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_GATE_REPORT_REF,
        "article_transformer_reference_linear_exactness_gate_report",
    )?;
    Ok(TassadarArticleTransformerReferenceLinearExactnessSummary::new(report))
}

pub fn tassadar_article_transformer_reference_linear_exactness_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_transformer_reference_linear_exactness_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerReferenceLinearExactnessSummary,
    TassadarArticleTransformerReferenceLinearExactnessSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerReferenceLinearExactnessSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_reference_linear_exactness_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerReferenceLinearExactnessSummaryError::Write {
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
) -> Result<T, TassadarArticleTransformerReferenceLinearExactnessSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleTransformerReferenceLinearExactnessSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerReferenceLinearExactnessSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_transformer_reference_linear_exactness_summary, read_repo_json,
        tassadar_article_transformer_reference_linear_exactness_summary_path,
        write_tassadar_article_transformer_reference_linear_exactness_summary,
        TassadarArticleTransformerReferenceLinearExactnessSummary,
        TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_SUMMARY_REPORT_REF,
    };

    #[test]
    fn reference_linear_exactness_summary_tracks_green_exactness_without_final_green() {
        let report = build_tassadar_article_transformer_reference_linear_exactness_summary()
            .expect("summary");

        assert_eq!(report.case_count, 13);
        assert_eq!(report.exact_case_count, 13);
        assert_eq!(report.mismatch_case_count, 0);
        assert_eq!(report.refused_case_count, 0);
        assert_eq!(report.direct_model_weight_proof_case_count, 3);
        assert!(report.tied_requirement_satisfied);
        assert_eq!(report.acceptance_status, "green");
        assert!(report.reference_linear_exactness_green);
        assert!(report.article_equivalence_green);
        assert!(report.mismatch_case_ids.is_empty());
        assert!(report.refused_case_ids.is_empty());
    }

    #[test]
    fn reference_linear_exactness_summary_matches_committed_truth() {
        let generated = build_tassadar_article_transformer_reference_linear_exactness_summary()
            .expect("summary");
        let committed: TassadarArticleTransformerReferenceLinearExactnessSummary = read_repo_json(
            TASSADAR_ARTICLE_TRANSFORMER_REFERENCE_LINEAR_EXACTNESS_SUMMARY_REPORT_REF,
            "article_transformer_reference_linear_exactness_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_reference_linear_exactness_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_reference_linear_exactness_summary.json");
        let written =
            write_tassadar_article_transformer_reference_linear_exactness_summary(&output_path)
                .expect("write summary");
        let persisted: TassadarArticleTransformerReferenceLinearExactnessSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_reference_linear_exactness_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_reference_linear_exactness_summary.json")
        );
    }
}
