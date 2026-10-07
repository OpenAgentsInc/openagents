use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::tassadar_alm_numeric::{
    build_tassadar_alm_numeric_program_corpus_fixture_v1, tassadar_alm_numeric_execute,
    TassadarAlmNumericAttentionRow, TassadarAlmNumericExecutionError, TassadarAlmNumericFfnRow,
    TassadarAlmNumericModel, TassadarAlmNumericProgramCorpusError,
    TassadarAlmNumericProgramFixture, TassadarAlmNumericTrace, TassadarAlmNumericWiringRow,
    TassadarAlmNumericWriteRow,
};
use crate::tassadar_alm_wasm_interpreter::tassadar_alm_wasm_collect;

/// Stable schema version for dense ALM weight-module artifacts.
pub const TASSADAR_ALM_DENSE_WEIGHT_MODULE_SCHEMA_VERSION: u16 = 1;
/// Stable module kind embedded in loadable dense artifacts.
pub const TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND: &str = "tassadar_alm_dense_weight_module.v1";
/// Stable generator identity for the first committed dense fixture.
pub const TASSADAR_ALM_DENSE_WEIGHT_MODULE_GENERATED_BY: &str =
    "psionic crates/psionic-compiler tassadar_alm_dense_module_v1";
/// Claim boundary for the dense materialization lane.
pub const TASSADAR_ALM_DENSE_WEIGHT_MODULE_CLAIM_BOUNDARY: &str = "the dense weight module is a \
     loadable full-width matrix representation of one digest-pinned ALM numeric model - it carries \
     W_Q/W_K/W_V-style hard-max attention projections, FFN matrices, residual wiring matrices, \
     write rows, source-model digest, and replay fixtures; it is not a trained transformer, does \
     not claim softmax semantics, and is valid only for deterministic replay inside the numeric \
     exactness window";

/// One dense residual wiring block for a single phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarAlmDenseWiringBlock {
    /// Phase index shared by every row in this block.
    pub phase: u32,
    /// Destination slot for each dense row.
    pub out_slots: Vec<u32>,
    /// Full-width residual weights, row-major: rows x d_model.
    pub w_residual: Vec<Vec<f64>>,
    /// Constant bias for each dense row.
    pub bias: Vec<f64>,
    /// Optional input field landing on each dense row.
    pub input_fields: Vec<Option<u16>>,
}

/// Descriptor for a dense hard-max attention head.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TassadarAlmDenseAttentionHead {
    /// Hard-max keyed memory read. W_Q selects the query slot; channel
    /// memory supplies keys and values.
    KeyedRead {
        /// Source channel id.
        channel: u16,
        /// Residual query slot.
        query_slot: u32,
        /// Destination residual slot.
        out_slot: u32,
    },
    /// Running accumulator. W_V selects the contribution slot.
    CumSum {
        /// Accumulator channel id.
        channel: u16,
        /// Residual contribution slot.
        value_slot: u32,
        /// Destination residual slot.
        out_slot: u32,
    },
}

/// Dense attention block for one phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarAlmDenseAttentionBlock {
    /// Phase index shared by every head in this block.
    pub phase: u32,
    /// Head descriptors carrying ALM channel semantics.
    pub heads: Vec<TassadarAlmDenseAttentionHead>,
    /// Query projection, row-major: heads x d_model.
    pub w_q: Vec<Vec<f64>>,
    /// Key projection, row-major: heads x d_model. ALM keyed reads use
    /// external channel memory, so rows are zero and the descriptor names
    /// the channel.
    pub w_k: Vec<Vec<f64>>,
    /// Value projection, row-major: heads x d_model. CumSum heads select a
    /// residual value slot; keyed reads use external channel memory.
    pub w_v: Vec<Vec<f64>>,
    /// Output projection, row-major: d_model x heads.
    pub w_o: Vec<Vec<f64>>,
}

/// Dense gated FFN block for one phase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarAlmDenseFfnBlock {
    /// Phase index shared by every neuron in this block.
    pub phase: u32,
    /// Multiplicand projection, row-major: neurons x d_model.
    pub w_value: Vec<Vec<f64>>,
    /// Gate projection, row-major: neurons x d_model.
    pub w_gate: Vec<Vec<f64>>,
    /// Output projection, row-major: d_model x neurons.
    pub w_out: Vec<Vec<f64>>,
}

