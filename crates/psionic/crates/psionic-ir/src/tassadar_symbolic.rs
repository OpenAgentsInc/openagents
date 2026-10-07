use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for the bounded Tassadar symbolic IR.
pub const TASSADAR_SYMBOLIC_PROGRAM_SCHEMA_VERSION: u16 = 1;
/// Stable language identifier for the bounded Tassadar symbolic IR.
pub const TASSADAR_SYMBOLIC_PROGRAM_LANGUAGE_ID: &str = "tassadar.symbolic_executor_ir.v1";
/// Coarse claim class for the bounded symbolic lane.
pub const TASSADAR_SYMBOLIC_PROGRAM_CLAIM_CLASS: &str = "compiled_bounded_exactness";

/// One runtime-lowering opcode family required by the symbolic program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSymbolicLoweringOpcode {
    /// `i32.const`
    I32Const,
    /// `i32.load`
    I32Load,
    /// `i32.store`
    I32Store,
    /// `local.get`
    LocalGet,
    /// `local.set`
    LocalSet,
    /// `i32.add`
    I32Add,
    /// `i32.sub`
    I32Sub,
    /// `i32.mul`
    I32Mul,
    /// `i32.lt`
    I32Lt,
    /// `output`
    Output,
    /// `return`
    Return,
}

impl TassadarSymbolicLoweringOpcode {
    /// Returns the stable lowering mnemonic.
    #[must_use]
    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::I32Const => "i32.const",
            Self::I32Load => "i32.load",
            Self::I32Store => "i32.store",
            Self::LocalGet => "local.get",
            Self::LocalSet => "local.set",
            Self::I32Add => "i32.add",
            Self::I32Sub => "i32.sub",
            Self::I32Mul => "i32.mul",
            Self::I32Lt => "i32.lt",
            Self::Output => "output",
            Self::Return => "return",
        }
    }
}

/// One named symbolic input bound to a runtime memory slot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicInput {
    /// Stable symbolic input name.
    pub name: String,
    /// Runtime memory slot carrying the input.
    pub memory_slot: u8,
}

/// One explicitly initialized runtime memory cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicMemoryCell {
    /// Runtime memory slot index.
    pub slot: u8,
    /// Initial value written into the slot before execution.
    pub value: i32,
}

/// One symbolic operand in the bounded IR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarSymbolicOperand {
    /// Named input or prior `let` binding.
    Name {
        /// Stable symbolic identifier.
        name: String,
    },
    /// Inline `i32` literal.
    Const {
        /// Literal value.
        value: i32,
    },
    /// Direct memory-slot read from the current state.
    MemorySlot {
        /// Runtime memory slot index.
        slot: u8,
    },
}

/// One binary arithmetic/comparison family in the bounded IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSymbolicBinaryOp {
    /// `left + right`
    Add,
    /// `left - right`
    Sub,
    /// `left * right`
    Mul,
    /// `i32(left < right)`
    Lt,
}

impl TassadarSymbolicBinaryOp {
    /// Returns the stable symbolic operator label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Lt => "lt",
        }
    }

    /// Returns the runtime-lowering opcode required for this operator.
    #[must_use]
    pub const fn lowering_opcode(self) -> TassadarSymbolicLoweringOpcode {
        match self {
            Self::Add => TassadarSymbolicLoweringOpcode::I32Add,
            Self::Sub => TassadarSymbolicLoweringOpcode::I32Sub,
            Self::Mul => TassadarSymbolicLoweringOpcode::I32Mul,
            Self::Lt => TassadarSymbolicLoweringOpcode::I32Lt,
        }
    }
}

/// One expression in the bounded straight-line symbolic IR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarSymbolicExpr {
    /// Direct alias of one operand.
    Operand {
        /// Operand carried through unchanged.
        operand: TassadarSymbolicOperand,
    },
    /// One bounded binary operation over existing symbolic values.
    Binary {
        /// Binary operator.
        op: TassadarSymbolicBinaryOp,
        /// Left operand.
        left: TassadarSymbolicOperand,
        /// Right operand.
        right: TassadarSymbolicOperand,
    },
}

impl TassadarSymbolicExpr {
    fn operands(&self) -> impl Iterator<Item = &TassadarSymbolicOperand> {
        let mut operands = Vec::with_capacity(2);
        match self {
            Self::Operand { operand } => operands.push(operand),
            Self::Binary { left, right, .. } => {
                operands.push(left);
                operands.push(right);
            }
        }
        operands.into_iter()
    }
}

/// One statement in the bounded symbolic IR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarSymbolicStatement {
    /// Bind one symbolic name to one expression result.
    Let {
        /// Stable symbolic binding name.
        name: String,
        /// Expression assigned to the binding.
        expr: TassadarSymbolicExpr,
    },
    /// Write one value into the current memory state.
    Store {
        /// Runtime memory slot to update.
        slot: u8,
        /// Value written into the slot.
        value: TassadarSymbolicOperand,
    },
    /// Emit one final output value.
    Output {
        /// Value emitted through the executor output sink.
        value: TassadarSymbolicOperand,
    },
}

