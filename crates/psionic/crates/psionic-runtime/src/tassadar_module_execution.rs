use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_LINEAR_MEMORY_PAGE_BYTES, TASSADAR_RUNTIME_BACKEND_ID,
    TassadarStructuredControlBinaryOp,
};

const TASSADAR_MODULE_EXECUTION_MAX_STEPS: usize = 4_096;
const TASSADAR_MODULE_EXECUTION_MAX_MEMORY_PAGES: u32 = 8;

/// Value types admitted by the bounded module-execution lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleValueType {
    /// 32-bit integer values.
    I32,
}

/// Mutability posture for one bounded module global.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleGlobalMutability {
    /// Global cannot be written after initialization.
    Const,
    /// Global may be updated during execution.
    Mutable,
}

/// One global in the bounded module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleGlobal {
    /// Stable global index.
    pub global_index: u32,
    /// Visible value type for the global.
    pub value_type: TassadarModuleValueType,
    /// Mutability posture.
    pub mutability: TassadarModuleGlobalMutability,
    /// Initial i32 value.
    pub initial_value: i32,
}

/// Table element kind admitted by the bounded module-execution lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleTableElementKind {
    /// Function-reference table entries.
    Funcref,
}

/// One function table in the bounded module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTable {
    /// Stable table index.
    pub table_index: u32,
    /// Table element kind.
    pub element_kind: TassadarModuleTableElementKind,
    /// Minimum declared table size.
    pub min_entries: u32,
    /// Optional maximum declared table size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_entries: Option<u32>,
    /// Function indices stored in table order.
    pub elements: Vec<Option<u32>>,
}

/// One active element segment in the bounded module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleElementSegment {
    /// Stable segment index.
    pub element_segment_index: u32,
    /// Target table index.
    pub table_index: u32,
    /// Table offset at which elements are written.
    pub offset: u32,
    /// Function indices written into the table in order.
    pub elements: Vec<Option<u32>>,
}

/// One bounded linear memory admitted by the module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleMemory {
    /// Stable memory index.
    pub memory_index: u32,
    /// Initial size in Wasm pages.
    pub initial_pages: u32,
    /// Maximum admitted size in Wasm pages.
    pub max_pages: u32,
}

/// One active data segment admitted by the module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleDataSegment {
    /// Stable data-segment index.
    pub data_segment_index: u32,
    /// Target memory index.
    pub memory_index: u32,
    /// Byte offset inside the target memory.
    pub offset: u32,
    /// Raw bytes copied during instantiation.
    pub bytes: Vec<u8>,
}

/// Stub kinds admitted at the host-import boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarHostImportStubKind {
    /// Deterministic zero-side-effect import that returns one fixed i32.
    DeterministicI32Const,
    /// Explicit unsupported host-call boundary.
    UnsupportedHostCall,
}

/// Side-effect posture for deterministic import stubs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarDeterministicImportSideEffectPolicy {
    /// Deterministic stubs must stay side-effect free.
    NoSideEffects,
}

/// Typed refusal kind surfaced by the bounded module-execution lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleExecutionRefusalKind {
    /// Host imports outside the deterministic stub set are refused.
    UnsupportedHostImport,
}

/// Hard capability boundary for host imports in the bounded module lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHostImportCapabilityBoundary {
    /// Stub kinds admitted by the current boundary.
    pub supported_stub_kinds: Vec<TassadarHostImportStubKind>,
    /// Side-effect posture for admitted deterministic stubs.
    pub side_effect_policy: TassadarDeterministicImportSideEffectPolicy,
    /// Typed refusal emitted for unsupported host calls.
    pub unsupported_host_call_refusal: TassadarModuleExecutionRefusalKind,
    /// Plain-language boundary note.
    pub claim_boundary: String,
}

/// Runtime capability report for the bounded module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleExecutionCapabilityReport {
    /// Runtime backend exposing the lane.
    pub runtime_backend: String,
    /// Whether globals are supported at all.
    pub supports_globals: bool,
    /// Whether mutable globals are supported.
    pub supports_mutable_globals: bool,
    /// Value types admitted for globals today.
    pub supported_global_value_types: Vec<TassadarModuleValueType>,
    /// Whether tables are supported at all.
    pub supports_tables: bool,
    /// Table element kind admitted today.
    pub supported_table_element_kind: TassadarModuleTableElementKind,
    /// Whether active element-segment instantiation is supported.
    pub supports_active_element_segments: bool,
    /// Whether explicit start-function execution is supported.
    pub supports_start_function_instantiation: bool,
    /// Whether direct in-module calls are supported.
    pub supports_direct_calls: bool,
    /// Whether bounded indirect calls are supported.
    pub supports_call_indirect: bool,
    /// Whether one bounded linear memory is supported.
    pub supports_linear_memory: bool,
    /// Whether active data-segment instantiation is supported.
    pub supports_active_data_segments: bool,
    /// Whether `memory.size` is supported.
    pub supports_memory_size: bool,
    /// Whether bounded `memory.grow` is supported.
    pub supports_memory_grow: bool,
    /// Whether bounded `memory.copy` is supported.
    pub supports_memory_copy: bool,
    /// Whether bounded `memory.fill` is supported.
    pub supports_memory_fill: bool,
    /// Maximum number of memories admitted today.
    pub max_memory_count: u32,
    /// Maximum number of pages per admitted memory.
    pub max_memory_pages: u32,
    /// Maximum table entries admitted by the bounded lane.
    pub max_table_entries: u32,
    /// Hard capability boundary for host imports.
    pub host_import_boundary: TassadarHostImportCapabilityBoundary,
    /// Plain-language lane boundary.
    pub claim_boundary: String,
}

/// Returns the current runtime capability report for the bounded module lane.
#[must_use]
pub fn tassadar_module_execution_capability_report() -> TassadarModuleExecutionCapabilityReport {
    TassadarModuleExecutionCapabilityReport {
        runtime_backend: String::from(TASSADAR_RUNTIME_BACKEND_ID),
        supports_globals: true,
        supports_mutable_globals: true,
        supported_global_value_types: vec![TassadarModuleValueType::I32],
        supports_tables: true,
        supported_table_element_kind: TassadarModuleTableElementKind::Funcref,
        supports_active_element_segments: true,
        supports_start_function_instantiation: true,
        supports_direct_calls: true,
        supports_call_indirect: true,
        supports_linear_memory: true,
        supports_active_data_segments: true,
        supports_memory_size: true,
        supports_memory_grow: true,
        supports_memory_copy: true,
        supports_memory_fill: true,
        max_memory_count: 1,
        max_memory_pages: TASSADAR_MODULE_EXECUTION_MAX_MEMORY_PAGES,
        max_table_entries: 64,
        host_import_boundary: TassadarHostImportCapabilityBoundary {
            supported_stub_kinds: vec![TassadarHostImportStubKind::DeterministicI32Const],
            side_effect_policy: TassadarDeterministicImportSideEffectPolicy::NoSideEffects,
            unsupported_host_call_refusal:
                TassadarModuleExecutionRefusalKind::UnsupportedHostImport,
            claim_boundary: String::from(
                "only deterministic zero-side-effect import stubs are admitted; arbitrary host calls remain explicitly refused",
            ),
        },
        claim_boundary: String::from(
            "bounded module execution covers i32 globals, one bounded linear memory with active data segments, memory.size, memory.grow, memory.copy, memory.fill, funcref tables, active element-segment instantiation, zero-parameter start functions, zero-parameter direct and indirect calls, and deterministic import stubs only; multi-memory, arbitrary signatures, arbitrary imports, and arbitrary host calls remain out of scope",
        ),
    }
}

/// One import stub in the bounded module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stub_kind", rename_all = "snake_case")]
pub enum TassadarHostImportStub {
    /// Deterministic import that returns one fixed i32 result.
    DeterministicI32Const {
        /// Stable import index.
        import_index: u32,
        /// Stable import reference.
        import_ref: String,
        /// Fixed return value.
        value: i32,
        /// Plain-language claim boundary.
        claim_boundary: String,
    },
    /// Unsupported host call that must refuse explicitly.
    UnsupportedHostCall {
        /// Stable import index.
        import_index: u32,
        /// Stable import reference.
        import_ref: String,
        /// Plain-language refusal detail.
        refusal_detail: String,
    },
}

impl TassadarHostImportStub {
    fn import_index(&self) -> u32 {
        match self {
            Self::DeterministicI32Const { import_index, .. }
            | Self::UnsupportedHostCall { import_index, .. } => *import_index,
        }
    }

    fn import_ref(&self) -> &str {
        match self {
            Self::DeterministicI32Const { import_ref, .. }
            | Self::UnsupportedHostCall { import_ref, .. } => import_ref.as_str(),
        }
    }
}

/// One instruction in the bounded module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "opcode", rename_all = "snake_case")]
pub enum TassadarModuleInstruction {
    /// Push one immediate i32.
    I32Const { value: i32 },
    /// Read one local.
    LocalGet { local_index: u32 },
    /// Pop one value into one local.
    LocalSet { local_index: u32 },
    /// Read one global.
    GlobalGet { global_index: u32 },
    /// Pop one value into one global.
    GlobalSet { global_index: u32 },
    /// Pop and discard one stack value.
    Drop,
    /// Pop two i32 values and push one result.
    BinaryOp {
        op: TassadarStructuredControlBinaryOp,
    },
    /// Pop one dynamic base address, read one little-endian i32, and push it.
    I32Load { memory_index: u32, offset: u32 },
    /// Pop one value and one dynamic base address and write one little-endian i32.
    I32Store { memory_index: u32, offset: u32 },
    /// Push the current memory size in pages.
    MemorySize { memory_index: u32 },
    /// Pop one page delta and grow memory if possible.
    MemoryGrow { memory_index: u32 },
    /// Pop `len`, `src`, and `dst` and copy bytes with memmove semantics.
    MemoryCopy {
        dst_memory_index: u32,
        src_memory_index: u32,
    },
    /// Pop `len`, `value`, and `dst` and fill bytes with the low byte of `value`.
    MemoryFill { memory_index: u32 },
    /// Call one declared function directly.
    Call { function_index: u32 },
    /// Resolve one table slot and call the referenced function.
    CallIndirect { table_index: u32 },
    /// Invoke one host import stub.
    HostCall { import_index: u32 },
    /// Return from the current function.
    Return,
}

/// One function in the bounded module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleFunction {
    /// Stable function index.
    pub function_index: u32,
    /// Stable function name.
    pub function_name: String,
    /// Number of parameters consumed by the function.
    pub param_count: u8,
    /// Number of locals including parameters.
    pub local_count: usize,
    /// Number of return values. Only 0 or 1 are supported.
    pub result_count: u8,
    /// Ordered instruction sequence.
    pub instructions: Vec<TassadarModuleInstruction>,
}

