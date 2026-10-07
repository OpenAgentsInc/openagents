use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_post_article_rebased_universality_verdict_split_report,
    TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    TassadarPostArticleRebasedUniversalityVerdictSplitReportError,
    TassadarPostArticleRebasedUniversalityVerdictSplitStatus,
};

pub const TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_rebased_universality_verdict_split_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleRebasedUniversalityVerdictSplitSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub current_served_internal_compute_profile_id: String,
    pub verdict_split_status: TassadarPostArticleRebasedUniversalityVerdictSplitStatus,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub supporting_material_row_count: u32,
    pub validation_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleRebasedUniversalityVerdictSplitSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleRebasedUniversalityVerdictSplitReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_post_article_rebased_universality_verdict_split_summary() -> Result<
    TassadarPostArticleRebasedUniversalityVerdictSplitSummary,
    TassadarPostArticleRebasedUniversalityVerdictSplitSummaryError,
> {
    let report = build_tassadar_post_article_rebased_universality_verdict_split_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleRebasedUniversalityVerdictSplitReport,
) -> TassadarPostArticleRebasedUniversalityVerdictSplitSummary {
    let mut summary = TassadarPostArticleRebasedUniversalityVerdictSplitSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_id.clone(),
        canonical_model_id: report.canonical_model_id.clone(),
        canonical_route_id: report.canonical_route_id.clone(),
        current_served_internal_compute_profile_id: report
            .current_served_internal_compute_profile_id
            .clone(),
        verdict_split_status: report.verdict_split_status,
        theory_green: report.theory_green,
        operator_green: report.operator_green,
        served_green: report.served_green,
        supporting_material_row_count: report.supporting_material_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article rebased universality verdict split summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, theory_green={}, operator_green={}, served_green={}, verdict_split_status={:?}, and rebase_claim_allowed={}.",
            report.machine_identity_id,
            report.canonical_route_id,
            report.theory_green,
            report.operator_green,
            report.served_green,
            report.verdict_split_status,
            report.rebase_claim_allowed,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_rebased_universality_verdict_split_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_rebased_universality_verdict_split_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_SUMMARY_REF)
}

pub fn write_tassadar_post_article_rebased_universality_verdict_split_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleRebasedUniversalityVerdictSplitSummary,
    TassadarPostArticleRebasedUniversalityVerdictSplitSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleRebasedUniversalityVerdictSplitSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_rebased_universality_verdict_split_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictSplitSummaryError::Write {
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
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleRebasedUniversalityVerdictSplitSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictSplitSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictSplitSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_rebased_universality_verdict_split_summary, read_repo_json,
        tassadar_post_article_rebased_universality_verdict_split_summary_path,
        write_tassadar_post_article_rebased_universality_verdict_split_summary,
        TassadarPostArticleRebasedUniversalityVerdictSplitSummary,
        TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_SUMMARY_REF,
    };
    use psionic_eval::TassadarPostArticleRebasedUniversalityVerdictSplitStatus;
    use tempfile::tempdir;

    #[test]
    fn post_article_rebased_universality_verdict_split_summary_keeps_rebase_green() {
        let summary = build_tassadar_post_article_rebased_universality_verdict_split_summary()
            .expect("summary");

        assert_eq!(
            summary.verdict_split_status,
            TassadarPostArticleRebasedUniversalityVerdictSplitStatus::TheoryGreenOperatorGreenServedSuppressed
        );
        assert!(summary.theory_green);
        assert!(summary.operator_green);
        assert!(!summary.served_green);
        assert_eq!(summary.supporting_material_row_count, 7);
        assert_eq!(summary.validation_row_count, 8);
        assert!(summary.deferred_issue_ids.is_empty());
        assert!(summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_rebased_universality_verdict_split_summary_matches_committed_truth() {
        let generated = build_tassadar_post_article_rebased_universality_verdict_split_summary()
            .expect("summary");
        let committed: TassadarPostArticleRebasedUniversalityVerdictSplitSummary =
            read_repo_json(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_rebased_universality_verdict_split_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_rebased_universality_verdict_split_summary.json")
        );
    }

    #[test]
    fn write_post_article_rebased_universality_verdict_split_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_rebased_universality_verdict_split_summary.json");
        let written =
            write_tassadar_post_article_rebased_universality_verdict_split_summary(&output_path)
                .expect("write summary");
        let persisted: TassadarPostArticleRebasedUniversalityVerdictSplitSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
