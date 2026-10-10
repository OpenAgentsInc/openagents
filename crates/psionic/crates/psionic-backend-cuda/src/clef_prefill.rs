//! Sequence-prefill operations for the Clef decision lane
//! (OpenAgentsInc/openagents #11195): the kernels in
//! `kernels/clef_prefill.cu` and the cuBLAS GEMMs between them, recorded
//! onto a [`CudaSubmission`]'s stream.
//!
//! Buffers are passed with element offsets so one allocation can hold a
//! whole chunk's activations. Every operation checks that its ranges fit the
//! buffers before it launches.

use std::ffi::c_void;

use psionic_runtime::RuntimeError;

use crate::{CudaBuffer, CudaSubmission};

unsafe extern "C" {
    fn psionic_clef_dequant_q8_0_f16(src: *const c_void, dst: *mut c_void, blocks: i64, stream: *mut c_void) -> i32;
    fn psionic_clef_dequant_q4_k_f16(src: *const c_void, dst: *mut c_void, super_blocks: i64, stream: *mut c_void) -> i32;
    fn psionic_clef_f16_to_f32(src: *const c_void, dst: *mut c_void, count: i64, accumulate: i32, stream: *mut c_void) -> i32;
    fn psionic_clef_f32_to_f16(src: *const c_void, dst: *mut c_void, count: i64, stream: *mut c_void) -> i32;
    fn psionic_clef_rms_norm_to_f16(x: *const c_void, w: *const c_void, out: *mut c_void, rows: i32, d: i32, eps: f32, stream: *mut c_void) -> i32;
    fn psionic_clef_rms_norm_f32(x: *const c_void, w: *const c_void, out: *mut c_void, rows: i32, d: i32, eps: f32, stream: *mut c_void) -> i32;
    fn psionic_clef_layer_norm_f32(x: *const c_void, w: *const c_void, b: *const c_void, out: *mut c_void, rows: i32, d: i32, eps: f32, stream: *mut c_void) -> i32;
    fn psionic_clef_conv1d_seq_silu(input: *const c_void, state: *mut c_void, w: *const c_void, out: *mut c_void, n: i32, channels: i32, k: i32, stream: *mut c_void) -> i32;
    fn psionic_clef_delta_prep(conv: *const c_void, alpha: *const c_void, beta_in: *const c_void, ssm_a: *const c_void, ssm_dt: *const c_void, qn: *mut c_void, kn: *mut c_void, decay: *mut c_void, beta: *mut c_void, kq: *mut c_void, n: i32, key_heads: i32, value_heads: i32, dim: i32, conv_width: i32, stream: *mut c_void) -> i32;
    fn psionic_clef_delta_seq(qn: *const c_void, kn: *const c_void, conv: *const c_void, decay: *const c_void, beta: *const c_void, kq: *const c_void, state: *mut c_void, out: *mut c_void, n: i32, key_heads: i32, value_heads: i32, dim: i32, v_head_reordered: i32, conv_width: i32, value_offset: i32, stream: *mut c_void) -> i32;
    fn psionic_clef_gated_norm_to_f16(o: *const c_void, z: *const c_void, w: *const c_void, out: *mut c_void, n: i32, heads: i32, dim: i32, eps: f32, stream: *mut c_void) -> i32;
    fn psionic_clef_attention_prep(qg: *const c_void, k: *const c_void, v: *const c_void, qw: *const c_void, kw: *const c_void, cos_sin: *const c_void, q16: *mut c_void, gate: *mut c_void, kcache: *mut c_void, vcache: *mut c_void, n: i32, heads: i32, kv_heads: i32, dim: i32, rot: i32, pos0: i32, scale: f32, eps: f32, stream: *mut c_void) -> i32;
    fn psionic_clef_causal_softmax_to_f16(scores: *const c_void, probs: *mut c_void, rows: i32, n: i32, keys: i32, pos0: i32, stream: *mut c_void) -> i32;
    fn psionic_clef_softmax_rows_f32(scores: *mut c_void, rows: i32, keys: i32, scale: f32, stream: *mut c_void) -> i32;
    fn psionic_clef_sigmoid_gate_to_f16(o: *const c_void, gate: *const c_void, out: *mut c_void, count: i64, stream: *mut c_void) -> i32;
    fn psionic_clef_silu_mul_to_f16(gate: *const c_void, up: *const c_void, out: *mut c_void, count: i64, stream: *mut c_void) -> i32;
    fn psionic_clef_stream_create(stream: *mut *mut c_void) -> i32;
    fn psionic_clef_stream_destroy(stream: *mut c_void) -> i32;
    fn psionic_clef_event_create(event: *mut *mut c_void) -> i32;
    fn psionic_clef_event_destroy(event: *mut c_void) -> i32;
    fn psionic_clef_event_record(event: *mut c_void, stream: *mut c_void) -> i32;
    fn psionic_clef_stream_wait_event(stream: *mut c_void, event: *mut c_void) -> i32;
    fn psionic_clef_span_sums(rows: *const c_void, spans: *const c_void, sums: *mut c_void, span_count: i32, d: i32, first: i32, n: i32, stream: *mut c_void) -> i32;
}

