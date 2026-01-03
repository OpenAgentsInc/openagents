//! # ml - Candle-powered ML inference
//!
//! Unified ML inference library with Candle backends (CPU/CUDA/Metal) and
//! a browser-first NIP-90 DVM implementation.

mod device;
mod error;
mod http;
#[cfg(not(target_arch = "wasm32"))]
mod gguf;
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
pub use gguf::{load_gguf_index, GgufIndex, GgufTensorDump};
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