impl TassadarModuleFunction {
    /// Creates one bounded module function.
    #[must_use]
    pub fn new(
        function_index: u32,
        function_name: impl Into<String>,
        param_count: u8,
        local_count: usize,
        result_count: u8,
        instructions: Vec<TassadarModuleInstruction>,
    ) -> Self {
        Self {
            function_index,
            function_name: function_name.into(),
            param_count,
            local_count,
            result_count,
            instructions,
        }
    }
}

/// One bounded module-execution program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleExecutionProgram {
    /// Stable program identifier.
    pub program_id: String,
    /// Entry function index.
    pub entry_function_index: u32,
    /// Maximum call depth before execution refuses.
    pub max_call_depth: u32,
    /// Declared globals in stable index order.
    pub globals: Vec<TassadarModuleGlobal>,
    /// Declared tables in stable index order.
    pub tables: Vec<TassadarModuleTable>,
    /// Declared memories in stable index order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memories: Vec<TassadarModuleMemory>,
    /// Active element segments applied during instantiation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub element_segments: Vec<TassadarModuleElementSegment>,
    /// Active data segments applied during instantiation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_segments: Vec<TassadarModuleDataSegment>,
    /// Optional zero-parameter start function executed before the entry export.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_function_index: Option<u32>,
    /// Declared host import stubs in stable index order.
    pub imports: Vec<TassadarHostImportStub>,
    /// Declared functions in stable index order.
    pub functions: Vec<TassadarModuleFunction>,
}

impl TassadarModuleExecutionProgram {
    /// Creates one bounded module-execution program.
    #[must_use]
    pub fn new(
        program_id: impl Into<String>,
        entry_function_index: u32,
        max_call_depth: u32,
        globals: Vec<TassadarModuleGlobal>,
        tables: Vec<TassadarModuleTable>,
        imports: Vec<TassadarHostImportStub>,
        functions: Vec<TassadarModuleFunction>,
    ) -> Self {
        Self {
            program_id: program_id.into(),
            entry_function_index,
            max_call_depth,
            globals,
            tables,
            memories: Vec::new(),
            element_segments: Vec::new(),
            data_segments: Vec::new(),
            start_function_index: None,
            imports,
            functions,
        }
    }

    /// Binds bounded memories to the program.
    #[must_use]
    pub fn with_memories(mut self, memories: Vec<TassadarModuleMemory>) -> Self {
        self.memories = memories;
        self
    }

    /// Binds active element segments to the program.
    #[must_use]
    pub fn with_element_segments(
        mut self,
        element_segments: Vec<TassadarModuleElementSegment>,
    ) -> Self {
        self.element_segments = element_segments;
        self
    }

    /// Binds active data segments to the program.
    #[must_use]
    pub fn with_data_segments(mut self, data_segments: Vec<TassadarModuleDataSegment>) -> Self {
        self.data_segments = data_segments;
        self
    }

    /// Binds one optional start function to the program.
    #[must_use]
    pub fn with_start_function_index(mut self, start_function_index: u32) -> Self {
        self.start_function_index = Some(start_function_index);
        self
    }

