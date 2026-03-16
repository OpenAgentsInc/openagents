use psionic_core::{DType, QuantizationMode, Shape};
use psionic_runtime::{
    TassadarExecutorDecodeMode, TassadarFixtureWeights as RuntimeTassadarFixtureWeights,
    TassadarProgramArtifact, TassadarTraceAbi, TassadarWasmProfile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ModelArtifactGovernance, ModelDescriptor, ModelIngressSurface, ModelInteropBoundary,
    ModelRuntimeSurface, ModelServingSurface, WeightBundleMetadata, WeightFormat, WeightSource,
    WeightTensorMetadata,
};

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
    /// Stable model family for the Phase 1 fixture.
    pub const MODEL_FAMILY: &str = "tassadar_executor";

    /// Creates the default Phase 1 Tassadar executor fixture.
    #[must_use]
    pub fn new() -> Self {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let runtime_weights = RuntimeTassadarFixtureWeights::core_i32_v1();
        let weight_bundle = build_weight_bundle(&runtime_weights, &profile, &trace_abi);
        let compatibility = TassadarExecutorCompatibility::reference_fixture(&profile, &trace_abi);
        let descriptor = TassadarExecutorModelDescriptor::new(
            ModelDescriptor::new(Self::MODEL_ID, Self::MODEL_FAMILY, "v0"),
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
    let metadata = build_metadata(&entries);

    TassadarExecutorWeightBundle {
        metadata,
        opcode_stack_effects,
        opcode_semantics,
        profile_limits,
        trace_abi_flags,
    }
}

fn build_metadata(entries: &[(WeightTensorMetadata, &[f32])]) -> WeightBundleMetadata {
    let mut ordered = entries.to_vec();
    ordered.sort_by(|(left, _), (right, _)| left.name.cmp(&right.name));

    let mut hasher = Sha256::new();
    for (metadata, values) in &ordered {
        digest_tensor_values(&mut hasher, metadata, values);
    }

    WeightBundleMetadata {
        format: WeightFormat::ProgrammaticFixture,
        source: WeightSource::Fixture,
        quantization: QuantizationMode::None,
        quantization_modes: Vec::new(),
        digest: hex::encode(hasher.finalize()),
        tensors: ordered
            .iter()
            .map(|(metadata, _)| metadata.clone())
            .collect(),
        artifacts: Vec::new(),
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
            if weights.source == WeightSource::Fixture
                || weights.format == WeightFormat::ProgrammaticFixture
            {
                ModelIngressSurface::Fixture
            } else if weights.is_artifact_backed() {
                ModelIngressSurface::DirectArtifactImport
            } else {
                ModelIngressSurface::PsionicNativeBundle
            }
        })
}

#[cfg(test)]
mod tests {
    use psionic_runtime::{
        run_tassadar_exact_parity, tassadar_validation_corpus, TassadarExecutorDecodeMode,
        TassadarFixtureRunner, TassadarProgramArtifact, TassadarTraceAbi,
    };

    use super::{TassadarExecutorContractError, TassadarExecutorFixture};
    use crate::{ModelIngressSurface, ModelRuntimeSurface, ModelServingSurface, WeightFormat};

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
}
