#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use bytemuck::cast_slice;
use futures::channel::oneshot;
use wgpu::util::DeviceExt;
use wasm_bindgen_futures::spawn_local;
use js_sys;
use web_sys;

use crate::gguf_web::{
    fetch_and_parse_index, fetch_range, fetch_range_with_total, GgufIndex, GgufTensor,
};
use crate::gptoss_tokenizer::GptOssTokenizer;
use crate::gptoss_viz::{push_gptoss_event, GptOssInferenceTelemetry, GptOssTelemetry, StageStatus};
use crate::state::{AppState, GpuContext};

const DEFAULT_METADATA_BYTES: u64 = 16 * 1024 * 1024;
const MAX_METADATA_ATTEMPTS: usize = 3;
const LOAD_CHUNK_BYTES: u64 = 8 * 1024 * 1024;
const PROGRESS_STEP_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_GGUF_URL: &str =
    "https://huggingface.co/openai/gpt-oss-20b/resolve/main/gpt-oss-20b-Q8_0.gguf";
const LOCAL_GGUF_URL: &str = "http://localhost:9898/gpt-oss-20b-Q8_0.gguf";
const CURRENT_DATE: &str = "2026-01-02";
const DEFAULT_USER_PROMPT: &str = "Give me one sentence about what GPT-OSS can do.";
const Q8_0_BLOCK_BYTES: usize = 34;
const Q8_0_BLOCK_VALUES: usize = 32;

pub(crate) struct GptOssRuntime {
    pub(crate) gguf_url: String,
    pub(crate) gpu: GpuContext,
    pub(crate) index: Option<GgufIndex>,
}

impl GptOssRuntime {
    pub(crate) fn new(gguf_url: String, gpu: GpuContext) -> Self {
        Self {
            gguf_url,
            gpu,
            index: None,
        }
    }

    pub(crate) async fn load_index(
        &mut self,
        initial_bytes: u64,
        max_attempts: usize,
    ) -> Result<&GgufIndex, String> {
        let index = fetch_and_parse_index(&self.gguf_url, initial_bytes, max_attempts).await?;
        self.index = Some(index);
        Ok(self.index.as_ref().expect("index set"))
    }

    pub(crate) async fn read_tensor_slice(
        &self,
        tensor: &GgufTensor,
        len: usize,
    ) -> Result<Vec<u8>, String> {
        let bytes = fetch_range(&self.gguf_url, tensor.absolute_offset, len as u64).await?;
        Ok(bytes)
    }
}

pub(crate) fn start_gptoss_load(state: Rc<RefCell<AppState>>) {
    let gguf_url = read_query_param("gguf")
        .filter(|url| !url.is_empty())
        .unwrap_or_else(default_gguf_url);

    {
        let Ok(mut guard) = state.try_borrow_mut() else {
            return;
        };
        if guard.gptoss.load_active {
            return;
        }
        reset_gptoss_state(&mut guard.gptoss);
        guard.gptoss.load_active = true;
        guard.gptoss.load_error = None;
        guard.gptoss.load_url = Some(gguf_url.clone());
    }

    let state_clone = state.clone();
    spawn_local(async move {
        if let Err(err) = run_gptoss_load(state_clone.clone(), gguf_url).await {
            if let Ok(mut guard) = state_clone.try_borrow_mut() {
                guard.gptoss.load_active = false;
                guard.gptoss.load_error = Some(err.clone());
            }
            emit_load_stage(
                &state_clone,
                "load_failed",
                StageStatus::Failed,
                Some(err),
                None,
                None,
            );
        }
    });
}

