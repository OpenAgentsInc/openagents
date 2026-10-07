use std::{convert::TryFrom, path::Path};

use psionic_ir::{
    TassadarFrozenCoreWasmWindow, TassadarNormalizedWasmConstExpr, TassadarNormalizedWasmDataMode,
    TassadarNormalizedWasmGlobalMutability, TassadarNormalizedWasmInstruction,
    TassadarNormalizedWasmModule, TassadarNormalizedWasmModuleError,
    TassadarNormalizedWasmTableElementKind, TassadarNormalizedWasmValueType,
    encode_tassadar_normalized_wasm_module, parse_tassadar_normalized_wasm_module,
    tassadar_frozen_core_wasm_window_v1,
};
use psionic_runtime::{
    TassadarCToWasmCompileConfig, TassadarCToWasmCompileReceipt, TassadarCompileRefusal,
    TassadarCompilerToolchainIdentity, TassadarCpuReferenceRunner, TassadarExecutionRefusal,
    TassadarInstruction, TassadarModuleDataSegment, TassadarModuleElementSegment,
    TassadarModuleExecutionProgram, TassadarModuleFunction, TassadarModuleGlobal,
    TassadarModuleGlobalMutability, TassadarModuleInstruction, TassadarModuleMemory,
    TassadarModuleTable, TassadarModuleTableElementKind, TassadarModuleValueType, TassadarProgram,
    TassadarProgramArtifact, TassadarProgramArtifactError, TassadarProgramSourceIdentity,
    TassadarProgramSourceKind, TassadarRustToWasmCompileConfig, TassadarRustToWasmCompileReceipt,
    TassadarTraceAbi, TassadarWasmProfile, compile_tassadar_c_source_to_wasm_receipt,
    compile_tassadar_rust_source_to_wasm_receipt, summarize_tassadar_wasm_binary,
    tassadar_trace_abi_for_profile_id,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_WASM_MODULE_ARTIFACT_BUNDLE_SCHEMA_VERSION: u16 = 1;
const TASSADAR_WASM_MODULE_COMPILER_FAMILY: &str = "tassadar_wasm_module_lowering";
const TASSADAR_WASM_MODULE_COMPILER_VERSION: &str = "v1";
const TASSADAR_WASM_TEXT_COMPILER_FAMILY: &str = "tassadar_wasm_text_parse";
const TASSADAR_WASM_TEXT_COMPILER_VERSION: &str = "v1";
const TASSADAR_WASM_MODULE_BUNDLE_CLAIM_BOUNDARY: &str = "bounded normalized Wasm module lowering compiles exported zero-parameter functions from the current straight-line core module slice into runnable Tassadar program artifacts; calls, structured control flow, dynamic memory addresses, multi-memory, byte-addressed memory ABI closure, and arbitrary Wasm remain out of scope";

/// Seeded validation case for the frozen core-Wasm harness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFrozenCoreWasmValidationFixture {
    /// Stable case id.
    pub case_id: String,
    /// Stable source ref or label.
    pub source_ref: String,
    /// Expected refused proposal family.
    pub proposal_family_id: String,
    /// Compiled Wasm payload.
    pub wasm_binary: Vec<u8>,
}

/// Returns the shared frozen core-Wasm window declaration imported by compiler-owned surfaces.
#[must_use]
pub fn tassadar_compiler_frozen_core_wasm_window() -> TassadarFrozenCoreWasmWindow {
    tassadar_frozen_core_wasm_window_v1()
}

/// Returns the canonical text fixture refs for the frozen core-Wasm harness.
#[must_use]
pub fn tassadar_frozen_core_wasm_text_fixture_refs() -> Vec<String> {
    vec![
        String::from("fixtures/tassadar/sources/tassadar_memory_lookup_kernel.wat"),
        String::from("fixtures/tassadar/sources/tassadar_multi_export_kernel.wat"),
        String::from("fixtures/tassadar/sources/tassadar_param_abi_kernel.wat"),
    ]
}

/// Returns the canonical binary fixture refs for the frozen core-Wasm harness.
#[must_use]
pub fn tassadar_frozen_core_wasm_binary_fixture_refs() -> Vec<String> {
    vec![
        String::from("fixtures/tassadar/wasm/tassadar_memory_lookup_kernel.wasm"),
        String::from("fixtures/tassadar/wasm/tassadar_multi_export_kernel.wasm"),
        String::from("fixtures/tassadar/wasm/tassadar_param_abi_kernel.wasm"),
    ]
}

/// Returns deterministic negative validation fixtures for out-of-window proposal families.
#[must_use]
pub fn tassadar_frozen_core_wasm_negative_validation_fixtures()
-> Vec<TassadarFrozenCoreWasmValidationFixture> {
    vec![
        TassadarFrozenCoreWasmValidationFixture {
            case_id: String::from("proposal.float_value_type_refused"),
            source_ref: String::from("fixtures/tassadar/sources/tassadar_float_kernel.wat"),
            proposal_family_id: String::from("floating_point"),
            wasm_binary: wat::parse_str(include_str!(
                "../../../fixtures/tassadar/sources/tassadar_float_kernel.wat"
            ))
            .expect("seeded float fixture should encode"),
        },
        TassadarFrozenCoreWasmValidationFixture {
            case_id: String::from("proposal.multi_memory_refused"),
            source_ref: String::from("fixtures/tassadar/sources/tassadar_multi_memory_kernel.wat"),
            proposal_family_id: String::from("multi_memory"),
            wasm_binary: wat::parse_str(include_str!(
                "../../../fixtures/tassadar/sources/tassadar_multi_memory_kernel.wat"
            ))
            .expect("seeded multi-memory fixture should encode"),
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum PendingValue {
    Const(i32),
    Local(u32),
    StackValue,
}

/// Exact execution manifest paired with one lowered module export artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmModuleExportExecutionManifest {
    /// Stable export name.
    pub export_name: String,
    /// Stable function index.
    pub function_index: u32,
    /// Expected emitted outputs on the CPU reference lane.
    pub expected_outputs: Vec<i32>,
    /// Expected final memory image on the CPU reference lane.
    pub expected_final_memory: Vec<i32>,
    /// Stable digest over the manifest.
    pub execution_digest: String,
}

impl TassadarWasmModuleExportExecutionManifest {
    fn new(
        export_name: impl Into<String>,
        function_index: u32,
        expected_outputs: Vec<i32>,
        expected_final_memory: Vec<i32>,
    ) -> Self {
        let mut manifest = Self {
            export_name: export_name.into(),
            function_index,
            expected_outputs,
            expected_final_memory,
            execution_digest: String::new(),
        };
        manifest.execution_digest = stable_digest(
            b"tassadar_wasm_module_export_execution_manifest|",
            &manifest,
        );
        manifest
    }
}

/// One lowered export artifact from the normalized Wasm module lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmModuleExportArtifact {
    /// Stable export name.
    pub export_name: String,
    /// Stable function index.
    pub function_index: u32,
    /// Runnable runtime-facing artifact.
    pub program_artifact: TassadarProgramArtifact,
    /// Digest-bound expected execution manifest.
    pub execution_manifest: TassadarWasmModuleExportExecutionManifest,
}

/// Digest-bound artifact bundle produced from one normalized Wasm module.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmModuleArtifactBundle {
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
    /// Normalized module IR bound into the bundle.
    pub normalized_module: TassadarNormalizedWasmModule,
    /// Ordered lowered function exports.
    pub lowered_exports: Vec<TassadarWasmModuleExportArtifact>,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl TassadarWasmModuleArtifactBundle {
    fn new(
        bundle_id: impl Into<String>,
        source_identity: TassadarProgramSourceIdentity,
        toolchain_identity: TassadarCompilerToolchainIdentity,
        normalized_module: TassadarNormalizedWasmModule,
        lowered_exports: Vec<TassadarWasmModuleExportArtifact>,
    ) -> Self {
        let mut bundle = Self {
            schema_version: TASSADAR_WASM_MODULE_ARTIFACT_BUNDLE_SCHEMA_VERSION,
            bundle_id: bundle_id.into(),
            claim_class: String::from("execution truth / compiled bounded exactness"),
            claim_boundary: String::from(TASSADAR_WASM_MODULE_BUNDLE_CLAIM_BOUNDARY),
            source_identity,
            toolchain_identity,
            normalized_module,
            lowered_exports,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(b"tassadar_wasm_module_artifact_bundle|", &bundle);
        bundle
    }
}

/// Full C-source to Wasm-module to Tassadar-artifact bundle pipeline result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCSourceArtifactBundlePipeline {
    /// Machine-readable compile receipt for the source-to-Wasm step.
    pub compile_receipt: TassadarCToWasmCompileReceipt,
    /// Digest-bound Wasm-module lowering result.
    pub artifact_bundle: TassadarWasmModuleArtifactBundle,
}

/// Full Rust-source to Wasm-module to Tassadar-artifact bundle pipeline result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustSourceArtifactBundlePipeline {
    /// Machine-readable compile receipt for the source-to-Wasm step.
    pub compile_receipt: TassadarRustToWasmCompileReceipt,
    /// Digest-bound Wasm-module lowering result.
    pub artifact_bundle: TassadarWasmModuleArtifactBundle,
}

/// Explicit compile configuration for one Wasm-text source fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmTextCompileConfig {
    /// Exported symbols that must survive parse and binary encoding.
    pub export_symbols: Vec<String>,
}

impl TassadarWasmTextCompileConfig {
    /// Returns the canonical config for the exact multi-export Wasm-text fixture.
    #[must_use]
    pub fn canonical_multi_export_kernel() -> Self {
        Self::new(["pair_sum", "local_double"])
    }

    /// Returns the canonical config for the exact memory-lookup Wasm-text fixture.
    #[must_use]
    pub fn canonical_memory_lookup_kernel() -> Self {
        Self::new(["load_middle", "load_edge_sum"])
    }

    /// Returns the canonical config for the parameter-ABI Wasm-text fixture.
    #[must_use]
    pub fn canonical_param_abi_kernel() -> Self {
        Self::new(["add_one"])
    }

    fn new(export_symbols: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            export_symbols: export_symbols.into_iter().map(String::from).collect(),
        }
    }

    /// Returns a stable digest over the compile configuration.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"tassadar_wasm_text_compile_config|", self)
    }

    /// Returns the stable feature list declared by the compile configuration.
    #[must_use]
    pub fn pipeline_features(&self) -> Vec<String> {
        let mut features = vec![
            String::from("compiler:wat"),
            String::from("syntax:wat"),
            String::from("target:wasm32-unknown-unknown"),
        ];
        for export in &self.export_symbols {
            features.push(format!("export:{export}"));
        }
        features.sort();
        features.dedup();
        features
    }
}