/// One bounded Tassadar symbolic program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicProgram {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable language identifier.
    pub language_id: String,
    /// Stable symbolic program identifier.
    pub program_id: String,
    /// Stable runtime profile identifier the program lowers against.
    pub profile_id: String,
    /// Number of runtime memory slots required by the program.
    pub memory_slots: usize,
    /// Named inputs bound to runtime memory slots.
    pub inputs: Vec<TassadarSymbolicInput>,
    /// Explicit static memory initialization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub initial_memory: Vec<TassadarSymbolicMemoryCell>,
    /// Ordered symbolic statements in execution order.
    pub statements: Vec<TassadarSymbolicStatement>,
}

impl TassadarSymbolicProgram {
    /// Parses one bounded symbolic program from the canonical textual IR.
    pub fn parse(source: &str) -> Result<Self, TassadarSymbolicProgramError> {
        let mut program_id: Option<String> = None;
        let mut profile_id: Option<String> = None;
        let mut memory_slots: Option<usize> = None;
        let mut inputs = Vec::new();
        let mut initial_memory = Vec::new();
        let mut statements = Vec::new();

        for (line_index, raw_line) in source.lines().enumerate() {
            let line_number = line_index + 1;
            let line = strip_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("program ") {
                if rest.trim().is_empty() {
                    return Err(TassadarSymbolicProgramError::InvalidDirective {
                        line: line_number,
                        directive: line.to_string(),
                    });
                }
                program_id = Some(rest.trim().to_string());
                continue;
            }
            if let Some(rest) = line.strip_prefix("profile ") {
                if rest.trim().is_empty() {
                    return Err(TassadarSymbolicProgramError::InvalidDirective {
                        line: line_number,
                        directive: line.to_string(),
                    });
                }
                profile_id = Some(rest.trim().to_string());
                continue;
            }
            if let Some(rest) = line.strip_prefix("memory_slots ") {
                let value = parse_usize(line_number, rest.trim())?;
                memory_slots = Some(value);
                continue;
            }
            if line.starts_with("input ") {
                inputs.push(parse_input(line_number, line)?);
                continue;
            }
            if line.starts_with("init ") {
                initial_memory.push(parse_init_cell(line_number, line)?);
                continue;
            }
            if line.starts_with("let ") {
                statements.push(parse_let_statement(line_number, line)?);
                continue;
            }
            if line.starts_with("store ") {
                statements.push(parse_store_statement(line_number, line)?);
                continue;
            }
            if line.starts_with("output ") {
                statements.push(parse_output_statement(line_number, line)?);
                continue;
            }
            if line.starts_with("if ")
                || line.starts_with("loop ")
                || line.starts_with("branch ")
                || line.starts_with("br_if ")
                || line.starts_with("goto ")
                || line.starts_with("return")
            {
                return Err(TassadarSymbolicProgramError::UnsupportedStatement {
                    line: line_number,
                    statement: line.to_string(),
                });
            }
            return Err(TassadarSymbolicProgramError::InvalidDirective {
                line: line_number,
                directive: line.to_string(),
            });
        }

        let program = Self {
            schema_version: TASSADAR_SYMBOLIC_PROGRAM_SCHEMA_VERSION,
            language_id: String::from(TASSADAR_SYMBOLIC_PROGRAM_LANGUAGE_ID),
            program_id: program_id.ok_or(TassadarSymbolicProgramError::MissingProgramId)?,
            profile_id: profile_id.ok_or(TassadarSymbolicProgramError::MissingProfileId)?,
            memory_slots: memory_slots.ok_or(TassadarSymbolicProgramError::MissingMemorySlots)?,
            inputs,
            initial_memory,
            statements,
        };
        program.validate()?;
        Ok(program)
    }

