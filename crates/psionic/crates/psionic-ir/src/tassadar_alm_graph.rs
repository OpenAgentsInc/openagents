use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for the bounded Tassadar ALM gate-graph IR.
pub const TASSADAR_ALM_GRAPH_SCHEMA_VERSION: u16 = 1;
/// Stable language identifier for the bounded Tassadar ALM gate-graph IR.
pub const TASSADAR_ALM_GRAPH_LANGUAGE_ID: &str = "tassadar.alm_gate_graph.v1";
/// Coarse claim class for the ALM gate-graph reference lane.
pub const TASSADAR_ALM_GRAPH_CLAIM_CLASS: &str = "exact_integer_reference_semantics";
/// Claim boundary for the ALM gate-graph reference lane.
pub const TASSADAR_ALM_GRAPH_CLAIM_BOUNDARY: &str = "the ALM gate graph and its exact evaluator \
     define executor-compiler reference semantics over checked 64-bit integers only; they do not \
     emit transformer weights, do not schedule layers, do not accept Wasm, and do not create any \
     learned-model, served-route, or transformer-execution claim";

/// Maximum number of gates one ALM graph may declare.
pub const TASSADAR_ALM_GRAPH_MAX_GATES: usize = 65_536;
/// Maximum number of channels one ALM graph may declare.
pub const TASSADAR_ALM_GRAPH_MAX_CHANNELS: usize = 256;
/// Maximum number of linear terms one gate may combine.
pub const TASSADAR_ALM_GRAPH_MAX_LINEAR_TERMS: usize = 64;
/// Maximum number of execution steps one evaluation may run.
pub const TASSADAR_ALM_EVALUATION_MAX_STEPS: usize = 1_048_576;

/// One value produced by an earlier gate in the per-step gate list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TassadarAlmValueId(pub u32);

/// One declared channel in the ALM graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TassadarAlmChannelId(pub u16);

/// Channel family for one declared ALM channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAlmChannelKind {
    /// Keyed write/read memory with latest-write-wins semantics.
    Keyed,
    /// Accumulator carrying an exact running sum.
    Accumulator,
}

/// One declared channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmChannelDecl {
    /// Stable channel identifier referenced by gates.
    pub channel_id: TassadarAlmChannelId,
    /// Stable human-readable channel name.
    pub name: String,
    /// Channel family.
    pub kind: TassadarAlmChannelKind,
}

/// One seed write applied to a keyed channel before step zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmSeedWrite {
    /// Target keyed channel.
    pub channel_id: TassadarAlmChannelId,
    /// Seed key.
    pub key: i64,
    /// Seed value.
    pub value: i64,
}

/// One ALM gate. Gates may only reference values produced by earlier gates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAlmGate {
    /// One per-step input field supplied by the caller.
    Input {
        /// Zero-based input field index.
        field: u16,
    },
    /// One integer constant.
    Const {
        /// Constant value.
        value: i64,
    },
    /// One exact linear combination of earlier values plus a bias.
    Linear {
        /// `(coefficient, value)` terms combined with checked arithmetic.
        terms: Vec<(i64, TassadarAlmValueId)>,
        /// Constant bias term.
        bias: i64,
    },
    /// One gated product `value * max(gate, 0)` (the ReGLU primitive).
    ReGlu {
        /// Multiplicand value.
        value: TassadarAlmValueId,
        /// Gate value passed through `max(_, 0)` before multiplication.
        gate: TassadarAlmValueId,
    },
    /// One keyed-channel write. Visible to reads from the next step onward.
    ChannelWrite {
        /// Target keyed channel.
        channel_id: TassadarAlmChannelId,
        /// Key value.
        key: TassadarAlmValueId,
        /// Written value. The gate's own output is the written value.
        value: TassadarAlmValueId,
    },
    /// One keyed-channel read with latest-write-wins semantics over seed
    /// writes plus writes from strictly earlier steps.
    ChannelRead {
        /// Source keyed channel.
        channel_id: TassadarAlmChannelId,
        /// Query key value.
        query: TassadarAlmValueId,
    },
    /// One accumulator contribution returning the exact running sum over all
    /// steps so far, including the current step's contribution.
    CumSum {
        /// Target accumulator channel.
        channel_id: TassadarAlmChannelId,
        /// Value contributed at this step.
        value: TassadarAlmValueId,
    },
}

