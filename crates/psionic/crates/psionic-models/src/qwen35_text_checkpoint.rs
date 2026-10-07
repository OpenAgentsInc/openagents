use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, MapAccess, Visitor},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use half::{bf16, f16};

pub const QWEN35_ROOT_MODEL_TYPE: &str = "qwen3_5";
pub const QWEN35_TEXT_MODEL_TYPE: &str = "qwen3_5_text";
pub const QWEN35_WRAPPER_ARCHITECTURE: &str = "Qwen3_5ForConditionalGeneration";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen35TextArchitectureReport {
    pub root_model_type: String,
    pub text_model_type: String,
    pub architectures: Vec<String>,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    pub head_dim: usize,
    pub vocab_size: usize,
    pub max_position_embeddings: usize,
    pub torch_dtype: String,
    pub rms_norm_eps: Option<f64>,
    pub hidden_act: Option<String>,
    pub layer_types: Vec<String>,
    pub full_attention_layers: Vec<usize>,
    pub linear_attention_layers: Vec<usize>,
    pub linear_key_head_dim: Option<usize>,
    pub linear_value_head_dim: Option<usize>,
    pub linear_num_key_heads: Option<usize>,
    pub linear_num_value_heads: Option<usize>,
    pub linear_conv_kernel_dim: Option<usize>,
    pub attn_output_gate: Option<bool>,
    pub output_gate_type: Option<String>,
    pub mtp_num_hidden_layers: usize,
    pub mtp_use_dedicated_embeddings: Option<bool>,
    pub rope_parameters_hash: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen35TextTensorSpec {
    pub name: String,
    pub dtype: String,
    pub shape: Vec<usize>,
}

impl Qwen35TextTensorSpec {
    pub fn is_mtp(&self) -> bool {
        is_mtp_tensor_name(self.name.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen35TextObservedTensorSpec {
    pub name: String,
    pub dtype: String,
    pub shape: Vec<usize>,
    pub shard: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen35TextSafetensorsShardHeaderReport {
    pub path: String,
    pub shard_name: String,
    pub byte_len: u64,
    pub header_sha256: String,
    pub tensor_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen35TextTensorShapeMismatch {
    pub name: String,
    pub expected_shape: Vec<usize>,
    pub observed_shape: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen35TextTensorDtypeMismatch {
    pub name: String,
    pub expected_dtype: String,
    pub observed_dtype: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen35TextTensorShardMismatch {
    pub name: String,
    pub indexed_shard: String,
    pub observed_shard: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen35TextTensorAdmissionReport {
    pub expected_text_tensor_count: usize,
    pub expected_decoder_tensor_count: usize,
    pub expected_mtp_tensor_count: usize,
    pub observed_index_tensor_count: usize,
    pub observed_header_tensor_count: usize,
    pub admitted_text_tensor_count: usize,
    pub admitted_decoder_tensor_count: usize,
    pub admitted_mtp_tensor_count: usize,
    pub visual_or_other_observed_tensor_count: usize,
    pub expected_shard_count: usize,
    pub observed_shard_count: usize,
    pub shard_headers: Vec<Qwen35TextSafetensorsShardHeaderReport>,
    pub missing_expected_tensors: Vec<Qwen35TextTensorSpec>,
    pub index_tensors_missing_from_headers: Vec<String>,
    pub header_tensors_missing_from_index: Vec<String>,
    pub visual_or_other_observed_tensors: Vec<String>,
    pub shape_mismatches: Vec<Qwen35TextTensorShapeMismatch>,
    pub dtype_mismatches: Vec<Qwen35TextTensorDtypeMismatch>,
    pub shard_mismatches: Vec<Qwen35TextTensorShardMismatch>,
    pub text_tensor_admission_passed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Qwen35TextObservedTensorSet {
    pub shards: Vec<Qwen35TextSafetensorsShardHeaderReport>,
    pub tensors: Vec<Qwen35TextObservedTensorSpec>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen35TextWeightIndex {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_tensor_bytes: Option<u64>,
    pub tensor_count: usize,
    pub shard_names: Vec<String>,
    pub weight_map: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen35TextTensorRowReadReceipt {
    pub tensor_name: String,
    pub shard_name: String,
    pub row_index: usize,
    pub dtype: String,
    pub shape: Vec<usize>,
    pub row_sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Qwen35TextTensorRow {
    pub values: Vec<f32>,
    pub receipt: Qwen35TextTensorRowReadReceipt,
}

#[derive(Debug, Error)]
pub enum Qwen35TextCheckpointError {
    #[error("qwen3_5_text checkpoint I/O failed at `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("qwen3_5_text checkpoint JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported qwen3_5_text architecture: {0}")]
    UnsupportedArchitecture(String),
    #[error("invalid qwen3_5_text checkpoint: {0}")]
    InvalidConfig(String),
    #[error("duplicate tensor mapping `{tensor_name}` in model index")]
    DuplicateTensorMapping { tensor_name: String },
    #[error(
        "tensor `{tensor_name}` is present in both `{first_shard}` and `{second_shard}` safetensors headers"
    )]
    DuplicateObservedTensor {
        tensor_name: String,
        first_shard: String,
        second_shard: String,
    },
    #[error("model directory is incomplete; missing safetensors shards: {shards:?}")]
    MissingShards { shards: Vec<String> },
    #[error("invalid safetensors shard `{path}`: {detail}")]
    InvalidShard { path: PathBuf, detail: String },
}

impl Qwen35TextCheckpointError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "checkpoint_io_failed",
            Self::Json(_) => "checkpoint_json_invalid",
            Self::UnsupportedArchitecture(_) => "unsupported_architecture",
            Self::InvalidConfig(_) => "checkpoint_config_invalid",
            Self::DuplicateTensorMapping { .. } => "duplicate_tensor_mapping",
            Self::DuplicateObservedTensor { .. } => "duplicate_observed_tensor",
            Self::MissingShards { .. } => "missing_shards",
            Self::InvalidShard { .. } => "invalid_shard",
        }
    }
}

pub fn qwen35_text_architecture_report(
    config_bytes: &[u8],
) -> Result<Qwen35TextArchitectureReport, Qwen35TextCheckpointError> {
    let root = serde_json::from_slice::<Value>(config_bytes)?;
    let text = root
        .get("text_config")
        .ok_or_else(|| {
            Qwen35TextCheckpointError::InvalidConfig(String::from(
                "Hugging Face config must contain text_config",
            ))
        })?
        .clone();
    let root_model_type = required_string(&root, "model_type")?;
    let text_model_type = required_string(&text, "model_type")?;
    if root_model_type != QWEN35_ROOT_MODEL_TYPE || text_model_type != QWEN35_TEXT_MODEL_TYPE {
        return Err(Qwen35TextCheckpointError::UnsupportedArchitecture(format!(
            "model_type `{root_model_type}` / `{text_model_type}`; expected `{QWEN35_ROOT_MODEL_TYPE}` / `{QWEN35_TEXT_MODEL_TYPE}`"
        )));
    }
    let architectures = root
        .get("architectures")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if architectures.as_slice() != [QWEN35_WRAPPER_ARCHITECTURE] {
        return Err(Qwen35TextCheckpointError::UnsupportedArchitecture(format!(
            "architectures must be exactly [`{QWEN35_WRAPPER_ARCHITECTURE}`], found {architectures:?}"
        )));
    }
    let layer_types = text
        .get("layer_types")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let num_hidden_layers = required_usize(&text, "num_hidden_layers")?;
    if layer_types.len() != num_hidden_layers {
        return Err(Qwen35TextCheckpointError::InvalidConfig(format!(
            "text_config.layer_types must contain {num_hidden_layers} entries"
        )));
    }
    let full_attention_layers = layer_types
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (value == "full_attention").then_some(index))
        .collect::<Vec<_>>();
    let linear_attention_layers = layer_types
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (value == "linear_attention").then_some(index))
        .collect::<Vec<_>>();
    if full_attention_layers.len() + linear_attention_layers.len() != layer_types.len() {
        return Err(Qwen35TextCheckpointError::InvalidConfig(String::from(
            "text_config.layer_types may only contain full_attention or linear_attention",
        )));
    }
    let hidden_size = required_usize(&text, "hidden_size")?;
    let num_attention_heads = required_usize(&text, "num_attention_heads")?;
    let head_dim = optional_usize(&text, "head_dim")
        .unwrap_or_else(|| hidden_size.checked_div(num_attention_heads).unwrap_or(0));
    if head_dim == 0 {
        return Err(Qwen35TextCheckpointError::InvalidConfig(String::from(
            "head_dim must be non-zero",
        )));
    }
    let dtype = text
        .get("torch_dtype")
        .or_else(|| text.get("dtype"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let rope_parameters_hash = text.get("rope_parameters").map(sha256_json).transpose()?;

    Ok(Qwen35TextArchitectureReport {
        root_model_type,
        text_model_type,
        architectures,
        hidden_size,
        intermediate_size: required_usize(&text, "intermediate_size")?,
        num_hidden_layers,
        num_attention_heads,
        num_key_value_heads: required_usize(&text, "num_key_value_heads")?,
        head_dim,
        vocab_size: required_usize(&text, "vocab_size")?,
        max_position_embeddings: required_usize(&text, "max_position_embeddings")?,
        torch_dtype: dtype,
        rms_norm_eps: text.get("rms_norm_eps").and_then(Value::as_f64),
        hidden_act: optional_string(&text, "hidden_act"),
        layer_types,
        full_attention_layers,
        linear_attention_layers,
        linear_key_head_dim: optional_usize(&text, "linear_key_head_dim"),
        linear_value_head_dim: optional_usize(&text, "linear_value_head_dim"),
        linear_num_key_heads: optional_usize(&text, "linear_num_key_heads"),
        linear_num_value_heads: optional_usize(&text, "linear_num_value_heads"),
        linear_conv_kernel_dim: optional_usize(&text, "linear_conv_kernel_dim"),
        attn_output_gate: text.get("attn_output_gate").and_then(Value::as_bool),
        output_gate_type: optional_string(&text, "output_gate_type"),
        mtp_num_hidden_layers: optional_usize(&text, "mtp_num_hidden_layers").unwrap_or(0),
        mtp_use_dedicated_embeddings: text
            .get("mtp_use_dedicated_embeddings")
            .and_then(Value::as_bool),
        rope_parameters_hash,
    })
}

pub fn qwen35_text_expected_tensor_specs(
    architecture: &Qwen35TextArchitectureReport,
) -> Result<Vec<Qwen35TextTensorSpec>, Qwen35TextCheckpointError> {
    let dtype = hf_dtype_to_safetensors_dtype(architecture.torch_dtype.as_str());
    let mut specs = vec![
        tensor(
            "model.language_model.embed_tokens.weight",
            &dtype,
            [architecture.vocab_size, architecture.hidden_size],
        ),
        tensor(
            "model.language_model.norm.weight",
            &dtype,
            [architecture.hidden_size],
        ),
        tensor(
            "lm_head.weight",
            &dtype,
            [architecture.vocab_size, architecture.hidden_size],
        ),
    ];

    for (layer, layer_type) in architecture.layer_types.iter().enumerate() {
        let prefix = format!("model.language_model.layers.{layer}");
        push_common_decoder_layer_specs(&mut specs, &prefix, &dtype, architecture);
        match layer_type.as_str() {
            "full_attention" => {
                push_full_attention_specs(&mut specs, &prefix, &dtype, architecture)
            }
            "linear_attention" => {
                push_linear_attention_specs(&mut specs, &prefix, &dtype, architecture)?
            }
            other => {
                return Err(Qwen35TextCheckpointError::InvalidConfig(format!(
                    "unsupported layer type `{other}`"
                )));
            }
        }
    }

    if architecture.mtp_num_hidden_layers > 0 {
        specs.extend([
            tensor(
                "mtp.pre_fc_norm_embedding.weight",
                &dtype,
                [architecture.hidden_size],
            ),
            tensor(
                "mtp.pre_fc_norm_hidden.weight",
                &dtype,
                [architecture.hidden_size],
            ),
            tensor(
                "mtp.fc.weight",
                &dtype,
                [architecture.hidden_size, architecture.hidden_size * 2],
            ),
            tensor("mtp.norm.weight", &dtype, [architecture.hidden_size]),
        ]);
        for layer in 0..architecture.mtp_num_hidden_layers {
            let prefix = format!("mtp.layers.{layer}");
            push_common_decoder_layer_specs(&mut specs, &prefix, &dtype, architecture);
            push_full_attention_specs(&mut specs, &prefix, &dtype, architecture);
        }
    }

    specs.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(specs)
}

pub fn qwen35_text_weight_index_from_bytes(
    index_bytes: &[u8],
) -> Result<Qwen35TextWeightIndex, Qwen35TextCheckpointError> {
    let document = match serde_json::from_slice::<WeightIndexDocument>(index_bytes) {
        Ok(document) => document,
        Err(error) => {
            let message = error.to_string();
            if let Some(tensor_name) = message
                .strip_prefix("duplicate tensor mapping `")
                .and_then(|message| message.split('`').next())
            {
                return Err(Qwen35TextCheckpointError::DuplicateTensorMapping {
                    tensor_name: String::from(tensor_name),
                });
            }
            return Err(Qwen35TextCheckpointError::Json(error));
        }
    };
    if document.weight_map.is_empty() {
        return Err(Qwen35TextCheckpointError::InvalidConfig(String::from(
            "model.safetensors.index.json has no tensor entries",
        )));
    }
    let total_tensor_bytes = document
        .metadata
        .and_then(|metadata| metadata.total_size)
        .map(|total_size| {
            if !total_size.is_finite()
                || total_size < 0.0
                || total_size.fract() != 0.0
                || total_size > u64::MAX as f64
            {
                return Err(Qwen35TextCheckpointError::InvalidConfig(format!(
                    "index metadata.total_size must be a non-negative integer, found {total_size}"
                )));
            }
            Ok(total_size as u64)
        })
        .transpose()?;
    let shard_names = document
        .weight_map
        .values()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(Qwen35TextWeightIndex {
        total_tensor_bytes,
        tensor_count: document.weight_map.len(),
        shard_names,
        weight_map: document.weight_map,
    })
}

pub fn qwen35_text_shard_paths_from_weight_map(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
) -> Result<Vec<PathBuf>, Qwen35TextCheckpointError> {
    let shard_names = weight_map.values().cloned().collect::<BTreeSet<_>>();
    let mut paths = Vec::with_capacity(shard_names.len());
    let mut missing = Vec::new();
    for shard_name in shard_names {
        let path = model_dir.join(&shard_name);
        if path.is_file() {
            paths.push(path);
        } else {
            missing.push(shard_name);
        }
    }
    if !missing.is_empty() {
        return Err(Qwen35TextCheckpointError::MissingShards { shards: missing });
    }
    Ok(paths)
}

pub fn qwen35_text_observed_tensors_from_shards(
    shard_paths: &[PathBuf],
) -> Result<Qwen35TextObservedTensorSet, Qwen35TextCheckpointError> {
    let mut shards = Vec::new();
    let mut tensors = Vec::new();
    let mut tensor_shards = BTreeMap::<String, String>::new();
    for path in shard_paths {
        let (shard, mut shard_tensors) = safetensors_header(path)?;
        for tensor in &shard_tensors {
            if let Some(first_shard) =
                tensor_shards.insert(tensor.name.clone(), tensor.shard.clone())
            {
                return Err(Qwen35TextCheckpointError::DuplicateObservedTensor {
                    tensor_name: tensor.name.clone(),
                    first_shard,
                    second_shard: tensor.shard.clone(),
                });
            }
        }
        shards.push(shard);
        tensors.append(&mut shard_tensors);
    }
    tensors.sort_by(|left, right| left.name.cmp(&right.name));
    shards.sort_by(|left, right| left.shard_name.cmp(&right.shard_name));
    Ok(Qwen35TextObservedTensorSet { shards, tensors })
}

pub fn qwen35_text_tensor_admission_report(
    expected_tensors: Vec<Qwen35TextTensorSpec>,
    weight_map: BTreeMap<String, String>,
    observed_tensors: Qwen35TextObservedTensorSet,
) -> Qwen35TextTensorAdmissionReport {
    let expected_by_name = expected_tensors
        .iter()
        .map(|spec| (spec.name.clone(), spec.clone()))
        .collect::<BTreeMap<_, _>>();
    let observed_by_name = observed_tensors
        .tensors
        .iter()
        .map(|spec| (spec.name.clone(), spec.clone()))
        .collect::<BTreeMap<_, _>>();
    let index_names = weight_map.keys().cloned().collect::<BTreeSet<_>>();
    let header_names = observed_by_name.keys().cloned().collect::<BTreeSet<_>>();
    let expected_names = expected_by_name.keys().cloned().collect::<BTreeSet<_>>();

    let expected_mtp_tensor_count = expected_tensors.iter().filter(|spec| spec.is_mtp()).count();
    let expected_decoder_tensor_count = expected_tensors.len() - expected_mtp_tensor_count;
    let mut missing_expected_tensors = Vec::new();
    let mut shape_mismatches = Vec::new();
    let mut dtype_mismatches = Vec::new();
    let mut shard_mismatches = Vec::new();
    let mut admitted_text_tensor_count = 0usize;
    let mut admitted_decoder_tensor_count = 0usize;
    let mut admitted_mtp_tensor_count = 0usize;
    for expected in &expected_tensors {
        let Some(indexed_shard) = weight_map.get(&expected.name) else {
            missing_expected_tensors.push(expected.clone());
            continue;
        };
        let Some(observed) = observed_by_name.get(&expected.name) else {
            missing_expected_tensors.push(expected.clone());
            continue;
        };
        let mut admitted = true;
        if observed.shape != expected.shape {
            shape_mismatches.push(Qwen35TextTensorShapeMismatch {
                name: expected.name.clone(),
                expected_shape: expected.shape.clone(),
                observed_shape: observed.shape.clone(),
            });
            admitted = false;
        }
        if observed.dtype != expected.dtype {
            dtype_mismatches.push(Qwen35TextTensorDtypeMismatch {
                name: expected.name.clone(),
                expected_dtype: expected.dtype.clone(),
                observed_dtype: observed.dtype.clone(),
            });
            admitted = false;
        }
        if indexed_shard != &observed.shard {
            shard_mismatches.push(Qwen35TextTensorShardMismatch {
                name: expected.name.clone(),
                indexed_shard: indexed_shard.clone(),
                observed_shard: observed.shard.clone(),
            });
            admitted = false;
        }
        if admitted {
            admitted_text_tensor_count += 1;
            if expected.is_mtp() {
                admitted_mtp_tensor_count += 1;
            } else {
                admitted_decoder_tensor_count += 1;
            }
        }
    }

    let index_tensors_missing_from_headers = index_names
        .difference(&header_names)
        .cloned()
        .collect::<Vec<_>>();
    let header_tensors_missing_from_index = header_names
        .difference(&index_names)
        .cloned()
        .collect::<Vec<_>>();
    let visual_or_other_observed_tensors = header_names
        .difference(&expected_names)
        .cloned()
        .collect::<Vec<_>>();
    let expected_shard_count = weight_map.values().collect::<BTreeSet<_>>().len();
    let observed_shard_count = observed_tensors.shards.len();
    let text_tensor_admission_passed = missing_expected_tensors.is_empty()
        && index_tensors_missing_from_headers.is_empty()
        && header_tensors_missing_from_index.is_empty()
        && shape_mismatches.is_empty()
        && dtype_mismatches.is_empty()
        && shard_mismatches.is_empty()
        && expected_shard_count == observed_shard_count;

    Qwen35TextTensorAdmissionReport {
        expected_text_tensor_count: expected_tensors.len(),
        expected_decoder_tensor_count,
        expected_mtp_tensor_count,
        observed_index_tensor_count: index_names.len(),
        observed_header_tensor_count: header_names.len(),
        admitted_text_tensor_count,
        admitted_decoder_tensor_count,
        admitted_mtp_tensor_count,
        visual_or_other_observed_tensor_count: visual_or_other_observed_tensors.len(),
        expected_shard_count,
        observed_shard_count,
        shard_headers: observed_tensors.shards,
        missing_expected_tensors,
        index_tensors_missing_from_headers,
        header_tensors_missing_from_index,
        visual_or_other_observed_tensors,
        shape_mismatches,
        dtype_mismatches,
        shard_mismatches,
        text_tensor_admission_passed,
    }
}

pub fn qwen35_text_read_indexed_tensor_row(
    model_dir: &Path,
    weight_map: &BTreeMap<String, String>,
    tensor_name: &str,
    row_index: usize,
) -> Result<Qwen35TextTensorRow, Qwen35TextCheckpointError> {
    let shard_name = weight_map.get(tensor_name).ok_or_else(|| {
        Qwen35TextCheckpointError::InvalidConfig(format!(
            "model index does not contain required tensor `{tensor_name}`"
        ))
    })?;
    let shard_path = model_dir.join(shard_name);
    let mut file = File::open(&shard_path).map_err(|source| Qwen35TextCheckpointError::Io {
        path: shard_path.clone(),
        source,
    })?;
    let mut len_bytes = [0u8; 8];
    file.read_exact(&mut len_bytes)
        .map_err(|source| Qwen35TextCheckpointError::Io {
            path: shard_path.clone(),
            source,
        })?;
    let header_len = u64::from_le_bytes(len_bytes);
    let header_len_usize =
        usize::try_from(header_len).map_err(|_| Qwen35TextCheckpointError::InvalidShard {
            path: shard_path.clone(),
            detail: String::from("header length does not fit usize"),
        })?;
    let mut header_bytes = vec![0u8; header_len_usize];
    file.read_exact(&mut header_bytes)
        .map_err(|source| Qwen35TextCheckpointError::Io {
            path: shard_path.clone(),
            source,
        })?;
    let header = serde_json::from_slice::<Value>(header_bytes.as_slice())?;
    let tensor_value = header.get(tensor_name).ok_or_else(|| {
        Qwen35TextCheckpointError::InvalidConfig(format!(
            "safetensors shard `{shard_name}` does not contain tensor `{tensor_name}`"
        ))
    })?;
    let dtype = required_string(tensor_value, "dtype")?;
    let shape = tensor_value
        .get("shape")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            Qwen35TextCheckpointError::InvalidConfig(format!(
                "tensor `{tensor_name}` in `{shard_name}` is missing shape"
            ))
        })?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| {
                    Qwen35TextCheckpointError::InvalidConfig(format!(
                        "tensor `{tensor_name}` has a non-usize shape entry"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if shape.is_empty() {
        return Err(Qwen35TextCheckpointError::InvalidConfig(format!(
            "indexed tensor read requires at least one dimension; `{tensor_name}` has shape {shape:?}"
        )));
    }
    let row_count = if shape.len() == 1 { 1 } else { shape[0] };
    if row_index >= row_count {
        return Err(Qwen35TextCheckpointError::InvalidConfig(format!(
            "row {row_index} is outside tensor `{tensor_name}` with {row_count} rows"
        )));
    }
    let offsets = tensor_value
        .get("data_offsets")
        .and_then(Value::as_array)
        .filter(|offsets| offsets.len() == 2)
        .ok_or_else(|| {
            Qwen35TextCheckpointError::InvalidConfig(format!(
                "tensor `{tensor_name}` in `{shard_name}` must have two data_offsets"
            ))
        })?;
    let data_offset_start = offsets[0].as_u64().ok_or_else(|| {
        Qwen35TextCheckpointError::InvalidConfig(format!(
            "tensor `{tensor_name}` has invalid data_offsets"
        ))
    })?;
    let data_offset_end = offsets[1].as_u64().ok_or_else(|| {
        Qwen35TextCheckpointError::InvalidConfig(format!(
            "tensor `{tensor_name}` has invalid data_offsets"
        ))
    })?;
    let element_bytes = safetensors_element_size(dtype.as_str())?;
    let row_element_count = if shape.len() == 1 {
        shape[0]
    } else {
        shape[1..]
            .iter()
            .try_fold(1usize, |count, dimension| count.checked_mul(*dimension))
            .ok_or_else(|| {
                Qwen35TextCheckpointError::InvalidConfig(format!(
                    "tensor `{tensor_name}` row element count overflow"
                ))
            })?
    };
    let row_bytes_len = row_element_count
        .checked_mul(element_bytes)
        .ok_or_else(|| {
            Qwen35TextCheckpointError::InvalidConfig(format!(
                "tensor `{tensor_name}` row byte length overflow"
            ))
        })?;
    let tensor_bytes_len = data_offset_end
        .checked_sub(data_offset_start)
        .ok_or_else(|| {
            Qwen35TextCheckpointError::InvalidConfig(format!(
                "tensor `{tensor_name}` has reversed data_offsets"
            ))
        })?;
    let expected_tensor_bytes = (row_count as u64)
        .checked_mul(row_bytes_len as u64)
        .ok_or_else(|| {
            Qwen35TextCheckpointError::InvalidConfig(format!(
                "tensor `{tensor_name}` data byte length overflow"
            ))
        })?;
    if tensor_bytes_len != expected_tensor_bytes {
        return Err(Qwen35TextCheckpointError::InvalidConfig(format!(
            "tensor `{tensor_name}` data_offsets describe {tensor_bytes_len} bytes, expected {expected_tensor_bytes}"
        )));
    }
    let row_offset = 8u64
        .checked_add(header_len)
        .and_then(|offset| offset.checked_add(data_offset_start))
        .and_then(|offset| {
            offset.checked_add((row_index as u64).checked_mul(row_bytes_len as u64)?)
        })
        .ok_or_else(|| {
            Qwen35TextCheckpointError::InvalidConfig(format!(
                "tensor `{tensor_name}` row offset overflow"
            ))
        })?;
    file.seek(SeekFrom::Start(row_offset))
        .map_err(|source| Qwen35TextCheckpointError::Io {
            path: shard_path.clone(),
            source,
        })?;
    let mut row_bytes = vec![0u8; row_bytes_len];
    file.read_exact(&mut row_bytes)
        .map_err(|source| Qwen35TextCheckpointError::Io {
            path: shard_path,
            source,
        })?;
    let values = decode_safetensors_row(dtype.as_str(), row_bytes.as_slice())?;
    Ok(Qwen35TextTensorRow {
        values,
        receipt: Qwen35TextTensorRowReadReceipt {
            tensor_name: String::from(tensor_name),
            shard_name: shard_name.clone(),
            row_index,
            dtype,
            shape,
            row_sha256: sha256_hex(row_bytes.as_slice()),
        },
    })
}

fn safetensors_header(
    path: &Path,
) -> Result<
    (
        Qwen35TextSafetensorsShardHeaderReport,
        Vec<Qwen35TextObservedTensorSpec>,
    ),
    Qwen35TextCheckpointError,
> {
    let mut file = File::open(path).map_err(|source| Qwen35TextCheckpointError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let byte_len = file
        .metadata()
        .map_err(|source| Qwen35TextCheckpointError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    let mut len_bytes = [0u8; 8];
    file.read_exact(&mut len_bytes)
        .map_err(|source| Qwen35TextCheckpointError::InvalidShard {
            path: path.to_path_buf(),
            detail: format!("cannot read eight-byte header length: {source}"),
        })?;
    let header_len_u64 = u64::from_le_bytes(len_bytes);
    if header_len_u64 > byte_len.saturating_sub(8) {
        return Err(Qwen35TextCheckpointError::InvalidShard {
            path: path.to_path_buf(),
            detail: format!(
                "declared header length {header_len_u64} exceeds {} available bytes",
                byte_len.saturating_sub(8)
            ),
        });
    }
    let header_len =
        usize::try_from(header_len_u64).map_err(|_| Qwen35TextCheckpointError::InvalidShard {
            path: path.to_path_buf(),
            detail: format!("header length {header_len_u64} does not fit usize"),
        })?;
    let mut header_bytes = vec![0u8; header_len];
    file.read_exact(&mut header_bytes).map_err(|source| {
        Qwen35TextCheckpointError::InvalidShard {
            path: path.to_path_buf(),
            detail: format!("cannot read declared header bytes: {source}"),
        }
    })?;
    let object = serde_json::from_slice::<UniqueJsonObject>(header_bytes.as_slice())
        .map_err(|error| Qwen35TextCheckpointError::InvalidShard {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?
        .0;
    let shard_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let mut tensors = Vec::new();
    for (name, tensor_value) in object {
        if name == "__metadata__" {
            continue;
        }
        let dtype = required_string(&tensor_value, "dtype").map_err(|error| {
            Qwen35TextCheckpointError::InvalidShard {
                path: path.to_path_buf(),
                detail: format!("tensor `{name}`: {error}"),
            }
        })?;
        let shape = tensor_value
            .get("shape")
            .and_then(Value::as_array)
            .ok_or_else(|| Qwen35TextCheckpointError::InvalidShard {
                path: path.to_path_buf(),
                detail: format!("tensor `{name}` is missing shape"),
            })?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| Qwen35TextCheckpointError::InvalidShard {
                        path: path.to_path_buf(),
                        detail: format!("tensor `{name}` has a non-usize shape entry"),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        tensors.push(Qwen35TextObservedTensorSpec {
            name,
            dtype,
            shape,
            shard: shard_name.clone(),
        });
    }
    tensors.sort_by(|left, right| left.name.cmp(&right.name));
    let tensor_count = tensors.len();
    Ok((
        Qwen35TextSafetensorsShardHeaderReport {
            path: path.display().to_string(),
            shard_name,
            byte_len,
            header_sha256: sha256_hex(header_bytes.as_slice()),
            tensor_count,
        },
        tensors,
    ))
}

fn push_common_decoder_layer_specs(
    specs: &mut Vec<Qwen35TextTensorSpec>,
    prefix: &str,
    dtype: &str,
    architecture: &Qwen35TextArchitectureReport,
) {
    specs.extend([
        tensor(
            format!("{prefix}.input_layernorm.weight"),
            dtype,
            [architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.post_attention_layernorm.weight"),
            dtype,
            [architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.mlp.gate_proj.weight"),
            dtype,
            [architecture.intermediate_size, architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.mlp.up_proj.weight"),
            dtype,
            [architecture.intermediate_size, architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.mlp.down_proj.weight"),
            dtype,
            [architecture.hidden_size, architecture.intermediate_size],
        ),
    ]);
}

fn push_full_attention_specs(
    specs: &mut Vec<Qwen35TextTensorSpec>,
    prefix: &str,
    dtype: &str,
    architecture: &Qwen35TextArchitectureReport,
) {
    let q_proj_out = architecture.num_attention_heads * architecture.head_dim * 2;
    let kv_proj_out = architecture.num_key_value_heads * architecture.head_dim;
    let o_proj_in = architecture.num_attention_heads * architecture.head_dim;
    specs.extend([
        tensor(
            format!("{prefix}.self_attn.q_proj.weight"),
            dtype,
            [q_proj_out, architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.self_attn.k_proj.weight"),
            dtype,
            [kv_proj_out, architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.self_attn.v_proj.weight"),
            dtype,
            [kv_proj_out, architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.self_attn.o_proj.weight"),
            dtype,
            [architecture.hidden_size, o_proj_in],
        ),
        tensor(
            format!("{prefix}.self_attn.q_norm.weight"),
            dtype,
            [architecture.head_dim],
        ),
        tensor(
            format!("{prefix}.self_attn.k_norm.weight"),
            dtype,
            [architecture.head_dim],
        ),
    ]);
}

fn push_linear_attention_specs(
    specs: &mut Vec<Qwen35TextTensorSpec>,
    prefix: &str,
    dtype: &str,
    architecture: &Qwen35TextArchitectureReport,
) -> Result<(), Qwen35TextCheckpointError> {
    let key_heads =
        required_linear_fact(architecture.linear_num_key_heads, "linear_num_key_heads")?;
    let value_heads = required_linear_fact(
        architecture.linear_num_value_heads,
        "linear_num_value_heads",
    )?;
    let key_dim = required_linear_fact(architecture.linear_key_head_dim, "linear_key_head_dim")?;
    let value_dim =
        required_linear_fact(architecture.linear_value_head_dim, "linear_value_head_dim")?;
    let conv_kernel = required_linear_fact(
        architecture.linear_conv_kernel_dim,
        "linear_conv_kernel_dim",
    )?;
    let key_width = key_heads * key_dim;
    let value_width = value_heads * value_dim;
    let qkv_width = key_width * 2 + value_width;
    specs.extend([
        tensor(format!("{prefix}.linear_attn.A_log"), dtype, [value_heads]),
        tensor(
            format!("{prefix}.linear_attn.dt_bias"),
            dtype,
            [value_heads],
        ),
        tensor(
            format!("{prefix}.linear_attn.in_proj_a.weight"),
            dtype,
            [value_heads, architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.linear_attn.in_proj_b.weight"),
            dtype,
            [value_heads, architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.linear_attn.in_proj_qkv.weight"),
            dtype,
            [qkv_width, architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.linear_attn.in_proj_z.weight"),
            dtype,
            [value_width, architecture.hidden_size],
        ),
        tensor(
            format!("{prefix}.linear_attn.out_proj.weight"),
            dtype,
            [architecture.hidden_size, value_width],
        ),
        tensor(
            format!("{prefix}.linear_attn.norm.weight"),
            dtype,
            [value_dim],
        ),
        tensor(
            format!("{prefix}.linear_attn.conv1d.weight"),
            dtype,
            [qkv_width, 1, conv_kernel],
        ),
    ]);
    Ok(())
}

fn required_linear_fact(
    value: Option<usize>,
    field: &str,
) -> Result<usize, Qwen35TextCheckpointError> {
    value.ok_or_else(|| {
        Qwen35TextCheckpointError::InvalidConfig(format!("linear_attention layers require {field}"))
    })
}

fn tensor<const N: usize>(
    name: impl Into<String>,
    dtype: &str,
    shape: [usize; N],
) -> Qwen35TextTensorSpec {
    Qwen35TextTensorSpec {
        name: name.into(),
        dtype: String::from(dtype),
        shape: shape.to_vec(),
    }
}

fn is_mtp_tensor_name(name: &str) -> bool {
    name.starts_with("mtp.")
}

fn required_string(value: &Value, key: &str) -> Result<String, Qwen35TextCheckpointError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| {
            Qwen35TextCheckpointError::InvalidConfig(format!("config is missing `{key}`"))
        })
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(String::from)
}

fn required_usize(value: &Value, key: &str) -> Result<usize, Qwen35TextCheckpointError> {
    optional_usize(value, key).ok_or_else(|| {
        Qwen35TextCheckpointError::InvalidConfig(format!("config is missing numeric `{key}`"))
    })
}

fn optional_usize(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn hf_dtype_to_safetensors_dtype(dtype: &str) -> String {
    match dtype {
        "bfloat16" => String::from("BF16"),
        "float16" => String::from("F16"),
        "float32" => String::from("F32"),
        other => other.to_ascii_uppercase(),
    }
}

fn safetensors_element_size(dtype: &str) -> Result<usize, Qwen35TextCheckpointError> {
    match dtype {
        "BF16" | "F16" => Ok(2),
        "F32" => Ok(4),
        other => Err(Qwen35TextCheckpointError::InvalidConfig(format!(
            "unsupported row-read dtype `{other}`"
        ))),
    }
}

fn decode_safetensors_row(
    dtype: &str,
    bytes: &[u8],
) -> Result<Vec<f32>, Qwen35TextCheckpointError> {
    match dtype {
        "BF16" => Ok(bytes
            .chunks_exact(2)
            .map(|bytes| bf16::from_bits(u16::from_le_bytes([bytes[0], bytes[1]])).to_f32())
            .collect()),
        "F16" => Ok(bytes
            .chunks_exact(2)
            .map(|bytes| f16::from_bits(u16::from_le_bytes([bytes[0], bytes[1]])).to_f32())
            .collect()),
        "F32" => Ok(bytes
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            .collect()),
        other => Err(Qwen35TextCheckpointError::InvalidConfig(format!(
            "unsupported row-read dtype `{other}`"
        ))),
    }
}

fn sha256_json<T: Serialize>(value: &T) -> Result<String, Qwen35TextCheckpointError> {
    Ok(sha256_hex(serde_json::to_vec(value)?.as_slice()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WeightIndexDocument {
    #[serde(default)]
    metadata: Option<WeightIndexMetadata>,
    #[serde(deserialize_with = "deserialize_unique_string_map")]
    weight_map: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WeightIndexMetadata {
    #[serde(default)]
    total_size: Option<f64>,
}

fn deserialize_unique_string_map<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct UniqueStringMapVisitor;

    impl<'de> Visitor<'de> for UniqueStringMapVisitor {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a tensor-name to shard-name object with unique keys")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut values = BTreeMap::new();
            while let Some((name, shard)) = map.next_entry::<String, String>()? {
                if values.insert(name.clone(), shard).is_some() {
                    return Err(de::Error::custom(format!(
                        "duplicate tensor mapping `{name}`"
                    )));
                }
            }
            Ok(values)
        }
    }

    deserializer.deserialize_map(UniqueStringMapVisitor)
}

struct UniqueJsonObject(BTreeMap<String, Value>);

impl<'de> Deserialize<'de> for UniqueJsonObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct UniqueJsonObjectVisitor;

        impl<'de> Visitor<'de> for UniqueJsonObjectVisitor {
            type Value = UniqueJsonObject;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON object with unique keys")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((name, value)) = map.next_entry::<String, Value>()? {
                    if values.insert(name.clone(), value).is_some() {
                        return Err(de::Error::custom(format!("duplicate tensor `{name}`")));
                    }
                }
                Ok(UniqueJsonObject(values))
            }
        }

        deserializer.deserialize_map(UniqueJsonObjectVisitor)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn qwen35_text_checkpoint_rejects_unsupported_architecture() {
        let error = qwen35_text_architecture_report(
            br#"{"architectures":["Other"],"model_type":"other","text_config":{"model_type":"other"}}"#,
        )
        .expect_err("unsupported architecture");
        assert_eq!(error.code(), "unsupported_architecture");
    }

    #[test]
    fn qwen35_text_checkpoint_rejects_duplicate_index_mapping() {
        let error = qwen35_text_weight_index_from_bytes(
            br#"{"weight_map":{"tensor":"a.safetensors","tensor":"b.safetensors"}}"#,
        )
        .expect_err("duplicate mapping");
        assert!(matches!(
            error,
            Qwen35TextCheckpointError::DuplicateTensorMapping { ref tensor_name }
                if tensor_name == "tensor"
        ));
    }

    #[test]
    fn qwen35_text_checkpoint_reports_shape_dtype_and_shard_drift() {
        let expected = vec![
            Qwen35TextTensorSpec {
                name: String::from("model.language_model.norm.weight"),
                dtype: String::from("BF16"),
                shape: vec![4],
            },
            Qwen35TextTensorSpec {
                name: String::from("model.language_model.embed_tokens.weight"),
                dtype: String::from("BF16"),
                shape: vec![8, 4],
            },
        ];
        let mut weight_map = BTreeMap::new();
        weight_map.insert(
            String::from("model.language_model.norm.weight"),
            String::from("expected.safetensors"),
        );
        let observed = Qwen35TextObservedTensorSet {
            shards: vec![Qwen35TextSafetensorsShardHeaderReport {
                path: String::from("observed.safetensors"),
                shard_name: String::from("observed.safetensors"),
                byte_len: 8,
                header_sha256: String::from("hash"),
                tensor_count: 1,
            }],
            tensors: vec![Qwen35TextObservedTensorSpec {
                name: String::from("model.language_model.norm.weight"),
                dtype: String::from("F16"),
                shape: vec![5],
                shard: String::from("observed.safetensors"),
            }],
        };

        let report = qwen35_text_tensor_admission_report(expected, weight_map, observed);
        assert_eq!(report.shape_mismatches.len(), 1);
        assert_eq!(report.dtype_mismatches.len(), 1);
        assert_eq!(report.shard_mismatches.len(), 1);
        assert_eq!(report.missing_expected_tensors.len(), 1);
        assert_eq!(
            report.missing_expected_tensors[0].name,
            "model.language_model.embed_tokens.weight"
        );
        assert!(!report.text_tensor_admission_passed);
    }

    #[test]
    fn qwen35_text_checkpoint_rejects_missing_bad_and_duplicate_shards() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut weight_map = BTreeMap::new();
        weight_map.insert(String::from("tensor"), String::from("missing.safetensors"));
        assert!(matches!(
            qwen35_text_shard_paths_from_weight_map(temp.path(), &weight_map),
            Err(Qwen35TextCheckpointError::MissingShards { .. })
        ));

        let bad = temp.path().join("bad.safetensors");
        fs::write(&bad, [1, 2, 3]).expect("bad shard");
        assert!(matches!(
            qwen35_text_observed_tensors_from_shards(&[bad]),
            Err(Qwen35TextCheckpointError::InvalidShard { .. })
        ));

        let first = temp.path().join("first.safetensors");
        let second = temp.path().join("second.safetensors");
        write_header_only_shard(&first, "same");
        write_header_only_shard(&second, "same");
        assert!(matches!(
            qwen35_text_observed_tensors_from_shards(&[first, second]),
            Err(Qwen35TextCheckpointError::DuplicateObservedTensor { .. })
        ));
    }

    fn write_header_only_shard(path: &Path, tensor_name: &str) {
        let header = serde_json::json!({
            (tensor_name): {
                "dtype": "BF16",
                "shape": [1],
                "data_offsets": [0, 0]
            }
        });
        let header = serde_json::to_vec(&header).expect("header JSON");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(header.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&header);
        fs::write(path, bytes).expect("write shard");
    }
}
