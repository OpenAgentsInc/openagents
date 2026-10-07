use psionic_ir::{TassadarAlmChannelId, TassadarAlmGate, TassadarAlmGraph, TassadarAlmValueId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable family identifier for the ALM Futamura specializer.
pub const TASSADAR_ALM_SPECIALIZER_FAMILY: &str = "tassadar_alm_first_futamura";
/// Stable version identifier for the ALM Futamura specializer.
pub const TASSADAR_ALM_SPECIALIZER_VERSION: &str = "v2";
/// Claim boundary for the ALM specialization lane.
pub const TASSADAR_ALM_SPECIALIZER_CLAIM_BOUNDARY: &str = "ALM specialization rewrites reads of \
     one static seeded channel into exact ReGLU step-function fetches and removes the channel; \
     the rewrite is claimed only for programs whose reads query seeded keys, because the \
     step-function fetch totalizes the partial keyed-read function instead of refusing between \
     keys; no tensor weights, serving, or public capability copy is created";

/// One specialization report binding the rewrite to its graphs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmSpecializationReport {
    /// Specializer family identifier.
    pub specializer_family: String,
    /// Specializer version identifier.
    pub specializer_version: String,
    /// Channel baked into gate structure.
    pub specialized_channel: u16,
    /// Seed entries baked into step functions.
    pub entry_count: usize,
    /// Keyed reads rewritten into fetch subgraphs.
    pub rewritten_reads: usize,
    /// Gates in the source graph.
    pub source_gate_count: usize,
    /// Gates in the specialized graph.
    pub specialized_gate_count: usize,
    /// Indicator subgraphs reused across reads instead of rebuilt.
    pub shared_indicator_hits: usize,
    /// Source graph digest.
    pub source_graph_digest: String,
    /// Specialized graph digest.
    pub specialized_graph_digest: String,
}

impl TassadarAlmSpecializationReport {
    /// Returns a stable digest over the report encoding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_alm_specialization_report|");
        hasher.update(serde_json::to_vec(self).unwrap_or_default());
        hex::encode(hasher.finalize())
    }
}

/// Specialization failure for one ALM graph.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarAlmSpecializationError {
    /// The designated channel is not declared on the graph.
    #[error("channel {channel} is not declared on the graph")]
    UnknownChannel {
        /// Undeclared channel id.
        channel: u16,
    },
    /// The designated channel is an accumulator, not keyed memory.
    #[error("channel {channel} is an accumulator and cannot be specialized")]
    NotKeyed {
        /// Accumulator channel id.
        channel: u16,
    },
    /// The designated channel is written by a gate, so it is not static.
    #[error("channel {channel} is written by gate {gate} and is not static")]
    ChannelIsDynamic {
        /// Dynamic channel id.
        channel: u16,
        /// Writing gate index.
        gate: u32,
    },
    /// The designated channel has no seed entries to bake.
    #[error("channel {channel} has no seed entries")]
    EmptySeed {
        /// Empty channel id.
        channel: u16,
    },
}

