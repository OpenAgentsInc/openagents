# Plan: Multi-Backend ML Inference Library (crates/ml)

## Goal
Build a Rust ML inference library with **both browser (WebGPU) AND native (CUDA/Metal)** support:
- **Browser**: WebGPU compute shaders for NIP-90 DVM from a browser tab
- **Native/Pylon**: Full GPU acceleration via Candle's CUDA/Metal backends

## Key Decision: Use Candle, Add WebGPU Backend

**Don't reinvent tensor primitives.** Candle (~/code/candle) provides:
- Battle-tested `Tensor`, `DType`, `Shape`, `Layout` types
- Clean `BackendStorage`/`BackendDevice` traits for extension
- CUDA and Metal backends ready to use
- 181 transformer model implementations
- Quantization (Q4, Q8) built-in
- WASM support with SIMD128 optimization

**What we add:**
- `candle-wgpu` subcrate implementing Candle's traits for WebGPU
- Unified `MlDevice` abstraction across all backends
- `MlProvider` implementing our `InferenceBackend` trait
- NIP-90 DVM integration (BrowserDvmService)

## Crate Structure

```
crates/ml/
├── Cargo.toml              # Feature flags: browser, native, cuda, metal
├── candle-wgpu/            # NEW: WebGPU backend for Candle
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── storage.rs      # WgpuStorage implements BackendStorage
│       ├── device.rs       # WgpuDevice implements BackendDevice
│       ├── ops/            # WGSL kernel dispatchers
│       │   ├── binary.rs
│       │   ├── unary.rs
│       │   ├── reduce.rs
│       │   ├── matmul.rs
│       │   └── attention.rs
│       └── shaders/        # WGSL compute shaders
│           ├── binary.wgsl
│           ├── unary.wgsl
│           ├── reduce.wgsl
│           ├── matmul.wgsl
│           └── attention.wgsl
├── src/
│   ├── lib.rs
│   ├── device.rs           # MlDevice enum (Candle | WebGpu | Cpu)
│   ├── provider.rs         # MlProvider implements InferenceBackend
│   └── browser_dvm.rs      # BrowserDvmService for NIP-90
└── docs/                   # Existing docs (update with Candle info)
```

## Feature Flags

```toml
[features]
default = ["browser"]

# Browser: WebGPU + CPU fallback
browser = ["candle-wgpu", "wasm-bindgen", "web-sys", "js-sys"]

# Native: Candle CUDA/Metal
native = ["candle-core", "candle-nn", "candle-transformers"]
cuda = ["native", "candle-core/cuda"]
metal = ["native", "candle-core/metal"]

[dependencies]
candle-core = { version = "0.8", optional = true }
candle-nn = { version = "0.8", optional = true }
candle-transformers = { version = "0.8", optional = true }
candle-wgpu = { path = "candle-wgpu", optional = true }
```

## Core Abstractions

### MlDevice (unified across backends)

```rust
pub enum MlDevice {
    #[cfg(feature = "native")]
    Candle(candle_core::Device),  // CUDA, Metal, or CPU

    #[cfg(feature = "browser")]
    WebGpu(candle_wgpu::WgpuDevice),

    Cpu,
}

impl MlDevice {
    pub async fn best_available() -> Result<Self, DeviceError> {
        // Native: CUDA > Metal > CPU
        // Browser: WebGPU > CPU
    }
}
```

### WgpuStorage (implements Candle's BackendStorage)

```rust
pub struct WgpuStorage {
    buffer: Arc<wgpu::Buffer>,
    dtype: DType,
    device: Arc<WgpuDevice>,
}

impl BackendStorage for WgpuStorage {
    fn dtype(&self) -> DType;
    fn to_cpu_storage(&self) -> Result<CpuStorage>;
    fn matmul(&self, rhs: &Self, ...) -> Result<Self>;  // Dispatch to WGSL
    fn unary_impl<B: UnaryOpT>(&self, ...) -> Result<Self>;
    fn binary_impl<B: BinaryOpT>(&self, ...) -> Result<Self>;
}
```

### MlProvider (implements InferenceBackend)

