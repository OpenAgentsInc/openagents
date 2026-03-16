use crate::{
    BackendProbeState, BackendToolchainIdentity, ExecutionProofArtifactResidency,
    ExecutionProofAugmentationPosture, ExecutionProofBundle, ExecutionProofBundleKind,
    ExecutionProofBundleStatus, ExecutionProofRuntimeIdentity, RuntimeManifest,
    RuntimeManifestArtifactBinding, RuntimeManifestArtifactKind,
    RuntimeManifestStaticConfigBinding, ValidationMatrixReference,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Current append-only trace ABI version for the Tassadar executor lane.
pub const TASSADAR_TRACE_ABI_VERSION: u16 = 1;
/// Stable CPU reference runner identifier for the Phase 1 lane.
pub const TASSADAR_CPU_REFERENCE_RUNNER_ID: &str = "tassadar.cpu_reference.v1";
/// Stable fixture runner identifier for the Phase 1 lane.
pub const TASSADAR_FIXTURE_RUNNER_ID: &str = "tassadar.fixture_runner.v1";
/// Stable opcode-vocabulary family identifier for the Phase 2 artifact lane.
pub const TASSADAR_OPCODE_VOCABULARY_ID: &str = "tassadar.opcodes.v1";
/// Current schema version for emitted Tassadar trace artifacts.
pub const TASSADAR_TRACE_ARTIFACT_SCHEMA_VERSION: u16 = 1;
/// Current schema version for emitted Tassadar trace-proof artifacts.
pub const TASSADAR_TRACE_PROOF_SCHEMA_VERSION: u16 = 1;
/// Stable claims-profile identifier for the Tassadar proof lane.
pub const TASSADAR_PROOF_CLAIMS_PROFILE_ID: &str = "tassadar.executor_trace.proof.v1";

/// Stable decode modes for the Tassadar executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorDecodeMode {
    /// Linear reference decode path over the executor trace.
    ReferenceLinear,
    /// Future hull-cache geometric fast path.
    HullCache,
}

impl TassadarExecutorDecodeMode {
    /// Returns the stable decode-mode identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReferenceLinear => "tassadar.decode.reference_linear.v1",
            Self::HullCache => "tassadar.decode.hull_cache.v1",
        }
    }

    /// Returns the cache algorithm paired with this decode mode.
    #[must_use]
    pub const fn cache_algorithm(self) -> TassadarExecutorCacheAlgorithm {
        match self {
            Self::ReferenceLinear => TassadarExecutorCacheAlgorithm::LinearScanKvCache,
            Self::HullCache => TassadarExecutorCacheAlgorithm::HullSupportCache,
        }
    }
}

/// Stable cache-algorithm identifiers for Tassadar decoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorCacheAlgorithm {
    /// Prefix-linear cache lookups over the trace.
    LinearScanKvCache,
    /// Future hull-backed geometric cache.
    HullSupportCache,
}

impl TassadarExecutorCacheAlgorithm {
    /// Returns the stable cache-algorithm identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinearScanKvCache => "tassadar.cache.linear_scan_kv.v1",
            Self::HullSupportCache => "tassadar.cache.hull_support.v1",
        }
    }
}

/// Machine-legible supported WebAssembly-first profile for the Phase 1 lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWasmProfileId {
    /// Narrow i32-only profile with explicit host-side `output`.
    CoreI32V1,
}

impl TassadarWasmProfileId {
    /// Returns the stable profile identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CoreI32V1 => "tassadar.wasm.core_i32.v1",
        }
    }
}

/// Condition semantics for the Phase 1 branch instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBranchMode {
    /// Pop one `i32`; branch when it is non-zero.
    BrIfNonZero,
}

/// Stable opcode set for the narrow WebAssembly-first profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarOpcode {
    /// Push one constant `i32`.
    I32Const,
    /// Push one local onto the stack.
    LocalGet,
    /// Pop one stack value into a local.
    LocalSet,
    /// Pop two `i32` values and push their sum.
    I32Add,
    /// Pop two `i32` values and push their difference.
    I32Sub,
    /// Pop two `i32` values and push their product.
    I32Mul,
    /// Load one memory slot and push it.
    I32Load,
    /// Pop one stack value into one memory slot.
    I32Store,
    /// Pop one condition value and optionally branch to the target pc.
    BrIf,
    /// Pop one stack value and emit it through the lane output sink.
    Output,
    /// Halt execution successfully.
    Return,
}

impl TassadarOpcode {
    /// Stable opcode ordering used by fixtures and metadata digests.
    pub const ALL: [Self; 11] = [
        Self::I32Const,
        Self::LocalGet,
        Self::LocalSet,
        Self::I32Add,
        Self::I32Sub,
        Self::I32Mul,
        Self::I32Load,
        Self::I32Store,
        Self::BrIf,
        Self::Output,
        Self::Return,
    ];

    /// Returns the stable opcode mnemonic.
    #[must_use]
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::I32Const => "i32.const",
            Self::LocalGet => "local.get",
            Self::LocalSet => "local.set",
            Self::I32Add => "i32.add",
            Self::I32Sub => "i32.sub",
            Self::I32Mul => "i32.mul",
            Self::I32Load => "i32.load",
            Self::I32Store => "i32.store",
            Self::BrIf => "br_if",
            Self::Output => "output",
            Self::Return => "return",
        }
    }

    /// Returns a stable ordinal for fixture-weight encoding.
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::I32Const => 0,
            Self::LocalGet => 1,
            Self::LocalSet => 2,
            Self::I32Add => 3,
            Self::I32Sub => 4,
            Self::I32Mul => 5,
            Self::I32Load => 6,
            Self::I32Store => 7,
            Self::BrIf => 8,
            Self::Output => 9,
            Self::Return => 10,
        }
    }
}

/// Immediate families carried by the narrow profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarImmediateKind {
    /// No immediate payload.
    None,
    /// One `i32` immediate value.
    I32,
    /// One local-slot index.
    LocalIndex,
    /// One memory-slot index.
    MemorySlot,
    /// One validated branch target pc.
    BranchTarget,
}

impl TassadarImmediateKind {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::I32 => 1,
            Self::LocalIndex => 2,
            Self::MemorySlot => 3,
            Self::BranchTarget => 4,
        }
    }
}

/// Branch/control effect classification for one opcode rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarControlClass {
    /// Linear control flow.
    Linear,
    /// Conditional control flow.
    ConditionalBranch,
    /// Terminal control flow.
    Return,
}

impl TassadarControlClass {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Linear => 0,
            Self::ConditionalBranch => 1,
            Self::Return => 2,
        }
    }
}

/// Arithmetic class encoded by one opcode rule when applicable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArithmeticOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
}

impl TassadarArithmeticOp {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Add => 1,
            Self::Sub => 2,
            Self::Mul => 3,
        }
    }
}

/// Memory and local access class for one opcode rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAccessClass {
    /// No side effect beyond stack/control behavior.
    None,
    /// Reads one local.
    LocalRead,
    /// Writes one local.
    LocalWrite,
    /// Reads one memory slot.
    MemoryRead,
    /// Writes one memory slot.
    MemoryWrite,
}

impl TassadarAccessClass {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::LocalRead => 1,
            Self::LocalWrite => 2,
            Self::MemoryRead => 3,
            Self::MemoryWrite => 4,
        }
    }
}

/// Machine-legible Phase 1 WebAssembly-first profile description.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmProfile {
    /// Stable profile identifier.
    pub profile_id: String,
    /// Allowed opcode set for this profile.
    pub allowed_opcodes: Vec<TassadarOpcode>,
    /// Maximum number of locals for the Phase 1 fixture.
    pub max_locals: usize,
    /// Maximum number of memory slots for the Phase 1 fixture.
    pub max_memory_slots: usize,
    /// Maximum instruction count accepted by the Phase 1 fixture.
    pub max_program_len: usize,
    /// Maximum execution steps before the runtime refuses the program.
    pub max_steps: usize,
    /// Branch semantics used by the profile.
    pub branch_mode: TassadarBranchMode,
    /// Whether the profile carries the host-side `output` helper opcode.
    pub host_output_opcode: bool,
}

impl TassadarWasmProfile {
    /// Returns the canonical Phase 1 profile.
    #[must_use]
    pub fn core_i32_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::CoreI32V1.as_str()),
            allowed_opcodes: TassadarOpcode::ALL.to_vec(),
            max_locals: 4,
            max_memory_slots: 8,
            max_program_len: 32,
            max_steps: 128,
            branch_mode: TassadarBranchMode::BrIfNonZero,
            host_output_opcode: true,
        }
    }

    /// Returns whether the profile explicitly supports one opcode.
    #[must_use]
    pub fn supports(&self, opcode: TassadarOpcode) -> bool {
        self.allowed_opcodes.contains(&opcode)
    }

    /// Returns a stable digest for the supported opcode vocabulary.
    #[must_use]
    pub fn opcode_vocabulary_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(TASSADAR_OPCODE_VOCABULARY_ID.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.profile_id.as_bytes());
        hasher.update(b"\n");
        for opcode in &self.allowed_opcodes {
            hasher.update(opcode.mnemonic().as_bytes());
            hasher.update(b"\n");
        }
        hex::encode(hasher.finalize())
    }
}

impl Default for TassadarWasmProfile {
    fn default() -> Self {
        Self::core_i32_v1()
    }
}

/// Explicit append-only trace ABI for the Phase 1 fixture lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceAbi {
    /// Stable ABI identifier.
    pub abi_id: String,
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable profile identifier the ABI is paired with.
    pub profile_id: String,
    /// Whether traces are append-only.
    pub append_only: bool,
    /// Whether stack snapshots are emitted per step.
    pub includes_stack_snapshots: bool,
    /// Whether local snapshots are emitted per step.
    pub includes_local_snapshots: bool,
    /// Whether memory snapshots are emitted per step.
    pub includes_memory_snapshots: bool,
}

impl TassadarTraceAbi {
    /// Returns the canonical Phase 1 trace ABI.
    #[must_use]
    pub fn core_i32_v1() -> Self {
        Self {
            abi_id: String::from("tassadar.trace.v1"),
            schema_version: TASSADAR_TRACE_ABI_VERSION,
            profile_id: String::from(TassadarWasmProfileId::CoreI32V1.as_str()),
            append_only: true,
            includes_stack_snapshots: true,
            includes_local_snapshots: true,
            includes_memory_snapshots: true,
        }
    }