    /// Validates one bounded symbolic program.
    pub fn validate(&self) -> Result<(), TassadarSymbolicProgramError> {
        if self.schema_version != TASSADAR_SYMBOLIC_PROGRAM_SCHEMA_VERSION {
            return Err(TassadarSymbolicProgramError::SchemaVersionMismatch {
                expected: TASSADAR_SYMBOLIC_PROGRAM_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }
        if self.language_id != TASSADAR_SYMBOLIC_PROGRAM_LANGUAGE_ID {
            return Err(TassadarSymbolicProgramError::LanguageIdMismatch {
                expected: String::from(TASSADAR_SYMBOLIC_PROGRAM_LANGUAGE_ID),
                actual: self.language_id.clone(),
            });
        }
        if self.program_id.trim().is_empty() {
            return Err(TassadarSymbolicProgramError::MissingProgramId);
        }
        if self.profile_id.trim().is_empty() {
            return Err(TassadarSymbolicProgramError::MissingProfileId);
        }
        if self.memory_slots > usize::from(u8::MAX) + 1 {
            return Err(
                TassadarSymbolicProgramError::MemorySlotsExceedLoweringRange {
                    memory_slots: self.memory_slots,
                },
            );
        }

        let mut input_names = BTreeSet::new();
        let mut input_slots = BTreeSet::new();
        for input in &self.inputs {
            ensure_identifier(input.name.as_str())?;
            if !input_names.insert(input.name.clone()) {
                return Err(TassadarSymbolicProgramError::DuplicateInputName {
                    name: input.name.clone(),
                });
            }
            if usize::from(input.memory_slot) >= self.memory_slots {
                return Err(TassadarSymbolicProgramError::InvalidMemorySlot {
                    slot: input.memory_slot,
                    memory_slots: self.memory_slots,
                });
            }
            if !input_slots.insert(input.memory_slot) {
                return Err(TassadarSymbolicProgramError::DuplicateInputMemorySlot {
                    slot: input.memory_slot,
                });
            }
        }

        let mut initialized_slots = BTreeSet::new();
        for cell in &self.initial_memory {
            if usize::from(cell.slot) >= self.memory_slots {
                return Err(TassadarSymbolicProgramError::InvalidMemorySlot {
                    slot: cell.slot,
                    memory_slots: self.memory_slots,
                });
            }
            if !initialized_slots.insert(cell.slot) {
                return Err(TassadarSymbolicProgramError::DuplicateInitialMemorySlot {
                    slot: cell.slot,
                });
            }
            if let Some(input) = self
                .inputs
                .iter()
                .find(|input| input.memory_slot == cell.slot)
            {
                return Err(TassadarSymbolicProgramError::InputInitialMemoryConflict {
                    input: input.name.clone(),
                    slot: cell.slot,
                });
            }
        }

        let mut bindings = BTreeSet::new();
        for statement in &self.statements {
            match statement {
                TassadarSymbolicStatement::Let { name, expr } => {
                    ensure_identifier(name.as_str())?;
                    if input_names.contains(name) || !bindings.insert(name.clone()) {
                        return Err(TassadarSymbolicProgramError::DuplicateBindingName {
                            name: name.clone(),
                        });
                    }
                    for operand in expr.operands() {
                        validate_operand(operand, &input_names, &bindings, self.memory_slots)?;
                    }
                }
                TassadarSymbolicStatement::Store { slot, value } => {
                    if usize::from(*slot) >= self.memory_slots {
                        return Err(TassadarSymbolicProgramError::InvalidMemorySlot {
                            slot: *slot,
                            memory_slots: self.memory_slots,
                        });
                    }
                    validate_operand(value, &input_names, &bindings, self.memory_slots)?;
                }
                TassadarSymbolicStatement::Output { value } => {
                    validate_operand(value, &input_names, &bindings, self.memory_slots)?;
                }
            }
        }

        if self.required_lowered_locals() > usize::from(u8::MAX) + 1 {
            return Err(TassadarSymbolicProgramError::TooManyBindingsForLowering {
                binding_count: self.required_lowered_locals(),
            });
        }

        Ok(())
    }

    /// Returns the memory slot for one named input.
    #[must_use]
    pub fn input_slot(&self, input_name: &str) -> Option<u8> {
        self.inputs
            .iter()
            .find(|input| input.name == input_name)
            .map(|input| input.memory_slot)
    }

    /// Returns the number of runtime locals required by the current lowering.
    #[must_use]
    pub fn required_lowered_locals(&self) -> usize {
        self.statements
            .iter()
            .filter(|statement| matches!(statement, TassadarSymbolicStatement::Let { .. }))
            .count()
    }

    /// Returns the ordered runtime-lowering opcode families required by the program.
    #[must_use]
    pub fn required_lowering_opcodes(&self) -> Vec<TassadarSymbolicLoweringOpcode> {
        let mut opcodes = BTreeSet::from([TassadarSymbolicLoweringOpcode::Return]);
        if self
            .statements
            .iter()
            .any(|statement| matches!(statement, TassadarSymbolicStatement::Let { .. }))
        {
            opcodes.insert(TassadarSymbolicLoweringOpcode::LocalSet);
        }
        for statement in &self.statements {
            match statement {
                TassadarSymbolicStatement::Let { expr, .. } => {
                    push_operand_opcodes(&mut opcodes, expr.operands());
                    if let TassadarSymbolicExpr::Binary { op, .. } = expr {
                        opcodes.insert(op.lowering_opcode());
                    }
                }
                TassadarSymbolicStatement::Store { value, .. } => {
                    push_operand_opcodes(&mut opcodes, std::iter::once(value));
                    opcodes.insert(TassadarSymbolicLoweringOpcode::I32Store);
                }
                TassadarSymbolicStatement::Output { value } => {
                    push_operand_opcodes(&mut opcodes, std::iter::once(value));
                    opcodes.insert(TassadarSymbolicLoweringOpcode::Output);
                }
            }
        }
        opcodes.into_iter().collect()
    }

    /// Projects symbolic inputs and static memory into one runtime memory image.
    pub fn initial_memory_image(
        &self,
        input_assignments: &BTreeMap<String, i32>,
    ) -> Result<Vec<i32>, TassadarSymbolicProgramError> {
        self.validate()?;
        let declared_inputs = self
            .inputs
            .iter()
            .map(|input| input.name.as_str())
            .collect::<BTreeSet<_>>();
        for input in &self.inputs {
            if !input_assignments.contains_key(input.name.as_str()) {
                return Err(TassadarSymbolicProgramError::MissingInputAssignment {
                    input: input.name.clone(),
                });
            }
        }
        for input_name in input_assignments.keys() {
            if !declared_inputs.contains(input_name.as_str()) {
                return Err(TassadarSymbolicProgramError::UnknownInputAssignment {
                    input: input_name.clone(),
                });
            }
        }
        let mut memory = vec![0; self.memory_slots];
        for cell in &self.initial_memory {
            memory[usize::from(cell.slot)] = cell.value;
        }
        for input in &self.inputs {
            memory[usize::from(input.memory_slot)] = input_assignments
                .get(input.name.as_str())
                .copied()
                .ok_or_else(|| TassadarSymbolicProgramError::MissingInputAssignment {
                    input: input.name.clone(),
                })?;
        }
        Ok(memory)
    }

    /// Evaluates one symbolic program directly on the bounded symbolic IR.
    pub fn evaluate(
        &self,
        input_assignments: &BTreeMap<String, i32>,
    ) -> Result<TassadarSymbolicExecution, TassadarSymbolicProgramError> {
        let mut memory = self.initial_memory_image(input_assignments)?;
        let mut bindings = BTreeMap::new();
        let mut outputs = Vec::new();

        for statement in &self.statements {
            match statement {
                TassadarSymbolicStatement::Let { name, expr } => {
                    let value = match expr {
                        TassadarSymbolicExpr::Operand { operand } => {
                            resolve_operand(operand, input_assignments, &bindings, &memory)?
                        }
                        TassadarSymbolicExpr::Binary { op, left, right } => {
                            let left_value =
                                resolve_operand(left, input_assignments, &bindings, &memory)?;
                            let right_value =
                                resolve_operand(right, input_assignments, &bindings, &memory)?;
                            match op {
                                TassadarSymbolicBinaryOp::Add => {
                                    left_value.saturating_add(right_value)
                                }
                                TassadarSymbolicBinaryOp::Sub => {
                                    left_value.saturating_sub(right_value)
                                }
                                TassadarSymbolicBinaryOp::Mul => {
                                    left_value.saturating_mul(right_value)
                                }
                                TassadarSymbolicBinaryOp::Lt => i32::from(left_value < right_value),
                            }
                        }
                    };
                    bindings.insert(name.clone(), value);
                }
                TassadarSymbolicStatement::Store { slot, value } => {
                    memory[usize::from(*slot)] =
                        resolve_operand(value, input_assignments, &bindings, &memory)?;
                }
                TassadarSymbolicStatement::Output { value } => {
                    outputs.push(resolve_operand(
                        value,
                        input_assignments,
                        &bindings,
                        &memory,
                    )?);
                }
            }
        }

        Ok(TassadarSymbolicExecution {
            program_id: self.program_id.clone(),
            profile_id: self.profile_id.clone(),
            outputs,
            final_memory: memory,
        })
    }

    /// Returns a stable digest over the symbolic program payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"tassadar_symbolic_program|", self)
    }
}

impl FromStr for TassadarSymbolicProgram {
    type Err = TassadarSymbolicProgramError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        Self::parse(source)
    }
}

