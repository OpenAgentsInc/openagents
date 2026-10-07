use psionic_ir::{
    TASSADAR_ALM_GRAPH_SCHEMA_VERSION, TassadarAlmChannelDecl, TassadarAlmChannelId,
    TassadarAlmChannelKind, TassadarAlmGate, TassadarAlmGraph, TassadarAlmSeedWrite,
    TassadarAlmValueId,
};
use psionic_runtime::{TassadarInstruction, TassadarProgram};
use thiserror::Error;

/// Stable lane identifier for the branch-capable ALM window interpreter.
pub const TASSADAR_ALM_WASM_INTERPRETER_LANE_ID: &str = "tassadar.alm_wasm_interpreter.v1";
/// Claim boundary for the branch-capable ALM window interpreter.
pub const TASSADAR_ALM_WASM_INTERPRETER_CLAIM_BOUNDARY: &str = "the ALM window interpreter \
     executes the bounded W1.1 twenty-one-opcode Tassadar i32 window (const, local get/set/tee, \
     drop, add, sub, mul, lt, eqz, eq, ne, gt, le, ge, load, store, br_if, output, return, nop) \
     under a fixed step budget with the program in \
     a static specializable channel; parity is claimed only for programs the CPU reference \
     runner accepts, because the gate graph yields seeded zeros where the runner refuses \
     malformed stack discipline; integer-exact, no f32, no serving";

/// Program channel id inside interpreter graphs (static, specializable).
pub const TASSADAR_ALM_WASM_PROGRAM_CHANNEL: TassadarAlmChannelId = TassadarAlmChannelId(0);

const STACK_CHANNEL: TassadarAlmChannelId = TassadarAlmChannelId(1);
const LOCALS_CHANNEL: TassadarAlmChannelId = TassadarAlmChannelId(2);
const MEMORY_CHANNEL: TassadarAlmChannelId = TassadarAlmChannelId(3);
const STATE_CHANNEL: TassadarAlmChannelId = TassadarAlmChannelId(4);
const STATE_KEY_PC: i64 = 0;
const STATE_KEY_DEPTH: i64 = 1;
const STATE_KEY_HALTED: i64 = 2;
const SINK_BIAS: i64 = -1_000;
const MAX_W1_1_OPCODE: usize = 20;

/// Conversion failure for one Tassadar program.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarAlmWasmInterpreterError {
    /// A branch target lies beyond the virtual return padding.
    #[error("instruction {index} branches to {target} beyond program length {length}")]
    BranchTargetOutOfRange {
        /// Offending instruction index.
        index: usize,
        /// Branch target.
        target: usize,
        /// Program length.
        length: usize,
    },
    /// A local index is outside the declared local count.
    #[error("instruction {index} uses local {local} outside {declared} locals")]
    LocalOutOfRange {
        /// Offending instruction index.
        index: usize,
        /// Local index.
        local: u8,
        /// Declared local count.
        declared: usize,
    },
    /// A memory slot is outside the declared slot count.
    #[error("instruction {index} uses slot {slot} outside {declared} slots")]
    SlotOutOfRange {
        /// Offending instruction index.
        index: usize,
        /// Memory slot.
        slot: u8,
        /// Declared slot count.
        declared: usize,
    },
}

fn encode(instruction: &TassadarInstruction) -> (i64, i64) {
    match instruction {
        TassadarInstruction::Nop => (12, 0),
        TassadarInstruction::I32Const { value } => (0, i64::from(*value)),
        TassadarInstruction::LocalGet { local } => (1, i64::from(*local)),
        TassadarInstruction::LocalSet { local } => (2, i64::from(*local)),
        TassadarInstruction::LocalTee { local } => (13, i64::from(*local)),
        TassadarInstruction::Drop => (14, 0),
        TassadarInstruction::I32Add => (3, 0),
        TassadarInstruction::I32Sub => (4, 0),
        TassadarInstruction::I32Mul => (5, 0),
        TassadarInstruction::I32Lt => (6, 0),
        TassadarInstruction::I32Eqz => (15, 0),
        TassadarInstruction::I32Eq => (16, 0),
        TassadarInstruction::I32Ne => (17, 0),
        TassadarInstruction::I32Gt => (18, 0),
        TassadarInstruction::I32Le => (19, 0),
        TassadarInstruction::I32Ge => (20, 0),
        TassadarInstruction::I32Load { slot } => (7, i64::from(*slot)),
        TassadarInstruction::I32Store { slot } => (8, i64::from(*slot)),
        TassadarInstruction::BrIf { target_pc } => (9, i64::from(*target_pc)),
        TassadarInstruction::Output => (10, 0),
        TassadarInstruction::Return => (11, 0),
    }
}

