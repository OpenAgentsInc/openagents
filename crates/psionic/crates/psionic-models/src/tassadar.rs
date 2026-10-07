use psionic_core::{DType, QuantizationMode, Shape};
use psionic_runtime::{
    TASSADAR_ARTICLE_CLASS_BENCHMARK_REF, TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
    TassadarExecutionEvidenceBundle, TassadarExecutorDecodeMode, TassadarExecutorExecutionReport,
    TassadarExecutorSelectionDiagnostic, TassadarFixtureWeights as RuntimeTassadarFixtureWeights,
    TassadarProgram, TassadarProgramArtifact, TassadarRuntimeCapabilityReport, TassadarTraceAbi,
    TassadarWasmProfile, build_tassadar_execution_evidence_bundle,
    diagnose_tassadar_executor_request, execute_tassadar_executor_request,
    tassadar_runtime_capability_report,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ModelArtifactGovernance, ModelDescriptor, ModelIngressSurface, ModelInteropBoundary,
    ModelRuntimeSurface, ModelServingSurface, TassadarArticleTransformer, WeightArtifactMetadata,
    WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
};

const TASSADAR_WORKLOAD_CAPABILITY_MATRIX_SCHEMA_VERSION: u16 = 1;
const TASSADAR_PUBLIC_BENCHMARK_PACKAGE_SET_REF: &str =
    "benchmark-set://openagents/tassadar/public";
const TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_benchmark_package_set_summary.json";
const TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF: &str =
    "benchmark://openagents/tassadar/reference_fixture/validation_corpus";
pub const TASSADAR_ARTICLE_TRANSFORMER_TRAINED_EXECUTOR_DESCRIPTOR_REF: &str = "fixtures/tassadar/models/tassadar_article_transformer_trace_bound_trained_v0_executor_descriptor.json";

/// Stable executor-family identity distinct from ordinary decoder families.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorFamily {
    /// WebAssembly-first append-only trace executor.
    WasmTraceExecutor,
}

/// Attention regime declared by one Tassadar executor descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorAttentionMode {
    /// Programmatic Phase 1 reference fixture rather than a full attention-backed executor.
    ReferenceFixture,
    /// Program-specialized deployment compiled into one weight artifact.
    ProgramSpecializedCompiled,
    /// Standard softmax-backed executor decode.
    StandardSoftmax,
    /// Hard-max lookup executor regime.
    HardMaxLookup,
    /// Sparse top-k lookup executor regime.
    SparseTopKLookup,
}

/// Exactness posture claimed by one Tassadar executor descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorExactnessPosture {
    /// Exact trace, halt, and output behavior is part of the contract.
    ExactTraceAndOutput,
    /// Only final outputs are exact; intermediate traces may differ.
    ExactOutputOnly,
}

/// Attention-geometry claims declared by one Tassadar executor descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAttentionGeometryContract {
    /// Head dimension for lookup-constrained heads when one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constrained_lookup_head_dim: Option<usize>,
    /// Whether the descriptor is eligible for the future hull-cache fast path.
    pub hull_cache_eligible: bool,
}

impl TassadarAttentionGeometryContract {
    /// Returns the current Phase 2 reference-fixture geometry contract.
    #[must_use]
    pub fn reference_fixture() -> Self {
        Self {
            constrained_lookup_head_dim: None,
            hull_cache_eligible: true,
        }
    }
}

/// Machine-legible compatibility contract between executor models and program artifacts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorCompatibility {
    /// Stable executor-family identity.
    pub executor_family: TassadarExecutorFamily,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI schema version.
    pub trace_abi_version: u16,
    /// Stable Wasm profile identifier.
    pub wasm_profile_id: String,
    /// Stable opcode vocabulary digest expected by the descriptor.
    pub opcode_vocabulary_digest: String,
    /// Decode modes this descriptor can support honestly.
    pub supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
    /// Declared attention regime for the descriptor.
    pub attention_mode: TassadarExecutorAttentionMode,
    /// Declared geometry constraints relevant to decode compatibility.
    pub attention_geometry: TassadarAttentionGeometryContract,
    /// Declared exactness posture for the descriptor.
    pub exactness_posture: TassadarExecutorExactnessPosture,
}

impl TassadarExecutorCompatibility {
    /// Returns the canonical Phase 2 reference-fixture compatibility contract.
    #[must_use]
    pub fn reference_fixture(profile: &TassadarWasmProfile, trace_abi: &TassadarTraceAbi) -> Self {
        Self {
            executor_family: TassadarExecutorFamily::WasmTraceExecutor,
            trace_abi_id: trace_abi.abi_id.clone(),
            trace_abi_version: trace_abi.schema_version,
            wasm_profile_id: profile.profile_id.clone(),
            opcode_vocabulary_digest: profile.opcode_vocabulary_digest(),
            supported_decode_modes: vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
                TassadarExecutorDecodeMode::SparseTopK,
            ],
            attention_mode: TassadarExecutorAttentionMode::ReferenceFixture,
            attention_geometry: TassadarAttentionGeometryContract::reference_fixture(),
            exactness_posture: TassadarExecutorExactnessPosture::ExactTraceAndOutput,
        }
    }

    /// Returns whether one decode mode is explicitly supported.
    #[must_use]
    pub fn supports_decode_mode(&self, decode_mode: TassadarExecutorDecodeMode) -> bool {
        self.supported_decode_modes.contains(&decode_mode)
    }
}

/// Builds the served executor-facing descriptor for the trained trace-bound
/// article Transformer route.
pub fn build_tassadar_article_transformer_trained_executor_descriptor()
-> Result<TassadarExecutorModelDescriptor, crate::TassadarArticleTransformerError> {
    let model = TassadarArticleTransformer::trained_trace_domain_reference()?;
    Ok(tassadar_article_transformer_executor_descriptor(&model))
}

/// Projects one article-Transformer artifact into the served executor-model
/// descriptor boundary used by routing, proof receipts, and article sessions.
#[must_use]
pub fn tassadar_article_transformer_executor_descriptor(
    model: &TassadarArticleTransformer,
) -> TassadarExecutorModelDescriptor {
    let profile = TassadarWasmProfile::article_i32_compute_v1();
    let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
    let compatibility = TassadarExecutorCompatibility {
        executor_family: TassadarExecutorFamily::WasmTraceExecutor,
        trace_abi_id: trace_abi.abi_id.clone(),
        trace_abi_version: trace_abi.schema_version,
        wasm_profile_id: profile.profile_id.clone(),
        opcode_vocabulary_digest: profile.opcode_vocabulary_digest(),
        supported_decode_modes: vec![
            TassadarExecutorDecodeMode::ReferenceLinear,
            TassadarExecutorDecodeMode::HullCache,
        ],
        attention_mode: TassadarExecutorAttentionMode::StandardSoftmax,
        attention_geometry: TassadarAttentionGeometryContract {
            constrained_lookup_head_dim: None,
            hull_cache_eligible: true,
        },
        exactness_posture: TassadarExecutorExactnessPosture::ExactTraceAndOutput,
    };
    TassadarExecutorModelDescriptor::new(
        ModelDescriptor::new(
            model.descriptor().model.model_id.clone(),
            model.descriptor().model.family.clone(),
            model.descriptor().model.revision.clone(),
        ),
        compatibility,
        profile,
        trace_abi,
        model.weight_metadata().clone(),
    )
}

/// Explicit workload-class taxonomy for public Tassadar capability publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkloadClass {
    /// Arithmetic-only microprograms.
    ArithmeticMicroprogram,
    /// CLRS-adjacent shortest-path witness workloads.
    ClrsShortestPath,
    /// Memory or local read-write microprograms.
    MemoryLookupMicroprogram,
    /// Branch and control-flow microprograms.
    BranchControlFlowMicroprogram,
    /// Richer WebAssembly kernels beyond the current microprogram corpus.
    MicroWasmKernel,
    /// Branch-heavy kernel programs with repeated control-flow pivots.
    BranchHeavyKernel,
    /// Memory-heavy kernel programs with dense read/write traffic.
    MemoryHeavyKernel,
    /// Long-loop kernels that push the executor toward longer horizons.
    LongLoopKernel,
    /// Sudoku-style exact search workloads.
    SudokuClass,
    /// Hungarian or min-cost-matching style workloads.
    HungarianMatching,
}

impl TassadarWorkloadClass {
    /// Returns all workload classes in stable publication order.
    #[must_use]
    pub const fn all() -> [Self; 10] {
        [
            Self::ArithmeticMicroprogram,
            Self::ClrsShortestPath,
            Self::MemoryLookupMicroprogram,
            Self::BranchControlFlowMicroprogram,
            Self::MicroWasmKernel,
            Self::BranchHeavyKernel,
            Self::MemoryHeavyKernel,
            Self::LongLoopKernel,
            Self::SudokuClass,
            Self::HungarianMatching,
        ]
    }

    /// Returns the stable workload label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArithmeticMicroprogram => "arithmetic_microprogram",
            Self::ClrsShortestPath => "clrs_shortest_path",
            Self::MemoryLookupMicroprogram => "memory_lookup_microprogram",
            Self::BranchControlFlowMicroprogram => "branch_control_flow_microprogram",
            Self::MicroWasmKernel => "micro_wasm_kernel",
            Self::BranchHeavyKernel => "branch_heavy_kernel",
            Self::MemoryHeavyKernel => "memory_heavy_kernel",
            Self::LongLoopKernel => "long_loop_kernel",
            Self::SudokuClass => "sudoku_class",
            Self::HungarianMatching => "hungarian_matching",
        }
    }
}

/// Served-support posture for one workload class on one Tassadar lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkloadSupportPosture {
    /// The workload class is published as exact on this lane.
    Exact,
    /// The workload class remains exact only by falling back to a slower direct path.
    ExactFallbackOnly,
    /// The workload class is bounded and partially supported on this lane.
    Partial,
    /// The workload class exists only as a research signal and is not a served claim.
    ResearchOnly,
    /// The workload class is outside the published support boundary.
    Unsupported,
}

/// Typed refusal reason for workload classes that are not published directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWorkloadCapabilityRefusalReason {
    /// The workload sits outside the published lane boundary.
    WorkloadClassOutOfScope,
    /// The workload has no benchmark gate for served publication yet.
    BenchmarkGateMissing,
    /// The lane still stops at a research or next-token claim boundary.
    ClaimBoundaryUnvalidated,
}

/// Benchmark-backed gate required before a workload row is published as support.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadBenchmarkGate {
    /// Stable benchmark or benchmark-set ref that gates publication.
    pub benchmark_gate_ref: String,
    /// Repo-owned evidence anchor proving the gate is public and reviewable.
    pub evidence_ref: String,
}

impl TassadarWorkloadBenchmarkGate {
    fn validate(
        &self,
        workload_class: TassadarWorkloadClass,
    ) -> Result<(), TassadarWorkloadCapabilityMatrixError> {
        if self.benchmark_gate_ref.trim().is_empty() {
            return Err(
                TassadarWorkloadCapabilityMatrixError::MissingBenchmarkGateRef { workload_class },
            );
        }
        if self.evidence_ref.trim().is_empty() {
            return Err(
                TassadarWorkloadCapabilityMatrixError::MissingBenchmarkEvidenceRef {
                    workload_class,
                },
            );
        }
        Ok(())
    }
}

/// One workload row in the public Tassadar capability matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadCapabilityRow {
    /// Stable workload-class identifier.
    pub workload_class: TassadarWorkloadClass,
    /// Current published support posture.
    pub support_posture: TassadarWorkloadSupportPosture,
    /// Decode modes the lane can speak about honestly for this class.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
    /// Exact fallback mode when the posture is fallback-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_fallback_decode_mode: Option<TassadarExecutorDecodeMode>,
    /// Typed refusal reasons for unsupported or unpublishable rows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusal_reasons: Vec<TassadarWorkloadCapabilityRefusalReason>,
    /// Benchmark gate required before the row is published as support.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_gate: Option<TassadarWorkloadBenchmarkGate>,
    /// Plain-language boundary note for the row.
    pub detail: String,
}

impl TassadarWorkloadCapabilityRow {
    fn publishable(&self) -> bool {
        matches!(
            self.support_posture,
            TassadarWorkloadSupportPosture::Exact
                | TassadarWorkloadSupportPosture::ExactFallbackOnly
                | TassadarWorkloadSupportPosture::Partial
        )
    }
}

/// Machine-readable workload capability matrix for one Tassadar model lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWorkloadCapabilityMatrix {
    /// Stable schema version for the matrix.
    pub schema_version: u16,
    /// Stable model identifier that owns the matrix.
    pub model_id: String,
    /// Stable model family.
    pub model_family: String,
    /// Ordered workload rows.
    pub rows: Vec<TassadarWorkloadCapabilityRow>,
    /// Plain-language claim boundary for the full matrix.
    pub claim_boundary: String,
    /// Stable digest over the full matrix.
    pub matrix_digest: String,
}

