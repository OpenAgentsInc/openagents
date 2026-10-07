use std::collections::BTreeSet;

use psionic_data::{
    DatasetPackingMode, DatasetPackingPlan, DatasetPackingPolicy, TassadarSequenceDatasetContract,
    TassadarSequenceDatasetError, TassadarSequenceSplit,
};
use psionic_eval::{
    TassadarSequenceDatasetBundle, TassadarSequenceEvalError, TassadarSequenceWorkload,
    build_tassadar_sequence_dataset, build_tassadar_sequence_dataset_with_trace_family,
};
use psionic_models::{
    TassadarExecutorLongTraceContract, TassadarExecutorTrainableSurface,
    TassadarSequenceTraceFamily, TassadarStructuralSupervisionCoverage, TassadarTraceTokenizer,
    TokenId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarExecutorStructuralSupervisionConfig, TassadarExecutorTeacherForcedTrainingStrategy,
};

fn default_structural_supervision_config() -> TassadarExecutorStructuralSupervisionConfig {
    TassadarExecutorStructuralSupervisionConfig::next_token_only()
}

fn default_long_trace_contract() -> TassadarExecutorLongTraceContract {
    TassadarExecutorLongTraceContract::FlatPrefixFullForward
}

fn default_trace_family() -> TassadarSequenceTraceFamily {
    TassadarSequenceTraceFamily::SequentialCpuReference
}

fn default_train_split_scope() -> Vec<TassadarSequenceSplit> {
    vec![TassadarSequenceSplit::Train]
}

fn train_split_scope_is_train_only(scope: &[TassadarSequenceSplit]) -> bool {
    scope == [TassadarSequenceSplit::Train]
}

/// Frozen train/eval packing contract for the tokenized Sudoku-v0 executor corpus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarSequenceTrainingManifest {
    /// Stable dataset storage key.
    pub dataset_storage_key: String,
    /// Stable digest over the dataset contract.
    pub dataset_digest: String,
    /// Stable digest over the tokenizer contract.
    pub tokenizer_digest: String,
    /// Stable vocabulary digest.
    pub vocabulary_digest: String,
    /// Active trainable surface for the run that materialized this manifest.
    #[serde(default = "default_trainable_surface")]
    pub trainable_surface: TassadarExecutorTrainableSurface,
    /// Explicit teacher-forced strategy bound to this manifest.
    #[serde(
        default = "default_teacher_forced_training_strategy",
        skip_serializing_if = "teacher_forced_training_strategy_is_full_forward_window"
    )]
    pub teacher_forced_training_strategy: TassadarExecutorTeacherForcedTrainingStrategy,
    /// Explicit long-trace contract bound to this manifest.
    #[serde(
        default = "default_long_trace_contract",
        skip_serializing_if = "long_trace_contract_is_flat_prefix"
    )]
    pub long_trace_contract: TassadarExecutorLongTraceContract,
    /// Explicit symbolic trace family bound to this manifest.
    #[serde(
        default = "default_trace_family",
        skip_serializing_if = "trace_family_is_sequential_cpu_reference"
    )]
    pub trace_family: TassadarSequenceTraceFamily,
    /// Dataset splits presented to the trainer as optimization data.
    #[serde(
        default = "default_train_split_scope",
        skip_serializing_if = "train_split_scope_is_train_only"
    )]
    pub train_split_scope: Vec<TassadarSequenceSplit>,
    /// Explicit structural-supervision weighting profile frozen into the run.
    #[serde(
        default = "default_structural_supervision_config",
        skip_serializing_if = "structural_supervision_config_is_next_token_only"
    )]
    pub structural_supervision: TassadarExecutorStructuralSupervisionConfig,
    /// Aggregate structural-supervision coverage across dataset splits.
    pub structural_supervision_inventory: TassadarSequenceStructuralSupervisionInventory,
    /// Shared packing policy used for the first training run.
    pub packing_policy: DatasetPackingPolicy,
    /// Packed train split.
    pub train_plan: DatasetPackingPlan,
    /// Packed validation split.
    pub validation_plan: DatasetPackingPlan,
    /// Packed test split.
    pub test_plan: DatasetPackingPlan,
    /// Stable digest over the full frozen training manifest.
    pub manifest_digest: String,
}

