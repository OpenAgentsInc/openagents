# Runtime Integration

How the ml crate integrates with `crates/runtime` via the ComputeProvider trait.

## Overview

The runtime crate defines a `ComputeProvider` trait for inference backends. The ml crate implements this trait as `WebGpuProvider`, enabling browser-based inference jobs.

```
┌─────────────────────────────────────────────────────────────┐
│                        Runtime                              │
│                                                             │
│  ┌─────────────┐    ┌──────────────┐    ┌───────────────┐  │
│  │   Agent     │───▶│ ComputeMount │───▶│ComputeProvider│  │
│  └─────────────┘    └──────────────┘    └───────────────┘  │
│                                                │            │
└────────────────────────────────────────────────┼────────────┘
                                                 │
                                    ┌────────────┼────────────┐
                                    │            ▼            │
                                    │    ┌──────────────┐     │
                                    │    │WebGpuProvider│     │
                                    │    └──────────────┘     │
                                    │            │            │
                                    │            ▼            │
                                    │    ┌──────────────┐     │
                                    │    │ WebGpuDevice │     │
                                    │    └──────────────┘     │
                                    │            │            │
                                    │            ▼            │
                                    │    ┌──────────────┐     │
                                    │    │ WGSL Shaders │     │
                                    │    └──────────────┘     │
                                    │                         │
                                    │      crates/ml          │
                                    └─────────────────────────┘
```

## ComputeProvider Trait

From `crates/runtime/src/compute.rs`:

```rust
/// Provider for compute jobs (inference, embeddings, etc.)
pub trait ComputeProvider: Send + Sync {
    /// Unique identifier for this provider
    fn id(&self) -> &str;

    /// Provider metadata
    fn info(&self) -> ProviderInfo;

    /// Check if provider supports a model
    fn supports_model(&self, model: &str) -> bool;

    /// Submit a compute request
    /// CRITICAL: Must return immediately, never block
    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError>;

    /// Get current state of a job
    fn get_job(&self, job_id: &str) -> Option<JobState>;

    /// Poll for streaming output chunk
    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError>;

    /// Cancel a running job
    fn cancel(&self, job_id: &str) -> Result<(), ComputeError>;
}
```

### Key Types

```rust
/// Information about a compute provider
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub supported_models: Vec<String>,
    pub max_context_length: usize,
    pub supports_streaming: bool,
}

/// Request for compute work
pub struct ComputeRequest {
    pub model: String,
    pub prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
    pub stream: bool,
}

/// State of a compute job
pub enum JobState {
    Pending,
    Running { started_at: u64 },
    Completed { result: ComputeResult },
    Failed { error: String },
    Cancelled,
}

/// Output chunk from streaming inference
pub struct ComputeChunk {
    pub text: String,
    pub is_final: bool,
}

/// Final compute result
pub struct ComputeResult {
    pub text: String,
    pub tokens_generated: usize,
    pub time_ms: u64,
}
```

## WebGpuProvider Implementation

