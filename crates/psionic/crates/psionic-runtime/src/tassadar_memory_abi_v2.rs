use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_LINEAR_MEMORY_ABI_ID: &str = "tassadar.memory.linear.v2";
const TASSADAR_SLOT_MEMORY_ABI_ID: &str = "tassadar.memory.slot_i32.v1";
const TASSADAR_LINEAR_MEMORY_ABI_SCHEMA_VERSION: u16 = 2;
const TASSADAR_SLOT_MEMORY_ABI_SCHEMA_VERSION: u16 = 1;
const TASSADAR_LINEAR_MEMORY_MAX_STEPS: usize = 4_096;
pub const TASSADAR_LINEAR_MEMORY_PAGE_BYTES: u32 = 65_536;

/// Public addressing family for one Tassadar memory ABI contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMemoryAddressingMode {
    /// Legacy fixed-width `i32` memory slots.
    FixedI32Slots,
    /// Byte-addressed linear memory with Wasm-like paging semantics.
    ByteAddressedLinearMemory,
}

/// Public trace strategy for one Tassadar memory ABI contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMemoryTraceMode {
    /// Full memory snapshots are emitted per step.
    FullSnapshots,
    /// Only byte-write and growth deltas are emitted per step.
    DeltaOriented,
}

/// Supported memory access width for the byte-addressed linear-memory lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLinearMemoryWidth {
    /// One byte.
    I8,
    /// Two bytes.
    I16,
    /// Four bytes.
    I32,
}

impl TassadarLinearMemoryWidth {
    /// Returns the width in bytes.
    #[must_use]
    pub const fn byte_width(self) -> u32 {
        match self {
            Self::I8 => 1,
            Self::I16 => 2,
            Self::I32 => 4,
        }
    }
}

/// Machine-legible memory ABI contract for one Tassadar lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemoryAbiContract {
    /// Stable ABI identifier.
    pub abi_id: String,
    /// Stable schema version.
    pub schema_version: u16,
    /// Declared memory addressing family.
    pub addressing_mode: TassadarMemoryAddressingMode,
    /// Page width when the memory model is page-addressed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_bytes: Option<u32>,
    /// Supported load widths.
    pub supported_load_widths: Vec<TassadarLinearMemoryWidth>,
    /// Supported store widths.
    pub supported_store_widths: Vec<TassadarLinearMemoryWidth>,
    /// Whether signed extension on narrow loads is supported.
    pub supports_sign_extension: bool,
    /// Whether `memory.size` is supported.
    pub supports_memory_size: bool,
    /// Whether `memory.grow` is supported.
    pub supports_memory_grow: bool,
    /// Whether `memory.copy` is supported.
    pub supports_memory_copy: bool,
    /// Whether `memory.fill` is supported.
    pub supports_memory_fill: bool,
    /// Declared trace strategy for memory state.
    pub trace_mode: TassadarMemoryTraceMode,
}

impl TassadarMemoryAbiContract {
    /// Returns the legacy fixed-slot memory ABI contract.
    #[must_use]
    pub fn slot_i32_v1() -> Self {
        Self {
            abi_id: String::from(TASSADAR_SLOT_MEMORY_ABI_ID),
            schema_version: TASSADAR_SLOT_MEMORY_ABI_SCHEMA_VERSION,
            addressing_mode: TassadarMemoryAddressingMode::FixedI32Slots,
            page_bytes: None,
            supported_load_widths: vec![TassadarLinearMemoryWidth::I32],
            supported_store_widths: vec![TassadarLinearMemoryWidth::I32],
            supports_sign_extension: false,
            supports_memory_size: false,
            supports_memory_grow: false,
            supports_memory_copy: false,
            supports_memory_fill: false,
            trace_mode: TassadarMemoryTraceMode::FullSnapshots,
        }
    }

    /// Returns the byte-addressed linear-memory ABI v2 contract.
    #[must_use]
    pub fn linear_memory_v2() -> Self {
        Self {
            abi_id: String::from(TASSADAR_LINEAR_MEMORY_ABI_ID),
            schema_version: TASSADAR_LINEAR_MEMORY_ABI_SCHEMA_VERSION,
            addressing_mode: TassadarMemoryAddressingMode::ByteAddressedLinearMemory,
            page_bytes: Some(TASSADAR_LINEAR_MEMORY_PAGE_BYTES),
            supported_load_widths: vec![
                TassadarLinearMemoryWidth::I8,
                TassadarLinearMemoryWidth::I16,
                TassadarLinearMemoryWidth::I32,
            ],
            supported_store_widths: vec![
                TassadarLinearMemoryWidth::I8,
                TassadarLinearMemoryWidth::I16,
                TassadarLinearMemoryWidth::I32,
            ],
            supports_sign_extension: true,
            supports_memory_size: true,
            supports_memory_grow: true,
            supports_memory_copy: true,
            supports_memory_fill: true,
            trace_mode: TassadarMemoryTraceMode::DeltaOriented,
        }
    }

    /// Returns a stable digest over the contract contents.
    #[must_use]
    pub fn compatibility_digest(&self) -> String {
        stable_digest(b"tassadar_memory_abi_contract|", self)
    }
}

