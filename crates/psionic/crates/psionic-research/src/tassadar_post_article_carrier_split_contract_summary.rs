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
    build_tassadar_post_article_carrier_split_contract_report,
    TassadarPostArticleCarrierSplitContractReport,
    TassadarPostArticleCarrierSplitContractReportError, TassadarPostArticleCarrierSplitStatus,
};

pub const TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_carrier_split_contract_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCarrierSplitContractSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_route_id: String,
    pub direct_carrier_id: String,
    pub resumable_carrier_id: String,
    pub reserved_capability_plane_id: String,
    pub carrier_split_status: TassadarPostArticleCarrierSplitStatus,
    pub carrier_split_publication_complete: bool,
    pub carrier_collapse_refused: bool,
    pub reserved_capability_plane_explicit: bool,
    pub decision_provenance_proof_complete: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCarrierSplitContractSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleCarrierSplitContractReportError),
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

pub fn build_tassadar_post_article_carrier_split_contract_summary() -> Result<
    TassadarPostArticleCarrierSplitContractSummary,
    TassadarPostArticleCarrierSplitContractSummaryError,
> {
    let report = build_tassadar_post_article_carrier_split_contract_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleCarrierSplitContractReport,
) -> TassadarPostArticleCarrierSplitContractSummary {
    let mut summary = TassadarPostArticleCarrierSplitContractSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_id.clone(),
        canonical_route_id: report.canonical_route_id.clone(),
        direct_carrier_id: report.direct_carrier_id.clone(),
        resumable_carrier_id: report.resumable_carrier_id.clone(),
        reserved_capability_plane_id: report.reserved_capability_plane_id.clone(),
        carrier_split_status: report.carrier_split_status,
        carrier_split_publication_complete: report.carrier_split_publication_complete,
        carrier_collapse_refused: report.carrier_collapse_refused,
        reserved_capability_plane_explicit: report.reserved_capability_plane_explicit,
        decision_provenance_proof_complete: report.decision_provenance_proof_complete,
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article carrier-split summary keeps machine_identity_id=`{}`, direct_carrier_id=`{}`, resumable_carrier_id=`{}`, carrier_split_status={:?}, carrier_collapse_refused={}, carrier_split_publication_complete={}, and deferred_issue_ids={}.",
            report.machine_identity_id,
            report.direct_carrier_id,
            report.resumable_carrier_id,
            report.carrier_split_status,
            report.carrier_collapse_refused,
            report.carrier_split_publication_complete,
            report.deferred_issue_ids.len(),
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_carrier_split_contract_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_carrier_split_contract_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_SUMMARY_REF)
}

pub fn write_tassadar_post_article_carrier_split_contract_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCarrierSplitContractSummary,
    TassadarPostArticleCarrierSplitContractSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCarrierSplitContractSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_carrier_split_contract_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCarrierSplitContractSummaryError::Write {
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
) -> Result<T, TassadarPostArticleCarrierSplitContractSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleCarrierSplitContractSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCarrierSplitContractSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_carrier_split_contract_summary, read_repo_json,
        tassadar_post_article_carrier_split_contract_summary_path,
        write_tassadar_post_article_carrier_split_contract_summary,
        TassadarPostArticleCarrierSplitContractSummary,
        TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_SUMMARY_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn carrier_split_summary_keeps_next_frontier_visible() {
        let summary =
            build_tassadar_post_article_carrier_split_contract_summary().expect("summary");

        assert!(summary.carrier_split_publication_complete);
        assert!(summary.carrier_collapse_refused);
        assert!(summary.reserved_capability_plane_explicit);
        assert!(summary.decision_provenance_proof_complete);
        assert_eq!(summary.deferred_issue_ids, vec![String::from("TAS-190")]);
        assert!(!summary.rebase_claim_allowed);
    }

    #[test]
    fn carrier_split_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_carrier_split_contract_summary().expect("summary");
        let committed: TassadarPostArticleCarrierSplitContractSummary =
            read_repo_json(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_carrier_split_contract_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_carrier_split_contract_summary.json")
        );
    }

    #[test]
    fn write_carrier_split_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_carrier_split_contract_summary.json");
        let written = write_tassadar_post_article_carrier_split_contract_summary(&output_path)
            .expect("write summary");
        let persisted: TassadarPostArticleCarrierSplitContractSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
