use std::collections::BTreeMap;

use psionic_runtime::{
    TassadarArithmeticOp, TassadarExecution, TassadarHaltReason, TassadarInstruction,
    TassadarProgram, TassadarTraceEvent, TassadarTraceStep, TassadarUnaryOp,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{TokenId, TokenSequence, TokenVocabulary, TokenizerBoundary};

const PAD_TOKEN: &str = "<pad>";
const BOS_TOKEN: &str = "<bos>";
const EOS_TOKEN: &str = "<eos>";
const UNKNOWN_TOKEN: &str = "<unk>";

const PROGRAM_TOKEN: &str = "<program>";
const TRACE_TOKEN: &str = "<trace>";
const HALT_TOKEN: &str = "<halt>";
const STEP_TOKEN: &str = "<step>";
const LIST_TOKEN: &str = "<list>";

const FIELD_LOCALS: &str = "<locals>";
const FIELD_MEMORY_SLOTS: &str = "<memory_slots>";
const FIELD_INITIAL_MEMORY: &str = "<initial_memory>";
const FIELD_STEP_INDEX: &str = "<step_index>";
const FIELD_PC: &str = "<pc>";
const FIELD_NEXT_PC: &str = "<next_pc>";
const FIELD_STACK_BEFORE: &str = "<stack_before>";
const FIELD_STACK_AFTER: &str = "<stack_after>";
const FIELD_LOCALS_AFTER: &str = "<locals_after>";
const FIELD_MEMORY_AFTER: &str = "<memory_after>";

const BOOL_FALSE: &str = "<bool_false>";
const BOOL_TRUE: &str = "<bool_true>";

const OP_I32_CONST: &str = "<op_i32_const>";
const OP_LOCAL_GET: &str = "<op_local_get>";
const OP_LOCAL_SET: &str = "<op_local_set>";
const OP_I32_ADD: &str = "<op_i32_add>";
const OP_I32_SUB: &str = "<op_i32_sub>";
const OP_I32_MUL: &str = "<op_i32_mul>";
const OP_I32_LT: &str = "<op_i32_lt>";
const OP_I32_LOAD: &str = "<op_i32_load>";
const OP_I32_STORE: &str = "<op_i32_store>";
const OP_BR_IF: &str = "<op_br_if>";
const OP_OUTPUT: &str = "<op_output>";
const OP_RETURN: &str = "<op_return>";
const OP_NOP: &str = "<op_nop>";
const OP_LOCAL_TEE: &str = "<op_local_tee>";
const OP_DROP: &str = "<op_drop>";
const OP_I32_EQZ: &str = "<op_i32_eqz>";
const OP_I32_EQ: &str = "<op_i32_eq>";
const OP_I32_NE: &str = "<op_i32_ne>";
const OP_I32_GT: &str = "<op_i32_gt>";
const OP_I32_LE: &str = "<op_i32_le>";
const OP_I32_GE: &str = "<op_i32_ge>";

const EVENT_CONST_PUSH: &str = "<event_const_push>";
const EVENT_LOCAL_GET: &str = "<event_local_get>";
const EVENT_LOCAL_SET: &str = "<event_local_set>";
const EVENT_BINARY_ADD: &str = "<event_binary_add>";
const EVENT_BINARY_SUB: &str = "<event_binary_sub>";
const EVENT_BINARY_MUL: &str = "<event_binary_mul>";
const EVENT_BINARY_LT: &str = "<event_binary_lt>";
const EVENT_LOAD: &str = "<event_load>";
const EVENT_STORE: &str = "<event_store>";
const EVENT_BRANCH: &str = "<event_branch>";
const EVENT_OUTPUT: &str = "<event_output>";
const EVENT_RETURN: &str = "<event_return>";
const EVENT_NOP: &str = "<event_nop>";
const EVENT_LOCAL_TEE: &str = "<event_local_tee>";
const EVENT_DROP: &str = "<event_drop>";
const EVENT_UNARY_EQZ: &str = "<event_unary_eqz>";
const EVENT_BINARY_EQ: &str = "<event_binary_eq>";
const EVENT_BINARY_NE: &str = "<event_binary_ne>";
const EVENT_BINARY_GT: &str = "<event_binary_gt>";
const EVENT_BINARY_LE: &str = "<event_binary_le>";
const EVENT_BINARY_GE: &str = "<event_binary_ge>";

const HALT_RETURNED: &str = "<halt_returned>";
const HALT_FELL_OFF_END: &str = "<halt_fell_off_end>";

const HUNGARIAN_V0_LOCAL_COUNT: u32 = 1;
const HUNGARIAN_V0_DIM: usize = 4;
const HUNGARIAN_V0_MEMORY_SLOTS: u32 = 21;
const HUNGARIAN_V0_OUTPUT_SLOT_BASE: usize = 16;
const HUNGARIAN_V0_BEST_COST_SLOT: usize = 20;
const HUNGARIAN_10X10_LOCAL_COUNT: u32 = 3;
const HUNGARIAN_10X10_MEMORY_SLOTS: u32 = 41;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TassadarWorkloadSpecificSupervisionScope {
    None,
    HungarianV0,
    Hungarian10x10,
}

/// Explicit symbolic target family for one tokenized Tassadar sequence dataset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSequenceTraceFamily {
    /// Canonical CPU-style append-only instruction trace.
    #[default]
    SequentialCpuReference,
    /// Research-only anti-diagonal Sudoku assignment wavefront.
    SudokuDiagonalWavefront,
    /// Research-only parallel Hungarian assignment frontier.
    HungarianAssignmentFrontier,
}

impl TassadarSequenceTraceFamily {
    /// Returns the stable trace-family label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SequentialCpuReference => "sequential_cpu_reference",
            Self::SudokuDiagonalWavefront => "sudoku_diagonal_wavefront",
            Self::HungarianAssignmentFrontier => "hungarian_assignment_frontier",
        }
    }

    /// Returns the stable dataset-suffix label for non-canonical families.
    #[must_use]
    pub const fn dataset_suffix(self) -> Option<&'static str> {
        match self {
            Self::SequentialCpuReference => None,
            Self::SudokuDiagonalWavefront => Some("sudoku_diagonal_wavefront"),
            Self::HungarianAssignmentFrontier => Some("hungarian_assignment_frontier"),
        }
    }

    /// Returns the honest reconstruction scope for the trace family.
    #[must_use]
    pub const fn reconstruction_scope(self) -> &'static str {
        match self {
            Self::SequentialCpuReference => "full_cpu_trace",
            Self::SudokuDiagonalWavefront | Self::HungarianAssignmentFrontier => {
                "final_outputs_only"
            }
        }
    }
}

/// Structural supervision family derived from the canonical Tassadar trace ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStructuralSupervisionFamily {
    /// Instruction-pointer state such as `pc` and `next_pc`.
    InstructionPointer,
    /// Conditional-branch state such as taken/not-taken outcomes.
    BranchOutcome,
    /// Stack-shape state that exposes stack-size deltas across steps.
    StackDelta,
    /// Memory-write state that exposes sparse memory diffs.
    MemoryDiff,
    /// Workload-specific structured state such as Hungarian dual variables.
    WorkloadSpecificState,
}

impl TassadarStructuralSupervisionFamily {
    /// Returns the stable family label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InstructionPointer => "instruction_pointer",
            Self::BranchOutcome => "branch_outcome",
            Self::StackDelta => "stack_delta",
            Self::MemoryDiff => "memory_diff",
            Self::WorkloadSpecificState => "workload_specific_state",
        }
    }
}

/// Aggregate target-token coverage for one structural supervision family set.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarStructuralSupervisionCoverage {
    /// Total target tokens observed in the covered slice.
    pub total_target_token_count: u32,
    /// Target tokens tagged as instruction-pointer state.
    pub instruction_pointer_token_count: u32,
    /// Target tokens tagged as branch-outcome state.
    pub branch_outcome_token_count: u32,
    /// Target tokens tagged as stack-delta state.
    pub stack_delta_token_count: u32,
    /// Target tokens tagged as memory-diff state.
    pub memory_diff_token_count: u32,
    /// Target tokens tagged as workload-specific state.
    pub workload_specific_state_token_count: u32,
}

impl TassadarStructuralSupervisionCoverage {
    fn increment(&mut self, family: TassadarStructuralSupervisionFamily) {
        match family {
            TassadarStructuralSupervisionFamily::InstructionPointer => {
                self.instruction_pointer_token_count =
                    self.instruction_pointer_token_count.saturating_add(1);
            }
            TassadarStructuralSupervisionFamily::BranchOutcome => {
                self.branch_outcome_token_count = self.branch_outcome_token_count.saturating_add(1);
            }
            TassadarStructuralSupervisionFamily::StackDelta => {
                self.stack_delta_token_count = self.stack_delta_token_count.saturating_add(1);
            }
            TassadarStructuralSupervisionFamily::MemoryDiff => {
                self.memory_diff_token_count = self.memory_diff_token_count.saturating_add(1);
            }
            TassadarStructuralSupervisionFamily::WorkloadSpecificState => {
                self.workload_specific_state_token_count =
                    self.workload_specific_state_token_count.saturating_add(1);
            }
        }
    }

    /// Adds another coverage summary into `self`.
    pub fn accumulate(&mut self, other: &Self) {
        self.total_target_token_count = self
            .total_target_token_count
            .saturating_add(other.total_target_token_count);
        self.instruction_pointer_token_count = self
            .instruction_pointer_token_count
            .saturating_add(other.instruction_pointer_token_count);
        self.branch_outcome_token_count = self
            .branch_outcome_token_count
            .saturating_add(other.branch_outcome_token_count);
        self.stack_delta_token_count = self
            .stack_delta_token_count
            .saturating_add(other.stack_delta_token_count);
        self.memory_diff_token_count = self
            .memory_diff_token_count
            .saturating_add(other.memory_diff_token_count);
        self.workload_specific_state_token_count = self
            .workload_specific_state_token_count
            .saturating_add(other.workload_specific_state_token_count);
    }
}

/// Tokenized executor example with explicit prompt/target boundaries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTokenizedExecutionSequence {
    /// Full ordered token sequence.
    pub sequence: TokenSequence,
    /// Prefix tokens that belong to the program prompt.
    pub prompt_token_count: usize,
    /// Tokens belonging to the predicted trace suffix.
    pub target_token_count: usize,
    /// Stable digest over the token ids.
    pub sequence_digest: String,
}

impl TassadarTokenizedExecutionSequence {
    fn new(sequence: TokenSequence, prompt_token_count: usize) -> Self {
        let target_token_count = sequence.len().saturating_sub(prompt_token_count);
        let sequence_digest = stable_digest(
            b"psionic_tassadar_tokenized_execution_sequence|",
            &sequence
                .as_slice()
                .iter()
                .map(|token| token.as_u32())
                .collect::<Vec<_>>(),
        );
        Self {
            sequence,
            prompt_token_count,
            target_token_count,
            sequence_digest,
        }
    }

    /// Returns token ids as raw `u32` values for data-package surfaces.
    #[must_use]
    pub fn token_ids_u32(&self) -> Vec<u32> {
        self.sequence
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect()
    }
}

