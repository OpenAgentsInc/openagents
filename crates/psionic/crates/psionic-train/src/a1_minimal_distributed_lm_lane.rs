use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_a1_minimal_distributed_lm_tokenized_dataset_digest,
    canonical_a1_minimal_distributed_lm_tokenizer_digest,
    canonical_a1_minimal_distributed_lm_tokenizer_vocab_size,
    canonical_a1_minimal_distributed_lm_validation_set_digest, PSION_CS336_A1_DEMO_LANE_ID,
};

pub const A1_MINIMAL_DISTRIBUTED_LM_LANE_CONTRACT_SCHEMA_VERSION: &str =
    "psion.a1_minimal_distributed_lm.lane_contract.v1";
pub const A1_MINIMAL_DISTRIBUTED_LM_LANE_ID: &str = "a1_minimal_distributed_lm_001";
pub const A1_MINIMAL_DISTRIBUTED_LM_RELEASE_ID: &str =
    "psionic-train.a1_minimal_distributed_lm.release.v1";
pub const A1_MINIMAL_DISTRIBUTED_LM_ENVIRONMENT_REF: &str =
    "psionic.environment.a1_minimal_distributed_lm.tiny_lm.operator@v1";
pub const A1_MINIMAL_DISTRIBUTED_LM_LANE_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/a1_minimal_distributed_lm_lane_contract_v1.json";

