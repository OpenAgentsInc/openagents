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
    build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError,
    TassadarPostArticleFastRouteLegitimacyStatus,
};

pub const TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_SUMMARY_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummary {
    pub schema_version: u16,
    pub report_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub reference_linear_proof_route_descriptor_digest: String,
    pub proof_transport_boundary_id: String,
    pub contract_status: TassadarPostArticleFastRouteLegitimacyStatus,
    pub supporting_material_row_count: u32,
    pub dependency_row_count: u32,
    pub route_family_row_count: u32,
    pub invalidation_row_count: u32,
    pub validation_row_count: u32,
    pub carrier_binding_complete: bool,
    pub unproven_fast_routes_quarantined: bool,
    pub resumable_family_not_presented_as_direct_machine: bool,
    pub served_or_plugin_machine_overclaim_refused: bool,
    pub fast_route_legitimacy_complete: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub detail: String,
    pub summary_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError),
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

pub fn build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary(
) -> Result<
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummary,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummaryError,
> {
    let report =
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report()?;
    Ok(build_summary_from_report(&report))
}

fn build_summary_from_report(
    report: &TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
) -> TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummary {
    let mut summary = TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummary {
        schema_version: 1,
        report_id: report.report_id.clone(),
        machine_identity_id: report.machine_identity_binding.machine_identity_id.clone(),
        canonical_model_id: report.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: report.machine_identity_binding.canonical_route_id.clone(),
        canonical_route_descriptor_digest: report
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        reference_linear_proof_route_descriptor_digest: report
            .machine_identity_binding
            .reference_linear_proof_route_descriptor_digest
            .clone(),
        proof_transport_boundary_id: report
            .machine_identity_binding
            .proof_transport_boundary_id
            .clone(),
        contract_status: report.contract_status,
        supporting_material_row_count: report.supporting_material_rows.len() as u32,
        dependency_row_count: report.dependency_rows.len() as u32,
        route_family_row_count: report.route_family_rows.len() as u32,
        invalidation_row_count: report.invalidation_rows.len() as u32,
        validation_row_count: report.validation_rows.len() as u32,
        carrier_binding_complete: report.carrier_binding_complete,
        unproven_fast_routes_quarantined: report.unproven_fast_routes_quarantined,
        resumable_family_not_presented_as_direct_machine: report
            .resumable_family_not_presented_as_direct_machine,
        served_or_plugin_machine_overclaim_refused: report
            .served_or_plugin_machine_overclaim_refused,
        fast_route_legitimacy_complete: report.fast_route_legitimacy_complete,
        next_stability_issue_id: report.next_stability_issue_id.clone(),
        closure_bundle_issue_id: report.closure_bundle_issue_id.clone(),
        detail: format!(
            "post-article fast-route legitimacy summary keeps machine_identity_id=`{}`, canonical_route_id=`{}`, contract_status={:?}, route_family_rows={}, fast_route_legitimacy_complete={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
            report.machine_identity_binding.machine_identity_id,
            report.machine_identity_binding.canonical_route_id,
            report.contract_status,
            report.route_family_rows.len(),
            report.fast_route_legitimacy_complete,
            report.next_stability_issue_id,
            report.closure_bundle_issue_id,
        ),
        summary_digest: String::new(),
    };
    summary.summary_digest = stable_digest(
        b"psionic_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary|",
        &summary,
    );
    summary
}

#[must_use]
pub fn tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_SUMMARY_REF,
    )
}

pub fn write_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummary,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary =
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummaryError::Write {
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
) -> Result<T, TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummaryError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary,
        read_json,
        tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary_path,
        write_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary,
        TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummary,
        TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_SUMMARY_REF,
    };
    use psionic_eval::TassadarPostArticleFastRouteLegitimacyStatus;
    use tempfile::tempdir;

    #[test]
    fn fast_route_legitimacy_and_carrier_binding_contract_summary_keeps_scope_bounded() {
        let summary =
            build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary()
                .expect("summary");

        assert_eq!(
            summary.contract_status,
            TassadarPostArticleFastRouteLegitimacyStatus::Green
        );
        assert_eq!(summary.supporting_material_row_count, 9);
        assert_eq!(summary.dependency_row_count, 6);
        assert_eq!(summary.route_family_row_count, 6);
        assert_eq!(summary.invalidation_row_count, 6);
        assert_eq!(summary.validation_row_count, 8);
        assert!(summary.carrier_binding_complete);
        assert!(summary.unproven_fast_routes_quarantined);
        assert!(summary.resumable_family_not_presented_as_direct_machine);
        assert!(summary.served_or_plugin_machine_overclaim_refused);
        assert!(summary.fast_route_legitimacy_complete);
        assert_eq!(summary.next_stability_issue_id, "TAS-215");
        assert_eq!(summary.closure_bundle_issue_id, "TAS-215");
    }

    #[test]
    fn fast_route_legitimacy_and_carrier_binding_contract_summary_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary()
                .expect("summary");
        let committed: TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummary =
            read_json(
                tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary_path(),
            )
            .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_fast_route_legitimacy_and_carrier_binding_contract_summary_persists_truth() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join(
            "tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary.json",
        );
        let written =
            write_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary(
                &path,
            )
            .expect("write summary");
        let persisted: TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractSummary =
            read_json(&path).expect("persisted summary");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary.json"
            )
        );
        assert_eq!(
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_SUMMARY_REF,
            "fixtures/tassadar/reports/tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary.json"
        );
    }
}