/// One validated byte-addressed instruction for the memory ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "opcode", rename_all = "snake_case")]
pub enum TassadarLinearMemoryInstruction {
    /// Push one `i32` constant.
    I32Const {
        /// Literal immediate value.
        value: i32,
    },
    /// Load bytes from one immediate byte address.
    I32Load {
        /// Byte address in linear memory.
        address: u32,
        /// Width to read.
        width: TassadarLinearMemoryWidth,
        /// Whether the value should be sign-extended.
        signed: bool,
    },
    /// Store bytes to one immediate byte address.
    I32Store {
        /// Byte address in linear memory.
        address: u32,
        /// Width to write.
        width: TassadarLinearMemoryWidth,
    },
    /// Push the current memory size in pages.
    MemorySize,
    /// Pop one page delta and grow memory if possible.
    MemoryGrow,
    /// Pop `len`, `src`, and `dst` and copy bytes with memmove semantics.
    MemoryCopy,
    /// Pop `len`, `value`, and `dst` and fill bytes with one byte.
    MemoryFill,
    /// Pop and emit one output value.
    Output,
    /// Halt successfully.
    Return,
}

/// One byte-addressed program for the memory ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinearMemoryProgram {
    /// Stable program identifier.
    pub program_id: String,
    /// Declared memory ABI contract.
    pub memory_abi: TassadarMemoryAbiContract,
    /// Initial page count.
    pub initial_pages: u32,
    /// Maximum page count allowed for `memory.grow`.
    pub max_pages: u32,
    /// Initial memory image to copy into the linear-memory prefix.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial_memory: Vec<u8>,
    /// Ordered instruction sequence.
    pub instructions: Vec<TassadarLinearMemoryInstruction>,
}

impl TassadarLinearMemoryProgram {
    /// Creates a new linear-memory program under the canonical v2 ABI.
    #[must_use]
    pub fn new(
        program_id: impl Into<String>,
        initial_pages: u32,
        max_pages: u32,
        instructions: Vec<TassadarLinearMemoryInstruction>,
    ) -> Self {
        Self {
            program_id: program_id.into(),
            memory_abi: TassadarMemoryAbiContract::linear_memory_v2(),
            initial_pages,
            max_pages,
            initial_memory: Vec::new(),
            instructions,
        }
    }

    /// Replaces the initial memory image.
    #[must_use]
    pub fn with_initial_memory(mut self, initial_memory: Vec<u8>) -> Self {
        self.initial_memory = initial_memory;
        self
    }

    fn validate(&self) -> Result<(), TassadarLinearMemoryExecutionError> {
        let expected_abi = TassadarMemoryAbiContract::linear_memory_v2();
        if self.memory_abi != expected_abi {
            return Err(TassadarLinearMemoryExecutionError::AbiMismatch {
                expected: expected_abi.compatibility_digest(),
                actual: self.memory_abi.compatibility_digest(),
            });
        }
        if self.max_pages < self.initial_pages {
            return Err(TassadarLinearMemoryExecutionError::MaxPagesBeforeInitial {
                initial_pages: self.initial_pages,
                max_pages: self.max_pages,
            });
        }
        let declared_bytes = linear_memory_len_bytes(self.initial_pages)?;
        if self.initial_memory.len() > declared_bytes {
            return Err(TassadarLinearMemoryExecutionError::InitialMemoryTooLarge {
                initial_bytes: self.initial_memory.len(),
                declared_bytes,
            });
        }
        Ok(())
    }
}

/// One byte-level memory write delta.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinearMemoryByteDelta {
    /// Byte address touched by the step.
    pub address: u32,
    /// Byte value before the step.
    pub before: u8,
    /// Byte value after the step.
    pub after: u8,
}

/// One memory-growth delta.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinearMemoryGrowthDelta {
    /// Page count before the growth.
    pub previous_pages: u32,
    /// Page count after the growth.
    pub new_pages: u32,
    /// Byte count added by the growth.
    pub added_bytes: u32,
}

/// One emitted trace event in the byte-addressed linear-memory lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarLinearMemoryTraceEvent {
    /// One constant was pushed.
    ConstPush {
        /// Value pushed to the stack.
        value: i32,
    },
    /// One load completed.
    Load {
        /// Byte address read.
        address: u32,
        /// Load width.
        width: TassadarLinearMemoryWidth,
        /// Whether the load used sign extension.
        signed: bool,
        /// Raw bytes read from memory.
        raw_bytes: Vec<u8>,
        /// Loaded `i32` value.
        value: i32,
    },
    /// One store completed.
    Store {
        /// Byte address written.
        address: u32,
        /// Store width.
        width: TassadarLinearMemoryWidth,
        /// Source `i32` value before truncation.
        value: i32,
        /// Raw bytes written to memory.
        written_bytes: Vec<u8>,
    },
    /// One `memory.size` observation completed.
    MemorySize {
        /// Size in pages observed by the step.
        pages: u32,
    },
    /// One `memory.grow` attempt completed.
    MemoryGrow {
        /// Requested delta in pages.
        requested_pages: i32,
        /// Previous size in pages.
        previous_pages: u32,
        /// Result pushed to the stack.
        result: i32,
    },
    /// One `memory.copy` completed.
    MemoryCopy {
        /// Destination byte address.
        dst_address: u32,
        /// Source byte address.
        src_address: u32,
        /// Byte length copied.
        byte_len: u32,
    },
    /// One `memory.fill` completed.
    MemoryFill {
        /// Destination byte address.
        address: u32,
        /// Repeated byte value.
        value: u8,
        /// Byte length filled.
        byte_len: u32,
    },
    /// One output value was emitted.
    Output {
        /// Value emitted by the host-side output sink.
        value: i32,
    },
    /// Execution returned successfully.
    Return,
}

