use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use psionic_catalog::{BlobReadPreference, LocalBlobOpenOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    GgufBlobArtifact, GgufMetadataValue, GgufRuntimeTokenizer, ModelLoadError, QWEN38_27B_MODEL_ID,
    QWEN38_27B_UPSTREAM_REVISION, Qwen38PromptError, Qwen38PromptMessage, Qwen38PromptOptions,
    Qwen38PromptRole, Qwen38Tokenizer, Qwen38TokenizerError, TokenizerBoundary,
    render_qwen38_prompt,
};

pub const QWEN38_GGUF_QUALIFICATION_SCHEMA_VERSION: &str =
    "psionic.qwen38_27b_gguf_qualification.v1";
pub const QWEN38_GGUF_REPOSITORY_ID: &str = "unsloth/Qwen3.8-27B-GGUF";
pub const QWEN38_GGUF_REPOSITORY_REVISION: &str = "fdd03b8bbd279c1694563650e79d85a2373d9934";
pub const QWEN38_GGUF_AUDITED_LLAMA_CPP_REVISION: &str = "9b05354ec6fb58b4e665e9a39ebc40285c015638";
pub const QWEN38_GGUF_PROMPT_TOKENIZER_FIXTURE_PATH: &str =
    "fixtures/qwen38/qwen38_prompt_tokenizer_golden_v1.json";

const QWEN38_CUDA_DEVICE_TOTAL_BYTES: u64 = 17_171_480_576;
const QWEN38_CUDA_OBSERVED_FREE_BYTES: u64 = 15_998_124_032;
const QWEN38_RECURRENT_STATE_BYTES: u64 = 156_893_184;
const QWEN38_SCRATCH_BYTES: u64 = 536_870_912;
const QWEN38_GRAPH_BYTES: u64 = 268_435_456;
const QWEN38_CUDA_CONTEXT_BYTES: u64 = 335_544_320;
const QWEN38_ALLOCATOR_MARGIN_BYTES: u64 = 268_435_456;
const QWEN38_KV_BYTES_PER_TOKEN: u64 = 16_384;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38GgufProfile {
    DynamicV3UdQ3KXl,
    Q3KM,
    Q4KM,
}

