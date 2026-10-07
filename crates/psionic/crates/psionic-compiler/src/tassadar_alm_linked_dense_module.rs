use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_ir::{
    TassadarComputationalModuleCapabilitySummary, TassadarComputationalModuleExport,
    TassadarComputationalModuleImport, TassadarComputationalModuleManifest,
    TassadarComputationalModuleStateField, TassadarModuleImportClass, TassadarModuleStateFieldKind,
    TassadarModuleTrustPosture,
};

use crate::tassadar_alm_dense_module::{
    build_tassadar_alm_dense_program_fixture_for, materialize_tassadar_alm_dense_weight_module,
    tassadar_alm_dense_weight_module_execute, tassadar_alm_dense_weight_module_to_numeric,
    TassadarAlmDenseExecutionError, TassadarAlmDenseProgramFixture, TassadarAlmDenseWeightModule,
};
use crate::tassadar_alm_numeric::{
    TassadarAlmNumericAttentionRow, TassadarAlmNumericExecutionError, TassadarAlmNumericFfnRow,
    TassadarAlmNumericModel, TassadarAlmNumericWiringRow, TassadarAlmNumericWriteRow,
};
use crate::tassadar_module_linker::{
    link_tassadar_module_dependency_graph, TassadarModuleLinkError, TassadarModuleLinkRequest,
    TassadarModuleLinkResolution,
};

/// Stable schema version for linked dense ALM module artifacts.
pub const TASSADAR_ALM_LINKED_DENSE_MODULE_SCHEMA_VERSION: u16 = 1;
/// Stable module kind embedded in linked dense artifacts.
pub const TASSADAR_ALM_LINKED_DENSE_MODULE_KIND: &str = "tassadar_alm_linked_dense_module.v1";
/// Stable generator identity for the first linked dense fixture.
pub const TASSADAR_ALM_LINKED_DENSE_MODULE_GENERATED_BY: &str =
    "psionic crates/psionic-compiler tassadar_alm_linked_dense_module_v1";
/// Claim class used by the module linker for dense ALM banks.
pub const TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS: &str =
    "compiled dense ALM module composition / exact replay gate";
/// Claim boundary for the linked dense materialization lane.
pub const TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_BOUNDARY: &str = "the linked dense module is a \
     bounded block-separated composition of two digest-pinned dense ALM weight modules. It links \
     their typed manifests through the module linker, offsets residual slots and keyed channels to \
     avoid collision, materializes one composed dense module, and conformance-replays projected \
     output rows against each source dense module. It is not a trained transformer, does not claim \
     softmax semantics, does not settle purchases, and is valid only for deterministic replay \
     inside the numeric exactness window";

/// One source bank embedded in a linked dense module.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarAlmLinkedDenseBank {
    /// Stable bank identifier inside the linked module.
    pub bank_id: String,
    /// Stable computational module ref used by the linker.
    pub module_ref: String,
    /// Source program id.
    pub program_id: String,
    /// Human-readable workload family label.
    pub workload_kind: String,
    /// Runtime profile id targeted by the source program.
    pub profile_id: String,
    /// Source dense module digest.
    pub dense_module_digest: String,
    /// Source numeric model digest.
    pub numeric_model_digest: String,
    /// Source dense replay trace digest.
    pub expected_trace_digest: String,
    /// Slot offset applied inside the composed model.
    pub slot_offset: u32,
    /// Channel offset applied inside the composed model.
    pub channel_offset: u16,
    /// Inclusive start column for this bank's projected rows in the composed trace.
    pub projected_output_start: usize,
    /// Exclusive end column for this bank's projected rows in the composed trace.
    pub projected_output_end: usize,
    /// Public-safe receipt refs inherited from the dense fixture.
    pub compile_receipt_refs: Vec<String>,
    /// Source dense module payload.
    pub dense_module: TassadarAlmDenseWeightModule,
}

/// One conformance case proving that a linked bank survives composition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarAlmLinkedDenseConformanceCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Source program id.
    pub program_id: String,
    /// Source dense module digest.
    pub dense_module_digest: String,
    /// Trace digest from executing the source dense module.
    pub source_trace_digest: String,
    /// Trace digest recomputed from the composed trace projection.
    pub projected_trace_digest: String,
    /// Whether every projected row matched the source dense row.
    pub projected_rows_match_source: bool,
    /// Number of rows compared.
    pub compared_step_count: usize,
    /// Start column in the composed trace row.
    pub projected_output_start: usize,
    /// End column in the composed trace row.
    pub projected_output_end: usize,
}

