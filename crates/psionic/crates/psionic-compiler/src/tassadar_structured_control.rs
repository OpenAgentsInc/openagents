use psionic_runtime::{
    TassadarCompilerToolchainIdentity, TassadarProgramSourceIdentity, TassadarProgramSourceKind,
    TassadarStructuredControlBinaryOp, TassadarStructuredControlError,
    TassadarStructuredControlExecution, TassadarStructuredControlHaltReason,
    TassadarStructuredControlInstruction, TassadarStructuredControlProgram,
    TassadarStructuredControlUnaryOp, execute_tassadar_structured_control_program,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use wasm_encoder::{
    BlockType, CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction,
    Module, TypeSection, ValType,
};
use wasmparser::{ExternalKind, Operator, Parser, Payload};

const TASSADAR_STRUCTURED_CONTROL_BUNDLE_SCHEMA_VERSION: u16 = 1;
const TASSADAR_STRUCTURED_CONTROL_COMPILER_FAMILY: &str = "tassadar_structured_control_lowering";
const TASSADAR_STRUCTURED_CONTROL_COMPILER_VERSION: &str = "v1";
const TASSADAR_STRUCTURED_CONTROL_CLAIM_BOUNDARY: &str = "bounded structured-control lowering compiles zero-parameter i32-only Wasm functions with empty block types into validated nested executor programs covering block, loop, if, else, br, br_if, and br_table; calls, imports, memories, tables, globals, block results, and arbitrary Wasm remain out of scope";

#[derive(Clone)]
struct ParsedFunctionSignature {
    params: Vec<wasmparser::ValType>,
    results: Vec<wasmparser::ValType>,
}

#[derive(Clone)]
struct ParsedFunctionBody {
    locals: Vec<wasmparser::ValType>,
    instructions: Vec<TassadarStructuredControlInstruction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredControlExecutionManifest {
    /// Stable export name.
    pub export_name: String,
    /// Stable function index.
    pub function_index: u32,
    /// Expected returned value from the CPU reference lane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_return_value: Option<i32>,
    /// Expected halt reason from the CPU reference lane.
    pub expected_halt_reason: TassadarStructuredControlHaltReason,
    /// Stable digest over the reference execution trace.
    pub expected_trace_digest: String,
    /// Ordered step count in the reference trace.
    pub expected_trace_step_count: usize,
    /// Expected final locals snapshot.
    pub expected_final_locals: Vec<i32>,
    /// Stable digest over the manifest.
    pub execution_digest: String,
}

impl TassadarStructuredControlExecutionManifest {
    fn new(
        export_name: impl Into<String>,
        function_index: u32,
        execution: &TassadarStructuredControlExecution,
    ) -> Self {
        let mut manifest = Self {
            export_name: export_name.into(),
            function_index,
            expected_return_value: execution.returned_value,
            expected_halt_reason: execution.halt_reason,
            expected_trace_digest: execution.execution_digest(),
            expected_trace_step_count: execution.steps.len(),
            expected_final_locals: execution.final_locals.clone(),
            execution_digest: String::new(),
        };
        manifest.execution_digest = stable_digest(
            b"tassadar_structured_control_execution_manifest|",
            &manifest,
        );
        manifest
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredControlArtifact {
    /// Stable export name.
    pub export_name: String,
    /// Stable function index.
    pub function_index: u32,
    /// Validated structured-control program.
    pub program: TassadarStructuredControlProgram,
    /// Digest-bound execution manifest from the CPU reference lane.
    pub execution_manifest: TassadarStructuredControlExecutionManifest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredControlBundle {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Coarse claim class.
    pub claim_class: String,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Shared source identity for all lowered exports.
    pub source_identity: TassadarProgramSourceIdentity,
    /// Shared lowering toolchain identity.
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    /// Ordered lowered exports.
    pub artifacts: Vec<TassadarStructuredControlArtifact>,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl TassadarStructuredControlBundle {
    fn new(
        source_identity: TassadarProgramSourceIdentity,
        toolchain_identity: TassadarCompilerToolchainIdentity,
        artifacts: Vec<TassadarStructuredControlArtifact>,
    ) -> Self {
        let source_digest_prefix = source_identity
            .source_digest
            .chars()
            .take(12)
            .collect::<String>();
        let mut bundle = Self {
            schema_version: TASSADAR_STRUCTURED_CONTROL_BUNDLE_SCHEMA_VERSION,
            bundle_id: format!("tassadar.structured_control.{source_digest_prefix}.bundle.v1"),
            claim_class: String::from("compiled_bounded_exactness"),
            claim_boundary: String::from(TASSADAR_STRUCTURED_CONTROL_CLAIM_BOUNDARY),
            source_identity,
            toolchain_identity,
            artifacts,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(b"tassadar_structured_control_bundle|", &bundle);
        bundle
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarStructuredControlBundleError {
    #[error("unsupported Wasm section `{section}` for the bounded structured-control lane")]
    UnsupportedSection { section: String },
    #[error("export `{export_name}` used unsupported kind `{kind}`")]
    UnsupportedExportKind { export_name: String, kind: String },
    #[error("structured-control module `{source_name}` exports no functions")]
    NoFunctionExports { source_name: String },
    #[error(
        "export `{export_name}` function {function_index} declares {param_count} params, but the bounded lane only admits zero-parameter functions"
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
        "export `{export_name}` function {function_index} uses unsupported block type for `{opcode}`"
    )]
    UnsupportedBlockType {
        export_name: String,
        function_index: u32,
        opcode: String,
    },
    #[error(
        "export `{export_name}` function {function_index} uses unsupported instruction `{opcode}`"
    )]
    UnsupportedInstruction {
        export_name: String,
        function_index: u32,
        opcode: String,
    },
    #[error(
        "export `{export_name}` function {function_index} has malformed structured control: {detail}"
    )]
    MalformedStructuredControl {
        export_name: String,
        function_index: u32,
        detail: String,
    },
    #[error("code/function section length mismatch: declared {declared}, found {actual}")]
    CodeBodyCountMismatch { declared: usize, actual: usize },
    #[error(transparent)]
    Runtime(#[from] TassadarStructuredControlError),
    #[error("failed to parse Wasm module: {0}")]
    Parse(String),
}

pub fn compile_tassadar_wasm_binary_module_to_structured_control_bundle(
    source_name: impl Into<String>,
    wasm_bytes: &[u8],
) -> Result<TassadarStructuredControlBundle, TassadarStructuredControlBundleError> {
    let source_name = source_name.into();
    let parsed = parse_structured_control_module(source_name.as_str(), wasm_bytes)?;
    if parsed.exports.is_empty() {
        return Err(TassadarStructuredControlBundleError::NoFunctionExports { source_name });
    }
    let source_identity = TassadarProgramSourceIdentity::new(
        TassadarProgramSourceKind::WasmBinary,
        source_name,
        stable_bytes_digest(wasm_bytes),
    );
    let toolchain_identity = TassadarCompilerToolchainIdentity::new(
        TASSADAR_STRUCTURED_CONTROL_COMPILER_FAMILY,
        TASSADAR_STRUCTURED_CONTROL_COMPILER_VERSION,
        "tassadar.structured_control.v1",
    )
    .with_pipeline_features(vec![
        String::from("wasm_binary_ingress"),
        String::from("structured_control"),
        String::from("branch_table"),
        String::from("zero_param_i32_only"),
    ]);

    let artifacts = parsed
        .exports
        .iter()
        .map(|export| {
            let signature = parsed
                .types
                .get(
                    *parsed
                        .function_type_indices
                        .get(export.index as usize)
                        .ok_or_else(|| {
                            TassadarStructuredControlBundleError::MalformedStructuredControl {
                                export_name: export.name.clone(),
                                function_index: export.index,
                                detail: String::from(
                                    "export referenced missing function type index",
                                ),
                            }
                        })? as usize,
                )
                .ok_or_else(|| {
                    TassadarStructuredControlBundleError::MalformedStructuredControl {
                        export_name: export.name.clone(),
                        function_index: export.index,
                        detail: String::from("export referenced missing signature"),
                    }
                })?;
            if !signature.params.is_empty() {
                return Err(
                    TassadarStructuredControlBundleError::UnsupportedParamCount {
                        export_name: export.name.clone(),
                        function_index: export.index,
                        param_count: signature.params.len(),
                    },
                );
            }
            if signature.results.len() > 1
                || signature
                    .results
                    .iter()
                    .any(|result| *result != wasmparser::ValType::I32)
            {
                return Err(
                    TassadarStructuredControlBundleError::UnsupportedResultTypes {
                        export_name: export.name.clone(),
                        function_index: export.index,
                        result_types: signature
                            .results
                            .iter()
                            .map(|result| format!("{result:?}"))
                            .collect(),
                    },
                );
            }
            let body = parsed.bodies.get(export.index as usize).ok_or_else(|| {
                TassadarStructuredControlBundleError::MalformedStructuredControl {
                    export_name: export.name.clone(),
                    function_index: export.index,
                    detail: String::from("export referenced missing function body"),
                }
            })?;
            for local in &body.locals {
                if *local != wasmparser::ValType::I32 {
                    return Err(TassadarStructuredControlBundleError::UnsupportedLocalType {
                        export_name: export.name.clone(),
                        function_index: export.index,
                        local_type: format!("{local:?}"),
                    });
                }
            }
            let program = TassadarStructuredControlProgram::new(
                format!(
                    "tassadar.structured_control.{}.artifact.v1",
                    sanitize_export_name(export.name.as_str())
                ),
                body.locals.len(),
                signature.results.len() as u8,
                body.instructions.clone(),
            );
            program.validate()?;
            let execution = execute_tassadar_structured_control_program(&program)?;
            Ok(TassadarStructuredControlArtifact {
                export_name: export.name.clone(),
                function_index: export.index,
                execution_manifest: TassadarStructuredControlExecutionManifest::new(
                    export.name.as_str(),
                    export.index,
                    &execution,
                ),
                program,
            })
        })
        .collect::<Result<Vec<_>, TassadarStructuredControlBundleError>>()?;

    Ok(TassadarStructuredControlBundle::new(
        source_identity,
        toolchain_identity,
        artifacts,
    ))
}

pub fn tassadar_seeded_structured_control_if_else_module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function(vec![], vec![ValType::I32]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("structured_if_else", ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new([(1, ValType::I32)]);
    function.instruction(&Instruction::I32Const(7));
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Const(5));
    function.instruction(&Instruction::I32LtS);
    function.instruction(&Instruction::If(BlockType::Empty));
    function.instruction(&Instruction::I32Const(11));
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::Else);
    function.instruction(&Instruction::I32Const(22));
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::End);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::Return);
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);
    module.finish()
}

