use std::collections::{BTreeMap, BTreeSet};

use psionic_ir::{
    TassadarNormalizedWasmConstExpr, TassadarNormalizedWasmDataMode,
    TassadarNormalizedWasmExportKind, TassadarNormalizedWasmFunction,
    TassadarNormalizedWasmFunctionBody, TassadarNormalizedWasmFunctionType,
    TassadarNormalizedWasmInstruction, TassadarNormalizedWasmMemory,
    TassadarNormalizedWasmMemoryType, TassadarNormalizedWasmModule,
    TassadarNormalizedWasmModuleError, TassadarNormalizedWasmValueType,
    encode_tassadar_normalized_wasm_module,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarCompilerToolchainIdentity, TassadarCpuReferenceRunner, TassadarExecutionRefusal,
    TassadarHaltReason, TassadarInstruction, TassadarProgram, TassadarProgramArtifact,
    TassadarProgramArtifactError, TassadarProgramSourceIdentity, TassadarProgramSourceKind,
    TassadarTraceAbi, TassadarWasmProfile,
};

const TASSADAR_MODULE_SPECIALIZATION_PLAN_SCHEMA_VERSION: u16 = 1;
const TASSADAR_MODULE_SPECIALIZATION_BUNDLE_SCHEMA_VERSION: u16 = 1;
const TASSADAR_MODULE_SPECIALIZATION_CLAIM_BOUNDARY: &str = "module-aware specialization consumes normalized Wasm module structure plus explicit call-graph reachability to lower one bounded exact export set into digest-bound Tassadar programs; this keeps shared module lineage and import-boundary facts explicit for research-only systems work and does not imply arbitrary Wasm closure or served module-specialized runtime support";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleSpecializationCallEdgeKind {
    DirectDefined,
    DirectImport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializationCallEdge {
    pub caller_function_index: u32,
    pub callee_function_index: u32,
    pub edge_kind: TassadarModuleSpecializationCallEdgeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_ref: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializationFunctionSummary {
    pub function_index: u32,
    pub type_index: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exported_names: Vec<String>,
    pub imported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_ref: Option<String>,
    pub instruction_count: u32,
    pub local_count: u32,
    pub result_count: u32,
    pub has_memory_access: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub direct_defined_callee_indices: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub direct_import_refs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializationExportSummary {
    pub export_name: String,
    pub function_index: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reachable_function_indices: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reachable_import_refs: Vec<String>,
    pub total_instruction_count: u32,
    pub direct_call_edge_count: u32,
    pub contains_memory_access: bool,
    pub call_graph_is_acyclic: bool,
    pub max_inline_depth: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializationPlan {
    pub schema_version: u16,
    pub module_digest: String,
    pub function_count: u32,
    pub defined_function_count: u32,
    pub imported_function_count: u32,
    pub export_count: u32,
    pub memory_count: u32,
    pub data_segment_count: u32,
    pub total_data_bytes: u64,
    pub function_summaries: Vec<TassadarModuleSpecializationFunctionSummary>,
    pub call_edges: Vec<TassadarModuleSpecializationCallEdge>,
    pub export_summaries: Vec<TassadarModuleSpecializationExportSummary>,
    pub claim_boundary: String,
    pub plan_digest: String,
}

impl TassadarModuleSpecializationPlan {
    fn new(
        module: &TassadarNormalizedWasmModule,
        function_summaries: Vec<TassadarModuleSpecializationFunctionSummary>,
        call_edges: Vec<TassadarModuleSpecializationCallEdge>,
        export_summaries: Vec<TassadarModuleSpecializationExportSummary>,
    ) -> Self {
        let defined_function_count = module
            .functions
            .iter()
            .filter(|function| function.body.is_some())
            .count() as u32;
        let imported_function_count = module.functions.len() as u32 - defined_function_count;
        let export_count = module
            .exports
            .iter()
            .filter(|export| export.kind == TassadarNormalizedWasmExportKind::Function)
            .count() as u32;
        let mut plan = Self {
            schema_version: TASSADAR_MODULE_SPECIALIZATION_PLAN_SCHEMA_VERSION,
            module_digest: module.module_digest.clone(),
            function_count: module.functions.len() as u32,
            defined_function_count,
            imported_function_count,
            export_count,
            memory_count: module.memories.len() as u32,
            data_segment_count: module.data_segments.len() as u32,
            total_data_bytes: module
                .data_segments
                .iter()
                .map(|segment| segment.bytes.len() as u64)
                .sum(),
            function_summaries,
            call_edges,
            export_summaries,
            claim_boundary: String::from(TASSADAR_MODULE_SPECIALIZATION_CLAIM_BOUNDARY),
            plan_digest: String::new(),
        };
        plan.plan_digest = stable_digest(b"tassadar_module_specialization_plan|", &plan);
        plan
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingValue {
    Const(i32),
    StackValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializedExportExecutionManifest {
    pub export_name: String,
    pub function_index: u32,
    pub trace_step_count: u64,
    pub expected_outputs: Vec<i32>,
    pub expected_final_memory: Vec<i32>,
    pub halt_reason: TassadarHaltReason,
    pub trace_digest: String,
    pub behavior_digest: String,
    pub execution_digest: String,
}

impl TassadarModuleSpecializedExportExecutionManifest {
    fn new(
        export_name: impl Into<String>,
        function_index: u32,
        trace_step_count: u64,
        expected_outputs: Vec<i32>,
        expected_final_memory: Vec<i32>,
        halt_reason: TassadarHaltReason,
        trace_digest: impl Into<String>,
        behavior_digest: impl Into<String>,
    ) -> Self {
        let mut manifest = Self {
            export_name: export_name.into(),
            function_index,
            trace_step_count,
            expected_outputs,
            expected_final_memory,
            halt_reason,
            trace_digest: trace_digest.into(),
            behavior_digest: behavior_digest.into(),
            execution_digest: String::new(),
        };
        manifest.execution_digest = stable_digest(
            b"tassadar_module_specialized_export_execution_manifest|",
            &manifest,
        );
        manifest
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializedExportArtifact {
    pub export_name: String,
    pub function_index: u32,
    pub reachable_function_indices: Vec<u32>,
    pub program_artifact: TassadarProgramArtifact,
    pub execution_manifest: TassadarModuleSpecializedExportExecutionManifest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleSpecializationBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub claim_class: String,
    pub claim_boundary: String,
    pub source_identity: TassadarProgramSourceIdentity,
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    pub normalized_module: TassadarNormalizedWasmModule,
    pub specialization_plan: TassadarModuleSpecializationPlan,
    pub lowered_exports: Vec<TassadarModuleSpecializedExportArtifact>,
    pub bundle_digest: String,
}

impl TassadarModuleSpecializationBundle {
    fn new(
        bundle_id: impl Into<String>,
        source_identity: TassadarProgramSourceIdentity,
        toolchain_identity: TassadarCompilerToolchainIdentity,
        normalized_module: TassadarNormalizedWasmModule,
        specialization_plan: TassadarModuleSpecializationPlan,
        lowered_exports: Vec<TassadarModuleSpecializedExportArtifact>,
    ) -> Self {
        let mut bundle = Self {
            schema_version: TASSADAR_MODULE_SPECIALIZATION_BUNDLE_SCHEMA_VERSION,
            bundle_id: bundle_id.into(),
            claim_class: String::from("compiled bounded exactness / research-only systems work"),
            claim_boundary: String::from(TASSADAR_MODULE_SPECIALIZATION_CLAIM_BOUNDARY),
            source_identity,
            toolchain_identity,
            normalized_module,
            specialization_plan,
            lowered_exports,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(b"tassadar_module_specialization_bundle|", &bundle);
        bundle
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarModuleSpecializationError {
    #[error(transparent)]
    Module(#[from] TassadarNormalizedWasmModuleError),
    #[error(
        "module-specialization trace ABI profile `{trace_abi_profile_id}` does not match Wasm profile `{wasm_profile_id}`"
    )]
    UnsupportedTraceAbi {
        trace_abi_profile_id: String,
        wasm_profile_id: String,
    },
    #[error("normalized module `{module_digest}` does not export any functions")]
    NoFunctionExports { module_digest: String },
    #[error("export `{export_name}` references imported function {function_index}")]
    ExportedImportUnsupported {
        export_name: String,
        function_index: u32,
    },
    #[error(
        "export `{export_name}` function {function_index} declares unsupported param_count={param_count}"
    )]
    UnsupportedParamCount {
        export_name: String,
        function_index: u32,
        param_count: usize,
    },
    #[error(
        "export `{export_name}` function {function_index} declares unsupported results {result_types:?}"
    )]
    UnsupportedResultTypes {
        export_name: String,
        function_index: u32,
        result_types: Vec<String>,
    },
    #[error(
        "export `{export_name}` function {function_index} declares unsupported local type `{local_type}`"
    )]
    UnsupportedLocalType {
        export_name: String,
        function_index: u32,
        local_type: String,
    },
    #[error(
        "export `{export_name}` function {function_index} requires flat local {local_index}, but the bounded runtime only encodes locals up to {max_supported}"
    )]
    UnsupportedLocalIndex {
        export_name: String,
        function_index: u32,
        local_index: u32,
        max_supported: u8,
    },
    #[error("unsupported Wasm memory shape: {detail}")]
    UnsupportedMemoryShape { detail: String },
    #[error("unsupported data segment {data_index}: {detail}")]
    UnsupportedDataSegment { data_index: u32, detail: String },
    #[error(
        "export `{export_name}` function {function_index} used a dynamic memory address for `{opcode}`; byte-addressed memory ABI closure remains out of scope"
    )]
    UnsupportedDynamicMemoryAddress {
        export_name: String,
        function_index: u32,
        opcode: String,
    },
    #[error(
        "export `{export_name}` function {function_index} used unsupported memory form for `{opcode}`: {detail}"
    )]
    UnsupportedMemoryImmediate {
        export_name: String,
        function_index: u32,
        opcode: String,
        detail: String,
    },
    #[error(
        "export `{export_name}` function {function_index} calls imported function {target_function_index} (`{import_ref}`), but import-boundary specialization remains explicitly unsupported"
    )]
    UnsupportedImportedCall {
        export_name: String,
        function_index: u32,
        target_function_index: u32,
        import_ref: String,
    },
    #[error(
        "export `{export_name}` function {function_index} re-entered function {target_function_index}; recursive module specialization remains explicitly unsupported"
    )]
    UnsupportedRecursiveCall {
        export_name: String,
        function_index: u32,
        target_function_index: u32,
    },
    #[error(
        "export `{export_name}` function {function_index} uses unsupported instruction `{opcode}` for the current module-specialization slice"
    )]
    UnsupportedInstruction {
        export_name: String,
        function_index: u32,
        opcode: String,
    },
    #[error(
        "export `{export_name}` function {function_index} violated bounded stack discipline: {detail}"
    )]
    InvalidStackState {
        export_name: String,
        function_index: u32,
        detail: String,
    },
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
}

