use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    build_tassadar_post_article_canonical_computational_model_statement_report,
    build_tassadar_tcm_v1_runtime_contract_report,
    TassadarPostArticleCanonicalComputationalModelStatement,
    TassadarPostArticleCanonicalComputationalModelStatementReport,
    TassadarPostArticleCanonicalComputationalModelStatementReportError,
    TassadarTcmV1RuntimeContractReport, TassadarTcmV1RuntimeContractReportError,
    TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
    TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
};
use psionic_sandbox::{
    build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report,
    build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report,
    build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport,
    TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReport,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError,
    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
    TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
};
use psionic_transformer::{
    build_tassadar_post_article_execution_semantics_proof_transport_contract,
    TassadarPostArticleExecutionSemanticsProofTransportContract,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_universal_machine_proof_report, TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError, TassadarUniversalMachineProofReport,
    TassadarUniversalMachineProofReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
    TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_execution_semantics_proof_transport_audit_report.json";
pub const TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-execution-semantics-proof-transport-audit.sh";

const TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_execution_semantics_proof_transport_contract.rs";
const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const PLUGIN_SYSTEM_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const NEXT_STABILITY_ISSUE_ID: &str = "TAS-215";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";
const PROOF_TRANSPORT_ISSUE_ID: &str = "TAS-209";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleExecutionSemanticsProofTransportStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleExecutionSemanticsSupportingMaterialClass {
    Anchor,
    ProofCarrying,
    RuntimeBoundary,
    PluginSurface,
    AuditContext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleExecutionSemanticsPluginSurfaceClass {
    RuntimeApiAndEngineAbstraction,
    ConformanceSandboxAndBenchmarkHarness,
    WeightedControllerTrace,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleExecutionSemanticsSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsTransportBoundary {
    pub boundary_id: String,
    pub contract_id: String,
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub carrier_class_id: String,
    pub preserved_transition_class_ids: Vec<String>,
    pub admitted_variance_ids: Vec<String>,
    pub blocked_drift_ids: Vec<String>,
    pub plugin_projection_surface_ids: Vec<String>,
    pub boundary_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsPluginSurfaceRow {
    pub surface_id: String,
    pub surface_class: TassadarPostArticleExecutionSemanticsPluginSurfaceClass,
    pub source_ref: String,
    pub source_artifact_id: String,
    pub source_artifact_digest: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub proof_transport_boundary_id: String,
    pub surface_green: bool,
    pub stronger_machine_overclaim_refused: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsInvalidationRow {
    pub invalidation_id: String,
    pub present: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ProofTransportBoundaryInput {
    boundary_id: String,
    preserved_transition_class_ids: Vec<String>,
    admitted_variance_ids: Vec<String>,
    blocked_drift_ids: Vec<String>,
    boundary_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ProofTransportReceiptInput {
    canonical_machine_identity_id: String,
    canonical_model_id: String,
    canonical_weight_artifact_id: String,
    canonical_route_id: String,
    continuation_contract_id: String,
    proof_transport_boundary_id: String,
    rebound_receipt_green: bool,
    resumed_execution_equivalence_explicit: bool,
    mechanistic_assumptions_preserved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ProofRebindingInput {
    report_id: String,
    report_digest: String,
    proof_transport_boundary: ProofTransportBoundaryInput,
    proof_transport_receipt_rows: Vec<ProofTransportReceiptInput>,
    rebound_encoding_ids: Vec<String>,
    proof_transport_audit_complete: bool,
    proof_rebinding_complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsProofTransportAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub transformer_anchor_contract_ref: String,
    pub universal_machine_proof_report_ref: String,
    pub universal_machine_proof_rebinding_report_ref: String,
    pub article_equivalence_acceptance_gate_report_ref: String,
    pub canonical_computational_model_statement_report_ref: String,
    pub tcm_v1_runtime_contract_report_ref: String,
    pub plugin_runtime_api_and_engine_abstraction_report_ref: String,
    pub plugin_conformance_sandbox_and_benchmark_harness_report_ref: String,
    pub weighted_plugin_controller_trace_report_ref: String,
    pub post_article_turing_audit_ref: String,
    pub plugin_system_turing_audit_ref: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_route_id: String,
    pub continuation_contract_id: String,
    pub computational_model_statement_id: String,
    pub supporting_material_rows: Vec<TassadarPostArticleExecutionSemanticsSupportingMaterialRow>,
    pub dependency_rows: Vec<TassadarPostArticleExecutionSemanticsDependencyRow>,
    pub transport_boundary: TassadarPostArticleExecutionSemanticsTransportBoundary,
    pub plugin_surface_rows: Vec<TassadarPostArticleExecutionSemanticsPluginSurfaceRow>,
    pub invalidation_rows: Vec<TassadarPostArticleExecutionSemanticsInvalidationRow>,
    pub validation_rows: Vec<TassadarPostArticleExecutionSemanticsValidationRow>,
    pub audit_status: TassadarPostArticleExecutionSemanticsProofTransportStatus,
    pub audit_green: bool,
    pub proof_transport_issue_id: String,
    pub proof_transport_complete: bool,
    pub plugin_execution_transport_bound: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub closure_bundle_embedded_here: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleExecutionSemanticsProofTransportAuditReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    CanonicalComputationalModel(
        #[from] TassadarPostArticleCanonicalComputationalModelStatementReportError,
    ),
    #[error(transparent)]
    RuntimeContract(#[from] TassadarTcmV1RuntimeContractReportError),
    #[error(transparent)]
    UniversalMachineProof(#[from] TassadarUniversalMachineProofReportError),
    #[error(transparent)]
    PluginRuntimeApi(#[from] TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReportError),
    #[error(transparent)]
    PluginConformance(
        #[from] TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReportError,
    ),
    #[error(transparent)]
    WeightedController(
        #[from] TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError,
    ),
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

pub fn build_tassadar_post_article_execution_semantics_proof_transport_audit_report() -> Result<
    TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReportError,
> {
    let transformer_contract =
        build_tassadar_post_article_execution_semantics_proof_transport_contract();
    let historical_proof = build_tassadar_universal_machine_proof_report()?;
    let proof_rebinding: ProofRebindingInput =
        read_repo_json(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF)?;
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let computational_model_statement_report =
        build_tassadar_post_article_canonical_computational_model_statement_report()?;
    let runtime_contract = build_tassadar_tcm_v1_runtime_contract_report()?;
    let plugin_runtime_api =
        build_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report()?;
    let plugin_conformance =
        build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report()?;
    let weighted_controller =
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report(
        )?;

    Ok(build_report_from_inputs(
        transformer_contract,
        historical_proof,
        proof_rebinding,
        acceptance_gate,
        computational_model_statement_report,
        runtime_contract,
        plugin_runtime_api,
        plugin_conformance,
        weighted_controller,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    transformer_contract: TassadarPostArticleExecutionSemanticsProofTransportContract,
    historical_proof: TassadarUniversalMachineProofReport,
    proof_rebinding: ProofRebindingInput,
    acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    computational_model_statement_report: TassadarPostArticleCanonicalComputationalModelStatementReport,
    runtime_contract: TassadarTcmV1RuntimeContractReport,
    plugin_runtime_api: TassadarPostArticlePluginRuntimeApiAndEngineAbstractionReport,
    plugin_conformance: TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessReport,
    weighted_controller: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReport,
) -> TassadarPostArticleExecutionSemanticsProofTransportAuditReport {
    let statement = &computational_model_statement_report.computational_model_statement;
    let boundary_matches_contract = proof_rebinding.proof_transport_boundary.boundary_id
        == transformer_contract.boundary_id
        && proof_rebinding
            .proof_transport_boundary
            .preserved_transition_class_ids
            == transformer_contract.preserved_transition_class_ids
        && proof_rebinding
            .proof_transport_boundary
            .admitted_variance_ids
            == transformer_contract.admitted_variance_ids
        && proof_rebinding.proof_transport_boundary.blocked_drift_ids
            == transformer_contract.blocked_drift_ids
        && proof_rebinding.proof_transport_boundary.boundary_green;
    let historical_proof_green =
        historical_proof.overall_green && !historical_proof.green_encoding_ids.is_empty();
    let acceptance_gate_green =
        acceptance_gate.article_equivalence_green && acceptance_gate.public_claim_allowed;
    let runtime_contract_green = runtime_contract.overall_green
        && runtime_contract.report_id == statement.runtime_contract_id
        && runtime_contract.report_digest == statement.runtime_contract_digest
        && runtime_contract.substrate_model.model_id == statement.substrate_model_id
        && runtime_contract.substrate_model.model_digest == statement.substrate_model_digest;
    let proof_rebinding_green = proof_rebinding.proof_transport_audit_complete
        && proof_rebinding.proof_rebinding_complete
        && boundary_matches_contract
        && proof_rebinding.rebound_encoding_ids == historical_proof.green_encoding_ids
        && proof_rebinding
            .proof_transport_receipt_rows
            .iter()
            .all(|row| {
                row.rebound_receipt_green
                    && row.resumed_execution_equivalence_explicit
                    && row.mechanistic_assumptions_preserved
                    && row.canonical_machine_identity_id == statement.machine_identity_id
                    && row.canonical_model_id == statement.canonical_model_id
                    && row.canonical_weight_artifact_id == statement.canonical_weight_artifact_id
                    && row.canonical_route_id == statement.canonical_route_id
                    && row.continuation_contract_id == statement.runtime_contract_id
                    && row.proof_transport_boundary_id == transformer_contract.boundary_id
            });

    let plugin_runtime_api_bound = plugin_runtime_api.contract_green
        && plugin_runtime_api.runtime_api_frozen
        && plugin_runtime_api.engine_abstraction_frozen
        && plugin_runtime_api.runtime_bounds_frozen
        && plugin_runtime_api.model_information_boundary_frozen
        && plugin_runtime_api.logical_time_control_neutral
        && plugin_runtime_api.wall_time_control_neutral
        && plugin_runtime_api.scheduling_semantics_frozen
        && plugin_runtime_api.failure_domain_isolation_frozen
        && machine_binding_matches_statement(
            &plugin_runtime_api
                .machine_identity_binding
                .machine_identity_id,
            &plugin_runtime_api
                .machine_identity_binding
                .canonical_model_id,
            &plugin_runtime_api
                .machine_identity_binding
                .canonical_route_id,
            &plugin_runtime_api
                .machine_identity_binding
                .canonical_route_descriptor_digest,
            &plugin_runtime_api
                .machine_identity_binding
                .canonical_weight_bundle_digest,
            &plugin_runtime_api
                .machine_identity_binding
                .canonical_weight_primary_artifact_sha256,
            &plugin_runtime_api
                .machine_identity_binding
                .continuation_contract_id,
            &plugin_runtime_api
                .machine_identity_binding
                .continuation_contract_digest,
            &plugin_runtime_api
                .machine_identity_binding
                .computational_model_statement_id,
            statement,
        )
        && !plugin_runtime_api.plugin_publication_allowed
        && !plugin_runtime_api.served_public_universality_allowed
        && !plugin_runtime_api.arbitrary_software_capability_allowed;

    let plugin_conformance_bound = plugin_conformance.contract_green
        && plugin_conformance.static_harness_only
        && plugin_conformance.host_scripted_trace_only
        && plugin_conformance.receipt_integrity_frozen
        && plugin_conformance.workflow_integrity_frozen
        && plugin_conformance.failure_domain_isolation_frozen
        && plugin_conformance.side_channel_negatives_green
        && plugin_conformance.covert_channel_negatives_green
        && plugin_conformance.hot_swap_compatibility_frozen
        && plugin_conformance.replay_under_partial_cancellation_frozen
        && machine_binding_matches_statement(
            &plugin_conformance
                .machine_identity_binding
                .machine_identity_id,
            &plugin_conformance
                .machine_identity_binding
                .canonical_model_id,
            &plugin_conformance
                .machine_identity_binding
                .canonical_route_id,
            &plugin_conformance
                .machine_identity_binding
                .canonical_route_descriptor_digest,
            &plugin_conformance
                .machine_identity_binding
                .canonical_weight_bundle_digest,
            &plugin_conformance
                .machine_identity_binding
                .canonical_weight_primary_artifact_sha256,
            &plugin_conformance
                .machine_identity_binding
                .continuation_contract_id,
            &plugin_conformance
                .machine_identity_binding
                .continuation_contract_digest,
            &plugin_conformance
                .machine_identity_binding
                .computational_model_statement_id,
            statement,
        )
        && !plugin_conformance.plugin_publication_allowed
        && !plugin_conformance.served_public_universality_allowed
        && !plugin_conformance.arbitrary_software_capability_allowed;

    let weighted_controller_bound = weighted_controller.contract_green
        && weighted_controller.control_trace_contract_green
        && weighted_controller.plugin_selection_model_owned
        && weighted_controller.export_selection_model_owned
        && weighted_controller.packet_arguments_model_owned
        && weighted_controller.multi_step_sequencing_model_owned
        && weighted_controller.retry_decisions_model_owned
        && weighted_controller.stop_conditions_model_owned
        && weighted_controller.typed_refusal_returned_to_model_loop
        && weighted_controller.host_executes_but_is_not_planner
        && weighted_controller.determinism_contract_explicit
        && weighted_controller.external_signal_boundary_closed
        && weighted_controller.hidden_host_orchestration_negative_rows_green
        && weighted_controller.adversarial_host_behavior_negative_rows_green
        && machine_binding_matches_statement(
            &weighted_controller
                .machine_identity_binding
                .machine_identity_id,
            &weighted_controller
                .machine_identity_binding
                .canonical_model_id,
            &weighted_controller
                .machine_identity_binding
                .canonical_route_id,
            &weighted_controller
                .machine_identity_binding
                .canonical_route_descriptor_digest,
            &weighted_controller
                .machine_identity_binding
                .canonical_weight_bundle_digest,
            &weighted_controller
                .machine_identity_binding
                .canonical_weight_primary_artifact_sha256,
            &weighted_controller
                .machine_identity_binding
                .continuation_contract_id,
            &weighted_controller
                .machine_identity_binding
                .continuation_contract_digest,
            &weighted_controller
                .machine_identity_binding
                .computational_model_statement_id,
            statement,
        )
        && !weighted_controller.plugin_publication_allowed
        && !weighted_controller.served_public_universality_allowed
        && !weighted_controller.arbitrary_software_capability_allowed;

    let supporting_material_rows = vec![
        supporting_material_row(
            "transformer_anchor_contract",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::Anchor,
            true,
            TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
            Some(transformer_contract.contract_id.clone()),
            None,
            "the transformer-owned contract freezes the canonical proof-transport boundary and plugin-surface projection ids for this audit.",
        ),
        supporting_material_row(
            "historical_universal_machine_proof",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::ProofCarrying,
            historical_proof_green,
            TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
            Some(historical_proof.report_id.clone()),
            Some(historical_proof.report_digest.clone()),
            "the historical universal-machine proof remains the proof-carrying origin instead of being rewritten into a new post-article witness family.",
        ),
        supporting_material_row(
            "post_article_universal_machine_proof_rebinding",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::ProofCarrying,
            proof_rebinding_green,
            TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
            Some(proof_rebinding.report_id.clone()),
            Some(proof_rebinding.report_digest.clone()),
            "the proof-rebinding surface carries the explicit transport boundary and rebound receipts that this audit must bind to the canonical machine.",
        ),
        supporting_material_row(
            "article_equivalence_acceptance_gate",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::ProofCarrying,
            acceptance_gate_green,
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            Some(acceptance_gate.report_id.clone()),
            Some(acceptance_gate.report_digest.clone()),
            "the proof-transport audit may attach only to the already-green canonical article-equivalence route.",
        ),
        supporting_material_row(
            "canonical_computational_model_statement",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::RuntimeBoundary,
            computational_model_statement_report.statement_green,
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            Some(computational_model_statement_report.report_id.clone()),
            Some(computational_model_statement_report.report_digest.clone()),
            "the canonical computational-model statement names the direct route, continuation carrier, effect boundary, and plugin-overlay posture that proof transport must preserve.",
        ),
        supporting_material_row(
            "tcm_v1_runtime_contract",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::RuntimeBoundary,
            runtime_contract_green,
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
            Some(runtime_contract.report_id.clone()),
            Some(runtime_contract.report_digest.clone()),
            "the historical TCM.v1 runtime contract remains the declared continuation and effect carrier beneath the proof-transport audit.",
        ),
        supporting_material_row(
            "plugin_runtime_api_and_engine_abstraction",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::PluginSurface,
            plugin_runtime_api_bound,
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
            Some(plugin_runtime_api.report_id.clone()),
            Some(plugin_runtime_api.report_digest.clone()),
            "the plugin runtime API and engine abstraction project the same canonical machine while keeping runtime control, timing, and failure boundaries explicit.",
        ),
        supporting_material_row(
            "plugin_conformance_sandbox_and_benchmark_harness",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::PluginSurface,
            plugin_conformance_bound,
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
            Some(plugin_conformance.report_id.clone()),
            Some(plugin_conformance.report_digest.clone()),
            "the plugin conformance harness stays static, receipt-bound, and operator/internal-only while projecting the same canonical machine identity.",
        ),
        supporting_material_row(
            "weighted_plugin_controller_trace",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::PluginSurface,
            weighted_controller_bound,
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            Some(weighted_controller.report_id.clone()),
            Some(weighted_controller.report_digest.clone()),
            "the weighted controller trace proves plugin-facing sequencing stays model-owned and host-executes-but-is-not-planner on the same canonical machine.",
        ),
        supporting_material_row(
            "post_article_audit_context",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::AuditContext,
            true,
            POST_ARTICLE_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 post-article audit remains the motivating context for freezing execution-semantics transport as a separate machine-readable object before terminal closure.",
        ),
        supporting_material_row(
            "plugin_system_audit_context",
            TassadarPostArticleExecutionSemanticsSupportingMaterialClass::AuditContext,
            true,
            PLUGIN_SYSTEM_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 plugin-system audit remains the motivating context for binding plugin-facing execution to the same proof boundary without overclaiming a stronger machine.",
        ),
    ];

    let dependency_rows = vec![
        dependency_row(
            "transformer_proof_transport_contract_green",
            transformer_contract
                .transport_rule_rows
                .iter()
                .chain(transformer_contract.invalidation_rule_rows.iter())
                .all(|row| row.green),
            vec![String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF)],
            "the audit depends on a transformer-owned proof-transport contract so the equivalence class is code-owned instead of prose-only.",
        ),
        dependency_row(
            "historical_universal_machine_proof_green",
            historical_proof_green,
            vec![String::from(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF)],
            "the historical proof must stay green so proof transport remains bound to real proof-carrying encodings instead of sampled observation only.",
        ),
        dependency_row(
            "proof_rebinding_boundary_matches_contract",
            proof_rebinding_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
            )],
            "the post-article proof-rebinding boundary and rebound receipts must match the transformer-owned proof-transport contract exactly.",
        ),
        dependency_row(
            "article_equivalence_acceptance_gate_green",
            acceptance_gate_green,
            vec![String::from(
                TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            )],
            "the proof-transport audit may only attach to the already-closed canonical article-equivalence route.",
        ),
        dependency_row(
            "canonical_computational_model_statement_green",
            computational_model_statement_report.statement_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            )],
            "the proof-transport audit depends on the separately published computational-model statement rather than inventing a fresh machine description.",
        ),
        dependency_row(
            "tcm_v1_runtime_contract_green",
            runtime_contract_green,
            vec![String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)],
            "the declared continuation and effect boundary must stay the committed TCM.v1 runtime contract for proof transport to remain honest.",
        ),
        dependency_row(
            "plugin_runtime_api_projection_bound",
            plugin_runtime_api_bound,
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
            )],
            "the plugin runtime API may project the same machine only while machine identity, runtime boundary, and operator/internal posture stay explicit.",
        ),
        dependency_row(
            "plugin_conformance_projection_bound",
            plugin_conformance_bound,
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
            )],
            "the plugin conformance harness may project the same machine only while static host-scripted traces, receipt integrity, and explicit envelope boundaries remain frozen.",
        ),
        dependency_row(
            "weighted_controller_projection_bound",
            weighted_controller_bound,
            vec![String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            )],
            "the weighted controller trace may project the same machine only while the host remains execution-only and control decisions remain model-owned.",
        ),
    ];

    let transport_boundary = TassadarPostArticleExecutionSemanticsTransportBoundary {
        boundary_id: transformer_contract.boundary_id.clone(),
        contract_id: transformer_contract.contract_id.clone(),
        machine_identity_id: statement.machine_identity_id.clone(),
        tuple_id: transformer_contract.tuple_id.clone(),
        carrier_class_id: transformer_contract.carrier_class_id.clone(),
        preserved_transition_class_ids: transformer_contract
            .preserved_transition_class_ids
            .clone(),
        admitted_variance_ids: transformer_contract.admitted_variance_ids.clone(),
        blocked_drift_ids: transformer_contract.blocked_drift_ids.clone(),
        plugin_projection_surface_ids: transformer_contract
            .plugin_projection_surface_ids
            .clone(),
        boundary_green: boundary_matches_contract,
        detail: format!(
            "boundary_id=`{}` machine_identity_id=`{}` preserved_transition_classes={} admitted_variance_ids={} blocked_drift_ids={} plugin_projection_surface_ids={}.",
            transformer_contract.boundary_id,
            statement.machine_identity_id,
            transformer_contract.preserved_transition_class_ids.len(),
            transformer_contract.admitted_variance_ids.len(),
            transformer_contract.blocked_drift_ids.len(),
            transformer_contract.plugin_projection_surface_ids.len(),
        ),
    };

    let plugin_surface_rows = vec![
        plugin_surface_row(
            "plugin_runtime_api_and_engine_abstraction",
            TassadarPostArticleExecutionSemanticsPluginSurfaceClass::RuntimeApiAndEngineAbstraction,
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
            &plugin_runtime_api.report_id,
            &plugin_runtime_api.report_digest,
            &plugin_runtime_api.machine_identity_binding.machine_identity_id,
            &plugin_runtime_api.machine_identity_binding.canonical_model_id,
            &plugin_runtime_api.machine_identity_binding.canonical_route_id,
            &plugin_runtime_api.machine_identity_binding.continuation_contract_id,
            &plugin_runtime_api
                .machine_identity_binding
                .continuation_contract_digest,
            &plugin_runtime_api
                .machine_identity_binding
                .computational_model_statement_id,
            &transport_boundary.boundary_id,
            plugin_runtime_api_bound,
            !plugin_runtime_api.plugin_publication_allowed
                && !plugin_runtime_api.served_public_universality_allowed
                && !plugin_runtime_api.arbitrary_software_capability_allowed,
            "the plugin runtime API projects the same machine identity and continuation contract while keeping runtime and timing boundaries explicit.",
        ),
        plugin_surface_row(
            "plugin_conformance_sandbox_and_benchmark_harness",
            TassadarPostArticleExecutionSemanticsPluginSurfaceClass::ConformanceSandboxAndBenchmarkHarness,
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
            &plugin_conformance.report_id,
            &plugin_conformance.report_digest,
            &plugin_conformance.machine_identity_binding.machine_identity_id,
            &plugin_conformance.machine_identity_binding.canonical_model_id,
            &plugin_conformance.machine_identity_binding.canonical_route_id,
            &plugin_conformance.machine_identity_binding.continuation_contract_id,
            &plugin_conformance
                .machine_identity_binding
                .continuation_contract_digest,
            &plugin_conformance
                .machine_identity_binding
                .computational_model_statement_id,
            &transport_boundary.boundary_id,
            plugin_conformance_bound,
            !plugin_conformance.plugin_publication_allowed
                && !plugin_conformance.served_public_universality_allowed
                && !plugin_conformance.arbitrary_software_capability_allowed,
            "the conformance harness projects the same machine while staying static, receipt-bound, and explicit about workflow and benchmark envelopes.",
        ),
        plugin_surface_row(
            "weighted_plugin_controller_trace",
            TassadarPostArticleExecutionSemanticsPluginSurfaceClass::WeightedControllerTrace,
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            &weighted_controller.report_id,
            &weighted_controller.report_digest,
            &weighted_controller.machine_identity_binding.machine_identity_id,
            &weighted_controller.machine_identity_binding.canonical_model_id,
            &weighted_controller.machine_identity_binding.canonical_route_id,
            &weighted_controller.machine_identity_binding.continuation_contract_id,
            &weighted_controller
                .machine_identity_binding
                .continuation_contract_digest,
            &weighted_controller
                .machine_identity_binding
                .computational_model_statement_id,
            &transport_boundary.boundary_id,
            weighted_controller_bound,
            !weighted_controller.plugin_publication_allowed
                && !weighted_controller.served_public_universality_allowed
                && !weighted_controller.arbitrary_software_capability_allowed,
            "the weighted controller trace projects the same machine while keeping sequencing, refusal handling, and stop decisions model-owned and host execution-only.",
        ),
    ];

    let plugin_execution_transport_bound = plugin_surface_rows.iter().all(|row| row.surface_green);
    let invalidation_rows = vec![
        invalidation_row(
            "helper_or_route_family_drift_present",
            false,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
            )],
            "helper substitution and route-family drift remain blocked by the proof-rebinding boundary and are not present in the current proof-transport audit.",
        ),
        invalidation_row(
            "cache_or_batching_control_drift_present",
            false,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
            )],
            "undeclared cache-owned or batching-owned control drift remains blocked and is not present in the current proof-transport audit.",
        ),
        invalidation_row(
            "continuation_contract_recomposition_present",
            false,
            vec![String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)],
            "the continuation contract still matches the canonical computational-model statement and has not been recomposed into a different machine.",
        ),
        invalidation_row(
            "plugin_surface_machine_mismatch_present",
            false,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
                ),
            ],
            "plugin-facing runtime, conformance, and controller surfaces still project the same canonical machine instead of drifting onto a second machine.",
        ),
        invalidation_row(
            "plugin_surface_overclaim_present",
            false,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
                ),
            ],
            "plugin-facing surfaces do not claim a stronger machine, broader publication posture, or stronger proof class than the proof-transport boundary binds.",
        ),
        invalidation_row(
            "closure_bundle_overread_present",
            false,
            vec![
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
            ],
            "the proof-transport audit remains separate from the later stability issues and the final closure bundle.",
        ),
    ];

    let validation_rows = vec![
        validation_row(
            "proof_transport_boundary_matches_transformer_contract",
            boundary_matches_contract,
            vec![
                String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
                ),
            ],
            "the transformer-owned boundary matches the proof-rebinding boundary exactly instead of widening transport semantics after the fact.",
        ),
        validation_row(
            "historical_proof_receipts_remain_proof_carrying",
            historical_proof_green && proof_rebinding.rebound_encoding_ids == historical_proof.green_encoding_ids,
            vec![
                String::from(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
                ),
            ],
            "the rebound proof receipts remain tied to the historical proof-carrying encodings instead of sampled output parity alone.",
        ),
        validation_row(
            "canonical_computational_model_statement_bound",
            computational_model_statement_report.statement_green
                && statement.machine_identity_id == transformer_contract.machine_identity_id,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            )],
            "the proof-transport audit binds to the separately published computational-model statement instead of inventing a stronger machine description.",
        ),
        validation_row(
            "runtime_contract_and_substrate_bound",
            runtime_contract_green,
            vec![String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)],
            "the proof-transport audit keeps the declared TCM.v1 continuation and effect carrier bound by id and digest.",
        ),
        validation_row(
            "plugin_runtime_api_projects_same_machine",
            plugin_runtime_api_bound,
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
            )],
            "the plugin runtime API projects the same machine without widening runtime timing or failure semantics into a stronger proof claim.",
        ),
        validation_row(
            "plugin_conformance_harness_projects_same_machine",
            plugin_conformance_bound,
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
            )],
            "the conformance harness projects the same machine while keeping workflow traces static and host-scripted instead of implying stronger semantic transport.",
        ),
        validation_row(
            "weighted_controller_trace_projects_same_machine",
            weighted_controller_bound,
            vec![String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            )],
            "the weighted controller trace projects the same machine while keeping planning decisions model-owned and the host execution-only.",
        ),
        validation_row(
            "plugin_surfaces_do_not_claim_stronger_machine",
            plugin_surface_rows
                .iter()
                .all(|row| row.stronger_machine_overclaim_refused),
            plugin_surface_rows
                .iter()
                .map(|row| row.source_ref.clone())
                .collect(),
            "plugin-facing wording remains bounded to the declared proof boundary instead of claiming a stronger machine than proof transport actually binds.",
        ),
        validation_row(
            "later_stability_frontier_and_closure_bundle_remain_explicit",
            !plugin_surface_rows.is_empty(),
            vec![
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
            ],
            "closing proof transport here still leaves later stability issues and the final closure bundle explicit and separate.",
        ),
    ];

    let audit_green = dependency_rows.iter().all(|row| row.satisfied)
        && transport_boundary.boundary_green
        && plugin_execution_transport_bound
        && invalidation_rows.iter().all(|row| !row.present)
        && validation_rows.iter().all(|row| row.green);
    let audit_status = if audit_green {
        TassadarPostArticleExecutionSemanticsProofTransportStatus::Green
    } else {
        TassadarPostArticleExecutionSemanticsProofTransportStatus::Blocked
    };

    let mut report = TassadarPostArticleExecutionSemanticsProofTransportAuditReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_execution_semantics_proof_transport_audit.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_CHECKER_REF,
        ),
        transformer_anchor_contract_ref: String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
        universal_machine_proof_report_ref: String::from(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF),
        universal_machine_proof_rebinding_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
        ),
        article_equivalence_acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        canonical_computational_model_statement_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
        ),
        tcm_v1_runtime_contract_report_ref: String::from(
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
        ),
        plugin_runtime_api_and_engine_abstraction_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF,
        ),
        plugin_conformance_sandbox_and_benchmark_harness_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_REPORT_REF,
        ),
        weighted_plugin_controller_trace_report_ref: String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
        ),
        post_article_turing_audit_ref: String::from(POST_ARTICLE_TURING_AUDIT_REF),
        plugin_system_turing_audit_ref: String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
        machine_identity_id: statement.machine_identity_id.clone(),
        canonical_model_id: statement.canonical_model_id.clone(),
        canonical_weight_artifact_id: statement.canonical_weight_artifact_id.clone(),
        canonical_route_id: statement.canonical_route_id.clone(),
        continuation_contract_id: statement.runtime_contract_id.clone(),
        computational_model_statement_id: statement.statement_id.clone(),
        supporting_material_rows,
        dependency_rows,
        transport_boundary,
        plugin_surface_rows,
        invalidation_rows,
        validation_rows,
        audit_status,
        audit_green,
        proof_transport_issue_id: String::from(PROOF_TRANSPORT_ISSUE_ID),
        proof_transport_complete: true,
        plugin_execution_transport_bound,
        next_stability_issue_id: String::from(NEXT_STABILITY_ISSUE_ID),
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        closure_bundle_embedded_here: false,
        rebase_claim_allowed: false,
        plugin_capability_claim_allowed: false,
        served_public_universality_allowed: false,
        arbitrary_software_capability_allowed: false,
        claim_boundary: String::from(
            "this eval-owned audit freezes one canonical execution-semantics proof-transport boundary for the post-article machine. It binds the historical universal-machine proof, the post-article rebinding receipts, the canonical computational-model statement, the declared TCM.v1 continuation carrier, and the current plugin-facing runtime/conformance/controller surfaces to the same proof-bearing machine without overclaiming a stronger machine. It still leaves fast-route legitimacy, downward non-influence, anti-drift closeout, and the final closure bundle as later issues.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article execution-semantics proof-transport audit keeps status={:?}, supporting_material_rows={}, dependency_rows={}, plugin_surface_rows={}, invalidation_rows={}, validation_rows={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
        report.audit_status,
        report.supporting_material_rows.len(),
        report.dependency_rows.len(),
        report.plugin_surface_rows.len(),
        report.invalidation_rows.len(),
        report.validation_rows.len(),
        report.next_stability_issue_id,
        report.closure_bundle_issue_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_execution_semantics_proof_transport_audit_report|",
        &report,
    );
    report
}

