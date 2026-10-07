use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, DataSegment, DataSegmentMode, ElementMode, ElementSection,
    Elements, EntityType, ExportKind, ExportSection, Function, FunctionSection, GlobalSection,
    GlobalType, ImportSection, Instruction, MemArg, MemorySection, Module, StartSection,
    TableSection, TableType, TypeSection, ValType,
};
use wasmparser::{
    DataKind, ElementItems, ElementKind, Encoding, ExternalKind, Operator, Parser, Payload,
    RefType, TypeRef,
};

const TASSADAR_NORMALIZED_WASM_MODULE_SCHEMA_VERSION: u16 = 1;
const TASSADAR_NORMALIZED_WASM_MODULE_CLAIM_BOUNDARY: &str = "normalized Wasm module ingress preserves explicit type, function, table, global, export, start, element-segment, memory, data-segment, and code-body structure for the current bounded core module slice only; lowering support stays narrower and does not imply arbitrary Wasm closure";

/// One normalized core Wasm value type accepted by the bounded Tassadar module
/// ingress lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNormalizedWasmValueType {
    /// 32-bit signed or unsigned integer lane.
    I32,
    /// 64-bit signed or unsigned integer lane.
    I64,
    /// 32-bit floating-point lane.
    F32,
    /// 64-bit floating-point lane.
    F64,
}

/// One normalized function signature from the Wasm type section.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmFunctionType {
    /// Stable type index in the module type space.
    pub type_index: u32,
    /// Ordered parameter types.
    pub params: Vec<TassadarNormalizedWasmValueType>,
    /// Ordered result types.
    pub results: Vec<TassadarNormalizedWasmValueType>,
}

/// One normalized core Wasm memory type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmMemoryType {
    /// Minimum size in Wasm pages.
    pub minimum_pages: u64,
    /// Optional maximum size in Wasm pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_pages: Option<u64>,
    /// Whether the memory is shared.
    pub shared: bool,
    /// Whether the memory uses 64-bit indexing.
    pub memory64: bool,
    /// Optional custom page-size exponent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size_log2: Option<u32>,
}

/// One normalized function import or definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmFunction {
    /// Stable function index in the module function index space.
    pub function_index: u32,
    /// Stable type index referenced by the function.
    pub type_index: u32,
    /// Import module name when this function is imported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_module: Option<String>,
    /// Import item name when this function is imported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_name: Option<String>,
    /// Normalized body for defined functions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<TassadarNormalizedWasmFunctionBody>,
}

impl TassadarNormalizedWasmFunction {
    /// Creates one imported function row.
    #[must_use]
    pub fn imported(
        function_index: u32,
        type_index: u32,
        import_module: impl Into<String>,
        import_name: impl Into<String>,
    ) -> Self {
        Self {
            function_index,
            type_index,
            import_module: Some(import_module.into()),
            import_name: Some(import_name.into()),
            body: None,
        }
    }

    /// Creates one defined function row.
    #[must_use]
    pub fn defined(
        function_index: u32,
        type_index: u32,
        body: TassadarNormalizedWasmFunctionBody,
    ) -> Self {
        Self {
            function_index,
            type_index,
            import_module: None,
            import_name: None,
            body: Some(body),
        }
    }

    /// Returns whether the function is imported.
    #[must_use]
    pub fn imported_function(&self) -> bool {
        self.body.is_none()
    }
}

/// One normalized core Wasm memory import or definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmMemory {
    /// Stable memory index.
    pub memory_index: u32,
    /// Imported or defined memory shape.
    pub memory_type: TassadarNormalizedWasmMemoryType,
    /// Import module name when this memory is imported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_module: Option<String>,
    /// Import item name when this memory is imported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_name: Option<String>,
}

impl TassadarNormalizedWasmMemory {
    /// Creates one imported memory row.
    #[must_use]
    pub fn imported(
        memory_index: u32,
        memory_type: TassadarNormalizedWasmMemoryType,
        import_module: impl Into<String>,
        import_name: impl Into<String>,
    ) -> Self {
        Self {
            memory_index,
            memory_type,
            import_module: Some(import_module.into()),
            import_name: Some(import_name.into()),
        }
    }

    /// Creates one defined memory row.
    #[must_use]
    pub fn defined(memory_index: u32, memory_type: TassadarNormalizedWasmMemoryType) -> Self {
        Self {
            memory_index,
            memory_type,
            import_module: None,
            import_name: None,
        }
    }

    /// Returns whether the memory is imported.
    #[must_use]
    pub fn imported_memory(&self) -> bool {
        self.import_module.is_some()
    }
}

/// Mutability posture for one normalized core Wasm global.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNormalizedWasmGlobalMutability {
    /// Immutable global.
    Const,
    /// Mutable global.
    Mutable,
}

/// One normalized defined Wasm global.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmGlobal {
    /// Stable global index.
    pub global_index: u32,
    /// Global value type.
    pub value_type: TassadarNormalizedWasmValueType,
    /// Global mutability posture.
    pub mutability: TassadarNormalizedWasmGlobalMutability,
    /// Constant initializer for the global.
    pub init_expr: TassadarNormalizedWasmConstExpr,
}

/// Table element kind admitted by the normalized IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNormalizedWasmTableElementKind {
    /// Function references.
    Funcref,
}

/// One normalized defined Wasm table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmTable {
    /// Stable table index.
    pub table_index: u32,
    /// Table element kind.
    pub element_kind: TassadarNormalizedWasmTableElementKind,
    /// Minimum declared entries.
    pub min_entries: u32,
    /// Optional maximum declared entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_entries: Option<u32>,
}

/// One normalized constant expression accepted in active data segments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarNormalizedWasmConstExpr {
    /// One `i32.const`.
    I32Const {
        /// Immediate value.
        value: i32,
    },
    /// One `global.get`.
    GlobalGet {
        /// Referenced global index.
        global_index: u32,
    },
}

/// One normalized instruction inside a Wasm function body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "opcode", rename_all = "snake_case")]
pub enum TassadarNormalizedWasmInstruction {
    /// Push one immediate `i32`.
    I32Const {
        /// Literal value.
        value: i32,
    },
    /// Read one local.
    LocalGet {
        /// Local index.
        local_index: u32,
    },
    /// Write one local.
    LocalSet {
        /// Local index.
        local_index: u32,
    },
    /// Tee one local.
    LocalTee {
        /// Local index.
        local_index: u32,
    },
    /// Read one global.
    GlobalGet {
        /// Global index.
        global_index: u32,
    },
    /// Write one global.
    GlobalSet {
        /// Global index.
        global_index: u32,
    },
    /// Signed `i32` less-than comparison.
    I32LtS,
    /// Integer addition.
    I32Add,
    /// Integer subtraction.
    I32Sub,
    /// Integer multiplication.
    I32Mul,
    /// Integer left shift.
    I32Shl,
    /// Word-sized `i32.load`.
    I32Load {
        /// Alignment exponent.
        align: u8,
        /// Byte offset immediate.
        offset: u64,
        /// Target memory index.
        memory_index: u32,
    },
    /// Word-sized `i32.store`.
    I32Store {
        /// Alignment exponent.
        align: u8,
        /// Byte offset immediate.
        offset: u64,
        /// Target memory index.
        memory_index: u32,
    },
    /// Push the current memory size in pages.
    MemorySize {
        /// Target memory index.
        memory_index: u32,
    },
    /// Pop one page delta and grow the target memory.
    MemoryGrow {
        /// Target memory index.
        memory_index: u32,
    },
    /// Pop `len`, `src`, and `dst` and copy bytes with memmove semantics.
    MemoryCopy {
        /// Destination memory index.
        dst_memory_index: u32,
        /// Source memory index.
        src_memory_index: u32,
    },
    /// Pop `len`, `value`, and `dst` and fill bytes with one byte.
    MemoryFill {
        /// Target memory index.
        memory_index: u32,
    },
    /// Direct function call.
    Call {
        /// Target function index.
        function_index: u32,
    },
    /// Indirect function call through one table.
    CallIndirect {
        /// Referenced function type.
        type_index: u32,
        /// Target table index.
        table_index: u32,
    },
    /// Explicit return.
    Return,
    /// Drop one stack value.
    Drop,
}

impl TassadarNormalizedWasmInstruction {
    /// Returns the stable mnemonic for the normalized instruction.
    #[must_use]
    pub const fn mnemonic(&self) -> &'static str {
        match self {
            Self::I32Const { .. } => "i32.const",
            Self::LocalGet { .. } => "local.get",
            Self::LocalSet { .. } => "local.set",
            Self::LocalTee { .. } => "local.tee",
            Self::GlobalGet { .. } => "global.get",
            Self::GlobalSet { .. } => "global.set",
            Self::I32LtS => "i32.lt_s",
            Self::I32Add => "i32.add",
            Self::I32Sub => "i32.sub",
            Self::I32Mul => "i32.mul",
            Self::I32Shl => "i32.shl",
            Self::I32Load { .. } => "i32.load",
            Self::I32Store { .. } => "i32.store",
            Self::MemorySize { .. } => "memory.size",
            Self::MemoryGrow { .. } => "memory.grow",
            Self::MemoryCopy { .. } => "memory.copy",
            Self::MemoryFill { .. } => "memory.fill",
            Self::Call { .. } => "call",
            Self::CallIndirect { .. } => "call_indirect",
            Self::Return => "return",
            Self::Drop => "drop",
        }
    }
}

