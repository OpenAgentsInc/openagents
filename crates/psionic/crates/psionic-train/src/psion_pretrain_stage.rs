use std::collections::{BTreeMap, BTreeSet};

use psionic_data::{
    DatasetIterationMode, DatasetShardOrdering, DatasetSplitKind, PsionTokenizedCorpusManifest,
};
use psionic_models::PsionCompactDecoderDescriptor;
use psionic_runtime::{DeliveredExecutionContext, TrainingCheckpointReference};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{PsionSamplingPolicyManifest, TrainingStageKind};

/// Stable schema version for the first Psion pretrain-stage config.
pub const PSION_PRETRAIN_STAGE_CONFIG_SCHEMA_VERSION: &str = "psion.pretrain_stage_config.v1";
/// Stable schema version for the first Psion pretrain-stage receipt.
pub const PSION_PRETRAIN_STAGE_RECEIPT_SCHEMA_VERSION: &str = "psion.pretrain_stage_receipt.v1";

/// Objective family admitted by the first Psion pretrain stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPretrainObjectiveKind {
    /// Standard next-token prediction over the curated corpus.
    NextTokenPrediction,
}

/// Loss-normalization posture admitted by the first Psion pretrain stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionPretrainLossNormalization {
    /// Normalize loss by target-token count.
    ByTargetToken,
}

/// Objective config bound to tokenizer, model, and dataset identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainObjectiveConfig {
    /// Objective family.
    pub objective_kind: PsionPretrainObjectiveKind,
    /// Loss normalization posture.
    pub loss_normalization: PsionPretrainLossNormalization,
    /// Optional label-smoothing coefficient in basis points.
    pub label_smoothing_bps: u32,
    /// Stable tokenizer-binding digest from the model descriptor.
    pub tokenizer_binding_digest: String,
    /// Stable dataset identity from the tokenized corpus.
    pub dataset_identity: String,
    /// Explicit context length bound to the model descriptor.
    pub max_context_tokens: usize,
}

/// Declared pretrain stage for one Psion run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainStageConfig {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Stage kind.
    pub stage_kind: TrainingStageKind,
    /// Stable model id.
    pub model_id: String,
    /// Stable model-descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable tokenizer-binding digest.
    pub tokenizer_binding_digest: String,
    /// Stable dataset identity.
    pub dataset_identity: String,
    /// Stable sampling-policy identifier.
    pub sampling_policy_id: String,
    /// Stable sampling-policy version.
    pub sampling_policy_version: String,
    /// Objective config bound to tokenizer and dataset identity.
    pub objective_config: PsionPretrainObjectiveConfig,
}

impl PsionPretrainStageConfig {
    /// Creates one declared pretrain stage config and validates it against the Psion artifacts.
    pub fn new(
        run_id: impl Into<String>,
        stage_id: impl Into<String>,
        objective_config: PsionPretrainObjectiveConfig,
        model_descriptor: &PsionCompactDecoderDescriptor,
        tokenized_corpus: &PsionTokenizedCorpusManifest,
        sampling_policy: &PsionSamplingPolicyManifest,
    ) -> Result<Self, PsionPretrainStageError> {
        let config = Self {
            schema_version: String::from(PSION_PRETRAIN_STAGE_CONFIG_SCHEMA_VERSION),
            run_id: run_id.into(),
            stage_id: stage_id.into(),
            stage_kind: TrainingStageKind::Pretrain,
            model_id: model_descriptor.model.model_id.clone(),
            model_descriptor_digest: model_descriptor.stable_digest(),
            tokenizer_binding_digest: model_descriptor.tokenizer_binding.stable_digest(),
            dataset_identity: tokenized_corpus
                .replay_contract
                .stable_dataset_identity
                .clone(),
            sampling_policy_id: sampling_policy.policy_id.clone(),
            sampling_policy_version: sampling_policy.policy_version.clone(),
            objective_config,
        };
        config.validate_against_inputs(model_descriptor, tokenized_corpus, sampling_policy)?;
        Ok(config)
    }

