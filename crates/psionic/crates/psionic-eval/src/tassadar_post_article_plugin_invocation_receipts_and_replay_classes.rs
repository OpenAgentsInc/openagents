use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_effectful_replay_audit_report, build_tassadar_installed_module_evidence_report,
    build_tassadar_module_promotion_state_report, TassadarEffectfulReplayAuditReportError,
    TassadarInstalledModuleEvidenceReportError, TassadarModulePromotionLifecycleState,
    TassadarModulePromotionStateReportError, TASSADAR_EFFECTFUL_REPLAY_AUDIT_REPORT_REF,
    TASSADAR_INSTALLED_MODULE_EVIDENCE_REPORT_REF, TASSADAR_MODULE_PROMOTION_STATE_REPORT_REF,
};
use psionic_runtime::{
    build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle,
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundle,
    TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF,
    TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
};
use psionic_sandbox::{
    build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError,
    TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-plugin-invocation-receipts-and-replay-classes.sh";

const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_closure_bundle_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CanonicalMachineClosureBundleFixture {
    report_id: String,
    report_digest: String,
    closure_bundle_digest: String,
    bundle_green: bool,
    closure_subject: CanonicalMachineClosureBundleSubjectFixture,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CanonicalMachineClosureBundleSubjectFixture {
    machine_identity_id: String,
    canonical_route_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginInvocationReceiptsAndReplayClassesStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginInvocationReceiptsDependencyClass {
    RuntimePrecedent,
    ReplayPrecedent,
    EvidencePrecedent,
    PromotionPrecedent,
    ClosureBundle,
    DesignInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationReceiptsMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub runtime_api_report_id: String,
    pub runtime_api_report_digest: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub runtime_bundle_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationReceiptsDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticlePluginInvocationReceiptsDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationReceiptIdentityRow {
    pub field_id: String,
    pub required: bool,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationReplayClassReportRow {
    pub replay_class_id: String,
    pub retry_posture_id: String,
    pub propagation_posture_id: String,
    pub route_evidence_required: bool,
    pub challenge_receipt_required: bool,
    pub promotion_allowed: bool,
    pub served_claim_allowed: bool,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationFailureClassReportRow {
    pub failure_class_id: String,
    pub class_kind: String,
    pub default_replay_class_id: String,
    pub retry_posture_id: String,
    pub propagation_posture_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationReceiptsValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub runtime_api_report_ref: String,
    pub effectful_replay_audit_report_ref: String,
    pub installed_module_evidence_report_ref: String,
    pub module_promotion_state_report_ref: String,
    pub canonical_machine_closure_bundle_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticlePluginInvocationReceiptsMachineIdentityBinding,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarPostArticlePluginInvocationReceiptsAndReplayClassesBundle,
    pub dependency_rows: Vec<TassadarPostArticlePluginInvocationReceiptsDependencyRow>,
    pub receipt_identity_rows: Vec<TassadarPostArticlePluginInvocationReceiptIdentityRow>,
    pub replay_class_rows: Vec<TassadarPostArticlePluginInvocationReplayClassReportRow>,
    pub failure_class_rows: Vec<TassadarPostArticlePluginInvocationFailureClassReportRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginInvocationReceiptsValidationRow>,
    pub contract_status: TassadarPostArticlePluginInvocationReceiptsAndReplayClassesStatus,
    pub contract_green: bool,
    pub operator_internal_only_posture: bool,
    pub receipt_identity_frozen: bool,
    pub resource_summary_required: bool,
    pub failure_lattice_frozen: bool,
    pub deterministic_replay_class_frozen: bool,
    pub snapshot_replay_class_frozen: bool,
    pub operator_replay_only_class_frozen: bool,
    pub publication_refusal_class_frozen: bool,
    pub route_evidence_binding_required: bool,
    pub challenge_receipt_binding_required: bool,
    pub replay_retry_propagation_typed: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub deferred_issue_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReportError {
    #[error(transparent)]
    RuntimeApi(#[from] TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError),
    #[error(transparent)]
    ReplayAudit(#[from] TassadarEffectfulReplayAuditReportError),
    #[error(transparent)]
    InstalledEvidence(#[from] TassadarInstalledModuleEvidenceReportError),
    #[error(transparent)]
    ModulePromotion(#[from] TassadarModulePromotionStateReportError),
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
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report() -> Result<
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReport,
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReportError,
> {
    let runtime_api =
        build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report()?;
    let closure_bundle: CanonicalMachineClosureBundleFixture =
        read_json(CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF)?;
    let replay_audit = build_tassadar_effectful_replay_audit_report()?;
    let installed_evidence = build_tassadar_installed_module_evidence_report()?;
    let module_promotion = build_tassadar_module_promotion_state_report()?;
    let runtime_bundle =
        build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle();

    let runtime_api_dependency_closed =
        runtime_api.contract_green && runtime_api.deferred_issue_ids.is_empty();
    let replay_audit_precedent_bound = replay_audit.challengeable_case_count >= 3
        && replay_audit.refusal_case_count >= 3
        && replay_audit
            .replay_safe_effect_family_ids
            .iter()
            .any(|id| id == "tassadar.effect_profile.virtual_fs_mounts.v1")
        && replay_audit
            .replay_safe_effect_family_ids
            .iter()
            .any(|id| id == "tassadar.internal_compute.async_lifecycle.v1")
        && replay_audit
            .refused_effect_family_ids
            .iter()
            .any(|id| id == "challenge_receipt_missing");
    let installed_evidence_bound = installed_evidence.complete_record_count >= 1
        && installed_evidence.revocation_ready_record_count >= 1
        && installed_evidence.audit_receipt_ready_record_count >= 1;
    let promotion_state_span_bound =
        module_promotion.records.iter().any(|record| {
            record.lifecycle_state == TassadarModulePromotionLifecycleState::ActivePromoted
        }) && module_promotion.records.iter().any(|record| {
            record.lifecycle_state == TassadarModulePromotionLifecycleState::ChallengeOpen
        }) && module_promotion.records.iter().any(|record| {
            record.lifecycle_state == TassadarModulePromotionLifecycleState::Quarantined
        }) && module_promotion
            .records
            .iter()
            .any(|record| record.lifecycle_state == TassadarModulePromotionLifecycleState::Revoked);

    let machine_identity_binding = TassadarPostArticlePluginInvocationReceiptsMachineIdentityBinding {
        machine_identity_id: runtime_api
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: runtime_api
            .machine_identity_binding
            .canonical_model_id
            .clone(),
        canonical_route_id: runtime_api
            .machine_identity_binding
            .canonical_route_id
            .clone(),
        canonical_route_descriptor_digest: runtime_api
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: runtime_api
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: runtime_api
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: runtime_api
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: runtime_api
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: runtime_api
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        runtime_api_report_id: runtime_api.report_id.clone(),
        runtime_api_report_digest: runtime_api.report_digest.clone(),
        packet_abi_version: runtime_api
            .machine_identity_binding
            .packet_abi_version
            .clone(),
        host_owned_runtime_api_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
        ),
        engine_abstraction_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID),
        invocation_receipt_profile_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
        ),
        closure_bundle_report_id: closure_bundle.report_id.clone(),
        closure_bundle_report_digest: closure_bundle.report_digest.clone(),
        closure_bundle_digest: closure_bundle.closure_bundle_digest.clone(),
        runtime_bundle_id: runtime_bundle.bundle_id.clone(),
        runtime_bundle_digest: runtime_bundle.bundle_digest.clone(),
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF,
        ),
        detail: format!(
            "machine_identity_id=`{}` canonical_route_id=`{}` runtime_api_report_id=`{}` invocation_receipt_profile_id=`{}` runtime_bundle_id=`{}` and closure_bundle_digest=`{}` remain bound together.",
            runtime_api.machine_identity_binding.machine_identity_id,
            runtime_api.machine_identity_binding.canonical_route_id,
            runtime_api.report_id,
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
            runtime_bundle.bundle_id,
            closure_bundle.closure_bundle_digest,
        ),
    };
    let closure_bundle_bound_by_digest = closure_bundle.bundle_green
        && closure_bundle.closure_subject.machine_identity_id
            == machine_identity_binding.machine_identity_id
        && closure_bundle.closure_subject.canonical_route_id
            == machine_identity_binding.canonical_route_id
        && machine_identity_binding.closure_bundle_report_id == closure_bundle.report_id
        && machine_identity_binding.closure_bundle_report_digest == closure_bundle.report_digest
        && machine_identity_binding.closure_bundle_digest == closure_bundle.closure_bundle_digest;

    let dependency_rows = vec![
        dependency_row(
            "runtime_api_contract_closed",
            TassadarPostArticlePluginInvocationReceiptsDependencyClass::RuntimePrecedent,
            runtime_api_dependency_closed,
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
            Some(runtime_api.report_id.clone()),
            Some(runtime_api.report_digest.clone()),
            "the earlier host-owned runtime API contract is green and no longer defers TAS-201.",
        ),
        dependency_row(
            "effectful_replay_precedent_bound",
            TassadarPostArticlePluginInvocationReceiptsDependencyClass::ReplayPrecedent,
            replay_audit_precedent_bound,
            TASSADAR_EFFECTFUL_REPLAY_AUDIT_REPORT_REF,
            Some(replay_audit.report_id.clone()),
            Some(replay_audit.report_digest.clone()),
            "the earlier replay audit keeps challengeable replay, refused missing-evidence lanes, and replay-safe effect families explicit.",
        ),
        dependency_row(
            "installed_module_evidence_ready",
            TassadarPostArticlePluginInvocationReceiptsDependencyClass::EvidencePrecedent,
            installed_evidence_bound,
            TASSADAR_INSTALLED_MODULE_EVIDENCE_REPORT_REF,
            Some(installed_evidence.report_id.clone()),
            Some(installed_evidence.report_digest.clone()),
            "installed module evidence keeps complete, audit-ready, and revocation-ready evidence explicit for route-bound plugin receipts.",
        ),
        dependency_row(
            "module_promotion_states_explicit",
            TassadarPostArticlePluginInvocationReceiptsDependencyClass::PromotionPrecedent,
            promotion_state_span_bound,
            TASSADAR_MODULE_PROMOTION_STATE_REPORT_REF,
            Some(module_promotion.report_id.clone()),
            Some(module_promotion.report_digest.clone()),
            "promotion state keeps active, challenge-open, quarantined, and revoked posture explicit for plugin receipts.",
        ),
        dependency_row(
            "canonical_machine_closure_bundle_published",
            TassadarPostArticlePluginInvocationReceiptsDependencyClass::ClosureBundle,
            closure_bundle_bound_by_digest,
            CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
            Some(closure_bundle.report_id.clone()),
            Some(closure_bundle.report_digest.clone()),
            "the plugin invocation receipt claim must inherit the canonical machine closure bundle by report id and digest instead of reconstructing machine identity from the runtime API alone.",
        ),
        dependency_row(
            "plugin_system_receipt_spec_cited",
            TassadarPostArticlePluginInvocationReceiptsDependencyClass::DesignInput,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system design note remains the cited design input for receipt identity, replay classes, and failure taxonomy.",
        ),
    ];

    let receipt_identity_rows = runtime_bundle
        .receipt_field_rows
        .iter()
        .map(|row| {
            receipt_identity_row(
                &row.field_id,
                row.required,
                &[
                    TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF,
                    LOCAL_PLUGIN_SYSTEM_SPEC_REF,
                ],
                &row.detail,
            )
        })
        .collect::<Vec<_>>();
    let replay_class_rows = runtime_bundle
        .replay_class_rows
        .iter()
        .map(|row| {
            replay_class_row(
                &row.replay_class_id,
                &row.retry_posture_id,
                &row.propagation_posture_id,
                row.route_evidence_required,
                row.challenge_receipt_required,
                row.promotion_allowed,
                row.served_claim_allowed,
                &[
                    TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF,
                    LOCAL_PLUGIN_SYSTEM_SPEC_REF,
                ],
                &row.detail,
            )
        })
        .collect::<Vec<_>>();
    let failure_class_rows = runtime_bundle
        .failure_class_rows
        .iter()
        .map(|row| {
            failure_class_row(
                &row.failure_class_id,
                &row.class_kind,
                &row.default_replay_class_id,
                &row.retry_posture_id,
                &row.propagation_posture_id,
                &[
                    TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF,
                    LOCAL_PLUGIN_SYSTEM_SPEC_REF,
                ],
                &row.detail,
            )
        })
        .collect::<Vec<_>>();

    let receipt_identity_frozen = receipt_identity_rows.len() == 18
        && receipt_identity_rows.iter().all(|row| row.green)
        && has_field(&receipt_identity_rows, "receipt_id", true)
        && has_field(&receipt_identity_rows, "install_id", true)
        && has_field(&receipt_identity_rows, "route_evidence_refs", true)
        && has_field(&receipt_identity_rows, "challenge_receipt_ref", false);
    let resource_summary_required = has_field(&receipt_identity_rows, "resource_summary", true)
        && runtime_bundle.case_receipts.iter().all(|case| {
            case.resource_summary.logical_duration_ticks > 0
                && case.resource_summary.timeout_ceiling_millis > 0
                && case.resource_summary.memory_ceiling_bytes > 0
        });
    let failure_lattice_frozen =
        failure_class_rows.len() == 12 && failure_class_rows.iter().all(|row| row.green);
    let deterministic_replay_class_frozen = replay_class_rows.iter().any(|row| {
        row.replay_class_id == "deterministic_replayable"
            && row.green
            && row.route_evidence_required
            && row.promotion_allowed
            && !row.served_claim_allowed
    });
    let snapshot_replay_class_frozen = replay_class_rows.iter().any(|row| {
        row.replay_class_id == "replayable_with_snapshots"
            && row.green
            && row.route_evidence_required
            && row.challenge_receipt_required
            && row.promotion_allowed
            && !row.served_claim_allowed
    });
    let operator_replay_only_class_frozen = replay_class_rows.iter().any(|row| {
        row.replay_class_id == "operator_replay_only"
            && row.green
            && !row.promotion_allowed
            && !row.served_claim_allowed
    });
    let publication_refusal_class_frozen = replay_class_rows.iter().any(|row| {
        row.replay_class_id == "non_replayable_refused_for_publication"
            && row.green
            && !row.promotion_allowed
            && !row.served_claim_allowed
    });
    let route_evidence_binding_required = runtime_bundle
        .case_receipts
        .iter()
        .all(|case| !case.route_evidence_refs.is_empty());
    let challenge_receipt_binding_required = runtime_bundle.case_receipts.iter().all(|case| {
        case.status != psionic_runtime::TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactSuccessReceipt
            && !(case.status
                == psionic_runtime::TassadarPostArticlePluginInvocationReceiptCaseStatus::ExactFailureReceipt
                && case.replay_class_id == "replayable_with_snapshots")
            || case.challenge_receipt_ref.is_some()
    });
    let replay_retry_propagation_typed = replay_class_rows
        .iter()
        .all(|row| !row.retry_posture_id.is_empty() && !row.propagation_posture_id.is_empty())
        && failure_class_rows
            .iter()
            .all(|row| !row.retry_posture_id.is_empty() && !row.propagation_posture_id.is_empty());
    let operator_internal_only_posture = runtime_api.operator_internal_only_posture
        && runtime_api.rebase_claim_allowed
        && !runtime_api.plugin_capability_claim_allowed
        && !runtime_api.plugin_publication_allowed;

    let validation_rows = vec![
        validation_row(
            "runtime_api_dependency_closed",
            runtime_api_dependency_closed,
            &[TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF],
            "the earlier runtime API contract is green and no longer defers TAS-201.",
        ),
        validation_row(
            "replay_audit_precedent_bound",
            replay_audit_precedent_bound,
            &[TASSADAR_EFFECTFUL_REPLAY_AUDIT_REPORT_REF],
            "the earlier replay audit still carries challengeable and refused replay evidence explicitly.",
        ),
        validation_row(
            "installed_module_evidence_bound",
            installed_evidence_bound,
            &[TASSADAR_INSTALLED_MODULE_EVIDENCE_REPORT_REF],
            "installed module evidence still provides complete and revocation-ready evidence for receipt lineage.",
        ),
        validation_row(
            "promotion_state_span_bound",
            promotion_state_span_bound,
            &[TASSADAR_MODULE_PROMOTION_STATE_REPORT_REF],
            "promotion-state precedent still spans promoted, challenge-open, quarantined, and revoked plugin posture.",
        ),
        validation_row(
            "receipt_identity_fields_complete",
            receipt_identity_frozen && resource_summary_required,
            &[TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF],
            "receipt identity stays complete, explicit, and resource-summary-bearing.",
        ),
        validation_row(
            "replay_class_set_complete",
            deterministic_replay_class_frozen
                && snapshot_replay_class_frozen
                && operator_replay_only_class_frozen
                && publication_refusal_class_frozen,
            &[TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF],
            "the four replay classes stay explicit and typed.",
        ),
        validation_row(
            "failure_taxonomy_complete",
            failure_lattice_frozen,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF,
                LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            ],
            "the minimum plugin failure taxonomy stays frozen and typed.",
        ),
        validation_row(
            "route_and_challenge_bindings_explicit",
            route_evidence_binding_required && challenge_receipt_binding_required,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF,
                TASSADAR_EFFECTFUL_REPLAY_AUDIT_REPORT_REF,
            ],
            "route-integrated evidence and challenge bindings remain explicit for success and snapshot-replayable failure receipts.",
        ),
        validation_row(
            "overclaim_posture_blocked",
            operator_internal_only_posture,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF,
            ],
            "the receipt contract stays operator/internal-only and does not imply weighted plugin control, publication rights, or public software capability.",
        ),
        validation_row(
            "closure_bundle_bound_by_digest",
            closure_bundle_bound_by_digest,
            &[
                CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
            ],
            "the invocation receipt claim now inherits the canonical machine closure bundle by digest instead of relying on adjacent machine fields only.",
        ),
    ];

    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && receipt_identity_frozen
        && resource_summary_required
        && failure_lattice_frozen
        && deterministic_replay_class_frozen
        && snapshot_replay_class_frozen
        && operator_replay_only_class_frozen
        && publication_refusal_class_frozen
        && route_evidence_binding_required
        && challenge_receipt_binding_required
        && replay_retry_propagation_typed
        && validation_rows.iter().all(|row| row.green)
        && operator_internal_only_posture;
    let contract_status = if contract_green {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesStatus::Green
    } else {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesStatus::Incomplete
    };
    let rebase_claim_allowed = contract_green;

    let mut report = TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_plugin_invocation_receipts_and_replay_classes.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_CHECKER_REF,
        ),
        runtime_api_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
        ),
        effectful_replay_audit_report_ref: String::from(TASSADAR_EFFECTFUL_REPLAY_AUDIT_REPORT_REF),
        installed_module_evidence_report_ref: String::from(
            TASSADAR_INSTALLED_MODULE_EVIDENCE_REPORT_REF,
        ),
        module_promotion_state_report_ref: String::from(
            TASSADAR_MODULE_PROMOTION_STATE_REPORT_REF,
        ),
        canonical_machine_closure_bundle_report_ref: String::from(
            CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
        ),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        supporting_material_refs: vec![
            String::from(CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF),
            String::from(TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF),
            String::from(TASSADAR_EFFECTFUL_REPLAY_AUDIT_REPORT_REF),
            String::from(TASSADAR_INSTALLED_MODULE_EVIDENCE_REPORT_REF),
            String::from(TASSADAR_MODULE_PROMOTION_STATE_REPORT_REF),
            String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        ],
        machine_identity_binding,
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_BUNDLE_REF,
        ),
        runtime_bundle,
        dependency_rows,
        receipt_identity_rows,
        replay_class_rows,
        failure_class_rows,
        validation_rows,
        contract_status,
        contract_green,
        operator_internal_only_posture,
        receipt_identity_frozen,
        resource_summary_required,
        failure_lattice_frozen,
        deterministic_replay_class_frozen,
        snapshot_replay_class_frozen,
        operator_replay_only_class_frozen,
        publication_refusal_class_frozen,
        route_evidence_binding_required,
        challenge_receipt_binding_required,
        replay_retry_propagation_typed,
        closure_bundle_bound_by_digest,
        rebase_claim_allowed,
        plugin_capability_claim_allowed: false,
        weighted_plugin_control_allowed: false,
        plugin_publication_allowed: false,
        served_public_universality_allowed: false,
        arbitrary_software_capability_allowed: false,
        deferred_issue_ids: Vec::new(),
        claim_boundary: String::from(
            "this eval report freezes the canonical post-article plugin invocation-receipt identity and replay-class lattice above the host-owned runtime API and the earlier replay/evidence/promotion precedents. The receipt claim now inherits the canonical machine closure bundle by report id and digest instead of reconstructing machine identity from the runtime API alone. It keeps receipt identity, typed retry and propagation posture, route-integrated evidence, challenge bindings, and operator-only or publication-refused lanes machine-readable while keeping weighted plugin control, plugin publication, served/public universality, and arbitrary software capability blocked.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article plugin invocation receipts report keeps contract_status={:?}, dependency_rows={}, receipt_identity_rows={}, replay_class_rows={}, failure_class_rows={}, validation_rows={}, closure_bundle_digest=`{}`, and deferred_issue_ids={}.",
        report.contract_status,
        report.dependency_rows.len(),
        report.receipt_identity_rows.len(),
        report.replay_class_rows.len(),
        report.failure_class_rows.len(),
        report.validation_rows.len(),
        report.machine_identity_binding.closure_bundle_digest,
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report_path() -> PathBuf
{
    repo_root().join(TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF)
}

pub fn write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReport,
    TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report =
        build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn dependency_row(
    dependency_id: &str,
    dependency_class: TassadarPostArticlePluginInvocationReceiptsDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticlePluginInvocationReceiptsDependencyRow {
    TassadarPostArticlePluginInvocationReceiptsDependencyRow {
        dependency_id: String::from(dependency_id),
        dependency_class,
        satisfied,
        source_ref: String::from(source_ref),
        bound_report_id,
        bound_report_digest,
        detail: String::from(detail),
    }
}

fn receipt_identity_row(
    field_id: &str,
    required: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticlePluginInvocationReceiptIdentityRow {
    TassadarPostArticlePluginInvocationReceiptIdentityRow {
        field_id: String::from(field_id),
        required,
        green: true,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn replay_class_row(
    replay_class_id: &str,
    retry_posture_id: &str,
    propagation_posture_id: &str,
    route_evidence_required: bool,
    challenge_receipt_required: bool,
    promotion_allowed: bool,
    served_claim_allowed: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticlePluginInvocationReplayClassReportRow {
    TassadarPostArticlePluginInvocationReplayClassReportRow {
        replay_class_id: String::from(replay_class_id),
        retry_posture_id: String::from(retry_posture_id),
        propagation_posture_id: String::from(propagation_posture_id),
        route_evidence_required,
        challenge_receipt_required,
        promotion_allowed,
        served_claim_allowed,
        green: true,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn failure_class_row(
    failure_class_id: &str,
    class_kind: &str,
    default_replay_class_id: &str,
    retry_posture_id: &str,
    propagation_posture_id: &str,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticlePluginInvocationFailureClassReportRow {
    TassadarPostArticlePluginInvocationFailureClassReportRow {
        failure_class_id: String::from(failure_class_id),
        class_kind: String::from(class_kind),
        default_replay_class_id: String::from(default_replay_class_id),
        retry_posture_id: String::from(retry_posture_id),
        propagation_posture_id: String::from(propagation_posture_id),
        green: true,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticlePluginInvocationReceiptsValidationRow {
    TassadarPostArticlePluginInvocationReceiptsValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn has_field(
    rows: &[TassadarPostArticlePluginInvocationReceiptIdentityRow],
    field_id: &str,
    required: bool,
) -> bool {
    rows.iter()
        .any(|row| row.field_id == field_id && row.required == required && row.green)
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

fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReportError> {
    let path = path.as_ref();
    let resolved_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root().join(path)
    };
    let bytes = fs::read(&resolved_path).map_err(|error| {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReportError::Read {
            path: resolved_path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReportError::Decode {
            path: resolved_path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report,
        read_json, tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report_path,
        write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report,
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReport,
        TassadarPostArticlePluginInvocationReceiptsAndReplayClassesStatus,
        TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF,
        TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
    };

    #[test]
    fn post_article_plugin_invocation_receipts_report_keeps_frontier_explicit() {
        let report =
            build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report()
                .expect("report");

        assert_eq!(
            report.contract_status,
            TassadarPostArticlePluginInvocationReceiptsAndReplayClassesStatus::Green
        );
        assert_eq!(
            report.machine_identity_binding.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            report.machine_identity_binding.host_owned_runtime_api_id,
            TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID
        );
        assert_eq!(
            report.machine_identity_binding.engine_abstraction_id,
            TASSADAR_POST_ARTICLE_PLUGIN_ENGINE_ABSTRACTION_ID
        );
        assert_eq!(
            report
                .machine_identity_binding
                .invocation_receipt_profile_id,
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID
        );
        assert_eq!(report.dependency_rows.len(), 6);
        assert_eq!(report.receipt_identity_rows.len(), 18);
        assert_eq!(report.replay_class_rows.len(), 4);
        assert_eq!(report.failure_class_rows.len(), 12);
        assert_eq!(report.validation_rows.len(), 10);
        assert!(report.deferred_issue_ids.is_empty());
        assert!(report.operator_internal_only_posture);
        assert!(report.receipt_identity_frozen);
        assert!(report.resource_summary_required);
        assert!(report.failure_lattice_frozen);
        assert!(report.deterministic_replay_class_frozen);
        assert!(report.snapshot_replay_class_frozen);
        assert!(report.operator_replay_only_class_frozen);
        assert!(report.publication_refusal_class_frozen);
        assert!(report.route_evidence_binding_required);
        assert!(report.challenge_receipt_binding_required);
        assert!(report.replay_retry_propagation_typed);
        assert!(report.closure_bundle_bound_by_digest);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_plugin_invocation_receipts_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report()
                .expect("report");
        let committed: TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReport =
            read_json(
                tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report_path(),
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report_path()
                .strip_prefix(super::repo_root())
                .expect("relative report path")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF
        );
    }

    #[test]
    fn write_post_article_plugin_invocation_receipts_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report.json",
        );
        let written =
            write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticlePluginInvocationReceiptsAndReplayClassesReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