pub fn tassadar_seeded_structured_control_loop_module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function(vec![], vec![ValType::I32]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("structured_loop", ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new([(1, ValType::I32)]);
    function.instruction(&Instruction::I32Const(4));
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::Block(BlockType::Empty));
    function.instruction(&Instruction::Loop(BlockType::Empty));
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Const(1));
    function.instruction(&Instruction::I32Sub);
    function.instruction(&Instruction::LocalTee(0));
    function.instruction(&Instruction::BrIf(0));
    function.instruction(&Instruction::End);
    function.instruction(&Instruction::End);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::Return);
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);
    module.finish()
}

pub fn tassadar_seeded_structured_control_branch_table_module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function(vec![], vec![ValType::I32]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(0);
    functions.function(0);
    functions.function(0);
    module.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("branch_table_inner", ExportKind::Func, 0);
    exports.export("branch_table_middle", ExportKind::Func, 1);
    exports.export("branch_table_outer", ExportKind::Func, 2);
    module.section(&exports);

    let mut code = CodeSection::new();
    for selector in [0_i32, 1_i32, 2_i32] {
        let mut function = Function::new([(1, ValType::I32)]);
        function.instruction(&Instruction::I32Const(300));
        function.instruction(&Instruction::LocalSet(0));
        function.instruction(&Instruction::Block(BlockType::Empty));
        function.instruction(&Instruction::Block(BlockType::Empty));
        function.instruction(&Instruction::Block(BlockType::Empty));
        function.instruction(&Instruction::I32Const(selector));
        function.instruction(&Instruction::BrTable(vec![0, 1].into(), 2));
        function.instruction(&Instruction::End);
        function.instruction(&Instruction::I32Const(100));
        function.instruction(&Instruction::LocalSet(0));
        function.instruction(&Instruction::Br(1));
        function.instruction(&Instruction::End);
        function.instruction(&Instruction::I32Const(200));
        function.instruction(&Instruction::LocalSet(0));
        function.instruction(&Instruction::Br(0));
        function.instruction(&Instruction::End);
        function.instruction(&Instruction::LocalGet(0));
        function.instruction(&Instruction::Return);
        function.instruction(&Instruction::End);
        code.function(&function);
    }
    module.section(&code);
    module.finish()
}

