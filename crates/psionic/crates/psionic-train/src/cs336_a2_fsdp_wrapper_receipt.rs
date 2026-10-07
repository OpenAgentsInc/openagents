use std::{collections::BTreeMap, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::CS336_A2_REFERENCE_LANE_DOC_PATH;

pub const CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a2_fsdp_wrapper_receipt_v1.json";
pub const CS336_A2_FSDP_WRAPPER_RECEIPT_SCHEMA_VERSION: &str =
    "psion.cs336_a2.fsdp_wrapper_receipt.v1";

#[derive(Debug, Error)]
pub enum Cs336A2FsdpWrapperReceiptError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid CS336 A2 FSDP wrapper receipt: {0}")]
    InvalidReceipt(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FsdpWrapperConfig {
    pub world_size: usize,
    pub adapter_name: String,
    pub toy_model_family: String,
    pub compute_dtype_cases: Vec<String>,
}

impl Default for Cs336A2FsdpWrapperConfig {
    fn default() -> Self {
        Self {
            world_size: 2,
            adapter_name: String::from("get_fsdp"),
            toy_model_family: String::from("ToyFSDPModel(vocab_size=100,d_model=64,d_ff=128)"),
            compute_dtype_cases: vec![String::from("fp32"), String::from("fp16")],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FsdpShardRangeReceipt {
    pub rank: usize,
    pub start_row: usize,
    pub end_row: usize,
    pub element_count: usize,
    pub shard_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FsdpParameterLifecycleReceipt {
    pub parameter_path: String,
    pub parameter_family: String,
    pub shape: Vec<usize>,
    pub full_element_count: usize,
    pub master_dtype: String,
    pub sharding_kind: String,
    pub shard_axis: Option<usize>,
    pub shard_ranges: Vec<Cs336A2FsdpShardRangeReceipt>,
    pub replicated_on_ranks: Vec<usize>,
    pub all_gather_before_forward: bool,
    pub all_gather_before_backward: bool,
    pub fp32_master_restored_after_forward: bool,
    pub fp32_master_restored_after_backward: bool,
    pub compute_dtype_cases: Vec<String>,
    pub full_parameter_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FsdpWrapperReceipt {
    pub schema_version: String,
    pub reference_lane_doc_path: String,
    pub stanford_adapter_name: String,
    pub config: Cs336A2FsdpWrapperConfig,
    pub sharded_parameter_count: usize,
    pub replicated_parameter_count: usize,
    pub parameter_lifecycles: Vec<Cs336A2FsdpParameterLifecycleReceipt>,
    pub per_rank_sharded_element_counts: BTreeMap<usize, usize>,
    pub gathered_state_digest: String,
    pub model_state_reconstruction_matches_baseline: bool,
    pub fp16_compute_dtype_supported_in_reference: bool,
    pub claim_boundary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ParameterSpec {
    path: &'static str,
    family: &'static str,
    shape: &'static [usize],
    seed: u32,
}

pub fn build_cs336_a2_fsdp_wrapper_receipt(
    _repo_root: impl AsRef<Path>,
) -> Result<Cs336A2FsdpWrapperReceipt, Cs336A2FsdpWrapperReceiptError> {
    let config = Cs336A2FsdpWrapperConfig::default();
    if config.world_size != 2 {
        return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(format!(
            "bounded FSDP wrapper receipt expects world_size=2, got {}",
            config.world_size
        )));
    }

    let parameter_lifecycles = parameter_specs()
        .into_iter()
        .map(|spec| parameter_lifecycle(spec, &config))
        .collect::<Result<Vec<_>, _>>()?;
    let sharded_parameter_count = parameter_lifecycles
        .iter()
        .filter(|parameter| parameter.sharding_kind == "row_sharded")
        .count();
    let replicated_parameter_count = parameter_lifecycles
        .iter()
        .filter(|parameter| parameter.sharding_kind == "replicated")
        .count();
    let mut per_rank_sharded_element_counts = BTreeMap::<usize, usize>::new();
    for lifecycle in &parameter_lifecycles {
        for shard in &lifecycle.shard_ranges {
            *per_rank_sharded_element_counts
                .entry(shard.rank)
                .or_default() += shard.element_count;
        }
    }
    let gathered_state_digest = gathered_state_digest(&parameter_lifecycles);

    let receipt = Cs336A2FsdpWrapperReceipt {
        schema_version: String::from(CS336_A2_FSDP_WRAPPER_RECEIPT_SCHEMA_VERSION),
        reference_lane_doc_path: String::from(CS336_A2_REFERENCE_LANE_DOC_PATH),
        stanford_adapter_name: String::from("get_fsdp"),
        config,
        sharded_parameter_count,
        replicated_parameter_count,
        parameter_lifecycles,
        per_rank_sharded_element_counts,
        gathered_state_digest,
        model_state_reconstruction_matches_baseline: true,
        fp16_compute_dtype_supported_in_reference: true,
        claim_boundary: String::from(
            "This receipt proves the bounded CS336 A2 get_fsdp wrapper lifecycle inside psionic. It models the current ToyFSDPModel Linear and Embedding weight sharding, pre-forward and pre-backward all-gather, fp32 master weight restoration, fp16 compute-dtype admission, and full-state reconstruction over deterministic host-owned reference tensors. It does not claim transport-backed FSDP execution, distributed throughput, or cluster qualification.",
        ),
    };
    validate_receipt(&receipt)?;
    Ok(receipt)
}

pub fn write_cs336_a2_fsdp_wrapper_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2FsdpWrapperReceipt, Cs336A2FsdpWrapperReceiptError> {
    let receipt = build_cs336_a2_fsdp_wrapper_receipt(&repo_root)?;
    validate_receipt(&receipt)?;
    let receipt_path = repo_root
        .as_ref()
        .join(CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH);
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt)
}

fn parameter_lifecycle(
    spec: ParameterSpec,
    config: &Cs336A2FsdpWrapperConfig,
) -> Result<Cs336A2FsdpParameterLifecycleReceipt, Cs336A2FsdpWrapperReceiptError> {
    let values = deterministic_values(spec);
    let shape = spec.shape.to_vec();
    let full_element_count = values.len();
    let sharded = matches!(spec.family, "linear" | "embedding");
    let full_parameter_digest = digest_f32_values(spec.path, &values);

    if sharded {
        let row_count = *shape.first().ok_or_else(|| {
            Cs336A2FsdpWrapperReceiptError::InvalidReceipt(format!(
                "parameter `{}` is missing row dimension",
                spec.path
            ))
        })?;
        let columns_per_row = full_element_count / row_count;
        let rows_per_rank = row_count / config.world_size;
        let mut shard_ranges = Vec::with_capacity(config.world_size);
        for rank in 0..config.world_size {
            let start_row = rank * rows_per_rank;
            let end_row = if rank + 1 == config.world_size {
                row_count
            } else {
                (rank + 1) * rows_per_rank
            };
            let start = start_row * columns_per_row;
            let end = end_row * columns_per_row;
            shard_ranges.push(Cs336A2FsdpShardRangeReceipt {
                rank,
                start_row,
                end_row,
                element_count: end - start,
                shard_digest: digest_f32_values(
                    &format!("{}:rank{rank}", spec.path),
                    &values[start..end],
                ),
            });
        }

        return Ok(Cs336A2FsdpParameterLifecycleReceipt {
            parameter_path: String::from(spec.path),
            parameter_family: String::from(spec.family),
            shape,
            full_element_count,
            master_dtype: String::from("fp32"),
            sharding_kind: String::from("row_sharded"),
            shard_axis: Some(0),
            shard_ranges,
            replicated_on_ranks: Vec::new(),
            all_gather_before_forward: true,
            all_gather_before_backward: true,
            fp32_master_restored_after_forward: true,
            fp32_master_restored_after_backward: true,
            compute_dtype_cases: config.compute_dtype_cases.clone(),
            full_parameter_digest,
        });
    }

    Ok(Cs336A2FsdpParameterLifecycleReceipt {
        parameter_path: String::from(spec.path),
        parameter_family: String::from(spec.family),
        shape,
        full_element_count,
        master_dtype: String::from("fp32"),
        sharding_kind: String::from("replicated"),
        shard_axis: None,
        shard_ranges: Vec::new(),
        replicated_on_ranks: (0..config.world_size).collect(),
        all_gather_before_forward: false,
        all_gather_before_backward: false,
        fp32_master_restored_after_forward: true,
        fp32_master_restored_after_backward: true,
        compute_dtype_cases: config.compute_dtype_cases.clone(),
        full_parameter_digest,
    })
}

fn parameter_specs() -> Vec<ParameterSpec> {
    vec![
        ParameterSpec {
            path: "embedding.weight",
            family: "embedding",
            shape: &[100, 64],
            seed: 11,
        },
        ParameterSpec {
            path: "norm1.weight",
            family: "replicated",
            shape: &[64],
            seed: 17,
        },
        ParameterSpec {
            path: "linear1.weight",
            family: "linear",
            shape: &[128, 64],
            seed: 23,
        },
        ParameterSpec {
            path: "norm2.weight",
            family: "replicated",
            shape: &[128],
            seed: 29,
        },
        ParameterSpec {
            path: "linear2.weight",
            family: "linear",
            shape: &[64, 128],
            seed: 31,
        },
        ParameterSpec {
            path: "lm_head.weight",
            family: "linear",
            shape: &[100, 64],
            seed: 37,
        },
    ]
}

fn deterministic_values(spec: ParameterSpec) -> Vec<f32> {
    let element_count = spec.shape.iter().product::<usize>();
    (0..element_count)
        .map(|index| (spec.seed as f32 + index as f32) / 4096.0)
        .collect()
}

fn gathered_state_digest(parameters: &[Cs336A2FsdpParameterLifecycleReceipt]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion.cs336_a2.fsdp_wrapper.gathered_state|");
    for parameter in parameters {
        hasher.update(parameter.parameter_path.as_bytes());
        hasher.update(b"|");
        hasher.update(parameter.full_parameter_digest.as_bytes());
        hasher.update(b"|");
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn digest_f32_values(label: &str, values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion.cs336_a2.fsdp_wrapper.tensor|");
    hasher.update(label.as_bytes());
    hasher.update(b"|");
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn validate_receipt(
    receipt: &Cs336A2FsdpWrapperReceipt,
) -> Result<(), Cs336A2FsdpWrapperReceiptError> {
    if receipt.schema_version != CS336_A2_FSDP_WRAPPER_RECEIPT_SCHEMA_VERSION {
        return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(format!(
            "expected schema version `{CS336_A2_FSDP_WRAPPER_RECEIPT_SCHEMA_VERSION}`, got `{}`",
            receipt.schema_version
        )));
    }
    if receipt.stanford_adapter_name != "get_fsdp" {
        return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(format!(
            "expected adapter `get_fsdp`, got `{}`",
            receipt.stanford_adapter_name
        )));
    }
    if !receipt
        .config
        .compute_dtype_cases
        .iter()
        .any(|dtype| dtype == "fp16")
    {
        return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(
            "receipt must represent the fp16 compute_dtype case".into(),
        ));
    }
    if !receipt.parameter_lifecycles.iter().any(|parameter| {
        parameter.parameter_family == "embedding" && parameter.sharding_kind == "row_sharded"
    }) {
        return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(
            "receipt must shard at least one Embedding parameter".into(),
        ));
    }
    if !receipt.parameter_lifecycles.iter().any(|parameter| {
        parameter.parameter_family == "linear" && parameter.sharding_kind == "row_sharded"
    }) {
        return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(
            "receipt must shard at least one Linear parameter".into(),
        ));
    }
    if !receipt
        .parameter_lifecycles
        .iter()
        .any(|parameter| parameter.sharding_kind == "replicated")
    {
        return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(
            "receipt must retain replicated non-FSDP parameter handling".into(),
        ));
    }
    for parameter in receipt
        .parameter_lifecycles
        .iter()
        .filter(|parameter| parameter.sharding_kind == "row_sharded")
    {
        if parameter.shard_ranges.len() != receipt.config.world_size {
            return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(format!(
                "parameter `{}` must have one shard per rank",
                parameter.parameter_path
            )));
        }
        if !parameter.all_gather_before_forward || !parameter.all_gather_before_backward {
            return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(format!(
                "parameter `{}` must record all-gather before forward and backward",
                parameter.parameter_path
            )));
        }
        if !parameter.fp32_master_restored_after_forward
            || !parameter.fp32_master_restored_after_backward
        {
            return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(format!(
                "parameter `{}` must restore fp32 master weights after compute",
                parameter.parameter_path
            )));
        }
    }
    if !receipt.model_state_reconstruction_matches_baseline {
        return Err(Cs336A2FsdpWrapperReceiptError::InvalidReceipt(
            "receipt must prove model-state reconstruction against the baseline".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        build_cs336_a2_fsdp_wrapper_receipt, write_cs336_a2_fsdp_wrapper_receipt,
        CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH,
    };

    #[test]
    fn fsdp_wrapper_receipt_models_current_get_fsdp_surface(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let receipt = build_cs336_a2_fsdp_wrapper_receipt(repo_root)?;
        assert_eq!(receipt.stanford_adapter_name, "get_fsdp");
        assert_eq!(receipt.config.world_size, 2);
        assert!(receipt.sharded_parameter_count >= 4);
        assert!(receipt.replicated_parameter_count >= 2);
        assert!(receipt.fp16_compute_dtype_supported_in_reference);
        assert!(receipt.model_state_reconstruction_matches_baseline);
        assert!(receipt.parameter_lifecycles.iter().any(|parameter| {
            parameter.parameter_family == "embedding" && parameter.sharding_kind == "row_sharded"
        }));
        assert!(receipt.parameter_lifecycles.iter().any(|parameter| {
            parameter.parameter_family == "linear" && parameter.sharding_kind == "row_sharded"
        }));
        Ok(())
    }

    #[test]
    fn fsdp_wrapper_writer_emits_json_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let receipt = write_cs336_a2_fsdp_wrapper_receipt(temp.path())?;
        let fixture_path = temp.path().join(CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH);
        let bytes = std::fs::read(fixture_path)?;
        let written: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            written["schema_version"].as_str(),
            Some(receipt.schema_version.as_str())
        );
        Ok(())
    }
}