/// One linked dense module that carries all source banks and the composed module.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarAlmLinkedDenseModule {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable linked module kind.
    pub module_kind: String,
    /// Stable linked module id.
    pub module_id: String,
    /// Dense ALM source bank metadata and payloads.
    pub banks: Vec<TassadarAlmLinkedDenseBank>,
    /// Linker resolution proving the selected dense banks and dependency graph.
    pub link_resolution: TassadarModuleLinkResolution,
    /// One block-separated dense module materialized from the linked numeric model.
    pub composed_dense_module: TassadarAlmDenseWeightModule,
    /// Explicit claim boundary for this bounded linked module.
    pub claim_boundary: String,
}

impl TassadarAlmLinkedDenseModule {
    /// Returns a stable digest over the linked module payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"tassadar_alm_linked_dense_module|", self)
    }
}

/// Digest-pinned linked dense fixture consumed by OpenAgents marketplace rails.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TassadarAlmLinkedDenseProgramFixture {
    /// Stable fixture schema version.
    pub schema_version: u16,
    /// Stable fixture identifier.
    pub fixture_id: String,
    /// Generator identity.
    pub generated_by: String,
    /// Claim boundary inherited from the linked dense module.
    pub claim_boundary: String,
    /// Stable linked module digest.
    pub linked_module_digest: String,
    /// Stable digest of the composed dense module.
    pub composed_dense_module_digest: String,
    /// Stable digest of the composed numeric model.
    pub composed_model_digest: String,
    /// Trace digest produced by executing the composed dense module.
    pub composed_trace_digest: String,
    /// Linked dense module payload.
    pub linked_module: TassadarAlmLinkedDenseModule,
    /// Per-step input rows shared by all linked banks.
    pub steps: Vec<Vec<i64>>,
    /// Final composed trace row, if the run emitted any step.
    pub expected_final_row: Option<Vec<i64>>,
    /// Per-bank replay conformance results.
    pub conformance_cases: Vec<TassadarAlmLinkedDenseConformanceCase>,
    /// Public-safe receipt refs proving the derivation and replay path.
    pub compile_receipt_refs: Vec<String>,
    /// Public-safe artifact refs for marketplace/listing payloads.
    pub marketplace_artifact_refs: Vec<String>,
}

