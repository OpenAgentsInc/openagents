use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle,
    TassadarPostArticleWeightedPluginControlTokenKind,
    TassadarPostArticleWeightedPluginControlTraceRow,
    TassadarPostArticleWeightedPluginControllerCaseRow,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundle,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundleError,
    TassadarPostArticleWeightedPluginHostNegativeRow,
    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
};

pub const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report.json";
pub const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_CHECKER_REF:
    &str =
    "scripts/check-tassadar-post-article-weighted-plugin-controller-trace-and-refusal-aware-model-loop.sh";

const RESULT_BINDING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_result_binding_schema_stability_and_composition_report.json";
const CONTROL_PLANE_PROOF_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_control_plane_decision_provenance_proof_report.json";
const ACCEPTANCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_equivalence_acceptance_gate_report.json";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const PLUGIN_SYSTEM_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleWeightedPluginControllerTraceDependencyClass {
    ResultBindingPrecedent,
    ControlPlanePrecedent,
    AcceptanceGatePrecedent,
    RuntimeEvidence,
    DesignInput,
    AuditInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub result_binding_report_id: String,
    pub result_binding_report_digest: String,
    pub control_plane_report_id: String,
    pub control_plane_report_digest: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub result_binding_contract_id: String,
    pub model_loop_return_profile_id: String,
    pub control_trace_contract_id: String,
    pub control_trace_profile_id: String,
    pub determinism_profile_id: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub runtime_bundle_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceDependencyRow {
    pub dependency_id: String,
    pub dependency_class: TassadarPostArticleWeightedPluginControllerTraceDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub result_binding_report_ref: String,
    pub control_plane_proof_report_ref: String,
    pub acceptance_gate_report_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding:
        TassadarPostArticleWeightedPluginControllerTraceMachineIdentityBinding,
    pub runtime_bundle_ref: String,
    pub runtime_bundle:
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundle,
    pub dependency_rows: Vec<TassadarPostArticleWeightedPluginControllerTraceDependencyRow>,
    pub controller_case_rows: Vec<TassadarPostArticleWeightedPluginControllerCaseRow>,
    pub control_trace_rows: Vec<TassadarPostArticleWeightedPluginControlTraceRow>,
    pub host_negative_rows: Vec<TassadarPostArticleWeightedPluginHostNegativeRow>,
    pub validation_rows: Vec<TassadarPostArticleWeightedPluginControllerTraceValidationRow>,
    pub contract_status:
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopStatus,
    pub contract_green: bool,
    pub control_trace_contract_green: bool,
    pub plugin_selection_model_owned: bool,
    pub export_selection_model_owned: bool,
    pub packet_arguments_model_owned: bool,
    pub multi_step_sequencing_model_owned: bool,
    pub retry_decisions_model_owned: bool,
    pub stop_conditions_model_owned: bool,
    pub typed_refusal_returned_to_model_loop: bool,
    pub host_executes_but_is_not_planner: bool,
    pub determinism_contract_explicit: bool,
    pub external_signal_boundary_closed: bool,
    pub hidden_host_orchestration_negative_rows_green: bool,
    pub adversarial_host_behavior_negative_rows_green: bool,
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
pub enum TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError {
    #[error(transparent)]
    Runtime(
        #[from] TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopBundleError,
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

pub fn build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report(
) -> Result<
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReport,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError,
> {
    let result_binding: ResultBindingEvalFixture = read_repo_json(RESULT_BINDING_REPORT_REF)?;
    let control_plane: ControlPlaneProofFixture = read_repo_json(CONTROL_PLANE_PROOF_REPORT_REF)?;
    let acceptance_gate: AcceptanceGateFixture = read_repo_json(ACCEPTANCE_GATE_REPORT_REF)?;
    let runtime_bundle =
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle();

    let machine_identity_binding = TassadarPostArticleWeightedPluginControllerTraceMachineIdentityBinding {
        machine_identity_id: result_binding
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: result_binding
            .machine_identity_binding
            .canonical_model_id
            .clone(),
        canonical_route_id: result_binding
            .machine_identity_binding
            .canonical_route_id
            .clone(),
        canonical_route_descriptor_digest: result_binding
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: result_binding
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: result_binding
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: result_binding
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: result_binding
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: result_binding
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        result_binding_report_id: result_binding.report_id.clone(),
        result_binding_report_digest: result_binding.report_digest.clone(),
        control_plane_report_id: control_plane.report_id.clone(),
        control_plane_report_digest: control_plane.report_digest.clone(),
        packet_abi_version: result_binding
            .machine_identity_binding
            .packet_abi_version
            .clone(),
        host_owned_runtime_api_id: result_binding
            .machine_identity_binding
            .host_owned_runtime_api_id
            .clone(),
        engine_abstraction_id: result_binding
            .machine_identity_binding
            .engine_abstraction_id
            .clone(),
        invocation_receipt_profile_id: result_binding
            .machine_identity_binding
            .invocation_receipt_profile_id
            .clone(),
        result_binding_contract_id: result_binding
            .machine_identity_binding
            .result_binding_contract_id
            .clone(),
        model_loop_return_profile_id: result_binding
            .machine_identity_binding
            .model_loop_return_profile_id
            .clone(),
        control_trace_contract_id: runtime_bundle.control_trace_contract_id.clone(),
        control_trace_profile_id: runtime_bundle.control_trace_profile_id.clone(),
        determinism_profile_id: runtime_bundle.determinism_profile_id.clone(),
        runtime_bundle_id: runtime_bundle.bundle_id.clone(),
        runtime_bundle_digest: runtime_bundle.bundle_digest.clone(),
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
        ),
        detail: format!(
            "machine_identity_id=`{}` canonical_route_id=`{}` result_binding_report_id=`{}` control_plane_report_id=`{}` and runtime_bundle_id=`{}` remain bound together.",
            result_binding.machine_identity_binding.machine_identity_id,
            result_binding.machine_identity_binding.canonical_route_id,
            result_binding.report_id,
            control_plane.report_id,
            runtime_bundle.bundle_id,
        ),
    };

    let dependency_rows = vec![
        dependency_row(
            "result_binding_closure_green",
            TassadarPostArticleWeightedPluginControllerTraceDependencyClass::ResultBindingPrecedent,
            result_binding.contract_green && result_binding.deferred_issue_ids.is_empty(),
            RESULT_BINDING_REPORT_REF,
            Some(result_binding.report_id.clone()),
            Some(result_binding.report_digest.clone()),
            "the result-binding contract must be green and no longer carry the controller defer pointer before weighted controller ownership can close honestly.",
        ),
        dependency_row(
            "control_plane_provenance_green",
            TassadarPostArticleWeightedPluginControllerTraceDependencyClass::ControlPlanePrecedent,
            control_plane.control_plane_ownership_green && control_plane.replay_posture_green,
            CONTROL_PLANE_PROOF_REPORT_REF,
            Some(control_plane.report_id.clone()),
            Some(control_plane.report_digest.clone()),
            "the earlier control-plane proof must stay green so plugin selection, retry, and stop ownership extend an existing control proof instead of inventing a new one.",
        ),
        dependency_row(
            "article_acceptance_gate_green",
            TassadarPostArticleWeightedPluginControllerTraceDependencyClass::AcceptanceGatePrecedent,
            acceptance_gate.acceptance_status == "green" && acceptance_gate.public_claim_allowed,
            ACCEPTANCE_GATE_REPORT_REF,
            Some(acceptance_gate.report_id.clone()),
            Some(acceptance_gate.report_digest.clone()),
            "the weighted controller trace must stay tied to the canonical owned route rather than a mixed helper route.",
        ),
        dependency_row(
            "runtime_bundle_present",
            TassadarPostArticleWeightedPluginControllerTraceDependencyClass::RuntimeEvidence,
            runtime_bundle.result_binding_contract_id
                == result_binding.machine_identity_binding.result_binding_contract_id
                && runtime_bundle.model_loop_return_profile_id
                    == result_binding.machine_identity_binding.model_loop_return_profile_id
                && runtime_bundle.starter_plugin_admission_rows.iter().any(|row| {
                    row.plugin_id == "plugin.text.stats"
                        && row.derived_from_shared_registration
                        && row.derived_from_catalog_exposure
                })
                && runtime_bundle.starter_plugin_admission_rows.iter().any(|row| {
                    row.plugin_id == "plugin.http.fetch_text"
                        && row.authoring_class_id == "networked_read_only"
                        && row.origin_class_id == "user_added"
                        && row.derived_from_shared_registration
                        && row.derived_from_catalog_exposure
                })
                && runtime_bundle.control_trace_rows.iter().any(|row| {
                    row.control_token_kind == TassadarPostArticleWeightedPluginControlTokenKind::Retry
                        && row.decision_owner_id == "model_weights"
                })
                && runtime_bundle.control_trace_rows.iter().any(|row| {
                    row.case_id == "text_stats_stop_after_success"
                        && row.control_token_kind
                            == TassadarPostArticleWeightedPluginControlTokenKind::PluginSelect
                        && row.decision_owner_id == "model_weights"
                        && row.plugin_id.as_deref() == Some("plugin.text.stats")
                }),
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
            Some(runtime_bundle.bundle_id.clone()),
            Some(runtime_bundle.bundle_digest.clone()),
            "the runtime bundle must carry explicit plugin selection, refusal, retry, continue, and stop rows against the same result-binding contract ids, and it must admit both the capability-free and networked_read_only user-added starter plugins from the shared registration and catalog path into the canonical weighted controller lane.",
        ),
        dependency_row(
            "plugin_system_spec_declared",
            TassadarPostArticleWeightedPluginControllerTraceDependencyClass::DesignInput,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system spec remains the design input for plugin selection, sequencing, refusal, retry, and stop ownership.",
        ),
        dependency_row(
            "plugin_system_audit_declared",
            TassadarPostArticleWeightedPluginControllerTraceDependencyClass::AuditInput,
            true,
            PLUGIN_SYSTEM_AUDIT_REF,
            None,
            None,
            "the plugin-system audit remains the supporting law source for determinism, information boundaries, and planner-indistinguishability negatives.",
        ),
    ];

    let plugin_selection_model_owned = runtime_bundle
        .controller_case_rows
        .iter()
        .all(|row| row.model_selects_plugin)
        && runtime_bundle.control_trace_rows.iter().all(|row| {
            row.control_token_kind
                != TassadarPostArticleWeightedPluginControlTokenKind::PluginSelect
                || row.decision_owner_id == "model_weights"
        });
    let export_selection_model_owned = runtime_bundle
        .controller_case_rows
        .iter()
        .all(|row| row.model_selects_export)
        && runtime_bundle.control_trace_rows.iter().all(|row| {
            row.control_token_kind
                != TassadarPostArticleWeightedPluginControlTokenKind::ExportSelect
                || row.decision_owner_id == "model_weights"
        });
    let packet_arguments_model_owned = runtime_bundle
        .controller_case_rows
        .iter()
        .all(|row| row.packet_arguments_model_owned)
        && runtime_bundle.control_trace_rows.iter().all(|row| {
            row.control_token_kind
                != TassadarPostArticleWeightedPluginControlTokenKind::PacketEncode
                || row.decision_owner_id == "model_weights"
        });
    let multi_step_sequencing_model_owned = runtime_bundle
        .controller_case_rows
        .iter()
        .all(|row| row.sequencing_model_owned)
        && runtime_bundle.control_trace_rows.iter().all(|row| {
            !matches!(
                row.control_token_kind,
                TassadarPostArticleWeightedPluginControlTokenKind::Continue
                    | TassadarPostArticleWeightedPluginControlTokenKind::Stop
            ) || row.decision_owner_id == "model_weights"
        });
    let retry_decisions_model_owned = runtime_bundle
        .controller_case_rows
        .iter()
        .any(|row| row.retry_model_owned)
        && runtime_bundle.control_trace_rows.iter().any(|row| {
            row.control_token_kind == TassadarPostArticleWeightedPluginControlTokenKind::Retry
                && row.decision_owner_id == "model_weights"
        });
    let stop_conditions_model_owned = runtime_bundle
        .controller_case_rows
        .iter()
        .all(|row| row.stop_model_owned)
        && runtime_bundle
            .control_trace_rows
            .iter()
            .filter(|row| {
                row.control_token_kind == TassadarPostArticleWeightedPluginControlTokenKind::Stop
            })
            .count()
            == runtime_bundle.controller_case_rows.len();
    let typed_refusal_returned_to_model_loop = runtime_bundle
        .controller_case_rows
        .iter()
        .all(|row| row.typed_refusal_returned_to_model_loop)
        && runtime_bundle.control_trace_rows.iter().any(|row| {
            row.control_token_kind
                == TassadarPostArticleWeightedPluginControlTokenKind::ResultRefusal
                && row.typed_refusal_reason_id.as_deref() == Some("runtime_timeout")
        })
        && runtime_bundle.control_trace_rows.iter().any(|row| {
            row.control_token_kind
                == TassadarPostArticleWeightedPluginControlTokenKind::ResultRefusal
                && row.typed_refusal_reason_id.as_deref()
                    == Some("model_plugin_schema_version_skew")
        });
    let host_executes_but_is_not_planner = runtime_bundle
        .controller_case_rows
        .iter()
        .all(|row| row.host_validates_only && row.host_executes_only)
        && runtime_bundle
            .control_trace_rows
            .iter()
            .all(|row| match row.control_token_kind {
                TassadarPostArticleWeightedPluginControlTokenKind::InvocationCommit
                | TassadarPostArticleWeightedPluginControlTokenKind::ResultAccept
                | TassadarPostArticleWeightedPluginControlTokenKind::ResultRefusal => {
                    row.decision_owner_id == "host_runtime"
                }
                _ => row.decision_owner_id == "model_weights",
            });
    let determinism_contract_explicit = control_plane.replay_posture_green
        && control_plane.determinism_class_contract.selected_class == "strict_deterministic"
        && control_plane.equivalent_choice_relation.relation_id
            == runtime_bundle.equivalent_choice_relation_id
        && runtime_bundle.controller_case_rows.iter().all(|row| {
            row.determinism_class_id == control_plane.determinism_class_contract.selected_class
                && row.sampling_policy_id == "sampling_policy.greedy_single_path.v1"
                && row.temperature_policy_id == "temperature.fixed_zero.v1"
                && row.randomness_control_id == "randomness.disallowed.v1"
        });
    let external_signal_boundary_closed = control_plane.information_boundary.green
        && control_plane
            .information_boundary
            .model_hidden_signal_ids
            .iter()
            .any(|signal| signal == "latency")
        && control_plane
            .information_boundary
            .model_hidden_signal_ids
            .iter()
            .any(|signal| signal == "cost")
        && control_plane
            .information_boundary
            .model_hidden_signal_ids
            .iter()
            .any(|signal| signal == "queue_pressure")
        && control_plane
            .information_boundary
            .model_hidden_signal_ids
            .iter()
            .any(|signal| signal == "scheduler_order")
        && control_plane
            .information_boundary
            .model_hidden_signal_ids
            .iter()
            .any(|signal| signal == "cache_hit_rate")
        && control_plane
            .information_boundary
            .model_hidden_signal_ids
            .iter()
            .any(|signal| signal == "helper_selection")
        && runtime_bundle.controller_case_rows.iter().all(|row| {
            row.external_signal_boundary_id
                == "tassadar.weighted_plugin.external_signal_boundary.v1"
        });
    let hidden_host_orchestration_negative_rows_green =
        runtime_bundle.host_negative_rows.iter().all(|row| {
            row.green
                && matches!(
                    row.check_id.as_str(),
                    "hidden_host_side_sequencing_blocked"
                        | "host_auto_retry_blocked"
                        | "fallback_export_selection_blocked"
                        | "heuristic_plugin_ranking_blocked"
                        | "schema_auto_repair_blocked"
                        | "cached_result_substitution_blocked"
                        | "candidate_precomputation_blocked"
                        | "hidden_topk_filtering_blocked"
                )
                .then_some(())
                .is_some()
                || !matches!(
                    row.check_id.as_str(),
                    "hidden_host_side_sequencing_blocked"
                        | "host_auto_retry_blocked"
                        | "fallback_export_selection_blocked"
                        | "heuristic_plugin_ranking_blocked"
                        | "schema_auto_repair_blocked"
                        | "cached_result_substitution_blocked"
                        | "candidate_precomputation_blocked"
                        | "hidden_topk_filtering_blocked"
                )
        }) && runtime_bundle
            .host_negative_rows
            .iter()
            .any(|row| row.check_id == "hidden_host_side_sequencing_blocked" && row.green);
    let adversarial_host_behavior_negative_rows_green = runtime_bundle
        .host_negative_rows
        .iter()
        .any(|row| row.check_id == "helper_substitution_blocked" && row.green)
        && runtime_bundle
            .host_negative_rows
            .iter()
            .any(|row| row.check_id == "runtime_learning_or_policy_drift_blocked" && row.green)
        && control_plane
            .hidden_control_channel_rows
            .iter()
            .all(|row| row.green);

    let validation_rows = vec![
        validation_row(
            "plugin_selection_model_owned",
            plugin_selection_model_owned,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                CONTROL_PLANE_PROOF_REPORT_REF,
            ],
            "plugin selection stays model-owned on every declared controller case.",
        ),
        validation_row(
            "export_selection_model_owned",
            export_selection_model_owned,
            &[TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF],
            "export selection stays model-owned instead of falling back to runtime defaults.",
        ),
        validation_row(
            "packet_arguments_model_owned",
            packet_arguments_model_owned,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                RESULT_BINDING_REPORT_REF,
            ],
            "packet arguments stay model-owned and remain bound to the result-binding contract.",
        ),
        validation_row(
            "multi_step_sequencing_model_owned",
            multi_step_sequencing_model_owned,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                CONTROL_PLANE_PROOF_REPORT_REF,
            ],
            "multi-step continuation and stop transitions stay model-owned instead of host-scripted.",
        ),
        validation_row(
            "retry_decisions_model_owned",
            retry_decisions_model_owned,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                RESULT_BINDING_REPORT_REF,
            ],
            "retry remains a model-owned decision after a typed refusal instead of becoming host policy.",
        ),
        validation_row(
            "stop_conditions_model_owned",
            stop_conditions_model_owned,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                CONTROL_PLANE_PROOF_REPORT_REF,
            ],
            "stop remains model-owned on every declared controller case.",
        ),
        validation_row(
            "typed_refusal_returned_to_model_loop",
            typed_refusal_returned_to_model_loop,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                RESULT_BINDING_REPORT_REF,
            ],
            "typed refusals return into the model loop explicitly instead of being hidden behind runtime retries or downgrade paths.",
        ),
        validation_row(
            "host_executes_but_is_not_planner",
            host_executes_but_is_not_planner,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                CONTROL_PLANE_PROOF_REPORT_REF,
            ],
            "the host executes declared calls and emits receipts but does not own planning tokens.",
        ),
        validation_row(
            "determinism_contract_explicit",
            determinism_contract_explicit,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                CONTROL_PLANE_PROOF_REPORT_REF,
            ],
            "determinism, sampling, temperature, and randomness controls remain explicit and bound to the inherited control-plane proof.",
        ),
        validation_row(
            "external_signal_boundary_closed",
            external_signal_boundary_closed,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                CONTROL_PLANE_PROOF_REPORT_REF,
            ],
            "latency, cost, queue, scheduling, cache, and helper-selection signals remain outside the model-visible controller surface.",
        ),
        validation_row(
            "hidden_host_orchestration_negatives_green",
            hidden_host_orchestration_negative_rows_green,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                PLUGIN_SYSTEM_AUDIT_REF,
            ],
            "host-side sequencing, retry, ranking, filtering, cache substitution, and schema repair negatives remain green.",
        ),
        validation_row(
            "adversarial_host_behaviors_blocked",
            adversarial_host_behavior_negative_rows_green,
            &[
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
                CONTROL_PLANE_PROOF_REPORT_REF,
                PLUGIN_SYSTEM_AUDIT_REF,
            ],
            "helper substitution and runtime policy-drift attacks remain explicitly blocked.",
        ),
    ];

    let control_trace_contract_green = runtime_bundle.controller_case_rows.iter().all(|row| {
        row.model_selects_plugin
            && row.model_selects_export
            && row.packet_arguments_model_owned
            && row.sequencing_model_owned
            && row.stop_model_owned
            && row.host_validates_only
            && row.host_executes_only
            && row.replay_stable
    }) && runtime_bundle
        .host_negative_rows
        .iter()
        .all(|row| row.green);

    let contract_green = dependency_rows.iter().all(|row| row.satisfied)
        && control_trace_contract_green
        && plugin_selection_model_owned
        && export_selection_model_owned
        && packet_arguments_model_owned
        && multi_step_sequencing_model_owned
        && retry_decisions_model_owned
        && stop_conditions_model_owned
        && typed_refusal_returned_to_model_loop
        && host_executes_but_is_not_planner
        && determinism_contract_explicit
        && external_signal_boundary_closed
        && hidden_host_orchestration_negative_rows_green
        && adversarial_host_behavior_negative_rows_green
        && validation_rows.iter().all(|row| row.green);
    let contract_status = if contract_green {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopStatus::Green
    } else {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopStatus::Incomplete
    };

    let supporting_material_refs = vec![
        String::from(RESULT_BINDING_REPORT_REF),
        String::from(CONTROL_PLANE_PROOF_REPORT_REF),
        String::from(ACCEPTANCE_GATE_REPORT_REF),
        String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
        ),
        String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        String::from(PLUGIN_SYSTEM_AUDIT_REF),
    ];

    let mut report =
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReport {
            schema_version: 1,
            report_id: String::from(
                "tassadar.post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop.report.v1",
            ),
            checker_script_ref: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_CHECKER_REF,
            ),
            result_binding_report_ref: String::from(RESULT_BINDING_REPORT_REF),
            control_plane_proof_report_ref: String::from(CONTROL_PLANE_PROOF_REPORT_REF),
            acceptance_gate_report_ref: String::from(ACCEPTANCE_GATE_REPORT_REF),
            local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
            supporting_material_refs,
            machine_identity_binding,
            runtime_bundle_ref: String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_BUNDLE_REF,
            ),
            runtime_bundle,
            dependency_rows,
            controller_case_rows: Vec::new(),
            control_trace_rows: Vec::new(),
            host_negative_rows: Vec::new(),
            validation_rows,
            contract_status,
            contract_green,
            control_trace_contract_green,
            plugin_selection_model_owned,
            export_selection_model_owned,
            packet_arguments_model_owned,
            multi_step_sequencing_model_owned,
            retry_decisions_model_owned,
            stop_conditions_model_owned,
            typed_refusal_returned_to_model_loop,
            host_executes_but_is_not_planner,
            determinism_contract_explicit,
            external_signal_boundary_closed,
            hidden_host_orchestration_negative_rows_green,
            adversarial_host_behavior_negative_rows_green,
            operator_internal_only_posture: true,
            rebase_claim_allowed: result_binding.rebase_claim_allowed,
            plugin_capability_claim_allowed: false,
            weighted_plugin_control_allowed: contract_green,
            plugin_publication_allowed: false,
            served_public_universality_allowed: false,
            arbitrary_software_capability_allowed: false,
            deferred_issue_ids: Vec::new(),
            claim_boundary: String::from(
                "this sandbox report closes the bounded weighted plugin controller trace on the canonical post-article machine identity. It proves that plugin selection, export selection, packet encoding, sequencing, typed-refusal handling, retry, and stop conditions remain model-owned while the host validates and executes declared calls without becoming the planner. It still does not widen plugin authority into publication, trust-tier promotion, served/public universality, or arbitrary public software execution.",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
    report.controller_case_rows = report.runtime_bundle.controller_case_rows.clone();
    report.control_trace_rows = report.runtime_bundle.control_trace_rows.clone();
    report.host_negative_rows = report.runtime_bundle.host_negative_rows.clone();
    report.summary = format!(
        "Post-article weighted plugin controller sandbox report keeps contract_status={:?}, dependency_rows={}, controller_case_rows={}, control_trace_rows={}, host_negative_rows={}, validation_rows={}, weighted_plugin_control_allowed={}, and deferred_issue_ids={}.",
        report.contract_status,
        report.dependency_rows.len(),
        report.controller_case_rows.len(),
        report.control_trace_rows.len(),
        report.host_negative_rows.len(),
        report.validation_rows.len(),
        report.weighted_plugin_control_allowed,
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
    )
}