```rust
impl InferenceBackend for MlProvider {
    fn id(&self) -> &str { "ml-candle" }
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;
    async fn complete_stream(&self, request: CompletionRequest) -> Result<Receiver<StreamChunk>>;
}
```

## WGSL Compute Kernels

| Kernel | Workgroup | Notes |
|--------|-----------|-------|
| matmul | 16x16x1 | Tiled GEMM with shared memory |
| softmax | 256x1x1 | Parallel reduction |
| attention | 16x16x1 | Fused QKV with causal mask |
| unary (exp, relu, silu, gelu) | 256x1x1 | Element-wise |
| binary (add, mul, sub, div) | 256x1x1 | Broadcasting |
| reduce (sum, max, mean) | 256x1x1 | Tree reduction |

## Integration Points

### Pylon (native runtime)
```rust
// crates/pylon/src/provider.rs
impl BackendRegistry {
    pub async fn detect() -> Self {
        #[cfg(feature = "ml-native")]
        if let Ok(provider) = ml::MlProvider::new().await {
            registry.register("ml-candle", provider);
        }
    }
}
```

### Browser DVM (NIP-90)
```rust
#[wasm_bindgen]
pub struct BrowserDvm {
    provider: MlProvider,
}

#[wasm_bindgen]
impl BrowserDvm {
    pub async fn new() -> Result<Self, JsValue>;
    pub async fn load_model(&mut self, url: &str) -> Result<(), JsValue>;
    pub async fn process_job(&self, prompt: &str) -> Result<String, JsValue>;
}
```

## Critical Files

| File | Purpose |
|------|---------|
| `~/code/candle/candle-core/src/backend.rs` | BackendStorage/BackendDevice traits to implement |
| `~/code/candle/candle-core/src/storage.rs` | Storage dispatch pattern to follow |
| `crates/compute/src/backends/mod.rs` | InferenceBackend trait to implement |
| `crates/pylon/src/provider.rs` | Backend registration for native |
| `crates/wgpui/src/platform.rs` | wgpu device init pattern |

## Implementation Phases

### Phase 1: candle-wgpu skeleton (2-3 days)
- [ ] Create `crates/ml/candle-wgpu/` subcrate
- [ ] Implement `WgpuDevice` with pipeline cache
- [ ] Implement `WgpuStorage` basic structure
- [ ] Add feature flags to `crates/ml/Cargo.toml`

### Phase 2: Core WGSL shaders (4-5 days)
- [ ] `matmul.wgsl` - tiled GEMM
- [ ] `unary.wgsl` - exp, log, relu, silu, gelu, tanh
- [ ] `binary.wgsl` - add, mul, sub, div with broadcasting
- [ ] `reduce.wgsl` - sum, max, mean

### Phase 3: BackendStorage implementation (3-4 days)
- [ ] Implement all `BackendStorage` trait methods
- [ ] Handle layout/strides for non-contiguous tensors
- [ ] CPU fallback for unsupported ops

### Phase 4: Attention & LLM (4-5 days)
- [ ] `attention.wgsl` - fused scaled dot-product
- [ ] KV cache management
- [ ] Safetensors HTTP range loader

### Phase 5: MlDevice unification (2 days)
- [ ] `MlDevice` enum wrapping Candle + WebGPU
- [ ] `best_available()` device selection
- [ ] Feature-gated compilation

### Phase 6: Provider integration (3-4 days)
- [ ] `MlProvider` implementing `InferenceBackend`
- [ ] Pylon backend registration
- [ ] Model loading (safetensors)

### Phase 7: Browser DVM (2-3 days)
- [ ] `BrowserDvmService` with wasm-bindgen exports
- [ ] NIP-90 job handling (kind 5050 → 6050)
- [ ] NIP-89 handler advertisement

### Phase 8: Testing (2-3 days)
- [ ] Unit tests for WGSL kernels
- [ ] Integration tests with small model
- [ ] Browser testing (Chrome, Firefox, Safari)

**Total: ~22-29 days**

## Browser Constraints

