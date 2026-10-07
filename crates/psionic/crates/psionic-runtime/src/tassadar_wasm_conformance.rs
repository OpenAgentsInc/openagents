use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use wasmi::{Engine, Error as WasmiError, Linker, Module, Store, Val};

use crate::{
    TassadarHostImportStub, TassadarModuleElementSegment, TassadarModuleExecutionError,
    TassadarModuleExecutionProgram, TassadarModuleGlobalMutability, TassadarModuleInstruction,
    TassadarModuleTable, TassadarStructuredControlBinaryOp,
    execute_tassadar_module_execution_program, tassadar_seeded_module_call_indirect_program,
    tassadar_seeded_module_deterministic_import_program,
    tassadar_seeded_module_global_state_program, tassadar_seeded_module_instantiation_program,
    tassadar_seeded_module_unsupported_host_import_program,
};

/// Stable reference-authority identifier for the bounded module conformance lane.
pub const TASSADAR_WASM_REFERENCE_AUTHORITY_ID: &str = "wasmi.reference.v1";
/// Stable deterministic seed for generated module conformance cases.
pub const TASSADAR_WASM_CONFORMANCE_GENERATOR_SEED: u64 = 0x5441_535F_3032_32;

/// Stable origin for one conformance case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWasmConformanceCaseOrigin {
    /// Hand-authored seeded case.
    Curated,
    /// Deterministically generated case.
    Generated,
}

/// One bounded module-execution conformance case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmConformanceCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable family identifier.
    pub family_id: String,
    /// Curated or generated origin.
    pub origin: TassadarWasmConformanceCaseOrigin,
    /// Bounded module-execution program under test.
    pub program: TassadarModuleExecutionProgram,
}

/// Runtime terminal kind observed by the bounded module lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRuntimeTerminalKind {
    /// Execution returned successfully.
    Returned,
    /// Execution refused or trapped through the runtime-owned error surface.
    Errored,
}

/// Reference-authority terminal kind observed by Wasmi.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarReferenceTerminalKind {
    /// The reference authority returned successfully.
    Returned,
    /// The reference authority trapped.
    Trapped,
}

/// Differential status for one module-execution conformance case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleDifferentialStatus {
    /// Runtime and reference authority both returned and matched exactly.
    ExactSuccess,
    /// Runtime errored with trap-equivalent behavior and the reference trapped.
    ExactTrapParity,
    /// Runtime surfaced an explicit boundary refusal and the reference trapped.
    BoundaryRefusal,
    /// Runtime and reference authority diverged.
    Drift,
}

/// Reference-authority execution outcome for one translated module.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmReferenceExecution {
    /// Stable reference-authority identifier.
    pub authority_id: String,
    /// Stable source digest over the translated reference WAT module.
    pub reference_module_source_digest: String,
    /// Returned or trapped terminal kind.
    pub terminal_kind: TassadarReferenceTerminalKind,
    /// Optional returned value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
    /// Final globals exported by the translated module.
    pub final_globals: Vec<i32>,
    /// Trap detail when the authority trapped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Differential result between the bounded runtime lane and the reference authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleExecutionDifferentialResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable family identifier.
    pub family_id: String,
    /// Curated or generated origin.
    pub origin: TassadarWasmConformanceCaseOrigin,
    /// Exact success, trap parity, explicit boundary refusal, or drift.
    pub status: TassadarModuleDifferentialStatus,
    /// Stable digest over the bounded program payload.
    pub program_digest: String,
    /// Stable source digest over the translated reference WAT module.
    pub reference_module_source_digest: String,
    /// Runtime returned or errored.
    pub runtime_terminal_kind: TassadarRuntimeTerminalKind,
    /// Returned value when the runtime succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_returned_value: Option<i32>,
    /// Final globals when the runtime succeeded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_final_globals: Vec<i32>,
    /// Stable execution digest when the runtime succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_execution_digest: Option<String>,
    /// Runtime error kind when the runtime errored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_error_kind: Option<String>,
    /// Runtime error detail when the runtime errored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_error_detail: Option<String>,
    /// Reference-authority execution outcome.
    pub reference_execution: TassadarWasmReferenceExecution,
}