async fn run_gptoss_load(state: Rc<RefCell<AppState>>, gguf_url: String) -> Result<(), String> {
    emit_load_stage(
        &state,
        "load_start",
        StageStatus::Started,
        Some(format!("url={}", gguf_url)),
        None,
        None,
    );

    emit_load_stage(
        &state,
        "gguf_parse",
        StageStatus::Started,
        Some("reading gguf header".to_string()),
        None,
        None,
    );

    let index = Rc::new(
        fetch_and_parse_index(&gguf_url, DEFAULT_METADATA_BYTES, MAX_METADATA_ATTEMPTS).await?,
    );
    emit_load_stage(
        &state,
        "gguf_parse",
        StageStatus::Completed,
        Some(format!(
            "tensors={} v{} data_offset={}",
            index.tensors.len(),
            index.version,
            format_bytes(index.tensor_data_offset)
        )),
        None,
        None,
    );

    emit_tensor_scan(&state, index.as_ref(), 18);

    let tokenizer = build_tokenizer(&state, index.as_ref())?;
    let _prompt_tokens = encode_prompt(&state, &tokenizer)?;

    let gpu_context = state.borrow().gpu_context.clone();
    if let Some(gpu) = gpu_context.clone() {
        let gguf = gguf_url.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        spawn_local(async move {
            if let Err(err) = run_q8_0_probe(&state_clone, &gguf, index_clone.as_ref(), &gpu)
                .await
            {
                emit_inference_stage(
                    &state_clone,
                    "q8_0_probe",
                    StageStatus::Failed,
                    None,
                    None,
                    Some(err),
                );
            }
        });
    }

    let (_probe, total) = fetch_range_with_total(&gguf_url, 0, 1).await?;
    let total_bytes = total.ok_or_else(|| {
        "range response missing Content-Range total size".to_string()
    })?;

    emit_load_stage(
        &state,
        "weights_fetch",
        StageStatus::Started,
        Some(format!("total={}", format_bytes(total_bytes))),
        Some(0),
        Some(total_bytes),
    );

    let mut offset = 0u64;
    let mut loaded = 0u64;
    let mut next_progress = PROGRESS_STEP_BYTES;
    let mut chunk_idx = 0u64;
    let mut tensor_cursor = tensor_start_cursor(index.as_ref());
    let mut tensor_emitted = 0usize;

    while offset < total_bytes {
        let len = (total_bytes - offset).min(LOAD_CHUNK_BYTES);
        let chunk = fetch_range(&gguf_url, offset, len).await?;
        loaded = loaded.saturating_add(chunk.len() as u64);
        offset = offset.saturating_add(len);
        chunk_idx = chunk_idx.saturating_add(1);

        if loaded >= next_progress || loaded >= total_bytes {
            emit_load_stage(
                &state,
                "weights_fetch",
                StageStatus::Progress,
                Some(format!(
                    "chunk={} offset={}",
                    chunk_idx,
                    format_bytes(offset)
                )),
                Some(loaded),
                Some(total_bytes),
            );
            next_progress = next_progress.saturating_add(PROGRESS_STEP_BYTES);
        }

        while let Some((next_offset, name)) = tensor_cursor.first().cloned() {
            if offset < next_offset {
                break;
            }
            tensor_cursor.remove(0);
            tensor_emitted = tensor_emitted.saturating_add(1);
            if tensor_emitted % 6 == 0 || tensor_emitted <= 12 {
                emit_load_stage(
                    &state,
                    "tensor_scan",
                    StageStatus::Progress,
                    Some(name),
                    Some(loaded),
                    Some(total_bytes),
                );
            }
        }
    }

    emit_load_stage(
        &state,
        "weights_fetch",
        StageStatus::Completed,
        Some(format!("loaded={}", format_bytes(loaded))),
        Some(loaded),
        Some(total_bytes),
    );

    emit_load_stage(
        &state,
        "load_complete",
        StageStatus::Completed,
        None,
        Some(loaded),
        Some(total_bytes),
    );

    if let Some(gpu) = gpu_context {
        emit_inference_stage(
            &state,
            "blk0_attention",
            StageStatus::Started,
            None,
            None,
            Some("after_load".to_string()),
        );
        if let Err(err) = run_block0_attention_probe(&state, &gguf_url, index.as_ref(), &gpu).await {
            emit_inference_stage(
                &state,
                "blk0_attention",
                StageStatus::Failed,
                None,
                None,
                Some(err),
            );
        } else {
            emit_inference_stage(
                &state,
                "blk0_attention",
                StageStatus::Completed,
                None,
                None,
                Some("ok".to_string()),
            );
        }
    }

    if let Ok(mut guard) = state.try_borrow_mut() {
        guard.gptoss.load_active = false;
    }
    Ok(())
}