impl TassadarModuleSpecializationError {
    #[must_use]
    pub fn kind_slug(&self) -> &'static str {
        match self {
            Self::Module(_) => "module",
            Self::UnsupportedTraceAbi { .. } => "unsupported_trace_abi",
            Self::NoFunctionExports { .. } => "no_function_exports",
            Self::ExportedImportUnsupported { .. } => "exported_import_unsupported",
            Self::UnsupportedParamCount { .. } => "unsupported_param_count",
            Self::UnsupportedResultTypes { .. } => "unsupported_result_types",
            Self::UnsupportedLocalType { .. } => "unsupported_local_type",
            Self::UnsupportedLocalIndex { .. } => "unsupported_local_index",
            Self::UnsupportedMemoryShape { .. } => "unsupported_memory_shape",
            Self::UnsupportedDataSegment { .. } => "unsupported_data_segment",
            Self::UnsupportedDynamicMemoryAddress { .. } => "unsupported_dynamic_memory_address",
            Self::UnsupportedMemoryImmediate { .. } => "unsupported_memory_immediate",
            Self::UnsupportedImportedCall { .. } => "unsupported_imported_call",
            Self::UnsupportedRecursiveCall { .. } => "unsupported_recursive_call",
            Self::UnsupportedInstruction { .. } => "unsupported_instruction",
            Self::InvalidStackState { .. } => "invalid_stack_state",
            Self::Execution(_) => "execution",
            Self::ProgramArtifact(_) => "program_artifact",
        }
    }
}