/// Reference-Wasm translation or authority failures.
#[derive(Debug, Error)]
pub enum TassadarWasmConformanceError {
    /// The bounded module program could not be translated into the reference Wasm subset.
    #[error(transparent)]
    Translation(#[from] TassadarWasmReferenceTranslationError),
    /// The reference authority rejected the translated module before execution.
    #[error(
        "reference authority failed to compile translated module for case `{case_id}`: {error}"
    )]
    ReferenceModule {
        /// Case identifier.
        case_id: String,
        /// Authority error.
        error: WasmiError,
    },
    /// The reference authority omitted the exported entry function.
    #[error("reference authority omitted exported entry function for case `{case_id}`")]
    MissingReferenceEntry {
        /// Case identifier.
        case_id: String,
    },
    /// The reference authority omitted one exported global.
    #[error("reference authority omitted exported global `{export_name}` for case `{case_id}`")]
    MissingReferenceGlobal {
        /// Case identifier.
        case_id: String,
        /// Export name.
        export_name: String,
    },
    /// The reference authority exported one global with the wrong type.
    #[error("reference authority exported non-i32 global `{export_name}` for case `{case_id}`")]
    ReferenceGlobalType {
        /// Case identifier.
        case_id: String,
        /// Export name.
        export_name: String,
    },
}

/// Translation failures from the bounded module lane into the reference Wasm subset.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarWasmReferenceTranslationError {
    /// The bounded program failed validation before reference translation.
    #[error("reference Wasm translation requires a valid bounded module program: {detail}")]
    ProgramInvalid {
        /// Validation failure detail.
        detail: String,
    },
    /// One table used sparse entries that the current reference translator does not encode.
    #[error(
        "reference Wasm translation does not yet encode sparse table `{table_index}` with empty entries"
    )]
    SparseTableUnsupported {
        /// Table index.
        table_index: u32,
    },
    /// One table mixed functions with different result counts.
    #[error(
        "reference Wasm translation requires one stable result count for table `{table_index}`"
    )]
    MixedTableResultCount {
        /// Table index.
        table_index: u32,
    },
    /// One function declared an unsupported result count.
    #[error("reference Wasm translation requires result_count <= 1 for function `{function_name}`")]
    UnsupportedResultCount {
        /// Function name.
        function_name: String,
        /// Declared result count.
        result_count: u8,
    },
}

impl TassadarModuleExecutionError {
    fn conformance_kind_slug(&self) -> &'static str {
        match self {
            Self::NoFunctions => "no_functions",
            Self::MissingEntryFunction { .. } => "missing_entry_function",
            Self::EntryFunctionHasParameters { .. } => "entry_function_has_parameters",
            Self::FunctionIndexDrift { .. } => "function_index_drift",
            Self::GlobalIndexDrift { .. } => "global_index_drift",
            Self::TableIndexDrift { .. } => "table_index_drift",
            Self::ImportIndexDrift { .. } => "import_index_drift",
            Self::UnsupportedMemoryCount { .. } => "unsupported_memory_count",
            Self::MemoryIndexDrift { .. } => "memory_index_drift",
            Self::ElementSegmentIndexDrift { .. } => "element_segment_index_drift",
            Self::DataSegmentIndexDrift { .. } => "data_segment_index_drift",
            Self::UnsupportedParamCount { .. } => "unsupported_param_count",
            Self::UnsupportedResultCount { .. } => "unsupported_result_count",
            Self::LocalCountTooSmall { .. } => "local_count_too_small",
            Self::LocalOutOfRange { .. } => "local_out_of_range",
            Self::UnsupportedGlobalValueType { .. } => "unsupported_global_value_type",
            Self::GlobalOutOfRange { .. } => "global_out_of_range",
            Self::MaxPagesBeforeInitial { .. } => "max_pages_before_initial",
            Self::MemoryPageLimitExceeded { .. } => "memory_page_limit_exceeded",
            Self::MemoryOutOfRange { .. } => "memory_out_of_range",
            Self::MissingStartFunction { .. } => "missing_start_function",
            Self::UnsupportedStartFunctionSignature { .. } => {
                "unsupported_start_function_signature"
            }
            Self::ImmutableGlobalWrite { .. } => "immutable_global_write",
            Self::TableOutOfRange { .. } => "table_out_of_range",
            Self::TableBelowMinimum { .. } => "table_below_minimum",
            Self::TableAboveMaximum { .. } => "table_above_maximum",
            Self::TableFunctionOutOfRange { .. } => "table_function_out_of_range",
            Self::ElementSegmentTableOutOfRange { .. } => "element_segment_table_out_of_range",
            Self::ElementSegmentOutOfRange { .. } => "element_segment_out_of_range",
            Self::ElementSegmentFunctionOutOfRange { .. } => {
                "element_segment_function_out_of_range"
            }
            Self::DataSegmentMemoryOutOfRange { .. } => "data_segment_memory_out_of_range",
            Self::DataSegmentOutOfRange { .. } => "data_segment_out_of_range",
            Self::DirectCallFunctionOutOfRange { .. } => "direct_call_function_out_of_range",
            Self::TableSelectorOutOfRange { .. } => "table_selector_out_of_range",
            Self::EmptyTableEntry { .. } => "empty_table_entry",
            Self::MemoryCopyMemoryOutOfRange { .. } => "memory_copy_memory_out_of_range",
            Self::ImportOutOfRange { .. } => "import_out_of_range",
            Self::MissingImportRef { .. } => "missing_import_ref",
            Self::UnsupportedHostImport { .. } => "unsupported_host_import",
            Self::StackUnderflow { .. } => "stack_underflow",
            Self::MemoryAddressOutOfRange { .. } => "memory_address_out_of_range",
            Self::MemoryCopyOutOfRange { .. } => "memory_copy_out_of_range",
            Self::MemoryFillOutOfRange { .. } => "memory_fill_out_of_range",
            Self::StepLimitExceeded { .. } => "step_limit_exceeded",
            Self::CallDepthExceeded { .. } => "call_depth_exceeded",
            Self::ByteLengthOverflow => "byte_length_overflow",
        }
    }
}

