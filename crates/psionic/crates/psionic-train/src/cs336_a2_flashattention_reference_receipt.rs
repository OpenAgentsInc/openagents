use std::{fs, path::Path};

use psionic_models::{
    Cs336A2FlashAttentionReferenceConfig, Cs336A2FlashAttentionReferenceError,
    cs336_a2_flash_attention_reference_backward, cs336_a2_flash_attention_reference_forward,
    cs336_a2_naive_attention_backward, cs336_a2_naive_attention_forward,
};
use psionic_nn::LayerError;
use psionic_transformer::{AttentionMask, AttentionTensor4, AttentionTensorError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a2_flashattention_reference_receipt_v1.json";
pub const CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_SCHEMA_VERSION: &str =
    "psion.cs336_a2.flashattention_reference_receipt.v1";

#[derive(Debug, Error)]
pub enum Cs336A2FlashAttentionReferenceReceiptError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Model(#[from] Cs336A2FlashAttentionReferenceError),
    #[error(transparent)]
    AttentionTensor(#[from] AttentionTensorError),
    #[error(transparent)]
    Layer(#[from] LayerError),
    #[error("invalid CS336 A2 FlashAttention reference receipt: {0}")]
    InvalidReceipt(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionReferenceInputConfig {
    pub batch_size: usize,
    pub head_count: usize,
    pub sequence_length: usize,
    pub head_dim: usize,
    pub causal: bool,
    pub query_block_rows: usize,
    pub key_block_rows: usize,
    pub max_abs_tolerance: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionReferenceMemoryComparison {
    pub naive_score_elements: usize,
    pub naive_probability_elements: usize,
    pub reference_score_tile_elements: usize,
    pub reference_probability_tile_elements: usize,
    pub reference_saved_lse_elements: usize,
    pub reference_saved_output_elements: usize,
    pub score_tile_reduction_ratio: f32,
    pub probability_tile_reduction_ratio: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FlashAttentionReferenceReceipt {
    pub schema_version: String,
    pub reference_lane_doc_path: String,
    pub baseline_profile_bundle_path: String,
    pub config: Cs336A2FlashAttentionReferenceInputConfig,
    pub forward_output_max_abs_diff: f32,
    pub forward_logsumexp_max_abs_diff: f32,
    pub backward_d_query_max_abs_diff: f32,
    pub backward_d_key_max_abs_diff: f32,
    pub backward_d_value_max_abs_diff: f32,
    pub memory: Cs336A2FlashAttentionReferenceMemoryComparison,
    pub reference_output_digest: String,
    pub reference_logsumexp_digest: String,
    pub reference_d_query_digest: String,
    pub reference_d_key_digest: String,
    pub reference_d_value_digest: String,
    pub claim_boundary: String,
}

pub fn build_cs336_a2_flashattention_reference_receipt()
-> Result<Cs336A2FlashAttentionReferenceReceipt, Cs336A2FlashAttentionReferenceReceiptError> {
    let config = Cs336A2FlashAttentionReferenceInputConfig {
        batch_size: 2,
        head_count: 2,
        sequence_length: 4,
        head_dim: 4,
        causal: true,
        query_block_rows: 3,
        key_block_rows: 2,
        max_abs_tolerance: 1e-4,
    };
    let tensor_shape = [
        config.batch_size,
        config.head_count,
        config.sequence_length,
        config.head_dim,
    ];
    let query = deterministic_attention_tensor(tensor_shape, -0.2, 0.01)?;
    let key = deterministic_attention_tensor(tensor_shape, 0.15, 0.0125)?;
    let value = deterministic_attention_tensor(tensor_shape, -0.35, 0.02)?;
    let grad_output = deterministic_attention_tensor(tensor_shape, 0.05, 0.0075)?;
    let mask = config.causal.then(|| {
        AttentionMask::causal(
            config.batch_size,
            config.sequence_length,
            config.sequence_length,
        )
    });
    let flash_config = Cs336A2FlashAttentionReferenceConfig {
        query_block_rows: config.query_block_rows,
        key_block_rows: config.key_block_rows,
    };

    let naive_forward = cs336_a2_naive_attention_forward(&query, &key, &value, mask.as_ref())?;
    let naive_backward = cs336_a2_naive_attention_backward(
        &query,
        &key,
        &value,
        &naive_forward,
        &grad_output,
        mask.as_ref(),
    )?;
    let reference_forward = cs336_a2_flash_attention_reference_forward(
        &query,
        &key,
        &value,
        mask.as_ref(),
        flash_config,
    )?;
    let reference_backward = cs336_a2_flash_attention_reference_backward(
        &query,
        &key,
        &value,
        &reference_forward,
        &grad_output,
        mask.as_ref(),
        flash_config,
    )?;

    let memory = Cs336A2FlashAttentionReferenceMemoryComparison {
        naive_score_elements: config.batch_size
            * config.head_count
            * config.sequence_length
            * config.sequence_length,
        naive_probability_elements: config.batch_size
            * config.head_count
            * config.sequence_length
            * config.sequence_length,
        reference_score_tile_elements: reference_forward.stats.score_tile_elements,
        reference_probability_tile_elements: reference_forward.stats.probability_tile_elements,
        reference_saved_lse_elements: reference_forward.stats.saved_lse_elements,
        reference_saved_output_elements: reference_forward.stats.saved_output_elements,
        score_tile_reduction_ratio: (config.batch_size
            * config.head_count
            * config.sequence_length
            * config.sequence_length) as f32
            / reference_forward.stats.score_tile_elements as f32,
        probability_tile_reduction_ratio: (config.batch_size
            * config.head_count
            * config.sequence_length
            * config.sequence_length) as f32
            / reference_forward.stats.probability_tile_elements as f32,
    };
    let claim_boundary = String::from(
        "This receipt proves a bounded owned FlashAttention2-style reference path inside psionic. It covers forward parity, backward parity, and tiled-memory posture for the CS336 A2 reference lane only. It does not claim a fused backend kernel or production actual-lane attention closure.",
    );
    let receipt = Cs336A2FlashAttentionReferenceReceipt {
        schema_version: String::from(CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_SCHEMA_VERSION),
        reference_lane_doc_path: String::from("docs/PSION_CS336_A2_REFERENCE_LANE.md"),
        baseline_profile_bundle_path: String::from(
            "fixtures/training/cs336_a2_baseline_profile_bundle_v1.json",
        ),
        config,
        forward_output_max_abs_diff: reference_forward
            .output
            .max_abs_diff(&naive_forward.output)?,
        forward_logsumexp_max_abs_diff: max_abs_diff(
            reference_forward.logsumexp.as_f32_slice()?,
            naive_forward.logsumexp.as_f32_slice()?,
        ),
        backward_d_query_max_abs_diff: reference_backward
            .d_query
            .max_abs_diff(&naive_backward.d_query)?,
        backward_d_key_max_abs_diff: reference_backward
            .d_key
            .max_abs_diff(&naive_backward.d_key)?,
        backward_d_value_max_abs_diff: reference_backward
            .d_value
            .max_abs_diff(&naive_backward.d_value)?,
        memory,
        reference_output_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention_reference.output",
            &reference_forward.output,
        ),
        reference_logsumexp_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention_reference.logsumexp",
            &reference_forward.logsumexp,
        ),
        reference_d_query_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention_reference.d_query",
            &reference_backward.d_query,
        ),
        reference_d_key_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention_reference.d_key",
            &reference_backward.d_key,
        ),
        reference_d_value_digest: stable_json_digest(
            b"psion.cs336_a2.flashattention_reference.d_value",
            &reference_backward.d_value,
        ),
        claim_boundary,
    };
    validate_receipt(&receipt)?;
    Ok(receipt)
}

pub fn write_cs336_a2_flashattention_reference_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2FlashAttentionReferenceReceipt, Cs336A2FlashAttentionReferenceReceiptError> {
    let receipt = build_cs336_a2_flashattention_reference_receipt()?;
    let receipt_path = repo_root
        .as_ref()
        .join(CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH);
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt)
}

fn validate_receipt(
    receipt: &Cs336A2FlashAttentionReferenceReceipt,
) -> Result<(), Cs336A2FlashAttentionReferenceReceiptError> {
    if receipt.schema_version != CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_SCHEMA_VERSION {
        return Err(Cs336A2FlashAttentionReferenceReceiptError::InvalidReceipt(
            format!(
                "expected schema version `{CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_SCHEMA_VERSION}`, got `{}`",
                receipt.schema_version
            ),
        ));
    }
    let tolerance = receipt.config.max_abs_tolerance;
    for (label, diff) in [
        ("forward_output", receipt.forward_output_max_abs_diff),
        ("forward_logsumexp", receipt.forward_logsumexp_max_abs_diff),
        ("backward_d_query", receipt.backward_d_query_max_abs_diff),
        ("backward_d_key", receipt.backward_d_key_max_abs_diff),
        ("backward_d_value", receipt.backward_d_value_max_abs_diff),
    ] {
        if diff > tolerance {
            return Err(Cs336A2FlashAttentionReferenceReceiptError::InvalidReceipt(
                format!("{label} diff {diff} exceeds tolerance {tolerance}"),
            ));
        }
    }
    if receipt.memory.reference_score_tile_elements >= receipt.memory.naive_score_elements {
        return Err(Cs336A2FlashAttentionReferenceReceiptError::InvalidReceipt(
            "reference score tile should be smaller than naive full score surface".into(),
        ));
    }
    if receipt.memory.reference_probability_tile_elements
        >= receipt.memory.naive_probability_elements
    {
        return Err(Cs336A2FlashAttentionReferenceReceiptError::InvalidReceipt(
            "reference probability tile should be smaller than naive full probability surface"
                .into(),
        ));
    }
    Ok(())
}

fn deterministic_attention_tensor(
    shape: [usize; 4],
    start: f32,
    step: f32,
) -> Result<AttentionTensor4, Cs336A2FlashAttentionReferenceReceiptError> {
    let mut values = Vec::with_capacity(shape.iter().product());
    for index in 0..shape.iter().product::<usize>() {
        values.push(start + index as f32 * step);
    }
    Ok(AttentionTensor4::new(shape, values)?)
}

fn max_abs_diff(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(left, right)| (left - right).abs())
        .fold(0.0f32, f32::max)
}

fn stable_json_digest<T: Serialize>(domain: &[u8], value: &T) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(serde_json::to_vec(value).expect("serializing stable digest input must succeed"));
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH,
        build_cs336_a2_flashattention_reference_receipt,
        write_cs336_a2_flashattention_reference_receipt,
    };

    #[test]
    fn flashattention_reference_receipt_has_expected_tolerance()
    -> Result<(), Box<dyn std::error::Error>> {
        let receipt = build_cs336_a2_flashattention_reference_receipt()?;
        assert!(receipt.forward_output_max_abs_diff <= receipt.config.max_abs_tolerance);
        assert!(receipt.backward_d_query_max_abs_diff <= receipt.config.max_abs_tolerance);
        assert!(receipt.memory.score_tile_reduction_ratio > 1.0);
        Ok(())
    }

    #[test]
    fn flashattention_reference_writer_emits_json_fixture() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempdir()?;
        let receipt = write_cs336_a2_flashattention_reference_receipt(temp.path())?;
        let fixture_path = temp
            .path()
            .join(CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH);
        assert!(fixture_path.exists());
        let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&fixture_path)?)?;
        assert_eq!(
            written["schema_version"].as_str(),
            Some(receipt.schema_version.as_str())
        );
        Ok(())
    }
}
