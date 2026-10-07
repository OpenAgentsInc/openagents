use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TassadarStructuredControlBinaryOp;

const TASSADAR_CALL_FRAME_MAX_STEPS: usize = 4_096;

/// One instruction in the bounded direct-call call-frame lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "opcode", rename_all = "snake_case")]
pub enum TassadarCallFrameInstruction {
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
    /// Tee one local from the stack top.
    LocalTee {
        /// Local index.
        local_index: u32,
    },
    /// Pop two `i32` values and push one arithmetic result.
    BinaryOp {
        /// Arithmetic family.
        op: TassadarStructuredControlBinaryOp,
    },
    /// Jump unconditionally to one program counter inside the current function.
    Jump {
        /// Target program counter.
        target_pc: usize,
    },
    /// Pop one value and jump when it is zero.
    JumpIfZero {
        /// Target program counter.
        target_pc: usize,
    },
    /// Call one direct target function.
    Call {
        /// Target function index.
        function_index: u32,
    },
    /// Return from the current function.
    Return,
}

/// One function in the bounded call-frame lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameFunction {
    /// Stable function index.
    pub function_index: u32,
    /// Stable function label.
    pub function_name: String,
    /// Number of parameters consumed by direct callers.
    pub param_count: u8,
    /// Number of locals including parameters.
    pub local_count: usize,
    /// Number of return values. Only `0` or `1` are supported.
    pub result_count: u8,
    /// Ordered instruction sequence.
    pub instructions: Vec<TassadarCallFrameInstruction>,
}

impl TassadarCallFrameFunction {
    /// Creates one bounded call-frame function.
    #[must_use]
    pub fn new(
        function_index: u32,
        function_name: impl Into<String>,
        param_count: u8,
        local_count: usize,
        result_count: u8,
        instructions: Vec<TassadarCallFrameInstruction>,
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

/// One multi-function program in the bounded call-frame lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameProgram {
    /// Stable program identifier.
    pub program_id: String,
    /// Entry function index.
    pub entry_function_index: u32,
    /// Maximum call depth before recursion is refused.
    pub max_call_depth: u32,
    /// Ordered function table.
    pub functions: Vec<TassadarCallFrameFunction>,
}

impl TassadarCallFrameProgram {
    /// Creates one call-frame program.
    #[must_use]
    pub fn new(
        program_id: impl Into<String>,
        entry_function_index: u32,
        max_call_depth: u32,
        functions: Vec<TassadarCallFrameFunction>,
    ) -> Self {
        Self {
            program_id: program_id.into(),
            entry_function_index,
            max_call_depth,
            functions,
        }
    }