pub fn build_tassadar_module_specialization_plan(
    module: &TassadarNormalizedWasmModule,
) -> Result<TassadarModuleSpecializationPlan, TassadarModuleSpecializationError> {
    module.validate_internal_consistency()?;

    let mut exported_names_by_function = BTreeMap::<u32, Vec<String>>::new();
    for export in &module.exports {
        if export.kind == TassadarNormalizedWasmExportKind::Function {
            exported_names_by_function
                .entry(export.index)
                .or_default()
                .push(export.export_name.clone());
        }
    }

    let mut call_edges = Vec::new();
    for function in &module.functions {
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        for instruction in &body.instructions {
            if let TassadarNormalizedWasmInstruction::Call { function_index } = instruction {
                let callee = module.functions.get(*function_index as usize).ok_or_else(|| {
                    TassadarModuleSpecializationError::InvalidStackState {
                        export_name: exported_names_by_function
                            .get(&function.function_index)
                            .and_then(|names| names.first().cloned())
                            .unwrap_or_else(|| format!("function_{}", function.function_index)),
                        function_index: function.function_index,
                        detail: format!(
                            "call referenced missing function {function_index} after module validation"
                        ),
                    }
                })?;
                let (edge_kind, import_ref) = if callee.imported_function() {
                    (
                        TassadarModuleSpecializationCallEdgeKind::DirectImport,
                        Some(import_ref(callee)),
                    )
                } else {
                    (
                        TassadarModuleSpecializationCallEdgeKind::DirectDefined,
                        None,
                    )
                };
                call_edges.push(TassadarModuleSpecializationCallEdge {
                    caller_function_index: function.function_index,
                    callee_function_index: *function_index,
                    edge_kind,
                    import_ref,
                });
            }
        }
    }
    call_edges.sort_by(|left, right| {
        (
            left.caller_function_index,
            left.callee_function_index,
            left.import_ref.as_deref().unwrap_or(""),
        )
            .cmp(&(
                right.caller_function_index,
                right.callee_function_index,
                right.import_ref.as_deref().unwrap_or(""),
            ))
    });

    let mut function_summaries = Vec::with_capacity(module.functions.len());
    for function in &module.functions {
        let signature = signature_for_function(module, function)?;
        let body = function.body.as_ref();
        let mut direct_defined_callee_indices = Vec::new();
        let mut direct_import_refs = Vec::new();
        let mut has_memory_access = false;
        if let Some(body) = body {
            for instruction in &body.instructions {
                match instruction {
                    TassadarNormalizedWasmInstruction::I32Load { .. }
                    | TassadarNormalizedWasmInstruction::I32Store { .. }
                    | TassadarNormalizedWasmInstruction::MemorySize { .. }
                    | TassadarNormalizedWasmInstruction::MemoryGrow { .. }
                    | TassadarNormalizedWasmInstruction::MemoryCopy { .. }
                    | TassadarNormalizedWasmInstruction::MemoryFill { .. } => {
                        has_memory_access = true;
                    }
                    TassadarNormalizedWasmInstruction::Call { function_index } => {
                        let callee =
                            module.functions.get(*function_index as usize).ok_or_else(|| {
                                TassadarModuleSpecializationError::InvalidStackState {
                                    export_name: exported_names_by_function
                                        .get(&function.function_index)
                                        .and_then(|names| names.first().cloned())
                                        .unwrap_or_else(|| {
                                            format!("function_{}", function.function_index)
                                        }),
                                    function_index: function.function_index,
                                    detail: format!(
                                        "call referenced missing function {function_index} after module validation"
                                    ),
                                }
                            })?;
                        if callee.imported_function() {
                            direct_import_refs.push(import_ref(callee));
                        } else {
                            direct_defined_callee_indices.push(*function_index);
                        }
                    }
                    _ => {}
                }
            }
        }
        direct_defined_callee_indices.sort_unstable();
        direct_defined_callee_indices.dedup();
        direct_import_refs.sort();
        direct_import_refs.dedup();

        let mut exported_names = exported_names_by_function
            .remove(&function.function_index)
            .unwrap_or_default();
        exported_names.sort();
        function_summaries.push(TassadarModuleSpecializationFunctionSummary {
            function_index: function.function_index,
            type_index: function.type_index,
            exported_names,
            imported: function.imported_function(),
            import_ref: function.imported_function().then(|| import_ref(function)),
            instruction_count: body.map_or(0, |body| body.instructions.len() as u32),
            local_count: body.map_or(0, |body| body.locals.len() as u32),
            result_count: signature.results.len() as u32,
            has_memory_access,
            direct_defined_callee_indices,
            direct_import_refs,
        });
    }
    function_summaries.sort_by_key(|summary| summary.function_index);

    let mut export_summaries = Vec::new();
    for export in &module.exports {
        if export.kind != TassadarNormalizedWasmExportKind::Function {
            continue;
        }
        let mut reachable_functions = BTreeSet::new();
        let mut reachable_import_refs = BTreeSet::new();
        let mut total_instruction_count = 0u32;
        let mut direct_call_edge_count = 0u32;
        let mut contains_memory_access = false;
        let mut max_inline_depth = 0u32;
        let mut call_graph_is_acyclic = true;
        collect_export_reachability(
            module,
            export.index,
            &mut Vec::new(),
            &mut reachable_functions,
            &mut reachable_import_refs,
            &mut total_instruction_count,
            &mut direct_call_edge_count,
            &mut contains_memory_access,
            &mut max_inline_depth,
            &mut call_graph_is_acyclic,
        )?;
        export_summaries.push(TassadarModuleSpecializationExportSummary {
            export_name: export.export_name.clone(),
            function_index: export.index,
            reachable_function_indices: reachable_functions.into_iter().collect(),
            reachable_import_refs: reachable_import_refs.into_iter().collect(),
            total_instruction_count,
            direct_call_edge_count,
            contains_memory_access,
            call_graph_is_acyclic,
            max_inline_depth,
        });
    }
    export_summaries.sort_by(|left, right| left.export_name.cmp(&right.export_name));
    if export_summaries.is_empty() {
        return Err(TassadarModuleSpecializationError::NoFunctionExports {
            module_digest: module.module_digest.clone(),
        });
    }

    Ok(TassadarModuleSpecializationPlan::new(
        module,
        function_summaries,
        call_edges,
        export_summaries,
    ))
}

pub fn compile_tassadar_module_specialization_bundle(
    bundle_id: impl Into<String>,
    source_name: impl Into<String>,
    normalized_module: TassadarNormalizedWasmModule,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
) -> Result<TassadarModuleSpecializationBundle, TassadarModuleSpecializationError> {
    if trace_abi.profile_id != profile.profile_id {
        return Err(TassadarModuleSpecializationError::UnsupportedTraceAbi {
            trace_abi_profile_id: trace_abi.profile_id.clone(),
            wasm_profile_id: profile.profile_id.clone(),
        });
    }
    let specialization_plan = build_tassadar_module_specialization_plan(&normalized_module)?;
    let normalized_module_bytes = encode_tassadar_normalized_wasm_module(&normalized_module)?;
    let normalized_module_digest = stable_bytes_digest(&normalized_module_bytes);
    let source_identity = TassadarProgramSourceIdentity::new(
        TassadarProgramSourceKind::WasmBinary,
        source_name,
        normalized_module_digest.clone(),
    );
    let toolchain_identity = TassadarCompilerToolchainIdentity::new(
        "tassadar_module_specialization",
        "v1",
        profile.profile_id.as_str(),
    )
    .with_pipeline_features(vec![
        String::from("normalized_module_ir"),
        String::from("call_graph_specialization"),
        String::from("direct_call_inlining"),
    ]);
    let base_memory = initial_memory_image(&normalized_module)?;
    let bundle_id = bundle_id.into();
    let mut lowered_exports = Vec::with_capacity(specialization_plan.export_summaries.len());
    for export_summary in &specialization_plan.export_summaries {
        lowered_exports.push(lower_export(
            &bundle_id,
            &normalized_module,
            export_summary,
            &base_memory,
            &normalized_module_digest,
            &source_identity,
            &toolchain_identity,
            profile,
            trace_abi,
        )?);
    }
    Ok(TassadarModuleSpecializationBundle::new(
        bundle_id,
        source_identity,
        toolchain_identity,
        normalized_module,
        specialization_plan,
        lowered_exports,
    ))
}

pub fn tassadar_seeded_module_specialization_call_graph_module()
-> Result<TassadarNormalizedWasmModule, TassadarNormalizedWasmModuleError> {
    TassadarNormalizedWasmModule::new(
        vec![TassadarNormalizedWasmFunctionType {
            type_index: 0,
            params: Vec::new(),
            results: vec![TassadarNormalizedWasmValueType::I32],
        }],
        vec![
            TassadarNormalizedWasmFunction::defined(
                0,
                0,
                TassadarNormalizedWasmFunctionBody::new(
                    Vec::new(),
                    vec![
                        TassadarNormalizedWasmInstruction::I32Const { value: 7 },
                        TassadarNormalizedWasmInstruction::Return,
                    ],
                ),
            ),
            TassadarNormalizedWasmFunction::defined(
                1,
                0,
                TassadarNormalizedWasmFunctionBody::new(
                    vec![TassadarNormalizedWasmValueType::I32],
                    vec![
                        TassadarNormalizedWasmInstruction::I32Const { value: 5 },
                        TassadarNormalizedWasmInstruction::LocalTee { local_index: 0 },
                        TassadarNormalizedWasmInstruction::I32Const { value: 9 },
                        TassadarNormalizedWasmInstruction::I32Mul,
                        TassadarNormalizedWasmInstruction::Return,
                    ],
                ),
            ),
            TassadarNormalizedWasmFunction::defined(
                2,
                0,
                TassadarNormalizedWasmFunctionBody::new(
                    Vec::new(),
                    vec![
                        TassadarNormalizedWasmInstruction::Call { function_index: 0 },
                        TassadarNormalizedWasmInstruction::Call { function_index: 1 },
                        TassadarNormalizedWasmInstruction::I32Add,
                        TassadarNormalizedWasmInstruction::Return,
                    ],
                ),
            ),
        ],
        Vec::<TassadarNormalizedWasmMemory>::new(),
        vec![
            psionic_ir::TassadarNormalizedWasmExport::new(
                "leaf_left",
                TassadarNormalizedWasmExportKind::Function,
                0,
            ),
            psionic_ir::TassadarNormalizedWasmExport::new(
                "leaf_right",
                TassadarNormalizedWasmExportKind::Function,
                1,
            ),
            psionic_ir::TassadarNormalizedWasmExport::new(
                "aggregate",
                TassadarNormalizedWasmExportKind::Function,
                2,
            ),
        ],
        Vec::new(),
    )
}

