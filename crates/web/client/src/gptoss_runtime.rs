#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen_futures::spawn_local;
use js_sys;
use web_sys;

use crate::gguf_web::{
    fetch_and_parse_index, fetch_range, fetch_range_with_total, GgufIndex, GgufTensor,
};
use crate::gptoss_viz::{push_gptoss_event, GptOssTelemetry, StageStatus};
use crate::state::{AppState, GpuContext};

const DEFAULT_METADATA_BYTES: u64 = 16 * 1024 * 1024;
const MAX_METADATA_ATTEMPTS: usize = 3;
const LOAD_CHUNK_BYTES: u64 = 8 * 1024 * 1024;
const PROGRESS_STEP_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_GGUF_URL: &str =
    "https://huggingface.co/openai/gpt-oss-20b/resolve/main/gpt-oss-20b-Q8_0.gguf";
const LOCAL_GGUF_URL: &str = "http://localhost:9898/gpt-oss-20b-Q8_0.gguf";

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

    let index = fetch_and_parse_index(&gguf_url, DEFAULT_METADATA_BYTES, MAX_METADATA_ATTEMPTS)
        .await?;
    emit_load_stage(
        &state,
        "gguf_parse",
        StageStatus::Completed,
        Some(format!("tensors={}", index.tensors.len())),
        None,
        None,
    );

    emit_tensor_scan(&state, &index, 18);

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
    let mut tensor_cursor = tensor_start_cursor(&index);
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
