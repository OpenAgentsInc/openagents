use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_STRUCTURED_CONTROL_MAX_STEPS: usize = 4_096;

/// Arithmetic family supported by the structured-control lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStructuredControlBinaryOp {
    /// Integer addition.
    Add,
    /// Integer subtraction.
    Sub,
    /// Integer multiplication.
    Mul,
    /// Equality comparison.
    Eq,
    /// Inequality comparison.
    Ne,
    /// Signed less-than comparison.
    LtS,
    /// Unsigned less-than comparison.
    LtU,
    /// Signed greater-than comparison.
    GtS,
    /// Unsigned greater-than comparison.
    GtU,
    /// Signed less-than-or-equal comparison.
    LeS,
    /// Unsigned less-than-or-equal comparison.
    LeU,
    /// Signed greater-than-or-equal comparison.
    GeS,
    /// Unsigned greater-than-or-equal comparison.
    GeU,
    /// Bitwise and.
    And,
    /// Bitwise or.
    Or,
    /// Bitwise xor.
    Xor,
    /// Left shift.
    Shl,
    /// Arithmetic right shift.
    ShrS,
    /// Logical right shift.
    ShrU,
}

/// Unary integer operation supported by the structured-control lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStructuredControlUnaryOp {
    /// Integer equals-zero check.
    Eqz,
}

/// One executor-ready structured-control instruction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "opcode", rename_all = "snake_case")]
pub enum TassadarStructuredControlInstruction {
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
    /// Pop one stack value into one local.
    LocalSet {
        /// Local index.
        local_index: u32,
    },
    /// Copy one stack-top value into one local without consuming it.
    LocalTee {
        /// Local index.
        local_index: u32,
    },
    /// Apply one unary integer operation.
    UnaryOp {
        /// Unary operation family.
        op: TassadarStructuredControlUnaryOp,
    },
    /// Pop two `i32` values and push one arithmetic result.
    BinaryOp {
        /// Arithmetic family.
        op: TassadarStructuredControlBinaryOp,
    },
    /// Drop one stack value.
    Drop,
    /// Structured `block`.
    Block {
        /// Stable compiler-generated label identifier.
        label_id: String,
        /// Nested instructions inside the block.
        instructions: Vec<TassadarStructuredControlInstruction>,
    },
    /// Structured `loop`.
    Loop {
        /// Stable compiler-generated label identifier.
        label_id: String,
        /// Nested instructions inside the loop body.
        instructions: Vec<TassadarStructuredControlInstruction>,
    },
    /// Structured `if` / `else`.
    If {
        /// Stable compiler-generated label identifier.
        label_id: String,
        /// Then branch instructions.
        then_instructions: Vec<TassadarStructuredControlInstruction>,
        /// Else branch instructions.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        else_instructions: Vec<TassadarStructuredControlInstruction>,
    },
    /// Unconditional branch to one enclosing structured label.
    Br {
        /// Relative depth to target.
        depth: u32,
    },
    /// Conditional branch to one enclosing structured label.
    BrIf {
        /// Relative depth to target.
        depth: u32,
    },
    /// Indexed branch to one enclosing structured label.
    BrTable {
        /// Branch targets for in-range selector values.
        target_depths: Vec<u32>,
        /// Default branch target for out-of-range selectors.
        default_depth: u32,
    },
    /// Explicit function return.
    Return,
}

/// One validated structured-control program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredControlProgram {
    /// Stable program identifier.
    pub program_id: String,
    /// Number of function locals.
    pub local_count: usize,
    /// Function result count. Only `0` or `1` are supported today.
    pub result_count: u8,
    /// Ordered structured instruction sequence.
    pub instructions: Vec<TassadarStructuredControlInstruction>,
}

impl TassadarStructuredControlProgram {
    /// Creates one structured-control program.
    #[must_use]
    pub fn new(
        program_id: impl Into<String>,
        local_count: usize,
        result_count: u8,
        instructions: Vec<TassadarStructuredControlInstruction>,
    ) -> Self {
        Self {
            program_id: program_id.into(),
            local_count,
            result_count,
            instructions,
        }
    }

