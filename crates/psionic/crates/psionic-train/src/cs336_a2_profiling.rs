use std::{fs, path::Path};

use psionic_core::{DType, Shape};
use psionic_models::{
    cs336_a1_scaled_dot_product_attention, Cs336A1TransformerLm,
};
use psionic_nn::{ModuleStateDict, NnTensor};
use psionic_transformer::AttentionMask;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    Cs336A1ReferenceTrainingConfig, Cs336A1ReferenceTrainingError, Cs336A1ReferenceTrainer,
    CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH,
};

pub const CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a2_baseline_profile_bundle_v1.json";
pub const CS336_A2_REFERENCE_LANE_DOC_PATH: &str = "docs/PSION_CS336_A2_REFERENCE_LANE.md";
pub const CS336_A2_BASELINE_PROFILE_BUNDLE_SCHEMA_VERSION: &str =
    "psion.cs336_a2.baseline_profile_bundle.v1";

#[derive(Debug, Error)]
pub enum Cs336A2ProfilingError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Layer(#[from] psionic_nn::LayerError),
    #[error(transparent)]
    Model(#[from] psionic_models::Cs336A1ReferenceError),
    #[error(transparent)]
    Training(#[from] Cs336A1ReferenceTrainingError),
    #[error("invalid CS336 A2 profiling bundle: {0}")]
    InvalidBundle(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2ProfilingConfig {
    pub training: Cs336A1ReferenceTrainingConfig,
    pub attention_batch_size: usize,
    pub attention_head_count: usize,
    pub attention_sequence_length: usize,
    pub attention_head_dim: usize,
    pub distributed_world_size: usize,
    pub distributed_bucket_size_bytes: u64,
}

impl Cs336A2ProfilingConfig {
    #[must_use]
    pub fn tiny() -> Self {
        Self {
            training: Cs336A1ReferenceTrainingConfig::tiny(),
            attention_batch_size: 2,
            attention_head_count: 2,
            attention_sequence_length: 4,
            attention_head_dim: 4,
            distributed_world_size: 2,
            distributed_bucket_size_bytes: 128,
        }
    }

    fn attention_element_count(&self) -> usize {
        self.attention_batch_size
            * self.attention_head_count
            * self.attention_sequence_length
            * self.attention_head_dim
    }
}

impl Default for Cs336A2ProfilingConfig {
    fn default() -> Self {
        Self::tiny()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cs336A2ProfileKind {
    AnalyticalReference,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2AttentionBaselineReceipt {
    pub route_id: String,
    pub profile_kind: Cs336A2ProfileKind,
    pub batch_size: usize,
    pub head_count: usize,
    pub sequence_length: usize,
    pub head_dim: usize,
    pub causal: bool,
    pub query_elements: usize,
    pub key_elements: usize,
    pub value_elements: usize,
    pub logits_elements: usize,
    pub probability_elements: usize,
    pub output_elements: usize,
    pub input_bytes: u64,
    pub peak_resident_bytes: u64,
    pub logical_flops: u64,
    pub normalized_time_units: u64,
    pub output_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2TrainingStepBaselineReceipt {
    pub route_id: String,
    pub profile_kind: Cs336A2ProfileKind,
    pub batch_tokens: usize,
    pub parameter_tensor_count: usize,
    pub parameter_element_count: usize,
    pub parameter_bytes: u64,
    pub expected_adamw_state_bytes_after_first_step: u64,
    pub loss_before: f32,
    pub loss_after: f32,
    pub gradient_norm_l2_before: f32,
    pub gradient_norm_l2_after: f32,
    pub learning_rate: f32,
    pub model_state_digest_before: String,
    pub model_state_digest_after: String,
    pub optimizer_state_digest_after: String,
    pub logical_flops: u64,
    pub normalized_time_units: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2DistributedBaselineReceipt {
    pub route_id: String,
    pub profile_kind: Cs336A2ProfileKind,
    pub world_size: usize,
    pub bucket_size_bytes: u64,
    pub synchronized_parameter_count: usize,
    pub synchronized_parameter_bytes: u64,
    pub expected_bucket_count: usize,
    pub allreduce_payload_bytes_per_step: u64,
    pub replicated_optimizer_state_bytes: u64,
    pub sharded_optimizer_state_bytes_per_rank: u64,
    pub local_tokens_per_rank: usize,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2BaselineProfileBundle {
    pub schema_version: String,
    pub reference_lane_doc_path: String,
    pub actual_lane_systems_bundle_doc_path: String,
    pub corpus_fixture_path: String,
    pub config: Cs336A2ProfilingConfig,
    pub attention_baseline: Cs336A2AttentionBaselineReceipt,
    pub training_step_baseline: Cs336A2TrainingStepBaselineReceipt,
    pub distributed_step_baseline: Cs336A2DistributedBaselineReceipt,
    pub bundle_digest: String,
    pub claim_boundary: String,
}

pub fn build_cs336_a2_baseline_profile_bundle(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2BaselineProfileBundle, Cs336A2ProfilingError> {
    let repo_root = repo_root.as_ref();
    let config = Cs336A2ProfilingConfig::tiny();
    let attention_baseline = build_attention_baseline(&config)?;
    let training_step_baseline = build_training_step_baseline(repo_root, &config)?;
    let distributed_step_baseline =
        build_distributed_step_baseline(&config, &training_step_baseline);
    let claim_boundary = String::from(
        "This bundle is the first bounded CS336 A2 profiling tranche inside psionic. It records deterministic baseline receipts for naive attention, the tiny A1-backed training step, and the pre-DDP distributed communication baseline. It does not claim admitted actual-lane throughput or cluster qualification.",
    );
    let bundle_digest = stable_json_digest(
        b"psion.cs336_a2.baseline_profile_bundle",
        &(
            &config,
            &attention_baseline,
            &training_step_baseline,
            &distributed_step_baseline,
            claim_boundary.as_str(),
        ),
    );
    let bundle = Cs336A2BaselineProfileBundle {
        schema_version: String::from(CS336_A2_BASELINE_PROFILE_BUNDLE_SCHEMA_VERSION),
        reference_lane_doc_path: String::from(CS336_A2_REFERENCE_LANE_DOC_PATH),
        actual_lane_systems_bundle_doc_path: String::from(
            "docs/PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE.md",
        ),
        corpus_fixture_path: String::from(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH),
        config,
        attention_baseline,
        training_step_baseline,
        distributed_step_baseline,
        bundle_digest,
        claim_boundary,
    };
    validate_bundle(&bundle)?;
    Ok(bundle)
}

pub fn write_cs336_a2_baseline_profile_bundle(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2BaselineProfileBundle, Cs336A2ProfilingError> {
    let bundle = build_cs336_a2_baseline_profile_bundle(&repo_root)?;
    let bundle_path = repo_root
        .as_ref()
        .join(CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH);
    if let Some(parent) = bundle_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(bundle_path, serde_json::to_vec_pretty(&bundle)?)?;
    Ok(bundle)
}

fn validate_bundle(bundle: &Cs336A2BaselineProfileBundle) -> Result<(), Cs336A2ProfilingError> {
    if bundle.schema_version != CS336_A2_BASELINE_PROFILE_BUNDLE_SCHEMA_VERSION {
        return Err(Cs336A2ProfilingError::InvalidBundle(format!(
            "expected schema version `{CS336_A2_BASELINE_PROFILE_BUNDLE_SCHEMA_VERSION}`, got `{}`",
            bundle.schema_version
        )));
    }
    if bundle.attention_baseline.logical_flops == 0 {
        return Err(Cs336A2ProfilingError::InvalidBundle(
            "attention baseline must have non-zero logical_flops".into(),
        ));
    }
    if bundle.training_step_baseline.parameter_element_count == 0 {
        return Err(Cs336A2ProfilingError::InvalidBundle(
            "training step baseline must describe non-zero parameters".into(),
        ));
    }
    if bundle.distributed_step_baseline.expected_bucket_count == 0 {
        return Err(Cs336A2ProfilingError::InvalidBundle(
            "distributed baseline must describe at least one bucket".into(),
        ));
    }
    Ok(())
}

fn build_attention_baseline(
    config: &Cs336A2ProfilingConfig,
) -> Result<Cs336A2AttentionBaselineReceipt, Cs336A2ProfilingError> {
    let shape = Shape::new(vec![
        config.attention_batch_size,
        config.attention_head_count,
        config.attention_sequence_length,
        config.attention_head_dim,
    ]);
    let query = deterministic_attention_tensor(config, 0.1, &shape)?;
    let key = deterministic_attention_tensor(config, 0.2, &shape)?;
    let value = deterministic_attention_tensor(config, 0.3, &shape)?;
    let mask = AttentionMask::causal(
        config.attention_batch_size,
        config.attention_sequence_length,
        config.attention_sequence_length,
    );
    let output = cs336_a1_scaled_dot_product_attention(&query, &key, &value, Some(&mask))?;
    let logits_elements = config.attention_batch_size
        * config.attention_head_count
        * config.attention_sequence_length
        * config.attention_sequence_length;
    let output_elements = config.attention_element_count();
    let input_elements = config.attention_element_count();
    let bytes_per_f32 = DType::F32.element_size_bytes() as u64;
    let logical_flops = attention_logical_flops(config) as u64;
    Ok(Cs336A2AttentionBaselineReceipt {
        route_id: String::from("naive_attention_baseline"),
        profile_kind: Cs336A2ProfileKind::AnalyticalReference,
        batch_size: config.attention_batch_size,
        head_count: config.attention_head_count,
        sequence_length: config.attention_sequence_length,
        head_dim: config.attention_head_dim,
        causal: true,
        query_elements: input_elements,
        key_elements: input_elements,
        value_elements: input_elements,
        logits_elements,
        probability_elements: logits_elements,
        output_elements,
        input_bytes: (input_elements as u64 * 3) * bytes_per_f32,
        peak_resident_bytes: ((input_elements as u64 * 3)
            + (logits_elements as u64 * 2)
            + output_elements as u64)
            * bytes_per_f32,
        logical_flops,
        normalized_time_units: logical_flops / 16,
        output_digest: stable_json_digest(
            b"psion.cs336_a2.attention_baseline_output",
            &output.as_f32_slice()?.to_vec(),
        ),
    })
}

fn build_training_step_baseline(
    repo_root: &Path,
    config: &Cs336A2ProfilingConfig,
) -> Result<Cs336A2TrainingStepBaselineReceipt, Cs336A2ProfilingError> {
    let corpus_path = repo_root.join(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH);
    let mut trainer = Cs336A1ReferenceTrainer::from_corpus_path(&corpus_path, config.training.clone())?;
    let loss_before = trainer.current_loss()?;
    let step = trainer.step()?;
    let model_state = Cs336A1TransformerLm::new(
        "cs336_a2_profile_lm",
        config.training.model_config(),
        config.training.rope_theta,
        config.training.rms_norm_eps,
    )?
    .state_dict();
    let parameter_summary = module_state_summary(&model_state);
    let batch_tokens = config.training.batch_size * config.training.context_length;
    let logical_flops = (parameter_summary.element_count as u64)
        * (batch_tokens as u64)
        * 2;
    Ok(Cs336A2TrainingStepBaselineReceipt {
        route_id: String::from("tiny_training_step_baseline"),
        profile_kind: Cs336A2ProfileKind::AnalyticalReference,
        batch_tokens,
        parameter_tensor_count: parameter_summary.tensor_count,
        parameter_element_count: parameter_summary.element_count,
        parameter_bytes: parameter_summary.total_bytes,
        expected_adamw_state_bytes_after_first_step: parameter_summary.total_bytes * 2,
        loss_before,
        loss_after: step.loss_after,
        gradient_norm_l2_before: step.gradient_clip.gradient_norm_l2_before,
        gradient_norm_l2_after: step.gradient_clip.gradient_norm_l2_after,
        learning_rate: step.learning_rate,
        model_state_digest_before: step.model_state_digest_before,
        model_state_digest_after: step.model_state_digest_after,
        optimizer_state_digest_after: step.optimizer_state_digest_after,
        logical_flops,
        normalized_time_units: logical_flops / 32,
    })
}

fn build_distributed_step_baseline(
    config: &Cs336A2ProfilingConfig,
    training_step: &Cs336A2TrainingStepBaselineReceipt,
) -> Cs336A2DistributedBaselineReceipt {
    let world_size = config.distributed_world_size.max(1);
    let local_tokens_per_rank =
        (training_step.batch_tokens + world_size.saturating_sub(1)) / world_size;
    let synchronized_parameter_bytes = training_step.parameter_bytes;
    let expected_bucket_count = usize::max(
        1,
        synchronized_parameter_bytes
            .div_ceil(config.distributed_bucket_size_bytes) as usize,
    );
    let allreduce_payload_bytes_per_step =
        synchronized_parameter_bytes * (world_size.saturating_sub(1) as u64);
    Cs336A2DistributedBaselineReceipt {
        route_id: String::from("distributed_step_baseline"),
        profile_kind: Cs336A2ProfileKind::AnalyticalReference,
        world_size,
        bucket_size_bytes: config.distributed_bucket_size_bytes,
        synchronized_parameter_count: training_step.parameter_tensor_count,
        synchronized_parameter_bytes,
        expected_bucket_count,
        allreduce_payload_bytes_per_step,
        replicated_optimizer_state_bytes: training_step.expected_adamw_state_bytes_after_first_step,
        sharded_optimizer_state_bytes_per_rank: training_step
            .expected_adamw_state_bytes_after_first_step
            .div_ceil(world_size as u64),
        local_tokens_per_rank,
        claim_boundary: String::from(
            "This baseline is the pre-DDP analytical comparison surface. It records the tiny reference model's full-state communication cost before the bounded individual-parameter, bucketed, and sharded optimizer paths land.",
        ),
    }
}

fn deterministic_attention_tensor(
    config: &Cs336A2ProfilingConfig,
    scale: f32,
    shape: &Shape,
) -> Result<NnTensor, psionic_nn::LayerError> {
    let values = (0..config.attention_element_count())
        .map(|index| scale + index as f32 / 100.0)
        .collect::<Vec<_>>();
    NnTensor::f32(shape.clone(), values)
}

fn attention_logical_flops(config: &Cs336A2ProfilingConfig) -> usize {
    let batches = config.attention_batch_size * config.attention_head_count;
    let q = config.attention_sequence_length;
    let k = config.attention_sequence_length;
    let d = config.attention_head_dim;
    let qk = 2 * batches * q * k * d;
    let pv = 2 * batches * q * k * d;
    let softmax = 3 * batches * q * k;
    qk + pv + softmax
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ModuleStateSummary {
    tensor_count: usize,
    element_count: usize,
    total_bytes: u64,
}

fn module_state_summary(state_dict: &ModuleStateDict) -> ModuleStateSummary {
    let mut tensor_count = 0usize;
    let mut element_count = 0usize;
    let mut total_bytes = 0u64;
    for entry in state_dict.entries.values() {
        if entry.kind != psionic_nn::ModuleStateEntryKind::Parameter {
            continue;
        }
        tensor_count += 1;
        let elements = entry.spec.shape().element_count();
        element_count += elements;
        total_bytes += (elements * entry.spec.dtype().element_size_bytes()) as u64;
    }
    ModuleStateSummary {
        tensor_count,
        element_count,
        total_bytes,
    }
}

fn stable_json_digest<T: Serialize>(domain: &[u8], value: &T) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(
        serde_json::to_vec(value).expect("serializing stable digest input must succeed"),
    );
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tempfile::tempdir;

    use super::{
        build_cs336_a2_baseline_profile_bundle, write_cs336_a2_baseline_profile_bundle,
        CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH,
    };

    #[test]
    fn baseline_profile_bundle_has_expected_routes() -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let bundle = build_cs336_a2_baseline_profile_bundle(repo_root)?;
        assert_eq!(bundle.attention_baseline.route_id, "naive_attention_baseline");
        assert_eq!(
            bundle.training_step_baseline.route_id,
            "tiny_training_step_baseline"
        );
        assert_eq!(
            bundle.distributed_step_baseline.route_id,
            "distributed_step_baseline"
        );
        assert!(
            bundle.training_step_baseline.gradient_norm_l2_after
                <= bundle.training_step_baseline.gradient_norm_l2_before
        );
        assert_ne!(
            bundle.training_step_baseline.model_state_digest_before,
            bundle.training_step_baseline.model_state_digest_after
        );
        assert!(bundle.distributed_step_baseline.expected_bucket_count >= 1);
        Ok(())
    }

    #[test]
    fn baseline_profile_bundle_writer_emits_json_fixture() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempdir()?;
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let seeded_corpus_path = temp
            .path()
            .join(crate::CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH);
        let source_corpus_path = repo_root.join(crate::CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH);
        if let Some(parent) = seeded_corpus_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(source_corpus_path, &seeded_corpus_path)?;
        let bundle = write_cs336_a2_baseline_profile_bundle(temp.path())?;
        let fixture_path = temp
            .path()
            .join(CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH);
        assert!(fixture_path.exists());
        let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&fixture_path)?)?;
        assert_eq!(
            written["schema_version"].as_str(),
            Some(bundle.schema_version.as_str())
        );
        Ok(())
    }
}