    /// Returns a stable digest over the ABI compatibility surface.
    #[must_use]
    pub fn compatibility_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.abi_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.schema_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.profile_id.as_bytes());
        hasher.update(b"\n");
        hasher.update([
            u8::from(self.append_only),
            u8::from(self.includes_stack_snapshots),
            u8::from(self.includes_local_snapshots),
            u8::from(self.includes_memory_snapshots),
        ]);
        hex::encode(hasher.finalize())
    }
}

impl Default for TassadarTraceAbi {
    fn default() -> Self {
        Self::core_i32_v1()
    }
}

/// One validated instruction in the narrow WebAssembly-first profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "opcode", rename_all = "snake_case")]
pub enum TassadarInstruction {
    /// Push one constant `i32`.
    I32Const {
        /// Literal immediate value.
        value: i32,
    },
    /// Push one local onto the stack.
    LocalGet {
        /// Local slot index.
        local: u8,
    },
    /// Pop one stack value into one local.
    LocalSet {
        /// Local slot index.
        local: u8,
    },
    /// Pop two `i32` values and push the sum.
    I32Add,
    /// Pop two `i32` values and push the difference.
    I32Sub,
    /// Pop two `i32` values and push the product.
    I32Mul,
    /// Push one memory slot onto the stack.
    I32Load {
        /// Memory slot index.
        slot: u8,
    },
    /// Pop one stack value into one memory slot.
    I32Store {
        /// Memory slot index.
        slot: u8,
    },
    /// Pop one condition and branch to `target_pc` when non-zero.
    BrIf {
        /// Validated direct pc target for the Phase 1 profile.
        target_pc: u16,
    },
    /// Pop and emit one stack value.
    Output,
    /// Halt successfully.
    Return,
}

impl TassadarInstruction {
    /// Returns the stable opcode class for this instruction.
    #[must_use]
    pub const fn opcode(&self) -> TassadarOpcode {
        match self {
            Self::I32Const { .. } => TassadarOpcode::I32Const,
            Self::LocalGet { .. } => TassadarOpcode::LocalGet,
            Self::LocalSet { .. } => TassadarOpcode::LocalSet,
            Self::I32Add => TassadarOpcode::I32Add,
            Self::I32Sub => TassadarOpcode::I32Sub,
            Self::I32Mul => TassadarOpcode::I32Mul,
            Self::I32Load { .. } => TassadarOpcode::I32Load,
            Self::I32Store { .. } => TassadarOpcode::I32Store,
            Self::BrIf { .. } => TassadarOpcode::BrIf,
            Self::Output => TassadarOpcode::Output,
            Self::Return => TassadarOpcode::Return,
        }
    }
}

/// One Phase 1 executor program validated against a fixed Wasm-like profile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgram {
    /// Stable program identifier.
    pub program_id: String,
    /// Stable profile identifier expected by this program.
    pub profile_id: String,
    /// Number of locals used by the program.
    pub local_count: usize,
    /// Number of memory slots surfaced to the program.
    pub memory_slots: usize,
    /// Initial memory contents for the program.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial_memory: Vec<i32>,
    /// Ordered instruction sequence.
    pub instructions: Vec<TassadarInstruction>,
}

impl TassadarProgram {
    /// Creates a new Phase 1 Tassadar program.
    #[must_use]
    pub fn new(
        program_id: impl Into<String>,
        profile: &TassadarWasmProfile,
        local_count: usize,
        memory_slots: usize,
        instructions: Vec<TassadarInstruction>,
    ) -> Self {
        Self {
            program_id: program_id.into(),
            profile_id: profile.profile_id.clone(),
            local_count,
            memory_slots,
            initial_memory: vec![0; memory_slots],
            instructions,
        }
    }

    /// Replaces the initial memory image.
    #[must_use]
    pub fn with_initial_memory(mut self, initial_memory: Vec<i32>) -> Self {
        self.initial_memory = initial_memory;
        self
    }

    /// Returns a stable digest over the validated program payload.
    #[must_use]
    pub fn program_digest(&self) -> String {
        hex::encode(Sha256::digest(serde_json::to_vec(self).unwrap_or_default()))
    }

    fn validate_against(
        &self,
        profile: &TassadarWasmProfile,
    ) -> Result<(), TassadarExecutionRefusal> {
        if self.profile_id != profile.profile_id {
            return Err(TassadarExecutionRefusal::ProfileMismatch {
                expected: profile.profile_id.clone(),
                actual: self.profile_id.clone(),
            });
        }
        if self.local_count > profile.max_locals {
            return Err(TassadarExecutionRefusal::TooManyLocals {
                requested: self.local_count,
                max_supported: profile.max_locals,
            });
        }
        if self.memory_slots > profile.max_memory_slots {
            return Err(TassadarExecutionRefusal::TooManyMemorySlots {
                requested: self.memory_slots,
                max_supported: profile.max_memory_slots,
            });
        }
        if self.instructions.len() > profile.max_program_len {
            return Err(TassadarExecutionRefusal::ProgramTooLong {
                instruction_count: self.instructions.len(),
                max_supported: profile.max_program_len,
            });
        }
        if self.initial_memory.len() != self.memory_slots {
            return Err(TassadarExecutionRefusal::InitialMemoryShapeMismatch {
                expected: self.memory_slots,
                actual: self.initial_memory.len(),
            });
        }
        for (pc, instruction) in self.instructions.iter().enumerate() {
            if !profile.supports(instruction.opcode()) {
                return Err(TassadarExecutionRefusal::UnsupportedOpcode {
                    pc,
                    opcode: instruction.opcode(),
                });
            }
            match instruction {
                TassadarInstruction::LocalGet { local }
                | TassadarInstruction::LocalSet { local }
                    if usize::from(*local) >= self.local_count =>
                {
                    return Err(TassadarExecutionRefusal::LocalOutOfRange {
                        pc,
                        local: usize::from(*local),
                        local_count: self.local_count,
                    });
                }
                TassadarInstruction::I32Load { slot } | TassadarInstruction::I32Store { slot }
                    if usize::from(*slot) >= self.memory_slots =>
                {
                    return Err(TassadarExecutionRefusal::MemorySlotOutOfRange {
                        pc,
                        slot: usize::from(*slot),
                        memory_slots: self.memory_slots,
                    });
                }
                TassadarInstruction::BrIf { target_pc }
                    if usize::from(*target_pc) >= self.instructions.len() =>
                {
                    return Err(TassadarExecutionRefusal::InvalidBranchTarget {
                        pc,
                        target_pc: usize::from(*target_pc),
                        instruction_count: self.instructions.len(),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Source-language family for one digest-bound Tassadar program artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarProgramSourceKind {
    /// Hand-authored fixture or reference program checked into the repo.
    Fixture,
    /// Lowered from a C source file.
    CSource,
    /// Lowered from Rust or a Rust-adjacent source program.
    RustSource,
    /// Lowered from a Wasm text-format module.
    WasmText,
    /// Imported from an already-built Wasm binary module.
    WasmBinary,
}

/// Source-identity facts for one digest-bound Tassadar program artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramSourceIdentity {
    /// Source-language family.
    pub source_kind: TassadarProgramSourceKind,
    /// Stable source identifier such as a fixture name or source path label.
    pub source_name: String,
    /// Stable digest for the source input to the lowering pipeline.
    pub source_digest: String,
}

impl TassadarProgramSourceIdentity {
    /// Creates one explicit source-identity record.
    #[must_use]
    pub fn new(
        source_kind: TassadarProgramSourceKind,
        source_name: impl Into<String>,
        source_digest: impl Into<String>,
    ) -> Self {
        Self {
            source_kind,
            source_name: source_name.into(),
            source_digest: source_digest.into(),
        }
    }

    /// Creates one fixture-source record by hashing the validated program.
    #[must_use]
    pub fn fixture(program: &TassadarProgram) -> Self {
        Self {
            source_kind: TassadarProgramSourceKind::Fixture,
            source_name: program.program_id.clone(),
            source_digest: program.program_digest(),
        }
    }

    /// Returns a stable digest over the source-identity record.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_serialized_digest(b"tassadar_program_source_identity|", self)
    }
}

/// Compiler/toolchain identity for one digest-bound Tassadar program artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompilerToolchainIdentity {
    /// Stable compiler or lowering-pipeline family label.
    pub compiler_family: String,
    /// Stable compiler or lowering-pipeline version label.
    pub compiler_version: String,
    /// Stable lowering target or target triple label.
    pub target: String,
    /// Stable lowering stages or feature flags selected for the artifact.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pipeline_features: Vec<String>,
}

impl TassadarCompilerToolchainIdentity {
    /// Creates one explicit compiler/toolchain identity.
    #[must_use]
    pub fn new(
        compiler_family: impl Into<String>,
        compiler_version: impl Into<String>,
        target: impl Into<String>,
    ) -> Self {
        Self {
            compiler_family: compiler_family.into(),
            compiler_version: compiler_version.into(),
            target: target.into(),
            pipeline_features: Vec::new(),
        }
    }

    /// Attaches stable lowering features or pipeline stages.
    #[must_use]
    pub fn with_pipeline_features(mut self, mut pipeline_features: Vec<String>) -> Self {
        pipeline_features.sort();
        pipeline_features.dedup();
        self.pipeline_features = pipeline_features;
        self
    }

    /// Returns a stable digest over the compiler/toolchain identity.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_serialized_digest(b"tassadar_compiler_toolchain_identity|", self)
    }
}

/// Digest-bound Tassadar program artifact ready to pair with executor models.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarProgramArtifact {
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable digest over the identity-relevant artifact fields.
    pub artifact_digest: String,
    /// Source-identity facts for the artifact.
    pub source_identity: TassadarProgramSourceIdentity,
    /// Compiler/toolchain identity for the artifact.
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    /// Stable Wasm profile identifier.
    pub wasm_profile_id: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI schema version.
    pub trace_abi_version: u16,
    /// Stable opcode-vocabulary digest the artifact expects.
    pub opcode_vocabulary_digest: String,
    /// Stable digest over the validated program payload.
    pub validated_program_digest: String,
    /// Optional digest for the original Wasm binary module when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wasm_binary_digest: Option<String>,
    /// Validated executor program carried by the artifact.
    pub validated_program: TassadarProgram,
}

impl TassadarProgramArtifact {
    /// Creates a digest-bound artifact from a validated program and explicit source/toolchain facts.
    pub fn new(
        artifact_id: impl Into<String>,
        source_identity: TassadarProgramSourceIdentity,
        toolchain_identity: TassadarCompilerToolchainIdentity,
        profile: &TassadarWasmProfile,
        trace_abi: &TassadarTraceAbi,
        validated_program: TassadarProgram,
    ) -> Result<Self, TassadarProgramArtifactError> {
        if trace_abi.profile_id != profile.profile_id {
            return Err(TassadarProgramArtifactError::TraceAbiProfileMismatch {
                trace_abi_profile_id: trace_abi.profile_id.clone(),
                wasm_profile_id: profile.profile_id.clone(),
            });
        }
        if validated_program.profile_id != profile.profile_id {
            return Err(TassadarProgramArtifactError::ProgramProfileMismatch {
                expected: profile.profile_id.clone(),
                actual: validated_program.profile_id.clone(),
            });
        }
        validated_program
            .validate_against(profile)
            .map_err(
                |error| TassadarProgramArtifactError::InvalidValidatedProgram {
                    message: error.to_string(),
                },
            )?;

        let validated_program_digest = validated_program.program_digest();
        let mut artifact = Self {
            artifact_id: artifact_id.into(),
            artifact_digest: String::new(),
            source_identity,
            toolchain_identity,
            wasm_profile_id: profile.profile_id.clone(),
            trace_abi_id: trace_abi.abi_id.clone(),
            trace_abi_version: trace_abi.schema_version,
            opcode_vocabulary_digest: profile.opcode_vocabulary_digest(),
            validated_program_digest,
            wasm_binary_digest: None,
            validated_program,
        };
        artifact.refresh_digest();
        Ok(artifact)
    }

    /// Creates the canonical fixture artifact posture for a validated program.
    pub fn fixture_reference(
        artifact_id: impl Into<String>,
        profile: &TassadarWasmProfile,
        trace_abi: &TassadarTraceAbi,
        validated_program: TassadarProgram,
    ) -> Result<Self, TassadarProgramArtifactError> {
        let source_identity = TassadarProgramSourceIdentity::fixture(&validated_program);
        let toolchain_identity = TassadarCompilerToolchainIdentity::new(
            "tassadar_fixture",
            "v1",
            profile.profile_id.as_str(),
        )
        .with_pipeline_features(vec![
            String::from("validated_program"),
            String::from("webassembly_first"),
        ]);
        Self::new(
            artifact_id,
            source_identity,
            toolchain_identity,
            profile,
            trace_abi,
            validated_program,
        )
    }

    /// Attaches an original Wasm binary digest when one exists.
    #[must_use]
    pub fn with_wasm_binary_digest(mut self, wasm_binary_digest: impl Into<String>) -> Self {
        self.wasm_binary_digest = Some(wasm_binary_digest.into());
        self.refresh_digest();
        self
    }

    /// Validates internal artifact consistency without pairing against a model descriptor.
    pub fn validate_internal_consistency(&self) -> Result<(), TassadarProgramArtifactError> {
        if self.validated_program_digest != self.validated_program.program_digest() {
            return Err(
                TassadarProgramArtifactError::ValidatedProgramDigestMismatch {
                    expected: self.validated_program.program_digest(),
                    actual: self.validated_program_digest.clone(),
                },
            );
        }
        let actual_digest = self.compute_digest();
        if self.artifact_digest != actual_digest {
            return Err(TassadarProgramArtifactError::ArtifactDigestMismatch {
                expected: actual_digest,
                actual: self.artifact_digest.clone(),
            });
        }
        Ok(())
    }

    fn refresh_digest(&mut self) {
        self.artifact_digest = self.compute_digest();
    }

    fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.artifact_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.wasm_profile_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_abi_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_abi_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.opcode_vocabulary_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.validated_program_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.source_identity).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.toolchain_identity).unwrap_or_default());
        hasher.update(b"\n");
        if let Some(wasm_binary_digest) = &self.wasm_binary_digest {
            hasher.update(wasm_binary_digest.as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

/// Artifact-assembly failures for digest-bound Tassadar program artifacts.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarProgramArtifactError {
    /// The ABI and Wasm profile targeted different execution profiles.
    #[error(
        "trace ABI profile `{trace_abi_profile_id}` does not match Wasm profile `{wasm_profile_id}`"
    )]
    TraceAbiProfileMismatch {
        /// Profile identifier declared by the trace ABI.
        trace_abi_profile_id: String,
        /// Profile identifier declared by the Wasm profile.
        wasm_profile_id: String,
    },
    /// The validated program targeted a different profile than the artifact profile.
    #[error("validated program profile mismatch: expected `{expected}`, got `{actual}`")]
    ProgramProfileMismatch {
        /// Expected profile identifier.
        expected: String,
        /// Actual validated-program profile identifier.
        actual: String,
    },
    /// The supplied validated program failed structural validation.
    #[error("validated program is not internally valid: {message}")]
    InvalidValidatedProgram {
        /// Validation failure summary.
        message: String,
    },
    /// The stored validated-program digest no longer matches the payload.
    #[error("validated program digest mismatch: expected `{expected}`, actual `{actual}`")]
    ValidatedProgramDigestMismatch {
        /// Expected digest recomputed from the validated program.
        expected: String,
        /// Actual stored digest.
        actual: String,
    },
    /// The stored artifact digest no longer matches the identity fields.
    #[error("artifact digest mismatch: expected `{expected}`, actual `{actual}`")]
    ArtifactDigestMismatch {
        /// Expected digest recomputed from the artifact fields.
        expected: String,
        /// Actual stored digest.
        actual: String,
    },
}