    /// Validates the public structured-control surface.
    pub fn validate(&self) -> Result<(), TassadarStructuredControlError> {
        if self.result_count > 1 {
            return Err(TassadarStructuredControlError::UnsupportedResultCount {
                result_count: self.result_count,
            });
        }
        validate_instruction_list(self.instructions.as_slice(), 0)
    }
}

/// One trace event emitted by the structured-control executor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarStructuredControlTraceEvent {
    /// One constant was pushed.
    ConstPush {
        /// Value pushed to the stack.
        value: i32,
    },
    /// One local was read.
    LocalGet {
        /// Local index.
        local_index: u32,
        /// Value pushed from the local.
        value: i32,
    },
    /// One local was written.
    LocalSet {
        /// Local index.
        local_index: u32,
        /// Value written.
        value: i32,
    },
    /// One local was tee'd from the stack.
    LocalTee {
        /// Local index.
        local_index: u32,
        /// Value copied into the local.
        value: i32,
    },
    /// One unary integer operation completed.
    UnaryOp {
        /// Unary operation family.
        op: TassadarStructuredControlUnaryOp,
        /// Input value.
        input: i32,
        /// Result value.
        result: i32,
    },
    /// One arithmetic operation completed.
    BinaryOp {
        /// Arithmetic family.
        op: TassadarStructuredControlBinaryOp,
        /// Left operand.
        left: i32,
        /// Right operand.
        right: i32,
        /// Result value.
        result: i32,
    },
    /// One value was dropped.
    Drop {
        /// Value removed from the stack.
        value: i32,
    },
    /// One `block` scope was entered.
    EnterBlock {
        /// Label identifier.
        label_id: String,
    },
    /// One `block` scope exited normally or via a consumed branch.
    ExitBlock {
        /// Label identifier.
        label_id: String,
    },
    /// One `loop` iteration started.
    EnterLoop {
        /// Label identifier.
        label_id: String,
        /// Zero-based iteration count.
        iteration: u32,
    },
    /// One loop back-edge was taken.
    LoopContinue {
        /// Label identifier.
        label_id: String,
        /// Iteration count before the continue.
        iteration: u32,
    },
    /// One `loop` exited.
    ExitLoop {
        /// Label identifier.
        label_id: String,
        /// Final iteration count for the exiting pass.
        iteration: u32,
    },
    /// One `if` condition selected a branch.
    IfDecision {
        /// Label identifier.
        label_id: String,
        /// Condition value popped from the stack.
        condition: i32,
        /// Whether the `then` branch was taken.
        took_then: bool,
    },
    /// One unconditional branch was issued.
    Branch {
        /// Relative depth requested by the instruction.
        depth: u32,
    },
    /// One conditional branch was evaluated.
    BranchIf {
        /// Relative depth requested by the instruction.
        depth: u32,
        /// Condition value popped from the stack.
        condition: i32,
        /// Whether the branch was taken.
        taken: bool,
    },
    /// One `br_table` branch was evaluated.
    BranchTable {
        /// Selector value popped from the stack.
        selector: i32,
        /// Chosen target depth after bounds resolution.
        chosen_depth: u32,
    },
    /// Execution returned.
    Return {
        /// Optional returned `i32` value.
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<i32>,
    },
}

/// One append-only structured-control trace step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredControlTraceStep {
    /// Step index in execution order.
    pub step_index: usize,
    /// Current structured-control nesting depth.
    pub control_depth: u32,
    /// Event emitted by the step.
    pub event: TassadarStructuredControlTraceEvent,
    /// Stack snapshot after the step.
    pub stack_after: Vec<i32>,
    /// Local snapshot after the step.
    pub locals_after: Vec<i32>,
}

/// Terminal reason for one structured-control execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStructuredControlHaltReason {
    /// The program executed `return`.
    Returned,
    /// The program finished the instruction stream without an explicit `return`.
    FellOffEnd,
}