/// Outcome of one Wasm-text to Wasm compile attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarWasmTextCompileOutcome {
    /// The source parsed successfully and produced one Wasm binary.
    Succeeded {
        /// Repo-relative Wasm binary ref.
        wasm_binary_ref: String,
        /// Stable digest over the encoded Wasm binary.
        wasm_binary_digest: String,
        /// Structural summary over the encoded Wasm binary.
        wasm_binary_summary: psionic_runtime::TassadarWasmBinarySummary,
    },
    /// The source refused with a typed machine-readable reason.
    Refused {
        /// Typed refusal record.
        refusal: TassadarCompileRefusal,
    },
}

/// Machine-readable receipt for one Wasm-text to Wasm compile attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmTextCompileReceipt {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable source-identity facts.
    pub source_identity: TassadarProgramSourceIdentity,
    /// Stable compiler/toolchain identity.
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    /// Stable compile configuration.
    pub compile_config: TassadarWasmTextCompileConfig,
    /// Successful output or typed refusal.
    pub outcome: TassadarWasmTextCompileOutcome,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the full receipt.
    pub receipt_digest: String,
}

impl TassadarWasmTextCompileReceipt {
    fn new(
        source_identity: TassadarProgramSourceIdentity,
        toolchain_identity: TassadarCompilerToolchainIdentity,
        compile_config: TassadarWasmTextCompileConfig,
        outcome: TassadarWasmTextCompileOutcome,
    ) -> Self {
        let mut receipt = Self {
            schema_version: 1,
            source_identity,
            toolchain_identity,
            compile_config,
            outcome,
            claim_boundary: String::from(
                "Wasm-text source to Wasm receipt only; proves explicit source/toolchain/config/output digests for the current bounded Wasm-text matrix fixtures and typed refusal on invalid text input, not arbitrary frontend closure or arbitrary Wasm lowering",
            ),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = stable_digest(b"tassadar_wasm_text_compile_receipt|", &receipt);
        receipt
    }

    /// Returns whether the compile succeeded.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        matches!(
            self.outcome,
            TassadarWasmTextCompileOutcome::Succeeded { .. }
        )
    }

    /// Returns the compiled Wasm ref when present.
    #[must_use]
    pub fn wasm_binary_ref(&self) -> Option<&str> {
        match &self.outcome {
            TassadarWasmTextCompileOutcome::Succeeded {
                wasm_binary_ref, ..
            } => Some(wasm_binary_ref.as_str()),
            TassadarWasmTextCompileOutcome::Refused { .. } => None,
        }
    }

    /// Returns the compiled Wasm digest when present.
    #[must_use]
    pub fn wasm_binary_digest(&self) -> Option<&str> {
        match &self.outcome {
            TassadarWasmTextCompileOutcome::Succeeded {
                wasm_binary_digest, ..
            } => Some(wasm_binary_digest.as_str()),
            TassadarWasmTextCompileOutcome::Refused { .. } => None,
        }
    }

    /// Returns the compiled Wasm summary when present.
    #[must_use]
    pub fn wasm_binary_summary(&self) -> Option<&psionic_runtime::TassadarWasmBinarySummary> {
        match &self.outcome {
            TassadarWasmTextCompileOutcome::Succeeded {
                wasm_binary_summary,
                ..
            } => Some(wasm_binary_summary),
            TassadarWasmTextCompileOutcome::Refused { .. } => None,
        }
    }

    /// Returns the typed refusal when the compile refused.
    #[must_use]
    pub fn refusal(&self) -> Option<&TassadarCompileRefusal> {
        match &self.outcome {
            TassadarWasmTextCompileOutcome::Succeeded { .. } => None,
            TassadarWasmTextCompileOutcome::Refused { refusal } => Some(refusal),
        }
    }
}

/// Full Wasm-text to Wasm-module to Tassadar-artifact bundle pipeline result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmTextArtifactBundlePipeline {
    /// Machine-readable compile receipt for the source-to-Wasm step.
    pub compile_receipt: TassadarWasmTextCompileReceipt,
    /// Digest-bound Wasm-module lowering result.
    pub artifact_bundle: TassadarWasmModuleArtifactBundle,
}

/// Failure while lowering one normalized Wasm module into runnable runtime
/// artifacts.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarWasmModuleArtifactBundleError {
    /// Parsing or validating the normalized module failed.
    #[error(transparent)]
    Module(#[from] TassadarNormalizedWasmModuleError),
    /// The selected runtime profile has no published trace ABI.
    #[error("no trace ABI is published for Wasm module lowering target `{profile_id}`")]
    UnsupportedTraceAbi {
        /// Runtime profile id.
        profile_id: String,
    },
    /// The module exported no functions to lower.
    #[error("normalized module `{module_digest}` exports no functions")]
    NoFunctionExports {
        /// Module digest.
        module_digest: String,
    },
    /// One function export pointed at an imported function rather than a body.
    #[error("export `{export_name}` points at imported function {function_index}")]
    ExportedImportUnsupported {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
    },
    /// The bounded runtime lane does not yet accept Wasm function parameters.
    #[error(
        "export `{export_name}` function {function_index} declares {param_count} params, but the bounded runtime lane only admits zero-parameter module exports today"
    )]
    UnsupportedParamCount {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
        /// Declared param count.
        param_count: usize,
    },
    /// The bounded runtime lane only accepts zero or one `i32` result.
    #[error(
        "export `{export_name}` function {function_index} declares unsupported results {result_types:?}"
    )]
    UnsupportedResultTypes {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
        /// Declared result types.
        result_types: Vec<String>,
    },
    /// One lowered local type falls outside the current i32-only runtime.
    #[error(
        "export `{export_name}` function {function_index} declares unsupported local type `{local_type}`"
    )]
    UnsupportedLocalType {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
        /// Unsupported local type.
        local_type: String,
    },
    /// One local index does not fit inside the current runtime instruction surface.
    #[error(
        "export `{export_name}` function {function_index} references local {local_index}, but the bounded runtime only encodes locals up to {max_supported}"
    )]
    UnsupportedLocalIndex {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
        /// Referenced local index.
        local_index: u32,
        /// Maximum encodable local index.
        max_supported: u8,
    },
    /// One memory layout is outside the current runtime representation.
    #[error("unsupported Wasm memory shape: {detail}")]
    UnsupportedMemoryShape {
        /// Human-readable detail.
        detail: String,
    },
    /// One data segment is outside the current runtime representation.
    #[error("unsupported data segment {data_index}: {detail}")]
    UnsupportedDataSegment {
        /// Data segment index.
        data_index: u32,
        /// Human-readable detail.
        detail: String,
    },
    /// One memory instruction relied on a dynamic address.
    #[error(
        "export `{export_name}` function {function_index} used a dynamic memory address for `{opcode}`; byte-addressed memory ABI closure remains out of scope"
    )]
    UnsupportedDynamicMemoryAddress {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
        /// Opcode mnemonic.
        opcode: String,
    },
    /// One memory immediate or address shape is unsupported.
    #[error(
        "export `{export_name}` function {function_index} used unsupported memory form for `{opcode}`: {detail}"
    )]
    UnsupportedMemoryImmediate {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
        /// Opcode mnemonic.
        opcode: String,
        /// Human-readable detail.
        detail: String,
    },
    /// One call instruction reached the lowering boundary before call frames land.
    #[error(
        "export `{export_name}` function {function_index} calls function {target_function_index}, but call-frame support is not yet landed"
    )]
    UnsupportedCall {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
        /// Callee function index.
        target_function_index: u32,
    },
    /// One parsed Wasm instruction is still outside the bounded runtime lowering slice.
    #[error(
        "export `{export_name}` function {function_index} uses unsupported instruction `{opcode}` for the current runtime lowering slice"
    )]
    UnsupportedInstruction {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
        /// Unsupported opcode mnemonic.
        opcode: String,
    },
    /// One drop instruction could not be represented by the bounded runtime.
    #[error(
        "export `{export_name}` function {function_index} requires `drop` over a materialized stack value"
    )]
    UnsupportedDrop {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
    },
    /// One lowered function violated the expected straight-line stack discipline.
    #[error(
        "export `{export_name}` function {function_index} violated bounded stack discipline: {detail}"
    )]
    InvalidStackState {
        /// Export name.
        export_name: String,
        /// Function index.
        function_index: u32,
        /// Human-readable detail.
        detail: String,
    },
    /// One runtime validation or execution refusal occurred after lowering.
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    /// Final program-artifact assembly failed validation.
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
}

impl TassadarWasmModuleArtifactBundleError {
    /// Returns the stable machine-readable refusal kind for this lowering error.
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
            Self::UnsupportedCall { .. } => "unsupported_call",
            Self::UnsupportedInstruction { .. } => "unsupported_instruction",
            Self::UnsupportedDrop { .. } => "unsupported_drop",
            Self::InvalidStackState { .. } => "invalid_stack_state",
            Self::Execution(_) => "execution",
            Self::ProgramArtifact(_) => "program_artifact",
        }
    }
}

/// Failure while compiling a C source all the way into a runnable Tassadar
/// artifact bundle.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarCSourceArtifactBundlePipelineError {
    /// The source-to-Wasm compile refused before lowering.
    #[error("C-to-Wasm compile refused: {refusal}")]
    CompileRefused {
        /// Compile receipt preserving the full refusal record.
        compile_receipt: TassadarCToWasmCompileReceipt,
        /// Typed refusal fact for the source-to-Wasm step.
        refusal: TassadarCompileRefusal,
    },
    /// The compiled Wasm output could not be read back for lowering.
    #[error("failed to read compiled Wasm `{path}`: {message}")]
    ReadCompiledWasm {
        /// Compile receipt that produced the output ref.
        compile_receipt: TassadarCToWasmCompileReceipt,
        /// Wasm output path.
        path: String,
        /// IO failure summary.
        message: String,
    },
    /// The C source compiled to Wasm, but lowering into the current Tassadar
    /// lane refused explicitly.
    #[error("Wasm-module lowering refused after compile receipt `{receipt_digest}`: {error}")]
    LoweringRefused {
        /// Compile receipt proving the source-to-Wasm step succeeded.
        compile_receipt: TassadarCToWasmCompileReceipt,
        /// Stable digest of the compile receipt.
        receipt_digest: String,
        /// Typed lowering refusal.
        error: TassadarWasmModuleArtifactBundleError,
    },
}

