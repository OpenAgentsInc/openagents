use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    TassadarArticleTransformer, TassadarExecutorContractError, TassadarExecutorFixture,
    TassadarExecutorModelDescriptor, TassadarGeneralizedAbiPublication,
    TassadarInternalComputeProfileClaimCheckResult,
    TassadarInternalComputeProfileLadderPublication, TassadarModuleExecutionCapabilityPublication,
    TassadarRustArticleProfileCompletenessPublication, TassadarTraceTokenizer,
    TassadarWorkloadCapabilityMatrix, TassadarWorkloadCapabilityMatrixError,
    TassadarWorkloadCapabilityRow, TassadarWorkloadSupportPosture,
    build_tassadar_article_transformer_trained_executor_descriptor,
    check_tassadar_internal_compute_profile_claim,
    tassadar_current_served_internal_compute_profile_claim, tassadar_generalized_abi_publication,
    tassadar_internal_compute_profile_ladder_publication,
    tassadar_rust_article_profile_completeness_publication,
};
use psionic_research::{
    TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_SUMMARY_REPORT_REF, TassadarAcceptanceReport,
    TassadarCompiledArticleClosureReport, TassadarLearnedLongHorizonPolicyReport,
    TassadarPromotionChecklistGateKind, TassadarPromotionPolicyReport,
    build_tassadar_article_runtime_closeout_summary_report, build_tassadar_promotion_policy_report,
};
use psionic_router::{
    TassadarPlannerExecutorDecodeCapability, TassadarPlannerExecutorRouteDescriptor,
    TassadarPlannerExecutorRoutePosture, TassadarPlannerExecutorRouteRefusalReason,
    TassadarPlannerExecutorWasmCapabilityMatrix, TassadarPlannerExecutorWasmCapabilityRow,
    TassadarPlannerExecutorWasmImportPosture, TassadarPlannerExecutorWasmOpcodeFamily,
};
use psionic_runtime::{
    TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF, TASSADAR_ARTICLE_CLASS_BENCHMARK_REF,
    TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF, TassadarDirectModelWeightExecutionProofReceipt,
    TassadarExecution, TassadarExecutionEvidenceBundle, TassadarExecutionRefusal,
    TassadarExecutorDecodeMode, TassadarExecutorExecutionReport,
    TassadarExecutorSelectionDiagnostic, TassadarFrozenCoreWasmClosureGateStatus,
    TassadarInstruction, TassadarProgramArtifact, TassadarRuntimeCapabilityReport,
    TassadarTraceEvent, TassadarTraceStep, TassadarValidationCase,
    build_tassadar_execution_evidence_bundle, diagnose_tassadar_executor_request,
    execute_tassadar_executor_request, tassadar_article_class_corpus,
    tassadar_article_hard_sudoku_suite, tassadar_hungarian_10x10_corpus,
    tassadar_trace_abi_for_profile_id, tassadar_wasm_profile_for_id,
};
use psionic_train::{TassadarExecutorPromotionGateReport, TassadarExecutorSequenceFitReport};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Dedicated served product identifier for the Tassadar executor lane.
pub const EXECUTOR_TRACE_PRODUCT_ID: &str = "psionic.executor_trace";

/// Dedicated planner-owned routing product for exact executor delegation.
pub const PLANNER_EXECUTOR_ROUTE_PRODUCT_ID: &str = "psionic.planner_executor_route";
/// Dedicated served product identifier for article-class Tassadar sessions.
pub const ARTICLE_EXECUTOR_SESSION_PRODUCT_ID: &str = "psionic.article_executor_session";
/// Dedicated planner-owned hybrid workflow product for article-class compute spans.
pub const ARTICLE_HYBRID_WORKFLOW_PRODUCT_ID: &str = "psionic.article_hybrid_workflow";
/// Canonical acceptance artifact for the article-session serving surface.
pub const TASSADAR_ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_executor_session_artifact.json";
/// Canonical acceptance artifact for the article hybrid-workflow surface.
pub const TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_hybrid_workflow_artifact.json";
/// Canonical served publication for the bounded article Transformer replacement certificate.
pub const TASSADAR_ARTICLE_TRANSFORMER_REPLACEMENT_PUBLICATION_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_replacement_publication.json";
/// Canonical acceptance artifact for the replay/live Tassadar lab surface.
pub const TASSADAR_LAB_SURFACE_ARTIFACT_REF: &str =
    "fixtures/tassadar/reports/tassadar_lab_surface_artifact.json";

const ARTICLE_EXECUTOR_READABLE_LOG_MAX_LINES: usize = 96;
const ARTICLE_EXECUTOR_TOKEN_TRACE_MAX_TOKENS: usize = 256;
const ARTICLE_EXECUTOR_TOKEN_TRACE_CHUNK_SIZE: usize = 32;

/// Benchmark-gated served capability publication for the explicit executor-trace lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorCapabilityPublication {
    /// Served product identifier.
    pub product_id: String,
    /// Served executor model descriptor.
    pub model_descriptor: TassadarExecutorModelDescriptor,
    /// Runtime capability visible to the caller.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Repo-facing publication for bounded module execution and host-import boundary truth.
    pub module_execution_capability: TassadarModuleExecutionCapabilityPublication,
    /// Rust-to-Wasm article profile completeness matrix for the served lane.
    pub rust_article_profile_completeness: TassadarRustArticleProfileCompletenessPublication,
    /// Generalized ABI family publication for the served lane.
    pub generalized_abi_family: TassadarGeneralizedAbiPublication,
    /// Named post-article internal-compute profile ladder.
    pub internal_compute_profile_ladder: TassadarInternalComputeProfileLadderPublication,
    /// Claim-check result for the named served internal-compute profile.
    pub internal_compute_profile_claim_check: TassadarInternalComputeProfileClaimCheckResult,
    /// Broad internal-compute portability report bound to the served lane.
    pub broad_internal_compute_portability_report_ref: String,
    /// Backend families currently carried by the broad internal-compute portability matrix.
    pub broad_internal_compute_portability_backend_family_ids: Vec<String>,
    /// Toolchain families currently carried by the broad internal-compute portability matrix.
    pub broad_internal_compute_portability_toolchain_family_ids: Vec<String>,
    /// Numeric portability report bound to the served lane.
    pub numeric_portability_report_ref: String,
    /// Backend families currently carried by the numeric portability matrix.
    pub numeric_portability_backend_family_ids: Vec<String>,
    /// Toolchain families currently carried by the numeric portability matrix.
    pub numeric_portability_toolchain_family_ids: Vec<String>,
    /// Numeric profile ids currently carried by the numeric portability matrix.
    pub numeric_portability_profile_ids: Vec<String>,
    /// Float-profile acceptance gate report bound to the served lane.
    pub float_profile_acceptance_gate_report_ref: String,
    /// Float-profile route-policy report bound to the served lane.
    pub float_profile_route_policy_report_ref: String,
    /// Exception-profile report bound to the served lane.
    pub exception_profile_report_ref: String,
    /// Exception profiles currently allowed as named public profiles.
    pub exception_profile_public_profile_ids: Vec<String>,
    /// Exception profiles currently allowed as default served profiles.
    pub exception_profile_default_served_profile_ids: Vec<String>,
    /// Portability envelopes currently bound to the named exception profiles.
    pub exception_profile_portability_envelope_ids: Vec<String>,
    /// SIMD-profile report bound to the served lane.
    pub simd_profile_report_ref: String,
    /// SIMD profiles currently allowed as named public profiles.
    pub simd_profile_public_profile_ids: Vec<String>,
    /// SIMD profiles currently allowed as default served profiles.
    pub simd_profile_default_served_profile_ids: Vec<String>,
    /// Exact backend ids currently bound to the named SIMD profile.
    pub simd_profile_exact_backend_ids: Vec<String>,
    /// Scalar-fallback backend ids currently bound to the named SIMD profile.
    pub simd_profile_fallback_backend_ids: Vec<String>,
    /// Refused backend ids currently bound to the named SIMD profile.
    pub simd_profile_refused_backend_ids: Vec<String>,
    /// Session-process profile report bound to the served lane.
    pub session_process_profile_report_ref: String,
    /// Session-process route-policy report bound to the served lane.
    pub session_process_route_policy_report_ref: String,
    /// Session-process profiles currently allowed as named public profiles.
    pub session_process_public_profile_ids: Vec<String>,
    /// Session-process profiles currently allowed as default served profiles.
    pub session_process_default_served_profile_ids: Vec<String>,
    /// Routeable interaction surfaces currently bound to the named session-process profile.
    pub session_process_routeable_interaction_surface_ids: Vec<String>,
    /// Refused interaction surfaces currently bound to the named session-process profile.
    pub session_process_refused_interaction_surface_ids: Vec<String>,
    /// Async-lifecycle profile report bound to the served lane.
    pub async_lifecycle_profile_report_ref: String,
    /// Async-lifecycle route-policy report bound to the served lane.
    pub async_lifecycle_route_policy_report_ref: String,
    /// Async-lifecycle profiles currently allowed as named public profiles.
    pub async_lifecycle_public_profile_ids: Vec<String>,
    /// Async-lifecycle profiles currently allowed as default served profiles.
    pub async_lifecycle_default_served_profile_ids: Vec<String>,
    /// Routeable lifecycle surfaces currently bound to the named async-lifecycle profile.
    pub async_lifecycle_routeable_surface_ids: Vec<String>,
    /// Refused lifecycle surfaces currently bound to the named async-lifecycle profile.
    pub async_lifecycle_refused_surface_ids: Vec<String>,
    /// Preemptive-job profile report bound to the served lane.
    pub preemptive_job_profile_report_ref: String,
    /// Preemptive-job fairness report bound to the served lane.
    pub preemptive_job_fairness_report_ref: String,
    /// Stable bounded preemptive-job profile identifier.
    pub preemptive_job_profile_id: String,
    /// Scheduler ids currently green for the bounded preemptive-job profile.
    pub preemptive_job_green_scheduler_ids: Vec<String>,
    /// Scheduler ids currently refused for the bounded preemptive-job profile.
    pub preemptive_job_refused_scheduler_ids: Vec<String>,
    /// Exact preemptive-job case count bound to the served lane.
    pub preemptive_job_exact_case_count: u32,
    /// Explicit refusal-row count bound to the served lane.
    pub preemptive_job_refusal_case_count: u32,
    /// Whether served publication is allowed for the bounded preemptive-job profile.
    pub preemptive_job_served_publication_allowed: bool,
    /// Broad internal-compute acceptance gate bound to the served lane.
    pub broad_internal_compute_acceptance_gate_report_ref: String,
    /// Broad internal-compute profile publication and current route selection.
    pub broad_internal_compute_profile_publication:
        crate::TassadarBroadInternalComputeProfilePublication,
    /// Frozen core-Wasm window declaration report bound to the served lane.
    pub frozen_core_wasm_window_report_ref: String,
    /// Frozen core-Wasm closure gate report bound to the served lane.
    pub frozen_core_wasm_closure_gate_report_ref: String,
    /// Current closure status for the frozen core-Wasm target.
    pub frozen_core_wasm_closure_status: TassadarFrozenCoreWasmClosureGateStatus,
    /// Whether served publication can claim full frozen core-Wasm closure.
    pub frozen_core_wasm_served_publication_allowed: bool,
    /// Deterministic-import and runtime-support subset promotion gate report bound to the served lane.
    pub subset_profile_promotion_gate_report_ref: String,
    /// Resumable multi-slice promotion report bound to the served lane.
    pub resumable_multi_slice_promotion_report_ref: String,
    /// Deterministic import-mediated effect-safe resume report bound to the served lane.
    pub effect_safe_resume_report_ref: String,
    /// Machine-readable workload capability matrix for the served lane.
    pub workload_capability_matrix: TassadarWorkloadCapabilityMatrix,
    /// Backend and quantization deployment truth carried through served publication.
    pub quantization_truth_envelope: crate::TassadarServedQuantizationTruthEnvelope,
}

/// Served publication for the Rust-only article runtime closeout surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarRustOnlyArticleRuntimeCloseoutPublication {
    /// Served product identifier.
    pub product_id: String,
    /// Stable served model identifier.
    pub model_id: String,
    /// Stable report reference for the committed closeout report.
    pub report_ref: String,
    /// Exact horizon count carried by the report.
    pub exact_horizon_count: u32,
    /// Floor pass count carried by the report.
    pub floor_pass_count: u32,
    /// Slowest committed horizon identifier.
    pub slowest_workload_horizon_id: String,
    /// Slowest measured direct throughput on the committed operator machine.
    pub slowest_measured_steps_per_second: f64,
    /// Explicit claim boundary.
    pub claim_boundary: String,
}

/// Served publication for the bounded fixture-to-Transformer replacement certificate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerReplacementPublication {
    /// Served product identifier.
    pub product_id: String,
    /// Stable report reference for the committed parity report.
    pub report_ref: String,
    /// Stable benchmark report reference carried through the parity report.
    pub benchmark_report_ref: String,
    /// Stable lineage contract reference carried through the parity report.
    pub lineage_contract_ref: String,
    /// Historical fixture-model identifier.
    pub fixture_model_id: String,
    /// Replacement Transformer-model identifier.
    pub transformer_model_id: String,
    /// Number of declared article cases certified by the replacement report.
    pub supported_case_count: u32,
    /// Stable declared article case ids.
    pub supported_case_ids: Vec<String>,
    /// Number of cases that remained routeable on both sides of the parity harness.
    pub routeable_case_count: u32,
    /// Number of cases with exact trace parity.
    pub exact_trace_case_count: u32,
    /// Number of cases with exact output and terminal-state parity.
    pub exact_output_case_count: u32,
    /// Whether the bounded replacement certificate is green.
    pub replacement_certified: bool,
    /// Whether final article equivalence is already green.
    pub article_equivalence_green: bool,
    /// Explicit served claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the served publication.
    pub publication_digest: String,
}

/// Capability-publication failure for the explicit executor-trace lane.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarExecutorCapabilityPublicationError {
    /// The requested executor model is not registered.
    #[error("unknown Tassadar executor model `{model_id}`")]
    UnknownModel {
        /// Requested model identifier.
        model_id: String,
    },
    /// The workload capability matrix is not publishable yet.
    #[error("invalid Tassadar workload capability publication: {error}")]
    InvalidWorkloadCapabilityMatrix {
        /// Validation failure from the shared model contract.
        error: TassadarWorkloadCapabilityMatrixError,
    },
    /// The backend-specific quantization truth envelope was not publishable.
    #[error("invalid Tassadar quantization truth envelope: {error}")]
    InvalidQuantizationTruthEnvelope {
        /// Validation failure from the served quantization-envelope projection.
        error: crate::TassadarServedQuantizationTruthEnvelopeError,
    },
    /// The Rust-only article runtime closeout report was not publishable.
    #[error("invalid Rust-only article runtime closeout report: {detail}")]
    InvalidRustOnlyArticleRuntimeCloseout {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The bounded fixture-to-Transformer replacement publication was not publishable.
    #[error("invalid article Transformer replacement publication: {detail}")]
    InvalidArticleTransformerReplacement {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The named internal-compute profile claim was not publishable.
    #[error("invalid internal-compute profile claim: {detail}")]
    InvalidInternalComputeProfileClaim {
        /// Machine-readable detail for the failed claim.
        detail: String,
    },
    /// The broad internal-compute profile publication was not publishable.
    #[error("invalid broad internal-compute profile publication: {detail}")]
    InvalidBroadInternalComputeProfilePublication {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The numeric portability report was not publishable.
    #[error("invalid numeric portability report: {detail}")]
    InvalidNumericPortability {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The float-profile acceptance gate was not publishable.
    #[error("invalid float-profile acceptance gate: {detail}")]
    InvalidFloatProfileAcceptanceGate {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The bounded exception profile was not publishable.
    #[error("invalid exception profile report: {detail}")]
    InvalidExceptionProfile {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The bounded SIMD profile was not publishable.
    #[error("invalid simd profile report: {detail}")]
    InvalidSimdProfile {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The bounded session-process profile was not publishable.
    #[error("invalid session-process profile report: {detail}")]
    InvalidSessionProcessProfile {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The bounded async-lifecycle profile was not publishable.
    #[error("invalid async-lifecycle profile report: {detail}")]
    InvalidAsyncLifecycleProfile {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The bounded preemptive-job profile was not publishable.
    #[error("invalid preemptive-job profile report: {detail}")]
    InvalidPreemptiveJobProfile {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The frozen core-Wasm closure gate was not publishable.
    #[error("invalid frozen core-Wasm closure gate: {detail}")]
    InvalidFrozenCoreWasmClosureGate {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
}

/// Explicit request contract for the served Tassadar executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier. Must be `psionic.executor_trace`.
    pub product_id: String,
    /// Optional explicit executor model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model_id: Option<String>,
    /// Digest-bound program artifact submitted to the executor.
    pub program_artifact: TassadarProgramArtifact,
    /// Requested decode mode for the execution.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Explicit environment refs carried into runtime-manifest lineage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl TassadarExecutorRequest {
    /// Creates a request for the explicit executor-trace product family.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        program_artifact: TassadarProgramArtifact,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
            requested_model_id: None,
            program_artifact,
            requested_decode_mode,
            environment_refs: Vec::new(),
        }
    }

    /// Pins execution to one explicit executor model.
    #[must_use]
    pub fn with_requested_model_id(mut self, requested_model_id: impl Into<String>) -> Self {
        self.requested_model_id = Some(requested_model_id.into());
        self
    }

    /// Carries environment refs into the served evidence bundle.
    #[must_use]
    pub fn with_environment_refs(mut self, environment_refs: Vec<String>) -> Self {
        let mut environment_refs = environment_refs;
        environment_refs.sort();
        environment_refs.dedup();
        self.environment_refs = environment_refs;
        self
    }

    /// Returns a stable digest for the request surface.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("Tassadar executor request should serialize");
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_executor_request|");
        hasher.update(encoded);
        hex::encode(hasher.finalize())
    }
}

/// Explicit refusal response for unsupported ABI, profile, decode, or model pairings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorRefusalResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Served executor model descriptor that evaluated the request.
    pub model_descriptor: TassadarExecutorModelDescriptor,
    /// Runtime capability report visible to the caller.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Contract error when model/program pairing failed before selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_error: Option<TassadarExecutorContractError>,
    /// Runtime selection diagnostic when decode selection reached refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<TassadarExecutorSelectionDiagnostic>,
    /// Human-readable refusal detail.
    pub detail: String,
}

/// Completed served executor response carrying the exact runtime truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Served executor model descriptor.
    pub model_descriptor: TassadarExecutorModelDescriptor,
    /// Runtime capability report visible to the caller.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Direct/fallback selection plus realized execution.
    pub execution_report: TassadarExecutorExecutionReport,
    /// Runtime-manifest, trace, proof, and proof-bundle evidence.
    pub evidence_bundle: TassadarExecutionEvidenceBundle,
    /// Ordered environment refs carried into execution lineage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl TassadarExecutorResponse {
    /// Returns the final scalar outputs emitted by the execution.
    #[must_use]
    pub fn final_outputs(&self) -> &[i32] {
        &self.execution_report.execution.outputs
    }

    /// Returns the emitted trace artifact.
    #[must_use]
    pub fn trace_artifact(&self) -> &psionic_runtime::TassadarTraceArtifact {
        &self.evidence_bundle.trace_artifact
    }

    /// Returns the proof-bearing trace artifact.
    #[must_use]
    pub fn trace_proof(&self) -> &psionic_runtime::TassadarTraceProofArtifact {
        &self.evidence_bundle.trace_proof
    }
}

/// Served outcome for one executor request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarExecutorOutcome {
    /// The executor accepted and completed the request.
    Completed { response: TassadarExecutorResponse },
    /// The executor refused the request explicitly.
    Refused {
        refusal: TassadarExecutorRefusalResponse,
    },
}

/// Step event emitted by the explicit executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTraceStepEvent {
    /// Ordinal step index.
    pub step_index: u64,
    /// Full append-only trace step.
    pub step: TassadarTraceStep,
}

/// Output event emitted by the explicit executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorOutputEvent {
    /// Output ordinal in the final ordered output vector.
    pub ordinal: usize,
    /// Output scalar value.
    pub value: i32,
}

/// Terminal event emitted by the explicit executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTerminalEvent {
    /// Final served outcome.
    pub outcome: TassadarExecutorOutcome,
}

/// Typed event emitted by the pull-driven executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutorStreamEvent {
    /// Runtime capability report surfaced before execution.
    Capability {
        /// Runtime capability report visible to the caller.
        runtime_capability: TassadarRuntimeCapabilityReport,
    },
    /// Decode-selection diagnostic surfaced before the first trace step.
    Selection {
        /// Direct/fallback/refused selection diagnostic.
        selection: TassadarExecutorSelectionDiagnostic,
    },
    /// One append-only trace step.
    TraceStep {
        /// Step event payload.
        trace_step: TassadarExecutorTraceStepEvent,
    },
    /// One emitted output value.
    Output {
        /// Output event payload.
        output: TassadarExecutorOutputEvent,
    },
    /// Terminal completion or refusal.
    Terminal {
        /// Terminal payload.
        terminal: TassadarExecutorTerminalEvent,
    },
}

/// Typed request-validation or model-resolution error for the served executor lane.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutorServiceError {
    /// The request targeted a different served product family.
    #[error("unsupported Tassadar served product `{product_id}`")]
    UnsupportedProduct {
        /// Product identifier supplied by the caller.
        product_id: String,
    },
    /// The request named an executor model that is not registered.
    #[error("unknown Tassadar executor model `{model_id}`")]
    UnknownModel {
        /// Requested model identifier.
        model_id: String,
    },
    /// The Transformer-backed article replacement lane could not be projected honestly.
    #[error("invalid article Transformer replacement lane: {detail}")]
    InvalidArticleTransformerReplacement {
        /// Machine-readable detail for the failed projection.
        detail: String,
    },
    /// The request reached execution even though runtime selection refused it.
    #[error(transparent)]
    ExecutionRefusal(#[from] TassadarExecutionRefusal),
}

/// Pre-execution contract and selection report for one executor request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorPreflightReport {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Served executor model descriptor that evaluated the request.
    pub model_descriptor: TassadarExecutorModelDescriptor,
    /// Runtime capability report visible to the caller.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Contract error when the request failed before decode selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_error: Option<TassadarExecutorContractError>,
    /// Decode selection diagnostic when contract validation succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<TassadarExecutorSelectionDiagnostic>,
}

/// Pull-driven local stream for explicit executor-trace products.
#[derive(Clone, Debug, Default)]
pub struct LocalTassadarExecutorStream {
    events: VecDeque<TassadarExecutorStreamEvent>,
}

impl LocalTassadarExecutorStream {
    fn from_events(events: Vec<TassadarExecutorStreamEvent>) -> Self {
        Self {
            events: VecDeque::from(events),
        }
    }

    /// Returns the next typed stream event.
    pub fn next_event(&mut self) -> Option<TassadarExecutorStreamEvent> {
        self.events.pop_front()
    }
}

/// Local reference implementation of the explicit Tassadar served product.
#[derive(Clone, Debug)]
pub struct LocalTassadarExecutorService {
    fixtures: BTreeMap<String, TassadarExecutorFixture>,
    custom_model_descriptors: BTreeMap<String, TassadarExecutorModelDescriptor>,
    default_model_id: String,
}

fn is_transformer_backed_article_model_id(model_id: &str) -> bool {
    model_id == TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
}

fn transformer_backed_article_fixture() -> TassadarExecutorFixture {
    TassadarExecutorFixture::article_i32_compute_v1()
}

fn transformer_backed_article_fixture_for_profile_id(
    profile_id: &str,
) -> Result<TassadarExecutorFixture, String> {
    TassadarExecutorFixture::for_profile_id(profile_id).ok_or_else(|| {
        format!(
            "trained Transformer-backed article route does not declare fixture compatibility for Wasm profile `{profile_id}`"
        )
    })
}

fn ensure_transformer_backed_article_replacement_ready() -> Result<(), String> {
    let parity_report = psionic_eval::build_tassadar_article_fixture_transformer_parity_report()
        .map_err(|error| {
            format!("failed to build fixture-to-Transformer parity report: {error}")
        })?;
    if !parity_report.replacement_certified {
        return Err(String::from(
            "fixture-to-Transformer parity report is not green enough to publish the trained Transformer-backed article route",
        ));
    }
    if parity_report.transformer_model_artifact.model_id
        != TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
    {
        return Err(format!(
            "parity report model `{}` did not match trained Transformer model `{}`",
            parity_report.transformer_model_artifact.model_id,
            TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
        ));
    }
    Ok(())
}

fn transformer_backed_article_model_descriptor() -> Result<TassadarExecutorModelDescriptor, String>
{
    ensure_transformer_backed_article_replacement_ready()?;
    build_tassadar_article_transformer_trained_executor_descriptor().map_err(|error| {
        format!("failed to build trained Transformer-backed article descriptor: {error}")
    })
}

fn transformer_backed_article_model_descriptor_for_profile_id(
    profile_id: &str,
) -> Result<TassadarExecutorModelDescriptor, String> {
    let mut descriptor = transformer_backed_article_model_descriptor()?;
    let fixture = transformer_backed_article_fixture_for_profile_id(profile_id)?;
    descriptor.compatibility = fixture.descriptor().compatibility.clone();
    descriptor.profile = fixture.descriptor().profile.clone();
    descriptor.trace_abi = fixture.descriptor().trace_abi.clone();
    Ok(descriptor)
}

fn transformer_backed_article_workload_capability_matrix()
-> Result<TassadarWorkloadCapabilityMatrix, String> {
    let descriptor = transformer_backed_article_model_descriptor()?;
    Ok(transformer_backed_article_workload_capability_matrix_for_descriptor(&descriptor))
}

fn transformer_backed_article_workload_capability_matrix_for_descriptor(
    descriptor: &TassadarExecutorModelDescriptor,
) -> TassadarWorkloadCapabilityMatrix {
    let baseline = transformer_backed_article_fixture().workload_capability_matrix();
    TassadarWorkloadCapabilityMatrix::new(
        descriptor.model.model_id.clone(),
        descriptor.model.family.clone(),
        baseline.rows,
        String::from(
            "this workload matrix projects the certified article fixture workload boundary onto the trained trace-bound Transformer-backed article route. It now carries explicit direct HullCache exactness on the declared article workload families while still leaving TAS-175 throughput closure open.",
        ),
    )
}

impl Default for LocalTassadarExecutorService {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalTassadarExecutorService {
    /// Creates the default in-process Tassadar executor service.
    #[must_use]
    pub fn new() -> Self {
        let fixture = TassadarExecutorFixture::new();
        let model_id = fixture.descriptor().model.model_id.clone();
        let mut fixtures = BTreeMap::new();
        fixtures.insert(model_id.clone(), fixture);
        Self {
            fixtures,
            custom_model_descriptors: BTreeMap::new(),
            default_model_id: model_id,
        }
    }

    /// Registers one additional executor fixture.
    #[must_use]
    pub fn with_fixture(mut self, fixture: TassadarExecutorFixture) -> Self {
        let model_id = fixture.descriptor().model.model_id.clone();
        self.fixtures.insert(model_id.clone(), fixture);
        if self.fixtures.len() == 1 {
            self.default_model_id = model_id;
        }
        self
    }

    /// Registers one additional Transformer-backed article model descriptor.
    #[must_use]
    pub fn with_model_descriptor(
        mut self,
        model_descriptor: TassadarExecutorModelDescriptor,
    ) -> Self {
        let model_id = model_descriptor.model.model_id.clone();
        self.custom_model_descriptors
            .insert(model_id.clone(), model_descriptor);
        self
    }