/// Failure returned while building a linked dense fixture.
#[derive(Debug, Error)]
pub enum TassadarAlmLinkedDenseProgramFixtureError {
    /// Dense source fixture could not be built.
    #[error(transparent)]
    DenseFixture(#[from] crate::tassadar_alm_dense_module::TassadarAlmDenseProgramFixtureError),
    /// Dense source module could not decode.
    #[error(transparent)]
    DenseModule(#[from] crate::tassadar_alm_dense_module::TassadarAlmDenseModuleError),
    /// Dense execution refused.
    #[error(transparent)]
    DenseExecution(#[from] TassadarAlmDenseExecutionError),
    /// Numeric execution refused.
    #[error(transparent)]
    NumericExecution(#[from] TassadarAlmNumericExecutionError),
    /// Module linker refused the selected dense bank manifests.
    #[error(transparent)]
    Link(#[from] TassadarModuleLinkError),
    /// Source dense modules did not share the same input arity.
    #[error("dense bank {program_id} input arity {found} differs from expected {expected}")]
    InputArityMismatch {
        /// Source program id.
        program_id: String,
        /// Found input arity.
        found: u16,
        /// Expected input arity.
        expected: u16,
    },
    /// Source dense modules did not share the same replay steps.
    #[error("dense bank {program_id} uses a different conformance step schedule")]
    StepScheduleMismatch {
        /// Source program id.
        program_id: String,
    },
    /// Slot count overflowed while composing dense banks.
    #[error("linked dense slot count overflowed")]
    SlotOverflow,
    /// Channel count overflowed while composing dense banks.
    #[error("linked dense channel count overflowed")]
    ChannelOverflow,
    /// A composed trace projection diverged from the source dense trace.
    #[error(
        "linked dense projection for {program_id} produced {projected_trace_digest} instead of {source_trace_digest}"
    )]
    ProjectionMismatch {
        /// Source program id.
        program_id: String,
        /// Source dense trace digest.
        source_trace_digest: String,
        /// Projected digest.
        projected_trace_digest: String,
    },
}

/// Builds the first linked dense fixture for OpenAgents compiled-module listings.
pub fn build_tassadar_alm_linked_dense_program_fixture_v1(
) -> Result<TassadarAlmLinkedDenseProgramFixture, TassadarAlmLinkedDenseProgramFixtureError> {
    let fixtures = vec![
        build_tassadar_alm_dense_program_fixture_for("tassadar_corpus.mul_add_v1")?,
        build_tassadar_alm_dense_program_fixture_for("tassadar_corpus.memory_roundtrip_v1")?,
    ];
    build_tassadar_alm_linked_dense_program_fixture(fixtures)
}

/// Builds a linked dense fixture from source dense program fixtures.
pub fn build_tassadar_alm_linked_dense_program_fixture(
    fixtures: Vec<TassadarAlmDenseProgramFixture>,
) -> Result<TassadarAlmLinkedDenseProgramFixture, TassadarAlmLinkedDenseProgramFixtureError> {
    let first = fixtures.first().ok_or(
        TassadarAlmLinkedDenseProgramFixtureError::StepScheduleMismatch {
            program_id: String::from("missing_dense_bank"),
        },
    )?;
    let expected_input_arity = first.dense_module.input_field_count;
    let steps = first.steps.clone();
    for fixture in &fixtures {
        if fixture.dense_module.input_field_count != expected_input_arity {
            return Err(
                TassadarAlmLinkedDenseProgramFixtureError::InputArityMismatch {
                    program_id: fixture.program_id.clone(),
                    found: fixture.dense_module.input_field_count,
                    expected: expected_input_arity,
                },
            );
        }
        if fixture.steps != steps {
            return Err(
                TassadarAlmLinkedDenseProgramFixtureError::StepScheduleMismatch {
                    program_id: fixture.program_id.clone(),
                },
            );
        }
    }

    let manifests = dense_bank_manifests(&fixtures);
    let request = TassadarModuleLinkRequest {
        consumer_family: String::from("tassadar_linked_dense_marketplace_v1"),
        requested_module_refs: manifests
            .iter()
            .map(|manifest| manifest.module_ref.clone())
            .collect(),
        rollback_module_refs: vec![],
        minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
        allowed_claim_classes: vec![String::from(TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS)],
    };
    let link_resolution = link_tassadar_module_dependency_graph(&manifests, &request)?;

    let decoded_models = fixtures
        .iter()
        .map(|fixture| tassadar_alm_dense_weight_module_to_numeric(&fixture.dense_module))
        .collect::<Result<Vec<_>, _>>()?;
    let (composed_model, bank_offsets) = compose_numeric_models(&fixtures, &decoded_models)?;
    let composed_dense_module = materialize_tassadar_alm_dense_weight_module(&composed_model);
    let composed_trace = tassadar_alm_dense_weight_module_execute(&composed_dense_module, &steps)?;
    let (banks, conformance_cases) = build_banks_and_conformance(
        &fixtures,
        &bank_offsets,
        &composed_trace.step_outputs,
        &steps,
    )?;

    let linked_module = TassadarAlmLinkedDenseModule {
        schema_version: TASSADAR_ALM_LINKED_DENSE_MODULE_SCHEMA_VERSION,
        module_kind: String::from(TASSADAR_ALM_LINKED_DENSE_MODULE_KIND),
        module_id: String::from("alm.linked_dense.tassadar_corpus.mul_add.memory_roundtrip.v1"),
        banks,
        link_resolution,
        composed_dense_module,
        claim_boundary: String::from(TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_BOUNDARY),
    };
    let linked_module_digest = linked_module.stable_digest();
    let composed_dense_module_digest = linked_module.composed_dense_module.stable_digest();
    let composed_model_digest = composed_model.stable_digest();
    let mut compile_receipt_refs = fixtures
        .iter()
        .flat_map(|fixture| fixture.compile_receipt_refs.clone())
        .collect::<Vec<_>>();
    compile_receipt_refs.push(format!(
        "receipt.psionic.tassadar_link_resolution.{}",
        &linked_module.link_resolution.resolution_digest[..16]
    ));
    compile_receipt_refs.push(format!(
        "receipt.psionic.tassadar_linked_dense_module.{}",
        &linked_module_digest[..16]
    ));
    compile_receipt_refs.push(format!(
        "receipt.psionic.tassadar_linked_dense_trace.{}",
        &composed_trace.trace_digest[..16]
    ));
    compile_receipt_refs.sort();
    compile_receipt_refs.dedup();

    Ok(TassadarAlmLinkedDenseProgramFixture {
        schema_version: TASSADAR_ALM_LINKED_DENSE_MODULE_SCHEMA_VERSION,
        fixture_id: String::from("tassadar_linked_dense.mul_add_memory_roundtrip.v1"),
        generated_by: String::from(TASSADAR_ALM_LINKED_DENSE_MODULE_GENERATED_BY),
        claim_boundary: String::from(TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_BOUNDARY),
        linked_module_digest: linked_module_digest.clone(),
        composed_dense_module_digest: composed_dense_module_digest.clone(),
        composed_model_digest,
        composed_trace_digest: composed_trace.trace_digest.clone(),
        linked_module,
        steps,
        expected_final_row: composed_trace.step_outputs.last().cloned(),
        conformance_cases,
        compile_receipt_refs,
        marketplace_artifact_refs: vec![
            format!(
                "artifact.public.tassadar_linked_dense_module.{}",
                &linked_module_digest[..16]
            ),
            format!(
                "artifact.public.tassadar_linked_dense_trace.{}",
                &composed_trace.trace_digest[..16]
            ),
            format!(
                "listing.public.tassadar_compiled_weight_module.{}",
                &linked_module_digest[..16]
            ),
        ],
    })
}

#[derive(Clone, Copy, Debug)]
struct BankOffset {
    slot_offset: u32,
    channel_offset: u16,
    projected_output_start: usize,
    projected_output_end: usize,
}

fn compose_numeric_models(
    fixtures: &[TassadarAlmDenseProgramFixture],
    models: &[TassadarAlmNumericModel],
) -> Result<(TassadarAlmNumericModel, Vec<BankOffset>), TassadarAlmLinkedDenseProgramFixtureError> {
    let mut slot_offset = 0_u32;
    let mut channel_offset = 0_u16;
    let mut output_start = 0_usize;
    let mut offsets = Vec::with_capacity(models.len());
    let mut seed_writes = Vec::new();
    let mut wiring = Vec::new();
    let mut attention = Vec::new();
    let mut ffn = Vec::new();
    let mut writes = Vec::new();
    let mut output_slots = Vec::new();
    let mut layer_count = 0_u32;
    for model in models {
        let output_end = output_start + model.output_slots.len();
        offsets.push(BankOffset {
            slot_offset,
            channel_offset,
            projected_output_start: output_start,
            projected_output_end: output_end,
        });
        seed_writes.extend(offset_seed_writes(&model.seed_writes, channel_offset)?);
        wiring.extend(offset_wiring(&model.wiring, slot_offset)?);
        attention.extend(offset_attention(
            &model.attention,
            slot_offset,
            channel_offset,
        )?);
        ffn.extend(offset_ffn(&model.ffn, slot_offset)?);
        writes.extend(offset_writes(&model.writes, slot_offset, channel_offset)?);
        output_slots.extend(offset_slots(&model.output_slots, slot_offset)?);
        layer_count = layer_count.max(model.layer_count);
        slot_offset = slot_offset
            .checked_add(model.slot_count)
            .ok_or(TassadarAlmLinkedDenseProgramFixtureError::SlotOverflow)?;
        channel_offset = next_channel_offset(model, channel_offset)?;
        output_start = output_end;
    }
    let graph_digest = stable_digest(
        b"tassadar_alm_linked_dense_graph|",
        &fixtures
            .iter()
            .map(|fixture| {
                (
                    fixture.program_id.as_str(),
                    fixture.dense_module_digest.as_str(),
                    fixture.expected_trace_digest.as_str(),
                )
            })
            .collect::<Vec<_>>(),
    );
    let bundle_digest = stable_digest(
        b"tassadar_alm_linked_dense_bundle|",
        &(
            graph_digest.as_str(),
            slot_offset,
            channel_offset,
            &output_slots,
        ),
    );
    Ok((
        TassadarAlmNumericModel {
            schema_version: 1,
            model_id: String::from(
                "alm.numeric.linked_dense.tassadar_corpus.mul_add.memory_roundtrip.v1",
            ),
            graph_digest,
            bundle_digest,
            input_field_count: models[0].input_field_count,
            slot_count: slot_offset,
            layer_count,
            seed_writes,
            wiring,
            attention,
            ffn,
            writes,
            output_slots,
        },
        offsets,
    ))
}

fn build_banks_and_conformance(
    fixtures: &[TassadarAlmDenseProgramFixture],
    offsets: &[BankOffset],
    composed_rows: &[Vec<i64>],
    steps: &[Vec<i64>],
) -> Result<
    (
        Vec<TassadarAlmLinkedDenseBank>,
        Vec<TassadarAlmLinkedDenseConformanceCase>,
    ),
    TassadarAlmLinkedDenseProgramFixtureError,
> {
    let mut banks = Vec::with_capacity(fixtures.len());
    let mut cases = Vec::with_capacity(fixtures.len());
    for (index, fixture) in fixtures.iter().enumerate() {
        let offset = offsets[index];
        let source_trace = tassadar_alm_dense_weight_module_execute(&fixture.dense_module, steps)?;
        let projected_rows = composed_rows
            .iter()
            .map(|row| row[offset.projected_output_start..offset.projected_output_end].to_vec())
            .collect::<Vec<_>>();
        let projected_trace_digest =
            trace_digest_for_rows(&source_trace.graph_digest, &projected_rows);
        if projected_rows != source_trace.step_outputs
            || projected_trace_digest != source_trace.trace_digest
        {
            return Err(
                TassadarAlmLinkedDenseProgramFixtureError::ProjectionMismatch {
                    program_id: fixture.program_id.clone(),
                    source_trace_digest: source_trace.trace_digest,
                    projected_trace_digest,
                },
            );
        }
        banks.push(TassadarAlmLinkedDenseBank {
            bank_id: format!("bank.{}.{}", index, fixture.program_id),
            module_ref: dense_bank_module_ref(&fixture.program_id),
            program_id: fixture.program_id.clone(),
            workload_kind: fixture.workload_kind.clone(),
            profile_id: fixture.profile_id.clone(),
            dense_module_digest: fixture.dense_module_digest.clone(),
            numeric_model_digest: fixture.numeric_model_digest.clone(),
            expected_trace_digest: fixture.expected_trace_digest.clone(),
            slot_offset: offset.slot_offset,
            channel_offset: offset.channel_offset,
            projected_output_start: offset.projected_output_start,
            projected_output_end: offset.projected_output_end,
            compile_receipt_refs: fixture.compile_receipt_refs.clone(),
            dense_module: fixture.dense_module.clone(),
        });
        cases.push(TassadarAlmLinkedDenseConformanceCase {
            case_id: format!("conformance.linked_dense.{}.v1", fixture.program_id),
            program_id: fixture.program_id.clone(),
            dense_module_digest: fixture.dense_module_digest.clone(),
            source_trace_digest: source_trace.trace_digest.clone(),
            projected_trace_digest,
            projected_rows_match_source: true,
            compared_step_count: steps.len(),
            projected_output_start: offset.projected_output_start,
            projected_output_end: offset.projected_output_end,
        });
    }
    Ok((banks, cases))
}

fn dense_bank_manifests(
    fixtures: &[TassadarAlmDenseProgramFixture],
) -> Vec<TassadarComputationalModuleManifest> {
    fixtures
        .iter()
        .enumerate()
        .map(|(index, fixture)| {
            let export_symbol = dense_bank_export_symbol(&fixture.program_id);
            let imports = if index == 0 {
                vec![]
            } else {
                vec![TassadarComputationalModuleImport {
                    symbol: dense_bank_export_symbol(&fixtures[0].program_id),
                    import_class: TassadarModuleImportClass::InternalModuleAbi,
                    required: true,
                    claim_boundary: String::from(
                        "the linked dense v1 composition requires the first dense bank as a bounded internal ABI dependency before listing",
                    ),
                }]
            };
            TassadarComputationalModuleManifest::new(
                format!("tassadar.module.{}.manifest.v1", dense_bank_id(&fixture.program_id)),
                dense_bank_module_ref(&fixture.program_id),
                "tassadar.alm.dense_module.abi.v1",
                TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS,
                TassadarModuleTrustPosture::BenchmarkGatedInternal,
                imports,
                vec![TassadarComputationalModuleExport {
                    symbol: export_symbol,
                    abi_version: 1,
                    input_channels: vec![String::from("step_input_0")],
                    output_channels: vec![String::from("interpreter_output_row")],
                    claim_boundary: format!(
                        "{} exports only its digest-pinned ALM interpreter output row under exact replay",
                        fixture.program_id
                    ),
                }],
                vec![TassadarComputationalModuleStateField {
                    field_id: format!("state.{}", dense_bank_id(&fixture.program_id)),
                    field_kind: TassadarModuleStateFieldKind::CandidateState,
                    shape: format!("slots[{}]", fixture.dense_module.d_model),
                    mutable: true,
                }],
                TassadarComputationalModuleCapabilitySummary {
                    capability_labels: vec![
                        String::from("dense_alm_bank"),
                        dense_bank_id(&fixture.program_id),
                    ],
                    supported_workload_families: vec![fixture.workload_kind.clone()],
                    refusal_boundaries: vec![
                        String::from("no arbitrary module install"),
                        String::from("no purchase settlement without replay verification"),
                        String::from("no softmax or learned-weight claim"),
                    ],
                },
                fixture.run_artifact_refs.clone(),
                fixture.compile_receipt_refs.clone(),
            )
        })
        .collect()
}

fn offset_seed_writes(
    seed_writes: &[(u16, f64, f64)],
    channel_offset: u16,
) -> Result<Vec<(u16, f64, f64)>, TassadarAlmLinkedDenseProgramFixtureError> {
    seed_writes
        .iter()
        .map(|(channel, key, value)| Ok((offset_channel(*channel, channel_offset)?, *key, *value)))
        .collect()
}

fn offset_wiring(
    wiring: &[TassadarAlmNumericWiringRow],
    slot_offset: u32,
) -> Result<Vec<TassadarAlmNumericWiringRow>, TassadarAlmLinkedDenseProgramFixtureError> {
    wiring
        .iter()
        .map(|row| {
            Ok(TassadarAlmNumericWiringRow {
                out_slot: offset_slot(row.out_slot, slot_offset)?,
                bias: row.bias,
                terms: row
                    .terms
                    .iter()
                    .map(|(coefficient, slot)| {
                        Ok::<_, TassadarAlmLinkedDenseProgramFixtureError>((
                            *coefficient,
                            offset_slot(*slot, slot_offset)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, TassadarAlmLinkedDenseProgramFixtureError>>()?,
                input_field: row.input_field,
                phase: row.phase,
            })
        })
        .collect()
}

fn offset_attention(
    attention: &[TassadarAlmNumericAttentionRow],
    slot_offset: u32,
    channel_offset: u16,
) -> Result<Vec<TassadarAlmNumericAttentionRow>, TassadarAlmLinkedDenseProgramFixtureError> {
    attention
        .iter()
        .map(|row| match row {
            TassadarAlmNumericAttentionRow::KeyedRead {
                channel,
                query_slot,
                out_slot,
                phase,
            } => Ok(TassadarAlmNumericAttentionRow::KeyedRead {
                channel: offset_channel(*channel, channel_offset)?,
                query_slot: offset_slot(*query_slot, slot_offset)?,
                out_slot: offset_slot(*out_slot, slot_offset)?,
                phase: *phase,
            }),
            TassadarAlmNumericAttentionRow::CumSum {
                channel,
                value_slot,
                out_slot,
                phase,
            } => Ok(TassadarAlmNumericAttentionRow::CumSum {
                channel: offset_channel(*channel, channel_offset)?,
                value_slot: offset_slot(*value_slot, slot_offset)?,
                out_slot: offset_slot(*out_slot, slot_offset)?,
                phase: *phase,
            }),
        })
        .collect()
}

fn offset_ffn(
    ffn: &[TassadarAlmNumericFfnRow],
    slot_offset: u32,
) -> Result<Vec<TassadarAlmNumericFfnRow>, TassadarAlmLinkedDenseProgramFixtureError> {
    ffn.iter()
        .map(|row| {
            Ok(TassadarAlmNumericFfnRow {
                value_slot: offset_slot(row.value_slot, slot_offset)?,
                gate_slot: offset_slot(row.gate_slot, slot_offset)?,
                out_slot: offset_slot(row.out_slot, slot_offset)?,
                phase: row.phase,
            })
        })
        .collect()
}

fn offset_writes(
    writes: &[TassadarAlmNumericWriteRow],
    slot_offset: u32,
    channel_offset: u16,
) -> Result<Vec<TassadarAlmNumericWriteRow>, TassadarAlmLinkedDenseProgramFixtureError> {
    writes
        .iter()
        .map(|row| {
            Ok(TassadarAlmNumericWriteRow {
                channel: offset_channel(row.channel, channel_offset)?,
                key_slot: offset_slot(row.key_slot, slot_offset)?,
                value_slot: offset_slot(row.value_slot, slot_offset)?,
            })
        })
        .collect()
}

fn offset_slots(
    slots: &[u32],
    slot_offset: u32,
) -> Result<Vec<u32>, TassadarAlmLinkedDenseProgramFixtureError> {
    slots
        .iter()
        .map(|slot| offset_slot(*slot, slot_offset))
        .collect()
}

fn offset_slot(slot: u32, offset: u32) -> Result<u32, TassadarAlmLinkedDenseProgramFixtureError> {
    slot.checked_add(offset)
        .ok_or(TassadarAlmLinkedDenseProgramFixtureError::SlotOverflow)
}

fn offset_channel(
    channel: u16,
    offset: u16,
) -> Result<u16, TassadarAlmLinkedDenseProgramFixtureError> {
    channel
        .checked_add(offset)
        .ok_or(TassadarAlmLinkedDenseProgramFixtureError::ChannelOverflow)
}

fn next_channel_offset(
    model: &TassadarAlmNumericModel,
    current_offset: u16,
) -> Result<u16, TassadarAlmLinkedDenseProgramFixtureError> {
    let mut max_channel = 0_u16;
    for (channel, _, _) in &model.seed_writes {
        max_channel = max_channel.max(*channel);
    }
    for row in &model.attention {
        let channel = match row {
            TassadarAlmNumericAttentionRow::KeyedRead { channel, .. }
            | TassadarAlmNumericAttentionRow::CumSum { channel, .. } => *channel,
        };
        max_channel = max_channel.max(channel);
    }
    for row in &model.writes {
        max_channel = max_channel.max(row.channel);
    }
    current_offset
        .checked_add(max_channel)
        .and_then(|value| value.checked_add(1))
        .ok_or(TassadarAlmLinkedDenseProgramFixtureError::ChannelOverflow)
}

fn dense_bank_id(program_id: &str) -> String {
    program_id
        .strip_prefix("tassadar_corpus.")
        .unwrap_or(program_id)
        .replace('.', "_")
}

fn dense_bank_module_ref(program_id: &str) -> String {
    format!("tassadar_dense_{}@1.0.0", dense_bank_id(program_id))
}

fn dense_bank_export_symbol(program_id: &str) -> String {
    format!("dense.{}.output_row", dense_bank_id(program_id))
}

fn trace_digest_for_rows(graph_digest: &str, rows: &[Vec<i64>]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tassadar_alm_trace|");
    hasher.update(graph_digest.as_bytes());
    for row in rows {
        hasher.update(b"|row|");
        for value in row {
            hasher.update(value.to_le_bytes());
        }
    }
    hex::encode(hasher.finalize())
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

    use crate::TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND;

    use super::*;

    #[test]
    fn linked_dense_module_composes_two_banks_and_replay_verifies() {
        let fixture =
            build_tassadar_alm_linked_dense_program_fixture_v1().expect("linked fixture builds");

        assert_eq!(
            fixture.linked_module.module_kind,
            TASSADAR_ALM_LINKED_DENSE_MODULE_KIND
        );
        assert_eq!(fixture.linked_module.banks.len(), 2);
        assert_eq!(fixture.conformance_cases.len(), 2);
        assert_eq!(
            fixture
                .linked_module
                .link_resolution
                .dependency_graph
                .nodes
                .len(),
            2
        );
        assert_eq!(
            fixture
                .linked_module
                .link_resolution
                .dependency_graph
                .edges
                .len(),
            1
        );
        assert!(fixture
            .conformance_cases
            .iter()
            .all(|case| case.projected_rows_match_source));
        assert!(fixture.marketplace_artifact_refs.iter().any(|reference| {
            reference.starts_with("listing.public.tassadar_compiled_weight_module.")
        }));
    }

    #[test]
    fn linked_dense_link_resolution_carries_compatibility_evidence_for_each_bank() {
        let fixture =
            build_tassadar_alm_linked_dense_program_fixture_v1().expect("linked fixture builds");
        let bank_refs = fixture
            .linked_module
            .banks
            .iter()
            .map(|bank| bank.module_ref.as_str())
            .collect::<Vec<_>>();
        let resolution = &fixture.linked_module.link_resolution;

        assert_eq!(resolution.posture, crate::TassadarModuleLinkPosture::Exact);
        assert_eq!(resolution.requested_module_refs, bank_refs);
        assert_eq!(resolution.selected_module_refs, bank_refs);
        assert_eq!(
            resolution.dependency_graph.consumer_family,
            "tassadar_linked_dense_marketplace_v1"
        );
        assert_eq!(resolution.dependency_graph.nodes.len(), bank_refs.len());
        assert!(resolution.dependency_graph.nodes.iter().all(|node| {
            bank_refs.contains(&node.module_ref.as_str())
                && node.trust_posture == TassadarModuleTrustPosture::BenchmarkGatedInternal
                && node.claim_class == TASSADAR_ALM_LINKED_DENSE_MODULE_CLAIM_CLASS
                && node.compatibility_digest.len() == 64
        }));
        assert!(resolution.dependency_graph.edges.iter().any(|edge| {
            edge.provider_module_ref == bank_refs[0] && edge.importer_module_ref == bank_refs[1]
        }));
        assert!(fixture
            .compile_receipt_refs
            .iter()
            .any(|receipt| { receipt.starts_with("receipt.psionic.tassadar_link_resolution.") }));
    }

    #[test]
    fn linked_dense_link_resolution_refuses_injected_claim_class_incompatibility() {
        let fixtures = vec![
            build_tassadar_alm_dense_program_fixture_for("tassadar_corpus.mul_add_v1")
                .expect("left fixture"),
            build_tassadar_alm_dense_program_fixture_for("tassadar_corpus.memory_roundtrip_v1")
                .expect("right fixture"),
        ];
        let manifests = dense_bank_manifests(&fixtures);
        let request = TassadarModuleLinkRequest {
            consumer_family: String::from("tassadar_linked_dense_marketplace_v1"),
            requested_module_refs: manifests
                .iter()
                .map(|manifest| manifest.module_ref.clone())
                .collect(),
            rollback_module_refs: vec![],
            minimum_trust_posture: TassadarModuleTrustPosture::BenchmarkGatedInternal,
            allowed_claim_classes: vec![String::from("wrong claim class")],
        };

        let error = link_tassadar_module_dependency_graph(&manifests, &request)
            .expect_err("claim-class incompatibility refuses");

        assert!(matches!(
            error,
            TassadarModuleLinkError::Compatibility(
                crate::TassadarModuleCompatibilityError::ClaimClassDisallowed { .. }
            )
        ));
    }

    #[test]
    fn linked_dense_fixture_is_deterministic_and_digest_pinned() {
        let a =
            build_tassadar_alm_linked_dense_program_fixture_v1().expect("linked fixture a builds");
        let b =
            build_tassadar_alm_linked_dense_program_fixture_v1().expect("linked fixture b builds");

        assert_eq!(a, b);
        assert_eq!(a.linked_module_digest, a.linked_module.stable_digest());
        assert_eq!(
            a.linked_module.composed_dense_module.module_kind,
            TASSADAR_ALM_DENSE_WEIGHT_MODULE_KIND
        );
        assert!(a
            .compile_receipt_refs
            .iter()
            .any(|receipt| receipt.starts_with("receipt.psionic.tassadar_linked_dense_trace.")));
    }

    #[test]
    fn linked_dense_fixture_refuses_mismatched_step_schedule() {
        let mut left = build_tassadar_alm_dense_program_fixture_for("tassadar_corpus.mul_add_v1")
            .expect("left fixture");
        let right =
            build_tassadar_alm_dense_program_fixture_for("tassadar_corpus.memory_roundtrip_v1")
                .expect("right fixture");
        left.steps.push(vec![0]);

        let error = build_tassadar_alm_linked_dense_program_fixture(vec![left, right])
            .expect_err("step mismatch refuses");

        assert!(matches!(
            error,
            TassadarAlmLinkedDenseProgramFixtureError::StepScheduleMismatch { .. }
        ));
    }
}

#[cfg(test)]
mod fixture_dump {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    #[ignore]
    fn dump_linked_dense_module_fixture() {
        let fixture =
            build_tassadar_alm_linked_dense_program_fixture_v1().expect("linked fixture builds");
        std::fs::write(
            "/tmp/tassadar-linked-dense-module-v1.json",
            serde_json::to_vec_pretty(&fixture).expect("encodes"),
        )
        .expect("writes");
        eprintln!("linked_module_digest={}", fixture.linked_module_digest);
        eprintln!("composed_trace_digest={}", fixture.composed_trace_digest);
    }
}
