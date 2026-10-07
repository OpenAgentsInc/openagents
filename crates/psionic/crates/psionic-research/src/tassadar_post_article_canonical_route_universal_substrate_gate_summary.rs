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
    build_tassadar_post_article_canonical_route_universal_substrate_gate_report,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_route_universal_substrate_gate_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub gate_status: TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus,
    pub bounded_universality_story_carried: bool,
    pub proof_rebinding_complete: bool,
    pub witness_suite_reissued: bool,
    pub portability_row_count: u32,
    pub refusal_boundary_row_count: u32,
    pub deferred_issue_ids: Vec<String>,
    pub universal_substrate_gate_allowed: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError),
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

pub fn build_tassadar_post_article_canonical_route_universal_substrate_gate_summary() -> Result<
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummary,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummaryError,
> {
    let report = build_tassadar_post_article_canonical_route_universal_substrate_gate_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
) -> TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummary {
    let mut summary = TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_id.clone(),
        canonical_model_id: report.canonical_model_id.clone(),
        canonical_route_id: report.canonical_route_id.clone(),
        gate_status: report.gate_status,
        bounded_universality_story_carried: report.bounded_universality_story_carried,
        proof_rebinding_complete: report.proof_rebinding_complete,
        witness_suite_reissued: report.witness_suite_reissued,
        portability_row_count: report.portability_rows.len() as u32,
        refusal_boundary_row_count: report.refusal_boundary_rows.len() as u32,
        deferred_issue_ids: report.deferred_issue_ids.clone(),
        universal_substrate_gate_allowed: report.universal_substrate_gate_allowed,
        rebase_claim_allowed: report.rebase_claim_allowed,
        plugin_capability_claim_allowed: report.plugin_capability_claim_allowed,
        served_public_universality_allowed: report.served_public_universality_allowed,
        arbitrary_software_capability_allowed: report.arbitrary_software_capability_allowed,
        detail: format!(
            "post-article canonical-route universal-substrate gate summary keeps machine_identity_id=`{}`, canonical_model_id=`{}`, canonical_route_id=`{}`, portability_row_count={}, refusal_boundary_row_count={}, and gate_status={:?}.",
            report.machine_identity_id,
            report.canonical_model_id,
            report.canonical_route_id,
            report.portability_rows.len(),
            report.refusal_boundary_rows.len(),
            report.gate_status,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_canonical_route_universal_substrate_gate_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_canonical_route_universal_substrate_gate_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_SUMMARY_REF)
}

pub fn write_tassadar_post_article_canonical_route_universal_substrate_gate_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummary,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_post_article_canonical_route_universal_substrate_gate_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummaryError::Write {
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
) -> Result<T, TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_route_universal_substrate_gate_summary,
        read_repo_json, tassadar_post_article_canonical_route_universal_substrate_gate_summary_path,
        write_tassadar_post_article_canonical_route_universal_substrate_gate_summary,
        TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummary,
        TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_SUMMARY_REF,
    };
    use psionic_eval::TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus;
    use tempfile::tempdir;

    #[test]
    fn canonical_route_universal_substrate_gate_summary_keeps_next_frontier_visible() {
        let summary = build_tassadar_post_article_canonical_route_universal_substrate_gate_summary()
            .expect("summary");

        assert_eq!(
            summary.gate_status,
            TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus::Green
        );
        assert!(summary.bounded_universality_story_carried);
        assert!(summary.proof_rebinding_complete);
        assert!(summary.witness_suite_reissued);
        assert_eq!(summary.portability_row_count, 3);
        assert_eq!(summary.refusal_boundary_row_count, 2);
        assert_eq!(summary.deferred_issue_ids, vec![String::from("TAS-193")]);
        assert!(summary.universal_substrate_gate_allowed);
        assert!(!summary.rebase_claim_allowed);
        assert!(!summary.plugin_capability_claim_allowed);
        assert!(!summary.served_public_universality_allowed);
        assert!(!summary.arbitrary_software_capability_allowed);
    }

    #[test]
    fn canonical_route_universal_substrate_gate_summary_matches_committed_truth() {
        let generated = build_tassadar_post_article_canonical_route_universal_substrate_gate_summary()
            .expect("summary");
        let committed: TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummary =
            read_repo_json(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_SUMMARY_REF,
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_canonical_route_universal_substrate_gate_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_canonical_route_universal_substrate_gate_summary.json")
        );
    }

    #[test]
    fn write_canonical_route_universal_substrate_gate_summary_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_canonical_route_universal_substrate_gate_summary.json");
        let written = write_tassadar_post_article_canonical_route_universal_substrate_gate_summary(
            &output_path,
        )
        .expect("write summary");
        let persisted: TassadarPostArticleCanonicalRouteUniversalSubstrateGateSummary =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read summary"))
                .expect("decode summary");
        assert_eq!(written, persisted);
    }
}