impl TassadarSequenceTrainingManifest {
    fn new(
        dataset: &TassadarSequenceDatasetContract,
        tokenizer_digest: &str,
        vocabulary_digest: &str,
        trainable_surface: TassadarExecutorTrainableSurface,
        teacher_forced_training_strategy: TassadarExecutorTeacherForcedTrainingStrategy,
        long_trace_contract: TassadarExecutorLongTraceContract,
        trace_family: TassadarSequenceTraceFamily,
        train_split_scope: Vec<TassadarSequenceSplit>,
        structural_supervision: TassadarExecutorStructuralSupervisionConfig,
        structural_supervision_inventory: TassadarSequenceStructuralSupervisionInventory,
        packing_policy: DatasetPackingPolicy,
        train_plan: DatasetPackingPlan,
        validation_plan: DatasetPackingPlan,
        test_plan: DatasetPackingPlan,
    ) -> Self {
        let mut manifest = Self {
            dataset_storage_key: dataset.storage_key(),
            dataset_digest: dataset.stable_digest(),
            tokenizer_digest: tokenizer_digest.to_string(),
            vocabulary_digest: vocabulary_digest.to_string(),
            trainable_surface,
            teacher_forced_training_strategy,
            long_trace_contract,
            trace_family,
            train_split_scope,
            structural_supervision,
            structural_supervision_inventory,
            packing_policy,
            train_plan,
            validation_plan,
            test_plan,
            manifest_digest: String::new(),
        };
        manifest.manifest_digest =
            stable_digest(b"psionic_tassadar_sequence_training_manifest|", &manifest);
        manifest
    }
}

/// Error returned while freezing the tokenized Tassadar training dataset.
#[derive(Debug, Error)]
pub enum TassadarSequenceTrainingError {
    /// Dataset generation failed.
    #[error(transparent)]
    SequenceEval(#[from] TassadarSequenceEvalError),
    /// Dataset packing or validation failed.
    #[error(transparent)]
    Dataset(#[from] TassadarSequenceDatasetError),
}

fn default_trainable_surface() -> TassadarExecutorTrainableSurface {
    TassadarExecutorTrainableSurface::OutputHeadOnly
}

fn default_teacher_forced_training_strategy() -> TassadarExecutorTeacherForcedTrainingStrategy {
    TassadarExecutorTeacherForcedTrainingStrategy::FullForwardWindow
}

fn teacher_forced_training_strategy_is_full_forward_window(
    strategy: &TassadarExecutorTeacherForcedTrainingStrategy,
) -> bool {
    *strategy == TassadarExecutorTeacherForcedTrainingStrategy::FullForwardWindow
}

fn long_trace_contract_is_flat_prefix(contract: &TassadarExecutorLongTraceContract) -> bool {
    *contract == TassadarExecutorLongTraceContract::FlatPrefixFullForward
}

fn trace_family_is_sequential_cpu_reference(family: &TassadarSequenceTraceFamily) -> bool {
    *family == TassadarSequenceTraceFamily::SequentialCpuReference
}

fn structural_supervision_config_is_next_token_only(
    config: &TassadarExecutorStructuralSupervisionConfig,
) -> bool {
    *config == TassadarExecutorStructuralSupervisionConfig::next_token_only()
}

/// Aggregate structural-supervision coverage for one split.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSequenceStructuralSupervisionSplitCoverage {
    /// Split represented by the summary.
    pub split: TassadarSequenceSplit,
    /// Number of examples inside the split.
    pub example_count: u32,
    /// Aggregate coverage across all target tokens in the split.
    pub coverage: TassadarStructuralSupervisionCoverage,
}

/// Aggregate structural-supervision coverage across the frozen dataset splits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSequenceStructuralSupervisionInventory {
    /// Training split coverage.
    pub train: TassadarSequenceStructuralSupervisionSplitCoverage,
    /// Validation split coverage.
    pub validation: TassadarSequenceStructuralSupervisionSplitCoverage,
    /// Test split coverage.
    pub test: TassadarSequenceStructuralSupervisionSplitCoverage,
}

