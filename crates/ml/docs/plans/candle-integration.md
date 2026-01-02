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