/// One append-only step in the linear-memory trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinearMemoryTraceStep {
    /// Step index in execution order.
    pub step_index: usize,
    /// Program counter before executing the step.
    pub pc: usize,
    /// Program counter after executing the step.
    pub next_pc: usize,
    /// Instruction executed at `pc`.
    pub instruction: TassadarLinearMemoryInstruction,
    /// Event emitted by the step.
    pub event: TassadarLinearMemoryTraceEvent,
    /// Stack snapshot before the step.
    pub stack_before: Vec<i32>,
    /// Stack snapshot after the step.
    pub stack_after: Vec<i32>,
    /// Byte deltas emitted by the step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory_byte_deltas: Vec<TassadarLinearMemoryByteDelta>,
    /// Memory-growth summary when one occurred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_growth_delta: Option<TassadarLinearMemoryGrowthDelta>,
    /// Memory size in pages after the step.
    pub memory_size_pages_after: u32,
}

/// Terminal reason for one linear-memory execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLinearMemoryHaltReason {
    /// The program executed `return`.
    Returned,
    /// The program advanced beyond the end of the instruction list.
    FellOffEnd,
}

/// One complete execution result for the byte-addressed linear-memory lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinearMemoryExecution {
    /// Stable program identifier.
    pub program_id: String,
    /// Declared memory ABI contract.
    pub memory_abi: TassadarMemoryAbiContract,
    /// Initial memory image used by the run.
    pub initial_memory: Vec<u8>,
    /// Initial page count.
    pub initial_pages: u32,
    /// Ordered append-only steps.
    pub steps: Vec<TassadarLinearMemoryTraceStep>,
    /// Output values emitted by the program.
    pub outputs: Vec<i32>,
    /// Final linear-memory bytes.
    pub final_memory: Vec<u8>,
    /// Final stack contents.
    pub final_stack: Vec<i32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarLinearMemoryHaltReason,
}

/// Trace-footprint summary for one linear-memory execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLinearMemoryTraceFootprint {
    /// Serialized byte count for the delta-oriented trace.
    pub delta_trace_bytes: u64,
    /// Serialized byte count for an equivalent full-snapshot trace.
    pub equivalent_full_snapshot_trace_bytes: u64,
    /// Total byte-write deltas emitted by the trace.
    pub byte_delta_count: u64,
    /// Total number of growth events emitted by the trace.
    pub memory_grow_event_count: u32,
}

/// Typed execution failures for the linear-memory ABI v2 lane.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TassadarLinearMemoryExecutionError {
    /// The program declared the wrong memory ABI contract.
    #[error("linear-memory ABI mismatch: expected {expected}, got {actual}")]
    AbiMismatch {
        /// Expected contract digest.
        expected: String,
        /// Actual contract digest.
        actual: String,
    },
    /// The initial memory image exceeded the declared page size.
    #[error(
        "initial linear memory image too large: declared {declared_bytes} bytes, got {initial_bytes}"
    )]
    InitialMemoryTooLarge {
        /// Number of initial bytes supplied.
        initial_bytes: usize,
        /// Number of bytes declared by the initial page count.
        declared_bytes: usize,
    },
    /// The max page count was smaller than the initial page count.
    #[error("max_pages {max_pages} is smaller than initial_pages {initial_pages}")]
    MaxPagesBeforeInitial {
        /// Initial page count.
        initial_pages: u32,
        /// Maximum page count.
        max_pages: u32,
    },
    /// One byte address fell outside the current memory range.
    #[error(
        "linear memory access at pc {pc} is out of range: address={address}, width_bytes={width_bytes}, memory_len={memory_len}"
    )]
    AddressOutOfRange {
        /// Program counter for the access.
        pc: usize,
        /// Starting byte address.
        address: u32,
        /// Access width in bytes.
        width_bytes: u32,
        /// Current memory length in bytes.
        memory_len: usize,
    },
    /// The program needed more stack items than were available.
    #[error("stack underflow at pc {pc}: needed {needed}, available {available}")]
    StackUnderflow {
        /// Program counter.
        pc: usize,
        /// Number of values required.
        needed: usize,
        /// Number of values available.
        available: usize,
    },
    /// The execution exceeded the bounded step limit for this lane.
    #[error("linear memory execution exceeded the step limit of {max_steps}")]
    StepLimitExceeded {
        /// Maximum supported steps.
        max_steps: usize,
    },
    /// Page arithmetic overflowed the supported host representation.
    #[error("linear memory byte length overflowed host usize")]
    ByteLengthOverflow,
    /// JSON serialization failed when computing the trace footprint.
    #[error("failed to serialize one trace view: {0}")]
    FootprintSerialization(String),
}

