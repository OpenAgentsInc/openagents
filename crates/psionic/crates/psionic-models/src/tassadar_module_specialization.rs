use psionic_core::{DType, QuantizationMode, Shape};
use psionic_ir::TassadarNormalizedWasmModule;
use psionic_runtime::{
    TassadarExecutorDecodeMode, TassadarModuleSpecializationBundle,
    TassadarModuleSpecializationError, TassadarModuleSpecializedExportArtifact,
    compile_tassadar_module_specialization_bundle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarCompiledProgramError, TassadarCompiledProgramExecutor, TassadarExecutorFixture,
    WeightArtifactMetadata, WeightBundleMetadata, WeightFormat, WeightSource, WeightTensorMetadata,
};

const TASSADAR_COMPILED_MODULE_SPECIALIZATION_CLAIM_BOUNDARY: &str = "module-aware compiled specialization keeps shared normalized-module structure, call-graph reachability, and per-export exactness lineage explicit as research-only systems work; it does not claim a served module-specialized runtime lane or arbitrary Wasm closure";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarCompiledModuleSpecializationWeightBundle {
    metadata: WeightBundleMetadata,
    module_header: Vec<f32>,
    function_table: Vec<f32>,
    call_graph_matrix: Vec<f32>,
    export_entry_table: Vec<f32>,
    initial_memory_image: Vec<f32>,
}

impl TassadarCompiledModuleSpecializationWeightBundle {
    #[must_use]
    pub fn metadata(&self) -> &WeightBundleMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn module_header(&self) -> &[f32] {
        &self.module_header
    }

    #[must_use]
    pub fn function_table(&self) -> &[f32] {
        &self.function_table
    }

    #[must_use]
    pub fn call_graph_matrix(&self) -> &[f32] {
        &self.call_graph_matrix
    }

    #[must_use]
    pub fn export_entry_table(&self) -> &[f32] {
        &self.export_entry_table
    }