pub fn tassadar_seeded_module_specialization_memory_call_graph_module()
-> Result<TassadarNormalizedWasmModule, TassadarNormalizedWasmModuleError> {
    TassadarNormalizedWasmModule::new(
        vec![TassadarNormalizedWasmFunctionType {
            type_index: 0,
            params: Vec::new(),
            results: vec![TassadarNormalizedWasmValueType::I32],
        }],
        vec![
            TassadarNormalizedWasmFunction::defined(
                0,
                0,
                TassadarNormalizedWasmFunctionBody::new(
                    Vec::new(),
                    vec![
                        TassadarNormalizedWasmInstruction::I32Const { value: 0 },
                        TassadarNormalizedWasmInstruction::I32Load {
                            align: 2,
                            offset: 0,
                            memory_index: 0,
                        },
                        TassadarNormalizedWasmInstruction::I32Const { value: 0 },
                        TassadarNormalizedWasmInstruction::I32Load {
                            align: 2,
                            offset: 4,
                            memory_index: 0,
                        },
                        TassadarNormalizedWasmInstruction::I32Add,
                        TassadarNormalizedWasmInstruction::Return,
                    ],
                ),
            ),
            TassadarNormalizedWasmFunction::defined(
                1,
                0,
                TassadarNormalizedWasmFunctionBody::new(
                    Vec::new(),
                    vec![
                        TassadarNormalizedWasmInstruction::Call { function_index: 0 },
                        TassadarNormalizedWasmInstruction::I32Const { value: 4 },
                        TassadarNormalizedWasmInstruction::I32Add,
                        TassadarNormalizedWasmInstruction::Return,
                    ],
                ),
            ),
        ],
        vec![TassadarNormalizedWasmMemory::defined(
            0,
            TassadarNormalizedWasmMemoryType {
                minimum_pages: 1,
                maximum_pages: None,
                shared: false,
                memory64: false,
                page_size_log2: None,
            },
        )],
        vec![
            psionic_ir::TassadarNormalizedWasmExport::new(
                "pair_sum",
                TassadarNormalizedWasmExportKind::Function,
                0,
            ),
            psionic_ir::TassadarNormalizedWasmExport::new(
                "pair_sum_plus_four",
                TassadarNormalizedWasmExportKind::Function,
                1,
            ),
            psionic_ir::TassadarNormalizedWasmExport::new(
                "memory",
                TassadarNormalizedWasmExportKind::Memory,
                0,
            ),
        ],
        vec![psionic_ir::TassadarNormalizedWasmDataSegment::new(
            0,
            TassadarNormalizedWasmDataMode::Active {
                memory_index: 0,
                offset_expr: TassadarNormalizedWasmConstExpr::I32Const { value: 0 },
            },
            vec![2, 0, 0, 0, 3, 0, 0, 0],
        )],
    )
}

pub fn tassadar_seeded_module_specialization_import_boundary_module()
-> Result<TassadarNormalizedWasmModule, TassadarNormalizedWasmModuleError> {
    TassadarNormalizedWasmModule::new(
        vec![TassadarNormalizedWasmFunctionType {
            type_index: 0,
            params: Vec::new(),
            results: vec![TassadarNormalizedWasmValueType::I32],
        }],
        vec![
            TassadarNormalizedWasmFunction::imported(0, 0, "env", "const_seven"),
            TassadarNormalizedWasmFunction::defined(
                1,
                0,
                TassadarNormalizedWasmFunctionBody::new(
                    Vec::new(),
                    vec![
                        TassadarNormalizedWasmInstruction::Call { function_index: 0 },
                        TassadarNormalizedWasmInstruction::I32Const { value: 1 },
                        TassadarNormalizedWasmInstruction::I32Add,
                        TassadarNormalizedWasmInstruction::Return,
                    ],
                ),
            ),
        ],
        Vec::new(),
        vec![psionic_ir::TassadarNormalizedWasmExport::new(
            "import_bridge",
            TassadarNormalizedWasmExportKind::Function,
            1,
        )],
        Vec::new(),
    )
}

struct LoweringContext<'a> {
    module: &'a TassadarNormalizedWasmModule,
    export_name: &'a str,
    local_bases: &'a BTreeMap<u32, u32>,
    total_local_count: usize,
    sink_local_index: Option<u32>,
    runtime_instructions: Vec<TassadarInstruction>,
    max_slot: usize,
}

fn collect_export_reachability(
    module: &TassadarNormalizedWasmModule,
    function_index: u32,
    path: &mut Vec<u32>,
    reachable_functions: &mut BTreeSet<u32>,
    reachable_import_refs: &mut BTreeSet<String>,
    total_instruction_count: &mut u32,
    direct_call_edge_count: &mut u32,
    contains_memory_access: &mut bool,
    max_inline_depth: &mut u32,
    call_graph_is_acyclic: &mut bool,
) -> Result<(), TassadarModuleSpecializationError> {
    if path.contains(&function_index) {
        *call_graph_is_acyclic = false;
        return Ok(());
    }
    let function = module
        .functions
        .get(function_index as usize)
        .ok_or_else(|| TassadarModuleSpecializationError::InvalidStackState {
            export_name: format!("function_{function_index}"),
            function_index,
            detail: String::from("reachability referenced a missing function"),
        })?;
    *max_inline_depth = (*max_inline_depth).max(path.len() as u32 + 1);
    if function.imported_function() {
        reachable_import_refs.insert(import_ref(function));
        return Ok(());
    }
    if !reachable_functions.insert(function_index) {
        return Ok(());
    }
    path.push(function_index);
    let body = function.body.as_ref().ok_or_else(|| {
        TassadarModuleSpecializationError::InvalidStackState {
            export_name: format!("function_{function_index}"),
            function_index,
            detail: String::from("defined function unexpectedly omitted its body"),
        }
    })?;
    *total_instruction_count += body.instructions.len() as u32;
    for instruction in &body.instructions {
        match instruction {
            TassadarNormalizedWasmInstruction::I32Load { .. }
            | TassadarNormalizedWasmInstruction::I32Store { .. }
            | TassadarNormalizedWasmInstruction::MemorySize { .. }
            | TassadarNormalizedWasmInstruction::MemoryGrow { .. }
            | TassadarNormalizedWasmInstruction::MemoryCopy { .. }
            | TassadarNormalizedWasmInstruction::MemoryFill { .. } => {
                *contains_memory_access = true;
            }
            TassadarNormalizedWasmInstruction::Call { function_index } => {
                *direct_call_edge_count += 1;
                collect_export_reachability(
                    module,
                    *function_index,
                    path,
                    reachable_functions,
                    reachable_import_refs,
                    total_instruction_count,
                    direct_call_edge_count,
                    contains_memory_access,
                    max_inline_depth,
                    call_graph_is_acyclic,
                )?;
            }
            _ => {}
        }
    }
    path.pop();
    Ok(())
}