/// One bounded ALM gate graph executed once per token step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmGraph {
    /// Schema version for this IR encoding.
    pub schema_version: u16,
    /// Stable graph identifier.
    pub graph_id: String,
    /// Number of per-step input fields the graph expects.
    pub input_field_count: u16,
    /// Declared channels.
    pub channels: Vec<TassadarAlmChannelDecl>,
    /// Seed writes applied to keyed channels before step zero.
    pub seed_writes: Vec<TassadarAlmSeedWrite>,
    /// Per-step gates in execution order.
    pub gates: Vec<TassadarAlmGate>,
    /// Values exposed as per-step outputs.
    pub outputs: Vec<TassadarAlmValueId>,
}

/// Validation failure for one ALM gate graph.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarAlmGraphError {
    /// The schema version is not supported.
    #[error("unsupported ALM graph schema version `{found}`")]
    UnsupportedSchemaVersion {
        /// Declared schema version.
        found: u16,
    },
    /// The graph identifier is empty.
    #[error("ALM graph id must not be empty")]
    EmptyGraphId,
    /// The graph declares no gates.
    #[error("ALM graph must declare at least one gate")]
    EmptyGates,
    /// The graph declares more gates than the bounded maximum.
    #[error("ALM graph declares {found} gates above the bound {bound}")]
    TooManyGates {
        /// Declared gate count.
        found: usize,
        /// Bounded maximum.
        bound: usize,
    },
    /// The graph declares more channels than the bounded maximum.
    #[error("ALM graph declares {found} channels above the bound {bound}")]
    TooManyChannels {
        /// Declared channel count.
        found: usize,
        /// Bounded maximum.
        bound: usize,
    },
    /// Two channel declarations share one channel id.
    #[error("ALM channel id {channel} is declared twice")]
    DuplicateChannel {
        /// Duplicated channel id.
        channel: u16,
    },
    /// A gate references a channel that is not declared.
    #[error("gate {gate} references undeclared channel {channel}")]
    UnknownChannel {
        /// Referencing gate index.
        gate: u32,
        /// Undeclared channel id.
        channel: u16,
    },
    /// A gate uses a channel with the wrong channel kind.
    #[error("gate {gate} uses channel {channel} with mismatched kind")]
    ChannelKindMismatch {
        /// Referencing gate index.
        gate: u32,
        /// Mismatched channel id.
        channel: u16,
    },
    /// A seed write targets a channel that is not declared keyed.
    #[error("seed write targets channel {channel} which is not a keyed channel")]
    SeedWriteChannelInvalid {
        /// Invalid channel id.
        channel: u16,
    },
    /// A gate references a value at or after its own position.
    #[error("gate {gate} references forward value {value}")]
    ForwardReference {
        /// Referencing gate index.
        gate: u32,
        /// Referenced forward value id.
        value: u32,
    },
    /// A gate references an input field at or above the declared count.
    #[error("gate {gate} references input field {field} above declared count {declared}")]
    InputFieldOutOfRange {
        /// Referencing gate index.
        gate: u32,
        /// Referenced field index.
        field: u16,
        /// Declared input field count.
        declared: u16,
    },
    /// A linear gate declares more terms than the bounded maximum.
    #[error("gate {gate} declares {found} linear terms above the bound {bound}")]
    TooManyLinearTerms {
        /// Referencing gate index.
        gate: u32,
        /// Declared term count.
        found: usize,
        /// Bounded maximum.
        bound: usize,
    },
    /// The graph declares no outputs.
    #[error("ALM graph must declare at least one output value")]
    EmptyOutputs,
    /// An output references a value the graph does not produce.
    #[error("output references unknown value {value}")]
    OutputOutOfRange {
        /// Referenced value id.
        value: u32,
    },
}