/// Direct symbolic execution result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicExecution {
    /// Stable program identifier.
    pub program_id: String,
    /// Stable profile identifier.
    pub profile_id: String,
    /// Emitted outputs.
    pub outputs: Vec<i32>,
    /// Final memory state after symbolic execution.
    pub final_memory: Vec<i32>,
}

/// Public sample symbolic program case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicProgramExample {
    /// Stable case identifier.
    pub case_id: String,
    /// Plain-language case summary.
    pub summary: String,
    /// Coarse claim class for the example family.
    pub claim_class: String,
    /// Boundary statement for the example family.
    pub claim_boundary: String,
    /// Bounded symbolic program.
    pub program: TassadarSymbolicProgram,
    /// Input assignments used by the seeded example.
    pub input_assignments: BTreeMap<String, i32>,
    /// Expected symbolic outputs.
    pub expected_outputs: Vec<i32>,
    /// Expected final memory image.
    pub expected_final_memory: Vec<i32>,
}

/// Parse, validation, or bounded symbolic execution failure.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarSymbolicProgramError {
    /// The textual IR omitted the required `program` header.
    #[error("symbolic IR is missing the required `program` header")]
    MissingProgramId,
    /// The textual IR omitted the required `profile` header.
    #[error("symbolic IR is missing the required `profile` header")]
    MissingProfileId,
    /// The textual IR omitted the required `memory_slots` header.
    #[error("symbolic IR is missing the required `memory_slots` header")]
    MissingMemorySlots,
    /// The schema version drifted.
    #[error("symbolic IR schema version mismatch: expected {expected}, got {actual}")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: u16,
        /// Actual schema version.
        actual: u16,
    },
    /// The language id drifted.
    #[error("symbolic IR language id mismatch: expected `{expected}`, got `{actual}`")]
    LanguageIdMismatch {
        /// Expected language id.
        expected: String,
        /// Actual language id.
        actual: String,
    },
    /// One line could not be classified into the supported grammar.
    #[error("invalid symbolic IR directive on line {line}: `{directive}`")]
    InvalidDirective {
        /// 1-based source line.
        line: usize,
        /// Raw directive text.
        directive: String,
    },
    /// One line requested unsupported control flow or another unsupported statement family.
    #[error("unsupported symbolic IR statement on line {line}: `{statement}`")]
    UnsupportedStatement {
        /// 1-based source line.
        line: usize,
        /// Raw statement text.
        statement: String,
    },
    /// One expression could not be parsed.
    #[error("invalid symbolic IR expression on line {line}: `{expression}`")]
    InvalidExpression {
        /// 1-based source line.
        line: usize,
        /// Raw expression text.
        expression: String,
    },
    /// One numeric literal could not be parsed.
    #[error("invalid symbolic IR integer on line {line}: `{value}`")]
    InvalidInteger {
        /// 1-based source line.
        line: usize,
        /// Raw integer text.
        value: String,
    },
    /// One identifier violated the bounded symbolic identifier rules.
    #[error("invalid symbolic IR identifier `{identifier}`")]
    InvalidIdentifier {
        /// Invalid identifier.
        identifier: String,
    },
    /// Two symbolic inputs shared the same name.
    #[error("duplicate symbolic input name `{name}`")]
    DuplicateInputName {
        /// Duplicate input name.
        name: String,
    },
    /// Two symbolic inputs shared the same runtime slot.
    #[error("duplicate symbolic input memory slot {slot}")]
    DuplicateInputMemorySlot {
        /// Duplicate input slot.
        slot: u8,
    },
    /// Two symbolic `init` directives targeted the same slot.
    #[error("duplicate symbolic initial-memory slot {slot}")]
    DuplicateInitialMemorySlot {
        /// Duplicate initialized slot.
        slot: u8,
    },
    /// A symbolic input and a static initialization both targeted the same slot.
    #[error("symbolic input `{input}` conflicts with static initialization of slot {slot}")]
    InputInitialMemoryConflict {
        /// Input name.
        input: String,
        /// Conflicting slot.
        slot: u8,
    },
    /// Two `let` bindings reused the same name or collided with an input.
    #[error("duplicate symbolic binding name `{name}`")]
    DuplicateBindingName {
        /// Duplicate binding name.
        name: String,
    },
    /// One operand referenced an unknown symbolic input or binding.
    #[error("unknown symbolic value `{name}`")]
    UnknownValue {
        /// Missing name.
        name: String,
    },
    /// One memory slot sat outside the declared runtime memory image.
    #[error("symbolic memory slot {slot} is out of range for memory_slots={memory_slots}")]
    InvalidMemorySlot {
        /// Invalid slot.
        slot: u8,
        /// Declared memory slot count.
        memory_slots: usize,
    },
    /// The symbolic program declared more memory than the current runtime slot encoding can lower.
    #[error(
        "symbolic program declares memory_slots={memory_slots}, which exceeds the current u8 lowering range"
    )]
    MemorySlotsExceedLoweringRange {
        /// Declared runtime memory slot count.
        memory_slots: usize,
    },
    /// The symbolic program declared more bindings than the current runtime local encoding can lower.
    #[error(
        "symbolic program declares {binding_count} bindings, which exceeds the current u8 lowering range"
    )]
    TooManyBindingsForLowering {
        /// Number of symbolic `let` bindings.
        binding_count: usize,
    },
    /// One required input assignment was missing during projection/evaluation.
    #[error("missing symbolic input assignment for `{input}`")]
    MissingInputAssignment {
        /// Missing input name.
        input: String,
    },
    /// One provided input assignment did not belong to the program.
    #[error("unknown symbolic input assignment `{input}`")]
    UnknownInputAssignment {
        /// Undeclared input name.
        input: String,
    },
}

