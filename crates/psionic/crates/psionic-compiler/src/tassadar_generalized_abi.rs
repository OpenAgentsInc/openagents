use psionic_ir::{
    TassadarGeneralizedAbiFixture, TassadarGeneralizedAbiFixtureId,
    TassadarGeneralizedAbiParamKind, TassadarGeneralizedAbiResultKind,
};
use psionic_runtime::{
    TassadarGeneralizedAbiError, TassadarGeneralizedAbiInstruction, TassadarGeneralizedAbiProgram,
    TassadarStructuredControlBinaryOp, tassadar_generalized_abi_program_id,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Lowered artifact for one generalized ABI fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralizedAbiArtifact {
    pub fixture_id: String,
    pub source_case_id: String,
    pub source_ref: String,
    pub program: TassadarGeneralizedAbiProgram,
    pub claim_class: String,
    pub artifact_digest: String,
}

impl TassadarGeneralizedAbiArtifact {
    fn new(
        fixture: &TassadarGeneralizedAbiFixture,
        program: TassadarGeneralizedAbiProgram,
    ) -> Self {
        let mut artifact = Self {
            fixture_id: String::from(fixture.fixture_id.as_str()),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            program,
            claim_class: String::from("compiled_bounded_exactness"),
            artifact_digest: String::new(),
        };
        artifact.artifact_digest =
            stable_digest(b"psionic_tassadar_generalized_abi_artifact|", &artifact);
        artifact
    }
}

/// Typed lowering failure for the generalized ABI family.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error_kind")]
pub enum TassadarGeneralizedAbiLoweringError {
    #[error(
        "generalized ABI fixture `{fixture_id}` declares unsupported param {param_index} kind `{kind:?}`"
    )]
    UnsupportedParamKind {
        fixture_id: String,
        param_index: u8,
        kind: TassadarGeneralizedAbiParamKind,
    },
    #[error("generalized ABI fixture `{fixture_id}` declares unsupported result kind `{kind:?}`")]
    UnsupportedResultKind {
        fixture_id: String,
        kind: TassadarGeneralizedAbiResultKind,
    },
    #[error("generalized ABI fixture `{fixture_id}` declares unsupported result kinds `{kinds:?}`")]
    UnsupportedResultKinds {
        fixture_id: String,
        kinds: Vec<TassadarGeneralizedAbiResultKind>,
    },
    #[error("generalized ABI fixture `{fixture_id}` failed runtime validation: {error}")]
    InvalidLoweredProgram {
        fixture_id: String,
        error: TassadarGeneralizedAbiError,
    },
}

