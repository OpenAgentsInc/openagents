use std::{
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
    build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract,
    TassadarPostArticleFastRouteCarrierRelation,
    TassadarPostArticleFastRouteFamilyClass,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContract,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_fast_route_architecture_selection_report,
    build_tassadar_article_fast_route_implementation_report,
    build_tassadar_post_article_canonical_machine_identity_lock_report,
    build_tassadar_post_article_continuation_non_computationality_contract_report,
    build_tassadar_post_article_execution_semantics_proof_transport_audit_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleFastRouteArchitectureSelectionError,
    TassadarArticleFastRouteArchitectureSelectionReport,
    TassadarArticleFastRouteCandidateKind,
    TassadarArticleFastRouteImplementationReport,
    TassadarArticleFastRouteImplementationReportError,
    TassadarPostArticleCanonicalMachineIdentityLockReport,
    TassadarPostArticleCanonicalMachineIdentityLockReportError,
    TassadarPostArticleContinuationNonComputationalityContractReport,
    TassadarPostArticleContinuationNonComputationalityContractReportError,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReportError,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report.json";
pub const TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_CHECKER_REF:
    &str =
    "scripts/check-tassadar-post-article-fast-route-legitimacy-and-carrier-binding-contract.sh";

const TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract.rs";
const NEXT_STABILITY_ISSUE_ID: &str = "TAS-215";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleFastRouteLegitimacyStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleFastRouteSupportingMaterialClass {
    Anchor,
    ArticleEvidence,
    MachineBinding,
    ContinuationBoundary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteMachineIdentityBinding {
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_fast_decode_mode: String,
    pub reference_linear_proof_route_descriptor_digest: String,
    pub selected_hull_cache_route_descriptor_digest: String,
    pub continuation_contract_id: String,
    pub computational_model_statement_id: String,
    pub proof_transport_boundary_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleFastRouteSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteCarrierClassificationRow {
    pub route_family_id: String,
    pub route_family_class: TassadarPostArticleFastRouteFamilyClass,
    pub carrier_relation: TassadarPostArticleFastRouteCarrierRelation,
    pub source_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_route_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_route_descriptor_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_machine_identity_id: Option<String>,
    pub proof_inheritance_allowed: bool,
    pub universality_inheritance_allowed: bool,
    pub served_or_plugin_machine_claim_allowed: bool,
    pub semantics_equivalence_evidence_refs: Vec<String>,
    pub route_family_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteInvalidationRow {
    pub invalidation_id: String,
    pub present: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub transformer_anchor_contract_ref: String,
    pub article_fast_route_architecture_selection_report_ref: String,
    pub article_fast_route_implementation_report_ref: String,
    pub article_equivalence_acceptance_gate_report_ref: String,
    pub canonical_machine_identity_lock_report_ref: String,
    pub canonical_computational_model_statement_report_ref: String,
    pub execution_semantics_proof_transport_audit_report_ref: String,
    pub continuation_non_computationality_contract_report_ref: String,
    pub universality_bridge_contract_report_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub fast_route_contract: TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContract,
    pub machine_identity_binding: TassadarPostArticleFastRouteMachineIdentityBinding,
    pub supporting_material_rows: Vec<TassadarPostArticleFastRouteSupportingMaterialRow>,
    pub dependency_rows: Vec<TassadarPostArticleFastRouteDependencyRow>,
    pub route_family_rows: Vec<TassadarPostArticleFastRouteCarrierClassificationRow>,
    pub invalidation_rows: Vec<TassadarPostArticleFastRouteInvalidationRow>,
    pub validation_rows: Vec<TassadarPostArticleFastRouteValidationRow>,
    pub contract_status: TassadarPostArticleFastRouteLegitimacyStatus,
    pub contract_green: bool,
    pub carrier_binding_complete: bool,
    pub unproven_fast_routes_quarantined: bool,
    pub resumable_family_not_presented_as_direct_machine: bool,
    pub served_or_plugin_machine_overclaim_refused: bool,
    pub fast_route_legitimacy_complete: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError {
    #[error(transparent)]
    ArchitectureSelection(#[from] TassadarArticleFastRouteArchitectureSelectionError),
    #[error(transparent)]
    FastRouteImplementation(#[from] TassadarArticleFastRouteImplementationReportError),
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    MachineLock(#[from] TassadarPostArticleCanonicalMachineIdentityLockReportError),
    #[error(transparent)]
    ComputationalModel(#[from] TassadarPostArticleCanonicalComputationalModelStatementReportError),
    #[error(transparent)]
    ProofTransport(#[from] TassadarPostArticleExecutionSemanticsProofTransportAuditReportError),
    #[error(transparent)]
    Continuation(#[from] TassadarPostArticleContinuationNonComputationalityContractReportError),
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

pub fn build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report(
) -> Result<
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError,
> {
    let fast_route_contract =
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract();
    let architecture_selection = build_tassadar_article_fast_route_architecture_selection_report()?;
    let fast_route_implementation = build_tassadar_article_fast_route_implementation_report()?;
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let machine_lock = build_tassadar_post_article_canonical_machine_identity_lock_report()?;
    let computational_model =
        build_tassadar_post_article_canonical_computational_model_statement_report()?;
    let proof_transport =
        build_tassadar_post_article_execution_semantics_proof_transport_audit_report()?;
    let continuation =
        build_tassadar_post_article_continuation_non_computationality_contract_report()?;
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;

    Ok(build_report_from_inputs(
        fast_route_contract,
        architecture_selection,
        fast_route_implementation,
        acceptance_gate,
        machine_lock,
        computational_model,
        proof_transport,
        continuation,
        bridge,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    fast_route_contract: TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContract,
    architecture_selection: TassadarArticleFastRouteArchitectureSelectionReport,
    fast_route_implementation: TassadarArticleFastRouteImplementationReport,
    acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    machine_lock: TassadarPostArticleCanonicalMachineIdentityLockReport,
    computational_model: TassadarPostArticleCanonicalComputationalModelStatementReport,
    proof_transport: TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    continuation: TassadarPostArticleContinuationNonComputationalityContractReport,
    bridge: TassadarPostArticleUniversalityBridgeContractReport,
) -> TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport {
    let hull_routeability = architecture_selection
        .routeability_checks
        .iter()
        .find(|row| row.candidate_kind == TassadarArticleFastRouteCandidateKind::HullCacheRuntime)
        .expect("hull-cache routeability should exist");
    let recurrent_verdict = candidate_verdict(
        &architecture_selection,
        TassadarArticleFastRouteCandidateKind::LinearRecurrentRuntime,
    );
    let hierarchical_hull_verdict = candidate_verdict(
        &architecture_selection,
        TassadarArticleFastRouteCandidateKind::HierarchicalHullResearch,
    );
    let hard_max_verdict = candidate_verdict(
        &architecture_selection,
        TassadarArticleFastRouteCandidateKind::TwoDimensionalHeadHardMaxResearch,
    );

    let machine_identity_binding = TassadarPostArticleFastRouteMachineIdentityBinding {
        machine_identity_id: machine_lock.canonical_machine_tuple.machine_identity_id.clone(),
        tuple_id: machine_lock.canonical_machine_tuple.tuple_id.clone(),
        canonical_model_id: machine_lock.canonical_machine_tuple.canonical_model_id.clone(),
        canonical_route_id: machine_lock.canonical_machine_tuple.canonical_route_id.clone(),
        canonical_route_descriptor_digest: machine_lock
            .canonical_machine_tuple
            .canonical_route_descriptor_digest
            .clone(),
        canonical_fast_decode_mode: bridge.bridge_machine_identity.direct_decode_mode.clone(),
        reference_linear_proof_route_descriptor_digest: architecture_selection
            .transformer_model_route_anchor_review
            .route_descriptor_digest
            .clone(),
        selected_hull_cache_route_descriptor_digest: hull_routeability
            .projected_route_descriptor_digest
            .clone()
            .expect("selected hull route should project descriptor"),
        continuation_contract_id: machine_lock
            .canonical_machine_tuple
            .continuation_contract_id
            .clone(),
        computational_model_statement_id: computational_model
            .computational_model_statement
            .statement_id
            .clone(),
        proof_transport_boundary_id: proof_transport.transport_boundary.boundary_id.clone(),
        detail: format!(
            "machine_identity_id=`{}` tuple_id=`{}` canonical_route_id=`{}` canonical_route_descriptor_digest=`{}` reference_linear_proof_route_descriptor_digest=`{}` and proof_transport_boundary_id=`{}` remain the fast-route legitimacy tuple.",
            machine_lock.canonical_machine_tuple.machine_identity_id,
            machine_lock.canonical_machine_tuple.tuple_id,
            machine_lock.canonical_machine_tuple.canonical_route_id,
            machine_lock
                .canonical_machine_tuple
                .canonical_route_descriptor_digest,
            architecture_selection
                .transformer_model_route_anchor_review
                .route_descriptor_digest,
            proof_transport.transport_boundary.boundary_id,
        ),
    };

    let canonical_machine_binding_complete = machine_lock.lock_green
        && computational_model.statement_green
        && proof_transport.audit_green
        && continuation.contract_green
        && bridge.bridge_contract_green
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == computational_model
                .computational_model_statement
                .machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == proof_transport.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == continuation.machine_identity_binding.machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == bridge.bridge_machine_identity.machine_identity_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == computational_model.computational_model_statement.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id == proof_transport.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == continuation.machine_identity_binding.canonical_route_id
        && machine_lock.canonical_machine_tuple.canonical_route_id
            == bridge.bridge_machine_identity.canonical_route_id
        && machine_lock
            .canonical_machine_tuple
            .canonical_route_descriptor_digest
            == computational_model
                .computational_model_statement
                .canonical_route_descriptor_digest
        && machine_lock
            .canonical_machine_tuple
            .canonical_route_descriptor_digest
            == bridge.bridge_machine_identity.canonical_route_descriptor_digest;

    let reference_linear_anchor_consistent = architecture_selection
        .transformer_model_route_anchor_review
        .passed
        && fast_route_implementation.direct_proof_review.descriptor_binding_green
        && architecture_selection
            .transformer_model_route_anchor_review
            .route_descriptor_digest
            == fast_route_implementation.direct_proof_review.route_descriptor_digest;

    let canonical_hull_cache_binding_explicit =
        architecture_selection.selected_candidate_kind
            == TassadarArticleFastRouteCandidateKind::HullCacheRuntime
            && architecture_selection.fast_route_selection_green
            && fast_route_implementation.selected_candidate_kind
                == TassadarArticleFastRouteCandidateKind::HullCacheRuntime.label()
            && fast_route_implementation.fast_route_implementation_green
            && hull_routeability.routeable
            && hull_routeability
                .projected_route_descriptor_digest
                .as_deref()
                == Some(machine_identity_binding.canonical_route_descriptor_digest.as_str())
            && machine_identity_binding.canonical_route_id
                == "tassadar.article_route.direct_hull_cache_runtime.v1"
            && machine_identity_binding.canonical_fast_decode_mode == "hull_cache";

    let resumable_family_classification_explicit = continuation.contract_green
        && continuation.continuation_extends_execution_without_second_machine
        && continuation.continuation_expressivity_extension_blocked
        && proof_transport.proof_transport_complete
        && proof_transport.plugin_execution_transport_bound;

    let research_families_explicitly_outside_carrier = !recurrent_verdict.routeability_green
        && !recurrent_verdict.promoted_article_lane
        && !hierarchical_hull_verdict.routeability_green
        && !hierarchical_hull_verdict.promoted_article_lane
        && !hard_max_verdict.routeability_green
        && !hard_max_verdict.promoted_article_lane;

    let served_or_plugin_machine_overclaim_refused = fast_route_implementation
        .replacement_review
        .replacement_certified
        && fast_route_implementation.replacement_review.transformer_model_id
            == machine_identity_binding.canonical_model_id
        && proof_transport.plugin_surface_rows.iter().all(|row| {
            row.machine_identity_id == machine_identity_binding.machine_identity_id
                && row.canonical_route_id == machine_identity_binding.canonical_route_id
                && row.surface_green
        });

    let supporting_material_rows = vec![
        supporting_material_row(
            "transformer_anchor_contract",
            TassadarPostArticleFastRouteSupportingMaterialClass::Anchor,
            true,
            TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
            Some(fast_route_contract.contract_id.clone()),
            None,
            "the transformer-owned contract names route-family classes, carrier relations, and invalidations explicitly.",
        ),
        supporting_material_row(
            "article_fast_route_architecture_selection",
            TassadarPostArticleFastRouteSupportingMaterialClass::ArticleEvidence,
            architecture_selection.fast_route_selection_green,
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
            Some(architecture_selection.report_id.clone()),
            Some(architecture_selection.report_digest.clone()),
            "architecture selection classifies hull-cache as the only promoted article fast family and keeps research candidates explicit.",
        ),
        supporting_material_row(
            "article_fast_route_implementation",
            TassadarPostArticleFastRouteSupportingMaterialClass::ArticleEvidence,
            fast_route_implementation.fast_route_implementation_green,
            TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
            Some(fast_route_implementation.report_id.clone()),
            Some(fast_route_implementation.report_digest.clone()),
            "fast-route implementation binds descriptor, article-session, hybrid-route, and direct-proof surfaces to the owned article Transformer.",
        ),
        supporting_material_row(
            "article_equivalence_acceptance_gate",
            TassadarPostArticleFastRouteSupportingMaterialClass::ArticleEvidence,
            acceptance_gate.article_equivalence_green,
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            Some(acceptance_gate.report_id.clone()),
            Some(acceptance_gate.report_digest.clone()),
            "the article acceptance gate keeps the canonical article-equivalence boundary explicit while the post-article machine stays closure-bundle bounded.",
        ),
        supporting_material_row(
            "canonical_machine_identity_lock",
            TassadarPostArticleFastRouteSupportingMaterialClass::MachineBinding,
            machine_lock.lock_green,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
            Some(machine_lock.report_id.clone()),
            Some(machine_lock.report_digest.clone()),
            "the canonical machine lock binds the canonical route, weight lineage, and continuation contract into one machine tuple.",
        ),
        supporting_material_row(
            "canonical_computational_model_statement",
            TassadarPostArticleFastRouteSupportingMaterialClass::MachineBinding,
            computational_model.statement_green,
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            Some(computational_model.report_id.clone()),
            Some(computational_model.report_digest.clone()),
            "the computational-model statement names the canonical fast route and keeps plugin surfaces above the machine.",
        ),
        supporting_material_row(
            "execution_semantics_proof_transport_audit",
            TassadarPostArticleFastRouteSupportingMaterialClass::MachineBinding,
            proof_transport.audit_green,
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            Some(proof_transport.report_id.clone()),
            Some(proof_transport.report_digest.clone()),
            "the proof-transport audit keeps plugin-facing surfaces bound to the same machine instead of inheriting proof from an arbitrary fast family.",
        ),
        supporting_material_row(
            "continuation_non_computationality_contract",
            TassadarPostArticleFastRouteSupportingMaterialClass::ContinuationBoundary,
            continuation.contract_green,
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
            Some(continuation.report_id.clone()),
            Some(continuation.report_digest.clone()),
            "the continuation contract classifies resumable continuation as an extension of one machine rather than a second direct route.",
        ),
        supporting_material_row(
            "universality_bridge_contract",
            TassadarPostArticleFastRouteSupportingMaterialClass::MachineBinding,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the bridge keeps direct and resumable carriers explicit and reserves later stability issues instead of collapsing them into one ambient route claim.",
        ),
    ];

    let dependency_rows = vec![
        dependency_row(
            "canonical_machine_binding_complete",
            canonical_machine_binding_complete,
            &[
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            ],
            "canonical machine identity, route identity, continuation contract, and proof-transport boundary agree across the bound post-article chain.",
        ),
        dependency_row(
            "reference_linear_baseline_explicit",
            reference_linear_anchor_consistent,
            &[
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
                TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
            ],
            "ReferenceLinear remains an explicit proof baseline because architecture selection and implementation agree on one reference proof-route digest.",
        ),
        dependency_row(
            "canonical_hull_cache_binding_explicit",
            canonical_hull_cache_binding_explicit,
            &[
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
                TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
            ],
            "HullCache becomes canonical only when selection, implementation, and machine binding agree on one descriptor and one route identity.",
        ),
        dependency_row(
            "resumable_family_classified_without_direct_machine_collapse",
            resumable_family_classification_explicit,
            &[
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            ],
            "resumable continuation stays inside the same machine only as a continuation carrier and not as the direct fast route.",
        ),
        dependency_row(
            "research_families_explicitly_outside_carrier",
            research_families_explicitly_outside_carrier,
            &[TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF],
            "research-only fast families stay explicit and outside the carrier until they earn route contracts and semantics-equivalence evidence.",
        ),
        dependency_row(
            "served_or_plugin_wording_bound_to_canonical_machine",
            served_or_plugin_machine_overclaim_refused,
            &[
                TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            ],
            "served and plugin-facing wording stays bound to the canonical machine tuple instead of leaning on an unbound fast-family alias.",
        ),
    ];

    let route_family_rows = vec![
        route_family_row(
            "reference_linear",
            TassadarPostArticleFastRouteFamilyClass::ReferenceLinearBaseline,
            TassadarPostArticleFastRouteCarrierRelation::HistoricalProofBaseline,
            vec![
                String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF),
                String::from(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF),
            ],
            None,
            Some(
                machine_identity_binding
                    .reference_linear_proof_route_descriptor_digest
                    .clone(),
            ),
            Some(machine_identity_binding.machine_identity_id.clone()),
            true,
            false,
            false,
            vec![
                String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF),
                String::from(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF),
            ],
            reference_linear_anchor_consistent,
            "ReferenceLinear stays classified as the proof-origin baseline and semantic anchor rather than the served canonical machine.",
        ),
        route_family_row(
            "hull_cache",
            TassadarPostArticleFastRouteFamilyClass::CanonicalHullCache,
            TassadarPostArticleFastRouteCarrierRelation::CanonicalDirectCarrierBound,
            vec![
                String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF),
                String::from(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
            ],
            Some(machine_identity_binding.canonical_route_id.clone()),
            Some(machine_identity_binding.canonical_route_descriptor_digest.clone()),
            Some(machine_identity_binding.machine_identity_id.clone()),
            true,
            true,
            true,
            vec![
                String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF),
                String::from(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
            ],
            canonical_hull_cache_binding_explicit,
            "HullCache is classified as the canonical direct carrier only because semantics-equivalence, proof transport, and canonical machine lock remain explicit and green.",
        ),
        route_family_row(
            "resumable_continuation_family",
            TassadarPostArticleFastRouteFamilyClass::ResumableContinuationFamily,
            TassadarPostArticleFastRouteCarrierRelation::ContinuationCarrierOnly,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            Some(machine_identity_binding.canonical_route_id.clone()),
            None,
            Some(machine_identity_binding.machine_identity_id.clone()),
            false,
            true,
            false,
            vec![
                String::from(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            resumable_family_classification_explicit,
            "Resumable continuation is classified as a continuation-only carrier on the same machine and not as a direct fast route underneath served or plugin surfaces.",
        ),
        route_family_row(
            "linear_recurrent_runtime",
            TassadarPostArticleFastRouteFamilyClass::ResearchOnlyFastRoute,
            TassadarPostArticleFastRouteCarrierRelation::OutsideCarrier,
            vec![String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF)],
            None,
            None,
            None,
            false,
            false,
            false,
            vec![String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF)],
            !recurrent_verdict.routeability_green && !recurrent_verdict.promoted_article_lane,
            "The recurrent runtime stays explicitly outside the carrier because the canonical route contract still lacks a recurrent decode family.",
        ),
        route_family_row(
            "hierarchical_hull_research",
            TassadarPostArticleFastRouteFamilyClass::ResearchOnlyFastRoute,
            TassadarPostArticleFastRouteCarrierRelation::OutsideCarrier,
            vec![String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF)],
            None,
            None,
            None,
            false,
            false,
            false,
            vec![String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF)],
            !hierarchical_hull_verdict.routeability_green
                && !hierarchical_hull_verdict.promoted_article_lane,
            "Hierarchical-hull stays explicitly outside the carrier as research-only evidence instead of a proof-bearing or platform-bearing route.",
        ),
        route_family_row(
            "two_dimensional_head_hard_max_research",
            TassadarPostArticleFastRouteFamilyClass::ResearchOnlyFastRoute,
            TassadarPostArticleFastRouteCarrierRelation::OutsideCarrier,
            vec![String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF)],
            None,
            None,
            None,
            false,
            false,
            false,
            vec![String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF)],
            !hard_max_verdict.routeability_green && !hard_max_verdict.promoted_article_lane,
            "The bounded 2D-head hard-max research lane stays explicitly outside the carrier until it owns canonical fast-route evidence.",
        ),
    ];

    let unproven_fast_routes_quarantined = route_family_rows
        .iter()
        .filter(|row| row.route_family_class == TassadarPostArticleFastRouteFamilyClass::ResearchOnlyFastRoute)
        .all(|row| {
            row.route_family_green
                && !row.proof_inheritance_allowed
                && !row.universality_inheritance_allowed
                && !row.served_or_plugin_machine_claim_allowed
        });
    let resumable_family_not_presented_as_direct_machine = route_family_rows
        .iter()
        .find(|row| row.route_family_id == "resumable_continuation_family")
        .is_some_and(|row| {
            row.route_family_green
                && !row.proof_inheritance_allowed
                && row.universality_inheritance_allowed
                && !row.served_or_plugin_machine_claim_allowed
        });

    let invalidation_rows = vec![
        invalidation_row(
            "canonical_machine_tuple_mismatch_detected",
            !canonical_machine_binding_complete,
            &[
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            ],
            "machine identity, route identity, or continuation contract drift invalidates the carrier-binding contract.",
        ),
        invalidation_row(
            "reference_linear_anchor_drift_detected",
            !reference_linear_anchor_consistent,
            &[
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
                TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
            ],
            "the reference proof baseline may not drift silently across architecture-selection and implementation surfaces.",
        ),
        invalidation_row(
            "hull_cache_binding_drift_detected",
            !canonical_hull_cache_binding_explicit,
            &[
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
                TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
            ],
            "the canonical HullCache route may not inherit machine status unless selection, implementation, and canonical binding remain aligned.",
        ),
        invalidation_row(
            "resumable_family_direct_machine_overclaim_detected",
            !resumable_family_not_presented_as_direct_machine,
            &[
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            ],
            "the resumable family may not be presented as the underlying direct machine.",
        ),
        invalidation_row(
            "research_fast_route_inside_carrier_detected",
            !unproven_fast_routes_quarantined,
            &[TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF],
            "research-only fast families may not quietly move inside the carrier.",
        ),
        invalidation_row(
            "served_or_plugin_unbound_route_overclaim_detected",
            !served_or_plugin_machine_overclaim_refused,
            &[
                TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            ],
            "served or plugin wording may not treat an unbound route family as the underlying machine.",
        ),
    ];
    let invalidation_rows_absent = invalidation_rows.iter().all(|row| !row.present);
    let carrier_binding_complete = dependency_rows.iter().all(|row| row.satisfied)
        && route_family_rows.iter().all(|row| row.route_family_green);
    let fast_route_legitimacy_complete = carrier_binding_complete
        && unproven_fast_routes_quarantined
        && resumable_family_not_presented_as_direct_machine
        && served_or_plugin_machine_overclaim_refused
        && invalidation_rows_absent;

    let validation_rows = vec![
        validation_row(
            "transformer_anchor_contract_published",
            true,
            &[TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF],
            "the transformer-owned fast-route legitimacy contract is published.",
        ),
        validation_row(
            "supporting_material_dependencies_green",
            supporting_material_rows.iter().all(|row| row.satisfied),
            &[
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
                TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
                TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            ],
            "declared supporting materials stay explicit and green.",
        ),
        validation_row(
            "reference_linear_baseline_classified",
            reference_linear_anchor_consistent,
            &[
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
                TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
            ],
            "ReferenceLinear remains an explicit proof baseline and not an ambient served machine alias.",
        ),
        validation_row(
            "canonical_hull_cache_route_bound",
            canonical_hull_cache_binding_explicit,
            &[
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
                TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
            ],
            "HullCache stays bound to the canonical machine only on explicit evidence.",
        ),
        validation_row(
            "resumable_family_continuation_only",
            resumable_family_not_presented_as_direct_machine,
            &[
                TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            ],
            "resumable continuation remains a continuation carrier rather than a direct fast machine.",
        ),
        validation_row(
            "research_fast_routes_quarantined",
            unproven_fast_routes_quarantined,
            &[TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF],
            "research-only fast routes stay outside the carrier and outside served or plugin wording.",
        ),
        validation_row(
            "served_or_plugin_machine_overclaim_refused",
            served_or_plugin_machine_overclaim_refused,
            &[
                TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            ],
            "served and plugin wording stay bound to the canonical machine tuple instead of an unbound route family.",
        ),
        validation_row(
            "fast_route_legitimacy_complete",
            fast_route_legitimacy_complete,
            &[TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF],
            "fast-route legitimacy and carrier binding are complete and the frontier moves to TAS-215.",
        ),
    ];

    let supporting_material_refs = vec![
        String::from(TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF),
        String::from(TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF),
        String::from(TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF),
        String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
        String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
        String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
        String::from(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF),
        String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
    ];

    let contract_green = carrier_binding_complete
        && invalidation_rows_absent
        && validation_rows.iter().all(|row| row.green);

    let mut report = TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_fast_route_legitimacy_and_carrier_binding_contract.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_CHECKER_REF,
        ),
        transformer_anchor_contract_ref: String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
        article_fast_route_architecture_selection_report_ref: String::from(
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
        ),
        article_fast_route_implementation_report_ref: String::from(
            TASSADAR_ARTICLE_FAST_ROUTE_IMPLEMENTATION_REPORT_REF,
        ),
        article_equivalence_acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        canonical_machine_identity_lock_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
        ),
        canonical_computational_model_statement_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
        ),
        execution_semantics_proof_transport_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
        ),
        continuation_non_computationality_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF,
        ),
        universality_bridge_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
        ),
        supporting_material_refs,
        fast_route_contract,
        machine_identity_binding,
        supporting_material_rows,
        dependency_rows,
        route_family_rows,
        invalidation_rows,
        validation_rows,
        contract_status: if contract_green {
            TassadarPostArticleFastRouteLegitimacyStatus::Green
        } else {
            TassadarPostArticleFastRouteLegitimacyStatus::Blocked
        },
        contract_green,
        carrier_binding_complete,
        unproven_fast_routes_quarantined,
        resumable_family_not_presented_as_direct_machine,
        served_or_plugin_machine_overclaim_refused,
        fast_route_legitimacy_complete,
        next_stability_issue_id: String::from(NEXT_STABILITY_ISSUE_ID),
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        claim_boundary: String::from(
            "this eval-owned contract report freezes only fast-route legitimacy and carrier binding on the canonical post-article machine. It classifies ReferenceLinear as the proof-origin baseline, HullCache as the canonical direct fast route only on explicit semantics-equivalence evidence, resumable continuation as continuation-only, and research fast families as outside the carrier. Downward non-influence, served conformance, anti-drift closeout, and the final closure bundle remain later issues.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article fast-route legitimacy and carrier-binding contract report keeps status={:?}, supporting_material_rows={}, dependency_rows={}, route_family_rows={}, invalidation_rows={}, validation_rows={}, fast_route_legitimacy_complete={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
        report.contract_status,
        report.supporting_material_rows.len(),
        report.dependency_rows.len(),
        report.route_family_rows.len(),
        report.invalidation_rows.len(),
        report.validation_rows.len(),
        report.fast_route_legitimacy_complete,
        report.next_stability_issue_id,
        report.closure_bundle_issue_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report|",
        &report,
    );
    report
}

fn candidate_verdict(
    architecture_selection: &TassadarArticleFastRouteArchitectureSelectionReport,
    candidate_kind: TassadarArticleFastRouteCandidateKind,
) -> &crate::TassadarArticleFastRouteCandidateVerdict {
    architecture_selection
        .candidate_verdicts
        .iter()
        .find(|row| row.candidate_kind == candidate_kind)
        .expect("candidate verdict should exist")
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleFastRouteSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleFastRouteSupportingMaterialRow {
    TassadarPostArticleFastRouteSupportingMaterialRow {
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
) -> TassadarPostArticleFastRouteDependencyRow {
    TassadarPostArticleFastRouteDependencyRow {
        dependency_id: String::from(dependency_id),
        satisfied,
        source_refs: source_refs.iter().map(|source| String::from(*source)).collect(),
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn route_family_row(
    route_family_id: &str,
    route_family_class: TassadarPostArticleFastRouteFamilyClass,
    carrier_relation: TassadarPostArticleFastRouteCarrierRelation,
    source_refs: Vec<String>,
    declared_route_id: Option<String>,
    declared_route_descriptor_digest: Option<String>,
    bound_machine_identity_id: Option<String>,
    proof_inheritance_allowed: bool,
    universality_inheritance_allowed: bool,
    served_or_plugin_machine_claim_allowed: bool,
    semantics_equivalence_evidence_refs: Vec<String>,
    route_family_green: bool,
    detail: &str,
) -> TassadarPostArticleFastRouteCarrierClassificationRow {
    TassadarPostArticleFastRouteCarrierClassificationRow {
        route_family_id: String::from(route_family_id),
        route_family_class,
        carrier_relation,
        source_refs,
        declared_route_id,
        declared_route_descriptor_digest,
        bound_machine_identity_id,
        proof_inheritance_allowed,
        universality_inheritance_allowed,
        served_or_plugin_machine_claim_allowed,
        semantics_equivalence_evidence_refs,
        route_family_green,
        detail: String::from(detail),
    }
}

fn invalidation_row(
    invalidation_id: &str,
    present: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticleFastRouteInvalidationRow {
    TassadarPostArticleFastRouteInvalidationRow {
        invalidation_id: String::from(invalidation_id),
        present,
        source_refs: source_refs.iter().map(|source| String::from(*source)).collect(),
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticleFastRouteValidationRow {
    TassadarPostArticleFastRouteValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs.iter().map(|source| String::from(*source)).collect(),
        detail: String::from(detail),
    }
}

#[must_use]
pub fn tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_REPORT_REF,
    )
}

pub fn write_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report =
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
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
) -> Result<T, TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report,
        read_json,
        tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report_path,
        write_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report,
        TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport,
        TassadarPostArticleFastRouteLegitimacyStatus,
    };
    use psionic_transformer::{
        TassadarPostArticleFastRouteCarrierRelation, TassadarPostArticleFastRouteFamilyClass,
    };
    use tempfile::tempdir;

    #[test]
    fn fast_route_legitimacy_and_carrier_binding_contract_report_keeps_route_classes_explicit() {
        let report =
            build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report()
                .expect("report");

        assert_eq!(
            report.contract_status,
            TassadarPostArticleFastRouteLegitimacyStatus::Green
        );
        assert!(report.contract_green);
        assert_eq!(report.supporting_material_rows.len(), 9);
        assert_eq!(report.dependency_rows.len(), 6);
        assert_eq!(report.route_family_rows.len(), 6);
        assert_eq!(report.invalidation_rows.len(), 6);
        assert_eq!(report.validation_rows.len(), 8);
        assert!(report.carrier_binding_complete);
        assert!(report.unproven_fast_routes_quarantined);
        assert!(report.resumable_family_not_presented_as_direct_machine);
        assert!(report.served_or_plugin_machine_overclaim_refused);
        assert!(report.fast_route_legitimacy_complete);
        assert_eq!(report.next_stability_issue_id, "TAS-215");
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
        assert!(report.invalidation_rows.iter().all(|row| !row.present));
        let reference_linear = report
            .route_family_rows
            .iter()
            .find(|row| row.route_family_id == "reference_linear")
            .expect("reference linear row");
        assert_eq!(
            reference_linear.route_family_class,
            TassadarPostArticleFastRouteFamilyClass::ReferenceLinearBaseline
        );
        assert_eq!(
            reference_linear.carrier_relation,
            TassadarPostArticleFastRouteCarrierRelation::HistoricalProofBaseline
        );
        let hull_cache = report
            .route_family_rows
            .iter()
            .find(|row| row.route_family_id == "hull_cache")
            .expect("hull-cache row");
        assert_eq!(
            hull_cache.carrier_relation,
            TassadarPostArticleFastRouteCarrierRelation::CanonicalDirectCarrierBound
        );
        assert!(hull_cache.served_or_plugin_machine_claim_allowed);
    }

    #[test]
    fn fast_route_legitimacy_and_carrier_binding_contract_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report()
                .expect("report");
        let committed: TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport =
            read_json(
                tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report_path(),
            )
            .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_fast_route_legitimacy_and_carrier_binding_contract_report_persists_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report.json");
        let written =
            write_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContractReport =
            read_json(&output_path).expect("persisted report");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report.json"
            )
        );
    }
}