impl TassadarWorkloadCapabilityMatrix {
    /// Creates a workload capability matrix in stable row order.
    #[must_use]
    pub fn new(
        model_id: impl Into<String>,
        model_family: impl Into<String>,
        mut rows: Vec<TassadarWorkloadCapabilityRow>,
        claim_boundary: impl Into<String>,
    ) -> Self {
        rows.sort_by_key(|row| row.workload_class.as_str());
        let mut matrix = Self {
            schema_version: TASSADAR_WORKLOAD_CAPABILITY_MATRIX_SCHEMA_VERSION,
            model_id: model_id.into(),
            model_family: model_family.into(),
            rows,
            claim_boundary: claim_boundary.into(),
            matrix_digest: String::new(),
        };
        matrix.refresh_digest();
        matrix
    }

    /// Returns one row by workload class.
    #[must_use]
    pub fn row(
        &self,
        workload_class: TassadarWorkloadClass,
    ) -> Option<&TassadarWorkloadCapabilityRow> {
        self.rows
            .iter()
            .find(|row| row.workload_class == workload_class)
    }

    /// Validates that the matrix is explicit enough for served publication.
    pub fn validate_publication(&self) -> Result<(), TassadarWorkloadCapabilityMatrixError> {
        if self.rows.is_empty() {
            return Err(TassadarWorkloadCapabilityMatrixError::EmptyMatrix);
        }
        let mut seen = Vec::with_capacity(self.rows.len());
        let mut publishable_rows = 0usize;
        for row in &self.rows {
            if seen.contains(&row.workload_class) {
                return Err(
                    TassadarWorkloadCapabilityMatrixError::DuplicateWorkloadClass {
                        workload_class: row.workload_class,
                    },
                );
            }
            seen.push(row.workload_class);
            if row.publishable() {
                publishable_rows += 1;
                if row.supported_decode_modes.is_empty() {
                    return Err(
                        TassadarWorkloadCapabilityMatrixError::PublishableWorkloadMissingDecodeModes {
                            workload_class: row.workload_class,
                        },
                    );
                }
                if matches!(
                    row.support_posture,
                    TassadarWorkloadSupportPosture::ExactFallbackOnly
                ) && row.exact_fallback_decode_mode.is_none()
                {
                    return Err(
                        TassadarWorkloadCapabilityMatrixError::FallbackWorkloadMissingExactFallback {
                            workload_class: row.workload_class,
                        },
                    );
                }
                row.benchmark_gate
                    .as_ref()
                    .ok_or(
                        TassadarWorkloadCapabilityMatrixError::PublishableWorkloadMissingBenchmarkGate {
                            workload_class: row.workload_class,
                        },
                    )?
                    .validate(row.workload_class)?;
            } else if row.refusal_reasons.is_empty() {
                return Err(
                    TassadarWorkloadCapabilityMatrixError::UnpublishedWorkloadMissingRefusalReasons {
                        workload_class: row.workload_class,
                    },
                );
            }
        }
        if publishable_rows == 0 {
            return Err(TassadarWorkloadCapabilityMatrixError::NoPublishableWorkloads);
        }
        Ok(())
    }

    fn refresh_digest(&mut self) {
        #[derive(Serialize)]
        struct MatrixDigestEncoding<'a> {
            schema_version: u16,
            model_id: &'a str,
            model_family: &'a str,
            rows: &'a [TassadarWorkloadCapabilityRow],
            claim_boundary: &'a str,
        }

        self.matrix_digest = stable_serialized_digest(
            b"tassadar_workload_capability_matrix|",
            &MatrixDigestEncoding {
                schema_version: self.schema_version,
                model_id: self.model_id.as_str(),
                model_family: self.model_family.as_str(),
                rows: self.rows.as_slice(),
                claim_boundary: self.claim_boundary.as_str(),
            },
        );
    }
}

/// Workload capability matrix generation or publication failure.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarWorkloadCapabilityMatrixError {
    /// No workload rows were present.
    #[error("Tassadar workload capability matrix must contain at least one row")]
    EmptyMatrix,
    /// The matrix duplicated a workload class.
    #[error("duplicate Tassadar workload class `{workload_class:?}` in capability matrix")]
    DuplicateWorkloadClass {
        /// Repeated workload class.
        workload_class: TassadarWorkloadClass,
    },
    /// One supported row did not advertise any decode modes.
    #[error("publishable workload `{workload_class:?}` is missing supported decode modes")]
    PublishableWorkloadMissingDecodeModes {
        /// Workload class missing decode modes.
        workload_class: TassadarWorkloadClass,
    },
    /// One fallback-only row omitted the exact fallback mode.
    #[error("fallback-only workload `{workload_class:?}` is missing an exact fallback mode")]
    FallbackWorkloadMissingExactFallback {
        /// Workload class missing the fallback mode.
        workload_class: TassadarWorkloadClass,
    },
    /// One publishable row omitted its benchmark gate.
    #[error("publishable workload `{workload_class:?}` is missing its benchmark gate")]
    PublishableWorkloadMissingBenchmarkGate {
        /// Workload class missing the gate.
        workload_class: TassadarWorkloadClass,
    },
    /// One benchmark gate omitted its benchmark or package-set ref.
    #[error("workload `{workload_class:?}` is missing its benchmark gate ref")]
    MissingBenchmarkGateRef {
        /// Workload class missing the gate ref.
        workload_class: TassadarWorkloadClass,
    },
    /// One benchmark gate omitted its evidence anchor.
    #[error("workload `{workload_class:?}` is missing its benchmark evidence ref")]
    MissingBenchmarkEvidenceRef {
        /// Workload class missing the evidence ref.
        workload_class: TassadarWorkloadClass,
    },
    /// One unpublished row omitted its typed refusal reasons.
    #[error("unpublished workload `{workload_class:?}` is missing refusal reasons")]
    UnpublishedWorkloadMissingRefusalReasons {
        /// Workload class missing refusal reasons.
        workload_class: TassadarWorkloadClass,
    },
    /// The matrix had no publishable workloads after benchmark gating.
    #[error("Tassadar workload capability matrix has no benchmark-gated publishable workloads")]
    NoPublishableWorkloads,
}

/// Typed compatibility failures when pairing a program artifact with an executor model descriptor.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutorContractError {
    /// The program artifact is not internally self-consistent.
    #[error("program artifact is internally inconsistent: {message}")]
    ProgramArtifactInconsistent {
        /// Internal artifact-consistency failure summary.
        message: String,
    },
    /// The artifact targeted a different Wasm profile than the descriptor.
    #[error("Wasm profile mismatch: expected `{expected}`, got `{actual}`")]
    WasmProfileMismatch {
        /// Expected descriptor profile identifier.
        expected: String,
        /// Actual artifact profile identifier.
        actual: String,
    },
    /// The artifact targeted a different trace ABI identifier.
    #[error("trace ABI mismatch: expected `{expected}`, got `{actual}`")]
    TraceAbiMismatch {
        /// Expected descriptor trace ABI identifier.
        expected: String,
        /// Actual artifact trace ABI identifier.
        actual: String,
    },
    /// The artifact targeted a different trace ABI schema version.
    #[error("trace ABI version mismatch: expected `{expected}`, got `{actual}`")]
    TraceAbiVersionMismatch {
        /// Expected descriptor trace ABI schema version.
        expected: u16,
        /// Actual artifact trace ABI schema version.
        actual: u16,
    },
    /// The artifact carried a different opcode-vocabulary digest than the descriptor.
    #[error("opcode vocabulary digest mismatch: expected `{expected}`, got `{actual}`")]
    OpcodeVocabularyDigestMismatch {
        /// Expected descriptor opcode vocabulary digest.
        expected: String,
        /// Actual artifact opcode vocabulary digest.
        actual: String,
    },
    /// The artifact's validated program profile no longer matches the descriptor.
    #[error("validated program profile mismatch: expected `{expected}`, got `{actual}`")]
    ProgramProfileMismatch {
        /// Expected descriptor/program profile identifier.
        expected: String,
        /// Actual validated-program profile identifier.
        actual: String,
    },
    /// The caller requested a decode mode this descriptor does not support.
    #[error("decode mode `{requested:?}` is unsupported; descriptor supports {supported:?}")]
    DecodeModeUnsupported {
        /// Requested decode mode.
        requested: TassadarExecutorDecodeMode,
        /// Supported decode modes declared by the descriptor.
        supported: Vec<TassadarExecutorDecodeMode>,
    },
}

/// Executor-class model descriptor for the Phase 1 Tassadar fixture lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorModelDescriptor {
    /// Shared model metadata.
    pub model: ModelDescriptor,
    /// Machine-legible executor/program compatibility contract.
    pub compatibility: TassadarExecutorCompatibility,
    /// Machine-legible supported Wasm-first profile.
    pub profile: TassadarWasmProfile,
    /// Append-only trace ABI declaration.
    pub trace_abi: TassadarTraceAbi,
    /// Programmatic fixture weight bundle metadata.
    pub weights: WeightBundleMetadata,
    /// Stable provenance and license facts when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_governance: Option<ModelArtifactGovernance>,
}

impl TassadarExecutorModelDescriptor {
    /// Creates a Tassadar executor model descriptor.
    #[must_use]
    pub fn new(
        model: ModelDescriptor,
        compatibility: TassadarExecutorCompatibility,
        profile: TassadarWasmProfile,
        trace_abi: TassadarTraceAbi,
        weights: WeightBundleMetadata,
    ) -> Self {
        Self {
            model,
            compatibility,
            profile,
            trace_abi,
            weights,
            artifact_governance: None,
        }
    }

    /// Attaches provenance and license facts for the backing artifact when known.
    #[must_use]
    pub fn with_artifact_governance(
        mut self,
        artifact_governance: ModelArtifactGovernance,
    ) -> Self {
        self.artifact_governance = Some(artifact_governance);
        self
    }

    /// Returns the explicit compatibility/native boundary for this model path.
    #[must_use]
    pub fn interop_boundary(&self) -> ModelInteropBoundary {
        ModelInteropBoundary {
            catalog_surface: self
                .artifact_governance
                .as_ref()
                .and_then(ModelArtifactGovernance::catalog_surface),
            ingress_surface: infer_executor_ingress_surface(
                &self.weights,
                self.artifact_governance.as_ref(),
            ),
            serving_surface: ModelServingSurface::PsionicNative,
            runtime_surface: ModelRuntimeSurface::PsionicNative,
        }
    }

    /// Returns a stable digest over the executor model descriptor.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded =
            serde_json::to_vec(self).expect("Tassadar executor model descriptor should serialize");
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_executor_model_descriptor|");
        hasher.update(encoded);
        hex::encode(hasher.finalize())
    }

    /// Validates one digest-bound program artifact against this descriptor.
    pub fn validate_program_artifact(
        &self,
        artifact: &TassadarProgramArtifact,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<(), TassadarExecutorContractError> {
        if artifact.wasm_profile_id != self.compatibility.wasm_profile_id {
            return Err(TassadarExecutorContractError::WasmProfileMismatch {
                expected: self.compatibility.wasm_profile_id.clone(),
                actual: artifact.wasm_profile_id.clone(),
            });
        }
        if artifact.trace_abi_id != self.compatibility.trace_abi_id {
            return Err(TassadarExecutorContractError::TraceAbiMismatch {
                expected: self.compatibility.trace_abi_id.clone(),
                actual: artifact.trace_abi_id.clone(),
            });
        }
        if artifact.trace_abi_version != self.compatibility.trace_abi_version {
            return Err(TassadarExecutorContractError::TraceAbiVersionMismatch {
                expected: self.compatibility.trace_abi_version,
                actual: artifact.trace_abi_version,
            });
        }
        if artifact.opcode_vocabulary_digest != self.compatibility.opcode_vocabulary_digest {
            return Err(
                TassadarExecutorContractError::OpcodeVocabularyDigestMismatch {
                    expected: self.compatibility.opcode_vocabulary_digest.clone(),
                    actual: artifact.opcode_vocabulary_digest.clone(),
                },
            );
        }
        if artifact.validated_program.profile_id != self.profile.profile_id {
            return Err(TassadarExecutorContractError::ProgramProfileMismatch {
                expected: self.profile.profile_id.clone(),
                actual: artifact.validated_program.profile_id.clone(),
            });
        }
        if !self
            .compatibility
            .supports_decode_mode(requested_decode_mode)
        {
            return Err(TassadarExecutorContractError::DecodeModeUnsupported {
                requested: requested_decode_mode,
                supported: self.compatibility.supported_decode_modes.clone(),
            });
        }
        artifact.validate_internal_consistency().map_err(|error| {
            TassadarExecutorContractError::ProgramArtifactInconsistent {
                message: error.to_string(),
            }
        })?;
        Ok(())
    }
}

/// Programmatic fixture bundle for the Phase 1 Tassadar executor lane.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarExecutorWeightBundle {
    metadata: WeightBundleMetadata,
    opcode_stack_effects: Vec<f32>,
    opcode_semantics: Vec<f32>,
    profile_limits: Vec<f32>,
    trace_abi_flags: Vec<f32>,
}

