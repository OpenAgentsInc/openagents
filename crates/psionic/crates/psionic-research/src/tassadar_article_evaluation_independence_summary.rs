use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    TassadarArticleEvaluationIndependenceAuditReport,
    TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_REPORT_REF,
};

pub const TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_evaluation_independence_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleEvaluationIndependenceSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub report_ref: String,
    pub report: TassadarArticleEvaluationIndependenceAuditReport,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: String,
    pub training_case_count: usize,
    pub evaluation_case_count: usize,
    pub exact_case_id_overlap_count: usize,
    pub exact_source_token_overlap_count: usize,
    pub exact_target_token_overlap_count: usize,
    pub exact_sequence_overlap_count: usize,
    pub near_duplicate_pair_count: usize,
    pub shared_generator_id_count: usize,
    pub shared_generator_rule_digest_count: usize,
    pub shared_profile_id_count: usize,
    pub evaluation_independence_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleEvaluationIndependenceSummary {
    fn new(report: TassadarArticleEvaluationIndependenceAuditReport) -> Self {
        let mut summary = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_evaluation_independence.summary.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_REPORT_REF),
            tied_requirement_id: report.acceptance_gate_tie.tied_requirement_id.clone(),
            tied_requirement_satisfied: report.acceptance_gate_tie.tied_requirement_satisfied,
            acceptance_status: format!("{:?}", report.acceptance_gate_tie.acceptance_status)
                .to_lowercase(),
            training_case_count: report.training_case_rows.len(),
            evaluation_case_count: report.evaluation_case_rows.len(),
            exact_case_id_overlap_count: report.exclusion_manifest.exact_case_id_overlap_ids.len(),
            exact_source_token_overlap_count: report
                .exclusion_manifest
                .exact_source_token_overlap_case_ids
                .len(),
            exact_target_token_overlap_count: report
                .exclusion_manifest
                .exact_target_token_overlap_case_ids
                .len(),
            exact_sequence_overlap_count: report
                .exclusion_manifest
                .exact_sequence_overlap_case_ids
                .len(),
            near_duplicate_pair_count: report.near_duplicate_review.near_duplicate_pair_count,
            shared_generator_id_count: report.generator_overlap_audit.shared_generator_ids.len(),
            shared_generator_rule_digest_count: report
                .generator_overlap_audit
                .shared_generator_rule_digests
                .len(),
            shared_profile_id_count: report.feature_distribution_review.shared_profile_ids.len(),
            evaluation_independence_green: report.evaluation_independence_green,
            article_equivalence_green: report.article_equivalence_green,
            report,
            claim_boundary: String::from(
                "this summary mirrors only the dataset-contamination and evaluation-independence audit for the owned reference-linear article route. It keeps the overlap, near-duplicate, generator, and feature-separation counts operator-readable without widening the underlying article-equivalence claim boundary.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        summary.summary = format!(
            "Article evaluation-independence summary now records training_case_count={}, evaluation_case_count={}, exact_case_id_overlap_count={}, exact_source_token_overlap_count={}, exact_target_token_overlap_count={}, exact_sequence_overlap_count={}, near_duplicate_pair_count={}, shared_generator_id_count={}, shared_generator_rule_digest_count={}, shared_profile_id_count={}, evaluation_independence_green={}, and article_equivalence_green={}.",
            summary.training_case_count,
            summary.evaluation_case_count,
            summary.exact_case_id_overlap_count,
            summary.exact_source_token_overlap_count,
            summary.exact_target_token_overlap_count,
            summary.exact_sequence_overlap_count,
            summary.near_duplicate_pair_count,
            summary.shared_generator_id_count,
            summary.shared_generator_rule_digest_count,
            summary.shared_profile_id_count,
            summary.evaluation_independence_green,
            summary.article_equivalence_green,
        );
        summary.report_digest = stable_digest(
            b"psionic_tassadar_article_evaluation_independence_summary|",
            &summary,
        );
        summary
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleEvaluationIndependenceSummaryError {
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

pub fn build_tassadar_article_evaluation_independence_summary() -> Result<
    TassadarArticleEvaluationIndependenceSummary,
    TassadarArticleEvaluationIndependenceSummaryError,
> {
    let report: TassadarArticleEvaluationIndependenceAuditReport = read_repo_json(
        TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_AUDIT_REPORT_REF,
        "article_evaluation_independence_audit_report",
    )?;
    Ok(TassadarArticleEvaluationIndependenceSummary::new(report))
}

pub fn tassadar_article_evaluation_independence_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_article_evaluation_independence_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleEvaluationIndependenceSummary,
    TassadarArticleEvaluationIndependenceSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleEvaluationIndependenceSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_evaluation_independence_summary()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleEvaluationIndependenceSummaryError::Write {
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
) -> Result<T, TassadarArticleEvaluationIndependenceSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleEvaluationIndependenceSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleEvaluationIndependenceSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_evaluation_independence_summary, read_repo_json,
        tassadar_article_evaluation_independence_summary_path,
        write_tassadar_article_evaluation_independence_summary,
        TassadarArticleEvaluationIndependenceSummary,
        TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_SUMMARY_REPORT_REF,
    };

    #[test]
    fn evaluation_independence_summary_tracks_green_audit_without_final_green() {
        let summary = build_tassadar_article_evaluation_independence_summary().expect("summary");

        assert_eq!(summary.training_case_count, 4);
        assert_eq!(summary.evaluation_case_count, 6);
        assert_eq!(summary.exact_case_id_overlap_count, 0);
        assert_eq!(summary.exact_source_token_overlap_count, 0);
        assert_eq!(summary.exact_target_token_overlap_count, 0);
        assert_eq!(summary.exact_sequence_overlap_count, 0);
        assert_eq!(summary.near_duplicate_pair_count, 0);
        assert_eq!(summary.shared_generator_id_count, 0);
        assert_eq!(summary.shared_generator_rule_digest_count, 0);
        assert_eq!(summary.shared_profile_id_count, 0);
        assert!(summary.tied_requirement_satisfied);
        assert_eq!(summary.acceptance_status, "green");
        assert!(summary.evaluation_independence_green);
        assert!(summary.article_equivalence_green);
    }

    #[test]
    fn evaluation_independence_summary_matches_committed_truth() {
        let generated = build_tassadar_article_evaluation_independence_summary().expect("summary");
        let committed: TassadarArticleEvaluationIndependenceSummary = read_repo_json(
            TASSADAR_ARTICLE_EVALUATION_INDEPENDENCE_SUMMARY_REPORT_REF,
            "article_evaluation_independence_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_evaluation_independence_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_evaluation_independence_summary.json");
        let written = write_tassadar_article_evaluation_independence_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarArticleEvaluationIndependenceSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_evaluation_independence_summary_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_evaluation_independence_summary.json")
        );
    }
}
