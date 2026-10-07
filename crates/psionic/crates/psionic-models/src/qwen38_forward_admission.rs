use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    QWEN38_27B_MODEL_ID, QWEN38_27B_SERVED_MODEL_ID, QWEN38_27B_UPSTREAM_REVISION,
    QWEN38_PRODUCT_FAMILY, Qwen35TextArchitectureReport, Qwen35TextCheckpointError,
    Qwen35TextTensorAdmissionReport, canonical_qwen38_27b_artifact_facts,
    qwen35_text_architecture_report, qwen35_text_expected_tensor_specs,
    qwen35_text_observed_tensors_from_shards, qwen35_text_shard_paths_from_weight_map,
    qwen35_text_tensor_admission_report, qwen35_text_weight_index_from_bytes,
};

pub const QWEN38_FORWARD_ADMISSION_SCHEMA_VERSION: &str = "psionic.qwen38_27b_forward_admission.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38ForwardAdmissionStatus {
    Admitted,
    Refused,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38SplitLayerShardResolution {
    pub layer_index: usize,
    pub shard_names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ForwardAdmissionReport {
    pub schema_version: String,
    pub product_family: String,
    pub model_id: String,
    pub served_model_id: String,
    pub upstream_revision: String,
    pub model_dir: String,
    pub artifact_facts_sha256: String,
    pub config_path: String,
    pub config_sha256: String,
    pub index_path: String,
    pub index_sha256: String,
    pub indexed_tensor_data_bytes: u64,
    pub indexed_tensor_count: usize,
    pub indexed_shard_count: usize,
    pub split_layer_shard_resolutions: Vec<Qwen38SplitLayerShardResolution>,
    pub architecture: Qwen35TextArchitectureReport,
    pub tensor_admission: Qwen35TextTensorAdmissionReport,
    pub tensor_admission_sha256: String,
    pub status: Qwen38ForwardAdmissionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
    pub claim_boundary: String,
}

impl Qwen38ForwardAdmissionReport {
    pub fn is_admitted(&self) -> bool {
        self.status == Qwen38ForwardAdmissionStatus::Admitted
    }
}

#[derive(Debug, Error)]
pub enum Qwen38ForwardAdmissionError {
    #[error("Qwen3.8 checkpoint I/O failed at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Qwen3.8 artifact digest drift for `{artifact}`: expected {expected}, found {actual}")]
    ArtifactDigestDrift {
        artifact: String,
        expected: String,
        actual: String,
    },
    #[error("Qwen3.8 architecture drift at `{field}`: expected {expected}, found {actual}")]
    ArchitectureDrift {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("Qwen3.8 index drift at `{field}`: expected {expected}, found {actual}")]
    IndexDrift {
        field: String,
        expected: String,
        actual: String,
    },
    #[error(transparent)]
    Checkpoint(#[from] Qwen35TextCheckpointError),
    #[error("failed to serialize Qwen3.8 forward admission evidence: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl Qwen38ForwardAdmissionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "qwen38_checkpoint_io_failed",
            Self::ArtifactDigestDrift { .. } => "qwen38_artifact_digest_drift",
            Self::ArchitectureDrift { .. } => "qwen38_architecture_drift",
            Self::IndexDrift { .. } => "qwen38_index_drift",
            Self::Checkpoint(error) => error.code(),
            Self::Serialization(_) => "qwen38_admission_serialization_failed",
        }
    }
}