/// One rule in the handcrafted/programmatic Tassadar fixture construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarOpcodeRule {
    /// Stable opcode identity.
    pub opcode: TassadarOpcode,
    /// Number of stack values consumed.
    pub pops: u8,
    /// Number of stack values produced.
    pub pushes: u8,
    /// Immediate family carried by the opcode.
    pub immediate_kind: TassadarImmediateKind,
    /// Local/memory access classification.
    pub access_class: TassadarAccessClass,
    /// Control-flow classification.
    pub control_class: TassadarControlClass,
    /// Arithmetic family when the opcode is arithmetic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arithmetic: Option<TassadarArithmeticOp>,
}

impl TassadarOpcodeRule {
    #[must_use]
    fn new(
        opcode: TassadarOpcode,
        pops: u8,
        pushes: u8,
        immediate_kind: TassadarImmediateKind,
        access_class: TassadarAccessClass,
        control_class: TassadarControlClass,
    ) -> Self {
        Self {
            opcode,
            pops,
            pushes,
            immediate_kind,
            access_class,
            control_class,
            arithmetic: None,
        }
    }

    #[must_use]
    fn with_arithmetic(mut self, arithmetic: TassadarArithmeticOp) -> Self {
        self.arithmetic = Some(arithmetic);
        self
    }
}

/// Handcrafted/programmatic rule tables backing the Phase 1 fixture lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFixtureWeights {
    /// Stable profile identifier the weights are constructed for.
    pub profile_id: String,
    /// Stable trace ABI identifier paired with the weights.
    pub trace_abi_id: String,
    /// Ordered opcode rule table.
    pub opcode_rules: Vec<TassadarOpcodeRule>,
}

impl TassadarFixtureWeights {
    /// Returns the canonical Phase 1 handcrafted fixture table.
    #[must_use]
    pub fn core_i32_v1() -> Self {
        Self {
            profile_id: String::from(TassadarWasmProfileId::CoreI32V1.as_str()),
            trace_abi_id: String::from("tassadar.trace.v1"),
            opcode_rules: vec![
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Const,
                    0,
                    1,
                    TassadarImmediateKind::I32,
                    TassadarAccessClass::None,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::LocalGet,
                    0,
                    1,
                    TassadarImmediateKind::LocalIndex,
                    TassadarAccessClass::LocalRead,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::LocalSet,
                    1,
                    0,
                    TassadarImmediateKind::LocalIndex,
                    TassadarAccessClass::LocalWrite,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Add,
                    2,
                    1,
                    TassadarImmediateKind::None,
                    TassadarAccessClass::None,
                    TassadarControlClass::Linear,
                )
                .with_arithmetic(TassadarArithmeticOp::Add),
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Sub,
                    2,
                    1,
                    TassadarImmediateKind::None,
                    TassadarAccessClass::None,
                    TassadarControlClass::Linear,
                )
                .with_arithmetic(TassadarArithmeticOp::Sub),
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Mul,
                    2,
                    1,
                    TassadarImmediateKind::None,
                    TassadarAccessClass::None,
                    TassadarControlClass::Linear,
                )
                .with_arithmetic(TassadarArithmeticOp::Mul),
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Load,
                    0,
                    1,
                    TassadarImmediateKind::MemorySlot,
                    TassadarAccessClass::MemoryRead,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::I32Store,
                    1,
                    0,
                    TassadarImmediateKind::MemorySlot,
                    TassadarAccessClass::MemoryWrite,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::BrIf,
                    1,
                    0,
                    TassadarImmediateKind::BranchTarget,
                    TassadarAccessClass::None,
                    TassadarControlClass::ConditionalBranch,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::Output,
                    1,
                    0,
                    TassadarImmediateKind::None,
                    TassadarAccessClass::None,
                    TassadarControlClass::Linear,
                ),
                TassadarOpcodeRule::new(
                    TassadarOpcode::Return,
                    0,
                    0,
                    TassadarImmediateKind::None,
                    TassadarAccessClass::None,
                    TassadarControlClass::Return,
                ),
            ],
        }
    }

    /// Returns one rule by opcode.
    #[must_use]
    pub fn rule_for(&self, opcode: TassadarOpcode) -> Option<&TassadarOpcodeRule> {
        self.opcode_rules.iter().find(|rule| rule.opcode == opcode)
    }
}

impl Default for TassadarFixtureWeights {
    fn default() -> Self {
        Self::core_i32_v1()
    }
}

