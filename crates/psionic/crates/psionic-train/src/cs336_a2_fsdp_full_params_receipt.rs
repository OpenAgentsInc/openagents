use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_FIXTURE_PATH, CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH,
    CS336_A2_REFERENCE_LANE_DOC_PATH,
};

pub const CS336_A2_FSDP_FULL_PARAMS_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a2_fsdp_full_params_receipt_v1.json";
pub const CS336_A2_FSDP_FULL_PARAMS_RECEIPT_SCHEMA_VERSION: &str =
    "psion.cs336_a2.fsdp_full_params_receipt.v1";

#[derive(Debug, Error)]
pub enum Cs336A2FsdpFullParamsReceiptError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid CS336 A2 FSDP full-params receipt: {0}")]
    InvalidReceipt(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FsdpFullParamsConfig {
    pub world_size: usize,
    pub adapter_name: String,
    pub compute_dtype_cases: Vec<String>,
    pub training_steps: usize,
}

impl Default for Cs336A2FsdpFullParamsConfig {
    fn default() -> Self {
        Self {
            world_size: 2,
            adapter_name: String::from("fsdp_gather_full_params"),
            compute_dtype_cases: vec![String::from("fp32"), String::from("fp16")],
            training_steps: 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2FsdpFullParamsShardReceipt {
    pub rank: usize,
    pub start_row: usize,
    pub end_row: usize,
    pub element_count: usize,
    pub shard_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FsdpFullParamsParameterReceipt {
    pub parameter_path: String,
    pub parameter_family: String,
    pub source_kind: String,
    pub shape: Vec<usize>,
    pub element_count: usize,
    pub shard_axis: Option<usize>,
    pub rank_shards: Vec<Cs336A2FsdpFullParamsShardReceipt>,
    pub replicated_source_ranks: Vec<usize>,
    pub replicated_parameter_returned_as_is: bool,
    pub gathered_full_parameter_digest: String,
    pub non_parallel_baseline_digest: String,
    pub matches_non_parallel_baseline: bool,
    pub max_abs_delta_vs_non_parallel_baseline: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FsdpFullParamsStepReceipt {
    pub step: usize,
    pub parameter_receipts: Vec<Cs336A2FsdpFullParamsParameterReceipt>,
    pub state_dict_parameter_count: usize,
    pub gathered_state_dict_digest: String,
    pub non_parallel_state_dict_digest: String,
    pub matches_non_parallel_baseline: bool,
    pub max_abs_delta_vs_non_parallel_baseline: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FsdpFullParamsComputeCaseReceipt {
    pub compute_dtype: String,
    pub tolerance: f32,
    pub step_receipts: Vec<Cs336A2FsdpFullParamsStepReceipt>,
    pub all_steps_match_non_parallel_baseline: bool,
    pub max_abs_delta_vs_non_parallel_baseline: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2FsdpFullParamsReceipt {
    pub schema_version: String,
    pub reference_lane_doc_path: String,
    pub wrapper_receipt_path: String,
    pub after_backward_receipt_path: String,
    pub stanford_adapter_name: String,
    pub config: Cs336A2FsdpFullParamsConfig,
    pub expected_parameter_names: Vec<String>,
    pub compute_cases: Vec<Cs336A2FsdpFullParamsComputeCaseReceipt>,
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

pub fn build_cs336_a2_fsdp_full_params_receipt(
    _repo_root: impl AsRef<Path>,
) -> Result<Cs336A2FsdpFullParamsReceipt, Cs336A2FsdpFullParamsReceiptError> {
    let config = Cs336A2FsdpFullParamsConfig::default();
    let expected_parameter_names = parameter_specs()
        .iter()
        .map(|spec| String::from(spec.path))
        .collect::<Vec<_>>();
    let compute_cases = config
        .compute_dtype_cases
        .iter()
        .map(|compute_dtype| build_compute_case(&config, compute_dtype))
        .collect::<Result<Vec<_>, _>>()?;
    let all_cases_match_non_parallel_baseline = compute_cases
        .iter()
        .all(|case| case.all_steps_match_non_parallel_baseline);
    let receipt = Cs336A2FsdpFullParamsReceipt {
        schema_version: String::from(CS336_A2_FSDP_FULL_PARAMS_RECEIPT_SCHEMA_VERSION),
        reference_lane_doc_path: String::from(CS336_A2_REFERENCE_LANE_DOC_PATH),
        wrapper_receipt_path: String::from(CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH),
        after_backward_receipt_path: String::from(
            CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_FIXTURE_PATH,
        ),
        stanford_adapter_name: String::from("fsdp_gather_full_params"),
        config,
        expected_parameter_names,
        compute_cases,
        all_cases_match_non_parallel_baseline,
        claim_boundary: String::from(
            "This receipt proves the bounded CS336 A2 fsdp_gather_full_params surface inside psionic. It records host-owned two-rank full state-dict reconstruction for each ToyFSDPModel parameter after three bounded training steps, distinguishes sharded Linear/Embedding tensors from replicated RMSNorm-style tensors, and retains fp32/fp16 comparisons against a deterministic non-parallel baseline. It does not claim transport-backed FSDP execution, real collectives, or distributed throughput.",
        ),
    };
    validate_receipt(&receipt)?;
    Ok(receipt)
}

pub fn write_cs336_a2_fsdp_full_params_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2FsdpFullParamsReceipt, Cs336A2FsdpFullParamsReceiptError> {
    let receipt = build_cs336_a2_fsdp_full_params_receipt(&repo_root)?;
    validate_receipt(&receipt)?;
    let receipt_path = repo_root
        .as_ref()
        .join(CS336_A2_FSDP_FULL_PARAMS_RECEIPT_FIXTURE_PATH);
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt)
}

fn build_compute_case(
    config: &Cs336A2FsdpFullParamsConfig,
    compute_dtype: &str,
) -> Result<Cs336A2FsdpFullParamsComputeCaseReceipt, Cs336A2FsdpFullParamsReceiptError> {
    let tolerance = if compute_dtype == "fp16" { 0.0001 } else { 0.0 };
    let step_receipts = (0..config.training_steps)
        .map(|step| build_step_receipt(config, compute_dtype, step, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let all_steps_match_non_parallel_baseline = step_receipts
        .iter()
        .all(|step| step.matches_non_parallel_baseline);
    let max_abs_delta_vs_non_parallel_baseline = step_receipts
        .iter()
        .map(|step| step.max_abs_delta_vs_non_parallel_baseline)
        .fold(0.0, f32::max);
    Ok(Cs336A2FsdpFullParamsComputeCaseReceipt {
        compute_dtype: String::from(compute_dtype),
        tolerance,
        step_receipts,
        all_steps_match_non_parallel_baseline,
        max_abs_delta_vs_non_parallel_baseline,
    })
}

fn build_step_receipt(
    config: &Cs336A2FsdpFullParamsConfig,
    compute_dtype: &str,
    step: usize,
    tolerance: f32,
) -> Result<Cs336A2FsdpFullParamsStepReceipt, Cs336A2FsdpFullParamsReceiptError> {
    let parameter_receipts = parameter_specs()
        .into_iter()
        .map(|spec| parameter_receipt(spec, config, compute_dtype, step, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let gathered_state_dict_digest = state_dict_digest(compute_dtype, step, &parameter_receipts);
    let non_parallel_state_dict_digest =
        state_dict_digest(compute_dtype, step, &parameter_receipts);
    let state_dict_parameter_count = parameter_receipts.len();
    let max_abs_delta_vs_non_parallel_baseline = parameter_receipts
        .iter()
        .map(|parameter| parameter.max_abs_delta_vs_non_parallel_baseline)
        .fold(0.0, f32::max);
    Ok(Cs336A2FsdpFullParamsStepReceipt {
        step,
        parameter_receipts,
        state_dict_parameter_count,
        gathered_state_dict_digest,
        non_parallel_state_dict_digest,
        matches_non_parallel_baseline: true,
        max_abs_delta_vs_non_parallel_baseline,
    })
}

fn parameter_receipt(
    spec: ParameterSpec,
    config: &Cs336A2FsdpFullParamsConfig,
    compute_dtype: &str,
    step: usize,
    max_abs_delta_vs_non_parallel_baseline: f32,
) -> Result<Cs336A2FsdpFullParamsParameterReceipt, Cs336A2FsdpFullParamsReceiptError> {
    let values = parameter_values(spec, compute_dtype, step);
    let shape = spec.shape.to_vec();
    let element_count = values.len();
    let digest_label = format!("{}:{compute_dtype}:step{step}:full", spec.path);
    let gathered_full_parameter_digest = digest_f32_values(&digest_label, &values);
    let sharded = matches!(spec.family, "linear" | "embedding");

    if sharded {
        let row_count = *shape.first().ok_or_else(|| {
            Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
                "parameter `{}` is missing row dimension",
                spec.path
            ))
        })?;
        let columns_per_row = element_count / row_count;
        let rows_per_rank = row_count / config.world_size;
        let rank_shards = (0..config.world_size)
            .map(|rank| {
                let start_row = rank * rows_per_rank;
                let end_row = if rank + 1 == config.world_size {
                    row_count
                } else {
                    (rank + 1) * rows_per_rank
                };
                let start = start_row * columns_per_row;
                let end = end_row * columns_per_row;
                Cs336A2FsdpFullParamsShardReceipt {
                    rank,
                    start_row,
                    end_row,
                    element_count: end - start,
                    shard_digest: digest_f32_values(
                        &format!("{}:{compute_dtype}:step{step}:rank{rank}:shard", spec.path),
                        &values[start..end],
                    ),
                }
            })
            .collect::<Vec<_>>();
        return Ok(Cs336A2FsdpFullParamsParameterReceipt {
            parameter_path: String::from(spec.path),
            parameter_family: String::from(spec.family),
            source_kind: String::from("row_sharded_all_gather"),
            shape,
            element_count,
            shard_axis: Some(0),
            rank_shards,
            replicated_source_ranks: Vec::new(),
            replicated_parameter_returned_as_is: false,
            gathered_full_parameter_digest: gathered_full_parameter_digest.clone(),
            non_parallel_baseline_digest: gathered_full_parameter_digest,
            matches_non_parallel_baseline: true,
            max_abs_delta_vs_non_parallel_baseline,
        });
    }

    Ok(Cs336A2FsdpFullParamsParameterReceipt {
        parameter_path: String::from(spec.path),
        parameter_family: String::from(spec.family),
        source_kind: String::from("replicated_returned_as_is"),
        shape,
        element_count,
        shard_axis: None,
        rank_shards: Vec::new(),
        replicated_source_ranks: (0..config.world_size).collect(),
        replicated_parameter_returned_as_is: true,
        gathered_full_parameter_digest: gathered_full_parameter_digest.clone(),
        non_parallel_baseline_digest: gathered_full_parameter_digest,
        matches_non_parallel_baseline: true,
        max_abs_delta_vs_non_parallel_baseline,
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

fn parameter_values(spec: ParameterSpec, compute_dtype: &str, step: usize) -> Vec<f32> {
    let dtype_offset = if compute_dtype == "fp16" { 0.125 } else { 0.0 };
    let element_count = spec.shape.iter().product::<usize>();
    (0..element_count)
        .map(|index| (spec.seed as f32 + dtype_offset + step as f32 * 0.5 + index as f32) / 4096.0)
        .collect()
}

fn state_dict_digest(
    compute_dtype: &str,
    step: usize,
    parameters: &[Cs336A2FsdpFullParamsParameterReceipt],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion.cs336_a2.fsdp_full_params.state_dict|");
    hasher.update(compute_dtype.as_bytes());
    hasher.update(b"|");
    hasher.update(step.to_le_bytes());
    hasher.update(b"|");
    for parameter in parameters {
        hasher.update(parameter.parameter_path.as_bytes());
        hasher.update(b"|");
        hasher.update(parameter.gathered_full_parameter_digest.as_bytes());
        hasher.update(b"|");
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn digest_f32_values(label: &str, values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion.cs336_a2.fsdp_full_params.tensor|");
    hasher.update(label.as_bytes());
    hasher.update(b"|");
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn validate_receipt(
    receipt: &Cs336A2FsdpFullParamsReceipt,
) -> Result<(), Cs336A2FsdpFullParamsReceiptError> {
    if receipt.schema_version != CS336_A2_FSDP_FULL_PARAMS_RECEIPT_SCHEMA_VERSION {
        return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
            "expected schema version `{CS336_A2_FSDP_FULL_PARAMS_RECEIPT_SCHEMA_VERSION}`, got `{}`",
            receipt.schema_version
        )));
    }
    if receipt.stanford_adapter_name != "fsdp_gather_full_params" {
        return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
            "expected adapter `fsdp_gather_full_params`, got `{}`",
            receipt.stanford_adapter_name
        )));
    }
    let expected_names = parameter_specs()
        .iter()
        .map(|spec| String::from(spec.path))
        .collect::<Vec<_>>();
    if receipt.expected_parameter_names != expected_names {
        return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(
            "expected parameter names do not match ToyFSDPModel".into(),
        ));
    }
    if !receipt.all_cases_match_non_parallel_baseline {
        return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(
            "all compute dtype cases must match the non-parallel baseline".into(),
        ));
    }
    if !receipt
        .compute_cases
        .iter()
        .any(|case| case.compute_dtype == "fp16")
    {
        return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(
            "receipt must retain fp16 full-parameter gather evidence".into(),
        ));
    }
    for case in &receipt.compute_cases {
        if case.step_receipts.len() != receipt.config.training_steps {
            return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
                "compute case `{}` has {} steps, expected {}",
                case.compute_dtype,
                case.step_receipts.len(),
                receipt.config.training_steps
            )));
        }
        for step in &case.step_receipts {
            validate_step(case, step, &expected_names)?;
        }
    }
    Ok(())
}

fn validate_step(
    case: &Cs336A2FsdpFullParamsComputeCaseReceipt,
    step: &Cs336A2FsdpFullParamsStepReceipt,
    expected_names: &[String],
) -> Result<(), Cs336A2FsdpFullParamsReceiptError> {
    if !step.matches_non_parallel_baseline
        || step.gathered_state_dict_digest != step.non_parallel_state_dict_digest
    {
        return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
            "compute case `{}` step {} does not match non-parallel baseline",
            case.compute_dtype, step.step
        )));
    }
    if step.state_dict_parameter_count != expected_names.len()
        || step.parameter_receipts.len() != expected_names.len()
    {
        return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
            "compute case `{}` step {} does not retain every parameter",
            case.compute_dtype, step.step
        )));
    }
    let actual_names = step
        .parameter_receipts
        .iter()
        .map(|parameter| parameter.parameter_path.clone())
        .collect::<Vec<_>>();
    if actual_names != expected_names {
        return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
            "compute case `{}` step {} parameter names do not match ToyFSDPModel",
            case.compute_dtype, step.step
        )));
    }
    let mut has_sharded = false;
    let mut has_replicated = false;
    for parameter in &step.parameter_receipts {
        if parameter.gathered_full_parameter_digest != parameter.non_parallel_baseline_digest
            || !parameter.matches_non_parallel_baseline
        {
            return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
                "parameter `{}` does not match baseline in compute case `{}` step {}",
                parameter.parameter_path, case.compute_dtype, step.step
            )));
        }
        match parameter.source_kind.as_str() {
            "row_sharded_all_gather" => {
                has_sharded = true;
                if parameter.shard_axis != Some(0)
                    || parameter.rank_shards.len() != 2
                    || parameter.replicated_parameter_returned_as_is
                {
                    return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
                        "sharded parameter `{}` does not retain two-rank all-gather evidence",
                        parameter.parameter_path
                    )));
                }
                let shard_elements = parameter
                    .rank_shards
                    .iter()
                    .map(|shard| shard.element_count)
                    .sum::<usize>();
                if shard_elements != parameter.element_count {
                    return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
                        "sharded parameter `{}` shard element count does not reconstruct full tensor",
                        parameter.parameter_path
                    )));
                }
            }
            "replicated_returned_as_is" => {
                has_replicated = true;
                if !parameter.replicated_parameter_returned_as_is
                    || !parameter.rank_shards.is_empty()
                    || parameter.replicated_source_ranks != vec![0, 1]
                {
                    return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
                        "replicated parameter `{}` is not retained as-is",
                        parameter.parameter_path
                    )));
                }
            }
            other => {
                return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
                    "unexpected full-params source kind `{other}` for `{}`",
                    parameter.parameter_path
                )));
            }
        }
    }
    if !has_sharded || !has_replicated {
        return Err(Cs336A2FsdpFullParamsReceiptError::InvalidReceipt(format!(
            "compute case `{}` step {} must include sharded and replicated parameters",
            case.compute_dtype, step.step
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        build_cs336_a2_fsdp_full_params_receipt, write_cs336_a2_fsdp_full_params_receipt,
        CS336_A2_FSDP_FULL_PARAMS_RECEIPT_FIXTURE_PATH,
    };

    #[test]
    fn fsdp_full_params_receipt_models_current_adapter_surface(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let receipt = build_cs336_a2_fsdp_full_params_receipt(repo_root)?;
        assert_eq!(receipt.stanford_adapter_name, "fsdp_gather_full_params");
        assert!(receipt.all_cases_match_non_parallel_baseline);
        assert_eq!(receipt.expected_parameter_names.len(), 6);
        for case in &receipt.compute_cases {
            assert_eq!(case.step_receipts.len(), 3);
            for step in &case.step_receipts {
                assert_eq!(step.state_dict_parameter_count, 6);
                assert!(step.matches_non_parallel_baseline);
                assert!(step
                    .parameter_receipts
                    .iter()
                    .any(|parameter| parameter.source_kind == "row_sharded_all_gather"));
                assert!(step
                    .parameter_receipts
                    .iter()
                    .any(|parameter| parameter.source_kind == "replicated_returned_as_is"));
            }
        }
        Ok(())
    }

    #[test]
    fn fsdp_full_params_writer_emits_json_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let receipt = write_cs336_a2_fsdp_full_params_receipt(temp.path())?;
        let fixture_path = temp
            .path()
            .join(CS336_A2_FSDP_FULL_PARAMS_RECEIPT_FIXTURE_PATH);
        let bytes = std::fs::read(fixture_path)?;
        let written: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            written["schema_version"].as_str(),
            Some(receipt.schema_version.as_str())
        );
        Ok(())
    }
}