/// Applies the first Futamura projection to one static seeded channel:
/// every read of `channel` becomes an exact ReGLU step-function fetch over
/// the channel's sorted seed entries, and the channel disappears from the
/// graph. Returns the specialized graph and its report.
pub fn specialize_tassadar_alm_graph(
    graph: &TassadarAlmGraph,
    channel: TassadarAlmChannelId,
) -> Result<(TassadarAlmGraph, TassadarAlmSpecializationReport), TassadarAlmSpecializationError> {
    let decl = graph
        .channels
        .iter()
        .find(|decl| decl.channel_id == channel)
        .ok_or(TassadarAlmSpecializationError::UnknownChannel { channel: channel.0 })?;
    if decl.kind != psionic_ir::TassadarAlmChannelKind::Keyed {
        return Err(TassadarAlmSpecializationError::NotKeyed { channel: channel.0 });
    }
    for (gate_index, gate) in graph.gates.iter().enumerate() {
        if let TassadarAlmGate::ChannelWrite { channel_id, .. } = gate {
            if *channel_id == channel {
                return Err(TassadarAlmSpecializationError::ChannelIsDynamic {
                    channel: channel.0,
                    gate: gate_index as u32,
                });
            }
        }
    }
    let mut entries: Vec<(i64, i64)> = graph
        .seed_writes
        .iter()
        .filter(|seed| seed.channel_id == channel)
        .map(|seed| (seed.key, seed.value))
        .collect();
    if entries.is_empty() {
        return Err(TassadarAlmSpecializationError::EmptySeed { channel: channel.0 });
    }
    entries.sort_by_key(|(key, _)| *key);
    // Rebuild the gate list, rewriting reads of the static channel into
    // step-function fetch subgraphs. Value ids shift as gates are inserted,
    // so carry an old-id -> new-id map.
    let mut new_gates: Vec<TassadarAlmGate> = Vec::new();
    let mut id_map: Vec<u32> = Vec::with_capacity(graph.gates.len());
    let mut rewritten_reads = 0_usize;
    // One shared constant-one gate for ReGLU step gating, created on first
    // rewritten read.
    let mut const_one: Option<u32> = None;
    // Shared step-function indicators: identical `1[q >= k]` subgraphs are
    // built once per (remapped query, threshold) and reused across reads —
    // the construction's shared-2N-neurons accounting.
    let mut indicator_cache: std::collections::BTreeMap<(u32, i64), u32> =
        std::collections::BTreeMap::new();
    let mut shared_indicator_hits = 0_usize;
    for gate in &graph.gates {
        let remap = |value: TassadarAlmValueId, id_map: &[u32]| -> TassadarAlmValueId {
            TassadarAlmValueId(id_map[value.0 as usize])
        };
        match gate {
            TassadarAlmGate::ChannelRead { channel_id, query } if *channel_id == channel => {
                rewritten_reads += 1;
                let query = remap(*query, &id_map);
                let one = match const_one {
                    Some(id) => id,
                    None => {
                        new_gates.push(TassadarAlmGate::Const { value: 1 });
                        let id = (new_gates.len() - 1) as u32;
                        const_one = Some(id);
                        id
                    }
                };
                // fetched = c0 + sum_i (c_i - c_{i-1}) * 1[q >= k_i].
                let mut terms: Vec<(i64, TassadarAlmValueId)> = Vec::new();
                let base = entries[0].1;
                for window in entries.windows(2) {
                    let (key, value) = window[1];
                    let previous_value = window[0].1;
                    let delta = value - previous_value;
                    if delta == 0 {
                        continue;
                    }
                    // 1[q >= key] = relu(q - key + 1) - relu(q - key),
                    // built once per (query, key) and shared thereafter.
                    let indicator = match indicator_cache.get(&(query.0, key)) {
                        Some(existing) => {
                            shared_indicator_hits += 1;
                            *existing
                        }
                        None => {
                            new_gates.push(TassadarAlmGate::Linear {
                                terms: vec![(1, query)],
                                bias: 1 - key,
                            });
                            let shifted_plus = (new_gates.len() - 1) as u32;
                            new_gates.push(TassadarAlmGate::ReGlu {
                                value: TassadarAlmValueId(one),
                                gate: TassadarAlmValueId(shifted_plus),
                            });
                            let relu_plus = (new_gates.len() - 1) as u32;
                            new_gates.push(TassadarAlmGate::Linear {
                                terms: vec![(1, query)],
                                bias: -key,
                            });
                            let shifted = (new_gates.len() - 1) as u32;
                            new_gates.push(TassadarAlmGate::ReGlu {
                                value: TassadarAlmValueId(one),
                                gate: TassadarAlmValueId(shifted),
                            });
                            let relu = (new_gates.len() - 1) as u32;
                            new_gates.push(TassadarAlmGate::Linear {
                                terms: vec![
                                    (1, TassadarAlmValueId(relu_plus)),
                                    (-1, TassadarAlmValueId(relu)),
                                ],
                                bias: 0,
                            });
                            let built = (new_gates.len() - 1) as u32;
                            indicator_cache.insert((query.0, key), built);
                            built
                        }
                    };
                    terms.push((delta, TassadarAlmValueId(indicator)));
                }
                new_gates.push(TassadarAlmGate::Linear { terms, bias: base });
                id_map.push((new_gates.len() - 1) as u32);
            }
            TassadarAlmGate::Input { field } => {
                new_gates.push(TassadarAlmGate::Input { field: *field });
                id_map.push((new_gates.len() - 1) as u32);
            }
            TassadarAlmGate::Const { value } => {
                new_gates.push(TassadarAlmGate::Const { value: *value });
                id_map.push((new_gates.len() - 1) as u32);
            }
            TassadarAlmGate::Linear { terms, bias } => {
                new_gates.push(TassadarAlmGate::Linear {
                    terms: terms
                        .iter()
                        .map(|(coefficient, value)| (*coefficient, remap(*value, &id_map)))
                        .collect(),
                    bias: *bias,
                });
                id_map.push((new_gates.len() - 1) as u32);
            }
            TassadarAlmGate::ReGlu { value, gate } => {
                new_gates.push(TassadarAlmGate::ReGlu {
                    value: remap(*value, &id_map),
                    gate: remap(*gate, &id_map),
                });
                id_map.push((new_gates.len() - 1) as u32);
            }
            TassadarAlmGate::ChannelWrite {
                channel_id,
                key,
                value,
            } => {
                new_gates.push(TassadarAlmGate::ChannelWrite {
                    channel_id: *channel_id,
                    key: remap(*key, &id_map),
                    value: remap(*value, &id_map),
                });
                id_map.push((new_gates.len() - 1) as u32);
            }
            TassadarAlmGate::ChannelRead { channel_id, query } => {
                new_gates.push(TassadarAlmGate::ChannelRead {
                    channel_id: *channel_id,
                    query: remap(*query, &id_map),
                });
                id_map.push((new_gates.len() - 1) as u32);
            }
            TassadarAlmGate::CumSum { channel_id, value } => {
                new_gates.push(TassadarAlmGate::CumSum {
                    channel_id: *channel_id,
                    value: remap(*value, &id_map),
                });
                id_map.push((new_gates.len() - 1) as u32);
            }
        }
    }
    let specialized = TassadarAlmGraph {
        schema_version: graph.schema_version,
        graph_id: format!("{}.specialized.{}", graph.graph_id, channel.0),
        input_field_count: graph.input_field_count,
        channels: graph
            .channels
            .iter()
            .filter(|decl| decl.channel_id != channel)
            .cloned()
            .collect(),
        seed_writes: graph
            .seed_writes
            .iter()
            .filter(|seed| seed.channel_id != channel)
            .cloned()
            .collect(),
        gates: new_gates,
        outputs: graph
            .outputs
            .iter()
            .map(|output| TassadarAlmValueId(id_map[output.0 as usize]))
            .collect(),
    };
    let report = TassadarAlmSpecializationReport {
        specializer_family: TASSADAR_ALM_SPECIALIZER_FAMILY.to_string(),
        specializer_version: TASSADAR_ALM_SPECIALIZER_VERSION.to_string(),
        specialized_channel: channel.0,
        entry_count: entries.len(),
        rewritten_reads,
        source_gate_count: graph.gates.len(),
        specialized_gate_count: specialized.gates.len(),
        source_graph_digest: graph.stable_digest(),
        specialized_graph_digest: specialized.stable_digest(),
        shared_indicator_hits,
    };
    Ok((specialized, report))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use psionic_ir::{
        tassadar_alm_verb_parity_workload, TassadarAlmChannelDecl, TassadarAlmChannelKind,
        TassadarAlmEvaluator, TassadarAlmSeedWrite, TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
    };

    use super::*;
    use crate::tassadar_alm_backend::{compile_tassadar_alm_graph, TassadarAlmCompiledExecutor};

    /// A static-program workload: a four-instruction delta program lives in
    /// a seeded channel; each step fetches program[cursor] and accumulates
    /// it. The program channel is static, so it is specializable.
    fn delta_program_graph() -> TassadarAlmGraph {
        TassadarAlmGraph {
            schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
            graph_id: "alm.test.delta_program".to_string(),
            input_field_count: 1,
            channels: vec![
                TassadarAlmChannelDecl {
                    channel_id: TassadarAlmChannelId(0),
                    name: "program".to_string(),
                    kind: TassadarAlmChannelKind::Keyed,
                },
                TassadarAlmChannelDecl {
                    channel_id: TassadarAlmChannelId(1),
                    name: "cursor".to_string(),
                    kind: TassadarAlmChannelKind::Accumulator,
                },
                TassadarAlmChannelDecl {
                    channel_id: TassadarAlmChannelId(2),
                    name: "total".to_string(),
                    kind: TassadarAlmChannelKind::Accumulator,
                },
            ],
            seed_writes: vec![
                TassadarAlmSeedWrite {
                    channel_id: TassadarAlmChannelId(0),
                    key: 0,
                    value: 3,
                },
                TassadarAlmSeedWrite {
                    channel_id: TassadarAlmChannelId(0),
                    key: 1,
                    value: 5,
                },
                TassadarAlmSeedWrite {
                    channel_id: TassadarAlmChannelId(0),
                    key: 2,
                    value: -2,
                },
                TassadarAlmSeedWrite {
                    channel_id: TassadarAlmChannelId(0),
                    key: 3,
                    value: 7,
                },
            ],
            gates: vec![
                // 0: constant one.
                TassadarAlmGate::Const { value: 1 },
                // 1: cursor = cumsum(1) - 1 needs the sum first.
                TassadarAlmGate::CumSum {
                    channel_id: TassadarAlmChannelId(1),
                    value: TassadarAlmValueId(0),
                },
                // 2: cursor index = position - 1.
                TassadarAlmGate::Linear {
                    terms: vec![(1, TassadarAlmValueId(1))],
                    bias: -1,
                },
                // 3: fetched = program[cursor].
                TassadarAlmGate::ChannelRead {
                    channel_id: TassadarAlmChannelId(0),
                    query: TassadarAlmValueId(2),
                },
                // 4: total = cumsum(fetched).
                TassadarAlmGate::CumSum {
                    channel_id: TassadarAlmChannelId(2),
                    value: TassadarAlmValueId(3),
                },
            ],
            outputs: vec![TassadarAlmValueId(3), TassadarAlmValueId(4)],
        }
    }

    #[test]
    fn specialized_graph_matches_original_outputs_and_drops_the_channel() {
        let graph = delta_program_graph();
        let steps = vec![vec![0_i64]; 4];
        let original = TassadarAlmEvaluator::evaluate(&graph, &steps).expect("evaluates");
        let (specialized, report) =
            specialize_tassadar_alm_graph(&graph, TassadarAlmChannelId(0)).expect("specializes");
        let rewritten = TassadarAlmEvaluator::evaluate(&specialized, &steps).expect("evaluates");
        assert_eq!(original.step_outputs, rewritten.step_outputs);
        assert_eq!(
            original.step_outputs,
            vec![vec![3, 3], vec![5, 8], vec![-2, 6], vec![7, 13]]
        );
        assert!(specialized.seed_writes.is_empty());
        assert!(!specialized
            .channels
            .iter()
            .any(|decl| decl.channel_id == TassadarAlmChannelId(0)));
        assert_eq!(report.entry_count, 4);
        assert_eq!(report.rewritten_reads, 1);
        assert!(report.specialized_gate_count > report.source_gate_count);
    }

    #[test]
    fn specialized_graph_compiles_and_executes_through_the_backend() {
        let graph = delta_program_graph();
        let steps = vec![vec![0_i64]; 4];
        let original = TassadarAlmEvaluator::evaluate(&graph, &steps).expect("evaluates");
        let (specialized, _) =
            specialize_tassadar_alm_graph(&graph, TassadarAlmChannelId(0)).expect("specializes");
        let bundle = compile_tassadar_alm_graph(&specialized).expect("compiles");
        let compiled = TassadarAlmCompiledExecutor::execute(&bundle, &steps).expect("executes");
        assert_eq!(compiled.step_outputs, original.step_outputs);
        assert!(bundle.seed_writes.is_empty());
    }

    #[test]
    fn dynamic_channels_refuse_specialization() {
        let graph = tassadar_alm_verb_parity_workload();
        // The parity channel is written every step; it is not static.
        let error = specialize_tassadar_alm_graph(&graph, TassadarAlmChannelId(0))
            .expect_err("refuses dynamic channel");
        assert!(matches!(
            error,
            TassadarAlmSpecializationError::ChannelIsDynamic { channel: 0, .. }
        ));
    }

    #[test]
    fn accumulator_and_unknown_channels_refuse_specialization() {
        let graph = delta_program_graph();
        assert_eq!(
            specialize_tassadar_alm_graph(&graph, TassadarAlmChannelId(1))
                .expect_err("refuses accumulator"),
            TassadarAlmSpecializationError::NotKeyed { channel: 1 }
        );
        assert_eq!(
            specialize_tassadar_alm_graph(&graph, TassadarAlmChannelId(9))
                .expect_err("refuses unknown"),
            TassadarAlmSpecializationError::UnknownChannel { channel: 9 }
        );
    }

    #[test]
    fn empty_seed_refuses_specialization() {
        let mut graph = delta_program_graph();
        graph.seed_writes.clear();
        assert_eq!(
            specialize_tassadar_alm_graph(&graph, TassadarAlmChannelId(0))
                .expect_err("refuses empty seed"),
            TassadarAlmSpecializationError::EmptySeed { channel: 0 }
        );
    }

    #[test]
    fn report_digest_is_stable() {
        let graph = delta_program_graph();
        let (_, a) =
            specialize_tassadar_alm_graph(&graph, TassadarAlmChannelId(0)).expect("specializes");
        let (_, b) =
            specialize_tassadar_alm_graph(&graph, TassadarAlmChannelId(0)).expect("specializes");
        assert_eq!(a.stable_digest(), b.stable_digest());
    }
}