pub fn tassadar_seeded_structured_control_invalid_label_module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function(vec![], vec![ValType::I32]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("invalid_label_depth", ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new([]);
    function.instruction(&Instruction::Block(BlockType::Empty));
    function.instruction(&Instruction::Br(1));
    function.instruction(&Instruction::End);
    function.instruction(&Instruction::I32Const(0));
    function.instruction(&Instruction::Return);
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);
    module.finish()
}

pub fn tassadar_seeded_numeric_i32_core_arithmetic_module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function(vec![], vec![ValType::I32]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("i32_core_arithmetic_suite", ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new([(1, ValType::I32)]);
    function.instruction(&Instruction::I32Const(2));
    function.instruction(&Instruction::I32Const(3));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Const(10));
    function.instruction(&Instruction::I32Const(4));
    function.instruction(&Instruction::I32Sub);
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Const(3));
    function.instruction(&Instruction::I32Const(7));
    function.instruction(&Instruction::I32Mul);
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::Return);
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);
    module.finish()
}

pub fn tassadar_seeded_numeric_i32_comparison_module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function(vec![], vec![ValType::I32]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("i32_comparison_suite", ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new([(1, ValType::I32)]);
    function.instruction(&Instruction::I32Const(0));
    function.instruction(&Instruction::I32Eqz);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::I32Const(7));
    function.instruction(&Instruction::I32Const(7));
    function.instruction(&Instruction::I32Eq);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::I32Const(7));
    function.instruction(&Instruction::I32Const(8));
    function.instruction(&Instruction::I32Ne);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::I32Const(2));
    function.instruction(&Instruction::I32Const(7));
    function.instruction(&Instruction::I32LtS);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::I32Const(-1));
    function.instruction(&Instruction::I32Const(1));
    function.instruction(&Instruction::I32LtU);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::I32Const(9));
    function.instruction(&Instruction::I32Const(3));
    function.instruction(&Instruction::I32GtS);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::I32Const(-1));
    function.instruction(&Instruction::I32Const(1));
    function.instruction(&Instruction::I32GtU);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::I32Const(3));
    function.instruction(&Instruction::I32Const(3));
    function.instruction(&Instruction::I32LeS);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::I32Const(1));
    function.instruction(&Instruction::I32Const(-1));
    function.instruction(&Instruction::I32LeU);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::I32Const(4));
    function.instruction(&Instruction::I32Const(4));
    function.instruction(&Instruction::I32GeS);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));
    function.instruction(&Instruction::I32Const(-1));
    function.instruction(&Instruction::I32Const(1));
    function.instruction(&Instruction::I32GeU);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::Return);
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);
    module.finish()
}

