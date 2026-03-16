use psionic_data::{
    DatasetKey, TassadarSequenceDatasetContract, TassadarSequenceDatasetError,
    TassadarSequenceExample, TassadarSequenceExampleMetadata, TassadarSequenceSplit,
    TokenizerDigest, TokenizerFamily,
};
use psionic_models::{TassadarTraceTokenizer, TokenizerBoundary};
use psionic_runtime::{
    TassadarCpuReferenceRunner, TassadarExecutionRefusal, TassadarProgramArtifact,
    TassadarProgramArtifactError, TassadarSudokuV0CorpusSplit, TassadarTraceAbi,
    TassadarWasmProfile, tassadar_sudoku_v0_corpus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable dataset reference for the first honest trained-executor tokenized corpus.
pub const TASSADAR_SUDOKU_SEQUENCE_DATASET_REF: &str = "oa.tassadar.sudoku_v0.sequence";

/// Packaged CPU-reference tokenized dataset for Tassadar Sudoku-v0 training.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSequenceDatasetBundle {
    /// Tokenizer used to produce the token ids.
    pub tokenizer_digest: TokenizerDigest,
    /// Stable vocabulary digest for the symbolic/byte token space.
    pub vocabulary_digest: String,
    /// Full versioned dataset contract.
    pub dataset: TassadarSequenceDatasetContract,
}

/// Sequence-dataset build failure for the trained-executor lane.
#[derive(Debug, Error)]
pub enum TassadarSequenceEvalError {
    /// Dataset contract validation failed.
    #[error(transparent)]
    Dataset(#[from] TassadarSequenceDatasetError),
    /// CPU-reference runner refused one program.
    #[error(transparent)]
    Execution(#[from] TassadarExecutionRefusal),
    /// Program-artifact construction failed.
    #[error(transparent)]
    ProgramArtifact(#[from] TassadarProgramArtifactError),
}

/// Builds the canonical tokenized Sudoku-v0 dataset directly from CPU-reference traces.
pub fn build_tassadar_sudoku_v0_sequence_dataset(
    version: &str,
) -> Result<TassadarSequenceDatasetBundle, TassadarSequenceEvalError> {
    let tokenizer = TassadarTraceTokenizer::new();
    let vocabulary_digest = tokenizer.stable_digest();
    let tokenizer_digest = TokenizerDigest::new(
        TokenizerFamily::Custom,
        stable_digest(
            b"psionic_tassadar_sequence_tokenizer_digest|",
            &(vocabulary_digest.as_str(), tokenizer.vocabulary().len()),
        ),
        tokenizer.vocabulary().len() as u32,
    )
    .with_special_tokens_digest(vocabulary_digest.clone());

    let profile = TassadarWasmProfile::sudoku_v0_search_v1();
    let trace_abi = TassadarTraceAbi::sudoku_v0_search_v1();
    let examples = tassadar_sudoku_v0_corpus()
        .into_iter()
        .map(|corpus_case| {
            let split = map_split(corpus_case.split);
            let case = corpus_case.validation_case;
            let artifact = TassadarProgramArtifact::fixture_reference(
                format!("tassadar-token-sequence-{}", case.case_id),
                &profile,
                &trace_abi,
                case.program.clone(),
            )?;
            let execution =
                TassadarCpuReferenceRunner::for_program(&case.program)?.execute(&case.program)?;
            let tokenized = tokenizer.tokenize_program_and_execution(&case.program, &execution);
            let case_id = case.case_id.clone();
            Ok(TassadarSequenceExample {
                sequence_id: format!("tassadar.sequence.{case_id}"),
                token_ids: tokenized.token_ids_u32(),
                metadata: TassadarSequenceExampleMetadata {
                    case_id,
                    puzzle_digest: stable_digest(
                        b"psionic_tassadar_sequence_puzzle|",
                        &corpus_case.puzzle_cells,
                    ),
                    program_id: case.program.program_id.clone(),
                    program_digest: case.program.program_digest(),
                    program_artifact_digest: artifact.artifact_digest.clone(),
                    trace_digest: execution.trace_digest(),
                    behavior_digest: execution.behavior_digest(),
                    split,
                    given_count: corpus_case.given_count as u32,
                    prompt_token_count: tokenized.prompt_token_count as u32,
                    target_token_count: tokenized.target_token_count as u32,
                    total_token_count: tokenized.sequence.len() as u32,
                    trace_step_count: execution.steps.len() as u32,
                    backward_branch_count: execution
                        .steps
                        .iter()
                        .filter(|step| step.next_pc <= step.pc)
                        .count() as u32,
                    max_stack_depth: execution
                        .steps
                        .iter()
                        .map(|step| step.stack_after.len().max(step.stack_before.len()) as u32)
                        .max()
                        .unwrap_or(0),
                },
            })
        })
        .collect::<Result<Vec<_>, TassadarSequenceEvalError>>()?;

    let dataset = TassadarSequenceDatasetContract::from_examples(
        DatasetKey::new(TASSADAR_SUDOKU_SEQUENCE_DATASET_REF, version),
        "Tassadar Sudoku-v0 Tokenized Executor Sequences",
        tokenizer_digest.clone(),
        vocabulary_digest.clone(),
        examples,
    )?;

    Ok(TassadarSequenceDatasetBundle {
        tokenizer_digest,
        vocabulary_digest,
        dataset,
    })
}

fn map_split(split: TassadarSudokuV0CorpusSplit) -> TassadarSequenceSplit {
    match split {
        TassadarSudokuV0CorpusSplit::Train => TassadarSequenceSplit::Train,
        TassadarSudokuV0CorpusSplit::Validation => TassadarSequenceSplit::Validation,
        TassadarSudokuV0CorpusSplit::Test => TassadarSequenceSplit::Test,
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("Tassadar sequence eval value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TASSADAR_SUDOKU_SEQUENCE_DATASET_REF, build_tassadar_sudoku_v0_sequence_dataset};
    use psionic_data::TassadarSequenceSplit;

    #[test]
    fn sudoku_v0_sequence_dataset_tracks_stable_splits_and_lineage()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_tassadar_sudoku_v0_sequence_dataset("train-v0")?;

        assert_eq!(
            bundle.dataset.storage_key(),
            format!("{TASSADAR_SUDOKU_SEQUENCE_DATASET_REF}@train-v0")
        );
        assert_eq!(
            bundle
                .dataset
                .split_examples(TassadarSequenceSplit::Train)
                .len(),
            4
        );
        assert_eq!(
            bundle
                .dataset
                .split_examples(TassadarSequenceSplit::Validation)
                .len(),
            2
        );
        assert_eq!(
            bundle
                .dataset
                .split_examples(TassadarSequenceSplit::Test)
                .len(),
            2
        );
        assert!(
            bundle
                .dataset
                .examples
                .iter()
                .all(|example| example.metadata.prompt_token_count > 0)
        );
        assert!(
            bundle
                .dataset
                .examples
                .iter()
                .all(|example| !example.metadata.program_digest.is_empty())
        );
        Ok(())
    }
}