fn lower_export(
    bundle_id: &str,
    module: &TassadarNormalizedWasmModule,
    export_summary: &TassadarModuleSpecializationExportSummary,
    base_memory: &[i32],
    wasm_binary_digest: &str,
    source_identity: &TassadarProgramSourceIdentity,
    toolchain_identity: &TassadarCompilerToolchainIdentity,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
) -> Result<TassadarModuleSpecializedExportArtifact, TassadarModuleSpecializationError> {
    let function = module
        .functions
        .get(export_summary.function_index as usize)
        .ok_or_else(|| TassadarModuleSpecializationError::InvalidStackState {
            export_name: export_summary.export_name.clone(),
            function_index: export_summary.function_index,
            detail: String::from("export referenced a missing function after module validation"),
        })?;
    if function.imported_function() {
        return Err(
            TassadarModuleSpecializationError::ExportedImportUnsupported {
                export_name: export_summary.export_name.clone(),
                function_index: export_summary.function_index,
            },
        );
    }
    let signature = signature_for_function(module, function)?;
    validate_signature_support(
        &export_summary.export_name,
        export_summary.function_index,
        signature,
    )?;

    let mut total_local_count = 0usize;
    let mut local_bases = BTreeMap::new();
    for function_index in &export_summary.reachable_function_indices {
        let reachable_function =
            module
                .functions
                .get(*function_index as usize)
                .ok_or_else(|| TassadarModuleSpecializationError::InvalidStackState {
                    export_name: export_summary.export_name.clone(),
                    function_index: *function_index,
                    detail: String::from("reachable function disappeared after module validation"),
                })?;
        let reachable_signature = signature_for_function(module, reachable_function)?;
        validate_signature_support(
            &export_summary.export_name,
            reachable_function.function_index,
            reachable_signature,
        )?;
        let body = reachable_function.body.as_ref().ok_or_else(|| {
            TassadarModuleSpecializationError::InvalidStackState {
                export_name: export_summary.export_name.clone(),
                function_index: reachable_function.function_index,
                detail: String::from("reachable defined function omitted its body"),
            }
        })?;
        for local in &body.locals {
            if *local != TassadarNormalizedWasmValueType::I32 {
                return Err(TassadarModuleSpecializationError::UnsupportedLocalType {
                    export_name: export_summary.export_name.clone(),
                    function_index: reachable_function.function_index,
                    local_type: format!("{local:?}"),
                });
            }
        }
        local_bases.insert(*function_index, total_local_count as u32);
        total_local_count = total_local_count.saturating_add(body.locals.len());
    }
    let requires_drop_sink = export_summary
        .reachable_function_indices
        .iter()
        .filter_map(|function_index| module.functions.get(*function_index as usize))
        .filter_map(|function| function.body.as_ref())
        .any(|body| {
            body.instructions
                .iter()
                .any(|instruction| matches!(instruction, TassadarNormalizedWasmInstruction::Drop))
        });
    let sink_local_index = if requires_drop_sink {
        Some(total_local_count as u32)
    } else {
        None
    };
    let program_local_count = total_local_count + usize::from(requires_drop_sink);
    validate_flat_local_count(
        &export_summary.export_name,
        export_summary.function_index,
        program_local_count,
    )?;

    let mut context = LoweringContext {
        module,
        export_name: export_summary.export_name.as_str(),
        local_bases: &local_bases,
        total_local_count: program_local_count,
        sink_local_index,
        runtime_instructions: Vec::new(),
        max_slot: base_memory.len().saturating_sub(1),
    };
    let returned = lower_defined_function(
        &mut context,
        export_summary.function_index,
        true,
        &mut Vec::new(),
    )?;
    emit_export_terminal(
        &export_summary.export_name,
        export_summary.function_index,
        signature.results.as_slice(),
        returned,
        &mut context.runtime_instructions,
        context.total_local_count,
    )?;

    let mut initial_memory = base_memory.to_vec();
    if context.max_slot + 1 > initial_memory.len() {
        initial_memory.resize(context.max_slot + 1, 0);
    }
    let program_id = format!(
        "{}.{}",
        sanitize_label(bundle_id),
        sanitize_label(&export_summary.export_name)
    );
    let program = TassadarProgram::new(
        program_id.clone(),
        profile,
        program_local_count,
        initial_memory.len(),
        context.runtime_instructions,
    )
    .with_initial_memory(initial_memory);
    let mut program_artifact = TassadarProgramArtifact::new(
        format!("{program_id}.artifact"),
        source_identity.clone(),
        toolchain_identity.clone(),
        profile,
        trace_abi,
        program,
    )?;
    program_artifact = program_artifact.with_wasm_binary_digest(wasm_binary_digest.to_string());

    let execution = TassadarCpuReferenceRunner::for_program(&program_artifact.validated_program)?
        .execute(&program_artifact.validated_program)?;
    let execution_manifest = TassadarModuleSpecializedExportExecutionManifest::new(
        export_summary.export_name.clone(),
        export_summary.function_index,
        execution.steps.len() as u64,
        execution.outputs.clone(),
        execution.final_memory.clone(),
        execution.halt_reason,
        execution.trace_digest(),
        execution.behavior_digest(),
    );

    Ok(TassadarModuleSpecializedExportArtifact {
        export_name: export_summary.export_name.clone(),
        function_index: export_summary.function_index,
        reachable_function_indices: export_summary.reachable_function_indices.clone(),
        program_artifact,
        execution_manifest,
    })
}