```rust
// crates/ml/src/provider/webgpu_provider.rs

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub struct WebGpuProvider {
    /// WebGPU device
    device: Arc<WebGpuDevice>,
    /// Loaded model
    model: Option<Arc<LoadedModel>>,
    /// Active jobs
    jobs: RwLock<HashMap<String, JobEntry>>,
    /// Output channels for streaming
    channels: RwLock<HashMap<String, OutputChannel>>,
    /// Provider configuration
    config: ProviderConfig,
}

struct JobEntry {
    state: JobState,
    request: ComputeRequest,
    created_at: web_time::Instant,
}

struct OutputChannel {
    chunks: Vec<ComputeChunk>,
    read_index: usize,
}

pub struct ProviderConfig {
    pub max_batch_size: usize,
    pub max_context_length: usize,
    pub default_temperature: f32,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1,
            max_context_length: 4096,
            default_temperature: 0.7,
        }
    }
}

impl WebGpuProvider {
    /// Create new provider (async initialization)
    pub async fn new(config: ProviderConfig) -> Result<Self, MlError> {
        let device = Arc::new(WebGpuDevice::new().await?);

        Ok(Self {
            device,
            model: None,
            jobs: RwLock::new(HashMap::new()),
            channels: RwLock::new(HashMap::new()),
            config,
        })
    }

    /// Load a model from URL
    pub async fn load_model(&mut self, url: &str, model_config: ModelConfig) -> Result<(), MlError> {
        let model = LoadedModel::from_url(&self.device, url, model_config).await?;
        self.model = Some(Arc::new(model));
        Ok(())
    }

    /// Generate unique job ID
    fn generate_job_id() -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        web_time::Instant::now().hash(&mut hasher);
        format!("job_{:016x}", hasher.finish())
    }

    /// Spawn inference task (browser)
    #[cfg(target_arch = "wasm32")]
    fn spawn_inference(&self, job_id: String, request: ComputeRequest) {
        let device = Arc::clone(&self.device);
        let model = self.model.as_ref().map(Arc::clone);
        let jobs = self.jobs.clone();
        let channels = self.channels.clone();
        let config = self.config.clone();

        wasm_bindgen_futures::spawn_local(async move {
            Self::run_inference(device, model, jobs, channels, job_id, request, config).await;
        });
    }

    /// Spawn inference task (native)
    #[cfg(not(target_arch = "wasm32"))]
    fn spawn_inference(&self, job_id: String, request: ComputeRequest) {
        let device = Arc::clone(&self.device);
        let model = self.model.as_ref().map(Arc::clone);
        let jobs = self.jobs.clone();
        let channels = self.channels.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            Self::run_inference(device, model, jobs, channels, job_id, request, config).await;
        });
    }

    /// Core inference loop
    async fn run_inference(
        device: Arc<WebGpuDevice>,
        model: Option<Arc<LoadedModel>>,
        jobs: RwLock<HashMap<String, JobEntry>>,
        channels: RwLock<HashMap<String, OutputChannel>>,
        job_id: String,
        request: ComputeRequest,
        config: ProviderConfig,
    ) {
        // Update state to running
        {
            let mut jobs = jobs.write();
            if let Some(entry) = jobs.get_mut(&job_id) {
                entry.state = JobState::Running {
                    started_at: web_time::Instant::now().elapsed().as_millis() as u64,
                };
            }
        }

        // Check model loaded
        let model = match model {
            Some(m) => m,
            None => {
                jobs.write().get_mut(&job_id).map(|e| {
                    e.state = JobState::Failed {
                        error: "No model loaded".to_string(),
                    };
                });
                return;
            }
        };

        let start = web_time::Instant::now();

        // Tokenize input
        let input_tokens = match model.tokenizer.encode(&request.prompt) {
            Ok(t) => t,
            Err(e) => {
                jobs.write().get_mut(&job_id).map(|entry| {
                    entry.state = JobState::Failed {
                        error: format!("Tokenization failed: {}", e),
                    };
                });
                return;
            }
        };

        // Generation parameters
        let max_tokens = request.max_tokens.unwrap_or(256);
        let temperature = request.temperature.unwrap_or(config.default_temperature);

        // Initialize KV cache
        let mut kv_cache = model.create_kv_cache(input_tokens.len());

        // Prefill: process all input tokens
        let mut logits = match model.prefill(&device, &input_tokens, &mut kv_cache).await {
            Ok(l) => l,
            Err(e) => {
                jobs.write().get_mut(&job_id).map(|entry| {
                    entry.state = JobState::Failed {
                        error: format!("Prefill failed: {}", e),
                    };
                });
                return;
            }
        };

        // Decode: generate tokens one at a time
        let mut output_tokens = Vec::with_capacity(max_tokens);
        let mut output_text = String::new();

        for _ in 0..max_tokens {
            // Sample next token
            let next_token = sample_token(&logits, temperature);

            // Check for EOS
            if next_token == model.tokenizer.eos_token_id() {
                break;
            }

            output_tokens.push(next_token);

            // Decode token to text
            let token_text = model.tokenizer.decode(&[next_token]).unwrap_or_default();
            output_text.push_str(&token_text);

            // Stream output if requested
            if request.stream {
                channels.write().entry(job_id.clone()).or_insert_with(|| {
                    OutputChannel { chunks: Vec::new(), read_index: 0 }
                }).chunks.push(ComputeChunk {
                    text: token_text,
                    is_final: false,
                });
            }

            // Generate next logits
            logits = match model.decode_step(&device, next_token, &mut kv_cache).await {
                Ok(l) => l,
                Err(e) => {
                    jobs.write().get_mut(&job_id).map(|entry| {
                        entry.state = JobState::Failed {
                            error: format!("Decode failed: {}", e),
                        };
                    });
                    return;
                }
            };
        }

        // Send final chunk
        if request.stream {
            channels.write().entry(job_id.clone()).or_insert_with(|| {
                OutputChannel { chunks: Vec::new(), read_index: 0 }
            }).chunks.push(ComputeChunk {
                text: String::new(),
                is_final: true,
            });
        }

        // Update state to completed
        let elapsed = start.elapsed();
        jobs.write().get_mut(&job_id).map(|entry| {
            entry.state = JobState::Completed {
                result: ComputeResult {
                    text: output_text,
                    tokens_generated: output_tokens.len(),
                    time_ms: elapsed.as_millis() as u64,
                },
            };
        });
    }
}

impl ComputeProvider for WebGpuProvider {
    fn id(&self) -> &str {
        "webgpu"
    }

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "webgpu".to_string(),
            name: "WebGPU Local Inference".to_string(),
            supported_models: self.model.as_ref()
                .map(|m| vec![m.config.model_id.clone()])
                .unwrap_or_default(),
            max_context_length: self.config.max_context_length,
            supports_streaming: true,
        }
    }

    fn supports_model(&self, model: &str) -> bool {
        self.model.as_ref()
            .map(|m| m.config.model_id == model)
            .unwrap_or(false)
    }

    /// Submit compute request
    /// CRITICAL: Returns immediately, spawns async work
    fn submit(&self, request: ComputeRequest) -> Result<String, ComputeError> {
        // Check model loaded
        if self.model.is_none() {
            return Err(ComputeError::NoModel);
        }

        // Check model support
        if !self.supports_model(&request.model) {
            return Err(ComputeError::UnsupportedModel(request.model.clone()));
        }

        // Generate job ID
        let job_id = Self::generate_job_id();

        // Store job entry
        self.jobs.write().insert(job_id.clone(), JobEntry {
            state: JobState::Pending,
            request: request.clone(),
            created_at: web_time::Instant::now(),
        });

        // Spawn inference task - NEVER BLOCK
        self.spawn_inference(job_id.clone(), request);

        Ok(job_id)
    }

    fn get_job(&self, job_id: &str) -> Option<JobState> {
        self.jobs.read().get(job_id).map(|e| e.state.clone())
    }

    fn poll_stream(&self, job_id: &str) -> Result<Option<ComputeChunk>, ComputeError> {
        let mut channels = self.channels.write();

        if let Some(channel) = channels.get_mut(job_id) {
            if channel.read_index < channel.chunks.len() {
                let chunk = channel.chunks[channel.read_index].clone();
                channel.read_index += 1;
                return Ok(Some(chunk));
            }
        }

        Ok(None)
    }

    fn cancel(&self, job_id: &str) -> Result<(), ComputeError> {
        let mut jobs = self.jobs.write();

        if let Some(entry) = jobs.get_mut(job_id) {
            match &entry.state {
                JobState::Pending | JobState::Running { .. } => {
                    entry.state = JobState::Cancelled;
                    Ok(())
                }
                _ => Err(ComputeError::JobNotRunning),
            }
        } else {
            Err(ComputeError::JobNotFound)
        }
    }
}
```

