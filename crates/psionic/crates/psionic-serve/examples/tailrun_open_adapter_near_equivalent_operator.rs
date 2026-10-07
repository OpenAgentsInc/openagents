use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use psionic_adapters::{
    AdapterArtifactFormat, AdapterArtifactIdentity, AdapterArtifactKind, AdapterResidencyMode,
    AdapterTargetFamily, LmHeadLoraAdapterArtifact,
};
use psionic_core::{QuantizationMode, TensorData};
use psionic_models::{GgufMetadataValue, GgufTensorType, TokenId, TokenSequence};
use psionic_runtime::BackendSelection;
use psionic_serve::{
    served_artifact_identity_for_decoder_model, CpuGgufTextGenerationService, GenerationOptions,
    GenerationRequest, TextGenerationExecutor,
};
use psionic_train::{
    open_adapter_pgolfish_config, open_adapter_pgolfish_samples, OpenAdapterPgolfishSampleSplit,
    PortableModelBundle, TrainingParameterGroupState, OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL,
};
use safetensors::{serialize_to_file, tensor::TensorView, Dtype as SafeTensorsDType};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const REPORT_SCHEMA_VERSION: &str = "psionic.tailrun_open_adapter_near_equivalent_report.v1";
const MANIFEST_SCHEMA_VERSION: &str = "psionic.tailrun_open_adapter_near_equivalent_manifest.v1";
const DEFAULT_SOURCE_REPORT: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/report.json";
const DEFAULT_SOURCE_BUNDLE: &str =
    "fixtures/apple_adapter/runs/tailrun_admitted_device_matrix_20260327b/m5_mlx/portable_bundle.safetensors";
const DEFAULT_OUTPUT_ROOT: &str =
    "fixtures/apple_adapter/promoted/tailrun_m5_near_equivalent_20260327";
const DEFAULT_PROMPT_TOKEN_ID: u32 = 7;
const DEFAULT_ANCHOR_SAMPLE_INDEX: usize = 1;

#[derive(Clone, Debug)]
struct Args {
    source_report: PathBuf,
    source_bundle: PathBuf,
    output_root: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
struct BenchmarkReport {
    host: String,
    backend_label: String,
    retained_run: RetainedRunReport,
}

#[derive(Clone, Debug, Deserialize)]
struct RetainedRunReport {
    steps_per_second: f64,
    source_tokens_per_second: f64,
    final_state_dict_digest: String,
}

#[derive(Clone, Debug, Serialize)]
struct NearEquivalentManifest {
    schema_version: String,
    bridge_kind: String,
    source_report_path: String,
    source_bundle_path: String,
    source_training_host: String,
    source_training_backend_label: String,
    source_training_steps_per_second: f64,
    source_training_source_tokens_per_second: f64,
    source_state_dict_digest: String,
    direct_inference_contract: String,
    served_inference_contract: String,
    strict_pgolf_promotion_disposition: String,
    strict_pgolf_promotion_reason: String,
    near_equivalent_promotion_disposition: String,
    near_equivalent_gate_reason: String,
    carrier_model_path: String,
    carrier_model_sha256: String,
    adapter_artifact_path: String,
    adapter_artifact_sha256: String,
    adapter_identity_digest: String,
    adapter_alpha: f32,
    prompt_token_id: u32,
    anchor_sample_id: String,
    expected_target_token_id: u32,
}

#[derive(Clone, Debug, Serialize)]
struct NearEquivalentReport {
    schema_version: String,
    source_report_path: String,
    source_bundle_path: String,
    output_root: String,
    carrier_model_path: String,
    adapter_artifact_path: String,
    prompt_token_id: u32,
    expected_target_token_id: u32,
    direct_inference_predicted_token_id: u32,
    baseline_served_output_tokens: Vec<u32>,
    overlay_served_output_tokens: Vec<u32>,
    baseline_served_output_text: String,
    overlay_served_output_text: String,
    adapter_binding_digest: String,
    runtime_support_level: String,
    claim_boundary: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    fs::create_dir_all(&args.output_root)?;