/// Executes one byte-addressed linear-memory program under the v2 ABI.
pub fn execute_tassadar_linear_memory_program(
    program: &TassadarLinearMemoryProgram,
) -> Result<TassadarLinearMemoryExecution, TassadarLinearMemoryExecutionError> {
    program.validate()?;
    let initial_len = linear_memory_len_bytes(program.initial_pages)?;
    let mut memory = vec![0u8; initial_len];
    memory[..program.initial_memory.len()].copy_from_slice(program.initial_memory.as_slice());

    let mut stack = Vec::new();
    let mut outputs = Vec::new();
    let mut steps = Vec::new();
    let mut pc = 0usize;
    let mut step_index = 0usize;

    let halt_reason = loop {
        if step_index >= TASSADAR_LINEAR_MEMORY_MAX_STEPS {
            return Err(TassadarLinearMemoryExecutionError::StepLimitExceeded {
                max_steps: TASSADAR_LINEAR_MEMORY_MAX_STEPS,
            });
        }
        if pc >= program.instructions.len() {
            break TassadarLinearMemoryHaltReason::FellOffEnd;
        }

        let instruction = program.instructions[pc].clone();
        let stack_before = stack.clone();
        let mut memory_byte_deltas = Vec::new();
        let mut memory_growth_delta = None;
        let (event, next_pc) = match instruction.clone() {
            TassadarLinearMemoryInstruction::I32Const { value } => {
                stack.push(value);
                (TassadarLinearMemoryTraceEvent::ConstPush { value }, pc + 1)
            }
            TassadarLinearMemoryInstruction::I32Load {
                address,
                width,
                signed,
            } => {
                let raw_bytes = read_linear_memory(&memory, pc, address, width)?;
                let value = decode_i32_from_bytes(raw_bytes.as_slice(), width, signed);
                stack.push(value);
                (
                    TassadarLinearMemoryTraceEvent::Load {
                        address,
                        width,
                        signed,
                        raw_bytes,
                        value,
                    },
                    pc + 1,
                )
            }
            TassadarLinearMemoryInstruction::I32Store { address, width } => {
                let value = pop_stack_value(&mut stack, pc)?;
                let written_bytes = encode_i32_to_bytes(value, width);
                let base = checked_memory_range(pc, address, width, memory.len())?;
                for (offset, byte) in written_bytes.iter().enumerate() {
                    let index = base + offset;
                    let before = memory[index];
                    memory[index] = *byte;
                    if before != *byte {
                        memory_byte_deltas.push(TassadarLinearMemoryByteDelta {
                            address: address.saturating_add(offset as u32),
                            before,
                            after: *byte,
                        });
                    }
                }
                (
                    TassadarLinearMemoryTraceEvent::Store {
                        address,
                        width,
                        value,
                        written_bytes,
                    },
                    pc + 1,
                )
            }
            TassadarLinearMemoryInstruction::MemorySize => {
                let pages = current_memory_pages(memory.len())?;
                stack.push(i32::try_from(pages).unwrap_or(i32::MAX));
                (TassadarLinearMemoryTraceEvent::MemorySize { pages }, pc + 1)
            }
            TassadarLinearMemoryInstruction::MemoryGrow => {
                let requested_pages = pop_stack_value(&mut stack, pc)?;
                let previous_pages = current_memory_pages(memory.len())?;
                let result = if requested_pages < 0 {
                    -1
                } else {
                    let requested_pages = requested_pages as u32;
                    if requested_pages == 0 {
                        i32::try_from(previous_pages).unwrap_or(i32::MAX)
                    } else {
                        let Some(new_pages) = previous_pages.checked_add(requested_pages) else {
                            stack.push(-1);
                            let result = -1;
                            let event = TassadarLinearMemoryTraceEvent::MemoryGrow {
                                requested_pages: requested_pages as i32,
                                previous_pages,
                                result,
                            };
                            steps.push(TassadarLinearMemoryTraceStep {
                                step_index,
                                pc,
                                next_pc: pc + 1,
                                instruction,
                                event,
                                stack_before,
                                stack_after: stack.clone(),
                                memory_byte_deltas,
                                memory_growth_delta,
                                memory_size_pages_after: previous_pages,
                            });
                            step_index = step_index.saturating_add(1);
                            pc += 1;
                            continue;
                        };
                        if new_pages > program.max_pages {
                            -1
                        } else {
                            let old_pages = previous_pages;
                            let new_len = linear_memory_len_bytes(new_pages)?;
                            let old_len = memory.len();
                            memory.resize(new_len, 0);
                            memory_growth_delta = Some(TassadarLinearMemoryGrowthDelta {
                                previous_pages: old_pages,
                                new_pages,
                                added_bytes: u32::try_from(new_len.saturating_sub(old_len))
                                    .unwrap_or(u32::MAX),
                            });
                            i32::try_from(old_pages).unwrap_or(i32::MAX)
                        }
                    }
                };
                stack.push(result);
                (
                    TassadarLinearMemoryTraceEvent::MemoryGrow {
                        requested_pages,
                        previous_pages,
                        result,
                    },
                    pc + 1,
                )
            }
            TassadarLinearMemoryInstruction::MemoryCopy => {
                let byte_len = pop_stack_value(&mut stack, pc)? as u32;
                let src_address = pop_stack_value(&mut stack, pc)? as u32;
                let dst_address = pop_stack_value(&mut stack, pc)? as u32;
                let src_base = checked_memory_range_bytes(pc, src_address, byte_len, memory.len())?;
                let dst_base = checked_memory_range_bytes(pc, dst_address, byte_len, memory.len())?;
                let copy_bytes = memory[src_base..src_base + byte_len as usize].to_vec();
                for (offset, byte) in copy_bytes.iter().enumerate() {
                    let index = dst_base + offset;
                    let before = memory[index];
                    memory[index] = *byte;
                    if before != *byte {
                        memory_byte_deltas.push(TassadarLinearMemoryByteDelta {
                            address: dst_address.saturating_add(offset as u32),
                            before,
                            after: *byte,
                        });
                    }
                }
                (
                    TassadarLinearMemoryTraceEvent::MemoryCopy {
                        dst_address,
                        src_address,
                        byte_len,
                    },
                    pc + 1,
                )
            }
            TassadarLinearMemoryInstruction::MemoryFill => {
                let byte_len = pop_stack_value(&mut stack, pc)? as u32;
                let value = pop_stack_value(&mut stack, pc)? as u8;
                let address = pop_stack_value(&mut stack, pc)? as u32;
                let base = checked_memory_range_bytes(pc, address, byte_len, memory.len())?;
                for offset in 0..byte_len as usize {
                    let index = base + offset;
                    let before = memory[index];
                    memory[index] = value;
                    if before != value {
                        memory_byte_deltas.push(TassadarLinearMemoryByteDelta {
                            address: address.saturating_add(offset as u32),
                            before,
                            after: value,
                        });
                    }
                }
                (
                    TassadarLinearMemoryTraceEvent::MemoryFill {
                        address,
                        value,
                        byte_len,
                    },
                    pc + 1,
                )
            }
            TassadarLinearMemoryInstruction::Output => {
                let value = pop_stack_value(&mut stack, pc)?;
                outputs.push(value);
                (TassadarLinearMemoryTraceEvent::Output { value }, pc + 1)
            }
            TassadarLinearMemoryInstruction::Return => {
                steps.push(TassadarLinearMemoryTraceStep {
                    step_index,
                    pc,
                    next_pc: pc + 1,
                    instruction,
                    event: TassadarLinearMemoryTraceEvent::Return,
                    stack_before,
                    stack_after: stack.clone(),
                    memory_byte_deltas,
                    memory_growth_delta,
                    memory_size_pages_after: current_memory_pages(memory.len())?,
                });
                break TassadarLinearMemoryHaltReason::Returned;
            }
        };

        steps.push(TassadarLinearMemoryTraceStep {
            step_index,
            pc,
            next_pc,
            instruction,
            event,
            stack_before,
            stack_after: stack.clone(),
            memory_byte_deltas,
            memory_growth_delta,
            memory_size_pages_after: current_memory_pages(memory.len())?,
        });
        step_index = step_index.saturating_add(1);
        pc = next_pc;
    };

    Ok(TassadarLinearMemoryExecution {
        program_id: program.program_id.clone(),
        memory_abi: program.memory_abi.clone(),
        initial_memory: program.initial_memory.clone(),
        initial_pages: program.initial_pages,
        steps,
        outputs,
        final_memory: memory,
        final_stack: stack,
        halt_reason,
    })
}