impl TassadarExecutorWeightBundle {
    /// Returns the stable weight metadata.
    #[must_use]
    pub fn metadata(&self) -> &WeightBundleMetadata {
        &self.metadata
    }

    /// Returns the `[opcode_count, 2]` stack-effect tensor.
    #[must_use]
    pub fn opcode_stack_effects(&self) -> &[f32] {
        &self.opcode_stack_effects
    }

    /// Returns the `[opcode_count, 5]` semantic-signature tensor.
    #[must_use]
    pub fn opcode_semantics(&self) -> &[f32] {
        &self.opcode_semantics
    }

    /// Returns the `[4]` profile-limits tensor.
    #[must_use]
    pub fn profile_limits(&self) -> &[f32] {
        &self.profile_limits
    }

    /// Returns the `[6]` trace-ABI tensor.
    #[must_use]
    pub fn trace_abi_flags(&self) -> &[f32] {
        &self.trace_abi_flags
    }
}

/// Canonical programmatic fixture model for the Phase 1 Tassadar lane.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarExecutorFixture {
    descriptor: TassadarExecutorModelDescriptor,
    runtime_weights: RuntimeTassadarFixtureWeights,
    weight_bundle: TassadarExecutorWeightBundle,
}

impl Default for TassadarExecutorFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl TassadarExecutorFixture {
    /// Stable model identifier for the Phase 1 fixture.
    pub const MODEL_ID: &str = "tassadar-executor-fixture-v0";
    /// Stable model identifier for the widened core-i32-v2 fixture.
    pub const CORE_I32_V2_MODEL_ID: &str = "tassadar-executor-core-i32-v2-v0";
    /// Stable model identifier for the article-shaped mixed-workload fixture.
    pub const ARTICLE_I32_COMPUTE_MODEL_ID: &str = "tassadar-executor-article-i32-compute-v0";
    /// Stable model identifier for the honest Sudoku-v0 search fixture.
    pub const SUDOKU_V0_SEARCH_MODEL_ID: &str = "tassadar-executor-sudoku-v0-search-v0";
    /// Stable model identifier for the honest Hungarian-v0 matching fixture.
    pub const HUNGARIAN_V0_MATCHING_MODEL_ID: &str = "tassadar-executor-hungarian-v0-matching-v0";
    /// Stable model identifier for the exact 10x10 Hungarian matching fixture.
    pub const HUNGARIAN_10X10_MATCHING_MODEL_ID: &str =
        "tassadar-executor-hungarian-10x10-matching-v0";
    /// Stable model identifier for the honest 9x9 Sudoku-class search fixture.
    pub const SUDOKU_9X9_SEARCH_MODEL_ID: &str = "tassadar-executor-sudoku-9x9-search-v0";
    /// Stable model identifier for the named hard 9x9 Sudoku-class search fixture.
    pub const SUDOKU_9X9_HARD_SEARCH_MODEL_ID: &str = "tassadar-executor-sudoku-9x9-hard-search-v0";
    /// Stable model family for the Phase 1 fixture.
    pub const MODEL_FAMILY: &str = "tassadar_executor";

    /// Creates the default Phase 1 Tassadar executor fixture.
    #[must_use]
    pub fn new() -> Self {
        Self::core_i32_v1()
    }

    /// Creates the canonical Phase 1 `core_i32_v1` executor fixture.
    #[must_use]
    pub fn core_i32_v1() -> Self {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let runtime_weights = RuntimeTassadarFixtureWeights::core_i32_v1();
        Self::from_parts(Self::MODEL_ID, profile, trace_abi, runtime_weights)
    }

    /// Creates the widened article-class `core_i32_v2` executor fixture.
    #[must_use]
    pub fn core_i32_v2() -> Self {
        let profile = TassadarWasmProfile::core_i32_v2();
        let trace_abi = TassadarTraceAbi::core_i32_v2();
        let runtime_weights = RuntimeTassadarFixtureWeights::core_i32_v2();
        Self::from_parts(
            Self::CORE_I32_V2_MODEL_ID,
            profile,
            trace_abi,
            runtime_weights,
        )
    }

    /// Creates the article-shaped mixed-workload executor fixture.
    #[must_use]
    pub fn article_i32_compute_v1() -> Self {
        let profile = TassadarWasmProfile::article_i32_compute_v1();
        let trace_abi = TassadarTraceAbi::article_i32_compute_v1();
        let runtime_weights = RuntimeTassadarFixtureWeights::article_i32_compute_v1();
        Self::from_parts(
            Self::ARTICLE_I32_COMPUTE_MODEL_ID,
            profile,
            trace_abi,
            runtime_weights,
        )
    }

    /// Creates the honest Sudoku-v0 search executor fixture.
    #[must_use]
    pub fn sudoku_v0_search_v1() -> Self {
        let profile = TassadarWasmProfile::sudoku_v0_search_v1();
        let trace_abi = TassadarTraceAbi::sudoku_v0_search_v1();
        let runtime_weights = RuntimeTassadarFixtureWeights::sudoku_v0_search_v1();
        Self::from_parts(
            Self::SUDOKU_V0_SEARCH_MODEL_ID,
            profile,
            trace_abi,
            runtime_weights,
        )
    }

    /// Creates the honest Hungarian-v0 matching executor fixture.
    #[must_use]
    pub fn hungarian_v0_matching_v1() -> Self {
        let profile = TassadarWasmProfile::hungarian_v0_matching_v1();
        let trace_abi = TassadarTraceAbi::hungarian_v0_matching_v1();
        let runtime_weights = RuntimeTassadarFixtureWeights::hungarian_v0_matching_v1();
        Self::from_parts(
            Self::HUNGARIAN_V0_MATCHING_MODEL_ID,
            profile,
            trace_abi,
            runtime_weights,
        )
    }

    /// Creates the exact 10x10 Hungarian matching executor fixture.
    #[must_use]
    pub fn hungarian_10x10_matching_v1() -> Self {
        let profile = TassadarWasmProfile::hungarian_10x10_matching_v1();
        let trace_abi = TassadarTraceAbi::hungarian_10x10_matching_v1();
        let runtime_weights = RuntimeTassadarFixtureWeights::hungarian_10x10_matching_v1();
        Self::from_parts(
            Self::HUNGARIAN_10X10_MATCHING_MODEL_ID,
            profile,
            trace_abi,
            runtime_weights,
        )
    }

    /// Creates the honest 9x9 Sudoku-class search executor fixture.
    #[must_use]
    pub fn sudoku_9x9_search_v1() -> Self {
        let profile = TassadarWasmProfile::sudoku_9x9_search_v1();
        let trace_abi = TassadarTraceAbi::sudoku_9x9_search_v1();
        let runtime_weights = RuntimeTassadarFixtureWeights::sudoku_9x9_search_v1();
        Self::from_parts(
            Self::SUDOKU_9X9_SEARCH_MODEL_ID,
            profile,
            trace_abi,
            runtime_weights,
        )
    }

    /// Creates the named hard 9x9 Sudoku-class search executor fixture.
    #[must_use]
    pub fn sudoku_9x9_hard_search_v1() -> Self {
        let profile = TassadarWasmProfile::sudoku_9x9_hard_search_v1();
        let trace_abi = TassadarTraceAbi::sudoku_9x9_hard_search_v1();
        let runtime_weights = RuntimeTassadarFixtureWeights::sudoku_9x9_hard_search_v1();
        Self::from_parts(
            Self::SUDOKU_9X9_HARD_SEARCH_MODEL_ID,
            profile,
            trace_abi,
            runtime_weights,
        )
    }

    /// Returns the canonical fixture for one supported Wasm profile id.
    #[must_use]
    pub fn for_profile_id(profile_id: &str) -> Option<Self> {
        match profile_id {
            value if value == TassadarWasmProfile::core_i32_v1().profile_id => {
                Some(Self::core_i32_v1())
            }
            value if value == TassadarWasmProfile::core_i32_v2().profile_id => {
                Some(Self::core_i32_v2())
            }
            value if value == TassadarWasmProfile::article_i32_compute_v1().profile_id => {
                Some(Self::article_i32_compute_v1())
            }
            value if value == TassadarWasmProfile::sudoku_v0_search_v1().profile_id => {
                Some(Self::sudoku_v0_search_v1())
            }
            value if value == TassadarWasmProfile::hungarian_v0_matching_v1().profile_id => {
                Some(Self::hungarian_v0_matching_v1())
            }
            value if value == TassadarWasmProfile::hungarian_10x10_matching_v1().profile_id => {
                Some(Self::hungarian_10x10_matching_v1())
            }
            value if value == TassadarWasmProfile::sudoku_9x9_search_v1().profile_id => {
                Some(Self::sudoku_9x9_search_v1())
            }
            value if value == TassadarWasmProfile::sudoku_9x9_hard_search_v1().profile_id => {
                Some(Self::sudoku_9x9_hard_search_v1())
            }
            _ => None,
        }
    }

    fn from_parts(
        model_id: &str,
        profile: TassadarWasmProfile,
        trace_abi: TassadarTraceAbi,
        runtime_weights: RuntimeTassadarFixtureWeights,
    ) -> Self {
        let weight_bundle = build_weight_bundle(&runtime_weights, &profile, &trace_abi);
        let compatibility = TassadarExecutorCompatibility::reference_fixture(&profile, &trace_abi);
        let descriptor = TassadarExecutorModelDescriptor::new(
            ModelDescriptor::new(model_id, Self::MODEL_FAMILY, "v0"),
            compatibility,
            profile,
            trace_abi,
            weight_bundle.metadata().clone(),
        );
        Self {
            descriptor,
            runtime_weights,
            weight_bundle,
        }
    }

    /// Returns the public model descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &TassadarExecutorModelDescriptor {
        &self.descriptor
    }

    /// Returns the handcrafted runtime-side rule tables.
    #[must_use]
    pub fn runtime_weights(&self) -> &RuntimeTassadarFixtureWeights {
        &self.runtime_weights
    }

    /// Returns the programmatic fixture weight bundle.
    #[must_use]
    pub fn weight_bundle(&self) -> &TassadarExecutorWeightBundle {
        &self.weight_bundle
    }

    /// Returns the current runtime capability report for the fixture-backed executor lane.
    #[must_use]
    pub fn runtime_capability_report(&self) -> TassadarRuntimeCapabilityReport {
        tassadar_runtime_capability_report()
    }

    /// Returns the runtime decode selection diagnostic for one requested program path.
    #[must_use]
    pub fn runtime_selection_diagnostic(
        &self,
        program: &TassadarProgram,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> TassadarExecutorSelectionDiagnostic {
        diagnose_tassadar_executor_request(
            program,
            requested_decode_mode,
            self.descriptor.trace_abi.schema_version,
            Some(
                self.descriptor
                    .compatibility
                    .supported_decode_modes
                    .as_slice(),
            ),
        )
    }

    /// Returns the public workload capability matrix for this executor lane.
    #[must_use]
    pub fn workload_capability_matrix(&self) -> TassadarWorkloadCapabilityMatrix {
        workload_capability_matrix_for_fixture(self)
    }

    /// Compiles one digest-bound program artifact into a program-specialized
    /// compiled-weight deployment.
    pub fn compile_program(
        &self,
        artifact_id: impl Into<String>,
        program_artifact: &TassadarProgramArtifact,
    ) -> Result<TassadarCompiledProgramExecutor, TassadarCompiledProgramError> {
        TassadarCompiledProgramExecutor::compile(artifact_id, self, program_artifact)
    }
}

/// Stable compiled-weight deployment kind for the current program-specialized path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompiledExecutorDeploymentKind {
    /// One compiled artifact is valid for exactly one digest-bound program.
    ProgramSpecialized,
}

/// Program-specialized compiled-weight bundle emitted for one exact Wasm program.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarCompiledProgramWeightBundle {
    metadata: WeightBundleMetadata,
    compiled_program_header: Vec<f32>,
    compiled_instruction_stream: Vec<f32>,
    compiled_initial_memory: Vec<f32>,
    compiled_decode_contract: Vec<f32>,
}

impl TassadarCompiledProgramWeightBundle {
    /// Returns the stable weight metadata.
    #[must_use]
    pub fn metadata(&self) -> &WeightBundleMetadata {
        &self.metadata
    }

    /// Returns the compiled program header tensor.
    #[must_use]
    pub fn compiled_program_header(&self) -> &[f32] {
        &self.compiled_program_header
    }

    /// Returns the padded compiled instruction stream tensor.
    #[must_use]
    pub fn compiled_instruction_stream(&self) -> &[f32] {
        &self.compiled_instruction_stream
    }

    /// Returns the padded compiled initial-memory tensor.
    #[must_use]
    pub fn compiled_initial_memory(&self) -> &[f32] {
        &self.compiled_initial_memory
    }

    /// Returns the compiled decode-contract tensor.
    #[must_use]
    pub fn compiled_decode_contract(&self) -> &[f32] {
        &self.compiled_decode_contract
    }
}