struct Builder {
    gates: Vec<TassadarAlmGate>,
    one: TassadarAlmValueId,
}

impl Builder {
    fn new() -> Self {
        let gates = vec![TassadarAlmGate::Const { value: 1 }];
        Self {
            gates,
            one: TassadarAlmValueId(0),
        }
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

    fn write(
        &mut self,
        channel_id: TassadarAlmChannelId,
        key: TassadarAlmValueId,
        value: TassadarAlmValueId,
    ) {
        self.push(TassadarAlmGate::ChannelWrite {
            channel_id,
            key,
            value,
        });
    }

    /// `1[input >= threshold]` from the two-ReLU step identity.
    fn ge(&mut self, input: TassadarAlmValueId, threshold: i64) -> TassadarAlmValueId {
        let one = self.one;
        let shifted_plus = self.linear(vec![(1, input)], 1 - threshold);
        let relu_plus = self.reglu(one, shifted_plus);
        let shifted = self.linear(vec![(1, input)], -threshold);
        let relu = self.reglu(one, shifted);
        self.linear(vec![(1, relu_plus), (-1, relu)], 0)
    }

    /// Masked write key: `active ? key : SINK_BIAS`.
    fn masked_key(
        &mut self,
        key: TassadarAlmValueId,
        active: TassadarAlmValueId,
    ) -> TassadarAlmValueId {
        let gated = self.reglu(key, active);
        self.linear(vec![(1, gated), (SINK_BIAS.abs(), active)], SINK_BIAS)
    }
}

/// Builds the branch-capable interpreter graph for one validated Tassadar
/// program. The caller supplies the step budget at execution time; the
/// graph's per-step outputs are `(out_flag, out_value, pc, depth, halted)`.
pub fn tassadar_alm_wasm_interpreter(
    program: &TassadarProgram,
) -> Result<TassadarAlmGraph, TassadarAlmWasmInterpreterError> {
    let length = program.instructions.len();
    for (index, instruction) in program.instructions.iter().enumerate() {
        match instruction {
            TassadarInstruction::BrIf { target_pc } => {
                if usize::from(*target_pc) > length {
                    return Err(TassadarAlmWasmInterpreterError::BranchTargetOutOfRange {
                        index,
                        target: usize::from(*target_pc),
                        length,
                    });
                }
            }
            TassadarInstruction::LocalGet { local }
            | TassadarInstruction::LocalSet { local }
            | TassadarInstruction::LocalTee { local } => {
                if usize::from(*local) >= program.local_count {
                    return Err(TassadarAlmWasmInterpreterError::LocalOutOfRange {
                        index,
                        local: *local,
                        declared: program.local_count,
                    });
                }
            }
            TassadarInstruction::I32Load { slot } | TassadarInstruction::I32Store { slot } => {
                if usize::from(*slot) >= program.memory_slots {
                    return Err(TassadarAlmWasmInterpreterError::SlotOutOfRange {
                        index,
                        slot: *slot,
                        declared: program.memory_slots,
                    });
                }
            }
            _ => {}
        }
    }
    let mut b = Builder::new();
    // Machine state from the previous step.
    let k_pc = b.constant(STATE_KEY_PC);
    let k_depth = b.constant(STATE_KEY_DEPTH);
    let k_halted = b.constant(STATE_KEY_HALTED);
    let pc = b.read(STATE_CHANNEL, k_pc);
    let depth = b.read(STATE_CHANNEL, k_depth);
    let halted = b.read(STATE_CHANNEL, k_halted);
    let not_halted = b.linear(vec![(-1, halted)], 1);
    // Instruction fetch.
    let op_key = b.linear(vec![(2, pc)], 0);
    let arg_key = b.linear(vec![(2, pc)], 1);
    let op = b.read(TASSADAR_ALM_WASM_PROGRAM_CHANNEL, op_key);
    let arg = b.read(TASSADAR_ALM_WASM_PROGRAM_CHANNEL, arg_key);
    // Decode: ge thresholds 1..=MAX_W1_1_OPCODE then one-hot differences.
    let ge: Vec<TassadarAlmValueId> = (1..=MAX_W1_1_OPCODE).map(|k| b.ge(op, k as i64)).collect();
    let is_const = b.linear(vec![(-1, ge[0])], 1);
    let mut is: Vec<TassadarAlmValueId> = vec![is_const];
    for k in 1..MAX_W1_1_OPCODE {
        is.push(b.linear(vec![(1, ge[k - 1]), (-1, ge[k])], 0));
    }
    is.push(b.linear(vec![(1, ge[MAX_W1_1_OPCODE - 1])], 0));
    // Stack reads from prior-step state.
    let top = b.read(STACK_CHANNEL, depth);
    let second_key = b.linear(vec![(1, depth)], -1);
    let second = b.read(STACK_CHANNEL, second_key);
    // Masked locals/memory reads (key 0 when inactive; key 0 is seeded).
    let local_read_gate = b.linear(vec![(1, is[1])], 0);
    let local_key = b.reglu(arg, local_read_gate);
    let local_value = b.read(LOCALS_CHANNEL, local_key);
    let memory_read_gate = b.linear(vec![(1, is[7])], 0);
    let memory_key = b.reglu(arg, memory_read_gate);
    let memory_value = b.read(MEMORY_CHANNEL, memory_key);
    // Arithmetic over (second = left, top = right).
    let sum = b.linear(vec![(1, second), (1, top)], 0);
    let diff = b.linear(vec![(1, second), (-1, top)], 0);
    let neg_top = b.linear(vec![(-1, top)], 0);
    let prod_pos = b.reglu(second, top);
    let prod_neg = b.reglu(second, neg_top);
    let prod = b.linear(vec![(1, prod_pos), (-1, prod_neg)], 0);
    // i32.lt: 1[left < right] = 1[top - second >= 1].
    let lt_input = b.linear(vec![(1, top), (-1, second)], 0);
    let lt_value = b.ge(lt_input, 1);
    let neg_diff = b.linear(vec![(-1, diff)], 0);
    let diff_nz_pos = b.ge(diff, 1);
    let diff_nz_neg = b.ge(neg_diff, 1);
    let diff_nonzero = b.linear(vec![(1, diff_nz_pos), (1, diff_nz_neg)], 0);
    let eq_value = b.linear(vec![(-1, diff_nonzero)], 1);
    let ne_value = diff_nonzero;
    let gt_value = b.ge(diff, 1);
    let le_value = b.linear(vec![(-1, gt_value)], 1);
    let ge_value = b.ge(diff, 0);
    // Branch condition: top != 0.
    let nz_pos = b.ge(top, 1);
    let nz_neg = b.ge(neg_top, 1);
    let nonzero = b.linear(vec![(1, nz_pos), (1, nz_neg)], 0);
    let eqz_value = b.linear(vec![(-1, nonzero)], 1);
    let taken = b.reglu(nonzero, is[9]);
    // Stack-depth bookkeeping.
    let delta_raw = b.linear(
        vec![
            (1, is[0]),
            (1, is[1]),
            (1, is[7]),
            (-1, is[2]),
            (-1, is[8]),
            (-1, is[9]),
            (-1, is[10]),
            (-1, is[14]),
            (-1, is[3]),
            (-1, is[4]),
            (-1, is[5]),
            (-1, is[6]),
            (-1, is[16]),
            (-1, is[17]),
            (-1, is[18]),
            (-1, is[19]),
            (-1, is[20]),
        ],
        0,
    );
    let delta = b.reglu(delta_raw, not_halted);
    let new_depth = b.linear(vec![(1, depth), (1, delta)], 0);
    // Program counter: pc + not_halted * (1 + taken * (target - pc - 1)).
    let displacement = b.linear(vec![(1, arg), (-1, pc)], -1);
    let branch_add = b.reglu(displacement, taken);
    let increment_raw = b.linear(vec![(1, branch_add)], 1);
    let increment = b.reglu(increment_raw, not_halted);
    let new_pc = b.linear(vec![(1, pc), (1, increment)], 0);
    // Sticky halt: halted OR is_return.
    let halt_and = b.reglu(halted, is[11]);
    let new_halted = b.linear(vec![(1, halted), (1, is[11]), (-1, halt_and)], 0);
    // Stack write (pushes and binary results land on the new top).
    let stack_active_raw = b.linear(
        vec![
            (1, is[0]),
            (1, is[1]),
            (1, is[7]),
            (1, is[3]),
            (1, is[4]),
            (1, is[5]),
            (1, is[6]),
            (1, is[15]),
            (1, is[16]),
            (1, is[17]),
            (1, is[18]),
            (1, is[19]),
            (1, is[20]),
        ],
        0,
    );
    let stack_active = b.reglu(stack_active_raw, not_halted);
    let w_const = b.reglu(arg, is[0]);
    let w_get = b.reglu(local_value, is[1]);
    let w_load = b.reglu(memory_value, is[7]);
    let w_add = b.reglu(sum, is[3]);
    let w_sub = b.reglu(diff, is[4]);
    let w_mul = b.reglu(prod, is[5]);
    let w_lt = b.reglu(lt_value, is[6]);
    let w_eqz = b.reglu(eqz_value, is[15]);
    let w_eq = b.reglu(eq_value, is[16]);
    let w_ne = b.reglu(ne_value, is[17]);
    let w_gt = b.reglu(gt_value, is[18]);
    let w_le = b.reglu(le_value, is[19]);
    let w_ge = b.reglu(ge_value, is[20]);
    let stack_value = b.linear(
        vec![
            (1, w_const),
            (1, w_get),
            (1, w_load),
            (1, w_add),
            (1, w_sub),
            (1, w_mul),
            (1, w_lt),
            (1, w_eqz),
            (1, w_eq),
            (1, w_ne),
            (1, w_gt),
            (1, w_le),
            (1, w_ge),
        ],
        0,
    );
    let stack_key = b.masked_key(new_depth, stack_active);
    b.write(STACK_CHANNEL, stack_key, stack_value);
    // Locals write (local.set pops top into the local).
    let set_active_raw = b.linear(vec![(1, is[2]), (1, is[13])], 0);
    let set_active = b.reglu(set_active_raw, not_halted);
    let set_key_raw = b.reglu(arg, set_active);
    let set_key = b.masked_key(set_key_raw, set_active);
    let set_value = b.reglu(top, set_active);
    b.write(LOCALS_CHANNEL, set_key, set_value);
    // Memory write (i32.store pops top into the slot).
    let store_active = b.reglu(is[8], not_halted);
    let store_key_raw = b.reglu(arg, store_active);
    let store_key = b.masked_key(store_key_raw, store_active);
    let store_value = b.reglu(top, store_active);
    b.write(MEMORY_CHANNEL, store_key, store_value);
    // State writes.
    b.write(STATE_CHANNEL, k_pc, new_pc);
    b.write(STATE_CHANNEL, k_depth, new_depth);
    b.write(STATE_CHANNEL, k_halted, new_halted);
    // Outputs.
    let out_flag = b.reglu(is[10], not_halted);
    let out_value = b.reglu(top, out_flag);
    let outputs = vec![out_flag, out_value, new_pc, new_depth, new_halted];
    // Seeds.
    let mut seed_writes: Vec<TassadarAlmSeedWrite> = Vec::new();
    for (index, instruction) in program.instructions.iter().enumerate() {
        let (opcode, operand) = encode(instruction);
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: TASSADAR_ALM_WASM_PROGRAM_CHANNEL,
            key: 2 * index as i64,
            value: opcode,
        });
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: TASSADAR_ALM_WASM_PROGRAM_CHANNEL,
            key: 2 * index as i64 + 1,
            value: operand,
        });
    }
    // Virtual returns at pc = len and len + 1 make fall-off-the-end halt
    // identically to the runner's FellOffEnd, and keep the frozen
    // post-halt pc fetchable.
    for pad in 0..2 {
        let pc_pad = (length + pad) as i64;
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: TASSADAR_ALM_WASM_PROGRAM_CHANNEL,
            key: 2 * pc_pad,
            value: 11,
        });
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: TASSADAR_ALM_WASM_PROGRAM_CHANNEL,
            key: 2 * pc_pad + 1,
            value: 0,
        });
    }
    for key in -1..=0 {
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: STACK_CHANNEL,
            key,
            value: 0,
        });
    }
    for local in 0..program.local_count.max(1) {
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: LOCALS_CHANNEL,
            key: local as i64,
            value: 0,
        });
    }
    for slot in 0..program.memory_slots.max(1) {
        let value = program
            .initial_memory
            .get(slot)
            .copied()
            .map_or(0, i64::from);
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: MEMORY_CHANNEL,
            key: slot as i64,
            value,
        });
    }
    for (key, value) in [
        (STATE_KEY_PC, 0),
        (STATE_KEY_DEPTH, 0),
        (STATE_KEY_HALTED, 0),
    ] {
        seed_writes.push(TassadarAlmSeedWrite {
            channel_id: STATE_CHANNEL,
            key,
            value,
        });
    }
    Ok(TassadarAlmGraph {
        schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
        graph_id: format!(
            "{TASSADAR_ALM_WASM_INTERPRETER_LANE_ID}.{}",
            program.program_id
        ),
        input_field_count: 1,
        channels: vec![
            TassadarAlmChannelDecl {
                channel_id: TASSADAR_ALM_WASM_PROGRAM_CHANNEL,
                name: "program".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            },
            TassadarAlmChannelDecl {
                channel_id: STACK_CHANNEL,
                name: "stack".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            },
            TassadarAlmChannelDecl {
                channel_id: LOCALS_CHANNEL,
                name: "locals".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            },
            TassadarAlmChannelDecl {
                channel_id: MEMORY_CHANNEL,
                name: "memory".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            },
            TassadarAlmChannelDecl {
                channel_id: STATE_CHANNEL,
                name: "state".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            },
        ],
        seed_writes,
        gates: b.gates,
        outputs,
    })
}