    /// Validates the public bounded module surface.
    pub fn validate(&self) -> Result<(), TassadarModuleExecutionError> {
        if self.functions.is_empty() {
            return Err(TassadarModuleExecutionError::NoFunctions);
        }
        let entry = self
            .functions
            .iter()
            .find(|function| function.function_index == self.entry_function_index)
            .ok_or(TassadarModuleExecutionError::MissingEntryFunction {
                entry_function_index: self.entry_function_index,
            })?;
        if entry.param_count != 0 {
            return Err(TassadarModuleExecutionError::EntryFunctionHasParameters {
                entry_function_index: self.entry_function_index,
                param_count: entry.param_count,
            });
        }
        if let Some(start_function_index) = self.start_function_index {
            let start = self
                .functions
                .iter()
                .find(|function| function.function_index == start_function_index)
                .ok_or(TassadarModuleExecutionError::MissingStartFunction {
                    start_function_index,
                })?;
            if start.param_count != 0 || start.result_count != 0 {
                return Err(
                    TassadarModuleExecutionError::UnsupportedStartFunctionSignature {
                        start_function_index,
                        param_count: start.param_count,
                        result_count: start.result_count,
                    },
                );
            }
        }

        for (expected_index, global) in self.globals.iter().enumerate() {
            if global.global_index != expected_index as u32 {
                return Err(TassadarModuleExecutionError::GlobalIndexDrift {
                    expected: expected_index as u32,
                    actual: global.global_index,
                });
            }
            if global.value_type != TassadarModuleValueType::I32 {
                return Err(TassadarModuleExecutionError::UnsupportedGlobalValueType {
                    global_index: global.global_index,
                    value_type: global.value_type,
                });
            }
        }

        for (expected_index, table) in self.tables.iter().enumerate() {
            if table.table_index != expected_index as u32 {
                return Err(TassadarModuleExecutionError::TableIndexDrift {
                    expected: expected_index as u32,
                    actual: table.table_index,
                });
            }
            if table.elements.len() < table.min_entries as usize {
                return Err(TassadarModuleExecutionError::TableBelowMinimum {
                    table_index: table.table_index,
                    min_entries: table.min_entries,
                    actual_entries: table.elements.len(),
                });
            }
            if let Some(max_entries) = table.max_entries
                && table.elements.len() > max_entries as usize
            {
                return Err(TassadarModuleExecutionError::TableAboveMaximum {
                    table_index: table.table_index,
                    max_entries,
                    actual_entries: table.elements.len(),
                });
            }
            for function_index in table.elements.iter().flatten() {
                if *function_index as usize >= self.functions.len() {
                    return Err(TassadarModuleExecutionError::TableFunctionOutOfRange {
                        table_index: table.table_index,
                        function_index: *function_index,
                        function_count: self.functions.len(),
                    });
                }
            }
        }

        if self.memories.len() > 1 {
            return Err(TassadarModuleExecutionError::UnsupportedMemoryCount {
                memory_count: self.memories.len(),
            });
        }
        for (expected_index, memory) in self.memories.iter().enumerate() {
            if memory.memory_index != expected_index as u32 {
                return Err(TassadarModuleExecutionError::MemoryIndexDrift {
                    expected: expected_index as u32,
                    actual: memory.memory_index,
                });
            }
            if memory.max_pages < memory.initial_pages {
                return Err(TassadarModuleExecutionError::MaxPagesBeforeInitial {
                    memory_index: memory.memory_index,
                    initial_pages: memory.initial_pages,
                    max_pages: memory.max_pages,
                });
            }
            if memory.max_pages > TASSADAR_MODULE_EXECUTION_MAX_MEMORY_PAGES {
                return Err(TassadarModuleExecutionError::MemoryPageLimitExceeded {
                    memory_index: memory.memory_index,
                    max_pages: memory.max_pages,
                    profile_max_pages: TASSADAR_MODULE_EXECUTION_MAX_MEMORY_PAGES,
                });
            }
        }

        for (expected_index, segment) in self.element_segments.iter().enumerate() {
            if segment.element_segment_index != expected_index as u32 {
                return Err(TassadarModuleExecutionError::ElementSegmentIndexDrift {
                    expected: expected_index as u32,
                    actual: segment.element_segment_index,
                });
            }
            let table = self.tables.get(segment.table_index as usize).ok_or(
                TassadarModuleExecutionError::ElementSegmentTableOutOfRange {
                    element_segment_index: segment.element_segment_index,
                    table_index: segment.table_index,
                    table_count: self.tables.len(),
                },
            )?;
            let end = segment.offset as usize + segment.elements.len();
            if end > table.elements.len() {
                return Err(TassadarModuleExecutionError::ElementSegmentOutOfRange {
                    element_segment_index: segment.element_segment_index,
                    table_index: segment.table_index,
                    offset: segment.offset,
                    segment_len: segment.elements.len(),
                    table_len: table.elements.len(),
                });
            }
            for function_index in segment.elements.iter().flatten() {
                if *function_index as usize >= self.functions.len() {
                    return Err(
                        TassadarModuleExecutionError::ElementSegmentFunctionOutOfRange {
                            element_segment_index: segment.element_segment_index,
                            table_index: segment.table_index,
                            function_index: *function_index,
                            function_count: self.functions.len(),
                        },
                    );
                }
            }
        }

        for (expected_index, segment) in self.data_segments.iter().enumerate() {
            if segment.data_segment_index != expected_index as u32 {
                return Err(TassadarModuleExecutionError::DataSegmentIndexDrift {
                    expected: expected_index as u32,
                    actual: segment.data_segment_index,
                });
            }
            let memory = self.memories.get(segment.memory_index as usize).ok_or(
                TassadarModuleExecutionError::DataSegmentMemoryOutOfRange {
                    data_segment_index: segment.data_segment_index,
                    memory_index: segment.memory_index,
                    memory_count: self.memories.len(),
                },
            )?;
            let memory_len = module_memory_len_bytes(memory.initial_pages)?;
            let start = usize::try_from(segment.offset)
                .map_err(|_| TassadarModuleExecutionError::ByteLengthOverflow)?;
            let end = start
                .checked_add(segment.bytes.len())
                .ok_or(TassadarModuleExecutionError::ByteLengthOverflow)?;
            if end > memory_len {
                return Err(TassadarModuleExecutionError::DataSegmentOutOfRange {
                    data_segment_index: segment.data_segment_index,
                    memory_index: segment.memory_index,
                    offset: segment.offset,
                    segment_len: segment.bytes.len(),
                    memory_len,
                });
            }
        }

        for (expected_index, import) in self.imports.iter().enumerate() {
            if import.import_index() != expected_index as u32 {
                return Err(TassadarModuleExecutionError::ImportIndexDrift {
                    expected: expected_index as u32,
                    actual: import.import_index(),
                });
            }
            if import.import_ref().trim().is_empty() {
                return Err(TassadarModuleExecutionError::MissingImportRef {
                    import_index: import.import_index(),
                });
            }
        }

        for (expected_index, function) in self.functions.iter().enumerate() {
            if function.function_index != expected_index as u32 {
                return Err(TassadarModuleExecutionError::FunctionIndexDrift {
                    expected: expected_index as u32,
                    actual: function.function_index,
                });
            }
            if function.param_count != 0 {
                return Err(TassadarModuleExecutionError::UnsupportedParamCount {
                    function_index: function.function_index,
                    param_count: function.param_count,
                });
            }
            if function.local_count < usize::from(function.param_count) {
                return Err(TassadarModuleExecutionError::LocalCountTooSmall {
                    function_index: function.function_index,
                    param_count: function.param_count,
                    local_count: function.local_count,
                });
            }
            if function.result_count > 1 {
                return Err(TassadarModuleExecutionError::UnsupportedResultCount {
                    function_index: function.function_index,
                    result_count: function.result_count,
                });
            }
            for instruction in &function.instructions {
                match instruction {
                    TassadarModuleInstruction::LocalGet { local_index }
                    | TassadarModuleInstruction::LocalSet { local_index }
                        if *local_index as usize >= function.local_count =>
                    {
                        return Err(TassadarModuleExecutionError::LocalOutOfRange {
                            function_index: function.function_index,
                            local_index: *local_index,
                            local_count: function.local_count,
                        });
                    }
                    TassadarModuleInstruction::GlobalGet { global_index }
                    | TassadarModuleInstruction::GlobalSet { global_index }
                        if *global_index as usize >= self.globals.len() =>
                    {
                        return Err(TassadarModuleExecutionError::GlobalOutOfRange {
                            global_index: *global_index,
                            global_count: self.globals.len(),
                        });
                    }
                    TassadarModuleInstruction::Call { function_index }
                        if *function_index as usize >= self.functions.len() =>
                    {
                        return Err(TassadarModuleExecutionError::DirectCallFunctionOutOfRange {
                            function_index: *function_index,
                            function_count: self.functions.len(),
                        });
                    }
                    TassadarModuleInstruction::CallIndirect { table_index }
                        if *table_index as usize >= self.tables.len() =>
                    {
                        return Err(TassadarModuleExecutionError::TableOutOfRange {
                            table_index: *table_index,
                            table_count: self.tables.len(),
                        });
                    }
                    TassadarModuleInstruction::HostCall { import_index }
                        if *import_index as usize >= self.imports.len() =>
                    {
                        return Err(TassadarModuleExecutionError::ImportOutOfRange {
                            import_index: *import_index,
                            import_count: self.imports.len(),
                        });
                    }
                    TassadarModuleInstruction::I32Load { memory_index, .. }
                    | TassadarModuleInstruction::I32Store { memory_index, .. }
                    | TassadarModuleInstruction::MemorySize { memory_index }
                    | TassadarModuleInstruction::MemoryGrow { memory_index }
                    | TassadarModuleInstruction::MemoryFill { memory_index }
                        if *memory_index as usize >= self.memories.len() =>
                    {
                        return Err(TassadarModuleExecutionError::MemoryOutOfRange {
                            memory_index: *memory_index,
                            memory_count: self.memories.len(),
                        });
                    }
                    TassadarModuleInstruction::MemoryCopy {
                        dst_memory_index,
                        src_memory_index,
                    } if *dst_memory_index as usize >= self.memories.len()
                        || *src_memory_index as usize >= self.memories.len() =>
                    {
                        return Err(TassadarModuleExecutionError::MemoryCopyMemoryOutOfRange {
                            dst_memory_index: *dst_memory_index,
                            src_memory_index: *src_memory_index,
                            memory_count: self.memories.len(),
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// One visible frame snapshot in the module-execution trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleFrameSnapshot {
    /// Stable function index.
    pub function_index: u32,
    /// Stable function name.
    pub function_name: String,
    /// Program counter inside the function.
    pub pc: usize,
    /// Current locals.
    pub locals: Vec<i32>,
    /// Current operand stack.
    pub operand_stack: Vec<i32>,
}

/// One memory-byte delta emitted by the module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleMemoryByteDelta {
    /// Target memory index.
    pub memory_index: u32,
    /// Byte address touched by the step.
    pub address: u32,
    /// Byte value before the step.
    pub before: u8,
    /// Byte value after the step.
    pub after: u8,
}

/// One memory-growth delta emitted by the module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleMemoryGrowthDelta {
    /// Target memory index.
    pub memory_index: u32,
    /// Page count before the growth.
    pub previous_pages: u32,
    /// Page count after the growth.
    pub new_pages: u32,
    /// Byte count added by the growth.
    pub added_bytes: u32,
}

/// One trace event in the bounded module-execution lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarModuleTraceEvent {
    /// One active element segment was applied during instantiation.
    ElementSegmentApplied {
        element_segment_index: u32,
        table_index: u32,
        offset: u32,
        elements: Vec<Option<u32>>,
    },
    /// One active data segment was applied during instantiation.
    DataSegmentApplied {
        data_segment_index: u32,
        memory_index: u32,
        offset: u32,
        byte_len: usize,
    },
    /// One constant was pushed.
    ConstPush { value: i32 },
    /// One local was read.
    LocalGet { local_index: u32, value: i32 },
    /// One local was written.
    LocalSet { local_index: u32, value: i32 },
    /// One global was read.
    GlobalGet { global_index: u32, value: i32 },
    /// One global was written.
    GlobalSet { global_index: u32, value: i32 },
    /// One stack value was dropped.
    Drop { value: i32 },
    /// One binary operation completed.
    BinaryOp {
        op: TassadarStructuredControlBinaryOp,
        left: i32,
        right: i32,
        result: i32,
    },
    /// One linear-memory load completed.
    Load {
        memory_index: u32,
        address: u32,
        offset: u32,
        raw_bytes: Vec<u8>,
        value: i32,
    },
    /// One linear-memory store completed.
    Store {
        memory_index: u32,
        address: u32,
        offset: u32,
        value: i32,
        written_bytes: Vec<u8>,
    },
    /// One `memory.size` observation completed.
    MemorySize { memory_index: u32, pages: u32 },
    /// One `memory.grow` attempt completed.
    MemoryGrow {
        memory_index: u32,
        requested_pages: i32,
        previous_pages: u32,
        result: i32,
    },
    /// One `memory.copy` completed.
    MemoryCopy {
        dst_memory_index: u32,
        src_memory_index: u32,
        dst_address: u32,
        src_address: u32,
        byte_len: u32,
    },
    /// One `memory.fill` completed.
    MemoryFill {
        memory_index: u32,
        address: u32,
        value: u8,
        byte_len: u32,
    },
    /// One direct call pushed a new frame.
    Call { function_index: u32 },
    /// One indirect call pushed a new frame.
    CallIndirect {
        table_index: u32,
        selector: i32,
        function_index: u32,
    },
    /// One host import stub completed.
    HostCall {
        import_ref: String,
        stub_kind: TassadarHostImportStubKind,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<i32>,
    },
    /// One frame returned.
    Return {
        function_index: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<i32>,
        implicit: bool,
    },
}

/// One append-only trace step for the bounded module lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceStep {
    /// Step index in execution order.
    pub step_index: usize,
    /// Current frame depth after the step.
    pub frame_depth_after: usize,
    /// Event emitted by the step.
    pub event: TassadarModuleTraceEvent,
    /// Current globals after the step.
    pub globals_after: Vec<i32>,
    /// Byte deltas emitted by the step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory_byte_deltas: Vec<TassadarModuleMemoryByteDelta>,
    /// Memory-growth summary when one occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_growth_delta: Option<TassadarModuleMemoryGrowthDelta>,
    /// Memory size in pages after the step for each admitted memory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory_size_pages_after: Vec<u32>,
    /// Current frame stack after the step.
    pub frame_stack_after: Vec<TassadarModuleFrameSnapshot>,
}

/// Terminal reason for one bounded module execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleExecutionHaltReason {
    /// The entry function returned.
    Returned,
    /// The entry function fell off the end.
    FellOffEnd,
}

/// One complete execution result for the bounded module lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleExecution {
    /// Stable program identifier.
    pub program_id: String,
    /// Ordered append-only trace steps.
    pub steps: Vec<TassadarModuleTraceStep>,
    /// Optional returned value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
    /// Final globals in stable index order.
    pub final_globals: Vec<i32>,
    /// Final memory digests in stable index order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub final_memory_digests: Vec<String>,
    /// Final memory sizes in pages in stable index order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub final_memory_pages: Vec<u32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarModuleExecutionHaltReason,
}

impl TassadarModuleExecution {
    /// Returns a stable digest over the visible execution truth.
    #[must_use]
    pub fn execution_digest(&self) -> String {
        stable_digest(b"tassadar_module_execution|", self)
    }
}

/// Validation or execution failure for the bounded module lane.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarModuleExecutionError {
    /// No functions were declared.
    #[error("module-execution program must contain at least one function")]
    NoFunctions,
    /// Entry function was missing.
    #[error("module-execution entry function {entry_function_index} is missing")]
    MissingEntryFunction { entry_function_index: u32 },
    /// Entry function declared parameters.
    #[error(
        "module-execution entry function {entry_function_index} declares unsupported param_count={param_count}"
    )]
    EntryFunctionHasParameters {
        entry_function_index: u32,
        param_count: u8,
    },
    /// Function indices drifted from stable order.
    #[error("module-execution function index drifted: expected {expected}, got {actual}")]
    FunctionIndexDrift { expected: u32, actual: u32 },
    /// One global index drifted from stable order.
    #[error("module-execution global index drifted: expected {expected}, got {actual}")]
    GlobalIndexDrift { expected: u32, actual: u32 },
    /// One table index drifted from stable order.
    #[error("module-execution table index drifted: expected {expected}, got {actual}")]
    TableIndexDrift { expected: u32, actual: u32 },
    /// One import index drifted from stable order.
    #[error("module-execution import index drifted: expected {expected}, got {actual}")]
    ImportIndexDrift { expected: u32, actual: u32 },
    /// The bounded lane currently admits at most one memory.
    #[error("module-execution currently admits at most one memory, got {memory_count}")]
    UnsupportedMemoryCount { memory_count: usize },
    /// One memory index drifted from stable order.
    #[error("module-execution memory index drifted: expected {expected}, got {actual}")]
    MemoryIndexDrift { expected: u32, actual: u32 },
    /// One element segment index drifted from stable order.
    #[error("module-execution element-segment index drifted: expected {expected}, got {actual}")]
    ElementSegmentIndexDrift { expected: u32, actual: u32 },
    /// One data-segment index drifted from stable order.
    #[error("module-execution data-segment index drifted: expected {expected}, got {actual}")]
    DataSegmentIndexDrift { expected: u32, actual: u32 },
    /// One function declared unsupported parameters.
    #[error(
        "module-execution function {function_index} declares unsupported param_count={param_count}"
    )]
    UnsupportedParamCount {
        function_index: u32,
        param_count: u8,
    },
    /// One function declared too many results.
    #[error(
        "module-execution function {function_index} declares unsupported result_count={result_count}"
    )]
    UnsupportedResultCount {
        function_index: u32,
        result_count: u8,
    },
    /// One function declared fewer locals than parameters.
    #[error(
        "module-execution function {function_index} declared local_count={local_count} below param_count={param_count}"
    )]
    LocalCountTooSmall {
        function_index: u32,
        param_count: u8,
        local_count: usize,
    },
    /// One local index exceeded the declared local count.
    #[error(
        "module-execution function {function_index} referenced local {local_index} out of range (local_count={local_count})"
    )]
    LocalOutOfRange {
        function_index: u32,
        local_index: u32,
        local_count: usize,
    },
    /// One global value type is unsupported.
    #[error(
        "module-execution global {global_index} declared unsupported value type `{value_type:?}`"
    )]
    UnsupportedGlobalValueType {
        global_index: u32,
        value_type: TassadarModuleValueType,
    },
    /// One global index exceeded the declared global count.
    #[error("module-execution global {global_index} is out of range (global_count={global_count})")]
    GlobalOutOfRange {
        global_index: u32,
        global_count: usize,
    },
    /// One memory declared max_pages below initial_pages.
    #[error(
        "module-execution memory {memory_index} declared max_pages={max_pages} below initial_pages={initial_pages}"
    )]
    MaxPagesBeforeInitial {
        memory_index: u32,
        initial_pages: u32,
        max_pages: u32,
    },
    /// One memory exceeded the bounded profile page cap.
    #[error(
        "module-execution memory {memory_index} declared max_pages={max_pages} above bounded profile cap {profile_max_pages}"
    )]
    MemoryPageLimitExceeded {
        memory_index: u32,
        max_pages: u32,
        profile_max_pages: u32,
    },
    /// One memory index exceeded the declared memory count.
    #[error("module-execution memory {memory_index} is out of range (memory_count={memory_count})")]
    MemoryOutOfRange {
        memory_index: u32,
        memory_count: usize,
    },
    /// The declared start function was missing.
    #[error("module-execution start function {start_function_index} is missing")]
    MissingStartFunction { start_function_index: u32 },
    /// The declared start function had an unsupported signature.
    #[error(
        "module-execution start function {start_function_index} requires param_count=0 and result_count=0, got param_count={param_count}, result_count={result_count}"
    )]
    UnsupportedStartFunctionSignature {
        start_function_index: u32,
        param_count: u8,
        result_count: u8,
    },
    /// One constant global was written.
    #[error("module-execution global {global_index} is const and cannot be written")]
    ImmutableGlobalWrite { global_index: u32 },
    /// One table index exceeded the declared table count.
    #[error("module-execution table {table_index} is out of range (table_count={table_count})")]
    TableOutOfRange {
        table_index: u32,
        table_count: usize,
    },
    /// One table had fewer elements than its minimum.
    #[error(
        "module-execution table {table_index} declared min_entries={min_entries} but carried {actual_entries} elements"
    )]
    TableBelowMinimum {
        table_index: u32,
        min_entries: u32,
        actual_entries: usize,
    },
    /// One table exceeded its maximum.
    #[error(
        "module-execution table {table_index} declared max_entries={max_entries} but carried {actual_entries} elements"
    )]
    TableAboveMaximum {
        table_index: u32,
        max_entries: u32,
        actual_entries: usize,
    },
    /// One function index in a table exceeded the declared function count.
    #[error(
        "module-execution table {table_index} referenced function {function_index} out of range (function_count={function_count})"
    )]
    TableFunctionOutOfRange {
        table_index: u32,
        function_index: u32,
        function_count: usize,
    },
    /// One element segment referenced a missing table.
    #[error(
        "module-execution element segment {element_segment_index} references table {table_index} out of range (table_count={table_count})"
    )]
    ElementSegmentTableOutOfRange {
        element_segment_index: u32,
        table_index: u32,
        table_count: usize,
    },
    /// One element segment exceeded the declared table length.
    #[error(
        "module-execution element segment {element_segment_index} writes {segment_len} entries at offset {offset} into table {table_index} of length {table_len}"
    )]
    ElementSegmentOutOfRange {
        element_segment_index: u32,
        table_index: u32,
        offset: u32,
        segment_len: usize,
        table_len: usize,
    },
    /// One element segment referenced a missing function.
    #[error(
        "module-execution element segment {element_segment_index} references function {function_index} out of range (function_count={function_count})"
    )]
    ElementSegmentFunctionOutOfRange {
        element_segment_index: u32,
        table_index: u32,
        function_index: u32,
        function_count: usize,
    },
    /// One data segment referenced a missing memory.
    #[error(
        "module-execution data segment {data_segment_index} references memory {memory_index} out of range (memory_count={memory_count})"
    )]
    DataSegmentMemoryOutOfRange {
        data_segment_index: u32,
        memory_index: u32,
        memory_count: usize,
    },
    /// One data segment exceeded the declared initial memory length.
    #[error(
        "module-execution data segment {data_segment_index} writes {segment_len} bytes at offset {offset} into memory {memory_index} of length {memory_len}"
    )]
    DataSegmentOutOfRange {
        data_segment_index: u32,
        memory_index: u32,
        offset: u32,
        segment_len: usize,
        memory_len: usize,
    },
    /// One direct call referenced a missing function.
    #[error(
        "module-execution direct call referenced function {function_index} out of range (function_count={function_count})"
    )]
    DirectCallFunctionOutOfRange {
        function_index: u32,
        function_count: usize,
    },
    /// One call_indirect selector exceeded the table length.
    #[error(
        "module-execution table {table_index} selector {selector} is out of range (entry_count={entry_count})"
    )]
    TableSelectorOutOfRange {
        table_index: u32,
        selector: i32,
        entry_count: usize,
    },
    /// One indirect call hit an empty table slot.
    #[error("module-execution table {table_index} selector {selector} resolved to an empty slot")]
    EmptyTableEntry { table_index: u32, selector: i32 },
    /// One `memory.copy` referenced a missing memory.
    #[error(
        "module-execution memory.copy referenced dst_memory={dst_memory_index} src_memory={src_memory_index} out of range (memory_count={memory_count})"
    )]
    MemoryCopyMemoryOutOfRange {
        dst_memory_index: u32,
        src_memory_index: u32,
        memory_count: usize,
    },
    /// One import index exceeded the declared import count.
    #[error("module-execution import {import_index} is out of range (import_count={import_count})")]
    ImportOutOfRange {
        import_index: u32,
        import_count: usize,
    },
    /// One import stub omitted its reference.
    #[error("module-execution import {import_index} is missing `import_ref`")]
    MissingImportRef { import_index: u32 },
    /// One host import stayed outside the admitted stub boundary.
    #[error("module-execution host import `{import_ref}` is unsupported: {detail}")]
    UnsupportedHostImport { import_ref: String, detail: String },
    /// One instruction needed more stack items than were available.
    #[error(
        "module-execution stack underflow in function {function_index} at pc {pc} for {context}: needed {needed}, available {available}"
    )]
    StackUnderflow {
        function_index: u32,
        pc: usize,
        context: String,
        needed: usize,
        available: usize,
    },
    /// One byte-addressed memory access exceeded the current memory size.
    #[error(
        "module-execution memory access in function {function_index} at pc {pc} exceeded memory {memory_index}: address={address}, width_bytes={width_bytes}, memory_len={memory_len}"
    )]
    MemoryAddressOutOfRange {
        function_index: u32,
        pc: usize,
        memory_index: u32,
        address: u32,
        width_bytes: u32,
        memory_len: usize,
    },
    /// One `memory.copy` range exceeded the source or destination memory.
    #[error(
        "module-execution memory.copy in function {function_index} at pc {pc} exceeded bounds: src_memory={src_memory_index} dst_memory={dst_memory_index} src_address={src_address} dst_address={dst_address} byte_len={byte_len} src_memory_len={src_memory_len} dst_memory_len={dst_memory_len}"
    )]
    MemoryCopyOutOfRange {
        function_index: u32,
        pc: usize,
        src_memory_index: u32,
        dst_memory_index: u32,
        src_address: u32,
        dst_address: u32,
        byte_len: u32,
        src_memory_len: usize,
        dst_memory_len: usize,
    },
    /// One `memory.fill` range exceeded the current memory.
    #[error(
        "module-execution memory.fill in function {function_index} at pc {pc} exceeded bounds: memory={memory_index} address={address} byte_len={byte_len} memory_len={memory_len}"
    )]
    MemoryFillOutOfRange {
        function_index: u32,
        pc: usize,
        memory_index: u32,
        address: u32,
        byte_len: u32,
        memory_len: usize,
    },
    /// Execution exceeded the bounded step limit.
    #[error("module-execution exceeded the step limit of {max_steps}")]
    StepLimitExceeded { max_steps: usize },
    /// Execution exceeded the bounded call-depth limit.
    #[error("module-execution exceeded max_call_depth={max_call_depth}")]
    CallDepthExceeded { max_call_depth: u32 },
    /// Page arithmetic overflowed the supported host representation.
    #[error("module-execution byte length overflowed host usize")]
    ByteLengthOverflow,
}