/// Digest-bound compiled-weight artifact carrying proof and lineage metadata for
/// one program-specialized deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledProgramWeightArtifact {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable digest over the compiled artifact contract.
    pub artifact_digest: String,
    /// Stable deployment kind for the compiled artifact.
    pub deployment_kind: TassadarCompiledExecutorDeploymentKind,
    /// Stable base model identifier used as the compilation source.
    pub base_model_id: String,
    /// Stable digest of the base fixture descriptor.
    pub base_model_descriptor_digest: String,
    /// Stable program-artifact reference.
    pub program_artifact_ref: String,
    /// Stable digest of the source program artifact.
    pub program_artifact_digest: String,
    /// Stable digest of the validated program payload.
    pub program_digest: String,
    /// Stable Wasm profile identifier the compiled artifact targets.
    pub wasm_profile_id: String,
    /// Stable trace ABI identifier the compiled artifact targets.
    pub trace_abi_id: String,
    /// Stable trace ABI version the compiled artifact targets.
    pub trace_abi_version: u16,
    /// Decode modes this compiled deployment can support honestly.
    pub supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
    /// Stable digest of the compiled weight bundle metadata.
    pub compiled_weight_bundle_digest: String,
    /// Stable byte digest of the serialized compiled-weight artifact payload.
    pub compiled_weight_artifact_sha256: String,
    /// Byte length of the serialized compiled-weight artifact payload.
    pub compiled_weight_artifact_bytes: u64,
    /// Runner-independent behavior digest proven at compile time.
    pub compiled_behavior_digest: String,
    /// Stable digest of the compile-time runtime manifest.
    pub compile_runtime_manifest_digest: String,
    /// Stable digest of the compile-time trace proof.
    pub compile_trace_proof_digest: String,
    /// Stable digest of the compile-time execution proof bundle.
    pub compile_execution_proof_bundle_digest: String,
}

impl TassadarCompiledProgramWeightArtifact {
    fn new(
        artifact_id: impl Into<String>,
        base_model_id: impl Into<String>,
        base_model_descriptor_digest: impl Into<String>,
        program_artifact: &TassadarProgramArtifact,
        supported_decode_modes: &[TassadarExecutorDecodeMode],
        weight_bundle: &TassadarCompiledProgramWeightBundle,
        evidence_bundle: &TassadarExecutionEvidenceBundle,
        compiled_behavior_digest: impl Into<String>,
    ) -> Self {
        let mut supported_decode_modes = supported_decode_modes.to_vec();
        supported_decode_modes.sort_by_key(|mode| mode.as_str());
        let primary_artifact = weight_bundle
            .metadata()
            .artifacts
            .first()
            .cloned()
            .unwrap_or_else(|| {
                WeightArtifactMetadata::new(
                    "compiled_weight_bundle.json",
                    0,
                    weight_bundle.metadata().digest.clone(),
                )
            });
        let mut artifact = Self {
            artifact_id: artifact_id.into(),
            artifact_digest: String::new(),
            deployment_kind: TassadarCompiledExecutorDeploymentKind::ProgramSpecialized,
            base_model_id: base_model_id.into(),
            base_model_descriptor_digest: base_model_descriptor_digest.into(),
            program_artifact_ref: program_artifact.artifact_id.clone(),
            program_artifact_digest: program_artifact.artifact_digest.clone(),
            program_digest: program_artifact.validated_program_digest.clone(),
            wasm_profile_id: program_artifact.wasm_profile_id.clone(),
            trace_abi_id: program_artifact.trace_abi_id.clone(),
            trace_abi_version: program_artifact.trace_abi_version,
            supported_decode_modes,
            compiled_weight_bundle_digest: weight_bundle.metadata().digest.clone(),
            compiled_weight_artifact_sha256: primary_artifact.sha256,
            compiled_weight_artifact_bytes: primary_artifact.byte_length,
            compiled_behavior_digest: compiled_behavior_digest.into(),
            compile_runtime_manifest_digest: evidence_bundle
                .runtime_manifest
                .manifest_digest
                .clone(),
            compile_trace_proof_digest: evidence_bundle.trace_proof.proof_digest.clone(),
            compile_execution_proof_bundle_digest: evidence_bundle.proof_bundle.stable_digest(),
        };
        artifact.refresh_digest();
        artifact
    }

    fn refresh_digest(&mut self) {
        self.artifact_digest =
            stable_serialized_digest(b"tassadar_compiled_program_weight_artifact|", self);
    }
}

/// Explicit runtime contract for serving or replaying one compiled-weight executor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledProgramRuntimeContract {
    /// Stable contract identifier.
    pub contract_id: String,
    /// Stable digest over the runtime contract.
    pub contract_digest: String,
    /// Stable deployment kind.
    pub deployment_kind: TassadarCompiledExecutorDeploymentKind,
    /// Stable model-descriptor digest for the compiled deployment.
    pub model_descriptor_digest: String,
    /// Stable program-artifact digest this runtime contract accepts.
    pub program_artifact_digest: String,
    /// Stable validated-program digest this runtime contract accepts.
    pub program_digest: String,
    /// Runtime backend that realizes the compiled deployment.
    pub runtime_backend: String,
    /// Reference runner identity that anchored compile-time exactness.
    pub reference_runner_id: String,
    /// Decode modes this runtime contract can support honestly.
    pub supported_decode_modes: Vec<TassadarExecutorDecodeMode>,
    /// Default decode mode for callers that do not request a fast path.
    pub default_decode_mode: TassadarExecutorDecodeMode,
    /// Stable Wasm profile identifier enforced by the contract.
    pub wasm_profile_id: String,
    /// Stable trace ABI identifier enforced by the contract.
    pub trace_abi_id: String,
    /// Stable trace ABI version enforced by the contract.
    pub trace_abi_version: u16,
    /// Stable compile-time runtime-manifest digest.
    pub compile_runtime_manifest_digest: String,
    /// Stable compile-time proof-bundle digest.
    pub compile_execution_proof_bundle_digest: String,
}

impl TassadarCompiledProgramRuntimeContract {
    fn new(
        contract_id: impl Into<String>,
        model_descriptor_digest: impl Into<String>,
        compiled_weight_artifact: &TassadarCompiledProgramWeightArtifact,
        reference_runner_id: impl Into<String>,
    ) -> Self {
        let mut supported_decode_modes = compiled_weight_artifact.supported_decode_modes.clone();
        supported_decode_modes.sort_by_key(|mode| mode.as_str());
        let mut contract = Self {
            contract_id: contract_id.into(),
            contract_digest: String::new(),
            deployment_kind: compiled_weight_artifact.deployment_kind,
            model_descriptor_digest: model_descriptor_digest.into(),
            program_artifact_digest: compiled_weight_artifact.program_artifact_digest.clone(),
            program_digest: compiled_weight_artifact.program_digest.clone(),
            runtime_backend: String::from("cpu"),
            reference_runner_id: reference_runner_id.into(),
            supported_decode_modes,
            default_decode_mode: TassadarExecutorDecodeMode::ReferenceLinear,
            wasm_profile_id: compiled_weight_artifact.wasm_profile_id.clone(),
            trace_abi_id: compiled_weight_artifact.trace_abi_id.clone(),
            trace_abi_version: compiled_weight_artifact.trace_abi_version,
            compile_runtime_manifest_digest: compiled_weight_artifact
                .compile_runtime_manifest_digest
                .clone(),
            compile_execution_proof_bundle_digest: compiled_weight_artifact
                .compile_execution_proof_bundle_digest
                .clone(),
        };
        contract.refresh_digest();
        contract
    }

    fn refresh_digest(&mut self) {
        self.contract_digest =
            stable_serialized_digest(b"tassadar_compiled_program_runtime_contract|", self);
    }
}

/// One compiled-weight deployment plus its explicit runtime contract.
#[derive(Clone, Debug, PartialEq)]
pub struct TassadarCompiledProgramExecutor {
    descriptor: TassadarExecutorModelDescriptor,
    weight_bundle: TassadarCompiledProgramWeightBundle,
    compiled_weight_artifact: TassadarCompiledProgramWeightArtifact,
    runtime_contract: TassadarCompiledProgramRuntimeContract,
    compile_evidence_bundle: TassadarExecutionEvidenceBundle,
}

impl TassadarCompiledProgramExecutor {
    /// Compiles one digest-bound program artifact into a program-specialized
    /// compiled-weight executor.
    pub fn compile(
        artifact_id: impl Into<String>,
        base_fixture: &TassadarExecutorFixture,
        program_artifact: &TassadarProgramArtifact,
    ) -> Result<Self, TassadarCompiledProgramError> {
        base_fixture
            .descriptor()
            .validate_program_artifact(
                program_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            )
            .map_err(|error| TassadarCompiledProgramError::DescriptorContract { error })?;
        let artifact_id = artifact_id.into();
        let weight_bundle = build_compiled_program_weight_bundle(
            &artifact_id,
            program_artifact,
            &base_fixture.descriptor().profile,
            &base_fixture.descriptor().trace_abi,
            base_fixture
                .descriptor()
                .compatibility
                .supported_decode_modes
                .as_slice(),
        );
        let descriptor = build_compiled_program_descriptor(
            &artifact_id,
            base_fixture,
            program_artifact,
            &weight_bundle,
        );
        let descriptor_digest = descriptor.stable_digest();
        let execution_report = execute_tassadar_executor_request(
            &program_artifact.validated_program,
            TassadarExecutorDecodeMode::ReferenceLinear,
            descriptor.trace_abi.schema_version,
            Some(descriptor.compatibility.supported_decode_modes.as_slice()),
        )
        .map_err(
            |diagnostic| TassadarCompiledProgramError::SelectionRefused {
                detail: diagnostic.detail,
            },
        )?;
        let effective_decode_mode = execution_report
            .selection
            .effective_decode_mode
            .unwrap_or(TassadarExecutorDecodeMode::ReferenceLinear);
        let evidence_bundle = build_tassadar_execution_evidence_bundle(
            format!("compile-{}", artifact_id),
            stable_request_digest(
                descriptor_digest.as_str(),
                program_artifact.artifact_digest.as_str(),
                TassadarExecutorDecodeMode::ReferenceLinear,
            ),
            "psionic.tassadar.compiled_weight",
            descriptor.model.model_id.clone(),
            descriptor_digest.clone(),
            vec![format!("compiled://{}", artifact_id)],
            program_artifact,
            effective_decode_mode,
            &execution_report.execution,
        );
        let compiled_weight_artifact = TassadarCompiledProgramWeightArtifact::new(
            format!("{artifact_id}.compiled_weight"),
            base_fixture.descriptor().model.model_id.clone(),
            base_fixture.descriptor().stable_digest(),
            program_artifact,
            descriptor.compatibility.supported_decode_modes.as_slice(),
            &weight_bundle,
            &evidence_bundle,
            execution_report.execution.behavior_digest(),
        );
        let runtime_contract = TassadarCompiledProgramRuntimeContract::new(
            format!("{artifact_id}.runtime_contract"),
            descriptor_digest,
            &compiled_weight_artifact,
            execution_report.execution.runner_id.clone(),
        );
        Ok(Self {
            descriptor,
            weight_bundle,
            compiled_weight_artifact,
            runtime_contract,
            compile_evidence_bundle: evidence_bundle,
        })
    }

    /// Returns the compiled deployment descriptor.
    #[must_use]
    pub fn descriptor(&self) -> &TassadarExecutorModelDescriptor {
        &self.descriptor
    }

    /// Returns the compiled weight bundle.
    #[must_use]
    pub fn weight_bundle(&self) -> &TassadarCompiledProgramWeightBundle {
        &self.weight_bundle
    }

    /// Returns the compiled-weight artifact record.
    #[must_use]
    pub fn compiled_weight_artifact(&self) -> &TassadarCompiledProgramWeightArtifact {
        &self.compiled_weight_artifact
    }

    /// Returns the explicit runtime contract for the compiled deployment.
    #[must_use]
    pub fn runtime_contract(&self) -> &TassadarCompiledProgramRuntimeContract {
        &self.runtime_contract
    }

    /// Returns the compile-time evidence bundle that bound the deployment to
    /// its source program, runtime manifest, and proof bundle.
    #[must_use]
    pub fn compile_evidence_bundle(&self) -> &TassadarExecutionEvidenceBundle {
        &self.compile_evidence_bundle
    }

    /// Returns the current runtime capability report for the compiled deployment.
    #[must_use]
    pub fn runtime_capability_report(&self) -> TassadarRuntimeCapabilityReport {
        tassadar_runtime_capability_report()
    }

