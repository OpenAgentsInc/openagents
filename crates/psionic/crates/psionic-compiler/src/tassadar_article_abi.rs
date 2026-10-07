use psionic_ir::{
    TassadarArticleAbiFixture, TassadarArticleAbiFixtureId, TassadarArticleAbiParamKind,
    TassadarArticleAbiResultKind,
};
use psionic_runtime::{
    TassadarArticleAbiError, TassadarArticleAbiInstruction, TassadarArticleAbiProgram,
    TassadarStructuredControlBinaryOp, tassadar_article_abi_program_id,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Compiler-facing lowered artifact for one bounded Rust-only article ABI fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleAbiArtifact {
    /// Stable fixture identifier.
    pub fixture_id: String,
    /// Stable source-canon case id.
    pub source_case_id: String,
    /// Stable Rust source reference.
    pub source_ref: String,
    /// Runtime-facing lowered program.
    pub program: TassadarArticleAbiProgram,
    /// Coarse claim class for the artifact.
    pub claim_class: String,
    /// Stable digest over the artifact.
    pub artifact_digest: String,
}

impl TassadarArticleAbiArtifact {
    fn new(fixture: &TassadarArticleAbiFixture, program: TassadarArticleAbiProgram) -> Self {
        let mut artifact = Self {
            fixture_id: String::from(fixture.fixture_id.as_str()),
            source_case_id: fixture.source_case_id.clone(),
            source_ref: fixture.source_ref.clone(),
            program,
            claim_class: String::from("compiled_bounded_exactness"),
            artifact_digest: String::new(),
        };
        artifact.artifact_digest =
            stable_digest(b"psionic_tassadar_article_abi_artifact|", &artifact);
        artifact
    }
}

/// Typed lowering failure for the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error_kind")]
pub enum TassadarArticleAbiLoweringError {
    #[error(
        "article ABI fixture `{fixture_id}` declares unsupported param {param_index} kind `{kind:?}`"
    )]
    UnsupportedParamKind {
        fixture_id: String,
        param_index: u8,
        kind: TassadarArticleAbiParamKind,
    },
    #[error("article ABI fixture `{fixture_id}` declares unsupported result kind `{kind:?}`")]
    UnsupportedResultKind {
        fixture_id: String,
        kind: TassadarArticleAbiResultKind,
    },
    #[error("article ABI fixture `{fixture_id}` declared an unsupported heap layout")]
    UnsupportedHeapLayout { fixture_id: String },
    #[error("article ABI fixture `{fixture_id}` failed runtime validation: {error}")]
    InvalidLoweredProgram {
        fixture_id: String,
        error: TassadarArticleAbiError,
    },
}