    /// Returns the benchmark-gated served capability publication for one fixture.
    pub fn capability_publication(
        &self,
        requested_model_id: Option<&str>,
    ) -> Result<TassadarExecutorCapabilityPublication, TassadarExecutorCapabilityPublicationError>
    {
        let requested_model_id = requested_model_id.unwrap_or(self.default_model_id.as_str());
        let (
            model_descriptor,
            workload_capability_matrix,
            runtime_capability,
            module_execution_capability,
        ) = if let Ok(model_descriptor) = self
            .resolve_custom_model_descriptor_by_model_id(Some(requested_model_id))
            .cloned()
        {
            (
                model_descriptor.clone(),
                transformer_backed_article_workload_capability_matrix_for_descriptor(
                    &model_descriptor,
                ),
                TassadarRuntimeCapabilityReport::current(),
                transformer_backed_article_fixture().module_execution_capability_publication(),
            )
        } else if is_transformer_backed_article_model_id(requested_model_id) {
            let model_descriptor =
                    transformer_backed_article_model_descriptor().map_err(|detail| {
                        TassadarExecutorCapabilityPublicationError::InvalidArticleTransformerReplacement {
                            detail,
                        }
                    })?;
            (
                    model_descriptor,
                    transformer_backed_article_workload_capability_matrix().map_err(|detail| {
                        TassadarExecutorCapabilityPublicationError::InvalidArticleTransformerReplacement {
                            detail,
                        }
                    })?,
                    TassadarRuntimeCapabilityReport::current(),
                    transformer_backed_article_fixture().module_execution_capability_publication(),
                )
        } else {
            let fixture =
                self.resolve_fixture_by_model_id(Some(requested_model_id))
                    .map_err(|model_id| {
                        TassadarExecutorCapabilityPublicationError::UnknownModel { model_id }
                    })?;
            (
                fixture.descriptor().clone(),
                fixture.workload_capability_matrix(),
                fixture.runtime_capability_report(),
                fixture.module_execution_capability_publication(),
            )
        };
        workload_capability_matrix
            .validate_publication()
            .map_err(|error| {
                TassadarExecutorCapabilityPublicationError::InvalidWorkloadCapabilityMatrix {
                    error,
                }
            })?;
        let internal_compute_profile_ladder =
            tassadar_internal_compute_profile_ladder_publication();
        let internal_compute_profile_claim_check = check_tassadar_internal_compute_profile_claim(
            &internal_compute_profile_ladder,
            tassadar_current_served_internal_compute_profile_claim(),
        );
        if !internal_compute_profile_claim_check.green {
            return Err(
                TassadarExecutorCapabilityPublicationError::InvalidInternalComputeProfileClaim {
                    detail: internal_compute_profile_claim_check.detail.clone(),
                },
            );
        }
        let quantization_truth_envelope = crate::build_tassadar_served_quantization_truth_envelope(
            runtime_capability.runtime_backend.as_str(),
        )
        .map_err(|error| {
            TassadarExecutorCapabilityPublicationError::InvalidQuantizationTruthEnvelope { error }
        })?;
        let broad_internal_compute_portability_report =
            psionic_eval::build_tassadar_broad_internal_compute_portability_report().map_err(
                |error| {
                    TassadarExecutorCapabilityPublicationError::InvalidBroadInternalComputeProfilePublication {
                        detail: format!(
                            "invalid broad internal-compute portability report: {error}"
                        ),
                    }
                },
            )?;
        let broad_internal_compute_profile_publication =
            crate::build_tassadar_broad_internal_compute_profile_publication().map_err(
                |error| {
                    TassadarExecutorCapabilityPublicationError::InvalidBroadInternalComputeProfilePublication {
                        detail: error.to_string(),
                    }
                },
            )?;
        let numeric_portability_report = psionic_eval::build_tassadar_numeric_portability_report()
            .map_err(|error| {
                TassadarExecutorCapabilityPublicationError::InvalidNumericPortability {
                    detail: format!("invalid numeric portability report: {error}"),
                }
            })?;
        psionic_eval::build_tassadar_float_profile_acceptance_gate_report().map_err(|error| {
            TassadarExecutorCapabilityPublicationError::InvalidFloatProfileAcceptanceGate {
                detail: format!("invalid float-profile acceptance gate report: {error}"),
            }
        })?;
        psionic_router::build_tassadar_float_profile_route_policy_report().map_err(|error| {
            TassadarExecutorCapabilityPublicationError::InvalidFloatProfileAcceptanceGate {
                detail: format!("invalid float-profile route policy report: {error}"),
            }
        })?;
        let exception_profile_report = psionic_eval::build_tassadar_exception_profile_report();
        if !exception_profile_report.overall_green
            || exception_profile_report
                .public_profile_allowed_profile_ids
                .is_empty()
        {
            return Err(
                TassadarExecutorCapabilityPublicationError::InvalidExceptionProfile {
                    detail: String::from(
                        "exception profile report must stay green and expose at least one named public profile",
                    ),
                },
            );
        }
        let simd_profile_report =
            psionic_eval::build_tassadar_simd_profile_report().map_err(|error| {
                TassadarExecutorCapabilityPublicationError::InvalidSimdProfile {
                    detail: format!("invalid simd profile report: {error}"),
                }
            })?;
        if !simd_profile_report.overall_green
            || simd_profile_report
                .public_profile_allowed_profile_ids
                .is_empty()
        {
            return Err(
                TassadarExecutorCapabilityPublicationError::InvalidSimdProfile {
                    detail: String::from(
                        "simd profile report must stay green and expose at least one named public profile",
                    ),
                },
            );
        }
        let session_process_profile_report =
            psionic_eval::build_tassadar_session_process_profile_report().map_err(|error| {
                TassadarExecutorCapabilityPublicationError::InvalidSessionProcessProfile {
                    detail: format!("invalid session-process profile report: {error}"),
                }
            })?;
        if !session_process_profile_report.overall_green
            || session_process_profile_report
                .public_profile_allowed_profile_ids
                .is_empty()
            || session_process_profile_report
                .routeable_interaction_surface_ids
                .is_empty()
        {
            return Err(
                TassadarExecutorCapabilityPublicationError::InvalidSessionProcessProfile {
                    detail: String::from(
                        "session-process profile report must stay green, expose at least one named public profile, and keep at least one routeable interaction surface",
                    ),
                },
            );
        }
        let session_process_route_policy_report =
            psionic_router::build_tassadar_session_process_route_policy_report().map_err(
                |error| TassadarExecutorCapabilityPublicationError::InvalidSessionProcessProfile {
                    detail: format!("invalid session-process route policy report: {error}"),
                },
            )?;
        if session_process_route_policy_report.promoted_profile_specific_route_count == 0
            || session_process_route_policy_report.refused_route_count == 0
        {
            return Err(
                TassadarExecutorCapabilityPublicationError::InvalidSessionProcessProfile {
                    detail: String::from(
                        "session-process route policy report must keep at least one promoted profile-specific route and one refused route",
                    ),
                },
            );
        }
        let async_lifecycle_profile_report =
            psionic_eval::build_tassadar_async_lifecycle_profile_report().map_err(|error| {
                TassadarExecutorCapabilityPublicationError::InvalidAsyncLifecycleProfile {
                    detail: format!("invalid async-lifecycle profile report: {error}"),
                }
            })?;
        if !async_lifecycle_profile_report.overall_green
            || async_lifecycle_profile_report
                .public_profile_allowed_profile_ids
                .is_empty()
            || async_lifecycle_profile_report
                .routeable_lifecycle_surface_ids
                .is_empty()
        {
            return Err(
                TassadarExecutorCapabilityPublicationError::InvalidAsyncLifecycleProfile {
                    detail: String::from(
                        "async-lifecycle profile report must stay green, expose at least one named public profile, and keep at least one routeable lifecycle surface",
                    ),
                },
            );
        }
        let async_lifecycle_route_policy_report =
            psionic_router::build_tassadar_async_lifecycle_route_policy_report().map_err(
                |error| TassadarExecutorCapabilityPublicationError::InvalidAsyncLifecycleProfile {
                    detail: format!("invalid async-lifecycle route policy report: {error}"),
                },
            )?;
        if async_lifecycle_route_policy_report.promoted_profile_specific_route_count == 0
            || async_lifecycle_route_policy_report.refused_route_count == 0
        {
            return Err(
                TassadarExecutorCapabilityPublicationError::InvalidAsyncLifecycleProfile {
                    detail: String::from(
                        "async-lifecycle route policy report must keep at least one promoted profile-specific route and one refused route",
                    ),
                },
            );
        }
        let preemptive_job_profile_report =
            psionic_eval::build_tassadar_preemptive_job_profile_report().map_err(|error| {
                TassadarExecutorCapabilityPublicationError::InvalidPreemptiveJobProfile {
                    detail: format!("invalid preemptive-job profile report: {error}"),
                }
            })?;
        if !preemptive_job_profile_report.overall_green
            || preemptive_job_profile_report.green_scheduler_ids.is_empty()
            || preemptive_job_profile_report
                .refused_scheduler_ids
                .is_empty()
            || preemptive_job_profile_report.served_publication_allowed
        {
            return Err(
                TassadarExecutorCapabilityPublicationError::InvalidPreemptiveJobProfile {
                    detail: String::from(
                        "preemptive-job profile report must stay green, keep at least one green scheduler and one refused scheduler, and remain non-served",
                    ),
                },
            );
        }
        psionic_eval::build_tassadar_subset_profile_promotion_gate_report().map_err(|error| {
            TassadarExecutorCapabilityPublicationError::InvalidBroadInternalComputeProfilePublication {
                detail: format!("invalid subset profile promotion gate report: {error}"),
            }
        })?;
        psionic_eval::build_tassadar_resumable_multi_slice_promotion_report().map_err(|error| {
            TassadarExecutorCapabilityPublicationError::InvalidBroadInternalComputeProfilePublication {
                detail: format!(
                    "invalid resumable multi-slice promotion report: {error}"
                ),
            }
        })?;
        psionic_eval::build_tassadar_effect_safe_resume_report().map_err(|error| {
            TassadarExecutorCapabilityPublicationError::InvalidBroadInternalComputeProfilePublication {
                detail: format!("invalid effect-safe resume report: {error}"),
            }
        })?;
        let frozen_core_wasm_closure_gate_report =
            psionic_eval::build_tassadar_frozen_core_wasm_closure_gate_report().map_err(
                |error| {
                    TassadarExecutorCapabilityPublicationError::InvalidFrozenCoreWasmClosureGate {
                        detail: format!("invalid frozen core-Wasm closure gate report: {error}"),
                    }
                },
            )?;
        Ok(TassadarExecutorCapabilityPublication {
            product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
            model_descriptor,
            runtime_capability,
            module_execution_capability,
            rust_article_profile_completeness:
                tassadar_rust_article_profile_completeness_publication(),
            generalized_abi_family: tassadar_generalized_abi_publication(),
            internal_compute_profile_ladder,
            internal_compute_profile_claim_check,
            broad_internal_compute_portability_report_ref: String::from(
                psionic_runtime::TASSADAR_BROAD_INTERNAL_COMPUTE_PORTABILITY_REPORT_REF,
            ),
            broad_internal_compute_portability_backend_family_ids:
                broad_internal_compute_portability_report.backend_family_ids,
            broad_internal_compute_portability_toolchain_family_ids:
                broad_internal_compute_portability_report.toolchain_family_ids,
            numeric_portability_report_ref: String::from(
                psionic_runtime::TASSADAR_NUMERIC_PORTABILITY_REPORT_REF,
            ),
            numeric_portability_backend_family_ids: numeric_portability_report.backend_family_ids,
            numeric_portability_toolchain_family_ids: numeric_portability_report
                .toolchain_family_ids,
            numeric_portability_profile_ids: numeric_portability_report.profile_ids,
            float_profile_acceptance_gate_report_ref: String::from(
                psionic_eval::TASSADAR_FLOAT_PROFILE_ACCEPTANCE_GATE_REPORT_REF,
            ),
            float_profile_route_policy_report_ref: String::from(
                psionic_router::TASSADAR_FLOAT_PROFILE_ROUTE_POLICY_REPORT_REF,
            ),
            exception_profile_report_ref: String::from(
                psionic_eval::TASSADAR_EXCEPTION_PROFILE_REPORT_REF,
            ),
            exception_profile_public_profile_ids: exception_profile_report
                .public_profile_allowed_profile_ids,
            exception_profile_default_served_profile_ids: exception_profile_report
                .default_served_profile_allowed_profile_ids,
            exception_profile_portability_envelope_ids: exception_profile_report
                .portability_envelope_ids,
            simd_profile_report_ref: String::from(psionic_eval::TASSADAR_SIMD_PROFILE_REPORT_REF),
            simd_profile_public_profile_ids: simd_profile_report.public_profile_allowed_profile_ids,
            simd_profile_default_served_profile_ids: simd_profile_report
                .default_served_profile_allowed_profile_ids,
            simd_profile_exact_backend_ids: simd_profile_report.exact_backend_ids,
            simd_profile_fallback_backend_ids: simd_profile_report.fallback_backend_ids,
            simd_profile_refused_backend_ids: simd_profile_report.refused_backend_ids,
            session_process_profile_report_ref: String::from(
                psionic_eval::TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF,
            ),
            session_process_route_policy_report_ref: String::from(
                psionic_router::TASSADAR_SESSION_PROCESS_ROUTE_POLICY_REPORT_REF,
            ),
            session_process_public_profile_ids: session_process_profile_report
                .public_profile_allowed_profile_ids,
            session_process_default_served_profile_ids: session_process_profile_report
                .default_served_profile_allowed_profile_ids,
            session_process_routeable_interaction_surface_ids: session_process_profile_report
                .routeable_interaction_surface_ids,
            session_process_refused_interaction_surface_ids: session_process_profile_report
                .refused_interaction_surface_ids,
            async_lifecycle_profile_report_ref: String::from(
                psionic_eval::TASSADAR_ASYNC_LIFECYCLE_PROFILE_REPORT_REF,
            ),
            async_lifecycle_route_policy_report_ref: String::from(
                psionic_router::TASSADAR_ASYNC_LIFECYCLE_ROUTE_POLICY_REPORT_REF,
            ),
            async_lifecycle_public_profile_ids: async_lifecycle_profile_report
                .public_profile_allowed_profile_ids,
            async_lifecycle_default_served_profile_ids: async_lifecycle_profile_report
                .default_served_profile_allowed_profile_ids,
            async_lifecycle_routeable_surface_ids: async_lifecycle_profile_report
                .routeable_lifecycle_surface_ids,
            async_lifecycle_refused_surface_ids: async_lifecycle_profile_report
                .refused_lifecycle_surface_ids,
            preemptive_job_profile_report_ref: String::from(
                psionic_eval::TASSADAR_PREEMPTIVE_JOB_PROFILE_REPORT_REF,
            ),
            preemptive_job_fairness_report_ref: preemptive_job_profile_report
                .fairness_report_ref
                .clone(),
            preemptive_job_profile_id: preemptive_job_profile_report.profile_id.clone(),
            preemptive_job_green_scheduler_ids: preemptive_job_profile_report
                .green_scheduler_ids
                .clone(),
            preemptive_job_refused_scheduler_ids: preemptive_job_profile_report
                .refused_scheduler_ids
                .clone(),
            preemptive_job_exact_case_count: preemptive_job_profile_report.exact_case_count,
            preemptive_job_refusal_case_count: preemptive_job_profile_report.refusal_case_count,
            preemptive_job_served_publication_allowed: preemptive_job_profile_report
                .served_publication_allowed,
            broad_internal_compute_acceptance_gate_report_ref: String::from(
                psionic_eval::TASSADAR_BROAD_INTERNAL_COMPUTE_ACCEPTANCE_GATE_REPORT_REF,
            ),
            broad_internal_compute_profile_publication,
            frozen_core_wasm_window_report_ref: String::from(
                psionic_eval::TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF,
            ),
            frozen_core_wasm_closure_gate_report_ref: String::from(
                psionic_eval::TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF,
            ),
            frozen_core_wasm_closure_status: frozen_core_wasm_closure_gate_report.closure_status,
            frozen_core_wasm_served_publication_allowed: frozen_core_wasm_closure_gate_report
                .served_publication_allowed,
            subset_profile_promotion_gate_report_ref: String::from(
                psionic_eval::TASSADAR_SUBSET_PROFILE_PROMOTION_GATE_REPORT_REF,
            ),
            resumable_multi_slice_promotion_report_ref: String::from(
                psionic_eval::TASSADAR_RESUMABLE_MULTI_SLICE_PROMOTION_REPORT_REF,
            ),
            effect_safe_resume_report_ref: String::from(
                psionic_eval::TASSADAR_EFFECT_SAFE_RESUME_REPORT_REF,
            ),
            workload_capability_matrix,
            quantization_truth_envelope,
        })
    }

    /// Returns the served Rust-only article runtime closeout publication for one fixture.
    pub fn rust_only_article_runtime_closeout_publication(
        &self,
        requested_model_id: Option<&str>,
    ) -> Result<
        TassadarRustOnlyArticleRuntimeCloseoutPublication,
        TassadarExecutorCapabilityPublicationError,
    > {
        let fixture = self
            .resolve_fixture_by_model_id(requested_model_id)
            .map_err(
                |model_id| TassadarExecutorCapabilityPublicationError::UnknownModel { model_id },
            )?;
        let report = build_tassadar_article_runtime_closeout_summary_report().map_err(|error| {
            TassadarExecutorCapabilityPublicationError::InvalidRustOnlyArticleRuntimeCloseout {
                detail: error.to_string(),
            }
        })?;
        Ok(TassadarRustOnlyArticleRuntimeCloseoutPublication {
            product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
            model_id: fixture.descriptor().model.model_id.clone(),
            report_ref: String::from(TASSADAR_ARTICLE_RUNTIME_CLOSEOUT_SUMMARY_REPORT_REF),
            exact_horizon_count: report.closeout_report.exact_horizon_count,
            floor_pass_count: report.closeout_report.floor_pass_count,
            slowest_workload_horizon_id: report.slowest_workload_horizon_id,
            slowest_measured_steps_per_second: report.slowest_measured_steps_per_second,
            claim_boundary: String::from(
                "this served publication cites the committed Rust-only article runtime closeout report for benchmark-only long-horizon kernels. It does not widen the main served Wasm profile matrix or imply broad long-horizon closure outside those committed workloads",
            ),
        })
    }

    /// Returns the served bounded replacement certificate for the article Transformer route.
    pub fn article_transformer_replacement_publication(
        &self,
        requested_model_id: Option<&str>,
    ) -> Result<
        TassadarArticleTransformerReplacementPublication,
        TassadarExecutorCapabilityPublicationError,
    > {
        let fixture = self
            .resolve_fixture_by_model_id(requested_model_id)
            .map_err(
                |model_id| TassadarExecutorCapabilityPublicationError::UnknownModel { model_id },
            )?;
        if fixture.descriptor().model.model_id
            != TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID
        {
            return Err(
                TassadarExecutorCapabilityPublicationError::InvalidArticleTransformerReplacement {
                    detail: format!(
                        "replacement publication is only defined for article fixture `{}`, found `{}`",
                        TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
                        fixture.descriptor().model.model_id
                    ),
                },
            );
        }
        let report = psionic_eval::build_tassadar_article_fixture_transformer_parity_report()
            .map_err(|error| {
                TassadarExecutorCapabilityPublicationError::InvalidArticleTransformerReplacement {
                    detail: error.to_string(),
                }
            })?;
        if !report.replacement_publication_allowed {
            return Err(
                TassadarExecutorCapabilityPublicationError::InvalidArticleTransformerReplacement {
                    detail: String::from(
                        "fixture-to-Transformer parity report is not green enough for served replacement publication",
                    ),
                },
            );
        }
        let mut publication = TassadarArticleTransformerReplacementPublication {
            product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
            report_ref: String::from(
                psionic_eval::TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF,
            ),
            benchmark_report_ref: report.benchmark_report_ref.clone(),
            lineage_contract_ref: report.lineage_contract_ref.clone(),
            fixture_model_id: report.fixture_model_id.clone(),
            transformer_model_id: report.transformer_model_artifact.model_id.clone(),
            supported_case_count: report.supported_case_count as u32,
            supported_case_ids: report.declared_case_ids.clone(),
            routeable_case_count: report.routeable_case_count as u32,
            exact_trace_case_count: report.exact_trace_case_count as u32,
            exact_output_case_count: report.exact_output_case_count as u32,
            replacement_certified: report.replacement_certified,
            article_equivalence_green: report.article_equivalence_green,
            claim_boundary: String::from(
                "this served publication cites the bounded fixture-to-Transformer parity certificate for the declared article corpus only. It certifies replacement of the old fixture lane as the bounded truth carrier on those workloads, supports the trained Transformer-backed article route as the canonical served model identity, and now carries the TAS-174 no-fallback exactness closure on that declared article workload boundary, but it does not by itself close TAS-175 throughput-floor closure or the broader article-equivalence claim boundary.",
            ),
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_article_transformer_replacement_publication|",
            &publication,
        );
        Ok(publication)
    }

    /// Executes one request through the explicit executor-trace surface.
    pub fn execute(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<TassadarExecutorOutcome, TassadarExecutorServiceError> {
        self.validate_product(request)?;
        let requested_model_id = request
            .requested_model_id
            .as_deref()
            .unwrap_or(self.default_model_id.as_str());
        if self
            .resolve_custom_model_descriptor_by_model_id(Some(requested_model_id))
            .is_ok()
        {
            return self
                .execute_with_custom_transformer_backed_article_model(request, requested_model_id);
        }
        if is_transformer_backed_article_model_id(requested_model_id) {
            return self.execute_with_transformer_backed_article_model(request);
        }
        let fixture = self
            .resolve_fixture_by_model_id(Some(requested_model_id))
            .map_err(|model_id| TassadarExecutorServiceError::UnknownModel { model_id })?;
        Ok(self.execute_with_fixture(fixture, request))
    }

    /// Starts a pull-driven explicit executor-trace stream.
    pub fn execute_stream(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<LocalTassadarExecutorStream, TassadarExecutorServiceError> {
        self.validate_product(request)?;
        let requested_model_id = request
            .requested_model_id
            .as_deref()
            .unwrap_or(self.default_model_id.as_str());
        let runtime_capability = if self
            .resolve_custom_model_descriptor_by_model_id(Some(requested_model_id))
            .is_ok()
            || is_transformer_backed_article_model_id(requested_model_id)
        {
            TassadarRuntimeCapabilityReport::current()
        } else {
            self.resolve_fixture_by_model_id(Some(requested_model_id))
                .map_err(|model_id| TassadarExecutorServiceError::UnknownModel { model_id })?
                .runtime_capability_report()
        };
        let outcome = self.execute(request)?;
        Ok(LocalTassadarExecutorStream::from_events(
            stream_events_for_outcome(runtime_capability, outcome),
        ))
    }

    /// Resolves the explicit model and selection truth without executing the program.
    pub fn preflight(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<TassadarExecutorPreflightReport, TassadarExecutorServiceError> {
        self.validate_product(request)?;
        let requested_model_id = request
            .requested_model_id
            .as_deref()
            .unwrap_or(self.default_model_id.as_str());
        if self
            .resolve_custom_model_descriptor_by_model_id(Some(requested_model_id))
            .is_ok()
        {
            let descriptor = self
                .custom_model_descriptor_for_profile_id(
                    requested_model_id,
                    request.program_artifact.wasm_profile_id.as_str(),
                )
                .map_err(|detail| {
                    TassadarExecutorServiceError::InvalidArticleTransformerReplacement { detail }
                })?;
            return Ok(self.preflight_with_model_descriptor(
                request,
                descriptor,
                TassadarRuntimeCapabilityReport::current(),
            ));
        }
        if is_transformer_backed_article_model_id(requested_model_id) {
            let descriptor = transformer_backed_article_model_descriptor_for_profile_id(
                request.program_artifact.wasm_profile_id.as_str(),
            )
            .map_err(|detail| {
                TassadarExecutorServiceError::InvalidArticleTransformerReplacement { detail }
            })?;
            return Ok(self.preflight_with_model_descriptor(
                request,
                descriptor,
                TassadarRuntimeCapabilityReport::current(),
            ));
        }
        let fixture = self
            .resolve_fixture_by_model_id(Some(requested_model_id))
            .map_err(|model_id| TassadarExecutorServiceError::UnknownModel { model_id })?;
        Ok(self.preflight_with_model_descriptor(
            request,
            fixture.descriptor().clone(),
            fixture.runtime_capability_report(),
        ))
    }

    fn validate_product(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<(), TassadarExecutorServiceError> {
        if request.product_id == EXECUTOR_TRACE_PRODUCT_ID {
            Ok(())
        } else {
            Err(TassadarExecutorServiceError::UnsupportedProduct {
                product_id: request.product_id.clone(),
            })
        }
    }

    fn resolve_fixture_by_model_id(
        &self,
        requested_model_id: Option<&str>,
    ) -> Result<&TassadarExecutorFixture, String> {
        let requested_model_id = requested_model_id.unwrap_or(self.default_model_id.as_str());
        self.fixtures
            .get(requested_model_id)
            .ok_or_else(|| requested_model_id.to_string())
    }

    fn resolve_custom_model_descriptor_by_model_id(
        &self,
        requested_model_id: Option<&str>,
    ) -> Result<&TassadarExecutorModelDescriptor, String> {
        let requested_model_id = requested_model_id.unwrap_or(self.default_model_id.as_str());
        self.custom_model_descriptors
            .get(requested_model_id)
            .ok_or_else(|| requested_model_id.to_string())
    }

    fn execute_with_fixture(
        &self,
        fixture: &TassadarExecutorFixture,
        request: &TassadarExecutorRequest,
    ) -> TassadarExecutorOutcome {
        self.execute_with_model_descriptor(
            request,
            fixture.descriptor().clone(),
            fixture.runtime_capability_report(),
        )
    }

    fn execute_with_transformer_backed_article_model(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<TassadarExecutorOutcome, TassadarExecutorServiceError> {
        let descriptor = transformer_backed_article_model_descriptor_for_profile_id(
            request.program_artifact.wasm_profile_id.as_str(),
        )
        .map_err(|detail| {
            TassadarExecutorServiceError::InvalidArticleTransformerReplacement { detail }
        })?;
        Ok(self.execute_with_model_descriptor(
            request,
            descriptor,
            TassadarRuntimeCapabilityReport::current(),
        ))
    }

    fn custom_model_descriptor_for_profile_id(
        &self,
        model_id: &str,
        profile_id: &str,
    ) -> Result<TassadarExecutorModelDescriptor, String> {
        let mut descriptor = self
            .resolve_custom_model_descriptor_by_model_id(Some(model_id))?
            .clone();
        let fixture = transformer_backed_article_fixture_for_profile_id(profile_id)?;
        descriptor.compatibility = fixture.descriptor().compatibility.clone();
        descriptor.profile = fixture.descriptor().profile.clone();
        descriptor.trace_abi = fixture.descriptor().trace_abi.clone();
        Ok(descriptor)
    }

    fn execute_with_custom_transformer_backed_article_model(
        &self,
        request: &TassadarExecutorRequest,
        requested_model_id: &str,
    ) -> Result<TassadarExecutorOutcome, TassadarExecutorServiceError> {
        let descriptor = self
            .custom_model_descriptor_for_profile_id(
                requested_model_id,
                request.program_artifact.wasm_profile_id.as_str(),
            )
            .map_err(|detail| {
                TassadarExecutorServiceError::InvalidArticleTransformerReplacement { detail }
            })?;
        Ok(self.execute_with_model_descriptor(
            request,
            descriptor,
            TassadarRuntimeCapabilityReport::current(),
        ))
    }

    fn preflight_with_model_descriptor(
        &self,
        request: &TassadarExecutorRequest,
        descriptor: TassadarExecutorModelDescriptor,
        runtime_capability: TassadarRuntimeCapabilityReport,
    ) -> TassadarExecutorPreflightReport {
        let contract_error = descriptor
            .validate_program_artifact(&request.program_artifact, request.requested_decode_mode)
            .err();
        let selection = if contract_error.is_none() {
            Some(diagnose_tassadar_executor_request(
                &request.program_artifact.validated_program,
                request.requested_decode_mode,
                request.program_artifact.trace_abi_version,
                Some(descriptor.compatibility.supported_decode_modes.as_slice()),
            ))
        } else {
            None
        };
        TassadarExecutorPreflightReport {
            request_id: request.request_id.clone(),
            product_id: request.product_id.clone(),
            model_descriptor: descriptor,
            runtime_capability,
            contract_error,
            selection,
        }
    }

    fn execute_with_model_descriptor(
        &self,
        request: &TassadarExecutorRequest,
        descriptor: TassadarExecutorModelDescriptor,
        runtime_capability: TassadarRuntimeCapabilityReport,
    ) -> TassadarExecutorOutcome {
        match descriptor
            .validate_program_artifact(&request.program_artifact, request.requested_decode_mode)
        {
            Ok(()) => {}
            Err(contract_error) => {
                return TassadarExecutorOutcome::Refused {
                    refusal: TassadarExecutorRefusalResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        model_descriptor: descriptor,
                        runtime_capability,
                        detail: contract_error.to_string(),
                        contract_error: Some(contract_error),
                        selection: None,
                    },
                };
            }
        }

        let selection = diagnose_tassadar_executor_request(
            &request.program_artifact.validated_program,
            request.requested_decode_mode,
            request.program_artifact.trace_abi_version,
            Some(descriptor.compatibility.supported_decode_modes.as_slice()),
        );
        if selection.effective_decode_mode.is_none() {
            return TassadarExecutorOutcome::Refused {
                refusal: TassadarExecutorRefusalResponse {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    model_descriptor: descriptor,
                    runtime_capability,
                    detail: selection.detail.clone(),
                    contract_error: None,
                    selection: Some(selection),
                },
            };
        }

        match execute_tassadar_executor_request(
            &request.program_artifact.validated_program,
            request.requested_decode_mode,
            request.program_artifact.trace_abi_version,
            Some(descriptor.compatibility.supported_decode_modes.as_slice()),
        ) {
            Ok(execution_report) => {
                let evidence_bundle = build_tassadar_execution_evidence_bundle(
                    request.request_id.clone(),
                    request.stable_digest(),
                    request.product_id.clone(),
                    descriptor.model.model_id.clone(),
                    descriptor.stable_digest(),
                    request.environment_refs.clone(),
                    &request.program_artifact,
                    execution_report
                        .selection
                        .effective_decode_mode
                        .expect("completed execution should surface an effective decode mode"),
                    &execution_report.execution,
                );
                TassadarExecutorOutcome::Completed {
                    response: TassadarExecutorResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        model_descriptor: descriptor,
                        runtime_capability,
                        execution_report,
                        evidence_bundle,
                        environment_refs: request.environment_refs.clone(),
                    },
                }
            }
            Err(selection) => TassadarExecutorOutcome::Refused {
                refusal: TassadarExecutorRefusalResponse {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    model_descriptor: descriptor,
                    runtime_capability,
                    detail: selection.detail.clone(),
                    contract_error: None,
                    selection: Some(selection),
                },
            },
        }
    }
}

/// Canonical benchmark/workload identity for one article-class Tassadar case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleBenchmarkIdentity {
    /// Stable benchmark ref for the article-class suite.
    pub benchmark_ref: String,
    /// Stable benchmark environment ref for the article-class suite.
    pub benchmark_environment_ref: String,
    /// Canonical committed benchmark report anchor.
    pub benchmark_report_ref: String,
    /// Stable workload family label.
    pub workload_family: String,
    /// Stable case identifier inside the article-class suite.
    pub case_id: String,
    /// Short case summary.
    pub case_summary: String,
    /// Stable program identifier for the selected workload.
    pub program_id: String,
    /// Stable Wasm profile identifier for the selected workload.
    pub wasm_profile_id: String,
    /// Digest of the validated program selected for the session.
    pub validated_program_digest: String,
}

impl TassadarArticleBenchmarkIdentity {
    fn for_case(case: &TassadarValidationCase, artifact: &TassadarProgramArtifact) -> Self {
        Self {
            benchmark_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REF),
            benchmark_environment_ref: String::from(
                TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF,
            ),
            benchmark_report_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
            workload_family: String::from(article_workload_family(case.case_id.as_str())),
            case_id: case.case_id.clone(),
            case_summary: case.summary.clone(),
            program_id: case.program.program_id.clone(),
            wasm_profile_id: case.program.profile_id.clone(),
            validated_program_digest: artifact.validated_program_digest.clone(),
        }
    }
}

/// Proof and trace identity that survives the article-session serving boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleProofIdentity {
    /// Underlying exact executor product that realized the compute span.
    pub executor_product_id: String,
    /// Stable trace artifact identifier.
    pub trace_artifact_id: String,
    /// Stable trace artifact digest.
    pub trace_artifact_digest: String,
    /// Stable trace digest.
    pub trace_digest: String,
    /// Stable trace-proof artifact identifier.
    pub trace_proof_id: String,
    /// Stable trace-proof digest.
    pub trace_proof_digest: String,
    /// Stable validated-program artifact digest.
    pub program_artifact_digest: String,
    /// Stable runtime-manifest identity digest.
    pub runtime_manifest_identity_digest: String,
    /// Stable runtime-manifest digest.
    pub runtime_manifest_digest: String,
    /// Stable proof-bundle request digest.
    pub proof_bundle_request_digest: String,
    /// Stable proof-bundle model id when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_bundle_model_id: Option<String>,
}

impl TassadarArticleProofIdentity {
    fn from_response(response: &TassadarExecutorResponse) -> Self {
        Self {
            executor_product_id: response.evidence_bundle.proof_bundle.product_id.clone(),
            trace_artifact_id: response.evidence_bundle.trace_artifact.artifact_id.clone(),
            trace_artifact_digest: response
                .evidence_bundle
                .trace_artifact
                .artifact_digest
                .clone(),
            trace_digest: response.evidence_bundle.trace_artifact.trace_digest.clone(),
            trace_proof_id: response
                .evidence_bundle
                .trace_proof
                .proof_artifact_id
                .clone(),
            trace_proof_digest: response.evidence_bundle.trace_proof.proof_digest.clone(),
            program_artifact_digest: response
                .evidence_bundle
                .trace_proof
                .program_artifact_digest
                .clone(),
            runtime_manifest_identity_digest: response
                .evidence_bundle
                .runtime_manifest
                .identity_digest
                .clone(),
            runtime_manifest_digest: response
                .evidence_bundle
                .runtime_manifest
                .manifest_digest
                .clone(),
            proof_bundle_request_digest: response
                .evidence_bundle
                .proof_bundle
                .request_digest
                .clone(),
            proof_bundle_model_id: response.evidence_bundle.proof_bundle.model_id.clone(),
        }
    }
}

/// Derived readable-log view over one article-class executor session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleReadableLogExcerpt {
    /// Maximum retained line count for the derived view.
    pub max_lines: usize,
    /// Total line count before truncation.
    pub total_line_count: usize,
    /// Whether the derived view was truncated.
    pub truncated: bool,
    /// Derived readable-log lines.
    pub lines: Vec<String>,
    /// Plain-language posture describing how the view relates to machine truth.
    pub derivation_posture: String,
}

/// Derived symbolic token-trace view over one article-class executor session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTokenTraceExcerpt {
    /// Maximum retained token count for the derived view.
    pub max_tokens: usize,
    /// Total token count before truncation.
    pub total_token_count: usize,
    /// Prompt/target boundary inside the symbolic token stream.
    pub prompt_token_count: usize,
    /// Whether the derived view was truncated.
    pub truncated: bool,
    /// Stable tokenizer digest for the derived symbolic view.
    pub tokenizer_digest: String,
    /// Derived symbolic token strings.
    pub tokens: Vec<String>,
    /// Plain-language posture describing how the view relates to machine truth.
    pub derivation_posture: String,
}

/// Specialized request for one canonical article-class Tassadar session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleExecutorSessionRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier. Must be `psionic.article_executor_session`.
    pub product_id: String,
    /// Stable article-class case identifier.
    pub article_case_id: String,
    /// Optional explicit executor model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model_id: Option<String>,
    /// Requested decode mode for the exact executor path.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Ordered environment refs carried into lineage in addition to the canonical benchmark ref.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
    /// Whether the session must fail closed unless a direct model-weight proof receipt is emitted.
    #[serde(default)]
    pub require_direct_model_weight_proof: bool,
}

impl TassadarArticleExecutorSessionRequest {
    /// Creates a new article-class executor session request.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        article_case_id: impl Into<String>,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(ARTICLE_EXECUTOR_SESSION_PRODUCT_ID),
            article_case_id: article_case_id.into(),
            requested_model_id: None,
            requested_decode_mode,
            environment_refs: Vec::new(),
            require_direct_model_weight_proof: false,
        }
    }

    /// Pins execution to one explicit executor model.
    #[must_use]
    pub fn with_requested_model_id(mut self, requested_model_id: impl Into<String>) -> Self {
        self.requested_model_id = Some(requested_model_id.into());
        self
    }

    /// Carries extra environment refs into the served evidence bundle.
    #[must_use]
    pub fn with_environment_refs(mut self, mut environment_refs: Vec<String>) -> Self {
        environment_refs.sort();
        environment_refs.dedup();
        self.environment_refs = environment_refs;
        self
    }

    /// Requires a direct model-weight proof receipt on completion.
    #[must_use]
    pub fn require_direct_model_weight_proof(mut self) -> Self {
        self.require_direct_model_weight_proof = true;
        self
    }

    /// Returns a stable digest for the article-session request.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"tassadar_article_executor_session_request|", self)
    }
}

/// Typed refusal response for article-session requests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleExecutorSessionRefusalResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Canonical benchmark/workload identity when the case was resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_identity: Option<TassadarArticleBenchmarkIdentity>,
    /// Served executor model descriptor when one was resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_descriptor: Option<TassadarExecutorModelDescriptor>,
    /// Runtime capability report visible to the caller.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Contract error when model/program pairing failed before selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_error: Option<TassadarExecutorContractError>,
    /// Runtime selection diagnostic when one was available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<TassadarExecutorSelectionDiagnostic>,
    /// Human-readable refusal detail.
    pub detail: String,
}

/// Completed served article session carrying benchmark, proof, and derived views.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleExecutorSessionResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Canonical benchmark/workload identity.
    pub benchmark_identity: TassadarArticleBenchmarkIdentity,
    /// Proof identity preserved across the serving boundary.
    pub proof_identity: TassadarArticleProofIdentity,
    /// Underlying exact executor response.
    pub executor_response: TassadarExecutorResponse,
    /// Derived readable-log view over the canonical trace.
    pub readable_log: TassadarArticleReadableLogExcerpt,
    /// Derived symbolic token-trace view over the canonical trace.
    pub token_trace: TassadarArticleTokenTraceExcerpt,
    /// Direct model-weight proof receipt when the caller required it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_model_weight_execution_proof_receipt:
        Option<TassadarDirectModelWeightExecutionProofReceipt>,
}

impl TassadarArticleExecutorSessionResponse {
    /// Returns the final scalar outputs emitted by the execution.
    #[must_use]
    pub fn final_outputs(&self) -> &[i32] {
        self.executor_response.final_outputs()
    }
}

/// Served outcome for one article-session request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarArticleExecutorSessionOutcome {
    /// The article session completed through the Psionic-owned executor lane.
    Completed {
        /// Completed article-session response.
        response: TassadarArticleExecutorSessionResponse,
    },
    /// The article session refused the request explicitly.
    Refused {
        /// Typed refusal response.
        refusal: TassadarArticleExecutorSessionRefusalResponse,
    },
}

/// Benchmark identity event emitted by the article-session stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleBenchmarkIdentityEvent {
    /// Canonical article benchmark/workload identity.
    pub benchmark_identity: TassadarArticleBenchmarkIdentity,
}

/// Proof identity event emitted by the article-session stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleProofIdentityEvent {
    /// Preserved proof identity for the completed exact executor span.
    pub proof_identity: TassadarArticleProofIdentity,
}

