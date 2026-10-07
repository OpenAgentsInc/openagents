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
    build_tassadar_post_article_universal_machine_proof_rebinding_report,
    TassadarPostArticleUniversalMachineProofRebindingReport,
    TassadarPostArticleUniversalMachineProofRebindingReportError,
    TassadarPostArticleUniversalMachineProofRebindingStatus,
};

pub const TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universal_machine_proof_rebinding_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalMachineProofRebindingSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_route_id: String,
    pub proof_transport_boundary_id: String,
    pub proof_rebinding_status: TassadarPostArticleUniversalMachineProofRebindingStatus,
    pub proof_transport_audit_complete: bool,
    pub proof_rebinding_complete: bool,
    pub rebound_encoding_ids: Vec<String>,
    pub carrier_split_publication_complete: bool,
    pub universality_witness_suite_reissued: bool,
    pub universal_substrate_gate_allowed: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleUniversalMachineProofRebindingSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleUniversalMachineProofRebindingReportError),
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

pub fn build_tassadar_post_article_universal_machine_proof_rebinding_summary() -> Result<
    TassadarPostArticleUniversalMachineProofRebindingSummary,
    TassadarPostArticleUniversalMachineProofRebindingSummaryError,
> {
    let report = build_tassadar_post_article_universal_machine_proof_rebinding_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleUniversalMachineProofRebindingReport,
) -> TassadarPostArticleUniversalMachineProofRebindingSummary {
    let mut summary = TassadarPostArticleUniversalMachineProofRebindingSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_id.clone(),
        canonical_model_id: report.canonical_model_id.clone(),
        canonical_weight_artifact_id: report.canonical_weight_artifact_id.clone(),
        canonical_route_id: report.canonical_route_id.clone(),
        proof_transport_boundary_id: report.proof_transport_boundary.boundary_id.clone(),
        proof_rebinding_status: report.proof_rebinding_status,
        proof_transport_audit_complete: report.proof_transport_audit_complete,
        proof_rebinding_complete: report.proof_rebinding_complete,
        rebound_encoding_ids: report.rebound_encoding_ids.clone(),
        carrier_split_publication_complete: report.carrier_split_publication_complete,
        universality_witness_suite_reissued: report.universality_witness_suite_reissued,
        universal_substrate_gate_allowed: report.universal_substrate_gate_allowed,
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article universal-machine proof rebinding summary keeps machine_identity_id=`{}`, canonical_model_id=`{}`, canonical_route_id=`{}`, rebound_encoding_ids={}, proof_transport_audit_complete={}, and proof_rebinding_status={:?}.",
            report.machine_identity_id,
            report.canonical_model_id,
            report.canonical_route_id,
            report.rebound_encoding_ids.len(),
            report.proof_transport_audit_complete,
            report.proof_rebinding_status,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_universal_machine_proof_rebinding_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_universal_machine_proof_rebinding_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_SUMMARY_REF)
}

pub fn write_tassadar_post_article_universal_machine_proof_rebinding_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleUniversalMachineProofRebindingSummary,
    TassadarPostArticleUniversalMachineProofRebindingSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleUniversalMachineProofRebindingSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_universal_machine_proof_rebinding_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleUniversalMachineProofRebindingSummaryError::Write {
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
) -> Result<T, TassadarPostArticleUniversalMachineProofRebindingSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleUniversalMachineProofRebindingSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleUniversalMachineProofRebindingSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_universal_machine_proof_rebinding_summary, read_repo_json,
        tassadar_post_article_universal_machine_proof_rebinding_summary_path,
        write_tassadar_post_article_universal_machine_proof_rebinding_summary,
        TassadarPostArticleUniversalMachineProofRebindingSummary,
        TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_SUMMARY_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn proof_rebinding_summary_keeps_next_frontier_visible() {
        let summary = build_tassadar_post_article_universal_machine_proof_rebinding_summary()
            .expect("summary");

        assert!(summary.proof_transport_audit_complete);
        assert!(summary.proof_rebinding_complete);
        assert_eq!(summary.rebound_encoding_ids.len(), 2);
        assert_eq!(summary.deferred_issue_ids, vec![String::from("TAS-191")]);
        assert!(!summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn proof_rebinding_summary_matches_committed_truth() {
        let generated = build_tassadar_post_article_universal_machine_proof_rebinding_summary()
            .expect("summary");
        let committed: TassadarPostArticleUniversalMachineProofRebindingSummary =
            read_repo_json(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_universal_machine_proof_rebinding_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_universal_machine_proof_rebinding_summary.json")
        );
    }

    #[test]
    fn write_proof_rebinding_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_universal_machine_proof_rebinding_summary.json");
        let written =
            write_tassadar_post_article_universal_machine_proof_rebinding_summary(&output_path)
                .expect("write summary");
        let persisted: TassadarPostArticleUniversalMachineProofRebindingSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