/// Executes one validated bounded module-execution program.
pub fn execute_tassadar_module_execution_program(
    program: &TassadarModuleExecutionProgram,
) -> Result<TassadarModuleExecution, TassadarModuleExecutionError> {
    program.validate()?;
    let mut state = ModuleExecutionState::new(program)?;
    instantiate_tables(program, &mut state)?;
    instantiate_memories(program, &mut state)?;

    if let Some(start_function_index) = program.start_function_index {
        let _ = execute_module_root_call(program, &mut state, start_function_index)?;
    }

    let (returned_value, halt_reason) =
        execute_module_root_call(program, &mut state, program.entry_function_index)?;
    let final_memory_digests = state
        .memories
        .iter()
        .map(|memory| stable_digest(b"tassadar_module_memory|", memory))
        .collect();
    let final_memory_pages = state.memory_sizes_pages()?;

    Ok(TassadarModuleExecution {
        program_id: program.program_id.clone(),
        steps: state.steps,
        returned_value,
        final_globals: state.globals,
        final_memory_digests,
        final_memory_pages,
        halt_reason,
    })
}

fn instantiate_tables(
    program: &TassadarModuleExecutionProgram,
    state: &mut ModuleExecutionState,
) -> Result<(), TassadarModuleExecutionError> {
    for segment in &program.element_segments {
        let table_count = state.tables.len();
        let table = state.tables.get_mut(segment.table_index as usize).ok_or(
            TassadarModuleExecutionError::ElementSegmentTableOutOfRange {
                element_segment_index: segment.element_segment_index,
                table_index: segment.table_index,
                table_count,
            },
        )?;
        let start = segment.offset as usize;
        let end = start + segment.elements.len();
        if end > table.len() {
            return Err(TassadarModuleExecutionError::ElementSegmentOutOfRange {
                element_segment_index: segment.element_segment_index,
                table_index: segment.table_index,
                offset: segment.offset,
                segment_len: segment.elements.len(),
                table_len: table.len(),
            });
        }
        for (offset, function_index) in segment.elements.iter().enumerate() {
            table[start + offset] = *function_index;
        }
        state.push_step(
            program,
            TassadarModuleTraceEvent::ElementSegmentApplied {
                element_segment_index: segment.element_segment_index,
                table_index: segment.table_index,
                offset: segment.offset,
                elements: segment.elements.clone(),
            },
        )?;
    }
    Ok(())
}

