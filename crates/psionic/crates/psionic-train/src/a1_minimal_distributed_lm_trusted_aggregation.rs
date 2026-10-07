use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_core::TensorData;
use psionic_models::{Cs336A1ReferenceConfig, Cs336A1TransformerLm};
use psionic_nn::{
    cross_entropy_loss, LayerError, LossReduction, ModuleStateDict, ModuleStateEntry,
    ModuleStateLoadMode, NnTensor, NnTrainingError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_a1_minimal_distributed_lm_lane_contract,
    canonical_a1_minimal_distributed_lm_tokenizer_dataset_bundle,
    initialize_cs336_a1_reference_model,
    load_a1_minimal_distributed_lm_local_update_artifact_manifest,
    load_a1_minimal_distributed_lm_local_update_checkpoint,
    load_a1_minimal_distributed_lm_local_update_contribution_receipt,
    run_and_write_a1_minimal_distributed_lm_local_update, A1MinimalDistributedLmInputShardRef,
    A1MinimalDistributedLmInputTokenRange, A1MinimalDistributedLmLaneContract,
    A1MinimalDistributedLmLocalUpdateArtifactManifest, A1MinimalDistributedLmLocalUpdateCheckpoint,
    A1MinimalDistributedLmLocalUpdateConfig, A1MinimalDistributedLmLocalUpdateContributionReceipt,
    A1MinimalDistributedLmLocalUpdateError, A1MinimalDistributedLmLocalUpdateReport,
    A1MinimalDistributedLmTokenizerDatasetBundle, Cs336A1ReferenceBatch,
    Cs336A1ReferenceTrainingError, A1_MINIMAL_DISTRIBUTED_LM_LANE_ID,
    A1_MINIMAL_DISTRIBUTED_LM_LOCAL_UPDATE_ARTIFACT_MANIFEST_FIXTURE_PATH,
    A1_MINIMAL_DISTRIBUTED_LM_LOCAL_UPDATE_CHECKPOINT_STEP4_FIXTURE_PATH,
    A1_MINIMAL_DISTRIBUTED_LM_LOCAL_UPDATE_CONTRIBUTION_RECEIPT_FIXTURE_PATH,
};

pub const A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATION_REPORT_SCHEMA_VERSION: &str =
    "psion.a1_minimal_distributed_lm.trusted_aggregation_report.v1";
pub const A1_MINIMAL_DISTRIBUTED_LM_AGGREGATE_CHECKPOINT_SCHEMA_VERSION: &str =
    "psion.a1_minimal_distributed_lm.aggregate_checkpoint.v1";
pub const A1_MINIMAL_DISTRIBUTED_LM_PROMOTION_RECEIPT_SCHEMA_VERSION: &str =
    "psion.a1_minimal_distributed_lm.promotion_receipt.v1";
pub const A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATION_RUN_ID: &str =
    "a1_minimal_distributed_lm_001.trusted_aggregation_fixture";
pub const A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATION_WINDOW_ID: &str =
    "window.a1_minimal_distributed_lm_001.trusted_aggregate.000001";
pub const A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATE_ID: &str =
    "aggregate.a1_minimal_distributed_lm_001.trusted_fixture.000001";
pub const A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATION_REPORT_FIXTURE_PATH: &str =
    "fixtures/psion/a1_minimal_distributed_lm/trusted_aggregation_report_v1.json";
pub const A1_MINIMAL_DISTRIBUTED_LM_AGGREGATE_CHECKPOINT_FIXTURE_PATH: &str =
    "fixtures/psion/a1_minimal_distributed_lm/aggregate_checkpoint_v1.json";
pub const A1_MINIMAL_DISTRIBUTED_LM_PROMOTION_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/psion/a1_minimal_distributed_lm/promotion_receipt_v1.json";