    let benchmark_report: BenchmarkReport =
        serde_json::from_slice(&fs::read(args.source_report.as_path())?)?;
    let bundle_bytes = fs::read(args.source_bundle.as_path())?;
    let bundle = PortableModelBundle::import_safetensors(bundle_bytes.as_slice())?;
    let config = open_adapter_pgolfish_config(
        OPEN_ADAPTER_MLX_METAL_BACKEND_LABEL,
        String::from("tailrun-open-adapter-near-equivalent"),
        String::from("tailrun.open_adapter.near_equivalent"),
        1,
    )?;
    let groups = bundle.to_training_groups()?;
    let lora_a = parameter_values(&groups, config.model.target.lora_a_group_id().as_str())?;
    let lora_b = parameter_values(&groups, config.model.target.lora_b_group_id().as_str())?;
    let anchor_sample = open_adapter_pgolfish_samples(
        "tailrun-near-equivalent",
        OpenAdapterPgolfishSampleSplit::Training,
    )?
    .into_iter()
    .nth(DEFAULT_ANCHOR_SAMPLE_INDEX)
    .ok_or("missing anchor sample")?;

    let carrier_model_path = args.output_root.join("carrier_model.gguf");
    write_prompt_aligned_dense_gguf(
        carrier_model_path.as_path(),
        DEFAULT_PROMPT_TOKEN_ID as usize,
        anchor_sample.hidden_state.as_slice(),
        config.model.vocab_size,
        config.model.hidden_size,
    )?;
    let carrier_model_bytes = fs::read(carrier_model_path.as_path())?;
    let carrier_model_sha256 = hex::encode(Sha256::digest(carrier_model_bytes.as_slice()));

    let mut service = CpuGgufTextGenerationService::from_gguf_path(carrier_model_path.as_path())?;
    let descriptor = service.model_descriptor().clone();
    let served_identity = served_artifact_identity_for_decoder_model(
        &descriptor,
        &BackendSelection::direct("cpu", None, vec![]),
    );

    let adapter_path = args.output_root.join("lm_head_lora_adapter.safetensors");
    write_lm_head_lora_adapter(
        adapter_path.as_path(),
        lora_a.as_slice(),
        lora_b.as_slice(),
        config.model.target.lora_rank,
        config.model.hidden_size,
        config.model.vocab_size,
    )?;
    let adapter_bytes = fs::read(adapter_path.as_path())?;
    let adapter_sha256 = hex::encode(Sha256::digest(adapter_bytes.as_slice()));
    let adapter_identity = AdapterArtifactIdentity::new(
        "tailrun-open-adapter-near-equivalent",
        "r1",
        AdapterArtifactKind::Lora,
        AdapterArtifactFormat::Safetensors,
        descriptor.model.model_id.clone(),
        descriptor.model.revision.clone(),
        served_identity.served_artifact_digest.clone(),
        adapter_sha256.clone(),
        QuantizationMode::None,
        AdapterTargetFamily::DecoderComposite,
        u64::try_from(
            config
                .model
                .target
                .lora_rank
                .saturating_mul(config.model.hidden_size + config.model.vocab_size),
        )
        .unwrap_or(u64::MAX),
    );
    let adapter = LmHeadLoraAdapterArtifact::from_safetensors_path(
        adapter_path.as_path(),
        adapter_identity.clone(),
        config.model.target.lora_alpha,
    )?;

    let mut direct_logits = vec![0.0_f32; config.model.vocab_size];
    adapter.apply_to_logits(
        anchor_sample.hidden_state.as_slice(),
        direct_logits.as_mut_slice(),
    )?;
    let direct_token_id = argmax(direct_logits.as_slice()) as u32;
    if direct_token_id != anchor_sample.target_token_id {
        return Err(format!(
            "direct near-equivalent inference predicted token {} instead of expected {}",
            direct_token_id, anchor_sample.target_token_id
        )
        .into());
    }

    let prompt_tokens = TokenSequence::new(vec![TokenId(DEFAULT_PROMPT_TOKEN_ID)]);
    let baseline = service.generate(&GenerationRequest::new_tokens(
        String::from("tailrun-near-equivalent-baseline"),
        descriptor.clone(),
        None,
        prompt_tokens.clone(),
        GenerationOptions::greedy(1),
    ))?;
    let binding = service.register_lm_head_lora_adapter(
        "tailrun-open-adapter-near-equivalent",
        adapter_path.as_path(),
        adapter_identity.clone(),
        config.model.target.lora_alpha,
        AdapterResidencyMode::HotSwapOverlay,
    )?;
    let overlay = service.generate(
        &GenerationRequest::new_tokens(
            String::from("tailrun-near-equivalent-overlay"),
            descriptor,
            None,
            prompt_tokens,
            GenerationOptions::greedy(1),
        )
        .with_adapter_serving(binding.clone()),
    )?;
    let overlay_token_id = overlay
        .output
        .tokens
        .as_slice()
        .first()
        .copied()
        .ok_or("overlay generation returned no tokens")?;
    if overlay_token_id.as_u32() != anchor_sample.target_token_id {
        return Err(format!(
            "served near-equivalent inference predicted token {} instead of expected {}",
            overlay_token_id.as_u32(),
            anchor_sample.target_token_id
        )
        .into());
    }

