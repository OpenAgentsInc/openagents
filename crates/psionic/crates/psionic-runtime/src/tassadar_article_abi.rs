use psionic_ir::{
    TassadarArticleAbiFixture, TassadarArticleAbiHeapLayout, TassadarArticleAbiParamKind,
    TassadarArticleAbiResultKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TassadarStructuredControlBinaryOp;

const TASSADAR_ARTICLE_ABI_MAX_STEPS: usize = 16_384;

/// One instruction in the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "opcode", rename_all = "snake_case")]
pub enum TassadarArticleAbiInstruction {
    /// Push one immediate `i32`.
    I32Const { value: i32 },
    /// Read one local.
    LocalGet { local_index: u32 },
    /// Pop one value into one local.
    LocalSet { local_index: u32 },
    /// Pop two values and push one `i32` result.
    BinaryOp {
        op: TassadarStructuredControlBinaryOp,
    },
    /// Load one `i32` from the invocation-owned heap buffer.
    I32LoadHeapAtIndex {
        pointer_local_index: u32,
        index_local_index: u32,
        stride_bytes: u32,
    },
    /// Pop one condition and branch when it is zero.
    BranchIfZero { target_pc: usize },
    /// Unconditional jump.
    Jump { target_pc: usize },
    /// Return from the entrypoint.
    Return,
}

/// One validated runtime program for the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleAbiProgram {
    /// Stable program identifier.
    pub program_id: String,
    /// Stable fixture identifier.
    pub fixture_id: String,
    /// Exported entrypoint name.
    pub export_name: String,
    /// Declared parameter kinds in stable order.
    pub param_kinds: Vec<TassadarArticleAbiParamKind>,
    /// Optional single result kind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_kind: Option<TassadarArticleAbiResultKind>,
    /// Number of locals including parameters.
    pub local_count: usize,
    /// Optional heap-backed input layout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heap_layout: Option<TassadarArticleAbiHeapLayout>,
    /// Ordered instruction sequence.
    pub instructions: Vec<TassadarArticleAbiInstruction>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

impl TassadarArticleAbiProgram {
    /// Validates the bounded Rust-only article ABI surface.
    pub fn validate(&self) -> Result<(), TassadarArticleAbiError> {
        if self.instructions.is_empty() {
            return Err(TassadarArticleAbiError::NoInstructions);
        }
        if self.local_count < self.param_kinds.len() {
            return Err(TassadarArticleAbiError::LocalCountTooSmall {
                local_count: self.local_count,
                param_count: self.param_kinds.len(),
            });
        }
        for (param_index, kind) in self.param_kinds.iter().copied().enumerate() {
            match kind {
                TassadarArticleAbiParamKind::I32
                | TassadarArticleAbiParamKind::PointerToI32
                | TassadarArticleAbiParamKind::LengthI32 => {}
                unsupported => {
                    return Err(TassadarArticleAbiError::UnsupportedParamKind {
                        param_index: param_index as u8,
                        kind: unsupported,
                    });
                }
            }
        }
        if let Some(result_kind) = self.result_kind
            && result_kind != TassadarArticleAbiResultKind::I32
        {
            return Err(TassadarArticleAbiError::UnsupportedResultKind { kind: result_kind });
        }
        if let Some(heap_layout) = &self.heap_layout {
            validate_heap_layout(heap_layout, &self.param_kinds)?;
        }
        for (pc, instruction) in self.instructions.iter().enumerate() {
            match instruction {
                TassadarArticleAbiInstruction::LocalGet { local_index }
                | TassadarArticleAbiInstruction::LocalSet { local_index }
                    if *local_index as usize >= self.local_count =>
                {
                    return Err(TassadarArticleAbiError::LocalOutOfRange {
                        local_index: *local_index,
                        local_count: self.local_count,
                    });
                }
                TassadarArticleAbiInstruction::I32LoadHeapAtIndex {
                    pointer_local_index,
                    index_local_index,
                    ..
                } => {
                    for local_index in [pointer_local_index, index_local_index] {
                        if *local_index as usize >= self.local_count {
                            return Err(TassadarArticleAbiError::LocalOutOfRange {
                                local_index: *local_index,
                                local_count: self.local_count,
                            });
                        }
                    }
                    if self.heap_layout.is_none() {
                        return Err(TassadarArticleAbiError::HeapLayoutRequired);
                    }
                }
                TassadarArticleAbiInstruction::BranchIfZero { target_pc }
                | TassadarArticleAbiInstruction::Jump { target_pc }
                    if *target_pc >= self.instructions.len() =>
                {
                    return Err(TassadarArticleAbiError::InvalidBranchTarget {
                        pc,
                        target_pc: *target_pc,
                        instruction_count: self.instructions.len(),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// One invocation over the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleAbiInvocation {
    /// Scalar arguments in stable order.
    pub args: Vec<i32>,
    /// Invocation-owned heap bytes for pointer/length fixtures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heap_bytes: Vec<u8>,
}

impl TassadarArticleAbiInvocation {
    /// Creates one invocation with the supplied arguments.
    #[must_use]
    pub fn new(args: Vec<i32>) -> Self {
        Self {
            args,
            heap_bytes: Vec::new(),
        }
    }

    /// Carries one invocation-owned heap image.
    #[must_use]
    pub fn with_heap_bytes(mut self, heap_bytes: Vec<u8>) -> Self {
        self.heap_bytes = heap_bytes;
        self
    }
}

/// One trace event in the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarArticleAbiTraceEvent {
    ConstPush {
        value: i32,
    },
    LocalGet {
        local_index: u32,
        value: i32,
    },
    LocalSet {
        local_index: u32,
        value: i32,
    },
    BinaryOp {
        op: TassadarStructuredControlBinaryOp,
        left: i32,
        right: i32,
        result: i32,
    },
    HeapLoad {
        address: u32,
        value: i32,
    },
    BranchIfZero {
        condition: i32,
        target_pc: usize,
        taken: bool,
    },
    Jump {
        target_pc: usize,
    },
    Return {
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<i32>,
    },
}

/// One append-only trace step in the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleAbiTraceStep {
    pub step_index: usize,
    pub pc_before: usize,
    pub event: TassadarArticleAbiTraceEvent,
    pub locals_after: Vec<i32>,
    pub operand_stack_after: Vec<i32>,
}

/// One complete execution result in the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleAbiExecution {
    pub program_id: String,
    pub fixture_id: String,
    pub export_name: String,
    pub invocation_arg_digest: String,
    pub heap_digest: String,
    pub steps: Vec<TassadarArticleAbiTraceStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
}

impl TassadarArticleAbiExecution {
    /// Returns one stable digest over the visible execution truth.
    #[must_use]
    pub fn execution_digest(&self) -> String {
        stable_digest(b"tassadar_article_abi_execution|", self)
    }
}

/// Validation or execution failure for the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error_kind")]
pub enum TassadarArticleAbiError {
    #[error("article ABI program declares no instructions")]
    NoInstructions,
    #[error(
        "article ABI program local_count {local_count} is smaller than param_count {param_count}"
    )]
    LocalCountTooSmall {
        local_count: usize,
        param_count: usize,
    },
    #[error("article ABI param {param_index} declares unsupported kind `{kind:?}`")]
    UnsupportedParamKind {
        param_index: u8,
        kind: TassadarArticleAbiParamKind,
    },
    #[error("article ABI result declares unsupported kind `{kind:?}`")]
    UnsupportedResultKind { kind: TassadarArticleAbiResultKind },
    #[error("article ABI heap layout is required for heap load instructions")]
    HeapLayoutRequired,
    #[error(
        "article ABI heap layout pointer_param_index {pointer_param_index} is out of range for {param_count} params"
    )]
    HeapPointerParamOutOfRange {
        pointer_param_index: u8,
        param_count: usize,
    },
    #[error(
        "article ABI heap layout length_param_index {length_param_index} is out of range for {param_count} params"
    )]
    HeapLengthParamOutOfRange {
        length_param_index: u8,
        param_count: usize,
    },
    #[error("article ABI heap layout pointer param {pointer_param_index} must be pointer_to_i32")]
    HeapPointerParamKindMismatch { pointer_param_index: u8 },
    #[error("article ABI heap layout length param {length_param_index} must be length_i32")]
    HeapLengthParamKindMismatch { length_param_index: u8 },
    #[error("article ABI heap layout element_width_bytes {element_width_bytes} is unsupported")]
    UnsupportedHeapElementWidth { element_width_bytes: u8 },
    #[error("article ABI local {local_index} is out of range (local_count={local_count})")]
    LocalOutOfRange {
        local_index: u32,
        local_count: usize,
    },
    #[error(
        "article ABI instruction at pc {pc} branches to invalid target {target_pc} (instruction_count={instruction_count})"
    )]
    InvalidBranchTarget {
        pc: usize,
        target_pc: usize,
        instruction_count: usize,
    },
    #[error("article ABI invocation arg count mismatch: expected {expected}, actual {actual}")]
    InvocationArgCountMismatch { expected: usize, actual: usize },
    #[error("article ABI heap-backed invocation is missing heap bytes")]
    MissingHeapBytes,
    #[error("article ABI pointer arg {pointer} must be non-negative")]
    NegativePointer { pointer: i32 },
    #[error("article ABI length arg {length} must be non-negative")]
    NegativeLength { length: i32 },
    #[error("article ABI pointer {pointer} is not aligned to {required_alignment} bytes")]
    UnalignedPointer {
        pointer: i32,
        required_alignment: u8,
    },
    #[error(
        "article ABI heap input overflows the invocation heap: pointer={pointer} length={length} element_width_bytes={element_width_bytes} heap_len={heap_len}"
    )]
    HeapInputOutOfRange {
        pointer: i32,
        length: i32,
        element_width_bytes: u8,
        heap_len: usize,
    },
    #[error(
        "article ABI stack underflow at pc {pc} for {context}: needed {needed}, available {available}"
    )]
    StackUnderflow {
        pc: usize,
        context: String,
        needed: usize,
        available: usize,
    },
    #[error(
        "article ABI heap load at address {address} with width {width_bytes} exceeded heap_len {heap_len}"
    )]
    HeapLoadOutOfRange {
        address: u32,
        width_bytes: u32,
        heap_len: usize,
    },
    #[error("article ABI execution exceeded the step limit of {max_steps}")]
    StepLimitExceeded { max_steps: usize },
}