/// One complete execution result for the structured-control lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuredControlExecution {
    /// Stable program identifier.
    pub program_id: String,
    /// Ordered append-only trace steps.
    pub steps: Vec<TassadarStructuredControlTraceStep>,
    /// Optional returned value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
    /// Final local snapshot.
    pub final_locals: Vec<i32>,
    /// Final stack snapshot.
    pub final_stack: Vec<i32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarStructuredControlHaltReason,
}

impl TassadarStructuredControlExecution {
    /// Returns a stable digest over the visible execution truth.
    #[must_use]
    pub fn execution_digest(&self) -> String {
        stable_digest(b"tassadar_structured_control_execution|", self)
    }
}

/// Typed validation or execution failure for the structured-control lane.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TassadarStructuredControlError {
    /// The function result arity is outside the bounded `0` or `1` lane.
    #[error(
        "unsupported structured-control result count {result_count}; only 0 or 1 are supported"
    )]
    UnsupportedResultCount {
        /// Declared result count.
        result_count: u8,
    },
    /// One branch targeted a missing enclosing label.
    #[error(
        "structured-control branch depth {depth} is out of range for nesting depth {available_depth}"
    )]
    InvalidBranchDepth {
        /// Requested branch depth.
        depth: u32,
        /// Available enclosing control depth.
        available_depth: u32,
    },
    /// One local index exceeded the declared local count.
    #[error("structured-control local {local_index} is out of range (local_count={local_count})")]
    LocalOutOfRange {
        /// Referenced local index.
        local_index: u32,
        /// Declared local count.
        local_count: usize,
    },
    /// One instruction needed more stack items than were available.
    #[error(
        "structured-control stack underflow in {context}: needed {needed}, available {available}"
    )]
    StackUnderflow {
        /// Human-readable context label.
        context: String,
        /// Values required.
        needed: usize,
        /// Values available.
        available: usize,
    },
    /// The execution exceeded the bounded step limit.
    #[error("structured-control execution exceeded the step limit of {max_steps}")]
    StepLimitExceeded {
        /// Maximum supported step count.
        max_steps: usize,
    },
    /// One branch escaped the function boundary after validation.
    #[error("structured-control branch depth {depth} escaped the function boundary")]
    BranchEscapedFunction {
        /// Residual branch depth at function boundary.
        depth: u32,
    },
}

/// Executes one validated structured-control program.
pub fn execute_tassadar_structured_control_program(
    program: &TassadarStructuredControlProgram,
) -> Result<TassadarStructuredControlExecution, TassadarStructuredControlError> {
    program.validate()?;
    let mut state = ExecutionState::new(program.local_count);
    let signal = execute_instruction_list(program.instructions.as_slice(), 0, program, &mut state)?;
    let (halt_reason, returned_value) = match signal {
        ControlSignal::Continue => (
            TassadarStructuredControlHaltReason::FellOffEnd,
            finalize_return_value(&mut state.stack, program.result_count)?,
        ),
        ControlSignal::Return(value) => (TassadarStructuredControlHaltReason::Returned, value),
        ControlSignal::Branch(depth) => {
            return Err(TassadarStructuredControlError::BranchEscapedFunction { depth });
        }
    };

    Ok(TassadarStructuredControlExecution {
        program_id: program.program_id.clone(),
        steps: state.steps,
        returned_value,
        final_locals: state.locals,
        final_stack: state.stack,
        halt_reason,
    })
}

