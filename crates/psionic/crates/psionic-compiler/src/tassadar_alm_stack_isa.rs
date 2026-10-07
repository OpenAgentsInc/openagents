use psionic_ir::{
    TassadarAlmChannelDecl, TassadarAlmChannelId, TassadarAlmChannelKind, TassadarAlmGate,
    TassadarAlmGraph, TassadarAlmSeedWrite, TassadarAlmValueId, TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable lane identifier for the bounded ALM stack-ISA interpreter.
pub const TASSADAR_ALM_STACK_ISA_LANE_ID: &str = "tassadar.alm_stack_isa_interpreter.v1";
/// Claim boundary for the bounded ALM stack-ISA interpreter lane.
pub const TASSADAR_ALM_STACK_ISA_CLAIM_BOUNDARY: &str = "the ALM stack-ISA interpreter executes \
     a bounded straight-line stack instruction set (push, add, sub, mul, out, halt) with the \
     program in a static seeded channel; it claims integer-exact reference semantics for encoded \
     programs only and makes no branch, loop, call, Wasm, or served-route claim";

/// Program channel id inside interpreter graphs.
pub const TASSADAR_ALM_STACK_ISA_PROGRAM_CHANNEL: TassadarAlmChannelId = TassadarAlmChannelId(0);

/// One bounded straight-line stack-ISA instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarStackIsaInstruction {
    /// Push an immediate value.
    Push(i64),
    /// Pop two values, push `second + top`.
    Add,
    /// Pop two values, push `second - top`.
    Sub,
    /// Pop two values, push `second * top`.
    Mul,
    /// Emit the top of the stack without popping.
    Out,
    /// Do nothing.
    Halt,
}

impl TassadarStackIsaInstruction {
    fn opcode(self) -> i64 {
        match self {
            Self::Push(_) => 0,
            Self::Add => 1,
            Self::Sub => 2,
            Self::Mul => 3,
            Self::Out => 4,
            Self::Halt => 5,
        }
    }

    fn operand(self) -> i64 {
        match self {
            Self::Push(immediate) => immediate,
            _ => 0,
        }
    }
}

/// Encoding or construction failure for one stack-ISA program.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarStackIsaError {
    /// The program is empty.
    #[error("stack-ISA program must contain at least one instruction")]
    EmptyProgram,
    /// The program underflows the stack at one instruction.
    #[error("instruction {index} underflows the stack (depth {depth})")]
    StackUnderflow {
        /// Offending instruction index.
        index: usize,
        /// Stack depth entering the instruction.
        depth: i64,
    },
    /// The program exceeds the declared maximum stack depth.
    #[error("instruction {index} exceeds max stack depth {max_depth}")]
    StackOverflow {
        /// Offending instruction index.
        index: usize,
        /// Declared maximum depth.
        max_depth: i64,
    },
}

/// Validates a program's stack discipline and returns its maximum depth.
pub fn tassadar_stack_isa_validate(
    program: &[TassadarStackIsaInstruction],
    max_depth: i64,
) -> Result<i64, TassadarStackIsaError> {
    if program.is_empty() {
        return Err(TassadarStackIsaError::EmptyProgram);
    }
    let mut depth = 0_i64;
    let mut peak = 0_i64;
    for (index, instruction) in program.iter().enumerate() {
        match instruction {
            TassadarStackIsaInstruction::Push(_) => {
                depth += 1;
                if depth > max_depth {
                    return Err(TassadarStackIsaError::StackOverflow { index, max_depth });
                }
            }
            TassadarStackIsaInstruction::Add
            | TassadarStackIsaInstruction::Sub
            | TassadarStackIsaInstruction::Mul => {
                if depth < 2 {
                    return Err(TassadarStackIsaError::StackUnderflow { index, depth });
                }
                depth -= 1;
            }
            TassadarStackIsaInstruction::Out => {
                if depth < 1 {
                    return Err(TassadarStackIsaError::StackUnderflow { index, depth });
                }
            }
            TassadarStackIsaInstruction::Halt => {}
        }
        peak = peak.max(depth);
    }
    Ok(peak)
}

