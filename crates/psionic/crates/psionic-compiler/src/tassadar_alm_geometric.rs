use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::tassadar_alm_backend::{
    TassadarAlmAttentionRow, TassadarAlmCompiledBundle, TassadarAlmCompiledExecutionError,
    TassadarAlmWiringRow,
};

/// Stable executor identifier for the geometric attention leg.
pub const TASSADAR_ALM_GEOMETRIC_EXECUTOR_ID: &str = "tassadar.alm_geometric_executor.v1";
/// Claim boundary for the geometric attention execution leg.
pub const TASSADAR_ALM_GEOMETRIC_CLAIM_BOUNDARY: &str = "the geometric executor realizes keyed \
     reads as parabolic-key argmax over append-only point lists and accumulators as \
     uniform-attention sums, in exact integers with linear-scan argmax; it proves mechanism \
     parity with the evaluator and row executor only and makes no f32, softmax, hull-fast-path, \
     or served-route claim";

/// One parabolic key/value point in a keyed channel's append-only list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmGeometricPoint {
    /// First key coordinate `2k`.
    pub key_x: i64,
    /// Second key coordinate `-k^2`.
    pub key_y: i64,
    /// Original key `k` (carried for the exact-match check).
    pub key: i64,
    /// Stored value.
    pub value: i64,
    /// Monotone write order for latest-write tie-breaking.
    pub write_order: u64,
}

/// One deterministic geometric execution trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmGeometricTrace {
    /// Executor identifier.
    pub executor_id: String,
    /// Digest of the executed bundle.
    pub bundle_digest: String,
    /// Source graph digest carried by the bundle.
    pub graph_digest: String,
    /// Number of executed steps.
    pub step_count: usize,
    /// Per-step output rows.
    pub step_outputs: Vec<Vec<i64>>,
    /// Total argmax score comparisons performed across all reads.
    pub argmax_comparisons: u64,
    /// Stable digest over the output rows, comparable with the evaluator's.
    pub trace_digest: String,
}

