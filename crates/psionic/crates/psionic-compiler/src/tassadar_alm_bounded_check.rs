use psionic_ir::{
    TassadarAlmChannelDecl, TassadarAlmChannelId, TassadarAlmChannelKind, TassadarAlmEvaluator,
    TassadarAlmGate, TassadarAlmGraph, TassadarAlmSeedWrite, TassadarAlmValueId,
    TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::tassadar_alm_backend::{
    compile_tassadar_alm_graph, tassadar_alm_errors_match, tassadar_alm_phase_kind,
    TassadarAlmCompiledBundle, TassadarAlmCompiledExecutor, TassadarAlmPhaseKind,
};
use crate::tassadar_alm_geometric::tassadar_alm_geometric_execute;
use crate::tassadar_alm_hull::tassadar_alm_hull_execute;
use crate::tassadar_alm_numeric::{
    materialize_tassadar_alm_numeric, tassadar_alm_numeric_execute,
    TassadarAlmNumericExecutionError,
};

/// Claim boundary for the bounded differential check harness.
pub const TASSADAR_ALM_BOUNDED_CHECK_CLAIM_BOUNDARY: &str = "the bounded check harness \
     exercises evaluator/compiled parity (including refusal parity) and independent \
     allocator-safety invariants over seeded generated graphs within a fixed size budget; it is \
     strong bounded evidence, not a proof, and creates no capability claim";

/// One independent bundle-invariant violation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarAlmInvariantViolation {
    /// A consumer is not scheduled strictly after its producer.
    #[error("gate {gate} consumes value {value} not strictly earlier (producer phase {producer_phase}, consumer phase {consumer_phase})")]
    PrecedenceViolated {
        /// Consuming gate index.
        gate: u32,
        /// Consumed value id.
        value: u32,
        /// Producer phase.
        producer_phase: u32,
        /// Consumer phase.
        consumer_phase: u32,
    },
    /// A gate sits in a phase of the wrong kind.
    #[error("gate {gate} placed in phase {phase} of kind {kind:?}")]
    PhaseKindViolated {
        /// Misplaced gate index.
        gate: u32,
        /// Assigned phase.
        phase: u32,
        /// Actual phase kind.
        kind: TassadarAlmPhaseKind,
    },
    /// Two gates with overlapping lifetimes share one residual slot.
    #[error("slot {slot} shared by gates {first} and {second} with overlapping lifetimes")]
    SlotLifetimeOverlap {
        /// Shared slot.
        slot: u32,
        /// First gate index.
        first: u32,
        /// Second gate index.
        second: u32,
    },
    /// A slot reuse has no matching subtraction record.
    #[error("slot {slot} reused by gate {gate} without a subtraction record")]
    MissingSubtraction {
        /// Reused slot.
        slot: u32,
        /// Reusing gate index.
        gate: u32,
    },
    /// Two cumsums on one accumulator channel are scheduled out of order.
    #[error("cumsum gate {gate} on channel {channel} does not follow the previous cumsum")]
    CumsumOrderViolated {
        /// Offending gate index.
        gate: u32,
        /// Accumulator channel id.
        channel: u16,
    },
    /// The bundle's placement table does not cover the graph.
    #[error("bundle placements cover {found} gates, graph declares {expected}")]
    PlacementArityMismatch {
        /// Placement count.
        found: usize,
        /// Gate count.
        expected: usize,
    },
}

fn gate_dependencies(gate: &TassadarAlmGate) -> Vec<u32> {
    match gate {
        TassadarAlmGate::Input { .. } | TassadarAlmGate::Const { .. } => Vec::new(),
        TassadarAlmGate::Linear { terms, .. } => terms.iter().map(|(_, value)| value.0).collect(),
        TassadarAlmGate::ReGlu { value, gate } => vec![value.0, gate.0],
        TassadarAlmGate::ChannelWrite { key, value, .. } => vec![key.0, value.0],
        TassadarAlmGate::ChannelRead { query, .. } => vec![query.0],
        TassadarAlmGate::CumSum { value, .. } => vec![value.0],
    }
}