    /// Validates the config against model, dataset, and sampling-policy inputs.
    pub fn validate_against_inputs(
        &self,
        model_descriptor: &PsionCompactDecoderDescriptor,
        tokenized_corpus: &PsionTokenizedCorpusManifest,
        sampling_policy: &PsionSamplingPolicyManifest,
    ) -> Result<(), PsionPretrainStageError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "pretrain_stage_config.schema_version",
        )?;
        if self.schema_version != PSION_PRETRAIN_STAGE_CONFIG_SCHEMA_VERSION {
            return Err(PsionPretrainStageError::SchemaVersionMismatch {
                expected: String::from(PSION_PRETRAIN_STAGE_CONFIG_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(self.run_id.as_str(), "pretrain_stage_config.run_id")?;
        ensure_nonempty(self.stage_id.as_str(), "pretrain_stage_config.stage_id")?;
        if self.stage_kind != TrainingStageKind::Pretrain {
            return Err(PsionPretrainStageError::StageKindMismatch {
                expected: TrainingStageKind::Pretrain,
                actual: self.stage_kind,
            });
        }
        check_string_match(
            self.model_id.as_str(),
            model_descriptor.model.model_id.as_str(),
            "model_id",
        )?;
        check_string_match(
            self.model_descriptor_digest.as_str(),
            model_descriptor.stable_digest().as_str(),
            "model_descriptor_digest",
        )?;
        check_string_match(
            self.tokenizer_binding_digest.as_str(),
            model_descriptor.tokenizer_binding.stable_digest().as_str(),
            "tokenizer_binding_digest",
        )?;
        check_string_match(
            self.dataset_identity.as_str(),
            tokenized_corpus
                .replay_contract
                .stable_dataset_identity
                .as_str(),
            "dataset_identity",
        )?;
        check_string_match(
            self.dataset_identity.as_str(),
            sampling_policy.dataset_identity.as_str(),
            "dataset_identity",
        )?;
        check_string_match(
            self.sampling_policy_id.as_str(),
            sampling_policy.policy_id.as_str(),
            "sampling_policy_id",
        )?;
        check_string_match(
            self.sampling_policy_version.as_str(),
            sampling_policy.policy_version.as_str(),
            "sampling_policy_version",
        )?;
        self.validate_objective_config(model_descriptor, tokenized_corpus)?;
        Ok(())
    }

    fn validate_objective_config(
        &self,
        model_descriptor: &PsionCompactDecoderDescriptor,
        tokenized_corpus: &PsionTokenizedCorpusManifest,
    ) -> Result<(), PsionPretrainStageError> {
        check_bps(
            self.objective_config.label_smoothing_bps,
            "objective_config.label_smoothing_bps",
        )?;
        check_string_match(
            self.objective_config.tokenizer_binding_digest.as_str(),
            model_descriptor.tokenizer_binding.stable_digest().as_str(),
            "objective_config.tokenizer_binding_digest",
        )?;
        check_string_match(
            self.objective_config.dataset_identity.as_str(),
            tokenized_corpus
                .replay_contract
                .stable_dataset_identity
                .as_str(),
            "objective_config.dataset_identity",
        )?;
        if self.objective_config.max_context_tokens != model_descriptor.config.max_context {
            return Err(PsionPretrainStageError::ContextLengthMismatch {
                expected: model_descriptor.config.max_context,
                actual: self.objective_config.max_context_tokens,
            });
        }
        Ok(())
    }
}

/// One source-family-aware report row for one split.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainSourceFamilyReportRow {
    /// Split name inside the tokenized corpus.
    pub split_name: String,
    /// Split kind.
    pub split_kind: DatasetSplitKind,
    /// Stable source-family identifier.
    pub source_family_id: String,
    /// Source ids represented by the row.
    pub source_ids: Vec<String>,
    /// Token share inside the split in basis points.
    pub token_share_bps_within_split: u32,
    /// Sequence share inside the split in basis points.
    pub sequence_share_bps_within_split: u32,
    /// Mean next-token loss in milli-units.
    pub mean_next_token_loss_milli: u32,
    /// Short explanation of the row.
    pub detail: String,
}

/// Replay receipt attached to one Psion pretrain stage run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainReplayReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable dataset identity used during replay.
    pub stable_dataset_identity: String,
    /// Iteration mode used by the replay.
    pub iteration_mode: DatasetIterationMode,
    /// Shard ordering used by the replay.
    pub shard_ordering: DatasetShardOrdering,
    /// Deterministic shuffle seed used by the replay.
    pub deterministic_shuffle_seed: u64,
    /// Number of successful replay checks.
    pub successful_replays: u16,
    /// Whether exact replay was observed.
    pub exact_replay_observed: bool,
    /// Stable digest over the replay receipt.
    pub replay_digest: String,
    /// Short summary of the replay evidence.
    pub summary: String,
}

impl PsionPretrainReplayReceipt {
    /// Creates one replay receipt and computes its stable digest.
    #[must_use]
    pub fn new(
        receipt_id: impl Into<String>,
        stable_dataset_identity: impl Into<String>,
        iteration_mode: DatasetIterationMode,
        shard_ordering: DatasetShardOrdering,
        deterministic_shuffle_seed: u64,
        successful_replays: u16,
        exact_replay_observed: bool,
        summary: impl Into<String>,
    ) -> Self {
        let mut receipt = Self {
            receipt_id: receipt_id.into(),
            stable_dataset_identity: stable_dataset_identity.into(),
            iteration_mode,
            shard_ordering,
            deterministic_shuffle_seed,
            successful_replays,
            exact_replay_observed,
            replay_digest: String::new(),
            summary: summary.into(),
        };
        receipt.replay_digest = stable_pretrain_replay_digest(&receipt);
        receipt
    }
}

/// Checkpoint lineage receipt emitted by one Psion pretrain stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainCheckpointLineageReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Promoted checkpoint emitted by the stage.
    pub promoted_checkpoint: TrainingCheckpointReference,
    /// Base checkpoint the stage resumed from when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_checkpoint: Option<TrainingCheckpointReference>,
    /// Stable promoted-checkpoint label.
    pub promoted_checkpoint_label: String,
    /// Stable model id bound to the checkpoint.
    pub model_id: String,
    /// Stable model-descriptor digest bound to the checkpoint.
    pub model_descriptor_digest: String,
    /// Stable digest over the lineage receipt.
    pub checkpoint_lineage_digest: String,
}

