use psionic_data::{
    DatasetKey, TassadarSequenceDatasetContract, TassadarSequenceDatasetError,
    TassadarSequenceExample, TassadarSequenceExampleMetadata, TassadarSequenceSplit,
    TokenizerDigest, TokenizerFamily,
};
use psionic_models::{TassadarTraceTokenizer, TokenizerBoundary};
use psionic_runtime::{
    TassadarCpuReferenceRunner, TassadarExecutionRefusal, TassadarProgram, TassadarProgramArtifact,
    TassadarProgramArtifactError, TassadarSudokuV0CorpusSplit, TassadarTraceAbi,
    TassadarWasmProfile, tassadar_sudoku_9x9_corpus, tassadar_sudoku_v0_corpus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable dataset reference for the first honest 4x4 trained-executor tokenized corpus.
pub const TASSADAR_SUDOKU_SEQUENCE_DATASET_REF: &str = "oa.tassadar.sudoku_v0.sequence";
/// Stable dataset reference for the real 9x9 Sudoku-class tokenized corpus.
pub const TASSADAR_SUDOKU_9X9_SEQUENCE_DATASET_REF: &str = "oa.tassadar.sudoku_9x9.sequence";

/// Workload selector for tokenized Tassadar executor datasets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSequenceWorkload {
    /// The original 4x4 Sudoku-v0 corpus.
    #[default]
    SudokuV0,
    /// The real 9x9 Sudoku-class corpus.
    Sudoku9x9,
}

impl TassadarSequenceWorkload {
    /// Returns the stable dataset reference for the workload.
    #[must_use]
    pub const fn dataset_ref(self) -> &'static str {
        match self {
            Self::SudokuV0 => TASSADAR_SUDOKU_SEQUENCE_DATASET_REF,
            Self::Sudoku9x9 => TASSADAR_SUDOKU_9X9_SEQUENCE_DATASET_REF,
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::SudokuV0 => "Tassadar Sudoku-v0 Tokenized Executor Sequences",
            Self::Sudoku9x9 => "Tassadar Sudoku 9x9 Tokenized Executor Sequences",
        }
    }

    fn profile(self) -> TassadarWasmProfile {
        match self {
            Self::SudokuV0 => TassadarWasmProfile::sudoku_v0_search_v1(),
            Self::Sudoku9x9 => TassadarWasmProfile::sudoku_9x9_search_v1(),
        }
    }

    fn trace_abi(self) -> TassadarTraceAbi {
        match self {
            Self::SudokuV0 => TassadarTraceAbi::sudoku_v0_search_v1(),
            Self::Sudoku9x9 => TassadarTraceAbi::sudoku_9x9_search_v1(),
        }
    }
}

/// Packaged CPU-reference tokenized dataset for Tassadar training.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSequenceDatasetBundle {
    /// Workload carried by the bundle.
    pub workload: TassadarSequenceWorkload,
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

/// Builds one canonical tokenized Tassadar dataset directly from CPU-reference traces.
pub fn build_tassadar_sequence_dataset(
    workload: TassadarSequenceWorkload,
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

    let profile = workload.profile();
    let trace_abi = workload.trace_abi();
    let examples = match workload {
        TassadarSequenceWorkload::SudokuV0 => tassadar_sudoku_v0_corpus()
            .into_iter()
            .map(|corpus_case| {
                build_sequence_example(
                    &tokenizer,
                    &profile,
                    &trace_abi,
                    map_split(corpus_case.split),
                    corpus_case.validation_case.case_id,
                    corpus_case.validation_case.program,
                    corpus_case.puzzle_cells,
                    corpus_case.given_count,
                )
            })
            .collect::<Result<Vec<_>, TassadarSequenceEvalError>>()?,
        TassadarSequenceWorkload::Sudoku9x9 => tassadar_sudoku_9x9_corpus()
            .into_iter()
            .map(|corpus_case| {
                build_sequence_example(
                    &tokenizer,
                    &profile,
                    &trace_abi,
                    map_split(corpus_case.split),
                    corpus_case.validation_case.case_id,
                    corpus_case.validation_case.program,
                    corpus_case.puzzle_cells,
                    corpus_case.given_count,
                )
            })
            .collect::<Result<Vec<_>, TassadarSequenceEvalError>>()?,
    };

    let dataset = TassadarSequenceDatasetContract::from_examples(
        DatasetKey::new(workload.dataset_ref(), version),
        workload.display_name(),
        tokenizer_digest.clone(),
        vocabulary_digest.clone(),
        examples,
    )?;

    Ok(TassadarSequenceDatasetBundle {
        workload,
        tokenizer_digest,
        vocabulary_digest,
        dataset,
    })
}