    let manifest = NearEquivalentManifest {
        schema_version: String::from(MANIFEST_SCHEMA_VERSION),
        bridge_kind: String::from("open_adapter_lm_head_lora_near_equivalent"),
        source_report_path: args.source_report.display().to_string(),
        source_bundle_path: args.source_bundle.display().to_string(),
        source_training_host: benchmark_report.host,
        source_training_backend_label: benchmark_report.backend_label,
        source_training_steps_per_second: benchmark_report.retained_run.steps_per_second,
        source_training_source_tokens_per_second: benchmark_report
            .retained_run
            .source_tokens_per_second,
        source_state_dict_digest: benchmark_report.retained_run.final_state_dict_digest,
        direct_inference_contract: String::from(
            "One retained LM-head LoRA adapter artifact is loaded directly and applied to the anchored hidden-state sample through `LmHeadLoraAdapterArtifact::apply_to_logits`.",
        ),
        served_inference_contract: String::from(
            "The same retained adapter artifact is bound into `CpuGgufTextGenerationService` over one prompt-aligned dense CPU GGUF carrier model and exercised through `GenerationRequest::new_tokens`.",
        ),
        strict_pgolf_promotion_disposition: String::from("refused"),
        strict_pgolf_promotion_reason: String::from(
            "The bounded home-device artifact is an open-adapter LM-head LoRA update, not a strict Parameter Golf promoted decoder bundle.",
        ),
        near_equivalent_promotion_disposition: String::from("allowed"),
        near_equivalent_gate_reason: String::from(
            "The retained artifact can be exercised honestly as a bounded near-equivalent by rebinding it to one prompt-aligned dense CPU GGUF carrier model in `psionic-serve` without pretending it is already the full PGOLF promoted family.",
        ),
        carrier_model_path: carrier_model_path.display().to_string(),
        carrier_model_sha256,
        adapter_artifact_path: adapter_path.display().to_string(),
        adapter_artifact_sha256: adapter_sha256,
        adapter_identity_digest: adapter_identity.stable_digest(),
        adapter_alpha: config.model.target.lora_alpha,
        prompt_token_id: DEFAULT_PROMPT_TOKEN_ID,
        anchor_sample_id: anchor_sample.sample_id.clone(),
        expected_target_token_id: anchor_sample.target_token_id,
    };
    let report = NearEquivalentReport {
        schema_version: String::from(REPORT_SCHEMA_VERSION),
        source_report_path: args.source_report.display().to_string(),
        source_bundle_path: args.source_bundle.display().to_string(),
        output_root: args.output_root.display().to_string(),
        carrier_model_path: carrier_model_path.display().to_string(),
        adapter_artifact_path: adapter_path.display().to_string(),
        prompt_token_id: DEFAULT_PROMPT_TOKEN_ID,
        expected_target_token_id: anchor_sample.target_token_id,
        direct_inference_predicted_token_id: direct_token_id,
        baseline_served_output_tokens: baseline
            .output
            .tokens
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        overlay_served_output_tokens: overlay
            .output
            .tokens
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        baseline_served_output_text: baseline.output.text,
        overlay_served_output_text: overlay.output.text,
        adapter_binding_digest: binding.served_adapter_digest,
        runtime_support_level: service.runtime_support().adapter_runtime.support_level,
        claim_boundary: String::from(
            "This retained near-equivalent proves that one bounded home-device-trained open-adapter artifact can be materialized into a direct-loadable LM-head LoRA safetensors file, can predict the anchored training token directly, and can be bound into `psionic-serve` over a prompt-aligned dense CPU GGUF carrier model. It does not claim that the artifact is already a strict Parameter Golf promoted decoder bundle, a production gpt-oss serving artifact, or a mixed-device promoted swarm output.",
        ),
    };

