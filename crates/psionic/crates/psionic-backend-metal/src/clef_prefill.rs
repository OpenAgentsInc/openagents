//! Sequence-prefill operations for the Clef decision lane on Metal
//! (OpenAgentsInc/openagents #11196): the kernels in
//! `kernels/clef_prefill.metal`, recorded onto one compute encoder per
//! [`ClefMetalBatch`] and run with [`ClefMetalBatch::commit_wait`].
//!
//! Buffers are shared (unified memory): the host writes inputs and reads
//! results through their contents directly. Offsets are in elements.
//! Projections are f16 x f16 -> f32 GEMMs on the Metal Performance
//! Primitives `matmul2d` tensor op, over weights dequantized to f16 once.

use std::collections::HashMap;
use std::ffi::c_void;

use metal::{
    Buffer, CommandBuffer, CommandQueue, CompileOptions, ComputeCommandEncoder, ComputePipelineState, Device,
    MTLResourceOptions, MTLSize,
};

const SOURCE: &str = include_str!("kernels/clef_prefill.metal");

/// A weight layout the lane dequantizes to f16 at load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClefMetalWeightFormat {
    /// GGML Q8_0 (34-byte blocks of 32).
    Q8_0,
    /// GGML Q4_K (144-byte super-blocks of 256).
    Q4K,
    /// Dense row-major f32.
    F32,
}

impl ClefMetalWeightFormat {
    /// Bytes of `elements` weights, or `None` when not whole blocks.
    #[must_use]
    pub const fn byte_len(self, elements: usize) -> Option<usize> {
        match self {
            Self::Q8_0 if elements % 32 == 0 => Some(elements / 32 * 34),
            Self::Q4K if elements % 256 == 0 => Some(elements / 256 * 144),
            Self::F32 => Some(elements * 4),
            _ => None,
        }
    }
}

/// A shared Metal buffer.
pub struct ClefMetalBuffer {
    raw: Buffer,
    bytes: usize,
}

// SAFETY: a shared MTLBuffer is a reference-counted Objective-C object that
// Metal allows any thread to use; the lane serializes all use behind a mutex.
unsafe impl Send for ClefMetalBuffer {}
unsafe impl Sync for ClefMetalBuffer {}

impl ClefMetalBuffer {
    /// Bytes allocated.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.bytes
    }

    fn check(&self, offset_bytes: usize, len_bytes: usize, what: &str) -> Result<(), String> {
        match offset_bytes.checked_add(len_bytes) {
            Some(end) if end <= self.bytes => Ok(()),
            _ => Err(format!(
                "clef metal {what}: {len_bytes} bytes at {offset_bytes} exceed a {}-byte buffer",
                self.bytes
            )),
        }
    }

    /// Writes raw bytes at a byte offset.
    pub fn write_bytes(&self, offset_bytes: usize, bytes: &[u8]) -> Result<(), String> {
        self.check(offset_bytes, bytes.len(), "write")?;
        // SAFETY: the range lies inside the shared allocation (checked above).
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                self.raw.contents().cast::<u8>().add(offset_bytes),
                bytes.len(),
            );
        }
        Ok(())
    }

    /// Writes f32 values at an element offset.
    pub fn write_f32(&self, offset: usize, values: &[f32]) -> Result<(), String> {
        // SAFETY: f32 has no padding; the slice covers exactly its values.
        let bytes = unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values)) };
        self.write_bytes(offset * 4, bytes)
    }

    /// Writes i32 values at an element offset.
    pub fn write_i32(&self, offset: usize, values: &[i32]) -> Result<(), String> {
        // SAFETY: i32 has no padding; the slice covers exactly its values.
        let bytes = unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values)) };
        self.write_bytes(offset * 4, bytes)
    }

    /// Reads `count` f32 values at an element offset.
    pub fn read_f32(&self, offset: usize, count: usize) -> Result<Vec<f32>, String> {
        self.check(offset * 4, count * 4, "read")?;
        let mut out = vec![0.0f32; count];
        // SAFETY: the range lies inside the shared allocation (checked above).
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.raw.contents().cast::<f32>().add(offset),
                out.as_mut_ptr(),
                count,
            );
        }
        Ok(out)
    }

    /// Zeroes the whole buffer.
    pub fn zero(&self) {
        // SAFETY: writes exactly the allocation.
        unsafe { std::ptr::write_bytes(self.raw.contents().cast::<u8>(), 0, self.bytes) }
    }
}