pub fn run_qwen38_forward_admission(
    model_dir: impl AsRef<Path>,
) -> Result<Qwen38ForwardAdmissionReport, Qwen38ForwardAdmissionError> {
    let model_dir = model_dir.as_ref();
    let facts = canonical_qwen38_27b_artifact_facts();
    let config_path = model_dir.join("config.json");
    let index_path = model_dir.join("model.safetensors.index.json");
    let config_bytes = read_bytes(&config_path)?;
    let index_bytes = read_bytes(&index_path)?;
    let config_sha256 = sha256_hex(config_bytes.as_slice());
    let index_sha256 = sha256_hex(index_bytes.as_slice());
    validate_digest(
        "config.json",
        facts.digests.config_sha256.as_str(),
        config_sha256.as_str(),
    )?;
    validate_digest(
        "model.safetensors.index.json",
        facts.digests.safetensors_index_sha256.as_str(),
        index_sha256.as_str(),
    )?;

    let architecture = qwen35_text_architecture_report(config_bytes.as_slice())?;
    validate_qwen38_architecture(&architecture)?;
    let index = qwen35_text_weight_index_from_bytes(index_bytes.as_slice())?;
    validate_index_fact(
        "metadata.total_size",
        facts.indexed_tensor_data_bytes,
        index.total_tensor_bytes.unwrap_or_default(),
    )?;
    validate_index_fact(
        "weight_map tensor count",
        facts.indexed_tensor_count,
        index.tensor_count,
    )?;
    validate_index_fact("shard count", facts.shards.len(), index.shard_names.len())?;
    let expected_shards = facts
        .shards
        .iter()
        .map(|shard| shard.filename.clone())
        .collect::<Vec<_>>();
    if index.shard_names != expected_shards {
        return Err(Qwen38ForwardAdmissionError::IndexDrift {
            field: String::from("shard_names"),
            expected: format!("{expected_shards:?}"),
            actual: format!("{:?}", index.shard_names),
        });
    }

    let shard_paths = qwen35_text_shard_paths_from_weight_map(model_dir, &index.weight_map)?;
    let split_layer_shard_resolutions =
        split_layer_shard_resolutions(&index.weight_map, architecture.num_hidden_layers);
    let observed = qwen35_text_observed_tensors_from_shards(&shard_paths)?;
    for (expected, observed) in facts.shards.iter().zip(&observed.shards) {
        if expected.filename != observed.shard_name {
            return Err(Qwen38ForwardAdmissionError::IndexDrift {
                field: String::from("observed_shard_name"),
                expected: expected.filename.clone(),
                actual: observed.shard_name.clone(),
            });
        }
        if expected.file_bytes != observed.byte_len {
            return Err(Qwen38ForwardAdmissionError::IndexDrift {
                field: format!("{}.file_bytes", expected.filename),
                expected: expected.file_bytes.to_string(),
                actual: observed.byte_len.to_string(),
            });
        }
    }
    let expected_tensors = qwen35_text_expected_tensor_specs(&architecture)?;
    let tensor_admission =
        qwen35_text_tensor_admission_report(expected_tensors, index.weight_map, observed);
    let tensor_admission_sha256 = qwen38_tensor_admission_sha256(&tensor_admission)?;
    let (status, refusal_code, refusal_detail) = if tensor_admission.text_tensor_admission_passed {
        (Qwen38ForwardAdmissionStatus::Admitted, None, None)
    } else {
        (
            Qwen38ForwardAdmissionStatus::Refused,
            Some(String::from("qwen38_text_tensor_admission_failed")),
            Some(format!(
                "Qwen3.8 text tensor admission refused: {} missing, {} shape drift, {} dtype drift, {} shard drift, {} index-only, and {} header-only tensors",
                tensor_admission.missing_expected_tensors.len(),
                tensor_admission.shape_mismatches.len(),
                tensor_admission.dtype_mismatches.len(),
                tensor_admission.shard_mismatches.len(),
                tensor_admission.index_tensors_missing_from_headers.len(),
                tensor_admission.header_tensors_missing_from_index.len(),
            )),
        )
    };

    Ok(Qwen38ForwardAdmissionReport {
        schema_version: String::from(QWEN38_FORWARD_ADMISSION_SCHEMA_VERSION),
        product_family: String::from(QWEN38_PRODUCT_FAMILY),
        model_id: String::from(QWEN38_27B_MODEL_ID),
        served_model_id: String::from(QWEN38_27B_SERVED_MODEL_ID),
        upstream_revision: String::from(QWEN38_27B_UPSTREAM_REVISION),
        model_dir: model_dir.display().to_string(),
        artifact_facts_sha256: facts.canonical_sha256(),
        config_path: config_path.display().to_string(),
        config_sha256,
        index_path: index_path.display().to_string(),
        index_sha256,
        indexed_tensor_data_bytes: facts.indexed_tensor_data_bytes,
        indexed_tensor_count: facts.indexed_tensor_count,
        indexed_shard_count: facts.shards.len(),
        split_layer_shard_resolutions,
        architecture,
        tensor_admission,
        tensor_admission_sha256,
        status,
        refusal_code,
        refusal_detail,
        claim_boundary: String::from(
            "This report admits the pinned Qwen/Qwen3.8-27B qwen3_5_text architecture and validates the complete indexed safetensors tensor table from all shard headers. It inventories decoder, MTP, and visual or other tensors separately. It does not read tensor data, execute a model layer, produce logits, generate tokens, serve requests, or admit media execution.",
        ),
    })
}

