//! DeepSeek-V4.1 hyper-connection mixing, Engram gather and gating, and
//! expert routing kernels (#12393).
//!
//! These are the per-token steps around the block-scaled matrix-vector
//! products of [`crate::v41_matvec`]:
//!
//! - Hyper-connections: [`CudaSubmission::v41_hc_mixes`] (the joint RMS
//!   projection of the four streams, the pre and post coefficients, and the
//!   Sinkhorn-normalized residual mix), [`CudaSubmission::v41_hc_collapse`],
//!   and [`CudaSubmission::v41_hc_expand`].
//! - Engram: [`CudaSubmission::v41_engram_gather`] reads the requested FP8
//!   table rows straight from page-locked host memory, and
//!   [`CudaSubmission::v41_engram_gate`] applies the per-stream gate.
//! - Routing and experts: [`CudaSubmission::v41_gate_logits`],
//!   [`CudaSubmission::v41_route`] (writes the selected expert ids where
//!   [`crate::v41_matvec::V41Slots`] reads them),
//!   [`CudaSubmission::v41_swiglu`], and
//!   [`CudaSubmission::v41_moe_combine`].
//!
//! Every call only enqueues work on the submission's stream: none allocates
//! or synchronizes, so W7 can record them into a CUDA graph. Callers own every
//! buffer, including the mixing projection's scratch.
//!
//! Each kernel repeats the CPU path's operations (`deepseek_v41_cpu`) in the
//! same order wherever that keeps it parallel: element-wise steps, the
//! Sinkhorn normalization, the four-term stream sums, the top-k rule, and the
//! expert sums. Long reductions use a fixed parallel order instead: a thread
//! adds every 256th product in order, a shuffle-down tree adds the 32 lanes of
//! a warp, and thread 0 adds the eight warp totals in order. The mixing
//! projection splits K into [`V41_HC_SPLITS`] slices and adds the slice totals
//! in order. `exp` and `log1p` run in double precision and are rounded to
//! float once. The `*_host` functions repeat all of this on the CPU and
//! reproduce the device bit for bit. The only exception is a double-precision
//! `exp` or `log1p` whose result falls within one double rounding error of a
//! float rounding boundary, about one evaluation in 2^28.

use crate::clef_prefill::{int, span};
use crate::v41_matvec::{V41Output, v41_round_bf16};
use crate::{CudaBuffer, CudaHostBuffer, CudaSubmission};
use psionic_runtime::RuntimeError;
use std::ffi::c_void;

/// K slices of the mixing projection, one block each.
pub const V41_HC_SPLITS: usize = 16;
/// Threads in a reduction block.
pub const V41_REDUCE_THREADS: usize = 256;
/// Experts one routing launch can rank.
pub const V41_ROUTE_MAX_EXPERTS: usize = 1024;
/// Largest top-k one routing launch can select.
pub const V41_ROUTE_MAX_TOP_K: usize = 32;

