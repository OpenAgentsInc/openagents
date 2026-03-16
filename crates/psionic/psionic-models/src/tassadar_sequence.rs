use std::collections::BTreeMap;

use psionic_runtime::{
    TassadarArithmeticOp, TassadarExecution, TassadarHaltReason, TassadarInstruction,
    TassadarProgram, TassadarTraceEvent, TassadarTraceStep,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
const OP_I32_LOAD: &str = "<op_i32_load>";
const OP_I32_STORE: &str = "<op_i32_store>";
const OP_BR_IF: &str = "<op_br_if>";
const OP_OUTPUT: &str = "<op_output>";
const OP_RETURN: &str = "<op_return>";

const EVENT_CONST_PUSH: &str = "<event_const_push>";
const EVENT_LOCAL_GET: &str = "<event_local_get>";
const EVENT_LOCAL_SET: &str = "<event_local_set>";
const EVENT_BINARY_ADD: &str = "<event_binary_add>";
const EVENT_BINARY_SUB: &str = "<event_binary_sub>";
const EVENT_BINARY_MUL: &str = "<event_binary_mul>";
const EVENT_LOAD: &str = "<event_load>";
const EVENT_STORE: &str = "<event_store>";
const EVENT_BRANCH: &str = "<event_branch>";
const EVENT_OUTPUT: &str = "<event_output>";
const EVENT_RETURN: &str = "<event_return>";

const HALT_RETURNED: &str = "<halt_returned>";
const HALT_FELL_OFF_END: &str = "<halt_fell_off_end>";

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
        tokens.push(self.vocabulary.bos_id());
        tokens.push(self.token_id(PROGRAM_TOKEN));
        tokens.push(self.token_id(FIELD_LOCALS));
        self.push_u32(&mut tokens, program.local_count as u32);
        tokens.push(self.token_id(FIELD_MEMORY_SLOTS));
        self.push_u32(&mut tokens, program.memory_slots as u32);
        tokens.push(self.token_id(FIELD_INITIAL_MEMORY));
        self.push_u32(&mut tokens, program.initial_memory.len() as u32);
        for value in &program.initial_memory {
            self.push_i32(&mut tokens, *value);
        }
        for instruction in &program.instructions {
            self.push_instruction(&mut tokens, instruction);
        }
        tokens.push(self.token_id(TRACE_TOKEN));
        let prompt_token_count = tokens.len();
        for step in &execution.steps {
            self.push_step(&mut tokens, step);
        }
        tokens.push(self.token_id(HALT_TOKEN));
        self.push_halt_reason(&mut tokens, execution.halt_reason);
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

    fn push_instruction(&self, tokens: &mut Vec<TokenId>, instruction: &TassadarInstruction) {
        match instruction {
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
            TassadarInstruction::I32Add => tokens.push(self.token_id(OP_I32_ADD)),
            TassadarInstruction::I32Sub => tokens.push(self.token_id(OP_I32_SUB)),
            TassadarInstruction::I32Mul => tokens.push(self.token_id(OP_I32_MUL)),
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

#[cfg(test)]
mod tests {
    use crate::TokenizerBoundary;
    use psionic_runtime::{TassadarCpuReferenceRunner, tassadar_sudoku_v0_corpus};

    use super::TassadarTraceTokenizer;

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
    fn tokenizer_digest_is_stable() {
        let tokenizer = TassadarTraceTokenizer::new();

        assert_eq!(tokenizer.stable_digest(), tokenizer.stable_digest());
        assert!(tokenizer.vocabulary().len() > 256);
    }
}