/// Collects emitted outputs and the halt flag from interpreter step rows.
#[must_use]
pub fn tassadar_alm_wasm_collect(step_outputs: &[Vec<i64>]) -> (Vec<i64>, bool) {
    let outputs = step_outputs
        .iter()
        .filter(|row| row[0] == 1)
        .map(|row| row[1])
        .collect();
    let halted = step_outputs.last().is_some_and(|row| row[4] >= 1);
    (outputs, halted)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use psionic_ir::TassadarAlmEvaluator;
    use psionic_runtime::{TassadarCpuReferenceRunner, TassadarWasmProfile};

    use super::*;
    use crate::tassadar_alm_backend::{TassadarAlmCompiledExecutor, compile_tassadar_alm_graph};
    use crate::tassadar_alm_specializer::specialize_tassadar_alm_graph;

    fn run_interpreter(graph: &TassadarAlmGraph, budget: usize) -> (Vec<i64>, bool) {
        let inputs = vec![vec![0_i64]; budget];
        let trace = TassadarAlmEvaluator::evaluate(graph, &inputs).expect("evaluates");
        tassadar_alm_wasm_collect(&trace.step_outputs)
    }

    fn reference_outputs(program: &TassadarProgram) -> Vec<i64> {
        let runner = TassadarCpuReferenceRunner::for_program(program).expect("runner");
        let execution = runner.execute(program).expect("executes");
        execution.outputs.iter().map(|v| i64::from(*v)).collect()
    }

    fn straight_line_program() -> TassadarProgram {
        use TassadarInstruction as I;
        TassadarProgram::new(
            "alm_wasm.straight_line",
            &TassadarWasmProfile::core_i32_v2(),
            1,
            1,
            vec![
                I::I32Const { value: 6 },
                I::I32Const { value: 7 },
                I::I32Mul,
                I::Output,
                I::Return,
            ],
        )
    }

    fn loop_sum_program() -> TassadarProgram {
        use TassadarInstruction as I;
        // acc = 0; i = 1; do { acc += i; i += 1 } while (i < 6); output acc.
        TassadarProgram::new(
            "alm_wasm.loop_sum",
            &TassadarWasmProfile::article_i32_compute_v1(),
            2,
            1,
            vec![
                I::I32Const { value: 0 },
                I::LocalSet { local: 0 },
                I::I32Const { value: 1 },
                I::LocalSet { local: 1 },
                I::LocalGet { local: 0 },
                I::LocalGet { local: 1 },
                I::I32Add,
                I::LocalSet { local: 0 },
                I::LocalGet { local: 1 },
                I::I32Const { value: 1 },
                I::I32Add,
                I::LocalSet { local: 1 },
                I::LocalGet { local: 1 },
                I::I32Const { value: 6 },
                I::I32Lt,
                I::BrIf { target_pc: 4 },
                I::LocalGet { local: 0 },
                I::Output,
                I::Return,
            ],
        )
    }

    fn w1_1_window_program() -> TassadarProgram {
        use TassadarInstruction as I;
        TassadarProgram::new(
            "alm_wasm.w1_1_window",
            &TassadarWasmProfile::core_i32_w1_1_v1(),
            1,
            1,
            vec![
                I::Nop,
                I::I32Const { value: 5 },
                I::LocalTee { local: 0 },
                I::Drop,
                I::LocalGet { local: 0 },
                I::I32Eqz,
                I::I32Const { value: 0 },
                I::I32Eq,
                I::I32Const { value: 7 },
                I::I32Const { value: 3 },
                I::I32Gt,
                I::I32Add,
                I::I32Const { value: 2 },
                I::I32Le,
                I::I32Const { value: 1 },
                I::I32Ge,
                I::I32Const { value: 0 },
                I::I32Ne,
                I::Output,
                I::Return,
            ],
        )
    }

    #[test]
    fn straight_line_matches_the_cpu_reference_runner() {
        let program = straight_line_program();
        let graph = tassadar_alm_wasm_interpreter(&program).expect("builds");
        let (outputs, halted) = run_interpreter(&graph, 10);
        assert!(halted);
        assert_eq!(outputs, vec![42]);
        assert_eq!(outputs, reference_outputs(&program));
    }

    #[test]
    fn backward_branch_loop_matches_the_cpu_reference_runner() {
        let program = loop_sum_program();
        let graph = tassadar_alm_wasm_interpreter(&program).expect("builds");
        let (outputs, halted) = run_interpreter(&graph, 100);
        assert!(halted);
        assert_eq!(outputs, vec![15]);
        assert_eq!(outputs, reference_outputs(&program));
    }

    #[test]
    fn w1_1_opcode_window_matches_the_cpu_reference_runner() {
        let program = w1_1_window_program();
        let graph = tassadar_alm_wasm_interpreter(&program).expect("builds");
        let (outputs, halted) = run_interpreter(&graph, 32);
        assert!(halted);
        assert_eq!(outputs, vec![1]);
        assert_eq!(outputs, reference_outputs(&program));
        assert_eq!(
            TassadarWasmProfile::core_i32_w1_1_v1()
                .allowed_opcodes
                .len(),
            21
        );
    }

    #[test]
    fn forward_conditional_skip_matches_both_arms() {
        use TassadarInstruction as I;
        for (condition, expected) in [(1, 222), (0, 111)] {
            let program = TassadarProgram::new(
                format!("alm_wasm.skip_{condition}"),
                &TassadarWasmProfile::core_i32_v2(),
                1,
                1,
                vec![
                    I::I32Const { value: condition },
                    I::BrIf { target_pc: 5 },
                    I::I32Const { value: 111 },
                    I::Output,
                    I::Return,
                    I::I32Const { value: 222 },
                    I::Output,
                    I::Return,
                ],
            );
            let graph = tassadar_alm_wasm_interpreter(&program).expect("builds");
            let (outputs, halted) = run_interpreter(&graph, 12);
            assert!(halted);
            assert_eq!(outputs, vec![expected]);
            assert_eq!(outputs, reference_outputs(&program));
        }
    }

    #[test]
    fn memory_roundtrip_with_initial_memory_matches_the_runner() {
        use TassadarInstruction as I;
        let mut program = TassadarProgram::new(
            "alm_wasm.memory_roundtrip",
            &TassadarWasmProfile::core_i32_v2(),
            1,
            2,
            vec![
                I::I32Load { slot: 0 },
                I::I32Const { value: 5 },
                I::I32Add,
                I::I32Store { slot: 1 },
                I::I32Load { slot: 1 },
                I::Output,
                I::Return,
            ],
        );
        program.initial_memory = vec![37, 0];
        let graph = tassadar_alm_wasm_interpreter(&program).expect("builds");
        let (outputs, halted) = run_interpreter(&graph, 12);
        assert!(halted);
        assert_eq!(outputs, vec![42]);
        assert_eq!(outputs, reference_outputs(&program));
    }

    #[test]
    fn fall_off_the_end_halts_like_the_runner() {
        use TassadarInstruction as I;
        let program = TassadarProgram::new(
            "alm_wasm.fall_off",
            &TassadarWasmProfile::core_i32_v2(),
            1,
            1,
            vec![I::I32Const { value: 9 }, I::Output],
        );
        let graph = tassadar_alm_wasm_interpreter(&program).expect("builds");
        let (outputs, halted) = run_interpreter(&graph, 10);
        assert!(halted);
        assert_eq!(outputs, vec![9]);
        assert_eq!(outputs, reference_outputs(&program));
    }

    #[test]
    fn compiled_and_specialized_legs_match_the_runner_on_the_loop() {
        let program = loop_sum_program();
        let graph = tassadar_alm_wasm_interpreter(&program).expect("builds");
        let expected = reference_outputs(&program);
        let budget = 100;
        let inputs = vec![vec![0_i64]; budget];
        // Compiled row execution.
        let bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        let compiled = TassadarAlmCompiledExecutor::execute(&bundle, &inputs).expect("executes");
        let (compiled_outputs, compiled_halted) = tassadar_alm_wasm_collect(&compiled.step_outputs);
        assert!(compiled_halted);
        assert_eq!(compiled_outputs, expected);
        // Program channel baked into gate structure, then compiled again.
        let (specialized, report) =
            specialize_tassadar_alm_graph(&graph, TASSADAR_ALM_WASM_PROGRAM_CHANNEL)
                .expect("specializes");
        assert_eq!(report.rewritten_reads, 2);
        let specialized_bundle = compile_tassadar_alm_graph(&specialized).expect("compiles");
        let specialized_run =
            TassadarAlmCompiledExecutor::execute(&specialized_bundle, &inputs).expect("executes");
        let (specialized_outputs, specialized_halted) =
            tassadar_alm_wasm_collect(&specialized_run.step_outputs);
        assert!(specialized_halted);
        assert_eq!(specialized_outputs, expected);
    }

    #[test]
    fn compiled_and_specialized_legs_match_the_runner_on_w1_1_window() {
        let program = w1_1_window_program();
        let graph = tassadar_alm_wasm_interpreter(&program).expect("builds");
        let expected = reference_outputs(&program);
        let budget = 32;
        let inputs = vec![vec![0_i64]; budget];
        let bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        let compiled = TassadarAlmCompiledExecutor::execute(&bundle, &inputs).expect("executes");
        let (compiled_outputs, compiled_halted) = tassadar_alm_wasm_collect(&compiled.step_outputs);
        assert!(compiled_halted);
        assert_eq!(compiled_outputs, expected);

        let (specialized, report) =
            specialize_tassadar_alm_graph(&graph, TASSADAR_ALM_WASM_PROGRAM_CHANNEL)
                .expect("specializes");
        assert_eq!(report.rewritten_reads, 2);
        let specialized_bundle = compile_tassadar_alm_graph(&specialized).expect("compiles");
        let specialized_run =
            TassadarAlmCompiledExecutor::execute(&specialized_bundle, &inputs).expect("executes");
        let (specialized_outputs, specialized_halted) =
            tassadar_alm_wasm_collect(&specialized_run.step_outputs);
        assert!(specialized_halted);
        assert_eq!(specialized_outputs, expected);
    }

    #[test]
    fn converter_refuses_out_of_range_references() {
        use TassadarInstruction as I;
        let profile = TassadarWasmProfile::core_i32_v2();
        let bad_branch = TassadarProgram::new(
            "alm_wasm.bad_branch",
            &profile,
            1,
            1,
            vec![I::I32Const { value: 1 }, I::BrIf { target_pc: 9 }],
        );
        assert!(matches!(
            tassadar_alm_wasm_interpreter(&bad_branch).expect_err("refuses"),
            TassadarAlmWasmInterpreterError::BranchTargetOutOfRange { target: 9, .. }
        ));
        let bad_local = TassadarProgram::new(
            "alm_wasm.bad_local",
            &profile,
            1,
            1,
            vec![I::LocalGet { local: 3 }],
        );
        assert!(matches!(
            tassadar_alm_wasm_interpreter(&bad_local).expect_err("refuses"),
            TassadarAlmWasmInterpreterError::LocalOutOfRange { local: 3, .. }
        ));
        let bad_slot = TassadarProgram::new(
            "alm_wasm.bad_slot",
            &profile,
            1,
            1,
            vec![I::I32Load { slot: 5 }],
        );
        assert!(matches!(
            tassadar_alm_wasm_interpreter(&bad_slot).expect_err("refuses"),
            TassadarAlmWasmInterpreterError::SlotOutOfRange { slot: 5, .. }
        ));
    }
}