    fs::write(
        args.output_root.join("near_equivalent_manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        args.output_root.join("near_equivalent_report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "tailrun open-adapter near-equivalent completed: output_root={} target_token={} overlay_token={}",
        args.output_root.display(),
        anchor_sample.target_token_id,
        overlay_token_id.as_u32()
    );
    Ok(())
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut source_report = PathBuf::from(DEFAULT_SOURCE_REPORT);
    let mut source_bundle = PathBuf::from(DEFAULT_SOURCE_BUNDLE);
    let mut output_root = PathBuf::from(DEFAULT_OUTPUT_ROOT);

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--source-report" => {
                source_report =
                    PathBuf::from(args.next().ok_or("missing value after --source-report")?);
            }
            "--source-bundle" => {
                source_bundle =
                    PathBuf::from(args.next().ok_or("missing value after --source-bundle")?);
            }
            "--output-root" => {
                output_root =
                    PathBuf::from(args.next().ok_or("missing value after --output-root")?);
            }
            "--help" | "-h" => {
                println!(
                    "Usage: tailrun_open_adapter_near_equivalent_operator [--source-report <path>] [--source-bundle <path>] [--output-root <path>]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    Ok(Args {
        source_report,
        source_bundle,
        output_root,
    })
}

fn parameter_values(
    groups: &[TrainingParameterGroupState],
    group_id: &str,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let group = groups
        .iter()
        .find(|group| group.group_id == group_id)
        .ok_or_else(|| format!("missing training group `{group_id}`"))?;
    match &group.parameter.data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.clone()),
        other => Err(format!(
            "training group `{group_id}` uses unsupported tensor data `{other:?}`"
        )
        .into()),
    }
}

fn write_lm_head_lora_adapter(
    path: &Path,
    lora_a: &[f32],
    lora_b: &[f32],
    rank: usize,
    hidden_size: usize,
    vocab_size: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let lora_a_bytes = encode_f32_bytes(lora_a);
    let lora_b_bytes = encode_f32_bytes(lora_b);
    let mut tensors = BTreeMap::new();
    tensors.insert(
        String::from("lm_head.lora_A.weight"),
        TensorView::new(
            SafeTensorsDType::F32,
            vec![rank, hidden_size],
            &lora_a_bytes,
        )?,
    );
    tensors.insert(
        String::from("lm_head.lora_B.weight"),
        TensorView::new(SafeTensorsDType::F32, vec![vocab_size, rank], &lora_b_bytes)?,
    );
    serialize_to_file(tensors, None, path)?;
    Ok(())
}