/// Builds the frozen sequence dataset plus generic packing plans for one Tassadar workload.
pub fn build_tassadar_sequence_training_manifest(
    workload: TassadarSequenceWorkload,
    version: &str,
    trainable_surface: TassadarExecutorTrainableSurface,
    teacher_forced_training_strategy: TassadarExecutorTeacherForcedTrainingStrategy,
    long_trace_contract: TassadarExecutorLongTraceContract,
    structural_supervision: TassadarExecutorStructuralSupervisionConfig,
) -> Result<TassadarSequenceTrainingManifest, TassadarSequenceTrainingError> {
    let bundle = build_tassadar_sequence_dataset(workload, version)?;
    build_tassadar_sequence_training_manifest_from_bundle(
        &bundle,
        trainable_surface,
        teacher_forced_training_strategy,
        long_trace_contract,
        structural_supervision,
    )
}

/// Builds one frozen sequence dataset plus generic packing plans for a specific trace family.
pub fn build_tassadar_sequence_training_manifest_with_trace_family(
    workload: TassadarSequenceWorkload,
    version: &str,
    trace_family: TassadarSequenceTraceFamily,
    trainable_surface: TassadarExecutorTrainableSurface,
    teacher_forced_training_strategy: TassadarExecutorTeacherForcedTrainingStrategy,
    long_trace_contract: TassadarExecutorLongTraceContract,
    structural_supervision: TassadarExecutorStructuralSupervisionConfig,
) -> Result<TassadarSequenceTrainingManifest, TassadarSequenceTrainingError> {
    let bundle =
        build_tassadar_sequence_dataset_with_trace_family(workload, version, trace_family)?;
    build_tassadar_sequence_training_manifest_from_bundle(
        &bundle,
        trainable_surface,
        teacher_forced_training_strategy,
        long_trace_contract,
        structural_supervision,
    )
}

/// Builds one frozen sequence dataset plus generic packing plans for a specific trace family and
/// explicit train split scope.
pub fn build_tassadar_sequence_training_manifest_with_trace_family_and_train_split_scope(
    workload: TassadarSequenceWorkload,
    version: &str,
    trace_family: TassadarSequenceTraceFamily,
    train_split_scope: &[TassadarSequenceSplit],
    trainable_surface: TassadarExecutorTrainableSurface,
    teacher_forced_training_strategy: TassadarExecutorTeacherForcedTrainingStrategy,
    long_trace_contract: TassadarExecutorLongTraceContract,
    structural_supervision: TassadarExecutorStructuralSupervisionConfig,
) -> Result<TassadarSequenceTrainingManifest, TassadarSequenceTrainingError> {
    let bundle =
        build_tassadar_sequence_dataset_with_trace_family(workload, version, trace_family)?;
    build_tassadar_sequence_training_manifest_from_bundle_with_train_split_scope(
        &bundle,
        train_split_scope,
        trainable_surface,
        teacher_forced_training_strategy,
        long_trace_contract,
        structural_supervision,
    )
}

/// Builds one frozen sequence training manifest from a pre-built dataset bundle.
pub fn build_tassadar_sequence_training_manifest_from_bundle(
    bundle: &TassadarSequenceDatasetBundle,
    trainable_surface: TassadarExecutorTrainableSurface,
    teacher_forced_training_strategy: TassadarExecutorTeacherForcedTrainingStrategy,
    long_trace_contract: TassadarExecutorLongTraceContract,
    structural_supervision: TassadarExecutorStructuralSupervisionConfig,
) -> Result<TassadarSequenceTrainingManifest, TassadarSequenceTrainingError> {
    build_tassadar_sequence_training_manifest_from_bundle_with_train_split_scope(
        bundle,
        default_train_split_scope().as_slice(),
        trainable_surface,
        teacher_forced_training_strategy,
        long_trace_contract,
        structural_supervision,
    )
}