pub fn tassadar_seeded_numeric_i32_bit_ops_module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function(vec![], vec![ValType::I32]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("i32_bit_ops_suite", ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new([(1, ValType::I32)]);
    function.instruction(&Instruction::I32Const(0));
    function.instruction(&Instruction::LocalSet(0));

    function.instruction(&Instruction::I32Const(6));
    function.instruction(&Instruction::I32Const(3));
    function.instruction(&Instruction::I32And);
    function.instruction(&Instruction::I32Const(2));
    function.instruction(&Instruction::I32Eq);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));

    function.instruction(&Instruction::I32Const(6));
    function.instruction(&Instruction::I32Const(3));
    function.instruction(&Instruction::I32Or);
    function.instruction(&Instruction::I32Const(7));
    function.instruction(&Instruction::I32Eq);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));

    function.instruction(&Instruction::I32Const(6));
    function.instruction(&Instruction::I32Const(3));
    function.instruction(&Instruction::I32Xor);
    function.instruction(&Instruction::I32Const(5));
    function.instruction(&Instruction::I32Eq);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));

    function.instruction(&Instruction::I32Const(1));
    function.instruction(&Instruction::I32Const(3));
    function.instruction(&Instruction::I32Shl);
    function.instruction(&Instruction::I32Const(8));
    function.instruction(&Instruction::I32Eq);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));

    function.instruction(&Instruction::I32Const(-8));
    function.instruction(&Instruction::I32Const(1));
    function.instruction(&Instruction::I32ShrS);
    function.instruction(&Instruction::I32Const(-4));
    function.instruction(&Instruction::I32Eq);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::LocalSet(0));

    function.instruction(&Instruction::I32Const(-8));
    function.instruction(&Instruction::I32Const(1));
    function.instruction(&Instruction::I32ShrU);
    function.instruction(&Instruction::I32Const(0x7fff_fffc_u32 as i32));
    function.instruction(&Instruction::I32Eq);
    function.instruction(&Instruction::LocalGet(0));
    function.instruction(&Instruction::I32Add);
    function.instruction(&Instruction::Return);
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);
    module.finish()
}

pub fn tassadar_seeded_numeric_i64_refusal_module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function(vec![], vec![ValType::I64]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("i64_refusal", ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new([]);
    function.instruction(&Instruction::I64Const(1));
    function.instruction(&Instruction::Return);
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);
    module.finish()
}

pub fn tassadar_seeded_numeric_float_refusal_module() -> Vec<u8> {
    let mut module = Module::new();
    let mut types = TypeSection::new();
    types.ty().function(vec![], vec![ValType::F32]);
    module.section(&types);

    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("float_refusal", ExportKind::Func, 0);
    module.section(&exports);

    let mut code = CodeSection::new();
    let mut function = Function::new([]);
    function.instruction(&Instruction::F32Const(1.5f32.into()));
    function.instruction(&Instruction::Return);
    function.instruction(&Instruction::End);
    code.function(&function);
    module.section(&code);
    module.finish()
}

struct ParsedStructuredControlModule {
    types: Vec<ParsedFunctionSignature>,
    function_type_indices: Vec<u32>,
    bodies: Vec<ParsedFunctionBody>,
    exports: Vec<ParsedFunctionExport>,
}

#[derive(Clone)]
struct ParsedFunctionExport {
    name: String,
    index: u32,
}

