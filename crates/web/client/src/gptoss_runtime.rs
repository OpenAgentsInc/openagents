#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use bytemuck::{cast_slice, Pod, Zeroable};
use futures::channel::oneshot;
use gloo_timers::future::TimeoutFuture;
use wgpu::util::DeviceExt;
use wasm_bindgen_futures::spawn_local;
use js_sys;
use web_sys;

use crate::gguf_web::{
    fetch_and_parse_index_source, fetch_range_source, fetch_range_with_total_source,
    pick_gguf_file, GgufIndex, GgufSource, GgufTensor,
};
use crate::gptoss_tokenizer::GptOssTokenizer;
use crate::gptoss_viz::{
    clear_gptoss_events, push_gptoss_event, GptOssInferenceTelemetry, GptOssTelemetry,
    GptOssTokenCandidate, StageStatus,
};
use crate::state::{AppState, GpuContext};

const DEFAULT_METADATA_BYTES: u64 = 16 * 1024 * 1024;
const MAX_METADATA_ATTEMPTS: usize = 3;
const LOAD_CHUNK_BYTES: u64 = 8 * 1024 * 1024;
const PROGRESS_STEP_BYTES: u64 = 64 * 1024 * 1024;
const LOCAL_GGUF_ROUTE: &str = "/gpt-oss-20b-Q8_0.gguf";
const LOCAL_GGUF_URL: &str = "http://localhost:8080/gpt-oss-20b-Q8_0.gguf";
const LOCAL_GGUF_DEV_URL: &str = "http://localhost:3000/gpt-oss-20b-Q8_0.gguf";
const LOCAL_GGUF_PATH: &str = "crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf";
const LOCAL_GGUF_SERVE_CMD: &str =
    "cargo run -p ml --bin gguf_serve -- crates/ml/models/gpt-oss-20b/gpt-oss-20b-Q8_0.gguf";
const CURRENT_DATE: &str = "2026-01-02";
const DEFAULT_USER_PROMPT: &str = "Give me one sentence about what GPT-OSS can do.";
const DEFAULT_DEVELOPER_PROMPT: &str = "";
const DEFAULT_MAX_NEW_TOKENS: usize = 8;
const DEFAULT_MAX_KV_TOKENS: usize = 32;
const DEFAULT_KV_BUDGET_BYTES: u64 = 6 * 1024 * 1024 * 1024;
const DEFAULT_SAMPLE_TOP_K: usize = 40;
const DEFAULT_SAMPLE_TEMP: f32 = 1.0;
const DEFAULT_SAMPLE_TOP_P: f32 = 1.0;
const Q8_0_BLOCK_BYTES: usize = 34;
const Q8_0_BLOCK_VALUES: usize = 32;
const MXFP4_BLOCK_BYTES: usize = 17;
const MXFP4_BLOCK_VALUES: usize = 32;
const MXFP4_VALUES: [f32; 16] = [
    0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, -0.0, -0.5, -1.0, -1.5, -2.0, -3.0, -4.0,
    -6.0,
];
const SWIGLU_ALPHA: f32 = 1.702;
const SWIGLU_LIMIT: f32 = 7.0;
const PROBE_TOLERANCE: f32 = 1e-3;
const ROPE_NTK_ALPHA: f32 = 1.0;
const ROPE_NTK_BETA: f32 = 32.0;
const TENSOR_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;
const TENSOR_CACHE_MAX_ENTRY_BYTES: usize = 4 * 1024 * 1024;
const TOKEN_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;
const TOKEN_CACHE_MAX_ENTRY_BYTES: usize = 256 * 1024;
const Q8_0_CACHE_MAX_BYTES: usize = 96 * 1024 * 1024;
const Q8_0_CACHE_MAX_ENTRY_BYTES: usize = 32 * 1024 * 1024;
const EXPERT_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;
const EXPERT_CACHE_MAX_ENTRY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
struct GptOssConfig {
    block_count: u32,
    context_length: u32,
    embedding_length: u32,
    feed_forward_length: u32,
    head_count: u32,
    head_count_kv: u32,
    rope_dimension_count: u32,
    rope_theta: f32,
    rope_scaling_factor: f32,
    rope_scaling_original_context: u32,
    rms_epsilon: f32,
    sliding_window: u32,
    expert_count: u32,
    experts_per_token: u32,
}

#[derive(Clone, Copy, Debug)]
struct SamplingConfig {
    enabled: bool,
    temperature: f32,
    top_k: usize,
    top_p: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct SamplingOverrides {
    enabled: Option<bool>,
    temperature: Option<f32>,
    top_k: Option<usize>,
    top_p: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
struct KvLimit {
    max_tokens: usize,
    per_layer_max: usize,
    budget_max: Option<usize>,
    budget_bytes: u64,
}

struct LayerKvCache {
    k: Vec<f32>,
    v: Vec<f32>,
    start: usize,
    len: usize,
    capacity: usize,
    stride: usize,
    gpu_k: Option<wgpu::Buffer>,
    gpu_v: Option<wgpu::Buffer>,
    gpu_bytes: usize,
    cpu_enabled: bool,
}

impl LayerKvCache {
    fn new() -> Self {
        Self {
            k: Vec::new(),
            v: Vec::new(),
            start: 0,
            len: 0,
            capacity: 0,
            stride: 0,
            gpu_k: None,
            gpu_v: None,
            gpu_bytes: 0,
            cpu_enabled: true,
        }
    }

    fn token_count(&self) -> usize {
        self.len
    }

    fn ensure_capacity(
        &mut self,
        max_len: usize,
        stride: usize,
        gpu: &GpuContext,
        store_cpu: bool,
    ) -> Result<(), String> {
        if max_len == 0 {
            return Err("kv cache max length is zero".to_string());
        }
        if stride == 0 {
            return Err("kv cache stride is zero".to_string());
        }
        if self.capacity == 0
            || self.capacity != max_len
            || self.stride != stride
            || self.cpu_enabled != store_cpu
        {
            self.capacity = max_len;
            self.stride = stride;
            self.start = 0;
            self.len = 0;
            self.cpu_enabled = store_cpu;
            if store_cpu {
                self.k = vec![0.0f32; max_len * stride];
                self.v = vec![0.0f32; max_len * stride];
            } else {
                self.k.clear();
                self.v.clear();
            }
            self.gpu_k = None;
            self.gpu_v = None;
            self.gpu_bytes = 0;
        }

        if self.gpu_k.is_none() || self.gpu_v.is_none() {
            let bytes = max_len
                .checked_mul(stride)
                .ok_or_else(|| "kv cache byte overflow".to_string())?
                .checked_mul(std::mem::size_of::<f32>())
                .ok_or_else(|| "kv cache byte overflow".to_string())?;
            let limits = gpu.device.limits();
            ensure_buffer_limit(
                "kv cache",
                bytes,
                limits.max_storage_buffer_binding_size.into(),
                limits.max_buffer_size,
            )?;
            let k_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kv_cache_k"),
                size: bytes as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let v_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kv_cache_v"),
                size: bytes as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.gpu_k = Some(k_buf);
            self.gpu_v = Some(v_buf);
            self.gpu_bytes = bytes * 2;
        }

        Ok(())
    }

    fn append(
        &mut self,
        k: &[f32],
        v: &[f32],
        kv_heads: usize,
        head_dim: usize,
        max_len: usize,
        gpu: &GpuContext,
        store_cpu: bool,
    ) -> Result<(), String> {
        let expected = kv_heads
            .checked_mul(head_dim)
            .ok_or_else(|| "kv append overflow".to_string())?;
        if k.len() != expected || v.len() != expected {
            return Err(format!(
                "kv append shape mismatch k={} v={} expected={expected}",
                k.len(),
                v.len()
            ));
        }
        self.ensure_capacity(max_len, expected, gpu, store_cpu)?;

        let slot = if self.len < self.capacity {
            let slot = (self.start + self.len) % self.capacity;
            self.len = self.len.saturating_add(1).min(self.capacity);
            slot
        } else {
            let slot = self.start;
            self.start = (self.start + 1) % self.capacity;
            slot
        };

        let base = slot
            .checked_mul(self.stride)
            .ok_or_else(|| "kv append base overflow".to_string())?;
        let end = base + self.stride;
        if self.cpu_enabled {
            self.k[base..end].copy_from_slice(k);
            self.v[base..end].copy_from_slice(v);
        }

        let byte_offset = base
            .checked_mul(std::mem::size_of::<f32>())
            .ok_or_else(|| "kv append offset overflow".to_string())? as u64;
        if let (Some(k_buf), Some(v_buf)) = (self.gpu_k.as_ref(), self.gpu_v.as_ref()) {
            gpu.queue.write_buffer(k_buf, byte_offset, cast_slice(k));
            gpu.queue.write_buffer(v_buf, byte_offset, cast_slice(v));
        }

        Ok(())
    }

    fn memory_bytes(&self) -> usize {
        self.gpu_bytes
    }
}

struct KvCache {
    layers: Vec<LayerKvCache>,
    seq_len: usize,
    max_len: usize,
}

impl KvCache {
    fn new(layer_count: usize, max_len: usize) -> Self {
        let mut layers = Vec::with_capacity(layer_count);
        for _ in 0..layer_count {
            layers.push(LayerKvCache::new());
        }
        Self {
            layers,
            seq_len: 0,
            max_len,
        }
    }

    fn layer_mut(&mut self, layer: usize) -> Result<&mut LayerKvCache, String> {
        self.layers
            .get_mut(layer)
            .ok_or_else(|| format!("kv cache missing layer {layer}"))
    }

    fn total_bytes(&self) -> usize {
        self.layers.iter().map(LayerKvCache::memory_bytes).sum()
    }
}

#[derive(Clone, Debug)]
struct CacheStats {
    hits: u64,
    misses: u64,
    evictions: u64,
    skipped: u64,
    bytes: usize,
    entries: usize,
}

#[derive(Clone, Debug)]
struct TensorCacheEntry {
    data: Vec<f32>,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct TensorCache {
    entries: HashMap<String, TensorCacheEntry>,
    order: VecDeque<String>,
    total_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    skipped: u64,
}

impl TensorCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            total_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            skipped: 0,
        }
    }

    fn get(&mut self, name: &str) -> Option<Vec<f32>> {
        let hit = self.entries.get(name).map(|entry| entry.data.clone());
        if hit.is_some() {
            self.hits = self.hits.saturating_add(1);
            self.touch(name);
            return hit;
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    fn insert(&mut self, name: String, data: Vec<f32>) {
        let bytes = data.len() * std::mem::size_of::<f32>();
        if bytes > TENSOR_CACHE_MAX_ENTRY_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if bytes > TENSOR_CACHE_MAX_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if let Some(existing) = self.entries.remove(&name) {
            self.total_bytes = self.total_bytes.saturating_sub(existing.bytes);
            self.order.retain(|v| v != &name);
        }
        while self.total_bytes.saturating_add(bytes) > TENSOR_CACHE_MAX_BYTES {
            if let Some(evicted) = self.order.pop_front() {
                if let Some(entry) = self.entries.remove(&evicted) {
                    self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
                    self.evictions = self.evictions.saturating_add(1);
                }
            } else {
                break;
            }
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.order.push_back(name.clone());
        self.entries.insert(
            name,
            TensorCacheEntry {
                data,
                bytes,
            },
        );
    }

    fn touch(&mut self, name: &str) {
        if let Some(pos) = self.order.iter().position(|v| v == name) {
            self.order.remove(pos);
            self.order.push_back(name.to_string());
        }
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            skipped: self.skipped,
            bytes: self.total_bytes,
            entries: self.entries.len(),
        }
    }
}

#[derive(Clone, Debug)]
struct TokenCacheEntry {
    data: Vec<f32>,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct TokenCache {
    entries: HashMap<u32, TokenCacheEntry>,
    order: VecDeque<u32>,
    total_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    skipped: u64,
}

impl TokenCache {
    fn new() -> Self {
        Self {
            entries: HashMap::default(),
            order: VecDeque::new(),
            total_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            skipped: 0,
        }
    }

    fn get(&mut self, token_id: u32) -> Option<Vec<f32>> {
        let hit = self.entries.get(&token_id).map(|entry| entry.data.clone());
        if hit.is_some() {
            self.hits = self.hits.saturating_add(1);
            self.touch(token_id);
            return hit;
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    fn insert(&mut self, token_id: u32, data: Vec<f32>) {
        let bytes = data.len() * std::mem::size_of::<f32>();
        if bytes > TOKEN_CACHE_MAX_ENTRY_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if bytes > TOKEN_CACHE_MAX_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if let Some(existing) = self.entries.remove(&token_id) {
            self.total_bytes = self.total_bytes.saturating_sub(existing.bytes);
            self.order.retain(|v| *v != token_id);
        }
        while self.total_bytes.saturating_add(bytes) > TOKEN_CACHE_MAX_BYTES {
            if let Some(evicted) = self.order.pop_front() {
                if let Some(entry) = self.entries.remove(&evicted) {
                    self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
                    self.evictions = self.evictions.saturating_add(1);
                }
            } else {
                break;
            }
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.order.push_back(token_id);
        self.entries.insert(
            token_id,
            TokenCacheEntry {
                data,
                bytes,
            },
        );
    }

    fn touch(&mut self, token_id: u32) {
        if let Some(pos) = self.order.iter().position(|v| *v == token_id) {
            self.order.remove(pos);
            self.order.push_back(token_id);
        }
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            skipped: self.skipped,
            bytes: self.total_bytes,
            entries: self.entries.len(),
        }
    }
}

#[derive(Clone, Debug)]
struct QuantCacheEntry {
    data: Vec<u8>,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct QuantCache {
    entries: HashMap<String, QuantCacheEntry>,
    order: VecDeque<String>,
    total_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    skipped: u64,
}

impl QuantCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            total_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            skipped: 0,
        }
    }

    fn get(&mut self, name: &str) -> Option<Vec<u8>> {
        let hit = self.entries.get(name).map(|entry| entry.data.clone());
        if hit.is_some() {
            self.hits = self.hits.saturating_add(1);
            self.touch(name);
            return hit;
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    fn insert(&mut self, name: String, data: Vec<u8>) {
        let bytes = data.len();
        if bytes > Q8_0_CACHE_MAX_ENTRY_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if bytes > Q8_0_CACHE_MAX_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if let Some(existing) = self.entries.remove(&name) {
            self.total_bytes = self.total_bytes.saturating_sub(existing.bytes);
            self.order.retain(|v| v != &name);
        }
        while self.total_bytes.saturating_add(bytes) > Q8_0_CACHE_MAX_BYTES {
            if let Some(evicted) = self.order.pop_front() {
                if let Some(entry) = self.entries.remove(&evicted) {
                    self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
                    self.evictions = self.evictions.saturating_add(1);
                }
            } else {
                break;
            }
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.order.push_back(name.clone());
        self.entries.insert(
            name,
            QuantCacheEntry {
                data,
                bytes,
            },
        );
    }

    fn touch(&mut self, name: &str) {
        if let Some(pos) = self.order.iter().position(|v| v == name) {
            self.order.remove(pos);
            self.order.push_back(name.to_string());
        }
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            skipped: self.skipped,
            bytes: self.total_bytes,
            entries: self.entries.len(),
        }
    }
}

#[derive(Clone, Debug)]
struct ExpertCacheEntry {
    data: Vec<u8>,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct ExpertCache {
    entries: HashMap<String, ExpertCacheEntry>,
    order: VecDeque<String>,
    total_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    skipped: u64,
}

impl ExpertCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            total_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            skipped: 0,
        }
    }

    fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        let hit = self.entries.get(key).map(|entry| entry.data.clone());
        if hit.is_some() {
            self.hits = self.hits.saturating_add(1);
            self.touch(key);
            return hit;
        }
        self.misses = self.misses.saturating_add(1);
        None
    }

    fn insert(&mut self, key: String, data: Vec<u8>) {
        let bytes = data.len();
        if bytes > EXPERT_CACHE_MAX_ENTRY_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if bytes > EXPERT_CACHE_MAX_BYTES {
            self.skipped = self.skipped.saturating_add(1);
            return;
        }
        if let Some(existing) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(existing.bytes);
            self.order.retain(|v| v != &key);
        }
        while self.total_bytes.saturating_add(bytes) > EXPERT_CACHE_MAX_BYTES {
            if let Some(evicted) = self.order.pop_front() {
                if let Some(entry) = self.entries.remove(&evicted) {
                    self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
                    self.evictions = self.evictions.saturating_add(1);
                }
            } else {
                break;
            }
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.order.push_back(key.clone());
        self.entries.insert(
            key,
            ExpertCacheEntry {
                data,
                bytes,
            },
        );
    }

    fn touch(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|v| v == key) {
            self.order.remove(pos);
            self.order.push_back(key.to_string());
        }
    }

    fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
            skipped: self.skipped,
            bytes: self.total_bytes,
            entries: self.entries.len(),
        }
    }
}

struct RuntimeCaches {
    token_embd: TokenCache,
    tensors: TensorCache,
    quant: QuantCache,
    experts: ExpertCache,
    moe_disabled: bool,
}

impl RuntimeCaches {
    fn new() -> Self {
        Self {
            token_embd: TokenCache::new(),
            tensors: TensorCache::new(),
            quant: QuantCache::new(),
            experts: ExpertCache::new(),
            moe_disabled: false,
        }
    }
}

#[derive(Default)]
struct GpuAllocTracker {
    bytes: usize,
    buffers: usize,
}

impl GpuAllocTracker {
    fn reset(&mut self) {
        self.bytes = 0;
        self.buffers = 0;
    }

    fn add_buffers(&mut self, bytes: usize, buffers: usize) {
        self.bytes = self.bytes.saturating_add(bytes);
        self.buffers = self.buffers.saturating_add(buffers);
    }
}

pub(crate) struct GptOssRuntime {
    pub(crate) gguf_source: GgufSource,
    pub(crate) gguf_label: String,
    pub(crate) gpu: GpuContext,
    pub(crate) index: Option<GgufIndex>,
}

impl GptOssRuntime {
    pub(crate) fn new(gguf_source: GgufSource, gpu: GpuContext) -> Self {
        let gguf_label = gguf_source.label();
        Self {
            gguf_source,
            gguf_label,
            gpu,
            index: None,
        }
    }

    pub(crate) async fn load_index(
        &mut self,
        initial_bytes: u64,
        max_attempts: usize,
    ) -> Result<&GgufIndex, String> {
        let index =
            fetch_and_parse_index_source(&self.gguf_source, initial_bytes, max_attempts).await?;
        self.index = Some(index);
        Ok(self.index.as_ref().expect("index set"))
    }

    pub(crate) async fn read_tensor_slice(
        &self,
        tensor: &GgufTensor,
        len: usize,
    ) -> Result<Vec<u8>, String> {
        let bytes =
            fetch_range_source(&self.gguf_source, tensor.absolute_offset, len as u64).await?;
        Ok(bytes)
    }
}