/// The Clef kernels on one Metal device.
pub struct ClefMetal {
    device: Device,
    queue: CommandQueue,
    pipelines: HashMap<&'static str, ComputePipelineState>,
    name: String,
}

// SAFETY: the device, queue and pipeline states are Objective-C objects
// Metal allows any thread to use; the lane serializes all use.
unsafe impl Send for ClefMetal {}
unsafe impl Sync for ClefMetal {}

const KERNELS: &[&str] = &[
    "clef_dequant_q8_0",
    "clef_dequant_q4_k",
    "clef_f32_to_f16",
    "clef_gemm",
    "clef_gemm_accumulate",
    "clef_gemm_f32",
    "clef_gemm_f32_nn",
    "clef_rms_norm_to_f16",
    "clef_rms_norm_f32",
    "clef_layer_norm_f32",
    "clef_conv1d_seq_silu",
    "clef_conv1d_state",
    "clef_delta_prep",
    "clef_delta_scan128_256x2",
    "clef_gated_norm_to_f16",
    "clef_attention_prep",
    "clef_attn_scores",
    "clef_causal_softmax_to_f16",
    "clef_attn_pv",
    "clef_softmax_rows_f32",
    "clef_sigmoid_gate_to_f16",
    "clef_silu_mul_to_f16",
    "clef_span_sums",
    "clef_linear_f32_ordered",
    "clef_head_side",
    "clef_head_context",
];

impl ClefMetal {
    /// Compiles the Clef kernels on the system default device.
    pub fn new() -> Result<Self, String> {
        let device = Device::system_default().ok_or("no Metal device is available")?;
        let library = device
            .new_library_with_source(SOURCE, &CompileOptions::new())
            .map_err(|error| format!("clef metal kernels: {error}"))?;
        let mut pipelines = HashMap::new();
        for name in KERNELS {
            let function = library
                .get_function(name, None)
                .map_err(|error| format!("clef metal kernel {name}: {error}"))?;
            let pipeline = device
                .new_compute_pipeline_state_with_function(&function)
                .map_err(|error| format!("clef metal pipeline {name}: {error}"))?;
            pipelines.insert(*name, pipeline);
        }
        let queue = device.new_command_queue();
        let name = device.name().to_string();
        Ok(Self {
            device,
            queue,
            pipelines,
            name,
        })
    }

    /// The device name.
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.name
    }

    /// A zeroed shared buffer of `bytes`.
    #[must_use]
    pub fn buffer(&self, bytes: usize) -> ClefMetalBuffer {
        let bytes = bytes.max(16);
        let raw = self.device.new_buffer(bytes as u64, MTLResourceOptions::StorageModeShared);
        let buffer = ClefMetalBuffer { raw, bytes };
        buffer.zero();
        buffer
    }

    /// A shared buffer holding `bytes`.
    #[must_use]
    pub fn buffer_with_bytes(&self, bytes: &[u8]) -> ClefMetalBuffer {
        let raw = self.device.new_buffer_with_data(
            bytes.as_ptr().cast::<c_void>(),
            bytes.len().max(16) as u64,
            MTLResourceOptions::StorageModeShared,
        );
        ClefMetalBuffer {
            raw,
            bytes: bytes.len().max(16),
        }
    }

    /// A shared buffer holding f32 values.
    #[must_use]
    pub fn buffer_f32(&self, values: &[f32]) -> ClefMetalBuffer {
        // SAFETY: f32 has no padding; the slice covers exactly its values.
        let bytes = unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values)) };
        self.buffer_with_bytes(bytes)
    }

    /// A batch of kernels recorded onto one command buffer.
    #[must_use]
    pub fn batch(&self) -> ClefMetalBatch<'_> {
        let command_buffer = self.queue.new_command_buffer().to_owned();
        let encoder = command_buffer.new_compute_command_encoder().to_owned();
        ClefMetalBatch {
            metal: self,
            command_buffer,
            encoder,
        }
    }
}