/// Builds the canonical tokenized 4x4 Sudoku-v0 dataset directly from CPU-reference traces.
pub fn build_tassadar_sudoku_v0_sequence_dataset(
    version: &str,
) -> Result<TassadarSequenceDatasetBundle, TassadarSequenceEvalError> {
    build_tassadar_sequence_dataset(TassadarSequenceWorkload::SudokuV0, version)
}

/// Builds the canonical tokenized real 9x9 Sudoku-class dataset directly from CPU-reference traces.
pub fn build_tassadar_sudoku_9x9_sequence_dataset(
    version: &str,
) -> Result<TassadarSequenceDatasetBundle, TassadarSequenceEvalError> {
    build_tassadar_sequence_dataset(TassadarSequenceWorkload::Sudoku9x9, version)
}

fn build_sequence_example(
    tokenizer: &TassadarTraceTokenizer,
    profile: &TassadarWasmProfile,
    trace_abi: &TassadarTraceAbi,
    split: TassadarSequenceSplit,
    case_id: String,
    program: TassadarProgram,
    puzzle_cells: Vec<i32>,
    given_count: usize,
) -> Result<TassadarSequenceExample, TassadarSequenceEvalError> {
    let artifact = TassadarProgramArtifact::fixture_reference(
        format!("tassadar-token-sequence-{case_id}"),
        profile,
        trace_abi,
        program.clone(),
    )?;
    let execution = TassadarCpuReferenceRunner::for_program(&program)?.execute(&program)?;
    let tokenized = tokenizer.tokenize_program_and_execution(&program, &execution);

    Ok(TassadarSequenceExample {
        sequence_id: format!("tassadar.sequence.{case_id}"),
        token_ids: tokenized.token_ids_u32(),
        metadata: TassadarSequenceExampleMetadata {
            case_id: case_id.clone(),
            puzzle_digest: stable_digest(b"psionic_tassadar_sequence_puzzle|", &puzzle_cells),
            program_id: program.program_id.clone(),
            program_digest: program.program_digest(),
            program_artifact_digest: artifact.artifact_digest.clone(),
            trace_digest: execution.trace_digest(),
            behavior_digest: execution.behavior_digest(),
            split,
            given_count: given_count as u32,
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
    use super::{
        TASSADAR_SUDOKU_9X9_SEQUENCE_DATASET_REF, TASSADAR_SUDOKU_SEQUENCE_DATASET_REF,
        build_tassadar_sudoku_9x9_sequence_dataset, build_tassadar_sudoku_v0_sequence_dataset,
    };
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

    #[test]
    fn sudoku_9x9_sequence_dataset_tracks_stable_splits_and_lineage()
    -> Result<(), Box<dyn std::error::Error>> {
        let bundle = build_tassadar_sudoku_9x9_sequence_dataset("scale-v0")?;

        assert_eq!(
            bundle.dataset.storage_key(),
            format!("{TASSADAR_SUDOKU_9X9_SEQUENCE_DATASET_REF}@scale-v0")
        );
        assert_eq!(
            bundle
                .dataset
                .split_examples(TassadarSequenceSplit::Train)
                .len(),
            2
        );
        assert_eq!(
            bundle
                .dataset
                .split_examples(TassadarSequenceSplit::Validation)
                .len(),
            1
        );
        assert_eq!(
            bundle
                .dataset
                .split_examples(TassadarSequenceSplit::Test)
                .len(),
            1
        );
        assert!(
            bundle
                .dataset
                .examples
                .iter()
                .all(|example| example.metadata.target_token_count > 0)
        );
        Ok(())
    }
}