pub(crate) fn start_gptoss_load(state: Rc<RefCell<AppState>>) {
    let (raw_url, file_opt) = {
        let input_override = state
            .try_borrow()
            .ok()
            .and_then(|guard| {
                let value = guard.gptoss.gguf_input.get_value().trim().to_string();
                if value.is_empty() {
                    None
                } else {
                    Some(value)
                }
            });
        let file = state
            .try_borrow()
            .ok()
            .and_then(|guard| guard.gptoss.gguf_file.clone());
        let raw_url = input_override
            .or_else(|| read_query_param("gguf").filter(|url| !url.is_empty()))
            .unwrap_or_default();
        (raw_url, file)
    };

    let mut gguf_source = None;
    let mut gguf_label = None;

    let wants_file = is_file_input(&raw_url) || (raw_url.trim().is_empty() && file_opt.is_some());
    if wants_file {
        if let Some(file) = file_opt.clone() {
            gguf_label = Some(gguf_file_label(&file));
            gguf_source = Some(GgufSource::File(file));
        } else {
            start_gptoss_file_pick(state);
            return;
        }
    }

    if gguf_source.is_none() {
        let gguf_url = match normalize_gguf_url(&raw_url) {
            Ok(url) => url,
            Err(err) => {
                if let Ok(mut guard) = state.try_borrow_mut() {
                    reset_gptoss_state(&mut guard.gptoss);
                    guard.gptoss.load_error = Some(err.clone());
                }
                emit_load_stage(
                    &state,
                    "load_failed",
                    StageStatus::Failed,
                    Some(err),
                    None,
                    None,
                );
                return;
            }
        };
        if gguf_url.is_empty() {
            start_gptoss_file_pick(state);
            return;
        }
        gguf_label = Some(gguf_url.clone());
        gguf_source = Some(GgufSource::Url(gguf_url));
    }

    let gguf_source = gguf_source.expect("gguf source set");
    let gguf_label = gguf_label.unwrap_or_else(|| gguf_source.label());

    {
        let Ok(mut guard) = state.try_borrow_mut() else {
            return;
        };
        if guard.gptoss.load_active {
            return;
        }
        reset_gptoss_state(&mut guard.gptoss);
        if guard.gptoss.gguf_input.get_value().trim().is_empty() {
            guard.gptoss.gguf_input.set_value(gguf_label.clone());
        }
        guard.gptoss.load_active = true;
        guard.gptoss.load_error = None;
        guard.gptoss.load_url = Some(gguf_label.clone());
    }

    let state_clone = state.clone();
    spawn_local(async move {
        if let Err(err) = run_gptoss_load(state_clone.clone(), gguf_source, gguf_label).await {
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

pub(crate) fn start_gptoss_file_pick(state: Rc<RefCell<AppState>>) {
    if let Ok(guard) = state.try_borrow() {
        if guard.gptoss.load_active {
            return;
        }
    }
    let state_clone = state.clone();
    spawn_local(async move {
        let file = match pick_gguf_file().await {
            Ok(file) => file,
            Err(err) => {
                if let Ok(mut guard) = state_clone.try_borrow_mut() {
                    guard.gptoss.load_error = Some(err);
                }
                return;
            }
        };
        let file_name = file.name().to_ascii_lowercase();
        if !file_name.ends_with(".gguf") {
            if let Ok(mut guard) = state_clone.try_borrow_mut() {
                guard.gptoss.load_error = Some("Selected file is not a .gguf".to_string());
            }
            return;
        }
        let input_label = gguf_file_input_label(&file);
        let display_label = gguf_file_label(&file);
        if let Ok(mut guard) = state_clone.try_borrow_mut() {
            guard.gptoss.gguf_file = Some(file);
            guard.gptoss.gguf_file_label = Some(display_label);
            guard.gptoss.gguf_input.set_value(input_label);
            guard.gptoss.load_error = None;
        }
        start_gptoss_load(state_clone);
    });
}

async fn run_gptoss_load(
    state: Rc<RefCell<AppState>>,
    gguf_source: GgufSource,
    gguf_label: String,
) -> Result<(), String> {
    emit_load_stage(
        &state,
        "load_start",
        StageStatus::Started,
        Some(format!("source={}", gguf_label)),
        None,
        None,
    );

    emit_load_stage(
        &state,
        "range_check",
        StageStatus::Started,
        None,
        None,
        None,
    );

    let (_probe, total) = fetch_range_with_total_source(&gguf_source, 0, 1)
        .await
        .map_err(|err| format_source_error(&gguf_source, &gguf_label, &err))?;
    let total_bytes = total.ok_or_else(|| {
        "Host does not support Range/CORS. Start gguf_serve.".to_string()
    })?;
    emit_load_stage(
        &state,
        "range_check",
        StageStatus::Completed,
        Some(format!("total={}", format_bytes(total_bytes))),
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
        fetch_and_parse_index_source(&gguf_source, DEFAULT_METADATA_BYTES, MAX_METADATA_ATTEMPTS)
            .await?,
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
    emit_metadata_keys(&state, index.as_ref(), 18);
    let config = parse_config(index.as_ref())?;
    let (input_layers, input_max_kv, input_max_new) = read_input_overrides(&state)?;
    let requested_layers = input_layers.or_else(|| read_query_usize("layers"));
    let max_layers = config.block_count as usize;
    let mut active_layers = requested_layers.unwrap_or(max_layers);
    active_layers = active_layers.min(max_layers);
    let layer_detail = if active_layers == 0 {
        "layers=0 (lm_head only)".to_string()
    } else if active_layers == max_layers {
        if requested_layers.is_none() {
            format!("layers={active_layers}/{max_layers} default")
        } else {
            format!("layers={active_layers}/{max_layers}")
        }
    } else if requested_layers.is_none() {
        format!("layers={active_layers}/{max_layers} default")
    } else {
        format!("layers={active_layers}/{max_layers}")
    };
    emit_load_stage(
        &state,
        "layer_limit",
        StageStatus::Completed,
        Some(layer_detail),
        None,
        None,
    );
    let force_dense = read_query_param("attn")
        .map(|value| matches!(value.as_str(), "dense" | "full" | "0"))
        .unwrap_or(false);
    let moe_fallback = read_query_param("moe")
        .map(|value| matches!(value.as_str(), "fallback" | "off" | "0"))
        .unwrap_or(false);
    emit_config(&state, &config);
    if moe_fallback {
        emit_load_stage(
            &state,
            "moe_mode",
            StageStatus::Completed,
            Some("fallback expert=0".to_string()),
            None,
            None,
        );
    }

    let gpu = state
        .borrow()
        .gpu_context
        .clone()
        .ok_or_else(|| "WebGPU device unavailable (enable WebGPU in Chrome)".to_string())?;
    emit_gpu_limits(&state, &gpu);
    let mut max_kv_tokens = {
        let mut max_kv = input_max_kv
            .or_else(|| read_query_usize("max_kv"))
            .unwrap_or(DEFAULT_MAX_KV_TOKENS);
        if config.context_length > 0 {
            max_kv = max_kv.min(config.context_length as usize);
        }
        max_kv.max(1)
    };
    let kv_limit = kv_limit_for_gpu(&config, &gpu, DEFAULT_KV_BUDGET_BYTES);
    let mut kv_clamp: Option<(usize, KvLimit)> = None;
    if let Some(limit) = kv_limit {
        if max_kv_tokens > limit.max_tokens {
            kv_clamp = Some((max_kv_tokens, limit));
            max_kv_tokens = limit.max_tokens.max(1);
        }
    }
    let max_new_tokens = input_max_new
        .or_else(|| read_query_usize("max_new"))
        .unwrap_or(DEFAULT_MAX_NEW_TOKENS)
        .max(1);
    let max_new_tokens = max_new_tokens
        .min(max_kv_tokens.saturating_sub(1).max(1));
    let max_prompt_tokens = max_kv_tokens.saturating_sub(max_new_tokens);
    let mut limit_detail = format!(
        "kv={max_kv_tokens} prompt={max_prompt_tokens} new={max_new_tokens}"
    );
    if let Some((requested, limit)) = kv_clamp {
        limit_detail.push_str(&format!(" clamp={requested}->{}", limit.max_tokens));
        if let Some(budget_max) = limit.budget_max {
            if budget_max == limit.max_tokens && budget_max < limit.per_layer_max {
                limit_detail.push_str(&format!(" budget={}", format_bytes(limit.budget_bytes)));
            }
        }
    }
    emit_load_stage(
        &state,
        "token_limits",
        StageStatus::Completed,
        Some(limit_detail),
        None,
        None,
    );

    let sampling_overrides = read_sampling_overrides(&state)?;
    let sampling = parse_sampling_config(sampling_overrides);

    let tokenizer = build_tokenizer(&state, index.as_ref())?;
    let prompt_tokens = encode_prompt(&state, &tokenizer, max_prompt_tokens)?;
    let stop_tokens = collect_stop_tokens(&tokenizer);

    {
        let gguf = gguf_source.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        let gpu_clone = gpu.clone();
        spawn_local(async move {
            if let Err(err) = run_q8_0_probe(&state_clone, &gguf, index_clone.as_ref(), &gpu_clone)
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
    {
        let gguf = gguf_source.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        let gpu_clone = gpu.clone();
        spawn_local(async move {
            if let Err(err) = run_mxfp4_probe(&state_clone, &gguf, index_clone.as_ref(), &gpu_clone)
                .await
            {
                emit_inference_stage(
                    &state_clone,
                    "mxfp4_probe",
                    StageStatus::Failed,
                    None,
                    None,
                    Some(err),
                );
            }
        });
    }
    {
        let gguf = gguf_source.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        let gpu_clone = gpu.clone();
        let config_clone = config.clone();
        spawn_local(async move {
            if let Err(err) = run_rmsnorm_probe(
                &state_clone,
                &gguf,
                index_clone.as_ref(),
                &config_clone,
                &gpu_clone,
            )
            .await
            {
                emit_inference_stage(
                    &state_clone,
                    "rmsnorm_probe",
                    StageStatus::Failed,
                    None,
                    None,
                    Some(err),
                );
            }
        });
    }
    {
        let gguf = gguf_source.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        let gpu_clone = gpu.clone();
        let config_clone = config.clone();
        spawn_local(async move {
            if let Err(err) =
                run_rope_probe(&state_clone, &gguf, index_clone.as_ref(), &config_clone, &gpu_clone)
                    .await
            {
                emit_inference_stage(
                    &state_clone,
                    "rope_probe",
                    StageStatus::Failed,
                    None,
                    None,
                    Some(err),
                );
            }
        });
    }
    {
        let gguf = gguf_source.clone();
        let state_clone = state.clone();
        let index_clone = index.clone();
        let gpu_clone = gpu.clone();
        let config_clone = config.clone();
        spawn_local(async move {
            if let Err(err) = run_attention_probe(
                &state_clone,
                &gguf,
                index_clone.as_ref(),
                &config_clone,
                &gpu_clone,
            )
            .await
            {
                emit_inference_stage(
                    &state_clone,
                    "attn_probe",
                    StageStatus::Failed,
                    None,
                    None,
                    Some(err),
                );
            }
        });
    }

    let stream_state = state.clone();
    let stream_source = gguf_source.clone();
    let stream_index = index.clone();
    let stream_future = async move {
        stream_full_weights(&stream_state, &stream_source, stream_index.as_ref(), total_bytes)
            .await
    };

    let gen_state = state.clone();
    let gen_source = gguf_source.clone();
    let gen_index = index.clone();
    let gen_future = async move {
        if let Err(err) = run_generation(
            &gen_state,
            &gen_source,
            gen_index.as_ref(),
            &gpu,
            &tokenizer,
            &config,
            &prompt_tokens,
            active_layers,
            moe_fallback,
            max_kv_tokens,
            max_new_tokens,
            force_dense,
            sampling,
            stop_tokens,
        )
        .await
        {
            emit_inference_stage(
                &gen_state,
                "generation",
                StageStatus::Failed,
                None,
                None,
                Some(err),
            );
        }
    };

    let (stream_res, _) = futures::join!(stream_future, gen_future);
    if let Err(err) = stream_res {
        emit_load_stage(
            &state,
            "weights_fetch",
            StageStatus::Failed,
            Some(format!("stream error: {err}")),
            None,
            None,
        );
    }

    if let Ok(mut guard) = state.try_borrow_mut() {
        guard.gptoss.load_active = false;
    }
    Ok(())
}

async fn stream_full_weights(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    total_bytes: u64,
) -> Result<(), String> {
    let start_ms = now_ms();
    emit_load_stage(
        state,
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
    let mut tensor_cursor = tensor_start_cursor(index);
    let mut tensor_emitted = 0usize;

    while offset < total_bytes {
        let len = (total_bytes - offset).min(LOAD_CHUNK_BYTES);
        let chunk = fetch_range_source(gguf_source, offset, len).await?;
        loaded = loaded.saturating_add(chunk.len() as u64);
        offset = offset.saturating_add(len);
        chunk_idx = chunk_idx.saturating_add(1);

        if loaded >= next_progress || loaded >= total_bytes {
            let now = now_ms();
            let elapsed_ms = now.saturating_sub(start_ms).max(1);
            let rate_value = loaded as f64 / (elapsed_ms as f64 / 1000.0);
            let rate = format_rate(rate_value);
            let eta = if rate_value > 0.0 {
                let remaining = total_bytes.saturating_sub(loaded) as f64;
                format!("{:.1}s", remaining / rate_value)
            } else {
                "--".to_string()
            };
            emit_load_stage(
                state,
                "weights_fetch",
                StageStatus::Progress,
                Some(format!(
                    "chunk={} offset={} rate={} eta={}",
                    chunk_idx,
                    format_bytes(offset),
                    rate,
                    eta
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
                    state,
                    "tensor_scan",
                    StageStatus::Progress,
                    Some(name),
                    Some(loaded),
                    Some(total_bytes),
                );
            }
        }

        yield_to_browser().await;
    }

    emit_load_stage(
        state,
        "weights_fetch",
        StageStatus::Completed,
        Some(format!(
            "loaded={} elapsed={:.1}s",
            format_bytes(loaded),
            (now_ms().saturating_sub(start_ms) as f32 / 1000.0).max(0.1)
        )),
        Some(loaded),
        Some(total_bytes),
    );

    emit_load_stage(
        state,
        "load_complete",
        StageStatus::Completed,
        None,
        Some(loaded),
        Some(total_bytes),
    );
    Ok(())
}

fn reset_gptoss_state(state: &mut crate::state::GptOssVizState) {
    clear_gptoss_events();
    state.load_stages.clear();
    state.inference_stages.clear();
    state.events.clear();
    state.token_stream.clear();
    state.last_token_id = None;
    state.top_k.clear();
    state.probability_history.clear();
    state.tokens_per_sec = None;
    state.entropy = None;
    state.entropy_history.clear();
    state.memory_usage = None;
    state.gpu_limits = None;
    state.token_limits = None;
    state.cache_status.clear();
    state.resident_tensors.clear();
    state.recent_tensors.clear();
    state.attention_weights = None;
    state.attention_layer = 0;
    state.attention_head = 0;
    state.attention_selected_layer = 0;
    state.attention_selected_head = 0;
    state.layer_activations.clear();
    state.max_layers = 1;
    state.max_heads = 1;
    state.layer_slider_bounds = wgpui::Bounds::ZERO;
    state.head_slider_bounds = wgpui::Bounds::ZERO;
    state.layer_slider_dragging = false;
    state.head_slider_dragging = false;
    state.drop_active = false;
    state.attention_mode = None;
    state.moe_mode = None;
    state.sampling_mode = None;
    state.cpu_fallback = None;
    state.active_layers = None;
    state.load_progress = None;
    state.current_stage = None;
    state.inference_error = None;
    state.last_token_ts_ms = None;
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

fn emit_metadata_keys(state: &Rc<RefCell<AppState>>, index: &GgufIndex, limit: usize) {
    let mut keys = index.metadata.values.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    for (idx, key) in keys.iter().take(limit).enumerate() {
        emit_load_stage(
            state,
            "gguf_meta",
            StageStatus::Progress,
            Some(format!("{}: {}", idx + 1, key)),
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

fn emit_tensor_resident(state: &Rc<RefCell<AppState>>, name: String, bytes: usize, kind: &str) {
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::TensorResident {
            name,
            bytes,
            kind: kind.to_string(),
        },
    );
}

fn emit_gpu_limits(state: &Rc<RefCell<AppState>>, gpu: &GpuContext) {
    let limits = gpu.device.limits();
    let features = gpu.device.features();
    let shader_f16 = features.contains(wgpu::Features::SHADER_F16);
    let detail = format!(
        "max_storage={} max_buffer={} bind_groups={} bindings_per_group={} storage_bindings={} dynamic_storage={} uniform_bindings={} f16={}",
        format_bytes(limits.max_storage_buffer_binding_size as u64),
        format_bytes(limits.max_buffer_size as u64),
        limits.max_bind_groups,
        limits.max_bindings_per_bind_group,
        limits.max_storage_buffers_per_shader_stage,
        limits.max_dynamic_storage_buffers_per_pipeline_layout,
        limits.max_uniform_buffers_per_shader_stage,
        shader_f16,
    );
    emit_load_stage(
        state,
        "gpu_limits",
        StageStatus::Completed,
        Some(detail),
        None,
        None,
    );
}

fn parse_config(index: &GgufIndex) -> Result<GptOssConfig, String> {
    let block_count = read_meta_u32(index, "llama.block_count")?;
    let context_length = read_meta_u32_optional(index, "gpt-oss.context_length")
        .or_else(|| read_meta_u32_optional(index, "llama.context_length"))
        .unwrap_or(0);
    let embedding_length = read_meta_u32(index, "llama.embedding_length")?;
    let feed_forward_length = read_meta_u32(index, "llama.feed_forward_length")?;
    let head_count = read_meta_u32(index, "llama.attention.head_count")?;
    let head_count_kv = read_meta_u32(index, "llama.attention.head_count_kv")?;
    let rope_dimension_count = read_meta_u32(index, "llama.rope.dimension_count")?;
    let rope_theta = read_meta_f32(index, "llama.rope.freq_base")?;
    let rope_scaling_factor =
        read_meta_f32_optional(index, "gpt-oss.rope.scaling.factor").unwrap_or(1.0);
    let rope_scaling_original_context = read_meta_u32_optional(
        index,
        "gpt-oss.rope.scaling.original_context_length",
    )
    .unwrap_or(0);
    let rms_epsilon = read_meta_f32(index, "llama.attention.layer_norm_rms_epsilon")?;
    let sliding_window = read_meta_u32(index, "llama.sliding_window")?;
    let expert_count = read_meta_u32(index, "llama.expert_count")?;
    let experts_per_token = read_meta_u32(index, "llama.expert_used_count")?;

    Ok(GptOssConfig {
        block_count,
        context_length,
        embedding_length,
        feed_forward_length,
        head_count,
        head_count_kv,
        rope_dimension_count,
        rope_theta,
        rope_scaling_factor,
        rope_scaling_original_context,
        rms_epsilon,
        sliding_window,
        expert_count,
        experts_per_token,
    })
}

fn emit_config(state: &Rc<RefCell<AppState>>, config: &GptOssConfig) {
    let rope_scale = if config.rope_scaling_factor > 1.0
        && config.rope_scaling_original_context > 0
    {
        format!(
            "yarn x{:.2} orig={}",
            config.rope_scaling_factor, config.rope_scaling_original_context
        )
    } else {
        "none".to_string()
    };
    emit_load_stage(
        state,
        "model_config",
        StageStatus::Completed,
        Some(format!(
            "blocks={} ctx={} embd={} ffn={} heads={} kv_heads={} rope_dim={} rope_theta={} rope_scale={} rms_eps={} window={} experts={} topk={}",
            config.block_count,
            if config.context_length > 0 {
                config.context_length.to_string()
            } else {
                "-".to_string()
            },
            config.embedding_length,
            config.feed_forward_length,
            config.head_count,
            config.head_count_kv,
            config.rope_dimension_count,
            config.rope_theta,
            rope_scale,
            config.rms_epsilon,
            config.sliding_window,
            config.expert_count,
            config.experts_per_token,
        )),
        None,
        None,
    );
}

fn read_meta_u32(index: &GgufIndex, key: &str) -> Result<u32, String> {
    let value = lookup_meta(index, key);
    if value.is_none() && key.ends_with("rope.dimension_count") {
        if let Ok(key_len) = read_meta_u32(index, "gpt-oss.attention.key_length") {
            return Ok(key_len);
        }
        if let Ok(value_len) = read_meta_u32(index, "gpt-oss.attention.value_length") {
            return Ok(value_len);
        }
        let embedding = read_meta_u32(index, "llama.embedding_length")?;
        let heads = read_meta_u32(index, "llama.attention.head_count")?;
        if heads > 0 {
            return Ok(embedding / heads);
        }
    }
    let Some(value) = value else {
        return Err(format!("missing gguf metadata key: {key}"));
    };
    match value {
        crate::gguf_web::GgufScalar::U32(v) => Ok(*v),
        crate::gguf_web::GgufScalar::I32(v) => Ok((*v).max(0) as u32),
        crate::gguf_web::GgufScalar::U64(v) => Ok((*v).min(u64::from(u32::MAX)) as u32),
        crate::gguf_web::GgufScalar::I64(v) => Ok((*v).max(0).min(i64::from(u32::MAX)) as u32),
        _ => Err(format!("gguf metadata {key} has non-integer type")),
    }
}

fn read_meta_u32_optional(index: &GgufIndex, key: &str) -> Option<u32> {
    let value = lookup_meta(index, key)?;
    let out = match value {
        crate::gguf_web::GgufScalar::U32(v) => *v,
        crate::gguf_web::GgufScalar::I32(v) => (*v).max(0) as u32,
        crate::gguf_web::GgufScalar::U64(v) => (*v).min(u64::from(u32::MAX)) as u32,
        crate::gguf_web::GgufScalar::I64(v) => (*v).max(0).min(i64::from(u32::MAX)) as u32,
        crate::gguf_web::GgufScalar::F32(v) => (*v).max(0.0) as u32,
        crate::gguf_web::GgufScalar::F64(v) => (*v).max(0.0) as u32,
        _ => return None,
    };
    Some(out)
}

fn read_meta_f32(index: &GgufIndex, key: &str) -> Result<f32, String> {
    let Some(value) = lookup_meta(index, key) else {
        return Err(format!("missing gguf metadata key: {key}"));
    };
    match value {
        crate::gguf_web::GgufScalar::F32(v) => Ok(*v),
        crate::gguf_web::GgufScalar::F64(v) => Ok(*v as f32),
        crate::gguf_web::GgufScalar::U32(v) => Ok(*v as f32),
        crate::gguf_web::GgufScalar::I32(v) => Ok(*v as f32),
        crate::gguf_web::GgufScalar::U64(v) => Ok(*v as f32),
        crate::gguf_web::GgufScalar::I64(v) => Ok(*v as f32),
        _ => Err(format!("gguf metadata {key} has non-float type")),
    }
}

fn read_meta_f32_optional(index: &GgufIndex, key: &str) -> Option<f32> {
    let value = lookup_meta(index, key)?;
    let out = match value {
        crate::gguf_web::GgufScalar::F32(v) => *v,
        crate::gguf_web::GgufScalar::F64(v) => *v as f32,
        crate::gguf_web::GgufScalar::U32(v) => *v as f32,
        crate::gguf_web::GgufScalar::I32(v) => *v as f32,
        crate::gguf_web::GgufScalar::U64(v) => *v as f32,
        crate::gguf_web::GgufScalar::I64(v) => *v as f32,
        _ => return None,
    };
    Some(out)
}

fn lookup_meta<'a>(index: &'a GgufIndex, key: &str) -> Option<&'a crate::gguf_web::GgufScalar> {
    if let Some(value) = index.metadata.values.get(key) {
        return Some(value);
    }
    if key == "llama.sliding_window" {
        return index.metadata.values.get("gpt-oss.attention.sliding_window");
    }
    if key == "gpt-oss.attention.sliding_window" {
        return index.metadata.values.get("llama.sliding_window");
    }
    let fallback = key
        .strip_prefix("llama.")
        .map(|rest| format!("gpt-oss.{rest}"))
        .or_else(|| key.strip_prefix("gpt-oss.").map(|rest| format!("llama.{rest}")))?;
    index.metadata.values.get(&fallback)
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
    max_prompt_tokens: usize,
) -> Result<Vec<u32>, String> {
    if max_prompt_tokens == 0 {
        return Err("prompt token limit is zero (raise max_kv or lower max_new)".to_string());
    }
    let user_prompt = state
        .try_borrow()
        .ok()
        .and_then(|guard| {
            let value = guard.gptoss.prompt_input.get_value().trim().to_string();
            if value.is_empty() {
                None
            } else {
                Some(value)
            }
        })
        .or_else(|| read_query_param("prompt").filter(|value| !value.is_empty()))
        .unwrap_or_else(default_user_prompt);
    let prompt = build_harmony_prompt(&user_prompt);

    emit_inference_stage(
        state,
        "prompt_encode",
        StageStatus::Started,
        Some(0),
        None,
        Some(format!("format=harmony chars={}", prompt.len())),
    );

    let mut tokens = tokenizer.encode_with_special_tokens(&prompt)?;
    let total = tokens.len();
    let mut truncated = 0usize;
    if total > max_prompt_tokens {
        truncated = total - max_prompt_tokens;
        tokens = tokens.split_off(total - max_prompt_tokens);
    }
    if tokens.is_empty() {
        return Err("prompt token list is empty".to_string());
    }

    emit_inference_stage(
        state,
        "prompt_encode",
        StageStatus::Completed,
        Some(tokens.len()),
        Some(tokens.len()),
        Some(format!(
            "format=harmony tokens={total} kept={} truncated={truncated}",
            tokens.len(),
        )),
    );

    Ok(tokens)
}

fn now_ms() -> u64 {
    js_sys::Date::now().max(0.0) as u64
}

async fn yield_to_browser() {
    TimeoutFuture::new(0).await;
}

pub(crate) fn read_query_param(key: &str) -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    params.get(key)
}

fn read_query_usize(key: &str) -> Option<usize> {
    read_query_param(key)
        .and_then(|value| value.parse::<usize>().ok())
}

fn read_query_f32(key: &str) -> Option<f32> {
    read_query_param(key)
        .and_then(|value| value.parse::<f32>().ok())
}

fn parse_usize_override(raw: &str, label: &str) -> Result<Option<usize>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<usize>()
        .map(Some)
        .map_err(|_| format!("invalid {label} value: {trimmed}"))
}

fn parse_layers_override(raw: &str) -> Result<Option<usize>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        return Ok(None);
    }
    trimmed
        .parse::<usize>()
        .map(Some)
        .map_err(|_| format!("invalid layers value: {trimmed}"))
}

fn parse_bool_override(raw: &str, label: &str) -> Result<Option<bool>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = match trimmed.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => {
            return Err(format!(
                "invalid {label} value: {trimmed} (use on/off)"
            ))
        }
    };
    Ok(Some(value))
}

fn parse_f32_override(raw: &str, label: &str) -> Result<Option<f32>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<f32>()
        .map(Some)
        .map_err(|_| format!("invalid {label} value: {trimmed}"))
}

fn read_input_overrides(
    state: &Rc<RefCell<AppState>>,
) -> Result<(Option<usize>, Option<usize>, Option<usize>), String> {
    let Ok(guard) = state.try_borrow() else {
        return Ok((None, None, None));
    };
    let layers = parse_layers_override(guard.gptoss.layers_input.get_value())?;
    let max_kv = parse_usize_override(guard.gptoss.max_kv_input.get_value(), "max_kv")?;
    let max_new = parse_usize_override(guard.gptoss.max_new_input.get_value(), "max_new")?;
    Ok((layers, max_kv, max_new))
}

fn read_sampling_overrides(state: &Rc<RefCell<AppState>>) -> Result<SamplingOverrides, String> {
    let Ok(guard) = state.try_borrow() else {
        return Ok(SamplingOverrides::default());
    };
    Ok(SamplingOverrides {
        enabled: parse_bool_override(guard.gptoss.sample_input.get_value(), "sample")?,
        temperature: parse_f32_override(guard.gptoss.temp_input.get_value(), "temp")?,
        top_k: parse_usize_override(guard.gptoss.top_k_input.get_value(), "top_k")?,
        top_p: parse_f32_override(guard.gptoss.top_p_input.get_value(), "top_p")?,
    })
}

pub(crate) fn default_gguf_url() -> String {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return String::new(),
    };
    let location = window.location();
    let host = location.hostname().ok();
    let local = matches!(host.as_deref(), Some("localhost") | Some("127.0.0.1"));
    if local {
        if let Ok(port) = location.port() {
            if port == "3000" {
                if let Ok(origin) = location.origin() {
                    return format!("{origin}{LOCAL_GGUF_ROUTE}");
                }
            }
        }
        LOCAL_GGUF_URL.to_string()
    } else {
        String::new()
    }
}

fn is_file_input(raw: &str) -> bool {
    let trimmed = raw.trim().to_ascii_lowercase();
    trimmed == "file" || trimmed.starts_with("file:")
}

pub(crate) fn gguf_file_input_label(file: &web_sys::File) -> String {
    format!("file:{}", file.name())
}

pub(crate) fn gguf_file_label(file: &web_sys::File) -> String {
    format!("file:{} ({})", file.name(), format_bytes(file.size() as u64))
}

fn normalize_gguf_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(trimmed.to_string());
    }
    if trimmed.starts_with('/') {
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("/users/") || lower.starts_with("/home/") {
            return Err(format!(
                "Local file paths are not supported in the browser.\nClick PICK FILE, drop a GGUF, or run: {LOCAL_GGUF_SERVE_CMD}\nThen use: {LOCAL_GGUF_URL}"
            ));
        }
        let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
        let origin = window
            .location()
            .origin()
            .map_err(|_| "failed to read window origin".to_string())?;
        return Ok(format!("{origin}{trimmed}"));
    }
    if trimmed.starts_with("file://") || trimmed.starts_with('~') {
        return Err(format!(
            "Local file paths are not supported in the browser.\nClick PICK FILE, drop a GGUF, or run: {LOCAL_GGUF_SERVE_CMD}\nThen use: {LOCAL_GGUF_URL}"
        ));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("localhost")
        || lower.starts_with("127.0.0.1")
        || lower.starts_with("0.0.0.0")
    {
        return Ok(format!("http://{trimmed}"));
    }
    Err("GGUF URL must start with http:// or https://".to_string())
}

pub(crate) fn local_gguf_path() -> &'static str {
    LOCAL_GGUF_PATH
}

pub(crate) fn local_gguf_url() -> &'static str {
    LOCAL_GGUF_URL
}