impl PsionPretrainCheckpointLineageReceipt {
    /// Creates one checkpoint-lineage receipt and computes its stable digest.
    #[must_use]
    pub fn new(
        receipt_id: impl Into<String>,
        promoted_checkpoint: TrainingCheckpointReference,
        base_checkpoint: Option<TrainingCheckpointReference>,
        promoted_checkpoint_label: impl Into<String>,
        model_id: impl Into<String>,
        model_descriptor_digest: impl Into<String>,
    ) -> Self {
        let mut receipt = Self {
            receipt_id: receipt_id.into(),
            promoted_checkpoint,
            base_checkpoint,
            promoted_checkpoint_label: promoted_checkpoint_label.into(),
            model_id: model_id.into(),
            model_descriptor_digest: model_descriptor_digest.into(),
            checkpoint_lineage_digest: String::new(),
        };
        receipt.checkpoint_lineage_digest = stable_pretrain_checkpoint_lineage_digest(&receipt);
        receipt
    }
}

/// Accelerator-backed execution summary for one realized pretrain stage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainStageAcceleratorReceipt {
    /// Whether this stage should count as accelerator-backed.
    pub accelerator_backed: bool,
    /// Optimizer steps completed on the realized trainer path.
    pub optimizer_steps_completed: u32,
    /// Mean observed step latency for the realized path.
    pub mean_step_latency_ms: u64,
    /// Short explanation of the accelerator posture.
    pub detail: String,
}

/// Full pretrain-stage receipt for one Psion stage run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionPretrainStageRunReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable run id.
    pub run_id: String,
    /// Stable stage id.
    pub stage_id: String,
    /// Stage kind.
    pub stage_kind: TrainingStageKind,
    /// Stable model id.
    pub model_id: String,
    /// Stable model-descriptor digest.
    pub model_descriptor_digest: String,
    /// Stable tokenizer-binding digest.
    pub tokenizer_binding_digest: String,
    /// Stable dataset identity.
    pub dataset_identity: String,
    /// Stable sampling-policy identifier.
    pub sampling_policy_id: String,
    /// Stable sampling-policy version.
    pub sampling_policy_version: String,
    /// Objective config bound to the stage.
    pub objective_config: PsionPretrainObjectiveConfig,
    /// Source-family-aware reporting rows.
    pub source_family_reports: Vec<PsionPretrainSourceFamilyReportRow>,
    /// Replay receipt for the run.
    pub replay_receipt: PsionPretrainReplayReceipt,
    /// Checkpoint lineage receipt for the run.
    pub checkpoint_lineage: PsionPretrainCheckpointLineageReceipt,
    /// Delivered execution facts for the realized trainer path when the caller
    /// binds explicit backend truth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivered_execution: Option<DeliveredExecutionContext>,
    /// Accelerator-backed execution facts for the realized trainer path when
    /// the stage is intended to prove one accelerator lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accelerator_execution: Option<PsionPretrainStageAcceleratorReceipt>,
    /// Short summary of the run.
    pub summary: String,
    /// Stable digest over the run receipt.
    pub receipt_digest: String,
}