## LoadedModel

Model wrapper with inference methods:

```rust
// crates/ml/src/provider/model.rs

pub struct LoadedModel {
    pub config: ModelConfig,
    pub tokenizer: Tokenizer,
    weights: WeightManager,
    layers: Vec<TransformerLayer>,
}

pub struct ModelConfig {
    pub model_id: String,
    pub hidden_size: usize,
    pub num_heads: usize,
    pub num_layers: usize,
    pub vocab_size: usize,
    pub max_seq_len: usize,
    pub rope_base: f32,
}

impl LoadedModel {
    /// Load model from URL
    pub async fn from_url(
        device: &WebGpuDevice,
        url: &str,
        config: ModelConfig,
    ) -> Result<Self, MlError> {
        let mut loader = SafetensorsLoader::new(url);

        // Load metadata first
        loader.load_metadata().await?;

        // Initialize weight manager with LRU cache
        let weights = WeightManager::new(512);  // 512 MB max

        // Load embedding and output weights (always needed)
        let embed_weight = loader.load_tensor(device, "model.embed_tokens.weight").await?;
        weights.insert("embed_tokens".to_string(), embed_weight);

        // Defer loading layer weights until needed (lazy loading)
        // ...

        Ok(Self {
            config,
            tokenizer: Tokenizer::from_url(&format!("{}/tokenizer.json", url)).await?,
            weights,
            layers: Vec::new(),  // Populated lazily
        })
    }

    /// Prefill: process all input tokens
    pub async fn prefill(
        &self,
        device: &WebGpuDevice,
        tokens: &[u32],
        kv_cache: &mut KvCache,
    ) -> Result<Tensor, MlError> {
        let seq_len = tokens.len();

        // Embed tokens
        let mut hidden = self.embed_tokens(device, tokens)?;

        // Apply each transformer layer
        for (layer_idx, layer) in self.layers.iter().enumerate() {
            hidden = layer.forward(device, &hidden, kv_cache, layer_idx, 0..seq_len).await?;
        }

        // Apply final norm and output projection
        let logits = self.output_projection(device, &hidden)?;

        // Return logits for last position only
        Ok(logits.slice(-1))
    }

    /// Decode: generate one token
    pub async fn decode_step(
        &self,
        device: &WebGpuDevice,
        token: u32,
        kv_cache: &mut KvCache,
    ) -> Result<Tensor, MlError> {
        let pos = kv_cache.seq_len;

        // Embed single token
        let mut hidden = self.embed_token(device, token)?;

        // Apply each transformer layer (using cached K, V)
        for (layer_idx, layer) in self.layers.iter().enumerate() {
            hidden = layer.forward(device, &hidden, kv_cache, layer_idx, pos..pos+1).await?;
        }

        // Update cache position
        kv_cache.seq_len += 1;

        // Apply final norm and output projection
        self.output_projection(device, &hidden)
    }

    pub fn create_kv_cache(&self, prefill_len: usize) -> KvCache {
        KvCache::new(
            self.config.num_layers,
            self.config.max_seq_len,
            self.config.num_heads,
            self.config.hidden_size / self.config.num_heads,
            prefill_len,
        )
    }
}
```