pub fn write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReport,
    TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report =
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn dependency_row(
    dependency_id: &str,
    dependency_class: TassadarPostArticleWeightedPluginControllerTraceDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleWeightedPluginControllerTraceDependencyRow {
    TassadarPostArticleWeightedPluginControllerTraceDependencyRow {
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
) -> TassadarPostArticleWeightedPluginControllerTraceValidationRow {
    TassadarPostArticleWeightedPluginControllerTraceValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ResultBindingMachineIdentityBindingFixture {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
    computational_model_statement_id: String,
    packet_abi_version: String,
    host_owned_runtime_api_id: String,
    engine_abstraction_id: String,
    invocation_receipt_profile_id: String,
    result_binding_contract_id: String,
    model_loop_return_profile_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ResultBindingEvalFixture {
    report_id: String,
    report_digest: String,
    machine_identity_binding: ResultBindingMachineIdentityBindingFixture,
    contract_green: bool,
    deferred_issue_ids: Vec<String>,
    rebase_claim_allowed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ControlPlaneDeterminismClassFixture {
    selected_class: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ControlPlaneEquivalentChoiceRelationFixture {
    relation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ControlPlaneInformationBoundaryFixture {
    green: bool,
    model_hidden_signal_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ControlPlaneHiddenControlChannelFixture {
    green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ControlPlaneProofFixture {
    report_id: String,
    report_digest: String,
    control_plane_ownership_green: bool,
    replay_posture_green: bool,
    determinism_class_contract: ControlPlaneDeterminismClassFixture,
    equivalent_choice_relation: ControlPlaneEquivalentChoiceRelationFixture,
    information_boundary: ControlPlaneInformationBoundaryFixture,
    hidden_control_channel_rows: Vec<ControlPlaneHiddenControlChannelFixture>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct AcceptanceGateFixture {
    report_id: String,
    report_digest: String,
    acceptance_status: String,
    public_claim_allowed: bool,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-sandbox crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError>
{
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report,
        read_repo_json,
        tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report_path,
        write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report,
        TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReport,
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
    };

    #[test]
    fn post_article_weighted_plugin_controller_sandbox_report_covers_declared_rows() {
        let report =
            build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report()
                .expect("report");

        assert_eq!(
            report.report_id,
            "tassadar.post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop.report.v1"
        );
        assert_eq!(report.dependency_rows.len(), 6);
        assert_eq!(report.runtime_bundle.starter_plugin_admission_rows.len(), 3);
        assert_eq!(report.controller_case_rows.len(), 6);
        assert_eq!(report.control_trace_rows.len(), 46);
        assert_eq!(report.host_negative_rows.len(), 10);
        assert_eq!(report.validation_rows.len(), 12);
        assert!(report.contract_green);
        assert!(report.weighted_plugin_control_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(report.deferred_issue_ids.is_empty());
        assert!(report
            .runtime_bundle
            .starter_plugin_admission_rows
            .iter()
            .any(|row| row.plugin_id == "plugin.text.stats"));
        assert!(report
            .runtime_bundle
            .starter_plugin_admission_rows
            .iter()
            .any(|row| row.plugin_id == "plugin.http.fetch_text"));
        assert!(report
            .runtime_bundle
            .starter_plugin_admission_rows
            .iter()
            .any(|row| row.plugin_id == "plugin.example.echo_guest"));
        assert_eq!(
            report.machine_identity_binding.control_trace_contract_id,
            "tassadar.weighted_plugin.controller_trace_contract.v1"
        );
    }

    #[test]
    fn post_article_weighted_plugin_controller_sandbox_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report()
                .expect("report");
        let committed: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReport =
            read_repo_json(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_REPORT_REF,
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report.json"
            )
        );
    }

    #[test]
    fn write_post_article_weighted_plugin_controller_sandbox_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report.json",
        );
        let written =
            write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticleWeightedPluginControllerTraceAndRefusalAwareModelLoopReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
