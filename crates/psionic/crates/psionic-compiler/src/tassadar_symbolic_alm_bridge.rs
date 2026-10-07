use std::collections::BTreeMap;

use psionic_ir::{
    TassadarAlmGate, TassadarAlmGraph, TassadarAlmValueId, TassadarSymbolicBinaryOp,
    TassadarSymbolicExpr, TassadarSymbolicOperand, TassadarSymbolicProgram,
    TassadarSymbolicStatement, TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable family identifier for the symbolic-to-ALM bridge.
pub const TASSADAR_SYMBOLIC_ALM_BRIDGE_FAMILY: &str = "tassadar_symbolic_alm_bridge";
/// Stable version identifier for the symbolic-to-ALM bridge.
pub const TASSADAR_SYMBOLIC_ALM_BRIDGE_VERSION: &str = "v1";
/// Claim boundary for the symbolic-to-ALM bridge lane.
pub const TASSADAR_SYMBOLIC_ALM_BRIDGE_CLAIM_BOUNDARY: &str = "the symbolic-to-ALM bridge \
     lowers the bounded straight-line symbolic IR into channel-free single-step ALM graphs; the \
     symbolic evaluator saturates in i32 while the ALM is checked-exact i64, so parity is \
     claimed only for executions that neither saturate in i32 nor overflow i64; no control \
     flow, Wasm, or served-route claim is created";

/// One digest-pinned bridge report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSymbolicAlmBridgeReport {
    /// Bridge family identifier.
    pub bridge_family: String,
    /// Bridge version identifier.
    pub bridge_version: String,
    /// Source symbolic program identifier.
    pub program_id: String,
    /// Source symbolic program digest.
    pub program_digest: String,
    /// Bridged ALM graph digest.
    pub graph_digest: String,
    /// Gates in the bridged graph.
    pub gate_count: usize,
    /// Outputs in the bridged graph.
    pub output_count: usize,
}

impl TassadarSymbolicAlmBridgeReport {
    /// Returns a stable digest over the report encoding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_symbolic_alm_bridge_report|");
        hasher.update(serde_json::to_vec(self).unwrap_or_default());
        hex::encode(hasher.finalize())
    }
}

/// Bridge failure for one symbolic program.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarSymbolicAlmBridgeError {
    /// An operand names a binding or input that does not exist yet.
    #[error("operand references unknown name `{name}`")]
    UnknownName {
        /// Unresolved symbolic name.
        name: String,
    },
    /// An operand or store references a slot outside the declared memory.
    #[error("memory slot {slot} is outside the declared {declared} slots")]
    SlotOutOfRange {
        /// Referenced slot.
        slot: u8,
        /// Declared slot count.
        declared: usize,
    },
    /// The program emits no outputs, so the graph would be invalid.
    #[error("symbolic program emits no outputs")]
    NoOutputs,
}

struct BridgeBuilder {
    gates: Vec<TassadarAlmGate>,
    one: Option<TassadarAlmValueId>,
}

impl BridgeBuilder {
    fn push(&mut self, gate: TassadarAlmGate) -> TassadarAlmValueId {
        self.gates.push(gate);
        TassadarAlmValueId((self.gates.len() - 1) as u32)
    }

    fn one(&mut self) -> TassadarAlmValueId {
        match self.one {
            Some(id) => id,
            None => {
                let id = self.push(TassadarAlmGate::Const { value: 1 });
                self.one = Some(id);
                id
            }
        }
    }

    /// `1[input >= threshold]` from the two-ReLU step identity.
    fn ge_indicator(&mut self, input: TassadarAlmValueId, threshold: i64) -> TassadarAlmValueId {
        let one = self.one();
        let shifted_plus = self.push(TassadarAlmGate::Linear {
            terms: vec![(1, input)],
            bias: 1 - threshold,
        });
        let relu_plus = self.push(TassadarAlmGate::ReGlu {
            value: one,
            gate: shifted_plus,
        });
        let shifted = self.push(TassadarAlmGate::Linear {
            terms: vec![(1, input)],
            bias: -threshold,
        });
        let relu = self.push(TassadarAlmGate::ReGlu {
            value: one,
            gate: shifted,
        });
        self.push(TassadarAlmGate::Linear {
            terms: vec![(1, relu_plus), (-1, relu)],
            bias: 0,
        })
    }