## KV Cache

```rust
// crates/ml/src/llm/kv_cache.rs

pub struct KvCache {
    /// Key cache: [num_layers, max_seq, num_heads, head_dim]
    pub keys: Vec<Tensor>,
    /// Value cache: [num_layers, max_seq, num_heads, head_dim]
    pub values: Vec<Tensor>,
    /// Current sequence length
    pub seq_len: usize,
    /// Maximum sequence length
    pub max_len: usize,
}

impl KvCache {
    pub fn new(
        num_layers: usize,
        max_seq_len: usize,
        num_heads: usize,
        head_dim: usize,
    ) -> Self {
        Self {
            keys: Vec::with_capacity(num_layers),
            values: Vec::with_capacity(num_layers),
            seq_len: 0,
            max_len: max_seq_len,
        }
    }

    /// Append new K, V to cache for a layer
    pub fn append(&mut self, device: &WebGpuDevice, layer: usize, k: &Tensor, v: &Tensor) {
        // In practice, this updates a GPU buffer region
        // For simplicity, shown as tensor append
        if self.keys.len() <= layer {
            self.keys.resize_with(layer + 1, || Tensor::empty());
            self.values.resize_with(layer + 1, || Tensor::empty());
        }

        // Append to existing cache
        // Real implementation uses buffer writes to avoid copies
    }

    /// Get cached K, V for a layer up to current position
    pub fn get(&self, layer: usize) -> Option<(&Tensor, &Tensor)> {
        if layer < self.keys.len() {
            Some((&self.keys[layer], &self.values[layer]))
        } else {
            None
        }
    }
}
```

## Sampling

