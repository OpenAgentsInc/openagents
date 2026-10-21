//! V4.1 hyper-connection mixing, Engram gather and gating, and expert
//! routing on Metal, a port of `psionic_backend_cuda::v41_mix` (#12393).
//! The configuration types and the host models are the CUDA backend's.
//!
//! The host models round `exp` and `log1p` once from double precision; the
//! Metal kernels equal that for every float input (see `checks.rs`), so the
//! models reproduce the device bit for bit wherever no result falls below
//! 2^-126.

pub use psionic_backend_cuda::v41_mix::{
    V41_HC_SPLITS, V41_REDUCE_THREADS, V41_ROUTE_MAX_EXPERTS, V41_ROUTE_MAX_TOP_K, V41EngramGate,
    V41HyperConfig, V41Router, v41_engram_gate_host, v41_engram_gather_host, v41_exp_once,
    v41_gate_logits_host, v41_hc_collapse_host, v41_hc_expand_host, v41_hc_mixes_host,
    v41_hc_split_sinkhorn_host, v41_moe_combine_host, v41_round_bf16_all, v41_route_host,
    v41_sigmoid_once, v41_swiglu_host,
};

use super::matvec::V41Output;
use super::{Args, V41Buffer, V41Submission, int};
use psionic_runtime::RuntimeError;

const THREADS: usize = V41_REDUCE_THREADS;
const ROUTE_THREADS: usize = 1024;

fn invalid(message: String) -> RuntimeError {
    RuntimeError::Backend(format!("v41 mixing kernels: {message}"))
}

fn positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn rounds(output: V41Output) -> i32 {
    i32::from(output == V41Output::Bf16)
}

fn check_hc(hc: V41HyperConfig) -> Result<(), RuntimeError> {
    if !(1..=4).contains(&hc.streams)
        || hc.width == 0
        || hc.sinkhorn_iterations == 0
        || !positive(hc.norm_epsilon)
        || !positive(hc.mix_epsilon)
        || i32::try_from(hc.elements()).is_err()
        || i32::try_from(hc.sinkhorn_iterations).is_err()
    {
        return Err(invalid(format!("hyper-connection configuration {hc:?}")));
    }
    Ok(())
}

fn check_router(router: V41Router) -> Result<(), RuntimeError> {
    if !(1..=V41_ROUTE_MAX_EXPERTS).contains(&router.experts)
        || !(1..=V41_ROUTE_MAX_TOP_K.min(router.experts)).contains(&router.top_k)
        || !positive(router.temperature)
        || !positive(router.route_scale)
    {
        return Err(invalid(format!("router {router:?}")));
    }
    Ok(())
}

/// One sub-block's mixing parameters on the device, as `V41HcWeights`.
#[derive(Clone, Copy, Debug)]
pub struct V41HcWeights<'a> {
    pub projection: &'a V41Buffer,
    pub scale: &'a V41Buffer,
    pub base: &'a V41Buffer,
}

/// An Engram table in unified memory, as `V41EngramTable`: `F8_E4M3` codes
/// `[rows, width]` and `F8_E8M0` scales `[rows, width / 32]`.
#[derive(Clone, Copy, Debug)]
pub struct V41EngramTable<'a> {
    pub codes: &'a V41Buffer,
    pub scales: &'a V41Buffer,
    pub rows: u64,
    pub width: usize,
}