/// Returns the public seeded symbolic examples for the bounded compile-exact lane.
#[must_use]
pub fn tassadar_symbolic_program_examples() -> Vec<TassadarSymbolicProgramExample> {
    let cases = vec![
        (
            "addition_pair",
            "straight-line bounded addition over two memory-backed inputs",
            ADDITION_PROGRAM,
            input_assignments(&[("lhs", 19), ("rhs", 23)]),
            "bounded straight-line symbolic lowering covers arithmetic and memory-backed inputs only; this example does not imply control-flow, subroutine, or arbitrary Wasm closure",
        ),
        (
            "parity_two_bits",
            "two-bit parity over a bounded symbolic arithmetic/comparison subset",
            PARITY_PROGRAM,
            input_assignments(&[("bit0", 1), ("bit1", 0)]),
            "bounded straight-line symbolic lowering covers a fixed-width parity subset using arithmetic and comparison only; it does not imply general Boolean circuit or loop closure",
        ),
        (
            "memory_accumulator",
            "bounded memory read-write accumulator over one seeded constant cell",
            MEMORY_ACCUMULATOR_PROGRAM,
            input_assignments(&[("value", 7)]),
            "bounded straight-line symbolic lowering covers explicit memory-slot reads and writes on the declared subset only; it does not imply wider memory-model closure",
        ),
        (
            "finite_state_counter",
            "bounded finite-state counter transition with explicit state update",
            FINITE_STATE_COUNTER_PROGRAM,
            input_assignments(&[("state", 1)]),
            "bounded straight-line symbolic lowering covers one explicit finite-state transition over declared state slots only; it does not imply general automata closure, loops, or arbitrary control flow",
        ),
        (
            "stack_machine_add_step",
            "bounded stack-machine add step over the top two stack cells",
            STACK_MACHINE_ADD_STEP_PROGRAM,
            input_assignments(&[("stack_top", 5), ("stack_next", 7), ("stack_pointer", 2)]),
            "bounded straight-line symbolic lowering covers one explicit stack-machine step over declared stack cells only; it does not imply general subroutine, recursion, or arbitrary stack semantics",
        ),
    ];

    cases
        .into_iter()
        .map(
            |(case_id, summary, source, input_assignments, claim_boundary)| {
                let program = TassadarSymbolicProgram::parse(source)
                    .expect("seeded symbolic example should parse");
                let execution = program
                    .evaluate(&input_assignments)
                    .expect("seeded symbolic example should evaluate");
                TassadarSymbolicProgramExample {
                    case_id: String::from(case_id),
                    summary: String::from(summary),
                    claim_class: String::from(TASSADAR_SYMBOLIC_PROGRAM_CLAIM_CLASS),
                    claim_boundary: String::from(claim_boundary),
                    program,
                    input_assignments,
                    expected_outputs: execution.outputs,
                    expected_final_memory: execution.final_memory,
                }
            },
        )
        .collect()
}