/// Element type of a GEMM operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClefElement {
    F32,
    F16,
}

impl ClefElement {
    const fn bytes(self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 => 2,
        }
    }

    const fn cuda_type(self) -> i32 {
        match self {
            Self::F32 => 0,
            Self::F16 => 2,
        }
    }
}

/// A quantized (or dense) weight layout the prefill dequantizes to f16.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClefWeightFormat {
    /// GGML Q8_0 (34-byte blocks of 32).
    Q8_0,
    /// GGML Q4_K (144-byte super-blocks of 256).
    Q4K,
    /// Dense row-major f32.
    F32,
}

impl ClefWeightFormat {
    /// Bytes of `elements` weights in this layout, or `None` when
    /// `elements` is not a whole number of blocks.
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

/// One operand of a raw GEMM: buffer, element offset, element type.
#[derive(Clone, Copy)]
pub struct ClefOperand<'a> {
    pub buffer: &'a CudaBuffer,
    pub offset: usize,
    pub element: ClefElement,
}

impl<'a> ClefOperand<'a> {
    #[must_use]
    pub const fn f32(buffer: &'a CudaBuffer, offset: usize) -> Self {
        Self { buffer, offset, element: ClefElement::F32 }
    }

    #[must_use]
    pub const fn f16(buffer: &'a CudaBuffer, offset: usize) -> Self {
        Self { buffer, offset, element: ClefElement::F16 }
    }

    /// Device pointer after checking that `extent` elements from the offset
    /// fit the buffer.
    fn ptr(&self, extent: usize, what: &str) -> Result<*mut c_void, RuntimeError> {
        span(self.buffer, self.offset, extent, self.element.bytes(), what)
    }
}

fn span(
    buffer: &CudaBuffer,
    offset: usize,
    extent: usize,
    element_bytes: usize,
    what: &str,
) -> Result<*mut c_void, RuntimeError> {
    let end = offset
        .checked_add(extent)
        .and_then(|elements| elements.checked_mul(element_bytes));
    match end {
        Some(end) if end <= buffer.byte_len() => {
            let base = buffer.platform.raw_device_ptr();
            if base.is_null() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda runtime substrate currently requires Linux libcudart",
                )));
            }
            // SAFETY: the range [offset, offset + extent) lies inside the
            // allocation (checked above); only the address is computed.
            Ok(unsafe { base.cast::<u8>().add(offset * element_bytes) }.cast())
        }
        _ => Err(RuntimeError::Backend(format!(
            "clef prefill {what}: {extent} elements at {offset} exceed a {}-byte buffer",
            buffer.byte_len()
        ))),
    }
}

fn f32s<'a>(buffer: &'a CudaBuffer, extent: usize, what: &str) -> Result<*mut c_void, RuntimeError> {
    span(buffer, 0, extent, 4, what)
}

fn f16s<'a>(buffer: &'a CudaBuffer, extent: usize, what: &str) -> Result<*mut c_void, RuntimeError> {
    span(buffer, 0, extent, 2, what)
}

fn dequantize(
    format: ClefWeightFormat,
    source: &CudaBuffer,
    destination: &CudaBuffer,
    elements: usize,
    stream: *mut c_void,
) -> Result<i32, RuntimeError> {
    let bytes = format.byte_len(elements).ok_or_else(|| {
        RuntimeError::Backend(format!(
            "clef dequantize: {elements} elements are not whole {format:?} blocks"
        ))
    })?;
    let src = span(source, 0, bytes, 1, "dequantize source")?;
    let dst = f16s(destination, elements, "dequantize destination")?;
    Ok(unsafe {
        match format {
            ClefWeightFormat::Q8_0 => psionic_clef_dequant_q8_0_f16(src, dst, (elements / 32) as i64, stream),
            ClefWeightFormat::Q4K => psionic_clef_dequant_q4_k_f16(src, dst, (elements / 256) as i64, stream),
            ClefWeightFormat::F32 => psionic_clef_f32_to_f16(src, dst, elements as i64, stream),
        }
    })
}