    /// Validates the public call-frame surface.
    pub fn validate(&self) -> Result<(), TassadarCallFrameError> {
        if self.functions.is_empty() {
            return Err(TassadarCallFrameError::NoFunctions);
        }
        let entry = self
            .functions
            .iter()
            .find(|function| function.function_index == self.entry_function_index)
            .ok_or(TassadarCallFrameError::MissingEntryFunction {
                entry_function_index: self.entry_function_index,
            })?;
        if entry.param_count != 0 {
            return Err(TassadarCallFrameError::EntryFunctionHasParameters {
                entry_function_index: self.entry_function_index,
                param_count: entry.param_count,
            });
        }
        for (expected_index, function) in self.functions.iter().enumerate() {
            if function.function_index != expected_index as u32 {
                return Err(TassadarCallFrameError::FunctionIndexDrift {
                    expected: expected_index as u32,
                    actual: function.function_index,
                });
            }
            if function.local_count < usize::from(function.param_count) {
                return Err(TassadarCallFrameError::LocalCountTooSmall {
                    function_index: function.function_index,
                    param_count: function.param_count,
                    local_count: function.local_count,
                });
            }
            if function.result_count > 1 {
                return Err(TassadarCallFrameError::UnsupportedResultCount {
                    function_index: function.function_index,
                    result_count: function.result_count,
                });
            }
            for instruction in &function.instructions {
                match instruction {
                    TassadarCallFrameInstruction::LocalGet { local_index }
                    | TassadarCallFrameInstruction::LocalSet { local_index }
                    | TassadarCallFrameInstruction::LocalTee { local_index }
                        if *local_index as usize >= function.local_count =>
                    {
                        return Err(TassadarCallFrameError::LocalOutOfRange {
                            function_index: function.function_index,
                            local_index: *local_index,
                            local_count: function.local_count,
                        });
                    }
                    TassadarCallFrameInstruction::Call { function_index }
                        if *function_index as usize >= self.functions.len() =>
                    {
                        return Err(TassadarCallFrameError::CallTargetOutOfRange {
                            function_index: function.function_index,
                            target_function_index: *function_index,
                            function_count: self.functions.len(),
                        });
                    }
                    TassadarCallFrameInstruction::Jump { target_pc }
                    | TassadarCallFrameInstruction::JumpIfZero { target_pc }
                        if *target_pc > function.instructions.len() =>
                    {
                        return Err(TassadarCallFrameError::ControlTargetOutOfRange {
                            function_index: function.function_index,
                            target_pc: *target_pc,
                            instruction_count: function.instructions.len(),
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// One visible call-frame snapshot captured in the trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameSnapshot {
    /// Stable function index.
    pub function_index: u32,
    /// Stable function label.
    pub function_name: String,
    /// Program counter inside the function.
    pub pc: usize,
    /// Current locals.
    pub locals: Vec<i32>,
    /// Current operand stack.
    pub operand_stack: Vec<i32>,
}

/// One trace event in the bounded call-frame lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarCallFrameTraceEvent {
    /// One constant was pushed.
    ConstPush { value: i32 },
    /// One local was read.
    LocalGet { local_index: u32, value: i32 },
    /// One local was written.
    LocalSet { local_index: u32, value: i32 },
    /// One local was tee'd.
    LocalTee { local_index: u32, value: i32 },
    /// One arithmetic operation completed.
    BinaryOp {
        op: TassadarStructuredControlBinaryOp,
        left: i32,
        right: i32,
        result: i32,
    },
    /// One unconditional jump advanced control inside the current frame.
    Jump {
        function_index: u32,
        from_pc: usize,
        target_pc: usize,
    },
    /// One conditional jump tested one value and either jumped or fell through.
    JumpIfZero {
        function_index: u32,
        from_pc: usize,
        target_pc: usize,
        value: i32,
        taken: bool,
    },
    /// One direct call pushed a new frame.
    Call {
        caller_function_index: u32,
        callee_function_index: u32,
        args: Vec<i32>,
    },
    /// One frame returned.
    Return {
        function_index: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<i32>,
        implicit: bool,
    },
}

/// One append-only call-frame trace step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameTraceStep {
    /// Step index in execution order.
    pub step_index: usize,
    /// Current frame depth after the step.
    pub frame_depth_after: usize,
    /// Event emitted by the step.
    pub event: TassadarCallFrameTraceEvent,
    /// Full frame stack snapshot after the step.
    pub frame_stack_after: Vec<TassadarCallFrameSnapshot>,
}

/// Terminal reason for one call-frame execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCallFrameHaltReason {
    /// The entry frame returned.
    Returned,
    /// The entry frame fell off the end.
    FellOffEnd,
}

/// One complete execution result for the bounded call-frame lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCallFrameExecution {
    /// Stable program identifier.
    pub program_id: String,
    /// Ordered append-only trace steps.
    pub steps: Vec<TassadarCallFrameTraceStep>,
    /// Optional returned value from the entry frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarCallFrameHaltReason,
}

impl TassadarCallFrameExecution {
    /// Returns a stable digest over the visible execution truth.
    #[must_use]
    pub fn execution_digest(&self) -> String {
        stable_digest(b"tassadar_call_frame_execution|", self)
    }
}

/// Typed validation or execution failure for the call-frame lane.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TassadarCallFrameError {
    #[error("call-frame program declares no functions")]
    NoFunctions,
    #[error("entry function {entry_function_index} is missing")]
    MissingEntryFunction { entry_function_index: u32 },
    #[error(
        "entry function {entry_function_index} declares {param_count} params, but the bounded lane only admits zero-parameter entry functions"
    )]
    EntryFunctionHasParameters {
        entry_function_index: u32,
        param_count: u8,
    },
    #[error("function index drift: expected {expected}, found {actual}")]
    FunctionIndexDrift { expected: u32, actual: u32 },
    #[error(
        "function {function_index} local_count {local_count} is smaller than param_count {param_count}"
    )]
    LocalCountTooSmall {
        function_index: u32,
        param_count: u8,
        local_count: usize,
    },
    #[error(
        "function {function_index} declares unsupported result count {result_count}; only 0 or 1 are supported"
    )]
    UnsupportedResultCount {
        function_index: u32,
        result_count: u8,
    },
    #[error(
        "function {function_index} local {local_index} is out of range (local_count={local_count})"
    )]
    LocalOutOfRange {
        function_index: u32,
        local_index: u32,
        local_count: usize,
    },
    #[error(
        "function {function_index} calls missing target {target_function_index} (function_count={function_count})"
    )]
    CallTargetOutOfRange {
        function_index: u32,
        target_function_index: u32,
        function_count: usize,
    },
    #[error(
        "function {function_index} branches to missing target pc {target_pc} (instruction_count={instruction_count})"
    )]
    ControlTargetOutOfRange {
        function_index: u32,
        target_pc: usize,
        instruction_count: usize,
    },
    #[error(
        "call-frame stack underflow in function {function_index} at pc {pc} for {context}: needed {needed}, available {available}"
    )]
    StackUnderflow {
        function_index: u32,
        pc: usize,
        context: String,
        needed: usize,
        available: usize,
    },
    #[error(
        "bounded recursion refusal: calling function {attempted_function_index} would exceed max_call_depth {max_call_depth}"
    )]
    RecursionDepthExceeded {
        attempted_function_index: u32,
        max_call_depth: u32,
    },
    #[error("call-frame execution exceeded the step limit of {max_steps}")]
    StepLimitExceeded { max_steps: usize },
}