impl V41Submission<'_> {
    /// One sub-block's mixes, as `CudaSubmission::v41_hc_mixes`. Two
    /// launches.
    ///
    /// # Errors
    ///
    /// Refuses a bad configuration and buffers too small for it.
    pub fn v41_hc_mixes(
        &mut self,
        hc: V41HyperConfig,
        weights: V41HcWeights<'_>,
        residual: &V41Buffer,
        scratch: &V41Buffer,
        mixes: &V41Buffer,
    ) -> Result<(), RuntimeError> {
        check_hc(hc)?;
        let m = hc.mixes();
        let residual = self.span(residual, 0, hc.elements(), 4, "residual")?;
        let projection = self.span(weights.projection, 0, m * hc.elements(), 4, "projection")?;
        let scale = self.span(weights.scale, 0, 3, 4, "scale")?;
        let base = self.span(weights.base, 0, m, 4, "base")?;
        let scratch = self.span(scratch, 0, hc.scratch_floats(), 4, "scratch")?;
        let out = self.span(mixes, 0, m, 4, "mixes")?;
        let elements = int(hc.elements(), "elements")?;
        self.launch(
            "v41_hc_project",
            Args::new()
                .ptr(residual)
                .ptr(projection)
                .ptr(scratch)
                .i32(int(m, "mixes")?)
                .i32(elements),
            (V41_HC_SPLITS, m + 1, 1),
            THREADS,
            0,
        )?;
        self.launch(
            "v41_hc_finish",
            Args::new()
                .ptr(scratch)
                .ptr(scale)
                .ptr(base)
                .ptr(scratch + ((m + 1) * V41_HC_SPLITS * 4) as u64)
                .ptr(out)
                .i32(int(hc.streams, "streams")?)
                .i32(elements)
                .f32(hc.norm_epsilon)
                .f32(hc.mix_epsilon)
                .i32(int(hc.sinkhorn_iterations, "iterations")?),
            (1, 1, 1),
            32,
            0,
        )
    }

    /// The mixes from projected values, as
    /// `CudaSubmission::v41_hc_split_sinkhorn`.
    ///
    /// # Errors
    ///
    /// Refuses a bad configuration and buffers too small for it.
    pub fn v41_hc_split_sinkhorn(
        &mut self,
        hc: V41HyperConfig,
        projected: &V41Buffer,
        scale: &V41Buffer,
        base: &V41Buffer,
        mixes: &V41Buffer,
    ) -> Result<(), RuntimeError> {
        check_hc(hc)?;
        let m = hc.mixes();
        let args = Args::new()
            .ptr(self.span(projected, 0, m, 4, "projected")?)
            .ptr(self.span(scale, 0, 3, 4, "scale")?)
            .ptr(self.span(base, 0, m, 4, "base")?)
            .ptr(self.span(mixes, 0, m, 4, "mixes")?)
            .i32(int(hc.streams, "streams")?)
            .f32(hc.mix_epsilon)
            .i32(int(hc.sinkhorn_iterations, "iterations")?);
        self.launch("v41_hc_split", args, (1, 1, 1), 32, 0)
    }

    /// Collapses the streams, as `CudaSubmission::v41_hc_collapse`.
    ///
    /// # Errors
    ///
    /// Refuses a bad configuration and buffers too small for it.
    pub fn v41_hc_collapse(
        &mut self,
        hc: V41HyperConfig,
        residual: &V41Buffer,
        previous_mixes: &V41Buffer,
        out: &V41Buffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        check_hc(hc)?;
        let args = Args::new()
            .ptr(self.span(residual, 0, hc.elements(), 4, "residual")?)
            .ptr(self.span(previous_mixes, 0, hc.streams, 4, "previous mixes")?)
            .ptr(self.span(out, 0, hc.width, 4, "collapsed output")?)
            .i32(int(hc.streams, "streams")?)
            .i32(int(hc.width, "width")?)
            .i32(rounds(output));
        self.launch(
            "v41_hc_collapse",
            args,
            (hc.width.div_ceil(THREADS), 1, 1),
            THREADS,
            0,
        )
    }

    /// Expands a sub-block's output back into the streams, as
    /// `CudaSubmission::v41_hc_expand`.
    ///
    /// # Errors
    ///
    /// Refuses a bad configuration and buffers too small for it.
    pub fn v41_hc_expand(
        &mut self,
        hc: V41HyperConfig,
        sublayer_output: &V41Buffer,
        residual: &V41Buffer,
        mixes: &V41Buffer,
        out: &V41Buffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        check_hc(hc)?;
        let args = Args::new()
            .ptr(self.span(sublayer_output, 0, hc.width, 4, "sub-block output")?)
            .ptr(self.span(residual, 0, hc.elements(), 4, "residual")?)
            .ptr(self.span(mixes, 0, hc.mixes(), 4, "mixes")?)
            .ptr(self.span(out, 0, hc.elements(), 4, "expanded output")?)
            .i32(int(hc.streams, "streams")?)
            .i32(int(hc.width, "width")?)
            .i32(rounds(output));
        self.launch(
            "v41_hc_expand",
            args,
            (hc.elements().div_ceil(THREADS), 1, 1),
            THREADS,
            0,
        )
    }

    /// Gathers Engram table rows by device `i64` ids, as
    /// `CudaSubmission::v41_engram_gather`. The table is in unified memory.
    ///
    /// # Errors
    ///
    /// Refuses a bad width or count and buffers too small for them.
    pub fn v41_engram_gather(
        &mut self,
        table: V41EngramTable<'_>,
        ids: &V41Buffer,
        count: usize,
        out: &V41Buffer,
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
        let rows_usize = usize::try_from(table.rows)
            .map_err(|_| invalid(format!("{} table rows", table.rows)))?;
        let codes = self.span(table.codes, 0, rows_usize * table.width, 1, "table codes")?;
        let scales = self.span(
            table.scales,
            0,
            rows_usize * (table.width / 32),
            1,
            "table scales",
        )?;
        let args = Args::new()
            .ptr(codes)
            .ptr(scales)
            .i64(rows)
            .ptr(self.span(ids, 0, count, 8, "row ids")?)
            .ptr(self.span(out, 0, count * table.width, 4, "gathered rows")?)
            .i32(int(table.width, "width")?)
            .i32(rounds(output));
        self.launch("v41_engram_gather", args, (count, 1, 1), 32, 0)
    }

    /// Applies the Engram gate, as `CudaSubmission::v41_engram_gate`.
    ///
    /// # Errors
    ///
    /// Refuses a bad geometry and buffers too small for it.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_engram_gate(
        &mut self,
        gate: V41EngramGate,
        residual: &V41Buffer,
        projected: &V41Buffer,
        q_weight: &V41Buffer,
        k_weight: &V41Buffer,
        out: &V41Buffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        let elements = gate
            .streams
            .checked_mul(gate.width)
            .filter(|n| *n > 0 && i32::try_from(*n + gate.width).is_ok())
            .ok_or_else(|| invalid(format!("Engram gate {gate:?}")))?;
        if gate.streams > 65_535 || !positive(gate.epsilon) {
            return Err(invalid(format!("Engram gate {gate:?}")));
        }
        let args = Args::new()
            .ptr(self.span(residual, 0, elements, 4, "residual")?)
            .ptr(self.span(projected, 0, elements + gate.width, 4, "keys and value")?)
            .ptr(self.span(q_weight, 0, elements, 4, "query weight")?)
            .ptr(self.span(k_weight, 0, elements, 4, "key weight")?)
            .ptr(self.span(out, 0, elements, 4, "gated residual")?)
            .i32(int(gate.streams, "streams")?)
            .i32(int(gate.width, "width")?)
            .f32(gate.epsilon)
            .f32(gate.inv_sqrt_width())
            .i32(rounds(output));
        self.launch("v41_engram_gate", args, (gate.streams, 1, 1), THREADS, 0)
    }

    /// Router logits from the BF16 gate, as `CudaSubmission::v41_gate_logits`.
    ///
    /// # Errors
    ///
    /// Refuses `cols` that isn't a nonzero multiple of 8 and buffers too
    /// small for the shape.
    pub fn v41_gate_logits(
        &mut self,
        weight: &V41Buffer,
        input: &V41Buffer,
        logits: &V41Buffer,
        experts: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        if experts == 0 || cols == 0 || !cols.is_multiple_of(8) {
            return Err(invalid(format!("gate {experts}x{cols}")));
        }
        let elements = experts
            .checked_mul(cols)
            .filter(|n| i32::try_from(*n).is_ok())
            .ok_or_else(|| invalid(format!("gate {experts}x{cols}")))?;
        let args = Args::new()
            .ptr(self.span(weight, 0, elements, 2, "gate weight")?)
            .ptr(self.span(input, 0, cols, 4, "gate input")?)
            .ptr(self.span(logits, 0, experts, 4, "logits")?)
            .i32(int(experts, "experts")?)
            .i32(int(cols, "cols")?);
        self.launch(
            "v41_gate_logits",
            args,
            (experts.div_ceil(THREADS / 32), 1, 1),
            THREADS,
            0,
        )
    }

    /// Selects experts, as `CudaSubmission::v41_route`.
    ///
    /// # Errors
    ///
    /// Refuses a bad router and buffers too small for it.
    pub fn v41_route(
        &mut self,
        router: V41Router,
        logits: &V41Buffer,
        bias: &V41Buffer,
        ids: &V41Buffer,
        weights: &V41Buffer,
    ) -> Result<(), RuntimeError> {
        check_router(router)?;
        let args = Args::new()
            .ptr(self.span(logits, 0, router.experts, 4, "logits")?)
            .ptr(self.span(bias, 0, router.experts, 4, "bias")?)
            .ptr(self.span(ids, 0, router.top_k, 4, "expert ids")?)
            .ptr(self.span(weights, 0, router.top_k, 4, "route weights")?)
            .i32(int(router.experts, "experts")?)
            .i32(int(router.top_k, "top k")?)
            .f32(router.temperature)
            .i32(i32::from(router.normalize))
            .f32(router.route_scale);
        self.launch("v41_route", args, (1, 1, 1), ROUTE_THREADS, 0)
    }

    /// Clamped SwiGLU, as `CudaSubmission::v41_swiglu`.
    ///
    /// # Errors
    ///
    /// Refuses a bad shape or limit and buffers too small for them.
    #[allow(clippy::too_many_arguments)]
    pub fn v41_swiglu(
        &mut self,
        gate: &V41Buffer,
        up: &V41Buffer,
        route_weights: Option<&V41Buffer>,
        slots: usize,
        width: usize,
        limit: f32,
        out: &V41Buffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        let n = slots
            .checked_mul(width)
            .filter(|n| *n > 0 && i32::try_from(*n).is_ok())
            .ok_or_else(|| invalid(format!("SwiGLU {slots}x{width}")))?;
        if !limit.is_finite() || limit < 0.0 {
            return Err(invalid(format!("SwiGLU limit {limit}")));
        }
        let args = Args::new()
            .ptr(self.span(gate, 0, n, 4, "gate projection")?)
            .ptr(self.span(up, 0, n, 4, "up projection")?)
            .ptr(self.span_opt(route_weights, 0, slots, 4, "route weights")?)
            .ptr(self.span(out, 0, n, 4, "SwiGLU output")?)
            .i32(int(slots, "slots")?)
            .i32(int(width, "width")?)
            .f32(limit)
            .i32(rounds(output));
        self.launch("v41_swiglu", args, (n.div_ceil(THREADS), 1, 1), THREADS, 0)
    }

    /// Adds the routed experts' outputs and the shared expert's, as
    /// `CudaSubmission::v41_moe_combine`.
    ///
    /// # Errors
    ///
    /// Refuses a bad shape and buffers too small for it.
    pub fn v41_moe_combine(
        &mut self,
        routed: &V41Buffer,
        slots: usize,
        shared: Option<&V41Buffer>,
        width: usize,
        out: &V41Buffer,
        output: V41Output,
    ) -> Result<(), RuntimeError> {
        let n = slots
            .checked_mul(width)
            .filter(|n| width > 0 && i32::try_from(*n).is_ok())
            .ok_or_else(|| invalid(format!("MoE sum {slots}x{width}")))?;
        let args = Args::new()
            .ptr(self.span(routed, 0, n, 4, "routed outputs")?)
            .ptr(self.span_opt(shared, 0, width, 4, "shared output")?)
            .ptr(self.span(out, 0, width, 4, "MoE output")?)
            .i32(int(slots, "slots")?)
            .i32(int(width, "width")?)
            .i32(rounds(output));
        self.launch(
            "v41_moe_combine",
            args,
            (width.div_ceil(THREADS), 1, 1),
            THREADS,
            0,
        )
    }
}