/// Executes one validated bounded Rust-only article ABI program.
pub fn execute_tassadar_article_abi_program(
    program: &TassadarArticleAbiProgram,
    invocation: &TassadarArticleAbiInvocation,
) -> Result<TassadarArticleAbiExecution, TassadarArticleAbiError> {
    program.validate()?;
    validate_invocation(program, invocation)?;

    let mut locals = vec![0; program.local_count];
    for (index, value) in invocation.args.iter().copied().enumerate() {
        locals[index] = value;
    }
    let mut operand_stack = Vec::new();
    let mut steps = Vec::new();
    let mut pc = 0usize;
    let mut step_index = 0usize;

    loop {
        if step_index >= TASSADAR_ARTICLE_ABI_MAX_STEPS {
            return Err(TassadarArticleAbiError::StepLimitExceeded {
                max_steps: TASSADAR_ARTICLE_ABI_MAX_STEPS,
            });
        }
        let instruction = program
            .instructions
            .get(pc)
            .ok_or(TassadarArticleAbiError::InvalidBranchTarget {
                pc,
                target_pc: pc,
                instruction_count: program.instructions.len(),
            })?
            .clone();
        let pc_before = pc;
        let event = match instruction {
            TassadarArticleAbiInstruction::I32Const { value } => {
                operand_stack.push(value);
                pc += 1;
                TassadarArticleAbiTraceEvent::ConstPush { value }
            }
            TassadarArticleAbiInstruction::LocalGet { local_index } => {
                let value = *locals.get(local_index as usize).ok_or(
                    TassadarArticleAbiError::LocalOutOfRange {
                        local_index,
                        local_count: locals.len(),
                    },
                )?;
                operand_stack.push(value);
                pc += 1;
                TassadarArticleAbiTraceEvent::LocalGet { local_index, value }
            }
            TassadarArticleAbiInstruction::LocalSet { local_index } => {
                let value = pop_operand(&mut operand_stack, pc, "local.set")?;
                let local_count = locals.len();
                *locals.get_mut(local_index as usize).ok_or(
                    TassadarArticleAbiError::LocalOutOfRange {
                        local_index,
                        local_count,
                    },
                )? = value;
                pc += 1;
                TassadarArticleAbiTraceEvent::LocalSet { local_index, value }
            }
            TassadarArticleAbiInstruction::BinaryOp { op } => {
                let right = pop_operand(&mut operand_stack, pc, "binary_op")?;
                let left = pop_operand(&mut operand_stack, pc, "binary_op")?;
                let result = execute_binary_op(op, left, right);
                operand_stack.push(result);
                pc += 1;
                TassadarArticleAbiTraceEvent::BinaryOp {
                    op,
                    left,
                    right,
                    result,
                }
            }
            TassadarArticleAbiInstruction::I32LoadHeapAtIndex {
                pointer_local_index,
                index_local_index,
                stride_bytes,
            } => {
                let base_pointer = *locals.get(pointer_local_index as usize).ok_or(
                    TassadarArticleAbiError::LocalOutOfRange {
                        local_index: pointer_local_index,
                        local_count: locals.len(),
                    },
                )?;
                let element_index = *locals.get(index_local_index as usize).ok_or(
                    TassadarArticleAbiError::LocalOutOfRange {
                        local_index: index_local_index,
                        local_count: locals.len(),
                    },
                )?;
                let address = heap_address(base_pointer, element_index, stride_bytes)?;
                let value = load_i32(&invocation.heap_bytes, address)?;
                operand_stack.push(value);
                pc += 1;
                TassadarArticleAbiTraceEvent::HeapLoad { address, value }
            }
            TassadarArticleAbiInstruction::BranchIfZero { target_pc } => {
                let condition = pop_operand(&mut operand_stack, pc, "branch_if_zero")?;
                let taken = condition == 0;
                if taken {
                    pc = target_pc;
                } else {
                    pc += 1;
                }
                TassadarArticleAbiTraceEvent::BranchIfZero {
                    condition,
                    target_pc,
                    taken,
                }
            }
            TassadarArticleAbiInstruction::Jump { target_pc } => {
                pc = target_pc;
                TassadarArticleAbiTraceEvent::Jump { target_pc }
            }
            TassadarArticleAbiInstruction::Return => {
                let value = match program.result_kind {
                    Some(TassadarArticleAbiResultKind::I32) => {
                        Some(pop_operand(&mut operand_stack, pc, "return_value")?)
                    }
                    Some(other) => {
                        return Err(TassadarArticleAbiError::UnsupportedResultKind { kind: other });
                    }
                    None => None,
                };
                steps.push(TassadarArticleAbiTraceStep {
                    step_index,
                    pc_before,
                    event: TassadarArticleAbiTraceEvent::Return { value },
                    locals_after: locals.clone(),
                    operand_stack_after: operand_stack.clone(),
                });
                return Ok(TassadarArticleAbiExecution {
                    program_id: program.program_id.clone(),
                    fixture_id: program.fixture_id.clone(),
                    export_name: program.export_name.clone(),
                    invocation_arg_digest: stable_digest(
                        b"tassadar_article_abi_invocation_args|",
                        &invocation.args,
                    ),
                    heap_digest: stable_digest(
                        b"tassadar_article_abi_heap_bytes|",
                        &invocation.heap_bytes,
                    ),
                    steps,
                    returned_value: value,
                });
            }
        };
        steps.push(TassadarArticleAbiTraceStep {
            step_index,
            pc_before,
            event,
            locals_after: locals.clone(),
            operand_stack_after: operand_stack.clone(),
        });
        step_index = step_index.saturating_add(1);
    }
}