fn parse_structured_control_module(
    source_name: &str,
    wasm_bytes: &[u8],
) -> Result<ParsedStructuredControlModule, TassadarStructuredControlBundleError> {
    let mut types = Vec::new();
    let mut function_type_indices = Vec::new();
    let mut bodies = Vec::new();
    let mut exports = Vec::new();

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload
            .map_err(|error| TassadarStructuredControlBundleError::Parse(error.to_string()))?;
        match payload {
            Payload::Version { encoding, .. } => {
                if encoding != wasmparser::Encoding::Module {
                    return Err(TassadarStructuredControlBundleError::Parse(String::from(
                        "component-model payloads are unsupported",
                    )));
                }
            }
            Payload::TypeSection(reader) => {
                for function_type in reader.into_iter_err_on_gc_types() {
                    let function_type = function_type.map_err(|error| {
                        TassadarStructuredControlBundleError::Parse(error.to_string())
                    })?;
                    types.push(ParsedFunctionSignature {
                        params: function_type.params().to_vec(),
                        results: function_type.results().to_vec(),
                    });
                }
            }
            Payload::FunctionSection(reader) => {
                for type_index in reader {
                    function_type_indices.push(type_index.map_err(|error| {
                        TassadarStructuredControlBundleError::Parse(error.to_string())
                    })?);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(|error| {
                        TassadarStructuredControlBundleError::Parse(error.to_string())
                    })?;
                    if export.kind != ExternalKind::Func {
                        return Err(
                            TassadarStructuredControlBundleError::UnsupportedExportKind {
                                export_name: export.name.to_string(),
                                kind: format!("{:?}", export.kind),
                            },
                        );
                    }
                    exports.push(ParsedFunctionExport {
                        name: export.name.to_string(),
                        index: export.index,
                    });
                }
            }
            Payload::CodeSectionEntry(body) => {
                bodies.push(parse_function_body(source_name, bodies.len() as u32, body)?);
            }
            payload @ (Payload::ImportSection(_)
            | Payload::MemorySection(_)
            | Payload::TableSection(_)
            | Payload::GlobalSection(_)
            | Payload::TagSection(_)
            | Payload::ElementSection(_)
            | Payload::DataSection(_)
            | Payload::StartSection { .. }) => {
                return Err(TassadarStructuredControlBundleError::UnsupportedSection {
                    section: payload_section_name(&payload),
                });
            }
            Payload::CustomSection(_)
            | Payload::End(_)
            | Payload::CodeSectionStart { .. }
            | Payload::DataCountSection { .. }
            | Payload::UnknownSection { .. } => {}
            other => {
                return Err(TassadarStructuredControlBundleError::UnsupportedSection {
                    section: payload_section_name(&other),
                });
            }
        }
    }

    if function_type_indices.len() != bodies.len() {
        return Err(
            TassadarStructuredControlBundleError::CodeBodyCountMismatch {
                declared: function_type_indices.len(),
                actual: bodies.len(),
            },
        );
    }
    Ok(ParsedStructuredControlModule {
        types,
        function_type_indices,
        bodies,
        exports,
    })
}