/// Direct model-weight proof event emitted by the article-session stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDirectModelWeightExecutionProofEvent {
    /// Direct model-weight proof payload.
    pub proof_receipt: TassadarDirectModelWeightExecutionProofReceipt,
}

/// Readable-log event emitted by the article-session stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleReadableLogLineEvent {
    /// Zero-based readable-log line index within the derived excerpt.
    pub line_index: usize,
    /// One derived readable-log line.
    pub line: String,
}

/// Token-trace event emitted by the article-session stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTokenTraceChunkEvent {
    /// Zero-based chunk index.
    pub chunk_index: usize,
    /// Prompt/target boundary in the underlying symbolic token stream.
    pub prompt_token_count: usize,
    /// Total token count before truncation.
    pub total_token_count: usize,
    /// Whether the underlying symbolic view was truncated.
    pub truncated: bool,
    /// Ordered symbolic tokens in this chunk.
    pub tokens: Vec<String>,
}

/// Terminal event emitted by the article-session stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleExecutorSessionTerminalEvent {
    /// Final served outcome.
    pub outcome: TassadarArticleExecutorSessionOutcome,
}

/// Typed event emitted by the pull-driven article-session stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarArticleExecutorSessionStreamEvent {
    /// Canonical benchmark/workload identity for the session.
    BenchmarkIdentity {
        /// Benchmark identity payload.
        benchmark_identity: TassadarArticleBenchmarkIdentityEvent,
    },
    /// Runtime capability surfaced before execution.
    Capability {
        /// Runtime capability report visible to the caller.
        runtime_capability: TassadarRuntimeCapabilityReport,
    },
    /// Decode-selection diagnostic surfaced before derived trace views.
    Selection {
        /// Direct/fallback/refused selection diagnostic.
        selection: TassadarExecutorSelectionDiagnostic,
    },
    /// Proof identity surfaced for completed sessions.
    ProofIdentity {
        /// Proof identity payload.
        proof_identity: TassadarArticleProofIdentityEvent,
    },
    /// Direct model-weight proof surfaced for proof-required sessions.
    DirectModelWeightExecutionProof {
        /// Direct model-weight proof payload.
        direct_model_weight_execution_proof: TassadarArticleDirectModelWeightExecutionProofEvent,
    },
    /// One derived readable-log line.
    ReadableLogLine {
        /// Readable-log line payload.
        readable_log_line: TassadarArticleReadableLogLineEvent,
    },
    /// One symbolic token-trace chunk.
    TokenTraceChunk {
        /// Token-trace chunk payload.
        token_trace_chunk: TassadarArticleTokenTraceChunkEvent,
    },
    /// One emitted output value.
    Output {
        /// Output event payload.
        output: TassadarExecutorOutputEvent,
    },
    /// Terminal completion or refusal.
    Terminal {
        /// Terminal payload.
        terminal: TassadarArticleExecutorSessionTerminalEvent,
    },
}

/// Product-validation error for the article-session serving surface.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarArticleExecutorSessionServiceError {
    /// The request targeted a different served product family.
    #[error("unsupported Tassadar article-session product `{product_id}`")]
    UnsupportedProduct {
        /// Product identifier supplied by the caller.
        product_id: String,
    },
}

/// Pull-driven local stream for explicit article-session products.
#[derive(Clone, Debug, Default)]
pub struct LocalTassadarArticleExecutorSessionStream {
    events: VecDeque<TassadarArticleExecutorSessionStreamEvent>,
}

impl LocalTassadarArticleExecutorSessionStream {
    fn from_events(events: Vec<TassadarArticleExecutorSessionStreamEvent>) -> Self {
        Self {
            events: VecDeque::from(events),
        }
    }

    /// Returns the next typed stream event.
    pub fn next_event(&mut self) -> Option<TassadarArticleExecutorSessionStreamEvent> {
        self.events.pop_front()
    }
}

/// Local reference implementation of the article-workload session surface.
#[derive(Clone, Debug)]
pub struct LocalTassadarArticleExecutorSessionService {
    executor_service: LocalTassadarExecutorService,
}

impl Default for LocalTassadarArticleExecutorSessionService {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalTassadarArticleExecutorSessionService {
    /// Creates the default in-process article-session service.
    #[must_use]
    pub fn new() -> Self {
        Self {
            executor_service: LocalTassadarExecutorService::new()
                .with_fixture(TassadarExecutorFixture::article_i32_compute_v1()),
        }
    }

    /// Replaces the backing executor service.
    #[must_use]
    pub fn with_executor_service(mut self, executor_service: LocalTassadarExecutorService) -> Self {
        self.executor_service = executor_service;
        self
    }

    /// Executes one article-class session through the specialized serving surface.
    pub fn execute(
        &self,
        request: &TassadarArticleExecutorSessionRequest,
    ) -> Result<TassadarArticleExecutorSessionOutcome, TassadarArticleExecutorSessionServiceError>
    {
        self.execute_with_derived_views(request, true)
    }

    pub(crate) fn execute_without_derived_views(
        &self,
        request: &TassadarArticleExecutorSessionRequest,
    ) -> Result<TassadarArticleExecutorSessionOutcome, TassadarArticleExecutorSessionServiceError>
    {
        self.execute_with_derived_views(request, false)
    }

    fn execute_with_derived_views(
        &self,
        request: &TassadarArticleExecutorSessionRequest,
        include_derived_views: bool,
    ) -> Result<TassadarArticleExecutorSessionOutcome, TassadarArticleExecutorSessionServiceError>
    {
        self.validate_product(request)?;
        let runtime_capability = TassadarRuntimeCapabilityReport::current();
        let Some(case) = article_case_by_id(request.article_case_id.as_str()) else {
            return Ok(TassadarArticleExecutorSessionOutcome::Refused {
                refusal: TassadarArticleExecutorSessionRefusalResponse {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    benchmark_identity: None,
                    model_descriptor: None,
                    runtime_capability,
                    contract_error: None,
                    selection: None,
                    detail: format!(
                        "article workload `{}` is not present in the canonical Tassadar article corpus",
                        request.article_case_id
                    ),
                },
            });
        };

        let executor_request = self.executor_request_for(request, &case);
        let benchmark_identity =
            TassadarArticleBenchmarkIdentity::for_case(&case, &executor_request.program_artifact);

        match self.executor_service.execute(&executor_request) {
            Ok(TassadarExecutorOutcome::Completed { response }) => {
                let proof_identity = TassadarArticleProofIdentity::from_response(&response);
                let (readable_log, token_trace) = if include_derived_views {
                    (
                        build_article_readable_log_excerpt(
                            &case,
                            &response.execution_report.execution,
                        ),
                        build_article_token_trace_excerpt(
                            &case,
                            &response.execution_report.execution,
                        ),
                    )
                } else {
                    (
                        TassadarArticleReadableLogExcerpt {
                            max_lines: 0,
                            total_line_count: 0,
                            truncated: false,
                            lines: Vec::new(),
                            derivation_posture: String::from(
                                "derived readable-log view intentionally suppressed for proof-only article-session execution",
                            ),
                        },
                        TassadarArticleTokenTraceExcerpt {
                            max_tokens: 0,
                            total_token_count: 0,
                            prompt_token_count: 0,
                            truncated: false,
                            tokenizer_digest: String::new(),
                            tokens: Vec::new(),
                            derivation_posture: String::from(
                                "derived token-trace view intentionally suppressed for proof-only article-session execution",
                            ),
                        },
                    )
                };
                let mut response = TassadarArticleExecutorSessionResponse {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    benchmark_identity,
                    proof_identity,
                    executor_response: response,
                    readable_log,
                    token_trace,
                    direct_model_weight_execution_proof_receipt: None,
                };
                if request.require_direct_model_weight_proof {
                    match crate::build_tassadar_direct_model_weight_execution_proof_receipt_for_article_session(
                        &self.executor_service,
                        request,
                        &response,
                    ) {
                        Ok(receipt) => {
                            response.direct_model_weight_execution_proof_receipt = Some(receipt);
                        }
                        Err(error) => {
                            return Ok(TassadarArticleExecutorSessionOutcome::Refused {
                                refusal: TassadarArticleExecutorSessionRefusalResponse {
                                    request_id: request.request_id.clone(),
                                    product_id: request.product_id.clone(),
                                    benchmark_identity: Some(response.benchmark_identity.clone()),
                                    model_descriptor: Some(
                                        response.executor_response.model_descriptor.clone(),
                                    ),
                                    runtime_capability: response
                                        .executor_response
                                        .runtime_capability
                                        .clone(),
                                    contract_error: None,
                                    selection: Some(
                                        response.executor_response.execution_report.selection.clone(),
                                    ),
                                    detail: format!(
                                        "article session cannot publish direct model-weight execution proof: {error}"
                                    ),
                                },
                            });
                        }
                    }
                }
                Ok(TassadarArticleExecutorSessionOutcome::Completed { response })
            }
            Ok(TassadarExecutorOutcome::Refused { refusal }) => {
                Ok(TassadarArticleExecutorSessionOutcome::Refused {
                    refusal: TassadarArticleExecutorSessionRefusalResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        benchmark_identity: Some(benchmark_identity),
                        model_descriptor: Some(refusal.model_descriptor),
                        runtime_capability: refusal.runtime_capability,
                        contract_error: refusal.contract_error,
                        selection: refusal.selection,
                        detail: refusal.detail,
                    },
                })
            }
            Err(TassadarExecutorServiceError::UnknownModel { model_id }) => {
                Ok(TassadarArticleExecutorSessionOutcome::Refused {
                    refusal: TassadarArticleExecutorSessionRefusalResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        benchmark_identity: Some(benchmark_identity),
                        model_descriptor: None,
                        runtime_capability,
                        contract_error: None,
                        selection: None,
                        detail: format!("unknown Tassadar executor model `{model_id}`"),
                    },
                })
            }
            Err(TassadarExecutorServiceError::InvalidArticleTransformerReplacement { detail }) => {
                Ok(TassadarArticleExecutorSessionOutcome::Refused {
                    refusal: TassadarArticleExecutorSessionRefusalResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        benchmark_identity: Some(benchmark_identity),
                        model_descriptor: None,
                        runtime_capability,
                        contract_error: None,
                        selection: None,
                        detail: format!(
                            "trained Transformer-backed article route is not publishable yet: {detail}"
                        ),
                    },
                })
            }
            Err(TassadarExecutorServiceError::UnsupportedProduct { product_id }) => {
                Err(TassadarArticleExecutorSessionServiceError::UnsupportedProduct { product_id })
            }
            Err(TassadarExecutorServiceError::ExecutionRefusal(refusal)) => {
                Ok(TassadarArticleExecutorSessionOutcome::Refused {
                    refusal: TassadarArticleExecutorSessionRefusalResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        benchmark_identity: Some(benchmark_identity),
                        model_descriptor: None,
                        runtime_capability,
                        contract_error: None,
                        selection: None,
                        detail: refusal.to_string(),
                    },
                })
            }
        }
    }

    /// Starts a pull-driven article-session stream.
    pub fn execute_stream(
        &self,
        request: &TassadarArticleExecutorSessionRequest,
    ) -> Result<LocalTassadarArticleExecutorSessionStream, TassadarArticleExecutorSessionServiceError>
    {
        let outcome = self.execute(request)?;
        Ok(LocalTassadarArticleExecutorSessionStream::from_events(
            tassadar_article_executor_session_stream_events_for_outcome(&outcome),
        ))
    }

    fn validate_product(
        &self,
        request: &TassadarArticleExecutorSessionRequest,
    ) -> Result<(), TassadarArticleExecutorSessionServiceError> {
        if request.product_id == ARTICLE_EXECUTOR_SESSION_PRODUCT_ID {
            Ok(())
        } else {
            Err(
                TassadarArticleExecutorSessionServiceError::UnsupportedProduct {
                    product_id: request.product_id.clone(),
                },
            )
        }
    }

    fn executor_request_for(
        &self,
        request: &TassadarArticleExecutorSessionRequest,
        case: &TassadarValidationCase,
    ) -> TassadarExecutorRequest {
        let profile = tassadar_wasm_profile_for_id(case.program.profile_id.as_str())
            .expect("canonical article case should use a supported profile");
        let trace_abi = tassadar_trace_abi_for_profile_id(case.program.profile_id.as_str())
            .expect("canonical article case should use a supported trace ABI");
        let artifact = TassadarProgramArtifact::fixture_reference(
            format!("artifact://tassadar/article_session/{}", case.case_id),
            &profile,
            &trace_abi,
            case.program.clone(),
        )
        .expect("canonical article case should assemble into a program artifact");
        TassadarExecutorRequest::new(
            request.request_id.clone(),
            artifact,
            request.requested_decode_mode,
        )
        .with_requested_model_id(request.requested_model_id.clone().unwrap_or_else(|| {
            String::from(TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID)
        }))
        .with_environment_refs(merge_article_environment_refs(
            request.environment_refs.as_slice(),
        ))
    }
}

/// Planner-visible fallback behavior when executor routing is not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerFallbackPolicy {
    /// Refuse the planner request rather than silently degrading it.
    Refuse,
    /// Return a typed planner fallback summary while preserving executor truth.
    PlannerSummary,
}

/// Planner-visible budget for one exact-computation subproblem.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRoutingBudget {
    /// Maximum accepted validated-program length.
    pub max_program_len: usize,
    /// Maximum accepted trace-step budget inferred from the targeted profile.
    pub max_trace_steps: usize,
    /// Maximum accepted environment refs carried into lineage.
    pub max_environment_refs: usize,
}

impl TassadarPlannerRoutingBudget {
    /// Creates one explicit routing budget.
    #[must_use]
    pub fn new(
        max_program_len: usize,
        max_trace_steps: usize,
        max_environment_refs: usize,
    ) -> Self {
        Self {
            max_program_len,
            max_trace_steps,
            max_environment_refs,
        }
    }
}

impl Default for TassadarPlannerRoutingBudget {
    fn default() -> Self {
        Self::new(128, 512, 8)
    }
}

/// Planner-visible policy for exact executor delegation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRoutingPolicy {
    /// Whether exact executor delegation is enabled at all.
    pub allow_executor_delegation: bool,
    /// Whether planner routing may accept runtime decode fallback.
    pub allow_runtime_decode_fallback: bool,
    /// Whether routing requires the requested decode path to remain direct.
    pub require_direct_decode: bool,
    /// Whether proof-bearing executor evidence must remain present on success.
    pub require_proof_bundle: bool,
    /// Typed behavior when executor routing is skipped or refused.
    pub fallback_policy: TassadarPlannerFallbackPolicy,
}

impl TassadarPlannerRoutingPolicy {
    /// Returns the canonical truthful planner routing policy.
    #[must_use]
    pub fn exact_executor_default() -> Self {
        Self {
            allow_executor_delegation: true,
            allow_runtime_decode_fallback: true,
            require_direct_decode: false,
            require_proof_bundle: true,
            fallback_policy: TassadarPlannerFallbackPolicy::Refuse,
        }
    }

    /// Replaces the fallback behavior.
    #[must_use]
    pub fn with_fallback_policy(mut self, fallback_policy: TassadarPlannerFallbackPolicy) -> Self {
        self.fallback_policy = fallback_policy;
        self
    }
}

impl Default for TassadarPlannerRoutingPolicy {
    fn default() -> Self {
        Self::exact_executor_default()
    }
}

/// Planner-owned exact-computation subproblem routed into Tassadar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerExecutorSubproblem {
    /// Stable subproblem identifier.
    pub subproblem_id: String,
    /// Human-readable planner objective for the exact executor call.
    pub objective: String,
    /// Optional explicit executor model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model_id: Option<String>,
    /// Digest-bound program artifact submitted to the executor.
    pub program_artifact: TassadarProgramArtifact,
    /// Requested decode mode for the exact executor path.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Ordered environment refs carried into executor lineage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl TassadarPlannerExecutorSubproblem {
    /// Creates one exact-computation subproblem.
    #[must_use]
    pub fn new(
        subproblem_id: impl Into<String>,
        objective: impl Into<String>,
        program_artifact: TassadarProgramArtifact,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Self {
        Self {
            subproblem_id: subproblem_id.into(),
            objective: objective.into(),
            requested_model_id: None,
            program_artifact,
            requested_decode_mode,
            environment_refs: Vec::new(),
        }
    }

    /// Pins execution to one explicit executor model.
    #[must_use]
    pub fn with_requested_model_id(mut self, requested_model_id: impl Into<String>) -> Self {
        self.requested_model_id = Some(requested_model_id.into());
        self
    }

    /// Carries environment refs into the executor lineage.
    #[must_use]
    pub fn with_environment_refs(mut self, mut environment_refs: Vec<String>) -> Self {
        environment_refs.sort();
        environment_refs.dedup();
        self.environment_refs = environment_refs;
        self
    }
}

/// Stable planner-owned request for exact executor delegation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRoutingRequest {
    /// Stable planner request identifier.
    pub request_id: String,
    /// Product identifier. Must be `psionic.planner_executor_route`.
    pub product_id: String,
    /// Stable planner session identifier.
    pub planner_session_id: String,
    /// Planner model identifier making the exact-computation request.
    pub planner_model_id: String,
    /// Exact-computation subproblem to delegate.
    pub subproblem: TassadarPlannerExecutorSubproblem,
    /// Planner routing policy.
    pub routing_policy: TassadarPlannerRoutingPolicy,
    /// Planner routing budget.
    pub routing_budget: TassadarPlannerRoutingBudget,
}

impl TassadarPlannerRoutingRequest {
    /// Creates one planner-owned exact routing request.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        planner_session_id: impl Into<String>,
        planner_model_id: impl Into<String>,
        subproblem: TassadarPlannerExecutorSubproblem,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(PLANNER_EXECUTOR_ROUTE_PRODUCT_ID),
            planner_session_id: planner_session_id.into(),
            planner_model_id: planner_model_id.into(),
            subproblem,
            routing_policy: TassadarPlannerRoutingPolicy::default(),
            routing_budget: TassadarPlannerRoutingBudget::default(),
        }
    }

    /// Replaces the routing policy.
    #[must_use]
    pub fn with_routing_policy(mut self, routing_policy: TassadarPlannerRoutingPolicy) -> Self {
        self.routing_policy = routing_policy;
        self
    }

    /// Replaces the routing budget.
    #[must_use]
    pub fn with_routing_budget(mut self, routing_budget: TassadarPlannerRoutingBudget) -> Self {
        self.routing_budget = routing_budget;
        self
    }

    /// Returns a stable digest over the planner routing request.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"tassadar_planner_routing_request|", self)
    }
}

/// Planner-visible route state after policy and executor resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerRouteState {
    /// The request delegated successfully into Tassadar.
    Delegated,
    /// The planner received an explicit typed fallback.
    PlannerFallback,
    /// The routing contract refused the request.
    Refused,
}

/// Planner-visible reason for a route decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPlannerRouteReason {
    /// Planner policy disabled exact executor delegation.
    PlannerPolicyDisabled,
    /// Validated-program length exceeded planner budget.
    ProgramLengthBudgetExceeded,
    /// Profile-backed trace-step budget exceeded planner budget.
    TraceStepBudgetExceeded,
    /// Environment refs exceeded planner budget.
    EnvironmentRefBudgetExceeded,
    /// Planner policy disallowed runtime decode fallback.
    ExecutorDecodeFallbackDisallowed,
    /// Planner policy required a direct decode path.
    ExecutorDirectPathRequired,
    /// Executor model/program contract was invalid.
    ExecutorContractRejected,
    /// Executor selection refused the request before execution.
    ExecutorSelectionRefused,
    /// Executor service rejected the request before delegation.
    ExecutorServiceRejected,
}

/// Replay-stable planner routing decision with executor truth attached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRoutingDecision {
    /// Stable digest for the planner request.
    pub planner_request_digest: String,
    /// Stable digest for the routing decision itself.
    pub routing_digest: String,
    /// Stable planner request identifier.
    pub planner_request_id: String,
    /// Stable planner session identifier.
    pub planner_session_id: String,
    /// Planner model identifier that requested the route.
    pub planner_model_id: String,
    /// Planner-owned product identifier.
    pub planner_product_id: String,
    /// Executor-trace product delegated to by the router.
    pub executor_product_id: String,
    /// Stable subproblem identifier.
    pub subproblem_id: String,
    /// Requested decode mode.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Effective decode mode after runtime selection, when execution remained viable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Stable digest for the executor request when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_request_digest: Option<String>,
    /// Route state exposed back to the planner.
    pub route_state: TassadarPlannerRouteState,
    /// Route reason when routing did not delegate directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_reason: Option<TassadarPlannerRouteReason>,
    /// Budget policy consulted during route selection.
    pub routing_budget: TassadarPlannerRoutingBudget,
    /// Fallback and decode policy consulted during route selection.
    pub routing_policy: TassadarPlannerRoutingPolicy,
    /// Runtime capability report preserved across the planner boundary.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Executor selection diagnostic preserved across the planner boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<TassadarExecutorSelectionDiagnostic>,
    /// Contract error preserved when model/program pairing failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_error: Option<TassadarExecutorContractError>,
    /// Human-readable route summary safe for logs or UI.
    pub detail: String,
}

impl TassadarPlannerRoutingDecision {
    fn new(
        request: &TassadarPlannerRoutingRequest,
        executor_request_digest: Option<String>,
        runtime_capability: TassadarRuntimeCapabilityReport,
        selection: Option<TassadarExecutorSelectionDiagnostic>,
        contract_error: Option<TassadarExecutorContractError>,
        route_state: TassadarPlannerRouteState,
        route_reason: Option<TassadarPlannerRouteReason>,
        detail: String,
    ) -> Self {
        #[derive(Serialize)]
        struct RoutingDigestInput<'a> {
            planner_request_digest: &'a str,
            planner_request_id: &'a str,
            planner_session_id: &'a str,
            planner_model_id: &'a str,
            planner_product_id: &'a str,
            subproblem_id: &'a str,
            requested_decode_mode: TassadarExecutorDecodeMode,
            effective_decode_mode: Option<TassadarExecutorDecodeMode>,
            executor_request_digest: Option<&'a str>,
            route_state: TassadarPlannerRouteState,
            route_reason: Option<TassadarPlannerRouteReason>,
            routing_budget: &'a TassadarPlannerRoutingBudget,
            routing_policy: &'a TassadarPlannerRoutingPolicy,
            runtime_capability: &'a TassadarRuntimeCapabilityReport,
            selection: &'a Option<TassadarExecutorSelectionDiagnostic>,
            contract_error: &'a Option<TassadarExecutorContractError>,
            detail: &'a str,
        }

        let planner_request_digest = request.stable_digest();
        let effective_decode_mode = selection
            .as_ref()
            .and_then(|value| value.effective_decode_mode);
        let routing_digest = stable_digest(
            b"tassadar_planner_routing_decision|",
            &RoutingDigestInput {
                planner_request_digest: planner_request_digest.as_str(),
                planner_request_id: request.request_id.as_str(),
                planner_session_id: request.planner_session_id.as_str(),
                planner_model_id: request.planner_model_id.as_str(),
                planner_product_id: request.product_id.as_str(),
                subproblem_id: request.subproblem.subproblem_id.as_str(),
                requested_decode_mode: request.subproblem.requested_decode_mode,
                effective_decode_mode,
                executor_request_digest: executor_request_digest.as_deref(),
                route_state,
                route_reason,
                routing_budget: &request.routing_budget,
                routing_policy: &request.routing_policy,
                runtime_capability: &runtime_capability,
                selection: &selection,
                contract_error: &contract_error,
                detail: detail.as_str(),
            },
        );
        Self {
            planner_request_digest,
            routing_digest,
            planner_request_id: request.request_id.clone(),
            planner_session_id: request.planner_session_id.clone(),
            planner_model_id: request.planner_model_id.clone(),
            planner_product_id: request.product_id.clone(),
            executor_product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
            subproblem_id: request.subproblem.subproblem_id.clone(),
            requested_decode_mode: request.subproblem.requested_decode_mode,
            effective_decode_mode,
            executor_request_digest,
            route_state,
            route_reason,
            routing_budget: request.routing_budget.clone(),
            routing_policy: request.routing_policy.clone(),
            runtime_capability,
            selection,
            contract_error,
            detail,
        }
    }
}

/// Planner-visible successful exact delegation result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerCompletedResponse {
    /// Replay-stable routing decision.
    pub routing_decision: TassadarPlannerRoutingDecision,
    /// Completed exact executor response with proof-bearing evidence.
    pub executor_response: TassadarExecutorResponse,
}

/// Planner-visible typed fallback preserving executor truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerFallbackResponse {
    /// Replay-stable routing decision.
    pub routing_decision: TassadarPlannerRoutingDecision,
    /// Human-readable planner fallback summary.
    pub fallback_summary: String,
    /// Executor refusal preserved when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_refusal: Option<TassadarExecutorRefusalResponse>,
}

/// Planner-visible typed refusal preserving executor truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPlannerRefusalResponse {
    /// Replay-stable routing decision.
    pub routing_decision: TassadarPlannerRoutingDecision,
    /// Human-readable refusal detail.
    pub detail: String,
    /// Executor refusal preserved when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_refusal: Option<TassadarExecutorRefusalResponse>,
}

/// Planner-visible outcome for one exact executor routing request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarPlannerRoutingOutcome {
    /// The planner delegated successfully into Tassadar.
    Completed {
        /// Completed exact routing response.
        response: TassadarPlannerCompletedResponse,
    },
    /// The planner received an explicit typed fallback instead of exact execution.
    Fallback {
        /// Typed fallback response.
        fallback: TassadarPlannerFallbackResponse,
    },
    /// The routing contract refused the request.
    Refused {
        /// Typed refusal response.
        refusal: TassadarPlannerRefusalResponse,
    },
}

/// Planner-owned request validation errors for hybrid exact routing.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarPlannerRouterError {
    /// The request targeted a different planner-owned product family.
    #[error("unsupported Tassadar planner routing product `{product_id}`")]
    UnsupportedProduct {
        /// Product identifier supplied by the caller.
        product_id: String,
    },
}

/// Capability-descriptor publication failure for the planner / executor route surface.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarPlannerRouteDescriptorError {
    /// Publishing the benchmark-gated executor capability failed.
    #[error("failed to publish Tassadar planner route descriptor: {detail}")]
    CapabilityPublication {
        /// Plain-text publication detail.
        detail: String,
    },
}

/// Serve-side refusal when a research lane tries to bypass the public promotion checklist.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarResearchPromotionError {
    /// Building the current promotion-policy report failed.
    #[error("failed to build Tassadar promotion policy report: {detail}")]
    Build {
        /// Plain-text build detail.
        detail: String,
    },
    /// The research lane remains blocked on one or more required promotion gates.
    #[error("Tassadar promotion policy `{policy_id}` blocks served publication: {detail}")]
    PromotionBlocked {
        /// Stable promotion-policy identifier.
        policy_id: String,
        /// Gate kinds that still block publication.
        failed_gates: Vec<TassadarPromotionChecklistGateKind>,
        /// Plain-text refusal detail.
        detail: String,
    },
}

/// Returns the current promotion-policy report only when the research lane is promotable.
pub fn require_tassadar_research_lane_promotion_ready()
-> Result<TassadarPromotionPolicyReport, TassadarResearchPromotionError> {
    let report = build_tassadar_promotion_policy_report().map_err(|error| {
        TassadarResearchPromotionError::Build {
            detail: error.to_string(),
        }
    })?;
    if report.status.allows_served_publication() {
        Ok(report)
    } else {
        let failed_gates = report.failed_gates();
        Err(TassadarResearchPromotionError::PromotionBlocked {
            policy_id: report.policy_id.clone(),
            detail: format!(
                "candidate model `{}` remains `{}` because gates {:?} are still unsatisfied",
                report.candidate_model_id,
                serde_json::to_string(&report.status)
                    .unwrap_or_else(|_| String::from("\"unknown_status\"")),
                failed_gates
            ),
            failed_gates,
        })
    }
}

/// Local planner router that delegates exact subproblems into Tassadar.
#[derive(Clone, Debug, Default)]
pub struct LocalTassadarPlannerRouter {
    executor_service: LocalTassadarExecutorService,
}

impl LocalTassadarPlannerRouter {
    /// Creates the default local planner router.
    #[must_use]
    pub fn new() -> Self {
        Self {
            executor_service: LocalTassadarExecutorService::new(),
        }
    }

    /// Replaces the backing local executor service.
    #[must_use]
    pub fn with_executor_service(mut self, executor_service: LocalTassadarExecutorService) -> Self {
        self.executor_service = executor_service;
        self
    }

    /// Publishes one routeable served capability descriptor for planner / executor negotiation.
    pub fn route_capability_descriptor(
        &self,
        requested_model_id: Option<&str>,
    ) -> Result<TassadarPlannerExecutorRouteDescriptor, TassadarPlannerRouteDescriptorError> {
        if let Ok(model_descriptor) = self
            .executor_service
            .resolve_custom_model_descriptor_by_model_id(requested_model_id)
        {
            return custom_transformer_backed_route_capability_descriptor(model_descriptor);
        }
        let publication = self
            .executor_service
            .capability_publication(requested_model_id)
            .map_err(
                |error| TassadarPlannerRouteDescriptorError::CapabilityPublication {
                    detail: error.to_string(),
                },
            )?;
        let model_id = publication.model_descriptor.model.model_id.clone();
        let wasm_capability_matrix = routeable_wasm_capability_matrix(&publication);
        let benchmark_report_ref =
            route_benchmark_report_ref(&publication, &wasm_capability_matrix);
        let decode_capabilities = aggregate_route_decode_capabilities(
            &publication,
            &wasm_capability_matrix,
            &benchmark_report_ref,
        );
        Ok(TassadarPlannerExecutorRouteDescriptor::new(
            format!("tassadar.planner_executor_route.{model_id}.v0"),
            model_id,
            benchmark_report_ref,
            publication
                .internal_compute_profile_claim_check
                .claim
                .profile_id
                .clone(),
            publication
                .internal_compute_profile_claim_check
                .claim_digest
                .clone(),
            publication.workload_capability_matrix.matrix_digest.clone(),
            wasm_capability_matrix,
            decode_capabilities,
            vec![
                TassadarPlannerExecutorRouteRefusalReason::UnsupportedProduct,
                TassadarPlannerExecutorRouteRefusalReason::UnknownModel,
                TassadarPlannerExecutorRouteRefusalReason::ProviderNotReady,
                TassadarPlannerExecutorRouteRefusalReason::BenchmarkGateMissing,
                TassadarPlannerExecutorRouteRefusalReason::DecodeModeUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::RuntimeFallbackDisallowed,
                TassadarPlannerExecutorRouteRefusalReason::DirectDecodeRequired,
                TassadarPlannerExecutorRouteRefusalReason::WasmModuleClassUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::WasmOpcodeFamilyUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported,
            ],
            "benchmark-gated served planner / executor route descriptor above the current explicit executor-trace lane, with routeable Wasm module-class boundaries kept separate from the coarse lane-wide decode summary",
        ))
    }