fn validate_heap_layout(
    heap_layout: &TassadarArticleAbiHeapLayout,
    param_kinds: &[TassadarArticleAbiParamKind],
) -> Result<(), TassadarArticleAbiError> {
    if heap_layout.pointer_param_index as usize >= param_kinds.len() {
        return Err(TassadarArticleAbiError::HeapPointerParamOutOfRange {
            pointer_param_index: heap_layout.pointer_param_index,
            param_count: param_kinds.len(),
        });
    }
    if heap_layout.length_param_index as usize >= param_kinds.len() {
        return Err(TassadarArticleAbiError::HeapLengthParamOutOfRange {
            length_param_index: heap_layout.length_param_index,
            param_count: param_kinds.len(),
        });
    }
    if param_kinds[heap_layout.pointer_param_index as usize]
        != TassadarArticleAbiParamKind::PointerToI32
    {
        return Err(TassadarArticleAbiError::HeapPointerParamKindMismatch {
            pointer_param_index: heap_layout.pointer_param_index,
        });
    }
    if param_kinds[heap_layout.length_param_index as usize]
        != TassadarArticleAbiParamKind::LengthI32
    {
        return Err(TassadarArticleAbiError::HeapLengthParamKindMismatch {
            length_param_index: heap_layout.length_param_index,
        });
    }
    if heap_layout.element_width_bytes != 4 {
        return Err(TassadarArticleAbiError::UnsupportedHeapElementWidth {
            element_width_bytes: heap_layout.element_width_bytes,
        });
    }
    Ok(())
}

