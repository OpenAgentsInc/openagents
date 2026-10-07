//! OpenAgents provider-facing types for Psionic.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

mod tassadar_accepted_outcome_binding;
mod tassadar_article_cross_machine_reproducibility_matrix;
mod tassadar_article_demo_benchmark_equivalence_gate;
mod tassadar_article_equivalence_acceptance_gate;
mod tassadar_article_equivalence_final_audit;
mod tassadar_article_interpreter_ownership_gate;
mod tassadar_article_kv_activation_discipline_audit;
mod tassadar_article_route_minimality_audit;
mod tassadar_async_lifecycle_profile;
mod tassadar_broad_family_specialization;
mod tassadar_broad_general_compute_economic_bridge;
mod tassadar_broad_internal_compute_acceptance_gate;
mod tassadar_broad_internal_compute_portability;
mod tassadar_broad_internal_compute_profile_publication;
mod tassadar_component_linking_profile;
mod tassadar_composite_accepted_outcome_template;
mod tassadar_composite_routing;
mod tassadar_cost_per_correct_job;
mod tassadar_counterfactual_route_quality;
mod tassadar_cross_profile_link_compatibility;
mod tassadar_delegation_benchmark;
mod tassadar_direct_model_weight_execution_proof;
mod tassadar_effect_safe_resume;
mod tassadar_effect_taxonomy;
mod tassadar_effectful_replay_audit;
mod tassadar_effective_unbounded_compute_claim;
mod tassadar_evidence_routing;
mod tassadar_exact_compute_market;
mod tassadar_exception_profile;
mod tassadar_execution_checkpoint;
mod tassadar_execution_unit_registration;
mod tassadar_float_profile_acceptance_gate;
mod tassadar_float_semantics;
mod tassadar_frozen_core_wasm_closure_gate;
mod tassadar_full_core_wasm_public_acceptance_gate;
mod tassadar_general_internal_compute_red_team;
mod tassadar_hybrid_process_controller;
mod tassadar_import_policy_matrix;
mod tassadar_installed_module_evidence;
mod tassadar_installed_process_lifecycle;
mod tassadar_internal_component_abi;
mod tassadar_internal_compute_package_manager;
mod tassadar_linked_program_bundle;
mod tassadar_minimal_universal_substrate_gate;
mod tassadar_module_catalog;
mod tassadar_module_installation;
mod tassadar_module_library;
mod tassadar_module_linker;
mod tassadar_module_manifest;
mod tassadar_module_overlap_resolution;
mod tassadar_module_promotion_state;
mod tassadar_module_trust_isolation;
mod tassadar_multi_memory_profile;
mod tassadar_numeric_portability;
mod tassadar_planner_policy;
mod tassadar_post_article_anti_drift_stability_closeout;
mod tassadar_post_article_bounded_weighted_plugin_platform_closeout;
mod tassadar_post_article_canonical_computational_model_statement;
mod tassadar_post_article_canonical_machine_closure_bundle;
mod tassadar_post_article_canonical_machine_identity_lock;
mod tassadar_post_article_canonical_route_semantic_preservation;
mod tassadar_post_article_canonical_route_universal_substrate_gate;
mod tassadar_post_article_carrier_split_contract;
mod tassadar_post_article_continuation_non_computationality_contract;
mod tassadar_post_article_control_plane_decision_provenance_proof;
mod tassadar_post_article_downward_non_influence_and_served_conformance;
mod tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract;
mod tassadar_post_article_execution_semantics_proof_transport_audit;
mod tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract;
mod tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate;
mod tassadar_post_article_plugin_capability_boundary;
mod tassadar_post_article_plugin_charter_authority_boundary;
mod tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness;
mod tassadar_post_article_plugin_invocation_receipts_and_replay_classes;
mod tassadar_post_article_plugin_manifest_identity_contract;
mod tassadar_post_article_plugin_packet_abi_and_rust_pdk;
mod tassadar_post_article_plugin_result_binding_schema_stability_and_composition;
mod tassadar_post_article_plugin_runtime_api_and_engine_abstraction;
mod tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility;
mod tassadar_post_article_rebased_universality_verdict_split;
mod tassadar_post_article_starter_plugin_catalog;
mod tassadar_post_article_turing_completeness_closeout;
mod tassadar_post_article_universal_machine_proof_rebinding;
mod tassadar_post_article_universality_bridge_contract;
mod tassadar_post_article_universality_portability_minimality_matrix;
mod tassadar_post_article_universality_witness_suite_reissue;
mod tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop;
mod tassadar_pre_closeout_universality;
mod tassadar_preemptive_job_profile;
mod tassadar_process_object_family;
mod tassadar_proposal_profile_ladder_claim_checker;
mod tassadar_quantization_truth_envelope;
mod tassadar_receipt_supervision;
mod tassadar_relaxed_simd_research;
mod tassadar_resumable_multi_slice_promotion;
mod tassadar_self_installation_gate;
mod tassadar_semantic_window_compatibility_delta;
mod tassadar_semantic_window_migration_planner;
mod tassadar_session_process_profile;
mod tassadar_shared_state_concurrency_verdicts;
mod tassadar_simd_profile;
mod tassadar_simulator_effect_profile;
mod tassadar_spill_tape_store_profile;
mod tassadar_subset_profile_promotion_gate;
mod tassadar_threads_research_profile;
mod tassadar_trap_exception;
mod tassadar_turing_completeness_closeout;
mod tassadar_universal_substrate_model;
mod tassadar_universality_verdicts;
mod tassadar_virtual_fs_mount_profile;
mod tassadar_wedge_taxonomy;
mod tassadar_world_mount_compatibility;

use std::collections::BTreeMap;

use ed25519_dalek::SigningKey;
use psionic_ir::{
    TASSADAR_EXCEPTION_PROFILE_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
    TASSADAR_EXCEPTION_PROFILE_TRY_CATCH_RETHROW_ID,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub use tassadar_accepted_outcome_binding::*;
pub use tassadar_article_cross_machine_reproducibility_matrix::*;
pub use tassadar_article_demo_benchmark_equivalence_gate::*;
pub use tassadar_article_equivalence_acceptance_gate::*;
pub use tassadar_article_equivalence_final_audit::*;
pub use tassadar_article_interpreter_ownership_gate::*;
pub use tassadar_article_kv_activation_discipline_audit::*;
pub use tassadar_article_route_minimality_audit::*;
pub use tassadar_async_lifecycle_profile::*;
pub use tassadar_broad_family_specialization::*;
pub use tassadar_broad_general_compute_economic_bridge::*;
pub use tassadar_broad_internal_compute_acceptance_gate::*;
pub use tassadar_broad_internal_compute_portability::*;
pub use tassadar_broad_internal_compute_profile_publication::*;
pub use tassadar_component_linking_profile::*;
pub use tassadar_composite_accepted_outcome_template::*;
pub use tassadar_composite_routing::*;
pub use tassadar_cost_per_correct_job::*;
pub use tassadar_counterfactual_route_quality::*;
pub use tassadar_cross_profile_link_compatibility::*;
pub use tassadar_delegation_benchmark::*;
pub use tassadar_direct_model_weight_execution_proof::*;
pub use tassadar_effect_safe_resume::*;
pub use tassadar_effect_taxonomy::*;
pub use tassadar_effectful_replay_audit::*;
pub use tassadar_effective_unbounded_compute_claim::*;
pub use tassadar_evidence_routing::*;
pub use tassadar_exact_compute_market::*;
pub use tassadar_exception_profile::*;
pub use tassadar_execution_checkpoint::*;
pub use tassadar_execution_unit_registration::*;
pub use tassadar_float_profile_acceptance_gate::*;
pub use tassadar_float_semantics::*;
pub use tassadar_frozen_core_wasm_closure_gate::*;
pub use tassadar_full_core_wasm_public_acceptance_gate::*;
pub use tassadar_general_internal_compute_red_team::*;
pub use tassadar_hybrid_process_controller::*;
pub use tassadar_import_policy_matrix::*;
pub use tassadar_installed_module_evidence::*;
pub use tassadar_installed_process_lifecycle::*;
pub use tassadar_internal_component_abi::*;
pub use tassadar_internal_compute_package_manager::*;
pub use tassadar_linked_program_bundle::*;
pub use tassadar_minimal_universal_substrate_gate::*;
pub use tassadar_module_catalog::*;
pub use tassadar_module_installation::*;
pub use tassadar_module_library::*;
pub use tassadar_module_linker::*;
pub use tassadar_module_manifest::*;
pub use tassadar_module_overlap_resolution::*;
pub use tassadar_module_promotion_state::*;
pub use tassadar_module_trust_isolation::*;
pub use tassadar_multi_memory_profile::*;
pub use tassadar_numeric_portability::*;
pub use tassadar_planner_policy::*;
pub use tassadar_post_article_anti_drift_stability_closeout::*;
pub use tassadar_post_article_bounded_weighted_plugin_platform_closeout::*;
pub use tassadar_post_article_canonical_computational_model_statement::*;
pub use tassadar_post_article_canonical_machine_closure_bundle::*;
pub use tassadar_post_article_canonical_machine_identity_lock::*;
pub use tassadar_post_article_canonical_route_semantic_preservation::*;
pub use tassadar_post_article_canonical_route_universal_substrate_gate::*;
pub use tassadar_post_article_carrier_split_contract::*;
pub use tassadar_post_article_continuation_non_computationality_contract::*;
pub use tassadar_post_article_control_plane_decision_provenance_proof::*;
pub use tassadar_post_article_downward_non_influence_and_served_conformance::*;
pub use tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract::*;
pub use tassadar_post_article_execution_semantics_proof_transport_audit::*;
pub use tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract::*;
pub use tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate::*;
pub use tassadar_post_article_plugin_capability_boundary::*;
pub use tassadar_post_article_plugin_charter_authority_boundary::*;
pub use tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness::*;
pub use tassadar_post_article_plugin_invocation_receipts_and_replay_classes::*;
pub use tassadar_post_article_plugin_manifest_identity_contract::*;
pub use tassadar_post_article_plugin_packet_abi_and_rust_pdk::*;
pub use tassadar_post_article_plugin_result_binding_schema_stability_and_composition::*;
pub use tassadar_post_article_plugin_runtime_api_and_engine_abstraction::*;
pub use tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility::*;
pub use tassadar_post_article_rebased_universality_verdict_split::*;
pub use tassadar_post_article_starter_plugin_catalog::*;
pub use tassadar_post_article_turing_completeness_closeout::*;
pub use tassadar_post_article_universal_machine_proof_rebinding::*;
pub use tassadar_post_article_universality_bridge_contract::*;
pub use tassadar_post_article_universality_portability_minimality_matrix::*;
pub use tassadar_post_article_universality_witness_suite_reissue::*;
pub use tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop::*;
pub use tassadar_pre_closeout_universality::*;
pub use tassadar_preemptive_job_profile::*;
pub use tassadar_process_object_family::*;
pub use tassadar_proposal_profile_ladder_claim_checker::*;
pub use tassadar_quantization_truth_envelope::*;
pub use tassadar_receipt_supervision::*;
pub use tassadar_relaxed_simd_research::*;
pub use tassadar_resumable_multi_slice_promotion::*;
pub use tassadar_self_installation_gate::*;
pub use tassadar_semantic_window_compatibility_delta::*;
pub use tassadar_semantic_window_migration_planner::*;
pub use tassadar_session_process_profile::*;
pub use tassadar_shared_state_concurrency_verdicts::*;
pub use tassadar_simd_profile::*;
pub use tassadar_simulator_effect_profile::*;
pub use tassadar_spill_tape_store_profile::*;
pub use tassadar_subset_profile_promotion_gate::*;
pub use tassadar_threads_research_profile::*;
pub use tassadar_trap_exception::*;
pub use tassadar_turing_completeness_closeout::*;
pub use tassadar_universal_substrate_model::*;
pub use tassadar_universality_verdicts::*;
pub use tassadar_virtual_fs_mount_profile::*;
pub use tassadar_wedge_taxonomy::*;
pub use tassadar_world_mount_compatibility::*;

use psionic_research::{
    TassadarDecompilationArtifactSummary, TassadarKernelModuleScalingSummaryReport,
    TassadarPromotionChecklistGateKind, TassadarPromotionPolicyReport,
    TassadarPromotionPolicyStatus, TassadarWorkloadCapabilityFrontierSummaryReport,
};
use psionic_router::{
    TASSADAR_PLANNER_EXECUTOR_ROUTE_PRODUCT_ID, TassadarPlannerExecutorRouteCandidate,
    TassadarPlannerExecutorRouteDescriptor,
};
use psionic_runtime::{
    AcceleratorDeliverabilityReport, AcceleratorExecutionRequirement, ActivationFingerprintInput,
    ActivationFingerprintProofAdapter, ActivationFingerprintVectorSample, AmdDeviceMetadata,
    AmdRecoveryProfile, AmdRiskProfile, AmdRuntimeMode, AmdTopologyInfo, BackendProbeState,
    BackendSelection, BackendToolchainIdentity, CacheAction, CacheInvalidationPolicy,
    CacheInvalidationTrigger, CacheKind, CacheObservation, ClusterComputeMarketTrustAssessment,
    ClusterEvidenceBundlePayload, ClusterEvidenceBundleStatus, ClusterExecutionCapabilityProfile,
    ClusterExecutionContext, ClusterSettlementProvenanceInput, CompilePathEvidence,
    DeliveredExecutionContext, DeviceInventoryQualifiers, ExecutionCapabilityProfile,
    ExecutionDeliveryProof, ExecutionProofAugmentationPosture, ExecutionProofBundle,
    ExecutionProofBundleKind, ExecutionProofBundleStatus, ExecutionProofRuntimeIdentity,
    ExecutionTopologyPlan, HealthStatus, KvCacheAccounting, KvCacheEncodingAccounting,
    KvCacheEncodingPolicy, KvCachePolicy, KvResidencyAccounting, LocalRuntimeDiagnostic,
    LocalRuntimeObservability, MemoryResidencySnapshot, ModelMemoryPlan, ModelResidencyPolicy,
    NvidiaDeviceMetadata, NvidiaRecoveryProfile, NvidiaRiskProfile, NvidiaTopologyInfo,
    PrefixCacheIdentity, PrefixCacheReusePolicy, PrefixCacheState,
    QuantizedActivationFingerprintAdapter, SandboxExecutionCapabilityProfile,
    SandboxExecutionEvidence, SandboxExecutionExitKind, SandboxExecutionRequestIdentity,
    ServedArtifactIdentity, SettlementLinkageInput, SignedClusterEvidenceBundle,
    TassadarExactnessRefusalReport, TassadarMismatchSummary, TassadarTraceArtifact,
    TassadarTraceDiffReport, ValidationMatrixReference, validation_reference_for_served_product,
    validation_reference_for_text_generation_model,
};
use psionic_serve::{
    AdapterServingBinding, DecoderModelDescriptor, EMBEDDINGS_PRODUCT_ID, EmbeddingModelDescriptor,
    EmbeddingNormalization, EmbeddingRequest, EmbeddingResponse, GenerationInput,
    GenerationLoadState, GenerationRequest, GenerationResponse, GenerationStreamStatus,
    GenerationStreamTerminal, GenerationStreamingPolicy, ModelArtifactGovernance,
    ModelArtifactProvenanceKind, PsionServedEvidenceBundle, PsionServedOutputClaimPosture,
    QuantizationMode, SessionId, TEXT_GENERATION_PRODUCT_ID, TassadarExecutorCapabilityPublication,
    TerminationReason, WeightArtifactMetadata, WeightBundleMetadata, WeightFormat, WeightSource,
    cache_invalidation_policy, cache_observations_for_embedding_model,
    default_decoder_kv_cache_encoding_policy, default_decoder_kv_cache_policy,
    default_embeddings_execution_profile, default_prefix_cache_policy,
    served_artifact_identity_for_decoder_model, served_artifact_identity_for_embedding_model,
    supported_decoder_kv_cache_encoding_policies,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "provider integration, capabilities, and receipts";

/// Provider-facing backend family identifier.
pub const BACKEND_FAMILY: &str = "psionic";

/// Stable provider-facing product identifier for bounded sandbox execution.
pub const SANDBOX_EXECUTION_PRODUCT_ID: &str = "psionic.sandbox_execution";

/// Provider-facing receipt for one persisted Tassadar trace artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceArtifactReceipt {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable artifact digest.
    pub artifact_digest: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI schema version.
    pub trace_abi_version: u16,
    /// Stable trace digest.
    pub trace_digest: String,
    /// Stable behavior digest.
    pub behavior_digest: String,
    /// Stable runner identifier.
    pub runner_id: String,
    /// Number of emitted steps.
    pub step_count: u64,
}

impl TassadarTraceArtifactReceipt {
    /// Builds one provider-facing receipt from a canonical Tassadar trace artifact.
    #[must_use]
    pub fn from_trace_artifact(artifact: &TassadarTraceArtifact) -> Self {
        Self {
            artifact_id: artifact.artifact_id.clone(),
            artifact_digest: artifact.artifact_digest.clone(),
            trace_abi_id: artifact.trace_abi_id.clone(),
            trace_abi_version: artifact.trace_abi_version,
            trace_digest: artifact.trace_digest.clone(),
            behavior_digest: artifact.behavior_digest.clone(),
            runner_id: artifact.runner_id.clone(),
            step_count: artifact.step_count,
        }
    }
}

/// Provider-facing receipt for one canonical Tassadar trace comparison.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceDiffReceipt {
    /// Stable digest of the expected trace stream.
    pub expected_trace_digest: String,
    /// Stable digest of the actual trace stream.
    pub actual_trace_digest: String,
    /// Whether the compared traces matched exactly.
    pub exact_match: bool,
    /// First differing step index when a mismatch exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_divergence_step_index: Option<usize>,
    /// Number of typed mismatch entries recorded in the comparison.
    pub diff_entry_count: usize,
}

impl TassadarTraceDiffReceipt {
    /// Builds one provider-facing receipt from a canonical Tassadar trace diff report.
    #[must_use]
    pub fn from_trace_diff(report: &TassadarTraceDiffReport) -> Self {
        Self {
            expected_trace_digest: report.expected_trace_digest.clone(),
            actual_trace_digest: report.actual_trace_digest.clone(),
            exact_match: report.exact_match,
            first_divergence_step_index: report.first_divergence_step_index,
            diff_entry_count: report.entries.len(),
        }
    }
}

/// Provider-facing receipt for one standardized Tassadar exactness/refusal report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactnessRefusalReceipt {
    /// Stable subject identifier.
    pub subject_id: String,
    /// Requested decode mode.
    pub requested_decode_mode: psionic_runtime::TassadarExecutorDecodeMode,
    /// Effective decode mode when execution remained allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<psionic_runtime::TassadarExecutorDecodeMode>,
    /// Direct/fallback/refused state reported by runtime selection.
    pub selection_state: psionic_runtime::TassadarExecutorSelectionState,
    /// Typed selection reason when fallback or refusal occurred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_reason: Option<psionic_runtime::TassadarExecutorSelectionReason>,
    /// Exact, mismatch, or refused posture.
    pub exactness_posture: psionic_runtime::TassadarExactnessPosture,
    /// Whether trace digests matched the declared reference.
    pub trace_digest_equal: bool,
    /// Whether final outputs matched the declared reference.
    pub outputs_equal: bool,
    /// Whether halt reasons matched the declared reference.
    pub halt_equal: bool,
    /// Reference behavior digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_behavior_digest: Option<String>,
    /// Observed behavior digest when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_behavior_digest: Option<String>,
    /// Typed mismatch summary when execution diverged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mismatch_summary: Option<TassadarMismatchSummary>,
    /// Typed runtime execution refusal when one occurred after selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_refusal: Option<psionic_runtime::TassadarExecutionRefusal>,
    /// Plain-language detail.
    pub detail: String,
}

impl TassadarExactnessRefusalReceipt {
    /// Builds a provider-facing receipt from the shared runtime report.
    #[must_use]
    pub fn from_runtime_report(report: &TassadarExactnessRefusalReport) -> Self {
        Self {
            subject_id: report.subject_id.clone(),
            requested_decode_mode: report.requested_decode_mode,
            effective_decode_mode: report.effective_decode_mode,
            selection_state: report.selection_state,
            selection_reason: report.selection_reason,
            exactness_posture: report.exactness_posture,
            trace_digest_equal: report.trace_digest_equal,
            outputs_equal: report.outputs_equal,
            halt_equal: report.halt_equal,
            expected_behavior_digest: report.expected_behavior_digest.clone(),
            actual_behavior_digest: report.actual_behavior_digest.clone(),
            mismatch_summary: report.mismatch_summary.clone(),
            execution_refusal: report.execution_refusal.clone(),
            detail: report.detail.clone(),
        }
    }
}

/// Provider-facing receipt for the current research-to-served promotion policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPromotionPolicyReceipt {
    /// Stable promotion-policy identifier.
    pub policy_id: String,
    /// Candidate run identifier.
    pub candidate_run_id: String,
    /// Candidate model identifier.
    pub candidate_model_id: String,
    /// Current claim class for the candidate lane.
    pub candidate_claim_class: psionic_runtime::TassadarClaimClass,
    /// Served product the lane would need to satisfy.
    pub target_product_id: String,
    /// Served route product the lane would need to satisfy.
    pub target_route_product_id: String,
    /// Current promotion status.
    pub status: TassadarPromotionPolicyStatus,
    /// Gates that still block served publication.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_gates: Vec<TassadarPromotionChecklistGateKind>,
    /// Plain-language receipt detail.
    pub detail: String,
}

impl TassadarPromotionPolicyReceipt {
    /// Builds a provider-facing receipt from the shared research report.
    #[must_use]
    pub fn from_report(report: &TassadarPromotionPolicyReport) -> Self {
        let failed_gates = report.failed_gates();
        Self {
            policy_id: report.policy_id.clone(),
            candidate_run_id: report.candidate_run_id.clone(),
            candidate_model_id: report.candidate_model_id.clone(),
            candidate_claim_class: report.candidate_claim_class,
            target_product_id: report.target_product_id.clone(),
            target_route_product_id: report.target_route_product_id.clone(),
            status: report.status,
            detail: if failed_gates.is_empty() {
                format!(
                    "candidate model `{}` currently clears the public promotion checklist",
                    report.candidate_model_id
                )
            } else {
                format!(
                    "candidate model `{}` remains blocked on promotion gates {:?}",
                    report.candidate_model_id, failed_gates
                )
            },
            failed_gates,
        }
    }
}

/// Provider-facing receipt for the current workload capability frontier summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadCapabilityFrontierReceipt {
    /// Stable research summary identifier.
    pub report_id: String,
    /// Number of workload families covered by the frontier.
    pub workload_family_count: u32,
    /// Count of preferred-lane recommendations by lane label.
    pub preferred_lane_counts: BTreeMap<String, u32>,
    /// Count of observation postures by posture label.
    pub observation_posture_counts: BTreeMap<String, u32>,
    /// Workload families with at least one under-mapped preferred lane.
    pub under_mapped_workload_family_ids: Vec<String>,
    /// Workload families still carrying refusal-first posture.
    pub refusal_first_workload_family_ids: Vec<String>,
    /// Plain-language receipt detail.
    pub detail: String,
}

impl TassadarWorkloadCapabilityFrontierReceipt {
    /// Builds a provider-facing receipt from the shared workload-frontier summary.
    #[must_use]
    pub fn from_summary(report: &TassadarWorkloadCapabilityFrontierSummaryReport) -> Self {
        Self {
            report_id: report.report_id.clone(),
            workload_family_count: report.frontier_report.frontier_rows.len() as u32,
            preferred_lane_counts: report.preferred_lane_counts.clone(),
            observation_posture_counts: report.observation_posture_counts.clone(),
            under_mapped_workload_family_ids: report.under_mapped_workload_family_ids.clone(),
            refusal_first_workload_family_ids: report.refusal_first_workload_family_ids.clone(),
            detail: format!(
                "workload frontier currently covers {} families, with {} under-mapped families and {} refusal-first families kept explicit",
                report.frontier_report.frontier_rows.len(),
                report.under_mapped_workload_family_ids.len(),
                report.refusal_first_workload_family_ids.len(),
            ),
        }
    }
}

/// Provider-facing receipt for the current kernel-vs-module scaling summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarKernelModuleScalingReceipt {
    /// Stable research summary identifier.
    pub report_id: String,
    /// Exact trace-length ceilings keyed by scaling phase.
    pub phase_exact_trace_thresholds: BTreeMap<String, u64>,
    /// Kernel-scale families already marked exact-but-cost-degraded.
    pub kernel_cost_degraded_family_ids: Vec<String>,
    /// Module-scale families that still stay exact in the current report.
    pub module_exact_family_ids: Vec<String>,
    /// Families that remain explicit refusal boundaries.
    pub refusal_boundary_family_ids: Vec<String>,
    /// Exact module-scale import-complexity ceiling when one is published.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_exact_import_complexity_threshold: Option<u64>,
    /// Plain-language receipt detail.
    pub detail: String,
}

impl TassadarKernelModuleScalingReceipt {
    /// Builds a provider-facing receipt from the shared kernel-vs-module scaling summary.
    #[must_use]
    pub fn from_summary(report: &TassadarKernelModuleScalingSummaryReport) -> Self {
        let module_exact_import_complexity_threshold = report
            .scaling_report
            .route_thresholds
            .iter()
            .find(|threshold| {
                threshold.phase.as_str() == "module_scale"
                    && threshold.axis.as_str() == "import_complexity"
            })
            .map(|threshold| threshold.max_exact_value);
        Self {
            report_id: report.report_id.clone(),
            phase_exact_trace_thresholds: report.phase_exact_trace_thresholds.clone(),
            kernel_cost_degraded_family_ids: report.kernel_cost_degraded_family_ids.clone(),
            module_exact_family_ids: report.module_exact_family_ids.clone(),
            refusal_boundary_family_ids: report.refusal_boundary_family_ids.clone(),
            module_exact_import_complexity_threshold,
            detail: format!(
                "kernel-vs-module scaling currently keeps exact trace ceilings {:?}, {} kernel cost-degraded families, {} exact module families, and {} refusal boundaries explicit",
                report.phase_exact_trace_thresholds,
                report.kernel_cost_degraded_family_ids.len(),
                report.module_exact_family_ids.len(),
                report.refusal_boundary_family_ids.len(),
            ),
        }
    }
}

/// Provider-facing receipt for one decompilable learned-executor artifact summary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecompilationReceipt {
    /// Stable case identifier.
    pub case_id: String,
    /// Seeded symbolic-reference case identifier.
    pub source_case_id: String,
    /// Candidate model identifier.
    pub candidate_model_id: String,
    /// Stable family label.
    pub family: psionic_models::TassadarDecompilationFamily,
    /// Stable reference program digest.
    pub reference_program_digest: String,
    /// Number of retrains compared for the case.
    pub retrain_count: u32,
    /// Number of distinct readable forms observed across retrains.
    pub distinct_readable_program_count: u32,
    /// Receipt-ready stability class.
    pub stability_class: psionic_models::TassadarDecompilationStabilityClass,
    /// Stable benchmark refs anchoring the receipt.
    pub benchmark_refs: Vec<String>,
    /// Whether the case is ready for provider-facing receipt projection.
    pub receipt_ready: bool,
    /// Plain-language receipt detail.
    pub detail: String,
}

impl TassadarDecompilationReceipt {
    /// Builds a provider-facing receipt from the shared research artifact summary.
    #[must_use]
    pub fn from_artifact_summary(summary: &TassadarDecompilationArtifactSummary) -> Self {
        Self {
            case_id: summary.case_id.clone(),
            source_case_id: summary.source_case_id.clone(),
            candidate_model_id: summary.candidate_model_id.clone(),
            family: summary.family,
            reference_program_digest: summary.reference_program_digest.clone(),
            retrain_count: summary.retrain_count,
            distinct_readable_program_count: summary.distinct_readable_program_count,
            stability_class: summary.stability_class,
            benchmark_refs: summary.benchmark_refs.clone(),
            receipt_ready: summary.receipt_ready,
            detail: if summary.receipt_ready {
                format!(
                    "decompilation case `{}` is receipt-ready with {} retrains and stability class {:?}",
                    summary.source_case_id, summary.retrain_count, summary.stability_class
                )
            } else {
                format!(
                    "decompilation case `{}` is not receipt-ready and remains bounded to research-only artifact comparison",
                    summary.source_case_id
                )
            },
        }
    }
}

/// Provider-facing benchmark-gated capability envelope for the Tassadar executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCapabilityEnvelope {
    /// Provider backend family.
    pub backend_family: String,
    /// Served product identifier.
    pub product_id: String,
    /// Runtime backend surfaced by the executor lane.
    pub runtime_backend: String,
    /// Served capability publication that the provider is exporting.
    pub publication: TassadarExecutorCapabilityPublication,
    /// Provider-facing projection of the served broad internal-compute publication.
    pub broad_internal_compute_profile_publication_receipt:
        TassadarBroadInternalComputeProfilePublicationReceipt,
    /// Provider-facing receipt for the broad internal-compute portability matrix.
    pub broad_internal_compute_portability_receipt: TassadarBroadInternalComputePortabilityReceipt,
    /// Provider-facing receipt for the numeric portability matrix.
    pub numeric_portability_receipt: TassadarNumericPortabilityReceipt,
    /// Provider-facing receipt for the float-profile acceptance gate.
    pub float_profile_acceptance_gate_receipt: TassadarFloatProfileAcceptanceGateReceipt,
    /// Provider-facing receipt for the bounded exceptions proposal profile.
    pub exception_profile_receipt: TassadarExceptionProfileReceipt,
    /// Provider-facing receipt for the bounded SIMD deterministic profile.
    pub simd_profile_receipt: TassadarSimdProfileReceipt,
    /// Provider-facing receipt for the bounded interactive session-process profile.
    pub session_process_profile_receipt: TassadarSessionProcessProfileReceipt,
    /// Provider-facing receipt for the bounded async-lifecycle profile.
    pub async_lifecycle_profile_receipt: TassadarAsyncLifecycleProfileReceipt,
    /// Provider-facing receipt for the bounded preemptive-job profile.
    pub preemptive_job_receipt: TassadarPreemptiveJobReceipt,
    /// Provider-facing receipt for the resumable multi-slice promotion lane.
    pub resumable_multi_slice_promotion_receipt: TassadarResumableMultiSlicePromotionReceipt,
    /// Provider-facing receipt for deterministic import-mediated effect-safe resume.
    pub effect_safe_resume_receipt: TassadarEffectSafeResumeReceipt,
    /// Provider-facing receipt for the subset-profile promotion gate.
    pub subset_profile_promotion_gate_receipt: TassadarSubsetProfilePromotionGateReceipt,
    /// Provider-facing receipt for the frozen core-Wasm closure gate.
    pub frozen_core_wasm_closure_gate_receipt: TassadarFrozenCoreWasmClosureGateReceipt,
    /// Backend and quantization deployment truth for the served lane.
    pub quantization_truth_envelope: TassadarDeploymentTruthEnvelope,
    /// Current provider readiness state.
    pub readiness: ProviderReadiness,
}

impl TassadarCapabilityEnvelope {
    /// Builds a provider-facing Tassadar capability envelope from the served publication.
    pub fn from_executor_capability_publication(
        publication: &TassadarExecutorCapabilityPublication,
        readiness: ProviderReadiness,
    ) -> Result<Self, TassadarCapabilityEnvelopeError> {
        if !publication.internal_compute_profile_claim_check.green {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishableInternalComputeProfileClaim {
                    detail: publication
                        .internal_compute_profile_claim_check
                        .detail
                        .clone(),
                },
            );
        }
        publication
            .workload_capability_matrix
            .validate_publication()
            .map_err(
                |error| TassadarCapabilityEnvelopeError::UnpublishableWorkloadMatrix {
                    detail: error.to_string(),
                },
            )?;
        let quantization_truth_envelope =
            TassadarDeploymentTruthEnvelope::from_served_quantization_truth_envelope(
                &publication.quantization_truth_envelope,
            )
            .map_err(|error| {
                TassadarCapabilityEnvelopeError::UnpublishableQuantizationTruthEnvelope {
                    detail: format!("{error:?}"),
                }
            })?;
        let broad_internal_compute_profile_publication_receipt =
            TassadarBroadInternalComputeProfilePublicationReceipt::from_publication(
                &publication.broad_internal_compute_profile_publication,
            );
        let broad_internal_compute_portability_report =
            psionic_eval::build_tassadar_broad_internal_compute_portability_report().map_err(
                |error| {
                    TassadarCapabilityEnvelopeError::UnpublishableBroadInternalComputePortability {
                        detail: format!(
                            "provider envelope requires a valid broad internal-compute portability report: {error}"
                        ),
                    }
                },
            )?;
        let broad_internal_compute_portability_receipt =
            TassadarBroadInternalComputePortabilityReceipt::from_report(
                &broad_internal_compute_portability_report,
            );
        let numeric_portability_report = psionic_eval::build_tassadar_numeric_portability_report()
            .map_err(
                |error| TassadarCapabilityEnvelopeError::UnpublishableNumericPortability {
                    detail: format!(
                        "provider envelope requires a valid numeric portability report: {error}"
                    ),
                },
            )?;
        let numeric_portability_receipt =
            TassadarNumericPortabilityReceipt::from_report(&numeric_portability_report);
        let float_profile_acceptance_gate_report =
            psionic_eval::build_tassadar_float_profile_acceptance_gate_report().map_err(
                |error| TassadarCapabilityEnvelopeError::UnpublishableFloatProfileAcceptanceGate {
                    detail: format!(
                        "provider envelope requires a valid float profile acceptance gate report: {error}"
                    ),
                },
            )?;
        let float_profile_acceptance_gate_receipt =
            TassadarFloatProfileAcceptanceGateReceipt::from_report(
                &float_profile_acceptance_gate_report,
            );
        let exception_profile_report = psionic_eval::build_tassadar_exception_profile_report();
        let exception_profile_receipt =
            TassadarExceptionProfileReceipt::from_report(&exception_profile_report);
        let simd_profile_report =
            psionic_eval::build_tassadar_simd_profile_report().map_err(|error| {
                TassadarCapabilityEnvelopeError::UnpublishableSimdProfile {
                    detail: format!(
                        "provider envelope requires a valid simd profile report: {error}"
                    ),
                }
            })?;
        let simd_profile_receipt = TassadarSimdProfileReceipt::from_report(&simd_profile_report);
        let session_process_profile_report =
            psionic_eval::build_tassadar_session_process_profile_report().map_err(|error| {
                TassadarCapabilityEnvelopeError::UnpublishableSessionProcessProfile {
                    detail: format!(
                        "provider envelope requires a valid session-process profile report: {error}"
                    ),
                }
            })?;
        let session_process_profile_receipt =
            TassadarSessionProcessProfileReceipt::from_report(&session_process_profile_report);
        let async_lifecycle_profile_report =
            psionic_eval::build_tassadar_async_lifecycle_profile_report().map_err(|error| {
                TassadarCapabilityEnvelopeError::UnpublishableAsyncLifecycleProfile {
                    detail: format!(
                        "provider envelope requires a valid async-lifecycle profile report: {error}"
                    ),
                }
            })?;
        let async_lifecycle_profile_receipt =
            TassadarAsyncLifecycleProfileReceipt::from_report(&async_lifecycle_profile_report);
        let preemptive_job_profile_report =
            psionic_eval::build_tassadar_preemptive_job_profile_report().map_err(|error| {
                TassadarCapabilityEnvelopeError::UnpublishablePreemptiveJobProfile {
                    detail: format!(
                        "provider envelope requires a valid preemptive-job profile report: {error}"
                    ),
                }
            })?;
        let preemptive_job_receipt =
            TassadarPreemptiveJobReceipt::from_report(&preemptive_job_profile_report);
        let resumable_multi_slice_promotion_report =
            psionic_eval::build_tassadar_resumable_multi_slice_promotion_report().map_err(
                |error| {
                    TassadarCapabilityEnvelopeError::UnpublishableBroadInternalComputeProfilePublication {
                        detail: format!(
                            "provider envelope requires a valid resumable multi-slice promotion report: {error}"
                        ),
                    }
                },
            )?;
        let resumable_multi_slice_promotion_receipt =
            TassadarResumableMultiSlicePromotionReceipt::from_report(
                &resumable_multi_slice_promotion_report,
            );
        let effect_safe_resume_report =
            psionic_eval::build_tassadar_effect_safe_resume_report().map_err(|error| {
                TassadarCapabilityEnvelopeError::UnpublishableBroadInternalComputeProfilePublication {
                    detail: format!(
                        "provider envelope requires a valid effect-safe resume report: {error}"
                    ),
                }
            })?;
        let effect_safe_resume_receipt =
            TassadarEffectSafeResumeReceipt::from_report(&effect_safe_resume_report);
        let subset_profile_promotion_gate_report =
            psionic_eval::build_tassadar_subset_profile_promotion_gate_report().map_err(
                |error| {
                    TassadarCapabilityEnvelopeError::UnpublishableSubsetProfilePromotionGate {
                        detail: format!(
                            "provider envelope requires a valid subset profile promotion gate report: {error}"
                        ),
                    }
                },
            )?;
        let subset_profile_promotion_gate_receipt =
            TassadarSubsetProfilePromotionGateReceipt::from_report(
                &subset_profile_promotion_gate_report,
            );
        let frozen_core_wasm_closure_gate_report =
            psionic_eval::build_tassadar_frozen_core_wasm_closure_gate_report().map_err(
                |error| {
                    TassadarCapabilityEnvelopeError::UnpublishableFrozenCoreWasmClosureGate {
                        detail: format!(
                            "provider envelope requires a valid frozen core-Wasm closure gate report: {error}"
                        ),
                    }
                },
            )?;
        let frozen_core_wasm_closure_gate_receipt =
            TassadarFrozenCoreWasmClosureGateReceipt::from_report(
                &frozen_core_wasm_closure_gate_report,
            );
        if broad_internal_compute_profile_publication_receipt
            .current_served_profile_id
            .trim()
            .is_empty()
            || broad_internal_compute_profile_publication_receipt
                .route_policy_report_ref
                .trim()
                .is_empty()
            || !broad_internal_compute_profile_publication_receipt
                .published_profile_ids
                .contains(
                    &broad_internal_compute_profile_publication_receipt.current_served_profile_id,
                )
            || !broad_internal_compute_profile_publication_receipt
                .public_profile_specific_route_ids
                .contains(&String::from(
                    "tassadar.internal_compute.deterministic_import_subset.v1",
                ))
            || !broad_internal_compute_profile_publication_receipt
                .public_profile_specific_route_ids
                .contains(&String::from(
                    "tassadar.internal_compute.runtime_support_subset.v1",
                ))
            || !broad_internal_compute_profile_publication_receipt
                .profile_specific_world_mount_template_ids
                .contains(&String::from(
                    "tassadar.internal_compute.deterministic_import_subset.v1",
                ))
            || !broad_internal_compute_profile_publication_receipt
                .profile_specific_world_mount_template_ids
                .contains(&String::from(
                    "tassadar.internal_compute.runtime_support_subset.v1",
                ))
            || !broad_internal_compute_profile_publication_receipt
                .profile_specific_accepted_outcome_template_ids
                .contains(&String::from(
                    "tassadar.internal_compute.deterministic_import_subset.v1",
                ))
            || !broad_internal_compute_profile_publication_receipt
                .profile_specific_accepted_outcome_template_ids
                .contains(&String::from(
                    "tassadar.internal_compute.runtime_support_subset.v1",
                ))
            || broad_internal_compute_profile_publication_receipt.current_served_profile_id
                != publication
                    .internal_compute_profile_claim_check
                    .claim
                    .profile_id
        {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishableBroadInternalComputeProfilePublication {
                    detail: String::from(
                        "provider envelope requires a non-empty current served broad profile id, a route-policy ref, a published current-served profile, explicit profile-specific route/mount/accepted-outcome promotion for the deterministic-import and runtime-support subsets, and end-to-end agreement with the served internal-compute claim",
                    ),
                },
            );
        }
        if publication
            .subset_profile_promotion_gate_report_ref
            .trim()
            .is_empty()
            || !subset_profile_promotion_gate_receipt.overall_green
            || !subset_profile_promotion_gate_receipt
                .green_profile_ids
                .contains(&String::from(
                    "tassadar.internal_compute.deterministic_import_subset.v1",
                ))
            || !subset_profile_promotion_gate_receipt
                .green_profile_ids
                .contains(&String::from(
                    "tassadar.internal_compute.runtime_support_subset.v1",
                ))
            || !subset_profile_promotion_gate_receipt
                .served_publication_allowed_profile_ids
                .is_empty()
        {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishableSubsetProfilePromotionGate {
                    detail: String::from(
                        "provider envelope requires a non-empty subset profile promotion gate ref, green deterministic-import and runtime-support subset rows, and zero served-publication-allowed subset profiles",
                    ),
                },
            );
        }
        if publication
            .broad_internal_compute_portability_report_ref
            .trim()
            .is_empty()
            || publication
                .broad_internal_compute_portability_backend_family_ids
                .is_empty()
            || publication
                .broad_internal_compute_portability_toolchain_family_ids
                .is_empty()
            || publication.broad_internal_compute_portability_backend_family_ids
                != broad_internal_compute_portability_receipt.backend_family_ids
            || publication.broad_internal_compute_portability_toolchain_family_ids
                != broad_internal_compute_portability_receipt.toolchain_family_ids
            || !broad_internal_compute_portability_receipt
                .publication_allowed_profile_ids
                .contains(&String::from(
                    "tassadar.internal_compute.article_closeout.v1",
                ))
            || !broad_internal_compute_portability_receipt
                .backend_family_ids
                .contains(&String::from("cpu_reference"))
            || !broad_internal_compute_portability_receipt
                .backend_family_ids
                .contains(&String::from("cuda_served"))
            || !broad_internal_compute_portability_receipt
                .backend_family_ids
                .contains(&String::from("metal_served"))
        {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishableBroadInternalComputePortability {
                    detail: String::from(
                        "provider envelope requires a non-empty broad portability ref, non-empty backend/toolchain families, exact agreement with the committed portability report, and explicit cpu/metal/cuda backend envelopes with article-closeout publication still bounded to the committed matrix",
                    ),
                },
            );
        }
        if publication
            .frozen_core_wasm_window_report_ref
            .trim()
            .is_empty()
            || publication
                .frozen_core_wasm_closure_gate_report_ref
                .trim()
                .is_empty()
            || publication.frozen_core_wasm_closure_status
                != frozen_core_wasm_closure_gate_receipt.closure_status
            || publication.frozen_core_wasm_served_publication_allowed
                != frozen_core_wasm_closure_gate_receipt.served_publication_allowed
        {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishableFrozenCoreWasmClosureGate {
                    detail: String::from(
                        "provider envelope requires non-empty frozen core-Wasm window and closure-gate refs plus exact agreement with the committed closure-gate status and served-publication posture",
                    ),
                },
            );
        }
        if publication.numeric_portability_report_ref.trim().is_empty()
            || publication
                .numeric_portability_backend_family_ids
                .is_empty()
            || publication
                .numeric_portability_toolchain_family_ids
                .is_empty()
            || publication.numeric_portability_profile_ids.is_empty()
            || publication.numeric_portability_backend_family_ids
                != numeric_portability_receipt.backend_family_ids
            || publication.numeric_portability_toolchain_family_ids
                != numeric_portability_receipt.toolchain_family_ids
            || publication.numeric_portability_profile_ids
                != numeric_portability_receipt.profile_ids
            || !numeric_portability_receipt
                .publication_allowed_profile_ids
                .contains(&String::from("tassadar.numeric_profile.f32_only.v1"))
            || !numeric_portability_receipt
                .publication_allowed_profile_ids
                .contains(&String::from("tassadar.numeric_profile.mixed_i32_f32.v1"))
            || !numeric_portability_receipt
                .suppressed_profile_ids
                .contains(&String::from(
                    "tassadar.numeric_profile.bounded_f64_conversion.v1",
                ))
        {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishableNumericPortability {
                    detail: String::from(
                        "provider envelope requires a non-empty numeric portability ref, non-empty backend/toolchain/profile ids, exact agreement with the committed numeric portability report, exact cpu-reference publication for the exact f32-only and mixed-i32-f32 profiles, and continued suppression of the bounded f64 conversion profile",
                    ),
                },
            );
        }
        if publication
            .float_profile_acceptance_gate_report_ref
            .trim()
            .is_empty()
            || publication
                .float_profile_route_policy_report_ref
                .trim()
                .is_empty()
            || !float_profile_acceptance_gate_receipt.overall_green
            || !float_profile_acceptance_gate_receipt
                .public_profile_allowed_profile_ids
                .contains(&String::from("tassadar.numeric_profile.f32_only.v1"))
            || !float_profile_acceptance_gate_receipt
                .public_profile_allowed_profile_ids
                .contains(&String::from("tassadar.numeric_profile.mixed_i32_f32.v1"))
            || !float_profile_acceptance_gate_receipt
                .default_served_profile_allowed_profile_ids
                .is_empty()
            || !float_profile_acceptance_gate_receipt
                .suppressed_profile_ids
                .contains(&String::from(
                    "tassadar.numeric_profile.bounded_f64_conversion.v1",
                ))
        {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishableFloatProfileAcceptanceGate {
                    detail: String::from(
                        "provider envelope requires non-empty float-profile acceptance-gate and route-policy refs, green exact numeric profile rows, no default served float-enabled profiles, and continued suppression of the bounded f64 conversion profile",
                    ),
                },
            );
        }
        if publication.exception_profile_report_ref.trim().is_empty()
            || publication.exception_profile_public_profile_ids
                != exception_profile_receipt.public_profile_allowed_profile_ids
            || publication.exception_profile_default_served_profile_ids
                != exception_profile_receipt.default_served_profile_allowed_profile_ids
            || publication.exception_profile_portability_envelope_ids
                != exception_profile_receipt.portability_envelope_ids
            || !exception_profile_receipt.overall_green
            || !exception_profile_receipt
                .public_profile_allowed_profile_ids
                .contains(&String::from(
                    TASSADAR_EXCEPTION_PROFILE_TRY_CATCH_RETHROW_ID,
                ))
            || !exception_profile_receipt
                .default_served_profile_allowed_profile_ids
                .is_empty()
            || !exception_profile_receipt
                .portability_envelope_ids
                .contains(&String::from(
                    TASSADAR_EXCEPTION_PROFILE_CURRENT_HOST_CPU_REFERENCE_ENVELOPE_ID,
                ))
            || exception_profile_receipt.exact_trap_stack_parity_case_count == 0
        {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishableExceptionProfile {
                    detail: String::from(
                        "provider envelope requires a non-empty exception-profile report ref, exact agreement with the committed public/default-served/portability ids, a green bounded exception profile, explicit trap-stack parity, the named try/catch/rethrow profile in public posture, a current-host cpu-reference envelope, and zero default served exception profiles",
                    ),
                },
            );
        }
        if publication.simd_profile_report_ref.trim().is_empty()
            || publication.simd_profile_public_profile_ids
                != simd_profile_receipt.public_profile_allowed_profile_ids
            || publication.simd_profile_default_served_profile_ids
                != simd_profile_receipt.default_served_profile_allowed_profile_ids
            || publication.simd_profile_exact_backend_ids != simd_profile_receipt.exact_backend_ids
            || publication.simd_profile_fallback_backend_ids
                != simd_profile_receipt.fallback_backend_ids
            || publication.simd_profile_refused_backend_ids
                != simd_profile_receipt.refused_backend_ids
            || !simd_profile_receipt.overall_green
            || !simd_profile_receipt
                .public_profile_allowed_profile_ids
                .contains(&String::from(
                    "tassadar.proposal_profile.simd_deterministic.v1",
                ))
            || !simd_profile_receipt
                .default_served_profile_allowed_profile_ids
                .is_empty()
            || !simd_profile_receipt
                .exact_backend_ids
                .contains(&String::from("cpu_reference_current_host"))
            || !simd_profile_receipt
                .fallback_backend_ids
                .contains(&String::from("metal_served"))
            || !simd_profile_receipt
                .fallback_backend_ids
                .contains(&String::from("cuda_served"))
            || simd_profile_receipt.refused_backend_ids.is_empty()
        {
            return Err(TassadarCapabilityEnvelopeError::UnpublishableSimdProfile {
                detail: String::from(
                    "provider envelope requires a non-empty simd-profile report ref, exact agreement with the committed public/default-served/backend ids, a green bounded simd profile, one named public simd profile, zero default served simd profiles, a cpu-reference exact row, explicit metal/cuda fallback rows, and at least one refused backend row",
                ),
            });
        }
        if publication
            .session_process_profile_report_ref
            .trim()
            .is_empty()
            || publication
                .session_process_route_policy_report_ref
                .trim()
                .is_empty()
            || publication.session_process_public_profile_ids
                != session_process_profile_receipt.public_profile_allowed_profile_ids
            || publication.session_process_default_served_profile_ids
                != session_process_profile_receipt.default_served_profile_allowed_profile_ids
            || publication.session_process_routeable_interaction_surface_ids
                != session_process_profile_receipt.routeable_interaction_surface_ids
            || publication.session_process_refused_interaction_surface_ids
                != session_process_profile_receipt.refused_interaction_surface_ids
            || !session_process_profile_receipt.overall_green
            || !session_process_profile_receipt
                .public_profile_allowed_profile_ids
                .contains(&String::from(
                    "tassadar.internal_compute.session_process.v1",
                ))
            || !session_process_profile_receipt
                .default_served_profile_allowed_profile_ids
                .is_empty()
            || !session_process_profile_receipt
                .routeable_interaction_surface_ids
                .contains(&String::from("deterministic_echo_turn_loop"))
            || !session_process_profile_receipt
                .routeable_interaction_surface_ids
                .contains(&String::from("stateful_counter_turn_loop"))
            || !session_process_profile_receipt
                .refused_interaction_surface_ids
                .contains(&String::from("open_ended_external_event_stream"))
        {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishableSessionProcessProfile {
                    detail: String::from(
                        "provider envelope requires non-empty session-process profile and route-policy refs, exact agreement with the committed public/default-served/interaction-surface ids, a green bounded session-process profile, one named public session-process profile, zero default served session-process profiles, two deterministic routeable interaction surfaces, and explicit refusal on the open-ended external event stream surface",
                    ),
                },
            );
        }
        if publication
            .async_lifecycle_profile_report_ref
            .trim()
            .is_empty()
            || publication
                .async_lifecycle_route_policy_report_ref
                .trim()
                .is_empty()
            || publication.async_lifecycle_public_profile_ids
                != async_lifecycle_profile_receipt.public_profile_allowed_profile_ids
            || publication.async_lifecycle_default_served_profile_ids
                != async_lifecycle_profile_receipt.default_served_profile_allowed_profile_ids
            || publication.async_lifecycle_routeable_surface_ids
                != async_lifecycle_profile_receipt.routeable_lifecycle_surface_ids
            || publication.async_lifecycle_refused_surface_ids
                != async_lifecycle_profile_receipt.refused_lifecycle_surface_ids
            || !async_lifecycle_profile_receipt.overall_green
            || !async_lifecycle_profile_receipt
                .public_profile_allowed_profile_ids
                .contains(&String::from(
                    "tassadar.internal_compute.async_lifecycle.v1",
                ))
            || !async_lifecycle_profile_receipt
                .default_served_profile_allowed_profile_ids
                .is_empty()
            || !async_lifecycle_profile_receipt
                .routeable_lifecycle_surface_ids
                .contains(&String::from("interruptible_counter_job"))
            || !async_lifecycle_profile_receipt
                .routeable_lifecycle_surface_ids
                .contains(&String::from("retryable_timeout_search_job"))
            || !async_lifecycle_profile_receipt
                .routeable_lifecycle_surface_ids
                .contains(&String::from("safe_boundary_cancellation_job"))
            || !async_lifecycle_profile_receipt
                .refused_lifecycle_surface_ids
                .contains(&String::from("open_ended_external_callback"))
            || !async_lifecycle_profile_receipt
                .refused_lifecycle_surface_ids
                .contains(&String::from("mid_effect_cancellation"))
            || !async_lifecycle_profile_receipt
                .refused_lifecycle_surface_ids
                .contains(&String::from("unbounded_retry_backoff"))
        {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishableAsyncLifecycleProfile {
                    detail: String::from(
                        "provider envelope requires non-empty async-lifecycle profile and route-policy refs, exact agreement with the committed public/default-served/lifecycle-surface ids, a green bounded async-lifecycle profile, one named public async-lifecycle profile, zero default served async-lifecycle profiles, three deterministic routeable lifecycle surfaces, and explicit refusal on open-ended callbacks, mid-effect cancellation, and unbounded retry",
                    ),
                },
            );
        }
        if publication
            .preemptive_job_profile_report_ref
            .trim()
            .is_empty()
            || publication
                .preemptive_job_fairness_report_ref
                .trim()
                .is_empty()
            || publication.preemptive_job_profile_id != preemptive_job_receipt.profile_id
            || publication.preemptive_job_green_scheduler_ids
                != preemptive_job_receipt.green_scheduler_ids
            || publication.preemptive_job_refused_scheduler_ids
                != preemptive_job_receipt.refused_scheduler_ids
            || publication.preemptive_job_exact_case_count
                != preemptive_job_receipt.exact_case_count
            || publication.preemptive_job_refusal_case_count
                != preemptive_job_receipt.refusal_case_count
            || publication.preemptive_job_served_publication_allowed
                != preemptive_job_receipt.served_publication_allowed
            || !preemptive_job_profile_report.overall_green
            || !preemptive_job_receipt
                .green_scheduler_ids
                .contains(&String::from("deterministic_round_robin"))
            || !preemptive_job_receipt
                .green_scheduler_ids
                .contains(&String::from("weighted_fair_slice_rotation"))
            || !preemptive_job_receipt
                .refused_scheduler_ids
                .contains(&String::from("host_nondeterministic_scheduler"))
            || preemptive_job_receipt.served_publication_allowed
            || !preemptive_job_receipt
                .resumable_process_ids
                .contains(&String::from("tassadar.process.long_loop_kernel.v1"))
            || !preemptive_job_receipt
                .resumable_process_ids
                .contains(&String::from("tassadar.process.search_frontier_kernel.v1"))
        {
            return Err(
                TassadarCapabilityEnvelopeError::UnpublishablePreemptiveJobProfile {
                    detail: String::from(
                        "provider envelope requires non-empty preemptive-job profile and fairness refs, exact agreement with the committed profile/scheduler/count posture, a green bounded preemptive-job profile, deterministic round-robin and weighted-fair scheduler rows, explicit refusal on the host-nondeterministic scheduler, resumable long-loop and search-frontier process ids, and zero served-publication widening",
                    ),
                },
            );
        }
        Ok(Self {
            backend_family: String::from(BACKEND_FAMILY),
            product_id: publication.product_id.clone(),
            runtime_backend: publication.runtime_capability.runtime_backend.clone(),
            publication: publication.clone(),
            broad_internal_compute_profile_publication_receipt,
            broad_internal_compute_portability_receipt,
            numeric_portability_receipt,
            float_profile_acceptance_gate_receipt,
            exception_profile_receipt,
            simd_profile_receipt,
            session_process_profile_receipt,
            async_lifecycle_profile_receipt,
            preemptive_job_receipt,
            resumable_multi_slice_promotion_receipt,
            effect_safe_resume_receipt,
            subset_profile_promotion_gate_receipt,
            frozen_core_wasm_closure_gate_receipt,
            quantization_truth_envelope,
            readiness,
        })
    }
}

/// Provider-facing capability-envelope failure for the Tassadar executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarCapabilityEnvelopeError {
    /// The served internal-compute profile claim was not publishable.
    UnpublishableInternalComputeProfileClaim {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served matrix is not benchmark-gated enough for provider publication.
    UnpublishableWorkloadMatrix {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served quantization truth envelope was not publishable provider-side.
    UnpublishableQuantizationTruthEnvelope {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served broad internal-compute publication was not publishable provider-side.
    UnpublishableBroadInternalComputeProfilePublication {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served broad internal-compute portability matrix was not publishable provider-side.
    UnpublishableBroadInternalComputePortability {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served numeric portability matrix was not publishable provider-side.
    UnpublishableNumericPortability {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served float-profile acceptance gate was not publishable provider-side.
    UnpublishableFloatProfileAcceptanceGate {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served exceptions proposal profile was not publishable provider-side.
    UnpublishableExceptionProfile {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served SIMD deterministic profile was not publishable provider-side.
    UnpublishableSimdProfile {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served interactive session-process profile was not publishable provider-side.
    UnpublishableSessionProcessProfile {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served async-lifecycle profile was not publishable provider-side.
    UnpublishableAsyncLifecycleProfile {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served preemptive-job profile was not publishable provider-side.
    UnpublishablePreemptiveJobProfile {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served subset-profile promotion gate was not publishable provider-side.
    UnpublishableSubsetProfilePromotionGate {
        /// Plain-text validation detail.
        detail: String,
    },
    /// The served frozen core-Wasm closure gate was not publishable provider-side.
    UnpublishableFrozenCoreWasmClosureGate {
        /// Plain-text validation detail.
        detail: String,
    },
}

/// Provider-facing capability envelope for planner / executor route negotiation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRouteCapabilityEnvelope {
    /// Provider backend family.
    pub backend_family: String,
    /// Planner-owned routed product identifier.
    pub product_id: String,
    /// Served route descriptor exported by the provider.
    pub route_descriptor: TassadarPlannerExecutorRouteDescriptor,
    /// Current provider readiness state.
    pub readiness: ProviderReadiness,
}

impl TassadarPlannerRouteCapabilityEnvelope {
    /// Builds a provider-facing planner-route capability envelope.
    pub fn from_route_descriptor(
        route_descriptor: &TassadarPlannerExecutorRouteDescriptor,
        readiness: ProviderReadiness,
    ) -> Result<Self, TassadarPlannerRouteCapabilityEnvelopeError> {
        if route_descriptor.product_id != TASSADAR_PLANNER_EXECUTOR_ROUTE_PRODUCT_ID {
            return Err(
                TassadarPlannerRouteCapabilityEnvelopeError::UnsupportedPlannerRouteProduct {
                    product_id: route_descriptor.product_id.clone(),
                },
            );
        }
        if route_descriptor.decode_capabilities.is_empty() {
            return Err(
                TassadarPlannerRouteCapabilityEnvelopeError::MissingDecodeCapabilities {
                    route_id: route_descriptor.route_id.clone(),
                },
            );
        }
        if route_descriptor.wasm_capability_matrix.rows.is_empty() {
            return Err(
                TassadarPlannerRouteCapabilityEnvelopeError::UnpublishableWasmCapabilityMatrix {
                    route_id: route_descriptor.route_id.clone(),
                    detail: String::from(
                        "route descriptor was missing routeable Wasm capability rows",
                    ),
                },
            );
        }
        if let Some(row) = route_descriptor
            .wasm_capability_matrix
            .rows
            .iter()
            .find(|row| {
                !row.supported_decode_modes.is_empty()
                    && row
                        .benchmark_report_ref
                        .as_deref()
                        .map_or(true, |value| value.trim().is_empty())
            })
        {
            return Err(
                TassadarPlannerRouteCapabilityEnvelopeError::UnpublishableWasmCapabilityMatrix {
                    route_id: route_descriptor.route_id.clone(),
                    detail: format!(
                        "module class `{}` was missing a benchmark report ref",
                        row.module_class.as_str()
                    ),
                },
            );
        }
        if route_descriptor.benchmark_report_ref.trim().is_empty() {
            return Err(
                TassadarPlannerRouteCapabilityEnvelopeError::MissingBenchmarkGate {
                    route_id: route_descriptor.route_id.clone(),
                },
            );
        }
        if route_descriptor
            .internal_compute_profile_id
            .trim()
            .is_empty()
        {
            return Err(
                TassadarPlannerRouteCapabilityEnvelopeError::MissingInternalComputeProfileId {
                    route_id: route_descriptor.route_id.clone(),
                },
            );
        }
        if route_descriptor
            .internal_compute_profile_claim_digest
            .trim()
            .is_empty()
        {
            return Err(
                TassadarPlannerRouteCapabilityEnvelopeError::MissingInternalComputeProfileClaimDigest {
                    route_id: route_descriptor.route_id.clone(),
                },
            );
        }
        Ok(Self {
            backend_family: String::from(BACKEND_FAMILY),
            product_id: route_descriptor.product_id.clone(),
            route_descriptor: route_descriptor.clone(),
            readiness,
        })
    }

    /// Converts the provider-facing envelope into one router-owned route candidate.
    #[must_use]
    pub fn as_route_candidate(
        &self,
        provider_id: impl Into<String>,
        worker_id: impl Into<String>,
    ) -> TassadarPlannerExecutorRouteCandidate {
        TassadarPlannerExecutorRouteCandidate::new(
            provider_id,
            worker_id,
            self.backend_family.clone(),
            self.readiness.status == HealthStatus::Ready,
            self.route_descriptor.clone(),
        )
    }
}

/// Provider-facing planner-route envelope failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarPlannerRouteCapabilityEnvelopeError {
    /// The routed product identifier was not the planner / executor route surface.
    UnsupportedPlannerRouteProduct {
        /// Unexpected product identifier.
        product_id: String,
    },
    /// No per-decode route capability rows were published.
    MissingDecodeCapabilities {
        /// Stable route identifier.
        route_id: String,
    },
    /// No benchmark gate was attached to the route descriptor.
    MissingBenchmarkGate {
        /// Stable route identifier.
        route_id: String,
    },
    /// The route descriptor did not publish a named internal-compute profile id.
    MissingInternalComputeProfileId {
        /// Stable route identifier.
        route_id: String,
    },
    /// The route descriptor did not publish a stable internal-compute claim digest.
    MissingInternalComputeProfileClaimDigest {
        /// Stable route identifier.
        route_id: String,
    },
    /// The route omitted the routeable Wasm capability matrix or published invalid served rows.
    UnpublishableWasmCapabilityMatrix {
        /// Stable route identifier.
        route_id: String,
        /// Plain-language validation detail.
        detail: String,
    },
}

/// Stable provider-facing summary of the weight bundle backing a served model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightBundleEvidence {
    /// Weight artifact format.
    pub format: WeightFormat,
    /// Weight source authority.
    pub source: WeightSource,
    /// Weight quantization posture.
    pub quantization: QuantizationMode,
    /// All quantization modes observed across the loaded logical weights.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quantization_modes: Vec<QuantizationMode>,
    /// Stable bundle digest.
    pub digest: String,
    /// External artifacts that backed the bundle, if any.
    pub artifacts: Vec<WeightArtifactMetadata>,
}

impl WeightBundleEvidence {
    /// Creates weight-bundle evidence from stable model metadata.
    #[must_use]
    pub fn from_metadata(metadata: &WeightBundleMetadata) -> Self {
        Self {
            format: metadata.format,
            source: metadata.source,
            quantization: metadata.quantization,
            quantization_modes: metadata.quantization_modes.clone(),
            digest: metadata.digest.clone(),
            artifacts: metadata.artifacts.clone(),
        }
    }
}

fn compiled_backend_features_for_selection(backend_selection: &BackendSelection) -> Vec<String> {
    let mut features = backend_selection
        .backend_extensions
        .iter()
        .map(|support| {
            format!(
                "{}:{}",
                support.kind.label(),
                match support.execution {
                    psionic_runtime::BackendExtensionExecution::Reference => "reference",
                    psionic_runtime::BackendExtensionExecution::BackendSpecialized =>
                        "backend_specialized",
                }
            )
        })
        .collect::<Vec<_>>();
    features.sort();
    features.dedup();
    features
}

fn backend_toolchain_identity_for_selection(
    backend_selection: &BackendSelection,
) -> BackendToolchainIdentity {
    let selected_devices = backend_selection.selected_devices();
    let probe_state = if selected_devices.is_empty() {
        BackendProbeState::CompiledOnly
    } else {
        BackendProbeState::CompiledAndProbed
    };
    let mut probed_backend_features = selected_devices
        .into_iter()
        .flat_map(|device| device.feature_flags.iter().cloned())
        .collect::<Vec<_>>();
    probed_backend_features.sort();
    probed_backend_features.dedup();

    BackendToolchainIdentity::new(
        backend_selection.effective_backend.as_str(),
        format!(
            "{}@{}",
            backend_selection.effective_backend,
            env!("CARGO_PKG_VERSION")
        ),
        compiled_backend_features_for_selection(backend_selection),
    )
    .with_probe(probe_state, probed_backend_features)
}

fn selected_device_inventory_for_selection(
    backend_selection: &BackendSelection,
) -> Option<DeviceInventoryQualifiers> {
    backend_selection.selected_device_inventory()
}

fn selected_devices_inventory_for_selection(
    backend_selection: &BackendSelection,
) -> Vec<DeviceInventoryQualifiers> {
    backend_selection.selected_devices_inventory()
}

fn execution_topology_for_selection(
    backend_selection: &BackendSelection,
) -> Option<ExecutionTopologyPlan> {
    backend_selection.execution_topology_plan()
}

fn apply_cluster_execution_surface(
    selected_device_inventory: &mut Option<DeviceInventoryQualifiers>,
    selected_devices: &mut Vec<DeviceInventoryQualifiers>,
    execution_topology: &mut Option<ExecutionTopologyPlan>,
    cluster_execution: &ClusterExecutionContext,
) {
    if let Some(cluster_execution_topology) = &cluster_execution.execution_topology {
        *execution_topology = Some(cluster_execution_topology.clone());
    }
    let cluster_selected_devices = cluster_execution.selected_devices_inventory();
    if let Some(first_cluster_device) = cluster_selected_devices.first().cloned() {
        *selected_device_inventory = Some(first_cluster_device);
        *selected_devices = cluster_selected_devices;
    }
}

fn embedding_delivery_proof(response: &EmbeddingResponse) -> Option<ExecutionDeliveryProof> {
    response
        .provenance
        .as_ref()
        .and_then(|value| value.delivery_proof.clone())
}

fn generation_delivery_proof(response: &GenerationResponse) -> Option<ExecutionDeliveryProof> {
    response
        .provenance
        .as_ref()
        .and_then(|value| value.delivery_proof.clone())
}

fn embedding_cluster_execution(response: &EmbeddingResponse) -> Option<ClusterExecutionContext> {
    response
        .provenance
        .as_ref()
        .and_then(|value| value.cluster_execution.clone())
}

fn generation_cluster_execution(response: &GenerationResponse) -> Option<ClusterExecutionContext> {
    response
        .provenance
        .as_ref()
        .and_then(|value| value.cluster_execution.clone())
}

fn generation_adapter_serving(response: &GenerationResponse) -> Option<AdapterServingBinding> {
    response
        .provenance
        .as_ref()
        .and_then(|value| value.adapter_serving.clone())
}

struct SettlementLinkageContext<'a> {
    request_digest: String,
    product_id: &'a str,
    model_id: &'a str,
    served_artifact: &'a ServedArtifactIdentity,
    adapter_serving: Option<&'a AdapterServingBinding>,
    runtime_backend: &'a str,
    cluster_execution: Option<&'a ClusterExecutionContext>,
    output_tokens: Option<usize>,
}

fn settlement_linkage(
    context: SettlementLinkageContext<'_>,
    delivery_proof: &ExecutionDeliveryProof,
) -> SettlementLinkageInput {
    SettlementLinkageInput {
        request_digest: context.request_digest,
        product_id: String::from(context.product_id),
        model_id: String::from(context.model_id),
        served_artifact_digest: context
            .adapter_serving
            .map(|binding| binding.served_adapter_digest.clone())
            .unwrap_or_else(|| context.served_artifact.served_artifact_digest.clone()),
        execution_plan_digest: delivery_proof.execution_plan_digest.clone(),
        runtime_backend: String::from(context.runtime_backend),
        kernel_count: delivery_proof.kernel_count,
        bytes_moved: delivery_proof.bytes_moved,
        plan_cache_hits: delivery_proof.plan_cache_hits,
        plan_cache_misses: delivery_proof.plan_cache_misses,
        kv_growth: delivery_proof.kv_growth.clone(),
        output_tokens: context.output_tokens,
        cluster_provenance: context
            .cluster_execution
            .and_then(ClusterSettlementProvenanceInput::from_cluster_execution),
    }
}

fn cluster_evidence_bundle_status(status: ReceiptStatus) -> ClusterEvidenceBundleStatus {
    match status {
        ReceiptStatus::Succeeded => ClusterEvidenceBundleStatus::Succeeded,
        ReceiptStatus::Cancelled => ClusterEvidenceBundleStatus::Cancelled,
        ReceiptStatus::Disconnected => ClusterEvidenceBundleStatus::Disconnected,
        ReceiptStatus::Failed => ClusterEvidenceBundleStatus::Failed,
    }
}

fn execution_proof_bundle_status(status: ReceiptStatus) -> ExecutionProofBundleStatus {
    match status {
        ReceiptStatus::Succeeded => ExecutionProofBundleStatus::Succeeded,
        ReceiptStatus::Cancelled => ExecutionProofBundleStatus::Cancelled,
        ReceiptStatus::Disconnected => ExecutionProofBundleStatus::Disconnected,
        ReceiptStatus::Failed => ExecutionProofBundleStatus::Failed,
    }
}

fn execution_proof_runtime_identity(
    runtime_backend: &str,
    backend_toolchain: &BackendToolchainIdentity,
    selected_device_inventory: &Option<DeviceInventoryQualifiers>,
    selected_devices: &[DeviceInventoryQualifiers],
) -> ExecutionProofRuntimeIdentity {
    ExecutionProofRuntimeIdentity::new(runtime_backend, backend_toolchain.clone())
        .with_selected_device_inventory(selected_device_inventory.clone())
        .with_selected_devices(selected_devices.to_vec())
}

fn embedding_activation_fingerprint_artifact(
    request_digest: &str,
    runtime_backend: &str,
    product_id: &str,
    model_id: &str,
    response: &EmbeddingResponse,
) -> Option<psionic_runtime::ActivationFingerprintProofArtifact> {
    let input = response.embeddings.iter().fold(
        ActivationFingerprintInput::new(request_digest, product_id, model_id, runtime_backend),
        |input, embedding| {
            input.with_sample(ActivationFingerprintVectorSample::new(
                format!("embedding:{}", embedding.index),
                embedding.values.clone(),
            ))
        },
    );
    if input.samples.is_empty() {
        return None;
    }
    Some(QuantizedActivationFingerprintAdapter::default().generate(&input))
}

struct ClusterEvidenceBundleExportContext<'a> {
    product_id: &'a str,
    request_id: &'a str,
    request_digest: &'a str,
    model_id: &'a str,
    model_revision: &'a str,
    runtime_backend: &'a str,
    served_artifact: &'a ServedArtifactIdentity,
    weight_bundle: &'a WeightBundleEvidence,
    status: ReceiptStatus,
    cluster_execution: &'a Option<ClusterExecutionContext>,
    delivery_proof: &'a Option<ExecutionDeliveryProof>,
    settlement_linkage: &'a Option<SettlementLinkageInput>,
    proof_bundle: &'a Option<ExecutionProofBundle>,
    failure_reason: &'a Option<String>,
    diagnostic: &'a Option<LocalRuntimeDiagnostic>,
}

fn cluster_evidence_bundle_payload(
    context: ClusterEvidenceBundleExportContext<'_>,
) -> Option<ClusterEvidenceBundlePayload> {
    let mut payload = ClusterEvidenceBundlePayload::new(
        context.product_id,
        context.request_id,
        context.request_digest,
        context.model_id,
        context.model_revision,
        context.runtime_backend,
        context.served_artifact.served_artifact_digest.as_str(),
        context.weight_bundle.digest.as_str(),
        cluster_evidence_bundle_status(context.status),
        context.cluster_execution.clone()?,
    );
    if let Some(delivery_proof) = context.delivery_proof.clone() {
        payload = payload.with_delivery_proof(delivery_proof);
    }
    if let Some(settlement_linkage) = context.settlement_linkage.clone() {
        payload = payload.with_settlement_linkage(settlement_linkage);
    }
    if let Some(proof_bundle) = context.proof_bundle.as_ref() {
        payload = payload.with_proof_bundle_digest(proof_bundle.stable_digest());
    }
    if let Some(failure_reason) = context.failure_reason {
        payload = payload.with_failure_reason(failure_reason.clone());
    }
    if let Some(diagnostic) = context.diagnostic.clone() {
        payload = payload.with_diagnostic(diagnostic);
    }
    Some(payload)
}

#[allow(clippy::too_many_arguments)]
fn embedding_execution_proof_bundle(
    status: ReceiptStatus,
    request_id: &str,
    request_digest: &str,
    runtime_backend: &str,
    backend_toolchain: &BackendToolchainIdentity,
    selected_device_inventory: &Option<DeviceInventoryQualifiers>,
    selected_devices: &[DeviceInventoryQualifiers],
    validation: ValidationMatrixReference,
    model_id: &str,
    served_artifact: &ServedArtifactIdentity,
    execution_topology: &Option<ExecutionTopologyPlan>,
    cluster_execution: &Option<ClusterExecutionContext>,
    compile_path: &Option<CompilePathEvidence>,
    delivery_proof: &Option<ExecutionDeliveryProof>,
    settlement_linkage: &Option<SettlementLinkageInput>,
    activation_fingerprint: &Option<psionic_runtime::ActivationFingerprintProofArtifact>,
    failure_reason: &Option<String>,
    diagnostic: &Option<LocalRuntimeDiagnostic>,
) -> ExecutionProofBundle {
    let mut bundle = ExecutionProofBundle::new(
        if cluster_execution.is_some() {
            ExecutionProofBundleKind::Clustered
        } else {
            ExecutionProofBundleKind::Local
        },
        execution_proof_bundle_status(status),
        request_id,
        request_digest,
        EMBEDDINGS_PRODUCT_ID,
        execution_proof_runtime_identity(
            runtime_backend,
            backend_toolchain,
            selected_device_inventory,
            selected_devices,
        ),
    )
    .with_model_id(model_id)
    .with_validation(validation)
    .with_activation_fingerprint_posture(ExecutionProofAugmentationPosture::Supported)
    .with_served_artifact(served_artifact);
    if let Some(execution_topology) = execution_topology.as_ref() {
        bundle = bundle.with_execution_topology(execution_topology);
    }
    if let Some(cluster_execution) = cluster_execution.as_ref() {
        bundle = bundle.with_cluster_execution(cluster_execution);
    }
    if let Some(compile_path) = compile_path.clone() {
        bundle = bundle.with_compile_path(compile_path);
    }
    if let Some(delivery_proof) = delivery_proof.clone() {
        bundle = bundle.with_delivery_proof(delivery_proof);
    }
    if let Some(settlement_linkage) = settlement_linkage.clone() {
        bundle = bundle.with_settlement_linkage(settlement_linkage);
    }
    if let Some(activation_fingerprint) = activation_fingerprint.clone() {
        bundle = bundle.with_activation_fingerprint(activation_fingerprint);
    }
    if let Some(failure_reason) = failure_reason.clone() {
        bundle = bundle.with_failure_reason(failure_reason);
    }
    if let Some(diagnostic) = diagnostic.clone() {
        bundle = bundle.with_diagnostic(diagnostic);
    }
    bundle
}

#[allow(clippy::too_many_arguments)]
fn generation_execution_proof_bundle(
    status: ReceiptStatus,
    request_id: &str,
    request_digest: &str,
    runtime_backend: &str,
    backend_toolchain: &BackendToolchainIdentity,
    selected_device_inventory: &Option<DeviceInventoryQualifiers>,
    selected_devices: &[DeviceInventoryQualifiers],
    validation: ValidationMatrixReference,
    product_id: &str,
    model_id: &str,
    served_artifact: &ServedArtifactIdentity,
    execution_topology: &Option<ExecutionTopologyPlan>,
    cluster_execution: &Option<ClusterExecutionContext>,
    compile_path: &Option<CompilePathEvidence>,
    delivery_proof: &Option<ExecutionDeliveryProof>,
    settlement_linkage: &Option<SettlementLinkageInput>,
    execution_plan_digest: &Option<String>,
    failure_reason: &Option<String>,
    diagnostic: &Option<LocalRuntimeDiagnostic>,
) -> ExecutionProofBundle {
    let mut bundle = ExecutionProofBundle::new(
        if cluster_execution.is_some() {
            ExecutionProofBundleKind::Clustered
        } else {
            ExecutionProofBundleKind::Local
        },
        execution_proof_bundle_status(status),
        request_id,
        request_digest,
        product_id,
        execution_proof_runtime_identity(
            runtime_backend,
            backend_toolchain,
            selected_device_inventory,
            selected_devices,
        ),
    )
    .with_model_id(model_id)
    .with_validation(validation)
    .with_activation_fingerprint_posture(ExecutionProofAugmentationPosture::Unavailable)
    .with_served_artifact(served_artifact);
    if let Some(execution_topology) = execution_topology.as_ref() {
        bundle = bundle.with_execution_topology(execution_topology);
    }
    if let Some(cluster_execution) = cluster_execution.as_ref() {
        bundle = bundle.with_cluster_execution(cluster_execution);
    }
    if let Some(compile_path) = compile_path.clone() {
        bundle = bundle.with_compile_path(compile_path);
    }
    if let Some(execution_plan_digest) = execution_plan_digest.as_ref() {
        bundle = bundle.with_execution_plan_digest(execution_plan_digest.clone());
    }
    if let Some(delivery_proof) = delivery_proof.clone() {
        bundle = bundle.with_delivery_proof(delivery_proof);
    }
    if let Some(settlement_linkage) = settlement_linkage.clone() {
        bundle = bundle.with_settlement_linkage(settlement_linkage);
    }
    if let Some(failure_reason) = failure_reason.clone() {
        bundle = bundle.with_failure_reason(failure_reason);
    }
    if let Some(diagnostic) = diagnostic.clone() {
        bundle = bundle.with_diagnostic(diagnostic);
    }
    bundle
}

fn sandbox_execution_proof_bundle(
    status: ReceiptStatus,
    request_id: &str,
    request_digest: &str,
    runtime_backend: &str,
    backend_toolchain: &BackendToolchainIdentity,
    selected_device_inventory: &Option<DeviceInventoryQualifiers>,
    selected_devices: &[DeviceInventoryQualifiers],
    evidence: &SandboxExecutionEvidence,
    cluster_execution: &Option<ClusterExecutionContext>,
    failure_reason: &Option<String>,
    diagnostic: &Option<LocalRuntimeDiagnostic>,
) -> ExecutionProofBundle {
    let mut bundle = ExecutionProofBundle::new(
        if cluster_execution.is_some() {
            ExecutionProofBundleKind::Clustered
        } else {
            ExecutionProofBundleKind::Sandbox
        },
        execution_proof_bundle_status(status),
        request_id,
        request_digest,
        SANDBOX_EXECUTION_PRODUCT_ID,
        execution_proof_runtime_identity(
            runtime_backend,
            backend_toolchain,
            selected_device_inventory,
            selected_devices,
        ),
    )
    .with_activation_fingerprint_posture(ExecutionProofAugmentationPosture::Unavailable)
    .with_sandbox_evidence(evidence);
    if let Some(cluster_execution) = cluster_execution.as_ref() {
        bundle = bundle.with_cluster_execution(cluster_execution);
    }
    if let Some(failure_reason) = failure_reason.clone() {
        bundle = bundle.with_failure_reason(failure_reason);
    }
    if let Some(diagnostic) = diagnostic.clone() {
        bundle = bundle.with_diagnostic(diagnostic);
    }
    bundle
}

/// Explicit policy for whether one model artifact may be advertised or served into compute-market supply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeMarketSupplyPolicy {
    /// Whether Psionic-owned fixture models may be advertised.
    pub allow_fixture_models: bool,
    /// Whether direct caller-supplied local files may be advertised.
    pub allow_local_path_artifacts: bool,
    /// Whether raw Ollama blobs without a resolved manifest may be advertised.
    pub allow_unbound_ollama_blobs: bool,
    /// Whether resolved local Ollama manifests may be advertised.
    pub allow_ollama_manifests: bool,
    /// Whether resolved local Ollama remote aliases may be advertised.
    pub allow_ollama_remote_aliases: bool,
    /// Whether external artifacts must declare at least one license payload.
    pub require_declared_license_for_external_artifacts: bool,
    /// Optional exact-license allowlist by stable digest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_license_digests: Vec<String>,
    /// Explicit exact-license denylist by stable digest.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied_license_digests: Vec<String>,
}

/// Stable refusal code for one compute-market supply policy violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputeMarketSupplyViolationCode {
    /// External artifact governance facts were missing entirely.
    MissingArtifactGovernance,
    /// The artifact provenance class is not allowed by policy.
    DisallowedProvenance,
    /// The artifact did not declare any license payloads when policy required them.
    MissingDeclaredLicense,
    /// The artifact declared licenses, but none matched the configured allowlist.
    LicenseNotAllowlisted,
    /// One declared license matched the configured denylist.
    LicenseDenied,
}

/// One explicit compute-market supply policy violation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeMarketSupplyViolation {
    /// Stable refusal code.
    pub code: ComputeMarketSupplyViolationCode,
    /// Plain-language refusal detail.
    pub message: String,
}

impl ComputeMarketSupplyViolation {
    fn new(code: ComputeMarketSupplyViolationCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Machine-checkable advertise/serve decision for compute-market supply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeMarketSupplyDecision {
    /// Whether the artifact may be advertised into the compute market.
    pub advertise_allowed: bool,
    /// Whether the artifact may be served for compute-market work.
    pub serve_allowed: bool,
    /// Explicit policy violations that forced refusal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub violations: Vec<ComputeMarketSupplyViolation>,
}

impl ComputeMarketSupplyDecision {
    fn allowed() -> Self {
        Self {
            advertise_allowed: true,
            serve_allowed: true,
            violations: Vec::new(),
        }
    }

    fn refused(violations: Vec<ComputeMarketSupplyViolation>) -> Self {
        Self {
            advertise_allowed: false,
            serve_allowed: false,
            violations,
        }
    }

    /// Returns whether the artifact is admissible for both advertise and serve paths.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.advertise_allowed && self.serve_allowed
    }
}

/// Returns the default explicit compute-market supply policy for provider-advertised Psionic artifacts.
#[must_use]
pub fn default_compute_market_supply_policy() -> ComputeMarketSupplyPolicy {
    ComputeMarketSupplyPolicy {
        allow_fixture_models: true,
        allow_local_path_artifacts: false,
        allow_unbound_ollama_blobs: false,
        allow_ollama_manifests: true,
        allow_ollama_remote_aliases: true,
        require_declared_license_for_external_artifacts: true,
        allowed_license_digests: Vec::new(),
        denied_license_digests: Vec::new(),
    }
}

/// Evaluates compute-market supply policy for one model artifact.
#[must_use]
pub fn evaluate_compute_market_supply(
    weights: &WeightBundleMetadata,
    artifact_governance: Option<&ModelArtifactGovernance>,
    policy: &ComputeMarketSupplyPolicy,
) -> ComputeMarketSupplyDecision {
    if weights.source == WeightSource::Fixture {
        return if policy.allow_fixture_models {
            ComputeMarketSupplyDecision::allowed()
        } else {
            ComputeMarketSupplyDecision::refused(vec![ComputeMarketSupplyViolation::new(
                ComputeMarketSupplyViolationCode::DisallowedProvenance,
                "fixture models are not allowed for compute-market supply",
            )])
        };
    }

    let Some(artifact_governance) = artifact_governance else {
        return ComputeMarketSupplyDecision::refused(vec![ComputeMarketSupplyViolation::new(
            ComputeMarketSupplyViolationCode::MissingArtifactGovernance,
            "external artifact is missing provenance and license governance metadata",
        )]);
    };

    let mut violations = Vec::new();
    let provenance_allowed = match artifact_governance.provenance.kind {
        ModelArtifactProvenanceKind::Fixture => policy.allow_fixture_models,
        ModelArtifactProvenanceKind::LocalPath => policy.allow_local_path_artifacts,
        ModelArtifactProvenanceKind::OllamaBlob => policy.allow_unbound_ollama_blobs,
        ModelArtifactProvenanceKind::OllamaManifest => policy.allow_ollama_manifests,
        ModelArtifactProvenanceKind::OllamaRemoteAlias => policy.allow_ollama_remote_aliases,
    };
    if !provenance_allowed {
        violations.push(ComputeMarketSupplyViolation::new(
            ComputeMarketSupplyViolationCode::DisallowedProvenance,
            format!(
                "artifact provenance `{}` is not allowed by compute-market supply policy",
                provenance_kind_label(artifact_governance.provenance.kind)
            ),
        ));
    }

    let license_digests = artifact_governance.licenses.digests();
    if policy.require_declared_license_for_external_artifacts
        && !artifact_governance.licenses.declared
    {
        violations.push(ComputeMarketSupplyViolation::new(
            ComputeMarketSupplyViolationCode::MissingDeclaredLicense,
            "external artifact does not declare any license payloads",
        ));
    }
    if !policy.allowed_license_digests.is_empty()
        && !license_digests
            .iter()
            .any(|digest| policy.allowed_license_digests.contains(digest))
    {
        violations.push(ComputeMarketSupplyViolation::new(
            ComputeMarketSupplyViolationCode::LicenseNotAllowlisted,
            "declared licenses do not match the configured allowlist",
        ));
    }
    if let Some(digest) = license_digests
        .iter()
        .find(|digest| policy.denied_license_digests.contains(*digest))
    {
        violations.push(ComputeMarketSupplyViolation::new(
            ComputeMarketSupplyViolationCode::LicenseDenied,
            format!("declared license digest `{digest}` is explicitly denied"),
        ));
    }

    if violations.is_empty() {
        ComputeMarketSupplyDecision::allowed()
    } else {
        ComputeMarketSupplyDecision::refused(violations)
    }
}

/// Returns a structured refusal diagnostic when compute-market supply policy disallows an artifact.
pub fn compute_market_supply_refusal_diagnostic(
    product_id: &str,
    model_id: &str,
    runtime_backend: &str,
    decision: &ComputeMarketSupplyDecision,
) -> Option<LocalRuntimeDiagnostic> {
    if decision.is_allowed() {
        return None;
    }

    let detail = decision
        .violations
        .iter()
        .map(|violation| violation.message.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    Some(
        LocalRuntimeDiagnostic::new(
            psionic_runtime::LocalRuntimeErrorCode::AdmissionRefused,
            403,
            format!("compute-market supply policy refused artifact: {detail}"),
        )
        .with_product_id(product_id)
        .with_model_id(model_id)
        .with_backend(runtime_backend),
    )
}

/// AMD-specific provider truth derived from reusable runtime/backend state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdCapabilityContext {
    /// Active AMD mode.
    pub mode: AmdRuntimeMode,
    /// Stable AMD topology snapshot.
    pub topology: AmdTopologyInfo,
    /// Risk posture for the AMD mode.
    pub risk: AmdRiskProfile,
    /// Recovery posture for the AMD mode.
    pub recovery: AmdRecoveryProfile,
}

impl AmdCapabilityContext {
    /// Derives AMD capability context from a runtime backend selection.
    #[must_use]
    pub fn from_backend_selection(backend_selection: &BackendSelection) -> Option<Self> {
        backend_selection
            .selected_device
            .as_ref()
            .and_then(|device| device.amd_metadata.as_ref())
            .map(Self::from_metadata)
    }

    /// Derives AMD capability context directly from runtime device metadata.
    #[must_use]
    pub fn from_metadata(metadata: &AmdDeviceMetadata) -> Self {
        Self {
            mode: metadata.mode,
            topology: metadata.topology.clone(),
            risk: metadata.risk.clone(),
            recovery: metadata.recovery.clone(),
        }
    }
}

/// NVIDIA-specific provider truth derived from reusable runtime/backend state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NvidiaCapabilityContext {
    /// Stable NVIDIA topology snapshot.
    pub topology: NvidiaTopologyInfo,
    /// Risk posture for the CUDA device.
    pub risk: NvidiaRiskProfile,
    /// Recovery posture for the CUDA device.
    pub recovery: NvidiaRecoveryProfile,
}

impl NvidiaCapabilityContext {
    /// Derives NVIDIA capability context from a runtime backend selection.
    #[must_use]
    pub fn from_backend_selection(backend_selection: &BackendSelection) -> Option<Self> {
        backend_selection
            .selected_device
            .as_ref()
            .and_then(|device| device.nvidia_metadata.as_ref())
            .map(Self::from_metadata)
    }

    /// Derives NVIDIA capability context directly from runtime device metadata.
    #[must_use]
    pub fn from_metadata(metadata: &NvidiaDeviceMetadata) -> Self {
        Self {
            topology: metadata.topology.clone(),
            risk: metadata.risk.clone(),
            recovery: metadata.recovery.clone(),
        }
    }
}

/// Provider-facing wrapper for live local-runtime observability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRuntimeObservabilityEnvelope {
    /// Engine backend family.
    pub backend_family: String,
    /// Current local-runtime observability snapshot.
    pub observability: LocalRuntimeObservability,
}

impl LocalRuntimeObservabilityEnvelope {
    /// Creates a provider-facing wrapper from a reusable runtime snapshot.
    #[must_use]
    pub fn new(observability: LocalRuntimeObservability) -> Self {
        Self {
            backend_family: String::from(BACKEND_FAMILY),
            observability,
        }
    }
}

/// Capability envelope for a bounded sandbox-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionCapabilityEnvelope {
    /// Engine backend family.
    pub backend_family: String,
    /// Product identifier.
    pub product_id: String,
    /// Runtime backend such as `cpu` or `cuda`.
    pub runtime_backend: String,
    /// Explicit backend selection and fallback truth.
    pub backend_selection: BackendSelection,
    /// Explicit compile-vs-probe backend/toolchain truth for the selected path.
    pub backend_toolchain: BackendToolchainIdentity,
    /// Reusable selected-device inventory and performance qualifiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_device_inventory: Option<DeviceInventoryQualifiers>,
    /// All selected devices participating in the effective backend path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_devices: Vec<DeviceInventoryQualifiers>,
    /// Explicit multi-device or sharded execution topology when the path is planned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_topology: Option<ExecutionTopologyPlan>,
    /// Explicit clustered execution or scheduling context when this lane is advertised through a cluster.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution: Option<ClusterExecutionContext>,
    /// AMD-specific capability context when the selected backend is AMD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd: Option<AmdCapabilityContext>,
    /// NVIDIA-specific capability context when the selected backend is CUDA.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<NvidiaCapabilityContext>,
    /// Explicit bounded sandbox execution profile.
    pub execution_profile: SandboxExecutionCapabilityProfile,
    /// Current readiness state.
    pub readiness: ProviderReadiness,
}

impl SandboxExecutionCapabilityEnvelope {
    /// Creates a capability envelope for a bounded sandbox-execution lane.
    #[must_use]
    pub fn new(
        backend_selection: BackendSelection,
        execution_profile: SandboxExecutionCapabilityProfile,
        readiness: ProviderReadiness,
    ) -> Self {
        Self {
            backend_family: String::from(BACKEND_FAMILY),
            product_id: String::from(SANDBOX_EXECUTION_PRODUCT_ID),
            runtime_backend: backend_selection.effective_backend.clone(),
            backend_toolchain: backend_toolchain_identity_for_selection(&backend_selection),
            selected_device_inventory: selected_device_inventory_for_selection(&backend_selection),
            selected_devices: selected_devices_inventory_for_selection(&backend_selection),
            execution_topology: execution_topology_for_selection(&backend_selection),
            cluster_execution: None,
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            nvidia: NvidiaCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            execution_profile,
            readiness,
        }
    }

    /// Attaches explicit clustered execution or scheduling context.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: ClusterExecutionContext) -> Self {
        apply_cluster_execution_surface(
            &mut self.selected_device_inventory,
            &mut self.selected_devices,
            &mut self.execution_topology,
            &cluster_execution,
        );
        self.cluster_execution = Some(cluster_execution);
        self
    }

    /// Attaches advertised clustered-lane capability truth independently of realized execution.
    #[must_use]
    pub fn with_cluster_execution_capability_profile(
        mut self,
        cluster_execution_capability_profile: ClusterExecutionCapabilityProfile,
    ) -> Self {
        self.backend_selection = self
            .backend_selection
            .with_cluster_execution_capability_profile(cluster_execution_capability_profile);
        self
    }

    /// Attaches published cluster trust posture independently of realized execution.
    #[must_use]
    pub fn with_cluster_compute_market_trust_assessment(
        mut self,
        cluster_compute_market_trust_assessment: ClusterComputeMarketTrustAssessment,
    ) -> Self {
        self.backend_selection = self
            .backend_selection
            .with_cluster_compute_market_trust_assessment(cluster_compute_market_trust_assessment);
        self
    }
}

/// Provider-facing receipt for one bounded sandbox-execution request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionReceipt {
    /// Product identifier.
    pub product_id: String,
    /// Backend family.
    pub backend_family: String,
    /// Runtime backend.
    pub runtime_backend: String,
    /// Explicit backend selection and fallback truth.
    pub backend_selection: BackendSelection,
    /// Explicit compile-vs-probe backend/toolchain truth for the realized path.
    pub backend_toolchain: BackendToolchainIdentity,
    /// Reusable selected-device inventory and performance qualifiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_device_inventory: Option<DeviceInventoryQualifiers>,
    /// All selected devices participating in the effective backend path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_devices: Vec<DeviceInventoryQualifiers>,
    /// Explicit multi-device or sharded execution topology when the path is planned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_topology: Option<ExecutionTopologyPlan>,
    /// Explicit clustered execution or scheduling context for the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution: Option<ClusterExecutionContext>,
    /// AMD-specific execution context when the selected backend is AMD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd: Option<AmdCapabilityContext>,
    /// NVIDIA-specific execution context when the selected backend is CUDA.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<NvidiaCapabilityContext>,
    /// Stable request identifier.
    pub request_id: String,
    /// Stable request digest.
    pub request_digest: String,
    /// Runtime-owned sandbox evidence for the realized request path.
    pub evidence: SandboxExecutionEvidence,
    /// Canonical execution-proof bundle for the realized request path.
    pub proof_bundle: ExecutionProofBundle,
    /// Promised-vs-delivered accelerator comparison for accelerator-sensitive offers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accelerator_deliverability: Option<AcceleratorDeliverabilityReport>,
    /// Timestamp when execution started.
    pub started_at_unix_ms: u64,
    /// Timestamp when execution ended.
    pub ended_at_unix_ms: u64,
    /// Terminal receipt status.
    pub status: ReceiptStatus,
    /// Optional failure reason surfaced from the sandbox terminal state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    /// Structured local-runtime diagnostic for failed requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<LocalRuntimeDiagnostic>,
}

impl SandboxExecutionReceipt {
    /// Creates a receipt from explicit sandbox request identity and runtime evidence.
    #[must_use]
    pub fn from_evidence(
        backend_selection: BackendSelection,
        request: &SandboxExecutionRequestIdentity,
        evidence: SandboxExecutionEvidence,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
        diagnostic: Option<LocalRuntimeDiagnostic>,
    ) -> Self {
        let status = match evidence.exit.kind {
            SandboxExecutionExitKind::Succeeded => ReceiptStatus::Succeeded,
            SandboxExecutionExitKind::Cancelled => ReceiptStatus::Cancelled,
            SandboxExecutionExitKind::NonZeroExit
            | SandboxExecutionExitKind::TimedOut
            | SandboxExecutionExitKind::Killed
            | SandboxExecutionExitKind::RefusedByPolicy => ReceiptStatus::Failed,
        };
        let failure_reason =
            (status != ReceiptStatus::Succeeded).then(|| evidence.exit.detail.clone());
        let backend_toolchain = backend_toolchain_identity_for_selection(&backend_selection);
        let selected_device_inventory = selected_device_inventory_for_selection(&backend_selection);
        let selected_devices = selected_devices_inventory_for_selection(&backend_selection);
        let execution_topology = execution_topology_for_selection(&backend_selection);
        let proof_bundle = sandbox_execution_proof_bundle(
            status,
            request.request_id.as_str(),
            evidence.request_digest.as_str(),
            backend_selection.effective_backend.as_str(),
            &backend_toolchain,
            &selected_device_inventory,
            selected_devices.as_slice(),
            &evidence,
            &None,
            &failure_reason,
            &diagnostic,
        );

        Self {
            product_id: String::from(SANDBOX_EXECUTION_PRODUCT_ID),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: backend_selection.effective_backend.clone(),
            backend_toolchain,
            selected_device_inventory,
            selected_devices,
            execution_topology,
            cluster_execution: None,
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            nvidia: NvidiaCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            request_id: request.request_id.clone(),
            request_digest: evidence.request_digest.clone(),
            evidence,
            proof_bundle,
            accelerator_deliverability: None,
            started_at_unix_ms,
            ended_at_unix_ms,
            status,
            failure_reason,
            diagnostic,
        }
    }

    /// Attaches a promised-vs-delivered accelerator comparison for this receipt.
    #[must_use]
    pub fn with_accelerator_requirement(
        mut self,
        requirement: AcceleratorExecutionRequirement,
    ) -> Self {
        let delivered = self.cluster_execution.clone().map_or_else(
            || DeliveredExecutionContext::from_backend_selection(&self.backend_selection),
            |cluster_execution| {
                DeliveredExecutionContext::from_backend_selection(&self.backend_selection)
                    .with_cluster_execution(cluster_execution)
            },
        );
        self.accelerator_deliverability = Some(requirement.evaluate(delivered));
        self
    }

    /// Attaches explicit clustered execution or scheduling context.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: ClusterExecutionContext) -> Self {
        apply_cluster_execution_surface(
            &mut self.selected_device_inventory,
            &mut self.selected_devices,
            &mut self.execution_topology,
            &cluster_execution,
        );
        self.cluster_execution = Some(cluster_execution);
        self.proof_bundle = sandbox_execution_proof_bundle(
            self.status,
            self.request_id.as_str(),
            self.request_digest.as_str(),
            self.runtime_backend.as_str(),
            &self.backend_toolchain,
            &self.selected_device_inventory,
            self.selected_devices.as_slice(),
            &self.evidence,
            &self.cluster_execution,
            &self.failure_reason,
            &self.diagnostic,
        );
        self
    }
}

/// Capability envelope for a provider-advertised embeddings product.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEnvelope {
    /// Engine backend family.
    pub backend_family: String,
    /// Product identifier.
    pub product_id: String,
    /// Runtime backend such as `cpu`.
    pub runtime_backend: String,
    /// Minimum hardware validation claim backing the current support posture.
    pub validation: ValidationMatrixReference,
    /// Explicit backend selection and fallback truth.
    pub backend_selection: BackendSelection,
    /// Explicit compile-vs-probe backend/toolchain truth for the selected path.
    pub backend_toolchain: BackendToolchainIdentity,
    /// Reusable selected-device inventory and performance qualifiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_device_inventory: Option<DeviceInventoryQualifiers>,
    /// All selected devices participating in the effective backend path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_devices: Vec<DeviceInventoryQualifiers>,
    /// Explicit multi-device or sharded execution topology when the path is planned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_topology: Option<ExecutionTopologyPlan>,
    /// Explicit clustered execution or scheduling context when this lane is advertised through a cluster.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution: Option<ClusterExecutionContext>,
    /// AMD-specific capability context when the selected backend is AMD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd: Option<AmdCapabilityContext>,
    /// NVIDIA-specific capability context when the selected backend is CUDA.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<NvidiaCapabilityContext>,
    /// Model identifier.
    pub model_id: String,
    /// Model family.
    pub model_family: String,
    /// Model revision.
    pub model_revision: String,
    /// Weight bundle identity for the loaded model.
    pub weight_bundle: WeightBundleEvidence,
    /// Stable served-artifact identity for the active model/backend path.
    pub served_artifact: ServedArtifactIdentity,
    /// Stable provenance and license facts for the backing artifact when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_governance: Option<ModelArtifactGovernance>,
    /// Explicit compute-market supply policy applied to the artifact.
    pub supply_policy: ComputeMarketSupplyPolicy,
    /// Explicit advertise/serve decision under the current policy.
    pub supply_decision: ComputeMarketSupplyDecision,
    /// Explicit runtime-owned cache invalidation policy.
    pub cache_invalidation_policy: CacheInvalidationPolicy,
    /// Explicit batch, queueing, and throughput profile for the served path.
    pub execution_profile: ExecutionCapabilityProfile,
    /// Stable output dimensions.
    pub dimensions: usize,
    /// Normalization policy applied to returned vectors.
    pub normalization: EmbeddingNormalization,
    /// Whether output order matches input order.
    pub preserves_input_order: bool,
    /// Whether an empty input batch returns an empty successful response.
    pub empty_batch_returns_empty: bool,
    /// Whether callers may request truncated output dimensions.
    pub supports_output_dimensions: bool,
    /// Whether callers may request overflow truncation on long embedding inputs.
    pub supports_input_truncation: bool,
    /// Current readiness status.
    pub readiness: ProviderReadiness,
}

impl CapabilityEnvelope {
    /// Creates a capability envelope for an embeddings model.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn embeddings(
        backend_selection: BackendSelection,
        model_id: impl Into<String>,
        model_family: impl Into<String>,
        model_revision: impl Into<String>,
        weight_bundle: WeightBundleEvidence,
        served_artifact: ServedArtifactIdentity,
        artifact_governance: Option<ModelArtifactGovernance>,
        dimensions: usize,
        normalization: EmbeddingNormalization,
        readiness: ProviderReadiness,
    ) -> Self {
        let supply_policy = default_compute_market_supply_policy();
        let supply_decision = evaluate_compute_market_supply(
            &WeightBundleMetadata {
                format: weight_bundle.format,
                source: weight_bundle.source,
                quantization: weight_bundle.quantization,
                quantization_modes: weight_bundle.quantization_modes.clone(),
                digest: weight_bundle.digest.clone(),
                tensors: Vec::new(),
                artifacts: weight_bundle.artifacts.clone(),
            },
            artifact_governance.as_ref(),
            &supply_policy,
        );
        Self {
            backend_family: String::from(BACKEND_FAMILY),
            product_id: String::from(EMBEDDINGS_PRODUCT_ID),
            runtime_backend: backend_selection.effective_backend.clone(),
            validation: validation_reference_for_served_product(
                &backend_selection,
                EMBEDDINGS_PRODUCT_ID,
            ),
            backend_toolchain: backend_toolchain_identity_for_selection(&backend_selection),
            selected_device_inventory: selected_device_inventory_for_selection(&backend_selection),
            selected_devices: selected_devices_inventory_for_selection(&backend_selection),
            execution_topology: execution_topology_for_selection(&backend_selection),
            cluster_execution: None,
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            nvidia: NvidiaCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            model_id: model_id.into(),
            model_family: model_family.into(),
            model_revision: model_revision.into(),
            weight_bundle,
            served_artifact,
            artifact_governance,
            supply_policy,
            supply_decision,
            cache_invalidation_policy: cache_invalidation_policy(),
            execution_profile: default_embeddings_execution_profile(),
            dimensions,
            normalization,
            preserves_input_order: true,
            empty_batch_returns_empty: true,
            supports_output_dimensions: true,
            supports_input_truncation: false,
            readiness,
        }
    }

    /// Attaches explicit clustered execution or scheduling context.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: ClusterExecutionContext) -> Self {
        apply_cluster_execution_surface(
            &mut self.selected_device_inventory,
            &mut self.selected_devices,
            &mut self.execution_topology,
            &cluster_execution,
        );
        self.cluster_execution = Some(cluster_execution);
        self
    }

    /// Attaches advertised clustered-lane capability truth independently of realized execution.
    #[must_use]
    pub fn with_cluster_execution_capability_profile(
        mut self,
        cluster_execution_capability_profile: ClusterExecutionCapabilityProfile,
    ) -> Self {
        self.backend_selection = self
            .backend_selection
            .with_cluster_execution_capability_profile(cluster_execution_capability_profile);
        self
    }

    /// Attaches published cluster trust posture independently of realized execution.
    #[must_use]
    pub fn with_cluster_compute_market_trust_assessment(
        mut self,
        cluster_compute_market_trust_assessment: ClusterComputeMarketTrustAssessment,
    ) -> Self {
        self.backend_selection = self
            .backend_selection
            .with_cluster_compute_market_trust_assessment(cluster_compute_market_trust_assessment);
        self
    }

    /// Creates a capability envelope directly from an embeddings model descriptor.
    #[must_use]
    pub fn from_embedding_model(
        backend_selection: BackendSelection,
        model: &EmbeddingModelDescriptor,
        readiness: ProviderReadiness,
    ) -> Self {
        let served_artifact =
            served_artifact_identity_for_embedding_model(model, &backend_selection);
        Self::embeddings(
            backend_selection,
            model.model.model_id.clone(),
            model.model.family.clone(),
            model.model.revision.clone(),
            WeightBundleEvidence::from_metadata(&model.weights),
            served_artifact,
            model.artifact_governance.clone(),
            model.dimensions,
            model.normalization,
            readiness,
        )
    }
}

/// Provider readiness contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderReadiness {
    /// Current status.
    pub status: HealthStatus,
    /// Plain-text explanation.
    pub message: String,
}

impl ProviderReadiness {
    /// Creates a ready state.
    #[must_use]
    pub fn ready(message: impl Into<String>) -> Self {
        Self {
            status: HealthStatus::Ready,
            message: message.into(),
        }
    }
}

/// Terminal receipt status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptStatus {
    /// Execution completed successfully.
    Succeeded,
    /// Execution was cancelled by the caller.
    Cancelled,
    /// Execution aborted because the client disconnected mid-stream.
    Disconnected,
    /// Execution failed.
    Failed,
}

/// Execution receipt for an embeddings job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionReceipt {
    /// Product identifier.
    pub product_id: String,
    /// Backend family.
    pub backend_family: String,
    /// Runtime backend.
    pub runtime_backend: String,
    /// Minimum hardware validation claim backing the current support posture.
    pub validation: ValidationMatrixReference,
    /// Explicit backend selection and fallback truth.
    pub backend_selection: BackendSelection,
    /// Explicit compile-vs-probe backend/toolchain truth for the realized path.
    pub backend_toolchain: BackendToolchainIdentity,
    /// Reusable selected-device inventory and performance qualifiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_device_inventory: Option<DeviceInventoryQualifiers>,
    /// All selected devices participating in the effective backend path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_devices: Vec<DeviceInventoryQualifiers>,
    /// Explicit multi-device or sharded execution topology when the path is planned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_topology: Option<ExecutionTopologyPlan>,
    /// Explicit clustered execution or scheduling context for the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution: Option<ClusterExecutionContext>,
    /// AMD-specific execution context when the selected backend is AMD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd: Option<AmdCapabilityContext>,
    /// NVIDIA-specific execution context when the selected backend is CUDA.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<NvidiaCapabilityContext>,
    /// Request identifier.
    pub request_id: String,
    /// Stable request digest.
    pub request_digest: String,
    /// Model identifier.
    pub model_id: String,
    /// Model family.
    pub model_family: String,
    /// Model revision.
    pub model_revision: String,
    /// Weight bundle identity used during execution.
    pub weight_bundle: WeightBundleEvidence,
    /// Stable served-artifact identity for the realized model/backend path.
    pub served_artifact: ServedArtifactIdentity,
    /// Explicit cache actions surfaced for the request path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cache_observations: Vec<CacheObservation>,
    /// Explicit warm/cold compile-path evidence for the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_path: Option<CompilePathEvidence>,
    /// Delivery-proof facts surfaced by the local runtime for this request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_proof: Option<ExecutionDeliveryProof>,
    /// Settlement-linkage inputs derived from the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settlement_linkage: Option<SettlementLinkageInput>,
    /// Canonical execution-proof bundle for the realized request path.
    pub proof_bundle: ExecutionProofBundle,
    /// Output dimensions.
    pub output_dimensions: usize,
    /// Number of request inputs.
    pub input_count: usize,
    /// Number of returned vectors.
    pub output_vector_count: usize,
    /// Normalization policy applied to returned vectors.
    pub normalization: EmbeddingNormalization,
    /// Requested output dimensions when the caller asked for truncated vectors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_output_dimensions: Option<usize>,
    /// End-to-end embeddings duration in nanoseconds, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration_ns: Option<u64>,
    /// Model-load or compile duration attributable to this request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration_ns: Option<u64>,
    /// Timestamp when execution started.
    pub started_at_unix_ms: u64,
    /// Timestamp when execution ended.
    pub ended_at_unix_ms: u64,
    /// Terminal status.
    pub status: ReceiptStatus,
    /// Optional failure reason.
    pub failure_reason: Option<String>,
    /// Structured local-runtime diagnostic for failed requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<LocalRuntimeDiagnostic>,
}

impl ExecutionReceipt {
    /// Creates a success receipt from request/response contracts.
    #[must_use]
    pub fn succeeded(
        backend_selection: BackendSelection,
        request: &EmbeddingRequest,
        response: &EmbeddingResponse,
        request_digest: impl Into<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        let request_digest = request_digest.into();
        let served_artifact =
            served_artifact_identity_for_embedding_model(&request.model, &backend_selection);
        let delivery_proof = embedding_delivery_proof(response);
        let cluster_execution = embedding_cluster_execution(response);
        let mut selected_device_inventory =
            selected_device_inventory_for_selection(&backend_selection);
        let mut selected_devices = selected_devices_inventory_for_selection(&backend_selection);
        let mut execution_topology = execution_topology_for_selection(&backend_selection);
        if let Some(cluster_execution) = cluster_execution.as_ref() {
            apply_cluster_execution_surface(
                &mut selected_device_inventory,
                &mut selected_devices,
                &mut execution_topology,
                cluster_execution,
            );
        }
        let validation = validation_reference_for_served_product(
            &backend_selection,
            request.product_id.as_str(),
        );
        let backend_toolchain = backend_toolchain_identity_for_selection(&backend_selection);
        let settlement_linkage = delivery_proof.as_ref().map(|delivery_proof| {
            settlement_linkage(
                SettlementLinkageContext {
                    request_digest: request_digest.clone(),
                    product_id: request.product_id.as_str(),
                    model_id: response.metadata.model_id.as_str(),
                    served_artifact: &served_artifact,
                    adapter_serving: None,
                    runtime_backend: backend_selection.effective_backend.as_str(),
                    cluster_execution: cluster_execution.as_ref(),
                    output_tokens: None,
                },
                delivery_proof,
            )
        });
        let compile_path = response
            .provenance
            .as_ref()
            .and_then(|value| value.compile_path.clone());
        let activation_fingerprint = embedding_activation_fingerprint_artifact(
            request_digest.as_str(),
            backend_selection.effective_backend.as_str(),
            request.product_id.as_str(),
            response.metadata.model_id.as_str(),
            response,
        );
        let proof_bundle = embedding_execution_proof_bundle(
            ReceiptStatus::Succeeded,
            request.request_id.as_str(),
            request_digest.as_str(),
            backend_selection.effective_backend.as_str(),
            &backend_toolchain,
            &selected_device_inventory,
            selected_devices.as_slice(),
            validation.clone(),
            response.metadata.model_id.as_str(),
            &served_artifact,
            &execution_topology,
            &cluster_execution,
            &compile_path,
            &delivery_proof,
            &settlement_linkage,
            &activation_fingerprint,
            &None,
            &None,
        );
        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: backend_selection.effective_backend.clone(),
            validation,
            backend_toolchain,
            selected_device_inventory,
            selected_devices,
            execution_topology,
            cluster_execution,
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            nvidia: NvidiaCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            request_id: request.request_id.clone(),
            request_digest,
            model_id: response.metadata.model_id.clone(),
            model_family: request.model.model.family.clone(),
            model_revision: request.model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&request.model.weights),
            served_artifact,
            cache_observations: response
                .provenance
                .as_ref()
                .map(|value| value.cache_observations.clone())
                .unwrap_or_else(|| cache_observations_for_embedding_model(&request.model, None)),
            compile_path,
            delivery_proof,
            settlement_linkage,
            proof_bundle,
            output_dimensions: response.metadata.dimensions,
            input_count: response.metadata.input_count,
            output_vector_count: response.metadata.vector_count,
            normalization: response.metadata.normalization,
            requested_output_dimensions: response.metadata.requested_output_dimensions,
            total_duration_ns: response.metrics.total_duration_ns,
            load_duration_ns: response.metrics.load_duration_ns,
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Succeeded,
            failure_reason: None,
            diagnostic: None,
        }
    }

    /// Creates a success receipt and computes the request digest internally.
    #[must_use]
    pub fn succeeded_for_response(
        backend_selection: BackendSelection,
        request: &EmbeddingRequest,
        response: &EmbeddingResponse,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        Self::succeeded(
            backend_selection,
            request,
            response,
            digest_embedding_request(request),
            started_at_unix_ms,
            ended_at_unix_ms,
        )
    }

    /// Creates a failure receipt for a request that could not be executed.
    #[must_use]
    pub fn failed_for_request(
        backend_selection: BackendSelection,
        request: &EmbeddingRequest,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
        failure_reason: impl Into<String>,
    ) -> Self {
        let served_artifact =
            served_artifact_identity_for_embedding_model(&request.model, &backend_selection);
        let validation = validation_reference_for_served_product(
            &backend_selection,
            request.product_id.as_str(),
        );
        let backend_toolchain = backend_toolchain_identity_for_selection(&backend_selection);
        let selected_device_inventory = selected_device_inventory_for_selection(&backend_selection);
        let selected_devices = selected_devices_inventory_for_selection(&backend_selection);
        let execution_topology = execution_topology_for_selection(&backend_selection);
        let failure_reason = Some(failure_reason.into());
        let proof_bundle = embedding_execution_proof_bundle(
            ReceiptStatus::Failed,
            request.request_id.as_str(),
            digest_embedding_request(request).as_str(),
            backend_selection.effective_backend.as_str(),
            &backend_toolchain,
            &selected_device_inventory,
            selected_devices.as_slice(),
            validation.clone(),
            request.model.model.model_id.as_str(),
            &served_artifact,
            &execution_topology,
            &None,
            &None,
            &None,
            &None,
            &None,
            &failure_reason,
            &None,
        );
        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: backend_selection.effective_backend.clone(),
            validation,
            backend_toolchain,
            selected_device_inventory,
            selected_devices,
            execution_topology,
            cluster_execution: None,
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            nvidia: NvidiaCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            request_id: request.request_id.clone(),
            request_digest: digest_embedding_request(request),
            model_id: request.model.model.model_id.clone(),
            model_family: request.model.model.family.clone(),
            model_revision: request.model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&request.model.weights),
            served_artifact,
            cache_observations: cache_observations_for_embedding_model(&request.model, None),
            compile_path: None,
            delivery_proof: None,
            settlement_linkage: None,
            proof_bundle,
            output_dimensions: request
                .output_dimensions
                .filter(|dimensions| *dimensions > 0 && *dimensions < request.model.dimensions)
                .unwrap_or(request.model.dimensions),
            input_count: request.inputs.len(),
            output_vector_count: 0,
            normalization: request.model.normalization,
            requested_output_dimensions: request
                .output_dimensions
                .filter(|dimensions| *dimensions > 0 && *dimensions < request.model.dimensions),
            total_duration_ns: None,
            load_duration_ns: None,
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Failed,
            failure_reason,
            diagnostic: None,
        }
    }

    /// Attaches a structured diagnostic, preserving the plain-text failure reason.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: LocalRuntimeDiagnostic) -> Self {
        if self.failure_reason.is_none() {
            self.failure_reason = Some(diagnostic.message.clone());
        }
        self.proof_bundle = self.proof_bundle.with_diagnostic(diagnostic.clone());
        self.diagnostic = Some(diagnostic);
        self
    }

    /// Attaches explicit clustered execution or scheduling context.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: ClusterExecutionContext) -> Self {
        apply_cluster_execution_surface(
            &mut self.selected_device_inventory,
            &mut self.selected_devices,
            &mut self.execution_topology,
            &cluster_execution,
        );
        self.cluster_execution = Some(cluster_execution);
        if let Some(cluster_execution) = self.cluster_execution.as_ref() {
            self.proof_bundle = self.proof_bundle.with_cluster_execution(cluster_execution);
        }
        self
    }

    /// Builds a signed-export payload for this clustered receipt when cluster evidence exists.
    #[must_use]
    pub fn cluster_evidence_bundle_payload(&self) -> Option<ClusterEvidenceBundlePayload> {
        cluster_evidence_bundle_payload(ClusterEvidenceBundleExportContext {
            product_id: self.product_id.as_str(),
            request_id: self.request_id.as_str(),
            request_digest: self.request_digest.as_str(),
            model_id: self.model_id.as_str(),
            model_revision: self.model_revision.as_str(),
            runtime_backend: self.runtime_backend.as_str(),
            served_artifact: &self.served_artifact,
            weight_bundle: &self.weight_bundle,
            status: self.status,
            cluster_execution: &self.cluster_execution,
            delivery_proof: &self.delivery_proof,
            settlement_linkage: &self.settlement_linkage,
            proof_bundle: &Some(self.proof_bundle.clone()),
            failure_reason: &self.failure_reason,
            diagnostic: &self.diagnostic,
        })
    }

    /// Signs the current clustered receipt export when cluster evidence exists.
    #[must_use]
    pub fn signed_cluster_evidence_bundle(
        &self,
        signer_node_id: impl Into<String>,
        signing_key: &SigningKey,
    ) -> Option<SignedClusterEvidenceBundle> {
        self.cluster_evidence_bundle_payload()
            .map(|payload| payload.sign(signer_node_id, signing_key))
    }
}

/// KV-cache mode exposed to provider capability consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvCacheMode {
    /// In-memory per-session KV cache.
    InMemory,
    /// Explicit paged KV cache.
    Paged,
    /// Future tiered/offloaded KV cache.
    Tiered,
}

/// Capability envelope for a text-generation provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextGenerationCapabilityEnvelope {
    /// Engine backend family.
    pub backend_family: String,
    /// Product identifier.
    pub product_id: String,
    /// Runtime backend such as `cpu`.
    pub runtime_backend: String,
    /// Minimum hardware validation claim backing the current support posture.
    pub validation: ValidationMatrixReference,
    /// Explicit backend selection and fallback truth.
    pub backend_selection: BackendSelection,
    /// Explicit compile-vs-probe backend/toolchain truth for the selected path.
    pub backend_toolchain: BackendToolchainIdentity,
    /// Reusable selected-device inventory and performance qualifiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_device_inventory: Option<DeviceInventoryQualifiers>,
    /// All selected devices participating in the effective backend path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_devices: Vec<DeviceInventoryQualifiers>,
    /// Explicit multi-device or sharded execution topology when the path is planned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_topology: Option<ExecutionTopologyPlan>,
    /// Explicit clustered execution or scheduling context when this lane is advertised through a cluster.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution: Option<ClusterExecutionContext>,
    /// AMD-specific capability context when the selected backend is AMD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd: Option<AmdCapabilityContext>,
    /// NVIDIA-specific capability context when the selected backend is CUDA.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<NvidiaCapabilityContext>,
    /// Model identifier.
    pub model_id: String,
    /// Model family.
    pub model_family: String,
    /// Model revision.
    pub model_revision: String,
    /// Weight bundle identity for the loaded model.
    pub weight_bundle: WeightBundleEvidence,
    /// Stable served-artifact identity for the active model/backend path.
    pub served_artifact: ServedArtifactIdentity,
    /// Stable provenance and license facts for the backing artifact when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_governance: Option<ModelArtifactGovernance>,
    /// Explicit compute-market supply policy applied to the artifact.
    pub supply_policy: ComputeMarketSupplyPolicy,
    /// Explicit advertise/serve decision under the current policy.
    pub supply_decision: ComputeMarketSupplyDecision,
    /// Explicit runtime-owned cache invalidation policy.
    pub cache_invalidation_policy: CacheInvalidationPolicy,
    /// Maximum supported context length.
    pub max_context: usize,
    /// Explicit resident-memory plan for the loaded model.
    pub memory_plan: ModelMemoryPlan,
    /// Active local-serving residency policy for the served model set.
    pub residency_policy: ModelResidencyPolicy,
    /// Explicit streaming policy for the local runtime API.
    pub streaming_policy: GenerationStreamingPolicy,
    /// Advertised KV cache posture.
    pub kv_cache_mode: KvCacheMode,
    /// Explicit paged-KV policy when the served path uses paged KV state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_policy: Option<KvCachePolicy>,
    /// Explicit KV-cache encoding policy when the served path uses KV state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_encoding_policy: Option<KvCacheEncodingPolicy>,
    /// Declared KV-cache encoding capabilities for the served path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_kv_cache_encoding_policies: Vec<KvCacheEncodingPolicy>,
    /// Explicit shared prompt-prefix reuse policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_policy: Option<PrefixCacheReusePolicy>,
    /// Explicit batch, queueing, and throughput profile for the served path.
    pub execution_profile: ExecutionCapabilityProfile,
    /// Current readiness state.
    pub readiness: ProviderReadiness,
}

impl TextGenerationCapabilityEnvelope {
    /// Creates a capability envelope from a decoder model descriptor.
    #[must_use]
    pub fn from_decoder_model(
        backend_selection: BackendSelection,
        model: &DecoderModelDescriptor,
        memory_plan: ModelMemoryPlan,
        residency_policy: ModelResidencyPolicy,
        kv_cache_mode: KvCacheMode,
        execution_profile: ExecutionCapabilityProfile,
        readiness: ProviderReadiness,
    ) -> Self {
        let served_artifact = served_artifact_identity_for_decoder_model(model, &backend_selection);
        let supply_policy = default_compute_market_supply_policy();
        let supply_decision = evaluate_compute_market_supply(
            &model.weights,
            model.artifact_governance.as_ref(),
            &supply_policy,
        );
        let runtime_backend = backend_selection.effective_backend.clone();
        Self {
            backend_family: String::from(BACKEND_FAMILY),
            product_id: String::from(TEXT_GENERATION_PRODUCT_ID),
            runtime_backend: runtime_backend.clone(),
            validation: validation_reference_for_text_generation_model(
                &backend_selection,
                &model.model.family,
            ),
            backend_toolchain: backend_toolchain_identity_for_selection(&backend_selection),
            selected_device_inventory: selected_device_inventory_for_selection(&backend_selection),
            selected_devices: selected_devices_inventory_for_selection(&backend_selection),
            execution_topology: execution_topology_for_selection(&backend_selection),
            cluster_execution: None,
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            nvidia: NvidiaCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            model_id: model.model.model_id.clone(),
            model_family: model.model.family.clone(),
            model_revision: model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&model.weights),
            served_artifact,
            artifact_governance: model.artifact_governance.clone(),
            supply_policy,
            supply_decision,
            cache_invalidation_policy: cache_invalidation_policy(),
            max_context: model.config.max_context,
            memory_plan,
            residency_policy,
            streaming_policy: psionic_serve::default_generation_streaming_policy(),
            kv_cache_policy: (kv_cache_mode == KvCacheMode::Paged)
                .then(|| default_decoder_kv_cache_policy(model)),
            kv_cache_encoding_policy: (kv_cache_mode == KvCacheMode::Paged)
                .then(|| default_decoder_kv_cache_encoding_policy(model, runtime_backend.as_str())),
            supported_kv_cache_encoding_policies: (kv_cache_mode == KvCacheMode::Paged)
                .then(|| {
                    supported_decoder_kv_cache_encoding_policies(model, runtime_backend.as_str())
                })
                .unwrap_or_default(),
            prefix_cache_policy: Some(default_prefix_cache_policy()),
            kv_cache_mode,
            execution_profile,
            readiness,
        }
    }

    /// Attaches explicit clustered execution or scheduling context.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: ClusterExecutionContext) -> Self {
        apply_cluster_execution_surface(
            &mut self.selected_device_inventory,
            &mut self.selected_devices,
            &mut self.execution_topology,
            &cluster_execution,
        );
        self.cluster_execution = Some(cluster_execution);
        self
    }

    /// Attaches advertised clustered-lane capability truth independently of realized execution.
    #[must_use]
    pub fn with_cluster_execution_capability_profile(
        mut self,
        cluster_execution_capability_profile: ClusterExecutionCapabilityProfile,
    ) -> Self {
        self.backend_selection = self
            .backend_selection
            .with_cluster_execution_capability_profile(cluster_execution_capability_profile);
        self
    }

    /// Attaches published cluster trust posture independently of realized execution.
    #[must_use]
    pub fn with_cluster_compute_market_trust_assessment(
        mut self,
        cluster_compute_market_trust_assessment: ClusterComputeMarketTrustAssessment,
    ) -> Self {
        self.backend_selection = self
            .backend_selection
            .with_cluster_compute_market_trust_assessment(cluster_compute_market_trust_assessment);
        self
    }
}

/// Execution receipt for a text-generation job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextGenerationReceipt {
    /// Product identifier.
    pub product_id: String,
    /// Backend family.
    pub backend_family: String,
    /// Runtime backend.
    pub runtime_backend: String,
    /// Minimum hardware validation claim backing the current support posture.
    pub validation: ValidationMatrixReference,
    /// Explicit backend selection and fallback truth.
    pub backend_selection: BackendSelection,
    /// Explicit compile-vs-probe backend/toolchain truth for the realized path.
    pub backend_toolchain: BackendToolchainIdentity,
    /// Reusable selected-device inventory and performance qualifiers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_device_inventory: Option<DeviceInventoryQualifiers>,
    /// All selected devices participating in the effective backend path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_devices: Vec<DeviceInventoryQualifiers>,
    /// Explicit multi-device or sharded execution topology when the path is planned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_topology: Option<ExecutionTopologyPlan>,
    /// Explicit clustered execution or scheduling context for the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cluster_execution: Option<ClusterExecutionContext>,
    /// AMD-specific execution context when the selected backend is AMD.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd: Option<AmdCapabilityContext>,
    /// NVIDIA-specific execution context when the selected backend is CUDA.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<NvidiaCapabilityContext>,
    /// Request identifier.
    pub request_id: String,
    /// Stable request digest.
    pub request_digest: String,
    /// Optional execution-plan digest.
    pub execution_plan_digest: Option<String>,
    /// Model identifier.
    pub model_id: String,
    /// Model family.
    pub model_family: String,
    /// Model revision.
    pub model_revision: String,
    /// Weight bundle identity used during execution.
    pub weight_bundle: WeightBundleEvidence,
    /// Stable served-artifact identity for the realized model/backend path.
    pub served_artifact: ServedArtifactIdentity,
    /// Explicit adapter-serving binding when the request targeted a hosted adapter product.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_serving: Option<AdapterServingBinding>,
    /// Shared Psion served-evidence bundle when the response belongs to the learned-model lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psion_served_evidence: Option<PsionServedEvidenceBundle>,
    /// Shared Psion served-output claim posture when the response belongs to the learned-model lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub psion_served_output_claim_posture: Option<PsionServedOutputClaimPosture>,
    /// Explicit cache actions surfaced for the request path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cache_observations: Vec<CacheObservation>,
    /// Explicit warm/cold compile-path evidence for the realized request path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_path: Option<CompilePathEvidence>,
    /// Delivery-proof facts surfaced by the local runtime for this request path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_proof: Option<ExecutionDeliveryProof>,
    /// Settlement-linkage inputs derived from the realized request path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settlement_linkage: Option<SettlementLinkageInput>,
    /// Canonical execution-proof bundle for the realized request path.
    pub proof_bundle: ExecutionProofBundle,
    /// Explicit resident-memory plan for the loaded model, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_plan: Option<ModelMemoryPlan>,
    /// Active local-serving residency policy, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub residency_policy: Option<ModelResidencyPolicy>,
    /// Aggregate resident-memory snapshot for the loaded-model set, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub residency_snapshot: Option<MemoryResidencySnapshot>,
    /// Streaming policy for the local runtime API, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming_policy: Option<GenerationStreamingPolicy>,
    /// Optional bound session identifier.
    pub session_id: Option<SessionId>,
    /// Prompt token count.
    pub input_tokens: usize,
    /// Output token count.
    pub output_tokens: usize,
    /// Cached token count after execution.
    pub cache_tokens: usize,
    /// End-to-end generation duration in nanoseconds, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_duration_ns: Option<u64>,
    /// Model-load or compile duration attributable to this request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_duration_ns: Option<u64>,
    /// Prompt-evaluation duration in nanoseconds, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_eval_duration_ns: Option<u64>,
    /// Output-generation duration in nanoseconds, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_duration_ns: Option<u64>,
    /// Whether the request took a warm or cold model path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_state: Option<GenerationLoadState>,
    /// Explicit paged-KV policy for the request path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_policy: Option<KvCachePolicy>,
    /// Explicit KV-cache encoding policy for the request path, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_encoding_policy: Option<KvCacheEncodingPolicy>,
    /// Explicit paged-KV accounting for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache: Option<KvCacheAccounting>,
    /// Explicit KV-cache encoding accounting for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_cache_encoding: Option<KvCacheEncodingAccounting>,
    /// Explicit hierarchical KV residency accounting for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_residency: Option<KvResidencyAccounting>,
    /// Shared prefix-cache state for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_state: Option<PrefixCacheState>,
    /// Shared prefix-cache reuse policy for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_policy: Option<PrefixCacheReusePolicy>,
    /// Shared prefix-cache identity for the request, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_cache_identity: Option<PrefixCacheIdentity>,
    /// Number of prompt-prefix tokens reused from the shared prefix cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_tokens_reused: Option<usize>,
    /// Terminal termination reason when execution succeeded.
    pub termination: Option<TerminationReason>,
    /// Timestamp when execution started.
    pub started_at_unix_ms: u64,
    /// Timestamp when execution ended.
    pub ended_at_unix_ms: u64,
    /// Terminal status.
    pub status: ReceiptStatus,
    /// Optional failure reason.
    pub failure_reason: Option<String>,
    /// Structured local-runtime diagnostic for failed requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<LocalRuntimeDiagnostic>,
}

impl TextGenerationReceipt {
    /// Creates a success receipt from request/response contracts.
    #[must_use]
    pub fn succeeded(
        backend_selection: BackendSelection,
        request: &GenerationRequest,
        response: &GenerationResponse,
        request_digest: impl Into<String>,
        execution_plan_digest: impl Into<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        let request_digest = request_digest.into();
        let execution_plan_digest = response
            .provenance
            .as_ref()
            .map(|value| value.execution_plan_digest.clone())
            .unwrap_or_else(|| execution_plan_digest.into());
        let served_artifact = response
            .provenance
            .as_ref()
            .map(|value| value.served_artifact.clone())
            .unwrap_or_else(|| {
                served_artifact_identity_for_decoder_model(&request.model, &backend_selection)
            });
        let delivery_proof = generation_delivery_proof(response);
        let cluster_execution = generation_cluster_execution(response);
        let adapter_serving = generation_adapter_serving(response);
        let mut selected_device_inventory =
            selected_device_inventory_for_selection(&backend_selection);
        let mut selected_devices = selected_devices_inventory_for_selection(&backend_selection);
        let mut execution_topology = execution_topology_for_selection(&backend_selection);
        if let Some(cluster_execution) = cluster_execution.as_ref() {
            apply_cluster_execution_surface(
                &mut selected_device_inventory,
                &mut selected_devices,
                &mut execution_topology,
                cluster_execution,
            );
        }
        let validation = validation_reference_for_text_generation_model(
            &backend_selection,
            &request.model.model.family,
        );
        let backend_toolchain = backend_toolchain_identity_for_selection(&backend_selection);
        let settlement_linkage = delivery_proof.as_ref().map(|delivery_proof| {
            settlement_linkage(
                SettlementLinkageContext {
                    request_digest: request_digest.clone(),
                    product_id: request.product_id.as_str(),
                    model_id: response.model_id.as_str(),
                    served_artifact: &served_artifact,
                    adapter_serving: adapter_serving.as_ref(),
                    runtime_backend: backend_selection.effective_backend.as_str(),
                    cluster_execution: cluster_execution.as_ref(),
                    output_tokens: Some(response.usage.output_tokens),
                },
                delivery_proof,
            )
        });
        let compile_path = response
            .provenance
            .as_ref()
            .and_then(|value| value.compile_path.clone());
        let execution_plan_digest = Some(execution_plan_digest);
        let proof_bundle = generation_execution_proof_bundle(
            ReceiptStatus::Succeeded,
            request.request_id.as_str(),
            request_digest.as_str(),
            backend_selection.effective_backend.as_str(),
            &backend_toolchain,
            &selected_device_inventory,
            selected_devices.as_slice(),
            validation.clone(),
            request.product_id.as_str(),
            response.model_id.as_str(),
            &served_artifact,
            &execution_topology,
            &cluster_execution,
            &compile_path,
            &delivery_proof,
            &settlement_linkage,
            &execution_plan_digest,
            &None,
            &None,
        );
        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: backend_selection.effective_backend.clone(),
            validation,
            backend_toolchain,
            selected_device_inventory,
            selected_devices,
            execution_topology,
            cluster_execution,
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            nvidia: NvidiaCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            request_id: request.request_id.clone(),
            request_digest,
            execution_plan_digest,
            model_id: response.model_id.clone(),
            model_family: request.model.model.family.clone(),
            model_revision: request.model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&request.model.weights),
            served_artifact,
            adapter_serving,
            psion_served_evidence: response
                .provenance
                .as_ref()
                .and_then(|value| value.psion_served_evidence.clone()),
            psion_served_output_claim_posture: response
                .provenance
                .as_ref()
                .and_then(|value| value.psion_served_output_claim_posture.clone()),
            cache_observations: response
                .provenance
                .as_ref()
                .map(|value| value.cache_observations.clone())
                .unwrap_or_default(),
            compile_path,
            delivery_proof,
            settlement_linkage,
            proof_bundle,
            memory_plan: response
                .provenance
                .as_ref()
                .and_then(|value| value.memory_plan.clone()),
            residency_policy: response
                .provenance
                .as_ref()
                .and_then(|value| value.residency_policy.clone()),
            residency_snapshot: response
                .provenance
                .as_ref()
                .and_then(|value| value.residency_snapshot.clone()),
            streaming_policy: response
                .provenance
                .as_ref()
                .and_then(|value| value.streaming_policy.clone()),
            session_id: response.session_id.clone(),
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            cache_tokens: response.usage.cache_tokens,
            total_duration_ns: response.metrics.total_duration_ns,
            load_duration_ns: response.metrics.load_duration_ns,
            prompt_eval_duration_ns: response.metrics.prompt_eval_duration_ns,
            eval_duration_ns: response.metrics.eval_duration_ns,
            load_state: response.provenance.as_ref().map(|value| value.load_state),
            kv_cache_policy: response
                .provenance
                .as_ref()
                .and_then(|value| value.kv_cache_policy.clone()),
            kv_cache_encoding_policy: response
                .provenance
                .as_ref()
                .and_then(|value| value.kv_cache_encoding_policy.clone()),
            kv_cache: response.metrics.kv_cache.clone(),
            kv_cache_encoding: response.metrics.kv_cache_encoding.clone(),
            kv_residency: response.metrics.kv_residency.clone(),
            prefix_cache_state: response
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_state),
            prefix_cache_policy: response
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_policy.clone()),
            prefix_cache_identity: response
                .provenance
                .as_ref()
                .and_then(|value| value.prefix_cache_identity.clone()),
            prefix_tokens_reused: response.metrics.prefix_tokens_reused,
            termination: Some(response.termination),
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Succeeded,
            failure_reason: None,
            diagnostic: None,
        }
    }

    /// Creates a success receipt and computes the request digest internally.
    #[must_use]
    pub fn succeeded_for_response(
        backend_selection: BackendSelection,
        request: &GenerationRequest,
        response: &GenerationResponse,
        execution_plan_digest: impl Into<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        Self::succeeded(
            backend_selection,
            request,
            response,
            digest_generation_request(request),
            execution_plan_digest,
            started_at_unix_ms,
            ended_at_unix_ms,
        )
    }

    /// Creates a failure receipt for a request that could not be executed.
    #[must_use]
    pub fn failed_for_request(
        backend_selection: BackendSelection,
        request: &GenerationRequest,
        execution_plan_digest: Option<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
        failure_reason: impl Into<String>,
    ) -> Self {
        let input_tokens = match &request.prompt {
            GenerationInput::Text(text) => text.split_whitespace().count(),
            GenerationInput::Tokens(tokens) => tokens.len(),
        };
        let served_artifact =
            served_artifact_identity_for_decoder_model(&request.model, &backend_selection);
        let validation = validation_reference_for_text_generation_model(
            &backend_selection,
            &request.model.model.family,
        );
        let backend_toolchain = backend_toolchain_identity_for_selection(&backend_selection);
        let selected_device_inventory = selected_device_inventory_for_selection(&backend_selection);
        let selected_devices = selected_devices_inventory_for_selection(&backend_selection);
        let execution_topology = execution_topology_for_selection(&backend_selection);
        let request_digest = digest_generation_request(request);
        let execution_plan_digest = execution_plan_digest;
        let failure_reason = Some(failure_reason.into());
        let proof_bundle = generation_execution_proof_bundle(
            ReceiptStatus::Failed,
            request.request_id.as_str(),
            request_digest.as_str(),
            backend_selection.effective_backend.as_str(),
            &backend_toolchain,
            &selected_device_inventory,
            selected_devices.as_slice(),
            validation.clone(),
            request.product_id.as_str(),
            request.model.model.model_id.as_str(),
            &served_artifact,
            &execution_topology,
            &None,
            &None,
            &None,
            &None,
            &execution_plan_digest,
            &failure_reason,
            &None,
        );

        Self {
            product_id: request.product_id.clone(),
            backend_family: String::from(BACKEND_FAMILY),
            runtime_backend: backend_selection.effective_backend.clone(),
            validation,
            backend_toolchain,
            selected_device_inventory,
            selected_devices,
            execution_topology,
            cluster_execution: None,
            amd: AmdCapabilityContext::from_backend_selection(&backend_selection),
            nvidia: NvidiaCapabilityContext::from_backend_selection(&backend_selection),
            backend_selection,
            request_id: request.request_id.clone(),
            request_digest,
            execution_plan_digest,
            model_id: request.model.model.model_id.clone(),
            model_family: request.model.model.family.clone(),
            model_revision: request.model.model.revision.clone(),
            weight_bundle: WeightBundleEvidence::from_metadata(&request.model.weights),
            served_artifact,
            adapter_serving: request.adapter_serving.clone(),
            psion_served_evidence: None,
            psion_served_output_claim_posture: None,
            cache_observations: failed_generation_cache_observations(request),
            compile_path: None,
            delivery_proof: None,
            settlement_linkage: None,
            proof_bundle,
            memory_plan: None,
            residency_policy: None,
            residency_snapshot: None,
            streaming_policy: None,
            session_id: request.session_id.clone(),
            input_tokens,
            output_tokens: 0,
            cache_tokens: 0,
            total_duration_ns: None,
            load_duration_ns: None,
            prompt_eval_duration_ns: None,
            eval_duration_ns: None,
            load_state: None,
            kv_cache_policy: None,
            kv_cache_encoding_policy: None,
            kv_cache: None,
            kv_cache_encoding: None,
            kv_residency: None,
            prefix_cache_state: None,
            prefix_cache_policy: None,
            prefix_cache_identity: None,
            prefix_tokens_reused: None,
            termination: None,
            started_at_unix_ms,
            ended_at_unix_ms,
            status: ReceiptStatus::Failed,
            failure_reason,
            diagnostic: None,
        }
    }

    /// Attaches a structured diagnostic, preserving the plain-text failure reason.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: LocalRuntimeDiagnostic) -> Self {
        if self.failure_reason.is_none() {
            self.failure_reason = Some(diagnostic.message.clone());
        }
        self.proof_bundle = self.proof_bundle.with_diagnostic(diagnostic.clone());
        self.diagnostic = Some(diagnostic);
        self
    }

    /// Attaches explicit clustered execution or scheduling context.
    #[must_use]
    pub fn with_cluster_execution(mut self, cluster_execution: ClusterExecutionContext) -> Self {
        apply_cluster_execution_surface(
            &mut self.selected_device_inventory,
            &mut self.selected_devices,
            &mut self.execution_topology,
            &cluster_execution,
        );
        self.cluster_execution = Some(cluster_execution);
        if let Some(cluster_execution) = self.cluster_execution.as_ref() {
            self.proof_bundle = self.proof_bundle.with_cluster_execution(cluster_execution);
        }
        self
    }

    /// Builds a signed-export payload for this clustered receipt when cluster evidence exists.
    #[must_use]
    pub fn cluster_evidence_bundle_payload(&self) -> Option<ClusterEvidenceBundlePayload> {
        cluster_evidence_bundle_payload(ClusterEvidenceBundleExportContext {
            product_id: self.product_id.as_str(),
            request_id: self.request_id.as_str(),
            request_digest: self.request_digest.as_str(),
            model_id: self.model_id.as_str(),
            model_revision: self.model_revision.as_str(),
            runtime_backend: self.runtime_backend.as_str(),
            served_artifact: &self.served_artifact,
            weight_bundle: &self.weight_bundle,
            status: self.status,
            cluster_execution: &self.cluster_execution,
            delivery_proof: &self.delivery_proof,
            settlement_linkage: &self.settlement_linkage,
            proof_bundle: &Some(self.proof_bundle.clone()),
            failure_reason: &self.failure_reason,
            diagnostic: &self.diagnostic,
        })
    }

    /// Signs the current clustered receipt export when cluster evidence exists.
    #[must_use]
    pub fn signed_cluster_evidence_bundle(
        &self,
        signer_node_id: impl Into<String>,
        signing_key: &SigningKey,
    ) -> Option<SignedClusterEvidenceBundle> {
        self.cluster_evidence_bundle_payload()
            .map(|payload| payload.sign(signer_node_id, signing_key))
    }

    /// Creates a receipt from a terminal streaming event, preserving partial output when present.
    #[must_use]
    pub fn from_stream_terminal(
        backend_selection: BackendSelection,
        request: &GenerationRequest,
        terminal: &GenerationStreamTerminal,
        execution_plan_digest: impl Into<String>,
        started_at_unix_ms: u64,
        ended_at_unix_ms: u64,
    ) -> Self {
        let mut receipt = Self::succeeded(
            backend_selection,
            request,
            &terminal.response,
            digest_generation_request(request),
            execution_plan_digest,
            started_at_unix_ms,
            ended_at_unix_ms,
        );
        match terminal.status {
            GenerationStreamStatus::Succeeded => receipt,
            GenerationStreamStatus::Cancelled => {
                receipt.status = ReceiptStatus::Cancelled;
                receipt.failure_reason.clone_from(&terminal.failure_reason);
                if let Some(diagnostic) = terminal.diagnostic.clone() {
                    receipt = receipt.with_diagnostic(diagnostic);
                }
                receipt
            }
            GenerationStreamStatus::Disconnected => {
                receipt.status = ReceiptStatus::Disconnected;
                receipt.failure_reason.clone_from(&terminal.failure_reason);
                if let Some(diagnostic) = terminal.diagnostic.clone() {
                    receipt = receipt.with_diagnostic(diagnostic);
                }
                receipt
            }
            GenerationStreamStatus::Failed => {
                receipt.status = ReceiptStatus::Failed;
                receipt.failure_reason.clone_from(&terminal.failure_reason);
                if let Some(diagnostic) = terminal.diagnostic.clone() {
                    receipt = receipt.with_diagnostic(diagnostic);
                }
                receipt
            }
        }
    }
}

/// Computes a deterministic digest for an embeddings request.
#[must_use]
pub fn digest_embedding_request(request: &EmbeddingRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request.request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(request.product_id.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.model.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.model.family.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.model.revision.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.dimensions.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", request.model.normalization).as_bytes());
    hasher.update(b"|");
    hasher.update(
        request
            .output_dimensions
            .filter(|dimensions| *dimensions > 0 && *dimensions < request.model.dimensions)
            .map_or_else(String::new, |dimensions| dimensions.to_string())
            .as_bytes(),
    );
    hasher.update(b"|");
    digest_weight_bundle(&mut hasher, &request.model.weights);
    if let Some(artifact_identity) = &request.model.artifact_identity {
        hasher.update(b"|");
        hasher.update(
            artifact_identity
                .model_blob_digest
                .as_deref()
                .unwrap_or("")
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            artifact_identity
                .tokenizer_digest
                .as_deref()
                .unwrap_or("")
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            artifact_identity
                .chat_template_digest
                .as_deref()
                .unwrap_or("")
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(artifact_identity.generation_defaults_digest.as_bytes());
    }
    for input in &request.inputs {
        hasher.update(b"|");
        hasher.update(input.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Computes a deterministic digest for a generation request.
#[must_use]
pub fn digest_generation_request(request: &GenerationRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request.request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(request.product_id.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.model.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.model.family.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.model.revision.as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.config.hidden_size.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.config.layer_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.config.vocab_size.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(request.model.config.max_context.to_string().as_bytes());
    hasher.update(b"|");
    digest_weight_bundle(&mut hasher, &request.model.weights);
    if let Some(artifact_identity) = &request.model.artifact_identity {
        hasher.update(b"|");
        hasher.update(
            artifact_identity
                .model_blob_digest
                .as_deref()
                .unwrap_or("")
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            artifact_identity
                .tokenizer_digest
                .as_deref()
                .unwrap_or("")
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            artifact_identity
                .chat_template_digest
                .as_deref()
                .unwrap_or("")
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(artifact_identity.generation_defaults_digest.as_bytes());
    }
    hasher.update(b"|");
    if let Some(session_id) = &request.session_id {
        hasher.update(session_id.as_str().as_bytes());
    }
    hasher.update(b"|");
    digest_generation_input(&mut hasher, &request.prompt);
    hasher.update(b"|");
    hasher.update(request.options.max_output_tokens.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", request.options.decode_strategy).as_bytes());
    hasher.update(b"|");
    if let Some(temperature) = request.options.temperature {
        hasher.update(format!("temperature={temperature}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(top_k) = request.options.top_k {
        hasher.update(format!("top_k={top_k}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(top_p) = request.options.top_p {
        hasher.update(format!("top_p={top_p}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(repeat_penalty) = request.options.repeat_penalty {
        hasher.update(format!("repeat_penalty={repeat_penalty}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(presence_penalty) = request.options.presence_penalty {
        hasher.update(format!("presence_penalty={presence_penalty}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(frequency_penalty) = request.options.frequency_penalty {
        hasher.update(format!("frequency_penalty={frequency_penalty}").as_bytes());
    }
    hasher.update(b"|");
    if let Some(seed) = request.options.seed {
        hasher.update(format!("seed={seed}").as_bytes());
    }
    hasher.update(b"|");
    for stop_sequence in &request.options.stop_sequences {
        hasher.update(stop_sequence.as_bytes());
        hasher.update(b"\x1f");
    }
    hasher.update(b"|");
    hasher.update(if request.reset_session { b"1" } else { b"0" });
    hex::encode(hasher.finalize())
}

/// Computes a deterministic digest for a sandbox-execution request identity.
#[must_use]
pub fn digest_sandbox_execution_request(request: &SandboxExecutionRequestIdentity) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request.request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(SANDBOX_EXECUTION_PRODUCT_ID.as_bytes());
    hasher.update(b"|");
    hasher.update(request.sandbox_profile_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(request.command_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(request.environment_digest.as_bytes());
    for digest in &request.input_artifact_digests {
        hasher.update(b"|");
        hasher.update(digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Provider-side adapter interface for the embeddings smoke path.
pub trait EmbeddingsProviderAdapter {
    /// Returns the advertised capability envelope.
    fn capability(&self) -> CapabilityEnvelope;

    /// Returns the provider readiness state.
    fn readiness(&self) -> ProviderReadiness;
}

/// Provider-side adapter interface for text generation.
pub trait TextGenerationProviderAdapter {
    /// Returns the advertised text-generation capability envelope.
    fn text_generation_capability(&self) -> TextGenerationCapabilityEnvelope;

    /// Returns the provider readiness state.
    fn readiness(&self) -> ProviderReadiness;
}

fn provenance_kind_label(kind: ModelArtifactProvenanceKind) -> &'static str {
    match kind {
        ModelArtifactProvenanceKind::Fixture => "fixture",
        ModelArtifactProvenanceKind::LocalPath => "local_path",
        ModelArtifactProvenanceKind::OllamaBlob => "ollama_blob",
        ModelArtifactProvenanceKind::OllamaManifest => "ollama_manifest",
        ModelArtifactProvenanceKind::OllamaRemoteAlias => "ollama_remote_alias",
    }
}

fn digest_generation_input(hasher: &mut Sha256, input: &GenerationInput) {
    match input {
        GenerationInput::Text(text) => {
            hasher.update(b"text|");
            hasher.update(text.as_bytes());
        }
        GenerationInput::Tokens(tokens) => {
            hasher.update(b"tokens|");
            for token in tokens.as_slice() {
                hasher.update(token.as_u32().to_string().as_bytes());
                hasher.update(b",");
            }
        }
    }
}

fn digest_weight_bundle(hasher: &mut Sha256, weight_bundle: &WeightBundleMetadata) {
    hasher.update(format!("{:?}", weight_bundle.format).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", weight_bundle.source).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", weight_bundle.quantization).as_bytes());
    hasher.update(b"|");
    hasher.update(weight_bundle.digest.as_bytes());
    for artifact in &weight_bundle.artifacts {
        hasher.update(b"|");
        hasher.update(artifact.name.as_bytes());
        hasher.update(b":");
        hasher.update(artifact.byte_length.to_string().as_bytes());
        hasher.update(b":");
        hasher.update(artifact.sha256.as_bytes());
    }
}

fn failed_generation_cache_observations(request: &GenerationRequest) -> Vec<CacheObservation> {
    let mut observations = vec![
        CacheObservation::new(
            CacheKind::ExecutionPlan,
            CacheAction::Bypass,
            "failed request did not complete enough work to surface reusable execution-plan state",
        ),
        CacheObservation::new(
            CacheKind::PagedTensorStorage,
            if request.model.weights.is_artifact_backed() {
                CacheAction::Restore
            } else {
                CacheAction::Bypass
            },
            if request.model.weights.is_artifact_backed() {
                "artifact-backed tensor storage remains restoreable from local model bytes"
            } else {
                "fixture-backed weights do not use reusable paged tensor storage"
            },
        ),
        CacheObservation::new(
            CacheKind::PrefixCache,
            CacheAction::Bypass,
            "failed request did not report a realized shared prefix-cache action",
        ),
    ];
    let kv_observation = if request.reset_session {
        CacheObservation::new(
            CacheKind::KvState,
            CacheAction::Invalidate,
            "session KV state was discarded before the failed request path",
        )
        .with_trigger(CacheInvalidationTrigger::ExplicitReset)
    } else if request.session_id.is_some() {
        CacheObservation::new(
            CacheKind::KvState,
            CacheAction::Bypass,
            "failed request did not report compatible session KV reuse",
        )
    } else {
        CacheObservation::new(
            CacheKind::KvState,
            CacheAction::Bypass,
            "failed request did not target session-bound KV reuse",
        )
    };
    observations.push(kv_observation);
    observations
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use psionic_cluster::{
        ClusterTrustPolicy, ConfiguredClusterPeer, LayerShardedExecutionRequest,
        NodeAttestationRequirement, NodeId, PipelineShardedExecutionRequest,
        TensorShardedExecutionRequest, TensorShardedModelEligibility,
        WholeRequestSchedulingRequest,
    };
    use psionic_core::{
        BackendExtensionKind, DType, Device, DeviceKind,
        QuantizationMode as RuntimeQuantizationMode,
    };
    use psionic_models::{TassadarExecutorFixture, TassadarWorkloadClass};
    use psionic_research::{
        TassadarPromotionChecklistGateKind, TassadarPromotionPolicyStatus,
        build_tassadar_decompilable_executor_artifacts_report,
        build_tassadar_kernel_module_scaling_summary_report,
        build_tassadar_promotion_policy_report,
        build_tassadar_workload_capability_frontier_summary_report,
    };
    use psionic_router::{
        TassadarPlannerExecutorNegotiatedRouteState,
        TassadarPlannerExecutorRouteNegotiationOutcome,
        TassadarPlannerExecutorRouteNegotiationRequest, TassadarPlannerExecutorRouteRefusalReason,
        TassadarPlannerExecutorWasmImportPosture, TassadarPlannerExecutorWasmOpcodeFamily,
        negotiate_tassadar_planner_executor_route,
    };
    use psionic_runtime::{
        AcceleratorDeliverabilityStatus, AcceleratorExecutionRequirement, AllocatorPoolPolicy,
        AllocatorPoolReport, AllocatorPoolState, AmdDeviceMetadata, AmdDriverBinding,
        AmdRecoveryAction, AmdRecoveryProfile, AmdRiskLevel, AmdRiskProfile, AmdRuntimeMode,
        AmdTopologyInfo, BackendDegradedPolicy, BackendExtensionSupport, BackendProbeState,
        BackendRuntimeResources, BackendSelection, BackendToolchainIdentity, CacheAction,
        ClusterAdmissionFactKind, ClusterArtifactResidencyDisposition, ClusterCacheCapability,
        ClusterCacheScope, ClusterCacheUsage, ClusterCommandAuthorityScopeEvidence,
        ClusterCommandProvenanceEvidence, ClusterCommitAuthorityEvidence,
        ClusterComputeMarketTrustAssessment, ClusterComputeMarketTrustDisposition,
        ClusterComputeMarketTrustRefusalReason, ClusterExecutionCapabilityProfile,
        ClusterExecutionContext, ClusterExecutionDisposition, ClusterExecutionLane,
        ClusterFallbackReason, ClusterFallbackStep, ClusterPipelineStage, ClusterPipelineStageRole,
        ClusterPolicyDigest, ClusterPolicyDigestKind, ClusterSelectedNode, ClusterServingSemantics,
        ClusterTransportClass, ClusterWarmRoutePosture, DeviceDescriptor, DeviceMemoryBudget,
        DeviceMemoryClass, DevicePerformanceClass, ExecutionCapabilityProfile,
        ExecutionDeliveryProof, ExecutionProofAugmentationPosture, ExecutionProofBundleKind,
        ExecutionTopologyKind, ExecutionTopologyPlan, HealthStatus, KernelCachePolicy,
        KernelCacheReport, KernelCacheState, KvCacheAccounting, KvCacheEncodingAccounting,
        KvCacheEncodingFamily, KvCacheEncodingObjective, KvCacheEncodingPolicy,
        KvResidencyAccounting, KvResidencyTier, KvResidencyTierState, LocalRuntimeDiagnostic,
        LocalRuntimeErrorCode, LocalRuntimeObservability, LocalServingIsolationPolicy,
        MemoryResidencySnapshot, ModelResidencyPolicy, NvidiaDeviceMetadata, NvidiaRecoveryAction,
        NvidiaRecoveryProfile, NvidiaRiskLevel, NvidiaRiskProfile, NvidiaTopologyInfo,
        PrefixCacheIdentity, PrefixCacheState, QuantizationExecution, QuantizationLoadPath,
        QuantizationSupport, RuntimeTransitionEvent, RuntimeTransitionKind,
        SandboxExecutionCapabilityProfile, SandboxExecutionEvidence, SandboxExecutionExit,
        SandboxExecutionExitKind, SandboxExecutionRequestIdentity, SandboxExecutionResourceSummary,
        ServedProductBackendPolicy, TassadarCpuReferenceRunner, TassadarExactnessPosture,
        TassadarExactnessRefusalReport, TassadarTraceArtifact, ValidationCoverage,
    };
    use psionic_serve::{
        AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterResidencyMode,
        AdapterServingBinding, AdapterTargetFamily, ByteProjectionEmbedder, EmbeddingMetrics,
        EmbeddingNormalization, EmbeddingRequest, EmbeddingResponse, EmbeddingVector,
        GenerationLoadState, GenerationMetrics, GenerationOptions, GenerationProvenance,
        GenerationRequest, GenerationResponse, GenerationStreamStatus, GenerationStreamTerminal,
        LocalTassadarExecutorService, LocalTassadarPlannerRouter, ModelArtifactGovernance,
        ModelArtifactLicenseEntry, ModelArtifactLicenseFacts, ModelArtifactProvenance,
        ModelArtifactProvenanceKind, PsionServedEvidenceBundle, PsionServedOutputClaimPosture,
        ReferenceWordDecoder, SessionId, SmokeByteEmbedder, TerminationReason, TokenSequence,
        WeightArtifactMetadata, WeightSource, default_decoder_kv_cache_encoding_policy,
        default_decoder_kv_cache_policy, default_decoder_memory_plan,
        default_generation_streaming_policy, default_prefix_cache_policy,
        default_text_generation_execution_profile,
    };
    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        CapabilityEnvelope, ComputeMarketSupplyViolationCode, ExecutionReceipt, KvCacheMode,
        LocalRuntimeObservabilityEnvelope, ProviderReadiness, ReceiptStatus,
        SandboxExecutionCapabilityEnvelope, SandboxExecutionReceipt, TassadarCapabilityEnvelope,
        TassadarCapabilityEnvelopeError, TassadarDecompilationReceipt,
        TassadarExactnessRefusalReceipt, TassadarKernelModuleScalingReceipt,
        TassadarPlannerRouteCapabilityEnvelope, TassadarPlannerRouteCapabilityEnvelopeError,
        TassadarPromotionPolicyReceipt, TassadarTraceArtifactReceipt, TassadarTraceDiffReceipt,
        TassadarWorkloadCapabilityFrontierReceipt, TextGenerationCapabilityEnvelope,
        TextGenerationReceipt, WeightBundleEvidence, cache_invalidation_policy,
        compute_market_supply_refusal_diagnostic, default_compute_market_supply_policy,
        digest_embedding_request, digest_generation_request, digest_sandbox_execution_request,
        evaluate_compute_market_supply, served_artifact_identity_for_decoder_model,
    };

    #[test]
    fn capability_envelope_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let envelope = CapabilityEnvelope::from_embedding_model(
            cpu_backend_selection(),
            &model,
            ProviderReadiness::ready("cpu backend ready"),
        );

        assert_eq!(
            serde_json::to_value(&envelope)?,
            json!({
                "backend_family": "psionic",
                "product_id": "psionic.embeddings",
                "runtime_backend": "cpu",
                "validation": {
                    "matrix_id": "psionic.minimum_hardware_validation.v1",
                    "documentation_path": "docs/HARDWARE_VALIDATION_MATRIX.md",
                    "claim_id": "cpu.embeddings.reference",
                    "coverage": "positive_execution"
                },
                "backend_selection": {
                    "requested_backend": "cpu",
                    "effective_backend": "cpu",
                    "selected_device": {
                        "backend": "cpu",
                        "device": {
                            "kind": "Cpu",
                            "ordinal": 0,
                            "label": "cpu:0"
                        },
                        "device_name": "host cpu",
                        "supported_dtypes": ["F32"],
                        "supported_quantization": [
                            {
                                "mode": "none",
                                "load_path": "dense_f32",
                                "execution": "native"
                            },
                            {
                                "mode": "int8_symmetric",
                                "load_path": "dequantized_f32",
                                "execution": "dequantize_to_f32"
                            },
                            {
                                "mode": "ggml_q4_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q4_1",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q8_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            }
                        ],
                        "memory_capacity_bytes": null,
                        "unified_memory": true,
                        "feature_flags": ["host_memory"]
                    },
                    "selected_devices": [{
                        "backend": "cpu",
                        "device": {
                            "kind": "Cpu",
                            "ordinal": 0,
                            "label": "cpu:0"
                        },
                        "device_name": "host cpu",
                        "supported_dtypes": ["F32"],
                        "supported_quantization": [
                            {
                                "mode": "none",
                                "load_path": "dense_f32",
                                "execution": "native"
                            },
                            {
                                "mode": "int8_symmetric",
                                "load_path": "dequantized_f32",
                                "execution": "dequantize_to_f32"
                            },
                            {
                                "mode": "ggml_q4_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q4_1",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q8_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            }
                        ],
                        "memory_capacity_bytes": null,
                        "unified_memory": true,
                        "feature_flags": ["host_memory"]
                    }],
                    "supported_ops": ["input", "constant", "matmul", "add"],
                    "policy": {
                        "unavailable": "refuse",
                        "degraded": "allow_same_backend"
                    },
                    "fallback_lattice": {
                        "unavailable": "refuse",
                        "degraded": "degrade",
                        "numerical_safety": "refuse",
                        "memory_pressure": "refuse",
                        "plan_unavailable": "same_backend_slow_path",
                        "transient_backend_failure": "retry"
                    },
                    "selection_state": "direct",
                    "fallback_trigger": null,
                    "fallback_action": null,
                    "fallback_reason": null,
                    "degraded_reason": null,
                    "retry_attempt": null,
                    "execution_topology": {
                        "effective_backend": "cpu",
                        "kind": "single_device",
                        "assignments": [{
                            "shard_id": 0,
                            "device": {
                                "stable_device_id": "cpu:0",
                                "placement_index": 0
                            },
                            "partition": {
                                "kind": "whole_model"
                            }
                        }]
                    }
                },
                "backend_toolchain": {
                    "effective_backend": "cpu",
                    "toolchain_version": format!("cpu@{}", env!("CARGO_PKG_VERSION")),
                    "probe_state": "compiled_and_probed",
                    "probed_backend_features": ["host_memory"]
                },
                "selected_device_inventory": {
                    "stable_device_id": "cpu:0",
                    "performance_class": "reference",
                    "memory_class": "host_only"
                },
                "selected_devices": [{
                    "stable_device_id": "cpu:0",
                    "performance_class": "reference",
                    "memory_class": "host_only"
                }],
                "execution_topology": {
                    "effective_backend": "cpu",
                    "kind": "single_device",
                    "assignments": [{
                        "shard_id": 0,
                        "device": {
                            "stable_device_id": "cpu:0",
                            "placement_index": 0
                        },
                        "partition": {
                            "kind": "whole_model"
                        }
                    }]
                },
                "model_id": "smoke-byte-embed-v0",
                "model_family": "smoke",
                "model_revision": "v0",
                "weight_bundle": {
                    "format": "ProgrammaticFixture",
                    "source": "Fixture",
                    "quantization": "none",
                    "digest": "30a2fd0264ef45e96101268ae97cfbdffb79540210c88ab834117bc0111c0b00",
                    "artifacts": []
                },
                "served_artifact": {
                    "model_id": "smoke-byte-embed-v0",
                    "model_revision": "v0",
                    "weight_bundle_digest": "30a2fd0264ef45e96101268ae97cfbdffb79540210c88ab834117bc0111c0b00",
                    "served_artifact_digest": envelope.served_artifact.served_artifact_digest.clone(),
                    "generation_defaults_digest": "6b25930e91686cee8bb5d4dae8dbed14f63c690c1c97ecb98552d8842e2d9395",
                    "weight_format": "programmatic_fixture",
                    "quantization_family": "none",
                    "backend": {
                        "effective_backend": "cpu",
                        "toolchain_version": format!("cpu@{}", env!("CARGO_PKG_VERSION")),
                        "probe_state": "compiled_only"
                    }
                },
                "supply_policy": {
                    "allow_fixture_models": true,
                    "allow_local_path_artifacts": false,
                    "allow_unbound_ollama_blobs": false,
                    "allow_ollama_manifests": true,
                    "allow_ollama_remote_aliases": true,
                    "require_declared_license_for_external_artifacts": true
                },
                "supply_decision": {
                    "advertise_allowed": true,
                    "serve_allowed": true
                },
                "cache_invalidation_policy": {
                    "runtime_binary_version": env!("CARGO_PKG_VERSION"),
                    "execution_plan": {
                        "scope": "process_local",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "rebuild",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "model_metadata_change",
                            "tokenizer_drift",
                            "chat_template_drift",
                            "generation_defaults_drift",
                            "quantization_change",
                            "plan_format_upgrade"
                        ]
                    },
                    "kernel_cache": {
                        "scope": "process_local",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "invalidate",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "kernel_format_upgrade"
                        ]
                    },
                    "paged_tensor_storage": {
                        "scope": "artifact_backed",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "restore",
                        "invalidates_on": [
                            "binary_upgrade",
                            "model_metadata_change",
                            "quantization_change",
                            "paged_tensor_format_upgrade"
                        ]
                    },
                    "prefix_cache": {
                        "scope": "shared_across_requests",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "rebuild",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "model_metadata_change",
                            "tokenizer_drift",
                            "chat_template_drift",
                            "generation_defaults_drift",
                            "quantization_change",
                            "prefix_cache_format_upgrade"
                        ]
                    },
                    "kv_state": {
                        "scope": "session_bound",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "invalidate",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "model_metadata_change",
                            "tokenizer_drift",
                            "chat_template_drift",
                            "generation_defaults_drift",
                            "quantization_change",
                            "kv_state_format_upgrade"
                        ]
                    }
                },
                "execution_profile": {
                    "batch_posture": "caller_static_batch",
                    "queue_policy": {
                        "discipline": "direct_caller_backpressure",
                        "max_active_requests": 1,
                        "max_queued_requests": 0,
                        "per_model_serialization": true
                    },
                    "throughput_class": "balanced"
                },
                "dimensions": 8,
                "normalization": "None",
                "preserves_input_order": true,
                "empty_batch_returns_empty": true,
                "supports_output_dimensions": true,
                "supports_input_truncation": false,
                "readiness": {
                    "status": "Ready",
                    "message": "cpu backend ready"
                }
            })
        );
        Ok(())
    }

    #[test]
    fn sandbox_execution_capability_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let envelope = SandboxExecutionCapabilityEnvelope::new(
            cpu_backend_selection(),
            SandboxExecutionCapabilityProfile::bounded_cpu(),
            ProviderReadiness::ready("sandbox lane ready"),
        );

        assert_eq!(
            serde_json::to_value(&envelope)?,
            json!({
                "backend_family": "psionic",
                "product_id": "psionic.sandbox_execution",
                "runtime_backend": "cpu",
                "backend_selection": {
                    "requested_backend": "cpu",
                    "effective_backend": "cpu",
                    "selected_device": {
                        "backend": "cpu",
                        "device": {
                            "kind": "Cpu",
                            "ordinal": 0,
                            "label": "cpu:0"
                        },
                        "device_name": "host cpu",
                        "supported_dtypes": ["F32"],
                        "supported_quantization": [
                            {
                                "mode": "none",
                                "load_path": "dense_f32",
                                "execution": "native"
                            },
                            {
                                "mode": "int8_symmetric",
                                "load_path": "dequantized_f32",
                                "execution": "dequantize_to_f32"
                            },
                            {
                                "mode": "ggml_q4_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q4_1",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q8_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            }
                        ],
                        "memory_capacity_bytes": null,
                        "unified_memory": true,
                        "feature_flags": ["host_memory"]
                    },
                    "selected_devices": [{
                        "backend": "cpu",
                        "device": {
                            "kind": "Cpu",
                            "ordinal": 0,
                            "label": "cpu:0"
                        },
                        "device_name": "host cpu",
                        "supported_dtypes": ["F32"],
                        "supported_quantization": [
                            {
                                "mode": "none",
                                "load_path": "dense_f32",
                                "execution": "native"
                            },
                            {
                                "mode": "int8_symmetric",
                                "load_path": "dequantized_f32",
                                "execution": "dequantize_to_f32"
                            },
                            {
                                "mode": "ggml_q4_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q4_1",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q8_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            }
                        ],
                        "memory_capacity_bytes": null,
                        "unified_memory": true,
                        "feature_flags": ["host_memory"]
                    }],
                    "supported_ops": ["input", "constant", "matmul", "add"],
                    "policy": {
                        "unavailable": "refuse",
                        "degraded": "allow_same_backend"
                    },
                    "fallback_lattice": {
                        "unavailable": "refuse",
                        "degraded": "degrade",
                        "numerical_safety": "refuse",
                        "memory_pressure": "refuse",
                        "plan_unavailable": "same_backend_slow_path",
                        "transient_backend_failure": "retry"
                    },
                    "selection_state": "direct",
                    "fallback_trigger": null,
                    "fallback_action": null,
                    "fallback_reason": null,
                    "degraded_reason": null,
                    "retry_attempt": null,
                    "execution_topology": {
                        "effective_backend": "cpu",
                        "kind": "single_device",
                        "assignments": [{
                            "shard_id": 0,
                            "device": {
                                "stable_device_id": "cpu:0",
                                "placement_index": 0
                            },
                            "partition": {
                                "kind": "whole_model"
                            }
                        }]
                    }
                },
                "backend_toolchain": {
                    "effective_backend": "cpu",
                    "toolchain_version": format!("cpu@{}", env!("CARGO_PKG_VERSION")),
                    "probe_state": "compiled_and_probed",
                    "probed_backend_features": ["host_memory"]
                },
                "selected_device_inventory": {
                    "stable_device_id": "cpu:0",
                    "performance_class": "reference",
                    "memory_class": "host_only"
                },
                "selected_devices": [{
                    "stable_device_id": "cpu:0",
                    "performance_class": "reference",
                    "memory_class": "host_only"
                }],
                "execution_topology": {
                    "effective_backend": "cpu",
                    "kind": "single_device",
                    "assignments": [{
                        "shard_id": 0,
                        "device": {
                            "stable_device_id": "cpu:0",
                            "placement_index": 0
                        },
                        "partition": {
                            "kind": "whole_model"
                        }
                    }]
                },
                "execution_profile": {
                    "dispatch_profile": {
                        "batch_posture": "single_request_only",
                        "queue_policy": {
                            "discipline": "direct_caller_backpressure",
                            "max_active_requests": 1,
                            "max_queued_requests": 0,
                            "per_model_serialization": true
                        },
                        "throughput_class": "latency_optimized"
                    },
                    "isolation_boundary": "container",
                    "filesystem": {
                        "root": "read_only",
                        "writable_mounts": ["/tmp"],
                        "max_write_bytes": 67108864
                    },
                    "network": {
                        "mode": "disabled",
                        "allow_loopback": false
                    },
                    "process": {
                        "max_processes": 32,
                        "max_threads_per_process": 8,
                        "allow_privilege_escalation": false
                    },
                    "resource_limits": {
                        "max_wall_time_ms": 300000,
                        "max_cpu_time_ms": 300000,
                        "max_memory_bytes": 2147483648u64,
                        "max_stdout_bytes": 1048576,
                        "max_stderr_bytes": 1048576
                    },
                    "accelerator_access": {
                        "mode": "disabled"
                    }
                },
                "readiness": {
                    "status": "Ready",
                    "message": "sandbox lane ready"
                }
            })
        );
        Ok(())
    }

    #[test]
    fn sandbox_execution_receipt_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = SandboxExecutionRequestIdentity {
            request_id: String::from("sandbox-req-1"),
            sandbox_profile_digest: SandboxExecutionCapabilityProfile::bounded_cpu()
                .stable_digest(),
            command_digest: String::from("command123"),
            environment_digest: String::from("env123"),
            input_artifact_digests: vec![String::from("input-a")],
        };
        let evidence = SandboxExecutionEvidence {
            request_digest: digest_sandbox_execution_request(&request),
            sandbox_profile_digest: request.sandbox_profile_digest.clone(),
            command_digest: request.command_digest.clone(),
            environment_digest: request.environment_digest.clone(),
            input_artifact_digests: request.input_artifact_digests.clone(),
            output_artifact_digests: vec![String::from("output-a")],
            exit: SandboxExecutionExit {
                kind: SandboxExecutionExitKind::TimedOut,
                exit_code: None,
                detail: String::from("sandbox exceeded wall-clock budget"),
            },
            resources: SandboxExecutionResourceSummary {
                wall_time_ms: 301_000,
                cpu_time_ms: 280_000,
                peak_memory_bytes: 256 * 1024 * 1024,
                filesystem_write_bytes: 8192,
                stdout_bytes: 32,
                stderr_bytes: 128,
                network_egress_bytes: 0,
            },
            stdout_sha256: Some(String::from("stdout123")),
            stderr_sha256: Some(String::from("stderr123")),
            delivery_proof: Some(ExecutionDeliveryProof {
                execution_plan_digest: String::from("plan123"),
                kernel_count: 2,
                bytes_moved: 4096,
                plan_cache_hits: 1,
                plan_cache_misses: 0,
                kv_growth: None,
                prefill_decode_handoff: None,
                kv_residency: None,
            }),
        };
        let diagnostic = Some(LocalRuntimeDiagnostic::new(
            LocalRuntimeErrorCode::AdmissionRefused,
            403,
            "sandbox wall-clock limit exceeded",
        ));

        let receipt = SandboxExecutionReceipt::from_evidence(
            cpu_backend_selection(),
            &request,
            evidence.clone(),
            10,
            311_000,
            diagnostic.clone(),
        );

        assert_eq!(receipt.status, ReceiptStatus::Failed);
        assert_eq!(
            receipt.failure_reason.as_deref(),
            Some("sandbox exceeded wall-clock budget")
        );
        assert_eq!(receipt.evidence, evidence);
        assert_eq!(receipt.diagnostic, diagnostic);
        assert_eq!(
            receipt.proof_bundle.bundle_kind,
            ExecutionProofBundleKind::Sandbox
        );
        assert_eq!(
            receipt
                .proof_bundle
                .artifact_residency
                .as_ref()
                .map(|value| value.output_artifact_digests.as_slice()),
            Some(&["output-a".to_string()][..])
        );
        assert_eq!(
            serde_json::from_value::<SandboxExecutionReceipt>(serde_json::to_value(&receipt)?)?,
            receipt
        );
        Ok(())
    }

    #[test]
    fn sandbox_execution_receipt_can_surface_accelerator_deliverability()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = SandboxExecutionRequestIdentity {
            request_id: String::from("sandbox-req-2"),
            sandbox_profile_digest: SandboxExecutionCapabilityProfile::bounded_accelerated(
                "cuda", 2,
            )
            .stable_digest(),
            command_digest: String::from("command456"),
            environment_digest: String::from("env456"),
            input_artifact_digests: vec![String::from("input-a")],
        };
        let evidence = SandboxExecutionEvidence {
            request_digest: digest_sandbox_execution_request(&request),
            sandbox_profile_digest: request.sandbox_profile_digest.clone(),
            command_digest: request.command_digest.clone(),
            environment_digest: request.environment_digest.clone(),
            input_artifact_digests: request.input_artifact_digests.clone(),
            output_artifact_digests: vec![String::from("output-a")],
            exit: SandboxExecutionExit {
                kind: SandboxExecutionExitKind::Succeeded,
                exit_code: Some(0),
                detail: String::from("sandbox completed successfully"),
            },
            resources: SandboxExecutionResourceSummary {
                wall_time_ms: 1000,
                cpu_time_ms: 600,
                peak_memory_bytes: 256 * 1024 * 1024,
                filesystem_write_bytes: 4096,
                stdout_bytes: 64,
                stderr_bytes: 0,
                network_egress_bytes: 0,
            },
            stdout_sha256: Some(String::from("stdout456")),
            stderr_sha256: None,
            delivery_proof: None,
        };

        let receipt = SandboxExecutionReceipt::from_evidence(
            cuda_multi_device_selection(),
            &request,
            evidence,
            20,
            1020,
            None,
        )
        .with_accelerator_requirement(
            AcceleratorExecutionRequirement::new("cuda", 1)
                .with_topology_kind(ExecutionTopologyKind::SingleDevice)
                .with_minimum_performance_class(DevicePerformanceClass::DiscreteAccelerator)
                .with_minimum_memory_class(DeviceMemoryClass::DedicatedDevice)
                .with_minimum_total_memory_bytes(16 * 1024 * 1024 * 1024),
        );

        let Some(report) = receipt.accelerator_deliverability else {
            return Err("accelerator deliverability missing".into());
        };
        assert_eq!(
            report.status,
            AcceleratorDeliverabilityStatus::CompatibleSubstitution
        );
        Ok(())
    }

    #[test]
    fn text_generation_capability_json_is_stable() -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_decoder_descriptor();
        let envelope = TextGenerationCapabilityEnvelope::from_decoder_model(
            cpu_backend_selection(),
            &model,
            default_decoder_memory_plan(&model, None, None),
            ModelResidencyPolicy::default(),
            KvCacheMode::Paged,
            default_text_generation_execution_profile(),
            ProviderReadiness::ready("cpu backend ready"),
        );

        assert_eq!(
            serde_json::to_value(&envelope)?,
            json!({
                "backend_family": "psionic",
                "product_id": "psionic.text_generation",
                "runtime_backend": "cpu",
                "validation": {
                    "matrix_id": "psionic.minimum_hardware_validation.v1",
                    "documentation_path": "docs/HARDWARE_VALIDATION_MATRIX.md",
                    "claim_id": "cpu.text_generation.reference",
                    "coverage": "positive_execution"
                },
                "backend_selection": {
                    "requested_backend": "cpu",
                    "effective_backend": "cpu",
                    "selected_device": {
                        "backend": "cpu",
                        "device": {
                            "kind": "Cpu",
                            "ordinal": 0,
                            "label": "cpu:0"
                        },
                        "device_name": "host cpu",
                        "supported_dtypes": ["F32"],
                        "supported_quantization": [
                            {
                                "mode": "none",
                                "load_path": "dense_f32",
                                "execution": "native"
                            },
                            {
                                "mode": "int8_symmetric",
                                "load_path": "dequantized_f32",
                                "execution": "dequantize_to_f32"
                            },
                            {
                                "mode": "ggml_q4_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q4_1",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q8_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            }
                        ],
                        "memory_capacity_bytes": null,
                        "unified_memory": true,
                        "feature_flags": ["host_memory"]
                    },
                    "selected_devices": [{
                        "backend": "cpu",
                        "device": {
                            "kind": "Cpu",
                            "ordinal": 0,
                            "label": "cpu:0"
                        },
                        "device_name": "host cpu",
                        "supported_dtypes": ["F32"],
                        "supported_quantization": [
                            {
                                "mode": "none",
                                "load_path": "dense_f32",
                                "execution": "native"
                            },
                            {
                                "mode": "int8_symmetric",
                                "load_path": "dequantized_f32",
                                "execution": "dequantize_to_f32"
                            },
                            {
                                "mode": "ggml_q4_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q4_1",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            },
                            {
                                "mode": "ggml_q8_0",
                                "load_path": "backend_quantized",
                                "execution": "native"
                            }
                        ],
                        "memory_capacity_bytes": null,
                        "unified_memory": true,
                        "feature_flags": ["host_memory"]
                    }],
                    "supported_ops": ["input", "constant", "matmul", "add"],
                    "policy": {
                        "unavailable": "refuse",
                        "degraded": "allow_same_backend"
                    },
                    "fallback_lattice": {
                        "unavailable": "refuse",
                        "degraded": "degrade",
                        "numerical_safety": "refuse",
                        "memory_pressure": "refuse",
                        "plan_unavailable": "same_backend_slow_path",
                        "transient_backend_failure": "retry"
                    },
                    "selection_state": "direct",
                    "fallback_trigger": null,
                    "fallback_action": null,
                    "fallback_reason": null,
                    "degraded_reason": null,
                    "retry_attempt": null,
                    "execution_topology": {
                        "effective_backend": "cpu",
                        "kind": "single_device",
                        "assignments": [{
                            "shard_id": 0,
                            "device": {
                                "stable_device_id": "cpu:0",
                                "placement_index": 0
                            },
                            "partition": {
                                "kind": "whole_model"
                            }
                        }]
                    }
                },
                "backend_toolchain": {
                    "effective_backend": "cpu",
                    "toolchain_version": format!("cpu@{}", env!("CARGO_PKG_VERSION")),
                    "probe_state": "compiled_and_probed",
                    "probed_backend_features": ["host_memory"]
                },
                "selected_device_inventory": {
                    "stable_device_id": "cpu:0",
                    "performance_class": "reference",
                    "memory_class": "host_only"
                },
                "selected_devices": [{
                    "stable_device_id": "cpu:0",
                    "performance_class": "reference",
                    "memory_class": "host_only"
                }],
                "execution_topology": {
                    "effective_backend": "cpu",
                    "kind": "single_device",
                    "assignments": [{
                        "shard_id": 0,
                        "device": {
                            "stable_device_id": "cpu:0",
                            "placement_index": 0
                        },
                        "partition": {
                            "kind": "whole_model"
                        }
                    }]
                },
                "model_id": "fixture-word-decoder-v0",
                "model_family": "fixture_decoder",
                "model_revision": "v0",
                "weight_bundle": {
                    "format": "ProgrammaticFixture",
                    "source": "Fixture",
                    "quantization": "none",
                    "digest": "7daf98e44b6eee34df8d97f24419709f23b19010cdb49c9b18b771936ced352b",
                    "artifacts": []
                },
                "served_artifact": {
                    "model_id": "fixture-word-decoder-v0",
                    "model_revision": "v0",
                    "weight_bundle_digest": "7daf98e44b6eee34df8d97f24419709f23b19010cdb49c9b18b771936ced352b",
                    "served_artifact_digest": envelope.served_artifact.served_artifact_digest.clone(),
                    "tokenizer_digest": "5464809cdd952c531b8536eeec1c728a8b6aa9621853f2bf63e569c9d5a9117f",
                    "generation_defaults_digest": "6b25930e91686cee8bb5d4dae8dbed14f63c690c1c97ecb98552d8842e2d9395",
                    "weight_format": "programmatic_fixture",
                    "quantization_family": "none",
                    "backend": {
                        "effective_backend": "cpu",
                        "toolchain_version": format!("cpu@{}", env!("CARGO_PKG_VERSION")),
                        "probe_state": "compiled_only"
                    }
                },
                "supply_policy": {
                    "allow_fixture_models": true,
                    "allow_local_path_artifacts": false,
                    "allow_unbound_ollama_blobs": false,
                    "allow_ollama_manifests": true,
                    "allow_ollama_remote_aliases": true,
                    "require_declared_license_for_external_artifacts": true
                },
                "supply_decision": {
                    "advertise_allowed": true,
                    "serve_allowed": true
                },
                "cache_invalidation_policy": {
                    "runtime_binary_version": env!("CARGO_PKG_VERSION"),
                    "execution_plan": {
                        "scope": "process_local",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "rebuild",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "model_metadata_change",
                            "tokenizer_drift",
                            "chat_template_drift",
                            "generation_defaults_drift",
                            "quantization_change",
                            "plan_format_upgrade"
                        ]
                    },
                    "kernel_cache": {
                        "scope": "process_local",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "invalidate",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "kernel_format_upgrade"
                        ]
                    },
                    "paged_tensor_storage": {
                        "scope": "artifact_backed",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "restore",
                        "invalidates_on": [
                            "binary_upgrade",
                            "model_metadata_change",
                            "quantization_change",
                            "paged_tensor_format_upgrade"
                        ]
                    },
                    "prefix_cache": {
                        "scope": "shared_across_requests",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "rebuild",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "model_metadata_change",
                            "tokenizer_drift",
                            "chat_template_drift",
                            "generation_defaults_drift",
                            "quantization_change",
                            "prefix_cache_format_upgrade"
                        ]
                    },
                    "kv_state": {
                        "scope": "session_bound",
                        "format_version": 1,
                        "compatible_action": "reuse",
                        "incompatible_action": "invalidate",
                        "invalidates_on": [
                            "binary_upgrade",
                            "backend_toolchain_upgrade",
                            "model_metadata_change",
                            "tokenizer_drift",
                            "chat_template_drift",
                            "generation_defaults_drift",
                            "quantization_change",
                            "kv_state_format_upgrade"
                        ]
                    }
                },
                "max_context": 8,
                "memory_plan": {
                    "weights_bytes": 0,
                    "kv_cache_bytes": 640,
                    "graph_bytes": 0,
                    "resident_host_bytes": 640,
                    "resident_device_bytes": 0
                },
                "residency_policy": {
                    "max_loaded_models": null,
                    "memory_budget": {
                        "resident_host_bytes": null,
                        "resident_device_bytes": null
                    },
                    "pressure_action": "refuse_new_model"
                },
                "streaming_policy": {
                    "backpressure": "pull_driven",
                    "disconnect": "abort_generation",
                    "cancellation": "abort_after_current_token"
                },
                "kv_cache_mode": "paged",
                "kv_cache_policy": {
                    "device_scope": "same_device_only",
                    "spill_policy": "refuse_new_pages",
                    "page_layout": {
                        "max_context_tokens": 8,
                        "tokens_per_page": 8,
                        "bytes_per_token": 80,
                        "page_bytes": 640,
                        "max_pages": 1
                    }
                },
                "kv_cache_encoding_policy": {
                    "family": "dense_f32",
                    "objective": "none",
                    "bits_per_channel": 32,
                    "model_family_bound": "fixture_decoder",
                    "context_length_bound": 8,
                    "host_bytes_per_token": 80,
                    "detail": "host-resident dense rows remain the active KV representation"
                },
                "supported_kv_cache_encoding_policies": [{
                    "family": "dense_f32",
                    "objective": "none",
                    "bits_per_channel": 32,
                    "model_family_bound": "fixture_decoder",
                    "context_length_bound": 8,
                    "host_bytes_per_token": 80,
                    "detail": "host-resident dense rows remain the active KV representation"
                }],
                "prefix_cache_policy": {
                    "shared_across_sessions": true,
                    "shared_across_users": false,
                    "shared_across_models": false,
                    "shared_across_backends": false,
                    "shared_across_sampler_settings": false
                },
                "execution_profile": {
                    "batch_posture": "single_request_only",
                    "queue_policy": {
                        "discipline": "direct_caller_backpressure",
                        "max_active_requests": 1,
                        "max_queued_requests": 0,
                        "per_model_serialization": true
                    },
                    "throughput_class": "latency_optimized",
                    "prefill_decode_capability": {
                        "supported_modes": ["disaggregated_colocated"],
                        "supported_transports": ["in_process_kv_state"],
                        "exposes_split_metrics": true,
                        "detail": "local text generation separates prompt-prefill and decode inside one runtime-owned KV seam"
                    }
                },
                "readiness": {
                    "status": "Ready",
                    "message": "cpu backend ready"
                }
            })
        );
        Ok(())
    }

    #[test]
    fn metal_gpt_oss_text_generation_capability_reports_explicit_validation()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_gpt_oss_decoder_descriptor();
        let envelope = TextGenerationCapabilityEnvelope::from_decoder_model(
            metal_backend_selection(),
            &model,
            default_decoder_memory_plan(&model, None, None),
            ModelResidencyPolicy::default(),
            KvCacheMode::Paged,
            default_text_generation_execution_profile(),
            ProviderReadiness::ready("metal backend ready"),
        );

        assert_eq!(
            envelope.validation.claim_id,
            "metal.gpt_oss.text_generation.apple_silicon"
        );
        assert_eq!(
            envelope.validation.coverage,
            ValidationCoverage::PositiveExecution
        );
        assert_eq!(envelope.model_family, "gpt-oss");
        assert_eq!(envelope.runtime_backend, "metal");
        assert_eq!(
            envelope
                .kv_cache_encoding_policy
                .as_ref()
                .map(|value| value.family),
            Some(KvCacheEncodingFamily::DenseF16Mirror)
        );
        assert!(
            envelope
                .supported_kv_cache_encoding_policies
                .iter()
                .any(|value| value.family == KvCacheEncodingFamily::DenseF16Mirror)
        );
        Ok(())
    }

    #[test]
    fn cuda_gpt_oss_text_generation_capability_keeps_turboquant_out_of_default_publication()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_gpt_oss_decoder_descriptor();
        let envelope = TextGenerationCapabilityEnvelope::from_decoder_model(
            cuda_backend_selection(),
            &model,
            default_decoder_memory_plan(&model, None, None),
            ModelResidencyPolicy::default(),
            KvCacheMode::Paged,
            default_text_generation_execution_profile(),
            ProviderReadiness::ready("cuda backend ready"),
        );

        assert_eq!(envelope.runtime_backend, "cuda");
        assert_eq!(
            envelope
                .kv_cache_encoding_policy
                .as_ref()
                .map(|value| value.family),
            Some(KvCacheEncodingFamily::DenseF16Mirror)
        );
        Ok(())
    }

    #[test]
    fn metal_gpt_oss_text_generation_fallback_capability_reports_explicit_refusal_validation()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_gpt_oss_decoder_descriptor();
        let envelope = TextGenerationCapabilityEnvelope::from_decoder_model(
            metal_fallback_selection(),
            &model,
            default_decoder_memory_plan(&model, None, None),
            ModelResidencyPolicy::default(),
            KvCacheMode::Paged,
            default_text_generation_execution_profile(),
            ProviderReadiness::ready("cpu fallback ready"),
        );

        assert_eq!(envelope.validation.claim_id, "metal.refusal.off_platform");
        assert_eq!(
            envelope.validation.coverage,
            ValidationCoverage::ExplicitRefusal
        );
        assert_eq!(envelope.model_family, "gpt-oss");
        assert_eq!(envelope.runtime_backend, "cpu");
        assert_eq!(envelope.backend_selection.requested_backend, "metal");
        assert_eq!(envelope.backend_selection.effective_backend, "cpu");
        Ok(())
    }

    #[test]
    fn compute_market_supply_refuses_unlicensed_local_path_artifacts()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let path = temp.path().join("byte_projection.safetensors");
        ByteProjectionEmbedder::write_default_safetensors_artifact(&path)?;
        let model = ByteProjectionEmbedder::from_safetensors_artifact(&path)?;

        let envelope = CapabilityEnvelope::from_embedding_model(
            cpu_backend_selection(),
            model.descriptor(),
            ProviderReadiness::ready("cpu backend ready"),
        );

        assert!(!envelope.supply_decision.advertise_allowed);
        assert!(!envelope.supply_decision.serve_allowed);
        assert!(matches!(
            envelope.supply_decision.violations.as_slice(),
            [
                super::ComputeMarketSupplyViolation {
                    code: ComputeMarketSupplyViolationCode::DisallowedProvenance,
                    ..
                },
                super::ComputeMarketSupplyViolation {
                    code: ComputeMarketSupplyViolationCode::MissingDeclaredLicense,
                    ..
                }
            ]
        ));

        let diagnostic = compute_market_supply_refusal_diagnostic(
            envelope.product_id.as_str(),
            envelope.model_id.as_str(),
            envelope.runtime_backend.as_str(),
            &envelope.supply_decision,
        )
        .expect("policy refusal diagnostic");
        assert_eq!(diagnostic.code, LocalRuntimeErrorCode::AdmissionRefused);
        assert_eq!(diagnostic.status, 403);
        assert!(
            diagnostic
                .message
                .contains("compute-market supply policy refused artifact")
        );
        Ok(())
    }

    #[test]
    fn compute_market_supply_allows_remote_alias_with_declared_license() {
        let mut model = sample_embedding_descriptor();
        model.weights.source = WeightSource::ExternalArtifact;
        model.weights.artifacts = vec![WeightArtifactMetadata::new("licensed.gguf", 16, "abc123")];
        model.artifact_governance = Some(ModelArtifactGovernance {
            provenance: ModelArtifactProvenance {
                kind: ModelArtifactProvenanceKind::OllamaRemoteAlias,
                source: String::from("registry.ollama.ai/library/qwen2:latest"),
                manifest_sha256: Some(String::from("manifest123")),
                remote_host: Some(String::from("cloud.example")),
                remote_model: Some(String::from("team/qwen2-licensed")),
                base_model: Some(String::from("qwen2-base")),
            },
            licenses: ModelArtifactLicenseFacts {
                declared: true,
                entries: vec![ModelArtifactLicenseEntry {
                    sha256: String::from("apache-digest"),
                    text: String::from("Apache-2.0"),
                }],
            },
        });

        let decision = evaluate_compute_market_supply(
            &model.weights,
            model.artifact_governance.as_ref(),
            &default_compute_market_supply_policy(),
        );
        assert!(decision.is_allowed());
        assert!(decision.violations.is_empty());
    }

    #[test]
    fn capability_envelope_preserves_backend_runtime_resources()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let envelope = CapabilityEnvelope::from_embedding_model(
            cpu_backend_selection()
                .with_runtime_resources(Some(sample_backend_runtime_resources()))
                .with_backend_extensions(vec![
                    BackendExtensionSupport::reference(BackendExtensionKind::RmsNorm),
                    BackendExtensionSupport::reference(BackendExtensionKind::RotaryEmbedding),
                ]),
            &model,
            ProviderReadiness::ready("cpu backend ready"),
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            encoded["validation"]["claim_id"],
            json!("cpu.embeddings.reference")
        );
        assert_eq!(
            encoded["validation"]["coverage"],
            json!("positive_execution")
        );
        assert_eq!(
            encoded["backend_selection"]["runtime_resources"]["allocator_pool"]["policy"]["max_cached_bytes"],
            json!(8 * 1024 * 1024)
        );
        assert_eq!(
            encoded["backend_selection"]["runtime_resources"]["kernel_cache"]["policy"]["enabled"],
            json!(false)
        );
        assert_eq!(
            encoded["backend_selection"]["runtime_resources"]["device_memory_budget"]["allocator_pool_budget_bytes"],
            json!(8 * 1024 * 1024)
        );
        assert_eq!(
            encoded["backend_selection"]["backend_extensions"],
            json!([
                {
                    "kind": "rms_norm",
                    "execution": "reference"
                },
                {
                    "kind": "rotary_embedding",
                    "execution": "reference"
                }
            ])
        );
        assert_eq!(
            encoded["backend_toolchain"]["compiled_backend_features"],
            json!(["rms_norm:reference", "rotary_embedding:reference"])
        );
        assert_eq!(
            encoded["backend_toolchain"]["probe_state"],
            json!("compiled_and_probed")
        );
        assert_eq!(
            encoded["selected_device_inventory"]["stable_device_id"],
            json!("cpu:0")
        );
        Ok(())
    }

    #[test]
    fn capability_envelope_can_surface_cluster_execution_context()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let cluster_execution = sample_cluster_execution_context();
        let envelope = CapabilityEnvelope::from_embedding_model(
            cpu_backend_selection(),
            &model,
            ProviderReadiness::ready("cluster lane ready"),
        )
        .with_cluster_execution(cluster_execution.clone());

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            encoded["cluster_execution"]["cluster_state_digest"],
            json!("cluster-state-digest")
        );
        assert_eq!(
            encoded["cluster_execution"]["commit_authority"]["fence_token"],
            json!("authority-fence-token")
        );
        assert_eq!(
            encoded["cluster_execution"]["policy_digests"][0]["kind"],
            json!("admission")
        );
        assert_eq!(
            encoded["cluster_execution"]["selected_nodes"][1]["artifact_residency"],
            json!("copy_required")
        );
        assert!(
            encoded["cluster_execution"]["communication_eligibility"]["capability_profile_digest"]
                .as_str()
                .is_some()
        );
        assert_eq!(
            encoded["cluster_execution"]["fallback_history"][0]["reason"],
            json!("node_unavailable")
        );
        assert_eq!(
            encoded["cluster_execution"]["serving_semantics"]["warm_route_posture"],
            json!("ready_node_selection")
        );
        assert_eq!(envelope.cluster_execution, Some(cluster_execution));
        Ok(())
    }

    #[test]
    fn capability_envelope_can_publish_declared_cluster_capability_profile_without_execution()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let envelope = CapabilityEnvelope::from_embedding_model(
            cpu_backend_selection(),
            &model,
            ProviderReadiness::ready("cluster capability advertised"),
        )
        .with_cluster_execution_capability_profile(
            ClusterExecutionCapabilityProfile::new("cpu")
                .with_supported_lanes(vec![ClusterExecutionLane::RemoteWholeRequest])
                .with_detail(
                    "backend `cpu` declares remote whole-request cluster dispatch for trusted operator lanes",
                ),
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["runtime_backend"],
            json!("cpu")
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_lanes"],
            json!(["remote_whole_request"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_communication_classes"],
            json!(["remote_dispatch"])
        );
        assert_eq!(encoded.get("cluster_execution"), None);
        assert!(envelope.cluster_execution.is_none());
        Ok(())
    }

    #[test]
    fn capability_envelope_can_publish_trusted_lan_cluster_trust_assessment_without_execution()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let trust_assessment = trusted_lan_cluster_trust_assessment();
        let envelope = CapabilityEnvelope::from_embedding_model(
            cpu_backend_selection(),
            &model,
            ProviderReadiness::ready("trusted-LAN cluster trust advertised"),
        )
        .with_cluster_compute_market_trust_assessment(trust_assessment.clone());

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            envelope
                .backend_selection
                .cluster_compute_market_trust_assessment,
            Some(trust_assessment.clone())
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_compute_market_trust_assessment"]["posture"],
            json!("trusted_lan_shared_admission")
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_compute_market_trust_assessment"]["disposition"],
            json!("refused")
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_compute_market_trust_assessment"]["refusal_reasons"],
            json!([
                "trusted_lan_shared_admission_only",
                "missing_authenticated_transport",
                "missing_attested_node_identity_admission",
                "missing_non_lan_discovery_posture"
            ])
        );
        assert_eq!(encoded.get("cluster_execution"), None);
        assert!(envelope.cluster_execution.is_none());
        Ok(())
    }

    #[test]
    fn capability_envelope_publishes_whole_request_cluster_profile_from_cluster_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let request = WholeRequestSchedulingRequest::new(sample_scheduler_node_id(), "cuda")
            .with_capability_profile(cuda_remote_dispatch_capability_profile());
        let envelope = CapabilityEnvelope::from_embedding_model(
            cuda_backend_selection(),
            &model,
            ProviderReadiness::ready("whole-request cluster capability advertised"),
        )
        .with_cluster_execution_capability_profile(request.capability_profile.clone());

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            envelope
                .backend_selection
                .cluster_execution_capability_profile,
            Some(request.capability_profile.clone())
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_lanes"],
            json!(["remote_whole_request"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_communication_classes"],
            json!(["remote_dispatch"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["serving_semantics_capabilities"]
                [0]["warm_route_posture"],
            json!("ready_node_selection")
        );
        assert!(envelope.cluster_execution.is_none());
        Ok(())
    }

    #[test]
    fn text_generation_capability_envelope_can_publish_attested_cluster_trust_assessment()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_decoder_descriptor();
        let trust_assessment = attested_cluster_trust_assessment();
        let envelope = TextGenerationCapabilityEnvelope::from_decoder_model(
            cuda_backend_selection(),
            &model,
            default_decoder_memory_plan(&model, None, None),
            ModelResidencyPolicy::default(),
            KvCacheMode::Paged,
            default_text_generation_execution_profile(),
            ProviderReadiness::ready("attested cluster trust advertised"),
        )
        .with_cluster_compute_market_trust_assessment(trust_assessment.clone());

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            envelope
                .backend_selection
                .cluster_compute_market_trust_assessment,
            Some(trust_assessment.clone())
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_compute_market_trust_assessment"]["posture"],
            json!("attested_configured_peers")
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_compute_market_trust_assessment"]["disposition"],
            json!("refused")
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_compute_market_trust_assessment"]["refusal_reasons"],
            json!(["missing_non_lan_discovery_posture"])
        );
        assert_eq!(
            trust_assessment.disposition,
            ClusterComputeMarketTrustDisposition::Refused
        );
        assert_eq!(
            trust_assessment.refusal_reasons,
            vec![ClusterComputeMarketTrustRefusalReason::MissingNonLanDiscoveryPosture]
        );
        assert!(envelope.cluster_execution.is_none());
        Ok(())
    }

    #[test]
    fn text_generation_capability_envelope_publishes_replica_profile_from_cluster_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_decoder_descriptor();
        let request = WholeRequestSchedulingRequest::new(sample_scheduler_node_id(), "cuda")
            .with_capability_profile(cuda_replica_routed_capability_profile());
        let envelope = TextGenerationCapabilityEnvelope::from_decoder_model(
            cuda_backend_selection(),
            &model,
            default_decoder_memory_plan(&model, None, None),
            ModelResidencyPolicy::default(),
            KvCacheMode::Paged,
            default_text_generation_execution_profile(),
            ProviderReadiness::ready("replicated cluster capability advertised"),
        )
        .with_cluster_execution_capability_profile(request.capability_profile.clone());

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            envelope
                .backend_selection
                .cluster_execution_capability_profile,
            Some(request.capability_profile.clone())
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_lanes"],
            json!(["remote_whole_request", "replica_routed"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_communication_classes"],
            json!(["remote_dispatch", "replica_routing"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["clustered_cache_capabilities"]
                [0]["prefix_scope"],
            json!("replica_local")
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["serving_semantics_capabilities"]
                [0]["warm_route_posture"],
            json!("route_pinned")
        );
        assert!(envelope.cluster_execution.is_none());
        Ok(())
    }

    #[test]
    fn text_generation_capability_envelope_publishes_layer_sharded_profile_from_cluster_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_decoder_descriptor();
        let request =
            LayerShardedExecutionRequest::new(sample_scheduler_node_id(), "served-artifact", 80, 2);
        let envelope = TextGenerationCapabilityEnvelope::from_decoder_model(
            cuda_backend_selection(),
            &model,
            default_decoder_memory_plan(&model, None, None),
            ModelResidencyPolicy::default(),
            KvCacheMode::Paged,
            default_text_generation_execution_profile(),
            ProviderReadiness::ready("layer-sharded cluster capability advertised"),
        )
        .with_cluster_execution_capability_profile(request.capability_profile.clone());

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            envelope
                .backend_selection
                .cluster_execution_capability_profile,
            Some(request.capability_profile.clone())
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_lanes"],
            json!(["remote_whole_request", "layer_sharded"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_communication_classes"],
            json!(["remote_dispatch", "layer_shard_handoff"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["clustered_cache_capabilities"]
                [0]["kv_scope"],
            json!("stage_local")
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["serving_semantics_capabilities"]
                [0]["warm_route_posture"],
            json!("topology_pinned")
        );
        assert!(envelope.cluster_execution.is_none());
        Ok(())
    }

    #[test]
    fn text_generation_capability_envelope_publishes_pipeline_sharded_profile_from_cluster_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_decoder_descriptor();
        let request = PipelineShardedExecutionRequest::new(
            sample_scheduler_node_id(),
            "served-artifact",
            80,
            2,
        );
        let envelope = TextGenerationCapabilityEnvelope::from_decoder_model(
            cuda_backend_selection(),
            &model,
            default_decoder_memory_plan(&model, None, None),
            ModelResidencyPolicy::default(),
            KvCacheMode::Paged,
            default_text_generation_execution_profile(),
            ProviderReadiness::ready("pipeline-sharded cluster capability advertised"),
        )
        .with_cluster_execution_capability_profile(request.capability_profile.clone());

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            envelope
                .backend_selection
                .cluster_execution_capability_profile,
            Some(request.capability_profile.clone())
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_lanes"],
            json!(["remote_whole_request", "pipeline_sharded"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_communication_classes"],
            json!(["remote_dispatch", "pipeline_stage_handoff"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["clustered_cache_capabilities"]
                [0]["invalidates_on_topology_change"],
            json!(true)
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["serving_semantics_capabilities"]
                [0]["warm_route_posture"],
            json!("topology_pinned")
        );
        assert!(envelope.cluster_execution.is_none());
        Ok(())
    }

    #[test]
    fn sandbox_capability_envelope_publishes_tensor_sharded_profile_from_cluster_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = TensorShardedExecutionRequest::new(
            sample_scheduler_node_id(),
            "served-artifact",
            TensorShardedModelEligibility::new(0, 1024),
            2,
        );
        let envelope = SandboxExecutionCapabilityEnvelope::new(
            cuda_backend_selection(),
            SandboxExecutionCapabilityProfile::bounded_accelerated("cuda", 2),
            ProviderReadiness::ready("tensor-sharded cluster capability advertised"),
        )
        .with_cluster_execution_capability_profile(request.capability_profile.clone());

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            envelope
                .backend_selection
                .cluster_execution_capability_profile,
            Some(request.capability_profile.clone())
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_lanes"],
            json!(["remote_whole_request", "tensor_sharded"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["supported_communication_classes"],
            json!(["remote_dispatch", "tensor_collective_mesh"])
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["clustered_cache_capabilities"]
                [0]["prefix_scope"],
            json!("stage_local")
        );
        assert_eq!(
            encoded["backend_selection"]["cluster_execution_capability_profile"]["serving_semantics_capabilities"]
                [0]["warm_route_posture"],
            json!("topology_pinned")
        );
        assert!(envelope.cluster_execution.is_none());
        Ok(())
    }

    #[test]
    fn capability_envelope_overrides_surface_for_replicated_cluster_execution()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let cluster_execution = sample_replicated_cluster_execution_context();
        let first = sample_cuda_device().inventory_qualifiers();
        let second = sample_cuda_device_1().inventory_qualifiers();
        let envelope = CapabilityEnvelope::from_embedding_model(
            cpu_backend_selection(),
            &model,
            ProviderReadiness::ready("replicated cluster lane ready"),
        )
        .with_cluster_execution(cluster_execution.clone());

        assert_eq!(
            envelope.execution_topology.as_ref().map(|plan| plan.kind),
            Some(psionic_runtime::ExecutionTopologyKind::Replicated)
        );
        assert_eq!(envelope.selected_devices.len(), 2);
        assert_eq!(
            envelope
                .selected_device_inventory
                .as_ref()
                .map(|device| { device.stable_device_id.as_str() }),
            Some(first.stable_device_id.as_str())
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["execution_topology"]["kind"], json!("replicated"));
        assert_eq!(
            encoded["selected_devices"][1]["stable_device_id"],
            json!(second.stable_device_id)
        );
        assert_eq!(
            encoded["cluster_execution"]["replica_nodes"][0]["routing"],
            json!("selected")
        );
        assert_eq!(
            encoded["cluster_execution"]["serving_semantics"]["warm_route_posture"],
            json!("route_pinned")
        );
        Ok(())
    }

    #[test]
    fn capability_envelope_without_probe_reports_compiled_only_toolchain_truth() {
        let model = sample_embedding_descriptor();
        let envelope = CapabilityEnvelope::from_embedding_model(
            BackendSelection::direct("cuda", None, dense_supported_ops()).with_backend_extensions(
                vec![BackendExtensionSupport::backend_specialized(
                    BackendExtensionKind::QuantizedMatmul,
                )],
            ),
            &model,
            ProviderReadiness::ready("cuda compiled but unprobed"),
        );

        assert_eq!(
            envelope.backend_toolchain,
            BackendToolchainIdentity {
                effective_backend: String::from("cuda"),
                toolchain_version: format!("cuda@{}", env!("CARGO_PKG_VERSION")),
                compiled_backend_features: vec![String::from(
                    "quantized_matmul:backend_specialized"
                )],
                probe_state: BackendProbeState::CompiledOnly,
                probed_backend_features: Vec::new(),
            }
        );
        assert_eq!(envelope.selected_device_inventory, None);
    }

    #[test]
    fn capability_envelope_can_surface_multi_device_topology_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let envelope = CapabilityEnvelope::from_embedding_model(
            cuda_multi_device_selection(),
            &model,
            ProviderReadiness::ready("cuda multi-device ready"),
        );
        assert_eq!(envelope.selected_devices.len(), 2);
        assert_eq!(
            envelope.execution_topology.as_ref().map(|plan| plan.kind),
            Some(ExecutionTopologyKind::LayerSharded)
        );
        assert_eq!(
            envelope
                .execution_topology
                .as_ref()
                .map(|plan| plan.assignments.len()),
            Some(2)
        );
        Ok(())
    }

    #[test]
    fn local_runtime_observability_envelope_serializes_stably()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = LocalRuntimeObservabilityEnvelope::new(LocalRuntimeObservability {
            isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
            cache_invalidation_policy: cache_invalidation_policy(),
            execution_profile: default_text_generation_execution_profile(),
            queue_depth: 0,
            queue_capacity: None,
            active_sessions: 2,
            active_requests: 1,
            memory_footprint: MemoryResidencySnapshot {
                loaded_models: 1,
                resident_host_bytes: 640,
                resident_device_bytes: 0,
            },
            backend_health: vec![psionic_runtime::BackendHealthObservation {
                backend: String::from("cpu"),
                status: HealthStatus::Ready,
                message: String::from("cpu backend ready"),
                observed_at_millis: 10,
                changed_at_millis: 10,
            }],
            recent_transitions: vec![
                RuntimeTransitionEvent::model(
                    RuntimeTransitionKind::ModelLoadedCold,
                    "fixture-word-decoder-v0",
                    5,
                ),
                RuntimeTransitionEvent {
                    kind: RuntimeTransitionKind::ModelBecameWarm,
                    model_id: Some(String::from("fixture-word-decoder-v0")),
                    backend: None,
                    previous_status: None,
                    status: None,
                    message: None,
                    observed_at_millis: 9,
                },
            ],
        });

        assert_eq!(
            serde_json::to_value(&envelope)?,
            json!({
                "backend_family": "psionic",
                "observability": {
                    "isolation_policy": {
                        "backend_interface_mode": "in_process",
                        "failure_boundary": "shared_host_process",
                        "request_failure_recovery": "refuse_request",
                        "backend_error_recovery": "reset_runtime_state",
                        "crash_recovery": "restart_host_process",
                        "reset_scopes": [
                            "loaded_models",
                            "sessions",
                            "prefix_cache",
                            "kv_state",
                            "backend_runtime_resources"
                        ]
                    },
                    "cache_invalidation_policy": {
                        "runtime_binary_version": env!("CARGO_PKG_VERSION"),
                        "execution_plan": {
                            "scope": "process_local",
                            "format_version": 1,
                            "compatible_action": "reuse",
                            "incompatible_action": "rebuild",
                            "invalidates_on": [
                                "binary_upgrade",
                                "backend_toolchain_upgrade",
                                "model_metadata_change",
                                "tokenizer_drift",
                                "chat_template_drift",
                                "generation_defaults_drift",
                                "quantization_change",
                                "plan_format_upgrade"
                            ]
                        },
                        "kernel_cache": {
                            "scope": "process_local",
                            "format_version": 1,
                            "compatible_action": "reuse",
                            "incompatible_action": "invalidate",
                            "invalidates_on": [
                                "binary_upgrade",
                                "backend_toolchain_upgrade",
                                "kernel_format_upgrade"
                            ]
                        },
                        "paged_tensor_storage": {
                            "scope": "artifact_backed",
                            "format_version": 1,
                            "compatible_action": "reuse",
                            "incompatible_action": "restore",
                            "invalidates_on": [
                                "binary_upgrade",
                                "model_metadata_change",
                                "quantization_change",
                                "paged_tensor_format_upgrade"
                            ]
                        },
                        "prefix_cache": {
                            "scope": "shared_across_requests",
                            "format_version": 1,
                            "compatible_action": "reuse",
                            "incompatible_action": "rebuild",
                            "invalidates_on": [
                                "binary_upgrade",
                                "backend_toolchain_upgrade",
                                "model_metadata_change",
                                "tokenizer_drift",
                                "chat_template_drift",
                                "generation_defaults_drift",
                                "quantization_change",
                                "prefix_cache_format_upgrade"
                            ]
                        },
                        "kv_state": {
                            "scope": "session_bound",
                            "format_version": 1,
                            "compatible_action": "reuse",
                            "incompatible_action": "invalidate",
                            "invalidates_on": [
                                "binary_upgrade",
                                "backend_toolchain_upgrade",
                                "model_metadata_change",
                                "tokenizer_drift",
                                "chat_template_drift",
                                "generation_defaults_drift",
                                "quantization_change",
                                "kv_state_format_upgrade"
                            ]
                        }
                    },
                    "execution_profile": {
                        "batch_posture": "single_request_only",
                        "queue_policy": {
                            "discipline": "direct_caller_backpressure",
                            "max_active_requests": 1,
                            "max_queued_requests": 0,
                            "per_model_serialization": true
                        },
                        "throughput_class": "latency_optimized",
                        "prefill_decode_capability": {
                            "supported_modes": ["disaggregated_colocated"],
                            "supported_transports": ["in_process_kv_state"],
                            "exposes_split_metrics": true,
                            "detail": "local text generation separates prompt-prefill and decode inside one runtime-owned KV seam"
                        }
                    },
                    "queue_depth": 0,
                    "active_sessions": 2,
                    "active_requests": 1,
                    "memory_footprint": {
                        "loaded_models": 1,
                        "resident_host_bytes": 640,
                        "resident_device_bytes": 0
                    },
                    "backend_health": [{
                        "backend": "cpu",
                        "status": "Ready",
                        "message": "cpu backend ready",
                        "observed_at_millis": 10,
                        "changed_at_millis": 10
                    }],
                    "recent_transitions": [
                        {
                            "kind": "model_loaded_cold",
                            "model_id": "fixture-word-decoder-v0",
                            "observed_at_millis": 5
                        },
                        {
                            "kind": "model_became_warm",
                            "model_id": "fixture-word-decoder-v0",
                            "observed_at_millis": 9
                        }
                    ]
                }
            })
        );
        Ok(())
    }

    #[test]
    fn fallback_capability_reports_requested_metal_but_effective_cpu()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = sample_embedding_descriptor();
        let envelope = CapabilityEnvelope::from_embedding_model(
            metal_fallback_selection(),
            &model,
            ProviderReadiness::ready("cpu fallback ready"),
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["runtime_backend"], json!("cpu"));
        assert_eq!(
            encoded["validation"]["claim_id"],
            json!("metal.refusal.off_platform")
        );
        assert_eq!(encoded["validation"]["coverage"], json!("explicit_refusal"));
        assert_eq!(
            encoded["backend_selection"]["requested_backend"],
            json!("metal")
        );
        assert_eq!(
            encoded["backend_selection"]["effective_backend"],
            json!("cpu")
        );
        assert_eq!(
            encoded["backend_selection"]["fallback_reason"],
            json!("metal backend unavailable: no supported Metal device")
        );
        assert_eq!(
            encoded["backend_selection"]["policy"],
            json!({
                "unavailable": "fallback_to_compatible_backend",
                "degraded": "allow_same_backend"
            })
        );
        assert_eq!(
            encoded["backend_selection"]["selection_state"],
            json!("cross_backend_fallback")
        );
        Ok(())
    }

    #[test]
    fn amd_kfd_capability_reports_mode_topology_and_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = CapabilityEnvelope::from_embedding_model(
            amd_kfd_selection(),
            &sample_embedding_descriptor(),
            ProviderReadiness {
                status: HealthStatus::Ready,
                message: String::from("amd_kfd ready on 1 AMD device"),
            },
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["runtime_backend"], json!("amd_kfd"));
        assert_eq!(
            encoded["validation"]["claim_id"],
            json!("amd_kfd.embeddings.not_yet_validated")
        );
        assert_eq!(
            encoded["validation"]["coverage"],
            json!("not_yet_validated")
        );
        assert_eq!(encoded["amd"]["mode"], json!("kfd"));
        assert_eq!(encoded["amd"]["topology"]["architecture"], json!("gfx1100"));
        assert_eq!(
            encoded["amd"]["risk"]["requires_explicit_opt_in"],
            json!(false)
        );
        assert_eq!(
            encoded["amd"]["recovery"]["expected_actions"],
            json!(["kernel_driver_reset", "reboot_host"])
        );
        Ok(())
    }

    #[test]
    fn amd_kfd_execution_capability_preserves_runtime_resources()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = CapabilityEnvelope::from_embedding_model(
            amd_kfd_execution_selection(),
            &sample_embedding_descriptor(),
            ProviderReadiness {
                status: HealthStatus::Ready,
                message: String::from("amd_kfd staging substrate ready"),
            },
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            encoded["backend_selection"]["runtime_resources"]["allocator_pool"]["policy"]["max_cached_buffers"],
            json!(64)
        );
        assert_eq!(
            encoded["backend_selection"]["runtime_resources"]["device_memory_budget"]["allocator_pool_budget_bytes"],
            json!(8 * 1024 * 1024u64)
        );
        assert_eq!(
            encoded["validation"]["claim_id"],
            json!("amd_kfd.embeddings.not_yet_validated")
        );
        Ok(())
    }

    #[test]
    fn amd_userspace_capability_reports_disabled_risk_posture()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = CapabilityEnvelope::from_embedding_model(
            amd_userspace_selection(),
            &sample_embedding_descriptor(),
            ProviderReadiness {
                status: HealthStatus::Offline,
                message: String::from("amd_userspace disabled pending explicit opt-in"),
            },
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["runtime_backend"], json!("amd_userspace"));
        assert_eq!(
            encoded["validation"]["claim_id"],
            json!("amd_userspace.embeddings.not_yet_validated")
        );
        assert_eq!(
            encoded["validation"]["coverage"],
            json!("not_yet_validated")
        );
        assert_eq!(encoded["amd"]["mode"], json!("userspace"));
        assert_eq!(
            encoded["amd"]["risk"]["requires_explicit_opt_in"],
            json!(true)
        );
        assert_eq!(
            encoded["amd"]["risk"]["may_unbind_kernel_driver"],
            json!(true)
        );
        assert_eq!(
            encoded["amd"]["recovery"]["driver_binding"],
            json!("kernel_amdgpu")
        );
        Ok(())
    }

    #[test]
    fn amd_userspace_execution_capability_preserves_runtime_resources()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = CapabilityEnvelope::from_embedding_model(
            amd_userspace_execution_selection(),
            &sample_embedding_descriptor(),
            ProviderReadiness {
                status: HealthStatus::Ready,
                message: String::from("amd_userspace staging substrate ready"),
            },
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(
            encoded["backend_selection"]["runtime_resources"]["kernel_cache"]["policy"]["enabled"],
            json!(false)
        );
        assert_eq!(encoded["amd"]["mode"], json!("userspace"));
        assert_eq!(
            encoded["validation"]["claim_id"],
            json!("amd_userspace.embeddings.not_yet_validated")
        );
        Ok(())
    }

    #[test]
    fn cuda_capability_reports_topology_risk_and_recovery_without_amd_context()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = CapabilityEnvelope::from_embedding_model(
            cuda_backend_selection(),
            &sample_embedding_descriptor(),
            ProviderReadiness::ready("cuda backend ready"),
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["runtime_backend"], json!("cuda"));
        assert_eq!(
            encoded["validation"]["claim_id"],
            json!("cuda.embeddings.nvidia")
        );
        assert_eq!(
            encoded["validation"]["coverage"],
            json!("positive_execution")
        );
        assert_eq!(
            encoded["backend_selection"]["requested_backend"],
            json!("cuda")
        );
        assert_eq!(
            encoded["backend_selection"]["effective_backend"],
            json!("cuda")
        );
        assert_eq!(
            encoded["backend_selection"]["selection_state"],
            json!("direct")
        );
        assert_eq!(
            encoded["backend_selection"]["selected_device"]["device"]["kind"],
            json!("Cuda")
        );
        assert_eq!(
            encoded["backend_selection"]["supported_ops"],
            json!(["input", "constant", "matmul", "add"])
        );
        assert_eq!(
            encoded["nvidia"]["topology"]["compute_capability"],
            json!("8.9")
        );
        assert_eq!(encoded["nvidia"]["risk"]["display_attached"], json!(false));
        assert_eq!(
            encoded["nvidia"]["recovery"]["expected_actions"],
            json!(["process_restart", "gpu_reset", "reboot_host"])
        );
        assert_eq!(encoded["amd"], serde_json::Value::Null);
        Ok(())
    }

    #[test]
    fn degraded_cuda_capability_reports_same_backend_degraded_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = CapabilityEnvelope::from_embedding_model(
            degraded_cuda_backend_selection(),
            &sample_embedding_descriptor(),
            ProviderReadiness {
                status: HealthStatus::Degraded,
                message: String::from(
                    "cuda discovered a display-attached GPU; local latency may vary",
                ),
            },
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["runtime_backend"], json!("cuda"));
        assert_eq!(
            encoded["validation"]["claim_id"],
            json!("cuda.embeddings.nvidia")
        );
        assert_eq!(
            encoded["backend_selection"]["selection_state"],
            json!("same_backend_degraded")
        );
        assert_eq!(
            encoded["backend_selection"]["degraded_reason"],
            json!("cuda discovered a display-attached GPU; local latency may vary")
        );
        assert_eq!(encoded["nvidia"]["risk"]["display_attached"], json!(true));
        assert_eq!(encoded["amd"], serde_json::Value::Null);
        Ok(())
    }

    #[test]
    fn cuda_fallback_capability_reports_requested_cuda_but_effective_cpu()
    -> Result<(), Box<dyn std::error::Error>> {
        let envelope = CapabilityEnvelope::from_embedding_model(
            cuda_fallback_selection(),
            &sample_embedding_descriptor(),
            ProviderReadiness::ready("cpu fallback ready"),
        );

        let encoded = serde_json::to_value(&envelope)?;
        assert_eq!(encoded["runtime_backend"], json!("cpu"));
        assert_eq!(
            encoded["validation"]["claim_id"],
            json!("cuda.refusal.unavailable")
        );
        assert_eq!(encoded["validation"]["coverage"], json!("explicit_refusal"));
        assert_eq!(
            encoded["backend_selection"]["requested_backend"],
            json!("cuda")
        );
        assert_eq!(
            encoded["backend_selection"]["effective_backend"],
            json!("cpu")
        );
        assert_eq!(
            encoded["backend_selection"]["selection_state"],
            json!("cross_backend_fallback")
        );
        assert_eq!(
            encoded["backend_selection"]["fallback_reason"],
            json!(
                "cuda backend unavailable: nvidia-smi is not installed or the NVIDIA driver is not reachable"
            )
        );
        assert_eq!(encoded["nvidia"], serde_json::Value::Null);
        assert_eq!(encoded["amd"], serde_json::Value::Null);
        Ok(())
    }

    #[test]
    fn execution_receipt_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-3",
            sample_embedding_descriptor(),
            vec![String::from("hello")],
        );
        let response = EmbeddingResponse::new(
            &request,
            vec![EmbeddingVector {
                index: 0,
                values: vec![0.1, 0.2, 0.3, 0.4],
                // Receipt tests do not care about matching model dimensions here.
            }],
        )
        .with_metrics(EmbeddingMetrics {
            total_duration_ns: Some(50),
            load_duration_ns: Some(5),
            prompt_eval_count: None,
            prompt_eval_duration_ns: None,
        });
        let receipt = ExecutionReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            10,
            20,
        );

        assert_eq!(receipt.status, ReceiptStatus::Succeeded);
        assert_eq!(receipt.validation.claim_id, "cpu.embeddings.reference");
        assert_eq!(
            receipt.validation.coverage,
            ValidationCoverage::PositiveExecution
        );
        assert_eq!(receipt.total_duration_ns, Some(50));
        assert_eq!(receipt.load_duration_ns, Some(5));
        let encoded = serde_json::to_string(&receipt)?;
        let decoded: ExecutionReceipt = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.runtime_backend, "cpu");
        assert_eq!(decoded.backend_selection.requested_backend, "cpu");
        assert_eq!(
            decoded.backend_toolchain.probe_state,
            BackendProbeState::CompiledAndProbed
        );
        assert_eq!(
            decoded.backend_toolchain.probed_backend_features,
            vec![String::from("host_memory")]
        );
        assert_eq!(
            decoded
                .selected_device_inventory
                .as_ref()
                .map(|value| value.performance_class),
            Some(psionic_runtime::DevicePerformanceClass::Reference)
        );
        assert_eq!(decoded.output_vector_count, 1);
        assert_eq!(decoded.input_count, 1);
        assert_eq!(decoded.normalization, EmbeddingNormalization::None);
        assert_eq!(
            decoded.proof_bundle.bundle_kind,
            ExecutionProofBundleKind::Local
        );
        assert_eq!(
            decoded
                .proof_bundle
                .validation
                .as_ref()
                .map(|value| value.claim_id.as_str()),
            Some("cpu.embeddings.reference")
        );
        assert_eq!(
            decoded.proof_bundle.activation_fingerprint_posture,
            ExecutionProofAugmentationPosture::Supported
        );
        assert_eq!(
            decoded
                .proof_bundle
                .activation_fingerprint
                .as_ref()
                .map(|value| value.scheme_id.as_str()),
            Some("psionic.activation_fingerprint.quantized.v1")
        );
        assert_eq!(
            decoded.proof_bundle.activation_fingerprint_ref.as_deref(),
            decoded
                .proof_bundle
                .activation_fingerprint
                .as_ref()
                .map(|value| value.artifact_digest.as_str())
        );
        assert_eq!(decoded.requested_output_dimensions, None);
        assert_eq!(decoded.failure_reason, None);
        assert_eq!(decoded.model_family, "smoke");
        assert_eq!(decoded.model_revision, "v0");
        assert_eq!(
            decoded.weight_bundle,
            WeightBundleEvidence::from_metadata(&request.model.weights)
        );
        Ok(())
    }

    #[test]
    fn embedding_execution_receipt_emits_activation_fingerprint_artifact()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "embed-proof-1",
            sample_embedding_descriptor(),
            vec![String::from("hello"), String::from("world")],
        );
        let response = EmbeddingResponse::new(
            &request,
            vec![
                EmbeddingVector {
                    index: 0,
                    values: vec![0.1, 0.2, 0.3, 0.4],
                },
                EmbeddingVector {
                    index: 1,
                    values: vec![0.5, 0.6, 0.7, 0.8],
                },
            ],
        );
        let receipt = ExecutionReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            10,
            20,
        );

        assert_eq!(
            receipt.proof_bundle.activation_fingerprint_posture,
            ExecutionProofAugmentationPosture::Supported
        );
        let artifact = receipt
            .proof_bundle
            .activation_fingerprint
            .as_ref()
            .expect("embedding receipts should carry an activation fingerprint");
        assert_eq!(
            artifact.scheme_id,
            "psionic.activation_fingerprint.quantized.v1"
        );
        assert_eq!(artifact.sample_count, 2);
        assert_eq!(
            receipt.proof_bundle.activation_fingerprint_ref.as_deref(),
            Some(artifact.artifact_digest.as_str())
        );
        assert!(artifact.is_self_consistent());
        Ok(())
    }

    #[test]
    fn execution_receipt_can_export_signed_cluster_evidence_bundle()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "embed-cluster-export-1",
            sample_embedding_descriptor(),
            vec![String::from("hello")],
        );
        let cluster_execution = sample_cluster_execution_context();
        let response = EmbeddingResponse::new(
            &request,
            vec![EmbeddingVector {
                index: 0,
                values: vec![0.1, 0.2, 0.3, 0.4],
            }],
        )
        .with_metrics_and_provenance(
            EmbeddingMetrics {
                total_duration_ns: Some(50),
                load_duration_ns: Some(5),
                prompt_eval_count: None,
                prompt_eval_duration_ns: None,
            },
            psionic_serve::EmbeddingProvenance {
                execution_plan_digest: String::from("embedding-cluster-plan"),
                cluster_execution: Some(cluster_execution.clone()),
                compile_path: None,
                delivery_proof: Some(ExecutionDeliveryProof {
                    execution_plan_digest: String::from("embedding-cluster-plan"),
                    kernel_count: 2,
                    bytes_moved: 512,
                    plan_cache_hits: 1,
                    plan_cache_misses: 0,
                    kv_growth: None,
                    prefill_decode_handoff: None,
                    kv_residency: None,
                }),
                cache_observations: Vec::new(),
            },
        );
        let receipt = ExecutionReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            10,
            20,
        );
        let signing_key = SigningKey::from_bytes(&[22; 32]);

        let Some(bundle) = receipt.signed_cluster_evidence_bundle("scheduler-node", &signing_key)
        else {
            return Err("clustered receipt should export a bundle".into());
        };

        assert_eq!(bundle.payload.cluster_execution, cluster_execution);
        assert_eq!(
            bundle
                .payload
                .delivery_proof
                .as_ref()
                .map(|value| value.execution_plan_digest.as_str()),
            Some("embedding-cluster-plan")
        );
        assert_eq!(
            bundle.payload.proof_bundle_digest.as_deref(),
            Some(receipt.proof_bundle.stable_digest().as_str())
        );
        assert!(bundle.verify().is_ok(), "bundle should verify");
        Ok(())
    }

    #[test]
    fn text_generation_receipt_preserves_cluster_execution_from_provenance()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-cluster-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-cluster-1")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let cluster_execution = sample_cluster_execution_context();
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(75),
                load_duration_ns: Some(25),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(15),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(60),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("cluster-plan"),
                cluster_execution: Some(cluster_execution.clone()),
                load_state: GenerationLoadState::Cold,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: Some(ExecutionDeliveryProof {
                    execution_plan_digest: String::from("cluster-plan"),
                    kernel_count: 2,
                    bytes_moved: 768,
                    plan_cache_hits: 1,
                    plan_cache_misses: 0,
                    kv_growth: None,
                    prefill_decode_handoff: None,
                    kv_residency: None,
                }),
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "cluster-plan",
            100,
            120,
        );

        assert_eq!(receipt.cluster_execution, Some(cluster_execution.clone()));
        assert_eq!(
            receipt
                .settlement_linkage
                .as_ref()
                .and_then(|value| value.cluster_provenance.as_ref())
                .map(|value| value.cluster_id.as_str()),
            Some("cluster-alpha")
        );
        assert_eq!(
            receipt
                .settlement_linkage
                .as_ref()
                .and_then(|value| value.cluster_provenance.as_ref())
                .map(|value| value.command_provenance.len()),
            Some(4)
        );
        let encoded = serde_json::to_value(&receipt)?;
        assert_eq!(
            encoded["cluster_execution"]["selected_nodes"][0]["node_id"],
            json!("worker-a")
        );
        assert_eq!(
            encoded["cluster_execution"]["degraded_reason"],
            json!("scheduler routed to the remaining healthy worker")
        );
        assert!(
            encoded["cluster_execution"]["communication_eligibility"]["capability_profile_digest"]
                .as_str()
                .is_some()
        );
        assert_eq!(
            encoded["settlement_linkage"]["cluster_provenance"]["command_provenance"][0]["fact_kind"],
            json!("scheduler_membership")
        );
        assert_eq!(
            encoded["settlement_linkage"]["cluster_provenance"]["coordinator_authority_digest"],
            json!("authority-digest")
        );
        assert_eq!(
            receipt.proof_bundle.bundle_kind,
            ExecutionProofBundleKind::Clustered
        );
        assert_eq!(
            receipt
                .proof_bundle
                .topology
                .as_ref()
                .and_then(|value| value.cluster_id.as_deref()),
            Some("cluster-alpha")
        );
        assert_eq!(
            serde_json::from_value::<TextGenerationReceipt>(encoded)?,
            receipt
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_preserves_psion_served_evidence_bundle()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-psion-evidence-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-psion-evidence-1")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let psion_served_evidence: PsionServedEvidenceBundle = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_served_evidence_direct_grounded_v1.json"
        ))?;
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(75),
                load_duration_ns: Some(25),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(15),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(60),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("psion-evidence-plan"),
                cluster_execution: None,
                load_state: GenerationLoadState::Cold,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: Some(ExecutionDeliveryProof {
                    execution_plan_digest: String::from("psion-evidence-plan"),
                    kernel_count: 2,
                    bytes_moved: 768,
                    plan_cache_hits: 1,
                    plan_cache_misses: 0,
                    kv_growth: None,
                    prefill_decode_handoff: None,
                    kv_residency: None,
                }),
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: Some(psion_served_evidence.clone()),
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "psion-evidence-plan",
            100,
            120,
        );

        assert_eq!(
            receipt.psion_served_evidence,
            Some(psion_served_evidence.clone())
        );
        let encoded = serde_json::to_value(&receipt)?;
        assert_eq!(
            encoded["psion_served_evidence"]["bundle_id"],
            json!("psion-served-evidence-direct-grounded-v1")
        );
        assert_eq!(
            serde_json::from_value::<TextGenerationReceipt>(encoded)?,
            receipt
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_preserves_psion_served_output_claim_posture()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-psion-claim-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-psion-claim-1")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let psion_served_evidence: PsionServedEvidenceBundle = serde_json::from_str(include_str!(
            "../../../fixtures/psion/serve/psion_served_evidence_direct_grounded_v1.json"
        ))?;
        let psion_served_output_claim_posture: PsionServedOutputClaimPosture =
            serde_json::from_str(include_str!(
                "../../../fixtures/psion/serve/psion_served_output_claim_direct_v1.json"
            ))?;
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(75),
                load_duration_ns: Some(25),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(15),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(60),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("psion-claim-plan"),
                cluster_execution: None,
                load_state: GenerationLoadState::Cold,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: Some(ExecutionDeliveryProof {
                    execution_plan_digest: String::from("psion-claim-plan"),
                    kernel_count: 2,
                    bytes_moved: 768,
                    plan_cache_hits: 1,
                    plan_cache_misses: 0,
                    kv_growth: None,
                    prefill_decode_handoff: None,
                    kv_residency: None,
                }),
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: Some(psion_served_evidence),
                psion_served_output_claim_posture: Some(psion_served_output_claim_posture.clone()),
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "psion-claim-plan",
            100,
            120,
        );

        assert_eq!(
            receipt.psion_served_output_claim_posture,
            Some(psion_served_output_claim_posture.clone())
        );
        let encoded = serde_json::to_value(&receipt)?;
        assert_eq!(
            encoded["psion_served_output_claim_posture"]["posture_id"],
            json!("psion-served-output-claim-direct-v1")
        );
        assert_eq!(
            serde_json::from_value::<TextGenerationReceipt>(encoded)?,
            receipt
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_can_export_signed_cluster_evidence_bundle()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-cluster-export-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-cluster-export-1")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let cluster_execution = sample_cluster_execution_context();
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(75),
                load_duration_ns: Some(25),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(15),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(60),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("cluster-plan-export"),
                cluster_execution: Some(cluster_execution.clone()),
                load_state: GenerationLoadState::Cold,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: Some(ExecutionDeliveryProof {
                    execution_plan_digest: String::from("cluster-plan-export"),
                    kernel_count: 2,
                    bytes_moved: 768,
                    plan_cache_hits: 1,
                    plan_cache_misses: 0,
                    kv_growth: None,
                    prefill_decode_handoff: None,
                    kv_residency: None,
                }),
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "cluster-plan-export",
            100,
            120,
        );
        let signing_key = SigningKey::from_bytes(&[21; 32]);

        let Some(bundle) = receipt.signed_cluster_evidence_bundle("scheduler-node", &signing_key)
        else {
            return Err("clustered receipt should export a bundle".into());
        };

        assert_eq!(bundle.payload.cluster_execution, cluster_execution);
        assert_eq!(
            bundle
                .payload
                .settlement_linkage
                .as_ref()
                .and_then(|value| value.cluster_provenance.as_ref())
                .map(|value| value.command_provenance.len()),
            Some(4)
        );
        assert_eq!(
            bundle.payload.proof_bundle_digest.as_deref(),
            Some(receipt.proof_bundle.stable_digest().as_str())
        );
        assert!(bundle.verify().is_ok(), "bundle should verify");
        Ok(())
    }

    #[test]
    fn text_generation_receipt_uses_adapter_binding_for_settlement_linkage()
    -> Result<(), Box<dyn std::error::Error>> {
        let adapter_serving = sample_adapter_serving_binding();
        let request = GenerationRequest::new_text(
            "gen-adapter-receipt-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-adapter-receipt-1")),
            "hello",
            GenerationOptions::greedy(2),
        )
        .with_adapter_serving(adapter_serving.clone());
        let served_artifact =
            served_artifact_identity_for_decoder_model(&request.model, &cpu_backend_selection());
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(75),
                load_duration_ns: Some(25),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(15),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(60),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact,
                adapter_serving: Some(adapter_serving.clone()),
                served_revision: None,
                execution_plan_digest: String::from("adapter-plan"),
                cluster_execution: None,
                load_state: GenerationLoadState::Cold,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: Some(ExecutionDeliveryProof {
                    execution_plan_digest: String::from("adapter-plan"),
                    kernel_count: 2,
                    bytes_moved: 768,
                    plan_cache_hits: 1,
                    plan_cache_misses: 0,
                    kv_growth: None,
                    prefill_decode_handoff: None,
                    kv_residency: None,
                }),
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "adapter-plan",
            100,
            120,
        );

        assert_eq!(receipt.product_id, "psionic.adapter_text_generation");
        assert_eq!(receipt.adapter_serving, Some(adapter_serving.clone()));
        assert_eq!(
            receipt
                .settlement_linkage
                .as_ref()
                .map(|value| value.served_artifact_digest.as_str()),
            Some(adapter_serving.served_adapter_digest.as_str())
        );

        let encoded = serde_json::to_value(&receipt)?;
        assert_eq!(
            encoded["adapter_serving"]["served_adapter_digest"],
            json!(adapter_serving.served_adapter_digest)
        );
        assert_eq!(
            encoded["settlement_linkage"]["served_artifact_digest"],
            json!(adapter_serving.served_adapter_digest)
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_surfaces_replicated_cluster_execution_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-cluster-replicated-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-cluster-replicated-1")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let cluster_execution = sample_replicated_cluster_execution_context();
        let first = sample_cuda_device().inventory_qualifiers();
        let second = sample_cuda_device_1().inventory_qualifiers();
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(75),
                load_duration_ns: Some(25),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(15),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(60),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("cluster-plan-replicated"),
                cluster_execution: Some(cluster_execution.clone()),
                load_state: GenerationLoadState::Warm,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: None,
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "cluster-plan-replicated",
            10,
            85,
        );

        assert_eq!(
            receipt.execution_topology.as_ref().map(|plan| plan.kind),
            Some(psionic_runtime::ExecutionTopologyKind::Replicated)
        );
        assert_eq!(receipt.selected_devices.len(), 2);
        assert_eq!(
            receipt
                .selected_device_inventory
                .as_ref()
                .map(|device| { device.stable_device_id.as_str() }),
            Some(first.stable_device_id.as_str())
        );
        assert_eq!(receipt.cluster_execution, Some(cluster_execution.clone()));

        let encoded = serde_json::to_value(&receipt)?;
        assert_eq!(encoded["execution_topology"]["kind"], json!("replicated"));
        assert_eq!(
            encoded["cluster_execution"]["replica_state_digest"],
            json!("replica-state-digest")
        );
        assert_eq!(
            encoded["cluster_execution"]["sharded_model_manifest_digest"],
            json!("replica-manifest-digest")
        );
        assert_eq!(
            encoded["cluster_execution"]["policy_digests"][0]["kind"],
            json!("replication")
        );
        assert_eq!(
            encoded["cluster_execution"]["clustered_cache_usage"]["prefix_action"],
            json!("reuse")
        );
        assert_eq!(
            encoded["cluster_execution"]["serving_semantics"]["warm_route_posture"],
            json!("route_pinned")
        );
        assert_eq!(
            encoded["selected_devices"][1]["stable_device_id"],
            json!(second.stable_device_id)
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_surfaces_layer_sharded_cluster_execution_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-cluster-sharded-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-cluster-sharded-1")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let cluster_execution = sample_layer_sharded_cluster_execution_context();
        let first = sample_cuda_device().inventory_qualifiers();
        let second = sample_cuda_device_1().inventory_qualifiers();
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(75),
                load_duration_ns: Some(25),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(15),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(60),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("cluster-plan-layer-sharded"),
                cluster_execution: Some(cluster_execution.clone()),
                load_state: GenerationLoadState::Warm,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: None,
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "cluster-plan-layer-sharded",
            10,
            85,
        );

        assert_eq!(
            receipt.execution_topology.as_ref().map(|plan| plan.kind),
            Some(psionic_runtime::ExecutionTopologyKind::LayerSharded)
        );
        assert_eq!(receipt.selected_devices.len(), 2);
        assert_eq!(
            receipt
                .selected_device_inventory
                .as_ref()
                .map(|device| { device.stable_device_id.as_str() }),
            Some(first.stable_device_id.as_str())
        );
        assert_eq!(receipt.cluster_execution, Some(cluster_execution.clone()));

        let encoded = serde_json::to_value(&receipt)?;
        assert_eq!(
            encoded["execution_topology"]["kind"],
            json!("layer_sharded")
        );
        assert_eq!(
            encoded["cluster_execution"]["policy_digests"][0]["kind"],
            json!("sharding")
        );
        assert_eq!(
            encoded["cluster_execution"]["sharded_model_manifest_digest"],
            json!("layer-manifest-digest")
        );
        assert_eq!(
            encoded["cluster_execution"]["clustered_cache_usage"]["kv_scope"],
            json!("stage_local")
        );
        assert_eq!(
            encoded["cluster_execution"]["serving_semantics"]["warm_route_posture"],
            json!("topology_pinned")
        );
        assert_eq!(
            encoded["cluster_execution"]["shard_handoffs"][0]["kind"],
            json!("activation")
        );
        assert_eq!(
            encoded["cluster_execution"]["shard_handoffs"][1]["estimated_bytes_per_token"],
            json!(4096)
        );
        assert_eq!(
            encoded["selected_devices"][1]["stable_device_id"],
            json!(second.stable_device_id)
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_surfaces_pipeline_sharded_cluster_execution_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-cluster-pipeline-sharded-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-cluster-pipeline-sharded-1")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let cluster_execution = sample_pipeline_sharded_cluster_execution_context();
        let first = sample_cuda_device().inventory_qualifiers();
        let second = sample_cuda_device_1().inventory_qualifiers();
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(95),
                load_duration_ns: Some(40),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(25),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(70),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("cluster-plan-pipeline-sharded"),
                cluster_execution: Some(cluster_execution.clone()),
                load_state: GenerationLoadState::Warm,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: None,
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "cluster-plan-pipeline-sharded",
            10,
            85,
        );

        assert_eq!(
            receipt.execution_topology.as_ref().map(|plan| plan.kind),
            Some(psionic_runtime::ExecutionTopologyKind::PipelineSharded)
        );
        assert_eq!(receipt.selected_devices.len(), 2);
        assert_eq!(receipt.cluster_execution, Some(cluster_execution.clone()));

        let encoded = serde_json::to_value(&receipt)?;
        assert_eq!(
            encoded["execution_topology"]["kind"],
            json!("pipeline_sharded")
        );
        assert_eq!(
            encoded["cluster_execution"]["pipeline_stages"][0]["role"],
            json!("entry")
        );
        assert_eq!(
            encoded["cluster_execution"]["pipeline_stages"][0]["handoff_transport"],
            json!("wider_network_stream")
        );
        assert_eq!(
            encoded["cluster_execution"]["pipeline_stages"][0]["handoff_latency_ms"],
            json!(32)
        );
        assert_eq!(
            encoded["cluster_execution"]["sharded_model_manifest_digest"],
            json!("pipeline-manifest-digest")
        );
        assert_eq!(
            encoded["cluster_execution"]["clustered_cache_usage"]["prefix_action"],
            json!("bypass")
        );
        assert_eq!(
            encoded["cluster_execution"]["serving_semantics"]["warm_route_posture"],
            json!("topology_pinned")
        );
        assert_eq!(
            encoded["selected_devices"][0]["stable_device_id"],
            json!(first.stable_device_id)
        );
        assert_eq!(
            encoded["selected_devices"][1]["stable_device_id"],
            json!(second.stable_device_id)
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_surfaces_gemma4_pipeline_sharded_cluster_execution_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-gemma4-cluster-pipeline-sharded-1",
            sample_gemma4_decoder_descriptor(),
            Some(SessionId::new("sess-gemma4-cluster-pipeline-sharded-1")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let cluster_execution = sample_pipeline_sharded_cluster_execution_context();
        let first = sample_cuda_device().inventory_qualifiers();
        let second = sample_cuda_device_1().inventory_qualifiers();
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(120),
                load_duration_ns: Some(35),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(24),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(86),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("gemma4-cluster-plan-pipeline-sharded"),
                cluster_execution: Some(cluster_execution.clone()),
                load_state: GenerationLoadState::Warm,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: None,
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "gemma4-cluster-plan-pipeline-sharded",
            10,
            120,
        );

        assert_eq!(receipt.model_id, "gemma4:e4b");
        assert_eq!(receipt.model_family, "gemma4");
        assert_eq!(
            receipt.execution_topology.as_ref().map(|plan| plan.kind),
            Some(psionic_runtime::ExecutionTopologyKind::PipelineSharded)
        );
        assert_eq!(receipt.cluster_execution, Some(cluster_execution.clone()));

        let encoded = serde_json::to_value(&receipt)?;
        assert_eq!(encoded["model_id"], json!("gemma4:e4b"));
        assert_eq!(encoded["model_family"], json!("gemma4"));
        assert_eq!(
            encoded["execution_topology"]["kind"],
            json!("pipeline_sharded")
        );
        assert_eq!(
            encoded["cluster_execution"]["disposition"],
            json!("sharded")
        );
        assert_eq!(
            encoded["cluster_execution"]["pipeline_stages"][0]["role"],
            json!("entry")
        );
        assert_eq!(
            encoded["cluster_execution"]["selected_nodes"][0]["device_inventory"]["stable_device_id"],
            json!(first.stable_device_id)
        );
        assert_eq!(
            encoded["cluster_execution"]["selected_nodes"][1]["device_inventory"]["stable_device_id"],
            json!(second.stable_device_id)
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_surfaces_tensor_sharded_cluster_execution_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-cluster-tensor-sharded-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-cluster-tensor-sharded-1")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let cluster_execution = sample_tensor_sharded_cluster_execution_context();
        let first = sample_cuda_device().inventory_qualifiers();
        let second = sample_cuda_device_1().inventory_qualifiers();
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(75),
                load_duration_ns: Some(25),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(15),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(60),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("cluster-plan-tensor-sharded"),
                cluster_execution: Some(cluster_execution.clone()),
                load_state: GenerationLoadState::Warm,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: None,
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "cluster-plan-tensor-sharded",
            10,
            85,
        );

        assert_eq!(
            receipt.execution_topology.as_ref().map(|plan| plan.kind),
            Some(psionic_runtime::ExecutionTopologyKind::TensorSharded)
        );
        assert_eq!(receipt.selected_devices.len(), 2);
        assert_eq!(receipt.cluster_execution, Some(cluster_execution.clone()));

        let encoded = serde_json::to_value(&receipt)?;
        assert_eq!(
            encoded["execution_topology"]["kind"],
            json!("tensor_sharded")
        );
        assert_eq!(
            encoded["cluster_execution"]["policy_digests"][0]["kind"],
            json!("sharding")
        );
        assert_eq!(
            encoded["cluster_execution"]["sharded_model_manifest_digest"],
            json!("tensor-manifest-digest")
        );
        assert_eq!(
            encoded["cluster_execution"]["clustered_cache_usage"]["prefix_scope"],
            json!("stage_local")
        );
        assert_eq!(
            encoded["cluster_execution"]["serving_semantics"]["warm_route_posture"],
            json!("topology_pinned")
        );
        assert_eq!(
            encoded["cluster_execution"]["shard_handoffs"][0]["kind"],
            json!("tensor_collective")
        );
        assert_eq!(
            encoded["cluster_execution"]["shard_handoffs"][0]["tensor_axis"],
            json!(1)
        );
        assert_eq!(
            encoded["cluster_execution"]["shard_handoffs"][0]["tensor_range_start"],
            json!(0)
        );
        assert_eq!(
            encoded["cluster_execution"]["shard_handoffs"][0]["tensor_range_end"],
            json!(32)
        );
        assert_eq!(
            encoded["selected_devices"][1]["stable_device_id"],
            json!(second.stable_device_id)
        );
        assert_eq!(
            receipt
                .selected_device_inventory
                .as_ref()
                .map(|device| device.stable_device_id.as_str()),
            Some(first.stable_device_id.as_str())
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "gen-3",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000003")),
            "hello",
            GenerationOptions::greedy(2),
        );
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            psionic_serve::TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(75),
                load_duration_ns: Some(25),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(15),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(60),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: Some(KvCacheAccounting {
                    current: psionic_runtime::KvCacheState {
                        tokens: 2,
                        bytes: 64,
                        pages: 1,
                    },
                    growth: psionic_runtime::KvCacheGrowth {
                        tokens: 2,
                        bytes: 64,
                        pages: 1,
                    },
                }),
                kv_residency: Some(
                    KvResidencyAccounting::from_policy(&default_decoder_kv_cache_policy(
                        &request.model,
                    ))
                    .with_tier(KvResidencyTierState::resident(
                        KvResidencyTier::Host,
                        psionic_runtime::KvCacheState {
                            tokens: 2,
                            bytes: 64,
                            pages: 1,
                        },
                    )),
                ),
                kv_cache_encoding: Some(KvCacheEncodingAccounting::active(
                    default_decoder_kv_cache_encoding_policy(&request.model, "cpu"),
                )),
                prefix_tokens_reused: Some(1),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("plan-digest-from-response"),
                cluster_execution: None,
                load_state: GenerationLoadState::Cold,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: Some(MemoryResidencySnapshot {
                    loaded_models: 1,
                    resident_host_bytes: 640,
                    resident_device_bytes: 0,
                }),
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: Some(default_decoder_kv_cache_encoding_policy(
                    &request.model,
                    "cpu",
                )),
                prefix_cache_state: Some(PrefixCacheState::Hit),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: Some(PrefixCacheIdentity {
                    served_artifact_digest: served_artifact_identity_for_decoder_model(
                        &request.model,
                        &cpu_backend_selection(),
                    )
                    .served_artifact_digest,
                    model_id: request.model.model.model_id.clone(),
                    model_revision: request.model.model.revision.clone(),
                    weight_bundle_digest: request.model.weights.digest.clone(),
                    tokenizer_family: request.model.tokenizer_family.clone(),
                    tokenizer_digest: Some(String::from("tokenizer-digest")),
                    chat_template_digest: None,
                    generation_defaults_digest: None,
                    tenant_id: None,
                    sampler_digest: None,
                    backend_compatibility: String::from("cpu"),
                    prefix_digest: String::from("prefix-digest"),
                    prefix_tokens: 1,
                }),
                compile_path: None,
                delivery_proof: Some(ExecutionDeliveryProof {
                    execution_plan_digest: String::from("plan-digest-from-response"),
                    kernel_count: 2,
                    bytes_moved: 512,
                    plan_cache_hits: 1,
                    plan_cache_misses: 0,
                    kv_growth: Some(psionic_runtime::KvCacheGrowth {
                        tokens: 2,
                        bytes: 64,
                        pages: 1,
                    }),
                    prefill_decode_handoff: None,
                    kv_residency: None,
                }),
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cpu_backend_selection(),
            &request,
            &response,
            "plan-digest-1",
            100,
            120,
        );

        assert_eq!(receipt.status, ReceiptStatus::Succeeded);
        assert_eq!(receipt.validation.claim_id, "cpu.text_generation.reference");
        assert_eq!(
            receipt.validation.coverage,
            ValidationCoverage::PositiveExecution
        );
        assert_eq!(receipt.output_tokens, 1);
        assert_eq!(
            receipt.termination,
            Some(psionic_serve::TerminationReason::EndOfSequence)
        );
        assert_eq!(
            receipt.execution_plan_digest.as_deref(),
            Some("plan-digest-from-response")
        );
        assert_eq!(receipt.total_duration_ns, Some(75));
        assert_eq!(receipt.load_duration_ns, Some(25));
        assert_eq!(receipt.prompt_eval_duration_ns, Some(15));
        assert_eq!(receipt.eval_duration_ns, Some(60));
        assert_eq!(receipt.load_state, Some(GenerationLoadState::Cold));
        assert_eq!(
            receipt.memory_plan,
            Some(default_decoder_memory_plan(&request.model, None, None))
        );
        assert_eq!(
            receipt.residency_policy,
            Some(ModelResidencyPolicy::default())
        );
        assert_eq!(
            receipt.residency_snapshot,
            Some(MemoryResidencySnapshot {
                loaded_models: 1,
                resident_host_bytes: 640,
                resident_device_bytes: 0,
            })
        );
        assert_eq!(receipt.streaming_policy, None);
        assert_eq!(
            receipt.kv_cache_policy,
            Some(default_decoder_kv_cache_policy(&request.model))
        );
        assert_eq!(
            receipt.kv_cache_encoding_policy,
            Some(default_decoder_kv_cache_encoding_policy(
                &request.model,
                "cpu"
            ))
        );
        assert_eq!(
            receipt.kv_cache.as_ref().map(|value| value.current.pages),
            Some(1)
        );
        assert_eq!(
            receipt
                .kv_cache_encoding
                .as_ref()
                .map(|value| value.active.family),
            Some(KvCacheEncodingFamily::DenseF32)
        );
        assert!(
            receipt
                .kv_residency
                .as_ref()
                .is_some_and(|value| value.has_tier(KvResidencyTier::Host))
        );
        assert_eq!(receipt.prefix_cache_state, Some(PrefixCacheState::Hit));
        assert_eq!(
            receipt.prefix_cache_policy,
            Some(default_prefix_cache_policy())
        );
        assert_eq!(receipt.prefix_tokens_reused, Some(1));
        assert_eq!(
            receipt
                .prefix_cache_identity
                .as_ref()
                .map(|value| value.prefix_digest.as_str()),
            Some("prefix-digest")
        );
        assert_eq!(
            receipt
                .delivery_proof
                .as_ref()
                .map(|value| value.kernel_count),
            Some(2)
        );
        assert_eq!(
            receipt
                .delivery_proof
                .as_ref()
                .map(|value| value.bytes_moved),
            Some(512)
        );
        assert_eq!(
            receipt
                .settlement_linkage
                .as_ref()
                .map(|value| value.request_digest.as_str()),
            Some(digest_generation_request(&request).as_str())
        );
        assert_eq!(
            receipt
                .settlement_linkage
                .as_ref()
                .map(|value| value.execution_plan_digest.as_str()),
            Some("plan-digest-from-response")
        );
        assert_eq!(
            receipt
                .settlement_linkage
                .as_ref()
                .and_then(|value| value.output_tokens),
            Some(1)
        );
        let encoded = serde_json::to_string(&receipt)?;
        let decoded: TextGenerationReceipt = serde_json::from_str(&encoded)?;
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.runtime_backend, "cpu");
        assert_eq!(decoded.backend_selection.requested_backend, "cpu");
        assert_eq!(
            decoded.backend_toolchain.probe_state,
            BackendProbeState::CompiledAndProbed
        );
        assert_eq!(
            decoded.backend_toolchain.probed_backend_features,
            vec![String::from("host_memory")]
        );
        assert_eq!(
            decoded
                .selected_device_inventory
                .as_ref()
                .map(|value| value.performance_class),
            Some(psionic_runtime::DevicePerformanceClass::Reference)
        );
        assert_eq!(decoded.model_family, "fixture_decoder");
        assert_eq!(decoded.model_revision, "v0");
        assert_eq!(
            decoded.weight_bundle,
            WeightBundleEvidence::from_metadata(&request.model.weights)
        );
        Ok(())
    }

    #[test]
    fn text_generation_receipt_preserves_explicit_turboquant_downgrade()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "turboquant-downgrade",
            sample_gpt_oss_decoder_descriptor(),
            None,
            "hello",
            GenerationOptions::greedy(1),
        );
        let kv_width = request.model.config.kv_width();
        let requested_turboquant_policy = KvCacheEncodingPolicy {
            family: KvCacheEncodingFamily::TurboQuant,
            objective: Some(KvCacheEncodingObjective::MeanSquaredError),
            bits_per_channel: Some(8),
            block_shape: Some(String::from("32")),
            outlier_policy: None,
            projection_id: None,
            codebook_id: Some(String::from("ggml_q8_1")),
            model_family_bound: Some(request.model.model.family.clone()),
            context_length_bound: Some(request.model.config.max_context),
            host_bytes_per_token: Some(
                kv_width
                    .saturating_mul(2)
                    .saturating_mul(std::mem::size_of::<f32>())
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            device_bytes_per_token: Some(
                kv_width
                    .saturating_div(32)
                    .saturating_mul(36)
                    .saturating_mul(2)
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            detail: Some(String::from(
                "experimental CUDA TurboQuant prototype backed by ggml_q8_1 device KV rows",
            )),
        };
        let active_dense_policy = default_decoder_kv_cache_encoding_policy(&request.model, "cuda");
        let response = GenerationResponse::new(
            &request,
            None,
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "hi",
            1,
            1,
            TerminationReason::EndOfSequence,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(25),
                load_duration_ns: Some(5),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(10),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(15),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: Some(KvCacheEncodingAccounting {
                    requested: Some(requested_turboquant_policy.clone()),
                    active: active_dense_policy.clone(),
                    downgraded: true,
                    refusal_reason: Some(String::from(
                        "TurboQuant KV only runs on CUDA GPT-OSS decode paths, active backend `cpu`",
                    )),
                }),
                prefix_tokens_reused: None,
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cuda_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("turboquant-downgrade-plan"),
                cluster_execution: None,
                load_state: GenerationLoadState::Warm,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: None,
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: None,
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: Some(active_dense_policy.clone()),
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_state: None,
                prefix_cache_refusal_reason: None,
                prefix_cache_policy: None,
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: None,
                cache_observations: Vec::new(),
                scheduler: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let receipt = TextGenerationReceipt::succeeded_for_response(
            cuda_backend_selection(),
            &request,
            &response,
            String::from("plan123"),
            10,
            20,
        );

        assert_eq!(
            receipt.kv_cache_encoding_policy,
            Some(active_dense_policy.clone())
        );
        assert_eq!(
            receipt
                .kv_cache_encoding
                .as_ref()
                .and_then(|value| value.requested.as_ref())
                .map(|value| value.family),
            Some(KvCacheEncodingFamily::TurboQuant)
        );
        assert_eq!(
            receipt
                .kv_cache_encoding
                .as_ref()
                .map(|value| value.active.family),
            Some(KvCacheEncodingFamily::DenseF16Mirror)
        );
        assert!(
            receipt
                .kv_cache_encoding
                .as_ref()
                .is_some_and(|value| value.downgraded)
        );
        assert!(
            receipt
                .kv_cache_encoding
                .as_ref()
                .and_then(|value| value.refusal_reason.as_deref())
                .unwrap_or_default()
                .contains("only runs on CUDA")
        );
        Ok(())
    }

    #[test]
    fn metal_gpt_oss_text_generation_receipt_reports_explicit_validation()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "metal-gpt-oss-req",
            sample_gpt_oss_decoder_descriptor(),
            Some(SessionId::new("metal-gpt-oss-session")),
            "hello",
            GenerationOptions::greedy(1),
        );
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "hi",
            1,
            1,
            TerminationReason::EndOfSequence,
        );

        let receipt = TextGenerationReceipt::succeeded_for_response(
            metal_backend_selection(),
            &request,
            &response,
            String::from("plan123"),
            10,
            20,
        );

        assert_eq!(
            receipt.validation.claim_id,
            "metal.gpt_oss.text_generation.apple_silicon"
        );
        assert_eq!(
            receipt.validation.coverage,
            ValidationCoverage::PositiveExecution
        );
        assert_eq!(receipt.model_family, "gpt-oss");
        Ok(())
    }

    #[test]
    fn metal_gpt_oss_text_generation_failed_receipt_reports_explicit_refusal_validation()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = GenerationRequest::new_text(
            "metal-gpt-oss-req-fallback",
            sample_gpt_oss_decoder_descriptor(),
            Some(SessionId::new("metal-gpt-oss-session-fallback")),
            "hello",
            GenerationOptions::greedy(1),
        );

        let receipt = TextGenerationReceipt::failed_for_request(
            metal_fallback_selection(),
            &request,
            None,
            10,
            20,
            "metal backend unavailable",
        )
        .with_diagnostic(
            LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::BackendUnavailable,
                503,
                "metal backend unavailable",
            )
            .with_product_id(request.product_id.clone())
            .with_model_id(request.model.model.model_id.clone())
            .with_backend("metal"),
        );

        assert_eq!(receipt.validation.claim_id, "metal.refusal.off_platform");
        assert_eq!(
            receipt.validation.coverage,
            ValidationCoverage::ExplicitRefusal
        );
        assert_eq!(receipt.model_family, "gpt-oss");
        assert_eq!(receipt.runtime_backend, "cpu");
        assert_eq!(receipt.backend_selection.requested_backend, "metal");
        assert_eq!(receipt.backend_selection.effective_backend, "cpu");
        assert_eq!(
            receipt.diagnostic.as_ref().map(|value| value.code),
            Some(LocalRuntimeErrorCode::BackendUnavailable)
        );
        Ok(())
    }

    #[test]
    fn streaming_terminal_receipt_preserves_partial_cancellation_output() {
        let request = GenerationRequest::new_text(
            "gen-stream-1",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-stream-1")),
            "hello",
            GenerationOptions::greedy(4),
        );
        let response = GenerationResponse::new(
            &request,
            request.session_id.clone(),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::OPEN_ID]),
            "open",
            1,
            2,
            TerminationReason::Cancelled,
        )
        .with_metrics_and_provenance(
            GenerationMetrics {
                total_duration_ns: Some(10),
                load_duration_ns: Some(2),
                prompt_eval_count: Some(1),
                prompt_eval_duration_ns: Some(4),
                context_window: None,
                eval_count: Some(1),
                eval_duration_ns: Some(6),
                time_to_first_token_ns: None,
                inter_token_latency_ns: None,
                kv_cache: None,
                kv_residency: None,
                kv_cache_encoding: None,
                prefix_tokens_reused: Some(0),
                termination_detail: None,
                qwen35_cuda_decode: None,
                gpt_oss_perf: None,
            },
            GenerationProvenance {
                served_artifact: served_artifact_identity_for_decoder_model(
                    &request.model,
                    &cpu_backend_selection(),
                ),
                adapter_serving: None,
                served_revision: None,
                execution_plan_digest: String::from("stream-plan"),
                cluster_execution: None,
                load_state: GenerationLoadState::Cold,
                isolation_policy: LocalServingIsolationPolicy::in_process_runtime(),
                streaming_policy: Some(default_generation_streaming_policy()),
                memory_plan: Some(default_decoder_memory_plan(&request.model, None, None)),
                residency_policy: Some(ModelResidencyPolicy::default()),
                residency_snapshot: Some(MemoryResidencySnapshot {
                    loaded_models: 1,
                    resident_host_bytes: 640,
                    resident_device_bytes: 0,
                }),
                kv_cache_policy: Some(default_decoder_kv_cache_policy(&request.model)),
                kv_cache_encoding_policy: None,
                prefix_cache_state: Some(PrefixCacheState::None),
                prefix_cache_policy: Some(default_prefix_cache_policy()),
                prefix_cache_identity: None,
                compile_path: None,
                delivery_proof: None,
                cache_observations: Vec::new(),
                scheduler: None,
                kv_ownership: None,
                prefix_cache_control: None,
                prefix_cache_refusal_reason: None,
                structured_output: None,
                psion_served_evidence: None,
                psion_served_output_claim_posture: None,
            },
        );
        let terminal = GenerationStreamTerminal {
            status: GenerationStreamStatus::Cancelled,
            response,
            failure_reason: Some(String::from("stream cancelled by caller")),
            diagnostic: Some(
                LocalRuntimeDiagnostic::new(
                    LocalRuntimeErrorCode::Cancelled,
                    499,
                    "stream cancelled by caller",
                )
                .with_product_id(request.product_id.clone())
                .with_model_id(request.model.model.model_id.clone())
                .with_backend("cpu"),
            ),
        };

        let receipt = TextGenerationReceipt::from_stream_terminal(
            cpu_backend_selection(),
            &request,
            &terminal,
            "stream-plan",
            5,
            12,
        );

        assert_eq!(receipt.status, ReceiptStatus::Cancelled);
        assert_eq!(receipt.validation.claim_id, "cpu.text_generation.reference");
        assert_eq!(receipt.output_tokens, 1);
        assert_eq!(
            receipt.streaming_policy,
            Some(default_generation_streaming_policy())
        );
        assert_eq!(
            receipt.failure_reason.as_deref(),
            Some("stream cancelled by caller")
        );
        assert_eq!(
            receipt.diagnostic.as_ref().map(|value| value.code),
            Some(LocalRuntimeErrorCode::Cancelled)
        );
    }

    #[test]
    fn failed_receipt_carries_reason() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-4",
            sample_embedding_descriptor(),
            vec![String::from("hello")],
        );

        let receipt = ExecutionReceipt::failed_for_request(
            metal_fallback_selection(),
            &request,
            5,
            6,
            "backend offline",
        )
        .with_diagnostic(
            LocalRuntimeDiagnostic::new(
                LocalRuntimeErrorCode::BackendUnavailable,
                503,
                "backend offline",
            )
            .with_product_id(request.product_id.clone())
            .with_model_id(request.model.model.model_id.clone())
            .with_backend("metal"),
        );
        assert_eq!(receipt.status, ReceiptStatus::Failed);
        assert_eq!(receipt.validation.claim_id, "metal.refusal.off_platform");
        assert_eq!(
            receipt.validation.coverage,
            ValidationCoverage::ExplicitRefusal
        );
        assert_eq!(receipt.input_count, 1);
        assert_eq!(receipt.normalization, EmbeddingNormalization::None);
        assert_eq!(receipt.requested_output_dimensions, None);
        assert_eq!(receipt.failure_reason.as_deref(), Some("backend offline"));
        assert_eq!(
            receipt.diagnostic.as_ref().map(|value| value.code),
            Some(LocalRuntimeErrorCode::BackendUnavailable)
        );
        assert_eq!(receipt.runtime_backend, "cpu");
        assert_eq!(receipt.backend_selection.requested_backend, "metal");
        assert_eq!(receipt.weight_bundle.digest, request.model.weights.digest);
        let encoded = serde_json::to_string(&receipt)?;
        let decoded = serde_json::from_str::<ExecutionReceipt>(&encoded)?;
        assert_eq!(
            decoded.diagnostic.as_ref().map(|value| value.code),
            Some(LocalRuntimeErrorCode::BackendUnavailable)
        );
        Ok(())
    }

    #[test]
    fn amd_receipt_carries_execution_context() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-amd-1",
            sample_embedding_descriptor(),
            vec![String::from("hello")],
        );

        let receipt = ExecutionReceipt::failed_for_request(
            amd_userspace_selection(),
            &request,
            10,
            11,
            "backend disabled",
        );
        assert_eq!(
            receipt.validation.claim_id,
            "amd_userspace.embeddings.not_yet_validated"
        );
        assert_eq!(
            receipt.validation.coverage,
            ValidationCoverage::NotYetValidated
        );
        let Some(amd) = receipt.amd else {
            return Err("amd context missing".into());
        };
        assert_eq!(amd.mode, AmdRuntimeMode::Userspace);
        assert!(amd.risk.requires_explicit_opt_in);
        assert_eq!(amd.recovery.driver_binding, AmdDriverBinding::KernelAmdgpu);
        assert_eq!(receipt.input_count, 1);
        assert_eq!(receipt.normalization, EmbeddingNormalization::None);
        Ok(())
    }

    #[test]
    fn nvidia_receipt_carries_execution_context() -> Result<(), Box<dyn std::error::Error>> {
        let request = EmbeddingRequest::new(
            "req-cuda-1",
            sample_embedding_descriptor(),
            vec![String::from("hello")],
        );

        let receipt = ExecutionReceipt::failed_for_request(
            cuda_backend_selection(),
            &request,
            10,
            11,
            "cuda execution failed after launch",
        );
        assert_eq!(receipt.validation.claim_id, "cuda.embeddings.nvidia");
        assert_eq!(
            receipt.validation.coverage,
            ValidationCoverage::PositiveExecution
        );
        let Some(nvidia) = receipt.nvidia else {
            return Err("nvidia context missing".into());
        };
        assert_eq!(nvidia.topology.compute_capability.as_deref(), Some("8.9"));
        assert_eq!(nvidia.risk.level, NvidiaRiskLevel::Standard);
        assert_eq!(nvidia.recovery.supports_gpu_reset, Some(true));
        assert_eq!(receipt.input_count, 1);
        assert_eq!(receipt.normalization, EmbeddingNormalization::None);
        Ok(())
    }

    #[test]
    fn request_digests_are_deterministic() {
        let embedding_request = EmbeddingRequest::new(
            "req-5",
            sample_embedding_descriptor(),
            vec![String::from("same input")],
        );
        let generation_request = GenerationRequest::new_tokens(
            "gen-5",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000005")),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::HELLO_ID]),
            GenerationOptions::greedy(2),
        );

        assert_eq!(
            digest_embedding_request(&embedding_request),
            digest_embedding_request(&embedding_request)
        );
        assert_eq!(
            digest_generation_request(&generation_request),
            digest_generation_request(&generation_request)
        );
    }

    #[test]
    fn sandbox_request_digests_are_deterministic() {
        let request = SandboxExecutionRequestIdentity {
            request_id: String::from("sandbox-req-1"),
            sandbox_profile_digest: String::from("profile123"),
            command_digest: String::from("command123"),
            environment_digest: String::from("env123"),
            input_artifact_digests: vec![String::from("input-a"), String::from("input-b")],
        };

        assert_eq!(
            digest_sandbox_execution_request(&request),
            digest_sandbox_execution_request(&request)
        );
    }

    #[test]
    fn request_digests_change_when_weight_identity_changes() {
        let mut embedding_request = EmbeddingRequest::new(
            "req-6",
            sample_embedding_descriptor(),
            vec![String::from("same input")],
        );
        let mut generation_request = GenerationRequest::new_tokens(
            "gen-6",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000006")),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::HELLO_ID]),
            GenerationOptions::greedy(2),
        );

        let embedding_digest = digest_embedding_request(&embedding_request);
        let generation_digest = digest_generation_request(&generation_request);

        embedding_request.model.weights.digest = String::from("different-embedding-bundle");
        generation_request.model.weights.quantization =
            psionic_serve::QuantizationMode::Int8Symmetric;

        assert_ne!(
            digest_embedding_request(&embedding_request),
            embedding_digest
        );
        assert_ne!(
            digest_generation_request(&generation_request),
            generation_digest
        );
    }

    #[test]
    fn request_digests_change_when_artifact_identity_changes() {
        let mut embedding_request = EmbeddingRequest::new(
            "req-6a",
            sample_embedding_descriptor(),
            vec![String::from("same input")],
        );
        let mut generation_request = GenerationRequest::new_tokens(
            "gen-6a",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000006a")),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::HELLO_ID]),
            GenerationOptions::greedy(2),
        );

        let embedding_digest = digest_embedding_request(&embedding_request);
        let generation_digest = digest_generation_request(&generation_request);

        embedding_request
            .model
            .artifact_identity
            .as_mut()
            .expect("fixture embedding descriptor should carry artifact identity")
            .generation_defaults_digest = String::from("different-defaults");
        generation_request
            .model
            .artifact_identity
            .as_mut()
            .expect("fixture decoder descriptor should carry artifact identity")
            .chat_template_digest = Some(String::from("different-template"));

        assert_ne!(
            digest_embedding_request(&embedding_request),
            embedding_digest
        );
        assert_ne!(
            digest_generation_request(&generation_request),
            generation_digest
        );
    }

    #[test]
    fn embedding_request_digest_changes_when_output_dimensions_change() {
        let request = EmbeddingRequest::new(
            "req-embed-digest-1",
            sample_embedding_descriptor(),
            vec![String::from("same input")],
        );
        let truncated = request.clone().with_output_dimensions(4);

        assert_ne!(
            digest_embedding_request(&request),
            digest_embedding_request(&truncated)
        );
    }

    #[test]
    fn generation_request_digest_changes_when_options_change() {
        let request = GenerationRequest::new_tokens(
            "gen-7",
            sample_decoder_descriptor(),
            Some(SessionId::new("sess-00000007")),
            TokenSequence::new(vec![psionic_serve::FixtureWordTokenizer::HELLO_ID]),
            GenerationOptions::sample(2),
        );
        let baseline = digest_generation_request(&request);

        let mut with_seed = request.clone();
        with_seed.options.seed = Some(17);
        assert_ne!(digest_generation_request(&with_seed), baseline);

        let mut with_temperature = request.clone();
        with_temperature.options.temperature = Some(0.7);
        assert_ne!(digest_generation_request(&with_temperature), baseline);

        let mut with_stop = request;
        with_stop.options.stop_sequences = vec![String::from("</end>")];
        assert_ne!(digest_generation_request(&with_stop), baseline);
    }

    #[test]
    fn readiness_helper_sets_ready_status() {
        let readiness = ProviderReadiness::ready("ok");
        assert_eq!(readiness.status, HealthStatus::Ready);
        assert_eq!(readiness.message, "ok");
    }

    fn sample_decoder_descriptor() -> psionic_serve::DecoderModelDescriptor {
        ReferenceWordDecoder::new().descriptor().clone()
    }

    fn sample_gpt_oss_decoder_descriptor() -> psionic_serve::DecoderModelDescriptor {
        let mut descriptor = sample_decoder_descriptor();
        descriptor.model.family = String::from("gpt-oss");
        descriptor
    }

    fn sample_gemma4_decoder_descriptor() -> psionic_serve::DecoderModelDescriptor {
        let mut descriptor = sample_decoder_descriptor();
        descriptor.model.model_id = String::from("gemma4:e4b");
        descriptor.model.family = String::from("gemma4");
        descriptor.tokenizer_family = String::from("gemma4");
        descriptor.weights.digest = String::from("gemma4-e4b-fixture-digest");
        descriptor
    }

    fn sample_adapter_serving_binding() -> AdapterServingBinding {
        let model = sample_decoder_descriptor();
        let base_served_artifact =
            served_artifact_identity_for_decoder_model(&model, &cpu_backend_selection());
        let base_served_artifact_digest = base_served_artifact.served_artifact_digest;
        AdapterServingBinding::new(
            "fixture-word-decoder-qna",
            model.model.model_id.clone(),
            model.model.revision.clone(),
            base_served_artifact_digest.clone(),
            AdapterResidencyMode::HotSwapOverlay,
            vec![AdapterArtifactIdentity::new(
                "adapter-qna",
                "r1",
                AdapterArtifactKind::Lora,
                AdapterArtifactFormat::Safetensors,
                model.model.model_id,
                model.model.revision,
                base_served_artifact_digest,
                "adapter-digest-qna",
                RuntimeQuantizationMode::GgmlQ8_0,
                AdapterTargetFamily::DecoderAttention,
                1_024_000,
            )],
        )
    }

    fn sample_embedding_descriptor() -> psionic_serve::EmbeddingModelDescriptor {
        SmokeByteEmbedder::new().descriptor().clone()
    }

    fn sample_scheduler_node_id() -> NodeId {
        NodeId::new("scheduler-node")
    }

    fn trusted_lan_cluster_trust_assessment() -> ClusterComputeMarketTrustAssessment {
        ClusterTrustPolicy::trusted_lan().compute_market_trust_assessment()
    }

    fn attested_cluster_trust_assessment() -> ClusterComputeMarketTrustAssessment {
        ClusterTrustPolicy::attested_configured_peers(vec![
            ConfiguredClusterPeer::new(
                NodeId::new("worker-a"),
                std::net::SocketAddr::from(([127, 0, 0, 1], 31001)),
                "peer-key-a",
            )
            .with_attestation_requirement(
                NodeAttestationRequirement::new("issuer-a", "attestation-a")
                    .with_device_identity_digest("device-a"),
            ),
        ])
        .compute_market_trust_assessment()
    }

    fn cpu_backend_selection() -> BackendSelection {
        BackendSelection::direct(
            "cpu",
            Some(sample_cpu_device()),
            vec![
                String::from("input"),
                String::from("constant"),
                String::from("matmul"),
                String::from("add"),
            ],
        )
    }

    fn cuda_remote_dispatch_capability_profile() -> ClusterExecutionCapabilityProfile {
        ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![ClusterExecutionLane::RemoteWholeRequest])
            .with_serving_semantics_capability(
                ClusterServingSemantics::new(
                    ClusterExecutionLane::RemoteWholeRequest,
                    ExecutionCapabilityProfile::single_request_latency_optimized(),
                    ClusterWarmRoutePosture::ReadyNodeSelection,
                )
                .with_detail(
                    "remote whole-request serving keeps canonical local single-request semantics while only requiring selection of one ready node",
                ),
            )
            .with_detail(
                "backend `cuda` declares whole-request remote dispatch on ready cluster nodes",
            )
    }

    fn cuda_replica_routed_capability_profile() -> ClusterExecutionCapabilityProfile {
        ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::ReplicaRouted,
            ])
            .with_serving_semantics_capability(
                ClusterServingSemantics::new(
                    ClusterExecutionLane::ReplicaRouted,
                    ExecutionCapabilityProfile::single_request_latency_optimized(),
                    ClusterWarmRoutePosture::RoutePinned,
                )
                .with_detail(
                    "replica-routed serving keeps canonical local single-request semantics while requiring the same warm replica identity for truthful reuse",
                ),
            )
            .with_clustered_cache_capability(
                ClusterCacheCapability::new(
                    ClusterExecutionLane::ReplicaRouted,
                    ClusterCacheScope::ReplicaLocal,
                    ClusterCacheScope::ReplicaLocal,
                )
                .with_residency_tiers(vec![KvResidencyTier::Host, KvResidencyTier::Device])
                .invalidates_on_route_change()
                .with_detail(
                    "replica-routed prefix and KV reuse are only truthful on one warm replica identity",
                ),
            )
            .with_detail(
                "backend `cuda` declares whole-request dispatch plus replica routing across warm lanes",
            )
    }

    fn cuda_layer_sharded_capability_profile() -> ClusterExecutionCapabilityProfile {
        ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::LayerSharded,
            ])
            .with_serving_semantics_capability(
                ClusterServingSemantics::new(
                    ClusterExecutionLane::LayerSharded,
                    ExecutionCapabilityProfile::single_request_latency_optimized(),
                    ClusterWarmRoutePosture::TopologyPinned,
                )
                .with_detail(
                    "layer-sharded serving keeps single-request execution semantics while requiring the same shard topology for truthful warm reuse",
                ),
            )
            .with_clustered_cache_capability(
                ClusterCacheCapability::new(
                    ClusterExecutionLane::LayerSharded,
                    ClusterCacheScope::StageLocal,
                    ClusterCacheScope::StageLocal,
                )
                .with_residency_tiers(vec![KvResidencyTier::Host, KvResidencyTier::Device])
                .invalidates_on_topology_change()
                .with_detail(
                    "layer-sharded prefix and KV reuse are only truthful while shard ownership remains pinned",
                ),
            )
            .with_detail(
                "backend `cuda` declares whole-request dispatch plus layer-sharded cluster handoff support under explicit transport policy",
            )
    }

    fn cuda_pipeline_sharded_capability_profile() -> ClusterExecutionCapabilityProfile {
        ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::PipelineSharded,
            ])
            .with_serving_semantics_capability(
                ClusterServingSemantics::new(
                    ClusterExecutionLane::PipelineSharded,
                    ExecutionCapabilityProfile::single_request_latency_optimized(),
                    ClusterWarmRoutePosture::TopologyPinned,
                )
                .with_detail(
                    "pipeline-sharded serving keeps single-request execution semantics while requiring the same stage topology for truthful warm reuse",
                ),
            )
            .with_clustered_cache_capability(
                ClusterCacheCapability::new(
                    ClusterExecutionLane::PipelineSharded,
                    ClusterCacheScope::StageLocal,
                    ClusterCacheScope::StageLocal,
                )
                .with_residency_tiers(vec![KvResidencyTier::Host, KvResidencyTier::Device])
                .invalidates_on_topology_change()
                .with_detail(
                    "pipeline-parallel prefix and KV reuse are only truthful while ordered stage ownership remains pinned",
                ),
            )
            .with_detail(
                "backend `cuda` declares whole-request dispatch plus public-network pipeline-parallel stage handoff support under explicit timing policy",
            )
    }

    fn cuda_tensor_sharded_capability_profile() -> ClusterExecutionCapabilityProfile {
        ClusterExecutionCapabilityProfile::new("cuda")
            .with_supported_lanes(vec![
                ClusterExecutionLane::RemoteWholeRequest,
                ClusterExecutionLane::TensorSharded,
            ])
            .with_serving_semantics_capability(
                ClusterServingSemantics::new(
                    ClusterExecutionLane::TensorSharded,
                    ExecutionCapabilityProfile::single_request_latency_optimized(),
                    ClusterWarmRoutePosture::TopologyPinned,
                )
                .with_detail(
                    "tensor-sharded serving keeps single-request execution semantics while requiring the same collective topology for truthful warm reuse",
                ),
            )
            .with_clustered_cache_capability(
                ClusterCacheCapability::new(
                    ClusterExecutionLane::TensorSharded,
                    ClusterCacheScope::StageLocal,
                    ClusterCacheScope::StageLocal,
                )
                .with_residency_tiers(vec![KvResidencyTier::Host, KvResidencyTier::Device])
                .invalidates_on_topology_change()
                .with_detail(
                    "tensor-sharded prefix and KV reuse are only truthful while collective shard ownership remains pinned",
                ),
            )
            .with_detail(
                "backend `cuda` declares whole-request dispatch plus tensor-collective mesh support under explicit low-latency transport policy",
            )
    }

    fn sample_cluster_execution_context() -> ClusterExecutionContext {
        let capability_profile = cuda_remote_dispatch_capability_profile();
        ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::TrustedLanDatagram,
            ClusterExecutionDisposition::RemoteWholeRequest,
        )
        .with_communication_eligibility(
            capability_profile
                .lane_communication_eligibility(ClusterExecutionLane::RemoteWholeRequest),
        )
        .with_artifact_residency_digest("artifact-residency-digest")
        .with_commit_authority(ClusterCommitAuthorityEvidence::new(
            "coordinator-a",
            7,
            41,
            "authority-fence-token",
            "authority-digest",
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Admission,
            "admission-policy-digest",
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Placement,
            "placement-policy-digest",
        ))
        .with_serving_semantics(
            capability_profile
                .serving_semantics_capability(ClusterExecutionLane::RemoteWholeRequest)
                .cloned()
                .expect("fixture remote whole-request profile should declare serving semantics"),
        )
        .with_command_provenance(sample_cluster_command_provenance(&["worker-a"]))
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda")
                .with_role("worker")
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
            ClusterSelectedNode::new("worker-b", "cuda")
                .with_role("worker")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::CopyRequired),
        ])
        .with_fallback(
            ClusterFallbackStep::new("worker-a", ClusterFallbackReason::NodeUnavailable)
                .from_node("worker-b")
                .with_detail("initial worker dropped during admission"),
        )
        .with_degraded_reason("scheduler routed to the remaining healthy worker")
    }

    fn sample_replicated_cluster_execution_context() -> ClusterExecutionContext {
        let first = sample_cuda_device().inventory_qualifiers();
        let second = sample_cuda_device_1().inventory_qualifiers();
        let capability_profile = cuda_replica_routed_capability_profile();
        ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::TrustedLanDatagram,
            ClusterExecutionDisposition::ReplicaRouted,
        )
        .with_communication_eligibility(
            capability_profile.lane_communication_eligibility(ClusterExecutionLane::ReplicaRouted),
        )
        .with_replica_state_digest("replica-state-digest")
        .with_sharded_model_manifest_digest("replica-manifest-digest")
        .with_execution_topology(ExecutionTopologyPlan::replicated(
            "cuda",
            vec![first.clone(), second.clone()],
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Replication,
            "replication-policy-digest",
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Admission,
            "admission-policy-digest",
        ))
        .with_serving_semantics(
            capability_profile
                .serving_semantics_capability(ClusterExecutionLane::ReplicaRouted)
                .cloned()
                .expect("fixture replica-routed profile should declare serving semantics"),
        )
        .with_command_provenance(sample_cluster_command_provenance(&["worker-a", "worker-b"]))
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda")
                .with_role("worker")
                .with_device_inventory(first.clone())
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
            ClusterSelectedNode::new("worker-b", "cuda")
                .with_role("worker")
                .with_device_inventory(second.clone())
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
        ])
        .with_replica_nodes(vec![
            psionic_runtime::ClusterReplicaNode::new(
                0,
                ClusterSelectedNode::new("worker-a", "cuda")
                    .with_role("worker")
                    .with_device_inventory(first)
                    .with_served_artifact_digest("served-artifact-digest")
                    .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
                psionic_runtime::ClusterReplicaWarmState::Warm,
                psionic_runtime::ClusterReplicaRoutingDisposition::Selected,
            )
            .with_load(2, 0)
            .with_detail("served request on the least-loaded warm replica"),
            psionic_runtime::ClusterReplicaNode::new(
                1,
                ClusterSelectedNode::new("worker-b", "cuda")
                    .with_role("worker")
                    .with_device_inventory(second)
                    .with_served_artifact_digest("served-artifact-digest")
                    .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
                psionic_runtime::ClusterReplicaWarmState::Warm,
                psionic_runtime::ClusterReplicaRoutingDisposition::WarmStandby,
            )
            .with_load(0, 0)
            .with_detail("warm standby retained for failover"),
        ])
        .with_clustered_cache_usage(
            ClusterCacheUsage::new(
                ClusterExecutionLane::ReplicaRouted,
                ClusterCacheScope::ReplicaLocal,
                ClusterCacheScope::ReplicaLocal,
                CacheAction::Reuse,
                CacheAction::Reuse,
            )
            .with_detail(
                "replica-routed prefix and KV reuse remained pinned to the selected warm replica",
            ),
        )
        .with_degraded_reason("replicated lane admitted with one standby replica")
    }

    fn sample_layer_sharded_cluster_execution_context() -> ClusterExecutionContext {
        let first = sample_cuda_device().inventory_qualifiers();
        let second = sample_cuda_device_1().inventory_qualifiers();
        let capability_profile = cuda_layer_sharded_capability_profile();
        ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::Mixed,
            ClusterExecutionDisposition::Sharded,
        )
        .with_communication_eligibility(
            capability_profile.lane_communication_eligibility(ClusterExecutionLane::LayerSharded),
        )
        .with_artifact_residency_digest("artifact-residency-digest")
        .with_sharded_model_manifest_digest("layer-manifest-digest")
        .with_execution_topology(ExecutionTopologyPlan::layer_sharded(
            "cuda",
            vec![(first.clone(), 0, 20), (second.clone(), 20, 40)],
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Sharding,
            "sharding-policy-digest",
        ))
        .with_serving_semantics(
            capability_profile
                .serving_semantics_capability(ClusterExecutionLane::LayerSharded)
                .cloned()
                .expect("fixture layer-sharded profile should declare serving semantics"),
        )
        .with_command_provenance(sample_cluster_command_provenance(&["worker-a", "worker-b"]))
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda")
                .with_role("worker")
                .with_device_inventory(first.clone())
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
            ClusterSelectedNode::new("worker-b", "cuda")
                .with_role("worker")
                .with_device_inventory(second.clone())
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
        ])
        .with_shard_handoffs(vec![
            psionic_runtime::ClusterShardHandoff::new(
                0,
                1,
                "worker-a",
                "worker-b",
                psionic_runtime::ClusterShardHandoffKind::Activation,
                ClusterTransportClass::TrustedLanStream,
                20,
                8192,
            )
            .with_detail("forward activations across the shard boundary"),
            psionic_runtime::ClusterShardHandoff::new(
                0,
                1,
                "worker-a",
                "worker-b",
                psionic_runtime::ClusterShardHandoffKind::KvCache,
                ClusterTransportClass::TrustedLanStream,
                20,
                4096,
            )
            .with_detail("forward kv cache across the shard boundary"),
        ])
        .with_clustered_cache_usage(
            ClusterCacheUsage::new(
                ClusterExecutionLane::LayerSharded,
                ClusterCacheScope::StageLocal,
                ClusterCacheScope::StageLocal,
                CacheAction::Bypass,
                CacheAction::Bypass,
            )
            .with_detail(
                "layer-sharded execution does not promise cluster-wide prefix or KV reuse outside one fixed shard topology",
            ),
        )
        .with_degraded_reason("layer-sharded lane uses mixed scheduler and handoff transport")
    }

    fn sample_pipeline_sharded_cluster_execution_context() -> ClusterExecutionContext {
        let first = sample_cuda_device().inventory_qualifiers();
        let second = sample_cuda_device_1().inventory_qualifiers();
        let capability_profile = cuda_pipeline_sharded_capability_profile();
        ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::WiderNetworkStream,
            ClusterExecutionDisposition::Sharded,
        )
        .with_communication_eligibility(
            capability_profile
                .lane_communication_eligibility(ClusterExecutionLane::PipelineSharded),
        )
        .with_artifact_residency_digest("artifact-residency-digest")
        .with_sharded_model_manifest_digest("pipeline-manifest-digest")
        .with_execution_topology(ExecutionTopologyPlan::pipeline_sharded(
            "cuda",
            vec![(first.clone(), 0, 20), (second.clone(), 20, 40)],
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Sharding,
            "pipeline-policy-digest",
        ))
        .with_serving_semantics(
            capability_profile
                .serving_semantics_capability(ClusterExecutionLane::PipelineSharded)
                .cloned()
                .expect("fixture pipeline-sharded profile should declare serving semantics"),
        )
        .with_command_provenance(sample_cluster_command_provenance(&["worker-a", "worker-b"]))
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda")
                .with_role("worker")
                .with_device_inventory(first.clone())
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
            ClusterSelectedNode::new("worker-b", "cuda")
                .with_role("worker")
                .with_device_inventory(second.clone())
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
        ])
        .with_shard_handoffs(vec![
            psionic_runtime::ClusterShardHandoff::new(
                0,
                1,
                "worker-a",
                "worker-b",
                psionic_runtime::ClusterShardHandoffKind::Activation,
                ClusterTransportClass::WiderNetworkStream,
                20,
                8192,
            )
            .with_detail("forward pipeline activations across the stage boundary"),
            psionic_runtime::ClusterShardHandoff::new(
                0,
                1,
                "worker-a",
                "worker-b",
                psionic_runtime::ClusterShardHandoffKind::KvCache,
                ClusterTransportClass::WiderNetworkStream,
                20,
                4096,
            )
            .with_detail("forward pipeline KV across the stage boundary"),
        ])
        .with_pipeline_stages(vec![
            ClusterPipelineStage::new(
                0,
                "worker-a",
                ClusterPipelineStageRole::Entry,
                0,
                20,
                30,
                60,
                20,
            )
            .with_handoff(
                ClusterTransportClass::WiderNetworkStream,
                Some(32),
                Some(3000),
            )
            .with_detail("pipeline entry stage owns layers [0..20)"),
            ClusterPipelineStage::new(
                1,
                "worker-b",
                ClusterPipelineStageRole::Exit,
                20,
                40,
                35,
                70,
                25,
            )
            .with_detail("pipeline exit stage owns layers [20..40)"),
        ])
        .with_clustered_cache_usage(
            ClusterCacheUsage::new(
                ClusterExecutionLane::PipelineSharded,
                ClusterCacheScope::StageLocal,
                ClusterCacheScope::StageLocal,
                CacheAction::Bypass,
                CacheAction::Bypass,
            )
            .with_detail(
                "pipeline-parallel execution does not promise cluster-wide prefix or KV reuse outside one fixed stage topology",
            ),
        )
        .with_degraded_reason("pipeline handoff adds 32 ms public-network latency to decode")
    }

    fn sample_tensor_sharded_cluster_execution_context() -> ClusterExecutionContext {
        let first = sample_cuda_device().inventory_qualifiers();
        let second = sample_cuda_device_1().inventory_qualifiers();
        let capability_profile = cuda_tensor_sharded_capability_profile();
        ClusterExecutionContext::new(
            "cluster-alpha",
            "cluster-state-digest",
            "cluster-topology-digest",
            "scheduler-node",
            ClusterTransportClass::Mixed,
            ClusterExecutionDisposition::Sharded,
        )
        .with_communication_eligibility(
            capability_profile.lane_communication_eligibility(ClusterExecutionLane::TensorSharded),
        )
        .with_artifact_residency_digest("artifact-residency-digest")
        .with_sharded_model_manifest_digest("tensor-manifest-digest")
        .with_execution_topology(ExecutionTopologyPlan::tensor_sharded(
            "cuda",
            1,
            vec![(first.clone(), 0, 32), (second.clone(), 32, 64)],
        ))
        .with_policy_digest(ClusterPolicyDigest::new(
            ClusterPolicyDigestKind::Sharding,
            "tensor-sharding-policy-digest",
        ))
        .with_serving_semantics(
            capability_profile
                .serving_semantics_capability(ClusterExecutionLane::TensorSharded)
                .cloned()
                .expect("fixture tensor-sharded profile should declare serving semantics"),
        )
        .with_command_provenance(sample_cluster_command_provenance(&["worker-a", "worker-b"]))
        .with_selected_nodes(vec![
            ClusterSelectedNode::new("worker-a", "cuda")
                .with_role("worker")
                .with_device_inventory(first.clone())
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
            ClusterSelectedNode::new("worker-b", "cuda")
                .with_role("worker")
                .with_device_inventory(second.clone())
                .with_served_artifact_digest("served-artifact-digest")
                .with_artifact_residency(ClusterArtifactResidencyDisposition::Resident),
        ])
        .with_shard_handoffs(vec![
            psionic_runtime::ClusterShardHandoff::new(
                0,
                1,
                "worker-a",
                "worker-b",
                psionic_runtime::ClusterShardHandoffKind::TensorCollective,
                ClusterTransportClass::TrustedLanStream,
                0,
                16_384,
            )
            .with_tensor_partition(1, 0, 32)
            .with_detail("synchronize tensor shard [0..32) on axis 1 across the CUDA mesh"),
        ])
        .with_clustered_cache_usage(
            ClusterCacheUsage::new(
                ClusterExecutionLane::TensorSharded,
                ClusterCacheScope::StageLocal,
                ClusterCacheScope::StageLocal,
                CacheAction::Bypass,
                CacheAction::Bypass,
            )
            .with_detail(
                "tensor-sharded execution does not promise cluster-wide prefix or KV reuse outside one fixed collective topology",
            ),
        )
    }

    fn sample_cluster_command_provenance(
        selected_node_ids: &[&str],
    ) -> Vec<ClusterCommandProvenanceEvidence> {
        let mut provenance = vec![
            ClusterCommandProvenanceEvidence::new(
                ClusterAdmissionFactKind::SchedulerMembership,
                "scheduler-node",
                ClusterCommandAuthorityScopeEvidence::SelfNode,
                "scheduler-membership-command",
                "scheduler-membership-auth",
                "command-authorization-policy",
            )
            .with_target_node_id("scheduler-node"),
        ];
        for node_id in selected_node_ids {
            provenance.push(
                ClusterCommandProvenanceEvidence::new(
                    ClusterAdmissionFactKind::SelectedMembership,
                    *node_id,
                    ClusterCommandAuthorityScopeEvidence::SelfNode,
                    format!("{node_id}-membership-command"),
                    format!("{node_id}-membership-auth"),
                    "command-authorization-policy",
                )
                .with_target_node_id(*node_id),
            );
        }
        if let Some(first_node_id) = selected_node_ids.first() {
            provenance.push(
                ClusterCommandProvenanceEvidence::new(
                    ClusterAdmissionFactKind::ArtifactResidency,
                    *first_node_id,
                    ClusterCommandAuthorityScopeEvidence::SelfNode,
                    format!("{first_node_id}-artifact-command"),
                    format!("{first_node_id}-artifact-auth"),
                    "command-authorization-policy",
                )
                .with_target_node_id(*first_node_id),
            );
        }
        provenance.push(ClusterCommandProvenanceEvidence::new(
            ClusterAdmissionFactKind::Leadership,
            "scheduler-node",
            ClusterCommandAuthorityScopeEvidence::ProposedLeader,
            "leadership-command",
            "leadership-auth",
            "command-authorization-policy",
        ));
        provenance
    }

    fn sample_backend_runtime_resources() -> BackendRuntimeResources {
        BackendRuntimeResources {
            execution_plan_cache: psionic_runtime::ExecutionPlanCacheReport {
                policy: psionic_runtime::ExecutionPlanCachePolicy::bounded(8, Some(4096)),
                state: psionic_runtime::ExecutionPlanCacheState {
                    cached_entries: 2,
                    cached_bytes: 512,
                },
            },
            allocator_pool: AllocatorPoolReport {
                policy: AllocatorPoolPolicy::exact_tensor_spec(64, 8 * 1024 * 1024),
                state: AllocatorPoolState {
                    cached_buffers: 3,
                    cached_bytes: 4096,
                },
            },
            kernel_cache: KernelCacheReport {
                policy: KernelCachePolicy::disabled(),
                state: KernelCacheState::default(),
            },
            device_memory_budget: Some(DeviceMemoryBudget::new(
                Some(16 * 1024 * 1024 * 1024),
                8 * 1024 * 1024,
                0,
            )),
        }
    }

    fn metal_fallback_selection() -> BackendSelection {
        BackendSelection::fallback(
            "metal",
            "cpu",
            Some(sample_cpu_device()),
            vec![
                String::from("input"),
                String::from("constant"),
                String::from("matmul"),
                String::from("add"),
            ],
            "metal backend unavailable: no supported Metal device",
        )
    }

    fn metal_backend_selection() -> BackendSelection {
        BackendSelection::direct_with_policy(
            "metal",
            Some(sample_metal_device()),
            dense_supported_ops(),
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
        )
        .with_runtime_resources(Some(sample_backend_runtime_resources()))
    }

    fn dense_supported_ops() -> Vec<String> {
        vec![
            String::from("input"),
            String::from("constant"),
            String::from("matmul"),
            String::from("add"),
        ]
    }

    fn sample_cpu_device() -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("cpu"),
            device: Device::cpu(),
            device_name: Some(String::from("host cpu")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: vec![
                QuantizationSupport {
                    mode: RuntimeQuantizationMode::None,
                    load_path: QuantizationLoadPath::DenseF32,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: RuntimeQuantizationMode::Int8Symmetric,
                    load_path: QuantizationLoadPath::DequantizedF32,
                    execution: QuantizationExecution::DequantizeToF32,
                },
                QuantizationSupport {
                    mode: RuntimeQuantizationMode::GgmlQ4_0,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: RuntimeQuantizationMode::GgmlQ4_1,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::Native,
                },
                QuantizationSupport {
                    mode: RuntimeQuantizationMode::GgmlQ8_0,
                    load_path: QuantizationLoadPath::BackendQuantized,
                    execution: QuantizationExecution::Native,
                },
            ],
            memory_capacity_bytes: None,
            unified_memory: Some(true),
            feature_flags: vec![String::from("host_memory")],
            amd_metadata: None,
            nvidia_metadata: None,
        }
    }

    fn sample_metal_device() -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("metal"),
            device: Device::new(DeviceKind::Metal, 0, Some(String::from("metal:0"))),
            device_name: Some(String::from("apple gpu")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: vec![QuantizationSupport {
                mode: RuntimeQuantizationMode::None,
                load_path: QuantizationLoadPath::DenseF32,
                execution: QuantizationExecution::Native,
            }],
            memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
            unified_memory: Some(true),
            feature_flags: vec![String::from("metal3"), String::from("apple_silicon")],
            amd_metadata: None,
            nvidia_metadata: None,
        }
    }

    fn amd_kfd_selection() -> BackendSelection {
        BackendSelection::direct(
            "amd_kfd",
            Some(sample_amd_kfd_device()),
            vec![String::from("probe_only")],
        )
    }

    fn amd_kfd_execution_selection() -> BackendSelection {
        BackendSelection::direct_with_policy(
            "amd_kfd",
            Some(sample_amd_kfd_device()),
            vec![
                String::from("input"),
                String::from("constant"),
                String::from("fill_buffer"),
                String::from("copy_buffer"),
            ],
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
        )
        .with_runtime_resources(Some(sample_backend_runtime_resources()))
    }

    fn amd_userspace_selection() -> BackendSelection {
        BackendSelection::direct(
            "amd_userspace",
            Some(sample_amd_userspace_device()),
            vec![String::from("probe_only")],
        )
    }

    fn amd_userspace_execution_selection() -> BackendSelection {
        BackendSelection::direct_with_policy(
            "amd_userspace",
            Some(sample_amd_userspace_device()),
            vec![
                String::from("input"),
                String::from("constant"),
                String::from("fill_buffer"),
                String::from("copy_buffer"),
            ],
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
        )
        .with_runtime_resources(Some(sample_backend_runtime_resources()))
    }

    fn cuda_backend_selection() -> BackendSelection {
        BackendSelection::direct_with_policy(
            "cuda",
            Some(sample_cuda_device()),
            dense_supported_ops(),
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
        )
        .with_runtime_resources(Some(sample_backend_runtime_resources()))
    }

    fn degraded_cuda_backend_selection() -> BackendSelection {
        BackendSelection::degraded(
            "cuda",
            Some(sample_degraded_cuda_device()),
            dense_supported_ops(),
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
            "cuda discovered a display-attached GPU; local latency may vary",
        )
        .with_runtime_resources(Some(sample_backend_runtime_resources()))
    }

    fn cuda_multi_device_selection() -> BackendSelection {
        let first = sample_cuda_device();
        let second = sample_cuda_device_1();
        BackendSelection::direct_with_policy(
            "cuda",
            Some(first.clone()),
            dense_supported_ops(),
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
        )
        .with_selected_devices(vec![first.clone(), second.clone()])
        .with_execution_topology(Some(ExecutionTopologyPlan::layer_sharded(
            "cuda",
            vec![
                (first.inventory_qualifiers(), 0, 20),
                (second.inventory_qualifiers(), 20, 40),
            ],
        )))
        .with_runtime_resources(Some(sample_backend_runtime_resources()))
    }

    fn cuda_fallback_selection() -> BackendSelection {
        BackendSelection::fallback_with_policy(
            "cuda",
            "cpu",
            Some(sample_cpu_device()),
            dense_supported_ops(),
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
            "cuda backend unavailable: nvidia-smi is not installed or the NVIDIA driver is not reachable",
        )
        .with_runtime_resources(Some(sample_backend_runtime_resources()))
    }

    fn sample_amd_kfd_device() -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("amd_kfd"),
            device: Device::new(DeviceKind::AmdKfd, 0, Some(String::from("amd_kfd:0"))),
            device_name: Some(String::from("AMD Radeon KFD Test")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(24 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("kfd_device_node")],
            amd_metadata: Some(AmdDeviceMetadata {
                mode: AmdRuntimeMode::Kfd,
                topology: AmdTopologyInfo {
                    architecture: Some(String::from("gfx1100")),
                    pci_bdf: Some(String::from("0000:03:00.0")),
                    xcc_count: Some(1),
                    shader_engine_count: Some(4),
                    compute_unit_count: Some(60),
                    vram_bytes: Some(24 * 1024 * 1024 * 1024),
                    visible_vram_bytes: Some(16 * 1024 * 1024 * 1024),
                },
                risk: AmdRiskProfile {
                    level: AmdRiskLevel::Standard,
                    requires_explicit_opt_in: false,
                    may_unbind_kernel_driver: false,
                    warnings: Vec::new(),
                },
                recovery: AmdRecoveryProfile {
                    driver_binding: AmdDriverBinding::KernelAmdgpu,
                    expected_actions: vec![
                        AmdRecoveryAction::KernelDriverReset,
                        AmdRecoveryAction::RebootHost,
                    ],
                },
            }),
            nvidia_metadata: None,
        }
    }

    fn sample_cuda_device() -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
            device_name: Some(String::from("NVIDIA CUDA Test Device")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: Some(NvidiaDeviceMetadata {
                topology: NvidiaTopologyInfo {
                    architecture: Some(String::from("ada")),
                    compute_capability: Some(String::from("8.9")),
                    pci_bdf: Some(String::from("00000000:01:00.0")),
                    sm_count: Some(76),
                    vram_bytes: Some(16 * 1024 * 1024 * 1024),
                    mig_profile: None,
                },
                risk: NvidiaRiskProfile {
                    level: NvidiaRiskLevel::Standard,
                    display_attached: Some(false),
                    mig_partitioned: false,
                    warnings: Vec::new(),
                },
                recovery: NvidiaRecoveryProfile {
                    supports_gpu_reset: Some(true),
                    expected_actions: vec![
                        NvidiaRecoveryAction::ProcessRestart,
                        NvidiaRecoveryAction::GpuReset,
                        NvidiaRecoveryAction::RebootHost,
                    ],
                },
            }),
        }
    }

    fn sample_degraded_cuda_device() -> DeviceDescriptor {
        let mut device = sample_cuda_device();
        let Some(metadata) = device.nvidia_metadata.as_mut() else {
            return device;
        };
        metadata.risk.level = NvidiaRiskLevel::Elevated;
        metadata.risk.display_attached = Some(true);
        metadata.risk.warnings = vec![String::from(
            "display-attached NVIDIA devices may show variable latency under local desktop load",
        )];
        device
    }

    fn sample_cuda_device_1() -> DeviceDescriptor {
        let mut device = sample_cuda_device();
        device.device = Device::new(DeviceKind::Cuda, 1, Some(String::from("cuda:1")));
        device.device_name = Some(String::from("NVIDIA CUDA Test Device 1"));
        let Some(metadata) = device.nvidia_metadata.as_mut() else {
            return device;
        };
        metadata.topology.pci_bdf = Some(String::from("00000000:02:00.0"));
        device
    }

    fn sample_amd_userspace_device() -> DeviceDescriptor {
        DeviceDescriptor {
            backend: String::from("amd_userspace"),
            device: Device::new(
                DeviceKind::AmdUserspace,
                0,
                Some(String::from("amd_userspace:0")),
            ),
            device_name: Some(String::from("AMD Radeon Userspace Test")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(24 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("userspace_opt_in_disabled")],
            amd_metadata: Some(AmdDeviceMetadata {
                mode: AmdRuntimeMode::Userspace,
                topology: AmdTopologyInfo {
                    architecture: Some(String::from("gfx1100")),
                    pci_bdf: Some(String::from("0000:03:00.0")),
                    xcc_count: Some(1),
                    shader_engine_count: Some(4),
                    compute_unit_count: Some(60),
                    vram_bytes: Some(24 * 1024 * 1024 * 1024),
                    visible_vram_bytes: Some(16 * 1024 * 1024 * 1024),
                },
                risk: AmdRiskProfile {
                    level: AmdRiskLevel::Elevated,
                    requires_explicit_opt_in: true,
                    may_unbind_kernel_driver: true,
                    warnings: vec![String::from(
                        "userspace mode may require unloading or rebinding amdgpu",
                    )],
                },
                recovery: AmdRecoveryProfile {
                    driver_binding: AmdDriverBinding::KernelAmdgpu,
                    expected_actions: vec![
                        AmdRecoveryAction::ProcessRestart,
                        AmdRecoveryAction::RebindKernelDriver,
                        AmdRecoveryAction::RebootHost,
                    ],
                },
            }),
            nvidia_metadata: None,
        }
    }

    #[test]
    fn tassadar_trace_artifact_receipt_round_trips_relevant_fields() {
        let case = psionic_runtime::tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let execution = psionic_runtime::TassadarCpuReferenceRunner::new()
            .execute(&case.program)
            .expect("case should run");
        let artifact = TassadarTraceArtifact::from_execution("tassadar.provider.trace", &execution);
        let receipt = TassadarTraceArtifactReceipt::from_trace_artifact(&artifact);

        assert_eq!(receipt.artifact_id, artifact.artifact_id);
        assert_eq!(receipt.artifact_digest, artifact.artifact_digest);
        assert_eq!(receipt.trace_digest, artifact.trace_digest);
        assert_eq!(receipt.behavior_digest, artifact.behavior_digest);
        assert_eq!(receipt.step_count, artifact.step_count);
    }

    #[test]
    fn tassadar_trace_diff_receipt_reports_first_divergence() {
        let case = psionic_runtime::tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let execution = psionic_runtime::TassadarCpuReferenceRunner::new()
            .execute(&case.program)
            .expect("case should run");
        let mut divergent = execution.clone();
        divergent.steps[0].next_pc = divergent.steps[0].next_pc.saturating_add(1);

        let report =
            psionic_runtime::TassadarTraceDiffReport::from_executions(&execution, &divergent);
        let receipt = TassadarTraceDiffReceipt::from_trace_diff(&report);

        assert!(!receipt.exact_match);
        assert_eq!(receipt.first_divergence_step_index, Some(0));
        assert_eq!(receipt.diff_entry_count, 1);
        assert_eq!(receipt.expected_trace_digest, execution.trace_digest());
        assert_eq!(receipt.actual_trace_digest, divergent.trace_digest());
    }

    #[test]
    fn tassadar_capability_envelope_serializes_served_publication() {
        let service = LocalTassadarExecutorService::new()
            .with_fixture(TassadarExecutorFixture::article_i32_compute_v1());
        let publication = service
            .capability_publication(Some(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID))
            .expect("article capability publication");
        let envelope = TassadarCapabilityEnvelope::from_executor_capability_publication(
            &publication,
            ProviderReadiness::ready("article lane ready"),
        )
        .expect("provider envelope should build");

        let encoded = serde_json::to_value(&envelope).expect("envelope should serialize");
        assert_eq!(encoded["backend_family"], json!("psionic"));
        assert_eq!(encoded["product_id"], json!("psionic.executor_trace"));
        assert_eq!(
            encoded["publication"]["model_descriptor"]["model"]["model_id"],
            json!(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID)
        );
        assert_eq!(
            encoded["publication"]["module_execution_capability"]["runtime_capability"]["supports_call_indirect"],
            json!(true)
        );
        assert_eq!(
            encoded["publication"]["module_execution_capability"]["runtime_capability"]["supports_active_element_segments"],
            json!(true)
        );
        assert_eq!(
            encoded["publication"]["module_execution_capability"]["runtime_capability"]["supports_start_function_instantiation"],
            json!(true)
        );
        assert_eq!(
            encoded["publication"]["module_execution_capability"]["runtime_capability"]["supports_linear_memory"],
            json!(true)
        );
        assert_eq!(
            encoded["publication"]["module_execution_capability"]["runtime_capability"]["supports_active_data_segments"],
            json!(true)
        );
        assert_eq!(
            encoded["publication"]["module_execution_capability"]["runtime_capability"]["supports_memory_grow"],
            json!(true)
        );
        assert_eq!(
            encoded["publication"]["module_execution_capability"]["runtime_capability"]["host_import_boundary"]
                ["unsupported_host_call_refusal"],
            json!("unsupported_host_import")
        );
        assert_eq!(
            encoded["publication"]["rust_article_profile_completeness"]["family_id"],
            json!("tassadar.wasm.rust_article_family.v1")
        );
        assert_eq!(
            encoded["publication"]["internal_compute_profile_claim_check"]["claim"]["profile_id"],
            json!("tassadar.internal_compute.article_closeout.v1")
        );
        assert_eq!(
            encoded["publication"]["internal_compute_profile_claim_check"]["green"],
            json!(true)
        );
        assert_eq!(
            encoded["publication"]["effect_safe_resume_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_effect_safe_resume_report.json")
        );
        assert_eq!(
            encoded["publication"]["subset_profile_promotion_gate_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_subset_profile_promotion_gate_report.json")
        );
        assert_eq!(
            encoded["publication"]["frozen_core_wasm_window_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json")
        );
        assert_eq!(
            encoded["publication"]["frozen_core_wasm_closure_gate_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_frozen_core_wasm_closure_gate_report.json")
        );
        assert_eq!(
            encoded["publication"]["frozen_core_wasm_closure_status"],
            json!("not_closed")
        );
        assert_eq!(
            encoded["publication"]["frozen_core_wasm_served_publication_allowed"],
            json!(false)
        );
        assert_eq!(
            encoded["publication"]["float_profile_acceptance_gate_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_float_profile_acceptance_gate_report.json")
        );
        assert_eq!(
            encoded["publication"]["float_profile_route_policy_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_float_profile_route_policy_report.json")
        );
        assert_eq!(
            encoded["publication"]["exception_profile_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_exception_profile_report.json")
        );
        assert_eq!(
            encoded["publication"]["exception_profile_public_profile_ids"],
            json!(["tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"])
        );
        assert_eq!(
            encoded["publication"]["exception_profile_default_served_profile_ids"],
            json!([])
        );
        assert_eq!(
            encoded["publication"]["exception_profile_portability_envelope_ids"],
            json!(["cpu_reference_current_host"])
        );
        assert_eq!(
            encoded["publication"]["simd_profile_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_simd_profile_report.json")
        );
        assert_eq!(
            encoded["publication"]["simd_profile_public_profile_ids"],
            json!(["tassadar.proposal_profile.simd_deterministic.v1"])
        );
        assert_eq!(
            encoded["publication"]["simd_profile_default_served_profile_ids"],
            json!([])
        );
        assert_eq!(
            encoded["publication"]["simd_profile_exact_backend_ids"],
            json!(["cpu_reference_current_host"])
        );
        assert_eq!(
            encoded["publication"]["simd_profile_fallback_backend_ids"],
            json!(["metal_served", "cuda_served"])
        );
        assert_eq!(
            encoded["publication"]["simd_profile_refused_backend_ids"],
            json!(["accelerator_specific_unbounded"])
        );
        assert_eq!(
            encoded["publication"]["session_process_profile_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_session_process_profile_report.json")
        );
        assert_eq!(
            encoded["publication"]["session_process_route_policy_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_session_process_route_policy_report.json")
        );
        assert_eq!(
            encoded["publication"]["session_process_public_profile_ids"],
            json!(["tassadar.internal_compute.session_process.v1"])
        );
        assert_eq!(
            encoded["publication"]["session_process_default_served_profile_ids"],
            json!([])
        );
        assert_eq!(
            encoded["publication"]["session_process_routeable_interaction_surface_ids"],
            json!(["deterministic_echo_turn_loop", "stateful_counter_turn_loop"])
        );
        assert_eq!(
            encoded["publication"]["session_process_refused_interaction_surface_ids"],
            json!(["open_ended_external_event_stream"])
        );
        assert_eq!(
            encoded["publication"]["async_lifecycle_profile_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_async_lifecycle_profile_report.json")
        );
        assert_eq!(
            encoded["publication"]["async_lifecycle_route_policy_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_async_lifecycle_route_policy_report.json")
        );
        assert_eq!(
            encoded["publication"]["async_lifecycle_public_profile_ids"],
            json!(["tassadar.internal_compute.async_lifecycle.v1"])
        );
        assert_eq!(
            encoded["publication"]["async_lifecycle_default_served_profile_ids"],
            json!([])
        );
        assert_eq!(
            encoded["publication"]["async_lifecycle_routeable_surface_ids"],
            json!([
                "interruptible_counter_job",
                "retryable_timeout_search_job",
                "safe_boundary_cancellation_job"
            ])
        );
        assert_eq!(
            encoded["publication"]["async_lifecycle_refused_surface_ids"],
            json!([
                "mid_effect_cancellation",
                "open_ended_external_callback",
                "unbounded_retry_backoff"
            ])
        );
        assert_eq!(
            encoded["publication"]["preemptive_job_profile_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_preemptive_job_report.json")
        );
        assert_eq!(
            encoded["publication"]["preemptive_job_fairness_report_ref"],
            json!("fixtures/tassadar/reports/tassadar_preemptive_job_fairness_report.json")
        );
        assert_eq!(
            encoded["publication"]["preemptive_job_profile_id"],
            json!("tassadar.internal_compute.preemptive_jobs.v1")
        );
        assert_eq!(
            encoded["publication"]["preemptive_job_green_scheduler_ids"],
            json!(["deterministic_round_robin", "weighted_fair_slice_rotation"])
        );
        assert_eq!(
            encoded["publication"]["preemptive_job_refused_scheduler_ids"],
            json!(["host_nondeterministic_scheduler"])
        );
        assert_eq!(
            encoded["publication"]["preemptive_job_exact_case_count"],
            json!(2)
        );
        assert_eq!(
            encoded["publication"]["preemptive_job_refusal_case_count"],
            json!(2)
        );
        assert_eq!(
            encoded["publication"]["preemptive_job_served_publication_allowed"],
            json!(false)
        );
        assert_eq!(
            encoded["broad_internal_compute_profile_publication_receipt"]["public_profile_specific_route_ids"],
            json!([
                "tassadar.internal_compute.deterministic_import_subset.v1",
                "tassadar.internal_compute.runtime_support_subset.v1"
            ])
        );
        assert_eq!(
            encoded["broad_internal_compute_portability_receipt"]["backend_family_ids"],
            json!(["cpu_reference", "cuda_served", "metal_served"])
        );
        assert_eq!(
            encoded["broad_internal_compute_portability_receipt"]["toolchain_family_ids"],
            json!([
                "rustc:wasm32-unknown-unknown",
                "rustc:wasm32-unknown-unknown+cuda_served",
                "rustc:wasm32-unknown-unknown+metal_served"
            ])
        );
        assert_eq!(
            encoded["numeric_portability_receipt"]["profile_ids"],
            json!([
                "tassadar.numeric_profile.bounded_f64_conversion.v1",
                "tassadar.numeric_profile.f32_only.v1",
                "tassadar.numeric_profile.mixed_i32_f32.v1"
            ])
        );
        assert_eq!(
            encoded["numeric_portability_receipt"]["publication_allowed_profile_ids"],
            json!([
                "tassadar.numeric_profile.f32_only.v1",
                "tassadar.numeric_profile.mixed_i32_f32.v1"
            ])
        );
        assert_eq!(
            encoded["float_profile_acceptance_gate_receipt"]["public_profile_allowed_profile_ids"],
            json!([
                "tassadar.numeric_profile.f32_only.v1",
                "tassadar.numeric_profile.mixed_i32_f32.v1"
            ])
        );
        assert_eq!(
            encoded["float_profile_acceptance_gate_receipt"]["default_served_profile_allowed_profile_ids"],
            json!([])
        );
        assert_eq!(
            encoded["exception_profile_receipt"]["public_profile_allowed_profile_ids"],
            json!(["tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"])
        );
        assert_eq!(
            encoded["exception_profile_receipt"]["default_served_profile_allowed_profile_ids"],
            json!([])
        );
        assert_eq!(
            encoded["exception_profile_receipt"]["portability_envelope_ids"],
            json!(["cpu_reference_current_host"])
        );
        assert_eq!(
            encoded["simd_profile_receipt"]["public_profile_allowed_profile_ids"],
            json!(["tassadar.proposal_profile.simd_deterministic.v1"])
        );
        assert_eq!(
            encoded["simd_profile_receipt"]["default_served_profile_allowed_profile_ids"],
            json!([])
        );
        assert_eq!(
            encoded["simd_profile_receipt"]["exact_backend_ids"],
            json!(["cpu_reference_current_host"])
        );
        assert_eq!(
            encoded["simd_profile_receipt"]["fallback_backend_ids"],
            json!(["metal_served", "cuda_served"])
        );
        assert_eq!(
            encoded["simd_profile_receipt"]["refused_backend_ids"],
            json!(["accelerator_specific_unbounded"])
        );
        assert_eq!(
            encoded["session_process_profile_receipt"]["public_profile_allowed_profile_ids"],
            json!(["tassadar.internal_compute.session_process.v1"])
        );
        assert_eq!(
            encoded["session_process_profile_receipt"]["default_served_profile_allowed_profile_ids"],
            json!([])
        );
        assert_eq!(
            encoded["session_process_profile_receipt"]["routeable_interaction_surface_ids"],
            json!(["deterministic_echo_turn_loop", "stateful_counter_turn_loop"])
        );
        assert_eq!(
            encoded["session_process_profile_receipt"]["refused_interaction_surface_ids"],
            json!(["open_ended_external_event_stream"])
        );
        assert_eq!(
            encoded["async_lifecycle_profile_receipt"]["public_profile_allowed_profile_ids"],
            json!(["tassadar.internal_compute.async_lifecycle.v1"])
        );
        assert_eq!(
            encoded["async_lifecycle_profile_receipt"]["default_served_profile_allowed_profile_ids"],
            json!([])
        );
        assert_eq!(
            encoded["async_lifecycle_profile_receipt"]["routeable_lifecycle_surface_ids"],
            json!([
                "interruptible_counter_job",
                "retryable_timeout_search_job",
                "safe_boundary_cancellation_job"
            ])
        );
        assert_eq!(
            encoded["async_lifecycle_profile_receipt"]["refused_lifecycle_surface_ids"],
            json!([
                "mid_effect_cancellation",
                "open_ended_external_callback",
                "unbounded_retry_backoff"
            ])
        );
        assert_eq!(
            encoded["preemptive_job_receipt"]["green_scheduler_ids"],
            json!(["deterministic_round_robin", "weighted_fair_slice_rotation"])
        );
        assert_eq!(
            encoded["preemptive_job_receipt"]["refused_scheduler_ids"],
            json!(["host_nondeterministic_scheduler"])
        );
        assert_eq!(
            encoded["preemptive_job_receipt"]["resumable_process_ids"],
            json!([
                "tassadar.process.long_loop_kernel.v1",
                "tassadar.process.search_frontier_kernel.v1"
            ])
        );
        assert_eq!(
            encoded["preemptive_job_receipt"]["served_publication_allowed"],
            json!(false)
        );
        assert_eq!(
            encoded["effect_safe_resume_receipt"]["target_profile_id"],
            json!("tassadar.internal_compute.deterministic_import_subset.v1")
        );
        assert_eq!(
            encoded["subset_profile_promotion_gate_receipt"]["overall_green"],
            json!(true)
        );
        assert_eq!(
            encoded["frozen_core_wasm_closure_gate_receipt"]["closure_status"],
            json!("not_closed")
        );
        assert_eq!(
            encoded["frozen_core_wasm_closure_gate_receipt"]["served_publication_allowed"],
            json!(false)
        );
        assert_eq!(
            encoded["quantization_truth_envelope"]["active_backend_family"],
            json!("cpu_reference")
        );
        let workload_classes = encoded["publication"]["workload_capability_matrix"]["rows"]
            .as_array()
            .expect("rows should encode as an array")
            .iter()
            .map(|row| row["workload_class"].clone())
            .collect::<Vec<_>>();
        assert!(workload_classes.contains(&json!("micro_wasm_kernel")));
    }

    #[test]
    fn tassadar_capability_envelope_rejects_unpublishable_matrix() {
        let service = LocalTassadarExecutorService::new()
            .with_fixture(TassadarExecutorFixture::article_i32_compute_v1());
        let mut publication = service
            .capability_publication(Some(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID))
            .expect("article capability publication");
        publication
            .workload_capability_matrix
            .rows
            .iter_mut()
            .find(|row| row.workload_class == TassadarWorkloadClass::MicroWasmKernel)
            .expect("micro workload row")
            .benchmark_gate = None;

        let err = TassadarCapabilityEnvelope::from_executor_capability_publication(
            &publication,
            ProviderReadiness::ready("article lane ready"),
        )
        .expect_err("provider envelope should reject an unpublishable matrix");
        assert!(matches!(
            err,
            TassadarCapabilityEnvelopeError::UnpublishableWorkloadMatrix { .. }
        ));
    }

    #[test]
    fn tassadar_capability_envelope_rejects_unpublishable_internal_compute_profile_claim() {
        let service = LocalTassadarExecutorService::new()
            .with_fixture(TassadarExecutorFixture::article_i32_compute_v1());
        let mut publication = service
            .capability_publication(Some(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID))
            .expect("article capability publication");
        publication.internal_compute_profile_claim_check.green = false;
        publication.internal_compute_profile_claim_check.detail =
            String::from("missing current profile evidence");

        let err = TassadarCapabilityEnvelope::from_executor_capability_publication(
            &publication,
            ProviderReadiness::ready("article lane ready"),
        )
        .expect_err("provider envelope should reject an unpublishable profile claim");
        assert!(matches!(
            err,
            TassadarCapabilityEnvelopeError::UnpublishableInternalComputeProfileClaim { .. }
        ));
    }

    #[test]
    fn tassadar_capability_envelope_rejects_unpublishable_frozen_core_wasm_closure_gate() {
        let service = LocalTassadarExecutorService::new()
            .with_fixture(TassadarExecutorFixture::article_i32_compute_v1());
        let mut publication = service
            .capability_publication(Some(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID))
            .expect("article capability publication");
        publication.frozen_core_wasm_closure_gate_report_ref.clear();

        let err = TassadarCapabilityEnvelope::from_executor_capability_publication(
            &publication,
            ProviderReadiness::ready("article lane ready"),
        )
        .expect_err("provider envelope should reject a missing closure-gate ref");
        assert!(matches!(
            err,
            TassadarCapabilityEnvelopeError::UnpublishableFrozenCoreWasmClosureGate { .. }
        ));
    }

    #[test]
    fn tassadar_workload_capability_frontier_receipt_projects_research_summary() {
        let summary =
            build_tassadar_workload_capability_frontier_summary_report().expect("frontier summary");
        let receipt = TassadarWorkloadCapabilityFrontierReceipt::from_summary(&summary);

        assert_eq!(receipt.report_id, summary.report_id);
        assert_eq!(
            receipt.workload_family_count,
            summary.frontier_report.frontier_rows.len() as u32
        );
        assert!(
            receipt
                .under_mapped_workload_family_ids
                .contains(&String::from("micro_wasm_kernel"))
        );
        assert!(
            receipt
                .refusal_first_workload_family_ids
                .contains(&String::from("sudoku_class"))
        );
    }

    #[test]
    fn tassadar_kernel_module_scaling_receipt_projects_research_summary() {
        let summary =
            build_tassadar_kernel_module_scaling_summary_report().expect("scaling summary");
        let receipt = TassadarKernelModuleScalingReceipt::from_summary(&summary);

        assert_eq!(receipt.report_id, summary.report_id);
        assert_eq!(receipt.module_exact_import_complexity_threshold, Some(0));
        assert!(
            receipt
                .kernel_cost_degraded_family_ids
                .contains(&String::from("backward_loop_kernel"))
        );
        assert!(
            receipt
                .refusal_boundary_family_ids
                .contains(&String::from("module_host_import_boundary"))
        );
    }

    #[test]
    fn tassadar_planner_route_capability_envelope_negotiates_direct_route_from_served_descriptor() {
        let router = LocalTassadarPlannerRouter::new().with_executor_service(
            LocalTassadarExecutorService::new()
                .with_fixture(TassadarExecutorFixture::article_i32_compute_v1()),
        );
        let route_descriptor = router
            .route_capability_descriptor(Some(
                TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
            ))
            .expect("planner route descriptor should publish");
        let envelope = TassadarPlannerRouteCapabilityEnvelope::from_route_descriptor(
            &route_descriptor,
            ProviderReadiness::ready("planner route ready"),
        )
        .expect("provider route envelope should build");

        let outcome = negotiate_tassadar_planner_executor_route(
            &[envelope.as_route_candidate("provider-a", "worker-a")],
            &TassadarPlannerExecutorRouteNegotiationRequest::new(
                "planner-route-request-direct",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            )
            .with_requested_model_id(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID)
            .with_requested_wasm_module_class(TassadarWorkloadClass::MicroWasmKernel),
        );

        match outcome {
            TassadarPlannerExecutorRouteNegotiationOutcome::Selected { selection } => {
                assert_eq!(selection.provider_id, "provider-a");
                assert_eq!(selection.worker_id, "worker-a");
                assert_eq!(
                    selection.route_state,
                    TassadarPlannerExecutorNegotiatedRouteState::Direct
                );
                assert_eq!(
                    selection.effective_decode_mode,
                    psionic_runtime::TassadarExecutorDecodeMode::HullCache
                );
                assert_eq!(
                    selection.route_descriptor.model_id,
                    TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID
                );
                assert_eq!(
                    selection.route_descriptor.internal_compute_profile_id,
                    "tassadar.internal_compute.article_closeout.v1"
                );
                assert_eq!(
                    selection
                        .wasm_capability
                        .expect("module-class-aware route selection")
                        .module_class,
                    TassadarWorkloadClass::MicroWasmKernel
                );
            }
            other => panic!("expected direct planner route selection, got {other:?}"),
        }
    }

    #[test]
    fn tassadar_planner_route_capability_envelope_surfaces_typed_refusal_when_runtime_fallback_is_disallowed()
     {
        let router = LocalTassadarPlannerRouter::new().with_executor_service(
            LocalTassadarExecutorService::new()
                .with_fixture(TassadarExecutorFixture::article_i32_compute_v1()),
        );
        let route_descriptor = router
            .route_capability_descriptor(Some(
                TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
            ))
            .expect("planner route descriptor should publish");
        let envelope = TassadarPlannerRouteCapabilityEnvelope::from_route_descriptor(
            &route_descriptor,
            ProviderReadiness::ready("planner route ready"),
        )
        .expect("provider route envelope should build");

        let outcome = negotiate_tassadar_planner_executor_route(
            &[envelope.as_route_candidate("provider-a", "worker-a")],
            &TassadarPlannerExecutorRouteNegotiationRequest::new(
                "planner-route-request-hull-fallback-disallowed",
                psionic_runtime::TassadarExecutorDecodeMode::HullCache,
            )
            .with_requested_model_id(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID)
            .with_requested_wasm_module_class(TassadarWorkloadClass::LongLoopKernel)
            .disallow_runtime_decode_fallback(),
        );

        match outcome {
            TassadarPlannerExecutorRouteNegotiationOutcome::Refused { refusal } => {
                assert_eq!(
                    refusal.refusal_reason,
                    TassadarPlannerExecutorRouteRefusalReason::RuntimeFallbackDisallowed
                );
            }
            other => panic!("expected typed fallback refusal, got {other:?}"),
        }
    }

    #[test]
    fn tassadar_planner_route_capability_envelope_rejects_missing_benchmark_gate() {
        let router = LocalTassadarPlannerRouter::new().with_executor_service(
            LocalTassadarExecutorService::new()
                .with_fixture(TassadarExecutorFixture::article_i32_compute_v1()),
        );
        let mut route_descriptor = router
            .route_capability_descriptor(Some(
                TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
            ))
            .expect("planner route descriptor should publish");
        route_descriptor.benchmark_report_ref.clear();

        let err = TassadarPlannerRouteCapabilityEnvelope::from_route_descriptor(
            &route_descriptor,
            ProviderReadiness::ready("planner route ready"),
        )
        .expect_err("provider route envelope should reject missing benchmark gate");
        assert!(matches!(
            err,
            TassadarPlannerRouteCapabilityEnvelopeError::MissingBenchmarkGate { .. }
        ));
    }

    #[test]
    fn tassadar_planner_route_capability_envelope_rejects_missing_internal_compute_profile_id() {
        let router = LocalTassadarPlannerRouter::new().with_executor_service(
            LocalTassadarExecutorService::new()
                .with_fixture(TassadarExecutorFixture::article_i32_compute_v1()),
        );
        let mut route_descriptor = router
            .route_capability_descriptor(Some(
                TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
            ))
            .expect("planner route descriptor should publish");
        route_descriptor.internal_compute_profile_id.clear();

        let err = TassadarPlannerRouteCapabilityEnvelope::from_route_descriptor(
            &route_descriptor,
            ProviderReadiness::ready("planner route ready"),
        )
        .expect_err("provider route envelope should reject missing internal profile id");
        assert!(matches!(
            err,
            TassadarPlannerRouteCapabilityEnvelopeError::MissingInternalComputeProfileId { .. }
        ));
    }

    #[test]
    fn tassadar_planner_route_capability_envelope_rejects_missing_internal_compute_profile_claim_digest()
     {
        let router = LocalTassadarPlannerRouter::new().with_executor_service(
            LocalTassadarExecutorService::new()
                .with_fixture(TassadarExecutorFixture::article_i32_compute_v1()),
        );
        let mut route_descriptor = router
            .route_capability_descriptor(Some(
                TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
            ))
            .expect("planner route descriptor should publish");
        route_descriptor
            .internal_compute_profile_claim_digest
            .clear();

        let err = TassadarPlannerRouteCapabilityEnvelope::from_route_descriptor(
            &route_descriptor,
            ProviderReadiness::ready("planner route ready"),
        )
        .expect_err("provider route envelope should reject missing internal profile claim digest");
        assert!(matches!(
            err,
            TassadarPlannerRouteCapabilityEnvelopeError::MissingInternalComputeProfileClaimDigest { .. }
        ));
    }

    #[test]
    fn tassadar_planner_route_capability_envelope_rejects_missing_wasm_benchmark_row() {
        let router = LocalTassadarPlannerRouter::new().with_executor_service(
            LocalTassadarExecutorService::new()
                .with_fixture(TassadarExecutorFixture::article_i32_compute_v1()),
        );
        let mut route_descriptor = router
            .route_capability_descriptor(Some(
                TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
            ))
            .expect("planner route descriptor should publish");
        route_descriptor
            .wasm_capability_matrix
            .rows
            .iter_mut()
            .find(|row| row.module_class == TassadarWorkloadClass::MicroWasmKernel)
            .expect("micro kernel route row")
            .benchmark_report_ref = None;

        let err = TassadarPlannerRouteCapabilityEnvelope::from_route_descriptor(
            &route_descriptor,
            ProviderReadiness::ready("planner route ready"),
        )
        .expect_err("provider route envelope should reject missing Wasm benchmark row");
        assert!(matches!(
            err,
            TassadarPlannerRouteCapabilityEnvelopeError::UnpublishableWasmCapabilityMatrix { .. }
        ));
    }

    #[test]
    fn tassadar_planner_route_capability_envelope_refuses_unsupported_import_posture_by_module_class()
     {
        let router = LocalTassadarPlannerRouter::new().with_executor_service(
            LocalTassadarExecutorService::new()
                .with_fixture(TassadarExecutorFixture::article_i32_compute_v1()),
        );
        let route_descriptor = router
            .route_capability_descriptor(Some(
                TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
            ))
            .expect("planner route descriptor should publish");
        let envelope = TassadarPlannerRouteCapabilityEnvelope::from_route_descriptor(
            &route_descriptor,
            ProviderReadiness::ready("planner route ready"),
        )
        .expect("provider route envelope should build");

        let outcome = negotiate_tassadar_planner_executor_route(
            &[envelope.as_route_candidate("provider-a", "worker-a")],
            &TassadarPlannerExecutorRouteNegotiationRequest::new(
                "planner-route-request-import-posture",
                psionic_runtime::TassadarExecutorDecodeMode::ReferenceLinear,
            )
            .with_requested_model_id(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID)
            .with_requested_wasm_module_class(TassadarWorkloadClass::MicroWasmKernel)
            .with_requested_wasm_import_posture(
                TassadarPlannerExecutorWasmImportPosture::DeterministicStubImportsOnly,
            )
            .with_required_wasm_opcode_families(vec![
                TassadarPlannerExecutorWasmOpcodeFamily::CoreI32Arithmetic,
            ]),
        );

        match outcome {
            TassadarPlannerExecutorRouteNegotiationOutcome::Refused { refusal } => {
                assert_eq!(
                    refusal.refusal_reason,
                    TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported
                );
            }
            other => panic!("expected import-posture refusal, got {other:?}"),
        }
    }

    #[test]
    fn tassadar_exactness_refusal_receipt_round_trips_runtime_report() {
        let case = psionic_runtime::tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let expected = TassadarCpuReferenceRunner::new()
            .execute(&case.program)
            .expect("case should run");
        let execution_report = psionic_runtime::execute_tassadar_executor_request(
            &case.program,
            psionic_runtime::TassadarExecutorDecodeMode::ReferenceLinear,
            psionic_runtime::TassadarTraceAbi::core_i32_v1().schema_version,
            None,
        )
        .expect("reference-linear execution should succeed");
        let runtime_report = TassadarExactnessRefusalReport::from_execution_report(
            &case.case_id,
            &expected,
            &execution_report,
        );

        let receipt = TassadarExactnessRefusalReceipt::from_runtime_report(&runtime_report);

        assert_eq!(receipt.subject_id, case.case_id);
        assert_eq!(receipt.exactness_posture, TassadarExactnessPosture::Exact);
        assert!(receipt.trace_digest_equal);
        assert!(receipt.outputs_equal);
        assert!(receipt.halt_equal);
        assert!(receipt.mismatch_summary.is_none());
    }

    #[test]
    fn tassadar_exactness_refusal_receipt_preserves_refusal_reason() {
        let case = psionic_runtime::tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let mut refused_program = case.program.clone();
        refused_program.profile_id = String::from("tassadar.wasm.unsupported_profile.v0");
        let selection = psionic_runtime::diagnose_tassadar_executor_request(
            &refused_program,
            psionic_runtime::TassadarExecutorDecodeMode::ReferenceLinear,
            psionic_runtime::TassadarTraceAbi::core_i32_v1().schema_version,
            None,
        );
        let runtime_report =
            TassadarExactnessRefusalReport::from_refusal(&case.case_id, &selection, None);

        let receipt = TassadarExactnessRefusalReceipt::from_runtime_report(&runtime_report);

        assert_eq!(receipt.exactness_posture, TassadarExactnessPosture::Refused);
        assert_eq!(
            receipt.selection_reason,
            Some(psionic_runtime::TassadarExecutorSelectionReason::UnsupportedWasmProfile)
        );
        assert!(receipt.expected_behavior_digest.is_none());
        assert!(receipt.actual_behavior_digest.is_none());
    }

    #[test]
    fn tassadar_promotion_policy_receipt_surfaces_refusal_and_route_blockers() {
        let report =
            build_tassadar_promotion_policy_report().expect("promotion policy report should build");

        let receipt = TassadarPromotionPolicyReceipt::from_report(&report);

        assert_eq!(
            receipt.status,
            TassadarPromotionPolicyStatus::PromotionBlocked
        );
        assert_eq!(
            receipt.failed_gates,
            vec![
                TassadarPromotionChecklistGateKind::RefusalBehavior,
                TassadarPromotionChecklistGateKind::RouteContractCompatibility,
            ]
        );
        assert_eq!(receipt.candidate_model_id, report.candidate_model_id);
    }

    #[test]
    fn tassadar_decompilation_receipt_projects_research_artifact_summary()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_decompilable_executor_artifacts_report()?;
        let summary = report
            .artifact_summaries
            .iter()
            .find(|summary| summary.source_case_id == "addition_pair")
            .expect("addition summary should exist");

        let receipt = TassadarDecompilationReceipt::from_artifact_summary(summary);

        assert_eq!(receipt.source_case_id, "addition_pair");
        assert_eq!(receipt.retrain_count, 3);
        assert!(receipt.receipt_ready);
        assert!(receipt.detail.contains("receipt-ready"));
        Ok(())
    }
}
