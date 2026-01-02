# ML Crate Architecture (Candle Integration)

This crate provides Candle-based inference for native (CUDA/Metal/CPU) and browser (WASM + WebGPU context, CPU execution) targets.

## Crate Structure

```
crates/ml/
├── Cargo.toml
├── candle-wgpu/           # Candle backend shim (CPU fallback with WebGPU context)
│   ├── Cargo.toml
│   ├── shaders/           # WGSL kernels (binary/unary/matmul/reduce/attention)
│   └── src/
│       ├── device.rs      # WgpuDevice (BackendDevice)
│       ├── storage.rs     # WgpuStorage (BackendStorage, CPU-backed)
│       └── ops/           # WebGPU dispatch helpers
├── docs/
│   ├── ARCHITECTURE.md        # This file
│   ├── COMPUTE-KERNELS.md     # Future WGSL kernels
│   ├── RUNTIME-INTEGRATION.md # Compute backend integration
│   └── BROWSER-PROVIDER.md    # Browser DVM service
└── src/
    ├── lib.rs
    ├── device.rs          # MlDevice (Candle/WebGPU/CPU)
    ├── error.rs           # MlError
    ├── http.rs            # Fetch helpers (native + wasm)
    ├── model.rs           # ModelSource + LoadedModel
    ├── sampling.rs        # GenerationConfig + sampling
    ├── tokenizer.rs       # Tokenizer wrapper
    ├── provider.rs        # MlProvider (InferenceBackend)
    └── browser_dvm.rs     # BrowserDvmService + wasm bindings
```

## Core Types

### MlDevice

Unified device selection across native and browser targets. CUDA and Metal are preferred when available.

```rust
pub enum MlDevice {
    Candle(candle_core::Device),
    WebGpu(candle_wgpu::WgpuDevice),
    Cpu,
}

impl MlDevice {
    pub async fn best_available() -> Result<Self>;
    pub fn candle_device(&self) -> candle_core::Device;
}
```

### ModelSource / LoadedModel

Model definitions loaded via Candle:

- **Browser**: Llama2-C GGUF via `quantized_llama2_c`
- **Native**: Gemma 3 Safetensors via `gemma3`

```rust
pub struct ModelSource {
    pub id: String,
    pub kind: ModelKind,
    pub weights: Vec<String>,
    pub tokenizer: Option<String>,
    pub config: Option<String>,
}

pub struct LoadedModel {
    pub id: String,
    pub kind: ModelKind,
    pub tokenizer: Tokenizer,
    pub max_seq_len: usize,
    pub vocab_size: usize,
}
```

### Tokenizer

WASM-safe wrapper over `tokenizers` with URL and file loading.

### Sampling + Generation

Generation follows a standard prefill + decode loop, with top-k / top-p / temperature / repetition penalty.

### MlProvider

Implements `compute::backends::InferenceBackend` for native runtime integration.

### BrowserDvmService

WebAssembly DVM service that listens for NIP-90 jobs and publishes results via WebSocket relays.

## Notes

- No `std::time::Instant` in browser code; `web_time::Instant` is used.
- GGUF models are fetched in full; safetensors use mmap on native.
- WebGPU backend accelerates contiguous f32 unary/binary/matmul/reduce/attention ops and falls back to CPU for unsupported layouts or dtypes.