#[cfg(test)]
mod shared_indicator_tests {
    #![allow(clippy::expect_used)]

    use psionic_ir::{
        TassadarAlmChannelDecl, TassadarAlmChannelKind, TassadarAlmEvaluator, TassadarAlmSeedWrite,
        TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
    };

    use super::*;

    /// Two reads of the same static channel with the same query: the second
    /// read's indicators must be shared, not rebuilt.
    #[test]
    fn same_query_reads_share_indicator_subgraphs() {
        let channel = TassadarAlmChannelId(0);
        let graph = TassadarAlmGraph {
            schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
            graph_id: "alm.specializer.shared".to_string(),
            input_field_count: 1,
            channels: vec![TassadarAlmChannelDecl {
                channel_id: channel,
                name: "table".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            }],
            seed_writes: (0..6)
                .map(|key| TassadarAlmSeedWrite {
                    channel_id: channel,
                    key,
                    value: key * 10 + 1,
                })
                .collect(),
            gates: vec![
                TassadarAlmGate::Input { field: 0 },
                TassadarAlmGate::ChannelRead {
                    channel_id: channel,
                    query: TassadarAlmValueId(0),
                },
                TassadarAlmGate::ChannelRead {
                    channel_id: channel,
                    query: TassadarAlmValueId(0),
                },
                TassadarAlmGate::Linear {
                    terms: vec![(1, TassadarAlmValueId(1)), (1, TassadarAlmValueId(2))],
                    bias: 0,
                },
            ],
            outputs: vec![TassadarAlmValueId(3)],
        };
        let (specialized, report) =
            specialize_tassadar_alm_graph(&graph, channel).expect("specializes");
        assert_eq!(report.rewritten_reads, 2);
        // Five non-zero deltas per read; the second read reuses all five.
        assert_eq!(report.shared_indicator_hits, 5);
        // Parity across in-range queries.
        for query in 0..6 {
            let original =
                TassadarAlmEvaluator::evaluate(&graph, &[vec![query]]).expect("evaluates");
            let rewritten =
                TassadarAlmEvaluator::evaluate(&specialized, &[vec![query]]).expect("evaluates");
            assert_eq!(original.step_outputs, rewritten.step_outputs);
        }
    }