/// One normalized function body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmFunctionBody {
    /// Ordered locals excluding parameters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locals: Vec<TassadarNormalizedWasmValueType>,
    /// Ordered instruction sequence without the implicit terminal `end`.
    pub instructions: Vec<TassadarNormalizedWasmInstruction>,
}

impl TassadarNormalizedWasmFunctionBody {
    /// Creates one normalized body.
    #[must_use]
    pub fn new(
        locals: Vec<TassadarNormalizedWasmValueType>,
        instructions: Vec<TassadarNormalizedWasmInstruction>,
    ) -> Self {
        Self {
            locals,
            instructions,
        }
    }
}

/// One normalized export row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmExport {
    /// Stable export name.
    pub export_name: String,
    /// Export kind.
    pub kind: TassadarNormalizedWasmExportKind,
    /// Referenced index in the corresponding index space.
    pub index: u32,
}

impl TassadarNormalizedWasmExport {
    /// Creates one export row.
    #[must_use]
    pub fn new(
        export_name: impl Into<String>,
        kind: TassadarNormalizedWasmExportKind,
        index: u32,
    ) -> Self {
        Self {
            export_name: export_name.into(),
            kind,
            index,
        }
    }
}

/// Export kinds accepted by the normalized module IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarNormalizedWasmExportKind {
    /// Function export.
    Function,
    /// Memory export.
    Memory,
}

/// One normalized data segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmDataSegment {
    /// Stable segment index.
    pub data_index: u32,
    /// Segment mode.
    pub mode: TassadarNormalizedWasmDataMode,
    /// Raw payload bytes.
    pub bytes: Vec<u8>,
}

impl TassadarNormalizedWasmDataSegment {
    /// Creates one normalized data segment.
    #[must_use]
    pub fn new(data_index: u32, mode: TassadarNormalizedWasmDataMode, bytes: Vec<u8>) -> Self {
        Self {
            data_index,
            mode,
            bytes,
        }
    }
}

/// One normalized data-segment mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TassadarNormalizedWasmDataMode {
    /// Passive data segment.
    Passive,
    /// Active segment against one memory index.
    Active {
        /// Target memory index.
        memory_index: u32,
        /// Offset constant expression.
        offset_expr: TassadarNormalizedWasmConstExpr,
    },
}

/// One normalized active function element segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmElementSegment {
    /// Stable element index.
    pub element_index: u32,
    /// Target table index.
    pub table_index: u32,
    /// Offset constant expression.
    pub offset_expr: TassadarNormalizedWasmConstExpr,
    /// Function indices written into the table.
    pub function_indices: Vec<u32>,
}

/// Canonical normalized Wasm module IR for the bounded Tassadar ingress lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarNormalizedWasmModule {
    /// Stable schema version.
    pub schema_version: u16,
    /// Ordered type section.
    pub types: Vec<TassadarNormalizedWasmFunctionType>,
    /// Ordered function index space including imports.
    pub functions: Vec<TassadarNormalizedWasmFunction>,
    /// Ordered defined table section.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tables: Vec<TassadarNormalizedWasmTable>,
    /// Ordered memory index space including imports.
    pub memories: Vec<TassadarNormalizedWasmMemory>,
    /// Ordered defined global section.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub globals: Vec<TassadarNormalizedWasmGlobal>,
    /// Ordered export section.
    pub exports: Vec<TassadarNormalizedWasmExport>,
    /// Optional start function index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_function_index: Option<u32>,
    /// Ordered active element segments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub element_segments: Vec<TassadarNormalizedWasmElementSegment>,
    /// Ordered data segments.
    pub data_segments: Vec<TassadarNormalizedWasmDataSegment>,
    /// Explicit claim boundary for the normalized IR.
    pub claim_boundary: String,
    /// Stable digest over the module.
    pub module_digest: String,
}

impl TassadarNormalizedWasmModule {
    /// Creates one validated normalized module.
    pub fn new(
        types: Vec<TassadarNormalizedWasmFunctionType>,
        functions: Vec<TassadarNormalizedWasmFunction>,
        memories: Vec<TassadarNormalizedWasmMemory>,
        exports: Vec<TassadarNormalizedWasmExport>,
        data_segments: Vec<TassadarNormalizedWasmDataSegment>,
    ) -> Result<Self, TassadarNormalizedWasmModuleError> {
        Self::new_full(
            types,
            functions,
            Vec::new(),
            memories,
            Vec::new(),
            exports,
            None,
            Vec::new(),
            data_segments,
        )
    }

    /// Creates one validated normalized module including the extended bounded
    /// table/global/start/element surface.
    pub fn new_full(
        types: Vec<TassadarNormalizedWasmFunctionType>,
        functions: Vec<TassadarNormalizedWasmFunction>,
        tables: Vec<TassadarNormalizedWasmTable>,
        memories: Vec<TassadarNormalizedWasmMemory>,
        globals: Vec<TassadarNormalizedWasmGlobal>,
        exports: Vec<TassadarNormalizedWasmExport>,
        start_function_index: Option<u32>,
        element_segments: Vec<TassadarNormalizedWasmElementSegment>,
        data_segments: Vec<TassadarNormalizedWasmDataSegment>,
    ) -> Result<Self, TassadarNormalizedWasmModuleError> {
        let mut module = Self {
            schema_version: TASSADAR_NORMALIZED_WASM_MODULE_SCHEMA_VERSION,
            types,
            functions,
            tables,
            memories,
            globals,
            exports,
            start_function_index,
            element_segments,
            data_segments,
            claim_boundary: String::from(TASSADAR_NORMALIZED_WASM_MODULE_CLAIM_BOUNDARY),
            module_digest: String::new(),
        };
        module.validate_internal_consistency()?;
        module.module_digest = stable_digest(b"tassadar_normalized_wasm_module|", &module);
        Ok(module)
    }

    /// Binds defined tables to the normalized module.
    #[must_use]
    pub fn with_tables(mut self, tables: Vec<TassadarNormalizedWasmTable>) -> Self {
        self.tables = tables;
        self.refresh_digest();
        self
    }

    /// Binds defined globals to the normalized module.
    #[must_use]
    pub fn with_globals(mut self, globals: Vec<TassadarNormalizedWasmGlobal>) -> Self {
        self.globals = globals;
        self.refresh_digest();
        self
    }

    /// Binds one optional start function to the normalized module.
    #[must_use]
    pub fn with_start_function_index(mut self, start_function_index: u32) -> Self {
        self.start_function_index = Some(start_function_index);
        self.refresh_digest();
        self
    }

    /// Binds active element segments to the normalized module.
    #[must_use]
    pub fn with_element_segments(
        mut self,
        element_segments: Vec<TassadarNormalizedWasmElementSegment>,
    ) -> Self {
        self.element_segments = element_segments;
        self.refresh_digest();
        self
    }

    fn refresh_digest(&mut self) {
        self.module_digest.clear();
        self.module_digest = stable_digest(b"tassadar_normalized_wasm_module|", self);
    }

    /// Returns one function export by stable export name.
    #[must_use]
    pub fn exported_function_by_name(
        &self,
        export_name: &str,
    ) -> Option<(
        &TassadarNormalizedWasmExport,
        &TassadarNormalizedWasmFunction,
    )> {
        let export = self.exports.iter().find(|export| {
            export.kind == TassadarNormalizedWasmExportKind::Function
                && export.export_name == export_name
        })?;
        let function = self
            .functions
            .iter()
            .find(|function| function.function_index == export.index)?;
        Some((export, function))
    }

    /// Returns the stable function export names in module order.
    #[must_use]
    pub fn exported_function_names(&self) -> Vec<String> {
        self.exports
            .iter()
            .filter(|export| export.kind == TassadarNormalizedWasmExportKind::Function)
            .map(|export| export.export_name.clone())
            .collect()
    }

