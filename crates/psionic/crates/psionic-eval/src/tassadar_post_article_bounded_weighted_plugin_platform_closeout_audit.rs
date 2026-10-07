use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report.json";
pub const TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_CHECKER_REF:
    &str = "scripts/check-tassadar-post-article-bounded-weighted-plugin-platform-closeout-audit.sh";

const TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_turing_completeness_closeout_audit_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_charter_authority_boundary_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_manifest_identity_contract_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_packet_abi_and_rust_pdk_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_runtime_api_and_engine_abstraction_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF_LOCAL:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_REPORT_REF_LOCAL:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_result_binding_schema_stability_and_composition_report.json";
const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report.json";
const TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL:
    &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report.json";
const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_closure_bundle_report.json";
const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const PLUGIN_SYSTEM_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const LOCAL_PLUGIN_SYSTEM_SPEC_REF: &str = "~/code/alpha/tassadar/plugin-system.md";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleBoundedWeightedPluginPlatformCloseoutStatus {
    OperatorGreenServedSuppressed,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass {
    ProofCarrying,
    Contract,
    ObservationalContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleBoundedWeightedPluginPlatformMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub computational_model_statement_id: String,
    pub control_plane_proof_report_id: String,
    pub control_plane_proof_report_digest: String,
    pub control_trace_contract_id: String,
    pub controller_eval_report_id: String,
    pub controller_eval_report_digest: String,
    pub authority_gate_report_id: String,
    pub authority_gate_report_digest: String,
    pub turing_closeout_report_id: String,
    pub turing_closeout_report_digest: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub canonical_architecture_anchor_crate: String,
    pub canonical_architecture_boundary_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleBoundedWeightedPluginPlatformDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleBoundedWeightedPluginPlatformValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub turing_closeout_audit_report_ref: String,
    pub plugin_charter_authority_boundary_report_ref: String,
    pub plugin_manifest_identity_contract_report_ref: String,
    pub plugin_packet_abi_and_rust_pdk_report_ref: String,
    pub plugin_runtime_api_and_engine_abstraction_report_ref: String,
    pub plugin_invocation_receipts_and_replay_classes_report_ref: String,
    pub plugin_world_mount_envelope_compiler_and_admissibility_report_ref: String,
    pub plugin_conformance_sandbox_and_benchmark_harness_eval_report_ref: String,
    pub plugin_result_binding_schema_stability_and_composition_report_ref: String,
    pub weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report_ref: String,
    pub plugin_authority_promotion_publication_and_trust_tier_gate_report_ref: String,
    pub canonical_machine_closure_bundle_report_ref: String,
    pub post_article_turing_audit_ref: String,
    pub plugin_system_turing_audit_ref: String,
    pub local_plugin_system_spec_ref: String,
    pub supporting_material_rows:
        Vec<TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialRow>,
    pub machine_identity_binding:
        TassadarPostArticleBoundedWeightedPluginPlatformMachineIdentityBinding,
    pub dependency_rows: Vec<TassadarPostArticleBoundedWeightedPluginPlatformDependencyRow>,
    pub validation_rows: Vec<TassadarPostArticleBoundedWeightedPluginPlatformValidationRow>,
    pub closeout_status: TassadarPostArticleBoundedWeightedPluginPlatformCloseoutStatus,
    pub closeout_green: bool,
    pub operator_internal_only_posture: bool,
    pub served_plugin_envelope_published: bool,
    pub closure_bundle_embedded_here: bool,
    pub closure_bundle_bound_by_digest: bool,
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
pub enum TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError {
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
struct CommonMachineIdentityInput {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
    #[serde(default)]
    computational_model_statement_id: Option<String>,
    #[serde(default)]
    control_plane_proof_report_id: Option<String>,
    #[serde(default)]
    control_plane_proof_report_digest: Option<String>,
    #[serde(default)]
    control_trace_contract_id: Option<String>,
    #[serde(default)]
    closure_bundle_report_id: Option<String>,
    #[serde(default)]
    closure_bundle_report_digest: Option<String>,
    #[serde(default)]
    closure_bundle_digest: Option<String>,
    #[serde(default)]
    canonical_architecture_anchor_crate: Option<String>,
    #[serde(default)]
    canonical_architecture_boundary_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CommonContractInput {
    report_id: String,
    report_digest: String,
    machine_identity_binding: CommonMachineIdentityInput,
    contract_green: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
    #[serde(default)]
    closure_bundle_bound_by_digest: bool,
    #[serde(default)]
    deferred_issue_ids: Vec<String>,
    #[serde(default)]
    operator_internal_only_posture: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TuringCloseoutInput {
    report_id: String,
    report_digest: String,
    machine_identity_binding: CommonMachineIdentityInput,
    closeout_green: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
    canonical_route_truth_carrier: bool,
    control_plane_proof_part_of_truth_carrier: bool,
    closure_bundle_embedded_here: bool,
    #[serde(default)]
    closure_bundle_bound_by_digest: bool,
    closure_bundle_issue_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CharterInput {
    report_id: String,
    report_digest: String,
    machine_identity_binding: CommonMachineIdentityInput,
    charter_green: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
    #[serde(default)]
    deferred_issue_ids: Vec<String>,
    internal_only_plugin_posture: bool,
    host_executes_but_does_not_decide: bool,
    proof_vs_audit_distinction_frozen: bool,
    three_plane_contract_frozen: bool,
    anti_interpreter_smuggling_frozen: bool,
    downward_non_influence_frozen: bool,
    observer_model_frozen: bool,
    state_class_split_frozen: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct PacketAbiInput {
    #[serde(flatten)]
    common: CommonContractInput,
    packet_abi_frozen: bool,
    rust_first_pdk_frozen: bool,
    typed_refusal_channel_frozen: bool,
    explicit_receipt_channel_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct RuntimeApiInput {
    #[serde(flatten)]
    common: CommonContractInput,
    runtime_api_frozen: bool,
    engine_abstraction_frozen: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct InvocationReceiptInput {
    #[serde(flatten)]
    common: CommonContractInput,
    receipt_identity_frozen: bool,
    failure_lattice_frozen: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct AdmissibilityInput {
    #[serde(flatten)]
    common: CommonContractInput,
    admissibility_frozen: bool,
    equivalent_choice_model_frozen: bool,
    world_mount_envelope_compiler_frozen: bool,
    receipt_visible_filtering_required: bool,
    trust_posture_binding_required: bool,
    publication_posture_binding_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ConformanceEvalInput {
    #[serde(flatten)]
    common: CommonContractInput,
    conformance_sandbox_green: bool,
    receipt_integrity_and_envelope_compatibility_explicit: bool,
    queue_saturation_explicit: bool,
    cancellation_latency_bounded: bool,
    timeout_enforcement_measured: bool,
    evidence_overhead_explicit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ResultBindingInput {
    #[serde(flatten)]
    common: CommonContractInput,
    result_binding_contract_green: bool,
    semantic_composition_closure_green: bool,
    proof_vs_observational_boundary_explicit: bool,
    schema_evolution_fail_closed: bool,
    version_skew_fail_closed: bool,
    ambiguous_composition_blocked: bool,
    adapter_defined_return_path_blocked: bool,
    non_lossy_schema_transition_required: bool,
    explicit_output_to_state_digest_binding: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ControllerEvalInput {
    #[serde(flatten)]
    common: CommonContractInput,
    control_trace_contract_green: bool,
    host_not_planner_green: bool,
    typed_refusal_loop_closed: bool,
    determinism_profile_explicit: bool,
    adversarial_negative_rows_green: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct AuthorityGateInput {
    #[serde(flatten)]
    common: CommonContractInput,
    trust_tier_gate_green: bool,
    promotion_receipts_explicit: bool,
    publication_posture_explicit: bool,
    observer_rights_explicit: bool,
    validator_hooks_explicit: bool,
    accepted_outcome_hooks_explicit: bool,
    profile_specific_named_routes_explicit: bool,
    broader_publication_refused: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CanonicalMachineClosureBundleInput {
    report_id: String,
    report_digest: String,
    closure_bundle_digest: String,
    bundle_green: bool,
    closure_subject: CanonicalMachineClosureBundleSubjectInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CanonicalMachineClosureBundleSubjectInput {
    machine_identity_id: String,
    canonical_route_id: String,
}

pub fn build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report(
) -> Result<
    TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReport,
    TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError,
> {
    let turing_closeout: TuringCloseoutInput =
        read_repo_json(TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL)?;
    let charter: CharterInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF_LOCAL,
    )?;
    let manifest: CommonContractInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF_LOCAL,
    )?;
    let packet_abi: PacketAbiInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_REPORT_REF_LOCAL,
    )?;
    let runtime_api: RuntimeApiInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF_LOCAL,
    )?;
    let invocation_receipts: InvocationReceiptInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL,
    )?;
    let admissibility: AdmissibilityInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF_LOCAL,
    )?;
    let conformance: ConformanceEvalInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
    )?;
    let result_binding: ResultBindingInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_REPORT_REF_LOCAL,
    )?;
    let controller: ControllerEvalInput = read_repo_json(
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
    )?;
    let authority: AuthorityGateInput = read_repo_json(
        TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
    )?;
    let closure_bundle: CanonicalMachineClosureBundleInput = read_repo_json(
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL,
    )?;

    let machine_identity_binding = TassadarPostArticleBoundedWeightedPluginPlatformMachineIdentityBinding {
        machine_identity_id: authority
            .common
            .machine_identity_binding
            .machine_identity_id
            .clone(),
        canonical_model_id: authority
            .common
            .machine_identity_binding
            .canonical_model_id
            .clone(),
        canonical_weight_bundle_digest: authority
            .common
            .machine_identity_binding
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: authority
            .common
            .machine_identity_binding
            .canonical_weight_primary_artifact_sha256
            .clone(),
        canonical_route_id: authority
            .common
            .machine_identity_binding
            .canonical_route_id
            .clone(),
        canonical_route_descriptor_digest: authority
            .common
            .machine_identity_binding
            .canonical_route_descriptor_digest
            .clone(),
        continuation_contract_id: authority
            .common
            .machine_identity_binding
            .continuation_contract_id
            .clone(),
        continuation_contract_digest: authority
            .common
            .machine_identity_binding
            .continuation_contract_digest
            .clone(),
        computational_model_statement_id: authority
            .common
            .machine_identity_binding
            .computational_model_statement_id
            .clone()
            .unwrap_or_default(),
        control_plane_proof_report_id: charter
            .machine_identity_binding
            .control_plane_proof_report_id
            .clone()
            .unwrap_or_default(),
        control_plane_proof_report_digest: charter
            .machine_identity_binding
            .control_plane_proof_report_digest
            .clone()
            .unwrap_or_default(),
        control_trace_contract_id: authority
            .common
            .machine_identity_binding
            .control_trace_contract_id
            .clone()
            .unwrap_or_default(),
        controller_eval_report_id: controller.common.report_id.clone(),
        controller_eval_report_digest: controller.common.report_digest.clone(),
        authority_gate_report_id: authority.common.report_id.clone(),
        authority_gate_report_digest: authority.common.report_digest.clone(),
        turing_closeout_report_id: turing_closeout.report_id.clone(),
        turing_closeout_report_digest: turing_closeout.report_digest.clone(),
        closure_bundle_report_id: authority
            .common
            .machine_identity_binding
            .closure_bundle_report_id
            .clone()
            .unwrap_or_else(|| closure_bundle.report_id.clone()),
        closure_bundle_report_digest: authority
            .common
            .machine_identity_binding
            .closure_bundle_report_digest
            .clone()
            .unwrap_or_else(|| closure_bundle.report_digest.clone()),
        closure_bundle_digest: authority
            .common
            .machine_identity_binding
            .closure_bundle_digest
            .clone()
            .unwrap_or_else(|| closure_bundle.closure_bundle_digest.clone()),
        canonical_architecture_anchor_crate: charter
            .machine_identity_binding
            .canonical_architecture_anchor_crate
            .clone()
            .unwrap_or_default(),
        canonical_architecture_boundary_ref: charter
            .machine_identity_binding
            .canonical_architecture_boundary_ref
            .clone()
            .unwrap_or_default(),
        detail: format!(
            "machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` computational_model_statement_id=`{}` control_plane_proof_report_id=`{}` control_trace_contract_id=`{}` authority_gate_report_id=`{}` turing_closeout_report_id=`{}` and closure_bundle_digest=`{}` stay bound to `{}`.",
            authority.common.machine_identity_binding.machine_identity_id,
            authority.common.machine_identity_binding.canonical_model_id,
            authority.common.machine_identity_binding.canonical_route_id,
            authority
                .common
                .machine_identity_binding
                .computational_model_statement_id
                .clone()
                .unwrap_or_default(),
            charter
                .machine_identity_binding
                .control_plane_proof_report_id
                .clone()
                .unwrap_or_default(),
            authority
                .common
                .machine_identity_binding
                .control_trace_contract_id
                .clone()
                .unwrap_or_default(),
            authority.common.report_id,
            turing_closeout.report_id,
            authority
                .common
                .machine_identity_binding
                .closure_bundle_digest
                .clone()
                .unwrap_or_else(|| closure_bundle.closure_bundle_digest.clone()),
            charter
                .machine_identity_binding
                .canonical_architecture_anchor_crate
                .clone()
                .unwrap_or_else(|| String::from("psionic-transformer")),
        ),
    };
    let closure_bundle_bound_by_digest = closure_bundle.bundle_green
        && turing_closeout.closure_bundle_bound_by_digest
        && controller.common.closure_bundle_bound_by_digest
        && authority.common.closure_bundle_bound_by_digest
        && closure_bundle.closure_subject.machine_identity_id
            == machine_identity_binding.machine_identity_id
        && closure_bundle.closure_subject.canonical_route_id
            == machine_identity_binding.canonical_route_id
        && machine_identity_binding.closure_bundle_report_id == closure_bundle.report_id
        && machine_identity_binding.closure_bundle_report_digest == closure_bundle.report_digest
        && machine_identity_binding.closure_bundle_digest == closure_bundle.closure_bundle_digest;

    let operator_internal_only_posture = charter.internal_only_plugin_posture
        && packet_abi.common.operator_internal_only_posture
        && runtime_api.common.operator_internal_only_posture
        && invocation_receipts.common.operator_internal_only_posture
        && admissibility.common.operator_internal_only_posture
        && conformance.common.operator_internal_only_posture
        && result_binding.common.operator_internal_only_posture
        && controller.common.operator_internal_only_posture
        && authority.common.operator_internal_only_posture;
    let served_plugin_envelope_published = authority.common.plugin_publication_allowed;
    let weighted_plugin_control_allowed = controller.common.weighted_plugin_control_allowed
        && authority.common.weighted_plugin_control_allowed
        && controller.control_trace_contract_green
        && authority.trust_tier_gate_green;
    let rebase_claim_allowed = turing_closeout.rebase_claim_allowed
        && charter.rebase_claim_allowed
        && manifest.rebase_claim_allowed
        && packet_abi.common.rebase_claim_allowed
        && runtime_api.common.rebase_claim_allowed
        && invocation_receipts.common.rebase_claim_allowed
        && admissibility.common.rebase_claim_allowed
        && conformance.common.rebase_claim_allowed
        && result_binding.common.rebase_claim_allowed
        && controller.common.rebase_claim_allowed
        && authority.common.rebase_claim_allowed;

    let supporting_material_rows = vec![
        supporting_material_row(
            "post_article_turing_closeout",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::ProofCarrying,
            turing_closeout.closeout_green,
            TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
            Some(turing_closeout.report_id.clone()),
            Some(turing_closeout.report_digest.clone()),
            "the bounded plugin-platform closeout inherits the post-article bounded Turing-completeness closeout as its lower-plane proof-bearing compute and control carrier instead of recomputing that claim locally.",
        ),
        supporting_material_row(
            "canonical_machine_closure_bundle",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::ProofCarrying,
            closure_bundle.bundle_green,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL,
            Some(closure_bundle.report_id.clone()),
            Some(closure_bundle.report_digest.clone()),
            "the bounded plugin-platform closeout inherits the canonical machine closure bundle by digest instead of reconstructing machine identity from the lower-plane proof and plugin surfaces alone.",
        ),
        supporting_material_row(
            "plugin_charter_authority_boundary",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::ProofCarrying,
            charter.charter_green,
            TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF_LOCAL,
            Some(charter.report_id.clone()),
            Some(charter.report_digest.clone()),
            "the plugin charter binds the capability lane to the canonical rebased machine identity, computational-model statement, and inherited control-plane proof before any later platform closeout may turn green.",
        ),
        supporting_material_row(
            "plugin_manifest_identity_contract",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::Contract,
            manifest.contract_green,
            TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF_LOCAL,
            Some(manifest.report_id.clone()),
            Some(manifest.report_digest.clone()),
            "manifest identity keeps plugin artifact identity, export declarations, and hot-swap boundaries machine-legible so the platform closeout does not float above unnamed plugin objects.",
        ),
        supporting_material_row(
            "plugin_packet_abi_and_rust_pdk",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::Contract,
            packet_abi.common.contract_green,
            TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_REPORT_REF_LOCAL,
            Some(packet_abi.common.report_id.clone()),
            Some(packet_abi.common.report_digest.clone()),
            "the packet ABI and Rust-first PDK freeze the invocation contract and typed-refusal channel instead of letting the platform closeout imply a broader or tool-shaped guest surface.",
        ),
        supporting_material_row(
            "plugin_runtime_api_and_engine_abstraction",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::Contract,
            runtime_api.common.contract_green,
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF_LOCAL,
            Some(runtime_api.common.report_id.clone()),
            Some(runtime_api.common.report_digest.clone()),
            "the runtime API keeps host execution and engine abstraction explicit so the platform closeout remains above a declared host-owned execution layer rather than silently absorbing it.",
        ),
        supporting_material_row(
            "plugin_invocation_receipts_and_replay_classes",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::Contract,
            invocation_receipts.common.contract_green,
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL,
            Some(invocation_receipts.common.report_id.clone()),
            Some(invocation_receipts.common.report_digest.clone()),
            "invocation receipts keep install/export/packet/replay identity machine-readable so later publication or benchmarking claims cannot erase what actually executed.",
        ),
        supporting_material_row(
            "plugin_world_mount_envelope_and_admissibility",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::Contract,
            admissibility.common.contract_green,
            TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF_LOCAL,
            Some(admissibility.common.report_id.clone()),
            Some(admissibility.common.report_digest.clone()),
            "admissibility keeps candidate sets, equivalent-choice classes, route binding, trust posture, and publication posture explicit so later controller behavior does not smuggle hidden choice or mount power.",
        ),
        supporting_material_row(
            "plugin_conformance_sandbox_and_benchmark_harness",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::Contract,
            conformance.common.contract_green,
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
            Some(conformance.common.report_id.clone()),
            Some(conformance.common.report_digest.clone()),
            "the conformance harness keeps the sandbox evidence and benchmark posture machine-readable so the closeout does not lean on unbounded or hidden operator experience.",
        ),
        supporting_material_row(
            "plugin_result_binding_schema_stability_and_composition",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::Contract,
            result_binding.common.contract_green,
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_REPORT_REF_LOCAL,
            Some(result_binding.common.report_id.clone()),
            Some(result_binding.common.report_digest.clone()),
            "result binding keeps schema stability, typed refusal normalization, and proof-versus-observational result boundaries explicit before weighted controller ownership turns into platform closure.",
        ),
        supporting_material_row(
            "weighted_plugin_controller_trace",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::ProofCarrying,
            controller.common.contract_green,
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
            Some(controller.common.report_id.clone()),
            Some(controller.common.report_digest.clone()),
            "the weighted controller trace is the plugin-specific proof-bearing extension above the inherited control-plane proof; the platform closeout must cite that extension explicitly.",
        ),
        supporting_material_row(
            "plugin_authority_promotion_publication_and_trust_tier_gate",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::Contract,
            authority.common.contract_green,
            TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
            Some(authority.common.report_id.clone()),
            Some(authority.common.report_digest.clone()),
            "the authority gate freezes trust tiers, publication posture, validator hooks, and challengeability so the closeout can say exactly what platform capability does and does not imply.",
        ),
        supporting_material_row(
            "post_article_turing_audit_context",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::ObservationalContext,
            true,
            POST_ARTICLE_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 post-article audit remains observational context that explains why the platform closeout stays separated from the lower-plane Turing closeout and the later machine closure bundle.",
        ),
        supporting_material_row(
            "plugin_system_turing_audit_context",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::ObservationalContext,
            true,
            PLUGIN_SYSTEM_TURING_AUDIT_REF,
            None,
            None,
            "the March 20 plugin-system audit remains observational context that names the bounded plugin-platform closeout without collapsing it into public publication or arbitrary software claims.",
        ),
        supporting_material_row(
            "local_plugin_system_spec",
            TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass::ObservationalContext,
            true,
            LOCAL_PLUGIN_SYSTEM_SPEC_REF,
            None,
            None,
            "the local plugin-system notes remain design input only; they do not substitute for the proof-carrying contracts or the machine-readable closeout itself.",
        ),
    ];

    let dependency_rows = vec![
        dependency_row(
            "bounded_turing_closeout_foundation",
            turing_closeout.closeout_green
                && turing_closeout.canonical_route_truth_carrier
                && turing_closeout.control_plane_proof_part_of_truth_carrier
                && !turing_closeout.closure_bundle_embedded_here
                && turing_closeout.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID,
            vec![String::from(
                TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
            )],
            "the plugin-platform closeout sits above the rebased Turing-completeness closeout and inherits its canonical-route plus control-plane truth carrier without treating this report as the final machine bundle.",
        ),
        dependency_row(
            "canonical_machine_closure_bundle_published",
            closure_bundle_bound_by_digest,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL,
            )],
            "the bounded plugin-platform claim must inherit the canonical machine closure bundle by report id and digest instead of recomposing the machine from lower-plane proofs and plugin contracts.",
        ),
        dependency_row(
            "plugin_charter_boundary",
            charter.charter_green
                && charter.internal_only_plugin_posture
                && charter.host_executes_but_does_not_decide
                && charter.proof_vs_audit_distinction_frozen
                && charter.three_plane_contract_frozen
                && charter.anti_interpreter_smuggling_frozen
                && charter.downward_non_influence_frozen
                && charter.observer_model_frozen
                && charter.state_class_split_frozen
                && charter.deferred_issue_ids.is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF_LOCAL,
            )],
            "the charter must already freeze the plugin lane, state classes, host-executes-but-may-not-decide posture, and proof-versus-audit distinction before a platform closeout can turn green.",
        ),
        dependency_row(
            "plugin_manifest_identity_contract",
            manifest.contract_green && manifest.deferred_issue_ids.is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF_LOCAL,
            )],
            "manifest identity must already be green so the platform closeout binds named plugin artifacts rather than a floating software notion.",
        ),
        dependency_row(
            "plugin_packet_abi_and_rust_pdk",
            packet_abi.common.contract_green
                && packet_abi.packet_abi_frozen
                && packet_abi.rust_first_pdk_frozen
                && packet_abi.typed_refusal_channel_frozen
                && packet_abi.explicit_receipt_channel_required
                && packet_abi.common.deferred_issue_ids.is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_REPORT_REF_LOCAL,
            )],
            "the platform closeout requires the narrow packet ABI, the Rust-first guest lane, the typed-refusal channel, and explicit receipt binding to already be frozen.",
        ),
        dependency_row(
            "plugin_runtime_api_and_engine_abstraction",
            runtime_api.common.contract_green
                && runtime_api.runtime_api_frozen
                && runtime_api.engine_abstraction_frozen
                && runtime_api.common.deferred_issue_ids.is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF_LOCAL,
            )],
            "the host-owned runtime API and engine abstraction must already be explicit so the closeout does not collapse host execution into model-owned control.",
        ),
        dependency_row(
            "plugin_invocation_receipts_and_replay",
            invocation_receipts.common.contract_green
                && invocation_receipts.receipt_identity_frozen
                && invocation_receipts.failure_lattice_frozen
                && invocation_receipts.common.deferred_issue_ids.is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL,
            )],
            "the platform closeout requires receipt identity and the typed failure lattice to already be frozen above the runtime API.",
        ),
        dependency_row(
            "plugin_world_mount_envelope_and_admissibility",
            admissibility.common.contract_green
                && admissibility.admissibility_frozen
                && admissibility.equivalent_choice_model_frozen
                && admissibility.world_mount_envelope_compiler_frozen
                && admissibility.receipt_visible_filtering_required
                && admissibility.trust_posture_binding_required
                && admissibility.publication_posture_binding_required
                && admissibility.common.deferred_issue_ids.is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF_LOCAL,
            )],
            "the platform closeout requires admissibility, equivalent-choice classes, mount envelopes, and receipt-visible posture filtering to already be explicit and fail-closed.",
        ),
        dependency_row(
            "plugin_conformance_sandbox_and_benchmark_harness",
            conformance.common.contract_green
                && conformance.conformance_sandbox_green
                && conformance.receipt_integrity_and_envelope_compatibility_explicit
                && conformance.queue_saturation_explicit
                && conformance.cancellation_latency_bounded
                && conformance.timeout_enforcement_measured
                && conformance.evidence_overhead_explicit
                && conformance.common.deferred_issue_ids.is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
            )],
            "the platform closeout requires conformance, benchmark posture, cancellation, timeout, and envelope integrity evidence to already be machine-readable.",
        ),
        dependency_row(
            "plugin_result_binding_schema_stability_and_composition",
            result_binding.common.contract_green
                && result_binding.result_binding_contract_green
                && result_binding.semantic_composition_closure_green
                && result_binding.proof_vs_observational_boundary_explicit
                && result_binding.schema_evolution_fail_closed
                && result_binding.version_skew_fail_closed
                && result_binding.ambiguous_composition_blocked
                && result_binding.adapter_defined_return_path_blocked
                && result_binding.non_lossy_schema_transition_required
                && result_binding.explicit_output_to_state_digest_binding
                && result_binding.common.deferred_issue_ids.is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_REPORT_REF_LOCAL,
            )],
            "the platform closeout requires result binding, schema stability, semantic composition, and proof-versus-observational result boundaries to already be frozen.",
        ),
        dependency_row(
            "weighted_plugin_controller_trace",
            controller.common.contract_green
                && controller.control_trace_contract_green
                && controller.host_not_planner_green
                && controller.typed_refusal_loop_closed
                && controller.determinism_profile_explicit
                && controller.adversarial_negative_rows_green
                && controller.common.deferred_issue_ids.is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
            )],
            "the bounded platform closeout depends on the inherited control-plane proof plus the plugin-specific weighted-controller extension instead of implying controller ownership from lower-plane proofs alone.",
        ),
        dependency_row(
            "plugin_authority_promotion_publication_and_trust_tier_gate",
            authority.common.contract_green
                && authority.trust_tier_gate_green
                && authority.promotion_receipts_explicit
                && authority.publication_posture_explicit
                && authority.observer_rights_explicit
                && authority.validator_hooks_explicit
                && authority.accepted_outcome_hooks_explicit
                && authority.profile_specific_named_routes_explicit
                && authority.broader_publication_refused
                && authority.common.deferred_issue_ids.is_empty(),
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
            )],
            "the platform closeout requires the authority, promotion, trust-tier, and publication posture gate to already be green so bounded capability does not imply broader publication or public platform rights.",
        ),
    ];

    let core_machine_tuple_locked = same_core_machine_tuple(
        &charter.machine_identity_binding,
        &turing_closeout.machine_identity_binding,
    ) && same_core_machine_tuple(
        &charter.machine_identity_binding,
        &manifest.machine_identity_binding,
    ) && same_core_machine_tuple(
        &charter.machine_identity_binding,
        &packet_abi.common.machine_identity_binding,
    ) && same_core_machine_tuple(
        &charter.machine_identity_binding,
        &runtime_api.common.machine_identity_binding,
    ) && same_core_machine_tuple(
        &charter.machine_identity_binding,
        &invocation_receipts.common.machine_identity_binding,
    ) && same_core_machine_tuple(
        &charter.machine_identity_binding,
        &admissibility.common.machine_identity_binding,
    ) && same_core_machine_tuple(
        &charter.machine_identity_binding,
        &conformance.common.machine_identity_binding,
    ) && same_core_machine_tuple(
        &charter.machine_identity_binding,
        &result_binding.common.machine_identity_binding,
    ) && same_core_machine_tuple(
        &charter.machine_identity_binding,
        &controller.common.machine_identity_binding,
    ) && same_core_machine_tuple(
        &charter.machine_identity_binding,
        &authority.common.machine_identity_binding,
    );
    let capability_machine_tuple_locked = same_capability_machine_tuple(
        &charter.machine_identity_binding,
        &manifest.machine_identity_binding,
    ) && same_capability_machine_tuple(
        &charter.machine_identity_binding,
        &packet_abi.common.machine_identity_binding,
    ) && same_capability_machine_tuple(
        &charter.machine_identity_binding,
        &runtime_api.common.machine_identity_binding,
    ) && same_capability_machine_tuple(
        &charter.machine_identity_binding,
        &invocation_receipts.common.machine_identity_binding,
    ) && same_capability_machine_tuple(
        &charter.machine_identity_binding,
        &admissibility.common.machine_identity_binding,
    ) && same_capability_machine_tuple(
        &charter.machine_identity_binding,
        &conformance.common.machine_identity_binding,
    ) && same_capability_machine_tuple(
        &charter.machine_identity_binding,
        &result_binding.common.machine_identity_binding,
    ) && same_capability_machine_tuple(
        &charter.machine_identity_binding,
        &controller.common.machine_identity_binding,
    ) && same_capability_machine_tuple(
        &charter.machine_identity_binding,
        &authority.common.machine_identity_binding,
    );

    let validation_rows = vec![
        validation_row(
            "canonical_machine_tuple_locked",
            core_machine_tuple_locked && capability_machine_tuple_locked,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
                ),
            ],
            "all proof-bearing and capability-bearing artifacts stay bound to one canonical machine tuple over model id, weight digest, route digest, continuation contract, and computational-model statement instead of silently recomposing different effective machines.",
        ),
        validation_row(
            "inherited_control_plane_plus_plugin_extension_explicit",
            turing_closeout.control_plane_proof_part_of_truth_carrier
                && charter
                    .machine_identity_binding
                    .control_plane_proof_report_id
                    .is_some()
                && controller.control_trace_contract_green
                && weighted_plugin_control_allowed,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
                ),
            ],
            "weighted plugin control remains explicitly inherited from the lower-plane control proof plus the plugin-specific controller extension instead of floating as an adjacent benchmark or runtime observation.",
        ),
        validation_row(
            "hidden_host_orchestration_quarantined",
            charter.host_executes_but_does_not_decide
                && controller.host_not_planner_green
                && controller.adversarial_negative_rows_green,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
                ),
            ],
            "the host remains execution-only, the controller report keeps the host out of the planner role, and adversarial host-negative rows stay green instead of letting hidden orchestration backfill the platform claim.",
        ),
        validation_row(
            "schema_drift_and_adapter_substitution_refused",
            result_binding.schema_evolution_fail_closed
                && result_binding.version_skew_fail_closed
                && result_binding.ambiguous_composition_blocked
                && result_binding.adapter_defined_return_path_blocked
                && result_binding.non_lossy_schema_transition_required,
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_REPORT_REF_LOCAL,
            )],
            "schema drift, version skew, ambiguous composition, and adapter-defined return paths stay fail-closed so platform closure cannot hide workflow logic in result adaptation.",
        ),
        validation_row(
            "envelope_leakage_and_receipt_filtering_refused",
            admissibility.world_mount_envelope_compiler_frozen
                && admissibility.receipt_visible_filtering_required
                && admissibility.trust_posture_binding_required
                && admissibility.publication_posture_binding_required
                && conformance.receipt_integrity_and_envelope_compatibility_explicit,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
                ),
            ],
            "mount envelopes, receipt-visible filtering, trust posture, publication posture, and sandboxed envelope compatibility stay explicit so the platform closeout does not leak hidden capability or routing power.",
        ),
        validation_row(
            "side_channel_and_benchmark_posture_explicit",
            controller.determinism_profile_explicit
                && conformance.queue_saturation_explicit
                && conformance.cancellation_latency_bounded
                && conformance.timeout_enforcement_measured
                && conformance.evidence_overhead_explicit,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
                ),
            ],
            "determinism, queue saturation, cancellation latency, timeout enforcement, and evidence overhead all stay explicit so timing and soft-failure behavior do not become hidden control channels.",
        ),
        validation_row(
            "proof_vs_audit_classification_preserved",
            charter.proof_vs_audit_distinction_frozen
                && result_binding.proof_vs_observational_boundary_explicit
                && !turing_closeout.closure_bundle_embedded_here,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
                ),
            ],
            "proof-carrying surfaces and observational audits remain explicitly classified so the platform closeout does not widen the claim by conflating sampled audits with proof-bearing contracts.",
        ),
        validation_row(
            "operator_and_served_posture_explicit",
            operator_internal_only_posture
                && authority.publication_posture_explicit
                && authority.broader_publication_refused
                && !authority.common.plugin_publication_allowed
                && !authority.common.served_public_universality_allowed,
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
            )],
            "the closeout stays operator/internal only, keeps served or public plugin publication suppressed, and does not imply that broader served posture already shares operator conformance.",
        ),
        validation_row(
            "arbitrary_public_wasm_and_tool_use_refused",
            !packet_abi.common.arbitrary_software_capability_allowed
                && !runtime_api.common.arbitrary_software_capability_allowed
                && !invocation_receipts.common.arbitrary_software_capability_allowed
                && !admissibility.common.arbitrary_software_capability_allowed
                && !conformance.common.arbitrary_software_capability_allowed
                && !result_binding.common.arbitrary_software_capability_allowed
                && !controller.common.arbitrary_software_capability_allowed
                && !authority.common.arbitrary_software_capability_allowed,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF_LOCAL,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
                ),
            ],
            "bounded weighted plugin capability does not imply arbitrary public Wasm, arbitrary public tools, or arbitrary software capability; those claims remain explicitly false.",
        ),
        validation_row(
            "closure_bundle_stays_separate",
            !turing_closeout.closure_bundle_embedded_here
                && turing_closeout.closure_bundle_issue_id == CLOSURE_BUNDLE_ISSUE_ID,
            vec![String::from(
                TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
            )],
            "the final claim-bearing canonical machine closure bundle stays separate from this platform closeout even though the platform claim now inherits it by digest.",
        ),
        validation_row(
            "closure_bundle_bound_by_digest",
            closure_bundle_bound_by_digest,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL,
            )],
            "the bounded plugin-platform claim now inherits the canonical machine closure bundle by digest instead of relying on adjacent machine fields only.",
        ),
    ];

    let closeout_green = dependency_rows.iter().all(|row| row.satisfied)
        && validation_rows.iter().all(|row| row.green)
        && operator_internal_only_posture
        && closure_bundle_bound_by_digest
        && !served_plugin_envelope_published;
    let closeout_status = if closeout_green {
        TassadarPostArticleBoundedWeightedPluginPlatformCloseoutStatus::OperatorGreenServedSuppressed
    } else {
        TassadarPostArticleBoundedWeightedPluginPlatformCloseoutStatus::Incomplete
    };
    let plugin_capability_claim_allowed = closeout_green;
    let plugin_publication_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let mut report = TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_bounded_weighted_plugin_platform_closeout_audit.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_CHECKER_REF,
        ),
        turing_closeout_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF_LOCAL,
        ),
        plugin_charter_authority_boundary_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CHARTER_AUTHORITY_BOUNDARY_REPORT_REF_LOCAL,
        ),
        plugin_manifest_identity_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_MANIFEST_IDENTITY_CONTRACT_REPORT_REF_LOCAL,
        ),
        plugin_packet_abi_and_rust_pdk_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_PACKET_ABI_AND_RUST_PDK_REPORT_REF_LOCAL,
        ),
        plugin_runtime_api_and_engine_abstraction_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RUNTIME_API_AND_ENGINE_ABSTRACTION_REPORT_REF_LOCAL,
        ),
        plugin_invocation_receipts_and_replay_classes_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPTS_AND_REPLAY_CLASSES_REPORT_REF_LOCAL,
        ),
        plugin_world_mount_envelope_compiler_and_admissibility_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_WORLD_MOUNT_ENVELOPE_COMPILER_AND_ADMISSIBILITY_REPORT_REF_LOCAL,
        ),
        plugin_conformance_sandbox_and_benchmark_harness_eval_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CONFORMANCE_SANDBOX_AND_BENCHMARK_HARNESS_EVAL_REPORT_REF_LOCAL,
        ),
        plugin_result_binding_schema_stability_and_composition_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_REPORT_REF_LOCAL,
        ),
        weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report_ref:
            String::from(
                TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_AND_REFUSAL_AWARE_MODEL_LOOP_EVAL_REPORT_REF_LOCAL,
            ),
        plugin_authority_promotion_publication_and_trust_tier_gate_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_AUTHORITY_PROMOTION_PUBLICATION_AND_TRUST_TIER_GATE_REPORT_REF_LOCAL,
        ),
        canonical_machine_closure_bundle_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL,
        ),
        post_article_turing_audit_ref: String::from(POST_ARTICLE_TURING_AUDIT_REF),
        plugin_system_turing_audit_ref: String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
        local_plugin_system_spec_ref: String::from(LOCAL_PLUGIN_SYSTEM_SPEC_REF),
        supporting_material_rows,
        machine_identity_binding,
        dependency_rows,
        validation_rows,
        closeout_status,
        closeout_green,
        operator_internal_only_posture,
        served_plugin_envelope_published,
        closure_bundle_embedded_here: false,
        closure_bundle_bound_by_digest,
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        claim_boundary: String::from(
            "this closeout audit states the first honest bounded weighted plugin-platform claim above the canonical post-`TAS-186` machine. It binds the lower-plane Turing closeout, plugin charter, manifest, ABI, runtime API, invocation receipts, admissibility envelope compiler, conformance harness, result-binding contract, weighted controller trace, and authority or promotion or publication gate into one operator/internal-only platform statement while inheriting the canonical machine closure bundle by report id and digest. It still keeps the closure bundle as a separate artifact, keeps served or public plugin publication suppressed, refuses any implication of arbitrary public Wasm or arbitrary public tool use, and does not imply served/public universality or arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article bounded weighted plugin-platform closeout audit keeps supporting_materials={}/{}, dependency_rows={}/{}, validation_rows={}/{}, closeout_status={:?}, closure_bundle_digest=`{}`, closure_bundle_issue_id=`{}`, and plugin_capability_claim_allowed={}.",
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
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.closeout_status,
        report.machine_identity_binding.closure_bundle_digest,
        report.closure_bundle_issue_id,
        report.plugin_capability_claim_allowed,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report|",
        &report,
    );
    Ok(report)
}

