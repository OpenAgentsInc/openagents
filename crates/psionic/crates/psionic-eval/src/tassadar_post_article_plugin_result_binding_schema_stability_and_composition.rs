use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle,
    TassadarPostArticlePluginResultBindingCaseRow,
    TassadarPostArticlePluginResultBindingCompatibilityStatus,
    TassadarPostArticlePluginResultBindingNegativeRow,
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundleError,
    TassadarPostArticlePluginResultCompositionRow,
    TassadarPostArticlePluginResultEvidenceBoundaryRow,
    TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
};
use psionic_transformer::{
    build_tassadar_post_article_plugin_result_binding_contract,
    TassadarPostArticlePluginResultBindingContract,
    TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID,
};

use crate::{
    build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report,
    TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReportError,
    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_EVAL_REPORT_REF:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_result_binding_schema_stability_and_composition_report.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_CHECKER_REF:
    &str =
    "scripts/check-tassadar-post-article-plugin-result-binding-schema-stability-and-composition.sh";

const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const PLUGIN_SYSTEM_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const EFFECTFUL_REPLAY_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json";
const INTERNAL_COMPONENT_ABI_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_component_abi_report.json";
const TRANSFORMER_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_plugin_result_binding_contract.rs";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionStatus {
    Green,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionDependencyClass {
    ConformancePrecedent,
    ReplayPrecedent,
    AbiPrecedent,
    TransformerContract,
    RuntimeEvidence,
    DesignInput,
    AuditInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub conformance_eval_report_id: String,
    pub conformance_eval_report_digest: String,
    pub packet_abi_version: String,
    pub host_owned_runtime_api_id: String,
    pub engine_abstraction_id: String,
    pub invocation_receipt_profile_id: String,
    pub conformance_harness_id: String,
    pub benchmark_harness_id: String,
    pub result_binding_contract_id: String,
    pub result_binding_contract_digest: String,
    pub model_loop_return_profile_id: String,
    pub runtime_bundle_id: String,
    pub runtime_bundle_digest: String,
    pub runtime_bundle_ref: String,
    pub effectful_replay_audit_report_id: String,
    pub internal_component_abi_report_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingDependencyRow {
    pub dependency_id: String,
    pub dependency_class:
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionDependencyClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_report_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub conformance_eval_report_ref: String,
    pub runtime_bundle_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_refs: Vec<String>,
    pub machine_identity_binding: TassadarPostArticlePluginResultBindingMachineIdentityBinding,
    pub transformer_contract: TassadarPostArticlePluginResultBindingContract,
    pub dependency_rows: Vec<TassadarPostArticlePluginResultBindingDependencyRow>,
    pub binding_rows: Vec<TassadarPostArticlePluginResultBindingCaseRow>,
    pub evidence_boundary_rows: Vec<TassadarPostArticlePluginResultEvidenceBoundaryRow>,
    pub composition_rows: Vec<TassadarPostArticlePluginResultCompositionRow>,
    pub negative_rows: Vec<TassadarPostArticlePluginResultBindingNegativeRow>,
    pub validation_rows: Vec<TassadarPostArticlePluginResultBindingValidationRow>,
    pub contract_status: TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionStatus,
    pub contract_green: bool,
    pub result_binding_contract_green: bool,
    pub explicit_output_to_state_digest_binding: bool,
    pub schema_evolution_fail_closed: bool,
    pub typed_refusal_normalization_preserved: bool,
    pub version_skew_fail_closed: bool,
    pub proof_vs_observational_boundary_explicit: bool,
    pub semantic_composition_closure_green: bool,
    pub non_lossy_schema_transition_required: bool,
    pub ambiguous_composition_blocked: bool,
    pub adapter_defined_return_path_blocked: bool,
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
pub enum TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReportError {
    #[error(transparent)]
    Conformance(
        #[from] TassadarPostArticlePluginConformanceSandboxAndBenchmarkHarnessEvalReportError,
    ),
    #[error(transparent)]
    Runtime(#[from] TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundleError),
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

pub fn build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report(
) -> Result<
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReport,
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReportError,
> {
    let conformance =
        build_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report()?;
    let runtime_bundle =
        build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle();
    let transformer_contract = build_tassadar_post_article_plugin_result_binding_contract();
    let effectful_replay_audit: EffectfulReplayAuditFixture =
        read_repo_json(EFFECTFUL_REPLAY_AUDIT_REPORT_REF)?;
    let internal_component_abi: InternalComponentAbiFixture =
        read_repo_json(INTERNAL_COMPONENT_ABI_REPORT_REF)?;
    let transformer_contract_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_result_binding_contract|",
        &transformer_contract,
    );

    let machine_identity_binding = TassadarPostArticlePluginResultBindingMachineIdentityBinding {
        machine_identity_id: conformance
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: conformance.machine_identity_binding.canonical_model_id.clone(),
        canonical_route_id: conformance.machine_identity_binding.canonical_route_id.clone(),
        canonical_route_descriptor_digest: conformance
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        canonical_weight_bundle_digest: conformance
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: conformance
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        continuation_contract_id: conformance
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: conformance
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: conformance
            .machine_identity_binding
            .computational_model_statement_id
            .clone(),
        conformance_eval_report_id: conformance.report_id.clone(),
        conformance_eval_report_digest: conformance.report_digest.clone(),
        packet_abi_version: conformance.machine_identity_binding.packet_abi_version.clone(),
        host_owned_runtime_api_id: conformance
            .machine_identity_binding
            .host_owned_runtime_api_id
            .clone(),
        engine_abstraction_id: conformance
            .machine_identity_binding
            .engine_abstraction_id
            .clone(),
        invocation_receipt_profile_id: conformance
            .machine_identity_binding
            .invocation_receipt_profile_id
            .clone(),
        conformance_harness_id: conformance
            .machine_identity_binding
            .conformance_harness_id
            .clone(),
        benchmark_harness_id: conformance
            .machine_identity_binding
            .benchmark_harness_id
            .clone(),
        result_binding_contract_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID,
        ),
        result_binding_contract_digest: transformer_contract_digest.clone(),
        model_loop_return_profile_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID,
        ),
        runtime_bundle_id: runtime_bundle.bundle_id.clone(),
        runtime_bundle_digest: runtime_bundle.bundle_digest.clone(),
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
        ),
        effectful_replay_audit_report_id: effectful_replay_audit.report_id.clone(),
        internal_component_abi_report_id: internal_component_abi.report_id.clone(),
        detail: format!(
            "machine_identity_id=`{}` canonical_route_id=`{}` conformance_eval_report_id=`{}` result_binding_contract_id=`{}` and runtime_bundle_id=`{}` remain bound together.",
            conformance.machine_identity_binding.machine_identity_id,
            conformance.machine_identity_binding.canonical_route_id,
            conformance.report_id,
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID,
            runtime_bundle.bundle_id,
        ),
    };

    let dependency_rows = vec![
        dependency_row(
            "conformance_harness_closure_green",
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionDependencyClass::ConformancePrecedent,
            conformance.contract_green && conformance.deferred_issue_ids.is_empty(),
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF,
            Some(conformance.report_id.clone()),
            Some(conformance.report_digest.clone()),
            "the prior conformance harness is green and no longer carries a defer pointer now that result binding closes its only declared gap.",
        ),
        dependency_row(
            "effectful_replay_audit_available",
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionDependencyClass::ReplayPrecedent,
            effectful_replay_audit.challengeable_case_count == 3
                && effectful_replay_audit.refusal_case_count == 3,
            EFFECTFUL_REPLAY_AUDIT_REPORT_REF,
            Some(effectful_replay_audit.report_id.clone()),
            Some(effectful_replay_audit.report_digest.clone()),
            "the replay precedent keeps proof-carrying receipts and typed refusals explicit for effectful surfaces that feed the plugin result-binding return path.",
        ),
        dependency_row(
            "internal_component_abi_available",
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionDependencyClass::AbiPrecedent,
            internal_component_abi.runtime_bundle.exact_interface_parity_count == 3
                && internal_component_abi.runtime_bundle.exact_refusal_parity_count == 2,
            INTERNAL_COMPONENT_ABI_REPORT_REF,
            Some(internal_component_abi.report_id.clone()),
            Some(internal_component_abi.report_digest.clone()),
            "the bounded internal component ABI keeps exact interface parity and exact refusal parity explicit so the plugin return path does not widen into adapter-defined union semantics.",
        ),
        dependency_row(
            "transformer_result_binding_contract_present",
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionDependencyClass::TransformerContract,
            transformer_contract.contract_id
                == TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID
                && transformer_contract.model_loop_return_profile_id
                    == TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID,
            TRANSFORMER_CONTRACT_SOURCE_REF,
            Some(transformer_contract.contract_id.clone()),
            Some(transformer_contract_digest.clone()),
            "the transformer-owned contract is the canonical abstract result-binding artifact for weighted plugin return-path ownership.",
        ),
        dependency_row(
            "runtime_result_binding_bundle_present",
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionDependencyClass::RuntimeEvidence,
            runtime_bundle.result_binding_contract_id
                == TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID
                && runtime_bundle.model_loop_return_profile_id
                    == TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID,
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
            Some(runtime_bundle.bundle_id.clone()),
            Some(runtime_bundle.bundle_digest.clone()),
            "the runtime bundle carries the concrete result-binding, evidence-boundary, composition, and fail-closed negative rows against the same contract ids.",
        ),
        dependency_row(
            "plugin_system_spec_declared",
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionDependencyClass::DesignInput,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system spec remains the design input for the explicit model-loop return path, typed refusal posture, and weighted continue/retry/stop loop.",
        ),
        dependency_row(
            "plugin_system_audit_declared",
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionDependencyClass::AuditInput,
            true,
            PLUGIN_SYSTEM_AUDIT_REF,
            None,
            None,
            "the plugin-system audit remains the supporting law source for semantic-preservation, non-lossy composition, and fail-closed ambiguity posture.",
        ),
    ];

    let result_binding_contract_green = transformer_contract
        .schema_evolution_rule_rows
        .iter()
        .chain(transformer_contract.refusal_normalization_rule_rows.iter())
        .chain(transformer_contract.composition_law_rows.iter())
        .all(|row| row.green);
    let explicit_output_to_state_digest_binding =
        runtime_bundle.binding_rows.iter().all(|row| {
            !row.output_digest.is_empty() && !row.next_model_visible_state_digest.is_empty()
        }) && runtime_bundle.binding_rows.iter().any(|row| {
            row.case_id == "fetch_text_exact_binding"
                && row.schema_transition_class_id == "schema_transition.exact_identity.v1"
        });
    let schema_evolution_fail_closed = runtime_bundle.binding_rows.iter().any(|row| {
        row.compatibility_status
            == TassadarPostArticlePluginResultBindingCompatibilityStatus::BackwardCompatible
            && row.semantic_closure_preserved
    }) && runtime_bundle.negative_rows.iter().any(|row| {
        row.check_id == "schema_auto_repair_blocked"
            && row.green
            && row.typed_refusal_reason_id == "schema_auto_repair_blocked"
    });
    let typed_refusal_normalization_preserved = runtime_bundle.binding_rows.iter().any(|row| {
        row.compatibility_status
            == TassadarPostArticlePluginResultBindingCompatibilityStatus::RefusalNormalized
            && row.typed_refusal_reason_id.as_deref() == Some("runtime_timeout")
    }) && transformer_contract
        .refusal_normalization_rule_rows
        .iter()
        .any(|row| row.rule_id == "typed_failure_classes_preserved" && row.green);
    let version_skew_fail_closed = runtime_bundle.binding_rows.iter().any(|row| {
        row.compatibility_status
            == TassadarPostArticlePluginResultBindingCompatibilityStatus::VersionSkewBlocked
            && row.typed_refusal_reason_id.as_deref() == Some("model_plugin_schema_version_skew")
    }) && transformer_contract
        .refusal_normalization_rule_rows
        .iter()
        .any(|row| row.rule_id == "version_skew_refuses_before_reinjection" && row.green);
    let proof_vs_observational_boundary_explicit = runtime_bundle
        .evidence_boundary_rows
        .iter()
        .any(|row| row.proof_required_for_reinjection && row.proof_receipt_id.is_some())
        && runtime_bundle.evidence_boundary_rows.iter().any(|row| {
            !row.proof_required_for_reinjection
                && row.proof_receipt_id.is_none()
                && row.observational_audit_id.is_some()
        });
    let semantic_composition_closure_green = runtime_bundle
        .composition_rows
        .iter()
        .filter(|row| !row.ambiguous_composition_blocked)
        .all(|row| row.semantic_closure_preserved);
    let non_lossy_schema_transition_required = runtime_bundle
        .composition_rows
        .iter()
        .all(|row| row.non_lossy_transition_required);
    let ambiguous_composition_blocked = runtime_bundle.composition_rows.iter().any(|row| {
        row.ambiguous_composition_blocked
            && row.typed_refusal_reason_id.as_deref() == Some("ambiguous_composition_introduced")
    }) && runtime_bundle.negative_rows.iter().any(|row| {
        row.check_id == "ambiguous_composition_blocked"
            && row.green
            && row.typed_refusal_reason_id == "ambiguous_composition_introduced"
    });
    let adapter_defined_return_path_blocked = transformer_contract
        .composition_law_rows
        .iter()
        .any(|row| row.rule_id == "adapter_defined_return_path_forbidden" && row.green);

    let validation_rows = vec![
        validation_row(
            "output_digest_binds_next_model_state",
            explicit_output_to_state_digest_binding,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
                TRANSFORMER_CONTRACT_SOURCE_REF,
            ],
            "every runtime binding row keeps one explicit output digest to next model-visible state digest binding under the transformer-owned contract.",
        ),
        validation_row(
            "backward_compatible_schema_evolution_explicit",
            schema_evolution_fail_closed,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
                TRANSFORMER_CONTRACT_SOURCE_REF,
            ],
            "backward-compatible additive evolution stays explicit and fail-closed instead of allowing schema auto-repair to rewrite task meaning.",
        ),
        validation_row(
            "typed_refusal_normalization_preserved",
            typed_refusal_normalization_preserved,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
                TRANSFORMER_CONTRACT_SOURCE_REF,
                EFFECTFUL_REPLAY_AUDIT_REPORT_REF,
            ],
            "typed refusal classes remain explicit and replay-compatible when plugin outputs normalize back into the model loop.",
        ),
        validation_row(
            "version_skew_fails_closed",
            version_skew_fail_closed,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
                TRANSFORMER_CONTRACT_SOURCE_REF,
            ],
            "model-version versus plugin-schema skew refuses reinjection before any adapter-defined repair path can run.",
        ),
        validation_row(
            "proof_required_bindings_stay_proof_bound",
            proof_vs_observational_boundary_explicit,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
                EFFECTFUL_REPLAY_AUDIT_REPORT_REF,
            ],
            "proof-carrying bindings stay distinct from observational audits so observations do not silently rewrite the proof state.",
        ),
        validation_row(
            "observational_only_bindings_do_not_overclaim",
            proof_vs_observational_boundary_explicit,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
                INTERNAL_COMPONENT_ABI_REPORT_REF,
            ],
            "observational compatibility projections remain bounded and do not masquerade as exact proof-carrying equivalence.",
        ),
        validation_row(
            "semantic_closure_under_multi_step_chaining",
            semantic_composition_closure_green,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
                PLUGIN_SYSTEM_AUDIT_REF,
            ],
            "multi-step composition remains semantically closed for the admitted chains instead of relying on host-only hidden glue.",
        ),
        validation_row(
            "non_lossy_schema_transition_required",
            non_lossy_schema_transition_required,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
                PLUGIN_SYSTEM_AUDIT_REF,
            ],
            "every composition row keeps non-lossy schema transition as a requirement rather than an optional best-effort adaptation.",
        ),
        validation_row(
            "ambiguous_composition_fails_closed",
            ambiguous_composition_blocked,
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
                PLUGIN_SYSTEM_AUDIT_REF,
            ],
            "composition ambiguity remains a typed refusal instead of letting the host choose one meaning.",
        ),
        validation_row(
            "lossy_coercion_refused",
            runtime_bundle.negative_rows.iter().any(|row| {
                row.check_id == "lossy_coercion_refused"
                    && row.green
                    && row.typed_refusal_reason_id == "lossy_schema_coercion_refused"
            }),
            &[TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF],
            "lossy field coercion remains explicitly refused on reinjection.",
        ),
        validation_row(
            "semantic_incompleteness_refused",
            runtime_bundle.negative_rows.iter().any(|row| {
                row.check_id == "semantically_incomplete_reinjection_blocked"
                    && row.green
                    && row.typed_refusal_reason_id
                        == "semantically_incomplete_reinjection_blocked"
            }),
            &[
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
                PLUGIN_SYSTEM_AUDIT_REF,
            ],
            "semantically incomplete reinjection remains fail-closed instead of silently truncating task meaning.",
        ),
        validation_row(
            "adapter_defined_return_path_forbidden",
            adapter_defined_return_path_blocked,
            &[TRANSFORMER_CONTRACT_SOURCE_REF, LOCAL_PLUGIN_SYSTEM_SPEC_REF],
            "the result return path stays contract-defined and stable enough for later weighted continuation to bind to it honestly.",
        ),
    ];

    let contract_green = conformance.contract_green
        && dependency_rows.iter().all(|row| row.satisfied)
        && result_binding_contract_green
        && explicit_output_to_state_digest_binding
        && schema_evolution_fail_closed
        && typed_refusal_normalization_preserved
        && version_skew_fail_closed
        && proof_vs_observational_boundary_explicit
        && semantic_composition_closure_green
        && non_lossy_schema_transition_required
        && ambiguous_composition_blocked
        && adapter_defined_return_path_blocked
        && validation_rows.iter().all(|row| row.green);
    let contract_status = if contract_green {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionStatus::Green
    } else {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionStatus::Incomplete
    };

    let supporting_material_refs = vec![
        String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF,
        ),
        String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
        ),
        String::from(EFFECTFUL_REPLAY_AUDIT_REPORT_REF),
        String::from(INTERNAL_COMPONENT_ABI_REPORT_REF),
        String::from(TRANSFORMER_CONTRACT_SOURCE_REF),
        String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        String::from(PLUGIN_SYSTEM_AUDIT_REF),
    ];

    let mut report = TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_plugin_result_binding_schema_stability_and_composition.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_CHECKER_REF,
        ),
        conformance_eval_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF,
        ),
        runtime_bundle_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
        ),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        supporting_material_refs,
        machine_identity_binding,
        transformer_contract,
        dependency_rows,
        binding_rows: runtime_bundle.binding_rows.clone(),
        evidence_boundary_rows: runtime_bundle.evidence_boundary_rows.clone(),
        composition_rows: runtime_bundle.composition_rows.clone(),
        negative_rows: runtime_bundle.negative_rows.clone(),
        validation_rows,
        contract_status,
        contract_green,
        result_binding_contract_green,
        explicit_output_to_state_digest_binding,
        schema_evolution_fail_closed,
        typed_refusal_normalization_preserved,
        version_skew_fail_closed,
        proof_vs_observational_boundary_explicit,
        semantic_composition_closure_green,
        non_lossy_schema_transition_required,
        ambiguous_composition_blocked,
        adapter_defined_return_path_blocked,
        operator_internal_only_posture: conformance.operator_internal_only_posture,
        rebase_claim_allowed: conformance.rebase_claim_allowed,
        plugin_capability_claim_allowed: false,
        weighted_plugin_control_allowed: false,
        plugin_publication_allowed: false,
        served_public_universality_allowed: false,
        arbitrary_software_capability_allowed: false,
        deferred_issue_ids: Vec::new(),
        claim_boundary: String::from(
            "this eval-owned closure report binds the transformer-owned plugin result-binding contract plus the runtime-owned schema-stability and composition evidence to the canonical post-article machine identity. It proves stable output-to-state digest binding, fail-closed schema evolution, typed refusal normalization, proof-versus-observational separation, and semantic closure under bounded multi-step composition while still refusing to treat plugin publication, served/public universality, or arbitrary software capability as closed. Weighted controller ownership now closes in the dedicated `TAS-204` controller-trace artifact instead of being implied by this result-binding report alone.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article plugin result-binding report keeps contract_status={:?}, dependency_rows={}, binding_rows={}, evidence_boundary_rows={}, composition_rows={}, negative_rows={}, validation_rows={}, and deferred_issue_ids={}.",
        report.contract_status,
        report.dependency_rows.len(),
        report.binding_rows.len(),
        report.evidence_boundary_rows.len(),
        report.composition_rows.len(),
        report.negative_rows.len(),
        report.validation_rows.len(),
        report.deferred_issue_ids.len(),
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report_path(
) -> PathBuf {
    repo_root()
        .join(TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_EVAL_REPORT_REF)
}

pub fn write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReport,
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report =
        build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report(
        )?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn dependency_row(
    dependency_id: &str,
    dependency_class:
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionDependencyClass,
    satisfied: bool,
    source_ref: &str,
    bound_report_id: Option<String>,
    bound_report_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticlePluginResultBindingDependencyRow {
    TassadarPostArticlePluginResultBindingDependencyRow {
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
) -> TassadarPostArticlePluginResultBindingValidationRow {
    TassadarPostArticlePluginResultBindingValidationRow {
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
struct EffectfulReplayAuditFixture {
    report_id: String,
    report_digest: String,
    challengeable_case_count: u32,
    refusal_case_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct InternalComponentAbiFixture {
    report_id: String,
    report_digest: String,
    runtime_bundle: InternalComponentAbiRuntimeBundleFixture,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct InternalComponentAbiRuntimeBundleFixture {
    exact_interface_parity_count: u32,
    exact_refusal_parity_count: u32,
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
) -> Result<T, TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report,
        read_repo_json,
        tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report_path,
        write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report,
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReport,
        TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_EVAL_REPORT_REF,
    };

    #[test]
    fn post_article_plugin_result_binding_report_covers_declared_rows() {
        let report =
            build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report()
                .expect("report");

        assert_eq!(
            report.report_id,
            "tassadar.post_article_plugin_result_binding_schema_stability_and_composition.report.v1"
        );
        assert_eq!(
            report.transformer_contract.contract_id,
            "tassadar.weighted_plugin.result_binding_contract.v1"
        );
        assert_eq!(
            report.machine_identity_binding.model_loop_return_profile_id,
            "tassadar.weighted_plugin.model_loop_return_profile.v1"
        );
        assert_eq!(report.dependency_rows.len(), 7);
        assert_eq!(report.binding_rows.len(), 6);
        assert_eq!(report.evidence_boundary_rows.len(), 4);
        assert_eq!(report.composition_rows.len(), 4);
        assert_eq!(report.negative_rows.len(), 4);
        assert_eq!(report.validation_rows.len(), 12);
        assert!(report.contract_green);
        assert!(report.result_binding_contract_green);
        assert!(report.explicit_output_to_state_digest_binding);
        assert!(report.schema_evolution_fail_closed);
        assert!(report.typed_refusal_normalization_preserved);
        assert!(report.version_skew_fail_closed);
        assert!(report.proof_vs_observational_boundary_explicit);
        assert!(report.semantic_composition_closure_green);
        assert!(report.non_lossy_schema_transition_required);
        assert!(report.ambiguous_composition_blocked);
        assert!(report.adapter_defined_return_path_blocked);
        assert!(report.deferred_issue_ids.is_empty());
        assert!(report.operator_internal_only_posture);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_plugin_result_binding_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report()
                .expect("report");
        let committed: TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReport =
            read_repo_json(
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_EVAL_REPORT_REF,
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_plugin_result_binding_schema_stability_and_composition_report.json")
        );
    }

    #[test]
    fn write_post_article_plugin_result_binding_report_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_plugin_result_binding_schema_stability_and_composition_report.json");
        let written =
            write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_eval_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionEvalReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
