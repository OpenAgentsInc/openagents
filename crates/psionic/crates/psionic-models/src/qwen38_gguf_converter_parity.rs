use std::{collections::BTreeMap, fs, path::Path};

use psionic_catalog::{BlobReadPreference, LocalBlobOpenOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    GgufBlobArtifact, GgufTensorRowReadReceipt, ModelLoadError, QWEN38_27B_UPSTREAM_REVISION,
    QWEN38_GGUF_AUDITED_LLAMA_CPP_REVISION, QWEN38_GGUF_REPOSITORY_ID,
    QWEN38_GGUF_REPOSITORY_REVISION, Qwen35TextCheckpointError, Qwen35TextTensorRowReadReceipt,
    Qwen38GgufProfile, qwen35_text_read_indexed_tensor_row, qwen35_text_weight_index_from_bytes,
};

pub const QWEN38_GGUF_CONVERTER_PARITY_SCHEMA_VERSION: &str =
    "psionic.qwen38_27b_gguf_converter_parity.v1";

const NUM_KEY_HEADS: usize = 16;
const NUM_VALUE_HEADS: usize = 48;
const VALUE_HEAD_DIM: usize = 128;
const VALUE_HEADS_PER_KEY: usize = NUM_VALUE_HEADS / NUM_KEY_HEADS;
const QK_ROWS: usize = 4096;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufNumericMetrics {
    pub element_count: usize,
    pub root_mean_square_error: f64,
    pub mean_absolute_error: f64,
    pub maximum_absolute_error: f64,
    pub cosine_similarity: f64,
    pub reference_projection_output: f64,
    pub gguf_projection_output: f64,
    pub projection_output_absolute_error: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufTransformCheck {
    pub transform: String,
    pub official_tensor: String,
    pub gguf_tensor: String,
    pub target_row: usize,
    pub official_source_row: usize,
    pub official_receipt: Qwen35TextTensorRowReadReceipt,
    pub gguf_receipt: GgufTensorRowReadReceipt,
    pub metrics: Qwen38GgufNumericMetrics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untransformed_root_mean_square_error: Option<f64>,
    pub parity_passed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufProfileQuality {
    pub profile: Qwen38GgufProfile,
    pub artifact_filename: String,
    pub artifact_sha256: String,
    pub sampled_rows: Vec<Qwen38GgufTransformCheck>,
    pub sampled_row_count: usize,
    pub mean_root_mean_square_error: f64,
    pub mean_cosine_similarity: f64,
    pub mean_projection_output_absolute_error: f64,
    pub transform_layout_passed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufQualityComparison {
    pub measurement_scope: String,
    pub primary_mean_root_mean_square_error: f64,
    pub q3_k_m_mean_root_mean_square_error: f64,
    pub q4_k_m_mean_root_mean_square_error: f64,
    pub primary_to_q3_k_m_rmse_ratio: f64,
    pub primary_mean_cosine_similarity: f64,
    pub q3_k_m_mean_cosine_similarity: f64,
    pub q4_k_m_mean_cosine_similarity: f64,
    pub result: String,
    pub generated_text_quality_status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufConverterParityReport {
    pub schema_version: String,
    pub official_model_revision: String,
    pub gguf_repository_id: String,
    pub gguf_repository_revision: String,
    pub converter_repository: String,
    pub converter_revision: String,
    pub converter_source_path: String,
    pub converter_contract: Vec<String>,
    pub profiles: Vec<Qwen38GgufProfileQuality>,
    pub quality_comparison: Qwen38GgufQualityComparison,
    pub all_transform_layout_checks_passed: bool,
    pub claim_boundary: String,
    pub report_sha256: String,
}

#[derive(Debug, Error)]
pub enum Qwen38GgufConverterParityError {
    #[error("Qwen3.8 converter parity failed: {0}")]
    Parity(String),
    #[error(transparent)]
    ModelLoad(#[from] ModelLoadError),
    #[error(transparent)]
    Checkpoint(#[from] Qwen35TextCheckpointError),
    #[error("Qwen3.8 converter parity I/O failed at `{path}`: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize Qwen3.8 converter parity: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub fn run_qwen38_gguf_converter_parity(
    official_model_dir: impl AsRef<Path>,
    gguf_dir: impl AsRef<Path>,
) -> Result<Qwen38GgufConverterParityReport, Qwen38GgufConverterParityError> {
    let official_model_dir = official_model_dir.as_ref();
    let gguf_dir = gguf_dir.as_ref();
    let index_path = official_model_dir.join("model.safetensors.index.json");
    let index_bytes =
        fs::read(&index_path).map_err(|source| Qwen38GgufConverterParityError::Io {
            path: index_path,
            source,
        })?;
    let index = qwen35_text_weight_index_from_bytes(&index_bytes)?;

    let mut profiles = Vec::new();
    for profile in [
        Qwen38GgufProfile::DynamicV3UdQ3KXl,
        Qwen38GgufProfile::Q3KM,
        Qwen38GgufProfile::Q4KM,
    ] {
        let artifact = GgufBlobArtifact::open_path(
            gguf_dir.join(profile.filename()),
            LocalBlobOpenOptions::default()
                .with_read_preference(BlobReadPreference::RequireMemoryMap),
        )?;
        profiles.push(profile_quality(
            official_model_dir,
            &index.weight_map,
            &artifact,
            profile,
        )?);
    }
    let primary = profile_result(&profiles, Qwen38GgufProfile::DynamicV3UdQ3KXl)?;
    let q3_k_m = profile_result(&profiles, Qwen38GgufProfile::Q3KM)?;
    let q4_k_m = profile_result(&profiles, Qwen38GgufProfile::Q4KM)?;
    let ratio = primary.mean_root_mean_square_error / q3_k_m.mean_root_mean_square_error;
    let result = if ratio <= 1.05
        && primary.mean_cosine_similarity + 0.0001 >= q3_k_m.mean_cosine_similarity
    {
        "primary_sampled_projection_quality_not_worse_than_q3_k_m"
    } else {
        "primary_sampled_projection_quality_below_q3_k_m"
    };
    let quality_comparison = Qwen38GgufQualityComparison {
        measurement_scope: String::from(
            "deterministic sampled transformed-weight rows and their scalar projection outputs",
        ),
        primary_mean_root_mean_square_error: primary.mean_root_mean_square_error,
        q3_k_m_mean_root_mean_square_error: q3_k_m.mean_root_mean_square_error,
        q4_k_m_mean_root_mean_square_error: q4_k_m.mean_root_mean_square_error,
        primary_to_q3_k_m_rmse_ratio: ratio,
        primary_mean_cosine_similarity: primary.mean_cosine_similarity,
        q3_k_m_mean_cosine_similarity: q3_k_m.mean_cosine_similarity,
        q4_k_m_mean_cosine_similarity: q4_k_m.mean_cosine_similarity,
        result: String::from(result),
        generated_text_quality_status: String::from(
            "not_measured_until_real_generation_exists_in_r6;_dynamic_v3_remains_candidate_only",
        ),
    };
    let all_transform_layout_checks_passed = profiles
        .iter()
        .all(|profile| profile.transform_layout_passed);
    if !all_transform_layout_checks_passed {
        let failures = profiles
            .iter()
            .flat_map(|profile| {
                profile
                    .sampled_rows
                    .iter()
                    .filter(|check| !check.parity_passed)
                    .map(|check| {
                        format!(
                            "{:?}:{}:{}->{} rmse={} raw_rmse={:?} max_error={}",
                            profile.profile,
                            check.transform,
                            check.official_source_row,
                            check.target_row,
                            check.metrics.root_mean_square_error,
                            check.untransformed_root_mean_square_error,
                            check.metrics.maximum_absolute_error,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        return parity_error(format!(
            "one or more GGUF profiles failed sampled converter-layout parity: {}",
            failures.join("; ")
        ));
    }

    let mut report = Qwen38GgufConverterParityReport {
        schema_version: String::from(QWEN38_GGUF_CONVERTER_PARITY_SCHEMA_VERSION),
        official_model_revision: String::from(QWEN38_27B_UPSTREAM_REVISION),
        gguf_repository_id: String::from(QWEN38_GGUF_REPOSITORY_ID),
        gguf_repository_revision: String::from(QWEN38_GGUF_REPOSITORY_REVISION),
        converter_repository: String::from("ggml-org/llama.cpp"),
        converter_revision: String::from(QWEN38_GGUF_AUDITED_LLAMA_CPP_REVISION),
        converter_source_path: String::from("conversion/qwen.py"),
        converter_contract: vec![
            String::from("A_log -> -exp(A_log) -> tiled V-head order"),
            String::from("dt_bias -> ssm_dt.bias -> tiled V-head order"),
            String::from("non-linear-attention norm weights -> weight + 1"),
            String::from("linear-attention norm weight remains unshifted"),
            String::from(
                "QKV V rows, Z rows, alpha rows, beta rows, and convolution V channels -> tiled V-head order",
            ),
            String::from("linear-attention output projection columns -> tiled V-head order"),
            String::from(
                "dense transform checks require exact bounded error; quantized tiled-vs-untransformed row RMSE allows a 5 percent quantization-noise margin",
            ),
        ],
        profiles,
        quality_comparison,
        all_transform_layout_checks_passed,
        claim_boundary: String::from(
            "This proves sampled converter-layout and projection-output parity. It does not prove generated-token parity or make Dynamic V3 canonical.",
        ),
        report_sha256: String::new(),
    };
    report.report_sha256 = report_digest(&report)?;
    Ok(report)
}

fn profile_quality(
    official_model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    artifact: &GgufBlobArtifact,
    profile: Qwen38GgufProfile,
) -> Result<Qwen38GgufProfileQuality, Qwen38GgufConverterParityError> {
    let mut sampled_rows = Vec::new();

    sampled_rows.push(vector_check(
        official_model_dir,
        weight_map,
        artifact,
        "negative_exponential_A_log_and_tiled_v_heads",
        "model.language_model.layers.0.linear_attn.A_log",
        "blk.0.ssm_a",
        |values| {
            reorder_vector(values)
                .into_iter()
                .map(|value| -value.exp())
                .collect()
        },
        2.0e-6,
    )?);
    sampled_rows.push(vector_check(
        official_model_dir,
        weight_map,
        artifact,
        "time_step_bias_rename_and_tiled_v_heads",
        "model.language_model.layers.0.linear_attn.dt_bias",
        "blk.0.ssm_dt.bias",
        reorder_vector,
        1.0e-7,
    )?);
    sampled_rows.push(vector_check(
        official_model_dir,
        weight_map,
        artifact,
        "non_linear_attention_norm_plus_one",
        "model.language_model.layers.0.input_layernorm.weight",
        "blk.0.attn_norm.weight",
        |values| values.into_iter().map(|value| value + 1.0).collect(),
        1.0e-7,
    )?);
    sampled_rows.push(vector_check(
        official_model_dir,
        weight_map,
        artifact,
        "linear_attention_norm_unshifted",
        "model.language_model.layers.0.linear_attn.norm.weight",
        "blk.0.ssm_norm.weight",
        |values| values,
        1.0e-7,
    )?);

    for target_row in [128, 2176, 4095] {
        sampled_rows.push(mapped_row_check(
            official_model_dir,
            weight_map,
            artifact,
            "qkv_v_rows_tiled",
            "model.language_model.layers.0.linear_attn.in_proj_qkv.weight",
            "blk.0.attn_qkv.weight",
            QK_ROWS + target_row,
            QK_ROWS + source_index_for_tiled(target_row, VALUE_HEAD_DIM),
            Some(QK_ROWS + target_row),
            f64::INFINITY,
        )?);
        sampled_rows.push(mapped_row_check(
            official_model_dir,
            weight_map,
            artifact,
            "z_gate_rows_tiled",
            "model.language_model.layers.0.linear_attn.in_proj_z.weight",
            "blk.0.attn_gate.weight",
            target_row,
            source_index_for_tiled(target_row, VALUE_HEAD_DIM),
            Some(target_row),
            f64::INFINITY,
        )?);
        sampled_rows.push(mapped_row_check(
            official_model_dir,
            weight_map,
            artifact,
            "convolution_v_channels_tiled_after_squeeze",
            "model.language_model.layers.0.linear_attn.conv1d.weight",
            "blk.0.ssm_conv1d.weight",
            QK_ROWS + target_row,
            QK_ROWS + source_index_for_tiled(target_row, VALUE_HEAD_DIM),
            Some(QK_ROWS + target_row),
            1.0e-7,
        )?);
    }
    for target_row in [1, 16, 31] {
        sampled_rows.push(mapped_row_check(
            official_model_dir,
            weight_map,
            artifact,
            "alpha_rows_tiled",
            "model.language_model.layers.0.linear_attn.in_proj_a.weight",
            "blk.0.ssm_alpha.weight",
            target_row,
            source_index_for_tiled(target_row, 1),
            Some(target_row),
            f64::INFINITY,
        )?);
        sampled_rows.push(mapped_row_check(
            official_model_dir,
            weight_map,
            artifact,
            "beta_rows_tiled",
            "model.language_model.layers.0.linear_attn.in_proj_b.weight",
            "blk.0.ssm_beta.weight",
            target_row,
            source_index_for_tiled(target_row, 1),
            Some(target_row),
            f64::INFINITY,
        )?);
    }
    for target_row in [0, 2559, 5119] {
        sampled_rows.push(output_projection_check(
            official_model_dir,
            weight_map,
            artifact,
            target_row,
        )?);
    }

    for (official_tensor, gguf_tensor, rows) in [
        (
            "model.language_model.embed_tokens.weight",
            "token_embd.weight",
            &[0, 1000, 248_319][..],
        ),
        (
            "model.language_model.layers.0.mlp.gate_proj.weight",
            "blk.0.ffn_gate.weight",
            &[0, 8704, 17_407][..],
        ),
        (
            "model.language_model.layers.3.self_attn.v_proj.weight",
            "blk.3.attn_v.weight",
            &[0, 511, 1023][..],
        ),
        ("lm_head.weight", "output.weight", &[0, 1000, 248_319][..]),
    ] {
        for row in rows {
            sampled_rows.push(mapped_row_check(
                official_model_dir,
                weight_map,
                artifact,
                "unmodified_weight_row_projection_quality",
                official_tensor,
                gguf_tensor,
                *row,
                *row,
                None,
                f64::INFINITY,
            )?);
        }
    }

    let transform_layout_passed = sampled_rows.iter().all(|check| check.parity_passed);
    let sampled_row_count = sampled_rows.len();
    let divisor = sampled_row_count as f64;
    let mean_root_mean_square_error = sampled_rows
        .iter()
        .map(|check| check.metrics.root_mean_square_error)
        .sum::<f64>()
        / divisor;
    let mean_cosine_similarity = sampled_rows
        .iter()
        .map(|check| check.metrics.cosine_similarity)
        .sum::<f64>()
        / divisor;
    let mean_projection_output_absolute_error = sampled_rows
        .iter()
        .map(|check| check.metrics.projection_output_absolute_error)
        .sum::<f64>()
        / divisor;
    Ok(Qwen38GgufProfileQuality {
        profile,
        artifact_filename: String::from(profile.filename()),
        artifact_sha256: String::from(profile.sha256()),
        sampled_rows,
        sampled_row_count,
        mean_root_mean_square_error,
        mean_cosine_similarity,
        mean_projection_output_absolute_error,
        transform_layout_passed,
    })
}

fn vector_check(
    official_model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    artifact: &GgufBlobArtifact,
    transform: &str,
    official_tensor: &str,
    gguf_tensor: &str,
    transform_values: impl FnOnce(Vec<f32>) -> Vec<f32>,
    maximum_error: f64,
) -> Result<Qwen38GgufTransformCheck, Qwen38GgufConverterParityError> {
    let official =
        qwen35_text_read_indexed_tensor_row(official_model_dir, weight_map, official_tensor, 0)?;
    let gguf = artifact.load_tensor_row(gguf_tensor, 0)?;
    let expected = transform_values(official.values.clone());
    let metrics = numeric_metrics(&expected, &gguf.values)?;
    Ok(Qwen38GgufTransformCheck {
        transform: String::from(transform),
        official_tensor: String::from(official_tensor),
        gguf_tensor: String::from(gguf_tensor),
        target_row: 0,
        official_source_row: 0,
        official_receipt: official.receipt,
        gguf_receipt: gguf.receipt,
        parity_passed: metrics.maximum_absolute_error <= maximum_error,
        metrics,
        untransformed_root_mean_square_error: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn mapped_row_check(
    official_model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    artifact: &GgufBlobArtifact,
    transform: &str,
    official_tensor: &str,
    gguf_tensor: &str,
    target_row: usize,
    source_row: usize,
    untransformed_row: Option<usize>,
    maximum_error: f64,
) -> Result<Qwen38GgufTransformCheck, Qwen38GgufConverterParityError> {
    let official = qwen35_text_read_indexed_tensor_row(
        official_model_dir,
        weight_map,
        official_tensor,
        source_row,
    )?;
    let gguf = artifact.load_tensor_row(gguf_tensor, target_row)?;
    let metrics = numeric_metrics(&official.values, &gguf.values)?;
    let untransformed_root_mean_square_error = untransformed_row
        .filter(|row| *row != source_row)
        .map(|row| {
            let raw = qwen35_text_read_indexed_tensor_row(
                official_model_dir,
                weight_map,
                official_tensor,
                row,
            )?;
            numeric_metrics(&raw.values, &gguf.values).map(|metrics| metrics.root_mean_square_error)
        })
        .transpose()?;
    let layout_passed = untransformed_root_mean_square_error
        .map(|raw_rmse| metrics.root_mean_square_error <= raw_rmse * 1.05)
        .unwrap_or(true);
    Ok(Qwen38GgufTransformCheck {
        transform: String::from(transform),
        official_tensor: String::from(official_tensor),
        gguf_tensor: String::from(gguf_tensor),
        target_row,
        official_source_row: source_row,
        official_receipt: official.receipt,
        gguf_receipt: gguf.receipt,
        parity_passed: layout_passed && metrics.maximum_absolute_error <= maximum_error,
        metrics,
        untransformed_root_mean_square_error,
    })
}

fn output_projection_check(
    official_model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    artifact: &GgufBlobArtifact,
    row: usize,
) -> Result<Qwen38GgufTransformCheck, Qwen38GgufConverterParityError> {
    let official_tensor = "model.language_model.layers.0.linear_attn.out_proj.weight";
    let gguf_tensor = "blk.0.ssm_out.weight";
    let official =
        qwen35_text_read_indexed_tensor_row(official_model_dir, weight_map, official_tensor, row)?;
    let expected = reorder_vector(official.values.clone());
    let gguf = artifact.load_tensor_row(gguf_tensor, row)?;
    let metrics = numeric_metrics(&expected, &gguf.values)?;
    let raw_metrics = numeric_metrics(&official.values, &gguf.values)?;
    Ok(Qwen38GgufTransformCheck {
        transform: String::from("output_projection_columns_tiled"),
        official_tensor: String::from(official_tensor),
        gguf_tensor: String::from(gguf_tensor),
        target_row: row,
        official_source_row: row,
        official_receipt: official.receipt,
        gguf_receipt: gguf.receipt,
        parity_passed: metrics.root_mean_square_error <= raw_metrics.root_mean_square_error * 1.05,
        metrics,
        untransformed_root_mean_square_error: Some(raw_metrics.root_mean_square_error),
    })
}

fn reorder_vector(values: Vec<f32>) -> Vec<f32> {
    (0..values.len())
        .map(|target| values[source_index_for_tiled(target, values.len() / NUM_VALUE_HEADS)])
        .collect()
}

fn source_index_for_tiled(target_index: usize, head_dim: usize) -> usize {
    let target_head = target_index / head_dim;
    let lane = target_index % head_dim;
    let value_slot = target_head / NUM_KEY_HEADS;
    let key_head = target_head % NUM_KEY_HEADS;
    (key_head * VALUE_HEADS_PER_KEY + value_slot) * head_dim + lane
}

fn numeric_metrics(
    reference: &[f32],
    observed: &[f32],
) -> Result<Qwen38GgufNumericMetrics, Qwen38GgufConverterParityError> {
    if reference.len() != observed.len() || reference.is_empty() {
        return parity_error(format!(
            "numeric comparison requires equal non-empty vectors, got {} and {} elements",
            reference.len(),
            observed.len()
        ));
    }
    let mut squared_error = 0.0f64;
    let mut absolute_error = 0.0f64;
    let mut maximum_absolute_error = 0.0f64;
    let mut dot = 0.0f64;
    let mut reference_norm = 0.0f64;
    let mut observed_norm = 0.0f64;
    let mut reference_projection_output = 0.0f64;
    let mut gguf_projection_output = 0.0f64;
    let element_count = reference.len();
    for (index, (&reference, &observed)) in reference.iter().zip(observed).enumerate() {
        let reference = f64::from(reference);
        let observed = f64::from(observed);
        let error = (reference - observed).abs();
        squared_error += error * error;
        absolute_error += error;
        maximum_absolute_error = maximum_absolute_error.max(error);
        dot += reference * observed;
        reference_norm += reference * reference;
        observed_norm += observed * observed;
        let input = (((index + 1) as f64 * 0.017).sin() + 0.5 * ((index + 1) as f64 * 0.013).cos())
            / (element_count as f64).sqrt();
        reference_projection_output += reference * input;
        gguf_projection_output += observed * input;
    }
    Ok(Qwen38GgufNumericMetrics {
        element_count,
        root_mean_square_error: (squared_error / element_count as f64).sqrt(),
        mean_absolute_error: absolute_error / element_count as f64,
        maximum_absolute_error,
        cosine_similarity: if reference_norm == 0.0 && observed_norm == 0.0 {
            1.0
        } else if reference_norm == 0.0 || observed_norm == 0.0 {
            0.0
        } else {
            dot / (reference_norm.sqrt() * observed_norm.sqrt())
        },
        reference_projection_output,
        gguf_projection_output,
        projection_output_absolute_error: (reference_projection_output - gguf_projection_output)
            .abs(),
    })
}

fn profile_result(
    profiles: &[Qwen38GgufProfileQuality],
    profile: Qwen38GgufProfile,
) -> Result<&Qwen38GgufProfileQuality, Qwen38GgufConverterParityError> {
    profiles
        .iter()
        .find(|result| result.profile == profile)
        .ok_or_else(|| {
            Qwen38GgufConverterParityError::Parity(format!(
                "missing profile result for {profile:?}"
            ))
        })
}

fn parity_error<T>(detail: String) -> Result<T, Qwen38GgufConverterParityError> {
    Err(Qwen38GgufConverterParityError::Parity(detail))
}

fn report_digest(report: &Qwen38GgufConverterParityReport) -> Result<String, serde_json::Error> {
    let mut canonical = report.clone();
    canonical.report_sha256.clear();
    serde_json::to_vec(&canonical).map(|bytes| hex::encode(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiled_v_head_mapping_matches_converter_contract() {
        assert_eq!(source_index_for_tiled(0, 1), 0);
        assert_eq!(source_index_for_tiled(1, 1), 3);
        assert_eq!(source_index_for_tiled(15, 1), 45);
        assert_eq!(source_index_for_tiled(16, 1), 1);
        assert_eq!(source_index_for_tiled(32, 1), 2);
        assert_eq!(source_index_for_tiled(47, 1), 47);
        assert_eq!(source_index_for_tiled(128, 128), 384);
    }

    #[test]
    fn numeric_metrics_capture_exact_and_changed_outputs() {
        let exact = numeric_metrics(&[1.0, -2.0, 3.0], &[1.0, -2.0, 3.0]).unwrap();
        assert_eq!(exact.root_mean_square_error, 0.0);
        assert_eq!(exact.cosine_similarity, 1.0);
        assert_eq!(exact.projection_output_absolute_error, 0.0);

        let changed = numeric_metrics(&[1.0, -2.0, 3.0], &[1.0, -1.0, 2.0]).unwrap();
        assert!(changed.root_mean_square_error > 0.0);
        assert!(changed.cosine_similarity < 1.0);
        assert!(changed.projection_output_absolute_error > 0.0);
    }
}