- Max workgroup size: 256 total invocations
- Max storage buffer: 128 MB
- No tokio - use `wasm_bindgen_futures::spawn_local`
- Use `web_time::Instant` not `std::time::Instant`
- Use `pollster::block_on` for sync in WASM

---

## Target Models

### Browser-Viable Models (128MB WebGPU limit)

| Model | Params | Q4 Size | Use Case | Candle WASM Example |
|-------|--------|---------|----------|---------------------|
| **Llama2-C-42M** | 42M | 27MB | Text generation | ✅ `llama2-c/` |
| **Whisper-Tiny** | 39M | 25MB | Speech-to-text | ✅ `whisper/` |
| **T5-Small** | 60M | 38MB | Translation/summarization | ✅ `t5/` |
| **MiniLM-L6-v2** | 22M | 14MB | Embeddings | ✅ `bert/` |
| **Phi-1.5** | 1.3B | 650MB | ⚠️ Too large | `phi/` |

**Primary browser target**: Llama2-C-42M (Q4 quantized, 27MB download, ~70MB runtime)

### Native/Pylon Models (full GPU)

| Model | Params | Q4 Size | Notes |
|-------|--------|---------|-------|
| **Gemma 3 1B** | 1B | ~500MB | Smallest Gemma, needs 4GB+ VRAM |
| **Gemma 3 4B** | 4B | ~2GB | Good quality/size balance |
| **Llama 3.2 1B** | 1B | ~500MB | Meta's efficient small model |
| **Phi-3 Mini** | 3.8B | ~2GB | Microsoft's efficient model |

**Primary native target**: Gemma 3 1B or Llama 3.2 1B with Q4 quantization

---

## Gemma 3 Architecture (for native/Pylon)