/// Executes one bounded direct-call multi-function program.
pub fn execute_tassadar_call_frame_program(
    program: &TassadarCallFrameProgram,
) -> Result<TassadarCallFrameExecution, TassadarCallFrameError> {
    program.validate()?;
    let entry_function = &program.functions[program.entry_function_index as usize];
    let mut state = CallFrameState {
        frames: vec![ExecutionFrame::new(entry_function, Vec::new())],
        steps: Vec::new(),
        step_index: 0,
    };
    let mut halt_reason = TassadarCallFrameHaltReason::FellOffEnd;
    let mut returned_value = None;

    while !state.frames.is_empty() {
        if state.step_index >= TASSADAR_CALL_FRAME_MAX_STEPS {
            return Err(TassadarCallFrameError::StepLimitExceeded {
                max_steps: TASSADAR_CALL_FRAME_MAX_STEPS,
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
                state.push_step(TassadarCallFrameTraceEvent::Return {
                    function_index,
                    value,
                    implicit: true,
                })?;
                continue;
            }
            halt_reason = TassadarCallFrameHaltReason::FellOffEnd;
            returned_value = value;
            state.push_step(TassadarCallFrameTraceEvent::Return {
                function_index,
                value,
                implicit: true,
            })?;
            break;
        }

        let instruction = function.instructions[state.frames[current_index].pc].clone();
        match instruction {
            TassadarCallFrameInstruction::I32Const { value } => {
                state.frames[current_index].operand_stack.push(value);
                state.frames[current_index].pc += 1;
                state.push_step(TassadarCallFrameTraceEvent::ConstPush { value })?;
            }
            TassadarCallFrameInstruction::LocalGet { local_index } => {
                let value = *state.frames[current_index]
                    .locals
                    .get(local_index as usize)
                    .ok_or(TassadarCallFrameError::LocalOutOfRange {
                        function_index,
                        local_index,
                        local_count: function.local_count,
                    })?;
                state.frames[current_index].operand_stack.push(value);
                state.frames[current_index].pc += 1;
                state.push_step(TassadarCallFrameTraceEvent::LocalGet { local_index, value })?;
            }
            TassadarCallFrameInstruction::LocalSet { local_index } => {
                let value = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "local.set",
                )?;
                *state.frames[current_index]
                    .locals
                    .get_mut(local_index as usize)
                    .ok_or(TassadarCallFrameError::LocalOutOfRange {
                        function_index,
                        local_index,
                        local_count: function.local_count,
                    })? = value;
                state.frames[current_index].pc += 1;
                state.push_step(TassadarCallFrameTraceEvent::LocalSet { local_index, value })?;
            }
            TassadarCallFrameInstruction::LocalTee { local_index } => {
                let value = *state.frames[current_index]
                    .operand_stack
                    .last()
                    .ok_or_else(|| TassadarCallFrameError::StackUnderflow {
                        function_index,
                        pc: state.frames[current_index].pc,
                        context: String::from("local.tee"),
                        needed: 1,
                        available: 0,
                    })?;
                *state.frames[current_index]
                    .locals
                    .get_mut(local_index as usize)
                    .ok_or(TassadarCallFrameError::LocalOutOfRange {
                        function_index,
                        local_index,
                        local_count: function.local_count,
                    })? = value;
                state.frames[current_index].pc += 1;
                state.push_step(TassadarCallFrameTraceEvent::LocalTee { local_index, value })?;
            }
            TassadarCallFrameInstruction::BinaryOp { op } => {
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
                state.frames[current_index].operand_stack.push(result);
                state.frames[current_index].pc += 1;
                state.push_step(TassadarCallFrameTraceEvent::BinaryOp {
                    op,
                    left,
                    right,
                    result,
                })?;
            }
            TassadarCallFrameInstruction::Jump { target_pc } => {
                let from_pc = state.frames[current_index].pc;
                state.frames[current_index].pc = target_pc;
                state.push_step(TassadarCallFrameTraceEvent::Jump {
                    function_index,
                    from_pc,
                    target_pc,
                })?;
            }
            TassadarCallFrameInstruction::JumpIfZero { target_pc } => {
                let from_pc = state.frames[current_index].pc;
                let value = pop_operand(
                    &mut state.frames[current_index],
                    function_index,
                    "jump_if_zero",
                )?;
                let taken = value == 0;
                state.frames[current_index].pc = if taken { target_pc } else { from_pc + 1 };
                state.push_step(TassadarCallFrameTraceEvent::JumpIfZero {
                    function_index,
                    from_pc,
                    target_pc,
                    value,
                    taken,
                })?;
            }
            TassadarCallFrameInstruction::Call {
                function_index: callee_index,
            } => {
                if state.frames.len() as u32 >= program.max_call_depth {
                    return Err(TassadarCallFrameError::RecursionDepthExceeded {
                        attempted_function_index: callee_index,
                        max_call_depth: program.max_call_depth,
                    });
                }
                let callee = &program.functions[callee_index as usize];
                let args = pop_call_args(&mut state.frames[current_index], function_index, callee)?;
                state.frames[current_index].pc += 1;
                state.frames.push(ExecutionFrame::new(callee, args.clone()));
                state.push_step(TassadarCallFrameTraceEvent::Call {
                    caller_function_index: function_index,
                    callee_function_index: callee_index,
                    args,
                })?;
            }
            TassadarCallFrameInstruction::Return => {
                let value =
                    finalize_function_return(&mut state.frames[current_index], function, true)?;
                state.frames.pop();
                if let Some(caller) = state.frames.last_mut() {
                    if let Some(value) = value {
                        caller.operand_stack.push(value);
                    }
                    state.push_step(TassadarCallFrameTraceEvent::Return {
                        function_index,
                        value,
                        implicit: false,
                    })?;
                    continue;
                }
                halt_reason = TassadarCallFrameHaltReason::Returned;
                returned_value = value;
                state.push_step(TassadarCallFrameTraceEvent::Return {
                    function_index,
                    value,
                    implicit: false,
                })?;
                break;
            }
        }
    }

    Ok(TassadarCallFrameExecution {
        program_id: program.program_id.clone(),
        steps: state.steps,
        returned_value,
        halt_reason,
    })
}