impl TassadarAlmGraph {
    /// Validates the bounded structural rules for this graph.
    pub fn validate(&self) -> Result<(), TassadarAlmGraphError> {
        if self.schema_version != TASSADAR_ALM_GRAPH_SCHEMA_VERSION {
            return Err(TassadarAlmGraphError::UnsupportedSchemaVersion {
                found: self.schema_version,
            });
        }
        if self.graph_id.trim().is_empty() {
            return Err(TassadarAlmGraphError::EmptyGraphId);
        }
        if self.gates.is_empty() {
            return Err(TassadarAlmGraphError::EmptyGates);
        }
        if self.gates.len() > TASSADAR_ALM_GRAPH_MAX_GATES {
            return Err(TassadarAlmGraphError::TooManyGates {
                found: self.gates.len(),
                bound: TASSADAR_ALM_GRAPH_MAX_GATES,
            });
        }
        if self.channels.len() > TASSADAR_ALM_GRAPH_MAX_CHANNELS {
            return Err(TassadarAlmGraphError::TooManyChannels {
                found: self.channels.len(),
                bound: TASSADAR_ALM_GRAPH_MAX_CHANNELS,
            });
        }
        let mut channel_kinds: BTreeMap<u16, TassadarAlmChannelKind> = BTreeMap::new();
        for decl in &self.channels {
            if channel_kinds.insert(decl.channel_id.0, decl.kind).is_some() {
                return Err(TassadarAlmGraphError::DuplicateChannel {
                    channel: decl.channel_id.0,
                });
            }
        }
        for seed in &self.seed_writes {
            match channel_kinds.get(&seed.channel_id.0) {
                Some(TassadarAlmChannelKind::Keyed) => {}
                _ => {
                    return Err(TassadarAlmGraphError::SeedWriteChannelInvalid {
                        channel: seed.channel_id.0,
                    });
                }
            }
        }
        let check_ref =
            |gate_index: usize, value: TassadarAlmValueId| -> Result<(), TassadarAlmGraphError> {
                if value.0 as usize >= gate_index {
                    return Err(TassadarAlmGraphError::ForwardReference {
                        gate: gate_index as u32,
                        value: value.0,
                    });
                }
                Ok(())
            };
        let check_channel = |gate_index: usize,
                             channel: TassadarAlmChannelId,
                             expected: TassadarAlmChannelKind|
         -> Result<(), TassadarAlmGraphError> {
            match channel_kinds.get(&channel.0) {
                None => Err(TassadarAlmGraphError::UnknownChannel {
                    gate: gate_index as u32,
                    channel: channel.0,
                }),
                Some(kind) if *kind != expected => {
                    Err(TassadarAlmGraphError::ChannelKindMismatch {
                        gate: gate_index as u32,
                        channel: channel.0,
                    })
                }
                Some(_) => Ok(()),
            }
        };
        for (gate_index, gate) in self.gates.iter().enumerate() {
            match gate {
                TassadarAlmGate::Input { field } => {
                    if *field >= self.input_field_count {
                        return Err(TassadarAlmGraphError::InputFieldOutOfRange {
                            gate: gate_index as u32,
                            field: *field,
                            declared: self.input_field_count,
                        });
                    }
                }
                TassadarAlmGate::Const { .. } => {}
                TassadarAlmGate::Linear { terms, .. } => {
                    if terms.len() > TASSADAR_ALM_GRAPH_MAX_LINEAR_TERMS {
                        return Err(TassadarAlmGraphError::TooManyLinearTerms {
                            gate: gate_index as u32,
                            found: terms.len(),
                            bound: TASSADAR_ALM_GRAPH_MAX_LINEAR_TERMS,
                        });
                    }
                    for (_, value) in terms {
                        check_ref(gate_index, *value)?;
                    }
                }
                TassadarAlmGate::ReGlu { value, gate } => {
                    check_ref(gate_index, *value)?;
                    check_ref(gate_index, *gate)?;
                }
                TassadarAlmGate::ChannelWrite {
                    channel_id,
                    key,
                    value,
                } => {
                    check_channel(gate_index, *channel_id, TassadarAlmChannelKind::Keyed)?;
                    check_ref(gate_index, *key)?;
                    check_ref(gate_index, *value)?;
                }
                TassadarAlmGate::ChannelRead { channel_id, query } => {
                    check_channel(gate_index, *channel_id, TassadarAlmChannelKind::Keyed)?;
                    check_ref(gate_index, *query)?;
                }
                TassadarAlmGate::CumSum { channel_id, value } => {
                    check_channel(gate_index, *channel_id, TassadarAlmChannelKind::Accumulator)?;
                    check_ref(gate_index, *value)?;
                }
            }
        }
        if self.outputs.is_empty() {
            return Err(TassadarAlmGraphError::EmptyOutputs);
        }
        for output in &self.outputs {
            if output.0 as usize >= self.gates.len() {
                return Err(TassadarAlmGraphError::OutputOutOfRange { value: output.0 });
            }
        }
        Ok(())
    }

    /// Returns a stable digest over the full graph encoding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_alm_graph|");
        hasher.update(serde_json::to_vec(self).unwrap_or_default());
        hex::encode(hasher.finalize())
    }
}

