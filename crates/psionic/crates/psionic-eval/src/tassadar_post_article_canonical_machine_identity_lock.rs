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

use psionic_transformer::{
    build_tassadar_post_article_canonical_machine_identity_lock_contract,
    TassadarPostArticleCanonicalMachineIdentityLockContract,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_identity_lock_report.json";
pub const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-canonical-machine-identity-lock.sh";

const TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_canonical_machine_identity_lock_contract.rs";
const TASSADAR_TCM_V1_MODEL_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_tcm_v1_model.json";
const TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_article_equivalence_acceptance_gate_report.json";
const TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_bridge_contract_report.json";
const TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_route_semantic_preservation_audit_report.json";
const TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_carrier_split_contract_report.json";
const TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_control_plane_decision_provenance_proof_report.json";
const TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universal_machine_proof_rebinding_report.json";
const TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_witness_suite_reissue_report.json";
const TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_route_universal_substrate_gate_report.json";
const TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_portability_minimality_matrix_report.json";
const TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_rebased_universality_verdict_split_report.json";
const TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_turing_completeness_closeout_audit_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report.json";
const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report.json";
const TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF_LOCAL:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report.json";
const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const PLUGIN_SYSTEM_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";

const CORE_TUPLE_FIELD_IDS: [&str; 8] = [
    "machine_identity_id",
    "canonical_model_id",
    "canonical_weight_bundle_digest",
    "canonical_weight_primary_artifact_sha256",
    "canonical_route_id",
    "canonical_route_descriptor_digest",
    "continuation_contract_id",
    "continuation_contract_digest",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalMachineIdentityLockStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalMachineSupportingMaterialClass {
    ProofCarrying,
    Contract,
    ObservationalContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalMachineArtifactScopeKind {
    Bridge,
    BenchmarkRoute,
    RouteAudit,
    Proof,
    Witness,
    Receipt,
    Controller,
    Gate,
    Closeout,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleCanonicalMachineSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineIdentityTuple {
    pub tuple_id: String,
    pub carrier_class_id: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub canonical_machine_lock_contract_id: String,
    pub bridge_contract_report_id: String,
    pub bridge_contract_report_digest: String,
    pub tcm_v1_model_id: String,
    pub tcm_v1_model_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineArtifactBindingRow {
    pub binding_id: String,
    pub scope_kind: TassadarPostArticleCanonicalMachineArtifactScopeKind,
    pub carrier_projection_id: String,
    pub source_ref: String,
    pub source_artifact_id: String,
    pub source_artifact_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_machine_identity_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_canonical_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_canonical_weight_bundle_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_canonical_weight_primary_artifact_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_canonical_route_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_canonical_route_descriptor_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_continuation_contract_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_continuation_contract_digest: Option<String>,
    pub source_tuple_field_ids: Vec<String>,
    pub lock_projected_field_ids: Vec<String>,
    pub self_carries_full_tuple: bool,
    pub bound_by_lock: bool,
    pub tuple_matches_canonical_lock: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineInvalidationRow {
    pub invalidation_id: String,
    pub present: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineIdentityLockReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub transformer_anchor_contract_ref: String,
    pub tcm_v1_model_ref: String,
    pub article_equivalence_acceptance_gate_report_ref: String,
    pub bridge_contract_report_ref: String,
    pub canonical_route_semantic_preservation_audit_report_ref: String,
    pub carrier_split_contract_report_ref: String,
    pub control_plane_decision_provenance_proof_report_ref: String,
    pub universal_machine_proof_rebinding_report_ref: String,
    pub universality_witness_suite_reissue_report_ref: String,
    pub canonical_route_universal_substrate_gate_report_ref: String,
    pub universality_portability_minimality_matrix_report_ref: String,
    pub rebased_universality_verdict_split_report_ref: String,
    pub turing_completeness_closeout_audit_report_ref: String,
    pub plugin_invocation_receipts_and_replay_classes_report_ref: String,
    pub weighted_plugin_controller_trace_eval_report_ref: String,
    pub plugin_conformance_sandbox_and_benchmark_harness_eval_report_ref: String,
    pub plugin_authority_promotion_publication_and_trust_tier_gate_report_ref: String,
    pub bounded_weighted_plugin_platform_closeout_audit_report_ref: String,
    pub post_article_turing_audit_ref: String,
    pub plugin_system_turing_audit_ref: String,
    pub canonical_machine_lock_contract: TassadarPostArticleCanonicalMachineIdentityLockContract,
    pub supporting_material_rows: Vec<TassadarPostArticleCanonicalMachineSupportingMaterialRow>,
    pub dependency_rows: Vec<TassadarPostArticleCanonicalMachineDependencyRow>,
    pub canonical_machine_tuple: TassadarPostArticleCanonicalMachineIdentityTuple,
    pub artifact_binding_rows: Vec<TassadarPostArticleCanonicalMachineArtifactBindingRow>,
    pub invalidation_rows: Vec<TassadarPostArticleCanonicalMachineInvalidationRow>,
    pub validation_rows: Vec<TassadarPostArticleCanonicalMachineValidationRow>,
    pub lock_status: TassadarPostArticleCanonicalMachineIdentityLockStatus,
    pub lock_green: bool,
    pub one_canonical_machine_named: bool,
    pub mixed_carrier_evidence_bundle_refused: bool,
    pub legacy_projection_binding_complete: bool,
    pub closure_bundle_embedded_here: bool,
    pub closure_bundle_issue_id: String,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCanonicalMachineIdentityLockReportError {
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TcmModelInput {
    model_id: String,
    model_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct AcceptanceGateInput {
    report_id: String,
    report_digest: String,
    article_equivalence_green: bool,
    public_claim_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BridgeMachineIdentityInput {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_weight_artifact_id: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BridgeComputationalModelStatementInput {
    statement_id: String,
    #[serde(alias = "canonical_machine_identity_id")]
    machine_identity_id: String,
    substrate_model_id: String,
    substrate_model_digest: String,
    runtime_contract_id: String,
    runtime_contract_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BridgeInput {
    report_id: String,
    report_digest: String,
    bridge_contract_green: bool,
    bridge_machine_identity: BridgeMachineIdentityInput,
    computational_model_statement: BridgeComputationalModelStatementInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CanonicalIdentityReviewInput {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
    canonical_identity_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CarrierSplitCarrierRowInput {
    carrier_id: String,
    machine_identity_id: String,
    carrier_truth_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TopLevelIdentityArtifactInput {
    report_id: String,
    report_digest: String,
    #[serde(default)]
    machine_identity_id: Option<String>,
    #[serde(default)]
    canonical_model_id: Option<String>,
    #[serde(default)]
    canonical_weight_bundle_digest: Option<String>,
    #[serde(default)]
    canonical_weight_primary_artifact_sha256: Option<String>,
    #[serde(default)]
    canonical_route_id: Option<String>,
    #[serde(default)]
    canonical_route_descriptor_digest: Option<String>,
    #[serde(default)]
    continuation_contract_id: Option<String>,
    #[serde(default)]
    continuation_contract_digest: Option<String>,
    #[serde(default)]
    rebase_claim_allowed: bool,
    #[serde(default)]
    plugin_capability_claim_allowed: bool,
    #[serde(default)]
    served_public_universality_allowed: bool,
    #[serde(default)]
    arbitrary_software_capability_allowed: bool,
    #[serde(default)]
    semantic_preservation_status: Option<String>,
    #[serde(default)]
    control_plane_ownership_green: bool,
    #[serde(default)]
    decision_provenance_proof_complete: bool,
    #[serde(default)]
    carrier_split_status: Option<String>,
    #[serde(default)]
    carrier_split_publication_complete: bool,
    #[serde(default)]
    carrier_collapse_refused: bool,
    #[serde(default)]
    proof_rebinding_complete: bool,
    #[serde(default)]
    witness_suite_reissued: bool,
    #[serde(default)]
    bounded_universality_story_carried: bool,
    #[serde(default)]
    gate_status: Option<String>,
    #[serde(default)]
    matrix_status: Option<String>,
    #[serde(default)]
    served_suppression_boundary_preserved: bool,
    #[serde(default)]
    verdict_split_status: Option<String>,
    #[serde(default)]
    canonical_identity_review: Option<CanonicalIdentityReviewInput>,
    #[serde(default)]
    primary_carrier_rows: Vec<CarrierSplitCarrierRowInput>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct MachineIdentityBindingInput {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct BoundArtifactInput {
    report_id: String,
    report_digest: String,
    machine_identity_binding: MachineIdentityBindingInput,
    #[serde(default)]
    contract_green: bool,
    #[serde(default)]
    rebase_claim_allowed: bool,
    #[serde(default)]
    plugin_capability_claim_allowed: bool,
    #[serde(default)]
    weighted_plugin_control_allowed: bool,
    #[serde(default)]
    plugin_publication_allowed: bool,
    #[serde(default)]
    served_public_universality_allowed: bool,
    #[serde(default)]
    arbitrary_software_capability_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CloseoutInput {
    report_id: String,
    report_digest: String,
    machine_identity_binding: MachineIdentityBindingInput,
    closeout_green: bool,
    closure_bundle_issue_id: String,
    closure_bundle_embedded_here: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
}

pub fn build_tassadar_post_article_canonical_machine_identity_lock_report() -> Result<
    TassadarPostArticleCanonicalMachineIdentityLockReport,
    TassadarPostArticleCanonicalMachineIdentityLockReportError,
> {
    let transformer_contract =
        build_tassadar_post_article_canonical_machine_identity_lock_contract();
    let tcm_v1_model: TcmModelInput = read_repo_json(TASSADAR_TCM_V1_MODEL_REF_LOCAL)?;
    let acceptance_gate: AcceptanceGateInput =
        read_repo_json(TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF_LOCAL)?;
    let bridge: BridgeInput =
        read_repo_json(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF_LOCAL)?;
    let semantic_preservation: TopLevelIdentityArtifactInput = read_repo_json(
        TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF_LOCAL,
    )?;
    let carrier_split: TopLevelIdentityArtifactInput =
        read_repo_json(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF_LOCAL)?;
    let control_plane: TopLevelIdentityArtifactInput = read_repo_json(
        TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF_LOCAL,
    )?;
    let proof_rebinding: TopLevelIdentityArtifactInput =
        read_repo_json(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF_LOCAL)?;
    let witness_suite: TopLevelIdentityArtifactInput =
        read_repo_json(TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF_LOCAL)?;
    let universal_substrate_gate: TopLevelIdentityArtifactInput = read_repo_json(
        TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF_LOCAL,
    )?;
    let portability_matrix: TopLevelIdentityArtifactInput = read_repo_json(
        TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF_LOCAL,
    )?;
    let rebased_verdict: TopLevelIdentityArtifactInput =
        read_repo_json(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF_LOCAL)?;
    let turing_closeout: CloseoutInput =
        read_repo_json(TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL)?;
    let invocation_receipts: BoundArtifactInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL,
    )?;
    let controller_eval: BoundArtifactInput = read_repo_json(
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
    )?;
    let conformance_eval: BoundArtifactInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
    )?;
    let authority_gate: BoundArtifactInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
    )?;
    let platform_closeout: CloseoutInput = read_repo_json(
        TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
    )?;

    let canonical_machine_tuple = TassadarPostArticleCanonicalMachineIdentityTuple {
        tuple_id: transformer_contract.tuple_id.clone(),
        carrier_class_id: transformer_contract.carrier_class_id.clone(),
        machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
        canonical_model_id: bridge.bridge_machine_identity.canonical_model_id.clone(),
        canonical_weight_artifact_id: bridge
            .bridge_machine_identity
            .canonical_weight_artifact_id
            .clone(),
        canonical_weight_bundle_digest: bridge
            .bridge_machine_identity
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: bridge
            .bridge_machine_identity
            .canonical_weight_primary_artifact_sha256
            .clone(),
        canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
        canonical_route_descriptor_digest: bridge
            .bridge_machine_identity
            .canonical_route_descriptor_digest
            .clone(),
        continuation_contract_id: bridge
            .bridge_machine_identity
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: bridge
            .bridge_machine_identity
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: bridge.computational_model_statement.statement_id.clone(),
        canonical_machine_lock_contract_id: transformer_contract.contract_id.clone(),
        bridge_contract_report_id: bridge.report_id.clone(),
        bridge_contract_report_digest: bridge.report_digest.clone(),
        tcm_v1_model_id: tcm_v1_model.model_id.clone(),
        tcm_v1_model_digest: tcm_v1_model.model_digest.clone(),
        detail: format!(
            "tuple_id=`{}` carrier_class_id=`{}` machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` continuation_contract_id=`{}` binds the post-article machine lock above `{}`.",
            transformer_contract.tuple_id,
            transformer_contract.carrier_class_id,
            bridge.bridge_machine_identity.machine_identity_id,
            bridge.bridge_machine_identity.canonical_model_id,
            bridge.bridge_machine_identity.canonical_route_id,
            bridge.bridge_machine_identity.continuation_contract_id,
            bridge.report_id,
        ),
    };

    let transformer_contract_green = transformer_contract
        .identity_rule_rows
        .iter()
        .chain(transformer_contract.invalidation_rule_rows.iter())
        .all(|row| row.green);
    let tcm_model_matches_bridge = tcm_v1_model.model_id
        == bridge.computational_model_statement.substrate_model_id
        && tcm_v1_model.model_digest == bridge.computational_model_statement.substrate_model_digest
        && bridge.computational_model_statement.machine_identity_id
            == canonical_machine_tuple.machine_identity_id
        && bridge.computational_model_statement.runtime_contract_id
            == canonical_machine_tuple.continuation_contract_id
        && bridge.computational_model_statement.runtime_contract_digest
            == canonical_machine_tuple.continuation_contract_digest;
    let acceptance_gate_green =
        acceptance_gate.article_equivalence_green && acceptance_gate.public_claim_allowed;
    let bridge_green = bridge.bridge_contract_green
        && !bridge
            .bridge_machine_identity
            .machine_identity_id
            .is_empty()
        && !bridge.bridge_machine_identity.canonical_model_id.is_empty()
        && !bridge
            .bridge_machine_identity
            .canonical_weight_bundle_digest
            .is_empty()
        && !bridge.bridge_machine_identity.canonical_route_id.is_empty()
        && !bridge
            .bridge_machine_identity
            .canonical_route_descriptor_digest
            .is_empty()
        && !bridge
            .bridge_machine_identity
            .continuation_contract_id
            .is_empty()
        && !bridge
            .bridge_machine_identity
            .continuation_contract_digest
            .is_empty();
    let turing_closeout_green = turing_closeout.closeout_green
        && !turing_closeout.closure_bundle_embedded_here
        && turing_closeout.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID;
    let platform_closeout_green = platform_closeout.closeout_green
        && !platform_closeout.closure_bundle_embedded_here
        && platform_closeout.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID
        && platform_closeout.plugin_capability_claim_allowed
        && platform_closeout.weighted_plugin_control_allowed
        && !platform_closeout.plugin_publication_allowed
        && !platform_closeout.served_public_universality_allowed
        && !platform_closeout.arbitrary_software_capability_allowed;
    let plugin_tuple_surfaces_green = invocation_receipts.contract_green
        && controller_eval.contract_green
        && conformance_eval.contract_green
        && authority_gate.contract_green;

    let supporting_material_rows = vec![
        supporting_material_row(
            "transformer_anchor_contract",
            TassadarPostArticleCanonicalMachineSupportingMaterialClass::Contract,
            transformer_contract_green,
            TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
            Some(transformer_contract.contract_id.clone()),
            None,
            "the transformer-owned lock contract is the code-owned anchor for the globally named machine tuple and its invalidation laws.",
        ),
        supporting_material_row(
            "tcm_v1_model",
            TassadarPostArticleCanonicalMachineSupportingMaterialClass::ProofCarrying,
            tcm_model_matches_bridge,
            TASSADAR_TCM_V1_MODEL_REF_LOCAL,
            Some(tcm_v1_model.model_id.clone()),
            Some(tcm_v1_model.model_digest.clone()),
            "the historical `TCM.v1` model remains the declared substrate below the canonical machine lock and must match the bridge computational-model statement by id and digest.",
        ),
        supporting_material_row(
            "article_equivalence_acceptance_gate",
            TassadarPostArticleCanonicalMachineSupportingMaterialClass::ProofCarrying,
            acceptance_gate_green,
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF_LOCAL,
            Some(acceptance_gate.report_id.clone()),
            Some(acceptance_gate.report_digest.clone()),
            "the canonical machine lock may only attach to the already-closed bounded article-equivalence route instead of floating above an unclosed benchmark surface.",
        ),
        supporting_material_row(
            "post_article_universality_bridge",
            TassadarPostArticleCanonicalMachineSupportingMaterialClass::ProofCarrying,
            bridge_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF_LOCAL,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the universality bridge still names the machine identity, route identity, continuation contract, and computational-model statement that the lock is freezing.",
        ),
        supporting_material_row(
            "post_article_turing_closeout",
            TassadarPostArticleCanonicalMachineSupportingMaterialClass::ProofCarrying,
            turing_closeout_green,
            TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
            Some(turing_closeout.report_id.clone()),
            Some(turing_closeout.report_digest.clone()),
            "the rebased Turing closeout remains one proof-bearing compute/control carrier beneath the machine lock instead of being recomputed here.",
        ),
        supporting_material_row(
            "bounded_weighted_plugin_platform_closeout",
            TassadarPostArticleCanonicalMachineSupportingMaterialClass::ProofCarrying,
            platform_closeout_green,
            TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
            Some(platform_closeout.report_id.clone()),
            Some(platform_closeout.report_digest.clone()),
            "the bounded weighted plugin-platform closeout remains the highest current capability-bearing surface that must inherit the same canonical machine tuple.",
        ),
        supporting_material_row(
            "post_article_turing_audit_context",
            TassadarPostArticleCanonicalMachineSupportingMaterialClass::ObservationalContext,
            true,
            POST_ARTICLE_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 post-article audit remains observational context that explains why machine identity, continuation, and closure-bundle separation must stay explicit.",
        ),
        supporting_material_row(
            "plugin_system_turing_audit_context",
            TassadarPostArticleCanonicalMachineSupportingMaterialClass::ObservationalContext,
            true,
            PLUGIN_SYSTEM_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 plugin-system audit remains observational context that explains why plugin receipts and controller traces must inherit the same machine lock rather than float on adjacent runtime observations.",
        ),
    ];

    let dependency_rows = vec![
        dependency_row(
            "transformer_anchor_contract_green",
            transformer_contract_green,
            vec![String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF)],
            "the canonical machine lock depends on the transformer-owned anchor contract so the tuple and invalidation laws are code-owned rather than comment-only.",
        ),
        dependency_row(
            "tcm_v1_model_matches_bridge_statement",
            tcm_model_matches_bridge,
            vec![
                String::from(TASSADAR_TCM_V1_MODEL_REF_LOCAL),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF_LOCAL),
            ],
            "the historical `TCM.v1` substrate and the bridge computational-model statement must agree by id and digest before the machine lock can name one machine honestly.",
        ),
        dependency_row(
            "article_equivalence_acceptance_gate_green",
            acceptance_gate_green,
            vec![String::from(
                TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF_LOCAL,
            )],
            "the canonical machine lock still depends on the bounded article-equivalence benchmark and route closure rather than bypassing it.",
        ),
        dependency_row(
            "post_article_bridge_green",
            bridge_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF_LOCAL,
            )],
            "the machine tuple still comes from the green bridge artifact instead of inventing a second route identity surface.",
        ),
        dependency_row(
            "turing_closeout_remains_separate",
            turing_closeout_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
            )],
            "the machine lock depends on the rebased Turing closeout staying green while still keeping the final closure bundle separate for `TAS-215`.",
        ),
        dependency_row(
            "plugin_platform_closeout_inherits_same_machine",
            platform_closeout_green && plugin_tuple_surfaces_green,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
                ),
            ],
            "the canonical machine lock depends on plugin receipts, controller traces, conformance evidence, authority posture, and bounded platform closeout inheriting the same tuple instead of floating on a second machine.",
        ),
    ];

    let artifact_binding_rows = vec![
        artifact_binding_row(
            "bridge_contract_anchor",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Bridge,
            "canonical_bridge_anchor",
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF_LOCAL,
            &bridge.report_id,
            &bridge.report_digest,
            ArtifactProjection {
                machine_identity_id: Some(bridge.bridge_machine_identity.machine_identity_id.clone()),
                canonical_model_id: Some(bridge.bridge_machine_identity.canonical_model_id.clone()),
                canonical_weight_bundle_digest: Some(
                    bridge.bridge_machine_identity.canonical_weight_bundle_digest.clone(),
                ),
                canonical_weight_primary_artifact_sha256: Some(
                    bridge
                        .bridge_machine_identity
                        .canonical_weight_primary_artifact_sha256
                        .clone(),
                ),
                canonical_route_id: Some(bridge.bridge_machine_identity.canonical_route_id.clone()),
                canonical_route_descriptor_digest: Some(
                    bridge
                        .bridge_machine_identity
                        .canonical_route_descriptor_digest
                        .clone(),
                ),
                continuation_contract_id: Some(
                    bridge.bridge_machine_identity.continuation_contract_id.clone(),
                ),
                continuation_contract_digest: Some(
                    bridge
                        .bridge_machine_identity
                        .continuation_contract_digest
                        .clone(),
                ),
            },
            bridge_green,
            &canonical_machine_tuple,
            "the bridge artifact already carries the full canonical tuple and remains the anchor that later projections must match.",
        ),
        artifact_binding_row(
            "article_equivalence_acceptance_gate",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::BenchmarkRoute,
            "direct_benchmark_route",
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF_LOCAL,
            &acceptance_gate.report_id,
            &acceptance_gate.report_digest,
            ArtifactProjection::default(),
            acceptance_gate_green,
            &canonical_machine_tuple,
            "the acceptance gate remains the bounded benchmark-bearing route gate and is explicitly bound here to the canonical tuple instead of inheriting route identity by proximity.",
        ),
        artifact_binding_row(
            "canonical_route_semantic_preservation",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::RouteAudit,
            "route_continuation_audit",
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF_LOCAL,
            &semantic_preservation.report_id,
            &semantic_preservation.report_digest,
            ArtifactProjection {
                machine_identity_id: semantic_machine_identity_id(&semantic_preservation),
                canonical_model_id: semantic_canonical_model_id(&semantic_preservation),
                canonical_weight_bundle_digest: semantic_preservation.canonical_weight_bundle_digest.clone(),
                canonical_weight_primary_artifact_sha256: semantic_preservation
                    .canonical_weight_primary_artifact_sha256
                    .clone(),
                canonical_route_id: semantic_canonical_route_id(&semantic_preservation),
                canonical_route_descriptor_digest: semantic_canonical_route_digest(
                    &semantic_preservation,
                ),
                continuation_contract_id: semantic_continuation_contract_id(&semantic_preservation),
                continuation_contract_digest: semantic_continuation_contract_digest(
                    &semantic_preservation,
                ),
            },
            semantic_preservation.semantic_preservation_status.as_deref() == Some("green")
                && semantic_preservation
                    .canonical_identity_review
                    .as_ref()
                    .is_some_and(|row| row.canonical_identity_green),
            &canonical_machine_tuple,
            "the semantic-preservation audit already carries the route and continuation identity review and is locked here as the continuation-bearing route audit surface.",
        ),
        artifact_binding_row(
            "carrier_split_contract",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Gate,
            "carrier_split_contract",
            TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF_LOCAL,
            &carrier_split.report_id,
            &carrier_split.report_digest,
            ArtifactProjection {
                machine_identity_id: carrier_split.machine_identity_id.clone(),
                canonical_model_id: None,
                canonical_weight_bundle_digest: None,
                canonical_weight_primary_artifact_sha256: None,
                canonical_route_id: carrier_split.canonical_route_id.clone(),
                canonical_route_descriptor_digest: None,
                continuation_contract_id: None,
                continuation_contract_digest: None,
            },
            carrier_split.carrier_split_status.as_deref() == Some("green")
                && carrier_split.carrier_split_publication_complete
                && carrier_split.carrier_collapse_refused
                && carrier_split
                    .primary_carrier_rows
                    .iter()
                    .all(|row| row.carrier_truth_green),
            &canonical_machine_tuple,
            "the carrier-split contract keeps direct and resumable truth classes explicit and is bound here so mixed-carrier recomposition stays machine-checkable.",
        ),
        artifact_binding_row(
            "control_plane_decision_provenance_proof",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Proof,
            "control_plane_proof",
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF_LOCAL,
            &control_plane.report_id,
            &control_plane.report_digest,
            ArtifactProjection {
                machine_identity_id: control_plane.machine_identity_id.clone(),
                canonical_model_id: control_plane.canonical_model_id.clone(),
                canonical_weight_bundle_digest: control_plane.canonical_weight_bundle_digest.clone(),
                canonical_weight_primary_artifact_sha256: control_plane
                    .canonical_weight_primary_artifact_sha256
                    .clone(),
                canonical_route_id: control_plane.canonical_route_id.clone(),
                canonical_route_descriptor_digest: control_plane
                    .canonical_route_descriptor_digest
                    .clone(),
                continuation_contract_id: control_plane.continuation_contract_id.clone(),
                continuation_contract_digest: control_plane.continuation_contract_digest.clone(),
            },
            control_plane.control_plane_ownership_green
                && control_plane.decision_provenance_proof_complete,
            &canonical_machine_tuple,
            "the control-plane proof still carries only a partial tuple itself, so the lock explicitly rebinds it onto the full canonical machine tuple.",
        ),
        artifact_binding_row(
            "universal_machine_proof_rebinding",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Proof,
            "resumable_universal_proof",
            TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF_LOCAL,
            &proof_rebinding.report_id,
            &proof_rebinding.report_digest,
            top_level_projection(&proof_rebinding),
            proof_rebinding.proof_rebinding_complete,
            &canonical_machine_tuple,
            "the universal-machine proof rebinding already carries most of the tuple and is explicitly frozen here as a resumable proof projection of the same machine.",
        ),
        artifact_binding_row(
            "universality_witness_suite_reissue",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Witness,
            "resumable_witness_suite",
            TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF_LOCAL,
            &witness_suite.report_id,
            &witness_suite.report_digest,
            top_level_projection(&witness_suite),
            witness_suite.witness_suite_reissued,
            &canonical_machine_tuple,
            "the witness-suite reissue is explicitly rebound here so exact and refusal-boundary witness families cannot drift onto a second effective machine.",
        ),
        artifact_binding_row(
            "canonical_route_universal_substrate_gate",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Gate,
            "universal_substrate_gate",
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF_LOCAL,
            &universal_substrate_gate.report_id,
            &universal_substrate_gate.report_digest,
            top_level_projection(&universal_substrate_gate),
            universal_substrate_gate.bounded_universality_story_carried
                && universal_substrate_gate.gate_status.as_deref() == Some("green"),
            &canonical_machine_tuple,
            "the universal-substrate gate is explicitly locked to the same machine tuple so the universality story cannot drift away from the route and proof surfaces below it.",
        ),
        artifact_binding_row(
            "universality_portability_minimality_matrix",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Gate,
            "portability_minimality_matrix",
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF_LOCAL,
            &portability_matrix.report_id,
            &portability_matrix.report_digest,
            top_level_projection(&portability_matrix),
            portability_matrix.matrix_status.as_deref() == Some("green")
                && portability_matrix.served_suppression_boundary_preserved,
            &canonical_machine_tuple,
            "the portability and minimality matrix is explicitly locked to the same machine so later machine-class or route drift stays visible.",
        ),
        artifact_binding_row(
            "rebased_universality_verdict_split",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Closeout,
            "rebased_verdict_split",
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF_LOCAL,
            &rebased_verdict.report_id,
            &rebased_verdict.report_digest,
            top_level_projection(&rebased_verdict),
            rebased_verdict.rebase_claim_allowed
                && rebased_verdict
                    .verdict_split_status
                    .as_deref()
                    == Some("theory_green_operator_green_served_suppressed"),
            &canonical_machine_tuple,
            "the rebased verdict split is explicitly locked to the same machine so theory/operator truth cannot drift onto a different route or continuation contract.",
        ),
        artifact_binding_row(
            "turing_completeness_closeout",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Closeout,
            "turing_closeout",
            TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
            &turing_closeout.report_id,
            &turing_closeout.report_digest,
            bound_projection(&turing_closeout.machine_identity_binding),
            turing_closeout_green,
            &canonical_machine_tuple,
            "the post-article Turing closeout is one proof-bearing closeout that must stay bound to the same canonical tuple while still leaving the final closure bundle separate.",
        ),
        artifact_binding_row(
            "plugin_invocation_receipts",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Receipt,
            "plugin_invocation_receipt_projection",
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL,
            &invocation_receipts.report_id,
            &invocation_receipts.report_digest,
            bound_projection(&invocation_receipts.machine_identity_binding),
            invocation_receipts.contract_green,
            &canonical_machine_tuple,
            "plugin invocation receipts already carry the full tuple and are locked here as one receipt-bearing projection of the same machine.",
        ),
        artifact_binding_row(
            "weighted_plugin_controller_trace_eval",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Controller,
            "weighted_controller_trace_projection",
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
            &controller_eval.report_id,
            &controller_eval.report_digest,
            bound_projection(&controller_eval.machine_identity_binding),
            controller_eval.contract_green && controller_eval.weighted_plugin_control_allowed,
            &canonical_machine_tuple,
            "the weighted controller trace already carries the full tuple and is locked here as the controller-bearing projection above the same canonical machine.",
        ),
        artifact_binding_row(
            "plugin_conformance_benchmark_harness",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::BenchmarkRoute,
            "plugin_conformance_benchmark_projection",
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
            &conformance_eval.report_id,
            &conformance_eval.report_digest,
            bound_projection(&conformance_eval.machine_identity_binding),
            conformance_eval.contract_green,
            &canonical_machine_tuple,
            "the plugin conformance benchmark harness already carries the full tuple and is locked here as the benchmark-bearing capability projection of the same machine.",
        ),
        artifact_binding_row(
            "plugin_authority_gate",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Gate,
            "plugin_authority_gate_projection",
            TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
            &authority_gate.report_id,
            &authority_gate.report_digest,
            bound_projection(&authority_gate.machine_identity_binding),
            authority_gate.contract_green,
            &canonical_machine_tuple,
            "the authority gate already carries the full tuple and is locked here so trust-tier and publication posture stay tied to the same machine identity.",
        ),
        artifact_binding_row(
            "bounded_weighted_plugin_platform_closeout",
            TassadarPostArticleCanonicalMachineArtifactScopeKind::Closeout,
            "bounded_plugin_platform_closeout_projection",
            TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
            &platform_closeout.report_id,
            &platform_closeout.report_digest,
            bound_projection(&platform_closeout.machine_identity_binding),
            platform_closeout_green,
            &canonical_machine_tuple,
            "the bounded weighted plugin-platform closeout already carries the full tuple and is locked here as the highest current capability-bearing closeout on the same machine.",
        ),
    ];

    let machine_identity_drift_present = artifact_binding_rows.iter().any(|row| {
        row.declared_machine_identity_id
            .as_deref()
            .is_some_and(|value| value != canonical_machine_tuple.machine_identity_id)
    });
    let model_or_weight_drift_present = artifact_binding_rows.iter().any(|row| {
        row.declared_canonical_model_id
            .as_deref()
            .is_some_and(|value| value != canonical_machine_tuple.canonical_model_id)
            || row
                .declared_canonical_weight_bundle_digest
                .as_deref()
                .is_some_and(|value| {
                    value != canonical_machine_tuple.canonical_weight_bundle_digest
                })
            || row
                .declared_canonical_weight_primary_artifact_sha256
                .as_deref()
                .is_some_and(|value| {
                    value != canonical_machine_tuple.canonical_weight_primary_artifact_sha256
                })
    });
    let route_or_continuation_drift_present = artifact_binding_rows.iter().any(|row| {
        row.declared_canonical_route_id
            .as_deref()
            .is_some_and(|value| value != canonical_machine_tuple.canonical_route_id)
            || row
                .declared_canonical_route_descriptor_digest
                .as_deref()
                .is_some_and(|value| {
                    value != canonical_machine_tuple.canonical_route_descriptor_digest
                })
            || row
                .declared_continuation_contract_id
                .as_deref()
                .is_some_and(|value| value != canonical_machine_tuple.continuation_contract_id)
            || row
                .declared_continuation_contract_digest
                .as_deref()
                .is_some_and(|value| value != canonical_machine_tuple.continuation_contract_digest)
    });
    let carrier_machine_ids = carrier_split
        .primary_carrier_rows
        .iter()
        .map(|row| row.machine_identity_id.clone())
        .collect::<BTreeSet<_>>();
    let dual_truth_carrier_recomposition_present = !carrier_split.carrier_collapse_refused
        || !carrier_split.carrier_split_publication_complete
        || carrier_machine_ids.len() != 1
        || carrier_machine_ids
            .iter()
            .next()
            .is_some_and(|value| value != &canonical_machine_tuple.machine_identity_id)
        || carrier_split
            .primary_carrier_rows
            .iter()
            .any(|row| !row.carrier_truth_green);
    let plugin_tuple_drift_present = !tuple_matches_bound(
        &invocation_receipts.machine_identity_binding,
        &canonical_machine_tuple,
    ) || !tuple_matches_bound(
        &controller_eval.machine_identity_binding,
        &canonical_machine_tuple,
    ) || !tuple_matches_bound(
        &conformance_eval.machine_identity_binding,
        &canonical_machine_tuple,
    ) || !tuple_matches_bound(
        &authority_gate.machine_identity_binding,
        &canonical_machine_tuple,
    ) || !tuple_matches_bound(
        &platform_closeout.machine_identity_binding,
        &canonical_machine_tuple,
    );
    let publication_or_served_overread_present = platform_closeout.plugin_publication_allowed
        || platform_closeout.served_public_universality_allowed
        || platform_closeout.arbitrary_software_capability_allowed
        || authority_gate.plugin_publication_allowed
        || authority_gate.served_public_universality_allowed
        || authority_gate.arbitrary_software_capability_allowed;
    let closure_bundle_embedded_here = false;

    let invalidation_rows = vec![
        invalidation_row(
            "machine_identity_drift_present",
            machine_identity_drift_present,
            collect_refs(&artifact_binding_rows),
            "any machine-identity mismatch across bound artifacts invalidates the canonical machine lock.",
        ),
        invalidation_row(
            "model_or_weight_drift_present",
            model_or_weight_drift_present,
            collect_refs(&artifact_binding_rows),
            "any model-id or weight-digest drift across bound artifacts invalidates the canonical machine lock.",
        ),
        invalidation_row(
            "route_or_continuation_drift_present",
            route_or_continuation_drift_present,
            collect_refs(&artifact_binding_rows),
            "any route-id, route-digest, or continuation-contract drift across bound artifacts invalidates the canonical machine lock.",
        ),
        invalidation_row(
            "dual_truth_carrier_recomposition_present",
            dual_truth_carrier_recomposition_present,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF_LOCAL,
            )],
            "collapsing direct and resumable truth carriers back into one ambient machine invalidates the lock instead of creating a larger implied machine.",
        ),
        invalidation_row(
            "plugin_receipt_or_controller_tuple_drift_present",
            plugin_tuple_drift_present,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
                ),
            ],
            "plugin receipts, controller traces, conformance evidence, authority posture, and bounded platform closeout must stay on the same tuple instead of floating on a second machine.",
        ),
        invalidation_row(
            "publication_or_served_overread_present",
            publication_or_served_overread_present,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
                ),
            ],
            "plugin publication, served/public universality, and arbitrary software capability remain out of scope here and would invalidate the lock if implied.",
        ),
        invalidation_row(
            "closure_bundle_embedded_here_present",
            closure_bundle_embedded_here,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
            )],
            "the canonical machine identity lock is not the final closure bundle and must keep that separation explicit.",
        ),
    ];

    let one_canonical_machine_named = !canonical_machine_tuple.machine_identity_id.is_empty()
        && !canonical_machine_tuple.canonical_model_id.is_empty()
        && !canonical_machine_tuple
            .canonical_weight_bundle_digest
            .is_empty()
        && !canonical_machine_tuple
            .canonical_weight_primary_artifact_sha256
            .is_empty()
        && !canonical_machine_tuple.canonical_route_id.is_empty()
        && !canonical_machine_tuple
            .canonical_route_descriptor_digest
            .is_empty()
        && !canonical_machine_tuple.continuation_contract_id.is_empty()
        && !canonical_machine_tuple
            .continuation_contract_digest
            .is_empty()
        && !canonical_machine_tuple.carrier_class_id.is_empty();
    let mixed_carrier_evidence_bundle_refused = !dual_truth_carrier_recomposition_present;
    let legacy_projection_binding_complete = artifact_binding_rows
        .iter()
        .filter(|row| !row.self_carries_full_tuple)
        .all(|row| row.bound_by_lock && row.green);
    let route_and_proof_rows_green = artifact_binding_rows
        .iter()
        .filter(|row| {
            matches!(
                row.scope_kind,
                TassadarPostArticleCanonicalMachineArtifactScopeKind::Bridge
                    | TassadarPostArticleCanonicalMachineArtifactScopeKind::BenchmarkRoute
                    | TassadarPostArticleCanonicalMachineArtifactScopeKind::RouteAudit
                    | TassadarPostArticleCanonicalMachineArtifactScopeKind::Proof
                    | TassadarPostArticleCanonicalMachineArtifactScopeKind::Witness
                    | TassadarPostArticleCanonicalMachineArtifactScopeKind::Gate
            )
        })
        .all(|row| row.green);
    let plugin_rows_green = artifact_binding_rows
        .iter()
        .filter(|row| {
            matches!(
                row.scope_kind,
                TassadarPostArticleCanonicalMachineArtifactScopeKind::Receipt
                    | TassadarPostArticleCanonicalMachineArtifactScopeKind::Controller
                    | TassadarPostArticleCanonicalMachineArtifactScopeKind::Closeout
            )
        })
        .all(|row| row.green);
    let suppression_boundary_preserved = !platform_closeout.plugin_publication_allowed
        && !platform_closeout.served_public_universality_allowed
        && !platform_closeout.arbitrary_software_capability_allowed
        && !authority_gate.plugin_publication_allowed
        && !authority_gate.served_public_universality_allowed
        && !authority_gate.arbitrary_software_capability_allowed;

    let validation_rows = vec![
        validation_row(
            "one_canonical_machine_named",
            one_canonical_machine_named,
            vec![
                String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF_LOCAL,
                ),
            ],
            "one globally named canonical machine tuple now exists over model id, weight digest, route digest, continuation contract, and the transformer-owned carrier class.",
        ),
        validation_row(
            "route_and_proof_chain_bound",
            route_and_proof_rows_green,
            collect_refs(&artifact_binding_rows),
            "route, benchmark, proof, witness, and gate surfaces are explicitly bound to the canonical tuple instead of inheriting identity implicitly.",
        ),
        validation_row(
            "plugin_receipts_controller_and_closeout_bound",
            plugin_rows_green,
            collect_refs(&artifact_binding_rows),
            "plugin receipts, controller traces, conformance evidence, authority posture, and bounded platform closeout are explicitly bound to the same canonical tuple.",
        ),
        validation_row(
            "legacy_projection_binding_complete",
            legacy_projection_binding_complete,
            collect_refs(&artifact_binding_rows),
            "legacy partial-tuple artifacts are explicitly rebound by the lock instead of relying on implied inheritance from adjacent issues.",
        ),
        validation_row(
            "mixed_carrier_evidence_bundle_refused",
            mixed_carrier_evidence_bundle_refused,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF_LOCAL,
            )],
            "mixed direct-versus-resumable evidence bundles stay explicit and fail-closed instead of silently recomposing one larger machine.",
        ),
        validation_row(
            "dependency_rows_green",
            dependency_rows.iter().all(|row| row.satisfied),
            dependency_rows
                .iter()
                .flat_map(|row| row.source_refs.clone())
                .collect(),
            "the declared dependency rows from the supporting-material surfaces remain satisfied and machine-checkable.",
        ),
        validation_row(
            "suppression_boundary_preserved",
            suppression_boundary_preserved,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
                ),
            ],
            "plugin publication, served/public universality, and arbitrary software capability remain explicitly out of scope while the lock is green.",
        ),
        validation_row(
            "closure_bundle_separation_preserved",
            !closure_bundle_embedded_here
                && turing_closeout.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID
                && platform_closeout.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
                ),
            ],
            "the machine-identity lock stays separate from the final claim-bearing closure bundle, which remains deferred to `TAS-215`.",
        ),
    ];

    let rebase_claim_allowed =
        rebased_verdict.rebase_claim_allowed && platform_closeout.rebase_claim_allowed;
    let plugin_capability_claim_allowed = platform_closeout.plugin_capability_claim_allowed;
    let weighted_plugin_control_allowed = platform_closeout.weighted_plugin_control_allowed
        && controller_eval.weighted_plugin_control_allowed;
    let plugin_publication_allowed = platform_closeout.plugin_publication_allowed;
    let served_public_universality_allowed = platform_closeout.served_public_universality_allowed;
    let arbitrary_software_capability_allowed =
        platform_closeout.arbitrary_software_capability_allowed;
    let lock_green = validation_rows.iter().all(|row| row.green)
        && invalidation_rows.iter().all(|row| !row.present);
    let lock_status = if lock_green {
        TassadarPostArticleCanonicalMachineIdentityLockStatus::Green
    } else {
        TassadarPostArticleCanonicalMachineIdentityLockStatus::Blocked
    };

    let mut report = TassadarPostArticleCanonicalMachineIdentityLockReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_canonical_machine_identity_lock.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_CHECKER_REF,
        ),
        transformer_anchor_contract_ref: String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
        tcm_v1_model_ref: String::from(TASSADAR_TCM_V1_MODEL_REF_LOCAL),
        article_equivalence_acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF_LOCAL,
        ),
        bridge_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF_LOCAL,
        ),
        canonical_route_semantic_preservation_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF_LOCAL,
        ),
        carrier_split_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF_LOCAL,
        ),
        control_plane_decision_provenance_proof_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF_LOCAL,
        ),
        universal_machine_proof_rebinding_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF_LOCAL,
        ),
        universality_witness_suite_reissue_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF_LOCAL,
        ),
        canonical_route_universal_substrate_gate_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF_LOCAL,
        ),
        universality_portability_minimality_matrix_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF_LOCAL,
        ),
        rebased_universality_verdict_split_report_ref: String::from(
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF_LOCAL,
        ),
        turing_completeness_closeout_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
        ),
        plugin_invocation_receipts_and_replay_classes_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL,
        ),
        weighted_plugin_controller_trace_eval_report_ref: String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
        ),
        plugin_conformance_sandbox_and_benchmark_harness_eval_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
        ),
        plugin_authority_promotion_publication_and_trust_tier_gate_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
        ),
        bounded_weighted_plugin_platform_closeout_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
        ),
        post_article_turing_audit_ref: String::from(POST_ARTICLE_TURING_AUDIT_REF),
        plugin_system_turing_audit_ref: String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
        canonical_machine_lock_contract: transformer_contract,
        supporting_material_rows,
        dependency_rows,
        canonical_machine_tuple,
        artifact_binding_rows,
        invalidation_rows,
        validation_rows,
        lock_status,
        lock_green,
        one_canonical_machine_named,
        mixed_carrier_evidence_bundle_refused,
        legacy_projection_binding_complete,
        closure_bundle_embedded_here,
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        claim_boundary: String::from(
            "this contract freezes one canonical machine identity lock across the post-article bridge, route audits, proof and witness surfaces, plugin receipts, controller traces, conformance harnesses, authority posture, and bounded platform closeout. It binds each cited artifact to one globally named tuple over model id, weight digest, route digest, continuation contract, and a transformer-owned carrier class, explicitly rebinds legacy partial-tuple artifacts through machine-readable lock rows instead of implied inheritance, refuses mixed-carrier recomposition, and still keeps plugin publication, served/public universality, arbitrary software capability, and the final claim-bearing closure bundle out of scope here.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article canonical machine identity lock keeps supporting_materials={}/{}, dependency_rows={}/{}, artifact_binding_rows={}/{}, invalidation_rows={} present={}, validation_rows={}/{}, lock_status={:?}, and closure_bundle_issue_id=`{}`.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report.supporting_material_rows.len(),
        report
            .dependency_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report.dependency_rows.len(),
        report
            .artifact_binding_rows
            .iter()
            .filter(|row| row.green)
            .count(),
        report.artifact_binding_rows.len(),
        report.invalidation_rows.len(),
        report.invalidation_rows.iter().filter(|row| row.present).count(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.lock_status,
        report.closure_bundle_issue_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_canonical_machine_identity_lock|",
        &report,
    );
    Ok(report)
}