fn split_layer_shard_resolutions(
    weight_map: &std::collections::BTreeMap<String, String>,
    layer_count: usize,
) -> Vec<Qwen38SplitLayerShardResolution> {
    (0..layer_count)
        .filter_map(|layer_index| {
            let prefix = format!("model.language_model.layers.{layer_index}.");
            let shard_names = weight_map
                .iter()
                .filter_map(|(name, shard)| name.starts_with(&prefix).then_some(shard.clone()))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            (shard_names.len() > 1).then_some(Qwen38SplitLayerShardResolution {
                layer_index,
                shard_names,
            })
        })
        .collect()
}

fn qwen38_tensor_admission_sha256(
    report: &Qwen35TextTensorAdmissionReport,
) -> Result<String, serde_json::Error> {
    let mut normalized = report.clone();
    for shard in &mut normalized.shard_headers {
        shard.path.clear();
    }
    sha256_json(&normalized)
}

fn validate_qwen38_architecture(
    architecture: &Qwen35TextArchitectureReport,
) -> Result<(), Qwen38ForwardAdmissionError> {
    let facts = canonical_qwen38_27b_artifact_facts();
    validate_architecture_fact(
        "root_model_type",
        facts.architecture.root_model_type.as_str(),
        architecture.root_model_type.as_str(),
    )?;
    validate_architecture_fact(
        "text_model_type",
        facts.architecture.decoder_architecture.as_str(),
        architecture.text_model_type.as_str(),
    )?;
    validate_architecture_fact(
        "hidden_size",
        facts.architecture.hidden_size,
        architecture.hidden_size,
    )?;
    validate_architecture_fact(
        "intermediate_size",
        facts.architecture.intermediate_size,
        architecture.intermediate_size,
    )?;
    validate_architecture_fact(
        "num_hidden_layers",
        facts.architecture.layer_count,
        architecture.num_hidden_layers,
    )?;
    validate_architecture_fact(
        "vocab_size",
        facts.architecture.vocabulary_size,
        architecture.vocab_size,
    )?;
    validate_architecture_fact(
        "num_attention_heads",
        facts.architecture.full_attention_head_count,
        architecture.num_attention_heads,
    )?;
    validate_architecture_fact(
        "num_key_value_heads",
        facts.architecture.full_attention_kv_head_count,
        architecture.num_key_value_heads,
    )?;
    validate_architecture_fact(
        "head_dim",
        facts.architecture.full_attention_head_size,
        architecture.head_dim,
    )?;
    validate_architecture_fact(
        "linear_num_key_heads",
        Some(facts.architecture.linear_attention_qk_head_count),
        architecture.linear_num_key_heads,
    )?;
    validate_architecture_fact(
        "linear_num_value_heads",
        Some(facts.architecture.linear_attention_value_head_count),
        architecture.linear_num_value_heads,
    )?;
    validate_architecture_fact(
        "linear_key_head_dim",
        Some(facts.architecture.linear_attention_head_size),
        architecture.linear_key_head_dim,
    )?;
    validate_architecture_fact(
        "linear_value_head_dim",
        Some(facts.architecture.linear_attention_head_size),
        architecture.linear_value_head_dim,
    )?;
    validate_architecture_fact(
        "linear_conv_kernel_dim",
        Some(facts.architecture.linear_convolution_width),
        architecture.linear_conv_kernel_dim,
    )?;
    validate_architecture_fact(
        "mtp_num_hidden_layers",
        facts.architecture.mtp_layer_count,
        architecture.mtp_num_hidden_layers,
    )?;
    validate_architecture_fact(
        "max_position_embeddings",
        facts.context.native_context_tokens,
        architecture.max_position_embeddings,
    )?;
    validate_architecture_fact("torch_dtype", "bfloat16", architecture.torch_dtype.as_str())?;
    let expected_full_attention_layers = (facts.architecture.full_attention_interval - 1
        ..facts.architecture.layer_count)
        .step_by(facts.architecture.full_attention_interval)
        .collect::<Vec<_>>();
    validate_architecture_fact(
        "full_attention_layers",
        expected_full_attention_layers,
        architecture.full_attention_layers.clone(),
    )?;
    Ok(())
}

fn validate_digest(
    artifact: &str,
    expected: &str,
    actual: &str,
) -> Result<(), Qwen38ForwardAdmissionError> {
    if expected == actual {
        return Ok(());
    }
    Err(Qwen38ForwardAdmissionError::ArtifactDigestDrift {
        artifact: String::from(artifact),
        expected: String::from(expected),
        actual: String::from(actual),
    })
}