fn write_prompt_aligned_dense_gguf(
    path: &Path,
    prompt_token_index: usize,
    anchor_hidden: &[f32],
    vocab_size: usize,
    hidden_size: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let head_count = 8usize;
    let kv_head_count = 4usize;
    let head_width = hidden_size / head_count.max(1);
    let mut metadata = vec![
        (
            String::from("general.architecture"),
            GgufMetadataValue::String(String::from("llama")),
        ),
        (
            String::from("general.name"),
            GgufMetadataValue::String(String::from("tailrun prompt-aligned dense carrier")),
        ),
        (
            String::from("llama.context_length"),
            GgufMetadataValue::U32(32),
        ),
        (
            String::from("llama.embedding_length"),
            GgufMetadataValue::U32(u32::try_from(hidden_size)?),
        ),
        (
            String::from("llama.feed_forward_length"),
            GgufMetadataValue::U32(8),
        ),
        (String::from("llama.block_count"), GgufMetadataValue::U32(1)),
        (
            String::from("llama.attention.head_count"),
            GgufMetadataValue::U32(u32::try_from(head_count)?),
        ),
        (
            String::from("llama.attention.head_count_kv"),
            GgufMetadataValue::U32(u32::try_from(kv_head_count)?),
        ),
        (
            String::from("llama.attention.layer_norm_rms_epsilon"),
            GgufMetadataValue::F32(1e-5),
        ),
        (
            String::from("llama.rope.freq_base"),
            GgufMetadataValue::F32(10_000.0),
        ),
        (
            String::from("tokenizer.ggml.model"),
            GgufMetadataValue::String(String::from("llama")),
        ),
        (
            String::from("tokenizer.ggml.tokens"),
            GgufMetadataValue::Array(
                (0..vocab_size)
                    .map(|token_id| GgufMetadataValue::String(format!("tok{token_id:04}")))
                    .collect(),
            ),
        ),
        (
            String::from("tokenizer.ggml.bos_token_id"),
            GgufMetadataValue::U32(1),
        ),
        (
            String::from("tokenizer.ggml.eos_token_id"),
            GgufMetadataValue::U32(2),
        ),
        (
            String::from("tokenizer.ggml.unknown_token_id"),
            GgufMetadataValue::U32(0),
        ),
        (
            String::from("tokenizer.ggml.add_bos_token"),
            GgufMetadataValue::Bool(false),
        ),
        (
            String::from("tokenizer.ggml.add_eos_token"),
            GgufMetadataValue::Bool(false),
        ),
    ];
    metadata.push((
        String::from("general.alignment"),
        GgufMetadataValue::U32(32),
    ));

    let anchor_rms = rms(anchor_hidden);
    let mut token_embeddings = vec![0.0_f32; vocab_size.saturating_mul(hidden_size)];
    let prompt_row_start = prompt_token_index.saturating_mul(hidden_size);
    let prompt_row_end = prompt_row_start.saturating_add(hidden_size);
    token_embeddings[prompt_row_start..prompt_row_end].copy_from_slice(anchor_hidden);

    let tensors = vec![
        dense_tensor(
            "token_embd.weight",
            vec![vocab_size, hidden_size],
            token_embeddings,
        ),
        dense_tensor(
            "output_norm.weight",
            vec![hidden_size],
            vec![anchor_rms; hidden_size],
        ),
        dense_tensor(
            "output.weight",
            vec![vocab_size, hidden_size],
            vec![0.0; vocab_size.saturating_mul(hidden_size)],
        ),
        dense_tensor(
            "blk.0.attn_norm.weight",
            vec![hidden_size],
            vec![1.0; hidden_size],
        ),
        dense_tensor(
            "blk.0.attn_q.weight",
            vec![hidden_size, hidden_size],
            vec![0.0; hidden_size.saturating_mul(hidden_size)],
        ),
        dense_tensor(
            "blk.0.attn_k.weight",
            vec![kv_head_count.saturating_mul(head_width), hidden_size],
            vec![
                0.0;
                kv_head_count
                    .saturating_mul(head_width)
                    .saturating_mul(hidden_size)
            ],
        ),
        dense_tensor(
            "blk.0.attn_v.weight",
            vec![kv_head_count.saturating_mul(head_width), hidden_size],
            vec![
                0.0;
                kv_head_count
                    .saturating_mul(head_width)
                    .saturating_mul(hidden_size)
            ],
        ),
        dense_tensor(
            "blk.0.attn_output.weight",
            vec![hidden_size, hidden_size],
            vec![0.0; hidden_size.saturating_mul(hidden_size)],
        ),
        dense_tensor(
            "blk.0.ffn_gate.weight",
            vec![8, hidden_size],
            vec![0.0; 8 * hidden_size],
        ),
        dense_tensor(
            "blk.0.ffn_down.weight",
            vec![hidden_size, 8],
            vec![0.0; hidden_size * 8],
        ),
        dense_tensor(
            "blk.0.ffn_up.weight",
            vec![8, hidden_size],
            vec![0.0; 8 * hidden_size],
        ),
        dense_tensor(
            "blk.0.ffn_norm.weight",
            vec![hidden_size],
            vec![1.0; hidden_size],
        ),
    ];

    write_test_gguf(path, metadata.as_slice(), tensors.as_slice())
}

fn argmax(values: &[f32]) -> usize {
    let mut best_index = 0usize;
    let mut best_value = f32::NEG_INFINITY;
    for (index, value) in values.iter().copied().enumerate() {
        if value > best_value {
            best_value = value;
            best_index = index;
        }
    }
    best_index
}

fn rms(values: &[f32]) -> f32 {
    let mean_square =
        values.iter().map(|value| value * value).sum::<f32>() / values.len().max(1) as f32;
    mean_square.sqrt().max(1e-6)
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

#[derive(Clone, Debug)]
struct TestGgufTensor {
    name: String,
    shape: Vec<usize>,
    tensor_type: GgufTensorType,
    bytes: Vec<u8>,
}

impl TestGgufTensor {
    fn new(
        name: impl Into<String>,
        shape: Vec<usize>,
        tensor_type: GgufTensorType,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            name: name.into(),
            shape,
            tensor_type,
            bytes,
        }
    }
}

fn dense_tensor(name: &str, shape: Vec<usize>, values: Vec<f32>) -> TestGgufTensor {
    TestGgufTensor::new(
        name,
        shape,
        GgufTensorType::F32,
        encode_f32_bytes(values.as_slice()),
    )
}

fn write_test_gguf(
    path: &Path,
    metadata: &[(String, GgufMetadataValue)],
    tensors: &[TestGgufTensor],
) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(path, build_test_gguf(metadata, tensors)?)?;
    Ok(())
}