/// Computes the serialized trace footprint for one linear-memory execution.
pub fn summarize_tassadar_linear_memory_trace_footprint(
    execution: &TassadarLinearMemoryExecution,
) -> Result<TassadarLinearMemoryTraceFootprint, TassadarLinearMemoryExecutionError> {
    let delta_trace_bytes = serde_json::to_vec(execution)
        .map_err(|error| {
            TassadarLinearMemoryExecutionError::FootprintSerialization(error.to_string())
        })?
        .len() as u64;

    let mut memory = vec![0u8; linear_memory_len_bytes(execution.initial_pages)?];
    memory[..execution.initial_memory.len()].copy_from_slice(execution.initial_memory.as_slice());
    let mut synthetic_steps = Vec::with_capacity(execution.steps.len());
    let mut byte_delta_count = 0u64;
    let mut memory_grow_event_count = 0u32;

    for step in execution.steps.iter() {
        for delta in step.memory_byte_deltas.iter() {
            let index = usize::try_from(delta.address)
                .map_err(|_| TassadarLinearMemoryExecutionError::ByteLengthOverflow)?;
            memory[index] = delta.after;
        }
        if let Some(growth) = &step.memory_growth_delta {
            memory.resize(linear_memory_len_bytes(growth.new_pages)?, 0);
            memory_grow_event_count = memory_grow_event_count.saturating_add(1);
        }
        byte_delta_count = byte_delta_count
            .saturating_add(u64::try_from(step.memory_byte_deltas.len()).unwrap_or(u64::MAX));
        synthetic_steps.push(FullSnapshotTraceStep {
            step_index: step.step_index,
            pc: step.pc,
            next_pc: step.next_pc,
            instruction: step.instruction.clone(),
            event: step.event.clone(),
            stack_before: step.stack_before.clone(),
            stack_after: step.stack_after.clone(),
            memory_after: memory.clone(),
        });
    }

    let equivalent_full_snapshot_trace_bytes = serde_json::to_vec(&synthetic_steps)
        .map_err(|error| {
            TassadarLinearMemoryExecutionError::FootprintSerialization(error.to_string())
        })?
        .len() as u64;

    Ok(TassadarLinearMemoryTraceFootprint {
        delta_trace_bytes,
        equivalent_full_snapshot_trace_bytes,
        byte_delta_count,
        memory_grow_event_count,
    })
}