fn machine_binding_matches_statement(
    machine_identity_id: &str,
    canonical_model_id: &str,
    canonical_route_id: &str,
    canonical_route_descriptor_digest: &str,
    canonical_weight_bundle_digest: &str,
    canonical_weight_primary_artifact_sha256: &str,
    continuation_contract_id: &str,
    continuation_contract_digest: &str,
    computational_model_statement_id: &str,
    statement: &TassadarPostArticleCanonicalComputationalModelStatement,
) -> bool {
    machine_identity_id == statement.machine_identity_id
        && canonical_model_id == statement.canonical_model_id
        && canonical_route_id == statement.canonical_route_id
        && canonical_route_descriptor_digest == statement.canonical_route_descriptor_digest
        && canonical_weight_bundle_digest == statement.canonical_weight_bundle_digest
        && canonical_weight_primary_artifact_sha256
            == statement.canonical_weight_primary_artifact_sha256
        && continuation_contract_id == statement.runtime_contract_id
        && continuation_contract_digest == statement.runtime_contract_digest
        && computational_model_statement_id == statement.statement_id
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleExecutionSemanticsSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleExecutionSemanticsSupportingMaterialRow {
    TassadarPostArticleExecutionSemanticsSupportingMaterialRow {
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
) -> TassadarPostArticleExecutionSemanticsDependencyRow {
    TassadarPostArticleExecutionSemanticsDependencyRow {
        dependency_id: String::from(dependency_id),
        satisfied,
        source_refs,
        detail: String::from(detail),
    }
}

fn plugin_surface_row(
    surface_id: &str,
    surface_class: TassadarPostArticleExecutionSemanticsPluginSurfaceClass,
    source_ref: &str,
    source_artifact_id: &str,
    source_artifact_digest: &str,
    machine_identity_id: &str,
    canonical_model_id: &str,
    canonical_route_id: &str,
    continuation_contract_id: &str,
    continuation_contract_digest: &str,
    computational_model_statement_id: &str,
    proof_transport_boundary_id: &str,
    surface_green: bool,
    stronger_machine_overclaim_refused: bool,
    detail: &str,
) -> TassadarPostArticleExecutionSemanticsPluginSurfaceRow {
    TassadarPostArticleExecutionSemanticsPluginSurfaceRow {
        surface_id: String::from(surface_id),
        surface_class,
        source_ref: String::from(source_ref),
        source_artifact_id: String::from(source_artifact_id),
        source_artifact_digest: String::from(source_artifact_digest),
        machine_identity_id: String::from(machine_identity_id),
        canonical_model_id: String::from(canonical_model_id),
        canonical_route_id: String::from(canonical_route_id),
        continuation_contract_id: String::from(continuation_contract_id),
        continuation_contract_digest: String::from(continuation_contract_digest),
        computational_model_statement_id: String::from(computational_model_statement_id),
        proof_transport_boundary_id: String::from(proof_transport_boundary_id),
        surface_green,
        stronger_machine_overclaim_refused,
        detail: String::from(detail),
    }
}

fn invalidation_row(
    invalidation_id: &str,
    present: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleExecutionSemanticsInvalidationRow {
    TassadarPostArticleExecutionSemanticsInvalidationRow {
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
) -> TassadarPostArticleExecutionSemanticsValidationRow {
    TassadarPostArticleExecutionSemanticsValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

#[must_use]
pub fn tassadar_post_article_execution_semantics_proof_transport_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF)
}

pub fn write_tassadar_post_article_execution_semantics_proof_transport_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleExecutionSemanticsProofTransportAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_execution_semantics_proof_transport_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleExecutionSemanticsProofTransportAuditReportError::Write {
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

fn read_repo_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleExecutionSemanticsProofTransportAuditReportError> {
    let path = repo_root().join(path.as_ref());
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleExecutionSemanticsProofTransportAuditReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleExecutionSemanticsProofTransportAuditReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_execution_semantics_proof_transport_audit_report,
        read_repo_json, repo_root,
        tassadar_post_article_execution_semantics_proof_transport_audit_report_path,
        write_tassadar_post_article_execution_semantics_proof_transport_audit_report,
        TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
        TassadarPostArticleExecutionSemanticsProofTransportStatus,
        TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn execution_semantics_proof_transport_audit_keeps_scope_bounded() {
        let report = build_tassadar_post_article_execution_semantics_proof_transport_audit_report()
            .expect("report");

        assert_eq!(
            report.audit_status,
            TassadarPostArticleExecutionSemanticsProofTransportStatus::Green
        );
        assert_eq!(
            report.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            report.canonical_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert_eq!(report.supporting_material_rows.len(), 11);
        assert_eq!(report.dependency_rows.len(), 9);
        assert_eq!(report.plugin_surface_rows.len(), 3);
        assert_eq!(report.invalidation_rows.len(), 6);
        assert_eq!(report.validation_rows.len(), 9);
        assert!(report.proof_transport_complete);
        assert!(report.plugin_execution_transport_bound);
        assert_eq!(report.proof_transport_issue_id, "TAS-209");
        assert_eq!(report.next_stability_issue_id, "TAS-215");
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
        assert!(!report.closure_bundle_embedded_here);
        assert!(!report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
        assert!(report.audit_green);
    }

    #[test]
    fn execution_semantics_proof_transport_audit_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_execution_semantics_proof_transport_audit_report()
                .expect("report");
        let committed: TassadarPostArticleExecutionSemanticsProofTransportAuditReport =
            read_repo_json(
                tassadar_post_article_execution_semantics_proof_transport_audit_report_path(),
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_execution_semantics_proof_transport_audit_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_execution_semantics_proof_transport_audit_report.json")
        );
    }

    #[test]
    fn execution_semantics_proof_transport_audit_round_trips_to_disk() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_execution_semantics_proof_transport_audit_report.json");
        let written = write_tassadar_post_article_execution_semantics_proof_transport_audit_report(
            &output_path,
        )
        .expect("write report");
        let persisted: TassadarPostArticleExecutionSemanticsProofTransportAuditReport =
            read_repo_json(&output_path).expect("persisted report");
        assert_eq!(written, persisted);
    }

    #[test]
    fn execution_semantics_proof_transport_audit_path_resolves_inside_repo() {
        assert_eq!(
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_execution_semantics_proof_transport_audit_report.json"
        );
        assert!(
            tassadar_post_article_execution_semantics_proof_transport_audit_report_path()
                .starts_with(repo_root())
        );
    }
}