/// Evaluation failure for one ALM evaluation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarAlmEvaluationError {
    /// The graph failed structural validation.
    #[error(transparent)]
    Graph(#[from] TassadarAlmGraphError),
    /// The evaluation exceeds the bounded step budget.
    #[error("ALM evaluation over {found} steps exceeds the bound {bound}")]
    TooManySteps {
        /// Requested step count.
        found: usize,
        /// Bounded maximum.
        bound: usize,
    },
    /// One step supplies the wrong number of input fields.
    #[error("step {step} supplies {found} input fields, expected {expected}")]
    InputArityMismatch {
        /// Failing step index.
        step: usize,
        /// Supplied field count.
        found: usize,
        /// Expected field count.
        expected: u16,
    },
    /// One keyed read found no write under its query key.
    #[error("step {step} gate {gate} read missing key {key} on channel {channel}")]
    MissingKey {
        /// Failing step index.
        step: usize,
        /// Failing gate index.
        gate: u32,
        /// Queried channel id.
        channel: u16,
        /// Missing key value.
        key: i64,
    },
    /// One exact integer operation overflowed 64 bits.
    #[error("step {step} gate {gate} overflowed exact 64-bit arithmetic")]
    Overflow {
        /// Failing step index.
        step: usize,
        /// Failing gate index.
        gate: u32,
    },
}

/// One deterministic evaluation trace over an ALM graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmTrace {
    /// Stable digest of the evaluated graph.
    pub graph_digest: String,
    /// Number of executed steps.
    pub step_count: usize,
    /// Declared per-step outputs, one row per step.
    pub step_outputs: Vec<Vec<i64>>,
    /// Total keyed-channel writes applied, including seed writes.
    pub keyed_write_count: usize,
    /// Stable digest over the output rows.
    pub trace_digest: String,
}

/// Exact-arithmetic reference evaluator for ALM gate graphs.
#[derive(Clone, Copy, Debug, Default)]
pub struct TassadarAlmEvaluator;

impl TassadarAlmEvaluator {
    /// Evaluates `graph` over `steps`, one input-field row per step.
    pub fn evaluate(
        graph: &TassadarAlmGraph,
        steps: &[Vec<i64>],
    ) -> Result<TassadarAlmTrace, TassadarAlmEvaluationError> {
        graph.validate()?;
        if steps.len() > TASSADAR_ALM_EVALUATION_MAX_STEPS {
            return Err(TassadarAlmEvaluationError::TooManySteps {
                found: steps.len(),
                bound: TASSADAR_ALM_EVALUATION_MAX_STEPS,
            });
        }
        let mut keyed: BTreeMap<u16, BTreeMap<i64, i64>> = BTreeMap::new();
        let mut accumulators: BTreeMap<u16, i64> = BTreeMap::new();
        for decl in &graph.channels {
            match decl.kind {
                TassadarAlmChannelKind::Keyed => {
                    keyed.insert(decl.channel_id.0, BTreeMap::new());
                }
                TassadarAlmChannelKind::Accumulator => {
                    accumulators.insert(decl.channel_id.0, 0);
                }
            }
        }
        let mut keyed_write_count = 0_usize;
        for seed in &graph.seed_writes {
            let cells = keyed.get_mut(&seed.channel_id.0).ok_or(
                TassadarAlmGraphError::SeedWriteChannelInvalid {
                    channel: seed.channel_id.0,
                },
            )?;
            cells.insert(seed.key, seed.value);
            keyed_write_count += 1;
        }
        let mut step_outputs: Vec<Vec<i64>> = Vec::with_capacity(steps.len());
        for (step_index, fields) in steps.iter().enumerate() {
            if fields.len() != graph.input_field_count as usize {
                return Err(TassadarAlmEvaluationError::InputArityMismatch {
                    step: step_index,
                    found: fields.len(),
                    expected: graph.input_field_count,
                });
            }
            let mut values: Vec<i64> = Vec::with_capacity(graph.gates.len());
            // Writes from this step become visible to reads at later steps.
            let mut pending_writes: Vec<(u16, i64, i64)> = Vec::new();
            for (gate_index, gate) in graph.gates.iter().enumerate() {
                let produced = match gate {
                    TassadarAlmGate::Input { field } => fields[*field as usize],
                    TassadarAlmGate::Const { value } => *value,
                    TassadarAlmGate::Linear { terms, bias } => {
                        let mut total = *bias;
                        for (coefficient, value) in terms {
                            let term = coefficient.checked_mul(values[value.0 as usize]).ok_or(
                                TassadarAlmEvaluationError::Overflow {
                                    step: step_index,
                                    gate: gate_index as u32,
                                },
                            )?;
                            total = total.checked_add(term).ok_or(
                                TassadarAlmEvaluationError::Overflow {
                                    step: step_index,
                                    gate: gate_index as u32,
                                },
                            )?;
                        }
                        total
                    }
                    TassadarAlmGate::ReGlu { value, gate } => {
                        let gated = values[gate.0 as usize].max(0);
                        values[value.0 as usize].checked_mul(gated).ok_or(
                            TassadarAlmEvaluationError::Overflow {
                                step: step_index,
                                gate: gate_index as u32,
                            },
                        )?
                    }
                    TassadarAlmGate::ChannelWrite {
                        channel_id,
                        key,
                        value,
                    } => {
                        let written = values[value.0 as usize];
                        pending_writes.push((channel_id.0, values[key.0 as usize], written));
                        written
                    }
                    TassadarAlmGate::ChannelRead { channel_id, query } => {
                        let key = values[query.0 as usize];
                        let cells = keyed.get(&channel_id.0).ok_or(
                            TassadarAlmGraphError::UnknownChannel {
                                gate: gate_index as u32,
                                channel: channel_id.0,
                            },
                        )?;
                        *cells
                            .get(&key)
                            .ok_or(TassadarAlmEvaluationError::MissingKey {
                                step: step_index,
                                gate: gate_index as u32,
                                channel: channel_id.0,
                                key,
                            })?
                    }
                    TassadarAlmGate::CumSum { channel_id, value } => {
                        let contribution = values[value.0 as usize];
                        let current = accumulators.get(&channel_id.0).copied().ok_or(
                            TassadarAlmGraphError::UnknownChannel {
                                gate: gate_index as u32,
                                channel: channel_id.0,
                            },
                        )?;
                        let total = current.checked_add(contribution).ok_or(
                            TassadarAlmEvaluationError::Overflow {
                                step: step_index,
                                gate: gate_index as u32,
                            },
                        )?;
                        accumulators.insert(channel_id.0, total);
                        total
                    }
                };
                values.push(produced);
            }
            for (channel, key, value) in pending_writes {
                let cells = keyed
                    .get_mut(&channel)
                    .ok_or(TassadarAlmGraphError::UnknownChannel { gate: 0, channel })?;
                cells.insert(key, value);
                keyed_write_count += 1;
            }
            step_outputs.push(
                graph
                    .outputs
                    .iter()
                    .map(|output| values[output.0 as usize])
                    .collect(),
            );
        }
        let graph_digest = graph.stable_digest();
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_alm_trace|");
        hasher.update(graph_digest.as_bytes());
        for row in &step_outputs {
            hasher.update(b"|row|");
            for value in row {
                hasher.update(value.to_le_bytes());
            }
        }
        let trace_digest = hex::encode(hasher.finalize());
        Ok(TassadarAlmTrace {
            graph_digest,
            step_count: steps.len(),
            step_outputs,
            keyed_write_count,
            trace_digest,
        })
    }
}

