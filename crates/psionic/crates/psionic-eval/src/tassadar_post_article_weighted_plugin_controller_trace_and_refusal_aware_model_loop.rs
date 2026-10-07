use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_sandbox::{
    build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError,
    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
};
use psionic_transformer::{
    build_tassadar_post_article_weighted_plugin_controller_trace_contract,
    TassadarPostArticleWeightedPluginControllerTraceContract,
    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_DETERMINISM_PROFILE_ID,
    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_CONTRACT_ID,
    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROL_TRACE_PROFILE_ID,
};

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

pub const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report.json";
pub const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_CHECKER_REF:
    &str =
    "scripts/check-tassadar-post-article-weighted-plugin-controller-trace-and-refusal-aware-model-loop.sh";

const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const PLUGIN_SYSTEM_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const TRANSFORMER_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_weighted_plugin_controller_trace_contract.rs";
const CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_closure_bundle_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleWeightedPluginControllerTraceEvalDependencyClass {
    SandboxPrecedent,
    TransformerContract,
    ClosureBundle,
    DesignInput,
    AuditInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceEvalMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub result_binding_contract_id: String,
    pub model_loop_return_profile_id: String,
    pub control_trace_contract_id: String,
    pub control_trace_contract_digest: String,
    pub control_trace_profile_id: String,
    pub determinism_profile_id: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub sandbox_report_id: String,
    pub sandbox_report_digest: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceEvalDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticleWeightedPluginControllerTraceEvalDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceEvalValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub sandbox_report_ref: String,
    pub canonical_machine_closure_bundle_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding:
        TassadarPostArticleWeightedPluginControllerTraceEvalMachineIdentityBinding,
    pub transformer_contract: TassadarPostArticleWeightedPluginControllerTraceContract,
    pub dependency_rows: Vec<TassadarPostArticleWeightedPluginControllerTraceEvalDependencyRow>,
    pub controller_case_rows:
        Vec<psionic_runtime::TassadarPostArticleWeightedPluginControllerCaseRow>,
    pub control_trace_rows: Vec<psionic_runtime::TassadarPostArticleWeightedPluginControlTraceRow>,
    pub host_negative_rows: Vec<psionic_runtime::TassadarPostArticleWeightedPluginHostNegativeRow>,
    pub validation_rows: Vec<TassadarPostArticleWeightedPluginControllerTraceEvalValidationRow>,
    pub contract_status:
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalStatus,
    pub contract_green: bool,
    pub control_trace_contract_green: bool,
    pub determinism_profile_explicit: bool,
    pub typed_refusal_loop_closed: bool,
    pub host_not_planner_green: bool,
    pub adversarial_negative_rows_green: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub operator_internal_only_posture: bool,
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
pub enum TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError {
    #[error(transparent)]
    Sandbox(
        #[from] TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError,
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

pub fn build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report(
) -> Result<
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError,
> {
    let sandbox =
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report()?;
    let closure_bundle: CanonicalMachineClosureBundleFixture =
        read_repo_json(CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF)?;
    let transformer_contract =
        build_tassadar_post_article_weighted_plugin_controller_trace_contract();
    let transformer_contract_digest = stable_digest(
        b"psionic_tassadar_post_article_weighted_plugin_controller_trace_contract|",
        &transformer_contract,
    );

    let machine_identity_binding =
        TassadarPostArticleWeightedPluginControllerTraceEvalMachineIdentityBinding {
            machine_identity_id: sandbox.machine_identity_binding.machine_identity_id.clone(),
            canonical_model_id: sandbox.machine_identity_binding.canonical_model_id.clone(),
            canonical_route_id: sandbox.machine_identity_binding.canonical_route_id.clone(),
            canonical_route_descriptor_digest: sandbox
                .machine_identity_binding
                .canonical_route_descriptor_digest
                .clone(),
            canonical_weight_bundle_digest: sandbox
                .machine_identity_binding
                .canonical_weight_bundle_digest
                .clone(),
            canonical_weight_primary_artifact_sha256: sandbox
                .machine_identity_binding
                .canonical_weight_primary_artifact_sha256
                .clone(),
            continuation_contract_id: sandbox
                .machine_identity_binding
                .continuation_contract_id
                .clone(),
            continuation_contract_digest: sandbox
                .machine_identity_binding
                .continuation_contract_digest
                .clone(),
            computational_model_statement_id: sandbox
                .machine_identity_binding
                .computational_model_statement_id
                .clone(),
            packet_abi_version: sandbox.machine_identity_binding.packet_abi_version.clone(),
            host_owned_runtime_api_id: sandbox
                .machine_identity_binding
                .host_owned_runtime_api_id
                .clone(),
            engine_abstraction_id: sandbox
                .machine_identity_binding
                .engine_abstraction_id
                .clone(),
            invocation_receipt_profile_id: sandbox
                .machine_identity_binding
                .invocation_receipt_profile_id
                .clone(),
            result_binding_contract_id: sandbox
                .machine_identity_binding
                .result_binding_contract_id
                .clone(),
            model_loop_return_profile_id: sandbox
                .machine_identity_binding
                .model_loop_return_profile_id
                .clone(),
            control_trace_contract_id: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_CONTRACT_ID,
            ),
            control_trace_contract_digest: transformer_contract_digest.clone(),
            control_trace_profile_id: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROL_TRACE_PROFILE_ID,
            ),
            determinism_profile_id: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_DETERMINISM_PROFILE_ID,
            ),
            closure_bundle_report_id: closure_bundle.report_id.clone(),
            closure_bundle_report_digest: closure_bundle.report_digest.clone(),
            closure_bundle_digest: closure_bundle.closure_bundle_digest.clone(),
            sandbox_report_id: sandbox.report_id.clone(),
            sandbox_report_digest: sandbox.report_digest.clone(),
            runtime_bundle_id: sandbox.machine_identity_binding.runtime_bundle_id.clone(),
            runtime_bundle_digest: sandbox.machine_identity_binding.runtime_bundle_digest.clone(),
            detail: format!(
                "machine_identity_id=`{}` canonical_route_id=`{}` sandbox_report_id=`{}` control_trace_contract_id=`{}` and closure_bundle_digest=`{}` remain bound together.",
                sandbox.machine_identity_binding.machine_identity_id,
                sandbox.machine_identity_binding.canonical_route_id,
                sandbox.report_id,
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_CONTRACT_ID,
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
            "sandbox_controller_trace_green",
            TassadarPostArticleWeightedPluginControllerTraceEvalDependencyClass::SandboxPrecedent,
            sandbox.contract_green && sandbox.weighted_plugin_control_allowed,
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            Some(sandbox.report_id.clone()),
            Some(sandbox.report_digest.clone()),
            "the sandbox report must already close the bounded controller trace and turn weighted plugin control green on the canonical route.",
        ),
        dependency_row(
            "transformer_controller_contract_present",
            TassadarPostArticleWeightedPluginControllerTraceEvalDependencyClass::TransformerContract,
            transformer_contract.contract_id
                == TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_CONTRACT_ID
                && transformer_contract.control_trace_profile_id
                    == TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROL_TRACE_PROFILE_ID
                && transformer_contract.determinism_profile_id
                    == TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_DETERMINISM_PROFILE_ID,
            TRANSFORMER_CONTRACT_SOURCE_REF,
            Some(transformer_contract.contract_id.clone()),
            Some(transformer_contract_digest.clone()),
            "the transformer-owned controller contract must be the canonical abstract source for the weighted plugin control trace.",
        ),
        dependency_row(
            "canonical_machine_closure_bundle_published",
            TassadarPostArticleWeightedPluginControllerTraceEvalDependencyClass::ClosureBundle,
            closure_bundle_bound_by_digest,
            CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
            Some(closure_bundle.report_id.clone()),
            Some(closure_bundle.report_digest.clone()),
            "the weighted controller claim must inherit the canonical machine closure bundle by report id and digest instead of reconstructing machine identity from the sandbox report alone.",
        ),
        dependency_row(
            "plugin_system_spec_declared",
            TassadarPostArticleWeightedPluginControllerTraceEvalDependencyClass::DesignInput,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system spec remains the design input for the weighted controller trace.",
        ),
        dependency_row(
            "plugin_system_audit_declared",
            TassadarPostArticleWeightedPluginControllerTraceEvalDependencyClass::AuditInput,
            true,
            PLUGIN_SYSTEM_AUDIT_REF,
            None,
            None,
            "the plugin-system audit remains the supporting law source for host-negative planner attacks and replay posture.",
        ),
    ];

    let control_trace_contract_green = transformer_contract
        .ownership_rule_rows
        .iter()
        .chain(transformer_contract.determinism_rule_rows.iter())
        .chain(transformer_contract.host_boundary_rule_rows.iter())
        .all(|row| row.green)
        && sandbox.control_trace_contract_green;
    let determinism_profile_explicit = sandbox.determinism_contract_explicit
        && transformer_contract
            .determinism_rule_rows
            .iter()
            .all(|row| row.green);
    let typed_refusal_loop_closed = sandbox.typed_refusal_returned_to_model_loop
        && transformer_contract
            .ownership_rule_rows
            .iter()
            .any(|row| row.rule_id == "model_decides_retry_or_refusal" && row.green);
    let host_not_planner_green = sandbox.host_executes_but_is_not_planner
        && transformer_contract
            .host_boundary_rule_rows
            .iter()
            .any(|row| row.rule_id == "host_validates_and_executes_but_does_not_plan" && row.green);
    let adversarial_negative_rows_green = sandbox.adversarial_host_behavior_negative_rows_green
        && transformer_contract
            .host_boundary_rule_rows
            .iter()
            .any(|row| row.rule_id == "helper_substitution_forbidden" && row.green)
        && transformer_contract
            .host_boundary_rule_rows
            .iter()
            .any(|row| row.rule_id == "runtime_learning_or_policy_drift_forbidden" && row.green);

    let validation_rows = vec![
        validation_row(
            "sandbox_controller_trace_green",
            sandbox.contract_green,
            &[TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF],
            "the sandbox-owned controller trace is green on the canonical route.",
        ),
        validation_row(
            "transformer_contract_green",
            control_trace_contract_green,
            &[
                TRANSFORMER_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            ],
            "the transformer-owned controller contract and sandbox-owned runtime evidence agree on the same weighted control surface.",
        ),
        validation_row(
            "plugin_selection_and_export_model_owned",
            sandbox.plugin_selection_model_owned && sandbox.export_selection_model_owned,
            &[TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF],
            "plugin and export selection remain model-owned on the canonical route.",
        ),
        validation_row(
            "packet_arguments_and_sequencing_model_owned",
            sandbox.packet_arguments_model_owned && sandbox.multi_step_sequencing_model_owned,
            &[TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF],
            "packet arguments and multi-step sequencing remain model-owned on the canonical route.",
        ),
        validation_row(
            "retry_and_stop_model_owned",
            sandbox.retry_decisions_model_owned && sandbox.stop_conditions_model_owned,
            &[TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF],
            "retry and stop remain model-owned instead of collapsing into runtime policy.",
        ),
        validation_row(
            "typed_refusal_loop_closed",
            typed_refusal_loop_closed,
            &[
                TRANSFORMER_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            ],
            "typed refusals return to the model loop explicitly and stay part of the controller trace.",
        ),
        validation_row(
            "host_not_planner_green",
            host_not_planner_green,
            &[
                TRANSFORMER_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            ],
            "the host executes declared calls but does not become the planner.",
        ),
        validation_row(
            "determinism_profile_explicit",
            determinism_profile_explicit,
            &[
                TRANSFORMER_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            ],
            "determinism, sampling, temperature, and randomness controls remain explicit and replayable.",
        ),
        validation_row(
            "adversarial_negative_rows_green",
            adversarial_negative_rows_green,
            &[
                TRANSFORMER_CONTRACT_SOURCE_REF,
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
                PLUGIN_SYSTEM_AUDIT_REF,
            ],
            "adversarial host behaviors remain explicitly blocked instead of becoming planner-indistinguishable shortcuts.",
        ),
        validation_row(
            "closure_bundle_bound_by_digest",
            closure_bundle_bound_by_digest,
            &[
                CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            ],
            "the weighted controller claim now inherits the canonical machine closure bundle by digest instead of relying on adjacent machine fields only.",
        ),
    ];

    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && control_trace_contract_green
        && determinism_profile_explicit
        && typed_refusal_loop_closed
        && host_not_planner_green
        && adversarial_negative_rows_green
        && validation_rows.iter().all(|row| row.green);
    let contract_status = if contract_green {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalStatus::Green
    } else {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalStatus::Incomplete
    };

    let supporting_material_refs = vec![
        String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
        ),
        String::from(CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF),
        String::from(TRANSFORMER_CONTRACT_SOURCE_REF),
        String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        String::from(PLUGIN_SYSTEM_AUDIT_REF),
    ];

    let mut report =
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport {
            schema_version: 1,
            report_id: String::from(
                "tassadar.post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop.eval_report.v1",
            ),
            checker_script_ref: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_CHECKER_REF,
            ),
            sandbox_report_ref: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            ),
            canonical_machine_closure_bundle_report_ref: String::from(
                CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF,
            ),
            local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            supporting_material_refs,
            machine_identity_binding,
            transformer_contract,
            dependency_rows,
            controller_case_rows: sandbox.controller_case_rows.clone(),
            control_trace_rows: sandbox.control_trace_rows.clone(),
            host_negative_rows: sandbox.host_negative_rows.clone(),
            validation_rows,
            contract_status,
            contract_green,
            control_trace_contract_green,
            determinism_profile_explicit,
            typed_refusal_loop_closed,
            host_not_planner_green,
            adversarial_negative_rows_green,
            closure_bundle_bound_by_digest,
            operator_internal_only_posture: sandbox.operator_internal_only_posture,
            rebase_claim_allowed: sandbox.rebase_claim_allowed,
            plugin_capability_claim_allowed: false,
            weighted_plugin_control_allowed: contract_green,
            plugin_publication_allowed: false,
            served_public_universality_allowed: false,
            arbitrary_software_capability_allowed: false,
            deferred_issue_ids: Vec::new(),
            claim_boundary: String::from(
                "this eval-owned closure report makes the weighted plugin controller trace true on the canonical post-article route. It closes model ownership over plugin selection, export selection, packet argument construction, sequencing, typed-refusal handling, retry, and stop conditions while inheriting the canonical machine closure bundle by report id and digest instead of reconstructing machine identity locally. It still keeps plugin authority, publication, trust-tier promotion, served/public universality, and arbitrary public software execution deferred to the later platform gate.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
    report.summary = format!(
        "Post-article weighted plugin controller eval report keeps contract_status={:?}, dependency_rows={}, controller_case_rows={}, control_trace_rows={}, host_negative_rows={}, validation_rows={}, closure_bundle_digest=`{}`, weighted_plugin_control_allowed={}, and deferred_issue_ids={}.",
        report.contract_status,
        report.dependency_rows.len(),
        report.controller_case_rows.len(),
        report.control_trace_rows.len(),
        report.host_negative_rows.len(),
        report.validation_rows.len(),
        report.machine_identity_binding.closure_bundle_digest,
        report.weighted_plugin_control_allowed,
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
    )
}

