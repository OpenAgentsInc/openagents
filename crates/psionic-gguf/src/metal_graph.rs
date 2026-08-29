//! GPU hybrid layer (GDN + FFN). Full-attention layers stay on CPU.

pub use crate::metal_gemm::{
    flush_hybrid_hidden as flush_gpu_hidden, reset_hybrid_state as reset_gpu_state,
    run_hybrid_layer as try_hybrid_layer, spec_from_meta, HybridSpec,
};