/// Executes one compiled ALM bundle through the geometric attention
/// mechanism: keyed reads as parabolic-key argmax with latest-write
/// tie-breaking and exact-match verification, cumsums as
/// uniform-attention sums.
pub fn tassadar_alm_geometric_execute(
    bundle: &TassadarAlmCompiledBundle,
    steps: &[Vec<i64>],
) -> Result<TassadarAlmGeometricTrace, TassadarAlmCompiledExecutionError> {
    // Channels as append-only parabolic point lists.
    let mut points: BTreeMap<u16, Vec<TassadarAlmGeometricPoint>> = BTreeMap::new();
    // Accumulators as contribution lists (uniform attention reads the
    // average; multiplying by the count recovers the exact sum).
    let mut contributions: BTreeMap<u16, Vec<i64>> = BTreeMap::new();
    let mut write_order = 0_u64;
    let mut push_point = |points: &mut BTreeMap<u16, Vec<TassadarAlmGeometricPoint>>,
                          channel: u16,
                          key: i64,
                          value: i64,
                          write_order: &mut u64| {
        let entry = points.entry(channel).or_default();
        entry.push(TassadarAlmGeometricPoint {
            key_x: 2 * key,
            key_y: key.saturating_mul(key).saturating_neg(),
            key,
            value,
            write_order: *write_order,
        });
        *write_order += 1;
    };
    for (channel, key, value) in &bundle.seed_writes {
        push_point(&mut points, *channel, *key, *value, &mut write_order);
    }
    // Phase-ordered plan mirroring the row executor.
    let mut plan: Vec<(u32, PlanRow)> = Vec::new();
    for (index, row) in bundle.wiring_rows.iter().enumerate() {
        let phase = match row {
            TassadarAlmWiringRow::Input { phase, .. }
            | TassadarAlmWiringRow::Const { phase, .. }
            | TassadarAlmWiringRow::Linear { phase, .. } => *phase,
        };
        plan.push((phase, PlanRow::Wiring(index)));
    }
    for (index, row) in bundle.attention_rows.iter().enumerate() {
        let phase = match row {
            TassadarAlmAttentionRow::KeyedRead { phase, .. }
            | TassadarAlmAttentionRow::CumSum { phase, .. } => *phase,
        };
        plan.push((phase, PlanRow::Attention(index)));
    }
    for (index, row) in bundle.ffn_rows.iter().enumerate() {
        plan.push((row.phase, PlanRow::Ffn(index)));
    }
    plan.sort_by_key(|(phase, row)| (*phase, row.order_key()));
    let mut step_outputs: Vec<Vec<i64>> = Vec::with_capacity(steps.len());
    let mut argmax_comparisons = 0_u64;
    for (step_index, fields) in steps.iter().enumerate() {
        if fields.len() != bundle.input_field_count as usize {
            return Err(TassadarAlmCompiledExecutionError::InputArityMismatch {
                step: step_index,
                found: fields.len(),
                expected: bundle.input_field_count,
            });
        }
        let mut residual: Vec<i64> = vec![0; bundle.slot_count as usize];
        for (_, row) in &plan {
            match row {
                PlanRow::Wiring(index) => match &bundle.wiring_rows[*index] {
                    TassadarAlmWiringRow::Input {
                        field, out_slot, ..
                    } => {
                        residual[*out_slot as usize] = fields[*field as usize];
                    }
                    TassadarAlmWiringRow::Const {
                        value, out_slot, ..
                    } => {
                        residual[*out_slot as usize] = *value;
                    }
                    TassadarAlmWiringRow::Linear {
                        terms,
                        bias,
                        out_slot,
                        ..
                    } => {
                        let mut total = *bias;
                        for (coefficient, slot) in terms {
                            let term = coefficient.checked_mul(residual[*slot as usize]).ok_or(
                                TassadarAlmCompiledExecutionError::Overflow { step: step_index },
                            )?;
                            total = total.checked_add(term).ok_or(
                                TassadarAlmCompiledExecutionError::Overflow { step: step_index },
                            )?;
                        }
                        residual[*out_slot as usize] = total;
                    }
                },
                PlanRow::Attention(index) => match &bundle.attention_rows[*index] {
                    TassadarAlmAttentionRow::KeyedRead {
                        channel,
                        query_slot,
                        out_slot,
                        ..
                    } => {
                        let query = residual[*query_slot as usize];
                        let channel_points =
                            points.get(channel).map(Vec::as_slice).unwrap_or_default();
                        // Geometric retrieval: score every point against the
                        // direction (q, 1); the parabolic embedding makes
                        // 2qk - k^2 uniquely maximal at k = q among distinct
                        // keys; equal scores (duplicate keys) break to the
                        // latest write order.
                        let mut best: Option<&TassadarAlmGeometricPoint> = None;
                        let mut best_score = i64::MIN;
                        for point in channel_points {
                            argmax_comparisons += 1;
                            let score = point
                                .key_x
                                .checked_mul(query)
                                .and_then(|qk| qk.checked_add(point.key_y))
                                .ok_or(TassadarAlmCompiledExecutionError::Overflow {
                                    step: step_index,
                                })?;
                            let better = match best {
                                None => true,
                                Some(current) => {
                                    score > best_score
                                        || (score == best_score
                                            && point.write_order > current.write_order)
                                }
                            };
                            if better {
                                best = Some(point);
                                best_score = score;
                            }
                        }
                        // Exact-match verification: a near-miss argmax is a
                        // refusal, never an interpolation.
                        match best {
                            Some(point) if point.key == query => {
                                residual[*out_slot as usize] = point.value;
                            }
                            _ => {
                                return Err(TassadarAlmCompiledExecutionError::MissingKey {
                                    step: step_index,
                                    channel: *channel,
                                    key: query,
                                });
                            }
                        }
                    }
                    TassadarAlmAttentionRow::CumSum {
                        channel,
                        value_slot,
                        out_slot,
                        ..
                    } => {
                        let contribution = residual[*value_slot as usize];
                        let entries = contributions.entry(*channel).or_default();
                        entries.push(contribution);
                        // Uniform attention returns the average over all
                        // contributions; multiplying by the count recovers
                        // the exact running sum. In exact integers the two
                        // compose to a checked summation.
                        let mut total = 0_i64;
                        for entry in entries.iter() {
                            total = total.checked_add(*entry).ok_or(
                                TassadarAlmCompiledExecutionError::Overflow { step: step_index },
                            )?;
                        }
                        residual[*out_slot as usize] = total;
                    }
                },
                PlanRow::Ffn(index) => {
                    let row = &bundle.ffn_rows[*index];
                    let gated = residual[row.gate_slot as usize].max(0);
                    let product = residual[row.value_slot as usize]
                        .checked_mul(gated)
                        .ok_or(TassadarAlmCompiledExecutionError::Overflow { step: step_index })?;
                    residual[row.out_slot as usize] = product;
                }
            }
        }
        for write in &bundle.write_rows {
            let key = residual[write.key_slot as usize];
            let value = residual[write.value_slot as usize];
            push_point(&mut points, write.channel, key, value, &mut write_order);
        }
        step_outputs.push(
            bundle
                .output_slots
                .iter()
                .map(|slot| residual[*slot as usize])
                .collect(),
        );
    }
    let mut hasher = Sha256::new();
    hasher.update(b"tassadar_alm_trace|");
    hasher.update(bundle.graph_digest.as_bytes());
    for row in &step_outputs {
        hasher.update(b"|row|");
        for value in row {
            hasher.update(value.to_le_bytes());
        }
    }
    let trace_digest = hex::encode(hasher.finalize());
    Ok(TassadarAlmGeometricTrace {
        executor_id: TASSADAR_ALM_GEOMETRIC_EXECUTOR_ID.to_string(),
        bundle_digest: bundle.stable_digest(),
        graph_digest: bundle.graph_digest.clone(),
        step_count: steps.len(),
        step_outputs,
        argmax_comparisons,
        trace_digest,
    })
}

