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
    build_tassadar_post_article_canonical_machine_identity_lock_report,
    TassadarPostArticleCanonicalMachineIdentityLockReport,
    TassadarPostArticleCanonicalMachineIdentityLockReportError,
    TassadarPostArticleCanonicalMachineIdentityLockStatus,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_identity_lock_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineIdentityLockSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_weight_bundle_digest: String,
    pub carrier_class_id: String,
    pub canonical_machine_lock_contract_id: String,
    pub lock_status: TassadarPostArticleCanonicalMachineIdentityLockStatus,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub artifact_binding_row_count: u32,
    pub legacy_projection_row_count: u32,
    pub invalidation_row_count: u32,
    pub validation_row_count: u32,
    pub one_canonical_machine_named: bool,
    pub mixed_carrier_evidence_bundle_refused: bool,
    pub legacy_projection_binding_complete: bool,
    pub closure_bundle_embedded_here: bool,
    pub closure_bundle_issue_id: String,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCanonicalMachineIdentityLockSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleCanonicalMachineIdentityLockReportError),
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

pub fn build_tassadar_post_article_canonical_machine_identity_lock_summary() -> Result<
    TassadarPostArticleCanonicalMachineIdentityLockSummary,
    TassadarPostArticleCanonicalMachineIdentityLockSummaryError,
> {
    let report = build_tassadar_post_article_canonical_machine_identity_lock_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleCanonicalMachineIdentityLockReport,
) -> TassadarPostArticleCanonicalMachineIdentityLockSummary {
    let legacy_projection_row_count = report
        .artifact_binding_rows
        .iter()
        .filter(|row| !row.self_carries_full_tuple)
        .count() as u32;
    let mut summary = TassadarPostArticleCanonicalMachineIdentityLockSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.canonical_machine_tuple.machine_identity_id.clone(),
        canonical_model_id: report.canonical_machine_tuple.canonical_model_id.clone(),
        canonical_route_id: report.canonical_machine_tuple.canonical_route_id.clone(),
        canonical_weight_bundle_digest: report
            .canonical_machine_tuple
            .canonical_weight_bundle_digest
            .clone(),
        carrier_class_id: report.canonical_machine_tuple.carrier_class_id.clone(),
        canonical_machine_lock_contract_id: report
            .canonical_machine_tuple
            .canonical_machine_lock_contract_id
            .clone(),
        lock_status: report.lock_status,
        supporting_material_row_count: report.supporting_material_rows.len() as u32,
        dependency_row_count: report.dependency_rows.len() as u32,
        artifact_binding_row_count: report.artifact_binding_rows.len() as u32,
        legacy_projection_row_count,
        invalidation_row_count: report.invalidation_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        one_canonical_machine_named: report.one_canonical_machine_named,
        mixed_carrier_evidence_bundle_refused: report.mixed_carrier_evidence_bundle_refused,
        legacy_projection_binding_complete: report.legacy_projection_binding_complete,
        closure_bundle_embedded_here: report.closure_bundle_embedded_here,
        closure_bundle_issue_id: report.closure_bundle_issue_id.clone(),
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        weighted_plugin_control_allowed: report.weighted_plugin_control_allowed,
        plugin_publication_allowed: report.plugin_publication_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article canonical machine identity lock summary keeps machine_identity_id=`{}`, carrier_class_id=`{}`, lock_status={:?}, artifact_binding_rows={}, legacy_projection_rows={}, invalidation_rows={}, validation_rows={}, and closure_bundle_issue_id=`{}`.",
            report.canonical_machine_tuple.machine_identity_id,
            report.canonical_machine_tuple.carrier_class_id,
            report.lock_status,
            report.artifact_binding_rows.len(),
            legacy_projection_row_count,
            report.invalidation_rows.len(),
            report.validation_rows.len(),
            report.closure_bundle_issue_id,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_canonical_machine_identity_lock_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_canonical_machine_identity_lock_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_SUMMARY_REF)
}

pub fn write_tassadar_post_article_canonical_machine_identity_lock_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCanonicalMachineIdentityLockSummary,
    TassadarPostArticleCanonicalMachineIdentityLockSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCanonicalMachineIdentityLockSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_canonical_machine_identity_lock_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCanonicalMachineIdentityLockSummaryError::Write {
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
) -> Result<T, TassadarPostArticleCanonicalMachineIdentityLockSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleCanonicalMachineIdentityLockSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalMachineIdentityLockSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_machine_identity_lock_summary, read_repo_json,
        tassadar_post_article_canonical_machine_identity_lock_summary_path,
        write_tassadar_post_article_canonical_machine_identity_lock_summary,
        TassadarPostArticleCanonicalMachineIdentityLockSummary,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_SUMMARY_REF,
    };
    use psionic_eval::TassadarPostArticleCanonicalMachineIdentityLockStatus;
    use tempfile::tempdir;

    #[test]
    fn post_article_canonical_machine_identity_lock_summary_keeps_scope_bounded() {
        let summary =
            build_tassadar_post_article_canonical_machine_identity_lock_summary().expect("summary");

        assert_eq!(
            summary.lock_status,
            TassadarPostArticleCanonicalMachineIdentityLockStatus::Green
        );
        assert_eq!(summary.supporting_material_row_count, 8);
        assert_eq!(summary.dependency_row_count, 6);
        assert_eq!(summary.artifact_binding_row_count, 16);
        assert_eq!(summary.legacy_projection_row_count, 9);
        assert_eq!(summary.invalidation_row_count, 7);
        assert_eq!(summary.validation_row_count, 8);
        assert!(summary.one_canonical_machine_named);
        assert!(summary.mixed_carrier_evidence_bundle_refused);
        assert!(summary.legacy_projection_binding_complete);
        assert!(!summary.closure_bundle_embedded_here);
        assert_eq!(summary.closure_bundle_issue_id, "TAS-215");
        assert!(summary.rebase_claim_allowed);
        assert!(summary.plugin_capability_claim_allowed);
        assert!(summary.weighted_plugin_control_allowed);
        assert!(!summary.plugin_publication_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_canonical_machine_identity_lock_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_canonical_machine_identity_lock_summary().expect("summary");
        let committed: TassadarPostArticleCanonicalMachineIdentityLockSummary =
            read_repo_json(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn post_article_canonical_machine_identity_lock_summary_round_trips_to_disk() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("lock_summary.json");
        let written = write_tassadar_post_article_canonical_machine_identity_lock_summary(&path)
            .expect("write summary");
        let reloaded: TassadarPostArticleCanonicalMachineIdentityLockSummary =
            serde_json::from_slice(&std::fs::read(&path).expect("read written summary"))
                .expect("decode summary");
        assert_eq!(written, reloaded);
    }

    #[test]
    fn post_article_canonical_machine_identity_lock_summary_path_resolves_inside_repo() {
        let path = tassadar_post_article_canonical_machine_identity_lock_summary_path();
        assert!(path.ends_with(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_SUMMARY_REF));
    }
}