pub fn write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report =
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn dependency_row(
    dependency_id: &str,
    dependency_class: TassadarPostArticleWeightedPluginControllerTraceEvalDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleWeightedPluginControllerTraceEvalDependencyRow {
    TassadarPostArticleWeightedPluginControllerTraceEvalDependencyRow {
        dependency_id: String::from(dependency_id),
        dependency_class,
        satisfied,
        source_ref: String::from(source_ref),
        bound_report_id,
        bound_report_digest,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: &[&str],
    detail: &str,
) -> TassadarPostArticleWeightedPluginControllerTraceEvalValidationRow {
    TassadarPostArticleWeightedPluginControllerTraceEvalValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
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

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<
    T,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError,
> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report,
        read_repo_json,
        tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report_path,
        write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report,
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport,
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
    };

    #[test]
    fn post_article_weighted_plugin_controller_eval_report_covers_declared_rows() {
        let report =
            build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report()
                .expect("report");

        assert_eq!(
            report.report_id,
            "tassadar.post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop.eval_report.v1"
        );
        assert_eq!(report.dependency_rows.len(), 5);
        assert_eq!(report.controller_case_rows.len(), 6);
        assert_eq!(report.control_trace_rows.len(), 46);
        assert_eq!(report.host_negative_rows.len(), 10);
        assert_eq!(report.validation_rows.len(), 10);
        assert!(report.contract_green);
        assert!(report.closure_bundle_bound_by_digest);
        assert!(report.weighted_plugin_control_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(report.deferred_issue_ids.is_empty());
    }

    #[test]
    fn post_article_weighted_plugin_controller_eval_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report()
                .expect("report");
        let committed: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport =
            read_repo_json(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF,
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report.json"
            )
        );
    }

    #[test]
    fn write_post_article_weighted_plugin_controller_eval_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report.json",
        );
        let written =
            write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopEvalReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