/// Failure while compiling a Rust source all the way into a runnable Tassadar
/// artifact bundle.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarRustSourceArtifactBundlePipelineError {
    /// The source-to-Wasm compile refused before lowering.
    #[error("Rust-to-Wasm compile refused: {refusal}")]
    CompileRefused {
        /// Compile receipt preserving the full refusal record.
        compile_receipt: TassadarRustToWasmCompileReceipt,
        /// Typed refusal fact for the source-to-Wasm step.
        refusal: TassadarCompileRefusal,
    },
    /// The compiled Wasm output could not be read back for lowering.
    #[error("failed to read compiled Wasm `{path}`: {message}")]
    ReadCompiledWasm {
        /// Compile receipt that produced the output ref.
        compile_receipt: TassadarRustToWasmCompileReceipt,
        /// Wasm output path.
        path: String,
        /// IO failure summary.
        message: String,
    },
    /// The Rust source compiled to Wasm, but lowering into the current
    /// Tassadar lane refused explicitly.
    #[error("Wasm-module lowering refused after compile receipt `{receipt_digest}`: {error}")]
    LoweringRefused {
        /// Compile receipt proving the source-to-Wasm step succeeded.
        compile_receipt: TassadarRustToWasmCompileReceipt,
        /// Stable digest of the compile receipt.
        receipt_digest: String,
        /// Typed lowering refusal.
        error: TassadarWasmModuleArtifactBundleError,
    },
}

/// Failure while compiling one Wasm-text source all the way into a runnable
/// Tassadar artifact bundle.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarWasmTextArtifactBundlePipelineError {
    /// The source-to-Wasm compile refused before lowering.
    #[error("Wasm-text compile refused: {refusal}")]
    CompileRefused {
        /// Compile receipt preserving the full refusal record.
        compile_receipt: TassadarWasmTextCompileReceipt,
        /// Typed refusal fact for the source-to-Wasm step.
        refusal: TassadarCompileRefusal,
    },
    /// The compiled Wasm output could not be written for lowering.
    #[error("failed to write compiled Wasm `{path}`: {message}")]
    WriteCompiledWasm {
        /// Compile receipt that produced the output ref.
        compile_receipt: TassadarWasmTextCompileReceipt,
        /// Wasm output path.
        path: String,
        /// IO failure summary.
        message: String,
    },
    /// The Wasm-text source compiled to Wasm, but lowering refused explicitly.
    #[error("Wasm-module lowering refused after compile receipt `{receipt_digest}`: {error}")]
    LoweringRefused {
        /// Compile receipt proving the source-to-Wasm step succeeded.
        compile_receipt: TassadarWasmTextCompileReceipt,
        /// Stable digest of the compile receipt.
        receipt_digest: String,
        /// Typed lowering refusal.
        error: TassadarWasmModuleArtifactBundleError,
    },
}

/// Parses one Wasm binary and lowers its exported bounded module functions into
/// runnable runtime artifacts.
pub fn compile_tassadar_wasm_binary_module_to_artifact_bundle(
    source_name: impl Into<String>,
    wasm_bytes: &[u8],
    profile: &TassadarWasmProfile,
) -> Result<TassadarWasmModuleArtifactBundle, TassadarWasmModuleArtifactBundleError> {
    let normalized_module = parse_tassadar_normalized_wasm_module(wasm_bytes)?;
    compile_tassadar_normalized_wasm_module_to_artifact_bundle_with_source(
        source_name.into(),
        stable_bytes_digest(wasm_bytes),
        normalized_module,
        profile,
    )
}

/// Compiles one C source through the runtime-owned source-to-Wasm lane and then
/// lowers the compiled Wasm output into the bounded Tassadar artifact lane.
pub fn compile_tassadar_c_source_to_artifact_bundle(
    source_name: impl Into<String>,
    source_bytes: &[u8],
    output_wasm_path: impl AsRef<Path>,
    compile_config: &TassadarCToWasmCompileConfig,
    profile: &TassadarWasmProfile,
) -> Result<TassadarCSourceArtifactBundlePipeline, TassadarCSourceArtifactBundlePipelineError> {
    let source_name = source_name.into();
    let output_wasm_path = output_wasm_path.as_ref();
    let compile_receipt = compile_tassadar_c_source_to_wasm_receipt(
        source_name.clone(),
        source_bytes,
        output_wasm_path,
        compile_config,
    );
    if let Some(refusal) = compile_receipt.refusal().cloned() {
        return Err(TassadarCSourceArtifactBundlePipelineError::CompileRefused {
            compile_receipt,
            refusal,
        });
    }

    let wasm_bytes = std::fs::read(output_wasm_path).map_err(|error| {
        TassadarCSourceArtifactBundlePipelineError::ReadCompiledWasm {
            compile_receipt: compile_receipt.clone(),
            path: output_wasm_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let artifact_bundle =
        compile_tassadar_wasm_binary_module_to_artifact_bundle(source_name, &wasm_bytes, profile)
            .map_err(
            |error| TassadarCSourceArtifactBundlePipelineError::LoweringRefused {
                compile_receipt: compile_receipt.clone(),
                receipt_digest: compile_receipt.receipt_digest.clone(),
                error,
            },
        )?;
    Ok(TassadarCSourceArtifactBundlePipeline {
        compile_receipt,
        artifact_bundle,
    })
}

/// Compiles one Rust source through the runtime-owned source-to-Wasm lane and
/// then lowers the compiled Wasm output into the bounded Tassadar artifact lane.
pub fn compile_tassadar_rust_source_to_artifact_bundle(
    source_name: impl Into<String>,
    source_bytes: &[u8],
    output_wasm_path: impl AsRef<Path>,
    compile_config: &TassadarRustToWasmCompileConfig,
    profile: &TassadarWasmProfile,
) -> Result<TassadarRustSourceArtifactBundlePipeline, TassadarRustSourceArtifactBundlePipelineError>
{
    let source_name = source_name.into();
    let output_wasm_path = output_wasm_path.as_ref();
    let compile_receipt = compile_tassadar_rust_source_to_wasm_receipt(
        source_name.clone(),
        source_bytes,
        output_wasm_path,
        compile_config,
    );
    if let Some(refusal) = compile_receipt.refusal().cloned() {
        return Err(
            TassadarRustSourceArtifactBundlePipelineError::CompileRefused {
                compile_receipt,
                refusal,
            },
        );
    }

    let wasm_bytes = std::fs::read(output_wasm_path).map_err(|error| {
        TassadarRustSourceArtifactBundlePipelineError::ReadCompiledWasm {
            compile_receipt: compile_receipt.clone(),
            path: output_wasm_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let artifact_bundle =
        compile_tassadar_wasm_binary_module_to_artifact_bundle(source_name, &wasm_bytes, profile)
            .map_err(
            |error| TassadarRustSourceArtifactBundlePipelineError::LoweringRefused {
                compile_receipt: compile_receipt.clone(),
                receipt_digest: compile_receipt.receipt_digest.clone(),
                error,
            },
        )?;
    Ok(TassadarRustSourceArtifactBundlePipeline {
        compile_receipt,
        artifact_bundle,
    })
}

/// Compiles one Wasm-text source into a Wasm binary and then lowers it into the
/// bounded Tassadar artifact lane.
pub fn compile_tassadar_wasm_text_to_artifact_bundle(
    source_name: impl Into<String>,
    source_text: &str,
    output_wasm_path: impl AsRef<Path>,
    compile_config: &TassadarWasmTextCompileConfig,
    profile: &TassadarWasmProfile,
) -> Result<TassadarWasmTextArtifactBundlePipeline, TassadarWasmTextArtifactBundlePipelineError> {
    let source_name = source_name.into();
    let source_identity = TassadarProgramSourceIdentity::new(
        TassadarProgramSourceKind::WasmText,
        source_name.clone(),
        stable_bytes_digest(source_text.as_bytes()),
    );
    let toolchain_identity = TassadarCompilerToolchainIdentity::new(
        TASSADAR_WASM_TEXT_COMPILER_FAMILY,
        TASSADAR_WASM_TEXT_COMPILER_VERSION,
        "wasm32-unknown-unknown",
    )
    .with_pipeline_features(compile_config.pipeline_features());
    let output_wasm_path = output_wasm_path.as_ref();

    let encoded_wasm = match wat::parse_str(source_text) {
        Ok(bytes) => bytes,
        Err(error) => {
            let compile_receipt = TassadarWasmTextCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarWasmTextCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::InvalidWasmOutput {
                        message: error.to_string(),
                    },
                },
            );
            return Err(
                TassadarWasmTextArtifactBundlePipelineError::CompileRefused {
                    refusal: compile_receipt
                        .refusal()
                        .expect("refused receipt should carry refusal")
                        .clone(),
                    compile_receipt,
                },
            );
        }
    };
    let wasm_binary_summary = match summarize_tassadar_wasm_binary(&encoded_wasm) {
        Ok(summary) => summary,
        Err(message) => {
            let compile_receipt = TassadarWasmTextCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarWasmTextCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::InvalidWasmOutput { message },
                },
            );
            return Err(
                TassadarWasmTextArtifactBundlePipelineError::CompileRefused {
                    refusal: compile_receipt
                        .refusal()
                        .expect("refused receipt should carry refusal")
                        .clone(),
                    compile_receipt,
                },
            );
        }
    };
    for expected_export in &compile_config.export_symbols {
        if !wasm_binary_summary
            .exported_functions
            .contains(expected_export)
        {
            let compile_receipt = TassadarWasmTextCompileReceipt::new(
                source_identity,
                toolchain_identity,
                compile_config.clone(),
                TassadarWasmTextCompileOutcome::Refused {
                    refusal: TassadarCompileRefusal::MissingExpectedExport {
                        expected: expected_export.clone(),
                        actual: wasm_binary_summary.exported_functions.clone(),
                    },
                },
            );
            return Err(
                TassadarWasmTextArtifactBundlePipelineError::CompileRefused {
                    refusal: compile_receipt
                        .refusal()
                        .expect("refused receipt should carry refusal")
                        .clone(),
                    compile_receipt,
                },
            );
        }
    }
    if let Some(parent) = output_wasm_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            let compile_receipt = TassadarWasmTextCompileReceipt::new(
                source_identity.clone(),
                toolchain_identity.clone(),
                compile_config.clone(),
                TassadarWasmTextCompileOutcome::Succeeded {
                    wasm_binary_ref: canonical_repo_relative_path(output_wasm_path),
                    wasm_binary_digest: stable_bytes_digest(&encoded_wasm),
                    wasm_binary_summary: wasm_binary_summary.clone(),
                },
            );
            TassadarWasmTextArtifactBundlePipelineError::WriteCompiledWasm {
                compile_receipt,
                path: parent.display().to_string(),
                message: error.to_string(),
            }
        })?;
    }
    std::fs::write(output_wasm_path, &encoded_wasm).map_err(|error| {
        let compile_receipt = TassadarWasmTextCompileReceipt::new(
            source_identity.clone(),
            toolchain_identity.clone(),
            compile_config.clone(),
            TassadarWasmTextCompileOutcome::Succeeded {
                wasm_binary_ref: canonical_repo_relative_path(output_wasm_path),
                wasm_binary_digest: stable_bytes_digest(&encoded_wasm),
                wasm_binary_summary: wasm_binary_summary.clone(),
            },
        );
        TassadarWasmTextArtifactBundlePipelineError::WriteCompiledWasm {
            compile_receipt,
            path: output_wasm_path.display().to_string(),
            message: error.to_string(),
        }
    })?;

    let compile_receipt = TassadarWasmTextCompileReceipt::new(
        source_identity,
        toolchain_identity,
        compile_config.clone(),
        TassadarWasmTextCompileOutcome::Succeeded {
            wasm_binary_ref: canonical_repo_relative_path(output_wasm_path),
            wasm_binary_digest: stable_bytes_digest(&encoded_wasm),
            wasm_binary_summary,
        },
    );
    let artifact_bundle =
        compile_tassadar_wasm_binary_module_to_artifact_bundle(source_name, &encoded_wasm, profile)
            .map_err(
                |error| TassadarWasmTextArtifactBundlePipelineError::LoweringRefused {
                    receipt_digest: compile_receipt.receipt_digest.clone(),
                    compile_receipt: compile_receipt.clone(),
                    error,
                },
            )?;
    Ok(TassadarWasmTextArtifactBundlePipeline {
        compile_receipt,
        artifact_bundle,
    })
}