/// Executes a program on a plain Rust stack machine, returning per-step
/// `(out, depth_after)` rows. The reference leg for interpreter parity.
pub fn tassadar_stack_isa_reference(
    program: &[TassadarStackIsaInstruction],
    max_depth: i64,
) -> Result<Vec<(i64, i64)>, TassadarStackIsaError> {
    tassadar_stack_isa_validate(program, max_depth)?;
    let mut stack: Vec<i64> = Vec::new();
    let mut rows = Vec::with_capacity(program.len());
    for instruction in program {
        let mut out = 0_i64;
        match instruction {
            TassadarStackIsaInstruction::Push(immediate) => stack.push(*immediate),
            TassadarStackIsaInstruction::Add => {
                let top = stack.pop().unwrap_or_default();
                let second = stack.pop().unwrap_or_default();
                stack.push(second + top);
            }
            TassadarStackIsaInstruction::Sub => {
                let top = stack.pop().unwrap_or_default();
                let second = stack.pop().unwrap_or_default();
                stack.push(second - top);
            }
            TassadarStackIsaInstruction::Mul => {
                let top = stack.pop().unwrap_or_default();
                let second = stack.pop().unwrap_or_default();
                stack.push(second * top);
            }
            TassadarStackIsaInstruction::Out => {
                out = stack.last().copied().unwrap_or_default();
            }
            TassadarStackIsaInstruction::Halt => {}
        }
        rows.push((out, stack.len() as i64));
    }
    Ok(rows)
}

struct GraphBuilder {
    gates: Vec<TassadarAlmGate>,
}

impl GraphBuilder {
    fn new() -> Self {
        Self { gates: Vec::new() }
    }

    fn push(&mut self, gate: TassadarAlmGate) -> TassadarAlmValueId {
        self.gates.push(gate);
        TassadarAlmValueId((self.gates.len() - 1) as u32)
    }

    fn constant(&mut self, value: i64) -> TassadarAlmValueId {
        self.push(TassadarAlmGate::Const { value })
    }

    fn linear(&mut self, terms: Vec<(i64, TassadarAlmValueId)>, bias: i64) -> TassadarAlmValueId {
        self.push(TassadarAlmGate::Linear { terms, bias })
    }

    fn reglu(&mut self, value: TassadarAlmValueId, gate: TassadarAlmValueId) -> TassadarAlmValueId {
        self.push(TassadarAlmGate::ReGlu { value, gate })
    }

    fn read(
        &mut self,
        channel_id: TassadarAlmChannelId,
        query: TassadarAlmValueId,
    ) -> TassadarAlmValueId {
        self.push(TassadarAlmGate::ChannelRead { channel_id, query })
    }

    fn cumsum(
        &mut self,
        channel_id: TassadarAlmChannelId,
        value: TassadarAlmValueId,
    ) -> TassadarAlmValueId {
        self.push(TassadarAlmGate::CumSum { channel_id, value })
    }

    /// `1[input >= threshold]` from the two-ReLU step identity.
    fn ge_indicator(
        &mut self,
        one: TassadarAlmValueId,
        input: TassadarAlmValueId,
        threshold: i64,
    ) -> TassadarAlmValueId {
        let shifted_plus = self.linear(vec![(1, input)], 1 - threshold);
        let relu_plus = self.reglu(one, shifted_plus);
        let shifted = self.linear(vec![(1, input)], -threshold);
        let relu = self.reglu(one, shifted);
        self.linear(vec![(1, relu_plus), (-1, relu)], 0)
    }
}