/// Returns a seeded width-parity program for the byte-addressed v2 lane.
#[must_use]
pub fn tassadar_seeded_linear_memory_width_parity_program() -> TassadarLinearMemoryProgram {
    TassadarLinearMemoryProgram::new(
        "tassadar.memory_abi_v2.width_parity.v1",
        1,
        1,
        vec![
            TassadarLinearMemoryInstruction::I32Const { value: 0x1234_5678 },
            TassadarLinearMemoryInstruction::I32Store {
                address: 0,
                width: TassadarLinearMemoryWidth::I32,
            },
            TassadarLinearMemoryInstruction::I32Load {
                address: 0,
                width: TassadarLinearMemoryWidth::I8,
                signed: false,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::I32Load {
                address: 0,
                width: TassadarLinearMemoryWidth::I16,
                signed: false,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::I32Load {
                address: 0,
                width: TassadarLinearMemoryWidth::I32,
                signed: false,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::I32Const { value: -1 },
            TassadarLinearMemoryInstruction::I32Store {
                address: 4,
                width: TassadarLinearMemoryWidth::I8,
            },
            TassadarLinearMemoryInstruction::I32Load {
                address: 4,
                width: TassadarLinearMemoryWidth::I8,
                signed: false,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::Return,
        ],
    )
}

/// Returns a seeded sign-extension program for the byte-addressed v2 lane.
#[must_use]
pub fn tassadar_seeded_linear_memory_sign_extension_program() -> TassadarLinearMemoryProgram {
    TassadarLinearMemoryProgram::new(
        "tassadar.memory_abi_v2.sign_extension.v1",
        1,
        1,
        vec![
            TassadarLinearMemoryInstruction::I32Load {
                address: 0,
                width: TassadarLinearMemoryWidth::I8,
                signed: true,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::I32Load {
                address: 0,
                width: TassadarLinearMemoryWidth::I8,
                signed: false,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::I32Load {
                address: 0,
                width: TassadarLinearMemoryWidth::I16,
                signed: true,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::I32Load {
                address: 0,
                width: TassadarLinearMemoryWidth::I16,
                signed: false,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::Return,
        ],
    )
    .with_initial_memory(vec![0x80, 0xFF])
}

/// Returns a seeded `memory.size` / `memory.grow` program for the v2 lane.
#[must_use]
pub fn tassadar_seeded_linear_memory_growth_program() -> TassadarLinearMemoryProgram {
    TassadarLinearMemoryProgram::new(
        "tassadar.memory_abi_v2.memory_grow.v1",
        1,
        3,
        vec![
            TassadarLinearMemoryInstruction::MemorySize,
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::I32Const { value: 1 },
            TassadarLinearMemoryInstruction::MemoryGrow,
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::MemorySize,
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::I32Const { value: 5 },
            TassadarLinearMemoryInstruction::MemoryGrow,
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::MemorySize,
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::Return,
        ],
    )
}

/// Returns a seeded memcpy-style program for the byte-addressed v2 lane.
#[must_use]
pub fn tassadar_seeded_linear_memory_memcpy_program(
    byte_count: u32,
) -> TassadarLinearMemoryProgram {
    let destination_base = 256u32;
    let mut instructions = Vec::with_capacity((byte_count as usize * 2) + 3);
    for offset in 0..byte_count {
        instructions.push(TassadarLinearMemoryInstruction::I32Load {
            address: offset,
            width: TassadarLinearMemoryWidth::I8,
            signed: false,
        });
        instructions.push(TassadarLinearMemoryInstruction::I32Store {
            address: destination_base + offset,
            width: TassadarLinearMemoryWidth::I8,
        });
    }
    instructions.push(TassadarLinearMemoryInstruction::I32Load {
        address: destination_base + byte_count.saturating_sub(1),
        width: TassadarLinearMemoryWidth::I8,
        signed: false,
    });
    instructions.push(TassadarLinearMemoryInstruction::Output);
    instructions.push(TassadarLinearMemoryInstruction::Return);

    let mut initial_memory = vec![0u8; usize::try_from(destination_base + byte_count).unwrap_or(0)];
    for offset in 0..byte_count {
        initial_memory[offset as usize] = ((offset * 3 + 1) % 251) as u8;
    }

    TassadarLinearMemoryProgram::new(
        format!("tassadar.memory_abi_v2.memcpy.b{byte_count}.v1"),
        1,
        1,
        instructions,
    )
    .with_initial_memory(initial_memory)
}

/// Returns one seeded copy-and-fill program for the byte-addressed v2 lane.
#[must_use]
pub fn tassadar_seeded_linear_memory_copy_fill_program() -> TassadarLinearMemoryProgram {
    TassadarLinearMemoryProgram::new(
        "tassadar.memory_abi_v2.copy_fill.v1",
        1,
        2,
        vec![
            TassadarLinearMemoryInstruction::I32Const { value: 16 },
            TassadarLinearMemoryInstruction::I32Const { value: 0 },
            TassadarLinearMemoryInstruction::I32Const { value: 8 },
            TassadarLinearMemoryInstruction::MemoryCopy,
            TassadarLinearMemoryInstruction::I32Const { value: 32 },
            TassadarLinearMemoryInstruction::I32Const { value: 0x7F },
            TassadarLinearMemoryInstruction::I32Const { value: 4 },
            TassadarLinearMemoryInstruction::MemoryFill,
            TassadarLinearMemoryInstruction::I32Load {
                address: 16,
                width: TassadarLinearMemoryWidth::I32,
                signed: false,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::I32Load {
                address: 20,
                width: TassadarLinearMemoryWidth::I32,
                signed: false,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::I32Load {
                address: 32,
                width: TassadarLinearMemoryWidth::I32,
                signed: false,
            },
            TassadarLinearMemoryInstruction::Output,
            TassadarLinearMemoryInstruction::Return,
        ],
    )
    .with_initial_memory(vec![1, 2, 3, 4, 5, 6, 7, 8])
}

fn read_linear_memory(
    memory: &[u8],
    pc: usize,
    address: u32,
    width: TassadarLinearMemoryWidth,
) -> Result<Vec<u8>, TassadarLinearMemoryExecutionError> {
    let base = checked_memory_range(pc, address, width, memory.len())?;
    let width_bytes = width.byte_width() as usize;
    Ok(memory[base..base + width_bytes].to_vec())
}

fn checked_memory_range(
    pc: usize,
    address: u32,
    width: TassadarLinearMemoryWidth,
    memory_len: usize,
) -> Result<usize, TassadarLinearMemoryExecutionError> {
    let base = usize::try_from(address)
        .map_err(|_| TassadarLinearMemoryExecutionError::ByteLengthOverflow)?;
    let width_bytes = width.byte_width();
    let end = base
        .checked_add(width_bytes as usize)
        .ok_or(TassadarLinearMemoryExecutionError::ByteLengthOverflow)?;
    if end > memory_len {
        return Err(TassadarLinearMemoryExecutionError::AddressOutOfRange {
            pc,
            address,
            width_bytes,
            memory_len,
        });
    }
    Ok(base)
}

fn checked_memory_range_bytes(
    pc: usize,
    address: u32,
    byte_len: u32,
    memory_len: usize,
) -> Result<usize, TassadarLinearMemoryExecutionError> {
    let base = usize::try_from(address)
        .map_err(|_| TassadarLinearMemoryExecutionError::ByteLengthOverflow)?;
    let end = base
        .checked_add(byte_len as usize)
        .ok_or(TassadarLinearMemoryExecutionError::ByteLengthOverflow)?;
    if end > memory_len {
        return Err(TassadarLinearMemoryExecutionError::AddressOutOfRange {
            pc,
            address,
            width_bytes: byte_len,
            memory_len,
        });
    }
    Ok(base)
}

fn decode_i32_from_bytes(bytes: &[u8], width: TassadarLinearMemoryWidth, signed: bool) -> i32 {
    match (width, signed) {
        (TassadarLinearMemoryWidth::I8, false) => i32::from(bytes[0]),
        (TassadarLinearMemoryWidth::I8, true) => i32::from(i8::from_le_bytes([bytes[0]])),
        (TassadarLinearMemoryWidth::I16, false) => {
            i32::from(u16::from_le_bytes([bytes[0], bytes[1]]))
        }
        (TassadarLinearMemoryWidth::I16, true) => {
            i32::from(i16::from_le_bytes([bytes[0], bytes[1]]))
        }
        (TassadarLinearMemoryWidth::I32, _) => {
            i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
        }
    }
}

fn encode_i32_to_bytes(value: i32, width: TassadarLinearMemoryWidth) -> Vec<u8> {
    let full = value.to_le_bytes();
    match width {
        TassadarLinearMemoryWidth::I8 => vec![full[0]],
        TassadarLinearMemoryWidth::I16 => vec![full[0], full[1]],
        TassadarLinearMemoryWidth::I32 => full.to_vec(),
    }
}

fn pop_stack_value(
    stack: &mut Vec<i32>,
    pc: usize,
) -> Result<i32, TassadarLinearMemoryExecutionError> {
    let available = stack.len();
    stack
        .pop()
        .ok_or(TassadarLinearMemoryExecutionError::StackUnderflow {
            pc,
            needed: 1,
            available,
        })
}

fn linear_memory_len_bytes(pages: u32) -> Result<usize, TassadarLinearMemoryExecutionError> {
    let bytes = u64::from(pages)
        .checked_mul(u64::from(TASSADAR_LINEAR_MEMORY_PAGE_BYTES))
        .ok_or(TassadarLinearMemoryExecutionError::ByteLengthOverflow)?;
    usize::try_from(bytes).map_err(|_| TassadarLinearMemoryExecutionError::ByteLengthOverflow)
}

fn current_memory_pages(memory_len: usize) -> Result<u32, TassadarLinearMemoryExecutionError> {
    let page_bytes = usize::try_from(TASSADAR_LINEAR_MEMORY_PAGE_BYTES)
        .map_err(|_| TassadarLinearMemoryExecutionError::ByteLengthOverflow)?;
    u32::try_from(memory_len / page_bytes)
        .map_err(|_| TassadarLinearMemoryExecutionError::ByteLengthOverflow)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[derive(Serialize)]
struct FullSnapshotTraceStep {
    step_index: usize,
    pc: usize,
    next_pc: usize,
    instruction: TassadarLinearMemoryInstruction,
    event: TassadarLinearMemoryTraceEvent,
    stack_before: Vec<i32>,
    stack_after: Vec<i32>,
    memory_after: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarLinearMemoryExecution, TassadarLinearMemoryExecutionError,
        TassadarLinearMemoryHaltReason, TassadarMemoryAbiContract,
        execute_tassadar_linear_memory_program, summarize_tassadar_linear_memory_trace_footprint,
        tassadar_seeded_linear_memory_copy_fill_program,
        tassadar_seeded_linear_memory_growth_program, tassadar_seeded_linear_memory_memcpy_program,
        tassadar_seeded_linear_memory_sign_extension_program,
        tassadar_seeded_linear_memory_width_parity_program,
    };

    fn execute(program: &super::TassadarLinearMemoryProgram) -> TassadarLinearMemoryExecution {
        execute_tassadar_linear_memory_program(program).expect("seeded program should execute")
    }

    #[test]
    fn linear_memory_abi_contracts_are_machine_legible() {
        let slot = TassadarMemoryAbiContract::slot_i32_v1();
        let linear = TassadarMemoryAbiContract::linear_memory_v2();

        assert_ne!(slot.compatibility_digest(), linear.compatibility_digest());
        assert_eq!(
            linear.page_bytes,
            Some(super::TASSADAR_LINEAR_MEMORY_PAGE_BYTES)
        );
        assert!(linear.supports_memory_grow);
        assert!(linear.supports_memory_copy);
        assert!(linear.supports_memory_fill);
    }

    #[test]
    fn linear_memory_abi_v2_load_store_widths_are_exact() {
        let execution = execute(&tassadar_seeded_linear_memory_width_parity_program());
        assert_eq!(execution.outputs, vec![120, 22_136, 305_419_896, 255]);
        assert_eq!(
            execution.halt_reason,
            TassadarLinearMemoryHaltReason::Returned
        );
        assert_eq!(execution.final_memory[0..4], [0x78, 0x56, 0x34, 0x12]);
        assert_eq!(execution.final_memory[4], 0xFF);
    }

    #[test]
    fn linear_memory_abi_v2_sign_extension_matches_wasm_style_behavior() {
        let execution = execute(&tassadar_seeded_linear_memory_sign_extension_program());
        assert_eq!(execution.outputs, vec![-128, 128, -128, 65_408]);
    }

    #[test]
    fn linear_memory_abi_v2_memory_size_and_grow_are_explicit() {
        let execution = execute(&tassadar_seeded_linear_memory_growth_program());
        assert_eq!(execution.outputs, vec![1, 1, 2, -1, 2]);
        assert_eq!(
            execution.final_memory.len(),
            2 * super::TASSADAR_LINEAR_MEMORY_PAGE_BYTES as usize
        );
        assert_eq!(
            execution
                .steps
                .iter()
                .filter(|step| step.memory_growth_delta.is_some())
                .count(),
            1
        );
    }

    #[test]
    fn linear_memory_abi_v2_memcpy_delta_trace_is_smaller_than_full_snapshots() {
        let program = tassadar_seeded_linear_memory_memcpy_program(64);
        let execution = execute(&program);
        let footprint =
            summarize_tassadar_linear_memory_trace_footprint(&execution).expect("footprint");

        assert_eq!(execution.outputs, vec![190]);
        assert_eq!(
            execution.final_memory[256..320],
            execution.final_memory[..64]
        );
        assert!(footprint.delta_trace_bytes < footprint.equivalent_full_snapshot_trace_bytes);
        assert_eq!(footprint.byte_delta_count, 64);
    }

    #[test]
    fn linear_memory_abi_v2_copy_and_fill_are_exact() {
        let execution = execute(&tassadar_seeded_linear_memory_copy_fill_program());
        assert_eq!(
            execution.outputs,
            vec![67_305_985, 134_678_021, 2_139_062_143]
        );
        assert_eq!(execution.final_memory[16..24], execution.final_memory[..8]);
        assert_eq!(execution.final_memory[32..36], [0x7F, 0x7F, 0x7F, 0x7F]);
        assert!(execution.steps.iter().any(|step| matches!(
            step.event,
            super::TassadarLinearMemoryTraceEvent::MemoryCopy { .. }
        )));
        assert!(execution.steps.iter().any(|step| matches!(
            step.event,
            super::TassadarLinearMemoryTraceEvent::MemoryFill { .. }
        )));
    }

    #[test]
    fn linear_memory_abi_v2_refuses_out_of_range_access() {
        let program = super::TassadarLinearMemoryProgram::new(
            "tassadar.memory_abi_v2.out_of_range.v1",
            1,
            1,
            vec![
                super::TassadarLinearMemoryInstruction::I32Load {
                    address: super::TASSADAR_LINEAR_MEMORY_PAGE_BYTES,
                    width: super::TassadarLinearMemoryWidth::I8,
                    signed: false,
                },
                super::TassadarLinearMemoryInstruction::Return,
            ],
        );
        let error = execute_tassadar_linear_memory_program(&program).expect_err("should refuse");
        assert!(matches!(
            error,
            TassadarLinearMemoryExecutionError::AddressOutOfRange { .. }
        ));
    }
}
