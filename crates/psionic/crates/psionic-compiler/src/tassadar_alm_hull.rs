use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::tassadar_alm_backend::{
    TassadarAlmAttentionRow, TassadarAlmCompiledBundle, TassadarAlmCompiledExecutionError,
    TassadarAlmWiringRow,
};

/// Stable executor identifier for the hull fast-path leg.
pub const TASSADAR_ALM_HULL_EXECUTOR_ID: &str = "tassadar.alm_hull_executor.v1";
/// Claim boundary for the hull fast-path execution leg.
pub const TASSADAR_ALM_HULL_CLAIM_BOUNDARY: &str = "the hull executor accelerates exact \
     integer parabolic-key argmax with a Li Chao tree over a declared query window, demoting \
     out-of-window reads to the linear-scan fallback with counts reported; it changes retrieval \
     cost only, proves digest parity with the evaluator, row, and geometric legs, and makes no \
     f32, softmax, or served-route claim";

/// Inclusive magnitude bound for hull-direct keys and queries.
pub const TASSADAR_ALM_HULL_WINDOW: i64 = 1 << 31;

#[derive(Clone, Copy, Debug)]
struct HullLine {
    /// Slope `2k`.
    slope: i64,
    /// Intercept `-k^2`.
    intercept: i64,
    /// Original key `k`.
    key: i64,
}

impl HullLine {
    fn evaluate(&self, x: i64) -> i128 {
        i128::from(self.slope) * i128::from(x) + i128::from(self.intercept)
    }
}

#[derive(Debug, Default)]
struct LiChaoNode {
    line: Option<HullLine>,
    left: Option<Box<LiChaoNode>>,
    right: Option<Box<LiChaoNode>>,
}

/// One per-channel hull cache: a Li Chao tree for argmax over keys plus a
/// latest-write-wins value map for retrieval and the exact-match check.
#[derive(Debug, Default)]
pub struct TassadarAlmHullCache {
    root: LiChaoNode,
    values: BTreeMap<i64, i64>,
    /// Keys outside the window force fallback for the whole channel.
    fallback_only: bool,
    /// Linear-scan mirror used for fallback reads.
    insertion_order: Vec<(i64, i64)>,
}

impl TassadarAlmHullCache {
    /// Inserts one write (latest write wins per key).
    pub fn insert(&mut self, key: i64, value: i64) {
        self.insertion_order.push((key, value));
        let fresh = self.values.insert(key, value).is_none();
        if key.abs() > TASSADAR_ALM_HULL_WINDOW {
            self.fallback_only = true;
            return;
        }
        if fresh && !self.fallback_only {
            let line = HullLine {
                slope: 2 * key,
                intercept: i128_to_saturated(-(i128::from(key) * i128::from(key))),
                key,
            };
            insert_line(
                &mut self.root,
                -TASSADAR_ALM_HULL_WINDOW,
                TASSADAR_ALM_HULL_WINDOW,
                line,
                &mut 0,
            );
        }
    }

    /// Reads `query` exactly, counting hull node visits or fallback
    /// comparisons. Returns `(value, direct)` or `None` for a missing key.
    pub fn read(
        &self,
        query: i64,
        node_visits: &mut u64,
        fallback_comparisons: &mut u64,
    ) -> Option<(i64, bool)> {
        if self.fallback_only || query.abs() > TASSADAR_ALM_HULL_WINDOW {
            // Linear-scan fallback, latest write wins.
            let mut found: Option<i64> = None;
            for (key, value) in &self.insertion_order {
                *fallback_comparisons += 1;
                if *key == query {
                    found = Some(*value);
                }
            }
            return found.map(|value| (value, false));
        }
        let best = query_line(
            &self.root,
            -TASSADAR_ALM_HULL_WINDOW,
            TASSADAR_ALM_HULL_WINDOW,
            query,
            node_visits,
        )?;
        // Exact-match verification: the hull names the maximizing key; only
        // an exact hit retrieves, mirroring the partial-read semantics.
        if best.key == query {
            self.values.get(&query).map(|value| (*value, true))
        } else {
            None
        }
    }
}