#[derive(Clone, Debug, Default)]
struct ArtifactProjection {
    machine_identity_id: Option<String>,
    canonical_model_id: Option<String>,
    canonical_weight_bundle_digest: Option<String>,
    canonical_weight_primary_artifact_sha256: Option<String>,
    canonical_route_id: Option<String>,
    canonical_route_descriptor_digest: Option<String>,
    continuation_contract_id: Option<String>,
    continuation_contract_digest: Option<String>,
}

fn top_level_projection(input: &TopLevelIdentityArtifactInput) -> ArtifactProjection {
    ArtifactProjection {
        machine_identity_id: top_machine_identity_id(input),
        canonical_model_id: top_canonical_model_id(input),
        canonical_weight_bundle_digest: input.canonical_weight_bundle_digest.clone(),
        canonical_weight_primary_artifact_sha256: input
            .canonical_weight_primary_artifact_sha256
            .clone(),
        canonical_route_id: top_canonical_route_id(input),
        canonical_route_descriptor_digest: top_route_descriptor_digest(input),
        continuation_contract_id: top_continuation_contract_id(input),
        continuation_contract_digest: top_continuation_contract_digest(input),
    }
}

fn bound_projection(input: &MachineIdentityBindingInput) -> ArtifactProjection {
    ArtifactProjection {
        machine_identity_id: Some(input.machine_identity_id.clone()),
        canonical_model_id: Some(input.canonical_model_id.clone()),
        canonical_weight_bundle_digest: Some(input.canonical_weight_bundle_digest.clone()),
        canonical_weight_primary_artifact_sha256: Some(
            input.canonical_weight_primary_artifact_sha256.clone(),
        ),
        canonical_route_id: Some(input.canonical_route_id.clone()),
        canonical_route_descriptor_digest: Some(input.canonical_route_descriptor_digest.clone()),
        continuation_contract_id: Some(input.continuation_contract_id.clone()),
        continuation_contract_digest: Some(input.continuation_contract_digest.clone()),
    }
}