/// Builds one frozen sequence training manifest from a pre-built dataset bundle and explicit
/// train split scope.
pub fn build_tassadar_sequence_training_manifest_from_bundle_with_train_split_scope(
    bundle: &TassadarSequenceDatasetBundle,
    train_split_scope: &[TassadarSequenceSplit],
    trainable_surface: TassadarExecutorTrainableSurface,
    teacher_forced_training_strategy: TassadarExecutorTeacherForcedTrainingStrategy,
    long_trace_contract: TassadarExecutorLongTraceContract,
    structural_supervision: TassadarExecutorStructuralSupervisionConfig,
) -> Result<TassadarSequenceTrainingManifest, TassadarSequenceTrainingError> {
    let dataset = &bundle.dataset;
    let tokenizer = TassadarTraceTokenizer::new();
    let max_tokens = dataset
        .examples
        .iter()
        .map(|example| example.token_ids.len() as u32)
        .max()
        .unwrap_or(1);
    let packing_policy = DatasetPackingPolicy::new(
        DatasetPackingMode::BatchByTokenBudget,
        max_tokens.max(1),
        max_tokens.saturating_mul(4).max(1),
        4,
    );
    let train_plan = packing_policy
        .plan(combined_train_sequence_descriptors(dataset, train_split_scope).as_slice())
        .map_err(TassadarSequenceDatasetError::from)?;
    let validation_plan =
        dataset.packing_plan(TassadarSequenceSplit::Validation, &packing_policy)?;
    let test_plan = dataset.packing_plan(TassadarSequenceSplit::Test, &packing_policy)?;
    let structural_supervision_inventory =
        build_structural_supervision_inventory(&tokenizer, dataset);
    Ok(TassadarSequenceTrainingManifest::new(
        dataset,
        bundle.tokenizer_digest.stable_digest().as_str(),
        bundle.vocabulary_digest.as_str(),
        trainable_surface,
        teacher_forced_training_strategy,
        long_trace_contract,
        bundle.trace_family,
        normalize_train_split_scope(train_split_scope),
        structural_supervision,
        structural_supervision_inventory,
        packing_policy,
        train_plan,
        validation_plan,
        test_plan,
    ))
}

fn normalize_train_split_scope(train_split_scope: &[TassadarSequenceSplit]) -> Vec<TassadarSequenceSplit> {
    let mut normalized = Vec::new();
    let mut seen = BTreeSet::new();
    for split in train_split_scope.iter().copied() {
        if seen.insert(split) {
            normalized.push(split);
        }
    }
    if normalized.is_empty() {
        default_train_split_scope()
    } else {
        normalized
    }
}

fn combined_train_sequence_descriptors(
    dataset: &TassadarSequenceDatasetContract,
    train_split_scope: &[TassadarSequenceSplit],
) -> Vec<psionic_data::DatasetSequenceDescriptor> {
    let mut descriptors = Vec::new();
    let mut seen = BTreeSet::new();
    for split in normalize_train_split_scope(train_split_scope) {
        for descriptor in dataset.sequence_descriptors(split) {
            if seen.insert(descriptor.sequence_id.clone()) {
                descriptors.push(descriptor);
            }
        }
    }
    descriptors
}

/// Builds the frozen sequence dataset plus generic packing plans for the 4x4 training run.
pub fn build_tassadar_sudoku_v0_sequence_training_manifest(
    version: &str,
) -> Result<TassadarSequenceTrainingManifest, TassadarSequenceTrainingError> {
    build_tassadar_sequence_training_manifest(
        TassadarSequenceWorkload::SudokuV0,
        version,
        TassadarExecutorTrainableSurface::OutputHeadOnly,
        TassadarExecutorTeacherForcedTrainingStrategy::FullForwardWindow,
        TassadarExecutorLongTraceContract::FlatPrefixFullForward,
        TassadarExecutorStructuralSupervisionConfig::next_token_only(),
    )
}

