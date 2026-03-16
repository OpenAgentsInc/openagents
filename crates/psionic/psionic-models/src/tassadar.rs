use psionic_core::{DType, QuantizationMode, Shape};
use psionic_runtime::{
    TassadarFixtureWeights as RuntimeTassadarFixtureWeights, TassadarTraceAbi, TassadarWasmProfile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ModelArtifactGovernance, ModelDescriptor, ModelIngressSurface, ModelInteropBoundary,
    ModelRuntimeSurface, ModelServingSurface, WeightBundleMetadata, WeightFormat, WeightSource,
    WeightTensorMetadata,
};

/// Executor-class model descriptor for the Phase 1 Tassadar fixture lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorModelDescriptor {
    /// Shared model metadata.
    pub model: ModelDescriptor,
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
        profile: TassadarWasmProfile,
        trace_abi: TassadarTraceAbi,
        weights: WeightBundleMetadata,
    ) -> Self {
        Self {
            model,
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
        let descriptor = TassadarExecutorModelDescriptor::new(
            ModelDescriptor::new(Self::MODEL_ID, Self::MODEL_FAMILY, "v0"),
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
        run_tassadar_exact_parity, tassadar_validation_corpus, TassadarFixtureRunner,
    };

    use super::TassadarExecutorFixture;
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
}