/// Independently verifies a compiled bundle's structural invariants
/// against its source graph, without reusing scheduler internals.
pub fn tassadar_alm_check_bundle_invariants(
    graph: &TassadarAlmGraph,
    bundle: &TassadarAlmCompiledBundle,
) -> Result<(), TassadarAlmInvariantViolation> {
    if bundle.placements.len() != graph.gates.len() {
        return Err(TassadarAlmInvariantViolation::PlacementArityMismatch {
            found: bundle.placements.len(),
            expected: graph.gates.len(),
        });
    }
    let phases: Vec<u32> = bundle.placements.iter().map(|p| p.phase).collect();
    let slots: Vec<u32> = bundle.placements.iter().map(|p| p.slot).collect();
    // Aliased write gates share their value operand's slot by design.
    let alias: Vec<Option<u32>> = graph
        .gates
        .iter()
        .map(|gate| match gate {
            TassadarAlmGate::ChannelWrite { value, .. } => Some(value.0),
            _ => None,
        })
        .collect();
    // Precedence and phase kinds.
    for (gate_index, gate) in graph.gates.iter().enumerate() {
        let phase = phases[gate_index];
        let expected_kind = match gate {
            TassadarAlmGate::Input { .. } | TassadarAlmGate::Const { .. } => {
                Some(TassadarAlmPhaseKind::Embedding)
            }
            TassadarAlmGate::Linear { .. } => Some(TassadarAlmPhaseKind::Persist),
            TassadarAlmGate::ReGlu { .. } => Some(TassadarAlmPhaseKind::Ffn),
            TassadarAlmGate::ChannelRead { .. } | TassadarAlmGate::CumSum { .. } => {
                Some(TassadarAlmPhaseKind::Attention)
            }
            TassadarAlmGate::ChannelWrite { .. } => None,
        };
        if let Some(kind) = expected_kind {
            if tassadar_alm_phase_kind(phase) != kind {
                return Err(TassadarAlmInvariantViolation::PhaseKindViolated {
                    gate: gate_index as u32,
                    phase,
                    kind: tassadar_alm_phase_kind(phase),
                });
            }
        }
        if !matches!(gate, TassadarAlmGate::ChannelWrite { .. }) {
            for dep in gate_dependencies(gate) {
                if phases[dep as usize] >= phase {
                    return Err(TassadarAlmInvariantViolation::PrecedenceViolated {
                        gate: gate_index as u32,
                        value: dep,
                        producer_phase: phases[dep as usize],
                        consumer_phase: phase,
                    });
                }
            }
        }
    }
    // Accumulator contributions are order-sensitive: same-channel cumsums
    // must keep their gate order in the schedule.
    let mut last_cumsum_phase: std::collections::BTreeMap<u16, u32> =
        std::collections::BTreeMap::new();
    for (gate_index, gate) in graph.gates.iter().enumerate() {
        if let TassadarAlmGate::CumSum { channel_id, .. } = gate {
            let phase = phases[gate_index];
            if let Some(previous) = last_cumsum_phase.get(&channel_id.0) {
                if phase <= *previous {
                    return Err(TassadarAlmInvariantViolation::CumsumOrderViolated {
                        gate: gate_index as u32,
                        channel: channel_id.0,
                    });
                }
            }
            last_cumsum_phase.insert(channel_id.0, phase);
        }
    }
    // Independent lifetime computation: birth = own phase; death = max
    // consumer phase, end-of-step for outputs and write operands.
    let max_phase = phases.iter().copied().max().unwrap_or(0);
    let end_phase = max_phase + 1;
    let mut deaths: Vec<u32> = phases.clone();
    for (gate_index, gate) in graph.gates.iter().enumerate() {
        let phase = phases[gate_index];
        for dep in gate_dependencies(gate) {
            let until = if matches!(gate, TassadarAlmGate::ChannelWrite { .. }) {
                end_phase
            } else {
                phase
            };
            if deaths[dep as usize] < until {
                deaths[dep as usize] = until;
            }
        }
    }
    for output in &graph.outputs {
        if deaths[output.0 as usize] < end_phase {
            deaths[output.0 as usize] = end_phase;
        }
    }
    // Slot lifetime disjointness for non-aliased producers, and
    // subtraction completeness for every reuse.
    let mut producers: Vec<usize> = (0..graph.gates.len())
        .filter(|index| alias[*index].is_none())
        .collect();
    producers.sort_by_key(|index| (phases[*index], *index));
    let mut last_on_slot: std::collections::BTreeMap<u32, usize> =
        std::collections::BTreeMap::new();
    for gate_index in producers {
        let slot = slots[gate_index];
        if let Some(previous) = last_on_slot.get(&slot).copied() {
            // Overlap check: the previous occupant must die strictly before
            // this gate's birth.
            if deaths[previous] >= phases[gate_index] {
                return Err(TassadarAlmInvariantViolation::SlotLifetimeOverlap {
                    slot,
                    first: previous as u32,
                    second: gate_index as u32,
                });
            }
            let has_subtraction = bundle.subtractions.iter().any(|subtraction| {
                subtraction.slot == slot
                    && subtraction.phase == phases[gate_index]
                    && subtraction.stale_gate == previous as u32
            });
            if !has_subtraction {
                return Err(TassadarAlmInvariantViolation::MissingSubtraction {
                    slot,
                    gate: gate_index as u32,
                });
            }
        }
        last_on_slot.insert(slot, gate_index);
    }
    Ok(())
}