fn check(code: i32, operation: &str) -> Result<(), RuntimeError> {
    if code == 0 {
        Ok(())
    } else {
        Err(RuntimeError::Backend(format!("{operation} failed with CUDA error {code}")))
    }
}

/// A CUDA stream of the prefill's own (weight dequantization runs here,
/// overlapping the GEMMs on the submission stream).
pub struct ClefStream {
    raw: *mut c_void,
}

impl ClefStream {
    pub fn new() -> Result<Self, RuntimeError> {
        let mut raw = std::ptr::null_mut();
        check(unsafe { psionic_clef_stream_create(&mut raw) }, "cudaStreamCreate")?;
        Ok(Self { raw })
    }

    /// Dequantizes on this stream (see [`CudaSubmission::clef_dequantize_to_f16`]).
    pub fn dequantize_to_f16(
        &self,
        format: ClefWeightFormat,
        source: &CudaBuffer,
        destination: &CudaBuffer,
        elements: usize,
    ) -> Result<(), RuntimeError> {
        check(dequantize(format, source, destination, elements, self.raw)?, "psionic_clef_dequantize")
    }

    pub fn record(&self, event: &ClefEvent) -> Result<(), RuntimeError> {
        check(unsafe { psionic_clef_event_record(event.raw, self.raw) }, "cudaEventRecord")
    }

    pub fn wait(&self, event: &ClefEvent) -> Result<(), RuntimeError> {
        check(unsafe { psionic_clef_stream_wait_event(self.raw, event.raw) }, "cudaStreamWaitEvent")
    }
}

impl Drop for ClefStream {
    fn drop(&mut self) {
        let _ = unsafe { psionic_clef_stream_destroy(self.raw) };
    }
}

/// A CUDA event without timing, for ordering two streams.
pub struct ClefEvent {
    raw: *mut c_void,
}

impl ClefEvent {
    pub fn new() -> Result<Self, RuntimeError> {
        let mut raw = std::ptr::null_mut();
        check(unsafe { psionic_clef_event_create(&mut raw) }, "cudaEventCreate")?;
        Ok(Self { raw })
    }
}

impl Drop for ClefEvent {
    fn drop(&mut self) {
        let _ = unsafe { psionic_clef_event_destroy(self.raw) };
    }
}

fn int(value: usize, what: &str) -> Result<i32, RuntimeError> {
    i32::try_from(value)
        .map_err(|_| RuntimeError::Backend(format!("clef prefill {what} exceeds i32")))
}

impl CudaSubmission {
    fn clef_launch(&mut self, code: i32, operation: &str) -> Result<(), RuntimeError> {
        self.platform.check_raw(code, operation)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Dequantizes `elements` weights (a whole matrix, row-major) to f16.
    pub fn clef_dequantize_to_f16(
        &mut self,
        format: ClefWeightFormat,
        source: &CudaBuffer,
        destination: &CudaBuffer,
        elements: usize,
    ) -> Result<(), RuntimeError> {
        let stream = self.platform.raw_stream()?;
        let code = dequantize(format, source, destination, elements, stream)?;
        self.clef_launch(code, "psionic_clef_dequantize")
    }

    /// Records `event` on this submission's stream.
    pub fn clef_record(&mut self, event: &ClefEvent) -> Result<(), RuntimeError> {
        let stream = self.platform.raw_stream()?;
        check(unsafe { psionic_clef_event_record(event.raw, stream) }, "cudaEventRecord")
    }

    /// Makes this submission's stream wait for `event`.
    pub fn clef_wait(&mut self, event: &ClefEvent) -> Result<(), RuntimeError> {
        let stream = self.platform.raw_stream()?;
        check(unsafe { psionic_clef_stream_wait_event(stream, event.raw) }, "cudaStreamWaitEvent")
    }

    /// `dst (+)= f32(src)` over `count` elements.
    pub fn clef_f16_to_f32(
        &mut self,
        source: &CudaBuffer,
        destination: &CudaBuffer,
        count: usize,
        accumulate: bool,
    ) -> Result<(), RuntimeError> {
        let src = f16s(source, count, "f16 source")?;
        let dst = f32s(destination, count, "f32 destination")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe { psionic_clef_f16_to_f32(src, dst, count as i64, i32::from(accumulate), stream) };
        self.clef_launch(code, "psionic_clef_f16_to_f32")
    }

    /// `out16[r] = RMSNorm(x[r]) * w` for `rows` rows of width `d`.
    pub fn clef_rms_norm_to_f16(
        &mut self,
        x: &CudaBuffer,
        w: &CudaBuffer,
        out: &CudaBuffer,
        rows: usize,
        d: usize,
        eps: f32,
    ) -> Result<(), RuntimeError> {
        let (x, w) = (f32s(x, rows * d, "rms x")?, f32s(w, d, "rms w")?);
        let out = f16s(out, rows * d, "rms out")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_rms_norm_to_f16(x, w, out, int(rows, "rows")?, int(d, "d")?, eps, stream)
        };
        self.clef_launch(code, "psionic_clef_rms_norm_to_f16")
    }