    /// Validates internal consistency for the normalized module.
    pub fn validate_internal_consistency(&self) -> Result<(), TassadarNormalizedWasmModuleError> {
        for (expected_index, ty) in self.types.iter().enumerate() {
            if ty.type_index != expected_index as u32 {
                return Err(TassadarNormalizedWasmModuleError::NonCanonicalTypeIndex {
                    expected: expected_index as u32,
                    actual: ty.type_index,
                });
            }
        }
        for (expected_index, function) in self.functions.iter().enumerate() {
            if function.function_index != expected_index as u32 {
                return Err(
                    TassadarNormalizedWasmModuleError::NonCanonicalFunctionIndex {
                        expected: expected_index as u32,
                        actual: function.function_index,
                    },
                );
            }
            if function.type_index as usize >= self.types.len() {
                return Err(
                    TassadarNormalizedWasmModuleError::FunctionTypeIndexOutOfRange {
                        function_index: function.function_index,
                        type_index: function.type_index,
                        type_count: self.types.len(),
                    },
                );
            }
            match (
                function.import_module.as_ref(),
                function.import_name.as_ref(),
                function.body.as_ref(),
            ) {
                (Some(_), Some(_), None) | (None, None, Some(_)) => {}
                (Some(_), Some(_), Some(_)) => {
                    return Err(TassadarNormalizedWasmModuleError::ImportedFunctionHasBody {
                        function_index: function.function_index,
                    });
                }
                (None, None, None) => {
                    return Err(
                        TassadarNormalizedWasmModuleError::DefinedFunctionMissingBody {
                            function_index: function.function_index,
                        },
                    );
                }
                _ => {
                    return Err(
                        TassadarNormalizedWasmModuleError::InvalidFunctionImportShape {
                            function_index: function.function_index,
                        },
                    );
                }
            }
        }
        for (expected_index, memory) in self.memories.iter().enumerate() {
            if memory.memory_index != expected_index as u32 {
                return Err(TassadarNormalizedWasmModuleError::NonCanonicalMemoryIndex {
                    expected: expected_index as u32,
                    actual: memory.memory_index,
                });
            }
            match (memory.import_module.as_ref(), memory.import_name.as_ref()) {
                (Some(_), Some(_)) | (None, None) => {}
                _ => {
                    return Err(
                        TassadarNormalizedWasmModuleError::InvalidMemoryImportShape {
                            memory_index: memory.memory_index,
                        },
                    );
                }
            }
        }
        for (expected_index, table) in self.tables.iter().enumerate() {
            if table.table_index != expected_index as u32 {
                return Err(TassadarNormalizedWasmModuleError::NonCanonicalTableIndex {
                    expected: expected_index as u32,
                    actual: table.table_index,
                });
            }
        }
        for (expected_index, global) in self.globals.iter().enumerate() {
            if global.global_index != expected_index as u32 {
                return Err(TassadarNormalizedWasmModuleError::NonCanonicalGlobalIndex {
                    expected: expected_index as u32,
                    actual: global.global_index,
                });
            }
            if let TassadarNormalizedWasmConstExpr::GlobalGet { global_index } = global.init_expr
                && global_index as usize >= self.globals.len()
            {
                return Err(
                    TassadarNormalizedWasmModuleError::GlobalInitIndexOutOfRange {
                        global_index: global.global_index,
                        referenced_global_index: global_index,
                        global_count: self.globals.len(),
                    },
                );
            }
        }
        for export in &self.exports {
            match export.kind {
                TassadarNormalizedWasmExportKind::Function
                    if export.index as usize >= self.functions.len() =>
                {
                    return Err(
                        TassadarNormalizedWasmModuleError::ExportFunctionIndexOutOfRange {
                            export_name: export.export_name.clone(),
                            function_index: export.index,
                            function_count: self.functions.len(),
                        },
                    );
                }
                TassadarNormalizedWasmExportKind::Memory
                    if export.index as usize >= self.memories.len() =>
                {
                    return Err(
                        TassadarNormalizedWasmModuleError::ExportMemoryIndexOutOfRange {
                            export_name: export.export_name.clone(),
                            memory_index: export.index,
                            memory_count: self.memories.len(),
                        },
                    );
                }
                _ => {}
            }
        }
        if let Some(start_function_index) = self.start_function_index
            && start_function_index as usize >= self.functions.len()
        {
            return Err(
                TassadarNormalizedWasmModuleError::StartFunctionIndexOutOfRange {
                    start_function_index,
                    function_count: self.functions.len(),
                },
            );
        }
        for (expected_index, segment) in self.element_segments.iter().enumerate() {
            if segment.element_index != expected_index as u32 {
                return Err(
                    TassadarNormalizedWasmModuleError::NonCanonicalElementIndex {
                        expected: expected_index as u32,
                        actual: segment.element_index,
                    },
                );
            }
            if segment.table_index as usize >= self.tables.len() {
                return Err(
                    TassadarNormalizedWasmModuleError::ElementTableIndexOutOfRange {
                        element_index: segment.element_index,
                        table_index: segment.table_index,
                        table_count: self.tables.len(),
                    },
                );
            }
            for function_index in &segment.function_indices {
                if *function_index as usize >= self.functions.len() {
                    return Err(
                        TassadarNormalizedWasmModuleError::ElementFunctionIndexOutOfRange {
                            element_index: segment.element_index,
                            function_index: *function_index,
                            function_count: self.functions.len(),
                        },
                    );
                }
            }
        }
        for (expected_index, segment) in self.data_segments.iter().enumerate() {
            if segment.data_index != expected_index as u32 {
                return Err(TassadarNormalizedWasmModuleError::NonCanonicalDataIndex {
                    expected: expected_index as u32,
                    actual: segment.data_index,
                });
            }
            if let TassadarNormalizedWasmDataMode::Active { memory_index, .. } = segment.mode {
                if memory_index as usize >= self.memories.len() {
                    return Err(
                        TassadarNormalizedWasmModuleError::DataMemoryIndexOutOfRange {
                            data_index: segment.data_index,
                            memory_index,
                            memory_count: self.memories.len(),
                        },
                    );
                }
            }
        }
        for function in self
            .functions
            .iter()
            .filter(|function| function.body.is_some())
        {
            let body = function.body.as_ref().expect("filtered above");
            for instruction in &body.instructions {
                match instruction {
                    TassadarNormalizedWasmInstruction::Call { function_index }
                        if *function_index as usize >= self.functions.len() =>
                    {
                        return Err(
                            TassadarNormalizedWasmModuleError::InstructionFunctionIndexOutOfRange {
                                function_index: function.function_index,
                                referenced_function_index: *function_index,
                                referenced_function_count: self.functions.len(),
                            },
                        );
                    }
                    TassadarNormalizedWasmInstruction::GlobalGet { global_index }
                    | TassadarNormalizedWasmInstruction::GlobalSet { global_index }
                        if *global_index as usize >= self.globals.len() =>
                    {
                        return Err(
                            TassadarNormalizedWasmModuleError::InstructionGlobalIndexOutOfRange {
                                function_index: function.function_index,
                                global_index: *global_index,
                                global_count: self.globals.len(),
                            },
                        );
                    }
                    TassadarNormalizedWasmInstruction::I32Load { memory_index, .. }
                    | TassadarNormalizedWasmInstruction::I32Store { memory_index, .. }
                    | TassadarNormalizedWasmInstruction::MemorySize { memory_index }
                    | TassadarNormalizedWasmInstruction::MemoryGrow { memory_index }
                    | TassadarNormalizedWasmInstruction::MemoryFill { memory_index }
                        if *memory_index as usize >= self.memories.len() =>
                    {
                        return Err(
                            TassadarNormalizedWasmModuleError::InstructionMemoryIndexOutOfRange {
                                function_index: function.function_index,
                                memory_index: *memory_index,
                                memory_count: self.memories.len(),
                            },
                        );
                    }
                    TassadarNormalizedWasmInstruction::MemoryCopy {
                        dst_memory_index,
                        src_memory_index,
                    } if *dst_memory_index as usize >= self.memories.len()
                        || *src_memory_index as usize >= self.memories.len() =>
                    {
                        return Err(
                            TassadarNormalizedWasmModuleError::InstructionMemoryIndexOutOfRange {
                                function_index: function.function_index,
                                memory_index: (*dst_memory_index).max(*src_memory_index),
                                memory_count: self.memories.len(),
                            },
                        );
                    }
                    TassadarNormalizedWasmInstruction::CallIndirect {
                        type_index,
                        table_index,
                    } => {
                        if *type_index as usize >= self.types.len() {
                            return Err(
                                TassadarNormalizedWasmModuleError::FunctionTypeIndexOutOfRange {
                                    function_index: function.function_index,
                                    type_index: *type_index,
                                    type_count: self.types.len(),
                                },
                            );
                        }
                        if *table_index as usize >= self.tables.len() {
                            return Err(
                                TassadarNormalizedWasmModuleError::InstructionTableIndexOutOfRange {
                                    function_index: function.function_index,
                                    table_index: *table_index,
                                    table_count: self.tables.len(),
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// Typed failure while parsing, validating, or encoding the normalized Wasm IR.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarNormalizedWasmModuleError {
    /// The input was not a valid Wasm module.
    #[error("malformed Wasm module: {message}")]
    MalformedBinary {
        /// Human-readable parse failure.
        message: String,
    },
    /// The input was a Wasm component or other unsupported encoding.
    #[error("unsupported Wasm encoding `{encoding}`")]
    UnsupportedEncoding {
        /// Unsupported encoding label.
        encoding: String,
    },
    /// One section is outside the bounded normalized module surface.
    #[error("unsupported Wasm section `{section}` in bounded normalized module ingress")]
    UnsupportedSection {
        /// Unsupported section label.
        section: String,
    },
    /// One import kind is outside the bounded normalized module surface.
    #[error("unsupported import kind `{kind}` for `{module}::{name}`")]
    UnsupportedImportKind {
        /// Import module label.
        module: String,
        /// Import item label.
        name: String,
        /// Unsupported import kind.
        kind: String,
    },
    /// One export kind is outside the bounded normalized module surface.
    #[error("unsupported export kind `{kind}` for export `{export_name}`")]
    UnsupportedExportKind {
        /// Export label.
        export_name: String,
        /// Unsupported export kind.
        kind: String,
    },
    /// One value type is outside the bounded normalized module surface.
    #[error("unsupported Wasm value type `{type_name}`")]
    UnsupportedValueType {
        /// Unsupported type label.
        type_name: String,
    },
    /// One constant expression is outside the bounded normalized module surface.
    #[error("unsupported constant expression for `{context}`: {detail}")]
    UnsupportedConstExpr {
        /// Context such as `data_segment_0`.
        context: String,
        /// Unsupported-expression detail.
        detail: String,
    },
    /// One instruction is outside the bounded normalized module surface.
    #[error("unsupported Wasm instruction `{opcode}` in function {function_index}")]
    UnsupportedInstruction {
        /// Function index.
        function_index: u32,
        /// Unsupported opcode mnemonic.
        opcode: String,
    },
    /// The function section and code section did not agree on arity.
    #[error(
        "function section declared {declared_function_count} defined functions, but code section provided {body_count} bodies"
    )]
    FunctionBodyCountMismatch {
        /// Defined function count from the function section.
        declared_function_count: usize,
        /// Bodies observed in the code section.
        body_count: usize,
    },
    /// One type row drifted from canonical sequential indexing.
    #[error("non-canonical type index: expected {expected}, got {actual}")]
    NonCanonicalTypeIndex {
        /// Expected sequential index.
        expected: u32,
        /// Actual stored index.
        actual: u32,
    },
    /// One function row drifted from canonical sequential indexing.
    #[error("non-canonical function index: expected {expected}, got {actual}")]
    NonCanonicalFunctionIndex {
        /// Expected sequential index.
        expected: u32,
        /// Actual stored index.
        actual: u32,
    },
    /// One memory row drifted from canonical sequential indexing.
    #[error("non-canonical memory index: expected {expected}, got {actual}")]
    NonCanonicalMemoryIndex {
        /// Expected sequential index.
        expected: u32,
        /// Actual stored index.
        actual: u32,
    },
    /// One table row drifted from canonical sequential indexing.
    #[error("non-canonical table index: expected {expected}, got {actual}")]
    NonCanonicalTableIndex { expected: u32, actual: u32 },
    /// One global row drifted from canonical sequential indexing.
    #[error("non-canonical global index: expected {expected}, got {actual}")]
    NonCanonicalGlobalIndex { expected: u32, actual: u32 },
    /// One element-segment row drifted from canonical sequential indexing.
    #[error("non-canonical element index: expected {expected}, got {actual}")]
    NonCanonicalElementIndex { expected: u32, actual: u32 },
    /// One data row drifted from canonical sequential indexing.
    #[error("non-canonical data index: expected {expected}, got {actual}")]
    NonCanonicalDataIndex {
        /// Expected sequential index.
        expected: u32,
        /// Actual stored index.
        actual: u32,
    },
    /// One function referenced an unknown type.
    #[error(
        "function {function_index} references type {type_index}, but only {type_count} types are present"
    )]
    FunctionTypeIndexOutOfRange {
        /// Function index.
        function_index: u32,
        /// Referenced type index.
        type_index: u32,
        /// Type count in the module.
        type_count: usize,
    },
    /// One imported function incorrectly carried a body.
    #[error("imported function {function_index} cannot also carry a body")]
    ImportedFunctionHasBody {
        /// Function index.
        function_index: u32,
    },
    /// One defined function omitted its body.
    #[error("defined function {function_index} is missing its body")]
    DefinedFunctionMissingBody {
        /// Function index.
        function_index: u32,
    },
    /// One function import row had an invalid field combination.
    #[error("function {function_index} has an invalid import/body shape")]
    InvalidFunctionImportShape {
        /// Function index.
        function_index: u32,
    },
    /// One memory import row had an invalid field combination.
    #[error("memory {memory_index} has an invalid import shape")]
    InvalidMemoryImportShape {
        /// Memory index.
        memory_index: u32,
    },
    /// One function export referenced a missing function.
    #[error(
        "function export `{export_name}` references function {function_index}, but only {function_count} functions are present"
    )]
    ExportFunctionIndexOutOfRange {
        /// Export label.
        export_name: String,
        /// Referenced function index.
        function_index: u32,
        /// Function count in the module.
        function_count: usize,
    },
    /// One memory export referenced a missing memory.
    #[error(
        "memory export `{export_name}` references memory {memory_index}, but only {memory_count} memories are present"
    )]
    ExportMemoryIndexOutOfRange {
        /// Export label.
        export_name: String,
        /// Referenced memory index.
        memory_index: u32,
        /// Memory count in the module.
        memory_count: usize,
    },
    /// One active data segment referenced a missing memory.
    #[error(
        "data segment {data_index} references memory {memory_index}, but only {memory_count} memories are present"
    )]
    DataMemoryIndexOutOfRange {
        /// Data segment index.
        data_index: u32,
        /// Referenced memory index.
        memory_index: u32,
        /// Memory count in the module.
        memory_count: usize,
    },
    /// One instruction referenced a missing memory.
    #[error(
        "function {function_index} references memory {memory_index}, but only {memory_count} memories are present"
    )]
    InstructionMemoryIndexOutOfRange {
        function_index: u32,
        memory_index: u32,
        memory_count: usize,
    },
    /// One global initializer referenced a missing global.
    #[error(
        "global {global_index} init expr references global {referenced_global_index}, but only {global_count} globals are present"
    )]
    GlobalInitIndexOutOfRange {
        global_index: u32,
        referenced_global_index: u32,
        global_count: usize,
    },
    /// The start function referenced a missing function.
    #[error(
        "start function {start_function_index} is out of range for function_count={function_count}"
    )]
    StartFunctionIndexOutOfRange {
        start_function_index: u32,
        function_count: usize,
    },
    /// One element segment referenced a missing table.
    #[error(
        "element segment {element_index} references table {table_index}, but only {table_count} tables are present"
    )]
    ElementTableIndexOutOfRange {
        element_index: u32,
        table_index: u32,
        table_count: usize,
    },
    /// One element segment referenced a missing function.
    #[error(
        "element segment {element_index} references function {function_index}, but only {function_count} functions are present"
    )]
    ElementFunctionIndexOutOfRange {
        element_index: u32,
        function_index: u32,
        function_count: usize,
    },
    /// One instruction referenced a missing function.
    #[error(
        "function {function_index} references function {referenced_function_index}, but only {referenced_function_count} functions are present"
    )]
    InstructionFunctionIndexOutOfRange {
        function_index: u32,
        referenced_function_index: u32,
        referenced_function_count: usize,
    },
    /// One instruction referenced a missing global.
    #[error(
        "function {function_index} references global {global_index}, but only {global_count} globals are present"
    )]
    InstructionGlobalIndexOutOfRange {
        function_index: u32,
        global_index: u32,
        global_count: usize,
    },
    /// One instruction referenced a missing table.
    #[error(
        "function {function_index} references table {table_index}, but only {table_count} tables are present"
    )]
    InstructionTableIndexOutOfRange {
        function_index: u32,
        table_index: u32,
        table_count: usize,
    },
}