fn instantiate_memories(
    program: &TassadarModuleExecutionProgram,
    state: &mut ModuleExecutionState,
) -> Result<(), TassadarModuleExecutionError> {
    for segment in &program.data_segments {
        let memory_count = state.memories.len();
        let memory = state
            .memories
            .get_mut(segment.memory_index as usize)
            .ok_or(TassadarModuleExecutionError::DataSegmentMemoryOutOfRange {
                data_segment_index: segment.data_segment_index,
                memory_index: segment.memory_index,
                memory_count,
            })?;
        let start = usize::try_from(segment.offset)
            .map_err(|_| TassadarModuleExecutionError::ByteLengthOverflow)?;
        let end = start
            .checked_add(segment.bytes.len())
            .ok_or(TassadarModuleExecutionError::ByteLengthOverflow)?;
        if end > memory.len() {
            return Err(TassadarModuleExecutionError::DataSegmentOutOfRange {
                data_segment_index: segment.data_segment_index,
                memory_index: segment.memory_index,
                offset: segment.offset,
                segment_len: segment.bytes.len(),
                memory_len: memory.len(),
            });
        }
        memory[start..end].copy_from_slice(segment.bytes.as_slice());
        state.push_step(
            program,
            TassadarModuleTraceEvent::DataSegmentApplied {
                data_segment_index: segment.data_segment_index,
                memory_index: segment.memory_index,
                offset: segment.offset,
                byte_len: segment.bytes.len(),
            },
        )?;
    }
    Ok(())
}

fn execute_module_root_call(
    program: &TassadarModuleExecutionProgram,
    state: &mut ModuleExecutionState,
    root_function_index: u32,
) -> Result<(Option<i32>, TassadarModuleExecutionHaltReason), TassadarModuleExecutionError> {
    let root = &program.functions[root_function_index as usize];
    state.frames.push(ModuleFrame::new(root));

    while !state.frames.is_empty() {
        if state.step_index >= TASSADAR_MODULE_EXECUTION_MAX_STEPS {
            return Err(TassadarModuleExecutionError::StepLimitExceeded {
                max_steps: TASSADAR_MODULE_EXECUTION_MAX_STEPS,
            });
        }
        let current_index = state.frames.len() - 1;
        let function_index = state.frames[current_index].function_index;
        let function = &program.functions[function_index as usize];
        if state.frames[current_index].pc >= function.instructions.len() {
            let value =
                finalize_function_return(&mut state.frames[current_index], function, false)?;
            state.frames.pop();
            if let Some(caller) = state.frames.last_mut() {
                if let Some(value) = value {
                    caller.operand_stack.push(value);
                }
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::Return {
                        function_index,
                        value,
                        implicit: true,
                    },
                )?;
                continue;
            }
            state.push_step(
                program,
                TassadarModuleTraceEvent::Return {
                    function_index,
                    value,
                    implicit: true,
                },
            )?;
            return Ok((value, TassadarModuleExecutionHaltReason::FellOffEnd));
        }

        let instruction = function.instructions[state.frames[current_index].pc].clone();
        match instruction {
            TassadarModuleInstruction::I32Const { value } => {
                state.frames[current_index].operand_stack.push(value);
                state.frames[current_index].pc += 1;
                state.push_step(program, TassadarModuleTraceEvent::ConstPush { value })?;
            }
            TassadarModuleInstruction::LocalGet { local_index } => {
                let value = *state.frames[current_index]
                    .locals
                    .get(local_index as usize)
                    .ok_or(TassadarModuleExecutionError::LocalOutOfRange {
                        function_index,
                        local_index,
                        local_count: function.local_count,
                    })?;
                state.frames[current_index].operand_stack.push(value);
                state.frames[current_index].pc += 1;
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::LocalGet { local_index, value },
                )?;
            }
            TassadarModuleInstruction::LocalSet { local_index } => {
                let value = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "local.set",
                )?;
                *state.frames[current_index]
                    .locals
                    .get_mut(local_index as usize)
                    .ok_or(TassadarModuleExecutionError::LocalOutOfRange {
                        function_index,
                        local_index,
                        local_count: function.local_count,
                    })? = value;
                state.frames[current_index].pc += 1;
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::LocalSet { local_index, value },
                )?;
            }
            TassadarModuleInstruction::GlobalGet { global_index } => {
                let value = *state.globals.get(global_index as usize).ok_or(
                    TassadarModuleExecutionError::GlobalOutOfRange {
                        global_index,
                        global_count: state.globals.len(),
                    },
                )?;
                state.frames[current_index].operand_stack.push(value);
                state.frames[current_index].pc += 1;
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::GlobalGet {
                        global_index,
                        value,
                    },
                )?;
            }
            TassadarModuleInstruction::GlobalSet { global_index } => {
                let value = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "global.set",
                )?;
                let global = program.globals.get(global_index as usize).ok_or(
                    TassadarModuleExecutionError::GlobalOutOfRange {
                        global_index,
                        global_count: program.globals.len(),
                    },
                )?;
                if global.mutability != TassadarModuleGlobalMutability::Mutable {
                    return Err(TassadarModuleExecutionError::ImmutableGlobalWrite {
                        global_index,
                    });
                }
                let global_count = state.globals.len();
                *state.globals.get_mut(global_index as usize).ok_or(
                    TassadarModuleExecutionError::GlobalOutOfRange {
                        global_index,
                        global_count,
                    },
                )? = value;
                state.frames[current_index].pc += 1;
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::GlobalSet {
                        global_index,
                        value,
                    },
                )?;
            }
            TassadarModuleInstruction::Drop => {
                let value = pop_operand(&mut state.frames[current_index], function_index, "drop")?;
                state.frames[current_index].pc += 1;
                state.push_step(program, TassadarModuleTraceEvent::Drop { value })?;
            }
            TassadarModuleInstruction::BinaryOp { op } => {
                let right = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "binary_op",
                )?;
                let left = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "binary_op",
                )?;
                let result = execute_binary_op(op, left, right);
                state.frames[current_index].operand_stack.push(result);
                state.frames[current_index].pc += 1;
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::BinaryOp {
                        op,
                        left,
                        right,
                        result,
                    },
                )?;
            }
            TassadarModuleInstruction::I32Load {
                memory_index,
                offset,
            } => {
                let dynamic_address =
                    pop_operand(&mut state.frames[current_index], function_index, "i32.load")?;
                let address = effective_memory_address(dynamic_address, offset);
                let memory = state.memories.get(memory_index as usize).ok_or(
                    TassadarModuleExecutionError::MemoryOutOfRange {
                        memory_index,
                        memory_count: state.memories.len(),
                    },
                )?;
                let raw_bytes = read_module_memory(
                    memory,
                    function_index,
                    state.frames[current_index].pc,
                    memory_index,
                    address,
                )?;
                let value =
                    i32::from_le_bytes(raw_bytes.as_slice().try_into().expect("exact 4-byte load"));
                state.frames[current_index].operand_stack.push(value);
                state.frames[current_index].pc += 1;
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::Load {
                        memory_index,
                        address,
                        offset,
                        raw_bytes,
                        value,
                    },
                )?;
            }
            TassadarModuleInstruction::I32Store {
                memory_index,
                offset,
            } => {
                let value = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "i32.store",
                )?;
                let dynamic_address = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "i32.store",
                )?;
                let address = effective_memory_address(dynamic_address, offset);
                let memory_len = state
                    .memories
                    .get(memory_index as usize)
                    .map_or(0, Vec::len);
                let memory_count = state.memories.len();
                let base = checked_module_memory_range(
                    function_index,
                    state.frames[current_index].pc,
                    memory_index,
                    address,
                    4,
                    memory_len,
                )?;
                let memory = state.memories.get_mut(memory_index as usize).ok_or(
                    TassadarModuleExecutionError::MemoryOutOfRange {
                        memory_index,
                        memory_count,
                    },
                )?;
                let written_bytes = value.to_le_bytes().to_vec();
                let mut memory_byte_deltas = Vec::new();
                for (byte_offset, byte) in written_bytes.iter().enumerate() {
                    let index = base + byte_offset;
                    let before = memory[index];
                    memory[index] = *byte;
                    if before != *byte {
                        memory_byte_deltas.push(TassadarModuleMemoryByteDelta {
                            memory_index,
                            address: address.saturating_add(byte_offset as u32),
                            before,
                            after: *byte,
                        });
                    }
                }
                state.frames[current_index].pc += 1;
                state.push_step_with_memory(
                    program,
                    TassadarModuleTraceEvent::Store {
                        memory_index,
                        address,
                        offset,
                        value,
                        written_bytes,
                    },
                    memory_byte_deltas,
                    None,
                )?;
            }
            TassadarModuleInstruction::MemorySize { memory_index } => {
                let memory = state.memories.get(memory_index as usize).ok_or(
                    TassadarModuleExecutionError::MemoryOutOfRange {
                        memory_index,
                        memory_count: state.memories.len(),
                    },
                )?;
                let pages = module_current_memory_pages(memory.len())?;
                state.frames[current_index]
                    .operand_stack
                    .push(i32::try_from(pages).unwrap_or(i32::MAX));
                state.frames[current_index].pc += 1;
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::MemorySize {
                        memory_index,
                        pages,
                    },
                )?;
            }
            TassadarModuleInstruction::MemoryGrow { memory_index } => {
                let requested_pages = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "memory.grow",
                )?;
                let memory_count = state.memories.len();
                let memory = state.memories.get_mut(memory_index as usize).ok_or(
                    TassadarModuleExecutionError::MemoryOutOfRange {
                        memory_index,
                        memory_count,
                    },
                )?;
                let previous_pages = module_current_memory_pages(memory.len())?;
                let max_pages = program.memories[memory_index as usize].max_pages;
                let mut memory_growth_delta = None;
                let result = if requested_pages < 0 {
                    -1
                } else {
                    let requested_pages_u32 = requested_pages as u32;
                    if requested_pages_u32 == 0 {
                        i32::try_from(previous_pages).unwrap_or(i32::MAX)
                    } else if let Some(new_pages) = previous_pages.checked_add(requested_pages_u32)
                    {
                        if new_pages > max_pages {
                            -1
                        } else {
                            let old_len = memory.len();
                            let new_len = module_memory_len_bytes(new_pages)?;
                            memory.resize(new_len, 0);
                            memory_growth_delta = Some(TassadarModuleMemoryGrowthDelta {
                                memory_index,
                                previous_pages,
                                new_pages,
                                added_bytes: u32::try_from(new_len.saturating_sub(old_len))
                                    .unwrap_or(u32::MAX),
                            });
                            i32::try_from(previous_pages).unwrap_or(i32::MAX)
                        }
                    } else {
                        -1
                    }
                };
                state.frames[current_index].operand_stack.push(result);
                state.frames[current_index].pc += 1;
                state.push_step_with_memory(
                    program,
                    TassadarModuleTraceEvent::MemoryGrow {
                        memory_index,
                        requested_pages,
                        previous_pages,
                        result,
                    },
                    Vec::new(),
                    memory_growth_delta,
                )?;
            }
            TassadarModuleInstruction::MemoryCopy {
                dst_memory_index,
                src_memory_index,
            } => {
                let byte_len = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "memory.copy",
                )? as u32;
                let src_address = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "memory.copy",
                )? as u32;
                let dst_address = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "memory.copy",
                )? as u32;
                let src_len = state
                    .memories
                    .get(src_memory_index as usize)
                    .map_or(0, Vec::len);
                let dst_len = state
                    .memories
                    .get(dst_memory_index as usize)
                    .map_or(0, Vec::len);
                let (src_base, dst_base) = checked_module_copy_ranges(
                    function_index,
                    state.frames[current_index].pc,
                    src_memory_index,
                    dst_memory_index,
                    src_address,
                    dst_address,
                    byte_len,
                    src_len,
                    dst_len,
                )?;
                let mut memory_byte_deltas = Vec::new();
                let memory_count = state.memories.len();
                if src_memory_index == dst_memory_index {
                    let memory = state.memories.get_mut(src_memory_index as usize).ok_or(
                        TassadarModuleExecutionError::MemoryCopyMemoryOutOfRange {
                            dst_memory_index,
                            src_memory_index,
                            memory_count,
                        },
                    )?;
                    let copy_bytes = memory[src_base..src_base + byte_len as usize].to_vec();
                    for (offset, byte) in copy_bytes.iter().enumerate() {
                        let index = dst_base + offset;
                        let before = memory[index];
                        memory[index] = *byte;
                        if before != *byte {
                            memory_byte_deltas.push(TassadarModuleMemoryByteDelta {
                                memory_index: dst_memory_index,
                                address: dst_address.saturating_add(offset as u32),
                                before,
                                after: *byte,
                            });
                        }
                    }
                } else {
                    let copy_bytes = state.memories[src_memory_index as usize]
                        [src_base..src_base + byte_len as usize]
                        .to_vec();
                    let memory = state.memories.get_mut(dst_memory_index as usize).ok_or(
                        TassadarModuleExecutionError::MemoryCopyMemoryOutOfRange {
                            dst_memory_index,
                            src_memory_index,
                            memory_count,
                        },
                    )?;
                    for (offset, byte) in copy_bytes.iter().enumerate() {
                        let index = dst_base + offset;
                        let before = memory[index];
                        memory[index] = *byte;
                        if before != *byte {
                            memory_byte_deltas.push(TassadarModuleMemoryByteDelta {
                                memory_index: dst_memory_index,
                                address: dst_address.saturating_add(offset as u32),
                                before,
                                after: *byte,
                            });
                        }
                    }
                }
                state.frames[current_index].pc += 1;
                state.push_step_with_memory(
                    program,
                    TassadarModuleTraceEvent::MemoryCopy {
                        dst_memory_index,
                        src_memory_index,
                        dst_address,
                        src_address,
                        byte_len,
                    },
                    memory_byte_deltas,
                    None,
                )?;
            }
            TassadarModuleInstruction::MemoryFill { memory_index } => {
                let byte_len = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "memory.fill",
                )? as u32;
                let value = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "memory.fill",
                )? as u8;
                let address = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "memory.fill",
                )? as u32;
                let memory_len = state
                    .memories
                    .get(memory_index as usize)
                    .map_or(0, Vec::len);
                let base = checked_module_fill_range(
                    function_index,
                    state.frames[current_index].pc,
                    memory_index,
                    address,
                    byte_len,
                    memory_len,
                )?;
                let memory_count = state.memories.len();
                let memory = state.memories.get_mut(memory_index as usize).ok_or(
                    TassadarModuleExecutionError::MemoryOutOfRange {
                        memory_index,
                        memory_count,
                    },
                )?;
                let mut memory_byte_deltas = Vec::new();
                for offset in 0..byte_len as usize {
                    let index = base + offset;
                    let before = memory[index];
                    memory[index] = value;
                    if before != value {
                        memory_byte_deltas.push(TassadarModuleMemoryByteDelta {
                            memory_index,
                            address: address.saturating_add(offset as u32),
                            before,
                            after: value,
                        });
                    }
                }
                state.frames[current_index].pc += 1;
                state.push_step_with_memory(
                    program,
                    TassadarModuleTraceEvent::MemoryFill {
                        memory_index,
                        address,
                        value,
                        byte_len,
                    },
                    memory_byte_deltas,
                    None,
                )?;
            }
            TassadarModuleInstruction::Call {
                function_index: target_function_index,
            } => {
                push_call_frame(program, state, current_index, target_function_index)?;
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::Call {
                        function_index: target_function_index,
                    },
                )?;
            }
            TassadarModuleInstruction::CallIndirect { table_index } => {
                let selector = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "call_indirect",
                )?;
                let table = state.tables.get(table_index as usize).ok_or(
                    TassadarModuleExecutionError::TableOutOfRange {
                        table_index,
                        table_count: state.tables.len(),
                    },
                )?;
                let target_function_index = table
                    .get(selector as usize)
                    .ok_or(TassadarModuleExecutionError::TableSelectorOutOfRange {
                        table_index,
                        selector,
                        entry_count: table.len(),
                    })?
                    .ok_or(TassadarModuleExecutionError::EmptyTableEntry {
                        table_index,
                        selector,
                    })?;
                push_call_frame(program, state, current_index, target_function_index)?;
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::CallIndirect {
                        table_index,
                        selector,
                        function_index: target_function_index,
                    },
                )?;
            }
            TassadarModuleInstruction::HostCall { import_index } => {
                let import = program.imports.get(import_index as usize).ok_or(
                    TassadarModuleExecutionError::ImportOutOfRange {
                        import_index,
                        import_count: program.imports.len(),
                    },
                )?;
                state.frames[current_index].pc += 1;
                match import {
                    TassadarHostImportStub::DeterministicI32Const {
                        import_ref, value, ..
                    } => {
                        state.frames[current_index].operand_stack.push(*value);
                        state.push_step(
                            program,
                            TassadarModuleTraceEvent::HostCall {
                                import_ref: import_ref.clone(),
                                stub_kind: TassadarHostImportStubKind::DeterministicI32Const,
                                result: Some(*value),
                            },
                        )?;
                    }
                    TassadarHostImportStub::UnsupportedHostCall {
                        import_ref,
                        refusal_detail,
                        ..
                    } => {
                        return Err(TassadarModuleExecutionError::UnsupportedHostImport {
                            import_ref: import_ref.clone(),
                            detail: refusal_detail.clone(),
                        });
                    }
                }
            }
            TassadarModuleInstruction::Return => {
                let value =
                    finalize_function_return(&mut state.frames[current_index], function, true)?;
                state.frames.pop();
                if let Some(caller) = state.frames.last_mut() {
                    if let Some(value) = value {
                        caller.operand_stack.push(value);
                    }
                    state.push_step(
                        program,
                        TassadarModuleTraceEvent::Return {
                            function_index,
                            value,
                            implicit: false,
                        },
                    )?;
                    continue;
                }
                state.push_step(
                    program,
                    TassadarModuleTraceEvent::Return {
                        function_index,
                        value,
                        implicit: false,
                    },
                )?;
                return Ok((value, TassadarModuleExecutionHaltReason::Returned));
            }
        }
    }

    Ok((None, TassadarModuleExecutionHaltReason::Returned))
}