fn validate_invocation(
    program: &TassadarArticleAbiProgram,
    invocation: &TassadarArticleAbiInvocation,
) -> Result<(), TassadarArticleAbiError> {
    if invocation.args.len() != program.param_kinds.len() {
        return Err(TassadarArticleAbiError::InvocationArgCountMismatch {
            expected: program.param_kinds.len(),
            actual: invocation.args.len(),
        });
    }
    if let Some(heap_layout) = &program.heap_layout {
        if invocation.heap_bytes.is_empty() {
            return Err(TassadarArticleAbiError::MissingHeapBytes);
        }
        let pointer = invocation.args[heap_layout.pointer_param_index as usize];
        let length = invocation.args[heap_layout.length_param_index as usize];
        if pointer < 0 {
            return Err(TassadarArticleAbiError::NegativePointer { pointer });
        }
        if length < 0 {
            return Err(TassadarArticleAbiError::NegativeLength { length });
        }
        let required_alignment = heap_layout.element_width_bytes;
        if pointer % i32::from(required_alignment) != 0 {
            return Err(TassadarArticleAbiError::UnalignedPointer {
                pointer,
                required_alignment,
            });
        }
        let needed_bytes = usize::try_from(pointer).unwrap_or_default()
            + usize::try_from(length).unwrap_or_default()
                * usize::from(heap_layout.element_width_bytes);
        if needed_bytes > invocation.heap_bytes.len() {
            return Err(TassadarArticleAbiError::HeapInputOutOfRange {
                pointer,
                length,
                element_width_bytes: heap_layout.element_width_bytes,
                heap_len: invocation.heap_bytes.len(),
            });
        }
    }
    Ok(())
}