/// One digest-pinned bounded-check report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmBoundedCheckReport {
    /// Seed driving the deterministic enumeration.
    pub seed: u64,
    /// Graphs generated and checked.
    pub graphs_checked: usize,
    /// Runs where both legs produced identical outputs.
    pub parity_agreements: usize,
    /// Runs where both legs refused with matching error families.
    pub refusal_agreements: usize,
    /// Parity failures (must be zero for a passing report).
    pub parity_failures: usize,
    /// Invariant violations (must be zero for a passing report).
    pub invariant_violations: usize,
}

impl TassadarAlmBoundedCheckReport {
    /// Returns a stable digest over the report encoding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_alm_bounded_check_report|");
        hasher.update(serde_json::to_vec(self).unwrap_or_default());
        hex::encode(hasher.finalize())
    }
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn pick(state: &mut u64, bound: usize) -> usize {
    (splitmix64(state) % bound as u64) as usize
}

fn small_value(state: &mut u64) -> i64 {
    (splitmix64(state) % 9) as i64 - 4
}

/// Generates one bounded random-but-deterministic graph.
fn generate_graph(state: &mut u64, index: usize, gate_budget: usize) -> TassadarAlmGraph {
    let keyed = TassadarAlmChannelId(0);
    let accumulator = TassadarAlmChannelId(1);
    let mut gates: Vec<TassadarAlmGate> = vec![TassadarAlmGate::Input { field: 0 }];
    let target = 2 + pick(state, gate_budget.saturating_sub(2).max(1));
    while gates.len() < target {
        let prior = gates.len();
        let reference = |state: &mut u64| TassadarAlmValueId(pick(state, prior) as u32);
        let choice = pick(state, 6);
        let gate = match choice {
            0 => TassadarAlmGate::Const {
                value: small_value(state),
            },
            1 => {
                let first = reference(state);
                let second = reference(state);
                TassadarAlmGate::Linear {
                    terms: vec![(small_value(state), first), (small_value(state), second)],
                    bias: small_value(state),
                }
            }
            2 => TassadarAlmGate::ReGlu {
                value: reference(state),
                gate: reference(state),
            },
            3 => TassadarAlmGate::ChannelWrite {
                channel_id: keyed,
                key: reference(state),
                value: reference(state),
            },
            4 => TassadarAlmGate::ChannelRead {
                channel_id: keyed,
                query: reference(state),
            },
            _ => TassadarAlmGate::CumSum {
                channel_id: accumulator,
                value: reference(state),
            },
        };
        gates.push(gate);
    }
    let outputs = vec![TassadarAlmValueId((gates.len() - 1) as u32)];
    // Generous seeds keep many reads in range while still leaving
    // out-of-range queries reachable, so both paths are exercised.
    let seed_writes = (-6..=6)
        .map(|key| TassadarAlmSeedWrite {
            channel_id: keyed,
            key,
            value: small_value(state),
        })
        .collect();
    TassadarAlmGraph {
        schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
        graph_id: format!("alm.bounded_check.{index}"),
        input_field_count: 1,
        channels: vec![
            TassadarAlmChannelDecl {
                channel_id: keyed,
                name: "memory".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            },
            TassadarAlmChannelDecl {
                channel_id: accumulator,
                name: "totals".to_string(),
                kind: TassadarAlmChannelKind::Accumulator,
            },
        ],
        seed_writes,
        gates,
        outputs,
    }
}