fn push_call_frame(
    program: &TassadarModuleExecutionProgram,
    state: &mut ModuleExecutionState,
    caller_frame_index: usize,
    target_function_index: u32,
) -> Result<(), TassadarModuleExecutionError> {
    if state.frames.len() >= program.max_call_depth as usize {
        return Err(TassadarModuleExecutionError::CallDepthExceeded {
            max_call_depth: program.max_call_depth,
        });
    }
    let target = program
        .functions
        .get(target_function_index as usize)
        .ok_or(TassadarModuleExecutionError::DirectCallFunctionOutOfRange {
            function_index: target_function_index,
            function_count: program.functions.len(),
        })?;
    state.frames[caller_frame_index].pc += 1;
    state.frames.push(ModuleFrame::new(target));
    Ok(())
}

/// Returns one seeded global-state parity program.
#[must_use]
pub fn tassadar_seeded_module_global_state_program() -> TassadarModuleExecutionProgram {
    TassadarModuleExecutionProgram::new(
        "tassadar.module_execution.global_state.v1",
        0,
        8,
        vec![TassadarModuleGlobal {
            global_index: 0,
            value_type: TassadarModuleValueType::I32,
            mutability: TassadarModuleGlobalMutability::Mutable,
            initial_value: 5,
        }],
        Vec::new(),
        Vec::new(),
        vec![TassadarModuleFunction::new(
            0,
            "entry",
            0,
            0,
            1,
            vec![
                TassadarModuleInstruction::GlobalGet { global_index: 0 },
                TassadarModuleInstruction::I32Const { value: 7 },
                TassadarModuleInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarModuleInstruction::GlobalSet { global_index: 0 },
                TassadarModuleInstruction::GlobalGet { global_index: 0 },
                TassadarModuleInstruction::Return,
            ],
        )],
    )
}

/// Returns one seeded indirect-call program over one bounded table.
#[must_use]
pub fn tassadar_seeded_module_call_indirect_program() -> TassadarModuleExecutionProgram {
    TassadarModuleExecutionProgram::new(
        "tassadar.module_execution.call_indirect.v1",
        0,
        8,
        Vec::new(),
        vec![TassadarModuleTable {
            table_index: 0,
            element_kind: TassadarModuleTableElementKind::Funcref,
            min_entries: 2,
            max_entries: Some(2),
            elements: vec![Some(1), Some(2)],
        }],
        Vec::new(),
        vec![
            TassadarModuleFunction::new(
                0,
                "entry",
                0,
                0,
                1,
                vec![
                    TassadarModuleInstruction::I32Const { value: 1 },
                    TassadarModuleInstruction::CallIndirect { table_index: 0 },
                    TassadarModuleInstruction::Return,
                ],
            ),
            TassadarModuleFunction::new(
                1,
                "slot_zero",
                0,
                0,
                1,
                vec![
                    TassadarModuleInstruction::I32Const { value: 111 },
                    TassadarModuleInstruction::Return,
                ],
            ),
            TassadarModuleFunction::new(
                2,
                "slot_one",
                0,
                0,
                1,
                vec![
                    TassadarModuleInstruction::I32Const { value: 222 },
                    TassadarModuleInstruction::Return,
                ],
            ),
        ],
    )
}