Based on [Gemma 3 Technical Report](https://arxiv.org/abs/2503.19786):

### Key Architecture Details
- **Attention**: Grouped-Query Attention (GQA) with QK-norm (replaces soft-capping)
- **Layer Pattern**: 5:1 local/global attention interleaving
  - Local: 1024-token sliding window, RoPE base 10k
  - Global: Full context, RoPE base 1M
- **Normalization**: RMSNorm (6 per attention block)
- **Activation**: GELU (pytorch_tanh variant)
- **Position**: RoPE with variable base frequency

### Gemma 3 1B Config
```
vocab_size: 262208
hidden_size: 2304
intermediate_size: 9216
num_hidden_layers: 26
num_attention_heads: 8
num_key_value_heads: 4
head_dim: 256
rms_norm_eps: 1e-06
max_position_embeddings: 131072
```

---

## Complete Ops List for Transformer Inference

### Required WGSL Kernels

| Op | Kernel | Gemma 3 | Llama2-C | Notes |
|----|--------|---------|----------|-------|
| **Embedding Lookup** | `embedding.wgsl` | ✅ | ✅ | Index into vocab embeddings |
| **RMSNorm** | `rmsnorm.wgsl` | ✅ | ✅ | Pre/post attention norm |
| **LayerNorm** | `layernorm.wgsl` | - | - | For BERT/T5 |
| **MatMul (GEMM)** | `matmul.wgsl` | ✅ | ✅ | Tiled 16x16 |
| **Linear + Bias** | `linear.wgsl` | ✅ | ✅ | W @ x + b |
| **Quantized MatMul** | `qmatmul.wgsl` | ✅ | ✅ | Q4/Q8 dequant on-the-fly |
| **RoPE** | `rope.wgsl` | ✅ | ✅ | Rotary position embeddings |
| **Attention (GQA)** | `attention_gqa.wgsl` | ✅ | ✅ | Grouped-query with causal mask |
| **Attention (Sliding)** | `attention_local.wgsl` | ✅ | - | 1024-token window |
| **QK-Norm** | `qknorm.wgsl` | ✅ | - | Gemma 3 specific |
| **SiLU** | `silu.wgsl` | - | ✅ | x * sigmoid(x) |
| **GELU** | `gelu.wgsl` | ✅ | - | Gemma uses gelu_tanh |
| **Softmax** | `softmax.wgsl` | ✅ | ✅ | Numerically stable |
| **Add** | `binary.wgsl` | ✅ | ✅ | Residual connections |
| **Mul** | `binary.wgsl` | ✅ | ✅ | Scaling, gating |
| **Reduce (sum/max)** | `reduce.wgsl` | ✅ | ✅ | For softmax, norms |
| **Argmax** | `argmax.wgsl` | ✅ | ✅ | Greedy sampling |
| **TopK** | `topk.wgsl` | ✅ | ✅ | For top-p sampling |

### KV Cache Management
```rust
pub struct KvCache {
    /// [num_layers, max_seq, num_kv_heads, head_dim]
    keys: Vec<Tensor>,
    values: Vec<Tensor>,
    seq_len: usize,
}

impl KvCache {
    fn append(&mut self, layer: usize, k: &Tensor, v: &Tensor);
    fn get(&self, layer: usize, start: usize, end: usize) -> (&Tensor, &Tensor);
    fn clear(&mut self);
}
```

---

## Tokenizer Integration

### Tokenizer Crate (WASM-compatible)
```toml
tokenizers = { version = "0.20", features = ["unstable_wasm"] }
```

### Tokenizer API
```rust
pub struct Tokenizer {
    inner: tokenizers::Tokenizer,
}

impl Tokenizer {
    /// Load from HuggingFace JSON
    pub fn from_bytes(json: &[u8]) -> Result<Self>;

    /// Load from URL (browser)
    pub async fn from_url(url: &str) -> Result<Self>;

    /// Encode text to tokens
    pub fn encode(&self, text: &str, add_special: bool) -> Result<Vec<u32>>;

    /// Decode tokens to text
    pub fn decode(&self, tokens: &[u32], skip_special: bool) -> Result<String>;

    /// Get special token IDs
    pub fn bos_token_id(&self) -> Option<u32>;
    pub fn eos_token_id(&self) -> Option<u32>;
    pub fn pad_token_id(&self) -> Option<u32>;
}
```

### Model-Specific Tokenizers

| Model | Tokenizer Type | Vocab Size | Files |
|-------|---------------|------------|-------|
| Gemma 3 | SentencePiece | 262,208 | `tokenizer.model` |
| Llama 2/3 | SentencePiece | 32,000 | `tokenizer.model` |
| Phi | GPT-2 BPE | 51,200 | `vocab.json`, `merges.txt` |
| Whisper | GPT-2 BPE | 50,257 | `vocab.json`, `merges.txt` |

---

## Generation Loop & Sampling

### Text Generation API
```rust
pub struct GenerationConfig {
    pub max_new_tokens: usize,
    pub temperature: f32,          // 0.0 = greedy, 1.0 = default
    pub top_p: f32,                // Nucleus sampling threshold
    pub top_k: usize,              // Top-K sampling
    pub repetition_penalty: f32,   // Penalize repeated tokens
    pub stop_tokens: Vec<u32>,     // EOS tokens
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_new_tokens: 256,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 50,
            repetition_penalty: 1.1,
            stop_tokens: vec![],
        }
    }
}
```

### Generation Loop
```rust
pub async fn generate(
    model: &Model,
    tokenizer: &Tokenizer,
    prompt: &str,
    config: GenerationConfig,
    on_token: impl Fn(String),  // Streaming callback
) -> Result<String> {
    let mut tokens = tokenizer.encode(prompt, true)?;
    let mut kv_cache = model.create_kv_cache();

    // Prefill: process prompt
    let mut logits = model.prefill(&tokens, &mut kv_cache).await?;

    // Decode: generate tokens one at a time
    for _ in 0..config.max_new_tokens {
        // Sample next token
        let next_token = sample(&logits, &config, &tokens)?;

        // Check for stop
        if config.stop_tokens.contains(&next_token) {
            break;
        }

        tokens.push(next_token);

        // Stream output
        let text = tokenizer.decode(&[next_token], true)?;
        on_token(text);

        // Forward one token
        logits = model.decode_step(next_token, &mut kv_cache).await?;
    }

    tokenizer.decode(&tokens, true)
}
```

### Sampling Strategies
```rust
fn sample(logits: &Tensor, config: &GenerationConfig, prev_tokens: &[u32]) -> Result<u32> {
    let mut logits = logits.to_vec::<f32>()?;

    // Apply repetition penalty
    for &token in prev_tokens {
        logits[token as usize] /= config.repetition_penalty;
    }

    // Temperature scaling
    if config.temperature > 0.0 {
        for l in &mut logits {
            *l /= config.temperature;
        }
    }

    // Softmax
    let probs = softmax(&logits);

    if config.temperature == 0.0 {
        // Greedy
        return Ok(argmax(&probs));
    }

    // Top-K filtering
    let mut indices: Vec<usize> = (0..probs.len()).collect();
    indices.sort_by(|&a, &b| probs[b].partial_cmp(&probs[a]).unwrap());
    indices.truncate(config.top_k);

    // Top-P (nucleus) filtering
    let mut cumsum = 0.0;
    let mut cutoff = indices.len();
    for (i, &idx) in indices.iter().enumerate() {
        cumsum += probs[idx];
        if cumsum >= config.top_p {
            cutoff = i + 1;
            break;
        }
    }
    indices.truncate(cutoff);

    // Renormalize and sample
    let filtered_probs: Vec<f32> = indices.iter().map(|&i| probs[i]).collect();
    let sum: f32 = filtered_probs.iter().sum();
    let normalized: Vec<f32> = filtered_probs.iter().map(|p| p / sum).collect();

    let r: f32 = random();
    let mut cumsum = 0.0;
    for (i, &p) in normalized.iter().enumerate() {
        cumsum += p;
        if cumsum >= r {
            return Ok(indices[i] as u32);
        }
    }

    Ok(indices[0] as u32)
}
```

---

## Test Plan

### Unit Tests (per WGSL kernel)
```rust
#[test]
fn test_matmul_small() {
    // 4x4 @ 4x4 = 4x4
    let a = Tensor::from_vec(vec![...], (4, 4), DType::F32);
    let b = Tensor::from_vec(vec![...], (4, 4), DType::F32);
    let c = matmul(&a, &b);
    assert_eq!(c.to_vec(), expected);
}

#[test]
fn test_rmsnorm() {
    let x = Tensor::from_vec(vec![1.0, 2.0, 3.0, 4.0], (1, 4), DType::F32);
    let weight = Tensor::ones((4,), DType::F32);
    let out = rmsnorm(&x, &weight, 1e-6);
    // Compare to PyTorch reference
}

#[test]
fn test_rope() {
    // Compare to Hugging Face transformers RoPE implementation
}
```

### Integration Tests (small model)
```rust
#[test]
fn test_llama2c_42m_inference() {
    let model = Model::load("models/llama2c-42m-q4.gguf")?;
    let tokenizer = Tokenizer::from_file("models/tokenizer.json")?;

    let output = generate(&model, &tokenizer, "Hello", GenerationConfig::default())?;
    assert!(!output.is_empty());
}
```

### Golden Output Tests
```
tests/
├── golden/
│   ├── llama2c_42m_hello.txt      # Expected output for "Hello"
│   ├── llama2c_42m_story.txt      # Expected output for "Once upon a time"
│   └── whisper_tiny_audio.txt     # Expected transcription
└── models/
    ├── llama2c-42m-q4.gguf        # Quantized model
    └── tokenizer.json              # Tokenizer config
```

### Browser Tests (wasm-pack)
```bash
wasm-pack test --chrome --headless
```

```rust
#[wasm_bindgen_test]
async fn test_browser_inference() {
    let model = Model::from_url("https://example.com/model.gguf").await?;
    let output = model.generate("Hello").await?;
    assert!(!output.is_empty());
}
```

---

## Sources

- [Gemma 3 Technical Report](https://arxiv.org/abs/2503.19786)
- [Gemma 3 HuggingFace Docs](https://huggingface.co/docs/transformers/en/model_doc/gemma3)
- [Candle WASM Examples](https://github.com/huggingface/candle/tree/main/candle-wasm-examples)
