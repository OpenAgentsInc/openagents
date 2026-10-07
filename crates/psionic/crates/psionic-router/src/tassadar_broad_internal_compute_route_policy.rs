use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
#[cfg(not(test))]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    TassadarBroadInternalComputeProfilePublicationReport,
    TassadarBroadInternalComputeProfilePublicationStatus,
    TassadarBroadInternalComputeWorldMountBindingStatus, TassadarInternalComputeExactnessPosture,
    TassadarInternalComputeImportPosture,
    TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
};

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_internal_compute_route_policy_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadInternalComputeCheckpointPosture {
    OneShotOnly,
    ResumableRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadInternalComputePortabilityEnvelope {
    DeclaredCpuMatrix,
    PortableBroadFamily,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadInternalComputeRouteDecisionStatus {
    Selected,
    PromotedProfileSpecific,
    Suppressed,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputeRoutePolicyRow {
    pub route_policy_id: String,
    pub target_profile_id: String,
    pub required_exactness_posture: TassadarInternalComputeExactnessPosture,
    pub required_checkpoint_posture: TassadarBroadInternalComputeCheckpointPosture,
    pub required_import_posture: TassadarInternalComputeImportPosture,
    pub required_portability_envelope: TassadarBroadInternalComputePortabilityEnvelope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_abi_shape_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_runtime_support_id: Option<String>,
    pub publication_status: TassadarBroadInternalComputeProfilePublicationStatus,
    pub decision_status: TassadarBroadInternalComputeRouteDecisionStatus,
    pub world_mount_binding_status: TassadarBroadInternalComputeWorldMountBindingStatus,
    pub accepted_outcome_binding_status: TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadInternalComputeRoutePolicyReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication_report_ref: String,
    pub current_served_profile_id: String,
    pub rows: Vec<TassadarBroadInternalComputeRoutePolicyRow>,
    pub selected_route_count: u32,
    pub promoted_profile_specific_route_count: u32,
    pub suppressed_route_count: u32,
    pub refused_route_count: u32,
    pub generated_from_refs: Vec<String>,
    pub world_mount_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub compute_market_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarBroadInternalComputeRoutePolicyReportError {
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
}

pub fn build_tassadar_broad_internal_compute_route_policy_report() -> Result<
    TassadarBroadInternalComputeRoutePolicyReport,
    TassadarBroadInternalComputeRoutePolicyReportError,
> {
    let publication_report: TassadarBroadInternalComputeProfilePublicationReport = read_json(
        repo_root().join(TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
    )?;
    let rows = vec![
        route_row(
            &publication_report,
            "route.article_closeout.served_exact",
            "tassadar.internal_compute.article_closeout.v1",
            TassadarInternalComputeExactnessPosture::ExactRouteBounded,
            TassadarBroadInternalComputeCheckpointPosture::OneShotOnly,
            TassadarInternalComputeImportPosture::NoImportsOnly,
            TassadarBroadInternalComputePortabilityEnvelope::DeclaredCpuMatrix,
            None,
            None,
        ),
        route_row(
            &publication_report,
            "route.generalized_abi.multi_export",
            "tassadar.internal_compute.generalized_abi.v1",
            TassadarInternalComputeExactnessPosture::ExactRouteBounded,
            TassadarBroadInternalComputeCheckpointPosture::OneShotOnly,
            TassadarInternalComputeImportPosture::NoImportsOnly,
            TassadarBroadInternalComputePortabilityEnvelope::DeclaredCpuMatrix,
            Some("bounded_multi_export_program_shapes"),
            Some("caller_owned_output_buffers"),
        ),
        route_row(
            &publication_report,
            "route.wider_numeric.data_layout",
            "tassadar.internal_compute.wider_numeric_data_layout.v1",
            TassadarInternalComputeExactnessPosture::Planned,
            TassadarBroadInternalComputeCheckpointPosture::OneShotOnly,
            TassadarInternalComputeImportPosture::NoImportsOnly,
            TassadarBroadInternalComputePortabilityEnvelope::DeclaredCpuMatrix,
            Some("generalized_abi_required"),
            Some("wider_numeric_runtime_support"),
        ),
        route_row(
            &publication_report,
            "route.runtime_support.linked_bundle",
            "tassadar.internal_compute.runtime_support_subset.v1",
            TassadarInternalComputeExactnessPosture::ExactRouteBounded,
            TassadarBroadInternalComputeCheckpointPosture::OneShotOnly,
            TassadarInternalComputeImportPosture::NoImportsOnly,
            TassadarBroadInternalComputePortabilityEnvelope::DeclaredCpuMatrix,
            Some("linked_program_bundle_abi"),
            Some("linked_program_start_order_receipts"),
        ),
        route_row(
            &publication_report,
            "route.deterministic_import.subset",
            "tassadar.internal_compute.deterministic_import_subset.v1",
            TassadarInternalComputeExactnessPosture::ExactRouteBounded,
            TassadarBroadInternalComputeCheckpointPosture::ResumableRequired,
            TassadarInternalComputeImportPosture::DeterministicStubImportsOnly,
            TassadarBroadInternalComputePortabilityEnvelope::DeclaredCpuMatrix,
            Some("generalized_abi_required"),
            Some("effect_safe_resume_receipts"),
        ),
        route_row(
            &publication_report,
            "route.resumable_multi_slice.checkpoint",
            "tassadar.internal_compute.resumable_multi_slice.v1",
            TassadarInternalComputeExactnessPosture::ExactRouteBounded,
            TassadarBroadInternalComputeCheckpointPosture::ResumableRequired,
            TassadarInternalComputeImportPosture::NoImportsOnly,
            TassadarBroadInternalComputePortabilityEnvelope::DeclaredCpuMatrix,
            Some("resumable_slice_abi"),
            Some("resumable_execution"),
        ),
        route_row(
            &publication_report,
            "route.portable_broad_family.declared_matrix",
            "tassadar.internal_compute.portable_broad_family.v1",
            TassadarInternalComputeExactnessPosture::Planned,
            TassadarBroadInternalComputeCheckpointPosture::OneShotOnly,
            TassadarInternalComputeImportPosture::Planned,
            TassadarBroadInternalComputePortabilityEnvelope::PortableBroadFamily,
            Some("generalized_abi_required"),
            Some("cross_machine_portability_envelopes"),
        ),
        route_row(
            &publication_report,
            "route.public_broad_family.publication",
            "tassadar.internal_compute.public_broad_family.v1",
            TassadarInternalComputeExactnessPosture::Planned,
            TassadarBroadInternalComputeCheckpointPosture::OneShotOnly,
            TassadarInternalComputeImportPosture::Planned,
            TassadarBroadInternalComputePortabilityEnvelope::PortableBroadFamily,
            Some("generalized_abi_required"),
            Some("broad_profile_publication_and_route_policy"),
        ),
    ];
    let selected_route_count = rows
        .iter()
        .filter(|row| {
            row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Selected
        })
        .count() as u32;
    let promoted_profile_specific_route_count = rows
        .iter()
        .filter(|row| {
            row.decision_status
                == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        })
        .count() as u32;
    let suppressed_route_count = rows
        .iter()
        .filter(|row| {
            row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Suppressed
        })
        .count() as u32;
    let refused_route_count = rows.len() as u32
        - selected_route_count
        - promoted_profile_specific_route_count
        - suppressed_route_count;
    let mut report = TassadarBroadInternalComputeRoutePolicyReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.broad_internal_compute_route_policy.report.v1"),
        publication_report_ref: String::from(
            TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
        ),
        current_served_profile_id: publication_report.current_served_profile_id.clone(),
        rows,
        selected_route_count,
        promoted_profile_specific_route_count,
        suppressed_route_count,
        refused_route_count,
        generated_from_refs: vec![
            String::from(TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF),
            publication_report.world_mount_compatibility_report_ref.clone(),
            publication_report.accepted_outcome_binding_report_ref.clone(),
            publication_report.exact_compute_market_report_ref.clone(),
        ],
        world_mount_dependency_marker: publication_report.world_mount_dependency_marker.clone(),
        kernel_policy_dependency_marker: publication_report.kernel_policy_dependency_marker.clone(),
        nexus_dependency_marker: publication_report.nexus_dependency_marker.clone(),
        compute_market_dependency_marker: publication_report.compute_market_dependency_marker.clone(),
        claim_boundary: String::from(
            "this router report selects among named internal-compute profiles by explicit exactness, checkpoint, import, ABI-shape, runtime-support, and portability criteria. It preserves selected, suppressed, and refused profile routes explicitly instead of flattening wider computation into one generic exact-compute lane",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Broad internal-compute route policy now records selected_routes={}, promoted_profile_specific_routes={}, suppressed_routes={}, refused_routes={}, current_served_profile=`{}`.",
        report.selected_route_count,
        report.promoted_profile_specific_route_count,
        report.suppressed_route_count,
        report.refused_route_count,
        report.current_served_profile_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_broad_internal_compute_route_policy_report|",
        &report,
    );
    Ok(report)
}

fn route_row(
    report: &TassadarBroadInternalComputeProfilePublicationReport,
    route_policy_id: &str,
    target_profile_id: &str,
    required_exactness_posture: TassadarInternalComputeExactnessPosture,
    required_checkpoint_posture: TassadarBroadInternalComputeCheckpointPosture,
    required_import_posture: TassadarInternalComputeImportPosture,
    required_portability_envelope: TassadarBroadInternalComputePortabilityEnvelope,
    required_abi_shape_id: Option<&str>,
    required_runtime_support_id: Option<&str>,
) -> TassadarBroadInternalComputeRoutePolicyRow {
    let publication_row = report
        .profile_rows
        .iter()
        .find(|row| row.profile_id == target_profile_id)
        .expect("publication report should cover every routed profile");
    let decision_status = match publication_row.publication_status {
        TassadarBroadInternalComputeProfilePublicationStatus::Published => {
            TassadarBroadInternalComputeRouteDecisionStatus::Selected
        }
        TassadarBroadInternalComputeProfilePublicationStatus::Suppressed
            if publication_row.world_mount_binding_status
                == TassadarBroadInternalComputeWorldMountBindingStatus::ProfileSpecificMountTemplateAvailable
                && publication_row.accepted_outcome_binding_status
                    == TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ProfileSpecificAcceptedOutcomeTemplateAvailable =>
        {
            TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        }
        TassadarBroadInternalComputeProfilePublicationStatus::Suppressed => {
            TassadarBroadInternalComputeRouteDecisionStatus::Suppressed
        }
        TassadarBroadInternalComputeProfilePublicationStatus::Failed => {
            TassadarBroadInternalComputeRouteDecisionStatus::Refused
        }
    };
    let note = match decision_status {
        TassadarBroadInternalComputeRouteDecisionStatus::Selected => format!(
            "route `{}` selects profile `{}` because the named profile is the current published lane for these route criteria",
            route_policy_id, target_profile_id
        ),
        TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific => format!(
            "route `{}` promotes profile `{}` as a publicly nameable profile-specific lane with explicit mount and accepted-outcome templates, while still keeping it separate from the current default served profile",
            route_policy_id, target_profile_id
        ),
        TassadarBroadInternalComputeRouteDecisionStatus::Suppressed => format!(
            "route `{}` points at profile `{}` but keeps it suppressed until the named profile's broader portability and public publication posture turn green",
            route_policy_id, target_profile_id
        ),
        TassadarBroadInternalComputeRouteDecisionStatus::Refused => format!(
            "route `{}` refuses profile `{}` because the named profile is still blocked on missing evidence or a non-green broader claim posture",
            route_policy_id, target_profile_id
        ),
    };
    TassadarBroadInternalComputeRoutePolicyRow {
        route_policy_id: String::from(route_policy_id),
        target_profile_id: String::from(target_profile_id),
        required_exactness_posture,
        required_checkpoint_posture,
        required_import_posture,
        required_portability_envelope,
        required_abi_shape_id: required_abi_shape_id.map(String::from),
        required_runtime_support_id: required_runtime_support_id.map(String::from),
        publication_status: publication_row.publication_status,
        decision_status,
        world_mount_binding_status: publication_row.world_mount_binding_status,
        accepted_outcome_binding_status: publication_row.accepted_outcome_binding_status,
        note,
    }
}

#[must_use]
pub fn tassadar_broad_internal_compute_route_policy_report_path() -> PathBuf {
    repo_root().join(TASSADAR_BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF)
}

pub fn write_tassadar_broad_internal_compute_route_policy_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarBroadInternalComputeRoutePolicyReport,
    TassadarBroadInternalComputeRoutePolicyReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarBroadInternalComputeRoutePolicyReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_broad_internal_compute_route_policy_report()?;
    let json = serde_json::to_string_pretty(&report)
        .expect("broad internal compute route policy serializes");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarBroadInternalComputeRoutePolicyReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

pub fn load_tassadar_broad_internal_compute_route_policy_report(
    path: impl AsRef<Path>,
) -> Result<
    TassadarBroadInternalComputeRoutePolicyReport,
    TassadarBroadInternalComputeRoutePolicyReportError,
> {
    read_json(path)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-router crate dir")
}

fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarBroadInternalComputeRoutePolicyReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarBroadInternalComputeRoutePolicyReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadInternalComputeRoutePolicyReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_broad_internal_compute_route_policy_report,
        load_tassadar_broad_internal_compute_route_policy_report,
        tassadar_broad_internal_compute_route_policy_report_path,
        TassadarBroadInternalComputeRouteDecisionStatus,
        TassadarBroadInternalComputeRoutePolicyReportError,
    };

    #[test]
    fn broad_internal_compute_route_policy_selects_article_and_suppresses_public_broad_family() {
        let report = build_tassadar_broad_internal_compute_route_policy_report().expect("report");
        assert_eq!(
            report.current_served_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.internal_compute.article_closeout.v1"
                && row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Selected
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.internal_compute.public_broad_family.v1"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::Suppressed
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.internal_compute.runtime_support_subset.v1"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_profile_id == "tassadar.internal_compute.deterministic_import_subset.v1"
                && row.decision_status
                    == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        }));
    }

    #[test]
    fn broad_internal_compute_route_policy_matches_committed_truth(
    ) -> Result<(), TassadarBroadInternalComputeRoutePolicyReportError> {
        let expected = build_tassadar_broad_internal_compute_route_policy_report()?;
        let committed = load_tassadar_broad_internal_compute_route_policy_report(
            tassadar_broad_internal_compute_route_policy_report_path(),
        )?;
        assert_eq!(committed, expected);
        Ok(())
    }
}