/// Lowers one canonical Rust-only article ABI fixture into a runtime program.
pub fn lower_tassadar_article_abi_fixture(
    fixture: &TassadarArticleAbiFixture,
) -> Result<TassadarArticleAbiArtifact, TassadarArticleAbiLoweringError> {
    for (param_index, kind) in fixture.param_kinds.iter().copied().enumerate() {
        match kind {
            TassadarArticleAbiParamKind::I32
            | TassadarArticleAbiParamKind::PointerToI32
            | TassadarArticleAbiParamKind::LengthI32 => {}
            unsupported => {
                return Err(TassadarArticleAbiLoweringError::UnsupportedParamKind {
                    fixture_id: String::from(fixture.fixture_id.as_str()),
                    param_index: param_index as u8,
                    kind: unsupported,
                });
            }
        }
    }

    let result_kind = match fixture.result_kinds.as_slice() {
        [] => None,
        [TassadarArticleAbiResultKind::I32] => Some(TassadarArticleAbiResultKind::I32),
        [unsupported]
        | [TassadarArticleAbiResultKind::I32, unsupported, ..]
        | [unsupported, ..] => {
            return Err(TassadarArticleAbiLoweringError::UnsupportedResultKind {
                fixture_id: String::from(fixture.fixture_id.as_str()),
                kind: *unsupported,
            });
        }
    };

    if let Some(heap_layout) = &fixture.heap_layout
        && heap_layout.element_width_bytes != 4
    {
        return Err(TassadarArticleAbiLoweringError::UnsupportedHeapLayout {
            fixture_id: String::from(fixture.fixture_id.as_str()),
        });
    }

    let program = match fixture.fixture_id {
        TassadarArticleAbiFixtureId::ScalarAddOne => TassadarArticleAbiProgram {
            program_id: tassadar_article_abi_program_id(fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            export_name: fixture.export_name.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kind,
            local_count: 1,
            heap_layout: None,
            instructions: vec![
                TassadarArticleAbiInstruction::LocalGet { local_index: 0 },
                TassadarArticleAbiInstruction::I32Const { value: 1 },
                TassadarArticleAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarArticleAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary.clone(),
        },
        TassadarArticleAbiFixtureId::HeapSumI32 => TassadarArticleAbiProgram {
            program_id: tassadar_article_abi_program_id(fixture),
            fixture_id: String::from(fixture.fixture_id.as_str()),
            export_name: fixture.export_name.clone(),
            param_kinds: fixture.param_kinds.clone(),
            result_kind,
            local_count: 4,
            heap_layout: fixture.heap_layout.clone(),
            instructions: vec![
                TassadarArticleAbiInstruction::I32Const { value: 0 },
                TassadarArticleAbiInstruction::LocalSet { local_index: 2 },
                TassadarArticleAbiInstruction::I32Const { value: 0 },
                TassadarArticleAbiInstruction::LocalSet { local_index: 3 },
                TassadarArticleAbiInstruction::LocalGet { local_index: 3 },
                TassadarArticleAbiInstruction::LocalGet { local_index: 1 },
                TassadarArticleAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::LtS,
                },
                TassadarArticleAbiInstruction::BranchIfZero { target_pc: 17 },
                TassadarArticleAbiInstruction::LocalGet { local_index: 2 },
                TassadarArticleAbiInstruction::I32LoadHeapAtIndex {
                    pointer_local_index: 0,
                    index_local_index: 3,
                    stride_bytes: 4,
                },
                TassadarArticleAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarArticleAbiInstruction::LocalSet { local_index: 2 },
                TassadarArticleAbiInstruction::LocalGet { local_index: 3 },
                TassadarArticleAbiInstruction::I32Const { value: 1 },
                TassadarArticleAbiInstruction::BinaryOp {
                    op: TassadarStructuredControlBinaryOp::Add,
                },
                TassadarArticleAbiInstruction::LocalSet { local_index: 3 },
                TassadarArticleAbiInstruction::Jump { target_pc: 4 },
                TassadarArticleAbiInstruction::LocalGet { local_index: 2 },
                TassadarArticleAbiInstruction::Return,
            ],
            claim_boundary: fixture.claim_boundary.clone(),
        },
        TassadarArticleAbiFixtureId::UnsupportedFloatParam => {
            return Err(TassadarArticleAbiLoweringError::UnsupportedParamKind {
                fixture_id: String::from(fixture.fixture_id.as_str()),
                param_index: 0,
                kind: TassadarArticleAbiParamKind::F32,
            });
        }
        TassadarArticleAbiFixtureId::UnsupportedMultiResult => {
            return Err(TassadarArticleAbiLoweringError::UnsupportedResultKind {
                fixture_id: String::from(fixture.fixture_id.as_str()),
                kind: TassadarArticleAbiResultKind::MultiI32Pair,
            });
        }
    };

    program.validate().map_err(
        |error| TassadarArticleAbiLoweringError::InvalidLoweredProgram {
            fixture_id: String::from(fixture.fixture_id.as_str()),
            error,
        },
    )?;

    Ok(TassadarArticleAbiArtifact::new(fixture, program))
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TassadarArticleAbiLoweringError, lower_tassadar_article_abi_fixture};
    use psionic_ir::TassadarArticleAbiFixture;
    use psionic_runtime::{
        execute_tassadar_article_abi_program, tassadar_article_abi_heap_sum_invocation,
        tassadar_article_abi_scalar_invocation,
    };

    #[test]
    fn article_abi_fixtures_lower_and_match_runtime_truth() {
        let scalar =
            lower_tassadar_article_abi_fixture(&TassadarArticleAbiFixture::scalar_add_one())
                .expect("lower scalar");
        let scalar_execution = execute_tassadar_article_abi_program(
            &scalar.program,
            &tassadar_article_abi_scalar_invocation(),
        )
        .expect("execute scalar");
        assert_eq!(scalar_execution.returned_value, Some(42));

        let heap = lower_tassadar_article_abi_fixture(&TassadarArticleAbiFixture::heap_sum_i32())
            .expect("lower heap");
        let heap_execution = execute_tassadar_article_abi_program(
            &heap.program,
            &tassadar_article_abi_heap_sum_invocation(),
        )
        .expect("execute heap");
        assert_eq!(heap_execution.returned_value, Some(20));
    }

    #[test]
    fn article_abi_lowering_refuses_unsupported_shapes() {
        let float_error = lower_tassadar_article_abi_fixture(
            &TassadarArticleAbiFixture::unsupported_float_param(),
        )
        .expect_err("float param should refuse");
        assert!(matches!(
            float_error,
            TassadarArticleAbiLoweringError::UnsupportedParamKind { .. }
        ));

        let multi_result_error = lower_tassadar_article_abi_fixture(
            &TassadarArticleAbiFixture::unsupported_multi_result(),
        )
        .expect_err("multi result should refuse");
        assert!(matches!(
            multi_result_error,
            TassadarArticleAbiLoweringError::UnsupportedResultKind { .. }
        ));
    }
}