    /// `out[r] = RMSNorm(x[r]) * w` (f32).
    pub fn clef_rms_norm_f32(
        &mut self,
        x: &CudaBuffer,
        w: &CudaBuffer,
        out: &CudaBuffer,
        rows: usize,
        d: usize,
        eps: f32,
    ) -> Result<(), RuntimeError> {
        let (x, w) = (f32s(x, rows * d, "rms x")?, f32s(w, d, "rms w")?);
        let out = f32s(out, rows * d, "rms out")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_rms_norm_f32(x, w, out, int(rows, "rows")?, int(d, "d")?, eps, stream)
        };
        self.clef_launch(code, "psionic_clef_rms_norm_f32")
    }

    /// `out[r] = LayerNorm(x[r]) (* w + b)` (f32).
    pub fn clef_layer_norm_f32(
        &mut self,
        x: &CudaBuffer,
        affine: Option<(&CudaBuffer, &CudaBuffer)>,
        out: &CudaBuffer,
        rows: usize,
        d: usize,
        eps: f32,
    ) -> Result<(), RuntimeError> {
        let x = f32s(x, rows * d, "layer norm x")?;
        let (w, b) = match affine {
            Some((w, b)) => (f32s(w, d, "layer norm w")?, f32s(b, d, "layer norm b")?),
            None => (std::ptr::null_mut(), std::ptr::null_mut()),
        };
        let out = f32s(out, rows * d, "layer norm out")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_layer_norm_f32(x, w, b, out, int(rows, "rows")?, int(d, "d")?, eps, stream)
        };
        self.clef_launch(code, "psionic_clef_layer_norm_f32")
    }

    /// Causal depthwise conv over `n` tokens with carried state
    /// `[channels, k-1]`, then SiLU.
    #[allow(clippy::too_many_arguments)]
    pub fn clef_conv1d_seq_silu(
        &mut self,
        input: &CudaBuffer,
        state: &CudaBuffer,
        weights: &CudaBuffer,
        out: &CudaBuffer,
        n: usize,
        channels: usize,
        kernel: usize,
    ) -> Result<(), RuntimeError> {
        let input = f32s(input, n * channels, "conv input")?;
        let state = f32s(state, channels * kernel.saturating_sub(1), "conv state")?;
        let weights = f32s(weights, channels * kernel, "conv weights")?;
        let out = f32s(out, n * channels, "conv out")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_conv1d_seq_silu(
                input,
                state,
                weights,
                out,
                int(n, "n")?,
                int(channels, "channels")?,
                int(kernel, "kernel")?,
                stream,
            )
        };
        self.clef_launch(code, "psionic_clef_conv1d_seq_silu")
    }

    /// Normalized q/k per key head and decay/beta per value head.
    #[allow(clippy::too_many_arguments)]
    pub fn clef_delta_prep(
        &mut self,
        conv: &CudaBuffer,
        alpha: &CudaBuffer,
        beta_in: &CudaBuffer,
        ssm_a: &CudaBuffer,
        ssm_dt: &CudaBuffer,
        qn: &CudaBuffer,
        kn: &CudaBuffer,
        decay: &CudaBuffer,
        beta: &CudaBuffer,
        key_dot_query: &CudaBuffer,
        n: usize,
        key_heads: usize,
        value_heads: usize,
        dim: usize,
        conv_width: usize,
    ) -> Result<(), RuntimeError> {
        let kq = f32s(key_dot_query, n * key_heads, "k.q")?;
        let conv = f32s(conv, n * conv_width, "delta conv")?;
        let alpha = f32s(alpha, n * value_heads, "delta alpha")?;
        let beta_in = f32s(beta_in, n * value_heads, "delta beta in")?;
        let ssm_a = f32s(ssm_a, value_heads, "ssm_a")?;
        let ssm_dt = f32s(ssm_dt, value_heads, "ssm_dt")?;
        let qn = f32s(qn, n * key_heads * dim, "qn")?;
        let kn = f32s(kn, n * key_heads * dim, "kn")?;
        let decay = f32s(decay, n * value_heads, "decay")?;
        let beta = f32s(beta, n * value_heads, "beta")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_delta_prep(
                conv,
                alpha,
                beta_in,
                ssm_a,
                ssm_dt,
                qn,
                kn,
                decay,
                beta,
                kq,
                int(n, "n")?,
                int(key_heads, "key heads")?,
                int(value_heads, "value heads")?,
                int(dim, "dim")?,
                int(conv_width, "conv width")?,
                stream,
            )
        };
        self.clef_launch(code, "psionic_clef_delta_prep")
    }

    /// The gated delta rule over `n` tokens (state carried in place).
    #[allow(clippy::too_many_arguments)]
    pub fn clef_delta_seq(
        &mut self,
        qn: &CudaBuffer,
        kn: &CudaBuffer,
        conv: &CudaBuffer,
        decay: &CudaBuffer,
        beta: &CudaBuffer,
        key_dot_query: &CudaBuffer,
        state: &CudaBuffer,
        out: &CudaBuffer,
        n: usize,
        key_heads: usize,
        value_heads: usize,
        dim: usize,
        v_head_reordered: bool,
        conv_width: usize,
        value_offset: usize,
    ) -> Result<(), RuntimeError> {
        let qn = f32s(qn, n * key_heads * dim, "qn")?;
        let kn = f32s(kn, n * key_heads * dim, "kn")?;
        let conv = f32s(conv, n * conv_width, "delta conv")?;
        let decay = f32s(decay, n * value_heads, "decay")?;
        let beta = f32s(beta, n * value_heads, "beta")?;
        let kq = f32s(key_dot_query, n * key_heads, "k.q")?;
        let state = f32s(state, value_heads * dim * dim, "delta state")?;
        let out = f32s(out, n * value_heads * dim, "delta out")?;
        if value_offset + value_heads * dim > conv_width {
            return Err(RuntimeError::Backend(String::from(
                "clef delta: value range exceeds the conv width",
            )));
        }
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_delta_seq(
                qn,
                kn,
                conv,
                decay,
                beta,
                kq,
                state,
                out,
                int(n, "n")?,
                int(key_heads, "key heads")?,
                int(value_heads, "value heads")?,
                int(dim, "dim")?,
                i32::from(v_head_reordered),
                int(conv_width, "conv width")?,
                int(value_offset, "value offset")?,
                stream,
            )
        };
        self.clef_launch(code, "psionic_clef_delta_seq")
    }

    /// Per-head `RMSNorm(o) * w * silu(z)` to f16.
    #[allow(clippy::too_many_arguments)]
    pub fn clef_gated_norm_to_f16(
        &mut self,
        o: &CudaBuffer,
        z: &CudaBuffer,
        w: &CudaBuffer,
        out: &CudaBuffer,
        n: usize,
        heads: usize,
        dim: usize,
        eps: f32,
    ) -> Result<(), RuntimeError> {
        let count = n * heads * dim;
        let o = f32s(o, count, "gated norm o")?;
        let z = f32s(z, count, "gated norm z")?;
        let w = f32s(w, dim, "gated norm w")?;
        let out = f16s(out, count, "gated norm out")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_gated_norm_to_f16(
                o,
                z,
                w,
                out,
                int(n, "n")?,
                int(heads, "heads")?,
                int(dim, "dim")?,
                eps,
                stream,
            )
        };
        self.clef_launch(code, "psionic_clef_gated_norm_to_f16")
    }

    /// Full-attention input prep: split q/gate, q/k RMSNorm, rotary, q
    /// scale; k and v appended to the f16 caches at `pos0`.
    #[allow(clippy::too_many_arguments)]
    pub fn clef_attention_prep(
        &mut self,
        query_gate: &CudaBuffer,
        key: &CudaBuffer,
        value: &CudaBuffer,
        query_norm: &CudaBuffer,
        key_norm: &CudaBuffer,
        cos_sin: &CudaBuffer,
        query16: &CudaBuffer,
        gate: &CudaBuffer,
        key_cache: &CudaBuffer,
        value_cache: &CudaBuffer,
        n: usize,
        heads: usize,
        kv_heads: usize,
        dim: usize,
        rotary: usize,
        pos0: usize,
        scale: f32,
        eps: f32,
    ) -> Result<(), RuntimeError> {
        let qg = f32s(query_gate, n * heads * dim * 2, "query/gate")?;
        let k = f32s(key, n * kv_heads * dim, "key")?;
        let v = f32s(value, n * kv_heads * dim, "value")?;
        let qw = f32s(query_norm, dim, "query norm")?;
        let kw = f32s(key_norm, dim, "key norm")?;
        let cs = f32s(cos_sin, n * rotary, "cos/sin")?;
        let q16 = f16s(query16, n * heads * dim, "query16")?;
        let gate = f32s(gate, n * heads * dim, "gate")?;
        let kc = f16s(key_cache, (pos0 + n) * kv_heads * dim, "key cache")?;
        let vc = f16s(value_cache, (pos0 + n) * kv_heads * dim, "value cache")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_attention_prep(
                qg,
                k,
                v,
                qw,
                kw,
                cs,
                q16,
                gate,
                kc,
                vc,
                int(n, "n")?,
                int(heads, "heads")?,
                int(kv_heads, "kv heads")?,
                int(dim, "dim")?,
                int(rotary, "rotary")?,
                int(pos0, "pos0")?,
                scale,
                eps,
                stream,
            )
        };
        self.clef_launch(code, "psionic_clef_attention_prep")
    }

    /// Causal softmax of `rows` score rows (row `r` is query `r % n`) to
    /// f16 probabilities.
    pub fn clef_causal_softmax_to_f16(
        &mut self,
        scores: &CudaBuffer,
        probs: &CudaBuffer,
        rows: usize,
        n: usize,
        keys: usize,
        pos0: usize,
    ) -> Result<(), RuntimeError> {
        if pos0 + n > keys {
            return Err(RuntimeError::Backend(String::from(
                "clef causal softmax: queries extend past the keys",
            )));
        }
        let scores = f32s(scores, rows * keys, "scores")?;
        let probs = f16s(probs, rows * keys, "probs")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_causal_softmax_to_f16(
                scores,
                probs,
                int(rows, "rows")?,
                int(n, "n")?,
                int(keys, "keys")?,
                int(pos0, "pos0")?,
                stream,
            )
        };
        self.clef_launch(code, "psionic_clef_causal_softmax_to_f16")
    }

    /// In-place softmax of `rows` full rows after multiplying by `scale`.
    pub fn clef_softmax_rows_f32(
        &mut self,
        scores: &CudaBuffer,
        rows: usize,
        keys: usize,
        scale: f32,
    ) -> Result<(), RuntimeError> {
        let scores = f32s(scores, rows * keys, "scores")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_softmax_rows_f32(scores, int(rows, "rows")?, int(keys, "keys")?, scale, stream)
        };
        self.clef_launch(code, "psionic_clef_softmax_rows_f32")
    }

    /// `out16 = o * sigmoid(gate)`.
    pub fn clef_sigmoid_gate_to_f16(
        &mut self,
        o: &CudaBuffer,
        gate: &CudaBuffer,
        out: &CudaBuffer,
        count: usize,
    ) -> Result<(), RuntimeError> {
        let (o, gate) = (f32s(o, count, "o")?, f32s(gate, count, "gate")?);
        let out = f16s(out, count, "gated out")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe { psionic_clef_sigmoid_gate_to_f16(o, gate, out, count as i64, stream) };
        self.clef_launch(code, "psionic_clef_sigmoid_gate_to_f16")
    }

    /// `out16 = silu(gate) * up`.
    pub fn clef_silu_mul_to_f16(
        &mut self,
        gate: &CudaBuffer,
        up: &CudaBuffer,
        out: &CudaBuffer,
        count: usize,
    ) -> Result<(), RuntimeError> {
        let (gate, up) = (f32s(gate, count, "ffn gate")?, f32s(up, count, "ffn up")?);
        let out = f16s(out, count, "ffn out")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe { psionic_clef_silu_mul_to_f16(gate, up, out, count as i64, stream) };
        self.clef_launch(code, "psionic_clef_silu_mul_to_f16")
    }

    /// Adds rows `[first, first + n)` of `rows` into the span sums
    /// (`spans` holds `[start, end)` pairs as i32).
    #[allow(clippy::too_many_arguments)]
    pub fn clef_span_sums(
        &mut self,
        rows: &CudaBuffer,
        spans: &CudaBuffer,
        sums: &CudaBuffer,
        span_count: usize,
        d: usize,
        first: usize,
        n: usize,
    ) -> Result<(), RuntimeError> {
        if span_count == 0 {
            return Ok(());
        }
        let rows = f32s(rows, n * d, "span rows")?;
        let spans = span(spans, 0, span_count * 2, 4, "spans")?;
        let sums = f32s(sums, span_count * d, "span sums")?;
        let stream = self.platform.raw_stream()?;
        let code = unsafe {
            psionic_clef_span_sums(
                rows,
                spans,
                sums,
                int(span_count, "span count")?,
                int(d, "d")?,
                int(first, "first")?,
                int(n, "n")?,
                stream,
            )
        };
        self.clef_launch(code, "psionic_clef_span_sums")
    }

    /// Row-major linear layer: `out[n, m] (+)= x[n, k] · W[m, k]^T`, with
    /// `x` and `W` of the same element type and `out` f32.
    /// `accumulate` adds into `out` (the residual stream);
    /// `compute_16f` uses f16 accumulation (f16 operands only).
    #[allow(clippy::too_many_arguments)]
    pub fn clef_linear(
        &mut self,
        x: ClefOperand<'_>,
        w: ClefOperand<'_>,
        out: ClefOperand<'_>,
        n: usize,
        m: usize,
        k: usize,
        accumulate: bool,
        compute_16f: bool,
    ) -> Result<(), RuntimeError> {
        if n == 0 {
            return Ok(());
        }
        let xp = x.ptr(n * k, "linear x")?;
        let wp = w.ptr(m * k, "linear w")?;
        let op = out.ptr(n * m, "linear out")?;
        if compute_16f && out.element != ClefElement::F16 {
            return Err(RuntimeError::Backend(String::from(
                "clef linear: f16 accumulation needs an f16 output",
            )));
        }
        self.platform.gemm_ex_raw(
            true,
            false,
            m,
            n,
            k,
            1.0,
            wp,
            w.element.cuda_type(),
            k,
            xp,
            x.element.cuda_type(),
            k,
            if accumulate { 1.0 } else { 0.0 },
            op,
            out.element.cuda_type(),
            m,
            compute_16f,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Raw column-major strided-batched GEMM (f32 accumulate) over operands
    /// with element offsets; `extent_*` are the element counts each operand
    /// spans across the whole batch (checked against the buffers).
    #[allow(clippy::too_many_arguments)]
    pub fn clef_gemm_strided_batched(
        &mut self,
        transpose_a: bool,
        transpose_b: bool,
        m: usize,
        n: usize,
        k: usize,
        a: ClefOperand<'_>,
        lda: usize,
        stride_a: usize,
        extent_a: usize,
        b: ClefOperand<'_>,
        ldb: usize,
        stride_b: usize,
        extent_b: usize,
        c: ClefOperand<'_>,
        ldc: usize,
        stride_c: usize,
        extent_c: usize,
        batch: usize,
    ) -> Result<(), RuntimeError> {
        if batch == 0 || m == 0 || n == 0 {
            return Ok(());
        }
        let ap = a.ptr(extent_a, "batched a")?;
        let bp = b.ptr(extent_b, "batched b")?;
        let cp = c.ptr(extent_c, "batched c")?;
        let stride = |value: usize| {
            i64::try_from(value)
                .map_err(|_| RuntimeError::Backend(String::from("clef batched stride exceeds i64")))
        };
        self.platform.gemm_strided_batched_ex_raw(
            transpose_a,
            transpose_b,
            m,
            n,
            k,
            ap,
            a.element.cuda_type(),
            lda,
            stride(stride_a)?,
            bp,
            b.element.cuda_type(),
            ldb,
            stride(stride_b)?,
            cp,
            c.element.cuda_type(),
            ldc,
            stride(stride_c)?,
            batch,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }
}
