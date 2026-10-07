use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF;
use psionic_transformer::{
    build_tassadar_post_article_continuation_non_computationality_contract,
    TassadarPostArticleContinuationNonComputationalityContract,
    TASSADAR_POST_ARTICLE_CONTINUATION_CHECKPOINT_SURFACE_ID,
    TASSADAR_POST_ARTICLE_CONTINUATION_INSTALLED_PROCESS_SURFACE_ID,
    TASSADAR_POST_ARTICLE_CONTINUATION_PROCESS_OBJECT_SURFACE_ID,
    TASSADAR_POST_ARTICLE_CONTINUATION_SESSION_PROCESS_SURFACE_ID,
    TASSADAR_POST_ARTICLE_CONTINUATION_SPILL_TAPE_SURFACE_ID,
    TASSADAR_POST_ARTICLE_CONTINUATION_WEIGHTED_CONTROLLER_SURFACE_ID,
};

use crate::{
    build_tassadar_execution_checkpoint_report, build_tassadar_installed_process_lifecycle_report,
    build_tassadar_post_article_canonical_machine_identity_lock_report,
    build_tassadar_post_article_execution_semantics_proof_transport_audit_report,
    build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report,
    build_tassadar_process_object_report, build_tassadar_session_process_profile_report,
    build_tassadar_spill_tape_store_report, TassadarExecutionCheckpointReport,
    TassadarExecutionCheckpointReportError, TassadarInstalledProcessLifecycleReport,
    TassadarInstalledProcessLifecycleReportError,
    TassadarPostArticleCanonicalMachineIdentityLockReport,
    TassadarPostArticleCanonicalMachineIdentityLockReportError,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    TassadarPostArticleExecutionSemanticsProofTransportAuditReportError,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError,
    TassadarProcessObjectReport, TassadarProcessObjectReportError,
    TassadarSessionProcessProfileReport, TassadarSessionProcessProfileReportError,
    TassadarSpillTapeStoreReport, TassadarSpillTapeStoreReportError,
    TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF, TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
    TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
    TASSADAR_PROCESS_OBJECT_REPORT_REF, TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
    TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
};
use psionic_runtime::{
    build_tassadar_post_article_canonical_computational_model_statement_report,
    TassadarPostArticleCanonicalComputationalModelStatementReport,
    TassadarPostArticleCanonicalComputationalModelStatementReportError,
};

pub const TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_continuation_non_computationality_contract_report.json";
pub const TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-continuation-non-computationality-contract.sh";

const TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_continuation_non_computationality_contract.rs";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";
const NEXT_STABILITY_ISSUE_ID: &str = "TAS-215";
const SUSPICIOUS_KEY_FRAGMENTS: &[&str] = &[
    "workflow",
    "planner",
    "heuristic",
    "oracle",
    "scheduler",
    "retry_policy",
    "next_action",
    "branch_policy",
    "tool_choice",
    "fallback_policy",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleContinuationNonComputationalityStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleContinuationSupportingMaterialClass {
    Anchor,
    MachineBinding,
    ContinuationSurface,
    ControllerBoundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleContinuationSurfaceKind {
    CheckpointExecution,
    SessionProcess,
    SpillTapeStore,
    DurableProcessObject,
    InstalledProcessLifecycle,
    WeightedPluginController,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationMachineIdentityBinding {
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub proof_transport_boundary_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleContinuationSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationSurfaceRow {
    pub surface_id: String,
    pub surface_kind: TassadarPostArticleContinuationSurfaceKind,
    pub source_report_ref: String,
    pub source_report_id: String,
    pub source_report_digest: String,
    pub canonical_machine_identity_id: String,
    pub structural_carrier_ids: Vec<String>,
    pub suspicious_key_hits: Vec<String>,
    pub state_transport_only: bool,
    pub exact_parity_or_typed_refusal_green: bool,
    pub expressivity_extension_blocked: bool,
    pub host_compute_relocation_detected: bool,
    pub surface_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationInvalidationRow {
    pub invalidation_id: String,
    pub present: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleContinuationNonComputationalityContractReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub transformer_anchor_contract_ref: String,
    pub canonical_machine_identity_lock_report_ref: String,
    pub canonical_computational_model_statement_report_ref: String,
    pub execution_semantics_proof_transport_audit_report_ref: String,
    pub execution_checkpoint_report_ref: String,
    pub process_object_report_ref: String,
    pub session_process_profile_report_ref: String,
    pub spill_tape_store_report_ref: String,
    pub installed_process_lifecycle_report_ref: String,
    pub weighted_plugin_controller_trace_eval_report_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub continuation_contract: TassadarPostArticleContinuationNonComputationalityContract,
    pub machine_identity_binding: TassadarPostArticleContinuationMachineIdentityBinding,
    pub supporting_material_rows: Vec<TassadarPostArticleContinuationSupportingMaterialRow>,
    pub dependency_rows: Vec<TassadarPostArticleContinuationDependencyRow>,
    pub continuation_surface_rows: Vec<TassadarPostArticleContinuationSurfaceRow>,
    pub invalidation_rows: Vec<TassadarPostArticleContinuationInvalidationRow>,
    pub validation_rows: Vec<TassadarPostArticleContinuationValidationRow>,
    pub contract_status: TassadarPostArticleContinuationNonComputationalityStatus,
    pub contract_green: bool,
    pub continuation_extends_execution_without_second_machine: bool,
    pub hidden_workflow_logic_refused: bool,
    pub continuation_expressivity_extension_blocked: bool,
    pub plugin_resume_hidden_compute_refused: bool,
    pub continuation_non_computationality_complete: bool,
    pub next_stability_issue_id: String,
    pub closure_bundle_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleContinuationNonComputationalityContractReportError {
    #[error(transparent)]
    MachineLock(#[from] TassadarPostArticleCanonicalMachineIdentityLockReportError),
    #[error(transparent)]
    ComputationalModel(#[from] TassadarPostArticleCanonicalComputationalModelStatementReportError),
    #[error(transparent)]
    ProofTransport(#[from] TassadarPostArticleExecutionSemanticsProofTransportAuditReportError),
    #[error(transparent)]
    ExecutionCheckpoint(#[from] TassadarExecutionCheckpointReportError),
    #[error(transparent)]
    ProcessObject(#[from] TassadarProcessObjectReportError),
    #[error(transparent)]
    SessionProcess(#[from] TassadarSessionProcessProfileReportError),
    #[error(transparent)]
    SpillTape(#[from] TassadarSpillTapeStoreReportError),
    #[error(transparent)]
    InstalledProcess(#[from] TassadarInstalledProcessLifecycleReportError),
    #[error(transparent)]
    WeightedController(
        #[from]
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError,
    ),
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

pub fn build_tassadar_post_article_continuation_non_computationality_contract_report() -> Result<
    TassadarPostArticleContinuationNonComputationalityContractReport,
    TassadarPostArticleContinuationNonComputationalityContractReportError,
> {
    let machine_lock = build_tassadar_post_article_canonical_machine_identity_lock_report()?;
    let computational_model =
        build_tassadar_post_article_canonical_computational_model_statement_report()?;
    let proof_transport =
        build_tassadar_post_article_execution_semantics_proof_transport_audit_report()?;
    let execution_checkpoint = build_tassadar_execution_checkpoint_report()?;
    let process_object = build_tassadar_process_object_report()?;
    let session_process = build_tassadar_session_process_profile_report()?;
    let spill_tape = build_tassadar_spill_tape_store_report()?;
    let installed_process = build_tassadar_installed_process_lifecycle_report()?;
    let weighted_controller =
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report()?;
    let continuation_contract =
        build_tassadar_post_article_continuation_non_computationality_contract();

    Ok(build_report_from_inputs(
        continuation_contract,
        machine_lock,
        computational_model,
        proof_transport,
        execution_checkpoint,
        process_object,
        session_process,
        spill_tape,
        installed_process,
        weighted_controller,
    )?)
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    continuation_contract: TassadarPostArticleContinuationNonComputationalityContract,
    machine_lock: TassadarPostArticleCanonicalMachineIdentityLockReport,
    computational_model: TassadarPostArticleCanonicalComputationalModelStatementReport,
    proof_transport: TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    execution_checkpoint: TassadarExecutionCheckpointReport,
    process_object: TassadarProcessObjectReport,
    session_process: TassadarSessionProcessProfileReport,
    spill_tape: TassadarSpillTapeStoreReport,
    installed_process: TassadarInstalledProcessLifecycleReport,
    weighted_controller: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
) -> Result<
    TassadarPostArticleContinuationNonComputationalityContractReport,
    TassadarPostArticleContinuationNonComputationalityContractReportError,
> {
    let machine_identity_binding =
        build_machine_identity_binding(&machine_lock, &computational_model, &proof_transport);
    let continuation_surface_rows = build_continuation_surface_rows(
        &machine_identity_binding,
        &execution_checkpoint,
        &process_object,
        &session_process,
        &spill_tape,
        &installed_process,
        &weighted_controller,
    )?;
    let supporting_material_rows = build_supporting_material_rows(
        &continuation_contract,
        &machine_lock,
        &computational_model,
        &proof_transport,
        &execution_checkpoint,
        &process_object,
        &session_process,
        &spill_tape,
        &installed_process,
        &weighted_controller,
    );
    let canonical_binding_green = machine_lock.lock_green
        && computational_model.statement_green
        && proof_transport.audit_green
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == computational_model
                .computational_model_statement
                .machine_identity_id
        && machine_lock.canonical_machine_tuple.machine_identity_id
            == proof_transport.machine_identity_id
        && machine_lock
            .canonical_machine_tuple
            .continuation_contract_id
            == computational_model
                .computational_model_statement
                .runtime_contract_id
        && machine_lock
            .canonical_machine_tuple
            .continuation_contract_id
            == proof_transport.continuation_contract_id
        && proof_transport.transport_boundary.boundary_id
            == machine_identity_binding.proof_transport_boundary_id;
    let hidden_workflow_logic_refused = continuation_surface_rows
        .iter()
        .filter(|row| {
            row.surface_kind != TassadarPostArticleContinuationSurfaceKind::WeightedPluginController
        })
        .all(|row| row.state_transport_only && row.suspicious_key_hits.is_empty());
    let continuation_expressivity_extension_blocked = continuation_surface_rows
        .iter()
        .all(|row| row.expressivity_extension_blocked);
    let plugin_resume_hidden_compute_refused = continuation_surface_rows
        .iter()
        .find(|row| {
            row.surface_kind == TassadarPostArticleContinuationSurfaceKind::WeightedPluginController
        })
        .is_some_and(|row| row.surface_green && !row.host_compute_relocation_detected);
    let dependency_rows = build_dependency_rows(
        canonical_binding_green,
        hidden_workflow_logic_refused,
        continuation_expressivity_extension_blocked,
        plugin_resume_hidden_compute_refused,
    );
    let invalidation_rows = build_invalidation_rows(
        canonical_binding_green,
        &continuation_surface_rows,
        plugin_resume_hidden_compute_refused,
    );
    let invalidation_rows_absent = invalidation_rows.iter().all(|row| !row.present);
    let contract_green = canonical_binding_green
        && continuation_surface_rows
            .iter()
            .all(|row| row.surface_green)
        && dependency_rows.iter().all(|row| row.satisfied)
        && invalidation_rows_absent;
    let continuation_extends_execution_without_second_machine = canonical_binding_green
        && continuation_surface_rows
            .iter()
            .all(|row| row.surface_green);
    let continuation_non_computationality_complete =
        contract_green && hidden_workflow_logic_refused && plugin_resume_hidden_compute_refused;
    let validation_rows = build_validation_rows(
        canonical_binding_green,
        &continuation_surface_rows,
        hidden_workflow_logic_refused,
        continuation_expressivity_extension_blocked,
        plugin_resume_hidden_compute_refused,
        invalidation_rows_absent,
        continuation_non_computationality_complete,
    );

    let supporting_material_refs = vec![
        String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF),
        String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF),
        String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF),
        String::from(TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF),
        String::from(TASSADAR_PROCESS_OBJECT_REPORT_REF),
        String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
        String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
        String::from(TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF),
        String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
        ),
    ];
    let mut report = TassadarPostArticleContinuationNonComputationalityContractReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_continuation_non_computationality_contract.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_CHECKER_REF,
        ),
        transformer_anchor_contract_ref: String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
        canonical_machine_identity_lock_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
        ),
        canonical_computational_model_statement_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
        ),
        execution_semantics_proof_transport_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
        ),
        execution_checkpoint_report_ref: String::from(TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF),
        process_object_report_ref: String::from(TASSADAR_PROCESS_OBJECT_REPORT_REF),
        session_process_profile_report_ref: String::from(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF),
        spill_tape_store_report_ref: String::from(TASSADAR_SPILL_TAPE_STORE_REPORT_REF),
        installed_process_lifecycle_report_ref: String::from(
            TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
        ),
        weighted_plugin_controller_trace_eval_report_ref: String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
        ),
        supporting_material_refs,
        continuation_contract,
        machine_identity_binding,
        supporting_material_rows,
        dependency_rows,
        continuation_surface_rows,
        invalidation_rows,
        validation_rows,
        contract_status: if contract_green {
            TassadarPostArticleContinuationNonComputationalityStatus::Green
        } else {
            TassadarPostArticleContinuationNonComputationalityStatus::Blocked
        },
        contract_green,
        continuation_extends_execution_without_second_machine,
        hidden_workflow_logic_refused,
        continuation_expressivity_extension_blocked,
        plugin_resume_hidden_compute_refused,
        continuation_non_computationality_complete,
        next_stability_issue_id: String::from(NEXT_STABILITY_ISSUE_ID),
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        claim_boundary: String::from(
            "this eval-owned contract report freezes one continuation non-computationality boundary on the canonical post-article machine. It proves that declared checkpoint, spill, session, process-object, and installed-process continuation surfaces transport state and lineage without becoming a second machine, and that weighted plugin control may consume the same continuation carrier only while the host stays non-planner. It still leaves fast-route legitimacy, broader anti-drift closeout, served/public universality, and the final closure bundle to later issues.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article continuation non-computationality contract report keeps status={:?}, supporting_material_rows={}, dependency_rows={}, continuation_surface_rows={}, invalidation_rows={}, validation_rows={}, continuation_non_computationality_complete={}, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
        report.contract_status,
        report.supporting_material_rows.len(),
        report.dependency_rows.len(),
        report.continuation_surface_rows.len(),
        report.invalidation_rows.len(),
        report.validation_rows.len(),
        report.continuation_non_computationality_complete,
        report.next_stability_issue_id,
        report.closure_bundle_issue_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_continuation_non_computationality_contract_report|",
        &report,
    );
    Ok(report)
}

fn build_machine_identity_binding(
    machine_lock: &TassadarPostArticleCanonicalMachineIdentityLockReport,
    computational_model: &TassadarPostArticleCanonicalComputationalModelStatementReport,
    proof_transport: &TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
) -> TassadarPostArticleContinuationMachineIdentityBinding {
    TassadarPostArticleContinuationMachineIdentityBinding {
        machine_identity_id: machine_lock.canonical_machine_tuple.machine_identity_id.clone(),
        tuple_id: machine_lock.canonical_machine_tuple.tuple_id.clone(),
        canonical_model_id: machine_lock.canonical_machine_tuple.canonical_model_id.clone(),
        canonical_weight_artifact_id: machine_lock
            .canonical_machine_tuple
            .canonical_weight_artifact_id
            .clone(),
        canonical_weight_bundle_digest: machine_lock
            .canonical_machine_tuple
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: machine_lock
            .canonical_machine_tuple
            .canonical_weight_primary_artifact_sha256
            .clone(),
        canonical_route_id: machine_lock.canonical_machine_tuple.canonical_route_id.clone(),
        canonical_route_descriptor_digest: machine_lock
            .canonical_machine_tuple
            .canonical_route_descriptor_digest
            .clone(),
        continuation_contract_id: machine_lock
            .canonical_machine_tuple
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: machine_lock
            .canonical_machine_tuple
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: computational_model
            .computational_model_statement
            .statement_id
            .clone(),
        proof_transport_boundary_id: proof_transport.transport_boundary.boundary_id.clone(),
        detail: format!(
            "machine_identity_id=`{}` tuple_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` continuation_contract_id=`{}` computational_model_statement_id=`{}` and proof_transport_boundary_id=`{}` remain the bound continuation tuple.",
            machine_lock.canonical_machine_tuple.machine_identity_id,
            machine_lock.canonical_machine_tuple.tuple_id,
            machine_lock.canonical_machine_tuple.canonical_model_id,
            machine_lock.canonical_machine_tuple.canonical_route_id,
            machine_lock.canonical_machine_tuple.continuation_contract_id,
            computational_model.computational_model_statement.statement_id,
            proof_transport.transport_boundary.boundary_id,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_supporting_material_rows(
    continuation_contract: &TassadarPostArticleContinuationNonComputationalityContract,
    machine_lock: &TassadarPostArticleCanonicalMachineIdentityLockReport,
    computational_model: &TassadarPostArticleCanonicalComputationalModelStatementReport,
    proof_transport: &TassadarPostArticleExecutionSemanticsProofTransportAuditReport,
    execution_checkpoint: &TassadarExecutionCheckpointReport,
    process_object: &TassadarProcessObjectReport,
    session_process: &TassadarSessionProcessProfileReport,
    spill_tape: &TassadarSpillTapeStoreReport,
    installed_process: &TassadarInstalledProcessLifecycleReport,
    weighted_controller: &TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
) -> Vec<TassadarPostArticleContinuationSupportingMaterialRow> {
    vec![
        supporting_material_row(
            "transformer_anchor_contract",
            TassadarPostArticleContinuationSupportingMaterialClass::Anchor,
            true,
            TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
            Some(continuation_contract.contract_id.clone()),
            None,
            "the transformer-owned contract names the continuation surfaces, blocked hidden-compute modes, and next stability frontier explicitly.",
        ),
        supporting_material_row(
            "canonical_machine_identity_lock",
            TassadarPostArticleContinuationSupportingMaterialClass::MachineBinding,
            machine_lock.lock_green,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
            Some(machine_lock.report_id.clone()),
            Some(machine_lock.report_digest.clone()),
            "the canonical machine lock binds continuation to one explicit post-article machine tuple.",
        ),
        supporting_material_row(
            "canonical_computational_model_statement",
            TassadarPostArticleContinuationSupportingMaterialClass::MachineBinding,
            computational_model.statement_green,
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            Some(computational_model.report_id.clone()),
            Some(computational_model.report_digest.clone()),
            "the computational-model statement names continuation as a property of the same canonical machine instead of a second machine.",
        ),
        supporting_material_row(
            "execution_semantics_proof_transport_audit",
            TassadarPostArticleContinuationSupportingMaterialClass::MachineBinding,
            proof_transport.audit_green,
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            Some(proof_transport.report_id.clone()),
            Some(proof_transport.report_digest.clone()),
            "the proof-transport audit binds the continuation carrier to the same proof-bearing machine without closing this issue implicitly.",
        ),
        supporting_material_row(
            "execution_checkpoint_profile",
            TassadarPostArticleContinuationSupportingMaterialClass::ContinuationSurface,
            execution_checkpoint.exact_resume_parity_count == execution_checkpoint.case_reports.len() as u32,
            TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
            Some(execution_checkpoint.report_id.clone()),
            Some(execution_checkpoint.report_digest.clone()),
            "execution checkpoints keep deterministic multi-slice continuation explicit through persisted checkpoint artifacts and typed resume refusals.",
        ),
        supporting_material_row(
            "process_object_family",
            TassadarPostArticleContinuationSupportingMaterialClass::ContinuationSurface,
            process_object.exact_process_parity_count == process_object.case_reports.len() as u32,
            TASSADAR_PROCESS_OBJECT_REPORT_REF,
            Some(process_object.report_id.clone()),
            Some(process_object.report_digest.clone()),
            "durable process objects keep snapshot, tape, and work-queue lineage explicit instead of becoming hidden workflow interpreters.",
        ),
        supporting_material_row(
            "session_process_profile",
            TassadarPostArticleContinuationSupportingMaterialClass::ContinuationSurface,
            session_process.overall_green,
            TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
            Some(session_process.report_id.clone()),
            Some(session_process.report_digest.clone()),
            "bounded session continuation remains finite and deterministic while open-ended external loops stay on typed refusal paths.",
        ),
        supporting_material_row(
            "spill_tape_store_profile",
            TassadarPostArticleContinuationSupportingMaterialClass::ContinuationSurface,
            spill_tape.exact_case_count == 2,
            TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
            Some(spill_tape.report_id.clone()),
            Some(spill_tape.report_digest.clone()),
            "spill and tape-store continuation keep exact parity or typed refusal explicit under one bounded portability envelope.",
        ),
        supporting_material_row(
            "installed_process_lifecycle_profile",
            TassadarPostArticleContinuationSupportingMaterialClass::ContinuationSurface,
            installed_process.overall_green,
            TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
            Some(installed_process.report_id.clone()),
            Some(installed_process.report_digest.clone()),
            "installed-process lifecycle receipts keep migration and rollback as typed transport surfaces rather than a second machine.",
        ),
        supporting_material_row(
            "weighted_plugin_controller_eval",
            TassadarPostArticleContinuationSupportingMaterialClass::ControllerBoundary,
            weighted_controller.contract_green,
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
            Some(weighted_controller.report_id.clone()),
            Some(weighted_controller.report_digest.clone()),
            "weighted plugin control remains model-owned and keeps the host execution-only while consuming the same continuation carrier.",
        ),
    ]
}

fn build_dependency_rows(
    canonical_binding_green: bool,
    hidden_workflow_logic_refused: bool,
    continuation_expressivity_extension_blocked: bool,
    plugin_resume_hidden_compute_refused: bool,
) -> Vec<TassadarPostArticleContinuationDependencyRow> {
    vec![
        dependency_row(
            "canonical_machine_binding_complete",
            canonical_binding_green,
            &[
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            ],
            "the canonical machine lock, computational-model statement, and proof-transport audit agree on one machine tuple and one continuation contract.",
        ),
        dependency_row(
            "checkpoint_and_process_structures_transport_state_only",
            hidden_workflow_logic_refused,
            &[
                TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
                TASSADAR_PROCESS_OBJECT_REPORT_REF,
                TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
            ],
            "checkpoint, process-object, and installed-process structures keep state and lineage explicit without smuggling hidden workflow logic.",
        ),
        dependency_row(
            "session_resume_stays_bounded",
            continuation_expressivity_extension_blocked,
            &[TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF],
            "session-process continuation stays finite and deterministic while open-ended external loops remain refused.",
        ),
        dependency_row(
            "spill_and_tape_resume_stays_bounded",
            continuation_expressivity_extension_blocked,
            &[TASSADAR_SPILL_TAPE_STORE_REPORT_REF],
            "spill and tape continuation preserve exact parity on admitted cases and keep widening on typed refusal paths.",
        ),
        dependency_row(
            "plugin_controller_resume_boundary_closed",
            plugin_resume_hidden_compute_refused,
            &[TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF],
            "plugin and controller layers may consume continuation only while resume, retry, and stop remain model-owned and the host stays non-planner.",
        ),
        dependency_row(
            "continuation_extends_execution_without_second_machine",
            canonical_binding_green && continuation_expressivity_extension_blocked,
            &[
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
                TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
                TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
                TASSADAR_PROCESS_OBJECT_REPORT_REF,
                TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
            ],
            "declared continuation surfaces extend one already-bound machine rather than creating a second proof-bearing machine.",
        ),
    ]
}

fn build_continuation_surface_rows(
    machine_identity_binding: &TassadarPostArticleContinuationMachineIdentityBinding,
    execution_checkpoint: &TassadarExecutionCheckpointReport,
    process_object: &TassadarProcessObjectReport,
    session_process: &TassadarSessionProcessProfileReport,
    spill_tape: &TassadarSpillTapeStoreReport,
    installed_process: &TassadarInstalledProcessLifecycleReport,
    weighted_controller: &TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
) -> Result<
    Vec<TassadarPostArticleContinuationSurfaceRow>,
    TassadarPostArticleContinuationNonComputationalityContractReportError,
> {
    let checkpoint_hits = suspicious_key_hits(
        &execution_checkpoint.runtime_bundle.case_receipts,
        "execution_checkpoint.runtime_bundle.case_receipts",
    )?;
    let checkpoint_state_transport_only = checkpoint_hits.is_empty()
        && execution_checkpoint
            .case_reports
            .iter()
            .all(|case| !case.checkpoint_artifacts.is_empty() && case.checkpoint_count > 0);
    let checkpoint_exact_or_refusal_green = execution_checkpoint.exact_resume_parity_count
        == execution_checkpoint.case_reports.len() as u32
        && execution_checkpoint.refusal_case_count
            >= execution_checkpoint.case_reports.len() as u32;
    let checkpoint_expressivity_extension_blocked = execution_checkpoint.refusal_case_count > 0;

    let session_hits = suspicious_key_hits(
        &session_process.runtime_report.rows,
        "session_process.runtime_report.rows",
    )?;
    let session_state_transport_only = session_hits.is_empty()
        && session_process
            .case_audits
            .iter()
            .all(|case| case.exact_state_parity);
    let session_exact_or_refusal_green = session_process.overall_green
        && session_process
            .case_audits
            .iter()
            .all(|case| case.exact_output_parity);
    let session_expressivity_extension_blocked = session_process
        .refused_interaction_surface_ids
        .contains(&String::from("open_ended_external_event_stream"));

    let spill_hits = suspicious_key_hits(
        &spill_tape.runtime_bundle.case_receipts,
        "spill_tape.runtime_bundle.case_receipts",
    )?;
    let spill_state_transport_only = spill_hits.is_empty()
        && spill_tape.case_reports.iter().all(|case| {
            case.spill_segment_count == case.spill_segment_artifacts.len() as u32
                && case.external_tape_segment_count
                    == case.external_tape_store_artifacts.len() as u32
        });
    let spill_exact_or_refusal_green = spill_tape.exact_case_count == 2
        && spill_tape
            .case_reports
            .iter()
            .filter(|case| {
                case.status
                    == psionic_runtime::TassadarSpillTapeCaseStatus::ExactSpillAndResumeParity
            })
            .all(|case| case.spill_vs_in_core_parity && case.external_tape_resume_parity);
    let spill_expressivity_extension_blocked = spill_tape.portability_envelope_ids
        == [String::from("cpu_reference_current_host")]
        && spill_tape.refusal_case_count >= 3;

    let process_hits = suspicious_key_hits(
        &process_object.runtime_bundle.case_receipts,
        "process_object.runtime_bundle.case_receipts",
    )?;
    let process_state_transport_only = process_hits.is_empty()
        && process_object
            .case_reports
            .iter()
            .all(|case| case.tape_entry_count > 0 && case.work_queue_depth > 0);
    let process_exact_or_refusal_green = process_object.exact_process_parity_count
        == process_object.case_reports.len() as u32
        && process_object.refusal_case_count >= process_object.case_reports.len() as u32;
    let process_expressivity_extension_blocked = process_object.refusal_case_count > 0;

    let installed_hits = suspicious_key_hits(
        &installed_process.runtime_bundle.case_receipts,
        "installed_process.runtime_bundle.case_receipts",
    )?;
    let installed_state_transport_only = installed_hits.is_empty()
        && installed_process
            .case_reports
            .iter()
            .all(|case| !case.snapshot_artifact.snapshot_path.is_empty());
    let installed_exact_or_refusal_green = installed_process.overall_green
        && installed_process.exact_migration_case_count == 1
        && installed_process.exact_rollback_case_count == 1;
    let installed_expressivity_extension_blocked = !installed_process.served_publication_allowed;

    let controller_hits = suspicious_key_hits(
        &weighted_controller.machine_identity_binding,
        "weighted_controller.machine_identity_binding",
    )?;
    let controller_state_transport_only = controller_hits.is_empty()
        && weighted_controller.control_trace_contract_green
        && weighted_controller.typed_refusal_loop_closed;
    let controller_exact_or_refusal_green = weighted_controller.contract_green
        && weighted_controller.control_trace_contract_green
        && weighted_controller.typed_refusal_loop_closed;
    let controller_expressivity_extension_blocked = weighted_controller.host_not_planner_green
        && weighted_controller.adversarial_negative_rows_green;
    let controller_host_compute_relocation_detected = !(weighted_controller.host_not_planner_green
        && weighted_controller.typed_refusal_loop_closed
        && validation_row_green(
            &weighted_controller.validation_rows,
            "retry_and_stop_model_owned",
        ));

    Ok(vec![
        continuation_surface_row(
            TASSADAR_POST_ARTICLE_CONTINUATION_CHECKPOINT_SURFACE_ID,
            TassadarPostArticleContinuationSurfaceKind::CheckpointExecution,
            TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
            &execution_checkpoint.report_id,
            &execution_checkpoint.report_digest,
            &machine_identity_binding.machine_identity_id,
            vec![execution_checkpoint.checkpoint_family_id.clone()],
            checkpoint_hits,
            checkpoint_state_transport_only,
            checkpoint_exact_or_refusal_green,
            checkpoint_expressivity_extension_blocked,
            false,
            "checkpoint artifacts carry deterministic continuation state, parity, and typed refusal posture without carrying hidden workflow authority.",
        ),
        continuation_surface_row(
            TASSADAR_POST_ARTICLE_CONTINUATION_SESSION_PROCESS_SURFACE_ID,
            TassadarPostArticleContinuationSurfaceKind::SessionProcess,
            TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
            &session_process.report_id,
            &session_process.report_digest,
            &machine_identity_binding.machine_identity_id,
            vec![
                session_process.profile_id.clone(),
                String::from("deterministic_echo_turn_loop"),
                String::from("stateful_counter_turn_loop"),
            ],
            session_hits,
            session_state_transport_only,
            session_exact_or_refusal_green,
            session_expressivity_extension_blocked,
            false,
            "session-process continuation stays finite and deterministic, and it refuses the open-ended external-event surface instead of becoming generic workflow control.",
        ),
        continuation_surface_row(
            TASSADAR_POST_ARTICLE_CONTINUATION_SPILL_TAPE_SURFACE_ID,
            TassadarPostArticleContinuationSurfaceKind::SpillTapeStore,
            TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
            &spill_tape.report_id,
            &spill_tape.report_digest,
            &machine_identity_binding.machine_identity_id,
            vec![
                spill_tape.profile_id.clone(),
                spill_tape.spill_segment_family_id.clone(),
                spill_tape.external_tape_store_family_id.clone(),
            ],
            spill_hits,
            spill_state_transport_only,
            spill_exact_or_refusal_green,
            spill_expressivity_extension_blocked,
            false,
            "spill and tape-store continuation preserve declared state across exact parity cases and keep missing segments, oversize state, and portability widening on typed refusal paths.",
        ),
        continuation_surface_row(
            TASSADAR_POST_ARTICLE_CONTINUATION_PROCESS_OBJECT_SURFACE_ID,
            TassadarPostArticleContinuationSurfaceKind::DurableProcessObject,
            TASSADAR_PROCESS_OBJECT_REPORT_REF,
            &process_object.report_id,
            &process_object.report_digest,
            &machine_identity_binding.machine_identity_id,
            vec![
                process_object.profile_id.clone(),
                process_object.snapshot_family_id.clone(),
                process_object.tape_family_id.clone(),
                process_object.work_queue_family_id.clone(),
            ],
            process_hits,
            process_state_transport_only,
            process_exact_or_refusal_green,
            process_expressivity_extension_blocked,
            false,
            "process snapshots, tapes, and work queues preserve durable execution state and typed refusals without carrying hidden planner logic.",
        ),
        continuation_surface_row(
            TASSADAR_POST_ARTICLE_CONTINUATION_INSTALLED_PROCESS_SURFACE_ID,
            TassadarPostArticleContinuationSurfaceKind::InstalledProcessLifecycle,
            TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
            &installed_process.report_id,
            &installed_process.report_digest,
            &machine_identity_binding.machine_identity_id,
            vec![
                installed_process.runtime_bundle.snapshot_family_id.clone(),
                installed_process.runtime_bundle.migration_receipt_family_id.clone(),
                installed_process.runtime_bundle.rollback_receipt_family_id.clone(),
            ],
            installed_hits,
            installed_state_transport_only,
            installed_exact_or_refusal_green,
            installed_expressivity_extension_blocked,
            false,
            "installed-process migration and rollback remain typed transport surfaces over the same machine instead of becoming a second machine.",
        ),
        continuation_surface_row(
            TASSADAR_POST_ARTICLE_CONTINUATION_WEIGHTED_CONTROLLER_SURFACE_ID,
            TassadarPostArticleContinuationSurfaceKind::WeightedPluginController,
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
            &weighted_controller.report_id,
            &weighted_controller.report_digest,
            &machine_identity_binding.machine_identity_id,
            vec![
                weighted_controller
                    .machine_identity_binding
                    .control_trace_contract_id
                    .clone(),
                weighted_controller
                    .machine_identity_binding
                    .model_loop_return_profile_id
                    .clone(),
                weighted_controller
                    .machine_identity_binding
                    .continuation_contract_id
                    .clone(),
            ],
            controller_hits,
            controller_state_transport_only,
            controller_exact_or_refusal_green,
            controller_expressivity_extension_blocked,
            controller_host_compute_relocation_detected,
            "weighted plugin control may consume continuation only while retry, refusal, and stop stay model-owned and the host stays execution-only.",
        ),
    ])
}

fn build_invalidation_rows(
    canonical_binding_green: bool,
    continuation_surface_rows: &[TassadarPostArticleContinuationSurfaceRow],
    plugin_resume_hidden_compute_refused: bool,
) -> Vec<TassadarPostArticleContinuationInvalidationRow> {
    let checkpoint_row = surface_row(
        continuation_surface_rows,
        TassadarPostArticleContinuationSurfaceKind::CheckpointExecution,
    );
    let spill_row = surface_row(
        continuation_surface_rows,
        TassadarPostArticleContinuationSurfaceKind::SpillTapeStore,
    );
    let session_row = surface_row(
        continuation_surface_rows,
        TassadarPostArticleContinuationSurfaceKind::SessionProcess,
    );
    let process_row = surface_row(
        continuation_surface_rows,
        TassadarPostArticleContinuationSurfaceKind::DurableProcessObject,
    );
    let installed_row = surface_row(
        continuation_surface_rows,
        TassadarPostArticleContinuationSurfaceKind::InstalledProcessLifecycle,
    );
    let controller_row = surface_row(
        continuation_surface_rows,
        TassadarPostArticleContinuationSurfaceKind::WeightedPluginController,
    );

    vec![
        invalidation_row(
            "second_machine_binding_detected",
            !canonical_binding_green,
            &[
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            ],
            "any mismatch in machine identity, route, weight lineage, or continuation contract means continuation has drifted onto a different machine.",
        ),
        invalidation_row(
            "checkpoint_workflow_logic_detected",
            !checkpoint_row.state_transport_only || !checkpoint_row.suspicious_key_hits.is_empty(),
            &[TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF],
            "checkpoint artifacts may not carry hidden workflow logic, planner state, or undeclared scheduling authority.",
        ),
        invalidation_row(
            "spill_or_tape_directive_logic_detected",
            !spill_row.state_transport_only || !spill_row.suspicious_key_hits.is_empty(),
            &[TASSADAR_SPILL_TAPE_STORE_REPORT_REF],
            "spill and tape segments may not choose control flow or encode directives beyond declared state transport.",
        ),
        invalidation_row(
            "session_surface_widening_detected",
            !session_row.expressivity_extension_blocked,
            &[TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF],
            "session continuation invalidates the contract if it widens into open-ended external control.",
        ),
        invalidation_row(
            "process_object_logic_smuggling_detected",
            !process_row.state_transport_only
                || !installed_row.state_transport_only
                || !process_row.suspicious_key_hits.is_empty()
                || !installed_row.suspicious_key_hits.is_empty(),
            &[
                TASSADAR_PROCESS_OBJECT_REPORT_REF,
                TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
            ],
            "process-object and installed-process lifecycle receipts may not smuggle planner or policy logic into continuation state.",
        ),
        invalidation_row(
            "resume_hidden_compute_detected",
            controller_row.host_compute_relocation_detected || !plugin_resume_hidden_compute_refused,
            &[TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF],
            "resume, retry, and stop semantics invalidate the contract if they relocate real computation into host-managed continuation.",
        ),
        invalidation_row(
            "continuation_overclaim_detected",
            continuation_surface_rows.iter().any(|row| !row.surface_green),
            &[
                TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
                TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
                TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
                TASSADAR_PROCESS_OBJECT_REPORT_REF,
                TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
            ],
            "continuation may not be overclaimed as green if any declared surface fails parity, typed refusal, or non-computationality checks.",
        ),
    ]
}

fn build_validation_rows(
    canonical_binding_green: bool,
    continuation_surface_rows: &[TassadarPostArticleContinuationSurfaceRow],
    hidden_workflow_logic_refused: bool,
    continuation_expressivity_extension_blocked: bool,
    plugin_resume_hidden_compute_refused: bool,
    invalidation_rows_absent: bool,
    continuation_non_computationality_complete: bool,
) -> Vec<TassadarPostArticleContinuationValidationRow> {
    let checkpoint_or_spill_green = continuation_surface_rows.iter().any(|row| {
        row.surface_kind == TassadarPostArticleContinuationSurfaceKind::CheckpointExecution
            && row.surface_green
    }) && continuation_surface_rows.iter().any(|row| {
        row.surface_kind == TassadarPostArticleContinuationSurfaceKind::SpillTapeStore
            && row.surface_green
    });
    let process_surfaces_green = continuation_surface_rows.iter().any(|row| {
        row.surface_kind == TassadarPostArticleContinuationSurfaceKind::DurableProcessObject
            && row.surface_green
    }) && continuation_surface_rows.iter().any(|row| {
        row.surface_kind == TassadarPostArticleContinuationSurfaceKind::InstalledProcessLifecycle
            && row.surface_green
    });
    let controller_green = continuation_surface_rows.iter().any(|row| {
        row.surface_kind == TassadarPostArticleContinuationSurfaceKind::WeightedPluginController
            && row.surface_green
    });

    vec![
        validation_row(
            "canonical_machine_binding_green",
            canonical_binding_green,
            &[
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_REPORT_REF,
                TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
                TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_AUDIT_REPORT_REF,
            ],
            "the canonical machine tuple, computational-model statement, and proof-transport audit agree on one continuation-bound machine.",
        ),
        validation_row(
            "checkpoint_and_spill_surfaces_green",
            checkpoint_or_spill_green,
            &[
                TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
                TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
            ],
            "checkpoint and spill/tape continuation stay exact-or-refusal-bounded on declared workloads.",
        ),
        validation_row(
            "session_surface_green",
            continuation_surface_rows.iter().any(|row| {
                row.surface_kind == TassadarPostArticleContinuationSurfaceKind::SessionProcess
                    && row.surface_green
            }),
            &[TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF],
            "session-process continuation stays finite, deterministic, and refusal-bounded.",
        ),
        validation_row(
            "process_surfaces_green",
            process_surfaces_green,
            &[
                TASSADAR_PROCESS_OBJECT_REPORT_REF,
                TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
            ],
            "durable process objects plus installed-process lifecycle receipts preserve state and lineage without becoming compute.",
        ),
        validation_row(
            "hidden_workflow_logic_refused",
            hidden_workflow_logic_refused,
            &[
                TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
                TASSADAR_PROCESS_OBJECT_REPORT_REF,
                TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
                TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
            ],
            "declared continuation structures carry state and lineage only; hidden workflow logic remains absent.",
        ),
        validation_row(
            "continuation_expressivity_extension_blocked",
            continuation_expressivity_extension_blocked,
            &[
                TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
                TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
                TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
            ],
            "continuation does not add expressivity beyond the base route because widening cases stay refused or blocked explicitly.",
        ),
        validation_row(
            "plugin_resume_hidden_compute_refused",
            controller_green && plugin_resume_hidden_compute_refused,
            &[TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF],
            "plugin and controller layers do not lean on resume semantics as hidden compute because the host remains non-planner and retry or stop stays model-owned.",
        ),
        validation_row(
            "invalidation_rows_absent",
            invalidation_rows_absent,
            &[
                TASSADAR_EXECUTION_CHECKPOINT_REPORT_REF,
                TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
                TASSADAR_SPILL_TAPE_STORE_REPORT_REF,
                TASSADAR_PROCESS_OBJECT_REPORT_REF,
                TASSADAR_INSTALLED_PROCESS_LIFECYCLE_REPORT_REF,
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
            ],
            "no hidden-workflow, second-machine, or host-relocated-compute invalidation is present.",
        ),
        validation_row(
            "continuation_non_computationality_complete",
            continuation_non_computationality_complete,
            &[TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF],
            "the continuation non-computationality contract is complete on one canonical machine and hands the frontier to TAS-215.",
        ),
    ]
}

#[must_use]
pub fn tassadar_post_article_continuation_non_computationality_contract_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CONTINUATION_NON_COMPUTATIONALITY_CONTRACT_REPORT_REF)
}

pub fn write_tassadar_post_article_continuation_non_computationality_contract_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleContinuationNonComputationalityContractReport,
    TassadarPostArticleContinuationNonComputationalityContractReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleContinuationNonComputationalityContractReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_continuation_non_computationality_contract_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleContinuationNonComputationalityContractReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleContinuationSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleContinuationSupportingMaterialRow {
    TassadarPostArticleContinuationSupportingMaterialRow {
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
) -> TassadarPostArticleContinuationDependencyRow {
    TassadarPostArticleContinuationDependencyRow {
        dependency_id: String::from(dependency_id),
        satisfied,
        source_refs: source_refs
            .iter()
            .map(|source| String::from(*source))
            .collect(),
        detail: String::from(detail),
    }
}

fn continuation_surface_row(
    surface_id: &str,
    surface_kind: TassadarPostArticleContinuationSurfaceKind,
    source_report_ref: &str,
    source_report_id: &str,
    source_report_digest: &str,
    canonical_machine_identity_id: &str,
    structural_carrier_ids: Vec<String>,
    suspicious_key_hits: Vec<String>,
    state_transport_only: bool,
    exact_parity_or_typed_refusal_green: bool,
    expressivity_extension_blocked: bool,
    host_compute_relocation_detected: bool,
    detail: &str,
) -> TassadarPostArticleContinuationSurfaceRow {
    let surface_green = state_transport_only
        && exact_parity_or_typed_refusal_green
        && expressivity_extension_blocked
        && !host_compute_relocation_detected
        && suspicious_key_hits.is_empty();
    TassadarPostArticleContinuationSurfaceRow {
        surface_id: String::from(surface_id),
        surface_kind,
        source_report_ref: String::from(source_report_ref),
        source_report_id: String::from(source_report_id),
        source_report_digest: String::from(source_report_digest),
        canonical_machine_identity_id: String::from(canonical_machine_identity_id),
        structural_carrier_ids,
        suspicious_key_hits,
        state_transport_only,
        exact_parity_or_typed_refusal_green,
        expressivity_extension_blocked,
        host_compute_relocation_detected,
        surface_green,
        detail: String::from(detail),
    }
}

fn invalidation_row(
    invalidation_id: &str,
    present: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticleContinuationInvalidationRow {
    TassadarPostArticleContinuationInvalidationRow {
        invalidation_id: String::from(invalidation_id),
        present,
        source_refs: source_refs
            .iter()
            .map(|source| String::from(*source))
            .collect(),
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticleContinuationValidationRow {
    TassadarPostArticleContinuationValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs
            .iter()
            .map(|source| String::from(*source))
            .collect(),
        detail: String::from(detail),
    }
}

fn surface_row(
    rows: &[TassadarPostArticleContinuationSurfaceRow],
    surface_kind: TassadarPostArticleContinuationSurfaceKind,
) -> &TassadarPostArticleContinuationSurfaceRow {
    rows.iter()
        .find(|row| row.surface_kind == surface_kind)
        .expect("surface row should exist")
}

fn validation_row_green(
    rows: &[crate::TassadarPostArticleWeightedPluginControllerTraceEvalValidationRow],
    validation_id: &str,
) -> bool {
    rows.iter()
        .find(|row| row.validation_id == validation_id)
        .is_some_and(|row| row.green)
}

fn suspicious_key_hits<T: Serialize>(
    value: &T,
    root_label: &str,
) -> Result<Vec<String>, TassadarPostArticleContinuationNonComputationalityContractReportError> {
    let value = serde_json::to_value(value)?;
    let mut hits = BTreeSet::new();
    collect_suspicious_key_hits(&value, root_label, &mut hits);
    Ok(hits.into_iter().collect())
}

fn collect_suspicious_key_hits(value: &Value, path: &str, hits: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                let key_lower = key.to_ascii_lowercase();
                if SUSPICIOUS_KEY_FRAGMENTS
                    .iter()
                    .any(|fragment| key_lower.contains(fragment))
                {
                    hits.insert(format!("{path}.{key}"));
                }
                collect_suspicious_key_hits(nested, &format!("{path}.{key}"), hits);
            }
        }
        Value::Array(values) => {
            for (index, nested) in values.iter().enumerate() {
                collect_suspicious_key_hits(nested, &format!("{path}[{index}]"), hits);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
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
) -> Result<T, TassadarPostArticleContinuationNonComputationalityContractReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleContinuationNonComputationalityContractReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleContinuationNonComputationalityContractReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_continuation_non_computationality_contract_report, read_json,
        tassadar_post_article_continuation_non_computationality_contract_report_path,
        write_tassadar_post_article_continuation_non_computationality_contract_report,
        TassadarPostArticleContinuationNonComputationalityContractReport,
        TassadarPostArticleContinuationNonComputationalityStatus,
    };
    use tempfile::tempdir;

    #[test]
    fn continuation_non_computationality_contract_report_keeps_boundary_explicit() {
        let report =
            build_tassadar_post_article_continuation_non_computationality_contract_report()
                .expect("report");

        assert_eq!(
            report.contract_status,
            TassadarPostArticleContinuationNonComputationalityStatus::Green
        );
        assert!(report.contract_green);
        assert_eq!(report.supporting_material_rows.len(), 10);
        assert_eq!(report.dependency_rows.len(), 6);
        assert_eq!(report.continuation_surface_rows.len(), 6);
        assert_eq!(report.invalidation_rows.len(), 7);
        assert_eq!(report.validation_rows.len(), 9);
        assert!(report.continuation_extends_execution_without_second_machine);
        assert!(report.hidden_workflow_logic_refused);
        assert!(report.continuation_expressivity_extension_blocked);
        assert!(report.plugin_resume_hidden_compute_refused);
        assert!(report.continuation_non_computationality_complete);
        assert_eq!(report.next_stability_issue_id, "TAS-215");
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
        assert!(report
            .continuation_surface_rows
            .iter()
            .all(|row| row.surface_green && row.suspicious_key_hits.is_empty()));
        assert!(report.invalidation_rows.iter().all(|row| !row.present));
    }

    #[test]
    fn continuation_non_computationality_contract_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_continuation_non_computationality_contract_report()
                .expect("report");
        let committed: TassadarPostArticleContinuationNonComputationalityContractReport =
            read_json(
                tassadar_post_article_continuation_non_computationality_contract_report_path(),
            )
            .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_continuation_non_computationality_contract_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_continuation_non_computationality_contract_report.json");
        let report = write_tassadar_post_article_continuation_non_computationality_contract_report(
            &output_path,
        )
        .expect("write report");
        let persisted: TassadarPostArticleContinuationNonComputationalityContractReport =
            read_json(&output_path).expect("persisted report");
        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_post_article_continuation_non_computationality_contract_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_continuation_non_computationality_contract_report.json")
        );
    }
}