fn reset_gptoss_state(state: &mut crate::state::GptOssVizState) {
    state.load_stages.clear();
    state.inference_stages.clear();
    state.events.clear();
    state.token_stream.clear();
    state.top_k.clear();
    state.tokens_per_sec = None;
    state.entropy = None;
    state.memory_usage = None;
    state.cache_status.clear();
    state.start_ts_ms = None;
}

fn emit_tensor_scan(state: &Rc<RefCell<AppState>>, index: &GgufIndex, limit: usize) {
    for (idx, tensor) in index.tensors.iter().take(limit).enumerate() {
        emit_load_stage(
            state,
            "tensor_index",
            StageStatus::Progress,
            Some(format!("{}: {}", idx + 1, tensor.name)),
            None,
            None,
        );
    }
}

fn emit_load_stage(
    state: &Rc<RefCell<AppState>>,
    stage: &str,
    status: StageStatus,
    detail: Option<String>,
    bytes: Option<u64>,
    total_bytes: Option<u64>,
) {
    push_gptoss_event(
        state,
        GptOssTelemetry::LoadStage {
            stage: stage.to_string(),
            status,
            detail,
            bytes,
            total_bytes,
            ts_ms: Some(now_ms()),
        },
    );
}

fn emit_inference_stage(
    state: &Rc<RefCell<AppState>>,
    stage: &str,
    status: StageStatus,
    step: Option<usize>,
    total_steps: Option<usize>,
    detail: Option<String>,
) {
    push_gptoss_event(
        state,
        GptOssTelemetry::InferenceStage {
            stage: stage.to_string(),
            status,
            step,
            total_steps,
            detail,
            ts_ms: Some(now_ms()),
        },
    );
}

fn emit_inference_event(
    state: &Rc<RefCell<AppState>>,
    event: GptOssInferenceTelemetry,
) {
    push_gptoss_event(
        state,
        GptOssTelemetry::InferenceEvent {
            event,
            ts_ms: Some(now_ms()),
        },
    );
}

fn build_tokenizer(
    state: &Rc<RefCell<AppState>>,
    index: &GgufIndex,
) -> Result<GptOssTokenizer, String> {
    emit_load_stage(
        state,
        "tokenizer_load",
        StageStatus::Started,
        Some("building BPE".to_string()),
        None,
        None,
    );

    let Some(tokenizer_meta) = index.metadata.tokenizer.clone() else {
        let err = "gguf tokenizer metadata missing".to_string();
        emit_load_stage(
            state,
            "tokenizer_load",
            StageStatus::Failed,
            Some(err.clone()),
            None,
            None,
        );
        return Err(err);
    };

    let token_count = tokenizer_meta.tokens.len();
    let merges_len = tokenizer_meta.merges.len();
    let model = tokenizer_meta
        .model
        .as_deref()
        .unwrap_or("-");
    let pre = tokenizer_meta
        .pre
        .as_deref()
        .unwrap_or("-");
    let chat_len = tokenizer_meta
        .chat_template
        .as_ref()
        .map(|value| value.len())
        .unwrap_or(0);
    let bos = tokenizer_meta
        .bos_token_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let eos = tokenizer_meta
        .eos_token_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let pad = tokenizer_meta
        .pad_token_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());

    let tokenizer = match GptOssTokenizer::from_gguf(tokenizer_meta.clone()) {
        Ok(tok) => tok,
        Err(err) => {
            emit_load_stage(
                state,
                "tokenizer_load",
                StageStatus::Failed,
                Some(err.clone()),
                None,
                None,
            );
            return Err(err);
        }
    };

    emit_load_stage(
        state,
        "tokenizer_load",
        StageStatus::Completed,
        Some(format!(
            "vocab={token_count} merges={merges_len} model={model} pre={pre} template={chat_len}b bos={bos} eos={eos} pad={pad}",
        )),
        None,
        None,
    );
    Ok(tokenizer)
}