fn lower_defined_function(
    context: &mut LoweringContext<'_>,
    function_index: u32,
    export_entry: bool,
    call_path: &mut Vec<u32>,
) -> Result<Option<PendingValue>, TassadarModuleSpecializationError> {
    if let Some(caller_function_index) = call_path.last().copied()
        && call_path.contains(&function_index)
    {
        return Err(
            TassadarModuleSpecializationError::UnsupportedRecursiveCall {
                export_name: context.export_name.to_string(),
                function_index: caller_function_index,
                target_function_index: function_index,
            },
        );
    }
    let function = context
        .module
        .functions
        .get(function_index as usize)
        .ok_or_else(|| TassadarModuleSpecializationError::InvalidStackState {
            export_name: context.export_name.to_string(),
            function_index,
            detail: String::from("lowering referenced a missing function"),
        })?;
    let body = function.body.as_ref().ok_or_else(|| {
        TassadarModuleSpecializationError::InvalidStackState {
            export_name: context.export_name.to_string(),
            function_index,
            detail: String::from("defined function unexpectedly omitted its body"),
        }
    })?;
    let signature = signature_for_function(context.module, function)?;
    if !export_entry {
        zero_initialize_locals(
            context.export_name,
            function_index,
            context.local_bases,
            body.locals.len(),
            &mut context.runtime_instructions,
            context.total_local_count,
        )?;
    }

    call_path.push(function_index);
    let result = (|| {
        let mut stack = Vec::<PendingValue>::new();
        for instruction in &body.instructions {
            match instruction {
                TassadarNormalizedWasmInstruction::I32Const { value } => {
                    stack.push(PendingValue::Const(*value));
                }
                TassadarNormalizedWasmInstruction::LocalGet { local_index } => {
                    let flat_local = mapped_local_index(
                        context.export_name,
                        function_index,
                        *local_index,
                        context.local_bases,
                        body.locals.len(),
                    )?;
                    context
                        .runtime_instructions
                        .push(TassadarInstruction::LocalGet { local: flat_local });
                    stack.push(PendingValue::StackValue);
                }
                TassadarNormalizedWasmInstruction::LocalSet { local_index } => {
                    let flat_local = mapped_local_index(
                        context.export_name,
                        function_index,
                        *local_index,
                        context.local_bases,
                        body.locals.len(),
                    )?;
                    let value = pop_stack_value(
                        context.export_name,
                        function_index,
                        &mut stack,
                        "local.set",
                    )?;
                    materialize_value(value, &mut context.runtime_instructions, flat_local);
                    context
                        .runtime_instructions
                        .push(TassadarInstruction::LocalSet { local: flat_local });
                }
                TassadarNormalizedWasmInstruction::LocalTee { local_index } => {
                    let flat_local = mapped_local_index(
                        context.export_name,
                        function_index,
                        *local_index,
                        context.local_bases,
                        body.locals.len(),
                    )?;
                    let value = pop_stack_value(
                        context.export_name,
                        function_index,
                        &mut stack,
                        "local.tee",
                    )?;
                    match value {
                        PendingValue::Const(value) => {
                            context
                                .runtime_instructions
                                .push(TassadarInstruction::I32Const { value });
                            context
                                .runtime_instructions
                                .push(TassadarInstruction::LocalSet { local: flat_local });
                            stack.push(PendingValue::Const(value));
                        }
                        PendingValue::StackValue => {
                            context
                                .runtime_instructions
                                .push(TassadarInstruction::LocalSet { local: flat_local });
                            context
                                .runtime_instructions
                                .push(TassadarInstruction::LocalGet { local: flat_local });
                            stack.push(PendingValue::StackValue);
                        }
                    }
                }
                TassadarNormalizedWasmInstruction::GlobalGet { .. }
                | TassadarNormalizedWasmInstruction::GlobalSet { .. }
                | TassadarNormalizedWasmInstruction::CallIndirect { .. }
                | TassadarNormalizedWasmInstruction::MemorySize { .. }
                | TassadarNormalizedWasmInstruction::MemoryGrow { .. }
                | TassadarNormalizedWasmInstruction::MemoryCopy { .. }
                | TassadarNormalizedWasmInstruction::MemoryFill { .. } => {
                    return Err(TassadarModuleSpecializationError::UnsupportedInstruction {
                        export_name: context.export_name.to_string(),
                        function_index,
                        opcode: instruction.mnemonic().to_string(),
                    });
                }
                TassadarNormalizedWasmInstruction::I32Add
                | TassadarNormalizedWasmInstruction::I32Sub
                | TassadarNormalizedWasmInstruction::I32Mul
                | TassadarNormalizedWasmInstruction::I32LtS => {
                    let right = pop_stack_value(
                        context.export_name,
                        function_index,
                        &mut stack,
                        instruction.mnemonic(),
                    )?;
                    let left = pop_stack_value(
                        context.export_name,
                        function_index,
                        &mut stack,
                        instruction.mnemonic(),
                    )?;
                    if let (PendingValue::Const(left), PendingValue::Const(right)) = (left, right) {
                        stack.push(PendingValue::Const(match instruction {
                            TassadarNormalizedWasmInstruction::I32Add => left.saturating_add(right),
                            TassadarNormalizedWasmInstruction::I32Sub => left.saturating_sub(right),
                            TassadarNormalizedWasmInstruction::I32Mul => left.saturating_mul(right),
                            TassadarNormalizedWasmInstruction::I32LtS => i32::from(left < right),
                            _ => unreachable!("filtered above"),
                        }));
                    } else {
                        materialize_pending_value(left, &mut context.runtime_instructions);
                        materialize_pending_value(right, &mut context.runtime_instructions);
                        context.runtime_instructions.push(match instruction {
                            TassadarNormalizedWasmInstruction::I32Add => {
                                TassadarInstruction::I32Add
                            }
                            TassadarNormalizedWasmInstruction::I32Sub => {
                                TassadarInstruction::I32Sub
                            }
                            TassadarNormalizedWasmInstruction::I32Mul => {
                                TassadarInstruction::I32Mul
                            }
                            TassadarNormalizedWasmInstruction::I32LtS => TassadarInstruction::I32Lt,
                            _ => unreachable!("filtered above"),
                        });
                        stack.push(PendingValue::StackValue);
                    }
                }
                TassadarNormalizedWasmInstruction::I32Load {
                    offset,
                    memory_index,
                    ..
                } => {
                    let address = pop_stack_value(
                        context.export_name,
                        function_index,
                        &mut stack,
                        "i32.load",
                    )?;
                    let slot = resolve_memory_slot(
                        context.export_name,
                        function_index,
                        "i32.load",
                        address,
                        *offset,
                        *memory_index,
                    )?;
                    context.max_slot = context.max_slot.max(usize::from(slot));
                    context
                        .runtime_instructions
                        .push(TassadarInstruction::I32Load { slot });
                    stack.push(PendingValue::StackValue);
                }
                TassadarNormalizedWasmInstruction::I32Store {
                    offset,
                    memory_index,
                    ..
                } => {
                    let value = pop_stack_value(
                        context.export_name,
                        function_index,
                        &mut stack,
                        "i32.store",
                    )?;
                    let address = pop_stack_value(
                        context.export_name,
                        function_index,
                        &mut stack,
                        "i32.store",
                    )?;
                    let slot = resolve_memory_slot(
                        context.export_name,
                        function_index,
                        "i32.store",
                        address,
                        *offset,
                        *memory_index,
                    )?;
                    context.max_slot = context.max_slot.max(usize::from(slot));
                    materialize_pending_value(value, &mut context.runtime_instructions);
                    context
                        .runtime_instructions
                        .push(TassadarInstruction::I32Store { slot });
                }
                TassadarNormalizedWasmInstruction::Call {
                    function_index: target,
                } => {
                    let callee = context.module.functions.get(*target as usize).ok_or_else(
                        || TassadarModuleSpecializationError::InvalidStackState {
                            export_name: context.export_name.to_string(),
                            function_index,
                            detail: format!(
                                "call referenced missing function {target} after module validation"
                            ),
                        },
                    )?;
                    if callee.imported_function() {
                        return Err(TassadarModuleSpecializationError::UnsupportedImportedCall {
                            export_name: context.export_name.to_string(),
                            function_index,
                            target_function_index: *target,
                            import_ref: import_ref(callee),
                        });
                    }
                    if let Some(result) =
                        lower_defined_function(context, *target, false, call_path)?
                    {
                        stack.push(result);
                    }
                }
                TassadarNormalizedWasmInstruction::Drop => {
                    let value =
                        pop_stack_value(context.export_name, function_index, &mut stack, "drop")?;
                    drop_pending_value(
                        context.export_name,
                        function_index,
                        value,
                        context.sink_local_index,
                        &mut context.runtime_instructions,
                    )?;
                }
                TassadarNormalizedWasmInstruction::Return => {
                    let result = finalize_function_result(
                        context.export_name,
                        function_index,
                        signature.results.as_slice(),
                        &mut stack,
                    )?;
                    return Ok(if export_entry {
                        result
                    } else {
                        escape_function_result(result)
                    });
                }
                TassadarNormalizedWasmInstruction::I32Shl => {
                    return Err(TassadarModuleSpecializationError::UnsupportedInstruction {
                        export_name: context.export_name.to_string(),
                        function_index,
                        opcode: String::from("i32.shl"),
                    });
                }
            }
        }
        let result = finalize_function_result(
            context.export_name,
            function_index,
            signature.results.as_slice(),
            &mut stack,
        )?;
        Ok(if export_entry {
            result
        } else {
            escape_function_result(result)
        })
    })();
    call_path.pop();
    result
}

fn validate_signature_support(
    export_name: &str,
    function_index: u32,
    signature: &TassadarNormalizedWasmFunctionType,
) -> Result<(), TassadarModuleSpecializationError> {
    if !signature.params.is_empty() {
        return Err(TassadarModuleSpecializationError::UnsupportedParamCount {
            export_name: export_name.to_string(),
            function_index,
            param_count: signature.params.len(),
        });
    }
    if signature.results.len() > 1
        || signature
            .results
            .iter()
            .any(|result| *result != TassadarNormalizedWasmValueType::I32)
    {
        return Err(TassadarModuleSpecializationError::UnsupportedResultTypes {
            export_name: export_name.to_string(),
            function_index,
            result_types: signature
                .results
                .iter()
                .map(|result| format!("{result:?}"))
                .collect(),
        });
    }
    Ok(())
}

fn signature_for_function<'a>(
    module: &'a TassadarNormalizedWasmModule,
    function: &TassadarNormalizedWasmFunction,
) -> Result<&'a TassadarNormalizedWasmFunctionType, TassadarModuleSpecializationError> {
    module
        .types
        .get(function.type_index as usize)
        .ok_or_else(|| TassadarModuleSpecializationError::InvalidStackState {
            export_name: format!("function_{}", function.function_index),
            function_index: function.function_index,
            detail: format!(
                "function referenced missing type {} after module validation",
                function.type_index
            ),
        })
}