/// Lowers one normalized Wasm module into runnable runtime artifacts.
pub fn compile_tassadar_normalized_wasm_module_to_artifact_bundle(
    source_name: impl Into<String>,
    normalized_module: &TassadarNormalizedWasmModule,
    profile: &TassadarWasmProfile,
) -> Result<TassadarWasmModuleArtifactBundle, TassadarWasmModuleArtifactBundleError> {
    let wasm_bytes = encode_tassadar_normalized_wasm_module(normalized_module)?;
    compile_tassadar_normalized_wasm_module_to_artifact_bundle_with_source(
        source_name.into(),
        stable_bytes_digest(&wasm_bytes),
        normalized_module.clone(),
        profile,
    )
}

fn compile_tassadar_normalized_wasm_module_to_artifact_bundle_with_source(
    source_name: String,
    source_digest: String,
    normalized_module: TassadarNormalizedWasmModule,
    profile: &TassadarWasmProfile,
) -> Result<TassadarWasmModuleArtifactBundle, TassadarWasmModuleArtifactBundleError> {
    normalized_module.validate_internal_consistency()?;
    let Some(trace_abi) = tassadar_trace_abi_for_profile_id(profile.profile_id.as_str()) else {
        return Err(TassadarWasmModuleArtifactBundleError::UnsupportedTraceAbi {
            profile_id: profile.profile_id.clone(),
        });
    };
    validate_memory_shape(&normalized_module)?;
    let source_identity = TassadarProgramSourceIdentity::new(
        TassadarProgramSourceKind::WasmBinary,
        source_name,
        source_digest,
    );
    let toolchain_identity = TassadarCompilerToolchainIdentity::new(
        TASSADAR_WASM_MODULE_COMPILER_FAMILY,
        TASSADAR_WASM_MODULE_COMPILER_VERSION,
        profile.profile_id.clone(),
    )
    .with_pipeline_features(vec![
        String::from("normalized_module_ir"),
        String::from("export_only_lowering"),
        String::from("straight_line_only"),
    ]);

    let function_exports = normalized_module
        .exports
        .iter()
        .filter(|export| export.kind == psionic_ir::TassadarNormalizedWasmExportKind::Function)
        .cloned()
        .collect::<Vec<_>>();
    if function_exports.is_empty() {
        return Err(TassadarWasmModuleArtifactBundleError::NoFunctionExports {
            module_digest: normalized_module.module_digest.clone(),
        });
    }

    let base_memory = initial_memory_image(&normalized_module)?;
    let lowered_exports = function_exports
        .into_iter()
        .map(|export| {
            lower_function_export(
                &normalized_module,
                &export.export_name,
                export.index,
                &base_memory,
                &source_identity,
                &toolchain_identity,
                profile,
                &trace_abi,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TassadarWasmModuleArtifactBundle::new(
        format!(
            "tassadar.wasm_module.{}.artifact_bundle.v1",
            &normalized_module.module_digest[..12]
        ),
        source_identity,
        toolchain_identity,
        normalized_module,
        lowered_exports,
    ))
}

/// Failure while lowering one normalized Wasm export into the bounded
/// module-execution runtime lane.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarWasmModuleExecutionLoweringError {
    /// The normalized module was invalid.
    #[error(transparent)]
    Module(#[from] TassadarNormalizedWasmModuleError),
    /// The requested export was missing or not a function export.
    #[error("module-execution lowering is missing function export `{export_name}`")]
    MissingFunctionExport { export_name: String },
    /// The current module-execution lane still refuses imported functions.
    #[error("module-execution lowering still refuses imported function {function_index}")]
    ImportedFunctionUnsupported { function_index: u32 },
    /// One memory shape is unsupported in the bounded module-execution lane.
    #[error("module-execution lowering refused memory {memory_index}: {detail}")]
    UnsupportedMemory { memory_index: u32, detail: String },
    /// One data-segment shape is unsupported in the bounded module-execution lane.
    #[error("module-execution lowering refused data segment {data_index}: {detail}")]
    UnsupportedDataSegment { data_index: u32, detail: String },
    /// One function declared unsupported parameters.
    #[error(
        "module-execution lowering requires zero-parameter functions, but function {function_index} declared {param_count}"
    )]
    UnsupportedParamCount {
        function_index: u32,
        param_count: usize,
    },
    /// One function declared unsupported result types.
    #[error(
        "module-execution lowering requires result_count <= 1 with i32-only returns for function {function_index}"
    )]
    UnsupportedResultTypes { function_index: u32 },
    /// One local type is unsupported.
    #[error(
        "module-execution lowering requires i32 locals, but function {function_index} used `{local_type:?}`"
    )]
    UnsupportedLocalType {
        function_index: u32,
        local_type: TassadarNormalizedWasmValueType,
    },
    /// One global type or initializer is unsupported.
    #[error(
        "module-execution lowering requires i32 globals with bounded const init for global {global_index}: {detail}"
    )]
    UnsupportedGlobal { global_index: u32, detail: String },
    /// One table shape is unsupported.
    #[error(
        "module-execution lowering requires bounded funcref tables for table {table_index}: {detail}"
    )]
    UnsupportedTable { table_index: u32, detail: String },
    /// One element segment was unsupported.
    #[error("module-execution lowering refused element segment {element_index}: {detail}")]
    UnsupportedElementSegment { element_index: u32, detail: String },
    /// One instruction is unsupported in the bounded module-execution lane.
    #[error(
        "module-execution lowering refused instruction `{opcode}` in function {function_index}"
    )]
    UnsupportedInstruction { function_index: u32, opcode: String },
    /// The lowered runtime program failed validation.
    #[error(transparent)]
    RuntimeProgram(#[from] psionic_runtime::TassadarModuleExecutionError),
}