/// Runs one bounded module-execution program against the runtime lane and the reference authority.
pub fn run_tassadar_module_execution_differential(
    case: &TassadarWasmConformanceCase,
) -> Result<TassadarModuleExecutionDifferentialResult, TassadarWasmConformanceError> {
    let reference_module_source =
        translate_tassadar_module_execution_program_to_wat(&case.program)?;
    let reference_module_source_digest = stable_bytes_digest(reference_module_source.as_bytes());
    let reference_execution = execute_reference_module(
        &case.case_id,
        &case.program,
        &reference_module_source,
        &reference_module_source_digest,
    )?;
    let runtime_execution = execute_tassadar_module_execution_program(&case.program);
    let program_digest = stable_serialized_digest(&case.program);

    let result = match runtime_execution {
        Ok(runtime_execution) => {
            let status = if reference_execution.terminal_kind
                == TassadarReferenceTerminalKind::Returned
                && runtime_execution.returned_value == reference_execution.returned_value
                && runtime_execution.final_globals == reference_execution.final_globals
            {
                TassadarModuleDifferentialStatus::ExactSuccess
            } else {
                TassadarModuleDifferentialStatus::Drift
            };
            TassadarModuleExecutionDifferentialResult {
                case_id: case.case_id.clone(),
                family_id: case.family_id.clone(),
                origin: case.origin,
                status,
                program_digest,
                reference_module_source_digest,
                runtime_terminal_kind: TassadarRuntimeTerminalKind::Returned,
                runtime_returned_value: runtime_execution.returned_value,
                runtime_final_globals: runtime_execution.final_globals.clone(),
                runtime_execution_digest: Some(runtime_execution.execution_digest()),
                runtime_error_kind: None,
                runtime_error_detail: None,
                reference_execution,
            }
        }
        Err(error) => {
            let status = match (&error, reference_execution.terminal_kind) {
                (
                    TassadarModuleExecutionError::UnsupportedHostImport { .. },
                    TassadarReferenceTerminalKind::Trapped,
                ) => TassadarModuleDifferentialStatus::BoundaryRefusal,
                (_, TassadarReferenceTerminalKind::Trapped) => {
                    TassadarModuleDifferentialStatus::ExactTrapParity
                }
                _ => TassadarModuleDifferentialStatus::Drift,
            };
            TassadarModuleExecutionDifferentialResult {
                case_id: case.case_id.clone(),
                family_id: case.family_id.clone(),
                origin: case.origin,
                status,
                program_digest,
                reference_module_source_digest,
                runtime_terminal_kind: TassadarRuntimeTerminalKind::Errored,
                runtime_returned_value: None,
                runtime_final_globals: Vec::new(),
                runtime_execution_digest: None,
                runtime_error_kind: Some(String::from(error.conformance_kind_slug())),
                runtime_error_detail: Some(error.to_string()),
                reference_execution,
            }
        }
    };
    Ok(result)
}

