use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    TassadarBroadInternalComputeProfilePublicationStatus,
    TassadarBroadInternalComputeWorldMountBindingStatus,
    TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
};
use psionic_router::{
    build_tassadar_broad_general_compute_validator_route_policy_report,
    TassadarBroadGeneralComputeAuthorityStatus,
    TassadarBroadGeneralComputeValidatorRoutePolicyReportError,
    TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_ROUTE_POLICY_REPORT_REF,
};

use crate::{
    build_tassadar_broad_internal_compute_profile_publication_report,
    TassadarBroadInternalComputeProfilePublicationReportError,
};

pub const TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_BRIDGE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_general_compute_validator_bridge_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadGeneralComputeValidatorBridgeStatus {
    AcceptedOutcomeReady,
    CandidateOnlyChallengeWindow,
    SuppressedPendingPolicy,
    RefusedPendingEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadGeneralComputeValidatorBridgeRow {
    pub profile_id: String,
    pub publication_status: TassadarBroadInternalComputeProfilePublicationStatus,
    pub world_mount_binding_status: TassadarBroadInternalComputeWorldMountBindingStatus,
    pub accepted_outcome_binding_status: TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    pub authority_status: TassadarBroadGeneralComputeAuthorityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator_policy_ref: Option<String>,
    pub validator_attachment_required: bool,
    pub challenge_window_minutes: u32,
    pub economic_receipt_allowed: bool,
    pub bridge_status: TassadarBroadGeneralComputeValidatorBridgeStatus,
    pub dependency_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadGeneralComputeValidatorBridgeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub publication_report_ref: String,
    pub validator_route_policy_report_ref: String,
    pub rows: Vec<TassadarBroadGeneralComputeValidatorBridgeRow>,
    pub accepted_outcome_ready_profile_ids: Vec<String>,
    pub candidate_only_profile_ids: Vec<String>,
    pub economic_receipt_allowed_profile_ids: Vec<String>,
    pub suppressed_profile_ids: Vec<String>,
    pub refused_profile_ids: Vec<String>,
    pub world_mount_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub compute_market_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarBroadGeneralComputeValidatorBridgeReportError {
    #[error(transparent)]
    Publication(#[from] TassadarBroadInternalComputeProfilePublicationReportError),
    #[error(transparent)]
    RoutePolicy(#[from] TassadarBroadGeneralComputeValidatorRoutePolicyReportError),
    #[error("validator route policy was missing profile `{profile_id}`")]
    MissingRoutePolicyProfile { profile_id: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

pub fn build_tassadar_broad_general_compute_validator_bridge_report() -> Result<
    TassadarBroadGeneralComputeValidatorBridgeReport,
    TassadarBroadGeneralComputeValidatorBridgeReportError,
> {
    let publication_report = build_tassadar_broad_internal_compute_profile_publication_report()?;
    let route_policy_report = build_tassadar_broad_general_compute_validator_route_policy_report()?;

    let rows = publication_report
        .profile_rows
        .iter()
        .map(|profile_row| {
            let route_row = route_policy_report
                .rows
                .iter()
                .find(|row| row.target_profile_id == profile_row.profile_id)
                .ok_or_else(|| {
                    TassadarBroadGeneralComputeValidatorBridgeReportError::MissingRoutePolicyProfile {
                        profile_id: profile_row.profile_id.clone(),
                    }
                })?;
            let bridge_status = match route_row.authority_status {
                TassadarBroadGeneralComputeAuthorityStatus::AcceptedOutcomeReady => {
                    TassadarBroadGeneralComputeValidatorBridgeStatus::AcceptedOutcomeReady
                }
                TassadarBroadGeneralComputeAuthorityStatus::CandidateOnlyChallengeWindow => {
                    TassadarBroadGeneralComputeValidatorBridgeStatus::CandidateOnlyChallengeWindow
                }
                TassadarBroadGeneralComputeAuthorityStatus::SuppressedPendingPolicy => {
                    TassadarBroadGeneralComputeValidatorBridgeStatus::SuppressedPendingPolicy
                }
                TassadarBroadGeneralComputeAuthorityStatus::RefusedPendingEvidence => {
                    TassadarBroadGeneralComputeValidatorBridgeStatus::RefusedPendingEvidence
                }
            };
            let dependency_refs = match bridge_status {
                TassadarBroadGeneralComputeValidatorBridgeStatus::AcceptedOutcomeReady => vec![
                    String::from("world-mounts.internal_compute.article_closeout.v1"),
                    String::from("kernel-policy.accepted-outcome.internal_compute.article_closeout.v1"),
                    String::from("nexus.accepted-outcome.internal_compute.article_closeout.v1"),
                    String::from("compute-market.internal_compute.article_closeout.v1"),
                ],
                TassadarBroadGeneralComputeValidatorBridgeStatus::CandidateOnlyChallengeWindow => vec![
                    format!("world-mounts.profile_specific.{}", profile_row.profile_id),
                    format!("kernel-policy.profile_specific.{}", profile_row.profile_id),
                    format!("nexus.challenge_window.{}", profile_row.profile_id),
                    format!("compute-market.profile_specific.{}", profile_row.profile_id),
                ],
                TassadarBroadGeneralComputeValidatorBridgeStatus::SuppressedPendingPolicy => vec![
                    format!("world-mounts.pending.{}", profile_row.profile_id),
                    format!("kernel-policy.pending.{}", profile_row.profile_id),
                ],
                TassadarBroadGeneralComputeValidatorBridgeStatus::RefusedPendingEvidence => vec![
                    format!("portability.pending.{}", profile_row.profile_id),
                    format!("validator.pending.{}", profile_row.profile_id),
                ],
            };
            Ok::<_, TassadarBroadGeneralComputeValidatorBridgeReportError>(
                TassadarBroadGeneralComputeValidatorBridgeRow {
                    profile_id: profile_row.profile_id.clone(),
                    publication_status: profile_row.publication_status,
                    world_mount_binding_status: profile_row.world_mount_binding_status,
                    accepted_outcome_binding_status: profile_row.accepted_outcome_binding_status,
                    authority_status: route_row.authority_status,
                    validator_policy_ref: route_row.validator_policy_ref.clone(),
                    validator_attachment_required: route_row.validator_policy_ref.is_some(),
                    challenge_window_minutes: route_row.challenge_window_minutes,
                    economic_receipt_allowed: route_row.economic_receipt_allowed,
                    bridge_status,
                    dependency_refs,
                    note: format!(
                        "{}; publication note: {}",
                        route_row.note, profile_row.note
                    ),
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let accepted_outcome_ready_profile_ids = rows
        .iter()
        .filter(|row| {
            row.bridge_status == TassadarBroadGeneralComputeValidatorBridgeStatus::AcceptedOutcomeReady
        })
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let candidate_only_profile_ids = rows
        .iter()
        .filter(|row| {
            row.bridge_status
                == TassadarBroadGeneralComputeValidatorBridgeStatus::CandidateOnlyChallengeWindow
        })
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let economic_receipt_allowed_profile_ids = rows
        .iter()
        .filter(|row| row.economic_receipt_allowed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let suppressed_profile_ids = rows
        .iter()
        .filter(|row| {
            row.bridge_status
                == TassadarBroadGeneralComputeValidatorBridgeStatus::SuppressedPendingPolicy
        })
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let refused_profile_ids = rows
        .iter()
        .filter(|row| {
            row.bridge_status
                == TassadarBroadGeneralComputeValidatorBridgeStatus::RefusedPendingEvidence
        })
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();

    let mut report = TassadarBroadGeneralComputeValidatorBridgeReport {
        schema_version: 1,
        report_id: String::from("tassadar.broad_general_compute_validator_bridge.report.v1"),
        publication_report_ref: String::from(
            TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
        ),
        validator_route_policy_report_ref: String::from(
            TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_ROUTE_POLICY_REPORT_REF,
        ),
        rows,
        accepted_outcome_ready_profile_ids,
        candidate_only_profile_ids,
        economic_receipt_allowed_profile_ids,
        suppressed_profile_ids,
        refused_profile_ids,
        world_mount_dependency_marker: String::from(
            "world-mounts remain the owner of canonical profile-specific mount templates outside standalone psionic",
        ),
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the owner of canonical validator-policy and accepted-outcome-template approval outside standalone psionic",
        ),
        nexus_dependency_marker: String::from(
            "nexus remains the owner of canonical candidate-outcome, challenge-window, and accepted-outcome issuance outside standalone psionic",
        ),
        compute_market_dependency_marker: String::from(
            "compute-market remains the owner of canonical broad-profile product publication and economic routing outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this eval report bridges named broad internal-compute profiles into validator-facing and authority-facing posture without flattening them into one generic broad-compute claim. It keeps accepted-outcome-ready, candidate-only, suppressed, and refused profiles explicit, along with their challenge-window and dependency posture.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Broad general-compute validator bridge now records accepted_outcome_ready_profiles={}, candidate_only_profiles={}, economic_receipt_allowed_profiles={}, suppressed_profiles={}, refused_profiles={}.",
        report.accepted_outcome_ready_profile_ids.len(),
        report.candidate_only_profile_ids.len(),
        report.economic_receipt_allowed_profile_ids.len(),
        report.suppressed_profile_ids.len(),
        report.refused_profile_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_broad_general_compute_validator_bridge_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_broad_general_compute_validator_bridge_report_path() -> PathBuf {
    repo_root().join(TASSADAR_BROAD_GENERAL_COMPUTE_VALIDATOR_BRIDGE_REPORT_REF)
}

pub fn write_tassadar_broad_general_compute_validator_bridge_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarBroadGeneralComputeValidatorBridgeReport,
    TassadarBroadGeneralComputeValidatorBridgeReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarBroadGeneralComputeValidatorBridgeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_broad_general_compute_validator_bridge_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("broad general-compute validator bridge serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarBroadGeneralComputeValidatorBridgeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarBroadGeneralComputeValidatorBridgeReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarBroadGeneralComputeValidatorBridgeReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadGeneralComputeValidatorBridgeReportError::Decode {
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
        build_tassadar_broad_general_compute_validator_bridge_report,
        read_json, tassadar_broad_general_compute_validator_bridge_report_path,
        TassadarBroadGeneralComputeValidatorBridgeReport,
        TassadarBroadGeneralComputeValidatorBridgeStatus,
    };

    #[test]
    fn broad_general_compute_validator_bridge_keeps_ready_candidate_and_refused_profiles_distinct() {
        let report = build_tassadar_broad_general_compute_validator_bridge_report().expect("report");

        assert_eq!(
            report.accepted_outcome_ready_profile_ids,
            vec![String::from("tassadar.internal_compute.article_closeout.v1")]
        );
        assert!(report
            .candidate_only_profile_ids
            .contains(&String::from(
                "tassadar.internal_compute.deterministic_import_subset.v1"
            )));
        assert!(report
            .candidate_only_profile_ids
            .contains(&String::from(
                "tassadar.internal_compute.runtime_support_subset.v1"
            )));
        assert!(report
            .refused_profile_ids
            .contains(&String::from(
                "tassadar.internal_compute.portable_broad_family.v1"
            )));
        assert!(report.rows.iter().any(|row| {
            row.profile_id == "tassadar.internal_compute.article_closeout.v1"
                && row.bridge_status
                    == TassadarBroadGeneralComputeValidatorBridgeStatus::AcceptedOutcomeReady
        }));
    }

    #[test]
    fn broad_general_compute_validator_bridge_matches_committed_truth() {
        let generated = build_tassadar_broad_general_compute_validator_bridge_report().expect("report");
        let committed: TassadarBroadGeneralComputeValidatorBridgeReport = read_json(
            tassadar_broad_general_compute_validator_bridge_report_path(),
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }
}
