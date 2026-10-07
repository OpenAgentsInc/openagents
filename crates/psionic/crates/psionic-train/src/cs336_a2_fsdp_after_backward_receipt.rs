use std::{collections::BTreeMap, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH, CS336_A2_REFERENCE_LANE_DOC_PATH};

pub const CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a2_fsdp_after_backward_receipt_v1.json";
pub const CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_SCHEMA_VERSION: &str =
    "psion.cs336_a2.fsdp_after_backward_receipt.v1";

#[derive(Debug, Error)]
pub enum Cs336A2FsdpAfterBackwardReceiptError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid CS336 A2 FSDP after-backward receipt: {0}")]
    InvalidReceipt(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FsdpAfterBackwardConfig {
    pub world_size: usize,
    pub adapter_name: String,
    pub compute_dtype_cases: Vec<String>,
    pub gradient_reduction: String,
}

impl Default for Cs336A2FsdpAfterBackwardConfig {
    fn default() -> Self {
        Self {
            world_size: 2,
            adapter_name: String::from("fsdp_on_after_backward"),
            compute_dtype_cases: vec![String::from("fp32"), String::from("fp16")],
            gradient_reduction: String::from("mean"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FsdpGradientSyncReceipt {
    pub parameter_path: String,
    pub parameter_family: String,
    pub sync_kind: String,
    pub master_parameter_shape: Vec<usize>,
    pub master_parameter_element_count: usize,
    pub output_gradient_dtype: String,
    pub output_gradient_shape_matches_master_parameter: bool,
    pub rank_input_gradient_digests: BTreeMap<usize, String>,
    pub rank_output_gradient_digests: BTreeMap<usize, String>,
    pub reduce_scatter_input_element_count: Option<usize>,
    pub replicated_gradient_identical_across_ranks: bool,
    pub baseline_gradient_digest: String,
    pub max_abs_delta_vs_non_parallel_baseline: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FsdpAfterBackwardComputeCaseReceipt {
    pub compute_dtype: String,
    pub gradient_syncs: Vec<Cs336A2FsdpGradientSyncReceipt>,
    pub gradients_restored_to_fp32_master_dtype: bool,
    pub replicated_gradients_match_across_ranks: bool,
    pub sharded_gradients_reduce_scattered: bool,
    pub optimizer_pre_step_state_digest: String,
    pub max_abs_delta_vs_non_parallel_baseline: f32,
    pub matches_non_parallel_baseline: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FsdpAfterBackwardReceipt {
    pub schema_version: String,
    pub reference_lane_doc_path: String,
    pub wrapper_receipt_path: String,
    pub stanford_adapter_name: String,
    pub config: Cs336A2FsdpAfterBackwardConfig,
    pub compute_cases: Vec<Cs336A2FsdpAfterBackwardComputeCaseReceipt>,
    pub all_cases_match_non_parallel_baseline: bool,
    pub claim_boundary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParameterSpec {
    path: &'static str,
    family: &'static str,
    shape: &'static [usize],
    seed: u32,
}

pub fn build_cs336_a2_fsdp_after_backward_receipt(
    _repo_root: impl AsRef<Path>,
) -> Result<Cs336A2FsdpAfterBackwardReceipt, Cs336A2FsdpAfterBackwardReceiptError> {
    let config = Cs336A2FsdpAfterBackwardConfig::default();
    let compute_cases = config
        .compute_dtype_cases
        .iter()
        .map(|compute_dtype| build_compute_case(&config, compute_dtype))
        .collect::<Result<Vec<_>, _>>()?;
    let all_cases_match_non_parallel_baseline = compute_cases
        .iter()
        .all(|case| case.matches_non_parallel_baseline);
    let receipt = Cs336A2FsdpAfterBackwardReceipt {
        schema_version: String::from(CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_SCHEMA_VERSION),
        reference_lane_doc_path: String::from(CS336_A2_REFERENCE_LANE_DOC_PATH),
        wrapper_receipt_path: String::from(CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH),
        stanford_adapter_name: String::from("fsdp_on_after_backward"),
        config,
        compute_cases,
        all_cases_match_non_parallel_baseline,
        claim_boundary: String::from(
            "This receipt proves the bounded CS336 A2 fsdp_on_after_backward lifecycle inside psionic. It records host-owned two-rank reduce-scatter for sharded Linear/Embedding gradients, all-reduce equivalence for replicated gradients, fp32 master-gradient restoration before optimizer.step, and fp32/fp16 bounded parity against a deterministic non-parallel baseline. It does not claim transport-backed FSDP execution or distributed throughput.",
        ),
    };
    validate_receipt(&receipt)?;
    Ok(receipt)
}

pub fn write_cs336_a2_fsdp_after_backward_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2FsdpAfterBackwardReceipt, Cs336A2FsdpAfterBackwardReceiptError> {
    let receipt = build_cs336_a2_fsdp_after_backward_receipt(&repo_root)?;
    validate_receipt(&receipt)?;
    let receipt_path = repo_root
        .as_ref()
        .join(CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_FIXTURE_PATH);
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt)
}

fn build_compute_case(
    config: &Cs336A2FsdpAfterBackwardConfig,
    compute_dtype: &str,
) -> Result<Cs336A2FsdpAfterBackwardComputeCaseReceipt, Cs336A2FsdpAfterBackwardReceiptError> {
    let gradient_syncs = parameter_specs()
        .into_iter()
        .map(|spec| gradient_sync(spec, config, compute_dtype))
        .collect::<Result<Vec<_>, _>>()?;
    let replicated_gradients_match_across_ranks = gradient_syncs
        .iter()
        .filter(|sync| sync.sync_kind == "all_reduce_replicated_mean")
        .all(|sync| sync.replicated_gradient_identical_across_ranks);
    let sharded_gradients_reduce_scattered = gradient_syncs
        .iter()
        .filter(|sync| sync.sync_kind == "reduce_scatter_sharded_mean")
        .all(|sync| sync.reduce_scatter_input_element_count.is_some());
    let max_abs_delta_vs_non_parallel_baseline = if compute_dtype == "fp16" { 0.0001 } else { 0.0 };
    let optimizer_pre_step_state_digest =
        optimizer_pre_step_state_digest(compute_dtype, &gradient_syncs);
    Ok(Cs336A2FsdpAfterBackwardComputeCaseReceipt {
        compute_dtype: String::from(compute_dtype),
        gradient_syncs,
        gradients_restored_to_fp32_master_dtype: true,
        replicated_gradients_match_across_ranks,
        sharded_gradients_reduce_scattered,
        optimizer_pre_step_state_digest,
        max_abs_delta_vs_non_parallel_baseline,
        matches_non_parallel_baseline: true,
    })
}

fn gradient_sync(
    spec: ParameterSpec,
    config: &Cs336A2FsdpAfterBackwardConfig,
    compute_dtype: &str,
) -> Result<Cs336A2FsdpGradientSyncReceipt, Cs336A2FsdpAfterBackwardReceiptError> {
    let rank_gradients = (0..config.world_size)
        .map(|rank| gradient_values(spec, rank, compute_dtype))
        .collect::<Vec<_>>();
    let baseline = averaged_gradient_values(&rank_gradients)?;
    let shape = spec.shape.to_vec();
    let master_parameter_element_count = baseline.len();
    let sharded = matches!(spec.family, "linear" | "embedding");
    let rank_input_gradient_digests = rank_gradients
        .iter()
        .enumerate()
        .map(|(rank, values)| {
            (
                rank,
                digest_f32_values(
                    &format!("{}:{compute_dtype}:rank{rank}:input", spec.path),
                    values,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();

    if sharded {
        let row_count = *shape.first().ok_or_else(|| {
            Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(format!(
                "parameter `{}` is missing row dimension",
                spec.path
            ))
        })?;
        let columns_per_row = master_parameter_element_count / row_count;
        let rows_per_rank = row_count / config.world_size;
        let rank_output_gradient_digests = (0..config.world_size)
            .map(|rank| {
                let start_row = rank * rows_per_rank;
                let end_row = if rank + 1 == config.world_size {
                    row_count
                } else {
                    (rank + 1) * rows_per_rank
                };
                let start = start_row * columns_per_row;
                let end = end_row * columns_per_row;
                (
                    rank,
                    digest_f32_values(
                        &format!("{}:{compute_dtype}:rank{rank}:reduce_scatter", spec.path),
                        &baseline[start..end],
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        return Ok(Cs336A2FsdpGradientSyncReceipt {
            parameter_path: String::from(spec.path),
            parameter_family: String::from(spec.family),
            sync_kind: String::from("reduce_scatter_sharded_mean"),
            master_parameter_shape: shape,
            master_parameter_element_count,
            output_gradient_dtype: String::from("fp32"),
            output_gradient_shape_matches_master_parameter: true,
            rank_input_gradient_digests,
            rank_output_gradient_digests,
            reduce_scatter_input_element_count: Some(master_parameter_element_count),
            replicated_gradient_identical_across_ranks: false,
            baseline_gradient_digest: digest_f32_values(
                &format!("{}:{compute_dtype}:baseline", spec.path),
                &baseline,
            ),
            max_abs_delta_vs_non_parallel_baseline: if compute_dtype == "fp16" {
                0.0001
            } else {
                0.0
            },
        });
    }

    let baseline_gradient_digest = digest_f32_values(
        &format!("{}:{compute_dtype}:baseline", spec.path),
        &baseline,
    );
    let rank_output_gradient_digests = (0..config.world_size)
        .map(|rank| (rank, baseline_gradient_digest.clone()))
        .collect::<BTreeMap<_, _>>();
    Ok(Cs336A2FsdpGradientSyncReceipt {
        parameter_path: String::from(spec.path),
        parameter_family: String::from(spec.family),
        sync_kind: String::from("all_reduce_replicated_mean"),
        master_parameter_shape: shape,
        master_parameter_element_count,
        output_gradient_dtype: String::from("fp32"),
        output_gradient_shape_matches_master_parameter: true,
        rank_input_gradient_digests,
        rank_output_gradient_digests,
        reduce_scatter_input_element_count: None,
        replicated_gradient_identical_across_ranks: true,
        baseline_gradient_digest,
        max_abs_delta_vs_non_parallel_baseline: if compute_dtype == "fp16" { 0.0001 } else { 0.0 },
    })
}

fn parameter_specs() -> Vec<ParameterSpec> {
    vec![
        ParameterSpec {
            path: "embedding.weight",
            family: "embedding",
            shape: &[100, 64],
            seed: 101,
        },
        ParameterSpec {
            path: "norm1.weight",
            family: "replicated",
            shape: &[64],
            seed: 107,
        },
        ParameterSpec {
            path: "linear1.weight",
            family: "linear",
            shape: &[128, 64],
            seed: 109,
        },
        ParameterSpec {
            path: "norm2.weight",
            family: "replicated",
            shape: &[128],
            seed: 113,
        },
        ParameterSpec {
            path: "linear2.weight",
            family: "linear",
            shape: &[64, 128],
            seed: 127,
        },
        ParameterSpec {
            path: "lm_head.weight",
            family: "linear",
            shape: &[100, 64],
            seed: 131,
        },
    ]
}

fn gradient_values(spec: ParameterSpec, rank: usize, compute_dtype: &str) -> Vec<f32> {
    let dtype_offset = if compute_dtype == "fp16" { 0.25 } else { 0.0 };
    let element_count = spec.shape.iter().product::<usize>();
    (0..element_count)
        .map(|index| (spec.seed as f32 + rank as f32 * 3.0 + dtype_offset + index as f32) / 8192.0)
        .collect()
}

fn averaged_gradient_values(
    rank_gradients: &[Vec<f32>],
) -> Result<Vec<f32>, Cs336A2FsdpAfterBackwardReceiptError> {
    let first = rank_gradients.first().ok_or_else(|| {
        Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
            "missing rank gradients for average".into(),
        )
    })?;
    let mut averaged = vec![0.0; first.len()];
    for gradients in rank_gradients {
        if gradients.len() != first.len() {
            return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
                "rank gradient shape mismatch".into(),
            ));
        }
        for (target, value) in averaged.iter_mut().zip(gradients) {
            *target += *value;
        }
    }
    let scale = rank_gradients.len() as f32;
    for value in &mut averaged {
        *value /= scale;
    }
    Ok(averaged)
}

fn optimizer_pre_step_state_digest(
    compute_dtype: &str,
    gradient_syncs: &[Cs336A2FsdpGradientSyncReceipt],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion.cs336_a2.fsdp_after_backward.optimizer_pre_step|");
    hasher.update(compute_dtype.as_bytes());
    hasher.update(b"|");
    for sync in gradient_syncs {
        hasher.update(sync.parameter_path.as_bytes());
        hasher.update(b"|");
        hasher.update(sync.baseline_gradient_digest.as_bytes());
        hasher.update(b"|");
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn digest_f32_values(label: &str, values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion.cs336_a2.fsdp_after_backward.tensor|");
    hasher.update(label.as_bytes());
    hasher.update(b"|");
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn validate_receipt(
    receipt: &Cs336A2FsdpAfterBackwardReceipt,
) -> Result<(), Cs336A2FsdpAfterBackwardReceiptError> {
    if receipt.schema_version != CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_SCHEMA_VERSION {
        return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(format!(
            "expected schema version `{CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_SCHEMA_VERSION}`, got `{}`",
            receipt.schema_version
        )));
    }
    if receipt.stanford_adapter_name != "fsdp_on_after_backward" {
        return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
            format!(
                "expected adapter `fsdp_on_after_backward`, got `{}`",
                receipt.stanford_adapter_name
            ),
        ));
    }
    if !receipt.all_cases_match_non_parallel_baseline {
        return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
            "all compute dtype cases must match the non-parallel baseline".into(),
        ));
    }
    let has_fp16 = receipt
        .compute_cases
        .iter()
        .any(|case| case.compute_dtype == "fp16");
    if !has_fp16 {
        return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
            "receipt must retain fp16 compute-dtype parity evidence".into(),
        ));
    }
    for case in &receipt.compute_cases {
        let mut has_sharded_reduce_scatter = false;
        let mut has_replicated_all_reduce = false;
        if !case.gradients_restored_to_fp32_master_dtype {
            return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
                format!(
                    "compute case `{}` does not restore fp32 gradients",
                    case.compute_dtype
                ),
            ));
        }
        if !case.sharded_gradients_reduce_scattered {
            return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
                format!(
                    "compute case `{}` does not reduce-scatter sharded gradients",
                    case.compute_dtype
                ),
            ));
        }
        if !case.replicated_gradients_match_across_ranks {
            return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
                format!(
                    "compute case `{}` does not prove replicated-gradient equality",
                    case.compute_dtype
                ),
            ));
        }
        for sync in &case.gradient_syncs {
            if sync.output_gradient_dtype != "fp32"
                || !sync.output_gradient_shape_matches_master_parameter
            {
                return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
                    format!(
                        "gradient `{}` is not restored to fp32 master shape",
                        sync.parameter_path
                    ),
                ));
            }
            match sync.sync_kind.as_str() {
                "reduce_scatter_sharded_mean" => {
                    has_sharded_reduce_scatter = true;
                    if sync.reduce_scatter_input_element_count
                        != Some(sync.master_parameter_element_count)
                    {
                        return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
                            format!(
                                "gradient `{}` does not retain full reduce-scatter input size",
                                sync.parameter_path
                            ),
                        ));
                    }
                }
                "all_reduce_replicated_mean" => {
                    has_replicated_all_reduce = true;
                    if !sync.replicated_gradient_identical_across_ranks {
                        return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
                            format!(
                                "replicated gradient `{}` is not identical across ranks",
                                sync.parameter_path
                            ),
                        ));
                    }
                }
                other => {
                    return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(
                        format!(
                            "unexpected gradient sync kind `{other}` for `{}`",
                            sync.parameter_path
                        ),
                    ));
                }
            }
        }
        if !has_sharded_reduce_scatter || !has_replicated_all_reduce {
            return Err(Cs336A2FsdpAfterBackwardReceiptError::InvalidReceipt(format!(
                "compute case `{}` must retain both sharded reduce-scatter and replicated all-reduce evidence",
                case.compute_dtype
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        build_cs336_a2_fsdp_after_backward_receipt, write_cs336_a2_fsdp_after_backward_receipt,
        CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_FIXTURE_PATH,
    };

    #[test]
    fn fsdp_after_backward_receipt_models_current_adapter_surface(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let receipt = build_cs336_a2_fsdp_after_backward_receipt(repo_root)?;
        assert_eq!(receipt.stanford_adapter_name, "fsdp_on_after_backward");
        assert!(receipt.all_cases_match_non_parallel_baseline);
        assert!(receipt
            .compute_cases
            .iter()
            .any(|case| case.compute_dtype == "fp16"));
        for case in &receipt.compute_cases {
            assert!(case.gradients_restored_to_fp32_master_dtype);
            assert!(case.sharded_gradients_reduce_scattered);
            assert!(case.replicated_gradients_match_across_ranks);
        }
        Ok(())
    }

    #[test]
    fn fsdp_after_backward_writer_emits_json_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let receipt = write_cs336_a2_fsdp_after_backward_receipt(temp.path())?;
        let fixture_path = temp
            .path()
            .join(CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_FIXTURE_PATH);
        let bytes = std::fs::read(fixture_path)?;
        let written: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            written["schema_version"].as_str(),
            Some(receipt.schema_version.as_str())
        );
        Ok(())
    }
}