    /// Routes one planner-owned exact subproblem into Tassadar when policy allows.
    pub fn route(
        &self,
        request: &TassadarPlannerRoutingRequest,
    ) -> Result<TassadarPlannerRoutingOutcome, TassadarPlannerRouterError> {
        self.validate_product(request)?;
        let executor_request = self.executor_request_for(request);
        let executor_request_digest = executor_request.stable_digest();
        let preflight = match self.executor_service.preflight(&executor_request) {
            Ok(preflight) => preflight,
            Err(TassadarExecutorServiceError::UnknownModel { model_id }) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail =
                    format!("planner requested unknown Tassadar executor model `{model_id}`");
                return Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ));
            }
            Err(TassadarExecutorServiceError::UnsupportedProduct { product_id }) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail = format!(
                    "planner delegated to unsupported Tassadar executor product `{product_id}`"
                );
                return Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ));
            }
            Err(TassadarExecutorServiceError::ExecutionRefusal(refusal)) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail = format!(
                    "executor service rejected planner request before delegation: {refusal}"
                );
                return Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ));
            }
            Err(TassadarExecutorServiceError::InvalidArticleTransformerReplacement { detail }) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                return Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    format!(
                        "trained Transformer-backed article route is not publishable yet: {detail}"
                    ),
                    None,
                ));
            }
        };

        if !request.routing_policy.allow_executor_delegation {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                preflight.selection,
                preflight.contract_error,
                TassadarPlannerRouteReason::PlannerPolicyDisabled,
                String::from("planner policy disabled exact Tassadar delegation"),
                None,
            ));
        }

        let program_len = request
            .subproblem
            .program_artifact
            .validated_program
            .instructions
            .len();
        if program_len > request.routing_budget.max_program_len {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                preflight.selection,
                preflight.contract_error,
                TassadarPlannerRouteReason::ProgramLengthBudgetExceeded,
                format!(
                    "validated program uses {} instructions which exceeds planner budget {}",
                    program_len, request.routing_budget.max_program_len
                ),
                None,
            ));
        }

        let conservative_trace_steps =
            route_trace_step_budget(&request.subproblem.program_artifact);
        if conservative_trace_steps > request.routing_budget.max_trace_steps {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                preflight.selection,
                preflight.contract_error,
                TassadarPlannerRouteReason::TraceStepBudgetExceeded,
                format!(
                    "profile-backed trace-step budget {} exceeds planner budget {}",
                    conservative_trace_steps, request.routing_budget.max_trace_steps
                ),
                None,
            ));
        }

        if request.subproblem.environment_refs.len() > request.routing_budget.max_environment_refs {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                preflight.selection,
                preflight.contract_error,
                TassadarPlannerRouteReason::EnvironmentRefBudgetExceeded,
                format!(
                    "environment ref count {} exceeds planner budget {}",
                    request.subproblem.environment_refs.len(),
                    request.routing_budget.max_environment_refs
                ),
                None,
            ));
        }

        if let Some(contract_error) = preflight.contract_error {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                preflight.selection,
                Some(contract_error.clone()),
                TassadarPlannerRouteReason::ExecutorContractRejected,
                contract_error.to_string(),
                None,
            ));
        }

        let selection = preflight
            .selection
            .expect("preflight should include selection when contract validation succeeds");
        if selection.effective_decode_mode.is_none() {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                Some(selection.clone()),
                None,
                TassadarPlannerRouteReason::ExecutorSelectionRefused,
                selection.detail.clone(),
                None,
            ));
        }
        if !request.routing_policy.allow_runtime_decode_fallback && selection.is_fallback() {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                Some(selection.clone()),
                None,
                TassadarPlannerRouteReason::ExecutorDecodeFallbackDisallowed,
                format!(
                    "planner policy disallowed runtime decode fallback: {}",
                    selection.detail
                ),
                None,
            ));
        }
        if request.routing_policy.require_direct_decode && selection.is_fallback() {
            return Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                preflight.runtime_capability,
                Some(selection.clone()),
                None,
                TassadarPlannerRouteReason::ExecutorDirectPathRequired,
                format!(
                    "planner policy required a direct decode path: {}",
                    selection.detail
                ),
                None,
            ));
        }

        match self.executor_service.execute(&executor_request) {
            Ok(TassadarExecutorOutcome::Completed { response }) => {
                if request.routing_policy.require_proof_bundle
                    && response.evidence_bundle.proof_bundle.product_id != EXECUTOR_TRACE_PRODUCT_ID
                {
                    let capability = response.runtime_capability.clone();
                    let selection = Some(response.execution_report.selection.clone());
                    return Ok(self.policy_terminal_outcome(
                        request,
                        Some(executor_request_digest),
                        capability,
                        selection,
                        None,
                        TassadarPlannerRouteReason::ExecutorServiceRejected,
                        String::from(
                            "completed executor response was missing the required proof-bearing product identity",
                        ),
                        None,
                    ));
                }
                let detail = format!(
                    "planner delegated exact subproblem `{}` into Tassadar via `{}`",
                    request.subproblem.subproblem_id, EXECUTOR_TRACE_PRODUCT_ID
                );
                let routing_decision = TassadarPlannerRoutingDecision::new(
                    request,
                    Some(executor_request_digest),
                    response.runtime_capability.clone(),
                    Some(response.execution_report.selection.clone()),
                    None,
                    TassadarPlannerRouteState::Delegated,
                    None,
                    detail,
                );
                Ok(TassadarPlannerRoutingOutcome::Completed {
                    response: TassadarPlannerCompletedResponse {
                        routing_decision,
                        executor_response: response,
                    },
                })
            }
            Ok(TassadarExecutorOutcome::Refused { refusal }) => Ok(self.policy_terminal_outcome(
                request,
                Some(executor_request_digest),
                refusal.runtime_capability.clone(),
                refusal.selection.clone(),
                refusal.contract_error.clone(),
                TassadarPlannerRouteReason::ExecutorSelectionRefused,
                refusal.detail.clone(),
                Some(refusal),
            )),
            Err(TassadarExecutorServiceError::UnknownModel { model_id }) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail =
                    format!("planner requested unknown Tassadar executor model `{model_id}`");
                Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ))
            }
            Err(TassadarExecutorServiceError::UnsupportedProduct { product_id }) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail = format!(
                    "planner delegated to unsupported Tassadar executor product `{product_id}`"
                );
                Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ))
            }
            Err(TassadarExecutorServiceError::ExecutionRefusal(refusal)) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                let detail = format!(
                    "executor service rejected planner request before delegation: {refusal}"
                );
                Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    detail,
                    None,
                ))
            }
            Err(TassadarExecutorServiceError::InvalidArticleTransformerReplacement { detail }) => {
                let capability = TassadarRuntimeCapabilityReport::current();
                Ok(self.policy_terminal_outcome(
                    request,
                    Some(executor_request_digest),
                    capability,
                    None,
                    None,
                    TassadarPlannerRouteReason::ExecutorServiceRejected,
                    format!(
                        "trained Transformer-backed article route is not publishable yet: {detail}"
                    ),
                    None,
                ))
            }
        }
    }

    fn validate_product(
        &self,
        request: &TassadarPlannerRoutingRequest,
    ) -> Result<(), TassadarPlannerRouterError> {
        if request.product_id == PLANNER_EXECUTOR_ROUTE_PRODUCT_ID {
            Ok(())
        } else {
            Err(TassadarPlannerRouterError::UnsupportedProduct {
                product_id: request.product_id.clone(),
            })
        }
    }

    fn executor_request_for(
        &self,
        request: &TassadarPlannerRoutingRequest,
    ) -> TassadarExecutorRequest {
        let mut executor_request = TassadarExecutorRequest::new(
            format!(
                "{}::{}",
                request.request_id, request.subproblem.subproblem_id
            ),
            request.subproblem.program_artifact.clone(),
            request.subproblem.requested_decode_mode,
        )
        .with_environment_refs(request.subproblem.environment_refs.clone());
        if let Some(requested_model_id) = request.subproblem.requested_model_id.as_deref() {
            executor_request = executor_request.with_requested_model_id(requested_model_id);
        }
        executor_request
    }

    fn policy_terminal_outcome(
        &self,
        request: &TassadarPlannerRoutingRequest,
        executor_request_digest: Option<String>,
        runtime_capability: TassadarRuntimeCapabilityReport,
        selection: Option<TassadarExecutorSelectionDiagnostic>,
        contract_error: Option<TassadarExecutorContractError>,
        route_reason: TassadarPlannerRouteReason,
        detail: String,
        executor_refusal: Option<TassadarExecutorRefusalResponse>,
    ) -> TassadarPlannerRoutingOutcome {
        let route_state = match request.routing_policy.fallback_policy {
            TassadarPlannerFallbackPolicy::Refuse => TassadarPlannerRouteState::Refused,
            TassadarPlannerFallbackPolicy::PlannerSummary => {
                TassadarPlannerRouteState::PlannerFallback
            }
        };
        let routing_decision = TassadarPlannerRoutingDecision::new(
            request,
            executor_request_digest,
            runtime_capability,
            selection,
            contract_error,
            route_state,
            Some(route_reason),
            detail.clone(),
        );
        match request.routing_policy.fallback_policy {
            TassadarPlannerFallbackPolicy::Refuse => TassadarPlannerRoutingOutcome::Refused {
                refusal: TassadarPlannerRefusalResponse {
                    routing_decision,
                    detail,
                    executor_refusal,
                },
            },
            TassadarPlannerFallbackPolicy::PlannerSummary => {
                TassadarPlannerRoutingOutcome::Fallback {
                    fallback: TassadarPlannerFallbackResponse {
                        routing_decision,
                        fallback_summary: detail,
                        executor_refusal,
                    },
                }
            }
        }
    }
}

fn routeable_wasm_capability_matrix(
    publication: &TassadarExecutorCapabilityPublication,
) -> TassadarPlannerExecutorWasmCapabilityMatrix {
    routeable_wasm_capability_matrix_from_parts(
        &publication.model_descriptor,
        &publication.workload_capability_matrix,
        &publication.module_execution_capability,
    )
}

fn routeable_wasm_capability_matrix_from_parts(
    model_descriptor: &TassadarExecutorModelDescriptor,
    workload_capability_matrix: &TassadarWorkloadCapabilityMatrix,
    module_execution_capability: &TassadarModuleExecutionCapabilityPublication,
) -> TassadarPlannerExecutorWasmCapabilityMatrix {
    let model_id = model_descriptor.model.model_id.clone();
    let rows = workload_capability_matrix
        .rows
        .iter()
        .map(routeable_wasm_capability_row)
        .collect::<Vec<_>>();
    TassadarPlannerExecutorWasmCapabilityMatrix::new(
        format!("tassadar.planner_executor_route.wasm_capability.{model_id}.v0"),
        model_id,
        rows,
        format!(
            "{} Route negotiation currently publishes only `{}` module classes above the served benchmark rows; {}",
            workload_capability_matrix.claim_boundary,
            TassadarPlannerExecutorWasmImportPosture::NoImportsOnly.as_str(),
            module_execution_capability.claim_boundary
        ),
    )
}

fn routeable_wasm_capability_row(
    workload_row: &TassadarWorkloadCapabilityRow,
) -> TassadarPlannerExecutorWasmCapabilityRow {
    let benchmark_report_ref = workload_row
        .benchmark_gate
        .as_ref()
        .map(|gate| gate.evidence_ref.clone());
    match workload_row.support_posture {
        TassadarWorkloadSupportPosture::Exact => TassadarPlannerExecutorWasmCapabilityRow::new(
            workload_row.workload_class,
            workload_row.supported_decode_modes.clone(),
            workload_row.supported_decode_modes.clone(),
            None,
            wasm_opcode_families_for_workload_class(workload_row.workload_class),
            TassadarPlannerExecutorWasmImportPosture::NoImportsOnly,
            benchmark_report_ref,
            vec![
                TassadarPlannerExecutorRouteRefusalReason::DecodeModeUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::WasmOpcodeFamilyUnsupported,
                TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported,
            ],
            workload_row.detail.clone(),
        ),
        TassadarWorkloadSupportPosture::ExactFallbackOnly => {
            TassadarPlannerExecutorWasmCapabilityRow::new(
                workload_row.workload_class,
                workload_row.supported_decode_modes.clone(),
                workload_row
                    .exact_fallback_decode_mode
                    .into_iter()
                    .collect::<Vec<_>>(),
                workload_row.exact_fallback_decode_mode,
                wasm_opcode_families_for_workload_class(workload_row.workload_class),
                TassadarPlannerExecutorWasmImportPosture::NoImportsOnly,
                benchmark_report_ref,
                vec![
                    TassadarPlannerExecutorRouteRefusalReason::DecodeModeUnsupported,
                    TassadarPlannerExecutorRouteRefusalReason::RuntimeFallbackDisallowed,
                    TassadarPlannerExecutorRouteRefusalReason::DirectDecodeRequired,
                    TassadarPlannerExecutorRouteRefusalReason::WasmOpcodeFamilyUnsupported,
                    TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported,
                ],
                workload_row.detail.clone(),
            )
        }
        TassadarWorkloadSupportPosture::Partial
        | TassadarWorkloadSupportPosture::ResearchOnly
        | TassadarWorkloadSupportPosture::Unsupported => {
            TassadarPlannerExecutorWasmCapabilityRow::new(
                workload_row.workload_class,
                Vec::new(),
                Vec::new(),
                None,
                wasm_opcode_families_for_workload_class(workload_row.workload_class),
                TassadarPlannerExecutorWasmImportPosture::NoImportsOnly,
                None,
                vec![TassadarPlannerExecutorRouteRefusalReason::WasmModuleClassUnsupported],
                workload_row.detail.clone(),
            )
        }
    }
}

fn wasm_opcode_families_for_workload_class(
    workload_class: psionic_models::TassadarWorkloadClass,
) -> Vec<TassadarPlannerExecutorWasmOpcodeFamily> {
    use psionic_models::TassadarWorkloadClass::{
        ArithmeticMicroprogram, BranchControlFlowMicroprogram, BranchHeavyKernel, ClrsShortestPath,
        HungarianMatching, LongLoopKernel, MemoryHeavyKernel, MemoryLookupMicroprogram,
        MicroWasmKernel, SudokuClass,
    };

    match workload_class {
        ArithmeticMicroprogram => vec![TassadarPlannerExecutorWasmOpcodeFamily::CoreI32Arithmetic],
        ClrsShortestPath
        | BranchControlFlowMicroprogram
        | BranchHeavyKernel
        | LongLoopKernel
        | SudokuClass => vec![
            TassadarPlannerExecutorWasmOpcodeFamily::CoreI32Arithmetic,
            TassadarPlannerExecutorWasmOpcodeFamily::StructuredControl,
        ],
        MemoryLookupMicroprogram => vec![
            TassadarPlannerExecutorWasmOpcodeFamily::CoreI32Arithmetic,
            TassadarPlannerExecutorWasmOpcodeFamily::LinearMemoryV2,
        ],
        MicroWasmKernel => vec![
            TassadarPlannerExecutorWasmOpcodeFamily::CoreI32Arithmetic,
            TassadarPlannerExecutorWasmOpcodeFamily::StructuredControl,
            TassadarPlannerExecutorWasmOpcodeFamily::LinearMemoryV2,
            TassadarPlannerExecutorWasmOpcodeFamily::DirectCallFrames,
        ],
        MemoryHeavyKernel | HungarianMatching => vec![
            TassadarPlannerExecutorWasmOpcodeFamily::CoreI32Arithmetic,
            TassadarPlannerExecutorWasmOpcodeFamily::StructuredControl,
            TassadarPlannerExecutorWasmOpcodeFamily::LinearMemoryV2,
        ],
    }
}

fn route_benchmark_report_ref(
    publication: &TassadarExecutorCapabilityPublication,
    wasm_capability_matrix: &TassadarPlannerExecutorWasmCapabilityMatrix,
) -> String {
    route_benchmark_report_ref_from_workload_capability_matrix(
        &publication.workload_capability_matrix,
        wasm_capability_matrix,
    )
}

fn route_benchmark_report_ref_from_workload_capability_matrix(
    workload_capability_matrix: &TassadarWorkloadCapabilityMatrix,
    wasm_capability_matrix: &TassadarPlannerExecutorWasmCapabilityMatrix,
) -> String {
    wasm_capability_matrix
        .rows
        .iter()
        .find_map(|row| row.benchmark_report_ref.clone())
        .unwrap_or_else(|| {
            workload_capability_matrix
                .rows
                .iter()
                .find_map(|row| {
                    row.benchmark_gate
                        .as_ref()
                        .map(|gate| gate.evidence_ref.clone())
                })
                .unwrap_or_default()
        })
}

fn aggregate_route_decode_capabilities(
    publication: &TassadarExecutorCapabilityPublication,
    wasm_capability_matrix: &TassadarPlannerExecutorWasmCapabilityMatrix,
    benchmark_report_ref: &str,
) -> Vec<TassadarPlannerExecutorDecodeCapability> {
    aggregate_route_decode_capabilities_from_model_descriptor(
        &publication.model_descriptor,
        wasm_capability_matrix,
        benchmark_report_ref,
    )
}

fn aggregate_route_decode_capabilities_from_model_descriptor(
    model_descriptor: &TassadarExecutorModelDescriptor,
    wasm_capability_matrix: &TassadarPlannerExecutorWasmCapabilityMatrix,
    benchmark_report_ref: &str,
) -> Vec<TassadarPlannerExecutorDecodeCapability> {
    model_descriptor
        .compatibility
        .supported_decode_modes
        .iter()
        .filter_map(|mode| {
            let supporting_rows = wasm_capability_matrix
                .rows
                .iter()
                .filter(|row| row.supported_decode_modes.contains(mode))
                .collect::<Vec<_>>();
            if supporting_rows.is_empty() {
                return None;
            }
            let route_posture = if supporting_rows
                .iter()
                .all(|row| row.direct_decode_modes.contains(mode))
            {
                TassadarPlannerExecutorRoutePosture::DirectGuaranteed
            } else {
                TassadarPlannerExecutorRoutePosture::FallbackCapable
            };
            Some(TassadarPlannerExecutorDecodeCapability {
                requested_decode_mode: *mode,
                route_posture,
                benchmark_report_ref: String::from(benchmark_report_ref),
                note: aggregate_route_decode_note(*mode, route_posture),
            })
        })
        .collect()
}

fn custom_transformer_backed_route_capability_descriptor(
    model_descriptor: &TassadarExecutorModelDescriptor,
) -> Result<TassadarPlannerExecutorRouteDescriptor, TassadarPlannerRouteDescriptorError> {
    let workload_capability_matrix =
        transformer_backed_article_workload_capability_matrix_for_descriptor(model_descriptor);
    workload_capability_matrix
        .validate_publication()
        .map_err(
            |error| TassadarPlannerRouteDescriptorError::CapabilityPublication {
                detail: format!("invalid custom workload capability publication: {error}"),
            },
        )?;
    let internal_compute_profile_ladder = tassadar_internal_compute_profile_ladder_publication();
    let internal_compute_profile_claim_check = check_tassadar_internal_compute_profile_claim(
        &internal_compute_profile_ladder,
        tassadar_current_served_internal_compute_profile_claim(),
    );
    if !internal_compute_profile_claim_check.green {
        return Err(TassadarPlannerRouteDescriptorError::CapabilityPublication {
            detail: internal_compute_profile_claim_check.detail.clone(),
        });
    }
    let module_execution_capability =
        transformer_backed_article_fixture().module_execution_capability_publication();
    let wasm_capability_matrix = routeable_wasm_capability_matrix_from_parts(
        model_descriptor,
        &workload_capability_matrix,
        &module_execution_capability,
    );
    let benchmark_report_ref = route_benchmark_report_ref_from_workload_capability_matrix(
        &workload_capability_matrix,
        &wasm_capability_matrix,
    );
    let decode_capabilities = aggregate_route_decode_capabilities_from_model_descriptor(
        model_descriptor,
        &wasm_capability_matrix,
        &benchmark_report_ref,
    );
    Ok(TassadarPlannerExecutorRouteDescriptor::new(
        format!(
            "tassadar.planner_executor_route.{}.v0",
            model_descriptor.model.model_id
        ),
        model_descriptor.model.model_id.clone(),
        benchmark_report_ref,
        internal_compute_profile_claim_check
            .claim
            .profile_id
            .clone(),
        internal_compute_profile_claim_check.claim_digest.clone(),
        workload_capability_matrix.matrix_digest.clone(),
        wasm_capability_matrix,
        decode_capabilities,
        vec![
            TassadarPlannerExecutorRouteRefusalReason::UnsupportedProduct,
            TassadarPlannerExecutorRouteRefusalReason::UnknownModel,
            TassadarPlannerExecutorRouteRefusalReason::ProviderNotReady,
            TassadarPlannerExecutorRouteRefusalReason::BenchmarkGateMissing,
            TassadarPlannerExecutorRouteRefusalReason::DecodeModeUnsupported,
            TassadarPlannerExecutorRouteRefusalReason::RuntimeFallbackDisallowed,
            TassadarPlannerExecutorRouteRefusalReason::DirectDecodeRequired,
            TassadarPlannerExecutorRouteRefusalReason::WasmModuleClassUnsupported,
            TassadarPlannerExecutorRouteRefusalReason::WasmOpcodeFamilyUnsupported,
            TassadarPlannerExecutorRouteRefusalReason::WasmImportPostureUnsupported,
        ],
        "benchmark-gated research route descriptor for one candidate Transformer-backed article model, with canonical internal-compute claim binding preserved while full served capability publication remains out of scope",
    ))
}

fn aggregate_route_decode_note(
    mode: TassadarExecutorDecodeMode,
    route_posture: TassadarPlannerExecutorRoutePosture,
) -> String {
    match (mode, route_posture) {
        (
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarPlannerExecutorRoutePosture::DirectGuaranteed,
        ) => {
            String::from("benchmark-gated exact dense floor for planner-owned executor delegation")
        }
        (
            TassadarExecutorDecodeMode::HullCache,
            TassadarPlannerExecutorRoutePosture::DirectGuaranteed,
        ) => String::from(
            "benchmark-gated HullCache route posture stays direct across the currently published Wasm module classes",
        ),
        (
            TassadarExecutorDecodeMode::SparseTopK,
            TassadarPlannerExecutorRoutePosture::DirectGuaranteed,
        ) => String::from(
            "benchmark-gated SparseTopK route posture stays direct across the currently published Wasm module classes",
        ),
        (
            TassadarExecutorDecodeMode::HullCache,
            TassadarPlannerExecutorRoutePosture::FallbackCapable,
        ) => String::from(
            "benchmark-gated HullCache route posture; routeable Wasm module classes keep exact direct versus explicit fallback boundaries separate",
        ),
        (
            TassadarExecutorDecodeMode::SparseTopK,
            TassadarPlannerExecutorRoutePosture::FallbackCapable,
        ) => String::from(
            "benchmark-gated SparseTopK route posture; routeable Wasm module classes keep exact direct versus explicit fallback boundaries separate",
        ),
        (
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarPlannerExecutorRoutePosture::FallbackCapable,
        ) => String::from(
            "reference_linear remains the dense floor while routeable Wasm module classes may still force explicit slower-path selection details",
        ),
    }
}

/// Specialized planner-owned request for one article-class exact workflow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHybridWorkflowRequest {
    /// Stable workflow request identifier.
    pub request_id: String,
    /// Product identifier. Must be `psionic.article_hybrid_workflow`.
    pub product_id: String,
    /// Stable planner session identifier.
    pub planner_session_id: String,
    /// Planner model identifier initiating the workflow.
    pub planner_model_id: String,
    /// Stable workflow step identifier.
    pub workflow_step_id: String,
    /// Human-readable planner objective for the exact compute span.
    pub objective: String,
    /// Stable canonical article-case identifier.
    pub article_case_id: String,
    /// Optional explicit executor model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model_id: Option<String>,
    /// Requested decode mode for the exact executor path.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Planner routing policy for the exact compute span.
    pub routing_policy: TassadarPlannerRoutingPolicy,
    /// Planner routing budget for the exact compute span.
    pub routing_budget: TassadarPlannerRoutingBudget,
    /// Ordered environment refs carried into executor lineage in addition to the canonical benchmark ref.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl TassadarArticleHybridWorkflowRequest {
    /// Creates one article-class hybrid workflow request.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        planner_session_id: impl Into<String>,
        planner_model_id: impl Into<String>,
        workflow_step_id: impl Into<String>,
        objective: impl Into<String>,
        article_case_id: impl Into<String>,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(ARTICLE_HYBRID_WORKFLOW_PRODUCT_ID),
            planner_session_id: planner_session_id.into(),
            planner_model_id: planner_model_id.into(),
            workflow_step_id: workflow_step_id.into(),
            objective: objective.into(),
            article_case_id: article_case_id.into(),
            requested_model_id: None,
            requested_decode_mode,
            routing_policy: TassadarPlannerRoutingPolicy::default(),
            routing_budget: TassadarPlannerRoutingBudget::new(16_384, 1_048_576, 8),
            environment_refs: Vec::new(),
        }
    }

    /// Pins execution to one explicit executor model.
    #[must_use]
    pub fn with_requested_model_id(mut self, requested_model_id: impl Into<String>) -> Self {
        self.requested_model_id = Some(requested_model_id.into());
        self
    }

    /// Replaces the routing policy.
    #[must_use]
    pub fn with_routing_policy(mut self, routing_policy: TassadarPlannerRoutingPolicy) -> Self {
        self.routing_policy = routing_policy;
        self
    }

    /// Replaces the routing budget.
    #[must_use]
    pub fn with_routing_budget(mut self, routing_budget: TassadarPlannerRoutingBudget) -> Self {
        self.routing_budget = routing_budget;
        self
    }

    /// Carries extra environment refs into the executor lineage.
    #[must_use]
    pub fn with_environment_refs(mut self, mut environment_refs: Vec<String>) -> Self {
        environment_refs.sort();
        environment_refs.dedup();
        self.environment_refs = environment_refs;
        self
    }

    /// Returns a stable digest over the hybrid workflow request.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"tassadar_article_hybrid_workflow_request|", self)
    }
}

/// Completed article-class hybrid workflow response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHybridWorkflowCompletedResponse {
    /// Stable workflow request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Canonical benchmark/workload identity.
    pub benchmark_identity: TassadarArticleBenchmarkIdentity,
    /// Proof identity preserved across the workflow boundary.
    pub proof_identity: TassadarArticleProofIdentity,
    /// Underlying planner-owned completed response.
    pub planner_response: TassadarPlannerCompletedResponse,
}

/// Fallback article-class hybrid workflow response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHybridWorkflowFallbackResponse {
    /// Stable workflow request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Canonical benchmark/workload identity when the case was resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_identity: Option<TassadarArticleBenchmarkIdentity>,
    /// Underlying planner fallback response.
    pub planner_fallback: TassadarPlannerFallbackResponse,
}

/// Refusal article-class hybrid workflow response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleHybridWorkflowRefusalResponse {
    /// Stable workflow request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Canonical benchmark/workload identity when the case was resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_identity: Option<TassadarArticleBenchmarkIdentity>,
    /// Underlying planner refusal response when routing reached the planner layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planner_refusal: Option<TassadarPlannerRefusalResponse>,
    /// Human-readable refusal detail.
    pub detail: String,
}

/// Outcome for one article-class hybrid workflow request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarArticleHybridWorkflowOutcome {
    /// The workflow delegated successfully into the exact executor lane.
    Completed {
        /// Completed workflow response.
        response: TassadarArticleHybridWorkflowCompletedResponse,
    },
    /// The workflow returned a typed planner fallback.
    Fallback {
        /// Typed workflow fallback.
        fallback: TassadarArticleHybridWorkflowFallbackResponse,
    },
    /// The workflow refused the request explicitly.
    Refused {
        /// Typed workflow refusal.
        refusal: TassadarArticleHybridWorkflowRefusalResponse,
    },
}

/// Product-validation error for the article hybrid-workflow surface.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarArticleHybridWorkflowServiceError {
    /// The request targeted a different product family.
    #[error("unsupported Tassadar article hybrid-workflow product `{product_id}`")]
    UnsupportedProduct {
        /// Product identifier supplied by the caller.
        product_id: String,
    },
}

/// Local article-class hybrid workflow service above the generic planner route.
#[derive(Clone, Debug)]
pub struct LocalTassadarArticleHybridWorkflowService {
    planner_router: LocalTassadarPlannerRouter,
}

impl Default for LocalTassadarArticleHybridWorkflowService {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalTassadarArticleHybridWorkflowService {
    /// Creates the default local article hybrid-workflow service.
    #[must_use]
    pub fn new() -> Self {
        let executor_service = LocalTassadarExecutorService::new()
            .with_fixture(TassadarExecutorFixture::article_i32_compute_v1());
        Self {
            planner_router: LocalTassadarPlannerRouter::new()
                .with_executor_service(executor_service),
        }
    }

    /// Replaces the backing planner router.
    #[must_use]
    pub fn with_planner_router(mut self, planner_router: LocalTassadarPlannerRouter) -> Self {
        self.planner_router = planner_router;
        self
    }

    /// Executes one article-class hybrid workflow.
    pub fn execute(
        &self,
        request: &TassadarArticleHybridWorkflowRequest,
    ) -> Result<TassadarArticleHybridWorkflowOutcome, TassadarArticleHybridWorkflowServiceError>
    {
        self.validate_product(request)?;
        let Some(case) = article_case_by_id(request.article_case_id.as_str()) else {
            return Ok(TassadarArticleHybridWorkflowOutcome::Refused {
                refusal: TassadarArticleHybridWorkflowRefusalResponse {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    benchmark_identity: None,
                    planner_refusal: None,
                    detail: format!(
                        "article workflow case `{}` is not present in the canonical Tassadar article corpus",
                        request.article_case_id
                    ),
                },
            });
        };

        let planner_request = self.planner_request_for(request, &case);
        let benchmark_identity = TassadarArticleBenchmarkIdentity::for_case(
            &case,
            &planner_request.subproblem.program_artifact,
        );
        match self.planner_router.route(&planner_request) {
            Ok(TassadarPlannerRoutingOutcome::Completed { response }) => {
                let proof_identity =
                    TassadarArticleProofIdentity::from_response(&response.executor_response);
                Ok(TassadarArticleHybridWorkflowOutcome::Completed {
                    response: TassadarArticleHybridWorkflowCompletedResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        benchmark_identity,
                        proof_identity,
                        planner_response: response,
                    },
                })
            }
            Ok(TassadarPlannerRoutingOutcome::Fallback { fallback }) => {
                Ok(TassadarArticleHybridWorkflowOutcome::Fallback {
                    fallback: TassadarArticleHybridWorkflowFallbackResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        benchmark_identity: Some(benchmark_identity),
                        planner_fallback: fallback,
                    },
                })
            }
            Ok(TassadarPlannerRoutingOutcome::Refused { refusal }) => {
                Ok(TassadarArticleHybridWorkflowOutcome::Refused {
                    refusal: TassadarArticleHybridWorkflowRefusalResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        benchmark_identity: Some(benchmark_identity),
                        detail: refusal.detail.clone(),
                        planner_refusal: Some(refusal),
                    },
                })
            }
            Err(TassadarPlannerRouterError::UnsupportedProduct { product_id }) => {
                Err(TassadarArticleHybridWorkflowServiceError::UnsupportedProduct { product_id })
            }
        }
    }

    fn validate_product(
        &self,
        request: &TassadarArticleHybridWorkflowRequest,
    ) -> Result<(), TassadarArticleHybridWorkflowServiceError> {
        if request.product_id == ARTICLE_HYBRID_WORKFLOW_PRODUCT_ID {
            Ok(())
        } else {
            Err(
                TassadarArticleHybridWorkflowServiceError::UnsupportedProduct {
                    product_id: request.product_id.clone(),
                },
            )
        }
    }

    fn planner_request_for(
        &self,
        request: &TassadarArticleHybridWorkflowRequest,
        case: &TassadarValidationCase,
    ) -> TassadarPlannerRoutingRequest {
        let profile = tassadar_wasm_profile_for_id(case.program.profile_id.as_str())
            .expect("canonical article case should use a supported profile");
        let trace_abi = tassadar_trace_abi_for_profile_id(case.program.profile_id.as_str())
            .expect("canonical article case should use a supported trace ABI");
        let artifact = TassadarProgramArtifact::fixture_reference(
            format!("artifact://tassadar/article_workflow/{}", case.case_id),
            &profile,
            &trace_abi,
            case.program.clone(),
        )
        .expect("canonical article case should assemble into a program artifact");
        let mut subproblem = TassadarPlannerExecutorSubproblem::new(
            request.workflow_step_id.clone(),
            request.objective.clone(),
            artifact,
            request.requested_decode_mode,
        )
        .with_environment_refs(merge_article_environment_refs(
            request.environment_refs.as_slice(),
        ));
        if let Some(requested_model_id) = request.requested_model_id.as_deref() {
            subproblem = subproblem.with_requested_model_id(requested_model_id);
        } else {
            subproblem = subproblem
                .with_requested_model_id(TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID);
        }
        TassadarPlannerRoutingRequest::new(
            request.request_id.clone(),
            request.planner_session_id.clone(),
            request.planner_model_id.clone(),
            subproblem,
        )
        .with_routing_policy(request.routing_policy.clone())
        .with_routing_budget(request.routing_budget.clone())
    }
}

fn route_trace_step_budget(program_artifact: &TassadarProgramArtifact) -> usize {
    tassadar_wasm_profile_for_id(program_artifact.wasm_profile_id.as_str()).map_or_else(
        || program_artifact.validated_program.instructions.len(),
        |profile| profile.max_steps,
    )
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar planner routing value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn article_case_by_id(case_id: &str) -> Option<TassadarValidationCase> {
    tassadar_article_class_corpus()
        .into_iter()
        .find(|case| case.case_id == case_id)
        .or_else(|| {
            tassadar_hungarian_10x10_corpus()
                .into_iter()
                .find(|case| case.case_id == case_id)
                .map(|case| case.validation_case)
        })
        .or_else(|| {
            tassadar_article_hard_sudoku_suite()
                .into_iter()
                .find(|case| case.case_id == case_id)
                .map(|case| case.validation_case)
        })
}

fn article_workload_family(case_id: &str) -> &'static str {
    match case_id {
        "micro_wasm_kernel" => "MicroWasmKernel",
        "branch_heavy_kernel" => "BranchHeavyKernel",
        "memory_heavy_kernel" => "MemoryHeavyKernel",
        "long_loop_kernel" => "LongLoopKernel",
        value if value.starts_with("sudoku_") => "SudokuClass",
        value if value.starts_with("hungarian_") => "HungarianMatching",
        _ => "ArticleWorkload",
    }
}

fn merge_article_environment_refs(environment_refs: &[String]) -> Vec<String> {
    let mut merged = environment_refs.to_vec();
    merged.push(String::from(
        TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF,
    ));
    merged.sort();
    merged.dedup();
    merged
}

fn build_article_readable_log_excerpt(
    case: &TassadarValidationCase,
    execution: &TassadarExecution,
) -> TassadarArticleReadableLogExcerpt {
    let mut lines = Vec::with_capacity(execution.steps.len().saturating_add(8));
    lines.push(format!(
        "benchmark_ref={}",
        TASSADAR_ARTICLE_CLASS_BENCHMARK_REF
    ));
    lines.push(format!(
        "workload_family={}",
        article_workload_family(case.case_id.as_str())
    ));
    lines.push(format!("case_id={}", case.case_id));
    lines.push(format!("program_id={}", execution.program_id));
    lines.push(format!("runner_id={}", execution.runner_id));
    for step in &execution.steps {
        lines.push(format!(
            "step={} pc={}->{} instr={} event={}",
            step.step_index,
            step.pc,
            step.next_pc,
            format_instruction(&step.instruction),
            format_event(&step.event)
        ));
    }
    lines.push(format!("halt={:?}", execution.halt_reason));
    lines.push(format!("outputs={:?}", execution.outputs));

    let total_line_count = lines.len();
    let truncated = total_line_count > ARTICLE_EXECUTOR_READABLE_LOG_MAX_LINES;
    let lines = if truncated {
        let retained = ARTICLE_EXECUTOR_READABLE_LOG_MAX_LINES.saturating_sub(1);
        let mut excerpt = lines.into_iter().take(retained).collect::<Vec<_>>();
        excerpt.push(format!(
            "... truncated {} additional lines; canonical machine truth remains bound to trace_digest={}",
            total_line_count.saturating_sub(retained),
            execution.trace_digest()
        ));
        excerpt
    } else {
        lines
    };

    TassadarArticleReadableLogExcerpt {
        max_lines: ARTICLE_EXECUTOR_READABLE_LOG_MAX_LINES,
        total_line_count,
        truncated,
        lines,
        derivation_posture: String::from(
            "readable log lines are derived, non-authoritative views over the canonical append-only trace and may be truncated without changing machine truth",
        ),
    }
}