/// One dense, loadable ALM weight module derived from a numeric model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarAlmDenseWeightModule {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable module kind.
    pub module_kind: String,
    /// Stable module id.
    pub module_id: String,
    /// Source numeric model id.
    pub source_model_id: String,
    /// Source numeric model digest.
    pub source_model_digest: String,
    /// Source graph digest.
    pub graph_digest: String,
    /// Source compiled-bundle digest.
    pub bundle_digest: String,
    /// Per-step input field count.
    pub input_field_count: u16,
    /// Dense residual width.
    pub d_model: u32,
    /// Scheduled layer count inherited from the source model.
    pub layer_count: u32,
    /// Seed writes applied before step zero.
    pub seed_writes: Vec<(u16, f64, f64)>,
    /// Dense residual wiring blocks.
    pub wiring_blocks: Vec<TassadarAlmDenseWiringBlock>,
    /// Dense hard-max attention blocks.
    pub attention_blocks: Vec<TassadarAlmDenseAttentionBlock>,
    /// Dense gated FFN blocks.
    pub ffn_blocks: Vec<TassadarAlmDenseFfnBlock>,
    /// End-of-step keyed write emissions.
    pub write_rows: Vec<TassadarAlmNumericWriteRow>,
    /// Residual slots exposed as outputs.
    pub output_slots: Vec<u32>,
    /// Explicit public claim boundary for the loadable module.
    pub claim_boundary: String,
}

impl TassadarAlmDenseWeightModule {
    /// Returns a stable digest over the module payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"tassadar_alm_dense_weight_module|", self)
    }
}

/// One digest-pinned dense program fixture for OpenAgents run artifacts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarAlmDenseProgramFixture {
    /// Stable fixture schema version.
    pub schema_version: u16,
    /// Stable fixture identifier.
    pub fixture_id: String,
    /// Generator identity.
    pub generated_by: String,
    /// Claim boundary inherited from the dense materializer.
    pub claim_boundary: String,
    /// Stable source program identifier.
    pub program_id: String,
    /// Stable source program digest.
    pub program_digest: String,
    /// Human-readable workload family label.
    pub workload_kind: String,
    /// Runtime profile id targeted by the program.
    pub profile_id: String,
    /// Source numeric model digest.
    pub numeric_model_digest: String,
    /// Dense module digest.
    pub dense_module_digest: String,
    /// Dense weight module.
    pub dense_module: TassadarAlmDenseWeightModule,
    /// Per-step input rows.
    pub steps: Vec<Vec<i64>>,
    /// Trace digest produced by executing the dense module.
    pub expected_trace_digest: String,
    /// Final numeric trace row, if the run emitted any step.
    pub expected_final_row: Option<Vec<i64>>,
    /// Outputs collected from the ALM interpreter row convention.
    pub expected_outputs: Vec<i64>,
    /// Whether the interpreter halted under the chosen step budget.
    pub halted: bool,
    /// Public-safe receipt refs proving the derivation path.
    pub compile_receipt_refs: Vec<String>,
    /// Public-safe run artifact refs for dispatch payloads.
    pub run_artifact_refs: Vec<String>,
}

/// Error returned while decoding a dense module back into the numeric model.
#[derive(Debug, Error, PartialEq)]
pub enum TassadarAlmDenseModuleError {
    /// A matrix has the wrong number of rows or columns.
    #[error("{field} has dimension {found}, expected {expected}")]
    DimensionMismatch {
        /// Matrix field name.
        field: &'static str,
        /// Found dimension.
        found: usize,
        /// Expected dimension.
        expected: usize,
    },
    /// A one-hot row or column was malformed.
    #[error("{field} is not a one-hot vector")]
    NotOneHot {
        /// Matrix field name.
        field: &'static str,
    },
}

