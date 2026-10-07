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
    build_tassadar_post_article_canonical_machine_closure_bundle_report,
    TassadarPostArticleCanonicalMachineClosureBundleReport,
    TassadarPostArticleCanonicalMachineClosureBundleReportError,
    TassadarPostArticleCanonicalMachineClosureBundleStatus,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_closure_bundle_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub closure_bundle_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub closure_bundle_digest: String,
    pub bundle_status: TassadarPostArticleCanonicalMachineClosureBundleStatus,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub invalidation_row_count: u32,
    pub validation_row_count: u32,
    pub proof_and_audit_classification_complete: bool,
    pub machine_subject_complete: bool,
    pub control_execution_and_continuation_bound: bool,
    pub hidden_state_and_observer_model_bound: bool,
    pub portability_and_minimality_bound: bool,
    pub anti_drift_closeout_inherited: bool,
    pub terminal_claims_must_reference_bundle_digest: bool,
    pub plugin_claims_must_reference_bundle_digest: bool,
    pub platform_claims_must_reference_bundle_digest: bool,
    pub closure_bundle_issue_id: String,
    pub next_issue_id: String,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCanonicalMachineClosureBundleSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleCanonicalMachineClosureBundleReportError),
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

pub fn build_tassadar_post_article_canonical_machine_closure_bundle_summary() -> Result<
    TassadarPostArticleCanonicalMachineClosureBundleSummary,
    TassadarPostArticleCanonicalMachineClosureBundleSummaryError,
> {
    let report = build_tassadar_post_article_canonical_machine_closure_bundle_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleCanonicalMachineClosureBundleReport,
) -> TassadarPostArticleCanonicalMachineClosureBundleSummary {
    let mut summary = TassadarPostArticleCanonicalMachineClosureBundleSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        closure_bundle_id: report.closure_subject.closure_bundle_id.clone(),
        machine_identity_id: report.closure_subject.machine_identity_id.clone(),
        canonical_model_id: report.closure_subject.canonical_model_id.clone(),
        canonical_route_id: report.closure_subject.canonical_route_id.clone(),
        continuation_contract_id: report.closure_subject.continuation_contract_id.clone(),
        closure_bundle_digest: report.closure_bundle_digest.clone(),
        bundle_status: report.bundle_status,
        supporting_material_row_count: report.supporting_material_rows.len() as u32,
        dependency_row_count: report.dependency_rows.len() as u32,
        invalidation_row_count: report.invalidation_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        proof_and_audit_classification_complete: report.proof_and_audit_classification_complete,
        machine_subject_complete: report.machine_subject_complete,
        control_execution_and_continuation_bound: report.control_execution_and_continuation_bound,
        hidden_state_and_observer_model_bound: report.hidden_state_and_observer_model_bound,
        portability_and_minimality_bound: report.portability_and_minimality_bound,
        anti_drift_closeout_inherited: report.anti_drift_closeout_inherited,
        terminal_claims_must_reference_bundle_digest: report
            .terminal_claims_must_reference_bundle_digest,
        plugin_claims_must_reference_bundle_digest: report
            .plugin_claims_must_reference_bundle_digest,
        platform_claims_must_reference_bundle_digest: report
            .platform_claims_must_reference_bundle_digest,
        closure_bundle_issue_id: report.closure_bundle_issue_id.clone(),
        next_issue_id: report.next_issue_id.clone(),
        detail: format!(
            "post-article canonical machine closure bundle summary keeps closure_bundle_id=`{}`, machine_identity_id=`{}`, canonical_route_id=`{}`, bundle_status={:?}, validation_rows={}, closure_bundle_digest=`{}`, and next_issue_id=`{}`.",
            report.closure_subject.closure_bundle_id,
            report.closure_subject.machine_identity_id,
            report.closure_subject.canonical_route_id,
            report.bundle_status,
            report.validation_rows.len(),
            report.closure_bundle_digest,
            report.next_issue_id,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_canonical_machine_closure_bundle_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_canonical_machine_closure_bundle_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_SUMMARY_REF)
}

pub fn write_tassadar_post_article_canonical_machine_closure_bundle_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCanonicalMachineClosureBundleSummary,
    TassadarPostArticleCanonicalMachineClosureBundleSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCanonicalMachineClosureBundleSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_canonical_machine_closure_bundle_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCanonicalMachineClosureBundleSummaryError::Write {
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
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleCanonicalMachineClosureBundleSummaryError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleCanonicalMachineClosureBundleSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalMachineClosureBundleSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_machine_closure_bundle_summary, read_json,
        tassadar_post_article_canonical_machine_closure_bundle_summary_path,
        write_tassadar_post_article_canonical_machine_closure_bundle_summary,
        TassadarPostArticleCanonicalMachineClosureBundleSummary,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_SUMMARY_REF,
    };

    #[test]
    fn closure_bundle_summary_keeps_digest_bound_machine_explicit() {
        let summary = build_tassadar_post_article_canonical_machine_closure_bundle_summary()
            .expect("summary");

        assert_eq!(
            summary.report_id,
            "tassadar.post_article_canonical_machine_closure_bundle.report.v1"
        );
        assert_eq!(
            summary.closure_bundle_id,
            "tassadar.post_article.canonical_machine.closure_bundle.v1"
        );
        assert!(summary.proof_and_audit_classification_complete);
        assert!(summary.machine_subject_complete);
        assert!(summary.control_execution_and_continuation_bound);
        assert!(summary.hidden_state_and_observer_model_bound);
        assert!(summary.portability_and_minimality_bound);
        assert!(summary.anti_drift_closeout_inherited);
        assert!(summary.terminal_claims_must_reference_bundle_digest);
        assert!(summary.plugin_claims_must_reference_bundle_digest);
        assert!(summary.platform_claims_must_reference_bundle_digest);
        assert_eq!(summary.closure_bundle_issue_id, "TAS-215");
        assert_eq!(summary.next_issue_id, "TAS-217");
        assert!(!summary.closure_bundle_digest.is_empty());
    }

    #[test]
    fn closure_bundle_summary_matches_committed_truth() {
        let expected = build_tassadar_post_article_canonical_machine_closure_bundle_summary()
            .expect("expected");
        let committed: TassadarPostArticleCanonicalMachineClosureBundleSummary =
            read_json(tassadar_post_article_canonical_machine_closure_bundle_summary_path())
                .expect("committed");

        assert_eq!(committed, expected);
        assert!(
            tassadar_post_article_canonical_machine_closure_bundle_summary_path()
                .ends_with("tassadar_post_article_canonical_machine_closure_bundle_summary.json")
        );
    }

    #[test]
    fn write_closure_bundle_summary_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_post_article_canonical_machine_closure_bundle_summary.json");
        let written =
            write_tassadar_post_article_canonical_machine_closure_bundle_summary(&output_path)
                .expect("written");
        let roundtrip: TassadarPostArticleCanonicalMachineClosureBundleSummary =
            read_json(&output_path).expect("roundtrip");

        assert_eq!(written, roundtrip);
        assert_eq!(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_SUMMARY_REF,
            "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_closure_bundle_summary.json"
        );
    }
}