    /// Validates one program artifact against the compiled deployment contract.
    pub fn validate_program_artifact(
        &self,
        artifact: &TassadarProgramArtifact,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<(), TassadarCompiledProgramError> {
        self.descriptor
            .validate_program_artifact(artifact, requested_decode_mode)
            .map_err(|error| TassadarCompiledProgramError::DescriptorContract { error })?;
        if artifact.artifact_digest != self.runtime_contract.program_artifact_digest {
            return Err(
                TassadarCompiledProgramError::ProgramArtifactDigestMismatch {
                    expected: self.runtime_contract.program_artifact_digest.clone(),
                    actual: artifact.artifact_digest.clone(),
                },
            );
        }
        if artifact.validated_program_digest != self.runtime_contract.program_digest {
            return Err(TassadarCompiledProgramError::ProgramDigestMismatch {
                expected: self.runtime_contract.program_digest.clone(),
                actual: artifact.validated_program_digest.clone(),
            });
        }
        Ok(())
    }

    /// Executes the compiled deployment against its exact matching program artifact.
    pub fn execute(
        &self,
        artifact: &TassadarProgramArtifact,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Result<TassadarCompiledProgramExecution, TassadarCompiledProgramError> {
        self.validate_program_artifact(artifact, requested_decode_mode)?;
        let execution_report = execute_tassadar_executor_request(
            &artifact.validated_program,
            requested_decode_mode,
            self.descriptor.trace_abi.schema_version,
            Some(
                self.descriptor
                    .compatibility
                    .supported_decode_modes
                    .as_slice(),
            ),
        )
        .map_err(
            |diagnostic| TassadarCompiledProgramError::SelectionRefused {
                detail: diagnostic.detail,
            },
        )?;
        let effective_decode_mode = execution_report
            .selection
            .effective_decode_mode
            .unwrap_or(TassadarExecutorDecodeMode::ReferenceLinear);
        let evidence_bundle = build_tassadar_execution_evidence_bundle(
            format!("execute-{}", self.compiled_weight_artifact.artifact_id),
            stable_request_digest(
                self.descriptor.stable_digest().as_str(),
                artifact.artifact_digest.as_str(),
                requested_decode_mode,
            ),
            "psionic.tassadar.compiled_executor",
            self.descriptor.model.model_id.clone(),
            self.descriptor.stable_digest(),
            vec![format!(
                "compiled://{}",
                self.compiled_weight_artifact.artifact_id
            )],
            artifact,
            effective_decode_mode,
            &execution_report.execution,
        );
        Ok(TassadarCompiledProgramExecution {
            execution_report,
            evidence_bundle,
        })
    }
}

/// One execution realized through the compiled-weight deployment contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledProgramExecution {
    /// Selection diagnostic plus exact execution report.
    pub execution_report: TassadarExecutorExecutionReport,
    /// Runtime-manifest and proof-bundle evidence for the execution.
    pub evidence_bundle: TassadarExecutionEvidenceBundle,
}

/// Suite-level compiled-weight artifact set used by research sweeps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledProgramSuiteArtifact {
    /// Stable suite artifact identifier.
    pub artifact_id: String,
    /// Stable digest over the suite artifact.
    pub artifact_digest: String,
    /// Stable suite reference such as a benchmark-ref/version pair.
    pub suite_ref: String,
    /// Stable digest of the base model descriptor used across the suite.
    pub base_model_descriptor_digest: String,
    /// Ordered compiled deployments for the suite.
    pub deployments: Vec<TassadarCompiledProgramWeightArtifact>,
    /// Aggregate serialized artifact bytes across the suite.
    pub total_compiled_weight_artifact_bytes: u64,
}

impl TassadarCompiledProgramSuiteArtifact {
    /// Compiles a full suite of digest-bound programs into program-specialized
    /// compiled-weight artifacts.
    pub fn compile(
        artifact_id: impl Into<String>,
        suite_ref: impl Into<String>,
        base_fixture: &TassadarExecutorFixture,
        program_artifacts: &[TassadarProgramArtifact],
    ) -> Result<Self, TassadarCompiledProgramError> {
        let mut deployments = Vec::with_capacity(program_artifacts.len());
        for artifact in program_artifacts {
            let deployment = base_fixture.compile_program(
                format!(
                    "{}.{}",
                    base_fixture.descriptor().model.model_id,
                    artifact.validated_program.program_id
                ),
                artifact,
            )?;
            deployments.push(deployment.compiled_weight_artifact().clone());
        }
        deployments.sort_by(|left, right| left.program_digest.cmp(&right.program_digest));
        let total_compiled_weight_artifact_bytes = deployments
            .iter()
            .map(|deployment| deployment.compiled_weight_artifact_bytes)
            .sum();
        let mut artifact = Self {
            artifact_id: artifact_id.into(),
            artifact_digest: String::new(),
            suite_ref: suite_ref.into(),
            base_model_descriptor_digest: base_fixture.descriptor().stable_digest(),
            deployments,
            total_compiled_weight_artifact_bytes,
        };
        artifact.refresh_digest();
        Ok(artifact)
    }

    fn refresh_digest(&mut self) {
        self.artifact_digest =
            stable_serialized_digest(b"tassadar_compiled_program_suite_artifact|", self);
    }
}

/// Typed failures while building or executing a program-specialized
/// compiled-weight deployment.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarCompiledProgramError {
    /// The generic executor descriptor contract rejected the program artifact.
    #[error("compiled program descriptor contract failed: {error}")]
    DescriptorContract {
        /// Typed contract failure.
        error: TassadarExecutorContractError,
    },
    /// The runtime refused the requested decode mode for the compiled program.
    #[error("compiled program selection refused: {detail}")]
    SelectionRefused {
        /// Human-readable refusal detail.
        detail: String,
    },
    /// The caller supplied the wrong program artifact for the compiled deployment.
    #[error("compiled program artifact mismatch: expected `{expected}`, got `{actual}`")]
    ProgramArtifactDigestMismatch {
        /// Expected program-artifact digest.
        expected: String,
        /// Actual supplied program-artifact digest.
        actual: String,
    },
    /// The caller supplied the wrong validated program for the compiled deployment.
    #[error("compiled program digest mismatch: expected `{expected}`, got `{actual}`")]
    ProgramDigestMismatch {
        /// Expected validated-program digest.
        expected: String,
        /// Actual supplied validated-program digest.
        actual: String,
    },
}

fn workload_capability_matrix_for_fixture(
    fixture: &TassadarExecutorFixture,
) -> TassadarWorkloadCapabilityMatrix {
    let descriptor = fixture.descriptor();
    let supported_decode_modes = descriptor.compatibility.supported_decode_modes.as_slice();
    let rows = match descriptor.model.model_id.as_str() {
        value if value == TassadarExecutorFixture::MODEL_ID => {
            let gate = validation_benchmark_gate();
            TassadarWorkloadClass::all()
                .into_iter()
                .map(|workload_class| match workload_class {
                    TassadarWorkloadClass::ArithmeticMicroprogram => exact_workload_row(
                        workload_class,
                        supported_decode_modes,
                        Some(gate.clone()),
                        "published exact on the validation-corpus arithmetic microprogram lane",
                    ),
                    TassadarWorkloadClass::ClrsShortestPath => exact_workload_row(
                        workload_class,
                        supported_decode_modes,
                        Some(gate.clone()),
                        "published exact on the bounded shortest-path witness in the validation corpus",
                    ),
                    TassadarWorkloadClass::MemoryLookupMicroprogram => exact_workload_row(
                        workload_class,
                        supported_decode_modes,
                        Some(gate.clone()),
                        "published exact on the validation-corpus memory lookup microprogram lane",
                    ),
                    TassadarWorkloadClass::BranchControlFlowMicroprogram => exact_workload_row(
                        workload_class,
                        supported_decode_modes,
                        Some(gate.clone()),
                        "published exact on the validation-corpus branch/control-flow microprogram lane",
                    ),
                    _ => unsupported_workload_row(
                        workload_class,
                        vec![TassadarWorkloadCapabilityRefusalReason::WorkloadClassOutOfScope],
                        "reference_fixture publication stops at the validation microprogram workload classes",
                    ),
                })
                .collect()
        }
        value if value == TassadarExecutorFixture::CORE_I32_V2_MODEL_ID => TassadarWorkloadClass::all()
            .into_iter()
            .map(|workload_class| {
                unsupported_workload_row(
                    workload_class,
                    vec![TassadarWorkloadCapabilityRefusalReason::BenchmarkGateMissing],
                    "core_i32_v2 is widened runtime substrate, but it stays unpublished until a public benchmark gate lands",
                )
            })
            .collect(),
        value if value == TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID => {
            let gate = article_benchmark_gate();
            TassadarWorkloadClass::all()
                .into_iter()
                .map(|workload_class| match workload_class {
                    TassadarWorkloadClass::MicroWasmKernel => exact_workload_row(
                        workload_class,
                        supported_decode_modes,
                        Some(gate.clone()),
                        "published exact on the article micro-kernel benchmark family",
                    ),
                    TassadarWorkloadClass::BranchHeavyKernel => fallback_workload_row(
                        workload_class,
                        supported_decode_modes,
                        TassadarExecutorDecodeMode::ReferenceLinear,
                        Some(gate.clone()),
                        "published exact only with explicit fallback to reference_linear when the requested fast path exceeds the validated branch-heavy envelope",
                    ),
                    TassadarWorkloadClass::MemoryHeavyKernel => exact_workload_row(
                        workload_class,
                        supported_decode_modes,
                        Some(gate.clone()),
                        "published exact on the current memory-heavy article kernel family",
                    ),
                    TassadarWorkloadClass::LongLoopKernel => fallback_workload_row(
                        workload_class,
                        supported_decode_modes,
                        TassadarExecutorDecodeMode::ReferenceLinear,
                        Some(gate.clone()),
                        "published exact only with explicit fallback to reference_linear on the current long-loop horizon family",
                    ),
                    TassadarWorkloadClass::SudokuClass => fallback_workload_row(
                        workload_class,
                        supported_decode_modes,
                        TassadarExecutorDecodeMode::ReferenceLinear,
                        Some(gate.clone()),
                        "published exact only with explicit fallback to reference_linear while Sudoku search fast paths remain outside the current direct closure",
                    ),
                    TassadarWorkloadClass::HungarianMatching => exact_workload_row(
                        workload_class,
                        supported_decode_modes,
                        Some(gate.clone()),
                        "published exact on the current Hungarian matching article family",
                    ),
                    _ => unsupported_workload_row(
                        workload_class,
                        vec![TassadarWorkloadCapabilityRefusalReason::BenchmarkGateMissing],
                        "article_i32_compute publication is benchmark-gated only for the article workload families",
                    ),
                })
                .collect()
        }
        value
            if value == TassadarExecutorFixture::SUDOKU_V0_SEARCH_MODEL_ID
                || value == TassadarExecutorFixture::SUDOKU_9X9_SEARCH_MODEL_ID
                || value == TassadarExecutorFixture::SUDOKU_9X9_HARD_SEARCH_MODEL_ID =>
        {
            exact_single_workload_matrix_rows(
                TassadarWorkloadClass::SudokuClass,
                supported_decode_modes,
                public_package_set_benchmark_gate(),
                "published exact only for the exact Sudoku search workload family under the public benchmark-package-set gate",
            )
        }
        value
            if value == TassadarExecutorFixture::HUNGARIAN_V0_MATCHING_MODEL_ID
                || value == TassadarExecutorFixture::HUNGARIAN_10X10_MATCHING_MODEL_ID =>
        {
            exact_single_workload_matrix_rows(
                TassadarWorkloadClass::HungarianMatching,
                supported_decode_modes,
                public_package_set_benchmark_gate(),
                "published exact only for the exact Hungarian matching workload family under the public benchmark-package-set gate",
            )
        }
        _ => TassadarWorkloadClass::all()
            .into_iter()
            .map(|workload_class| {
                unsupported_workload_row(
                    workload_class,
                    vec![TassadarWorkloadCapabilityRefusalReason::WorkloadClassOutOfScope],
                    "this executor fixture does not publish workload capability outside its declared fixture lane",
                )
            })
            .collect(),
    };
    let claim_boundary = match descriptor.model.model_id.as_str() {
        value if value == TassadarExecutorFixture::MODEL_ID => {
            "reference_fixture publishes only the validation-corpus microprogram workload classes; article, Sudoku, and matching claims remain out of scope"
        }
        value if value == TassadarExecutorFixture::CORE_I32_V2_MODEL_ID => {
            "core_i32_v2 remains runtime substrate only; served capability publication stays closed until a benchmark gate lands"
        }
        value if value == TassadarExecutorFixture::ARTICLE_I32_COMPUTE_MODEL_ID => {
            "article_i32_compute publishes only the article benchmark families; validation microprogram publication remains owned by reference_fixture"
        }
        value
            if value == TassadarExecutorFixture::SUDOKU_V0_SEARCH_MODEL_ID
                || value == TassadarExecutorFixture::SUDOKU_9X9_SEARCH_MODEL_ID
                || value == TassadarExecutorFixture::SUDOKU_9X9_HARD_SEARCH_MODEL_ID =>
        {
            "Sudoku fixture publication is exact only for Sudoku-class search workloads"
        }
        value
            if value == TassadarExecutorFixture::HUNGARIAN_V0_MATCHING_MODEL_ID
                || value == TassadarExecutorFixture::HUNGARIAN_10X10_MATCHING_MODEL_ID =>
        {
            "Hungarian fixture publication is exact only for Hungarian matching workloads"
        }
        _ => "fixture publication remains closed outside explicit workload rows",
    };
    TassadarWorkloadCapabilityMatrix::new(
        descriptor.model.model_id.clone(),
        descriptor.model.family.clone(),
        rows,
        claim_boundary,
    )
}

