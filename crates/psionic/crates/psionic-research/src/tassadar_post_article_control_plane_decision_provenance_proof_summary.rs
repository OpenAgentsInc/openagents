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
    build_tassadar_post_article_control_plane_decision_provenance_proof_report,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReportError,
    TassadarPostArticleControlPlaneOwnershipStatus,
};

pub const TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_control_plane_decision_provenance_proof_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleControlPlaneDecisionProvenanceProofSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub control_plane_ownership_status: TassadarPostArticleControlPlaneOwnershipStatus,
    pub control_plane_ownership_green: bool,
    pub replay_posture_green: bool,
    pub decision_provenance_proof_complete: bool,
    pub carrier_split_publication_complete: bool,
    pub deferred_issue_ids: Vec<String>,
    pub decision_binding_ids: Vec<String>,
    pub hidden_control_channel_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleControlPlaneDecisionProvenanceProofSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleControlPlaneDecisionProvenanceProofReportError),
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

pub fn build_tassadar_post_article_control_plane_decision_provenance_proof_summary() -> Result<
    TassadarPostArticleControlPlaneDecisionProvenanceProofSummary,
    TassadarPostArticleControlPlaneDecisionProvenanceProofSummaryError,
> {
    let report = build_tassadar_post_article_control_plane_decision_provenance_proof_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
) -> TassadarPostArticleControlPlaneDecisionProvenanceProofSummary {
    let mut summary = TassadarPostArticleControlPlaneDecisionProvenanceProofSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_id.clone(),
        canonical_route_id: report.canonical_route_id.clone(),
        control_plane_ownership_status: report.control_plane_ownership_status,
        control_plane_ownership_green: report.control_plane_ownership_green,
        replay_posture_green: report.replay_posture_green,
        decision_provenance_proof_complete: report.decision_provenance_proof_complete,
        carrier_split_publication_complete: report.carrier_split_publication_complete,
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        decision_binding_ids: report
            .decision_binding_rows
            .iter()
            .map(|row| row.decision_id.clone())
            .collect(),
        hidden_control_channel_ids: report
            .hidden_control_channel_rows
            .iter()
            .map(|row| row.validation_id.clone())
            .collect(),
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article control-plane summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, decision_bindings={}, hidden_control_channels={}, control_plane_ownership_status={:?}, decision_provenance_proof_complete={}, and carrier_split_publication_complete={}.",
            report.machine_identity_id,
            report.canonical_route_id,
            report.decision_binding_rows.len(),
            report.hidden_control_channel_rows.len(),
            report.control_plane_ownership_status,
            report.decision_provenance_proof_complete,
            report.carrier_split_publication_complete,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_control_plane_decision_provenance_proof_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_control_plane_decision_provenance_proof_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_SUMMARY_REF)
}

pub fn write_tassadar_post_article_control_plane_decision_provenance_proof_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleControlPlaneDecisionProvenanceProofSummary,
    TassadarPostArticleControlPlaneDecisionProvenanceProofSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleControlPlaneDecisionProvenanceProofSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_control_plane_decision_provenance_proof_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleControlPlaneDecisionProvenanceProofSummaryError::Write {
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
) -> Result<T, TassadarPostArticleControlPlaneDecisionProvenanceProofSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleControlPlaneDecisionProvenanceProofSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleControlPlaneDecisionProvenanceProofSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_control_plane_decision_provenance_proof_summary,
        read_repo_json, tassadar_post_article_control_plane_decision_provenance_proof_summary_path,
        write_tassadar_post_article_control_plane_decision_provenance_proof_summary,
        TassadarPostArticleControlPlaneDecisionProvenanceProofSummary,
        TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_SUMMARY_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn control_plane_summary_keeps_carrier_deferral_visible() {
        let summary = build_tassadar_post_article_control_plane_decision_provenance_proof_summary()
            .expect("summary");

        assert!(summary.control_plane_ownership_green);
        assert!(summary.replay_posture_green);
        assert!(summary.decision_provenance_proof_complete);
        assert_eq!(
            summary.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            summary.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert!(!summary.carrier_split_publication_complete);
        assert_eq!(summary.deferred_issue_ids, vec![String::from("TAS-189")]);
        assert!(!summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn control_plane_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_control_plane_decision_provenance_proof_summary()
                .expect("summary");
        let committed: TassadarPostArticleControlPlaneDecisionProvenanceProofSummary =
            read_repo_json(
                TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_SUMMARY_REF,
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_control_plane_decision_provenance_proof_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_control_plane_decision_provenance_proof_summary.json")
        );
    }

    #[test]
    fn write_control_plane_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_control_plane_decision_provenance_proof_summary.json");
        let written = write_tassadar_post_article_control_plane_decision_provenance_proof_summary(
            &output_path,
        )
        .expect("write summary");
        let persisted: TassadarPostArticleControlPlaneDecisionProvenanceProofSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