fn pop_operand(
    operand_stack: &mut Vec<i32>,
    pc: usize,
    context: &str,
) -> Result<i32, TassadarArticleAbiError> {
    let available = operand_stack.len();
    operand_stack
        .pop()
        .ok_or_else(|| TassadarArticleAbiError::StackUnderflow {
            pc,
            context: String::from(context),
            needed: 1,
            available,
        })
}

fn heap_address(
    base_pointer: i32,
    element_index: i32,
    stride_bytes: u32,
) -> Result<u32, TassadarArticleAbiError> {
    let base_pointer =
        u32::try_from(base_pointer).map_err(|_| TassadarArticleAbiError::NegativePointer {
            pointer: base_pointer,
        })?;
    let element_index =
        u32::try_from(element_index).map_err(|_| TassadarArticleAbiError::NegativeLength {
            length: element_index,
        })?;
    base_pointer
        .checked_add(element_index.saturating_mul(stride_bytes))
        .ok_or(TassadarArticleAbiError::HeapLoadOutOfRange {
            address: u32::MAX,
            width_bytes: stride_bytes,
            heap_len: 0,
        })
}

fn load_i32(heap_bytes: &[u8], address: u32) -> Result<i32, TassadarArticleAbiError> {
    let address = usize::try_from(address).unwrap_or(usize::MAX);
    let end = address.saturating_add(4);
    if end > heap_bytes.len() {
        return Err(TassadarArticleAbiError::HeapLoadOutOfRange {
            address: address as u32,
            width_bytes: 4,
            heap_len: heap_bytes.len(),
        });
    }
    Ok(i32::from_le_bytes(
        heap_bytes[address..end].try_into().expect("length checked"),
    ))
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

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

/// Returns one canonical direct scalar-parameter invocation.
#[must_use]
pub fn tassadar_article_abi_scalar_invocation() -> TassadarArticleAbiInvocation {
    TassadarArticleAbiInvocation::new(vec![41])
}

/// Returns one canonical heap-backed invocation for the `heap_sum_i32` fixture.
#[must_use]
pub fn tassadar_article_abi_heap_sum_invocation() -> TassadarArticleAbiInvocation {
    TassadarArticleAbiInvocation::new(vec![0, 4]).with_heap_bytes(
        [5_i32, -2_i32, 10_i32, 7_i32]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect(),
    )
}

/// Returns one canonical offset-pointer invocation for the `heap_sum_i32` fixture.
#[must_use]
pub fn tassadar_article_abi_heap_sum_offset_invocation() -> TassadarArticleAbiInvocation {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&11_i32.to_le_bytes());
    bytes.extend_from_slice(&7_i32.to_le_bytes());
    bytes.extend_from_slice(&8_i32.to_le_bytes());
    bytes.extend_from_slice(&9_i32.to_le_bytes());
    TassadarArticleAbiInvocation::new(vec![4, 3]).with_heap_bytes(bytes)
}

