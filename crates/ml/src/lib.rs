//! # ml - Browser-First ML Inference Library
//!
//! A WebGPU-accelerated ML inference library designed to run in the browser,
//! enabling users to serve NIP-90 inference jobs from a browser tab.
//!
//! ## Architecture
//!
//! - `tensor` - Core tensor types (Tensor, DType, Shape, Storage)
//! - `device` - WebGPU device abstraction with CPU fallback
//! - `shaders` - WGSL compute kernels for ML operations
//! - `ops` - High-level operations (matmul, softmax, attention)
//! - `model` - Safetensors loading and weight management
//! - `llm` - Text generation pipeline (tokenizer, KV cache)
//! - `provider` - ComputeProvider and NIP-90 DVM integration
//!
//! ## Browser-First Design
//!
//! This library prioritizes browser execution:
//! - Uses `web_time::Instant` instead of `std::time::Instant`
//! - Async via `wasm_bindgen_futures::spawn_local` (not tokio)
//! - Max workgroup size: 256 invocations
//! - Streaming model weights via HTTP range requests
//!
//! ## Example
//!
//! ```ignore
//! use ml::provider::WebGpuProvider;
//!
//! let provider = WebGpuProvider::new().await?;
//! let job_id = provider.submit(request)?;
//!
//! // Poll for completion (non-blocking)
//! while let Some(chunk) = provider.poll_stream(&job_id)? {
//!     // Handle streaming output
//! }
//! ```

// Core modules (to be implemented)
// pub mod tensor;
// pub mod device;
// pub mod shaders;
// pub mod ops;
// pub mod model;
// pub mod llm;
// pub mod provider;