unsafe extern "C" {
    fn psionic_v41_hc_mixes(
        residual: *const c_void,
        weights: *const c_void,
        scale: *const c_void,
        base: *const c_void,
        scratch: *mut c_void,
        out: *mut c_void,
        streams: i32,
        width: i32,
        norm_epsilon: f32,
        mix_epsilon: f32,
        iterations: i32,
        tokens: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_hc_split(
        projected: *const c_void,
        scale: *const c_void,
        base: *const c_void,
        out: *mut c_void,
        streams: i32,
        mix_epsilon: f32,
        iterations: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_hc_collapse(
        residual: *const c_void,
        mixes: *const c_void,
        out: *mut c_void,
        streams: i32,
        width: i32,
        round_bf16: i32,
        tokens: i32,
        mixes_stride: i64,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_hc_expand(
        output: *const c_void,
        residual: *const c_void,
        mixes: *const c_void,
        out: *mut c_void,
        streams: i32,
        width: i32,
        round_bf16: i32,
        tokens: i32,
        mixes_stride: i64,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_engram_gather(
        codes: *const c_void,
        scales: *const c_void,
        rows: i64,
        ids: *const c_void,
        out: *mut c_void,
        count: i32,
        width: i32,
        round_bf16: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_engram_gate(
        residual: *const c_void,
        projected: *const c_void,
        q_weight: *const c_void,
        k_weight: *const c_void,
        out: *mut c_void,
        streams: i32,
        width: i32,
        epsilon: f32,
        inv_sqrt_width: f32,
        round_bf16: i32,
        tokens: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_gate_logits(
        weight: *const c_void,
        input: *const c_void,
        logits: *mut c_void,
        experts: i32,
        cols: i32,
        tokens: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_route(
        logits: *const c_void,
        bias: *const c_void,
        ids: *mut c_void,
        weights: *mut c_void,
        experts: i32,
        top_k: i32,
        temperature: f32,
        normalize: i32,
        route_scale: f32,
        tokens: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_swiglu(
        gate: *const c_void,
        up: *const c_void,
        route_weights: *const c_void,
        out: *mut c_void,
        slots: i32,
        width: i32,
        limit: f32,
        round_bf16: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_v41_moe_combine(
        routed: *const c_void,
        shared: *const c_void,
        out: *mut c_void,
        slots: i32,
        width: i32,
        round_bf16: i32,
        tokens: i32,
        stream: *mut c_void,
    ) -> i32;
}

fn invalid(message: String) -> RuntimeError {
    RuntimeError::Backend(format!("v41 mixing kernels: {message}"))
}

fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn rounds(output: V41Output) -> i32 {
    i32::from(output == V41Output::Bf16)
}

// ---------------------------------------------------------------------------
// Hyper-connections
// ---------------------------------------------------------------------------

/// Hyper-connection geometry and constants, as
/// `deepseek_v41_cpu::hyperconnections::HyperConnections`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V41HyperConfig {
    /// Residual streams, 1 to 4 (the mixes must fit one warp).
    pub streams: usize,
    /// Values per stream.
    pub width: usize,
    /// RMS epsilon of the joint projection.
    pub norm_epsilon: f32,
    /// Epsilon added to the pre coefficients and inside the Sinkhorn sums.
    pub mix_epsilon: f32,
    /// Sinkhorn iterations; the first normalizes columns only.
    pub sinkhorn_iterations: usize,
}

impl V41HyperConfig {
    /// DeepSeek-V4.1-Flash: 4 streams of 5,120, epsilons 1e-20 and 1e-6, 20
    /// iterations.
    #[must_use]
    pub const fn flash() -> Self {
        Self {
            streams: 4,
            width: 5120,
            norm_epsilon: 1e-20,
            mix_epsilon: 1e-6,
            sinkhorn_iterations: 20,
        }
    }

    /// Mix values: `streams` pre, `streams` post, then `streams²` comb
    /// entries row-major `[source, target]`.
    #[must_use]
    pub const fn mixes(self) -> usize {
        (self.streams + 2) * self.streams
    }

    /// Residual values, `streams × width`.
    #[must_use]
    pub const fn elements(self) -> usize {
        self.streams * self.width
    }

    /// Floats of scratch [`CudaSubmission::v41_hc_mixes`] needs: the slice
    /// partials, then the `mixes()` projected values (`dot × 1 / rms`), which
    /// stay readable after the call.
    #[must_use]
    pub const fn scratch_floats(self) -> usize {
        (self.mixes() + 1) * V41_HC_SPLITS + self.mixes()
    }

    fn check(self) -> Result<(), RuntimeError> {
        if !(1..=4).contains(&self.streams)
            || self.width == 0
            || self.sinkhorn_iterations == 0
            || !positive(self.norm_epsilon)
            || !positive(self.mix_epsilon)
            || i32::try_from(self.elements()).is_err()
            || i32::try_from(self.sinkhorn_iterations).is_err()
        {
            return Err(invalid(format!("hyper-connection configuration {self:?}")));
        }
        Ok(())
    }
}

/// One sub-block's mixing parameters on the device, all float32:
/// `hc_{attn,ffn}_fn` `[mixes, streams × width]`, `hc_*_scale` `[3]`, and
/// `hc_*_base` `[mixes]`.
#[derive(Clone, Copy, Debug)]
pub struct V41HcWeights<'a> {
    pub projection: &'a CudaBuffer,
    pub scale: &'a CudaBuffer,
    pub base: &'a CudaBuffer,
}

impl CudaSubmission {
    /// Computes one sub-block's mixes from its input residual
    /// (`streams × width` floats) into `mixes` (`hc.mixes()` floats), as
    /// `HyperConnections::mixes`: the residual's joint RMS, the projection,
    /// then [`Self::v41_hc_split_sinkhorn`]'s steps. Two launches.
    ///
    /// A nonfinite logit or variance writes NaN to every mix value; the CPU
    /// path refuses such input instead.
    ///
    /// # Errors
    ///
    /// Refuses a bad configuration and buffers too small for it.
    pub fn v41_hc_mixes(
        &mut self,
        hc: V41HyperConfig,
        weights: V41HcWeights<'_>,
        residual: &CudaBuffer,
        scratch: &CudaBuffer,
        mixes: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.v41_hc_mixes_tokens(hc, weights, residual, scratch, mixes, 1)
    }

    /// [`Self::v41_hc_mixes`] for `tokens` tokens at once (#12464): token
    /// `t`'s residual, scratch, and mixes follow token `t - 1`'s.
    ///
    /// # Errors
    ///
    /// As [`Self::v41_hc_mixes`].
    pub fn v41_hc_mixes_tokens(
        &mut self,
        hc: V41HyperConfig,
        weights: V41HcWeights<'_>,
        residual: &CudaBuffer,
        scratch: &CudaBuffer,
        mixes: &CudaBuffer,
        tokens: usize,
    ) -> Result<(), RuntimeError> {
        hc.check()?;
        let m = hc.mixes();
        let residual = span(residual, 0, hc.elements() * tokens, 4, "residual")?;
        let projection = span(weights.projection, 0, m * hc.elements(), 4, "projection")?;
        let scale = span(weights.scale, 0, 3, 4, "scale")?;
        let base = span(weights.base, 0, m, 4, "base")?;
        let scratch = span(scratch, 0, hc.scratch_floats() * tokens, 4, "scratch")?;
        let out = span(mixes, 0, m * tokens, 4, "mixes")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_hc_mixes(
                residual,
                projection,
                scale,
                base,
                scratch,
                out,
                int(hc.streams, "streams")?,
                int(hc.width, "width")?,
                hc.norm_epsilon,
                hc.mix_epsilon,
                int(hc.sinkhorn_iterations, "iterations")?,
                int(tokens, "tokens")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 hyper-connection mixes")
    }

    /// The mixes from projected values (`hc.mixes()` floats, already times
    /// 1 / rms), as `HyperConnections::split_sinkhorn`: logits
    /// `projected × scale + base`; `pre = sigmoid + eps`; `post = 2 sigmoid`;
    /// comb is a row softmax plus eps, then the Sinkhorn iterations.
    ///
    /// # Errors
    ///
    /// Refuses a bad configuration and buffers too small for it.
    pub fn v41_hc_split_sinkhorn(
        &mut self,
        hc: V41HyperConfig,
        projected: &CudaBuffer,
        scale: &CudaBuffer,
        base: &CudaBuffer,
        mixes: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        hc.check()?;
        let m = hc.mixes();
        let projected = span(projected, 0, m, 4, "projected")?;
        let scale = span(scale, 0, 3, 4, "scale")?;
        let base = span(base, 0, m, 4, "base")?;
        let out = span(mixes, 0, m, 4, "mixes")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_hc_split(
                projected,
                scale,
                base,
                out,
                int(hc.streams, "streams")?,
                hc.mix_epsilon,
                int(hc.sinkhorn_iterations, "iterations")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 hyper-connection Sinkhorn")
    }

    /// Collapses the streams into one sub-block input (`width` floats):
    /// `out[d] = sum_j pre[j] × residual[j, d]`, added in stream order. `pre`
    /// is the first `streams` floats of `previous_mixes`, the mixes of the
    /// previous sub-block, as the reference uses them.
    ///
    /// # Errors
    ///
    /// Refuses a bad configuration and buffers too small for it.
    pub fn v41_hc_collapse(
        &mut self,
        hc: V41HyperConfig,
        residual: &CudaBuffer,
        previous_mixes: &CudaBuffer,
        out: &CudaBuffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        self.v41_hc_collapse_tokens(hc, residual, previous_mixes, out, output, 1, 0)
    }

    /// [`Self::v41_hc_collapse`] for `tokens` tokens, with each token's
    /// previous mixes `mixes_stride` floats after the last.
    ///
    /// # Errors
    ///
    /// As [`Self::v41_hc_collapse`].
    #[allow(clippy::too_many_arguments)]
    pub fn v41_hc_collapse_tokens(
        &mut self,
        hc: V41HyperConfig,
        residual: &CudaBuffer,
        previous_mixes: &CudaBuffer,
        out: &CudaBuffer,
        output: V41Output,
        tokens: usize,
        mixes_stride: usize,
    ) -> Result<(), RuntimeError> {
        hc.check()?;
        let t = tokens.max(1);
        let residual = span(residual, 0, hc.elements() * t, 4, "residual")?;
        let mixes = span(
            previous_mixes,
            0,
            (t - 1) * mixes_stride + hc.streams,
            4,
            "previous mixes",
        )?;
        let out = span(out, 0, hc.width * t, 4, "collapsed output")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_hc_collapse(
                residual,
                mixes,
                out,
                int(hc.streams, "streams")?,
                int(hc.width, "width")?,
                rounds(output),
                int(t, "tokens")?,
                i64::try_from(mixes_stride).map_err(|_| invalid(String::from("stride")))?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 hyper-connection collapse")
    }

    /// Expands a sub-block's output (`width` floats) back into the streams
    /// (`streams × width` floats):
    /// `out[k, d] = post[k] × output[d] + sum_j comb[j, k] × residual[j, d]`,
    /// with `post` and `comb` from this sub-block's `mixes`.
    ///
    /// # Errors
    ///
    /// Refuses a bad configuration and buffers too small for it.
    pub fn v41_hc_expand(
        &mut self,
        hc: V41HyperConfig,
        sublayer_output: &CudaBuffer,
        residual: &CudaBuffer,
        mixes: &CudaBuffer,
        out: &CudaBuffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        self.v41_hc_expand_tokens(hc, sublayer_output, residual, mixes, out, output, 1, 0)
    }

    /// [`Self::v41_hc_expand`] for `tokens` tokens, with each token's mixes
    /// `mixes_stride` floats after the last.
    ///
    /// # Errors
    ///
    /// As [`Self::v41_hc_expand`].
    #[allow(clippy::too_many_arguments)]
    pub fn v41_hc_expand_tokens(
        &mut self,
        hc: V41HyperConfig,
        sublayer_output: &CudaBuffer,
        residual: &CudaBuffer,
        mixes: &CudaBuffer,
        out: &CudaBuffer,
        output: V41Output,
        tokens: usize,
        mixes_stride: usize,
    ) -> Result<(), RuntimeError> {
        hc.check()?;
        let t = tokens.max(1);
        let sublayer = span(sublayer_output, 0, hc.width * t, 4, "sub-block output")?;
        let residual = span(residual, 0, hc.elements() * t, 4, "residual")?;
        let mixes = span(mixes, 0, (t - 1) * mixes_stride + hc.mixes(), 4, "mixes")?;
        let out = span(out, 0, hc.elements() * t, 4, "expanded output")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_hc_expand(
                sublayer,
                residual,
                mixes,
                out,
                int(hc.streams, "streams")?,
                int(hc.width, "width")?,
                rounds(output),
                int(t, "tokens")?,
                i64::try_from(mixes_stride).map_err(|_| invalid(String::from("stride")))?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 hyper-connection expand")
    }
}

// ---------------------------------------------------------------------------
// Engram
// ---------------------------------------------------------------------------

/// An Engram embedding table in page-locked host memory, in its checkpoint
/// form: `F8_E4M3` codes `[rows, width]` and `F8_E8M0` scales
/// `[rows, width / 32]`. The tables hold about 101 GB each, so they stay in
/// host RAM and the device reads only the rows it's asked for.
#[derive(Clone, Copy, Debug)]
pub struct V41EngramTable<'a> {
    pub codes: &'a CudaHostBuffer,
    pub scales: &'a CudaHostBuffer,
    pub rows: u64,
    /// Values per row, a multiple of 32 (256 in the checkpoint).
    pub width: usize,
}

/// The Engram gate's geometry, as `deepseek_v41_cpu::engram::EngramGate`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V41EngramGate {
    pub streams: usize,
    pub width: usize,
    pub epsilon: f32,
}

impl V41EngramGate {
    /// DeepSeek-V4.1-Flash: 4 streams of 5,120, epsilon 1e-20.
    #[must_use]
    pub const fn flash() -> Self {
        Self {
            streams: 4,
            width: 5120,
            epsilon: 1e-20,
        }
    }

    /// `width^-0.5` as the CPU path computes it: in f64, then rounded.
    #[must_use]
    pub fn inv_sqrt_width(self) -> f32 {
        (self.width as f64).powf(-0.5) as f32
    }
}

fn host_span(
    buffer: &CudaHostBuffer,
    bytes: u64,
    what: &str,
) -> Result<*const c_void, RuntimeError> {
    let base = buffer.raw_host_ptr();
    if base.is_null() {
        return Err(invalid(String::from(
            "host tables need the Linux CUDA runtime",
        )));
    }
    if usize::try_from(bytes).map_or(true, |bytes| bytes > buffer.byte_len()) {
        return Err(invalid(format!(
            "{what}: {bytes} bytes exceed a {}-byte host buffer",
            buffer.byte_len()
        )));
    }
    Ok(base.cast_const())
}

impl CudaSubmission {
    /// Gathers `count` table rows by the device `i64` row ids in `ids` into
    /// `out` (`count × width` floats), dequantized as the CPU loader does:
    /// each E4M3 value times its group's UE8M0 scale. One warp reads each
    /// row from host memory over the bus, so only requested rows cross it.
    /// An id outside the table writes NaN for that row.
    ///
    /// `V41Output::Bf16` rounds as the reference's BF16 lookup does (exact
    /// for normal scales).
    ///
    /// # Errors
    ///
    /// Refuses a width that isn't a nonzero multiple of 32, `count` outside
    /// 1 to 65,535, and buffers too small for them.
    pub fn v41_engram_gather(
        &mut self,
        table: V41EngramTable<'_>,
        ids: &CudaBuffer,
        count: usize,
        out: &CudaBuffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        if table.width == 0 || !table.width.is_multiple_of(32) || !(1..=65_535).contains(&count) {
            return Err(invalid(format!(
                "Engram width {} with {count} rows",
                table.width
            )));
        }
        let rows =
            i64::try_from(table.rows).map_err(|_| invalid(format!("{} table rows", table.rows)))?;
        let width = table.width as u64;
        let code_bytes = table
            .rows
            .checked_mul(width)
            .ok_or_else(|| invalid(String::from("table size overflow")))?;
        let codes = host_span(table.codes, code_bytes, "table codes")?;
        let scales = host_span(table.scales, table.rows * (width / 32), "table scales")?;
        let ids = span(ids, 0, count, 8, "row ids")?;
        let out = span(out, 0, count * table.width, 4, "gathered rows")?;
        // SAFETY: the table extents were checked against the host buffers,
        // the device refuses ids outside them, and the other extents were
        // checked against their buffers.
        let code = unsafe {
            psionic_v41_engram_gather(
                codes,
                scales,
                rows,
                ids,
                out,
                int(count, "count")?,
                int(table.width, "width")?,
                rounds(output),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 Engram gather")
    }

    /// Applies the Engram gate, as `EngramGate::apply`: for each stream `h`,
    /// `dot = sum(x × (q × k) × key) × rstd(x) × rstd(key) × width^-0.5`,
    /// `gate = sigmoid(sign(dot) × sqrt(max(|dot|, 1e-6)))`, and
    /// `out[h] = x[h] + gate × value`. `projected` holds the keys
    /// (`streams × width`) followed by the value (`width`); `q_weight` and
    /// `k_weight` are `streams × width`. A nonfinite sum or dot writes NaN to
    /// that stream.
    ///
    /// # Errors
    ///
    /// Refuses a bad geometry and buffers too small for it.
    pub fn v41_engram_gate(
        &mut self,
        gate: V41EngramGate,
        residual: &CudaBuffer,
        projected: &CudaBuffer,
        q_weight: &CudaBuffer,
        k_weight: &CudaBuffer,
        out: &CudaBuffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        self.v41_engram_gate_tokens(
            gate, residual, projected, q_weight, k_weight, out, output, 1,
        )
    }

    /// [`Self::v41_engram_gate`] for `tokens` tokens one after another.
    ///
    /// # Errors
    ///
    /// As [`Self::v41_engram_gate`].
    #[allow(clippy::too_many_arguments)]
    pub fn v41_engram_gate_tokens(
        &mut self,
        gate: V41EngramGate,
        residual: &CudaBuffer,
        projected: &CudaBuffer,
        q_weight: &CudaBuffer,
        k_weight: &CudaBuffer,
        out: &CudaBuffer,
        output: V41Output,
        tokens: usize,
    ) -> Result<(), RuntimeError> {
        let t = tokens.max(1);
        let elements = gate
            .streams
            .checked_mul(gate.width)
            .filter(|n| *n > 0 && i32::try_from(*n + gate.width).is_ok())
            .ok_or_else(|| invalid(format!("Engram gate {gate:?}")))?;
        if gate.streams > 65_535 || !positive(gate.epsilon) {
            return Err(invalid(format!("Engram gate {gate:?}")));
        }
        let residual = span(residual, 0, elements * t, 4, "residual")?;
        let projected = span(
            projected,
            0,
            (elements + gate.width) * t,
            4,
            "keys and value",
        )?;
        let q_weight = span(q_weight, 0, elements, 4, "query weight")?;
        let k_weight = span(k_weight, 0, elements, 4, "key weight")?;
        let out = span(out, 0, elements * t, 4, "gated residual")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_engram_gate(
                residual,
                projected,
                q_weight,
                k_weight,
                out,
                int(gate.streams, "streams")?,
                int(gate.width, "width")?,
                gate.epsilon,
                gate.inv_sqrt_width(),
                rounds(output),
                int(t, "tokens")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 Engram gate")
    }
}

// ---------------------------------------------------------------------------
// Routing and experts
// ---------------------------------------------------------------------------

/// The router, as `deepseek_v41_cpu::moe::Router` plus the expert count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct V41Router {
    /// Routed experts, 1 to 1,024.
    pub experts: usize,
    /// Experts selected per token, 1 to 32 and at most `experts`.
    pub top_k: usize,
    pub temperature: f32,
    /// Divide the selected scores by their sum when `top_k > 1`.
    pub normalize: bool,
    pub route_scale: f32,
}

impl V41Router {
    /// DeepSeek-V4.1-Flash: top 6 of 384, temperature 1, normalized, scale
    /// 1.5.
    #[must_use]
    pub const fn flash() -> Self {
        Self {
            experts: 384,
            top_k: 6,
            temperature: 1.0,
            normalize: true,
            route_scale: 1.5,
        }
    }

    fn check(self) -> Result<(), RuntimeError> {
        if !(1..=V41_ROUTE_MAX_EXPERTS).contains(&self.experts)
            || !(1..=V41_ROUTE_MAX_TOP_K.min(self.experts)).contains(&self.top_k)
            || !positive(self.temperature)
            || !positive(self.route_scale)
        {
            return Err(invalid(format!("router {self:?}")));
        }
        Ok(())
    }
}

impl CudaSubmission {
    /// Router logits: the BF16 gate weight (`experts × cols` little-endian
    /// BF16, 16-byte aligned rows) times the float32 input (`cols` floats),
    /// with float32 products and sums, as `Gate.forward` promotes both. One
    /// warp per expert: lane `l` adds the products of 8-value chunks
    /// `l, l + 32, ...` in order, then a shuffle-down tree adds the lanes.
    ///
    /// # Errors
    ///
    /// Refuses `cols` that isn't a nonzero multiple of 8 and buffers too
    /// small for the shape.
    pub fn v41_gate_logits(
        &mut self,
        weight: &CudaBuffer,
        input: &CudaBuffer,
        logits: &CudaBuffer,
        experts: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        self.v41_gate_logits_tokens(weight, input, logits, experts, cols, 1)
    }

    /// [`Self::v41_gate_logits`] for `tokens` inputs one after another.
    ///
    /// # Errors
    ///
    /// As [`Self::v41_gate_logits`].
    pub fn v41_gate_logits_tokens(
        &mut self,
        weight: &CudaBuffer,
        input: &CudaBuffer,
        logits: &CudaBuffer,
        experts: usize,
        cols: usize,
        tokens: usize,
    ) -> Result<(), RuntimeError> {
        let t = tokens.max(1);
        if experts == 0 || cols == 0 || !cols.is_multiple_of(8) {
            return Err(invalid(format!("gate {experts}x{cols}")));
        }
        let elements = experts
            .checked_mul(cols)
            .filter(|n| i32::try_from(*n).is_ok())
            .ok_or_else(|| invalid(format!("gate {experts}x{cols}")))?;
        let weight = span(weight, 0, elements, 2, "gate weight")?;
        let input = span(input, 0, cols * t, 4, "gate input")?;
        let logits = span(logits, 0, experts * t, 4, "logits")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_gate_logits(
                weight,
                input,
                logits,
                int(experts, "experts")?,
                int(cols, "cols")?,
                int(t, "tokens")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 gate logits")
    }

    /// Selects experts, as `Router::select`: `score = sqrt(softplus(logit /
    /// temperature))` (the logit itself above 20, as PyTorch's softplus),
    /// ranked by `score + bias`, top-k with equal values going to the lower
    /// id. Weights are the unbiased scores, divided by their sum plus 1e-20
    /// (added in rank order) when normalizing, times `route_scale`.
    ///
    /// Writes `top_k` device `i32` ids to `ids` in ascending id order, the
    /// order the reference adds expert outputs, and their weights to
    /// `weights` in the same order. The ids are the
    /// [`crate::v41_matvec::V41Slots::ids`] of the expert launches. A
    /// nonfinite logit or biased score writes id -1 and a NaN weight to every
    /// slot, so the expert launches write NaN.
    ///
    /// # Errors
    ///
    /// Refuses a bad router and buffers too small for it.
    pub fn v41_route(
        &mut self,
        router: V41Router,
        logits: &CudaBuffer,
        bias: &CudaBuffer,
        ids: &CudaBuffer,
        weights: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.v41_route_tokens(router, logits, bias, ids, weights, 1)
    }

    /// [`Self::v41_route`] for `tokens` tokens one after another.
    ///
    /// # Errors
    ///
    /// As [`Self::v41_route`].
    pub fn v41_route_tokens(
        &mut self,
        router: V41Router,
        logits: &CudaBuffer,
        bias: &CudaBuffer,
        ids: &CudaBuffer,
        weights: &CudaBuffer,
        tokens: usize,
    ) -> Result<(), RuntimeError> {
        router.check()?;
        let t = tokens.max(1);
        let logits = span(logits, 0, router.experts * t, 4, "logits")?;
        let bias = span(bias, 0, router.experts, 4, "bias")?;
        let ids = span(ids, 0, router.top_k * t, 4, "expert ids")?;
        let weights = span(weights, 0, router.top_k * t, 4, "route weights")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_route(
                logits,
                bias,
                ids,
                weights,
                int(router.experts, "experts")?,
                int(router.top_k, "top k")?,
                router.temperature,
                i32::from(router.normalize),
                router.route_scale,
                int(t, "tokens")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 routing")
    }

    /// Clamped SwiGLU between the expert projections, for `slots` rows of
    /// `width`: the gate (w1 output) clamped above at `limit`, the up
    /// projection (w3 output) clamped to `[-limit, limit]` (no clamp when
    /// `limit` is 0), `out = gate × sigmoid(gate) × up × weight`. `weight` is
    /// `route_weights[slot]`, or 1 without them (the shared expert). NaN
    /// inputs give NaN. Use `V41Output::Bf16` for the reference's BF16 cast
    /// before w2.
    ///
    /// # Errors
    ///
    /// Refuses a bad shape or limit and buffers too small for them.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_swiglu(
        &mut self,
        gate: &CudaBuffer,
        up: &CudaBuffer,
        route_weights: Option<&CudaBuffer>,
        slots: usize,
        width: usize,
        limit: f32,
        out: &CudaBuffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        let n = slots
            .checked_mul(width)
            .filter(|n| *n > 0 && i32::try_from(*n).is_ok())
            .ok_or_else(|| invalid(format!("SwiGLU {slots}x{width}")))?;
        if !limit.is_finite() || limit < 0.0 {
            return Err(invalid(format!("SwiGLU limit {limit}")));
        }
        let gate = span(gate, 0, n, 4, "gate projection")?;
        let up = span(up, 0, n, 4, "up projection")?;
        let route_weights = match route_weights {
            Some(weights) => span(weights, 0, slots, 4, "route weights")?.cast_const(),
            None => std::ptr::null(),
        };
        let out = span(out, 0, n, 4, "SwiGLU output")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_swiglu(
                gate,
                up,
                route_weights,
                out,
                int(slots, "slots")?,
                int(width, "width")?,
                limit,
                rounds(output),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 SwiGLU")
    }

    /// Adds the routed experts' outputs (`slots × width`) in slot order,
    /// starting from 0, then the shared expert's output when given, as the
    /// reference's MoE sum. With [`Self::v41_route`]'s ids the slots are in
    /// expert-id order.
    ///
    /// # Errors
    ///
    /// Refuses a bad shape and buffers too small for it.
    pub fn v41_moe_combine(
        &mut self,
        routed: &CudaBuffer,
        slots: usize,
        shared: Option<&CudaBuffer>,
        width: usize,
        out: &CudaBuffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        self.v41_moe_combine_tokens(routed, slots, shared, width, out, output, 1)
    }

    /// [`Self::v41_moe_combine`] for `tokens` tokens one after another.
    ///
    /// # Errors
    ///
    /// As [`Self::v41_moe_combine`].
    #[allow(clippy::too_many_arguments)]
    pub fn v41_moe_combine_tokens(
        &mut self,
        routed: &CudaBuffer,
        slots: usize,
        shared: Option<&CudaBuffer>,
        width: usize,
        out: &CudaBuffer,
        output: V41Output,
        tokens: usize,
    ) -> Result<(), RuntimeError> {
        let t = tokens.max(1);
        let n = slots
            .checked_mul(width)
            .filter(|n| width > 0 && i32::try_from(*n).is_ok())
            .ok_or_else(|| invalid(format!("MoE sum {slots}x{width}")))?;
        let routed = span(routed, 0, n * t, 4, "routed outputs")?;
        let shared = match shared {
            Some(shared) => span(shared, 0, width * t, 4, "shared output")?.cast_const(),
            None => std::ptr::null(),
        };
        let out = span(out, 0, width * t, 4, "MoE output")?;
        // SAFETY: every extent was checked against its buffer above.
        let code = unsafe {
            psionic_v41_moe_combine(
                routed,
                shared,
                out,
                int(slots, "slots")?,
                int(width, "width")?,
                rounds(output),
                int(t, "tokens")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "v41 MoE sum")
    }
}

// ---------------------------------------------------------------------------
// Host models
// ---------------------------------------------------------------------------

/// `exp` as the device evaluates it: in f64, rounded once.
#[must_use]
pub fn v41_exp_once(x: f32) -> f32 {
    f64::from(x).exp() as f32
}

fn log1p_once(x: f32) -> f32 {
    f64::from(x).ln_1p() as f32
}

/// The CPU path's sigmoid formula with [`v41_exp_once`].
#[must_use]
pub fn v41_sigmoid_once(x: f32) -> f32 {
    if x >= 0.0 {
        1.0 / (1.0 + v41_exp_once(-x))
    } else {
        let e = v41_exp_once(x);
        e / (1.0 + e)
    }
}

/// One warp's shuffle-down tree (offsets 16 to 1); lane 0's result.
fn warp_tree(lanes: &[f32; 32]) -> f32 {
    let mut v = *lanes;
    for offset in [16, 8, 4, 2, 1] {
        let before = v;
        for lane in 0..32 {
            let other = if lane + offset < 32 {
                before[lane + offset]
            } else {
                before[lane]
            };
            v[lane] = before[lane] + other;
        }
    }
    v[0]
}

/// A 256-thread block's sum of `terms` (indices `0..n`): thread `t` adds
/// terms `t, t + 256, ...` in order from 0, then [`warp_tree`] per warp, then
/// warp totals in order.
fn block_sum_host(n: usize, term: impl Fn(usize) -> f32) -> f32 {
    let mut threads = [0.0_f32; V41_REDUCE_THREADS];
    for (t, acc) in threads.iter_mut().enumerate() {
        let mut i = t;
        while i < n {
            *acc += term(i);
            i += V41_REDUCE_THREADS;
        }
    }
    let warps: Vec<f32> = threads
        .chunks_exact(32)
        .map(|w| warp_tree(w.try_into().expect("32 lanes")))
        .collect();
    warps[1..].iter().fold(warps[0], |a, b| a + b)
}

/// The device's [`CudaSubmission::v41_hc_mixes`] on the host: returns the
/// projected values and the mixes.
///
/// # Panics
///
/// Panics when a slice is shorter than the configuration needs.
#[must_use]
pub fn v41_hc_mixes_host(
    hc: V41HyperConfig,
    projection: &[f32],
    scale: &[f32],
    base: &[f32],
    residual: &[f32],
) -> (Vec<f32>, Vec<f32>) {
    let m = hc.mixes();
    let elements = hc.elements();
    let chunk = elements.div_ceil(V41_HC_SPLITS);
    let slice_sum = |row: usize, split: usize| {
        let begin = split * chunk;
        let end = elements.min(begin + chunk);
        let n = end.saturating_sub(begin);
        block_sum_host(n, |i| {
            let x = residual[begin + i];
            let w = if row < m {
                projection[row * elements + begin + i]
            } else {
                x
            };
            w * x
        })
    };
    let total =
        |row: usize| (1..V41_HC_SPLITS).fold(slice_sum(row, 0), |a, s| a + slice_sum(row, s));
    let variance = total(m) / elements as f32;
    let inv_rms = (variance + hc.norm_epsilon).sqrt().recip();
    let projected: Vec<f32> = (0..m)
        .map(|row| {
            if variance.is_finite() {
                total(row) * inv_rms
            } else {
                f32::NAN
            }
        })
        .collect();
    let mixes = v41_hc_split_sinkhorn_host(hc, &projected, scale, base);
    (projected, mixes)
}

/// The device's [`CudaSubmission::v41_hc_split_sinkhorn`] on the host: the
/// CPU path's steps with [`v41_exp_once`].
///
/// # Panics
///
/// Panics when a slice is shorter than the configuration needs.
#[must_use]
pub fn v41_hc_split_sinkhorn_host(
    hc: V41HyperConfig,
    projected: &[f32],
    scale: &[f32],
    base: &[f32],
) -> Vec<f32> {
    let h = hc.streams;
    let m = hc.mixes();
    let logits: Vec<f32> = (0..m)
        .map(|i| {
            let s = scale[if i < h {
                0
            } else if i < 2 * h {
                1
            } else {
                2
            }];
            projected[i] * s + base[i]
        })
        .collect();
    if logits.iter().any(|x| !x.is_finite()) {
        return vec![f32::NAN; m];
    }
    let eps = hc.mix_epsilon;
    let mut out: Vec<f32> = logits[..h]
        .iter()
        .map(|x| v41_sigmoid_once(*x) + eps)
        .chain(logits[h..2 * h].iter().map(|x| 2.0 * v41_sigmoid_once(*x)))
        .collect();
    let mut comb = logits[2 * h..].to_vec();
    for row in comb.chunks_exact_mut(h) {
        let max = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        for x in row.iter_mut() {
            *x = v41_exp_once(*x - max);
        }
        let sum = row[1..].iter().fold(row[0], |a, b| a + b);
        for x in row {
            *x = *x / sum + eps;
        }
    }
    for iteration in 0..hc.sinkhorn_iterations {
        if iteration > 0 {
            for row in comb.chunks_exact_mut(h) {
                let sum = row[1..].iter().fold(row[0], |a, b| a + b) + eps;
                for x in row {
                    *x /= sum;
                }
            }
        }
        for k in 0..h {
            let sum = (1..h).fold(comb[k], |a, j| a + comb[j * h + k]) + eps;
            for j in 0..h {
                comb[j * h + k] /= sum;
            }
        }
    }
    out.extend(comb);
    out
}

/// The device's [`CudaSubmission::v41_hc_collapse`] on the host (float32,
/// before any BF16 rounding).
///
/// # Panics
///
/// Panics when a slice is shorter than the configuration needs.
#[must_use]
pub fn v41_hc_collapse_host(hc: V41HyperConfig, residual: &[f32], pre: &[f32]) -> Vec<f32> {
    let w = hc.width;
    (0..w)
        .map(|d| {
            (1..hc.streams).fold(pre[0] * residual[d], |a, j| {
                a + pre[j] * residual[j * w + d]
            })
        })
        .collect()
}

/// The device's [`CudaSubmission::v41_hc_expand`] on the host (float32,
/// before any BF16 rounding).
///
/// # Panics
///
/// Panics when a slice is shorter than the configuration needs.
#[must_use]
pub fn v41_hc_expand_host(
    hc: V41HyperConfig,
    output: &[f32],
    residual: &[f32],
    mixes: &[f32],
) -> Vec<f32> {
    let (h, w) = (hc.streams, hc.width);
    let comb = &mixes[2 * h..];
    (0..h * w)
        .map(|i| {
            let (k, d) = (i / w, i % w);
            let sum = (1..h).fold(comb[k] * residual[d], |a, j| {
                a + comb[j * h + k] * residual[j * w + d]
            });
            mixes[h + k] * output[d] + sum
        })
        .collect()
}

/// The device's [`CudaSubmission::v41_engram_gather`] on the host, from the
/// whole table (float32, before any BF16 rounding).
///
/// # Panics
///
/// Panics when a table slice is shorter than `rows × width`.
#[must_use]
pub fn v41_engram_gather_host(
    codes: &[u8],
    scales: &[u8],
    rows: u64,
    ids: &[i64],
    width: usize,
) -> Vec<f32> {
    let groups = width / 32;
    ids.iter()
        .flat_map(|id| {
            let row = u64::try_from(*id).ok().filter(|id| *id < rows);
            (0..width).map(move |i| match row {
                Some(row) => {
                    let row = usize::try_from(row).expect("row fits usize");
                    e4m3(codes[row * width + i]) * e8m0(scales[row * groups + i / 32])
                }
                None => f32::NAN,
            })
        })
        .collect()
}

/// The device's [`CudaSubmission::v41_engram_gate`] on the host (float32,
/// before any BF16 rounding).
///
/// # Panics
///
/// Panics when a slice is shorter than the geometry needs.
#[must_use]
pub fn v41_engram_gate_host(
    gate: V41EngramGate,
    residual: &[f32],
    projected: &[f32],
    q_weight: &[f32],
    k_weight: &[f32],
) -> Vec<f32> {
    let (s, w) = (gate.streams, gate.width);
    let value = &projected[s * w..(s + 1) * w];
    let mut out = Vec::with_capacity(s * w);
    for h in 0..s {
        let start = h * w;
        let x = &residual[start..start + w];
        let key = &projected[start..start + w];
        let xs = block_sum_host(w, |d| x[d] * x[d]);
        let ks = block_sum_host(w, |d| key[d] * key[d]);
        let dot = block_sum_host(w, |d| {
            x[d] * (q_weight[start + d] * k_weight[start + d]) * key[d]
        });
        let xvar = xs / w as f32;
        let kvar = ks / w as f32;
        let rstd = (xvar + gate.epsilon).sqrt().recip() * (kvar + gate.epsilon).sqrt().recip();
        let scaled = dot * rstd * gate.inv_sqrt_width();
        let g = if xvar.is_finite() && kvar.is_finite() && scaled.is_finite() {
            v41_sigmoid_once(scaled.abs().max(1e-6).sqrt().copysign(scaled))
        } else {
            f32::NAN
        };
        out.extend((0..w).map(|d| x[d] + g * value[d]));
    }
    out
}

/// The device's [`CudaSubmission::v41_gate_logits`] on the host.
///
/// # Panics
///
/// Panics when a slice is shorter than the shape needs.
#[must_use]
pub fn v41_gate_logits_host(weight: &[u8], input: &[f32], experts: usize, cols: usize) -> Vec<f32> {
    (0..experts)
        .map(|row| {
            let mut lanes = [0.0_f32; 32];
            for (lane, acc) in lanes.iter_mut().enumerate() {
                let mut chunk = lane;
                while chunk * 8 < cols {
                    for j in 0..8 {
                        let k = chunk * 8 + j;
                        let at = 2 * (row * cols + k);
                        let w = half::bf16::from_le_bytes([weight[at], weight[at + 1]]).to_f32();
                        *acc += w * input[k];
                    }
                    chunk += 32;
                }
            }
            warp_tree(&lanes)
        })
        .collect()
}

/// The device's [`CudaSubmission::v41_route`] on the host: ids in ascending
/// order with their weights, or `-1` and NaN in every slot for nonfinite
/// input.
///
/// # Panics
///
/// Panics when a slice is shorter than `router.experts`.
#[must_use]
pub fn v41_route_host(router: V41Router, logits: &[f32], bias: &[f32]) -> (Vec<i32>, Vec<f32>) {
    let n = router.experts;
    let mut scores = Vec::with_capacity(n);
    let mut ranked = Vec::with_capacity(n);
    for e in 0..n {
        let x = logits[e] / router.temperature;
        let score = if x > 20.0 {
            x.sqrt()
        } else {
            log1p_once(v41_exp_once(x)).sqrt()
        };
        let r = score + bias[e];
        if !logits[e].is_finite() || !x.is_finite() || !r.is_finite() {
            return (vec![-1; router.top_k], vec![f32::NAN; router.top_k]);
        }
        scores.push(score);
        ranked.push(r);
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| {
        ranked[*b]
            .partial_cmp(&ranked[*a])
            .expect("finite")
            .then(a.cmp(b))
    });
    order.truncate(router.top_k);
    let mut weights: Vec<f32> = order.iter().map(|e| scores[*e]).collect();
    if router.normalize && router.top_k > 1 {
        let sum = weights[1..].iter().fold(weights[0], |a, b| a + b) + 1e-20;
        for w in &mut weights {
            *w /= sum;
        }
    }
    let mut pairs: Vec<(i32, f32)> = order
        .iter()
        .zip(&weights)
        .map(|(e, w)| (*e as i32, w * router.route_scale))
        .collect();
    pairs.sort_by_key(|(id, _)| *id);
    pairs.into_iter().unzip()
}

/// The device's [`CudaSubmission::v41_swiglu`] on the host (float32, before
/// any BF16 rounding).
///
/// # Panics
///
/// Panics when a slice is shorter than the shape needs.
#[must_use]
pub fn v41_swiglu_host(
    gate: &[f32],
    up: &[f32],
    route_weights: Option<&[f32]>,
    width: usize,
    limit: f32,
) -> Vec<f32> {
    gate.iter()
        .zip(up)
        .enumerate()
        .map(|(i, (g, u))| {
            if g.is_nan() || u.is_nan() {
                return f32::NAN;
            }
            let (mut g, mut u) = (*g, *u);
            if limit > 0.0 {
                g = g.min(limit);
                u = u.max(-limit).min(limit);
            }
            let w = route_weights.map_or(1.0, |w| w[i / width]);
            g * v41_sigmoid_once(g) * u * w
        })
        .collect()
}

/// The device's [`CudaSubmission::v41_moe_combine`] on the host (float32,
/// before any BF16 rounding).
///
/// # Panics
///
/// Panics when a slice is shorter than the shape needs.
#[must_use]
pub fn v41_moe_combine_host(
    routed: &[f32],
    slots: usize,
    shared: Option<&[f32]>,
    width: usize,
) -> Vec<f32> {
    (0..width)
        .map(|d| {
            let acc = (0..slots).fold(0.0_f32, |a, s| a + routed[s * width + d]);
            shared.map_or(acc, |shared| acc + shared[d])
        })
        .collect()
}

/// Rounds each value to BF16 as `V41Output::Bf16` does.
#[must_use]
pub fn v41_round_bf16_all(values: &[f32]) -> Vec<f32> {
    values.iter().map(|v| v41_round_bf16(*v)).collect()
}

fn e4m3(byte: u8) -> f32 {
    let magnitude = u32::from(byte & 0x7f);
    let value = if magnitude == 0x7f {
        f32::NAN
    } else {
        f32::from_bits(magnitude << 20) * f32::from_bits((120 + 127) << 23)
    };
    f32::from_bits(value.to_bits() | (u32::from(byte & 0x80) << 24))
}

fn e8m0(byte: u8) -> f32 {
    match byte {
        0xff => f32::NAN,
        0 => f32::from_bits(0x0040_0000),
        exponent => f32::from_bits(u32::from(exponent) << 23),
    }
}

#[cfg(test)]
#[path = "v41_mix_tests.rs"]
mod tests;