/// Error returned by dense execution.
#[derive(Debug, Error, PartialEq)]
pub enum TassadarAlmDenseExecutionError {
    /// Dense module could not be decoded.
    #[error(transparent)]
    DenseModule(#[from] TassadarAlmDenseModuleError),
    /// Numeric executor refused the decoded model.
    #[error(transparent)]
    NumericExecution(#[from] TassadarAlmNumericExecutionError),
}

/// Failure returned while building the dense fixture.
#[derive(Debug, Error)]
pub enum TassadarAlmDenseProgramFixtureError {
    /// Numeric corpus build failed.
    #[error(transparent)]
    NumericCorpus(#[from] TassadarAlmNumericProgramCorpusError),
    /// Dense module could not decode.
    #[error(transparent)]
    DenseModule(#[from] TassadarAlmDenseModuleError),
    /// Dense execution refused.
    #[error(transparent)]
    NumericExecution(#[from] TassadarAlmNumericExecutionError),
    /// The named source program was not in the numeric corpus.
    #[error("program {program_id} was not found in the numeric corpus")]
    ProgramNotFound {
        /// Missing program id.
        program_id: String,
    },
    /// Dense execution diverged from the source fixture.
    #[error(
        "program {program_id} dense trace {dense_trace_digest} diverged from numeric trace {numeric_trace_digest}"
    )]
    TraceMismatch {
        /// Source program id.
        program_id: String,
        /// Dense trace digest.
        dense_trace_digest: String,
        /// Numeric trace digest.
        numeric_trace_digest: String,
    },
}

/// Materializes one numeric model into full-width dense blocks.
#[must_use]
pub fn materialize_tassadar_alm_dense_weight_module(
    model: &TassadarAlmNumericModel,
) -> TassadarAlmDenseWeightModule {
    let d_model = model.slot_count as usize;
    let mut wiring_by_phase: BTreeMap<u32, Vec<&TassadarAlmNumericWiringRow>> = BTreeMap::new();
    for row in &model.wiring {
        wiring_by_phase.entry(row.phase).or_default().push(row);
    }
    let wiring_blocks = wiring_by_phase
        .into_iter()
        .map(|(phase, rows)| {
            let mut out_slots = Vec::with_capacity(rows.len());
            let mut w_residual = Vec::with_capacity(rows.len());
            let mut bias = Vec::with_capacity(rows.len());
            let mut input_fields = Vec::with_capacity(rows.len());
            for row in rows {
                let mut dense_row = vec![0.0; d_model];
                for (coefficient, slot) in &row.terms {
                    dense_row[*slot as usize] += *coefficient;
                }
                out_slots.push(row.out_slot);
                w_residual.push(dense_row);
                bias.push(row.bias);
                input_fields.push(row.input_field);
            }
            TassadarAlmDenseWiringBlock {
                phase,
                out_slots,
                w_residual,
                bias,
                input_fields,
            }
        })
        .collect();

    let mut attention_by_phase: BTreeMap<u32, Vec<&TassadarAlmNumericAttentionRow>> =
        BTreeMap::new();
    for row in &model.attention {
        let phase = match row {
            TassadarAlmNumericAttentionRow::KeyedRead { phase, .. }
            | TassadarAlmNumericAttentionRow::CumSum { phase, .. } => *phase,
        };
        attention_by_phase.entry(phase).or_default().push(row);
    }
    let attention_blocks = attention_by_phase
        .into_iter()
        .map(|(phase, rows)| {
            let head_count = rows.len();
            let mut heads = Vec::with_capacity(head_count);
            let mut w_q = Vec::with_capacity(head_count);
            let mut w_k = Vec::with_capacity(head_count);
            let mut w_v = Vec::with_capacity(head_count);
            let mut w_o = vec![vec![0.0; head_count]; d_model];
            for (head_index, row) in rows.into_iter().enumerate() {
                let mut q = vec![0.0; d_model];
                let k = vec![0.0; d_model];
                let mut v = vec![0.0; d_model];
                let out_slot = match row {
                    TassadarAlmNumericAttentionRow::KeyedRead {
                        channel,
                        query_slot,
                        out_slot,
                        ..
                    } => {
                        q[*query_slot as usize] = 1.0;
                        heads.push(TassadarAlmDenseAttentionHead::KeyedRead {
                            channel: *channel,
                            query_slot: *query_slot,
                            out_slot: *out_slot,
                        });
                        *out_slot
                    }
                    TassadarAlmNumericAttentionRow::CumSum {
                        channel,
                        value_slot,
                        out_slot,
                        ..
                    } => {
                        v[*value_slot as usize] = 1.0;
                        heads.push(TassadarAlmDenseAttentionHead::CumSum {
                            channel: *channel,
                            value_slot: *value_slot,
                            out_slot: *out_slot,
                        });
                        *out_slot
                    }
                };
                w_o[out_slot as usize][head_index] = 1.0;
                w_q.push(q);
                w_k.push(k);
                w_v.push(v);
            }
            TassadarAlmDenseAttentionBlock {
                phase,
                heads,
                w_q,
                w_k,
                w_v,
                w_o,
            }
        })
        .collect();

    let mut ffn_by_phase: BTreeMap<u32, Vec<&TassadarAlmNumericFfnRow>> = BTreeMap::new();
    for row in &model.ffn {
        ffn_by_phase.entry(row.phase).or_default().push(row);
    }
    let ffn_blocks = ffn_by_phase
        .into_iter()
        .map(|(phase, rows)| {
            let neuron_count = rows.len();
            let mut w_value = Vec::with_capacity(neuron_count);
            let mut w_gate = Vec::with_capacity(neuron_count);
            let mut w_out = vec![vec![0.0; neuron_count]; d_model];
            for (neuron_index, row) in rows.into_iter().enumerate() {
                let mut value_row = vec![0.0; d_model];
                let mut gate_row = vec![0.0; d_model];
                value_row[row.value_slot as usize] = 1.0;
                gate_row[row.gate_slot as usize] = 1.0;
                w_out[row.out_slot as usize][neuron_index] = 1.0;
                w_value.push(value_row);
                w_gate.push(gate_row);
            }
            TassadarAlmDenseFfnBlock {
                phase,
                w_value,
                w_gate,
                w_out,
            }
        })
        .collect();

    TassadarAlmDenseWeightModule {
        schema_version: TASSADAR_ALM_DENSE_WEIGHT_MODULE_SCHEMA_VERSION,
        module_kind: String::from(TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND),
        module_id: format!("alm.dense.{}", model.model_id),
        source_model_id: model.model_id.clone(),
        source_model_digest: model.stable_digest(),
        graph_digest: model.graph_digest.clone(),
        bundle_digest: model.bundle_digest.clone(),
        input_field_count: model.input_field_count,
        d_model: model.slot_count,
        layer_count: model.layer_count,
        seed_writes: model.seed_writes.clone(),
        wiring_blocks,
        attention_blocks,
        ffn_blocks,
        write_rows: model.writes.clone(),
        output_slots: model.output_slots.clone(),
        claim_boundary: String::from(TASSADAR_ALM_DENSE_WEIGHT_MODULE_CLAIM_BOUNDARY),
    }
}

/// Decodes a dense module back into the numeric model it represents.
pub fn tassadar_alm_dense_weight_module_to_numeric(
    module: &TassadarAlmDenseWeightModule,
) -> Result<TassadarAlmNumericModel, TassadarAlmDenseModuleError> {
    let d_model = module.d_model as usize;
    let mut wiring = Vec::new();
    for block in &module.wiring_blocks {
        validate_len(
            "wiring.outSlots",
            block.out_slots.len(),
            block.w_residual.len(),
        )?;
        validate_len("wiring.bias", block.bias.len(), block.w_residual.len())?;
        validate_len(
            "wiring.inputFields",
            block.input_fields.len(),
            block.w_residual.len(),
        )?;
        for (row_index, dense_row) in block.w_residual.iter().enumerate() {
            validate_len("wiring.wResidual.row", dense_row.len(), d_model)?;
            let terms = dense_row
                .iter()
                .enumerate()
                .filter_map(|(slot, coefficient)| {
                    (*coefficient != 0.0).then_some((*coefficient, slot as u32))
                })
                .collect();
            wiring.push(TassadarAlmNumericWiringRow {
                out_slot: block.out_slots[row_index],
                bias: block.bias[row_index],
                terms,
                input_field: block.input_fields[row_index],
                phase: block.phase,
            });
        }
    }

    let mut attention = Vec::new();
    for block in &module.attention_blocks {
        let head_count = block.heads.len();
        validate_len("attention.wQ", block.w_q.len(), head_count)?;
        validate_len("attention.wK", block.w_k.len(), head_count)?;
        validate_len("attention.wV", block.w_v.len(), head_count)?;
        validate_len("attention.wO", block.w_o.len(), d_model)?;
        for row in &block.w_o {
            validate_len("attention.wO.row", row.len(), head_count)?;
        }
        for (head_index, head) in block.heads.iter().enumerate() {
            validate_len("attention.wQ.row", block.w_q[head_index].len(), d_model)?;
            validate_len("attention.wK.row", block.w_k[head_index].len(), d_model)?;
            validate_len("attention.wV.row", block.w_v[head_index].len(), d_model)?;
            let out_slot = one_hot_column_slot(&block.w_o, head_index, "attention.wO")?;
            match head {
                TassadarAlmDenseAttentionHead::KeyedRead {
                    channel,
                    query_slot,
                    out_slot: descriptor_out,
                } => {
                    let matrix_query = one_hot_row_slot(&block.w_q[head_index], "attention.wQ")?;
                    validate_zero_row(&block.w_v[head_index], "attention.wV.keyedRead")?;
                    if matrix_query != *query_slot || out_slot != *descriptor_out {
                        return Err(TassadarAlmDenseModuleError::NotOneHot {
                            field: "attention.keyedRead.descriptor",
                        });
                    }
                    attention.push(TassadarAlmNumericAttentionRow::KeyedRead {
                        channel: *channel,
                        query_slot: *query_slot,
                        out_slot,
                        phase: block.phase,
                    });
                }
                TassadarAlmDenseAttentionHead::CumSum {
                    channel,
                    value_slot,
                    out_slot: descriptor_out,
                } => {
                    validate_zero_row(&block.w_q[head_index], "attention.wQ.cumSum")?;
                    let matrix_value = one_hot_row_slot(&block.w_v[head_index], "attention.wV")?;
                    if matrix_value != *value_slot || out_slot != *descriptor_out {
                        return Err(TassadarAlmDenseModuleError::NotOneHot {
                            field: "attention.cumSum.descriptor",
                        });
                    }
                    attention.push(TassadarAlmNumericAttentionRow::CumSum {
                        channel: *channel,
                        value_slot: *value_slot,
                        out_slot,
                        phase: block.phase,
                    });
                }
            }
        }
    }

    let mut ffn = Vec::new();
    for block in &module.ffn_blocks {
        let neuron_count = block.w_value.len();
        validate_len("ffn.wGate", block.w_gate.len(), neuron_count)?;
        validate_len("ffn.wOut", block.w_out.len(), d_model)?;
        for row in &block.w_out {
            validate_len("ffn.wOut.row", row.len(), neuron_count)?;
        }
        for neuron_index in 0..neuron_count {
            validate_len("ffn.wValue.row", block.w_value[neuron_index].len(), d_model)?;
            validate_len("ffn.wGate.row", block.w_gate[neuron_index].len(), d_model)?;
            ffn.push(TassadarAlmNumericFfnRow {
                value_slot: one_hot_row_slot(&block.w_value[neuron_index], "ffn.wValue")?,
                gate_slot: one_hot_row_slot(&block.w_gate[neuron_index], "ffn.wGate")?,
                out_slot: one_hot_column_slot(&block.w_out, neuron_index, "ffn.wOut")?,
                phase: block.phase,
            });
        }
    }

    Ok(TassadarAlmNumericModel {
        schema_version: 1,
        model_id: module.source_model_id.clone(),
        graph_digest: module.graph_digest.clone(),
        bundle_digest: module.bundle_digest.clone(),
        input_field_count: module.input_field_count,
        slot_count: module.d_model,
        layer_count: module.layer_count,
        seed_writes: module.seed_writes.clone(),
        wiring,
        attention,
        ffn,
        writes: module.write_rows.clone(),
        output_slots: module.output_slots.clone(),
    })
}

/// Executes one dense module by decoding it to the equivalent numeric model.
pub fn tassadar_alm_dense_weight_module_execute(
    module: &TassadarAlmDenseWeightModule,
    steps: &[Vec<i64>],
) -> Result<TassadarAlmNumericTrace, TassadarAlmDenseExecutionError> {
    let model = tassadar_alm_dense_weight_module_to_numeric(module)?;
    Ok(tassadar_alm_numeric_execute(&model, steps)?)
}

/// Builds the first dense fixture wired into OpenAgents run artifacts.
pub fn build_tassadar_alm_dense_program_fixture_v1(
) -> Result<TassadarAlmDenseProgramFixture, TassadarAlmDenseProgramFixtureError> {
    build_tassadar_alm_dense_program_fixture_for("tassadar_corpus.loop_sum_v1")
}

/// Builds a dense fixture for a named source program in the numeric corpus.
pub fn build_tassadar_alm_dense_program_fixture_for(
    program_id: &str,
) -> Result<TassadarAlmDenseProgramFixture, TassadarAlmDenseProgramFixtureError> {
    let corpus = build_tassadar_alm_numeric_program_corpus_fixture_v1()?;
    let fixture = corpus
        .fixtures
        .into_iter()
        .find(|candidate| candidate.program_id == program_id)
        .ok_or_else(|| TassadarAlmDenseProgramFixtureError::ProgramNotFound {
            program_id: program_id.to_string(),
        })?;
    build_dense_fixture_from_numeric(fixture)
}

fn build_dense_fixture_from_numeric(
    fixture: TassadarAlmNumericProgramFixture,
) -> Result<TassadarAlmDenseProgramFixture, TassadarAlmDenseProgramFixtureError> {
    let dense_module = materialize_tassadar_alm_dense_weight_module(&fixture.model);
    let decoded = tassadar_alm_dense_weight_module_to_numeric(&dense_module)?;
    let trace = tassadar_alm_numeric_execute(&decoded, &fixture.steps)?;
    if trace.trace_digest != fixture.expected_trace_digest {
        return Err(TassadarAlmDenseProgramFixtureError::TraceMismatch {
            program_id: fixture.program_id,
            dense_trace_digest: trace.trace_digest,
            numeric_trace_digest: fixture.expected_trace_digest,
        });
    }
    let (outputs, halted) = tassadar_alm_wasm_collect(&trace.step_outputs);
    let dense_module_digest = dense_module.stable_digest();
    let mut compile_receipt_refs = fixture.compile_receipt_refs;
    compile_receipt_refs.push(format!(
        "receipt.psionic.tassadar_dense_module.{}",
        &dense_module_digest[..16]
    ));
    Ok(TassadarAlmDenseProgramFixture {
        schema_version: TASSADAR_ALM_DENSE_WEIGHT_MODULE_SCHEMA_VERSION,
        fixture_id: format!("{}.dense_weight_module.v1", fixture.program_id),
        generated_by: String::from(TASSADAR_ALM_DENSE_WEIGHT_MODULE_GENERATED_BY),
        claim_boundary: String::from(TASSADAR_ALM_DENSE_WEIGHT_MODULE_CLAIM_BOUNDARY),
        program_id: fixture.program_id,
        program_digest: fixture.program_digest,
        workload_kind: fixture.workload_kind,
        profile_id: fixture.profile_id,
        numeric_model_digest: fixture.expected_model_digest,
        dense_module_digest: dense_module_digest.clone(),
        dense_module,
        steps: fixture.steps,
        expected_trace_digest: trace.trace_digest,
        expected_final_row: trace.step_outputs.last().cloned(),
        expected_outputs: outputs,
        halted,
        compile_receipt_refs,
        run_artifact_refs: vec![
            format!(
                "artifact.public.tassadar_dense_weight_module.{}",
                &dense_module_digest[..16]
            ),
            format!(
                "artifact.public.tassadar_dense_trace.{}",
                &fixture.expected_trace_digest[..16]
            ),
        ],
    })
}

fn validate_len(
    field: &'static str,
    found: usize,
    expected: usize,
) -> Result<(), TassadarAlmDenseModuleError> {
    if found == expected {
        Ok(())
    } else {
        Err(TassadarAlmDenseModuleError::DimensionMismatch {
            field,
            found,
            expected,
        })
    }
}

fn validate_zero_row(row: &[f64], field: &'static str) -> Result<(), TassadarAlmDenseModuleError> {
    if row.iter().all(|value| *value == 0.0) {
        Ok(())
    } else {
        Err(TassadarAlmDenseModuleError::NotOneHot { field })
    }
}

fn one_hot_row_slot(row: &[f64], field: &'static str) -> Result<u32, TassadarAlmDenseModuleError> {
    let mut slot = None;
    for (index, value) in row.iter().enumerate() {
        if *value == 1.0 && slot.is_none() {
            slot = Some(index as u32);
        } else if *value != 0.0 {
            return Err(TassadarAlmDenseModuleError::NotOneHot { field });
        }
    }
    slot.ok_or(TassadarAlmDenseModuleError::NotOneHot { field })
}

fn one_hot_column_slot(
    matrix: &[Vec<f64>],
    column: usize,
    field: &'static str,
) -> Result<u32, TassadarAlmDenseModuleError> {
    let mut slot = None;
    for (row_index, row) in matrix.iter().enumerate() {
        let value = row[column];
        if value == 1.0 && slot.is_none() {
            slot = Some(row_index as u32);
        } else if value != 0.0 {
            return Err(TassadarAlmDenseModuleError::NotOneHot { field });
        }
    }
    slot.ok_or(TassadarAlmDenseModuleError::NotOneHot { field })
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn dense_module_roundtrips_to_the_source_numeric_digest_and_trace() {
        let fixture = build_tassadar_alm_dense_program_fixture_v1().expect("dense fixture builds");
        let decoded = tassadar_alm_dense_weight_module_to_numeric(&fixture.dense_module)
            .expect("dense decodes");
        assert_eq!(
            fixture.dense_module.source_model_digest,
            fixture.numeric_model_digest
        );
        let trace = tassadar_alm_dense_weight_module_execute(&fixture.dense_module, &fixture.steps)
            .expect("dense executes");
        assert_eq!(trace.trace_digest, fixture.expected_trace_digest);
        let decoded_trace =
            tassadar_alm_numeric_execute(&decoded, &fixture.steps).expect("decoded executes");
        assert_eq!(decoded_trace.trace_digest, fixture.expected_trace_digest);
        assert_eq!(fixture.expected_outputs, vec![15]);
        assert!(fixture.halted);
    }

    #[test]
    fn dense_module_carries_loadable_wq_wk_wv_and_ffn_blocks() {
        let fixture = build_tassadar_alm_dense_program_fixture_v1().expect("dense fixture builds");
        let module = &fixture.dense_module;
        let d_model = module.d_model as usize;
        assert_eq!(module.module_kind, TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND);
        assert_eq!(module.schema_version, 1);
        assert!(!module.attention_blocks.is_empty());
        assert!(!module.ffn_blocks.is_empty());
        assert!(!module.wiring_blocks.is_empty());
        for block in &module.attention_blocks {
            assert_eq!(block.w_q.len(), block.heads.len());
            assert_eq!(block.w_k.len(), block.heads.len());
            assert_eq!(block.w_v.len(), block.heads.len());
            assert_eq!(block.w_o.len(), d_model);
            assert!(block.w_q.iter().all(|row| row.len() == d_model));
            assert!(block.w_k.iter().all(|row| row.len() == d_model));
            assert!(block.w_v.iter().all(|row| row.len() == d_model));
            assert!(block.w_o.iter().all(|row| row.len() == block.heads.len()));
        }
        for block in &module.ffn_blocks {
            assert_eq!(block.w_value.len(), block.w_gate.len());
            assert_eq!(block.w_out.len(), d_model);
            assert!(block.w_value.iter().all(|row| row.len() == d_model));
            assert!(block.w_gate.iter().all(|row| row.len() == d_model));
            assert!(block
                .w_out
                .iter()
                .all(|row| row.len() == block.w_value.len()));
        }
    }

    #[test]
    fn dense_fixture_is_deterministic_and_digest_pinned() {
        let a = build_tassadar_alm_dense_program_fixture_v1().expect("fixture a");
        let b = build_tassadar_alm_dense_program_fixture_v1().expect("fixture b");
        assert_eq!(a, b);
        assert_eq!(a.dense_module_digest, a.dense_module.stable_digest());
        assert!(a
            .compile_receipt_refs
            .iter()
            .any(|receipt| receipt.starts_with("receipt.psionic.tassadar_dense_module.")));
        assert!(a
            .run_artifact_refs
            .iter()
            .any(|receipt| receipt.starts_with("artifact.public.tassadar_dense_weight_module.")));
    }

    #[test]
    fn malformed_dense_module_refuses_before_execution() {
        let mut fixture =
            build_tassadar_alm_dense_program_fixture_v1().expect("dense fixture builds");
        fixture.dense_module.ffn_blocks[0].w_value[0].push(0.0);
        let error = tassadar_alm_dense_weight_module_to_numeric(&fixture.dense_module)
            .expect_err("malformed dimensions refuse");
        assert!(matches!(
            error,
            TassadarAlmDenseModuleError::DimensionMismatch { .. }
        ));
    }
}

#[cfg(test)]
mod fixture_dump {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    #[ignore]
    fn dump_dense_weight_module_fixture() {
        let fixture = build_tassadar_alm_dense_program_fixture_v1().expect("dense fixture builds");
        std::fs::write(
            "/tmp/tassadar-dense-weight-module-v1.json",
            serde_json::to_vec_pretty(&fixture).expect("encodes"),
        )
        .expect("writes");
        eprintln!("dense_module_digest={}", fixture.dense_module_digest);
        eprintln!("trace_digest={}", fixture.expected_trace_digest);
    }
}