/// Reference workload: exact running sum over one input field.
#[must_use]
pub fn tassadar_alm_running_sum_workload() -> TassadarAlmGraph {
    TassadarAlmGraph {
        schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
        graph_id: "alm.test.running_sum".to_string(),
        input_field_count: 1,
        channels: vec![TassadarAlmChannelDecl {
            channel_id: TassadarAlmChannelId(0),
            name: "total".to_string(),
            kind: TassadarAlmChannelKind::Accumulator,
        }],
        seed_writes: Vec::new(),
        gates: vec![
            TassadarAlmGate::Input { field: 0 },
            TassadarAlmGate::CumSum {
                channel_id: TassadarAlmChannelId(0),
                value: TassadarAlmValueId(0),
            },
        ],
        outputs: vec![TassadarAlmValueId(1)],
    }
}

/// The Percepta post's toy: track whether the count of verbs so far is
/// odd or even. Each step reads the previous parity from a keyed channel
/// (seeded for step zero), XORs it with the current indicator via the
/// identity `a ^ b = a + b - 2ab`, and writes the new parity back under
/// the current position key.
#[must_use]
pub fn tassadar_alm_verb_parity_workload() -> TassadarAlmGraph {
    TassadarAlmGraph {
        schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
        graph_id: "alm.test.verb_parity".to_string(),
        input_field_count: 1,
        channels: vec![
            TassadarAlmChannelDecl {
                channel_id: TassadarAlmChannelId(0),
                name: "parity".to_string(),
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
            // 0: is_verb input bit.
            TassadarAlmGate::Input { field: 0 },
            // 1: constant one.
            TassadarAlmGate::Const { value: 1 },
            // 2: position = cumsum(1) = t + 1.
            TassadarAlmGate::CumSum {
                channel_id: TassadarAlmChannelId(1),
                value: TassadarAlmValueId(1),
            },
            // 3: previous position key = position - 1 = t.
            TassadarAlmGate::Linear {
                terms: vec![(1, TassadarAlmValueId(2))],
                bias: -1,
            },
            // 4: previous parity read.
            TassadarAlmGate::ChannelRead {
                channel_id: TassadarAlmChannelId(0),
                query: TassadarAlmValueId(3),
            },
            // 5: product = is_verb * relu(prev_parity); both are 0/1 bits.
            TassadarAlmGate::ReGlu {
                value: TassadarAlmValueId(0),
                gate: TassadarAlmValueId(4),
            },
            // 6: xor = is_verb + prev_parity - 2 * product.
            TassadarAlmGate::Linear {
                terms: vec![
                    (1, TassadarAlmValueId(0)),
                    (1, TassadarAlmValueId(4)),
                    (-2, TassadarAlmValueId(5)),
                ],
                bias: 0,
            },
            // 7: write parity under the current position key.
            TassadarAlmGate::ChannelWrite {
                channel_id: TassadarAlmChannelId(0),
                key: TassadarAlmValueId(2),
                value: TassadarAlmValueId(6),
            },
        ],
        outputs: vec![TassadarAlmValueId(6)],
    }
}

/// A bounded three-instruction stack micro: PUSH 3, PUSH 5, ADD, OUT.
/// Opcodes: 0 = PUSH(operand), 1 = ADD, 2 = OUT. Stack values live in a
/// keyed channel keyed by depth; depth is an accumulator over per-opcode
/// deltas; writes are masked through indicator gates built from the ReLU
/// step identity `1[x >= c] = relu(x - c + 1) - relu(x - c)`.
#[must_use]
pub fn tassadar_alm_stack_micro_workload() -> TassadarAlmGraph {
    // 0: opcode, 1: operand, 2: constant one for relu gating.
    let mut gates: Vec<TassadarAlmGate> = vec![
        TassadarAlmGate::Input { field: 0 },
        TassadarAlmGate::Input { field: 1 },
        TassadarAlmGate::Const { value: 1 },
    ];
    // Step indicators: 1[op >= c] = relu(op - c + 1) - relu(op - c).
    // 3: op - 0 + 1, 4: relu of it, 5: op - 0, 6: relu, 7: ge0 = 4 - 6.
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(0))],
        bias: 1,
    });
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(2),
        gate: TassadarAlmValueId(3),
    });
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(0))],
        bias: 0,
    });
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(2),
        gate: TassadarAlmValueId(5),
    });
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(4)), (-1, TassadarAlmValueId(6))],
        bias: 0,
    });
    // 8..=12: ge1 indicator.
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(0))],
        bias: 0,
    });
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(2),
        gate: TassadarAlmValueId(8),
    });
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(0))],
        bias: -1,
    });
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(2),
        gate: TassadarAlmValueId(10),
    });
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(9)), (-1, TassadarAlmValueId(11))],
        bias: 0,
    });
    // 13..=17: ge2 indicator.
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(0))],
        bias: -1,
    });
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(2),
        gate: TassadarAlmValueId(13),
    });
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(0))],
        bias: -2,
    });
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(2),
        gate: TassadarAlmValueId(15),
    });
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(14)), (-1, TassadarAlmValueId(16))],
        bias: 0,
    });
    // 18: is_push = ge0 - ge1; 19: is_add = ge1 - ge2; 20: is_out = ge2.
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(7)), (-1, TassadarAlmValueId(12))],
        bias: 0,
    });
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(12)), (-1, TassadarAlmValueId(17))],
        bias: 0,
    });
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(17))],
        bias: 0,
    });
    // 21: delta = is_push - is_add.
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(18)), (-1, TassadarAlmValueId(19))],
        bias: 0,
    });
    // 22: depth = cumsum(delta); after PUSH depth is the new slot index.
    gates.push(TassadarAlmGate::CumSum {
        channel_id: TassadarAlmChannelId(1),
        value: TassadarAlmValueId(21),
    });
    // 23: top key before this step for ADD = depth + 1 (two operands at
    // depth+1 and depth); for OUT the top is at depth.
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(22))],
        bias: 1,
    });
    // 24: addend_a = is_add * read(depth + 1).
    // Reads must always resolve, so the stack channel is seeded for the
    // keys this bounded program can touch when the opcode masks them out.
    gates.push(TassadarAlmGate::ChannelRead {
        channel_id: TassadarAlmChannelId(0),
        query: TassadarAlmValueId(23),
    });
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(24),
        gate: TassadarAlmValueId(19),
    });
    // 26: operand_b = read(depth).
    gates.push(TassadarAlmGate::ChannelRead {
        channel_id: TassadarAlmChannelId(0),
        query: TassadarAlmValueId(22),
    });
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(26),
        gate: TassadarAlmValueId(19),
    });
    // 28: sum = masked_a + masked_b.
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(1, TassadarAlmValueId(25)), (1, TassadarAlmValueId(27))],
        bias: 0,
    });
    // 29: pushed = is_push * operand.
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(1),
        gate: TassadarAlmValueId(18),
    });
    // 30: keep = (1 - is_push - is_add) * read(depth) — preserves the top
    // for OUT steps so the unconditional write is value-neutral.
    gates.push(TassadarAlmGate::Linear {
        terms: vec![(-1, TassadarAlmValueId(18)), (-1, TassadarAlmValueId(19))],
        bias: 1,
    });
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(26),
        gate: TassadarAlmValueId(30),
    });
    // 32: written = pushed + sum + keep.
    gates.push(TassadarAlmGate::Linear {
        terms: vec![
            (1, TassadarAlmValueId(29)),
            (1, TassadarAlmValueId(28)),
            (1, TassadarAlmValueId(31)),
        ],
        bias: 0,
    });
    // 33: write at the post-step depth key.
    gates.push(TassadarAlmGate::ChannelWrite {
        channel_id: TassadarAlmChannelId(0),
        key: TassadarAlmValueId(22),
        value: TassadarAlmValueId(32),
    });
    // 34: out = is_out * read(depth).
    gates.push(TassadarAlmGate::ReGlu {
        value: TassadarAlmValueId(26),
        gate: TassadarAlmValueId(20),
    });
    TassadarAlmGraph {
        schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
        graph_id: "alm.test.stack_micro".to_string(),
        input_field_count: 2,
        channels: vec![
            TassadarAlmChannelDecl {
                channel_id: TassadarAlmChannelId(0),
                name: "stack".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            },
            TassadarAlmChannelDecl {
                channel_id: TassadarAlmChannelId(1),
                name: "depth".to_string(),
                kind: TassadarAlmChannelKind::Accumulator,
            },
        ],
        // Bounded program touches keys 0..=3 before they are written.
        seed_writes: (0..=3)
            .map(|key| TassadarAlmSeedWrite {
                channel_id: TassadarAlmChannelId(0),
                key,
                value: 0,
            })
            .collect(),
        gates,
        outputs: vec![TassadarAlmValueId(34), TassadarAlmValueId(22)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(id: u32) -> TassadarAlmValueId {
        TassadarAlmValueId(id)
    }

    fn channel(id: u16) -> TassadarAlmChannelId {
        TassadarAlmChannelId(id)
    }

    #[test]
    fn running_sum_matches_exact_prefix_sums() {
        let graph = tassadar_alm_running_sum_workload();
        let steps = vec![vec![3], vec![5], vec![-2], vec![10]];
        let trace = TassadarAlmEvaluator::evaluate(&graph, &steps).expect("evaluates");
        assert_eq!(
            trace.step_outputs,
            vec![vec![3], vec![8], vec![6], vec![16]]
        );
        assert_eq!(trace.step_count, 4);
        assert_eq!(trace.keyed_write_count, 0);
    }

    #[test]
    fn trace_digest_is_deterministic_and_input_sensitive() {
        let graph = tassadar_alm_running_sum_workload();
        let a = TassadarAlmEvaluator::evaluate(&graph, &[vec![1], vec![2]]).expect("evaluates");
        let b = TassadarAlmEvaluator::evaluate(&graph, &[vec![1], vec![2]]).expect("evaluates");
        let c = TassadarAlmEvaluator::evaluate(&graph, &[vec![1], vec![3]]).expect("evaluates");
        assert_eq!(a.trace_digest, b.trace_digest);
        assert_ne!(a.trace_digest, c.trace_digest);
    }

    #[test]
    fn verb_parity_toy_tracks_running_parity() {
        let graph = tassadar_alm_verb_parity_workload();
        // "the cat runs and the dog jumps over fences quickly"
        let is_verb = [0_i64, 0, 1, 0, 0, 0, 1, 0, 0, 0];
        let steps: Vec<Vec<i64>> = is_verb.iter().map(|bit| vec![*bit]).collect();
        let trace = TassadarAlmEvaluator::evaluate(&graph, &steps).expect("evaluates");
        let parities: Vec<i64> = trace.step_outputs.iter().map(|row| row[0]).collect();
        assert_eq!(parities, vec![0, 0, 1, 1, 1, 1, 0, 0, 0, 0]);
        // Seed write plus one parity write per step.
        assert_eq!(trace.keyed_write_count, 1 + steps.len());
    }

    #[test]
    fn stack_micro_executes_push_push_add_out() {
        let graph = tassadar_alm_stack_micro_workload();
        // PUSH 3, PUSH 5, ADD, OUT.
        let steps = vec![vec![0, 3], vec![0, 5], vec![1, 0], vec![2, 0]];
        let trace = TassadarAlmEvaluator::evaluate(&graph, &steps).expect("evaluates");
        let outs: Vec<i64> = trace.step_outputs.iter().map(|row| row[0]).collect();
        let depths: Vec<i64> = trace.step_outputs.iter().map(|row| row[1]).collect();
        assert_eq!(depths, vec![1, 2, 1, 1]);
        assert_eq!(outs, vec![0, 0, 0, 8]);
    }

    #[test]
    fn forward_reference_is_rejected() {
        let mut graph = tassadar_alm_running_sum_workload();
        graph.gates[0] = TassadarAlmGate::Linear {
            terms: vec![(1, value(1))],
            bias: 0,
        };
        assert_eq!(
            graph.validate(),
            Err(TassadarAlmGraphError::ForwardReference { gate: 0, value: 1 })
        );
    }

    #[test]
    fn channel_kind_mismatch_is_rejected() {
        let mut graph = tassadar_alm_running_sum_workload();
        graph.gates[1] = TassadarAlmGate::ChannelRead {
            channel_id: channel(0),
            query: value(0),
        };
        assert_eq!(
            graph.validate(),
            Err(TassadarAlmGraphError::ChannelKindMismatch {
                gate: 1,
                channel: 0
            })
        );
    }

    #[test]
    fn missing_key_read_is_a_typed_refusal() {
        let graph = TassadarAlmGraph {
            schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
            graph_id: "alm.test.missing_key".to_string(),
            input_field_count: 1,
            channels: vec![TassadarAlmChannelDecl {
                channel_id: channel(0),
                name: "memory".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            }],
            seed_writes: Vec::new(),
            gates: vec![
                TassadarAlmGate::Input { field: 0 },
                TassadarAlmGate::ChannelRead {
                    channel_id: channel(0),
                    query: value(0),
                },
            ],
            outputs: vec![value(1)],
        };
        assert_eq!(
            TassadarAlmEvaluator::evaluate(&graph, &[vec![7]]),
            Err(TassadarAlmEvaluationError::MissingKey {
                step: 0,
                gate: 1,
                channel: 0,
                key: 7
            })
        );
    }

    #[test]
    fn same_step_writes_are_invisible_until_the_next_step() {
        // Write key 1 at every step, read key 1 every step (seeded with 0):
        // each read must return the PREVIOUS step's write, not the current.
        let graph = TassadarAlmGraph {
            schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
            graph_id: "alm.test.visibility".to_string(),
            input_field_count: 1,
            channels: vec![TassadarAlmChannelDecl {
                channel_id: channel(0),
                name: "cell".to_string(),
                kind: TassadarAlmChannelKind::Keyed,
            }],
            seed_writes: vec![TassadarAlmSeedWrite {
                channel_id: channel(0),
                key: 1,
                value: 0,
            }],
            gates: vec![
                TassadarAlmGate::Input { field: 0 },
                TassadarAlmGate::Const { value: 1 },
                TassadarAlmGate::ChannelRead {
                    channel_id: channel(0),
                    query: value(1),
                },
                TassadarAlmGate::ChannelWrite {
                    channel_id: channel(0),
                    key: value(1),
                    value: value(0),
                },
            ],
            outputs: vec![value(2)],
        };
        let trace = TassadarAlmEvaluator::evaluate(&graph, &[vec![10], vec![20], vec![30]])
            .expect("evaluates");
        let reads: Vec<i64> = trace.step_outputs.iter().map(|row| row[0]).collect();
        assert_eq!(reads, vec![0, 10, 20]);
    }

    #[test]
    fn overflow_is_a_typed_refusal() {
        let graph = TassadarAlmGraph {
            schema_version: TASSADAR_ALM_GRAPH_SCHEMA_VERSION,
            graph_id: "alm.test.overflow".to_string(),
            input_field_count: 1,
            channels: Vec::new(),
            seed_writes: Vec::new(),
            gates: vec![
                TassadarAlmGate::Input { field: 0 },
                TassadarAlmGate::Linear {
                    terms: vec![(i64::MAX, value(0))],
                    bias: i64::MAX,
                },
            ],
            outputs: vec![value(1)],
        };
        assert_eq!(
            TassadarAlmEvaluator::evaluate(&graph, &[vec![2]]),
            Err(TassadarAlmEvaluationError::Overflow { step: 0, gate: 1 })
        );
    }
}