#[must_use]
pub fn tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF,
    )
}

pub fn write_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReport,
    TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError::Write {
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
    material_class: TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialRow {
    TassadarPostArticleBoundedWeightedPluginPlatformSupportingMaterialRow {
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
) -> TassadarPostArticleBoundedWeightedPluginPlatformDependencyRow {
    TassadarPostArticleBoundedWeightedPluginPlatformDependencyRow {
        dependency_id: String::from(dependency_id),
        satisfied,
        source_refs,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleBoundedWeightedPluginPlatformValidationRow {
    TassadarPostArticleBoundedWeightedPluginPlatformValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn same_core_machine_tuple(
    base: &CommonMachineIdentityInput,
    candidate: &CommonMachineIdentityInput,
) -> bool {
    base.machine_identity_id == candidate.machine_identity_id
        && base.canonical_model_id == candidate.canonical_model_id
        && base.canonical_weight_bundle_digest == candidate.canonical_weight_bundle_digest
        && base.canonical_weight_primary_artifact_sha256
            == candidate.canonical_weight_primary_artifact_sha256
        && base.canonical_route_id == candidate.canonical_route_id
        && base.canonical_route_descriptor_digest == candidate.canonical_route_descriptor_digest
        && base.continuation_contract_id == candidate.continuation_contract_id
        && base.continuation_contract_digest == candidate.continuation_contract_digest
}

fn same_capability_machine_tuple(
    base: &CommonMachineIdentityInput,
    candidate: &CommonMachineIdentityInput,
) -> bool {
    same_core_machine_tuple(base, candidate)
        && base.computational_model_statement_id == candidate.computational_model_statement_id
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_committed_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report,
        read_committed_json,
        tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report_path,
        write_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report,
        TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReport,
        TassadarPostArticleBoundedWeightedPluginPlatformCloseoutStatus,
        TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn post_article_bounded_weighted_plugin_platform_closeout_turns_green_when_prereqs_hold() {
        let report =
            build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report()
                .expect("report");

        assert_eq!(
            report.closeout_status,
            TassadarPostArticleBoundedWeightedPluginPlatformCloseoutStatus::OperatorGreenServedSuppressed
        );
        assert_eq!(report.supporting_material_rows.len(), 14);
        assert_eq!(report.dependency_rows.len(), 12);
        assert_eq!(report.validation_rows.len(), 10);
        assert!(report.closeout_green);
        assert!(report.operator_internal_only_posture);
        assert!(!report.served_plugin_envelope_published);
        assert!(report.closure_bundle_bound_by_digest);
        assert!(!report.closure_bundle_embedded_here);
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
        assert!(report.rebase_claim_allowed);
        assert!(report.plugin_capability_claim_allowed);
        assert!(report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_bounded_weighted_plugin_platform_closeout_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report()
                .expect("report");
        let committed: TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReport =
            read_committed_json(
                tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report_path(),
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report.json")
        );
        assert_eq!(
            TASSADAR_POST_ARTICLE_BOUNDED_WEIGHTED_PLUGIN_PLATFORM_CLOSEOUT_AUDIT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report.json"
        );
    }

    #[test]
    fn write_post_article_bounded_weighted_plugin_platform_closeout_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report.json",
        );
        let written =
            write_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report(
                &output_path,
            )
            .expect("write report");
        let persisted: TassadarPostArticleBoundedWeightedPluginPlatformCloseoutAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