/// One emitted trace event in the append-only ABI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarTraceEvent {
    /// One constant was pushed.
    ConstPush {
        /// Value that was pushed.
        value: i32,
    },
    /// One local was read.
    LocalGet {
        /// Local index read.
        local: u8,
        /// Value loaded from the local.
        value: i32,
    },
    /// One local was written.
    LocalSet {
        /// Local index written.
        local: u8,
        /// Value written into the local.
        value: i32,
    },
    /// One arithmetic op was applied.
    BinaryOp {
        /// Arithmetic family.
        op: TassadarArithmeticOp,
        /// Left operand.
        left: i32,
        /// Right operand.
        right: i32,
        /// Result value.
        result: i32,
    },
    /// One memory slot was loaded.
    Load {
        /// Slot index read.
        slot: u8,
        /// Value loaded from the slot.
        value: i32,
    },
    /// One memory slot was written.
    Store {
        /// Slot index written.
        slot: u8,
        /// Value written to the slot.
        value: i32,
    },
    /// One conditional branch was evaluated.
    Branch {
        /// Raw condition popped from the stack.
        condition: i32,
        /// Whether the branch was taken.
        taken: bool,
        /// Branch target pc.
        target_pc: usize,
    },
    /// One output was emitted.
    Output {
        /// Value emitted by the host-side output sink.
        value: i32,
    },
    /// Execution returned successfully.
    Return,
}

/// One append-only step in the Tassadar execution trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceStep {
    /// Step index in execution order.
    pub step_index: usize,
    /// Program counter before executing the step.
    pub pc: usize,
    /// Program counter after executing the step.
    pub next_pc: usize,
    /// Instruction executed at `pc`.
    pub instruction: TassadarInstruction,
    /// Event emitted by the step.
    pub event: TassadarTraceEvent,
    /// Stack snapshot before the step.
    pub stack_before: Vec<i32>,
    /// Stack snapshot after the step.
    pub stack_after: Vec<i32>,
    /// Local snapshot after the step.
    pub locals_after: Vec<i32>,
    /// Memory snapshot after the step.
    pub memory_after: Vec<i32>,
}

/// Terminal reason for one Phase 1 execution run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarHaltReason {
    /// The program executed `return`.
    Returned,
    /// The program advanced beyond the end of the instruction list.
    FellOffEnd,
}

/// One complete execution result for the Phase 1 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecution {
    /// Stable program identifier.
    pub program_id: String,
    /// Stable profile identifier.
    pub profile_id: String,
    /// Stable runner identifier.
    pub runner_id: String,
    /// ABI declaration used for the trace.
    pub trace_abi: TassadarTraceAbi,
    /// Ordered append-only steps.
    pub steps: Vec<TassadarTraceStep>,
    /// Output values emitted by the program.
    pub outputs: Vec<i32>,
    /// Final locals snapshot.
    pub final_locals: Vec<i32>,
    /// Final memory snapshot.
    pub final_memory: Vec<i32>,
    /// Final stack snapshot.
    pub final_stack: Vec<i32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarHaltReason,
}

impl TassadarExecution {
    /// Returns a runner-independent digest over the trace and terminal state.
    #[must_use]
    pub fn behavior_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.program_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.profile_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_digest().as_bytes());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.outputs).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.final_locals).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.final_memory).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.final_stack).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(format!("{:?}", self.halt_reason).as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Returns a stable digest over the append-only trace only.
    #[must_use]
    pub fn trace_digest(&self) -> String {
        let bytes = serde_json::to_vec(&self.steps).unwrap_or_default();
        hex::encode(Sha256::digest(bytes))
    }
}

/// Emitted trace artifact for one realized Tassadar execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceArtifact {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable artifact identifier.
    pub artifact_id: String,
    /// Stable digest over the trace artifact.
    pub artifact_digest: String,
    /// Stable program identifier.
    pub program_id: String,
    /// Stable runner identifier.
    pub runner_id: String,
    /// Stable trace ABI identifier.
    pub trace_abi_id: String,
    /// Stable trace ABI schema version.
    pub trace_abi_version: u16,
    /// Stable digest over the append-only trace.
    pub trace_digest: String,
    /// Stable digest over the behavior-relevant result.
    pub behavior_digest: String,
    /// Number of emitted steps.
    pub step_count: u64,
    /// Ordered append-only steps.
    pub steps: Vec<TassadarTraceStep>,
}

impl TassadarTraceArtifact {
    /// Builds a trace artifact from one execution.
    #[must_use]
    pub fn from_execution(artifact_id: impl Into<String>, execution: &TassadarExecution) -> Self {
        let mut artifact = Self {
            schema_version: TASSADAR_TRACE_ARTIFACT_SCHEMA_VERSION,
            artifact_id: artifact_id.into(),
            artifact_digest: String::new(),
            program_id: execution.program_id.clone(),
            runner_id: execution.runner_id.clone(),
            trace_abi_id: execution.trace_abi.abi_id.clone(),
            trace_abi_version: execution.trace_abi.schema_version,
            trace_digest: execution.trace_digest(),
            behavior_digest: execution.behavior_digest(),
            step_count: execution.steps.len() as u64,
            steps: execution.steps.clone(),
        };
        artifact.refresh_digest();
        artifact
    }

    fn refresh_digest(&mut self) {
        self.artifact_digest = self.compute_digest();
    }

    fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_trace_artifact|");
        hasher.update(self.schema_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.artifact_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.program_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.runner_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_abi_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_abi_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.behavior_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.step_count.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.steps).unwrap_or_default());
        hex::encode(hasher.finalize())
    }
}

/// Proof-bearing trace artifact for one realized Tassadar execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTraceProofArtifact {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable proof-artifact identifier.
    pub proof_artifact_id: String,
    /// Stable digest over the trace proof.
    pub proof_digest: String,
    /// Stable trace-artifact reference.
    pub trace_artifact_ref: String,
    /// Stable trace-artifact digest.
    pub trace_artifact_digest: String,
    /// Stable trace digest.
    pub trace_digest: String,
    /// Stable validated-program digest.
    pub program_digest: String,
    /// Stable program-artifact digest.
    pub program_artifact_digest: String,
    /// Stable Wasm profile identifier.
    pub wasm_profile_id: String,
    /// Stable model-descriptor digest.
    pub model_descriptor_digest: String,
    /// Decode mode used for the execution.
    pub decode_mode: TassadarExecutorDecodeMode,
    /// Stable cache-algorithm identifier.
    pub cache_algorithm_id: String,
    /// Runtime backend that realized the trace.
    pub runtime_backend: String,
    /// Reference-runner identity.
    pub reference_runner_id: String,
    /// Validation reference carried with the proof.
    pub validation: ValidationMatrixReference,
    /// Stable runtime-manifest identity digest.
    pub runtime_manifest_identity_digest: String,
    /// Stable runtime-manifest digest.
    pub runtime_manifest_digest: String,
}

impl TassadarTraceProofArtifact {
    /// Builds a trace-proof artifact from explicit lineage inputs.
    #[must_use]
    pub fn new(
        proof_artifact_id: impl Into<String>,
        trace_artifact: &TassadarTraceArtifact,
        program_artifact: &TassadarProgramArtifact,
        model_descriptor_digest: impl Into<String>,
        decode_mode: TassadarExecutorDecodeMode,
        runtime_backend: impl Into<String>,
        reference_runner_id: impl Into<String>,
        validation: ValidationMatrixReference,
        runtime_manifest: &RuntimeManifest,
    ) -> Self {
        let mut artifact = Self {
            schema_version: TASSADAR_TRACE_PROOF_SCHEMA_VERSION,
            proof_artifact_id: proof_artifact_id.into(),
            proof_digest: String::new(),
            trace_artifact_ref: trace_artifact.artifact_id.clone(),
            trace_artifact_digest: trace_artifact.artifact_digest.clone(),
            trace_digest: trace_artifact.trace_digest.clone(),
            program_digest: program_artifact.validated_program_digest.clone(),
            program_artifact_digest: program_artifact.artifact_digest.clone(),
            wasm_profile_id: program_artifact.wasm_profile_id.clone(),
            model_descriptor_digest: model_descriptor_digest.into(),
            decode_mode,
            cache_algorithm_id: String::from(decode_mode.cache_algorithm().as_str()),
            runtime_backend: runtime_backend.into(),
            reference_runner_id: reference_runner_id.into(),
            validation,
            runtime_manifest_identity_digest: runtime_manifest.identity_digest.clone(),
            runtime_manifest_digest: runtime_manifest.manifest_digest.clone(),
        };
        artifact.refresh_digest();
        artifact
    }

    fn refresh_digest(&mut self) {
        self.proof_digest = self.compute_digest();
    }

    fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_trace_proof_artifact|");
        hasher.update(self.schema_version.to_be_bytes());
        hasher.update(b"\n");
        hasher.update(self.proof_artifact_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_artifact_ref.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_artifact_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.trace_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.program_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.program_artifact_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.wasm_profile_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.model_descriptor_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.decode_mode.as_str().as_bytes());
        hasher.update(b"\n");
        hasher.update(self.cache_algorithm_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.runtime_backend.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.reference_runner_id.as_bytes());
        hasher.update(b"\n");
        hasher.update(serde_json::to_vec(&self.validation).unwrap_or_default());
        hasher.update(b"\n");
        hasher.update(self.runtime_manifest_identity_digest.as_bytes());
        hasher.update(b"\n");
        hasher.update(self.runtime_manifest_digest.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Runtime-manifest and proof-bundle evidence for one Tassadar execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutionEvidenceBundle {
    /// Digest-bound runtime manifest for the execution lane.
    pub runtime_manifest: RuntimeManifest,
    /// Emitted trace artifact.
    pub trace_artifact: TassadarTraceArtifact,
    /// Proof-bearing trace artifact.
    pub trace_proof: TassadarTraceProofArtifact,
    /// Canonical Psionic proof bundle carrying the execution identity.
    pub proof_bundle: ExecutionProofBundle,
}

/// Returns the current validation reference for the Tassadar proof lane.
#[must_use]
pub fn tassadar_validation_reference() -> ValidationMatrixReference {
    ValidationMatrixReference::not_yet_validated("tassadar.executor_trace.phase4")
}

/// Builds runtime-manifest and proof-bundle evidence for one Tassadar execution.
#[must_use]
pub fn build_tassadar_execution_evidence_bundle(
    request_id: impl Into<String>,
    request_digest: impl Into<String>,
    product_id: impl Into<String>,
    model_id: impl Into<String>,
    model_descriptor_digest: impl Into<String>,
    environment_refs: Vec<String>,
    program_artifact: &TassadarProgramArtifact,
    decode_mode: TassadarExecutorDecodeMode,
    execution: &TassadarExecution,
) -> TassadarExecutionEvidenceBundle {
    let request_id = request_id.into();
    let request_digest = request_digest.into();
    let product_id = product_id.into();
    let model_id = model_id.into();
    let model_descriptor_digest = model_descriptor_digest.into();
    let validation = tassadar_validation_reference();
    let trace_artifact = TassadarTraceArtifact::from_execution(
        format!("tassadar://trace/{request_id}/{}", execution.trace_digest()),
        execution,
    );
    let runtime_identity = ExecutionProofRuntimeIdentity::new(
        "cpu",
        BackendToolchainIdentity::new(
            "cpu",
            execution.runner_id.clone(),
            vec![
                String::from("tassadar_executor"),
                String::from(decode_mode.as_str()),
                execution.profile_id.clone(),
            ],
        )
        .with_probe(BackendProbeState::CompiledOnly, Vec::new()),
    );
    let mut runtime_manifest = RuntimeManifest::new(
        format!("tassadar-runtime-manifest-{request_id}"),
        runtime_identity.clone(),
    )
    .with_validation(validation.clone())
    .with_claims_profile_id(TASSADAR_PROOF_CLAIMS_PROFILE_ID)
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ProgramArtifact,
        program_artifact.artifact_id.clone(),
        program_artifact.artifact_digest.clone(),
    ))
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ModelDescriptor,
        model_id.clone(),
        model_descriptor_digest.clone(),
    ))
    .with_artifact_binding(RuntimeManifestArtifactBinding::new(
        RuntimeManifestArtifactKind::ExecutionTrace,
        trace_artifact.artifact_id.clone(),
        trace_artifact.artifact_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.source_program_digest",
        program_artifact.source_identity.stable_digest(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.compile_toolchain_digest",
        program_artifact.toolchain_identity.stable_digest(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.program_digest",
        program_artifact.validated_program_digest.clone(),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.decode_mode",
        stable_bytes_digest(decode_mode.as_str().as_bytes()),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.cache_algorithm",
        stable_bytes_digest(decode_mode.cache_algorithm().as_str().as_bytes()),
    ))
    .with_static_config_binding(RuntimeManifestStaticConfigBinding::new(
        "tassadar.reference_runner",
        stable_bytes_digest(execution.runner_id.as_bytes()),
    ));
    for environment_ref in environment_refs {
        runtime_manifest = runtime_manifest.with_environment_ref(environment_ref);
    }

    let trace_proof = TassadarTraceProofArtifact::new(
        format!(
            "tassadar://trace_proof/{request_id}/{}",
            trace_artifact.trace_digest
        ),
        &trace_artifact,
        program_artifact,
        model_descriptor_digest.clone(),
        decode_mode,
        runtime_identity.runtime_backend.clone(),
        execution.runner_id.clone(),
        validation.clone(),
        &runtime_manifest,
    );

    let mut proof_bundle = ExecutionProofBundle::new(
        ExecutionProofBundleKind::Local,
        if execution.halt_reason == TassadarHaltReason::Returned {
            ExecutionProofBundleStatus::Succeeded
        } else {
            ExecutionProofBundleStatus::Failed
        },
        request_id,
        request_digest,
        product_id,
        runtime_identity,
    )
    .with_model_id(model_id)
    .with_validation(validation)
    .with_activation_fingerprint_posture(ExecutionProofAugmentationPosture::Unavailable);
    proof_bundle.artifact_residency = Some(ExecutionProofArtifactResidency {
        served_artifact_digest: None,
        weight_bundle_digest: Some(model_descriptor_digest),
        cluster_artifact_residency_digest: None,
        sharded_model_manifest_digest: None,
        input_artifact_digests: vec![program_artifact.artifact_digest.clone()],
        output_artifact_digests: vec![
            trace_artifact.artifact_digest.clone(),
            trace_proof.proof_digest.clone(),
        ],
        stdout_sha256: None,
        stderr_sha256: None,
    });
    if execution.halt_reason != TassadarHaltReason::Returned {
        proof_bundle =
            proof_bundle.with_failure_reason(format!("tassadar_halt={:?}", execution.halt_reason));
    }

    TassadarExecutionEvidenceBundle {
        runtime_manifest,
        trace_artifact,
        trace_proof,
        proof_bundle,
    }
}

/// Typed refusal surfaces for unsupported or invalid Phase 1 programs.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutionRefusal {
    /// The program targeted a different profile.
    #[error("profile mismatch: expected `{expected}`, got `{actual}`")]
    ProfileMismatch {
        /// Expected profile identifier.
        expected: String,
        /// Actual program profile identifier.
        actual: String,
    },
    /// The program requested too many locals.
    #[error("program requested {requested} locals, max supported is {max_supported}")]
    TooManyLocals {
        /// Locals requested by the program.
        requested: usize,
        /// Maximum supported locals.
        max_supported: usize,
    },
    /// The program requested too many memory slots.
    #[error("program requested {requested} memory slots, max supported is {max_supported}")]
    TooManyMemorySlots {
        /// Memory slots requested by the program.
        requested: usize,
        /// Maximum supported slots.
        max_supported: usize,
    },
    /// The program exceeded the instruction budget.
    #[error("program uses {instruction_count} instructions, max supported is {max_supported}")]
    ProgramTooLong {
        /// Program instruction count.
        instruction_count: usize,
        /// Maximum supported instruction count.
        max_supported: usize,
    },
    /// The initial memory image does not match the declared slot count.
    #[error("initial memory image length mismatch: expected {expected}, got {actual}")]
    InitialMemoryShapeMismatch {
        /// Expected slot count.
        expected: usize,
        /// Actual memory image length.
        actual: usize,
    },
    /// The program used an opcode not supported by the active profile.
    #[error("unsupported opcode `{}` at pc {}", opcode.mnemonic(), pc)]
    UnsupportedOpcode {
        /// Program counter of the failing instruction.
        pc: usize,
        /// Unsupported opcode.
        opcode: TassadarOpcode,
    },
    /// The program addressed a local outside the declared range.
    #[error(
        "local {} out of range at pc {} (local_count={})",
        local,
        pc,
        local_count
    )]
    LocalOutOfRange {
        /// Program counter of the failing instruction.
        pc: usize,
        /// Requested local index.
        local: usize,
        /// Declared local count.
        local_count: usize,
    },
    /// The program addressed a memory slot outside the declared range.
    #[error(
        "memory slot {} out of range at pc {} (memory_slots={})",
        slot,
        pc,
        memory_slots
    )]
    MemorySlotOutOfRange {
        /// Program counter of the failing instruction.
        pc: usize,
        /// Requested memory slot.
        slot: usize,
        /// Declared memory slot count.
        memory_slots: usize,
    },
    /// The program targeted an invalid branch pc.
    #[error(
        "branch target {} out of range at pc {} (instruction_count={})",
        target_pc,
        pc,
        instruction_count
    )]
    InvalidBranchTarget {
        /// Program counter of the failing instruction.
        pc: usize,
        /// Invalid target pc.
        target_pc: usize,
        /// Total instruction count.
        instruction_count: usize,
    },
    /// Runtime stack underflow occurred.
    #[error("stack underflow at pc {}: needed {}, had {}", pc, needed, available)]
    StackUnderflow {
        /// Program counter of the failing instruction.
        pc: usize,
        /// Number of values required.
        needed: usize,
        /// Number of values available.
        available: usize,
    },
    /// The runtime step budget was exhausted.
    #[error("step limit exceeded: used more than {}", max_steps)]
    StepLimitExceeded {
        /// Maximum step budget for the active profile.
        max_steps: usize,
    },
    /// The fixture lane is missing an opcode rule for the instruction.
    #[error("fixture rule missing for opcode `{}`", opcode.mnemonic())]
    FixtureRuleMissing {
        /// Opcode that could not be resolved in the fixture table.
        opcode: TassadarOpcode,
    },
    /// The fixture table and execution behavior diverged.
    #[error(
        "fixture rule mismatch for `{}`: expected pops={} pushes={}, got pops={} pushes={}",
        opcode.mnemonic(), expected_pops, expected_pushes, actual_pops, actual_pushes
    )]
    FixtureRuleMismatch {
        /// Opcode whose rule diverged.
        opcode: TassadarOpcode,
        /// Expected stack pops from the fixture rule.
        expected_pops: u8,
        /// Expected stack pushes from the fixture rule.
        expected_pushes: u8,
        /// Actual stack pops observed during execution.
        actual_pops: u8,
        /// Actual stack pushes observed during execution.
        actual_pushes: u8,
    },
    /// The fixture and reference lanes disagreed on trace/state behavior.
    #[error(
        "exact parity mismatch for program `{program_id}`: reference={reference_digest} fixture={fixture_digest}"
    )]
    ParityMismatch {
        /// Program identifier compared by the harness.
        program_id: String,
        /// Runner-independent behavior digest from the reference lane.
        reference_digest: String,
        /// Runner-independent behavior digest from the fixture lane.
        fixture_digest: String,
    },
    /// The supplied trace no longer replays exactly against the reference runner.
    #[error(
        "deterministic replay mismatch for program `{program_id}`: expected={expected_digest} actual={actual_digest}"
    )]
    ReplayMismatch {
        /// Program identifier being replayed.
        program_id: String,
        /// Expected runner-independent behavior digest.
        expected_digest: String,
        /// Actual runner-independent behavior digest.
        actual_digest: String,
    },
}

/// Exact parity report between the reference runner and the fixture runner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarParityReport {
    /// Program identifier compared by the harness.
    pub program_id: String,
    /// Reference execution.
    pub reference: TassadarExecution,
    /// Fixture execution.
    pub fixture: TassadarExecution,
}