/// Returns one seeded instantiation program over active element segments and a
/// start function.
#[must_use]
pub fn tassadar_seeded_module_instantiation_program() -> TassadarModuleExecutionProgram {
    TassadarModuleExecutionProgram::new(
        "tassadar.module_execution.instantiation.v1",
        0,
        8,
        vec![TassadarModuleGlobal {
            global_index: 0,
            value_type: TassadarModuleValueType::I32,
            mutability: TassadarModuleGlobalMutability::Mutable,
            initial_value: 1,
        }],
        vec![TassadarModuleTable {
            table_index: 0,
            element_kind: TassadarModuleTableElementKind::Funcref,
            min_entries: 3,
            max_entries: Some(3),
            elements: vec![Some(2), None, None],
        }],
        Vec::new(),
        vec![
            TassadarModuleFunction::new(
                0,
                "entry",
                0,
                0,
                1,
                vec![
                    TassadarModuleInstruction::GlobalGet { global_index: 0 },
                    TassadarModuleInstruction::I32Const { value: 1 },
                    TassadarModuleInstruction::CallIndirect { table_index: 0 },
                    TassadarModuleInstruction::BinaryOp {
                        op: TassadarStructuredControlBinaryOp::Add,
                    },
                    TassadarModuleInstruction::Return,
                ],
            ),
            TassadarModuleFunction::new(
                1,
                "start",
                0,
                0,
                0,
                vec![
                    TassadarModuleInstruction::Call { function_index: 4 },
                    TassadarModuleInstruction::Return,
                ],
            ),
            TassadarModuleFunction::new(
                2,
                "slot_zero",
                0,
                0,
                1,
                vec![
                    TassadarModuleInstruction::I32Const { value: 9 },
                    TassadarModuleInstruction::Return,
                ],
            ),
            TassadarModuleFunction::new(
                3,
                "slot_one",
                0,
                0,
                1,
                vec![
                    TassadarModuleInstruction::I32Const { value: 11 },
                    TassadarModuleInstruction::Return,
                ],
            ),
            TassadarModuleFunction::new(
                4,
                "init_global",
                0,
                0,
                0,
                vec![
                    TassadarModuleInstruction::I32Const { value: 31 },
                    TassadarModuleInstruction::GlobalSet { global_index: 0 },
                    TassadarModuleInstruction::Return,
                ],
            ),
        ],
    )
    .with_element_segments(vec![TassadarModuleElementSegment {
        element_segment_index: 0,
        table_index: 0,
        offset: 1,
        elements: vec![Some(3), Some(2)],
    }])
    .with_start_function_index(1)
}

/// Returns one seeded dynamic-memory program over one bounded memory.
#[must_use]
pub fn tassadar_seeded_module_dynamic_memory_program() -> TassadarModuleExecutionProgram {
    TassadarModuleExecutionProgram::new(
        "tassadar.module_execution.dynamic_memory.v1",
        0,
        8,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![TassadarModuleFunction::new(
            0,
            "entry",
            0,
            3,
            1,
            vec![
                TassadarModuleInstruction::I32Const { value: 8 },
                TassadarModuleInstruction::I32Const { value: 0 },
                TassadarModuleInstruction::I32Const { value: 4 },
                TassadarModuleInstruction::MemoryCopy {
                    dst_memory_index: 0,
                    src_memory_index: 0,
                },
                TassadarModuleInstruction::MemorySize { memory_index: 0 },
                TassadarModuleInstruction::LocalSet { local_index: 0 },
                TassadarModuleInstruction::I32Const { value: 1 },
                TassadarModuleInstruction::MemoryGrow { memory_index: 0 },
                TassadarModuleInstruction::LocalSet { local_index: 1 },
                TassadarModuleInstruction::MemorySize { memory_index: 0 },
                TassadarModuleInstruction::LocalSet { local_index: 2 },
                TassadarModuleInstruction::I32Const { value: 12 },
                TassadarModuleInstruction::I32Const { value: 7 },
                TassadarModuleInstruction::I32Const { value: 4 },
                TassadarModuleInstruction::MemoryFill { memory_index: 0 },
                TassadarModuleInstruction::I32Const { value: 8 },
                TassadarModuleInstruction::I32Load {
                    memory_index: 0,
                    offset: 0,
                },
                TassadarModuleInstruction::I32Const { value: 12 },
                TassadarModuleInstruction::I32Load {
                    memory_index: 0,
                    offset: 0,
                },
                TassadarModuleInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarModuleInstruction::LocalGet { local_index: 0 },
                TassadarModuleInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarModuleInstruction::LocalGet { local_index: 1 },
                TassadarModuleInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarModuleInstruction::LocalGet { local_index: 2 },
                TassadarModuleInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarModuleInstruction::Return,
            ],
        )],
    )
    .with_memories(vec![TassadarModuleMemory {
        memory_index: 0,
        initial_pages: 1,
        max_pages: 3,
    }])
    .with_data_segments(vec![TassadarModuleDataSegment {
        data_segment_index: 0,
        memory_index: 0,
        offset: 0,
        bytes: vec![1, 2, 3, 4],
    }])
}

/// Returns one seeded deterministic-import program.
#[must_use]
pub fn tassadar_seeded_module_deterministic_import_program() -> TassadarModuleExecutionProgram {
    TassadarModuleExecutionProgram::new(
        "tassadar.module_execution.import_stub.v1",
        0,
        8,
        Vec::new(),
        Vec::new(),
        vec![TassadarHostImportStub::DeterministicI32Const {
            import_index: 0,
            import_ref: String::from("env.clock_stub"),
            value: 41,
            claim_boundary: String::from(
                "deterministic zero-side-effect stub returning one fixed i32",
            ),
        }],
        vec![TassadarModuleFunction::new(
            0,
            "entry",
            0,
            0,
            1,
            vec![
                TassadarModuleInstruction::HostCall { import_index: 0 },
                TassadarModuleInstruction::I32Const { value: 1 },
                TassadarModuleInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarModuleInstruction::Return,
            ],
        )],
    )
}

/// Returns one seeded unsupported-host-call refusal program.
#[must_use]
pub fn tassadar_seeded_module_unsupported_host_import_program() -> TassadarModuleExecutionProgram {
    TassadarModuleExecutionProgram::new(
        "tassadar.module_execution.unsupported_host_import.v1",
        0,
        8,
        Vec::new(),
        Vec::new(),
        vec![TassadarHostImportStub::UnsupportedHostCall {
            import_index: 0,
            import_ref: String::from("host.fs_write"),
            refusal_detail: String::from(
                "host calls outside deterministic stub admission remain unsupported",
            ),
        }],
        vec![TassadarModuleFunction::new(
            0,
            "entry",
            0,
            0,
            1,
            vec![
                TassadarModuleInstruction::HostCall { import_index: 0 },
                TassadarModuleInstruction::Return,
            ],
        )],
    )
}

struct ModuleFrame {
    function_index: u32,
    pc: usize,
    locals: Vec<i32>,
    operand_stack: Vec<i32>,
}

impl ModuleFrame {
    fn new(function: &TassadarModuleFunction) -> Self {
        Self {
            function_index: function.function_index,
            pc: 0,
            locals: vec![0; function.local_count],
            operand_stack: Vec::new(),
        }
    }
}

struct ModuleExecutionState {
    globals: Vec<i32>,
    tables: Vec<Vec<Option<u32>>>,
    memories: Vec<Vec<u8>>,
    frames: Vec<ModuleFrame>,
    steps: Vec<TassadarModuleTraceStep>,
    step_index: usize,
}

impl ModuleExecutionState {
    fn new(program: &TassadarModuleExecutionProgram) -> Result<Self, TassadarModuleExecutionError> {
        Ok(Self {
            globals: program
                .globals
                .iter()
                .map(|global| global.initial_value)
                .collect(),
            tables: program
                .tables
                .iter()
                .map(|table| table.elements.clone())
                .collect(),
            memories: program
                .memories
                .iter()
                .map(|memory| Ok(vec![0u8; module_memory_len_bytes(memory.initial_pages)?]))
                .collect::<Result<Vec<_>, TassadarModuleExecutionError>>()?,
            frames: Vec::new(),
            steps: Vec::new(),
            step_index: 0,
        })
    }

    fn push_step(
        &mut self,
        program: &TassadarModuleExecutionProgram,
        event: TassadarModuleTraceEvent,
    ) -> Result<(), TassadarModuleExecutionError> {
        self.push_step_with_memory(program, event, Vec::new(), None)
    }

    fn push_step_with_memory(
        &mut self,
        program: &TassadarModuleExecutionProgram,
        event: TassadarModuleTraceEvent,
        memory_byte_deltas: Vec<TassadarModuleMemoryByteDelta>,
        memory_growth_delta: Option<TassadarModuleMemoryGrowthDelta>,
    ) -> Result<(), TassadarModuleExecutionError> {
        if self.step_index >= TASSADAR_MODULE_EXECUTION_MAX_STEPS {
            return Err(TassadarModuleExecutionError::StepLimitExceeded {
                max_steps: TASSADAR_MODULE_EXECUTION_MAX_STEPS,
            });
        }
        self.steps.push(TassadarModuleTraceStep {
            step_index: self.step_index,
            frame_depth_after: self.frames.len(),
            event,
            globals_after: self.globals.clone(),
            memory_byte_deltas,
            memory_growth_delta,
            memory_size_pages_after: self.memory_sizes_pages()?,
            frame_stack_after: self
                .frames
                .iter()
                .map(|frame| TassadarModuleFrameSnapshot {
                    function_index: frame.function_index,
                    function_name: program.functions[frame.function_index as usize]
                        .function_name
                        .clone(),
                    pc: frame.pc,
                    locals: frame.locals.clone(),
                    operand_stack: frame.operand_stack.clone(),
                })
                .collect(),
        });
        self.step_index = self.step_index.saturating_add(1);
        Ok(())
    }

    fn memory_sizes_pages(&self) -> Result<Vec<u32>, TassadarModuleExecutionError> {
        self.memories
            .iter()
            .map(|memory| module_current_memory_pages(memory.len()))
            .collect()
    }
}

fn pop_operand(
    frame: &mut ModuleFrame,
    function_index: u32,
    context: &str,
) -> Result<i32, TassadarModuleExecutionError> {
    let available = frame.operand_stack.len();
    frame
        .operand_stack
        .pop()
        .ok_or_else(|| TassadarModuleExecutionError::StackUnderflow {
            function_index,
            pc: frame.pc,
            context: String::from(context),
            needed: 1,
            available,
        })
}

fn finalize_function_return(
    frame: &mut ModuleFrame,
    function: &TassadarModuleFunction,
    explicit: bool,
) -> Result<Option<i32>, TassadarModuleExecutionError> {
    let value = match function.result_count {
        0 => None,
        1 => Some(pop_operand(
            frame,
            function.function_index,
            if explicit {
                "return"
            } else {
                "implicit_return"
            },
        )?),
        other => {
            return Err(TassadarModuleExecutionError::UnsupportedResultCount {
                function_index: function.function_index,
                result_count: other,
            });
        }
    };
    Ok(value)
}

fn execute_binary_op(op: TassadarStructuredControlBinaryOp, left: i32, right: i32) -> i32 {
    match op {
        TassadarStructuredControlBinaryOp::Add => left.saturating_add(right),
        TassadarStructuredControlBinaryOp::Sub => left.saturating_sub(right),
        TassadarStructuredControlBinaryOp::Mul => left.saturating_mul(right),
        TassadarStructuredControlBinaryOp::Eq => i32::from(left == right),
        TassadarStructuredControlBinaryOp::Ne => i32::from(left != right),
        TassadarStructuredControlBinaryOp::LtS => i32::from(left < right),
        TassadarStructuredControlBinaryOp::LtU => i32::from((left as u32) < (right as u32)),
        TassadarStructuredControlBinaryOp::GtS => i32::from(left > right),
        TassadarStructuredControlBinaryOp::GtU => i32::from((left as u32) > (right as u32)),
        TassadarStructuredControlBinaryOp::LeS => i32::from(left <= right),
        TassadarStructuredControlBinaryOp::LeU => i32::from((left as u32) <= (right as u32)),
        TassadarStructuredControlBinaryOp::GeS => i32::from(left >= right),
        TassadarStructuredControlBinaryOp::GeU => i32::from((left as u32) >= (right as u32)),
        TassadarStructuredControlBinaryOp::And => left & right,
        TassadarStructuredControlBinaryOp::Or => left | right,
        TassadarStructuredControlBinaryOp::Xor => left ^ right,
        TassadarStructuredControlBinaryOp::Shl => left.wrapping_shl(right as u32),
        TassadarStructuredControlBinaryOp::ShrS => left.wrapping_shr(right as u32),
        TassadarStructuredControlBinaryOp::ShrU => {
            ((left as u32).wrapping_shr(right as u32)) as i32
        }
    }
}

