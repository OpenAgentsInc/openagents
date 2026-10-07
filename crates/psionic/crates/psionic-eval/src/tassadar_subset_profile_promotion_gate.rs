use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    TassadarBroadInternalComputeProfilePublicationStatus,
    TassadarBroadInternalComputeWorldMountBindingStatus,
    TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
};
use psionic_router::{
    build_tassadar_broad_internal_compute_route_policy_report,
    TassadarBroadInternalComputeRouteDecisionStatus,
    TassadarBroadInternalComputeRoutePolicyReportError,
    TASSADAR_BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_broad_internal_compute_acceptance_gate_report,
    build_tassadar_broad_internal_compute_profile_publication_report,
    build_tassadar_effect_safe_resume_report, build_tassadar_linked_program_bundle_eval_report,
    TassadarBroadInternalComputeAcceptanceGateReportError,
    TassadarBroadInternalComputeAcceptanceStatus,
    TassadarBroadInternalComputeProfilePublicationReportError, TassadarEffectSafeResumeReportError,
    TASSADAR_BROAD_INTERNAL_COMPUTE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_EFFECT_SAFE_RESUME_REPORT_REF, TASSADAR_LINKED_PROGRAM_BUNDLE_EVAL_REPORT_REF,
};

pub const TASSADAR_SUBSET_PROFILE_PROMOTION_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_subset_profile_promotion_gate_report.json";

const DETERMINISTIC_IMPORT_SUBSET_PROFILE_ID: &str =
    "tassadar.internal_compute.deterministic_import_subset.v1";