pub(crate) fn local_gguf_dev_url() -> &'static str {
    LOCAL_GGUF_DEV_URL
}

pub(crate) fn local_gguf_serve_cmd() -> &'static str {
    LOCAL_GGUF_SERVE_CMD
}

pub(crate) fn default_user_prompt() -> String {
    DEFAULT_USER_PROMPT.to_string()
}

pub(crate) fn default_max_kv_tokens() -> usize {
    DEFAULT_MAX_KV_TOKENS
}

pub(crate) fn default_max_new_tokens() -> usize {
    DEFAULT_MAX_NEW_TOKENS
}

pub(crate) fn default_sample_temp() -> f32 {
    DEFAULT_SAMPLE_TEMP
}

pub(crate) fn default_sample_top_k() -> usize {
    DEFAULT_SAMPLE_TOP_K
}

pub(crate) fn default_sample_top_p() -> f32 {
    DEFAULT_SAMPLE_TOP_P
}

fn is_local_url(url: &str) -> bool {
    let url = url.to_ascii_lowercase();
    url.starts_with("http://localhost")
        || url.starts_with("http://127.0.0.1")
        || url.starts_with("https://localhost")
        || url.starts_with("https://127.0.0.1")
}

fn is_bun_dev_url(url: &str) -> bool {
    let url = url.to_ascii_lowercase();
    url.starts_with("http://localhost:3000")
        || url.starts_with("http://127.0.0.1:3000")
}

fn format_source_error(source: &GgufSource, label: &str, err: &str) -> String {
    if source.is_file() {
        return format!("Local GGUF read failed ({label}): {err}");
    }
    format_range_error(label, err)
}

fn format_range_error(url: &str, err: &str) -> String {
    let lower = err.to_ascii_lowercase();
    let detail = if lower.contains("fetch failed: 404") || lower.contains(" 404") {
        format!("GGUF not found at {url}")
    } else if lower.contains("fetch failed: 416") || lower.contains(" 416") {
        format!("Range request rejected by {url}")
    } else if lower.contains("failed to fetch")
        || lower.contains("networkerror")
        || lower.contains("load failed")
    {
        format!("Cannot connect to {url}")
    } else {
        format!("Range/CORS check failed for {url}: {err}")
    };

    if is_bun_dev_url(url) {
        format!("{detail}\nRun: cd crates/web && bun run build && bun run dev")
    } else if is_local_url(url) {
        format!("{detail}\nRun: {LOCAL_GGUF_SERVE_CMD}\nOr click PICK FILE / drop a GGUF")
    } else if detail.contains("Range/CORS") {
        detail
    } else {
        format!("{detail}. Host must support Range + CORS.")
    }
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
    let developer_prompt = if DEFAULT_DEVELOPER_PROMPT.is_empty() {
        "# Instructions\n\n".to_string()
    } else {
        format!("# Instructions\n\n{DEFAULT_DEVELOPER_PROMPT}")
    };

    let mut prompt = String::new();
    prompt.push_str("<|start|>system<|message|>");
    prompt.push_str(&system_prompt);
    prompt.push_str("<|end|><|start|>developer<|message|>");
    prompt.push_str(&developer_prompt);
    prompt.push_str("<|end|><|start|>user<|message|>");
    prompt.push_str(user_prompt);
    prompt.push_str("<|end|><|start|>assistant<|channel|>final<|message|>");
    prompt
}

fn parse_sampling_config(overrides: SamplingOverrides) -> SamplingConfig {
    let mut enabled = false;
    let input_present = overrides.enabled.is_some()
        || overrides.temperature.is_some()
        || overrides.top_k.is_some()
        || overrides.top_p.is_some();

    let temp = if let Some(value) = overrides.temperature {
        enabled = true;
        value
    } else if !input_present {
        read_query_f32("temp")
            .map(|value| {
                enabled = true;
                value
            })
            .unwrap_or(1.0)
    } else {
        1.0
    };

    let top_k = if let Some(value) = overrides.top_k {
        enabled = true;
        value
    } else if !input_present {
        read_query_usize("top_k")
            .map(|value| {
                enabled = true;
                value
            })
            .unwrap_or(DEFAULT_SAMPLE_TOP_K)
    } else {
        DEFAULT_SAMPLE_TOP_K
    };

    let top_p = if let Some(value) = overrides.top_p {
        enabled = true;
        value
    } else if !input_present {
        read_query_f32("top_p")
            .map(|value| {
                enabled = true;
                value
            })
            .unwrap_or(1.0)
    } else {
        1.0
    };

    if let Some(flag) = overrides.enabled {
        enabled = flag;
    } else if !input_present {
        if let Some(flag) = read_query_param("sample") {
            enabled = matches!(flag.as_str(), "1" | "true" | "yes" | "on");
        }
    }

    SamplingConfig {
        enabled,
        temperature: temp.max(1e-4),
        top_k,
        top_p: top_p.clamp(0.0, 1.0),
    }
}

fn collect_stop_tokens(tokenizer: &GptOssTokenizer) -> Vec<u32> {
    let mut tokens = Vec::new();
    for name in ["<|return|>", "<|call|>"] {
        if let Some(id) = tokenizer.token_id(name) {
            tokens.push(id);
        }
    }
    if let Some(id) = tokenizer.eos_token_id() {
        tokens.push(id);
    }
    tokens.sort_unstable();
    tokens.dedup();
    tokens
}

fn coherence_score(text: &str) -> f32 {
    let mut total = 0u32;
    let mut readable = 0u32;
    for ch in text.chars() {
        total += 1;
        if ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace() || ch.is_ascii_punctuation() {
            readable += 1;
        }
    }
    if total == 0 {
        return 0.0;
    }
    readable as f32 / total as f32
}

