use std::cell::RefCell;
use std::rc::Rc;

use serde::Deserialize;
use js_sys;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::state::{
    AppState, CacheInfo, GptOssLogEntry, GptOssStage, GptOssStageStatus, GptOssVizState,
    MemoryUsage, MlTokenCandidate,
};

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
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GptOssTokenCandidate {
    token_id: u32,
    token_text: String,
    probability: f32,
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
                update_stage(
                    &mut self.load_stages,
                    stage.clone(),
                    status,
                    detail.clone(),
                    bytes,
                    total_bytes,
                    None,
                    None,
                    ts_ms,
                );
                push_log(self, format!("LOAD {stage}"), status, detail, ts_ms);
            }
            GptOssTelemetry::InferenceStage {
                stage,
                status,
                step,
                total_steps,
                detail,
                ts_ms,
            } => {
                update_stage(
                    &mut self.inference_stages,
                    stage.clone(),
                    status,
                    detail.clone(),
                    None,
                    None,
                    step,
                    total_steps,
                    ts_ms,
                );
                push_log(self, format!("INFER {stage}"), status, detail, ts_ms);
            }
            GptOssTelemetry::InferenceEvent { event, ts_ms } => {
                match event {
                    GptOssInferenceTelemetry::TokenGenerated {
                        token_text,
                        top_k,
                        entropy,
                        tokens_per_sec,
                        ..
                    } => {
                        self.token_stream.push_str(&token_text);
                        trim_token_stream(&mut self.token_stream, 420);
                        self.top_k = top_k
                            .into_iter()
                            .map(|c| MlTokenCandidate {
                                token_id: c.token_id,
                                token_text: c.token_text,
                                probability: c.probability,
                            })
                            .collect();
                        self.tokens_per_sec = Some(tokens_per_sec);
                        self.entropy = Some(entropy);
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
                    GptOssInferenceTelemetry::AttentionWeights { .. }
                    | GptOssInferenceTelemetry::LayerActivation { .. } => {}
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

pub(crate) fn push_gptoss_event(
    state: &Rc<RefCell<AppState>>,
    event: GptOssTelemetry,
) {
    if let Ok(mut guard) = state.try_borrow_mut() {
        guard.gptoss.apply_telemetry(event);
    }
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