fn i128_to_saturated(value: i128) -> i64 {
    value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn insert_line(node: &mut LiChaoNode, lo: i64, hi: i64, mut line: HullLine, depth: &mut u32) {
    *depth += 1;
    if *depth > 96 {
        return;
    }
    let Some(mut current) = node.line else {
        node.line = Some(line);
        return;
    };
    let mid = lo + (hi - lo) / 2;
    let line_better_mid = line.evaluate(mid) > current.evaluate(mid);
    if line_better_mid {
        std::mem::swap(&mut line, &mut current);
        node.line = Some(current);
    }
    let current = node.line.unwrap_or(line);
    if lo == hi {
        return;
    }
    let line_better_lo = line.evaluate(lo) > current.evaluate(lo);
    let line_better_hi = line.evaluate(hi) > current.evaluate(hi);
    if line_better_lo {
        let left = node.left.get_or_insert_with(Box::default);
        insert_line(left, lo, mid, line, depth);
    } else if line_better_hi {
        let right = node.right.get_or_insert_with(Box::default);
        insert_line(right, mid + 1, hi, line, depth);
    }
}

fn query_line(
    node: &LiChaoNode,
    lo: i64,
    hi: i64,
    x: i64,
    node_visits: &mut u64,
) -> Option<HullLine> {
    *node_visits += 1;
    let mut best = node.line;
    if lo == hi {
        return best;
    }
    let mid = lo + (hi - lo) / 2;
    let child = if x <= mid {
        node.left.as_deref().map(|left| (left, lo, mid))
    } else {
        node.right.as_deref().map(|right| (right, mid + 1, hi))
    };
    if let Some((child, child_lo, child_hi)) = child {
        if let Some(candidate) = query_line(child, child_lo, child_hi, x, node_visits) {
            best = match best {
                None => Some(candidate),
                Some(current) if candidate.evaluate(x) > current.evaluate(x) => Some(candidate),
                Some(current) => Some(current),
            };
        }
    }
    best
}

/// One deterministic hull execution trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmHullTrace {
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
    /// Hull-direct reads.
    pub direct_reads: u64,
    /// Fallback (linear-scan) reads.
    pub fallback_reads: u64,
    /// Li Chao node visits across all direct reads.
    pub hull_node_visits: u64,
    /// Comparisons performed on the fallback path.
    pub fallback_comparisons: u64,
    /// Comparisons the linear-scan leg would have performed for the same
    /// reads (the avoided baseline).
    pub linear_scan_baseline: u64,
    /// Stable digest over the output rows, comparable with the evaluator's.
    pub trace_digest: String,
}