fn hex_preview(bytes: &[u8], len: usize) -> String {
    let take = bytes.len().min(len);
    let mut out = String::new();
    for (idx, byte) in bytes.iter().take(take).enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

fn top_k_from_logits(
    logits: &[f32],
    tokenizer: &GptOssTokenizer,
    k: usize,
) -> Result<(Vec<GptOssTokenCandidate>, f32, u32, String), String> {
    if logits.is_empty() {
        return Err("empty logits".to_string());
    }
    let mut max_logit = f32::NEG_INFINITY;
    for &logit in logits {
        if logit > max_logit {
            max_logit = logit;
        }
    }

    let mut sum_exp = 0.0f32;
    for &logit in logits {
        sum_exp += (logit - max_logit).exp();
    }
    if sum_exp <= 0.0 {
        return Err("softmax sum is zero".to_string());
    }

    let mut entropy = 0.0f32;
    for &logit in logits {
        let p = (logit - max_logit).exp() / sum_exp;
        if p > 0.0 {
            entropy -= p * p.ln();
        }
    }

    let mut top: Vec<(usize, f32)> = Vec::with_capacity(k.min(logits.len()));
    for (idx, &logit) in logits.iter().enumerate() {
        if top.len() < k {
            top.push((idx, logit));
            top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        } else if let Some(last) = top.last() {
            if logit > last.1 {
                top.pop();
                top.push((idx, logit));
                top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }

    let mut candidates = Vec::with_capacity(top.len());
    for (idx, logit) in top.iter() {
        let prob = (logit - max_logit).exp() / sum_exp;
        let token_id = *idx as u32;
        candidates.push(GptOssTokenCandidate {
            token_id,
            token_text: tokenizer.decode_utf8_lossy(&[token_id]),
            probability: prob,
        });
    }

    let (best_idx, _) = top
        .first()
        .copied()
        .unwrap_or((0usize, logits[0]));
    let best_token_id = best_idx as u32;
    let best_text = tokenizer.decode_utf8_lossy(&[best_token_id]);

    Ok((candidates, entropy, best_token_id, best_text))
}

fn sample_from_logits(
    logits: &[f32],
    tokenizer: &GptOssTokenizer,
    sampling: SamplingConfig,
    display_k: usize,
) -> Result<(Vec<GptOssTokenCandidate>, f32, u32, String), String> {
    let (top_k, entropy, best_id, best_text) = top_k_from_logits(logits, tokenizer, display_k)?;
    if !sampling.enabled {
        return Ok((top_k, entropy, best_id, best_text));
    }

    let effective_top_k = if sampling.top_k == 0 {
        DEFAULT_SAMPLE_TOP_K
    } else {
        sampling.top_k
    };
    let (mut indices, mut weights) =
        top_k_softmax_scaled(logits, effective_top_k, sampling.temperature)?;
    apply_top_p(&mut indices, &mut weights, sampling.top_p);
    let selected = sample_index(&indices, &weights);
    let token_id = selected as u32;
    let token_text = tokenizer.decode_utf8_lossy(&[token_id]);

    Ok((top_k, entropy, token_id, token_text))
}

fn top_k_softmax(values: &[f32], k: usize) -> Result<(Vec<usize>, Vec<f32>), String> {
    if values.is_empty() {
        return Err("empty values".to_string());
    }
    let k = k.max(1).min(values.len());
    let mut top: Vec<(usize, f32)> = Vec::with_capacity(k);
    for (idx, &value) in values.iter().enumerate() {
        if top.len() < k {
            top.push((idx, value));
            top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        } else if let Some(last) = top.last() {
            if value > last.1 {
                top.pop();
                top.push((idx, value));
                top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }

    let mut max_val = f32::NEG_INFINITY;
    for &(_, value) in &top {
        if value > max_val {
            max_val = value;
        }
    }
    let mut sum = 0.0f32;
    for &(_, value) in &top {
        sum += (value - max_val).exp();
    }
    if sum <= 0.0 {
        return Err("softmax sum is zero".to_string());
    }

    let mut indices = Vec::with_capacity(top.len());
    let mut weights = Vec::with_capacity(top.len());
    for &(idx, value) in &top {
        indices.push(idx);
        weights.push((value - max_val).exp() / sum);
    }

    Ok((indices, weights))
}

fn top_k_softmax_scaled(
    values: &[f32],
    k: usize,
    temperature: f32,
) -> Result<(Vec<usize>, Vec<f32>), String> {
    if values.is_empty() {
        return Err("empty values".to_string());
    }
    let temp = temperature.max(1e-6);
    let k = k.max(1).min(values.len());
    let mut top: Vec<(usize, f32)> = Vec::with_capacity(k);
    for (idx, &value) in values.iter().enumerate() {
        if top.len() < k {
            top.push((idx, value));
            top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        } else if let Some(last) = top.last() {
            if value > last.1 {
                top.pop();
                top.push((idx, value));
                top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }

    let mut max_val = f32::NEG_INFINITY;
    for &(_, value) in &top {
        let scaled = value / temp;
        if scaled > max_val {
            max_val = scaled;
        }
    }
    let mut sum = 0.0f32;
    for &(_, value) in &top {
        sum += (value / temp - max_val).exp();
    }
    if sum <= 0.0 {
        return Err("softmax sum is zero".to_string());
    }

    let mut indices = Vec::with_capacity(top.len());
    let mut weights = Vec::with_capacity(top.len());
    for &(idx, value) in &top {
        indices.push(idx);
        weights.push((value / temp - max_val).exp() / sum);
    }

    Ok((indices, weights))
}

fn apply_top_p(indices: &mut Vec<usize>, weights: &mut Vec<f32>, top_p: f32) {
    if indices.is_empty() || weights.is_empty() || top_p >= 1.0 {
        return;
    }
    let mut cumulative = 0.0f32;
    let mut cutoff = weights.len();
    for (idx, weight) in weights.iter().enumerate() {
        cumulative += *weight;
        if cumulative >= top_p {
            cutoff = idx + 1;
            break;
        }
    }
    indices.truncate(cutoff);
    weights.truncate(cutoff);
    let sum: f32 = weights.iter().sum();
    if sum > 0.0 {
        for weight in weights.iter_mut() {
            *weight /= sum;
        }
    }
}

fn sample_index(indices: &[usize], weights: &[f32]) -> usize {
    if indices.is_empty() || weights.is_empty() {
        return 0;
    }
    let draw = js_sys::Math::random() as f32;
    let mut cumulative = 0.0f32;
    for (idx, weight) in weights.iter().enumerate() {
        cumulative += *weight;
        if draw <= cumulative {
            return indices[idx];
        }
    }
    *indices.last().unwrap_or(&0)
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

fn format_rate(bytes_per_sec: f64) -> String {
    let rate = bytes_per_sec.max(0.0) as u64;
    format!("{}/s", format_bytes(rate))
}

fn ensure_storage_limit(label: &str, size: usize, max: usize) -> Result<(), String> {
    if size > max {
        return Err(format!(
            "{label} size {} exceeds max storage {}",
            format_bytes(size as u64),
            format_bytes(max as u64)
        ));
    }
    Ok(())
}

fn ensure_buffer_limit(
    label: &str,
    size: usize,
    max_storage: u64,
    max_buffer: u64,
) -> Result<(), String> {
    if size as u64 > max_storage {
        return Err(format!(
            "{label} size {} exceeds max storage {}",
            format_bytes(size as u64),
            format_bytes(max_storage)
        ));
    }
    if size as u64 > max_buffer {
        return Err(format!(
            "{label} size {} exceeds max buffer {}",
            format_bytes(size as u64),
            format_bytes(max_buffer)
        ));
    }
    Ok(())
}

async fn run_q8_0_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
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

    let mut quant =
        fetch_range_source(gguf_source, tensor.absolute_offset, bytes_needed as u64).await?;
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }

    let x = build_input(k);
    let weights = dequant_q8_0(&quant, values)?;
    let y_cpu = matmul_cpu(&weights, &x, k, n);

    let mut gpu_tracker = GpuAllocTracker::default();
    let y_gpu = gpu_matmul_q8_0(&quant, &x, k, n, gpu, &mut gpu_tracker).await?;
    let (max_abs, mean_abs) = diff_stats(&y_cpu, &y_gpu);

    emit_inference_event(
        state,
        GptOssInferenceTelemetry::MemoryUsage {
            gpu_allocated: gpu_tracker.bytes,
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

async fn run_mxfp4_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
) -> Result<(), String> {
    let tensor = index
        .tensors
        .iter()
        .find(|t| t.name == "blk.0.ffn_gate_exps.weight")
        .or_else(|| index.tensors.iter().find(|t| t.ggml_type == 39))
        .ok_or_else(|| "no MXFP4 tensor found".to_string())?;

    if tensor.ggml_type != 39 {
        return Err(format!(
            "tensor {} is {}, expected MXFP4",
            tensor.name, tensor.ggml_type_name
        ));
    }

    if tensor.dims.len() != 3 {
        return Err(format!(
            "mxfp4 tensor {} dims {:?} expected 3d",
            tensor.name, tensor.dims
        ));
    }

    let experts = tensor.dims[0] as usize;
    let rows = tensor.dims[1] as usize;
    let cols = tensor.dims[2] as usize;
    if experts == 0 || rows == 0 || cols == 0 {
        return Err("mxfp4 tensor has empty dims".to_string());
    }

    let expert_idx = 0usize;
    let k = rows.min(32).max(1);
    let n = cols;
    let values = k
        .checked_mul(n)
        .ok_or_else(|| "mxfp4 probe shape overflow".to_string())?;
    if values % MXFP4_BLOCK_VALUES != 0 {
        return Err("mxfp4 probe values not divisible by block size".to_string());
    }

    let expert_values = rows
        .checked_mul(cols)
        .ok_or_else(|| "mxfp4 expert shape overflow".to_string())?;
    if expert_values % MXFP4_BLOCK_VALUES != 0 {
        return Err("mxfp4 expert values not divisible by block size".to_string());
    }

    let expert_blocks = expert_values / MXFP4_BLOCK_VALUES;
    let expert_bytes = expert_blocks * MXFP4_BLOCK_BYTES;
    let blocks = values / MXFP4_BLOCK_VALUES;
    let bytes_needed = blocks * MXFP4_BLOCK_BYTES;
    if bytes_needed > expert_bytes {
        return Err("mxfp4 probe slice exceeds expert size".to_string());
    }

    emit_inference_stage(
        state,
        "mxfp4_probe",
        StageStatus::Started,
        None,
        None,
        Some(format!(
            "tensor={} expert={} of {} k={} n={}",
            tensor.name, expert_idx, experts, k, n
        )),
    );

    let offset = tensor
        .absolute_offset
        .saturating_add((expert_idx * expert_bytes) as u64);
    let mut quant = fetch_range_source(gguf_source, offset, bytes_needed as u64).await?;
    emit_inference_stage(
        state,
        "mxfp4_header",
        StageStatus::Completed,
        None,
        None,
        Some(hex_preview(&quant, 64)),
    );
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }

    let x = build_input(k);
    let weights = dequant_mxfp4(&quant, values)?;
    let y_cpu = matmul_cpu(&weights, &x, k, n);
    let mut gpu_tracker = GpuAllocTracker::default();
    let y_gpu = gpu_matmul_mxfp4(&quant, &x, k, n, gpu, &mut gpu_tracker).await?;
    let (max_abs, mean_abs) = diff_stats(&y_cpu, &y_gpu);

    emit_inference_stage(
        state,
        "mxfp4_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!("max_abs={max_abs:.4} mean_abs={mean_abs:.4}")),
    );

    Ok(())
}

struct ProbeQkv {
    q: Vec<f32>,
    k: Vec<f32>,
    v: Vec<f32>,
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
}

async fn build_probe_qkv(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    config: &GptOssConfig,
    gpu: &GpuContext,
) -> Result<ProbeQkv, String> {
    let token_embd = find_tensor(index, "token_embd.weight")?;
    let attn_norm = find_tensor(index, "blk.0.attn_norm.weight")?;
    let attn_q_w = find_tensor(index, "blk.0.attn_q.weight")?;
    let attn_q_b = find_tensor(index, "blk.0.attn_q.bias")?;
    let attn_k_w = find_tensor(index, "blk.0.attn_k.weight")?;
    let attn_k_b = find_tensor(index, "blk.0.attn_k.bias")?;
    let attn_v_w = find_tensor(index, "blk.0.attn_v.weight")?;
    let attn_v_b = find_tensor(index, "blk.0.attn_v.bias")?;

    let token_row = fetch_q8_0_row(gguf_source, token_embd, 0).await?;
    let attn_norm_w = fetch_f32_tensor_raw(gguf_source, attn_norm).await?;
    let normed = rms_norm(&token_row, &attn_norm_w, config.rms_epsilon)?;

    let mut caches = RuntimeCaches::new();
    let mut gpu_tracker = GpuAllocTracker::default();
    let q = matmul_q8_0_with_bias(
        state,
        gguf_source,
        attn_q_w,
        attn_q_b,
        &normed,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        false,
    )
    .await?;
    let k = matmul_q8_0_with_bias(
        state,
        gguf_source,
        attn_k_w,
        attn_k_b,
        &normed,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        false,
    )
    .await?;
    let v = matmul_q8_0_with_bias(
        state,
        gguf_source,
        attn_v_w,
        attn_v_b,
        &normed,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        false,
    )
    .await?;

    let heads = config.head_count.max(1) as usize;
    let kv_heads = config.head_count_kv.max(1) as usize;
    let q_head_dim = q.len() / heads.max(1);
    let k_head_dim = k.len() / kv_heads.max(1);
    if q_head_dim == 0 || k_head_dim == 0 || q_head_dim != k_head_dim {
        return Err(format!(
            "probe q/k head dims mismatch q_dim={q_head_dim} k_dim={k_head_dim}"
        ));
    }
    let head_dim = q_head_dim;
    if q.len() != heads * head_dim || k.len() != kv_heads * head_dim || v.len() != kv_heads * head_dim
    {
        return Err("probe qkv shape mismatch".to_string());
    }

    Ok(ProbeQkv {
        q,
        k,
        v,
        heads,
        kv_heads,
        head_dim,
    })
}

fn ensure_probe_tolerance(label: &str, max_abs: f32, mean_abs: f32) -> Result<(), String> {
    if max_abs > PROBE_TOLERANCE {
        return Err(format!(
            "{label} max_abs {max_abs:.4} exceeds tolerance {PROBE_TOLERANCE:.4} (mean={mean_abs:.4})"
        ));
    }
    Ok(())
}

async fn run_rmsnorm_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    config: &GptOssConfig,
    gpu: &GpuContext,
) -> Result<(), String> {
    emit_inference_stage(
        state,
        "rmsnorm_probe",
        StageStatus::Started,
        None,
        None,
        Some("blk.0.attn_norm".to_string()),
    );

    let token_embd = find_tensor(index, "token_embd.weight")?;
    let attn_norm = find_tensor(index, "blk.0.attn_norm.weight")?;
    let token_row = fetch_q8_0_row(gguf_source, token_embd, 0).await?;
    let attn_norm_w = fetch_f32_tensor_raw(gguf_source, attn_norm).await?;
    let cpu = rms_norm(&token_row, &attn_norm_w, config.rms_epsilon)?;
    let mut gpu_tracker = GpuAllocTracker::default();
    let gpu_out =
        rms_norm_gpu(&token_row, &attn_norm_w, config.rms_epsilon, gpu, &mut gpu_tracker)
            .await?;
    let (max_abs, mean_abs) = diff_stats(&cpu, &gpu_out);
    ensure_probe_tolerance("rmsnorm_probe", max_abs, mean_abs)?;

    emit_inference_stage(
        state,
        "rmsnorm_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!("max_abs={max_abs:.4} mean_abs={mean_abs:.4}")),
    );
    Ok(())
}

async fn run_rope_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    config: &GptOssConfig,
    gpu: &GpuContext,
) -> Result<(), String> {
    emit_inference_stage(
        state,
        "rope_probe",
        StageStatus::Started,
        None,
        None,
        Some("blk.0.attn_q/attn_k".to_string()),
    );

    let probe = build_probe_qkv(state, gguf_source, index, config, gpu).await?;
    let mut q_cpu = probe.q.clone();
    let mut k_cpu = probe.k.clone();
    apply_rope(
        &mut q_cpu,
        probe.heads,
        probe.head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;
    apply_rope(
        &mut k_cpu,
        probe.kv_heads,
        probe.head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;

    let mut gpu_tracker = GpuAllocTracker::default();
    let q_gpu = apply_rope_gpu(
        &probe.q,
        probe.heads,
        probe.head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        &mut gpu_tracker,
    )
    .await?;
    let k_gpu = apply_rope_gpu(
        &probe.k,
        probe.kv_heads,
        probe.head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        &mut gpu_tracker,
    )
    .await?;

    let (q_max, q_mean) = diff_stats(&q_cpu, &q_gpu);
    let (k_max, k_mean) = diff_stats(&k_cpu, &k_gpu);
    ensure_probe_tolerance("rope_probe q", q_max, q_mean)?;
    ensure_probe_tolerance("rope_probe k", k_max, k_mean)?;

    emit_inference_stage(
        state,
        "rope_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "q_max={q_max:.4} q_mean={q_mean:.4} k_max={k_max:.4} k_mean={k_mean:.4}"
        )),
    );
    Ok(())
}

async fn run_attention_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_source: &GgufSource,
    index: &GgufIndex,
    config: &GptOssConfig,
    gpu: &GpuContext,
) -> Result<(), String> {
    emit_inference_stage(
        state,
        "attn_probe",
        StageStatus::Started,
        None,
        None,
        Some("blk.0 attn".to_string()),
    );

    let probe = build_probe_qkv(state, gguf_source, index, config, gpu).await?;
    let attn_sinks = find_tensor(index, "blk.0.attn_sinks.weight")?;
    let sinks = fetch_f32_tensor_raw(gguf_source, attn_sinks).await?;
    if sinks.len() < probe.heads {
        return Err("attn_probe sinks length mismatch".to_string());
    }

    let mut q_pos1 = probe.q.clone();
    let mut k_pos0 = probe.k.clone();
    let mut k_pos1 = probe.k.clone();
    apply_rope(
        &mut q_pos1,
        probe.heads,
        probe.head_dim,
        1,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;
    apply_rope(
        &mut k_pos0,
        probe.kv_heads,
        probe.head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;
    apply_rope(
        &mut k_pos1,
        probe.kv_heads,
        probe.head_dim,
        1,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
    )?;

    let mut layer_cache = LayerKvCache::new();
    let max_len = 4usize;
    layer_cache.append(
        &k_pos0,
        &probe.v,
        probe.kv_heads,
        probe.head_dim,
        max_len,
        gpu,
        true,
    )?;
    layer_cache.append(
        &k_pos1,
        &probe.v,
        probe.kv_heads,
        probe.head_dim,
        max_len,
        gpu,
        true,
    )?;
    let window = layer_cache.token_count();
    let cpu_out = attention_with_cache(
        &q_pos1,
        &layer_cache,
        &sinks,
        probe.heads,
        probe.kv_heads,
        probe.head_dim,
        window,
    )?;
    let mut gpu_tracker = GpuAllocTracker::default();
    let gpu_out = attention_with_cache_gpu(
        &q_pos1,
        &layer_cache,
        &sinks,
        probe.heads,
        probe.kv_heads,
        probe.head_dim,
        window,
        gpu,
        &mut gpu_tracker,
    )
    .await?;
    let (max_abs, mean_abs) = diff_stats(&cpu_out, &gpu_out);
    ensure_probe_tolerance("attn_probe", max_abs, mean_abs)?;

    emit_inference_stage(
        state,
        "attn_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!("max_abs={max_abs:.4} mean_abs={mean_abs:.4}")),
    );
    Ok(())
}

async fn run_block0_attention_probe(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
    tokenizer: &GptOssTokenizer,
    config: &GptOssConfig,
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
    let attn_sinks = find_tensor(index, "blk.0.attn_sinks.weight")?;
    let post_attn_norm = find_tensor(index, "blk.0.post_attention_norm.weight")?;
    let output_norm_tensor = find_tensor(index, "output_norm.weight")?;
    let output_weight = find_tensor(index, "output.weight")?;
    emit_tensor_resident(
        state,
        output_weight.name.clone(),
        output_weight.nbytes as usize,
        "q8_0",
    );
    let mut caches = RuntimeCaches::new();
    let mut gpu_tracker = GpuAllocTracker::default();

    let token_row = match fetch_q8_0_row_gpu(gguf_url, token_embd, 0, gpu, &mut gpu_tracker).await {
        Ok(value) => value,
        Err(_) => fetch_q8_0_row(gguf_url, token_embd, 0).await?,
    };
    let attn_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, attn_norm, &mut caches.tensors).await?;
    let mut hidden =
        match rms_norm_gpu(&token_row, &attn_norm_w, config.rms_epsilon, gpu, &mut gpu_tracker)
            .await
        {
            Ok(value) => value,
            Err(_) => rms_norm(&token_row, &attn_norm_w, config.rms_epsilon)?,
        };

    emit_inference_stage(
        state,
        "blk0_qkv",
        StageStatus::Started,
        None,
        None,
        Some("qkv".to_string()),
    );

    let mut q = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_q_w,
        attn_q_b,
        &hidden,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        true,
    )
    .await?;
    let mut k = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_k_w,
        attn_k_b,
        &hidden,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        true,
    )
    .await?;
    let v = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_v_w,
        attn_v_b,
        &hidden,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        true,
    )
    .await?;

    emit_inference_stage(
        state,
        "blk0_qkv",
        StageStatus::Completed,
        None,
        None,
        Some(format!("q={} k={} v={}", q.len(), k.len(), v.len())),
    );

    emit_inference_stage(
        state,
        "blk0_rope",
        StageStatus::Started,
        None,
        None,
        Some("pos=0".to_string()),
    );

    let q_head_dim = (q.len() / config.head_count.max(1) as usize).max(1);
    let k_head_dim = (k.len() / config.head_count_kv.max(1) as usize).max(1);
    let q_mode = match apply_rope_gpu(
        &q,
        config.head_count as usize,
        q_head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        &mut gpu_tracker,
    )
    .await
    {
        Ok(value) => {
            q = value;
            "gpu"
        }
        Err(_) => {
            apply_rope(
                &mut q,
                config.head_count as usize,
                q_head_dim,
                0,
                config.rope_theta,
                config.rope_dimension_count,
                config.rope_scaling_factor,
                config.rope_scaling_original_context,
            )?;
            "cpu"
        }
    };
    let k_mode = match apply_rope_gpu(
        &k,
        config.head_count_kv as usize,
        k_head_dim,
        0,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        &mut gpu_tracker,
    )
    .await
    {
        Ok(value) => {
            k = value;
            "gpu"
        }
        Err(_) => {
            apply_rope(
                &mut k,
                config.head_count_kv as usize,
                k_head_dim,
                0,
                config.rope_theta,
                config.rope_dimension_count,
                config.rope_scaling_factor,
                config.rope_scaling_original_context,
            )?;
            "cpu"
        }
    };

    emit_inference_stage(
        state,
        "blk0_rope",
        StageStatus::Completed,
        None,
        None,
        Some(format!("q={q_mode} k={k_mode}")),
    );

    let sinks =
        fetch_f32_tensor_cached(state, gguf_url, attn_sinks, &mut caches.tensors).await?;
    let attn_out = attention_single_token(
        &q,
        &k,
        &v,
        &sinks,
        config.head_count as usize,
        config.head_count_kv as usize,
        q_head_dim,
    )?;
    let attn_proj = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_out_w,
        attn_out_b,
        &attn_out,
        gpu,
        &mut gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        true,
    )
    .await?;
    hidden = match vector_add_gpu(&hidden, &attn_proj, gpu, &mut gpu_tracker).await {
        Ok(value) => value,
        Err(_) => {
            for (out, base) in hidden.iter_mut().zip(attn_proj.iter()) {
                *out += *base;
            }
            hidden
        }
    };

    let post_attn_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, post_attn_norm, &mut caches.tensors).await?;
    hidden = match rms_norm_gpu(&hidden, &post_attn_norm_w, config.rms_epsilon, gpu, &mut gpu_tracker)
        .await
    {
        Ok(value) => value,
        Err(_) => rms_norm(&hidden, &post_attn_norm_w, config.rms_epsilon)?,
    };

    let gate_inp_w = find_tensor(index, "blk.0.ffn_gate_inp.weight")?;
    let gate_inp_b = find_tensor(index, "blk.0.ffn_gate_inp.bias")?;
    let gate_w =
        fetch_f32_tensor_cached(state, gguf_url, gate_inp_w, &mut caches.tensors).await?;
    let gate_b =
        fetch_f32_tensor_cached(state, gguf_url, gate_inp_b, &mut caches.tensors).await?;
    let gate_scores = match linear_f32_with_bias_gpu(
        &gate_w,
        &gate_b,
        &hidden,
        gate_inp_w,
        gpu,
        &mut gpu_tracker,
        true,
    )
    .await
    {
        Ok(value) => value,
        Err(_) => linear_f32_with_bias(&gate_w, &gate_b, &hidden, gate_inp_w)?,
    };
    let (expert_indices, expert_weights) =
        top_k_softmax(&gate_scores, config.experts_per_token as usize)?;
    emit_inference_stage(
        state,
        "moe_router",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "top={:?} w={:?}",
            expert_indices, expert_weights
        )),
    );

    let gate_exps_w = find_tensor(index, "blk.0.ffn_gate_exps.weight")?;
    let gate_exps_b = find_tensor(index, "blk.0.ffn_gate_exps.bias")?;
    let up_exps_w = find_tensor(index, "blk.0.ffn_up_exps.weight")?;
    let up_exps_b = find_tensor(index, "blk.0.ffn_up_exps.bias")?;
    let down_exps_w = find_tensor(index, "blk.0.ffn_down_exps.weight")?;
    let down_exps_b = find_tensor(index, "blk.0.ffn_down_exps.bias")?;

    emit_inference_stage(
        state,
        "moe_mlp",
        StageStatus::Started,
        None,
        None,
        Some(format!("experts={}", expert_indices.len())),
    );

    let mut mlp_accum = vec![0.0f32; hidden.len()];
    for (expert_idx, weight) in expert_indices.iter().zip(expert_weights.iter()) {
        let gate_quant = fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            gate_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await?;
        let mut gate_out =
            matmul_mxfp4_expert(&gate_quant, &hidden, gate_exps_w, gpu, &mut gpu_tracker).await?;
        let gate_bias = fetch_f32_row(gguf_url, gate_exps_b, *expert_idx).await?;
        apply_bias_gpu(&mut gate_out, &gate_bias, gpu, &mut gpu_tracker, true).await?;

        let up_quant = fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            up_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await?;
        let mut up_out =
            matmul_mxfp4_expert(&up_quant, &hidden, up_exps_w, gpu, &mut gpu_tracker).await?;
        let up_bias = fetch_f32_row(gguf_url, up_exps_b, *expert_idx).await?;
        apply_bias_gpu(&mut up_out, &up_bias, gpu, &mut gpu_tracker, true).await?;

        let swiglu_out = match swiglu_gpu(&gate_out, &up_out, gpu, &mut gpu_tracker).await {
            Ok(value) => value,
            Err(_) => swiglu(&gate_out, &up_out)?,
        };
        let down_quant = fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            down_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await?;
        let mut down_out = matmul_mxfp4_expert(
            &down_quant,
            &swiglu_out,
            down_exps_w,
            gpu,
            &mut gpu_tracker,
        )
        .await?;
        let down_bias = fetch_f32_row(gguf_url, down_exps_b, *expert_idx).await?;
        apply_bias_gpu(&mut down_out, &down_bias, gpu, &mut gpu_tracker, true).await?;

        if let Ok(value) =
            scale_add_gpu(&mlp_accum, &down_out, *weight, gpu, &mut gpu_tracker).await
        {
            mlp_accum = value;
        } else {
            for (acc, val) in mlp_accum.iter_mut().zip(down_out.iter()) {
                *acc += *val * *weight;
            }
        }
    }

    hidden = match vector_add_gpu(&hidden, &mlp_accum, gpu, &mut gpu_tracker).await {
        Ok(value) => value,
        Err(_) => {
            for (out, add) in hidden.iter_mut().zip(mlp_accum.iter()) {
                *out += *add;
            }
            hidden
        }
    };

    emit_inference_stage(
        state,
        "moe_mlp",
        StageStatus::Completed,
        None,
        None,
        Some("ok".to_string()),
    );

    let output_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, output_norm_tensor, &mut caches.tensors).await?;
    let final_hidden =
        match rms_norm_gpu(&hidden, &output_norm_w, config.rms_epsilon, gpu, &mut gpu_tracker)
            .await
        {
            Ok(value) => value,
            Err(_) => rms_norm(&hidden, &output_norm_w, config.rms_epsilon)?,
        };

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

    emit_inference_stage(
        state,
        "logits_probe",
        StageStatus::Started,
        None,
        None,
        Some("output.weight".to_string()),
    );

    let start_ms = now_ms();
    let logits = gpu_matmul_q8_0_chunked(
        gguf_url,
        output_weight,
        &final_hidden,
        gpu,
        &mut gpu_tracker,
    )
    .await?;
    let (top_k, entropy, token_id, token_text) = top_k_from_logits(&logits, tokenizer, 5)?;
    let elapsed_ms = now_ms().saturating_sub(start_ms).max(1);
    let tokens_per_sec = 1000.0 / elapsed_ms as f32;

    emit_inference_event(
        state,
        GptOssInferenceTelemetry::TokenGenerated {
            token_id,
            token_text,
            top_k,
            entropy,
            tokens_per_sec,
        },
    );

    emit_inference_stage(
        state,
        "logits_probe",
        StageStatus::Completed,
        None,
        None,
        Some(format!("elapsed={}ms", elapsed_ms)),
    );

    Ok(())
}