/// Returns the curated bounded module conformance cases for the current lane.
#[must_use]
pub fn tassadar_curated_wasm_conformance_cases() -> Vec<TassadarWasmConformanceCase> {
    vec![
        TassadarWasmConformanceCase {
            case_id: String::from("module_global_state_exact"),
            family_id: String::from("curated.global_state"),
            origin: TassadarWasmConformanceCaseOrigin::Curated,
            program: tassadar_seeded_module_global_state_program(),
        },
        TassadarWasmConformanceCase {
            case_id: String::from("module_call_indirect_exact"),
            family_id: String::from("curated.call_indirect"),
            origin: TassadarWasmConformanceCaseOrigin::Curated,
            program: tassadar_seeded_module_call_indirect_program(),
        },
        TassadarWasmConformanceCase {
            case_id: String::from("module_instantiation_exact"),
            family_id: String::from("curated.instantiation"),
            origin: TassadarWasmConformanceCaseOrigin::Curated,
            program: tassadar_seeded_module_instantiation_program(),
        },
        TassadarWasmConformanceCase {
            case_id: String::from("module_deterministic_import_exact"),
            family_id: String::from("curated.deterministic_import"),
            origin: TassadarWasmConformanceCaseOrigin::Curated,
            program: tassadar_seeded_module_deterministic_import_program(),
        },
        TassadarWasmConformanceCase {
            case_id: String::from("module_call_indirect_selector_trap"),
            family_id: String::from("curated.call_indirect_trap"),
            origin: TassadarWasmConformanceCaseOrigin::Curated,
            program: tassadar_seeded_module_call_indirect_selector_trap_program(),
        },
        TassadarWasmConformanceCase {
            case_id: String::from("module_unsupported_host_boundary"),
            family_id: String::from("curated.unsupported_host_import"),
            origin: TassadarWasmConformanceCaseOrigin::Curated,
            program: tassadar_seeded_module_unsupported_host_import_program(),
        },
    ]
}

/// Returns a deterministic generated conformance suite within the current bounded subset.
#[must_use]
pub fn tassadar_generated_wasm_conformance_cases(
    seed: u64,
    exact_case_count: usize,
    trap_case_count: usize,
) -> Vec<TassadarWasmConformanceCase> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut cases = Vec::new();
    for index in 0..exact_case_count {
        let selector = if rng.random_bool(0.5) { 0 } else { 1 };
        let function_zero = rng.random_range(9..90);
        let function_one = rng.random_range(91..190);
        let delta = rng.random_range(-16..=16);
        let initial = rng.random_range(-16..=16);
        let use_table = rng.random_bool(0.5);
        let program = if use_table {
            generated_call_indirect_program(
                format!("tassadar.module_execution.generated_call_indirect_exact.{index}.v1"),
                selector,
                function_zero,
                function_one,
            )
        } else {
            generated_global_state_program(
                format!("tassadar.module_execution.generated_global_state_exact.{index}.v1"),
                initial,
                delta,
                generated_global_op(&mut rng),
            )
        };
        cases.push(TassadarWasmConformanceCase {
            case_id: format!("generated_exact_{index}"),
            family_id: if use_table {
                String::from("generated.call_indirect")
            } else {
                String::from("generated.global_state")
            },
            origin: TassadarWasmConformanceCaseOrigin::Generated,
            program,
        });
    }
    for index in 0..trap_case_count {
        let selector = rng.random_range(2..6);
        let function_zero = rng.random_range(17..70);
        let function_one = rng.random_range(71..140);
        cases.push(TassadarWasmConformanceCase {
            case_id: format!("generated_trap_{index}"),
            family_id: String::from("generated.call_indirect_trap"),
            origin: TassadarWasmConformanceCaseOrigin::Generated,
            program: generated_call_indirect_program(
                format!("tassadar.module_execution.generated_call_indirect_trap.{index}.v1"),
                selector,
                function_zero,
                function_one,
            ),
        });
    }
    cases
}

/// Returns one seeded out-of-range indirect-call trap program.
#[must_use]
pub fn tassadar_seeded_module_call_indirect_selector_trap_program() -> TassadarModuleExecutionProgram
{
    generated_call_indirect_program(
        "tassadar.module_execution.call_indirect_selector_trap.v1",
        2,
        111,
        222,
    )
}