/// Executes one compiled ALM bundle with hull-accelerated keyed reads.
pub fn tassadar_alm_hull_execute(
    bundle: &TassadarAlmCompiledBundle,
    steps: &[Vec<i64>],
) -> Result<TassadarAlmHullTrace, TassadarAlmCompiledExecutionError> {
    let mut caches: BTreeMap<u16, TassadarAlmHullCache> = BTreeMap::new();
    let mut accumulators: BTreeMap<u16, i64> = BTreeMap::new();
    let mut channel_sizes: BTreeMap<u16, u64> = BTreeMap::new();
    for (channel, key, value) in &bundle.seed_writes {
        caches.entry(*channel).or_default().insert(*key, *value);
        *channel_sizes.entry(*channel).or_insert(0) += 1;
    }
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
    let mut direct_reads = 0_u64;
    let mut fallback_reads = 0_u64;
    let mut hull_node_visits = 0_u64;
    let mut fallback_comparisons = 0_u64;
    let mut linear_scan_baseline = 0_u64;
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
                        linear_scan_baseline += channel_sizes.get(channel).copied().unwrap_or(0);
                        let cache = caches.entry(*channel).or_default();
                        let outcome =
                            cache.read(query, &mut hull_node_visits, &mut fallback_comparisons);
                        match outcome {
                            Some((value, direct)) => {
                                if direct {
                                    direct_reads += 1;
                                } else {
                                    fallback_reads += 1;
                                }
                                residual[*out_slot as usize] = value;
                            }
                            None => {
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
                        let current = accumulators.get(channel).copied().unwrap_or(0);
                        let total = current.checked_add(contribution).ok_or(
                            TassadarAlmCompiledExecutionError::Overflow { step: step_index },
                        )?;
                        accumulators.insert(*channel, total);
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
            caches.entry(write.channel).or_default().insert(key, value);
            *channel_sizes.entry(write.channel).or_insert(0) += 1;
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
    Ok(TassadarAlmHullTrace {
        executor_id: TASSADAR_ALM_HULL_EXECUTOR_ID.to_string(),
        bundle_digest: bundle.stable_digest(),
        graph_digest: bundle.graph_digest.clone(),
        step_count: steps.len(),
        step_outputs,
        direct_reads,
        fallback_reads,
        hull_node_visits,
        fallback_comparisons,
        linear_scan_baseline,
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
        tassadar_alm_stack_micro_workload, tassadar_alm_verb_parity_workload,
        TassadarAlmChannelDecl, TassadarAlmChannelId, TassadarAlmChannelKind, TassadarAlmEvaluator,
        TassadarAlmGate, TassadarAlmGraph, TassadarAlmSeedWrite, TassadarAlmValueId,
        TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
    };

    use super::*;
    use crate::tassadar_alm_backend::compile_tassadar_alm_graph;
    use crate::tassadar_alm_geometric::tassadar_alm_geometric_execute;

    fn assert_hull_parity(graph: &TassadarAlmGraph, steps: &[Vec<i64>]) -> TassadarAlmHullTrace {
        let reference = TassadarAlmEvaluator::evaluate(graph, steps).expect("evaluates");
        let bundle = compile_tassadar_alm_graph(graph).expect("compiles");
        let geometric = tassadar_alm_geometric_execute(&bundle, steps).expect("geo executes");
        let hull = tassadar_alm_hull_execute(&bundle, steps).expect("hull executes");
        assert_eq!(hull.step_outputs, reference.step_outputs);
        assert_eq!(hull.trace_digest, reference.trace_digest);
        assert_eq!(hull.trace_digest, geometric.trace_digest);
        hull
    }

    #[test]
    fn committed_workloads_agree_through_the_hull() {
        let parity_steps: Vec<Vec<i64>> = [0_i64, 0, 1, 0, 0, 0, 1, 0, 0, 0]
            .iter()
            .map(|bit| vec![*bit])
            .collect();
        let trace = assert_hull_parity(&tassadar_alm_verb_parity_workload(), &parity_steps);
        assert!(trace.direct_reads > 0);
        assert_eq!(trace.fallback_reads, 0);
        assert_hull_parity(
            &tassadar_alm_stack_micro_workload(),
            &[vec![0, 3], vec![0, 5], vec![1, 0], vec![2, 0]],
        );
    }

    /// Long-horizon chain: each step writes under the position key and
    /// reads the previous position, so the channel grows linearly and the
    /// linear-scan baseline grows quadratically while hull visits stay
    /// logarithmic per read.
    fn chain_graph() -> TassadarAlmGraph {
        TassadarAlmGraph {
            schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
            graph_id: "alm.hull.chain".to_string(),
            input_field_count: 1,
            channels: vec![
                TassadarAlmChannelDecl {
                    channel_id: TassadarAlmChannelId(0),
                    name: "chain".to_string(),
                    kind: TassadarAlmChannelKind::Keyed,
                },
                TassadarAlmChannelDecl {
                    channel_id: TassadarAlmChannelId(1),
                    name: "position".to_string(),
                    kind: TassadarAlmChannelKind::Accumulator,
                },
            ],
            seed_writes: vec![TassadarAlmSeedWrite {
                channel_id: TassadarAlmChannelId(0),
                key: 0,
                value: 0,
            }],
            gates: vec![
                TassadarAlmGate::Input { field: 0 },
                TassadarAlmGate::Const { value: 1 },
                TassadarAlmGate::CumSum {
                    channel_id: TassadarAlmChannelId(1),
                    value: TassadarAlmValueId(1),
                },
                TassadarAlmGate::Linear {
                    terms: vec![(1, TassadarAlmValueId(2))],
                    bias: -1,
                },
                TassadarAlmGate::ChannelRead {
                    channel_id: TassadarAlmChannelId(0),
                    query: TassadarAlmValueId(3),
                },
                TassadarAlmGate::Linear {
                    terms: vec![(1, TassadarAlmValueId(4)), (1, TassadarAlmValueId(0))],
                    bias: 0,
                },
                TassadarAlmGate::ChannelWrite {
                    channel_id: TassadarAlmChannelId(0),
                    key: TassadarAlmValueId(2),
                    value: TassadarAlmValueId(5),
                },
            ],
            outputs: vec![TassadarAlmValueId(5)],
        }
    }

    #[test]
    fn hull_visits_beat_the_linear_baseline_by_an_order_of_magnitude() {
        let graph = chain_graph();
        let steps: Vec<Vec<i64>> = (0..2000).map(|_| vec![1_i64]).collect();
        let trace = assert_hull_parity(&graph, &steps);
        // Running total of 2000 ones.
        assert_eq!(trace.step_outputs[1999], vec![2000]);
        assert_eq!(trace.fallback_reads, 0);
        assert!(trace.linear_scan_baseline > 1_000_000, "baseline {trace:?}");
        assert!(
            trace.hull_node_visits * 10 < trace.linear_scan_baseline,
            "hull visits {} vs baseline {}",
            trace.hull_node_visits,
            trace.linear_scan_baseline
        );
    }

    #[test]
    fn out_of_window_keys_demote_to_fallback_with_correct_results() {
        let graph = TassadarAlmGraph {
            schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
            graph_id: "alm.hull.fallback".to_string(),
            input_field_count: 1,
            channels: vec![TassadarAlmChannelDecl {
                channel_id: TassadarAlmChannelId(0),
                name: "memory".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            }],
            seed_writes: vec![
                TassadarAlmSeedWrite {
                    channel_id: TassadarAlmChannelId(0),
                    key: (1_i64 << 31) + 7,
                    value: 99,
                },
                TassadarAlmSeedWrite {
                    channel_id: TassadarAlmChannelId(0),
                    key: 3,
                    value: 42,
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
        let trace =
            tassadar_alm_hull_execute(&bundle, &[vec![3], vec![(1 << 31) + 7]]).expect("executes");
        assert_eq!(trace.step_outputs, vec![vec![42], vec![99]]);
        assert_eq!(trace.direct_reads, 0);
        assert_eq!(trace.fallback_reads, 2);
        assert!(trace.fallback_comparisons > 0);
    }

    #[test]
    fn missing_keys_refuse_through_the_hull_path() {
        let graph = chain_graph();
        let bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        // Step count is fine, but querying beyond seeded/written keys
        // happens if we tamper the seed away.
        let mut unseeded = graph.clone();
        unseeded.seed_writes.clear();
        let bundle_unseeded = compile_tassadar_alm_graph(&unseeded).expect("compiles");
        let error = tassadar_alm_hull_execute(&bundle_unseeded, &[vec![1]]).expect_err("refuses");
        assert!(matches!(
            error,
            TassadarAlmCompiledExecutionError::MissingKey { step: 0, .. }
        ));
        // The seeded bundle still works.
        let trace = tassadar_alm_hull_execute(&bundle, &[vec![1]]).expect("executes");
        assert_eq!(trace.step_outputs, vec![vec![1]]);
    }
}