fn encode_prompt(
    state: &Rc<RefCell<AppState>>,
    tokenizer: &GptOssTokenizer,
) -> Result<Vec<u32>, String> {
    let user_prompt = read_query_param("prompt")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_user_prompt);
    let prompt = build_harmony_prompt(&user_prompt);

    emit_inference_stage(
        state,
        "prompt_encode",
        StageStatus::Started,
        Some(0),
        None,
        Some(format!("chars={}", prompt.len())),
    );

    let tokens = tokenizer.encode_with_special_tokens(&prompt)?;
    let total = tokens.len();

    emit_inference_stage(
        state,
        "prompt_encode",
        StageStatus::Completed,
        Some(total),
        Some(total),
        Some(format!("tokens={total}")),
    );

    Ok(tokens)
}

fn now_ms() -> u64 {
    js_sys::Date::now().max(0.0) as u64
}

fn read_query_param(key: &str) -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    params.get(key)
}

fn default_gguf_url() -> String {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return DEFAULT_GGUF_URL.to_string(),
    };
    let host = window.location().hostname().ok();
    let local = matches!(host.as_deref(), Some("localhost") | Some("127.0.0.1"));
    if local {
        LOCAL_GGUF_URL.to_string()
    } else {
        DEFAULT_GGUF_URL.to_string()
    }
}

fn default_user_prompt() -> String {
    DEFAULT_USER_PROMPT.to_string()
}

fn build_harmony_prompt(user_prompt: &str) -> String {
    let system_prompt = format!(
        "You are ChatGPT, a large language model trained by OpenAI.\n\
Knowledge cutoff: 2024-06\n\
Current date: {CURRENT_DATE}\n\n\
Reasoning: low\n\n\
# Valid channels: analysis, commentary, final. Channel must be included for every message.\n\
Calls to these tools must go to the commentary channel: 'functions'."
    );

    let mut prompt = String::new();
    prompt.push_str("<|start|>system<|message|>");
    prompt.push_str(&system_prompt);
    prompt.push_str("<|end|><|start|>user<|message|>");
    prompt.push_str(user_prompt);
    prompt.push_str("<|end|><|start|>assistant");
    prompt
}

fn tensor_start_cursor(index: &GgufIndex) -> Vec<(u64, String)> {
    let mut entries = index
        .tensors
        .iter()
        .map(|tensor| (tensor.absolute_offset, tensor.name.clone()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.0);
    entries
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1}GB", bytes as f32 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1}MB", bytes as f32 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1}KB", bytes as f32 / 1_000.0)
    } else {
        format!("{bytes}B")
    }
}

async fn run_q8_0_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &str,
    index: &GgufIndex,
    gpu: &GpuContext,
) -> Result<(), String> {
    let tensor = index
        .tensors
        .iter()
        .find(|t| t.name == "output.weight")
        .or_else(|| index.tensors.iter().find(|t| t.ggml_type == 8))
        .ok_or_else(|| "no Q8_0 tensor found".to_string())?;

    let dims = &tensor.dims;
    let mut n = dims.get(0).copied().unwrap_or(64) as usize;
    let mut k = dims.get(1).copied().unwrap_or(128) as usize;
    n = n.min(64).max(1);
    k = k.min(128).max(1);
    let mut values = k * n;
    if values % Q8_0_BLOCK_VALUES != 0 {
        let rem = values % Q8_0_BLOCK_VALUES;
        if n > rem {
            n -= rem;
        }
        values = k * n;
    }
    if values == 0 || values % Q8_0_BLOCK_VALUES != 0 {
        return Err("invalid q8_0 probe shape".to_string());
    }

    let blocks = values / Q8_0_BLOCK_VALUES;
    let bytes_needed = blocks * Q8_0_BLOCK_BYTES;
    if bytes_needed as u64 > tensor.nbytes {
        return Err("q8_0 probe slice exceeds tensor size".to_string());
    }

    emit_inference_stage(
        state,
        "q8_0_probe",
        StageStatus::Started,
        None,
        None,
        Some(format!("tensor={}", tensor.name)),
    );

    let mut quant = fetch_range(gguf_url, tensor.absolute_offset, bytes_needed as u64).await?;
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }

    let x = build_input(k);
    let weights = dequant_q8_0(&quant, values)?;
    let y_cpu = matmul_cpu(&weights, &x, k, n);

    let y_gpu = gpu_matmul_q8_0(&quant, &x, k, n, gpu).await?;
    let (max_abs, mean_abs) = diff_stats(&y_cpu, &y_gpu);

    let gpu_bytes = quant.len()
        + x.len() * std::mem::size_of::<f32>()
        + y_gpu.len() * std::mem::size_of::<f32>();
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::MemoryUsage {
            gpu_allocated: gpu_bytes,
            cache_total: 0,
            activations: 0,
        },
    );

    emit_inference_stage(
        state,
        "q8_0_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!("max_abs={max_abs:.4} mean_abs={mean_abs:.4}")),
    );

    Ok(())
}