fn parse_function_body(
    source_name: &str,
    function_index: u32,
    body: wasmparser::FunctionBody<'_>,
) -> Result<ParsedFunctionBody, TassadarStructuredControlBundleError> {
    let mut locals = Vec::new();
    let locals_reader = body
        .get_locals_reader()
        .map_err(|error| TassadarStructuredControlBundleError::Parse(error.to_string()))?;
    for local in locals_reader {
        let (count, ty) = local
            .map_err(|error| TassadarStructuredControlBundleError::Parse(error.to_string()))?;
        locals.extend(std::iter::repeat_n(ty, count as usize));
    }

    let mut frame_stack = vec![ControlFrame::Root {
        instructions: Vec::new(),
    }];
    let mut next_label_id = 0u32;
    let mut saw_root_end = false;

    let mut reader = body
        .get_operators_reader()
        .map_err(|error| TassadarStructuredControlBundleError::Parse(error.to_string()))?;
    while !reader.eof() {
        let operator = reader
            .read()
            .map_err(|error| TassadarStructuredControlBundleError::Parse(error.to_string()))?;
        match operator {
            Operator::I32Const { value } => frame_stack
                .last_mut()
                .expect("root frame")
                .push(TassadarStructuredControlInstruction::I32Const { value }),
            Operator::LocalGet { local_index } => frame_stack
                .last_mut()
                .expect("root frame")
                .push(TassadarStructuredControlInstruction::LocalGet { local_index }),
            Operator::LocalSet { local_index } => frame_stack
                .last_mut()
                .expect("root frame")
                .push(TassadarStructuredControlInstruction::LocalSet { local_index }),
            Operator::LocalTee { local_index } => frame_stack
                .last_mut()
                .expect("root frame")
                .push(TassadarStructuredControlInstruction::LocalTee { local_index }),
            Operator::I32Add => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
            ),
            Operator::I32Sub => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Sub,
                },
            ),
            Operator::I32Mul => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Mul,
                },
            ),
            Operator::I32Eqz => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::UnaryOp {
                    op: TassadarStructuredControlUnaryOp::Eqz,
                },
            ),
            Operator::I32Eq => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Eq,
                },
            ),
            Operator::I32Ne => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Ne,
                },
            ),
            Operator::I32LtS => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::LtS,
                },
            ),
            Operator::I32LtU => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::LtU,
                },
            ),
            Operator::I32GtS => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::GtS,
                },
            ),
            Operator::I32GtU => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::GtU,
                },
            ),
            Operator::I32LeS => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::LeS,
                },
            ),
            Operator::I32LeU => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::LeU,
                },
            ),
            Operator::I32GeS => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::GeS,
                },
            ),
            Operator::I32GeU => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::GeU,
                },
            ),
            Operator::I32And => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::And,
                },
            ),
            Operator::I32Or => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Or,
                },
            ),
            Operator::I32Xor => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Xor,
                },
            ),
            Operator::I32Shl => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Shl,
                },
            ),
            Operator::I32ShrS => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::ShrS,
                },
            ),
            Operator::I32ShrU => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::ShrU,
                },
            ),
            Operator::Drop => frame_stack
                .last_mut()
                .expect("root frame")
                .push(TassadarStructuredControlInstruction::Drop),
            Operator::Block { blockty } => {
                require_empty_block_type(source_name, function_index, "block", blockty)?;
                frame_stack.push(ControlFrame::Block {
                    label_id: next_label(next_label_id, "block"),
                    instructions: Vec::new(),
                });
                next_label_id = next_label_id.saturating_add(1);
            }
            Operator::Loop { blockty } => {
                require_empty_block_type(source_name, function_index, "loop", blockty)?;
                frame_stack.push(ControlFrame::Loop {
                    label_id: next_label(next_label_id, "loop"),
                    instructions: Vec::new(),
                });
                next_label_id = next_label_id.saturating_add(1);
            }
            Operator::If { blockty } => {
                require_empty_block_type(source_name, function_index, "if", blockty)?;
                frame_stack.push(ControlFrame::If {
                    label_id: next_label(next_label_id, "if"),
                    then_instructions: Vec::new(),
                    else_instructions: Vec::new(),
                    in_else: false,
                });
                next_label_id = next_label_id.saturating_add(1);
            }
            Operator::Else => match frame_stack.last_mut() {
                Some(ControlFrame::If { in_else, .. }) if !*in_else => {
                    *in_else = true;
                }
                _ => {
                    return Err(
                        TassadarStructuredControlBundleError::MalformedStructuredControl {
                            export_name: source_name.to_string(),
                            function_index,
                            detail: String::from("encountered `else` outside one open `if` frame"),
                        },
                    );
                }
            },
            Operator::End => {
                let frame = frame_stack.pop().ok_or_else(|| {
                    TassadarStructuredControlBundleError::MalformedStructuredControl {
                        export_name: source_name.to_string(),
                        function_index,
                        detail: String::from("encountered `end` with no open frame"),
                    }
                })?;
                match frame {
                    ControlFrame::Root { instructions } => {
                        saw_root_end = true;
                        frame_stack.push(ControlFrame::Root { instructions });
                        break;
                    }
                    other => {
                        let instruction = other.finish_instruction();
                        frame_stack
                            .last_mut()
                            .expect("parent frame")
                            .push(instruction);
                    }
                }
            }
            Operator::Br { relative_depth } => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::Br {
                    depth: relative_depth,
                },
            ),
            Operator::BrIf { relative_depth } => frame_stack.last_mut().expect("root frame").push(
                TassadarStructuredControlInstruction::BrIf {
                    depth: relative_depth,
                },
            ),
            Operator::BrTable { targets } => {
                let target_depths =
                    targets
                        .targets()
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|error| {
                            TassadarStructuredControlBundleError::Parse(error.to_string())
                        })?;
                frame_stack.last_mut().expect("root frame").push(
                    TassadarStructuredControlInstruction::BrTable {
                        target_depths,
                        default_depth: targets.default(),
                    },
                );
            }
            Operator::Return => frame_stack
                .last_mut()
                .expect("root frame")
                .push(TassadarStructuredControlInstruction::Return),
            other => {
                return Err(
                    TassadarStructuredControlBundleError::UnsupportedInstruction {
                        export_name: source_name.to_string(),
                        function_index,
                        opcode: format!("{other:?}"),
                    },
                );
            }
        }
    }

    if !saw_root_end {
        return Err(
            TassadarStructuredControlBundleError::MalformedStructuredControl {
                export_name: source_name.to_string(),
                function_index,
                detail: String::from("function body ended without a root `end`"),
            },
        );
    }
    if frame_stack.len() != 1 {
        return Err(
            TassadarStructuredControlBundleError::MalformedStructuredControl {
                export_name: source_name.to_string(),
                function_index,
                detail: String::from("function body left unclosed structured-control frames"),
            },
        );
    }
    let ControlFrame::Root { instructions } = frame_stack.pop().expect("root frame") else {
        unreachable!("frame stack should end at root")
    };
    Ok(ParsedFunctionBody {
        locals,
        instructions,
    })
}