impl TassadarParityReport {
    /// Ensures the report is exact across trace, outputs, and terminal state.
    pub fn require_exact(&self) -> Result<(), TassadarExecutionRefusal> {
        if self.reference.behavior_digest() == self.fixture.behavior_digest() {
            Ok(())
        } else {
            Err(TassadarExecutionRefusal::ParityMismatch {
                program_id: self.program_id.clone(),
                reference_digest: self.reference.behavior_digest(),
                fixture_digest: self.fixture.behavior_digest(),
            })
        }
    }
}

/// CPU reference runner for the Phase 1 profile.
#[derive(Clone, Debug, Default)]
pub struct TassadarCpuReferenceRunner {
    profile: TassadarWasmProfile,
    trace_abi: TassadarTraceAbi,
}

impl TassadarCpuReferenceRunner {
    /// Creates the canonical reference runner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            profile: TassadarWasmProfile::core_i32_v1(),
            trace_abi: TassadarTraceAbi::core_i32_v1(),
        }
    }

    /// Executes one validated Tassadar program on the direct CPU reference path.
    pub fn execute(
        &self,
        program: &TassadarProgram,
    ) -> Result<TassadarExecution, TassadarExecutionRefusal> {
        execute_program(
            program,
            &self.profile,
            &self.trace_abi,
            TASSADAR_CPU_REFERENCE_RUNNER_ID,
            None,
        )
    }
}

/// Fixture-backed Phase 1 runner using handcrafted opcode-rule tables.
#[derive(Clone, Debug, Default)]
pub struct TassadarFixtureRunner {
    profile: TassadarWasmProfile,
    trace_abi: TassadarTraceAbi,
    weights: TassadarFixtureWeights,
}

impl TassadarFixtureRunner {
    /// Creates the canonical fixture-backed runner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            profile: TassadarWasmProfile::core_i32_v1(),
            trace_abi: TassadarTraceAbi::core_i32_v1(),
            weights: TassadarFixtureWeights::core_i32_v1(),
        }
    }

    /// Returns the handcrafted rule tables backing the fixture runner.
    #[must_use]
    pub fn weights(&self) -> &TassadarFixtureWeights {
        &self.weights
    }

    /// Executes one validated Tassadar program against the fixture runner.
    pub fn execute(
        &self,
        program: &TassadarProgram,
    ) -> Result<TassadarExecution, TassadarExecutionRefusal> {
        execute_program(
            program,
            &self.profile,
            &self.trace_abi,
            TASSADAR_FIXTURE_RUNNER_ID,
            Some(&self.weights),
        )
    }
}

/// Replays one program through both runners and checks exact parity.
pub fn run_tassadar_exact_parity(
    program: &TassadarProgram,
) -> Result<TassadarParityReport, TassadarExecutionRefusal> {
    let reference = TassadarCpuReferenceRunner::new().execute(program)?;
    let fixture = TassadarFixtureRunner::new().execute(program)?;
    let report = TassadarParityReport {
        program_id: program.program_id.clone(),
        reference,
        fixture,
    };
    report.require_exact()?;
    Ok(report)
}

/// Deterministically replays the supplied execution against the direct CPU runner.
pub fn replay_tassadar_execution(
    program: &TassadarProgram,
    expected: &TassadarExecution,
) -> Result<(), TassadarExecutionRefusal> {
    let actual = TassadarCpuReferenceRunner::new().execute(program)?;
    if actual.behavior_digest() == expected.behavior_digest() {
        Ok(())
    } else {
        Err(TassadarExecutionRefusal::ReplayMismatch {
            program_id: program.program_id.clone(),
            expected_digest: expected.behavior_digest(),
            actual_digest: actual.behavior_digest(),
        })
    }
}

/// Small reference corpus that keeps the Phase 1 trace/output boundary honest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TassadarValidationCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Short case summary.
    pub summary: String,
    /// Validated Phase 1 program.
    pub program: TassadarProgram,
    /// Exact expected trace for the case.
    pub expected_trace: Vec<TassadarTraceStep>,
    /// Exact expected output values.
    pub expected_outputs: Vec<i32>,
}

/// Returns the canonical Phase 1 validation corpus.
#[must_use]
pub fn tassadar_validation_corpus() -> Vec<TassadarValidationCase> {
    vec![
        locals_add_case(),
        memory_roundtrip_case(),
        branch_guard_case(),
    ]
}

fn execute_program(
    program: &TassadarProgram,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    runner_id: &str,
    fixture_weights: Option<&TassadarFixtureWeights>,
) -> Result<TassadarExecution, TassadarExecutionRefusal> {
    program.validate_against(profile)?;

    let mut pc = 0usize;
    let mut steps = Vec::new();
    let mut outputs = Vec::new();
    let mut stack = Vec::new();
    let mut locals = vec![0; program.local_count];
    let mut memory = program.initial_memory.clone();
    let mut step_index = 0usize;
    let mut halt_reason = TassadarHaltReason::FellOffEnd;

    while pc < program.instructions.len() {
        if step_index >= profile.max_steps {
            return Err(TassadarExecutionRefusal::StepLimitExceeded {
                max_steps: profile.max_steps,
            });
        }

        let instruction = program.instructions[pc].clone();
        let stack_before = stack.clone();
        let opcode = instruction.opcode();
        let rule = match fixture_weights {
            Some(weights) => Some(
                weights
                    .rule_for(opcode)
                    .ok_or(TassadarExecutionRefusal::FixtureRuleMissing { opcode })?,
            ),
            None => None,
        };
        let mut next_pc = pc + 1;
        let event = match instruction.clone() {
            TassadarInstruction::I32Const { value } => {
                stack.push(value);
                TassadarTraceEvent::ConstPush { value }
            }
            TassadarInstruction::LocalGet { local } => {
                let value = locals[usize::from(local)];
                stack.push(value);
                TassadarTraceEvent::LocalGet { local, value }
            }
            TassadarInstruction::LocalSet { local } => {
                let value = pop_value(&mut stack, pc, 1)?;
                locals[usize::from(local)] = value;
                TassadarTraceEvent::LocalSet { local, value }
            }
            TassadarInstruction::I32Add => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = left + right;
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Add,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Sub => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = left - right;
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Sub,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Mul => {
                let (left, right) = pop_binary_operands(&mut stack, pc)?;
                let result = left * right;
                stack.push(result);
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Mul,
                    left,
                    right,
                    result,
                }
            }
            TassadarInstruction::I32Load { slot } => {
                let value = memory[usize::from(slot)];
                stack.push(value);
                TassadarTraceEvent::Load { slot, value }
            }
            TassadarInstruction::I32Store { slot } => {
                let value = pop_value(&mut stack, pc, 1)?;
                memory[usize::from(slot)] = value;
                TassadarTraceEvent::Store { slot, value }
            }
            TassadarInstruction::BrIf { target_pc } => {
                let condition = pop_value(&mut stack, pc, 1)?;
                let taken = condition != 0;
                if taken {
                    next_pc = usize::from(target_pc);
                }
                TassadarTraceEvent::Branch {
                    condition,
                    taken,
                    target_pc: usize::from(target_pc),
                }
            }
            TassadarInstruction::Output => {
                let value = pop_value(&mut stack, pc, 1)?;
                outputs.push(value);
                TassadarTraceEvent::Output { value }
            }
            TassadarInstruction::Return => {
                halt_reason = TassadarHaltReason::Returned;
                TassadarTraceEvent::Return
            }
        };

        if let Some(rule) = rule {
            let observed = observed_rule_signature(&instruction, &event);
            if rule.pops != observed.pops || rule.pushes != observed.pushes {
                return Err(TassadarExecutionRefusal::FixtureRuleMismatch {
                    opcode,
                    expected_pops: rule.pops,
                    expected_pushes: rule.pushes,
                    actual_pops: observed.pops,
                    actual_pushes: observed.pushes,
                });
            }
        }

        steps.push(TassadarTraceStep {
            step_index,
            pc,
            next_pc,
            instruction: instruction.clone(),
            event,
            stack_before,
            stack_after: stack.clone(),
            locals_after: locals.clone(),
            memory_after: memory.clone(),
        });

        step_index += 1;
        if matches!(instruction, TassadarInstruction::Return) {
            return Ok(TassadarExecution {
                program_id: program.program_id.clone(),
                profile_id: program.profile_id.clone(),
                runner_id: String::from(runner_id),
                trace_abi: trace_abi.clone(),
                steps,
                outputs,
                final_locals: locals,
                final_memory: memory,
                final_stack: stack,
                halt_reason,
            });
        }

        pc = next_pc;
    }

    Ok(TassadarExecution {
        program_id: program.program_id.clone(),
        profile_id: program.profile_id.clone(),
        runner_id: String::from(runner_id),
        trace_abi: trace_abi.clone(),
        steps,
        outputs,
        final_locals: locals,
        final_memory: memory,
        final_stack: stack,
        halt_reason,
    })
}

fn pop_binary_operands(
    stack: &mut Vec<i32>,
    pc: usize,
) -> Result<(i32, i32), TassadarExecutionRefusal> {
    if stack.len() < 2 {
        return Err(TassadarExecutionRefusal::StackUnderflow {
            pc,
            needed: 2,
            available: stack.len(),
        });
    }
    let right = stack.pop().expect("len checked");
    let left = stack.pop().expect("len checked");
    Ok((left, right))
}

fn pop_value(
    stack: &mut Vec<i32>,
    pc: usize,
    needed: usize,
) -> Result<i32, TassadarExecutionRefusal> {
    stack.pop().ok_or(TassadarExecutionRefusal::StackUnderflow {
        pc,
        needed,
        available: 0,
    })
}