struct ExecutionFrame {
    function_index: u32,
    function_name: String,
    pc: usize,
    locals: Vec<i32>,
    operand_stack: Vec<i32>,
}

impl ExecutionFrame {
    fn new(function: &TassadarCallFrameFunction, args: Vec<i32>) -> Self {
        let mut locals = vec![0; function.local_count];
        for (index, value) in args.into_iter().enumerate() {
            locals[index] = value;
        }
        Self {
            function_index: function.function_index,
            function_name: function.function_name.clone(),
            pc: 0,
            locals,
            operand_stack: Vec::new(),
        }
    }
}

struct CallFrameState {
    frames: Vec<ExecutionFrame>,
    steps: Vec<TassadarCallFrameTraceStep>,
    step_index: usize,
}

impl CallFrameState {
    fn push_step(
        &mut self,
        event: TassadarCallFrameTraceEvent,
    ) -> Result<(), TassadarCallFrameError> {
        self.steps.push(TassadarCallFrameTraceStep {
            step_index: self.step_index,
            frame_depth_after: self.frames.len(),
            event,
            frame_stack_after: self
                .frames
                .iter()
                .map(|frame| TassadarCallFrameSnapshot {
                    function_index: frame.function_index,
                    function_name: frame.function_name.clone(),
                    pc: frame.pc,
                    locals: frame.locals.clone(),
                    operand_stack: frame.operand_stack.clone(),
                })
                .collect(),
        });
        self.step_index = self.step_index.saturating_add(1);
        Ok(())
    }
}