async fn run_generation(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
    tokenizer: &GptOssTokenizer,
    config: &GptOssConfig,
    prompt_tokens: &[u32],
    active_layers: usize,
    moe_fallback: bool,
    max_kv_tokens: usize,
    max_new_tokens: usize,
    force_dense: bool,
    sampling: SamplingConfig,
    stop_tokens: Vec<u32>,
) -> Result<(), String> {
    if prompt_tokens.is_empty() {
        return Err("prompt token list is empty".to_string());
    }
    let allow_cpu_fallback = false;
    let generation_start_ms = now_ms();

    emit_inference_stage(
        state,
        "generation",
        StageStatus::Started,
        None,
        None,
        Some(format!("prompt_tokens={} new={max_new_tokens}", prompt_tokens.len())),
    );

    let total_prefill = prompt_tokens.len();
    let mut cache = KvCache::new(config.block_count as usize, max_kv_tokens);
    let mut caches = RuntimeCaches::new();
    let mut gpu_tracker = GpuAllocTracker::default();
    let mut last_logits: Option<Vec<f32>> = None;
    let mut last_step_ms = 1u64;
    let attention_mode = if force_dense {
        "dense (override)".to_string()
    } else if config.sliding_window > 0 {
        format!("window={} even", config.sliding_window)
    } else {
        "dense".to_string()
    };
    let moe_mode = if moe_fallback {
        "fallback expert=0".to_string()
    } else if config.expert_count > 0 {
        format!("experts={} topk={}", config.expert_count, config.experts_per_token)
    } else {
        "disabled".to_string()
    };
    let sample_mode = if sampling.enabled {
        let top_k_label = if sampling.top_k == 0 {
            "auto".to_string()
        } else {
            sampling.top_k.to_string()
        };
        format!(
            "temp={:.2},top_k={},top_p={:.2}",
            sampling.temperature, top_k_label, sampling.top_p
        )
    } else {
        "greedy".to_string()
    };
    emit_inference_stage(
        state,
        "runtime_mode",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "layers={active_layers} attn={attention_mode} moe={moe_mode} sample={sample_mode} kv_max={max_kv_tokens} new={max_new_tokens} cpu_fallback={}",
            if allow_cpu_fallback { "on" } else { "off" }
        )),
    );

    emit_inference_stage(
        state,
        "prefill",
        StageStatus::Started,
        Some(0),
        Some(total_prefill),
        Some(format!("tokens={total_prefill}")),
    );
    let prefill_start_ms = now_ms();

    for (idx, &token_id) in prompt_tokens.iter().enumerate() {
        let position = cache.seq_len;
        if position >= cache.max_len {
            return Err("kv cache max length exceeded".to_string());
        }
        let start_ms = now_ms();
        let logits = run_forward_token(
            state,
            gguf_url,
            index,
            gpu,
            config,
            token_id,
            position,
            &mut cache,
            &mut caches,
            &mut gpu_tracker,
            active_layers,
            moe_fallback,
            force_dense,
            true,
            allow_cpu_fallback,
        )
        .await?;
        last_step_ms = now_ms().saturating_sub(start_ms).max(1);
        last_logits = Some(logits);
        emit_inference_stage(
            state,
            "prefill",
            StageStatus::Progress,
            Some(idx + 1),
            Some(total_prefill),
            Some(format!("token_id={token_id}")),
        );
        yield_to_browser().await;
    }

    let prefill_ms = now_ms().saturating_sub(prefill_start_ms).max(1);
    let prefill_tok_s = if total_prefill > 0 {
        total_prefill as f32 / (prefill_ms as f32 / 1000.0)
    } else {
        0.0
    };
    emit_inference_stage(
        state,
        "prefill",
        StageStatus::Completed,
        Some(total_prefill),
        Some(total_prefill),
        Some(format!("ok ms={prefill_ms} tok/s={prefill_tok_s:.1}")),
    );

    let mut logits = last_logits.ok_or_else(|| "prefill produced no logits".to_string())?;
    let mut generated = 0usize;
    let mut stop_reason = "max_new".to_string();
    let mut decoded = String::new();

    emit_inference_stage(
        state,
        "decode",
        StageStatus::Started,
        Some(0),
        Some(max_new_tokens),
        None,
    );
    let decode_start_ms = now_ms();

    while generated < max_new_tokens {
        let (top_k, entropy, next_id, next_text) =
            sample_from_logits(&logits, tokenizer, sampling, 5)?;
        let stop_token = stop_tokens.contains(&next_id);
        let token_text = if stop_token {
            String::new()
        } else {
            next_text
        };
        if !stop_token {
            decoded.push_str(&token_text);
        }
        let tokens_per_sec = 1000.0 / last_step_ms as f32;

        emit_inference_event(
            state,
            GptOssInferenceTelemetry::TokenGenerated {
                token_id: next_id,
                token_text,
                top_k,
                entropy,
                tokens_per_sec,
            },
        );

        generated += 1;
        emit_inference_stage(
            state,
            "decode",
            StageStatus::Progress,
            Some(generated),
            Some(max_new_tokens),
            Some(format!("token_id={next_id}")),
        );
        yield_to_browser().await;

        if stop_token {
            stop_reason = "stop_token".to_string();
            break;
        }

        let position = cache.seq_len;
        let start_ms = now_ms();
        logits = run_forward_token(
            state,
            gguf_url,
            index,
            gpu,
            config,
            next_id,
            position,
            &mut cache,
            &mut caches,
            &mut gpu_tracker,
            active_layers,
            moe_fallback,
            force_dense,
            false,
            allow_cpu_fallback,
        )
        .await?;
        last_step_ms = now_ms().saturating_sub(start_ms).max(1);
    }

    let decode_ms = now_ms().saturating_sub(decode_start_ms).max(1);
    let decode_tok_s = if generated > 0 {
        generated as f32 / (decode_ms as f32 / 1000.0)
    } else {
        0.0
    };
    emit_inference_stage(
        state,
        "decode",
        StageStatus::Completed,
        Some(generated),
        Some(max_new_tokens),
        Some(format!(
            "{stop_reason} ms={decode_ms} tok/s={decode_tok_s:.1}",
        )),
    );

    let avg_ms = if generated > 0 {
        decode_ms / generated as u64
    } else {
        0
    };
    let budget_label = if generated == 0 || avg_ms <= 30_000 {
        "ok"
    } else {
        "slow"
    };
    emit_inference_stage(
        state,
        "decode_budget",
        StageStatus::Completed,
        None,
        None,
        Some(format!("avg_ms={avg_ms} label={budget_label}")),
    );

    let coherence = coherence_score(&decoded);
    let coherence_label = if decoded.is_empty() {
        "empty"
    } else if coherence >= 0.6 {
        "ok"
    } else {
        "low"
    };
    emit_inference_stage(
        state,
        "coherence_check",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "score={coherence:.2} label={coherence_label} chars={}",
            decoded.len()
        )),
    );

    let generation_ms = now_ms().saturating_sub(generation_start_ms).max(1);
    emit_inference_stage(
        state,
        "generation",
        StageStatus::Completed,
        Some(generated),
        Some(max_new_tokens),
        Some(format!("{stop_reason} ms={generation_ms}")),
    );

    Ok(())
}

async fn run_forward_token(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
    config: &GptOssConfig,
    token_id: u32,
    position: usize,
    cache: &mut KvCache,
    caches: &mut RuntimeCaches,
    gpu_tracker: &mut GpuAllocTracker,
    active_layers: usize,
    moe_fallback: bool,
    force_dense: bool,
    is_prefill: bool,
    allow_cpu_fallback: bool,
) -> Result<Vec<f32>, String> {
    gpu_tracker.reset();
    let token_embd = find_tensor(index, "token_embd.weight")?;
    emit_inference_stage(
        state,
        "token_embd",
        StageStatus::Started,
        None,
        None,
        Some(format!("token_id={token_id}")),
    );
    let emb_start = now_ms();
    let (mut hidden, emb_hit, emb_mode) =
        match fetch_q8_0_row_cached_gpu(
            gguf_url,
            token_embd,
            token_id as usize,
            &mut caches.token_embd,
            gpu,
            gpu_tracker,
        )
        .await
        {
            Ok((data, hit)) => {
                let mode = if hit { "cache" } else { "gpu" };
                (data, hit, mode)
            }
            Err(err) => {
                if !allow_cpu_fallback {
                    return Err(format!("token_embd gpu failed: {err}"));
                }
                let (data, hit) = fetch_q8_0_row_cached(
                    gguf_url,
                    token_embd,
                    token_id as usize,
                    &mut caches.token_embd,
                )
                .await?;
                let mode = if hit { "cache" } else { "cpu" };
                (data, hit, mode)
            }
        };
    let emb_ms = now_ms().saturating_sub(emb_start).max(1);
    emit_inference_stage(
        state,
        "token_embd",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "token_id={token_id} {emb_mode} ms={emb_ms} cache={}",
            if emb_hit { "hit" } else { "miss" }
        )),
    );

    let layer_limit = active_layers.min(config.block_count as usize);
    let mut applied_layers = 0usize;
    for layer in 0..layer_limit as u32 {
        let fallback = hidden.clone();
        match run_transformer_layer(
            state,
            gguf_url,
            index,
            gpu,
            config,
            layer,
            position,
            hidden,
            cache,
            caches,
            gpu_tracker,
            layer_limit,
            moe_fallback,
            force_dense,
            is_prefill,
            allow_cpu_fallback,
        )
        .await
        {
            Ok(next_hidden) => {
                hidden = next_hidden;
                applied_layers = applied_layers.saturating_add(1);
            }
            Err(err) => {
                hidden = fallback;
                emit_inference_stage(
                    state,
                    "layer_fallback",
                    StageStatus::Failed,
                    Some(applied_layers),
                    Some(layer_limit),
                    Some(format!("layer={layer} err={err}")),
                );
                break;
            }
        }
        yield_to_browser().await;
    }
    if applied_layers < layer_limit {
        emit_inference_stage(
            state,
            "layer_fallback",
            StageStatus::Completed,
            Some(applied_layers),
            Some(layer_limit),
            Some("skipped remaining layers".to_string()),
        );
    }

    let output_norm_tensor = find_tensor(index, "output_norm.weight")?;
    let output_norm_start = now_ms();
    let output_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, output_norm_tensor, &mut caches.tensors).await?;
    let (final_hidden, output_norm_mode) = match rms_norm_gpu(
        &hidden,
        &output_norm_w,
        config.rms_epsilon,
        gpu,
        gpu_tracker,
    )
    .await
    {
        Ok(value) => (value, "gpu"),
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("output_norm gpu failed: {err}"));
            }
            (rms_norm(&hidden, &output_norm_w, config.rms_epsilon)?, "cpu")
        }
    };
    let output_norm_ms = now_ms().saturating_sub(output_norm_start).max(1);
    emit_inference_stage(
        state,
        "output_norm",
        StageStatus::Completed,
        None,
        None,
        Some(format!("{output_norm_mode} ms={output_norm_ms}")),
    );

    let output_weight = find_tensor(index, "output.weight")?;
    let logits_start = now_ms();
    emit_inference_stage(
        state,
        "weights_fetch",
        StageStatus::Started,
        None,
        None,
        Some(format!("{} bytes={}", output_weight.name, format_bytes(output_weight.nbytes))),
    );
    let logits = gpu_matmul_q8_0_chunked(
        gguf_url,
        output_weight,
        &final_hidden,
        gpu,
        gpu_tracker,
    )
    .await?;
    let logits_ms = now_ms().saturating_sub(logits_start).max(1);
    emit_inference_stage(
        state,
        "weights_fetch",
        StageStatus::Completed,
        None,
        None,
        Some(format!("{} ok ms={logits_ms}", output_weight.name)),
    );

    cache.seq_len = position.saturating_add(1);
    let token_stats = caches.token_embd.stats();
    let tensor_stats = caches.tensors.stats();
    let quant_stats = caches.quant.stats();
    let expert_stats = caches.experts.stats();
    emit_inference_stage(
        state,
        "token_cache",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "hits={} misses={} evict={} skip={} bytes={} entries={}",
            token_stats.hits,
            token_stats.misses,
            token_stats.evictions,
            token_stats.skipped,
            format_bytes(token_stats.bytes as u64),
            token_stats.entries
        )),
    );
    emit_inference_stage(
        state,
        "tensor_cache",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "hits={} misses={} evict={} skip={} bytes={} entries={}",
            tensor_stats.hits,
            tensor_stats.misses,
            tensor_stats.evictions,
            tensor_stats.skipped,
            format_bytes(tensor_stats.bytes as u64),
            tensor_stats.entries
        )),
    );
    emit_inference_stage(
        state,
        "q8_0_cache",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "hits={} misses={} evict={} skip={} bytes={} entries={}",
            quant_stats.hits,
            quant_stats.misses,
            quant_stats.evictions,
            quant_stats.skipped,
            format_bytes(quant_stats.bytes as u64),
            quant_stats.entries
        )),
    );
    emit_inference_stage(
        state,
        "expert_cache",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "hits={} misses={} evict={} skip={} bytes={} entries={}",
            expert_stats.hits,
            expert_stats.misses,
            expert_stats.evictions,
            expert_stats.skipped,
            format_bytes(expert_stats.bytes as u64),
            expert_stats.entries
        )),
    );
    let gpu_bytes = gpu_tracker.bytes.saturating_add(cache.total_bytes());
    emit_inference_stage(
        state,
        "gpu_alloc",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "bytes={} buffers={} kv={}",
            format_bytes(gpu_bytes as u64),
            gpu_tracker.buffers,
            format_bytes(cache.total_bytes() as u64)
        )),
    );
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::MemoryUsage {
            gpu_allocated: gpu_bytes,
            cache_total: cache.total_bytes()
                + token_stats.bytes
                + tensor_stats.bytes
                + quant_stats.bytes
                + expert_stats.bytes,
            activations: final_hidden.len() * std::mem::size_of::<f32>(),
        },
    );

    Ok(logits)
}

async fn run_single_token_full(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
    tokenizer: &GptOssTokenizer,
    config: &GptOssConfig,
    token_id: u32,
) -> Result<(), String> {
    let mut cache = KvCache::new(config.block_count as usize, 1);
    let mut caches = RuntimeCaches::new();
    let mut gpu_tracker = GpuAllocTracker::default();
    let logits = run_forward_token(
        state,
        gguf_url,
        index,
        gpu,
        config,
        token_id,
        0,
        &mut cache,
        &mut caches,
        &mut gpu_tracker,
        config.block_count as usize,
        false,
        false,
        true,
        true,
    )
    .await?;
    let (top_k, entropy, next_id, next_text) = top_k_from_logits(&logits, tokenizer, 5)?;
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::TokenGenerated {
            token_id: next_id,
            token_text: next_text,
            top_k,
            entropy,
            tokens_per_sec: 0.0,
        },
    );
    Ok(())
}