fn execute_reference_module(
    case_id: &str,
    program: &TassadarModuleExecutionProgram,
    wat_source: &str,
    reference_module_source_digest: &str,
) -> Result<TassadarWasmReferenceExecution, TassadarWasmConformanceError> {
    let engine = Engine::default();
    let module = Module::new(&engine, wat_source).map_err(|error| {
        TassadarWasmConformanceError::ReferenceModule {
            case_id: String::from(case_id),
            error,
        }
    })?;
    let mut store = Store::new(&engine, ());
    let linker = Linker::<()>::new(&engine);
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|error| TassadarWasmConformanceError::ReferenceModule {
            case_id: String::from(case_id),
            error,
        })?;

    let terminal = if entry_result_count(program) == 0 {
        instance
            .get_typed_func::<(), ()>(&store, "entry")
            .map_err(|_| TassadarWasmConformanceError::MissingReferenceEntry {
                case_id: String::from(case_id),
            })?
            .call(&mut store, ())
            .map(|()| None)
    } else {
        instance
            .get_typed_func::<(), i32>(&store, "entry")
            .map_err(|_| TassadarWasmConformanceError::MissingReferenceEntry {
                case_id: String::from(case_id),
            })?
            .call(&mut store, ())
            .map(Some)
    };

    let final_globals =
        exported_reference_globals(case_id, &instance, &store, program.globals.len())?;
    let outcome = match terminal {
        Ok(returned_value) => TassadarWasmReferenceExecution {
            authority_id: String::from(TASSADAR_WASM_REFERENCE_AUTHORITY_ID),
            reference_module_source_digest: String::from(reference_module_source_digest),
            terminal_kind: TassadarReferenceTerminalKind::Returned,
            returned_value,
            final_globals,
            detail: None,
        },
        Err(error) => TassadarWasmReferenceExecution {
            authority_id: String::from(TASSADAR_WASM_REFERENCE_AUTHORITY_ID),
            reference_module_source_digest: String::from(reference_module_source_digest),
            terminal_kind: TassadarReferenceTerminalKind::Trapped,
            returned_value: None,
            final_globals,
            detail: Some(error.to_string()),
        },
    };
    Ok(outcome)
}

fn exported_reference_globals(
    case_id: &str,
    instance: &wasmi::Instance,
    store: &Store<()>,
    global_count: usize,
) -> Result<Vec<i32>, TassadarWasmConformanceError> {
    let mut globals = Vec::with_capacity(global_count);
    for index in 0..global_count {
        let export_name = format!("global_{index}");
        let global = instance.get_global(store, &export_name).ok_or_else(|| {
            TassadarWasmConformanceError::MissingReferenceGlobal {
                case_id: String::from(case_id),
                export_name: export_name.clone(),
            }
        })?;
        let value = match global.get(store) {
            Val::I32(value) => value,
            _ => {
                return Err(TassadarWasmConformanceError::ReferenceGlobalType {
                    case_id: String::from(case_id),
                    export_name,
                });
            }
        };
        globals.push(value);
    }
    Ok(globals)
}

fn translate_tassadar_module_execution_program_to_wat(
    program: &TassadarModuleExecutionProgram,
) -> Result<String, TassadarWasmReferenceTranslationError> {
    program.validate().map_err(
        |error| TassadarWasmReferenceTranslationError::ProgramInvalid {
            detail: error.to_string(),
        },
    )?;

    let mut lines = vec![String::from("(module")];
    lines.push(String::from("  (type $ret0 (func))"));
    lines.push(String::from("  (type $ret1 (func (result i32)))"));

    for global in &program.globals {
        let mutability = match global.mutability {
            TassadarModuleGlobalMutability::Const => String::from("i32"),
            TassadarModuleGlobalMutability::Mutable => String::from("(mut i32)"),
        };
        lines.push(format!(
            "  (global $g{} {} (i32.const {}))",
            global.global_index, mutability, global.initial_value
        ));
        lines.push(format!(
            "  (export \"global_{}\" (global $g{}))",
            global.global_index, global.global_index
        ));
    }

    for memory in &program.memories {
        lines.push(format!(
            "  (memory $mem{} {} {})",
            memory.memory_index, memory.initial_pages, memory.max_pages
        ));
    }

    for data_segment in &program.data_segments {
        lines.push(format!(
            "  (data (memory $mem{}) (i32.const {}) \"{}\")",
            data_segment.memory_index,
            data_segment.offset,
            render_wat_bytes(data_segment.bytes.as_slice())
        ));
    }

    for table in &program.tables {
        let instantiated_elements = instantiated_table_elements(program, table)?;
        for element in &instantiated_elements {
            if element.is_none() {
                return Err(
                    TassadarWasmReferenceTranslationError::SparseTableUnsupported {
                        table_index: table.table_index,
                    },
                );
            }
        }
        let max = table
            .max_entries
            .map_or(String::new(), |max_entries| format!(" {max_entries}"));
        lines.push(format!(
            "  (table $table{} {}{} funcref)",
            table.table_index, table.min_entries, max
        ));
        let functions = table
            .elements
            .iter()
            .enumerate()
            .map(|(index, _)| instantiated_elements[index])
            .flatten()
            .map(|function_index| format!("$f{function_index}"))
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!("  (elem (i32.const 0) {functions})"));
    }

    for stub in &program.imports {
        match stub {
            TassadarHostImportStub::DeterministicI32Const {
                import_index,
                value,
                ..
            } => {
                lines.push(format!(
                    "  (func $stub{} (type $ret1)\n    i32.const {}\n    return\n  )",
                    import_index, value
                ));
            }
            TassadarHostImportStub::UnsupportedHostCall { import_index, .. } => {
                lines.push(format!(
                    "  (func $stub{} (type $ret1)\n    unreachable\n  )",
                    import_index
                ));
            }
        }
    }

    for function in &program.functions {
        if function.result_count > 1 {
            return Err(
                TassadarWasmReferenceTranslationError::UnsupportedResultCount {
                    function_name: function.function_name.clone(),
                    result_count: function.result_count,
                },
            );
        }
        let export = if function.function_index == program.entry_function_index {
            " (export \"entry\")"
        } else {
            ""
        };
        let result = if function.result_count == 1 {
            " (result i32)"
        } else {
            ""
        };
        let locals = if function.local_count == 0 {
            String::new()
        } else {
            format!(" (local {})", vec!["i32"; function.local_count].join(" "))
        };
        lines.push(format!(
            "  (func $f{}{} (type ${}){}{}",
            function.function_index,
            export,
            result_type_id(function.result_count),
            result,
            locals
        ));
        for instruction in &function.instructions {
            lines.push(format!("    {}", render_instruction(program, instruction)?));
        }
        lines.push(String::from("  )"));
    }

    if let Some(start_function_index) = program.start_function_index {
        lines.push(format!("  (start $f{start_function_index})"));
    }

    lines.push(String::from(")"));
    Ok(lines.join("\n"))
}