#[derive(Debug, Error)]
pub enum A1MinimalDistributedLmTrustedAggregationError {
    #[error("invalid A1 minimal distributed LM trusted aggregation: {detail}")]
    Invalid { detail: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    Model(#[from] psionic_models::Cs336A1ReferenceError),
    #[error(transparent)]
    Nn(#[from] NnTrainingError),
    #[error(transparent)]
    Layer(#[from] LayerError),
    #[error(transparent)]
    ModuleState(#[from] psionic_nn::ModuleStateError),
    #[error(transparent)]
    ModuleStateLoad(#[from] psionic_nn::ModuleStateLoadError),
    #[error(transparent)]
    LocalUpdate(#[from] A1MinimalDistributedLmLocalUpdateError),
    #[error(transparent)]
    ReferenceTraining(#[from] Cs336A1ReferenceTrainingError),
    #[error(transparent)]
    Bundle(#[from] crate::A1MinimalDistributedLmTokenizerDatasetBundleError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmTrustedAggregationConfig {
    pub run_id: String,
    pub aggregate_window_id: String,
    pub accepted_aggregate_id: String,
    pub promoted_checkpoint_ref: String,
    pub output_checkpoint_pointer: String,
}

impl Default for A1MinimalDistributedLmTrustedAggregationConfig {
    fn default() -> Self {
        Self {
            run_id: String::from(A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATION_RUN_ID),
            aggregate_window_id: String::from(
                A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATION_WINDOW_ID,
            ),
            accepted_aggregate_id: String::from(A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATE_ID),
            promoted_checkpoint_ref: String::from(
                "checkpoint://a1_minimal_distributed_lm_001.trusted_aggregation_fixture/promoted/step-000004",
            ),
            output_checkpoint_pointer: String::from(
                "pointer://a1_minimal_distributed_lm_001.trusted_aggregation_fixture/promoted/latest",
            ),
        }
    }
}

impl A1MinimalDistributedLmTrustedAggregationConfig {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmTrustedAggregationError> {
        ensure_nonempty(self.run_id.as_str(), "aggregation_config.run_id")?;
        ensure_nonempty(
            self.aggregate_window_id.as_str(),
            "aggregation_config.aggregate_window_id",
        )?;
        ensure_nonempty(
            self.accepted_aggregate_id.as_str(),
            "aggregation_config.accepted_aggregate_id",
        )?;
        ensure_nonempty(
            self.promoted_checkpoint_ref.as_str(),
            "aggregation_config.promoted_checkpoint_ref",
        )?;
        ensure_nonempty(
            self.output_checkpoint_pointer.as_str(),
            "aggregation_config.output_checkpoint_pointer",
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmAcceptedLocalUpdate {
    pub run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub contribution_id: String,
    pub worker_id: String,
    pub node_pubkey: String,
    pub tokenizer_digest: String,
    pub tokenized_dataset_digest: String,
    pub validation_set_digest: String,
    pub input_shard: A1MinimalDistributedLmInputShardRef,
    pub input_token_range: A1MinimalDistributedLmInputTokenRange,
    pub base_checkpoint_ref: String,
    pub base_checkpoint_digest: String,
    pub local_step_count: u64,
    pub consumed_token_count: u64,
    pub loss_before: f32,
    pub loss_after: f32,
    pub output_checkpoint_ref: String,
    pub output_checkpoint_digest: String,
    pub output_delta_ref: String,
    pub output_delta_digest: String,
    pub source_report_digest: String,
    pub source_artifact_manifest_digest: String,
    pub source_contribution_digest: String,
    pub validator_disposition: String,
    pub accepted_for_aggregation: bool,
    pub aggregation_weight_basis: String,
    pub aggregation_weight_value: u64,
    pub accepted_local_update_digest: String,
}

impl A1MinimalDistributedLmAcceptedLocalUpdate {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.accepted_local_update_digest.clear();
        sha256_uri_digest(
            b"psion_a1_minimal_distributed_lm_accepted_local_update|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), A1MinimalDistributedLmTrustedAggregationError> {
        ensure_nonempty(self.run_id.as_str(), "accepted_update.run_id")?;
        ensure_nonempty(self.window_id.as_str(), "accepted_update.window_id")?;
        ensure_nonempty(self.assignment_id.as_str(), "accepted_update.assignment_id")?;
        ensure_nonempty(
            self.contribution_id.as_str(),
            "accepted_update.contribution_id",
        )?;
        ensure_nonempty(self.worker_id.as_str(), "accepted_update.worker_id")?;
        ensure_nonempty(self.node_pubkey.as_str(), "accepted_update.node_pubkey")?;
        ensure_sha256_uri(
            self.tokenizer_digest.as_str(),
            "accepted_update.tokenizer_digest",
        )?;
        ensure_sha256_uri(
            self.tokenized_dataset_digest.as_str(),
            "accepted_update.tokenized_dataset_digest",
        )?;
        ensure_sha256_uri(
            self.validation_set_digest.as_str(),
            "accepted_update.validation_set_digest",
        )?;
        ensure_nonempty(
            self.base_checkpoint_ref.as_str(),
            "accepted_update.base_checkpoint_ref",
        )?;
        ensure_sha256_uri(
            self.base_checkpoint_digest.as_str(),
            "accepted_update.base_checkpoint_digest",
        )?;
        if self.local_step_count == 0
            || self.consumed_token_count == 0
            || self.aggregation_weight_value == 0
        {
            return invalid_aggregation(String::from(
                "accepted update counters and aggregation weight must be nonzero",
            ));
        }
        ensure_finite(self.loss_before, "accepted_update.loss_before")?;
        ensure_finite(self.loss_after, "accepted_update.loss_after")?;
        ensure_nonempty(
            self.output_checkpoint_ref.as_str(),
            "accepted_update.output_checkpoint_ref",
        )?;
        ensure_sha256_uri(
            self.output_checkpoint_digest.as_str(),
            "accepted_update.output_checkpoint_digest",
        )?;
        ensure_nonempty(
            self.output_delta_ref.as_str(),
            "accepted_update.output_delta_ref",
        )?;
        ensure_sha256_uri(
            self.output_delta_digest.as_str(),
            "accepted_update.output_delta_digest",
        )?;
        ensure_sha256_uri(
            self.source_report_digest.as_str(),
            "accepted_update.source_report_digest",
        )?;
        ensure_sha256_uri(
            self.source_artifact_manifest_digest.as_str(),
            "accepted_update.source_artifact_manifest_digest",
        )?;
        ensure_sha256_uri(
            self.source_contribution_digest.as_str(),
            "accepted_update.source_contribution_digest",
        )?;
        if self.validator_disposition != "accepted" || !self.accepted_for_aggregation {
            return invalid_aggregation(String::from(
                "trusted aggregation may consume only accepted updates",
            ));
        }
        if self.aggregation_weight_basis != "tokens"
            || self.aggregation_weight_value != self.consumed_token_count
        {
            return invalid_aggregation(String::from(
                "accepted update aggregation weight must use consumed tokens",
            ));
        }
        ensure_sha256_uri(
            self.accepted_local_update_digest.as_str(),
            "accepted_update.accepted_local_update_digest",
        )?;
        if self.accepted_local_update_digest != self.stable_digest() {
            return invalid_aggregation(String::from("accepted update digest drifted"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmAggregateCheckpoint {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub aggregate_window_id: String,
    pub accepted_aggregate_id: String,
    pub tokenizer_digest: String,
    pub tokenized_dataset_digest: String,
    pub validation_set_digest: String,
    pub base_checkpoint_ref: String,
    pub base_checkpoint_digest: String,
    pub aggregated_delta_digest: String,
    pub output_checkpoint_pointer: String,
    pub promoted_checkpoint_ref: String,
    pub validation_loss_before: f32,
    pub validation_loss_after: f32,
    pub model_state: ModuleStateDict,
    pub checkpoint_digest: String,
}

impl A1MinimalDistributedLmAggregateCheckpoint {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.checkpoint_digest.clear();
        sha256_uri_digest(
            b"psion_a1_minimal_distributed_lm_aggregate_checkpoint|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), A1MinimalDistributedLmTrustedAggregationError> {
        if self.schema_version != A1_MINIMAL_DISTRIBUTED_LM_AGGREGATE_CHECKPOINT_SCHEMA_VERSION {
            return invalid_aggregation(String::from("aggregate checkpoint schema drifted"));
        }
        validate_lane_identity(self.lane_id.as_str())?;
        ensure_nonempty(self.run_id.as_str(), "aggregate_checkpoint.run_id")?;
        ensure_nonempty(
            self.aggregate_window_id.as_str(),
            "aggregate_checkpoint.aggregate_window_id",
        )?;
        ensure_nonempty(
            self.accepted_aggregate_id.as_str(),
            "aggregate_checkpoint.accepted_aggregate_id",
        )?;
        ensure_sha256_uri(
            self.tokenizer_digest.as_str(),
            "aggregate_checkpoint.tokenizer_digest",
        )?;
        ensure_sha256_uri(
            self.tokenized_dataset_digest.as_str(),
            "aggregate_checkpoint.tokenized_dataset_digest",
        )?;
        ensure_sha256_uri(
            self.validation_set_digest.as_str(),
            "aggregate_checkpoint.validation_set_digest",
        )?;
        ensure_nonempty(
            self.base_checkpoint_ref.as_str(),
            "aggregate_checkpoint.base_checkpoint_ref",
        )?;
        ensure_sha256_uri(
            self.base_checkpoint_digest.as_str(),
            "aggregate_checkpoint.base_checkpoint_digest",
        )?;
        ensure_sha256_uri(
            self.aggregated_delta_digest.as_str(),
            "aggregate_checkpoint.aggregated_delta_digest",
        )?;
        ensure_nonempty(
            self.output_checkpoint_pointer.as_str(),
            "aggregate_checkpoint.output_checkpoint_pointer",
        )?;
        ensure_nonempty(
            self.promoted_checkpoint_ref.as_str(),
            "aggregate_checkpoint.promoted_checkpoint_ref",
        )?;
        ensure_finite(
            self.validation_loss_before,
            "aggregate_checkpoint.validation_loss_before",
        )?;
        ensure_finite(
            self.validation_loss_after,
            "aggregate_checkpoint.validation_loss_after",
        )?;
        ensure_sha256_uri(
            self.checkpoint_digest.as_str(),
            "aggregate_checkpoint.checkpoint_digest",
        )?;
        if self.checkpoint_digest != self.stable_digest() {
            return invalid_aggregation(String::from("aggregate checkpoint digest drifted"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmPromotionReceipt {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub aggregate_window_id: String,
    pub accepted_aggregate_id: String,
    pub aggregated_delta_digest: String,
    pub output_checkpoint_pointer: String,
    pub output_checkpoint_digest: String,
    pub promoted_checkpoint_ref: String,
    pub validation_loss_before: f32,
    pub validation_loss_after: f32,
    pub accepted_contribution_count: usize,
    pub model_progress_participant_count: usize,
    pub promotion_verdict: String,
    pub promotion_receipt_digest: String,
    pub claim_boundary: String,
}

impl A1MinimalDistributedLmPromotionReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.promotion_receipt_digest.clear();
        sha256_uri_digest(
            b"psion_a1_minimal_distributed_lm_promotion_receipt|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), A1MinimalDistributedLmTrustedAggregationError> {
        if self.schema_version != A1_MINIMAL_DISTRIBUTED_LM_PROMOTION_RECEIPT_SCHEMA_VERSION {
            return invalid_aggregation(String::from("promotion receipt schema drifted"));
        }
        validate_lane_identity(self.lane_id.as_str())?;
        ensure_nonempty(self.run_id.as_str(), "promotion_receipt.run_id")?;
        ensure_nonempty(
            self.aggregate_window_id.as_str(),
            "promotion_receipt.aggregate_window_id",
        )?;
        ensure_nonempty(
            self.accepted_aggregate_id.as_str(),
            "promotion_receipt.accepted_aggregate_id",
        )?;
        ensure_sha256_uri(
            self.aggregated_delta_digest.as_str(),
            "promotion_receipt.aggregated_delta_digest",
        )?;
        ensure_nonempty(
            self.output_checkpoint_pointer.as_str(),
            "promotion_receipt.output_checkpoint_pointer",
        )?;
        ensure_sha256_uri(
            self.output_checkpoint_digest.as_str(),
            "promotion_receipt.output_checkpoint_digest",
        )?;
        ensure_nonempty(
            self.promoted_checkpoint_ref.as_str(),
            "promotion_receipt.promoted_checkpoint_ref",
        )?;
        ensure_finite(
            self.validation_loss_before,
            "promotion_receipt.validation_loss_before",
        )?;
        ensure_finite(
            self.validation_loss_after,
            "promotion_receipt.validation_loss_after",
        )?;
        if self.accepted_contribution_count < 2
            || self.model_progress_participant_count != self.accepted_contribution_count
        {
            return invalid_aggregation(String::from(
                "promotion receipt must count at least two model-progress participants",
            ));
        }
        if self.promotion_verdict != "promoted" {
            return invalid_aggregation(String::from("promotion receipt must be promoted"));
        }
        ensure_sha256_uri(
            self.promotion_receipt_digest.as_str(),
            "promotion_receipt.promotion_receipt_digest",
        )?;
        if self.promotion_receipt_digest != self.stable_digest() {
            return invalid_aggregation(String::from("promotion receipt digest drifted"));
        }
        if !self.claim_boundary.contains("trusted aggregation fixture") {
            return invalid_aggregation(String::from(
                "promotion receipt must keep trusted-fixture claim boundary",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmTrustedAggregationReport {
    pub schema_version: String,
    pub lane_id: String,
    pub run_id: String,
    pub aggregate_window_id: String,
    pub trusted_aggregation_rule: String,
    pub tokenizer_digest: String,
    pub tokenized_dataset_digest: String,
    pub validation_set_digest: String,
    pub model_config_digest: String,
    pub optimizer_config_digest: String,
    pub scheduler_config_digest: String,
    pub base_checkpoint_ref: String,
    pub base_checkpoint_digest: String,
    pub accepted_aggregate_id: String,
    pub accepted_local_updates: Vec<A1MinimalDistributedLmAcceptedLocalUpdate>,
    pub accepted_contribution_count: usize,
    pub model_progress_participant_count: usize,
    pub aggregation_weight_basis: String,
    pub total_aggregation_weight: u64,
    pub aggregated_delta_digest: String,
    pub output_checkpoint_pointer: String,
    pub output_checkpoint_digest: String,
    pub promoted_checkpoint_ref: String,
    pub validation_loss_before: f32,
    pub validation_loss_after: f32,
    pub promotion_receipt_digest: String,
    pub aggregate_report_digest: String,
    pub claim_boundary: String,
}

impl A1MinimalDistributedLmTrustedAggregationReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.aggregate_report_digest.clear();
        sha256_uri_digest(
            b"psion_a1_minimal_distributed_lm_trusted_aggregation_report|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), A1MinimalDistributedLmTrustedAggregationError> {
        if self.schema_version
            != A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATION_REPORT_SCHEMA_VERSION
        {
            return invalid_aggregation(String::from("trusted aggregation report schema drifted"));
        }
        validate_lane_identity(self.lane_id.as_str())?;
        ensure_nonempty(self.run_id.as_str(), "aggregation_report.run_id")?;
        ensure_nonempty(
            self.aggregate_window_id.as_str(),
            "aggregation_report.aggregate_window_id",
        )?;
        if self.trusted_aggregation_rule != "trusted_weighted_delta_average_v1" {
            return invalid_aggregation(String::from("aggregation rule drifted"));
        }
        ensure_sha256_uri(
            self.tokenizer_digest.as_str(),
            "aggregation_report.tokenizer_digest",
        )?;
        ensure_sha256_uri(
            self.tokenized_dataset_digest.as_str(),
            "aggregation_report.tokenized_dataset_digest",
        )?;
        ensure_sha256_uri(
            self.validation_set_digest.as_str(),
            "aggregation_report.validation_set_digest",
        )?;
        ensure_sha256_uri(
            self.model_config_digest.as_str(),
            "aggregation_report.model_config_digest",
        )?;
        ensure_sha256_uri(
            self.optimizer_config_digest.as_str(),
            "aggregation_report.optimizer_config_digest",
        )?;
        ensure_sha256_uri(
            self.scheduler_config_digest.as_str(),
            "aggregation_report.scheduler_config_digest",
        )?;
        ensure_nonempty(
            self.base_checkpoint_ref.as_str(),
            "aggregation_report.base_checkpoint_ref",
        )?;
        ensure_sha256_uri(
            self.base_checkpoint_digest.as_str(),
            "aggregation_report.base_checkpoint_digest",
        )?;
        ensure_nonempty(
            self.accepted_aggregate_id.as_str(),
            "aggregation_report.accepted_aggregate_id",
        )?;
        if self.accepted_local_updates.len() < 2
            || self.accepted_contribution_count != self.accepted_local_updates.len()
            || self.model_progress_participant_count != self.accepted_local_updates.len()
        {
            return invalid_aggregation(String::from(
                "aggregation report must retain at least two accepted model-progress updates",
            ));
        }
        let mut total_weight = 0_u64;
        let mut seen_contributions = BTreeMap::new();
        for update in &self.accepted_local_updates {
            update.validate()?;
            if update.run_id != self.run_id
                || update.window_id != self.aggregate_window_id
                || update.tokenizer_digest != self.tokenizer_digest
                || update.tokenized_dataset_digest != self.tokenized_dataset_digest
                || update.validation_set_digest != self.validation_set_digest
                || update.base_checkpoint_ref != self.base_checkpoint_ref
                || update.base_checkpoint_digest != self.base_checkpoint_digest
            {
                return invalid_aggregation(String::from(
                    "accepted local update is incompatible with aggregation report",
                ));
            }
            if seen_contributions
                .insert(update.contribution_id.clone(), ())
                .is_some()
            {
                return invalid_aggregation(String::from(
                    "accepted local update contribution_id duplicated",
                ));
            }
            total_weight = total_weight.saturating_add(update.aggregation_weight_value);
        }
        if self.aggregation_weight_basis != "tokens"
            || self.total_aggregation_weight != total_weight
        {
            return invalid_aggregation(String::from("aggregation report total weight drifted"));
        }
        ensure_sha256_uri(
            self.aggregated_delta_digest.as_str(),
            "aggregation_report.aggregated_delta_digest",
        )?;
        ensure_nonempty(
            self.output_checkpoint_pointer.as_str(),
            "aggregation_report.output_checkpoint_pointer",
        )?;
        ensure_sha256_uri(
            self.output_checkpoint_digest.as_str(),
            "aggregation_report.output_checkpoint_digest",
        )?;
        ensure_nonempty(
            self.promoted_checkpoint_ref.as_str(),
            "aggregation_report.promoted_checkpoint_ref",
        )?;
        ensure_finite(
            self.validation_loss_before,
            "aggregation_report.validation_loss_before",
        )?;
        ensure_finite(
            self.validation_loss_after,
            "aggregation_report.validation_loss_after",
        )?;
        if self.validation_loss_after > self.validation_loss_before {
            return invalid_aggregation(String::from(
                "trusted aggregate must not promote worse validation loss",
            ));
        }
        ensure_sha256_uri(
            self.promotion_receipt_digest.as_str(),
            "aggregation_report.promotion_receipt_digest",
        )?;
        ensure_sha256_uri(
            self.aggregate_report_digest.as_str(),
            "aggregation_report.aggregate_report_digest",
        )?;
        if self.aggregate_report_digest != self.stable_digest() {
            return invalid_aggregation(String::from("aggregation report digest drifted"));
        }
        if !self
            .claim_boundary
            .contains("trusted local-update aggregation")
        {
            return invalid_aggregation(String::from(
                "aggregation report must keep trusted aggregation claim boundary",
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
struct GeneratedLocalUpdate {
    report: A1MinimalDistributedLmLocalUpdateReport,
    manifest: A1MinimalDistributedLmLocalUpdateArtifactManifest,
    receipt: A1MinimalDistributedLmLocalUpdateContributionReceipt,
    checkpoint: A1MinimalDistributedLmLocalUpdateCheckpoint,
}

pub fn write_a1_minimal_distributed_lm_trusted_aggregation_fixture(
    output_root: impl AsRef<Path>,
) -> Result<
    A1MinimalDistributedLmTrustedAggregationReport,
    A1MinimalDistributedLmTrustedAggregationError,
> {
    run_and_write_a1_minimal_distributed_lm_trusted_aggregation(
        output_root,
        A1MinimalDistributedLmTrustedAggregationConfig::default(),
    )
}

pub fn run_and_write_a1_minimal_distributed_lm_trusted_aggregation(
    output_root: impl AsRef<Path>,
    config: A1MinimalDistributedLmTrustedAggregationConfig,
) -> Result<
    A1MinimalDistributedLmTrustedAggregationReport,
    A1MinimalDistributedLmTrustedAggregationError,
> {
    let output_root = output_root.as_ref();
    let (report, checkpoint, promotion) = build_trusted_aggregation_artifacts(config)?;
    write_json(
        output_root,
        A1_MINIMAL_DISTRIBUTED_LM_AGGREGATE_CHECKPOINT_FIXTURE_PATH,
        &checkpoint,
    )?;
    write_json(
        output_root,
        A1_MINIMAL_DISTRIBUTED_LM_PROMOTION_RECEIPT_FIXTURE_PATH,
        &promotion,
    )?;
    write_json(
        output_root,
        A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATION_REPORT_FIXTURE_PATH,
        &report,
    )?;
    Ok(report)
}

pub fn load_a1_minimal_distributed_lm_trusted_aggregation_report(
    path: impl AsRef<Path>,
) -> Result<
    A1MinimalDistributedLmTrustedAggregationReport,
    A1MinimalDistributedLmTrustedAggregationError,
> {
    let report: A1MinimalDistributedLmTrustedAggregationReport = read_json(path.as_ref())?;
    report.validate()?;
    Ok(report)
}

pub fn load_a1_minimal_distributed_lm_aggregate_checkpoint(
    path: impl AsRef<Path>,
) -> Result<A1MinimalDistributedLmAggregateCheckpoint, A1MinimalDistributedLmTrustedAggregationError>
{
    let checkpoint: A1MinimalDistributedLmAggregateCheckpoint = read_json(path.as_ref())?;
    checkpoint.validate()?;
    Ok(checkpoint)
}

pub fn load_a1_minimal_distributed_lm_promotion_receipt(
    path: impl AsRef<Path>,
) -> Result<A1MinimalDistributedLmPromotionReceipt, A1MinimalDistributedLmTrustedAggregationError> {
    let receipt: A1MinimalDistributedLmPromotionReceipt = read_json(path.as_ref())?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_trusted_aggregation_artifacts(
    config: A1MinimalDistributedLmTrustedAggregationConfig,
) -> Result<
    (
        A1MinimalDistributedLmTrustedAggregationReport,
        A1MinimalDistributedLmAggregateCheckpoint,
        A1MinimalDistributedLmPromotionReceipt,
    ),
    A1MinimalDistributedLmTrustedAggregationError,
> {
    config.validate()?;
    let contract = canonical_a1_minimal_distributed_lm_lane_contract();
    contract.validate().map_err(|error| {
        A1MinimalDistributedLmTrustedAggregationError::Invalid {
            detail: format!("lane contract invalid: {error}"),
        }
    })?;
    let bundle = canonical_a1_minimal_distributed_lm_tokenizer_dataset_bundle()?;
    bundle.validate()?;
    let local_updates = [
        generate_local_update(&config, 0, 0)?,
        generate_local_update(
            &config,
            1,
            contract.aggregation_rule.local_update_window_steps,
        )?,
    ];
    let accepted_updates = local_updates
        .iter()
        .map(|update| accepted_update_from_generated(update))
        .collect::<Result<Vec<_>, _>>()?;
    let base_model_state = base_model_state(&contract)?;
    let (aggregate_state, aggregated_delta_digest) =
        aggregate_lm_head_weighted_average(&base_model_state, &local_updates, &accepted_updates)?;
    let validation_loss_before = validation_loss(&contract, &bundle, &base_model_state)?;
    let validation_loss_after = validation_loss(&contract, &bundle, &aggregate_state)?;
    if validation_loss_after > validation_loss_before {
        return invalid_aggregation(String::from(
            "trusted aggregate did not improve validation loss",
        ));
    }
    let base_checkpoint_digest = accepted_updates[0].base_checkpoint_digest.clone();
    let mut checkpoint = A1MinimalDistributedLmAggregateCheckpoint {
        schema_version: String::from(A1_MINIMAL_DISTRIBUTED_LM_AGGREGATE_CHECKPOINT_SCHEMA_VERSION),
        lane_id: String::from(A1_MINIMAL_DISTRIBUTED_LM_LANE_ID),
        run_id: config.run_id.clone(),
        aggregate_window_id: config.aggregate_window_id.clone(),
        accepted_aggregate_id: config.accepted_aggregate_id.clone(),
        tokenizer_digest: contract.tokenizer_artifact_digest.clone(),
        tokenized_dataset_digest: contract.tokenized_dataset_digest.clone(),
        validation_set_digest: contract.validation_set_digest.clone(),
        base_checkpoint_ref: contract.checkpoint_family.base_checkpoint_ref.clone(),
        base_checkpoint_digest: base_checkpoint_digest.clone(),
        aggregated_delta_digest: aggregated_delta_digest.clone(),
        output_checkpoint_pointer: config.output_checkpoint_pointer.clone(),
        promoted_checkpoint_ref: config.promoted_checkpoint_ref.clone(),
        validation_loss_before,
        validation_loss_after,
        model_state: aggregate_state,
        checkpoint_digest: String::new(),
    };
    checkpoint.checkpoint_digest = checkpoint.stable_digest();
    checkpoint.validate()?;

    let mut promotion = A1MinimalDistributedLmPromotionReceipt {
        schema_version: String::from(A1_MINIMAL_DISTRIBUTED_LM_PROMOTION_RECEIPT_SCHEMA_VERSION),
        lane_id: String::from(A1_MINIMAL_DISTRIBUTED_LM_LANE_ID),
        run_id: config.run_id.clone(),
        aggregate_window_id: config.aggregate_window_id.clone(),
        accepted_aggregate_id: config.accepted_aggregate_id.clone(),
        aggregated_delta_digest: aggregated_delta_digest.clone(),
        output_checkpoint_pointer: config.output_checkpoint_pointer.clone(),
        output_checkpoint_digest: checkpoint.checkpoint_digest.clone(),
        promoted_checkpoint_ref: config.promoted_checkpoint_ref.clone(),
        validation_loss_before,
        validation_loss_after,
        accepted_contribution_count: accepted_updates.len(),
        model_progress_participant_count: accepted_updates.len(),
        promotion_verdict: String::from("promoted"),
        promotion_receipt_digest: String::new(),
        claim_boundary: String::from(
            "This is a trusted aggregation fixture for a1_minimal_distributed_lm_001, not permissionless public model-progress acceptance.",
        ),
    };
    promotion.promotion_receipt_digest = promotion.stable_digest();
    promotion.validate()?;

    let total_weight = accepted_updates
        .iter()
        .map(|update| update.aggregation_weight_value)
        .sum::<u64>();
    let mut report = A1MinimalDistributedLmTrustedAggregationReport {
        schema_version: String::from(
            A1_MINIMAL_DISTRIBUTED_LM_TRUSTED_AGGREGATION_REPORT_SCHEMA_VERSION,
        ),
        lane_id: String::from(A1_MINIMAL_DISTRIBUTED_LM_LANE_ID),
        run_id: config.run_id,
        aggregate_window_id: config.aggregate_window_id,
        trusted_aggregation_rule: contract.aggregation_rule.rule_id.clone(),
        tokenizer_digest: contract.tokenizer_artifact_digest.clone(),
        tokenized_dataset_digest: contract.tokenized_dataset_digest.clone(),
        validation_set_digest: contract.validation_set_digest.clone(),
        model_config_digest: sha256_uri_digest(
            b"psion_a1_minimal_distributed_lm_model_config|",
            &contract.model_config,
        ),
        optimizer_config_digest: sha256_uri_digest(
            b"psion_a1_minimal_distributed_lm_optimizer_config|",
            &contract.optimizer_config,
        ),
        scheduler_config_digest: sha256_uri_digest(
            b"psion_a1_minimal_distributed_lm_scheduler_config|",
            &contract.scheduler_config,
        ),
        base_checkpoint_ref: contract.checkpoint_family.base_checkpoint_ref,
        base_checkpoint_digest,
        accepted_aggregate_id: config.accepted_aggregate_id,
        accepted_local_updates: accepted_updates,
        accepted_contribution_count: local_updates.len(),
        model_progress_participant_count: local_updates.len(),
        aggregation_weight_basis: String::from("tokens"),
        total_aggregation_weight: total_weight,
        aggregated_delta_digest,
        output_checkpoint_pointer: config.output_checkpoint_pointer,
        output_checkpoint_digest: checkpoint.checkpoint_digest.clone(),
        promoted_checkpoint_ref: config.promoted_checkpoint_ref,
        validation_loss_before,
        validation_loss_after,
        promotion_receipt_digest: promotion.promotion_receipt_digest.clone(),
        aggregate_report_digest: String::new(),
        claim_boundary: String::from(
            "This report proves trusted local-update aggregation for the fixed A1 minimal LM lane. It is not permissionless public model-progress acceptance.",
        ),
    };
    report.aggregate_report_digest = report.stable_digest();
    report.validate()?;
    Ok((report, checkpoint, promotion))
}

fn generate_local_update(
    config: &A1MinimalDistributedLmTrustedAggregationConfig,
    index: usize,
    start_cursor: u64,
) -> Result<GeneratedLocalUpdate, A1MinimalDistributedLmTrustedAggregationError> {
    let scratch = scratch_root(index)?;
    if scratch.exists() {
        fs::remove_dir_all(scratch.as_path()).map_err(|error| {
            A1MinimalDistributedLmTrustedAggregationError::Write {
                path: scratch.display().to_string(),
                error,
            }
        })?;
    }
    fs::create_dir_all(scratch.as_path()).map_err(|error| {
        A1MinimalDistributedLmTrustedAggregationError::CreateDir {
            path: scratch.display().to_string(),
            error,
        }
    })?;
    let ordinal = index + 1;
    let local_config = A1MinimalDistributedLmLocalUpdateConfig {
        run_id: config.run_id.clone(),
        window_id: config.aggregate_window_id.clone(),
        stage_id: String::from(
            "stage.a1_minimal_distributed_lm_001.trusted_aggregate.local_update",
        ),
        assignment_id: format!(
            "a1_minimal_distributed_lm_001.trusted_aggregation.assignment.{ordinal:04}"
        ),
        worker_id: format!("psionic.trusted_aggregation.fixture.worker.{ordinal:04}"),
        node_pubkey: format!("pylon-node-pubkey.fixture.a1_minimal_distributed_lm.{ordinal:04}"),
        contribution_id: format!(
            "contribution.a1_minimal_distributed_lm_001.trusted_aggregation.{ordinal:04}"
        ),
        contributor_set_revision_id: String::from(
            "contributors.a1_minimal_distributed_lm_001.trusted_aggregation.revision.000001",
        ),
        start_deterministic_cursor: start_cursor,
        local_step_count: 4,
        checkpoint_after_steps: 2,
    };
    let report =
        run_and_write_a1_minimal_distributed_lm_local_update(scratch.as_path(), local_config)?;
    let manifest = load_a1_minimal_distributed_lm_local_update_artifact_manifest(
        scratch.join(A1_MINIMAL_DISTRIBUTED_LM_LOCAL_UPDATE_ARTIFACT_MANIFEST_FIXTURE_PATH),
    )?;
    let receipt = load_a1_minimal_distributed_lm_local_update_contribution_receipt(
        scratch.join(A1_MINIMAL_DISTRIBUTED_LM_LOCAL_UPDATE_CONTRIBUTION_RECEIPT_FIXTURE_PATH),
    )?;
    let checkpoint = load_a1_minimal_distributed_lm_local_update_checkpoint(
        scratch.join(A1_MINIMAL_DISTRIBUTED_LM_LOCAL_UPDATE_CHECKPOINT_STEP4_FIXTURE_PATH),
    )?;
    fs::remove_dir_all(scratch.as_path()).map_err(|error| {
        A1MinimalDistributedLmTrustedAggregationError::Write {
            path: scratch.display().to_string(),
            error,
        }
    })?;
    Ok(GeneratedLocalUpdate {
        report,
        manifest,
        receipt,
        checkpoint,
    })
}

fn accepted_update_from_generated(
    generated: &GeneratedLocalUpdate,
) -> Result<A1MinimalDistributedLmAcceptedLocalUpdate, A1MinimalDistributedLmTrustedAggregationError>
{
    generated.report.validate()?;
    generated.manifest.validate()?;
    generated.receipt.validate()?;
    generated.checkpoint.validate()?;
    let mut update = A1MinimalDistributedLmAcceptedLocalUpdate {
        run_id: generated.report.run_id.clone(),
        window_id: generated.manifest.window_id.clone(),
        assignment_id: generated.report.assignment_id.clone(),
        contribution_id: generated.manifest.contribution_id.clone(),
        worker_id: generated.report.worker_id.clone(),
        node_pubkey: generated.manifest.node_pubkey.clone(),
        tokenizer_digest: generated.report.tokenizer_digest.clone(),
        tokenized_dataset_digest: generated.report.tokenized_dataset_digest.clone(),
        validation_set_digest: generated.report.validation_set_digest.clone(),
        input_shard: generated.manifest.input_shard.clone(),
        input_token_range: generated.manifest.input_token_range.clone(),
        base_checkpoint_ref: generated.report.base_checkpoint_ref.clone(),
        base_checkpoint_digest: generated.manifest.base_checkpoint_digest.clone(),
        local_step_count: generated.report.local_step_count,
        consumed_token_count: generated.report.consumed_token_count,
        loss_before: generated.report.loss_before,
        loss_after: generated.report.loss_after,
        output_checkpoint_ref: generated.manifest.output_checkpoint_ref.clone(),
        output_checkpoint_digest: generated.checkpoint.checkpoint_digest.clone(),
        output_delta_ref: generated.manifest.output_delta_ref.clone(),
        output_delta_digest: generated.report.delta_digest.clone(),
        source_report_digest: generated.report.report_digest.clone(),
        source_artifact_manifest_digest: generated.manifest.artifact_manifest_digest.clone(),
        source_contribution_digest: generated.receipt.contribution_digest.clone(),
        validator_disposition: String::from("accepted"),
        accepted_for_aggregation: true,
        aggregation_weight_basis: String::from("tokens"),
        aggregation_weight_value: generated.report.consumed_token_count,
        accepted_local_update_digest: String::new(),
    };
    update.accepted_local_update_digest = update.stable_digest();
    update.validate()?;
    Ok(update)
}

fn scratch_root(index: usize) -> Result<PathBuf, A1MinimalDistributedLmTrustedAggregationError> {
    let path = std::env::temp_dir().join(format!(
        "psionic_a1_minimal_lm_trusted_aggregation_{}_{}",
        std::process::id(),
        index
    ));
    Ok(path)
}

fn base_model_state(
    contract: &A1MinimalDistributedLmLaneContract,
) -> Result<ModuleStateDict, A1MinimalDistributedLmTrustedAggregationError> {
    let mut model = new_model_from_contract(contract)?;
    initialize_cs336_a1_reference_model(&mut model)?;
    Ok(model.state_dict())
}

fn aggregate_lm_head_weighted_average(
    base_state: &ModuleStateDict,
    local_updates: &[GeneratedLocalUpdate],
    accepted_updates: &[A1MinimalDistributedLmAcceptedLocalUpdate],
) -> Result<(ModuleStateDict, String), A1MinimalDistributedLmTrustedAggregationError> {
    if local_updates.len() != accepted_updates.len() || local_updates.len() < 2 {
        return invalid_aggregation(String::from(
            "trusted aggregation requires at least two local updates",
        ));
    }
    let base_values = lm_head_values(base_state)?;
    let mut aggregate_delta = vec![0.0_f32; base_values.len()];
    let total_weight = accepted_updates
        .iter()
        .map(|update| update.aggregation_weight_value)
        .sum::<u64>();
    if total_weight == 0 {
        return invalid_aggregation(String::from("total aggregation weight is zero"));
    }
    for (generated, accepted) in local_updates.iter().zip(accepted_updates.iter()) {
        let values = lm_head_values(&generated.checkpoint.model_state)?;
        if values.len() != base_values.len() {
            return invalid_aggregation(String::from(
                "local update lm_head shape does not match base",
            ));
        }
        let scale = accepted.aggregation_weight_value as f32 / total_weight as f32;
        for ((slot, base), value) in aggregate_delta
            .iter_mut()
            .zip(base_values.iter())
            .zip(values.iter())
        {
            *slot += (*value - *base) * scale;
        }
    }
    let mut aggregate_state = base_state.clone();
    let aggregate_values = dense_tensor_values_mut(
        aggregate_state
            .entries
            .get_mut("lm_head.weight")
            .ok_or_else(|| A1MinimalDistributedLmTrustedAggregationError::Invalid {
                detail: String::from("base state missing lm_head.weight"),
            })?,
    )?;
    for (value, delta) in aggregate_values.iter_mut().zip(aggregate_delta.iter()) {
        *value += *delta;
    }
    aggregate_state = ModuleStateDict::new(
        aggregate_state.root_module_id,
        aggregate_state.root_module_kind,
        aggregate_state.view,
        aggregate_state.entries,
    )?;
    let digest = sha256_uri_digest(
        b"psion_a1_minimal_distributed_lm_aggregated_lm_head_delta|",
        &(String::from("lm_head.weight"), aggregate_delta),
    );
    Ok((aggregate_state, digest))
}

fn validation_loss(
    contract: &A1MinimalDistributedLmLaneContract,
    bundle: &A1MinimalDistributedLmTokenizerDatasetBundle,
    state: &ModuleStateDict,
) -> Result<f32, A1MinimalDistributedLmTrustedAggregationError> {
    let mut model = new_model_from_contract(contract)?;
    model.load_state_dict(state, ModuleStateLoadMode::Strict)?;
    let validation_tokens = bundle
        .validation_shards
        .first()
        .ok_or_else(|| A1MinimalDistributedLmTrustedAggregationError::Invalid {
            detail: String::from("tokenizer/dataset bundle has no validation shard"),
        })?
        .tokens
        .clone();
    if validation_tokens.len() <= contract.model_config.context_length {
        return invalid_aggregation(String::from("validation token window too short"));
    }
    let window_count = validation_tokens.len() - contract.model_config.context_length;
    let mut total = 0.0;
    for start in 0..window_count {
        let batch = Cs336A1ReferenceBatch {
            iteration: start as u64,
            batch_size: 1,
            context_length: contract.model_config.context_length,
            start_positions: vec![start],
            inputs: validation_tokens[start..start + contract.model_config.context_length].to_vec(),
            targets: validation_tokens[start + 1..start + contract.model_config.context_length + 1]
                .to_vec(),
        };
        let logits = model.forward_tokens(batch.token_shape(), &batch.input_ids())?;
        let loss = cross_entropy_loss(&logits, &batch.target_ids(), LossReduction::Mean)?;
        total += scalar_from_nn_tensor(&loss)?;
    }
    Ok(total / window_count as f32)
}

fn new_model_from_contract(
    contract: &A1MinimalDistributedLmLaneContract,
) -> Result<Cs336A1TransformerLm, A1MinimalDistributedLmTrustedAggregationError> {
    let config = Cs336A1ReferenceConfig {
        vocab_size: contract.model_config.vocab_size as usize,
        context_length: contract.model_config.context_length,
        d_model: contract.model_config.d_model,
        num_layers: contract.model_config.num_layers,
        num_heads: contract.model_config.num_heads,
        d_ff: contract.model_config.d_ff,
    };
    Ok(Cs336A1TransformerLm::new(
        "a1_minimal_distributed_lm_trusted_aggregation",
        config,
        contract.model_config.rope_theta,
        contract.model_config.rms_norm_eps,
    )?)
}

fn lm_head_values(
    state: &ModuleStateDict,
) -> Result<&[f32], A1MinimalDistributedLmTrustedAggregationError> {
    let entry = state.entries.get("lm_head.weight").ok_or_else(|| {
        A1MinimalDistributedLmTrustedAggregationError::Invalid {
            detail: String::from("state missing lm_head.weight"),
        }
    })?;
    dense_tensor_values(&entry.data)
}

fn dense_tensor_values(
    data: &TensorData,
) -> Result<&[f32], A1MinimalDistributedLmTrustedAggregationError> {
    match data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.as_slice()),
        other => invalid_aggregation(format!("expected dense floating tensor, found `{other:?}`")),
    }
}

fn dense_tensor_values_mut(
    entry: &mut ModuleStateEntry,
) -> Result<&mut Vec<f32>, A1MinimalDistributedLmTrustedAggregationError> {
    match &mut entry.data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values),
        other => invalid_aggregation(format!(
            "expected mutable dense floating tensor at `{}`, found `{other:?}`",
            entry.path
        )),
    }
}

fn scalar_from_nn_tensor(
    tensor: &NnTensor,
) -> Result<f32, A1MinimalDistributedLmTrustedAggregationError> {
    let values = tensor.as_f32_slice()?;
    if values.len() != 1 {
        return invalid_aggregation(format!("expected scalar tensor, found {}", values.len()));
    }
    Ok(values[0])
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
) -> Result<T, A1MinimalDistributedLmTrustedAggregationError> {
    let bytes =
        fs::read(path).map_err(
            |error| A1MinimalDistributedLmTrustedAggregationError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_json<T: Serialize>(
    output_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<(), A1MinimalDistributedLmTrustedAggregationError> {
    let output_path = output_root.join(relative_path);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            A1MinimalDistributedLmTrustedAggregationError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(output_path.as_path(), bytes).map_err(|error| {
        A1MinimalDistributedLmTrustedAggregationError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })
}

fn validate_lane_identity(
    lane_id: &str,
) -> Result<(), A1MinimalDistributedLmTrustedAggregationError> {
    if lane_id != A1_MINIMAL_DISTRIBUTED_LM_LANE_ID {
        return invalid_aggregation(format!(
            "lane_id must stay `{}` but was `{lane_id}`",
            A1_MINIMAL_DISTRIBUTED_LM_LANE_ID
        ));
    }
    Ok(())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), A1MinimalDistributedLmTrustedAggregationError> {
    if value.trim().is_empty() {
        return invalid_aggregation(format!("field `{field}` must not be empty"));
    }
    Ok(())
}

fn ensure_finite(
    value: f32,
    field: &str,
) -> Result<(), A1MinimalDistributedLmTrustedAggregationError> {
    if !value.is_finite() {
        return invalid_aggregation(format!("field `{field}` must be finite"));
    }
    Ok(())
}

fn ensure_sha256_uri(
    value: &str,
    field: &str,
) -> Result<(), A1MinimalDistributedLmTrustedAggregationError> {
    ensure_nonempty(value, field)?;
    let Some(hex) = value.strip_prefix("sha256:") else {
        return invalid_aggregation(format!("field `{field}` must use sha256:<hex> form"));
    };
    if hex.len() != 64 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return invalid_aggregation(format!(
            "field `{field}` must contain a 64-hex sha256 digest"
        ));
    }
    Ok(())
}

fn invalid_aggregation<T>(
    detail: String,
) -> Result<T, A1MinimalDistributedLmTrustedAggregationError> {
    Err(A1MinimalDistributedLmTrustedAggregationError::Invalid { detail })
}

fn sha256_uri_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("A1 minimal distributed LM trusted aggregation payload should serialize"),
    );
    format!("sha256:{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        load_a1_minimal_distributed_lm_aggregate_checkpoint,
        load_a1_minimal_distributed_lm_promotion_receipt,
        load_a1_minimal_distributed_lm_trusted_aggregation_report,
        write_a1_minimal_distributed_lm_trusted_aggregation_fixture,
        A1MinimalDistributedLmAggregateCheckpoint, A1MinimalDistributedLmPromotionReceipt,
        A1MinimalDistributedLmTrustedAggregationError,
        A1MinimalDistributedLmTrustedAggregationReport,
    };
    use tempfile::tempdir;

    fn fixture_report() -> A1MinimalDistributedLmTrustedAggregationReport {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/a1_minimal_distributed_lm/trusted_aggregation_report_v1.json"
        ))
        .expect("A1 minimal distributed LM trusted aggregation report fixture should parse")
    }

    fn fixture_checkpoint() -> A1MinimalDistributedLmAggregateCheckpoint {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/a1_minimal_distributed_lm/aggregate_checkpoint_v1.json"
        ))
        .expect("A1 minimal distributed LM aggregate checkpoint fixture should parse")
    }

    fn fixture_promotion() -> A1MinimalDistributedLmPromotionReceipt {
        serde_json::from_str(include_str!(
            "../../../fixtures/psion/a1_minimal_distributed_lm/promotion_receipt_v1.json"
        ))
        .expect("A1 minimal distributed LM promotion receipt fixture should parse")
    }

    #[test]
    fn a1_minimal_distributed_lm_trusted_aggregation_fixture_validates() {
        fixture_report()
            .validate()
            .expect("trusted aggregation report should validate");
        fixture_checkpoint()
            .validate()
            .expect("aggregate checkpoint should validate");
        fixture_promotion()
            .validate()
            .expect("promotion receipt should validate");
    }

    #[test]
    fn a1_minimal_distributed_lm_trusted_aggregation_writer_is_reproducible(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let root = tempdir()?;
        let generated = write_a1_minimal_distributed_lm_trusted_aggregation_fixture(root.path())?;
        assert_eq!(generated, fixture_report());
        let loaded_report =
            load_a1_minimal_distributed_lm_trusted_aggregation_report(root.path().join(
                "fixtures/psion/a1_minimal_distributed_lm/trusted_aggregation_report_v1.json",
            ))?;
        assert_eq!(loaded_report, fixture_report());
        let loaded_checkpoint = load_a1_minimal_distributed_lm_aggregate_checkpoint(
            root.path()
                .join("fixtures/psion/a1_minimal_distributed_lm/aggregate_checkpoint_v1.json"),
        )?;
        assert_eq!(loaded_checkpoint, fixture_checkpoint());
        let loaded_promotion = load_a1_minimal_distributed_lm_promotion_receipt(
            root.path()
                .join("fixtures/psion/a1_minimal_distributed_lm/promotion_receipt_v1.json"),
        )?;
        assert_eq!(loaded_promotion, fixture_promotion());
        Ok(())
    }

    #[test]
    fn a1_minimal_distributed_lm_trusted_aggregation_maps_openagents_fields() {
        let report = fixture_report();
        let promotion = fixture_promotion();
        assert_eq!(
            report.accepted_aggregate_id,
            "aggregate.a1_minimal_distributed_lm_001.trusted_fixture.000001"
        );
        assert_eq!(report.accepted_contribution_count, 2);
        assert_eq!(report.model_progress_participant_count, 2);
        assert_eq!(report.aggregation_weight_basis, "tokens");
        assert_eq!(report.total_aggregation_weight, 16);
        assert_eq!(
            report.aggregated_delta_digest,
            promotion.aggregated_delta_digest
        );
        assert_eq!(
            report.output_checkpoint_pointer,
            promotion.output_checkpoint_pointer
        );
        assert_eq!(
            report.promoted_checkpoint_ref,
            promotion.promoted_checkpoint_ref
        );
        assert_eq!(promotion.promotion_verdict, "promoted");
        assert!(report.validation_loss_after <= report.validation_loss_before);
    }

    #[test]
    fn a1_minimal_distributed_lm_trusted_aggregation_rejects_mismatched_tokenizer() {
        let mut report = fixture_report();
        report.accepted_local_updates[0].tokenizer_digest =
            String::from("sha256:0000000000000000000000000000000000000000000000000000000000000000");
        report.accepted_local_updates[0].accepted_local_update_digest =
            report.accepted_local_updates[0].stable_digest();
        report.aggregate_report_digest = report.stable_digest();
        let error = report
            .validate()
            .expect_err("mismatched tokenizer should fail closed");
        assert!(matches!(
            error,
            A1MinimalDistributedLmTrustedAggregationError::Invalid { .. }
        ));
    }

    #[test]
    fn a1_minimal_distributed_lm_trusted_aggregation_rejects_mismatched_window() {
        let mut report = fixture_report();
        report.accepted_local_updates[0].window_id = String::from("window.stale");
        report.accepted_local_updates[0].accepted_local_update_digest =
            report.accepted_local_updates[0].stable_digest();
        report.aggregate_report_digest = report.stable_digest();
        let error = report
            .validate()
            .expect_err("mismatched window should fail closed");
        assert!(matches!(
            error,
            A1MinimalDistributedLmTrustedAggregationError::Invalid { .. }
        ));
    }
}
