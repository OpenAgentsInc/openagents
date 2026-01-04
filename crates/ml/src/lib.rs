//! # ml - Candle-powered ML inference
//!
//! Unified ML inference library with Candle backends (CPU/CUDA/Metal) and
//! a browser-first NIP-90 DVM implementation.

mod device;
mod error;
mod http;
#[cfg(not(target_arch = "wasm32"))]
mod gguf;
#[cfg(not(target_arch = "wasm32"))]
mod gptoss_tokenizer;
#[cfg(not(target_arch = "wasm32"))]
mod gptoss_native;
#[cfg(not(target_arch = "wasm32"))]
mod gptoss_engine;
#[cfg(feature = "native")]
mod gptoss_backend;
#[cfg(all(feature = "candle", feature = "wgpu", not(target_arch = "wasm32")))]
mod gguf_gate;
mod model;
mod sampling;
mod telemetry;
mod tokenizer;

#[cfg(feature = "native")]
mod provider;

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
mod browser_dvm;

#[cfg(test)]
mod tests;

pub use device::MlDevice;
pub use error::{MlError, Result};
#[cfg(not(target_arch = "wasm32"))]
pub use gguf::{
    load_gguf_index, load_gguf_model, GgufIndex, GgufMetadata, GgufModel, GgufScalar,
    GgufTensorDump, GgufTokenizer,
};
#[cfg(not(target_arch = "wasm32"))]
pub use gptoss_tokenizer::GptOssTokenizer;
#[cfg(not(target_arch = "wasm32"))]
pub use gptoss_engine::{
    GptOssCompletion, GptOssEngine, GptOssEngineConfig, GptOssModelConfig, GptOssTokenEvent,
};
#[cfg(feature = "native")]
pub use gptoss_backend::GptOssGgufBackend;
#[cfg(not(target_arch = "wasm32"))]
pub use gptoss_native::{
    apply_bias, apply_rope, attention_head_weights, attention_with_cache, dequant_mxfp4,
    dequant_q8_0, dot_q8_0_row, find_tensor, matmul_f32, matmul_mxfp4_expert, matmul_q8_0,
    read_f32_row, read_f32_tensor, read_meta_f32, read_meta_f32_optional, read_meta_u32,
    read_meta_u32_optional, read_mxfp4_expert, read_q8_0_row, read_tensor_slice, rms_norm, swiglu,
    top_k_softmax, KvCache, LayerKvCache,
};
#[cfg(all(feature = "candle", feature = "wgpu", not(target_arch = "wasm32")))]
pub use gguf_gate::{run_q8_0_gate, GateConfig, GateOutcome};
pub use model::{GenerationOutcome, LoadedModel, ModelKind, ModelSource};
pub use sampling::GenerationConfig;
pub use telemetry::{
    InferenceHook, InferenceTelemetry, ModelLifecycleHook, ModelLifecycleTelemetry, StageStatus,
    TokenCandidate,
};
pub use tokenizer::Tokenizer;

#[cfg(feature = "native")]
pub use provider::{MlProvider, MlProviderConfig};

#[cfg(all(feature = "browser", target_arch = "wasm32"))]
pub use browser_dvm::{BrowserDvm, BrowserDvmService, DvmConfig};