fn locals_add_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::core_i32_v1();
    TassadarValidationCase {
        case_id: String::from("locals_add"),
        summary: String::from("local set/get plus addition and output"),
        program: TassadarProgram::new(
            "tassadar.locals_add.v1",
            &profile,
            2,
            2,
            vec![
                TassadarInstruction::I32Const { value: 7 },
                TassadarInstruction::LocalSet { local: 0 },
                TassadarInstruction::I32Const { value: 5 },
                TassadarInstruction::LocalSet { local: 1 },
                TassadarInstruction::LocalGet { local: 0 },
                TassadarInstruction::LocalGet { local: 1 },
                TassadarInstruction::I32Add,
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        ),
        expected_trace: vec![
            trace_step(
                0,
                0,
                1,
                TassadarInstruction::I32Const { value: 7 },
                TassadarTraceEvent::ConstPush { value: 7 },
                &[],
                &[7],
                &[0, 0],
                &[0, 0],
            ),
            trace_step(
                1,
                1,
                2,
                TassadarInstruction::LocalSet { local: 0 },
                TassadarTraceEvent::LocalSet { local: 0, value: 7 },
                &[7],
                &[],
                &[7, 0],
                &[0, 0],
            ),
            trace_step(
                2,
                2,
                3,
                TassadarInstruction::I32Const { value: 5 },
                TassadarTraceEvent::ConstPush { value: 5 },
                &[],
                &[5],
                &[7, 0],
                &[0, 0],
            ),
            trace_step(
                3,
                3,
                4,
                TassadarInstruction::LocalSet { local: 1 },
                TassadarTraceEvent::LocalSet { local: 1, value: 5 },
                &[5],
                &[],
                &[7, 5],
                &[0, 0],
            ),
            trace_step(
                4,
                4,
                5,
                TassadarInstruction::LocalGet { local: 0 },
                TassadarTraceEvent::LocalGet { local: 0, value: 7 },
                &[],
                &[7],
                &[7, 5],
                &[0, 0],
            ),
            trace_step(
                5,
                5,
                6,
                TassadarInstruction::LocalGet { local: 1 },
                TassadarTraceEvent::LocalGet { local: 1, value: 5 },
                &[7],
                &[7, 5],
                &[7, 5],
                &[0, 0],
            ),
            trace_step(
                6,
                6,
                7,
                TassadarInstruction::I32Add,
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Add,
                    left: 7,
                    right: 5,
                    result: 12,
                },
                &[7, 5],
                &[12],
                &[7, 5],
                &[0, 0],
            ),
            trace_step(
                7,
                7,
                8,
                TassadarInstruction::Output,
                TassadarTraceEvent::Output { value: 12 },
                &[12],
                &[],
                &[7, 5],
                &[0, 0],
            ),
            trace_step(
                8,
                8,
                9,
                TassadarInstruction::Return,
                TassadarTraceEvent::Return,
                &[],
                &[],
                &[7, 5],
                &[0, 0],
            ),
        ],
        expected_outputs: vec![12],
    }
}

fn memory_roundtrip_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::core_i32_v1();
    TassadarValidationCase {
        case_id: String::from("memory_roundtrip"),
        summary: String::from("memory store/load plus multiplication and output"),
        program: TassadarProgram::new(
            "tassadar.memory_roundtrip.v1",
            &profile,
            0,
            4,
            vec![
                TassadarInstruction::I32Const { value: 9 },
                TassadarInstruction::I32Store { slot: 2 },
                TassadarInstruction::I32Load { slot: 2 },
                TassadarInstruction::I32Const { value: 3 },
                TassadarInstruction::I32Mul,
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        ),
        expected_trace: vec![
            trace_step(
                0,
                0,
                1,
                TassadarInstruction::I32Const { value: 9 },
                TassadarTraceEvent::ConstPush { value: 9 },
                &[],
                &[9],
                &[],
                &[0, 0, 0, 0],
            ),
            trace_step(
                1,
                1,
                2,
                TassadarInstruction::I32Store { slot: 2 },
                TassadarTraceEvent::Store { slot: 2, value: 9 },
                &[9],
                &[],
                &[],
                &[0, 0, 9, 0],
            ),
            trace_step(
                2,
                2,
                3,
                TassadarInstruction::I32Load { slot: 2 },
                TassadarTraceEvent::Load { slot: 2, value: 9 },
                &[],
                &[9],
                &[],
                &[0, 0, 9, 0],
            ),
            trace_step(
                3,
                3,
                4,
                TassadarInstruction::I32Const { value: 3 },
                TassadarTraceEvent::ConstPush { value: 3 },
                &[9],
                &[9, 3],
                &[],
                &[0, 0, 9, 0],
            ),
            trace_step(
                4,
                4,
                5,
                TassadarInstruction::I32Mul,
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Mul,
                    left: 9,
                    right: 3,
                    result: 27,
                },
                &[9, 3],
                &[27],
                &[],
                &[0, 0, 9, 0],
            ),
            trace_step(
                5,
                5,
                6,
                TassadarInstruction::Output,
                TassadarTraceEvent::Output { value: 27 },
                &[27],
                &[],
                &[],
                &[0, 0, 9, 0],
            ),
            trace_step(
                6,
                6,
                7,
                TassadarInstruction::Return,
                TassadarTraceEvent::Return,
                &[],
                &[],
                &[],
                &[0, 0, 9, 0],
            ),
        ],
        expected_outputs: vec![27],
    }
}

fn branch_guard_case() -> TassadarValidationCase {
    let profile = TassadarWasmProfile::core_i32_v1();
    TassadarValidationCase {
        case_id: String::from("branch_guard"),
        summary: String::from("mul/sub with both untaken and taken conditional branches"),
        program: TassadarProgram::new(
            "tassadar.branch_guard.v1",
            &profile,
            0,
            0,
            vec![
                TassadarInstruction::I32Const { value: 3 },
                TassadarInstruction::I32Const { value: 4 },
                TassadarInstruction::I32Mul,
                TassadarInstruction::I32Const { value: 12 },
                TassadarInstruction::I32Sub,
                TassadarInstruction::BrIf { target_pc: 8 },
                TassadarInstruction::I32Const { value: 7 },
                TassadarInstruction::Output,
                TassadarInstruction::I32Const { value: 1 },
                TassadarInstruction::BrIf { target_pc: 12 },
                TassadarInstruction::I32Const { value: 99 },
                TassadarInstruction::Output,
                TassadarInstruction::Return,
            ],
        ),
        expected_trace: vec![
            trace_step(
                0,
                0,
                1,
                TassadarInstruction::I32Const { value: 3 },
                TassadarTraceEvent::ConstPush { value: 3 },
                &[],
                &[3],
                &[],
                &[],
            ),
            trace_step(
                1,
                1,
                2,
                TassadarInstruction::I32Const { value: 4 },
                TassadarTraceEvent::ConstPush { value: 4 },
                &[3],
                &[3, 4],
                &[],
                &[],
            ),
            trace_step(
                2,
                2,
                3,
                TassadarInstruction::I32Mul,
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Mul,
                    left: 3,
                    right: 4,
                    result: 12,
                },
                &[3, 4],
                &[12],
                &[],
                &[],
            ),
            trace_step(
                3,
                3,
                4,
                TassadarInstruction::I32Const { value: 12 },
                TassadarTraceEvent::ConstPush { value: 12 },
                &[12],
                &[12, 12],
                &[],
                &[],
            ),
            trace_step(
                4,
                4,
                5,
                TassadarInstruction::I32Sub,
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Sub,
                    left: 12,
                    right: 12,
                    result: 0,
                },
                &[12, 12],
                &[0],
                &[],
                &[],
            ),
            trace_step(
                5,
                5,
                6,
                TassadarInstruction::BrIf { target_pc: 8 },
                TassadarTraceEvent::Branch {
                    condition: 0,
                    taken: false,
                    target_pc: 8,
                },
                &[0],
                &[],
                &[],
                &[],
            ),
            trace_step(
                6,
                6,
                7,
                TassadarInstruction::I32Const { value: 7 },
                TassadarTraceEvent::ConstPush { value: 7 },
                &[],
                &[7],
                &[],
                &[],
            ),
            trace_step(
                7,
                7,
                8,
                TassadarInstruction::Output,
                TassadarTraceEvent::Output { value: 7 },
                &[7],
                &[],
                &[],
                &[],
            ),
            trace_step(
                8,
                8,
                9,
                TassadarInstruction::I32Const { value: 1 },
                TassadarTraceEvent::ConstPush { value: 1 },
                &[],
                &[1],
                &[],
                &[],
            ),
            trace_step(
                9,
                9,
                12,
                TassadarInstruction::BrIf { target_pc: 12 },
                TassadarTraceEvent::Branch {
                    condition: 1,
                    taken: true,
                    target_pc: 12,
                },
                &[1],
                &[],
                &[],
                &[],
            ),
            trace_step(
                10,
                12,
                13,
                TassadarInstruction::Return,
                TassadarTraceEvent::Return,
                &[],
                &[],
                &[],
                &[],
            ),
        ],
        expected_outputs: vec![7],
    }
}

fn trace_step(
    step_index: usize,
    pc: usize,
    next_pc: usize,
    instruction: TassadarInstruction,
    event: TassadarTraceEvent,
    stack_before: &[i32],
    stack_after: &[i32],
    locals_after: &[i32],
    memory_after: &[i32],
) -> TassadarTraceStep {
    TassadarTraceStep {
        step_index,
        pc,
        next_pc,
        instruction,
        event,
        stack_before: stack_before.to_vec(),
        stack_after: stack_after.to_vec(),
        locals_after: locals_after.to_vec(),
        memory_after: memory_after.to_vec(),
    }
}