/// Symbolic decode of one tokenized execution sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecodedSymbolicSequence {
    /// Ordered symbolic token strings.
    pub tokens: Vec<String>,
    /// Prompt/target boundary.
    pub prompt_token_count: usize,
}

/// Typed prompt surface reconstructed from the canonical article trace domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecodedArticleTracePrompt {
    /// Number of locals declared in the prompt.
    pub local_count: usize,
    /// Number of memory slots declared in the prompt.
    pub memory_slots: usize,
    /// Initial memory image emitted by the prompt.
    pub initial_memory: Vec<i32>,
    /// Ordered instruction stream declared in the prompt.
    pub instructions: Vec<TassadarInstruction>,
}

impl TassadarDecodedArticleTracePrompt {
    /// Materializes one runtime program with an explicit profile id.
    #[must_use]
    pub fn materialize_program(
        &self,
        program_id: impl Into<String>,
        profile_id: impl Into<String>,
    ) -> TassadarProgram {
        TassadarProgram {
            program_id: program_id.into(),
            profile_id: profile_id.into(),
            local_count: self.local_count,
            memory_slots: self.memory_slots,
            initial_memory: self.initial_memory.clone(),
            instructions: self.instructions.clone(),
        }
    }
}

/// Typed article trace sequence reconstructed from the canonical token domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarDecodedArticleTraceDomain {
    /// Program prompt reconstructed from the prompt prefix.
    pub prompt: TassadarDecodedArticleTracePrompt,
    /// Ordered append-only trace steps.
    pub steps: Vec<TassadarTraceStep>,
    /// Terminal halt reason.
    pub halt_reason: TassadarHaltReason,
    /// Prompt token count reconstructed from the trace-start boundary.
    pub prompt_token_count: usize,
    /// Target token count reconstructed from the append-only suffix.
    pub target_token_count: usize,
    /// Stable digest over the original token sequence.
    pub sequence_digest: String,
}

impl TassadarDecodedArticleTraceDomain {
    /// Materializes one runtime program with an explicit profile id.
    #[must_use]
    pub fn materialize_program(
        &self,
        program_id: impl Into<String>,
        profile_id: impl Into<String>,
    ) -> TassadarProgram {
        self.prompt.materialize_program(program_id, profile_id)
    }

    /// Materializes one runtime execution with explicit identity fields.
    #[must_use]
    pub fn materialize_execution(
        &self,
        program_id: impl Into<String>,
        profile_id: impl Into<String>,
        runner_id: impl Into<String>,
        trace_abi: psionic_runtime::TassadarTraceAbi,
    ) -> TassadarExecution {
        let outputs = self
            .steps
            .iter()
            .filter_map(|step| match step.event {
                TassadarTraceEvent::Output { value } => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        let final_locals = self
            .steps
            .last()
            .map(|step| step.locals_after.clone())
            .unwrap_or_else(|| vec![0; self.prompt.local_count]);
        let final_memory = self
            .steps
            .last()
            .map(|step| step.memory_after.clone())
            .unwrap_or_else(|| {
                let mut memory = self.prompt.initial_memory.clone();
                memory.resize(self.prompt.memory_slots, 0);
                memory
            });
        let final_stack = self
            .steps
            .last()
            .map(|step| step.stack_after.clone())
            .unwrap_or_default();
        TassadarExecution {
            program_id: program_id.into(),
            profile_id: profile_id.into(),
            runner_id: runner_id.into(),
            trace_abi,
            steps: self.steps.clone(),
            outputs,
            final_locals,
            final_memory,
            final_stack,
            halt_reason: self.halt_reason,
        }
    }
}

/// Typed decode failure for the canonical article trace token domain.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error_kind")]
pub enum TassadarArticleTraceDecodeError {
    #[error("unexpected end of sequence at token index {index} while parsing {context}")]
    UnexpectedEnd { index: usize, context: String },
    #[error("unexpected token at index {index}: expected {expected}, found {found}")]
    UnexpectedToken {
        index: usize,
        expected: String,
        found: String,
    },
    #[error(
        "decoded prompt token count mismatch: encoded {encoded_prompt_token_count}, actual {actual_prompt_token_count}"
    )]
    PromptBoundaryMismatch {
        encoded_prompt_token_count: usize,
        actual_prompt_token_count: usize,
    },
    #[error("failed to fit decoded immediate `{field}` value {value} into target type {target}")]
    ImmediateOutOfRange {
        field: String,
        value: u32,
        target: String,
    },
    #[error(
        "trailing tokens remain after decoding the canonical article trace domain starting at token index {start_index}"
    )]
    TrailingTokens { start_index: usize },
}

/// Deterministic byte-and-symbol tokenizer for Wasm program plus trace sequences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TassadarTraceTokenizer {
    vocabulary: TokenVocabulary,
    lookup: BTreeMap<String, TokenId>,
    byte_token_start: u32,
}

impl Default for TassadarTraceTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl TassadarTraceTokenizer {
    /// Creates the canonical token vocabulary for the current Tassadar trace lane.
    #[must_use]
    pub fn new() -> Self {
        let mut tokens = vec![
            String::from(PAD_TOKEN),
            String::from(BOS_TOKEN),
            String::from(EOS_TOKEN),
            String::from(UNKNOWN_TOKEN),
            String::from(PROGRAM_TOKEN),
            String::from(TRACE_TOKEN),
            String::from(HALT_TOKEN),
            String::from(STEP_TOKEN),
            String::from(LIST_TOKEN),
            String::from(FIELD_LOCALS),
            String::from(FIELD_MEMORY_SLOTS),
            String::from(FIELD_INITIAL_MEMORY),
            String::from(FIELD_STEP_INDEX),
            String::from(FIELD_PC),
            String::from(FIELD_NEXT_PC),
            String::from(FIELD_STACK_BEFORE),
            String::from(FIELD_STACK_AFTER),
            String::from(FIELD_LOCALS_AFTER),
            String::from(FIELD_MEMORY_AFTER),
            String::from(BOOL_FALSE),
            String::from(BOOL_TRUE),
            String::from(OP_I32_CONST),
            String::from(OP_LOCAL_GET),
            String::from(OP_LOCAL_SET),
            String::from(OP_I32_ADD),
            String::from(OP_I32_SUB),
            String::from(OP_I32_MUL),
            String::from(OP_I32_LT),
            String::from(OP_I32_LOAD),
            String::from(OP_I32_STORE),
            String::from(OP_BR_IF),
            String::from(OP_OUTPUT),
            String::from(OP_RETURN),
            String::from(EVENT_CONST_PUSH),
            String::from(EVENT_LOCAL_GET),
            String::from(EVENT_LOCAL_SET),
            String::from(EVENT_BINARY_ADD),
            String::from(EVENT_BINARY_SUB),
            String::from(EVENT_BINARY_MUL),
            String::from(EVENT_BINARY_LT),
            String::from(EVENT_LOAD),
            String::from(EVENT_STORE),
            String::from(EVENT_BRANCH),
            String::from(EVENT_OUTPUT),
            String::from(EVENT_RETURN),
            String::from(HALT_RETURNED),
            String::from(HALT_FELL_OFF_END),
        ];
        let byte_token_start = tokens.len() as u32;
        for value in 0_u16..=255 {
            tokens.push(format!("<byte_{value:02x}>"));
        }
        tokens.extend([
            String::from(OP_NOP),
            String::from(OP_LOCAL_TEE),
            String::from(OP_DROP),
            String::from(OP_I32_EQZ),
            String::from(OP_I32_EQ),
            String::from(OP_I32_NE),
            String::from(OP_I32_GT),
            String::from(OP_I32_LE),
            String::from(OP_I32_GE),
            String::from(EVENT_NOP),
            String::from(EVENT_LOCAL_TEE),
            String::from(EVENT_DROP),
            String::from(EVENT_UNARY_EQZ),
            String::from(EVENT_BINARY_EQ),
            String::from(EVENT_BINARY_NE),
            String::from(EVENT_BINARY_GT),
            String::from(EVENT_BINARY_LE),
            String::from(EVENT_BINARY_GE),
        ]);

        let vocabulary = TokenVocabulary::new(
            tokens.clone(),
            TokenId(0),
            TokenId(1),
            TokenId(2),
            TokenId(3),
        );
        let lookup = tokens
            .into_iter()
            .enumerate()
            .map(|(index, token)| (token, TokenId(index as u32)))
            .collect();
        Self {
            vocabulary,
            lookup,
            byte_token_start,
        }
    }