fn pop_operand(
    frame: &mut ExecutionFrame,
    function_index: u32,
    context: &str,
) -> Result<i32, TassadarCallFrameError> {
    let available = frame.operand_stack.len();
    frame
        .operand_stack
        .pop()
        .ok_or_else(|| TassadarCallFrameError::StackUnderflow {
            function_index,
            pc: frame.pc,
            context: String::from(context),
            needed: 1,
            available,
        })
}

fn pop_call_args(
    frame: &mut ExecutionFrame,
    caller_function_index: u32,
    callee: &TassadarCallFrameFunction,
) -> Result<Vec<i32>, TassadarCallFrameError> {
    let needed = usize::from(callee.param_count);
    let available = frame.operand_stack.len();
    if available < needed {
        return Err(TassadarCallFrameError::StackUnderflow {
            function_index: caller_function_index,
            pc: frame.pc,
            context: String::from("call_args"),
            needed,
            available,
        });
    }
    let mut args = Vec::with_capacity(needed);
    for _ in 0..needed {
        args.push(frame.operand_stack.pop().expect("availability checked"));
    }
    args.reverse();
    Ok(args)
}

fn finalize_function_return(
    frame: &mut ExecutionFrame,
    function: &TassadarCallFrameFunction,
    advance_pc: bool,
) -> Result<Option<i32>, TassadarCallFrameError> {
    if advance_pc {
        frame.pc += 1;
    }
    match function.result_count {
        0 => Ok(None),
        1 => {
            let available = frame.operand_stack.len();
            frame.operand_stack.pop().map(Some).ok_or_else(|| {
                TassadarCallFrameError::StackUnderflow {
                    function_index: function.function_index,
                    pc: frame.pc.saturating_sub(usize::from(advance_pc)),
                    context: String::from("return_value"),
                    needed: 1,
                    available,
                }
            })
        }
        result_count => Err(TassadarCallFrameError::UnsupportedResultCount {
            function_index: function.function_index,
            result_count,
        }),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

/// Returns a seeded direct-call parity program.
#[must_use]
pub fn tassadar_seeded_call_frame_direct_call_program() -> TassadarCallFrameProgram {
    TassadarCallFrameProgram::new(
        "tassadar.call_frames.direct_call.v1",
        0,
        8,
        vec![
            TassadarCallFrameFunction::new(
                0,
                "entry",
                0,
                0,
                1,
                vec![
                    TassadarCallFrameInstruction::I32Const { value: 7 },
                    TassadarCallFrameInstruction::Call { function_index: 1 },
                    TassadarCallFrameInstruction::Call { function_index: 1 },
                    TassadarCallFrameInstruction::Return,
                ],
            ),
            TassadarCallFrameFunction::new(
                1,
                "add_one",
                1,
                1,
                1,
                vec![
                    TassadarCallFrameInstruction::LocalGet { local_index: 0 },
                    TassadarCallFrameInstruction::I32Const { value: 1 },
                    TassadarCallFrameInstruction::BinaryOp {
                        op: TassadarStructuredControlBinaryOp::Add,
                    },
                    TassadarCallFrameInstruction::Return,
                ],
            ),
        ],
    )
}

/// Returns a seeded multi-function nested-call workload.
#[must_use]
pub fn tassadar_seeded_call_frame_multi_function_program() -> TassadarCallFrameProgram {
    TassadarCallFrameProgram::new(
        "tassadar.call_frames.multi_function.v1",
        0,
        8,
        vec![
            TassadarCallFrameFunction::new(
                0,
                "entry",
                0,
                0,
                1,
                vec![
                    TassadarCallFrameInstruction::I32Const { value: 3 },
                    TassadarCallFrameInstruction::I32Const { value: 4 },
                    TassadarCallFrameInstruction::Call { function_index: 1 },
                    TassadarCallFrameInstruction::Return,
                ],
            ),
            TassadarCallFrameFunction::new(
                1,
                "sum_of_squares",
                2,
                2,
                1,
                vec![
                    TassadarCallFrameInstruction::LocalGet { local_index: 0 },
                    TassadarCallFrameInstruction::Call { function_index: 2 },
                    TassadarCallFrameInstruction::LocalGet { local_index: 1 },
                    TassadarCallFrameInstruction::Call { function_index: 2 },
                    TassadarCallFrameInstruction::BinaryOp {
                        op: TassadarStructuredControlBinaryOp::Add,
                    },
                    TassadarCallFrameInstruction::Return,
                ],
            ),
            TassadarCallFrameFunction::new(
                2,
                "square",
                1,
                1,
                1,
                vec![
                    TassadarCallFrameInstruction::LocalGet { local_index: 0 },
                    TassadarCallFrameInstruction::LocalGet { local_index: 0 },
                    TassadarCallFrameInstruction::BinaryOp {
                        op: TassadarStructuredControlBinaryOp::Mul,
                    },
                    TassadarCallFrameInstruction::Return,
                ],
            ),
        ],
    )
}

/// Returns a seeded bounded-recursion refusal program.
#[must_use]
pub fn tassadar_seeded_call_frame_recursion_program() -> TassadarCallFrameProgram {
    TassadarCallFrameProgram::new(
        "tassadar.call_frames.recursion_refusal.v1",
        0,
        3,
        vec![
            TassadarCallFrameFunction::new(
                0,
                "entry",
                0,
                0,
                1,
                vec![
                    TassadarCallFrameInstruction::Call { function_index: 1 },
                    TassadarCallFrameInstruction::Return,
                ],
            ),
            TassadarCallFrameFunction::new(
                1,
                "recurse",
                0,
                0,
                1,
                vec![
                    TassadarCallFrameInstruction::Call { function_index: 1 },
                    TassadarCallFrameInstruction::Return,
                ],
            ),
        ],
    )
}

/// Returns a seeded bounded-recursion exactness program with conditional control.
#[must_use]
pub fn tassadar_seeded_call_frame_recursive_sum_program() -> TassadarCallFrameProgram {
    TassadarCallFrameProgram::new(
        "tassadar.call_frames.recursive_sum.v1",
        0,
        8,
        vec![
            TassadarCallFrameFunction::new(
                0,
                "entry",
                0,
                0,
                1,
                vec![
                    TassadarCallFrameInstruction::I32Const { value: 5 },
                    TassadarCallFrameInstruction::Call { function_index: 1 },
                    TassadarCallFrameInstruction::Return,
                ],
            ),
            TassadarCallFrameFunction::new(
                1,
                "recursive_sum",
                1,
                1,
                1,
                vec![
                    TassadarCallFrameInstruction::LocalGet { local_index: 0 },
                    TassadarCallFrameInstruction::JumpIfZero { target_pc: 9 },
                    TassadarCallFrameInstruction::LocalGet { local_index: 0 },
                    TassadarCallFrameInstruction::LocalGet { local_index: 0 },
                    TassadarCallFrameInstruction::I32Const { value: 1 },
                    TassadarCallFrameInstruction::BinaryOp {
                        op: TassadarStructuredControlBinaryOp::Sub,
                    },
                    TassadarCallFrameInstruction::Call { function_index: 1 },
                    TassadarCallFrameInstruction::BinaryOp {
                        op: TassadarStructuredControlBinaryOp::Add,
                    },
                    TassadarCallFrameInstruction::Return,
                    TassadarCallFrameInstruction::I32Const { value: 0 },
                    TassadarCallFrameInstruction::Return,
                ],
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::{
        execute_tassadar_call_frame_program, tassadar_seeded_call_frame_direct_call_program,
        tassadar_seeded_call_frame_multi_function_program,
        tassadar_seeded_call_frame_recursion_program,
        tassadar_seeded_call_frame_recursive_sum_program, TassadarCallFrameError,
        TassadarCallFrameFunction, TassadarCallFrameHaltReason, TassadarCallFrameInstruction,
        TassadarCallFrameProgram,
    };

    #[test]
    fn call_frame_program_executes_direct_calls_exactly() {
        let execution =
            execute_tassadar_call_frame_program(&tassadar_seeded_call_frame_direct_call_program())
                .expect("execute");
        assert_eq!(execution.returned_value, Some(9));
        assert_eq!(execution.halt_reason, TassadarCallFrameHaltReason::Returned);
    }

    #[test]
    fn call_frame_program_replays_multi_function_frame_stack() {
        let execution = execute_tassadar_call_frame_program(
            &tassadar_seeded_call_frame_multi_function_program(),
        )
        .expect("execute");
        assert_eq!(execution.returned_value, Some(25));
        assert!(execution
            .steps
            .iter()
            .any(|step| step.frame_depth_after == 3));
        assert!(execution.steps.iter().any(|step| {
            step.frame_stack_after
                .iter()
                .any(|frame| frame.function_name == "square")
        }));
    }

    #[test]
    fn call_frame_program_refuses_bounded_recursion_overflow() {
        let error =
            execute_tassadar_call_frame_program(&tassadar_seeded_call_frame_recursion_program())
                .expect_err("should refuse");
        assert!(matches!(
            error,
            TassadarCallFrameError::RecursionDepthExceeded { .. }
        ));
    }

    #[test]
    fn call_frame_program_executes_bounded_recursion_exactly() {
        let execution = execute_tassadar_call_frame_program(
            &tassadar_seeded_call_frame_recursive_sum_program(),
        )
        .expect("execute");
        assert_eq!(execution.returned_value, Some(15));
        assert!(execution.steps.iter().any(|step| {
            matches!(
                step.event,
                super::TassadarCallFrameTraceEvent::JumpIfZero { taken: true, .. }
            )
        }));
        assert!(
            execution
                .steps
                .iter()
                .map(|step| step.frame_depth_after)
                .max()
                .unwrap_or_default()
                >= 6
        );
    }

    #[test]
    fn call_frame_program_rejects_out_of_range_control_target() {
        let program = TassadarCallFrameProgram::new(
            "tassadar.call_frames.bad_jump.v1",
            0,
            4,
            vec![TassadarCallFrameFunction::new(
                0,
                "entry",
                0,
                0,
                0,
                vec![TassadarCallFrameInstruction::Jump { target_pc: 2 }],
            )],
        );
        let error = execute_tassadar_call_frame_program(&program).expect_err("should reject");
        assert!(matches!(
            error,
            TassadarCallFrameError::ControlTargetOutOfRange { .. }
        ));
    }
}