fn build_article_token_trace_excerpt(
    case: &TassadarValidationCase,
    execution: &TassadarExecution,
) -> TassadarArticleTokenTraceExcerpt {
    let tokenizer = TassadarTraceTokenizer::new();
    let tokenized = tokenizer.tokenize_program_and_execution(&case.program, execution);
    let decoded = tokenizer.decode_symbolic(&tokenized);
    let prompt_token_count = decoded.prompt_token_count;
    let total_token_count = decoded.tokens.len();
    let truncated = total_token_count > ARTICLE_EXECUTOR_TOKEN_TRACE_MAX_TOKENS;
    let tokens = if truncated {
        decoded
            .tokens
            .into_iter()
            .take(ARTICLE_EXECUTOR_TOKEN_TRACE_MAX_TOKENS)
            .collect()
    } else {
        decoded.tokens
    };
    TassadarArticleTokenTraceExcerpt {
        max_tokens: ARTICLE_EXECUTOR_TOKEN_TRACE_MAX_TOKENS,
        total_token_count,
        prompt_token_count,
        truncated,
        tokenizer_digest: tokenizer.stable_digest(),
        tokens,
        derivation_posture: String::from(
            "symbolic token traces are deterministic derivations of the canonical append-only trace artifact and may be truncated for serving without changing proof or benchmark identity",
        ),
    }
}

/// Derives the canonical article-session stream transcript from one completed outcome.
#[must_use]
pub fn tassadar_article_executor_session_stream_events_for_outcome(
    outcome: &TassadarArticleExecutorSessionOutcome,
) -> Vec<TassadarArticleExecutorSessionStreamEvent> {
    let runtime_capability = match outcome {
        TassadarArticleExecutorSessionOutcome::Completed { response } => {
            response.executor_response.runtime_capability.clone()
        }
        TassadarArticleExecutorSessionOutcome::Refused { refusal } => {
            refusal.runtime_capability.clone()
        }
    };
    let mut events = Vec::new();
    match outcome {
        TassadarArticleExecutorSessionOutcome::Completed { response } => {
            events.push(
                TassadarArticleExecutorSessionStreamEvent::BenchmarkIdentity {
                    benchmark_identity: TassadarArticleBenchmarkIdentityEvent {
                        benchmark_identity: response.benchmark_identity.clone(),
                    },
                },
            );
            events
                .push(TassadarArticleExecutorSessionStreamEvent::Capability { runtime_capability });
            events.push(TassadarArticleExecutorSessionStreamEvent::Selection {
                selection: response
                    .executor_response
                    .execution_report
                    .selection
                    .clone(),
            });
            events.push(TassadarArticleExecutorSessionStreamEvent::ProofIdentity {
                proof_identity: TassadarArticleProofIdentityEvent {
                    proof_identity: response.proof_identity.clone(),
                },
            });
            if let Some(proof_receipt) = &response.direct_model_weight_execution_proof_receipt {
                events.push(
                    TassadarArticleExecutorSessionStreamEvent::DirectModelWeightExecutionProof {
                        direct_model_weight_execution_proof:
                            TassadarArticleDirectModelWeightExecutionProofEvent {
                                proof_receipt: proof_receipt.clone(),
                            },
                    },
                );
            }
            for (line_index, line) in response.readable_log.lines.iter().enumerate() {
                events.push(TassadarArticleExecutorSessionStreamEvent::ReadableLogLine {
                    readable_log_line: TassadarArticleReadableLogLineEvent {
                        line_index,
                        line: line.clone(),
                    },
                });
            }
            for (chunk_index, chunk) in response
                .token_trace
                .tokens
                .chunks(ARTICLE_EXECUTOR_TOKEN_TRACE_CHUNK_SIZE)
                .enumerate()
            {
                events.push(TassadarArticleExecutorSessionStreamEvent::TokenTraceChunk {
                    token_trace_chunk: TassadarArticleTokenTraceChunkEvent {
                        chunk_index,
                        prompt_token_count: response.token_trace.prompt_token_count,
                        total_token_count: response.token_trace.total_token_count,
                        truncated: response.token_trace.truncated,
                        tokens: chunk.to_vec(),
                    },
                });
            }
            for (ordinal, value) in response.final_outputs().iter().enumerate() {
                events.push(TassadarArticleExecutorSessionStreamEvent::Output {
                    output: TassadarExecutorOutputEvent {
                        ordinal,
                        value: *value,
                    },
                });
            }
        }
        TassadarArticleExecutorSessionOutcome::Refused { refusal } => {
            if let Some(benchmark_identity) = refusal.benchmark_identity.clone() {
                events.push(
                    TassadarArticleExecutorSessionStreamEvent::BenchmarkIdentity {
                        benchmark_identity: TassadarArticleBenchmarkIdentityEvent {
                            benchmark_identity,
                        },
                    },
                );
            }
            events
                .push(TassadarArticleExecutorSessionStreamEvent::Capability { runtime_capability });
            if let Some(selection) = refusal.selection.clone() {
                events.push(TassadarArticleExecutorSessionStreamEvent::Selection { selection });
            }
        }
    }
    events.push(TassadarArticleExecutorSessionStreamEvent::Terminal {
        terminal: TassadarArticleExecutorSessionTerminalEvent {
            outcome: outcome.clone(),
        },
    });
    events
}

fn format_instruction(instruction: &TassadarInstruction) -> String {
    match instruction {
        TassadarInstruction::Nop => String::from("nop"),
        TassadarInstruction::I32Const { value } => format!("i32.const {value}"),
        TassadarInstruction::LocalGet { local } => format!("local.get {local}"),
        TassadarInstruction::LocalSet { local } => format!("local.set {local}"),
        TassadarInstruction::LocalTee { local } => format!("local.tee {local}"),
        TassadarInstruction::Drop => String::from("drop"),
        TassadarInstruction::I32Add => String::from("i32.add"),
        TassadarInstruction::I32Sub => String::from("i32.sub"),
        TassadarInstruction::I32Mul => String::from("i32.mul"),
        TassadarInstruction::I32Lt => String::from("i32.lt"),
        TassadarInstruction::I32Eqz => String::from("i32.eqz"),
        TassadarInstruction::I32Eq => String::from("i32.eq"),
        TassadarInstruction::I32Ne => String::from("i32.ne"),
        TassadarInstruction::I32Gt => String::from("i32.gt"),
        TassadarInstruction::I32Le => String::from("i32.le"),
        TassadarInstruction::I32Ge => String::from("i32.ge"),
        TassadarInstruction::I32Load { slot } => format!("i32.load {slot}"),
        TassadarInstruction::I32Store { slot } => format!("i32.store {slot}"),
        TassadarInstruction::BrIf { target_pc } => format!("br_if {target_pc}"),
        TassadarInstruction::Output => String::from("output"),
        TassadarInstruction::Return => String::from("return"),
    }
}

const TASSADAR_ACCEPTANCE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_acceptance_report.json";
const TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json";
const TASSADAR_LEARNED_HORIZON_POLICY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_learned_horizon_policy_report.json";
const TASSADAR_LEARNED_PROMOTION_GATE_REPORT_REF: &str =
    "fixtures/tassadar/runs/sudoku_v0_promotion_v3/promotion_gate_report.json";
const TASSADAR_LEARNED_9X9_SEQUENCE_FIT_REPORT_REF: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_reference_run_v0/sequence_fit_report.json";
const TASSADAR_ARCHITECTURE_COMPARISON_REPORT_REF: &str = "fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v12/architecture_comparison_report.json";
const TASSADAR_REPLAY_SOURCE_BADGE: &str = "replay.tassadar";
const TASSADAR_LIVE_SOURCE_BADGE: &str = "psionic.tassadar";
const TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION: u16 = 1;

/// Stable replay identifiers for the canonical Tassadar lab explorer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLabReplayId {
    ArticleSessionDirectMemoryHeavy,
    ArticleSessionFallbackBranchHeavy,
    ArticleSessionRefusalNonArticle,
    ArticleHybridDelegatedMemoryHeavy,
    ArticleHybridFallbackBranchHeavy,
    ArticleHybridRefusalOverBudget,
    AcceptanceReport,
    CompiledArticleClosureReport,
    LearnedHorizonPolicyReport,
    LearnedPromotionGateV3,
    LearnedSudoku9x9FitReport,
    ArchitectureComparisonV12,
}

impl TassadarLabReplayId {
    /// Returns the stable replay id string.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::ArticleSessionDirectMemoryHeavy => "article_session_direct_memory_heavy",
            Self::ArticleSessionFallbackBranchHeavy => "article_session_fallback_branch_heavy",
            Self::ArticleSessionRefusalNonArticle => "article_session_refusal_non_article",
            Self::ArticleHybridDelegatedMemoryHeavy => "article_hybrid_delegated_memory_heavy",
            Self::ArticleHybridFallbackBranchHeavy => "article_hybrid_fallback_branch_heavy",
            Self::ArticleHybridRefusalOverBudget => "article_hybrid_refusal_over_budget",
            Self::AcceptanceReport => "acceptance_report",
            Self::CompiledArticleClosureReport => "compiled_article_closure_report",
            Self::LearnedHorizonPolicyReport => "learned_horizon_policy_report",
            Self::LearnedPromotionGateV3 => "learned_promotion_gate_v3",
            Self::LearnedSudoku9x9FitReport => "learned_sudoku_9x9_fit_report",
            Self::ArchitectureComparisonV12 => "architecture_comparison_v12",
        }
    }

    #[must_use]
    const fn family_label(self) -> &'static str {
        match self {
            Self::ArticleSessionDirectMemoryHeavy
            | Self::ArticleSessionFallbackBranchHeavy
            | Self::ArticleSessionRefusalNonArticle => "Article session artifact",
            Self::ArticleHybridDelegatedMemoryHeavy
            | Self::ArticleHybridFallbackBranchHeavy
            | Self::ArticleHybridRefusalOverBudget => "Article hybrid workflow artifact",
            Self::AcceptanceReport => "Acceptance report",
            Self::CompiledArticleClosureReport => "Compiled article closure report",
            Self::LearnedHorizonPolicyReport => "Learned horizon policy report",
            Self::LearnedPromotionGateV3 => "Learned promotion gate",
            Self::LearnedSudoku9x9FitReport => "Learned 9x9 fit report",
            Self::ArchitectureComparisonV12 => "Architecture comparison",
        }
    }

    #[must_use]
    const fn label(self) -> &'static str {
        match self {
            Self::ArticleSessionDirectMemoryHeavy => "Direct memory-heavy article session",
            Self::ArticleSessionFallbackBranchHeavy => "Fallback branch-heavy article session",
            Self::ArticleSessionRefusalNonArticle => "Refused non-article article session",
            Self::ArticleHybridDelegatedMemoryHeavy => "Delegated memory-heavy hybrid workflow",
            Self::ArticleHybridFallbackBranchHeavy => "Fallback branch-heavy hybrid workflow",
            Self::ArticleHybridRefusalOverBudget => "Refused over-budget hybrid workflow",
            Self::AcceptanceReport => "Tassadar acceptance report",
            Self::CompiledArticleClosureReport => "Compiled article closure report",
            Self::LearnedHorizonPolicyReport => "Learned long-horizon policy",
            Self::LearnedPromotionGateV3 => "Learned 4x4 promotion gate",
            Self::LearnedSudoku9x9FitReport => "Learned 9x9 fit report",
            Self::ArchitectureComparisonV12 => "Architecture comparison v12",
        }
    }

    #[must_use]
    const fn description(self) -> &'static str {
        match self {
            Self::ArticleSessionDirectMemoryHeavy => {
                "Replays the direct HullCache article-session artifact over the canonical memory-heavy kernel."
            }
            Self::ArticleSessionFallbackBranchHeavy => {
                "Replays the typed sparse-top-k fallback artifact on the branch-heavy kernel."
            }
            Self::ArticleSessionRefusalNonArticle => {
                "Replays the explicit non-article refusal case for the specialized article-session surface."
            }
            Self::ArticleHybridDelegatedMemoryHeavy => {
                "Replays the delegated planner-owned article workflow over the memory-heavy kernel."
            }
            Self::ArticleHybridFallbackBranchHeavy => {
                "Replays the planner fallback path when branch-heavy sparse execution falls back under policy."
            }
            Self::ArticleHybridRefusalOverBudget => {
                "Replays the typed planner refusal when the workflow budget rejects the article request."
            }
            Self::AcceptanceReport => {
                "Replays the machine-readable acceptance verdicts that define the current honest Tassadar claim boundary."
            }
            Self::CompiledArticleClosureReport => {
                "Replays the compiled article-closure checker over the 9x9 Sudoku, 10x10 Hungarian, and kernel-suite evidence roots."
            }
            Self::LearnedHorizonPolicyReport => {
                "Replays the learned long-horizon guardrail that keeps article claims honest when fit and benchmark scope stay bounded."
            }
            Self::LearnedPromotionGateV3 => {
                "Replays the green learned 4x4 promotion gate that proves the bounded learned lane reached its exact validation bar."
            }
            Self::LearnedSudoku9x9FitReport => {
                "Replays the honest 9x9 fit report that keeps full-sequence overflow and bounded-window scope explicit."
            }
            Self::ArchitectureComparisonV12 => {
                "Replays the same-corpus architecture-family comparison across hull, sparse, hybrid, and recurrent learned lanes."
            }
        }
    }

    #[must_use]
    const fn artifact_ref(self) -> &'static str {
        match self {
            Self::ArticleSessionDirectMemoryHeavy
            | Self::ArticleSessionFallbackBranchHeavy
            | Self::ArticleSessionRefusalNonArticle => {
                TASSADAR_ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF
            }
            Self::ArticleHybridDelegatedMemoryHeavy
            | Self::ArticleHybridFallbackBranchHeavy
            | Self::ArticleHybridRefusalOverBudget => TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF,
            Self::AcceptanceReport => TASSADAR_ACCEPTANCE_REPORT_REF,
            Self::CompiledArticleClosureReport => TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF,
            Self::LearnedHorizonPolicyReport => TASSADAR_LEARNED_HORIZON_POLICY_REPORT_REF,
            Self::LearnedPromotionGateV3 => TASSADAR_LEARNED_PROMOTION_GATE_REPORT_REF,
            Self::LearnedSudoku9x9FitReport => TASSADAR_LEARNED_9X9_SEQUENCE_FIT_REPORT_REF,
            Self::ArchitectureComparisonV12 => TASSADAR_ARCHITECTURE_COMPARISON_REPORT_REF,
        }
    }
}

/// One catalog entry exposed to pane consumers for replay-first exploration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLabReplayCatalogEntry {
    /// Stable replay identifier.
    pub replay_id: TassadarLabReplayId,
    /// Short family label used to group related replay roots.
    pub family_label: String,
    /// Human-readable replay label.
    pub label: String,
    /// Repo-relative artifact root backing the replay.
    pub artifact_ref: String,
    /// Plain-language description of the replay.
    pub description: String,
}

/// Stable source posture for one prepared Tassadar lab snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLabSourceKind {
    LiveArticleSession,
    LiveArticleHybridWorkflow,
    ReplayArtifact,
}

/// One metric chip surfaced to the pane without forcing the app to interpret
/// report internals itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLabMetricChip {
    /// Short label safe for WGPUI cards.
    pub label: String,
    /// Display-ready value.
    pub value: String,
    /// Stable tone hint such as `green`, `amber`, `red`, or `blue`.
    pub tone: String,
}

/// One detail row surfaced to the pane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLabFactLine {
    /// Left-hand label.
    pub label: String,
    /// Display-ready value.
    pub value: String,
}

/// Renderer-neutral snapshot for one replayed or live Tassadar lab view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLabSnapshot {
    /// Stable schema version.
    pub schema_version: u16,
    /// Short source badge such as `psionic.tassadar` or `replay.tassadar`.
    pub source_badge: String,
    /// Whether the view came from a live service or a replay artifact.
    pub source_kind: TassadarLabSourceKind,
    /// Stable replay identifier when the snapshot came from a committed artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_id: Option<TassadarLabReplayId>,
    /// Short family label shown in the pane hero.
    pub family_label: String,
    /// Main subject label for the current view.
    pub subject_label: String,
    /// Short status label such as `direct exact`, `planner fallback`, or `acceptance green`.
    pub status_label: String,
    /// One-sentence detail describing the current posture.
    pub detail_label: String,
    /// Repo-relative artifact ref when the snapshot is backed by a committed artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_ref: Option<String>,
    /// Preserved benchmark identity when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_identity: Option<TassadarArticleBenchmarkIdentity>,
    /// Preserved proof identity when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_identity: Option<TassadarArticleProofIdentity>,
    /// Runtime capability visible to the consumer when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_capability: Option<TassadarRuntimeCapabilityReport>,
    /// Requested decode mode when the view originates from a request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Effective decode mode after runtime routing when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Route or selection posture label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_state_label: Option<String>,
    /// Route or selection detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_detail: Option<String>,
    /// Program identifier when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_id: Option<String>,
    /// Wasm profile identifier when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_profile_id: Option<String>,
    /// Derived readable-log excerpt when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readable_log: Option<TassadarArticleReadableLogExcerpt>,
    /// Derived symbolic token-trace excerpt when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_trace: Option<TassadarArticleTokenTraceExcerpt>,
    /// Final scalar outputs when one exists.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub final_outputs: Vec<i32>,
    /// Summary chips used by the pane overview cards.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metric_chips: Vec<TassadarLabMetricChip>,
    /// Detail rows for program and evidence panels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fact_lines: Vec<TassadarLabFactLine>,
    /// Ordered summary events for the hero feed and replay-first mode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
}

/// Stable update emitted by the Tassadar lab surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarLabUpdate {
    /// One generic status line, mainly used for report replays.
    StatusLine {
        /// Zero-based line index.
        line_index: usize,
        /// Renderable line.
        line: String,
    },
    /// Preserved article benchmark identity.
    BenchmarkIdentity {
        /// Benchmark identity payload.
        benchmark_identity: TassadarArticleBenchmarkIdentityEvent,
    },
    /// Preserved runtime capability.
    Capability {
        /// Runtime capability payload.
        runtime_capability: TassadarRuntimeCapabilityReport,
    },
    /// Preserved decode-selection diagnostic.
    Selection {
        /// Selection payload.
        selection: TassadarExecutorSelectionDiagnostic,
    },
    /// Preserved planner routing posture.
    RoutingStatus {
        /// Stable route-state label.
        route_state: String,
        /// Renderable route detail.
        detail: String,
        /// Effective decode mode when one exists.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    },
    /// Preserved proof identity.
    ProofIdentity {
        /// Proof identity payload.
        proof_identity: TassadarArticleProofIdentityEvent,
    },
    /// One derived readable-log line.
    ReadableLogLine {
        /// Readable-log payload.
        readable_log_line: TassadarArticleReadableLogLineEvent,
    },
    /// One symbolic token-trace chunk.
    TokenTraceChunk {
        /// Token-trace payload.
        token_trace_chunk: TassadarArticleTokenTraceChunkEvent,
    },
    /// One emitted output value.
    Output {
        /// Output payload.
        output: TassadarExecutorOutputEvent,
    },
    /// Terminal update for either live or replay paths.
    Terminal {
        /// Stable terminal status label.
        status_label: String,
        /// Renderable detail.
        detail: String,
    },
}

/// One prepared replay/live response for desktop consumers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLabPreparedView {
    /// Prepared renderer-neutral snapshot.
    pub snapshot: TassadarLabSnapshot,
    /// Ordered updates suitable for replay-first playback.
    pub updates: Vec<TassadarLabUpdate>,
}

/// Stable request surface for the replay/live Tassadar lab adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "request_kind", rename_all = "snake_case")]
pub enum TassadarLabRequest {
    /// Prepare one live specialized article session.
    ArticleExecutorSession {
        /// Underlying served request.
        request: TassadarArticleExecutorSessionRequest,
    },
    /// Prepare one live specialized article hybrid workflow.
    ArticleHybridWorkflow {
        /// Underlying served request.
        request: TassadarArticleHybridWorkflowRequest,
    },
    /// Prepare one canonical replay root from committed artifacts.
    Replay {
        /// Stable replay id.
        replay_id: TassadarLabReplayId,
    },
}

