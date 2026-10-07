use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::{
    tassadar_current_served_internal_compute_profile_claim,
    tassadar_internal_compute_profile_ladder_publication,
    TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
    TassadarBroadInternalComputeProfilePublicationReport,
    TassadarBroadInternalComputeProfilePublicationRow,
    TassadarBroadInternalComputeProfilePublicationStatus,
    TassadarBroadInternalComputeWorldMountBindingStatus,
    TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF,
};

use crate::{
    build_tassadar_broad_internal_compute_acceptance_gate_report,
    build_tassadar_effect_safe_resume_report, build_tassadar_linked_program_bundle_eval_report,
    TassadarBroadInternalComputeAcceptanceGateReportError,
    TassadarBroadInternalComputeAcceptanceStatus, TassadarEffectSafeResumeReport,
    TassadarLinkedProgramBundleEvalReport,
    TASSADAR_BROAD_INTERNAL_COMPUTE_ACCEPTANCE_GATE_REPORT_REF,
};

const TASSADAR_WORLD_MOUNT_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_world_mount_compatibility_report.json";
const TASSADAR_ACCEPTED_OUTCOME_BINDING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_accepted_outcome_binding_report.json";
const TASSADAR_EXACT_COMPUTE_MARKET_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_exact_compute_market_report.json";

#[derive(Debug, Error)]
pub enum TassadarBroadInternalComputeProfilePublicationReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarBroadInternalComputeAcceptanceGateReportError),
    #[error("acceptance gate was missing profile `{profile_id}`")]
    MissingAcceptanceGateProfile { profile_id: String },
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

pub fn build_tassadar_broad_internal_compute_profile_publication_report() -> Result<
    TassadarBroadInternalComputeProfilePublicationReport,
    TassadarBroadInternalComputeProfilePublicationReportError,