#[derive(Debug, Error)]
pub enum A1MinimalDistributedLmLaneContractError {
    #[error("A1 minimal distributed LM lane contract is invalid: {detail}")]
    InvalidContract { detail: String },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmModelConfig {
    pub architecture: String,
    pub vocab_size: u32,
    pub context_length: usize,
    pub d_model: usize,
    pub num_layers: usize,
    pub num_heads: usize,
    pub d_ff: usize,
    pub rope_theta: f32,
    pub rms_norm_eps: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmOptimizerConfig {
    pub optimizer: String,
    pub adam_beta1: f32,
    pub adam_beta2: f32,
    pub adam_epsilon: f32,
    pub weight_decay: f32,
    pub gradient_clip_norm: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmSchedulerConfig {
    pub scheduler: String,
    pub max_learning_rate: f32,
    pub min_learning_rate: f32,
    pub warmup_iters: u64,
    pub cosine_cycle_iters: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmCheckpointFamily {
    pub checkpoint_family_id: String,
    pub base_checkpoint_ref: String,
    pub local_update_artifact_schema: String,
    pub aggregate_checkpoint_schema: String,
    pub promoted_checkpoint_pointer_schema: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmAggregationRule {
    pub rule_id: String,
    pub local_update_window_steps: u64,
    pub accepted_update_input: String,
    pub aggregate_output: String,
    pub promotion_gate: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmAggregationWeightBasis {
    pub basis_id: String,
    pub model_progress_weight_basis: String,
    pub support_work_weight_basis: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmContributionReceiptSchema {
    pub schema_version: String,
    pub required_fields: Vec<String>,
    pub model_progress_work_classes: Vec<String>,
    pub support_work_classes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmValidatorAcceptancePolicy {
    pub policy_id: String,
    pub required_checks: Vec<String>,
    pub rejection_checks: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmCloseoutAndPromotionSemantics {
    pub closeout_authority: String,
    pub participant_counter_source: String,
    pub model_progress_counter_source: String,
    pub promotion_semantics: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A1MinimalDistributedLmLaneContract {
    pub schema_version: String,
    pub lane_id: String,
    pub release_id: String,
    pub environment_ref: String,
    pub run_id_family: String,
    pub tokenizer_artifact_digest: String,
    pub tokenized_dataset_digest: String,
    pub validation_set_digest: String,
    pub model_config: A1MinimalDistributedLmModelConfig,
    pub optimizer_config: A1MinimalDistributedLmOptimizerConfig,
    pub scheduler_config: A1MinimalDistributedLmSchedulerConfig,
    pub checkpoint_family: A1MinimalDistributedLmCheckpointFamily,
    pub aggregation_rule: A1MinimalDistributedLmAggregationRule,
    pub aggregation_weight_basis: A1MinimalDistributedLmAggregationWeightBasis,
    pub contribution_receipt_schema: A1MinimalDistributedLmContributionReceiptSchema,
    pub validator_acceptance_policy: A1MinimalDistributedLmValidatorAcceptancePolicy,
    pub closeout_and_promotion: A1MinimalDistributedLmCloseoutAndPromotionSemantics,
    pub claim_boundary: String,
    pub contract_digest: String,
}

impl A1MinimalDistributedLmLaneContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        sha256_uri_digest(b"psion_a1_minimal_distributed_lm_lane_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), A1MinimalDistributedLmLaneContractError> {
        if self.schema_version != A1_MINIMAL_DISTRIBUTED_LM_LANE_CONTRACT_SCHEMA_VERSION {
            return invalid_contract(format!(
                "schema_version must stay `{}` but was `{}`",
                A1_MINIMAL_DISTRIBUTED_LM_LANE_CONTRACT_SCHEMA_VERSION, self.schema_version
            ));
        }
        if self.lane_id != A1_MINIMAL_DISTRIBUTED_LM_LANE_ID {
            return invalid_contract(format!(
                "lane_id must stay `{}` but was `{}`",
                A1_MINIMAL_DISTRIBUTED_LM_LANE_ID, self.lane_id
            ));
        }
        if self.lane_id == PSION_CS336_A1_DEMO_LANE_ID {
            return invalid_contract(String::from(
                "minimal distributed LM lane must not reuse the bounded CS336 A1 demo lane id",
            ));
        }
        if self.release_id != A1_MINIMAL_DISTRIBUTED_LM_RELEASE_ID {
            return invalid_contract(String::from("release_id drifted"));
        }
        if self.environment_ref != A1_MINIMAL_DISTRIBUTED_LM_ENVIRONMENT_REF {
            return invalid_contract(String::from("environment_ref drifted"));
        }
        ensure_nonempty(self.run_id_family.as_str(), "run_id_family")?;
        if self.run_id_family != A1_MINIMAL_DISTRIBUTED_LM_LANE_ID {
            return invalid_contract(String::from("run_id_family drifted"));
        }
        ensure_sha256_uri(
            self.tokenizer_artifact_digest.as_str(),
            "tokenizer_artifact_digest",
        )?;
        ensure_sha256_uri(
            self.tokenized_dataset_digest.as_str(),
            "tokenized_dataset_digest",
        )?;
        ensure_sha256_uri(self.validation_set_digest.as_str(), "validation_set_digest")?;
        if self.tokenizer_artifact_digest != expected_tokenizer_digest()? {
            return invalid_contract(String::from("tokenizer_artifact_digest drifted"));
        }
        if self.tokenized_dataset_digest != expected_tokenized_dataset_digest()? {
            return invalid_contract(String::from("tokenized_dataset_digest drifted"));
        }
        if self.validation_set_digest != expected_validation_set_digest()? {
            return invalid_contract(String::from("validation_set_digest drifted"));
        }
        ensure_sha256_uri(self.contract_digest.as_str(), "contract_digest")?;
        if self.contract_digest != self.stable_digest() {
            return invalid_contract(String::from("contract_digest does not match stable digest"));
        }
        self.model_config.validate()?;
        self.optimizer_config.validate()?;
        self.scheduler_config.validate()?;
        self.checkpoint_family.validate()?;
        self.aggregation_rule.validate()?;
        self.aggregation_weight_basis.validate()?;
        self.contribution_receipt_schema.validate()?;
        self.validator_acceptance_policy.validate()?;
        self.closeout_and_promotion.validate()?;
        ensure_nonempty(self.claim_boundary.as_str(), "claim_boundary")?;
        if !self.claim_boundary.contains("not OpenWebText leaderboard")
            || !self.claim_boundary.contains("not broad pretraining")
        {
            return invalid_contract(String::from(
                "claim_boundary must explicitly exclude OpenWebText leaderboard and broad pretraining claims",
            ));
        }
        Ok(())
    }
}

impl A1MinimalDistributedLmModelConfig {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmLaneContractError> {
        ensure_nonempty(self.architecture.as_str(), "model_config.architecture")?;
        if self.architecture != "tiny_transformer_lm" {
            return invalid_contract(String::from("model architecture drifted"));
        }
        if self.vocab_size < 256
            || self.context_length == 0
            || self.d_model == 0
            || self.num_layers == 0
            || self.num_heads == 0
            || self.d_ff == 0
        {
            return invalid_contract(String::from("model config dimensions must be nonzero"));
        }
        if self.d_model % self.num_heads != 0 {
            return invalid_contract(String::from("d_model must be divisible by num_heads"));
        }
        if self.rope_theta <= 0.0 || self.rms_norm_eps <= 0.0 {
            return invalid_contract(String::from(
                "model config numeric tolerances must be positive",
            ));
        }
        if self.vocab_size != expected_tokenizer_vocab_size()? {
            return invalid_contract(String::from(
                "model vocab_size must match the frozen tokenizer bundle",
            ));
        }
        Ok(())
    }
}

impl A1MinimalDistributedLmOptimizerConfig {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmLaneContractError> {
        ensure_nonempty(self.optimizer.as_str(), "optimizer_config.optimizer")?;
        if self.optimizer != "adamw" {
            return invalid_contract(String::from("optimizer must stay adamw"));
        }
        if !(0.0..1.0).contains(&self.adam_beta1)
            || !(0.0..1.0).contains(&self.adam_beta2)
            || self.adam_epsilon <= 0.0
            || self.weight_decay < 0.0
            || self.gradient_clip_norm <= 0.0
        {
            return invalid_contract(String::from("optimizer config has invalid numeric bounds"));
        }
        Ok(())
    }
}

impl A1MinimalDistributedLmSchedulerConfig {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmLaneContractError> {
        ensure_nonempty(self.scheduler.as_str(), "scheduler_config.scheduler")?;
        if self.scheduler != "linear_warmup_cosine_decay" {
            return invalid_contract(String::from(
                "scheduler must stay linear_warmup_cosine_decay",
            ));
        }
        if self.max_learning_rate <= 0.0
            || self.min_learning_rate <= 0.0
            || self.max_learning_rate < self.min_learning_rate
            || self.cosine_cycle_iters == 0
            || self.warmup_iters > self.cosine_cycle_iters
        {
            return invalid_contract(String::from("scheduler config has invalid numeric bounds"));
        }
        Ok(())
    }
}

impl A1MinimalDistributedLmCheckpointFamily {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmLaneContractError> {
        ensure_nonempty(
            self.checkpoint_family_id.as_str(),
            "checkpoint_family.checkpoint_family_id",
        )?;
        ensure_nonempty(
            self.base_checkpoint_ref.as_str(),
            "checkpoint_family.base_checkpoint_ref",
        )?;
        ensure_nonempty(
            self.local_update_artifact_schema.as_str(),
            "checkpoint_family.local_update_artifact_schema",
        )?;
        ensure_nonempty(
            self.aggregate_checkpoint_schema.as_str(),
            "checkpoint_family.aggregate_checkpoint_schema",
        )?;
        ensure_nonempty(
            self.promoted_checkpoint_pointer_schema.as_str(),
            "checkpoint_family.promoted_checkpoint_pointer_schema",
        )?;
        Ok(())
    }
}

impl A1MinimalDistributedLmAggregationRule {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmLaneContractError> {
        ensure_nonempty(self.rule_id.as_str(), "aggregation_rule.rule_id")?;
        ensure_nonempty(
            self.accepted_update_input.as_str(),
            "aggregation_rule.accepted_update_input",
        )?;
        ensure_nonempty(
            self.aggregate_output.as_str(),
            "aggregation_rule.aggregate_output",
        )?;
        ensure_nonempty(
            self.promotion_gate.as_str(),
            "aggregation_rule.promotion_gate",
        )?;
        if self.local_update_window_steps == 0 {
            return invalid_contract(String::from(
                "aggregation local_update_window_steps must be nonzero",
            ));
        }
        Ok(())
    }
}

impl A1MinimalDistributedLmAggregationWeightBasis {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmLaneContractError> {
        ensure_nonempty(self.basis_id.as_str(), "aggregation_weight_basis.basis_id")?;
        ensure_nonempty(
            self.model_progress_weight_basis.as_str(),
            "aggregation_weight_basis.model_progress_weight_basis",
        )?;
        ensure_nonempty(
            self.support_work_weight_basis.as_str(),
            "aggregation_weight_basis.support_work_weight_basis",
        )?;
        ensure_nonempty(self.detail.as_str(), "aggregation_weight_basis.detail")?;
        Ok(())
    }
}

impl A1MinimalDistributedLmContributionReceiptSchema {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmLaneContractError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "contribution_receipt_schema.schema_version",
        )?;
        ensure_vec_nonempty(
            self.required_fields.as_slice(),
            "contribution_receipt_schema.required_fields",
        )?;
        ensure_vec_nonempty(
            self.model_progress_work_classes.as_slice(),
            "contribution_receipt_schema.model_progress_work_classes",
        )?;
        ensure_vec_nonempty(
            self.support_work_classes.as_slice(),
            "contribution_receipt_schema.support_work_classes",
        )?;
        for required in [
            "worker_id",
            "assignment_id",
            "run_id",
            "input_shard",
            "base_checkpoint",
            "output_artifact",
            "loss_before",
            "loss_after",
            "validator_verdict",
        ] {
            if !self.required_fields.iter().any(|field| field == required) {
                return invalid_contract(format!(
                    "contribution receipt schema lost required field `{required}`"
                ));
            }
        }
        if !self
            .model_progress_work_classes
            .iter()
            .any(|class| class == "local_update_training")
        {
            return invalid_contract(String::from(
                "receipt schema must keep local_update_training as model-progress work",
            ));
        }
        Ok(())
    }
}

impl A1MinimalDistributedLmValidatorAcceptancePolicy {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmLaneContractError> {
        ensure_nonempty(
            self.policy_id.as_str(),
            "validator_acceptance_policy.policy_id",
        )?;
        ensure_vec_nonempty(
            self.required_checks.as_slice(),
            "validator_acceptance_policy.required_checks",
        )?;
        ensure_vec_nonempty(
            self.rejection_checks.as_slice(),
            "validator_acceptance_policy.rejection_checks",
        )?;
        Ok(())
    }
}

impl A1MinimalDistributedLmCloseoutAndPromotionSemantics {
    fn validate(&self) -> Result<(), A1MinimalDistributedLmLaneContractError> {
        ensure_nonempty(
            self.closeout_authority.as_str(),
            "closeout_and_promotion.closeout_authority",
        )?;
        ensure_nonempty(
            self.participant_counter_source.as_str(),
            "closeout_and_promotion.participant_counter_source",
        )?;
        ensure_nonempty(
            self.model_progress_counter_source.as_str(),
            "closeout_and_promotion.model_progress_counter_source",
        )?;
        ensure_nonempty(
            self.promotion_semantics.as_str(),
            "closeout_and_promotion.promotion_semantics",
        )?;
        if self.participant_counter_source != "training_accepted_contributors" {
            return invalid_contract(String::from(
                "participant counter must map to training_accepted_contributors",
            ));
        }
        if self.model_progress_counter_source != "training_model_progress_contributors" {
            return invalid_contract(String::from(
                "model-progress participant counter must map to training_model_progress_contributors",
            ));
        }
        Ok(())
    }
}

#[must_use]
pub fn canonical_a1_minimal_distributed_lm_lane_contract() -> A1MinimalDistributedLmLaneContract {
    let mut contract = A1MinimalDistributedLmLaneContract {
        schema_version: String::from(A1_MINIMAL_DISTRIBUTED_LM_LANE_CONTRACT_SCHEMA_VERSION),
        lane_id: String::from(A1_MINIMAL_DISTRIBUTED_LM_LANE_ID),
        release_id: String::from(A1_MINIMAL_DISTRIBUTED_LM_RELEASE_ID),
        environment_ref: String::from(A1_MINIMAL_DISTRIBUTED_LM_ENVIRONMENT_REF),
        run_id_family: String::from("a1_minimal_distributed_lm_001"),
        tokenizer_artifact_digest: expected_tokenizer_digest()
            .expect("canonical A1 minimal distributed LM tokenizer digest should resolve"),
        tokenized_dataset_digest: expected_tokenized_dataset_digest()
            .expect("canonical A1 minimal distributed LM tokenized dataset digest should resolve"),
        validation_set_digest: expected_validation_set_digest()
            .expect("canonical A1 minimal distributed LM validation digest should resolve"),
        model_config: A1MinimalDistributedLmModelConfig {
            architecture: String::from("tiny_transformer_lm"),
            vocab_size: expected_tokenizer_vocab_size()
                .expect("canonical A1 minimal distributed LM tokenizer vocab size should resolve"),
            context_length: 2,
            d_model: 2,
            num_layers: 1,
            num_heads: 1,
            d_ff: 4,
            rope_theta: 10_000.0,
            rms_norm_eps: 1e-5,
        },
        optimizer_config: A1MinimalDistributedLmOptimizerConfig {
            optimizer: String::from("adamw"),
            adam_beta1: 0.9,
            adam_beta2: 0.95,
            adam_epsilon: 1e-8,
            weight_decay: 0.0,
            gradient_clip_norm: 1.0,
        },
        scheduler_config: A1MinimalDistributedLmSchedulerConfig {
            scheduler: String::from("linear_warmup_cosine_decay"),
            max_learning_rate: 0.15,
            min_learning_rate: 0.02,
            warmup_iters: 1,
            cosine_cycle_iters: 4,
        },
        checkpoint_family: A1MinimalDistributedLmCheckpointFamily {
            checkpoint_family_id: String::from("psion.a1_minimal_distributed_lm.checkpoints.v1"),
            base_checkpoint_ref: String::from("base://a1_minimal_distributed_lm/step-000000"),
            local_update_artifact_schema: String::from(
                "psion.a1_minimal_distributed_lm.local_update_artifact.v1",
            ),
            aggregate_checkpoint_schema: String::from(
                "psion.a1_minimal_distributed_lm.aggregate_checkpoint.v1",
            ),
            promoted_checkpoint_pointer_schema: String::from(
                "psion.a1_minimal_distributed_lm.promoted_checkpoint_pointer.v1",
            ),
        },
        aggregation_rule: A1MinimalDistributedLmAggregationRule {
            rule_id: String::from("trusted_weighted_delta_average_v1"),
            local_update_window_steps: 4,
            accepted_update_input: String::from(
                "accepted local-update artifacts with base-checkpoint and shard digests",
            ),
            aggregate_output: String::from("one aggregate checkpoint plus validation-loss receipt"),
            promotion_gate: String::from(
                "promote only if aggregate validates and validation loss is finite and not worse than base",
            ),
        },
        aggregation_weight_basis: A1MinimalDistributedLmAggregationWeightBasis {
            basis_id: String::from("accepted_tokens_processed_v1"),
            model_progress_weight_basis: String::from(
                "accepted local-update tokens processed against the same base checkpoint",
            ),
            support_work_weight_basis: String::from(
                "support/verifier work counts for participant truth but has zero aggregation weight",
            ),
            detail: String::from(
                "Only accepted local-update training artifacts can advance the aggregate checkpoint; validation replay, eval, checkpoint verification, and rematerialization are accepted compute work but not model-progress weight.",
            ),
        },
        contribution_receipt_schema: A1MinimalDistributedLmContributionReceiptSchema {
            schema_version: String::from("psion.a1_minimal_distributed_lm.contribution_receipt.v1"),
            required_fields: vec![
                String::from("worker_id"),
                String::from("assignment_id"),
                String::from("run_id"),
                String::from("work_class"),
                String::from("input_shard"),
                String::from("base_checkpoint"),
                String::from("output_artifact"),
                String::from("artifact_digest"),
                String::from("loss_before"),
                String::from("loss_after"),
                String::from("validator_verdict"),
                String::from("closeout_verdict"),
            ],
            model_progress_work_classes: vec![String::from("local_update_training")],
            support_work_classes: vec![
                String::from("tokenized_shard_validation"),
                String::from("validation_replay"),
                String::from("checkpoint_verification"),
                String::from("eval_batch"),
                String::from("artifact_rematerialization"),
                String::from("independent_scored_training_window"),
            ],
        },
        validator_acceptance_policy: A1MinimalDistributedLmValidatorAcceptancePolicy {
            policy_id: String::from("a1_minimal_distributed_lm_validator_acceptance_v1"),
            required_checks: vec![
                String::from("schema_version_matches"),
                String::from("run_id_matches_assignment"),
                String::from("tokenizer_digest_matches_contract"),
                String::from("tokenized_dataset_digest_matches_contract"),
                String::from("base_checkpoint_matches_assignment"),
                String::from("artifact_digest_matches_payload"),
                String::from("loss_values_are_finite"),
                String::from("validator_replay_succeeds"),
            ],
            rejection_checks: vec![
                String::from("stale_assignment"),
                String::from("digest_mismatch"),
                String::from("missing_artifact"),
                String::from("nonfinite_loss"),
                String::from("base_checkpoint_mismatch"),
            ],
        },
        closeout_and_promotion: A1MinimalDistributedLmCloseoutAndPromotionSemantics {
            closeout_authority: String::from("Nexus closeout truth"),
            participant_counter_source: String::from("training_accepted_contributors"),
            model_progress_counter_source: String::from("training_model_progress_contributors"),
            promotion_semantics: String::from(
                "Nexus may count accepted support/verifier work as participants, but only accepted local-update artifacts that enter the promoted aggregate checkpoint count as model-progress participants.",
            ),
        },
        claim_boundary: String::from(
            "This lane is a fixed tiny LM contract for distributed local-update and verifier/support work. It is not OpenWebText leaderboard parity, not broad pretraining, not a model-size claim, and not the existing bounded four-step CS336 A1 demo lane.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract
}

pub fn write_a1_minimal_distributed_lm_lane_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), A1MinimalDistributedLmLaneContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            A1MinimalDistributedLmLaneContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_a1_minimal_distributed_lm_lane_contract();
    contract.validate()?;
    let mut bytes = serde_json::to_vec_pretty(&contract)?;
    bytes.push(b'\n');
    fs::write(output_path, bytes).map_err(|error| A1MinimalDistributedLmLaneContractError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn expected_tokenizer_digest() -> Result<String, A1MinimalDistributedLmLaneContractError> {
    canonical_a1_minimal_distributed_lm_tokenizer_digest().map_err(canonical_bundle_error)
}

fn expected_tokenized_dataset_digest() -> Result<String, A1MinimalDistributedLmLaneContractError> {
    canonical_a1_minimal_distributed_lm_tokenized_dataset_digest().map_err(canonical_bundle_error)
}

fn expected_validation_set_digest() -> Result<String, A1MinimalDistributedLmLaneContractError> {
    canonical_a1_minimal_distributed_lm_validation_set_digest().map_err(canonical_bundle_error)
}

fn expected_tokenizer_vocab_size() -> Result<u32, A1MinimalDistributedLmLaneContractError> {
    canonical_a1_minimal_distributed_lm_tokenizer_vocab_size().map_err(canonical_bundle_error)
}

fn canonical_bundle_error(
    error: impl std::fmt::Display,
) -> A1MinimalDistributedLmLaneContractError {
    A1MinimalDistributedLmLaneContractError::InvalidContract {
        detail: format!("canonical tokenizer/dataset bundle invalid: {error}"),
    }
}

fn sha256_uri_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("A1 minimal distributed LM contract payload should serialize"),
    );
    format!("sha256:{:x}", hasher.finalize())
}

fn ensure_nonempty(
    value: &str,
    field: &str,
) -> Result<(), A1MinimalDistributedLmLaneContractError> {
    if value.trim().is_empty() {
        return invalid_contract(format!("field `{field}` must not be empty"));
    }
    Ok(())
}

fn ensure_vec_nonempty(
    values: &[String],
    field: &str,
) -> Result<(), A1MinimalDistributedLmLaneContractError> {
    if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
        return invalid_contract(format!("field `{field}` must contain nonempty values"));
    }
    Ok(())
}

fn ensure_sha256_uri(
    value: &str,
    field: &str,
) -> Result<(), A1MinimalDistributedLmLaneContractError> {
    ensure_nonempty(value, field)?;
    let Some(hex) = value.strip_prefix("sha256:") else {
        return invalid_contract(format!("field `{field}` must use sha256:<hex> form"));
    };
    if hex.len() != 64 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return invalid_contract(format!(
            "field `{field}` must contain a 64-hex sha256 digest"
        ));
    }
    Ok(())
}

fn invalid_contract<T>(detail: String) -> Result<T, A1MinimalDistributedLmLaneContractError> {
    Err(A1MinimalDistributedLmLaneContractError::InvalidContract { detail })
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_a1_minimal_distributed_lm_lane_contract, A1MinimalDistributedLmLaneContract,
        A1MinimalDistributedLmLaneContractError,
    };
    use crate::PSION_CS336_A1_DEMO_LANE_ID;

    fn fixture_contract() -> A1MinimalDistributedLmLaneContract {
        serde_json::from_str(include_str!(
            "../../../fixtures/training/a1_minimal_distributed_lm_lane_contract_v1.json"
        ))
        .expect("A1 minimal distributed LM lane contract fixture should parse")
    }

    #[test]
    fn a1_minimal_distributed_lm_lane_fixture_validates() {
        fixture_contract()
            .validate()
            .expect("A1 minimal distributed LM lane contract fixture should validate");
    }

    #[test]
    fn a1_minimal_distributed_lm_canonical_contract_matches_fixture() {
        assert_eq!(
            fixture_contract(),
            canonical_a1_minimal_distributed_lm_lane_contract()
        );
    }

    #[test]
    fn a1_minimal_distributed_lm_rejects_demo_lane_alias() {
        let mut contract = fixture_contract();
        contract.lane_id = String::from(PSION_CS336_A1_DEMO_LANE_ID);
        contract.contract_digest = contract.stable_digest();
        let error = contract
            .validate()
            .expect_err("demo lane alias should be rejected");
        assert!(matches!(
            error,
            A1MinimalDistributedLmLaneContractError::InvalidContract { .. }
        ));
    }

    #[test]
    fn a1_minimal_distributed_lm_rejects_digest_drift() {
        let mut contract = fixture_contract();
        contract.contract_digest =
            String::from("sha256:0000000000000000000000000000000000000000000000000000000000000000");
        let error = contract
            .validate()
            .expect_err("contract digest drift should be rejected");
        assert!(matches!(
            error,
            A1MinimalDistributedLmLaneContractError::InvalidContract { .. }
        ));
    }
}