fn observed_rule_signature(
    instruction: &TassadarInstruction,
    event: &TassadarTraceEvent,
) -> TassadarOpcodeRule {
    match (instruction, event) {
        (TassadarInstruction::I32Const { .. }, TassadarTraceEvent::ConstPush { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::I32Const,
                0,
                1,
                TassadarImmediateKind::I32,
                TassadarAccessClass::None,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::LocalGet { .. }, TassadarTraceEvent::LocalGet { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::LocalGet,
                0,
                1,
                TassadarImmediateKind::LocalIndex,
                TassadarAccessClass::LocalRead,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::LocalSet { .. }, TassadarTraceEvent::LocalSet { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::LocalSet,
                1,
                0,
                TassadarImmediateKind::LocalIndex,
                TassadarAccessClass::LocalWrite,
                TassadarControlClass::Linear,
            )
        }
        (
            TassadarInstruction::I32Add,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Add,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Add,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Add),
        (
            TassadarInstruction::I32Sub,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Sub,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Sub,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Sub),
        (
            TassadarInstruction::I32Mul,
            TassadarTraceEvent::BinaryOp {
                op: TassadarArithmeticOp::Mul,
                ..
            },
        ) => TassadarOpcodeRule::new(
            TassadarOpcode::I32Mul,
            2,
            1,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        )
        .with_arithmetic(TassadarArithmeticOp::Mul),
        (TassadarInstruction::I32Load { .. }, TassadarTraceEvent::Load { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::I32Load,
                0,
                1,
                TassadarImmediateKind::MemorySlot,
                TassadarAccessClass::MemoryRead,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::I32Store { .. }, TassadarTraceEvent::Store { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::I32Store,
                1,
                0,
                TassadarImmediateKind::MemorySlot,
                TassadarAccessClass::MemoryWrite,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::BrIf { .. }, TassadarTraceEvent::Branch { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::BrIf,
                1,
                0,
                TassadarImmediateKind::BranchTarget,
                TassadarAccessClass::None,
                TassadarControlClass::ConditionalBranch,
            )
        }
        (TassadarInstruction::Output, TassadarTraceEvent::Output { .. }) => {
            TassadarOpcodeRule::new(
                TassadarOpcode::Output,
                1,
                0,
                TassadarImmediateKind::None,
                TassadarAccessClass::None,
                TassadarControlClass::Linear,
            )
        }
        (TassadarInstruction::Return, TassadarTraceEvent::Return) => TassadarOpcodeRule::new(
            TassadarOpcode::Return,
            0,
            0,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Return,
        ),
        _ => TassadarOpcodeRule::new(
            instruction.opcode(),
            u8::MAX,
            u8::MAX,
            TassadarImmediateKind::None,
            TassadarAccessClass::None,
            TassadarControlClass::Linear,
        ),
    }
}

fn stable_serialized_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn stable_bytes_digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_execution_evidence_bundle, replay_tassadar_execution,
        run_tassadar_exact_parity, tassadar_validation_corpus, TassadarCompilerToolchainIdentity,
        TassadarCpuReferenceRunner, TassadarExecutionRefusal, TassadarExecutorDecodeMode,
        TassadarFixtureRunner, TassadarInstruction, TassadarProgram, TassadarProgramArtifact,
        TassadarProgramArtifactError, TassadarProgramSourceIdentity, TassadarProgramSourceKind,
        TassadarTraceAbi, TassadarWasmProfile,
    };

    #[test]
    fn cpu_reference_runner_matches_exact_trace_fixtures() {
        let runner = TassadarCpuReferenceRunner::new();
        for case in tassadar_validation_corpus() {
            let execution = runner.execute(&case.program).expect("case should run");
            assert_eq!(
                execution.steps, case.expected_trace,
                "case={}",
                case.case_id
            );
            assert_eq!(
                execution.outputs, case.expected_outputs,
                "case={}",
                case.case_id
            );
        }
    }

    #[test]
    fn fixture_runner_matches_exact_trace_fixtures() {
        let runner = TassadarFixtureRunner::new();
        for case in tassadar_validation_corpus() {
            let execution = runner.execute(&case.program).expect("case should run");
            assert_eq!(
                execution.steps, case.expected_trace,
                "case={}",
                case.case_id
            );
            assert_eq!(
                execution.outputs, case.expected_outputs,
                "case={}",
                case.case_id
            );
        }
    }

    #[test]
    fn parity_harness_is_exact_on_validation_corpus() {
        for case in tassadar_validation_corpus() {
            let report = run_tassadar_exact_parity(&case.program).expect("parity should hold");
            report.require_exact().expect("report should be exact");
        }
    }

    #[test]
    fn replay_is_deterministic_on_validation_corpus() {
        let runner = TassadarCpuReferenceRunner::new();
        for case in tassadar_validation_corpus() {
            let execution = runner.execute(&case.program).expect("case should run");
            replay_tassadar_execution(&case.program, &execution)
                .expect("replay should match exactly");
        }
    }

    #[test]
    fn invalid_memory_slot_refuses_with_typed_error() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let program = TassadarProgram::new(
            "tassadar.invalid_memory.v1",
            &profile,
            0,
            1,
            vec![
                TassadarInstruction::I32Load { slot: 1 },
                TassadarInstruction::Return,
            ],
        );
        let error = TassadarCpuReferenceRunner::new()
            .execute(&program)
            .expect_err("invalid slot should refuse");
        assert_eq!(
            error,
            TassadarExecutionRefusal::MemorySlotOutOfRange {
                pc: 0,
                slot: 1,
                memory_slots: 1,
            }
        );
    }

    #[test]
    fn stack_underflow_refuses_with_typed_error() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let program = TassadarProgram::new(
            "tassadar.stack_underflow.v1",
            &profile,
            0,
            0,
            vec![TassadarInstruction::I32Add, TassadarInstruction::Return],
        );
        let error = TassadarCpuReferenceRunner::new()
            .execute(&program)
            .expect_err("underflow should refuse");
        assert_eq!(
            error,
            TassadarExecutionRefusal::StackUnderflow {
                pc: 0,
                needed: 2,
                available: 0,
            }
        );
    }

    #[test]
    fn program_artifact_is_digest_bound_and_internally_consistent() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let artifact = TassadarProgramArtifact::new(
            "tassadar.locals_add.artifact.v1",
            TassadarProgramSourceIdentity::new(
                TassadarProgramSourceKind::Fixture,
                "locals_add",
                "sha256:fixture-source",
            ),
            TassadarCompilerToolchainIdentity::new("clang", "18.1.0", "wasm32-unknown-unknown")
                .with_pipeline_features(vec![String::from("phase2_artifact_contract")]),
            &profile,
            &trace_abi,
            case.program,
        )
        .expect("artifact should assemble");
        artifact
            .validate_internal_consistency()
            .expect("artifact should stay internally consistent");
        assert_eq!(artifact.wasm_profile_id, profile.profile_id);
        assert_eq!(
            artifact.opcode_vocabulary_digest,
            profile.opcode_vocabulary_digest()
        );
    }

    #[test]
    fn program_artifact_rejects_profile_mismatch() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let mut case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        case.program.profile_id = String::from("tassadar.wasm.other.v1");
        let error = TassadarProgramArtifact::fixture_reference(
            "tassadar.bad_profile.artifact.v1",
            &profile,
            &trace_abi,
            case.program,
        )
        .expect_err("profile mismatch should refuse");
        assert_eq!(
            error,
            TassadarProgramArtifactError::ProgramProfileMismatch {
                expected: profile.profile_id,
                actual: String::from("tassadar.wasm.other.v1"),
            }
        );
    }

    #[test]
    fn tassadar_execution_evidence_bundle_is_replay_stable_on_validation_corpus() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let runner = TassadarFixtureRunner::new();

        for case in tassadar_validation_corpus() {
            let artifact = TassadarProgramArtifact::fixture_reference(
                format!("tassadar://artifact/test/{}", case.case_id),
                &profile,
                &trace_abi,
                case.program.clone(),
            )
            .expect("fixture artifact should build");
            let execution = runner
                .execute(&case.program)
                .expect("fixture execution should pass");
            replay_tassadar_execution(&case.program, &execution)
                .expect("replay should match fixture execution");
            let replayed = runner
                .execute(&case.program)
                .expect("replayed execution should pass");

            let first = build_tassadar_execution_evidence_bundle(
                format!("request-{}", case.case_id),
                format!("digest-{}", case.case_id),
                "tassadar_reference_fixture",
                "tassadar-executor-fixture-v0",
                "model-descriptor-digest",
                vec![String::from("env.openagents.tassadar.benchmark@2026.03.15")],
                &artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
                &execution,
            );
            let second = build_tassadar_execution_evidence_bundle(
                format!("request-{}", case.case_id),
                format!("digest-{}", case.case_id),
                "tassadar_reference_fixture",
                "tassadar-executor-fixture-v0",
                "model-descriptor-digest",
                vec![String::from("env.openagents.tassadar.benchmark@2026.03.15")],
                &artifact,
                TassadarExecutorDecodeMode::ReferenceLinear,
                &replayed,
            );

            assert_eq!(
                first.runtime_manifest.identity_digest,
                second.runtime_manifest.identity_digest
            );
            assert_eq!(
                first.runtime_manifest.manifest_digest,
                second.runtime_manifest.manifest_digest
            );
            assert_eq!(
                first.trace_artifact.artifact_digest,
                second.trace_artifact.artifact_digest
            );
            assert_eq!(
                first.trace_proof.proof_digest,
                second.trace_proof.proof_digest
            );
            assert_eq!(
                first.proof_bundle.stable_digest(),
                second.proof_bundle.stable_digest()
            );
        }
    }

    #[test]
    fn tassadar_trace_proof_artifact_carries_required_identity_fields() {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let case = tassadar_validation_corpus()
            .into_iter()
            .next()
            .expect("validation corpus");
        let artifact = TassadarProgramArtifact::fixture_reference(
            "tassadar://artifact/test/proof_fields",
            &profile,
            &trace_abi,
            case.program.clone(),
        )
        .expect("fixture artifact should build");
        let execution = TassadarFixtureRunner::new()
            .execute(&case.program)
            .expect("fixture execution should pass");
        let evidence = build_tassadar_execution_evidence_bundle(
            "request-proof-fields",
            "digest-proof-fields",
            "tassadar_reference_fixture",
            "tassadar-executor-fixture-v0",
            "model-descriptor-digest",
            vec![String::from("env.openagents.tassadar.benchmark@2026.03.15")],
            &artifact,
            TassadarExecutorDecodeMode::ReferenceLinear,
            &execution,
        );

        assert_eq!(
            evidence.trace_proof.trace_digest,
            evidence.trace_artifact.trace_digest
        );
        assert_eq!(
            evidence.trace_proof.program_digest,
            artifact.validated_program_digest
        );
        assert_eq!(
            evidence.trace_proof.wasm_profile_id,
            artifact.wasm_profile_id
        );
        assert_eq!(
            evidence.trace_proof.cache_algorithm_id,
            "tassadar.cache.linear_scan_kv.v1"
        );
        assert_eq!(evidence.trace_proof.runtime_backend, "cpu");
        assert_eq!(
            evidence.trace_proof.reference_runner_id,
            execution.runner_id
        );
        assert_eq!(
            evidence.trace_proof.runtime_manifest_identity_digest,
            evidence.runtime_manifest.identity_digest
        );
    }
}