```rust
// crates/ml/src/llm/sampling.rs

/// Sample next token from logits
pub fn sample_token(logits: &Tensor, temperature: f32) -> u32 {
    // Read logits to CPU (last position only)
    let logits_cpu: Vec<f32> = logits.to_vec_sync();

    if temperature < 1e-6 {
        // Greedy: argmax
        argmax(&logits_cpu)
    } else {
        // Sample with temperature
        let scaled: Vec<f32> = logits_cpu.iter().map(|x| x / temperature).collect();
        let probs = softmax_cpu(&scaled);
        sample_from_probs(&probs)
    }
}

/// Sample with top-p (nucleus) sampling
pub fn sample_top_p(logits: &Tensor, temperature: f32, top_p: f32) -> u32 {
    let logits_cpu: Vec<f32> = logits.to_vec_sync();
    let scaled: Vec<f32> = logits_cpu.iter().map(|x| x / temperature).collect();
    let probs = softmax_cpu(&scaled);

    // Sort by probability
    let mut indexed: Vec<(usize, f32)> = probs.iter().cloned().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Find cutoff
    let mut cumsum = 0.0;
    let mut cutoff_idx = indexed.len();
    for (i, (_, p)) in indexed.iter().enumerate() {
        cumsum += p;
        if cumsum >= top_p {
            cutoff_idx = i + 1;
            break;
        }
    }

    // Renormalize and sample
    let truncated: Vec<(usize, f32)> = indexed[..cutoff_idx].to_vec();
    let sum: f32 = truncated.iter().map(|(_, p)| p).sum();
    let normalized: Vec<(usize, f32)> = truncated.iter().map(|(i, p)| (*i, p / sum)).collect();

    sample_from_indexed_probs(&normalized)
}

fn argmax(v: &[f32]) -> u32 {
    v.iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i as u32)
        .unwrap_or(0)
}

fn softmax_cpu(v: &[f32]) -> Vec<f32> {
    let max = v.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp: Vec<f32> = v.iter().map(|x| (x - max).exp()).collect();
    let sum: f32 = exp.iter().sum();
    exp.iter().map(|x| x / sum).collect()
}

fn sample_from_probs(probs: &[f32]) -> u32 {
    let r: f32 = getrandom::random();
    let mut cumsum = 0.0;
    for (i, p) in probs.iter().enumerate() {
        cumsum += p;
        if cumsum >= r {
            return i as u32;
        }
    }
    (probs.len() - 1) as u32
}
```

## Error Types

```rust
// crates/ml/src/provider/error.rs

#[derive(Debug, thiserror::Error)]
pub enum ComputeError {
    #[error("No model loaded")]
    NoModel,

    #[error("Unsupported model: {0}")]
    UnsupportedModel(String),

    #[error("Job not found")]
    JobNotFound,

    #[error("Job not running")]
    JobNotRunning,

    #[error("Inference error: {0}")]
    Inference(String),
}
```

## Registration with Runtime

```rust
// Example: registering WebGpuProvider with runtime

use runtime::compute::ComputeMount;
use ml::provider::WebGpuProvider;

async fn setup_runtime() -> Result<Runtime, Error> {
    // Create WebGPU provider
    let provider = WebGpuProvider::new(Default::default()).await?;

    // Load model
    provider.load_model(
        "https://huggingface.co/model/resolve/main/model.safetensors",
        ModelConfig {
            model_id: "llama-7b".to_string(),
            hidden_size: 4096,
            num_heads: 32,
            num_layers: 32,
            vocab_size: 32000,
            max_seq_len: 4096,
            rope_base: 10000.0,
        },
    ).await?;

    // Create runtime with compute mount
    let runtime = Runtime::builder()
        .with_mount("/compute", ComputeMount::new(Box::new(provider)))
        .build()?;

    Ok(runtime)
}
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_lifecycle() {
        // Synchronous test without real GPU

        let jobs: RwLock<HashMap<String, JobEntry>> = RwLock::new(HashMap::new());
        let job_id = "test_job".to_string();

        // Insert pending job
        jobs.write().insert(job_id.clone(), JobEntry {
            state: JobState::Pending,
            request: ComputeRequest {
                model: "test".to_string(),
                prompt: "Hello".to_string(),
                max_tokens: Some(10),
                temperature: None,
                stream: false,
            },
            created_at: web_time::Instant::now(),
        });

        // Verify pending
        assert!(matches!(
            jobs.read().get(&job_id).map(|e| &e.state),
            Some(JobState::Pending)
        ));

        // Update to running
        jobs.write().get_mut(&job_id).unwrap().state = JobState::Running { started_at: 0 };

        // Update to completed
        jobs.write().get_mut(&job_id).unwrap().state = JobState::Completed {
            result: ComputeResult {
                text: "World".to_string(),
                tokens_generated: 1,
                time_ms: 100,
            },
        };

        // Verify completed
        assert!(matches!(
            jobs.read().get(&job_id).map(|e| &e.state),
            Some(JobState::Completed { .. })
        ));
    }
}
```
