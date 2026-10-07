use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_post_article_universality_bridge_contract_report,
    TassadarPostArticleCarrierTopology, TassadarPostArticlePlaneKind,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
    TassadarPostArticleUniversalityBridgeStatus,
};

pub const TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_bridge_contract_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleUniversalityBridgeContractSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub bridge_machine_identity_id: String,
    pub carrier_topology: TassadarPostArticleCarrierTopology,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub direct_carrier_id: String,
    pub resumable_carrier_id: String,
    pub reserved_capability_plane_id: String,
    pub reserved_later_invariant_ids: Vec<String>,
    pub reserved_capability_issue_ids: Vec<String>,
    pub bridge_status: TassadarPostArticleUniversalityBridgeStatus,
    pub bridge_contract_green: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleUniversalityBridgeContractSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleUniversalityBridgeContractReportError),
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

pub fn build_tassadar_post_article_universality_bridge_contract_summary() -> Result<
    TassadarPostArticleUniversalityBridgeContractSummary,
    TassadarPostArticleUniversalityBridgeContractSummaryError,
> {
    let eval_report = build_tassadar_post_article_universality_bridge_contract_report()?;
    Ok(build_summary_from_report(&eval_report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleUniversalityBridgeContractReport,
) -> TassadarPostArticleUniversalityBridgeContractSummary {
    let carrier_ids = report
        .carrier_rows
        .iter()
        .map(|row| row.carrier_id.clone())
        .collect::<Vec<_>>();
    let reserved_capability_issue_ids = report
        .plane_contract_rows
        .iter()
        .filter(|row| row.plane_kind == TassadarPostArticlePlaneKind::Capability)
        .flat_map(|row| row.reserved_issue_ids.iter().cloned())
        .chain(
            report
                .reservation_hook_rows
                .iter()
                .flat_map(|row| row.reserved_issue_ids.iter().cloned()),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let mut summary = TassadarPostArticleUniversalityBridgeContractSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        bridge_machine_identity_id: report.bridge_machine_identity.machine_identity_id.clone(),
        carrier_topology: report.carrier_topology,
        canonical_model_id: report.bridge_machine_identity.canonical_model_id.clone(),
        canonical_route_id: report.bridge_machine_identity.canonical_route_id.clone(),
        continuation_contract_id: report
            .bridge_machine_identity
            .continuation_contract_id
            .clone(),
        direct_carrier_id: carrier_ids
            .first()
            .cloned()
            .expect("direct carrier should exist"),
        resumable_carrier_id: carrier_ids
            .get(1)
            .cloned()
            .expect("resumable carrier should exist"),
        reserved_capability_plane_id: carrier_ids
            .get(2)
            .cloned()
            .expect("capability plane should exist"),
        reserved_later_invariant_ids: report.reserved_later_invariant_ids.clone(),
        reserved_capability_issue_ids,
        bridge_status: report.bridge_status,
        bridge_contract_green: report.bridge_contract_green,
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        detail: format!(
            "post-article universality bridge summary keeps machine_identity_id=`{}`, carrier_topology={:?}, canonical_route_id=`{}`, reserved_capability_issue_ids={}, bridge_status={:?}, rebase_claim_allowed={}, plugin_capability_claim_allowed={}, and served_public_universality_allowed={}.",
            report.bridge_machine_identity.machine_identity_id,
            report.carrier_topology,
            report.bridge_machine_identity.canonical_route_id,
            report
                .reservation_hook_rows
                .iter()
                .flat_map(|row| row.reserved_issue_ids.iter())
                .collect::<BTreeSet<_>>()
                .len(),
            report.bridge_status,
            report.rebase_claim_allowed,
            report.plugin_capability_claim_allowed,
            report.served_public_universality_allowed,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_universality_bridge_contract_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_universality_bridge_contract_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_SUMMARY_REF)
}

pub fn write_tassadar_post_article_universality_bridge_contract_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleUniversalityBridgeContractSummary,
    TassadarPostArticleUniversalityBridgeContractSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleUniversalityBridgeContractSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_universality_bridge_contract_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleUniversalityBridgeContractSummaryError::Write {
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
) -> Result<T, TassadarPostArticleUniversalityBridgeContractSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleUniversalityBridgeContractSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleUniversalityBridgeContractSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_universality_bridge_contract_summary, read_repo_json,
        tassadar_post_article_universality_bridge_contract_summary_path,
        write_tassadar_post_article_universality_bridge_contract_summary,
        TassadarPostArticleUniversalityBridgeContractSummary,
        TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_SUMMARY_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn post_article_bridge_contract_summary_keeps_reserved_plugin_frontier_visible() {
        let summary =
            build_tassadar_post_article_universality_bridge_contract_summary().expect("summary");

        assert!(summary.bridge_contract_green);
        assert!(!summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert_eq!(
            summary.bridge_machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            summary.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert!(summary
            .reserved_capability_issue_ids
            .contains(&String::from("TAS-195")));
        assert!(summary
            .reserved_capability_issue_ids
            .contains(&String::from("TAS-216")));
    }

    #[test]
    fn post_article_bridge_contract_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_universality_bridge_contract_summary().expect("summary");
        let committed: TassadarPostArticleUniversalityBridgeContractSummary =
            read_repo_json(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_universality_bridge_contract_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_universality_bridge_contract_summary.json")
        );
    }

    #[test]
    fn write_post_article_bridge_contract_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_universality_bridge_contract_summary.json");
        let written =
            write_tassadar_post_article_universality_bridge_contract_summary(&output_path)
                .expect("write summary");
        let persisted: TassadarPostArticleUniversalityBridgeContractSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
