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

use psionic_runtime::{
    build_tassadar_post_article_canonical_computational_model_statement_report,
    TassadarPostArticleCanonicalComputationalModelStatementReport,
    TassadarPostArticleCanonicalComputationalModelStatementReportError,
    TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
};
use psionic_transformer::{
    build_tassadar_post_article_downward_non_influence_and_served_conformance_contract,
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceContract,
    TassadarPostArticleLowerPlaneTruthKind,
};

use crate::{
    build_tassadar_post_article_canonical_machine_identity_lock_report,
    build_tassadar_post_article_continuation_non_computationality_contract_report,
    build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report,
    build_tassadar_post_article_execution_semantics_proof_transport_audit_report,
    build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report,
    build_tassadar_post_article_rebased_universality_verdict_split_report,
    build_tassadar_universality_verdict_split_report,
    TassadarPostArticleCanonicalMachineIdentityLockReport,
    TassadarPostArticleCanonicalMachineIdentityLockReportError,
    TassadarPostArticleContinuationNonComputationalityContractReport,
    TassadarPostArticleContinuationNonComputationalityContractReportError,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReportError,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError,
    TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    TassadarPostArticleRebasedUniversalityVerdictSplitReportError,
    TassadarUniversalityVerdictSplitReport, TassadarUniversalityVerdictSplitReportError,
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
    TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_downward_non_influence_and_served_conformance_report.json";
pub const TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_CHECKER_REF:
    &str =
    "scripts/check-tassadar-post-article-downward-non-influence-and-served-conformance.sh";

const TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_downward_non_influence_and_served_conformance_contract.rs";
const SERVED_CONFORMANCE_ENVELOPE_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_served_conformance_envelope.json";
const BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REF: &str =
    "fixtures/tassadar/reports/tassadar_broad_internal_compute_profile_publication_report.json";
const PLUGIN_SYSTEM_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const NEXT_STABILITY_ISSUE_ID: &str = "TAS-215";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleDownwardNonInfluenceStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass {
    Anchor,
    LowerPlaneTruth,
    ServedEnvelope,
    VerdictBoundary,
    HistoricalContext,
    AuditContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDownwardNonInfluenceMachineIdentityBinding {
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub direct_decode_mode: String,
    pub current_served_internal_compute_profile_id: String,
    pub computational_model_statement_id: String,
    pub continuation_contract_id: String,
    pub proof_transport_boundary_id: String,
    pub equivalent_choice_boundary_id: String,
    pub served_conformance_envelope_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDownwardNonInfluenceSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDownwardNonInfluenceDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleLowerPlaneBindingRow {
    pub truth_surface_id: String,
    pub truth_kind: TassadarPostArticleLowerPlaneTruthKind,
    pub source_artifact_id: String,
    pub source_artifact_digest: String,
    pub canonical_machine_bound: bool,
    pub canonical_route_bound: bool,
    pub plugin_or_served_rewrite_forbidden: bool,
    pub broader_claim_inheritance_allowed: bool,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleServedDeviationRow {
    pub deviation_id: String,
    pub explicit_in_contract: bool,
    pub explicit_in_served_envelope: bool,
    pub operator_truth_remains_broader: bool,
    pub fail_closed_if_widened: bool,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDownwardNonInfluenceInvalidationRow {
    pub invalidation_id: String,
    pub present: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDownwardNonInfluenceValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub transformer_anchor_contract_ref: String,
    pub canonical_computational_model_statement_report_ref: String,
    pub canonical_machine_identity_lock_report_ref: String,
    pub execution_semantics_proof_transport_audit_report_ref: String,
    pub continuation_non_computationality_contract_report_ref: String,
    pub fast_route_legitimacy_and_carrier_binding_contract_report_ref: String,
    pub equivalent_choice_neutrality_and_admissibility_contract_report_ref: String,
    pub rebased_universality_verdict_split_report_ref: String,
    pub historical_universality_verdict_split_report_ref: String,
    pub served_conformance_envelope_ref: String,
    pub broad_internal_compute_profile_publication_report_ref: String,
    pub plugin_system_audit_ref: String,
    pub post_article_turing_audit_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub downward_non_influence_contract:
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceContract,
    pub machine_identity_binding: TassadarPostArticleDownwardNonInfluenceMachineIdentityBinding,
    pub supporting_material_rows: Vec<TassadarPostArticleDownwardNonInfluenceSupportingMaterialRow>,
    pub dependency_rows: Vec<TassadarPostArticleDownwardNonInfluenceDependencyRow>,
    pub lower_plane_truth_rows: Vec<TassadarPostArticleLowerPlaneBindingRow>,
    pub served_deviation_rows: Vec<TassadarPostArticleServedDeviationRow>,
    pub invalidation_rows: Vec<TassadarPostArticleDownwardNonInfluenceInvalidationRow>,
    pub validation_rows: Vec<TassadarPostArticleDownwardNonInfluenceValidationRow>,
    pub contract_status: TassadarPostArticleDownwardNonInfluenceStatus,
    pub contract_green: bool,
    pub downward_non_influence_complete: bool,
    pub served_conformance_envelope_complete: bool,
    pub lower_plane_truth_rewrite_refused: bool,
    pub served_posture_narrower_than_operator_truth: bool,
    pub served_posture_fail_closed: bool,
    pub plugin_or_served_overclaim_refused: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError {
    #[error(transparent)]
    ComputationalModel(#[from] TassadarPostArticleCanonicalComputationalModelStatementReportError),
    #[error(transparent)]
    MachineLock(#[from] TassadarPostArticleCanonicalMachineIdentityLockReportError),
    #[error(transparent)]
    ProofTransport(#[from] TassadarPostArticleExecutionSemanticsProofTransportAuditReportError),
    #[error(transparent)]
    Continuation(#[from] TassadarPostArticleContinuationNonComputationalityContractReportError),
    #[error(transparent)]
    FastRoute(#[from] TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError),
    #[error(transparent)]
    EquivalentChoice(
        #[from] TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReportError,
    ),
    #[error(transparent)]
    RebasedVerdict(#[from] TassadarPostArticleRebasedUniversalityVerdictSplitReportError),
    #[error(transparent)]
    HistoricalVerdict(#[from] TassadarUniversalityVerdictSplitReportError),
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ServedConformanceEnvelopeInput {
    publication_id: String,
    current_served_internal_compute_profile_id: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    selected_decode_mode: String,
    allowed_narrower_deviation_ids: Vec<String>,
    required_identical_property_ids: Vec<String>,
    fail_closed_condition_ids: Vec<String>,
    served_suppression_boundary_preserved: bool,
    served_public_universality_allowed: bool,
    publication_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BroadInternalComputeProfilePublicationInput {
    report_id: String,
    current_served_profile_id: String,
    public_profile_claim_allowed: Option<bool>,
    operator_profile_claim_allowed: Option<bool>,
    route_policy_report_ref: Option<String>,
    report_digest: String,
}

pub fn build_tassadar_post_article_downward_non_influence_and_served_conformance_report(
) -> Result<
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport,
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError,
> {
    let contract =
        build_tassadar_post_article_downward_non_influence_and_served_conformance_contract();
    let computational_model =
        build_tassadar_post_article_canonical_computational_model_statement_report()?;
    let machine_lock = build_tassadar_post_article_canonical_machine_identity_lock_report()?;
    let proof_transport =
        build_tassadar_post_article_execution_semantics_proof_transport_audit_report()?;
    let continuation =
        build_tassadar_post_article_continuation_non_computationality_contract_report()?;
    let fast_route =
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report()?;
    let equivalent_choice =
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report(
        )?;
    let rebased_verdict = build_tassadar_post_article_rebased_universality_verdict_split_report()?;
    let historical_verdict = build_tassadar_universality_verdict_split_report()?;
    let served_envelope: ServedConformanceEnvelopeInput =
        read_artifact(SERVED_CONFORMANCE_ENVELOPE_REF)?;
    let broad_profile: BroadInternalComputeProfilePublicationInput =
        read_artifact(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REF)?;

    Ok(build_report_from_inputs(
        contract,
        computational_model,
        machine_lock,
        proof_transport,
        continuation,
        fast_route,
        equivalent_choice,
        rebased_verdict,
        historical_verdict,
        served_envelope,
        broad_profile,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    contract: TassadarPostArticleDownwardNonInfluenceAndServedConformanceContract,
    computational_model: TassadarPostArticleCanonicalComputationalModelStatementReport,
    machine_lock: TassadarPostArticleCanonicalMachineIdentityLockReport,
    proof_transport: TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    continuation: TassadarPostArticleContinuationNonComputationalityContractReport,
    fast_route: TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    equivalent_choice: TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContractReport,
    rebased_verdict: TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    historical_verdict: TassadarUniversalityVerdictSplitReport,
    served_envelope: ServedConformanceEnvelopeInput,
    broad_profile: BroadInternalComputeProfilePublicationInput,
) -> TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport {
    let machine_identity_consistent = computational_model.computational_model_statement.machine_identity_id
        == machine_lock.canonical_machine_tuple.machine_identity_id
        && computational_model.computational_model_statement.machine_identity_id
            == proof_transport.machine_identity_id
        && computational_model.computational_model_statement.machine_identity_id
            == continuation.machine_identity_binding.machine_identity_id
        && computational_model.computational_model_statement.machine_identity_id
            == fast_route.machine_identity_binding.machine_identity_id
        && computational_model.computational_model_statement.machine_identity_id
            == equivalent_choice.machine_identity_binding.machine_identity_id
        && computational_model.computational_model_statement.machine_identity_id
            == rebased_verdict.machine_identity_id;
    let canonical_route_consistent = computational_model.computational_model_statement.canonical_route_id
        == machine_lock.canonical_machine_tuple.canonical_route_id
        && computational_model.computational_model_statement.canonical_route_id
            == proof_transport.canonical_route_id
        && computational_model.computational_model_statement.canonical_route_id
            == continuation.machine_identity_binding.canonical_route_id
        && computational_model.computational_model_statement.canonical_route_id
            == fast_route.machine_identity_binding.canonical_route_id
        && computational_model.computational_model_statement.canonical_route_id
            == equivalent_choice.machine_identity_binding.canonical_route_id
        && computational_model.computational_model_statement.canonical_route_id
            == rebased_verdict.canonical_route_id
        && computational_model.computational_model_statement.canonical_route_id
            == served_envelope.canonical_route_id;
    let route_descriptor_consistent = computational_model
        .computational_model_statement
        .canonical_route_descriptor_digest
        == machine_lock
            .canonical_machine_tuple
            .canonical_route_descriptor_digest
        && computational_model
            .computational_model_statement
            .canonical_route_descriptor_digest
            == continuation
                .machine_identity_binding
                .canonical_route_descriptor_digest
        && computational_model
            .computational_model_statement
            .canonical_route_descriptor_digest
            == fast_route
                .machine_identity_binding
                .canonical_route_descriptor_digest
        && computational_model
            .computational_model_statement
            .canonical_route_descriptor_digest
            == equivalent_choice
                .machine_identity_binding
                .canonical_route_descriptor_digest
        && computational_model
            .computational_model_statement
            .canonical_route_descriptor_digest
            == served_envelope.canonical_route_descriptor_digest;
    let proof_boundary_consistent = proof_transport.transport_boundary.boundary_id
        == continuation.machine_identity_binding.proof_transport_boundary_id
        && proof_transport.transport_boundary.boundary_id
            == fast_route.machine_identity_binding.proof_transport_boundary_id;
    let selected_decode_mode_consistent = served_envelope.selected_decode_mode
        == computational_model.computational_model_statement.direct_decode_mode
        || served_envelope.selected_decode_mode
            == format!(
                "tassadar.decode.{}.v1",
                computational_model.computational_model_statement.direct_decode_mode
            );
    let served_profile_matches = broad_profile.current_served_profile_id
        == served_envelope.current_served_internal_compute_profile_id
        && rebased_verdict.current_served_internal_compute_profile_id
            == served_envelope.current_served_internal_compute_profile_id;
    let contract_allowed_deviations: BTreeSet<_> = contract
        .allowed_narrower_deviation_ids
        .iter()
        .cloned()
        .collect();
    let served_allowed_deviations: BTreeSet<_> = served_envelope
        .allowed_narrower_deviation_ids
        .iter()
        .cloned()
        .collect();
    let contract_fail_closed: BTreeSet<_> = contract
        .fail_closed_condition_ids
        .iter()
        .cloned()
        .collect();
    let served_fail_closed: BTreeSet<_> = served_envelope
        .fail_closed_condition_ids
        .iter()
        .cloned()
        .collect();
    let allowed_deviations_explicit = contract_allowed_deviations == served_allowed_deviations;
    let fail_closed_conditions_explicit = contract_fail_closed == served_fail_closed;
    let frontier_advances = contract.next_stability_issue_id == NEXT_STABILITY_ISSUE_ID
        && computational_model.next_stability_issue_id == NEXT_STABILITY_ISSUE_ID
        && proof_transport.next_stability_issue_id == NEXT_STABILITY_ISSUE_ID
        && continuation.next_stability_issue_id == NEXT_STABILITY_ISSUE_ID
        && fast_route.next_stability_issue_id == NEXT_STABILITY_ISSUE_ID
        && equivalent_choice.next_stability_issue_id == NEXT_STABILITY_ISSUE_ID;
    let served_posture_narrower_than_operator_truth = rebased_verdict.theory_green
        && rebased_verdict.operator_green
        && !rebased_verdict.served_green
        && historical_verdict.theory_green
        && historical_verdict.operator_green
        && !historical_verdict.served_green
        && served_envelope.served_suppression_boundary_preserved
        && !served_envelope.served_public_universality_allowed;
    let served_posture_fail_closed = fail_closed_conditions_explicit
        && served_envelope
            .required_identical_property_ids
            .contains(&String::from("canonical_route_id_matches_bridge_identity"))
        && served_envelope
            .required_identical_property_ids
            .contains(&String::from("canonical_route_descriptor_digest_matches_bridge_identity"))
        && served_envelope
            .required_identical_property_ids
            .contains(&String::from("selected_decode_mode_matches_direct_article_route"));
    let plugin_or_served_overclaim_refused = !rebased_verdict.plugin_capability_claim_allowed
        && !rebased_verdict.served_public_universality_allowed
        && !rebased_verdict.arbitrary_software_capability_allowed
        && !served_envelope.served_public_universality_allowed
        && !broad_profile.public_profile_claim_allowed.unwrap_or(false)
        && !historical_verdict.served_green;

    let machine_identity_binding = TassadarPostArticleDownwardNonInfluenceMachineIdentityBinding {
        machine_identity_id: machine_lock.canonical_machine_tuple.machine_identity_id.clone(),
        tuple_id: machine_lock.canonical_machine_tuple.tuple_id.clone(),
        canonical_model_id: machine_lock.canonical_machine_tuple.canonical_model_id.clone(),
        canonical_route_id: machine_lock.canonical_machine_tuple.canonical_route_id.clone(),
        canonical_route_descriptor_digest: machine_lock
            .canonical_machine_tuple
            .canonical_route_descriptor_digest
            .clone(),
        direct_decode_mode: computational_model
            .computational_model_statement
            .direct_decode_mode
            .clone(),
        current_served_internal_compute_profile_id: served_envelope
            .current_served_internal_compute_profile_id
            .clone(),
        computational_model_statement_id: computational_model
            .computational_model_statement
            .statement_id
            .clone(),
        continuation_contract_id: continuation
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        proof_transport_boundary_id: proof_transport.transport_boundary.boundary_id.clone(),
        equivalent_choice_boundary_id: equivalent_choice
            .equivalent_choice_contract
            .boundary_id
            .clone(),
        served_conformance_envelope_id: served_envelope.publication_id.clone(),
        detail: format!(
            "machine_identity_id=`{}` tuple_id=`{}` canonical_route_id=`{}` direct_decode_mode=`{}` served_profile_id=`{}` proof_transport_boundary_id=`{}`",
            machine_lock.canonical_machine_tuple.machine_identity_id,
            machine_lock.canonical_machine_tuple.tuple_id,
            machine_lock.canonical_machine_tuple.canonical_route_id,
            computational_model.computational_model_statement.direct_decode_mode,
            served_envelope.current_served_internal_compute_profile_id,
            proof_transport.transport_boundary.boundary_id,
        ),
    };

    let supporting_material_rows = vec![
        supporting_material_row(
            "transformer_anchor_contract",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::Anchor,
            true,
            TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
            Some(contract.contract_id.clone()),
            None,
            "the transformer-owned anchor freezes the lower-plane truth rows, allowed served deviations, blocked rewrite ids, and fail-closed conditions explicitly.",
        ),
        supporting_material_row(
            "canonical_computational_model_statement",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::LowerPlaneTruth,
            computational_model.statement_green,
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            Some(computational_model.report_id.clone()),
            Some(computational_model.report_digest.clone()),
            "the canonical computational-model statement names the lower-plane machine, direct route, continuation inheritance, and plugin-above-machine boundary.",
        ),
        supporting_material_row(
            "canonical_machine_identity_lock",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::LowerPlaneTruth,
            machine_lock.lock_green,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
            Some(machine_lock.report_id.clone()),
            Some(machine_lock.report_digest.clone()),
            "the machine lock binds one tuple and route descriptor digest that later served or plugin layers may not recompute.",
        ),
        supporting_material_row(
            "execution_semantics_proof_transport",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::LowerPlaneTruth,
            proof_transport.audit_green,
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            Some(proof_transport.report_id.clone()),
            Some(proof_transport.report_digest.clone()),
            "the proof-transport audit freezes which transition classes survive rebinding and blocks stronger-machine drift.",
        ),
        supporting_material_row(
            "continuation_non_computationality",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::LowerPlaneTruth,
            continuation.contract_green,
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
            Some(continuation.report_id.clone()),
            Some(continuation.report_digest.clone()),
            "the continuation contract keeps checkpoint, spill, session, and process surfaces transport-only on the same machine tuple.",
        ),
        supporting_material_row(
            "fast_route_legitimacy_and_carrier_binding",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::LowerPlaneTruth,
            fast_route.contract_green,
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
            Some(fast_route.report_id.clone()),
            Some(fast_route.report_digest.clone()),
            "the fast-route contract keeps only the declared direct carrier inside the machine while other families remain narrower or outside carrier.",
        ),
        supporting_material_row(
            "equivalent_choice_neutrality_and_admissibility",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::LowerPlaneTruth,
            equivalent_choice.contract_green,
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
            Some(equivalent_choice.report_id.clone()),
            Some(equivalent_choice.report_digest.clone()),
            "equivalent-choice neutrality prevents later layers from laundering hidden steering back into the lower plane.",
        ),
        supporting_material_row(
            "served_conformance_envelope",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::ServedEnvelope,
            served_envelope.served_suppression_boundary_preserved
                && !served_envelope.served_public_universality_allowed,
            SERVED_CONFORMANCE_ENVELOPE_REF,
            Some(served_envelope.publication_id.clone()),
            Some(served_envelope.publication_digest.clone()),
            "the served conformance envelope names the only allowed narrower deviations and the fail-closed widening conditions for served posture.",
        ),
        supporting_material_row(
            "rebased_universality_verdict_split",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::VerdictBoundary,
            rebased_verdict.verdict_split_green,
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            Some(rebased_verdict.report_id.clone()),
            Some(rebased_verdict.report_digest.clone()),
            "the rebased verdict split keeps theory/operator green while served remains suppressed and public or plugin widening remains blocked.",
        ),
        supporting_material_row(
            "historical_universality_verdict_split",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::HistoricalContext,
            historical_verdict.overall_green && !historical_verdict.served_green,
            TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            Some(historical_verdict.report_id.clone()),
            Some(historical_verdict.report_digest.clone()),
            "the historical verdict split remains the earlier theory/operator green, served suppressed boundary that later served posture must not overread.",
        ),
        supporting_material_row(
            "broad_internal_compute_profile_publication",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::ServedEnvelope,
            broad_profile.current_served_profile_id
                == served_envelope.current_served_internal_compute_profile_id,
            BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REF,
            Some(broad_profile.report_id.clone()),
            Some(broad_profile.report_digest.clone()),
            "the broad internal-compute profile publication still names the article-closeout served profile instead of a stronger served universality profile.",
        ),
        supporting_material_row(
            "plugin_system_turing_audit",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::AuditContext,
            true,
            PLUGIN_SYSTEM_AUDIT_REF,
            None,
            None,
            "the plugin-system audit is retained as supporting context for the downward non-influence and served-envelope boundary.",
        ),
        supporting_material_row(
            "post_article_turing_audit",
            TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass::AuditContext,
            true,
            POST_ARTICLE_TURING_AUDIT_REF,
            None,
            None,
            "the post-article Turing-completeness audit is retained as supporting context for the post-article machine and served suppression posture.",
        ),
    ];

    let dependency_rows = vec![
        dependency_row(
            "canonical_machine_identity_consistent",
            machine_identity_consistent,
            [
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            ],
            "all lower-plane truth artifacts and the rebased verdict split must cite one canonical machine identity.",
        ),
        dependency_row(
            "canonical_route_identity_consistent",
            canonical_route_consistent && route_descriptor_consistent,
            [
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
            ],
            "all lower-plane truth artifacts plus the served envelope must keep the same canonical route id and route descriptor digest.",
        ),
        dependency_row(
            "proof_transport_boundary_consistent",
            proof_boundary_consistent,
            [
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
            ],
            "continuation and fast-route posture must inherit the same proof-transport boundary instead of redefining execution semantics later.",
        ),
        dependency_row(
            "served_allowed_deviations_explicit",
            allowed_deviations_explicit,
            [
                TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
            ],
            "the served envelope must list exactly the allowed narrower deviations declared by the anchor contract.",
        ),
        dependency_row(
            "served_fail_closed_conditions_explicit",
            fail_closed_conditions_explicit,
            [
                TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
            ],
            "the served envelope must list exactly the fail-closed widening conditions declared by the anchor contract.",
        ),
        dependency_row(
            "served_profile_matches_publication",
            served_profile_matches,
            [
                BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
                TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            ],
            "the served profile named by the publication, served envelope, and rebased verdict split must stay the same narrower article-closeout profile.",
        ),
        dependency_row(
            "served_suppression_and_overclaim_blocked",
            served_posture_narrower_than_operator_truth && plugin_or_served_overclaim_refused,
            [
                TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
                BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REF,
            ],
            "historical and rebased verdict surfaces must keep served/public universality and plugin capability blocked while served posture stays narrower than operator truth.",
        ),
        dependency_row(
            "frontier_advances_to_tas_214",
            frontier_advances,
            [
                TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
            ],
            "closing this issue must move the lower-plane anti-drift frontier to TAS-215 rather than leaving TAS-213 open implicitly.",
        ),
    ];

    let lower_plane_truth_rows = vec![
        lower_plane_binding_row(
            "canonical_computational_model_statement",
            TassadarPostArticleLowerPlaneTruthKind::CanonicalComputationalModel,
            computational_model.report_id.clone(),
            computational_model.report_digest.clone(),
            machine_identity_consistent,
            canonical_route_consistent && route_descriptor_consistent,
            true,
            false,
            computational_model.statement_green,
            [TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF],
            "the published computational-model statement is a lower-plane truth row and remains canonically bound.",
        ),
        lower_plane_binding_row(
            "canonical_machine_identity_lock",
            TassadarPostArticleLowerPlaneTruthKind::CanonicalMachineIdentityLock,
            machine_lock.report_id.clone(),
            machine_lock.report_digest.clone(),
            machine_identity_consistent,
            canonical_route_consistent && route_descriptor_consistent,
            true,
            false,
            machine_lock.lock_green,
            [TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF],
            "the machine lock keeps tuple, route, and continuation identity frozen against later rewrite.",
        ),
        lower_plane_binding_row(
            "execution_semantics_proof_transport",
            TassadarPostArticleLowerPlaneTruthKind::ExecutionSemanticsProofTransport,
            proof_transport.report_id.clone(),
            proof_transport.report_digest.clone(),
            machine_identity_consistent,
            canonical_route_consistent,
            true,
            false,
            proof_transport.audit_green && proof_boundary_consistent,
            [TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF],
            "the proof-transport boundary remains part of lower-plane truth and stays bound to the canonical machine rather than a later plugin or served layer.",
        ),
        lower_plane_binding_row(
            "continuation_non_computationality",
            TassadarPostArticleLowerPlaneTruthKind::ContinuationBoundary,
            continuation.report_id.clone(),
            continuation.report_digest.clone(),
            machine_identity_consistent,
            canonical_route_consistent,
            true,
            false,
            continuation.contract_green && proof_boundary_consistent,
            [TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF],
            "continuation remains transport-only on the canonical machine and may not be widened into hidden compute by later layers.",
        ),
        lower_plane_binding_row(
            "fast_route_legitimacy_and_carrier_binding",
            TassadarPostArticleLowerPlaneTruthKind::FastRouteCarrierBinding,
            fast_route.report_id.clone(),
            fast_route.report_digest.clone(),
            machine_identity_consistent,
            canonical_route_consistent && route_descriptor_consistent,
            true,
            false,
            fast_route.contract_green,
            [TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF],
            "fast-route carrier identity remains frozen in the lower plane and is not widened by served or plugin posture.",
        ),
        lower_plane_binding_row(
            "equivalent_choice_neutrality_and_admissibility",
            TassadarPostArticleLowerPlaneTruthKind::EquivalentChoiceBoundary,
            equivalent_choice.report_id.clone(),
            equivalent_choice.report_digest.clone(),
            machine_identity_consistent,
            canonical_route_consistent && route_descriptor_consistent,
            true,
            false,
            equivalent_choice.contract_green,
            [TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF],
            "equivalent-choice and admissibility neutrality stay lower-plane truth instead of becoming a host-side steering channel.",
        ),
    ];

    let served_deviation_rows = contract
        .allowed_narrower_deviation_ids
        .iter()
        .map(|deviation_id| {
            let explicit_in_served_envelope = served_envelope
                .allowed_narrower_deviation_ids
                .iter()
                .any(|id| id == deviation_id);
            TassadarPostArticleServedDeviationRow {
                deviation_id: deviation_id.clone(),
                explicit_in_contract: true,
                explicit_in_served_envelope,
                operator_truth_remains_broader: served_posture_narrower_than_operator_truth,
                fail_closed_if_widened: served_posture_fail_closed,
                green: explicit_in_served_envelope
                    && served_posture_narrower_than_operator_truth
                    && served_posture_fail_closed,
                source_refs: vec![
                    String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
                    String::from(SERVED_CONFORMANCE_ENVELOPE_REF),
                    String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
                ],
                detail: format!(
                    "served deviation `{}` is allowed only as an explicit narrower-than-operator posture inside the declared served conformance envelope.",
                    deviation_id
                ),
            }
        })
        .collect::<Vec<_>>();

    let lower_plane_truth_rewrite_refused = lower_plane_truth_rows.iter().all(|row| row.green);
    let downward_non_influence_complete =
        lower_plane_truth_rewrite_refused && dependency_rows.iter().all(|row| row.satisfied);
    let served_conformance_envelope_complete = served_deviation_rows.iter().all(|row| row.green)
        && served_posture_narrower_than_operator_truth
        && served_posture_fail_closed;

    let invalidation_rows = contract
        .invalidation_rule_rows
        .iter()
        .map(|row| TassadarPostArticleDownwardNonInfluenceInvalidationRow {
            invalidation_id: row.rule_id.clone(),
            present: true,
            source_refs: vec![
                String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
                String::from(SERVED_CONFORMANCE_ENVELOPE_REF),
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            ],
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();

    let validation_rows = vec![
        validation_row(
            "lower_plane_truth_rows_all_green",
            lower_plane_truth_rewrite_refused,
            [
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
            ],
            "every declared lower-plane truth row must stay green on the same canonical machine.",
        ),
        validation_row(
            "canonical_route_and_decode_mode_bound",
            canonical_route_consistent
                && route_descriptor_consistent
                && selected_decode_mode_consistent,
            [
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
            ],
            "the served envelope must keep the same route id, route descriptor digest, and direct decode mode as lower-plane truth.",
        ),
        validation_row(
            "served_allowed_deviations_explicit",
            allowed_deviations_explicit,
            [
                TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
            ],
            "the served envelope must enumerate exactly the allowed narrower deviation ids declared by the anchor contract.",
        ),
        validation_row(
            "served_fail_closed_conditions_explicit",
            served_posture_fail_closed,
            [
                TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
            ],
            "the served envelope must enumerate the declared fail-closed conditions and keep the canonical-route identity properties explicit.",
        ),
        validation_row(
            "served_posture_remains_narrower_than_operator_truth",
            served_posture_narrower_than_operator_truth,
            [
                TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
            ],
            "served posture must remain explicitly narrower than operator truth rather than silently inheriting operator-green universality.",
        ),
        validation_row(
            "plugin_and_public_overclaim_blocked",
            plugin_or_served_overclaim_refused,
            [
                TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
                BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REF,
            ],
            "plugin capability, served/public universality, and arbitrary software capability must remain blocked from this narrower served posture.",
        ),
        validation_row(
            "served_profile_matches_publication",
            served_profile_matches,
            [
                BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REF,
                SERVED_CONFORMANCE_ENVELOPE_REF,
                TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            ],
            "the current served profile id must remain the same narrower article-closeout profile across publication, envelope, and rebased verdict split surfaces.",
        ),
        validation_row(
            "anti_drift_frontier_moves_to_tas_214",
            frontier_advances,
            [
                TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
            ],
            "closing downward non-influence and served conformance must hand the frontier to TAS-215.",
        ),
    ];

    let contract_green = supporting_material_rows.iter().all(|row| row.satisfied)
        && dependency_rows.iter().all(|row| row.satisfied)
        && lower_plane_truth_rewrite_refused
        && served_conformance_envelope_complete
        && validation_rows.iter().all(|row| row.green);
    let contract_status = if contract_green {
        TassadarPostArticleDownwardNonInfluenceStatus::Green
    } else {
        TassadarPostArticleDownwardNonInfluenceStatus::Blocked
    };

    let mut report = TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_downward_non_influence_and_served_conformance.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_CHECKER_REF,
        ),
        transformer_anchor_contract_ref: String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
        canonical_computational_model_statement_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
        ),
        canonical_machine_identity_lock_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
        ),
        execution_semantics_proof_transport_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
        ),
        continuation_non_computationality_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
        ),
        fast_route_legitimacy_and_carrier_binding_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
        ),
        equivalent_choice_neutrality_and_admissibility_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF,
        ),
        rebased_universality_verdict_split_report_ref: String::from(
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
        ),
        historical_universality_verdict_split_report_ref: String::from(
            TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
        ),
        served_conformance_envelope_ref: String::from(SERVED_CONFORMANCE_ENVELOPE_REF),
        broad_internal_compute_profile_publication_report_ref: String::from(
            BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REF,
        ),
        plugin_system_audit_ref: String::from(PLUGIN_SYSTEM_AUDIT_REF),
        post_article_turing_audit_ref: String::from(POST_ARTICLE_TURING_AUDIT_REF),
        supporting_material_refs: vec![
            String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
            String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
            String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
            String::from(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF),
            String::from(TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF),
            String::from(TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_REPORT_REF),
            String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            String::from(SERVED_CONFORMANCE_ENVELOPE_REF),
            String::from(BROAD_INTERNAL_COMPUTE_PROFILE_PUBLICATION_REF),
            String::from(PLUGIN_SYSTEM_AUDIT_REF),
            String::from(POST_ARTICLE_TURING_AUDIT_REF),
        ],
        downward_non_influence_contract: contract,
        machine_identity_binding,
        supporting_material_rows,
        dependency_rows,
        lower_plane_truth_rows,
        served_deviation_rows,
        invalidation_rows,
        validation_rows,
        contract_status,
        contract_green,
        downward_non_influence_complete,
        served_conformance_envelope_complete,
        lower_plane_truth_rewrite_refused,
        served_posture_narrower_than_operator_truth,
        served_posture_fail_closed,
        plugin_or_served_overclaim_refused,
        next_stability_issue_id: String::from(NEXT_STABILITY_ISSUE_ID),
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        claim_boundary: String::from(
            "this eval-owned artifact closes only the downward non-influence and served conformance envelope boundary for the canonical post-article machine. It makes lower-plane truth rows, served narrowing allowances, fail-closed widening conditions, and blocked overclaim posture machine-checkable. It does not itself close anti-drift publication, the canonical machine closure bundle, plugin publication, served/public universality, or arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article downward non-influence and served conformance report keeps contract_status={:?}, lower_plane_truth_rows={}, served_deviation_rows={}, validation_rows={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
        report.contract_status,
        report.lower_plane_truth_rows.len(),
        report.served_deviation_rows.len(),
        report.validation_rows.len(),
        report.next_stability_issue_id,
        report.closure_bundle_issue_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_downward_non_influence_and_served_conformance_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_post_article_downward_non_influence_and_served_conformance_report_path(
) -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF)
}

pub fn write_tassadar_post_article_downward_non_influence_and_served_conformance_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport,
    TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_downward_non_influence_and_served_conformance_report(
    )?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError::Write {
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

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleDownwardNonInfluenceSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleDownwardNonInfluenceSupportingMaterialRow {
    TassadarPostArticleDownwardNonInfluenceSupportingMaterialRow {
        material_id: String::from(material_id),
        material_class,
        satisfied,
        source_ref: String::from(source_ref),
        source_artifact_id,
        source_artifact_digest,
        detail: String::from(detail),
    }
}

fn dependency_row<const N: usize>(
    dependency_id: &str,
    satisfied: bool,
    source_refs: [&str; N],
    detail: &str,
) -> TassadarPostArticleDownwardNonInfluenceDependencyRow {
    TassadarPostArticleDownwardNonInfluenceDependencyRow {
        dependency_id: String::from(dependency_id),
        satisfied,
        source_refs: source_refs.into_iter().map(String::from).collect(),
        detail: String::from(detail),
    }
}

fn lower_plane_binding_row<const N: usize>(
    truth_surface_id: &str,
    truth_kind: TassadarPostArticleLowerPlaneTruthKind,
    source_artifact_id: String,
    source_artifact_digest: String,
    canonical_machine_bound: bool,
    canonical_route_bound: bool,
    plugin_or_served_rewrite_forbidden: bool,
    broader_claim_inheritance_allowed: bool,
    green: bool,
    source_refs: [&str; N],
    detail: &str,
) -> TassadarPostArticleLowerPlaneBindingRow {
    TassadarPostArticleLowerPlaneBindingRow {
        truth_surface_id: String::from(truth_surface_id),
        truth_kind,
        source_artifact_id,
        source_artifact_digest,
        canonical_machine_bound,
        canonical_route_bound,
        plugin_or_served_rewrite_forbidden,
        broader_claim_inheritance_allowed,
        green,
        source_refs: source_refs.into_iter().map(String::from).collect(),
        detail: String::from(detail),
    }
}

fn validation_row<const N: usize>(
    validation_id: &str,
    green: bool,
    source_refs: [&str; N],
    detail: &str,
) -> TassadarPostArticleDownwardNonInfluenceValidationRow {
    TassadarPostArticleDownwardNonInfluenceValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs.into_iter().map(String::from).collect(),
        detail: String::from(detail),
    }
}

fn read_artifact<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_report_from_inputs,
        build_tassadar_post_article_downward_non_influence_and_served_conformance_report,
        read_artifact, read_json,
        tassadar_post_article_downward_non_influence_and_served_conformance_report_path,
        write_tassadar_post_article_downward_non_influence_and_served_conformance_report,
        BroadInternalComputeProfilePublicationInput, ServedConformanceEnvelopeInput,
        TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport,
        TassadarPostArticleDownwardNonInfluenceStatus,
        TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF,
    };
    use crate::{
        build_tassadar_post_article_canonical_machine_identity_lock_report,
        build_tassadar_post_article_continuation_non_computationality_contract_report,
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report,
        build_tassadar_post_article_execution_semantics_proof_transport_audit_report,
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report,
        build_tassadar_post_article_rebased_universality_verdict_split_report,
        build_tassadar_universality_verdict_split_report,
    };
    use psionic_runtime::build_tassadar_post_article_canonical_computational_model_statement_report;
    use psionic_transformer::build_tassadar_post_article_downward_non_influence_and_served_conformance_contract;

    #[test]
    fn downward_non_influence_and_served_conformance_report_keeps_boundary_green() {
        let report =
            build_tassadar_post_article_downward_non_influence_and_served_conformance_report()
                .expect("report");

        assert_eq!(
            report.report_id,
            "tassadar.post_article_downward_non_influence_and_served_conformance.report.v1"
        );
        assert_eq!(
            report.contract_status,
            TassadarPostArticleDownwardNonInfluenceStatus::Green
        );
        assert!(report.contract_green);
        assert!(report.downward_non_influence_complete);
        assert!(report.served_conformance_envelope_complete);
        assert!(report.lower_plane_truth_rewrite_refused);
        assert!(report.served_posture_narrower_than_operator_truth);
        assert!(report.served_posture_fail_closed);
        assert!(report.plugin_or_served_overclaim_refused);
        assert_eq!(report.lower_plane_truth_rows.len(), 6);
        assert_eq!(report.served_deviation_rows.len(), 3);
        assert_eq!(report.next_stability_issue_id, "TAS-215");
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
    }

    #[test]
    fn downward_non_influence_and_served_conformance_report_blocks_missing_served_deviation() {
        let contract =
            build_tassadar_post_article_downward_non_influence_and_served_conformance_contract();
        let computational_model =
            build_tassadar_post_article_canonical_computational_model_statement_report()
                .expect("computational_model");
        let machine_lock =
            build_tassadar_post_article_canonical_machine_identity_lock_report().expect(
                "machine_lock",
            );
        let proof_transport =
            build_tassadar_post_article_execution_semantics_proof_transport_audit_report()
                .expect("proof_transport");
        let continuation =
            build_tassadar_post_article_continuation_non_computationality_contract_report()
                .expect("continuation");
        let fast_route =
            build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report()
                .expect("fast_route");
        let equivalent_choice =
            build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report()
                .expect("equivalent_choice");
        let rebased_verdict =
            build_tassadar_post_article_rebased_universality_verdict_split_report()
                .expect("rebased_verdict");
        let historical_verdict =
            build_tassadar_universality_verdict_split_report().expect("historical_verdict");
        let mut served_envelope: ServedConformanceEnvelopeInput =
            read_artifact(
                "fixtures/tassadar/reports/tassadar_post_article_universality_served_conformance_envelope.json",
            )
            .expect("served_envelope");
        served_envelope.allowed_narrower_deviation_ids.pop();
        let broad_profile: BroadInternalComputeProfilePublicationInput = read_artifact(
            "fixtures/tassadar/reports/tassadar_broad_internal_compute_profile_publication_report.json",
        )
        .expect("broad_profile");

        let report = build_report_from_inputs(
            contract,
            computational_model,
            machine_lock,
            proof_transport,
            continuation,
            fast_route,
            equivalent_choice,
            rebased_verdict,
            historical_verdict,
            served_envelope,
            broad_profile,
        );

        assert_eq!(
            report.contract_status,
            TassadarPostArticleDownwardNonInfluenceStatus::Blocked
        );
        assert!(!report.contract_green);
        assert!(!report.served_conformance_envelope_complete);
        assert!(report
            .validation_rows
            .iter()
            .any(|row| row.validation_id == "served_allowed_deviations_explicit" && !row.green));
    }

    #[test]
    fn downward_non_influence_and_served_conformance_report_matches_committed_truth() {
        let expected =
            build_tassadar_post_article_downward_non_influence_and_served_conformance_report()
                .expect("expected");
        let committed: TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport =
            read_json(
                tassadar_post_article_downward_non_influence_and_served_conformance_report_path(),
            )
            .expect("committed");

        assert_eq!(committed, expected);
        assert!(
            tassadar_post_article_downward_non_influence_and_served_conformance_report_path()
                .ends_with(
                    "tassadar_post_article_downward_non_influence_and_served_conformance_report.json"
                )
        );
    }

    #[test]
    fn write_downward_non_influence_and_served_conformance_report_persists_truth() {
        let dir = tempfile::tempdir().expect("tempdir");
        let output_path = dir
            .path()
            .join("tassadar_post_article_downward_non_influence_and_served_conformance_report.json");
        let written =
            write_tassadar_post_article_downward_non_influence_and_served_conformance_report(
                &output_path,
            )
            .expect("written");
        let reread: TassadarPostArticleDownwardNonInfluenceAndServedConformanceReport =
            read_json(&output_path).expect("reread");

        assert_eq!(written, reread);
        assert_eq!(
            tassadar_post_article_downward_non_influence_and_served_conformance_report_path()
                .strip_prefix(super::repo_root())
                .expect("relative")
                .to_string_lossy(),
            TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_REPORT_REF
        );
    }
}