async fn run_transformer_layer(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    index: &GgufIndex,
    gpu: &GpuContext,
    config: &GptOssConfig,
    layer: u32,
    position: usize,
    mut hidden: Vec<f32>,
    cache: &mut KvCache,
    caches: &mut RuntimeCaches,
    gpu_tracker: &mut GpuAllocTracker,
    total_layers: usize,
    moe_fallback: bool,
    force_dense: bool,
    is_prefill: bool,
    allow_cpu_fallback: bool,
) -> Result<Vec<f32>, String> {
    let attn_norm = find_tensor(index, &format!("blk.{layer}.attn_norm.weight"))?;
    let attn_q_w = find_tensor(index, &format!("blk.{layer}.attn_q.weight"))?;
    let attn_q_b = find_tensor(index, &format!("blk.{layer}.attn_q.bias"))?;
    let attn_k_w = find_tensor(index, &format!("blk.{layer}.attn_k.weight"))?;
    let attn_k_b = find_tensor(index, &format!("blk.{layer}.attn_k.bias"))?;
    let attn_v_w = find_tensor(index, &format!("blk.{layer}.attn_v.weight"))?;
    let attn_v_b = find_tensor(index, &format!("blk.{layer}.attn_v.bias"))?;
    let attn_out_w = find_tensor(index, &format!("blk.{layer}.attn_output.weight"))?;
    let attn_out_b = find_tensor(index, &format!("blk.{layer}.attn_output.bias"))?;
    let attn_sinks = find_tensor(index, &format!("blk.{layer}.attn_sinks.weight"))?;
    let post_attn_norm = find_tensor(index, &format!("blk.{layer}.post_attention_norm.weight"))?;

    let layer_attn_start = now_ms();
    emit_inference_stage(
        state,
        "layer_attn",
        StageStatus::Started,
        Some((layer + 1) as usize),
        Some(total_layers),
        Some(format!("layer={layer}")),
    );

    let attn_norm_start = now_ms();
    let attn_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, attn_norm, &mut caches.tensors).await?;
    let (mut normed, attn_norm_mode) =
        match rms_norm_gpu(&hidden, &attn_norm_w, config.rms_epsilon, gpu, gpu_tracker).await {
            Ok(value) => (value, "gpu"),
            Err(err) => {
                if !allow_cpu_fallback {
                    return Err(format!("attn_norm gpu failed: {err}"));
                }
                (rms_norm(&hidden, &attn_norm_w, config.rms_epsilon)?, "cpu")
            }
        };
    let attn_norm_ms = now_ms().saturating_sub(attn_norm_start).max(1);
    emit_inference_stage(
        state,
        "attn_norm",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} {attn_norm_mode} ms={attn_norm_ms}")),
    );

    let q_start = now_ms();
    let mut q = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_q_w,
        attn_q_b,
        &normed,
        gpu,
        gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        allow_cpu_fallback,
    )
    .await?;
    let q_ms = now_ms().saturating_sub(q_start).max(1);
    emit_inference_stage(
        state,
        "attn_q",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} gpu ms={q_ms}")),
    );
    let k_start = now_ms();
    let mut k = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_k_w,
        attn_k_b,
        &normed,
        gpu,
        gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        allow_cpu_fallback,
    )
    .await?;
    let k_ms = now_ms().saturating_sub(k_start).max(1);
    emit_inference_stage(
        state,
        "attn_k",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} gpu ms={k_ms}")),
    );
    let v_start = now_ms();
    let v = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_v_w,
        attn_v_b,
        &normed,
        gpu,
        gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        allow_cpu_fallback,
    )
    .await?;
    let v_ms = now_ms().saturating_sub(v_start).max(1);
    emit_inference_stage(
        state,
        "attn_v",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} gpu ms={v_ms}")),
    );

    let heads = config.head_count as usize;
    let kv_heads = config.head_count_kv as usize;
    let q_head_dim = q.len() / heads.max(1);
    let k_head_dim = k.len() / kv_heads.max(1);
    if q_head_dim == 0 || k_head_dim == 0 || q_head_dim != k_head_dim {
        return Err(format!(
            "attention head dim mismatch q_dim={q_head_dim} k_dim={k_head_dim}"
        ));
    }
    let head_dim = q_head_dim;
    let rope_start = now_ms();
    let q_mode = match apply_rope_gpu(
        &q,
        heads,
        q_head_dim,
        position,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        gpu_tracker,
    )
    .await
    {
        Ok(value) => {
            q = value;
            "gpu"
        }
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("rope q gpu failed: {err}"));
            }
            apply_rope(
                &mut q,
                heads,
                q_head_dim,
                position,
                config.rope_theta,
                config.rope_dimension_count,
                config.rope_scaling_factor,
                config.rope_scaling_original_context,
            )?;
            "cpu"
        }
    };
    let k_mode = match apply_rope_gpu(
        &k,
        kv_heads,
        k_head_dim,
        position,
        config.rope_theta,
        config.rope_dimension_count,
        config.rope_scaling_factor,
        config.rope_scaling_original_context,
        gpu,
        gpu_tracker,
    )
    .await
    {
        Ok(value) => {
            k = value;
            "gpu"
        }
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("rope k gpu failed: {err}"));
            }
            apply_rope(
                &mut k,
                kv_heads,
                k_head_dim,
                position,
                config.rope_theta,
                config.rope_dimension_count,
                config.rope_scaling_factor,
                config.rope_scaling_original_context,
            )?;
            "cpu"
        }
    };
    let rope_ms = now_ms().saturating_sub(rope_start).max(1);
    emit_inference_stage(
        state,
        "rope",
        StageStatus::Completed,
        None,
        None,
        Some(format!(
            "layer={layer} q={q_mode} k={k_mode} ms={rope_ms}"
        )),
    );

    let sinks =
        fetch_f32_tensor_cached(state, gguf_url, attn_sinks, &mut caches.tensors).await?;
    let max_len = cache.max_len;
    let layer_cache = cache.layer_mut(layer as usize)?;
    layer_cache.append(
        &k,
        &v,
        kv_heads,
        head_dim,
        max_len,
        gpu,
        allow_cpu_fallback,
    )?;
    let seq_len = layer_cache.token_count();
    let window = if force_dense || config.sliding_window == 0 {
        seq_len
    } else if layer % 2 == 0 {
        config.sliding_window as usize
    } else {
        seq_len
    };
    let window = window.max(1).min(seq_len);
    let offset = if layer_cache.capacity > 0 {
        (layer_cache.start + seq_len.saturating_sub(window)) % layer_cache.capacity
    } else {
        0
    };
    emit_inference_event(
        state,
        GptOssInferenceTelemetry::CacheStatus {
            layer: layer as usize,
            seq_len,
            max_len,
            offset,
            memory_bytes: layer_cache.memory_bytes(),
        },
    );

    let attn_start = now_ms();
    let mut attn_fallback = false;
    let mut attn_mode = "gpu";
    let phase = if is_prefill { "prefill" } else { "decode" };
    let attn_out = match attention_with_cache_gpu(
        &q,
        layer_cache,
        &sinks,
        heads,
        kv_heads,
        head_dim,
        window,
        gpu,
        gpu_tracker,
    )
    .await
    {
        Ok(out) => out,
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("gpu attention failed: {err}"));
            }
            attn_mode = "cpu";
            match attention_with_cache(
                &q,
                layer_cache,
                &sinks,
                heads,
                kv_heads,
                head_dim,
                window,
            ) {
                Ok(out) => out,
                Err(err) => {
                    attn_fallback = true;
                    emit_inference_stage(
                        state,
                        "attn_fallback",
                        StageStatus::Completed,
                        None,
                        None,
                        Some(format!("layer={layer} phase={phase} fallback=single_token err={err}")),
                    );
                    attention_single_token(&q, &k, &v, &sinks, heads, kv_heads, head_dim)?
                }
            }
        }
    };
    let attn_ms = now_ms().saturating_sub(attn_start).max(1);
    let attn_detail = if attn_fallback {
        format!("layer={layer} {attn_mode} phase={phase} window={window} ms={attn_ms} fallback")
    } else {
        format!("layer={layer} {attn_mode} phase={phase} window={window} ms={attn_ms}")
    };
    emit_inference_stage(
        state,
        "attn_score",
        StageStatus::Completed,
        None,
        None,
        Some(attn_detail),
    );
    let (selected_layer, selected_head) = state
        .try_borrow()
        .ok()
        .map(|guard| {
            (
                guard.gptoss.attention_selected_layer,
                guard.gptoss.attention_selected_head,
            )
        })
        .unwrap_or((0, 0));
    if selected_layer == layer as usize {
        if let Ok(weights) = attention_head_weights_gpu(
            &q,
            layer_cache,
            &sinks,
            selected_head,
            kv_heads,
            head_dim,
            window,
            gpu,
            gpu_tracker,
        )
        .await
        {
            emit_inference_event(
                state,
                GptOssInferenceTelemetry::AttentionWeights {
                    layer: layer as usize,
                    head: selected_head,
                    weights: vec![weights],
                },
            );
        } else if allow_cpu_fallback {
            if let Ok(weights) = attention_head_weights(
                &q,
                layer_cache,
                &sinks,
                selected_head,
                heads,
                kv_heads,
                head_dim,
                window,
            ) {
                emit_inference_event(
                    state,
                    GptOssInferenceTelemetry::AttentionWeights {
                        layer: layer as usize,
                        head: selected_head,
                        weights: vec![weights],
                    },
                );
            }
        }
    }

    let proj_start = now_ms();
    let attn_proj = matmul_q8_0_with_bias(
        state,
        gguf_url,
        attn_out_w,
        attn_out_b,
        &attn_out,
        gpu,
        gpu_tracker,
        &mut caches.tensors,
        &mut caches.quant,
        allow_cpu_fallback,
    )
    .await?;
    let proj_ms = now_ms().saturating_sub(proj_start).max(1);
    emit_inference_stage(
        state,
        "attn_out",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} gpu ms={proj_ms}")),
    );
    hidden = match vector_add_gpu(&hidden, &attn_proj, gpu, gpu_tracker).await {
        Ok(value) => value,
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("attn_residual gpu failed: {err}"));
            }
            for (out, base) in hidden.iter_mut().zip(attn_proj.iter()) {
                *out += *base;
            }
            hidden
        }
    };

    let layer_attn_ms = now_ms().saturating_sub(layer_attn_start).max(1);
    emit_inference_stage(
        state,
        "layer_attn",
        StageStatus::Completed,
        Some((layer + 1) as usize),
        Some(total_layers),
        Some(format!("ok ms={layer_attn_ms}")),
    );

    let post_norm_start = now_ms();
    let post_attn_norm_w =
        fetch_f32_tensor_cached(state, gguf_url, post_attn_norm, &mut caches.tensors).await?;
    let (next_normed, post_norm_mode) = match rms_norm_gpu(
        &hidden,
        &post_attn_norm_w,
        config.rms_epsilon,
        gpu,
        gpu_tracker,
    )
    .await
    {
        Ok(value) => (value, "gpu"),
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("post_attn_norm gpu failed: {err}"));
            }
            (rms_norm(&hidden, &post_attn_norm_w, config.rms_epsilon)?, "cpu")
        }
    };
    normed = next_normed;
    let post_norm_ms = now_ms().saturating_sub(post_norm_start).max(1);
    emit_inference_stage(
        state,
        "post_attn_norm",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} {post_norm_mode} ms={post_norm_ms}")),
    );

    let layer_mlp_start = now_ms();
    emit_inference_stage(
        state,
        "layer_mlp",
        StageStatus::Started,
        Some((layer + 1) as usize),
        Some(total_layers),
        Some(format!("layer={layer}")),
    );

    let gate_inp_w = find_tensor(index, &format!("blk.{layer}.ffn_gate_inp.weight"))?;
    let gate_inp_b = find_tensor(index, &format!("blk.{layer}.ffn_gate_inp.bias"))?;
    let gate_start = now_ms();
    let gate_w =
        fetch_f32_tensor_cached(state, gguf_url, gate_inp_w, &mut caches.tensors).await?;
    let gate_b =
        fetch_f32_tensor_cached(state, gguf_url, gate_inp_b, &mut caches.tensors).await?;
    let (gate_scores, gate_mode) = match linear_f32_with_bias_gpu(
        &gate_w,
        &gate_b,
        &normed,
        gate_inp_w,
        gpu,
        gpu_tracker,
        allow_cpu_fallback,
    )
    .await
    {
        Ok(value) => (value, "gpu"),
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("moe_gate gpu failed: {err}"));
            }
            (
                linear_f32_with_bias(&gate_w, &gate_b, &normed, gate_inp_w)?,
                "cpu",
            )
        }
    };
    let gate_ms = now_ms().saturating_sub(gate_start).max(1);
    emit_inference_stage(
        state,
        "moe_gate",
        StageStatus::Completed,
        None,
        None,
        Some(format!("layer={layer} {gate_mode} ms={gate_ms}")),
    );
    let moe_fallback_active = moe_fallback || caches.moe_disabled;
    let (expert_indices, expert_weights) = if moe_fallback_active {
        (vec![0usize], vec![1.0f32])
    } else {
        top_k_softmax(&gate_scores, config.experts_per_token as usize)?
    };
    emit_inference_stage(
        state,
        "moe_router",
        StageStatus::Completed,
        Some((layer + 1) as usize),
        Some(total_layers),
        Some(format!(
            "layer={layer} experts={:?}",
            expert_indices
        )),
    );

    let gate_exps_w = find_tensor(index, &format!("blk.{layer}.ffn_gate_exps.weight"))?;
    let gate_exps_b = find_tensor(index, &format!("blk.{layer}.ffn_gate_exps.bias"))?;
    let up_exps_w = find_tensor(index, &format!("blk.{layer}.ffn_up_exps.weight"))?;
    let up_exps_b = find_tensor(index, &format!("blk.{layer}.ffn_up_exps.bias"))?;
    let down_exps_w = find_tensor(index, &format!("blk.{layer}.ffn_down_exps.weight"))?;
    let down_exps_b = find_tensor(index, &format!("blk.{layer}.ffn_down_exps.bias"))?;

    let mut mlp_accum = vec![0.0f32; hidden.len()];
    let mut moe_error: Option<String> = None;
    for (expert_idx, weight) in expert_indices.iter().zip(expert_weights.iter()) {
        let expert_start = now_ms();
        let gate_quant = match fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            gate_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        let mut gate_out =
            match matmul_mxfp4_expert(&gate_quant, &normed, gate_exps_w, gpu, gpu_tracker).await {
                Ok(value) => value,
                Err(err) => {
                    moe_error = Some(err);
                    break;
                }
            };
        let gate_bias = match fetch_f32_row(gguf_url, gate_exps_b, *expert_idx).await {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        if let Err(err) =
            apply_bias_gpu(&mut gate_out, &gate_bias, gpu, gpu_tracker, allow_cpu_fallback).await
        {
            moe_error = Some(err);
            break;
        }

        let up_quant = match fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            up_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        let mut up_out =
            match matmul_mxfp4_expert(&up_quant, &normed, up_exps_w, gpu, gpu_tracker).await {
                Ok(value) => value,
                Err(err) => {
                    moe_error = Some(err);
                    break;
                }
            };
        let up_bias = match fetch_f32_row(gguf_url, up_exps_b, *expert_idx).await {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        if let Err(err) =
            apply_bias_gpu(&mut up_out, &up_bias, gpu, gpu_tracker, allow_cpu_fallback).await
        {
            moe_error = Some(err);
            break;
        }

        let swiglu_out = match swiglu_gpu(&gate_out, &up_out, gpu, gpu_tracker).await {
            Ok(value) => Some(value),
            Err(err) => {
                if !allow_cpu_fallback {
                    moe_error = Some(format!("swiglu gpu failed: {err}"));
                    None
                } else {
                    match swiglu(&gate_out, &up_out) {
                        Ok(value) => Some(value),
                        Err(err) => {
                            moe_error = Some(err);
                            None
                        }
                    }
                }
            }
        };
        let swiglu_out = match swiglu_out {
            Some(value) => value,
            None => break,
        };
        let down_quant = match fetch_mxfp4_expert_cached(
            state,
            gguf_url,
            down_exps_w,
            *expert_idx,
            &mut caches.experts,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        let mut down_out = match matmul_mxfp4_expert(
            &down_quant,
            &swiglu_out,
            down_exps_w,
            gpu,
            gpu_tracker,
        )
        .await
        {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        let down_bias = match fetch_f32_row(gguf_url, down_exps_b, *expert_idx).await {
            Ok(value) => value,
            Err(err) => {
                moe_error = Some(err);
                break;
            }
        };
        if let Err(err) =
            apply_bias_gpu(&mut down_out, &down_bias, gpu, gpu_tracker, allow_cpu_fallback).await
        {
            moe_error = Some(err);
            break;
        }

        match scale_add_gpu(&mlp_accum, &down_out, *weight, gpu, gpu_tracker).await {
            Ok(value) => {
                mlp_accum = value;
            }
            Err(err) => {
                if !allow_cpu_fallback {
                    moe_error = Some(format!("moe_accum gpu failed: {err}"));
                    break;
                }
                for (acc, val) in mlp_accum.iter_mut().zip(down_out.iter()) {
                    *acc += *val * *weight;
                }
            }
        }
        let expert_ms = now_ms().saturating_sub(expert_start).max(1);
        emit_inference_stage(
            state,
            "moe_expert",
            StageStatus::Completed,
            None,
            None,
            Some(format!("layer={layer} expert={expert_idx} ms={expert_ms}")),
        );
    }
    if let Some(err) = moe_error {
        caches.moe_disabled = true;
        emit_inference_stage(
            state,
            "moe_expert",
            StageStatus::Failed,
            None,
            None,
            Some(format!("layer={layer} err={err}")),
        );
        emit_inference_stage(
            state,
            "moe_mode",
            StageStatus::Completed,
            None,
            None,
            Some("fallback (mlp skipped)".to_string()),
        );
    }

    hidden = match vector_add_gpu(&hidden, &mlp_accum, gpu, gpu_tracker).await {
        Ok(value) => value,
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("mlp_residual gpu failed: {err}"));
            }
            for (out, add) in hidden.iter_mut().zip(mlp_accum.iter()) {
                *out += *add;
            }
            hidden
        }
    };

    let layer_mlp_ms = now_ms().saturating_sub(layer_mlp_start).max(1);
    emit_inference_stage(
        state,
        "layer_mlp",
        StageStatus::Completed,
        Some((layer + 1) as usize),
        Some(total_layers),
        Some(format!("ok ms={layer_mlp_ms}")),
    );

    emit_inference_event(
        state,
        GptOssInferenceTelemetry::LayerActivation {
            layer: layer as usize,
            attention_norm: l2_norm(&attn_proj),
            mlp_norm: l2_norm(&mlp_accum),
            output_norm: l2_norm(&hidden),
        },
    );

    Ok(hidden)
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

async fn fetch_f32_tensor_raw(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
) -> Result<Vec<f32>, String> {
    if tensor.ggml_type != 0 {
        return Err(format!(
            "tensor {} is {}, expected F32",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let bytes = fetch_range_source(gguf_url, tensor.absolute_offset, tensor.nbytes).await?;
    if bytes.len() % 4 != 0 {
        return Err(format!("tensor {} f32 byte len mismatch", tensor.name));
    }
    let mut floats = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        floats.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(floats)
}

async fn fetch_f32_tensor_cached(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    cache: &mut TensorCache,
) -> Result<Vec<f32>, String> {
    if let Some(hit) = cache.get(&tensor.name) {
        emit_inference_stage(
            state,
            "tensor_fetch",
            StageStatus::Completed,
            None,
            None,
            Some(format!("{} cache=hit", tensor.name)),
        );
        return Ok(hit);
    }
    let start_ms = now_ms();
    emit_inference_stage(
        state,
        "tensor_fetch",
        StageStatus::Started,
        None,
        None,
        Some(format!(
            "{} bytes={}",
            tensor.name,
            format_bytes(tensor.nbytes)
        )),
    );
    let data = fetch_f32_tensor_raw(gguf_url, tensor).await?;
    let elapsed_ms = now_ms().saturating_sub(start_ms).max(1);
    emit_tensor_resident(state, tensor.name.clone(), data.len() * 4, "f32");
    cache.insert(tensor.name.clone(), data.clone());
    emit_inference_stage(
        state,
        "tensor_fetch",
        StageStatus::Completed,
        None,
        None,
        Some(format!("{} cache=miss ms={elapsed_ms}", tensor.name)),
    );
    Ok(data)
}

async fn fetch_f32_row(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    row: usize,
) -> Result<Vec<f32>, String> {
    if tensor.ggml_type != 0 {
        return Err(format!(
            "tensor {} is {}, expected F32",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let dims = &tensor.dims;
    let rows = dims.get(0).copied().unwrap_or(0) as usize;
    let cols = dims.get(1).copied().unwrap_or(0) as usize;
    if row >= rows || cols == 0 {
        return Err(format!("row {row} out of range for {}", tensor.name));
    }
    let row_bytes = cols * 4;
    let offset = tensor
        .absolute_offset
        .saturating_add((row_bytes * row) as u64);
    let bytes = fetch_range_source(gguf_url, offset, row_bytes as u64).await?;
    if bytes.len() % 4 != 0 {
        return Err(format!("tensor {} f32 row len mismatch", tensor.name));
    }
    let mut floats = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        floats.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(floats)
}

async fn fetch_q8_0_row(
    gguf_url: &GgufSource,
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
    let bytes = fetch_range_source(gguf_url, offset, row_bytes as u64).await?;
    let values = cols;
    let mut quant = bytes;
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }
    dequant_q8_0(&quant, values)
}

async fn fetch_q8_0_row_gpu(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    row: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
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
    let bytes = fetch_range_source(gguf_url, offset, row_bytes as u64).await?;
    let mut quant = bytes;
    if quant.len() % 4 != 0 {
        let padded = (quant.len() + 3) / 4 * 4;
        quant.resize(padded, 0);
    }
    dequant_q8_0_gpu(&quant, cols, gpu, gpu_tracker).await
}

async fn fetch_q8_0_row_cached(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    row: usize,
    cache: &mut TokenCache,
) -> Result<(Vec<f32>, bool), String> {
    if let Ok(token_id) = u32::try_from(row) {
        if let Some(hit) = cache.get(token_id) {
            return Ok((hit, true));
        }
        let data = fetch_q8_0_row(gguf_url, tensor, row).await?;
        cache.insert(token_id, data.clone());
        return Ok((data, false));
    }
    let data = fetch_q8_0_row(gguf_url, tensor, row).await?;
    Ok((data, false))
}

async fn fetch_q8_0_row_cached_gpu(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    row: usize,
    cache: &mut TokenCache,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<(Vec<f32>, bool), String> {
    if let Ok(token_id) = u32::try_from(row) {
        if let Some(hit) = cache.get(token_id) {
            return Ok((hit, true));
        }
        let data = fetch_q8_0_row_gpu(gguf_url, tensor, row, gpu, gpu_tracker).await?;
        cache.insert(token_id, data.clone());
        return Ok((data, false));
    }
    let data = fetch_q8_0_row_gpu(gguf_url, tensor, row, gpu, gpu_tracker).await?;
    Ok((data, false))
}

async fn fetch_mxfp4_expert_raw(
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    expert_idx: usize,
) -> Result<Vec<u8>, String> {
    if tensor.ggml_type != 39 {
        return Err(format!(
            "tensor {} is {}, expected MXFP4",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let dims = &tensor.dims;
    let experts = dims.get(0).copied().unwrap_or(0) as usize;
    let n = dims.get(1).copied().unwrap_or(0) as usize;
    let k = dims.get(2).copied().unwrap_or(0) as usize;
    if expert_idx >= experts || n == 0 || k == 0 {
        return Err(format!("expert {expert_idx} out of range for {}", tensor.name));
    }
    let values = n
        .checked_mul(k)
        .ok_or_else(|| "mxfp4 expert value overflow".to_string())?;
    if values % MXFP4_BLOCK_VALUES != 0 {
        return Err("mxfp4 expert values not divisible by block size".to_string());
    }
    let blocks = values / MXFP4_BLOCK_VALUES;
    let bytes_needed = blocks * MXFP4_BLOCK_BYTES;
    let offset = tensor
        .absolute_offset
        .saturating_add((expert_idx * bytes_needed) as u64);
    let mut bytes = fetch_range_source(gguf_url, offset, bytes_needed as u64).await?;
    if bytes.len() % 4 != 0 {
        let padded = (bytes.len() + 3) / 4 * 4;
        bytes.resize(padded, 0);
    }
    Ok(bytes)
}

async fn fetch_mxfp4_expert_cached(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    tensor: &GgufTensor,
    expert_idx: usize,
    cache: &mut ExpertCache,
) -> Result<Vec<u8>, String> {
    let key = format!("{}#{}", tensor.name, expert_idx);
    if let Some(hit) = cache.get(&key) {
        emit_inference_stage(
            state,
            "expert_fetch",
            StageStatus::Completed,
            None,
            None,
            Some(format!("{} cache=hit", key)),
        );
        return Ok(hit);
    }
    let start_ms = now_ms();
    emit_inference_stage(
        state,
        "expert_fetch",
        StageStatus::Started,
        None,
        None,
        Some(format!("{} bytes=~{}", key, format_bytes(tensor.nbytes))),
    );
    let data = fetch_mxfp4_expert_raw(gguf_url, tensor, expert_idx).await?;
    let elapsed_ms = now_ms().saturating_sub(start_ms).max(1);
    emit_tensor_resident(state, key.clone(), data.len(), "mxfp4");
    cache.insert(key.clone(), data.clone());
    emit_inference_stage(
        state,
        "expert_fetch",
        StageStatus::Completed,
        None,
        None,
        Some(format!("{} cache=miss ms={elapsed_ms}", key)),
    );
    Ok(data)
}

async fn matmul_q8_0_with_bias(
    state: &Rc<RefCell<AppState>>,
    gguf_url: &GgufSource,
    weight: &GgufTensor,
    bias: &GgufTensor,
    input: &[f32],
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
    cache: &mut TensorCache,
    quant_cache: &mut QuantCache,
    allow_cpu_fallback: bool,
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
    let max_storage = gpu.device.limits().max_storage_buffer_binding_size as usize;
    let weight_bytes = usize::try_from(weight.nbytes).unwrap_or(usize::MAX);
    let chunked = weight_bytes > max_storage;
    let start_ms = now_ms();
    let mut cached_quant = None;
    let cache_note = if chunked {
        "cache=skip"
    } else {
        cached_quant = quant_cache.get(&weight.name);
        if cached_quant.is_some() {
            "cache=hit"
        } else {
            "cache=miss"
        }
    };
    emit_inference_stage(
        state,
        "weights_fetch",
        StageStatus::Started,
        None,
        None,
        Some(if chunked {
            format!(
                "{} bytes={} (chunked)",
                weight.name,
                format_bytes(weight.nbytes)
            )
        } else {
            format!(
                "{} bytes={} {cache_note}",
                weight.name,
                format_bytes(weight.nbytes)
            )
        }),
    );
    let mut retry_chunked = false;
    let mut out = if chunked {
        gpu_matmul_q8_0_chunked(gguf_url, weight, input, gpu, gpu_tracker).await?
    } else {
        let quant = if let Some(hit) = cached_quant {
            hit
        } else {
            let mut quant =
                fetch_range_source(gguf_url, weight.absolute_offset, weight.nbytes).await?;
            if quant.len() % 4 != 0 {
                let padded = (quant.len() + 3) / 4 * 4;
                quant.resize(padded, 0);
            }
            emit_tensor_resident(state, weight.name.clone(), quant.len(), "q8_0");
            quant_cache.insert(weight.name.clone(), quant.clone());
            quant
        };
        match gpu_matmul_q8_0(&quant, input, k, n, gpu, gpu_tracker).await {
            Ok(out) => out,
            Err(err) => {
                retry_chunked = true;
                emit_inference_stage(
                    state,
                    "weights_retry",
                    StageStatus::Progress,
                    None,
                    None,
                    Some(format!("{} retry=chunked err={err}", weight.name)),
                );
                gpu_matmul_q8_0_chunked(gguf_url, weight, input, gpu, gpu_tracker).await?
            }
        }
    };
    let fetch_ms = now_ms().saturating_sub(start_ms).max(1);
    emit_inference_stage(
        state,
        "weights_fetch",
        StageStatus::Completed,
        None,
        None,
        Some(if chunked {
            format!(
                "{} bytes={} ms={fetch_ms} (chunked)",
                weight.name,
                format_bytes(weight.nbytes)
            )
        } else if retry_chunked {
            format!(
                "{} bytes={} ms={fetch_ms} (retry chunked)",
                weight.name,
                format_bytes(weight.nbytes)
            )
        } else {
            format!(
                "{} bytes={} ms={fetch_ms} {cache_note}",
                weight.name,
                format_bytes(weight.nbytes)
            )
        }),
    );
    let bias_vals = fetch_f32_tensor_cached(state, gguf_url, bias, cache).await?;
    if bias_vals.len() == out.len() {
        if let Err(err) =
            apply_bias_gpu(&mut out, &bias_vals, gpu, gpu_tracker, allow_cpu_fallback).await
        {
            return Err(err);
        }
    }
    Ok(out)
}

async fn matmul_mxfp4_expert(
    quant: &[u8],
    input: &[f32],
    tensor: &GgufTensor,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if tensor.ggml_type != 39 {
        return Err(format!(
            "tensor {} is {}, expected MXFP4",
            tensor.name, tensor.ggml_type_name
        ));
    }
    let dims = &tensor.dims;
    let n = dims.get(1).copied().unwrap_or(0) as usize;
    let k = dims.get(2).copied().unwrap_or(0) as usize;
    if input.len() != k || n == 0 {
        return Err(format!(
            "matmul shape mismatch for {} (k={}, n={}, input={})",
            tensor.name,
            k,
            n,
            input.len()
        ));
    }
    gpu_matmul_mxfp4(quant, input, k, n, gpu, gpu_tracker).await
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

fn apply_bias(values: &mut [f32], bias: &[f32]) {
    if bias.len() != values.len() {
        return;
    }
    for (v, b) in values.iter_mut().zip(bias.iter()) {
        *v += *b;
    }
}

async fn apply_bias_gpu(
    values: &mut Vec<f32>,
    bias: &[f32],
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
    allow_cpu_fallback: bool,
) -> Result<(), String> {
    if bias.len() != values.len() {
        return Ok(());
    }
    match vector_add_gpu(values, bias, gpu, gpu_tracker).await {
        Ok(out) => {
            *values = out;
        }
        Err(err) => {
            if !allow_cpu_fallback {
                return Err(format!("bias gpu failed: {err}"));
            }
            apply_bias(values, bias);
        }
    }
    Ok(())
}

fn swiglu(gate: &[f32], up: &[f32]) -> Result<Vec<f32>, String> {
    if gate.len() != up.len() {
        return Err("swiglu shape mismatch".to_string());
    }
    let mut out = Vec::with_capacity(gate.len());
    for (&g, &u) in gate.iter().zip(up.iter()) {
        let g_clamped = g.min(SWIGLU_LIMIT);
        let u_clamped = u.max(-SWIGLU_LIMIT).min(SWIGLU_LIMIT);
        let sigmoid = 1.0 / (1.0 + (-SWIGLU_ALPHA * g_clamped).exp());
        let glu = g_clamped * sigmoid;
        out.push(glu * (u_clamped + 1.0));
    }
    Ok(out)
}

fn attention_single_token(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    sinks: &[f32],
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
) -> Result<Vec<f32>, String> {
    if heads == 0 || kv_heads == 0 || head_dim == 0 {
        return Err("attention invalid dims".to_string());
    }
    if q.len() != heads * head_dim {
        return Err("attention q shape mismatch".to_string());
    }
    if k.len() != kv_heads * head_dim || v.len() != kv_heads * head_dim {
        return Err("attention kv shape mismatch".to_string());
    }

    let sm_scale = 1.0 / (head_dim as f32).sqrt();
    let mut out = vec![0.0f32; heads * head_dim];
    for h in 0..heads {
        let q_base = h * head_dim;
        let kv = h % kv_heads;
        let k_base = kv * head_dim;
        let mut dot = 0.0f32;
        for i in 0..head_dim {
            dot += q[q_base + i] * k[k_base + i];
        }
        let score = dot * sm_scale;
        let sink = sinks.get(h).copied().unwrap_or(0.0);
        let w = score.exp();
        let s = sink.exp();
        let weight = w / (w + s);
        let v_base = k_base;
        for i in 0..head_dim {
            out[q_base + i] = v[v_base + i] * weight;
        }
    }
    Ok(out)
}

fn attention_with_cache(
    q: &[f32],
    cache: &LayerKvCache,
    sinks: &[f32],
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    window: usize,
) -> Result<Vec<f32>, String> {
    if heads == 0 || kv_heads == 0 || head_dim == 0 {
        return Err("attention invalid dims".to_string());
    }
    let stride = kv_heads
        .checked_mul(head_dim)
        .ok_or_else(|| "attention stride overflow".to_string())?;
    if cache.capacity == 0 || cache.stride != stride || cache.k.len() != cache.v.len() {
        return Err("attention cache uninitialized".to_string());
    }
    if q.len() != heads * head_dim {
        return Err("attention q shape mismatch".to_string());
    }

    let token_count = cache.len;
    if token_count == 0 {
        return Err("attention cache empty".to_string());
    }
    let window = window.max(1).min(token_count);
    let start = (cache.start + token_count.saturating_sub(window)) % cache.capacity;
    let sm_scale = 1.0 / (head_dim as f32).sqrt();
    let mut out = vec![0.0f32; heads * head_dim];

    for h in 0..heads {
        let q_base = h * head_dim;
        let kv = h % kv_heads;
        let sink = sinks.get(h).copied().unwrap_or(0.0);
        let mut max_score = sink;

        for t in 0..window {
            let token = (start + t) % cache.capacity;
            let k_base = (token * kv_heads + kv) * head_dim;
            let mut dot = 0.0f32;
            for i in 0..head_dim {
                dot += q[q_base + i] * cache.k[k_base + i];
            }
            let score = dot * sm_scale;
            if score > max_score {
                max_score = score;
            }
        }

        let mut weights = Vec::with_capacity(window);
        let mut denom = (sink - max_score).exp();
        for t in 0..window {
            let token = (start + t) % cache.capacity;
            let k_base = (token * kv_heads + kv) * head_dim;
            let mut dot = 0.0f32;
            for i in 0..head_dim {
                dot += q[q_base + i] * cache.k[k_base + i];
            }
            let score = dot * sm_scale;
            let w = (score - max_score).exp();
            weights.push(w);
            denom += w;
        }

        if denom <= 0.0 {
            return Err("attention softmax denom is zero".to_string());
        }

        for (idx, w) in weights.iter().enumerate() {
            let weight = *w / denom;
            let token = (start + idx) % cache.capacity;
            let v_base = (token * kv_heads + kv) * head_dim;
            for i in 0..head_dim {
                out[q_base + i] += cache.v[v_base + i] * weight;
            }
        }
    }

    Ok(out)
}

fn attention_head_weights(
    q: &[f32],
    cache: &LayerKvCache,
    sinks: &[f32],
    head_index: usize,
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    window: usize,
) -> Result<Vec<f32>, String> {
    if heads == 0 || kv_heads == 0 || head_dim == 0 {
        return Err("attention invalid dims".to_string());
    }
    if head_index >= heads {
        return Err("attention head index out of range".to_string());
    }
    let stride = kv_heads
        .checked_mul(head_dim)
        .ok_or_else(|| "attention stride overflow".to_string())?;
    if cache.capacity == 0 || cache.stride != stride || cache.k.len() != cache.v.len() {
        return Err("attention cache uninitialized".to_string());
    }
    if q.len() != heads * head_dim {
        return Err("attention q shape mismatch".to_string());
    }

    let token_count = cache.len;
    if token_count == 0 {
        return Err("attention cache empty".to_string());
    }
    let window = window.max(1).min(token_count);
    let start = (cache.start + token_count.saturating_sub(window)) % cache.capacity;
    let sm_scale = 1.0 / (head_dim as f32).sqrt();

    let q_base = head_index * head_dim;
    let kv = head_index % kv_heads;
    let sink = sinks.get(head_index).copied().unwrap_or(0.0);
    let mut max_score = sink;
    for t in 0..window {
        let token = (start + t) % cache.capacity;
        let k_base = (token * kv_heads + kv) * head_dim;
        let mut dot = 0.0f32;
        for i in 0..head_dim {
            dot += q[q_base + i] * cache.k[k_base + i];
        }
        let score = dot * sm_scale;
        if score > max_score {
            max_score = score;
        }
    }

    let mut weights = Vec::with_capacity(window);
    let mut denom = (sink - max_score).exp();
    for t in 0..window {
        let token = (start + t) % cache.capacity;
        let k_base = (token * kv_heads + kv) * head_dim;
        let mut dot = 0.0f32;
        for i in 0..head_dim {
            dot += q[q_base + i] * cache.k[k_base + i];
        }
        let score = dot * sm_scale;
        let w = (score - max_score).exp();
        weights.push(w);
        denom += w;
    }

    if denom <= 0.0 {
        return Err("attention softmax denom is zero".to_string());
    }
    for weight in &mut weights {
        *weight /= denom;
    }
    Ok(weights)
}

struct RopeScaling {
    theta: f32,
    scaling_factor: f32,
    concentration: f32,
    low: f32,
    high: f32,
    use_yarn: bool,
}

fn compute_rope_scaling(
    theta: f32,
    rope_dim: usize,
    rope_scaling_factor: f32,
    rope_scaling_original_context: u32,
) -> RopeScaling {
    let theta = if theta <= 0.0 { 10000.0 } else { theta };
    let scaling_factor = rope_scaling_factor.max(1.0);
    let original_context = rope_scaling_original_context as f32;
    let use_yarn = scaling_factor > 1.0 && original_context > 0.0;
    let concentration = if use_yarn {
        0.1 * scaling_factor.ln() + 1.0
    } else {
        1.0
    };
    let theta_log = theta.ln();
    let d_half = rope_dim as f32 / 2.0;
    let mut low = 0.0f32;
    let mut high = 0.0f32;
    if use_yarn {
        let denom = theta_log.max(1e-6);
        low = d_half
            * (original_context / (ROPE_NTK_BETA * 2.0 * std::f32::consts::PI)).ln()
            / denom;
        high = d_half
            * (original_context / (ROPE_NTK_ALPHA * 2.0 * std::f32::consts::PI)).ln()
            / denom;
        if !(low > 0.0 && high > low && high < d_half - 1.0) {
            low = 0.0;
            high = 0.0;
        }
    }
    RopeScaling {
        theta,
        scaling_factor,
        concentration,
        low,
        high,
        use_yarn,
    }
}

fn apply_rope(
    values: &mut [f32],
    heads: usize,
    head_dim: usize,
    position: usize,
    theta: f32,
    rope_dim: u32,
    rope_scaling_factor: f32,
    rope_scaling_original_context: u32,
) -> Result<(), String> {
    if head_dim == 0 || heads == 0 {
        return Err("rope invalid head dims".to_string());
    }
    let expected = heads
        .checked_mul(head_dim)
        .ok_or_else(|| "rope shape overflow".to_string())?;
    if values.len() != expected {
        return Err(format!(
            "rope shape mismatch values={} heads={} head_dim={}",
            values.len(),
            heads,
            head_dim
        ));
    }
    let rope_dim = rope_dim.min(head_dim as u32) as usize;
    if rope_dim == 0 {
        return Ok(());
    }
    if rope_dim % 2 != 0 {
        return Err("rope_dim must be even".to_string());
    }

    let scaling = compute_rope_scaling(
        theta,
        rope_dim,
        rope_scaling_factor,
        rope_scaling_original_context,
    );
    let theta = scaling.theta;
    let scaling_factor = scaling.scaling_factor;
    let use_yarn = scaling.use_yarn;
    let concentration = scaling.concentration;
    let low = scaling.low;
    let high = scaling.high;
    for h in 0..heads {
        let base = h * head_dim;
        for i in (0..rope_dim).step_by(2) {
            let freq = theta.powf(i as f32 / rope_dim as f32);
            let mut inv_freq = 1.0 / freq;
            if use_yarn && high > low {
                let t = (i / 2) as f32;
                let ramp = (t - low) / (high - low);
                let mask = 1.0 - ramp.clamp(0.0, 1.0);
                let interp = 1.0 / (scaling_factor * freq);
                let extrap = 1.0 / freq;
                inv_freq = interp * (1.0 - mask) + extrap * mask;
            }
            let angle = position as f32 * inv_freq;
            let (sin, cos) = angle.sin_cos();
            let sin = sin * concentration;
            let cos = cos * concentration;
            let a = values[base + i];
            let b = values[base + i + 1];
            values[base + i] = a * cos - b * sin;
            values[base + i + 1] = a * sin + b * cos;
        }
    }
    Ok(())
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

fn dequant_mxfp4(data: &[u8], values: usize) -> Result<Vec<f32>, String> {
    if values % MXFP4_BLOCK_VALUES != 0 {
        return Err("value count not divisible by MXFP4 block size".to_string());
    }
    let blocks = values / MXFP4_BLOCK_VALUES;
    let needed = blocks * MXFP4_BLOCK_BYTES;
    if data.len() < needed {
        return Err(format!(
            "insufficient MXFP4 data: need {needed}, have {}",
            data.len()
        ));
    }

    let mut out = vec![0.0f32; values];
    for block in 0..blocks {
        let base = block * MXFP4_BLOCK_BYTES;
        let scale_byte = data[base];
        let scale = (2.0f32).powi(scale_byte as i32 - 127);
        for i in 0..MXFP4_BLOCK_VALUES {
            let byte = data[base + 1 + i / 2];
            let nibble = if i % 2 == 0 { byte & 0x0F } else { byte >> 4 };
            let value = MXFP4_VALUES[nibble as usize] * scale;
            out[block * MXFP4_BLOCK_VALUES + i] = value;
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

fn linear_f32_with_bias(
    weights: &[f32],
    bias: &[f32],
    x: &[f32],
    tensor: &GgufTensor,
) -> Result<Vec<f32>, String> {
    let dims = &tensor.dims;
    let n = dims.get(0).copied().unwrap_or(0) as usize;
    let k = dims.get(1).copied().unwrap_or(0) as usize;
    if x.len() != k || n == 0 {
        return Err(format!(
            "linear shape mismatch for {} (k={}, n={}, input={})",
            tensor.name,
            k,
            n,
            x.len()
        ));
    }
    if weights.len() != k * n {
        return Err(format!(
            "linear weight size mismatch for {} (have={}, want={})",
            tensor.name,
            weights.len(),
            k * n
        ));
    }
    let mut y = matmul_cpu(weights, x, k, n);
    if bias.len() == n {
        for (out, b) in y.iter_mut().zip(bias.iter()) {
            *out += *b;
        }
    }
    Ok(y)
}

async fn linear_f32_with_bias_gpu(
    weights: &[f32],
    bias: &[f32],
    x: &[f32],
    tensor: &GgufTensor,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
    allow_cpu_fallback: bool,
) -> Result<Vec<f32>, String> {
    let dims = &tensor.dims;
    let n = dims.get(0).copied().unwrap_or(0) as usize;
    let k = dims.get(1).copied().unwrap_or(0) as usize;
    if x.len() != k || n == 0 {
        return Err(format!(
            "linear shape mismatch for {} (k={}, n={}, input={})",
            tensor.name,
            k,
            n,
            x.len()
        ));
    }
    if weights.len() != k * n {
        return Err(format!(
            "linear weight size mismatch for {} (have={}, want={})",
            tensor.name,
            weights.len(),
            k * n
        ));
    }
    let mut y = gpu_matmul_f32(weights, x, k, n, gpu, gpu_tracker).await?;
    if bias.len() == n {
        apply_bias_gpu(&mut y, bias, gpu, gpu_tracker, allow_cpu_fallback).await?;
    }
    Ok(y)
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

fn pick_workgroup_size(device: &wgpu::Device) -> u32 {
    let limits = device.limits();
    let max_x = limits.max_compute_workgroup_size_x;
    let max_invocations = limits.max_compute_invocations_per_workgroup;
    let max_size = max_x.min(max_invocations).max(1);
    for candidate in [256u32, 128, 64, 32, 16, 8, 4, 2, 1] {
        if candidate <= max_size {
            return candidate;
        }
    }
    1
}

fn kv_limit_for_gpu(
    config: &GptOssConfig,
    gpu: &GpuContext,
    budget_bytes: u64,
) -> Option<KvLimit> {
    let head_count = config.head_count.max(1) as usize;
    let kv_heads = config.head_count_kv.max(1) as usize;
    let embed = config.embedding_length as usize;
    if embed == 0 {
        return None;
    }
    let head_dim = (embed / head_count).max(1);
    let stride = kv_heads.checked_mul(head_dim)?;
    let bytes_per_token = u64::try_from(stride)
        .ok()?
        .checked_mul(std::mem::size_of::<f32>() as u64)?;
    if bytes_per_token == 0 {
        return None;
    }
    let limits = gpu.device.limits();
    let max_storage = limits.max_storage_buffer_binding_size as usize;
    let max_buffer = limits.max_buffer_size as usize;
    let max_bytes = max_storage.min(max_buffer);
    let per_layer_max_u64 = (max_bytes as u64) / bytes_per_token;
    let per_layer_max = usize::try_from(per_layer_max_u64).unwrap_or(usize::MAX);
    let layer_count = config.block_count.max(1) as usize;
    let budget_max_u64 = if layer_count == 0 || budget_bytes == 0 {
        None
    } else {
        let per_token_all_layers = bytes_per_token
            .checked_mul(layer_count as u64)
            .filter(|value| *value > 0)?;
        Some(budget_bytes / per_token_all_layers)
    };
    let budget_max = budget_max_u64
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0);
    let mut max_tokens = per_layer_max.max(1);
    if let Some(budget_tokens) = budget_max {
        max_tokens = max_tokens.min(budget_tokens.max(1));
    }
    Some(KvLimit {
        max_tokens,
        per_layer_max: per_layer_max.max(1),
        budget_max,
        budget_bytes,
    })
}

fn shader_with_workgroup(source: &str, workgroup_size: u32) -> String {
    source.replace("{{WORKGROUP_SIZE}}", &workgroup_size.to_string())
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RmsNormParams {
    n: u32,
    _pad0: u32,
    eps: f32,
    _pad1: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RopeParams {
    heads: u32,
    head_dim: u32,
    rope_dim: u32,
    position: u32,
    theta: f32,
    scaling_factor: f32,
    low: f32,
    high: f32,
    concentration: f32,
    use_yarn: u32,
    _pad0: u32,
    _pad1: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AttentionParams {
    heads: u32,
    kv_heads: u32,
    head_dim: u32,
    seq_len: u32,
    window_start: u32,
    capacity: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DequantParams {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct VecAddParams {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SwigluParams {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct F32MatmulParams {
    k: u32,
    n: u32,
    _pad0: u32,
    _pad1: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct AttentionWeightsParams {
    head_index: u32,
    kv_heads: u32,
    head_dim: u32,
    seq_len: u32,
    window_start: u32,
    capacity: u32,
    _pad0: u32,
    _pad1: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ScaleAddParams {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    weight: f32,
    _pad3: u32,
    _pad4: u32,
    _pad5: u32,
}

async fn rms_norm_gpu(
    input: &[f32],
    weight: &[f32],
    eps: f32,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if input.len() != weight.len() {
        return Err("rms_norm shape mismatch".to_string());
    }
    let n = input.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let device = &gpu.device;
    let queue = &gpu.queue;
    let bytes = n * std::mem::size_of::<f32>();
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    ensure_storage_limit("rms_norm input", bytes, max_storage)?;
    ensure_storage_limit("rms_norm weight", bytes, max_storage)?;
    ensure_storage_limit("rms_norm output", bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(RMSNORM_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rmsnorm"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rmsnorm_bindings"),
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
        label: Some("rmsnorm_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("rmsnorm_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rmsnorm_input"),
        contents: cast_slice(input),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let weight_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rmsnorm_weight"),
        contents: cast_slice(weight),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rmsnorm_output"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = RmsNormParams {
        n: u32::try_from(n).map_err(|_| "rms_norm length overflow".to_string())?,
        _pad0: 0,
        eps,
        _pad1: 0,
    };
    let params_bytes = std::mem::size_of::<RmsNormParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rmsnorm_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rmsnorm_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: weight_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rmsnorm_readback"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(bytes * 4 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("rmsnorm_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("rmsnorm_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback, 0, bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("rmsnorm map_async failed: {err:?}")),
        Err(_) => return Err("rmsnorm map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn apply_rope_gpu(
    values: &[f32],
    heads: usize,
    head_dim: usize,
    position: usize,
    theta: f32,
    rope_dim: u32,
    rope_scaling_factor: f32,
    rope_scaling_original_context: u32,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if head_dim == 0 || heads == 0 {
        return Err("rope invalid head dims".to_string());
    }
    let expected = heads
        .checked_mul(head_dim)
        .ok_or_else(|| "rope shape overflow".to_string())?;
    if values.len() != expected {
        return Err(format!(
            "rope shape mismatch values={} heads={} head_dim={}",
            values.len(),
            heads,
            head_dim
        ));
    }
    let rope_dim = rope_dim.min(head_dim as u32) as usize;
    if rope_dim == 0 {
        return Ok(values.to_vec());
    }
    if rope_dim % 2 != 0 {
        return Err("rope_dim must be even".to_string());
    }
    let pairs = heads
        .checked_mul(rope_dim / 2)
        .ok_or_else(|| "rope pair overflow".to_string())?;
    if pairs == 0 {
        return Ok(values.to_vec());
    }

    let scaling = compute_rope_scaling(theta, rope_dim, rope_scaling_factor, rope_scaling_original_context);
    let device = &gpu.device;
    let queue = &gpu.queue;
    let bytes = values.len() * std::mem::size_of::<f32>();
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    ensure_storage_limit("rope values", bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(ROPE_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rope"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rope_bindings"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
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
        label: Some("rope_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("rope_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let values_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rope_values"),
        contents: cast_slice(values),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    });
    let params = RopeParams {
        heads: u32::try_from(heads).map_err(|_| "rope heads overflow".to_string())?,
        head_dim: u32::try_from(head_dim).map_err(|_| "rope head_dim overflow".to_string())?,
        rope_dim: u32::try_from(rope_dim).map_err(|_| "rope rope_dim overflow".to_string())?,
        position: u32::try_from(position).map_err(|_| "rope position overflow".to_string())?,
        theta: scaling.theta,
        scaling_factor: scaling.scaling_factor,
        low: scaling.low,
        high: scaling.high,
        concentration: scaling.concentration,
        use_yarn: if scaling.use_yarn { 1 } else { 0 },
        _pad0: 0,
        _pad1: 0,
    };
    let params_bytes = std::mem::size_of::<RopeParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("rope_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("rope_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: values_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("rope_readback"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(bytes * 2 + params_bytes, 3);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("rope_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("rope_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let pair_count = u32::try_from(pairs).map_err(|_| "rope pair count overflow".to_string())?;
        let groups = (pair_count + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&values_buffer, 0, &readback, 0, bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("rope map_async failed: {err:?}")),
        Err(_) => return Err("rope map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn dequant_q8_0_gpu(
    quant: &[u8],
    n: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if n == 0 {
        return Ok(Vec::new());
    }
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let out_bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("q8_0 dequant input", quant.len(), max_storage)?;
    ensure_storage_limit("q8_0 dequant output", out_bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(Q8_0_DEQUANT_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("q8_0_dequant"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("q8_0_dequant_bindings"),
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
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
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
        label: Some("q8_0_dequant_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("q8_0_dequant_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let quant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_dequant_quant"),
        contents: quant,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_dequant_out"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = DequantParams {
        n: u32::try_from(n).map_err(|_| "q8_0 dequant n overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };
    let params_bytes = std::mem::size_of::<DequantParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_dequant_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("q8_0_dequant_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: quant_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_dequant_readback"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(quant.len() + out_bytes * 2 + params_bytes, 4);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("q8_0_dequant_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("q8_0_dequant_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("q8_0 dequant map_async failed: {err:?}")),
        Err(_) => return Err("q8_0 dequant map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn vector_add_gpu(
    a: &[f32],
    b: &[f32],
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if a.len() != b.len() {
        return Err("vec_add shape mismatch".to_string());
    }
    let n = a.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("vec_add a", bytes, max_storage)?;
    ensure_storage_limit("vec_add b", bytes, max_storage)?;
    ensure_storage_limit("vec_add out", bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(VEC_ADD_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vec_add"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("vec_add_bindings"),
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
        label: Some("vec_add_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("vec_add_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let a_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vec_add_a"),
        contents: cast_slice(a),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let b_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vec_add_b"),
        contents: cast_slice(b),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vec_add_out"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = VecAddParams {
        n: u32::try_from(n).map_err(|_| "vec_add n overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };
    let params_bytes = std::mem::size_of::<VecAddParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vec_add_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vec_add_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: a_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: b_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vec_add_readback"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(bytes * 3 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("vec_add_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("vec_add_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("vec_add map_async failed: {err:?}")),
        Err(_) => return Err("vec_add map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn scale_add_gpu(
    acc: &[f32],
    input: &[f32],
    weight: f32,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if acc.len() != input.len() {
        return Err("scale_add shape mismatch".to_string());
    }
    let n = acc.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("scale_add acc", bytes, max_storage)?;
    ensure_storage_limit("scale_add input", bytes, max_storage)?;
    ensure_storage_limit("scale_add out", bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(SCALE_ADD_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("scale_add"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scale_add_bindings"),
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
        label: Some("scale_add_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("scale_add_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let acc_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scale_add_acc"),
        contents: cast_slice(acc),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scale_add_input"),
        contents: cast_slice(input),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scale_add_out"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = ScaleAddParams {
        n: u32::try_from(n).map_err(|_| "scale_add n overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
        weight,
        _pad3: 0,
        _pad4: 0,
        _pad5: 0,
    };
    let params_bytes = std::mem::size_of::<ScaleAddParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scale_add_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scale_add_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: acc_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("scale_add_readback"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(bytes * 3 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("scale_add_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("scale_add_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("scale_add map_async failed: {err:?}")),
        Err(_) => return Err("scale_add map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn swiglu_gpu(
    gate: &[f32],
    up: &[f32],
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if gate.len() != up.len() {
        return Err("swiglu shape mismatch".to_string());
    }
    let n = gate.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("swiglu gate", bytes, max_storage)?;
    ensure_storage_limit("swiglu up", bytes, max_storage)?;
    ensure_storage_limit("swiglu out", bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(SWIGLU_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("swiglu"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("swiglu_bindings"),
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
        label: Some("swiglu_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("swiglu_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let gate_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("swiglu_gate"),
        contents: cast_slice(gate),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let up_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("swiglu_up"),
        contents: cast_slice(up),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("swiglu_out"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = SwigluParams {
        n: u32::try_from(n).map_err(|_| "swiglu n overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };
    let params_bytes = std::mem::size_of::<SwigluParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("swiglu_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("swiglu_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: gate_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: up_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("swiglu_readback"),
        size: bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(bytes * 3 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("swiglu_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("swiglu_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("swiglu map_async failed: {err:?}")),
        Err(_) => return Err("swiglu map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn gpu_matmul_f32(
    weights: &[f32],
    x: &[f32],
    k: usize,
    n: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let weights_bytes = weights.len() * std::mem::size_of::<f32>();
    let x_bytes = x.len() * std::mem::size_of::<f32>();
    let y_bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("f32 weights", weights_bytes, max_storage)?;
    ensure_storage_limit("f32 input", x_bytes, max_storage)?;
    ensure_storage_limit("f32 output", y_bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(MATMUL_F32_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("f32_matmul"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("f32_matmul_bindings"),
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
        label: Some("f32_matmul_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("f32_matmul_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let weights_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("f32_matmul_weights"),
        contents: cast_slice(weights),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("f32_matmul_x"),
        contents: cast_slice(x),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let y_bytes = y_bytes as u64;
    let y_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("f32_matmul_y"),
        size: y_bytes,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = F32MatmulParams {
        k: u32::try_from(k).map_err(|_| "f32 matmul k overflow".to_string())?,
        n: u32::try_from(n).map_err(|_| "f32 matmul n overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
    };
    let params_bytes = std::mem::size_of::<F32MatmulParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("f32_matmul_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("f32_matmul_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: weights_buffer.as_entire_binding(),
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
        label: Some("f32_matmul_readback"),
        size: y_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(weights_bytes + x_bytes + (y_bytes as usize) * 2 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("f32_matmul_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("f32_matmul_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
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
        Ok(Err(err)) => return Err(format!("f32 matmul map_async failed: {err:?}")),
        Err(_) => return Err("f32 matmul map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn attention_with_cache_gpu(
    q: &[f32],
    cache: &LayerKvCache,
    sinks: &[f32],
    heads: usize,
    kv_heads: usize,
    head_dim: usize,
    window: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if heads == 0 || kv_heads == 0 || head_dim == 0 {
        return Err("attention invalid dims".to_string());
    }
    let stride = kv_heads
        .checked_mul(head_dim)
        .ok_or_else(|| "attention stride overflow".to_string())?;
    if cache.capacity == 0 || cache.stride != stride || cache.k.len() != cache.v.len() {
        return Err("attention cache uninitialized".to_string());
    }
    if q.len() != heads * head_dim {
        return Err("attention q shape mismatch".to_string());
    }
    let token_count = cache.len;
    if token_count == 0 {
        return Err("attention cache empty".to_string());
    }
    let window = window.max(1).min(token_count);
    let window_start = (cache.start + token_count.saturating_sub(window)) % cache.capacity;
    let k_buf = cache
        .gpu_k
        .as_ref()
        .ok_or_else(|| "attention gpu cache missing".to_string())?;
    let v_buf = cache
        .gpu_v
        .as_ref()
        .ok_or_else(|| "attention gpu cache missing".to_string())?;

    let sink_len = heads.max(1);
    let mut sink_values = vec![0.0f32; sink_len];
    for (idx, value) in sinks.iter().enumerate().take(sink_len) {
        sink_values[idx] = *value;
    }

    let device = &gpu.device;
    let queue = &gpu.queue;
    let q_bytes = q.len() * std::mem::size_of::<f32>();
    let k_bytes = cache
        .capacity
        .checked_mul(stride)
        .ok_or_else(|| "attention cache overflow".to_string())?
        * std::mem::size_of::<f32>();
    let v_bytes = k_bytes;
    let sink_bytes = sink_values.len() * std::mem::size_of::<f32>();
    let out_bytes = q_bytes;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    ensure_storage_limit("attn q", q_bytes, max_storage)?;
    ensure_storage_limit("attn k", k_bytes, max_storage)?;
    ensure_storage_limit("attn v", v_bytes, max_storage)?;
    ensure_storage_limit("attn sinks", sink_bytes, max_storage)?;
    ensure_storage_limit("attn out", out_bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device)
        .min(u32::try_from(heads).unwrap_or(1))
        .max(1);
    let shader_source = shader_with_workgroup(ATTENTION_DECODE_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("attn_decode"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("attn_decode_bindings"),
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
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
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
        label: Some("attn_decode_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("attn_decode_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let q_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_q"),
        contents: cast_slice(q),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let k_buffer = k_buf.clone();
    let v_buffer = v_buf.clone();
    let sink_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_sinks"),
        contents: cast_slice(&sink_values),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("attn_out"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = AttentionParams {
        heads: u32::try_from(heads).map_err(|_| "attn heads overflow".to_string())?,
        kv_heads: u32::try_from(kv_heads).map_err(|_| "attn kv_heads overflow".to_string())?,
        head_dim: u32::try_from(head_dim).map_err(|_| "attn head_dim overflow".to_string())?,
        seq_len: u32::try_from(window).map_err(|_| "attn seq_len overflow".to_string())?,
        window_start: u32::try_from(window_start)
            .map_err(|_| "attn window_start overflow".to_string())?,
        capacity: u32::try_from(cache.capacity)
            .map_err(|_| "attn capacity overflow".to_string())?,
    };
    let params_bytes = std::mem::size_of::<AttentionParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("attn_decode_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: q_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: k_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: v_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: sink_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("attn_readback"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(
        q_bytes + k_bytes + v_bytes + sink_bytes + out_bytes * 2 + params_bytes,
        7,
    );

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("attn_decode_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("attn_decode_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        let head_count = u32::try_from(heads).map_err(|_| "attn head count overflow".to_string())?;
        let groups = (head_count + workgroup_size - 1) / workgroup_size;
        pass.dispatch_workgroups(groups, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("attn map_async failed: {err:?}")),
        Err(_) => return Err("attn map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn attention_head_weights_gpu(
    q: &[f32],
    cache: &LayerKvCache,
    sinks: &[f32],
    head_index: usize,
    kv_heads: usize,
    head_dim: usize,
    window: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    if kv_heads == 0 || head_dim == 0 {
        return Err("attention invalid dims".to_string());
    }
    let stride = kv_heads
        .checked_mul(head_dim)
        .ok_or_else(|| "attention stride overflow".to_string())?;
    if cache.capacity == 0 || cache.stride != stride || cache.k.len() != cache.v.len() {
        return Err("attention cache uninitialized".to_string());
    }
    let token_count = cache.len;
    if token_count == 0 {
        return Err("attention cache empty".to_string());
    }
    let window = window.max(1).min(token_count);
    let window_start = (cache.start + token_count.saturating_sub(window)) % cache.capacity;

    let q_len = head_index
        .checked_add(1)
        .ok_or_else(|| "attention head index overflow".to_string())?
        .checked_mul(head_dim)
        .ok_or_else(|| "attention head dim overflow".to_string())?;
    if q.len() < q_len {
        return Err("attention q shape mismatch".to_string());
    }
    let sink_len = head_index + 1;
    if sinks.len() < sink_len {
        return Err("attention sinks shape mismatch".to_string());
    }

    let device = &gpu.device;
    let queue = &gpu.queue;
    let q_bytes = q.len() * std::mem::size_of::<f32>();
    let k_bytes = cache
        .capacity
        .checked_mul(stride)
        .ok_or_else(|| "attention cache overflow".to_string())?
        * std::mem::size_of::<f32>();
    let sink_bytes = sinks.len() * std::mem::size_of::<f32>();
    let out_bytes = window * std::mem::size_of::<f32>();
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    ensure_storage_limit("attn weights q", q_bytes, max_storage)?;
    ensure_storage_limit("attn weights k", k_bytes, max_storage)?;
    ensure_storage_limit("attn weights sinks", sink_bytes, max_storage)?;
    ensure_storage_limit("attn weights out", out_bytes, max_storage)?;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("attn_weights"),
        source: wgpu::ShaderSource::Wgsl(ATTENTION_WEIGHTS_SHADER.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("attn_weights_bindings"),
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
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
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
        label: Some("attn_weights_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("attn_weights_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let q_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_weights_q"),
        contents: cast_slice(q),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let k_buffer = cache
        .gpu_k
        .as_ref()
        .ok_or_else(|| "attention gpu cache missing".to_string())?
        .clone();
    let sink_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_weights_sinks"),
        contents: cast_slice(sinks),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("attn_weights_out"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = AttentionWeightsParams {
        head_index: u32::try_from(head_index)
            .map_err(|_| "attention head index overflow".to_string())?,
        kv_heads: u32::try_from(kv_heads).map_err(|_| "attention kv_heads overflow".to_string())?,
        head_dim: u32::try_from(head_dim).map_err(|_| "attention head_dim overflow".to_string())?,
        seq_len: u32::try_from(window).map_err(|_| "attention seq_len overflow".to_string())?,
        window_start: u32::try_from(window_start)
            .map_err(|_| "attention window_start overflow".to_string())?,
        capacity: u32::try_from(cache.capacity)
            .map_err(|_| "attention capacity overflow".to_string())?,
        _pad0: 0,
        _pad1: 0,
    };
    let params_bytes = std::mem::size_of::<AttentionWeightsParams>();
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("attn_weights_params"),
        contents: cast_slice(&[params]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("attn_weights_bind_group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: q_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: k_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: sink_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: out_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("attn_weights_readback"),
        size: out_bytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    gpu_tracker.add_buffers(q_bytes + sink_bytes + out_bytes * 2 + params_bytes, 5);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("attn_weights_encoder"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("attn_weights_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_bytes as u64);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device.poll(wgpu::Maintain::Wait);
    match rx.await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("attn weights map_async failed: {err:?}")),
        Err(_) => return Err("attn weights map_async channel failed".to_string()),
    }

    let data = slice.get_mapped_range();
    let output = cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    Ok(output)
}

async fn gpu_matmul_q8_0(
    quant: &[u8],
    x: &[f32],
    k: usize,
    n: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let x_bytes = x.len() * std::mem::size_of::<f32>();
    let y_bytes = n * std::mem::size_of::<f32>();
    ensure_storage_limit("q8_0 weights", quant.len(), max_storage)?;
    ensure_storage_limit("q8_0 input", x_bytes, max_storage)?;
    ensure_storage_limit("q8_0 output", y_bytes, max_storage)?;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(Q8_0_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("q8_0_probe"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
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
    let y_bytes = y_bytes as u64;
    let y_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_probe_y"),
        size: y_bytes,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params = [k as u32, n as u32, 0u32, 0u32];
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
    let params_bytes = std::mem::size_of::<u32>() * 4;
    gpu_tracker.add_buffers(
        quant.len()
            + x.len() * std::mem::size_of::<f32>()
            + (y_bytes as usize) * 2
            + params_bytes,
        5,
    );

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

async fn gpu_matmul_q8_0_chunked(
    gguf_url: &GgufSource,
    weight: &GgufTensor,
    input: &[f32],
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
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
    if n % Q8_0_BLOCK_VALUES != 0 {
        return Err("q8_0 n not divisible by block size".to_string());
    }

    let row_bytes = (n / Q8_0_BLOCK_VALUES) * Q8_0_BLOCK_BYTES;
    let max_bytes = gpu.device.limits().max_storage_buffer_binding_size as usize;
    let x_bytes = input.len() * std::mem::size_of::<f32>();
    let y_bytes = n * std::mem::size_of::<f32>();
    if row_bytes > max_bytes {
        return Err(format!(
            "q8_0 row bytes {} exceed max storage {}",
            format_bytes(row_bytes as u64),
            format_bytes(max_bytes as u64)
        ));
    }
    ensure_storage_limit("q8_0 input", x_bytes, max_bytes)?;
    ensure_storage_limit("q8_0 output", y_bytes, max_bytes)?;
    let max_rows = (max_bytes.saturating_sub(3) / row_bytes).max(1);
    let chunk_rows = max_rows;

    let device = &gpu.device;
    let queue = &gpu.queue;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(Q8_0_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("q8_0_chunked"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("q8_0_chunked_bindings"),
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
        label: Some("q8_0_chunked_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("q8_0_chunked_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_chunked_x"),
        contents: cast_slice(input),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    let y_bytes = y_bytes as u64;
    let mut zeroes = vec![0.0f32; n];
    let y_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("q8_0_chunked_y"),
        contents: cast_slice(&zeroes),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
    });
    zeroes.clear();

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("q8_0_chunked_readback"),
        size: y_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params_bytes = std::mem::size_of::<u32>() * 4;
    gpu_tracker.add_buffers(
        input.len() * std::mem::size_of::<f32>()
            + y_bytes as usize
            + y_bytes as usize,
        3,
    );

    let mut row_offset = 0usize;
    while row_offset < k {
        let rows = (k - row_offset).min(chunk_rows);
        let offset = weight
            .absolute_offset
            .saturating_add((row_offset * row_bytes) as u64);
        let len = rows * row_bytes;
        let mut quant = fetch_range_source(gguf_url, offset, len as u64).await?;
        if quant.len() % 4 != 0 {
            let padded = (quant.len() + 3) / 4 * 4;
            quant.resize(padded, 0);
        }
        ensure_storage_limit("q8_0 weights", quant.len(), max_bytes)?;

        let quant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("q8_0_chunked_quant"),
            contents: &quant,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        gpu_tracker.add_buffers(quant.len(), 1);
        let params = [rows as u32, n as u32, row_offset as u32, if row_offset == 0 { 0 } else { 1 }];
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("q8_0_chunked_params"),
            contents: cast_slice(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        gpu_tracker.add_buffers(params_bytes, 1);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("q8_0_chunked_bind_group"),
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

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("q8_0_chunked_encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("q8_0_chunked_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
            pass.dispatch_workgroups(groups, 1, 1);
        }
        queue.submit(Some(encoder.finish()));

        row_offset += rows;
    }

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("q8_0_chunked_readback_encoder"),
    });
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

async fn gpu_matmul_mxfp4(
    quant: &[u8],
    x: &[f32],
    k: usize,
    n: usize,
    gpu: &GpuContext,
    gpu_tracker: &mut GpuAllocTracker,
) -> Result<Vec<f32>, String> {
    let device = &gpu.device;
    let queue = &gpu.queue;
    let max_storage = device.limits().max_storage_buffer_binding_size as usize;
    let x_bytes = x.len() * std::mem::size_of::<f32>();
    let y_bytes = n * std::mem::size_of::<f32>();
    if n % MXFP4_BLOCK_VALUES != 0 {
        return Err("mxfp4 n not divisible by block size".to_string());
    }
    let row_bytes = (n / MXFP4_BLOCK_VALUES) * MXFP4_BLOCK_BYTES;
    let expected_bytes = row_bytes
        .checked_mul(k)
        .ok_or_else(|| "mxfp4 weight byte overflow".to_string())?;
    if quant.len() < expected_bytes {
        return Err("mxfp4 weights truncated".to_string());
    }
    ensure_storage_limit("mxfp4 input", x_bytes, max_storage)?;
    ensure_storage_limit("mxfp4 output", y_bytes, max_storage)?;
    let chunked = expected_bytes > max_storage;

    let workgroup_size = pick_workgroup_size(device);
    let shader_source = shader_with_workgroup(MXFP4_SHADER, workgroup_size);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mxfp4_probe"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mxfp4_probe_bindings"),
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
        label: Some("mxfp4_probe_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("mxfp4_probe_pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });

    let x_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("mxfp4_probe_x"),
        contents: cast_slice(x),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });
    let y_bytes = y_bytes as u64;
    let y_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mxfp4_probe_y"),
        size: y_bytes,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mxfp4_probe_readback"),
        size: y_bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let params_bytes = std::mem::size_of::<u32>() * 4;
    gpu_tracker.add_buffers(
        x.len() * std::mem::size_of::<f32>() + (y_bytes as usize) * 2,
        3,
    );

    let mut row_offset = 0usize;
    while row_offset < k {
        let rows = if chunked {
            let max_rows = (max_storage / row_bytes).max(1);
            (k - row_offset).min(max_rows)
        } else {
            k
        };
        let start = row_offset * row_bytes;
        let end = start + rows * row_bytes;
        let slice = &quant[start..end];
        let mut quant_chunk = slice.to_vec();
        if quant_chunk.len() % 4 != 0 {
            let padded = (quant_chunk.len() + 3) / 4 * 4;
            quant_chunk.resize(padded, 0);
        }
        if chunked {
            ensure_storage_limit("mxfp4 weights", quant_chunk.len(), max_storage)?;
        } else {
            ensure_storage_limit("mxfp4 weights", quant_chunk.len(), max_storage)?;
        }

        let quant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mxfp4_probe_quant"),
            contents: &quant_chunk,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        gpu_tracker.add_buffers(quant_chunk.len(), 1);
        let params = [
            rows as u32,
            n as u32,
            row_offset as u32,
            if row_offset == 0 { 0 } else { 1 },
        ];
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mxfp4_probe_params"),
            contents: cast_slice(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        gpu_tracker.add_buffers(params_bytes, 1);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mxfp4_probe_bind_group"),
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

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mxfp4_probe_encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("mxfp4_probe_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            let groups = (n as u32 + workgroup_size - 1) / workgroup_size;
            pass.dispatch_workgroups(groups, 1, 1);
        }
        queue.submit(Some(encoder.finish()));

        if chunked {
            row_offset += rows;
        } else {
            break;
        }
    }

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mxfp4_probe_readback_encoder"),
    });
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

const RMSNORM_SHADER: &str = include_str!("shaders/rmsnorm.wgsl");
const ROPE_SHADER: &str = include_str!("shaders/rope.wgsl");
const ATTENTION_DECODE_SHADER: &str = include_str!("shaders/attention_decode.wgsl");
const Q8_0_DEQUANT_SHADER: &str = include_str!("shaders/q8_0_dequant.wgsl");
const VEC_ADD_SHADER: &str = include_str!("shaders/vec_add.wgsl");
const SWIGLU_SHADER: &str = include_str!("shaders/swiglu.wgsl");
const MATMUL_F32_SHADER: &str = include_str!("shaders/matmul_f32.wgsl");
const ATTENTION_WEIGHTS_SHADER: &str = include_str!("shaders/attention_weights.wgsl");
const SCALE_ADD_SHADER: &str = include_str!("shaders/scale_add.wgsl");

const Q8_0_SHADER: &str = r#"
struct Params {
    k: u32,
    n: u32,
    row_offset: u32,
    accumulate: u32,
};

@group(0) @binding(0)
var<storage, read> quant: array<u32>;

@group(0) @binding(1)
var<storage, read> x: array<f32>;

@group(0) @binding(2)
var<storage, read_write> y: array<f32>;

@group(0) @binding(3)
var<uniform> params: Params;

fn unpack_f16(bits: u16) -> f32 {
    let sign = (bits >> 15u) & 1u;
    let exp = (bits >> 10u) & 0x1fu;
    let frac = bits & 0x03ffu;
    var val: f32;
    if (exp == 0u) {
        if (frac == 0u) {
            val = 0.0;
        } else {
            val = f32(frac) * exp2(-24.0);
        }
    } else if (exp == 31u) {
        val = 1.0 / 0.0;
    } else {
        val = (1.0 + f32(frac) / 1024.0) * exp2(f32(exp) - 15.0);
    }
    if (sign == 1u) {
        val = -val;
    }
    return val;
}

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

@compute @workgroup_size({{WORKGROUP_SIZE}})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    if (col >= params.n) {
        return;
    }
    let mut acc = select(0.0, y[col], params.accumulate != 0u);
    for (var row = 0u; row < params.k; row = row + 1u) {
        let idx = row * params.n + col;
        let block = idx / 32u;
        let offset = idx % 32u;
        let w = q8_0_unpack(block, offset);
        acc = acc + x[params.row_offset + row] * w;
    }
    y[col] = acc;
}
"#;

const MXFP4_SHADER: &str = r#"
struct Params {
    k: u32,
    n: u32,
    row_offset: u32,
    accumulate: u32,
};

@group(0) @binding(0)
var<storage, read> quant: array<u32>;

@group(0) @binding(1)
var<storage, read> x: array<f32>;

@group(0) @binding(2)
var<storage, read_write> y: array<f32>;

@group(0) @binding(3)
var<uniform> params: Params;

const FP4_TABLE: array<f32, 16> = array<f32, 16>(
    0.0, 0.5, 1.0, 1.5,
    2.0, 3.0, 4.0, 6.0,
    -0.0, -0.5, -1.0, -1.5,
    -2.0, -3.0, -4.0, -6.0
);

fn load_byte(offset: u32) -> u32 {
    let word = quant[offset / 4u];
    let shift = (offset & 3u) * 8u;
    return (word >> shift) & 0xffu;
}

fn mxfp4_unpack(block: u32, idx: u32) -> f32 {
    let base = block * 17u;
    let scale_byte = load_byte(base);
    let exp = f32(i32(scale_byte)) - 127.0;
    let scale = exp2(exp);
    let byte_index = base + 1u + (idx / 2u);
    let packed = load_byte(byte_index);
    let nibble = select(packed & 0x0fu, packed >> 4u, (idx & 1u) == 1u);
    return FP4_TABLE[nibble] * scale;
}

@compute @workgroup_size({{WORKGROUP_SIZE}})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    if (col >= params.n) {
        return;
    }
    let mut acc = select(0.0, y[col], params.accumulate != 0u);
    for (var row = 0u; row < params.k; row = row + 1u) {
        let idx = row * params.n + col;
        let block = idx / 32u;
        let offset = idx % 32u;
        let w = mxfp4_unpack(block, offset);
        acc = acc + x[params.row_offset + row] * w;
    }
    y[col] = acc;
}
"#;