enum ControlFrame {
    Root {
        instructions: Vec<TassadarStructuredControlInstruction>,
    },
    Block {
        label_id: String,
        instructions: Vec<TassadarStructuredControlInstruction>,
    },
    Loop {
        label_id: String,
        instructions: Vec<TassadarStructuredControlInstruction>,
    },
    If {
        label_id: String,
        then_instructions: Vec<TassadarStructuredControlInstruction>,
        else_instructions: Vec<TassadarStructuredControlInstruction>,
        in_else: bool,
    },
}

impl ControlFrame {
    fn push(&mut self, instruction: TassadarStructuredControlInstruction) {
        match self {
            Self::Root { instructions }
            | Self::Block { instructions, .. }
            | Self::Loop { instructions, .. } => instructions.push(instruction),
            Self::If {
                then_instructions,
                else_instructions,
                in_else,
                ..
            } => {
                if *in_else {
                    else_instructions.push(instruction);
                } else {
                    then_instructions.push(instruction);
                }
            }
        }
    }

    fn finish_instruction(self) -> TassadarStructuredControlInstruction {
        match self {
            Self::Root { .. } => unreachable!("root frame does not lower into one instruction"),
            Self::Block {
                label_id,
                instructions,
            } => TassadarStructuredControlInstruction::Block {
                label_id,
                instructions,
            },
            Self::Loop {
                label_id,
                instructions,
            } => TassadarStructuredControlInstruction::Loop {
                label_id,
                instructions,
            },
            Self::If {
                label_id,
                then_instructions,
                else_instructions,
                ..
            } => TassadarStructuredControlInstruction::If {
                label_id,
                then_instructions,
                else_instructions,
            },
        }
    }
}

fn require_empty_block_type(
    export_name: &str,
    function_index: u32,
    opcode: &str,
    block_type: wasmparser::BlockType,
) -> Result<(), TassadarStructuredControlBundleError> {
    if !matches!(block_type, wasmparser::BlockType::Empty) {
        return Err(TassadarStructuredControlBundleError::UnsupportedBlockType {
            export_name: export_name.to_string(),
            function_index,
            opcode: String::from(opcode),
        });
    }
    Ok(())
}

fn payload_section_name(payload: &Payload<'_>) -> String {
    match payload {
        Payload::TypeSection(_) => String::from("type"),
        Payload::ImportSection(_) => String::from("import"),
        Payload::FunctionSection(_) => String::from("function"),
        Payload::TableSection(_) => String::from("table"),
        Payload::MemorySection(_) => String::from("memory"),
        Payload::TagSection(_) => String::from("tag"),
        Payload::GlobalSection(_) => String::from("global"),
        Payload::ExportSection(_) => String::from("export"),
        Payload::StartSection { .. } => String::from("start"),
        Payload::ElementSection(_) => String::from("element"),
        Payload::CodeSectionStart { .. } => String::from("code_start"),
        Payload::CodeSectionEntry(_) => String::from("code"),
        Payload::DataSection(_) => String::from("data"),
        Payload::DataCountSection { .. } => String::from("data_count"),
        Payload::CustomSection(_) => String::from("custom"),
        Payload::Version { .. } => String::from("version"),
        Payload::End(_) => String::from("end"),
        Payload::UnknownSection { .. } => String::from("unknown"),
        _ => String::from("other"),
    }
}

fn sanitize_export_name(export_name: &str) -> String {
    let mut output = String::with_capacity(export_name.len());
    for character in export_name.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else {
            output.push('_');
        }
    }
    output
}