/// Builds the frozen sequence dataset plus generic packing plans for the 9x9 scale-out run.
pub fn build_tassadar_sudoku_9x9_sequence_training_manifest(
    version: &str,
) -> Result<TassadarSequenceTrainingManifest, TassadarSequenceTrainingError> {
    build_tassadar_sequence_training_manifest(
        TassadarSequenceWorkload::Sudoku9x9,
        version,
        TassadarExecutorTrainableSurface::OutputHeadOnly,
        TassadarExecutorTeacherForcedTrainingStrategy::FullForwardWindow,
        TassadarExecutorLongTraceContract::FlatPrefixFullForward,
        TassadarExecutorStructuralSupervisionConfig::next_token_only(),
    )
}

fn build_structural_supervision_inventory(
    tokenizer: &TassadarTraceTokenizer,
    dataset: &TassadarSequenceDatasetContract,
) -> TassadarSequenceStructuralSupervisionInventory {
    TassadarSequenceStructuralSupervisionInventory {
        train: build_split_structural_supervision_coverage(
            tokenizer,
            dataset,
            TassadarSequenceSplit::Train,
        ),
        validation: build_split_structural_supervision_coverage(
            tokenizer,
            dataset,
            TassadarSequenceSplit::Validation,
        ),
        test: build_split_structural_supervision_coverage(
            tokenizer,
            dataset,
            TassadarSequenceSplit::Test,
        ),
    }
}

fn build_split_structural_supervision_coverage(
    tokenizer: &TassadarTraceTokenizer,
    dataset: &TassadarSequenceDatasetContract,
    split: TassadarSequenceSplit,
) -> TassadarSequenceStructuralSupervisionSplitCoverage {
    let mut coverage = TassadarStructuralSupervisionCoverage::default();
    let mut example_count = 0_u32;
    for example in dataset
        .examples
        .iter()
        .filter(|example| example.metadata.split == split)
    {
        let tokens = example
            .token_ids
            .iter()
            .map(|token| TokenId(*token))
            .collect::<Vec<_>>();
        coverage.accumulate(&tokenizer.summarize_target_structural_supervision(
            tokens.as_slice(),
            example.metadata.prompt_token_count as usize,
        ));
        example_count = example_count.saturating_add(1);
    }
    TassadarSequenceStructuralSupervisionSplitCoverage {
        split,
        example_count,
        coverage,
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar sequence training value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_sudoku_9x9_sequence_training_manifest,
        build_tassadar_sudoku_v0_sequence_training_manifest,
    };

    #[test]
    fn training_manifest_freezes_split_packing_for_sudoku_v0_sequences()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest = build_tassadar_sudoku_v0_sequence_training_manifest("train-v0")?;

        assert_eq!(manifest.train_plan.total_source_sequences, 4);
        assert_eq!(manifest.validation_plan.total_source_sequences, 2);
        assert_eq!(manifest.test_plan.total_source_sequences, 2);
        assert!(
            manifest
                .structural_supervision_inventory
                .train
                .coverage
                .total_target_token_count
                > 0
        );
        assert!(
            manifest
                .structural_supervision_inventory
                .train
                .coverage
                .instruction_pointer_token_count
                > 0
        );
        assert!(!manifest.tokenizer_digest.is_empty());
        assert!(!manifest.vocabulary_digest.is_empty());
        assert!(!manifest.manifest_digest.is_empty());
        Ok(())
    }

    #[test]
    fn training_manifest_freezes_split_packing_for_sudoku_9x9_sequences()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest = build_tassadar_sudoku_9x9_sequence_training_manifest("scale-v0")?;

        assert_eq!(manifest.train_plan.total_source_sequences, 2);
        assert_eq!(manifest.validation_plan.total_source_sequences, 1);
        assert_eq!(manifest.test_plan.total_source_sequences, 1);
        assert!(
            manifest
                .structural_supervision_inventory
                .validation
                .coverage
                .total_target_token_count
                > 0
        );
        assert!(!manifest.manifest_digest.is_empty());
        Ok(())
    }
}
