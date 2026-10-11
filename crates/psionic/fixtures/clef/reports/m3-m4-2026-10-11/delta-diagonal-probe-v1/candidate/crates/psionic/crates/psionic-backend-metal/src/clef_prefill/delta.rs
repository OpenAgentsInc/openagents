//! Optional blockwise recurrence, compiled separately from the default kernels.

use super::*;
use crate::clef_delta::{CLEF_DELTA_THREADS, ClefDeltaPlan, ClefMetalDelta};

const DELTA_SOURCE: &str = include_str!("../kernels/clef_delta_block.metal");
const PREPARE: &str = "clef_delta_block32_prepare";
const SCAN: &str = "clef_delta_block32_scan";

impl ClefMetal {
    /// Compiles the explicitly selected implementations. Sequential DeltaNet
    /// does not compile or initialize the experimental block library.
    pub fn new_with_execution_options(
        attention: ClefMetalAttention,
        gemm: ClefMetalGemm,
        delta: ClefMetalDelta,
    ) -> Result<Self, String> {
        let mut metal = Self::new_with_options(attention, gemm)?;
        if delta == ClefMetalDelta::BlockTensorOps {
            let library = metal
                .device
                .new_library_with_source(DELTA_SOURCE, &CompileOptions::new())
                .map_err(|error| format!("clef block delta kernels: {error}"))?;
            for name in [PREPARE, SCAN] {
                let function = library
                    .get_function(name, None)
                    .map_err(|error| format!("clef block delta kernel {name}: {error}"))?;
                let pipeline = metal
                    .device
                    .new_compute_pipeline_state_with_function(&function)
                    .map_err(|error| format!("clef block delta pipeline {name}: {error}"))?;
                if pipeline.thread_execution_width() != 32
                    || pipeline.max_total_threads_per_threadgroup() < CLEF_DELTA_THREADS as u64
                {
                    return Err(String::from(
                        "clef block delta requires four 32-thread SIMD groups",
                    ));
                }
                if pipeline.static_threadgroup_memory_length()
                    > metal.device.max_threadgroup_memory_length()
                {
                    return Err(String::from(
                        "clef block delta exceeds device threadgroup memory",
                    ));
                }
                metal.pipelines.insert(name, pipeline);
            }
        }
        Ok(metal)
    }

    /// The recurrent implementation selected at construction.
    #[must_use]
    pub fn delta_mode(&self) -> ClefMetalDelta {
        if self.pipelines.contains_key(SCAN) {
            ClefMetalDelta::BlockTensorOps
        } else {
            ClefMetalDelta::Sequential
        }
    }
}

impl ClefMetalBatch<'_> {
    /// Runs the experimental block32 recurrence with f32 inputs and state.
    ///
    /// `workspace` belongs to this request. Reusing it across layers is valid
    /// when those dispatches are ordered on the same batch. The caller supplies
    /// finite inputs with decay and beta in `[0, 1]`, as produced by delta prep.
    /// `key_dot_query` contains the prepared dot product for each query/key head,
    /// including the query scale, as in the sequential scan.
    #[allow(clippy::too_many_arguments)]
    pub fn delta_scan_block(
        &self,
        qn: &ClefMetalBuffer,
        kn: &ClefMetalBuffer,
        conv: &ClefMetalBuffer,
        decay: &ClefMetalBuffer,
        beta: &ClefMetalBuffer,
        key_dot_query: &ClefMetalBuffer,
        state: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        workspace: &ClefMetalBuffer,
        n: usize,
        key_heads: usize,
        value_heads: usize,
        dim: usize,
        v_head_reordered: bool,
        conv_width: usize,
        value_offset: usize,
    ) -> Result<(), String> {
        let plan = ClefDeltaPlan::new(
            n,
            key_heads,
            value_heads,
            dim,
            v_head_reordered,
            conv_width,
            value_offset,
        )?;
        if n == 0 {
            return Ok(());
        }
        for (buffer, bytes, name) in [
            (qn, plan.query_key_bytes, "block delta query"),
            (kn, plan.query_key_bytes, "block delta key"),
            (conv, plan.conv_bytes, "block delta convolution"),
            (decay, plan.decay_beta_bytes, "block delta decay"),
            (beta, plan.decay_beta_bytes, "block delta beta"),
            (
                key_dot_query,
                plan.query_key_bytes / dim,
                "block delta key/query dot",
            ),
            (state, plan.state_bytes, "block delta state"),
            (out, plan.output_bytes, "block delta output"),
            (workspace, plan.workspace_bytes, "block delta workspace"),
        ] {
            buffer.check(0, bytes, name)?;
        }
        if self.metal.delta_mode() != ClefMetalDelta::BlockTensorOps {
            return Err(String::from("clef block delta was not selected at startup"));
        }
        let args = DeltaScanArgs {
            n: plan.n,
            key_heads: plan.key_heads,
            value_heads: plan.value_heads,
            v_head_reordered: plan.v_head_reordered,
            conv_width: plan.conv_width,
            value_offset: plan.value_offset,
        };
        self.pipeline(PREPARE)?;
        for (index, buffer) in [qn, kn, conv, decay, beta, workspace, key_dot_query]
            .into_iter()
            .enumerate()
        {
            self.bind(index as u64, buffer, 0);
        }
        self.bytes(7, &args);
        self.groups(
            size(plan.blocks, value_heads, 1),
            size(CLEF_DELTA_THREADS, 1, 1),
        );
        // groups() inserts the device barrier between preparation and scan.
        self.pipeline(SCAN)?;
        for (index, buffer) in [qn, kn, workspace, state, out].into_iter().enumerate() {
            self.bind(index as u64, buffer, 0);
        }
        self.bytes(5, &args);
        self.groups(size(4, value_heads, 1), size(CLEF_DELTA_THREADS, 1, 1));
        Ok(())
    }
}
