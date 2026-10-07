use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleTransformerGeneralizationGateReport,
    TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_generalization_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerGeneralizationSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub generalization_gate_report_ref: String,
    pub generalization_gate_report: TassadarArticleTransformerGeneralizationGateReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub case_count: usize,
    pub exact_case_count: usize,
    pub mismatch_case_count: usize,
    pub refused_case_count: usize,
    pub out_of_distribution_case_count: usize,
    pub randomized_case_count: usize,
    pub adversarial_case_count: usize,
    pub curriculum_run_count: usize,
    pub generalization_green: bool,
    pub article_equivalence_green: bool,
    pub mismatch_case_ids: Vec<String>,
    pub refused_case_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleTransformerGeneralizationSummary {
    fn new(generalization_gate_report: TassadarArticleTransformerGeneralizationGateReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_transformer.generalization_gate.summary.v1"),
            generalization_gate_report_ref: String::from(
                TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
            ),
            tied_requirement_id: generalization_gate_report
                .acceptance_gate_tie
                .tied_requirement_id
                .clone(),
            tied_requirement_satisfied: generalization_gate_report
                .acceptance_gate_tie
                .tied_requirement_satisfied,
            acceptance_status: format!(
                "{:?}",
                generalization_gate_report.acceptance_gate_tie.acceptance_status
            )
            .to_lowercase(),
            case_count: generalization_gate_report.case_count,
            exact_case_count: generalization_gate_report.exact_case_count,
            mismatch_case_count: generalization_gate_report.mismatch_case_count,
            refused_case_count: generalization_gate_report.refused_case_count,
            out_of_distribution_case_count: generalization_gate_report
                .out_of_distribution_case_count,
            randomized_case_count: generalization_gate_report.randomized_program_review.case_count,
            adversarial_case_count: generalization_gate_report.adversarial_variant_review.case_count,
            curriculum_run_count: generalization_gate_report.curriculum_order_review.run_count,
            generalization_green: generalization_gate_report.generalization_green,
            article_equivalence_green: generalization_gate_report.article_equivalence_green,
            mismatch_case_ids: generalization_gate_report.mismatch_case_ids.clone(),
            refused_case_ids: generalization_gate_report.refused_case_ids.clone(),
            generalization_gate_report,
            claim_boundary: String::from(
                "this summary mirrors the owned-route generalization and anti-memorization gate only. It keeps the held-out, adversarial, scaling, and mixed-order counts operator-readable without widening the underlying article-equivalence claim boundary.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article Transformer generalization summary now records case_count={}, exact_case_count={}, mismatch_case_count={}, refused_case_count={}, out_of_distribution_case_count={}, randomized_case_count={}, adversarial_case_count={}, curriculum_run_count={}, generalization_green={}, and article_equivalence_green={}.",
            summary.case_count,
            summary.exact_case_count,
            summary.mismatch_case_count,
            summary.refused_case_count,
            summary.out_of_distribution_case_count,
            summary.randomized_case_count,
            summary.adversarial_case_count,
            summary.curriculum_run_count,
            summary.generalization_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_transformer_generalization_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerGeneralizationSummaryError {
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

pub fn build_tassadar_article_transformer_generalization_summary() -> Result<
    TassadarArticleTransformerGeneralizationSummary,
    TassadarArticleTransformerGeneralizationSummaryError,
> {
    let report: TassadarArticleTransformerGeneralizationGateReport = read_repo_json(
        TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_GATE_REPORT_REF,
        "article_transformer_generalization_gate",
    )?;
    Ok(TassadarArticleTransformerGeneralizationSummary::new(
        report,
    ))
}

pub fn tassadar_article_transformer_generalization_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_transformer_generalization_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerGeneralizationSummary,
    TassadarArticleTransformerGeneralizationSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerGeneralizationSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_generalization_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerGeneralizationSummaryError::Write {
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
) -> Result<T, TassadarArticleTransformerGeneralizationSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleTransformerGeneralizationSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerGeneralizationSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_transformer_generalization_summary, read_repo_json,
        tassadar_article_transformer_generalization_summary_path,
        write_tassadar_article_transformer_generalization_summary,
        TassadarArticleTransformerGeneralizationSummary,
        TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_SUMMARY_REPORT_REF,
    };

    #[test]
    fn generalization_summary_tracks_green_gate_without_final_green() {
        let summary = build_tassadar_article_transformer_generalization_summary()
            .expect("generalization summary");

        assert_eq!(summary.case_count, 6);
        assert_eq!(summary.exact_case_count, 6);
        assert_eq!(summary.mismatch_case_count, 0);
        assert_eq!(summary.refused_case_count, 0);
        assert_eq!(summary.out_of_distribution_case_count, 6);
        assert_eq!(summary.randomized_case_count, 2);
        assert_eq!(summary.adversarial_case_count, 2);
        assert_eq!(summary.curriculum_run_count, 2);
        assert!(summary.generalization_green);
        assert!(summary.article_equivalence_green);
        assert!(summary.mismatch_case_ids.is_empty());
        assert!(summary.refused_case_ids.is_empty());
    }

    #[test]
    fn generalization_summary_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_generalization_summary().expect("summary");
        let committed: TassadarArticleTransformerGeneralizationSummary = read_repo_json(
            TASSADAR_ARTICLE_TRANSFORMER_GENERALIZATION_SUMMARY_REPORT_REF,
            "article_transformer_generalization_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_generalization_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_generalization_summary.json");
        let written = write_tassadar_article_transformer_generalization_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleTransformerGeneralizationSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read"))
                .expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_generalization_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_generalization_summary.json")
        );
    }
}