fn validate_instruction_list(
    instructions: &[TassadarStructuredControlInstruction],
    available_depth: u32,
) -> Result<(), TassadarStructuredControlError> {
    for instruction in instructions {
        match instruction {
            TassadarStructuredControlInstruction::Block { instructions, .. }
            | TassadarStructuredControlInstruction::Loop { instructions, .. } => {
                validate_instruction_list(instructions.as_slice(), available_depth + 1)?;
            }
            TassadarStructuredControlInstruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                validate_instruction_list(then_instructions.as_slice(), available_depth + 1)?;
                validate_instruction_list(else_instructions.as_slice(), available_depth + 1)?;
            }
            TassadarStructuredControlInstruction::Br { depth }
            | TassadarStructuredControlInstruction::BrIf { depth }
                if *depth >= available_depth =>
            {
                return Err(TassadarStructuredControlError::InvalidBranchDepth {
                    depth: *depth,
                    available_depth,
                });
            }
            TassadarStructuredControlInstruction::BrTable {
                target_depths,
                default_depth,
            } => {
                for depth in target_depths.iter().copied().chain([*default_depth]) {
                    if depth >= available_depth {
                        return Err(TassadarStructuredControlError::InvalidBranchDepth {
                            depth,
                            available_depth,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

enum ControlSignal {
    Continue,
    Branch(u32),
    Return(Option<i32>),
}

struct ExecutionState {
    stack: Vec<i32>,
    locals: Vec<i32>,
    steps: Vec<TassadarStructuredControlTraceStep>,
    step_index: usize,
}

impl ExecutionState {
    fn new(local_count: usize) -> Self {
        Self {
            stack: Vec::new(),
            locals: vec![0; local_count],
            steps: Vec::new(),
            step_index: 0,
        }
    }

    fn push_step(
        &mut self,
        control_depth: u32,
        event: TassadarStructuredControlTraceEvent,
    ) -> Result<(), TassadarStructuredControlError> {
        if self.step_index >= TASSADAR_STRUCTURED_CONTROL_MAX_STEPS {
            return Err(TassadarStructuredControlError::StepLimitExceeded {
                max_steps: TASSADAR_STRUCTURED_CONTROL_MAX_STEPS,
            });
        }
        self.steps.push(TassadarStructuredControlTraceStep {
            step_index: self.step_index,
            control_depth,
            event,
            stack_after: self.stack.clone(),
            locals_after: self.locals.clone(),
        });
        self.step_index = self.step_index.saturating_add(1);
        Ok(())
    }
}

fn execute_instruction_list(
    instructions: &[TassadarStructuredControlInstruction],
    control_depth: u32,
    program: &TassadarStructuredControlProgram,
    state: &mut ExecutionState,
) -> Result<ControlSignal, TassadarStructuredControlError> {
    for instruction in instructions {
        match instruction {
            TassadarStructuredControlInstruction::I32Const { value } => {
                state.stack.push(*value);
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::ConstPush { value: *value },
                )?;
            }
            TassadarStructuredControlInstruction::LocalGet { local_index } => {
                let value = get_local(state, *local_index)?;
                state.stack.push(value);
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::LocalGet {
                        local_index: *local_index,
                        value,
                    },
                )?;
            }
            TassadarStructuredControlInstruction::LocalSet { local_index } => {
                let value = pop_stack_value(state, "local.set")?;
                *get_local_mut(state, *local_index)? = value;
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::LocalSet {
                        local_index: *local_index,
                        value,
                    },
                )?;
            }
            TassadarStructuredControlInstruction::LocalTee { local_index } => {
                let value = *state.stack.last().ok_or_else(|| {
                    TassadarStructuredControlError::StackUnderflow {
                        context: String::from("local.tee"),
                        needed: 1,
                        available: 0,
                    }
                })?;
                *get_local_mut(state, *local_index)? = value;
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::LocalTee {
                        local_index: *local_index,
                        value,
                    },
                )?;
            }
            TassadarStructuredControlInstruction::UnaryOp { op } => {
                let input = pop_stack_value(state, "unary_op")?;
                let result = match op {
                    TassadarStructuredControlUnaryOp::Eqz => i32::from(input == 0),
                };
                state.stack.push(result);
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::UnaryOp {
                        op: *op,
                        input,
                        result,
                    },
                )?;
            }
            TassadarStructuredControlInstruction::BinaryOp { op } => {
                let right = pop_stack_value(state, "binary_op")?;
                let left = pop_stack_value(state, "binary_op")?;
                let result = match op {
                    TassadarStructuredControlBinaryOp::Add => left.saturating_add(right),
                    TassadarStructuredControlBinaryOp::Sub => left.saturating_sub(right),
                    TassadarStructuredControlBinaryOp::Mul => left.saturating_mul(right),
                    TassadarStructuredControlBinaryOp::Eq => i32::from(left == right),
                    TassadarStructuredControlBinaryOp::Ne => i32::from(left != right),
                    TassadarStructuredControlBinaryOp::LtS => i32::from(left < right),
                    TassadarStructuredControlBinaryOp::LtU => {
                        i32::from((left as u32) < (right as u32))
                    }
                    TassadarStructuredControlBinaryOp::GtS => i32::from(left > right),
                    TassadarStructuredControlBinaryOp::GtU => {
                        i32::from((left as u32) > (right as u32))
                    }
                    TassadarStructuredControlBinaryOp::LeS => i32::from(left <= right),
                    TassadarStructuredControlBinaryOp::LeU => {
                        i32::from((left as u32) <= (right as u32))
                    }
                    TassadarStructuredControlBinaryOp::GeS => i32::from(left >= right),
                    TassadarStructuredControlBinaryOp::GeU => {
                        i32::from((left as u32) >= (right as u32))
                    }
                    TassadarStructuredControlBinaryOp::And => left & right,
                    TassadarStructuredControlBinaryOp::Or => left | right,
                    TassadarStructuredControlBinaryOp::Xor => left ^ right,
                    TassadarStructuredControlBinaryOp::Shl => left.wrapping_shl(right as u32),
                    TassadarStructuredControlBinaryOp::ShrS => left.wrapping_shr(right as u32),
                    TassadarStructuredControlBinaryOp::ShrU => {
                        ((left as u32).wrapping_shr(right as u32)) as i32
                    }
                };
                state.stack.push(result);
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::BinaryOp {
                        op: *op,
                        left,
                        right,
                        result,
                    },
                )?;
            }
            TassadarStructuredControlInstruction::Drop => {
                let value = pop_stack_value(state, "drop")?;
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::Drop { value },
                )?;
            }
            TassadarStructuredControlInstruction::Block {
                label_id,
                instructions,
            } => {
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::EnterBlock {
                        label_id: label_id.clone(),
                    },
                )?;
                let signal = execute_instruction_list(
                    instructions.as_slice(),
                    control_depth + 1,
                    program,
                    state,
                )?;
                match signal {
                    ControlSignal::Continue | ControlSignal::Branch(0) => {
                        state.push_step(
                            control_depth,
                            TassadarStructuredControlTraceEvent::ExitBlock {
                                label_id: label_id.clone(),
                            },
                        )?;
                    }
                    ControlSignal::Branch(depth) => {
                        state.push_step(
                            control_depth,
                            TassadarStructuredControlTraceEvent::ExitBlock {
                                label_id: label_id.clone(),
                            },
                        )?;
                        return Ok(ControlSignal::Branch(depth - 1));
                    }
                    ControlSignal::Return(value) => {
                        state.push_step(
                            control_depth,
                            TassadarStructuredControlTraceEvent::ExitBlock {
                                label_id: label_id.clone(),
                            },
                        )?;
                        return Ok(ControlSignal::Return(value));
                    }
                }
            }
            TassadarStructuredControlInstruction::Loop {
                label_id,
                instructions,
            } => {
                let mut iteration = 0u32;
                loop {
                    state.push_step(
                        control_depth,
                        TassadarStructuredControlTraceEvent::EnterLoop {
                            label_id: label_id.clone(),
                            iteration,
                        },
                    )?;
                    let signal = execute_instruction_list(
                        instructions.as_slice(),
                        control_depth + 1,
                        program,
                        state,
                    )?;
                    match signal {
                        ControlSignal::Continue => {
                            state.push_step(
                                control_depth,
                                TassadarStructuredControlTraceEvent::ExitLoop {
                                    label_id: label_id.clone(),
                                    iteration,
                                },
                            )?;
                            break;
                        }
                        ControlSignal::Branch(0) => {
                            state.push_step(
                                control_depth,
                                TassadarStructuredControlTraceEvent::LoopContinue {
                                    label_id: label_id.clone(),
                                    iteration,
                                },
                            )?;
                            iteration = iteration.saturating_add(1);
                        }
                        ControlSignal::Branch(depth) => {
                            state.push_step(
                                control_depth,
                                TassadarStructuredControlTraceEvent::ExitLoop {
                                    label_id: label_id.clone(),
                                    iteration,
                                },
                            )?;
                            return Ok(ControlSignal::Branch(depth - 1));
                        }
                        ControlSignal::Return(value) => {
                            state.push_step(
                                control_depth,
                                TassadarStructuredControlTraceEvent::ExitLoop {
                                    label_id: label_id.clone(),
                                    iteration,
                                },
                            )?;
                            return Ok(ControlSignal::Return(value));
                        }
                    }
                }
            }
            TassadarStructuredControlInstruction::If {
                label_id,
                then_instructions,
                else_instructions,
            } => {
                let condition = pop_stack_value(state, "if")?;
                let took_then = condition != 0;
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::IfDecision {
                        label_id: label_id.clone(),
                        condition,
                        took_then,
                    },
                )?;
                let selected = if took_then {
                    then_instructions.as_slice()
                } else {
                    else_instructions.as_slice()
                };
                let signal = execute_instruction_list(selected, control_depth + 1, program, state)?;
                match signal {
                    ControlSignal::Continue | ControlSignal::Branch(0) => {}
                    ControlSignal::Branch(depth) => return Ok(ControlSignal::Branch(depth - 1)),
                    ControlSignal::Return(value) => return Ok(ControlSignal::Return(value)),
                }
            }
            TassadarStructuredControlInstruction::Br { depth } => {
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::Branch { depth: *depth },
                )?;
                return Ok(ControlSignal::Branch(*depth));
            }
            TassadarStructuredControlInstruction::BrIf { depth } => {
                let condition = pop_stack_value(state, "br_if")?;
                let taken = condition != 0;
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::BranchIf {
                        depth: *depth,
                        condition,
                        taken,
                    },
                )?;
                if taken {
                    return Ok(ControlSignal::Branch(*depth));
                }
            }
            TassadarStructuredControlInstruction::BrTable {
                target_depths,
                default_depth,
            } => {
                let selector = pop_stack_value(state, "br_table")?;
                let chosen_depth = if selector >= 0 {
                    target_depths
                        .get(selector as usize)
                        .copied()
                        .unwrap_or(*default_depth)
                } else {
                    *default_depth
                };
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::BranchTable {
                        selector,
                        chosen_depth,
                    },
                )?;
                return Ok(ControlSignal::Branch(chosen_depth));
            }
            TassadarStructuredControlInstruction::Return => {
                let value = finalize_return_value(&mut state.stack, program.result_count)?;
                state.push_step(
                    control_depth,
                    TassadarStructuredControlTraceEvent::Return { value },
                )?;
                return Ok(ControlSignal::Return(value));
            }
        }
    }
    Ok(ControlSignal::Continue)
}