pub(crate) fn research_only_workload_capability_matrix(
    model: &ModelDescriptor,
    supported_decode_modes: &[TassadarExecutorDecodeMode],
    primary_workload_class: TassadarWorkloadClass,
    claim_boundary: impl Into<String>,
    primary_detail: impl Into<String>,
) -> TassadarWorkloadCapabilityMatrix {
    let primary_detail = primary_detail.into();
    let rows = TassadarWorkloadClass::all()
        .into_iter()
        .map(|workload_class| {
            if workload_class == primary_workload_class {
                research_only_workload_row(
                    workload_class,
                    supported_decode_modes,
                    primary_detail.clone(),
                )
            } else {
                unsupported_workload_row(
                    workload_class,
                    vec![TassadarWorkloadCapabilityRefusalReason::WorkloadClassOutOfScope],
                    "this research lane does not publish served capability outside its primary workload class",
                )
            }
        })
        .collect();
    TassadarWorkloadCapabilityMatrix::new(
        model.model_id.clone(),
        model.family.clone(),
        rows,
        claim_boundary,
    )
}

pub(crate) fn workload_class_for_tassadar_model_id(
    model_id: &str,
) -> Option<TassadarWorkloadClass> {
    if model_id.contains("sudoku") {
        Some(TassadarWorkloadClass::SudokuClass)
    } else if model_id.contains("hungarian") {
        Some(TassadarWorkloadClass::HungarianMatching)
    } else if model_id.contains("article-i32-compute") {
        Some(TassadarWorkloadClass::MicroWasmKernel)
    } else if model_id.contains("core-i32") || model_id.contains("fixture") {
        Some(TassadarWorkloadClass::ArithmeticMicroprogram)
    } else {
        None
    }
}

fn exact_single_workload_matrix_rows(
    primary_workload_class: TassadarWorkloadClass,
    supported_decode_modes: &[TassadarExecutorDecodeMode],
    benchmark_gate: TassadarWorkloadBenchmarkGate,
    supported_detail: &str,
) -> Vec<TassadarWorkloadCapabilityRow> {
    TassadarWorkloadClass::all()
        .into_iter()
        .map(|workload_class| {
            if workload_class == primary_workload_class {
                exact_workload_row(
                    workload_class,
                    supported_decode_modes,
                    Some(benchmark_gate.clone()),
                    supported_detail,
                )
            } else {
                unsupported_workload_row(
                    workload_class,
                    vec![TassadarWorkloadCapabilityRefusalReason::WorkloadClassOutOfScope],
                    "this exact fixture publishes only its dedicated workload family",
                )
            }
        })
        .collect()
}

fn exact_workload_row(
    workload_class: TassadarWorkloadClass,
    supported_decode_modes: &[TassadarExecutorDecodeMode],
    benchmark_gate: Option<TassadarWorkloadBenchmarkGate>,
    detail: impl Into<String>,
) -> TassadarWorkloadCapabilityRow {
    TassadarWorkloadCapabilityRow {
        workload_class,
        support_posture: TassadarWorkloadSupportPosture::Exact,
        supported_decode_modes: sorted_supported_decode_modes(supported_decode_modes),
        exact_fallback_decode_mode: None,
        refusal_reasons: Vec::new(),
        benchmark_gate,
        detail: detail.into(),
    }
}

fn fallback_workload_row(
    workload_class: TassadarWorkloadClass,
    supported_decode_modes: &[TassadarExecutorDecodeMode],
    exact_fallback_decode_mode: TassadarExecutorDecodeMode,
    benchmark_gate: Option<TassadarWorkloadBenchmarkGate>,
    detail: impl Into<String>,
) -> TassadarWorkloadCapabilityRow {
    TassadarWorkloadCapabilityRow {
        workload_class,
        support_posture: TassadarWorkloadSupportPosture::ExactFallbackOnly,
        supported_decode_modes: sorted_supported_decode_modes(supported_decode_modes),
        exact_fallback_decode_mode: Some(exact_fallback_decode_mode),
        refusal_reasons: Vec::new(),
        benchmark_gate,
        detail: detail.into(),
    }
}

fn research_only_workload_row(
    workload_class: TassadarWorkloadClass,
    supported_decode_modes: &[TassadarExecutorDecodeMode],
    detail: impl Into<String>,
) -> TassadarWorkloadCapabilityRow {
    TassadarWorkloadCapabilityRow {
        workload_class,
        support_posture: TassadarWorkloadSupportPosture::ResearchOnly,
        supported_decode_modes: sorted_supported_decode_modes(supported_decode_modes),
        exact_fallback_decode_mode: None,
        refusal_reasons: vec![TassadarWorkloadCapabilityRefusalReason::ClaimBoundaryUnvalidated],
        benchmark_gate: None,
        detail: detail.into(),
    }
}

fn unsupported_workload_row(
    workload_class: TassadarWorkloadClass,
    refusal_reasons: Vec<TassadarWorkloadCapabilityRefusalReason>,
    detail: impl Into<String>,
) -> TassadarWorkloadCapabilityRow {
    TassadarWorkloadCapabilityRow {
        workload_class,
        support_posture: TassadarWorkloadSupportPosture::Unsupported,
        supported_decode_modes: Vec::new(),
        exact_fallback_decode_mode: None,
        refusal_reasons,
        benchmark_gate: None,
        detail: detail.into(),
    }
}

fn sorted_supported_decode_modes(
    supported_decode_modes: &[TassadarExecutorDecodeMode],
) -> Vec<TassadarExecutorDecodeMode> {
    let mut supported_decode_modes = supported_decode_modes.to_vec();
    supported_decode_modes.sort_by_key(|mode| mode.as_str());
    supported_decode_modes.dedup();
    supported_decode_modes
}

fn validation_benchmark_gate() -> TassadarWorkloadBenchmarkGate {
    TassadarWorkloadBenchmarkGate {
        benchmark_gate_ref: String::from(TASSADAR_REFERENCE_FIXTURE_BENCHMARK_REF),
        evidence_ref: String::from(TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_REPORT_REF),
    }
}

fn article_benchmark_gate() -> TassadarWorkloadBenchmarkGate {
    TassadarWorkloadBenchmarkGate {
        benchmark_gate_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REF),
        evidence_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
    }
}

fn public_package_set_benchmark_gate() -> TassadarWorkloadBenchmarkGate {
    TassadarWorkloadBenchmarkGate {
        benchmark_gate_ref: String::from(TASSADAR_PUBLIC_BENCHMARK_PACKAGE_SET_REF),
        evidence_ref: String::from(TASSADAR_BENCHMARK_PACKAGE_SET_SUMMARY_REPORT_REF),
    }
}

fn build_weight_bundle(
    runtime_weights: &RuntimeTassadarFixtureWeights,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
) -> TassadarExecutorWeightBundle {
    let opcode_stack_effects = runtime_weights
        .opcode_rules
        .iter()
        .flat_map(|rule| [f32::from(rule.pops), f32::from(rule.pushes)])
        .collect::<Vec<_>>();
    let opcode_semantics = runtime_weights
        .opcode_rules
        .iter()
        .flat_map(|rule| {
            [
                f32::from(rule.opcode.ordinal()),
                f32::from(rule.immediate_kind.code()),
                f32::from(rule.access_class.code()),
                f32::from(rule.control_class.code()),
                rule.arithmetic.map_or(0.0, |arith| f32::from(arith.code())),
            ]
        })
        .collect::<Vec<_>>();
    let profile_limits = vec![
        profile.max_locals as f32,
        profile.max_memory_slots as f32,
        profile.max_program_len as f32,
        profile.max_steps as f32,
    ];
    let trace_abi_flags = vec![
        trace_abi.schema_version as f32,
        if trace_abi.append_only { 1.0 } else { 0.0 },
        if trace_abi.includes_stack_snapshots {
            1.0
        } else {
            0.0
        },
        if trace_abi.includes_local_snapshots {
            1.0
        } else {
            0.0
        },
        if trace_abi.includes_memory_snapshots {
            1.0
        } else {
            0.0
        },
        runtime_weights.opcode_rules.len() as f32,
    ];

    let entries = vec![
        (
            WeightTensorMetadata::new(
                "opcode_semantics",
                Shape::new(vec![runtime_weights.opcode_rules.len(), 5]),
                DType::F32,
            ),
            opcode_semantics.as_slice(),
        ),
        (
            WeightTensorMetadata::new(
                "opcode_stack_effects",
                Shape::new(vec![runtime_weights.opcode_rules.len(), 2]),
                DType::F32,
            ),
            opcode_stack_effects.as_slice(),
        ),
        (
            WeightTensorMetadata::new("profile_limits", Shape::new(vec![4]), DType::F32),
            profile_limits.as_slice(),
        ),
        (
            WeightTensorMetadata::new("trace_abi_flags", Shape::new(vec![6]), DType::F32),
            trace_abi_flags.as_slice(),
        ),
    ];
    let metadata = build_metadata(&entries, WeightSource::Fixture, Vec::new());

    TassadarExecutorWeightBundle {
        metadata,
        opcode_stack_effects,
        opcode_semantics,
        profile_limits,
        trace_abi_flags,
    }
}

fn build_metadata(
    entries: &[(WeightTensorMetadata, &[f32])],
    source: WeightSource,
    artifacts: Vec<WeightArtifactMetadata>,
) -> WeightBundleMetadata {
    let mut ordered = entries.to_vec();
    ordered.sort_by(|(left, _), (right, _)| left.name.cmp(&right.name));

    let mut hasher = Sha256::new();
    for (metadata, values) in &ordered {
        digest_tensor_values(&mut hasher, metadata, values);
    }

    WeightBundleMetadata {
        format: WeightFormat::ProgrammaticFixture,
        source,
        quantization: QuantizationMode::None,
        quantization_modes: Vec::new(),
        digest: hex::encode(hasher.finalize()),
        tensors: ordered
            .iter()
            .map(|(metadata, _)| metadata.clone())
            .collect(),
        artifacts,
    }
}

