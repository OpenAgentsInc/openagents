use std::cell::RefCell;
use std::rc::Rc;

use serde::Deserialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::state::{
    AppState, CacheInfo, LayerActivity, MemoryUsage, MlTokenCandidate, MlVizState,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum MlVizTelemetry {
    TokenGenerated {
        token_id: u32,
        token_text: String,
        top_k: Vec<MlVizTokenCandidate>,
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
pub(crate) struct MlVizTokenCandidate {
    token_id: u32,
    token_text: String,
    probability: f32,
}

impl MlVizState {
    pub(crate) fn apply_telemetry(&mut self, event: MlVizTelemetry) {
        match event {
            MlVizTelemetry::TokenGenerated {
                token_text,
                top_k,
                entropy,
                tokens_per_sec,
                ..
            } => {
                self.token_stream.push_str(&token_text);
                trim_token_stream(&mut self.token_stream, 320);
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
                if self.entropy_history.len() >= 48 {
                    self.entropy_history.pop_front();
                }
                self.entropy_history.push_back(entropy);

                if self.probability_history.len() >= 16 {
                    self.probability_history.pop_front();
                }
                self.probability_history.push_back(self.top_k.clone());
            }
            MlVizTelemetry::AttentionWeights { layer, head, weights } => {
                self.attention_weights = Some(weights);
                self.attention_layer = layer;
                self.attention_head = head;
                self.max_layers = self.max_layers.max(layer + 1);
                self.max_heads = self.max_heads.max(head + 1);
            }
            MlVizTelemetry::LayerActivation {
                layer,
                attention_norm,
                mlp_norm,
                output_norm,
            } => {
                if self.layer_activations.len() <= layer {
                    self.layer_activations.resize_with(layer + 1, || LayerActivity {
                        layer: 0,
                        attention_norm: 0.0,
                        mlp_norm: 0.0,
                        output_norm: 0.0,
                    });
                }
                self.layer_activations[layer] = LayerActivity {
                    layer,
                    attention_norm,
                    mlp_norm,
                    output_norm,
                };
                self.max_layers = self.max_layers.max(layer + 1);
            }
            MlVizTelemetry::CacheStatus {
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
                self.max_layers = self.max_layers.max(layer + 1);
            }
            MlVizTelemetry::MemoryUsage {
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
        }

        self.selected_layer = self.selected_layer.min(self.max_layers.saturating_sub(1));
        self.selected_head = self.selected_head.min(self.max_heads.saturating_sub(1));
    }
}

pub(crate) fn init_ml_viz_runtime(state: Rc<RefCell<AppState>>) {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };

    if let Ok(data_val) = js_sys::Reflect::get(&window, &JsValue::from_str("ML_VIZ_DATA")) {
        if data_val.is_truthy() {
            if let Ok(events) = serde_wasm_bindgen::from_value::<Vec<MlVizTelemetry>>(data_val) {
                let mut guard = state.borrow_mut();
                for event in events {
                    guard.ml_viz.apply_telemetry(event);
                }
            }
        }
    }

    if let Ok(url_val) = js_sys::Reflect::get(&window, &JsValue::from_str("ML_VIZ_DATA_URL")) {
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
        return Err(JsValue::from_str("ml viz telemetry fetch failed"));
    }

    let text = JsFuture::from(resp.text()?).await?;
    let text = text.as_string().unwrap_or_default();
    let events: Vec<MlVizTelemetry> = serde_json::from_str(&text)
        .map_err(|err| JsValue::from_str(&err.to_string()))?;

    let mut guard = state.borrow_mut();
    for event in events {
        guard.ml_viz.apply_telemetry(event);
    }
    Ok(())
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