    /// Returns a stable digest over the tokenizer contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(
            b"psionic_tassadar_trace_tokenizer|",
            &self.vocabulary.tokens(),
        )
    }

    /// Tokenizes one validated program and CPU-reference execution trace.
    #[must_use]
    pub fn tokenize_program_and_execution(
        &self,
        program: &TassadarProgram,
        execution: &TassadarExecution,
    ) -> TassadarTokenizedExecutionSequence {
        let mut tokens = Vec::new();
        let prompt_token_count = self.push_program_prompt(&mut tokens, program);
        for step in &execution.steps {
            self.push_step(&mut tokens, step);
        }
        tokens.push(self.token_id(HALT_TOKEN));
        self.push_halt_reason(&mut tokens, execution.halt_reason);
        tokens.push(self.vocabulary.eos_id());
        TassadarTokenizedExecutionSequence::new(TokenSequence::new(tokens), prompt_token_count)
    }

    /// Tokenizes one Sudoku solution as an anti-diagonal assignment wavefront.
    #[must_use]
    pub fn tokenize_program_and_sudoku_diagonal_wavefront(
        &self,
        program: &TassadarProgram,
        solved_outputs: &[i32],
        grid_width: usize,
    ) -> TassadarTokenizedExecutionSequence {
        let mut tokens = Vec::new();
        let prompt_token_count = self.push_program_prompt(&mut tokens, program);
        let mut emitted_count = 0_u32;
        for wavefront_index in 0..grid_width.saturating_mul(2).saturating_sub(1) {
            let mut payload = Vec::new();
            for row in 0..grid_width {
                let Some(col) = wavefront_index.checked_sub(row) else {
                    continue;
                };
                if col >= grid_width {
                    continue;
                }
                let cell_index = row.saturating_mul(grid_width).saturating_add(col);
                let Some(value) = solved_outputs.get(cell_index) else {
                    continue;
                };
                payload.push(cell_index as i32);
                payload.push(*value);
            }
            emitted_count = emitted_count.saturating_add((payload.len() / 2) as u32);
            self.push_wavefront_step(
                &mut tokens,
                wavefront_index as u32,
                (payload.len() / 2) as u32,
                emitted_count,
                payload.as_slice(),
                emitted_count as i32,
            );
        }
        tokens.push(self.token_id(HALT_TOKEN));
        self.push_halt_reason(&mut tokens, TassadarHaltReason::Returned);
        tokens.push(self.vocabulary.eos_id());
        TassadarTokenizedExecutionSequence::new(TokenSequence::new(tokens), prompt_token_count)
    }

    /// Tokenizes one Hungarian optimum as a parallel assignment frontier.
    #[must_use]
    pub fn tokenize_program_and_hungarian_assignment_frontier(
        &self,
        program: &TassadarProgram,
        optimal_assignment: &[i32],
        cost_matrix: &[i32],
        optimal_cost: i32,
    ) -> TassadarTokenizedExecutionSequence {
        let mut tokens = Vec::new();
        let prompt_token_count = self.push_program_prompt(&mut tokens, program);
        let dimension = optimal_assignment.len();
        let mut payload = Vec::with_capacity(dimension.saturating_mul(3));
        for (row, column) in optimal_assignment.iter().enumerate() {
            let column = *column as usize;
            let row_cost = cost_matrix
                .get(row.saturating_mul(dimension).saturating_add(column))
                .copied()
                .unwrap_or_default();
            payload.push(row as i32);
            payload.push(column as i32);
            payload.push(row_cost);
        }
        self.push_wavefront_step(
            &mut tokens,
            0,
            dimension as u32,
            optimal_cost.max(0) as u32,
            payload.as_slice(),
            optimal_cost,
        );
        tokens.push(self.token_id(HALT_TOKEN));
        self.push_halt_reason(&mut tokens, TassadarHaltReason::Returned);
        tokens.push(self.vocabulary.eos_id());
        TassadarTokenizedExecutionSequence::new(TokenSequence::new(tokens), prompt_token_count)
    }

    /// Decodes token ids back into symbolic token strings with the same prompt boundary.
    #[must_use]
    pub fn decode_symbolic(
        &self,
        tokenized: &TassadarTokenizedExecutionSequence,
    ) -> TassadarDecodedSymbolicSequence {
        TassadarDecodedSymbolicSequence {
            tokens: tokenized
                .sequence
                .as_slice()
                .iter()
                .map(|token| {
                    self.vocabulary
                        .token(*token)
                        .unwrap_or(UNKNOWN_TOKEN)
                        .to_string()
                })
                .collect(),
            prompt_token_count: tokenized.prompt_token_count,
        }
    }

    /// Reconstructs the typed article trace domain from one tokenized sequence.
    pub fn decode_article_trace_domain(
        &self,
        tokenized: &TassadarTokenizedExecutionSequence,
    ) -> Result<TassadarDecodedArticleTraceDomain, TassadarArticleTraceDecodeError> {
        let tokens = tokenized.sequence.as_slice();
        let (prompt, mut index, prompt_token_count) = self.parse_program_prompt(tokens)?;
        if prompt_token_count != tokenized.prompt_token_count {
            return Err(TassadarArticleTraceDecodeError::PromptBoundaryMismatch {
                encoded_prompt_token_count: tokenized.prompt_token_count,
                actual_prompt_token_count: prompt_token_count,
            });
        }
        let mut steps = Vec::new();
        while tokens.get(index).copied() != Some(self.token_id(HALT_TOKEN)) {
            let (step, next_index) = self.parse_trace_step(tokens, index)?;
            steps.push(step);
            index = next_index;
        }
        self.expect_token(tokens, index, HALT_TOKEN)?;
        index = index.saturating_add(1);
        let halt_reason = self.parse_halt_reason(tokens, index)?;
        index = index.saturating_add(1);
        if tokens.get(index).copied() != Some(self.vocabulary.eos_id()) {
            return Err(self.unexpected_token(index, "<eos>", tokens.get(index).copied()));
        }
        index = index.saturating_add(1);
        if index != tokens.len() {
            return Err(TassadarArticleTraceDecodeError::TrailingTokens { start_index: index });
        }
        Ok(TassadarDecodedArticleTraceDomain {
            prompt,
            steps,
            halt_reason,
            prompt_token_count,
            target_token_count: tokenized.target_token_count,
            sequence_digest: tokenized.sequence_digest.clone(),
        })
    }

    /// Composes one full tokenized sequence from prompt and target token slices.
    #[must_use]
    pub fn compose_prompt_and_target_sequence(
        &self,
        prompt_tokens: &[TokenId],
        target_tokens: &[TokenId],
    ) -> TassadarTokenizedExecutionSequence {
        let mut sequence = Vec::with_capacity(prompt_tokens.len() + target_tokens.len());
        sequence.extend_from_slice(prompt_tokens);
        sequence.extend_from_slice(target_tokens);
        TassadarTokenizedExecutionSequence::new(TokenSequence::new(sequence), prompt_tokens.len())
    }

    /// Retokenizes one symbolic token-label text surface while preserving an
    /// explicit prompt/target split.
    #[must_use]
    pub fn retokenize_symbolic_text(
        &self,
        symbolic_text: &str,
        prompt_token_count: usize,
    ) -> TassadarTokenizedExecutionSequence {
        TassadarTokenizedExecutionSequence::new(
            TokenizerBoundary::encode(self, symbolic_text),
            prompt_token_count,
        )
    }

    /// Re-encodes prompt and target symbolic token-label text independently
    /// before composing the canonical prompt/target split.
    #[must_use]
    pub fn compose_prompt_and_target_symbolic_text(
        &self,
        prompt_text: &str,
        target_text: &str,
    ) -> TassadarTokenizedExecutionSequence {
        let prompt_tokens = TokenizerBoundary::encode(self, prompt_text);
        let target_tokens = TokenizerBoundary::encode(self, target_text);
        self.compose_prompt_and_target_sequence(prompt_tokens.as_slice(), target_tokens.as_slice())
    }

    /// Extracts output values from one symbolic executor trace.
    #[must_use]
    pub fn extract_output_values(&self, tokens: &[TokenId]) -> Vec<i32> {
        let mut outputs = Vec::new();
        let output_token = self.token_id(EVENT_OUTPUT);
        let mut index = 0_usize;
        while index < tokens.len() {
            if tokens[index] == output_token {
                if let Some(value) = self.parse_i32(tokens, index + 1) {
                    outputs.push(value);
                }
                index = index.saturating_add(5);
                continue;
            }
            index = index.saturating_add(1);
        }
        outputs
    }

    /// Extracts the final halt marker from one symbolic executor trace.
    #[must_use]
    pub fn extract_halt_marker(&self, tokens: &[TokenId]) -> Option<String> {
        let halt_token = self.token_id(HALT_TOKEN);
        for window in tokens.windows(2) {
            if window[0] == halt_token {
                return self
                    .vocabulary
                    .token(window[1])
                    .map(std::string::ToString::to_string);
            }
        }
        None
    }

    /// Reconstructs final Sudoku outputs from one anti-diagonal wavefront trace.
    #[must_use]
    pub fn extract_sudoku_diagonal_wavefront_outputs(
        &self,
        tokens: &[TokenId],
        prompt_token_count: usize,
        cell_count: usize,
    ) -> Vec<i32> {
        let mut outputs = vec![0; cell_count];
        let mut index = prompt_token_count;
        while let Some(step) = self.parse_wavefront_step(tokens, index) {
            if step.stack_after.len() % 2 == 0 {
                for pair in step.stack_after.chunks_exact(2) {
                    let cell_index = pair[0].max(0) as usize;
                    if cell_index < outputs.len() {
                        outputs[cell_index] = pair[1];
                    }
                }
            }
            index = step.next_index;
        }
        outputs
    }

    /// Reconstructs final Hungarian outputs from one parallel assignment frontier trace.
    #[must_use]
    pub fn extract_hungarian_assignment_frontier_outputs(
        &self,
        tokens: &[TokenId],
        prompt_token_count: usize,
        dimension: usize,
    ) -> Vec<i32> {
        let mut assignments = vec![0; dimension];
        let mut total_cost = 0_i32;
        let mut index = prompt_token_count;
        while let Some(step) = self.parse_wavefront_step(tokens, index) {
            if let Some(summary_output) = step.summary_output {
                total_cost = summary_output;
            }
            if step.stack_after.len() % 3 == 0 {
                for triple in step.stack_after.chunks_exact(3) {
                    let row = triple[0].max(0) as usize;
                    if row < assignments.len() {
                        assignments[row] = triple[1];
                    }
                }
            }
            index = step.next_index;
        }
        assignments.push(total_cost);
        assignments
    }

    /// Classifies each target token into zero or more structural supervision families.
    #[must_use]
    pub fn classify_target_structural_supervision(
        &self,
        tokens: &[TokenId],
        prompt_token_count: usize,
    ) -> Vec<Vec<TassadarStructuralSupervisionFamily>> {
        let mut families = vec![Vec::new(); tokens.len().saturating_sub(prompt_token_count)];
        let workload_scope =
            self.detect_workload_specific_supervision_scope(tokens, prompt_token_count);
        let mut index = prompt_token_count;
        let halt_token = self.token_id(HALT_TOKEN);
        let eos_token = self.vocabulary.eos_id();
        let step_token = self.token_id(STEP_TOKEN);
        while index < tokens.len() {
            let token = tokens[index];
            if token == halt_token || token == eos_token {
                break;
            }
            if token != step_token {
                index = index.saturating_add(1);
                continue;
            }
            index = index.saturating_add(1);
            index = self.consume_field_u32(
                tokens,
                prompt_token_count,
                index,
                FIELD_STEP_INDEX,
                None,
                &mut families,
            );
            index = self.consume_field_u32(
                tokens,
                prompt_token_count,
                index,
                FIELD_PC,
                Some(TassadarStructuralSupervisionFamily::InstructionPointer),
                &mut families,
            );
            index = self.consume_field_u32(
                tokens,
                prompt_token_count,
                index,
                FIELD_NEXT_PC,
                Some(TassadarStructuralSupervisionFamily::InstructionPointer),
                &mut families,
            );
            index = self.consume_instruction(tokens, index);
            let memory_write_slot =
                self.consume_event(tokens, prompt_token_count, index, &mut families);
            index = memory_write_slot.0;
            index = self.consume_stack_list(
                tokens,
                prompt_token_count,
                index,
                FIELD_STACK_BEFORE,
                &mut families,
            );
            index = self.consume_stack_list(
                tokens,
                prompt_token_count,
                index,
                FIELD_STACK_AFTER,
                &mut families,
            );
            index = self.consume_locals_after_list(
                tokens,
                prompt_token_count,
                index,
                workload_scope,
                &mut families,
            );
            index = self.consume_memory_after_list(
                tokens,
                prompt_token_count,
                index,
                memory_write_slot.1,
                workload_scope,
                &mut families,
            );
        }
        families
    }

    /// Summarizes structural-supervision coverage for one token sequence.
    #[must_use]
    pub fn summarize_target_structural_supervision(
        &self,
        tokens: &[TokenId],
        prompt_token_count: usize,
    ) -> TassadarStructuralSupervisionCoverage {
        let families = self.classify_target_structural_supervision(tokens, prompt_token_count);
        let mut coverage = TassadarStructuralSupervisionCoverage {
            total_target_token_count: families.len() as u32,
            ..TassadarStructuralSupervisionCoverage::default()
        };
        for token_families in families {
            for family in token_families {
                coverage.increment(family);
            }
        }
        coverage
    }

    fn push_step(&self, tokens: &mut Vec<TokenId>, step: &TassadarTraceStep) {
        tokens.push(self.token_id(STEP_TOKEN));
        tokens.push(self.token_id(FIELD_STEP_INDEX));
        self.push_u32(tokens, step.step_index as u32);
        tokens.push(self.token_id(FIELD_PC));
        self.push_u32(tokens, step.pc as u32);
        tokens.push(self.token_id(FIELD_NEXT_PC));
        self.push_u32(tokens, step.next_pc as u32);
        self.push_instruction(tokens, &step.instruction);
        self.push_event(tokens, &step.event);
        self.push_i32_list(tokens, FIELD_STACK_BEFORE, &step.stack_before);
        self.push_i32_list(tokens, FIELD_STACK_AFTER, &step.stack_after);
        self.push_i32_list(tokens, FIELD_LOCALS_AFTER, &step.locals_after);
        self.push_i32_list(tokens, FIELD_MEMORY_AFTER, &step.memory_after);
    }

    fn push_program_prompt(&self, tokens: &mut Vec<TokenId>, program: &TassadarProgram) -> usize {
        tokens.push(self.vocabulary.bos_id());
        tokens.push(self.token_id(PROGRAM_TOKEN));
        tokens.push(self.token_id(FIELD_LOCALS));
        self.push_u32(tokens, program.local_count as u32);
        tokens.push(self.token_id(FIELD_MEMORY_SLOTS));
        self.push_u32(tokens, program.memory_slots as u32);
        tokens.push(self.token_id(FIELD_INITIAL_MEMORY));
        self.push_u32(tokens, program.initial_memory.len() as u32);
        for value in &program.initial_memory {
            self.push_i32(tokens, *value);
        }
        for instruction in &program.instructions {
            self.push_instruction(tokens, instruction);
        }
        tokens.push(self.token_id(TRACE_TOKEN));
        tokens.len()
    }

    fn push_wavefront_step(
        &self,
        tokens: &mut Vec<TokenId>,
        step_index: u32,
        payload_item_count: u32,
        summary_index: u32,
        payload: &[i32],
        summary_output: i32,
    ) {
        tokens.push(self.token_id(STEP_TOKEN));
        tokens.push(self.token_id(FIELD_STEP_INDEX));
        self.push_u32(tokens, step_index);
        tokens.push(self.token_id(FIELD_PC));
        self.push_u32(tokens, payload_item_count);
        tokens.push(self.token_id(FIELD_NEXT_PC));
        self.push_u32(tokens, summary_index);
        tokens.push(self.token_id(OP_OUTPUT));
        tokens.push(self.token_id(EVENT_OUTPUT));
        self.push_i32(tokens, summary_output);
        self.push_i32_list(tokens, FIELD_STACK_BEFORE, &[]);
        self.push_i32_list(tokens, FIELD_STACK_AFTER, payload);
        self.push_i32_list(tokens, FIELD_LOCALS_AFTER, &[]);
        self.push_i32_list(tokens, FIELD_MEMORY_AFTER, &[]);
    }

    fn push_instruction(&self, tokens: &mut Vec<TokenId>, instruction: &TassadarInstruction) {
        match instruction {
            TassadarInstruction::Nop => tokens.push(self.token_id(OP_NOP)),
            TassadarInstruction::I32Const { value } => {
                tokens.push(self.token_id(OP_I32_CONST));
                self.push_i32(tokens, *value);
            }
            TassadarInstruction::LocalGet { local } => {
                tokens.push(self.token_id(OP_LOCAL_GET));
                self.push_u32(tokens, u32::from(*local));
            }
            TassadarInstruction::LocalSet { local } => {
                tokens.push(self.token_id(OP_LOCAL_SET));
                self.push_u32(tokens, u32::from(*local));
            }
            TassadarInstruction::LocalTee { local } => {
                tokens.push(self.token_id(OP_LOCAL_TEE));
                self.push_u32(tokens, u32::from(*local));
            }
            TassadarInstruction::Drop => tokens.push(self.token_id(OP_DROP)),
            TassadarInstruction::I32Add => tokens.push(self.token_id(OP_I32_ADD)),
            TassadarInstruction::I32Sub => tokens.push(self.token_id(OP_I32_SUB)),
            TassadarInstruction::I32Mul => tokens.push(self.token_id(OP_I32_MUL)),
            TassadarInstruction::I32Lt => tokens.push(self.token_id(OP_I32_LT)),
            TassadarInstruction::I32Eqz => tokens.push(self.token_id(OP_I32_EQZ)),
            TassadarInstruction::I32Eq => tokens.push(self.token_id(OP_I32_EQ)),
            TassadarInstruction::I32Ne => tokens.push(self.token_id(OP_I32_NE)),
            TassadarInstruction::I32Gt => tokens.push(self.token_id(OP_I32_GT)),
            TassadarInstruction::I32Le => tokens.push(self.token_id(OP_I32_LE)),
            TassadarInstruction::I32Ge => tokens.push(self.token_id(OP_I32_GE)),
            TassadarInstruction::I32Load { slot } => {
                tokens.push(self.token_id(OP_I32_LOAD));
                self.push_u32(tokens, u32::from(*slot));
            }
            TassadarInstruction::I32Store { slot } => {
                tokens.push(self.token_id(OP_I32_STORE));
                self.push_u32(tokens, u32::from(*slot));
            }
            TassadarInstruction::BrIf { target_pc } => {
                tokens.push(self.token_id(OP_BR_IF));
                self.push_u32(tokens, u32::from(*target_pc));
            }
            TassadarInstruction::Output => tokens.push(self.token_id(OP_OUTPUT)),
            TassadarInstruction::Return => tokens.push(self.token_id(OP_RETURN)),
        }
    }

    fn push_event(&self, tokens: &mut Vec<TokenId>, event: &TassadarTraceEvent) {
        match event {
            TassadarTraceEvent::Nop => tokens.push(self.token_id(EVENT_NOP)),
            TassadarTraceEvent::ConstPush { value } => {
                tokens.push(self.token_id(EVENT_CONST_PUSH));
                self.push_i32(tokens, *value);
            }
            TassadarTraceEvent::LocalGet { local, value } => {
                tokens.push(self.token_id(EVENT_LOCAL_GET));
                self.push_u32(tokens, u32::from(*local));
                self.push_i32(tokens, *value);
            }
            TassadarTraceEvent::LocalSet { local, value } => {
                tokens.push(self.token_id(EVENT_LOCAL_SET));
                self.push_u32(tokens, u32::from(*local));
                self.push_i32(tokens, *value);
            }
            TassadarTraceEvent::LocalTee { local, value } => {
                tokens.push(self.token_id(EVENT_LOCAL_TEE));
                self.push_u32(tokens, u32::from(*local));
                self.push_i32(tokens, *value);
            }
            TassadarTraceEvent::Drop { value } => {
                tokens.push(self.token_id(EVENT_DROP));
                self.push_i32(tokens, *value);
            }
            TassadarTraceEvent::UnaryOp {
                op,
                operand,
                result,
            } => {
                tokens.push(match op {
                    TassadarUnaryOp::Eqz => self.token_id(EVENT_UNARY_EQZ),
                });
                self.push_i32(tokens, *operand);
                self.push_i32(tokens, *result);
            }
            TassadarTraceEvent::BinaryOp {
                op,
                left,
                right,
                result,
            } => {
                tokens.push(self.binary_event_token_id(*op));
                self.push_i32(tokens, *left);
                self.push_i32(tokens, *right);
                self.push_i32(tokens, *result);
            }
            TassadarTraceEvent::Load { slot, value } => {
                tokens.push(self.token_id(EVENT_LOAD));
                self.push_u32(tokens, u32::from(*slot));
                self.push_i32(tokens, *value);
            }
            TassadarTraceEvent::Store { slot, value } => {
                tokens.push(self.token_id(EVENT_STORE));
                self.push_u32(tokens, u32::from(*slot));
                self.push_i32(tokens, *value);
            }
            TassadarTraceEvent::Branch {
                condition,
                taken,
                target_pc,
            } => {
                tokens.push(self.token_id(EVENT_BRANCH));
                self.push_i32(tokens, *condition);
                tokens.push(if *taken {
                    self.token_id(BOOL_TRUE)
                } else {
                    self.token_id(BOOL_FALSE)
                });
                self.push_u32(tokens, *target_pc as u32);
            }
            TassadarTraceEvent::Output { value } => {
                tokens.push(self.token_id(EVENT_OUTPUT));
                self.push_i32(tokens, *value);
            }
            TassadarTraceEvent::Return => tokens.push(self.token_id(EVENT_RETURN)),
        }
    }

    fn push_halt_reason(&self, tokens: &mut Vec<TokenId>, halt_reason: TassadarHaltReason) {
        tokens.push(match halt_reason {
            TassadarHaltReason::Returned => self.token_id(HALT_RETURNED),
            TassadarHaltReason::FellOffEnd => self.token_id(HALT_FELL_OFF_END),
        });
    }

    fn push_i32_list(&self, tokens: &mut Vec<TokenId>, field_token: &str, values: &[i32]) {
        tokens.push(self.token_id(field_token));
        tokens.push(self.token_id(LIST_TOKEN));
        self.push_u32(tokens, values.len() as u32);
        for value in values {
            self.push_i32(tokens, *value);
        }
    }

    fn push_u32(&self, tokens: &mut Vec<TokenId>, value: u32) {
        for byte in value.to_le_bytes() {
            tokens.push(self.byte_token(byte));
        }
    }

    fn push_i32(&self, tokens: &mut Vec<TokenId>, value: i32) {
        for byte in value.to_le_bytes() {
            tokens.push(self.byte_token(byte));
        }
    }

    fn token_id(&self, token: &str) -> TokenId {
        *self
            .lookup
            .get(token)
            .expect("Tassadar trace token should exist")
    }

    fn byte_token(&self, value: u8) -> TokenId {
        TokenId(self.byte_token_start + u32::from(value))
    }

    fn binary_event_token_id(&self, op: TassadarArithmeticOp) -> TokenId {
        match op {
            TassadarArithmeticOp::Add => self.token_id(EVENT_BINARY_ADD),
            TassadarArithmeticOp::Sub => self.token_id(EVENT_BINARY_SUB),
            TassadarArithmeticOp::Mul => self.token_id(EVENT_BINARY_MUL),
            TassadarArithmeticOp::Lt => self.token_id(EVENT_BINARY_LT),
            TassadarArithmeticOp::Eq => self.token_id(EVENT_BINARY_EQ),
            TassadarArithmeticOp::Ne => self.token_id(EVENT_BINARY_NE),
            TassadarArithmeticOp::Gt => self.token_id(EVENT_BINARY_GT),
            TassadarArithmeticOp::Le => self.token_id(EVENT_BINARY_LE),
            TassadarArithmeticOp::Ge => self.token_id(EVENT_BINARY_GE),
        }
    }

    fn expect_token(
        &self,
        tokens: &[TokenId],
        index: usize,
        expected: &str,
    ) -> Result<(), TassadarArticleTraceDecodeError> {
        let Some(token) = tokens.get(index).copied() else {
            return Err(TassadarArticleTraceDecodeError::UnexpectedEnd {
                index,
                context: String::from(expected),
            });
        };
        if token == self.token_id(expected) {
            return Ok(());
        }
        Err(self.unexpected_token(index, expected, Some(token)))
    }

    fn unexpected_token(
        &self,
        index: usize,
        expected: &str,
        found: Option<TokenId>,
    ) -> TassadarArticleTraceDecodeError {
        TassadarArticleTraceDecodeError::UnexpectedToken {
            index,
            expected: String::from(expected),
            found: self.token_label(found),
        }
    }

    fn token_label(&self, token: Option<TokenId>) -> String {
        token
            .and_then(|value| self.vocabulary.token(value))
            .unwrap_or("<end>")
            .to_string()
    }

    fn parse_program_prompt(
        &self,
        tokens: &[TokenId],
    ) -> Result<(TassadarDecodedArticleTracePrompt, usize, usize), TassadarArticleTraceDecodeError>
    {
        if tokens.first().copied() != Some(self.vocabulary.bos_id()) {
            return Err(TassadarArticleTraceDecodeError::UnexpectedToken {
                index: 0,
                expected: String::from("<bos>"),
                found: self.token_label(tokens.first().copied()),
            });
        }
        let mut index = 1_usize;
        self.expect_token(tokens, index, PROGRAM_TOKEN)?;
        index = index.saturating_add(1);
        self.expect_token(tokens, index, FIELD_LOCALS)?;
        let local_count = self.parse_u32_required(tokens, index + 1, "locals")? as usize;
        index = index.saturating_add(5);
        self.expect_token(tokens, index, FIELD_MEMORY_SLOTS)?;
        let memory_slots = self.parse_u32_required(tokens, index + 1, "memory_slots")? as usize;
        index = index.saturating_add(5);
        self.expect_token(tokens, index, FIELD_INITIAL_MEMORY)?;
        let initial_memory_len =
            self.parse_u32_required(tokens, index + 1, "initial_memory_length")? as usize;
        index = index.saturating_add(5);
        let mut initial_memory = Vec::with_capacity(initial_memory_len);
        for memory_index in 0..initial_memory_len {
            initial_memory.push(self.parse_i32_required(
                tokens,
                index,
                &format!("initial_memory[{memory_index}]"),
            )?);
            index = index.saturating_add(4);
        }
        let mut instructions = Vec::new();
        while tokens.get(index).copied() != Some(self.token_id(TRACE_TOKEN)) {
            let (instruction, next_index) = self.parse_instruction_required(tokens, index)?;
            instructions.push(instruction);
            index = next_index;
        }
        self.expect_token(tokens, index, TRACE_TOKEN)?;
        index = index.saturating_add(1);
        Ok((
            TassadarDecodedArticleTracePrompt {
                local_count,
                memory_slots,
                initial_memory,
                instructions,
            },
            index,
            index,
        ))
    }

    fn parse_trace_step(
        &self,
        tokens: &[TokenId],
        index: usize,
    ) -> Result<(TassadarTraceStep, usize), TassadarArticleTraceDecodeError> {
        let mut index = index;
        self.expect_token(tokens, index, STEP_TOKEN)?;
        index = index.saturating_add(1);
        self.expect_token(tokens, index, FIELD_STEP_INDEX)?;
        let step_index = self.parse_u32_required(tokens, index + 1, "step_index")? as usize;
        index = index.saturating_add(5);
        self.expect_token(tokens, index, FIELD_PC)?;
        let pc = self.parse_u32_required(tokens, index + 1, "pc")? as usize;
        index = index.saturating_add(5);
        self.expect_token(tokens, index, FIELD_NEXT_PC)?;
        let next_pc = self.parse_u32_required(tokens, index + 1, "next_pc")? as usize;
        index = index.saturating_add(5);
        let (instruction, next_instruction_index) =
            self.parse_instruction_required(tokens, index)?;
        index = next_instruction_index;
        let (event, next_event_index) = self.parse_event_required(tokens, index)?;
        index = next_event_index;
        let (stack_before, next_stack_before_index) =
            self.parse_i32_list_required(tokens, index, FIELD_STACK_BEFORE)?;
        index = next_stack_before_index;
        let (stack_after, next_stack_after_index) =
            self.parse_i32_list_required(tokens, index, FIELD_STACK_AFTER)?;
        index = next_stack_after_index;
        let (locals_after, next_locals_after_index) =
            self.parse_i32_list_required(tokens, index, FIELD_LOCALS_AFTER)?;
        index = next_locals_after_index;
        let (memory_after, next_memory_after_index) =
            self.parse_i32_list_required(tokens, index, FIELD_MEMORY_AFTER)?;
        index = next_memory_after_index;
        Ok((
            TassadarTraceStep {
                step_index,
                pc,
                next_pc,
                instruction,
                event,
                stack_before,
                stack_after,
                locals_after,
                memory_after,
            },
            index,
        ))
    }

    fn parse_instruction_required(
        &self,
        tokens: &[TokenId],
        index: usize,
    ) -> Result<(TassadarInstruction, usize), TassadarArticleTraceDecodeError> {
        let token = tokens.get(index).copied().ok_or_else(|| {
            TassadarArticleTraceDecodeError::UnexpectedEnd {
                index,
                context: String::from("instruction"),
            }
        })?;
        if token == self.token_id(OP_NOP) {
            return Ok((TassadarInstruction::Nop, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_CONST) {
            return Ok((
                TassadarInstruction::I32Const {
                    value: self.parse_i32_required(tokens, index + 1, "instruction.i32_const")?,
                },
                index.saturating_add(5),
            ));
        }
        if token == self.token_id(OP_LOCAL_GET) {
            return Ok((
                TassadarInstruction::LocalGet {
                    local: self.parse_u8_required(tokens, index + 1, "instruction.local_get")?,
                },
                index.saturating_add(5),
            ));
        }
        if token == self.token_id(OP_LOCAL_SET) {
            return Ok((
                TassadarInstruction::LocalSet {
                    local: self.parse_u8_required(tokens, index + 1, "instruction.local_set")?,
                },
                index.saturating_add(5),
            ));
        }
        if token == self.token_id(OP_LOCAL_TEE) {
            return Ok((
                TassadarInstruction::LocalTee {
                    local: self.parse_u8_required(tokens, index + 1, "instruction.local_tee")?,
                },
                index.saturating_add(5),
            ));
        }
        if token == self.token_id(OP_DROP) {
            return Ok((TassadarInstruction::Drop, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_ADD) {
            return Ok((TassadarInstruction::I32Add, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_SUB) {
            return Ok((TassadarInstruction::I32Sub, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_MUL) {
            return Ok((TassadarInstruction::I32Mul, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_LT) {
            return Ok((TassadarInstruction::I32Lt, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_EQZ) {
            return Ok((TassadarInstruction::I32Eqz, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_EQ) {
            return Ok((TassadarInstruction::I32Eq, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_NE) {
            return Ok((TassadarInstruction::I32Ne, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_GT) {
            return Ok((TassadarInstruction::I32Gt, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_LE) {
            return Ok((TassadarInstruction::I32Le, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_GE) {
            return Ok((TassadarInstruction::I32Ge, index.saturating_add(1)));
        }
        if token == self.token_id(OP_I32_LOAD) {
            return Ok((
                TassadarInstruction::I32Load {
                    slot: self.parse_u8_required(tokens, index + 1, "instruction.i32_load")?,
                },
                index.saturating_add(5),
            ));
        }
        if token == self.token_id(OP_I32_STORE) {
            return Ok((
                TassadarInstruction::I32Store {
                    slot: self.parse_u8_required(tokens, index + 1, "instruction.i32_store")?,
                },
                index.saturating_add(5),
            ));
        }
        if token == self.token_id(OP_BR_IF) {
            return Ok((
                TassadarInstruction::BrIf {
                    target_pc: self.parse_u16_required(tokens, index + 1, "instruction.br_if")?,
                },
                index.saturating_add(5),
            ));
        }
        if token == self.token_id(OP_OUTPUT) {
            return Ok((TassadarInstruction::Output, index.saturating_add(1)));
        }
        if token == self.token_id(OP_RETURN) {
            return Ok((TassadarInstruction::Return, index.saturating_add(1)));
        }
        Err(self.unexpected_token(index, "instruction opcode", Some(token)))
    }

    fn parse_event_required(
        &self,
        tokens: &[TokenId],
        index: usize,
    ) -> Result<(TassadarTraceEvent, usize), TassadarArticleTraceDecodeError> {
        let token = tokens.get(index).copied().ok_or_else(|| {
            TassadarArticleTraceDecodeError::UnexpectedEnd {
                index,
                context: String::from("event"),
            }
        })?;
        if token == self.token_id(EVENT_NOP) {
            return Ok((TassadarTraceEvent::Nop, index.saturating_add(1)));
        }
        if token == self.token_id(EVENT_CONST_PUSH) {
            return Ok((
                TassadarTraceEvent::ConstPush {
                    value: self.parse_i32_required(tokens, index + 1, "event.const_push")?,
                },
                index.saturating_add(5),
            ));
        }
        if token == self.token_id(EVENT_LOCAL_GET) {
            return Ok((
                TassadarTraceEvent::LocalGet {
                    local: self.parse_u8_required(tokens, index + 1, "event.local_get.local")?,
                    value: self.parse_i32_required(tokens, index + 5, "event.local_get.value")?,
                },
                index.saturating_add(9),
            ));
        }
        if token == self.token_id(EVENT_LOCAL_SET) {
            return Ok((
                TassadarTraceEvent::LocalSet {
                    local: self.parse_u8_required(tokens, index + 1, "event.local_set.local")?,
                    value: self.parse_i32_required(tokens, index + 5, "event.local_set.value")?,
                },
                index.saturating_add(9),
            ));
        }
        if token == self.token_id(EVENT_LOCAL_TEE) {
            return Ok((
                TassadarTraceEvent::LocalTee {
                    local: self.parse_u8_required(tokens, index + 1, "event.local_tee.local")?,
                    value: self.parse_i32_required(tokens, index + 5, "event.local_tee.value")?,
                },
                index.saturating_add(9),
            ));
        }
        if token == self.token_id(EVENT_DROP) {
            return Ok((
                TassadarTraceEvent::Drop {
                    value: self.parse_i32_required(tokens, index + 1, "event.drop.value")?,
                },
                index.saturating_add(5),
            ));
        }
        if token == self.token_id(EVENT_UNARY_EQZ) {
            return Ok((
                TassadarTraceEvent::UnaryOp {
                    op: TassadarUnaryOp::Eqz,
                    operand: self.parse_i32_required(tokens, index + 1, "event.eqz.operand")?,
                    result: self.parse_i32_required(tokens, index + 5, "event.eqz.result")?,
                },
                index.saturating_add(9),
            ));
        }
        if token == self.token_id(EVENT_BINARY_ADD) {
            return Ok((
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Add,
                    left: self.parse_i32_required(tokens, index + 1, "event.binary_add.left")?,
                    right: self.parse_i32_required(tokens, index + 5, "event.binary_add.right")?,
                    result: self.parse_i32_required(
                        tokens,
                        index + 9,
                        "event.binary_add.result",
                    )?,
                },
                index.saturating_add(13),
            ));
        }
        if token == self.token_id(EVENT_BINARY_SUB) {
            return Ok((
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Sub,
                    left: self.parse_i32_required(tokens, index + 1, "event.binary_sub.left")?,
                    right: self.parse_i32_required(tokens, index + 5, "event.binary_sub.right")?,
                    result: self.parse_i32_required(
                        tokens,
                        index + 9,
                        "event.binary_sub.result",
                    )?,
                },
                index.saturating_add(13),
            ));
        }
        if token == self.token_id(EVENT_BINARY_MUL) {
            return Ok((
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Mul,
                    left: self.parse_i32_required(tokens, index + 1, "event.binary_mul.left")?,
                    right: self.parse_i32_required(tokens, index + 5, "event.binary_mul.right")?,
                    result: self.parse_i32_required(
                        tokens,
                        index + 9,
                        "event.binary_mul.result",
                    )?,
                },
                index.saturating_add(13),
            ));
        }
        if token == self.token_id(EVENT_BINARY_LT) {
            return Ok((
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Lt,
                    left: self.parse_i32_required(tokens, index + 1, "event.binary_lt.left")?,
                    right: self.parse_i32_required(tokens, index + 5, "event.binary_lt.right")?,
                    result: self.parse_i32_required(tokens, index + 9, "event.binary_lt.result")?,
                },
                index.saturating_add(13),
            ));
        }
        if token == self.token_id(EVENT_BINARY_EQ) {
            return Ok((
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Eq,
                    left: self.parse_i32_required(tokens, index + 1, "event.binary_eq.left")?,
                    right: self.parse_i32_required(tokens, index + 5, "event.binary_eq.right")?,
                    result: self.parse_i32_required(tokens, index + 9, "event.binary_eq.result")?,
                },
                index.saturating_add(13),
            ));
        }
        if token == self.token_id(EVENT_BINARY_NE) {
            return Ok((
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Ne,
                    left: self.parse_i32_required(tokens, index + 1, "event.binary_ne.left")?,
                    right: self.parse_i32_required(tokens, index + 5, "event.binary_ne.right")?,
                    result: self.parse_i32_required(tokens, index + 9, "event.binary_ne.result")?,
                },
                index.saturating_add(13),
            ));
        }
        if token == self.token_id(EVENT_BINARY_GT) {
            return Ok((
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Gt,
                    left: self.parse_i32_required(tokens, index + 1, "event.binary_gt.left")?,
                    right: self.parse_i32_required(tokens, index + 5, "event.binary_gt.right")?,
                    result: self.parse_i32_required(tokens, index + 9, "event.binary_gt.result")?,
                },
                index.saturating_add(13),
            ));
        }
        if token == self.token_id(EVENT_BINARY_LE) {
            return Ok((
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Le,
                    left: self.parse_i32_required(tokens, index + 1, "event.binary_le.left")?,
                    right: self.parse_i32_required(tokens, index + 5, "event.binary_le.right")?,
                    result: self.parse_i32_required(tokens, index + 9, "event.binary_le.result")?,
                },
                index.saturating_add(13),
            ));
        }
        if token == self.token_id(EVENT_BINARY_GE) {
            return Ok((
                TassadarTraceEvent::BinaryOp {
                    op: TassadarArithmeticOp::Ge,
                    left: self.parse_i32_required(tokens, index + 1, "event.binary_ge.left")?,
                    right: self.parse_i32_required(tokens, index + 5, "event.binary_ge.right")?,
                    result: self.parse_i32_required(tokens, index + 9, "event.binary_ge.result")?,
                },
                index.saturating_add(13),
            ));
        }
        if token == self.token_id(EVENT_LOAD) {
            return Ok((
                TassadarTraceEvent::Load {
                    slot: self.parse_u8_required(tokens, index + 1, "event.load.slot")?,
                    value: self.parse_i32_required(tokens, index + 5, "event.load.value")?,
                },
                index.saturating_add(9),
            ));
        }
        if token == self.token_id(EVENT_STORE) {
            return Ok((
                TassadarTraceEvent::Store {
                    slot: self.parse_u8_required(tokens, index + 1, "event.store.slot")?,
                    value: self.parse_i32_required(tokens, index + 5, "event.store.value")?,
                },
                index.saturating_add(9),
            ));
        }
        if token == self.token_id(EVENT_BRANCH) {
            return Ok((
                TassadarTraceEvent::Branch {
                    condition: self.parse_i32_required(
                        tokens,
                        index + 1,
                        "event.branch.condition",
                    )?,
                    taken: self.parse_bool_required(tokens, index + 5)?,
                    target_pc: self.parse_u32_required(
                        tokens,
                        index + 6,
                        "event.branch.target_pc",
                    )? as usize,
                },
                index.saturating_add(10),
            ));
        }
        if token == self.token_id(EVENT_OUTPUT) {
            return Ok((
                TassadarTraceEvent::Output {
                    value: self.parse_i32_required(tokens, index + 1, "event.output.value")?,
                },
                index.saturating_add(5),
            ));
        }
        if token == self.token_id(EVENT_RETURN) {
            return Ok((TassadarTraceEvent::Return, index.saturating_add(1)));
        }
        Err(self.unexpected_token(index, "event opcode", Some(token)))
    }

    fn parse_halt_reason(
        &self,
        tokens: &[TokenId],
        index: usize,
    ) -> Result<TassadarHaltReason, TassadarArticleTraceDecodeError> {
        let token = tokens.get(index).copied().ok_or_else(|| {
            TassadarArticleTraceDecodeError::UnexpectedEnd {
                index,
                context: String::from("halt_reason"),
            }
        })?;
        if token == self.token_id(HALT_RETURNED) {
            return Ok(TassadarHaltReason::Returned);
        }
        if token == self.token_id(HALT_FELL_OFF_END) {
            return Ok(TassadarHaltReason::FellOffEnd);
        }
        Err(self.unexpected_token(index, "halt reason", Some(token)))
    }

    fn parse_bool_required(
        &self,
        tokens: &[TokenId],
        index: usize,
    ) -> Result<bool, TassadarArticleTraceDecodeError> {
        let token = tokens.get(index).copied().ok_or_else(|| {
            TassadarArticleTraceDecodeError::UnexpectedEnd {
                index,
                context: String::from("bool"),
            }
        })?;
        if token == self.token_id(BOOL_TRUE) {
            return Ok(true);
        }
        if token == self.token_id(BOOL_FALSE) {
            return Ok(false);
        }
        Err(self.unexpected_token(index, "bool marker", Some(token)))
    }

    fn parse_u32_required(
        &self,
        tokens: &[TokenId],
        start: usize,
        field: &str,
    ) -> Result<u32, TassadarArticleTraceDecodeError> {
        self.parse_u32(tokens, start).ok_or_else(|| {
            TassadarArticleTraceDecodeError::UnexpectedEnd {
                index: start,
                context: String::from(field),
            }
        })
    }

    fn parse_i32_required(
        &self,
        tokens: &[TokenId],
        start: usize,
        field: &str,
    ) -> Result<i32, TassadarArticleTraceDecodeError> {
        self.parse_i32(tokens, start).ok_or_else(|| {
            TassadarArticleTraceDecodeError::UnexpectedEnd {
                index: start,
                context: String::from(field),
            }
        })
    }

    fn parse_u8_required(
        &self,
        tokens: &[TokenId],
        start: usize,
        field: &str,
    ) -> Result<u8, TassadarArticleTraceDecodeError> {
        let value = self.parse_u32_required(tokens, start, field)?;
        u8::try_from(value).map_err(|_| TassadarArticleTraceDecodeError::ImmediateOutOfRange {
            field: String::from(field),
            value,
            target: String::from("u8"),
        })
    }

    fn parse_u16_required(
        &self,
        tokens: &[TokenId],
        start: usize,
        field: &str,
    ) -> Result<u16, TassadarArticleTraceDecodeError> {
        let value = self.parse_u32_required(tokens, start, field)?;
        u16::try_from(value).map_err(|_| TassadarArticleTraceDecodeError::ImmediateOutOfRange {
            field: String::from(field),
            value,
            target: String::from("u16"),
        })
    }

    fn parse_i32_list_required(
        &self,
        tokens: &[TokenId],
        index: usize,
        field_token: &str,
    ) -> Result<(Vec<i32>, usize), TassadarArticleTraceDecodeError> {
        self.expect_token(tokens, index, field_token)?;
        self.expect_token(tokens, index + 1, LIST_TOKEN)?;
        let values = self
            .parse_i32_list(tokens, index, field_token)
            .ok_or_else(|| TassadarArticleTraceDecodeError::UnexpectedEnd {
                index,
                context: format!("{field_token}_list"),
            })?;
        let next_index = index
            .saturating_add(6)
            .saturating_add(values.len().saturating_mul(4));
        Ok((values, next_index))
    }

    fn parse_i32(&self, tokens: &[TokenId], start: usize) -> Option<i32> {
        let bytes = [
            self.byte_value(*tokens.get(start)?)?,
            self.byte_value(*tokens.get(start + 1)?)?,
            self.byte_value(*tokens.get(start + 2)?)?,
            self.byte_value(*tokens.get(start + 3)?)?,
        ];
        Some(i32::from_le_bytes(bytes))
    }

    fn byte_value(&self, token: TokenId) -> Option<u8> {
        let raw = token.as_u32();
        let end = self.byte_token_start + 256;
        if raw < self.byte_token_start || raw >= end {
            return None;
        }
        Some((raw - self.byte_token_start) as u8)
    }

    fn consume_field_u32(
        &self,
        tokens: &[TokenId],
        prompt_token_count: usize,
        index: usize,
        field_token: &str,
        family: Option<TassadarStructuralSupervisionFamily>,
        families: &mut [Vec<TassadarStructuralSupervisionFamily>],
    ) -> usize {
        if index >= tokens.len() || tokens[index] != self.token_id(field_token) {
            return index.saturating_add(1);
        }
        if let Some(family) = family {
            self.mark_target_range(prompt_token_count, index, 5, family, families);
        }
        index.saturating_add(5).min(tokens.len())
    }

    fn consume_instruction(&self, tokens: &[TokenId], index: usize) -> usize {
        if index >= tokens.len() {
            return index;
        }
        let token = tokens[index];
        let with_immediate = [
            self.token_id(OP_I32_CONST),
            self.token_id(OP_LOCAL_GET),
            self.token_id(OP_LOCAL_SET),
            self.token_id(OP_LOCAL_TEE),
            self.token_id(OP_I32_LOAD),
            self.token_id(OP_I32_STORE),
            self.token_id(OP_BR_IF),
        ];
        if with_immediate.contains(&token) {
            index.saturating_add(5).min(tokens.len())
        } else {
            index.saturating_add(1).min(tokens.len())
        }
    }

    fn consume_event(
        &self,
        tokens: &[TokenId],
        prompt_token_count: usize,
        index: usize,
        families: &mut [Vec<TassadarStructuralSupervisionFamily>],
    ) -> (usize, Option<usize>) {
        if index >= tokens.len() {
            return (index, None);
        }
        let token = tokens[index];
        if token == self.token_id(EVENT_NOP) {
            return (index.saturating_add(1).min(tokens.len()), None);
        }
        if token == self.token_id(EVENT_CONST_PUSH) {
            return (index.saturating_add(5).min(tokens.len()), None);
        }
        if token == self.token_id(EVENT_LOCAL_GET)
            || token == self.token_id(EVENT_LOCAL_SET)
            || token == self.token_id(EVENT_LOCAL_TEE)
        {
            return (index.saturating_add(9).min(tokens.len()), None);
        }
        if token == self.token_id(EVENT_DROP) {
            return (index.saturating_add(5).min(tokens.len()), None);
        }
        if token == self.token_id(EVENT_UNARY_EQZ) {
            return (index.saturating_add(9).min(tokens.len()), None);
        }
        if [
            self.token_id(EVENT_BINARY_ADD),
            self.token_id(EVENT_BINARY_SUB),
            self.token_id(EVENT_BINARY_MUL),
            self.token_id(EVENT_BINARY_LT),
            self.token_id(EVENT_BINARY_EQ),
            self.token_id(EVENT_BINARY_NE),
            self.token_id(EVENT_BINARY_GT),
            self.token_id(EVENT_BINARY_LE),
            self.token_id(EVENT_BINARY_GE),
        ]
        .contains(&token)
        {
            return (index.saturating_add(13).min(tokens.len()), None);
        }
        if token == self.token_id(EVENT_LOAD) {
            return (index.saturating_add(9).min(tokens.len()), None);
        }
        if token == self.token_id(EVENT_STORE) {
            self.mark_target_range(
                prompt_token_count,
                index,
                9,
                TassadarStructuralSupervisionFamily::MemoryDiff,
                families,
            );
            let slot = self
                .parse_u32(tokens, index.saturating_add(1))
                .map(|value| value as usize);
            return (index.saturating_add(9).min(tokens.len()), slot);
        }
        if token == self.token_id(EVENT_BRANCH) {
            self.mark_target_range(
                prompt_token_count,
                index,
                10,
                TassadarStructuralSupervisionFamily::BranchOutcome,
                families,
            );
            return (index.saturating_add(10).min(tokens.len()), None);
        }
        if token == self.token_id(EVENT_OUTPUT) {
            return (index.saturating_add(5).min(tokens.len()), None);
        }
        if token == self.token_id(EVENT_RETURN) {
            return (index.saturating_add(1).min(tokens.len()), None);
        }
        (index.saturating_add(1).min(tokens.len()), None)
    }

    fn consume_stack_list(
        &self,
        tokens: &[TokenId],
        prompt_token_count: usize,
        index: usize,
        field_token: &str,
        families: &mut [Vec<TassadarStructuralSupervisionFamily>],
    ) -> usize {
        if index.saturating_add(6) > tokens.len() || tokens[index] != self.token_id(field_token) {
            return index.saturating_add(1);
        }
        self.mark_target_range(
            prompt_token_count,
            index,
            6,
            TassadarStructuralSupervisionFamily::StackDelta,
            families,
        );
        self.consume_i32_list(tokens, index, field_token)
    }

    fn consume_i32_list(&self, tokens: &[TokenId], index: usize, field_token: &str) -> usize {
        if index.saturating_add(6) > tokens.len() || tokens[index] != self.token_id(field_token) {
            return index.saturating_add(1);
        }
        if tokens[index + 1] != self.token_id(LIST_TOKEN) {
            return index.saturating_add(1);
        }
        let value_count = self.parse_u32(tokens, index + 2).unwrap_or(0) as usize;
        index
            .saturating_add(6)
            .saturating_add(value_count.saturating_mul(4))
            .min(tokens.len())
    }

    fn consume_locals_after_list(
        &self,
        tokens: &[TokenId],
        prompt_token_count: usize,
        index: usize,
        workload_scope: TassadarWorkloadSpecificSupervisionScope,
        families: &mut [Vec<TassadarStructuralSupervisionFamily>],
    ) -> usize {
        if index.saturating_add(6) > tokens.len()
            || tokens[index] != self.token_id(FIELD_LOCALS_AFTER)
            || tokens[index + 1] != self.token_id(LIST_TOKEN)
        {
            return index.saturating_add(1);
        }
        let value_count = self.parse_u32(tokens, index + 2).unwrap_or(0) as usize;
        let marked_value_count = match workload_scope {
            TassadarWorkloadSpecificSupervisionScope::None => 0,
            TassadarWorkloadSpecificSupervisionScope::HungarianV0 => value_count.min(1),
            TassadarWorkloadSpecificSupervisionScope::Hungarian10x10 => value_count.min(3),
        };
        if marked_value_count > 0 {
            self.mark_target_range(
                prompt_token_count,
                index.saturating_add(6),
                marked_value_count.saturating_mul(4),
                TassadarStructuralSupervisionFamily::WorkloadSpecificState,
                families,
            );
        }
        self.consume_i32_list(tokens, index, FIELD_LOCALS_AFTER)
    }

    fn consume_memory_after_list(
        &self,
        tokens: &[TokenId],
        prompt_token_count: usize,
        index: usize,
        memory_write_slot: Option<usize>,
        workload_scope: TassadarWorkloadSpecificSupervisionScope,
        families: &mut [Vec<TassadarStructuralSupervisionFamily>],
    ) -> usize {
        if index.saturating_add(6) > tokens.len()
            || tokens[index] != self.token_id(FIELD_MEMORY_AFTER)
            || tokens[index + 1] != self.token_id(LIST_TOKEN)
        {
            return index.saturating_add(1);
        }
        let value_count = self.parse_u32(tokens, index + 2).unwrap_or(0) as usize;
        if let Some(slot) = memory_write_slot {
            if slot < value_count {
                let value_start = index
                    .saturating_add(6)
                    .saturating_add(slot.saturating_mul(4));
                self.mark_target_range(
                    prompt_token_count,
                    value_start,
                    4,
                    TassadarStructuralSupervisionFamily::MemoryDiff,
                    families,
                );
                if self.is_workload_specific_memory_slot(workload_scope, slot, value_count) {
                    self.mark_target_range(
                        prompt_token_count,
                        value_start,
                        4,
                        TassadarStructuralSupervisionFamily::WorkloadSpecificState,
                        families,
                    );
                }
            }
        }
        index
            .saturating_add(6)
            .saturating_add(value_count.saturating_mul(4))
            .min(tokens.len())
    }

    fn detect_workload_specific_supervision_scope(
        &self,
        tokens: &[TokenId],
        prompt_token_count: usize,
    ) -> TassadarWorkloadSpecificSupervisionScope {
        if prompt_token_count < 17
            || tokens.first().copied() != Some(self.vocabulary.bos_id())
            || tokens.get(1).copied() != Some(self.token_id(PROGRAM_TOKEN))
            || tokens.get(2).copied() != Some(self.token_id(FIELD_LOCALS))
            || tokens.get(7).copied() != Some(self.token_id(FIELD_MEMORY_SLOTS))
            || tokens.get(12).copied() != Some(self.token_id(FIELD_INITIAL_MEMORY))
        {
            return TassadarWorkloadSpecificSupervisionScope::None;
        }
        let local_count = self.parse_u32(tokens, 3);
        let memory_slots = self.parse_u32(tokens, 8);
        let initial_memory_len = self.parse_u32(tokens, 13);
        match (local_count, memory_slots, initial_memory_len) {
            (
                Some(HUNGARIAN_V0_LOCAL_COUNT),
                Some(HUNGARIAN_V0_MEMORY_SLOTS),
                Some(HUNGARIAN_V0_MEMORY_SLOTS),
            ) => TassadarWorkloadSpecificSupervisionScope::HungarianV0,
            (
                Some(HUNGARIAN_10X10_LOCAL_COUNT),
                Some(HUNGARIAN_10X10_MEMORY_SLOTS),
                Some(HUNGARIAN_10X10_MEMORY_SLOTS),
            ) => TassadarWorkloadSpecificSupervisionScope::Hungarian10x10,
            _ => TassadarWorkloadSpecificSupervisionScope::None,
        }
    }

    fn is_workload_specific_memory_slot(
        &self,
        workload_scope: TassadarWorkloadSpecificSupervisionScope,
        slot: usize,
        value_count: usize,
    ) -> bool {
        if slot >= value_count {
            return false;
        }
        match workload_scope {
            TassadarWorkloadSpecificSupervisionScope::None => false,
            TassadarWorkloadSpecificSupervisionScope::HungarianV0 => {
                slot == HUNGARIAN_V0_BEST_COST_SLOT
                    || (HUNGARIAN_V0_OUTPUT_SLOT_BASE
                        ..HUNGARIAN_V0_OUTPUT_SLOT_BASE + HUNGARIAN_V0_DIM)
                        .contains(&slot)
            }
            TassadarWorkloadSpecificSupervisionScope::Hungarian10x10 => true,
        }
    }

    fn parse_u32(&self, tokens: &[TokenId], start: usize) -> Option<u32> {
        let bytes = [
            self.byte_value(*tokens.get(start)?)?,
            self.byte_value(*tokens.get(start + 1)?)?,
            self.byte_value(*tokens.get(start + 2)?)?,
            self.byte_value(*tokens.get(start + 3)?)?,
        ];
        Some(u32::from_le_bytes(bytes))
    }

    fn parse_i32_list(
        &self,
        tokens: &[TokenId],
        index: usize,
        field_token: &str,
    ) -> Option<Vec<i32>> {
        if index.saturating_add(6) > tokens.len()
            || tokens.get(index).copied()? != self.token_id(field_token)
            || tokens.get(index + 1).copied()? != self.token_id(LIST_TOKEN)
        {
            return None;
        }
        let value_count = self.parse_u32(tokens, index + 2)? as usize;
        let values_start = index + 6;
        let mut values = Vec::with_capacity(value_count);
        for value_index in 0..value_count {
            values.push(self.parse_i32(tokens, values_start + value_index.saturating_mul(4))?);
        }
        Some(values)
    }

    fn parse_wavefront_step(
        &self,
        tokens: &[TokenId],
        index: usize,
    ) -> Option<ParsedWavefrontStep> {
        if tokens.get(index).copied()? != self.token_id(STEP_TOKEN) {
            return None;
        }
        if tokens.get(index + 1).copied()? != self.token_id(FIELD_STEP_INDEX) {
            return None;
        }
        let step_index = self.parse_u32(tokens, index + 2)?;
        if tokens.get(index + 6).copied()? != self.token_id(FIELD_PC) {
            return None;
        }
        let payload_item_count = self.parse_u32(tokens, index + 7)?;
        if tokens.get(index + 11).copied()? != self.token_id(FIELD_NEXT_PC) {
            return None;
        }
        let summary_index = self.parse_u32(tokens, index + 12)?;
        let op_index = index + 16;
        if tokens.get(op_index).copied()? != self.token_id(OP_OUTPUT) {
            return None;
        }
        if tokens.get(op_index + 1).copied()? != self.token_id(EVENT_OUTPUT) {
            return None;
        }
        let summary_output = self.parse_i32(tokens, op_index + 2)?;
        let stack_before_index = op_index + 6;
        let stack_before = self.parse_i32_list(tokens, stack_before_index, FIELD_STACK_BEFORE)?;
        let stack_after_index = stack_before_index + 6 + stack_before.len().saturating_mul(4);
        let stack_after = self.parse_i32_list(tokens, stack_after_index, FIELD_STACK_AFTER)?;
        let locals_after_index = stack_after_index + 6 + stack_after.len().saturating_mul(4);
        let locals_after = self.parse_i32_list(tokens, locals_after_index, FIELD_LOCALS_AFTER)?;
        let memory_after_index = locals_after_index + 6 + locals_after.len().saturating_mul(4);
        let memory_after = self.parse_i32_list(tokens, memory_after_index, FIELD_MEMORY_AFTER)?;
        let next_index = memory_after_index + 6 + memory_after.len().saturating_mul(4);
        Some(ParsedWavefrontStep {
            step_index,
            payload_item_count,
            summary_index,
            summary_output: Some(summary_output),
            stack_after,
            next_index,
        })
    }

    fn mark_target_range(
        &self,
        prompt_token_count: usize,
        start: usize,
        len: usize,
        family: TassadarStructuralSupervisionFamily,
        families: &mut [Vec<TassadarStructuralSupervisionFamily>],
    ) {
        let end = start.saturating_add(len);
        for absolute_index in start..end {
            if absolute_index < prompt_token_count {
                continue;
            }
            let target_index = absolute_index - prompt_token_count;
            let Some(entry) = families.get_mut(target_index) else {
                continue;
            };
            if !entry.contains(&family) {
                entry.push(family);
            }
        }
    }
}

impl TokenizerBoundary for TassadarTraceTokenizer {
    fn encode(&self, text: &str) -> TokenSequence {
        let tokens: Vec<_> = text
            .split_whitespace()
            .map(|piece| {
                self.lookup
                    .get(piece)
                    .copied()
                    .unwrap_or(self.vocabulary.unknown_id())
            })
            .collect();
        TokenSequence::new(tokens)
    }

    fn decode(&self, tokens: &[TokenId]) -> String {
        tokens
            .iter()
            .map(|token| self.vocabulary.token(*token).unwrap_or(UNKNOWN_TOKEN))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn append_decoded_token(&self, text: &mut String, token: TokenId) {
        let piece = self.vocabulary.token(token).unwrap_or(UNKNOWN_TOKEN);
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(piece);
    }

    fn vocabulary(&self) -> &TokenVocabulary {
        &self.vocabulary
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("Tassadar tokenizer value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedWavefrontStep {
    step_index: u32,
    payload_item_count: u32,
    summary_index: u32,
    summary_output: Option<i32>,
    stack_after: Vec<i32>,
    next_index: usize,
}

#[cfg(test)]
mod tests {
    use crate::TokenizerBoundary;
    use psionic_runtime::{
        TassadarCpuReferenceRunner, tassadar_article_class_corpus, tassadar_hungarian_10x10_corpus,
        tassadar_hungarian_v0_corpus, tassadar_sudoku_9x9_corpus, tassadar_sudoku_v0_corpus,
    };

    use super::{
        TassadarSequenceTraceFamily, TassadarStructuralSupervisionCoverage, TassadarTraceTokenizer,
    };

    #[test]
    fn tokenizer_roundtrips_symbolic_tokens_for_sudoku_v0_reference_case()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let case = tassadar_sudoku_v0_corpus()
            .into_iter()
            .next()
            .expect("sudoku corpus should not be empty");
        let execution = TassadarCpuReferenceRunner::for_program(&case.validation_case.program)?
            .execute(&case.validation_case.program)?;
        let tokenized =
            tokenizer.tokenize_program_and_execution(&case.validation_case.program, &execution);
        let decoded = tokenizer.decode_symbolic(&tokenized);
        let reencoded = tokenizer.encode(decoded.tokens.join(" ").as_str());

        assert_eq!(reencoded, tokenized.sequence);
        assert!(tokenized.prompt_token_count > 0);
        assert!(tokenized.target_token_count > 0);
        assert!(!tokenized.sequence_digest.is_empty());
        Ok(())
    }

    #[test]
    fn tokenizer_roundtrips_typed_article_trace_domain_for_article_class_case()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let case = tassadar_article_class_corpus()
            .into_iter()
            .next()
            .expect("article corpus should not be empty");
        let execution =
            TassadarCpuReferenceRunner::for_program(&case.program)?.execute(&case.program)?;
        let tokenized = tokenizer.tokenize_program_and_execution(&case.program, &execution);
        let decoded = tokenizer.decode_article_trace_domain(&tokenized)?;
        let reconstructed_program = decoded.materialize_program(
            case.program.program_id.clone(),
            case.program.profile_id.clone(),
        );
        let reconstructed_execution = decoded.materialize_execution(
            execution.program_id.clone(),
            execution.profile_id.clone(),
            execution.runner_id.clone(),
            execution.trace_abi.clone(),
        );

        assert_eq!(decoded.prompt_token_count, tokenized.prompt_token_count);
        assert_eq!(decoded.target_token_count, tokenized.target_token_count);
        assert_eq!(decoded.sequence_digest, tokenized.sequence_digest);
        assert_eq!(reconstructed_program, case.program);
        assert_eq!(reconstructed_execution, execution);
        Ok(())
    }

    #[test]
    fn tokenizer_can_recompose_prompt_and_target_sequence_without_drift()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let case = tassadar_sudoku_v0_corpus()
            .into_iter()
            .next()
            .expect("sudoku corpus should not be empty");
        let execution = TassadarCpuReferenceRunner::for_program(&case.validation_case.program)?
            .execute(&case.validation_case.program)?;
        let tokenized =
            tokenizer.tokenize_program_and_execution(&case.validation_case.program, &execution);
        let recomposed = tokenizer.compose_prompt_and_target_sequence(
            &tokenized.sequence.as_slice()[..tokenized.prompt_token_count],
            &tokenized.sequence.as_slice()[tokenized.prompt_token_count..],
        );

        assert_eq!(recomposed, tokenized);
        Ok(())
    }

    #[test]
    fn tokenizer_retokenizes_symbolic_text_across_whitespace_variants()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let case = tassadar_article_class_corpus()
            .into_iter()
            .next()
            .expect("article corpus should not be empty");
        let execution =
            TassadarCpuReferenceRunner::for_program(&case.program)?.execute(&case.program)?;
        let tokenized = tokenizer.tokenize_program_and_execution(&case.program, &execution);
        let symbolic = tokenizer.decode_symbolic(&tokenized);
        let whitespace_perturbed = format!(
            "\n{}\n",
            symbolic
                .tokens
                .iter()
                .enumerate()
                .map(|(index, token)| match index % 3 {
                    0 => format!("{token}\n"),
                    1 => format!("{token}\t"),
                    _ => token.clone(),
                })
                .collect::<Vec<_>>()
                .join("  ")
        );

        let perturbed =
            tokenizer.retokenize_symbolic_text(&whitespace_perturbed, tokenized.prompt_token_count);

        assert_eq!(perturbed, tokenized);
        Ok(())
    }

    #[test]
    fn tokenizer_can_compose_prompt_and_target_from_symbolic_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let case = tassadar_article_class_corpus()
            .into_iter()
            .next()
            .expect("article corpus should not be empty");
        let execution =
            TassadarCpuReferenceRunner::for_program(&case.program)?.execute(&case.program)?;
        let tokenized = tokenizer.tokenize_program_and_execution(&case.program, &execution);
        let symbolic = tokenizer.decode_symbolic(&tokenized);
        let prompt_text = symbolic.tokens[..symbolic.prompt_token_count].join("\n");
        let target_text = symbolic.tokens[symbolic.prompt_token_count..].join("\t");

        let recomposed =
            tokenizer.compose_prompt_and_target_symbolic_text(&prompt_text, &target_text);

        assert_eq!(recomposed, tokenized);
        Ok(())
    }

    #[test]
    fn tokenizer_digest_is_stable() {
        let tokenizer = TassadarTraceTokenizer::new();

        assert_eq!(tokenizer.stable_digest(), tokenizer.stable_digest());
        assert!(tokenizer.vocabulary().len() > 256);
    }

    #[test]
    fn tokenizer_derives_structural_supervision_coverage_for_reference_trace()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let case = tassadar_sudoku_v0_corpus()
            .into_iter()
            .next()
            .expect("sudoku corpus should not be empty");
        let execution = TassadarCpuReferenceRunner::for_program(&case.validation_case.program)?
            .execute(&case.validation_case.program)?;
        let tokenized =
            tokenizer.tokenize_program_and_execution(&case.validation_case.program, &execution);
        let coverage = tokenizer.summarize_target_structural_supervision(
            tokenized.sequence.as_slice(),
            tokenized.prompt_token_count,
        );

        assert_eq!(
            coverage.total_target_token_count,
            tokenized.target_token_count as u32
        );
        assert!(coverage.instruction_pointer_token_count > 0);
        assert!(coverage.stack_delta_token_count > 0);
        assert_eq!(
            coverage.workload_specific_state_token_count,
            TassadarStructuralSupervisionCoverage::default().workload_specific_state_token_count
        );
        Ok(())
    }

    #[test]
    fn tokenizer_derives_workload_specific_supervision_for_hungarian_trace()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let case = tassadar_hungarian_v0_corpus()
            .into_iter()
            .next()
            .expect("hungarian corpus should not be empty");
        let execution = TassadarCpuReferenceRunner::for_program(&case.validation_case.program)?
            .execute(&case.validation_case.program)?;
        let tokenized =
            tokenizer.tokenize_program_and_execution(&case.validation_case.program, &execution);
        let coverage = tokenizer.summarize_target_structural_supervision(
            tokenized.sequence.as_slice(),
            tokenized.prompt_token_count,
        );

        assert_eq!(
            coverage.total_target_token_count,
            tokenized.target_token_count as u32
        );
        assert!(coverage.workload_specific_state_token_count > 0);
        Ok(())
    }

    #[test]
    fn tokenizer_roundtrips_sudoku_wavefront_outputs_exactly()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let case = tassadar_sudoku_9x9_corpus()
            .into_iter()
            .next()
            .expect("sudoku corpus should not be empty");
        let execution = TassadarCpuReferenceRunner::for_program(&case.validation_case.program)?
            .execute(&case.validation_case.program)?;
        let tokenized = tokenizer.tokenize_program_and_sudoku_diagonal_wavefront(
            &case.validation_case.program,
            execution.outputs.as_slice(),
            9,
        );

        let reconstructed = tokenizer.extract_sudoku_diagonal_wavefront_outputs(
            tokenized.sequence.as_slice(),
            tokenized.prompt_token_count,
            execution.outputs.len(),
        );

        assert_eq!(reconstructed, execution.outputs);
        assert_eq!(
            TassadarSequenceTraceFamily::SudokuDiagonalWavefront.reconstruction_scope(),
            "final_outputs_only"
        );
        Ok(())
    }

    #[test]
    fn tokenizer_roundtrips_hungarian_assignment_frontier_outputs_exactly()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = TassadarTraceTokenizer::new();
        let case = tassadar_hungarian_10x10_corpus()
            .into_iter()
            .next()
            .expect("hungarian corpus should not be empty");
        let execution = TassadarCpuReferenceRunner::for_program(&case.validation_case.program)?
            .execute(&case.validation_case.program)?;
        let tokenized = tokenizer.tokenize_program_and_hungarian_assignment_frontier(
            &case.validation_case.program,
            case.optimal_assignment.as_slice(),
            case.cost_matrix.as_slice(),
            case.optimal_cost,
        );

        let reconstructed = tokenizer.extract_hungarian_assignment_frontier_outputs(
            tokenized.sequence.as_slice(),
            tokenized.prompt_token_count,
            case.optimal_assignment.len(),
        );

        assert_eq!(reconstructed, execution.outputs);
        Ok(())
    }
}
