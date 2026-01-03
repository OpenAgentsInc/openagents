use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use serde::Deserialize;
use js_sys;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::state::{
    AppState, CacheInfo, GptOssLogEntry, GptOssStage, GptOssStageStatus, GptOssVizState,
    LayerActivity, MemoryUsage, MlTokenCandidate, TensorInfo,
};

const ATTENTION_HISTORY_MAX_ROWS: usize = 12;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StageStatus {
    Started,
    Progress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum GptOssTelemetry {
    LoadStage {
        stage: String,
        status: StageStatus,
        detail: Option<String>,
        bytes: Option<u64>,
        total_bytes: Option<u64>,
        ts_ms: Option<u64>,
    },
    InferenceStage {
        stage: String,
        status: StageStatus,
        step: Option<usize>,
        total_steps: Option<usize>,
        detail: Option<String>,
        ts_ms: Option<u64>,
    },
    InferenceEvent {
        event: GptOssInferenceTelemetry,
        ts_ms: Option<u64>,
    },
}

thread_local! {
    static PENDING_GPTOSS_EVENTS: RefCell<VecDeque<GptOssTelemetry>> =
        RefCell::new(VecDeque::new());
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum GptOssInferenceTelemetry {
    TokenGenerated {
        token_id: u32,
        token_text: String,
        top_k: Vec<GptOssTokenCandidate>,
        entropy: f32,
        tokens_per_sec: f32,
    },
    AttentionWeights {
        layer: usize,
        head: usize,
        weights: Vec<Vec<f32>>,
    },
    LayerActivation {
        layer: usize,
        attention_norm: f32,
        mlp_norm: f32,
        output_norm: f32,
    },
    CacheStatus {
        layer: usize,
        seq_len: usize,
        max_len: usize,
        offset: usize,
        memory_bytes: usize,
    },
    MemoryUsage {
        gpu_allocated: usize,
        cache_total: usize,
        activations: usize,
    },
    TensorResident {
        name: String,
        bytes: usize,
        kind: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GptOssTokenCandidate {
    pub(crate) token_id: u32,
    pub(crate) token_text: String,
    pub(crate) probability: f32,
}

impl GptOssVizState {
    fn apply_telemetry(&mut self, event: GptOssTelemetry) {
        match event {
            GptOssTelemetry::LoadStage {
                stage,
                status,
                detail,
                bytes,
                total_bytes,
                ts_ms,
            } => {
                let detail_clone = detail.clone();
                if matches!(status, StageStatus::Started | StageStatus::Progress) {
                    self.current_stage = Some(format!("LOAD {stage}"));
                }
                if stage == "load_start" {
                    self.load_progress = Some(0.0);
                }
                if stage == "load_complete" {
                    self.load_progress = Some(1.0);
                }
                if stage == "load_failed" {
                    self.load_progress = None;
                }
                if stage == "weights_fetch" {
                    if let (Some(bytes), Some(total)) = (bytes, total_bytes) {
                        if total > 0 {
                            let progress = (bytes as f32 / total as f32).clamp(0.0, 1.0);
                            self.load_progress = Some(progress);
                        }
                    }
                }
                if matches!(stage.as_str(), "tensor_scan" | "tensor_index") {
                    if let Some(name) = detail.clone() {
                        if self.recent_tensors.len() >= 12 {
                            self.recent_tensors.pop_front();
                        }
                        self.recent_tensors.push_back(name);
                    }
                }
                if stage == "moe_mode" {
                    if let Some(detail) = detail_clone.as_ref() {
                        self.moe_mode = Some(detail.clone());
                    }
                }
                if stage == "gpu_limits" {
                    if let Some(detail) = detail_clone.as_ref() {
                        self.gpu_limits = Some(detail.clone());
                    }
                }
                if stage == "token_limits" {
                    if let Some(detail) = detail_clone.as_ref() {
                        self.token_limits = Some(detail.clone());
                    }
                }
                if stage == "model_config" {
                    if let Some(detail) = detail_clone.as_ref() {
                        apply_model_config(self, detail);
                    }
                }
                update_stage(
                    &mut self.load_stages,
                    stage.clone(),
                    status,
                    detail_clone.clone(),
                    bytes,
                    total_bytes,
                    None,
                    None,
                    ts_ms,
                );
                push_log(self, format!("LOAD {stage}"), status, detail, ts_ms);
                refresh_current_stage(self);
            }
            GptOssTelemetry::InferenceStage {
                stage,
                status,
                step,
                total_steps,
                detail,
                ts_ms,
            } => {
                let detail_clone = detail.clone();
                if matches!(status, StageStatus::Started | StageStatus::Progress) {
                    self.current_stage = Some(format!("INFER {stage}"));
                }
                if stage == "runtime_mode" {
                    if let Some(detail) = detail_clone.as_ref() {
                        apply_runtime_mode(self, detail);
                    }
                }
                if stage == "moe_mode" {
                    if let Some(detail) = detail_clone.as_ref() {
                        self.moe_mode = Some(detail.clone());
                    }
                }
                if stage == "attn_fallback" {
                    if let Some(detail) = detail_clone.as_ref() {
                        self.attention_mode = Some(detail.clone());
                    }
                }
                if stage == "attn_score" {
                    if let Some(detail) = detail_clone.as_ref() {
                        if let Some(mode) = extract_attn_mode(detail) {
                            self.attention_mode = Some(mode);
                        }
                    }
                }
                if matches!(status, StageStatus::Failed) {
                    let msg = detail_clone
                        .clone()
                        .unwrap_or_else(|| format!("{stage} failed"));
                    let ignore = matches!(
                        stage.as_str(),
                        "q8_0_probe"
                            | "mxfp4_probe"
                            | "rmsnorm_probe"
                            | "rope_probe"
                            | "attn_probe"
                            | "moe_expert"
                            | "layer_fallback"
                    );
                    if !ignore {
                        self.inference_error = Some(msg);
                    }
                }
                update_stage(
                    &mut self.inference_stages,
                    stage.clone(),
                    status,
                    detail_clone.clone(),
                    None,
                    None,
                    step,
                    total_steps,
                    ts_ms,
                );
                push_log(self, format!("INFER {stage}"), status, detail, ts_ms);
                refresh_current_stage(self);
            }
            GptOssTelemetry::InferenceEvent { event, ts_ms } => {
                match event {
                    GptOssInferenceTelemetry::TokenGenerated {
                        token_id,
                        token_text,
                        top_k,
                        entropy,
                        tokens_per_sec,
                    } => {
                        self.token_stream.push_str(&token_text);
                        trim_token_stream(&mut self.token_stream, 420);
                        let converted = top_k
                            .iter()
                            .map(|c| MlTokenCandidate {
                                token_id: c.token_id,
                                token_text: c.token_text.clone(),
                                probability: c.probability,
                            })
                            .collect::<Vec<_>>();
                        self.top_k = converted.clone();
                        self.probability_history.push_back(converted);
                        if self.probability_history.len() > 18 {
                            self.probability_history.pop_front();
                        }
                        self.last_token_id = Some(token_id);
                        self.tokens_per_sec = Some(tokens_per_sec);
                        self.entropy = Some(entropy);
                        self.entropy_history.push_back(entropy);
                        if self.entropy_history.len() > 32 {
                            self.entropy_history.pop_front();
                        }
                        self.last_token_ts_ms = ts_ms;
                    }
                    GptOssInferenceTelemetry::CacheStatus {
                        layer,
                        seq_len,
                        max_len,
                        offset,
                        memory_bytes,
                    } => {
                        if self.cache_status.len() <= layer {
                            self.cache_status.resize_with(layer + 1, || CacheInfo {
                                layer: 0,
                                seq_len: 0,
                                max_len: 0,
                                offset: 0,
                                memory_bytes: 0,
                            });
                        }
                        self.cache_status[layer] = CacheInfo {
                            layer,
                            seq_len,
                            max_len,
                            offset,
                            memory_bytes,
                        };
                    }
                    GptOssInferenceTelemetry::MemoryUsage {
                        gpu_allocated,
                        cache_total,
                        activations,
                    } => {
                        self.memory_usage = Some(MemoryUsage {
                            gpu_allocated,
                            cache_total,
                            activations,
                        });
                    }
                    GptOssInferenceTelemetry::TensorResident { name, bytes, kind } => {
                        self.resident_tensors.push(TensorInfo { name, bytes, kind });
                        if self.resident_tensors.len() > 12 {
                            let drop_count = self.resident_tensors.len() - 12;
                            self.resident_tensors.drain(0..drop_count);
                        }
                    }
                    GptOssInferenceTelemetry::AttentionWeights { layer, head, weights } => {
                        if !weights.is_empty() {
                            let mut merged = weights;
                            if merged.len() == 1 {
                                if let Some(existing) = self.attention_weights.as_ref() {
                                    if self.attention_layer == layer
                                        && self.attention_head == head
                                        && !existing.is_empty()
                                    {
                                        let cols = existing[0].len();
                                        if cols == merged[0].len() {
                                            let mut combined = existing.clone();
                                            combined.push(merged.remove(0));
                                            if combined.len() > ATTENTION_HISTORY_MAX_ROWS {
                                                let drop = combined.len() - ATTENTION_HISTORY_MAX_ROWS;
                                                combined.drain(0..drop);
                                            }
                                            merged = combined;
                                        }
                                    }
                                }
                            }
                            self.attention_weights = Some(merged);
                            self.attention_layer = layer;
                            self.attention_head = head;
                        }
                    }
                    GptOssInferenceTelemetry::LayerActivation {
                        layer,
                        attention_norm,
                        mlp_norm,
                        output_norm,
                    } => {
                        if self.layer_activations.len() <= layer {
                            let target_len = layer + 1;
                            let start = self.layer_activations.len();
                            for idx in start..target_len {
                                self.layer_activations.push(LayerActivity {
                                    layer: idx,
                                    attention_norm: 0.0,
                                    mlp_norm: 0.0,
                                    output_norm: 0.0,
                                });
                            }
                        }
                        if let Some(entry) = self.layer_activations.get_mut(layer) {
                            entry.attention_norm = attention_norm;
                            entry.mlp_norm = mlp_norm;
                            entry.output_norm = output_norm;
                        }
                    }
                }

                if let Some(ts_ms) = ts_ms {
                    if self.start_ts_ms.is_none() {
                        self.start_ts_ms = Some(ts_ms);
                    }
                }
            }
        }
    }
}

pub(crate) fn push_gptoss_event(_state: &Rc<RefCell<AppState>>, event: GptOssTelemetry) {
    PENDING_GPTOSS_EVENTS.with(|queue| {
        queue.borrow_mut().push_back(event);
    });
}

pub(crate) fn flush_gptoss_events(state: &mut GptOssVizState) {
    PENDING_GPTOSS_EVENTS.with(|queue| {
        let mut queue = queue.borrow_mut();
        while let Some(event) = queue.pop_front() {
            state.apply_telemetry(event);
        }
    });
}

pub(crate) fn clear_gptoss_events() {
    PENDING_GPTOSS_EVENTS.with(|queue| {
        queue.borrow_mut().clear();
    });
}

fn update_stage(
    stages: &mut Vec<GptOssStage>,
    name: String,
    status: StageStatus,
    detail: Option<String>,
    bytes: Option<u64>,
    total_bytes: Option<u64>,
    step: Option<usize>,
    total_steps: Option<usize>,
    ts_ms: Option<u64>,
) {
    let status = match status {
        StageStatus::Started | StageStatus::Progress => GptOssStageStatus::Running,
        StageStatus::Completed => GptOssStageStatus::Completed,
        StageStatus::Failed => GptOssStageStatus::Failed,
    };

    if let Some(existing) = stages.iter_mut().find(|stage| stage.name == name) {
        existing.status = status;
        existing.detail = detail;
        existing.bytes = bytes;
        existing.total_bytes = total_bytes;
        existing.step = step;
        existing.total_steps = total_steps;
        existing.ts_ms = ts_ms;
    } else {
        stages.push(GptOssStage {
            name,
            status,
            detail,
            bytes,
            total_bytes,
            step,
            total_steps,
            ts_ms,
        });
    }
}

fn refresh_current_stage(state: &mut GptOssVizState) {
    let mut best_ts = 0u64;
    let mut best_label: Option<String> = None;

    for stage in &state.load_stages {
        if stage.status != GptOssStageStatus::Running {
            continue;
        }
        let ts = stage.ts_ms.unwrap_or(0);
        if ts >= best_ts {
            best_ts = ts;
            best_label = Some(format!("LOAD {}", stage.name));
        }
    }

    for stage in &state.inference_stages {
        if stage.status != GptOssStageStatus::Running {
            continue;
        }
        let ts = stage.ts_ms.unwrap_or(0);
        if ts >= best_ts {
            best_ts = ts;
            best_label = Some(format!("INFER {}", stage.name));
        }
    }

    state.current_stage = best_label;
}

fn push_log(
    state: &mut GptOssVizState,
    label: String,
    status: StageStatus,
    detail: Option<String>,
    ts_ms: Option<u64>,
) {
    let status = match status {
        StageStatus::Started | StageStatus::Progress => GptOssStageStatus::Running,
        StageStatus::Completed => GptOssStageStatus::Completed,
        StageStatus::Failed => GptOssStageStatus::Failed,
    };
    let message = if let Some(detail) = detail {
        format!("{label} | {detail}")
    } else {
        label
    };
    if let Some(ts) = ts_ms {
        if state.start_ts_ms.is_none() {
            state.start_ts_ms = Some(ts);
        }
    }
    if state.events.len() >= 160 {
        state.events.pop_front();
    }
    state.events.push_back(GptOssLogEntry {
        ts_ms,
        message,
        status,
    });
}

fn trim_token_stream(stream: &mut String, max_len: usize) {
    let total = stream.chars().count();
    if total <= max_len {
        return;
    }
    let skip = total.saturating_sub(max_len);
    let start = stream
        .char_indices()
        .nth(skip)
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    *stream = stream[start..].to_string();
}

fn apply_runtime_mode(state: &mut GptOssVizState, detail: &str) {
    let mut moe_mode: Option<String> = None;
    let mut moe_topk: Option<String> = None;
    let mut moe_expert: Option<String> = None;
    let mut sample_mode: Option<String> = None;
    let mut cpu_fallback: Option<String> = None;
    for part in detail.split_whitespace() {
        if let Some((key, value)) = part.split_once('=') {
            match key {
                "layers" => {
                    state.active_layers = value.parse::<usize>().ok();
                }
                "attn" => {
                    state.attention_mode = Some(value.to_string());
                }
                "moe" => {
                    moe_mode = Some(value.to_string());
                }
                "expert" => {
                    moe_expert = Some(value.to_string());
                }
                "topk" => {
                    moe_topk = Some(value.to_string());
                }
                "sample" => {
                    sample_mode = Some(value.to_string());
                }
                "cpu_fallback" => {
                    cpu_fallback = Some(value.to_string());
                }
                _ => {}
            }
        }
    }
    if let Some(moe) = moe_mode {
        let mut mode = moe;
        if let Some(expert) = moe_expert {
            mode = format!("{mode} expert={expert}");
        }
        if let Some(topk) = moe_topk {
            mode = format!("{mode} topk={topk}");
        }
        state.moe_mode = Some(mode);
    }
    if let Some(sample) = sample_mode {
        state.sampling_mode = Some(sample);
    }
    if let Some(fallback) = cpu_fallback {
        state.cpu_fallback = Some(fallback);
    }
}

fn apply_model_config(state: &mut GptOssVizState, detail: &str) {
    let mut blocks: Option<usize> = None;
    let mut heads: Option<usize> = None;
    for part in detail.split_whitespace() {
        if let Some((key, value)) = part.split_once('=') {
            match key {
                "blocks" => {
                    blocks = value.parse::<usize>().ok();
                }
                "heads" => {
                    heads = value.parse::<usize>().ok();
                }
                _ => {}
            }
        }
    }
    if let Some(blocks) = blocks {
        state.max_layers = blocks.max(1);
        let max_layer = state.max_layers.saturating_sub(1);
        state.attention_selected_layer = state.attention_selected_layer.min(max_layer);
    }
    if let Some(heads) = heads {
        state.max_heads = heads.max(1);
        let max_head = state.max_heads.saturating_sub(1);
        state.attention_selected_head = state.attention_selected_head.min(max_head);
    }
}

fn extract_attn_mode(detail: &str) -> Option<String> {
    let mut mode: Option<String> = None;
    let mut phase: Option<String> = None;
    let mut window: Option<String> = None;
    let mut fallback = false;
    for part in detail.split_whitespace() {
        if part == "gpu" || part == "cpu" {
            mode = Some(part.to_string());
        } else if let Some(value) = part.strip_prefix("phase=") {
            phase = Some(value.to_string());
        } else if let Some(value) = part.strip_prefix("window=") {
            window = Some(value.to_string());
        } else if part == "fallback" {
            fallback = true;
        }
    }
    if mode.is_none() && phase.is_none() && window.is_none() && !fallback {
        return None;
    }
    let mut out = String::new();
    if let Some(mode) = mode {
        out.push_str(&mode);
    }
    if let Some(phase) = phase {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str("phase=");
        out.push_str(&phase);
    }
    if let Some(window) = window {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str("window=");
        out.push_str(&window);
    }
    if fallback {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str("fallback");
    }
    Some(out)
}

pub(crate) fn init_gptoss_viz_runtime(state: Rc<RefCell<AppState>>) {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };

    if let Ok(data_val) = js_sys::Reflect::get(&window, &JsValue::from_str("GPTOSS_DATA")) {
        if data_val.is_truthy() {
            if let Ok(events) = serde_wasm_bindgen::from_value::<Vec<GptOssTelemetry>>(data_val) {
                let mut guard = state.borrow_mut();
                for event in events {
                    guard.gptoss.apply_telemetry(event);
                }
            }
        }
    }

    if let Ok(url_val) = js_sys::Reflect::get(&window, &JsValue::from_str("GPTOSS_DATA_URL")) {
        if let Some(url) = url_val.as_string() {
            let state_clone = state.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(err) = fetch_events(&state_clone, &url).await {
                    web_sys::console::error_1(&err);
                }
            });
        }
    }
}

async fn fetch_events(state: &Rc<RefCell<AppState>>, url: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let resp_value = JsFuture::from(window.fetch_with_str(url)).await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;

    if !resp.ok() {
        return Err(JsValue::from_str("gptoss telemetry fetch failed"));
    }

    let text = JsFuture::from(resp.text()?).await?;
    let text = text.as_string().unwrap_or_default();
    let events: Vec<GptOssTelemetry> = serde_json::from_str(&text)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;

    let mut guard = state.borrow_mut();
    for event in events {
        guard.gptoss.apply_telemetry(event);
    }
    Ok(())
}