> {
    let ladder = tassadar_internal_compute_profile_ladder_publication();
    let acceptance_gate = build_tassadar_broad_internal_compute_acceptance_gate_report()?;
    let effect_safe_resume_report =
        build_tassadar_effect_safe_resume_report().expect("effect-safe resume report should build");
    let linked_program_bundle_eval_report = build_tassadar_linked_program_bundle_eval_report();
    let current_served_profile_id =
        tassadar_current_served_internal_compute_profile_claim().profile_id;

    let profile_rows = ladder
        .profiles
        .iter()
        .map(|profile| {
            let gate_row = acceptance_gate
                .profile_rows
                .iter()
                .find(|row| row.profile_id == profile.profile_id)
                .ok_or_else(|| {
                    TassadarBroadInternalComputeProfilePublicationReportError::MissingAcceptanceGateProfile {
                        profile_id: profile.profile_id.clone(),
                    }
                })?;
            let publication_status = match gate_row.gate_status {
                TassadarBroadInternalComputeAcceptanceStatus::Green => {
                    TassadarBroadInternalComputeProfilePublicationStatus::Published
                }
                TassadarBroadInternalComputeAcceptanceStatus::Suppressed => {
                    TassadarBroadInternalComputeProfilePublicationStatus::Suppressed
                }
                TassadarBroadInternalComputeAcceptanceStatus::Failed => {
                    TassadarBroadInternalComputeProfilePublicationStatus::Failed
                }
            };
            let profile_specific_mount_ready = profile_specific_mount_ready(
                profile.profile_id.as_str(),
                &effect_safe_resume_report,
                &linked_program_bundle_eval_report,
            );
            let (world_mount_binding_status, accepted_outcome_binding_status, note) =
                match publication_status {
                    TassadarBroadInternalComputeProfilePublicationStatus::Published => (
                        TassadarBroadInternalComputeWorldMountBindingStatus::CompatibleWithCurrentExactComputeMount,
                        TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ExactComputeEnvelopeAvailable,
                        format!(
                            "profile `{}` is the current served internal-compute profile and remains compatible with the current exact-compute mount, accepted-outcome, and market envelope refs",
                            profile.profile_id
                        ),
                    ),
                    TassadarBroadInternalComputeProfilePublicationStatus::Suppressed
                        if profile_specific_mount_ready =>
                    (
                        TassadarBroadInternalComputeWorldMountBindingStatus::ProfileSpecificMountTemplateAvailable,
                        TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ProfileSpecificAcceptedOutcomeTemplateAvailable,
                        format!(
                            "profile `{}` stays suppressed as a served capability, but now has explicit profile-specific route, mount, and accepted-outcome templates for bounded public use under named policy and portability envelopes",
                            profile.profile_id
                        ),
                    ),
                    TassadarBroadInternalComputeProfilePublicationStatus::Suppressed => (
                        TassadarBroadInternalComputeWorldMountBindingStatus::RequiresProfileSpecificMountPolicy,
                        TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::RequiresProfileSpecificAcceptedOutcomeTemplate,
                        format!(
                            "profile `{}` is benchmarked enough to name publicly, but broader mount policy and accepted-outcome templates must stay explicit before it can widen the current served lane",
                            profile.profile_id
                        ),
                    ),
                    TassadarBroadInternalComputeProfilePublicationStatus::Failed => (
                        TassadarBroadInternalComputeWorldMountBindingStatus::RefusedPendingProfileEvidence,
                        TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::RefusedPendingProfileEvidence,
                        format!(
                            "profile `{}` remains refused for broader route publication because portability, evidence, or refusal posture is still incomplete",
                            profile.profile_id
                        ),
                    ),
                };
            Ok::<
                TassadarBroadInternalComputeProfilePublicationRow,
                TassadarBroadInternalComputeProfilePublicationReportError,
            >(TassadarBroadInternalComputeProfilePublicationRow {
                profile_id: profile.profile_id.clone(),
                publication_status,
                exactness_posture: profile.exactness_posture,
                import_posture: profile.import_posture,
                portability_posture: profile.portability_posture,
                publication_allowed_row_count: gate_row.publication_allowed_row_count,
                missing_required_evidence_refs: gate_row.missing_required_evidence_refs.clone(),
                world_mount_binding_status,
                accepted_outcome_binding_status,
                note,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let published_profile_ids = profile_rows
        .iter()
        .filter(|row| {
            row.publication_status
                == TassadarBroadInternalComputeProfilePublicationStatus::Published
        })
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let suppressed_profile_ids = profile_rows
        .iter()
        .filter(|row| {
            row.publication_status
                == TassadarBroadInternalComputeProfilePublicationStatus::Suppressed
        })
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();
    let failed_profile_ids = profile_rows
        .iter()
        .filter(|row| {
            row.publication_status == TassadarBroadInternalComputeProfilePublicationStatus::Failed
        })
        .map(|row| row.profile_id.clone())
        .collect::<Vec<_>>();

    let mut report = TassadarBroadInternalComputeProfilePublicationReport {
        schema_version: 1,
        report_id: String::from("tassadar.broad_internal_compute_profile_publication.report.v1"),
        portability_report_ref: String::from(
            psionic_runtime::TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF,
        ),
        acceptance_gate_report_ref: String::from(
            TASSADAR_BROAD_INTERNAL_COMPUTE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        world_mount_compatibility_report_ref: String::from(
            TASSADAR_WORLD_MOUNT_COMPATIBILITY_REPORT_REF,
        ),
        accepted_outcome_binding_report_ref: String::from(
            TASSADAR_ACCEPTED_OUTCOME_BINDING_REPORT_REF,
        ),
        exact_compute_market_report_ref: String::from(TASSADAR_EXACT_COMPUTE_MARKET_REPORT_REF),
        current_served_profile_id,
        profile_rows,
        published_profile_ids,
        suppressed_profile_ids,
        failed_profile_ids,
        world_mount_dependency_marker: String::from(
            "world-mounts remain the owner of canonical task-scoped mount objects and mount-policy widening outside standalone psionic",
        ),
        nexus_dependency_marker: String::from(
            "nexus remains the owner of canonical accepted-outcome issuance and settlement-qualified closure outside standalone psionic",
        ),
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the owner of canonical world-mount validator policy and settlement-gated accepted-outcome authority outside standalone psionic",
        ),
        compute_market_dependency_marker: String::from(
            "compute-market remains the owner of canonical quote publication, buyer matching, and market-wide broad-profile exposure outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this eval report publishes named broad internal-compute profiles above the current exact-compute family without flattening them into one generic claim. It keeps current served publication, suppressed broader profiles, profile-specific mount posture, and accepted-outcome template posture explicit instead of silently widening one green article lane into broad served or market closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Broad internal-compute profile publication now records published_profiles={}, suppressed_profiles={}, failed_profiles={}, current_served_profile=`{}`.",
        report.published_profile_ids.len(),
        report.suppressed_profile_ids.len(),
        report.failed_profile_ids.len(),
        report.current_served_profile_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_broad_internal_compute_profile_publication_report|",
        &report,
    );
    Ok(report)
}

pub fn tassadar_broad_internal_compute_profile_publication_report_path() -> PathBuf {
    repo_root().join(TASSADAR_BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REPORT_REF)
}

pub fn write_tassadar_broad_internal_compute_profile_publication_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarBroadInternalComputeProfilePublicationReport,
    TassadarBroadInternalComputeProfilePublicationReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarBroadInternalComputeProfilePublicationReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_broad_internal_compute_profile_publication_report()?;
    let bytes = serde_json::to_vec_pretty(&report)
        .expect("broad internal compute profile publication serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarBroadInternalComputeProfilePublicationReportError::Write {
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

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn profile_specific_mount_ready(
    profile_id: &str,
    effect_safe_resume_report: &TassadarEffectSafeResumeReport,
    linked_program_bundle_eval_report: &TassadarLinkedProgramBundleEvalReport,
) -> bool {
    match profile_id {
        "tassadar.internal_compute.deterministic_import_subset.v1" => {
            effect_safe_resume_report.target_profile_id == profile_id
                && effect_safe_resume_report.admitted_case_count > 0
                && effect_safe_resume_report.refusal_case_count > 0
                && !effect_safe_resume_report
                    .continuation_safe_effect_refs
                    .is_empty()
        }
        "tassadar.internal_compute.runtime_support_subset.v1" => {
            linked_program_bundle_eval_report.exact_case_count > 0
                && linked_program_bundle_eval_report.helper_lineage_complete_case_count > 0
                && linked_program_bundle_eval_report.graph_valid_case_count > 0
                && linked_program_bundle_eval_report.start_order_exact_case_count > 0
        }
        _ => false,
    }
}

#[cfg(test)]
pub fn load_tassadar_broad_internal_compute_profile_publication_report(
    path: impl AsRef<Path>,
) -> Result<
    TassadarBroadInternalComputeProfilePublicationReport,
    TassadarBroadInternalComputeProfilePublicationReportError,
> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarBroadInternalComputeProfilePublicationReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarBroadInternalComputeProfilePublicationReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_broad_internal_compute_profile_publication_report,
        load_tassadar_broad_internal_compute_profile_publication_report,
        tassadar_broad_internal_compute_profile_publication_report_path,
        TassadarBroadInternalComputeProfilePublicationReportError,
    };
    use psionic_models::{
        TassadarBroadInternalComputeAcceptedOutcomeBindingStatus,
        TassadarBroadInternalComputeProfilePublicationStatus,
        TassadarBroadInternalComputeWorldMountBindingStatus,
    };

    #[test]
    fn broad_internal_compute_profile_publication_keeps_article_live_and_public_broad_suppressed() {
        let report =
            build_tassadar_broad_internal_compute_profile_publication_report().expect("report");
        assert_eq!(
            report.current_served_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert!(report.published_profile_ids.contains(&String::from(
            "tassadar.internal_compute.article_closeout.v1"
        )));
        assert!(report.profile_rows.iter().any(|row| {
            row.profile_id == "tassadar.internal_compute.article_closeout.v1"
                && row.publication_status
                    == TassadarBroadInternalComputeProfilePublicationStatus::Published
                && row.world_mount_binding_status
                    == TassadarBroadInternalComputeWorldMountBindingStatus::CompatibleWithCurrentExactComputeMount
                && row.accepted_outcome_binding_status
                    == TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ExactComputeEnvelopeAvailable
        }));
        assert!(report.profile_rows.iter().any(|row| {
            row.profile_id == "tassadar.internal_compute.public_broad_family.v1"
                && row.publication_status
                    == TassadarBroadInternalComputeProfilePublicationStatus::Suppressed
                && row.world_mount_binding_status
                    == TassadarBroadInternalComputeWorldMountBindingStatus::RequiresProfileSpecificMountPolicy
                && row.accepted_outcome_binding_status
                    == TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::RequiresProfileSpecificAcceptedOutcomeTemplate
        }));
        assert!(report.profile_rows.iter().any(|row| {
            row.profile_id == "tassadar.internal_compute.deterministic_import_subset.v1"
                && row.publication_status
                    == TassadarBroadInternalComputeProfilePublicationStatus::Suppressed
                && row.world_mount_binding_status
                    == TassadarBroadInternalComputeWorldMountBindingStatus::ProfileSpecificMountTemplateAvailable
                && row.accepted_outcome_binding_status
                    == TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ProfileSpecificAcceptedOutcomeTemplateAvailable
        }));
        assert!(report.profile_rows.iter().any(|row| {
            row.profile_id == "tassadar.internal_compute.runtime_support_subset.v1"
                && row.publication_status
                    == TassadarBroadInternalComputeProfilePublicationStatus::Suppressed
                && row.world_mount_binding_status
                    == TassadarBroadInternalComputeWorldMountBindingStatus::ProfileSpecificMountTemplateAvailable
                && row.accepted_outcome_binding_status
                    == TassadarBroadInternalComputeAcceptedOutcomeBindingStatus::ProfileSpecificAcceptedOutcomeTemplateAvailable
        }));
    }

    #[test]
    fn broad_internal_compute_profile_publication_matches_committed_truth(
    ) -> Result<(), TassadarBroadInternalComputeProfilePublicationReportError> {
        let expected = build_tassadar_broad_internal_compute_profile_publication_report()?;
        let committed = load_tassadar_broad_internal_compute_profile_publication_report(
            tassadar_broad_internal_compute_profile_publication_report_path(),
        )?;
        assert_eq!(committed, expected);
        Ok(())
    }
}