/// Returns one runtime refusal invocation for the `heap_sum_i32` fixture.
#[must_use]
pub fn tassadar_article_abi_heap_sum_out_of_range_invocation() -> TassadarArticleAbiInvocation {
    TassadarArticleAbiInvocation::new(vec![0, 5]).with_heap_bytes(
        [1_i32, 2_i32, 3_i32, 4_i32]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect(),
    )
}

/// Returns one stable program id derived from one canonical fixture.
#[must_use]
pub fn tassadar_article_abi_program_id(fixture: &TassadarArticleAbiFixture) -> String {
    format!("tassadar.article_abi.{}.v1", fixture.fixture_id.as_str())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarArticleAbiError, TassadarArticleAbiInstruction, TassadarArticleAbiProgram,
        execute_tassadar_article_abi_program, tassadar_article_abi_heap_sum_invocation,
        tassadar_article_abi_heap_sum_offset_invocation,
        tassadar_article_abi_heap_sum_out_of_range_invocation, tassadar_article_abi_program_id,
        tassadar_article_abi_scalar_invocation,
    };
    use psionic_ir::TassadarArticleAbiFixture;

    #[test]
    fn article_abi_scalar_param_and_return_are_exact() {
        let fixture = TassadarArticleAbiFixture::scalar_add_one();
        let program = TassadarArticleAbiProgram {
            program_id: tassadar_article_abi_program_id(&fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            export_name: fixture.export_name.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kind: fixture.result_kinds.first().copied(),
            local_count: 1,
            heap_layout: None,
            instructions: vec![
                TassadarArticleAbiInstruction::LocalGet { local_index: 0 },
                TassadarArticleAbiInstruction::I32Const { value: 1 },
                TassadarArticleAbiInstruction::BinaryOp {
                    op: crate::TassadarStructuredControlBinaryOp::Add,
                },
                TassadarArticleAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary,
        };

        let execution = execute_tassadar_article_abi_program(
            &program,
            &tassadar_article_abi_scalar_invocation(),
        )
        .expect("execute");
        assert_eq!(execution.returned_value, Some(42));
    }

    #[test]
    fn article_abi_heap_input_and_return_are_exact() {
        let fixture = TassadarArticleAbiFixture::heap_sum_i32();
        let program = TassadarArticleAbiProgram {
            program_id: tassadar_article_abi_program_id(&fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            export_name: fixture.export_name.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kind: fixture.result_kinds.first().copied(),
            local_count: 4,
            heap_layout: fixture.heap_layout.clone(),
            instructions: vec![
                TassadarArticleAbiInstruction::I32Const { value: 0 },
                TassadarArticleAbiInstruction::LocalSet { local_index: 2 },
                TassadarArticleAbiInstruction::I32Const { value: 0 },
                TassadarArticleAbiInstruction::LocalSet { local_index: 3 },
                TassadarArticleAbiInstruction::LocalGet { local_index: 3 },
                TassadarArticleAbiInstruction::LocalGet { local_index: 1 },
                TassadarArticleAbiInstruction::BinaryOp {
                    op: crate::TassadarStructuredControlBinaryOp::LtS,
                },
                TassadarArticleAbiInstruction::BranchIfZero { target_pc: 17 },
                TassadarArticleAbiInstruction::LocalGet { local_index: 2 },
                TassadarArticleAbiInstruction::I32LoadHeapAtIndex {
                    pointer_local_index: 0,
                    index_local_index: 3,
                    stride_bytes: 4,
                },
                TassadarArticleAbiInstruction::BinaryOp {
                    op: crate::TassadarStructuredControlBinaryOp::Add,
                },
                TassadarArticleAbiInstruction::LocalSet { local_index: 2 },
                TassadarArticleAbiInstruction::LocalGet { local_index: 3 },
                TassadarArticleAbiInstruction::I32Const { value: 1 },
                TassadarArticleAbiInstruction::BinaryOp {
                    op: crate::TassadarStructuredControlBinaryOp::Add,
                },
                TassadarArticleAbiInstruction::LocalSet { local_index: 3 },
                TassadarArticleAbiInstruction::Jump { target_pc: 4 },
                TassadarArticleAbiInstruction::LocalGet { local_index: 2 },
                TassadarArticleAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary,
        };

        let execution = execute_tassadar_article_abi_program(
            &program,
            &tassadar_article_abi_heap_sum_invocation(),
        )
        .expect("execute");
        assert_eq!(execution.returned_value, Some(20));

        let offset_execution = execute_tassadar_article_abi_program(
            &program,
            &tassadar_article_abi_heap_sum_offset_invocation(),
        )
        .expect("execute");
        assert_eq!(offset_execution.returned_value, Some(24));
    }

    #[test]
    fn article_abi_out_of_range_heap_input_refuses_explicitly() {
        let fixture = TassadarArticleAbiFixture::heap_sum_i32();
        let program = TassadarArticleAbiProgram {
            program_id: tassadar_article_abi_program_id(&fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            export_name: fixture.export_name.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kind: fixture.result_kinds.first().copied(),
            local_count: 4,
            heap_layout: fixture.heap_layout.clone(),
            instructions: vec![
                TassadarArticleAbiInstruction::LocalGet { local_index: 0 },
                TassadarArticleAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary,
        };

        let error = execute_tassadar_article_abi_program(
            &program,
            &tassadar_article_abi_heap_sum_out_of_range_invocation(),
        )
        .expect_err("heap input should refuse");
        assert!(matches!(
            error,
            TassadarArticleAbiError::HeapInputOutOfRange { .. }
        ));
    }
}