const ADDITION_PROGRAM: &str = r#"
program tassadar.symbolic.addition_pair.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 2
input lhs = slot(0)
input rhs = slot(1)
let sum = add(lhs, rhs)
output sum
"#;

const PARITY_PROGRAM: &str = r#"
program tassadar.symbolic.parity_two_bits.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 2
input bit0 = slot(0)
input bit1 = slot(1)
let sum = add(bit0, bit1)
let lt_two = lt(sum, const(2))
let lt_one = lt(sum, const(1))
let parity = sub(lt_two, lt_one)
output parity
"#;

const MEMORY_ACCUMULATOR_PROGRAM: &str = r#"
program tassadar.symbolic.memory_accumulator.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 3
input value = slot(0)
init slot(2) = 10
let total = add(value, slot(2))
store slot(2) = total
output total
"#;

const FINITE_STATE_COUNTER_PROGRAM: &str = r#"
program tassadar.symbolic.finite_state_counter.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 1
input state = slot(0)
let can_advance = lt(state, const(2))
let next_state = add(state, can_advance)
store slot(0) = next_state
output next_state
"#;

const STACK_MACHINE_ADD_STEP_PROGRAM: &str = r#"
program tassadar.symbolic.stack_machine_add_step.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 3
input stack_top = slot(0)
input stack_next = slot(1)
input stack_pointer = slot(2)
let summed = add(stack_top, stack_next)
let next_stack_pointer = sub(stack_pointer, const(1))
store slot(0) = summed
store slot(1) = const(0)
store slot(2) = next_stack_pointer
output summed
"#;

fn input_assignments(pairs: &[(&str, i32)]) -> BTreeMap<String, i32> {
    pairs
        .iter()
        .map(|(name, value)| (String::from(*name), *value))
        .collect()
}

fn push_operand_opcodes<'a>(
    opcodes: &mut BTreeSet<TassadarSymbolicLoweringOpcode>,
    operands: impl Iterator<Item = &'a TassadarSymbolicOperand>,
) {
    for operand in operands {
        match operand {
            TassadarSymbolicOperand::Name { .. } => {
                opcodes.insert(TassadarSymbolicLoweringOpcode::LocalGet);
                opcodes.insert(TassadarSymbolicLoweringOpcode::I32Load);
            }
            TassadarSymbolicOperand::Const { .. } => {
                opcodes.insert(TassadarSymbolicLoweringOpcode::I32Const);
            }
            TassadarSymbolicOperand::MemorySlot { .. } => {
                opcodes.insert(TassadarSymbolicLoweringOpcode::I32Load);
            }
        }
    }
}

fn parse_input(
    line_number: usize,
    line: &str,
) -> Result<TassadarSymbolicInput, TassadarSymbolicProgramError> {
    let body = line.trim_start_matches("input ").trim();
    let (name, binding) =
        body.split_once('=')
            .ok_or_else(|| TassadarSymbolicProgramError::InvalidDirective {
                line: line_number,
                directive: line.to_string(),
            })?;
    let name = name.trim();
    ensure_identifier(name)?;
    let slot = parse_slot_operand(line_number, binding.trim())?;
    Ok(TassadarSymbolicInput {
        name: String::from(name),
        memory_slot: slot,
    })
}