fn artifact_binding_row(
    binding_id: &str,
    scope_kind: TassadarPostArticleCanonicalMachineArtifactScopeKind,
    carrier_projection_id: &str,
    source_ref: &str,
    source_artifact_id: &str,
    source_artifact_digest: &str,
    projection: ArtifactProjection,
    prerequisite_green: bool,
    canonical_machine_tuple: &TassadarPostArticleCanonicalMachineIdentityTuple,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineArtifactBindingRow {
    let mut source_tuple_field_ids = Vec::new();
    maybe_push_field(
        &mut source_tuple_field_ids,
        "machine_identity_id",
        projection.machine_identity_id.as_ref(),
    );
    maybe_push_field(
        &mut source_tuple_field_ids,
        "canonical_model_id",
        projection.canonical_model_id.as_ref(),
    );
    maybe_push_field(
        &mut source_tuple_field_ids,
        "canonical_weight_bundle_digest",
        projection.canonical_weight_bundle_digest.as_ref(),
    );
    maybe_push_field(
        &mut source_tuple_field_ids,
        "canonical_weight_primary_artifact_sha256",
        projection.canonical_weight_primary_artifact_sha256.as_ref(),
    );
    maybe_push_field(
        &mut source_tuple_field_ids,
        "canonical_route_id",
        projection.canonical_route_id.as_ref(),
    );
    maybe_push_field(
        &mut source_tuple_field_ids,
        "canonical_route_descriptor_digest",
        projection.canonical_route_descriptor_digest.as_ref(),
    );
    maybe_push_field(
        &mut source_tuple_field_ids,
        "continuation_contract_id",
        projection.continuation_contract_id.as_ref(),
    );
    maybe_push_field(
        &mut source_tuple_field_ids,
        "continuation_contract_digest",
        projection.continuation_contract_digest.as_ref(),
    );
    let mut lock_projected_field_ids = CORE_TUPLE_FIELD_IDS
        .iter()
        .filter(|field| !source_tuple_field_ids.iter().any(|value| value == *field))
        .map(|field| String::from(*field))
        .collect::<Vec<_>>();
    lock_projected_field_ids.push(String::from("carrier_class_id"));
    let self_carries_full_tuple = CORE_TUPLE_FIELD_IDS
        .iter()
        .all(|field| source_tuple_field_ids.iter().any(|value| value == field));
    let source_tuple_field_count = source_tuple_field_ids.len();
    let tuple_matches_canonical_lock = prerequisite_green
        && matches_opt(
            projection.machine_identity_id.as_deref(),
            &canonical_machine_tuple.machine_identity_id,
        )
        && matches_opt(
            projection.canonical_model_id.as_deref(),
            &canonical_machine_tuple.canonical_model_id,
        )
        && matches_opt(
            projection.canonical_weight_bundle_digest.as_deref(),
            &canonical_machine_tuple.canonical_weight_bundle_digest,
        )
        && matches_opt(
            projection
                .canonical_weight_primary_artifact_sha256
                .as_deref(),
            &canonical_machine_tuple.canonical_weight_primary_artifact_sha256,
        )
        && matches_opt(
            projection.canonical_route_id.as_deref(),
            &canonical_machine_tuple.canonical_route_id,
        )
        && matches_opt(
            projection.canonical_route_descriptor_digest.as_deref(),
            &canonical_machine_tuple.canonical_route_descriptor_digest,
        )
        && matches_opt(
            projection.continuation_contract_id.as_deref(),
            &canonical_machine_tuple.continuation_contract_id,
        )
        && matches_opt(
            projection.continuation_contract_digest.as_deref(),
            &canonical_machine_tuple.continuation_contract_digest,
        );
    let bound_by_lock = prerequisite_green && !source_artifact_digest.is_empty();
    let green = prerequisite_green && tuple_matches_canonical_lock && bound_by_lock;

    TassadarPostArticleCanonicalMachineArtifactBindingRow {
        binding_id: String::from(binding_id),
        scope_kind,
        carrier_projection_id: String::from(carrier_projection_id),
        source_ref: String::from(source_ref),
        source_artifact_id: String::from(source_artifact_id),
        source_artifact_digest: String::from(source_artifact_digest),
        declared_machine_identity_id: projection.machine_identity_id,
        declared_canonical_model_id: projection.canonical_model_id,
        declared_canonical_weight_bundle_digest: projection.canonical_weight_bundle_digest,
        declared_canonical_weight_primary_artifact_sha256: projection
            .canonical_weight_primary_artifact_sha256,
        declared_canonical_route_id: projection.canonical_route_id,
        declared_canonical_route_descriptor_digest: projection.canonical_route_descriptor_digest,
        declared_continuation_contract_id: projection.continuation_contract_id,
        declared_continuation_contract_digest: projection.continuation_contract_digest,
        source_tuple_field_ids,
        lock_projected_field_ids,
        self_carries_full_tuple,
        bound_by_lock,
        tuple_matches_canonical_lock,
        green,
        detail: format!(
            "{} source_tuple_fields={} projected_fields={} carrier_projection_id=`{}`.",
            detail,
            source_tuple_field_count,
            CORE_TUPLE_FIELD_IDS.len() + 1 - source_tuple_field_count,
            carrier_projection_id,
        ),
    }
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleCanonicalMachineSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineSupportingMaterialRow {
    TassadarPostArticleCanonicalMachineSupportingMaterialRow {
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
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineDependencyRow {
    TassadarPostArticleCanonicalMachineDependencyRow {
        dependency_id: String::from(dependency_id),
        satisfied,
        source_refs,
        detail: String::from(detail),
    }
}

fn invalidation_row(
    invalidation_id: &str,
    present: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineInvalidationRow {
    TassadarPostArticleCanonicalMachineInvalidationRow {
        invalidation_id: String::from(invalidation_id),
        present,
        source_refs,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineValidationRow {
    TassadarPostArticleCanonicalMachineValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn maybe_push_field(target: &mut Vec<String>, field_id: &str, value: Option<&String>) {
    if value.is_some_and(|value| !value.is_empty()) {
        target.push(String::from(field_id));
    }
}

fn matches_opt(value: Option<&str>, canonical: &str) -> bool {
    value.is_none_or(|value| value == canonical)
}

fn top_machine_identity_id(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    input.machine_identity_id.clone().or_else(|| {
        input
            .canonical_identity_review
            .as_ref()
            .map(|row| row.machine_identity_id.clone())
    })
}

fn top_canonical_model_id(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    input.canonical_model_id.clone().or_else(|| {
        input
            .canonical_identity_review
            .as_ref()
            .map(|row| row.canonical_model_id.clone())
    })
}

fn top_canonical_route_id(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    input.canonical_route_id.clone().or_else(|| {
        input
            .canonical_identity_review
            .as_ref()
            .map(|row| row.canonical_route_id.clone())
    })
}

fn top_route_descriptor_digest(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    input.canonical_route_descriptor_digest.clone().or_else(|| {
        input
            .canonical_identity_review
            .as_ref()
            .map(|row| row.canonical_route_descriptor_digest.clone())
    })
}

fn top_continuation_contract_id(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    input.continuation_contract_id.clone().or_else(|| {
        input
            .canonical_identity_review
            .as_ref()
            .map(|row| row.continuation_contract_id.clone())
    })
}

fn top_continuation_contract_digest(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    input.continuation_contract_digest.clone().or_else(|| {
        input
            .canonical_identity_review
            .as_ref()
            .map(|row| row.continuation_contract_digest.clone())
    })
}

fn semantic_machine_identity_id(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    top_machine_identity_id(input)
}

fn semantic_canonical_model_id(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    top_canonical_model_id(input)
}

fn semantic_canonical_route_id(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    top_canonical_route_id(input)
}

fn semantic_canonical_route_digest(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    top_route_descriptor_digest(input)
}

fn semantic_continuation_contract_id(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    top_continuation_contract_id(input)
}

fn semantic_continuation_contract_digest(input: &TopLevelIdentityArtifactInput) -> Option<String> {
    top_continuation_contract_digest(input)
}

fn tuple_matches_bound(
    input: &MachineIdentityBindingInput,
    canonical_machine_tuple: &TassadarPostArticleCanonicalMachineIdentityTuple,
) -> bool {
    input.machine_identity_id == canonical_machine_tuple.machine_identity_id
        && input.canonical_model_id == canonical_machine_tuple.canonical_model_id
        && input.canonical_weight_bundle_digest
            == canonical_machine_tuple.canonical_weight_bundle_digest
        && input.canonical_weight_primary_artifact_sha256
            == canonical_machine_tuple.canonical_weight_primary_artifact_sha256
        && input.canonical_route_id == canonical_machine_tuple.canonical_route_id
        && input.canonical_route_descriptor_digest
            == canonical_machine_tuple.canonical_route_descriptor_digest
        && input.continuation_contract_id == canonical_machine_tuple.continuation_contract_id
        && input.continuation_contract_digest
            == canonical_machine_tuple.continuation_contract_digest
}

fn collect_refs(rows: &[TassadarPostArticleCanonicalMachineArtifactBindingRow]) -> Vec<String> {
    rows.iter().map(|row| row.source_ref.clone()).collect()
}

#[must_use]
pub fn tassadar_post_article_canonical_machine_identity_lock_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF)
}

pub fn write_tassadar_post_article_canonical_machine_identity_lock_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCanonicalMachineIdentityLockReport,
    TassadarPostArticleCanonicalMachineIdentityLockReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCanonicalMachineIdentityLockReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_canonical_machine_identity_lock_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCanonicalMachineIdentityLockReportError::Write {
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

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleCanonicalMachineIdentityLockReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleCanonicalMachineIdentityLockReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalMachineIdentityLockReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_repo_json_test<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleCanonicalMachineIdentityLockReportError> {
    read_repo_json(relative_path)
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_machine_identity_lock_report, read_repo_json_test,
        tassadar_post_article_canonical_machine_identity_lock_report_path,
        write_tassadar_post_article_canonical_machine_identity_lock_report,
        TassadarPostArticleCanonicalMachineIdentityLockReport,
        TassadarPostArticleCanonicalMachineIdentityLockStatus,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn post_article_canonical_machine_identity_lock_report_keeps_frontier_frozen() {
        let report =
            build_tassadar_post_article_canonical_machine_identity_lock_report().expect("report");

        assert_eq!(
            report.lock_status,
            TassadarPostArticleCanonicalMachineIdentityLockStatus::Green
        );
        assert!(report.lock_green);
        assert_eq!(report.supporting_material_rows.len(), 8);
        assert_eq!(report.dependency_rows.len(), 6);
        assert_eq!(report.artifact_binding_rows.len(), 16);
        assert_eq!(report.invalidation_rows.len(), 7);
        assert_eq!(report.validation_rows.len(), 8);
        assert!(report.one_canonical_machine_named);
        assert!(report.mixed_carrier_evidence_bundle_refused);
        assert!(report.legacy_projection_binding_complete);
        assert!(!report.closure_bundle_embedded_here);
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
        assert!(report.rebase_claim_allowed);
        assert!(report.plugin_capability_claim_allowed);
        assert!(report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
        assert!(report.invalidation_rows.iter().all(|row| !row.present));
        assert!(report.validation_rows.iter().all(|row| row.green));
    }

    #[test]
    fn post_article_canonical_machine_identity_lock_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_canonical_machine_identity_lock_report().expect("report");
        let committed: TassadarPostArticleCanonicalMachineIdentityLockReport =
            read_repo_json_test(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF)
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn post_article_canonical_machine_identity_lock_report_round_trips_to_disk() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("lock_report.json");
        let written = write_tassadar_post_article_canonical_machine_identity_lock_report(&path)
            .expect("write report");
        let reloaded: TassadarPostArticleCanonicalMachineIdentityLockReport =
            serde_json::from_slice(&std::fs::read(&path).expect("read written report"))
                .expect("decode report");
        assert_eq!(written, reloaded);
    }

    #[test]
    fn post_article_canonical_machine_identity_lock_report_path_resolves_inside_repo() {
        let path = tassadar_post_article_canonical_machine_identity_lock_report_path();
        assert!(path.ends_with(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF));
    }
}