fn build_test_gguf(
    metadata: &[(String, GgufMetadataValue)],
    tensors: &[TestGgufTensor],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let alignment = metadata
        .iter()
        .find(|(key, _)| key == "general.alignment")
        .and_then(|(_, value)| match value {
            GgufMetadataValue::U64(value) => Some(*value as usize),
            GgufMetadataValue::U32(value) => Some(*value as usize),
            _ => None,
        })
        .unwrap_or(32)
        .max(1);

    let mut bytes = Vec::new();
    bytes.extend(b"GGUF");
    push_u32(&mut bytes, 3);
    push_u64(&mut bytes, u64::try_from(tensors.len())?);
    push_u64(&mut bytes, u64::try_from(metadata.len())?);

    for (key, value) in metadata {
        push_gguf_string(&mut bytes, key)?;
        push_u32(&mut bytes, gguf_metadata_value_type(value));
        push_gguf_value(&mut bytes, value)?;
    }

    let mut next_offset = 0usize;
    let mut tensor_offsets = Vec::with_capacity(tensors.len());
    for tensor in tensors {
        tensor_offsets.push(next_offset);
        next_offset = align_usize(next_offset + tensor.bytes.len(), alignment);
    }

    for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
        push_gguf_string(&mut bytes, tensor.name.as_str())?;
        push_u32(&mut bytes, u32::try_from(tensor.shape.len())?);
        for dimension in tensor.shape.iter().rev() {
            push_u64(&mut bytes, u64::try_from(*dimension)?);
        }
        push_u32(&mut bytes, gguf_tensor_type_code(tensor.tensor_type));
        push_u64(&mut bytes, u64::try_from(*offset)?);
    }

    let tensor_data_offset = align_usize(bytes.len(), alignment);
    bytes.resize(tensor_data_offset, 0);

    for (tensor, offset) in tensors.iter().zip(&tensor_offsets) {
        let start = tensor_data_offset + offset;
        if bytes.len() < start {
            bytes.resize(start, 0);
        }
        bytes.extend_from_slice(tensor.bytes.as_slice());
        bytes.resize(align_usize(bytes.len(), alignment), 0);
    }

    Ok(bytes)
}

fn align_usize(value: usize, alignment: usize) -> usize {
    let remainder = value % alignment;
    if remainder == 0 {
        value
    } else {
        value + alignment - remainder
    }
}

fn gguf_metadata_value_type(value: &GgufMetadataValue) -> u32 {
    match value {
        GgufMetadataValue::U8(_) => 0,
        GgufMetadataValue::I8(_) => 1,
        GgufMetadataValue::U16(_) => 2,
        GgufMetadataValue::I16(_) => 3,
        GgufMetadataValue::U32(_) => 4,
        GgufMetadataValue::I32(_) => 5,
        GgufMetadataValue::F32(_) => 6,
        GgufMetadataValue::Bool(_) => 7,
        GgufMetadataValue::String(_) => 8,
        GgufMetadataValue::Array(_) => 9,
        GgufMetadataValue::U64(_) => 10,
        GgufMetadataValue::I64(_) => 11,
        GgufMetadataValue::F64(_) => 12,
    }
}

fn gguf_tensor_type_code(tensor_type: GgufTensorType) -> u32 {
    match tensor_type {
        GgufTensorType::F32 => 0,
        other => panic!("unsupported synthetic gguf tensor type: {other:?}"),
    }
}

fn push_gguf_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    push_u64(bytes, u64::try_from(value.len())?);
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn push_gguf_value(
    bytes: &mut Vec<u8>,
    value: &GgufMetadataValue,
) -> Result<(), Box<dyn std::error::Error>> {
    match value {
        GgufMetadataValue::U8(value) => bytes.push(*value),
        GgufMetadataValue::I8(value) => bytes.push(value.to_le_bytes()[0]),
        GgufMetadataValue::U16(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::I16(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::U32(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::I32(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::U64(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::I64(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::F32(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::F64(value) => bytes.extend(value.to_le_bytes()),
        GgufMetadataValue::Bool(value) => bytes.push(u8::from(*value)),
        GgufMetadataValue::String(value) => push_gguf_string(bytes, value)?,
        GgufMetadataValue::Array(values) => {
            let value_type = values.first().map_or(4, gguf_metadata_value_type);
            push_u32(bytes, value_type);
            push_u64(bytes, u64::try_from(values.len())?);
            for value in values {
                push_gguf_value(bytes, value)?;
            }
        }
    }
    Ok(())
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}