// ---------------------------------------------------------------------------
// Host models with subnormals flushed
// ---------------------------------------------------------------------------

use super::matvec::ftz;

/// `exp` as the Metal kernels evaluate it: the host's double-rounded value
/// with a subnormal result flushed to zero.
#[must_use]
pub fn v41_exp_once_ftz(x: f32) -> f32 {
    ftz(v41_exp_once(x))
}

/// The CPU path's sigmoid as the Metal kernels evaluate it.
#[must_use]
pub fn v41_sigmoid_once_ftz(x: f32) -> f32 {
    if x >= 0.0 {
        ftz(1.0 / (1.0 + v41_exp_once_ftz(-x)))
    } else {
        let e = v41_exp_once_ftz(x);
        ftz(e / (1.0 + e))
    }
}

/// [`v41_swiglu_host`] as an Apple GPU computes it: every result's
/// subnormals flushed. The two differ only for a gate below about -87,
/// where `exp` underflows: the CUDA kernel keeps a value near 1e-37 there
/// and the Metal kernel gives 0.
#[must_use]
pub fn v41_swiglu_host_ftz(
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
            ftz(ftz(ftz(g * v41_sigmoid_once_ftz(g)) * u) * w)
        })
        .collect()
}

#[cfg(test)]
#[path = "mix_tests.rs"]
mod tests;