#[derive(Clone, Copy, Debug)]
enum PlanRow {
    Wiring(usize),
    Attention(usize),
    Ffn(usize),
}

impl PlanRow {
    fn order_key(self) -> (u8, usize) {
        match self {
            PlanRow::Attention(index) => (0, index),
            PlanRow::Wiring(index) => (1, index),
            PlanRow::Ffn(index) => (2, index),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use psionic_ir::{
        tassadar_alm_running_sum_workload, tassadar_alm_stack_micro_workload,
        tassadar_alm_verb_parity_workload, tassadar_symbolic_program_examples,
        TassadarAlmEvaluator,
    };

    use super::*;
    use crate::tassadar_alm_backend::{compile_tassadar_alm_graph, TassadarAlmCompiledExecutor};
    use crate::tassadar_alm_specializer::specialize_tassadar_alm_graph;
    use crate::tassadar_alm_stack_isa::{
        tassadar_alm_stack_isa_interpreter, TassadarStackIsaInstruction,
        TASSADAR_ALM_STACK_ISA_PROGRAM_CHANNEL,
    };
    use crate::tassadar_symbolic_alm_bridge::compile_tassadar_symbolic_to_alm;

    fn assert_three_leg_parity(graph: &psionic_ir::TassadarAlmGraph, steps: &[Vec<i64>]) {
        let reference = TassadarAlmEvaluator::evaluate(graph, steps).expect("evaluates");
        let bundle = compile_tassadar_alm_graph(graph).expect("compiles");
        let row = TassadarAlmCompiledExecutor::execute(&bundle, steps).expect("row executes");
        let geometric = tassadar_alm_geometric_execute(&bundle, steps).expect("geo executes");
        assert_eq!(geometric.step_outputs, reference.step_outputs);
        assert_eq!(geometric.trace_digest, reference.trace_digest);
        assert_eq!(geometric.trace_digest, row.trace_digest);
    }

    #[test]
    fn committed_workloads_agree_across_all_three_legs() {
        assert_three_leg_parity(
            &tassadar_alm_running_sum_workload(),
            &[vec![3], vec![5], vec![-2], vec![10]],
        );
        let parity_steps: Vec<Vec<i64>> = [0_i64, 0, 1, 0, 0, 0, 1, 0, 0, 0]
            .iter()
            .map(|bit| vec![*bit])
            .collect();
        assert_three_leg_parity(&tassadar_alm_verb_parity_workload(), &parity_steps);
        assert_three_leg_parity(
            &tassadar_alm_stack_micro_workload(),
            &[vec![0, 3], vec![0, 5], vec![1, 0], vec![2, 0]],
        );
    }

    #[test]
    fn stack_isa_universal_and_specialized_agree_geometrically() {
        use TassadarStackIsaInstruction as I;
        let program = vec![I::Push(3), I::Push(5), I::Add, I::Push(2), I::Mul, I::Out];
        let (universal, step_count) =
            tassadar_alm_stack_isa_interpreter(&program, 4).expect("builds");
        let steps = vec![vec![0_i64]; step_count];
        assert_three_leg_parity(&universal, &steps);
        let (specialized, _) =
            specialize_tassadar_alm_graph(&universal, TASSADAR_ALM_STACK_ISA_PROGRAM_CHANNEL)
                .expect("specializes");
        assert_three_leg_parity(&specialized, &steps);
    }

    #[test]
    fn bridged_symbolic_examples_agree_geometrically() {
        for example in tassadar_symbolic_program_examples() {
            let (graph, _) = compile_tassadar_symbolic_to_alm(&example.program).expect("bridges");
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
                            .expect("assigned"),
                    )
                })
                .collect();
            assert_three_leg_parity(&graph, &[row]);
        }
    }

    #[test]
    fn near_miss_argmax_refuses_instead_of_interpolating() {
        // Read key 5 from a channel seeded only with keys 0 and 9: the
        // geometric argmax lands on a present key, but it is not the query,
        // so the read must refuse exactly like the evaluator.
        use psionic_ir::{
            TassadarAlmChannelDecl, TassadarAlmChannelId, TassadarAlmChannelKind, TassadarAlmGate,
            TassadarAlmGraph, TassadarAlmSeedWrite, TassadarAlmValueId,
            TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
        };
        let graph = TassadarAlmGraph {
            schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
            graph_id: "alm.geometric.near_miss".to_string(),
            input_field_count: 1,
            channels: vec![TassadarAlmChannelDecl {
                channel_id: TassadarAlmChannelId(0),
                name: "memory".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            }],
            seed_writes: vec![
                TassadarAlmSeedWrite {
                    channel_id: TassadarAlmChannelId(0),
                    key: 0,
                    value: 11,
                },
                TassadarAlmSeedWrite {
                    channel_id: TassadarAlmChannelId(0),
                    key: 9,
                    value: 22,
                },
            ],
            gates: vec![
                TassadarAlmGate::Input { field: 0 },
                TassadarAlmGate::ChannelRead {
                    channel_id: TassadarAlmChannelId(0),
                    query: TassadarAlmValueId(0),
                },
            ],
            outputs: vec![TassadarAlmValueId(1)],
        };
        let bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        let error = tassadar_alm_geometric_execute(&bundle, &[vec![5]]).expect_err("refuses");
        assert!(matches!(
            error,
            TassadarAlmCompiledExecutionError::MissingKey {
                step: 0,
                channel: 0,
                key: 5
            }
        ));
        // And an exact hit still works.
        let trace = tassadar_alm_geometric_execute(&bundle, &[vec![9]]).expect("executes");
        assert_eq!(trace.step_outputs, vec![vec![22]]);
        assert!(trace.argmax_comparisons > 0);
    }

    #[test]
    fn duplicate_keys_break_ties_to_the_latest_write() {
        // Two writes to key 1 across steps: the geometric scores tie
        // exactly, so write order must decide, matching the evaluator's
        // latest-write-wins semantics.
        use psionic_ir::{
            TassadarAlmChannelDecl, TassadarAlmChannelId, TassadarAlmChannelKind, TassadarAlmGate,
            TassadarAlmGraph, TassadarAlmSeedWrite, TassadarAlmValueId,
            TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
        };
        let graph = TassadarAlmGraph {
            schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
            graph_id: "alm.geometric.latest_write".to_string(),
            input_field_count: 1,
            channels: vec![TassadarAlmChannelDecl {
                channel_id: TassadarAlmChannelId(0),
                name: "cell".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            }],
            seed_writes: vec![TassadarAlmSeedWrite {
                channel_id: TassadarAlmChannelId(0),
                key: 1,
                value: 0,
            }],
            gates: vec![
                TassadarAlmGate::Input { field: 0 },
                TassadarAlmGate::Const { value: 1 },
                TassadarAlmGate::ChannelRead {
                    channel_id: TassadarAlmChannelId(0),
                    query: TassadarAlmValueId(1),
                },
                TassadarAlmGate::ChannelWrite {
                    channel_id: TassadarAlmChannelId(0),
                    key: TassadarAlmValueId(1),
                    value: TassadarAlmValueId(0),
                },
            ],
            outputs: vec![TassadarAlmValueId(2)],
        };
        let bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        let trace = tassadar_alm_geometric_execute(&bundle, &[vec![10], vec![20], vec![30]])
            .expect("executes");
        let reads: Vec<i64> = trace.step_outputs.iter().map(|row| row[0]).collect();
        assert_eq!(reads, vec![0, 10, 20]);
    }
}