async fn run_block0_attention_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &str,
    index: &GgufIndex,
    gpu: &GpuContext,
) -> Result<(), String> {
    let token_embd = find_tensor(index, "token_embd.weight")?;
    let attn_norm = find_tensor(index, "blk.0.attn_norm.weight")?;
    let attn_q_w = find_tensor(index, "blk.0.attn_q.weight")?;
    let attn_q_b = find_tensor(index, "blk.0.attn_q.bias")?;
    let attn_k_w = find_tensor(index, "blk.0.attn_k.weight")?;
    let attn_k_b = find_tensor(index, "blk.0.attn_k.bias")?;
    let attn_v_w = find_tensor(index, "blk.0.attn_v.weight")?;
    let attn_v_b = find_tensor(index, "blk.0.attn_v.bias")?;
    let attn_out_w = find_tensor(index, "blk.0.attn_output.weight")?;
    let attn_out_b = find_tensor(index, "blk.0.attn_output.bias")?;

    let token_row = fetch_q8_0_row(gguf_url, token_embd, 0).await?;
    let attn_norm_w = fetch_f32_tensor(gguf_url, attn_norm).await?;
    let mut hidden = rms_norm(&token_row, &attn_norm_w, 1e-5)?;

    emit_inference_stage(
        state,
        "blk0_qkv",
        StageStatus::Started,
        None,
        None,
        Some("qkv".to_string()),
    );

    let q = matmul_q8_0_with_bias(gguf_url, attn_q_w, attn_q_b, &hidden, gpu).await?;
    let k = matmul_q8_0_with_bias(gguf_url, attn_k_w, attn_k_b, &hidden, gpu).await?;
    let v = matmul_q8_0_with_bias(gguf_url, attn_v_w, attn_v_b, &hidden, gpu).await?;

    emit_inference_stage(
        state,
        "blk0_qkv",
        StageStatus::Completed,
        None,
        None,
        Some(format!("q={} k={} v={}", q.len(), k.len(), v.len())),
    );

    let attn_out = v;
    let attn_proj = matmul_q8_0_with_bias(gguf_url, attn_out_w, attn_out_b, &attn_out, gpu).await?;
    for (out, base) in hidden.iter_mut().zip(attn_proj.iter()) {
        *out += *base;
    }

    let attention_norm = l2_norm(&attn_proj);
    let output_norm = l2_norm(&hidden);
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::LayerActivation {
            layer: 0,
            attention_norm,
            mlp_norm: 0.0,
            output_norm,
        },
    );

    Ok(())
}

fn build_input(k: usize) -> Vec<f32> {
    let mut x = Vec::with_capacity(k);
    for i in 0..k {
        let step = (i % 13) as f32 - 6.0;
        x.push(step * 0.01);
    }
    x
}

fn find_tensor<'a>(index: &'a GgufIndex, name: &str) -> Result<&'a GgufTensor, String> {
    index
        .tensors
        .iter()
        .find(|tensor| tensor.name == name)
        .ok_or_else(|| format!("tensor not found: {name}"))
}