fn validate_architecture_fact<T>(
    field: &str,
    expected: T,
    actual: T,
) -> Result<(), Qwen38ForwardAdmissionError>
where
    T: PartialEq + std::fmt::Debug,
{
    if expected == actual {
        return Ok(());
    }
    Err(Qwen38ForwardAdmissionError::ArchitectureDrift {
        field: String::from(field),
        expected: format!("{expected:?}"),
        actual: format!("{actual:?}"),
    })
}

fn validate_index_fact<T>(
    field: &str,
    expected: T,
    actual: T,
) -> Result<(), Qwen38ForwardAdmissionError>
where
    T: PartialEq + ToString,
{
    if expected == actual {
        return Ok(());
    }
    Err(Qwen38ForwardAdmissionError::IndexDrift {
        field: String::from(field),
        expected: expected.to_string(),
        actual: actual.to_string(),
    })
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, Qwen38ForwardAdmissionError> {
    fs::read(path).map_err(|source| Qwen38ForwardAdmissionError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn sha256_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    Ok(sha256_hex(serde_json::to_vec(value)?.as_slice()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwen38_forward_official_checkpoint_admits_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let model_dir = std::env::var("PSIONIC_QWEN38_MODEL_DIR")
            .unwrap_or_else(|_| String::from("target/models/qwen/Qwen3.8-27B"));
        if !Path::new(model_dir.as_str()).join("config.json").is_file() {
            return Ok(());
        }

        let report = run_qwen38_forward_admission(model_dir)?;
        assert!(report.is_admitted());
        assert_eq!(
            report.schema_version,
            QWEN38_FORWARD_ADMISSION_SCHEMA_VERSION
        );
        assert_eq!(report.model_id, QWEN38_27B_MODEL_ID);
        assert_eq!(report.served_model_id, QWEN38_27B_SERVED_MODEL_ID);
        assert_eq!(report.indexed_tensor_count, 1_199);
        assert_eq!(report.indexed_shard_count, 18);
        assert_eq!(report.tensor_admission.expected_text_tensor_count, 866);
        assert_eq!(report.tensor_admission.expected_decoder_tensor_count, 851);
        assert_eq!(report.tensor_admission.expected_mtp_tensor_count, 15);
        assert_eq!(report.tensor_admission.admitted_decoder_tensor_count, 851);
        assert_eq!(report.tensor_admission.admitted_mtp_tensor_count, 15);
        assert_eq!(
            report
                .tensor_admission
                .visual_or_other_observed_tensor_count,
            333
        );
        assert_eq!(report.tensor_admission.expected_shard_count, 18);
        assert_eq!(report.tensor_admission.observed_shard_count, 18);
        assert_eq!(
            report
                .split_layer_shard_resolutions
                .iter()
                .map(|resolution| resolution.layer_index)
                .collect::<Vec<_>>(),
            [4, 15, 21, 29, 37, 45, 53, 61]
        );
        assert_eq!(
            report.split_layer_shard_resolutions[0].shard_names,
            [
                "model-00001-of-00018.safetensors",
                "model-00002-of-00018.safetensors"
            ]
        );
        assert!(
            report
                .tensor_admission
                .visual_or_other_observed_tensors
                .iter()
                .all(|name| !name.starts_with("mtp."))
        );
        assert!(
            report
                .tensor_admission
                .shard_headers
                .iter()
                .any(|shard| shard.tensor_count > 0)
        );
        assert_eq!(
            serde_json::from_slice::<Qwen38ForwardAdmissionReport>(&serde_json::to_vec(&report)?)?,
            report
        );
        Ok(())
    }

    #[test]
    fn qwen38_forward_digest_and_architecture_drift_are_typed() {
        let error = validate_digest("config.json", "expected", "actual").expect_err("digest drift");
        assert_eq!(error.code(), "qwen38_artifact_digest_drift");

        let config = br#"{
            "architectures":["Qwen3_5ForConditionalGeneration"],
            "model_type":"qwen3_5",
            "text_config":{
                "model_type":"qwen3_5_text",
                "hidden_size":4,
                "intermediate_size":8,
                "num_hidden_layers":1,
                "num_attention_heads":1,
                "num_key_value_heads":1,
                "head_dim":4,
                "vocab_size":8,
                "max_position_embeddings":16,
                "dtype":"bfloat16",
                "layer_types":["full_attention"]
            }
        }"#;
        let architecture = qwen35_text_architecture_report(config).expect("shared architecture");
        let error = validate_qwen38_architecture(&architecture).expect_err("Qwen3.8 drift");
        assert!(matches!(
            error,
            Qwen38ForwardAdmissionError::ArchitectureDrift { ref field, .. }
                if field == "hidden_size"
        ));
    }
}