/// Lowers one normalized Wasm function export into the bounded module-execution runtime lane.
pub fn compile_tassadar_normalized_wasm_module_export_to_module_execution_program(
    source_name: impl Into<String>,
    normalized_module: &TassadarNormalizedWasmModule,
    export_name: &str,
) -> Result<TassadarModuleExecutionProgram, TassadarWasmModuleExecutionLoweringError> {
    normalized_module.validate_internal_consistency()?;
    let (export, _) = normalized_module
        .exported_function_by_name(export_name)
        .ok_or_else(
            || TassadarWasmModuleExecutionLoweringError::MissingFunctionExport {
                export_name: export_name.to_string(),
            },
        )?;
    if normalized_module
        .functions
        .iter()
        .any(|function| function.imported_function())
    {
        let function_index = normalized_module
            .functions
            .iter()
            .find(|function| function.imported_function())
            .map_or(0, |function| function.function_index);
        return Err(
            TassadarWasmModuleExecutionLoweringError::ImportedFunctionUnsupported {
                function_index,
            },
        );
    }

    let mut lowered_globals = Vec::with_capacity(normalized_module.globals.len());
    for global in &normalized_module.globals {
        let initial_value = match global.init_expr {
            TassadarNormalizedWasmConstExpr::I32Const { value } => value,
            TassadarNormalizedWasmConstExpr::GlobalGet { global_index } => lowered_globals
                .get(global_index as usize)
                .map(|global: &TassadarModuleGlobal| global.initial_value)
                .ok_or_else(
                    || TassadarWasmModuleExecutionLoweringError::UnsupportedGlobal {
                        global_index: global.global_index,
                        detail: format!("global.get init {global_index} is not yet resolvable"),
                    },
                )?,
        };
        if global.value_type != TassadarNormalizedWasmValueType::I32 {
            return Err(
                TassadarWasmModuleExecutionLoweringError::UnsupportedGlobal {
                    global_index: global.global_index,
                    detail: format!("value type {:?}", global.value_type),
                },
            );
        }
        lowered_globals.push(TassadarModuleGlobal {
            global_index: global.global_index,
            value_type: TassadarModuleValueType::I32,
            mutability: match global.mutability {
                TassadarNormalizedWasmGlobalMutability::Const => {
                    TassadarModuleGlobalMutability::Const
                }
                TassadarNormalizedWasmGlobalMutability::Mutable => {
                    TassadarModuleGlobalMutability::Mutable
                }
            },
            initial_value,
        });
    }

    let lowered_tables = normalized_module
        .tables
        .iter()
        .map(|table| {
            if table.element_kind != TassadarNormalizedWasmTableElementKind::Funcref {
                return Err(TassadarWasmModuleExecutionLoweringError::UnsupportedTable {
                    table_index: table.table_index,
                    detail: String::from("non-funcref table"),
                });
            }
            if table.max_entries.map_or(table.min_entries, |max| max) > 64 {
                return Err(TassadarWasmModuleExecutionLoweringError::UnsupportedTable {
                    table_index: table.table_index,
                    detail: String::from("table exceeds 64-entry bounded lane"),
                });
            }
            let entry_count = table.max_entries.unwrap_or(table.min_entries) as usize;
            Ok(TassadarModuleTable {
                table_index: table.table_index,
                element_kind: TassadarModuleTableElementKind::Funcref,
                min_entries: table.min_entries,
                max_entries: table.max_entries,
                elements: vec![None; entry_count],
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let lowered_memories = normalized_module
        .memories
        .iter()
        .map(|memory| {
            if normalized_module.memories.len() > 1 {
                return Err(TassadarWasmModuleExecutionLoweringError::UnsupportedMemory {
                    memory_index: memory.memory_index,
                    detail: format!(
                        "module declares {} memories, but the bounded module lane admits one memory at most",
                        normalized_module.memories.len()
                    ),
                });
            }
            if memory.imported_memory() {
                return Err(TassadarWasmModuleExecutionLoweringError::UnsupportedMemory {
                    memory_index: memory.memory_index,
                    detail: String::from("imported memories remain out of scope"),
                });
            }
            if memory.memory_type.memory64 {
                return Err(TassadarWasmModuleExecutionLoweringError::UnsupportedMemory {
                    memory_index: memory.memory_index,
                    detail: String::from("memory64 remains out of scope"),
                });
            }
            if memory.memory_type.shared {
                return Err(TassadarWasmModuleExecutionLoweringError::UnsupportedMemory {
                    memory_index: memory.memory_index,
                    detail: String::from("shared memories remain out of scope"),
                });
            }
            if memory.memory_type.page_size_log2.is_some() {
                return Err(TassadarWasmModuleExecutionLoweringError::UnsupportedMemory {
                    memory_index: memory.memory_index,
                    detail: String::from("custom page sizes remain out of scope"),
                });
            }
            let initial_pages = u32::try_from(memory.memory_type.minimum_pages).map_err(|_| {
                TassadarWasmModuleExecutionLoweringError::UnsupportedMemory {
                    memory_index: memory.memory_index,
                    detail: format!(
                        "minimum_pages {} exceeded u32",
                        memory.memory_type.minimum_pages
                    ),
                }
            })?;
            let max_pages = u32::try_from(
                memory
                    .memory_type
                    .maximum_pages
                    .unwrap_or(memory.memory_type.minimum_pages),
            )
            .map_err(|_| TassadarWasmModuleExecutionLoweringError::UnsupportedMemory {
                memory_index: memory.memory_index,
                detail: String::from("maximum_pages exceeded u32"),
            })?;
            Ok(TassadarModuleMemory {
                memory_index: memory.memory_index,
                initial_pages,
                max_pages,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let lowered_functions = normalized_module
        .functions
        .iter()
        .map(|function| lower_module_execution_function(normalized_module, function))
        .collect::<Result<Vec<_>, _>>()?;

    let lowered_element_segments = normalized_module
        .element_segments
        .iter()
        .map(|segment| {
            let offset = match segment.offset_expr {
                TassadarNormalizedWasmConstExpr::I32Const { value } if value >= 0 => value as u32,
                TassadarNormalizedWasmConstExpr::I32Const { value } => {
                    return Err(
                        TassadarWasmModuleExecutionLoweringError::UnsupportedElementSegment {
                            element_index: segment.element_index,
                            detail: format!("negative offset {value}"),
                        },
                    );
                }
                TassadarNormalizedWasmConstExpr::GlobalGet { global_index } => {
                    return Err(
                        TassadarWasmModuleExecutionLoweringError::UnsupportedElementSegment {
                            element_index: segment.element_index,
                            detail: format!("global.get offset {global_index}"),
                        },
                    );
                }
            };
            Ok(TassadarModuleElementSegment {
                element_segment_index: segment.element_index,
                table_index: segment.table_index,
                offset,
                elements: segment.function_indices.iter().copied().map(Some).collect(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let lowered_data_segments = normalized_module
        .data_segments
        .iter()
        .map(|segment| {
            let (memory_index, offset) = match &segment.mode {
                TassadarNormalizedWasmDataMode::Passive => {
                    return Err(
                        TassadarWasmModuleExecutionLoweringError::UnsupportedDataSegment {
                            data_index: segment.data_index,
                            detail: String::from("passive data segments remain out of scope"),
                        },
                    );
                }
                TassadarNormalizedWasmDataMode::Active {
                    memory_index,
                    offset_expr,
                } => {
                    let offset = match offset_expr {
                        TassadarNormalizedWasmConstExpr::I32Const { value } if *value >= 0 => {
                            *value as u32
                        }
                        TassadarNormalizedWasmConstExpr::I32Const { value } => {
                            return Err(
                                TassadarWasmModuleExecutionLoweringError::UnsupportedDataSegment {
                                    data_index: segment.data_index,
                                    detail: format!("negative offset {value}"),
                                },
                            );
                        }
                        TassadarNormalizedWasmConstExpr::GlobalGet { global_index } => {
                            return Err(
                                TassadarWasmModuleExecutionLoweringError::UnsupportedDataSegment {
                                    data_index: segment.data_index,
                                    detail: format!("global.get offset {global_index}"),
                                },
                            );
                        }
                    };
                    (*memory_index, offset)
                }
            };
            Ok(TassadarModuleDataSegment {
                data_segment_index: segment.data_index,
                memory_index,
                offset,
                bytes: segment.bytes.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut program = TassadarModuleExecutionProgram::new(
        format!(
            "tassadar.wasm_module_execution.{}.{}.program.v1",
            &normalized_module.module_digest[..12],
            sanitize_label(export_name)
        ),
        export.index,
        (lowered_functions.len() as u32).saturating_add(4),
        lowered_globals,
        lowered_tables,
        Vec::new(),
        lowered_functions,
    )
    .with_memories(lowered_memories)
    .with_element_segments(lowered_element_segments);
    if !lowered_data_segments.is_empty() {
        program = program.with_data_segments(lowered_data_segments);
    }
    if let Some(start_function_index) = normalized_module.start_function_index {
        program = program.with_start_function_index(start_function_index);
    }
    let _ = source_name.into();
    program.validate()?;
    Ok(program)
}

fn lower_module_execution_function(
    module: &TassadarNormalizedWasmModule,
    function: &psionic_ir::TassadarNormalizedWasmFunction,
) -> Result<TassadarModuleFunction, TassadarWasmModuleExecutionLoweringError> {
    let body = function.body.as_ref().ok_or(
        TassadarWasmModuleExecutionLoweringError::ImportedFunctionUnsupported {
            function_index: function.function_index,
        },
    )?;
    let signature = &module.types[function.type_index as usize];
    if !signature.params.is_empty() {
        return Err(
            TassadarWasmModuleExecutionLoweringError::UnsupportedParamCount {
                function_index: function.function_index,
                param_count: signature.params.len(),
            },
        );
    }
    if signature.results.len() > 1
        || signature
            .results
            .iter()
            .any(|result| *result != TassadarNormalizedWasmValueType::I32)
    {
        return Err(
            TassadarWasmModuleExecutionLoweringError::UnsupportedResultTypes {
                function_index: function.function_index,
            },
        );
    }
    for local in &body.locals {
        if *local != TassadarNormalizedWasmValueType::I32 {
            return Err(
                TassadarWasmModuleExecutionLoweringError::UnsupportedLocalType {
                    function_index: function.function_index,
                    local_type: *local,
                },
            );
        }
    }

    let mut instructions = Vec::new();
    for instruction in &body.instructions {
        match instruction {
            TassadarNormalizedWasmInstruction::I32Const { value } => {
                instructions.push(TassadarModuleInstruction::I32Const { value: *value });
            }
            TassadarNormalizedWasmInstruction::LocalGet { local_index } => {
                instructions.push(TassadarModuleInstruction::LocalGet {
                    local_index: *local_index,
                });
            }
            TassadarNormalizedWasmInstruction::LocalSet { local_index } => {
                instructions.push(TassadarModuleInstruction::LocalSet {
                    local_index: *local_index,
                });
            }
            TassadarNormalizedWasmInstruction::LocalTee { local_index } => {
                instructions.push(TassadarModuleInstruction::LocalSet {
                    local_index: *local_index,
                });
                instructions.push(TassadarModuleInstruction::LocalGet {
                    local_index: *local_index,
                });
            }
            TassadarNormalizedWasmInstruction::GlobalGet { global_index } => {
                instructions.push(TassadarModuleInstruction::GlobalGet {
                    global_index: *global_index,
                });
            }
            TassadarNormalizedWasmInstruction::GlobalSet { global_index } => {
                instructions.push(TassadarModuleInstruction::GlobalSet {
                    global_index: *global_index,
                });
            }
            TassadarNormalizedWasmInstruction::I32Add => {
                instructions.push(TassadarModuleInstruction::BinaryOp {
                    op: psionic_runtime::TassadarStructuredControlBinaryOp::Add,
                });
            }
            TassadarNormalizedWasmInstruction::I32Sub => {
                instructions.push(TassadarModuleInstruction::BinaryOp {
                    op: psionic_runtime::TassadarStructuredControlBinaryOp::Sub,
                });
            }
            TassadarNormalizedWasmInstruction::I32Mul => {
                instructions.push(TassadarModuleInstruction::BinaryOp {
                    op: psionic_runtime::TassadarStructuredControlBinaryOp::Mul,
                });
            }
            TassadarNormalizedWasmInstruction::I32LtS => {
                instructions.push(TassadarModuleInstruction::BinaryOp {
                    op: psionic_runtime::TassadarStructuredControlBinaryOp::LtS,
                });
            }
            TassadarNormalizedWasmInstruction::I32Load {
                memory_index,
                offset,
                ..
            } => {
                let offset = u32::try_from(*offset).map_err(|_| {
                    TassadarWasmModuleExecutionLoweringError::UnsupportedInstruction {
                        function_index: function.function_index,
                        opcode: String::from("i32.load"),
                    }
                })?;
                instructions.push(TassadarModuleInstruction::I32Load {
                    memory_index: *memory_index,
                    offset,
                });
            }
            TassadarNormalizedWasmInstruction::I32Store {
                memory_index,
                offset,
                ..
            } => {
                let offset = u32::try_from(*offset).map_err(|_| {
                    TassadarWasmModuleExecutionLoweringError::UnsupportedInstruction {
                        function_index: function.function_index,
                        opcode: String::from("i32.store"),
                    }
                })?;
                instructions.push(TassadarModuleInstruction::I32Store {
                    memory_index: *memory_index,
                    offset,
                });
            }
            TassadarNormalizedWasmInstruction::MemorySize { memory_index } => {
                instructions.push(TassadarModuleInstruction::MemorySize {
                    memory_index: *memory_index,
                });
            }
            TassadarNormalizedWasmInstruction::MemoryGrow { memory_index } => {
                instructions.push(TassadarModuleInstruction::MemoryGrow {
                    memory_index: *memory_index,
                });
            }
            TassadarNormalizedWasmInstruction::MemoryCopy {
                dst_memory_index,
                src_memory_index,
            } => {
                instructions.push(TassadarModuleInstruction::MemoryCopy {
                    dst_memory_index: *dst_memory_index,
                    src_memory_index: *src_memory_index,
                });
            }
            TassadarNormalizedWasmInstruction::MemoryFill { memory_index } => {
                instructions.push(TassadarModuleInstruction::MemoryFill {
                    memory_index: *memory_index,
                });
            }
            TassadarNormalizedWasmInstruction::Call { function_index } => {
                instructions.push(TassadarModuleInstruction::Call {
                    function_index: *function_index,
                });
            }
            TassadarNormalizedWasmInstruction::CallIndirect {
                type_index,
                table_index,
            } => {
                let signature = &module.types[*type_index as usize];
                if !signature.params.is_empty()
                    || signature.results.len() > 1
                    || signature
                        .results
                        .iter()
                        .any(|result| *result != TassadarNormalizedWasmValueType::I32)
                {
                    return Err(
                        TassadarWasmModuleExecutionLoweringError::UnsupportedInstruction {
                            function_index: function.function_index,
                            opcode: String::from("call_indirect"),
                        },
                    );
                }
                instructions.push(TassadarModuleInstruction::CallIndirect {
                    table_index: *table_index,
                });
            }
            TassadarNormalizedWasmInstruction::Drop => {
                instructions.push(TassadarModuleInstruction::Drop);
            }
            TassadarNormalizedWasmInstruction::Return => {
                instructions.push(TassadarModuleInstruction::Return);
            }
            other => {
                return Err(
                    TassadarWasmModuleExecutionLoweringError::UnsupportedInstruction {
                        function_index: function.function_index,
                        opcode: other.mnemonic().to_string(),
                    },
                );
            }
        }
    }

    Ok(TassadarModuleFunction::new(
        function.function_index,
        format!("f{}", function.function_index),
        0,
        body.locals.len(),
        signature.results.len() as u8,
        instructions,
    ))
}

fn lower_function_export(
    module: &TassadarNormalizedWasmModule,
    export_name: &str,
    function_index: u32,
    base_memory: &[i32],
    source_identity: &TassadarProgramSourceIdentity,
    toolchain_identity: &TassadarCompilerToolchainIdentity,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
) -> Result<TassadarWasmModuleExportArtifact, TassadarWasmModuleArtifactBundleError> {
    let function = module
        .functions
        .iter()
        .find(|function| function.function_index == function_index)
        .ok_or_else(
            || TassadarWasmModuleArtifactBundleError::InvalidStackState {
                export_name: export_name.to_string(),
                function_index,
                detail: String::from(
                    "export referenced a missing function after module validation",
                ),
            },
        )?;
    let body = function.body.as_ref().ok_or_else(|| {
        TassadarWasmModuleArtifactBundleError::ExportedImportUnsupported {
            export_name: export_name.to_string(),
            function_index,
        }
    })?;
    let signature = &module.types[function.type_index as usize];
    if !signature.params.is_empty() {
        return Err(
            TassadarWasmModuleArtifactBundleError::UnsupportedParamCount {
                export_name: export_name.to_string(),
                function_index,
                param_count: signature.params.len(),
            },
        );
    }
    if signature.results.len() > 1
        || signature
            .results
            .iter()
            .any(|result| *result != TassadarNormalizedWasmValueType::I32)
    {
        return Err(
            TassadarWasmModuleArtifactBundleError::UnsupportedResultTypes {
                export_name: export_name.to_string(),
                function_index,
                result_types: signature
                    .results
                    .iter()
                    .map(|result| format!("{result:?}"))
                    .collect(),
            },
        );
    }
    for local in &body.locals {
        if *local != TassadarNormalizedWasmValueType::I32 {
            return Err(
                TassadarWasmModuleArtifactBundleError::UnsupportedLocalType {
                    export_name: export_name.to_string(),
                    function_index,
                    local_type: format!("{local:?}"),
                },
            );
        }
    }

    let local_count = body.locals.len();
    let mut runtime_instructions = Vec::new();
    let mut stack = Vec::<PendingValue>::new();
    let mut max_slot = base_memory.len().saturating_sub(1);
    let mut terminated = false;

    for instruction in &body.instructions {
        if terminated {
            return Err(TassadarWasmModuleArtifactBundleError::InvalidStackState {
                export_name: export_name.to_string(),
                function_index,
                detail: String::from("instructions continued after explicit return"),
            });
        }
        match instruction {
            TassadarNormalizedWasmInstruction::I32Const { value } => {
                stack.push(PendingValue::Const(*value));
            }
            TassadarNormalizedWasmInstruction::LocalGet { local_index } => {
                validate_local_index(export_name, function_index, *local_index, local_count)?;
                stack.push(PendingValue::Local(*local_index));
            }
            TassadarNormalizedWasmInstruction::LocalSet { local_index } => {
                validate_local_index(export_name, function_index, *local_index, local_count)?;
                let value = pop_stack_value(export_name, function_index, &mut stack, "local.set")?;
                materialize_value(
                    export_name,
                    function_index,
                    value,
                    &mut runtime_instructions,
                    local_count,
                )?;
                runtime_instructions.push(TassadarInstruction::LocalSet {
                    local: u8::try_from(*local_index)
                        .expect("validated local index should fit in u8"),
                });
            }
            TassadarNormalizedWasmInstruction::LocalTee { local_index } => {
                validate_local_index(export_name, function_index, *local_index, local_count)?;
                let value = pop_stack_value(export_name, function_index, &mut stack, "local.tee")?;
                materialize_value(
                    export_name,
                    function_index,
                    value,
                    &mut runtime_instructions,
                    local_count,
                )?;
                let local =
                    u8::try_from(*local_index).expect("validated local index should fit in u8");
                runtime_instructions.push(TassadarInstruction::LocalSet { local });
                runtime_instructions.push(TassadarInstruction::LocalGet { local });
                stack.push(PendingValue::StackValue);
            }
            TassadarNormalizedWasmInstruction::GlobalGet { .. }
            | TassadarNormalizedWasmInstruction::GlobalSet { .. }
            | TassadarNormalizedWasmInstruction::CallIndirect { .. }
            | TassadarNormalizedWasmInstruction::MemorySize { .. }
            | TassadarNormalizedWasmInstruction::MemoryGrow { .. }
            | TassadarNormalizedWasmInstruction::MemoryCopy { .. }
            | TassadarNormalizedWasmInstruction::MemoryFill { .. } => {
                return Err(
                    TassadarWasmModuleArtifactBundleError::UnsupportedInstruction {
                        export_name: export_name.to_string(),
                        function_index,
                        opcode: instruction.mnemonic().to_string(),
                    },
                );
            }
            TassadarNormalizedWasmInstruction::I32Add
            | TassadarNormalizedWasmInstruction::I32Sub
            | TassadarNormalizedWasmInstruction::I32Mul
            | TassadarNormalizedWasmInstruction::I32LtS => {
                let right = pop_stack_value(
                    export_name,
                    function_index,
                    &mut stack,
                    instruction.mnemonic(),
                )?;
                let left = pop_stack_value(
                    export_name,
                    function_index,
                    &mut stack,
                    instruction.mnemonic(),
                )?;
                materialize_value(
                    export_name,
                    function_index,
                    left,
                    &mut runtime_instructions,
                    local_count,
                )?;
                materialize_value(
                    export_name,
                    function_index,
                    right,
                    &mut runtime_instructions,
                    local_count,
                )?;
                runtime_instructions.push(match instruction {
                    TassadarNormalizedWasmInstruction::I32Add => TassadarInstruction::I32Add,
                    TassadarNormalizedWasmInstruction::I32Sub => TassadarInstruction::I32Sub,
                    TassadarNormalizedWasmInstruction::I32Mul => TassadarInstruction::I32Mul,
                    TassadarNormalizedWasmInstruction::I32LtS => TassadarInstruction::I32Lt,
                    _ => unreachable!("filtered above"),
                });
                stack.push(PendingValue::StackValue);
            }
            TassadarNormalizedWasmInstruction::I32Shl => {
                return Err(
                    TassadarWasmModuleArtifactBundleError::UnsupportedInstruction {
                        export_name: export_name.to_string(),
                        function_index,
                        opcode: String::from("i32.shl"),
                    },
                );
            }
            TassadarNormalizedWasmInstruction::I32Load {
                offset,
                memory_index,
                ..
            } => {
                let address = pop_stack_value(export_name, function_index, &mut stack, "i32.load")?;
                let slot = resolve_memory_slot(
                    export_name,
                    function_index,
                    "i32.load",
                    address,
                    *offset,
                    *memory_index,
                )?;
                max_slot = max_slot.max(usize::from(slot));
                runtime_instructions.push(TassadarInstruction::I32Load { slot });
                stack.push(PendingValue::StackValue);
            }
            TassadarNormalizedWasmInstruction::I32Store {
                offset,
                memory_index,
                ..
            } => {
                let value = pop_stack_value(export_name, function_index, &mut stack, "i32.store")?;
                let address =
                    pop_stack_value(export_name, function_index, &mut stack, "i32.store")?;
                let slot = resolve_memory_slot(
                    export_name,
                    function_index,
                    "i32.store",
                    address,
                    *offset,
                    *memory_index,
                )?;
                max_slot = max_slot.max(usize::from(slot));
                materialize_value(
                    export_name,
                    function_index,
                    value,
                    &mut runtime_instructions,
                    local_count,
                )?;
                runtime_instructions.push(TassadarInstruction::I32Store { slot });
            }
            TassadarNormalizedWasmInstruction::Call {
                function_index: target,
            } => {
                return Err(TassadarWasmModuleArtifactBundleError::UnsupportedCall {
                    export_name: export_name.to_string(),
                    function_index,
                    target_function_index: *target,
                });
            }
            TassadarNormalizedWasmInstruction::Drop => {
                let value = pop_stack_value(export_name, function_index, &mut stack, "drop")?;
                if matches!(value, PendingValue::StackValue) {
                    return Err(TassadarWasmModuleArtifactBundleError::UnsupportedDrop {
                        export_name: export_name.to_string(),
                        function_index,
                    });
                }
            }
            TassadarNormalizedWasmInstruction::Return => {
                emit_return(
                    export_name,
                    function_index,
                    &signature.results,
                    &mut stack,
                    &mut runtime_instructions,
                    local_count,
                )?;
                terminated = true;
            }
        }
    }

    if !terminated {
        emit_return(
            export_name,
            function_index,
            &signature.results,
            &mut stack,
            &mut runtime_instructions,
            local_count,
        )?;
    }

    let mut initial_memory = base_memory.to_vec();
    if max_slot >= initial_memory.len() {
        initial_memory.resize(max_slot + 1, 0);
    }
    let program_id = format!(
        "tassadar.wasm_module.{}.{}.program.v1",
        &module.module_digest[..12],
        sanitize_label(export_name)
    );
    let validated_program = TassadarProgram::new(
        program_id.clone(),
        profile,
        local_count,
        initial_memory.len(),
        runtime_instructions,
    )
    .with_initial_memory(initial_memory);
    let runner = TassadarCpuReferenceRunner::for_profile(profile.clone()).ok_or_else(|| {
        TassadarWasmModuleArtifactBundleError::UnsupportedTraceAbi {
            profile_id: profile.profile_id.clone(),
        }
    })?;
    let execution = runner.execute(&validated_program)?;
    let program_artifact = TassadarProgramArtifact::new(
        format!(
            "tassadar.wasm_module.{}.{}.artifact.v1",
            &module.module_digest[..12],
            sanitize_label(export_name)
        ),
        source_identity.clone(),
        toolchain_identity.clone(),
        profile,
        trace_abi,
        validated_program,
    )?;
    Ok(TassadarWasmModuleExportArtifact {
        export_name: export_name.to_string(),
        function_index,
        program_artifact,
        execution_manifest: TassadarWasmModuleExportExecutionManifest::new(
            export_name,
            function_index,
            execution.outputs,
            execution.final_memory,
        ),
    })
}

fn validate_memory_shape(
    module: &TassadarNormalizedWasmModule,
) -> Result<(), TassadarWasmModuleArtifactBundleError> {
    if module.memories.len() > 1 {
        return Err(
            TassadarWasmModuleArtifactBundleError::UnsupportedMemoryShape {
                detail: format!(
                    "module declares {} memories, but the bounded runtime still exposes one memory image at most",
                    module.memories.len()
                ),
            },
        );
    }
    for memory in &module.memories {
        if memory.memory_type.memory64 {
            return Err(
                TassadarWasmModuleArtifactBundleError::UnsupportedMemoryShape {
                    detail: format!("memory {} is memory64", memory.memory_index),
                },
            );
        }
        if memory.memory_type.shared {
            return Err(
                TassadarWasmModuleArtifactBundleError::UnsupportedMemoryShape {
                    detail: format!("memory {} is shared", memory.memory_index),
                },
            );
        }
        if memory.memory_type.page_size_log2.is_some() {
            return Err(
                TassadarWasmModuleArtifactBundleError::UnsupportedMemoryShape {
                    detail: format!("memory {} uses a custom page size", memory.memory_index),
                },
            );
        }
    }
    Ok(())
}

fn initial_memory_image(
    module: &TassadarNormalizedWasmModule,
) -> Result<Vec<i32>, TassadarWasmModuleArtifactBundleError> {
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
                    return Err(
                        TassadarWasmModuleArtifactBundleError::UnsupportedDataSegment {
                            data_index: segment.data_index,
                            detail: format!("negative offset {value}"),
                        },
                    );
                }
                other => {
                    return Err(
                        TassadarWasmModuleArtifactBundleError::UnsupportedDataSegment {
                            data_index: segment.data_index,
                            detail: format!("non-constant offset {other:?}"),
                        },
                    );
                }
            },
        };
        if memory_index != 0 {
            return Err(
                TassadarWasmModuleArtifactBundleError::UnsupportedDataSegment {
                    data_index: segment.data_index,
                    detail: format!("memory index {memory_index}"),
                },
            );
        }
        if offset % 4 != 0 {
            return Err(
                TassadarWasmModuleArtifactBundleError::UnsupportedDataSegment {
                    data_index: segment.data_index,
                    detail: format!("byte offset {offset} is not word aligned"),
                },
            );
        }
        if segment.bytes.len() % 4 != 0 {
            return Err(
                TassadarWasmModuleArtifactBundleError::UnsupportedDataSegment {
                    data_index: segment.data_index,
                    detail: format!("byte length {} is not a multiple of 4", segment.bytes.len()),
                },
            );
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

fn validate_local_index(
    export_name: &str,
    function_index: u32,
    local_index: u32,
    local_count: usize,
) -> Result<(), TassadarWasmModuleArtifactBundleError> {
    let local_limit = usize::from(u8::MAX);
    if local_index as usize >= local_count {
        return Err(TassadarWasmModuleArtifactBundleError::InvalidStackState {
            export_name: export_name.to_string(),
            function_index,
            detail: format!(
                "local {} is out of range for {} locals",
                local_index, local_count
            ),
        });
    }
    if local_index as usize > local_limit {
        return Err(
            TassadarWasmModuleArtifactBundleError::UnsupportedLocalIndex {
                export_name: export_name.to_string(),
                function_index,
                local_index,
                max_supported: u8::MAX,
            },
        );
    }
    Ok(())
}

fn pop_stack_value(
    export_name: &str,
    function_index: u32,
    stack: &mut Vec<PendingValue>,
    opcode: &str,
) -> Result<PendingValue, TassadarWasmModuleArtifactBundleError> {
    stack.pop().ok_or_else(
        || TassadarWasmModuleArtifactBundleError::InvalidStackState {
            export_name: export_name.to_string(),
            function_index,
            detail: format!("stack underflow while lowering `{opcode}`"),
        },
    )
}

fn materialize_value(
    export_name: &str,
    function_index: u32,
    value: PendingValue,
    instructions: &mut Vec<TassadarInstruction>,
    local_count: usize,
) -> Result<(), TassadarWasmModuleArtifactBundleError> {
    match value {
        PendingValue::Const(value) => instructions.push(TassadarInstruction::I32Const { value }),
        PendingValue::Local(local_index) => {
            validate_local_index(export_name, function_index, local_index, local_count)?;
            instructions.push(TassadarInstruction::LocalGet {
                local: u8::try_from(local_index).expect("validated local index should fit in u8"),
            });
        }
        PendingValue::StackValue => {}
    }
    Ok(())
}

fn resolve_memory_slot(
    export_name: &str,
    function_index: u32,
    opcode: &str,
    address: PendingValue,
    offset: u64,
    memory_index: u32,
) -> Result<u8, TassadarWasmModuleArtifactBundleError> {
    if memory_index != 0 {
        return Err(
            TassadarWasmModuleArtifactBundleError::UnsupportedMemoryImmediate {
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
                TassadarWasmModuleArtifactBundleError::UnsupportedMemoryImmediate {
                    export_name: export_name.to_string(),
                    function_index,
                    opcode: opcode.to_string(),
                    detail: format!("negative address {value}"),
                },
            );
        }
        _ => {
            return Err(
                TassadarWasmModuleArtifactBundleError::UnsupportedDynamicMemoryAddress {
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
            TassadarWasmModuleArtifactBundleError::UnsupportedMemoryImmediate {
                export_name: export_name.to_string(),
                function_index,
                opcode: opcode.to_string(),
                detail: format!("absolute byte address {absolute} is not word aligned"),
            },
        );
    }
    let slot = absolute / 4;
    u8::try_from(slot).map_err(|_| {
        TassadarWasmModuleArtifactBundleError::UnsupportedMemoryImmediate {
            export_name: export_name.to_string(),
            function_index,
            opcode: opcode.to_string(),
            detail: format!("slot {slot} exceeds u8 runtime address space"),
        }
    })
}

fn emit_return(
    export_name: &str,
    function_index: u32,
    results: &[TassadarNormalizedWasmValueType],
    stack: &mut Vec<PendingValue>,
    runtime_instructions: &mut Vec<TassadarInstruction>,
    local_count: usize,
) -> Result<(), TassadarWasmModuleArtifactBundleError> {
    match results {
        [] => {
            if !stack.is_empty() {
                return Err(TassadarWasmModuleArtifactBundleError::InvalidStackState {
                    export_name: export_name.to_string(),
                    function_index,
                    detail: format!(
                        "implicit or explicit void return left {} values on the stack",
                        stack.len()
                    ),
                });
            }
        }
        [TassadarNormalizedWasmValueType::I32] => {
            let result = pop_stack_value(export_name, function_index, stack, "return")?;
            materialize_value(
                export_name,
                function_index,
                result,
                runtime_instructions,
                local_count,
            )?;
            if !stack.is_empty() {
                return Err(TassadarWasmModuleArtifactBundleError::InvalidStackState {
                    export_name: export_name.to_string(),
                    function_index,
                    detail: format!(
                        "return left {} extra values below the final result",
                        stack.len()
                    ),
                });
            }
            runtime_instructions.push(TassadarInstruction::Output);
        }
        other => {
            return Err(
                TassadarWasmModuleArtifactBundleError::UnsupportedResultTypes {
                    export_name: export_name.to_string(),
                    function_index,
                    result_types: other.iter().map(|value| format!("{value:?}")).collect(),
                },
            );
        }
    }
    runtime_instructions.push(TassadarInstruction::Return);
    Ok(())
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

fn canonical_repo_relative_path(path: &Path) -> String {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root should resolve from psionic-compiler crate dir");
    let canonical_path = path.canonicalize().unwrap_or_else(|_| repo_root.join(path));
    canonical_path
        .strip_prefix(&repo_root)
        .unwrap_or(&canonical_path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use psionic_ir::{
        TassadarNormalizedWasmMemory, TassadarNormalizedWasmMemoryType,
        encode_tassadar_normalized_wasm_module, tassadar_seeded_dynamic_memory_module,
        tassadar_seeded_instantiation_module, tassadar_seeded_multi_function_module,
    };
    use psionic_runtime::{
        TassadarCToWasmCompileConfig, TassadarCompileRefusal, TassadarCpuReferenceRunner,
        TassadarWasmProfile, execute_tassadar_module_execution_program,
        tassadar_canonical_c_source_path, tassadar_canonical_wasm_binary_path,
    };

    use super::{
        TassadarCSourceArtifactBundlePipelineError, TassadarWasmModuleArtifactBundle,
        TassadarWasmModuleArtifactBundleError, TassadarWasmModuleExecutionLoweringError,
        TassadarWasmTextArtifactBundlePipelineError, TassadarWasmTextCompileConfig,
        compile_tassadar_c_source_to_artifact_bundle,
        compile_tassadar_normalized_wasm_module_export_to_module_execution_program,
        compile_tassadar_normalized_wasm_module_to_artifact_bundle,
        compile_tassadar_wasm_binary_module_to_artifact_bundle,
        compile_tassadar_wasm_text_to_artifact_bundle, tassadar_compiler_frozen_core_wasm_window,
        tassadar_frozen_core_wasm_binary_fixture_refs,
        tassadar_frozen_core_wasm_negative_validation_fixtures,
        tassadar_frozen_core_wasm_text_fixture_refs,
    };

    #[test]
    fn wasm_module_bundle_refuses_parametrized_canonical_micro_kernel() {
        let wasm_bytes =
            std::fs::read(tassadar_canonical_wasm_binary_path()).expect("canonical wasm binary");
        let error = compile_tassadar_wasm_binary_module_to_artifact_bundle(
            "fixtures/tassadar/wasm/tassadar_micro_wasm_kernel.wasm",
            &wasm_bytes,
            &TassadarWasmProfile::article_i32_compute_v1(),
        )
        .expect_err("canonical kernel should stay outside zero-parameter lowering");
        assert!(
            matches!(
                error,
                TassadarWasmModuleArtifactBundleError::UnsupportedParamCount { .. }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn wasm_module_bundle_lowers_multi_function_exports_and_matches_cpu_reference() {
        let module = tassadar_seeded_multi_function_module().expect("seeded module should build");
        let bundle = compile_tassadar_normalized_wasm_module_to_artifact_bundle(
            "seeded://tassadar/wasm/multi_function_v1",
            &module,
            &TassadarWasmProfile::article_i32_compute_v1(),
        )
        .expect("seeded module should lower");
        assert_eq!(bundle.lowered_exports.len(), 2);

        for artifact in &bundle.lowered_exports {
            let execution = TassadarCpuReferenceRunner::for_program(
                &artifact.program_artifact.validated_program,
            )
            .expect("lowered program should select a runner")
            .execute(&artifact.program_artifact.validated_program)
            .expect("lowered program should execute exactly");
            assert_eq!(
                execution.outputs,
                artifact.execution_manifest.expected_outputs
            );
            assert_eq!(
                execution.final_memory,
                artifact.execution_manifest.expected_final_memory
            );
        }
        assert_eq!(
            bundle
                .lowered_exports
                .iter()
                .map(|artifact| (
                    artifact.export_name.as_str(),
                    artifact.execution_manifest.expected_outputs.clone()
                ))
                .collect::<std::collections::BTreeMap<_, _>>(),
            std::collections::BTreeMap::from([("local_double", vec![14]), ("pair_sum", vec![5]),])
        );
    }

    #[test]
    fn wasm_binary_roundtrip_parse_and_lower_stays_machine_legible() {
        let module = tassadar_seeded_multi_function_module().expect("seeded module should build");
        let bytes =
            encode_tassadar_normalized_wasm_module(&module).expect("seeded module should encode");
        let bundle = compile_tassadar_wasm_binary_module_to_artifact_bundle(
            "seeded://tassadar/wasm/multi_function_v1",
            &bytes,
            &TassadarWasmProfile::article_i32_compute_v1(),
        )
        .expect("seeded bytes should lower");
        assert_eq!(bundle.normalized_module, module);
        assert_eq!(
            bundle.normalized_module.exported_function_names(),
            vec![String::from("pair_sum"), String::from("local_double")]
        );
    }

    #[test]
    fn wasm_module_execution_lowering_runs_instantiation_exactly() {
        let module = tassadar_seeded_instantiation_module().expect("seeded module should build");
        let program = compile_tassadar_normalized_wasm_module_export_to_module_execution_program(
            "seeded://tassadar/wasm/instantiation_v1",
            &module,
            "entry",
        )
        .expect("module-execution lowering should succeed");
        let execution =
            execute_tassadar_module_execution_program(&program).expect("lowered program executes");
        assert_eq!(execution.returned_value, Some(42));
        assert_eq!(execution.final_globals, vec![31]);
    }

    #[test]
    fn wasm_module_execution_lowering_runs_dynamic_memory_exactly() {
        let module = tassadar_seeded_dynamic_memory_module().expect("seeded module should build");
        let program = compile_tassadar_normalized_wasm_module_export_to_module_execution_program(
            "seeded://tassadar/wasm/dynamic_memory_v1",
            &module,
            "entry",
        )
        .expect("dynamic-memory module should lower");
        let execution =
            execute_tassadar_module_execution_program(&program).expect("lowered program executes");
        assert_eq!(execution.returned_value, Some(185_207_052));
        assert_eq!(execution.final_memory_pages, vec![2]);
    }

    #[test]
    fn wasm_module_execution_lowering_refuses_multi_memory_shape() {
        let mut module =
            tassadar_seeded_dynamic_memory_module().expect("seeded module should build");
        module.memories.push(TassadarNormalizedWasmMemory::defined(
            1,
            TassadarNormalizedWasmMemoryType {
                minimum_pages: 1,
                maximum_pages: Some(1),
                shared: false,
                memory64: false,
                page_size_log2: None,
            },
        ));
        let error = compile_tassadar_normalized_wasm_module_export_to_module_execution_program(
            "seeded://tassadar/wasm/multi_memory_v1",
            &module,
            "entry",
        )
        .expect_err("multi-memory module should refuse");
        assert!(matches!(
            error,
            TassadarWasmModuleExecutionLoweringError::UnsupportedMemory { .. }
        ));
    }

    #[test]
    fn wasm_text_pipeline_lowers_multi_export_fixture_exactly() {
        let source_path = fixture_source_path("tassadar_multi_export_kernel.wat");
        let source_bytes = std::fs::read(&source_path).expect("multi-export fixture should exist");
        let source_text =
            std::str::from_utf8(&source_bytes).expect("Wasm-text fixture should be valid UTF-8");
        let pipeline = compile_tassadar_wasm_text_to_artifact_bundle(
            source_path.display().to_string(),
            source_text,
            temp_test_wasm_path("multi-export"),
            &TassadarWasmTextCompileConfig::canonical_multi_export_kernel(),
            &TassadarWasmProfile::article_i32_compute_v1(),
        )
        .expect("multi-export Wasm-text fixture should lower exactly");
        assert_eq!(
            exact_outputs_by_export(&pipeline.artifact_bundle),
            std::collections::BTreeMap::from([("local_double", vec![14]), ("pair_sum", vec![5]),])
        );
        assert_eq!(
            pipeline
                .compile_receipt
                .wasm_binary_summary()
                .expect("successful compile should publish a Wasm summary")
                .exported_functions,
            vec![String::from("local_double"), String::from("pair_sum")]
        );
    }

    #[test]
    fn wasm_text_pipeline_lowers_memory_lookup_fixture_exactly() {
        let source_path = fixture_source_path("tassadar_memory_lookup_kernel.wat");
        let source_bytes = std::fs::read(&source_path).expect("memory-lookup fixture should exist");
        let source_text =
            std::str::from_utf8(&source_bytes).expect("Wasm-text fixture should be valid UTF-8");
        let pipeline = compile_tassadar_wasm_text_to_artifact_bundle(
            source_path.display().to_string(),
            source_text,
            temp_test_wasm_path("memory-lookup"),
            &TassadarWasmTextCompileConfig::canonical_memory_lookup_kernel(),
            &TassadarWasmProfile::article_i32_compute_v1(),
        )
        .expect("memory-lookup Wasm-text fixture should lower exactly");
        assert_eq!(
            exact_outputs_by_export(&pipeline.artifact_bundle),
            std::collections::BTreeMap::from([
                ("load_edge_sum", vec![34]),
                ("load_middle", vec![19]),
            ])
        );
        assert_eq!(
            pipeline
                .compile_receipt
                .wasm_binary_summary()
                .expect("successful compile should publish a Wasm summary")
                .memory_count,
            1
        );
    }

    #[test]
    fn wasm_text_pipeline_refuses_param_abi_fixture_explicitly() {
        let source_path = fixture_source_path("tassadar_param_abi_kernel.wat");
        let source_bytes = std::fs::read(&source_path).expect("param-ABI fixture should exist");
        let source_text =
            std::str::from_utf8(&source_bytes).expect("Wasm-text fixture should be valid UTF-8");
        let error = compile_tassadar_wasm_text_to_artifact_bundle(
            source_path.display().to_string(),
            source_text,
            temp_test_wasm_path("param-abi"),
            &TassadarWasmTextCompileConfig::canonical_param_abi_kernel(),
            &TassadarWasmProfile::article_i32_compute_v1(),
        )
        .expect_err("parameterized Wasm-text fixture should refuse current lowering");
        assert!(matches!(
            error,
            TassadarWasmTextArtifactBundlePipelineError::LoweringRefused {
                error: TassadarWasmModuleArtifactBundleError::UnsupportedParamCount { .. },
                ..
            }
        ));
    }

    #[test]
    fn c_source_pipeline_surfaces_toolchain_unavailable_receipt() {
        let source_path = tassadar_canonical_c_source_path();
        let source_bytes = std::fs::read(&source_path).expect("multi-export fixture should exist");
        let mut config = TassadarCToWasmCompileConfig::canonical_micro_wasm_kernel();
        config.compiler_binary = String::from("clang-not-installed-for-tassadar");
        let error = compile_tassadar_c_source_to_artifact_bundle(
            source_path.display().to_string(),
            &source_bytes,
            temp_test_wasm_path("missing-toolchain"),
            &config,
            &TassadarWasmProfile::article_i32_compute_v1(),
        )
        .expect_err("missing toolchain should refuse before lowering");
        assert!(matches!(
            error,
            TassadarCSourceArtifactBundlePipelineError::CompileRefused {
                refusal: TassadarCompileRefusal::ToolchainUnavailable { .. },
                ..
            }
        ));
    }

    #[test]
    fn frozen_core_wasm_harness_imports_shared_window_contract() {
        let window = tassadar_compiler_frozen_core_wasm_window();
        assert_eq!(window.window_id, "tassadar.frozen_core_wasm.window.v1");
        assert_eq!(
            tassadar_frozen_core_wasm_text_fixture_refs(),
            vec![
                String::from("fixtures/tassadar/sources/tassadar_memory_lookup_kernel.wat"),
                String::from("fixtures/tassadar/sources/tassadar_multi_export_kernel.wat"),
                String::from("fixtures/tassadar/sources/tassadar_param_abi_kernel.wat"),
            ]
        );
        assert_eq!(
            tassadar_frozen_core_wasm_binary_fixture_refs(),
            vec![
                String::from("fixtures/tassadar/wasm/tassadar_memory_lookup_kernel.wasm"),
                String::from("fixtures/tassadar/wasm/tassadar_multi_export_kernel.wasm"),
                String::from("fixtures/tassadar/wasm/tassadar_param_abi_kernel.wasm"),
            ]
        );
    }

    #[test]
    fn frozen_core_wasm_negative_validation_fixtures_are_seeded_and_machine_legible() {
        let fixtures = tassadar_frozen_core_wasm_negative_validation_fixtures();
        assert_eq!(fixtures.len(), 2);
        assert!(fixtures.iter().any(|fixture| fixture.case_id
            == "proposal.float_value_type_refused"
            && fixture.proposal_family_id == "floating_point"
            && !fixture.wasm_binary.is_empty()));
        assert!(
            fixtures
                .iter()
                .any(|fixture| fixture.case_id == "proposal.multi_memory_refused"
                    && fixture.proposal_family_id == "multi_memory"
                    && !fixture.wasm_binary.is_empty())
        );
    }

    fn exact_outputs_by_export(
        bundle: &TassadarWasmModuleArtifactBundle,
    ) -> std::collections::BTreeMap<&str, Vec<i32>> {
        bundle
            .lowered_exports
            .iter()
            .map(|artifact| {
                let execution = TassadarCpuReferenceRunner::for_program(
                    &artifact.program_artifact.validated_program,
                )
                .expect("lowered program should select a runner")
                .execute(&artifact.program_artifact.validated_program)
                .expect("lowered program should execute exactly");
                (artifact.export_name.as_str(), execution.outputs)
            })
            .collect()
    }

    fn temp_test_wasm_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "psionic-tassadar-compiler-{label}-{}.wasm",
            std::process::id()
        ))
    }

    fn fixture_source_path(file_name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("tassadar")
            .join("sources")
            .join(file_name)
    }
}