fn digest_tensor_values(hasher: &mut Sha256, metadata: &WeightTensorMetadata, values: &[f32]) {
    hasher.update(metadata.name.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.dtype).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", metadata.quantization).as_bytes());
    hasher.update(b"|");
    for dimension in metadata.shape.dims() {
        hasher.update(dimension.to_be_bytes());
    }
    hasher.update(b"|");
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

fn infer_executor_ingress_surface(
    weights: &WeightBundleMetadata,
    artifact_governance: Option<&ModelArtifactGovernance>,
) -> ModelIngressSurface {
    artifact_governance
        .map(ModelArtifactGovernance::ingress_surface)
        .unwrap_or_else(|| {
            if weights.source == WeightSource::Fixture && !weights.is_artifact_backed() {
                ModelIngressSurface::Fixture
            } else if weights.is_artifact_backed() {
                ModelIngressSurface::DirectArtifactImport
            } else {
                ModelIngressSurface::PsionicNativeBundle
            }
        })
}

fn build_compiled_program_descriptor(
    artifact_id: &str,
    base_fixture: &TassadarExecutorFixture,
    program_artifact: &TassadarProgramArtifact,
    weight_bundle: &TassadarCompiledProgramWeightBundle,
) -> TassadarExecutorModelDescriptor {
    let compatibility = TassadarExecutorCompatibility {
        executor_family: TassadarExecutorFamily::WasmTraceExecutor,
        trace_abi_id: base_fixture.descriptor().trace_abi.abi_id.clone(),
        trace_abi_version: base_fixture.descriptor().trace_abi.schema_version,
        wasm_profile_id: base_fixture.descriptor().profile.profile_id.clone(),
        opcode_vocabulary_digest: base_fixture
            .descriptor()
            .compatibility
            .opcode_vocabulary_digest
            .clone(),
        supported_decode_modes: base_fixture
            .descriptor()
            .compatibility
            .supported_decode_modes
            .clone(),
        attention_mode: TassadarExecutorAttentionMode::ProgramSpecializedCompiled,
        attention_geometry: base_fixture
            .descriptor()
            .compatibility
            .attention_geometry
            .clone(),
        exactness_posture: TassadarExecutorExactnessPosture::ExactTraceAndOutput,
    };
    TassadarExecutorModelDescriptor::new(
        ModelDescriptor::new(
            format!(
                "{}-compiled-{}",
                base_fixture.descriptor().model.model_id,
                program_artifact.validated_program.program_id
            ),
            format!("{}.compiled", TassadarExecutorFixture::MODEL_FAMILY),
            artifact_id,
        ),
        compatibility,
        base_fixture.descriptor().profile.clone(),
        base_fixture.descriptor().trace_abi.clone(),
        weight_bundle.metadata().clone(),
    )
}

fn build_compiled_program_weight_bundle(
    artifact_id: &str,
    program_artifact: &TassadarProgramArtifact,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    supported_decode_modes: &[TassadarExecutorDecodeMode],
) -> TassadarCompiledProgramWeightBundle {
    let compiled_program_header = vec![
        program_artifact.validated_program.local_count as f32,
        program_artifact.validated_program.memory_slots as f32,
        program_artifact.validated_program.instructions.len() as f32,
        program_artifact.validated_program.initial_memory.len() as f32,
        profile.max_program_len as f32,
        profile.max_steps as f32,
        trace_abi.schema_version as f32,
        supported_decode_modes.len() as f32,
    ];
    let mut compiled_instruction_stream = vec![0.0_f32; profile.max_program_len * 4];
    for (index, instruction) in program_artifact
        .validated_program
        .instructions
        .iter()
        .enumerate()
    {
        let encoded = encode_compiled_instruction(index, instruction);
        let offset = index * 4;
        compiled_instruction_stream[offset..offset + 4].copy_from_slice(&encoded);
    }
    let mut compiled_initial_memory = vec![0.0_f32; profile.max_memory_slots];
    for (index, value) in program_artifact
        .validated_program
        .initial_memory
        .iter()
        .enumerate()
    {
        compiled_initial_memory[index] = *value as f32;
    }
    let compiled_decode_contract = vec![
        if supported_decode_modes.contains(&TassadarExecutorDecodeMode::ReferenceLinear) {
            1.0
        } else {
            0.0
        },
        if supported_decode_modes.contains(&TassadarExecutorDecodeMode::HullCache) {
            1.0
        } else {
            0.0
        },
        if supported_decode_modes.contains(&TassadarExecutorDecodeMode::SparseTopK) {
            1.0
        } else {
            0.0
        },
        decode_mode_code(TassadarExecutorDecodeMode::ReferenceLinear),
        if trace_abi.append_only { 1.0 } else { 0.0 },
        1.0,
    ];
    let entries = vec![
        (
            WeightTensorMetadata::new(
                "compiled_decode_contract",
                Shape::new(vec![compiled_decode_contract.len()]),
                DType::F32,
            ),
            compiled_decode_contract.as_slice(),
        ),
        (
            WeightTensorMetadata::new(
                "compiled_initial_memory",
                Shape::new(vec![compiled_initial_memory.len()]),
                DType::F32,
            ),
            compiled_initial_memory.as_slice(),
        ),
        (
            WeightTensorMetadata::new(
                "compiled_instruction_stream",
                Shape::new(vec![profile.max_program_len, 4]),
                DType::F32,
            ),
            compiled_instruction_stream.as_slice(),
        ),
        (
            WeightTensorMetadata::new(
                "compiled_program_header",
                Shape::new(vec![compiled_program_header.len()]),
                DType::F32,
            ),
            compiled_program_header.as_slice(),
        ),
    ];
    let artifact_bytes = serde_json::to_vec(&CompiledWeightArtifactEncoding {
        artifact_id,
        program_artifact_digest: program_artifact.artifact_digest.as_str(),
        compiled_program_header: compiled_program_header.as_slice(),
        compiled_instruction_stream: compiled_instruction_stream.as_slice(),
        compiled_initial_memory: compiled_initial_memory.as_slice(),
        compiled_decode_contract: compiled_decode_contract.as_slice(),
    })
    .expect("compiled Tassadar weight bundle should serialize");
    let artifact_metadata = vec![WeightArtifactMetadata::new(
        format!("{artifact_id}.compiled_weight.json"),
        artifact_bytes.len() as u64,
        hex::encode(Sha256::digest(artifact_bytes)),
    )];
    let metadata = build_metadata(&entries, WeightSource::ExternalArtifact, artifact_metadata);
    TassadarCompiledProgramWeightBundle {
        metadata,
        compiled_program_header,
        compiled_instruction_stream,
        compiled_initial_memory,
        compiled_decode_contract,
    }
}

fn encode_compiled_instruction(
    index: usize,
    instruction: &psionic_runtime::TassadarInstruction,
) -> [f32; 4] {
    use psionic_runtime::TassadarInstruction;

    let (immediate_kind, immediate_value) = match instruction {
        TassadarInstruction::I32Const { value } => (1.0, *value as f32),
        TassadarInstruction::LocalGet { local }
        | TassadarInstruction::LocalSet { local }
        | TassadarInstruction::LocalTee { local } => (2.0, f32::from(*local)),
        TassadarInstruction::I32Load { slot } | TassadarInstruction::I32Store { slot } => {
            (3.0, f32::from(*slot))
        }
        TassadarInstruction::BrIf { target_pc } => (4.0, f32::from(*target_pc)),
        TassadarInstruction::Nop
        | TassadarInstruction::Drop
        | TassadarInstruction::I32Add
        | TassadarInstruction::I32Sub
        | TassadarInstruction::I32Mul
        | TassadarInstruction::I32Lt
        | TassadarInstruction::I32Eqz
        | TassadarInstruction::I32Eq
        | TassadarInstruction::I32Ne
        | TassadarInstruction::I32Gt
        | TassadarInstruction::I32Le
        | TassadarInstruction::I32Ge
        | TassadarInstruction::Output
        | TassadarInstruction::Return => (0.0, 0.0),
    };
    [
        index as f32,
        f32::from(instruction.opcode().ordinal()),
        immediate_kind,
        immediate_value,
    ]
}

fn decode_mode_code(mode: TassadarExecutorDecodeMode) -> f32 {
    match mode {
        TassadarExecutorDecodeMode::ReferenceLinear => 1.0,
        TassadarExecutorDecodeMode::HullCache => 2.0,
        TassadarExecutorDecodeMode::SparseTopK => 3.0,
    }
}

fn stable_request_digest(
    model_descriptor_digest: &str,
    program_artifact_digest: &str,
    requested_decode_mode: TassadarExecutorDecodeMode,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tassadar_compiled_program_request|");
    hasher.update(model_descriptor_digest.as_bytes());
    hasher.update(b"\n");
    hasher.update(program_artifact_digest.as_bytes());
    hasher.update(b"\n");
    hasher.update(requested_decode_mode.as_str().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_serialized_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[derive(Serialize)]
struct CompiledWeightArtifactEncoding<'a> {
    artifact_id: &'a str,
    program_artifact_digest: &'a str,
    compiled_program_header: &'a [f32],
    compiled_instruction_stream: &'a [f32],
    compiled_initial_memory: &'a [f32],
    compiled_decode_contract: &'a [f32],
}

#[cfg(test)]
mod tests {
    use psionic_runtime::{
        TassadarExecutorDecodeMode, TassadarFixtureRunner, TassadarProgramArtifact,
        TassadarTraceAbi, run_tassadar_exact_parity, tassadar_article_class_corpus,
        tassadar_sudoku_v0_search_program, tassadar_validation_corpus,
    };

    use super::{
        TassadarCompiledProgramError, TassadarCompiledProgramSuiteArtifact,
        TassadarExecutorContractError, TassadarExecutorFixture, TassadarWasmProfile,
        TassadarWorkloadCapabilityMatrixError, TassadarWorkloadCapabilityRefusalReason,
        TassadarWorkloadClass, TassadarWorkloadSupportPosture,
        build_tassadar_article_transformer_trained_executor_descriptor,
    };
    use crate::{
        ModelIngressSurface, ModelRuntimeSurface, ModelServingSurface, WeightFormat, WeightSource,
    };

    fn sample_validation_artifact() -> (TassadarExecutorFixture, TassadarProgramArtifact) {
        let fixture = TassadarExecutorFixture::new();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar.locals_add.artifact.v1",
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            case.program,
        )
        .expect("artifact should assemble");
        (fixture, artifact)
    }

    #[test]
    fn tassadar_fixture_descriptor_reports_programmatic_fixture_boundary() {
        let fixture = TassadarExecutorFixture::new();
        let descriptor = fixture.descriptor();
        assert_eq!(descriptor.model.model_id, TassadarExecutorFixture::MODEL_ID);
        assert_eq!(descriptor.weights.format, WeightFormat::ProgrammaticFixture);
        assert_eq!(
            descriptor.interop_boundary().ingress_surface,
            ModelIngressSurface::Fixture
        );
        assert_eq!(
            descriptor.interop_boundary().serving_surface,
            ModelServingSurface::PsionicNative
        );
        assert_eq!(
            descriptor.interop_boundary().runtime_surface,
            ModelRuntimeSurface::PsionicNative
        );
    }

    #[test]
    fn tassadar_fixture_bundle_matches_runtime_rule_table_shape() {
        let fixture = TassadarExecutorFixture::new();
        let bundle = fixture.weight_bundle();
        assert_eq!(
            bundle
                .metadata()
                .tensors
                .iter()
                .map(|tensor| tensor.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "opcode_semantics",
                "opcode_stack_effects",
                "profile_limits",
                "trace_abi_flags"
            ]
        );
        assert_eq!(
            bundle.opcode_stack_effects().len(),
            fixture.runtime_weights().opcode_rules.len() * 2
        );
        assert_eq!(
            bundle.opcode_semantics().len(),
            fixture.runtime_weights().opcode_rules.len() * 5
        );
    }

    #[test]
    fn tassadar_fixture_model_aligns_with_runtime_validation_corpus() {
        let fixture = TassadarExecutorFixture::new();
        let runner = TassadarFixtureRunner::new();
        assert_eq!(
            fixture.descriptor().profile.profile_id,
            fixture.runtime_weights().profile_id
        );
        assert_eq!(
            fixture.descriptor().trace_abi.abi_id,
            fixture.runtime_weights().trace_abi_id
        );
        for case in tassadar_validation_corpus() {
            let execution = runner.execute(&case.program).expect("case should run");
            assert_eq!(
                execution.outputs, case.expected_outputs,
                "case={}",
                case.case_id
            );
            run_tassadar_exact_parity(&case.program).expect("exact parity should hold");
        }
    }

    #[test]
    fn tassadar_article_class_fixture_aligns_with_runtime_article_corpus() {
        let fixture = TassadarExecutorFixture::article_i32_compute_v1();
        let runner = TassadarFixtureRunner::for_profile(fixture.descriptor().profile.clone())
            .expect("article-class fixture runner");
        assert_eq!(
            fixture.descriptor().profile.profile_id,
            fixture.runtime_weights().profile_id
        );
        assert_eq!(
            fixture.descriptor().trace_abi.profile_id,
            fixture.runtime_weights().profile_id
        );
        for case in tassadar_article_class_corpus() {
            assert_eq!(
                case.program.profile_id,
                fixture.descriptor().profile.profile_id
            );
            let execution = runner.execute(&case.program).expect("case should run");
            assert_eq!(
                execution.outputs, case.expected_outputs,
                "case={}",
                case.case_id
            );
            run_tassadar_exact_parity(&case.program).expect("exact parity should hold");
        }
    }

    #[test]
    fn article_fixture_workload_capability_matrix_is_machine_legible() {
        let fixture = TassadarExecutorFixture::article_i32_compute_v1();
        let matrix = fixture.workload_capability_matrix();

        matrix
            .validate_publication()
            .expect("article fixture matrix should be publishable");
        assert_eq!(
            matrix
                .row(TassadarWorkloadClass::MicroWasmKernel)
                .expect("micro row")
                .support_posture,
            TassadarWorkloadSupportPosture::Exact
        );
        assert_eq!(
            matrix
                .row(TassadarWorkloadClass::LongLoopKernel)
                .expect("long-loop row")
                .support_posture,
            TassadarWorkloadSupportPosture::ExactFallbackOnly
        );
        assert_eq!(
            matrix
                .row(TassadarWorkloadClass::ArithmeticMicroprogram)
                .expect("arithmetic row")
                .refusal_reasons,
            vec![TassadarWorkloadCapabilityRefusalReason::BenchmarkGateMissing]
        );
        let encoded = serde_json::to_value(&matrix).expect("matrix should serialize");
        let workload_classes = encoded["rows"]
            .as_array()
            .expect("rows should encode as an array")
            .iter()
            .map(|row| row["workload_class"].clone())
            .collect::<Vec<_>>();
        assert!(workload_classes.contains(&serde_json::json!("arithmetic_microprogram")));
        assert!(workload_classes.contains(&serde_json::json!("micro_wasm_kernel")));
    }

    #[test]
    fn workload_capability_matrix_requires_benchmark_gate_for_published_rows() {
        let fixture = TassadarExecutorFixture::article_i32_compute_v1();
        let mut matrix = fixture.workload_capability_matrix();
        let row = matrix
            .rows
            .iter_mut()
            .find(|row| row.workload_class == TassadarWorkloadClass::MicroWasmKernel)
            .expect("micro row");
        row.benchmark_gate = None;
        let err = matrix
            .validate_publication()
            .expect_err("published rows should require a benchmark gate");
        assert_eq!(
            err,
            TassadarWorkloadCapabilityMatrixError::PublishableWorkloadMissingBenchmarkGate {
                workload_class: TassadarWorkloadClass::MicroWasmKernel,
            }
        );
    }

    #[test]
    fn core_i32_v2_workload_capability_matrix_stays_unpublishable_without_benchmark_gate() {
        let fixture = TassadarExecutorFixture::core_i32_v2();
        let err = fixture
            .workload_capability_matrix()
            .validate_publication()
            .expect_err("core_i32_v2 should remain unpublished");
        assert_eq!(
            err,
            TassadarWorkloadCapabilityMatrixError::NoPublishableWorkloads
        );
    }

    #[test]
    fn tassadar_sudoku_v0_search_fixture_accepts_real_search_program_artifact() {
        let fixture = TassadarExecutorFixture::sudoku_v0_search_v1();
        let program = tassadar_sudoku_v0_search_program(
            "tassadar.sudoku_v0.models_fixture",
            [
                0, 1, 0, 0, //
                0, 4, 1, 0, //
                1, 0, 0, 4, //
                0, 3, 0, 0,
            ],
        );
        let artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar.sudoku_v0.models_fixture.artifact",
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            program.clone(),
        )
        .expect("Sudoku-v0 search artifact should assemble");
        fixture
            .descriptor()
            .validate_program_artifact(&artifact, TassadarExecutorDecodeMode::ReferenceLinear)
            .expect("Sudoku-v0 search artifact should validate against the fixture");
        let execution = TassadarFixtureRunner::for_profile(fixture.descriptor().profile.clone())
            .expect("Sudoku-v0 fixture runner")
            .execute(&program)
            .expect("Sudoku-v0 program should execute through the fixture runner");
        assert_eq!(
            execution.outputs,
            vec![
                2, 1, 4, 3, //
                3, 4, 1, 2, //
                1, 2, 3, 4, //
                4, 3, 2, 1,
            ]
        );
    }

    #[test]
    fn tassadar_descriptor_accepts_matching_program_artifact() {
        let fixture = TassadarExecutorFixture::new();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar.locals_add.artifact.v1",
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            case.program,
        )
        .expect("artifact should assemble");
        fixture
            .descriptor()
            .validate_program_artifact(&artifact, TassadarExecutorDecodeMode::ReferenceLinear)
            .expect("artifact should be compatible");
    }

    #[test]
    fn tassadar_descriptor_rejects_trace_abi_mismatch() {
        let fixture = TassadarExecutorFixture::new();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let mut artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar.locals_add.artifact.v1",
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            case.program,
        )
        .expect("artifact should assemble");
        artifact.trace_abi_id = String::from("tassadar.trace.other.v1");
        let error = fixture
            .descriptor()
            .validate_program_artifact(&artifact, TassadarExecutorDecodeMode::ReferenceLinear)
            .expect_err("trace ABI mismatch should refuse");
        assert_eq!(
            error,
            TassadarExecutorContractError::TraceAbiMismatch {
                expected: fixture.descriptor().trace_abi.abi_id.clone(),
                actual: String::from("tassadar.trace.other.v1"),
            }
        );
    }

    #[test]
    fn tassadar_descriptor_rejects_opcode_vocabulary_mismatch() {
        let fixture = TassadarExecutorFixture::new();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let mut artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar.locals_add.artifact.v1",
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            case.program,
        )
        .expect("artifact should assemble");
        artifact.opcode_vocabulary_digest = String::from("sha256:not-the-real-vocab");
        let error = fixture
            .descriptor()
            .validate_program_artifact(&artifact, TassadarExecutorDecodeMode::ReferenceLinear)
            .expect_err("opcode mismatch should refuse");
        assert_eq!(
            error,
            TassadarExecutorContractError::OpcodeVocabularyDigestMismatch {
                expected: fixture
                    .descriptor()
                    .compatibility
                    .opcode_vocabulary_digest
                    .clone(),
                actual: String::from("sha256:not-the-real-vocab"),
            }
        );
    }

    #[test]
    fn tassadar_descriptor_accepts_hull_cache_decode_mode() {
        let fixture = TassadarExecutorFixture::new();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar.locals_add.artifact.v1",
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            case.program,
        )
        .expect("artifact should assemble");
        fixture
            .descriptor()
            .validate_program_artifact(&artifact, TassadarExecutorDecodeMode::HullCache)
            .expect("hull-cache decode mode should validate");
        assert!(
            fixture
                .descriptor()
                .compatibility
                .attention_geometry
                .hull_cache_eligible
        );
    }

    #[test]
    fn trained_article_transformer_executor_descriptor_projects_fast_route_support() {
        let descriptor = build_tassadar_article_transformer_trained_executor_descriptor()
            .expect("trained article Transformer descriptor");

        assert_eq!(
            descriptor.model.model_id,
            crate::TassadarArticleTransformer::TRAINED_TRACE_BOUND_MODEL_ID
        );
        assert_eq!(
            descriptor.compatibility.supported_decode_modes,
            vec![
                TassadarExecutorDecodeMode::ReferenceLinear,
                TassadarExecutorDecodeMode::HullCache,
            ]
        );
        assert_eq!(
            descriptor.compatibility.attention_mode,
            super::TassadarExecutorAttentionMode::StandardSoftmax
        );
        assert!(
            descriptor
                .compatibility
                .attention_geometry
                .hull_cache_eligible
        );
        assert_eq!(
            descriptor.profile.profile_id,
            TassadarWasmProfile::article_i32_compute_v1().profile_id
        );
    }

    #[test]
    fn tassadar_descriptor_accepts_sparse_top_k_decode_mode() {
        let fixture = TassadarExecutorFixture::new();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar.locals_add.artifact.v1",
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            case.program,
        )
        .expect("artifact should assemble");
        fixture
            .descriptor()
            .validate_program_artifact(&artifact, TassadarExecutorDecodeMode::SparseTopK)
            .expect("sparse top-k decode mode should validate");
    }

    #[test]
    fn tassadar_fixture_reports_runtime_capability_and_selection_truth() {
        let fixture = TassadarExecutorFixture::new();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let capability = fixture.runtime_capability_report();
        assert!(capability.supports_executor_trace);
        assert!(capability.supports_hull_decode);
        assert!(capability.supports_sparse_top_k_decode);
        assert_eq!(capability.validated_trace_abi_versions, vec![1]);
        let diagnostic = fixture
            .runtime_selection_diagnostic(&case.program, TassadarExecutorDecodeMode::HullCache);
        assert!(!diagnostic.is_refused());
        assert_eq!(
            diagnostic.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::HullCache)
        );
        let sparse_diagnostic = fixture
            .runtime_selection_diagnostic(&case.program, TassadarExecutorDecodeMode::SparseTopK);
        assert_eq!(
            sparse_diagnostic.effective_decode_mode,
            Some(TassadarExecutorDecodeMode::SparseTopK)
        );
    }

    #[test]
    fn tassadar_descriptor_rejects_internally_inconsistent_artifact() {
        let fixture = TassadarExecutorFixture::new();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let mut artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar.locals_add.artifact.v1",
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            case.program,
        )
        .expect("artifact should assemble");
        artifact.validated_program_digest = String::from("sha256:stale");
        let error = fixture
            .descriptor()
            .validate_program_artifact(&artifact, TassadarExecutorDecodeMode::ReferenceLinear)
            .expect_err("inconsistent artifact should refuse");
        assert!(matches!(
            error,
            TassadarExecutorContractError::ProgramArtifactInconsistent { .. }
        ));
    }

    #[test]
    fn tassadar_descriptor_rejects_trace_abi_version_mismatch() {
        let fixture = TassadarExecutorFixture::new();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let mut artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar.locals_add.artifact.v1",
            &fixture.descriptor().profile,
            &fixture.descriptor().trace_abi,
            case.program,
        )
        .expect("artifact should assemble");
        artifact.trace_abi_version = TassadarTraceAbi::core_i32_v1().schema_version + 1;
        let error = fixture
            .descriptor()
            .validate_program_artifact(&artifact, TassadarExecutorDecodeMode::ReferenceLinear)
            .expect_err("trace ABI version mismatch should refuse");
        assert_eq!(
            error,
            TassadarExecutorContractError::TraceAbiVersionMismatch {
                expected: fixture.descriptor().trace_abi.schema_version,
                actual: TassadarTraceAbi::core_i32_v1().schema_version + 1,
            }
        );
    }

    #[test]
    fn compiled_program_executor_reports_external_artifact_boundary() {
        let (fixture, artifact) = sample_validation_artifact();
        let compiled = fixture
            .compile_program("tassadar.validation.compiled", &artifact)
            .expect("compiled deployment should build");
        assert_eq!(
            compiled.descriptor().compatibility.attention_mode,
            super::TassadarExecutorAttentionMode::ProgramSpecializedCompiled
        );
        assert_eq!(
            compiled.descriptor().interop_boundary().ingress_surface,
            ModelIngressSurface::DirectArtifactImport
        );
        assert_eq!(
            compiled.weight_bundle().metadata().source,
            WeightSource::ExternalArtifact
        );
        assert_eq!(
            compiled.weight_bundle().metadata().format,
            WeightFormat::ProgrammaticFixture
        );
        assert_eq!(
            compiled.runtime_contract().program_artifact_digest,
            artifact.artifact_digest
        );
    }

    #[test]
    fn compiled_program_executor_executes_matching_program_with_proof_lineage() {
        let (fixture, artifact) = sample_validation_artifact();
        let compiled = fixture
            .compile_program("tassadar.validation.compiled", &artifact)
            .expect("compiled deployment should build");
        let expected = TassadarFixtureRunner::for_program(&artifact.validated_program)
            .expect("fixture runner")
            .execute(&artifact.validated_program)
            .expect("reference execution");
        let execution = compiled
            .execute(&artifact, TassadarExecutorDecodeMode::HullCache)
            .expect("matching program should execute");
        assert_eq!(
            execution.execution_report.execution.outputs,
            expected.outputs
        );
        assert!(
            !execution
                .evidence_bundle
                .trace_proof
                .proof_digest
                .is_empty()
        );
        assert_eq!(
            execution
                .evidence_bundle
                .trace_proof
                .program_artifact_digest,
            artifact.artifact_digest
        );
    }

    #[test]
    fn compiled_program_executor_rejects_mismatched_program_artifact() {
        let (fixture, artifact) = sample_validation_artifact();
        let compiled = fixture
            .compile_program("tassadar.validation.compiled", &artifact)
            .expect("compiled deployment should build");
        let (_, wrong_artifact) = {
            let fixture = TassadarExecutorFixture::new();
            let case = tassadar_validation_corpus()
                .into_iter()
                .nth(1)
                .expect("second validation case");
            let artifact = TassadarProgramArtifact::fixture_reference(
                "tassadar.memory_roundtrip.artifact.v1",
                &fixture.descriptor().profile,
                &fixture.descriptor().trace_abi,
                case.program,
            )
            .expect("artifact should assemble");
            (fixture, artifact)
        };
        let error = compiled
            .execute(&wrong_artifact, TassadarExecutorDecodeMode::ReferenceLinear)
            .expect_err("wrong program should refuse");
        assert!(matches!(
            error,
            TassadarCompiledProgramError::ProgramArtifactDigestMismatch { .. }
        ));
    }

    #[test]
    fn compiled_program_suite_artifact_aggregates_program_deployments() {
        let fixture = TassadarExecutorFixture::new();
        let artifacts = tassadar_validation_corpus()
            .into_iter()
            .map(|case| {
                TassadarProgramArtifact::fixture_reference(
                    format!("artifact.{}", case.case_id),
                    &fixture.descriptor().profile,
                    &fixture.descriptor().trace_abi,
                    case.program,
                )
                .expect("artifact should assemble")
            })
            .collect::<Vec<_>>();
        let suite = TassadarCompiledProgramSuiteArtifact::compile(
            "tassadar.validation.compiled_suite",
            "benchmark://tassadar/reference_fixture@2026.03.16",
            &fixture,
            artifacts.as_slice(),
        )
        .expect("suite should compile");
        assert_eq!(suite.deployments.len(), artifacts.len());
        assert!(suite.total_compiled_weight_artifact_bytes > 0);
        assert!(!suite.artifact_digest.is_empty());
    }

    #[test]
    fn tassadar_fixture_supports_named_hard_sudoku_profile() {
        let fixture = TassadarExecutorFixture::for_profile_id(
            TassadarWasmProfile::sudoku_9x9_hard_search_v1()
                .profile_id
                .as_str(),
        )
        .expect("named hard Sudoku profile should resolve to a fixture");

        assert_eq!(
            fixture.descriptor().model.model_id,
            TassadarExecutorFixture::SUDOKU_9X9_HARD_SEARCH_MODEL_ID
        );
        assert_eq!(
            fixture.descriptor().profile.profile_id,
            TassadarWasmProfile::sudoku_9x9_hard_search_v1().profile_id
        );
        assert_eq!(
            fixture.descriptor().trace_abi.profile_id,
            TassadarTraceAbi::sudoku_9x9_hard_search_v1().profile_id
        );
    }
}