fn initial_memory_image(
    module: &TassadarNormalizedWasmModule,
) -> Result<Vec<i32>, TassadarModuleSpecializationError> {
    if module.memories.len() > 1 {
        return Err(TassadarModuleSpecializationError::UnsupportedMemoryShape {
            detail: format!("memory_count={}", module.memories.len()),
        });
    }
    if let Some(memory) = module.memories.first()
        && (memory.memory_type.shared
            || memory.memory_type.memory64
            || memory.memory_type.page_size_log2.is_some())
    {
        return Err(TassadarModuleSpecializationError::UnsupportedMemoryShape {
            detail: format!(
                "shared={} memory64={} page_size_log2={:?}",
                memory.memory_type.shared,
                memory.memory_type.memory64,
                memory.memory_type.page_size_log2
            ),
        });
    }

    let mut memory = Vec::<i32>::new();
    for segment in &module.data_segments {
        let (memory_index, offset) = match &segment.mode {
            TassadarNormalizedWasmDataMode::Passive => continue,
            TassadarNormalizedWasmDataMode::Active {
                memory_index,
                offset_expr,
            } => match offset_expr {
                TassadarNormalizedWasmConstExpr::I32Const { value } if *value >= 0 => {
                    (*memory_index, *value as u64)
                }
                TassadarNormalizedWasmConstExpr::I32Const { value } => {
                    return Err(TassadarModuleSpecializationError::UnsupportedDataSegment {
                        data_index: segment.data_index,
                        detail: format!("negative offset {value}"),
                    });
                }
                other => {
                    return Err(TassadarModuleSpecializationError::UnsupportedDataSegment {
                        data_index: segment.data_index,
                        detail: format!("non-constant offset {other:?}"),
                    });
                }
            },
        };
        if memory_index != 0 {
            return Err(TassadarModuleSpecializationError::UnsupportedDataSegment {
                data_index: segment.data_index,
                detail: format!("memory index {memory_index}"),
            });
        }
        if offset % 4 != 0 {
            return Err(TassadarModuleSpecializationError::UnsupportedDataSegment {
                data_index: segment.data_index,
                detail: format!("byte offset {offset} is not word aligned"),
            });
        }
        if segment.bytes.len() % 4 != 0 {
            return Err(TassadarModuleSpecializationError::UnsupportedDataSegment {
                data_index: segment.data_index,
                detail: format!("byte length {} is not a multiple of 4", segment.bytes.len()),
            });
        }
        let start_slot = (offset / 4) as usize;
        let end_slot = start_slot + (segment.bytes.len() / 4);
        if end_slot > memory.len() {
            memory.resize(end_slot, 0);
        }
        for (slot_offset, chunk) in segment.bytes.chunks_exact(4).enumerate() {
            memory[start_slot + slot_offset] =
                i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
    }
    Ok(memory)
}

fn mapped_local_index(
    export_name: &str,
    function_index: u32,
    local_index: u32,
    local_bases: &BTreeMap<u32, u32>,
    function_local_count: usize,
) -> Result<u8, TassadarModuleSpecializationError> {
    if local_index as usize >= function_local_count {
        return Err(TassadarModuleSpecializationError::InvalidStackState {
            export_name: export_name.to_string(),
            function_index,
            detail: format!(
                "local {local_index} is out of range for {function_local_count} locals"
            ),
        });
    }
    let base = local_bases.get(&function_index).copied().ok_or_else(|| {
        TassadarModuleSpecializationError::InvalidStackState {
            export_name: export_name.to_string(),
            function_index,
            detail: String::from("missing local-base assignment for reachable function"),
        }
    })?;
    let flat_local = base.saturating_add(local_index);
    u8::try_from(flat_local).map_err(
        |_| TassadarModuleSpecializationError::UnsupportedLocalIndex {
            export_name: export_name.to_string(),
            function_index,
            local_index: flat_local,
            max_supported: u8::MAX,
        },
    )
}

fn validate_flat_local_count(
    export_name: &str,
    function_index: u32,
    local_count: usize,
) -> Result<(), TassadarModuleSpecializationError> {
    if local_count > usize::from(u8::MAX) + 1 {
        return Err(TassadarModuleSpecializationError::UnsupportedLocalIndex {
            export_name: export_name.to_string(),
            function_index,
            local_index: local_count as u32,
            max_supported: u8::MAX,
        });
    }
    Ok(())
}

fn zero_initialize_locals(
    export_name: &str,
    function_index: u32,
    local_bases: &BTreeMap<u32, u32>,
    local_count: usize,
    runtime_instructions: &mut Vec<TassadarInstruction>,
    total_local_count: usize,
) -> Result<(), TassadarModuleSpecializationError> {
    for local_index in 0..local_count {
        let flat_local = mapped_local_index(
            export_name,
            function_index,
            local_index as u32,
            local_bases,
            local_count,
        )?;
        if usize::from(flat_local) >= total_local_count {
            return Err(TassadarModuleSpecializationError::InvalidStackState {
                export_name: export_name.to_string(),
                function_index,
                detail: format!(
                    "flat local {flat_local} exceeded total_local_count={total_local_count}"
                ),
            });
        }
        runtime_instructions.push(TassadarInstruction::I32Const { value: 0 });
        runtime_instructions.push(TassadarInstruction::LocalSet { local: flat_local });
    }
    Ok(())
}

fn pop_stack_value(
    export_name: &str,
    function_index: u32,
    stack: &mut Vec<PendingValue>,
    opcode: &str,
) -> Result<PendingValue, TassadarModuleSpecializationError> {
    stack
        .pop()
        .ok_or_else(|| TassadarModuleSpecializationError::InvalidStackState {
            export_name: export_name.to_string(),
            function_index,
            detail: format!("stack underflow while lowering `{opcode}`"),
        })
}

fn materialize_value(
    value: PendingValue,
    runtime_instructions: &mut Vec<TassadarInstruction>,
    _local: u8,
) {
    if let PendingValue::Const(value) = value {
        runtime_instructions.push(TassadarInstruction::I32Const { value });
    }
}

fn materialize_pending_value(
    value: PendingValue,
    runtime_instructions: &mut Vec<TassadarInstruction>,
) {
    if let PendingValue::Const(value) = value {
        runtime_instructions.push(TassadarInstruction::I32Const { value });
    }
}

fn resolve_memory_slot(
    export_name: &str,
    function_index: u32,
    opcode: &str,
    address: PendingValue,
    offset: u64,
    memory_index: u32,
) -> Result<u8, TassadarModuleSpecializationError> {
    if memory_index != 0 {
        return Err(
            TassadarModuleSpecializationError::UnsupportedMemoryImmediate {
                export_name: export_name.to_string(),
                function_index,
                opcode: opcode.to_string(),
                detail: format!("memory index {memory_index}"),
            },
        );
    }
    let base = match address {
        PendingValue::Const(value) if value >= 0 => value as u64,
        PendingValue::Const(value) => {
            return Err(
                TassadarModuleSpecializationError::UnsupportedMemoryImmediate {
                    export_name: export_name.to_string(),
                    function_index,
                    opcode: opcode.to_string(),
                    detail: format!("negative address {value}"),
                },
            );
        }
        PendingValue::StackValue => {
            return Err(
                TassadarModuleSpecializationError::UnsupportedDynamicMemoryAddress {
                    export_name: export_name.to_string(),
                    function_index,
                    opcode: opcode.to_string(),
                },
            );
        }
    };
    let absolute = base.saturating_add(offset);
    if absolute % 4 != 0 {
        return Err(
            TassadarModuleSpecializationError::UnsupportedMemoryImmediate {
                export_name: export_name.to_string(),
                function_index,
                opcode: opcode.to_string(),
                detail: format!("absolute byte address {absolute} is not word aligned"),
            },
        );
    }
    let slot = absolute / 4;
    u8::try_from(slot).map_err(
        |_| TassadarModuleSpecializationError::UnsupportedMemoryImmediate {
            export_name: export_name.to_string(),
            function_index,
            opcode: opcode.to_string(),
            detail: format!("slot {slot} exceeds u8 runtime address space"),
        },
    )
}

fn finalize_function_result(
    export_name: &str,
    function_index: u32,
    results: &[TassadarNormalizedWasmValueType],
    stack: &mut Vec<PendingValue>,
) -> Result<Option<PendingValue>, TassadarModuleSpecializationError> {
    match results {
        [] => {
            if !stack.is_empty() {
                return Err(TassadarModuleSpecializationError::InvalidStackState {
                    export_name: export_name.to_string(),
                    function_index,
                    detail: format!("void return left {} values on the stack", stack.len()),
                });
            }
            Ok(None)
        }
        [TassadarNormalizedWasmValueType::I32] => {
            let result = pop_stack_value(export_name, function_index, stack, "return")?;
            if !stack.is_empty() {
                return Err(TassadarModuleSpecializationError::InvalidStackState {
                    export_name: export_name.to_string(),
                    function_index,
                    detail: format!(
                        "return left {} extra values below the final result",
                        stack.len()
                    ),
                });
            }
            Ok(Some(result))
        }
        other => Err(TassadarModuleSpecializationError::UnsupportedResultTypes {
            export_name: export_name.to_string(),
            function_index,
            result_types: other.iter().map(|value| format!("{value:?}")).collect(),
        }),
    }
}

fn escape_function_result(result: Option<PendingValue>) -> Option<PendingValue> {
    match result {
        Some(PendingValue::Const(value)) => Some(PendingValue::Const(value)),
        Some(PendingValue::StackValue) => Some(PendingValue::StackValue),
        None => None,
    }
}

fn emit_export_terminal(
    export_name: &str,
    function_index: u32,
    results: &[TassadarNormalizedWasmValueType],
    result: Option<PendingValue>,
    runtime_instructions: &mut Vec<TassadarInstruction>,
    _local_count: usize,
) -> Result<(), TassadarModuleSpecializationError> {
    match (results, result) {
        ([], None) => {}
        ([TassadarNormalizedWasmValueType::I32], Some(result)) => {
            materialize_pending_value(result, runtime_instructions);
            runtime_instructions.push(TassadarInstruction::Output);
        }
        ([TassadarNormalizedWasmValueType::I32], None) => {
            return Err(TassadarModuleSpecializationError::InvalidStackState {
                export_name: export_name.to_string(),
                function_index,
                detail: String::from("entry function returned no value for i32 result"),
            });
        }
        ([], Some(_)) => {
            return Err(TassadarModuleSpecializationError::InvalidStackState {
                export_name: export_name.to_string(),
                function_index,
                detail: String::from("void entry function returned a value"),
            });
        }
        (other, _) => {
            return Err(TassadarModuleSpecializationError::UnsupportedResultTypes {
                export_name: export_name.to_string(),
                function_index,
                result_types: other.iter().map(|value| format!("{value:?}")).collect(),
            });
        }
    }
    runtime_instructions.push(TassadarInstruction::Return);
    Ok(())
}

fn drop_pending_value(
    export_name: &str,
    function_index: u32,
    value: PendingValue,
    sink_local_index: Option<u32>,
    runtime_instructions: &mut Vec<TassadarInstruction>,
) -> Result<(), TassadarModuleSpecializationError> {
    match value {
        PendingValue::Const(_) => Ok(()),
        PendingValue::StackValue => {
            let sink_local_index = sink_local_index.ok_or_else(|| {
                TassadarModuleSpecializationError::InvalidStackState {
                    export_name: export_name.to_string(),
                    function_index,
                    detail: String::from("drop required a sink local but none was allocated"),
                }
            })?;
            runtime_instructions.push(TassadarInstruction::LocalSet {
                local: u8::try_from(sink_local_index).map_err(|_| {
                    TassadarModuleSpecializationError::UnsupportedLocalIndex {
                        export_name: export_name.to_string(),
                        function_index,
                        local_index: sink_local_index,
                        max_supported: u8::MAX,
                    }
                })?,
            });
            Ok(())
        }
    }
}

fn import_ref(function: &TassadarNormalizedWasmFunction) -> String {
    format!(
        "{}::{}",
        function.import_module.as_deref().unwrap_or(""),
        function.import_name.as_deref().unwrap_or("")
    )
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

fn stable_bytes_digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarModuleSpecializationError, build_tassadar_module_specialization_plan,
        compile_tassadar_module_specialization_bundle,
        tassadar_seeded_module_specialization_call_graph_module,
        tassadar_seeded_module_specialization_import_boundary_module,
        tassadar_seeded_module_specialization_memory_call_graph_module,
    };
    use crate::{TassadarTraceAbi, TassadarWasmProfile};
    use psionic_ir::tassadar_seeded_multi_function_module;

    #[test]
    fn module_specialization_plan_tracks_call_graph_and_exports()
    -> Result<(), Box<dyn std::error::Error>> {
        let module = tassadar_seeded_module_specialization_call_graph_module()?;
        let plan = build_tassadar_module_specialization_plan(&module)?;
        assert_eq!(plan.export_count, 3);
        assert_eq!(plan.function_count, 3);
        let aggregate = plan
            .export_summaries
            .iter()
            .find(|summary| summary.export_name == "aggregate")
            .expect("aggregate export should be present");
        assert_eq!(aggregate.reachable_function_indices, vec![0, 1, 2]);
        assert_eq!(aggregate.direct_call_edge_count, 2);
        assert!(aggregate.call_graph_is_acyclic);
        Ok(())
    }

    #[test]
    fn module_specialization_lowering_executes_multi_function_exports_exactly()
    -> Result<(), Box<dyn std::error::Error>> {
        let profile = TassadarWasmProfile::core_i32_v2();
        let trace_abi = TassadarTraceAbi::core_i32_v2();
        let independent = compile_tassadar_module_specialization_bundle(
            "seeded_multi_function",
            "seeded_multi_function_module",
            tassadar_seeded_multi_function_module()?,
            &profile,
            &trace_abi,
        )?;
        let call_graph = compile_tassadar_module_specialization_bundle(
            "seeded_call_graph",
            "seeded_call_graph_module",
            tassadar_seeded_module_specialization_call_graph_module()?,
            &profile,
            &trace_abi,
        )?;
        let memory_graph = compile_tassadar_module_specialization_bundle(
            "seeded_memory_call_graph",
            "seeded_memory_call_graph_module",
            tassadar_seeded_module_specialization_memory_call_graph_module()?,
            &profile,
            &trace_abi,
        )?;

        let pair_sum = independent
            .lowered_exports
            .iter()
            .find(|export| export.export_name == "pair_sum")
            .expect("pair_sum export should be present");
        assert_eq!(pair_sum.execution_manifest.expected_outputs, vec![5]);

        let aggregate = call_graph
            .lowered_exports
            .iter()
            .find(|export| export.export_name == "aggregate")
            .expect("aggregate export should be present");
        assert_eq!(aggregate.execution_manifest.expected_outputs, vec![52]);

        let plus_four = memory_graph
            .lowered_exports
            .iter()
            .find(|export| export.export_name == "pair_sum_plus_four")
            .expect("pair_sum_plus_four export should be present");
        assert_eq!(plus_four.execution_manifest.expected_outputs, vec![9]);
        assert_eq!(
            plus_four.execution_manifest.expected_final_memory,
            vec![2, 3]
        );
        Ok(())
    }

    #[test]
    fn module_specialization_refuses_import_boundary_calls_explicitly() {
        let profile = TassadarWasmProfile::core_i32_v2();
        let trace_abi = TassadarTraceAbi::core_i32_v2();
        let error = compile_tassadar_module_specialization_bundle(
            "seeded_import_bridge",
            "seeded_import_bridge_module",
            tassadar_seeded_module_specialization_import_boundary_module()
                .expect("seeded import module should build"),
            &profile,
            &trace_abi,
        )
        .expect_err("import-boundary specialization should refuse");
        assert!(matches!(
            error,
            TassadarModuleSpecializationError::UnsupportedImportedCall { .. }
        ));
    }
}