fn next_label(label_index: u32, kind: &str) -> String {
    format!("{kind}_{label_index}")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn stable_bytes_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tassadar_structured_control_source|");
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_runtime::execute_tassadar_structured_control_program;

    use super::{
        TassadarStructuredControlBundleError,
        compile_tassadar_wasm_binary_module_to_structured_control_bundle,
        tassadar_seeded_numeric_float_refusal_module, tassadar_seeded_numeric_i32_bit_ops_module,
        tassadar_seeded_numeric_i32_comparison_module,
        tassadar_seeded_numeric_i32_core_arithmetic_module,
        tassadar_seeded_numeric_i64_refusal_module,
        tassadar_seeded_structured_control_branch_table_module,
        tassadar_seeded_structured_control_if_else_module,
        tassadar_seeded_structured_control_invalid_label_module,
        tassadar_seeded_structured_control_loop_module,
    };

    #[test]
    fn structured_control_if_else_bundle_matches_cpu_reference_truth() {
        let bundle = compile_tassadar_wasm_binary_module_to_structured_control_bundle(
            "if_else",
            &tassadar_seeded_structured_control_if_else_module(),
        )
        .expect("bundle");
        assert_eq!(bundle.artifacts.len(), 1);
        let artifact = &bundle.artifacts[0];
        let execution =
            execute_tassadar_structured_control_program(&artifact.program).expect("execute");
        assert_eq!(
            execution.returned_value,
            artifact.execution_manifest.expected_return_value
        );
        assert_eq!(
            execution.execution_digest(),
            artifact.execution_manifest.expected_trace_digest
        );
    }

    #[test]
    fn structured_control_loop_bundle_matches_cpu_reference_truth() {
        let bundle = compile_tassadar_wasm_binary_module_to_structured_control_bundle(
            "loop",
            &tassadar_seeded_structured_control_loop_module(),
        )
        .expect("bundle");
        let artifact = &bundle.artifacts[0];
        let execution =
            execute_tassadar_structured_control_program(&artifact.program).expect("execute");
        assert_eq!(execution.returned_value, Some(0));
        assert_eq!(
            execution.execution_digest(),
            artifact.execution_manifest.expected_trace_digest
        );
    }

    #[test]
    fn structured_control_branch_table_bundle_matches_cpu_reference_truth() {
        let bundle = compile_tassadar_wasm_binary_module_to_structured_control_bundle(
            "branch_table",
            &tassadar_seeded_structured_control_branch_table_module(),
        )
        .expect("bundle");
        let expected = vec![
            ("branch_table_inner", Some(100)),
            ("branch_table_middle", Some(200)),
            ("branch_table_outer", Some(300)),
        ];
        let actual = bundle
            .artifacts
            .iter()
            .map(|artifact| {
                (
                    artifact.export_name.as_str(),
                    artifact.execution_manifest.expected_return_value,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    #[test]
    fn structured_control_malformed_label_module_refuses_explicitly() {
        let error = compile_tassadar_wasm_binary_module_to_structured_control_bundle(
            "invalid_label",
            &tassadar_seeded_structured_control_invalid_label_module(),
        )
        .expect_err("should refuse");
        assert!(matches!(
            error,
            TassadarStructuredControlBundleError::Runtime(
                psionic_runtime::TassadarStructuredControlError::InvalidBranchDepth { .. }
            )
        ));
    }

    #[test]
    fn structured_control_numeric_i32_comparison_bundle_matches_cpu_reference_truth() {
        let bundle = compile_tassadar_wasm_binary_module_to_structured_control_bundle(
            "numeric_i32_comparison",
            &tassadar_seeded_numeric_i32_comparison_module(),
        )
        .expect("bundle");
        assert_eq!(bundle.artifacts.len(), 1);
        let artifact = &bundle.artifacts[0];
        let execution =
            execute_tassadar_structured_control_program(&artifact.program).expect("execute");
        assert_eq!(execution.returned_value, Some(10));
        assert_eq!(
            execution.execution_digest(),
            artifact.execution_manifest.expected_trace_digest
        );
    }

    #[test]
    fn structured_control_numeric_i32_core_arithmetic_bundle_matches_cpu_reference_truth() {
        let bundle = compile_tassadar_wasm_binary_module_to_structured_control_bundle(
            "numeric_i32_core_arithmetic",
            &tassadar_seeded_numeric_i32_core_arithmetic_module(),
        )
        .expect("bundle");
        assert_eq!(bundle.artifacts.len(), 1);
        let artifact = &bundle.artifacts[0];
        let execution =
            execute_tassadar_structured_control_program(&artifact.program).expect("execute");
        assert_eq!(execution.returned_value, Some(32));
        assert_eq!(
            execution.execution_digest(),
            artifact.execution_manifest.expected_trace_digest
        );
    }

    #[test]
    fn structured_control_numeric_i32_bit_ops_bundle_matches_cpu_reference_truth() {
        let bundle = compile_tassadar_wasm_binary_module_to_structured_control_bundle(
            "numeric_i32_bit_ops",
            &tassadar_seeded_numeric_i32_bit_ops_module(),
        )
        .expect("bundle");
        assert_eq!(bundle.artifacts.len(), 1);
        let artifact = &bundle.artifacts[0];
        let execution =
            execute_tassadar_structured_control_program(&artifact.program).expect("execute");
        assert_eq!(execution.returned_value, Some(6));
        assert_eq!(
            execution.execution_digest(),
            artifact.execution_manifest.expected_trace_digest
        );
    }

    #[test]
    fn structured_control_numeric_i64_module_refuses_explicitly() {
        let error = compile_tassadar_wasm_binary_module_to_structured_control_bundle(
            "numeric_i64_refusal",
            &tassadar_seeded_numeric_i64_refusal_module(),
        )
        .expect_err("i64 should refuse");
        assert!(matches!(
            error,
            TassadarStructuredControlBundleError::UnsupportedResultTypes { .. }
                | TassadarStructuredControlBundleError::UnsupportedInstruction { .. }
        ));
    }

    #[test]
    fn structured_control_numeric_float_module_refuses_explicitly() {
        let error = compile_tassadar_wasm_binary_module_to_structured_control_bundle(
            "numeric_float_refusal",
            &tassadar_seeded_numeric_float_refusal_module(),
        )
        .expect_err("float should refuse");
        assert!(matches!(
            error,
            TassadarStructuredControlBundleError::UnsupportedResultTypes { .. }
                | TassadarStructuredControlBundleError::UnsupportedInstruction { .. }
        ));
    }
}