/// Kernels recorded onto one command buffer.
pub struct ClefMetalBatch<'a> {
    metal: &'a ClefMetal,
    command_buffer: CommandBuffer,
    encoder: ComputeCommandEncoder,
}

fn size(width: usize, height: usize, depth: usize) -> MTLSize {
    MTLSize::new(width as u64, height as u64, depth as u64)
}

fn int(value: usize, what: &str) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| format!("clef metal {what} exceeds u32"))
}

#[repr(C)]
struct DeltaPrepArgs {
    key_heads: u32,
    value_heads: u32,
    dim: u32,
    conv_width: u32,
}

#[repr(C)]
struct DeltaScanArgs {
    n: u32,
    key_heads: u32,
    value_heads: u32,
    v_head_reordered: u32,
    conv_width: u32,
    value_offset: u32,
}

#[repr(C)]
struct AttentionPrepArgs {
    heads: u32,
    kv_heads: u32,
    dim: u32,
    rot: u32,
    pos0: u32,
    scale: f32,
    eps: f32,
}

#[repr(C)]
struct AttnArgs {
    n: u32,
    keys: u32,
    heads: u32,
    kv_heads: u32,
    head: u32,
}

impl ClefMetalBatch<'_> {
    fn pipeline(&self, name: &'static str) -> Result<(), String> {
        let pipeline = self
            .metal
            .pipelines
            .get(name)
            .ok_or_else(|| format!("clef metal kernel {name} is not loaded"))?;
        self.encoder.set_compute_pipeline_state(pipeline);
        Ok(())
    }

    fn bind(&self, index: u64, buffer: &ClefMetalBuffer, offset_bytes: usize) {
        self.encoder.set_buffer(index, Some(&buffer.raw), offset_bytes as u64);
    }

    fn bytes<T>(&self, index: u64, value: &T) {
        self.encoder
            .set_bytes(index, std::mem::size_of::<T>() as u64, (value as *const T).cast::<c_void>());
    }

    fn groups(&self, groups: MTLSize, threads: MTLSize) {
        self.encoder.dispatch_thread_groups(groups, threads);
    }

    fn threads(&self, count: usize, per_group: usize) {
        let groups = count.div_ceil(per_group).max(1);
        self.encoder
            .dispatch_thread_groups(size(groups, 1, 1), size(per_group, 1, 1));
    }

    /// Ends the batch, runs it, and waits for it.
    pub fn commit_wait(self) -> Result<(), String> {
        self.encoder.end_encoding();
        self.command_buffer.commit();
        self.command_buffer.wait_until_completed();
        match self.command_buffer.status() {
            metal::MTLCommandBufferStatus::Completed => Ok(()),
            status => Err(format!("clef metal command buffer ended {status:?}")),
        }
    }

    /// Dequantizes `elements` weights (row-major) into an f16 buffer.
    pub fn dequantize_to_f16(
        &self,
        format: ClefMetalWeightFormat,
        source: &ClefMetalBuffer,
        destination: &ClefMetalBuffer,
        elements: usize,
    ) -> Result<(), String> {
        let bytes = format
            .byte_len(elements)
            .ok_or("clef metal dequantize: not whole blocks")?;
        source.check(0, bytes, "dequantize source")?;
        destination.check(0, elements * 2, "dequantize destination")?;
        self.pipeline(match format {
            ClefMetalWeightFormat::Q8_0 => "clef_dequant_q8_0",
            ClefMetalWeightFormat::Q4K => "clef_dequant_q4_k",
            ClefMetalWeightFormat::F32 => "clef_f32_to_f16",
        })?;
        self.bind(0, source, 0);
        self.bind(1, destination, 0);
        let count = elements as u64;
        self.bytes(2, &count);
        self.threads(elements, 256);
        Ok(())
    }

    /// `out[n, m] (+)= x16[n, k] · W16[m, k]^T` (`out` f32 from element
    /// `out_offset`; `x16` from element `x_offset`).
    #[allow(clippy::too_many_arguments)]
    pub fn gemm(
        &self,
        x16: &ClefMetalBuffer,
        x_offset: usize,
        w16: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        out_offset: usize,
        n: usize,
        m: usize,
        k: usize,
        accumulate: bool,
    ) -> Result<(), String> {
        if n == 0 {
            return Ok(());
        }
        x16.check(x_offset * 2, n * k * 2, "gemm x")?;
        w16.check(0, m * k * 2, "gemm w")?;
        out.check(out_offset * 4, n * m * 4, "gemm out")?;
        self.pipeline(if accumulate { "clef_gemm_accumulate" } else { "clef_gemm" })?;
        self.bind(0, x16, x_offset * 2);
        self.bind(1, w16, 0);
        self.bind(2, out, out_offset * 4);
        let nmk = [int(n, "n")?, int(m, "m")?, int(k, "k")?, 0];
        self.bytes(3, &nmk);
        self.groups(size(m.div_ceil(64), n.div_ceil(64), 1), size(128, 1, 1));
        Ok(())
    }

    /// `out[n, m] = x[n, k] · W[m, k]^T` in f32 (the head's products).
    #[allow(clippy::too_many_arguments)]
    pub fn gemm_f32(
        &self,
        x: &ClefMetalBuffer,
        w: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        n: usize,
        m: usize,
        k: usize,
    ) -> Result<(), String> {
        if n == 0 {
            return Ok(());
        }
        x.check(0, n * k * 4, "gemm_f32 x")?;
        w.check(0, m * k * 4, "gemm_f32 w")?;
        out.check(0, n * m * 4, "gemm_f32 out")?;
        self.pipeline("clef_gemm_f32")?;
        self.bind(0, x, 0);
        self.bind(1, w, 0);
        self.bind(2, out, 0);
        let nmk = [int(n, "n")?, int(m, "m")?, int(k, "k")?, 0];
        self.bytes(3, &nmk);
        self.groups(size(m.div_ceil(64), n.div_ceil(64), 1), size(128, 1, 1));
        Ok(())
    }

    /// `out[n, m] = x[n, k] · W[k, m]` in f32 (`W` row-major `[k, m]`).
    #[allow(clippy::too_many_arguments)]
    pub fn gemm_f32_nn(
        &self,
        x: &ClefMetalBuffer,
        w: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        n: usize,
        m: usize,
        k: usize,
    ) -> Result<(), String> {
        if n == 0 {
            return Ok(());
        }
        x.check(0, n * k * 4, "gemm_f32_nn x")?;
        w.check(0, k * m * 4, "gemm_f32_nn w")?;
        out.check(0, n * m * 4, "gemm_f32_nn out")?;
        self.pipeline("clef_gemm_f32_nn")?;
        self.bind(0, x, 0);
        self.bind(1, w, 0);
        self.bind(2, out, 0);
        let nmk = [int(n, "n")?, int(m, "m")?, int(k, "k")?, 0];
        self.bytes(3, &nmk);
        self.groups(size(m.div_ceil(64), n.div_ceil(64), 1), size(128, 1, 1));
        Ok(())
    }

    /// `out16[r] = RMSNorm(x[r]) * w` for `rows` rows of width `d`.
    pub fn rms_norm_to_f16(
        &self,
        x: &ClefMetalBuffer,
        w: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        rows: usize,
        d: usize,
        eps: f32,
    ) -> Result<(), String> {
        x.check(0, rows * d * 4, "rms x")?;
        out.check(0, rows * d * 2, "rms out")?;
        self.pipeline("clef_rms_norm_to_f16")?;
        self.bind(0, x, 0);
        self.bind(1, w, 0);
        self.bind(2, out, 0);
        self.bytes(3, &int(d, "d")?);
        self.bytes(4, &eps);
        self.groups(size(rows, 1, 1), size(256, 1, 1));
        Ok(())
    }

    /// `out[r] = RMSNorm(x[r]) * w` (f32).
    pub fn rms_norm_f32(
        &self,
        x: &ClefMetalBuffer,
        w: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        rows: usize,
        d: usize,
        eps: f32,
    ) -> Result<(), String> {
        x.check(0, rows * d * 4, "rms x")?;
        out.check(0, rows * d * 4, "rms out")?;
        self.pipeline("clef_rms_norm_f32")?;
        self.bind(0, x, 0);
        self.bind(1, w, 0);
        self.bind(2, out, 0);
        self.bytes(3, &int(d, "d")?);
        self.bytes(4, &eps);
        self.groups(size(rows, 1, 1), size(256, 1, 1));
        Ok(())
    }

    /// `out[r] = LayerNorm(x[r]) (* w + b)` (f32).
    #[allow(clippy::too_many_arguments)]
    pub fn layer_norm_f32(
        &self,
        x: &ClefMetalBuffer,
        affine: Option<(&ClefMetalBuffer, &ClefMetalBuffer)>,
        out: &ClefMetalBuffer,
        rows: usize,
        d: usize,
        eps: f32,
    ) -> Result<(), String> {
        x.check(0, rows * d * 4, "layer norm x")?;
        out.check(0, rows * d * 4, "layer norm out")?;
        self.pipeline("clef_layer_norm_f32")?;
        self.bind(0, x, 0);
        let flag: u32 = match affine {
            Some((w, b)) => {
                self.bind(1, w, 0);
                self.bind(2, b, 0);
                1
            }
            None => {
                self.bind(1, x, 0);
                self.bind(2, x, 0);
                0
            }
        };
        self.bind(3, out, 0);
        self.bytes(4, &int(d, "d")?);
        self.bytes(5, &eps);
        self.bytes(6, &flag);
        self.groups(size(rows, 1, 1), size(256, 1, 1));
        Ok(())
    }

    /// Causal depthwise conv over `n` tokens with carried state, then SiLU;
    /// updates the state.
    #[allow(clippy::too_many_arguments)]
    pub fn conv1d_seq_silu(
        &self,
        input: &ClefMetalBuffer,
        state: &ClefMetalBuffer,
        weights: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        n: usize,
        channels: usize,
        kernel: usize,
    ) -> Result<(), String> {
        if !(1..=4).contains(&kernel) {
            return Err(String::from("clef metal conv: kernel size outside 1..=4"));
        }
        input.check(0, n * channels * 4, "conv input")?;
        out.check(0, n * channels * 4, "conv out")?;
        let nck = [int(n, "n")?, int(channels, "channels")?, int(kernel, "kernel")?, 0];
        self.pipeline("clef_conv1d_seq_silu")?;
        self.bind(0, input, 0);
        self.bind(1, state, 0);
        self.bind(2, weights, 0);
        self.bind(3, out, 0);
        self.bytes(4, &nck);
        self.threads(n * channels, 256);
        self.pipeline("clef_conv1d_state")?;
        self.bind(0, input, 0);
        self.bind(1, state, 0);
        self.bytes(2, &nck);
        self.threads(channels, 256);
        Ok(())
    }

    /// Normalized q/k per key head and decay/beta per value head.
    #[allow(clippy::too_many_arguments)]
    pub fn delta_prep(
        &self,
        conv: &ClefMetalBuffer,
        alpha: &ClefMetalBuffer,
        beta_in: &ClefMetalBuffer,
        ssm_a: &ClefMetalBuffer,
        ssm_dt: &ClefMetalBuffer,
        qn: &ClefMetalBuffer,
        kn: &ClefMetalBuffer,
        decay: &ClefMetalBuffer,
        beta: &ClefMetalBuffer,
        key_dot_query: &ClefMetalBuffer,
        n: usize,
        key_heads: usize,
        value_heads: usize,
        dim: usize,
        conv_width: usize,
    ) -> Result<(), String> {
        if dim > 1024 || n == 0 {
            return if n == 0 { Ok(()) } else { Err(String::from("clef metal delta prep: dim above 1024")) };
        }
        qn.check(0, n * key_heads * dim * 4, "qn")?;
        let args = DeltaPrepArgs {
            key_heads: int(key_heads, "key heads")?,
            value_heads: int(value_heads, "value heads")?,
            dim: int(dim, "dim")?,
            conv_width: int(conv_width, "conv width")?,
        };
        self.pipeline("clef_delta_prep")?;
        for (index, buffer) in [conv, alpha, beta_in, ssm_a, ssm_dt, qn, kn, decay, beta, key_dot_query]
            .into_iter()
            .enumerate()
        {
            self.bind(index as u64, buffer, 0);
        }
        self.bytes(10, &args);
        self.groups(size(n, key_heads, 1), size(dim.div_ceil(32) * 32, 1, 1));
        Ok(())
    }

    /// The gated delta rule over `n` tokens for 128-wide heads (state carried
    /// in place).
    #[allow(clippy::too_many_arguments)]
    pub fn delta_scan(
        &self,
        qn: &ClefMetalBuffer,
        kn: &ClefMetalBuffer,
        conv: &ClefMetalBuffer,
        decay: &ClefMetalBuffer,
        beta: &ClefMetalBuffer,
        key_dot_query: &ClefMetalBuffer,
        state: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        n: usize,
        key_heads: usize,
        value_heads: usize,
        dim: usize,
        v_head_reordered: bool,
        conv_width: usize,
        value_offset: usize,
    ) -> Result<(), String> {
        if dim != 128 {
            return Err(format!("clef metal delta scan: head dim {dim} (this lane takes 128)"));
        }
        if n == 0 {
            return Ok(());
        }
        state.check(0, value_heads * dim * dim * 4, "delta state")?;
        out.check(0, n * value_heads * dim * 4, "delta out")?;
        let args = DeltaScanArgs {
            n: int(n, "n")?,
            key_heads: int(key_heads, "key heads")?,
            value_heads: int(value_heads, "value heads")?,
            v_head_reordered: u32::from(v_head_reordered),
            conv_width: int(conv_width, "conv width")?,
            value_offset: int(value_offset, "value offset")?,
        };
        self.pipeline("clef_delta_scan128_256x2")?;
        for (index, buffer) in [qn, kn, conv, decay, beta, key_dot_query, state, out].into_iter().enumerate() {
            self.bind(index as u64, buffer, 0);
        }
        self.bytes(8, &args);
        // 64 state rows per threadgroup
        self.groups(size(value_heads * dim / 64, 1, 1), size(256, 1, 1));
        Ok(())
    }

    /// Per-head `RMSNorm(o) * w * silu(z)` to f16.
    #[allow(clippy::too_many_arguments)]
    pub fn gated_norm_to_f16(
        &self,
        o: &ClefMetalBuffer,
        z: &ClefMetalBuffer,
        w: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        n: usize,
        heads: usize,
        dim: usize,
        eps: f32,
    ) -> Result<(), String> {
        out.check(0, n * heads * dim * 2, "gated norm out")?;
        self.pipeline("clef_gated_norm_to_f16")?;
        self.bind(0, o, 0);
        self.bind(1, z, 0);
        self.bind(2, w, 0);
        self.bind(3, out, 0);
        let hd = [int(heads, "heads")?, int(dim, "dim")?];
        self.bytes(4, &hd);
        self.bytes(5, &eps);
        self.groups(size(n, heads, 1), size(dim.div_ceil(32) * 32, 1, 1));
        Ok(())
    }

    /// Full-attention input prep: q/gate split, q/k RMSNorm, rotary, q
    /// scale; k and v appended to the f16 caches at `pos0`.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_prep(
        &self,
        query_gate: &ClefMetalBuffer,
        key: &ClefMetalBuffer,
        value: &ClefMetalBuffer,
        query_norm: &ClefMetalBuffer,
        key_norm: &ClefMetalBuffer,
        cos_sin: &ClefMetalBuffer,
        query16: &ClefMetalBuffer,
        gate: &ClefMetalBuffer,
        key_cache: &ClefMetalBuffer,
        value_cache: &ClefMetalBuffer,
        n: usize,
        heads: usize,
        kv_heads: usize,
        dim: usize,
        rotary: usize,
        pos0: usize,
        scale: f32,
        eps: f32,
    ) -> Result<(), String> {
        if dim > 1024 || rotary > dim {
            return Err(String::from("clef metal attention prep: dims outside this lane"));
        }
        key_cache.check(0, (pos0 + n) * kv_heads * dim * 2, "key cache")?;
        query16.check(0, n * heads * dim * 2, "query16")?;
        let args = AttentionPrepArgs {
            heads: int(heads, "heads")?,
            kv_heads: int(kv_heads, "kv heads")?,
            dim: int(dim, "dim")?,
            rot: int(rotary, "rotary")?,
            pos0: int(pos0, "pos0")?,
            scale,
            eps,
        };
        self.pipeline("clef_attention_prep")?;
        for (index, buffer) in [
            query_gate, key, value, query_norm, key_norm, cos_sin, query16, gate, key_cache, value_cache,
        ]
        .into_iter()
        .enumerate()
        {
            self.bind(index as u64, buffer, 0);
        }
        self.bytes(10, &args);
        self.groups(size(n, heads + kv_heads, 1), size(dim.div_ceil(32) * 32, 1, 1));
        Ok(())
    }

    /// Causal attention of `n` queries at positions `first..` (f16, scaled,
    /// `[n, heads, 256]`) over the f16 caches, into f32 `out`
    /// `[n, heads, 256]`, through `scores` (`n x (first + n)` f32) and
    /// `probs` (the same in f16), one query head at a time.
    #[allow(clippy::too_many_arguments)]
    pub fn attention(
        &self,
        query16: &ClefMetalBuffer,
        key_cache: &ClefMetalBuffer,
        value_cache: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        scores: &ClefMetalBuffer,
        probs: &ClefMetalBuffer,
        n: usize,
        heads: usize,
        kv_heads: usize,
        dim: usize,
        first: usize,
    ) -> Result<(), String> {
        if dim != 256 || kv_heads == 0 || heads % kv_heads != 0 {
            return Err(format!("clef metal attention: head dim {dim} (this lane takes 256)"));
        }
        if n == 0 {
            return Ok(());
        }
        let keys = first + n;
        scores.check(0, n * keys * 4, "attention scores")?;
        probs.check(0, n * keys * 2, "attention probabilities")?;
        out.check(0, n * heads * dim * 4, "attention out")?;
        for head in 0..heads {
            let args = AttnArgs {
                n: int(n, "n")?,
                keys: int(keys, "keys")?,
                heads: int(heads, "heads")?,
                kv_heads: int(kv_heads, "kv heads")?,
                head: int(head, "head")?,
            };
            self.pipeline("clef_attn_scores")?;
            self.bind(0, query16, 0);
            self.bind(1, key_cache, 0);
            self.bind(2, scores, 0);
            self.bytes(3, &args);
            self.groups(size(keys.div_ceil(64), n.div_ceil(64), 1), size(128, 1, 1));
            self.pipeline("clef_causal_softmax_to_f16")?;
            self.bind(0, scores, 0);
            self.bind(1, probs, 0);
            let nkf = [int(n, "n")?, int(keys, "keys")?, int(first, "first")?, 0];
            self.bytes(2, &nkf);
            self.groups(size(n, 1, 1), size(256, 1, 1));
            self.pipeline("clef_attn_pv")?;
            self.bind(0, probs, 0);
            self.bind(1, value_cache, 0);
            self.bind(2, out, 0);
            self.bytes(3, &args);
            self.groups(size(dim.div_ceil(64), n.div_ceil(64), 1), size(128, 1, 1));
        }
        Ok(())
    }

    /// In-place softmax of `rows` full rows after multiplying by `scale`.
    pub fn softmax_rows_f32(&self, scores: &ClefMetalBuffer, rows: usize, keys: usize, scale: f32) -> Result<(), String> {
        scores.check(0, rows * keys * 4, "softmax scores")?;
        self.pipeline("clef_softmax_rows_f32")?;
        self.bind(0, scores, 0);
        self.bytes(1, &int(keys, "keys")?);
        self.bytes(2, &scale);
        self.groups(size(rows, 1, 1), size(256, 1, 1));
        Ok(())
    }

    /// `out16 = o * sigmoid(gate)`.
    pub fn sigmoid_gate_to_f16(
        &self,
        o: &ClefMetalBuffer,
        gate: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        count: usize,
    ) -> Result<(), String> {
        out.check(0, count * 2, "gated out")?;
        self.pipeline("clef_sigmoid_gate_to_f16")?;
        self.bind(0, o, 0);
        self.bind(1, gate, 0);
        self.bind(2, out, 0);
        self.bytes(3, &(count as u64));
        self.threads(count, 256);
        Ok(())
    }

    /// `out16 = silu(gate) * up`.
    pub fn silu_mul_to_f16(
        &self,
        gate: &ClefMetalBuffer,
        up: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        count: usize,
    ) -> Result<(), String> {
        out.check(0, count * 2, "ffn out")?;
        self.pipeline("clef_silu_mul_to_f16")?;
        self.bind(0, gate, 0);
        self.bind(1, up, 0);
        self.bind(2, out, 0);
        self.bytes(3, &(count as u64));
        self.threads(count, 256);
        Ok(())
    }

    /// Adds rows `[first, first + n)` into the span sums.
    #[allow(clippy::too_many_arguments)]
    pub fn span_sums(
        &self,
        rows: &ClefMetalBuffer,
        spans: &ClefMetalBuffer,
        sums: &ClefMetalBuffer,
        span_count: usize,
        d: usize,
        first: usize,
        n: usize,
    ) -> Result<(), String> {
        if span_count == 0 {
            return Ok(());
        }
        sums.check(0, span_count * d * 4, "span sums")?;
        self.pipeline("clef_span_sums")?;
        self.bind(0, rows, 0);
        self.bind(1, spans, 0);
        self.bind(2, sums, 0);
        let dfn = [int(d, "d")?, int(first, "first")?, int(n, "n")?, 0];
        self.bytes(3, &dfn);
        self.groups(size(span_count, 1, 1), size(256, 1, 1));
        Ok(())
    }

    /// `out[n, m] = x[n, k] · W[m, k]^T` (f32) with each output summed in a
    /// fixed order; `out` from element `out_offset`.
    #[allow(clippy::too_many_arguments)]
    pub fn linear_f32_ordered(
        &self,
        x: &ClefMetalBuffer,
        w: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        out_offset: usize,
        n: usize,
        m: usize,
        k: usize,
    ) -> Result<(), String> {
        if n == 0 {
            return Ok(());
        }
        out.check(out_offset * 4, n * m * 4, "ordered out")?;
        self.pipeline("clef_linear_f32_ordered")?;
        self.bind(0, x, 0);
        self.bind(1, w, 0);
        self.bind(2, out, out_offset * 4);
        let nmk = [int(n, "n")?, int(m, "m")?, int(k, "k")?, 0];
        self.bytes(3, &nmk);
        self.groups(size(m.div_ceil(64), n.div_ceil(64), 1), size(256, 1, 1));
        Ok(())
    }

    /// The head's memory-attention query side (see the kernel).
    #[allow(clippy::too_many_arguments)]
    pub fn head_side(
        &self,
        q: &ClefMetalBuffer,
        wk: &ClefMetalBuffer,
        side: &ClefMetalBuffer,
        n: usize,
        heads: usize,
        width: usize,
    ) -> Result<(), String> {
        side.check(0, n * heads * width * 4, "head side")?;
        self.pipeline("clef_head_side")?;
        self.bind(0, q, 0);
        self.bind(1, wk, 0);
        self.bind(2, side, 0);
        let hw = [int(heads, "heads")?, int(width, "width")?];
        self.bytes(3, &hw);
        self.groups(size(n * heads, 1, 1), size(256, 1, 1));
        Ok(())
    }

    /// `out[i, e] = W_v[e, :] · mixed[i, head(e), :] + b_v[e]`.
    #[allow(clippy::too_many_arguments)]
    pub fn head_context(
        &self,
        mixed: &ClefMetalBuffer,
        wv: &ClefMetalBuffer,
        bv: &ClefMetalBuffer,
        out: &ClefMetalBuffer,
        n: usize,
        heads: usize,
        width: usize,
    ) -> Result<(), String> {
        out.check(0, n * width * 4, "head context")?;
        self.pipeline("clef_head_context")?;
        self.bind(0, mixed, 0);
        self.bind(1, wv, 0);
        self.bind(2, bv, 0);
        self.bind(3, out, 0);
        let nhw = [int(n, "n")?, int(heads, "heads")?, int(width, "width")?, 0];
        self.bytes(4, &nhw);
        self.threads(n * width * 32, 256);
        Ok(())
    }
}