/// Lowers one generalized ABI fixture into a runtime program.
pub fn lower_tassadar_generalized_abi_fixture(
    fixture: &TassadarGeneralizedAbiFixture,
) -> Result<TassadarGeneralizedAbiArtifact, TassadarGeneralizedAbiLoweringError> {
    for (param_index, kind) in fixture.param_kinds.iter().copied().enumerate() {
        match kind {
            TassadarGeneralizedAbiParamKind::I32
            | TassadarGeneralizedAbiParamKind::I64
            | TassadarGeneralizedAbiParamKind::PointerToI32
            | TassadarGeneralizedAbiParamKind::LengthI32 => {}
            unsupported => {
                return Err(TassadarGeneralizedAbiLoweringError::UnsupportedParamKind {
                    fixture_id: String::from(fixture.fixture_id.as_str()),
                    param_index: param_index as u8,
                    kind: unsupported,
                });
            }
        }
    }

    let result_kinds = match fixture.result_kinds.as_slice() {
        [] => Vec::new(),
        [TassadarGeneralizedAbiResultKind::I32]
        | [TassadarGeneralizedAbiResultKind::I64]
        | [
            TassadarGeneralizedAbiResultKind::I32,
            TassadarGeneralizedAbiResultKind::I32,
        ] => fixture.result_kinds.clone(),
        [unsupported] => {
            return Err(TassadarGeneralizedAbiLoweringError::UnsupportedResultKind {
                fixture_id: String::from(fixture.fixture_id.as_str()),
                kind: *unsupported,
            });
        }
        kinds => {
            return Err(
                TassadarGeneralizedAbiLoweringError::UnsupportedResultKinds {
                    fixture_id: String::from(fixture.fixture_id.as_str()),
                    kinds: kinds.to_vec(),
                },
            );
        }
    };

    let program = match fixture.fixture_id {
        TassadarGeneralizedAbiFixtureId::PairAddI32 => TassadarGeneralizedAbiProgram {
            program_id: tassadar_generalized_abi_program_id(fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            program_shape_id: fixture.program_shape_id.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kinds,
            local_count: 2,
            memory_regions: fixture.memory_regions.clone(),
            runtime_support_ids: fixture.runtime_support_ids.clone(),
            instructions: vec![
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 0 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 1 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarGeneralizedAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary.clone(),
        },
        TassadarGeneralizedAbiFixtureId::PairAddI64 => TassadarGeneralizedAbiProgram {
            program_id: tassadar_generalized_abi_program_id(fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            program_shape_id: fixture.program_shape_id.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kinds,
            local_count: 2,
            memory_regions: fixture.memory_regions.clone(),
            runtime_support_ids: fixture.runtime_support_ids.clone(),
            instructions: vec![
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 0 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 1 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarGeneralizedAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary.clone(),
        },
        TassadarGeneralizedAbiFixtureId::DualHeapDotI32 => TassadarGeneralizedAbiProgram {
            program_id: tassadar_generalized_abi_program_id(fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            program_shape_id: fixture.program_shape_id.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kinds,
            local_count: 7,
            memory_regions: fixture.memory_regions.clone(),
            runtime_support_ids: fixture.runtime_support_ids.clone(),
            instructions: vec![
                TassadarGeneralizedAbiInstruction::I32Const { value: 0 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 3 },
                TassadarGeneralizedAbiInstruction::I32Const { value: 0 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 4 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 4 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 2 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::LtS,
                },
                TassadarGeneralizedAbiInstruction::BranchIfZero { target_pc: 23 },
                TassadarGeneralizedAbiInstruction::I32LoadRegionAtIndex {
                    region_id: String::from("left_input"),
                    index_local_index: 4,
                },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 5 },
                TassadarGeneralizedAbiInstruction::I32LoadRegionAtIndex {
                    region_id: String::from("right_input"),
                    index_local_index: 4,
                },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 6 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 3 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 5 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 6 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Mul,
                },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 3 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 4 },
                TassadarGeneralizedAbiInstruction::I32Const { value: 1 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 4 },
                TassadarGeneralizedAbiInstruction::Jump { target_pc: 4 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 3 },
                TassadarGeneralizedAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary.clone(),
        },
        TassadarGeneralizedAbiFixtureId::SumAndMaxStatusOutput => TassadarGeneralizedAbiProgram {
            program_id: tassadar_generalized_abi_program_id(fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            program_shape_id: fixture.program_shape_id.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kinds,
            local_count: 8,
            memory_regions: fixture.memory_regions.clone(),
            runtime_support_ids: fixture.runtime_support_ids.clone(),
            instructions: vec![
                TassadarGeneralizedAbiInstruction::I32Const { value: 0 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 4 },
                TassadarGeneralizedAbiInstruction::I32Const { value: 0 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 5 },
                TassadarGeneralizedAbiInstruction::I32Const { value: 0 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 6 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 6 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 1 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::LtS,
                },
                TassadarGeneralizedAbiInstruction::BranchIfZero { target_pc: 27 },
                TassadarGeneralizedAbiInstruction::I32LoadRegionAtIndex {
                    region_id: String::from("input_values"),
                    index_local_index: 6,
                },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 7 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 4 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 7 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 4 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 7 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 5 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::GtS,
                },
                TassadarGeneralizedAbiInstruction::BranchIfZero { target_pc: 22 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 7 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 5 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 6 },
                TassadarGeneralizedAbiInstruction::I32Const { value: 1 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 6 },
                TassadarGeneralizedAbiInstruction::Jump { target_pc: 6 },
                TassadarGeneralizedAbiInstruction::I32Const { value: 0 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 6 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 4 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 7 },
                TassadarGeneralizedAbiInstruction::I32StoreRegionAtIndex {
                    region_id: String::from("output_values"),
                    index_local_index: 6,
                    value_local_index: 7,
                },
                TassadarGeneralizedAbiInstruction::I32Const { value: 1 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 6 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 5 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 7 },
                TassadarGeneralizedAbiInstruction::I32StoreRegionAtIndex {
                    region_id: String::from("output_values"),
                    index_local_index: 6,
                    value_local_index: 7,
                },
                TassadarGeneralizedAbiInstruction::I32Const { value: 0 },
                TassadarGeneralizedAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary.clone(),
        },
        TassadarGeneralizedAbiFixtureId::PairSumAndDiffI32 => TassadarGeneralizedAbiProgram {
            program_id: tassadar_generalized_abi_program_id(fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            program_shape_id: fixture.program_shape_id.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kinds,
            local_count: 2,
            memory_regions: fixture.memory_regions.clone(),
            runtime_support_ids: fixture.runtime_support_ids.clone(),
            instructions: vec![
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 0 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 1 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 0 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 1 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Sub,
                },
                TassadarGeneralizedAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary.clone(),
        },
        TassadarGeneralizedAbiFixtureId::SumAndMaxI64StatusOutput => {
            TassadarGeneralizedAbiProgram {
                program_id: tassadar_generalized_abi_program_id(fixture),
                fixture_id: String::from(fixture.fixture_id.as_str()),
                source_case_id: fixture.source_case_id.clone(),
                source_ref: fixture.source_ref.clone(),
                export_name: fixture.export_name.clone(),
                program_shape_id: fixture.program_shape_id.clone(),
                param_kinds: fixture.param_kinds.clone(),
                result_kinds,
                local_count: 8,
                memory_regions: fixture.memory_regions.clone(),
                runtime_support_ids: fixture.runtime_support_ids.clone(),
                instructions: vec![
                    TassadarGeneralizedAbiInstruction::I64Const { value: 0 },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 4 },
                    TassadarGeneralizedAbiInstruction::I64Const { value: 0 },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 5 },
                    TassadarGeneralizedAbiInstruction::I32Const { value: 0 },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 6 },
                    TassadarGeneralizedAbiInstruction::LocalGet { local_index: 6 },
                    TassadarGeneralizedAbiInstruction::LocalGet { local_index: 1 },
                    TassadarGeneralizedAbiInstruction::BinaryOp {
                        op: TassadarStructuredControlBinaryOp::LtS,
                    },
                    TassadarGeneralizedAbiInstruction::BranchIfZero { target_pc: 27 },
                    TassadarGeneralizedAbiInstruction::I64LoadRegionAtIndex {
                        region_id: String::from("input_values_i64"),
                        index_local_index: 6,
                    },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 7 },
                    TassadarGeneralizedAbiInstruction::LocalGet { local_index: 4 },
                    TassadarGeneralizedAbiInstruction::LocalGet { local_index: 7 },
                    TassadarGeneralizedAbiInstruction::BinaryOp {
                        op: TassadarStructuredControlBinaryOp::Add,
                    },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 4 },
                    TassadarGeneralizedAbiInstruction::LocalGet { local_index: 7 },
                    TassadarGeneralizedAbiInstruction::LocalGet { local_index: 5 },
                    TassadarGeneralizedAbiInstruction::BinaryOp {
                        op: TassadarStructuredControlBinaryOp::GtS,
                    },
                    TassadarGeneralizedAbiInstruction::BranchIfZero { target_pc: 22 },
                    TassadarGeneralizedAbiInstruction::LocalGet { local_index: 7 },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 5 },
                    TassadarGeneralizedAbiInstruction::LocalGet { local_index: 6 },
                    TassadarGeneralizedAbiInstruction::I32Const { value: 1 },
                    TassadarGeneralizedAbiInstruction::BinaryOp {
                        op: TassadarStructuredControlBinaryOp::Add,
                    },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 6 },
                    TassadarGeneralizedAbiInstruction::Jump { target_pc: 6 },
                    TassadarGeneralizedAbiInstruction::I32Const { value: 0 },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 6 },
                    TassadarGeneralizedAbiInstruction::LocalGet { local_index: 4 },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 7 },
                    TassadarGeneralizedAbiInstruction::I64StoreRegionAtIndex {
                        region_id: String::from("output_values_i64"),
                        index_local_index: 6,
                        value_local_index: 7,
                    },
                    TassadarGeneralizedAbiInstruction::I32Const { value: 1 },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 6 },
                    TassadarGeneralizedAbiInstruction::LocalGet { local_index: 5 },
                    TassadarGeneralizedAbiInstruction::LocalSet { local_index: 7 },
                    TassadarGeneralizedAbiInstruction::I64StoreRegionAtIndex {
                        region_id: String::from("output_values_i64"),
                        index_local_index: 6,
                        value_local_index: 7,
                    },
                    TassadarGeneralizedAbiInstruction::I32Const { value: 0 },
                    TassadarGeneralizedAbiInstruction::Return,
                ],
                claim_boundary: fixture.claim_boundary.clone(),
            }
        }
        TassadarGeneralizedAbiFixtureId::MultiExportPairSum => TassadarGeneralizedAbiProgram {
            program_id: tassadar_generalized_abi_program_id(fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            program_shape_id: fixture.program_shape_id.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kinds,
            local_count: 0,
            memory_regions: fixture.memory_regions.clone(),
            runtime_support_ids: fixture.runtime_support_ids.clone(),
            instructions: vec![
                TassadarGeneralizedAbiInstruction::I32Const { value: 2 },
                TassadarGeneralizedAbiInstruction::I32Const { value: 3 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarGeneralizedAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary.clone(),
        },
        TassadarGeneralizedAbiFixtureId::MultiExportLocalDouble => TassadarGeneralizedAbiProgram {
            program_id: tassadar_generalized_abi_program_id(fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            export_name: fixture.export_name.clone(),
            program_shape_id: fixture.program_shape_id.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kinds,
            local_count: 1,
            memory_regions: fixture.memory_regions.clone(),
            runtime_support_ids: fixture.runtime_support_ids.clone(),
            instructions: vec![
                TassadarGeneralizedAbiInstruction::I32Const { value: 7 },
                TassadarGeneralizedAbiInstruction::LocalSet { local_index: 0 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 0 },
                TassadarGeneralizedAbiInstruction::LocalGet { local_index: 0 },
                TassadarGeneralizedAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarGeneralizedAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary.clone(),
        },
        TassadarGeneralizedAbiFixtureId::UnsupportedFloatParam => {
            return Err(TassadarGeneralizedAbiLoweringError::UnsupportedParamKind {
                fixture_id: String::from(fixture.fixture_id.as_str()),
                param_index: 0,
                kind: TassadarGeneralizedAbiParamKind::F32,
            });
        }
        TassadarGeneralizedAbiFixtureId::UnsupportedMultiResult => {
            return Err(
                TassadarGeneralizedAbiLoweringError::UnsupportedResultKinds {
                    fixture_id: String::from(fixture.fixture_id.as_str()),
                    kinds: fixture.result_kinds.clone(),
                },
            );
        }
        TassadarGeneralizedAbiFixtureId::UnsupportedHostHandle => {
            return Err(TassadarGeneralizedAbiLoweringError::UnsupportedParamKind {
                fixture_id: String::from(fixture.fixture_id.as_str()),
                param_index: 0,
                kind: TassadarGeneralizedAbiParamKind::HostHandle,
            });
        }
        TassadarGeneralizedAbiFixtureId::UnsupportedReturnedBuffer => {
            return Err(TassadarGeneralizedAbiLoweringError::UnsupportedResultKind {
                fixture_id: String::from(fixture.fixture_id.as_str()),
                kind: TassadarGeneralizedAbiResultKind::BufferPointerAndLength,
            });
        }
    };

    program.validate().map_err(|error| {
        TassadarGeneralizedAbiLoweringError::InvalidLoweredProgram {
            fixture_id: String::from(fixture.fixture_id.as_str()),
            error,
        }
    })?;

    Ok(TassadarGeneralizedAbiArtifact::new(fixture, program))
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TassadarGeneralizedAbiLoweringError, lower_tassadar_generalized_abi_fixture};
    use psionic_ir::TassadarGeneralizedAbiFixture;
    use psionic_runtime::{
        execute_tassadar_generalized_abi_program,
        tassadar_generalized_abi_dual_heap_dot_invocation,
        tassadar_generalized_abi_i64_status_output_invocation,
        tassadar_generalized_abi_pair_add_i64_invocation,
        tassadar_generalized_abi_pair_add_invocation,
        tassadar_generalized_abi_pair_sum_and_diff_i32_invocation,
        tassadar_generalized_abi_status_output_invocation,
    };

    #[test]
    fn generalized_abi_fixtures_lower_and_match_runtime_truth() {
        let pair =
            lower_tassadar_generalized_abi_fixture(&TassadarGeneralizedAbiFixture::pair_add_i32())
                .expect("pair add should lower");
        let pair_execution = execute_tassadar_generalized_abi_program(
            &pair.program,
            &tassadar_generalized_abi_pair_add_invocation(),
        )
        .expect("pair add should execute");
        assert_eq!(pair_execution.returned_value, Some(42));

        let pair_i64 =
            lower_tassadar_generalized_abi_fixture(&TassadarGeneralizedAbiFixture::pair_add_i64())
                .expect("pair add i64 should lower");
        let pair_i64_execution = execute_tassadar_generalized_abi_program(
            &pair_i64.program,
            &tassadar_generalized_abi_pair_add_i64_invocation(),
        )
        .expect("pair add i64 should execute");
        assert_eq!(pair_i64_execution.returned_i64, Some(42));

        let dot = lower_tassadar_generalized_abi_fixture(
            &TassadarGeneralizedAbiFixture::dual_heap_dot_i32(),
        )
        .expect("dual heap dot should lower");
        let dot_execution = execute_tassadar_generalized_abi_program(
            &dot.program,
            &tassadar_generalized_abi_dual_heap_dot_invocation(),
        )
        .expect("dual heap dot should execute");
        assert_eq!(dot_execution.returned_value, Some(32));

        let output = lower_tassadar_generalized_abi_fixture(
            &TassadarGeneralizedAbiFixture::sum_and_max_status_output(),
        )
        .expect("status output should lower");
        let output_execution = execute_tassadar_generalized_abi_program(
            &output.program,
            &tassadar_generalized_abi_status_output_invocation(),
        )
        .expect("status output should execute");
        assert_eq!(output_execution.returned_value, Some(0));
        assert_eq!(output_execution.output_regions[0].words, vec![19, 9]);

        let pair_multi = lower_tassadar_generalized_abi_fixture(
            &TassadarGeneralizedAbiFixture::pair_sum_and_diff_i32(),
        )
        .expect("pair sum and diff should lower");
        let pair_multi_execution = execute_tassadar_generalized_abi_program(
            &pair_multi.program,
            &tassadar_generalized_abi_pair_sum_and_diff_i32_invocation(),
        )
        .expect("pair sum and diff should execute");
        assert_eq!(pair_multi_execution.returned_values, vec![42, -2]);

        let i64_output = lower_tassadar_generalized_abi_fixture(
            &TassadarGeneralizedAbiFixture::sum_and_max_i64_status_output(),
        )
        .expect("i64 output shape should lower");
        let i64_output_execution = execute_tassadar_generalized_abi_program(
            &i64_output.program,
            &tassadar_generalized_abi_i64_status_output_invocation(),
        )
        .expect("i64 output shape should execute");
        assert_eq!(i64_output_execution.returned_value, Some(0));
        assert_eq!(i64_output_execution.output_regions[0].words, vec![19, 9]);
    }

    #[test]
    fn generalized_abi_lowering_refuses_unsupported_shapes() {
        for fixture in [
            TassadarGeneralizedAbiFixture::unsupported_float_param(),
            TassadarGeneralizedAbiFixture::unsupported_host_handle(),
        ] {
            let error = lower_tassadar_generalized_abi_fixture(&fixture)
                .expect_err("fixture should refuse");
            assert!(matches!(
                error,
                TassadarGeneralizedAbiLoweringError::UnsupportedParamKind { .. }
            ));
        }

        for fixture in [
            TassadarGeneralizedAbiFixture::unsupported_multi_result(),
            TassadarGeneralizedAbiFixture::unsupported_returned_buffer(),
        ] {
            let error = lower_tassadar_generalized_abi_fixture(&fixture)
                .expect_err("fixture should refuse");
            assert!(matches!(
                error,
                TassadarGeneralizedAbiLoweringError::UnsupportedResultKind { .. }
                    | TassadarGeneralizedAbiLoweringError::UnsupportedResultKinds { .. }
            ));
        }
    }
}