async fn fetch_f32_tensor(gguf_url: &str, tensor: &GgufTensor) -> Result<Vec<f32>, String> {
    if tensor.ggml_type != 0 {
        return Err(format!(
            "tensor {} is {}, expected F32",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let bytes = fetch_range(gguf_url, tensor.absolute_offset, tensor.nbytes).await?;
    if bytes.len() % 4 != 0 {
        return Err(format!("tensor {} f32 byte len mismatch", tensor.name));
    }
    let mut floats = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        floats.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(floats)
}

async fn fetch_q8_0_row(
    gguf_url: &str,
    tensor: &GgufTensor,
    row: usize,
) -> Result<Vec<f32>, String> {
    if tensor.ggml_type != 8 {
        return Err(format!(
            "tensor {} is {}, expected Q8_0",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let dims = &tensor.dims;
    let rows = dims.get(0).copied().unwrap_or(0) as usize;
    let cols = dims.get(1).copied().unwrap_or(0) as usize;
    if row >= rows || cols == 0 {
        return Err(format!("row {row} out of range for {}", tensor.name));
    }
    if cols % Q8_0_BLOCK_VALUES != 0 {
        return Err("q8_0 row cols not divisible by block size".to_string());
    }
    let blocks_per_row = cols / Q8_0_BLOCK_VALUES;
    let row_bytes = blocks_per_row * Q8_0_BLOCK_BYTES;
    let offset = tensor
        .absolute_offset
        .saturating_add((row_bytes * row) as u64);
    let bytes = fetch_range(gguf_url, offset, row_bytes as u64).await?;
    let values = cols;
    let mut quant = bytes;
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }
    dequant_q8_0(&quant, values)
}

async fn matmul_q8_0_with_bias(
    gguf_url: &str,
    weight: &GgufTensor,
    bias: &GgufTensor,
    input: &[f32],
    gpu: &GpuContext,
) -> Result<Vec<f32>, String> {
    if weight.ggml_type != 8 {
        return Err(format!(
            "tensor {} is {}, expected Q8_0",
            weight.name, weight.ggml_type_name
        ));
    }
    let dims = &weight.dims;
    let n = dims.get(0).copied().unwrap_or(0) as usize;
    let k = dims.get(1).copied().unwrap_or(0) as usize;
    if input.len() != k || n == 0 {
        return Err(format!(
            "matmul shape mismatch for {} (k={}, n={}, input={})",
            weight.name,
            k,
            n,
            input.len()
        ));
    }
    let mut quant = fetch_range(gguf_url, weight.absolute_offset, weight.nbytes).await?;
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }
    let mut out = gpu_matmul_q8_0(&quant, input, k, n, gpu).await?;
    let bias_vals = fetch_f32_tensor(gguf_url, bias).await?;
    if bias_vals.len() == out.len() {
        for (o, b) in out.iter_mut().zip(bias_vals.iter()) {
            *o += *b;
        }
    }
    Ok(out)
}

fn rms_norm(input: &[f32], weight: &[f32], eps: f32) -> Result<Vec<f32>, String> {
    if input.len() != weight.len() {
        return Err("rms_norm shape mismatch".to_string());
    }
    let mut sum_sq = 0.0f32;
    for v in input {
        sum_sq += v * v;
    }
    let mean = sum_sq / input.len().max(1) as f32;
    let inv = (mean + eps).sqrt().recip();
    let mut out = Vec::with_capacity(input.len());
    for (v, w) in input.iter().zip(weight.iter()) {
        out.push(v * inv * w);
    }
    Ok(out)
}

fn l2_norm(values: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for v in values {
        sum += v * v;
    }
    sum.sqrt()
}

fn dequant_q8_0(data: &[u8], values: usize) -> Result<Vec<f32>, String> {
    if values % Q8_0_BLOCK_VALUES != 0 {
        return Err("value count not divisible by Q8_0 block size".to_string());
    }
    let blocks = values / Q8_0_BLOCK_VALUES;
    let needed = blocks * Q8_0_BLOCK_BYTES;
    if data.len() < needed {
        return Err(format!(
            "insufficient Q8_0 data: need {needed}, have {}",
            data.len()
        ));
    }

    let mut out = vec![0.0f32; values];
    for block in 0..blocks {
        let base = block * Q8_0_BLOCK_BYTES;
        let scale_bits = u16::from_le_bytes([data[base], data[base + 1]]);
        let scale = f16_to_f32(scale_bits);
        for i in 0..Q8_0_BLOCK_VALUES {
            let q = data[base + 2 + i] as i8;
            out[block * Q8_0_BLOCK_VALUES + i] = scale * q as f32;
        }
    }
    Ok(out)
}

fn matmul_cpu(weights: &[f32], x: &[f32], k: usize, n: usize) -> Vec<f32> {
    let mut y = vec![0.0f32; n];
    for col in 0..n {
        let mut acc = 0.0f32;
        for row in 0..k {
            acc += x[row] * weights[row * n + col];
        }
        y[col] = acc;
    }
    y
}

fn diff_stats(y_cpu: &[f32], y_gpu: &[f32]) -> (f32, f32) {
    let mut max_abs = 0.0f32;
    let mut mean_abs = 0.0f32;
    let len = y_cpu.len().min(y_gpu.len());
    if len == 0 {
        return (0.0, 0.0);
    }
    for (cpu, gpu) in y_cpu.iter().zip(y_gpu.iter()) {
        let diff = (cpu - gpu).abs();
        max_abs = max_abs.max(diff);
        mean_abs += diff;
    }
    mean_abs /= len as f32;
    (max_abs, mean_abs)
}

async fn gpu_matmul_q8_0(
    quant: &[u8],
    x: &[f32],
    k: usize,
    n: usize,
    gpu: &GpuContext,
) -> Result<Vec<f32>, String> {
    let device = &gpu.device;
    let queue = &gpu.queue;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("q8_0_probe"),
        source: wgpu::ShaderSource::Wgsl(Q8_0_SHADER.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("q8_0_probe_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("q8_0_probe_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("q8_0_probe_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let quant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_probe_quant"),
        contents: quant,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_probe_x"),
        contents: cast_slice(x),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let y_bytes = (n * std::mem::size_of::<f32>()) as u64;
    let y_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_probe_y"),
        size: y_bytes,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = [k as u32, n as u32, n as u32, 0u32];
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_probe_params"),
        contents: cast_slice(&params),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("q8_0_probe_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: quant_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: x_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: y_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_probe_readback"),
        size: y_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("q8_0_probe_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("q8_0_probe_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let workgroup_size = 64u32;
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&y_buffer, 0, &readback, 0, y_bytes);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("map_async failed: {err:?}")),
        Err(_) => return Err("map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) & 1) as u32;
    let exp = ((bits >> 10) & 0x1f) as i32;
    let frac = (bits & 0x03ff) as u32;
    let mut val = if exp == 0 {
        if frac == 0 {
            0.0
        } else {
            (frac as f32) * 2f32.powi(-24)
        }
    } else if exp == 31 {
        f32::INFINITY
    } else {
        (1.0 + (frac as f32) * 0.000_976_562_5) * 2f32.powi(exp - 15)
    };
    if sign == 1 {
        val = -val;
    }
    val
}

const Q8_0_SHADER: &str = r#"
struct Params {
    k: u32,
    n: u32,
    stride: u32,
    _pad: u32,
};

@group(0) @binding(0)
var<storage, read> quant: array<u32>;

@group(0) @binding(1)
var<storage, read> x: array<f32>;

@group(0) @binding(2)
var<storage, read_write> y: array<f32>;

@group(0) @binding(3)
var<uniform> params: Params;

fn q8_0_unpack(block: u32, idx: u32) -> f32 {
    let base = block * 34u;
    let scale_bits = u16(quant[(base + 0u) / 4u] & 0xffffu);
    let scale = unpack_f16(scale_bits);
    let byte_index = base + 2u + idx;
    let word = quant[byte_index / 4u];
    let shift = (byte_index & 3u) * 8u;
    let byte = u32((word >> shift) & 0xffu);
    let signed = i32(byte << 24u) >> 24;
    return scale * f32(signed);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    if (col >= params.n) {
        return;
    }
    let mut acc = 0.0;
    for (var row = 0u; row < params.k; row = row + 1u) {
        let idx = row * params.n + col;
        let block = idx / 32u;
        let offset = idx % 32u;
        let w = q8_0_unpack(block, offset);
        acc = acc + x[row] * w;
    }
    y[col] = acc;
}
"#;