impl Qwen38GgufProfile {
    pub const fn filename(self) -> &'static str {
        match self {
            Self::DynamicV3UdQ3KXl => "Qwen3.8-27B-UD-Q3_K_XL.gguf",
            Self::Q3KM => "Qwen3.8-27B-Q3_K_M.gguf",
            Self::Q4KM => "Qwen3.8-27B-Q4_K_M.gguf",
        }
    }

    pub const fn byte_length(self) -> u64 {
        match self {
            Self::DynamicV3UdQ3KXl => 13_441_059_904,
            Self::Q3KM => 13_818_690_528,
            Self::Q4KM => 17_106_775_008,
        }
    }

    pub const fn sha256(self) -> &'static str {
        match self {
            Self::DynamicV3UdQ3KXl => {
                "00cf92e666c6af6566996c38c89a44ccdb6449ea25ef0f112a452c853b2a71e2"
            }
            Self::Q3KM => "7f3b845b563888ec3abc269474cf744bf703a7ce8766dbb7f696c63975facfd7",
            Self::Q4KM => "7e78da5d7e3ae28d178121f58646953305f3e5bd3cb46f4a75584e8b6c6fe169",
        }
    }

    fn expected_type_counts(self) -> BTreeMap<String, usize> {
        let counts: &[(&str, usize)] = match self {
            Self::DynamicV3UdQ3KXl => &[
                ("f32", 360),
                ("iq3_s", 130),
                ("iq4_xs", 357),
                ("q3_k", 1),
                ("q5_k", 18),
            ],
            Self::Q3KM => &[
                ("f32", 456),
                ("q3_k", 213),
                ("q4_k", 189),
                ("q5_k", 6),
                ("q6_k", 1),
                ("q8_0", 1),
            ],
            Self::Q4KM => &[
                ("f32", 456),
                ("q4_k", 294),
                ("q5_k", 48),
                ("q6_k", 67),
                ("q8_0", 1),
            ],
        };
        counts
            .iter()
            .map(|(name, count)| (String::from(*name), *count))
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufSourceProvenance {
    pub repository_id: String,
    pub repository_revision: String,
    pub filename: String,
    pub byte_length: u64,
    pub sha256: String,
    pub quantized_by: String,
    pub base_model_name: String,
    pub base_model_repository_url: String,
    pub official_model_id: String,
    pub official_model_revision: String,
    pub converter_revision_embedded: Option<String>,
    pub audited_converter: String,
    pub audited_converter_revision: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufArchitectureFacts {
    pub architecture: String,
    pub model_name: String,
    pub block_count_including_mtp: usize,
    pub decoder_block_count: usize,
    pub nextn_predict_layers: usize,
    pub context_length: usize,
    pub embedding_length: usize,
    pub feed_forward_length: usize,
    pub attention_head_count: usize,
    pub attention_kv_head_count: usize,
    pub rope_dimension_sections: Vec<usize>,
    pub rope_frequency_base: usize,
    pub ssm_conv_kernel: usize,
    pub ssm_state_size: usize,
    pub ssm_group_count: usize,
    pub ssm_time_step_rank: usize,
    pub ssm_inner_size: usize,
    pub full_attention_interval: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufTensorInventoryEntry {
    pub name: String,
    pub shape: Vec<usize>,
    pub tensor_type: String,
    pub blob_byte_offset: usize,
    pub byte_length: usize,
    pub standard_generation_disposition: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufStorageInventory {
    pub tensor_count: usize,
    pub type_counts: BTreeMap<String, usize>,
    pub required_runtime_types: Vec<String>,
    pub all_types_have_native_loader_cpu_and_cuda_support: bool,
    pub tensor_inventory_sha256: String,
    pub tensors: Vec<Qwen38GgufTensorInventoryEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufTokenizerQualification {
    pub model: String,
    pub pretokenizer: String,
    pub vocabulary_size: usize,
    pub embedded_chat_template_sha256: String,
    pub official_prompt_renderer_policy: String,
    pub prompt_tokenizer_fixture_sha256: String,
    pub compared_case_count: usize,
    pub exact_token_id_match: bool,
    pub comparison_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufMtpDisposition {
    pub tensor_prefix: String,
    pub tensor_count: usize,
    pub stored_bytes: u64,
    pub standard_generation_policy: String,
    pub execution_owner: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufMemoryEnvelope {
    pub context_tokens: usize,
    pub artifact_bytes_conservative: u64,
    pub recurrent_state_bytes: u64,
    pub kv_cache_bytes: u64,
    pub scratch_bytes: u64,
    pub graph_capture_bytes: u64,
    pub cuda_context_bytes: u64,
    pub allocator_margin_bytes: u64,
    pub conservative_total_bytes: u64,
    pub device_total_bytes: u64,
    pub observed_free_bytes: u64,
    pub bytes_remaining_against_observed_free: i64,
    pub preflight_status: String,
    pub runtime_peak_status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufAdmission {
    pub artifact_qualification: String,
    pub standard_generation_text_only: bool,
    pub cuda_full_residency_contexts: Vec<usize>,
    pub cpu_offload_comparator: bool,
    pub dynamic_v3_canonical_status: String,
    pub v_head_layout: String,
    pub v_head_layout_evidence: String,
    pub unknown_producer_policy: String,
    pub converter_parity_report_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38GgufQualificationReport {
    pub schema_version: String,
    pub profile: Qwen38GgufProfile,
    pub source: Qwen38GgufSourceProvenance,
    pub architecture: Qwen38GgufArchitectureFacts,
    pub storage: Qwen38GgufStorageInventory,
    pub tokenizer: Qwen38GgufTokenizerQualification,
    pub mtp: Qwen38GgufMtpDisposition,
    pub memory_envelopes: Vec<Qwen38GgufMemoryEnvelope>,
    pub admission: Qwen38GgufAdmission,
    pub qualification_sha256: String,
}

#[derive(Debug, Error)]
pub enum Qwen38GgufQualificationError {
    #[error("Qwen3.8 GGUF qualification failed: {0}")]
    Qualification(String),
    #[error(transparent)]
    ModelLoad(#[from] ModelLoadError),
    #[error(transparent)]
    Prompt(#[from] Qwen38PromptError),
    #[error(transparent)]
    Tokenizer(#[from] Qwen38TokenizerError),
    #[error("Qwen3.8 GGUF qualification I/O failed at `{path}`: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize Qwen3.8 GGUF qualification: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub fn qualify_qwen38_gguf(
    gguf_path: impl AsRef<Path>,
    official_model_dir: impl AsRef<Path>,
    prompt_fixture_path: impl AsRef<Path>,
    profile: Qwen38GgufProfile,
    converter_parity_report_sha256: impl Into<String>,
) -> Result<Qwen38GgufQualificationReport, Qwen38GgufQualificationError> {
    let gguf_path = gguf_path.as_ref();
    let official_model_dir = official_model_dir.as_ref();
    let prompt_fixture_path = prompt_fixture_path.as_ref();
    let artifact = GgufBlobArtifact::open_path(
        gguf_path,
        LocalBlobOpenOptions::default()
            .with_read_preference(BlobReadPreference::RequireMemoryMap)
            .with_expected_sha256(format!("sha256:{}", profile.sha256())),
    )?;
    let content = artifact.content();
    if artifact.blob_metadata().byte_length != profile.byte_length() {
        return qualification_error(format!(
            "profile {:?} expected {} bytes, found {}",
            profile,
            profile.byte_length(),
            artifact.blob_metadata().byte_length
        ));
    }

    let source = Qwen38GgufSourceProvenance {
        repository_id: String::from(QWEN38_GGUF_REPOSITORY_ID),
        repository_revision: String::from(QWEN38_GGUF_REPOSITORY_REVISION),
        filename: String::from(profile.filename()),
        byte_length: profile.byte_length(),
        sha256: String::from(profile.sha256()),
        quantized_by: required_string(content.metadata(), "general.quantized_by")?,
        base_model_name: required_string(content.metadata(), "general.base_model.0.name")?,
        base_model_repository_url: required_string(
            content.metadata(),
            "general.base_model.0.repo_url",
        )?,
        official_model_id: String::from(QWEN38_27B_MODEL_ID),
        official_model_revision: String::from(QWEN38_27B_UPSTREAM_REVISION),
        converter_revision_embedded: None,
        audited_converter: String::from("ggml-org/llama.cpp conversion/qwen.py"),
        audited_converter_revision: String::from(QWEN38_GGUF_AUDITED_LLAMA_CPP_REVISION),
    };
    validate_source(&source)?;

    let architecture = architecture_facts(content.metadata())?;
    validate_architecture(&architecture)?;
    let storage = storage_inventory(content)?;
    validate_storage(profile, &storage)?;
    let tokenizer = tokenizer_qualification(content, official_model_dir, prompt_fixture_path)?;
    let mtp = mtp_disposition(&storage);
    if mtp.tensor_count != 15 {
        return qualification_error(format!(
            "expected 15 stored MTP tensors, found {}",
            mtp.tensor_count
        ));
    }
    let memory_envelopes = [4096, 8192]
        .into_iter()
        .map(|context_tokens| memory_envelope(profile, context_tokens))
        .collect::<Vec<_>>();
    let cuda_full_residency_contexts = if profile == Qwen38GgufProfile::Q4KM {
        Vec::new()
    } else {
        vec![4096]
    };
    let mut report = Qwen38GgufQualificationReport {
        schema_version: String::from(QWEN38_GGUF_QUALIFICATION_SCHEMA_VERSION),
        profile,
        source,
        architecture,
        storage,
        tokenizer,
        mtp,
        memory_envelopes,
        admission: Qwen38GgufAdmission {
            artifact_qualification: String::from("admitted_for_native_runtime_implementation"),
            standard_generation_text_only: true,
            cuda_full_residency_contexts,
            cpu_offload_comparator: profile == Qwen38GgufProfile::Q4KM,
            dynamic_v3_canonical_status: String::from(
                "candidate_only_pending_real_generation_quality_and_parity_in_r6",
            ),
            v_head_layout: String::from("tiled"),
            v_head_layout_evidence: String::from(
                "pinned llama.cpp converter provenance plus sampled official-BF16 parity",
            ),
            unknown_producer_policy: String::from(
                "refuse_without_equivalent_converter_and_v_head_layout_evidence",
            ),
            converter_parity_report_sha256: converter_parity_report_sha256.into(),
        },
        qualification_sha256: String::new(),
    };
    report.qualification_sha256 = report_digest(&report)?;
    Ok(report)
}

fn architecture_facts(
    metadata: &BTreeMap<String, GgufMetadataValue>,
) -> Result<Qwen38GgufArchitectureFacts, Qwen38GgufQualificationError> {
    Ok(Qwen38GgufArchitectureFacts {
        architecture: required_string(metadata, "general.architecture")?,
        model_name: required_string(metadata, "general.name")?,
        block_count_including_mtp: required_usize(metadata, "qwen35.block_count")?,
        decoder_block_count: 64,
        nextn_predict_layers: required_usize(metadata, "qwen35.nextn_predict_layers")?,
        context_length: required_usize(metadata, "qwen35.context_length")?,
        embedding_length: required_usize(metadata, "qwen35.embedding_length")?,
        feed_forward_length: required_usize(metadata, "qwen35.feed_forward_length")?,
        attention_head_count: required_usize(metadata, "qwen35.attention.head_count")?,
        attention_kv_head_count: required_usize(metadata, "qwen35.attention.head_count_kv")?,
        rope_dimension_sections: required_usize_array(metadata, "qwen35.rope.dimension_sections")?,
        rope_frequency_base: required_usize(metadata, "qwen35.rope.freq_base")?,
        ssm_conv_kernel: required_usize(metadata, "qwen35.ssm.conv_kernel")?,
        ssm_state_size: required_usize(metadata, "qwen35.ssm.state_size")?,
        ssm_group_count: required_usize(metadata, "qwen35.ssm.group_count")?,
        ssm_time_step_rank: required_usize(metadata, "qwen35.ssm.time_step_rank")?,
        ssm_inner_size: required_usize(metadata, "qwen35.ssm.inner_size")?,
        full_attention_interval: required_usize(metadata, "qwen35.full_attention_interval")?,
    })
}

fn validate_architecture(
    facts: &Qwen38GgufArchitectureFacts,
) -> Result<(), Qwen38GgufQualificationError> {
    let matches = facts.architecture == "qwen35"
        && facts.model_name == "Qwen3.8-27B"
        && facts.block_count_including_mtp == 65
        && facts.decoder_block_count == 64
        && facts.nextn_predict_layers == 1
        && facts.context_length == 262_144
        && facts.embedding_length == 5120
        && facts.feed_forward_length == 17_408
        && facts.attention_head_count == 24
        && facts.attention_kv_head_count == 4
        && facts.rope_dimension_sections == [11, 11, 10, 0]
        && facts.rope_frequency_base == 10_000_000
        && facts.ssm_conv_kernel == 4
        && facts.ssm_state_size == 128
        && facts.ssm_group_count == 16
        && facts.ssm_time_step_rank == 48
        && facts.ssm_inner_size == 6144
        && facts.full_attention_interval == 4;
    if !matches {
        return qualification_error(format!(
            "GGUF architecture facts differ from canonical Qwen3.8-27B: {facts:?}"
        ));
    }
    Ok(())
}

fn validate_source(
    source: &Qwen38GgufSourceProvenance,
) -> Result<(), Qwen38GgufQualificationError> {
    if source.quantized_by != "Unsloth" {
        return qualification_error(format!(
            "unknown GGUF producer `{}`; expected `Unsloth`",
            source.quantized_by
        ));
    }
    if source.base_model_name != "Qwen3.8 27B"
        || source.base_model_repository_url != "https://huggingface.co/Qwen/Qwen3.8-27B"
    {
        return qualification_error(format!(
            "GGUF base-model provenance does not identify Qwen/Qwen3.8-27B: name=`{}` repo=`{}`",
            source.base_model_name, source.base_model_repository_url
        ));
    }
    Ok(())
}

fn storage_inventory(
    content: &crate::GgufContent,
) -> Result<Qwen38GgufStorageInventory, Qwen38GgufQualificationError> {
    let expected = expected_tensor_shapes();
    let mut tensors = Vec::new();
    let mut type_counts = BTreeMap::new();
    let mut missing = expected.keys().cloned().collect::<BTreeSet<_>>();
    for tensor in content.tensor_infos() {
        let expected_shape = expected.get(&tensor.name).ok_or_else(|| {
            Qwen38GgufQualificationError::Qualification(format!(
                "unexpected GGUF tensor `{}`",
                tensor.name
            ))
        })?;
        if tensor.shape.dims() != expected_shape.as_slice() {
            return qualification_error(format!(
                "GGUF tensor `{}` expected shape {:?}, found {:?}",
                tensor.name,
                expected_shape,
                tensor.shape.dims()
            ));
        }
        missing.remove(&tensor.name);
        let tensor_type = tensor.tensor_type.to_string();
        *type_counts.entry(tensor_type.clone()).or_insert(0) += 1;
        let (blob_byte_offset, byte_length) = content.tensor_byte_range(&tensor.name)?;
        tensors.push(Qwen38GgufTensorInventoryEntry {
            name: tensor.name.clone(),
            shape: tensor.shape.dims().to_vec(),
            tensor_type,
            blob_byte_offset,
            byte_length,
            standard_generation_disposition: if tensor.name.starts_with("blk.64.") {
                String::from("stored_skipped_mtp")
            } else {
                String::from("required_text_runtime")
            },
        });
    }
    if !missing.is_empty() {
        return qualification_error(format!("missing GGUF tensors: {missing:?}"));
    }
    let tensor_inventory_sha256 = sha256_json(&tensors)?;
    let required_runtime_types = type_counts.keys().cloned().collect::<Vec<_>>();
    let supported = [
        "f32", "q3_k", "q4_k", "q5_k", "q6_k", "q8_0", "iq3_s", "iq4_xs",
    ];
    let all_types_have_native_loader_cpu_and_cuda_support = required_runtime_types
        .iter()
        .all(|kind| supported.contains(&kind.as_str()));
    Ok(Qwen38GgufStorageInventory {
        tensor_count: tensors.len(),
        type_counts,
        required_runtime_types,
        all_types_have_native_loader_cpu_and_cuda_support,
        tensor_inventory_sha256,
        tensors,
    })
}

fn validate_storage(
    profile: Qwen38GgufProfile,
    storage: &Qwen38GgufStorageInventory,
) -> Result<(), Qwen38GgufQualificationError> {
    if storage.tensor_count != 866 {
        return qualification_error(format!(
            "expected 866 GGUF tensors, found {}",
            storage.tensor_count
        ));
    }
    if storage.type_counts != profile.expected_type_counts() {
        return qualification_error(format!(
            "profile {:?} storage inventory differs: expected {:?}, found {:?}",
            profile,
            profile.expected_type_counts(),
            storage.type_counts
        ));
    }
    if !storage.all_types_have_native_loader_cpu_and_cuda_support {
        return qualification_error(format!(
            "profile {:?} contains a storage type without loader, CPU, and CUDA support",
            profile
        ));
    }
    Ok(())
}

fn tokenizer_qualification(
    content: &crate::GgufContent,
    official_model_dir: &Path,
    fixture_path: &Path,
) -> Result<Qwen38GgufTokenizerQualification, Qwen38GgufQualificationError> {
    let tokenizer_metadata = content.load_tokenizer()?;
    let runtime = GgufRuntimeTokenizer::from_gguf(&tokenizer_metadata).map_err(|error| {
        Qwen38GgufQualificationError::Qualification(format!(
            "failed to construct GGUF runtime tokenizer: {error}"
        ))
    })?;
    let official = Qwen38Tokenizer::from_official_file(official_model_dir.join("tokenizer.json"))?;
    let fixture_bytes =
        fs::read(fixture_path).map_err(|source| Qwen38GgufQualificationError::Io {
            path: fixture_path.to_path_buf(),
            source,
        })?;
    #[derive(Deserialize)]
    struct Fixture {
        tokenizer_cases: Vec<TokenizerCase>,
    }
    #[derive(Deserialize)]
    struct TokenizerCase {
        text: String,
    }
    let fixture: Fixture = serde_json::from_slice(&fixture_bytes)?;
    let mut comparisons = Vec::new();
    for case in &fixture.tokenizer_cases {
        let official_ids = official.encode_text(&case.text)?;
        let gguf_ids = runtime
            .encode(&case.text)
            .as_slice()
            .iter()
            .map(|token| token.0)
            .collect::<Vec<_>>();
        comparisons.push((case.text.clone(), official_ids, gguf_ids));
    }
    let rendered = render_qwen38_prompt(
        &[Qwen38PromptMessage::text(
            Qwen38PromptRole::User,
            "Verify the bounded Qwen3.8 GGUF qualification path.",
        )],
        &Qwen38PromptOptions::default(),
    )?;
    let official_ids = official.encode_text(&rendered.text)?;
    let gguf_ids = runtime
        .encode(&rendered.text)
        .as_slice()
        .iter()
        .map(|token| token.0)
        .collect::<Vec<_>>();
    comparisons.push((
        String::from("rendered_qwen38_prompt"),
        official_ids,
        gguf_ids,
    ));
    let exact_token_id_match = comparisons
        .iter()
        .all(|(_, official_ids, gguf_ids)| official_ids == gguf_ids);
    if !exact_token_id_match {
        return qualification_error(String::from(
            "GGUF tokenizer does not match the official Qwen3.8 tokenizer fixtures",
        ));
    }
    let embedded_template = required_string(content.metadata(), "tokenizer.chat_template")?;
    Ok(Qwen38GgufTokenizerQualification {
        model: required_string(content.metadata(), "tokenizer.ggml.model")?,
        pretokenizer: required_string(content.metadata(), "tokenizer.ggml.pre")?,
        vocabulary_size: tokenizer_metadata.vocabulary.len(),
        embedded_chat_template_sha256: sha256_hex(embedded_template.as_bytes()),
        official_prompt_renderer_policy: String::from(
            "override_embedded_template_with_digest_bound_psionic_qwen38_renderer",
        ),
        prompt_tokenizer_fixture_sha256: sha256_hex(&fixture_bytes),
        compared_case_count: comparisons.len(),
        exact_token_id_match,
        comparison_sha256: sha256_json(&comparisons)?,
    })
}

fn mtp_disposition(storage: &Qwen38GgufStorageInventory) -> Qwen38GgufMtpDisposition {
    let mtp_tensors = storage
        .tensors
        .iter()
        .filter(|tensor| tensor.name.starts_with("blk.64."))
        .collect::<Vec<_>>();
    Qwen38GgufMtpDisposition {
        tensor_prefix: String::from("blk.64."),
        tensor_count: mtp_tensors.len(),
        stored_bytes: mtp_tensors
            .iter()
            .map(|tensor| tensor.byte_length as u64)
            .sum(),
        standard_generation_policy: String::from("inventory_and_skip"),
        execution_owner: String::from("R9A_optional_speculative_decoding"),
    }
}

fn memory_envelope(profile: Qwen38GgufProfile, context_tokens: usize) -> Qwen38GgufMemoryEnvelope {
    let kv_cache_bytes = context_tokens as u64 * QWEN38_KV_BYTES_PER_TOKEN;
    let conservative_total_bytes = profile.byte_length()
        + QWEN38_RECURRENT_STATE_BYTES
        + kv_cache_bytes
        + QWEN38_SCRATCH_BYTES
        + QWEN38_GRAPH_BYTES
        + QWEN38_CUDA_CONTEXT_BYTES
        + QWEN38_ALLOCATOR_MARGIN_BYTES;
    let remaining = QWEN38_CUDA_OBSERVED_FREE_BYTES as i128 - conservative_total_bytes as i128;
    let fits = remaining >= 0;
    Qwen38GgufMemoryEnvelope {
        context_tokens,
        artifact_bytes_conservative: profile.byte_length(),
        recurrent_state_bytes: QWEN38_RECURRENT_STATE_BYTES,
        kv_cache_bytes,
        scratch_bytes: QWEN38_SCRATCH_BYTES,
        graph_capture_bytes: QWEN38_GRAPH_BYTES,
        cuda_context_bytes: QWEN38_CUDA_CONTEXT_BYTES,
        allocator_margin_bytes: QWEN38_ALLOCATOR_MARGIN_BYTES,
        conservative_total_bytes,
        device_total_bytes: QWEN38_CUDA_DEVICE_TOTAL_BYTES,
        observed_free_bytes: QWEN38_CUDA_OBSERVED_FREE_BYTES,
        bytes_remaining_against_observed_free: remaining as i64,
        preflight_status: if fits && context_tokens == 4096 {
            String::from("admitted")
        } else if fits {
            String::from("fits_estimate_but_not_admitted_without_runtime_peak_and_parity")
        } else {
            String::from("refused_full_cuda_residency")
        },
        runtime_peak_status: String::from("not_measured_in_r5"),
    }
}

fn expected_tensor_shapes() -> BTreeMap<String, Vec<usize>> {
    let mut expected = BTreeMap::from([
        (String::from("token_embd.weight"), vec![248_320, 5120]),
        (String::from("output_norm.weight"), vec![5120]),
        (String::from("output.weight"), vec![248_320, 5120]),
    ]);
    for layer in 0..64 {
        let prefix = format!("blk.{layer}");
        if (layer + 1) % 4 == 0 {
            for (suffix, shape) in [
                ("attn_k.weight", vec![1024, 5120]),
                ("attn_k_norm.weight", vec![256]),
                ("attn_norm.weight", vec![5120]),
                ("attn_output.weight", vec![5120, 6144]),
                ("attn_q.weight", vec![12_288, 5120]),
                ("attn_q_norm.weight", vec![256]),
                ("attn_v.weight", vec![1024, 5120]),
                ("ffn_down.weight", vec![5120, 17_408]),
                ("ffn_gate.weight", vec![17_408, 5120]),
                ("ffn_up.weight", vec![17_408, 5120]),
                ("post_attention_norm.weight", vec![5120]),
            ] {
                expected.insert(format!("{prefix}.{suffix}"), shape);
            }
        } else {
            for (suffix, shape) in [
                ("attn_gate.weight", vec![6144, 5120]),
                ("attn_norm.weight", vec![5120]),
                ("attn_qkv.weight", vec![10_240, 5120]),
                ("ffn_down.weight", vec![5120, 17_408]),
                ("ffn_gate.weight", vec![17_408, 5120]),
                ("ffn_up.weight", vec![17_408, 5120]),
                ("post_attention_norm.weight", vec![5120]),
                ("ssm_a", vec![48]),
                ("ssm_alpha.weight", vec![48, 5120]),
                ("ssm_beta.weight", vec![48, 5120]),
                ("ssm_conv1d.weight", vec![10_240, 4]),
                ("ssm_dt.bias", vec![48]),
                ("ssm_norm.weight", vec![128]),
                ("ssm_out.weight", vec![5120, 6144]),
            ] {
                expected.insert(format!("{prefix}.{suffix}"), shape);
            }
        }
    }
    for (suffix, shape) in [
        ("attn_k.weight", vec![1024, 5120]),
        ("attn_k_norm.weight", vec![256]),
        ("attn_norm.weight", vec![5120]),
        ("attn_output.weight", vec![5120, 6144]),
        ("attn_q.weight", vec![12_288, 5120]),
        ("attn_q_norm.weight", vec![256]),
        ("attn_v.weight", vec![1024, 5120]),
        ("ffn_down.weight", vec![5120, 17_408]),
        ("ffn_gate.weight", vec![17_408, 5120]),
        ("ffn_up.weight", vec![17_408, 5120]),
        ("post_attention_norm.weight", vec![5120]),
        ("nextn.eh_proj.weight", vec![5120, 10_240]),
        ("nextn.enorm.weight", vec![5120]),
        ("nextn.hnorm.weight", vec![5120]),
        ("nextn.shared_head_norm.weight", vec![5120]),
    ] {
        expected.insert(format!("blk.64.{suffix}"), shape);
    }
    expected
}

fn required_string(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<String, Qwen38GgufQualificationError> {
    metadata
        .get(key)
        .and_then(GgufMetadataValue::as_str)
        .map(String::from)
        .ok_or_else(|| {
            Qwen38GgufQualificationError::Qualification(format!(
                "missing or invalid GGUF string metadata `{key}`"
            ))
        })
}

fn required_usize(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<usize, Qwen38GgufQualificationError> {
    metadata
        .get(key)
        .and_then(|value| {
            value.as_u64().or_else(|| {
                let value = value.as_f32()?;
                (value.is_finite() && value >= 0.0 && value.fract() == 0.0).then_some(value as u64)
            })
        })
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            Qwen38GgufQualificationError::Qualification(format!(
                "missing or invalid GGUF integer metadata `{key}`"
            ))
        })
}

fn required_usize_array(
    metadata: &BTreeMap<String, GgufMetadataValue>,
    key: &str,
) -> Result<Vec<usize>, Qwen38GgufQualificationError> {
    metadata
        .get(key)
        .and_then(GgufMetadataValue::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .as_u64()
                        .and_then(|value| usize::try_from(value).ok())
                        .ok_or_else(|| {
                            Qwen38GgufQualificationError::Qualification(format!(
                                "GGUF metadata `{key}` has a non-integer array value"
                            ))
                        })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .ok_or_else(|| {
            Qwen38GgufQualificationError::Qualification(format!(
                "missing or invalid GGUF array metadata `{key}`"
            ))
        })
}

fn qualification_error<T>(detail: String) -> Result<T, Qwen38GgufQualificationError> {
    Err(Qwen38GgufQualificationError::Qualification(detail))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_json(value: &impl Serialize) -> Result<String, serde_json::Error> {
    serde_json::to_vec(value).map(|bytes| sha256_hex(&bytes))
}

fn report_digest(report: &Qwen38GgufQualificationReport) -> Result<String, serde_json::Error> {
    let mut canonical = report.clone();
    canonical.qualification_sha256.clear();
    sha256_json(&canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_qwen38_gguf_tensor_contract_has_expected_partition() {
        let expected = expected_tensor_shapes();
        assert_eq!(expected.len(), 866);
        assert_eq!(
            expected
                .keys()
                .filter(|name| name.starts_with("blk.64."))
                .count(),
            15
        );
        assert_eq!(expected["blk.0.ssm_out.weight"], vec![5120, 6144]);
        assert_eq!(expected["blk.3.attn_q.weight"], vec![12_288, 5120]);
    }

    #[test]
    fn memory_admission_is_context_and_profile_specific() {
        let primary_4096 = memory_envelope(Qwen38GgufProfile::DynamicV3UdQ3KXl, 4096);
        assert_eq!(primary_4096.conservative_total_bytes, 15_074_348_096);
        assert_eq!(
            primary_4096.bytes_remaining_against_observed_free,
            923_775_936
        );
        assert_eq!(primary_4096.preflight_status, "admitted");

        let primary_8192 = memory_envelope(Qwen38GgufProfile::DynamicV3UdQ3KXl, 8192);
        assert_eq!(
            primary_8192.preflight_status,
            "fits_estimate_but_not_admitted_without_runtime_peak_and_parity"
        );
        let q4 = memory_envelope(Qwen38GgufProfile::Q4KM, 4096);
        assert_eq!(q4.preflight_status, "refused_full_cuda_residency");
    }

    #[test]
    fn profile_storage_inventories_are_exact() {
        assert_eq!(
            Qwen38GgufProfile::DynamicV3UdQ3KXl
                .expected_type_counts()
                .values()
                .sum::<usize>(),
            866
        );
        assert_eq!(
            Qwen38GgufProfile::Q3KM
                .expected_type_counts()
                .values()
                .sum::<usize>(),
            866
        );
        assert_eq!(
            Qwen38GgufProfile::Q4KM
                .expected_type_counts()
                .values()
                .sum::<usize>(),
            866
        );
    }
}