fn parse_init_cell(
    line_number: usize,
    line: &str,
) -> Result<TassadarSymbolicMemoryCell, TassadarSymbolicProgramError> {
    let body = line.trim_start_matches("init ").trim();
    let (slot, value) =
        body.split_once('=')
            .ok_or_else(|| TassadarSymbolicProgramError::InvalidDirective {
                line: line_number,
                directive: line.to_string(),
            })?;
    Ok(TassadarSymbolicMemoryCell {
        slot: parse_slot_operand(line_number, slot.trim())?,
        value: parse_i32(line_number, value.trim())?,
    })
}

fn parse_let_statement(
    line_number: usize,
    line: &str,
) -> Result<TassadarSymbolicStatement, TassadarSymbolicProgramError> {
    let body = line.trim_start_matches("let ").trim();
    let (name, expr) =
        body.split_once('=')
            .ok_or_else(|| TassadarSymbolicProgramError::InvalidDirective {
                line: line_number,
                directive: line.to_string(),
            })?;
    let name = name.trim();
    ensure_identifier(name)?;
    Ok(TassadarSymbolicStatement::Let {
        name: String::from(name),
        expr: parse_expr(line_number, expr.trim())?,
    })
}

fn parse_store_statement(
    line_number: usize,
    line: &str,
) -> Result<TassadarSymbolicStatement, TassadarSymbolicProgramError> {
    let body = line.trim_start_matches("store ").trim();
    let (slot, value) =
        body.split_once('=')
            .ok_or_else(|| TassadarSymbolicProgramError::InvalidDirective {
                line: line_number,
                directive: line.to_string(),
            })?;
    Ok(TassadarSymbolicStatement::Store {
        slot: parse_slot_operand(line_number, slot.trim())?,
        value: parse_operand(line_number, value.trim())?,
    })
}

fn parse_output_statement(
    line_number: usize,
    line: &str,
) -> Result<TassadarSymbolicStatement, TassadarSymbolicProgramError> {
    let value = line.trim_start_matches("output ").trim();
    Ok(TassadarSymbolicStatement::Output {
        value: parse_operand(line_number, value)?,
    })
}

fn parse_expr(
    line_number: usize,
    expression: &str,
) -> Result<TassadarSymbolicExpr, TassadarSymbolicProgramError> {
    for (operator_name, operator) in [
        ("add", TassadarSymbolicBinaryOp::Add),
        ("sub", TassadarSymbolicBinaryOp::Sub),
        ("mul", TassadarSymbolicBinaryOp::Mul),
        ("lt", TassadarSymbolicBinaryOp::Lt),
    ] {
        if let Some(inner) = expression
            .strip_prefix(operator_name)
            .and_then(|rest| rest.trim().strip_prefix('('))
            .and_then(|rest| rest.strip_suffix(')'))
        {
            let (left, right) = split_binary_operands(line_number, expression, inner)?;
            return Ok(TassadarSymbolicExpr::Binary {
                op: operator,
                left: parse_operand(line_number, left)?,
                right: parse_operand(line_number, right)?,
            });
        }
    }

    Ok(TassadarSymbolicExpr::Operand {
        operand: parse_operand(line_number, expression)?,
    })
}

fn split_binary_operands<'a>(
    line_number: usize,
    full_expression: &str,
    inner: &'a str,
) -> Result<(&'a str, &'a str), TassadarSymbolicProgramError> {
    let mut depth = 0_i32;
    for (index, character) in inner.char_indices() {
        match character {
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let left = inner[..index].trim();
                let right = inner[index + 1..].trim();
                if left.is_empty() || right.is_empty() {
                    break;
                }
                return Ok((left, right));
            }
            _ => {}
        }
    }
    Err(TassadarSymbolicProgramError::InvalidExpression {
        line: line_number,
        expression: full_expression.to_string(),
    })
}

fn parse_operand(
    line_number: usize,
    operand: &str,
) -> Result<TassadarSymbolicOperand, TassadarSymbolicProgramError> {
    if let Some(inner) = operand
        .strip_prefix("const(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return Ok(TassadarSymbolicOperand::Const {
            value: parse_i32(line_number, inner.trim())?,
        });
    }
    if operand.starts_with("slot(") {
        return Ok(TassadarSymbolicOperand::MemorySlot {
            slot: parse_slot_operand(line_number, operand)?,
        });
    }
    ensure_identifier(operand)?;
    Ok(TassadarSymbolicOperand::Name {
        name: operand.to_string(),
    })
}

fn parse_slot_operand(
    line_number: usize,
    operand: &str,
) -> Result<u8, TassadarSymbolicProgramError> {
    let Some(inner) = operand
        .trim()
        .strip_prefix("slot(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return Err(TassadarSymbolicProgramError::InvalidExpression {
            line: line_number,
            expression: operand.to_string(),
        });
    };
    inner
        .trim()
        .parse::<u8>()
        .map_err(|_| TassadarSymbolicProgramError::InvalidInteger {
            line: line_number,
            value: inner.trim().to_string(),
        })
}