impl PsionPretrainStageRunReceipt {
    /// Validates the stage receipt against the declared stage config and Psion artifacts.
    pub fn validate_against_inputs(
        &self,
        stage_config: &PsionPretrainStageConfig,
        model_descriptor: &PsionCompactDecoderDescriptor,
        tokenized_corpus: &PsionTokenizedCorpusManifest,
        sampling_policy: &PsionSamplingPolicyManifest,
    ) -> Result<(), PsionPretrainStageError> {
        stage_config.validate_against_inputs(
            model_descriptor,
            tokenized_corpus,
            sampling_policy,
        )?;
        ensure_nonempty(
            self.schema_version.as_str(),
            "pretrain_stage_receipt.schema_version",
        )?;
        if self.schema_version != PSION_PRETRAIN_STAGE_RECEIPT_SCHEMA_VERSION {
            return Err(PsionPretrainStageError::SchemaVersionMismatch {
                expected: String::from(PSION_PRETRAIN_STAGE_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        check_string_match(self.run_id.as_str(), stage_config.run_id.as_str(), "run_id")?;
        check_string_match(
            self.stage_id.as_str(),
            stage_config.stage_id.as_str(),
            "stage_id",
        )?;
        if self.stage_kind != TrainingStageKind::Pretrain {
            return Err(PsionPretrainStageError::StageKindMismatch {
                expected: TrainingStageKind::Pretrain,
                actual: self.stage_kind,
            });
        }
        check_string_match(
            self.model_id.as_str(),
            stage_config.model_id.as_str(),
            "model_id",
        )?;
        check_string_match(
            self.model_descriptor_digest.as_str(),
            stage_config.model_descriptor_digest.as_str(),
            "model_descriptor_digest",
        )?;
        check_string_match(
            self.tokenizer_binding_digest.as_str(),
            stage_config.tokenizer_binding_digest.as_str(),
            "tokenizer_binding_digest",
        )?;
        check_string_match(
            self.dataset_identity.as_str(),
            stage_config.dataset_identity.as_str(),
            "dataset_identity",
        )?;
        check_string_match(
            self.sampling_policy_id.as_str(),
            stage_config.sampling_policy_id.as_str(),
            "sampling_policy_id",
        )?;
        check_string_match(
            self.sampling_policy_version.as_str(),
            stage_config.sampling_policy_version.as_str(),
            "sampling_policy_version",
        )?;
        if self.objective_config != stage_config.objective_config {
            return Err(PsionPretrainStageError::ObjectiveConfigMismatch);
        }
        self.validate_source_family_reports(tokenized_corpus)?;
        self.validate_replay_receipt(tokenized_corpus)?;
        self.validate_checkpoint_lineage(model_descriptor)?;
        self.validate_delivered_execution()?;
        self.validate_accelerator_execution()?;
        ensure_nonempty(self.summary.as_str(), "pretrain_stage_receipt.summary")?;
        if self.receipt_digest != stable_pretrain_stage_receipt_digest(self) {
            return Err(PsionPretrainStageError::ReceiptDigestMismatch);
        }
        Ok(())
    }

    fn validate_source_family_reports(
        &self,
        tokenized_corpus: &PsionTokenizedCorpusManifest,
    ) -> Result<(), PsionPretrainStageError> {
        if self.source_family_reports.is_empty() {
            return Err(PsionPretrainStageError::MissingField {
                field: String::from("pretrain_stage_receipt.source_family_reports"),
            });
        }
        let split_map = tokenized_corpus
            .splits
            .iter()
            .map(|split| {
                (
                    split.split_name.as_str(),
                    (split.kind, split.source_family_ids.as_slice()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let family_bindings = tokenized_corpus
            .source_family_bindings
            .iter()
            .map(|binding| {
                (
                    binding.source_family_id.as_str(),
                    binding.source_ids.as_slice(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut seen_rows = BTreeSet::new();
        let mut split_family_coverage = BTreeMap::<String, BTreeSet<String>>::new();
        let mut token_share_totals = BTreeMap::<String, u32>::new();
        let mut sequence_share_totals = BTreeMap::<String, u32>::new();
        for row in &self.source_family_reports {
            ensure_nonempty(
                row.split_name.as_str(),
                "pretrain_stage_receipt.source_family_reports[].split_name",
            )?;
            ensure_nonempty(
                row.source_family_id.as_str(),
                "pretrain_stage_receipt.source_family_reports[].source_family_id",
            )?;
            ensure_nonempty(
                row.detail.as_str(),
                "pretrain_stage_receipt.source_family_reports[].detail",
            )?;
            if !seen_rows.insert((row.split_name.clone(), row.source_family_id.clone())) {
                return Err(PsionPretrainStageError::DuplicateSourceFamilyReportRow {
                    split_name: row.split_name.clone(),
                    source_family_id: row.source_family_id.clone(),
                });
            }
            let Some((split_kind, source_family_ids)) = split_map.get(row.split_name.as_str())
            else {
                return Err(PsionPretrainStageError::UnknownSplitName {
                    split_name: row.split_name.clone(),
                });
            };
            if *split_kind != row.split_kind {
                return Err(PsionPretrainStageError::SplitKindMismatch {
                    split_name: row.split_name.clone(),
                    expected: *split_kind,
                    actual: row.split_kind,
                });
            }
            if !source_family_ids.contains(&row.source_family_id) {
                return Err(PsionPretrainStageError::UnknownSourceFamilyForSplit {
                    split_name: row.split_name.clone(),
                    source_family_id: row.source_family_id.clone(),
                });
            }
            let Some(bound_source_ids) = family_bindings.get(row.source_family_id.as_str()) else {
                return Err(PsionPretrainStageError::UnknownSourceFamilyForSplit {
                    split_name: row.split_name.clone(),
                    source_family_id: row.source_family_id.clone(),
                });
            };
            if row.source_ids.is_empty() {
                return Err(PsionPretrainStageError::MissingField {
                    field: format!(
                        "pretrain_stage_receipt.source_family_reports.{}.{}.source_ids",
                        row.split_name, row.source_family_id
                    ),
                });
            }
            for source_id in &row.source_ids {
                if !bound_source_ids
                    .iter()
                    .any(|bound_source_id| bound_source_id.as_str() == source_id.as_str())
                {
                    return Err(PsionPretrainStageError::UnknownSourceIdForFamily {
                        source_family_id: row.source_family_id.clone(),
                        source_id: source_id.clone(),
                    });
                }
            }
            check_bps(
                row.token_share_bps_within_split,
                "source_family_reports[].token_share_bps_within_split",
            )?;
            check_bps(
                row.sequence_share_bps_within_split,
                "source_family_reports[].sequence_share_bps_within_split",
            )?;
            token_share_totals
                .entry(row.split_name.clone())
                .and_modify(|total| *total = total.saturating_add(row.token_share_bps_within_split))
                .or_insert(row.token_share_bps_within_split);
            sequence_share_totals
                .entry(row.split_name.clone())
                .and_modify(|total| {
                    *total = total.saturating_add(row.sequence_share_bps_within_split)
                })
                .or_insert(row.sequence_share_bps_within_split);
            split_family_coverage
                .entry(row.split_name.clone())
                .or_insert_with(BTreeSet::new)
                .insert(row.source_family_id.clone());
        }
        for split in &tokenized_corpus.splits {
            let covered_families = split_family_coverage
                .get(split.split_name.as_str())
                .cloned()
                .unwrap_or_default();
            let expected_families = split
                .source_family_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            if covered_families != expected_families {
                return Err(PsionPretrainStageError::SourceFamilyCoverageMismatch {
                    split_name: split.split_name.clone(),
                });
            }
            if token_share_totals
                .get(split.split_name.as_str())
                .copied()
                .unwrap_or(0)
                != 10_000
            {
                return Err(PsionPretrainStageError::InvalidSplitShareTotal {
                    split_name: split.split_name.clone(),
                    share_kind: String::from("token"),
                });
            }
            if sequence_share_totals
                .get(split.split_name.as_str())
                .copied()
                .unwrap_or(0)
                != 10_000
            {
                return Err(PsionPretrainStageError::InvalidSplitShareTotal {
                    split_name: split.split_name.clone(),
                    share_kind: String::from("sequence"),
                });
            }
        }
        Ok(())
    }

    fn validate_replay_receipt(
        &self,
        tokenized_corpus: &PsionTokenizedCorpusManifest,
    ) -> Result<(), PsionPretrainStageError> {
        ensure_nonempty(
            self.replay_receipt.receipt_id.as_str(),
            "pretrain_stage_receipt.replay_receipt.receipt_id",
        )?;
        ensure_nonempty(
            self.replay_receipt.summary.as_str(),
            "pretrain_stage_receipt.replay_receipt.summary",
        )?;
        check_string_match(
            self.replay_receipt.stable_dataset_identity.as_str(),
            tokenized_corpus
                .replay_contract
                .stable_dataset_identity
                .as_str(),
            "replay_receipt.stable_dataset_identity",
        )?;
        if self.replay_receipt.iteration_mode != tokenized_corpus.replay_contract.iteration_mode
            || self.replay_receipt.shard_ordering != tokenized_corpus.replay_contract.shard_ordering
            || self.replay_receipt.deterministic_shuffle_seed
                != tokenized_corpus.replay_contract.deterministic_shuffle_seed
        {
            return Err(PsionPretrainStageError::ReplayContractMismatch);
        }
        if self.replay_receipt.successful_replays == 0 {
            return Err(PsionPretrainStageError::MissingSuccessfulReplay);
        }
        if self.replay_receipt.replay_digest != stable_pretrain_replay_digest(&self.replay_receipt)
        {
            return Err(PsionPretrainStageError::ReplayDigestMismatch);
        }
        Ok(())
    }

    fn validate_checkpoint_lineage(
        &self,
        model_descriptor: &PsionCompactDecoderDescriptor,
    ) -> Result<(), PsionPretrainStageError> {
        ensure_nonempty(
            self.checkpoint_lineage.receipt_id.as_str(),
            "pretrain_stage_receipt.checkpoint_lineage.receipt_id",
        )?;
        ensure_nonempty(
            self.checkpoint_lineage.promoted_checkpoint_label.as_str(),
            "pretrain_stage_receipt.checkpoint_lineage.promoted_checkpoint_label",
        )?;
        check_string_match(
            self.checkpoint_lineage.model_id.as_str(),
            model_descriptor.model.model_id.as_str(),
            "checkpoint_lineage.model_id",
        )?;
        check_string_match(
            self.checkpoint_lineage.model_descriptor_digest.as_str(),
            model_descriptor.stable_digest().as_str(),
            "checkpoint_lineage.model_descriptor_digest",
        )?;
        ensure_nonempty(
            self.checkpoint_lineage
                .promoted_checkpoint
                .checkpoint_family
                .as_str(),
            "pretrain_stage_receipt.checkpoint_lineage.promoted_checkpoint.checkpoint_family",
        )?;
        if let Some(base_checkpoint) = &self.checkpoint_lineage.base_checkpoint {
            if base_checkpoint.checkpoint_family
                != self
                    .checkpoint_lineage
                    .promoted_checkpoint
                    .checkpoint_family
            {
                return Err(PsionPretrainStageError::CheckpointFamilyMismatch {
                    expected: self
                        .checkpoint_lineage
                        .promoted_checkpoint
                        .checkpoint_family
                        .clone(),
                    actual: base_checkpoint.checkpoint_family.clone(),
                });
            }
        }
        if self.checkpoint_lineage.checkpoint_lineage_digest
            != stable_pretrain_checkpoint_lineage_digest(&self.checkpoint_lineage)
        {
            return Err(PsionPretrainStageError::CheckpointLineageDigestMismatch);
        }
        Ok(())
    }

    fn validate_delivered_execution(&self) -> Result<(), PsionPretrainStageError> {
        let Some(delivered_execution) = &self.delivered_execution else {
            return Ok(());
        };
        ensure_nonempty(
            delivered_execution.runtime_backend.as_str(),
            "pretrain_stage_receipt.delivered_execution.runtime_backend",
        )?;
        if delivered_execution.selected_devices.is_empty() {
            return Err(PsionPretrainStageError::InvalidDeliveredExecution {
                detail: String::from(
                    "pretrain_stage_receipt.delivered_execution.selected_devices must not be empty",
                ),
            });
        }
        if let Some(execution_topology) = &delivered_execution.execution_topology {
            if execution_topology.effective_backend != delivered_execution.runtime_backend {
                return Err(PsionPretrainStageError::InvalidDeliveredExecution {
                    detail: String::from(
                        "pretrain_stage_receipt.delivered_execution.execution_topology.effective_backend must match runtime_backend",
                    ),
                });
            }
        }
        Ok(())
    }

    fn validate_accelerator_execution(&self) -> Result<(), PsionPretrainStageError> {
        let Some(accelerator_execution) = &self.accelerator_execution else {
            return Ok(());
        };
        ensure_nonempty(
            accelerator_execution.detail.as_str(),
            "pretrain_stage_receipt.accelerator_execution.detail",
        )?;
        if accelerator_execution.optimizer_steps_completed == 0 {
            return Err(PsionPretrainStageError::InvalidDeliveredExecution {
                detail: String::from(
                    "pretrain_stage_receipt.accelerator_execution.optimizer_steps_completed must be greater than zero",
                ),
            });
        }
        if accelerator_execution.mean_step_latency_ms == 0 {
            return Err(PsionPretrainStageError::InvalidDeliveredExecution {
                detail: String::from(
                    "pretrain_stage_receipt.accelerator_execution.mean_step_latency_ms must be greater than zero",
                ),
            });
        }
        if self.delivered_execution.is_none() {
            return Err(PsionPretrainStageError::InvalidDeliveredExecution {
                detail: String::from(
                    "pretrain_stage_receipt.accelerator_execution requires delivered_execution",
                ),
            });
        }
        Ok(())
    }
}

/// Runs one declared Psion pretrain stage and emits the typed receipt.
pub fn run_psion_pretrain_stage(
    stage_config: &PsionPretrainStageConfig,
    source_family_reports: Vec<PsionPretrainSourceFamilyReportRow>,
    replay_receipt: PsionPretrainReplayReceipt,
    checkpoint_lineage: PsionPretrainCheckpointLineageReceipt,
    summary: impl Into<String>,
    model_descriptor: &PsionCompactDecoderDescriptor,
    tokenized_corpus: &PsionTokenizedCorpusManifest,
    sampling_policy: &PsionSamplingPolicyManifest,
) -> Result<PsionPretrainStageRunReceipt, PsionPretrainStageError> {
    run_psion_pretrain_stage_with_execution(
        stage_config,
        source_family_reports,
        replay_receipt,
        checkpoint_lineage,
        None,
        None,
        summary,
        model_descriptor,
        tokenized_corpus,
        sampling_policy,
    )
}

/// Runs one declared Psion pretrain stage and emits the typed receipt with an
/// optional explicit execution-backend binding.
pub fn run_psion_pretrain_stage_with_execution(
    stage_config: &PsionPretrainStageConfig,
    source_family_reports: Vec<PsionPretrainSourceFamilyReportRow>,
    replay_receipt: PsionPretrainReplayReceipt,
    checkpoint_lineage: PsionPretrainCheckpointLineageReceipt,
    delivered_execution: Option<DeliveredExecutionContext>,
    accelerator_execution: Option<PsionPretrainStageAcceleratorReceipt>,
    summary: impl Into<String>,
    model_descriptor: &PsionCompactDecoderDescriptor,
    tokenized_corpus: &PsionTokenizedCorpusManifest,
    sampling_policy: &PsionSamplingPolicyManifest,
) -> Result<PsionPretrainStageRunReceipt, PsionPretrainStageError> {
    let mut receipt = PsionPretrainStageRunReceipt {
        schema_version: String::from(PSION_PRETRAIN_STAGE_RECEIPT_SCHEMA_VERSION),
        run_id: stage_config.run_id.clone(),
        stage_id: stage_config.stage_id.clone(),
        stage_kind: TrainingStageKind::Pretrain,
        model_id: stage_config.model_id.clone(),
        model_descriptor_digest: stage_config.model_descriptor_digest.clone(),
        tokenizer_binding_digest: stage_config.tokenizer_binding_digest.clone(),
        dataset_identity: stage_config.dataset_identity.clone(),
        sampling_policy_id: stage_config.sampling_policy_id.clone(),
        sampling_policy_version: stage_config.sampling_policy_version.clone(),
        objective_config: stage_config.objective_config.clone(),
        source_family_reports,
        replay_receipt,
        checkpoint_lineage,
        delivered_execution,
        accelerator_execution,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_pretrain_stage_receipt_digest(&receipt);
    receipt.validate_against_inputs(
        stage_config,
        model_descriptor,
        tokenized_corpus,
        sampling_policy,
    )?;
    Ok(receipt)
}

/// Error returned by the Psion pretrain-stage contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PsionPretrainStageError {
    /// One required field was missing or empty.
    #[error("Psion pretrain stage field `{field}` is missing")]
    MissingField {
        /// Field path.
        field: String,
    },
    /// One schema version drifted from the expected contract.
    #[error("Psion pretrain stage expected schema version `{expected}`, found `{actual}`")]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },
    /// One string field drifted from the expected value.
    #[error("Psion pretrain stage field `{field}` expected `{expected}`, found `{actual}`")]
    FieldMismatch {
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
    /// Stage kind drifted from the explicit pretrain contract.
    #[error("Psion pretrain stage expected stage kind `{expected:?}`, found `{actual:?}`")]
    StageKindMismatch {
        /// Expected stage kind.
        expected: TrainingStageKind,
        /// Actual stage kind.
        actual: TrainingStageKind,
    },
    /// One explicit delivered-execution binding was internally inconsistent.
    #[error("Psion pretrain stage delivered execution is invalid: {detail}")]
    InvalidDeliveredExecution {
        /// Human-readable validation detail.
        detail: String,
    },
    /// Label-smoothing basis points were invalid.
    #[error("Psion pretrain stage field `{field}` must stay within 0..=10000 basis points, found `{actual_bps}`")]
    InvalidBpsValue {
        /// Field name.
        field: String,
        /// Actual bps value.
        actual_bps: u32,
    },
    /// Context length drifted from the model descriptor.
    #[error("Psion pretrain stage expected context length `{expected}`, found `{actual}`")]
    ContextLengthMismatch {
        /// Expected context length.
        expected: usize,
        /// Actual context length.
        actual: usize,
    },
    /// The receipt objective config drifted from the declared stage config.
    #[error("Psion pretrain stage objective config drifted from the declared stage config")]
    ObjectiveConfigMismatch,
    /// One report row was duplicated.
    #[error("Psion pretrain stage repeated source-family report for split `{split_name}` family `{source_family_id}`")]
    DuplicateSourceFamilyReportRow {
        /// Split name.
        split_name: String,
        /// Source-family identifier.
        source_family_id: String,
    },
    /// One split name was unknown to the tokenized corpus.
    #[error("Psion pretrain stage does not know split `{split_name}`")]
    UnknownSplitName {
        /// Unknown split name.
        split_name: String,
    },
    /// One split kind drifted from the tokenized corpus.
    #[error("Psion pretrain stage split `{split_name}` expected kind `{expected:?}`, found `{actual:?}`")]
    SplitKindMismatch {
        /// Split name.
        split_name: String,
        /// Expected split kind.
        expected: DatasetSplitKind,
        /// Actual split kind.
        actual: DatasetSplitKind,
    },
    /// One source family was not represented in the split.
    #[error("Psion pretrain stage split `{split_name}` does not know source family `{source_family_id}`")]
    UnknownSourceFamilyForSplit {
        /// Split name.
        split_name: String,
        /// Unknown source-family id.
        source_family_id: String,
    },
    /// One source id was not bound to the reported family.
    #[error("Psion pretrain stage family `{source_family_id}` does not know source `{source_id}`")]
    UnknownSourceIdForFamily {
        /// Source-family identifier.
        source_family_id: String,
        /// Unknown source identifier.
        source_id: String,
    },
    /// The report rows did not cover exactly the families in one split.
    #[error("Psion pretrain stage source-family reporting for split `{split_name}` must cover exactly the source families represented by the tokenized corpus split")]
    SourceFamilyCoverageMismatch {
        /// Split name.
        split_name: String,
    },
    /// Token or sequence share totals did not sum to 10000 within one split.
    #[error("Psion pretrain stage `{share_kind}` share totals for split `{split_name}` must sum to 10000 basis points")]
    InvalidSplitShareTotal {
        /// Split name.
        split_name: String,
        /// Share kind.
        share_kind: String,
    },
    /// Replay contract facts drifted from the tokenized corpus.
    #[error("Psion pretrain stage replay contract drifted from the tokenized corpus")]
    ReplayContractMismatch,
    /// The receipt did not record any successful replay.
    #[error("Psion pretrain stage replay receipt requires at least one successful replay")]
    MissingSuccessfulReplay,
    /// The replay digest drifted from the replay receipt payload.
    #[error("Psion pretrain stage replay digest drifted from the replay receipt payload")]
    ReplayDigestMismatch,
    /// Base and promoted checkpoints used different families.
    #[error(
        "Psion pretrain stage checkpoint family mismatch: expected `{expected}`, found `{actual}`"
    )]
    CheckpointFamilyMismatch {
        /// Expected checkpoint family.
        expected: String,
        /// Actual checkpoint family.
        actual: String,
    },
    /// The checkpoint-lineage digest drifted from the receipt payload.
    #[error("Psion pretrain stage checkpoint-lineage digest drifted from the receipt payload")]
    CheckpointLineageDigestMismatch,
    /// The run receipt digest drifted from the receipt payload.
    #[error("Psion pretrain stage receipt digest drifted from the receipt payload")]
    ReceiptDigestMismatch,
}

fn stable_pretrain_replay_digest(receipt: &PsionPretrainReplayReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_pretrain_replay_receipt|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.stable_dataset_identity.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", receipt.iteration_mode).as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:?}", receipt.shard_ordering).as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.deterministic_shuffle_seed.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.successful_replays.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(if receipt.exact_replay_observed {
        b"exact".as_slice()
    } else {
        b"non_exact".as_slice()
    });
    hasher.update(b"|");
    hasher.update(receipt.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_pretrain_checkpoint_lineage_digest(
    receipt: &PsionPretrainCheckpointLineageReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_pretrain_checkpoint_lineage|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.promoted_checkpoint.checkpoint_family.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.promoted_checkpoint.stream_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.promoted_checkpoint.object_digest.as_bytes());
    if let Some(base_checkpoint) = &receipt.base_checkpoint {
        hasher.update(b"|base|");
        hasher.update(base_checkpoint.stream_id.as_bytes());
        hasher.update(b"|");
        hasher.update(base_checkpoint.object_digest.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(receipt.promoted_checkpoint_label.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.model_descriptor_digest.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_pretrain_stage_receipt_digest(receipt: &PsionPretrainStageRunReceipt) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_pretrain_stage_receipt|");
    hasher.update(receipt.run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.stage_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.model_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.model_descriptor_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.tokenizer_binding_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.dataset_identity.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.sampling_policy_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.sampling_policy_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.replay_receipt.replay_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(
        receipt
            .checkpoint_lineage
            .checkpoint_lineage_digest
            .as_bytes(),
    );
    if let Some(delivered_execution) = &receipt.delivered_execution {
        hasher.update(b"|backend|");
        hasher.update(delivered_execution.runtime_backend.as_bytes());
        for device in &delivered_execution.selected_devices {
            hasher.update(b"|device|");
            hasher.update(device.stable_device_id.as_bytes());
        }
        if let Some(execution_topology) = &delivered_execution.execution_topology {
            hasher.update(b"|topology|");
            hasher.update(execution_topology.effective_backend.as_bytes());
        }
    }
    if let Some(accelerator_execution) = &receipt.accelerator_execution {
        hasher.update(b"|accelerator|");
        hasher.update(if accelerator_execution.accelerator_backed {
            b"true".as_slice()
        } else {
            b"false".as_slice()
        });
        hasher.update(b"|");
        hasher.update(
            accelerator_execution
                .optimizer_steps_completed
                .to_string()
                .as_bytes(),
        );
        hasher.update(b"|");
        hasher.update(
            accelerator_execution
                .mean_step_latency_ms
                .to_string()
                .as_bytes(),
        );
    }
    hasher.update(b"|");
    hasher.update(receipt.summary.as_bytes());
    for row in &receipt.source_family_reports {
        hasher.update(b"|row|");
        hasher.update(row.split_name.as_bytes());
        hasher.update(b"|");
        hasher.update(format!("{:?}", row.split_kind).as_bytes());
        hasher.update(b"|");
        hasher.update(row.source_family_id.as_bytes());
        hasher.update(b"|");
        hasher.update(row.token_share_bps_within_split.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.sequence_share_bps_within_split.to_string().as_bytes());
        hasher.update(b"|");
        hasher.update(row.mean_next_token_loss_milli.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionPretrainStageError> {
    if value.trim().is_empty() {
        return Err(PsionPretrainStageError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionPretrainStageError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(PsionPretrainStageError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn check_bps(value: u32, field: &str) -> Result<(), PsionPretrainStageError> {
    if value > 10_000 {
        return Err(PsionPretrainStageError::InvalidBpsValue {
            field: String::from(field),
            actual_bps: value,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psionic_data::PsionTokenizedCorpusManifest;
    use psionic_models::PsionCompactDecoderDescriptor;
    use psionic_runtime::{
        DeliveredExecutionContext, DeviceInventoryQualifiers, DeviceMemoryClass,
        DevicePerformanceClass,
    };

    fn model_descriptor() -> PsionCompactDecoderDescriptor {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/models/psion_compact_decoder_pilot_descriptor_v1.json"
        ))
        .expect("model descriptor should parse")
    }

    fn tokenized_corpus() -> PsionTokenizedCorpusManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json"
        ))
        .expect("tokenized corpus should parse")
    }

    fn sampling_policy() -> PsionSamplingPolicyManifest {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/sampling/psion_sampling_policy_manifest_v1.json"
        ))
        .expect("sampling policy should parse")
    }

    fn pretrain_stage_config() -> PsionPretrainStageConfig {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_pretrain_stage_config_v1.json"
        ))
        .expect("pretrain stage config should parse")
    }

    fn pretrain_stage_receipt() -> PsionPretrainStageRunReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json"
        ))
        .expect("pretrain stage receipt should parse")
    }

    #[test]
    fn pretrain_stage_config_binds_model_dataset_and_sampling_policy() {
        pretrain_stage_config()
            .validate_against_inputs(&model_descriptor(), &tokenized_corpus(), &sampling_policy())
            .expect("pretrain stage config should validate");
    }

    #[test]
    fn pretrain_stage_receipt_validates_replay_and_checkpoint_lineage() {
        pretrain_stage_receipt()
            .validate_against_inputs(
                &pretrain_stage_config(),
                &model_descriptor(),
                &tokenized_corpus(),
                &sampling_policy(),
            )
            .expect("pretrain stage receipt should validate");
    }

    #[test]
    fn source_family_report_must_cover_each_split_family_pair() {
        let mut receipt = pretrain_stage_receipt();
        receipt.source_family_reports.pop();
        let error = receipt
            .validate_against_inputs(
                &pretrain_stage_config(),
                &model_descriptor(),
                &tokenized_corpus(),
                &sampling_policy(),
            )
            .expect_err("missing split-family row should be rejected");
        assert!(matches!(
            error,
            PsionPretrainStageError::SourceFamilyCoverageMismatch { .. }
        ));
    }

    #[test]
    fn replay_receipt_must_preserve_the_tokenized_corpus_contract() {
        let mut receipt = pretrain_stage_receipt();
        receipt.replay_receipt.deterministic_shuffle_seed = 42;
        receipt.replay_receipt.replay_digest =
            stable_pretrain_replay_digest(&receipt.replay_receipt);
        let error = receipt
            .validate_against_inputs(
                &pretrain_stage_config(),
                &model_descriptor(),
                &tokenized_corpus(),
                &sampling_policy(),
            )
            .expect_err("replay drift should be rejected");
        assert!(matches!(
            error,
            PsionPretrainStageError::ReplayContractMismatch
        ));
    }

    #[test]
    fn pretrain_stage_receipt_accepts_explicit_cuda_execution_binding() {
        let config = pretrain_stage_config();
        let mut receipt = pretrain_stage_receipt();
        receipt.delivered_execution = Some(DeliveredExecutionContext::new(
            "cuda",
            None,
            vec![DeviceInventoryQualifiers {
                stable_device_id: String::from("cuda:0"),
                topology_key: None,
                performance_class: DevicePerformanceClass::DiscreteAccelerator,
                memory_class: DeviceMemoryClass::DedicatedDevice,
                total_memory_bytes: Some(24 * 1024 * 1024 * 1024),
                free_memory_bytes: Some(20 * 1024 * 1024 * 1024),
            }],
        ));
        receipt.accelerator_execution = Some(PsionPretrainStageAcceleratorReceipt {
            accelerator_backed: true,
            optimizer_steps_completed: 16,
            mean_step_latency_ms: 40,
            detail: String::from(
                "Accelerated reference stage completed bounded CUDA-backed optimizer steps.",
            ),
        });
        receipt.receipt_digest = stable_pretrain_stage_receipt_digest(&receipt);
        receipt
            .validate_against_inputs(
                &config,
                &model_descriptor(),
                &tokenized_corpus(),
                &sampling_policy(),
            )
            .expect("cuda execution binding should validate");
    }
}