fn get_local(
    state: &ExecutionState,
    local_index: u32,
) -> Result<i32, TassadarStructuredControlError> {
    state.locals.get(local_index as usize).copied().ok_or(
        TassadarStructuredControlError::LocalOutOfRange {
            local_index,
            local_count: state.locals.len(),
        },
    )
}

fn get_local_mut(
    state: &mut ExecutionState,
    local_index: u32,
) -> Result<&mut i32, TassadarStructuredControlError> {
    let local_count = state.locals.len();
    state.locals.get_mut(local_index as usize).ok_or(
        TassadarStructuredControlError::LocalOutOfRange {
            local_index,
            local_count,
        },
    )
}

fn pop_stack_value(
    state: &mut ExecutionState,
    context: &str,
) -> Result<i32, TassadarStructuredControlError> {
    let available = state.stack.len();
    state
        .stack
        .pop()
        .ok_or_else(|| TassadarStructuredControlError::StackUnderflow {
            context: String::from(context),
            needed: 1,
            available,
        })
}

fn finalize_return_value(
    stack: &mut Vec<i32>,
    result_count: u8,
) -> Result<Option<i32>, TassadarStructuredControlError> {
    match result_count {
        0 => Ok(None),
        1 => {
            let available = stack.len();
            stack
                .pop()
                .map(Some)
                .ok_or_else(|| TassadarStructuredControlError::StackUnderflow {
                    context: String::from("return_value"),
                    needed: 1,
                    available,
                })
        }
        other => Err(TassadarStructuredControlError::UnsupportedResultCount {
            result_count: other,
        }),
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
        TassadarStructuredControlBinaryOp, TassadarStructuredControlError,
        TassadarStructuredControlHaltReason, TassadarStructuredControlInstruction,
        TassadarStructuredControlProgram, TassadarStructuredControlUnaryOp,
        execute_tassadar_structured_control_program,
    };

    #[test]
    fn structured_control_program_validates_branch_depths() {
        let program = TassadarStructuredControlProgram::new(
            "invalid_depth",
            0,
            0,
            vec![TassadarStructuredControlInstruction::Br { depth: 0 }],
        );
        let error = program.validate().expect_err("validation should fail");
        assert!(matches!(
            error,
            TassadarStructuredControlError::InvalidBranchDepth { .. }
        ));
    }

    #[test]
    fn structured_control_program_executes_if_else_exactly() {
        let program = TassadarStructuredControlProgram::new(
            "if_else",
            1,
            1,
            vec![
                TassadarStructuredControlInstruction::I32Const { value: 7 },
                TassadarStructuredControlInstruction::LocalSet { local_index: 0 },
                TassadarStructuredControlInstruction::LocalGet { local_index: 0 },
                TassadarStructuredControlInstruction::I32Const { value: 5 },
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::LtS,
                },
                TassadarStructuredControlInstruction::If {
                    label_id: String::from("if_0"),
                    then_instructions: vec![
                        TassadarStructuredControlInstruction::I32Const { value: 11 },
                        TassadarStructuredControlInstruction::LocalSet { local_index: 0 },
                    ],
                    else_instructions: vec![
                        TassadarStructuredControlInstruction::I32Const { value: 22 },
                        TassadarStructuredControlInstruction::LocalSet { local_index: 0 },
                    ],
                },
                TassadarStructuredControlInstruction::LocalGet { local_index: 0 },
                TassadarStructuredControlInstruction::Return,
            ],
        );
        let execution = execute_tassadar_structured_control_program(&program).expect("execute");
        assert_eq!(execution.returned_value, Some(22));
        assert_eq!(execution.final_locals, vec![22]);
        assert_eq!(
            execution.halt_reason,
            TassadarStructuredControlHaltReason::Returned
        );
    }

    #[test]
    fn structured_control_program_executes_loop_and_branch_table_exactly() {
        let loop_program = TassadarStructuredControlProgram::new(
            "loop",
            1,
            1,
            vec![
                TassadarStructuredControlInstruction::I32Const { value: 4 },
                TassadarStructuredControlInstruction::LocalSet { local_index: 0 },
                TassadarStructuredControlInstruction::Block {
                    label_id: String::from("block_0"),
                    instructions: vec![TassadarStructuredControlInstruction::Loop {
                        label_id: String::from("loop_0"),
                        instructions: vec![
                            TassadarStructuredControlInstruction::LocalGet { local_index: 0 },
                            TassadarStructuredControlInstruction::I32Const { value: 1 },
                            TassadarStructuredControlInstruction::BinaryOp {
                                op: TassadarStructuredControlBinaryOp::Sub,
                            },
                            TassadarStructuredControlInstruction::LocalTee { local_index: 0 },
                            TassadarStructuredControlInstruction::BrIf { depth: 0 },
                        ],
                    }],
                },
                TassadarStructuredControlInstruction::LocalGet { local_index: 0 },
                TassadarStructuredControlInstruction::Return,
            ],
        );
        let loop_execution =
            execute_tassadar_structured_control_program(&loop_program).expect("loop execute");
        assert_eq!(loop_execution.returned_value, Some(0));

        let branch_table_program = TassadarStructuredControlProgram::new(
            "branch_table",
            1,
            1,
            vec![
                TassadarStructuredControlInstruction::I32Const { value: 300 },
                TassadarStructuredControlInstruction::LocalSet { local_index: 0 },
                TassadarStructuredControlInstruction::Block {
                    label_id: String::from("outer"),
                    instructions: vec![
                        TassadarStructuredControlInstruction::Block {
                            label_id: String::from("middle"),
                            instructions: vec![
                                TassadarStructuredControlInstruction::Block {
                                    label_id: String::from("inner"),
                                    instructions: vec![
                                        TassadarStructuredControlInstruction::I32Const { value: 1 },
                                        TassadarStructuredControlInstruction::BrTable {
                                            target_depths: vec![0, 1],
                                            default_depth: 2,
                                        },
                                    ],
                                },
                                TassadarStructuredControlInstruction::I32Const { value: 100 },
                                TassadarStructuredControlInstruction::LocalSet { local_index: 0 },
                                TassadarStructuredControlInstruction::Br { depth: 1 },
                            ],
                        },
                        TassadarStructuredControlInstruction::I32Const { value: 200 },
                        TassadarStructuredControlInstruction::LocalSet { local_index: 0 },
                        TassadarStructuredControlInstruction::Br { depth: 0 },
                    ],
                },
                TassadarStructuredControlInstruction::LocalGet { local_index: 0 },
                TassadarStructuredControlInstruction::Return,
            ],
        );
        let branch_execution = execute_tassadar_structured_control_program(&branch_table_program)
            .expect("branch execute");
        assert_eq!(branch_execution.returned_value, Some(200));
    }

    #[test]
    fn structured_control_program_executes_widened_i32_numeric_ops_exactly() {
        let program = TassadarStructuredControlProgram::new(
            "numeric_widening",
            1,
            1,
            vec![
                TassadarStructuredControlInstruction::I32Const { value: 0 },
                TassadarStructuredControlInstruction::UnaryOp {
                    op: TassadarStructuredControlUnaryOp::Eqz,
                },
                TassadarStructuredControlInstruction::LocalSet { local_index: 0 },
                TassadarStructuredControlInstruction::I32Const { value: 7 },
                TassadarStructuredControlInstruction::I32Const { value: 7 },
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Eq,
                },
                TassadarStructuredControlInstruction::LocalGet { local_index: 0 },
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarStructuredControlInstruction::LocalSet { local_index: 0 },
                TassadarStructuredControlInstruction::I32Const { value: 6 },
                TassadarStructuredControlInstruction::I32Const { value: 3 },
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::And,
                },
                TassadarStructuredControlInstruction::I32Const { value: 2 },
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Eq,
                },
                TassadarStructuredControlInstruction::LocalGet { local_index: 0 },
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarStructuredControlInstruction::LocalSet { local_index: 0 },
                TassadarStructuredControlInstruction::I32Const { value: -8 },
                TassadarStructuredControlInstruction::I32Const { value: 1 },
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::ShrU,
                },
                TassadarStructuredControlInstruction::I32Const {
                    value: 0x7fff_fffc_u32 as i32,
                },
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Eq,
                },
                TassadarStructuredControlInstruction::LocalGet { local_index: 0 },
                TassadarStructuredControlInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarStructuredControlInstruction::Return,
            ],
        );
        let execution = execute_tassadar_structured_control_program(&program).expect("execute");
        assert_eq!(execution.returned_value, Some(4));
    }
}