/// Parses one Wasm binary into the normalized module IR.
pub fn parse_tassadar_normalized_wasm_module(
    bytes: &[u8],
) -> Result<TassadarNormalizedWasmModule, TassadarNormalizedWasmModuleError> {
    let mut types = Vec::new();
    let mut functions = Vec::new();
    let mut tables = Vec::new();
    let mut memories = Vec::new();
    let mut globals = Vec::new();
    let mut exports = Vec::new();
    let mut declared_function_type_indices = Vec::new();
    let mut code_bodies = Vec::new();
    let mut start_function_index = None;
    let mut element_segments = Vec::new();
    let mut data_segments = Vec::new();

    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(binary_error)? {
            Payload::Version { encoding, .. } => {
                if encoding != Encoding::Module {
                    return Err(TassadarNormalizedWasmModuleError::UnsupportedEncoding {
                        encoding: format!("{encoding:?}"),
                    });
                }
            }
            Payload::TypeSection(reader) => {
                for (type_index, func_type) in reader.into_iter_err_on_gc_types().enumerate() {
                    let func_type = func_type.map_err(binary_error)?;
                    types.push(TassadarNormalizedWasmFunctionType {
                        type_index: type_index as u32,
                        params: func_type
                            .params()
                            .iter()
                            .copied()
                            .map(normalize_value_type)
                            .collect::<Result<Vec<_>, _>>()?,
                        results: func_type
                            .results()
                            .iter()
                            .copied()
                            .map(normalize_value_type)
                            .collect::<Result<Vec<_>, _>>()?,
                    });
                }
            }
            Payload::ImportSection(reader) => {
                for import_group in reader {
                    let import_group = import_group.map_err(binary_error)?;
                    match import_group {
                        wasmparser::Imports::Single(_, import) => {
                            normalize_import(
                                import.module,
                                import.name,
                                import.ty,
                                &mut functions,
                                &mut memories,
                            )?;
                        }
                        wasmparser::Imports::Compact1 { module, items } => {
                            for item in items {
                                let item = item.map_err(binary_error)?;
                                normalize_import(
                                    module,
                                    item.name,
                                    item.ty,
                                    &mut functions,
                                    &mut memories,
                                )?;
                            }
                        }
                        wasmparser::Imports::Compact2 { module, ty, names } => {
                            for name in names {
                                let name = name.map_err(binary_error)?;
                                normalize_import(module, name, ty, &mut functions, &mut memories)?;
                            }
                        }
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for function_type_index in reader {
                    declared_function_type_indices.push(function_type_index.map_err(binary_error)?);
                }
            }
            Payload::TableSection(reader) => {
                for table in reader {
                    let table = table.map_err(binary_error)?;
                    tables.push(normalize_table(table, tables.len() as u32)?);
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.map_err(binary_error)?;
                    let memory_index = memories.len() as u32;
                    memories.push(TassadarNormalizedWasmMemory::defined(
                        memory_index,
                        normalize_memory_type(memory),
                    ));
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let global = global.map_err(binary_error)?;
                    globals.push(normalize_global(global, globals.len() as u32)?);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(binary_error)?;
                    let kind = match export.kind {
                        ExternalKind::Func => TassadarNormalizedWasmExportKind::Function,
                        ExternalKind::Memory => TassadarNormalizedWasmExportKind::Memory,
                        other => {
                            return Err(TassadarNormalizedWasmModuleError::UnsupportedExportKind {
                                export_name: export.name.to_string(),
                                kind: format!("{other:?}"),
                            });
                        }
                    };
                    exports.push(TassadarNormalizedWasmExport::new(
                        export.name,
                        kind,
                        export.index,
                    ));
                }
            }
            Payload::StartSection { func, .. } => {
                start_function_index = Some(func);
            }
            Payload::ElementSection(reader) => {
                for element in reader {
                    let element = element.map_err(binary_error)?;
                    element_segments.push(normalize_element_segment(
                        element,
                        element_segments.len() as u32,
                    )?);
                }
            }
            Payload::CodeSectionEntry(body) => {
                code_bodies.push(parse_function_body(
                    functions.len() as u32 + code_bodies.len() as u32,
                    body,
                )?);
            }
            Payload::DataSection(reader) => {
                for data in reader {
                    let data = data.map_err(binary_error)?;
                    let mode = match data.kind {
                        DataKind::Passive => TassadarNormalizedWasmDataMode::Passive,
                        DataKind::Active {
                            memory_index,
                            offset_expr,
                        } => TassadarNormalizedWasmDataMode::Active {
                            memory_index,
                            offset_expr: parse_const_expr(
                                &offset_expr,
                                format!("data_segment_{}", data_segments.len()),
                            )?,
                        },
                    };
                    data_segments.push(TassadarNormalizedWasmDataSegment::new(
                        data_segments.len() as u32,
                        mode,
                        data.data.to_vec(),
                    ));
                }
            }
            Payload::CustomSection(_) | Payload::CodeSectionStart { .. } | Payload::End(_) => {}
            Payload::TagSection(_) => {
                return Err(TassadarNormalizedWasmModuleError::UnsupportedSection {
                    section: String::from("tag"),
                });
            }
            Payload::DataCountSection { .. } => {
                return Err(TassadarNormalizedWasmModuleError::UnsupportedSection {
                    section: String::from("data_count"),
                });
            }
            other => {
                return Err(TassadarNormalizedWasmModuleError::UnsupportedSection {
                    section: format!("{other:?}"),
                });
            }
        }
    }

    if declared_function_type_indices.len() != code_bodies.len() {
        return Err(
            TassadarNormalizedWasmModuleError::FunctionBodyCountMismatch {
                declared_function_count: declared_function_type_indices.len(),
                body_count: code_bodies.len(),
            },
        );
    }

    for (type_index, body) in declared_function_type_indices.into_iter().zip(code_bodies) {
        functions.push(TassadarNormalizedWasmFunction::defined(
            functions.len() as u32,
            type_index,
            body,
        ));
    }

    TassadarNormalizedWasmModule::new_full(
        types,
        functions,
        tables,
        memories,
        globals,
        exports,
        start_function_index,
        element_segments,
        data_segments,
    )
}

/// Encodes one validated normalized module back into canonical Wasm bytes.
pub fn encode_tassadar_normalized_wasm_module(
    module: &TassadarNormalizedWasmModule,
) -> Result<Vec<u8>, TassadarNormalizedWasmModuleError> {
    module.validate_internal_consistency()?;

    let mut encoded = Module::new();

    let mut type_section = TypeSection::new();
    for ty in &module.types {
        type_section.ty().function(
            ty.params
                .iter()
                .copied()
                .map(encode_value_type)
                .collect::<Result<Vec<_>, _>>()?,
            ty.results
                .iter()
                .copied()
                .map(encode_value_type)
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    if !type_section.is_empty() {
        encoded.section(&type_section);
    }

    let mut import_section = ImportSection::new();
    for function in module
        .functions
        .iter()
        .filter(|function| function.imported_function())
    {
        import_section.import(
            function.import_module.as_deref().unwrap_or_default(),
            function.import_name.as_deref().unwrap_or_default(),
            EntityType::Function(function.type_index),
        );
    }
    for memory in module
        .memories
        .iter()
        .filter(|memory| memory.imported_memory())
    {
        import_section.import(
            memory.import_module.as_deref().unwrap_or_default(),
            memory.import_name.as_deref().unwrap_or_default(),
            encode_memory_type(&memory.memory_type),
        );
    }
    if !import_section.is_empty() {
        encoded.section(&import_section);
    }

    let mut function_section = FunctionSection::new();
    for function in module
        .functions
        .iter()
        .filter(|function| !function.imported_function())
    {
        function_section.function(function.type_index);
    }
    if !function_section.is_empty() {
        encoded.section(&function_section);
    }

    let mut table_section = TableSection::new();
    for table in &module.tables {
        table_section.table(encode_table_type(table)?);
    }
    if !table_section.is_empty() {
        encoded.section(&table_section);
    }

    let mut memory_section = MemorySection::new();
    for memory in module
        .memories
        .iter()
        .filter(|memory| !memory.imported_memory())
    {
        memory_section.memory(encode_memory_type(&memory.memory_type));
    }
    if !memory_section.is_empty() {
        encoded.section(&memory_section);
    }

    let mut global_section = GlobalSection::new();
    for global in &module.globals {
        global_section.global(
            encode_global_type(global)?,
            &encode_const_expr(&global.init_expr),
        );
    }
    if !global_section.is_empty() {
        encoded.section(&global_section);
    }

    let mut export_section = ExportSection::new();
    for export in &module.exports {
        export_section.export(
            export.export_name.as_str(),
            match export.kind {
                TassadarNormalizedWasmExportKind::Function => ExportKind::Func,
                TassadarNormalizedWasmExportKind::Memory => ExportKind::Memory,
            },
            export.index,
        );
    }
    if !export_section.is_empty() {
        encoded.section(&export_section);
    }

    if let Some(start_function_index) = module.start_function_index {
        encoded.section(&StartSection {
            function_index: start_function_index,
        });
    }

    let mut element_section = ElementSection::new();
    for element in &module.element_segments {
        element_section.segment(wasm_encoder::ElementSegment {
            mode: ElementMode::Active {
                table: Some(element.table_index),
                offset: &encode_const_expr(&element.offset_expr),
            },
            elements: Elements::Functions(std::borrow::Cow::Owned(
                element.function_indices.clone(),
            )),
        });
    }
    if !element_section.is_empty() {
        encoded.section(&element_section);
    }

    let mut code_section = CodeSection::new();
    for function in module
        .functions
        .iter()
        .filter(|function| !function.imported_function())
    {
        let body = function
            .body
            .as_ref()
            .expect("validated defined function should have a body");
        let mut encoded_function = Function::new_with_locals_types(
            body.locals
                .iter()
                .copied()
                .map(encode_value_type)
                .collect::<Result<Vec<_>, _>>()?,
        );
        for instruction in &body.instructions {
            encode_instruction(instruction, &mut encoded_function);
        }
        encoded_function.instruction(&Instruction::End);
        code_section.function(&encoded_function);
    }
    if !code_section.is_empty() {
        encoded.section(&code_section);
    }

    let mut data_section = DataSection::new();
    for data_segment in &module.data_segments {
        let mode = match &data_segment.mode {
            TassadarNormalizedWasmDataMode::Passive => DataSegmentMode::Passive,
            TassadarNormalizedWasmDataMode::Active {
                memory_index,
                offset_expr,
            } => DataSegmentMode::Active {
                memory_index: *memory_index,
                offset: &encode_const_expr(offset_expr),
            },
        };
        data_section.segment(DataSegment {
            mode,
            data: data_segment.bytes.iter().copied(),
        });
    }
    if !data_section.is_empty() {
        encoded.section(&data_section);
    }

    Ok(encoded.finish())
}

/// Returns one canonical seeded multi-function Wasm module for bounded
/// section-level round-trips and exact lowering tests.
pub fn tassadar_seeded_multi_function_module()
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
                    vec![TassadarNormalizedWasmValueType::I32],
                    vec![
                        TassadarNormalizedWasmInstruction::I32Const { value: 7 },
                        TassadarNormalizedWasmInstruction::LocalTee { local_index: 0 },
                        TassadarNormalizedWasmInstruction::I32Const { value: 2 },
                        TassadarNormalizedWasmInstruction::I32Mul,
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
            TassadarNormalizedWasmExport::new(
                "pair_sum",
                TassadarNormalizedWasmExportKind::Function,
                0,
            ),
            TassadarNormalizedWasmExport::new(
                "local_double",
                TassadarNormalizedWasmExportKind::Function,
                1,
            ),
            TassadarNormalizedWasmExport::new(
                "memory",
                TassadarNormalizedWasmExportKind::Memory,
                0,
            ),
        ],
        vec![TassadarNormalizedWasmDataSegment::new(
            0,
            TassadarNormalizedWasmDataMode::Active {
                memory_index: 0,
                offset_expr: TassadarNormalizedWasmConstExpr::I32Const { value: 0 },
            },
            vec![2, 0, 0, 0, 3, 0, 0, 0],
        )],
    )
}

/// Returns one canonical seeded module covering globals, tables, active
/// element segments, start, direct call, and indirect call.
pub fn tassadar_seeded_instantiation_module()
-> Result<TassadarNormalizedWasmModule, TassadarNormalizedWasmModuleError> {
    TassadarNormalizedWasmModule::new_full(
        vec![
            TassadarNormalizedWasmFunctionType {
                type_index: 0,
                params: Vec::new(),
                results: vec![TassadarNormalizedWasmValueType::I32],
            },
            TassadarNormalizedWasmFunctionType {
                type_index: 1,
                params: Vec::new(),
                results: Vec::new(),
            },
        ],
        vec![
            TassadarNormalizedWasmFunction::defined(
                0,
                0,
                TassadarNormalizedWasmFunctionBody::new(
                    Vec::new(),
                    vec![
                        TassadarNormalizedWasmInstruction::GlobalGet { global_index: 0 },
                        TassadarNormalizedWasmInstruction::I32Const { value: 1 },
                        TassadarNormalizedWasmInstruction::CallIndirect {
                            type_index: 0,
                            table_index: 0,
                        },
                        TassadarNormalizedWasmInstruction::I32Add,
                        TassadarNormalizedWasmInstruction::Return,
                    ],
                ),
            ),
            TassadarNormalizedWasmFunction::defined(
                1,
                1,
                TassadarNormalizedWasmFunctionBody::new(
                    Vec::new(),
                    vec![
                        TassadarNormalizedWasmInstruction::Call { function_index: 4 },
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
                        TassadarNormalizedWasmInstruction::I32Const { value: 9 },
                        TassadarNormalizedWasmInstruction::Return,
                    ],
                ),
            ),
            TassadarNormalizedWasmFunction::defined(
                3,
                0,
                TassadarNormalizedWasmFunctionBody::new(
                    Vec::new(),
                    vec![
                        TassadarNormalizedWasmInstruction::I32Const { value: 11 },
                        TassadarNormalizedWasmInstruction::Return,
                    ],
                ),
            ),
            TassadarNormalizedWasmFunction::defined(
                4,
                1,
                TassadarNormalizedWasmFunctionBody::new(
                    Vec::new(),
                    vec![
                        TassadarNormalizedWasmInstruction::I32Const { value: 31 },
                        TassadarNormalizedWasmInstruction::GlobalSet { global_index: 0 },
                        TassadarNormalizedWasmInstruction::Return,
                    ],
                ),
            ),
        ],
        vec![TassadarNormalizedWasmTable {
            table_index: 0,
            element_kind: TassadarNormalizedWasmTableElementKind::Funcref,
            min_entries: 3,
            max_entries: Some(3),
        }],
        Vec::new(),
        vec![TassadarNormalizedWasmGlobal {
            global_index: 0,
            value_type: TassadarNormalizedWasmValueType::I32,
            mutability: TassadarNormalizedWasmGlobalMutability::Mutable,
            init_expr: TassadarNormalizedWasmConstExpr::I32Const { value: 1 },
        }],
        vec![TassadarNormalizedWasmExport::new(
            "entry",
            TassadarNormalizedWasmExportKind::Function,
            0,
        )],
        Some(1),
        vec![TassadarNormalizedWasmElementSegment {
            element_index: 0,
            table_index: 0,
            offset_expr: TassadarNormalizedWasmConstExpr::I32Const { value: 0 },
            function_indices: vec![2, 3, 2],
        }],
        Vec::new(),
    )
}

/// Returns one canonical seeded module covering bounded dynamic memory,
/// active data segments, `memory.copy`, `memory.fill`, `memory.size`, and
/// `memory.grow`.
pub fn tassadar_seeded_dynamic_memory_module()
-> Result<TassadarNormalizedWasmModule, TassadarNormalizedWasmModuleError> {
    TassadarNormalizedWasmModule::new(
        vec![TassadarNormalizedWasmFunctionType {
            type_index: 0,
            params: Vec::new(),
            results: vec![TassadarNormalizedWasmValueType::I32],
        }],
        vec![TassadarNormalizedWasmFunction::defined(
            0,
            0,
            TassadarNormalizedWasmFunctionBody::new(
                vec![
                    TassadarNormalizedWasmValueType::I32,
                    TassadarNormalizedWasmValueType::I32,
                    TassadarNormalizedWasmValueType::I32,
                ],
                vec![
                    TassadarNormalizedWasmInstruction::I32Const { value: 8 },
                    TassadarNormalizedWasmInstruction::I32Const { value: 0 },
                    TassadarNormalizedWasmInstruction::I32Const { value: 4 },
                    TassadarNormalizedWasmInstruction::MemoryCopy {
                        dst_memory_index: 0,
                        src_memory_index: 0,
                    },
                    TassadarNormalizedWasmInstruction::MemorySize { memory_index: 0 },
                    TassadarNormalizedWasmInstruction::LocalSet { local_index: 0 },
                    TassadarNormalizedWasmInstruction::I32Const { value: 1 },
                    TassadarNormalizedWasmInstruction::MemoryGrow { memory_index: 0 },
                    TassadarNormalizedWasmInstruction::LocalSet { local_index: 1 },
                    TassadarNormalizedWasmInstruction::MemorySize { memory_index: 0 },
                    TassadarNormalizedWasmInstruction::LocalSet { local_index: 2 },
                    TassadarNormalizedWasmInstruction::I32Const { value: 12 },
                    TassadarNormalizedWasmInstruction::I32Const { value: 7 },
                    TassadarNormalizedWasmInstruction::I32Const { value: 4 },
                    TassadarNormalizedWasmInstruction::MemoryFill { memory_index: 0 },
                    TassadarNormalizedWasmInstruction::I32Const { value: 8 },
                    TassadarNormalizedWasmInstruction::I32Load {
                        align: 2,
                        offset: 0,
                        memory_index: 0,
                    },
                    TassadarNormalizedWasmInstruction::I32Const { value: 12 },
                    TassadarNormalizedWasmInstruction::I32Load {
                        align: 2,
                        offset: 0,
                        memory_index: 0,
                    },
                    TassadarNormalizedWasmInstruction::I32Add,
                    TassadarNormalizedWasmInstruction::LocalGet { local_index: 0 },
                    TassadarNormalizedWasmInstruction::I32Add,
                    TassadarNormalizedWasmInstruction::LocalGet { local_index: 1 },
                    TassadarNormalizedWasmInstruction::I32Add,
                    TassadarNormalizedWasmInstruction::LocalGet { local_index: 2 },
                    TassadarNormalizedWasmInstruction::I32Add,
                    TassadarNormalizedWasmInstruction::Return,
                ],
            ),
        )],
        vec![TassadarNormalizedWasmMemory::defined(
            0,
            TassadarNormalizedWasmMemoryType {
                minimum_pages: 1,
                maximum_pages: Some(3),
                shared: false,
                memory64: false,
                page_size_log2: None,
            },
        )],
        vec![
            TassadarNormalizedWasmExport::new(
                "entry",
                TassadarNormalizedWasmExportKind::Function,
                0,
            ),
            TassadarNormalizedWasmExport::new(
                "memory",
                TassadarNormalizedWasmExportKind::Memory,
                0,
            ),
        ],
        vec![TassadarNormalizedWasmDataSegment::new(
            0,
            TassadarNormalizedWasmDataMode::Active {
                memory_index: 0,
                offset_expr: TassadarNormalizedWasmConstExpr::I32Const { value: 0 },
            },
            vec![1, 2, 3, 4],
        )],
    )
}

fn normalize_import(
    module: &str,
    name: &str,
    ty: TypeRef,
    functions: &mut Vec<TassadarNormalizedWasmFunction>,
    memories: &mut Vec<TassadarNormalizedWasmMemory>,
) -> Result<(), TassadarNormalizedWasmModuleError> {
    match ty {
        TypeRef::Func(type_index) | TypeRef::FuncExact(type_index) => {
            functions.push(TassadarNormalizedWasmFunction::imported(
                functions.len() as u32,
                type_index,
                module,
                name,
            ));
            Ok(())
        }
        TypeRef::Memory(memory_type) => {
            memories.push(TassadarNormalizedWasmMemory::imported(
                memories.len() as u32,
                normalize_memory_type(memory_type),
                module,
                name,
            ));
            Ok(())
        }
        TypeRef::Table(_) => Err(TassadarNormalizedWasmModuleError::UnsupportedImportKind {
            module: module.to_string(),
            name: name.to_string(),
            kind: String::from("table"),
        }),
        TypeRef::Global(_) => Err(TassadarNormalizedWasmModuleError::UnsupportedImportKind {
            module: module.to_string(),
            name: name.to_string(),
            kind: String::from("global"),
        }),
        TypeRef::Tag(_) => Err(TassadarNormalizedWasmModuleError::UnsupportedImportKind {
            module: module.to_string(),
            name: name.to_string(),
            kind: String::from("tag"),
        }),
    }
}

fn parse_function_body(
    function_index: u32,
    body: wasmparser::FunctionBody<'_>,
) -> Result<TassadarNormalizedWasmFunctionBody, TassadarNormalizedWasmModuleError> {
    let locals = body
        .get_locals_reader()
        .map_err(binary_error)?
        .into_iter()
        .map(|local| {
            let (count, value_type) = local.map_err(binary_error)?;
            let normalized = normalize_value_type(value_type)?;
            Ok(std::iter::repeat_n(normalized, count as usize).collect::<Vec<_>>())
        })
        .collect::<Result<Vec<_>, TassadarNormalizedWasmModuleError>>()?
        .into_iter()
        .flatten()
        .collect();

    let mut instructions = Vec::new();
    let mut operators = body.get_operators_reader().map_err(binary_error)?;
    while !operators.eof() {
        let operator = operators.read().map_err(binary_error)?;
        match operator {
            Operator::End => break,
            Operator::I32Const { value } => {
                instructions.push(TassadarNormalizedWasmInstruction::I32Const { value });
            }
            Operator::LocalGet { local_index } => {
                instructions.push(TassadarNormalizedWasmInstruction::LocalGet { local_index });
            }
            Operator::LocalSet { local_index } => {
                instructions.push(TassadarNormalizedWasmInstruction::LocalSet { local_index });
            }
            Operator::LocalTee { local_index } => {
                instructions.push(TassadarNormalizedWasmInstruction::LocalTee { local_index });
            }
            Operator::GlobalGet { global_index } => {
                instructions.push(TassadarNormalizedWasmInstruction::GlobalGet { global_index });
            }
            Operator::GlobalSet { global_index } => {
                instructions.push(TassadarNormalizedWasmInstruction::GlobalSet { global_index });
            }
            Operator::I32Add => instructions.push(TassadarNormalizedWasmInstruction::I32Add),
            Operator::I32Sub => instructions.push(TassadarNormalizedWasmInstruction::I32Sub),
            Operator::I32Mul => instructions.push(TassadarNormalizedWasmInstruction::I32Mul),
            Operator::I32Shl => instructions.push(TassadarNormalizedWasmInstruction::I32Shl),
            Operator::I32LtS => instructions.push(TassadarNormalizedWasmInstruction::I32LtS),
            Operator::I32Load { memarg } => {
                instructions.push(TassadarNormalizedWasmInstruction::I32Load {
                    align: memarg.align,
                    offset: memarg.offset,
                    memory_index: memarg.memory,
                });
            }
            Operator::I32Store { memarg } => {
                instructions.push(TassadarNormalizedWasmInstruction::I32Store {
                    align: memarg.align,
                    offset: memarg.offset,
                    memory_index: memarg.memory,
                });
            }
            Operator::MemorySize { mem, .. } => {
                instructions
                    .push(TassadarNormalizedWasmInstruction::MemorySize { memory_index: mem });
            }
            Operator::MemoryGrow { mem, .. } => {
                instructions
                    .push(TassadarNormalizedWasmInstruction::MemoryGrow { memory_index: mem });
            }
            Operator::MemoryCopy { dst_mem, src_mem } => {
                instructions.push(TassadarNormalizedWasmInstruction::MemoryCopy {
                    dst_memory_index: dst_mem,
                    src_memory_index: src_mem,
                });
            }
            Operator::MemoryFill { mem } => {
                instructions
                    .push(TassadarNormalizedWasmInstruction::MemoryFill { memory_index: mem });
            }
            Operator::Call { function_index } => {
                instructions.push(TassadarNormalizedWasmInstruction::Call { function_index });
            }
            Operator::CallIndirect {
                type_index,
                table_index,
            } => {
                instructions.push(TassadarNormalizedWasmInstruction::CallIndirect {
                    type_index,
                    table_index,
                });
            }
            Operator::Return => instructions.push(TassadarNormalizedWasmInstruction::Return),
            Operator::Drop => instructions.push(TassadarNormalizedWasmInstruction::Drop),
            other => {
                return Err(TassadarNormalizedWasmModuleError::UnsupportedInstruction {
                    function_index,
                    opcode: format!("{other:?}"),
                });
            }
        }
    }
    Ok(TassadarNormalizedWasmFunctionBody::new(
        locals,
        instructions,
    ))
}

fn parse_const_expr(
    expr: &wasmparser::ConstExpr<'_>,
    context: String,
) -> Result<TassadarNormalizedWasmConstExpr, TassadarNormalizedWasmModuleError> {
    let mut operators = expr.get_operators_reader();
    let first = operators.read().map_err(binary_error)?;
    let parsed = match first {
        Operator::I32Const { value } => TassadarNormalizedWasmConstExpr::I32Const { value },
        Operator::GlobalGet { global_index } => {
            TassadarNormalizedWasmConstExpr::GlobalGet { global_index }
        }
        other => {
            return Err(TassadarNormalizedWasmModuleError::UnsupportedConstExpr {
                context,
                detail: format!("{other:?}"),
            });
        }
    };
    match operators.read().map_err(binary_error)? {
        Operator::End => Ok(parsed),
        other => Err(TassadarNormalizedWasmModuleError::UnsupportedConstExpr {
            context,
            detail: format!("{other:?}"),
        }),
    }
}

fn normalize_value_type(
    value_type: wasmparser::ValType,
) -> Result<TassadarNormalizedWasmValueType, TassadarNormalizedWasmModuleError> {
    match value_type {
        wasmparser::ValType::I32 => Ok(TassadarNormalizedWasmValueType::I32),
        wasmparser::ValType::I64 => Ok(TassadarNormalizedWasmValueType::I64),
        wasmparser::ValType::F32 => Ok(TassadarNormalizedWasmValueType::F32),
        wasmparser::ValType::F64 => Ok(TassadarNormalizedWasmValueType::F64),
        other => Err(TassadarNormalizedWasmModuleError::UnsupportedValueType {
            type_name: format!("{other:?}"),
        }),
    }
}

fn encode_value_type(
    value_type: TassadarNormalizedWasmValueType,
) -> Result<ValType, TassadarNormalizedWasmModuleError> {
    Ok(match value_type {
        TassadarNormalizedWasmValueType::I32 => ValType::I32,
        TassadarNormalizedWasmValueType::I64 => ValType::I64,
        TassadarNormalizedWasmValueType::F32 => ValType::F32,
        TassadarNormalizedWasmValueType::F64 => ValType::F64,
    })
}

fn normalize_memory_type(memory_type: wasmparser::MemoryType) -> TassadarNormalizedWasmMemoryType {
    TassadarNormalizedWasmMemoryType {
        minimum_pages: memory_type.initial,
        maximum_pages: memory_type.maximum,
        shared: memory_type.shared,
        memory64: memory_type.memory64,
        page_size_log2: memory_type.page_size_log2,
    }
}

fn normalize_table(
    table: wasmparser::Table<'_>,
    table_index: u32,
) -> Result<TassadarNormalizedWasmTable, TassadarNormalizedWasmModuleError> {
    if !matches!(table.init, wasmparser::TableInit::RefNull) {
        return Err(TassadarNormalizedWasmModuleError::UnsupportedSection {
            section: String::from("table_init_expr"),
        });
    }
    if table.ty.element_type != RefType::FUNCREF || table.ty.table64 || table.ty.shared {
        return Err(TassadarNormalizedWasmModuleError::UnsupportedSection {
            section: String::from("table_type"),
        });
    }
    Ok(TassadarNormalizedWasmTable {
        table_index,
        element_kind: TassadarNormalizedWasmTableElementKind::Funcref,
        min_entries: table.ty.initial as u32,
        max_entries: table.ty.maximum.map(|value| value as u32),
    })
}

fn normalize_global(
    global: wasmparser::Global<'_>,
    global_index: u32,
) -> Result<TassadarNormalizedWasmGlobal, TassadarNormalizedWasmModuleError> {
    Ok(TassadarNormalizedWasmGlobal {
        global_index,
        value_type: normalize_value_type(global.ty.content_type)?,
        mutability: if global.ty.mutable {
            TassadarNormalizedWasmGlobalMutability::Mutable
        } else {
            TassadarNormalizedWasmGlobalMutability::Const
        },
        init_expr: parse_const_expr(&global.init_expr, format!("global_{global_index}"))?,
    })
}

fn normalize_element_segment(
    element: wasmparser::Element<'_>,
    element_index: u32,
) -> Result<TassadarNormalizedWasmElementSegment, TassadarNormalizedWasmModuleError> {
    let (table_index, offset_expr) = match element.kind {
        ElementKind::Active {
            table_index,
            offset_expr,
        } => (
            table_index.unwrap_or(0),
            parse_const_expr(&offset_expr, format!("element_{element_index}"))?,
        ),
        ElementKind::Passive | ElementKind::Declared => {
            return Err(TassadarNormalizedWasmModuleError::UnsupportedSection {
                section: String::from("passive_or_declared_element"),
            });
        }
    };
    let function_indices = match element.items {
        ElementItems::Functions(reader) => reader
            .into_iter()
            .map(|function_index| function_index.map_err(binary_error))
            .collect::<Result<Vec<_>, _>>()?,
        ElementItems::Expressions(_, _) => {
            return Err(TassadarNormalizedWasmModuleError::UnsupportedSection {
                section: String::from("element_expressions"),
            });
        }
    };
    Ok(TassadarNormalizedWasmElementSegment {
        element_index,
        table_index,
        offset_expr,
        function_indices,
    })
}

fn encode_memory_type(memory_type: &TassadarNormalizedWasmMemoryType) -> wasm_encoder::MemoryType {
    wasm_encoder::MemoryType {
        minimum: memory_type.minimum_pages,
        maximum: memory_type.maximum_pages,
        memory64: memory_type.memory64,
        shared: memory_type.shared,
        page_size_log2: memory_type.page_size_log2,
    }
}

fn encode_table_type(
    table: &TassadarNormalizedWasmTable,
) -> Result<TableType, TassadarNormalizedWasmModuleError> {
    let element_type = match table.element_kind {
        TassadarNormalizedWasmTableElementKind::Funcref => wasm_encoder::RefType::FUNCREF,
    };
    Ok(TableType {
        element_type,
        table64: false,
        minimum: u64::from(table.min_entries),
        maximum: table.max_entries.map(u64::from),
        shared: false,
    })
}

fn encode_global_type(
    global: &TassadarNormalizedWasmGlobal,
) -> Result<GlobalType, TassadarNormalizedWasmModuleError> {
    Ok(GlobalType {
        val_type: encode_value_type(global.value_type)?,
        mutable: matches!(
            global.mutability,
            TassadarNormalizedWasmGlobalMutability::Mutable
        ),
        shared: false,
    })
}

fn encode_instruction(instruction: &TassadarNormalizedWasmInstruction, function: &mut Function) {
    match instruction {
        TassadarNormalizedWasmInstruction::I32Const { value } => {
            function.instruction(&Instruction::I32Const(*value));
        }
        TassadarNormalizedWasmInstruction::LocalGet { local_index } => {
            function.instruction(&Instruction::LocalGet(*local_index));
        }
        TassadarNormalizedWasmInstruction::LocalSet { local_index } => {
            function.instruction(&Instruction::LocalSet(*local_index));
        }
        TassadarNormalizedWasmInstruction::LocalTee { local_index } => {
            function.instruction(&Instruction::LocalTee(*local_index));
        }
        TassadarNormalizedWasmInstruction::GlobalGet { global_index } => {
            function.instruction(&Instruction::GlobalGet(*global_index));
        }
        TassadarNormalizedWasmInstruction::GlobalSet { global_index } => {
            function.instruction(&Instruction::GlobalSet(*global_index));
        }
        TassadarNormalizedWasmInstruction::I32LtS => {
            function.instruction(&Instruction::I32LtS);
        }
        TassadarNormalizedWasmInstruction::I32Add => {
            function.instruction(&Instruction::I32Add);
        }
        TassadarNormalizedWasmInstruction::I32Sub => {
            function.instruction(&Instruction::I32Sub);
        }
        TassadarNormalizedWasmInstruction::I32Mul => {
            function.instruction(&Instruction::I32Mul);
        }
        TassadarNormalizedWasmInstruction::I32Shl => {
            function.instruction(&Instruction::I32Shl);
        }
        TassadarNormalizedWasmInstruction::I32Load {
            align,
            offset,
            memory_index,
        } => {
            function.instruction(&Instruction::I32Load(MemArg {
                offset: *offset,
                align: u32::from(*align),
                memory_index: *memory_index,
            }));
        }
        TassadarNormalizedWasmInstruction::I32Store {
            align,
            offset,
            memory_index,
        } => {
            function.instruction(&Instruction::I32Store(MemArg {
                offset: *offset,
                align: u32::from(*align),
                memory_index: *memory_index,
            }));
        }
        TassadarNormalizedWasmInstruction::MemorySize { memory_index } => {
            function.instruction(&Instruction::MemorySize(*memory_index));
        }
        TassadarNormalizedWasmInstruction::MemoryGrow { memory_index } => {
            function.instruction(&Instruction::MemoryGrow(*memory_index));
        }
        TassadarNormalizedWasmInstruction::MemoryCopy {
            dst_memory_index,
            src_memory_index,
        } => {
            function.instruction(&Instruction::MemoryCopy {
                dst_mem: *dst_memory_index,
                src_mem: *src_memory_index,
            });
        }
        TassadarNormalizedWasmInstruction::MemoryFill { memory_index } => {
            function.instruction(&Instruction::MemoryFill(*memory_index));
        }
        TassadarNormalizedWasmInstruction::Call { function_index } => {
            function.instruction(&Instruction::Call(*function_index));
        }
        TassadarNormalizedWasmInstruction::CallIndirect {
            type_index,
            table_index,
        } => {
            function.instruction(&Instruction::CallIndirect {
                type_index: *type_index,
                table_index: *table_index,
            });
        }
        TassadarNormalizedWasmInstruction::Return => {
            function.instruction(&Instruction::Return);
        }
        TassadarNormalizedWasmInstruction::Drop => {
            function.instruction(&Instruction::Drop);
        }
    }
}

fn encode_const_expr(expr: &TassadarNormalizedWasmConstExpr) -> ConstExpr {
    match expr {
        TassadarNormalizedWasmConstExpr::I32Const { value } => ConstExpr::i32_const(*value),
        TassadarNormalizedWasmConstExpr::GlobalGet { global_index } => {
            ConstExpr::global_get(*global_index)
        }
    }
}

fn binary_error(error: wasmparser::BinaryReaderError) -> TassadarNormalizedWasmModuleError {
    TassadarNormalizedWasmModuleError::MalformedBinary {
        message: error.to_string(),
    }
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
        TassadarNormalizedWasmModuleError, encode_tassadar_normalized_wasm_module,
        parse_tassadar_normalized_wasm_module, tassadar_seeded_dynamic_memory_module,
        tassadar_seeded_instantiation_module, tassadar_seeded_multi_function_module,
    };

    #[test]
    fn normalized_wasm_module_roundtrips_multi_function_sections() {
        let module = tassadar_seeded_multi_function_module().expect("seeded module should build");
        let encoded =
            encode_tassadar_normalized_wasm_module(&module).expect("seeded module should encode");
        let reparsed =
            parse_tassadar_normalized_wasm_module(&encoded).expect("encoded module should parse");
        assert_eq!(reparsed, module);
        assert_eq!(
            reparsed.exported_function_names(),
            vec![String::from("pair_sum"), String::from("local_double")]
        );
    }

    #[test]
    fn normalized_wasm_module_roundtrips_instantiation_sections() {
        let module = tassadar_seeded_instantiation_module().expect("seeded module should build");
        let encoded =
            encode_tassadar_normalized_wasm_module(&module).expect("seeded module should encode");
        let reparsed =
            parse_tassadar_normalized_wasm_module(&encoded).expect("encoded module should parse");
        assert_eq!(reparsed, module);
        assert_eq!(reparsed.tables.len(), 1);
        assert_eq!(reparsed.globals.len(), 1);
        assert_eq!(reparsed.start_function_index, Some(1));
        assert_eq!(reparsed.element_segments.len(), 1);
    }

    #[test]
    fn normalized_wasm_module_roundtrips_dynamic_memory_sections() {
        let module = tassadar_seeded_dynamic_memory_module().expect("seeded module should build");
        let encoded =
            encode_tassadar_normalized_wasm_module(&module).expect("seeded module should encode");
        let reparsed =
            parse_tassadar_normalized_wasm_module(&encoded).expect("encoded module should parse");
        assert_eq!(reparsed, module);
        assert_eq!(reparsed.memories.len(), 1);
        assert_eq!(reparsed.data_segments.len(), 1);
        assert!(
            reparsed.functions[0]
                .body
                .as_ref()
                .expect("defined body")
                .instructions
                .iter()
                .any(|instruction| matches!(
                    instruction,
                    super::TassadarNormalizedWasmInstruction::MemoryCopy { .. }
                ))
        );
    }

    #[test]
    fn normalized_wasm_module_rejects_malformed_binary() {
        let error = parse_tassadar_normalized_wasm_module(&[0x00, 0x61, 0x73])
            .expect_err("truncated bytes should refuse");
        assert!(matches!(
            error,
            TassadarNormalizedWasmModuleError::MalformedBinary { .. }
        ));
    }

    #[test]
    fn normalized_wasm_module_rejects_unsupported_export_kinds() {
        let mut module = wasm_encoder::Module::new();
        let mut types = wasm_encoder::TypeSection::new();
        types.ty().function(
            Vec::<wasm_encoder::ValType>::new(),
            Vec::<wasm_encoder::ValType>::new(),
        );
        module.section(&types);

        let mut functions = wasm_encoder::FunctionSection::new();
        functions.function(0);
        module.section(&functions);

        let mut globals = wasm_encoder::GlobalSection::new();
        globals.global(
            wasm_encoder::GlobalType {
                val_type: wasm_encoder::ValType::I32,
                mutable: false,
                shared: false,
            },
            &wasm_encoder::ConstExpr::i32_const(0),
        );
        module.section(&globals);

        let mut exports = wasm_encoder::ExportSection::new();
        exports.export("g0", wasm_encoder::ExportKind::Global, 0);
        module.section(&exports);

        let mut code = wasm_encoder::CodeSection::new();
        let mut function = wasm_encoder::Function::new(Vec::<(u32, wasm_encoder::ValType)>::new());
        function.instruction(&wasm_encoder::Instruction::End);
        code.function(&function);
        module.section(&code);

        let encoded = module.finish();

        let error = parse_tassadar_normalized_wasm_module(&encoded)
            .expect_err("global exports should remain outside the bounded IR");
        assert_eq!(
            error,
            TassadarNormalizedWasmModuleError::UnsupportedExportKind {
                export_name: String::from("g0"),
                kind: String::from("Global"),
            }
        );
    }
}