/// Builds the universal interpreter graph for one encoded program and
/// returns it with the step count to run.
pub fn tassadar_alm_stack_isa_interpreter(
    program: &[TassadarStackIsaInstruction],
    max_depth: i64,
) -> Result<(TassadarAlmGraph, usize), TassadarStackIsaError> {
    tassadar_stack_isa_validate(program, max_depth)?;
    let program_channel = TASSADAR_ALM_STACK_ISA_PROGRAM_CHANNEL;
    let stack_channel = TassadarAlmChannelId(1);
    let cursor_channel = TassadarAlmChannelId(2);
    let depth_channel = TassadarAlmChannelId(3);
    let mut builder = GraphBuilder::new();
    let one = builder.constant(1);
    // position = cumsum(1) = t + 1; cursor t = position - 1.
    let position = builder.cumsum(cursor_channel, one);
    let cursor = builder.linear(vec![(1, position)], -1);
    // Instruction fetch from the static program channel.
    let opcode_key = builder.linear(vec![(2, cursor)], 0);
    let operand_key = builder.linear(vec![(2, cursor)], 1);
    let opcode = builder.read(program_channel, opcode_key);
    let operand = builder.read(program_channel, operand_key);
    // Opcode decode: ge_k for k = 1..=5, then one-hot differences.
    let ge1 = builder.ge_indicator(one, opcode, 1);
    let ge2 = builder.ge_indicator(one, opcode, 2);
    let ge3 = builder.ge_indicator(one, opcode, 3);
    let ge4 = builder.ge_indicator(one, opcode, 4);
    let ge5 = builder.ge_indicator(one, opcode, 5);
    let is_push = builder.linear(vec![(-1, ge1)], 1);
    let is_add = builder.linear(vec![(1, ge1), (-1, ge2)], 0);
    let is_sub = builder.linear(vec![(1, ge2), (-1, ge3)], 0);
    let is_mul = builder.linear(vec![(1, ge3), (-1, ge4)], 0);
    let is_out = builder.linear(vec![(1, ge4), (-1, ge5)], 0);
    let is_halt = builder.linear(vec![(1, ge5)], 0);
    // Stack-depth bookkeeping.
    let delta = builder.linear(
        vec![(1, is_push), (-1, is_add), (-1, is_sub), (-1, is_mul)],
        0,
    );
    let depth_after = builder.cumsum(depth_channel, delta);
    let depth_before = builder.linear(vec![(1, depth_after), (-1, delta)], 0);
    // Operand reads from strictly-prior state: top and second.
    let top = builder.read(stack_channel, depth_before);
    let second_key = builder.linear(vec![(1, depth_before)], -1);
    let second = builder.read(stack_channel, second_key);
    // Arithmetic results.
    let sum = builder.linear(vec![(1, second), (1, top)], 0);
    let difference = builder.linear(vec![(1, second), (-1, top)], 0);
    // product = second * relu(top) - second * relu(-top).
    let product_positive = builder.reglu(second, top);
    let negated_top = builder.linear(vec![(-1, top)], 0);
    let product_negative = builder.reglu(second, negated_top);
    let product = builder.linear(vec![(1, product_positive), (-1, product_negative)], 0);
    // Masked write value: exactly one mask is hot per step.
    let write_push = builder.reglu(operand, is_push);
    let write_add = builder.reglu(sum, is_add);
    let write_sub = builder.reglu(difference, is_sub);
    let write_mul = builder.reglu(product, is_mul);
    let keep_gate = builder.linear(vec![(1, is_out), (1, is_halt)], 0);
    let write_keep = builder.reglu(top, keep_gate);
    let written = builder.linear(
        vec![
            (1, write_push),
            (1, write_add),
            (1, write_sub),
            (1, write_mul),
            (1, write_keep),
        ],
        0,
    );
    builder.push(TassadarAlmGate::ChannelWrite {
        channel_id: stack_channel,
        key: depth_after,
        value: written,
    });
    let out = builder.reglu(top, is_out);
    let gates = builder.gates;
    // Seed writes: the encoded program plus the stack floor.
    let mut seed_writes: Vec<TassadarAlmSeedWrite> = Vec::new();
    for (index, instruction) in program.iter().enumerate() {
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: program_channel,
            key: 2 * index as i64,
            value: instruction.opcode(),
        });
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: program_channel,
            key: 2 * index as i64 + 1,
            value: instruction.operand(),
        });
    }
    for key in -1..=max_depth {
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: stack_channel,
            key,
            value: 0,
        });
    }
    let graph = TassadarAlmGraph {
        schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
        graph_id: format!(
            "{TASSADAR_ALM_STACK_ISA_LANE_ID}.program_len_{}",
            program.len()
        ),
        input_field_count: 1,
        channels: vec![
            TassadarAlmChannelDecl {
                channel_id: program_channel,
                name: "program".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            },
            TassadarAlmChannelDecl {
                channel_id: stack_channel,
                name: "stack".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            },
            TassadarAlmChannelDecl {
                channel_id: cursor_channel,
                name: "cursor".to_string(),
                kind: TassadarAlmChannelKind::Accumulator,
            },
            TassadarAlmChannelDecl {
                channel_id: depth_channel,
                name: "depth".to_string(),
                kind: TassadarAlmChannelKind::Accumulator,
            },
        ],
        seed_writes,
        gates,
        outputs: vec![out, depth_after],
    };
    Ok((graph, program.len()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use psionic_ir::TassadarAlmEvaluator;

    use super::*;
    use crate::tassadar_alm_backend::{compile_tassadar_alm_graph, TassadarAlmCompiledExecutor};
    use crate::tassadar_alm_specializer::specialize_tassadar_alm_graph;

    fn arithmetic_program() -> Vec<TassadarStackIsaInstruction> {
        // (3 + 5) * 2 - 4 = 12, emitted twice around a halt.
        use TassadarStackIsaInstruction as I;
        vec![
            I::Push(3),
            I::Push(5),
            I::Add,
            I::Push(2),
            I::Mul,
            I::Push(4),
            I::Sub,
            I::Out,
            I::Halt,
            I::Out,
        ]
    }

    fn run_graph(graph: &TassadarAlmGraph, steps: usize) -> Vec<(i64, i64)> {
        let inputs = vec![vec![0_i64]; steps];
        let trace = TassadarAlmEvaluator::evaluate(graph, &inputs).expect("evaluates");
        trace
            .step_outputs
            .iter()
            .map(|row| (row[0], row[1]))
            .collect()
    }

    fn run_compiled(graph: &TassadarAlmGraph, steps: usize) -> Vec<(i64, i64)> {
        let bundle = compile_tassadar_alm_graph(graph).expect("compiles");
        let inputs = vec![vec![0_i64]; steps];
        let trace = TassadarAlmCompiledExecutor::execute(&bundle, &inputs).expect("executes");
        trace
            .step_outputs
            .iter()
            .map(|row| (row[0], row[1]))
            .collect()
    }

    #[test]
    fn universal_interpreter_matches_the_rust_reference() {
        let program = arithmetic_program();
        let reference = tassadar_stack_isa_reference(&program, 4).expect("references");
        let (graph, steps) = tassadar_alm_stack_isa_interpreter(&program, 4).expect("builds");
        assert_eq!(run_graph(&graph, steps), reference);
        // The OUT rows carry 12.
        assert_eq!(reference[7], (12, 1));
        assert_eq!(reference[9], (12, 1));
    }

    #[test]
    fn six_way_agreement_universal_specialized_evaluated_compiled() {
        let program = arithmetic_program();
        let reference = tassadar_stack_isa_reference(&program, 4).expect("references");
        let (universal, steps) = tassadar_alm_stack_isa_interpreter(&program, 4).expect("builds");
        let (specialized, report) =
            specialize_tassadar_alm_graph(&universal, TASSADAR_ALM_STACK_ISA_PROGRAM_CHANNEL)
                .expect("specializes");
        assert_eq!(report.rewritten_reads, 2);
        // The program channel is gone from the specialized graph.
        assert!(!specialized
            .channels
            .iter()
            .any(|decl| decl.channel_id == TASSADAR_ALM_STACK_ISA_PROGRAM_CHANNEL));
        let legs = [
            run_graph(&universal, steps),
            run_graph(&specialized, steps),
            run_compiled(&universal, steps),
            run_compiled(&specialized, steps),
        ];
        for leg in &legs {
            assert_eq!(leg, &reference);
        }
    }

    #[test]
    fn negative_and_zero_operands_survive_the_masked_multiply() {
        use TassadarStackIsaInstruction as I;
        let program = vec![
            I::Push(-7),
            I::Push(3),
            I::Mul,
            I::Out,
            I::Push(0),
            I::Mul,
            I::Out,
        ];
        let reference = tassadar_stack_isa_reference(&program, 4).expect("references");
        let (graph, steps) = tassadar_alm_stack_isa_interpreter(&program, 4).expect("builds");
        assert_eq!(run_graph(&graph, steps), reference);
        assert_eq!(reference[3].0, -21);
        assert_eq!(reference[6].0, 0);
    }

    #[test]
    fn encoder_refuses_underflow_overflow_and_empty_programs() {
        use TassadarStackIsaInstruction as I;
        assert_eq!(
            tassadar_stack_isa_validate(&[], 4),
            Err(TassadarStackIsaError::EmptyProgram)
        );
        assert_eq!(
            tassadar_stack_isa_validate(&[I::Add], 4),
            Err(TassadarStackIsaError::StackUnderflow { index: 0, depth: 0 })
        );
        assert_eq!(
            tassadar_stack_isa_validate(&[I::Out], 4),
            Err(TassadarStackIsaError::StackUnderflow { index: 0, depth: 0 })
        );
        assert_eq!(
            tassadar_stack_isa_validate(&[I::Push(1), I::Push(2), I::Push(3)], 2),
            Err(TassadarStackIsaError::StackOverflow {
                index: 2,
                max_depth: 2
            })
        );
    }

    #[test]
    fn deep_expression_uses_the_full_declared_stack() {
        use TassadarStackIsaInstruction as I;
        // ((1+2) * (3+4)) + (5*6) = 21 + 30 = 51.
        let program = vec![
            I::Push(1),
            I::Push(2),
            I::Add,
            I::Push(3),
            I::Push(4),
            I::Add,
            I::Mul,
            I::Push(5),
            I::Push(6),
            I::Mul,
            I::Add,
            I::Out,
        ];
        let reference = tassadar_stack_isa_reference(&program, 3).expect("references");
        let (graph, steps) = tassadar_alm_stack_isa_interpreter(&program, 3).expect("builds");
        let rows = run_graph(&graph, steps);
        assert_eq!(rows, reference);
        assert_eq!(rows[11].0, 51);
    }
}