/// Runs the bounded differential check over `graph_count` generated graphs.
#[must_use]
pub fn tassadar_alm_bounded_check(
    seed: u64,
    graph_count: usize,
    gate_budget: usize,
) -> TassadarAlmBoundedCheckReport {
    let mut state = seed;
    let mut parity_agreements = 0_usize;
    let mut refusal_agreements = 0_usize;
    let mut parity_failures = 0_usize;
    let mut invariant_violations = 0_usize;
    for index in 0..graph_count {
        let graph = generate_graph(&mut state, index, gate_budget);
        let steps: Vec<Vec<i64>> = (0..3).map(|_| vec![small_value(&mut state)]).collect();
        let Ok(bundle) = compile_tassadar_alm_graph(&graph) else {
            // Generated graphs are valid by construction; a compile failure
            // is an invariant violation of the harness itself.
            invariant_violations += 1;
            continue;
        };
        if tassadar_alm_check_bundle_invariants(&graph, &bundle).is_err() {
            invariant_violations += 1;
            continue;
        }
        let evaluated = TassadarAlmEvaluator::evaluate(&graph, &steps);
        let compiled = TassadarAlmCompiledExecutor::execute(&bundle, &steps);
        let geometric = tassadar_alm_geometric_execute(&bundle, &steps);
        let hull = tassadar_alm_hull_execute(&bundle, &steps);
        let numeric = {
            let model = materialize_tassadar_alm_numeric(&bundle);
            tassadar_alm_numeric_execute(&model, &steps)
        };
        // The numeric leg's domain is narrower by design: its 2^53 exactness
        // window sits inside i64, so a window breach is an acceptable
        // demotion regardless of how the integer legs fared. Inside the
        // window it must agree exactly.
        let numeric_consistent = match (&evaluated, &numeric) {
            (_, Err(TassadarAlmNumericExecutionError::ExactnessWindowExceeded { .. })) => true,
            (Ok(reference), Ok(numeric_trace)) => {
                reference.step_outputs == numeric_trace.step_outputs
            }
            (
                Err(psionic_ir::TassadarAlmEvaluationError::MissingKey { .. }),
                Err(TassadarAlmNumericExecutionError::MissingKey { .. }),
            ) => true,
            _ => false,
        };
        match (evaluated, compiled, geometric, hull) {
            (Ok(reference), Ok(executed), Ok(geometric_trace), Ok(hull_trace)) => {
                if reference.step_outputs == executed.step_outputs
                    && reference.step_outputs == geometric_trace.step_outputs
                    && reference.step_outputs == hull_trace.step_outputs
                    && numeric_consistent
                {
                    parity_agreements += 1;
                } else {
                    parity_failures += 1;
                }
            }
            (Err(reference_error), Err(executed_error), Err(geometric_error), Err(hull_error)) => {
                if tassadar_alm_errors_match(&reference_error, &executed_error)
                    && tassadar_alm_errors_match(&reference_error, &geometric_error)
                    && tassadar_alm_errors_match(&reference_error, &hull_error)
                    && numeric_consistent
                {
                    refusal_agreements += 1;
                } else {
                    parity_failures += 1;
                }
            }
            _ => {
                parity_failures += 1;
            }
        }
    }
    TassadarAlmBoundedCheckReport {
        seed,
        graphs_checked: graph_count,
        parity_agreements,
        refusal_agreements,
        parity_failures,
        invariant_violations,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use psionic_ir::tassadar_alm_stack_micro_workload;

    use super::*;

    /// The committed bounded run: one pinned seed, zero failures allowed.
    #[test]
    fn bounded_differential_check_passes_on_the_committed_seed() {
        let report = tassadar_alm_bounded_check(0xA11C_E5EE_D001, 400, 12);
        assert_eq!(report.parity_failures, 0, "report: {report:?}");
        assert_eq!(report.invariant_violations, 0, "report: {report:?}");
        assert_eq!(report.graphs_checked, 400);
        // Both the success and refusal paths must actually be exercised.
        assert!(report.parity_agreements > 0, "report: {report:?}");
        assert!(report.refusal_agreements > 0, "report: {report:?}");
    }

    #[test]
    fn bounded_check_reports_are_deterministic() {
        let a = tassadar_alm_bounded_check(7, 50, 10);
        let b = tassadar_alm_bounded_check(7, 50, 10);
        assert_eq!(a.stable_digest(), b.stable_digest());
    }

    #[test]
    fn invariant_checker_accepts_honest_bundles() {
        let graph = tassadar_alm_stack_micro_workload();
        let bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        tassadar_alm_check_bundle_invariants(&graph, &bundle).expect("invariants hold");
    }

    #[test]
    fn invariant_checker_catches_forced_slot_collisions() {
        let graph = tassadar_alm_stack_micro_workload();
        let mut bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        // Force two live values onto one slot.
        let victim = bundle
            .placements
            .iter()
            .position(|placement| placement.slot != bundle.placements[0].slot)
            .expect("distinct slots exist");
        bundle.placements[victim].slot = bundle.placements[0].slot;
        let violation = tassadar_alm_check_bundle_invariants(&graph, &bundle).expect_err("catches");
        assert!(matches!(
            violation,
            TassadarAlmInvariantViolation::SlotLifetimeOverlap { .. }
                | TassadarAlmInvariantViolation::MissingSubtraction { .. }
        ));
    }

    #[test]
    fn invariant_checker_catches_dropped_subtractions() {
        let graph = tassadar_alm_stack_micro_workload();
        let mut bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        assert!(!bundle.subtractions.is_empty());
        bundle.subtractions.clear();
        let violation = tassadar_alm_check_bundle_invariants(&graph, &bundle).expect_err("catches");
        assert!(matches!(
            violation,
            TassadarAlmInvariantViolation::MissingSubtraction { .. }
        ));
    }

    #[test]
    fn invariant_checker_catches_placement_arity_mismatch() {
        let graph = tassadar_alm_stack_micro_workload();
        let mut bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        bundle.placements.pop();
        assert!(matches!(
            tassadar_alm_check_bundle_invariants(&graph, &bundle).expect_err("catches"),
            TassadarAlmInvariantViolation::PlacementArityMismatch { .. }
        ));
    }
}