fn module_memory_len_bytes(pages: u32) -> Result<usize, TassadarModuleExecutionError> {
    let bytes = u64::from(pages)
        .checked_mul(u64::from(TASSADAR_LINEAR_MEMORY_PAGE_BYTES))
        .ok_or(TassadarModuleExecutionError::ByteLengthOverflow)?;
    usize::try_from(bytes).map_err(|_| TassadarModuleExecutionError::ByteLengthOverflow)
}

fn module_current_memory_pages(memory_len: usize) -> Result<u32, TassadarModuleExecutionError> {
    let page_bytes = usize::try_from(TASSADAR_LINEAR_MEMORY_PAGE_BYTES)
        .map_err(|_| TassadarModuleExecutionError::ByteLengthOverflow)?;
    u32::try_from(memory_len / page_bytes)
        .map_err(|_| TassadarModuleExecutionError::ByteLengthOverflow)
}

fn effective_memory_address(base: i32, offset: u32) -> u32 {
    (base as u32).wrapping_add(offset)
}

fn checked_module_memory_range(
    function_index: u32,
    pc: usize,
    memory_index: u32,
    address: u32,
    width_bytes: u32,
    memory_len: usize,
) -> Result<usize, TassadarModuleExecutionError> {
    let base =
        usize::try_from(address).map_err(|_| TassadarModuleExecutionError::ByteLengthOverflow)?;
    let end = base
        .checked_add(width_bytes as usize)
        .ok_or(TassadarModuleExecutionError::ByteLengthOverflow)?;
    if end > memory_len {
        return Err(TassadarModuleExecutionError::MemoryAddressOutOfRange {
            function_index,
            pc,
            memory_index,
            address,
            width_bytes,
            memory_len,
        });
    }
    Ok(base)
}

fn checked_module_copy_ranges(
    function_index: u32,
    pc: usize,
    src_memory_index: u32,
    dst_memory_index: u32,
    src_address: u32,
    dst_address: u32,
    byte_len: u32,
    src_memory_len: usize,
    dst_memory_len: usize,
) -> Result<(usize, usize), TassadarModuleExecutionError> {
    let src_base = usize::try_from(src_address)
        .map_err(|_| TassadarModuleExecutionError::ByteLengthOverflow)?;
    let dst_base = usize::try_from(dst_address)
        .map_err(|_| TassadarModuleExecutionError::ByteLengthOverflow)?;
    let src_end = src_base
        .checked_add(byte_len as usize)
        .ok_or(TassadarModuleExecutionError::ByteLengthOverflow)?;
    let dst_end = dst_base
        .checked_add(byte_len as usize)
        .ok_or(TassadarModuleExecutionError::ByteLengthOverflow)?;
    if src_end > src_memory_len || dst_end > dst_memory_len {
        return Err(TassadarModuleExecutionError::MemoryCopyOutOfRange {
            function_index,
            pc,
            src_memory_index,
            dst_memory_index,
            src_address,
            dst_address,
            byte_len,
            src_memory_len,
            dst_memory_len,
        });
    }
    Ok((src_base, dst_base))
}

fn checked_module_fill_range(
    function_index: u32,
    pc: usize,
    memory_index: u32,
    address: u32,
    byte_len: u32,
    memory_len: usize,
) -> Result<usize, TassadarModuleExecutionError> {
    let base =
        usize::try_from(address).map_err(|_| TassadarModuleExecutionError::ByteLengthOverflow)?;
    let end = base
        .checked_add(byte_len as usize)
        .ok_or(TassadarModuleExecutionError::ByteLengthOverflow)?;
    if end > memory_len {
        return Err(TassadarModuleExecutionError::MemoryFillOutOfRange {
            function_index,
            pc,
            memory_index,
            address,
            byte_len,
            memory_len,
        });
    }
    Ok(base)
}

fn read_module_memory(
    memory: &[u8],
    function_index: u32,
    pc: usize,
    memory_index: u32,
    address: u32,
) -> Result<Vec<u8>, TassadarModuleExecutionError> {
    let base =
        checked_module_memory_range(function_index, pc, memory_index, address, 4, memory.len())?;
    Ok(memory[base..base + 4].to_vec())
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
        TassadarHostImportStubKind, TassadarModuleExecutionRefusalKind,
        TassadarModuleGlobalMutability, TassadarModuleValueType,
        execute_tassadar_module_execution_program, tassadar_module_execution_capability_report,
        tassadar_seeded_module_call_indirect_program,
        tassadar_seeded_module_deterministic_import_program,
        tassadar_seeded_module_dynamic_memory_program, tassadar_seeded_module_global_state_program,
        tassadar_seeded_module_instantiation_program,
        tassadar_seeded_module_unsupported_host_import_program,
    };

    #[test]
    fn module_execution_capability_report_is_machine_legible() {
        let report = tassadar_module_execution_capability_report();
        assert!(report.supports_globals);
        assert!(report.supports_active_element_segments);
        assert!(report.supports_start_function_instantiation);
        assert!(report.supports_direct_calls);
        assert!(report.supports_call_indirect);
        assert!(report.supports_linear_memory);
        assert!(report.supports_active_data_segments);
        assert!(report.supports_memory_size);
        assert!(report.supports_memory_grow);
        assert!(report.supports_memory_copy);
        assert!(report.supports_memory_fill);
        assert_eq!(
            report.supported_global_value_types,
            vec![TassadarModuleValueType::I32]
        );
        assert_eq!(report.max_memory_count, 1);
        assert_eq!(
            report.host_import_boundary.supported_stub_kinds,
            vec![TassadarHostImportStubKind::DeterministicI32Const]
        );
        assert_eq!(
            report.host_import_boundary.unsupported_host_call_refusal,
            TassadarModuleExecutionRefusalKind::UnsupportedHostImport
        );
    }

    #[test]
    fn module_execution_global_state_parity_is_exact() {
        let program = tassadar_seeded_module_global_state_program();
        assert_eq!(
            program.globals[0].mutability,
            TassadarModuleGlobalMutability::Mutable
        );
        let execution = execute_tassadar_module_execution_program(&program).expect("execute");
        assert_eq!(execution.returned_value, Some(12));
        assert_eq!(execution.final_globals, vec![12]);
    }

    #[test]
    fn module_execution_call_indirect_dispatches_exactly() {
        let program = tassadar_seeded_module_call_indirect_program();
        let execution = execute_tassadar_module_execution_program(&program).expect("execute");
        assert_eq!(execution.returned_value, Some(222));
        assert!(execution.steps.iter().any(|step| matches!(
            step.event,
            super::TassadarModuleTraceEvent::CallIndirect { .. }
        )));
    }

    #[test]
    fn module_execution_instantiation_applies_start_and_elements_exactly() {
        let program = tassadar_seeded_module_instantiation_program();
        let execution = execute_tassadar_module_execution_program(&program).expect("execute");
        assert_eq!(execution.returned_value, Some(42));
        assert_eq!(execution.final_globals, vec![31]);
        assert!(execution.steps.iter().any(|step| matches!(
            step.event,
            super::TassadarModuleTraceEvent::ElementSegmentApplied { .. }
        )));
        assert!(execution.steps.iter().any(|step| matches!(
            step.event,
            super::TassadarModuleTraceEvent::Call { function_index: 4 }
        )));
    }

    #[test]
    fn module_execution_dynamic_memory_is_exact() {
        let program = tassadar_seeded_module_dynamic_memory_program();
        let execution = execute_tassadar_module_execution_program(&program).expect("execute");
        assert_eq!(execution.returned_value, Some(185_207_052));
        assert_eq!(execution.final_memory_pages, vec![2]);
        assert_eq!(execution.final_memory_digests.len(), 1);
        assert!(execution.steps.iter().any(|step| matches!(
            step.event,
            super::TassadarModuleTraceEvent::DataSegmentApplied { .. }
        )));
        assert!(execution.steps.iter().any(|step| matches!(
            step.event,
            super::TassadarModuleTraceEvent::MemoryCopy { .. }
        )));
        assert!(execution.steps.iter().any(|step| matches!(
            step.event,
            super::TassadarModuleTraceEvent::MemoryFill { .. }
        )));
        assert!(execution.steps.iter().any(|step| matches!(
            step.event,
            super::TassadarModuleTraceEvent::MemoryGrow { result: 1, .. }
        )));
    }

    #[test]
    fn module_execution_dynamic_memory_refuses_out_of_range_access() {
        let mut program = tassadar_seeded_module_dynamic_memory_program();
        program.functions[0].instructions = vec![
            super::TassadarModuleInstruction::I32Const { value: i32::MAX },
            super::TassadarModuleInstruction::I32Load {
                memory_index: 0,
                offset: 1024,
            },
            super::TassadarModuleInstruction::Return,
        ];
        let error = execute_tassadar_module_execution_program(&program).expect_err("should refuse");
        assert!(matches!(
            error,
            super::TassadarModuleExecutionError::MemoryAddressOutOfRange { .. }
        ));
    }

    #[test]
    fn module_execution_dynamic_memory_grow_above_max_returns_minus_one() {
        let mut program = tassadar_seeded_module_dynamic_memory_program();
        program.functions[0].instructions = vec![
            super::TassadarModuleInstruction::I32Const { value: 9 },
            super::TassadarModuleInstruction::MemoryGrow { memory_index: 0 },
            super::TassadarModuleInstruction::Return,
        ];
        let execution = execute_tassadar_module_execution_program(&program).expect("execute");
        assert_eq!(execution.returned_value, Some(-1));
        assert_eq!(execution.final_memory_pages, vec![1]);
    }

    #[test]
    fn module_execution_deterministic_import_stub_is_exact() {
        let program = tassadar_seeded_module_deterministic_import_program();
        let execution = execute_tassadar_module_execution_program(&program).expect("execute");
        assert_eq!(execution.returned_value, Some(42));
    }

    #[test]
    fn module_execution_unsupported_host_call_refuses_explicitly() {
        let error = execute_tassadar_module_execution_program(
            &tassadar_seeded_module_unsupported_host_import_program(),
        )
        .expect_err("host call should refuse");
        assert!(matches!(
            error,
            super::TassadarModuleExecutionError::UnsupportedHostImport { .. }
        ));
    }
}