    /// Exact dynamic product `left * right` via the two-ReGLU identity.
    fn product(
        &mut self,
        left: TassadarAlmValueId,
        right: TassadarAlmValueId,
    ) -> TassadarAlmValueId {
        let positive = self.push(TassadarAlmGate::ReGlu {
            value: left,
            gate: right,
        });
        let negated = self.push(TassadarAlmGate::Linear {
            terms: vec![(-1, right)],
            bias: 0,
        });
        let negative = self.push(TassadarAlmGate::ReGlu {
            value: left,
            gate: negated,
        });
        self.push(TassadarAlmGate::Linear {
            terms: vec![(1, positive), (-1, negative)],
            bias: 0,
        })
    }
}

/// Compiles one bounded symbolic program into a channel-free single-step
/// ALM graph plus its bridge report.
pub fn compile_tassadar_symbolic_to_alm(
    program: &TassadarSymbolicProgram,
) -> Result<(TassadarAlmGraph, TassadarSymbolicAlmBridgeReport), TassadarSymbolicAlmBridgeError> {
    let mut builder = BridgeBuilder {
        gates: Vec::new(),
        one: None,
    };
    // Input fields in declaration order; memory slots resolve to the bound
    // input, the initial-memory constant, or zero.
    let mut name_values: BTreeMap<String, TassadarAlmValueId> = BTreeMap::new();
    let mut slot_values: BTreeMap<u8, TassadarAlmValueId> = BTreeMap::new();
    for (field, input) in program.inputs.iter().enumerate() {
        let value = builder.push(TassadarAlmGate::Input {
            field: field as u16,
        });
        name_values.insert(input.name.clone(), value);
        slot_values.insert(input.memory_slot, value);
    }
    for cell in &program.initial_memory {
        let value = builder.push(TassadarAlmGate::Const {
            value: i64::from(cell.value),
        });
        slot_values.entry(cell.slot).or_insert(value);
    }
    let declared = program.memory_slots;
    let mut resolve = |operand: &TassadarSymbolicOperand,
                       builder: &mut BridgeBuilder,
                       name_values: &BTreeMap<String, TassadarAlmValueId>,
                       slot_values: &mut BTreeMap<u8, TassadarAlmValueId>|
     -> Result<TassadarAlmValueId, TassadarSymbolicAlmBridgeError> {
        match operand {
            TassadarSymbolicOperand::Name { name } => name_values
                .get(name)
                .copied()
                .ok_or_else(|| TassadarSymbolicAlmBridgeError::UnknownName { name: name.clone() }),
            TassadarSymbolicOperand::Const { value } => Ok(builder.push(TassadarAlmGate::Const {
                value: i64::from(*value),
            })),
            TassadarSymbolicOperand::MemorySlot { slot } => {
                if usize::from(*slot) >= declared {
                    return Err(TassadarSymbolicAlmBridgeError::SlotOutOfRange {
                        slot: *slot,
                        declared,
                    });
                }
                Ok(*slot_values
                    .entry(*slot)
                    .or_insert_with(|| builder.push(TassadarAlmGate::Const { value: 0 })))
            }
        }
    };
    let mut outputs: Vec<TassadarAlmValueId> = Vec::new();
    for statement in &program.statements {
        match statement {
            TassadarSymbolicStatement::Let { name, expr } => {
                let value = match expr {
                    TassadarSymbolicExpr::Operand { operand } => {
                        resolve(operand, &mut builder, &name_values, &mut slot_values)?
                    }
                    TassadarSymbolicExpr::Binary { op, left, right } => {
                        let left_value =
                            resolve(left, &mut builder, &name_values, &mut slot_values)?;
                        let right_value =
                            resolve(right, &mut builder, &name_values, &mut slot_values)?;
                        match op {
                            TassadarSymbolicBinaryOp::Add => {
                                builder.push(TassadarAlmGate::Linear {
                                    terms: vec![(1, left_value), (1, right_value)],
                                    bias: 0,
                                })
                            }
                            TassadarSymbolicBinaryOp::Sub => {
                                builder.push(TassadarAlmGate::Linear {
                                    terms: vec![(1, left_value), (-1, right_value)],
                                    bias: 0,
                                })
                            }
                            TassadarSymbolicBinaryOp::Mul => {
                                builder.product(left_value, right_value)
                            }
                            TassadarSymbolicBinaryOp::Lt => {
                                // i32(left < right) = 1[right - left >= 1].
                                let difference = builder.push(TassadarAlmGate::Linear {
                                    terms: vec![(1, right_value), (-1, left_value)],
                                    bias: 0,
                                });
                                builder.ge_indicator(difference, 1)
                            }
                        }
                    }
                };
                name_values.insert(name.clone(), value);
            }
            TassadarSymbolicStatement::Store { slot, value } => {
                if usize::from(*slot) >= declared {
                    return Err(TassadarSymbolicAlmBridgeError::SlotOutOfRange {
                        slot: *slot,
                        declared,
                    });
                }
                let resolved = resolve(value, &mut builder, &name_values, &mut slot_values)?;
                slot_values.insert(*slot, resolved);
            }
            TassadarSymbolicStatement::Output { value } => {
                let resolved = resolve(value, &mut builder, &name_values, &mut slot_values)?;
                outputs.push(resolved);
            }
        }
    }
    if outputs.is_empty() {
        return Err(TassadarSymbolicAlmBridgeError::NoOutputs);
    }
    let graph = TassadarAlmGraph {
        schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
        graph_id: format!("alm.symbolic_bridge.{}", program.program_id),
        input_field_count: program.inputs.len() as u16,
        channels: Vec::new(),
        seed_writes: Vec::new(),
        gates: builder.gates,
        outputs,
    };
    let report = TassadarSymbolicAlmBridgeReport {
        bridge_family: TASSADAR_SYMBOLIC_ALM_BRIDGE_FAMILY.to_string(),
        bridge_version: TASSADAR_SYMBOLIC_ALM_BRIDGE_VERSION.to_string(),
        program_id: program.program_id.clone(),
        program_digest: program.stable_digest(),
        graph_digest: graph.stable_digest(),
        gate_count: graph.gates.len(),
        output_count: graph.outputs.len(),
    };
    Ok((graph, report))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use psionic_ir::{tassadar_symbolic_program_examples, TassadarAlmEvaluator};

    use super::*;
    use crate::tassadar_alm_backend::{compile_tassadar_alm_graph, TassadarAlmCompiledExecutor};

    #[test]
    fn every_committed_symbolic_example_agrees_across_three_legs() {
        let examples = tassadar_symbolic_program_examples();
        assert!(!examples.is_empty());
        for example in examples {
            let symbolic = example
                .program
                .evaluate(&example.input_assignments)
                .expect("symbolic evaluates");
            assert_eq!(
                symbolic.outputs, example.expected_outputs,
                "case {}",
                example.case_id
            );
            let (graph, report) =
                compile_tassadar_symbolic_to_alm(&example.program).expect("bridges");
            assert_eq!(report.output_count, example.expected_outputs.len());
            // One step; input fields follow the program's input declaration
            // order.
            let row: Vec<i64> = example
                .program
                .inputs
                .iter()
                .map(|input| {
                    i64::from(
                        example
                            .input_assignments
                            .get(&input.name)
                            .copied()
                            .expect("example assigns every input"),
                    )
                })
                .collect();
            let evaluated =
                TassadarAlmEvaluator::evaluate(&graph, &[row.clone()]).expect("evaluates");
            let expected: Vec<i64> = example
                .expected_outputs
                .iter()
                .map(|value| i64::from(*value))
                .collect();
            assert_eq!(
                evaluated.step_outputs[0], expected,
                "case {}",
                example.case_id
            );
            let bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
            let compiled = TassadarAlmCompiledExecutor::execute(&bundle, &[row]).expect("executes");
            assert_eq!(
                compiled.step_outputs[0], expected,
                "case {}",
                example.case_id
            );
        }
    }

    #[test]
    fn bridge_reports_are_digest_stable() {
        let example = tassadar_symbolic_program_examples()
            .into_iter()
            .next()
            .expect("at least one example");
        let (_, a) = compile_tassadar_symbolic_to_alm(&example.program).expect("bridges");
        let (_, b) = compile_tassadar_symbolic_to_alm(&example.program).expect("bridges");
        assert_eq!(a.stable_digest(), b.stable_digest());
    }

    #[test]
    fn unknown_names_refuse() {
        let mut example = tassadar_symbolic_program_examples()
            .into_iter()
            .next()
            .expect("at least one example");
        example
            .program
            .statements
            .push(TassadarSymbolicStatement::Output {
                value: TassadarSymbolicOperand::Name {
                    name: "missing_binding".to_string(),
                },
            });
        assert_eq!(
            compile_tassadar_symbolic_to_alm(&example.program).expect_err("refuses"),
            TassadarSymbolicAlmBridgeError::UnknownName {
                name: "missing_binding".to_string()
            }
        );
    }

    #[test]
    fn out_of_range_slots_refuse() {
        let mut example = tassadar_symbolic_program_examples()
            .into_iter()
            .next()
            .expect("at least one example");
        example
            .program
            .statements
            .push(TassadarSymbolicStatement::Store {
                slot: 250,
                value: TassadarSymbolicOperand::Const { value: 1 },
            });
        let error = compile_tassadar_symbolic_to_alm(&example.program).expect_err("refuses");
        assert!(matches!(
            error,
            TassadarSymbolicAlmBridgeError::SlotOutOfRange { slot: 250, .. }
        ));
    }
}