/// Error while adapting replay/live Tassadar truth for the desktop lab.
#[derive(Debug, Error)]
pub enum TassadarLabServiceError {
    /// The specialized article-session surface rejected the request.
    #[error(transparent)]
    ArticleExecutorSession(#[from] TassadarArticleExecutorSessionServiceError),
    /// The specialized article hybrid-workflow surface rejected the request.
    #[error(transparent)]
    ArticleHybridWorkflow(#[from] TassadarArticleHybridWorkflowServiceError),
    /// Reading one committed replay artifact failed.
    #[error("failed to read `{path}`: {error}")]
    Read {
        /// Artifact path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Decoding one committed replay artifact failed.
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Deserialize {
        /// Artifact kind.
        artifact_kind: String,
        /// Artifact path.
        path: String,
        /// Source error.
        error: serde_json::Error,
    },
    /// One replay entry could not be found inside its source artifact.
    #[error("replay case `{case_name}` is missing from `{artifact_ref}`")]
    MissingReplayCase {
        /// Stable case name.
        case_name: String,
        /// Artifact ref consulted by the service.
        artifact_ref: String,
    },
    /// A canonical article case could not be resolved.
    #[error("canonical article case `{case_id}` is missing")]
    MissingArticleCase {
        /// Case identifier.
        case_id: String,
    },
}

/// Local replay/live adapter for the desktop Tassadar pane.
#[derive(Clone, Debug)]
pub struct LocalTassadarLabService {
    article_executor_session: LocalTassadarArticleExecutorSessionService,
    article_hybrid_workflow: LocalTassadarArticleHybridWorkflowService,
}

impl Default for LocalTassadarLabService {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalTassadarLabService {
    /// Creates the default local lab adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            article_executor_session: LocalTassadarArticleExecutorSessionService::new(),
            article_hybrid_workflow: LocalTassadarArticleHybridWorkflowService::new(),
        }
    }

    /// Returns the canonical replay catalog the desktop should expose.
    #[must_use]
    pub fn replay_catalog(&self) -> Vec<TassadarLabReplayCatalogEntry> {
        canonical_tassadar_lab_replay_catalog()
    }

    /// Prepares one replay or live view as a renderer-neutral snapshot plus ordered updates.
    pub fn prepare(
        &self,
        request: &TassadarLabRequest,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        match request {
            TassadarLabRequest::ArticleExecutorSession { request } => {
                self.prepare_article_executor_session(request)
            }
            TassadarLabRequest::ArticleHybridWorkflow { request } => {
                self.prepare_article_hybrid_workflow(request)
            }
            TassadarLabRequest::Replay { replay_id } => self.prepare_replay(*replay_id),
        }
    }

    fn prepare_article_executor_session(
        &self,
        request: &TassadarArticleExecutorSessionRequest,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        let outcome = self.article_executor_session.execute(request)?;
        let updates = tassadar_article_executor_session_stream_events_for_outcome(&outcome)
            .into_iter()
            .map(tassadar_lab_update_from_article_event)
            .collect::<Vec<_>>();
        let snapshot = snapshot_from_article_executor_session(request, &outcome);
        Ok(TassadarLabPreparedView { snapshot, updates })
    }

    fn prepare_article_hybrid_workflow(
        &self,
        request: &TassadarArticleHybridWorkflowRequest,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        let outcome = self.article_hybrid_workflow.execute(request)?;
        let snapshot = snapshot_from_article_hybrid_workflow(request, &outcome)?;
        let updates = updates_from_article_hybrid_workflow(&outcome)?;
        Ok(TassadarLabPreparedView { snapshot, updates })
    }

    fn prepare_replay(
        &self,
        replay_id: TassadarLabReplayId,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        match replay_id {
            TassadarLabReplayId::ArticleSessionDirectMemoryHeavy
            | TassadarLabReplayId::ArticleSessionFallbackBranchHeavy
            | TassadarLabReplayId::ArticleSessionRefusalNonArticle => {
                self.prepare_replay_article_executor_session(replay_id)
            }
            TassadarLabReplayId::ArticleHybridDelegatedMemoryHeavy
            | TassadarLabReplayId::ArticleHybridFallbackBranchHeavy
            | TassadarLabReplayId::ArticleHybridRefusalOverBudget => {
                self.prepare_replay_article_hybrid_workflow(replay_id)
            }
            TassadarLabReplayId::AcceptanceReport => self.prepare_replay_acceptance_report(),
            TassadarLabReplayId::CompiledArticleClosureReport => {
                self.prepare_replay_compiled_article_closure_report()
            }
            TassadarLabReplayId::LearnedHorizonPolicyReport => {
                self.prepare_replay_learned_horizon_policy_report()
            }
            TassadarLabReplayId::LearnedPromotionGateV3 => {
                self.prepare_replay_learned_promotion_gate_report()
            }
            TassadarLabReplayId::LearnedSudoku9x9FitReport => {
                self.prepare_replay_learned_sudoku_9x9_fit_report()
            }
            TassadarLabReplayId::ArchitectureComparisonV12 => {
                self.prepare_replay_architecture_comparison_report()
            }
        }
    }

    fn prepare_replay_article_executor_session(
        &self,
        replay_id: TassadarLabReplayId,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        let artifact: TassadarLabArticleExecutorSessionArtifact = read_repo_json(
            TASSADAR_ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF,
            "tassadar_lab_article_executor_session_artifact",
        )?;
        let case_name = match replay_id {
            TassadarLabReplayId::ArticleSessionDirectMemoryHeavy => "direct_memory_heavy_hull",
            TassadarLabReplayId::ArticleSessionFallbackBranchHeavy => {
                "fallback_branch_heavy_sparse_top_k"
            }
            TassadarLabReplayId::ArticleSessionRefusalNonArticle => "refusal_non_article_workload",
            _ => unreachable!("replay id routed to wrong artifact family"),
        };
        let case = artifact
            .cases
            .into_iter()
            .find(|candidate| candidate.name == case_name)
            .ok_or_else(|| TassadarLabServiceError::MissingReplayCase {
                case_name: String::from(case_name),
                artifact_ref: String::from(TASSADAR_ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF),
            })?;
        let mut prepared = self.prepare_article_executor_session(&case.request)?;
        prepared.snapshot.source_badge = String::from(TASSADAR_REPLAY_SOURCE_BADGE);
        prepared.snapshot.source_kind = TassadarLabSourceKind::ReplayArtifact;
        prepared.snapshot.replay_id = Some(replay_id);
        prepared.snapshot.artifact_ref =
            Some(String::from(TASSADAR_ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF));
        prepared.snapshot.events.insert(
            0,
            format!(
                "replay root={} case={}",
                TASSADAR_ARTICLE_EXECUTOR_SESSION_ARTIFACT_REF, case.name
            ),
        );
        Ok(prepared)
    }

    fn prepare_replay_article_hybrid_workflow(
        &self,
        replay_id: TassadarLabReplayId,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        let artifact: TassadarLabArticleHybridWorkflowArtifact = read_repo_json(
            TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF,
            "tassadar_lab_article_hybrid_workflow_artifact",
        )?;
        let case_name = match replay_id {
            TassadarLabReplayId::ArticleHybridDelegatedMemoryHeavy => "delegated_memory_heavy_hull",
            TassadarLabReplayId::ArticleHybridFallbackBranchHeavy => {
                "fallback_branch_heavy_sparse_top_k"
            }
            TassadarLabReplayId::ArticleHybridRefusalOverBudget => {
                "refusal_overbudget_memory_heavy"
            }
            _ => unreachable!("replay id routed to wrong artifact family"),
        };
        let case = artifact
            .cases
            .into_iter()
            .find(|candidate| candidate.name == case_name)
            .ok_or_else(|| TassadarLabServiceError::MissingReplayCase {
                case_name: String::from(case_name),
                artifact_ref: String::from(TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF),
            })?;
        let mut prepared = self.prepare_article_hybrid_workflow(&case.request)?;
        prepared.snapshot.source_badge = String::from(TASSADAR_REPLAY_SOURCE_BADGE);
        prepared.snapshot.source_kind = TassadarLabSourceKind::ReplayArtifact;
        prepared.snapshot.replay_id = Some(replay_id);
        prepared.snapshot.artifact_ref =
            Some(String::from(TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF));
        prepared.snapshot.events.insert(
            0,
            format!(
                "replay root={} case={}",
                TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF, case.name
            ),
        );
        Ok(prepared)
    }

    fn prepare_replay_acceptance_report(
        &self,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        let report: TassadarAcceptanceReport =
            read_repo_json(TASSADAR_ACCEPTANCE_REPORT_REF, "tassadar_acceptance_report")?;
        let verdicts = [
            ("research_only", &report.research_only),
            ("compiled_exact", &report.compiled_exact),
            ("learned_bounded", &report.learned_bounded),
            (
                "fast_path_declared_workload_exact",
                &report.fast_path_declared_workload_exact,
            ),
            ("compiled_article_class", &report.compiled_article_class),
            ("learned_article_class", &report.learned_article_class),
            ("article_closure", &report.article_closure),
        ];
        let mut events = vec![format!(
            "allowed_claim_classes={}",
            join_claim_classes(report.allowed_claim_classes.as_slice())
        )];
        for (verdict_id, verdict) in verdicts {
            events.push(format!(
                "{}={} // {}",
                verdict_id, verdict.passed, verdict.detail
            ));
        }
        let snapshot = TassadarLabSnapshot {
            schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
            source_badge: String::from(TASSADAR_REPLAY_SOURCE_BADGE),
            source_kind: TassadarLabSourceKind::ReplayArtifact,
            replay_id: Some(TassadarLabReplayId::AcceptanceReport),
            family_label: String::from("Acceptance report"),
            subject_label: String::from("Tassadar acceptance and article-closure truth"),
            status_label: if report.current_truth_holds {
                String::from("acceptance green")
            } else {
                String::from("acceptance red")
            },
            detail_label: format!(
                "article parity language allowed={} // current_truth_holds={}",
                report.article_parity_language_allowed, report.current_truth_holds
            ),
            artifact_ref: Some(String::from(TASSADAR_ACCEPTANCE_REPORT_REF)),
            benchmark_identity: None,
            proof_identity: None,
            runtime_capability: None,
            requested_decode_mode: None,
            effective_decode_mode: None,
            route_state_label: Some(if report.article_parity_language_allowed {
                String::from("article parity allowed")
            } else {
                String::from("article parity blocked")
            }),
            route_detail: Some(report.article_closure.detail.clone()),
            program_id: None,
            wasm_profile_id: None,
            readable_log: None,
            token_trace: None,
            final_outputs: Vec::new(),
            metric_chips: vec![
                chip(
                    "Allowed claims",
                    report.allowed_claim_classes.len().to_string(),
                    if report.allowed_claim_classes.is_empty() {
                        "red"
                    } else {
                        "green"
                    },
                ),
                chip(
                    "Disallowed claims",
                    report.disallowed_claim_classes.len().to_string(),
                    if report.disallowed_claim_classes.is_empty() {
                        "green"
                    } else {
                        "amber"
                    },
                ),
                chip(
                    "Article parity",
                    yes_no(report.article_parity_language_allowed),
                    if report.article_parity_language_allowed {
                        "green"
                    } else {
                        "red"
                    },
                ),
                chip(
                    "Current truth",
                    yes_no(report.current_truth_holds),
                    if report.current_truth_holds {
                        "green"
                    } else {
                        "red"
                    },
                ),
            ],
            fact_lines: vec![
                fact("Checker", report.checker_command),
                fact("Report ref", report.report_ref),
                fact("Fixture root", report.fixture_root),
                fact("Article closure", report.article_closure.detail),
                fact(
                    "Allowed claim classes",
                    join_claim_classes(report.allowed_claim_classes.as_slice()),
                ),
            ],
            events: events.clone(),
        };
        Ok(TassadarLabPreparedView {
            snapshot,
            updates: status_line_updates(events, snapshot_terminal_label(true)),
        })
    }

    fn prepare_replay_compiled_article_closure_report(
        &self,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        let report: TassadarCompiledArticleClosureReport = read_repo_json(
            TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF,
            "tassadar_compiled_article_closure_report",
        )?;
        let mut events = vec![format!(
            "required_workload_families={}",
            report.required_workload_families.join(", ")
        )];
        for requirement in &report.requirements {
            events.push(format!(
                "{}={} // {}",
                requirement.requirement_id, requirement.passed, requirement.detail
            ));
        }
        if !report.missing_requirements.is_empty() {
            events.push(format!(
                "missing_requirements={}",
                report.missing_requirements.join(", ")
            ));
        }
        let passed = report.passed;
        let snapshot = TassadarLabSnapshot {
            schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
            source_badge: String::from(TASSADAR_REPLAY_SOURCE_BADGE),
            source_kind: TassadarLabSourceKind::ReplayArtifact,
            replay_id: Some(TassadarLabReplayId::CompiledArticleClosureReport),
            family_label: String::from("Compiled article closure report"),
            subject_label: String::from("Compiled article-class closure"),
            status_label: if passed {
                String::from("compiled closure green")
            } else {
                String::from("compiled closure red")
            },
            detail_label: report.detail.clone(),
            artifact_ref: Some(String::from(TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF)),
            benchmark_identity: None,
            proof_identity: None,
            runtime_capability: None,
            requested_decode_mode: None,
            effective_decode_mode: None,
            route_state_label: Some(if passed {
                String::from("requirements cleared")
            } else {
                String::from("requirements blocked")
            }),
            route_detail: Some(report.detail.clone()),
            program_id: None,
            wasm_profile_id: None,
            readable_log: None,
            token_trace: None,
            final_outputs: Vec::new(),
            metric_chips: vec![
                chip(
                    "Requirements",
                    report.requirements.len().to_string(),
                    "blue",
                ),
                chip(
                    "Missing",
                    report.missing_requirements.len().to_string(),
                    if report.missing_requirements.is_empty() {
                        "green"
                    } else {
                        "red"
                    },
                ),
                chip(
                    "Article roots",
                    report.article_artifact_roots.len().to_string(),
                    "blue",
                ),
                chip(
                    "Passed",
                    yes_no(passed),
                    if passed { "green" } else { "red" },
                ),
            ],
            fact_lines: vec![
                fact("Checker", report.checker_command),
                fact("Report ref", report.report_ref),
                fact(
                    "Required workload families",
                    report.required_workload_families.join(", "),
                ),
                fact(
                    "Article artifact roots",
                    report.article_artifact_roots.join(", "),
                ),
                fact("Bounded proxy roots", report.bounded_proxy_roots.join(", ")),
            ],
            events: events.clone(),
        };
        Ok(TassadarLabPreparedView {
            snapshot,
            updates: status_line_updates(events, snapshot_terminal_label(passed)),
        })
    }

    fn prepare_replay_learned_horizon_policy_report(
        &self,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        let report: TassadarLearnedLongHorizonPolicyReport = read_repo_json(
            TASSADAR_LEARNED_HORIZON_POLICY_REPORT_REF,
            "tassadar_learned_horizon_policy_report",
        )?;
        let mut events = vec![
            format!("guard_status={:?}", report.guard_status),
            format!("benchmark_status={:?}", report.benchmark_status),
            format!(
                "sudoku_9x9_full_sequence_fits={}",
                report.sudoku_9x9_full_sequence_fits_model_context
            ),
            format!(
                "hungarian_v0_full_sequence_fits={}",
                report.hungarian_v0_full_sequence_fits_model_context
            ),
        ];
        if let Some(ref detail) = report.refusal_detail {
            events.push(detail.clone());
        }
        if let Some(ref requirement) = report.replacement_requirement {
            events.push(requirement.clone());
        }
        let exact_guard = matches!(
            report.guard_status,
            psionic_research::TassadarLearnedLongHorizonGuardStatus::ExactBenchmarkLanded
        );
        let snapshot = TassadarLabSnapshot {
            schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
            source_badge: String::from(TASSADAR_REPLAY_SOURCE_BADGE),
            source_kind: TassadarLabSourceKind::ReplayArtifact,
            replay_id: Some(TassadarLabReplayId::LearnedHorizonPolicyReport),
            family_label: String::from("Learned horizon policy report"),
            subject_label: String::from("Learned long-horizon guardrail"),
            status_label: if exact_guard {
                String::from("learned horizon exact")
            } else {
                String::from("learned horizon refusal")
            },
            detail_label: report
                .refusal_detail
                .clone()
                .unwrap_or_else(|| String::from("exact learned long-horizon benchmark landed")),
            artifact_ref: Some(String::from(TASSADAR_LEARNED_HORIZON_POLICY_REPORT_REF)),
            benchmark_identity: None,
            proof_identity: None,
            runtime_capability: None,
            requested_decode_mode: None,
            effective_decode_mode: None,
            route_state_label: Some(match report.guard_status {
                psionic_research::TassadarLearnedLongHorizonGuardStatus::ExactBenchmarkLanded => {
                    String::from("exact benchmark landed")
                }
                psionic_research::TassadarLearnedLongHorizonGuardStatus::ExplicitRefusalPolicy => {
                    String::from("explicit refusal policy")
                }
            }),
            route_detail: report.refusal_detail.clone(),
            program_id: None,
            wasm_profile_id: None,
            readable_log: None,
            token_trace: None,
            final_outputs: Vec::new(),
            metric_chips: vec![
                chip(
                    "Guard",
                    format!("{:?}", report.guard_status).to_ascii_lowercase(),
                    if exact_guard { "green" } else { "amber" },
                ),
                chip(
                    "Article floor",
                    report.article_class_trace_step_floor.to_string(),
                    "blue",
                ),
                chip(
                    "9x9 fits",
                    yes_no(report.sudoku_9x9_full_sequence_fits_model_context),
                    if report.sudoku_9x9_full_sequence_fits_model_context {
                        "green"
                    } else {
                        "red"
                    },
                ),
                chip(
                    "Hungarian fits",
                    yes_no(report.hungarian_v0_full_sequence_fits_model_context),
                    if report.hungarian_v0_full_sequence_fits_model_context {
                        "green"
                    } else {
                        "red"
                    },
                ),
            ],
            fact_lines: vec![
                fact(
                    "Enforced claim class",
                    format!("{:?}", report.enforced_claim_class),
                ),
                fact("Bounded green artifact", report.bounded_green_artifact_ref),
                fact("9x9 scope", report.sudoku_9x9_scope_statement),
                fact("Hungarian verdict", report.hungarian_v0_verdict),
                fact(
                    "Refusal reasons",
                    report
                        .refusal_reasons
                        .iter()
                        .map(|reason| format!("{reason:?}").to_ascii_lowercase())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ],
            events: events.clone(),
        };
        Ok(TassadarLabPreparedView {
            snapshot,
            updates: status_line_updates(events, snapshot_terminal_label(exact_guard)),
        })
    }

    fn prepare_replay_learned_promotion_gate_report(
        &self,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        let report: TassadarExecutorPromotionGateReport = read_repo_json(
            TASSADAR_LEARNED_PROMOTION_GATE_REPORT_REF,
            "tassadar_executor_promotion_gate_report",
        )?;
        let mut events = vec![
            format!(
                "first_target_exactness_bps={}",
                report.first_target_exactness_bps
            ),
            format!(
                "first_32_token_exactness_bps={}",
                report.first_32_token_exactness_bps
            ),
            format!("exact_trace_case_count={}", report.exact_trace_case_count),
        ];
        for failure in &report.failures {
            events.push(format!(
                "{:?} actual={} required={}",
                failure.kind, failure.actual, failure.required
            ));
        }
        let passed = report.passed;
        let snapshot = TassadarLabSnapshot {
            schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
            source_badge: String::from(TASSADAR_REPLAY_SOURCE_BADGE),
            source_kind: TassadarLabSourceKind::ReplayArtifact,
            replay_id: Some(TassadarLabReplayId::LearnedPromotionGateV3),
            family_label: String::from("Learned promotion gate"),
            subject_label: format!("Learned 4x4 promotion gate for {}", report.run_id),
            status_label: if passed {
                String::from("promotion green")
            } else {
                String::from("promotion red")
            },
            detail_label: if passed {
                String::from("bounded learned 4x4 lane clears the exact validation gate")
            } else {
                String::from("bounded learned 4x4 lane still fails the exact validation gate")
            },
            artifact_ref: Some(String::from(TASSADAR_LEARNED_PROMOTION_GATE_REPORT_REF)),
            benchmark_identity: None,
            proof_identity: None,
            runtime_capability: None,
            requested_decode_mode: None,
            effective_decode_mode: None,
            route_state_label: Some(if passed {
                String::from("gate passed")
            } else {
                String::from("gate failed")
            }),
            route_detail: Some(format!(
                "checkpoint={} stage={}",
                report.checkpoint_id, report.selected_stage_id
            )),
            program_id: None,
            wasm_profile_id: None,
            readable_log: None,
            token_trace: None,
            final_outputs: Vec::new(),
            metric_chips: vec![
                chip(
                    "First target",
                    report.first_target_exactness_bps.to_string(),
                    if report.first_target_exactness_bps == 10_000 {
                        "green"
                    } else {
                        "red"
                    },
                ),
                chip(
                    "First 32",
                    report.first_32_token_exactness_bps.to_string(),
                    if report.first_32_token_exactness_bps > 9_000 {
                        "green"
                    } else {
                        "red"
                    },
                ),
                chip(
                    "Exact traces",
                    report.exact_trace_case_count.to_string(),
                    if report.exact_trace_case_count > 0 {
                        "green"
                    } else {
                        "red"
                    },
                ),
                chip(
                    "Passed",
                    yes_no(passed),
                    if passed { "green" } else { "red" },
                ),
            ],
            fact_lines: vec![
                fact("Run id", report.run_id),
                fact("Trainable surface", report.trainable_surface),
                fact("Checkpoint id", report.checkpoint_id),
                fact("Selected stage", report.selected_stage_id),
            ],
            events: events.clone(),
        };
        Ok(TassadarLabPreparedView {
            snapshot,
            updates: status_line_updates(events, snapshot_terminal_label(passed)),
        })
    }

    fn prepare_replay_learned_sudoku_9x9_fit_report(
        &self,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        let report: TassadarExecutorSequenceFitReport = read_repo_json(
            TASSADAR_LEARNED_9X9_SEQUENCE_FIT_REPORT_REF,
            "tassadar_executor_sequence_fit_report",
        )?;
        let mut events = vec![
            format!(
                "full_sequence_fits={}",
                report.full_sequence_fits_model_context
            ),
            format!(
                "context_overflow_max={}",
                report.full_sequence_context_overflow_max
            ),
            report.scope_statement.clone(),
            report.outcome_statement.clone(),
        ];
        events.extend(report.blocking_reasons.iter().cloned());
        let fits = report.full_sequence_fits_model_context;
        let snapshot = TassadarLabSnapshot {
            schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
            source_badge: String::from(TASSADAR_REPLAY_SOURCE_BADGE),
            source_kind: TassadarLabSourceKind::ReplayArtifact,
            replay_id: Some(TassadarLabReplayId::LearnedSudoku9x9FitReport),
            family_label: String::from("Learned 9x9 fit report"),
            subject_label: format!("Learned 9x9 fit for {}", report.run_id),
            status_label: if fits {
                String::from("9x9 fit green")
            } else {
                String::from("9x9 fit red")
            },
            detail_label: report.scope_statement.clone(),
            artifact_ref: Some(String::from(TASSADAR_LEARNED_9X9_SEQUENCE_FIT_REPORT_REF)),
            benchmark_identity: None,
            proof_identity: None,
            runtime_capability: None,
            requested_decode_mode: None,
            effective_decode_mode: None,
            route_state_label: Some(format!("{:?}", report.fit_disposition).to_ascii_lowercase()),
            route_detail: Some(report.outcome_statement.clone()),
            program_id: None,
            wasm_profile_id: None,
            readable_log: None,
            token_trace: None,
            final_outputs: Vec::new(),
            metric_chips: vec![
                chip(
                    "Model max seq",
                    report.model_max_sequence_tokens.to_string(),
                    "blue",
                ),
                chip(
                    "Total tokens max",
                    report.total_token_count_max.to_string(),
                    if fits { "green" } else { "amber" },
                ),
                chip(
                    "Overflow max",
                    report.full_sequence_context_overflow_max.to_string(),
                    if report.full_sequence_context_overflow_max == 0 {
                        "green"
                    } else {
                        "red"
                    },
                ),
                chip("Fits", yes_no(fits), if fits { "green" } else { "red" }),
            ],
            fact_lines: vec![
                fact("Model id", report.model_id),
                fact(
                    "Long trace contract",
                    format!("{:?}", report.long_trace_contract).to_ascii_lowercase(),
                ),
                fact(
                    "Fit disposition",
                    format!("{:?}", report.fit_disposition).to_ascii_lowercase(),
                ),
                fact("Scope", report.scope_statement),
                fact("Outcome", report.outcome_statement),
            ],
            events: events.clone(),
        };
        Ok(TassadarLabPreparedView {
            snapshot,
            updates: status_line_updates(events, snapshot_terminal_label(fits)),
        })
    }

    fn prepare_replay_architecture_comparison_report(
        &self,
    ) -> Result<TassadarLabPreparedView, TassadarLabServiceError> {
        let report: TassadarLabArchitectureComparisonReport = read_repo_json(
            TASSADAR_ARCHITECTURE_COMPARISON_REPORT_REF,
            "tassadar_executor_architecture_comparison_report",
        )?;
        let events = vec![
            format!(
                "hull first_32_bps={} neural_tps={}",
                report
                    .hull_specialized_lookup
                    .correctness
                    .first_32_token_exactness_bps,
                report
                    .hull_specialized_lookup
                    .speed
                    .neural_tokens_per_second
            ),
            format!(
                "sparse first_32_bps={} neural_tps={}",
                report
                    .sparse_lookup_baseline
                    .correctness
                    .first_32_token_exactness_bps,
                report.sparse_lookup_baseline.speed.neural_tokens_per_second
            ),
            format!(
                "hybrid first_32_bps={} neural_tps={}",
                report
                    .hybrid_attention_baseline
                    .correctness
                    .first_32_token_exactness_bps,
                report
                    .hybrid_attention_baseline
                    .speed
                    .neural_tokens_per_second
            ),
            format!(
                "recurrent first_32_bps={} neural_tps={}",
                report
                    .recurrent_windowed_baseline
                    .correctness
                    .first_32_token_exactness_bps,
                report
                    .recurrent_windowed_baseline
                    .speed
                    .neural_tokens_per_second
            ),
            format!(
                "sparse_matches_hull={}",
                report.sparse_matches_hull_exactness
            ),
            format!(
                "hybrid_matches_or_beats_hull={}",
                report.hybrid_matches_or_beats_hull_exactness
            ),
            format!(
                "recurrent_changes_long_trace_contract={}",
                report.recurrent_changes_long_trace_contract
            ),
        ];
        let snapshot = TassadarLabSnapshot {
            schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
            source_badge: String::from(TASSADAR_REPLAY_SOURCE_BADGE),
            source_kind: TassadarLabSourceKind::ReplayArtifact,
            replay_id: Some(TassadarLabReplayId::ArchitectureComparisonV12),
            family_label: String::from("Architecture comparison"),
            subject_label: String::from("Hull, sparse, hybrid, and recurrent learned families"),
            status_label: if report.hybrid_matches_or_beats_hull_exactness {
                String::from("hybrid matches hull")
            } else {
                String::from("hybrid trails hull")
            },
            detail_label: report.summary.clone(),
            artifact_ref: Some(String::from(TASSADAR_ARCHITECTURE_COMPARISON_REPORT_REF)),
            benchmark_identity: None,
            proof_identity: None,
            runtime_capability: None,
            requested_decode_mode: None,
            effective_decode_mode: None,
            route_state_label: Some(if report.recurrent_changes_long_trace_contract {
                String::from("recurrent contract changed")
            } else {
                String::from("same long-trace contract")
            }),
            route_detail: Some(format!(
                "sparse_matches_hull={} // hybrid_matches_or_beats_hull={} // recurrent_matches_hull={}",
                report.sparse_matches_hull_exactness,
                report.hybrid_matches_or_beats_hull_exactness,
                report.recurrent_matches_hull_exactness
            )),
            program_id: None,
            wasm_profile_id: None,
            readable_log: None,
            token_trace: None,
            final_outputs: Vec::new(),
            metric_chips: vec![
                chip(
                    "Hull first32",
                    report
                        .hull_specialized_lookup
                        .correctness
                        .first_32_token_exactness_bps
                        .to_string(),
                    "blue",
                ),
                chip(
                    "Sparse first32",
                    report
                        .sparse_lookup_baseline
                        .correctness
                        .first_32_token_exactness_bps
                        .to_string(),
                    if report.sparse_matches_hull_exactness {
                        "green"
                    } else {
                        "amber"
                    },
                ),
                chip(
                    "Hybrid first32",
                    report
                        .hybrid_attention_baseline
                        .correctness
                        .first_32_token_exactness_bps
                        .to_string(),
                    if report.hybrid_matches_or_beats_hull_exactness {
                        "green"
                    } else {
                        "red"
                    },
                ),
                chip(
                    "Recurrent contract",
                    yes_no(report.recurrent_changes_long_trace_contract),
                    if report.recurrent_changes_long_trace_contract {
                        "green"
                    } else {
                        "amber"
                    },
                ),
                chip(
                    "Hybrid tps",
                    report
                        .hybrid_attention_baseline
                        .speed
                        .neural_tokens_per_second
                        .to_string(),
                    "blue",
                ),
            ],
            fact_lines: vec![
                fact("Dataset storage key", report.dataset_storage_key),
                fact("Dataset digest", report.dataset_digest),
                fact("Split", report.split),
                fact(
                    "Hull claim boundary",
                    report.hull_specialized_lookup.claim_boundary,
                ),
                fact(
                    "Hybrid claim boundary",
                    report.hybrid_attention_baseline.claim_boundary,
                ),
                fact(
                    "Recurrent claim boundary",
                    report.recurrent_windowed_baseline.claim_boundary,
                ),
                fact("Summary", report.summary.clone()),
            ],
            events: events.clone(),
        };
        Ok(TassadarLabPreparedView {
            snapshot,
            updates: status_line_updates(
                events,
                if report.hybrid_matches_or_beats_hull_exactness {
                    String::from("hybrid family matches the current hull exactness bar")
                } else {
                    String::from("hybrid family remains below the current hull exactness bar")
                },
            ),
        })
    }
}

fn canonical_tassadar_lab_replay_catalog() -> Vec<TassadarLabReplayCatalogEntry> {
    [
        TassadarLabReplayId::ArticleSessionDirectMemoryHeavy,
        TassadarLabReplayId::ArticleSessionFallbackBranchHeavy,
        TassadarLabReplayId::ArticleSessionRefusalNonArticle,
        TassadarLabReplayId::ArticleHybridDelegatedMemoryHeavy,
        TassadarLabReplayId::ArticleHybridFallbackBranchHeavy,
        TassadarLabReplayId::ArticleHybridRefusalOverBudget,
        TassadarLabReplayId::AcceptanceReport,
        TassadarLabReplayId::CompiledArticleClosureReport,
        TassadarLabReplayId::LearnedHorizonPolicyReport,
        TassadarLabReplayId::LearnedPromotionGateV3,
        TassadarLabReplayId::LearnedSudoku9x9FitReport,
        TassadarLabReplayId::ArchitectureComparisonV12,
    ]
    .into_iter()
    .map(|replay_id| TassadarLabReplayCatalogEntry {
        replay_id,
        family_label: String::from(replay_id.family_label()),
        label: String::from(replay_id.label()),
        artifact_ref: String::from(replay_id.artifact_ref()),
        description: String::from(replay_id.description()),
    })
    .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TassadarLabArticleExecutorSessionArtifactCase {
    name: String,
    request: TassadarArticleExecutorSessionRequest,
    outcome: TassadarArticleExecutorSessionOutcome,
    stream_events: Vec<TassadarArticleExecutorSessionStreamEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TassadarLabArticleExecutorSessionArtifact {
    schema_version: u16,
    product_id: String,
    benchmark_report_ref: String,
    cases: Vec<TassadarLabArticleExecutorSessionArtifactCase>,
    artifact_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TassadarLabArticleHybridWorkflowArtifactCase {
    name: String,
    request: TassadarArticleHybridWorkflowRequest,
    outcome: TassadarArticleHybridWorkflowOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct TassadarLabArticleHybridWorkflowArtifact {
    schema_version: u16,
    product_id: String,
    benchmark_report_ref: String,
    cases: Vec<TassadarLabArticleHybridWorkflowArtifactCase>,
    artifact_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarLabArchitectureComparisonCorrectness {
    first_32_token_exactness_bps: u32,
    exact_trace_case_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarLabArchitectureComparisonSpeed {
    neural_tokens_per_second: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarLabArchitectureComparisonFamily {
    model_id: String,
    claim_boundary: String,
    architecture_identity: String,
    correctness: TassadarLabArchitectureComparisonCorrectness,
    speed: TassadarLabArchitectureComparisonSpeed,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarLabArchitectureComparisonReport {
    dataset_storage_key: String,
    dataset_digest: String,
    split: String,
    prompt_window_token_cap: u32,
    target_token_cap: u32,
    hull_specialized_lookup: TassadarLabArchitectureComparisonFamily,
    sparse_lookup_baseline: TassadarLabArchitectureComparisonFamily,
    hybrid_attention_baseline: TassadarLabArchitectureComparisonFamily,
    recurrent_windowed_baseline: TassadarLabArchitectureComparisonFamily,
    sparse_matches_hull_exactness: bool,
    hybrid_matches_or_beats_hull_exactness: bool,
    recurrent_matches_hull_exactness: bool,
    recurrent_changes_long_trace_contract: bool,
    summary: String,
    report_digest: String,
}

fn snapshot_from_article_executor_session(
    request: &TassadarArticleExecutorSessionRequest,
    outcome: &TassadarArticleExecutorSessionOutcome,
) -> TassadarLabSnapshot {
    match outcome {
        TassadarArticleExecutorSessionOutcome::Completed { response } => {
            let selection = &response.executor_response.execution_report.selection;
            TassadarLabSnapshot {
                schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
                source_badge: String::from(TASSADAR_LIVE_SOURCE_BADGE),
                source_kind: TassadarLabSourceKind::LiveArticleSession,
                replay_id: None,
                family_label: String::from("Article session"),
                subject_label: format!(
                    "{} // {}",
                    response.benchmark_identity.workload_family,
                    response.benchmark_identity.case_id
                ),
                status_label: format!("{} exact", selection_state_label(selection.selection_state)),
                detail_label: selection.detail.clone(),
                artifact_ref: None,
                benchmark_identity: Some(response.benchmark_identity.clone()),
                proof_identity: Some(response.proof_identity.clone()),
                runtime_capability: Some(response.executor_response.runtime_capability.clone()),
                requested_decode_mode: Some(request.requested_decode_mode),
                effective_decode_mode: selection.effective_decode_mode,
                route_state_label: Some(
                    selection_state_label(selection.selection_state).to_string(),
                ),
                route_detail: Some(selection.detail.clone()),
                program_id: Some(response.benchmark_identity.program_id.clone()),
                wasm_profile_id: Some(response.benchmark_identity.wasm_profile_id.clone()),
                readable_log: Some(response.readable_log.clone()),
                token_trace: Some(response.token_trace.clone()),
                final_outputs: response.final_outputs().to_vec(),
                metric_chips: vec![
                    chip(
                        "State",
                        selection_state_label(selection.selection_state),
                        selection_tone(selection.selection_state),
                    ),
                    chip(
                        "Requested decode",
                        format!("{:?}", request.requested_decode_mode).to_ascii_lowercase(),
                        "blue",
                    ),
                    chip(
                        "Effective decode",
                        selection.effective_decode_mode.map_or_else(
                            || String::from("none"),
                            |mode| format!("{mode:?}").to_ascii_lowercase(),
                        ),
                        if selection.is_refused() {
                            "red"
                        } else {
                            "green"
                        },
                    ),
                    chip(
                        "Outputs",
                        response.final_outputs().len().to_string(),
                        "blue",
                    ),
                ],
                fact_lines: vec![
                    fact("Request id", request.request_id.clone()),
                    fact(
                        "Case summary",
                        response.benchmark_identity.case_summary.clone(),
                    ),
                    fact(
                        "Trace artifact",
                        response.proof_identity.trace_artifact_id.clone(),
                    ),
                    fact("Trace digest", response.proof_identity.trace_digest.clone()),
                    fact(
                        "Runtime backend",
                        response
                            .executor_response
                            .runtime_capability
                            .runtime_backend
                            .clone(),
                    ),
                ],
                events: vec![
                    format!(
                        "selection={} // {}",
                        selection_state_label(selection.selection_state),
                        selection.detail
                    ),
                    format!(
                        "proof trace_digest={}",
                        response.proof_identity.trace_digest
                    ),
                    format!("outputs={:?}", response.final_outputs()),
                ],
            }
        }
        TassadarArticleExecutorSessionOutcome::Refused { refusal } => {
            let selection = refusal.selection.as_ref();
            TassadarLabSnapshot {
                schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
                source_badge: String::from(TASSADAR_LIVE_SOURCE_BADGE),
                source_kind: TassadarLabSourceKind::LiveArticleSession,
                replay_id: None,
                family_label: String::from("Article session"),
                subject_label: refusal.benchmark_identity.as_ref().map_or_else(
                    || String::from("Article session refusal"),
                    |identity| format!("{} // {}", identity.workload_family, identity.case_id),
                ),
                status_label: String::from("refused"),
                detail_label: refusal.detail.clone(),
                artifact_ref: None,
                benchmark_identity: refusal.benchmark_identity.clone(),
                proof_identity: None,
                runtime_capability: Some(refusal.runtime_capability.clone()),
                requested_decode_mode: Some(request.requested_decode_mode),
                effective_decode_mode: selection.and_then(|value| value.effective_decode_mode),
                route_state_label: Some(String::from("refused")),
                route_detail: selection.map_or_else(
                    || Some(refusal.detail.clone()),
                    |value| Some(value.detail.clone()),
                ),
                program_id: refusal
                    .benchmark_identity
                    .as_ref()
                    .map(|identity| identity.program_id.clone()),
                wasm_profile_id: refusal
                    .benchmark_identity
                    .as_ref()
                    .map(|identity| identity.wasm_profile_id.clone()),
                readable_log: None,
                token_trace: None,
                final_outputs: Vec::new(),
                metric_chips: vec![
                    chip("State", "refused", "red"),
                    chip(
                        "Requested decode",
                        format!("{:?}", request.requested_decode_mode).to_ascii_lowercase(),
                        "blue",
                    ),
                    chip(
                        "Effective decode",
                        selection
                            .and_then(|value| value.effective_decode_mode)
                            .map_or_else(
                                || String::from("none"),
                                |mode| format!("{mode:?}").to_ascii_lowercase(),
                            ),
                        "red",
                    ),
                    chip(
                        "Supports executor",
                        yes_no(refusal.runtime_capability.supports_executor_trace),
                        if refusal.runtime_capability.supports_executor_trace {
                            "amber"
                        } else {
                            "red"
                        },
                    ),
                ],
                fact_lines: vec![
                    fact("Request id", request.request_id.clone()),
                    fact("Detail", refusal.detail.clone()),
                    fact(
                        "Runtime backend",
                        refusal.runtime_capability.runtime_backend.clone(),
                    ),
                ],
                events: vec![refusal.detail.clone()],
            }
        }
    }
}

fn snapshot_from_article_hybrid_workflow(
    request: &TassadarArticleHybridWorkflowRequest,
    outcome: &TassadarArticleHybridWorkflowOutcome,
) -> Result<TassadarLabSnapshot, TassadarLabServiceError> {
    match outcome {
        TassadarArticleHybridWorkflowOutcome::Completed { response } => {
            let decision = &response.planner_response.routing_decision;
            let case = article_case_by_id(response.benchmark_identity.case_id.as_str())
                .ok_or_else(|| TassadarLabServiceError::MissingArticleCase {
                    case_id: response.benchmark_identity.case_id.clone(),
                })?;
            let readable_log = build_article_readable_log_excerpt(
                &case,
                &response
                    .planner_response
                    .executor_response
                    .execution_report
                    .execution,
            );
            let token_trace = build_article_token_trace_excerpt(
                &case,
                &response
                    .planner_response
                    .executor_response
                    .execution_report
                    .execution,
            );
            Ok(TassadarLabSnapshot {
                schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
                source_badge: String::from(TASSADAR_LIVE_SOURCE_BADGE),
                source_kind: TassadarLabSourceKind::LiveArticleHybridWorkflow,
                replay_id: None,
                family_label: String::from("Article hybrid workflow"),
                subject_label: format!(
                    "{} // {}",
                    response.benchmark_identity.workload_family,
                    response.benchmark_identity.case_id
                ),
                status_label: String::from("delegated exact"),
                detail_label: decision.detail.clone(),
                artifact_ref: None,
                benchmark_identity: Some(response.benchmark_identity.clone()),
                proof_identity: Some(response.proof_identity.clone()),
                runtime_capability: Some(decision.runtime_capability.clone()),
                requested_decode_mode: Some(request.requested_decode_mode),
                effective_decode_mode: decision.effective_decode_mode,
                route_state_label: Some(route_state_label(decision.route_state).to_string()),
                route_detail: Some(decision.detail.clone()),
                program_id: Some(response.benchmark_identity.program_id.clone()),
                wasm_profile_id: Some(response.benchmark_identity.wasm_profile_id.clone()),
                readable_log: Some(readable_log),
                token_trace: Some(token_trace),
                final_outputs: response
                    .planner_response
                    .executor_response
                    .final_outputs()
                    .to_vec(),
                metric_chips: vec![
                    chip("Route", route_state_label(decision.route_state), "green"),
                    chip(
                        "Requested decode",
                        format!("{:?}", request.requested_decode_mode).to_ascii_lowercase(),
                        "blue",
                    ),
                    chip(
                        "Effective decode",
                        decision.effective_decode_mode.map_or_else(
                            || String::from("none"),
                            |mode| format!("{mode:?}").to_ascii_lowercase(),
                        ),
                        "green",
                    ),
                    chip(
                        "Outputs",
                        response
                            .planner_response
                            .executor_response
                            .final_outputs()
                            .len()
                            .to_string(),
                        "blue",
                    ),
                ],
                fact_lines: vec![
                    fact("Request id", request.request_id.clone()),
                    fact("Planner session id", request.planner_session_id.clone()),
                    fact("Workflow step id", request.workflow_step_id.clone()),
                    fact("Routing digest", decision.routing_digest.clone()),
                    fact("Trace digest", response.proof_identity.trace_digest.clone()),
                ],
                events: vec![
                    format!(
                        "route={} // {}",
                        route_state_label(decision.route_state),
                        decision.detail
                    ),
                    format!(
                        "proof trace_digest={}",
                        response.proof_identity.trace_digest
                    ),
                    format!(
                        "outputs={:?}",
                        response.planner_response.executor_response.final_outputs()
                    ),
                ],
            })
        }
        TassadarArticleHybridWorkflowOutcome::Fallback { fallback } => {
            let decision = &fallback.planner_fallback.routing_decision;
            Ok(TassadarLabSnapshot {
                schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
                source_badge: String::from(TASSADAR_LIVE_SOURCE_BADGE),
                source_kind: TassadarLabSourceKind::LiveArticleHybridWorkflow,
                replay_id: None,
                family_label: String::from("Article hybrid workflow"),
                subject_label: fallback.benchmark_identity.as_ref().map_or_else(
                    || String::from("Article hybrid workflow fallback"),
                    |identity| format!("{} // {}", identity.workload_family, identity.case_id),
                ),
                status_label: String::from("planner fallback"),
                detail_label: fallback.planner_fallback.fallback_summary.clone(),
                artifact_ref: None,
                benchmark_identity: fallback.benchmark_identity.clone(),
                proof_identity: None,
                runtime_capability: Some(decision.runtime_capability.clone()),
                requested_decode_mode: Some(request.requested_decode_mode),
                effective_decode_mode: decision.effective_decode_mode,
                route_state_label: Some(route_state_label(decision.route_state).to_string()),
                route_detail: Some(decision.detail.clone()),
                program_id: fallback
                    .benchmark_identity
                    .as_ref()
                    .map(|identity| identity.program_id.clone()),
                wasm_profile_id: fallback
                    .benchmark_identity
                    .as_ref()
                    .map(|identity| identity.wasm_profile_id.clone()),
                readable_log: None,
                token_trace: None,
                final_outputs: Vec::new(),
                metric_chips: vec![
                    chip("Route", route_state_label(decision.route_state), "amber"),
                    chip(
                        "Requested decode",
                        format!("{:?}", request.requested_decode_mode).to_ascii_lowercase(),
                        "blue",
                    ),
                    chip(
                        "Effective decode",
                        decision.effective_decode_mode.map_or_else(
                            || String::from("none"),
                            |mode| format!("{mode:?}").to_ascii_lowercase(),
                        ),
                        "amber",
                    ),
                    chip(
                        "Executor refusal",
                        yes_no(fallback.planner_fallback.executor_refusal.is_some()),
                        "amber",
                    ),
                ],
                fact_lines: vec![
                    fact("Request id", request.request_id.clone()),
                    fact("Workflow step id", request.workflow_step_id.clone()),
                    fact(
                        "Fallback summary",
                        fallback.planner_fallback.fallback_summary.clone(),
                    ),
                    fact("Routing digest", decision.routing_digest.clone()),
                ],
                events: vec![
                    format!(
                        "route={} // {}",
                        route_state_label(decision.route_state),
                        decision.detail
                    ),
                    fallback.planner_fallback.fallback_summary.clone(),
                ],
            })
        }
        TassadarArticleHybridWorkflowOutcome::Refused { refusal } => {
            let (runtime_capability, route_state_label_value, route_detail) =
                refusal.planner_refusal.as_ref().map_or(
                    (None, String::from("refused"), refusal.detail.clone()),
                    |planner_refusal| {
                        (
                            Some(planner_refusal.routing_decision.runtime_capability.clone()),
                            route_state_label(planner_refusal.routing_decision.route_state)
                                .to_string(),
                            planner_refusal.detail.clone(),
                        )
                    },
                );
            Ok(TassadarLabSnapshot {
                schema_version: TASSADAR_LAB_SNAPSHOT_SCHEMA_VERSION,
                source_badge: String::from(TASSADAR_LIVE_SOURCE_BADGE),
                source_kind: TassadarLabSourceKind::LiveArticleHybridWorkflow,
                replay_id: None,
                family_label: String::from("Article hybrid workflow"),
                subject_label: refusal.benchmark_identity.as_ref().map_or_else(
                    || String::from("Article hybrid workflow refusal"),
                    |identity| format!("{} // {}", identity.workload_family, identity.case_id),
                ),
                status_label: String::from("planner refusal"),
                detail_label: refusal.detail.clone(),
                artifact_ref: None,
                benchmark_identity: refusal.benchmark_identity.clone(),
                proof_identity: None,
                runtime_capability,
                requested_decode_mode: Some(request.requested_decode_mode),
                effective_decode_mode: refusal.planner_refusal.as_ref().and_then(
                    |planner_refusal| planner_refusal.routing_decision.effective_decode_mode,
                ),
                route_state_label: Some(route_state_label_value),
                route_detail: Some(route_detail),
                program_id: refusal
                    .benchmark_identity
                    .as_ref()
                    .map(|identity| identity.program_id.clone()),
                wasm_profile_id: refusal
                    .benchmark_identity
                    .as_ref()
                    .map(|identity| identity.wasm_profile_id.clone()),
                readable_log: None,
                token_trace: None,
                final_outputs: Vec::new(),
                metric_chips: vec![
                    chip("Route", "refused", "red"),
                    chip(
                        "Requested decode",
                        format!("{:?}", request.requested_decode_mode).to_ascii_lowercase(),
                        "blue",
                    ),
                    chip(
                        "Planner refusal",
                        yes_no(refusal.planner_refusal.is_some()),
                        "red",
                    ),
                ],
                fact_lines: vec![
                    fact("Request id", request.request_id.clone()),
                    fact("Workflow step id", request.workflow_step_id.clone()),
                    fact("Detail", refusal.detail.clone()),
                ],
                events: vec![refusal.detail.clone()],
            })
        }
    }
}

fn updates_from_article_hybrid_workflow(
    outcome: &TassadarArticleHybridWorkflowOutcome,
) -> Result<Vec<TassadarLabUpdate>, TassadarLabServiceError> {
    let mut updates = Vec::new();
    match outcome {
        TassadarArticleHybridWorkflowOutcome::Completed { response } => {
            let decision = &response.planner_response.routing_decision;
            let case = article_case_by_id(response.benchmark_identity.case_id.as_str())
                .ok_or_else(|| TassadarLabServiceError::MissingArticleCase {
                    case_id: response.benchmark_identity.case_id.clone(),
                })?;
            let readable_log = build_article_readable_log_excerpt(
                &case,
                &response
                    .planner_response
                    .executor_response
                    .execution_report
                    .execution,
            );
            let token_trace = build_article_token_trace_excerpt(
                &case,
                &response
                    .planner_response
                    .executor_response
                    .execution_report
                    .execution,
            );
            updates.push(TassadarLabUpdate::BenchmarkIdentity {
                benchmark_identity: TassadarArticleBenchmarkIdentityEvent {
                    benchmark_identity: response.benchmark_identity.clone(),
                },
            });
            updates.push(TassadarLabUpdate::Capability {
                runtime_capability: decision.runtime_capability.clone(),
            });
            if let Some(selection) = decision.selection.clone() {
                updates.push(TassadarLabUpdate::Selection { selection });
            }
            updates.push(TassadarLabUpdate::RoutingStatus {
                route_state: route_state_label(decision.route_state).to_string(),
                detail: decision.detail.clone(),
                effective_decode_mode: decision.effective_decode_mode,
            });
            updates.push(TassadarLabUpdate::ProofIdentity {
                proof_identity: TassadarArticleProofIdentityEvent {
                    proof_identity: response.proof_identity.clone(),
                },
            });
            for (line_index, line) in readable_log.lines.into_iter().enumerate() {
                updates.push(TassadarLabUpdate::ReadableLogLine {
                    readable_log_line: TassadarArticleReadableLogLineEvent { line_index, line },
                });
            }
            for (chunk_index, chunk) in token_trace
                .tokens
                .chunks(ARTICLE_EXECUTOR_TOKEN_TRACE_CHUNK_SIZE)
                .enumerate()
            {
                updates.push(TassadarLabUpdate::TokenTraceChunk {
                    token_trace_chunk: TassadarArticleTokenTraceChunkEvent {
                        chunk_index,
                        prompt_token_count: token_trace.prompt_token_count,
                        total_token_count: token_trace.total_token_count,
                        truncated: token_trace.truncated,
                        tokens: chunk.to_vec(),
                    },
                });
            }
            for (ordinal, value) in response
                .planner_response
                .executor_response
                .final_outputs()
                .iter()
                .enumerate()
            {
                updates.push(TassadarLabUpdate::Output {
                    output: TassadarExecutorOutputEvent {
                        ordinal,
                        value: *value,
                    },
                });
            }
            updates.push(TassadarLabUpdate::Terminal {
                status_label: String::from("delegated exact"),
                detail: decision.detail.clone(),
            });
        }
        TassadarArticleHybridWorkflowOutcome::Fallback { fallback } => {
            let decision = &fallback.planner_fallback.routing_decision;
            if let Some(benchmark_identity) = fallback.benchmark_identity.clone() {
                updates.push(TassadarLabUpdate::BenchmarkIdentity {
                    benchmark_identity: TassadarArticleBenchmarkIdentityEvent {
                        benchmark_identity,
                    },
                });
            }
            updates.push(TassadarLabUpdate::Capability {
                runtime_capability: decision.runtime_capability.clone(),
            });
            if let Some(selection) = decision.selection.clone() {
                updates.push(TassadarLabUpdate::Selection { selection });
            }
            updates.push(TassadarLabUpdate::RoutingStatus {
                route_state: route_state_label(decision.route_state).to_string(),
                detail: decision.detail.clone(),
                effective_decode_mode: decision.effective_decode_mode,
            });
            updates.push(TassadarLabUpdate::StatusLine {
                line_index: 0,
                line: fallback.planner_fallback.fallback_summary.clone(),
            });
            updates.push(TassadarLabUpdate::Terminal {
                status_label: String::from("planner fallback"),
                detail: fallback.planner_fallback.fallback_summary.clone(),
            });
        }
        TassadarArticleHybridWorkflowOutcome::Refused { refusal } => {
            if let Some(benchmark_identity) = refusal.benchmark_identity.clone() {
                updates.push(TassadarLabUpdate::BenchmarkIdentity {
                    benchmark_identity: TassadarArticleBenchmarkIdentityEvent {
                        benchmark_identity,
                    },
                });
            }
            if let Some(planner_refusal) = refusal.planner_refusal.as_ref() {
                updates.push(TassadarLabUpdate::Capability {
                    runtime_capability: planner_refusal.routing_decision.runtime_capability.clone(),
                });
                if let Some(selection) = planner_refusal.routing_decision.selection.clone() {
                    updates.push(TassadarLabUpdate::Selection { selection });
                }
                updates.push(TassadarLabUpdate::RoutingStatus {
                    route_state: route_state_label(planner_refusal.routing_decision.route_state)
                        .to_string(),
                    detail: planner_refusal.detail.clone(),
                    effective_decode_mode: planner_refusal.routing_decision.effective_decode_mode,
                });
            }
            updates.push(TassadarLabUpdate::StatusLine {
                line_index: 0,
                line: refusal.detail.clone(),
            });
            updates.push(TassadarLabUpdate::Terminal {
                status_label: String::from("planner refusal"),
                detail: refusal.detail.clone(),
            });
        }
    }
    Ok(updates)
}

fn tassadar_lab_update_from_article_event(
    event: TassadarArticleExecutorSessionStreamEvent,
) -> TassadarLabUpdate {
    match event {
        TassadarArticleExecutorSessionStreamEvent::BenchmarkIdentity { benchmark_identity } => {
            TassadarLabUpdate::BenchmarkIdentity { benchmark_identity }
        }
        TassadarArticleExecutorSessionStreamEvent::Capability { runtime_capability } => {
            TassadarLabUpdate::Capability { runtime_capability }
        }
        TassadarArticleExecutorSessionStreamEvent::Selection { selection } => {
            TassadarLabUpdate::Selection { selection }
        }
        TassadarArticleExecutorSessionStreamEvent::ProofIdentity { proof_identity } => {
            TassadarLabUpdate::ProofIdentity { proof_identity }
        }
        TassadarArticleExecutorSessionStreamEvent::DirectModelWeightExecutionProof {
            direct_model_weight_execution_proof,
        } => TassadarLabUpdate::StatusLine {
            line_index: 0,
            line: format!(
                "direct model-weight proof {}",
                direct_model_weight_execution_proof
                    .proof_receipt
                    .receipt_digest
            ),
        },
        TassadarArticleExecutorSessionStreamEvent::ReadableLogLine { readable_log_line } => {
            TassadarLabUpdate::ReadableLogLine { readable_log_line }
        }
        TassadarArticleExecutorSessionStreamEvent::TokenTraceChunk { token_trace_chunk } => {
            TassadarLabUpdate::TokenTraceChunk { token_trace_chunk }
        }
        TassadarArticleExecutorSessionStreamEvent::Output { output } => {
            TassadarLabUpdate::Output { output }
        }
        TassadarArticleExecutorSessionStreamEvent::Terminal { terminal } => {
            let (status_label, detail) = match terminal.outcome {
                TassadarArticleExecutorSessionOutcome::Completed { response } => (
                    String::from("completed"),
                    response.executor_response.execution_report.selection.detail,
                ),
                TassadarArticleExecutorSessionOutcome::Refused { refusal } => {
                    (String::from("refused"), refusal.detail)
                }
            };
            TassadarLabUpdate::Terminal {
                status_label,
                detail,
            }
        }
    }
}

fn chip(
    label: impl Into<String>,
    value: impl Into<String>,
    tone: impl Into<String>,
) -> TassadarLabMetricChip {
    TassadarLabMetricChip {
        label: label.into(),
        value: value.into(),
        tone: tone.into(),
    }
}

fn fact(label: impl Into<String>, value: impl Into<String>) -> TassadarLabFactLine {
    TassadarLabFactLine {
        label: label.into(),
        value: value.into(),
    }
}

fn status_line_updates(events: Vec<String>, terminal_detail: String) -> Vec<TassadarLabUpdate> {
    let mut updates = events
        .into_iter()
        .enumerate()
        .map(|(line_index, line)| TassadarLabUpdate::StatusLine { line_index, line })
        .collect::<Vec<_>>();
    updates.push(TassadarLabUpdate::Terminal {
        status_label: String::from("replay loaded"),
        detail: terminal_detail,
    });
    updates
}

fn snapshot_terminal_label(passed: bool) -> String {
    if passed {
        String::from("replay confirms a green posture")
    } else {
        String::from("replay keeps the current red posture explicit")
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn selection_state_label(state: psionic_runtime::TassadarExecutorSelectionState) -> &'static str {
    match state {
        psionic_runtime::TassadarExecutorSelectionState::Direct => "direct",
        psionic_runtime::TassadarExecutorSelectionState::Fallback => "fallback",
        psionic_runtime::TassadarExecutorSelectionState::Refused => "refused",
    }
}

fn selection_tone(state: psionic_runtime::TassadarExecutorSelectionState) -> &'static str {
    match state {
        psionic_runtime::TassadarExecutorSelectionState::Direct => "green",
        psionic_runtime::TassadarExecutorSelectionState::Fallback => "amber",
        psionic_runtime::TassadarExecutorSelectionState::Refused => "red",
    }
}

fn route_state_label(state: TassadarPlannerRouteState) -> &'static str {
    match state {
        TassadarPlannerRouteState::Delegated => "delegated",
        TassadarPlannerRouteState::PlannerFallback => "planner_fallback",
        TassadarPlannerRouteState::Refused => "refused",
    }
}

fn join_claim_classes(values: &[psionic_runtime::TassadarClaimClass]) -> String {
    values
        .iter()
        .map(|claim_class| format!("{claim_class:?}").to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(", ")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn read_repo_json<T>(
    repo_relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarLabServiceError>
where
    T: for<'de> Deserialize<'de>,
{
    let path = repo_root().join(repo_relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarLabServiceError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| TassadarLabServiceError::Deserialize {
        artifact_kind: String::from(artifact_kind),
        path: path.display().to_string(),
        error,
    })
}

fn format_event(event: &TassadarTraceEvent) -> String {
    match event {
        TassadarTraceEvent::Nop => String::from("nop"),
        TassadarTraceEvent::ConstPush { value } => format!("const_push value={value}"),
        TassadarTraceEvent::LocalGet { local, value } => {
            format!("local_get local={local} value={value}")
        }
        TassadarTraceEvent::LocalSet { local, value } => {
            format!("local_set local={local} value={value}")
        }
        TassadarTraceEvent::LocalTee { local, value } => {
            format!("local_tee local={local} value={value}")
        }
        TassadarTraceEvent::Drop { value } => format!("drop value={value}"),
        TassadarTraceEvent::UnaryOp {
            op,
            operand,
            result,
        } => format!("unary_{op:?} operand={operand} result={result}"),
        TassadarTraceEvent::BinaryOp {
            op,
            left,
            right,
            result,
        } => format!("binary_{op:?} left={left} right={right} result={result}"),
        TassadarTraceEvent::Load { slot, value } => format!("load slot={slot} value={value}"),
        TassadarTraceEvent::Store { slot, value } => format!("store slot={slot} value={value}"),
        TassadarTraceEvent::Branch {
            condition,
            taken,
            target_pc,
        } => format!("branch condition={condition} taken={taken} target_pc={target_pc}"),
        TassadarTraceEvent::Output { value } => format!("output value={value}"),
        TassadarTraceEvent::Return => String::from("return"),
    }
}

fn stream_events_for_outcome(
    runtime_capability: TassadarRuntimeCapabilityReport,
    outcome: TassadarExecutorOutcome,
) -> Vec<TassadarExecutorStreamEvent> {
    let mut events = vec![TassadarExecutorStreamEvent::Capability { runtime_capability }];
    match &outcome {
        TassadarExecutorOutcome::Completed { response } => {
            events.push(TassadarExecutorStreamEvent::Selection {
                selection: response.execution_report.selection.clone(),
            });
            let mut output_ordinal = 0usize;
            for step in &response.execution_report.execution.steps {
                events.push(TassadarExecutorStreamEvent::TraceStep {
                    trace_step: TassadarExecutorTraceStepEvent {
                        step_index: step.step_index as u64,
                        step: step.clone(),
                    },
                });
                if let TassadarTraceEvent::Output { value } = step.event {
                    events.push(TassadarExecutorStreamEvent::Output {
                        output: TassadarExecutorOutputEvent {
                            ordinal: output_ordinal,
                            value,
                        },
                    });
                    output_ordinal += 1;
                }
            }
        }
        TassadarExecutorOutcome::Refused { refusal } => {
            if let Some(selection) = &refusal.selection {
                events.push(TassadarExecutorStreamEvent::Selection {
                    selection: selection.clone(),
                });
            }
        }
    }
    events.push(TassadarExecutorStreamEvent::Terminal {
        terminal: TassadarExecutorTerminalEvent { outcome },
    });
    events
}

#[cfg(test)]
mod tests {
    use super::{
        ARTICLE_EXECUTOR_SESSION_PRODUCT_ID, ARTICLE_HYBRID_WORKFLOW_PRODUCT_ID,
        EXECUTOR_TRACE_PRODUCT_ID, LocalTassadarArticleExecutorSessionService,
        LocalTassadarArticleHybridWorkflowService, LocalTassadarExecutorService,
        LocalTassadarLabService, LocalTassadarPlannerRouter, PLANNER_EXECUTOR_ROUTE_PRODUCT_ID,
        TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF, TassadarArticleExecutorSessionOutcome,
        TassadarArticleExecutorSessionRequest, TassadarArticleExecutorSessionServiceError,
        TassadarArticleExecutorSessionStreamEvent, TassadarArticleHybridWorkflowOutcome,
        TassadarArticleHybridWorkflowRequest, TassadarArticleHybridWorkflowServiceError,
        TassadarExecutorCapabilityPublicationError, TassadarExecutorOutcome,
        TassadarExecutorRequest, TassadarExecutorServiceError, TassadarExecutorStreamEvent,
        TassadarLabPreparedView, TassadarLabReplayId, TassadarLabRequest, TassadarLabSourceKind,
        TassadarLabUpdate, TassadarPlannerExecutorSubproblem, TassadarPlannerFallbackPolicy,
        TassadarPlannerRouteReason, TassadarPlannerRouterError, TassadarPlannerRoutingBudget,
        TassadarPlannerRoutingOutcome, TassadarPlannerRoutingPolicy, TassadarPlannerRoutingRequest,
        TassadarResearchPromotionError, require_tassadar_research_lane_promotion_ready,
    };
    use psionic_models::{
        TassadarArticleTransformer, TassadarExecutorFixture, TassadarWorkloadClass,
    };
    use psionic_research::TassadarPromotionChecklistGateKind;
    use psionic_router::{
        TassadarPlannerExecutorRoutePosture, TassadarPlannerExecutorWasmImportPosture,
    };
    use psionic_runtime::{
        TassadarExecutorDecodeMode, TassadarExecutorSelectionState, TassadarInstruction,
        TassadarProgram, TassadarProgramArtifact, TassadarTraceAbi, TassadarWasmProfile,
        tassadar_article_class_corpus, tassadar_article_hard_sudoku_suite,
        tassadar_hungarian_10x10_corpus, tassadar_validation_corpus,
    };

    fn request_for_case(case_id: &str) -> TassadarExecutorRequest {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let case = tassadar_validation_corpus()
            .into_iter()
            .find(|case| case.case_id == case_id)
            .expect("requested validation case should exist");
        let artifact = TassadarProgramArtifact::fixture_reference(
            format!("artifact://tassadar/{case_id}"),
            &profile,
            &trace_abi,
            case.program,
        )
        .expect("fixture artifact should build");
        TassadarExecutorRequest::new(
            format!("request-{case_id}"),
            artifact,
            TassadarExecutorDecodeMode::HullCache,
        )
        .with_environment_refs(vec![String::from("env.openagents.tassadar.benchmark")])
    }

    fn article_request_for_case(
        case_id: &str,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> TassadarArticleExecutorSessionRequest {
        TassadarArticleExecutorSessionRequest::new(
            format!("article-request-{case_id}-{requested_decode_mode:?}"),
            case_id,
            requested_decode_mode,
        )
    }

    fn article_workflow_request_for_case(
        case_id: &str,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> TassadarArticleHybridWorkflowRequest {
        TassadarArticleHybridWorkflowRequest::new(
            format!("article-workflow-request-{case_id}-{requested_decode_mode:?}"),
            "planner-session-article-alpha",
            "planner-article-fixture-v0",
            format!("workflow-step-{case_id}"),
            "delegate exact article workload into Tassadar",
            case_id,
            requested_decode_mode,
        )
    }

    #[test]
    fn executor_service_capability_publication_serializes_benchmark_gated_matrix() {
        let service = LocalTassadarExecutorService::new()
            .with_fixture(TassadarExecutorFixture::article_i32_compute_v1());
        let publication = service
            .capability_publication(Some(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID))
            .expect("article fixture should publish capability");

        assert_eq!(publication.product_id, EXECUTOR_TRACE_PRODUCT_ID);
        assert_eq!(publication.runtime_capability.runtime_backend, "cpu");
        let encoded = serde_json::to_value(&publication).expect("publication should serialize");
        assert_eq!(
            encoded["model_descriptor"]["model"]["model_id"],
            serde_json::json!(TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID)
        );
        assert_eq!(
            encoded["module_execution_capability"]["runtime_capability"]["supports_call_indirect"],
            serde_json::json!(true)
        );
        assert_eq!(
            encoded["module_execution_capability"]["runtime_capability"]["supports_active_element_segments"],
            serde_json::json!(true)
        );
        assert_eq!(
            encoded["module_execution_capability"]["runtime_capability"]["supports_start_function_instantiation"],
            serde_json::json!(true)
        );
        assert_eq!(
            encoded["module_execution_capability"]["runtime_capability"]["supports_linear_memory"],
            serde_json::json!(true)
        );
        assert_eq!(
            encoded["module_execution_capability"]["runtime_capability"]["supports_active_data_segments"],
            serde_json::json!(true)
        );
        assert_eq!(
            encoded["module_execution_capability"]["runtime_capability"]["supports_memory_grow"],
            serde_json::json!(true)
        );
        assert_eq!(
            encoded["module_execution_capability"]["runtime_capability"]["host_import_boundary"]["unsupported_host_call_refusal"],
            serde_json::json!("unsupported_host_import")
        );
        assert_eq!(
            encoded["rust_article_profile_completeness"]["family_id"],
            serde_json::json!("tassadar.wasm.rust_article_family.v1")
        );
        assert_eq!(
            encoded["rust_article_profile_completeness"]["report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_rust_article_profile_completeness_report.json"
            )
        );
        assert_eq!(
            encoded["generalized_abi_family"]["family_id"],
            serde_json::json!("tassadar.rust_generalized_abi.v1")
        );
        assert_eq!(
            encoded["generalized_abi_family"]["report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_generalized_abi_family_report.json"
            )
        );
        assert_eq!(
            encoded["internal_compute_profile_ladder"]["report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_internal_compute_profile_ladder_report.json"
            )
        );
        assert_eq!(
            encoded["internal_compute_profile_claim_check"]["claim"]["profile_id"],
            serde_json::json!("tassadar.internal_compute.article_closeout.v1")
        );
        assert_eq!(
            encoded["internal_compute_profile_claim_check"]["green"],
            serde_json::json!(true)
        );
        assert_eq!(
            encoded["broad_internal_compute_portability_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_broad_internal_compute_portability_report.json"
            )
        );
        let backend_families = encoded["broad_internal_compute_portability_backend_family_ids"]
            .as_array()
            .expect("backend family ids should encode as an array")
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(backend_families.contains(&serde_json::json!("cpu_reference")));
        assert!(backend_families.contains(&serde_json::json!("metal_served")));
        assert!(backend_families.contains(&serde_json::json!("cuda_served")));
        let toolchain_families = encoded["broad_internal_compute_portability_toolchain_family_ids"]
            .as_array()
            .expect("toolchain family ids should encode as an array")
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(toolchain_families.contains(&serde_json::json!("rustc:wasm32-unknown-unknown")));
        assert!(toolchain_families.contains(&serde_json::json!(
            "rustc:wasm32-unknown-unknown+metal_served"
        )));
        assert!(toolchain_families.contains(&serde_json::json!(
            "rustc:wasm32-unknown-unknown+cuda_served"
        )));
        assert_eq!(
            encoded["broad_internal_compute_acceptance_gate_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_broad_internal_compute_acceptance_gate.json"
            )
        );
        assert_eq!(
            encoded["numeric_portability_report_ref"],
            serde_json::json!("fixtures/tassadar/reports/tassadar_numeric_portability_report.json")
        );
        let numeric_backend_families = encoded["numeric_portability_backend_family_ids"]
            .as_array()
            .expect("numeric backend family ids should encode as an array")
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(numeric_backend_families.contains(&serde_json::json!("cpu_reference")));
        assert!(numeric_backend_families.contains(&serde_json::json!("metal_served")));
        assert!(numeric_backend_families.contains(&serde_json::json!("cuda_served")));
        let numeric_toolchain_families = encoded["numeric_portability_toolchain_family_ids"]
            .as_array()
            .expect("numeric toolchain family ids should encode as an array")
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            numeric_toolchain_families.contains(&serde_json::json!("rustc:wasm32-unknown-unknown"))
        );
        let numeric_profile_ids = encoded["numeric_portability_profile_ids"]
            .as_array()
            .expect("numeric profile ids should encode as an array")
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            numeric_profile_ids
                .contains(&serde_json::json!("tassadar.numeric_profile.f32_only.v1"))
        );
        assert!(numeric_profile_ids.contains(&serde_json::json!(
            "tassadar.numeric_profile.mixed_i32_f32.v1"
        )));
        assert!(numeric_profile_ids.contains(&serde_json::json!(
            "tassadar.numeric_profile.bounded_f64_conversion.v1"
        )));
        assert_eq!(
            encoded["float_profile_acceptance_gate_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_float_profile_acceptance_gate_report.json"
            )
        );
        assert_eq!(
            encoded["float_profile_route_policy_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_float_profile_route_policy_report.json"
            )
        );
        assert_eq!(
            encoded["exception_profile_report_ref"],
            serde_json::json!("fixtures/tassadar/reports/tassadar_exception_profile_report.json")
        );
        assert_eq!(
            encoded["exception_profile_public_profile_ids"],
            serde_json::json!(["tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"])
        );
        assert_eq!(
            encoded["exception_profile_default_served_profile_ids"],
            serde_json::json!([])
        );
        assert_eq!(
            encoded["exception_profile_portability_envelope_ids"],
            serde_json::json!(["cpu_reference_current_host"])
        );
        assert_eq!(
            encoded["simd_profile_report_ref"],
            serde_json::json!("fixtures/tassadar/reports/tassadar_simd_profile_report.json")
        );
        assert_eq!(
            encoded["simd_profile_public_profile_ids"],
            serde_json::json!(["tassadar.proposal_profile.simd_deterministic.v1"])
        );
        assert_eq!(
            encoded["simd_profile_default_served_profile_ids"],
            serde_json::json!([])
        );
        assert_eq!(
            encoded["simd_profile_exact_backend_ids"],
            serde_json::json!(["cpu_reference_current_host"])
        );
        assert_eq!(
            encoded["simd_profile_fallback_backend_ids"],
            serde_json::json!(["metal_served", "cuda_served"])
        );
        assert_eq!(
            encoded["simd_profile_refused_backend_ids"],
            serde_json::json!(["accelerator_specific_unbounded"])
        );
        assert_eq!(
            encoded["session_process_profile_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_session_process_profile_report.json"
            )
        );
        assert_eq!(
            encoded["session_process_route_policy_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_session_process_route_policy_report.json"
            )
        );
        assert_eq!(
            encoded["session_process_public_profile_ids"],
            serde_json::json!(["tassadar.internal_compute.session_process.v1"])
        );
        assert_eq!(
            encoded["session_process_default_served_profile_ids"],
            serde_json::json!([])
        );
        assert_eq!(
            encoded["session_process_routeable_interaction_surface_ids"],
            serde_json::json!(["deterministic_echo_turn_loop", "stateful_counter_turn_loop"])
        );
        assert_eq!(
            encoded["session_process_refused_interaction_surface_ids"],
            serde_json::json!(["open_ended_external_event_stream"])
        );
        assert_eq!(
            encoded["async_lifecycle_profile_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_async_lifecycle_profile_report.json"
            )
        );
        assert_eq!(
            encoded["async_lifecycle_route_policy_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_async_lifecycle_route_policy_report.json"
            )
        );
        assert_eq!(
            encoded["async_lifecycle_public_profile_ids"],
            serde_json::json!(["tassadar.internal_compute.async_lifecycle.v1"])
        );
        assert_eq!(
            encoded["async_lifecycle_default_served_profile_ids"],
            serde_json::json!([])
        );
        assert_eq!(
            encoded["async_lifecycle_routeable_surface_ids"],
            serde_json::json!([
                "interruptible_counter_job",
                "retryable_timeout_search_job",
                "safe_boundary_cancellation_job"
            ])
        );
        assert_eq!(
            encoded["async_lifecycle_refused_surface_ids"],
            serde_json::json!([
                "mid_effect_cancellation",
                "open_ended_external_callback",
                "unbounded_retry_backoff"
            ])
        );
        assert_eq!(
            encoded["preemptive_job_profile_report_ref"],
            serde_json::json!("fixtures/tassadar/reports/tassadar_preemptive_job_report.json")
        );
        assert_eq!(
            encoded["preemptive_job_fairness_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_preemptive_job_fairness_report.json"
            )
        );
        assert_eq!(
            encoded["preemptive_job_profile_id"],
            serde_json::json!("tassadar.internal_compute.preemptive_jobs.v1")
        );
        assert_eq!(
            encoded["preemptive_job_green_scheduler_ids"],
            serde_json::json!(["deterministic_round_robin", "weighted_fair_slice_rotation"])
        );
        assert_eq!(
            encoded["preemptive_job_refused_scheduler_ids"],
            serde_json::json!(["host_nondeterministic_scheduler"])
        );
        assert_eq!(
            encoded["preemptive_job_exact_case_count"],
            serde_json::json!(2)
        );
        assert_eq!(
            encoded["preemptive_job_refusal_case_count"],
            serde_json::json!(2)
        );
        assert_eq!(
            encoded["preemptive_job_served_publication_allowed"],
            serde_json::json!(false)
        );
        assert_eq!(
            encoded["broad_internal_compute_profile_publication"]["current_served_profile_id"],
            serde_json::json!("tassadar.internal_compute.article_closeout.v1")
        );
        assert_eq!(
            encoded["broad_internal_compute_profile_publication"]["route_policy_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_broad_internal_compute_route_policy_report.json"
            )
        );
        assert_eq!(
            encoded["frozen_core_wasm_window_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json"
            )
        );
        assert_eq!(
            encoded["frozen_core_wasm_closure_gate_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_frozen_core_wasm_closure_gate_report.json"
            )
        );
        assert_eq!(
            encoded["frozen_core_wasm_closure_status"],
            serde_json::json!("not_closed")
        );
        assert_eq!(
            encoded["frozen_core_wasm_served_publication_allowed"],
            serde_json::json!(false)
        );
        assert_eq!(
            encoded["broad_internal_compute_profile_publication"]["public_profile_specific_route_ids"],
            serde_json::json!([
                "tassadar.internal_compute.deterministic_import_subset.v1",
                "tassadar.internal_compute.runtime_support_subset.v1"
            ])
        );
        assert_eq!(
            encoded["subset_profile_promotion_gate_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_subset_profile_promotion_gate_report.json"
            )
        );
        assert_eq!(
            encoded["resumable_multi_slice_promotion_report_ref"],
            serde_json::json!(
                "fixtures/tassadar/reports/tassadar_resumable_multi_slice_promotion_report.json"
            )
        );
        assert_eq!(
            encoded["effect_safe_resume_report_ref"],
            serde_json::json!("fixtures/tassadar/reports/tassadar_effect_safe_resume_report.json")
        );
        assert_eq!(
            encoded["quantization_truth_envelope"]["active_backend_family"],
            serde_json::json!("cpu_reference")
        );
        let workload_classes = encoded["workload_capability_matrix"]["rows"]
            .as_array()
            .expect("rows should encode as an array")
            .iter()
            .map(|row| row["workload_class"].clone())
            .collect::<Vec<_>>();
        assert!(workload_classes.contains(&serde_json::json!("micro_wasm_kernel")));
    }

    #[test]
    fn executor_service_publishes_rust_only_article_runtime_closeout_surface() {
        let service = LocalTassadarExecutorService::new()
            .with_fixture(TassadarExecutorFixture::article_i32_compute_v1());
        let publication = service
            .rust_only_article_runtime_closeout_publication(Some(
                TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
            ))
            .expect("runtime closeout publication");

        assert_eq!(
            publication.report_ref,
            "fixtures/tassadar/reports/tassadar_article_runtime_closeout_summary.json"
        );
        assert_eq!(publication.exact_horizon_count, 4);
        assert_eq!(publication.floor_pass_count, 4);
        assert!(!publication.slowest_workload_horizon_id.is_empty());
    }

    #[test]
    fn executor_service_publishes_article_transformer_replacement_surface() {
        let service = LocalTassadarExecutorService::new()
            .with_fixture(TassadarExecutorFixture::article_i32_compute_v1());
        let publication = service
            .article_transformer_replacement_publication(Some(
                TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
            ))
            .expect("replacement publication");

        assert_eq!(
            publication.report_ref,
            "fixtures/tassadar/reports/tassadar_article_fixture_transformer_parity_report.json"
        );
        assert_eq!(
            publication.lineage_contract_ref,
            "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0_lineage_contract.json"
        );
        assert_eq!(publication.supported_case_count, 13);
        assert_eq!(publication.routeable_case_count, 13);
        assert_eq!(publication.exact_trace_case_count, 13);
        assert_eq!(publication.exact_output_case_count, 13);
        assert!(publication.replacement_certified);
        assert!(publication.article_equivalence_green);
        assert!(!publication.publication_digest.is_empty());
    }

    #[test]
    fn article_transformer_replacement_publication_rejects_non_article_fixture() {
        let service = LocalTassadarExecutorService::new()
            .with_fixture(TassadarExecutorFixture::core_i32_v2());
        let err = service
            .article_transformer_replacement_publication(Some(
                TassadarExecutorFixture::CORE_I32_V2_MODEL_ID,
            ))
            .expect_err("non-article fixture should stay outside the replacement publication");

        assert_eq!(
            err,
            TassadarExecutorCapabilityPublicationError::InvalidArticleTransformerReplacement {
                detail: format!(
                    "replacement publication is only defined for article fixture `{}`, found `{}`",
                    TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID,
                    TassadarExecutorFixture::CORE_I32_V2_MODEL_ID
                ),
            }
        );
    }

    #[test]
    fn executor_service_capability_publication_rejects_unbenchmarked_fixture() {
        let service = LocalTassadarExecutorService::new()
            .with_fixture(TassadarExecutorFixture::core_i32_v2());
        let err = service
            .capability_publication(Some(TassadarExecutorFixture::CORE_I32_V2_MODEL_ID))
            .expect_err("core_i32_v2 should stay unpublished");

        assert_eq!(
            err,
            TassadarExecutorCapabilityPublicationError::InvalidWorkloadCapabilityMatrix {
                error:
                    psionic_models::TassadarWorkloadCapabilityMatrixError::NoPublishableWorkloads,
            }
        );
    }

    #[test]
    fn article_session_executes_completed_request_with_benchmark_and_proof_identity() {
        let service = LocalTassadarArticleExecutorSessionService::new();
        let request =
            article_request_for_case("memory_heavy_kernel", TassadarExecutorDecodeMode::HullCache);
        let expected_outputs = tassadar_article_class_corpus()
            .into_iter()
            .find(|case| case.case_id == "memory_heavy_kernel")
            .expect("article case should exist")
            .expected_outputs;

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarArticleExecutorSessionOutcome::Completed { response } => {
                assert_eq!(response.product_id, ARTICLE_EXECUTOR_SESSION_PRODUCT_ID);
                assert_eq!(response.benchmark_identity.case_id, "memory_heavy_kernel");
                assert_eq!(
                    response.benchmark_identity.workload_family,
                    "MemoryHeavyKernel"
                );
                assert_eq!(
                    response.proof_identity.executor_product_id,
                    EXECUTOR_TRACE_PRODUCT_ID
                );
                assert_eq!(
                    response.executor_response.model_descriptor.model.model_id,
                    TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
                );
                assert_eq!(response.final_outputs(), expected_outputs.as_slice());
                assert_eq!(
                    response
                        .executor_response
                        .execution_report
                        .selection
                        .effective_decode_mode,
                    Some(TassadarExecutorDecodeMode::HullCache)
                );
                assert!(!response.readable_log.lines.is_empty());
                assert!(!response.token_trace.tokens.is_empty());
            }
            other => panic!("expected completed article session, got {other:?}"),
        }
    }

    #[test]
    fn article_session_keeps_representative_article_workloads_direct_on_hull_cache() {
        let service = LocalTassadarArticleExecutorSessionService::new();
        for case_id in ["long_loop_kernel", "sudoku_v0_test_a", "hungarian_matching"] {
            let request = article_request_for_case(case_id, TassadarExecutorDecodeMode::HullCache);
            let outcome = service.execute(&request).expect("request should be typed");
            match outcome {
                TassadarArticleExecutorSessionOutcome::Completed { response } => {
                    let selection = &response.executor_response.execution_report.selection;
                    assert_eq!(response.benchmark_identity.case_id, case_id);
                    assert_eq!(
                        response.executor_response.model_descriptor.model.model_id,
                        TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
                    );
                    assert_eq!(
                        selection.effective_decode_mode,
                        Some(TassadarExecutorDecodeMode::HullCache),
                        "case={case_id}"
                    );
                    assert_eq!(
                        selection.selection_state,
                        TassadarExecutorSelectionState::Direct,
                        "case={case_id}"
                    );
                }
                other => panic!("expected completed direct article session, got {other:?}"),
            }
        }
    }

    #[test]
    fn article_session_keeps_hungarian_10x10_demo_direct_on_hull_cache() {
        let service = LocalTassadarArticleExecutorSessionService::new();
        let request = article_request_for_case(
            "hungarian_10x10_test_a",
            TassadarExecutorDecodeMode::HullCache,
        );
        let expected_outputs = tassadar_hungarian_10x10_corpus()
            .into_iter()
            .find(|case| case.case_id == "hungarian_10x10_test_a")
            .expect("article case should exist")
            .validation_case
            .expected_outputs;

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarArticleExecutorSessionOutcome::Completed { response } => {
                let selection = &response.executor_response.execution_report.selection;
                assert_eq!(
                    response.benchmark_identity.case_id,
                    "hungarian_10x10_test_a"
                );
                assert_eq!(
                    response.executor_response.model_descriptor.model.model_id,
                    TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
                );
                assert_eq!(response.final_outputs(), expected_outputs.as_slice());
                assert_eq!(
                    selection.effective_decode_mode,
                    Some(TassadarExecutorDecodeMode::HullCache)
                );
                assert_eq!(
                    selection.selection_state,
                    TassadarExecutorSelectionState::Direct
                );
            }
            other => panic!("expected completed direct article session, got {other:?}"),
        }
    }

    #[test]
    fn article_session_keeps_hard_sudoku_suite_direct_on_hull_cache() {
        let service = LocalTassadarArticleExecutorSessionService::new();
        for case in tassadar_article_hard_sudoku_suite() {
            let request = article_request_for_case(
                case.case_id.as_str(),
                TassadarExecutorDecodeMode::HullCache,
            );
            let outcome = service.execute(&request).expect("request should be typed");
            match outcome {
                TassadarArticleExecutorSessionOutcome::Completed { response } => {
                    let selection = &response.executor_response.execution_report.selection;
                    assert_eq!(response.benchmark_identity.case_id, case.case_id);
                    assert_eq!(
                        response.executor_response.model_descriptor.model.model_id,
                        TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
                    );
                    assert_eq!(
                        response.executor_response.final_outputs(),
                        case.validation_case.expected_outputs.as_slice()
                    );
                    assert_eq!(
                        selection.effective_decode_mode,
                        Some(TassadarExecutorDecodeMode::HullCache)
                    );
                    assert_eq!(
                        selection.selection_state,
                        TassadarExecutorSelectionState::Direct
                    );
                }
                other => panic!("expected completed direct article session, got {other:?}"),
            }
        }
    }

    #[test]
    fn article_session_stream_surfaces_benchmark_proof_log_trace_and_terminal() {
        let service = LocalTassadarArticleExecutorSessionService::new();
        let request =
            article_request_for_case("memory_heavy_kernel", TassadarExecutorDecodeMode::HullCache);
        let mut stream = service
            .execute_stream(&request)
            .expect("stream should be created");

        let mut saw_benchmark = false;
        let mut saw_proof = false;
        let mut saw_log = false;
        let mut saw_token_trace = false;
        let mut saw_terminal = false;
        while let Some(event) = stream.next_event() {
            match event {
                TassadarArticleExecutorSessionStreamEvent::BenchmarkIdentity { .. } => {
                    saw_benchmark = true
                }
                TassadarArticleExecutorSessionStreamEvent::ProofIdentity { .. } => saw_proof = true,
                TassadarArticleExecutorSessionStreamEvent::ReadableLogLine { .. } => saw_log = true,
                TassadarArticleExecutorSessionStreamEvent::TokenTraceChunk { .. } => {
                    saw_token_trace = true
                }
                TassadarArticleExecutorSessionStreamEvent::Terminal { .. } => {
                    saw_terminal = true;
                    break;
                }
                TassadarArticleExecutorSessionStreamEvent::Capability { .. }
                | TassadarArticleExecutorSessionStreamEvent::DirectModelWeightExecutionProof {
                    ..
                }
                | TassadarArticleExecutorSessionStreamEvent::Selection { .. }
                | TassadarArticleExecutorSessionStreamEvent::Output { .. } => {}
            }
        }

        assert!(saw_benchmark);
        assert!(saw_proof);
        assert!(saw_log);
        assert!(saw_token_trace);
        assert!(saw_terminal);
    }

    #[test]
    fn article_session_preserves_fallback_truth_for_unsupported_fast_path_workloads() {
        let service = LocalTassadarArticleExecutorSessionService::new();
        let request = article_request_for_case(
            "branch_heavy_kernel",
            TassadarExecutorDecodeMode::SparseTopK,
        );

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarArticleExecutorSessionOutcome::Completed { response } => {
                let selection = &response.executor_response.execution_report.selection;
                assert!(selection.is_fallback());
                assert_eq!(
                    selection.effective_decode_mode,
                    Some(TassadarExecutorDecodeMode::ReferenceLinear)
                );
            }
            other => panic!("expected completed fallback article session, got {other:?}"),
        }
    }

    #[test]
    fn article_session_emits_direct_model_weight_proof_when_required() {
        let service = LocalTassadarArticleExecutorSessionService::new();
        let request = article_request_for_case(
            "long_loop_kernel",
            TassadarExecutorDecodeMode::ReferenceLinear,
        )
        .require_direct_model_weight_proof();

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarArticleExecutorSessionOutcome::Completed { response } => {
                let receipt = response
                    .direct_model_weight_execution_proof_receipt
                    .expect("proof receipt should be present");
                assert_eq!(receipt.article_case_id, "long_loop_kernel");
                assert_eq!(receipt.external_call_count, 0);
                assert!(!receipt.fallback_observed);
                assert_eq!(
                    receipt.requested_decode_mode,
                    TassadarExecutorDecodeMode::ReferenceLinear
                );
                assert_eq!(
                    receipt.effective_decode_mode,
                    TassadarExecutorDecodeMode::ReferenceLinear
                );
                assert!(!receipt.route_binding.route_descriptor_digest.is_empty());
            }
            other => panic!("expected completed proof-required article session, got {other:?}"),
        }
    }

    #[test]
    fn article_session_refuses_proof_required_fallback_requests() {
        let service = LocalTassadarArticleExecutorSessionService::new();
        let request = article_request_for_case(
            "branch_heavy_kernel",
            TassadarExecutorDecodeMode::SparseTopK,
        )
        .require_direct_model_weight_proof();

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarArticleExecutorSessionOutcome::Refused { refusal } => {
                assert!(
                    refusal
                        .detail
                        .contains("direct model-weight execution proof")
                );
            }
            other => panic!("expected proof-required fallback refusal, got {other:?}"),
        }
    }

    #[test]
    fn article_session_refuses_non_article_workloads_explicitly() {
        let service = LocalTassadarArticleExecutorSessionService::new();
        let request =
            article_request_for_case("locals_add", TassadarExecutorDecodeMode::ReferenceLinear);

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarArticleExecutorSessionOutcome::Refused { refusal } => {
                assert!(refusal.benchmark_identity.is_none());
                assert!(refusal.detail.contains("canonical Tassadar article corpus"));
            }
            other => panic!("expected refusal, got {other:?}"),
        }
    }

    #[test]
    fn article_session_rejects_non_article_product_id() {
        let service = LocalTassadarArticleExecutorSessionService::new();
        let mut request =
            article_request_for_case("memory_heavy_kernel", TassadarExecutorDecodeMode::HullCache);
        request.product_id = String::from("psionic.text_generation");

        let error = service
            .execute(&request)
            .expect_err("wrong article-session product should fail before execution");
        assert_eq!(
            error,
            TassadarArticleExecutorSessionServiceError::UnsupportedProduct {
                product_id: String::from("psionic.text_generation"),
            }
        );
    }

    #[test]
    fn article_hybrid_workflow_delegates_completed_request_with_routing_and_proof_identity() {
        let service = LocalTassadarArticleHybridWorkflowService::new();
        let request = article_workflow_request_for_case(
            "memory_heavy_kernel",
            TassadarExecutorDecodeMode::HullCache,
        );

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarArticleHybridWorkflowOutcome::Completed { response } => {
                assert_eq!(response.product_id, ARTICLE_HYBRID_WORKFLOW_PRODUCT_ID);
                assert_eq!(response.benchmark_identity.case_id, "memory_heavy_kernel");
                assert_eq!(
                    response.proof_identity.executor_product_id,
                    EXECUTOR_TRACE_PRODUCT_ID
                );
                assert_eq!(
                    response
                        .planner_response
                        .routing_decision
                        .planner_product_id,
                    PLANNER_EXECUTOR_ROUTE_PRODUCT_ID
                );
                assert_eq!(
                    response.planner_response.routing_decision.route_state,
                    super::TassadarPlannerRouteState::Delegated
                );
            }
            other => panic!("expected completed hybrid workflow, got {other:?}"),
        }
    }

    #[test]
    fn article_hybrid_workflow_keeps_representative_article_workloads_direct_on_hull_cache() {
        let service = LocalTassadarArticleHybridWorkflowService::new();
        for case_id in ["long_loop_kernel", "sudoku_v0_test_a", "hungarian_matching"] {
            let request =
                article_workflow_request_for_case(case_id, TassadarExecutorDecodeMode::HullCache);
            let outcome = service.execute(&request).expect("request should be typed");
            match outcome {
                TassadarArticleHybridWorkflowOutcome::Completed { response } => {
                    let routing = &response.planner_response.routing_decision;
                    let selection = &response
                        .planner_response
                        .executor_response
                        .execution_report
                        .selection;
                    assert_eq!(response.benchmark_identity.case_id, case_id);
                    assert_eq!(
                        routing.effective_decode_mode,
                        Some(TassadarExecutorDecodeMode::HullCache),
                        "case={case_id}"
                    );
                    assert_eq!(
                        selection.effective_decode_mode,
                        Some(TassadarExecutorDecodeMode::HullCache),
                        "case={case_id}"
                    );
                    assert_eq!(
                        selection.selection_state,
                        TassadarExecutorSelectionState::Direct,
                        "case={case_id}"
                    );
                }
                other => panic!("expected completed direct hybrid workflow, got {other:?}"),
            }
        }
    }

    #[test]
    fn article_hybrid_workflow_keeps_hungarian_10x10_demo_direct_on_hull_cache() {
        let service = LocalTassadarArticleHybridWorkflowService::new();
        let request = article_workflow_request_for_case(
            "hungarian_10x10_test_a",
            TassadarExecutorDecodeMode::HullCache,
        );
        let expected_outputs = tassadar_hungarian_10x10_corpus()
            .into_iter()
            .find(|case| case.case_id == "hungarian_10x10_test_a")
            .expect("article case should exist")
            .validation_case
            .expected_outputs;

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarArticleHybridWorkflowOutcome::Completed { response } => {
                let routing = &response.planner_response.routing_decision;
                let selection = &response
                    .planner_response
                    .executor_response
                    .execution_report
                    .selection;
                assert_eq!(
                    response.benchmark_identity.case_id,
                    "hungarian_10x10_test_a"
                );
                assert_eq!(
                    response.planner_response.executor_response.final_outputs(),
                    expected_outputs.as_slice()
                );
                assert_eq!(
                    routing.effective_decode_mode,
                    Some(TassadarExecutorDecodeMode::HullCache)
                );
                assert_eq!(
                    selection.effective_decode_mode,
                    Some(TassadarExecutorDecodeMode::HullCache)
                );
                assert_eq!(
                    selection.selection_state,
                    TassadarExecutorSelectionState::Direct
                );
            }
            other => panic!("expected completed direct hybrid workflow, got {other:?}"),
        }
    }

    #[test]
    fn article_hybrid_workflow_keeps_hard_sudoku_suite_direct_on_hull_cache() {
        let service = LocalTassadarArticleHybridWorkflowService::new();
        for case in tassadar_article_hard_sudoku_suite() {
            let request = article_workflow_request_for_case(
                case.case_id.as_str(),
                TassadarExecutorDecodeMode::HullCache,
            );
            let outcome = service.execute(&request).expect("request should be typed");
            match outcome {
                TassadarArticleHybridWorkflowOutcome::Completed { response } => {
                    let routing = &response.planner_response.routing_decision;
                    let selection = &response
                        .planner_response
                        .executor_response
                        .execution_report
                        .selection;
                    assert_eq!(response.benchmark_identity.case_id, case.case_id);
                    assert_eq!(
                        response
                            .planner_response
                            .executor_response
                            .model_descriptor
                            .model
                            .model_id,
                        TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
                    );
                    assert_eq!(
                        response.planner_response.executor_response.final_outputs(),
                        case.validation_case.expected_outputs.as_slice()
                    );
                    assert_eq!(
                        routing.effective_decode_mode,
                        Some(TassadarExecutorDecodeMode::HullCache)
                    );
                    assert_eq!(
                        selection.effective_decode_mode,
                        Some(TassadarExecutorDecodeMode::HullCache)
                    );
                    assert_eq!(
                        selection.selection_state,
                        TassadarExecutorSelectionState::Direct
                    );
                }
                other => panic!("expected completed direct hybrid workflow, got {other:?}"),
            }
        }
    }

    #[test]
    fn article_hybrid_workflow_preserves_typed_planner_fallback() {
        let service = LocalTassadarArticleHybridWorkflowService::new();
        let request = article_workflow_request_for_case(
            "branch_heavy_kernel",
            TassadarExecutorDecodeMode::SparseTopK,
        )
        .with_routing_policy(
            TassadarPlannerRoutingPolicy::exact_executor_default()
                .with_fallback_policy(TassadarPlannerFallbackPolicy::PlannerSummary),
        );
        let request = TassadarArticleHybridWorkflowRequest {
            routing_policy: TassadarPlannerRoutingPolicy {
                allow_runtime_decode_fallback: false,
                ..request.routing_policy
            },
            ..request
        };

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarArticleHybridWorkflowOutcome::Fallback { fallback } => {
                assert_eq!(
                    fallback
                        .benchmark_identity
                        .as_ref()
                        .map(|identity| identity.case_id.as_str()),
                    Some("branch_heavy_kernel")
                );
                assert_eq!(
                    fallback.planner_fallback.routing_decision.route_reason,
                    Some(TassadarPlannerRouteReason::ExecutorDecodeFallbackDisallowed)
                );
            }
            other => panic!("expected workflow fallback, got {other:?}"),
        }
    }

    #[test]
    fn article_hybrid_workflow_preserves_typed_planner_refusal() {
        let service = LocalTassadarArticleHybridWorkflowService::new();
        let request = article_workflow_request_for_case(
            "memory_heavy_kernel",
            TassadarExecutorDecodeMode::HullCache,
        )
        .with_routing_budget(TassadarPlannerRoutingBudget::new(1, 32, 8));

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarArticleHybridWorkflowOutcome::Refused { refusal } => {
                assert_eq!(
                    refusal
                        .benchmark_identity
                        .as_ref()
                        .map(|identity| identity.case_id.as_str()),
                    Some("memory_heavy_kernel")
                );
                assert_eq!(
                    refusal
                        .planner_refusal
                        .as_ref()
                        .and_then(|planner_refusal| planner_refusal.routing_decision.route_reason),
                    Some(TassadarPlannerRouteReason::ProgramLengthBudgetExceeded)
                );
            }
            other => panic!("expected workflow refusal, got {other:?}"),
        }
    }

    #[test]
    fn article_hybrid_workflow_rejects_non_hybrid_product_id() {
        let service = LocalTassadarArticleHybridWorkflowService::new();
        let mut request = article_workflow_request_for_case(
            "memory_heavy_kernel",
            TassadarExecutorDecodeMode::HullCache,
        );
        request.product_id = String::from("psionic.text_generation");

        let error = service
            .execute(&request)
            .expect_err("wrong hybrid-workflow product should fail before execution");
        assert_eq!(
            error,
            TassadarArticleHybridWorkflowServiceError::UnsupportedProduct {
                product_id: String::from("psionic.text_generation"),
            }
        );
    }

    #[test]
    fn tassadar_lab_live_article_session_prepares_snapshot_and_updates() {
        let service = LocalTassadarLabService::new();
        let prepared = service
            .prepare(&TassadarLabRequest::ArticleExecutorSession {
                request: article_request_for_case(
                    "memory_heavy_kernel",
                    TassadarExecutorDecodeMode::HullCache,
                ),
            })
            .expect("live article session should prepare");

        assert_eq!(
            prepared.snapshot.source_kind,
            TassadarLabSourceKind::LiveArticleSession
        );
        assert_eq!(prepared.snapshot.replay_id, None);
        assert_eq!(
            prepared
                .snapshot
                .benchmark_identity
                .as_ref()
                .map(|identity| identity.case_id.as_str()),
            Some("memory_heavy_kernel")
        );
        assert!(prepared.snapshot.proof_identity.is_some());
        assert!(prepared.snapshot.readable_log.is_some());
        assert!(prepared.snapshot.token_trace.is_some());
        assert!(
            prepared
                .updates
                .iter()
                .any(|update| matches!(update, TassadarLabUpdate::ProofIdentity { .. }))
        );
        assert!(
            prepared
                .updates
                .iter()
                .any(|update| matches!(update, TassadarLabUpdate::Terminal { .. }))
        );
    }

    #[test]
    fn tassadar_lab_replay_catalog_and_prepared_replays_cover_live_and_report_cases() {
        let service = LocalTassadarLabService::new();
        let catalog = service.replay_catalog();
        assert!(catalog.len() >= 12);
        assert!(catalog.iter().any(|entry| {
            entry.replay_id == TassadarLabReplayId::ArticleHybridFallbackBranchHeavy
                && entry.artifact_ref == super::TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF
        }));
        assert!(catalog.iter().any(|entry| {
            entry.replay_id == TassadarLabReplayId::AcceptanceReport
                && entry.artifact_ref == super::TASSADAR_ACCEPTANCE_REPORT_REF
        }));

        let hybrid_replay = service
            .prepare(&TassadarLabRequest::Replay {
                replay_id: TassadarLabReplayId::ArticleHybridFallbackBranchHeavy,
            })
            .expect("hybrid replay should prepare");
        assert_eq!(
            hybrid_replay.snapshot.source_kind,
            TassadarLabSourceKind::ReplayArtifact
        );
        assert_eq!(
            hybrid_replay.snapshot.replay_id,
            Some(TassadarLabReplayId::ArticleHybridFallbackBranchHeavy)
        );
        assert_eq!(
            hybrid_replay.snapshot.artifact_ref.as_deref(),
            Some(super::TASSADAR_ARTICLE_HYBRID_WORKFLOW_ARTIFACT_REF)
        );
        assert!(
            hybrid_replay
                .updates
                .iter()
                .any(|update| matches!(update, TassadarLabUpdate::RoutingStatus { .. }))
        );

        let acceptance_replay = service
            .prepare(&TassadarLabRequest::Replay {
                replay_id: TassadarLabReplayId::AcceptanceReport,
            })
            .expect("acceptance replay should prepare");
        assert_eq!(
            acceptance_replay.snapshot.replay_id,
            Some(TassadarLabReplayId::AcceptanceReport)
        );
        assert!(
            acceptance_replay
                .snapshot
                .status_label
                .contains("acceptance")
        );
        assert!(
            acceptance_replay
                .updates
                .iter()
                .any(|update| matches!(update, TassadarLabUpdate::StatusLine { .. }))
        );
    }

    #[test]
    fn tassadar_lab_types_roundtrip_through_json() {
        let service = LocalTassadarLabService::new();
        let request = TassadarLabRequest::Replay {
            replay_id: TassadarLabReplayId::CompiledArticleClosureReport,
        };
        let encoded_request =
            serde_json::to_vec(&request).expect("lab replay request should serialize");
        let decoded_request: TassadarLabRequest =
            serde_json::from_slice(&encoded_request).expect("lab replay request should decode");
        assert_eq!(decoded_request, request);

        let prepared = service
            .prepare(&request)
            .expect("compiled article replay should prepare");
        let encoded_prepared =
            serde_json::to_vec(&prepared).expect("prepared lab view should serialize");
        let decoded_prepared: TassadarLabPreparedView =
            serde_json::from_slice(&encoded_prepared).expect("prepared lab view should decode");
        assert_eq!(decoded_prepared.snapshot, prepared.snapshot);
        assert_eq!(decoded_prepared.updates, prepared.updates);
    }

    #[test]
    fn executor_service_executes_completed_request_with_explicit_product_semantics() {
        let service = LocalTassadarExecutorService::new();
        let request = request_for_case("locals_add");
        let outcome = service.execute(&request).expect("request should execute");

        match outcome {
            TassadarExecutorOutcome::Completed { response } => {
                assert_eq!(response.product_id, EXECUTOR_TRACE_PRODUCT_ID);
                assert_eq!(response.final_outputs(), &[12]);
                assert_eq!(
                    response.execution_report.selection.effective_decode_mode,
                    Some(TassadarExecutorDecodeMode::HullCache)
                );
                assert_eq!(
                    response.evidence_bundle.proof_bundle.product_id,
                    EXECUTOR_TRACE_PRODUCT_ID
                );
                assert_eq!(
                    response.trace_artifact().program_id,
                    "tassadar.locals_add.v1"
                );
            }
            TassadarExecutorOutcome::Refused { refusal } => {
                panic!("request should not be refused: {}", refusal.detail);
            }
        }
    }

    #[test]
    fn executor_service_returns_explicit_refusal_for_contract_mismatch() {
        let service = LocalTassadarExecutorService::new();
        let mut request = request_for_case("locals_add");
        request.program_artifact.trace_abi_version += 1;

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarExecutorOutcome::Completed { .. } => {
                panic!("mismatched ABI should not complete");
            }
            TassadarExecutorOutcome::Refused { refusal } => {
                assert!(refusal.contract_error.is_some());
                assert!(refusal.selection.is_none());
            }
        }
    }

    #[test]
    fn executor_stream_surfaces_capability_selection_trace_and_terminal() {
        let service = LocalTassadarExecutorService::new();
        let request = request_for_case("memory_roundtrip");
        let mut stream = service
            .execute_stream(&request)
            .expect("stream should be created");

        let first = stream.next_event().expect("capability event");
        assert!(matches!(
            first,
            TassadarExecutorStreamEvent::Capability { .. }
        ));
        let second = stream.next_event().expect("selection event");
        assert!(matches!(
            second,
            TassadarExecutorStreamEvent::Selection { .. }
        ));

        let mut saw_trace = false;
        let mut saw_terminal = false;
        while let Some(event) = stream.next_event() {
            match event {
                TassadarExecutorStreamEvent::TraceStep { .. } => saw_trace = true,
                TassadarExecutorStreamEvent::Terminal { .. } => {
                    saw_terminal = true;
                    break;
                }
                TassadarExecutorStreamEvent::Output { .. }
                | TassadarExecutorStreamEvent::Capability { .. }
                | TassadarExecutorStreamEvent::Selection { .. } => {}
            }
        }

        assert!(saw_trace);
        assert!(saw_terminal);
    }

    #[test]
    fn executor_service_rejects_non_executor_product_id() {
        let service = LocalTassadarExecutorService::new();
        let mut request = request_for_case("branch_guard");
        request.product_id = String::from("psionic.text_generation");

        let error = service
            .execute(&request)
            .expect_err("wrong product should fail before execution");
        assert_eq!(
            error,
            TassadarExecutorServiceError::UnsupportedProduct {
                product_id: String::from("psionic.text_generation"),
            }
        );
    }

    fn planner_request_for_case(case_id: &str) -> TassadarPlannerRoutingRequest {
        TassadarPlannerRoutingRequest::new(
            format!("planner-request-{case_id}"),
            "session-alpha",
            "planner-fixture-v0",
            TassadarPlannerExecutorSubproblem::new(
                format!("subproblem-{case_id}"),
                "exact arithmetic subproblem",
                request_for_case(case_id).program_artifact,
                TassadarExecutorDecodeMode::HullCache,
            )
            .with_environment_refs(vec![String::from("env.openagents.tassadar.benchmark")]),
        )
    }

    fn backward_branch_sparse_artifact() -> TassadarProgramArtifact {
        let profile = TassadarWasmProfile::core_i32_v2();
        let trace_abi = TassadarTraceAbi::core_i32_v2();
        let program = TassadarProgram::new(
            "tassadar.backward_branch_sparse.v1",
            &profile,
            1,
            0,
            vec![
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::BrIf { target_pc: 2 },
                TassadarInstruction::I32Const { value: 9 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        );
        TassadarProgramArtifact::fixture_reference(
            "artifact://tassadar/backward_branch_sparse",
            &profile,
            &trace_abi,
            program,
        )
        .expect("backward branch fixture should build")
    }

    #[test]
    fn planner_router_delegates_completed_exact_request_with_full_executor_truth() {
        let router = LocalTassadarPlannerRouter::new();
        let request = planner_request_for_case("locals_add");

        let outcome = router
            .route(&request)
            .expect("planner route should succeed");
        match outcome {
            TassadarPlannerRoutingOutcome::Completed { response } => {
                assert_eq!(
                    response.routing_decision.planner_product_id,
                    PLANNER_EXECUTOR_ROUTE_PRODUCT_ID
                );
                assert_eq!(
                    response.routing_decision.executor_product_id,
                    EXECUTOR_TRACE_PRODUCT_ID
                );
                assert_eq!(response.executor_response.final_outputs(), &[12]);
                assert_eq!(
                    response
                        .executor_response
                        .evidence_bundle
                        .proof_bundle
                        .product_id,
                    EXECUTOR_TRACE_PRODUCT_ID
                );
                assert_eq!(
                    response.routing_decision.effective_decode_mode,
                    Some(TassadarExecutorDecodeMode::HullCache)
                );
                assert!(response.routing_decision.selection.is_some());
                assert!(!response.routing_decision.routing_digest.is_empty());
            }
            other => panic!("expected delegated completion, got {other:?}"),
        }
    }

    #[test]
    fn planner_router_route_capability_descriptor_is_machine_legible() {
        let router = LocalTassadarPlannerRouter::new();
        let descriptor = router
            .route_capability_descriptor(Some(
                TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID,
            ))
            .expect("route descriptor should publish");

        assert_eq!(
            descriptor.product_id,
            psionic_router::TASSADAR_PLANNER_EXECUTOR_ROUTE_PRODUCT_ID
        );
        assert_eq!(
            descriptor.model_id,
            TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
        );
        assert_eq!(
            descriptor.internal_compute_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert!(!descriptor.internal_compute_profile_claim_digest.is_empty());
        let reference_linear = descriptor
            .decode_capabilities
            .iter()
            .find(|capability| {
                capability.requested_decode_mode == TassadarExecutorDecodeMode::ReferenceLinear
            })
            .expect("reference-linear decode capability");
        assert_eq!(
            reference_linear.route_posture,
            TassadarPlannerExecutorRoutePosture::DirectGuaranteed
        );
        let hull_cache = descriptor
            .decode_capabilities
            .iter()
            .find(|capability| {
                capability.requested_decode_mode == TassadarExecutorDecodeMode::HullCache
            })
            .expect("hull-cache decode capability");
        assert_eq!(
            hull_cache.route_posture,
            TassadarPlannerExecutorRoutePosture::FallbackCapable
        );
        assert_eq!(
            hull_cache.benchmark_report_ref,
            TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF
        );
        let micro_kernel = descriptor
            .wasm_capability_matrix
            .rows
            .iter()
            .find(|row| row.module_class == TassadarWorkloadClass::MicroWasmKernel)
            .expect("micro kernel row");
        assert_eq!(
            micro_kernel.import_posture,
            TassadarPlannerExecutorWasmImportPosture::NoImportsOnly
        );
        assert!(
            micro_kernel
                .direct_decode_modes
                .contains(&TassadarExecutorDecodeMode::HullCache)
        );
        let long_loop = descriptor
            .wasm_capability_matrix
            .rows
            .iter()
            .find(|row| row.module_class == TassadarWorkloadClass::LongLoopKernel)
            .expect("long loop row");
        assert_eq!(
            long_loop.exact_fallback_decode_mode,
            Some(TassadarExecutorDecodeMode::ReferenceLinear)
        );
        assert!(
            !long_loop
                .direct_decode_modes
                .contains(&TassadarExecutorDecodeMode::HullCache)
        );
        assert!(!descriptor.descriptor_digest.is_empty());
    }

    #[test]
    fn planner_router_can_return_typed_fallback_when_policy_disallows_runtime_decode_fallback() {
        let router = LocalTassadarPlannerRouter::new().with_executor_service(
            LocalTassadarExecutorService::new()
                .with_fixture(TassadarExecutorFixture::core_i32_v2()),
        );
        let request = TassadarPlannerRoutingRequest::new(
            "planner-request-sparse-fallback",
            "session-beta",
            "planner-fixture-v0",
            TassadarPlannerExecutorSubproblem::new(
                "subproblem-sparse-fallback",
                "exact sparse top-k subproblem",
                backward_branch_sparse_artifact(),
                TassadarExecutorDecodeMode::SparseTopK,
            ),
        )
        .with_routing_budget(TassadarPlannerRoutingBudget::new(128, 512, 8))
        .with_routing_policy(
            TassadarPlannerRoutingPolicy::exact_executor_default()
                .with_fallback_policy(TassadarPlannerFallbackPolicy::PlannerSummary),
        );
        let request = TassadarPlannerRoutingRequest {
            subproblem: request
                .subproblem
                .with_requested_model_id(TassadarExecutorFixture::CORE_I32_V2_MODEL_ID),
            routing_policy: TassadarPlannerRoutingPolicy {
                allow_runtime_decode_fallback: false,
                ..request.routing_policy
            },
            ..request
        };

        let outcome = router
            .route(&request)
            .expect("planner route should be typed");
        match outcome {
            TassadarPlannerRoutingOutcome::Fallback { fallback } => {
                assert_eq!(
                    fallback.routing_decision.route_reason,
                    Some(TassadarPlannerRouteReason::ExecutorDecodeFallbackDisallowed)
                );
                assert!(
                    fallback
                        .routing_decision
                        .selection
                        .as_ref()
                        .is_some_and(|selection| selection.is_fallback())
                );
                assert!(fallback.fallback_summary.contains("disallowed"));
            }
            other => panic!("expected typed fallback, got {other:?}"),
        }
    }

    #[test]
    fn research_lane_promotion_readiness_is_blocked_until_refusal_and_route_gates_clear() {
        let err = require_tassadar_research_lane_promotion_ready()
            .expect_err("current research lane should remain blocked");

        match err {
            TassadarResearchPromotionError::PromotionBlocked { failed_gates, .. } => {
                assert_eq!(
                    failed_gates,
                    vec![
                        TassadarPromotionChecklistGateKind::RefusalBehavior,
                        TassadarPromotionChecklistGateKind::RouteContractCompatibility,
                    ]
                );
            }
            other => panic!("expected blocked promotion policy, got {other:?}"),
        }
    }

    #[test]
    fn planner_router_refuses_when_program_exceeds_budget() {
        let router = LocalTassadarPlannerRouter::new();
        let request = planner_request_for_case("memory_roundtrip")
            .with_routing_budget(TassadarPlannerRoutingBudget::new(4, 512, 8));

        let outcome = router
            .route(&request)
            .expect("planner route should be typed");
        match outcome {
            TassadarPlannerRoutingOutcome::Refused { refusal } => {
                assert_eq!(
                    refusal.routing_decision.route_reason,
                    Some(TassadarPlannerRouteReason::ProgramLengthBudgetExceeded)
                );
                assert!(refusal.detail.contains("exceeds planner budget"));
            }
            other => panic!("expected budget refusal, got {other:?}"),
        }
    }

    #[test]
    fn planner_router_rejects_non_planner_product_id() {
        let router = LocalTassadarPlannerRouter::new();
        let mut request = planner_request_for_case("locals_add");
        request.product_id = String::from("psionic.text_generation");

        let error = router
            .route(&request)
            .expect_err("wrong planner product should fail before routing");
        assert_eq!(
            error,
            TassadarPlannerRouterError::UnsupportedProduct {
                product_id: String::from("psionic.text_generation"),
            }
        );
    }
}