fn render_instruction(
    program: &TassadarModuleExecutionProgram,
    instruction: &TassadarModuleInstruction,
) -> Result<String, TassadarWasmReferenceTranslationError> {
    match instruction {
        TassadarModuleInstruction::I32Const { value } => Ok(format!("i32.const {value}")),
        TassadarModuleInstruction::LocalGet { local_index } => {
            Ok(format!("local.get {local_index}"))
        }
        TassadarModuleInstruction::LocalSet { local_index } => {
            Ok(format!("local.set {local_index}"))
        }
        TassadarModuleInstruction::GlobalGet { global_index } => {
            Ok(format!("global.get $g{global_index}"))
        }
        TassadarModuleInstruction::GlobalSet { global_index } => {
            Ok(format!("global.set $g{global_index}"))
        }
        TassadarModuleInstruction::Drop => Ok(String::from("drop")),
        TassadarModuleInstruction::BinaryOp { op } => Ok(render_binary_op(*op).to_string()),
        TassadarModuleInstruction::I32Load {
            memory_index,
            offset,
        } => Ok(format!(
            "i32.load offset={} memory $mem{}",
            offset, memory_index
        )),
        TassadarModuleInstruction::I32Store {
            memory_index,
            offset,
        } => Ok(format!(
            "i32.store offset={} memory $mem{}",
            offset, memory_index
        )),
        TassadarModuleInstruction::MemorySize { memory_index } => {
            Ok(format!("memory.size $mem{memory_index}"))
        }
        TassadarModuleInstruction::MemoryGrow { memory_index } => {
            Ok(format!("memory.grow $mem{memory_index}"))
        }
        TassadarModuleInstruction::MemoryCopy {
            dst_memory_index,
            src_memory_index,
        } => Ok(format!(
            "memory.copy $mem{} $mem{}",
            dst_memory_index, src_memory_index
        )),
        TassadarModuleInstruction::MemoryFill { memory_index } => {
            Ok(format!("memory.fill $mem{memory_index}"))
        }
        TassadarModuleInstruction::Call { function_index } => {
            Ok(format!("call $f{function_index}"))
        }
        TassadarModuleInstruction::CallIndirect { table_index } => {
            let table = &program.tables[*table_index as usize];
            let result_count = table_result_count(program, table)?;
            Ok(format!(
                "call_indirect $table{} (type ${})",
                table_index,
                result_type_id(result_count)
            ))
        }
        TassadarModuleInstruction::HostCall { import_index } => {
            Ok(format!("call $stub{import_index}"))
        }
        TassadarModuleInstruction::Return => Ok(String::from("return")),
    }
}

fn render_wat_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("\\{:02x}", byte))
        .collect::<String>()
}