const RUNTIME_SUPPORT_SUBSET_PROFILE_ID: &str =
    "tassadar.internal_compute.runtime_support_subset.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSubsetProfilePromotionGateRow {
    pub profile_id: String,
    pub positive_evidence_case_count: u32,
    pub negative_evidence_case_count: u32,
    pub replay_proof_complete: bool,
    pub refusal_posture_complete: bool,
    pub publication_allowed_row_count: u32,
    pub missing_required_evidence_refs: Vec<String>,
    pub publication_status: TassadarBroadInternalComputeProfilePublicationStatus,
    pub route_decision_status: TassadarBroadInternalComputeRouteDecisionStatus,
    pub world_mount_binding_status: TassadarBroadInternalComputeWorldMountBindingStatus,
    pub accepted_outcome_binding_status: TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    pub gate_green: bool,
    pub served_publication_allowed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSubsetProfilePromotionGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_report_ref: String,
    pub publication_report_ref: String,
    pub route_policy_report_ref: String,
    pub effect_safe_resume_report_ref: String,
    pub linked_program_bundle_eval_report_ref: String,
    pub profile_rows: Vec<TassadarSubsetProfilePromotionGateRow>,
    pub green_profile_ids: Vec<String>,
    pub served_publication_allowed_profile_ids: Vec<String>,
    pub failed_profile_ids: Vec<String>,
    pub overall_green: bool,
    pub world_mount_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub nexus_dependency_marker: String,
    pub compute_market_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSubsetProfilePromotionGateReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarBroadInternalComputeAcceptanceGateReportError),
    #[error(transparent)]
    Publication(#[from] TassadarBroadInternalComputeProfilePublicationReportError),
    #[error(transparent)]
    RoutePolicy(#[from] TassadarBroadInternalComputeRoutePolicyReportError),
    #[error(transparent)]
    EffectSafeResume(#[from] TassadarEffectSafeResumeReportError),
    #[error("acceptance gate was missing profile `{profile_id}`")]
    MissingAcceptanceGateProfile { profile_id: String },
    #[error("publication report was missing profile `{profile_id}`")]
    MissingPublicationProfile { profile_id: String },
    #[error("route policy was missing profile `{profile_id}`")]
    MissingRouteProfile { profile_id: String },
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

pub fn build_tassadar_subset_profile_promotion_gate_report(
) -> Result<TassadarSubsetProfilePromotionGateReport, TassadarSubsetProfilePromotionGateReportError>
{
    let acceptance_gate = build_tassadar_broad_internal_compute_acceptance_gate_report()?;
    let publication_report = build_tassadar_broad_internal_compute_profile_publication_report()?;
    let route_policy = build_tassadar_broad_internal_compute_route_policy_report()?;
    let effect_safe_resume_report = build_tassadar_effect_safe_resume_report()?;
    let linked_program_bundle_eval_report = build_tassadar_linked_program_bundle_eval_report();

    let deterministic_row = build_deterministic_import_row(
        &acceptance_gate,
        &publication_report,
        &route_policy,
        &effect_safe_resume_report,
    )?;
    let runtime_support_row = build_runtime_support_row(
        &acceptance_gate,
        &publication_report,
        &route_policy,
        &linked_program_bundle_eval_report,
    )?;
    let profile_rows = vec![deterministic_row, runtime_support_row];
    let green_profile_ids = profile_rows
        .iter()
        .filter(|row| row.gate_green)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let served_publication_allowed_profile_ids = profile_rows
        .iter()
        .filter(|row| row.served_publication_allowed)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let failed_profile_ids = profile_rows
        .iter()
        .filter(|row| !row.gate_green)
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let overall_green = failed_profile_ids.is_empty();

    let mut report = TassadarSubsetProfilePromotionGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.subset_profile_promotion_gate.report.v1"),
        acceptance_gate_report_ref: String::from(
            TASSADAR_BROAD_INTERNAL_COMPUTE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        publication_report_ref: String::from(
            TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
        ),
        route_policy_report_ref: String::from(TASSADAR_BROAD_INTERNAL_COMPUTE_ROUTE_POLICY_REPORT_REF),
        effect_safe_resume_report_ref: String::from(TASSADAR_EFFECT_SAFE_RESUME_REPORT_REF),
        linked_program_bundle_eval_report_ref: String::from(
            TASSADAR_LINKED_PROGRAM_BUNDLE_EVAL_REPORT_REF,
        ),
        profile_rows,
        green_profile_ids,
        served_publication_allowed_profile_ids,
        failed_profile_ids,
        overall_green,
        world_mount_dependency_marker: publication_report.world_mount_dependency_marker,
        kernel_policy_dependency_marker: publication_report.kernel_policy_dependency_marker,
        nexus_dependency_marker: publication_report.nexus_dependency_marker,
        compute_market_dependency_marker: publication_report.compute_market_dependency_marker,
        claim_boundary: String::from(
            "this gate promotes only the deterministic-import subset and runtime-support subset into explicit named-profile publication discipline. A green row means the profile is evidence-complete enough to name publicly while still staying suppressed behind profile-specific mount, accepted-outcome, and market policy; it does not imply broad served publication, arbitrary imports, arbitrary helper/runtime support, or wider internal-compute closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Subset profile promotion gate now records green_profiles={}, served_publication_allowed_profiles={}, failed_profiles={}, overall_green={}.",
        report.green_profile_ids.len(),
        report.served_publication_allowed_profile_ids.len(),
        report.failed_profile_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_subset_profile_promotion_gate_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_subset_profile_promotion_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SUBSET_PROFILE_PROMOTION_GATE_REPORT_REF)
}

pub fn write_tassadar_subset_profile_promotion_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSubsetProfilePromotionGateReport, TassadarSubsetProfilePromotionGateReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSubsetProfilePromotionGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_subset_profile_promotion_gate_report()?;
    let json =
        serde_json::to_string_pretty(&report).expect("subset profile promotion gate serializes");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSubsetProfilePromotionGateReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_subset_profile_promotion_gate_report(
    path: impl AsRef<Path>,
) -> Result<TassadarSubsetProfilePromotionGateReport, TassadarSubsetProfilePromotionGateReportError>
{
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarSubsetProfilePromotionGateReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSubsetProfilePromotionGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn build_deterministic_import_row(
    acceptance_gate: &crate::TassadarBroadInternalComputeAcceptanceGateReport,
    publication_report: &psionic_models::TassadarBroadInternalComputeProfilePublicationReport,
    route_policy: &psionic_router::TassadarBroadInternalComputeRoutePolicyReport,
    effect_safe_resume_report: &crate::TassadarEffectSafeResumeReport,
) -> Result<TassadarSubsetProfilePromotionGateRow, TassadarSubsetProfilePromotionGateReportError> {
    let acceptance_row = acceptance_gate
        .profile_rows
        .iter()
        .find(|row| row.profile_id == DETERMINISTIC_IMPORT_SUBSET_PROFILE_ID)
        .ok_or_else(|| {
            TassadarSubsetProfilePromotionGateReportError::MissingAcceptanceGateProfile {
                profile_id: String::from(DETERMINISTIC_IMPORT_SUBSET_PROFILE_ID),
            }
        })?;
    let publication_row = publication_report
        .profile_rows
        .iter()
        .find(|row| row.profile_id == DETERMINISTIC_IMPORT_SUBSET_PROFILE_ID)
        .ok_or_else(|| {
            TassadarSubsetProfilePromotionGateReportError::MissingPublicationProfile {
                profile_id: String::from(DETERMINISTIC_IMPORT_SUBSET_PROFILE_ID),
            }
        })?;
    let route_row = route_policy
        .rows
        .iter()
        .find(|row| row.target_profile_id == DETERMINISTIC_IMPORT_SUBSET_PROFILE_ID)
        .ok_or_else(
            || TassadarSubsetProfilePromotionGateReportError::MissingRouteProfile {
                profile_id: String::from(DETERMINISTIC_IMPORT_SUBSET_PROFILE_ID),
            },
        )?;

    let replay_proof_complete = effect_safe_resume_report.admitted_case_count > 0
        && !effect_safe_resume_report
            .runtime_bundle_ref
            .trim()
            .is_empty();
    let refusal_posture_complete = effect_safe_resume_report.refusal_case_count > 0
        && !effect_safe_resume_report
            .continuation_refused_effect_refs
            .is_empty();
    let gate_green = acceptance_row.gate_status == TassadarBroadInternalComputeAcceptanceStatus::Suppressed
        && publication_row.publication_status
            == TassadarBroadInternalComputeProfilePublicationStatus::Suppressed
        && route_row.decision_status
            == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        && publication_row.missing_required_evidence_refs.is_empty()
        && replay_proof_complete
        && refusal_posture_complete
        && publication_row.world_mount_binding_status
            == TassadarBroadInternalComputeWorldMountBindingStatus::ProfileSpecificMountTemplateAvailable
        && publication_row.accepted_outcome_binding_status
            == TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ProfileSpecificAcceptedOutcomeTemplateAvailable;
    let served_publication_allowed = publication_row.publication_status
        == TassadarBroadInternalComputeProfilePublicationStatus::Published
        && route_row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Selected;

    Ok(TassadarSubsetProfilePromotionGateRow {
        profile_id: String::from(DETERMINISTIC_IMPORT_SUBSET_PROFILE_ID),
        positive_evidence_case_count: effect_safe_resume_report.admitted_case_count,
        negative_evidence_case_count: effect_safe_resume_report.refusal_case_count,
        replay_proof_complete,
        refusal_posture_complete,
        publication_allowed_row_count: publication_row.publication_allowed_row_count,
        missing_required_evidence_refs: publication_row.missing_required_evidence_refs.clone(),
        publication_status: publication_row.publication_status,
        route_decision_status: route_row.decision_status,
        world_mount_binding_status: publication_row.world_mount_binding_status,
        accepted_outcome_binding_status: publication_row.accepted_outcome_binding_status,
        gate_green,
        served_publication_allowed,
        detail: if gate_green {
            String::from(
                "deterministic import subset is promotion-safe to name publicly because deterministic stub replay proofs, refusal posture, profile-specific public route promotion, and profile-specific mount/accepted-outcome templates are all explicit; it remains separate from the default served profile until portability and broader publication criteria go green",
            )
        } else {
            String::from(
                "deterministic import subset is not promotion-safe because replay proofs, refusal posture, or publication-policy boundaries are still incomplete",
            )
        },
    })
}

fn build_runtime_support_row(
    acceptance_gate: &crate::TassadarBroadInternalComputeAcceptanceGateReport,
    publication_report: &psionic_models::TassadarBroadInternalComputeProfilePublicationReport,
    route_policy: &psionic_router::TassadarBroadInternalComputeRoutePolicyReport,
    linked_program_bundle_eval_report: &crate::TassadarLinkedProgramBundleEvalReport,
) -> Result<TassadarSubsetProfilePromotionGateRow, TassadarSubsetProfilePromotionGateReportError> {
    let acceptance_row = acceptance_gate
        .profile_rows
        .iter()
        .find(|row| row.profile_id == RUNTIME_SUPPORT_SUBSET_PROFILE_ID)
        .ok_or_else(|| {
            TassadarSubsetProfilePromotionGateReportError::MissingAcceptanceGateProfile {
                profile_id: String::from(RUNTIME_SUPPORT_SUBSET_PROFILE_ID),
            }
        })?;
    let publication_row = publication_report
        .profile_rows
        .iter()
        .find(|row| row.profile_id == RUNTIME_SUPPORT_SUBSET_PROFILE_ID)
        .ok_or_else(|| {
            TassadarSubsetProfilePromotionGateReportError::MissingPublicationProfile {
                profile_id: String::from(RUNTIME_SUPPORT_SUBSET_PROFILE_ID),
            }
        })?;
    let route_row = route_policy
        .rows
        .iter()
        .find(|row| row.target_profile_id == RUNTIME_SUPPORT_SUBSET_PROFILE_ID)
        .ok_or_else(
            || TassadarSubsetProfilePromotionGateReportError::MissingRouteProfile {
                profile_id: String::from(RUNTIME_SUPPORT_SUBSET_PROFILE_ID),
            },
        )?;

    let positive_evidence_case_count = linked_program_bundle_eval_report.exact_case_count
        + linked_program_bundle_eval_report.rollback_case_count;
    let replay_proof_complete = linked_program_bundle_eval_report.graph_valid_case_count > 0
        && linked_program_bundle_eval_report.start_order_exact_case_count > 0
        && linked_program_bundle_eval_report.helper_lineage_complete_case_count > 0;
    let refusal_posture_complete = linked_program_bundle_eval_report.refused_case_count > 0;
    let gate_green = acceptance_row.gate_status == TassadarBroadInternalComputeAcceptanceStatus::Suppressed
        && publication_row.publication_status
            == TassadarBroadInternalComputeProfilePublicationStatus::Suppressed
        && route_row.decision_status
            == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        && publication_row.missing_required_evidence_refs.is_empty()
        && positive_evidence_case_count > 0
        && replay_proof_complete
        && refusal_posture_complete
        && publication_row.world_mount_binding_status
            == TassadarBroadInternalComputeWorldMountBindingStatus::ProfileSpecificMountTemplateAvailable
        && publication_row.accepted_outcome_binding_status
            == TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ProfileSpecificAcceptedOutcomeTemplateAvailable;
    let served_publication_allowed = publication_row.publication_status
        == TassadarBroadInternalComputeProfilePublicationStatus::Published
        && route_row.decision_status == TassadarBroadInternalComputeRouteDecisionStatus::Selected;

    Ok(TassadarSubsetProfilePromotionGateRow {
        profile_id: String::from(RUNTIME_SUPPORT_SUBSET_PROFILE_ID),
        positive_evidence_case_count,
        negative_evidence_case_count: linked_program_bundle_eval_report.refused_case_count,
        replay_proof_complete,
        refusal_posture_complete,
        publication_allowed_row_count: publication_row.publication_allowed_row_count,
        missing_required_evidence_refs: publication_row.missing_required_evidence_refs.clone(),
        publication_status: publication_row.publication_status,
        route_decision_status: route_row.decision_status,
        world_mount_binding_status: publication_row.world_mount_binding_status,
        accepted_outcome_binding_status: publication_row.accepted_outcome_binding_status,
        gate_green,
        served_publication_allowed,
        detail: if gate_green {
            String::from(
                "runtime-support subset is promotion-safe to name publicly because linked-bundle graph truth, helper lineage, start-order replay, refusal posture, profile-specific public route promotion, and profile-specific mount/accepted-outcome templates are all explicit; it remains separate from the default served profile until portability and broader publication criteria go green",
            )
        } else {
            String::from(
                "runtime-support subset is not promotion-safe because linked-bundle replay proofs, refusal posture, or publication-policy boundaries are still incomplete",
            )
        },
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
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
        build_tassadar_subset_profile_promotion_gate_report,
        load_tassadar_subset_profile_promotion_gate_report,
        tassadar_subset_profile_promotion_gate_report_path,
        write_tassadar_subset_profile_promotion_gate_report,
    };
    use psionic_models::{
        TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
        TassadarBroadInternalComputeWorldMountBindingStatus,
    };
    use psionic_router::TassadarBroadInternalComputeRouteDecisionStatus;

    #[test]
    fn subset_profile_promotion_gate_keeps_named_subsets_green_but_suppressed() {
        let report = build_tassadar_subset_profile_promotion_gate_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(report.green_profile_ids.len(), 2);
        assert!(report.green_profile_ids.contains(&String::from(
            "tassadar.internal_compute.deterministic_import_subset.v1"
        )));
        assert!(report.green_profile_ids.contains(&String::from(
            "tassadar.internal_compute.runtime_support_subset.v1"
        )));
        assert!(report.served_publication_allowed_profile_ids.is_empty());
        assert!(report.profile_rows.iter().all(|row| row.gate_green));
        assert!(report
            .profile_rows
            .iter()
            .all(|row| !row.served_publication_allowed));
        assert!(report.profile_rows.iter().all(|row| {
            row.route_decision_status
                == TassadarBroadInternalComputeRouteDecisionStatus::PromotedProfileSpecific
        }));
        assert!(report.profile_rows.iter().all(|row| {
            row.world_mount_binding_status
                == TassadarBroadInternalComputeWorldMountBindingStatus::ProfileSpecificMountTemplateAvailable
        }));
        assert!(report.profile_rows.iter().all(|row| {
            row.accepted_outcome_binding_status
                == TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ProfileSpecificAcceptedOutcomeTemplateAvailable
        }));
    }

    #[test]
    fn subset_profile_promotion_gate_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let expected = build_tassadar_subset_profile_promotion_gate_report()?;
        let committed = load_tassadar_subset_profile_promotion_gate_report(
            tassadar_subset_profile_promotion_gate_report_path(),
        )?;
        assert_eq!(committed, expected);
        Ok(())
    }

    #[test]
    fn write_subset_profile_promotion_gate_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output_path = std::env::temp_dir().join("tassadar_subset_profile_promotion_gate.json");
        let report = write_tassadar_subset_profile_promotion_gate_report(&output_path)?;
        let persisted = load_tassadar_subset_profile_promotion_gate_report(&output_path)?;
        assert_eq!(persisted, report);
        Ok(())
    }
}
