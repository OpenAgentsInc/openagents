//! GPU hybrid GDN + FFN and 27B full-attention in one Metal stream.

pub use crate::metal_gemm::{
    flush_hybrid_hidden as flush_gpu_hidden, reset_hybrid_state as reset_gpu_state,
    run_full_layer as try_full_layer, run_hybrid_layer as try_hybrid_layer, spec_from_meta,
    AttnSpec, HybridSpec,
};
