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

use psionic_runtime::TassadarPostArticlePluginAdmissibilityCaseStatus;
use psionic_sandbox::{
    build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report,
    build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError,
    TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReport,
    TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError,
    TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
    TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
};
use psionic_transformer::{
    build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract,
    TassadarPostArticleEquivalentChoiceClassKind,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContract,
};

use crate::{
    build_tassadar_post_article_control_plane_decision_provenance_proof_report,
    build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReportError,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError,
    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
    TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report.json";
pub const TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_CHECKER_REF:
    &str =
    "scripts/check-tassadar-post-article-equivalent-choice-neutrality-and-admissibility-contract.sh";

const TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract.rs";
const WORLD_MOUNT_COMPATIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_world_mount_compatibility_report.json";
const IMPORT_POLICY_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_import_policy_matrix_report.json";
const PLUGIN_SYSTEM_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const NEXT_STABILITY_ISSUE_ID: &str = "TAS-215";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleEquivalentChoiceNeutralityStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleEquivalentChoiceSupportingMaterialClass {
    Anchor,
    PriorStabilityClosure,
    AdmissibilityPrecedent,
    RuntimeNeutrality,
    ControlPlaneProof,
    ControllerBoundary,
    BridgeFrontier,
    PolicyPrecedent,
    DesignInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceMachineIdentityBinding {
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub admissibility_report_id: String,
    pub admissibility_report_digest: String,
    pub world_mount_envelope_compiler_id: String,
    pub admissibility_contract_id: String,
    pub control_plane_equivalent_choice_relation_id: String,
    pub runtime_api_report_id: String,
    pub runtime_api_report_digest: String,
    pub logical_time_control_neutral: bool,
    pub wall_time_control_neutral: bool,
    pub cost_model_invariance_required: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleEquivalentChoiceSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceClassBindingRow {
    pub equivalent_choice_class_id: String,
    pub class_kind: TassadarPostArticleEquivalentChoiceClassKind,
    pub candidate_set_ids: Vec<String>,
    pub representative_case_ids: Vec<String>,
    pub bounded_candidate_count: u32,
    pub bounded_candidate_count_matches_admissibility: bool,
    pub contract_neutral_choice_required: bool,
    pub neutral_choice_auditable: bool,
    pub receipt_visible_justification_required: bool,
    pub route_and_mount_binding_required: bool,
    pub hidden_ordering_allowed: bool,
    pub latency_or_cost_discriminator_allowed: bool,
    pub soft_failure_discriminator_allowed: bool,
    pub typed_outcome_reason_ids: Vec<String>,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceCaseBindingRow {
    pub case_id: String,
    pub equivalent_choice_class_id: String,
    pub case_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_plugin_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_plugin_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_outcome_reason_id: Option<String>,
    pub filter_transform_receipt_ids: Vec<String>,
    pub receipt_visible_justification_present: bool,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceInvalidationRow {
    pub invalidation_id: String,
    pub present: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub transformer_anchor_contract_ref: String,
    pub plugin_world_mount_envelope_compiler_and_admissibility_report_ref: String,
    pub plugin_runtime_api_and_engine_abstraction_report_ref: String,
    pub control_plane_decision_provenance_proof_report_ref: String,
    pub fast_route_legitimacy_and_carrier_binding_contract_report_ref: String,
    pub weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report_ref: String,
    pub universality_bridge_contract_report_ref: String,
    pub world_mount_compatibility_report_ref: String,
    pub import_policy_matrix_report_ref: String,
    pub plugin_system_audit_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub equivalent_choice_contract:
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContract,
    pub machine_identity_binding: TassadarPostArticleEquivalentChoiceMachineIdentityBinding,
    pub supporting_material_rows: Vec<TassadarPostArticleEquivalentChoiceSupportingMaterialRow>,
    pub dependency_rows: Vec<TassadarPostArticleEquivalentChoiceDependencyRow>,
    pub equivalent_choice_class_rows: Vec<TassadarPostArticleEquivalentChoiceClassBindingRow>,
    pub case_binding_rows: Vec<TassadarPostArticleEquivalentChoiceCaseBindingRow>,
    pub invalidation_rows: Vec<TassadarPostArticleEquivalentChoiceInvalidationRow>,
    pub validation_rows: Vec<TassadarPostArticleEquivalentChoiceValidationRow>,
    pub contract_status: TassadarPostArticleEquivalentChoiceNeutralityStatus,
    pub contract_green: bool,
    pub equivalent_choice_neutrality_complete: bool,
    pub admissibility_narrowing_receipt_visible: bool,
    pub hidden_ordering_or_ranking_quarantined: bool,
    pub latency_cost_and_soft_failure_channels_blocked: bool,
    pub served_or_plugin_equivalence_overclaim_refused: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError {
    #[error(transparent)]
    Admissibility(
        #[from] TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReportError,
    ),
    #[error(transparent)]
    RuntimeApi(#[from] TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError),
    #[error(transparent)]
    ControlPlane(#[from] TassadarPostArticleControlPlaneDecisionProvenanceProofReportError),
    #[error(transparent)]
    FastRoute(
        #[from] TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError,
    ),
    #[error(transparent)]
    Controller(
        #[from] TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError,
    ),
    #[error(transparent)]
    Bridge(#[from] TassadarPostArticleUniversalityBridgeContractReportError),
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

pub fn build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report(
) -> Result<
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError,
> {
    let equivalent_choice_contract =
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract();
    let admissibility =
        build_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report()?;
    let runtime_api = build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report(
    )?;
    let control_plane = build_tassadar_post_article_control_plane_decision_provenance_proof_report(
    )?;
    let fast_route =
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report()?;
    let controller =
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report(
        )?;
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;

    Ok(build_report_from_inputs(
        equivalent_choice_contract,
        admissibility,
        runtime_api,
        control_plane,
        fast_route,
        controller,
        bridge,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    equivalent_choice_contract: TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContract,
    admissibility: TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReport,
    runtime_api: TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport,
    control_plane: TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    fast_route: TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    controller: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
    bridge: TassadarPostArticleUniversalityBridgeContractReport,
) -> TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport {
    let machine_identity_binding = TassadarPostArticleEquivalentChoiceMachineIdentityBinding {
        machine_identity_id: admissibility.machine_identity_binding.machine_identity_id.clone(),
        tuple_id: equivalent_choice_contract.tuple_id.clone(),
        canonical_model_id: admissibility.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: admissibility.machine_identity_binding.canonical_route_id.clone(),
        canonical_route_descriptor_digest: admissibility
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        admissibility_report_id: admissibility.report_id.clone(),
        admissibility_report_digest: admissibility.report_digest.clone(),
        world_mount_envelope_compiler_id: admissibility
            .machine_identity_binding
            .world_mount_envelope_compiler_id
            .clone(),
        admissibility_contract_id: admissibility
            .machine_identity_binding
            .admissibility_contract_id
            .clone(),
        control_plane_equivalent_choice_relation_id: control_plane
            .equivalent_choice_relation
            .relation_id
            .clone(),
        runtime_api_report_id: runtime_api.report_id.clone(),
        runtime_api_report_digest: runtime_api.report_digest.clone(),
        logical_time_control_neutral: runtime_api.logical_time_control_neutral,
        wall_time_control_neutral: runtime_api.wall_time_control_neutral,
        cost_model_invariance_required: runtime_api.cost_model_invariance_required,
        detail: format!(
            "machine_identity_id=`{}` canonical_route_id=`{}` admissibility_report_id=`{}` control_plane_equivalent_choice_relation_id=`{}` and runtime_api_report_id=`{}` remain bound together.",
            admissibility.machine_identity_binding.machine_identity_id,
            admissibility.machine_identity_binding.canonical_route_id,
            admissibility.report_id,
            control_plane.equivalent_choice_relation.relation_id,
            runtime_api.report_id,
        ),
    };

    let bridge_hook = bridge
        .reservation_hook_rows
        .iter()
        .find(|row| row.hook_id == "schema_version_negotiation_hook");
    let bridge_frontier_advanced = bridge_hook
        .map(|row| row.reserved_issue_ids.iter().any(|id| id == NEXT_STABILITY_ISSUE_ID))
        .unwrap_or(false);

    let supporting_material_rows = vec![
        supporting_material_row(
            "transformer_anchor_contract",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::Anchor,
            true,
            TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
            Some(equivalent_choice_contract.contract_id.clone()),
            Some(stable_digest(
                b"psionic_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract|",
                &equivalent_choice_contract,
            )),
            "the transformer-owned contract is the canonical abstract source for equivalent-choice neutrality and admissibility.",
        ),
        supporting_material_row(
            "fast_route_legitimacy_and_carrier_binding",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::PriorStabilityClosure,
            fast_route.contract_green && fast_route.fast_route_legitimacy_complete,
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
            Some(fast_route.report_id.clone()),
            Some(fast_route.report_digest.clone()),
            "fast-route legitimacy must already be closed so equivalent-choice neutrality lands on one canonical carrier classification.",
        ),
        supporting_material_row(
            "plugin_world_mount_admissibility",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::AdmissibilityPrecedent,
            admissibility.contract_green && admissibility.equivalent_choice_model_frozen,
            TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            Some(admissibility.report_id.clone()),
            Some(admissibility.report_digest.clone()),
            "the bounded admissibility report provides the explicit equivalent-choice classes and typed denied or suppressed outcomes this contract binds.",
        ),
        supporting_material_row(
            "plugin_runtime_api_and_engine_abstraction",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::RuntimeNeutrality,
            runtime_api.contract_green,
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
            Some(runtime_api.report_id.clone()),
            Some(runtime_api.report_digest.clone()),
            "the runtime API report keeps time, cost, queue, and scheduling signals hidden or fixed so they cannot become hidden admissibility discriminators.",
        ),
        supporting_material_row(
            "control_plane_decision_provenance_proof",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::ControlPlaneProof,
            control_plane.control_plane_ownership_green
                && control_plane.decision_provenance_proof_complete,
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            Some(control_plane.report_id.clone()),
            Some(control_plane.report_digest.clone()),
            "the control-plane proof keeps latency, cost, queue, scheduler, cache-hit, and helper-selection signals outside the model-visible choice boundary.",
        ),
        supporting_material_row(
            "weighted_plugin_controller_boundary",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::ControllerBoundary,
            controller.contract_green && controller.weighted_plugin_control_allowed,
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
            Some(controller.report_id.clone()),
            Some(controller.report_digest.clone()),
            "the weighted controller eval report keeps heuristic ranking, hidden top-k filtering, cached substitution, and candidate precomputation explicitly blocked.",
        ),
        supporting_material_row(
            "universality_bridge_frontier",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::BridgeFrontier,
            bridge.bridge_contract_green && bridge_frontier_advanced,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the bridge frontier must move past equivalent-choice neutrality so the next anti-drift issue is TAS-215 instead of leaving this contract implicit.",
        ),
        supporting_material_row(
            "world_mount_compatibility_precedent",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::PolicyPrecedent,
            true,
            WORLD_MOUNT_COMPATIBILITY_REPORT_REF,
            None,
            None,
            "world-mount compatibility remains an explicit supporting precedent for admissible candidate and envelope binding.",
        ),
        supporting_material_row(
            "import_policy_matrix_precedent",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::PolicyPrecedent,
            true,
            IMPORT_POLICY_MATRIX_REPORT_REF,
            None,
            None,
            "the import-policy matrix remains an explicit supporting precedent for admissibility narrowing and refusal posture.",
        ),
        supporting_material_row(
            "plugin_system_audit",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::DesignInput,
            true,
            PLUGIN_SYSTEM_AUDIT_REF,
            None,
            None,
            "the March 20 plugin-system audit is the public design input for this stability contract.",
        ),
        supporting_material_row(
            "local_plugin_system_spec",
            TassadarPostArticleEquivalentChoiceSupportingMaterialClass::DesignInput,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system spec remains the implementation-facing design input for equivalent-choice neutrality and admissibility.",
        ),
    ];

    let hidden_controller_channels_blocked = has_green_negative_rows(
        &controller,
        &[
            "heuristic_plugin_ranking",
            "candidate_precomputation",
            "hidden_topk_filtering",
            "cached_result_substitution",
        ],
    );
    let hidden_signals_closed = runtime_api.logical_time_control_neutral
        && runtime_api.wall_time_control_neutral
        && runtime_api.cost_model_invariance_required
        && runtime_api.scheduling_semantics_frozen
        && control_plane.equivalent_choice_relation.choice_neutrality_green
        && control_plane.equivalent_choice_relation.green
        && control_plane.information_boundary.green
        && contains_hidden_signals(
            &control_plane.information_boundary.model_hidden_signal_ids,
            &[
                "latency",
                "cost",
                "queue_pressure",
                "scheduler_order",
                "cache_hit_rate",
                "helper_selection",
            ],
        );
    let typed_soft_failure_behavior_preserved = admissibility.denial_behavior_frozen
        && admissibility
            .case_rows
            .iter()
            .filter(|case| case.status != TassadarPostArticlePluginAdmissibilityCaseStatus::ExactAdmittedEnvelope)
            .all(|case| case.denial_reason_id.is_some());

    let dependency_rows = vec![
        dependency_row(
            "fast_route_legitimacy_closed",
            fast_route.contract_green
                && fast_route.fast_route_legitimacy_complete
                && fast_route.next_stability_issue_id == NEXT_STABILITY_ISSUE_ID,
            &[
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
            ],
            "fast-route legitimacy must already be green and advance the anti-drift frontier to TAS-215.",
        ),
        dependency_row(
            "admissibility_contract_closed",
            admissibility.contract_green
                && admissibility.admissibility_frozen
                && admissibility.equivalent_choice_model_frozen
                && admissibility.receipt_visible_filtering_required,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            ],
            "the bounded admissibility report must already make candidate sets, equivalent-choice classes, and receipt-visible filtering explicit.",
        ),
        dependency_row(
            "runtime_signal_neutrality_closed",
            runtime_api.contract_green && hidden_signals_closed,
            &[TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF],
            "runtime time, cost, queue, and scheduling signals must already remain hidden or fixed instead of becoming choice discriminators.",
        ),
        dependency_row(
            "control_plane_choice_boundary_closed",
            control_plane.control_plane_ownership_green
                && control_plane.decision_provenance_proof_complete
                && control_plane.equivalent_choice_relation.choice_neutrality_green,
            &[TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF],
            "the control-plane proof must already keep equivalent-choice neutrality and hidden-channel closure explicit at the decision surface.",
        ),
        dependency_row(
            "controller_hidden_ranking_negatives_closed",
            controller.contract_green && hidden_controller_channels_blocked,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
            ],
            "the weighted controller report must already block heuristic ranking, hidden top-k filtering, candidate precomputation, and cached substitution.",
        ),
        dependency_row(
            "bridge_frontier_advanced_to_tas_213",
            bridge.bridge_contract_green && bridge_frontier_advanced,
            &[TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF],
            "the universality bridge must advance the reserved anti-drift frontier to TAS-215 once equivalent-choice neutrality is explicit.",
        ),
        dependency_row(
            "typed_soft_failure_posture_explicit",
            typed_soft_failure_behavior_preserved,
            &[TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF],
            "typed denied, suppressed, and quarantined outcomes must stay explicit so soft-failure effects do not become hidden steering channels.",
        ),
    ];

    let equivalent_choice_class_rows = equivalent_choice_contract
        .equivalent_choice_class_rows
        .iter()
        .map(|contract_row| {
            build_class_binding_row(contract_row, &admissibility)
        })
        .collect::<Vec<_>>();
    let case_binding_rows = admissibility
        .case_rows
        .iter()
        .map(build_case_binding_row)
        .collect::<Vec<_>>();

    let admissibility_narrowing_receipt_visible = equivalent_choice_class_rows
        .iter()
        .all(|row| row.receipt_visible_justification_required && row.green)
        && case_binding_rows
            .iter()
            .all(|row| row.receipt_visible_justification_present && row.green);
    let hidden_ordering_or_ranking_quarantined = hidden_controller_channels_blocked
        && admissibility
            .case_rows
            .iter()
            .any(|case| case.denial_reason_id.as_deref()
                == Some("equivalent_choice_neutrality_violation"));
    let latency_cost_and_soft_failure_channels_blocked =
        hidden_signals_closed && typed_soft_failure_behavior_preserved;
    let served_or_plugin_equivalence_overclaim_refused = fast_route
        .served_or_plugin_machine_overclaim_refused
        && !admissibility.plugin_publication_allowed
        && !admissibility.served_public_universality_allowed
        && !admissibility.arbitrary_software_capability_allowed
        && !bridge.served_public_universality_allowed;

    let invalidation_rows = vec![
        invalidation_row(
            "hidden_ordering_inside_equivalent_class",
            &[
                TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
            ],
            "hidden ordering or ranking inside an equivalent-choice class invalidates the contract.",
        ),
        invalidation_row(
            "latency_or_cost_steering_inside_equivalent_class",
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
                TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            ],
            "latency, cost, queue pressure, scheduler order, or cache-hit steering inside an equivalent-choice class invalidates the contract.",
        ),
        invalidation_row(
            "soft_failure_steering_inside_equivalent_class",
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            ],
            "soft-failure steering without typed denied, suppressed, or quarantined posture invalidates the contract.",
        ),
        invalidation_row(
            "unreceipted_admissibility_narrowing",
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            ],
            "admissibility narrowing without receipt-visible justification invalidates the contract.",
        ),
        invalidation_row(
            "route_or_mount_rebinding_without_binding",
            &[
                TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            ],
            "route or mount rebinding without explicit canonical-machine binding invalidates the contract.",
        ),
        invalidation_row(
            "served_or_plugin_overread",
            &[
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            ],
            "served or plugin overread from equivalent-choice neutrality into broader universality or publication claims invalidates the contract.",
        ),
        invalidation_row(
            "closure_bundle_overread",
            &[TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF],
            "the final closure bundle may not be inferred from equivalent-choice neutrality and admissibility alone.",
        ),
    ];

    let validation_rows = vec![
        validation_row(
            "machine_identity_binding_matches_admissibility",
            machine_identity_binding.machine_identity_id
                == equivalent_choice_contract.machine_identity_id
                && machine_identity_binding.machine_identity_id
                    == fast_route.machine_identity_binding.machine_identity_id
                && machine_identity_binding.machine_identity_id
                    == admissibility.machine_identity_binding.machine_identity_id,
            &[
                TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
            ],
            "the contract, admissibility report, and fast-route report all bind to the same canonical machine identity.",
        ),
        validation_row(
            "equivalent_choice_classes_match_admissibility_report",
            equivalent_choice_class_rows.iter().all(|row| row.green),
            &[
                TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            ],
            "every declared equivalent-choice class remains matched to the bounded admissibility report with explicit candidate-set and case bindings.",
        ),
        validation_row(
            "admissibility_narrowing_receipt_visible",
            admissibility_narrowing_receipt_visible,
            &[TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF],
            "every admitted or denied or suppressed or quarantined case remains justified by explicit receipts or typed reasons instead of hidden narrowing.",
        ),
        validation_row(
            "route_and_mount_binding_explicit",
            equivalent_choice_class_rows
                .iter()
                .all(|row| row.route_and_mount_binding_required && !row.candidate_set_ids.is_empty()),
            &[TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF],
            "route and mount binding remain explicit for every equivalent-choice class.",
        ),
        validation_row(
            "hidden_ordering_and_ranking_quarantined",
            hidden_ordering_or_ranking_quarantined,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            ],
            "hidden ordering, ranking, top-k filtering, precomputation, and cached substitution remain blocked and fail closed.",
        ),
        validation_row(
            "latency_cost_and_soft_failure_channels_blocked",
            latency_cost_and_soft_failure_channels_blocked,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
                TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            ],
            "latency, cost, queue, scheduler, cache-hit, and soft-failure effects remain hidden or typed instead of distinguishing equivalent admissible choices.",
        ),
        validation_row(
            "served_or_plugin_overclaim_refused",
            served_or_plugin_equivalence_overclaim_refused,
            &[
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            ],
            "equivalent-choice neutrality stays bounded and does not widen plugin publication, served/public universality, or arbitrary software capability claims.",
        ),
        validation_row(
            "bridge_frontier_advances_to_tas_213",
            bridge_frontier_advanced,
            &[TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF],
            "the bridge now reserves TAS-215 as the next anti-drift issue after equivalent-choice neutrality closes.",
        ),
    ];

    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && equivalent_choice_class_rows.iter().all(|row| row.green)
        && case_binding_rows.iter().all(|row| row.green)
        && validation_rows.iter().all(|row| row.green);
    let contract_status = if contract_green {
        TassadarPostArticleEquivalentChoiceNeutralityStatus::Green
    } else {
        TassadarPostArticleEquivalentChoiceNeutralityStatus::Blocked
    };

    let supporting_material_refs = vec![
        String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
        String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
        ),
        String::from(TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF),
        String::from(TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF),
        String::from(
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
        ),
        String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
        ),
        String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
        String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
        String::from(IMPORT_POLICY_MATRIX_REPORT_REF),
        String::from(PLUGIN_SYSTEM_AUDIT_REF),
        String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
    ];

    let mut report =
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport {
            schema_version: 1,
            report_id: String::from(
                "tassadar.post_article_equivalent_choice_neutrality_and_admissibility.report.v1",
            ),
            checker_script_ref: String::from(
                TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_CHECKER_REF,
            ),
            transformer_anchor_contract_ref: String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
            plugin_world_mount_envelope_compiler_and_admissibility_report_ref: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            ),
            plugin_runtime_api_and_engine_abstraction_report_ref: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
            ),
            control_plane_decision_provenance_proof_report_ref: String::from(
                TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            ),
            fast_route_legitimacy_and_carrier_binding_contract_report_ref: String::from(
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
            ),
            weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report_ref:
                String::from(
                    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
                ),
            universality_bridge_contract_report_ref: String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            ),
            world_mount_compatibility_report_ref: String::from(WORLD_MOUNT_COMPATIBILITY_REPORT_REF),
            import_policy_matrix_report_ref: String::from(IMPORT_POLICY_MATRIX_REPORT_REF),
            plugin_system_audit_ref: String::from(PLUGIN_SYSTEM_AUDIT_REF),
            local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            supporting_material_refs,
            equivalent_choice_contract,
            machine_identity_binding,
            supporting_material_rows,
            dependency_rows,
            equivalent_choice_class_rows,
            case_binding_rows,
            invalidation_rows,
            validation_rows,
            contract_status,
            contract_green,
            equivalent_choice_neutrality_complete: contract_green,
            admissibility_narrowing_receipt_visible,
            hidden_ordering_or_ranking_quarantined,
            latency_cost_and_soft_failure_channels_blocked,
            served_or_plugin_equivalence_overclaim_refused,
            next_stability_issue_id: String::from(NEXT_STABILITY_ISSUE_ID),
            closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
            claim_boundary: String::from(
                "this eval-owned contract report freezes equivalent-choice neutrality and admissibility on the canonical post-article machine. It binds bounded equivalent-choice classes, receipt-visible admissibility narrowing, typed denied or suppressed or quarantined outcomes, runtime signal neutrality, controller negative rows, and the bridge frontier to one explicit machine-readable boundary. It does not itself close downward non-influence, served conformance, anti-drift closeout, or the final closure bundle.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
    report.summary = format!(
        "Post-article equivalent-choice neutrality report keeps contract_status={:?}, dependency_rows={}, equivalent_choice_class_rows={}, case_binding_rows={}, validation_rows={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
        report.contract_status,
        report.dependency_rows.len(),
        report.equivalent_choice_class_rows.len(),
        report.case_binding_rows.len(),
        report.validation_rows.len(),
        report.next_stability_issue_id,
        report.closure_bundle_issue_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report|",
        &report,
    );
    report
}

fn build_class_binding_row(
    contract_row: &psionic_transformer::TassadarPostArticleEquivalentChoiceNeutralityClassRow,
    admissibility: &TassadarPostArticlePluginWorldMountEnvelopeCompilerAndAdmissibilityReport,
) -> TassadarPostArticleEquivalentChoiceClassBindingRow {
    let admissibility_row = admissibility
        .equivalent_choice_rows
        .iter()
        .find(|row| row.equivalent_choice_class_id == contract_row.equivalent_choice_class_id);
    let candidate_sets = admissibility
        .candidate_set_rows
        .iter()
        .filter(|row| row.equivalent_choice_class_id == contract_row.equivalent_choice_class_id)
        .collect::<Vec<_>>();
    let cases = admissibility
        .case_rows
        .iter()
        .filter(|row| row.equivalent_choice_class_id == contract_row.equivalent_choice_class_id)
        .collect::<Vec<_>>();
    let typed_outcome_reason_ids = cases
        .iter()
        .filter_map(|case| case.denial_reason_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let bounded_candidate_count_matches_admissibility = admissibility_row
        .map(|row| row.bounded_candidate_count == contract_row.bounded_candidate_count)
        .unwrap_or(false)
        && candidate_sets
            .iter()
            .all(|row| row.candidate_plugin_ids.len() as u32 == contract_row.bounded_candidate_count);
    let route_and_mount_binding_explicit = candidate_sets
        .iter()
        .all(|row| !row.route_policy_id.is_empty() && !row.world_mount_id.is_empty());
    let neutral_choice_auditable = admissibility_row
        .map(|row| row.neutral_choice_auditable)
        .unwrap_or(false);
    let receipt_visible_justification_required = admissibility_row
        .map(|row| row.receipt_visible_transforms_required)
        .unwrap_or(false);
    let hidden_ordering_allowed = admissibility_row
        .map(|row| row.hidden_ranking_allowed)
        .unwrap_or(true);
    let green = admissibility_row.map(|row| row.green).unwrap_or(false)
        && bounded_candidate_count_matches_admissibility
        && neutral_choice_auditable
        && receipt_visible_justification_required
        && !hidden_ordering_allowed
        && route_and_mount_binding_explicit
        && !cases.is_empty();

    TassadarPostArticleEquivalentChoiceClassBindingRow {
        equivalent_choice_class_id: contract_row.equivalent_choice_class_id.clone(),
        class_kind: contract_row.class_kind,
        candidate_set_ids: candidate_sets
            .iter()
            .map(|row| row.candidate_set_id.clone())
            .collect(),
        representative_case_ids: cases.iter().map(|row| row.case_id.clone()).collect(),
        bounded_candidate_count: contract_row.bounded_candidate_count,
        bounded_candidate_count_matches_admissibility,
        contract_neutral_choice_required: contract_row.neutral_choice_required,
        neutral_choice_auditable,
        receipt_visible_justification_required,
        route_and_mount_binding_required: contract_row.route_and_mount_binding_required
            && route_and_mount_binding_explicit,
        hidden_ordering_allowed,
        latency_or_cost_discriminator_allowed: contract_row.latency_or_cost_discriminator_allowed,
        soft_failure_discriminator_allowed: contract_row.soft_failure_discriminator_allowed,
        typed_outcome_reason_ids,
        green,
        source_refs: vec![
            String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF,
            ),
            String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
            String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        ],
        detail: format!(
            "equivalent_choice_class_id=`{}` keeps class_kind={:?}, candidate_sets={}, representative_cases={}, bounded_candidate_count_matches_admissibility={}, neutral_choice_auditable={}, receipt_visible_justification_required={}, and hidden_ordering_allowed={}.",
            contract_row.equivalent_choice_class_id,
            contract_row.class_kind,
            candidate_sets.len(),
            cases.len(),
            bounded_candidate_count_matches_admissibility,
            neutral_choice_auditable,
            receipt_visible_justification_required,
            hidden_ordering_allowed,
        ),
    }
}

fn build_case_binding_row(
    case: &psionic_sandbox::TassadarPostArticlePluginAdmissibilityCaseReportRow,
) -> TassadarPostArticleEquivalentChoiceCaseBindingRow {
    let receipt_visible_justification_present =
        !case.filter_transform_receipt_ids.is_empty() || case.denial_reason_id.is_some();
    let green = match case.status {
        TassadarPostArticlePluginAdmissibilityCaseStatus::ExactAdmittedEnvelope => {
            receipt_visible_justification_present
                && case.selected_plugin_id.is_some()
                && case.selected_plugin_version.is_some()
                && case.envelope_id.is_some()
        }
        TassadarPostArticlePluginAdmissibilityCaseStatus::ExactDeniedEnvelope
        | TassadarPostArticlePluginAdmissibilityCaseStatus::ExactSuppressedEnvelope
        | TassadarPostArticlePluginAdmissibilityCaseStatus::ExactQuarantinedEnvelope => {
            receipt_visible_justification_present && case.denial_reason_id.is_some()
        }
    };

    TassadarPostArticleEquivalentChoiceCaseBindingRow {
        case_id: case.case_id.clone(),
        equivalent_choice_class_id: case.equivalent_choice_class_id.clone(),
        case_status: case_status_label(case.status).to_string(),
        selected_plugin_id: case.selected_plugin_id.clone(),
        selected_plugin_version: case.selected_plugin_version.clone(),
        typed_outcome_reason_id: case.denial_reason_id.clone(),
        filter_transform_receipt_ids: case.filter_transform_receipt_ids.clone(),
        receipt_visible_justification_present,
        green,
        source_refs: case.source_refs.clone(),
        detail: case.detail.clone(),
    }
}

fn has_green_negative_rows(
    controller: &TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
    required_negative_class_ids: &[&str],
) -> bool {
    required_negative_class_ids.iter().all(|required| {
        controller.host_negative_rows.iter().any(|row| {
            row.negative_class_id == *required && row.green
        })
    })
}

fn contains_hidden_signals(hidden_signal_ids: &[String], required_signals: &[&str]) -> bool {
    required_signals
        .iter()
        .all(|required| hidden_signal_ids.iter().any(|signal| signal == required))
}

fn case_status_label(status: TassadarPostArticlePluginAdmissibilityCaseStatus) -> &'static str {
    match status {
        TassadarPostArticlePluginAdmissibilityCaseStatus::ExactAdmittedEnvelope => {
            "exact_admitted_envelope"
        }
        TassadarPostArticlePluginAdmissibilityCaseStatus::ExactDeniedEnvelope => {
            "exact_denied_envelope"
        }
        TassadarPostArticlePluginAdmissibilityCaseStatus::ExactSuppressedEnvelope => {
            "exact_suppressed_envelope"
        }
        TassadarPostArticlePluginAdmissibilityCaseStatus::ExactQuarantinedEnvelope => {
            "exact_quarantined_envelope"
        }
    }
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleEquivalentChoiceSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleEquivalentChoiceSupportingMaterialRow {
    TassadarPostArticleEquivalentChoiceSupportingMaterialRow {
        material_id: String::from(material_id),
        material_class,
        satisfied,
        source_ref: String::from(source_ref),
        source_artifact_id,
        source_artifact_digest,
        detail: String::from(detail),
    }
}

fn dependency_row(
    dependency_id: &str,
    satisfied: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticleEquivalentChoiceDependencyRow {
    TassadarPostArticleEquivalentChoiceDependencyRow {
        dependency_id: String::from(dependency_id),
        satisfied,
        source_refs: source_refs.iter().map(|item| String::from(*item)).collect(),
        detail: String::from(detail),
    }
}

fn invalidation_row(
    invalidation_id: &str,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticleEquivalentChoiceInvalidationRow {
    TassadarPostArticleEquivalentChoiceInvalidationRow {
        invalidation_id: String::from(invalidation_id),
        present: true,
        source_refs: source_refs.iter().map(|item| String::from(*item)).collect(),
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticleEquivalentChoiceValidationRow {
    TassadarPostArticleEquivalentChoiceValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs.iter().map(|item| String::from(*item)).collect(),
        detail: String::from(detail),
    }
}

#[must_use]
pub fn tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
    )
}

pub fn write_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report =
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report(
        )?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError::Write {
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
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
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
) -> Result<T, TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError>
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report,
        read_json,
        tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report_path,
        write_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report,
        TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
        TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
    };

    #[test]
    fn equivalent_choice_neutrality_report_keeps_choice_classes_explicit() {
        let report =
            build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report()
                .expect("report");

        assert_eq!(
            report.report_id,
            "tassadar.post_article_equivalent_choice_neutrality_and_admissibility.report.v1"
        );
        assert!(report.contract_green);
        assert!(report.equivalent_choice_neutrality_complete);
        assert!(report.admissibility_narrowing_receipt_visible);
        assert!(report.hidden_ordering_or_ranking_quarantined);
        assert!(report.latency_cost_and_soft_failure_channels_blocked);
        assert!(report.served_or_plugin_equivalence_overclaim_refused);
        assert_eq!(report.equivalent_choice_class_rows.len(), 5);
        assert!(
            report
                .equivalent_choice_class_rows
                .iter()
                .any(|row| row.equivalent_choice_class_id
                    == "choice.search_core_pair.closed_world_neutral.v1"
                    && row.bounded_candidate_count == 2
                    && row.neutral_choice_auditable
                    && !row.hidden_ordering_allowed)
        );
        assert_eq!(report.next_stability_issue_id, "TAS-215");
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
    }

    #[test]
    fn equivalent_choice_neutrality_report_matches_committed_truth() {
        let expected =
            build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report()
                .expect("expected");
        let committed: TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport =
            read_json(
                tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report_path(),
            )
            .expect("committed");

        assert_eq!(committed, expected);
        assert!(
            tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report_path()
                .ends_with(
                    "tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report.json"
                )
        );
    }

    #[test]
    fn write_equivalent_choice_neutrality_report_persists_truth() {
        let dir = tempfile::tempdir().expect("tempdir");
        let output_path = dir.path().join(
            "tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report.json",
        );

        let written =
            write_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report(
                &output_path,
            )
            .expect("written");
        let reread: TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport =
            read_json(&output_path).expect("reread");

        assert_eq!(written, reread);
        assert_eq!(
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report.json"
        );
    }
}