fn table_result_count(
    program: &TassadarModuleExecutionProgram,
    table: &TassadarModuleTable,
) -> Result<u8, TassadarWasmReferenceTranslationError> {
    let mut result_count = None;
    for function_index in instantiated_table_elements(program, table)?
        .into_iter()
        .flatten()
    {
        let function = &program.functions[function_index as usize];
        if function.result_count > 1 {
            return Err(
                TassadarWasmReferenceTranslationError::UnsupportedResultCount {
                    function_name: function.function_name.clone(),
                    result_count: function.result_count,
                },
            );
        }
        match result_count {
            Some(previous) if previous != function.result_count => {
                return Err(
                    TassadarWasmReferenceTranslationError::MixedTableResultCount {
                        table_index: table.table_index,
                    },
                );
            }
            Some(_) => {}
            None => result_count = Some(function.result_count),
        }
    }
    Ok(result_count.unwrap_or(0))
}

fn instantiated_table_elements(
    program: &TassadarModuleExecutionProgram,
    table: &TassadarModuleTable,
) -> Result<Vec<Option<u32>>, TassadarWasmReferenceTranslationError> {
    let mut elements = table.elements.clone();
    for TassadarModuleElementSegment {
        table_index,
        offset,
        elements: segment_elements,
        ..
    } in &program.element_segments
    {
        if *table_index != table.table_index {
            continue;
        }
        let start = *offset as usize;
        let end = start + segment_elements.len();
        if end > elements.len() {
            return Err(TassadarWasmReferenceTranslationError::ProgramInvalid {
                detail: format!(
                    "element segment writes beyond table {} during reference translation",
                    table.table_index
                ),
            });
        }
        for (slot, function_index) in segment_elements.iter().enumerate() {
            elements[start + slot] = *function_index;
        }
    }
    Ok(elements)
}

fn result_type_id(result_count: u8) -> &'static str {
    if result_count == 0 { "ret0" } else { "ret1" }
}

fn entry_result_count(program: &TassadarModuleExecutionProgram) -> u8 {
    program
        .functions
        .iter()
        .find(|function| function.function_index == program.entry_function_index)
        .map_or(0, |function| function.result_count)
}

fn render_binary_op(op: TassadarStructuredControlBinaryOp) -> &'static str {
    match op {
        TassadarStructuredControlBinaryOp::Add => "i32.add",
        TassadarStructuredControlBinaryOp::Sub => "i32.sub",
        TassadarStructuredControlBinaryOp::Mul => "i32.mul",
        TassadarStructuredControlBinaryOp::Eq => "i32.eq",
        TassadarStructuredControlBinaryOp::Ne => "i32.ne",
        TassadarStructuredControlBinaryOp::LtS => "i32.lt_s",
        TassadarStructuredControlBinaryOp::LtU => "i32.lt_u",
        TassadarStructuredControlBinaryOp::GtS => "i32.gt_s",
        TassadarStructuredControlBinaryOp::GtU => "i32.gt_u",
        TassadarStructuredControlBinaryOp::LeS => "i32.le_s",
        TassadarStructuredControlBinaryOp::LeU => "i32.le_u",
        TassadarStructuredControlBinaryOp::GeS => "i32.ge_s",
        TassadarStructuredControlBinaryOp::GeU => "i32.ge_u",
        TassadarStructuredControlBinaryOp::And => "i32.and",
        TassadarStructuredControlBinaryOp::Or => "i32.or",
        TassadarStructuredControlBinaryOp::Xor => "i32.xor",
        TassadarStructuredControlBinaryOp::Shl => "i32.shl",
        TassadarStructuredControlBinaryOp::ShrS => "i32.shr_s",
        TassadarStructuredControlBinaryOp::ShrU => "i32.shr_u",
    }
}

fn generated_global_op(rng: &mut StdRng) -> TassadarStructuredControlBinaryOp {
    match rng.random_range(0..5) {
        0 => TassadarStructuredControlBinaryOp::Add,
        1 => TassadarStructuredControlBinaryOp::Sub,
        2 => TassadarStructuredControlBinaryOp::Xor,
        3 => TassadarStructuredControlBinaryOp::Or,
        _ => TassadarStructuredControlBinaryOp::And,
    }
}

fn generated_global_state_program(
    program_id: impl Into<String>,
    initial_value: i32,
    delta: i32,
    op: TassadarStructuredControlBinaryOp,
) -> TassadarModuleExecutionProgram {
    TassadarModuleExecutionProgram::new(
        program_id,
        0,
        8,
        vec![crate::TassadarModuleGlobal {
            global_index: 0,
            value_type: crate::TassadarModuleValueType::I32,
            mutability: TassadarModuleGlobalMutability::Mutable,
            initial_value,
        }],
        Vec::new(),
        Vec::new(),
        vec![crate::TassadarModuleFunction::new(
            0,
            "entry",
            0,
            0,
            1,
            vec![
                TassadarModuleInstruction::GlobalGet { global_index: 0 },
                TassadarModuleInstruction::I32Const { value: delta },
                TassadarModuleInstruction::BinaryOp { op },
                TassadarModuleInstruction::GlobalSet { global_index: 0 },
                TassadarModuleInstruction::GlobalGet { global_index: 0 },
                TassadarModuleInstruction::Return,
            ],
        )],
    )
}