    #[test]
    fn shared_specialization_is_smaller_than_double_single_read_cost() {
        let channel = TassadarAlmChannelId(0);
        let build = |reads: usize| -> TassadarAlmGraph {
            let mut gates = vec![TassadarAlmGate::Input { field: 0 }];
            for _ in 0..reads {
                gates.push(TassadarAlmGate::ChannelRead {
                    channel_id: channel,
                    query: TassadarAlmValueId(0),
                });
            }
            let outputs = vec![TassadarAlmValueId(gates.len() as u32 - 1)];
            TassadarAlmGraph {
                schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
                graph_id: format!("alm.specializer.size_{reads}"),
                input_field_count: 1,
                channels: vec![TassadarAlmChannelDecl {
                    channel_id: channel,
                    name: "table".to_string(),
                    kind: TassadarAlmChannelKind::Keyed,
                }],
                seed_writes: (0..8)
                    .map(|key| TassadarAlmSeedWrite {
                        channel_id: channel,
                        key,
                        value: key + 1,
                    })
                    .collect(),
                gates,
                outputs,
            }
        };
        let (single, _) = specialize_tassadar_alm_graph(&build(1), channel).expect("specializes");
        let (double, _) = specialize_tassadar_alm_graph(&build(2), channel).expect("specializes");
        // The second read adds only its fetch linear, not a fresh indicator
        // bank, so the double graph is far below twice the single cost.
        assert!(
            double.gates.len() < single.gates.len() + 4,
            "double {} vs single {}",
            double.gates.len(),
            single.gates.len()
        );
    }
}