    #[must_use]
    pub fn initial_memory_image(&self) -> &[f32] {
        &self.initial_memory_image
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledModuleSpecializationExportArtifact {
    pub export_name: String,
    pub function_index: u32,
    pub reachable_function_indices: Vec<u32>,
    pub program_artifact_ref: String,
    pub program_artifact_digest: String,
    pub program_digest: String,
    pub compiled_weight_artifact_digest: String,
    pub compiled_weight_bundle_digest: String,
    pub runtime_contract_digest: String,
    pub compile_runtime_manifest_digest: String,
    pub compile_trace_proof_digest: String,
    pub compile_execution_proof_bundle_digest: String,
    pub runtime_execution_proof_bundle_digest: String,
    pub compiled_runtime_manifest_digest: String,
    pub compiled_runtime_trace_proof_digest: String,
    pub expected_trace_digest: String,
    pub compiled_trace_digest: String,
    pub expected_behavior_digest: String,
    pub compiled_behavior_digest: String,
    pub exact_trace_match: bool,
    pub final_output_match: bool,
    pub final_memory_match: bool,
    pub halt_match: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledModuleSpecializationArtifact {
    pub artifact_id: String,
    pub artifact_digest: String,
    pub claim_class: String,
    pub claim_boundary: String,
    pub base_model_id: String,
    pub base_model_descriptor_digest: String,
    pub normalized_module_digest: String,
    pub module_specialization_bundle_digest: String,
    pub module_specialization_plan_digest: String,
    pub compiled_module_weight_bundle_digest: String,
    pub compiled_module_weight_artifact_sha256: String,
    pub compiled_module_weight_artifact_bytes: u64,
    pub exports: Vec<TassadarCompiledModuleSpecializationExportArtifact>,
}

impl TassadarCompiledModuleSpecializationArtifact {
    fn new(
        artifact_id: impl Into<String>,
        base_fixture: &TassadarExecutorFixture,
        bundle: &TassadarModuleSpecializationBundle,
        weight_bundle: &TassadarCompiledModuleSpecializationWeightBundle,
        exports: Vec<TassadarCompiledModuleSpecializationExportArtifact>,
        artifact_sha256: impl Into<String>,
        artifact_bytes: u64,
    ) -> Self {
        let mut artifact = Self {
            artifact_id: artifact_id.into(),
            artifact_digest: String::new(),
            claim_class: String::from("compiled bounded exactness / research-only systems work"),
            claim_boundary: String::from(TASSADAR_COMPILED_MODULE_SPECIALIZATION_CLAIM_BOUNDARY),
            base_model_id: base_fixture.descriptor().model.model_id.clone(),
            base_model_descriptor_digest: base_fixture.descriptor().stable_digest(),
            normalized_module_digest: bundle.normalized_module.module_digest.clone(),
            module_specialization_bundle_digest: bundle.bundle_digest.clone(),
            module_specialization_plan_digest: bundle.specialization_plan.plan_digest.clone(),
            compiled_module_weight_bundle_digest: weight_bundle.metadata().digest.clone(),
            compiled_module_weight_artifact_sha256: artifact_sha256.into(),
            compiled_module_weight_artifact_bytes: artifact_bytes,
            exports,
        };
        artifact.artifact_digest = stable_serialized_digest(
            b"tassadar_compiled_module_specialization_artifact|",
            &artifact,
        );
        artifact
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TassadarCompiledModuleSpecializationExportDeployment {
    lowered_export: TassadarModuleSpecializedExportArtifact,
    compiled_executor: TassadarCompiledProgramExecutor,
}

impl TassadarCompiledModuleSpecializationExportDeployment {
    #[must_use]
    pub fn lowered_export(&self) -> &TassadarModuleSpecializedExportArtifact {
        &self.lowered_export
    }

    #[must_use]
    pub fn compiled_executor(&self) -> &TassadarCompiledProgramExecutor {
        &self.compiled_executor
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TassadarCompiledModuleSpecialization {
    bundle: TassadarModuleSpecializationBundle,
    weight_bundle: TassadarCompiledModuleSpecializationWeightBundle,
    artifact: TassadarCompiledModuleSpecializationArtifact,
    export_deployments: Vec<TassadarCompiledModuleSpecializationExportDeployment>,
}

impl TassadarCompiledModuleSpecialization {
    pub fn compile(
        artifact_id: impl Into<String>,
        source_name: impl Into<String>,
        base_fixture: &TassadarExecutorFixture,
        normalized_module: TassadarNormalizedWasmModule,
    ) -> Result<Self, TassadarCompiledModuleSpecializationError> {
        let artifact_id = artifact_id.into();
        let bundle = compile_tassadar_module_specialization_bundle(
            format!("{artifact_id}.bundle"),
            source_name,
            normalized_module,
            &base_fixture.descriptor().profile,
            &base_fixture.descriptor().trace_abi,
        )?;

        let mut export_deployments = Vec::with_capacity(bundle.lowered_exports.len());
        let mut export_artifacts = Vec::with_capacity(bundle.lowered_exports.len());
        for lowered_export in &bundle.lowered_exports {
            let compiled_executor = base_fixture.compile_program(
                format!(
                    "{}.{}",
                    artifact_id,
                    sanitize_label(lowered_export.export_name.as_str())
                ),
                &lowered_export.program_artifact,
            )?;
            let compiled_execution = compiled_executor.execute(
                &lowered_export.program_artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
            )?;
            let compiled = &compiled_execution.execution_report.execution;
            export_artifacts.push(TassadarCompiledModuleSpecializationExportArtifact {
                export_name: lowered_export.export_name.clone(),
                function_index: lowered_export.function_index,
                reachable_function_indices: lowered_export.reachable_function_indices.clone(),
                program_artifact_ref: lowered_export.program_artifact.artifact_id.clone(),
                program_artifact_digest: lowered_export.program_artifact.artifact_digest.clone(),
                program_digest: lowered_export
                    .program_artifact
                    .validated_program_digest
                    .clone(),
                compiled_weight_artifact_digest: compiled_executor
                    .compiled_weight_artifact()
                    .artifact_digest
                    .clone(),
                compiled_weight_bundle_digest: compiled_executor
                    .compiled_weight_artifact()
                    .compiled_weight_bundle_digest
                    .clone(),
                runtime_contract_digest: compiled_executor
                    .runtime_contract()
                    .contract_digest
                    .clone(),
                compile_runtime_manifest_digest: compiled_executor
                    .compiled_weight_artifact()
                    .compile_runtime_manifest_digest
                    .clone(),
                compile_trace_proof_digest: compiled_executor
                    .compiled_weight_artifact()
                    .compile_trace_proof_digest
                    .clone(),
                compile_execution_proof_bundle_digest: compiled_executor
                    .compiled_weight_artifact()
                    .compile_execution_proof_bundle_digest
                    .clone(),
                runtime_execution_proof_bundle_digest: compiled_execution
                    .evidence_bundle
                    .proof_bundle
                    .stable_digest(),
                compiled_runtime_manifest_digest: compiled_execution
                    .evidence_bundle
                    .runtime_manifest
                    .manifest_digest
                    .clone(),
                compiled_runtime_trace_proof_digest: compiled_execution
                    .evidence_bundle
                    .trace_proof
                    .proof_digest
                    .clone(),
                expected_trace_digest: lowered_export.execution_manifest.trace_digest.clone(),
                compiled_trace_digest: compiled.trace_digest(),
                expected_behavior_digest: lowered_export.execution_manifest.behavior_digest.clone(),
                compiled_behavior_digest: compiled.behavior_digest(),
                exact_trace_match: lowered_export.execution_manifest.trace_digest
                    == compiled.trace_digest(),
                final_output_match: lowered_export.execution_manifest.expected_outputs
                    == compiled.outputs,
                final_memory_match: lowered_export.execution_manifest.expected_final_memory
                    == compiled.final_memory,
                halt_match: lowered_export.execution_manifest.halt_reason == compiled.halt_reason,
            });
            export_deployments.push(TassadarCompiledModuleSpecializationExportDeployment {
                lowered_export: lowered_export.clone(),
                compiled_executor,
            });
        }
        export_artifacts.sort_by(|left, right| left.export_name.cmp(&right.export_name));
        export_deployments.sort_by(|left, right| {
            left.lowered_export
                .export_name
                .cmp(&right.lowered_export.export_name)
        });

        let weight_bundle =
            build_compiled_module_specialization_weight_bundle(&artifact_id, &bundle)?;
        let primary_artifact = weight_bundle
            .metadata()
            .artifacts
            .first()
            .cloned()
            .unwrap_or_else(|| {
                WeightArtifactMetadata::new(
                    "compiled_module_specialization.json",
                    0,
                    weight_bundle.metadata().digest.clone(),
                )
            });
        let artifact = TassadarCompiledModuleSpecializationArtifact::new(
            format!("{artifact_id}.artifact"),
            base_fixture,
            &bundle,
            &weight_bundle,
            export_artifacts,
            primary_artifact.sha256,
            primary_artifact.byte_length,
        );
        Ok(Self {
            bundle,
            weight_bundle,
            artifact,
            export_deployments,
        })
    }

    #[must_use]
    pub fn bundle(&self) -> &TassadarModuleSpecializationBundle {
        &self.bundle
    }

    #[must_use]
    pub fn weight_bundle(&self) -> &TassadarCompiledModuleSpecializationWeightBundle {
        &self.weight_bundle
    }

    #[must_use]
    pub fn artifact(&self) -> &TassadarCompiledModuleSpecializationArtifact {
        &self.artifact
    }

    #[must_use]
    pub fn export_deployments(&self) -> &[TassadarCompiledModuleSpecializationExportDeployment] {
        &self.export_deployments
    }

    #[must_use]
    pub fn total_unspecialized_compiled_weight_artifact_bytes(&self) -> u64 {
        self.export_deployments
            .iter()
            .map(|deployment| {
                deployment
                    .compiled_executor
                    .compiled_weight_artifact()
                    .compiled_weight_artifact_bytes
            })
            .sum()
    }
}

impl TassadarExecutorFixture {
    pub fn compile_module_specialization(
        &self,
        artifact_id: impl Into<String>,
        source_name: impl Into<String>,
        normalized_module: TassadarNormalizedWasmModule,
    ) -> Result<TassadarCompiledModuleSpecialization, TassadarCompiledModuleSpecializationError>
    {
        TassadarCompiledModuleSpecialization::compile(
            artifact_id,
            source_name,
            self,
            normalized_module,
        )
    }
}

#[derive(Debug, Error)]
pub enum TassadarCompiledModuleSpecializationError {
    #[error(transparent)]
    Specialization(#[from] TassadarModuleSpecializationError),
    #[error(transparent)]
    Compiled(#[from] TassadarCompiledProgramError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Serialize)]
struct CompiledModuleSpecializationArtifactEncoding<'a> {
    artifact_id: &'a str,
    normalized_module_digest: &'a str,
    module_specialization_plan_digest: &'a str,
    module_header: &'a [f32],
    function_table: &'a [f32],
    call_graph_matrix: &'a [f32],
    export_entry_table: &'a [f32],
    initial_memory_image: &'a [f32],
}

fn build_compiled_module_specialization_weight_bundle(
    artifact_id: &str,
    bundle: &TassadarModuleSpecializationBundle,
) -> Result<
    TassadarCompiledModuleSpecializationWeightBundle,
    TassadarCompiledModuleSpecializationError,
> {
    let function_count = bundle.specialization_plan.function_count as usize;
    let module_header = vec![
        bundle.schema_version as f32,
        bundle.specialization_plan.schema_version as f32,
        bundle.specialization_plan.function_count as f32,
        bundle.specialization_plan.defined_function_count as f32,
        bundle.specialization_plan.imported_function_count as f32,
        bundle.specialization_plan.export_count as f32,
        bundle.specialization_plan.memory_count as f32,
        bundle.specialization_plan.data_segment_count as f32,
        bundle.specialization_plan.total_data_bytes as f32,
        bundle.specialization_plan.call_edges.len() as f32,
    ];
    let function_table = bundle
        .specialization_plan
        .function_summaries
        .iter()
        .flat_map(|summary| {
            [
                summary.function_index as f32,
                summary.type_index as f32,
                if summary.imported { 1.0 } else { 0.0 },
                summary.exported_names.len() as f32,
                summary.instruction_count as f32,
                summary.local_count as f32,
                summary.result_count as f32,
                if summary.has_memory_access { 1.0 } else { 0.0 },
                summary.direct_defined_callee_indices.len() as f32,
                summary.direct_import_refs.len() as f32,
            ]
        })
        .collect::<Vec<_>>();
    let mut call_graph_matrix = vec![0.0_f32; function_count * function_count];
    for edge in &bundle.specialization_plan.call_edges {
        if !matches!(
            edge.edge_kind,
            psionic_runtime::TassadarModuleSpecializationCallEdgeKind::DirectDefined
        ) {
            continue;
        }
        let index = edge.caller_function_index as usize * function_count
            + edge.callee_function_index as usize;
        call_graph_matrix[index] = 1.0;
    }
    let export_entry_table = bundle
        .specialization_plan
        .export_summaries
        .iter()
        .flat_map(|summary| {
            [
                summary.function_index as f32,
                summary.reachable_function_indices.len() as f32,
                summary.reachable_import_refs.len() as f32,
                summary.direct_call_edge_count as f32,
                summary.max_inline_depth as f32,
                if summary.contains_memory_access {
                    1.0
                } else {
                    0.0
                },
                if summary.call_graph_is_acyclic {
                    1.0
                } else {
                    0.0
                },
            ]
        })
        .collect::<Vec<_>>();
    let initial_memory_image = bundle
        .lowered_exports
        .first()
        .map(|export| {
            export
                .program_artifact
                .validated_program
                .initial_memory
                .iter()
                .map(|value| *value as f32)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let entries = vec![
        (
            WeightTensorMetadata::new(
                "call_graph_matrix",
                Shape::new(vec![function_count, function_count]),
                DType::F32,
            ),
            call_graph_matrix.as_slice(),
        ),
        (
            WeightTensorMetadata::new(
                "export_entry_table",
                Shape::new(vec![bundle.specialization_plan.export_summaries.len(), 7]),
                DType::F32,
            ),
            export_entry_table.as_slice(),
        ),
        (
            WeightTensorMetadata::new(
                "function_table",
                Shape::new(vec![
                    bundle.specialization_plan.function_summaries.len(),
                    10,
                ]),
                DType::F32,
            ),
            function_table.as_slice(),
        ),
        (
            WeightTensorMetadata::new(
                "initial_memory_image",
                Shape::new(vec![initial_memory_image.len()]),
                DType::F32,
            ),
            initial_memory_image.as_slice(),
        ),
        (
            WeightTensorMetadata::new(
                "module_header",
                Shape::new(vec![module_header.len()]),
                DType::F32,
            ),
            module_header.as_slice(),
        ),
    ];
    let artifact_bytes = serde_json::to_vec(&CompiledModuleSpecializationArtifactEncoding {
        artifact_id,
        normalized_module_digest: bundle.normalized_module.module_digest.as_str(),
        module_specialization_plan_digest: bundle.specialization_plan.plan_digest.as_str(),
        module_header: module_header.as_slice(),
        function_table: function_table.as_slice(),
        call_graph_matrix: call_graph_matrix.as_slice(),
        export_entry_table: export_entry_table.as_slice(),
        initial_memory_image: initial_memory_image.as_slice(),
    })?;
    let artifact_metadata = vec![WeightArtifactMetadata::new(
        format!("{artifact_id}.compiled_module_specialization.json"),
        artifact_bytes.len() as u64,
        hex::encode(Sha256::digest(artifact_bytes)),
    )];
    let metadata = build_metadata(&entries, WeightSource::ExternalArtifact, artifact_metadata);
    Ok(TassadarCompiledModuleSpecializationWeightBundle {
        metadata,
        module_header,
        function_table,
        call_graph_matrix,
        export_entry_table,
        initial_memory_image,
    })
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

fn stable_serialized_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn sanitize_label(label: &str) -> String {
    let mut sanitized = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    while sanitized.contains("__") {
        sanitized = sanitized.replace("__", "_");
    }
    sanitized.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::TassadarCompiledModuleSpecialization;
    use crate::TassadarExecutorFixture;
    use psionic_ir::tassadar_seeded_multi_function_module;
    use psionic_runtime::{
        tassadar_seeded_module_specialization_call_graph_module,
        tassadar_seeded_module_specialization_memory_call_graph_module,
    };

    #[test]
    fn module_specialization_artifact_is_machine_legible() -> Result<(), Box<dyn std::error::Error>>
    {
        let compiled = TassadarExecutorFixture::core_i32_v2().compile_module_specialization(
            "seeded_call_graph_specialization",
            "seeded_call_graph_module",
            tassadar_seeded_module_specialization_call_graph_module()?,
        )?;
        assert_eq!(compiled.bundle().specialization_plan.export_count, 3);
        assert_eq!(compiled.artifact().exports.len(), 3);
        assert!(
            compiled
                .artifact()
                .exports
                .iter()
                .all(|export| export.exact_trace_match
                    && export.final_output_match
                    && export.final_memory_match)
        );
        assert!(compiled.artifact().compiled_module_weight_artifact_bytes > 0);
        Ok(())
    }

    #[test]
    fn module_specialization_bundle_is_smaller_than_unspecialized_sum_on_seeded_modules()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = TassadarExecutorFixture::core_i32_v2();
        for module in [
            tassadar_seeded_multi_function_module()?,
            tassadar_seeded_module_specialization_call_graph_module()?,
            tassadar_seeded_module_specialization_memory_call_graph_module()?,
        ] {
            let compiled = TassadarCompiledModuleSpecialization::compile(
                "seeded_module_specialization",
                "seeded_module",
                &fixture,
                module,
            )?;
            assert!(
                compiled.artifact().compiled_module_weight_artifact_bytes
                    < compiled.total_unspecialized_compiled_weight_artifact_bytes()
            );
        }
        Ok(())
    }
}