fn generated_call_indirect_program(
    program_id: impl Into<String>,
    selector: i32,
    slot_zero_value: i32,
    slot_one_value: i32,
) -> TassadarModuleExecutionProgram {
    TassadarModuleExecutionProgram::new(
        program_id,
        0,
        8,
        Vec::new(),
        vec![crate::TassadarModuleTable {
            table_index: 0,
            element_kind: crate::TassadarModuleTableElementKind::Funcref,
            min_entries: 2,
            max_entries: Some(2),
            elements: vec![Some(1), Some(2)],
        }],
        Vec::new(),
        vec![
            crate::TassadarModuleFunction::new(
                0,
                "entry",
                0,
                0,
                1,
                vec![
                    TassadarModuleInstruction::I32Const { value: selector },
                    TassadarModuleInstruction::CallIndirect { table_index: 0 },
                    TassadarModuleInstruction::Return,
                ],
            ),
            crate::TassadarModuleFunction::new(
                1,
                "slot_zero",
                0,
                0,
                1,
                vec![
                    TassadarModuleInstruction::I32Const {
                        value: slot_zero_value,
                    },
                    TassadarModuleInstruction::Return,
                ],
            ),
            crate::TassadarModuleFunction::new(
                2,
                "slot_one",
                0,
                0,
                1,
                vec![
                    TassadarModuleInstruction::I32Const {
                        value: slot_one_value,
                    },
                    TassadarModuleInstruction::Return,
                ],
            ),
        ],
    )
}

fn stable_bytes_digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stable_serialized_digest<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    stable_bytes_digest(&bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_WASM_CONFORMANCE_GENERATOR_SEED, TassadarModuleDifferentialStatus,
        run_tassadar_module_execution_differential, tassadar_curated_wasm_conformance_cases,
        tassadar_generated_wasm_conformance_cases,
        tassadar_seeded_module_call_indirect_selector_trap_program,
        translate_tassadar_module_execution_program_to_wat,
    };

    #[test]
    fn module_execution_reference_translation_includes_entry_export() {
        let wat = translate_tassadar_module_execution_program_to_wat(
            &tassadar_seeded_module_call_indirect_selector_trap_program(),
        )
        .expect("translation should succeed");
        assert!(wat.contains("(export \"entry\")"));
        assert!(wat.contains("call_indirect"));
    }

    #[test]
    fn module_execution_differential_matches_curated_exact_cases() {
        for case in tassadar_curated_wasm_conformance_cases()
            .into_iter()
            .filter(|case| {
                case.family_id == "curated.global_state"
                    || case.family_id == "curated.call_indirect"
                    || case.family_id == "curated.instantiation"
                    || case.family_id == "curated.deterministic_import"
            })
        {
            let result =
                run_tassadar_module_execution_differential(&case).expect("differential should run");
            assert_eq!(
                result.status,
                TassadarModuleDifferentialStatus::ExactSuccess
            );
        }
    }

    #[test]
    fn module_execution_differential_classifies_trap_and_boundary_cases() {
        for case in tassadar_curated_wasm_conformance_cases()
            .into_iter()
            .filter(|case| {
                case.family_id == "curated.call_indirect_trap"
                    || case.family_id == "curated.unsupported_host_import"
            })
        {
            let result =
                run_tassadar_module_execution_differential(&case).expect("differential should run");
            if case.family_id == "curated.call_indirect_trap" {
                assert_eq!(
                    result.status,
                    TassadarModuleDifferentialStatus::ExactTrapParity
                );
            } else {
                assert_eq!(
                    result.status,
                    TassadarModuleDifferentialStatus::BoundaryRefusal
                );
            }
        }
    }

    #[test]
    fn generated_module_execution_cases_are_reproducible() {
        let cases_a = tassadar_generated_wasm_conformance_cases(
            TASSADAR_WASM_CONFORMANCE_GENERATOR_SEED,
            4,
            2,
        );
        let cases_b = tassadar_generated_wasm_conformance_cases(
            TASSADAR_WASM_CONFORMANCE_GENERATOR_SEED,
            4,
            2,
        );
        assert_eq!(cases_a, cases_b);
    }
}