fn parse_i32(line_number: usize, value: &str) -> Result<i32, TassadarSymbolicProgramError> {
    value
        .parse::<i32>()
        .map_err(|_| TassadarSymbolicProgramError::InvalidInteger {
            line: line_number,
            value: value.to_string(),
        })
}

fn parse_usize(line_number: usize, value: &str) -> Result<usize, TassadarSymbolicProgramError> {
    value
        .parse::<usize>()
        .map_err(|_| TassadarSymbolicProgramError::InvalidInteger {
            line: line_number,
            value: value.to_string(),
        })
}

fn validate_operand(
    operand: &TassadarSymbolicOperand,
    input_names: &BTreeSet<String>,
    bindings: &BTreeSet<String>,
    memory_slots: usize,
) -> Result<(), TassadarSymbolicProgramError> {
    match operand {
        TassadarSymbolicOperand::Name { name } => {
            if !input_names.contains(name) && !bindings.contains(name) {
                return Err(TassadarSymbolicProgramError::UnknownValue { name: name.clone() });
            }
        }
        TassadarSymbolicOperand::Const { .. } => {}
        TassadarSymbolicOperand::MemorySlot { slot } => {
            if usize::from(*slot) >= memory_slots {
                return Err(TassadarSymbolicProgramError::InvalidMemorySlot {
                    slot: *slot,
                    memory_slots,
                });
            }
        }
    }
    Ok(())
}

fn resolve_operand(
    operand: &TassadarSymbolicOperand,
    input_assignments: &BTreeMap<String, i32>,
    bindings: &BTreeMap<String, i32>,
    memory: &[i32],
) -> Result<i32, TassadarSymbolicProgramError> {
    match operand {
        TassadarSymbolicOperand::Name { name } => bindings
            .get(name)
            .copied()
            .or_else(|| input_assignments.get(name).copied())
            .ok_or_else(|| TassadarSymbolicProgramError::UnknownValue { name: name.clone() }),
        TassadarSymbolicOperand::Const { value } => Ok(*value),
        TassadarSymbolicOperand::MemorySlot { slot } => memory
            .get(usize::from(*slot))
            .copied()
            .ok_or(TassadarSymbolicProgramError::InvalidMemorySlot {
                slot: *slot,
                memory_slots: memory.len(),
            }),
    }
}

fn ensure_identifier(identifier: &str) -> Result<(), TassadarSymbolicProgramError> {
    let mut characters = identifier.chars();
    let Some(first) = characters.next() else {
        return Err(TassadarSymbolicProgramError::InvalidIdentifier {
            identifier: String::from(identifier),
        });
    };
    if !(first.is_ascii_alphabetic() || first == '_')
        || !characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(TassadarSymbolicProgramError::InvalidIdentifier {
            identifier: String::from(identifier),
        });
    }
    Ok(())
}

fn strip_comment(line: &str) -> &str {
    line.split_once('#').map_or(line, |(prefix, _)| prefix)
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
        TASSADAR_SYMBOLIC_PROGRAM_CLAIM_CLASS, TASSADAR_SYMBOLIC_PROGRAM_LANGUAGE_ID,
        TassadarSymbolicProgram, TassadarSymbolicProgramError, tassadar_symbolic_program_examples,
    };

    #[test]
    fn symbolic_program_examples_parse_and_evaluate() {
        let examples = tassadar_symbolic_program_examples();
        assert_eq!(examples.len(), 5);

        for example in examples {
            assert_eq!(
                example.program.language_id,
                TASSADAR_SYMBOLIC_PROGRAM_LANGUAGE_ID
            );
            assert_eq!(example.claim_class, TASSADAR_SYMBOLIC_PROGRAM_CLAIM_CLASS);
            let execution = example
                .program
                .evaluate(&example.input_assignments)
                .expect("seeded example should evaluate");
            assert_eq!(execution.outputs, example.expected_outputs);
            assert_eq!(execution.final_memory, example.expected_final_memory);
        }
    }

    #[test]
    fn symbolic_program_parser_refuses_control_flow_statements() {
        let error = TassadarSymbolicProgram::parse(
            r#"
program tassadar.symbolic.unsupported.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 1
if bit0
"#,
        )
        .expect_err("control flow should stay explicit as unsupported");

        assert_eq!(
            error,
            TassadarSymbolicProgramError::UnsupportedStatement {
                line: 5,
                statement: String::from("if bit0"),
            }
        );
    }

    #[test]
    fn symbolic_program_requires_declared_inputs() {
        let program = TassadarSymbolicProgram::parse(
            r#"
program tassadar.symbolic.input_check.v1
profile tassadar.wasm.article_i32_compute.v1
memory_slots 1
input bit0 = slot(0)
output bit0
"#,
        )
        .expect("program should parse");
        let error = program
            .evaluate(&Default::default())
            .expect_err("missing inputs should stay explicit");
        assert_eq!(
            error,
            TassadarSymbolicProgramError::MissingInputAssignment {
                input: String::from("bit0"),
            }
        );
    }
}
