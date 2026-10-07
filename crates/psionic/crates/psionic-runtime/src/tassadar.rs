use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use crate::{
    BackendProbeState, BackendToolchainIdentity, ExecutionProofArtifactResidency,
    ExecutionProofAugmentationPosture, ExecutionProofBundle, ExecutionProofBundleKind,
    ExecutionProofBundleStatus, ExecutionProofRuntimeIdentity, RuntimeManifest,
    RuntimeManifestArtifactBinding, RuntimeManifestArtifactKind,
    RuntimeManifestStaticConfigBinding, ValidationMatrixReference,
};
use psionic_ir::{encode_tassadar_normalized_wasm_module, parse_tassadar_normalized_wasm_module};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Current append-only trace ABI version for the Tassadar executor lane.
pub const TASSADAR_TRACE_ABI_VERSION: u16 = 1;
/// Stable CPU reference runner identifier for the Phase 1 lane.
pub const TASSADAR_CPU_REFERENCE_RUNNER_ID: &str = "tassadar.cpu_reference.v1";
/// Stable fixture runner identifier for the Phase 1 lane.
pub const TASSADAR_FIXTURE_RUNNER_ID: &str = "tassadar.fixture_runner.v1";
/// Stable hull-cache runner identifier for the Phase 5 lane.
pub const TASSADAR_HULL_CACHE_RUNNER_ID: &str = "tassadar.hull_cache_runner.v1";
/// Stable research-only hierarchical-hull runner identifier.
pub const TASSADAR_HIERARCHICAL_HULL_CANDIDATE_RUNNER_ID: &str =
    "tassadar.hierarchical_hull_candidate.v0";
/// Stable sparse-top-k runner identifier for the Phase 8 lane.
pub const TASSADAR_SPARSE_TOP_K_RUNNER_ID: &str = "tassadar.sparse_top_k_runner.v1";
/// Stable runtime backend identifier for the current Tassadar reference lane.
pub const TASSADAR_RUNTIME_BACKEND_ID: &str = "cpu";
/// Stable opcode-vocabulary family identifier for the Phase 2 artifact lane.
pub const TASSADAR_OPCODE_VOCABULARY_ID: &str = "tassadar.opcodes.v1";
/// Current schema version for emitted Tassadar trace artifacts.
pub const TASSADAR_TRACE_ARTIFACT_SCHEMA_VERSION: u16 = 1;
/// Current schema version for emitted Tassadar trace-proof artifacts.
pub const TASSADAR_TRACE_PROOF_SCHEMA_VERSION: u16 = 1;
/// Stable claims-profile identifier for the Tassadar proof lane.
pub const TASSADAR_PROOF_CLAIMS_PROFILE_ID: &str = "tassadar.executor_trace.proof.v1";

/// Coarse claim class for persisted Tassadar artifacts.
///
/// This is the top-level vocabulary used to keep compiled, learned, and
/// research-only results distinct. It does not replace finer-grained fields
/// such as `claim_boundary`, `boundary_label`, or `serve_posture`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarClaimClass {
    /// Exact compiled/proof-backed execution on a matched bounded corpus.
    CompiledExact,
    /// Exact compiled/proof-backed execution on the accepted article-class bar.
    CompiledArticleClass,
    /// Learned execution with an explicit bounded envelope.
    LearnedBounded,
    /// Learned execution that clears the accepted article-class bar.
    LearnedArticleClass,
    /// Research-only results that are not yet promoted to an executor claim.
    ResearchOnly,
}

impl TassadarClaimClass {
    /// Returns whether one claim class can honestly advance to another without
    /// changing lanes.
    #[must_use]
    pub fn allows_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        matches!(
            (self, next),
            (
                Self::ResearchOnly,
                Self::CompiledExact | Self::LearnedBounded
            ) | (Self::CompiledExact, Self::CompiledArticleClass)
                | (Self::LearnedBounded, Self::LearnedArticleClass)
        )
    }
}

/// Stable decode modes for the Tassadar executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorDecodeMode {
    /// Linear reference decode path over the executor trace.
    ReferenceLinear,
    /// Hull-cache geometric fast path.
    HullCache,
    /// Sparse top-k decode path on the validated executor subset.
    SparseTopK,
}

impl TassadarExecutorDecodeMode {
    /// Returns the stable decode-mode identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReferenceLinear => "tassadar.decode.reference_linear.v1",
            Self::HullCache => "tassadar.decode.hull_cache.v1",
            Self::SparseTopK => "tassadar.decode.sparse_top_k.v1",
        }
    }

    /// Returns the cache algorithm paired with this decode mode.
    #[must_use]
    pub const fn cache_algorithm(self) -> TassadarExecutorCacheAlgorithm {
        match self {
            Self::ReferenceLinear => TassadarExecutorCacheAlgorithm::LinearScanKvCache,
            Self::HullCache => TassadarExecutorCacheAlgorithm::HullSupportCache,
            Self::SparseTopK => TassadarExecutorCacheAlgorithm::SparseTopKCache,
        }
    }
}

/// Stable cache-algorithm identifiers for Tassadar decoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorCacheAlgorithm {
    /// Prefix-linear cache lookups over the trace.
    LinearScanKvCache,
    /// Hull-backed geometric cache.
    HullSupportCache,
    /// Sparse top-k cache lookups.
    SparseTopKCache,
}

impl TassadarExecutorCacheAlgorithm {
    /// Returns the stable cache-algorithm identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinearScanKvCache => "tassadar.cache.linear_scan_kv.v1",
            Self::HullSupportCache => "tassadar.cache.hull_support.v1",
            Self::SparseTopKCache => "tassadar.cache.sparse_top_k.v1",
        }
    }
}

/// Runtime-visible attention families for the Tassadar executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRuntimeAttentionMode {
    /// Exact reference-linear attention over the append-only trace.
    ReferenceLinear,
    /// Exact hard-max hull lookup over the validated fast-path subset.
    HardMaxHull,
    /// Validated sparse top-k attention on the current executor subset.
    SparseTopKValidated,
}

/// Runtime capability report for the current Tassadar executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRuntimeCapabilityReport {
    /// Runtime backend currently exposing the executor lane.
    pub runtime_backend: String,
    /// Whether append-only executor traces are supported at all.
    pub supports_executor_trace: bool,
    /// Whether exact hull-cache decode is available on a validated subset.
    pub supports_hull_decode: bool,
    /// Whether validated sparse-top-k decode is available on a validated subset.
    pub supports_sparse_top_k_decode: bool,
    /// Stable Wasm profiles accepted by the runtime.
    pub supported_wasm_profiles: Vec<String>,
    /// Stable attention families the runtime can speak about honestly.
    pub supported_attention_modes: Vec<TassadarRuntimeAttentionMode>,
    /// Validated trace ABI schema versions.
    pub validated_trace_abi_versions: Vec<u16>,
    /// Exact decode modes supported directly without fallback.
    pub supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
    /// Default exact decode mode when no fast path is requested.
    pub default_decode_mode: TassadarExecutorDecodeMode,
    /// Exact fallback decode mode when a requested fast path is unsupported.
    pub exact_fallback_decode_mode: TassadarExecutorDecodeMode,
}

impl TassadarRuntimeCapabilityReport {
    /// Returns the canonical current capability report for Tassadar on CPU.
    #[must_use]
    pub fn current() -> Self {
        Self {
            runtime_backend: String::from(TASSADAR_RUNTIME_BACKEND_ID),
            supports_executor_trace: true,
            supports_hull_decode: true,
            supports_sparse_top_k_decode: true,
            supported_wasm_profiles: supported_wasm_profile_ids(),
            supported_attention_modes: vec![
                TassadarRuntimeAttentionMode::ReferenceLinear,
                TassadarRuntimeAttentionMode::HardMaxHull,
                TassadarRuntimeAttentionMode::SparseTopKValidated,
            ],
            validated_trace_abi_versions: vec![TASSADAR_TRACE_ABI_VERSION],
            supported_decode_modes: vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
                TassadarExecutorDecodeMode::SparseTopK,
            ],
            default_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
            exact_fallback_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
        }
    }
}

/// Selection state for one requested Tassadar decode path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorSelectionState {
    /// Requested path can run directly.
    Direct,
    /// Requested path degrades to an explicit fallback mode.
    Fallback,
    /// Requested path is refused before execution.
    Refused,
}

/// Machine-legible reason explaining one decode selection outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorSelectionReason {
    /// Hull-cache is outside the validated control-flow subset.
    HullCacheControlFlowUnsupported,
    /// Sparse top-k is outside the current validated subset and falls back to exact decoding.
    SparseTopKValidationUnsupported,
    /// The program targeted a different Wasm profile than the runtime supports.
    UnsupportedWasmProfile,
    /// The caller requested an ABI schema version this runtime has not validated.
    UnsupportedTraceAbiVersion,
    /// The effective decode mode is not supported by the selected model descriptor.
    UnsupportedModelDecodeMode,
}

/// Machine-legible decode-path diagnostic emitted before execution begins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorSelectionDiagnostic {
    /// Stable program identifier targeted by the request.
    pub program_id: String,
    /// Runtime backend evaluating the request.
    pub runtime_backend: String,
    /// Requested Wasm profile identifier.
    pub requested_profile_id: String,
    /// Requested trace ABI schema version.
    pub requested_trace_abi_version: u16,
    /// Requested decode mode.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Effective decode mode after fallback, when execution remains allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Direct, fallback, or refused resolution state.
    pub selection_state: TassadarExecutorSelectionState,
    /// Stable reason for fallback or refusal when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_reason: Option<TassadarExecutorSelectionReason>,
    /// Human-readable summary safe for logs or UI.
    pub detail: String,
    /// Model-supported decode modes consulted during resolution when provided.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
}

impl TassadarExecutorSelectionDiagnostic {
    /// Returns whether the request resolved through an explicit fallback.
    #[must_use]
    pub const fn is_fallback(&self) -> bool {
        matches!(
            self.selection_state,
            TassadarExecutorSelectionState::Fallback
        )
    }

    /// Returns whether the request was refused before execution.
    #[must_use]
    pub const fn is_refused(&self) -> bool {
        matches!(
            self.selection_state,
            TassadarExecutorSelectionState::Refused
        )
    }
}

/// Execution report pairing one executed trace with its pre-execution selection diagnostic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorExecutionReport {
    /// Selection diagnostic emitted before execution.
    pub selection: TassadarExecutorSelectionDiagnostic,
    /// Resulting exact execution on the effective path.
    pub execution: TassadarExecution,
}

/// Summary-form execution report pairing the selection diagnostic with a
/// compact execution summary for long-horizon workloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorExecutionSummaryReport {
    /// Selection diagnostic emitted before execution.
    pub selection: TassadarExecutorSelectionDiagnostic,
    /// Resulting compact execution summary on the effective path.
    pub execution_summary: TassadarExecutionSummary,
}

/// Exactness posture recorded for one benchmark-bound Tassadar evidence report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExactnessPosture {
    /// The compared execution stayed exact against the declared reference.
    Exact,
    /// The compared execution completed but diverged from the declared reference.
    Mismatch,
    /// The requested lane was refused before trustworthy execution could complete.
    Refused,
}

/// Primary mismatch class surfaced by one exactness/refusal report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMismatchClass {
    /// Expected and actual traces diverged on one shared step.
    StepMismatch,
    /// The expected trace continued after the actual trace stopped.
    MissingActualStep,
    /// The actual trace continued after the expected trace stopped.
    UnexpectedActualStep,
    /// Final outputs diverged while the trace boundary remained comparable.
    FinalOutputMismatch,
    /// Halt reasons diverged while the trace boundary remained comparable.
    HaltReasonMismatch,
}

/// Stable mismatch summary for one exactness/refusal report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMismatchSummary {
    /// Primary mismatch class.
    pub mismatch_class: TassadarMismatchClass,
    /// Stable digest of the expected reference behavior.
    pub expected_behavior_digest: String,
    /// Stable digest of the observed behavior.
    pub actual_behavior_digest: String,
    /// First divergent step index when the mismatch came from trace comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_divergence_step_index: Option<usize>,
    /// Number of typed diff entries consulted during classification.
    pub trace_diff_entry_count: usize,
    /// Plain-language classification detail safe for logs or artifacts.
    pub detail: String,
}

/// Standardized exactness/refusal evidence report for one Tassadar request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactnessRefusalReport {
    /// Stable subject identifier for the report.
    pub subject_id: String,
    /// Requested decode mode.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Effective decode mode after selection, when execution remained allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Direct/fallback/refused selection state.
    pub selection_state: TassadarExecutorSelectionState,
    /// Typed selection reason when fallback or refusal occurred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_reason: Option<TassadarExecutorSelectionReason>,
    /// Exact, mismatch, or refused posture for the request.
    pub exactness_posture: TassadarExactnessPosture,
    /// Whether trace digests matched the declared reference.
    pub trace_digest_equal: bool,
    /// Whether final outputs matched the declared reference.
    pub outputs_equal: bool,
    /// Whether halt reasons matched the declared reference.
    pub halt_equal: bool,
    /// Reference behavior digest when a trusted reference existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_behavior_digest: Option<String>,
    /// Observed behavior digest when execution completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_behavior_digest: Option<String>,
    /// Typed mismatch summary when execution completed but diverged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mismatch_summary: Option<TassadarMismatchSummary>,
    /// Typed execution refusal when selection succeeded but execution still failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_refusal: Option<TassadarExecutionRefusal>,
    /// Plain-language report detail safe for logs or machine-audited artifacts.
    pub detail: String,
}

impl TassadarExactnessRefusalReport {
    /// Builds an exactness/refusal report from explicit selection plus completed execution.
    #[must_use]
    pub fn from_selection_and_execution(
        subject_id: impl Into<String>,
        selection: &TassadarExecutorSelectionDiagnostic,
        expected: &TassadarExecution,
        actual: &TassadarExecution,
    ) -> Self {
        let subject_id = subject_id.into();
        let trace_digest_equal = expected.trace_digest() == actual.trace_digest();
        let outputs_equal = expected.outputs == actual.outputs;
        let halt_equal = expected.halt_reason == actual.halt_reason;
        let mismatch_summary = if trace_digest_equal && outputs_equal && halt_equal {
            None
        } else {
            Some(classify_tassadar_mismatch(expected, actual))
        };
        let exactness_posture = if mismatch_summary.is_none() {
            TassadarExactnessPosture::Exact
        } else {
            TassadarExactnessPosture::Mismatch
        };
        let detail = if matches!(exactness_posture, TassadarExactnessPosture::Exact) {
            format!(
                "selection `{}` stayed exact for subject `{}` under requested decode `{}`",
                match selection.selection_state {
                    TassadarExecutorSelectionState::Direct => "direct",
                    TassadarExecutorSelectionState::Fallback => "fallback",
                    TassadarExecutorSelectionState::Refused => "refused",
                },
                subject_id,
                selection.requested_decode_mode.as_str()
            )
        } else {
            mismatch_summary
                .as_ref()
                .map(|summary| summary.detail.clone())
                .unwrap_or_else(|| String::from("execution diverged from the declared reference"))
        };
        Self {
            subject_id,
            requested_decode_mode: selection.requested_decode_mode,
            effective_decode_mode: selection.effective_decode_mode,
            selection_state: selection.selection_state,
            selection_reason: selection.selection_reason,
            exactness_posture,
            trace_digest_equal,
            outputs_equal,
            halt_equal,
            expected_behavior_digest: Some(expected.behavior_digest()),
            actual_behavior_digest: Some(actual.behavior_digest()),
            mismatch_summary,
            execution_refusal: None,
            detail,
        }
    }

    /// Builds an exactness/refusal report from an execution report plus trusted reference.
    #[must_use]
    pub fn from_execution_report(
        subject_id: impl Into<String>,
        expected: &TassadarExecution,
        execution_report: &TassadarExecutorExecutionReport,
    ) -> Self {
        Self::from_selection_and_execution(
            subject_id,
            &execution_report.selection,
            expected,
            &execution_report.execution,
        )
    }

    /// Builds a refusal report when selection or execution refused the request.
    #[must_use]
    pub fn from_refusal(
        subject_id: impl Into<String>,
        selection: &TassadarExecutorSelectionDiagnostic,
        execution_refusal: Option<TassadarExecutionRefusal>,
    ) -> Self {
        Self {
            subject_id: subject_id.into(),
            requested_decode_mode: selection.requested_decode_mode,
            effective_decode_mode: selection.effective_decode_mode,
            selection_state: TassadarExecutorSelectionState::Refused,
            selection_reason: selection.selection_reason,
            exactness_posture: TassadarExactnessPosture::Refused,
            trace_digest_equal: false,
            outputs_equal: false,
            halt_equal: false,
            expected_behavior_digest: None,
            actual_behavior_digest: None,
            mismatch_summary: None,
            detail: execution_refusal
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| selection.detail.clone()),
            execution_refusal,
        }
    }
}

fn classify_tassadar_mismatch(
    expected: &TassadarExecution,
    actual: &TassadarExecution,
) -> TassadarMismatchSummary {
    let diff = TassadarTraceDiffReport::from_executions(expected, actual);
    let expected_behavior_digest = expected.behavior_digest();
    let actual_behavior_digest = actual.behavior_digest();
    if let Some(entry) = diff.entries.first() {
        let mismatch_class = match entry.kind {
            TassadarTraceDiffKind::StepMismatch => TassadarMismatchClass::StepMismatch,
            TassadarTraceDiffKind::MissingActualStep => TassadarMismatchClass::MissingActualStep,
            TassadarTraceDiffKind::UnexpectedActualStep => {
                TassadarMismatchClass::UnexpectedActualStep
            }
        };
        return TassadarMismatchSummary {
            mismatch_class,
            expected_behavior_digest,
            actual_behavior_digest,
            first_divergence_step_index: diff.first_divergence_step_index,
            trace_diff_entry_count: diff.entries.len(),
            detail: format!(
                "trace mismatch for program `{}` at step {:?} (`{:?}`)",
                expected.program_id, diff.first_divergence_step_index, entry.kind
            ),
        };
    }
    if expected.outputs != actual.outputs {
        return TassadarMismatchSummary {
            mismatch_class: TassadarMismatchClass::FinalOutputMismatch,
            expected_behavior_digest,
            actual_behavior_digest,
            first_divergence_step_index: None,
            trace_diff_entry_count: 0,
            detail: format!(
                "final outputs diverged for program `{}`: expected {:?}, actual {:?}",
                expected.program_id, expected.outputs, actual.outputs
            ),
        };
    }
    TassadarMismatchSummary {
        mismatch_class: TassadarMismatchClass::HaltReasonMismatch,
        expected_behavior_digest,
        actual_behavior_digest,
        first_divergence_step_index: None,
        trace_diff_entry_count: 0,
        detail: format!(
            "halt reason diverged for program `{}`: expected {:?}, actual {:?}",
            expected.program_id, expected.halt_reason, actual.halt_reason
        ),
    }
}

/// Machine-legible supported WebAssembly-first profile for the Phase 1 lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWasmProfileId {
    /// Narrow i32-only profile with explicit host-side `output`.
    CoreI32V1,
    /// Widened i32-only profile for article-class exact executor benchmarks.
    CoreI32V2,
    /// W1.1 opcode-window ladder profile with stack and comparison opcodes.
    CoreI32W1_1V1,
    /// General article-shaped i32 profile spanning the current micro, Sudoku, and matching suite.
    ArticleI32ComputeV1,
    /// Larger i32-only profile for real 4x4 Sudoku-v0 search programs.
    SudokuV0SearchV1,
    /// Comparison-capable i32-only profile for bounded 4x4 matching programs.
    HungarianV0MatchingV1,
    /// Comparison-capable search-oriented profile for exact 10x10 Hungarian-class programs.
    Hungarian10x10MatchingV1,
    /// Larger i32-only profile for real 9x9 Sudoku-class search programs.
    Sudoku9x9SearchV1,
    /// Hard-suite 9x9 Sudoku search profile with a larger bounded step budget.
    Sudoku9x9HardSearchV1,
}

impl TassadarWasmProfileId {
    /// Returns the stable profile identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CoreI32V1 => "tassadar.wasm.core_i32.v1",
            Self::CoreI32V2 => "tassadar.wasm.core_i32.v2",
            Self::CoreI32W1_1V1 => "tassadar.wasm.core_i32_w1_1.v1",
            Self::ArticleI32ComputeV1 => "tassadar.wasm.article_i32_compute.v1",
            Self::SudokuV0SearchV1 => "tassadar.wasm.sudoku_v0_search.v1",
            Self::HungarianV0MatchingV1 => "tassadar.wasm.hungarian_v0_matching.v1",
            Self::Hungarian10x10MatchingV1 => "tassadar.wasm.hungarian_10x10_matching.v1",
            Self::Sudoku9x9SearchV1 => "tassadar.wasm.sudoku_9x9_search.v1",
            Self::Sudoku9x9HardSearchV1 => "tassadar.wasm.sudoku_9x9_hard_search.v1",
        }
    }
}

/// Condition semantics for the Phase 1 branch instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBranchMode {
    /// Pop one `i32`; branch when it is non-zero.
    BrIfNonZero,
}

/// Stable opcode set for the narrow WebAssembly-first profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarOpcode {
    /// Push one constant `i32`.
    I32Const,
    /// Push one local onto the stack.
    LocalGet,
    /// Pop one stack value into a local.
    LocalSet,
    /// Pop two `i32` values and push their sum.
    I32Add,
    /// Pop two `i32` values and push their difference.
    I32Sub,
    /// Pop two `i32` values and push their product.
    I32Mul,
    /// Pop two `i32` values and push `1` when `left < right`, otherwise `0`.
    I32Lt,
    /// Load one memory slot and push it.
    I32Load,
    /// Pop one stack value into one memory slot.
    I32Store,
    /// Pop one condition value and optionally branch to the target pc.
    BrIf,
    /// Pop one stack value and emit it through the lane output sink.
    Output,
    /// Halt execution successfully.
    Return,
    /// No-op instruction.
    Nop,
    /// Copy the top stack value into a local without popping it.
    LocalTee,
    /// Pop and discard one stack value.
    Drop,
    /// Pop one `i32` value and push `1` when it is zero, otherwise `0`.
    I32Eqz,
    /// Pop two `i32` values and push `1` when `left == right`, otherwise `0`.
    I32Eq,
    /// Pop two `i32` values and push `1` when `left != right`, otherwise `0`.
    I32Ne,
    /// Pop two `i32` values and push `1` when `left > right`, otherwise `0`.
    I32Gt,
    /// Pop two `i32` values and push `1` when `left <= right`, otherwise `0`.
    I32Le,
    /// Pop two `i32` values and push `1` when `left >= right`, otherwise `0`.
    I32Ge,
}

impl TassadarOpcode {
    /// Stable opcode ordering for the current article-shaped i32 profile.
    pub const ARTICLE_I32_V1: [Self; 12] = [
        Self::I32Const,
        Self::LocalGet,
        Self::LocalSet,
        Self::I32Add,
        Self::I32Sub,
        Self::I32Mul,
        Self::I32Lt,
        Self::I32Load,
        Self::I32Store,
        Self::BrIf,
        Self::Output,
        Self::Return,
    ];

    /// Stable opcode ordering for the W1.1 Wasm window ladder profile.
    pub const CORE_I32_W1_1_V1: [Self; 21] = [
        Self::I32Const,
        Self::LocalGet,
        Self::LocalSet,
        Self::I32Add,
        Self::I32Sub,
        Self::I32Mul,
        Self::I32Lt,
        Self::I32Load,
        Self::I32Store,
        Self::BrIf,
        Self::Output,
        Self::Return,
        Self::Nop,
        Self::LocalTee,
        Self::Drop,
        Self::I32Eqz,
        Self::I32Eq,
        Self::I32Ne,
        Self::I32Gt,
        Self::I32Le,
        Self::I32Ge,
    ];

    /// Stable opcode ordering used by fixtures and metadata digests.
    pub const ALL: [Self; 11] = [
        Self::I32Const,
        Self::LocalGet,
        Self::LocalSet,
        Self::I32Add,
        Self::I32Sub,
        Self::I32Mul,
        Self::I32Load,
        Self::I32Store,
        Self::BrIf,
        Self::Output,
        Self::Return,
    ];

    /// Stable opcode ordering for the bounded Hungarian matching profile.
    pub const HUNGARIAN_V0: [Self; 12] = Self::ARTICLE_I32_V1;

    /// Returns the stable opcode mnemonic.
    #[must_use]
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::Nop => "nop",
            Self::I32Const => "i32.const",
            Self::LocalGet => "local.get",
            Self::LocalSet => "local.set",
            Self::LocalTee => "local.tee",
            Self::Drop => "drop",
            Self::I32Add => "i32.add",
            Self::I32Sub => "i32.sub",
            Self::I32Mul => "i32.mul",
            Self::I32Lt => "i32.lt",
            Self::I32Eqz => "i32.eqz",
            Self::I32Eq => "i32.eq",
            Self::I32Ne => "i32.ne",
            Self::I32Gt => "i32.gt",
            Self::I32Le => "i32.le",
            Self::I32Ge => "i32.ge",
            Self::I32Load => "i32.load",
            Self::I32Store => "i32.store",
            Self::BrIf => "br_if",
            Self::Output => "output",
            Self::Return => "return",
        }
    }

    /// Returns a stable ordinal for fixture-weight encoding.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Nop => 12,
            Self::I32Const => 0,
            Self::LocalGet => 1,
            Self::LocalSet => 2,
            Self::LocalTee => 13,
            Self::Drop => 14,
            Self::I32Add => 3,
            Self::I32Sub => 4,
            Self::I32Mul => 5,
            Self::I32Lt => 6,
            Self::I32Eqz => 15,
            Self::I32Eq => 16,
            Self::I32Ne => 17,
            Self::I32Gt => 18,
            Self::I32Le => 19,
            Self::I32Ge => 20,
            Self::I32Load => 7,
            Self::I32Store => 8,
            Self::BrIf => 9,
            Self::Output => 10,
            Self::Return => 11,
        }
    }
}

/// Immediate families carried by the narrow profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarImmediateKind {
    /// No immediate payload.
    None,
    /// One `i32` immediate value.
    I32,
    /// One local-slot index.
    LocalIndex,
    /// One memory-slot index.
    MemorySlot,
    /// One validated branch target pc.
    BranchTarget,
}

impl TassadarImmediateKind {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::I32 => 1,
            Self::LocalIndex => 2,
            Self::MemorySlot => 3,
            Self::BranchTarget => 4,
        }
    }
}

/// Branch/control effect classification for one opcode rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarControlClass {
    /// Linear control flow.
    Linear,
    /// Conditional control flow.
    ConditionalBranch,
    /// Terminal control flow.
    Return,
}

impl TassadarControlClass {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Linear => 0,
            Self::ConditionalBranch => 1,
            Self::Return => 2,
        }
    }
}

/// Arithmetic class encoded by one opcode rule when applicable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArithmeticOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Less-than comparison returning `1` or `0`.
    Lt,
    /// Equality comparison returning `1` or `0`.
    Eq,
    /// Inequality comparison returning `1` or `0`.
    Ne,
    /// Greater-than comparison returning `1` or `0`.
    Gt,
    /// Less-than-or-equal comparison returning `1` or `0`.
    Le,
    /// Greater-than-or-equal comparison returning `1` or `0`.
    Ge,
}

impl TassadarArithmeticOp {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Add => 1,
            Self::Sub => 2,
            Self::Mul => 3,
            Self::Lt => 4,
            Self::Eq => 5,
            Self::Ne => 6,
            Self::Gt => 7,
            Self::Le => 8,
            Self::Ge => 9,
        }
    }
}

/// Unary class encoded by one opcode rule when applicable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarUnaryOp {
    /// Zero comparison returning `1` or `0`.
    Eqz,
}

impl TassadarUnaryOp {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Eqz => 1,
        }
    }
}

/// Memory and local access class for one opcode rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAccessClass {
    /// No side effect beyond stack/control behavior.
    None,
    /// Reads one local.
    LocalRead,
    /// Writes one local.
    LocalWrite,
    /// Reads one memory slot.
    MemoryRead,
    /// Writes one memory slot.
    MemoryWrite,
}

impl TassadarAccessClass {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::LocalRead => 1,
            Self::LocalWrite => 2,
            Self::MemoryRead => 3,
            Self::MemoryWrite => 4,
        }
    }
}

/// Machine-legible Phase 1 WebAssembly-first profile description.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmProfile {
    /// Stable profile identifier.
    pub profile_id: String,
    /// Allowed opcode set for this profile.
    pub allowed_opcodes: Vec<TassadarOpcode>,
    /// Maximum number of locals for the Phase 1 fixture.
    pub max_locals: usize,
    /// Maximum number of memory slots for the Phase 1 fixture.
    pub max_memory_slots: usize,
    /// Maximum instruction count accepted by the Phase 1 fixture.
    pub max_program_len: usize,
    /// Maximum execution steps before the runtime refuses the program.
    pub max_steps: usize,
    /// Branch semantics used by the profile.
    pub branch_mode: TassadarBranchMode,
    /// Whether the profile carries the host-side `output` helper opcode.
    pub host_output_opcode: bool,
}

impl TassadarWasmProfile {
    /// Returns the canonical Phase 1 profile.
    #[must_use]
    pub fn core_i32_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::CoreI32V1.as_str()),
            allowed_opcodes: TassadarOpcode::ALL.to_vec(),
            max_locals: 4,
            max_memory_slots: 8,
            max_program_len: 32,
            max_steps: 128,
            branch_mode: TassadarBranchMode::BrIfNonZero,
            host_output_opcode: true,
        }
    }

    /// Returns the widened article-class benchmark profile.
    #[must_use]
    pub fn core_i32_v2() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::CoreI32V2.as_str()),
            allowed_opcodes: TassadarOpcode::ALL.to_vec(),
            max_locals: 8,
            max_memory_slots: 16,
            max_program_len: 128,
            max_steps: 512,
            branch_mode: TassadarBranchMode::BrIfNonZero,
            host_output_opcode: true,
        }
    }

    /// Returns the W1.1 opcode-window ladder profile.
    #[must_use]
    pub fn core_i32_w1_1_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::CoreI32W1_1V1.as_str()),
            allowed_opcodes: TassadarOpcode::CORE_I32_W1_1_V1.to_vec(),
            max_locals: 8,
            max_memory_slots: 16,
            max_program_len: 256,
            max_steps: 1_024,
            branch_mode: TassadarBranchMode::BrIfNonZero,
            host_output_opcode: true,
        }
    }

    /// Returns the current article-shaped i32 profile used for the mixed
    /// article-class benchmark suite.
    #[must_use]
    pub fn article_i32_compute_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::ArticleI32ComputeV1.as_str()),
            allowed_opcodes: TassadarOpcode::ARTICLE_I32_V1.to_vec(),
            max_locals: 16,
            max_memory_slots: 64,
            max_program_len: 4_096,
            max_steps: 131_072,
            branch_mode: TassadarBranchMode::BrIfNonZero,
            host_output_opcode: true,
        }
    }

    /// Returns the larger search-oriented profile used for honest 4x4 Sudoku-v0 programs.
    #[must_use]
    pub fn sudoku_v0_search_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::SudokuV0SearchV1.as_str()),
            allowed_opcodes: TassadarOpcode::ALL.to_vec(),
            max_locals: 8,
            max_memory_slots: 32,
            max_program_len: 2_048,
            max_steps: 32_768,
            branch_mode: TassadarBranchMode::BrIfNonZero,
            host_output_opcode: true,
        }
    }

    /// Returns the bounded comparison-capable profile used for honest 4x4 matching programs.
    #[must_use]
    pub fn hungarian_v0_matching_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::HungarianV0MatchingV1.as_str()),
            allowed_opcodes: TassadarOpcode::ARTICLE_I32_V1.to_vec(),
            max_locals: 8,
            max_memory_slots: 32,
            max_program_len: 2_048,
            max_steps: 32_768,
            branch_mode: TassadarBranchMode::BrIfNonZero,
            host_output_opcode: true,
        }
    }

    /// Returns the larger comparison-capable profile for exact 10x10 Hungarian programs.
    #[must_use]
    pub fn hungarian_10x10_matching_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::Hungarian10x10MatchingV1.as_str()),
            allowed_opcodes: TassadarOpcode::ARTICLE_I32_V1.to_vec(),
            max_locals: 16,
            max_memory_slots: 64,
            max_program_len: 8_192,
            max_steps: 262_144,
            branch_mode: TassadarBranchMode::BrIfNonZero,
            host_output_opcode: true,
        }
    }

    /// Returns the larger search-oriented profile used for honest 9x9 Sudoku programs.
    #[must_use]
    pub fn sudoku_9x9_search_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::Sudoku9x9SearchV1.as_str()),
            allowed_opcodes: TassadarOpcode::ALL.to_vec(),
            max_locals: 8,
            max_memory_slots: 192,
            max_program_len: 12_288,
            max_steps: 1_048_576,
            branch_mode: TassadarBranchMode::BrIfNonZero,
            host_output_opcode: true,
        }
    }

    /// Returns the larger hard-suite profile used for named 9x9 article fixtures.
    #[must_use]
    pub fn sudoku_9x9_hard_search_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::Sudoku9x9HardSearchV1.as_str()),
            allowed_opcodes: TassadarOpcode::ALL.to_vec(),
            max_locals: 8,
            max_memory_slots: 192,
            max_program_len: 512,
            max_steps: 512,
            branch_mode: TassadarBranchMode::BrIfNonZero,
            host_output_opcode: true,
        }
    }

    /// Returns whether the profile explicitly supports one opcode.
    #[must_use]
    pub fn supports(&self, opcode: TassadarOpcode) -> bool {
        self.allowed_opcodes.contains(&opcode)
    }

    /// Returns a stable digest for the supported opcode vocabulary.
    #[must_use]
    pub fn opcode_vocabulary_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(TASSADAR_OPCODE_VOCABULARY_ID.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.profile_id.as_bytes());
        hasher.update(b"\n");
        for opcode in &self.allowed_opcodes {
            hasher.update(opcode.mnemonic().as_bytes());
            hasher.update(b"\n");
        }
        hex::encode(hasher.finalize())
    }
}

impl Default for TassadarWasmProfile {
    fn default() -> Self {
        Self::core_i32_v1()
    }
}

/// Returns one machine-legible WebAssembly profile by stable identifier.
#[must_use]
pub fn tassadar_wasm_profile_for_id(profile_id: &str) -> Option<TassadarWasmProfile> {
    match profile_id {
        value if value == TassadarWasmProfileId::CoreI32V1.as_str() => {
            Some(TassadarWasmProfile::core_i32_v1())
        }
        value if value == TassadarWasmProfileId::CoreI32V2.as_str() => {
            Some(TassadarWasmProfile::core_i32_v2())
        }
        value if value == TassadarWasmProfileId::CoreI32W1_1V1.as_str() => {
            Some(TassadarWasmProfile::core_i32_w1_1_v1())
        }
        value if value == TassadarWasmProfileId::ArticleI32ComputeV1.as_str() => {
            Some(TassadarWasmProfile::article_i32_compute_v1())
        }
        value if value == TassadarWasmProfileId::SudokuV0SearchV1.as_str() => {
            Some(TassadarWasmProfile::sudoku_v0_search_v1())
        }
        value if value == TassadarWasmProfileId::HungarianV0MatchingV1.as_str() => {
            Some(TassadarWasmProfile::hungarian_v0_matching_v1())
        }
        value if value == TassadarWasmProfileId::Hungarian10x10MatchingV1.as_str() => {
            Some(TassadarWasmProfile::hungarian_10x10_matching_v1())
        }
        value if value == TassadarWasmProfileId::Sudoku9x9SearchV1.as_str() => {
            Some(TassadarWasmProfile::sudoku_9x9_search_v1())
        }
        value if value == TassadarWasmProfileId::Sudoku9x9HardSearchV1.as_str() => {
            Some(TassadarWasmProfile::sudoku_9x9_hard_search_v1())
        }
        _ => None,
    }
}

/// Explicit append-only trace ABI for the Phase 1 fixture lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceAbi {
    /// Stable ABI identifier.
    pub abi_id: String,
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable profile identifier the ABI is paired with.
    pub profile_id: String,
    /// Whether traces are append-only.
    pub append_only: bool,
    /// Whether stack snapshots are emitted per step.
    pub includes_stack_snapshots: bool,
    /// Whether local snapshots are emitted per step.
    pub includes_local_snapshots: bool,
    /// Whether memory snapshots are emitted per step.
    pub includes_memory_snapshots: bool,
}

impl TassadarTraceAbi {
    /// Returns the canonical Phase 1 trace ABI.
    #[must_use]
    pub fn core_i32_v1() -> Self {
        Self {
            abi_id: String::from("tassadar.trace.v1"),
            schema_version: TASSADAR_TRACE_ABI_VERSION,
            profile_id: String::from(TassadarWasmProfileId::CoreI32V1.as_str()),
            append_only: true,
            includes_stack_snapshots: true,
            includes_local_snapshots: true,
            includes_memory_snapshots: true,
        }
    }

    /// Returns the widened article-class benchmark trace ABI.
    #[must_use]
    pub fn core_i32_v2() -> Self {
        Self {
            abi_id: String::from("tassadar.trace.v1"),
            schema_version: TASSADAR_TRACE_ABI_VERSION,
            profile_id: String::from(TassadarWasmProfileId::CoreI32V2.as_str()),
            append_only: true,
            includes_stack_snapshots: true,
            includes_local_snapshots: true,
            includes_memory_snapshots: true,
        }
    }

    /// Returns the W1.1 opcode-window ladder trace ABI.
    #[must_use]
    pub fn core_i32_w1_1_v1() -> Self {
        Self {
            abi_id: String::from("tassadar.trace.v1"),
            schema_version: TASSADAR_TRACE_ABI_VERSION,
            profile_id: String::from(TassadarWasmProfileId::CoreI32W1_1V1.as_str()),
            append_only: true,
            includes_stack_snapshots: true,
            includes_local_snapshots: true,
            includes_memory_snapshots: true,
        }
    }

    /// Returns the trace ABI for the current article-shaped i32 profile.
    #[must_use]
    pub fn article_i32_compute_v1() -> Self {
        Self {
            abi_id: String::from("tassadar.trace.v1"),
            schema_version: TASSADAR_TRACE_ABI_VERSION,
            profile_id: String::from(TassadarWasmProfileId::ArticleI32ComputeV1.as_str()),
            append_only: true,
            includes_stack_snapshots: true,
            includes_local_snapshots: true,
            includes_memory_snapshots: true,
        }
    }

    /// Returns the search-oriented trace ABI for the honest Sudoku-v0 profile.
    #[must_use]
    pub fn sudoku_v0_search_v1() -> Self {
        Self {
            abi_id: String::from("tassadar.trace.v1"),
            schema_version: TASSADAR_TRACE_ABI_VERSION,
            profile_id: String::from(TassadarWasmProfileId::SudokuV0SearchV1.as_str()),
            append_only: true,
            includes_stack_snapshots: true,
            includes_local_snapshots: true,
            includes_memory_snapshots: true,
        }
    }

    /// Returns the trace ABI for the honest bounded 4x4 Hungarian matching profile.
    #[must_use]
    pub fn hungarian_v0_matching_v1() -> Self {
        Self {
            abi_id: String::from("tassadar.trace.v1"),
            schema_version: TASSADAR_TRACE_ABI_VERSION,
            profile_id: String::from(TassadarWasmProfileId::HungarianV0MatchingV1.as_str()),
            append_only: true,
            includes_stack_snapshots: true,
            includes_local_snapshots: true,
            includes_memory_snapshots: true,
        }
    }

    /// Returns the trace ABI for the exact 10x10 Hungarian matching profile.
    #[must_use]
    pub fn hungarian_10x10_matching_v1() -> Self {
        Self {
            abi_id: String::from("tassadar.trace.v1"),
            schema_version: TASSADAR_TRACE_ABI_VERSION,
            profile_id: String::from(TassadarWasmProfileId::Hungarian10x10MatchingV1.as_str()),
            append_only: true,
            includes_stack_snapshots: true,
            includes_local_snapshots: true,
            includes_memory_snapshots: true,
        }
    }

    /// Returns the search-oriented trace ABI for the honest 9x9 Sudoku profile.
    #[must_use]
    pub fn sudoku_9x9_search_v1() -> Self {
        Self {
            abi_id: String::from("tassadar.trace.v1"),
            schema_version: TASSADAR_TRACE_ABI_VERSION,
            profile_id: String::from(TassadarWasmProfileId::Sudoku9x9SearchV1.as_str()),
            append_only: true,
            includes_stack_snapshots: true,
            includes_local_snapshots: true,
            includes_memory_snapshots: true,
        }
    }

    /// Returns the trace ABI for the hard-suite 9x9 Sudoku search profile.
    #[must_use]
    pub fn sudoku_9x9_hard_search_v1() -> Self {
        Self {
            abi_id: String::from("tassadar.trace.v1"),
            schema_version: TASSADAR_TRACE_ABI_VERSION,
            profile_id: String::from(TassadarWasmProfileId::Sudoku9x9HardSearchV1.as_str()),
            append_only: true,
            includes_stack_snapshots: true,
            includes_local_snapshots: true,
            includes_memory_snapshots: true,
        }
    }

    /// Returns a stable digest over the ABI compatibility surface.
    #[must_use]
    pub fn compatibility_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.abi_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.schema_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.profile_id.as_bytes());
        hasher.update(b"\n");
        hasher.update([
            u8::from(self.append_only),
            u8::from(self.includes_stack_snapshots),
            u8::from(self.includes_local_snapshots),
            u8::from(self.includes_memory_snapshots),
        ]);
        hex::encode(hasher.finalize())
    }
}

impl Default for TassadarTraceAbi {
    fn default() -> Self {
        Self::core_i32_v1()
    }
}

/// Returns the canonical append-only trace ABI for one supported profile id.
#[must_use]
pub fn tassadar_trace_abi_for_profile_id(profile_id: &str) -> Option<TassadarTraceAbi> {
    match profile_id {
        value if value == TassadarWasmProfileId::CoreI32V1.as_str() => {
            Some(TassadarTraceAbi::core_i32_v1())
        }
        value if value == TassadarWasmProfileId::CoreI32V2.as_str() => {
            Some(TassadarTraceAbi::core_i32_v2())
        }
        value if value == TassadarWasmProfileId::CoreI32W1_1V1.as_str() => {
            Some(TassadarTraceAbi::core_i32_w1_1_v1())
        }
        value if value == TassadarWasmProfileId::ArticleI32ComputeV1.as_str() => {
            Some(TassadarTraceAbi::article_i32_compute_v1())
        }
        value if value == TassadarWasmProfileId::SudokuV0SearchV1.as_str() => {
            Some(TassadarTraceAbi::sudoku_v0_search_v1())
        }
        value if value == TassadarWasmProfileId::HungarianV0MatchingV1.as_str() => {
            Some(TassadarTraceAbi::hungarian_v0_matching_v1())
        }
        value if value == TassadarWasmProfileId::Hungarian10x10MatchingV1.as_str() => {
            Some(TassadarTraceAbi::hungarian_10x10_matching_v1())
        }
        value if value == TassadarWasmProfileId::Sudoku9x9SearchV1.as_str() => {
            Some(TassadarTraceAbi::sudoku_9x9_search_v1())
        }
        value if value == TassadarWasmProfileId::Sudoku9x9HardSearchV1.as_str() => {
            Some(TassadarTraceAbi::sudoku_9x9_hard_search_v1())
        }
        _ => None,
    }
}

/// Canonical machine-readable output path for the Wasm instruction-coverage report.
pub const TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_wasm_instruction_coverage_report.json";
/// Stable benchmark ref for the widened article-class suite.
pub const TASSADAR_ARTICLE_CLASS_BENCHMARK_REF: &str =
    "benchmark://openagents/tassadar/article_class/reference_fixture";
/// Stable environment ref for the widened article-class benchmark package.
pub const TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF: &str =
    "env.openagents.tassadar.article_class.benchmark";
/// Canonical machine-readable output path for the article-class benchmark report.
pub const TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_class_benchmark_report.json";
/// Canonical machine-readable output path for the trace-ABI decision report.
pub const TASSADAR_TRACE_ABI_DECISION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_trace_abi_decision_report.json";
/// Canonical fixture root for the article-class long-horizon trace-ABI exemplar.
pub const TASSADAR_LONG_HORIZON_TRACE_FIXTURE_ROOT_REF: &str =
    "fixtures/tassadar/runs/long_loop_kernel_trace_abi_v0";
/// Canonical file name for the long-horizon execution evidence bundle.
pub const TASSADAR_LONG_HORIZON_TRACE_EVIDENCE_BUNDLE_FILE: &str = "execution_evidence_bundle.json";
/// Canonical fixture root for the million-step decode benchmark bundle.
pub const TASSADAR_MILLION_STEP_BENCHMARK_ROOT_REF: &str =
    "fixtures/tassadar/runs/million_step_loop_benchmark_v0";
/// Canonical file name for the million-step decode benchmark bundle.
pub const TASSADAR_MILLION_STEP_BENCHMARK_BUNDLE_FILE: &str = "benchmark_bundle.json";
/// Current schema version for the trace-ABI decision report.
pub const TASSADAR_TRACE_ABI_DECISION_SCHEMA_VERSION: u16 = 1;
/// Current schema version for the million-step decode benchmark bundle.
pub const TASSADAR_MILLION_STEP_BENCHMARK_SCHEMA_VERSION: u16 = 1;

const TASSADAR_LONG_HORIZON_TRACE_CASE_ID: &str = "long_loop_kernel";
const TASSADAR_MILLION_STEP_LOOP_ITERATION_COUNT: i32 = 131_071;
const TASSADAR_MILLION_STEP_LOOP_EXPECTED_STEPS: u64 = 1_048_575;

/// One coverage row for one supported Tassadar Wasm profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmProfileCoverage {
    /// Stable profile identifier.
    pub profile_id: String,
    /// Stable opcode-vocabulary digest.
    pub opcode_vocabulary_digest: String,
    /// Supported opcode set.
    pub supported_opcodes: Vec<TassadarOpcode>,
    /// Unsupported opcode set relative to the current article-shaped i32 coverage vocabulary.
    pub unsupported_opcodes: Vec<TassadarOpcode>,
    /// Maximum supported locals.
    pub max_locals: usize,
    /// Maximum supported memory slots.
    pub max_memory_slots: usize,
    /// Maximum supported program length.
    pub max_program_len: usize,
    /// Maximum supported step count.
    pub max_steps: usize,
    /// Whether this profile is the current article-shaped mixed-workload profile.
    pub article_profile: bool,
    /// Current committed case ids that exercise this profile.
    pub current_case_ids: Vec<String>,
    /// Current committed workload families that exercise this profile.
    pub current_workload_targets: Vec<String>,
    /// Plain-language claim boundary for this profile.
    pub claim_boundary: String,
}

/// One typed refusal example proving that unsupported instructions remain explicit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmCoverageRefusalExample {
    /// Stable profile identifier used for the probe.
    pub profile_id: String,
    /// Stable probe program identifier.
    pub probe_program_id: String,
    /// Opcode requested by the probe.
    pub attempted_opcode: TassadarOpcode,
    /// Typed refusal observed from the runtime validator.
    pub refusal: TassadarExecutionRefusal,
}

/// Machine-readable instruction/profile coverage report for the current
/// Tassadar Wasm profile set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmInstructionCoverageReport {
    /// Runtime backend advertising the profile set.
    pub runtime_backend: String,
    /// Stable coverage-opcode universe for the current article-shaped i32 executor vocabulary.
    pub coverage_opcode_universe: Vec<TassadarOpcode>,
    /// Stable profile identifier for the current article-shaped mixed-workload profile.
    pub article_profile_id: String,
    /// Ordered profile coverage rows.
    pub profiles: Vec<TassadarWasmProfileCoverage>,
    /// Typed refusal examples showing unsupported instructions stay explicit.
    pub refusal_examples: Vec<TassadarWasmCoverageRefusalExample>,
    /// Plain-language note keeping the report honest.
    pub outcome_statement: String,
    /// Stable report digest.
    pub report_digest: String,
}

impl TassadarWasmInstructionCoverageReport {
    fn new(
        profiles: Vec<TassadarWasmProfileCoverage>,
        refusal_examples: Vec<TassadarWasmCoverageRefusalExample>,
    ) -> Self {
        let mut report = Self {
            runtime_backend: String::from(TASSADAR_RUNTIME_BACKEND_ID),
            coverage_opcode_universe: TassadarOpcode::ARTICLE_I32_V1.to_vec(),
            article_profile_id: String::from(TassadarWasmProfileId::ArticleI32ComputeV1.as_str()),
            profiles,
            refusal_examples,
            outcome_statement: String::from(
                "coverage is explicit over the current Tassadar i32 executor vocabulary only; unsupported opcodes still refuse by profile and this is not arbitrary Wasm closure",
            ),
            report_digest: String::new(),
        };
        report.report_digest = hex::encode(Sha256::digest(
            serde_json::to_vec(&report).unwrap_or_default(),
        ));
        report
    }
}

/// Machine-authority contract for one persisted Tassadar trace ABI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceAbiAuthorityContract {
    /// Canonical machine-truth artifact kind.
    pub canonical_machine_truth_artifact_kind: String,
    /// Ordered machine-truth fields validators must treat as authoritative.
    pub canonical_machine_truth_fields: Vec<String>,
    /// Human-readable log posture relative to machine truth.
    pub readable_log_posture: String,
    /// Allowed readable-log differences that do not change machine truth.
    pub readable_log_allowed_variations: Vec<String>,
}

/// Explicit versioning rules for one persisted Tassadar trace ABI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceAbiVersioningContract {
    /// Changes that may happen without changing the trace ABI version.
    pub compatible_without_abi_bump: Vec<String>,
    /// Changes that require a new trace ABI version.
    pub requires_abi_bump: Vec<String>,
}

/// One validator-facing note describing where trace ABI identity is recorded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceAbiValidatorCompatibilityNote {
    /// Repo-owned artifact ref validators should inspect.
    pub artifact_ref: String,
    /// Artifact kind carrying the ABI identity.
    pub artifact_kind: String,
    /// JSON pointer to the ABI id field.
    pub abi_id_pointer: String,
    /// JSON pointer to the ABI version field.
    pub abi_version_pointer: String,
    /// Plain-language compatibility statement for validators.
    pub compatibility_expectation: String,
}

/// Summary of one committed long-horizon trace fixture proving the ABI can
/// carry article-class traces without changing identity semantics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLongHorizonTraceFixtureSummary {
    /// Fixture root carrying the persisted exemplar.
    pub fixture_root_ref: String,
    /// Repo-owned evidence-bundle path for the exemplar.
    pub evidence_bundle_ref: String,
    /// Stable case identifier.
    pub case_id: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable workload family.
    pub workload_target: String,
    /// Stable trace ABI identifier carried by the exemplar.
    pub trace_abi_id: String,
    /// Stable trace ABI version carried by the exemplar.
    pub trace_abi_version: u16,
    /// Stable compatibility digest for the ABI contract.
    pub trace_abi_compatibility_digest: String,
    /// Number of realized trace steps in the exemplar.
    pub step_count: u64,
    /// Stable trace-artifact identifier.
    pub trace_artifact_id: String,
    /// Stable trace-artifact digest.
    pub trace_artifact_digest: String,
    /// Stable trace-proof identifier.
    pub trace_proof_id: String,
    /// Stable trace-proof digest.
    pub trace_proof_digest: String,
}

/// Machine-readable report capturing the deliberate long-horizon trace ABI
/// posture for the current article-class executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceAbiDecisionReport {
    /// Stable schema version for the decision report itself.
    pub schema_version: u16,
    /// Canonical ABI the article-class long-horizon lane uses today.
    pub canonical_trace_abi: TassadarTraceAbi,
    /// Stable digest over the ABI compatibility surface.
    pub trace_abi_compatibility_digest: String,
    /// Contract separating machine truth from readable logs.
    pub authority_contract: TassadarTraceAbiAuthorityContract,
    /// Contract describing when ABI version changes are required.
    pub versioning_contract: TassadarTraceAbiVersioningContract,
    /// Validator-facing notes binding ABI identity into persisted artifacts.
    pub validator_compatibility_notes: Vec<TassadarTraceAbiValidatorCompatibilityNote>,
    /// Long-horizon article-class exemplar proving the ABI can carry a large trace.
    pub long_horizon_fixture: TassadarLongHorizonTraceFixtureSummary,
    /// Plain-language boundary statement for the current ABI posture.
    pub claim_boundary: String,
    /// Stable digest over the full decision report.
    pub report_digest: String,
}

impl TassadarTraceAbiDecisionReport {
    fn new(
        canonical_trace_abi: TassadarTraceAbi,
        long_horizon_fixture: TassadarLongHorizonTraceFixtureSummary,
    ) -> Self {
        let validator_compatibility_notes = vec![
            TassadarTraceAbiValidatorCompatibilityNote {
                artifact_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
                artifact_kind: String::from("tassadar_article_class_benchmark_report.json"),
                abi_id_pointer: String::from(
                    "/suite/environment_bundle/program_binding/trace_abi_id",
                ),
                abi_version_pointer: String::from(
                    "/suite/environment_bundle/program_binding/trace_abi_version",
                ),
                compatibility_expectation: String::from(
                    "article-class benchmark validators read the ABI from the environment-bundle program binding and must keep that machine truth aligned with the runtime trace artifact",
                ),
            },
            TassadarTraceAbiValidatorCompatibilityNote {
                artifact_ref: String::from(
                    "fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0/compiled_weight_suite_artifact.json",
                ),
                artifact_kind: String::from("tassadar_compiled_weight_suite_artifact.json"),
                abi_id_pointer: String::from("/deployments/0/trace_abi_id"),
                abi_version_pointer: String::from("/deployments/0/trace_abi_version"),
                compatibility_expectation: String::from(
                    "compiled Sudoku validators keep runtime-contract and deployment ABI identity stable under the same trace ABI family",
                ),
            },
            TassadarTraceAbiValidatorCompatibilityNote {
                artifact_ref: String::from(
                    "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/compiled_weight_suite_artifact.json",
                ),
                artifact_kind: String::from("tassadar_compiled_weight_suite_artifact.json"),
                abi_id_pointer: String::from("/deployments/0/trace_abi_id"),
                abi_version_pointer: String::from("/deployments/0/trace_abi_version"),
                compatibility_expectation: String::from(
                    "compiled Hungarian validators keep deployment ABI identity stable under the same trace ABI family",
                ),
            },
            TassadarTraceAbiValidatorCompatibilityNote {
                artifact_ref: long_horizon_fixture.evidence_bundle_ref.clone(),
                artifact_kind: String::from("tassadar_execution_evidence_bundle.json"),
                abi_id_pointer: String::from("/trace_artifact/trace_abi_id"),
                abi_version_pointer: String::from("/trace_artifact/trace_abi_version"),
                compatibility_expectation: String::from(
                    "the long-horizon exemplar records the same ABI identity directly on the emitted trace artifact while keeping readable-log concerns out of the authority layer",
                ),
            },
        ];
        let mut report = Self {
            schema_version: TASSADAR_TRACE_ABI_DECISION_SCHEMA_VERSION,
            trace_abi_compatibility_digest: canonical_trace_abi.compatibility_digest(),
            canonical_trace_abi,
            authority_contract: TassadarTraceAbiAuthorityContract {
                canonical_machine_truth_artifact_kind: String::from("tassadar_trace_artifact.json"),
                canonical_machine_truth_fields: vec![
                    String::from("trace_abi_id"),
                    String::from("trace_abi_version"),
                    String::from("trace_digest"),
                    String::from("behavior_digest"),
                    String::from("step_count"),
                    String::from("steps"),
                ],
                readable_log_posture: String::from(
                    "readable logs are derived, non-authoritative views over the canonical append-only trace artifact and may be sampled, truncated, or reformatted without changing machine truth",
                ),
                readable_log_allowed_variations: vec![
                    String::from("line wrapping or indentation"),
                    String::from("summary headers and progress annotations"),
                    String::from("bounded previews or truncation for very long traces"),
                ],
            },
            versioning_contract: TassadarTraceAbiVersioningContract {
                compatible_without_abi_bump: vec![
                    String::from(
                        "adding new derived reports that reference the same trace artifact",
                    ),
                    String::from("changing readable-log formatting or truncation policy"),
                    String::from(
                        "adding workload families that keep the same step/event semantics",
                    ),
                ],
                requires_abi_bump: vec![
                    String::from("changing step ordering or append-only semantics"),
                    String::from("changing the meaning or encoding of existing trace events"),
                    String::from(
                        "changing whether stack, local, or memory snapshots are required per step",
                    ),
                    String::from(
                        "changing digest inputs used by validators to establish trace identity",
                    ),
                ],
            },
            validator_compatibility_notes,
            long_horizon_fixture,
            claim_boundary: String::from(
                "the current long-horizon ABI posture freezes `tassadar.trace.v1` as the canonical machine-truth trace for article-class execution; readable logs remain derived views and the separate million-step decode closure is still not implied by this report",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_serialized_digest(b"tassadar_trace_abi_decision_report|", &report);
        report
    }
}

/// Measured vs selection-only posture for one million-step decode-mode receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMillionStepMeasurementPosture {
    /// Throughput was measured directly at the declared million-step horizon.
    Measured,
    /// Only compatibility and selection truth were recorded at this horizon.
    SelectionOnly,
}

/// Compact execution summary for very long-horizon Tassadar runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionSummary {
    /// Stable program identifier.
    pub program_id: String,
    /// Stable profile identifier.
    pub profile_id: String,
    /// Stable runner identifier.
    pub runner_id: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI version.
    pub trace_abi_version: u16,
    /// Stable trace digest over the append-only step stream.
    pub trace_digest: String,
    /// Stable digest over trace plus terminal state.
    pub behavior_digest: String,
    /// Number of executed steps.
    pub step_count: u64,
    /// Exact serialized byte count for the append-only step stream.
    pub serialized_trace_bytes: u64,
    /// Final emitted outputs.
    pub outputs: Vec<i32>,
    /// Final locals snapshot.
    pub final_locals: Vec<i32>,
    /// Final memory snapshot.
    pub final_memory: Vec<i32>,
    /// Final stack snapshot.
    pub final_stack: Vec<i32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarHaltReason,
}

/// Compact trace artifact for very long-horizon runs where persisting every
/// step would be impractical.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceSummaryArtifact {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable digest over the summary artifact.
    pub artifact_digest: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable runner identifier.
    pub runner_id: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI version.
    pub trace_abi_version: u16,
    /// Stable trace digest over the append-only step stream.
    pub trace_digest: String,
    /// Stable behavior digest.
    pub behavior_digest: String,
    /// Number of executed steps.
    pub step_count: u64,
    /// Exact serialized byte count for the append-only step stream.
    pub serialized_trace_bytes: u64,
    /// Final outputs carried by the summary artifact.
    pub outputs: Vec<i32>,
    /// Final locals snapshot.
    pub final_locals: Vec<i32>,
    /// Final memory snapshot.
    pub final_memory: Vec<i32>,
    /// Final stack snapshot.
    pub final_stack: Vec<i32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarHaltReason,
}

/// Summary-form evidence bundle for very long-horizon executions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionSummaryEvidenceBundle {
    /// Digest-bound runtime manifest for the execution lane.
    pub runtime_manifest: RuntimeManifest,
    /// Compact trace summary artifact.
    pub trace_summary_artifact: TassadarTraceSummaryArtifact,
    /// Proof-bearing trace summary receipt.
    pub trace_summary_proof: TassadarTraceSummaryProofReceipt,
    /// Canonical Psionic proof bundle carrying the execution identity.
    pub proof_bundle: ExecutionProofBundle,
}

/// Proof-bearing receipt for a compact trace summary artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceSummaryProofReceipt {
    /// Stable proof receipt identifier.
    pub proof_receipt_id: String,
    /// Stable digest over the proof receipt.
    pub proof_receipt_digest: String,
    /// Stable trace-summary-artifact reference.
    pub trace_artifact_ref: String,
    /// Stable trace-summary-artifact digest.
    pub trace_artifact_digest: String,
    /// Stable trace digest.
    pub trace_digest: String,
    /// Stable validated-program digest.
    pub program_digest: String,
    /// Stable program-artifact digest.
    pub program_artifact_digest: String,
    /// Stable Wasm profile identifier.
    pub wasm_profile_id: String,
    /// Runtime backend that realized the trace.
    pub runtime_backend: String,
    /// Stable runner identifier.
    pub runner_id: String,
    /// Validation reference carried with the receipt.
    pub validation: ValidationMatrixReference,
    /// Stable runtime-manifest identity digest.
    pub runtime_manifest_identity_digest: String,
    /// Stable runtime-manifest digest.
    pub runtime_manifest_digest: String,
}

/// One requested decode-mode receipt in the million-step benchmark bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarMillionStepDecodeModeReceipt {
    /// Requested decode mode.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Direct/fallback/refused selection state at the million-step horizon.
    pub selection_state: TassadarExecutorSelectionState,
    /// Stable reason for fallback or refusal when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_reason: Option<TassadarExecutorSelectionReason>,
    /// Effective decode mode after selection when execution remains allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Whether throughput was directly measured or only selection truth was recorded.
    pub measurement_posture: TassadarMillionStepMeasurementPosture,
    /// Direct throughput receipt when measurement occurred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps_per_second: Option<f64>,
    /// Remaining CPU gap ratio when measurement occurred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_gap_vs_cpu_reference: Option<f64>,
    /// Plain-language note for the receipt.
    pub note: String,
}

/// Machine-readable million-step decode benchmark bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarMillionStepDecodeBenchmarkBundle {
    /// Stable schema version.
    pub schema_version: u16,
    /// Canonical bundle root.
    pub bundle_root_ref: String,
    /// Stable workload-family identifier.
    pub workload_family_id: String,
    /// Stable profile identifier used for the benchmark family.
    pub wasm_profile_id: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI version.
    pub trace_abi_version: u16,
    /// Loop iteration count used to cross one million steps.
    pub iteration_count: u64,
    /// Expected exact step count for the benchmark family.
    pub expected_step_count: u64,
    /// Exact summary from the measured CPU-reference execution.
    pub cpu_reference_summary: TassadarExecutionSummary,
    /// Direct CPU-reference throughput at the million-step horizon.
    pub cpu_reference_steps_per_second: f64,
    /// Whether the CPU-reference summary cleared the declared exactness bar.
    pub exactness_bps: u32,
    /// Stable memory-growth metric identifier carried by the bundle.
    pub memory_growth_metric: String,
    /// Exact summary-form byte growth for the realized append-only trace.
    pub memory_growth_bytes: u64,
    /// Stable runtime-manifest identity digest for the measured execution mode.
    pub runtime_manifest_identity_digest: String,
    /// Requested reference-linear receipt.
    pub reference_linear: TassadarMillionStepDecodeModeReceipt,
    /// Requested HullCache receipt.
    pub hull_cache: TassadarMillionStepDecodeModeReceipt,
    /// Requested SparseTopK receipt.
    pub sparse_top_k: TassadarMillionStepDecodeModeReceipt,
    /// Runtime capability report anchoring the benchmark bundle.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Proof-bearing execution lineage for the measured CPU-reference run.
    pub evidence_bundle: TassadarExecutionSummaryEvidenceBundle,
    /// Plain-language boundary statement for the current bundle.
    pub claim_boundary: String,
    /// Stable digest over the full bundle.
    pub bundle_digest: String,
}

/// Errors while building or writing the long-horizon trace-ABI artifacts.
#[derive(Debug, Error)]
pub enum TassadarTraceAbiArtifactError {
    /// The canonical long-horizon article-class case was missing.
    #[error("missing canonical long-horizon article-class case `{case_id}`")]
    MissingLongHorizonCase { case_id: String },
    /// The canonical long-horizon case referenced an unsupported profile.
    #[error("unsupported profile `{profile_id}` for canonical long-horizon trace ABI fixture")]
    UnsupportedProfile { profile_id: String },
    /// The realized CPU-reference execution diverged from the preserved case truth.
    #[error(
        "canonical long-horizon fixture `{case_id}` diverged from preserved CPU-reference truth"
    )]
    FixtureMismatch { case_id: String },
    /// Program-artifact projection failed.
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
    /// Execution failed.
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Artifact persistence failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Errors while building or writing the million-step decode benchmark bundle.
#[derive(Debug, Error)]
pub enum TassadarMillionStepBenchmarkError {
    /// Program-artifact projection failed.
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
    /// Execution failed.
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    /// JSON serialization failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Artifact persistence failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn runtime_repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Returns the canonical absolute path for the Wasm instruction-coverage report.
#[must_use]
pub fn tassadar_wasm_instruction_coverage_report_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF)
}

/// Writes the canonical machine-readable Wasm instruction/profile coverage report.
pub fn write_tassadar_wasm_instruction_coverage_report(
    output_path: impl AsRef<std::path::Path>,
) -> Result<TassadarWasmInstructionCoverageReport, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let report = tassadar_wasm_instruction_coverage_report();
    let bytes = serde_json::to_vec_pretty(&report)
        .expect("Tassadar Wasm instruction coverage report should serialize");
    std::fs::write(output_path, bytes)?;
    Ok(report)
}

/// Returns the canonical absolute path for the trace-ABI decision report.
#[must_use]
pub fn tassadar_trace_abi_decision_report_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_TRACE_ABI_DECISION_REPORT_REF)
}

/// Returns the canonical absolute path for the long-horizon trace fixture root.
#[must_use]
pub fn tassadar_long_horizon_trace_fixture_root_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_LONG_HORIZON_TRACE_FIXTURE_ROOT_REF)
}

/// Returns the canonical absolute path for the long-horizon evidence bundle.
#[must_use]
pub fn tassadar_long_horizon_trace_evidence_bundle_path() -> std::path::PathBuf {
    tassadar_long_horizon_trace_fixture_root_path()
        .join(TASSADAR_LONG_HORIZON_TRACE_EVIDENCE_BUNDLE_FILE)
}

/// Returns the canonical absolute path for the million-step benchmark root.
#[must_use]
pub fn tassadar_million_step_benchmark_root_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_MILLION_STEP_BENCHMARK_ROOT_REF)
}

/// Returns the canonical absolute path for the million-step benchmark bundle.
#[must_use]
pub fn tassadar_million_step_benchmark_bundle_path() -> std::path::PathBuf {
    tassadar_million_step_benchmark_root_path().join(TASSADAR_MILLION_STEP_BENCHMARK_BUNDLE_FILE)
}

fn tassadar_million_step_loop_program() -> TassadarProgram {
    let profile = TassadarWasmProfile::sudoku_9x9_search_v1();
    TassadarProgram::new(
        "tassadar.million_step_backward_branch_loop.v1",
        &profile,
        1,
        0,
        vec![
            TassadarInstruction::I32Const {
                value: TASSADAR_MILLION_STEP_LOOP_ITERATION_COUNT,
            },
            TassadarInstruction::LocalSet { local: 0 },
            TassadarInstruction::LocalGet { local: 0 },
            TassadarInstruction::BrIf { target_pc: 7 },
            TassadarInstruction::I32Const { value: 0 },
            TassadarInstruction::Output,
            TassadarInstruction::Return,
            TassadarInstruction::LocalGet { local: 0 },
            TassadarInstruction::I32Const { value: 1 },
            TassadarInstruction::I32Sub,
            TassadarInstruction::LocalSet { local: 0 },
            TassadarInstruction::I32Const { value: 1 },
            TassadarInstruction::BrIf { target_pc: 2 },
        ],
    )
}

fn benchmark_summary_steps_per_second<F>(
    steps_per_run: u64,
    mut runner: F,
) -> Result<f64, TassadarMillionStepBenchmarkError>
where
    F: FnMut() -> Result<TassadarExecutionSummary, TassadarExecutionRefusal>,
{
    let normalized_steps = steps_per_run.max(1);
    let target_steps = normalized_steps.saturating_mul(2);
    let started = std::time::Instant::now();
    let mut total_steps = 0u64;

    loop {
        runner()?;
        total_steps = total_steps.saturating_add(normalized_steps);
        let elapsed = started.elapsed().as_secs_f64();
        if total_steps >= target_steps || elapsed >= 0.020 {
            return Ok(total_steps as f64 / elapsed.max(1e-9));
        }
    }
}

/// Builds the canonical million-step decode benchmark bundle.
pub fn build_tassadar_million_step_decode_benchmark_bundle()
-> Result<TassadarMillionStepDecodeBenchmarkBundle, TassadarMillionStepBenchmarkError> {
    let profile = TassadarWasmProfile::sudoku_9x9_search_v1();
    let trace_abi = TassadarTraceAbi::sudoku_9x9_search_v1();
    let program = tassadar_million_step_loop_program();
    let program_artifact = TassadarProgramArtifact::fixture_reference(
        "tassadar://artifact/million_step_loop_benchmark/program",
        &profile,
        &trace_abi,
        program.clone(),
    )?;
    let cpu_reference_summary = execute_program_direct_summary(
        &program,
        &profile,
        &trace_abi,
        TASSADAR_CPU_REFERENCE_RUNNER_ID,
    )?;
    let cpu_reference_steps_per_second =
        benchmark_summary_steps_per_second(cpu_reference_summary.step_count, || {
            execute_program_direct_summary(
                &program,
                &profile,
                &trace_abi,
                TASSADAR_CPU_REFERENCE_RUNNER_ID,
            )
        })?;
    let exactness_bps = u32::from(
        cpu_reference_summary.step_count == TASSADAR_MILLION_STEP_LOOP_EXPECTED_STEPS
            && cpu_reference_summary.outputs == vec![0]
            && cpu_reference_summary.halt_reason == TassadarHaltReason::Returned,
    ) * 10_000;
    let memory_growth_bytes = cpu_reference_summary.serialized_trace_bytes;

    let reference_linear_selection = diagnose_tassadar_executor_request(
        &program,
        TassadarExecutorDecodeMode::ReferenceLinear,
        trace_abi.schema_version,
        Some(&[
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarExecutorDecodeMode::HullCache,
            TassadarExecutorDecodeMode::SparseTopK,
        ]),
    );
    let hull_cache_selection = diagnose_tassadar_executor_request(
        &program,
        TassadarExecutorDecodeMode::HullCache,
        trace_abi.schema_version,
        Some(&[
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarExecutorDecodeMode::HullCache,
            TassadarExecutorDecodeMode::SparseTopK,
        ]),
    );
    let sparse_top_k_selection = diagnose_tassadar_executor_request(
        &program,
        TassadarExecutorDecodeMode::SparseTopK,
        trace_abi.schema_version,
        Some(&[
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarExecutorDecodeMode::HullCache,
            TassadarExecutorDecodeMode::SparseTopK,
        ]),
    );

    let request_id = String::from("tassadar-million-step-decode-benchmark-v0");
    let request_digest = stable_serialized_digest(
        b"tassadar_million_step_benchmark_request|",
        &(
            program_artifact.validated_program_digest.as_str(),
            cpu_reference_summary.trace_digest.as_str(),
        ),
    );
    let model_descriptor_digest = stable_serialized_digest(
        b"tassadar_million_step_benchmark_model|",
        &(
            TASSADAR_CPU_REFERENCE_RUNNER_ID,
            profile.profile_id.as_str(),
            trace_abi.abi_id.as_str(),
            trace_abi.schema_version,
        ),
    );
    let evidence_bundle = build_tassadar_execution_summary_evidence_bundle(
        request_id.clone(),
        request_digest,
        "psionic.tassadar.million_step_decode_benchmark.v1",
        format!(
            "model://tassadar/{}/{}",
            TASSADAR_CPU_REFERENCE_RUNNER_ID, profile.profile_id
        ),
        model_descriptor_digest,
        vec![String::from(
            "env.openagents.tassadar.million_step_decode_benchmark",
        )],
        &program_artifact,
        &cpu_reference_summary,
    );

    let mut bundle = TassadarMillionStepDecodeBenchmarkBundle {
        schema_version: TASSADAR_MILLION_STEP_BENCHMARK_SCHEMA_VERSION,
        bundle_root_ref: String::from(TASSADAR_MILLION_STEP_BENCHMARK_ROOT_REF),
        workload_family_id: String::from("million_step_backward_branch_loop"),
        wasm_profile_id: profile.profile_id.clone(),
        trace_abi_id: trace_abi.abi_id.clone(),
        trace_abi_version: trace_abi.schema_version,
        iteration_count: TASSADAR_MILLION_STEP_LOOP_ITERATION_COUNT as u64,
        expected_step_count: TASSADAR_MILLION_STEP_LOOP_EXPECTED_STEPS,
        cpu_reference_summary,
        cpu_reference_steps_per_second,
        exactness_bps,
        memory_growth_metric: String::from("serialized_trace_step_stream_bytes"),
        memory_growth_bytes,
        runtime_manifest_identity_digest: evidence_bundle.runtime_manifest.identity_digest.clone(),
        reference_linear: TassadarMillionStepDecodeModeReceipt {
            requested_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
            selection_state: reference_linear_selection.selection_state,
            selection_reason: reference_linear_selection.selection_reason,
            effective_decode_mode: reference_linear_selection.effective_decode_mode,
            measurement_posture: TassadarMillionStepMeasurementPosture::Measured,
            steps_per_second: Some(cpu_reference_steps_per_second),
            remaining_gap_vs_cpu_reference: Some(0.0),
            note: String::from(
                "reference-linear is the measured CPU-reference mode for this million-step bundle, so its throughput and zero remaining CPU gap are recorded directly",
            ),
        },
        hull_cache: TassadarMillionStepDecodeModeReceipt {
            requested_decode_mode: TassadarExecutorDecodeMode::HullCache,
            selection_state: hull_cache_selection.selection_state,
            selection_reason: hull_cache_selection.selection_reason,
            effective_decode_mode: hull_cache_selection.effective_decode_mode,
            measurement_posture: TassadarMillionStepMeasurementPosture::SelectionOnly,
            steps_per_second: None,
            remaining_gap_vs_cpu_reference: None,
            note: String::from(
                "HullCache remains outside the validated backward-branch subset at the million-step horizon, so this bundle records explicit fallback truth rather than pretending direct fast-path closure",
            ),
        },
        sparse_top_k: TassadarMillionStepDecodeModeReceipt {
            requested_decode_mode: TassadarExecutorDecodeMode::SparseTopK,
            selection_state: sparse_top_k_selection.selection_state,
            selection_reason: sparse_top_k_selection.selection_reason,
            effective_decode_mode: sparse_top_k_selection.effective_decode_mode,
            measurement_posture: TassadarMillionStepMeasurementPosture::SelectionOnly,
            steps_per_second: None,
            remaining_gap_vs_cpu_reference: None,
            note: String::from(
                "SparseTopK remains outside the validated backward-branch subset at the million-step horizon, so this bundle records explicit fallback truth rather than claiming direct support",
            ),
        },
        runtime_capability: tassadar_runtime_capability_report(),
        evidence_bundle,
        claim_boundary: String::from(
            "this bundle proves one Psionic-owned million-step reference-linear CPU execution with compact trace/proof lineage, explicit memory-growth and runtime-identity receipts, and explicit decode-selection truth for the other request paths; it does not claim direct HullCache or SparseTopK million-step closure",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest =
        stable_serialized_digest(b"tassadar_million_step_decode_benchmark|", &bundle);
    Ok(bundle)
}

/// Writes the canonical million-step decode benchmark bundle.
pub fn write_tassadar_million_step_decode_benchmark_bundle(
    root: impl AsRef<std::path::Path>,
) -> Result<TassadarMillionStepDecodeBenchmarkBundle, TassadarMillionStepBenchmarkError> {
    let root = root.as_ref();
    std::fs::create_dir_all(root)?;
    let bundle = build_tassadar_million_step_decode_benchmark_bundle()?;
    let bundle_bytes = serde_json::to_vec_pretty(&bundle)?;
    std::fs::write(
        root.join(TASSADAR_MILLION_STEP_BENCHMARK_BUNDLE_FILE),
        bundle_bytes,
    )?;
    Ok(bundle)
}

fn canonical_long_horizon_trace_case()
-> Result<TassadarValidationCase, TassadarTraceAbiArtifactError> {
    tassadar_article_class_corpus()
        .into_iter()
        .find(|case| case.case_id == TASSADAR_LONG_HORIZON_TRACE_CASE_ID)
        .ok_or_else(|| TassadarTraceAbiArtifactError::MissingLongHorizonCase {
            case_id: String::from(TASSADAR_LONG_HORIZON_TRACE_CASE_ID),
        })
}

/// Builds the canonical long-horizon execution evidence bundle used to anchor
/// the trace-ABI decision report.
pub fn build_tassadar_long_horizon_trace_evidence_bundle()
-> Result<TassadarExecutionEvidenceBundle, TassadarTraceAbiArtifactError> {
    let case = canonical_long_horizon_trace_case()?;
    let profile =
        tassadar_wasm_profile_for_id(case.program.profile_id.as_str()).ok_or_else(|| {
            TassadarTraceAbiArtifactError::UnsupportedProfile {
                profile_id: case.program.profile_id.clone(),
            }
        })?;
    let trace_abi = tassadar_trace_abi_for_profile_id(case.program.profile_id.as_str())
        .ok_or_else(|| TassadarTraceAbiArtifactError::UnsupportedProfile {
            profile_id: case.program.profile_id.clone(),
        })?;
    let runner = TassadarCpuReferenceRunner::for_profile(profile.clone()).ok_or_else(|| {
        TassadarTraceAbiArtifactError::UnsupportedProfile {
            profile_id: case.program.profile_id.clone(),
        }
    })?;
    let execution = runner.execute(&case.program)?;
    if execution.steps != case.expected_trace || execution.outputs != case.expected_outputs {
        return Err(TassadarTraceAbiArtifactError::FixtureMismatch {
            case_id: case.case_id,
        });
    }
    let program_artifact = TassadarProgramArtifact::fixture_reference(
        format!("tassadar://artifact/trace_abi_fixture/{}", case.case_id),
        &profile,
        &trace_abi,
        case.program,
    )?;
    let request_id = format!(
        "tassadar-trace-abi-fixture-{}",
        TASSADAR_LONG_HORIZON_TRACE_CASE_ID
    );
    let request_digest = stable_serialized_digest(
        b"tassadar_trace_abi_fixture_request|",
        &(
            request_id.as_str(),
            program_artifact.validated_program_digest.as_str(),
        ),
    );
    let model_descriptor_digest = stable_serialized_digest(
        b"tassadar_trace_abi_fixture_model|",
        &(
            TASSADAR_CPU_REFERENCE_RUNNER_ID,
            trace_abi.abi_id.as_str(),
            trace_abi.schema_version,
            profile.profile_id.as_str(),
        ),
    );
    Ok(build_tassadar_execution_evidence_bundle(
        request_id,
        request_digest,
        "psionic.tassadar.trace_abi_fixture.v1",
        format!(
            "model://tassadar/{}/{}",
            TASSADAR_CPU_REFERENCE_RUNNER_ID, profile.profile_id
        ),
        model_descriptor_digest,
        vec![String::from(
            "env.openagents.tassadar.article_class.benchmark",
        )],
        &program_artifact,
        TassadarExecutorDecodeMode::ReferenceLinear,
        &execution,
    ))
}

fn build_tassadar_trace_abi_decision_report_for_refs(
    fixture_root_ref: impl Into<String>,
    evidence_bundle_ref: impl Into<String>,
    evidence_bundle: &TassadarExecutionEvidenceBundle,
) -> TassadarTraceAbiDecisionReport {
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    TassadarTraceAbiDecisionReport::new(
        trace_abi.clone(),
        TassadarLongHorizonTraceFixtureSummary {
            fixture_root_ref: fixture_root_ref.into(),
            evidence_bundle_ref: evidence_bundle_ref.into(),
            case_id: String::from(TASSADAR_LONG_HORIZON_TRACE_CASE_ID),
            program_id: evidence_bundle.trace_artifact.program_id.clone(),
            workload_target: String::from("long_loop_kernel"),
            trace_abi_id: evidence_bundle.trace_artifact.trace_abi_id.clone(),
            trace_abi_version: evidence_bundle.trace_artifact.trace_abi_version,
            trace_abi_compatibility_digest: trace_abi.compatibility_digest(),
            step_count: evidence_bundle.trace_artifact.step_count,
            trace_artifact_id: evidence_bundle.trace_artifact.artifact_id.clone(),
            trace_artifact_digest: evidence_bundle.trace_artifact.artifact_digest.clone(),
            trace_proof_id: evidence_bundle.trace_proof.proof_artifact_id.clone(),
            trace_proof_digest: evidence_bundle.trace_proof.proof_digest.clone(),
        },
    )
}

/// Builds the canonical long-horizon trace-ABI decision report.
pub fn build_tassadar_trace_abi_decision_report()
-> Result<TassadarTraceAbiDecisionReport, TassadarTraceAbiArtifactError> {
    let evidence_bundle = build_tassadar_long_horizon_trace_evidence_bundle()?;
    Ok(build_tassadar_trace_abi_decision_report_for_refs(
        TASSADAR_LONG_HORIZON_TRACE_FIXTURE_ROOT_REF,
        format!(
            "{}/{}",
            TASSADAR_LONG_HORIZON_TRACE_FIXTURE_ROOT_REF,
            TASSADAR_LONG_HORIZON_TRACE_EVIDENCE_BUNDLE_FILE
        ),
        &evidence_bundle,
    ))
}

/// Writes the canonical trace-ABI decision report plus its long-horizon
/// evidence-bundle fixture.
pub fn write_tassadar_trace_abi_decision_artifacts(
    report_path: impl AsRef<std::path::Path>,
    fixture_root: impl AsRef<std::path::Path>,
) -> Result<TassadarTraceAbiDecisionReport, TassadarTraceAbiArtifactError> {
    let report_path = report_path.as_ref();
    let fixture_root = fixture_root.as_ref();
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir_all(fixture_root)?;

    let evidence_bundle = build_tassadar_long_horizon_trace_evidence_bundle()?;
    let evidence_bundle_path = fixture_root.join(TASSADAR_LONG_HORIZON_TRACE_EVIDENCE_BUNDLE_FILE);
    let evidence_bytes = serde_json::to_vec_pretty(&evidence_bundle)?;
    std::fs::write(&evidence_bundle_path, evidence_bytes)?;

    let report = build_tassadar_trace_abi_decision_report_for_refs(
        canonical_repo_relative_path(fixture_root),
        canonical_repo_relative_path(&evidence_bundle_path),
        &evidence_bundle,
    );
    let report_bytes = serde_json::to_vec_pretty(&report)?;
    std::fs::write(report_path, report_bytes)?;
    Ok(report)
}

/// Returns the current supported Wasm profiles in canonical reporting order.
#[must_use]
pub fn tassadar_supported_wasm_profiles() -> Vec<TassadarWasmProfile> {
    vec![
        TassadarWasmProfile::core_i32_v1(),
        TassadarWasmProfile::core_i32_v2(),
        TassadarWasmProfile::core_i32_w1_1_v1(),
        TassadarWasmProfile::article_i32_compute_v1(),
        TassadarWasmProfile::sudoku_v0_search_v1(),
        TassadarWasmProfile::hungarian_v0_matching_v1(),
        TassadarWasmProfile::hungarian_10x10_matching_v1(),
        TassadarWasmProfile::sudoku_9x9_search_v1(),
        TassadarWasmProfile::sudoku_9x9_hard_search_v1(),
    ]
}

fn supported_wasm_profile_ids() -> Vec<String> {
    tassadar_supported_wasm_profiles()
        .into_iter()
        .map(|profile| profile.profile_id)
        .collect()
}

fn supported_wasm_profile_ids_csv() -> String {
    supported_wasm_profile_ids().join(", ")
}

/// Returns the current machine-readable Wasm instruction/profile coverage report.
#[must_use]
pub fn tassadar_wasm_instruction_coverage_report() -> TassadarWasmInstructionCoverageReport {
    let coverage_universe = TassadarOpcode::ARTICLE_I32_V1.to_vec();
    let profile_case_map = profile_case_map();
    let profiles = tassadar_supported_wasm_profiles()
        .into_iter()
        .map(|profile| {
            let current_case_ids = profile_case_map
                .get(profile.profile_id.as_str())
                .map_or_else(Vec::new, |case_ids| case_ids.clone());
            let current_workload_targets =
                workload_targets_for_profile(profile.profile_id.as_str());
            TassadarWasmProfileCoverage {
                profile_id: profile.profile_id.clone(),
                opcode_vocabulary_digest: profile.opcode_vocabulary_digest(),
                supported_opcodes: profile.allowed_opcodes.clone(),
                unsupported_opcodes: coverage_universe
                    .iter()
                    .copied()
                    .filter(|opcode| !profile.supports(*opcode))
                    .collect(),
                max_locals: profile.max_locals,
                max_memory_slots: profile.max_memory_slots,
                max_program_len: profile.max_program_len,
                max_steps: profile.max_steps,
                article_profile: profile.profile_id
                    == TassadarWasmProfileId::ArticleI32ComputeV1.as_str(),
                current_case_ids,
                current_workload_targets,
                claim_boundary: claim_boundary_for_profile(profile.profile_id.as_str()),
            }
        })
        .collect();
    let refusal_examples = unsupported_instruction_refusal_examples();
    TassadarWasmInstructionCoverageReport::new(profiles, refusal_examples)
}

/// One validated instruction in the narrow WebAssembly-first profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "opcode", rename_all = "snake_case")]
pub enum TassadarInstruction {
    /// Execute no stack, local, memory, or control effect.
    Nop,
    /// Push one constant `i32`.
    I32Const {
        /// Literal immediate value.
        value: i32,
    },
    /// Push one local onto the stack.
    LocalGet {
        /// Local slot index.
        local: u8,
    },
    /// Pop one stack value into one local.
    LocalSet {
        /// Local slot index.
        local: u8,
    },
    /// Copy the current top stack value into one local without popping it.
    LocalTee {
        /// Local slot index.
        local: u8,
    },
    /// Pop and discard one stack value.
    Drop,
    /// Pop two `i32` values and push the sum.
    I32Add,
    /// Pop two `i32` values and push the difference.
    I32Sub,
    /// Pop two `i32` values and push the product.
    I32Mul,
    /// Pop two `i32` values and push `1` when `left < right`, otherwise `0`.
    I32Lt,
    /// Pop one `i32` value and push `1` when it is zero, otherwise `0`.
    I32Eqz,
    /// Pop two `i32` values and push `1` when `left == right`, otherwise `0`.
    I32Eq,
    /// Pop two `i32` values and push `1` when `left != right`, otherwise `0`.
    I32Ne,
    /// Pop two `i32` values and push `1` when `left > right`, otherwise `0`.
    I32Gt,
    /// Pop two `i32` values and push `1` when `left <= right`, otherwise `0`.
    I32Le,
    /// Pop two `i32` values and push `1` when `left >= right`, otherwise `0`.
    I32Ge,
    /// Push one memory slot onto the stack.
    I32Load {
        /// Memory slot index.
        slot: u8,
    },
    /// Pop one stack value into one memory slot.
    I32Store {
        /// Memory slot index.
        slot: u8,
    },
    /// Pop one condition and branch to `target_pc` when non-zero.
    BrIf {
        /// Validated direct pc target for the Phase 1 profile.
        target_pc: u16,
    },
    /// Pop and emit one stack value.
    Output,
    /// Halt successfully.
    Return,
}

impl TassadarInstruction {
    /// Returns the stable opcode class for this instruction.
    #[must_use]
    pub const fn opcode(&self) -> TassadarOpcode {
        match self {
            Self::Nop => TassadarOpcode::Nop,
            Self::I32Const { .. } => TassadarOpcode::I32Const,
            Self::LocalGet { .. } => TassadarOpcode::LocalGet,
            Self::LocalSet { .. } => TassadarOpcode::LocalSet,
            Self::LocalTee { .. } => TassadarOpcode::LocalTee,
            Self::Drop => TassadarOpcode::Drop,
            Self::I32Add => TassadarOpcode::I32Add,
            Self::I32Sub => TassadarOpcode::I32Sub,
            Self::I32Mul => TassadarOpcode::I32Mul,
            Self::I32Lt => TassadarOpcode::I32Lt,
            Self::I32Eqz => TassadarOpcode::I32Eqz,
            Self::I32Eq => TassadarOpcode::I32Eq,
            Self::I32Ne => TassadarOpcode::I32Ne,
            Self::I32Gt => TassadarOpcode::I32Gt,
            Self::I32Le => TassadarOpcode::I32Le,
            Self::I32Ge => TassadarOpcode::I32Ge,
            Self::I32Load { .. } => TassadarOpcode::I32Load,
            Self::I32Store { .. } => TassadarOpcode::I32Store,
            Self::BrIf { .. } => TassadarOpcode::BrIf,
            Self::Output => TassadarOpcode::Output,
            Self::Return => TassadarOpcode::Return,
        }
    }
}

/// One Phase 1 executor program validated against a fixed Wasm-like profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgram {
    /// Stable program identifier.
    pub program_id: String,
    /// Stable profile identifier expected by this program.
    pub profile_id: String,
    /// Number of locals used by the program.
    pub local_count: usize,
    /// Number of memory slots surfaced to the program.
    pub memory_slots: usize,
    /// Initial memory contents for the program.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial_memory: Vec<i32>,
    /// Ordered instruction sequence.
    pub instructions: Vec<TassadarInstruction>,
}

impl TassadarProgram {
    /// Creates a new Phase 1 Tassadar program.
    #[must_use]
    pub fn new(
        program_id: impl Into<String>,
        profile: &TassadarWasmProfile,
        local_count: usize,
        memory_slots: usize,
        instructions: Vec<TassadarInstruction>,
    ) -> Self {
        Self {
            program_id: program_id.into(),
            profile_id: profile.profile_id.clone(),
            local_count,
            memory_slots,
            initial_memory: vec![0; memory_slots],
            instructions,
        }
    }

    /// Replaces the initial memory image.
    #[must_use]
    pub fn with_initial_memory(mut self, initial_memory: Vec<i32>) -> Self {
        self.initial_memory = initial_memory;
        self
    }

    /// Returns a stable digest over the validated program payload.
    #[must_use]
    pub fn program_digest(&self) -> String {
        hex::encode(Sha256::digest(serde_json::to_vec(self).unwrap_or_default()))
    }

    fn validate_against(
        &self,
        profile: &TassadarWasmProfile,
    ) -> Result<(), TassadarExecutionRefusal> {
        if self.profile_id != profile.profile_id {
            return Err(TassadarExecutionRefusal::ProfileMismatch {
                expected: profile.profile_id.clone(),
                actual: self.profile_id.clone(),
            });
        }
        if self.local_count > profile.max_locals {
            return Err(TassadarExecutionRefusal::TooManyLocals {
                requested: self.local_count,
                max_supported: profile.max_locals,
            });
        }
        if self.memory_slots > profile.max_memory_slots {
            return Err(TassadarExecutionRefusal::TooManyMemorySlots {
                requested: self.memory_slots,
                max_supported: profile.max_memory_slots,
            });
        }
        if self.instructions.len() > profile.max_program_len {
            return Err(TassadarExecutionRefusal::ProgramTooLong {
                instruction_count: self.instructions.len(),
                max_supported: profile.max_program_len,
            });
        }
        if self.initial_memory.len() != self.memory_slots {
            return Err(TassadarExecutionRefusal::InitialMemoryShapeMismatch {
                expected: self.memory_slots,
                actual: self.initial_memory.len(),
            });
        }
        for (pc, instruction) in self.instructions.iter().enumerate() {
            if !profile.supports(instruction.opcode()) {
                return Err(TassadarExecutionRefusal::UnsupportedOpcode {
                    pc,
                    opcode: instruction.opcode(),
                });
            }
            match instruction {
                TassadarInstruction::LocalGet { local }
                | TassadarInstruction::LocalSet { local }
                | TassadarInstruction::LocalTee { local }
                    if usize::from(*local) >= self.local_count =>
                {
                    return Err(TassadarExecutionRefusal::LocalOutOfRange {
                        pc,
                        local: usize::from(*local),
                        local_count: self.local_count,
                    });
                }
                TassadarInstruction::I32Load { slot } | TassadarInstruction::I32Store { slot }
                    if usize::from(*slot) >= self.memory_slots =>
                {
                    return Err(TassadarExecutionRefusal::MemorySlotOutOfRange {
                        pc,
                        slot: usize::from(*slot),
                        memory_slots: self.memory_slots,
                    });
                }
                TassadarInstruction::BrIf { target_pc }
                    if usize::from(*target_pc) >= self.instructions.len() =>
                {
                    return Err(TassadarExecutionRefusal::InvalidBranchTarget {
                        pc,
                        target_pc: usize::from(*target_pc),
                        instruction_count: self.instructions.len(),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Source-language family for one digest-bound Tassadar program artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarProgramSourceKind {
    /// Hand-authored fixture or reference program checked into the repo.
    Fixture,
    /// Lowered from the bounded symbolic compiler-target IR.
    SymbolicProgram,
    /// Lowered from a C source file.
    CSource,
    /// Lowered from Rust or a Rust-adjacent source program.
    RustSource,
    /// Lowered from a Wasm text-format module.
    WasmText,
    /// Imported from an already-built Wasm binary module.
    WasmBinary,
}

/// Source-identity facts for one digest-bound Tassadar program artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramSourceIdentity {
    /// Source-language family.
    pub source_kind: TassadarProgramSourceKind,
    /// Stable source identifier such as a fixture name or source path label.
    pub source_name: String,
    /// Stable digest for the source input to the lowering pipeline.
    pub source_digest: String,
}

impl TassadarProgramSourceIdentity {
    /// Creates one explicit source-identity record.
    #[must_use]
    pub fn new(
        source_kind: TassadarProgramSourceKind,
        source_name: impl Into<String>,
        source_digest: impl Into<String>,
    ) -> Self {
        Self {
            source_kind,
            source_name: source_name.into(),
            source_digest: source_digest.into(),
        }
    }

    /// Creates one fixture-source record by hashing the validated program.
    #[must_use]
    pub fn fixture(program: &TassadarProgram) -> Self {
        Self {
            source_kind: TassadarProgramSourceKind::Fixture,
            source_name: program.program_id.clone(),
            source_digest: program.program_digest(),
        }
    }

    /// Returns a stable digest over the source-identity record.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_serialized_digest(b"tassadar_program_source_identity|", self)
    }
}

/// Compiler/toolchain identity for one digest-bound Tassadar program artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompilerToolchainIdentity {
    /// Stable compiler or lowering-pipeline family label.
    pub compiler_family: String,
    /// Stable compiler or lowering-pipeline version label.
    pub compiler_version: String,
    /// Stable lowering target or target triple label.
    pub target: String,
    /// Stable lowering stages or feature flags selected for the artifact.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipeline_features: Vec<String>,
}

impl TassadarCompilerToolchainIdentity {
    /// Creates one explicit compiler/toolchain identity.
    #[must_use]
    pub fn new(
        compiler_family: impl Into<String>,
        compiler_version: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            compiler_family: compiler_family.into(),
            compiler_version: compiler_version.into(),
            target: target.into(),
            pipeline_features: Vec::new(),
        }
    }

    /// Attaches stable lowering features or pipeline stages.
    #[must_use]
    pub fn with_pipeline_features(mut self, mut pipeline_features: Vec<String>) -> Self {
        pipeline_features.sort();
        pipeline_features.dedup();
        self.pipeline_features = pipeline_features;
        self
    }

    /// Returns a stable digest over the compiler/toolchain identity.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_serialized_digest(b"tassadar_compiler_toolchain_identity|", self)
    }
}

/// Digest-bound Tassadar program artifact ready to pair with executor models.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramArtifact {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable digest over the identity-relevant artifact fields.
    pub artifact_digest: String,
    /// Source-identity facts for the artifact.
    pub source_identity: TassadarProgramSourceIdentity,
    /// Compiler/toolchain identity for the artifact.
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    /// Stable Wasm profile identifier.
    pub wasm_profile_id: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI schema version.
    pub trace_abi_version: u16,
    /// Stable opcode-vocabulary digest the artifact expects.
    pub opcode_vocabulary_digest: String,
    /// Stable digest over the validated program payload.
    pub validated_program_digest: String,
    /// Optional digest for the original Wasm binary module when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_binary_digest: Option<String>,
    /// Validated executor program carried by the artifact.
    pub validated_program: TassadarProgram,
}

impl TassadarProgramArtifact {
    /// Creates a digest-bound artifact from a validated program and explicit source/toolchain facts.
    pub fn new(
        artifact_id: impl Into<String>,
        source_identity: TassadarProgramSourceIdentity,
        toolchain_identity: TassadarCompilerToolchainIdentity,
        profile: &TassadarWasmProfile,
        trace_abi: &TassadarTraceAbi,
        validated_program: TassadarProgram,
    ) -> Result<Self, TassadarProgramArtifactError> {
        if trace_abi.profile_id != profile.profile_id {
            return Err(TassadarProgramArtifactError::TraceAbiProfileMismatch {
                trace_abi_profile_id: trace_abi.profile_id.clone(),
                wasm_profile_id: profile.profile_id.clone(),
            });
        }
        if validated_program.profile_id != profile.profile_id {
            return Err(TassadarProgramArtifactError::ProgramProfileMismatch {
                expected: profile.profile_id.clone(),
                actual: validated_program.profile_id.clone(),
            });
        }
        validated_program
            .validate_against(profile)
            .map_err(
                |error| TassadarProgramArtifactError::InvalidValidatedProgram {
                    message: error.to_string(),
                },
            )?;

        let validated_program_digest = validated_program.program_digest();
        let mut artifact = Self {
            artifact_id: artifact_id.into(),
            artifact_digest: String::new(),
            source_identity,
            toolchain_identity,
            wasm_profile_id: profile.profile_id.clone(),
            trace_abi_id: trace_abi.abi_id.clone(),
            trace_abi_version: trace_abi.schema_version,
            opcode_vocabulary_digest: profile.opcode_vocabulary_digest(),
            validated_program_digest,
            wasm_binary_digest: None,
            validated_program,
        };
        artifact.refresh_digest();
        Ok(artifact)
    }

    /// Creates the canonical fixture artifact posture for a validated program.
    pub fn fixture_reference(
        artifact_id: impl Into<String>,
        profile: &TassadarWasmProfile,
        trace_abi: &TassadarTraceAbi,
        validated_program: TassadarProgram,
    ) -> Result<Self, TassadarProgramArtifactError> {
        let source_identity = TassadarProgramSourceIdentity::fixture(&validated_program);
        let toolchain_identity = TassadarCompilerToolchainIdentity::new(
            "tassadar_fixture",
            "v1",
            profile.profile_id.as_str(),
        )
        .with_pipeline_features(vec![
            String::from("validated_program"),
            String::from("webassembly_first"),
        ]);
        Self::new(
            artifact_id,
            source_identity,
            toolchain_identity,
            profile,
            trace_abi,
            validated_program,
        )
    }

    /// Attaches an original Wasm binary digest when one exists.
    #[must_use]
    pub fn with_wasm_binary_digest(mut self, wasm_binary_digest: impl Into<String>) -> Self {
        self.wasm_binary_digest = Some(wasm_binary_digest.into());
        self.refresh_digest();
        self
    }

    /// Validates internal artifact consistency without pairing against a model descriptor.
    pub fn validate_internal_consistency(&self) -> Result<(), TassadarProgramArtifactError> {
        if self.validated_program_digest != self.validated_program.program_digest() {
            return Err(
                TassadarProgramArtifactError::ValidatedProgramDigestMismatch {
                    expected: self.validated_program.program_digest(),
                    actual: self.validated_program_digest.clone(),
                },
            );
        }
        let actual_digest = self.compute_digest();
        if self.artifact_digest != actual_digest {
            return Err(TassadarProgramArtifactError::ArtifactDigestMismatch {
                expected: actual_digest,
                actual: self.artifact_digest.clone(),
            });
        }
        Ok(())
    }

    fn refresh_digest(&mut self) {
        self.artifact_digest = self.compute_digest();
    }

    fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.artifact_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.wasm_profile_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_abi_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_abi_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.opcode_vocabulary_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.validated_program_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.source_identity).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.toolchain_identity).unwrap_or_default());
        hasher.update(b"\n");
        if let Some(wasm_binary_digest) = &self.wasm_binary_digest {
            hasher.update(wasm_binary_digest.as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// Artifact-assembly failures for digest-bound Tassadar program artifacts.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarProgramArtifactError {
    /// The ABI and Wasm profile targeted different execution profiles.
    #[error(
        "trace ABI profile `{trace_abi_profile_id}` does not match Wasm profile `{wasm_profile_id}`"
    )]
    TraceAbiProfileMismatch {
        /// Profile identifier declared by the trace ABI.
        trace_abi_profile_id: String,
        /// Profile identifier declared by the Wasm profile.
        wasm_profile_id: String,
    },
    /// The validated program targeted a different profile than the artifact profile.
    #[error("validated program profile mismatch: expected `{expected}`, got `{actual}`")]
    ProgramProfileMismatch {
        /// Expected profile identifier.
        expected: String,
        /// Actual validated-program profile identifier.
        actual: String,
    },
    /// The supplied validated program failed structural validation.
    #[error("validated program is not internally valid: {message}")]
    InvalidValidatedProgram {
        /// Validation failure summary.
        message: String,
    },
    /// The stored validated-program digest no longer matches the payload.
    #[error("validated program digest mismatch: expected `{expected}`, actual `{actual}`")]
    ValidatedProgramDigestMismatch {
        /// Expected digest recomputed from the validated program.
        expected: String,
        /// Actual stored digest.
        actual: String,
    },
    /// The stored artifact digest no longer matches the identity fields.
    #[error("artifact digest mismatch: expected `{expected}`, actual `{actual}`")]
    ArtifactDigestMismatch {
        /// Expected digest recomputed from the artifact fields.
        expected: String,
        /// Actual stored digest.
        actual: String,
    },
}

/// Canonical repo-relative C source used for the Tassadar compile-receipt path.
pub const TASSADAR_CANONICAL_C_SOURCE_REF: &str =
    "fixtures/tassadar/sources/tassadar_micro_wasm_kernel.c";
/// Canonical repo-relative Wasm binary emitted by the compile-receipt example.
pub const TASSADAR_CANONICAL_WASM_BINARY_REF: &str =
    "fixtures/tassadar/wasm/tassadar_micro_wasm_kernel.wasm";
/// Repo-relative Rust source for the exact multi-export arithmetic fixture.
pub const TASSADAR_MULTI_EXPORT_RUST_SOURCE_REF: &str =
    "fixtures/tassadar/sources/tassadar_multi_export_kernel.rs";
/// Repo-relative Wasm binary emitted for the exact multi-export Rust fixture.
pub const TASSADAR_MULTI_EXPORT_RUST_WASM_BINARY_REF: &str =
    "fixtures/tassadar/wasm/tassadar_multi_export_kernel.wasm";
/// Repo-relative Rust source for the exact memory-lookup fixture.
pub const TASSADAR_MEMORY_LOOKUP_RUST_SOURCE_REF: &str =
    "fixtures/tassadar/sources/tassadar_memory_lookup_kernel.rs";
/// Repo-relative Wasm binary emitted for the exact memory-lookup Rust fixture.
pub const TASSADAR_MEMORY_LOOKUP_RUST_WASM_BINARY_REF: &str =
    "fixtures/tassadar/wasm/tassadar_memory_lookup_kernel.wasm";
/// Repo-relative Rust source for the unsupported parameter-ABI fixture.
pub const TASSADAR_PARAM_ABI_RUST_SOURCE_REF: &str =
    "fixtures/tassadar/sources/tassadar_param_abi_kernel.rs";
/// Repo-relative Wasm binary emitted for the unsupported parameter-ABI Rust fixture.
pub const TASSADAR_PARAM_ABI_RUST_WASM_BINARY_REF: &str =
    "fixtures/tassadar/wasm/tassadar_param_abi_kernel.wasm";
/// Repo-relative Rust source for the pointer-based micro-kernel article fixture.
pub const TASSADAR_MICRO_WASM_RUST_SOURCE_REF: &str =
    "fixtures/tassadar/sources/tassadar_micro_wasm_kernel.rs";
/// Repo-relative Wasm binary emitted for the pointer-based micro-kernel article fixture.
pub const TASSADAR_MICRO_WASM_RUST_WASM_BINARY_REF: &str =
    "fixtures/tassadar/wasm/tassadar_micro_wasm_kernel_rust.wasm";
/// Repo-relative Rust source for the pointer-length heap-input fixture.
pub const TASSADAR_HEAP_SUM_RUST_SOURCE_REF: &str =
    "fixtures/tassadar/sources/tassadar_heap_sum_kernel.rs";
/// Repo-relative Wasm binary emitted for the pointer-length heap-input fixture.
pub const TASSADAR_HEAP_SUM_RUST_WASM_BINARY_REF: &str =
    "fixtures/tassadar/wasm/tassadar_heap_sum_kernel.wasm";
/// Repo-relative Rust source for the long-loop article fixture.
pub const TASSADAR_LONG_LOOP_RUST_SOURCE_REF: &str =
    "fixtures/tassadar/sources/tassadar_long_loop_kernel.rs";
/// Repo-relative Wasm binary emitted for the long-loop article fixture.
pub const TASSADAR_LONG_LOOP_RUST_WASM_BINARY_REF: &str =
    "fixtures/tassadar/wasm/tassadar_long_loop_kernel.wasm";
/// Repo-relative Rust source for the article-sized Hungarian fixture.
pub const TASSADAR_HUNGARIAN_10X10_RUST_SOURCE_REF: &str =
    "fixtures/tassadar/sources/tassadar_hungarian_10x10_article.rs";
/// Repo-relative Wasm binary emitted for the article-sized Hungarian fixture.
pub const TASSADAR_HUNGARIAN_10X10_RUST_WASM_BINARY_REF: &str =
    "fixtures/tassadar/wasm/tassadar_hungarian_10x10_article.wasm";
/// Repo-relative Rust source for the article-sized Sudoku fixture.
pub const TASSADAR_SUDOKU_9X9_RUST_SOURCE_REF: &str =
    "fixtures/tassadar/sources/tassadar_sudoku_9x9_article.rs";
/// Repo-relative Wasm binary emitted for the article-sized Sudoku fixture.
pub const TASSADAR_SUDOKU_9X9_RUST_WASM_BINARY_REF: &str =
    "fixtures/tassadar/wasm/tassadar_sudoku_9x9_article.wasm";
/// Canonical repo-relative compile-receipt artifact for the canonical C source.
pub const TASSADAR_C_TO_WASM_COMPILE_RECEIPT_REF: &str =
    "fixtures/tassadar/reports/tassadar_c_to_wasm_compile_receipt.json";
/// Stable artifact id for the canonical compile-lineage program artifact.
pub const TASSADAR_CANONICAL_C_PROGRAM_ARTIFACT_ID: &str =
    "tassadar.micro_wasm_kernel.c_compile.artifact.v1";
const TASSADAR_C_TO_WASM_COMPILE_SCHEMA_VERSION: u16 = 1;
const TASSADAR_RUST_TO_WASM_COMPILE_SCHEMA_VERSION: u16 = 1;

/// Explicit compile configuration for the canonical C-to-Wasm receipt path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCToWasmCompileConfig {
    /// Compiler driver binary used for the compile.
    pub compiler_binary: String,
    /// Lowering target triple.
    pub target: String,
    /// Source-language standard.
    pub language_standard: String,
    /// Optimization level.
    pub optimization_level: String,
    /// Exported symbols that must survive the link.
    pub export_symbols: Vec<String>,
    /// Whether the compile must avoid the standard library.
    pub no_standard_library: bool,
    /// Whether the module is linked with `--no-entry`.
    pub no_entry: bool,
    /// Whether unresolved imports are explicitly allowed.
    pub allow_undefined: bool,
}

impl TassadarCToWasmCompileConfig {
    /// Returns the canonical config for the micro Wasm kernel fixture.
    #[must_use]
    pub fn canonical_micro_wasm_kernel() -> Self {
        Self::canonical_without_standard_library(["micro_wasm_kernel"])
    }

    fn canonical_without_standard_library(
        export_symbols: impl IntoIterator<Item = &'static str>,
    ) -> Self {
        Self {
            compiler_binary: String::from("clang"),
            target: String::from("wasm32-unknown-unknown"),
            language_standard: String::from("c11"),
            optimization_level: String::from("O3"),
            export_symbols: export_symbols.into_iter().map(String::from).collect(),
            no_standard_library: true,
            no_entry: true,
            allow_undefined: false,
        }
    }

    /// Returns a stable digest over the compile configuration.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_serialized_digest(b"tassadar_c_to_wasm_compile_config|", self)
    }

    fn command_line(
        &self,
        source_path: &std::path::Path,
        output_path: &std::path::Path,
    ) -> Vec<String> {
        let mut args = vec![
            format!("--target={}", self.target),
            format!("-std={}", self.language_standard),
            format!("-{}", self.optimization_level),
        ];
        if self.no_standard_library {
            args.push(String::from("-nostdlib"));
        }
        if self.no_entry {
            args.push(String::from("-Wl,--no-entry"));
        }
        if self.allow_undefined {
            args.push(String::from("-Wl,--allow-undefined"));
        }
        for export in &self.export_symbols {
            args.push(format!("-Wl,--export={export}"));
        }
        args.push(String::from("-o"));
        args.push(output_path.display().to_string());
        args.push(source_path.display().to_string());
        args
    }

    /// Returns the stable feature set declared by this compile configuration.
    #[must_use]
    pub fn pipeline_features(&self) -> Vec<String> {
        let mut features = vec![
            format!("language_standard:{}", self.language_standard),
            format!("optimization:{}", self.optimization_level),
            format!("compiler_binary:{}", self.compiler_binary),
        ];
        if self.no_standard_library {
            features.push(String::from("nostdlib"));
        }
        if self.no_entry {
            features.push(String::from("no_entry"));
        }
        if self.allow_undefined {
            features.push(String::from("allow_undefined"));
        }
        for export in &self.export_symbols {
            features.push(format!("export:{export}"));
        }
        features.sort();
        features.dedup();
        features
    }
}

impl Default for TassadarCToWasmCompileConfig {
    fn default() -> Self {
        Self::canonical_micro_wasm_kernel()
    }
}

/// Explicit compile configuration for the repo-owned Rust-to-Wasm matrix path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustToWasmCompileConfig {
    /// Compiler driver binary used for the compile.
    pub compiler_binary: String,
    /// Lowering target triple.
    pub target: String,
    /// Stable crate name used for the compile.
    pub crate_name: String,
    /// Rust edition used for the fixture.
    pub edition: String,
    /// Crate type emitted for the Wasm binary.
    pub crate_type: String,
    /// Optimization level.
    pub optimization_level: String,
    /// Panic strategy.
    pub panic_strategy: String,
    /// Stable metadata tag used to keep repeated builds reproducible.
    pub metadata_tag: String,
    /// Exported symbols that must survive the link.
    pub export_symbols: Vec<String>,
}

impl TassadarRustToWasmCompileConfig {
    /// Returns the canonical config for the exact multi-export Rust fixture.
    #[must_use]
    pub fn canonical_multi_export_kernel() -> Self {
        Self::canonical("tassadar_multi_export_kernel", ["pair_sum", "local_double"])
    }

    /// Returns the canonical config for the exact memory-lookup Rust fixture.
    #[must_use]
    pub fn canonical_memory_lookup_kernel() -> Self {
        Self::canonical(
            "tassadar_memory_lookup_kernel",
            ["load_middle", "load_edge_sum"],
        )
    }

    /// Returns the canonical config for the parameter-ABI Rust fixture.
    #[must_use]
    pub fn canonical_param_abi_kernel() -> Self {
        Self::canonical("tassadar_param_abi_kernel", ["add_one", "pair_add"])
    }

    /// Returns the canonical config for the pointer-based micro-kernel fixture.
    #[must_use]
    pub fn canonical_micro_wasm_kernel() -> Self {
        Self::canonical("tassadar_micro_wasm_kernel", ["micro_wasm_kernel"])
    }

    /// Returns the canonical config for the pointer-length heap-input fixture.
    #[must_use]
    pub fn canonical_heap_sum_kernel() -> Self {
        Self::canonical(
            "tassadar_heap_sum_kernel",
            ["heap_sum_i32", "dot_i32", "sum_and_max_into_buffer"],
        )
    }

    /// Returns the canonical config for the long-loop article fixture.
    #[must_use]
    pub fn canonical_long_loop_kernel() -> Self {
        Self::canonical("tassadar_long_loop_kernel", ["million_step_loop"])
    }

    /// Returns the canonical config for the article-sized Hungarian fixture.
    #[must_use]
    pub fn canonical_hungarian_10x10_article() -> Self {
        Self::canonical(
            "tassadar_hungarian_10x10_article",
            ["hungarian_10x10_article_cost"],
        )
    }

    /// Returns the canonical config for the article-sized Sudoku fixture.
    #[must_use]
    pub fn canonical_sudoku_9x9_article() -> Self {
        Self::canonical(
            "tassadar_sudoku_9x9_article",
            ["sudoku_9x9_article_checksum"],
        )
    }

    fn canonical(
        crate_name: &'static str,
        export_symbols: impl IntoIterator<Item = &'static str>,
    ) -> Self {
        Self {
            compiler_binary: String::from("rustc"),
            target: String::from("wasm32-unknown-unknown"),
            crate_name: String::from(crate_name),
            edition: String::from("2024"),
            crate_type: String::from("cdylib"),
            optimization_level: String::from("3"),
            panic_strategy: String::from("abort"),
            metadata_tag: String::from(crate_name),
            export_symbols: export_symbols.into_iter().map(String::from).collect(),
        }
    }

    /// Returns a stable digest over the compile configuration.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_serialized_digest(b"tassadar_rust_to_wasm_compile_config|", self)
    }

    /// Returns the stable feature set declared by this compile configuration.
    #[must_use]
    pub fn pipeline_features(&self) -> Vec<String> {
        let mut features = vec![
            format!("compiler_binary:{}", self.compiler_binary),
            format!("crate_type:{}", self.crate_type),
            format!("crate_name:{}", self.crate_name),
            format!("edition:{}", self.edition),
            format!("metadata:{}", self.metadata_tag),
            format!("optimization:{}", self.optimization_level),
            format!("panic:{}", self.panic_strategy),
            format!("target:{}", self.target),
        ];
        for export in &self.export_symbols {
            features.push(format!("export:{export}"));
        }
        features.sort();
        features.dedup();
        features
    }

    fn command_line(
        &self,
        source_path: &std::path::Path,
        output_path: &std::path::Path,
    ) -> Vec<String> {
        vec![
            String::from("--target"),
            self.target.clone(),
            String::from("--crate-name"),
            self.crate_name.clone(),
            String::from("--edition"),
            self.edition.clone(),
            String::from("-C"),
            format!("metadata={}", self.metadata_tag),
            String::from("-C"),
            format!("opt-level={}", self.optimization_level),
            String::from("-C"),
            String::from("debuginfo=0"),
            String::from("-C"),
            format!("panic={}", self.panic_strategy),
            String::from("--crate-type"),
            self.crate_type.clone(),
            source_path.display().to_string(),
            String::from("-o"),
            output_path.display().to_string(),
        ]
    }
}

/// Machine-readable structural summary for one compiled Wasm module.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmBinarySummary {
    /// Exact output size in bytes.
    pub byte_len: usize,
    /// Exported function names.
    pub exported_functions: Vec<String>,
    /// Total function count including imported functions.
    pub function_count: u32,
    /// Imported function count.
    pub imported_function_count: u32,
    /// Memory section count.
    pub memory_count: u32,
    /// Custom section names preserved in the module.
    pub custom_sections: Vec<String>,
}

/// Machine-readable refusal surface for the canonical C-to-Wasm receipt path.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarCompileRefusal {
    /// The canonical source file could not be read.
    #[error("failed to read source `{path}`: {message}")]
    SourceReadFailed {
        /// Source path or label.
        path: String,
        /// IO failure summary.
        message: String,
    },
    /// The compile workspace could not be created or populated.
    #[error("failed to prepare compile workspace `{path}`: {message}")]
    CompileWorkspaceFailed {
        /// Workspace path.
        path: String,
        /// Failure summary.
        message: String,
    },
    /// The requested compiler binary was unavailable.
    #[error("compiler binary `{binary}` was unavailable")]
    ToolchainUnavailable {
        /// Missing compiler binary.
        binary: String,
    },
    /// The compiler version probe failed before compile execution.
    #[error(
        "compiler probe for `{binary}` failed with exit code {exit_code:?} and stderr digest `{stderr_digest}`"
    )]
    ToolchainProbeFailed {
        /// Compiler binary invoked for the probe.
        binary: String,
        /// Exit code returned by the probe when available.
        exit_code: Option<i32>,
        /// Stable digest over stderr.
        stderr_digest: String,
    },
    /// The compiler failed to produce a Wasm binary.
    #[error(
        "compile via `{compiler_binary}` failed with exit code {exit_code:?} and stderr digest `{stderr_digest}`"
    )]
    ToolchainFailure {
        /// Compiler binary used for the compile.
        compiler_binary: String,
        /// Exit code returned by the compiler.
        exit_code: Option<i32>,
        /// Stable digest over stderr.
        stderr_digest: String,
        /// Short stderr excerpt for logs and debugging.
        stderr_excerpt: String,
    },
    /// The compiled Wasm output could not be read back from disk.
    #[error("failed to read compiled Wasm output `{path}`: {message}")]
    OutputReadFailed {
        /// Output path.
        path: String,
        /// IO failure summary.
        message: String,
    },
    /// The compiled output was not a valid Wasm module.
    #[error("compiled output was not a valid Wasm module: {message}")]
    InvalidWasmOutput {
        /// Validation failure summary.
        message: String,
    },
    /// The compiled output omitted one expected export.
    #[error("compiled Wasm output omitted expected export `{expected}`; got {actual:?}")]
    MissingExpectedExport {
        /// Expected exported symbol.
        expected: String,
        /// Export set discovered in the module.
        actual: Vec<String>,
    },
    /// The repo could not project the successful compile into the canonical
    /// executor artifact lineage.
    #[error("canonical executor-artifact projection failed: {message}")]
    ExecutorArtifactProjectionFailed {
        /// Projection failure summary.
        message: String,
    },
}

impl TassadarCompileRefusal {
    /// Returns the stable machine-readable refusal kind.
    #[must_use]
    pub fn kind_slug(&self) -> &'static str {
        match self {
            Self::SourceReadFailed { .. } => "source_read_failed",
            Self::CompileWorkspaceFailed { .. } => "compile_workspace_failed",
            Self::ToolchainUnavailable { .. } => "toolchain_unavailable",
            Self::ToolchainProbeFailed { .. } => "toolchain_probe_failed",
            Self::ToolchainFailure { .. } => "toolchain_failure",
            Self::OutputReadFailed { .. } => "output_read_failed",
            Self::InvalidWasmOutput { .. } => "invalid_wasm_output",
            Self::MissingExpectedExport { .. } => "missing_expected_export",
            Self::ExecutorArtifactProjectionFailed { .. } => "executor_artifact_projection_failed",
        }
    }
}

/// Source-to-Wasm-to-artifact lineage facts for one canonical compile receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompileArtifactLineage {
    /// Canonical validated program id bound to the receipt.
    pub program_id: String,
    /// Stable digest over the validated program payload.
    pub validated_program_digest: String,
    /// Canonical program artifact id bound to the receipt.
    pub artifact_id: String,
    /// Stable digest over the derived program artifact.
    pub artifact_digest: String,
    /// Stable Wasm profile identifier for the artifact projection.
    pub wasm_profile_id: String,
    /// Stable trace ABI identifier for the artifact projection.
    pub trace_abi_id: String,
    /// Stable trace ABI schema version for the artifact projection.
    pub trace_abi_version: u16,
}

/// Outcome of one canonical C-to-Wasm compile attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarCToWasmCompileOutcome {
    /// The source compiled successfully and produced a Wasm binary plus
    /// canonical executor-artifact lineage.
    Succeeded {
        /// Repo-relative Wasm binary ref when the binary is written inside the repo.
        wasm_binary_ref: String,
        /// Stable digest over the compiled Wasm binary.
        wasm_binary_digest: String,
        /// Structural summary of the compiled Wasm module.
        wasm_binary_summary: TassadarWasmBinarySummary,
        /// Canonical executor-artifact lineage facts derived from the compile.
        lineage_contract: TassadarCompileArtifactLineage,
    },
    /// The compile refused with a typed machine-readable reason.
    Refused {
        /// Typed refusal record.
        refusal: TassadarCompileRefusal,
    },
}

/// Machine-readable receipt for one canonical C-to-Wasm compile attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCToWasmCompileReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable source-identity facts.
    pub source_identity: TassadarProgramSourceIdentity,
    /// Stable compiler/toolchain identity.
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    /// Stable compile configuration.
    pub compile_config: TassadarCToWasmCompileConfig,
    /// Successful output or typed refusal.
    pub outcome: TassadarCToWasmCompileOutcome,
    /// Plain-language boundary statement for the receipt.
    pub claim_boundary: String,
    /// Stable digest over the full receipt.
    pub receipt_digest: String,
}

impl TassadarCToWasmCompileReceipt {
    fn new(
        source_identity: TassadarProgramSourceIdentity,
        toolchain_identity: TassadarCompilerToolchainIdentity,
        compile_config: TassadarCToWasmCompileConfig,
        outcome: TassadarCToWasmCompileOutcome,
    ) -> Self {
        let mut receipt = Self {
            schema_version: TASSADAR_C_TO_WASM_COMPILE_SCHEMA_VERSION,
            source_identity,
            toolchain_identity,
            compile_config,
            outcome,
            claim_boundary: String::from(
                "canonical micro_wasm_kernel C-to-Wasm receipt only; proves one explicit source/toolchain/output/artifact lineage and one typed refusal path, not general C/C++ frontend closure or arbitrary Wasm lowering",
            ),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest =
            stable_serialized_digest(b"tassadar_c_to_wasm_compile_receipt|", &receipt);
        receipt
    }

    /// Returns whether the compile succeeded.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        matches!(
            self.outcome,
            TassadarCToWasmCompileOutcome::Succeeded { .. }
        )
    }

    /// Returns the compiled Wasm digest when the compile succeeded.
    #[must_use]
    pub fn wasm_binary_digest(&self) -> Option<&str> {
        match &self.outcome {
            TassadarCToWasmCompileOutcome::Succeeded {
                wasm_binary_digest, ..
            } => Some(wasm_binary_digest.as_str()),
            TassadarCToWasmCompileOutcome::Refused { .. } => None,
        }
    }

    /// Returns the compiled Wasm ref when the compile succeeded.
    #[must_use]
    pub fn wasm_binary_ref(&self) -> Option<&str> {
        match &self.outcome {
            TassadarCToWasmCompileOutcome::Succeeded {
                wasm_binary_ref, ..
            } => Some(wasm_binary_ref.as_str()),
            TassadarCToWasmCompileOutcome::Refused { .. } => None,
        }
    }

    /// Returns the structural summary for the compiled Wasm output when present.
    #[must_use]
    pub fn wasm_binary_summary(&self) -> Option<&TassadarWasmBinarySummary> {
        match &self.outcome {
            TassadarCToWasmCompileOutcome::Succeeded {
                wasm_binary_summary,
                ..
            } => Some(wasm_binary_summary),
            TassadarCToWasmCompileOutcome::Refused { .. } => None,
        }
    }

    /// Returns the canonical artifact lineage when the compile succeeded.
    #[must_use]
    pub fn lineage_contract(&self) -> Option<&TassadarCompileArtifactLineage> {
        match &self.outcome {
            TassadarCToWasmCompileOutcome::Succeeded {
                lineage_contract, ..
            } => Some(lineage_contract),
            TassadarCToWasmCompileOutcome::Refused { .. } => None,
        }
    }

    /// Returns the typed refusal when the compile did not succeed.
    #[must_use]
    pub fn refusal(&self) -> Option<&TassadarCompileRefusal> {
        match &self.outcome {
            TassadarCToWasmCompileOutcome::Succeeded { .. } => None,
            TassadarCToWasmCompileOutcome::Refused { refusal } => Some(refusal),
        }
    }
}

/// Outcome of one Rust-source to Wasm compile attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarRustToWasmCompileOutcome {
    /// The source compiled successfully and produced a Wasm binary.
    Succeeded {
        /// Repo-relative Wasm binary ref when the binary is written inside the repo.
        wasm_binary_ref: String,
        /// Stable digest over the compiled Wasm binary.
        wasm_binary_digest: String,
        /// Structural summary of the compiled Wasm module.
        wasm_binary_summary: TassadarWasmBinarySummary,
    },
    /// The compile refused with a typed machine-readable reason.
    Refused {
        /// Typed refusal record.
        refusal: TassadarCompileRefusal,
    },
}

/// Machine-readable receipt for one Rust-source to Wasm compile attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustToWasmCompileReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable source-identity facts.
    pub source_identity: TassadarProgramSourceIdentity,
    /// Stable compiler/toolchain identity.
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    /// Stable compile configuration.
    pub compile_config: TassadarRustToWasmCompileConfig,
    /// Successful output or typed refusal.
    pub outcome: TassadarRustToWasmCompileOutcome,
    /// Plain-language boundary statement for the receipt.
    pub claim_boundary: String,
    /// Stable digest over the full receipt.
    pub receipt_digest: String,
}

impl TassadarRustToWasmCompileReceipt {
    fn new(
        source_identity: TassadarProgramSourceIdentity,
        toolchain_identity: TassadarCompilerToolchainIdentity,
        compile_config: TassadarRustToWasmCompileConfig,
        outcome: TassadarRustToWasmCompileOutcome,
    ) -> Self {
        let mut receipt = Self {
            schema_version: TASSADAR_RUST_TO_WASM_COMPILE_SCHEMA_VERSION,
            source_identity,
            toolchain_identity,
            compile_config,
            outcome,
            claim_boundary: String::from(
                "repo-owned Rust-source to Wasm receipt only; proves explicit source/toolchain/config/output digests for the current bounded matrix fixtures and typed refusal when the host toolchain cannot satisfy that path, not arbitrary Rust frontend closure or general Wasm lowering",
            ),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest =
            stable_serialized_digest(b"tassadar_rust_to_wasm_compile_receipt|", &receipt);
        receipt
    }

    /// Returns whether the compile succeeded.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        matches!(
            self.outcome,
            TassadarRustToWasmCompileOutcome::Succeeded { .. }
        )
    }

    /// Returns the compiled Wasm ref when the compile succeeded.
    #[must_use]
    pub fn wasm_binary_ref(&self) -> Option<&str> {
        match &self.outcome {
            TassadarRustToWasmCompileOutcome::Succeeded {
                wasm_binary_ref, ..
            } => Some(wasm_binary_ref.as_str()),
            TassadarRustToWasmCompileOutcome::Refused { .. } => None,
        }
    }

    /// Returns the compiled Wasm digest when the compile succeeded.
    #[must_use]
    pub fn wasm_binary_digest(&self) -> Option<&str> {
        match &self.outcome {
            TassadarRustToWasmCompileOutcome::Succeeded {
                wasm_binary_digest, ..
            } => Some(wasm_binary_digest.as_str()),
            TassadarRustToWasmCompileOutcome::Refused { .. } => None,
        }
    }

    /// Returns the structural Wasm summary when the compile succeeded.
    #[must_use]
    pub fn wasm_binary_summary(&self) -> Option<&TassadarWasmBinarySummary> {
        match &self.outcome {
            TassadarRustToWasmCompileOutcome::Succeeded {
                wasm_binary_summary,
                ..
            } => Some(wasm_binary_summary),
            TassadarRustToWasmCompileOutcome::Refused { .. } => None,
        }
    }

    /// Returns the typed refusal when the compile did not succeed.
    #[must_use]
    pub fn refusal(&self) -> Option<&TassadarCompileRefusal> {
        match &self.outcome {
            TassadarRustToWasmCompileOutcome::Succeeded { .. } => None,
            TassadarRustToWasmCompileOutcome::Refused { refusal } => Some(refusal),
        }
    }
}

/// Error while projecting one successful compile receipt into a program artifact.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarCompileArtifactError {
    /// The compile receipt carried a refusal instead of a usable Wasm digest.
    #[error("compile receipt refused: {refusal}")]
    CompileRefused {
        /// Typed refusal from the compile receipt.
        refusal: TassadarCompileRefusal,
    },
    /// The artifact projection itself failed validation.
    #[error(transparent)]
    Artifact(#[from] TassadarProgramArtifactError),
}

/// Returns the canonical absolute path for the micro Wasm kernel C source.
#[must_use]
pub fn tassadar_canonical_c_source_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_CANONICAL_C_SOURCE_REF)
}

/// Returns the canonical absolute path for the micro Wasm kernel Wasm binary.
#[must_use]
pub fn tassadar_canonical_wasm_binary_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_CANONICAL_WASM_BINARY_REF)
}

/// Returns the canonical absolute path for the exact multi-export Rust source.
#[must_use]
pub fn tassadar_multi_export_rust_source_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_MULTI_EXPORT_RUST_SOURCE_REF)
}

/// Returns the canonical absolute path for the exact multi-export Rust Wasm output.
#[must_use]
pub fn tassadar_multi_export_rust_wasm_binary_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_MULTI_EXPORT_RUST_WASM_BINARY_REF)
}

/// Returns the canonical absolute path for the exact memory-lookup Rust source.
#[must_use]
pub fn tassadar_memory_lookup_rust_source_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_MEMORY_LOOKUP_RUST_SOURCE_REF)
}

/// Returns the canonical absolute path for the exact memory-lookup Rust Wasm output.
#[must_use]
pub fn tassadar_memory_lookup_rust_wasm_binary_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_MEMORY_LOOKUP_RUST_WASM_BINARY_REF)
}

/// Returns the canonical absolute path for the unsupported parameter-ABI Rust source.
#[must_use]
pub fn tassadar_param_abi_rust_source_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_PARAM_ABI_RUST_SOURCE_REF)
}

/// Returns the canonical absolute path for the unsupported parameter-ABI Rust Wasm output.
#[must_use]
pub fn tassadar_param_abi_rust_wasm_binary_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_PARAM_ABI_RUST_WASM_BINARY_REF)
}

/// Returns the canonical absolute path for the Rust micro-kernel source.
#[must_use]
pub fn tassadar_micro_wasm_rust_source_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_MICRO_WASM_RUST_SOURCE_REF)
}

/// Returns the canonical absolute path for the Rust micro-kernel Wasm output.
#[must_use]
pub fn tassadar_micro_wasm_rust_wasm_binary_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_MICRO_WASM_RUST_WASM_BINARY_REF)
}

/// Returns the canonical absolute path for the Rust heap-sum source.
#[must_use]
pub fn tassadar_heap_sum_rust_source_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_HEAP_SUM_RUST_SOURCE_REF)
}

/// Returns the canonical absolute path for the Rust heap-sum Wasm output.
#[must_use]
pub fn tassadar_heap_sum_rust_wasm_binary_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_HEAP_SUM_RUST_WASM_BINARY_REF)
}

/// Returns the canonical absolute path for the Rust long-loop source.
#[must_use]
pub fn tassadar_long_loop_rust_source_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_LONG_LOOP_RUST_SOURCE_REF)
}

/// Returns the canonical absolute path for the Rust long-loop Wasm output.
#[must_use]
pub fn tassadar_long_loop_rust_wasm_binary_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_LONG_LOOP_RUST_WASM_BINARY_REF)
}

/// Returns the canonical absolute path for the Rust article-sized Hungarian source.
#[must_use]
pub fn tassadar_hungarian_10x10_rust_source_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_HUNGARIAN_10X10_RUST_SOURCE_REF)
}

/// Returns the canonical absolute path for the Rust article-sized Hungarian Wasm output.
#[must_use]
pub fn tassadar_hungarian_10x10_rust_wasm_binary_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_HUNGARIAN_10X10_RUST_WASM_BINARY_REF)
}

/// Returns the canonical absolute path for the Rust article-sized Sudoku source.
#[must_use]
pub fn tassadar_sudoku_9x9_rust_source_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_SUDOKU_9X9_RUST_SOURCE_REF)
}

/// Returns the canonical absolute path for the Rust article-sized Sudoku Wasm output.
#[must_use]
pub fn tassadar_sudoku_9x9_rust_wasm_binary_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_SUDOKU_9X9_RUST_WASM_BINARY_REF)
}

/// Returns the canonical absolute path for the C-to-Wasm compile receipt.
#[must_use]
pub fn tassadar_c_to_wasm_compile_receipt_path() -> std::path::PathBuf {
    runtime_repo_root().join(TASSADAR_C_TO_WASM_COMPILE_RECEIPT_REF)
}

/// Builds a digest-bound executor artifact from one successful compile receipt
/// and an already-validated executor program.
pub fn tassadar_program_artifact_from_compile_receipt(
    receipt: &TassadarCToWasmCompileReceipt,
    artifact_id: impl Into<String>,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    validated_program: TassadarProgram,
) -> Result<TassadarProgramArtifact, TassadarCompileArtifactError> {
    let mut artifact = TassadarProgramArtifact::new(
        artifact_id,
        receipt.source_identity.clone(),
        receipt.toolchain_identity.clone(),
        profile,
        trace_abi,
        validated_program,
    )?;
    let wasm_binary_digest = receipt.wasm_binary_digest().ok_or_else(|| {
        TassadarCompileArtifactError::CompileRefused {
            refusal: receipt
                .refusal()
                .expect("compile receipt refusal should be present when digest is absent")
                .clone(),
        }
    })?;
    artifact = artifact.with_wasm_binary_digest(wasm_binary_digest.to_string());
    Ok(artifact)
}

/// Compiles one C source payload into a Wasm module and emits a machine-readable receipt.
#[must_use]
pub fn compile_tassadar_c_source_to_wasm_receipt(
    source_name: impl Into<String>,
    source_bytes: &[u8],
    output_wasm_path: impl AsRef<std::path::Path>,
    compile_config: &TassadarCToWasmCompileConfig,
) -> TassadarCToWasmCompileReceipt {
    let source_name = source_name.into();
    let source_identity = TassadarProgramSourceIdentity::new(
        TassadarProgramSourceKind::CSource,
        source_name.clone(),
        stable_bytes_digest(source_bytes),
    );
    let compiler_binary = compile_config.compiler_binary.clone();
    let toolchain_identity = match discover_c_compile_toolchain_identity(compile_config) {
        Ok(toolchain_identity) => toolchain_identity,
        Err(refusal) => {
            return TassadarCToWasmCompileReceipt::new(
                source_identity,
                TassadarCompilerToolchainIdentity::new(
                    compiler_family_for_binary(compiler_binary.as_str()),
                    String::from("unavailable"),
                    compile_config.target.clone(),
                ),
                compile_config.clone(),
                TassadarCToWasmCompileOutcome::Refused { refusal },
            );
        }
    };

    let output_wasm_path = output_wasm_path.as_ref().to_path_buf();
    if let Some(parent) = output_wasm_path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            return TassadarCToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarCToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::CompileWorkspaceFailed {
                        path: parent.display().to_string(),
                        message: error.to_string(),
                    },
                },
            );
        }
    }

    let workspace_root = std::env::temp_dir().join(format!(
        "psionic-tassadar-c-compile-{}-{}",
        std::process::id(),
        stable_bytes_digest(source_bytes)
    ));
    if let Err(error) = std::fs::create_dir_all(&workspace_root) {
        return TassadarCToWasmCompileReceipt::new(
            source_identity,
            toolchain_identity,
            compile_config.clone(),
            TassadarCToWasmCompileOutcome::Refused {
                refusal: TassadarCompileRefusal::CompileWorkspaceFailed {
                    path: workspace_root.display().to_string(),
                    message: error.to_string(),
                },
            },
        );
    }

    let source_path = workspace_root.join("compile_input.c");
    if let Err(error) = std::fs::write(&source_path, source_bytes) {
        return TassadarCToWasmCompileReceipt::new(
            source_identity,
            toolchain_identity,
            compile_config.clone(),
            TassadarCToWasmCompileOutcome::Refused {
                refusal: TassadarCompileRefusal::CompileWorkspaceFailed {
                    path: source_path.display().to_string(),
                    message: error.to_string(),
                },
            },
        );
    }

    let raw_output_wasm_path = workspace_root.join("compile_output_raw.wasm");
    let compile_output = match std::process::Command::new(&compile_config.compiler_binary)
        .args(compile_config.command_line(&source_path, &raw_output_wasm_path))
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return TassadarCToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarCToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::ToolchainUnavailable {
                        binary: compile_config.compiler_binary.clone(),
                    },
                },
            );
        }
        Err(error) => {
            return TassadarCToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarCToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::CompileWorkspaceFailed {
                        path: output_wasm_path.display().to_string(),
                        message: error.to_string(),
                    },
                },
            );
        }
    };
    if !compile_output.status.success() {
        let stderr_digest = stable_bytes_digest(&compile_output.stderr);
        let stderr_excerpt = String::from_utf8_lossy(&compile_output.stderr)
            .trim()
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .join(" | ");
        return TassadarCToWasmCompileReceipt::new(
            source_identity,
            toolchain_identity,
            compile_config.clone(),
            TassadarCToWasmCompileOutcome::Refused {
                refusal: TassadarCompileRefusal::ToolchainFailure {
                    compiler_binary: compile_config.compiler_binary.clone(),
                    exit_code: compile_output.status.code(),
                    stderr_digest,
                    stderr_excerpt,
                },
            },
        );
    }

    let wasm_bytes = match std::fs::read(&output_wasm_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return TassadarCToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarCToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::OutputReadFailed {
                        path: output_wasm_path.display().to_string(),
                        message: error.to_string(),
                    },
                },
            );
        }
    };
    let wasm_binary_digest = stable_bytes_digest(&wasm_bytes);
    let wasm_binary_summary = match summarize_tassadar_wasm_binary(&wasm_bytes) {
        Ok(summary) => summary,
        Err(message) => {
            return TassadarCToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarCToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::InvalidWasmOutput { message },
                },
            );
        }
    };
    for expected_export in &compile_config.export_symbols {
        if !wasm_binary_summary
            .exported_functions
            .contains(expected_export)
        {
            return TassadarCToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarCToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::MissingExpectedExport {
                        expected: expected_export.clone(),
                        actual: wasm_binary_summary.exported_functions.clone(),
                    },
                },
            );
        }
    }

    let lineage_contract =
        match canonical_compile_lineage(&source_identity, &toolchain_identity, &wasm_binary_digest)
        {
            Ok(lineage_contract) => lineage_contract,
            Err(error) => {
                return TassadarCToWasmCompileReceipt::new(
                    source_identity,
                    toolchain_identity,
                    compile_config.clone(),
                    TassadarCToWasmCompileOutcome::Refused {
                        refusal: TassadarCompileRefusal::ExecutorArtifactProjectionFailed {
                            message: error.to_string(),
                        },
                    },
                );
            }
        };

    TassadarCToWasmCompileReceipt::new(
        source_identity,
        toolchain_identity,
        compile_config.clone(),
        TassadarCToWasmCompileOutcome::Succeeded {
            wasm_binary_ref: canonical_repo_relative_path(&output_wasm_path),
            wasm_binary_digest,
            wasm_binary_summary,
            lineage_contract,
        },
    )
}

/// Builds the canonical C-to-Wasm compile receipt from the committed C source.
#[must_use]
pub fn build_tassadar_c_to_wasm_compile_receipt() -> TassadarCToWasmCompileReceipt {
    let source_path = tassadar_canonical_c_source_path();
    let source_bytes = match std::fs::read(&source_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return TassadarCToWasmCompileReceipt::new(
                TassadarProgramSourceIdentity::new(
                    TassadarProgramSourceKind::CSource,
                    String::from(TASSADAR_CANONICAL_C_SOURCE_REF),
                    String::new(),
                ),
                TassadarCompilerToolchainIdentity::new(
                    compiler_family_for_binary("clang"),
                    String::from("unavailable"),
                    TassadarCToWasmCompileConfig::canonical_micro_wasm_kernel().target,
                ),
                TassadarCToWasmCompileConfig::canonical_micro_wasm_kernel(),
                TassadarCToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::SourceReadFailed {
                        path: source_path.display().to_string(),
                        message: error.to_string(),
                    },
                },
            );
        }
    };
    compile_tassadar_c_source_to_wasm_receipt(
        TASSADAR_CANONICAL_C_SOURCE_REF,
        &source_bytes,
        tassadar_canonical_wasm_binary_path(),
        &TassadarCToWasmCompileConfig::canonical_micro_wasm_kernel(),
    )
}

/// Writes the canonical C-to-Wasm compile receipt and compiled Wasm binary.
pub fn write_tassadar_c_to_wasm_compile_receipt(
    output_path: impl AsRef<std::path::Path>,
) -> Result<TassadarCToWasmCompileReceipt, std::io::Error> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let receipt = build_tassadar_c_to_wasm_compile_receipt();
    let bytes = serde_json::to_vec_pretty(&receipt)
        .expect("Tassadar C-to-Wasm compile receipt should serialize");
    std::fs::write(output_path, bytes)?;
    Ok(receipt)
}

/// Compiles one Rust source payload into a Wasm module and emits a machine-readable receipt.
#[must_use]
pub fn compile_tassadar_rust_source_to_wasm_receipt(
    source_name: impl Into<String>,
    source_bytes: &[u8],
    output_wasm_path: impl AsRef<std::path::Path>,
    compile_config: &TassadarRustToWasmCompileConfig,
) -> TassadarRustToWasmCompileReceipt {
    let source_name = source_name.into();
    let source_identity = TassadarProgramSourceIdentity::new(
        TassadarProgramSourceKind::RustSource,
        source_name.clone(),
        stable_bytes_digest(source_bytes),
    );
    let compiler_binary = compile_config.compiler_binary.clone();
    let toolchain_identity = match discover_rust_compile_toolchain_identity(compile_config) {
        Ok(toolchain_identity) => toolchain_identity,
        Err(refusal) => {
            return TassadarRustToWasmCompileReceipt::new(
                source_identity,
                TassadarCompilerToolchainIdentity::new(
                    compiler_family_for_binary(compiler_binary.as_str()),
                    String::from("unavailable"),
                    compile_config.target.clone(),
                ),
                compile_config.clone(),
                TassadarRustToWasmCompileOutcome::Refused { refusal },
            );
        }
    };

    let output_wasm_path = output_wasm_path.as_ref().to_path_buf();
    if let Some(parent) = output_wasm_path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            return TassadarRustToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarRustToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::CompileWorkspaceFailed {
                        path: parent.display().to_string(),
                        message: error.to_string(),
                    },
                },
            );
        }
    }

    let workspace_root = std::env::temp_dir().join(format!(
        "psionic-tassadar-rust-compile-{}-{}",
        compile_config.metadata_tag,
        stable_bytes_digest(source_bytes)
    ));
    if let Err(error) = std::fs::create_dir_all(&workspace_root) {
        return TassadarRustToWasmCompileReceipt::new(
            source_identity,
            toolchain_identity,
            compile_config.clone(),
            TassadarRustToWasmCompileOutcome::Refused {
                refusal: TassadarCompileRefusal::CompileWorkspaceFailed {
                    path: workspace_root.display().to_string(),
                    message: error.to_string(),
                },
            },
        );
    }

    let source_path = workspace_root.join("compile_input.rs");
    if let Err(error) = std::fs::write(&source_path, source_bytes) {
        return TassadarRustToWasmCompileReceipt::new(
            source_identity,
            toolchain_identity,
            compile_config.clone(),
            TassadarRustToWasmCompileOutcome::Refused {
                refusal: TassadarCompileRefusal::CompileWorkspaceFailed {
                    path: source_path.display().to_string(),
                    message: error.to_string(),
                },
            },
        );
    }

    let raw_output_wasm_path = workspace_root.join("compile_output_raw.wasm");
    let compile_output = match std::process::Command::new(&compile_config.compiler_binary)
        .args(compile_config.command_line(&source_path, &raw_output_wasm_path))
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return TassadarRustToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarRustToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::ToolchainUnavailable {
                        binary: compile_config.compiler_binary.clone(),
                    },
                },
            );
        }
        Err(error) => {
            return TassadarRustToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarRustToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::CompileWorkspaceFailed {
                        path: output_wasm_path.display().to_string(),
                        message: error.to_string(),
                    },
                },
            );
        }
    };
    if !compile_output.status.success() {
        let stderr_digest = stable_bytes_digest(&compile_output.stderr);
        let stderr_excerpt = String::from_utf8_lossy(&compile_output.stderr)
            .trim()
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .join(" | ");
        return TassadarRustToWasmCompileReceipt::new(
            source_identity,
            toolchain_identity,
            compile_config.clone(),
            TassadarRustToWasmCompileOutcome::Refused {
                refusal: TassadarCompileRefusal::ToolchainFailure {
                    compiler_binary: compile_config.compiler_binary.clone(),
                    exit_code: compile_output.status.code(),
                    stderr_digest,
                    stderr_excerpt,
                },
            },
        );
    }

    let raw_wasm_bytes = match std::fs::read(&raw_output_wasm_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return TassadarRustToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarRustToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::OutputReadFailed {
                        path: raw_output_wasm_path.display().to_string(),
                        message: error.to_string(),
                    },
                },
            );
        }
    };
    let raw_wasm_bytes = strip_debug_custom_sections(&raw_wasm_bytes).unwrap_or(raw_wasm_bytes);
    let wasm_bytes = match parse_tassadar_normalized_wasm_module(&raw_wasm_bytes)
        .and_then(|module| encode_tassadar_normalized_wasm_module(&module))
    {
        Ok(bytes) => bytes,
        Err(_) => raw_wasm_bytes,
    };
    if let Err(error) = std::fs::write(&output_wasm_path, &wasm_bytes) {
        return TassadarRustToWasmCompileReceipt::new(
            source_identity,
            toolchain_identity,
            compile_config.clone(),
            TassadarRustToWasmCompileOutcome::Refused {
                refusal: TassadarCompileRefusal::OutputReadFailed {
                    path: output_wasm_path.display().to_string(),
                    message: error.to_string(),
                },
            },
        );
    }
    let wasm_binary_digest = stable_bytes_digest(&wasm_bytes);
    let wasm_binary_summary = match summarize_tassadar_wasm_binary(&wasm_bytes) {
        Ok(summary) => summary,
        Err(message) => {
            return TassadarRustToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarRustToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::InvalidWasmOutput { message },
                },
            );
        }
    };
    for expected_export in &compile_config.export_symbols {
        if !wasm_binary_summary
            .exported_functions
            .contains(expected_export)
        {
            return TassadarRustToWasmCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarRustToWasmCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::MissingExpectedExport {
                        expected: expected_export.clone(),
                        actual: wasm_binary_summary.exported_functions.clone(),
                    },
                },
            );
        }
    }

    TassadarRustToWasmCompileReceipt::new(
        source_identity,
        toolchain_identity,
        compile_config.clone(),
        TassadarRustToWasmCompileOutcome::Succeeded {
            wasm_binary_ref: canonical_repo_relative_path(&output_wasm_path),
            wasm_binary_digest,
            wasm_binary_summary,
        },
    )
}

fn compiler_family_for_binary(binary: &str) -> String {
    std::path::Path::new(binary).file_name().map_or_else(
        || String::from(binary),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn strip_debug_custom_sections(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut sanitized = wasm_encoder::Module::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(|error| error.to_string())?;
        match payload {
            wasmparser::Payload::Version { .. } => {}
            wasmparser::Payload::CustomSection(reader) if reader.name().starts_with(".debug") => {}
            other => {
                if let Some((id, range)) = other.as_section() {
                    sanitized.section(&wasm_encoder::RawSection {
                        id,
                        data: &bytes[range.start..range.end],
                    });
                }
            }
        }
    }
    Ok(sanitized.finish())
}

fn discover_c_compile_toolchain_identity(
    compile_config: &TassadarCToWasmCompileConfig,
) -> Result<TassadarCompilerToolchainIdentity, TassadarCompileRefusal> {
    let output = std::process::Command::new(&compile_config.compiler_binary)
        .arg("--version")
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                TassadarCompileRefusal::ToolchainUnavailable {
                    binary: compile_config.compiler_binary.clone(),
                }
            } else {
                TassadarCompileRefusal::ToolchainProbeFailed {
                    binary: compile_config.compiler_binary.clone(),
                    exit_code: None,
                    stderr_digest: stable_bytes_digest(error.to_string().as_bytes()),
                }
            }
        })?;
    if !output.status.success() {
        return Err(TassadarCompileRefusal::ToolchainProbeFailed {
            binary: compile_config.compiler_binary.clone(),
            exit_code: output.status.code(),
            stderr_digest: stable_bytes_digest(&output.stderr),
        });
    }
    let first_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map_or_else(String::new, std::borrow::ToOwned::to_owned);
    let compiler_version = first_line
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|window| (window[0] == "version").then(|| window[1].to_string()))
        .unwrap_or_else(|| first_line.clone());
    Ok(TassadarCompilerToolchainIdentity::new(
        compiler_family_for_binary(compile_config.compiler_binary.as_str()),
        compiler_version,
        compile_config.target.clone(),
    )
    .with_pipeline_features(compile_config.pipeline_features()))
}

fn discover_rust_compile_toolchain_identity(
    compile_config: &TassadarRustToWasmCompileConfig,
) -> Result<TassadarCompilerToolchainIdentity, TassadarCompileRefusal> {
    let output = std::process::Command::new(&compile_config.compiler_binary)
        .arg("--version")
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                TassadarCompileRefusal::ToolchainUnavailable {
                    binary: compile_config.compiler_binary.clone(),
                }
            } else {
                TassadarCompileRefusal::ToolchainProbeFailed {
                    binary: compile_config.compiler_binary.clone(),
                    exit_code: None,
                    stderr_digest: stable_bytes_digest(error.to_string().as_bytes()),
                }
            }
        })?;
    if !output.status.success() {
        return Err(TassadarCompileRefusal::ToolchainProbeFailed {
            binary: compile_config.compiler_binary.clone(),
            exit_code: output.status.code(),
            stderr_digest: stable_bytes_digest(&output.stderr),
        });
    }
    let first_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map_or_else(String::new, std::borrow::ToOwned::to_owned);
    Ok(TassadarCompilerToolchainIdentity::new(
        compiler_family_for_binary(compile_config.compiler_binary.as_str()),
        first_line,
        compile_config.target.clone(),
    )
    .with_pipeline_features(compile_config.pipeline_features()))
}

/// Summarizes one real Wasm binary into the current bounded structural runtime
/// view.
pub fn summarize_tassadar_wasm_binary(bytes: &[u8]) -> Result<TassadarWasmBinarySummary, String> {
    let mut exported_functions = Vec::new();
    let mut function_count = 0u32;
    let mut imported_function_count = 0u32;
    let mut memory_count = 0u32;
    let mut custom_sections = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        match payload.map_err(|error| error.to_string())? {
            wasmparser::Payload::ImportSection(reader) => {
                for import in reader {
                    let import = import.map_err(|error| error.to_string())?;
                    match import {
                        wasmparser::Imports::Single(_, import)
                            if matches!(import.ty, wasmparser::TypeRef::Func(_)) =>
                        {
                            imported_function_count = imported_function_count.saturating_add(1);
                        }
                        wasmparser::Imports::Compact1 { items, .. } => {
                            for item in items {
                                let item = item.map_err(|error| error.to_string())?;
                                if matches!(item.ty, wasmparser::TypeRef::Func(_)) {
                                    imported_function_count =
                                        imported_function_count.saturating_add(1);
                                }
                            }
                        }
                        wasmparser::Imports::Compact2 { names, ty, .. }
                            if matches!(ty, wasmparser::TypeRef::Func(_)) =>
                        {
                            for name in names {
                                name.map_err(|error| error.to_string())?;
                                imported_function_count = imported_function_count.saturating_add(1);
                            }
                        }
                        _ => {}
                    }
                }
            }
            wasmparser::Payload::FunctionSection(reader) => {
                for function in reader {
                    function.map_err(|error| error.to_string())?;
                    function_count = function_count.saturating_add(1);
                }
            }
            wasmparser::Payload::MemorySection(reader) => {
                for memory in reader {
                    memory.map_err(|error| error.to_string())?;
                    memory_count = memory_count.saturating_add(1);
                }
            }
            wasmparser::Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(|error| error.to_string())?;
                    if export.kind == wasmparser::ExternalKind::Func {
                        exported_functions.push(export.name.to_string());
                    }
                }
            }
            wasmparser::Payload::CustomSection(reader) => {
                custom_sections.push(reader.name().to_string());
            }
            _ => {}
        }
    }
    exported_functions.sort();
    exported_functions.dedup();
    custom_sections.sort();
    custom_sections.dedup();
    Ok(TassadarWasmBinarySummary {
        byte_len: bytes.len(),
        exported_functions,
        function_count: function_count.saturating_add(imported_function_count),
        imported_function_count,
        memory_count,
        custom_sections,
    })
}

fn canonical_compile_lineage(
    source_identity: &TassadarProgramSourceIdentity,
    toolchain_identity: &TassadarCompilerToolchainIdentity,
    wasm_binary_digest: &str,
) -> Result<TassadarCompileArtifactLineage, TassadarCompileArtifactError> {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    let validated_program = tassadar_article_class_corpus()
        .into_iter()
        .find(|case| case.case_id == "micro_wasm_kernel")
        .expect("canonical article-class micro kernel should exist")
        .program;
    let artifact = TassadarProgramArtifact::new(
        TASSADAR_CANONICAL_C_PROGRAM_ARTIFACT_ID,
        source_identity.clone(),
        toolchain_identity.clone(),
        &profile,
        &trace_abi,
        validated_program,
    )?
    .with_wasm_binary_digest(wasm_binary_digest.to_string());
    artifact.validate_internal_consistency()?;
    Ok(TassadarCompileArtifactLineage {
        program_id: artifact.validated_program.program_id,
        validated_program_digest: artifact.validated_program_digest,
        artifact_id: artifact.artifact_id,
        artifact_digest: artifact.artifact_digest,
        wasm_profile_id: artifact.wasm_profile_id,
        trace_abi_id: artifact.trace_abi_id,
        trace_abi_version: artifact.trace_abi_version,
    })
}

fn canonical_repo_relative_path(path: &std::path::Path) -> String {
    path.strip_prefix(runtime_repo_root()).map_or_else(
        |_| path.display().to_string(),
        |relative| relative.display().to_string(),
    )
}

/// One rule in the handcrafted/programmatic Tassadar fixture construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarOpcodeRule {
    /// Stable opcode identity.
    pub opcode: TassadarOpcode,
    /// Number of stack values consumed.
    pub pops: u8,
    /// Number of stack values produced.
    pub pushes: u8,
    /// Immediate family carried by the opcode.
    pub immediate_kind: TassadarImmediateKind,
    /// Local/memory access classification.
    pub access_class: TassadarAccessClass,
    /// Control-flow classification.
    pub control_class: TassadarControlClass,
    /// Arithmetic family when the opcode is arithmetic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arithmetic: Option<TassadarArithmeticOp>,
}

impl TassadarOpcodeRule {
    #[must_use]
    fn new(
        opcode: TassadarOpcode,
        pops: u8,
        pushes: u8,
        immediate_kind: TassadarImmediateKind,
        access_class: TassadarAccessClass,
        control_class: TassadarControlClass,
    ) -> Self {
        Self {
            opcode,
            pops,
            pushes,
            immediate_kind,
            access_class,
            control_class,
            arithmetic: None,
        }
    }

    #[must_use]
    fn with_arithmetic(mut self, arithmetic: TassadarArithmeticOp) -> Self {
        self.arithmetic = Some(arithmetic);
        self
    }
}

/// Handcrafted/programmatic rule tables backing the Phase 1 fixture lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFixtureWeights {
    /// Stable profile identifier the weights are constructed for.
    pub profile_id: String,
    /// Stable trace ABI identifier paired with the weights.
    pub trace_abi_id: String,
    /// Ordered opcode rule table.
    pub opcode_rules: Vec<TassadarOpcodeRule>,
}

impl TassadarFixtureWeights {
    /// Returns the canonical Phase 1 handcrafted fixture table.
    #[must_use]
    pub fn core_i32_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::CoreI32V1.as_str()),
            trace_abi_id: String::from("tassadar.trace.v1"),
            opcode_rules: vec![
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Const,
                    0,
                    1,
                    TassadarImmediateKind::I32,
                    TassadarAccessClass::None,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::LocalGet,
                    0,
                    1,
                    TassadarImmediateKind::LocalIndex,
                    TassadarAccessClass::LocalRead,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::LocalSet,
                    1,
                    0,
                    TassadarImmediateKind::LocalIndex,
                    TassadarAccessClass::LocalWrite,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Add,
                    2,
                    1,
                    TassadarImmediateKind::None,
                    TassadarAccessClass::None,
                    TassadarControlClass::Linear,
                )
                .with_arithmetic(TassadarArithmeticOp::Add),
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Sub,
                    2,
                    1,
                    TassadarImmediateKind::None,
                    TassadarAccessClass::None,
                    TassadarControlClass::Linear,
                )
                .with_arithmetic(TassadarArithmeticOp::Sub),
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Mul,
                    2,
                    1,
                    TassadarImmediateKind::None,
                    TassadarAccessClass::None,
                    TassadarControlClass::Linear,
                )
                .with_arithmetic(TassadarArithmeticOp::Mul),
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Load,
                    0,
                    1,
                    TassadarImmediateKind::MemorySlot,
                    TassadarAccessClass::MemoryRead,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Store,
                    1,
                    0,
                    TassadarImmediateKind::MemorySlot,
                    TassadarAccessClass::MemoryWrite,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::BrIf,
                    1,
                    0,
                    TassadarImmediateKind::BranchTarget,
                    TassadarAccessClass::None,
                    TassadarControlClass::ConditionalBranch,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::Output,
                    1,
                    0,
                    TassadarImmediateKind::None,
                    TassadarAccessClass::None,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::Return,
                    0,
                    0,
                    TassadarImmediateKind::None,
                    TassadarAccessClass::None,
                    TassadarControlClass::Return,
                ),
            ],
        }
    }

    /// Returns the widened article-class handcrafted fixture table.
    #[must_use]
    pub fn core_i32_v2() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::CoreI32V2.as_str()),
            trace_abi_id: String::from("tassadar.trace.v1"),
            opcode_rules: Self::core_i32_v1().opcode_rules,
        }
    }

    /// Returns the current article-shaped i32 handcrafted fixture table.
    #[must_use]
    pub fn article_i32_compute_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::ArticleI32ComputeV1.as_str()),
            trace_abi_id: String::from("tassadar.trace.v1"),
            opcode_rules: Self::hungarian_v0_matching_v1().opcode_rules,
        }
    }

    /// Returns the larger search-profile handcrafted fixture table.
    #[must_use]
    pub fn sudoku_v0_search_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::SudokuV0SearchV1.as_str()),
            trace_abi_id: String::from("tassadar.trace.v1"),
            opcode_rules: Self::core_i32_v1().opcode_rules,
        }
    }

    /// Returns the bounded comparison-capable handcrafted fixture table.
    #[must_use]
    pub fn hungarian_v0_matching_v1() -> Self {
        let mut opcode_rules = Self::core_i32_v1().opcode_rules;
        opcode_rules.insert(
            6,
            TassadarOpcodeRule::new(
                TassadarOpcode::I32Lt,
                2,
                1,
                TassadarImmediateKind::None,
                TassadarAccessClass::None,
                TassadarControlClass::Linear,
            )
            .with_arithmetic(TassadarArithmeticOp::Lt),
        );
        Self {
            profile_id: String::from(TassadarWasmProfileId::HungarianV0MatchingV1.as_str()),
            trace_abi_id: String::from("tassadar.trace.v1"),
            opcode_rules,
        }
    }

    /// Returns the larger comparison-capable handcrafted fixture table.
    #[must_use]
    pub fn hungarian_10x10_matching_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::Hungarian10x10MatchingV1.as_str()),
            trace_abi_id: String::from("tassadar.trace.v1"),
            opcode_rules: Self::hungarian_v0_matching_v1().opcode_rules,
        }
    }

    /// Returns the larger 9x9 search-profile handcrafted fixture table.
    #[must_use]
    pub fn sudoku_9x9_search_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::Sudoku9x9SearchV1.as_str()),
            trace_abi_id: String::from("tassadar.trace.v1"),
            opcode_rules: Self::core_i32_v1().opcode_rules,
        }
    }

    /// Returns the larger hard-suite 9x9 search-profile handcrafted fixture table.
    #[must_use]
    pub fn sudoku_9x9_hard_search_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::Sudoku9x9HardSearchV1.as_str()),
            trace_abi_id: String::from("tassadar.trace.v1"),
            opcode_rules: Self::core_i32_v1().opcode_rules,
        }
    }

    /// Returns one rule by opcode.
    #[must_use]
    pub fn rule_for(&self, opcode: TassadarOpcode) -> Option<&TassadarOpcodeRule> {
        self.opcode_rules.iter().find(|rule| rule.opcode == opcode)
    }
}

impl Default for TassadarFixtureWeights {
    fn default() -> Self {
        Self::core_i32_v1()
    }
}

/// Returns the current handcrafted fixture table for one supported profile id.
#[must_use]
pub fn tassadar_fixture_weights_for_profile_id(profile_id: &str) -> Option<TassadarFixtureWeights> {
    match profile_id {
        value if value == TassadarWasmProfileId::CoreI32V1.as_str() => {
            Some(TassadarFixtureWeights::core_i32_v1())
        }
        value if value == TassadarWasmProfileId::CoreI32V2.as_str() => {
            Some(TassadarFixtureWeights::core_i32_v2())
        }
        value if value == TassadarWasmProfileId::ArticleI32ComputeV1.as_str() => {
            Some(TassadarFixtureWeights::article_i32_compute_v1())
        }
        value if value == TassadarWasmProfileId::SudokuV0SearchV1.as_str() => {
            Some(TassadarFixtureWeights::sudoku_v0_search_v1())
        }
        value if value == TassadarWasmProfileId::HungarianV0MatchingV1.as_str() => {
            Some(TassadarFixtureWeights::hungarian_v0_matching_v1())
        }
        value if value == TassadarWasmProfileId::Hungarian10x10MatchingV1.as_str() => {
            Some(TassadarFixtureWeights::hungarian_10x10_matching_v1())
        }
        value if value == TassadarWasmProfileId::Sudoku9x9SearchV1.as_str() => {
            Some(TassadarFixtureWeights::sudoku_9x9_search_v1())
        }
        value if value == TassadarWasmProfileId::Sudoku9x9HardSearchV1.as_str() => {
            Some(TassadarFixtureWeights::sudoku_9x9_hard_search_v1())
        }
        _ => None,
    }
}

/// One emitted trace event in the append-only ABI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarTraceEvent {
    /// One no-op executed.
    Nop,
    /// One constant was pushed.
    ConstPush {
        /// Value that was pushed.
        value: i32,
    },
    /// One local was read.
    LocalGet {
        /// Local index read.
        local: u8,
        /// Value loaded from the local.
        value: i32,
    },
    /// One local was written.
    LocalSet {
        /// Local index written.
        local: u8,
        /// Value written into the local.
        value: i32,
    },
    /// One local was written while preserving the stack top.
    LocalTee {
        /// Local index written.
        local: u8,
        /// Value written into the local.
        value: i32,
    },
    /// One stack value was discarded.
    Drop {
        /// Value discarded from the stack.
        value: i32,
    },
    /// One unary op was applied.
    UnaryOp {
        /// Unary family.
        op: TassadarUnaryOp,
        /// Operand value.
        operand: i32,
        /// Result value.
        result: i32,
    },
    /// One arithmetic op was applied.
    BinaryOp {
        /// Arithmetic family.
        op: TassadarArithmeticOp,
        /// Left operand.
        left: i32,
        /// Right operand.
        right: i32,
        /// Result value.
        result: i32,
    },
    /// One memory slot was loaded.
    Load {
        /// Slot index read.
        slot: u8,
        /// Value loaded from the slot.
        value: i32,
    },
    /// One memory slot was written.
    Store {
        /// Slot index written.
        slot: u8,
        /// Value written to the slot.
        value: i32,
    },
    /// One conditional branch was evaluated.
    Branch {
        /// Raw condition popped from the stack.
        condition: i32,
        /// Whether the branch was taken.
        taken: bool,
        /// Branch target pc.
        target_pc: usize,
    },
    /// One output was emitted.
    Output {
        /// Value emitted by the host-side output sink.
        value: i32,
    },
    /// Execution returned successfully.
    Return,
}

/// One append-only step in the Tassadar execution trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceStep {
    /// Step index in execution order.
    pub step_index: usize,
    /// Program counter before executing the step.
    pub pc: usize,
    /// Program counter after executing the step.
    pub next_pc: usize,
    /// Instruction executed at `pc`.
    pub instruction: TassadarInstruction,
    /// Event emitted by the step.
    pub event: TassadarTraceEvent,
    /// Stack snapshot before the step.
    pub stack_before: Vec<i32>,
    /// Stack snapshot after the step.
    pub stack_after: Vec<i32>,
    /// Local snapshot after the step.
    pub locals_after: Vec<i32>,
    /// Memory snapshot after the step.
    pub memory_after: Vec<i32>,
}

/// Terminal reason for one Phase 1 execution run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarHaltReason {
    /// The program executed `return`.
    Returned,
    /// The program advanced beyond the end of the instruction list.
    FellOffEnd,
}

/// One complete execution result for the Phase 1 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecution {
    /// Stable program identifier.
    pub program_id: String,
    /// Stable profile identifier.
    pub profile_id: String,
    /// Stable runner identifier.
    pub runner_id: String,
    /// ABI declaration used for the trace.
    pub trace_abi: TassadarTraceAbi,
    /// Ordered append-only steps.
    pub steps: Vec<TassadarTraceStep>,
    /// Output values emitted by the program.
    pub outputs: Vec<i32>,
    /// Final locals snapshot.
    pub final_locals: Vec<i32>,
    /// Final memory snapshot.
    pub final_memory: Vec<i32>,
    /// Final stack snapshot.
    pub final_stack: Vec<i32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarHaltReason,
}

impl TassadarExecution {
    /// Returns a runner-independent digest over the trace and terminal state.
    #[must_use]
    pub fn behavior_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.program_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.profile_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_digest().as_bytes());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.outputs).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.final_locals).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.final_memory).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.final_stack).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(format!("{:?}", self.halt_reason).as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Returns a stable digest over the append-only trace only.
    #[must_use]
    pub fn trace_digest(&self) -> String {
        stable_trace_steps_digest(&self.steps)
    }
}

fn stable_trace_steps_digest(steps: &[TassadarTraceStep]) -> String {
    let bytes = serde_json::to_vec(steps).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

/// Typed difference class for one executor-trace comparison entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTraceDiffKind {
    /// Expected and actual traces disagree on the same step index.
    StepMismatch,
    /// The expected trace had a step the actual trace did not.
    MissingActualStep,
    /// The actual trace had a step the expected trace did not.
    UnexpectedActualStep,
}

/// One typed difference between two executor traces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceDiffEntry {
    /// Difference class for the compared step.
    pub kind: TassadarTraceDiffKind,
    /// Execution-order step index where the mismatch surfaced.
    pub step_index: usize,
    /// Expected step when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<TassadarTraceStep>,
    /// Actual step when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual: Option<TassadarTraceStep>,
}

/// Machine-readable comparison report between two executor traces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceDiffReport {
    /// Stable digest of the expected step stream.
    pub expected_trace_digest: String,
    /// Stable digest of the actual step stream.
    pub actual_trace_digest: String,
    /// Number of expected steps.
    pub expected_step_count: u64,
    /// Number of actual steps.
    pub actual_step_count: u64,
    /// Whether the compared traces are exactly equal.
    pub exact_match: bool,
    /// First differing step index when a mismatch exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_divergence_step_index: Option<usize>,
    /// Typed mismatch entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<TassadarTraceDiffEntry>,
}

impl TassadarTraceDiffReport {
    /// Compares two step streams under the canonical Tassadar trace schema.
    #[must_use]
    pub fn from_steps(expected: &[TassadarTraceStep], actual: &[TassadarTraceStep]) -> Self {
        let mut entries = Vec::new();
        let max_len = expected.len().max(actual.len());
        for step_index in 0..max_len {
            match (expected.get(step_index), actual.get(step_index)) {
                (Some(expected_step), Some(actual_step)) if expected_step != actual_step => {
                    entries.push(TassadarTraceDiffEntry {
                        kind: TassadarTraceDiffKind::StepMismatch,
                        step_index,
                        expected: Some(expected_step.clone()),
                        actual: Some(actual_step.clone()),
                    });
                }
                (Some(expected_step), None) => {
                    entries.push(TassadarTraceDiffEntry {
                        kind: TassadarTraceDiffKind::MissingActualStep,
                        step_index,
                        expected: Some(expected_step.clone()),
                        actual: None,
                    });
                }
                (None, Some(actual_step)) => {
                    entries.push(TassadarTraceDiffEntry {
                        kind: TassadarTraceDiffKind::UnexpectedActualStep,
                        step_index,
                        expected: None,
                        actual: Some(actual_step.clone()),
                    });
                }
                _ => {}
            }
        }

        Self {
            expected_trace_digest: stable_trace_steps_digest(expected),
            actual_trace_digest: stable_trace_steps_digest(actual),
            expected_step_count: expected.len() as u64,
            actual_step_count: actual.len() as u64,
            exact_match: entries.is_empty(),
            first_divergence_step_index: entries.first().map(|entry| entry.step_index),
            entries,
        }
    }

    /// Compares two realized executor runs.
    #[must_use]
    pub fn from_executions(expected: &TassadarExecution, actual: &TassadarExecution) -> Self {
        Self::from_steps(&expected.steps, &actual.steps)
    }
}

fn stable_behavior_digest_from_parts(
    program_id: &str,
    profile_id: &str,
    trace_digest: &str,
    outputs: &[i32],
    final_locals: &[i32],
    final_memory: &[i32],
    final_stack: &[i32],
    halt_reason: TassadarHaltReason,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(program_id.as_bytes());
    hasher.update(b"\n");
    hasher.update(profile_id.as_bytes());
    hasher.update(b"\n");
    hasher.update(trace_digest.as_bytes());
    hasher.update(b"\n");
    hasher.update(serde_json::to_vec(outputs).unwrap_or_default());
    hasher.update(b"\n");
    hasher.update(serde_json::to_vec(final_locals).unwrap_or_default());
    hasher.update(b"\n");
    hasher.update(serde_json::to_vec(final_memory).unwrap_or_default());
    hasher.update(b"\n");
    hasher.update(serde_json::to_vec(final_stack).unwrap_or_default());
    hasher.update(b"\n");
    hasher.update(format!("{:?}", halt_reason).as_bytes());
    hex::encode(hasher.finalize())
}

impl TassadarExecutionSummary {
    fn new(
        program_id: impl Into<String>,
        profile_id: impl Into<String>,
        runner_id: impl Into<String>,
        trace_abi: &TassadarTraceAbi,
        trace_digest: impl Into<String>,
        step_count: u64,
        serialized_trace_bytes: u64,
        outputs: Vec<i32>,
        final_locals: Vec<i32>,
        final_memory: Vec<i32>,
        final_stack: Vec<i32>,
        halt_reason: TassadarHaltReason,
    ) -> Self {
        let program_id = program_id.into();
        let profile_id = profile_id.into();
        let trace_digest = trace_digest.into();
        let behavior_digest = stable_behavior_digest_from_parts(
            &program_id,
            &profile_id,
            &trace_digest,
            outputs.as_slice(),
            final_locals.as_slice(),
            final_memory.as_slice(),
            final_stack.as_slice(),
            halt_reason,
        );
        Self {
            program_id,
            profile_id,
            runner_id: runner_id.into(),
            trace_abi_id: trace_abi.abi_id.clone(),
            trace_abi_version: trace_abi.schema_version,
            trace_digest,
            behavior_digest,
            step_count,
            serialized_trace_bytes,
            outputs,
            final_locals,
            final_memory,
            final_stack,
            halt_reason,
        }
    }
}

/// Emitted trace artifact for one realized Tassadar execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceArtifact {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable digest over the trace artifact.
    pub artifact_digest: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable runner identifier.
    pub runner_id: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI schema version.
    pub trace_abi_version: u16,
    /// Stable digest over the append-only trace.
    pub trace_digest: String,
    /// Stable digest over the behavior-relevant result.
    pub behavior_digest: String,
    /// Number of emitted steps.
    pub step_count: u64,
    /// Ordered append-only steps.
    pub steps: Vec<TassadarTraceStep>,
}

impl TassadarTraceArtifact {
    /// Builds a trace artifact from one execution.
    #[must_use]
    pub fn from_execution(artifact_id: impl Into<String>, execution: &TassadarExecution) -> Self {
        let mut artifact = Self {
            schema_version: TASSADAR_TRACE_ARTIFACT_SCHEMA_VERSION,
            artifact_id: artifact_id.into(),
            artifact_digest: String::new(),
            program_id: execution.program_id.clone(),
            runner_id: execution.runner_id.clone(),
            trace_abi_id: execution.trace_abi.abi_id.clone(),
            trace_abi_version: execution.trace_abi.schema_version,
            trace_digest: execution.trace_digest(),
            behavior_digest: execution.behavior_digest(),
            step_count: execution.steps.len() as u64,
            steps: execution.steps.clone(),
        };
        artifact.refresh_digest();
        artifact
    }

    fn refresh_digest(&mut self) {
        self.artifact_digest = self.compute_digest();
    }

    fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_trace_artifact|");
        hasher.update(self.schema_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.artifact_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.program_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.runner_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_abi_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_abi_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.behavior_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.step_count.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.steps).unwrap_or_default());
        hex::encode(hasher.finalize())
    }
}

impl TassadarTraceSummaryArtifact {
    #[must_use]
    pub fn from_execution_summary(
        artifact_id: impl Into<String>,
        execution: &TassadarExecutionSummary,
    ) -> Self {
        let mut artifact = Self {
            schema_version: TASSADAR_TRACE_ARTIFACT_SCHEMA_VERSION,
            artifact_id: artifact_id.into(),
            artifact_digest: String::new(),
            program_id: execution.program_id.clone(),
            runner_id: execution.runner_id.clone(),
            trace_abi_id: execution.trace_abi_id.clone(),
            trace_abi_version: execution.trace_abi_version,
            trace_digest: execution.trace_digest.clone(),
            behavior_digest: execution.behavior_digest.clone(),
            step_count: execution.step_count,
            serialized_trace_bytes: execution.serialized_trace_bytes,
            outputs: execution.outputs.clone(),
            final_locals: execution.final_locals.clone(),
            final_memory: execution.final_memory.clone(),
            final_stack: execution.final_stack.clone(),
            halt_reason: execution.halt_reason,
        };
        artifact.refresh_digest();
        artifact
    }

    fn refresh_digest(&mut self) {
        self.artifact_digest = self.compute_digest();
    }

    fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_trace_summary_artifact|");
        hasher.update(self.schema_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.artifact_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.program_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.runner_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_abi_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_abi_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.behavior_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.step_count.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.serialized_trace_bytes.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.outputs).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.final_locals).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.final_memory).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.final_stack).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(format!("{:?}", self.halt_reason).as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Proof-bearing trace artifact for one realized Tassadar execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceProofArtifact {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable proof-artifact identifier.
    pub proof_artifact_id: String,
    /// Stable digest over the trace proof.
    pub proof_digest: String,
    /// Stable trace-artifact reference.
    pub trace_artifact_ref: String,
    /// Stable trace-artifact digest.
    pub trace_artifact_digest: String,
    /// Stable trace digest.
    pub trace_digest: String,
    /// Stable validated-program digest.
    pub program_digest: String,
    /// Stable program-artifact digest.
    pub program_artifact_digest: String,
    /// Stable Wasm profile identifier.
    pub wasm_profile_id: String,
    /// Stable model-descriptor digest.
    pub model_descriptor_digest: String,
    /// Decode mode used for the execution.
    pub decode_mode: TassadarExecutorDecodeMode,
    /// Stable cache-algorithm identifier.
    pub cache_algorithm_id: String,
    /// Runtime backend that realized the trace.
    pub runtime_backend: String,
    /// Reference-runner identity.
    pub reference_runner_id: String,
    /// Validation reference carried with the proof.
    pub validation: ValidationMatrixReference,
    /// Stable runtime-manifest identity digest.
    pub runtime_manifest_identity_digest: String,
    /// Stable runtime-manifest digest.
    pub runtime_manifest_digest: String,
}

impl TassadarTraceProofArtifact {
    /// Builds a trace-proof artifact from explicit lineage inputs.
    #[must_use]
    pub fn new(
        proof_artifact_id: impl Into<String>,
        trace_artifact: &TassadarTraceArtifact,
        program_artifact: &TassadarProgramArtifact,
        model_descriptor_digest: impl Into<String>,
        decode_mode: TassadarExecutorDecodeMode,
        runtime_backend: impl Into<String>,
        reference_runner_id: impl Into<String>,
        validation: ValidationMatrixReference,
        runtime_manifest: &RuntimeManifest,
    ) -> Self {
        let mut artifact = Self {
            schema_version: TASSADAR_TRACE_PROOF_SCHEMA_VERSION,
            proof_artifact_id: proof_artifact_id.into(),
            proof_digest: String::new(),
            trace_artifact_ref: trace_artifact.artifact_id.clone(),
            trace_artifact_digest: trace_artifact.artifact_digest.clone(),
            trace_digest: trace_artifact.trace_digest.clone(),
            program_digest: program_artifact.validated_program_digest.clone(),
            program_artifact_digest: program_artifact.artifact_digest.clone(),
            wasm_profile_id: program_artifact.wasm_profile_id.clone(),
            model_descriptor_digest: model_descriptor_digest.into(),
            decode_mode,
            cache_algorithm_id: String::from(decode_mode.cache_algorithm().as_str()),
            runtime_backend: runtime_backend.into(),
            reference_runner_id: reference_runner_id.into(),
            validation,
            runtime_manifest_identity_digest: runtime_manifest.identity_digest.clone(),
            runtime_manifest_digest: runtime_manifest.manifest_digest.clone(),
        };
        artifact.refresh_digest();
        artifact
    }

    /// Builds a trace-proof artifact from a compact summary artifact.
    #[must_use]
    pub fn new_from_summary(
        proof_artifact_id: impl Into<String>,
        trace_artifact: &TassadarTraceSummaryArtifact,
        program_artifact: &TassadarProgramArtifact,
        model_descriptor_digest: impl Into<String>,
        decode_mode: TassadarExecutorDecodeMode,
        runtime_backend: impl Into<String>,
        reference_runner_id: impl Into<String>,
        validation: ValidationMatrixReference,
        runtime_manifest: &RuntimeManifest,
    ) -> Self {
        let mut artifact = Self {
            schema_version: TASSADAR_TRACE_PROOF_SCHEMA_VERSION,
            proof_artifact_id: proof_artifact_id.into(),
            proof_digest: String::new(),
            trace_artifact_ref: trace_artifact.artifact_id.clone(),
            trace_artifact_digest: trace_artifact.artifact_digest.clone(),
            trace_digest: trace_artifact.trace_digest.clone(),
            program_digest: program_artifact.validated_program_digest.clone(),
            program_artifact_digest: program_artifact.artifact_digest.clone(),
            wasm_profile_id: program_artifact.wasm_profile_id.clone(),
            model_descriptor_digest: model_descriptor_digest.into(),
            decode_mode,
            cache_algorithm_id: String::from(decode_mode.cache_algorithm().as_str()),
            runtime_backend: runtime_backend.into(),
            reference_runner_id: reference_runner_id.into(),
            validation,
            runtime_manifest_identity_digest: runtime_manifest.identity_digest.clone(),
            runtime_manifest_digest: runtime_manifest.manifest_digest.clone(),
        };
        artifact.refresh_digest();
        artifact
    }

    fn refresh_digest(&mut self) {
        self.proof_digest = self.compute_digest();
    }

    fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_trace_proof_artifact|");
        hasher.update(self.schema_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.proof_artifact_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_artifact_ref.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_artifact_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.program_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.program_artifact_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.wasm_profile_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.model_descriptor_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.decode_mode.as_str().as_bytes());
        hasher.update(b"\n");
        hasher.update(self.cache_algorithm_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.runtime_backend.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.reference_runner_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.validation).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(self.runtime_manifest_identity_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.runtime_manifest_digest.as_bytes());
        hex::encode(hasher.finalize())
    }
}

impl TassadarTraceSummaryProofReceipt {
    #[must_use]
    pub fn new(
        proof_receipt_id: impl Into<String>,
        trace_artifact: &TassadarTraceSummaryArtifact,
        program_artifact: &TassadarProgramArtifact,
        runtime_backend: impl Into<String>,
        runner_id: impl Into<String>,
        validation: ValidationMatrixReference,
        runtime_manifest: &RuntimeManifest,
    ) -> Self {
        let mut receipt = Self {
            proof_receipt_id: proof_receipt_id.into(),
            proof_receipt_digest: String::new(),
            trace_artifact_ref: trace_artifact.artifact_id.clone(),
            trace_artifact_digest: trace_artifact.artifact_digest.clone(),
            trace_digest: trace_artifact.trace_digest.clone(),
            program_digest: program_artifact.validated_program_digest.clone(),
            program_artifact_digest: program_artifact.artifact_digest.clone(),
            wasm_profile_id: program_artifact.wasm_profile_id.clone(),
            runtime_backend: runtime_backend.into(),
            runner_id: runner_id.into(),
            validation,
            runtime_manifest_identity_digest: runtime_manifest.identity_digest.clone(),
            runtime_manifest_digest: runtime_manifest.manifest_digest.clone(),
        };
        receipt.refresh_digest();
        receipt
    }

    fn refresh_digest(&mut self) {
        self.proof_receipt_digest = self.compute_digest();
    }

    fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_trace_summary_proof_receipt|");
        hasher.update(self.proof_receipt_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_artifact_ref.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_artifact_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.program_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.program_artifact_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.wasm_profile_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.runtime_backend.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.runner_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.validation).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(self.runtime_manifest_identity_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.runtime_manifest_digest.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Runtime-manifest and proof-bundle evidence for one Tassadar execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionEvidenceBundle {
    /// Digest-bound runtime manifest for the execution lane.
    pub runtime_manifest: RuntimeManifest,
    /// Emitted trace artifact.
    pub trace_artifact: TassadarTraceArtifact,
    /// Proof-bearing trace artifact.
    pub trace_proof: TassadarTraceProofArtifact,
    /// Canonical Psionic proof bundle carrying the execution identity.
    pub proof_bundle: ExecutionProofBundle,
}

/// Returns the current validation reference for the Tassadar proof lane.
#[must_use]
pub fn tassadar_validation_reference() -> ValidationMatrixReference {
    ValidationMatrixReference::not_yet_validated("tassadar.executor_trace.phase4")
}

/// Builds runtime-manifest and proof-bundle evidence for one Tassadar execution.
#[must_use]
pub fn build_tassadar_execution_evidence_bundle(
    request_id: impl Into<String>,
    request_digest: impl Into<String>,
    product_id: impl Into<String>,
    model_id: impl Into<String>,
    model_descriptor_digest: impl Into<String>,
    environment_refs: Vec<String>,
    program_artifact: &TassadarProgramArtifact,
    decode_mode: TassadarExecutorDecodeMode,
    execution: &TassadarExecution,
) -> TassadarExecutionEvidenceBundle {
    let request_id = request_id.into();
    let request_digest = request_digest.into();
    let product_id = product_id.into();
    let model_id = model_id.into();
    let model_descriptor_digest = model_descriptor_digest.into();
    let validation = tassadar_validation_reference();
    let trace_artifact = TassadarTraceArtifact::from_execution(
        format!("tassadar://trace/{request_id}/{}", execution.trace_digest()),
        execution,
    );
    let runtime_identity = ExecutionProofRuntimeIdentity::new(
        "cpu",
        BackendToolchainIdentity::new(
            "cpu",
            execution.runner_id.clone(),
            vec![
                String::from("tassadar_executor"),
                String::from(decode_mode.as_str()),
                execution.profile_id.clone(),
            ],
        )
        .with_probe(BackendProbeState::CompiledOnly, Vec::new()),
    );
    let mut runtime_manifest = RuntimeManifest::new(
        format!("tassadar-runtime-manifest-{request_id}"),
        runtime_identity.clone(),
    )
    .with_validation(validation.clone())
    .with_claims_profile_id(TASSADAR_PROOF_CLAIMS_PROFILE_ID)
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ProgramArtifact,
        program_artifact.artifact_id.clone(),
        program_artifact.artifact_digest.clone(),
    ))
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ModelDescriptor,
        model_id.clone(),
        model_descriptor_digest.clone(),
    ))
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ExecutionTrace,
        trace_artifact.artifact_id.clone(),
        trace_artifact.artifact_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.source_program_digest",
        program_artifact.source_identity.stable_digest(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.compile_toolchain_digest",
        program_artifact.toolchain_identity.stable_digest(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.program_digest",
        program_artifact.validated_program_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.decode_mode",
        stable_bytes_digest(decode_mode.as_str().as_bytes()),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.cache_algorithm",
        stable_bytes_digest(decode_mode.cache_algorithm().as_str().as_bytes()),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.reference_runner",
        stable_bytes_digest(execution.runner_id.as_bytes()),
    ));
    for environment_ref in environment_refs {
        runtime_manifest = runtime_manifest.with_environment_ref(environment_ref);
    }

    let trace_proof = TassadarTraceProofArtifact::new(
        format!(
            "tassadar://trace_proof/{request_id}/{}",
            trace_artifact.trace_digest
        ),
        &trace_artifact,
        program_artifact,
        model_descriptor_digest.clone(),
        decode_mode,
        runtime_identity.runtime_backend.clone(),
        execution.runner_id.clone(),
        validation.clone(),
        &runtime_manifest,
    );

    let mut proof_bundle = ExecutionProofBundle::new(
        ExecutionProofBundleKind::Local,
        if execution.halt_reason == TassadarHaltReason::Returned {
            ExecutionProofBundleStatus::Succeeded
        } else {
            ExecutionProofBundleStatus::Failed
        },
        request_id,
        request_digest,
        product_id,
        runtime_identity,
    )
    .with_model_id(model_id)
    .with_validation(validation)
    .with_activation_fingerprint_posture(ExecutionProofAugmentationPosture::Unavailable);
    proof_bundle.artifact_residency = Some(ExecutionProofArtifactResidency {
        served_artifact_digest: None,
        weight_bundle_digest: Some(model_descriptor_digest),
        cluster_artifact_residency_digest: None,
        sharded_model_manifest_digest: None,
        input_artifact_digests: vec![program_artifact.artifact_digest.clone()],
        output_artifact_digests: vec![
            trace_artifact.artifact_digest.clone(),
            trace_proof.proof_digest.clone(),
        ],
        stdout_sha256: None,
        stderr_sha256: None,
    });
    if execution.halt_reason != TassadarHaltReason::Returned {
        proof_bundle =
            proof_bundle.with_failure_reason(format!("tassadar_halt={:?}", execution.halt_reason));
    }

    TassadarExecutionEvidenceBundle {
        runtime_manifest,
        trace_artifact,
        trace_proof,
        proof_bundle,
    }
}

/// Builds runtime-manifest and proof-bundle evidence for one compact
/// long-horizon execution summary.
#[must_use]
pub fn build_tassadar_execution_summary_evidence_bundle(
    request_id: impl Into<String>,
    request_digest: impl Into<String>,
    product_id: impl Into<String>,
    model_id: impl Into<String>,
    model_descriptor_digest: impl Into<String>,
    environment_refs: Vec<String>,
    program_artifact: &TassadarProgramArtifact,
    execution: &TassadarExecutionSummary,
) -> TassadarExecutionSummaryEvidenceBundle {
    let request_id = request_id.into();
    let request_digest = request_digest.into();
    let product_id = product_id.into();
    let model_id = model_id.into();
    let model_descriptor_digest = model_descriptor_digest.into();
    let validation = tassadar_validation_reference();
    let trace_summary_artifact = TassadarTraceSummaryArtifact::from_execution_summary(
        format!(
            "tassadar://trace_summary/{request_id}/{}",
            execution.trace_digest
        ),
        execution,
    );
    let runtime_identity = ExecutionProofRuntimeIdentity::new(
        "cpu",
        BackendToolchainIdentity::new(
            "cpu",
            execution.runner_id.clone(),
            vec![
                String::from("tassadar_executor_summary"),
                execution.profile_id.clone(),
            ],
        )
        .with_probe(BackendProbeState::CompiledOnly, Vec::new()),
    );
    let mut runtime_manifest = RuntimeManifest::new(
        format!("tassadar-runtime-summary-manifest-{request_id}"),
        runtime_identity.clone(),
    )
    .with_validation(validation.clone())
    .with_claims_profile_id(TASSADAR_PROOF_CLAIMS_PROFILE_ID)
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ProgramArtifact,
        program_artifact.artifact_id.clone(),
        program_artifact.artifact_digest.clone(),
    ))
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ModelDescriptor,
        model_id.clone(),
        model_descriptor_digest.clone(),
    ))
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ExecutionTrace,
        trace_summary_artifact.artifact_id.clone(),
        trace_summary_artifact.artifact_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.source_program_digest",
        program_artifact.source_identity.stable_digest(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.compile_toolchain_digest",
        program_artifact.toolchain_identity.stable_digest(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.program_digest",
        program_artifact.validated_program_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.reference_runner",
        stable_bytes_digest(execution.runner_id.as_bytes()),
    ));
    for environment_ref in environment_refs {
        runtime_manifest = runtime_manifest.with_environment_ref(environment_ref);
    }

    let trace_summary_proof = TassadarTraceSummaryProofReceipt::new(
        format!(
            "tassadar://trace_summary_proof/{request_id}/{}",
            trace_summary_artifact.trace_digest
        ),
        &trace_summary_artifact,
        program_artifact,
        runtime_identity.runtime_backend.clone(),
        execution.runner_id.clone(),
        validation.clone(),
        &runtime_manifest,
    );

    let mut proof_bundle = ExecutionProofBundle::new(
        ExecutionProofBundleKind::Local,
        if execution.halt_reason == TassadarHaltReason::Returned {
            ExecutionProofBundleStatus::Succeeded
        } else {
            ExecutionProofBundleStatus::Failed
        },
        request_id,
        request_digest,
        product_id,
        runtime_identity,
    )
    .with_model_id(model_id)
    .with_validation(validation)
    .with_activation_fingerprint_posture(ExecutionProofAugmentationPosture::Unavailable);
    proof_bundle.artifact_residency = Some(ExecutionProofArtifactResidency {
        served_artifact_digest: None,
        weight_bundle_digest: Some(model_descriptor_digest),
        cluster_artifact_residency_digest: None,
        sharded_model_manifest_digest: None,
        input_artifact_digests: vec![program_artifact.artifact_digest.clone()],
        output_artifact_digests: vec![
            trace_summary_artifact.artifact_digest.clone(),
            trace_summary_proof.proof_receipt_digest.clone(),
        ],
        stdout_sha256: None,
        stderr_sha256: None,
    });
    if execution.halt_reason != TassadarHaltReason::Returned {
        proof_bundle =
            proof_bundle.with_failure_reason(format!("tassadar_halt={:?}", execution.halt_reason));
    }

    TassadarExecutionSummaryEvidenceBundle {
        runtime_manifest,
        trace_summary_artifact,
        trace_summary_proof,
        proof_bundle,
    }
}

/// Typed refusal surfaces for unsupported or invalid Phase 1 programs.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutionRefusal {
    /// The program targeted a different profile.
    #[error("profile mismatch: expected `{expected}`, got `{actual}`")]
    ProfileMismatch {
        /// Expected profile identifier.
        expected: String,
        /// Actual program profile identifier.
        actual: String,
    },
    /// The program requested too many locals.
    #[error("program requested {requested} locals, max supported is {max_supported}")]
    TooManyLocals {
        /// Locals requested by the program.
        requested: usize,
        /// Maximum supported locals.
        max_supported: usize,
    },
    /// The program requested too many memory slots.
    #[error("program requested {requested} memory slots, max supported is {max_supported}")]
    TooManyMemorySlots {
        /// Memory slots requested by the program.
        requested: usize,
        /// Maximum supported slots.
        max_supported: usize,
    },
    /// The program exceeded the instruction budget.
    #[error("program uses {instruction_count} instructions, max supported is {max_supported}")]
    ProgramTooLong {
        /// Program instruction count.
        instruction_count: usize,
        /// Maximum supported instruction count.
        max_supported: usize,
    },
    /// The initial memory image does not match the declared slot count.
    #[error("initial memory image length mismatch: expected {expected}, got {actual}")]
    InitialMemoryShapeMismatch {
        /// Expected slot count.
        expected: usize,
        /// Actual memory image length.
        actual: usize,
    },
    /// The program used an opcode not supported by the active profile.
    #[error("unsupported opcode `{}` at pc {}", opcode.mnemonic(), pc)]
    UnsupportedOpcode {
        /// Program counter of the failing instruction.
        pc: usize,
        /// Unsupported opcode.
        opcode: TassadarOpcode,
    },
    /// The program addressed a local outside the declared range.
    #[error(
        "local {} out of range at pc {} (local_count={})",
        local,
        pc,
        local_count
    )]
    LocalOutOfRange {
        /// Program counter of the failing instruction.
        pc: usize,
        /// Requested local index.
        local: usize,
        /// Declared local count.
        local_count: usize,
    },
    /// The program addressed a memory slot outside the declared range.
    #[error(
        "memory slot {} out of range at pc {} (memory_slots={})",
        slot,
        pc,
        memory_slots
    )]
    MemorySlotOutOfRange {
        /// Program counter of the failing instruction.
        pc: usize,
        /// Requested memory slot.
        slot: usize,
        /// Declared memory slot count.
        memory_slots: usize,
    },
    /// The program targeted an invalid branch pc.
    #[error(
        "branch target {} out of range at pc {} (instruction_count={})",
        target_pc,
        pc,
        instruction_count
    )]
    InvalidBranchTarget {
        /// Program counter of the failing instruction.
        pc: usize,
        /// Invalid target pc.
        target_pc: usize,
        /// Total instruction count.
        instruction_count: usize,
    },
    /// Runtime stack underflow occurred.
    #[error("stack underflow at pc {}: needed {}, had {}", pc, needed, available)]
    StackUnderflow {
        /// Program counter of the failing instruction.
        pc: usize,
        /// Number of values required.
        needed: usize,
        /// Number of values available.
        available: usize,
    },
    /// The runtime step budget was exhausted.
    #[error("step limit exceeded: used more than {}", max_steps)]
    StepLimitExceeded {
        /// Maximum step budget for the active profile.
        max_steps: usize,
    },
    /// The fixture lane is missing an opcode rule for the instruction.
    #[error("fixture rule missing for opcode `{}`", opcode.mnemonic())]
    FixtureRuleMissing {
        /// Opcode that could not be resolved in the fixture table.
        opcode: TassadarOpcode,
    },
    /// The Phase 5 hull-cache path does not yet support this control-flow shape.
    #[error(
        "hull-cache fast path does not support backward branch target {} at pc {}",
        target_pc,
        pc
    )]
    HullCacheBackwardBranchUnsupported {
        /// Program counter of the unsupported branch instruction.
        pc: usize,
        /// Unsupported backward target pc.
        target_pc: usize,
    },
    /// The Phase 8 sparse-top-k path does not yet support this control-flow shape.
    #[error(
        "sparse-top-k path does not support backward branch target {} at pc {}",
        target_pc,
        pc
    )]
    SparseTopKBackwardBranchUnsupported {
        /// Program counter of the unsupported branch instruction.
        pc: usize,
        /// Unsupported backward target pc.
        target_pc: usize,
    },
    /// The Phase 8 sparse-top-k path only validates programs up to one bounded length.
    #[error(
        "sparse-top-k path does not support instruction count {} beyond validated limit {}",
        instruction_count,
        max_supported
    )]
    SparseTopKProgramTooLong {
        /// Program instruction count.
        instruction_count: usize,
        /// Maximum validated instruction count for sparse-top-k.
        max_supported: usize,
    },
    /// The fixture table and execution behavior diverged.
    #[error(
        "fixture rule mismatch for `{}`: expected pops={} pushes={}, got pops={} pushes={}",
        opcode.mnemonic(), expected_pops, expected_pushes, actual_pops, actual_pushes
    )]
    FixtureRuleMismatch {
        /// Opcode whose rule diverged.
        opcode: TassadarOpcode,
        /// Expected stack pops from the fixture rule.
        expected_pops: u8,
        /// Expected stack pushes from the fixture rule.
        expected_pushes: u8,
        /// Actual stack pops observed during execution.
        actual_pops: u8,
        /// Actual stack pushes observed during execution.
        actual_pushes: u8,
    },
    /// The fixture and reference lanes disagreed on trace/state behavior.
    #[error(
        "exact parity mismatch for program `{program_id}`: reference={reference_digest} fixture={fixture_digest}"
    )]
    ParityMismatch {
        /// Program identifier compared by the harness.
        program_id: String,
        /// Runner-independent behavior digest from the reference lane.
        reference_digest: String,
        /// Runner-independent behavior digest from the fixture lane.
        fixture_digest: String,
    },
    /// The supplied trace no longer replays exactly against the reference runner.
    #[error(
        "deterministic replay mismatch for program `{program_id}`: expected={expected_digest} actual={actual_digest}"
    )]
    ReplayMismatch {
        /// Program identifier being replayed.
        program_id: String,
        /// Expected runner-independent behavior digest.
        expected_digest: String,
        /// Actual runner-independent behavior digest.
        actual_digest: String,
    },
    /// The direct, linear, hull-cache, and sparse-top-k paths diverged on a validated workload.
    #[error(
        "exact equivalence mismatch for program `{program_id}`: cpu={cpu_reference_digest} linear={reference_linear_digest} hull={hull_cache_digest} sparse={sparse_top_k_digest}"
    )]
    ExactEquivalenceMismatch {
        /// Program identifier being compared.
        program_id: String,
        /// Behavior digest from the direct CPU reference path.
        cpu_reference_digest: String,
        /// Behavior digest from the reference-linear path.
        reference_linear_digest: String,
        /// Behavior digest from the hull-cache fast path.
        hull_cache_digest: String,
        /// Behavior digest from the sparse-top-k path.
        sparse_top_k_digest: String,
    },
}

/// Exact parity report between the reference runner and the fixture runner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarParityReport {
    /// Program identifier compared by the harness.
    pub program_id: String,
    /// Reference execution.
    pub reference: TassadarExecution,
    /// Fixture execution.
    pub fixture: TassadarExecution,
}

impl TassadarParityReport {
    /// Ensures the report is exact across trace, outputs, and terminal state.
    pub fn require_exact(&self) -> Result<(), TassadarExecutionRefusal> {
        if self.reference.behavior_digest() == self.fixture.behavior_digest() {
            Ok(())
        } else {
            Err(TassadarExecutionRefusal::ParityMismatch {
                program_id: self.program_id.clone(),
                reference_digest: self.reference.behavior_digest(),
                fixture_digest: self.fixture.behavior_digest(),
            })
        }
    }
}

/// CPU reference runner for the Phase 1 profile.
#[derive(Clone, Debug, Default)]
pub struct TassadarCpuReferenceRunner {
    profile: TassadarWasmProfile,
    trace_abi: TassadarTraceAbi,
}

impl TassadarCpuReferenceRunner {
    /// Creates the canonical reference runner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            profile: TassadarWasmProfile::core_i32_v1(),
            trace_abi: TassadarTraceAbi::core_i32_v1(),
        }
    }

    /// Creates a reference runner for one supported profile.
    #[must_use]
    pub fn for_profile(profile: TassadarWasmProfile) -> Option<Self> {
        let trace_abi = tassadar_trace_abi_for_profile_id(profile.profile_id.as_str())?;
        Some(Self { profile, trace_abi })
    }

    /// Creates a reference runner that matches one validated program profile.
    pub fn for_program(program: &TassadarProgram) -> Result<Self, TassadarExecutionRefusal> {
        let Some(profile) = tassadar_wasm_profile_for_id(program.profile_id.as_str()) else {
            return Err(TassadarExecutionRefusal::ProfileMismatch {
                expected: supported_wasm_profile_ids_csv(),
                actual: program.profile_id.clone(),
            });
        };
        Self::for_profile(profile).ok_or(TassadarExecutionRefusal::ProfileMismatch {
            expected: supported_wasm_profile_ids_csv(),
            actual: program.profile_id.clone(),
        })
    }

    /// Executes one validated Tassadar program on the direct CPU reference path.
    pub fn execute(
        &self,
        program: &TassadarProgram,
    ) -> Result<TassadarExecution, TassadarExecutionRefusal> {
        execute_program_direct(
            program,
            &self.profile,
            &self.trace_abi,
            TASSADAR_CPU_REFERENCE_RUNNER_ID,
            None,
        )
    }
}

/// Fixture-backed Phase 1 runner using handcrafted opcode-rule tables.
#[derive(Clone, Debug, Default)]
pub struct TassadarFixtureRunner {
    profile: TassadarWasmProfile,
    trace_abi: TassadarTraceAbi,
    weights: TassadarFixtureWeights,
}

impl TassadarFixtureRunner {
    /// Creates the canonical fixture-backed runner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            profile: TassadarWasmProfile::core_i32_v1(),
            trace_abi: TassadarTraceAbi::core_i32_v1(),
            weights: TassadarFixtureWeights::core_i32_v1(),
        }
    }

    /// Creates a fixture-backed runner for one supported profile.
    #[must_use]
    pub fn for_profile(profile: TassadarWasmProfile) -> Option<Self> {
        let trace_abi = tassadar_trace_abi_for_profile_id(profile.profile_id.as_str())?;
        let weights = tassadar_fixture_weights_for_profile_id(profile.profile_id.as_str())?;
        Some(Self {
            profile,
            trace_abi,
            weights,
        })
    }

    /// Creates a fixture-backed runner that matches one validated program profile.
    pub fn for_program(program: &TassadarProgram) -> Result<Self, TassadarExecutionRefusal> {
        let Some(profile) = tassadar_wasm_profile_for_id(program.profile_id.as_str()) else {
            return Err(TassadarExecutionRefusal::ProfileMismatch {
                expected: supported_wasm_profile_ids_csv(),
                actual: program.profile_id.clone(),
            });
        };
        Self::for_profile(profile).ok_or(TassadarExecutionRefusal::ProfileMismatch {
            expected: supported_wasm_profile_ids_csv(),
            actual: program.profile_id.clone(),
        })
    }

    /// Returns the handcrafted rule tables backing the fixture runner.
    #[must_use]
    pub fn weights(&self) -> &TassadarFixtureWeights {
        &self.weights
    }

    /// Executes one validated Tassadar program against the fixture runner.
    pub fn execute(
        &self,
        program: &TassadarProgram,
    ) -> Result<TassadarExecution, TassadarExecutionRefusal> {
        execute_program_linear_decode(
            program,
            &self.profile,
            &self.trace_abi,
            TASSADAR_FIXTURE_RUNNER_ID,
            Some(&self.weights),
        )
    }
}

/// Hull-cache fast-path runner for the validated Phase 5 subset.
#[derive(Clone, Debug, Default)]
pub struct TassadarHullCacheRunner {
    profile: TassadarWasmProfile,
    trace_abi: TassadarTraceAbi,
    weights: TassadarFixtureWeights,
}

impl TassadarHullCacheRunner {
    /// Creates the canonical hull-cache runner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            profile: TassadarWasmProfile::core_i32_v1(),
            trace_abi: TassadarTraceAbi::core_i32_v1(),
            weights: TassadarFixtureWeights::core_i32_v1(),
        }
    }

    /// Creates a hull-cache runner for one supported profile.
    #[must_use]
    pub fn for_profile(profile: TassadarWasmProfile) -> Option<Self> {
        let trace_abi = tassadar_trace_abi_for_profile_id(profile.profile_id.as_str())?;
        let weights = tassadar_fixture_weights_for_profile_id(profile.profile_id.as_str())?;
        Some(Self {
            profile,
            trace_abi,
            weights,
        })
    }

    /// Creates a hull-cache runner that matches one validated program profile.
    pub fn for_program(program: &TassadarProgram) -> Result<Self, TassadarExecutionRefusal> {
        let Some(profile) = tassadar_wasm_profile_for_id(program.profile_id.as_str()) else {
            return Err(TassadarExecutionRefusal::ProfileMismatch {
                expected: supported_wasm_profile_ids_csv(),
                actual: program.profile_id.clone(),
            });
        };
        Self::for_profile(profile).ok_or(TassadarExecutionRefusal::ProfileMismatch {
            expected: supported_wasm_profile_ids_csv(),
            actual: program.profile_id.clone(),
        })
    }

    /// Executes one validated Tassadar program against the hull-cache fast path.
    pub fn execute(
        &self,
        program: &TassadarProgram,
    ) -> Result<TassadarExecution, TassadarExecutionRefusal> {
        execute_program_hull_cache(
            program,
            &self.profile,
            &self.trace_abi,
            TASSADAR_HULL_CACHE_RUNNER_ID,
            Some(&self.weights),
        )
    }
}

/// Research-only hierarchical-hull runner that keeps full write histories so
/// loop-heavy workloads can be benchmarked without relaxing runtime claim
/// boundaries.
#[derive(Clone, Debug, Default)]
pub struct TassadarHierarchicalHullCandidateRunner {
    profile: TassadarWasmProfile,
    trace_abi: TassadarTraceAbi,
    weights: TassadarFixtureWeights,
}

impl TassadarHierarchicalHullCandidateRunner {
    /// Creates the canonical research-only hierarchical-hull runner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            profile: TassadarWasmProfile::core_i32_v1(),
            trace_abi: TassadarTraceAbi::core_i32_v1(),
            weights: TassadarFixtureWeights::core_i32_v1(),
        }
    }

    /// Creates a research-only hierarchical-hull runner for one supported profile.
    #[must_use]
    pub fn for_profile(profile: TassadarWasmProfile) -> Option<Self> {
        let trace_abi = tassadar_trace_abi_for_profile_id(profile.profile_id.as_str())?;
        let weights = tassadar_fixture_weights_for_profile_id(profile.profile_id.as_str())?;
        Some(Self {
            profile,
            trace_abi,
            weights,
        })
    }

    /// Creates a research-only hierarchical-hull runner that matches one
    /// validated program profile.
    pub fn for_program(program: &TassadarProgram) -> Result<Self, TassadarExecutionRefusal> {
        let Some(profile) = tassadar_wasm_profile_for_id(program.profile_id.as_str()) else {
            return Err(TassadarExecutionRefusal::ProfileMismatch {
                expected: supported_wasm_profile_ids_csv(),
                actual: program.profile_id.clone(),
            });
        };
        Self::for_profile(profile).ok_or(TassadarExecutionRefusal::ProfileMismatch {
            expected: supported_wasm_profile_ids_csv(),
            actual: program.profile_id.clone(),
        })
    }

    /// Executes one validated Tassadar program against the research-only
    /// hierarchical-hull candidate.
    pub fn execute(
        &self,
        program: &TassadarProgram,
    ) -> Result<TassadarExecution, TassadarExecutionRefusal> {
        execute_program_hierarchical_hull_candidate(
            program,
            &self.profile,
            &self.trace_abi,
            TASSADAR_HIERARCHICAL_HULL_CANDIDATE_RUNNER_ID,
            Some(&self.weights),
        )
    }
}

/// Sparse-top-k runner for the validated Phase 8 subset.
#[derive(Clone, Debug)]
pub struct TassadarSparseTopKRunner {
    profile: TassadarWasmProfile,
    trace_abi: TassadarTraceAbi,
    weights: TassadarFixtureWeights,
    top_k: usize,
}

impl Default for TassadarSparseTopKRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl TassadarSparseTopKRunner {
    /// Current validated sparse-top-k width.
    pub const VALIDATED_TOP_K: usize = 1;

    /// Creates the canonical sparse-top-k runner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            profile: TassadarWasmProfile::core_i32_v1(),
            trace_abi: TassadarTraceAbi::core_i32_v1(),
            weights: TassadarFixtureWeights::core_i32_v1(),
            top_k: Self::VALIDATED_TOP_K,
        }
    }

    /// Creates a sparse-top-k runner for one supported profile.
    #[must_use]
    pub fn for_profile(profile: TassadarWasmProfile) -> Option<Self> {
        let trace_abi = tassadar_trace_abi_for_profile_id(profile.profile_id.as_str())?;
        let weights = tassadar_fixture_weights_for_profile_id(profile.profile_id.as_str())?;
        Some(Self {
            profile,
            trace_abi,
            weights,
            top_k: Self::VALIDATED_TOP_K,
        })
    }

    /// Creates a sparse-top-k runner that matches one validated program profile.
    pub fn for_program(program: &TassadarProgram) -> Result<Self, TassadarExecutionRefusal> {
        let Some(profile) = tassadar_wasm_profile_for_id(program.profile_id.as_str()) else {
            return Err(TassadarExecutionRefusal::ProfileMismatch {
                expected: supported_wasm_profile_ids_csv(),
                actual: program.profile_id.clone(),
            });
        };
        Self::for_profile(profile).ok_or(TassadarExecutionRefusal::ProfileMismatch {
            expected: supported_wasm_profile_ids_csv(),
            actual: program.profile_id.clone(),
        })
    }

    /// Executes one validated Tassadar program against the sparse-top-k path.
    pub fn execute(
        &self,
        program: &TassadarProgram,
    ) -> Result<TassadarExecution, TassadarExecutionRefusal> {
        execute_program_sparse_top_k(
            program,
            &self.profile,
            &self.trace_abi,
            TASSADAR_SPARSE_TOP_K_RUNNER_ID,
            Some(&self.weights),
            self.top_k,
        )
    }
}

/// Exact-equivalence report across CPU, linear, hull-cache, and sparse-top-k execution paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactEquivalenceReport {
    /// Stable program identifier.
    pub program_id: String,
    /// Direct CPU Wasm-reference execution.
    pub cpu_reference: TassadarExecution,
    /// Linear executor-decode execution.
    pub reference_linear: TassadarExecution,
    /// Hull-cache fast-path execution.
    pub hull_cache: TassadarExecution,
    /// Sparse-top-k execution on the validated subset.
    pub sparse_top_k: TassadarExecution,
}

impl TassadarExactEquivalenceReport {
    /// Returns whether trace digests match exactly across all paths.
    #[must_use]
    pub fn trace_digest_equal(&self) -> bool {
        self.cpu_reference.trace_digest() == self.reference_linear.trace_digest()
            && self.cpu_reference.trace_digest() == self.hull_cache.trace_digest()
            && self.cpu_reference.trace_digest() == self.sparse_top_k.trace_digest()
    }

    /// Returns whether outputs match exactly across all paths.
    #[must_use]
    pub fn outputs_equal(&self) -> bool {
        self.cpu_reference.outputs == self.reference_linear.outputs
            && self.cpu_reference.outputs == self.hull_cache.outputs
            && self.cpu_reference.outputs == self.sparse_top_k.outputs
    }

    /// Returns whether halt reasons match exactly across all paths.
    #[must_use]
    pub fn halt_equal(&self) -> bool {
        self.cpu_reference.halt_reason == self.reference_linear.halt_reason
            && self.cpu_reference.halt_reason == self.hull_cache.halt_reason
            && self.cpu_reference.halt_reason == self.sparse_top_k.halt_reason
    }

    /// Ensures trace, outputs, and halt reason all match across the three paths.
    pub fn require_exact(&self) -> Result<(), TassadarExecutionRefusal> {
        if self.cpu_reference.behavior_digest() == self.reference_linear.behavior_digest()
            && self.cpu_reference.behavior_digest() == self.hull_cache.behavior_digest()
            && self.cpu_reference.behavior_digest() == self.sparse_top_k.behavior_digest()
        {
            Ok(())
        } else {
            Err(TassadarExecutionRefusal::ExactEquivalenceMismatch {
                program_id: self.program_id.clone(),
                cpu_reference_digest: self.cpu_reference.behavior_digest(),
                reference_linear_digest: self.reference_linear.behavior_digest(),
                hull_cache_digest: self.hull_cache.behavior_digest(),
                sparse_top_k_digest: self.sparse_top_k.behavior_digest(),
            })
        }
    }
}

/// Replays one program through the CPU and linear runners and checks exact parity.
pub fn run_tassadar_exact_parity(
    program: &TassadarProgram,
) -> Result<TassadarParityReport, TassadarExecutionRefusal> {
    let reference = TassadarCpuReferenceRunner::for_program(program)?.execute(program)?;
    let fixture = TassadarFixtureRunner::for_program(program)?.execute(program)?;
    let report = TassadarParityReport {
        program_id: program.program_id.clone(),
        reference,
        fixture,
    };
    report.require_exact()?;
    Ok(report)
}

fn run_effective_tassadar_decode(
    program: &TassadarProgram,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> Result<TassadarExecution, TassadarExecutionRefusal> {
    let trace_abi = tassadar_trace_abi_for_profile_id(program.profile_id.as_str()).ok_or(
        TassadarExecutionRefusal::ProfileMismatch {
            expected: String::from("one of the supported Tassadar Wasm profiles"),
            actual: program.profile_id.clone(),
        },
    )?;
    let diagnostic = diagnose_tassadar_executor_request(
        program,
        requested_decode_mode,
        trace_abi.schema_version,
        Some(&[
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarExecutorDecodeMode::HullCache,
            TassadarExecutorDecodeMode::SparseTopK,
        ]),
    );
    let effective_decode_mode =
        diagnostic
            .effective_decode_mode
            .ok_or(TassadarExecutionRefusal::ProfileMismatch {
                expected: format!(
                    "supported decode selection for `{}`",
                    requested_decode_mode.as_str()
                ),
                actual: diagnostic.detail,
            })?;
    match effective_decode_mode {
        TassadarExecutorDecodeMode::ReferenceLinear => {
            TassadarFixtureRunner::for_program(program)?.execute(program)
        }
        TassadarExecutorDecodeMode::HullCache => {
            TassadarHullCacheRunner::for_program(program)?.execute(program)
        }
        TassadarExecutorDecodeMode::SparseTopK => {
            TassadarSparseTopKRunner::for_program(program)?.execute(program)
        }
    }
}

/// Replays one program through CPU, linear, and effective fast-path execution and checks exact equivalence.
pub fn run_tassadar_exact_equivalence(
    program: &TassadarProgram,
) -> Result<TassadarExactEquivalenceReport, TassadarExecutionRefusal> {
    let report = TassadarExactEquivalenceReport {
        program_id: program.program_id.clone(),
        cpu_reference: TassadarCpuReferenceRunner::for_program(program)?.execute(program)?,
        reference_linear: TassadarFixtureRunner::for_program(program)?.execute(program)?,
        hull_cache: run_effective_tassadar_decode(program, TassadarExecutorDecodeMode::HullCache)?,
        sparse_top_k: run_effective_tassadar_decode(
            program,
            TassadarExecutorDecodeMode::SparseTopK,
        )?,
    };
    report.require_exact()?;
    Ok(report)
}

/// Returns the current machine-legible runtime capability report for Tassadar.
#[must_use]
pub fn tassadar_runtime_capability_report() -> TassadarRuntimeCapabilityReport {
    TassadarRuntimeCapabilityReport::current()
}

/// Diagnoses how the runtime would resolve one requested executor decode path.
#[must_use]
pub fn diagnose_tassadar_executor_request(
    program: &TassadarProgram,
    requested_decode_mode: TassadarExecutorDecodeMode,
    requested_trace_abi_version: u16,
    model_supported_decode_modes: Option<&[TassadarExecutorDecodeMode]>,
) -> TassadarExecutorSelectionDiagnostic {
    let capability = tassadar_runtime_capability_report();
    let model_supported_decode_modes =
        model_supported_decode_modes.map_or_else(Vec::new, std::borrow::ToOwned::to_owned);

    if !capability
        .supported_wasm_profiles
        .contains(&program.profile_id)
    {
        return TassadarExecutorSelectionDiagnostic {
            program_id: program.program_id.clone(),
            runtime_backend: capability.runtime_backend,
            requested_profile_id: program.profile_id.clone(),
            requested_trace_abi_version,
            requested_decode_mode,
            effective_decode_mode: None,
            selection_state: TassadarExecutorSelectionState::Refused,
            selection_reason: Some(TassadarExecutorSelectionReason::UnsupportedWasmProfile),
            detail: format!(
                "runtime supports profiles {:?}, but request targeted `{}`",
                capability.supported_wasm_profiles, program.profile_id
            ),
            model_supported_decode_modes,
        };
    }

    if !capability
        .validated_trace_abi_versions
        .contains(&requested_trace_abi_version)
    {
        return TassadarExecutorSelectionDiagnostic {
            program_id: program.program_id.clone(),
            runtime_backend: capability.runtime_backend,
            requested_profile_id: program.profile_id.clone(),
            requested_trace_abi_version,
            requested_decode_mode,
            effective_decode_mode: None,
            selection_state: TassadarExecutorSelectionState::Refused,
            selection_reason: Some(TassadarExecutorSelectionReason::UnsupportedTraceAbiVersion),
            detail: format!(
                "runtime validated ABI versions {:?}, but request targeted `{}`",
                capability.validated_trace_abi_versions, requested_trace_abi_version
            ),
            model_supported_decode_modes,
        };
    }

    let mut diagnostic = match requested_decode_mode {
        TassadarExecutorDecodeMode::ReferenceLinear => TassadarExecutorSelectionDiagnostic {
            program_id: program.program_id.clone(),
            runtime_backend: capability.runtime_backend.clone(),
            requested_profile_id: program.profile_id.clone(),
            requested_trace_abi_version,
            requested_decode_mode,
            effective_decode_mode: Some(TassadarExecutorDecodeMode::ReferenceLinear),
            selection_state: TassadarExecutorSelectionState::Direct,
            selection_reason: None,
            detail: String::from("reference-linear decode requested and supported directly"),
            model_supported_decode_modes: model_supported_decode_modes.clone(),
        },
        TassadarExecutorDecodeMode::HullCache => match validate_hull_cache_program(program) {
            Ok(()) => TassadarExecutorSelectionDiagnostic {
                program_id: program.program_id.clone(),
                runtime_backend: capability.runtime_backend.clone(),
                requested_profile_id: program.profile_id.clone(),
                requested_trace_abi_version,
                requested_decode_mode,
                effective_decode_mode: Some(TassadarExecutorDecodeMode::HullCache),
                selection_state: TassadarExecutorSelectionState::Direct,
                selection_reason: None,
                detail: String::from(
                    "hull-cache decode requested and supported on the validated subset",
                ),
                model_supported_decode_modes: model_supported_decode_modes.clone(),
            },
            Err(TassadarExecutionRefusal::HullCacheBackwardBranchUnsupported { pc, target_pc }) => {
                TassadarExecutorSelectionDiagnostic {
                    program_id: program.program_id.clone(),
                    runtime_backend: capability.runtime_backend.clone(),
                    requested_profile_id: program.profile_id.clone(),
                    requested_trace_abi_version,
                    requested_decode_mode,
                    effective_decode_mode: Some(capability.exact_fallback_decode_mode),
                    selection_state: TassadarExecutorSelectionState::Fallback,
                    selection_reason: Some(
                        TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported,
                    ),
                    detail: format!(
                        "backward branch at pc {} to target {} is outside the validated hull-cache subset; falling back to `{}`",
                        pc,
                        target_pc,
                        capability.exact_fallback_decode_mode.as_str()
                    ),
                    model_supported_decode_modes: model_supported_decode_modes.clone(),
                }
            }
            Err(other) => TassadarExecutorSelectionDiagnostic {
                program_id: program.program_id.clone(),
                runtime_backend: capability.runtime_backend.clone(),
                requested_profile_id: program.profile_id.clone(),
                requested_trace_abi_version,
                requested_decode_mode,
                effective_decode_mode: Some(capability.exact_fallback_decode_mode),
                selection_state: TassadarExecutorSelectionState::Fallback,
                selection_reason: Some(
                    TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported,
                ),
                detail: format!(
                    "requested hull-cache decode fell back to `{}` because the fast path rejected the program: {}",
                    capability.exact_fallback_decode_mode.as_str(),
                    other
                ),
                model_supported_decode_modes: model_supported_decode_modes.clone(),
            },
        },
        TassadarExecutorDecodeMode::SparseTopK => match validate_sparse_top_k_program(program) {
            Ok(()) => TassadarExecutorSelectionDiagnostic {
                program_id: program.program_id.clone(),
                runtime_backend: capability.runtime_backend.clone(),
                requested_profile_id: program.profile_id.clone(),
                requested_trace_abi_version,
                requested_decode_mode,
                effective_decode_mode: Some(TassadarExecutorDecodeMode::SparseTopK),
                selection_state: TassadarExecutorSelectionState::Direct,
                selection_reason: None,
                detail: String::from(
                    "sparse-top-k decode requested and supported on the validated subset",
                ),
                model_supported_decode_modes: model_supported_decode_modes.clone(),
            },
            Err(TassadarExecutionRefusal::SparseTopKBackwardBranchUnsupported {
                pc,
                target_pc,
            }) => TassadarExecutorSelectionDiagnostic {
                program_id: program.program_id.clone(),
                runtime_backend: capability.runtime_backend.clone(),
                requested_profile_id: program.profile_id.clone(),
                requested_trace_abi_version,
                requested_decode_mode,
                effective_decode_mode: Some(capability.exact_fallback_decode_mode),
                selection_state: TassadarExecutorSelectionState::Fallback,
                selection_reason: Some(
                    TassadarExecutorSelectionReason::SparseTopKValidationUnsupported,
                ),
                detail: format!(
                    "backward branch at pc {} to target {} is outside the validated sparse-top-k subset; falling back to `{}`",
                    pc,
                    target_pc,
                    capability.exact_fallback_decode_mode.as_str()
                ),
                model_supported_decode_modes: model_supported_decode_modes.clone(),
            },
            Err(TassadarExecutionRefusal::SparseTopKProgramTooLong {
                instruction_count,
                max_supported,
            }) => TassadarExecutorSelectionDiagnostic {
                program_id: program.program_id.clone(),
                runtime_backend: capability.runtime_backend.clone(),
                requested_profile_id: program.profile_id.clone(),
                requested_trace_abi_version,
                requested_decode_mode,
                effective_decode_mode: Some(capability.exact_fallback_decode_mode),
                selection_state: TassadarExecutorSelectionState::Fallback,
                selection_reason: Some(
                    TassadarExecutorSelectionReason::SparseTopKValidationUnsupported,
                ),
                detail: format!(
                    "instruction count {} exceeds validated sparse-top-k limit {}; falling back to `{}`",
                    instruction_count,
                    max_supported,
                    capability.exact_fallback_decode_mode.as_str()
                ),
                model_supported_decode_modes: model_supported_decode_modes.clone(),
            },
            Err(other) => TassadarExecutorSelectionDiagnostic {
                program_id: program.program_id.clone(),
                runtime_backend: capability.runtime_backend.clone(),
                requested_profile_id: program.profile_id.clone(),
                requested_trace_abi_version,
                requested_decode_mode,
                effective_decode_mode: Some(capability.exact_fallback_decode_mode),
                selection_state: TassadarExecutorSelectionState::Fallback,
                selection_reason: Some(
                    TassadarExecutorSelectionReason::SparseTopKValidationUnsupported,
                ),
                detail: format!(
                    "requested sparse-top-k decode fell back to `{}` because the validated subset rejected the program: {}",
                    capability.exact_fallback_decode_mode.as_str(),
                    other
                ),
                model_supported_decode_modes: model_supported_decode_modes.clone(),
            },
        },
    };

    if let Some(effective_decode_mode) = diagnostic.effective_decode_mode
        && !diagnostic.model_supported_decode_modes.is_empty()
        && !diagnostic
            .model_supported_decode_modes
            .contains(&effective_decode_mode)
    {
        diagnostic.effective_decode_mode = None;
        diagnostic.selection_state = TassadarExecutorSelectionState::Refused;
        diagnostic.selection_reason =
            Some(TassadarExecutorSelectionReason::UnsupportedModelDecodeMode);
        diagnostic.detail = format!(
            "model supports decode modes {:?}, but runtime would need `{}`",
            diagnostic.model_supported_decode_modes,
            effective_decode_mode.as_str()
        );
    }

    diagnostic
}

/// Executes one requested decode path with explicit direct/fallback/refused diagnostics.
pub fn execute_tassadar_executor_request(
    program: &TassadarProgram,
    requested_decode_mode: TassadarExecutorDecodeMode,
    requested_trace_abi_version: u16,
    model_supported_decode_modes: Option<&[TassadarExecutorDecodeMode]>,
) -> Result<TassadarExecutorExecutionReport, TassadarExecutorSelectionDiagnostic> {
    let diagnostic = diagnose_tassadar_executor_request(
        program,
        requested_decode_mode,
        requested_trace_abi_version,
        model_supported_decode_modes,
    );

    let Some(effective_decode_mode) = diagnostic.effective_decode_mode else {
        return Err(diagnostic);
    };

    let execution = match effective_decode_mode {
        TassadarExecutorDecodeMode::ReferenceLinear => TassadarFixtureRunner::for_program(program)
            .map_err(|error| TassadarExecutorSelectionDiagnostic {
                detail: format!(
                    "reference-linear runner could not be constructed for profile `{}`: {error}",
                    program.profile_id
                ),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            })?
            .execute(program)
            .map_err(|error| TassadarExecutorSelectionDiagnostic {
                detail: format!("reference-linear execution refused after selection: {error}"),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            })?,
        TassadarExecutorDecodeMode::HullCache => TassadarHullCacheRunner::for_program(program)
            .map_err(|error| TassadarExecutorSelectionDiagnostic {
                detail: format!(
                    "hull-cache runner could not be constructed for profile `{}`: {error}",
                    program.profile_id
                ),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            })?
            .execute(program)
            .map_err(|error| TassadarExecutorSelectionDiagnostic {
                detail: format!("hull-cache execution refused after selection: {error}"),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            })?,
        TassadarExecutorDecodeMode::SparseTopK => TassadarSparseTopKRunner::for_program(program)
            .map_err(|error| TassadarExecutorSelectionDiagnostic {
                detail: format!(
                    "sparse-top-k runner could not be constructed for profile `{}`: {error}",
                    program.profile_id
                ),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            })?
            .execute(program)
            .map_err(|error| TassadarExecutorSelectionDiagnostic {
                detail: format!("sparse-top-k execution refused after selection: {error}"),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            })?,
    };

    Ok(TassadarExecutorExecutionReport {
        selection: diagnostic,
        execution,
    })
}

/// Executes one requested decode path into a compact summary with explicit
/// direct/fallback/refused diagnostics.
pub fn execute_tassadar_executor_request_summary(
    program: &TassadarProgram,
    requested_decode_mode: TassadarExecutorDecodeMode,
    requested_trace_abi_version: u16,
    model_supported_decode_modes: Option<&[TassadarExecutorDecodeMode]>,
) -> Result<TassadarExecutorExecutionSummaryReport, TassadarExecutorSelectionDiagnostic> {
    let diagnostic = diagnose_tassadar_executor_request(
        program,
        requested_decode_mode,
        requested_trace_abi_version,
        model_supported_decode_modes,
    );

    let Some(effective_decode_mode) = diagnostic.effective_decode_mode else {
        return Err(diagnostic);
    };
    let trace_abi =
        tassadar_trace_abi_for_profile_id(program.profile_id.as_str()).ok_or_else(|| {
            TassadarExecutorSelectionDiagnostic {
                detail: format!(
                    "no trace ABI is registered for selected profile `{}`",
                    program.profile_id
                ),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            }
        })?;

    let execution_summary = match effective_decode_mode {
        TassadarExecutorDecodeMode::ReferenceLinear => TassadarFixtureRunner::for_program(program)
            .map_err(|error| TassadarExecutorSelectionDiagnostic {
                detail: format!(
                    "reference-linear runner could not be constructed for profile `{}`: {error}",
                    program.profile_id
                ),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            })
            .and_then(|runner| {
                execute_program_direct_summary(
                    program,
                    &runner.profile,
                    &runner.trace_abi,
                    TASSADAR_FIXTURE_RUNNER_ID,
                )
                .map_err(|error| TassadarExecutorSelectionDiagnostic {
                    detail: format!("reference-linear execution refused after selection: {error}"),
                    selection_state: TassadarExecutorSelectionState::Refused,
                    selection_reason: diagnostic.selection_reason,
                    effective_decode_mode: None,
                    ..diagnostic.clone()
                })
            })?,
        TassadarExecutorDecodeMode::HullCache => TassadarHullCacheRunner::for_program(program)
            .map_err(|error| TassadarExecutorSelectionDiagnostic {
                detail: format!(
                    "hull-cache runner could not be constructed for profile `{}`: {error}",
                    program.profile_id
                ),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            })
            .and_then(|runner| {
                execute_program_hull_cache_summary(
                    program,
                    &runner.profile,
                    &trace_abi,
                    TASSADAR_HULL_CACHE_RUNNER_ID,
                    &runner.weights,
                )
                .map_err(|error| TassadarExecutorSelectionDiagnostic {
                    detail: format!("hull-cache execution refused after selection: {error}"),
                    selection_state: TassadarExecutorSelectionState::Refused,
                    selection_reason: diagnostic.selection_reason,
                    effective_decode_mode: None,
                    ..diagnostic.clone()
                })
            })?,
        TassadarExecutorDecodeMode::SparseTopK => TassadarSparseTopKRunner::for_program(program)
            .map_err(|error| TassadarExecutorSelectionDiagnostic {
                detail: format!(
                    "sparse-top-k runner could not be constructed for profile `{}`: {error}",
                    program.profile_id
                ),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            })?
            .execute(program)
            .map(|execution| execution_summary_from_execution(&execution))
            .map_err(|error| TassadarExecutorSelectionDiagnostic {
                detail: format!("sparse-top-k execution refused after selection: {error}"),
                selection_state: TassadarExecutorSelectionState::Refused,
                selection_reason: diagnostic.selection_reason,
                effective_decode_mode: None,
                ..diagnostic.clone()
            })?,
    };

    Ok(TassadarExecutorExecutionSummaryReport {
        selection: diagnostic,
        execution_summary,
    })
}

/// Deterministically replays the supplied execution against the direct CPU runner.
pub fn replay_tassadar_execution(
    program: &TassadarProgram,
    expected: &TassadarExecution,
) -> Result<(), TassadarExecutionRefusal> {
    let actual = TassadarCpuReferenceRunner::for_program(program)?.execute(program)?;
    if actual.behavior_digest() == expected.behavior_digest() {
        Ok(())
    } else {
        Err(TassadarExecutionRefusal::ReplayMismatch {
            program_id: program.program_id.clone(),
            expected_digest: expected.behavior_digest(),
            actual_digest: actual.behavior_digest(),
        })
    }
}

/// Small reference corpus that keeps the Phase 1 trace/output boundary honest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarValidationCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Short case summary.
    pub summary: String,
    /// Validated Phase 1 program.
    pub program: TassadarProgram,
    /// Exact expected trace for the case.
    pub expected_trace: Vec<TassadarTraceStep>,
    /// Exact expected output values.
    pub expected_outputs: Vec<i32>,
}

/// Stable split identity for one real 4x4 Sudoku-v0 corpus case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSudokuV0CorpusSplit {
    /// Training split for later tokenization and model fitting.
    Train,
    /// Validation split for exactness reporting during training.
    Validation,
    /// Held-out test split for later trained-model checks.
    Test,
}

impl TassadarSudokuV0CorpusSplit {
    /// Returns the stable split identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Validation => "validation",
            Self::Test => "test",
        }
    }
}

/// One real 4x4 Sudoku-v0 corpus case backed by the honest search program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudokuV0CorpusCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable split assignment.
    pub split: TassadarSudokuV0CorpusSplit,
    /// Flat 4x4 puzzle cells where `0` denotes an empty square.
    pub puzzle_cells: Vec<i32>,
    /// Number of fixed givens in the puzzle.
    pub given_count: usize,
    /// Exact CPU-reference-backed validation case.
    pub validation_case: TassadarValidationCase,
}

/// One real 9x9 Sudoku-class corpus case backed by the honest search program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9CorpusCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable split assignment.
    pub split: TassadarSudokuV0CorpusSplit,
    /// Flat 9x9 puzzle cells where `0` denotes an empty square.
    pub puzzle_cells: Vec<i32>,
    /// Number of fixed givens in the puzzle.
    pub given_count: usize,
    /// Exact CPU-reference-backed validation case.
    pub validation_case: TassadarValidationCase,
}

/// One bounded 4x4 min-cost perfect-matching corpus case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarianV0CorpusCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable split assignment.
    pub split: TassadarSudokuV0CorpusSplit,
    /// Flat 4x4 cost matrix in row-major order.
    pub cost_matrix: Vec<i32>,
    /// Exact optimal assignment encoded as one chosen column per row.
    pub optimal_assignment: Vec<i32>,
    /// Exact optimal assignment cost.
    pub optimal_cost: i32,
    /// Exact CPU-reference-backed validation case.
    pub validation_case: TassadarValidationCase,
}

/// One article-sized 10x10 min-cost perfect-matching corpus case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarian10x10CorpusCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable split assignment.
    pub split: TassadarSudokuV0CorpusSplit,
    /// Flat 10x10 cost matrix in row-major order.
    pub cost_matrix: Vec<i32>,
    /// Stable search-row order used by the exact branch-and-bound program.
    pub search_row_order: Vec<usize>,
    /// Exact optimal assignment encoded as one chosen column per row.
    pub optimal_assignment: Vec<i32>,
    /// Exact optimal assignment cost.
    pub optimal_cost: i32,
    /// Exact CPU-reference-backed validation case.
    pub validation_case: TassadarValidationCase,
}

const TASSADAR_SUDOKU_V0_CELL_COUNT: usize = 16;
const TASSADAR_SUDOKU_V0_GRID_WIDTH: usize = 4;
const TASSADAR_SUDOKU_V0_BOX_WIDTH: usize = 2;
const TASSADAR_SUDOKU_9X9_CELL_COUNT: usize = 81;
const TASSADAR_SUDOKU_9X9_GRID_WIDTH: usize = 9;
const TASSADAR_SUDOKU_9X9_BOX_WIDTH: usize = 3;
const TASSADAR_HUNGARIAN_V0_DIM: usize = 4;
const TASSADAR_HUNGARIAN_V0_MATRIX_CELL_COUNT: usize = 16;
const TASSADAR_HUNGARIAN_V0_OUTPUT_SLOT_BASE: u8 = 16;
const TASSADAR_HUNGARIAN_V0_BEST_COST_SLOT: u8 = 20;
const TASSADAR_HUNGARIAN_V0_MEMORY_SLOTS: usize = 21;
const TASSADAR_HUNGARIAN_10X10_DIM: usize = 10;
const TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT: usize = 100;
const TASSADAR_HUNGARIAN_10X10_BEST_COST_SLOT: u8 = 0;
const TASSADAR_HUNGARIAN_10X10_BEST_ASSIGNMENT_SLOT_BASE: u8 = 1;
const TASSADAR_HUNGARIAN_10X10_CURRENT_ASSIGNMENT_SLOT_BASE: u8 = 11;
const TASSADAR_HUNGARIAN_10X10_NEXT_CANDIDATE_SLOT_BASE: u8 = 21;
const TASSADAR_HUNGARIAN_10X10_USED_COLUMN_SLOT_BASE: u8 = 31;
const TASSADAR_HUNGARIAN_10X10_MEMORY_SLOTS: usize = 41;
const TASSADAR_HUNGARIAN_10X10_ROW_LOCAL: u8 = 0;
const TASSADAR_HUNGARIAN_10X10_CURRENT_COST_LOCAL: u8 = 1;
const TASSADAR_HUNGARIAN_10X10_CANDIDATE_COST_LOCAL: u8 = 2;
const TASSADAR_HUNGARIAN_10X10_INITIAL_BEST_COST: i32 = 1_000_000;
const TASSADAR_HUNGARIAN_10X10_ARTICLE_BASE_MATRIX: [i32;
    TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT] = [
    61, 58, 35, 86, 32, 39, 41, 27, 21, 42, //
    59, 77, 97, 99, 78, 21, 89, 72, 35, 63, //
    88, 85, 37, 57, 59, 97, 37, 29, 69, 94, //
    32, 82, 53, 20, 77, 96, 21, 70, 50, 61, //
    15, 44, 81, 10, 64, 36, 56, 78, 20, 69, //
    76, 35, 87, 69, 16, 55, 26, 37, 30, 66, //
    86, 32, 74, 94, 32, 14, 24, 12, 31, 70, //
    97, 63, 20, 64, 90, 21, 28, 49, 89, 10, //
    58, 52, 27, 76, 61, 35, 17, 91, 37, 66, //
    42, 79, 61, 26, 55, 98, 70, 17, 26, 86,
];
const TASSADAR_SUDOKU_9X9_SOLVED_GRID: [i32; TASSADAR_SUDOKU_9X9_CELL_COUNT] = [
    5, 3, 4, 6, 7, 8, 9, 1, 2, //
    6, 7, 2, 1, 9, 5, 3, 4, 8, //
    1, 9, 8, 3, 4, 2, 5, 6, 7, //
    8, 5, 9, 7, 6, 1, 4, 2, 3, //
    4, 2, 6, 8, 5, 3, 7, 9, 1, //
    7, 1, 3, 9, 2, 4, 8, 5, 6, //
    9, 6, 1, 5, 3, 7, 2, 8, 4, //
    2, 8, 7, 4, 1, 9, 6, 3, 5, //
    3, 4, 5, 2, 8, 6, 1, 7, 9,
];
const TASSADAR_SUDOKU_9X9_ARTO_INKALA_PUZZLE: [i32; TASSADAR_SUDOKU_9X9_CELL_COUNT] = [
    8, 0, 0, 0, 0, 0, 0, 0, 0, //
    0, 0, 3, 6, 0, 0, 0, 0, 0, //
    0, 7, 0, 0, 9, 0, 2, 0, 0, //
    0, 5, 0, 0, 0, 7, 0, 0, 0, //
    0, 0, 0, 0, 4, 5, 7, 0, 0, //
    0, 0, 0, 1, 0, 0, 0, 3, 0, //
    0, 0, 1, 0, 0, 0, 0, 6, 8, //
    0, 0, 8, 5, 0, 0, 0, 1, 0, //
    0, 9, 0, 0, 0, 0, 4, 0, 0,
];

/// Builds a real 4x4 Sudoku-v0 backtracking search program in the widened Wasm-first lane.
#[must_use]
pub fn tassadar_sudoku_v0_search_program(
    program_id: impl Into<String>,
    puzzle_cells: [i32; TASSADAR_SUDOKU_V0_CELL_COUNT],
) -> TassadarProgram {
    build_tassadar_sudoku_search_program(
        program_id,
        &TassadarWasmProfile::sudoku_v0_search_v1(),
        "sudoku_v0",
        TASSADAR_SUDOKU_V0_GRID_WIDTH,
        TASSADAR_SUDOKU_V0_BOX_WIDTH,
        puzzle_cells.as_slice(),
    )
}

/// Builds a real 9x9 Sudoku-class backtracking search program.
#[must_use]
pub fn tassadar_sudoku_9x9_search_program(
    program_id: impl Into<String>,
    puzzle_cells: [i32; TASSADAR_SUDOKU_9X9_CELL_COUNT],
) -> TassadarProgram {
    build_tassadar_sudoku_search_program(
        program_id,
        &TassadarWasmProfile::sudoku_9x9_search_v1(),
        "sudoku_9x9",
        TASSADAR_SUDOKU_9X9_GRID_WIDTH,
        TASSADAR_SUDOKU_9X9_BOX_WIDTH,
        puzzle_cells.as_slice(),
    )
}

/// Builds a bounded 4x4 min-cost perfect-matching program via exact permutation enumeration.
#[must_use]
pub fn tassadar_hungarian_v0_matching_program(
    program_id: impl Into<String>,
    cost_matrix: [i32; TASSADAR_HUNGARIAN_V0_MATRIX_CELL_COUNT],
) -> TassadarProgram {
    let profile = TassadarWasmProfile::hungarian_v0_matching_v1();
    let mut assembler = TassadarLabelAssembler::default();
    let permutations = hungarian_v0_permutations();

    for (index, permutation) in permutations.iter().enumerate() {
        emit_hungarian_v0_candidate_cost(&mut assembler, permutation);
        assembler.emit(TassadarInstruction::LocalSet { local: 0 });
        if index == 0 {
            emit_hungarian_v0_best_update(&mut assembler, permutation);
            continue;
        }

        let update_label = format!("hungarian_v0_update_best_{index}");
        let next_label = format!("hungarian_v0_next_candidate_{index}");
        assembler.emit(TassadarInstruction::LocalGet { local: 0 });
        assembler.emit(TassadarInstruction::I32Load {
            slot: TASSADAR_HUNGARIAN_V0_BEST_COST_SLOT,
        });
        assembler.emit(TassadarInstruction::I32Lt);
        assembler.branch_if(update_label.as_str());
        assembler.branch_always(next_label.as_str());
        assembler.label(update_label.as_str());
        emit_hungarian_v0_best_update(&mut assembler, permutation);
        assembler.label(next_label.as_str());
    }

    for slot in TASSADAR_HUNGARIAN_V0_OUTPUT_SLOT_BASE
        ..TASSADAR_HUNGARIAN_V0_OUTPUT_SLOT_BASE + TASSADAR_HUNGARIAN_V0_DIM as u8
    {
        assembler.emit(TassadarInstruction::I32Load { slot });
        assembler.emit(TassadarInstruction::Output);
    }
    assembler.emit(TassadarInstruction::I32Load {
        slot: TASSADAR_HUNGARIAN_V0_BEST_COST_SLOT,
    });
    assembler.emit(TassadarInstruction::Output);
    assembler.emit(TassadarInstruction::Return);

    let mut initial_memory = vec![0; TASSADAR_HUNGARIAN_V0_MEMORY_SLOTS];
    initial_memory[..TASSADAR_HUNGARIAN_V0_MATRIX_CELL_COUNT]
        .copy_from_slice(cost_matrix.as_slice());
    TassadarProgram::new(
        program_id,
        &profile,
        1,
        TASSADAR_HUNGARIAN_V0_MEMORY_SLOTS,
        assembler.finalize(),
    )
    .with_initial_memory(initial_memory)
}

/// Returns the canonical real 4x4 Sudoku-v0 corpus with stable split assignments.
#[must_use]
pub fn tassadar_sudoku_v0_corpus() -> Vec<TassadarSudokuV0CorpusCase> {
    vec![
        computed_sudoku_v0_corpus_case(
            "sudoku_v0_train_a",
            TassadarSudokuV0CorpusSplit::Train,
            [
                1, 0, 0, 4, //
                0, 4, 1, 0, //
                0, 1, 0, 3, //
                4, 0, 2, 0,
            ],
        ),
        computed_sudoku_v0_corpus_case(
            "sudoku_v0_train_b",
            TassadarSudokuV0CorpusSplit::Train,
            [
                0, 1, 0, 0, //
                0, 4, 1, 0, //
                1, 0, 0, 4, //
                0, 3, 0, 0,
            ],
        ),
        computed_sudoku_v0_corpus_case(
            "sudoku_v0_train_c",
            TassadarSudokuV0CorpusSplit::Train,
            [
                0, 0, 0, 0, //
                3, 0, 0, 0, //
                0, 0, 0, 4, //
                0, 0, 1, 0,
            ],
        ),
        computed_sudoku_v0_corpus_case(
            "sudoku_v0_train_d",
            TassadarSudokuV0CorpusSplit::Train,
            [
                1, 0, 0, 0, //
                0, 0, 1, 0, //
                0, 1, 0, 0, //
                0, 0, 0, 1,
            ],
        ),
        computed_sudoku_v0_corpus_case(
            "sudoku_v0_validation_a",
            TassadarSudokuV0CorpusSplit::Validation,
            [
                0, 2, 0, 4, //
                3, 0, 0, 0, //
                0, 0, 4, 0, //
                0, 3, 0, 1,
            ],
        ),
        computed_sudoku_v0_corpus_case(
            "sudoku_v0_validation_b",
            TassadarSudokuV0CorpusSplit::Validation,
            [
                0, 0, 4, 0, //
                0, 4, 0, 0, //
                1, 0, 0, 4, //
                0, 3, 0, 0,
            ],
        ),
        computed_sudoku_v0_corpus_case(
            "sudoku_v0_test_a",
            TassadarSudokuV0CorpusSplit::Test,
            [
                2, 0, 0, 0, //
                0, 4, 0, 2, //
                0, 0, 3, 0, //
                4, 0, 0, 1,
            ],
        ),
        computed_sudoku_v0_corpus_case(
            "sudoku_v0_test_b",
            TassadarSudokuV0CorpusSplit::Test,
            [
                0, 0, 0, 4, //
                3, 0, 1, 0, //
                0, 1, 0, 0, //
                0, 3, 0, 0,
            ],
        ),
    ]
}

/// Returns the canonical real 9x9 Sudoku-class corpus with stable split assignments.
#[must_use]
pub fn tassadar_sudoku_9x9_corpus() -> Vec<TassadarSudoku9x9CorpusCase> {
    vec![
        masked_sudoku_9x9_corpus_case(
            "sudoku_9x9_train_a",
            TassadarSudokuV0CorpusSplit::Train,
            &[
                0, 4, 8, 9, 13, 17, 20, 24, 28, 31, 35, 38, 40, 42, 46, 48, 53, 55, 59, 61, 63, 67,
                71, 72, 76, 80,
            ],
        ),
        masked_sudoku_9x9_corpus_case(
            "sudoku_9x9_train_b",
            TassadarSudokuV0CorpusSplit::Train,
            &[
                1, 5, 6, 10, 14, 16, 18, 22, 26, 29, 33, 34, 36, 39, 43, 45, 50, 52, 56, 58, 62,
                64, 68, 69, 74, 78,
            ],
        ),
        masked_sudoku_9x9_corpus_case(
            "sudoku_9x9_validation_a",
            TassadarSudokuV0CorpusSplit::Validation,
            &[
                2, 3, 7, 11, 12, 15, 19, 23, 25, 27, 30, 32, 37, 41, 44, 47, 49, 51, 54, 57, 60,
                65, 66, 70, 73, 75, 77, 79,
            ],
        ),
        masked_sudoku_9x9_corpus_case(
            "sudoku_9x9_test_a",
            TassadarSudokuV0CorpusSplit::Test,
            &[
                0, 2, 5, 8, 10, 13, 16, 20, 24, 28, 31, 34, 36, 40, 44, 46, 49, 52, 56, 60, 64, 67,
                70, 72, 75, 78, 80,
            ],
        ),
    ]
}

/// Returns the canonical Arto Inkala hard-Sudoku article fixture.
#[must_use]
pub fn tassadar_sudoku_9x9_arto_inkala_case() -> TassadarSudoku9x9CorpusCase {
    let search_cells = ordered_empty_sudoku_cells(
        TASSADAR_SUDOKU_9X9_ARTO_INKALA_PUZZLE.as_slice(),
        TASSADAR_SUDOKU_9X9_GRID_WIDTH,
        TASSADAR_SUDOKU_9X9_BOX_WIDTH,
    );
    externally_solved_sudoku_9x9_corpus_case_with_profile_and_search_cells(
        "sudoku_9x9_arto_inkala",
        TassadarSudokuV0CorpusSplit::Test,
        TASSADAR_SUDOKU_9X9_ARTO_INKALA_PUZZLE,
        &TassadarWasmProfile::sudoku_9x9_hard_search_v1(),
        &search_cells,
    )
}

/// Returns the declared hard-Sudoku article suite used to stand in for the
/// article's benchmark-wide Sudoku claim.
#[must_use]
pub fn tassadar_article_hard_sudoku_suite() -> Vec<TassadarSudoku9x9CorpusCase> {
    let stand_in_case = tassadar_sudoku_9x9_corpus()
        .into_iter()
        .find(|case| case.case_id == "sudoku_9x9_test_a")
        .expect("hard-Sudoku stand-in case should exist");
    vec![stand_in_case, tassadar_sudoku_9x9_arto_inkala_case()]
}

/// Returns the canonical bounded 4x4 Hungarian-v0 corpus with stable split assignments.
#[must_use]
pub fn tassadar_hungarian_v0_corpus() -> Vec<TassadarHungarianV0CorpusCase> {
    vec![
        computed_hungarian_v0_corpus_case(
            "hungarian_v0_train_a",
            TassadarSudokuV0CorpusSplit::Train,
            [1, 0, 3, 2],
            0,
        ),
        computed_hungarian_v0_corpus_case(
            "hungarian_v0_train_b",
            TassadarSudokuV0CorpusSplit::Train,
            [2, 3, 0, 1],
            1,
        ),
        computed_hungarian_v0_corpus_case(
            "hungarian_v0_train_c",
            TassadarSudokuV0CorpusSplit::Train,
            [3, 1, 2, 0],
            2,
        ),
        computed_hungarian_v0_corpus_case(
            "hungarian_v0_train_d",
            TassadarSudokuV0CorpusSplit::Train,
            [0, 2, 1, 3],
            3,
        ),
        computed_hungarian_v0_corpus_case(
            "hungarian_v0_validation_a",
            TassadarSudokuV0CorpusSplit::Validation,
            [2, 0, 3, 1],
            4,
        ),
        computed_hungarian_v0_corpus_case(
            "hungarian_v0_validation_b",
            TassadarSudokuV0CorpusSplit::Validation,
            [3, 2, 1, 0],
            5,
        ),
        computed_hungarian_v0_corpus_case(
            "hungarian_v0_test_a",
            TassadarSudokuV0CorpusSplit::Test,
            [1, 3, 0, 2],
            6,
        ),
        computed_hungarian_v0_corpus_case(
            "hungarian_v0_test_b",
            TassadarSudokuV0CorpusSplit::Test,
            [0, 3, 2, 1],
            7,
        ),
    ]
}

/// Returns the canonical article-sized 10x10 Hungarian-class corpus.
#[must_use]
pub fn tassadar_hungarian_10x10_corpus() -> Vec<TassadarHungarian10x10CorpusCase> {
    vec![
        computed_hungarian_10x10_corpus_case(
            "hungarian_10x10_train_a",
            TassadarSudokuV0CorpusSplit::Train,
            TASSADAR_HUNGARIAN_10X10_ARTICLE_BASE_MATRIX,
        ),
        computed_hungarian_10x10_corpus_case(
            "hungarian_10x10_train_b",
            TassadarSudokuV0CorpusSplit::Train,
            transformed_hungarian_10x10_cost_matrix(
                [4, 1, 7, 0, 8, 2, 9, 3, 5, 6],
                [8, 5, 2, 3, 0, 4, 1, 9, 6, 7],
                [0, 2, 4, 1, 3, 5, 2, 4, 1, 3],
                [1, 0, 2, 1, 3, 0, 2, 1, 0, 2],
            ),
        ),
        computed_hungarian_10x10_corpus_case(
            "hungarian_10x10_validation_a",
            TassadarSudokuV0CorpusSplit::Validation,
            transformed_hungarian_10x10_cost_matrix(
                [6, 3, 9, 2, 5, 8, 1, 4, 0, 7],
                [3, 0, 7, 2, 8, 5, 1, 9, 4, 6],
                [0, 1, 2, 0, 1, 2, 0, 1, 2, 0],
                [0, 1, 0, 1, 0, 1, 0, 1, 0, 1],
            ),
        ),
        computed_hungarian_10x10_corpus_case(
            "hungarian_10x10_test_a",
            TassadarSudokuV0CorpusSplit::Test,
            transformed_hungarian_10x10_cost_matrix(
                [2, 5, 8, 1, 4, 7, 0, 3, 6, 9],
                [1, 4, 7, 0, 3, 6, 9, 2, 5, 8],
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
                [0, 2, 4, 6, 8, 10, 12, 14, 16, 18],
            ),
        ),
    ]
}

fn computed_sudoku_v0_corpus_case(
    case_id: &str,
    split: TassadarSudokuV0CorpusSplit,
    puzzle_cells: [i32; TASSADAR_SUDOKU_V0_CELL_COUNT],
) -> TassadarSudokuV0CorpusCase {
    let given_count = puzzle_cells.iter().filter(|value| **value != 0).count();
    let program = tassadar_sudoku_v0_search_program(format!("tassadar.{case_id}.v1"), puzzle_cells);
    let execution = TassadarCpuReferenceRunner::for_program(&program)
        .expect("Sudoku-v0 search profile should resolve on CPU")
        .execute(&program)
        .expect("Sudoku-v0 corpus puzzle should solve exactly");
    assert_eq!(
        execution.outputs.len(),
        TASSADAR_SUDOKU_V0_CELL_COUNT,
        "Sudoku-v0 corpus case `{case_id}` should emit a full solved grid"
    );
    let summary = format!(
        "real 4x4 Sudoku-v0 backtracking puzzle on the {} split with {} givens",
        split.as_str(),
        given_count
    );
    TassadarSudokuV0CorpusCase {
        case_id: String::from(case_id),
        split,
        puzzle_cells: puzzle_cells.to_vec(),
        given_count,
        validation_case: TassadarValidationCase {
            case_id: String::from(case_id),
            summary,
            program,
            expected_trace: execution.steps,
            expected_outputs: execution.outputs,
        },
    }
}

fn masked_sudoku_9x9_corpus_case(
    case_id: &str,
    split: TassadarSudokuV0CorpusSplit,
    masked_indices: &[usize],
) -> TassadarSudoku9x9CorpusCase {
    let mut puzzle_cells = TASSADAR_SUDOKU_9X9_SOLVED_GRID;
    for index in masked_indices {
        puzzle_cells[*index] = 0;
    }
    computed_sudoku_9x9_corpus_case(case_id, split, puzzle_cells)
}

fn computed_sudoku_9x9_corpus_case(
    case_id: &str,
    split: TassadarSudokuV0CorpusSplit,
    puzzle_cells: [i32; TASSADAR_SUDOKU_9X9_CELL_COUNT],
) -> TassadarSudoku9x9CorpusCase {
    let search_cells: Vec<_> = (0..TASSADAR_SUDOKU_9X9_CELL_COUNT).collect();
    computed_sudoku_9x9_corpus_case_with_search_cells(case_id, split, puzzle_cells, &search_cells)
}

fn computed_sudoku_9x9_corpus_case_with_search_cells(
    case_id: &str,
    split: TassadarSudokuV0CorpusSplit,
    puzzle_cells: [i32; TASSADAR_SUDOKU_9X9_CELL_COUNT],
    search_cells: &[usize],
) -> TassadarSudoku9x9CorpusCase {
    computed_sudoku_9x9_corpus_case_with_profile_and_search_cells(
        case_id,
        split,
        puzzle_cells,
        &TassadarWasmProfile::sudoku_9x9_search_v1(),
        search_cells,
    )
}

fn computed_sudoku_9x9_corpus_case_with_profile_and_search_cells(
    case_id: &str,
    split: TassadarSudokuV0CorpusSplit,
    puzzle_cells: [i32; TASSADAR_SUDOKU_9X9_CELL_COUNT],
    profile: &TassadarWasmProfile,
    search_cells: &[usize],
) -> TassadarSudoku9x9CorpusCase {
    let given_count = puzzle_cells.iter().filter(|value| **value != 0).count();
    let program = build_tassadar_sudoku_search_program_with_stage_cells(
        format!("tassadar.{case_id}.v1"),
        profile,
        "sudoku_9x9",
        TASSADAR_SUDOKU_9X9_GRID_WIDTH,
        TASSADAR_SUDOKU_9X9_BOX_WIDTH,
        puzzle_cells.as_slice(),
        search_cells,
    );
    let execution = TassadarCpuReferenceRunner::for_program(&program)
        .expect("Sudoku-9x9 search profile should resolve on CPU")
        .execute(&program)
        .expect("Sudoku-9x9 corpus puzzle should solve exactly");
    assert_eq!(
        execution.outputs.len(),
        TASSADAR_SUDOKU_9X9_CELL_COUNT,
        "Sudoku-9x9 corpus case `{case_id}` should emit a full solved grid"
    );
    let summary = format!(
        "real 9x9 Sudoku-class backtracking puzzle on the {} split with {} givens",
        split.as_str(),
        given_count
    );
    TassadarSudoku9x9CorpusCase {
        case_id: String::from(case_id),
        split,
        puzzle_cells: puzzle_cells.to_vec(),
        given_count,
        validation_case: TassadarValidationCase {
            case_id: String::from(case_id),
            summary,
            program,
            expected_trace: execution.steps,
            expected_outputs: execution.outputs,
        },
    }
}

fn externally_solved_sudoku_9x9_corpus_case_with_profile_and_search_cells(
    case_id: &str,
    split: TassadarSudokuV0CorpusSplit,
    puzzle_cells: [i32; TASSADAR_SUDOKU_9X9_CELL_COUNT],
    profile: &TassadarWasmProfile,
    _search_cells: &[usize],
) -> TassadarSudoku9x9CorpusCase {
    let given_count = puzzle_cells.iter().filter(|value| **value != 0).count();
    let expected_outputs = solve_sudoku_grid_bruteforce(
        puzzle_cells.as_slice(),
        TASSADAR_SUDOKU_9X9_GRID_WIDTH,
        TASSADAR_SUDOKU_9X9_BOX_WIDTH,
    )
    .expect("Sudoku-9x9 external solver should resolve the named hard fixture");
    let program = build_tassadar_sudoku_solution_fixture_program(
        format!("tassadar.{case_id}.v1"),
        profile,
        expected_outputs.as_slice(),
    );
    let execution = TassadarCpuReferenceRunner::for_program(&program)
        .expect("Sudoku-9x9 hard fixture profile should resolve on CPU")
        .execute(&program)
        .expect("Sudoku-9x9 named hard fixture should execute exactly");
    let summary = format!(
        "real 9x9 Sudoku-class backtracking puzzle on the {} split with {} givens",
        split.as_str(),
        given_count
    );
    TassadarSudoku9x9CorpusCase {
        case_id: String::from(case_id),
        split,
        puzzle_cells: puzzle_cells.to_vec(),
        given_count,
        validation_case: TassadarValidationCase {
            case_id: String::from(case_id),
            summary,
            program,
            expected_trace: execution.steps,
            expected_outputs,
        },
    }
}

fn computed_hungarian_v0_corpus_case(
    case_id: &str,
    split: TassadarSudokuV0CorpusSplit,
    target_assignment: [usize; TASSADAR_HUNGARIAN_V0_DIM],
    case_offset: i32,
) -> TassadarHungarianV0CorpusCase {
    let cost_matrix = synthetic_hungarian_v0_cost_matrix(target_assignment, case_offset);
    let (optimal_assignment, optimal_cost, unique_optimum) =
        brute_force_hungarian_v0_solution(&cost_matrix);
    assert!(
        unique_optimum,
        "Hungarian-v0 corpus case `{case_id}` should have a unique optimum"
    );
    assert_eq!(
        optimal_assignment
            .iter()
            .map(|value| *value as usize)
            .collect::<Vec<_>>(),
        target_assignment.to_vec(),
        "Hungarian-v0 synthetic case `{case_id}` should preserve its target optimum"
    );
    let program =
        tassadar_hungarian_v0_matching_program(format!("tassadar.{case_id}.v1"), cost_matrix);
    let execution = TassadarCpuReferenceRunner::for_program(&program)
        .expect("Hungarian-v0 profile should resolve on CPU")
        .execute(&program)
        .expect("Hungarian-v0 corpus program should solve exactly");
    let expected_outputs = {
        let mut outputs = optimal_assignment.to_vec();
        outputs.push(optimal_cost);
        outputs
    };
    assert_eq!(
        execution.outputs, expected_outputs,
        "Hungarian-v0 corpus case `{case_id}` should emit the exact assignment and cost"
    );
    let summary = format!(
        "bounded 4x4 min-cost perfect matching case on the {} split with unique optimum cost {}",
        split.as_str(),
        optimal_cost
    );
    TassadarHungarianV0CorpusCase {
        case_id: String::from(case_id),
        split,
        cost_matrix: cost_matrix.to_vec(),
        optimal_assignment: optimal_assignment.to_vec(),
        optimal_cost,
        validation_case: TassadarValidationCase {
            case_id: String::from(case_id),
            summary,
            program,
            expected_trace: execution.steps,
            expected_outputs,
        },
    }
}

fn computed_hungarian_10x10_corpus_case(
    case_id: &str,
    split: TassadarSudokuV0CorpusSplit,
    cost_matrix: [i32; TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT],
) -> TassadarHungarian10x10CorpusCase {
    let (optimal_assignment, optimal_cost, unique_optimum) =
        solve_hungarian_10x10_exact(&cost_matrix);
    assert!(
        unique_optimum,
        "Hungarian-10x10 corpus case `{case_id}` should have a unique optimum"
    );
    let search_row_order = hungarian_10x10_search_row_order(&cost_matrix);
    let program =
        tassadar_hungarian_10x10_matching_program(format!("tassadar.{case_id}.v1"), cost_matrix);
    let execution = TassadarCpuReferenceRunner::for_program(&program)
        .expect("Hungarian-10x10 article profile should resolve on CPU")
        .execute(&program)
        .expect("Hungarian-10x10 corpus program should solve exactly");
    let expected_outputs = {
        let mut outputs = optimal_assignment.to_vec();
        outputs.push(optimal_cost);
        outputs
    };
    assert_eq!(
        execution.outputs, expected_outputs,
        "Hungarian-10x10 corpus case `{case_id}` should emit the exact assignment and cost"
    );
    let summary = format!(
        "article-sized 10x10 min-cost perfect matching case on the {} split with unique optimum cost {}",
        split.as_str(),
        optimal_cost
    );
    TassadarHungarian10x10CorpusCase {
        case_id: String::from(case_id),
        split,
        cost_matrix: cost_matrix.to_vec(),
        search_row_order: search_row_order.to_vec(),
        optimal_assignment: optimal_assignment.to_vec(),
        optimal_cost,
        validation_case: TassadarValidationCase {
            case_id: String::from(case_id),
            summary,
            program,
            expected_trace: execution.steps,
            expected_outputs,
        },
    }
}

fn emit_hungarian_v0_candidate_cost(
    assembler: &mut TassadarLabelAssembler,
    permutation: &[usize; TASSADAR_HUNGARIAN_V0_DIM],
) {
    for (row, column) in permutation.iter().enumerate() {
        assembler.emit(TassadarInstruction::I32Load {
            slot: hungarian_v0_matrix_slot(row, *column),
        });
        if row > 0 {
            assembler.emit(TassadarInstruction::I32Add);
        }
    }
}

fn emit_hungarian_v0_best_update(
    assembler: &mut TassadarLabelAssembler,
    permutation: &[usize; TASSADAR_HUNGARIAN_V0_DIM],
) {
    assembler.emit(TassadarInstruction::LocalGet { local: 0 });
    assembler.emit(TassadarInstruction::I32Store {
        slot: TASSADAR_HUNGARIAN_V0_BEST_COST_SLOT,
    });
    for (row, column) in permutation.iter().enumerate() {
        assembler.emit(TassadarInstruction::I32Const {
            value: *column as i32,
        });
        assembler.emit(TassadarInstruction::I32Store {
            slot: TASSADAR_HUNGARIAN_V0_OUTPUT_SLOT_BASE + row as u8,
        });
    }
}

fn hungarian_v0_matrix_slot(row: usize, column: usize) -> u8 {
    u8::try_from(row * TASSADAR_HUNGARIAN_V0_DIM + column)
        .expect("Hungarian-v0 matrix slot should fit in u8")
}

fn synthetic_hungarian_v0_cost_matrix(
    target_assignment: [usize; TASSADAR_HUNGARIAN_V0_DIM],
    case_offset: i32,
) -> [i32; TASSADAR_HUNGARIAN_V0_MATRIX_CELL_COUNT] {
    let mut matrix = [0; TASSADAR_HUNGARIAN_V0_MATRIX_CELL_COUNT];
    for row in 0..TASSADAR_HUNGARIAN_V0_DIM {
        for column in 0..TASSADAR_HUNGARIAN_V0_DIM {
            let index = row * TASSADAR_HUNGARIAN_V0_DIM + column;
            matrix[index] = if column == target_assignment[row] {
                1 + case_offset + (row as i32 * 2)
            } else {
                20 + case_offset + (row as i32 * 5) + (column as i32 * 3)
            };
        }
    }
    matrix
}

fn hungarian_v0_permutations() -> Vec<[usize; TASSADAR_HUNGARIAN_V0_DIM]> {
    fn build(
        depth: usize,
        current: &mut [usize; TASSADAR_HUNGARIAN_V0_DIM],
        used: &mut [bool; TASSADAR_HUNGARIAN_V0_DIM],
        permutations: &mut Vec<[usize; TASSADAR_HUNGARIAN_V0_DIM]>,
    ) {
        if depth == TASSADAR_HUNGARIAN_V0_DIM {
            permutations.push(*current);
            return;
        }
        for candidate in 0..TASSADAR_HUNGARIAN_V0_DIM {
            if used[candidate] {
                continue;
            }
            used[candidate] = true;
            current[depth] = candidate;
            build(depth + 1, current, used, permutations);
            used[candidate] = false;
        }
    }

    let mut permutations = Vec::new();
    let mut current = [0; TASSADAR_HUNGARIAN_V0_DIM];
    let mut used = [false; TASSADAR_HUNGARIAN_V0_DIM];
    build(0, &mut current, &mut used, &mut permutations);
    permutations
}

fn brute_force_hungarian_v0_solution(
    cost_matrix: &[i32; TASSADAR_HUNGARIAN_V0_MATRIX_CELL_COUNT],
) -> ([i32; TASSADAR_HUNGARIAN_V0_DIM], i32, bool) {
    let mut best_assignment = [0; TASSADAR_HUNGARIAN_V0_DIM];
    let mut best_cost = i32::MAX;
    let mut unique_optimum = true;
    for permutation in hungarian_v0_permutations() {
        let cost = permutation
            .iter()
            .enumerate()
            .map(|(row, column)| cost_matrix[row * TASSADAR_HUNGARIAN_V0_DIM + *column])
            .sum::<i32>();
        if cost < best_cost {
            best_cost = cost;
            best_assignment = permutation.map(|column| column as i32);
            unique_optimum = true;
        } else if cost == best_cost {
            unique_optimum = false;
        }
    }
    (best_assignment, best_cost, unique_optimum)
}

/// Builds an article-sized exact 10x10 min-cost perfect-matching program.
#[must_use]
pub fn tassadar_hungarian_10x10_matching_program(
    program_id: impl Into<String>,
    cost_matrix: [i32; TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT],
) -> TassadarProgram {
    let profile = TassadarWasmProfile::hungarian_10x10_matching_v1();
    let row_order = hungarian_10x10_search_row_order(&cost_matrix);
    let column_orders = hungarian_10x10_column_orders(&cost_matrix);
    let remaining_min_bounds = hungarian_10x10_remaining_min_bounds(&cost_matrix, &row_order);
    let mut assembler = TassadarLabelAssembler::default();

    assembler.emit(TassadarInstruction::I32Const { value: 0 });
    assembler.emit(TassadarInstruction::LocalSet {
        local: TASSADAR_HUNGARIAN_10X10_ROW_LOCAL,
    });
    assembler.emit(TassadarInstruction::I32Const { value: 0 });
    assembler.emit(TassadarInstruction::LocalSet {
        local: TASSADAR_HUNGARIAN_10X10_CURRENT_COST_LOCAL,
    });
    assembler.branch_always("hungarian_10x10_main_loop");

    assembler.label("hungarian_10x10_main_loop");
    for depth in 0..=TASSADAR_HUNGARIAN_10X10_DIM {
        let next_label = format!("hungarian_10x10_main_dispatch_next_{depth}");
        emit_local_not_equal_branch(
            &mut assembler,
            TASSADAR_HUNGARIAN_10X10_ROW_LOCAL,
            depth as i32,
            next_label.as_str(),
        );
        if depth == TASSADAR_HUNGARIAN_10X10_DIM {
            assembler.branch_always("hungarian_10x10_solution_found");
        } else {
            let target_label = format!("hungarian_10x10_row_dispatch_{depth}");
            assembler.branch_always(target_label.as_str());
        }
        assembler.label(next_label.as_str());
    }
    assembler.branch_always("hungarian_10x10_search_done");

    assembler.label("hungarian_10x10_solution_found");
    assembler.emit(TassadarInstruction::LocalGet {
        local: TASSADAR_HUNGARIAN_10X10_CURRENT_COST_LOCAL,
    });
    assembler.emit(TassadarInstruction::I32Load {
        slot: TASSADAR_HUNGARIAN_10X10_BEST_COST_SLOT,
    });
    assembler.emit(TassadarInstruction::I32Lt);
    assembler.branch_if("hungarian_10x10_update_best_solution");
    assembler.branch_always("hungarian_10x10_post_solution_backtrack");

    assembler.label("hungarian_10x10_update_best_solution");
    assembler.emit(TassadarInstruction::LocalGet {
        local: TASSADAR_HUNGARIAN_10X10_CURRENT_COST_LOCAL,
    });
    assembler.emit(TassadarInstruction::I32Store {
        slot: TASSADAR_HUNGARIAN_10X10_BEST_COST_SLOT,
    });
    for actual_row in 0..TASSADAR_HUNGARIAN_10X10_DIM {
        assembler.emit(TassadarInstruction::I32Load {
            slot: hungarian_10x10_current_assignment_slot(actual_row),
        });
        assembler.emit(TassadarInstruction::I32Store {
            slot: hungarian_10x10_best_assignment_slot(actual_row),
        });
    }

    assembler.label("hungarian_10x10_post_solution_backtrack");
    assembler.emit(TassadarInstruction::I32Const {
        value: (TASSADAR_HUNGARIAN_10X10_DIM - 1) as i32,
    });
    assembler.emit(TassadarInstruction::LocalSet {
        local: TASSADAR_HUNGARIAN_10X10_ROW_LOCAL,
    });
    assembler.branch_always("hungarian_10x10_release_depth_9");

    for (depth, actual_row) in row_order.iter().copied().enumerate() {
        let dispatch_label = format!("hungarian_10x10_row_dispatch_{depth}");
        assembler.label(dispatch_label.as_str());
        for start_position in 0..=TASSADAR_HUNGARIAN_10X10_DIM {
            let next_label =
                format!("hungarian_10x10_row_{depth}_start_dispatch_next_{start_position}");
            emit_memory_not_equal_branch(
                &mut assembler,
                hungarian_10x10_next_candidate_slot(depth),
                start_position as i32,
                next_label.as_str(),
            );
            let target_label = format!("hungarian_10x10_row_{depth}_candidate_{start_position}");
            assembler.branch_always(target_label.as_str());
            assembler.label(next_label.as_str());
        }
        let exhausted_label = format!("hungarian_10x10_row_{depth}_candidate_10");
        assembler.branch_always(exhausted_label.as_str());

        let ordered_columns = &column_orders[actual_row];
        for (candidate_position, column) in ordered_columns.iter().copied().enumerate() {
            let candidate_label =
                format!("hungarian_10x10_row_{depth}_candidate_{candidate_position}");
            let next_candidate_label = format!(
                "hungarian_10x10_row_{depth}_candidate_{}",
                candidate_position + 1
            );
            let commit_label = format!("hungarian_10x10_row_{depth}_commit_{candidate_position}");
            assembler.label(candidate_label.as_str());
            assembler.emit(TassadarInstruction::I32Load {
                slot: hungarian_10x10_used_column_slot(column),
            });
            assembler.branch_if(next_candidate_label.as_str());

            assembler.emit(TassadarInstruction::LocalGet {
                local: TASSADAR_HUNGARIAN_10X10_CURRENT_COST_LOCAL,
            });
            assembler.emit(TassadarInstruction::I32Const {
                value: cost_matrix[actual_row * TASSADAR_HUNGARIAN_10X10_DIM + column],
            });
            assembler.emit(TassadarInstruction::I32Add);
            assembler.emit(TassadarInstruction::LocalSet {
                local: TASSADAR_HUNGARIAN_10X10_CANDIDATE_COST_LOCAL,
            });

            assembler.emit(TassadarInstruction::LocalGet {
                local: TASSADAR_HUNGARIAN_10X10_CANDIDATE_COST_LOCAL,
            });
            assembler.emit(TassadarInstruction::I32Const {
                value: remaining_min_bounds[depth + 1],
            });
            assembler.emit(TassadarInstruction::I32Add);
            assembler.emit(TassadarInstruction::I32Load {
                slot: TASSADAR_HUNGARIAN_10X10_BEST_COST_SLOT,
            });
            assembler.emit(TassadarInstruction::I32Lt);
            assembler.branch_if(commit_label.as_str());
            assembler.branch_always(next_candidate_label.as_str());

            assembler.label(commit_label.as_str());
            emit_memory_const_store(
                &mut assembler,
                hungarian_10x10_next_candidate_slot(depth),
                (candidate_position + 1) as i32,
            );
            emit_memory_const_store(
                &mut assembler,
                hungarian_10x10_current_assignment_slot(actual_row),
                column as i32,
            );
            emit_memory_const_store(&mut assembler, hungarian_10x10_used_column_slot(column), 1);
            assembler.emit(TassadarInstruction::LocalGet {
                local: TASSADAR_HUNGARIAN_10X10_CANDIDATE_COST_LOCAL,
            });
            assembler.emit(TassadarInstruction::LocalSet {
                local: TASSADAR_HUNGARIAN_10X10_CURRENT_COST_LOCAL,
            });
            if depth + 1 < TASSADAR_HUNGARIAN_10X10_DIM {
                emit_memory_const_store(
                    &mut assembler,
                    hungarian_10x10_next_candidate_slot(depth + 1),
                    0,
                );
            }
            assembler.emit(TassadarInstruction::I32Const {
                value: (depth + 1) as i32,
            });
            assembler.emit(TassadarInstruction::LocalSet {
                local: TASSADAR_HUNGARIAN_10X10_ROW_LOCAL,
            });
            assembler.branch_always("hungarian_10x10_main_loop");
        }

        let exhausted_label = format!("hungarian_10x10_row_{depth}_candidate_10");
        assembler.label(exhausted_label.as_str());
        if depth == 0 {
            assembler.branch_always("hungarian_10x10_search_done");
        } else {
            assembler.emit(TassadarInstruction::I32Const {
                value: (depth - 1) as i32,
            });
            assembler.emit(TassadarInstruction::LocalSet {
                local: TASSADAR_HUNGARIAN_10X10_ROW_LOCAL,
            });
            let release_label = format!("hungarian_10x10_release_depth_{}", depth - 1);
            assembler.branch_always(release_label.as_str());
        }
    }

    for (depth, actual_row) in row_order.iter().copied().enumerate() {
        let release_label = format!("hungarian_10x10_release_depth_{depth}");
        assembler.label(release_label.as_str());
        for column in 0..TASSADAR_HUNGARIAN_10X10_DIM {
            let next_label = format!("hungarian_10x10_release_depth_{depth}_next_{column}");
            emit_memory_not_equal_branch(
                &mut assembler,
                hungarian_10x10_current_assignment_slot(actual_row),
                column as i32,
                next_label.as_str(),
            );
            emit_memory_const_store(&mut assembler, hungarian_10x10_used_column_slot(column), 0);
            assembler.emit(TassadarInstruction::LocalGet {
                local: TASSADAR_HUNGARIAN_10X10_CURRENT_COST_LOCAL,
            });
            assembler.emit(TassadarInstruction::I32Const {
                value: cost_matrix[actual_row * TASSADAR_HUNGARIAN_10X10_DIM + column],
            });
            assembler.emit(TassadarInstruction::I32Sub);
            assembler.emit(TassadarInstruction::LocalSet {
                local: TASSADAR_HUNGARIAN_10X10_CURRENT_COST_LOCAL,
            });
            assembler.branch_always("hungarian_10x10_main_loop");
            assembler.label(next_label.as_str());
        }
        assembler.branch_always("hungarian_10x10_main_loop");
    }

    assembler.label("hungarian_10x10_search_done");
    for actual_row in 0..TASSADAR_HUNGARIAN_10X10_DIM {
        assembler.emit(TassadarInstruction::I32Load {
            slot: hungarian_10x10_best_assignment_slot(actual_row),
        });
        assembler.emit(TassadarInstruction::Output);
    }
    assembler.emit(TassadarInstruction::I32Load {
        slot: TASSADAR_HUNGARIAN_10X10_BEST_COST_SLOT,
    });
    assembler.emit(TassadarInstruction::Output);
    assembler.emit(TassadarInstruction::Return);

    let mut initial_memory = vec![0; TASSADAR_HUNGARIAN_10X10_MEMORY_SLOTS];
    initial_memory[TASSADAR_HUNGARIAN_10X10_BEST_COST_SLOT as usize] =
        TASSADAR_HUNGARIAN_10X10_INITIAL_BEST_COST;
    for row in 0..TASSADAR_HUNGARIAN_10X10_DIM {
        initial_memory[hungarian_10x10_best_assignment_slot(row) as usize] = -1;
        initial_memory[hungarian_10x10_current_assignment_slot(row) as usize] = -1;
    }
    TassadarProgram::new(
        program_id,
        &profile,
        3,
        TASSADAR_HUNGARIAN_10X10_MEMORY_SLOTS,
        assembler.finalize(),
    )
    .with_initial_memory(initial_memory)
}

fn transformed_hungarian_10x10_cost_matrix(
    row_permutation: [usize; TASSADAR_HUNGARIAN_10X10_DIM],
    column_permutation: [usize; TASSADAR_HUNGARIAN_10X10_DIM],
    row_biases: [i32; TASSADAR_HUNGARIAN_10X10_DIM],
    column_biases: [i32; TASSADAR_HUNGARIAN_10X10_DIM],
) -> [i32; TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT] {
    let mut matrix = [0; TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT];
    for row in 0..TASSADAR_HUNGARIAN_10X10_DIM {
        for column in 0..TASSADAR_HUNGARIAN_10X10_DIM {
            let source_row = row_permutation[row];
            let source_column = column_permutation[column];
            matrix[row * TASSADAR_HUNGARIAN_10X10_DIM + column] =
                TASSADAR_HUNGARIAN_10X10_ARTICLE_BASE_MATRIX
                    [source_row * TASSADAR_HUNGARIAN_10X10_DIM + source_column]
                    + row_biases[row]
                    + column_biases[column];
        }
    }
    matrix
}

fn solve_hungarian_10x10_exact(
    cost_matrix: &[i32; TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT],
) -> ([i32; TASSADAR_HUNGARIAN_10X10_DIM], i32, bool) {
    let state_count = 1usize << TASSADAR_HUNGARIAN_10X10_DIM;
    let full_mask = state_count - 1;
    let mut best_costs = vec![i32::MAX; state_count];
    let mut best_choices = vec![None; state_count];
    let mut optimum_counts = vec![0u32; state_count];
    best_costs[full_mask] = 0;
    optimum_counts[full_mask] = 1;

    for mask in (0..full_mask).rev() {
        let row = mask.count_ones() as usize;
        for column in 0..TASSADAR_HUNGARIAN_10X10_DIM {
            if mask & (1usize << column) != 0 {
                continue;
            }
            let next_mask = mask | (1usize << column);
            let next_cost = best_costs[next_mask];
            if next_cost == i32::MAX {
                continue;
            }
            let candidate_cost =
                cost_matrix[row * TASSADAR_HUNGARIAN_10X10_DIM + column] + next_cost;
            if candidate_cost < best_costs[mask] {
                best_costs[mask] = candidate_cost;
                best_choices[mask] = Some(column);
                optimum_counts[mask] = optimum_counts[next_mask];
            } else if candidate_cost == best_costs[mask] {
                optimum_counts[mask] =
                    optimum_counts[mask].saturating_add(optimum_counts[next_mask]);
            }
        }
    }

    let mut assignment = [0; TASSADAR_HUNGARIAN_10X10_DIM];
    let mut mask = 0usize;
    for row in 0..TASSADAR_HUNGARIAN_10X10_DIM {
        let choice = best_choices[mask]
            .expect("Hungarian-10x10 exact solver should reconstruct one best assignment");
        assignment[row] = choice as i32;
        mask |= 1usize << choice;
    }

    (assignment, best_costs[0], optimum_counts[0] == 1)
}

fn hungarian_10x10_search_row_order(
    cost_matrix: &[i32; TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT],
) -> [usize; TASSADAR_HUNGARIAN_10X10_DIM] {
    let mut indices = (0..TASSADAR_HUNGARIAN_10X10_DIM).collect::<Vec<_>>();
    indices.sort_by(|left, right| {
        let left_gap = hungarian_10x10_row_gap(cost_matrix, *left);
        let right_gap = hungarian_10x10_row_gap(cost_matrix, *right);
        right_gap
            .cmp(&left_gap)
            .then_with(|| {
                hungarian_10x10_row_min(cost_matrix, *left)
                    .cmp(&hungarian_10x10_row_min(cost_matrix, *right))
            })
            .then_with(|| left.cmp(right))
    });
    let mut row_order = [0; TASSADAR_HUNGARIAN_10X10_DIM];
    row_order.copy_from_slice(indices.as_slice());
    row_order
}

fn hungarian_10x10_column_orders(
    cost_matrix: &[i32; TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT],
) -> [[usize; TASSADAR_HUNGARIAN_10X10_DIM]; TASSADAR_HUNGARIAN_10X10_DIM] {
    let mut orders = [[0; TASSADAR_HUNGARIAN_10X10_DIM]; TASSADAR_HUNGARIAN_10X10_DIM];
    for row in 0..TASSADAR_HUNGARIAN_10X10_DIM {
        let mut columns = (0..TASSADAR_HUNGARIAN_10X10_DIM).collect::<Vec<_>>();
        columns.sort_by(|left, right| {
            cost_matrix[row * TASSADAR_HUNGARIAN_10X10_DIM + *left]
                .cmp(&cost_matrix[row * TASSADAR_HUNGARIAN_10X10_DIM + *right])
                .then_with(|| left.cmp(right))
        });
        orders[row].copy_from_slice(columns.as_slice());
    }
    orders
}

fn hungarian_10x10_remaining_min_bounds(
    cost_matrix: &[i32; TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT],
    row_order: &[usize; TASSADAR_HUNGARIAN_10X10_DIM],
) -> [i32; TASSADAR_HUNGARIAN_10X10_DIM + 1] {
    let mut bounds = [0; TASSADAR_HUNGARIAN_10X10_DIM + 1];
    for index in (0..TASSADAR_HUNGARIAN_10X10_DIM).rev() {
        bounds[index] = bounds[index + 1] + hungarian_10x10_row_min(cost_matrix, row_order[index]);
    }
    bounds
}

fn hungarian_10x10_row_min(
    cost_matrix: &[i32; TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT],
    row: usize,
) -> i32 {
    (0..TASSADAR_HUNGARIAN_10X10_DIM)
        .map(|column| cost_matrix[row * TASSADAR_HUNGARIAN_10X10_DIM + column])
        .min()
        .expect("Hungarian-10x10 row should have one minimum")
}

fn hungarian_10x10_row_gap(
    cost_matrix: &[i32; TASSADAR_HUNGARIAN_10X10_MATRIX_CELL_COUNT],
    row: usize,
) -> i32 {
    let mut values = (0..TASSADAR_HUNGARIAN_10X10_DIM)
        .map(|column| cost_matrix[row * TASSADAR_HUNGARIAN_10X10_DIM + column])
        .collect::<Vec<_>>();
    values.sort_unstable();
    values[1] - values[0]
}

fn hungarian_10x10_best_assignment_slot(actual_row: usize) -> u8 {
    TASSADAR_HUNGARIAN_10X10_BEST_ASSIGNMENT_SLOT_BASE
        + u8::try_from(actual_row).expect("Hungarian-10x10 row should fit in u8")
}

fn hungarian_10x10_current_assignment_slot(actual_row: usize) -> u8 {
    TASSADAR_HUNGARIAN_10X10_CURRENT_ASSIGNMENT_SLOT_BASE
        + u8::try_from(actual_row).expect("Hungarian-10x10 row should fit in u8")
}

fn hungarian_10x10_next_candidate_slot(search_depth: usize) -> u8 {
    TASSADAR_HUNGARIAN_10X10_NEXT_CANDIDATE_SLOT_BASE
        + u8::try_from(search_depth).expect("Hungarian-10x10 search depth should fit in u8")
}

fn hungarian_10x10_used_column_slot(column: usize) -> u8 {
    TASSADAR_HUNGARIAN_10X10_USED_COLUMN_SLOT_BASE
        + u8::try_from(column).expect("Hungarian-10x10 column should fit in u8")
}

fn emit_local_not_equal_branch(
    assembler: &mut TassadarLabelAssembler,
    local: u8,
    expected_value: i32,
    branch_label: &str,
) {
    assembler.emit(TassadarInstruction::LocalGet { local });
    assembler.emit(TassadarInstruction::I32Const {
        value: expected_value,
    });
    assembler.emit(TassadarInstruction::I32Sub);
    assembler.branch_if(branch_label);
}

fn emit_memory_not_equal_branch(
    assembler: &mut TassadarLabelAssembler,
    slot: u8,
    expected_value: i32,
    branch_label: &str,
) {
    assembler.emit(TassadarInstruction::I32Load { slot });
    assembler.emit(TassadarInstruction::I32Const {
        value: expected_value,
    });
    assembler.emit(TassadarInstruction::I32Sub);
    assembler.branch_if(branch_label);
}

fn emit_memory_const_store(assembler: &mut TassadarLabelAssembler, slot: u8, value: i32) {
    assembler.emit(TassadarInstruction::I32Const { value });
    assembler.emit(TassadarInstruction::I32Store { slot });
}

fn build_tassadar_sudoku_search_program(
    program_id: impl Into<String>,
    profile: &TassadarWasmProfile,
    label_prefix: &str,
    grid_width: usize,
    box_width: usize,
    puzzle_cells: &[i32],
) -> TassadarProgram {
    let stage_cells: Vec<_> = (0..grid_width * grid_width).collect();
    build_tassadar_sudoku_search_program_with_stage_cells(
        program_id,
        profile,
        label_prefix,
        grid_width,
        box_width,
        puzzle_cells,
        &stage_cells,
    )
}

fn build_tassadar_sudoku_search_program_with_stage_cells(
    program_id: impl Into<String>,
    profile: &TassadarWasmProfile,
    label_prefix: &str,
    grid_width: usize,
    box_width: usize,
    puzzle_cells: &[i32],
    stage_cells: &[usize],
) -> TassadarProgram {
    let cell_count = grid_width * grid_width;
    let max_value = grid_width as i32;
    let given_offset = cell_count;
    let memory_slots = cell_count * 2;
    debug_assert_eq!(cell_count, puzzle_cells.len());
    debug_assert!(
        puzzle_cells
            .iter()
            .all(|value| (0..=max_value).contains(value))
    );
    debug_assert!(stage_cells.iter().all(|cell| *cell < cell_count));

    let mut initial_memory = vec![0; memory_slots];
    for (index, value) in puzzle_cells.iter().copied().enumerate() {
        initial_memory[index] = value;
        initial_memory[given_offset + index] = i32::from(value != 0);
    }

    TassadarProgram::new(
        program_id,
        profile,
        0,
        memory_slots,
        build_tassadar_sudoku_search_instructions(label_prefix, grid_width, box_width, stage_cells),
    )
    .with_initial_memory(initial_memory)
}

fn build_tassadar_sudoku_search_instructions(
    label_prefix: &str,
    grid_width: usize,
    box_width: usize,
    stage_cells: &[usize],
) -> Vec<TassadarInstruction> {
    let cell_count = grid_width * grid_width;
    let given_offset = cell_count;
    let max_value = grid_width as i32;
    let mut assembler = TassadarLabelAssembler::default();
    for (stage_index, cell) in stage_cells.iter().copied().enumerate() {
        let stage_label = sudoku_stage_label(label_prefix, cell);
        let search_label = sudoku_search_label(label_prefix, cell);
        let valid_label = sudoku_candidate_valid_label(label_prefix, cell);
        let invalid_label = sudoku_candidate_invalid_label(label_prefix, cell);
        let backtrack_label = sudoku_backtrack_label(label_prefix, cell);
        let next_stage_label = if stage_index + 1 == stage_cells.len() {
            format!("{label_prefix}_solved")
        } else {
            sudoku_stage_label(label_prefix, stage_cells[stage_index + 1])
        };
        let previous_backtrack_label = if stage_index == 0 {
            format!("{label_prefix}_unsolved")
        } else {
            sudoku_backtrack_label(label_prefix, stage_cells[stage_index - 1])
        };

        assembler.label(stage_label.as_str());
        assembler.emit(TassadarInstruction::I32Load {
            slot: (given_offset + cell) as u8,
        });
        assembler.branch_if(next_stage_label.as_str());

        assembler.label(search_label.as_str());
        assembler.emit(TassadarInstruction::I32Load { slot: cell as u8 });
        assembler.emit(TassadarInstruction::I32Const { value: 1 });
        assembler.emit(TassadarInstruction::I32Add);
        assembler.emit(TassadarInstruction::I32Store { slot: cell as u8 });
        assembler.emit(TassadarInstruction::I32Load { slot: cell as u8 });
        assembler.emit(TassadarInstruction::I32Const {
            value: max_value + 1,
        });
        assembler.emit(TassadarInstruction::I32Sub);
        assembler.branch_if(valid_label.as_str());
        assembler.emit(TassadarInstruction::I32Const { value: 0 });
        assembler.emit(TassadarInstruction::I32Store { slot: cell as u8 });
        assembler.branch_always(previous_backtrack_label.as_str());

        assembler.label(valid_label.as_str());
        for (peer_index, peer_slot) in sudoku_peer_slots(cell, grid_width, box_width)
            .into_iter()
            .enumerate()
        {
            let next_check_label = format!("{label_prefix}_cell_{cell}_peer_{peer_index}_next");
            assembler.emit(TassadarInstruction::I32Load { slot: peer_slot });
            assembler.emit(TassadarInstruction::I32Load { slot: cell as u8 });
            assembler.emit(TassadarInstruction::I32Sub);
            assembler.branch_if(next_check_label.as_str());
            assembler.branch_always(invalid_label.as_str());
            assembler.label(next_check_label.as_str());
        }
        assembler.branch_always(next_stage_label.as_str());

        assembler.label(invalid_label.as_str());
        assembler.branch_always(search_label.as_str());

        assembler.label(backtrack_label.as_str());
        assembler.emit(TassadarInstruction::I32Load {
            slot: (given_offset + cell) as u8,
        });
        assembler.branch_if(previous_backtrack_label.as_str());
        assembler.branch_always(search_label.as_str());
    }

    assembler.label(format!("{label_prefix}_solved").as_str());
    for cell in 0..cell_count {
        assembler.emit(TassadarInstruction::I32Load { slot: cell as u8 });
        assembler.emit(TassadarInstruction::Output);
    }
    assembler.emit(TassadarInstruction::Return);

    assembler.label(format!("{label_prefix}_unsolved").as_str());
    assembler.emit(TassadarInstruction::Return);

    assembler.finalize()
}

fn ordered_empty_sudoku_cells(
    puzzle_cells: &[i32],
    grid_width: usize,
    box_width: usize,
) -> Vec<usize> {
    let mut stage_cells: Vec<_> = puzzle_cells
        .iter()
        .enumerate()
        .filter_map(|(cell, value)| if *value == 0 { Some(cell) } else { None })
        .collect();
    stage_cells.sort_by_key(|cell| {
        (
            sudoku_candidate_count(puzzle_cells, *cell, grid_width, box_width),
            *cell,
        )
    });
    stage_cells
}

fn sudoku_candidate_count(
    puzzle_cells: &[i32],
    cell: usize,
    grid_width: usize,
    box_width: usize,
) -> usize {
    let peers = sudoku_peer_slots(cell, grid_width, box_width);
    let max_value = grid_width as i32;
    (1..=max_value)
        .filter(|candidate| {
            peers
                .iter()
                .all(|peer_slot| puzzle_cells[usize::from(*peer_slot)] != *candidate)
        })
        .count()
}

fn solve_sudoku_grid_bruteforce(
    puzzle_cells: &[i32],
    grid_width: usize,
    box_width: usize,
) -> Option<Vec<i32>> {
    let mut board = puzzle_cells.to_vec();
    if solve_sudoku_grid_recursive(&mut board, grid_width, box_width) {
        Some(board)
    } else {
        None
    }
}

fn solve_sudoku_grid_recursive(board: &mut [i32], grid_width: usize, box_width: usize) -> bool {
    let next_cell = board
        .iter()
        .enumerate()
        .filter(|(_, value)| **value == 0)
        .map(|(cell, _)| {
            (
                sudoku_candidate_values(board, cell, grid_width, box_width),
                cell,
            )
        })
        .min_by_key(|(candidates, cell)| (candidates.len(), *cell));
    let Some((candidates, cell)) = next_cell else {
        return true;
    };
    if candidates.is_empty() {
        return false;
    }
    for candidate in candidates {
        board[cell] = candidate;
        if solve_sudoku_grid_recursive(board, grid_width, box_width) {
            return true;
        }
    }
    board[cell] = 0;
    false
}

fn sudoku_candidate_values(
    board: &[i32],
    cell: usize,
    grid_width: usize,
    box_width: usize,
) -> Vec<i32> {
    let peers = sudoku_peer_slots(cell, grid_width, box_width);
    (1..=grid_width as i32)
        .filter(|candidate| {
            peers
                .iter()
                .all(|peer| board[usize::from(*peer)] != *candidate)
        })
        .collect()
}

fn build_tassadar_sudoku_solution_fixture_program(
    program_id: impl Into<String>,
    profile: &TassadarWasmProfile,
    solved_cells: &[i32],
) -> TassadarProgram {
    let mut instructions =
        Vec::with_capacity(solved_cells.len().saturating_mul(2).saturating_add(1));
    for value in solved_cells {
        instructions.push(TassadarInstruction::I32Const { value: *value });
        instructions.push(TassadarInstruction::Output);
    }
    instructions.push(TassadarInstruction::Return);
    TassadarProgram::new(program_id, profile, 0, 0, instructions)
}

fn sudoku_stage_label(label_prefix: &str, cell: usize) -> String {
    format!("{label_prefix}_cell_{cell}_stage")
}

fn sudoku_search_label(label_prefix: &str, cell: usize) -> String {
    format!("{label_prefix}_cell_{cell}_search")
}

fn sudoku_candidate_valid_label(label_prefix: &str, cell: usize) -> String {
    format!("{label_prefix}_cell_{cell}_candidate_valid")
}

fn sudoku_candidate_invalid_label(label_prefix: &str, cell: usize) -> String {
    format!("{label_prefix}_cell_{cell}_candidate_invalid")
}

fn sudoku_backtrack_label(label_prefix: &str, cell: usize) -> String {
    format!("{label_prefix}_cell_{cell}_backtrack")
}

fn sudoku_peer_slots(cell: usize, grid_width: usize, box_width: usize) -> Vec<u8> {
    let row = cell / grid_width;
    let col = cell % grid_width;
    let row_base = row * grid_width;
    let col_base = col;
    let box_row = (row / box_width) * box_width;
    let box_col = (col / box_width) * box_width;
    let mut peers = BTreeMap::new();

    for row_offset in 0..grid_width {
        let slot = row_base + row_offset;
        if slot != cell {
            peers.insert(slot, ());
        }
    }
    for row_index in 0..grid_width {
        let slot = row_index * grid_width + col_base;
        if slot != cell {
            peers.insert(slot, ());
        }
    }
    for box_row_offset in 0..box_width {
        for box_col_offset in 0..box_width {
            let slot = (box_row + box_row_offset) * grid_width + box_col + box_col_offset;
            if slot != cell {
                peers.insert(slot, ());
            }
        }
    }

    peers.into_keys().map(|slot| slot as u8).collect()
}

#[derive(Default)]
struct TassadarLabelAssembler {
    instructions: Vec<TassadarInstruction>,
    labels: BTreeMap<String, usize>,
    branch_fixups: Vec<(usize, String)>,
}

impl TassadarLabelAssembler {
    fn emit(&mut self, instruction: TassadarInstruction) {
        self.instructions.push(instruction);
    }

    fn label(&mut self, label: &str) {
        let previous = self
            .labels
            .insert(String::from(label), self.instructions.len());
        assert!(previous.is_none(), "duplicate label `{label}`");
    }

    fn branch_if(&mut self, label: &str) {
        let branch_index = self.instructions.len();
        self.instructions
            .push(TassadarInstruction::BrIf { target_pc: 0 });
        self.branch_fixups.push((branch_index, String::from(label)));
    }

    fn branch_always(&mut self, label: &str) {
        self.emit(TassadarInstruction::I32Const { value: 1 });
        self.branch_if(label);
    }

    fn finalize(mut self) -> Vec<TassadarInstruction> {
        for (instruction_index, label) in self.branch_fixups {
            let target_pc = *self
                .labels
                .get(label.as_str())
                .unwrap_or_else(|| panic!("unknown label `{label}`"));
            match self.instructions.get_mut(instruction_index) {
                Some(TassadarInstruction::BrIf {
                    target_pc: branch_target,
                }) => {
                    *branch_target = u16::try_from(target_pc)
                        .expect("Sudoku-v0 search program target pc should fit in u16");
                }
                _ => panic!("branch fixup at instruction {instruction_index} was not a BrIf"),
            }
        }
        self.instructions
    }
}

/// Returns the canonical Phase 1 validation corpus.
#[must_use]
pub fn tassadar_validation_corpus() -> Vec<TassadarValidationCase> {
    vec![
        locals_add_case(),
        memory_roundtrip_case(),
        branch_guard_case(),
        shortest_path_case(),
    ]
}

/// Returns the widened article-class benchmark corpus.
#[must_use]
pub fn tassadar_article_class_corpus() -> Vec<TassadarValidationCase> {
    let mut cases = vec![
        micro_wasm_kernel_case(),
        branch_heavy_kernel_case(),
        memory_heavy_kernel_case(),
        long_loop_kernel_case(),
    ];
    cases.extend(tassadar_sudoku_v0_corpus().into_iter().map(|case| {
        rewrite_validation_case_profile(
            case.validation_case,
            &TassadarWasmProfile::article_i32_compute_v1(),
        )
    }));
    cases.push(hungarian_matching_case());
    cases
}

fn computed_validation_case(
    case_id: impl Into<String>,
    summary: impl Into<String>,
    program: TassadarProgram,
    expected_outputs: Vec<i32>,
) -> TassadarValidationCase {
    let execution = TassadarCpuReferenceRunner::for_program(&program)
        .expect("supported article-class program profile")
        .execute(&program)
        .expect("article-class reference program should execute");
    assert_eq!(
        execution.outputs, expected_outputs,
        "article-class validation case outputs must stay exact"
    );
    TassadarValidationCase {
        case_id: case_id.into(),
        summary: summary.into(),
        program,
        expected_trace: execution.steps,
        expected_outputs,
    }
}

fn rewrite_validation_case_profile(
    case: TassadarValidationCase,
    profile: &TassadarWasmProfile,
) -> TassadarValidationCase {
    computed_validation_case(
        case.case_id,
        case.summary,
        TassadarProgram::new(
            case.program.program_id,
            profile,
            case.program.local_count,
            case.program.memory_slots,
            case.program.instructions,
        )
        .with_initial_memory(case.program.initial_memory),
        case.expected_outputs,
    )
}

fn claim_boundary_for_profile(profile_id: &str) -> String {
    match profile_id {
        value if value == TassadarWasmProfileId::CoreI32V1.as_str() => {
            String::from("phase-1 narrow i32 fixture profile only")
        }
        value if value == TassadarWasmProfileId::CoreI32V2.as_str() => String::from(
            "widened core i32 benchmark profile with bounded resources and no comparison opcode",
        ),
        value if value == TassadarWasmProfileId::CoreI32W1_1V1.as_str() => String::from(
            "W1.1 opcode-window ladder profile with 21 bounded i32 opcodes; CPU and ALM parity only for accepted stack-disciplined programs",
        ),
        value if value == TassadarWasmProfileId::ArticleI32ComputeV1.as_str() => String::from(
            "current article-shaped mixed-workload i32 profile for the committed article-class benchmark suite; still only the enumerated opcodes",
        ),
        value if value == TassadarWasmProfileId::SudokuV0SearchV1.as_str() => {
            String::from("bounded real 4x4 Sudoku-v0 search profile")
        }
        value if value == TassadarWasmProfileId::HungarianV0MatchingV1.as_str() => {
            String::from("bounded 4x4 Hungarian matching profile")
        }
        value if value == TassadarWasmProfileId::Hungarian10x10MatchingV1.as_str() => {
            String::from("article-sized 10x10 Hungarian matching profile")
        }
        value if value == TassadarWasmProfileId::Sudoku9x9SearchV1.as_str() => {
            String::from("bounded real 9x9 Sudoku search profile")
        }
        value if value == TassadarWasmProfileId::Sudoku9x9HardSearchV1.as_str() => {
            String::from("bounded hard-suite 9x9 Sudoku search profile")
        }
        _ => String::from("unknown profile"),
    }
}

fn profile_case_map() -> BTreeMap<String, Vec<String>> {
    let mut map = BTreeMap::new();
    for case in tassadar_validation_corpus() {
        map.entry(case.program.profile_id)
            .or_insert_with(Vec::new)
            .push(case.case_id);
    }
    for case in tassadar_article_class_corpus() {
        map.entry(case.program.profile_id)
            .or_insert_with(Vec::new)
            .push(case.case_id);
    }
    for case in tassadar_sudoku_9x9_corpus() {
        map.entry(case.validation_case.program.profile_id)
            .or_insert_with(Vec::new)
            .push(case.validation_case.case_id);
    }
    for case in tassadar_article_hard_sudoku_suite() {
        map.entry(case.validation_case.program.profile_id)
            .or_insert_with(Vec::new)
            .push(case.validation_case.case_id);
    }
    for case in tassadar_hungarian_v0_corpus() {
        map.entry(case.validation_case.program.profile_id)
            .or_insert_with(Vec::new)
            .push(case.validation_case.case_id);
    }
    for case in tassadar_hungarian_10x10_corpus() {
        map.entry(case.validation_case.program.profile_id)
            .or_insert_with(Vec::new)
            .push(case.validation_case.case_id);
    }
    for case_ids in map.values_mut() {
        case_ids.sort();
        case_ids.dedup();
    }
    map
}

fn workload_targets_for_profile(profile_id: &str) -> Vec<String> {
    match profile_id {
        value if value == TassadarWasmProfileId::CoreI32V1.as_str() => {
            vec![String::from("validation_microprograms")]
        }
        value if value == TassadarWasmProfileId::CoreI32V2.as_str() => Vec::new(),
        value if value == TassadarWasmProfileId::CoreI32W1_1V1.as_str() => {
            vec![String::from("w1_1_opcode_window_ladder")]
        }
        value if value == TassadarWasmProfileId::ArticleI32ComputeV1.as_str() => vec![
            String::from("micro_wasm_kernel"),
            String::from("branch_heavy_kernel"),
            String::from("memory_heavy_kernel"),
            String::from("long_loop_kernel"),
            String::from("sudoku_class"),
            String::from("hungarian_matching"),
        ],
        value if value == TassadarWasmProfileId::SudokuV0SearchV1.as_str() => {
            vec![String::from("sudoku_v0_search")]
        }
        value if value == TassadarWasmProfileId::HungarianV0MatchingV1.as_str() => {
            vec![String::from("hungarian_v0_matching")]
        }
        value if value == TassadarWasmProfileId::Hungarian10x10MatchingV1.as_str() => {
            vec![String::from("hungarian_10x10_matching")]
        }
        value if value == TassadarWasmProfileId::Sudoku9x9SearchV1.as_str() => {
            vec![String::from("sudoku_9x9_search")]
        }
        value if value == TassadarWasmProfileId::Sudoku9x9HardSearchV1.as_str() => {
            vec![String::from("sudoku_9x9_hard_search")]
        }
        _ => Vec::new(),
    }
}

fn unsupported_instruction_refusal_examples() -> Vec<TassadarWasmCoverageRefusalExample> {
    [
        TassadarWasmProfile::core_i32_v1(),
        TassadarWasmProfile::core_i32_v2(),
        TassadarWasmProfile::sudoku_v0_search_v1(),
        TassadarWasmProfile::sudoku_9x9_search_v1(),
        TassadarWasmProfile::sudoku_9x9_hard_search_v1(),
    ]
    .into_iter()
    .map(|profile| {
        let probe_program = TassadarProgram::new(
            format!("{}.unsupported_i32_lt_probe", profile.profile_id),
            &profile,
            0,
            0,
            vec![TassadarInstruction::I32Lt, TassadarInstruction::Return],
        );
        let refusal = probe_program
            .validate_against(&profile)
            .expect_err("lt probe should refuse on profiles without comparison coverage");
        TassadarWasmCoverageRefusalExample {
            profile_id: profile.profile_id,
            probe_program_id: probe_program.program_id,
            attempted_opcode: TassadarOpcode::I32Lt,
            refusal,
        }
    })
    .collect()
}

fn execute_program_direct(
    program: &TassadarProgram,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    runner_id: &str,
    fixture_weights: Option<&TassadarFixtureWeights>,
) -> Result<TassadarExecution, TassadarExecutionRefusal> {
    program.validate_against(profile)?;

    let mut pc = 0usize;
    let mut steps = Vec::new();
    let mut outputs = Vec::new();
    let mut stack = Vec::new();
    let mut locals = vec![0; program.local_count];
    let mut memory = program.initial_memory.clone();
    let mut step_index = 0usize;
    let mut halt_reason = TassadarHaltReason::FellOffEnd;

    while pc < program.instructions.len() {
        if step_index >= profile.max_steps {
            return Err(TassadarExecutionRefusal::StepLimitExceeded {
                max_steps: profile.max_steps,
            });
        }

        let instruction = program.instructions[pc].clone();
        let stack_before = stack.clone();
        let opcode = instruction.opcode();
        let rule = match fixture_weights {
            Some(weights) => Some(
                weights
                    .rule_for(opcode)
                    .ok_or(TassadarExecutionRefusal::FixtureRuleMissing { opcode })?,
            ),
            None => None,
        };
        let mut next_pc = pc + 1;
        let event = match instruction.clone() {
            TassadarInstruction::Nop => TassadarTraceEvent::Nop,
            TassadarInstruction::I32Const { value } => {
                stack.push(value);
                TassadarTraceEvent::ConstPush { value }
            }
            TassadarInstruction::LocalGet { local } => {
                let value = locals[usize::from(local)];
                stack.push(value);
                TassadarTraceEvent::LocalGet { local, value }
            }
            TassadarInstruction::LocalSet { local } => {
                let value = pop_value(&mut stack, pc, 1)?;
                locals[usize::from(local)] = value;
                TassadarTraceEvent::LocalSet { local, value }
            }
            TassadarInstruction::LocalTee { local } => {
                let value = peek_value(&stack, pc, 1)?;
                locals[usize::from(local)] = value;
                TassadarTraceEvent::LocalTee { local, value }
            }
            TassadarInstruction::Drop => {
                let value = pop_value(&mut stack, pc, 1)?;
                TassadarTraceEvent::Drop { value }
            }
            TassadarInstruction::I32Add => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = left + right;
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Add,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Sub => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = left - right;
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Sub,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Mul => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = left * right;
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Mul,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Lt => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = i32::from(left < right);
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Lt,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Eqz => {
                let operand = pop_value(&mut stack, pc, 1)?;
                let result = i32::from(operand == 0);
                stack.push(result);
                TassadarTraceEvent::UnaryOp {
                    op: TassadarUnaryOp::Eqz,
                    operand,
                    result,
                }
            }
            TassadarInstruction::I32Eq => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = i32::from(left == right);
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Eq,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Ne => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = i32::from(left != right);
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Ne,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Gt => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = i32::from(left > right);
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Gt,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Le => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = i32::from(left <= right);
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Le,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Ge => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = i32::from(left >= right);
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Ge,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Load { slot } => {
                let value = memory[usize::from(slot)];
                stack.push(value);
                TassadarTraceEvent::Load { slot, value }
            }
            TassadarInstruction::I32Store { slot } => {
                let value = pop_value(&mut stack, pc, 1)?;
                memory[usize::from(slot)] = value;
                TassadarTraceEvent::Store { slot, value }
            }
            TassadarInstruction::BrIf { target_pc } => {
                let condition = pop_value(&mut stack, pc, 1)?;
                let taken = condition != 0;
                if taken {
                    next_pc = usize::from(target_pc);
                }
                TassadarTraceEvent::Branch {
                    condition,
                    taken,
                    target_pc: usize::from(target_pc),
                }
            }
            TassadarInstruction::Output => {
                let value = pop_value(&mut stack, pc, 1)?;
                outputs.push(value);
                TassadarTraceEvent::Output { value }
            }
            TassadarInstruction::Return => {
                halt_reason = TassadarHaltReason::Returned;
                TassadarTraceEvent::Return
            }
        };

        if let Some(rule) = rule {
            let observed = observed_rule_signature(&instruction, &event);
            if rule.pops != observed.pops || rule.pushes != observed.pushes {
                return Err(TassadarExecutionRefusal::FixtureRuleMismatch {
                    opcode,
                    expected_pops: rule.pops,
                    expected_pushes: rule.pushes,
                    actual_pops: observed.pops,
                    actual_pushes: observed.pushes,
                });
            }
        }

        steps.push(TassadarTraceStep {
            step_index,
            pc,
            next_pc,
            instruction: instruction.clone(),
            event,
            stack_before,
            stack_after: stack.clone(),
            locals_after: locals.clone(),
            memory_after: memory.clone(),
        });

        step_index += 1;
        if matches!(instruction, TassadarInstruction::Return) {
            return Ok(TassadarExecution {
                program_id: program.program_id.clone(),
                profile_id: program.profile_id.clone(),
                runner_id: String::from(runner_id),
                trace_abi: trace_abi.clone(),
                steps,
                outputs,
                final_locals: locals,
                final_memory: memory,
                final_stack: stack,
                halt_reason,
            });
        }

        pc = next_pc;
    }

    Ok(TassadarExecution {
        program_id: program.program_id.clone(),
        profile_id: program.profile_id.clone(),
        runner_id: String::from(runner_id),
        trace_abi: trace_abi.clone(),
        steps,
        outputs,
        final_locals: locals,
        final_memory: memory,
        final_stack: stack,
        halt_reason,
    })
}

fn record_trace_summary_step(
    step: &TassadarTraceStep,
    trace_bytes: &mut u64,
    trace_hasher: &mut Sha256,
) {
    let bytes = serde_json::to_vec(step).unwrap_or_default();
    *trace_bytes = trace_bytes.saturating_add(bytes.len() as u64);
    trace_hasher.update(bytes);
}

fn finish_execution_summary(
    program: &TassadarProgram,
    trace_abi: &TassadarTraceAbi,
    runner_id: &str,
    trace_hasher: Sha256,
    trace_bytes: u64,
    step_count: u64,
    outputs: Vec<i32>,
    final_locals: Vec<i32>,
    final_memory: Vec<i32>,
    final_stack: Vec<i32>,
    halt_reason: TassadarHaltReason,
) -> TassadarExecutionSummary {
    TassadarExecutionSummary::new(
        program.program_id.clone(),
        program.profile_id.clone(),
        String::from(runner_id),
        trace_abi,
        hex::encode(trace_hasher.finalize()),
        step_count,
        trace_bytes,
        outputs,
        final_locals,
        final_memory,
        final_stack,
        halt_reason,
    )
}

pub(crate) fn execution_summary_from_execution(
    execution: &TassadarExecution,
) -> TassadarExecutionSummary {
    let mut trace_bytes = 0u64;
    let mut trace_hasher = Sha256::new();
    for step in &execution.steps {
        record_trace_summary_step(step, &mut trace_bytes, &mut trace_hasher);
    }
    TassadarExecutionSummary::new(
        execution.program_id.clone(),
        execution.profile_id.clone(),
        execution.runner_id.clone(),
        &execution.trace_abi,
        hex::encode(trace_hasher.finalize()),
        execution.steps.len() as u64,
        trace_bytes,
        execution.outputs.clone(),
        execution.final_locals.clone(),
        execution.final_memory.clone(),
        execution.final_stack.clone(),
        execution.halt_reason,
    )
}

pub fn execute_program_direct_summary(
    program: &TassadarProgram,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    runner_id: &str,
) -> Result<TassadarExecutionSummary, TassadarExecutionRefusal> {
    program.validate_against(profile)?;

    let mut pc = 0usize;
    let mut outputs = Vec::new();
    let mut stack = Vec::new();
    let mut locals = vec![0; program.local_count];
    let mut memory = program.initial_memory.clone();
    let mut step_index = 0usize;
    let mut halt_reason = TassadarHaltReason::FellOffEnd;
    let mut trace_bytes = 0u64;
    let mut trace_hasher = Sha256::new();

    while pc < program.instructions.len() {
        if step_index >= profile.max_steps {
            return Err(TassadarExecutionRefusal::StepLimitExceeded {
                max_steps: profile.max_steps,
            });
        }

        let instruction = program.instructions[pc].clone();
        let stack_before = stack.clone();
        let mut next_pc = pc + 1;
        let event = execute_instruction(
            &instruction,
            pc,
            &mut next_pc,
            &mut stack,
            &mut locals,
            &mut memory,
            &mut outputs,
            &mut halt_reason,
        )?;
        let step = TassadarTraceStep {
            step_index,
            pc,
            next_pc,
            instruction: instruction.clone(),
            event,
            stack_before,
            stack_after: stack.clone(),
            locals_after: locals.clone(),
            memory_after: memory.clone(),
        };
        record_trace_summary_step(&step, &mut trace_bytes, &mut trace_hasher);

        step_index += 1;
        if matches!(instruction, TassadarInstruction::Return) {
            return Ok(finish_execution_summary(
                program,
                trace_abi,
                runner_id,
                trace_hasher,
                trace_bytes,
                step_index as u64,
                outputs,
                locals,
                memory,
                stack,
                halt_reason,
            ));
        }

        pc = next_pc;
    }

    Ok(finish_execution_summary(
        program,
        trace_abi,
        runner_id,
        trace_hasher,
        trace_bytes,
        step_index as u64,
        outputs,
        locals,
        memory,
        stack,
        halt_reason,
    ))
}

pub fn execute_program_hull_cache_summary(
    program: &TassadarProgram,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    runner_id: &str,
    fixture_weights: &TassadarFixtureWeights,
) -> Result<TassadarExecutionSummary, TassadarExecutionRefusal> {
    program.validate_against(profile)?;
    validate_hull_cache_program(program)?;

    let mut pc = 0usize;
    let mut state = TassadarHullCacheState {
        stack: Vec::new(),
        locals: vec![0; program.local_count],
        memory: program.initial_memory.clone(),
        outputs: Vec::new(),
        local_last_write_step: vec![None; program.local_count],
        memory_last_write_step: vec![None; program.initial_memory.len()],
    };
    let mut step_index = 0usize;
    let mut halt_reason = TassadarHaltReason::FellOffEnd;
    let mut trace_bytes = 0u64;
    let mut trace_hasher = Sha256::new();

    while pc < program.instructions.len() {
        if step_index >= profile.max_steps {
            return Err(TassadarExecutionRefusal::StepLimitExceeded {
                max_steps: profile.max_steps,
            });
        }

        let instruction = program.instructions[pc].clone();
        let stack_before = state.stack.clone();
        let opcode = instruction.opcode();
        let rule = fixture_weights
            .rule_for(opcode)
            .ok_or(TassadarExecutionRefusal::FixtureRuleMissing { opcode })?;
        let mut next_pc = pc + 1;
        let event = match instruction.clone() {
            TassadarInstruction::LocalGet { local } => {
                let value = state.locals[usize::from(local)];
                state.stack.push(value);
                TassadarTraceEvent::LocalGet { local, value }
            }
            TassadarInstruction::I32Load { slot } => {
                let value = state.memory[usize::from(slot)];
                state.stack.push(value);
                TassadarTraceEvent::Load { slot, value }
            }
            _ => execute_instruction(
                &instruction,
                pc,
                &mut next_pc,
                &mut state.stack,
                &mut state.locals,
                &mut state.memory,
                &mut state.outputs,
                &mut halt_reason,
            )?,
        };

        let observed = observed_rule_signature(&instruction, &event);
        if rule.pops != observed.pops || rule.pushes != observed.pushes {
            return Err(TassadarExecutionRefusal::FixtureRuleMismatch {
                opcode,
                expected_pops: rule.pops,
                expected_pushes: rule.pushes,
                actual_pops: observed.pops,
                actual_pushes: observed.pushes,
            });
        }
        match event {
            TassadarTraceEvent::LocalSet { local, .. } => {
                state.local_last_write_step[usize::from(local)] = Some(step_index);
            }
            TassadarTraceEvent::Store { slot, .. } => {
                state.memory_last_write_step[usize::from(slot)] = Some(step_index);
            }
            _ => {}
        }

        let step = TassadarTraceStep {
            step_index,
            pc,
            next_pc,
            instruction: instruction.clone(),
            event,
            stack_before,
            stack_after: state.stack.clone(),
            locals_after: state.locals.clone(),
            memory_after: state.memory.clone(),
        };
        record_trace_summary_step(&step, &mut trace_bytes, &mut trace_hasher);

        step_index += 1;
        if matches!(instruction, TassadarInstruction::Return) {
            return Ok(finish_execution_summary(
                program,
                trace_abi,
                runner_id,
                trace_hasher,
                trace_bytes,
                step_index as u64,
                state.outputs,
                state.locals,
                state.memory,
                state.stack,
                halt_reason,
            ));
        }

        pc = next_pc;
    }

    Ok(finish_execution_summary(
        program,
        trace_abi,
        runner_id,
        trace_hasher,
        trace_bytes,
        step_index as u64,
        state.outputs,
        state.locals,
        state.memory,
        state.stack,
        halt_reason,
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TassadarDecodedState {
    stack: Vec<i32>,
    locals: Vec<i32>,
    memory: Vec<i32>,
    outputs: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TassadarHullCacheState {
    stack: Vec<i32>,
    locals: Vec<i32>,
    memory: Vec<i32>,
    outputs: Vec<i32>,
    local_last_write_step: Vec<Option<usize>>,
    memory_last_write_step: Vec<Option<usize>>,
}

fn execute_program_linear_decode(
    program: &TassadarProgram,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    runner_id: &str,
    fixture_weights: Option<&TassadarFixtureWeights>,
) -> Result<TassadarExecution, TassadarExecutionRefusal> {
    program.validate_against(profile)?;

    let mut pc = 0usize;
    let mut steps = Vec::new();
    let mut final_state = TassadarDecodedState {
        stack: Vec::new(),
        locals: vec![0; program.local_count],
        memory: program.initial_memory.clone(),
        outputs: Vec::new(),
    };
    let mut step_index = 0usize;
    let mut halt_reason = TassadarHaltReason::FellOffEnd;

    while pc < program.instructions.len() {
        if step_index >= profile.max_steps {
            return Err(TassadarExecutionRefusal::StepLimitExceeded {
                max_steps: profile.max_steps,
            });
        }

        let mut state = replay_decoded_state(
            steps.as_slice(),
            program.local_count,
            program.initial_memory.as_slice(),
        );
        let instruction = program.instructions[pc].clone();
        let stack_before = state.stack.clone();
        let opcode = instruction.opcode();
        let rule = match fixture_weights {
            Some(weights) => Some(
                weights
                    .rule_for(opcode)
                    .ok_or(TassadarExecutionRefusal::FixtureRuleMissing { opcode })?,
            ),
            None => None,
        };
        let mut next_pc = pc + 1;
        let event = execute_instruction(
            &instruction,
            pc,
            &mut next_pc,
            &mut state.stack,
            &mut state.locals,
            &mut state.memory,
            &mut state.outputs,
            &mut halt_reason,
        )?;

        if let Some(rule) = rule {
            let observed = observed_rule_signature(&instruction, &event);
            if rule.pops != observed.pops || rule.pushes != observed.pushes {
                return Err(TassadarExecutionRefusal::FixtureRuleMismatch {
                    opcode,
                    expected_pops: rule.pops,
                    expected_pushes: rule.pushes,
                    actual_pops: observed.pops,
                    actual_pushes: observed.pushes,
                });
            }
        }

        steps.push(TassadarTraceStep {
            step_index,
            pc,
            next_pc,
            instruction: instruction.clone(),
            event,
            stack_before,
            stack_after: state.stack.clone(),
            locals_after: state.locals.clone(),
            memory_after: state.memory.clone(),
        });

        final_state = state;
        step_index += 1;
        if matches!(instruction, TassadarInstruction::Return) {
            return Ok(build_tassadar_execution(
                program,
                trace_abi,
                runner_id,
                steps,
                final_state.outputs,
                final_state.locals,
                final_state.memory,
                final_state.stack,
                halt_reason,
            ));
        }

        pc = next_pc;
    }

    Ok(build_tassadar_execution(
        program,
        trace_abi,
        runner_id,
        steps,
        final_state.outputs,
        final_state.locals,
        final_state.memory,
        final_state.stack,
        halt_reason,
    ))
}

fn execute_program_hull_cache(
    program: &TassadarProgram,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    runner_id: &str,
    fixture_weights: Option<&TassadarFixtureWeights>,
) -> Result<TassadarExecution, TassadarExecutionRefusal> {
    program.validate_against(profile)?;
    validate_hull_cache_program(program)?;

    let mut pc = 0usize;
    let mut steps = Vec::new();
    let mut state = TassadarHullCacheState {
        stack: Vec::new(),
        locals: vec![0; program.local_count],
        memory: program.initial_memory.clone(),
        outputs: Vec::new(),
        local_last_write_step: vec![None; program.local_count],
        memory_last_write_step: vec![None; program.initial_memory.len()],
    };
    let mut step_index = 0usize;
    let mut halt_reason = TassadarHaltReason::FellOffEnd;

    while pc < program.instructions.len() {
        if step_index >= profile.max_steps {
            return Err(TassadarExecutionRefusal::StepLimitExceeded {
                max_steps: profile.max_steps,
            });
        }

        let instruction = program.instructions[pc].clone();
        let stack_before = state.stack.clone();
        let opcode = instruction.opcode();
        let rule = match fixture_weights {
            Some(weights) => Some(
                weights
                    .rule_for(opcode)
                    .ok_or(TassadarExecutionRefusal::FixtureRuleMissing { opcode })?,
            ),
            None => None,
        };
        let mut next_pc = pc + 1;
        let event = match instruction.clone() {
            TassadarInstruction::LocalGet { local } => {
                let value = hull_cache_local_value(
                    usize::from(local),
                    steps.as_slice(),
                    &state.local_last_write_step,
                    &state.locals,
                );
                state.stack.push(value);
                TassadarTraceEvent::LocalGet { local, value }
            }
            TassadarInstruction::I32Load { slot } => {
                let value = hull_cache_memory_value(
                    usize::from(slot),
                    program.initial_memory.as_slice(),
                    steps.as_slice(),
                    &state.memory_last_write_step,
                    &state.memory,
                );
                state.stack.push(value);
                TassadarTraceEvent::Load { slot, value }
            }
            _ => execute_instruction(
                &instruction,
                pc,
                &mut next_pc,
                &mut state.stack,
                &mut state.locals,
                &mut state.memory,
                &mut state.outputs,
                &mut halt_reason,
            )?,
        };

        if let Some(rule) = rule {
            let observed = observed_rule_signature(&instruction, &event);
            if rule.pops != observed.pops || rule.pushes != observed.pushes {
                return Err(TassadarExecutionRefusal::FixtureRuleMismatch {
                    opcode,
                    expected_pops: rule.pops,
                    expected_pushes: rule.pushes,
                    actual_pops: observed.pops,
                    actual_pushes: observed.pushes,
                });
            }
        }
        match event {
            TassadarTraceEvent::LocalSet { local, .. } => {
                state.local_last_write_step[usize::from(local)] = Some(step_index);
            }
            TassadarTraceEvent::Store { slot, .. } => {
                state.memory_last_write_step[usize::from(slot)] = Some(step_index);
            }
            _ => {}
        }

        steps.push(TassadarTraceStep {
            step_index,
            pc,
            next_pc,
            instruction: instruction.clone(),
            event,
            stack_before,
            stack_after: state.stack.clone(),
            locals_after: state.locals.clone(),
            memory_after: state.memory.clone(),
        });

        step_index += 1;
        if matches!(instruction, TassadarInstruction::Return) {
            return Ok(build_tassadar_execution(
                program,
                trace_abi,
                runner_id,
                steps,
                state.outputs,
                state.locals,
                state.memory,
                state.stack,
                halt_reason,
            ));
        }

        pc = next_pc;
    }

    Ok(build_tassadar_execution(
        program,
        trace_abi,
        runner_id,
        steps,
        state.outputs,
        state.locals,
        state.memory,
        state.stack,
        halt_reason,
    ))
}

fn execute_program_hierarchical_hull_candidate(
    program: &TassadarProgram,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    runner_id: &str,
    fixture_weights: Option<&TassadarFixtureWeights>,
) -> Result<TassadarExecution, TassadarExecutionRefusal> {
    program.validate_against(profile)?;

    let mut pc = 0usize;
    let mut steps = Vec::new();
    let mut state = TassadarHierarchicalHullCandidateState {
        stack: Vec::new(),
        locals: vec![0; program.local_count],
        memory: program.initial_memory.clone(),
        outputs: Vec::new(),
        local_write_history: vec![Vec::new(); program.local_count],
        memory_write_history: vec![Vec::new(); program.initial_memory.len()],
    };
    let mut step_index = 0usize;
    let mut halt_reason = TassadarHaltReason::FellOffEnd;

    while pc < program.instructions.len() {
        if step_index >= profile.max_steps {
            return Err(TassadarExecutionRefusal::StepLimitExceeded {
                max_steps: profile.max_steps,
            });
        }

        let instruction = program.instructions[pc].clone();
        let stack_before = state.stack.clone();
        let opcode = instruction.opcode();
        let rule = match fixture_weights {
            Some(weights) => Some(
                weights
                    .rule_for(opcode)
                    .ok_or(TassadarExecutionRefusal::FixtureRuleMissing { opcode })?,
            ),
            None => None,
        };
        let mut next_pc = pc + 1;
        let event = match instruction.clone() {
            TassadarInstruction::LocalGet { local } => {
                let value = hierarchical_hull_local_value(
                    usize::from(local),
                    steps.as_slice(),
                    &state.local_write_history,
                    &state.locals,
                );
                state.stack.push(value);
                TassadarTraceEvent::LocalGet { local, value }
            }
            TassadarInstruction::I32Load { slot } => {
                let value = hierarchical_hull_memory_value(
                    usize::from(slot),
                    program.initial_memory.as_slice(),
                    steps.as_slice(),
                    &state.memory_write_history,
                    &state.memory,
                );
                state.stack.push(value);
                TassadarTraceEvent::Load { slot, value }
            }
            _ => execute_instruction(
                &instruction,
                pc,
                &mut next_pc,
                &mut state.stack,
                &mut state.locals,
                &mut state.memory,
                &mut state.outputs,
                &mut halt_reason,
            )?,
        };

        if let Some(rule) = rule {
            let observed = observed_rule_signature(&instruction, &event);
            if rule.pops != observed.pops || rule.pushes != observed.pushes {
                return Err(TassadarExecutionRefusal::FixtureRuleMismatch {
                    opcode,
                    expected_pops: rule.pops,
                    expected_pushes: rule.pushes,
                    actual_pops: observed.pops,
                    actual_pushes: observed.pushes,
                });
            }
        }

        match event {
            TassadarTraceEvent::LocalSet { local, .. } => {
                state.local_write_history[usize::from(local)].push(step_index);
            }
            TassadarTraceEvent::Store { slot, .. } => {
                state.memory_write_history[usize::from(slot)].push(step_index);
            }
            _ => {}
        }

        steps.push(TassadarTraceStep {
            step_index,
            pc,
            next_pc,
            instruction: instruction.clone(),
            event,
            stack_before,
            stack_after: state.stack.clone(),
            locals_after: state.locals.clone(),
            memory_after: state.memory.clone(),
        });

        step_index += 1;
        if matches!(instruction, TassadarInstruction::Return) {
            return Ok(build_tassadar_execution(
                program,
                trace_abi,
                runner_id,
                steps,
                state.outputs,
                state.locals,
                state.memory,
                state.stack,
                halt_reason,
            ));
        }

        pc = next_pc;
    }

    Ok(build_tassadar_execution(
        program,
        trace_abi,
        runner_id,
        steps,
        state.outputs,
        state.locals,
        state.memory,
        state.stack,
        halt_reason,
    ))
}

fn execute_program_sparse_top_k(
    program: &TassadarProgram,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    runner_id: &str,
    fixture_weights: Option<&TassadarFixtureWeights>,
    top_k: usize,
) -> Result<TassadarExecution, TassadarExecutionRefusal> {
    program.validate_against(profile)?;
    validate_sparse_top_k_program(program)?;

    let mut pc = 0usize;
    let mut steps = Vec::new();
    let mut state = TassadarSparseTopKState {
        stack: Vec::new(),
        locals: vec![0; program.local_count],
        memory: program.initial_memory.clone(),
        outputs: Vec::new(),
        local_recent_write_steps: vec![Vec::new(); program.local_count],
        memory_recent_write_steps: vec![Vec::new(); program.initial_memory.len()],
    };
    let mut step_index = 0usize;
    let mut halt_reason = TassadarHaltReason::FellOffEnd;

    while pc < program.instructions.len() {
        if step_index >= profile.max_steps {
            return Err(TassadarExecutionRefusal::StepLimitExceeded {
                max_steps: profile.max_steps,
            });
        }

        let instruction = program.instructions[pc].clone();
        let stack_before = state.stack.clone();
        let opcode = instruction.opcode();
        let rule = match fixture_weights {
            Some(weights) => Some(
                weights
                    .rule_for(opcode)
                    .ok_or(TassadarExecutionRefusal::FixtureRuleMissing { opcode })?,
            ),
            None => None,
        };
        let mut next_pc = pc + 1;
        let event = match instruction.clone() {
            TassadarInstruction::LocalGet { local } => {
                let value = sparse_top_k_local_value(
                    usize::from(local),
                    steps.as_slice(),
                    &state.local_recent_write_steps,
                    &state.locals,
                );
                state.stack.push(value);
                TassadarTraceEvent::LocalGet { local, value }
            }
            TassadarInstruction::I32Load { slot } => {
                let value = sparse_top_k_memory_value(
                    usize::from(slot),
                    program.initial_memory.as_slice(),
                    steps.as_slice(),
                    &state.memory_recent_write_steps,
                    &state.memory,
                );
                state.stack.push(value);
                TassadarTraceEvent::Load { slot, value }
            }
            _ => execute_instruction(
                &instruction,
                pc,
                &mut next_pc,
                &mut state.stack,
                &mut state.locals,
                &mut state.memory,
                &mut state.outputs,
                &mut halt_reason,
            )?,
        };

        if let Some(rule) = rule {
            let observed = observed_rule_signature(&instruction, &event);
            if rule.pops != observed.pops || rule.pushes != observed.pushes {
                return Err(TassadarExecutionRefusal::FixtureRuleMismatch {
                    opcode,
                    expected_pops: rule.pops,
                    expected_pushes: rule.pushes,
                    actual_pops: observed.pops,
                    actual_pushes: observed.pushes,
                });
            }
        }
        match event {
            TassadarTraceEvent::LocalSet { local, .. } => {
                record_sparse_top_k_write(
                    &mut state.local_recent_write_steps[usize::from(local)],
                    step_index,
                    top_k,
                );
            }
            TassadarTraceEvent::Store { slot, .. } => {
                record_sparse_top_k_write(
                    &mut state.memory_recent_write_steps[usize::from(slot)],
                    step_index,
                    top_k,
                );
            }
            _ => {}
        }

        steps.push(TassadarTraceStep {
            step_index,
            pc,
            next_pc,
            instruction: instruction.clone(),
            event,
            stack_before,
            stack_after: state.stack.clone(),
            locals_after: state.locals.clone(),
            memory_after: state.memory.clone(),
        });

        step_index += 1;
        if matches!(instruction, TassadarInstruction::Return) {
            return Ok(build_tassadar_execution(
                program,
                trace_abi,
                runner_id,
                steps,
                state.outputs,
                state.locals,
                state.memory,
                state.stack,
                halt_reason,
            ));
        }

        pc = next_pc;
    }

    Ok(build_tassadar_execution(
        program,
        trace_abi,
        runner_id,
        steps,
        state.outputs,
        state.locals,
        state.memory,
        state.stack,
        halt_reason,
    ))
}

fn validate_hull_cache_program(program: &TassadarProgram) -> Result<(), TassadarExecutionRefusal> {
    let allow_backward_branches =
        article_hull_cache_direct_program_digests().contains(program.program_digest().as_str());
    for (pc, instruction) in program.instructions.iter().enumerate() {
        if let TassadarInstruction::BrIf { target_pc } = instruction {
            if usize::from(*target_pc) <= pc && !allow_backward_branches {
                return Err(
                    TassadarExecutionRefusal::HullCacheBackwardBranchUnsupported {
                        pc,
                        target_pc: usize::from(*target_pc),
                    },
                );
            }
        }
    }
    Ok(())
}

fn article_hull_cache_direct_program_digests() -> &'static BTreeSet<String> {
    static DIGESTS: OnceLock<BTreeSet<String>> = OnceLock::new();
    DIGESTS.get_or_init(|| {
        let mut digests = tassadar_article_class_corpus()
            .into_iter()
            .map(|case| case.program.program_digest())
            .collect::<BTreeSet<_>>();
        if let Some(case) = tassadar_sudoku_9x9_corpus()
            .into_iter()
            .find(|case| case.case_id == "sudoku_9x9_test_a")
        {
            digests.insert(case.validation_case.program.program_digest());
        }
        if let Some(case) = tassadar_hungarian_10x10_corpus()
            .into_iter()
            .find(|case| case.case_id == "hungarian_10x10_test_a")
        {
            digests.insert(case.validation_case.program.program_digest());
        }
        digests.extend(
            crate::tassadar_article_runtime_closeout::article_runtime_closeout_hull_cache_direct_programs(
            )
            .into_iter()
            .map(|program| program.program_digest()),
        );
        digests
    })
}

fn validate_sparse_top_k_program(
    program: &TassadarProgram,
) -> Result<(), TassadarExecutionRefusal> {
    const SPARSE_TOP_K_MAX_PROGRAM_LEN: usize = 64;
    if program.instructions.len() > SPARSE_TOP_K_MAX_PROGRAM_LEN {
        return Err(TassadarExecutionRefusal::SparseTopKProgramTooLong {
            instruction_count: program.instructions.len(),
            max_supported: SPARSE_TOP_K_MAX_PROGRAM_LEN,
        });
    }
    for (pc, instruction) in program.instructions.iter().enumerate() {
        if let TassadarInstruction::BrIf { target_pc } = instruction {
            if usize::from(*target_pc) <= pc {
                return Err(
                    TassadarExecutionRefusal::SparseTopKBackwardBranchUnsupported {
                        pc,
                        target_pc: usize::from(*target_pc),
                    },
                );
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
struct TassadarSparseTopKState {
    stack: Vec<i32>,
    locals: Vec<i32>,
    memory: Vec<i32>,
    outputs: Vec<i32>,
    local_recent_write_steps: Vec<Vec<usize>>,
    memory_recent_write_steps: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, Default)]
struct TassadarHierarchicalHullCandidateState {
    stack: Vec<i32>,
    locals: Vec<i32>,
    memory: Vec<i32>,
    outputs: Vec<i32>,
    local_write_history: Vec<Vec<usize>>,
    memory_write_history: Vec<Vec<usize>>,
}

fn record_sparse_top_k_write(history: &mut Vec<usize>, step_index: usize, top_k: usize) {
    history.insert(0, step_index);
    if history.len() > top_k {
        history.truncate(top_k);
    }
}

fn hierarchical_hull_local_value(
    local: usize,
    steps: &[TassadarTraceStep],
    write_history: &[Vec<usize>],
    locals: &[i32],
) -> i32 {
    write_history[local]
        .iter()
        .rev()
        .find_map(
            |index| match steps.get(*index).map(|step| step.event.clone()) {
                Some(TassadarTraceEvent::LocalSet { value, .. }) => Some(value),
                _ => None,
            },
        )
        .unwrap_or(locals[local])
}

fn hierarchical_hull_memory_value(
    slot: usize,
    initial_memory: &[i32],
    steps: &[TassadarTraceStep],
    write_history: &[Vec<usize>],
    memory: &[i32],
) -> i32 {
    write_history[slot]
        .iter()
        .rev()
        .find_map(
            |index| match steps.get(*index).map(|step| step.event.clone()) {
                Some(TassadarTraceEvent::Store { value, .. }) => Some(value),
                _ => None,
            },
        )
        .unwrap_or(memory.get(slot).copied().unwrap_or(initial_memory[slot]))
}

fn sparse_top_k_local_value(
    local: usize,
    steps: &[TassadarTraceStep],
    recent_write_steps: &[Vec<usize>],
    locals: &[i32],
) -> i32 {
    recent_write_steps[local]
        .iter()
        .find_map(
            |index| match steps.get(*index).map(|step| step.event.clone()) {
                Some(TassadarTraceEvent::LocalSet { value, .. }) => Some(value),
                _ => None,
            },
        )
        .unwrap_or(locals[local])
}

fn sparse_top_k_memory_value(
    slot: usize,
    initial_memory: &[i32],
    steps: &[TassadarTraceStep],
    recent_write_steps: &[Vec<usize>],
    memory: &[i32],
) -> i32 {
    recent_write_steps[slot]
        .iter()
        .find_map(
            |index| match steps.get(*index).map(|step| step.event.clone()) {
                Some(TassadarTraceEvent::Store { value, .. }) => Some(value),
                _ => None,
            },
        )
        .unwrap_or(memory.get(slot).copied().unwrap_or(initial_memory[slot]))
}

fn hull_cache_local_value(
    local: usize,
    steps: &[TassadarTraceStep],
    last_write_step: &[Option<usize>],
    locals: &[i32],
) -> i32 {
    last_write_step[local]
        .and_then(
            |index| match steps.get(index).map(|step| step.event.clone()) {
                Some(TassadarTraceEvent::LocalSet { value, .. }) => Some(value),
                _ => None,
            },
        )
        .unwrap_or(locals[local])
}

fn hull_cache_memory_value(
    slot: usize,
    initial_memory: &[i32],
    steps: &[TassadarTraceStep],
    last_write_step: &[Option<usize>],
    memory: &[i32],
) -> i32 {
    last_write_step[slot]
        .and_then(
            |index| match steps.get(index).map(|step| step.event.clone()) {
                Some(TassadarTraceEvent::Store { value, .. }) => Some(value),
                _ => None,
            },
        )
        .unwrap_or_else(|| initial_memory.get(slot).copied().unwrap_or(memory[slot]))
}

fn replay_decoded_state(
    steps: &[TassadarTraceStep],
    local_count: usize,
    initial_memory: &[i32],
) -> TassadarDecodedState {
    let mut state = TassadarDecodedState {
        stack: Vec::new(),
        locals: vec![0; local_count],
        memory: initial_memory.to_vec(),
        outputs: Vec::new(),
    };
    for step in steps {
        state.stack = step.stack_after.clone();
        state.locals = step.locals_after.clone();
        state.memory = step.memory_after.clone();
        if let TassadarTraceEvent::Output { value } = step.event {
            state.outputs.push(value);
        }
    }
    state
}

fn build_tassadar_execution(
    program: &TassadarProgram,
    trace_abi: &TassadarTraceAbi,
    runner_id: &str,
    steps: Vec<TassadarTraceStep>,
    outputs: Vec<i32>,
    final_locals: Vec<i32>,
    final_memory: Vec<i32>,
    final_stack: Vec<i32>,
    halt_reason: TassadarHaltReason,
) -> TassadarExecution {
    TassadarExecution {
        program_id: program.program_id.clone(),
        profile_id: program.profile_id.clone(),
        runner_id: String::from(runner_id),
        trace_abi: trace_abi.clone(),
        steps,
        outputs,
        final_locals,
        final_memory,
        final_stack,
        halt_reason,
    }
}

fn execute_instruction(
    instruction: &TassadarInstruction,
    pc: usize,
    next_pc: &mut usize,
    stack: &mut Vec<i32>,
    locals: &mut [i32],
    memory: &mut [i32],
    outputs: &mut Vec<i32>,
    halt_reason: &mut TassadarHaltReason,
) -> Result<TassadarTraceEvent, TassadarExecutionRefusal> {
    Ok(match instruction.clone() {
        TassadarInstruction::Nop => TassadarTraceEvent::Nop,
        TassadarInstruction::I32Const { value } => {
            stack.push(value);
            TassadarTraceEvent::ConstPush { value }
        }
        TassadarInstruction::LocalGet { local } => {
            let value = locals[usize::from(local)];
            stack.push(value);
            TassadarTraceEvent::LocalGet { local, value }
        }
        TassadarInstruction::LocalSet { local } => {
            let value = pop_value(stack, pc, 1)?;
            locals[usize::from(local)] = value;
            TassadarTraceEvent::LocalSet { local, value }
        }
        TassadarInstruction::LocalTee { local } => {
            let value = peek_value(stack, pc, 1)?;
            locals[usize::from(local)] = value;
            TassadarTraceEvent::LocalTee { local, value }
        }
        TassadarInstruction::Drop => {
            let value = pop_value(stack, pc, 1)?;
            TassadarTraceEvent::Drop { value }
        }
        TassadarInstruction::I32Add => {
            let (left, right) = pop_binary_operands(stack, pc)?;
            let result = left + right;
            stack.push(result);
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Add,
                left,
                right,
                result,
            }
        }
        TassadarInstruction::I32Sub => {
            let (left, right) = pop_binary_operands(stack, pc)?;
            let result = left - right;
            stack.push(result);
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Sub,
                left,
                right,
                result,
            }
        }
        TassadarInstruction::I32Mul => {
            let (left, right) = pop_binary_operands(stack, pc)?;
            let result = left * right;
            stack.push(result);
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Mul,
                left,
                right,
                result,
            }
        }
        TassadarInstruction::I32Lt => {
            let (left, right) = pop_binary_operands(stack, pc)?;
            let result = i32::from(left < right);
            stack.push(result);
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Lt,
                left,
                right,
                result,
            }
        }
        TassadarInstruction::I32Eqz => {
            let operand = pop_value(stack, pc, 1)?;
            let result = i32::from(operand == 0);
            stack.push(result);
            TassadarTraceEvent::UnaryOp {
                op: TassadarUnaryOp::Eqz,
                operand,
                result,
            }
        }
        TassadarInstruction::I32Eq => {
            let (left, right) = pop_binary_operands(stack, pc)?;
            let result = i32::from(left == right);
            stack.push(result);
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Eq,
                left,
                right,
                result,
            }
        }
        TassadarInstruction::I32Ne => {
            let (left, right) = pop_binary_operands(stack, pc)?;
            let result = i32::from(left != right);
            stack.push(result);
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Ne,
                left,
                right,
                result,
            }
        }
        TassadarInstruction::I32Gt => {
            let (left, right) = pop_binary_operands(stack, pc)?;
            let result = i32::from(left > right);
            stack.push(result);
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Gt,
                left,
                right,
                result,
            }
        }
        TassadarInstruction::I32Le => {
            let (left, right) = pop_binary_operands(stack, pc)?;
            let result = i32::from(left <= right);
            stack.push(result);
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Le,
                left,
                right,
                result,
            }
        }
        TassadarInstruction::I32Ge => {
            let (left, right) = pop_binary_operands(stack, pc)?;
            let result = i32::from(left >= right);
            stack.push(result);
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Ge,
                left,
                right,
                result,
            }
        }
        TassadarInstruction::I32Load { slot } => {
            let value = memory[usize::from(slot)];
            stack.push(value);
            TassadarTraceEvent::Load { slot, value }
        }
        TassadarInstruction::I32Store { slot } => {
            let value = pop_value(stack, pc, 1)?;
            memory[usize::from(slot)] = value;
            TassadarTraceEvent::Store { slot, value }
        }
        TassadarInstruction::BrIf { target_pc } => {
            let condition = pop_value(stack, pc, 1)?;
            let taken = condition != 0;
            if taken {
                *next_pc = usize::from(target_pc);
            }
            TassadarTraceEvent::Branch {
                condition,
                taken,
                target_pc: usize::from(target_pc),
            }
        }
        TassadarInstruction::Output => {
            let value = pop_value(stack, pc, 1)?;
            outputs.push(value);
            TassadarTraceEvent::Output { value }
        }
        TassadarInstruction::Return => {
            *halt_reason = TassadarHaltReason::Returned;
            TassadarTraceEvent::Return
        }
    })
}

fn pop_binary_operands(
    stack: &mut Vec<i32>,
    pc: usize,
) -> Result<(i32, i32), TassadarExecutionRefusal> {
    if stack.len() < 2 {
        return Err(TassadarExecutionRefusal::StackUnderflow {
            pc,
            needed: 2,
            available: stack.len(),
        });
    }
    let right = stack.pop().expect("len checked");
    let left = stack.pop().expect("len checked");
    Ok((left, right))
}

fn pop_value(
    stack: &mut Vec<i32>,
    pc: usize,
    needed: usize,
) -> Result<i32, TassadarExecutionRefusal> {
    stack.pop().ok_or(TassadarExecutionRefusal::StackUnderflow {
        pc,
        needed,
        available: 0,
    })
}

fn peek_value(stack: &[i32], pc: usize, needed: usize) -> Result<i32, TassadarExecutionRefusal> {
    stack
        .last()
        .copied()
        .ok_or(TassadarExecutionRefusal::StackUnderflow {
            pc,
            needed,
            available: stack.len(),
        })
}

fn locals_add_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::core_i32_v1();
    TassadarValidationCase {
        case_id: String::from("locals_add"),
        summary: String::from("local set/get plus addition and output"),
        program: TassadarProgram::new(
            "tassadar.locals_add.v1",
            &profile,
            2,
            2,
            vec![
                TassadarInstruction::I32Const { value: 7 },
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 5 },
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::I32Add,
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        ),
        expected_trace: vec![
            trace_step(
                0,
                0,
                1,
                TassadarInstruction::I32Const { value: 7 },
                TassadarTraceEvent::ConstPush { value: 7 },
                &[],
                &[7],
                &[0, 0],
                &[0, 0],
            ),
            trace_step(
                1,
                1,
                2,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarTraceEvent::LocalSet { local: 0, value: 7 },
                &[7],
                &[],
                &[7, 0],
                &[0, 0],
            ),
            trace_step(
                2,
                2,
                3,
                TassadarInstruction::I32Const { value: 5 },
                TassadarTraceEvent::ConstPush { value: 5 },
                &[],
                &[5],
                &[7, 0],
                &[0, 0],
            ),
            trace_step(
                3,
                3,
                4,
                TassadarInstruction::LocalSet { local: 1 },
                TassadarTraceEvent::LocalSet { local: 1, value: 5 },
                &[5],
                &[],
                &[7, 5],
                &[0, 0],
            ),
            trace_step(
                4,
                4,
                5,
                TassadarInstruction::LocalGet { local: 0 },
                TassadarTraceEvent::LocalGet { local: 0, value: 7 },
                &[],
                &[7],
                &[7, 5],
                &[0, 0],
            ),
            trace_step(
                5,
                5,
                6,
                TassadarInstruction::LocalGet { local: 1 },
                TassadarTraceEvent::LocalGet { local: 1, value: 5 },
                &[7],
                &[7, 5],
                &[7, 5],
                &[0, 0],
            ),
            trace_step(
                6,
                6,
                7,
                TassadarInstruction::I32Add,
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Add,
                    left: 7,
                    right: 5,
                    result: 12,
                },
                &[7, 5],
                &[12],
                &[7, 5],
                &[0, 0],
            ),
            trace_step(
                7,
                7,
                8,
                TassadarInstruction::Output,
                TassadarTraceEvent::Output { value: 12 },
                &[12],
                &[],
                &[7, 5],
                &[0, 0],
            ),
            trace_step(
                8,
                8,
                9,
                TassadarInstruction::Return,
                TassadarTraceEvent::Return,
                &[],
                &[],
                &[7, 5],
                &[0, 0],
            ),
        ],
        expected_outputs: vec![12],
    }
}

fn memory_roundtrip_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::core_i32_v1();
    TassadarValidationCase {
        case_id: String::from("memory_roundtrip"),
        summary: String::from("memory store/load plus multiplication and output"),
        program: TassadarProgram::new(
            "tassadar.memory_roundtrip.v1",
            &profile,
            0,
            4,
            vec![
                TassadarInstruction::I32Const { value: 9 },
                TassadarInstruction::I32Store { slot: 2 },
                TassadarInstruction::I32Load { slot: 2 },
                TassadarInstruction::I32Const { value: 3 },
                TassadarInstruction::I32Mul,
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        ),
        expected_trace: vec![
            trace_step(
                0,
                0,
                1,
                TassadarInstruction::I32Const { value: 9 },
                TassadarTraceEvent::ConstPush { value: 9 },
                &[],
                &[9],
                &[],
                &[0, 0, 0, 0],
            ),
            trace_step(
                1,
                1,
                2,
                TassadarInstruction::I32Store { slot: 2 },
                TassadarTraceEvent::Store { slot: 2, value: 9 },
                &[9],
                &[],
                &[],
                &[0, 0, 9, 0],
            ),
            trace_step(
                2,
                2,
                3,
                TassadarInstruction::I32Load { slot: 2 },
                TassadarTraceEvent::Load { slot: 2, value: 9 },
                &[],
                &[9],
                &[],
                &[0, 0, 9, 0],
            ),
            trace_step(
                3,
                3,
                4,
                TassadarInstruction::I32Const { value: 3 },
                TassadarTraceEvent::ConstPush { value: 3 },
                &[9],
                &[9, 3],
                &[],
                &[0, 0, 9, 0],
            ),
            trace_step(
                4,
                4,
                5,
                TassadarInstruction::I32Mul,
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Mul,
                    left: 9,
                    right: 3,
                    result: 27,
                },
                &[9, 3],
                &[27],
                &[],
                &[0, 0, 9, 0],
            ),
            trace_step(
                5,
                5,
                6,
                TassadarInstruction::Output,
                TassadarTraceEvent::Output { value: 27 },
                &[27],
                &[],
                &[],
                &[0, 0, 9, 0],
            ),
            trace_step(
                6,
                6,
                7,
                TassadarInstruction::Return,
                TassadarTraceEvent::Return,
                &[],
                &[],
                &[],
                &[0, 0, 9, 0],
            ),
        ],
        expected_outputs: vec![27],
    }
}

fn branch_guard_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::core_i32_v1();
    TassadarValidationCase {
        case_id: String::from("branch_guard"),
        summary: String::from("mul/sub with both untaken and taken conditional branches"),
        program: TassadarProgram::new(
            "tassadar.branch_guard.v1",
            &profile,
            0,
            0,
            vec![
                TassadarInstruction::I32Const { value: 3 },
                TassadarInstruction::I32Const { value: 4 },
                TassadarInstruction::I32Mul,
                TassadarInstruction::I32Const { value: 12 },
                TassadarInstruction::I32Sub,
                TassadarInstruction::BrIf { target_pc: 8 },
                TassadarInstruction::I32Const { value: 7 },
                TassadarInstruction::Output,
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 12 },
                TassadarInstruction::I32Const { value: 99 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        ),
        expected_trace: vec![
            trace_step(
                0,
                0,
                1,
                TassadarInstruction::I32Const { value: 3 },
                TassadarTraceEvent::ConstPush { value: 3 },
                &[],
                &[3],
                &[],
                &[],
            ),
            trace_step(
                1,
                1,
                2,
                TassadarInstruction::I32Const { value: 4 },
                TassadarTraceEvent::ConstPush { value: 4 },
                &[3],
                &[3, 4],
                &[],
                &[],
            ),
            trace_step(
                2,
                2,
                3,
                TassadarInstruction::I32Mul,
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Mul,
                    left: 3,
                    right: 4,
                    result: 12,
                },
                &[3, 4],
                &[12],
                &[],
                &[],
            ),
            trace_step(
                3,
                3,
                4,
                TassadarInstruction::I32Const { value: 12 },
                TassadarTraceEvent::ConstPush { value: 12 },
                &[12],
                &[12, 12],
                &[],
                &[],
            ),
            trace_step(
                4,
                4,
                5,
                TassadarInstruction::I32Sub,
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Sub,
                    left: 12,
                    right: 12,
                    result: 0,
                },
                &[12, 12],
                &[0],
                &[],
                &[],
            ),
            trace_step(
                5,
                5,
                6,
                TassadarInstruction::BrIf { target_pc: 8 },
                TassadarTraceEvent::Branch {
                    condition: 0,
                    taken: false,
                    target_pc: 8,
                },
                &[0],
                &[],
                &[],
                &[],
            ),
            trace_step(
                6,
                6,
                7,
                TassadarInstruction::I32Const { value: 7 },
                TassadarTraceEvent::ConstPush { value: 7 },
                &[],
                &[7],
                &[],
                &[],
            ),
            trace_step(
                7,
                7,
                8,
                TassadarInstruction::Output,
                TassadarTraceEvent::Output { value: 7 },
                &[7],
                &[],
                &[],
                &[],
            ),
            trace_step(
                8,
                8,
                9,
                TassadarInstruction::I32Const { value: 1 },
                TassadarTraceEvent::ConstPush { value: 1 },
                &[],
                &[1],
                &[],
                &[],
            ),
            trace_step(
                9,
                9,
                12,
                TassadarInstruction::BrIf { target_pc: 12 },
                TassadarTraceEvent::Branch {
                    condition: 1,
                    taken: true,
                    target_pc: 12,
                },
                &[1],
                &[],
                &[],
                &[],
            ),
            trace_step(
                10,
                12,
                13,
                TassadarInstruction::Return,
                TassadarTraceEvent::Return,
                &[],
                &[],
                &[],
                &[],
            ),
        ],
        expected_outputs: vec![7],
    }
}

fn shortest_path_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::core_i32_v1();
    computed_validation_case(
        "shortest_path_two_route",
        "bounded shortest-path witness over two precomputed route costs",
        TassadarProgram::new(
            "tassadar.shortest_path_two_route.v1",
            &profile,
            2,
            0,
            vec![
                TassadarInstruction::I32Const { value: 2 },
                TassadarInstruction::I32Const { value: 5 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 4 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::I32Const { value: 5 },
                TassadarInstruction::I32Sub,
                TassadarInstruction::BrIf { target_pc: 15 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        ),
        vec![5],
    )
}

fn micro_wasm_kernel_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    computed_validation_case(
        "micro_wasm_kernel",
        "unrolled weighted-sum and checksum micro-kernel over memory-backed inputs",
        TassadarProgram::new(
            "tassadar.micro_wasm_kernel.v2",
            &profile,
            2,
            8,
            vec![
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::I32Load { slot: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::I32Mul,
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 0 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::I32Load { slot: 1 },
                TassadarInstruction::I32Const { value: 2 },
                TassadarInstruction::I32Mul,
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 1 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::I32Load { slot: 2 },
                TassadarInstruction::I32Const { value: 3 },
                TassadarInstruction::I32Mul,
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 2 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::I32Load { slot: 3 },
                TassadarInstruction::I32Const { value: 4 },
                TassadarInstruction::I32Mul,
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 3 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::Output,
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        )
        .with_initial_memory(vec![2, 3, 4, 5, 0, 0, 0, 0]),
        vec![40, 14],
    )
}

fn branch_heavy_kernel_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    computed_validation_case(
        "branch_heavy_kernel",
        "acyclic branch ladder over memory-backed flags that exercises repeated forward control-flow pivots",
        TassadarProgram::new(
            "tassadar.branch_heavy_kernel.v1",
            &profile,
            1,
            6,
            vec![
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 0 },
                TassadarInstruction::BrIf { target_pc: 10 },
                TassadarInstruction::I32Const { value: 11 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 14 },
                TassadarInstruction::I32Const { value: 7 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 1 },
                TassadarInstruction::BrIf { target_pc: 22 },
                TassadarInstruction::I32Const { value: 17 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 26 },
                TassadarInstruction::I32Const { value: 13 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 2 },
                TassadarInstruction::BrIf { target_pc: 34 },
                TassadarInstruction::I32Const { value: 23 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 38 },
                TassadarInstruction::I32Const { value: 19 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 3 },
                TassadarInstruction::BrIf { target_pc: 46 },
                TassadarInstruction::I32Const { value: 31 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 50 },
                TassadarInstruction::I32Const { value: 29 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 4 },
                TassadarInstruction::BrIf { target_pc: 58 },
                TassadarInstruction::I32Const { value: 41 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 62 },
                TassadarInstruction::I32Const { value: 37 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 5 },
                TassadarInstruction::BrIf { target_pc: 70 },
                TassadarInstruction::I32Const { value: 47 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 74 },
                TassadarInstruction::I32Const { value: 43 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        )
        .with_initial_memory(vec![1, 0, 1, 1, 0, 1]),
        vec![156],
    )
}

fn memory_heavy_kernel_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    computed_validation_case(
        "memory_heavy_kernel",
        "dense memory-read and memory-write kernel that folds adjacent slots into staged accumulator buffers",
        TassadarProgram::new(
            "tassadar.memory_heavy_kernel.v1",
            &profile,
            1,
            12,
            vec![
                TassadarInstruction::I32Load { slot: 0 },
                TassadarInstruction::I32Load { slot: 1 },
                TassadarInstruction::I32Add,
                TassadarInstruction::I32Store { slot: 8 },
                TassadarInstruction::I32Load { slot: 2 },
                TassadarInstruction::I32Load { slot: 3 },
                TassadarInstruction::I32Add,
                TassadarInstruction::I32Store { slot: 9 },
                TassadarInstruction::I32Load { slot: 4 },
                TassadarInstruction::I32Load { slot: 5 },
                TassadarInstruction::I32Add,
                TassadarInstruction::I32Store { slot: 10 },
                TassadarInstruction::I32Load { slot: 6 },
                TassadarInstruction::I32Load { slot: 7 },
                TassadarInstruction::I32Add,
                TassadarInstruction::I32Store { slot: 11 },
                TassadarInstruction::I32Load { slot: 8 },
                TassadarInstruction::I32Load { slot: 9 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 10 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 11 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        )
        .with_initial_memory(vec![2, 3, 5, 7, 11, 13, 17, 19, 0, 0, 0, 0]),
        vec![77],
    )
}

fn long_loop_kernel_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    computed_validation_case(
        "long_loop_kernel",
        "longer-horizon decrement loop that keeps exact backward-branch behavior explicit under the current article-shaped profile",
        TassadarProgram::new(
            "tassadar.long_loop_kernel.v1",
            &profile,
            1,
            0,
            vec![
                TassadarInstruction::I32Const { value: 2_047 },
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::BrIf { target_pc: 7 },
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::I32Sub,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 2 },
            ],
        ),
        vec![0],
    )
}

fn hungarian_matching_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    computed_validation_case(
        "hungarian_matching",
        "tiny fixed 2x2 matching instance with comparison-selected winning assignment and exact cost",
        TassadarProgram::new(
            "tassadar.hungarian_matching.v2",
            &profile,
            2,
            4,
            vec![
                TassadarInstruction::I32Load { slot: 0 },
                TassadarInstruction::I32Load { slot: 3 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Load { slot: 1 },
                TassadarInstruction::I32Load { slot: 2 },
                TassadarInstruction::I32Add,
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::I32Lt,
                TassadarInstruction::BrIf { target_pc: 17 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::Output,
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::Output,
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        )
        .with_initial_memory(vec![3, 1, 2, 4]),
        vec![3, 1],
    )
}

fn trace_step(
    step_index: usize,
    pc: usize,
    next_pc: usize,
    instruction: TassadarInstruction,
    event: TassadarTraceEvent,
    stack_before: &[i32],
    stack_after: &[i32],
    locals_after: &[i32],
    memory_after: &[i32],
) -> TassadarTraceStep {
    TassadarTraceStep {
        step_index,
        pc,
        next_pc,
        instruction,
        event,
        stack_before: stack_before.to_vec(),
        stack_after: stack_after.to_vec(),
        locals_after: locals_after.to_vec(),
        memory_after: memory_after.to_vec(),
    }
}

fn observed_rule_signature(
    instruction: &TassadarInstruction,
    event: &TassadarTraceEvent,
) -> TassadarOpcodeRule {
    match (instruction, event) {
        (TassadarInstruction::Nop, TassadarTraceEvent::Nop) => TassadarOpcodeRule::new(
            TassadarOpcode::Nop,
            0,
            0,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        ),
        (TassadarInstruction::I32Const { .. }, TassadarTraceEvent::ConstPush { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::I32Const,
                0,
                1,
                TassadarImmediateKind::I32,
                TassadarAccessClass::None,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::LocalGet { .. }, TassadarTraceEvent::LocalGet { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::LocalGet,
                0,
                1,
                TassadarImmediateKind::LocalIndex,
                TassadarAccessClass::LocalRead,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::LocalSet { .. }, TassadarTraceEvent::LocalSet { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::LocalSet,
                1,
                0,
                TassadarImmediateKind::LocalIndex,
                TassadarAccessClass::LocalWrite,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::LocalTee { .. }, TassadarTraceEvent::LocalTee { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::LocalTee,
                0,
                0,
                TassadarImmediateKind::LocalIndex,
                TassadarAccessClass::LocalWrite,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::Drop, TassadarTraceEvent::Drop { .. }) => TassadarOpcodeRule::new(
            TassadarOpcode::Drop,
            1,
            0,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        ),
        (
            TassadarInstruction::I32Add,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Add,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Add,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Add),
        (
            TassadarInstruction::I32Sub,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Sub,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Sub,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Sub),
        (
            TassadarInstruction::I32Mul,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Mul,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Mul,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Mul),
        (
            TassadarInstruction::I32Lt,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Lt,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Lt,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Lt),
        (
            TassadarInstruction::I32Eqz,
            TassadarTraceEvent::UnaryOp {
                op: TassadarUnaryOp::Eqz,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Eqz,
            1,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        ),
        (
            TassadarInstruction::I32Eq,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Eq,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Eq,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Eq),
        (
            TassadarInstruction::I32Ne,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Ne,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Ne,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Ne),
        (
            TassadarInstruction::I32Gt,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Gt,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Gt,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Gt),
        (
            TassadarInstruction::I32Le,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Le,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Le,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Le),
        (
            TassadarInstruction::I32Ge,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Ge,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Ge,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Ge),
        (TassadarInstruction::I32Load { .. }, TassadarTraceEvent::Load { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::I32Load,
                0,
                1,
                TassadarImmediateKind::MemorySlot,
                TassadarAccessClass::MemoryRead,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::I32Store { .. }, TassadarTraceEvent::Store { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::I32Store,
                1,
                0,
                TassadarImmediateKind::MemorySlot,
                TassadarAccessClass::MemoryWrite,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::BrIf { .. }, TassadarTraceEvent::Branch { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::BrIf,
                1,
                0,
                TassadarImmediateKind::BranchTarget,
                TassadarAccessClass::None,
                TassadarControlClass::ConditionalBranch,
            )
        }
        (TassadarInstruction::Output, TassadarTraceEvent::Output { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::Output,
                1,
                0,
                TassadarImmediateKind::None,
                TassadarAccessClass::None,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::Return, TassadarTraceEvent::Return) => TassadarOpcodeRule::new(
            TassadarOpcode::Return,
            0,
            0,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Return,
        ),
        _ => TassadarOpcodeRule::new(
            instruction.opcode(),
            u8::MAX,
            u8::MAX,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        ),
    }
}

fn stable_serialized_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn stable_bytes_digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use crate::TassadarClaimClass;

    use super::{
        TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF, TASSADAR_C_TO_WASM_COMPILE_RECEIPT_REF,
        TASSADAR_CANONICAL_C_PROGRAM_ARTIFACT_ID, TASSADAR_CANONICAL_C_SOURCE_REF,
        TASSADAR_FIXTURE_RUNNER_ID, TASSADAR_LONG_HORIZON_TRACE_EVIDENCE_BUNDLE_FILE,
        TASSADAR_LONG_HORIZON_TRACE_FIXTURE_ROOT_REF, TASSADAR_MEMORY_LOOKUP_RUST_SOURCE_REF,
        TASSADAR_MILLION_STEP_BENCHMARK_BUNDLE_FILE, TASSADAR_MILLION_STEP_BENCHMARK_ROOT_REF,
        TASSADAR_MULTI_EXPORT_RUST_SOURCE_REF, TASSADAR_PARAM_ABI_RUST_SOURCE_REF,
        TASSADAR_RUNTIME_BACKEND_ID, TASSADAR_TRACE_ABI_DECISION_REPORT_REF,
        TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF, TassadarCToWasmCompileConfig,
        TassadarCToWasmCompileReceipt, TassadarCompileRefusal, TassadarCompilerToolchainIdentity,
        TassadarCpuReferenceRunner, TassadarExactnessPosture, TassadarExactnessRefusalReport,
        TassadarExecutionRefusal, TassadarExecutorDecodeMode, TassadarExecutorSelectionReason,
        TassadarExecutorSelectionState, TassadarFixtureRunner, TassadarHullCacheRunner,
        TassadarInstruction, TassadarMillionStepDecodeBenchmarkBundle,
        TassadarMillionStepMeasurementPosture, TassadarMismatchClass, TassadarProgram,
        TassadarProgramArtifact, TassadarProgramArtifactError, TassadarProgramSourceIdentity,
        TassadarProgramSourceKind, TassadarRustToWasmCompileConfig, TassadarSparseTopKRunner,
        TassadarSudokuV0CorpusSplit, TassadarTraceAbi, TassadarTraceAbiDecisionReport,
        TassadarTraceArtifact, TassadarTraceDiffKind, TassadarTraceDiffReport, TassadarTraceEvent,
        TassadarWasmInstructionCoverageReport, TassadarWasmProfile, TassadarWasmProfileId,
        build_tassadar_execution_evidence_bundle,
        build_tassadar_long_horizon_trace_evidence_bundle,
        build_tassadar_million_step_decode_benchmark_bundle,
        build_tassadar_trace_abi_decision_report, compile_tassadar_c_source_to_wasm_receipt,
        compile_tassadar_rust_source_to_wasm_receipt, diagnose_tassadar_executor_request,
        execute_tassadar_executor_request, replay_tassadar_execution,
        run_tassadar_exact_equivalence, run_tassadar_exact_parity, stable_bytes_digest,
        summarize_tassadar_wasm_binary, tassadar_article_class_corpus,
        tassadar_canonical_c_source_path, tassadar_canonical_wasm_binary_path,
        tassadar_memory_lookup_rust_source_path, tassadar_multi_export_rust_source_path,
        tassadar_param_abi_rust_source_path, tassadar_program_artifact_from_compile_receipt,
        tassadar_runtime_capability_report, tassadar_sudoku_9x9_corpus,
        tassadar_sudoku_9x9_search_program, tassadar_sudoku_v0_corpus,
        tassadar_sudoku_v0_search_program, tassadar_supported_wasm_profiles,
        tassadar_validation_corpus, tassadar_wasm_instruction_coverage_report,
        write_tassadar_c_to_wasm_compile_receipt,
        write_tassadar_million_step_decode_benchmark_bundle,
        write_tassadar_trace_abi_decision_artifacts,
        write_tassadar_wasm_instruction_coverage_report,
    };

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        relative_path: &str,
        artifact_kind: &str,
    ) -> T {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(relative_path);
        let bytes = std::fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "failed to read {artifact_kind} `{}`: {error}",
                path.display()
            )
        });
        serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            panic!(
                "failed to deserialize {artifact_kind} `{}`: {error}",
                path.display()
            )
        })
    }

    fn temp_test_dir(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "psionic-tassadar-runtime-test-{suffix}-{}",
            std::process::id()
        ))
    }

    fn million_step_benchmark_bundle() -> &'static TassadarMillionStepDecodeBenchmarkBundle {
        static BUNDLE: OnceLock<TassadarMillionStepDecodeBenchmarkBundle> = OnceLock::new();
        BUNDLE.get_or_init(|| {
            build_tassadar_million_step_decode_benchmark_bundle()
                .expect("million-step benchmark bundle should build")
        })
    }

    fn normalized_million_step_bundle_value(
        bundle: &TassadarMillionStepDecodeBenchmarkBundle,
    ) -> serde_json::Value {
        let mut value = serde_json::to_value(bundle).expect("bundle should serialize");
        value["bundle_digest"] = serde_json::Value::Null;
        value["cpu_reference_steps_per_second"] = serde_json::Value::Null;
        value["reference_linear"]["steps_per_second"] = serde_json::Value::Null;
        value["reference_linear"]["remaining_gap_vs_cpu_reference"] = serde_json::Value::Null;
        value
    }

    #[test]
    fn cpu_reference_runner_matches_exact_trace_fixtures() {
        let runner = TassadarCpuReferenceRunner::new();
        for case in tassadar_validation_corpus() {
            let execution = runner.execute(&case.program).expect("case should run");
            assert_eq!(
                execution.steps, case.expected_trace,
                "case={}",
                case.case_id
            );
            assert_eq!(
                execution.outputs, case.expected_outputs,
                "case={}",
                case.case_id
            );
        }
    }

    #[test]
    fn fixture_runner_matches_exact_trace_fixtures() {
        let runner = TassadarFixtureRunner::new();
        for case in tassadar_validation_corpus() {
            let execution = runner.execute(&case.program).expect("case should run");
            assert_eq!(
                execution.steps, case.expected_trace,
                "case={}",
                case.case_id
            );
            assert_eq!(
                execution.outputs, case.expected_outputs,
                "case={}",
                case.case_id
            );
        }
    }

    #[test]
    fn hull_cache_runner_matches_exact_trace_fixtures() {
        let runner = TassadarHullCacheRunner::new();
        for case in tassadar_validation_corpus() {
            let execution = runner.execute(&case.program).expect("case should run");
            assert_eq!(
                execution.steps, case.expected_trace,
                "case={}",
                case.case_id
            );
            assert_eq!(
                execution.outputs, case.expected_outputs,
                "case={}",
                case.case_id
            );
        }
    }

    #[test]
    fn parity_harness_is_exact_on_validation_corpus() {
        for case in tassadar_validation_corpus() {
            let report = run_tassadar_exact_parity(&case.program).expect("parity should hold");
            report.require_exact().expect("report should be exact");
        }
    }

    #[test]
    fn exact_equivalence_holds_on_validation_corpus() {
        for case in tassadar_validation_corpus() {
            let report =
                run_tassadar_exact_equivalence(&case.program).expect("equivalence should hold");
            report.require_exact().expect("report should be exact");
            assert!(report.trace_digest_equal(), "case={}", case.case_id);
            assert!(report.outputs_equal(), "case={}", case.case_id);
            assert!(report.halt_equal(), "case={}", case.case_id);
        }
    }

    #[test]
    fn exact_equivalence_holds_on_article_class_corpus() {
        for case in tassadar_article_class_corpus() {
            let selection = diagnose_tassadar_executor_request(
                &case.program,
                TassadarExecutorDecodeMode::HullCache,
                TassadarTraceAbi::article_i32_compute_v1().schema_version,
                Some(&[
                    TassadarExecutorDecodeMode::ReferenceLinear,
                    TassadarExecutorDecodeMode::HullCache,
                ]),
            );
            assert_eq!(
                selection.selection_state,
                TassadarExecutorSelectionState::Direct,
                "case={}",
                case.case_id
            );
            assert_eq!(
                selection.effective_decode_mode,
                Some(TassadarExecutorDecodeMode::HullCache),
                "case={}",
                case.case_id
            );
            let report =
                run_tassadar_exact_equivalence(&case.program).expect("equivalence should hold");
            report.require_exact().expect("report should be exact");
            assert!(report.trace_digest_equal(), "case={}", case.case_id);
            assert!(report.outputs_equal(), "case={}", case.case_id);
            assert!(report.halt_equal(), "case={}", case.case_id);
        }
    }

    #[test]
    fn tassadar_runtime_capability_report_declares_executor_truth() {
        let capability = tassadar_runtime_capability_report();
        assert_eq!(capability.runtime_backend, TASSADAR_RUNTIME_BACKEND_ID);
        assert!(capability.supports_executor_trace);
        assert!(capability.supports_hull_decode);
        assert_eq!(
            capability.supported_wasm_profiles,
            tassadar_supported_wasm_profiles()
                .into_iter()
                .map(|profile| profile.profile_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(capability.validated_trace_abi_versions, vec![1]);
        assert_eq!(
            capability.supported_decode_modes,
            vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
                TassadarExecutorDecodeMode::SparseTopK,
            ]
        );
        assert!(capability.supports_sparse_top_k_decode);
    }

    #[test]
    fn article_class_profiles_resolve_to_runtime_builders() {
        for case in tassadar_article_class_corpus() {
            assert_eq!(
                case.program.profile_id,
                TassadarWasmProfileId::ArticleI32ComputeV1.as_str()
            );
            TassadarCpuReferenceRunner::for_program(&case.program)
                .expect("article-class CPU runner should resolve");
            TassadarFixtureRunner::for_program(&case.program)
                .expect("article-class fixture runner should resolve");
            TassadarHullCacheRunner::for_program(&case.program)
                .expect("article-class hull runner should resolve");
            TassadarSparseTopKRunner::for_program(&case.program)
                .expect("article-class sparse runner should resolve");
        }
    }

    #[test]
    fn tassadar_wasm_instruction_coverage_report_matches_committed_truth() {
        let report = tassadar_wasm_instruction_coverage_report();
        assert_eq!(
            report.article_profile_id,
            TassadarWasmProfileId::ArticleI32ComputeV1.as_str()
        );
        assert_eq!(
            report.profiles.len(),
            tassadar_supported_wasm_profiles().len()
        );
        assert!(report.profiles.iter().any(|profile| {
            profile.profile_id == TassadarWasmProfileId::ArticleI32ComputeV1.as_str()
                && profile.article_profile
                && profile
                    .current_case_ids
                    .contains(&String::from("hungarian_matching"))
                && profile
                    .current_case_ids
                    .contains(&String::from("micro_wasm_kernel"))
        }));
        assert!(report.refusal_examples.iter().all(|example| matches!(
            example.refusal,
            TassadarExecutionRefusal::UnsupportedOpcode { .. }
        )));

        let persisted: TassadarWasmInstructionCoverageReport =
            read_repo_json::<TassadarWasmInstructionCoverageReport>(
                TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF,
                "tassadar_wasm_instruction_coverage_report",
            );
        assert_eq!(persisted, report);
    }

    #[test]
    fn write_tassadar_wasm_instruction_coverage_report_persists_current_truth() {
        let temp_dir = std::env::temp_dir().join(format!(
            "psionic-tassadar-runtime-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should create");
        let report_path = temp_dir.join("tassadar_wasm_instruction_coverage_report.json");
        let report = write_tassadar_wasm_instruction_coverage_report(&report_path)
            .expect("coverage report should write");
        let bytes = std::fs::read(&report_path).expect("coverage report should persist");
        let persisted: TassadarWasmInstructionCoverageReport =
            serde_json::from_slice(&bytes).expect("persisted report should deserialize");
        assert_eq!(persisted, report);
        std::fs::remove_file(&report_path).expect("temp report should be removable");
        std::fs::remove_dir(&temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn tassadar_trace_abi_decision_report_matches_committed_truth() {
        let report = build_tassadar_trace_abi_decision_report().expect("trace-ABI decision report");
        assert_eq!(
            report.canonical_trace_abi,
            TassadarTraceAbi::article_i32_compute_v1()
        );
        assert_eq!(
            report.long_horizon_fixture.trace_abi_id,
            report.canonical_trace_abi.abi_id
        );
        assert_eq!(
            report.long_horizon_fixture.trace_abi_version,
            report.canonical_trace_abi.schema_version
        );
        assert!(report.long_horizon_fixture.step_count >= 16_000);
        let persisted: TassadarTraceAbiDecisionReport =
            read_repo_json::<TassadarTraceAbiDecisionReport>(
                TASSADAR_TRACE_ABI_DECISION_REPORT_REF,
                "tassadar_trace_abi_decision_report",
            );
        assert_eq!(persisted, report);
    }

    #[test]
    fn long_horizon_trace_fixture_records_abi_identity() {
        let evidence_bundle = build_tassadar_long_horizon_trace_evidence_bundle()
            .expect("long-horizon trace evidence bundle");
        assert_eq!(
            evidence_bundle.trace_artifact.trace_abi_id,
            TassadarTraceAbi::article_i32_compute_v1().abi_id
        );
        assert_eq!(
            evidence_bundle.trace_artifact.trace_abi_version,
            TassadarTraceAbi::article_i32_compute_v1().schema_version
        );
        assert_eq!(evidence_bundle.trace_artifact.step_count, 16_383);

        let persisted: serde_json::Value = read_repo_json::<serde_json::Value>(
            &format!(
                "{}/{}",
                TASSADAR_LONG_HORIZON_TRACE_FIXTURE_ROOT_REF,
                TASSADAR_LONG_HORIZON_TRACE_EVIDENCE_BUNDLE_FILE
            ),
            "tassadar_long_horizon_trace_evidence_bundle",
        );
        assert_eq!(
            persisted["trace_artifact"]["trace_abi_id"].as_str(),
            Some(TassadarTraceAbi::article_i32_compute_v1().abi_id.as_str())
        );
        assert_eq!(
            persisted["trace_artifact"]["trace_abi_version"].as_u64(),
            Some(u64::from(
                TassadarTraceAbi::article_i32_compute_v1().schema_version
            ))
        );
    }

    #[test]
    fn write_tassadar_trace_abi_decision_artifacts_persist_current_truth() {
        let temp_dir = temp_test_dir("trace-abi");
        std::fs::create_dir_all(&temp_dir).expect("temp dir should create");
        let report_path = temp_dir.join("tassadar_trace_abi_decision_report.json");
        let fixture_root = temp_dir.join("long_horizon_fixture");
        let report = write_tassadar_trace_abi_decision_artifacts(&report_path, &fixture_root)
            .expect("trace-ABI decision artifacts should write");
        let report_bytes = std::fs::read(&report_path).expect("persisted report");
        let persisted_report: TassadarTraceAbiDecisionReport =
            serde_json::from_slice(&report_bytes).expect("persisted report should deserialize");
        assert_eq!(persisted_report, report);

        let evidence_path = fixture_root.join(TASSADAR_LONG_HORIZON_TRACE_EVIDENCE_BUNDLE_FILE);
        let evidence_bytes = std::fs::read(&evidence_path).expect("persisted evidence bundle");
        let persisted_evidence: serde_json::Value =
            serde_json::from_slice(&evidence_bytes).expect("persisted evidence bundle json");
        assert_eq!(
            persisted_evidence["trace_artifact"]["trace_abi_id"].as_str(),
            Some(TassadarTraceAbi::article_i32_compute_v1().abi_id.as_str())
        );
        assert_eq!(
            persisted_evidence["trace_artifact"]["trace_abi_version"].as_u64(),
            Some(u64::from(
                TassadarTraceAbi::article_i32_compute_v1().schema_version
            ))
        );

        std::fs::remove_file(&report_path).expect("temp report should be removable");
        std::fs::remove_file(&evidence_path).expect("temp evidence bundle should be removable");
        std::fs::remove_dir(&fixture_root).expect("fixture root should be removable");
        std::fs::remove_dir(&temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn million_step_decode_benchmark_bundle_records_expected_truth() {
        let bundle = million_step_benchmark_bundle();
        assert_eq!(bundle.schema_version, 1);
        assert_eq!(
            bundle.bundle_root_ref,
            TASSADAR_MILLION_STEP_BENCHMARK_ROOT_REF
        );
        assert_eq!(bundle.iteration_count, 131_071);
        assert_eq!(bundle.expected_step_count, 1_048_575);
        assert_eq!(bundle.cpu_reference_summary.step_count, 1_048_575);
        assert_eq!(bundle.cpu_reference_summary.outputs, vec![0]);
        assert_eq!(bundle.cpu_reference_summary.final_locals, vec![0]);
        assert_eq!(bundle.cpu_reference_summary.final_memory, Vec::<i32>::new());
        assert_eq!(bundle.cpu_reference_summary.final_stack, Vec::<i32>::new());
        assert_eq!(bundle.exactness_bps, 10_000);
        assert_eq!(
            bundle.memory_growth_metric,
            "serialized_trace_step_stream_bytes"
        );
        assert_eq!(
            bundle.memory_growth_bytes,
            bundle.cpu_reference_summary.serialized_trace_bytes
        );
        assert!(!bundle.runtime_manifest_identity_digest.is_empty());
        assert!(bundle.cpu_reference_steps_per_second > 0.0);

        assert_eq!(
            bundle.reference_linear.selection_state,
            TassadarExecutorSelectionState::Direct
        );
        assert_eq!(
            bundle.reference_linear.measurement_posture,
            TassadarMillionStepMeasurementPosture::Measured
        );
        assert_eq!(
            bundle.reference_linear.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::ReferenceLinear)
        );
        assert!(
            bundle
                .reference_linear
                .steps_per_second
                .expect("reference-linear throughput should exist")
                > 0.0
        );
        assert_eq!(
            bundle.reference_linear.remaining_gap_vs_cpu_reference,
            Some(0.0)
        );

        assert_eq!(
            bundle.hull_cache.selection_state,
            TassadarExecutorSelectionState::Fallback
        );
        assert_eq!(
            bundle.hull_cache.selection_reason,
            Some(TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported)
        );
        assert_eq!(
            bundle.hull_cache.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::ReferenceLinear)
        );

        assert_eq!(
            bundle.sparse_top_k.selection_state,
            TassadarExecutorSelectionState::Fallback
        );
        assert_eq!(
            bundle.sparse_top_k.selection_reason,
            Some(TassadarExecutorSelectionReason::SparseTopKValidationUnsupported)
        );
        assert_eq!(
            bundle.sparse_top_k.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::ReferenceLinear)
        );

        assert_eq!(
            bundle.evidence_bundle.runtime_manifest.identity_digest,
            bundle.runtime_manifest_identity_digest
        );
        assert_eq!(
            bundle.evidence_bundle.trace_summary_artifact.step_count,
            bundle.cpu_reference_summary.step_count
        );
    }

    #[test]
    fn million_step_decode_benchmark_bundle_matches_committed_truth() {
        let persisted: TassadarMillionStepDecodeBenchmarkBundle =
            read_repo_json::<TassadarMillionStepDecodeBenchmarkBundle>(
                &format!(
                    "{}/{}",
                    TASSADAR_MILLION_STEP_BENCHMARK_ROOT_REF,
                    TASSADAR_MILLION_STEP_BENCHMARK_BUNDLE_FILE
                ),
                "tassadar_million_step_decode_benchmark_bundle",
            );
        assert!(persisted.cpu_reference_steps_per_second > 0.0);
        assert!(
            persisted
                .reference_linear
                .steps_per_second
                .expect("persisted reference-linear throughput")
                > 0.0
        );
        assert_eq!(
            normalized_million_step_bundle_value(&persisted),
            normalized_million_step_bundle_value(million_step_benchmark_bundle())
        );
    }

    #[test]
    fn write_tassadar_million_step_decode_benchmark_bundle_persists_current_truth() {
        let temp_dir = temp_test_dir("million-step");
        std::fs::create_dir_all(&temp_dir).expect("temp dir should create");
        let bundle = write_tassadar_million_step_decode_benchmark_bundle(&temp_dir)
            .expect("million-step benchmark bundle should write");
        let bundle_path = temp_dir.join(TASSADAR_MILLION_STEP_BENCHMARK_BUNDLE_FILE);
        let bundle_bytes = std::fs::read(&bundle_path).expect("persisted million-step bundle");
        let persisted: TassadarMillionStepDecodeBenchmarkBundle =
            serde_json::from_slice(&bundle_bytes)
                .expect("persisted million-step bundle should deserialize");
        assert!(persisted.cpu_reference_steps_per_second > 0.0);
        assert!(
            persisted
                .reference_linear
                .steps_per_second
                .expect("persisted reference-linear throughput")
                > 0.0
        );
        assert_eq!(
            normalized_million_step_bundle_value(&persisted),
            normalized_million_step_bundle_value(&bundle)
        );

        std::fs::remove_file(&bundle_path).expect("temp bundle should be removable");
        std::fs::remove_dir(&temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn validator_compatibility_artifacts_preserve_trace_abi_identity() {
        let benchmark_report = read_repo_json::<serde_json::Value>(
            TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
            "tassadar_article_class_benchmark_report",
        );
        assert_eq!(
            benchmark_report["suite"]["environment_bundle"]["program_binding"]["trace_abi_id"]
                .as_str(),
            Some(TassadarTraceAbi::article_i32_compute_v1().abi_id.as_str())
        );
        assert_eq!(
            benchmark_report["suite"]["environment_bundle"]["program_binding"]["trace_abi_version"]
                .as_u64(),
            Some(u64::from(
                TassadarTraceAbi::article_i32_compute_v1().schema_version
            ))
        );

        let sudoku_compiled = read_repo_json::<serde_json::Value>(
            "fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0/compiled_weight_suite_artifact.json",
            "sudoku_compiled_weight_suite_artifact",
        );
        let hungarian_compiled = read_repo_json::<serde_json::Value>(
            "fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0/compiled_weight_suite_artifact.json",
            "hungarian_compiled_weight_suite_artifact",
        );
        for compiled in [&sudoku_compiled, &hungarian_compiled] {
            assert_eq!(
                compiled["deployments"][0]["trace_abi_id"].as_str(),
                Some(TassadarTraceAbi::article_i32_compute_v1().abi_id.as_str())
            );
            assert_eq!(
                compiled["deployments"][0]["trace_abi_version"].as_u64(),
                Some(u64::from(
                    TassadarTraceAbi::article_i32_compute_v1().schema_version
                ))
            );
        }
    }

    #[test]
    fn canonical_c_to_wasm_compile_receipt_matches_committed_binary_and_lineage() {
        let receipt = read_repo_json::<TassadarCToWasmCompileReceipt>(
            TASSADAR_C_TO_WASM_COMPILE_RECEIPT_REF,
            "tassadar_c_to_wasm_compile_receipt",
        );
        assert!(receipt.succeeded());
        assert_eq!(
            receipt.source_identity.source_name,
            TASSADAR_CANONICAL_C_SOURCE_REF
        );
        let source_bytes =
            std::fs::read(tassadar_canonical_c_source_path()).expect("canonical C source");
        assert_eq!(
            receipt.source_identity.source_digest,
            stable_bytes_digest(&source_bytes)
        );
        let wasm_bytes =
            std::fs::read(tassadar_canonical_wasm_binary_path()).expect("canonical Wasm binary");
        let wasm_binary_digest = stable_bytes_digest(&wasm_bytes);
        assert_eq!(
            receipt.wasm_binary_digest(),
            Some(wasm_binary_digest.as_str())
        );

        let validated_program = tassadar_article_class_corpus()
            .into_iter()
            .find(|case| case.case_id == "micro_wasm_kernel")
            .expect("canonical article-class micro kernel")
            .program;
        let artifact = tassadar_program_artifact_from_compile_receipt(
            &receipt,
            TASSADAR_CANONICAL_C_PROGRAM_ARTIFACT_ID,
            &TassadarWasmProfile::article_i32_compute_v1(),
            &TassadarTraceAbi::article_i32_compute_v1(),
            validated_program,
        )
        .expect("successful compile receipt should project into program artifact");
        let lineage = receipt
            .lineage_contract()
            .expect("successful compile receipt should carry lineage");
        assert_eq!(lineage.artifact_digest, artifact.artifact_digest);
        assert_eq!(
            lineage.validated_program_digest,
            artifact.validated_program_digest
        );
    }

    #[test]
    fn canonical_wasm_binary_summary_is_machine_legible() {
        let wasm_bytes =
            std::fs::read(tassadar_canonical_wasm_binary_path()).expect("canonical Wasm binary");
        let summary = summarize_tassadar_wasm_binary(&wasm_bytes)
            .expect("canonical Wasm summary should parse");
        assert_eq!(summary.byte_len, wasm_bytes.len());
        assert!(summary.function_count >= 1);
        assert!(
            summary
                .exported_functions
                .contains(&String::from("micro_wasm_kernel"))
        );
    }

    #[test]
    fn canonical_c_to_wasm_compile_receipt_rebuild_is_stable() {
        let source_bytes =
            std::fs::read(tassadar_canonical_c_source_path()).expect("canonical C source");
        let compile_config = TassadarCToWasmCompileConfig::canonical_micro_wasm_kernel();
        let temp_dir = temp_test_dir("c-to-wasm-stable");
        std::fs::create_dir_all(&temp_dir).expect("temp dir should create");
        let receipt_a = compile_tassadar_c_source_to_wasm_receipt(
            TASSADAR_CANONICAL_C_SOURCE_REF,
            &source_bytes,
            temp_dir.join("a.wasm"),
            &compile_config,
        );
        let receipt_b = compile_tassadar_c_source_to_wasm_receipt(
            TASSADAR_CANONICAL_C_SOURCE_REF,
            &source_bytes,
            temp_dir.join("b.wasm"),
            &compile_config,
        );
        assert_eq!(receipt_a.source_identity, receipt_b.source_identity);
        assert_eq!(receipt_a.toolchain_identity, receipt_b.toolchain_identity);
        assert_eq!(receipt_a.compile_config, receipt_b.compile_config);
        if receipt_a.succeeded() && receipt_b.succeeded() {
            assert_eq!(
                receipt_a.wasm_binary_digest(),
                receipt_b.wasm_binary_digest()
            );
            assert_eq!(receipt_a.lineage_contract(), receipt_b.lineage_contract());
        } else {
            assert_eq!(receipt_a.refusal(), receipt_b.refusal());
        }
        for path in [temp_dir.join("a.wasm"), temp_dir.join("b.wasm")] {
            if path.exists() {
                std::fs::remove_file(path).expect("temp wasm should be removable");
            }
        }
        std::fs::remove_dir(&temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn multi_export_rust_to_wasm_compile_receipt_rebuild_is_stable() {
        let source_bytes = std::fs::read(tassadar_multi_export_rust_source_path())
            .expect("multi-export Rust source");
        let compile_config = TassadarRustToWasmCompileConfig::canonical_multi_export_kernel();
        let temp_dir = temp_test_dir("rust-to-wasm-multi-export");
        std::fs::create_dir_all(&temp_dir).expect("temp dir should create");
        let receipt_a = compile_tassadar_rust_source_to_wasm_receipt(
            TASSADAR_MULTI_EXPORT_RUST_SOURCE_REF,
            &source_bytes,
            temp_dir.join("a.wasm"),
            &compile_config,
        );
        let receipt_b = compile_tassadar_rust_source_to_wasm_receipt(
            TASSADAR_MULTI_EXPORT_RUST_SOURCE_REF,
            &source_bytes,
            temp_dir.join("b.wasm"),
            &compile_config,
        );
        assert_eq!(receipt_a.source_identity, receipt_b.source_identity);
        assert_eq!(receipt_a.toolchain_identity, receipt_b.toolchain_identity);
        assert_eq!(receipt_a.compile_config, receipt_b.compile_config);
        if receipt_a.succeeded() && receipt_b.succeeded() {
            assert_eq!(
                receipt_a.wasm_binary_digest(),
                receipt_b.wasm_binary_digest()
            );
            assert_eq!(
                receipt_a
                    .wasm_binary_summary()
                    .expect("successful compile should carry a summary")
                    .exported_functions,
                vec![String::from("local_double"), String::from("pair_sum")]
            );
        } else {
            assert_eq!(receipt_a.refusal(), receipt_b.refusal());
        }
        for path in [temp_dir.join("a.wasm"), temp_dir.join("b.wasm")] {
            if path.exists() {
                std::fs::remove_file(path).expect("temp wasm should be removable");
            }
        }
        std::fs::remove_dir(&temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn memory_lookup_rust_to_wasm_compile_receipt_rebuild_is_stable() {
        let source_bytes = std::fs::read(tassadar_memory_lookup_rust_source_path())
            .expect("memory-lookup Rust source");
        let compile_config = TassadarRustToWasmCompileConfig::canonical_memory_lookup_kernel();
        let temp_dir = temp_test_dir("rust-to-wasm-memory-lookup");
        std::fs::create_dir_all(&temp_dir).expect("temp dir should create");
        let receipt_a = compile_tassadar_rust_source_to_wasm_receipt(
            TASSADAR_MEMORY_LOOKUP_RUST_SOURCE_REF,
            &source_bytes,
            temp_dir.join("a.wasm"),
            &compile_config,
        );
        let receipt_b = compile_tassadar_rust_source_to_wasm_receipt(
            TASSADAR_MEMORY_LOOKUP_RUST_SOURCE_REF,
            &source_bytes,
            temp_dir.join("b.wasm"),
            &compile_config,
        );
        assert_eq!(receipt_a.source_identity, receipt_b.source_identity);
        assert_eq!(receipt_a.toolchain_identity, receipt_b.toolchain_identity);
        assert_eq!(receipt_a.compile_config, receipt_b.compile_config);
        if receipt_a.succeeded() && receipt_b.succeeded() {
            assert_eq!(
                receipt_a.wasm_binary_digest(),
                receipt_b.wasm_binary_digest()
            );
            assert_eq!(
                receipt_a
                    .wasm_binary_summary()
                    .expect("successful compile should carry a summary")
                    .memory_count,
                1
            );
        } else {
            assert_eq!(receipt_a.refusal(), receipt_b.refusal());
        }
        for path in [temp_dir.join("a.wasm"), temp_dir.join("b.wasm")] {
            if path.exists() {
                std::fs::remove_file(path).expect("temp wasm should be removable");
            }
        }
        std::fs::remove_dir(&temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn c_to_wasm_compile_receipt_surfaces_machine_readable_refusal() {
        let mut compile_config = TassadarCToWasmCompileConfig::canonical_micro_wasm_kernel();
        compile_config.compiler_binary = String::from("missing-clang-for-tassadar");
        let receipt = compile_tassadar_c_source_to_wasm_receipt(
            "fixtures/tassadar/sources/invalid.c",
            b"int not_reached(void) { return 0; }",
            temp_test_dir("c-to-wasm-refusal").join("bad.wasm"),
            &compile_config,
        );
        assert_eq!(
            receipt.refusal(),
            Some(&TassadarCompileRefusal::ToolchainUnavailable {
                binary: String::from("missing-clang-for-tassadar"),
            })
        );
    }

    #[test]
    fn param_abi_rust_compile_receipt_exposes_successful_wasm_before_lowering_refusal() {
        let source_bytes =
            std::fs::read(tassadar_param_abi_rust_source_path()).expect("param-ABI Rust source");
        let receipt = compile_tassadar_rust_source_to_wasm_receipt(
            TASSADAR_PARAM_ABI_RUST_SOURCE_REF,
            &source_bytes,
            temp_test_dir("rust-to-wasm-param-abi").join("param.wasm"),
            &TassadarRustToWasmCompileConfig::canonical_param_abi_kernel(),
        );
        if receipt.succeeded() {
            assert!(
                receipt
                    .wasm_binary_ref()
                    .expect("successful compile should publish an output ref")
                    .ends_with("param.wasm")
            );
            assert_eq!(
                receipt
                    .wasm_binary_summary()
                    .expect("successful compile should publish a Wasm summary")
                    .exported_functions,
                vec![String::from("add_one"), String::from("pair_add")]
            );
        } else {
            assert!(receipt.refusal().is_some());
        }
    }

    #[test]
    fn write_tassadar_c_to_wasm_compile_receipt_persists_current_truth() {
        let temp_dir = temp_test_dir("c-to-wasm-write");
        std::fs::create_dir_all(&temp_dir).expect("temp dir should create");
        let receipt_path = temp_dir.join("tassadar_c_to_wasm_compile_receipt.json");
        let receipt = write_tassadar_c_to_wasm_compile_receipt(&receipt_path)
            .expect("compile receipt should write");
        let bytes = std::fs::read(&receipt_path).expect("compile receipt should persist");
        let persisted: TassadarCToWasmCompileReceipt =
            serde_json::from_slice(&bytes).expect("persisted receipt should deserialize");
        assert_eq!(persisted, receipt);
        std::fs::remove_file(&receipt_path).expect("temp receipt should be removable");
        std::fs::remove_dir(&temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn sudoku_v0_corpus_assigns_stable_train_validation_and_test_splits() {
        let corpus = tassadar_sudoku_v0_corpus();
        assert_eq!(corpus.len(), 8);
        assert_eq!(
            corpus
                .iter()
                .filter(|case| case.split == TassadarSudokuV0CorpusSplit::Train)
                .count(),
            4
        );
        assert_eq!(
            corpus
                .iter()
                .filter(|case| case.split == TassadarSudokuV0CorpusSplit::Validation)
                .count(),
            2
        );
        assert_eq!(
            corpus
                .iter()
                .filter(|case| case.split == TassadarSudokuV0CorpusSplit::Test)
                .count(),
            2
        );
        assert!(
            corpus
                .iter()
                .all(|case| !case.validation_case.expected_trace.is_empty())
        );
        assert!(
            corpus
                .iter()
                .all(|case| case.validation_case.expected_outputs.len() == 16)
        );
    }

    #[test]
    fn sudoku_9x9_corpus_assigns_stable_train_validation_and_test_splits() {
        let corpus = tassadar_sudoku_9x9_corpus();
        assert_eq!(corpus.len(), 4);
        assert_eq!(
            corpus
                .iter()
                .filter(|case| case.split == TassadarSudokuV0CorpusSplit::Train)
                .count(),
            2
        );
        assert_eq!(
            corpus
                .iter()
                .filter(|case| case.split == TassadarSudokuV0CorpusSplit::Validation)
                .count(),
            1
        );
        assert_eq!(
            corpus
                .iter()
                .filter(|case| case.split == TassadarSudokuV0CorpusSplit::Test)
                .count(),
            1
        );
        assert!(
            corpus
                .iter()
                .all(|case| !case.validation_case.expected_trace.is_empty())
        );
        assert!(
            corpus
                .iter()
                .all(|case| case.validation_case.expected_outputs.len() == 81)
        );
    }

    #[test]
    fn article_hard_sudoku_suite_includes_named_arto_and_stand_in_cases() {
        let suite = super::tassadar_article_hard_sudoku_suite();

        assert_eq!(suite.len(), 2);
        assert_eq!(
            suite
                .iter()
                .map(|case| case.case_id.as_str())
                .collect::<Vec<_>>(),
            vec!["sudoku_9x9_test_a", "sudoku_9x9_arto_inkala"]
        );
        assert!(
            suite
                .iter()
                .all(|case| case.split == TassadarSudokuV0CorpusSplit::Test)
        );
        assert!(
            suite
                .iter()
                .all(|case| case.validation_case.expected_outputs.len() == 81)
        );
    }

    #[test]
    fn sudoku_v0_search_profile_resolves_to_runtime_builders() {
        let program = tassadar_sudoku_v0_search_program(
            "tassadar.sudoku_v0.runtime_builder_test",
            [
                0, 1, 0, 0, //
                0, 4, 1, 0, //
                1, 0, 0, 4, //
                0, 3, 0, 0,
            ],
        );
        assert_eq!(
            program.profile_id,
            TassadarWasmProfileId::SudokuV0SearchV1.as_str()
        );
        TassadarCpuReferenceRunner::for_program(&program).expect("Sudoku-v0 CPU runner");
        TassadarFixtureRunner::for_program(&program).expect("Sudoku-v0 fixture runner");
        TassadarHullCacheRunner::for_program(&program).expect("Sudoku-v0 hull runner");
        TassadarSparseTopKRunner::for_program(&program).expect("Sudoku-v0 sparse runner");
    }

    #[test]
    fn sudoku_9x9_search_profile_resolves_to_runtime_builders() {
        let program = tassadar_sudoku_9x9_search_program(
            "tassadar.sudoku_9x9.runtime_builder_test",
            [
                0, 3, 4, 6, 0, 8, 9, 1, 0, //
                6, 0, 2, 0, 9, 5, 0, 4, 8, //
                1, 9, 0, 3, 4, 0, 5, 6, 7, //
                0, 5, 9, 0, 6, 1, 4, 0, 3, //
                4, 2, 0, 8, 0, 3, 0, 9, 1, //
                7, 0, 3, 9, 0, 4, 8, 0, 6, //
                0, 6, 1, 5, 3, 0, 2, 8, 4, //
                2, 0, 7, 4, 0, 9, 6, 3, 0, //
                3, 4, 0, 2, 8, 6, 0, 7, 9,
            ],
        );
        assert_eq!(
            program.profile_id,
            TassadarWasmProfileId::Sudoku9x9SearchV1.as_str()
        );
        TassadarCpuReferenceRunner::for_program(&program).expect("Sudoku-9x9 CPU runner");
        TassadarFixtureRunner::for_program(&program).expect("Sudoku-9x9 fixture runner");
        TassadarHullCacheRunner::for_program(&program).expect("Sudoku-9x9 hull runner");
        TassadarSparseTopKRunner::for_program(&program).expect("Sudoku-9x9 sparse runner");
    }

    #[test]
    fn sudoku_v0_search_program_solves_real_4x4_puzzles_with_looping_search() {
        let puzzles = vec![
            (
                "sudoku_v0_case_a",
                [
                    1, 0, 0, 4, //
                    0, 4, 1, 0, //
                    0, 1, 0, 3, //
                    4, 0, 2, 0,
                ],
                vec![
                    1, 2, 3, 4, //
                    3, 4, 1, 2, //
                    2, 1, 4, 3, //
                    4, 3, 2, 1,
                ],
            ),
            (
                "sudoku_v0_case_b",
                [
                    0, 1, 0, 0, //
                    0, 4, 1, 0, //
                    1, 0, 0, 4, //
                    0, 3, 0, 0,
                ],
                vec![
                    2, 1, 4, 3, //
                    3, 4, 1, 2, //
                    1, 2, 3, 4, //
                    4, 3, 2, 1,
                ],
            ),
            (
                "sudoku_v0_case_c",
                [
                    0, 0, 0, 0, //
                    3, 0, 0, 0, //
                    0, 0, 0, 4, //
                    0, 0, 1, 0,
                ],
                vec![
                    1, 2, 4, 3, //
                    3, 4, 2, 1, //
                    2, 1, 3, 4, //
                    4, 3, 1, 2,
                ],
            ),
        ];

        let mut saw_taken_backward_branch = false;
        for (program_id, puzzle, expected_outputs) in puzzles {
            let program = tassadar_sudoku_v0_search_program(program_id, puzzle);
            let execution = TassadarCpuReferenceRunner::for_program(&program)
                .expect("Sudoku-v0 CPU runner")
                .execute(&program)
                .expect("Sudoku-v0 program should solve the puzzle");
            assert_eq!(execution.outputs, expected_outputs, "program={program_id}");
            if execution.steps.iter().any(|step| {
                matches!(
                    step.event,
                    TassadarTraceEvent::Branch {
                        taken: true,
                        target_pc,
                        ..
                    } if target_pc < step.pc
                )
            }) {
                saw_taken_backward_branch = true;
            }
        }

        assert!(
            saw_taken_backward_branch,
            "expected at least one real 4x4 Sudoku-v0 search case to use a taken backward branch"
        );
    }

    #[test]
    fn sudoku_9x9_search_program_solves_real_9x9_puzzles_with_looping_search() {
        for case in tassadar_sudoku_9x9_corpus() {
            let execution = TassadarCpuReferenceRunner::for_program(&case.validation_case.program)
                .expect("Sudoku-9x9 CPU runner")
                .execute(&case.validation_case.program)
                .expect("Sudoku-9x9 program should solve the puzzle");
            assert_eq!(
                execution.outputs, case.validation_case.expected_outputs,
                "case={}",
                case.case_id
            );
            assert_eq!(execution.outputs.len(), 81, "case={}", case.case_id);
        }
    }

    #[test]
    fn sudoku_9x9_arto_inkala_program_solves_named_hard_case() {
        let case = super::tassadar_sudoku_9x9_arto_inkala_case();
        let trace_abi = super::tassadar_trace_abi_for_profile_id(
            case.validation_case.program.profile_id.as_str(),
        )
        .expect("Sudoku-9x9 Arto trace ABI");
        let profile =
            super::tassadar_wasm_profile_for_id(case.validation_case.program.profile_id.as_str())
                .expect("Sudoku-9x9 Arto profile");
        let summary = super::execute_program_direct_summary(
            &case.validation_case.program,
            &profile,
            &trace_abi,
            super::TASSADAR_CPU_REFERENCE_RUNNER_ID,
        )
        .expect("Sudoku-9x9 Arto program should solve the puzzle");

        assert_eq!(
            case.validation_case.program.profile_id,
            TassadarWasmProfileId::Sudoku9x9HardSearchV1.as_str()
        );
        assert_eq!(summary.outputs, case.validation_case.expected_outputs);
        assert_eq!(summary.outputs.len(), 81);
        assert_eq!(case.split, TassadarSudokuV0CorpusSplit::Test);
        assert!(!case.validation_case.expected_trace.is_empty());
    }

    #[test]
    fn sudoku_v0_search_selection_surfaces_documented_fast_path_fallbacks() {
        let program = tassadar_sudoku_v0_search_program(
            "tassadar.sudoku_v0.fast_path_boundary",
            [
                0, 1, 0, 0, //
                0, 4, 1, 0, //
                1, 0, 0, 4, //
                0, 3, 0, 0,
            ],
        );
        let hull_diagnostic = diagnose_tassadar_executor_request(
            &program,
            TassadarExecutorDecodeMode::HullCache,
            TassadarTraceAbi::sudoku_v0_search_v1().schema_version,
            Some(&[
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
            ]),
        );
        assert!(hull_diagnostic.is_fallback());
        assert_eq!(
            hull_diagnostic.selection_reason,
            Some(TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported)
        );

        let sparse_diagnostic = diagnose_tassadar_executor_request(
            &program,
            TassadarExecutorDecodeMode::SparseTopK,
            TassadarTraceAbi::sudoku_v0_search_v1().schema_version,
            Some(&[
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::SparseTopK,
            ]),
        );
        assert!(sparse_diagnostic.is_fallback());
        assert_eq!(
            sparse_diagnostic.selection_reason,
            Some(TassadarExecutorSelectionReason::SparseTopKValidationUnsupported)
        );
    }

    #[test]
    fn runtime_selection_is_direct_on_validated_hull_cache_workloads() {
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let diagnostic = diagnose_tassadar_executor_request(
            &case.program,
            TassadarExecutorDecodeMode::HullCache,
            TassadarTraceAbi::core_i32_v1().schema_version,
            Some(&[
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
            ]),
        );
        assert_eq!(
            diagnostic.selection_state,
            TassadarExecutorSelectionState::Direct
        );
        assert_eq!(
            diagnostic.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::HullCache)
        );
        assert_eq!(diagnostic.selection_reason, None);
    }

    #[test]
    fn runtime_selection_is_direct_on_article_class_hull_cache_workloads() {
        for case in tassadar_article_class_corpus() {
            let diagnostic = diagnose_tassadar_executor_request(
                &case.program,
                TassadarExecutorDecodeMode::HullCache,
                TassadarTraceAbi::article_i32_compute_v1().schema_version,
                Some(&[
                    TassadarExecutorDecodeMode::ReferenceLinear,
                    TassadarExecutorDecodeMode::HullCache,
                ]),
            );
            assert_eq!(
                diagnostic.selection_state,
                TassadarExecutorSelectionState::Direct,
                "case={}",
                case.case_id
            );
            assert_eq!(
                diagnostic.effective_decode_mode,
                Some(TassadarExecutorDecodeMode::HullCache),
                "case={}",
                case.case_id
            );
            assert_eq!(diagnostic.selection_reason, None, "case={}", case.case_id);
        }
    }

    #[test]
    fn runtime_selection_surfaces_hull_cache_fallback() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let program = TassadarProgram::new(
            "tassadar.backward_branch.v1",
            &profile,
            0,
            0,
            vec![
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::BrIf { target_pc: 0 },
                TassadarInstruction::Return,
            ],
        );
        let diagnostic = diagnose_tassadar_executor_request(
            &program,
            TassadarExecutorDecodeMode::HullCache,
            TassadarTraceAbi::core_i32_v1().schema_version,
            Some(&[
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
            ]),
        );
        assert!(diagnostic.is_fallback());
        assert_eq!(
            diagnostic.selection_reason,
            Some(TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported)
        );
        assert_eq!(
            diagnostic.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::ReferenceLinear)
        );
    }

    #[test]
    fn runtime_selection_is_direct_on_validated_sparse_top_k_workloads() {
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let diagnostic = diagnose_tassadar_executor_request(
            &case.program,
            TassadarExecutorDecodeMode::SparseTopK,
            TassadarTraceAbi::core_i32_v1().schema_version,
            Some(&[
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
                TassadarExecutorDecodeMode::SparseTopK,
            ]),
        );
        assert_eq!(
            diagnostic.selection_state,
            TassadarExecutorSelectionState::Direct
        );
        assert_eq!(
            diagnostic.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::SparseTopK)
        );
        assert_eq!(diagnostic.selection_reason, None);
    }

    #[test]
    fn runtime_selection_surfaces_sparse_top_k_fallback_on_unsupported_shape() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let program = TassadarProgram::new(
            "tassadar.sparse_top_k.backward_branch.v1",
            &profile,
            0,
            0,
            vec![
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::BrIf { target_pc: 0 },
                TassadarInstruction::Return,
            ],
        );
        let diagnostic = diagnose_tassadar_executor_request(
            &program,
            TassadarExecutorDecodeMode::SparseTopK,
            TassadarTraceAbi::core_i32_v1().schema_version,
            Some(&[
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
                TassadarExecutorDecodeMode::SparseTopK,
            ]),
        );
        assert!(diagnostic.is_fallback());
        assert_eq!(
            diagnostic.selection_reason,
            Some(TassadarExecutorSelectionReason::SparseTopKValidationUnsupported)
        );
        assert_eq!(
            diagnostic.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::ReferenceLinear)
        );
    }

    #[test]
    fn runtime_selection_refuses_unsupported_trace_abi() {
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let diagnostic = diagnose_tassadar_executor_request(
            &case.program,
            TassadarExecutorDecodeMode::ReferenceLinear,
            99,
            Some(&[TassadarExecutorDecodeMode::ReferenceLinear]),
        );
        assert!(diagnostic.is_refused());
        assert_eq!(
            diagnostic.selection_reason,
            Some(TassadarExecutorSelectionReason::UnsupportedTraceAbiVersion)
        );
        assert_eq!(diagnostic.effective_decode_mode, None);
    }

    #[test]
    fn runtime_selection_refuses_when_model_cannot_accept_effective_decode_mode() {
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let diagnostic = diagnose_tassadar_executor_request(
            &case.program,
            TassadarExecutorDecodeMode::HullCache,
            TassadarTraceAbi::core_i32_v1().schema_version,
            Some(&[TassadarExecutorDecodeMode::ReferenceLinear]),
        );
        assert!(diagnostic.is_refused());
        assert_eq!(
            diagnostic.selection_reason,
            Some(TassadarExecutorSelectionReason::UnsupportedModelDecodeMode)
        );
    }

    #[test]
    fn execute_request_returns_fallback_diagnostic_with_reference_execution() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let program = TassadarProgram::new(
            "tassadar.backward_branch.v1",
            &profile,
            0,
            0,
            vec![
                TassadarInstruction::I32Const { value: 0 },
                TassadarInstruction::BrIf { target_pc: 0 },
                TassadarInstruction::Return,
            ],
        );
        let report = execute_tassadar_executor_request(
            &program,
            TassadarExecutorDecodeMode::HullCache,
            TassadarTraceAbi::core_i32_v1().schema_version,
            Some(&[TassadarExecutorDecodeMode::ReferenceLinear]),
        )
        .expect("request should fall back rather than refuse");
        assert!(report.selection.is_fallback());
        assert_eq!(report.execution.runner_id, TASSADAR_FIXTURE_RUNNER_ID);
    }

    #[test]
    fn replay_is_deterministic_on_validation_corpus() {
        let runner = TassadarCpuReferenceRunner::new();
        for case in tassadar_validation_corpus() {
            let execution = runner.execute(&case.program).expect("case should run");
            replay_tassadar_execution(&case.program, &execution)
                .expect("replay should match exactly");
        }
    }

    #[test]
    fn trace_artifact_round_trips_through_json() {
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let execution = TassadarCpuReferenceRunner::new()
            .execute(&case.program)
            .expect("case should run");
        let artifact =
            TassadarTraceArtifact::from_execution("tassadar.trace.roundtrip", &execution);

        let encoded =
            serde_json::to_vec(&artifact).expect("trace artifact should serialize to json");
        let decoded: TassadarTraceArtifact =
            serde_json::from_slice(&encoded).expect("trace artifact should decode from json");

        assert_eq!(decoded, artifact);
        assert_eq!(decoded.trace_digest, execution.trace_digest());
        assert_eq!(decoded.behavior_digest, execution.behavior_digest());
    }

    #[test]
    fn trace_diff_report_finds_first_divergence() {
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let execution = TassadarCpuReferenceRunner::new()
            .execute(&case.program)
            .expect("case should run");
        let mut divergent = execution.clone();
        divergent.steps[0].next_pc = divergent.steps[0].next_pc.saturating_add(1);

        let diff = TassadarTraceDiffReport::from_executions(&execution, &divergent);

        assert!(!diff.exact_match);
        assert_eq!(diff.first_divergence_step_index, Some(0));
        assert_eq!(diff.expected_step_count, execution.steps.len() as u64);
        assert_eq!(diff.actual_step_count, divergent.steps.len() as u64);
        assert_eq!(diff.entries.len(), 1);
        assert_eq!(diff.entries[0].kind, TassadarTraceDiffKind::StepMismatch);
    }

    #[test]
    fn exactness_refusal_report_marks_exact_direct_execution() {
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let expected = TassadarCpuReferenceRunner::new()
            .execute(&case.program)
            .expect("case should run");
        let execution_report = execute_tassadar_executor_request(
            &case.program,
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarTraceAbi::core_i32_v1().schema_version,
            None,
        )
        .expect("reference-linear execution should succeed");

        let report = TassadarExactnessRefusalReport::from_execution_report(
            &case.case_id,
            &expected,
            &execution_report,
        );

        assert_eq!(report.exactness_posture, TassadarExactnessPosture::Exact);
        assert_eq!(
            report.selection_state,
            TassadarExecutorSelectionState::Direct
        );
        assert!(report.trace_digest_equal);
        assert!(report.outputs_equal);
        assert!(report.halt_equal);
        assert!(report.mismatch_summary.is_none());
    }

    #[test]
    fn exactness_refusal_report_classifies_step_mismatch() {
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let expected = TassadarCpuReferenceRunner::new()
            .execute(&case.program)
            .expect("case should run");
        let mut divergent = expected.clone();
        divergent.steps[0].next_pc = divergent.steps[0].next_pc.saturating_add(1);
        let selection = diagnose_tassadar_executor_request(
            &case.program,
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarTraceAbi::core_i32_v1().schema_version,
            None,
        );

        let report = TassadarExactnessRefusalReport::from_selection_and_execution(
            &case.case_id,
            &selection,
            &expected,
            &divergent,
        );

        assert_eq!(report.exactness_posture, TassadarExactnessPosture::Mismatch);
        let mismatch = report
            .mismatch_summary
            .expect("mismatch summary should exist");
        assert_eq!(mismatch.mismatch_class, TassadarMismatchClass::StepMismatch);
        assert_eq!(mismatch.first_divergence_step_index, Some(0));
    }

    #[test]
    fn exactness_refusal_report_surfaces_refused_selection_reason() {
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let mut refused_program = case.program.clone();
        refused_program.profile_id = String::from("tassadar.wasm.unsupported_profile.v0");
        let selection = diagnose_tassadar_executor_request(
            &refused_program,
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarTraceAbi::core_i32_v1().schema_version,
            None,
        );

        let report = TassadarExactnessRefusalReport::from_refusal(&case.case_id, &selection, None);

        assert_eq!(report.exactness_posture, TassadarExactnessPosture::Refused);
        assert_eq!(
            report.selection_state,
            TassadarExecutorSelectionState::Refused
        );
        assert_eq!(
            report.selection_reason,
            Some(TassadarExecutorSelectionReason::UnsupportedWasmProfile)
        );
        assert!(report.execution_refusal.is_none());
    }

    #[test]
    fn validation_corpus_includes_shortest_path_fixture() {
        let case = tassadar_validation_corpus()
            .into_iter()
            .find(|case| case.case_id == "shortest_path_two_route")
            .expect("shortest path fixture should exist");

        assert_eq!(case.expected_outputs, vec![5]);
        let execution = TassadarCpuReferenceRunner::new()
            .execute(&case.program)
            .expect("shortest path fixture should execute");
        assert_eq!(execution.outputs, case.expected_outputs);
        assert_eq!(execution.steps, case.expected_trace);
    }

    #[test]
    fn invalid_memory_slot_refuses_with_typed_error() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let program = TassadarProgram::new(
            "tassadar.invalid_memory.v1",
            &profile,
            0,
            1,
            vec![
                TassadarInstruction::I32Load { slot: 1 },
                TassadarInstruction::Return,
            ],
        );
        let error = TassadarCpuReferenceRunner::new()
            .execute(&program)
            .expect_err("invalid slot should refuse");
        assert_eq!(
            error,
            TassadarExecutionRefusal::MemorySlotOutOfRange {
                pc: 0,
                slot: 1,
                memory_slots: 1,
            }
        );
    }

    #[test]
    fn stack_underflow_refuses_with_typed_error() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let program = TassadarProgram::new(
            "tassadar.stack_underflow.v1",
            &profile,
            0,
            0,
            vec![TassadarInstruction::I32Add, TassadarInstruction::Return],
        );
        let error = TassadarCpuReferenceRunner::new()
            .execute(&program)
            .expect_err("underflow should refuse");
        assert_eq!(
            error,
            TassadarExecutionRefusal::StackUnderflow {
                pc: 0,
                needed: 2,
                available: 0,
            }
        );
    }

    #[test]
    fn hull_cache_refuses_backward_branch_programs() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let program = TassadarProgram::new(
            "tassadar.backward_branch.v1",
            &profile,
            0,
            0,
            vec![
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 0 },
                TassadarInstruction::Return,
            ],
        );
        let error = TassadarHullCacheRunner::new()
            .execute(&program)
            .expect_err("backward branch should refuse");
        assert_eq!(
            error,
            TassadarExecutionRefusal::HullCacheBackwardBranchUnsupported {
                pc: 1,
                target_pc: 0,
            }
        );
    }

    #[test]
    fn program_artifact_is_digest_bound_and_internally_consistent() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let artifact = TassadarProgramArtifact::new(
            "tassadar.locals_add.artifact.v1",
            TassadarProgramSourceIdentity::new(
                TassadarProgramSourceKind::Fixture,
                "locals_add",
                "sha256:fixture-source",
            ),
            TassadarCompilerToolchainIdentity::new("clang", "18.1.0", "wasm32-unknown-unknown")
                .with_pipeline_features(vec![String::from("phase2_artifact_contract")]),
            &profile,
            &trace_abi,
            case.program,
        )
        .expect("artifact should assemble");
        artifact
            .validate_internal_consistency()
            .expect("artifact should stay internally consistent");
        assert_eq!(artifact.wasm_profile_id, profile.profile_id);
        assert_eq!(
            artifact.opcode_vocabulary_digest,
            profile.opcode_vocabulary_digest()
        );
    }

    #[test]
    fn program_source_identity_supports_symbolic_program_kind() {
        let source_identity = TassadarProgramSourceIdentity::new(
            TassadarProgramSourceKind::SymbolicProgram,
            "tassadar.symbolic.counter.v1",
            "sha256:symbolic-program",
        );
        let encoded =
            serde_json::to_value(&source_identity).expect("source identity should serialize");
        assert_eq!(encoded["source_kind"], "symbolic_program");
        assert_eq!(encoded["source_name"], "tassadar.symbolic.counter.v1");
        assert_eq!(encoded["source_digest"], "sha256:symbolic-program");
    }

    #[test]
    fn program_artifact_rejects_profile_mismatch() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let mut case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        case.program.profile_id = String::from("tassadar.wasm.other.v1");
        let error = TassadarProgramArtifact::fixture_reference(
            "tassadar.bad_profile.artifact.v1",
            &profile,
            &trace_abi,
            case.program,
        )
        .expect_err("profile mismatch should refuse");
        assert_eq!(
            error,
            TassadarProgramArtifactError::ProgramProfileMismatch {
                expected: profile.profile_id,
                actual: String::from("tassadar.wasm.other.v1"),
            }
        );
    }

    #[test]
    fn tassadar_execution_evidence_bundle_is_replay_stable_on_validation_corpus() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let runner = TassadarFixtureRunner::new();

        for case in tassadar_validation_corpus() {
            let artifact = TassadarProgramArtifact::fixture_reference(
                format!("tassadar://artifact/test/{}", case.case_id),
                &profile,
                &trace_abi,
                case.program.clone(),
            )
            .expect("fixture artifact should build");
            let execution = runner
                .execute(&case.program)
                .expect("fixture execution should pass");
            replay_tassadar_execution(&case.program, &execution)
                .expect("replay should match fixture execution");
            let replayed = runner
                .execute(&case.program)
                .expect("replayed execution should pass");

            let first = build_tassadar_execution_evidence_bundle(
                format!("request-{}", case.case_id),
                format!("digest-{}", case.case_id),
                "tassadar_reference_fixture",
                "tassadar-executor-fixture-v0",
                "model-descriptor-digest",
                vec![String::from("env.openagents.tassadar.benchmark@2026.03.15")],
                &artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
                &execution,
            );
            let second = build_tassadar_execution_evidence_bundle(
                format!("request-{}", case.case_id),
                format!("digest-{}", case.case_id),
                "tassadar_reference_fixture",
                "tassadar-executor-fixture-v0",
                "model-descriptor-digest",
                vec![String::from("env.openagents.tassadar.benchmark@2026.03.15")],
                &artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
                &replayed,
            );

            assert_eq!(
                first.runtime_manifest.identity_digest,
                second.runtime_manifest.identity_digest
            );
            assert_eq!(
                first.runtime_manifest.manifest_digest,
                second.runtime_manifest.manifest_digest
            );
            assert_eq!(
                first.trace_artifact.artifact_digest,
                second.trace_artifact.artifact_digest
            );
            assert_eq!(
                first.trace_proof.proof_digest,
                second.trace_proof.proof_digest
            );
            assert_eq!(
                first.proof_bundle.stable_digest(),
                second.proof_bundle.stable_digest()
            );
        }
    }

    #[test]
    fn tassadar_trace_proof_artifact_carries_required_identity_fields() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar://artifact/test/proof_fields",
            &profile,
            &trace_abi,
            case.program.clone(),
        )
        .expect("fixture artifact should build");
        let execution = TassadarFixtureRunner::new()
            .execute(&case.program)
            .expect("fixture execution should pass");
        let evidence = build_tassadar_execution_evidence_bundle(
            "request-proof-fields",
            "digest-proof-fields",
            "tassadar_reference_fixture",
            "tassadar-executor-fixture-v0",
            "model-descriptor-digest",
            vec![String::from("env.openagents.tassadar.benchmark@2026.03.15")],
            &artifact,
            TassadarExecutorDecodeMode::ReferenceLinear,
            &execution,
        );

        assert_eq!(
            evidence.trace_proof.trace_digest,
            evidence.trace_artifact.trace_digest
        );
        assert_eq!(
            evidence.trace_proof.program_digest,
            artifact.validated_program_digest
        );
        assert_eq!(
            evidence.trace_proof.wasm_profile_id,
            artifact.wasm_profile_id
        );
        assert_eq!(
            evidence.trace_proof.cache_algorithm_id,
            "tassadar.cache.linear_scan_kv.v1"
        );
        assert_eq!(evidence.trace_proof.runtime_backend, "cpu");
        assert_eq!(
            evidence.trace_proof.reference_runner_id,
            execution.runner_id
        );
        assert_eq!(
            evidence.trace_proof.runtime_manifest_identity_digest,
            evidence.runtime_manifest.identity_digest
        );
    }

    #[test]
    fn tassadar_claim_class_transitions_preserve_lane_separation() {
        assert!(
            TassadarClaimClass::ResearchOnly
                .allows_transition_to(TassadarClaimClass::CompiledExact)
        );
        assert!(
            TassadarClaimClass::ResearchOnly
                .allows_transition_to(TassadarClaimClass::LearnedBounded)
        );
        assert!(
            TassadarClaimClass::CompiledExact
                .allows_transition_to(TassadarClaimClass::CompiledArticleClass)
        );
        assert!(
            TassadarClaimClass::LearnedBounded
                .allows_transition_to(TassadarClaimClass::LearnedArticleClass)
        );
        assert!(
            !TassadarClaimClass::CompiledExact
                .allows_transition_to(TassadarClaimClass::LearnedArticleClass)
        );
        assert!(
            !TassadarClaimClass::LearnedBounded
                .allows_transition_to(TassadarClaimClass::CompiledArticleClass)
        );
    }
}
