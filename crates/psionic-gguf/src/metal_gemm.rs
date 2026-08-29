//! Metal Q8_0 matvec on the shared mmap buffer.
//!
//! Frameworks stay `dlopen`-only so lib tests do not link Metal.

use std::cell::Cell;
use std::ffi::{c_void, CStr};
use std::ptr;
use std::sync::OnceLock;

use crate::format::GgufMeta;
use crate::generate::Q8_BLOCK;
use crate::generate::Q8_K;
use crate::metal_wrap::MetalShared;
use crate::mmap::MappedWeights;

#[derive(Clone, Copy)]
pub struct HybridSpec {
    pub hidden: usize,
    pub epsilon: f32,
    pub state_size: usize,
    pub group_count: usize,
    pub time_step_rank: usize,
    pub inner_size: usize,
    pub conv_kernel: usize,
    pub v_head_reordered: bool,
    pub ffn: usize,
}

#[derive(Clone, Copy)]
pub struct AttnSpec {
    pub heads: usize,
    pub kv_heads: usize,
    pub dim: usize,
    pub rotary: usize,
    pub rope_theta: f32,
    pub mrope: [usize; 4],
    pub n_ctx: usize,
}

pub fn spec_from_meta(meta: &GgufMeta) -> HybridSpec {
    HybridSpec {
        hidden: meta.kv_u64("qwen35.embedding_length").unwrap_or(8) as usize,
        epsilon: meta
            .kv_f32("qwen35.attention.layer_norm_rms_epsilon")
            .unwrap_or(1e-6),
        state_size: meta.kv_u64("qwen35.ssm.state_size").unwrap_or(4) as usize,
        group_count: meta.kv_u64("qwen35.ssm.group_count").unwrap_or(1) as usize,
        time_step_rank: meta.kv_u64("qwen35.ssm.time_step_rank").unwrap_or(1) as usize,
        inner_size: meta.kv_u64("qwen35.ssm.inner_size").unwrap_or(4) as usize,
        conv_kernel: meta.kv_u64("qwen35.ssm.conv_kernel").unwrap_or(4) as usize,
        v_head_reordered: meta
            .kv_u64("qwen35.ssm.v_head_reordered")
            .map(|v| v != 0)
            .unwrap_or(true),
        ffn: meta.kv_u64("qwen35.feed_forward_length").unwrap_or(0) as usize,
    }
}

thread_local! {
    static ACTIVE: Cell<*const MetalShared> = const { Cell::new(ptr::null()) };
}

pub struct BindGuard {
    prev: *const MetalShared,
}

impl Drop for BindGuard {
    fn drop(&mut self) {
        ACTIVE.with(|slot| slot.set(self.prev));
    }
}

pub fn bind(metal: Option<&MetalShared>) -> BindGuard {
    ACTIVE.with(|slot| {
        let prev = slot.get();
        slot.set(
            metal
                .map(|m| m as *const MetalShared)
                .unwrap_or(ptr::null()),
        );
        BindGuard { prev }
    })
}

pub fn try_q8_matvec(mapped: &MappedWeights, name: &str, x: &[f32]) -> Option<Vec<f32>> {
    let metal = ACTIVE.with(|slot| {
        let ptr = slot.get();
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &*ptr })
        }
    })?;
    let view = mapped.tensors.get(name)?;
    if view.info.ggml_type != 8 {
        return None;
    }
    let width = x.len();
    if width == 0 {
        return None;
    }
    let blocks = width.div_ceil(Q8_K);
    let row_bytes = blocks * Q8_BLOCK;
    if row_bytes == 0 || view.len < row_bytes {
        return None;
    }
    let rows = view.len / row_bytes;
    let base = mapped.mmap.as_ptr() as usize;
    let offset = (view.data as usize).checked_sub(base)?;
    if offset.saturating_add(view.len) > metal.length as usize {
        return None;
    }
    q8_matvec(metal, offset, rows, width, x)
}

pub fn try_q8_matvec_many(
    mapped: &MappedWeights,
    names: &[&str],
    x: &[f32],
) -> Option<Vec<Vec<f32>>> {
    if names.is_empty() {
        return Some(Vec::new());
    }
    let metal = unsafe { &*active_metal()? };
    let jobs: Vec<Q8Job> = names
        .iter()
        .map(|name| resolve_q8(mapped, metal, name, x.len()))
        .collect::<Option<_>>()?;
    q8_matvec_many(metal, &jobs, x)
}

pub fn try_q8_ffn(
    mapped: &MappedWeights,
    gate: &str,
    up: &str,
    down: &str,
    x: &[f32],
) -> Option<Vec<f32>> {
    let metal = unsafe { &*active_metal()? };
    let gate_j = resolve_q8(mapped, metal, gate, x.len())?;
    let up_j = resolve_q8(mapped, metal, up, x.len())?;
    if gate_j.rows != up_j.rows {
        return None;
    }
    let down_j = resolve_q8(mapped, metal, down, gate_j.rows)?;
    q8_ffn(metal, &gate_j, &up_j, &down_j, x)
}

pub fn bound() -> bool {
    ACTIVE.with(|slot| !slot.get().is_null())
}

pub(crate) fn active_metal() -> Option<*const MetalShared> {
    ACTIVE.with(|slot| {
        let ptr = slot.get();
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    })
}

struct Q8Job {
    offset: usize,
    rows: usize,
    width: usize,
}

fn resolve_q8(
    mapped: &MappedWeights,
    metal: &MetalShared,
    name: &str,
    width: usize,
) -> Option<Q8Job> {
    let view = mapped.tensors.get(name)?;
    if view.info.ggml_type != 8 || width == 0 {
        return None;
    }
    let blocks = width.div_ceil(Q8_K);
    let row_bytes = blocks * Q8_BLOCK;
    if row_bytes == 0 || view.len < row_bytes {
        return None;
    }
    let rows = view.len / row_bytes;
    let base = mapped.mmap.as_ptr() as usize;
    let offset = (view.data as usize).checked_sub(base)?;
    if offset.saturating_add(view.len) > metal.length as usize {
        return None;
    }
    Some(Q8Job {
        offset,
        rows,
        width,
    })
}

fn resolve_f32(mapped: &MappedWeights, metal: &MetalShared, name: &str) -> Option<(usize, usize)> {
    let view = mapped.tensors.get(name)?;
    if view.info.ggml_type != 0 || view.len < 4 {
        return None;
    }
    let n = view.len / 4;
    let base = mapped.mmap.as_ptr() as usize;
    let offset = (view.data as usize).checked_sub(base)?;
    if offset.saturating_add(view.len) > metal.length as usize {
        return None;
    }
    Some((offset, n))
}

pub fn reset_hybrid_state() {
    #[cfg(target_os = "macos")]
    macos::reset_resident();
}

pub fn flush_hybrid_hidden(n: usize) -> Option<Vec<f32>> {
    #[cfg(target_os = "macos")]
    {
        macos::flush_hidden(n)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = n;
        None
    }
}

pub fn run_hybrid_layer(
    mapped: &MappedWeights,
    spec: &HybridSpec,
    index: usize,
    input: &[f32],
    slot: usize,
    n_hybrid: usize,
) -> Option<Vec<f32>> {
    if !bound() || spec.ffn == 0 || input.len() != spec.hidden {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        macos::run_hybrid_layer(mapped, spec, index, input, slot, n_hybrid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (mapped, spec, index, input, slot, n_hybrid);
        None
    }
}

pub fn run_full_layer(
    mapped: &MappedWeights,
    spec: &HybridSpec,
    attn: &AttnSpec,
    index: usize,
    input: &[f32],
    slot: usize,
    n_full: usize,
    position: usize,
) -> Option<Vec<f32>> {
    if !bound() || spec.ffn == 0 || input.len() != spec.hidden {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        macos::run_full_layer(mapped, spec, attn, index, input, slot, n_full, position)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (mapped, spec, attn, index, input, slot, n_full, position);
        None
    }
}

pub fn gpu_attn_live() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::gpu_attn_live()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn begin_gpu_token() {
    #[cfg(target_os = "macos")]
    macos::begin_token();
}

pub fn try_greedy_id(mapped: &MappedWeights, hidden: &[f32]) -> Option<u32> {
    if !bound() || hidden.is_empty() {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        macos::try_greedy_id(mapped, hidden)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (mapped, hidden);
        None
    }
}

#[cfg(target_os = "macos")]
fn q8_matvec(
    metal: &MetalShared,
    offset: usize,
    rows: usize,
    width: usize,
    x: &[f32],
) -> Option<Vec<f32>> {
    macos::q8_matvec(metal, offset, rows, width, x)
}

#[cfg(target_os = "macos")]
fn q8_matvec_many(metal: &MetalShared, jobs: &[Q8Job], x: &[f32]) -> Option<Vec<Vec<f32>>> {
    macos::q8_matvec_many(metal, jobs, x)
}

#[cfg(not(target_os = "macos"))]
fn q8_matvec_many(_metal: &MetalShared, _jobs: &[Q8Job], _x: &[f32]) -> Option<Vec<Vec<f32>>> {
    None
}

#[cfg(target_os = "macos")]
fn q8_ffn(
    metal: &MetalShared,
    gate: &Q8Job,
    up: &Q8Job,
    down: &Q8Job,
    x: &[f32],
) -> Option<Vec<f32>> {
    macos::q8_ffn(metal, gate, up, down, x)
}

#[cfg(not(target_os = "macos"))]
fn q8_ffn(
    _metal: &MetalShared,
    _gate: &Q8Job,
    _up: &Q8Job,
    _down: &Q8Job,
    _x: &[f32],
) -> Option<Vec<f32>> {
    None
}

#[cfg(not(target_os = "macos"))]
fn q8_matvec(
    _metal: &MetalShared,
    _offset: usize,
    _rows: usize,
    _width: usize,
    _x: &[f32],
) -> Option<Vec<f32>> {
    None
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    const STORAGE_SHARED: usize = 0;
    const THREADS: usize = 256;
    const Q8_ROWS: usize = 4;

    const SHADER: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct Params {
    uint rows;
    uint width;
    uint off_lo;
    uint off_hi;
};

kernel void q8_matvec(
    device const uchar *weights [[buffer(0)]],
    device const float *x [[buffer(1)]],
    device float *y [[buffer(2)]],
    constant Params &p [[buffer(3)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]],
    ushort tiisg [[thread_index_in_simdgroup]],
    ushort sgitg [[simdgroup_index_in_threadgroup]])
{
    const uint nr0 = 4u;
    const uint nsg = 8u;
    uint r0 = gid * nr0;
    const uint qk = 32u;
    const uint qb = 34u;
    uint blocks = (p.width + qk - 1u) / qk;
    ulong off = ulong(p.off_lo) | (ulong(p.off_hi) << 32u);
    uint row_bytes = blocks * qb;
    uint ix = uint(tiisg) / 4u;
    uint il = uint(tiisg) % 4u;
    uint ib0 = uint(sgitg) * 8u + ix;
    float sumv[4] = {0.0f, 0.0f, 0.0f, 0.0f};
    if (r0 < p.rows) {
        for (uint ib = ib0; ib < blocks; ib += nsg * 8u) {
            uint xbase = ib * qk + il * 8u;
            float4 yl0 = 0.0f;
            float4 yl1 = 0.0f;
            if (xbase + 7u < p.width) {
                yl0 = float4(x[xbase], x[xbase + 1u], x[xbase + 2u], x[xbase + 3u]);
                yl1 = float4(x[xbase + 4u], x[xbase + 5u], x[xbase + 6u], x[xbase + 7u]);
            } else {
                for (uint i = 0u; i < 4u; i++) {
                    if (xbase + i < p.width) {
                        yl0[i] = x[xbase + i];
                    }
                    if (xbase + 4u + i < p.width) {
                        yl1[i] = x[xbase + 4u + i];
                    }
                }
            }
            for (uint row = 0u; row < nr0; row++) {
                if (r0 + row >= p.rows) {
                    break;
                }
                device const uchar *blk = weights + off + (r0 + row) * row_bytes + ib * qb;
                float d = float(as_type<half>(ushort(uint(blk[0]) | (uint(blk[1]) << 8u))));
                uint qoff = 2u + il * 8u;
                float4 q0 = float4(
                    float(char(blk[qoff])),
                    float(char(blk[qoff + 1u])),
                    float(char(blk[qoff + 2u])),
                    float(char(blk[qoff + 3u])));
                float4 q1 = float4(
                    float(char(blk[qoff + 4u])),
                    float(char(blk[qoff + 5u])),
                    float(char(blk[qoff + 6u])),
                    float(char(blk[qoff + 7u])));
                sumv[row] += (dot(q0, yl0) + dot(q1, yl1)) * d;
            }
        }
    }
    threadgroup float red[4][8];
    for (uint row = 0u; row < nr0; row++) {
        sumv[row] = simd_sum(sumv[row]);
        if (tiisg == 0u) {
            red[row][sgitg] = sumv[row];
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (tid == 0u && r0 < p.rows) {
        for (uint row = 0u; row < nr0; row++) {
            if (r0 + row >= p.rows) {
                break;
            }
            float s = 0.0f;
            for (uint g = 0u; g < nsg; g++) {
                s += red[row][g];
            }
            y[r0 + row] = s;
        }
    }
}

kernel void silu_mul(
    device const float *g [[buffer(0)]],
    device const float *u [[buffer(1)]],
    device float *h [[buffer(2)]],
    constant uint &n [[buffer(3)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint i = gid * 256u + tid;
    if (i >= n) {
        return;
    }
    float v = g[i];
    h[i] = (v / (1.0f + exp(-v))) * u[i];
}

struct RmsP { uint n; float eps; };

kernel void rms_norm(
    device const float *x [[buffer(0)]],
    device const float *w [[buffer(1)]],
    device float *y [[buffer(2)]],
    constant RmsP &p [[buffer(3)]],
    uint tid [[thread_index_in_threadgroup]])
{
    threadgroup float partial[256];
    float ss = 0.0f;
    for (uint i = tid; i < p.n; i += 256u) {
        ss += x[i] * x[i];
    }
    partial[tid] = ss;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = 128u; s > 0u; s >>= 1u) {
        if (tid < s) {
            partial[tid] += partial[tid + s];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    float scale = rsqrt(partial[0] / float(p.n) + p.eps);
    for (uint i = tid; i < p.n; i += 256u) {
        y[i] = x[i] * scale * w[i];
    }
}

kernel void add_vec(
    device const float *a [[buffer(0)]],
    device const float *b [[buffer(1)]],
    device float *y [[buffer(2)]],
    constant uint &n [[buffer(3)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint i = gid * 256u + tid;
    if (i < n) {
        y[i] = a[i] + b[i];
    }
}

struct ConvP { uint channels; uint ksize; uint taps; };

kernel void conv1d_silu(
    device const float *input [[buffer(0)]],
    device float *state [[buffer(1)]],
    device const float *w [[buffer(2)]],
    device float *output [[buffer(3)]],
    constant ConvP &p [[buffer(4)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint row = gid * 256u + tid;
    if (row >= p.channels) {
        return;
    }
    device const float *rw = w + row * p.ksize;
    device float *rs = state + row * p.taps;
    float acc = input[row] * rw[p.taps];
    for (uint t = 0u; t < p.taps; t++) {
        acc += rs[t] * rw[t];
    }
    output[row] = acc / (1.0f + exp(-acc));
    if (p.taps > 0u) {
        for (uint t = 0u; t + 1u < p.taps; t++) {
            rs[t] = rs[t + 1u];
        }
        rs[p.taps - 1u] = input[row];
    }
}

kernel void decay_beta(
    device const float *alpha [[buffer(0)]],
    device const float *beta [[buffer(1)]],
    device const float *a [[buffer(2)]],
    device const float *dt [[buffer(3)]],
    device float *decay [[buffer(4)]],
    device float *bsig [[buffer(5)]],
    constant uint &n [[buffer(6)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint i = gid * 256u + tid;
    if (i >= n) {
        return;
    }
    float v = alpha[i] + dt[i];
    float sp = v > 20.0f ? v : log(1.0f + exp(v));
    decay[i] = exp(sp * a[i]);
    bsig[i] = 1.0f / (1.0f + exp(-beta[i]));
}

struct DeltaP {
    uint dim;
    uint rank;
    uint groups;
    uint q_size;
    uint v_off;
    uint reordered;
};

kernel void delta_heads(
    device const float *conv [[buffer(0)]],
    device const float *decay [[buffer(1)]],
    device const float *bsig [[buffer(2)]],
    device float *state [[buffer(3)]],
    device float *out [[buffer(4)]],
    constant DeltaP &p [[buffer(5)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint vh = gid;
    uint dim = p.dim;
    if (vh >= p.rank || tid >= dim) {
        return;
    }
    uint kh = p.reordered != 0u ? (vh % p.groups) : (p.groups == 0u ? 0u : vh / max(p.rank / p.groups, 1u));
    device const float *q = conv + kh * dim;
    device const float *k = conv + p.q_size + kh * dim;
    device const float *v = conv + p.v_off + vh * dim;
    device float *st = state + vh * dim * dim;
    threadgroup float nq[128];
    threadgroup float nk[128];
    threadgroup float qss[128];
    threadgroup float kss[128];
    qss[tid] = q[tid] * q[tid];
    kss[tid] = k[tid] * k[tid];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = 64u; s > 0u; s >>= 1u) {
        if (tid < s) {
            qss[tid] += qss[tid + s];
            kss[tid] += kss[tid + s];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    float qn = max(sqrt(qss[0]), 1e-6f);
    float kn = max(sqrt(kss[0]), 1e-6f);
    float scale = rsqrt(float(dim));
    nq[tid] = q[tid] / qn * scale;
    nk[tid] = k[tid] / kn;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float dcy = decay[vh];
    float bt = bsig[vh];
    float kv = 0.0f;
    for (uint col = 0u; col < dim; col++) {
        float s = st[tid * dim + col] * dcy;
        st[tid * dim + col] = s;
        kv += s * nk[col];
    }
    float del = (v[tid] - kv) * bt;
    for (uint col = 0u; col < dim; col++) {
        st[tid * dim + col] += del * nk[col];
    }
    float acc = 0.0f;
    for (uint col = 0u; col < dim; col++) {
        acc += st[tid * dim + col] * nq[col];
    }
    out[vh * dim + tid] = acc;
}

struct HeadRmsP { uint dim; uint rank; float eps; };

kernel void per_head_rms(
    device const float *x [[buffer(0)]],
    device const float *w [[buffer(1)]],
    device float *y [[buffer(2)]],
    constant HeadRmsP &p [[buffer(3)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint dim = p.dim;
    if (gid >= p.rank || tid >= dim) {
        return;
    }
    device const float *head = x + gid * dim;
    threadgroup float ss[128];
    ss[tid] = head[tid] * head[tid];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = 64u; s > 0u; s >>= 1u) {
        if (tid < s) {
            ss[tid] += ss[tid + s];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    float scale = rsqrt(ss[0] / float(dim) + p.eps);
    y[gid * dim + tid] = head[tid] * scale * w[tid];
}

kernel void copy_vec(
    device const float *a [[buffer(0)]],
    device float *y [[buffer(1)]],
    constant uint &n [[buffer(2)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint i = gid * 256u + tid;
    if (i < n) {
        y[i] = a[i];
    }
}

struct SplitP { uint heads; uint dim; };

kernel void split_qg(
    device const float *qg [[buffer(0)]],
    device float *q [[buffer(1)]],
    device float *g [[buffer(2)]],
    constant SplitP &p [[buffer(3)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    if (gid >= p.heads || tid >= p.dim) {
        return;
    }
    uint src = gid * p.dim * 2u + tid;
    uint dst = gid * p.dim + tid;
    q[dst] = qg[src];
    g[dst] = qg[src + p.dim];
}

kernel void per_head_rms256(
    device const float *x [[buffer(0)]],
    device const float *w [[buffer(1)]],
    device float *y [[buffer(2)]],
    constant HeadRmsP &p [[buffer(3)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint dim = p.dim;
    if (gid >= p.rank || tid >= dim) {
        return;
    }
    device const float *head = x + gid * dim;
    threadgroup float ss[256];
    ss[tid] = head[tid] * head[tid];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = 128u; s > 0u; s >>= 1u) {
        if (tid < s) {
            ss[tid] += ss[tid + s];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    float scale = rsqrt(ss[0] / float(dim) + p.eps);
    y[gid * dim + tid] = head[tid] * scale * w[tid];
}

struct RopeP {
    uint n_heads;
    uint head_dim;
    uint rotary;
    uint has_mrope;
    float pos0;
    float theta_scale;
    uint s0;
    uint s1;
    uint s2;
    uint s3;
};

kernel void rope_neox(
    device float *x [[buffer(0)]],
    constant RopeP &p [[buffer(1)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint h = gid;
    uint pairs = p.rotary / 2u;
    if (h >= p.n_heads || tid >= pairs) {
        return;
    }
    uint pair = tid;
    uint base = h * p.head_dim;
    uint i1 = base + pair;
    uint i2 = base + pair + pairs;
    if (i2 >= base + p.head_dim) {
        return;
    }
    float base_pos = p.pos0;
    if (p.has_mrope != 0u) {
        uint sect = p.s0 + p.s1 + p.s2 + p.s3;
        if (sect > 0u) {
            uint sector = pair % sect;
            uint e = p.s0 + p.s1 + p.s2;
            if (sector >= e) {
                base_pos = 0.0f;
            }
        }
    }
    float tb = base_pos * pow(p.theta_scale, float(pair));
    float c = cos(tb);
    float s = sin(tb);
    float x0 = x[i1];
    float x1 = x[i2];
    x[i1] = x0 * c - x1 * s;
    x[i2] = x0 * s + x1 * c;
}

struct AttnP {
    uint heads;
    uint kv_heads;
    uint dim;
    uint seq;
    uint kv_width;
    uint n_ctx;
    uint slot;
    float scale;
};

kernel void write_kv(
    device const float *k_now [[buffer(0)]],
    device const float *v_now [[buffer(1)]],
    device float *k_cache [[buffer(2)]],
    device float *v_cache [[buffer(3)]],
    constant AttnP &p [[buffer(4)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint i = gid * 256u + tid;
    if (i >= p.kv_width || p.seq == 0u) {
        return;
    }
    uint off = (p.slot * p.n_ctx + (p.seq - 1u)) * p.kv_width + i;
    k_cache[off] = k_now[i];
    v_cache[off] = v_now[i];
}

kernel void attend_decode(
    device const float *q [[buffer(0)]],
    device const float *k_cache [[buffer(1)]],
    device const float *v_cache [[buffer(2)]],
    device float *out [[buffer(3)]],
    constant AttnP &p [[buffer(4)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint h = gid;
    uint dim = p.dim;
    uint seq = p.seq;
    uint group = p.heads / max(p.kv_heads, 1u);
    uint kvh = min(h / max(group, 1u), p.kv_heads - 1u);
    device const float *qh = q + h * dim;
    threadgroup float logits[4096];
    if (tid == 0u) {
        float m = -INFINITY;
        for (uint t = 0u; t < seq; t++) {
            device const float *kh = k_cache + (p.slot * p.n_ctx + t) * p.kv_width + kvh * dim;
            float s = 0.0f;
            for (uint i = 0u; i < dim; i++) {
                s += qh[i] * kh[i];
            }
            float logit = s * p.scale;
            logits[t] = logit;
            m = max(m, logit);
        }
        float den = 0.0f;
        for (uint t = 0u; t < seq; t++) {
            float e = exp(logits[t] - m);
            logits[t] = e;
            den += e;
        }
        den = max(den, FLT_MIN);
        for (uint t = 0u; t < seq; t++) {
            logits[t] /= den;
        }
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (tid >= dim) {
        return;
    }
    float acc = 0.0f;
    for (uint t = 0u; t < seq; t++) {
        device const float *vh = v_cache + (p.slot * p.n_ctx + t) * p.kv_width + kvh * dim;
        acc += vh[tid] * logits[t];
    }
    out[h * dim + tid] = acc;
}

kernel void sig_mul(
    device const float *a [[buffer(0)]],
    device const float *g [[buffer(1)]],
    device float *y [[buffer(2)]],
    constant uint &n [[buffer(3)]],
    uint tid [[thread_index_in_threadgroup]],
    uint gid [[threadgroup_position_in_grid]])
{
    uint i = gid * 256u + tid;
    if (i >= n) {
        return;
    }
    y[i] = a[i] * (1.0f / (1.0f + exp(-g[i])));
}

kernel void argmax_f32(
    device const float *y [[buffer(0)]],
    device uint *out [[buffer(1)]],
    constant uint &n [[buffer(2)]],
    uint tid [[thread_index_in_threadgroup]])
{
    uint best_i = tid;
    float best_v = tid < n ? y[tid] : -INFINITY;
    for (uint i = tid + 256u; i < n; i += 256u) {
        float v = y[i];
        if (v > best_v || (v == best_v && i < best_i)) {
            best_v = v;
            best_i = i;
        }
    }
    threadgroup float vs[256];
    threadgroup uint is[256];
    vs[tid] = best_v;
    is[tid] = best_i;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = 128u; s > 0u; s >>= 1u) {
        if (tid < s) {
            if (vs[tid + s] > vs[tid] || (vs[tid + s] == vs[tid] && is[tid + s] < is[tid])) {
                vs[tid] = vs[tid + s];
                is[tid] = is[tid + s];
            }
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (tid == 0u) {
        out[0] = is[0];
    }
}
"#;

    #[repr(C)]
    struct Params {
        rows: u32,
        width: u32,
        off_lo: u32,
        off_hi: u32,
    }

    type Msg0 = unsafe extern "C" fn(*mut c_void, *const c_void) -> *mut c_void;
    type Msg1 = unsafe extern "C" fn(*mut c_void, *const c_void, *mut c_void) -> *mut c_void;
    type MsgBuf = unsafe extern "C" fn(*mut c_void, *const c_void, usize, usize) -> *mut c_void;
    type MsgLib = unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        *mut c_void,
        *mut c_void,
        *mut *mut c_void,
    ) -> *mut c_void;
    type MsgPipe = unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        *mut c_void,
        *mut *mut c_void,
    ) -> *mut c_void;
    type MsgSetBuf = unsafe extern "C" fn(*mut c_void, *const c_void, *mut c_void, usize, usize);
    type MsgSetBytes =
        unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, usize, usize);
    extern "C" {
        fn oa_metal_dispatch(
            encoder: *mut c_void,
            sel: *const c_void,
            groups: usize,
            threads: usize,
        );
    }
    type MsgVoid = unsafe extern "C" fn(*mut c_void, *const c_void);

    struct ComputeFns {
        get_class: unsafe extern "C" fn(*const i8) -> *mut c_void,
        retain: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        release: unsafe extern "C" fn(*mut c_void),
        msg0: Msg0,
        msg1: Msg1,
        msg_buf: MsgBuf,
        msg_lib: MsgLib,
        msg_pipe: MsgPipe,
        msg_set_buf: MsgSetBuf,
        msg_set_bytes: MsgSetBytes,
        msg_void: MsgVoid,
        sel_new_queue: *const c_void,
        sel_new_lib: *const c_void,
        sel_new_fn: *const c_void,
        sel_new_pipe: *const c_void,
        sel_new_buf: *const c_void,
        sel_contents: *const c_void,
        sel_cmd: *const c_void,
        sel_enc: *const c_void,
        sel_set_pipe: *const c_void,
        sel_set_buf: *const c_void,
        sel_set_bytes: *const c_void,
        sel_dispatch: *const c_void,
        sel_end: *const c_void,
        sel_commit: *const c_void,
        sel_wait: *const c_void,
        sel_utf8: *const c_void,
    }

    unsafe impl Send for ComputeFns {}
    unsafe impl Sync for ComputeFns {}

    static FNS: OnceLock<Option<ComputeFns>> = OnceLock::new();

    fn fns() -> Option<&'static ComputeFns> {
        FNS.get_or_init(|| unsafe { load() }).as_ref()
    }

    unsafe fn load() -> Option<ComputeFns> {
        extern "C" {
            fn dlopen(filename: *const i8, flags: i32) -> *mut c_void;
            fn dlsym(handle: *mut c_void, symbol: *const i8) -> *mut c_void;
        }
        const RTLD_LAZY: i32 = 1;
        let objc = dlopen(c_path(b"/usr/lib/libobjc.A.dylib\0")?, RTLD_LAZY);
        let _foundation = dlopen(
            c_path(b"/System/Library/Frameworks/Foundation.framework/Foundation\0")?,
            RTLD_LAZY,
        );
        if objc.is_null() {
            return None;
        }
        let sel: unsafe extern "C" fn(*const i8) -> *const c_void =
            transmute_sym(dlsym(objc, c_path(b"sel_registerName\0")?))?;
        let get_class: unsafe extern "C" fn(*const i8) -> *mut c_void =
            transmute_sym(dlsym(objc, c_path(b"objc_getClass\0")?))?;
        let retain: unsafe extern "C" fn(*mut c_void) -> *mut c_void =
            transmute_sym(dlsym(objc, c_path(b"objc_retain\0")?))?;
        let release: unsafe extern "C" fn(*mut c_void) =
            transmute_sym(dlsym(objc, c_path(b"objc_release\0")?))?;
        let msg = dlsym(objc, c_path(b"objc_msgSend\0")?);
        if msg.is_null() {
            return None;
        }
        let s = |name: &[u8]| -> Option<*const c_void> {
            let sel = sel(c_path(name)?);
            if sel.is_null() {
                None
            } else {
                Some(sel)
            }
        };
        Some(ComputeFns {
            get_class,
            retain,
            release,
            msg0: transmute_sym(msg)?,
            msg1: transmute_sym(msg)?,
            msg_buf: transmute_sym(msg)?,
            msg_lib: transmute_sym(msg)?,
            msg_pipe: transmute_sym(msg)?,
            msg_set_buf: transmute_sym(msg)?,
            msg_set_bytes: transmute_sym(msg)?,
            msg_void: transmute_sym(msg)?,
            sel_new_queue: s(b"newCommandQueue\0")?,
            sel_new_lib: s(b"newLibraryWithSource:options:error:\0")?,
            sel_new_fn: s(b"newFunctionWithName:\0")?,
            sel_new_pipe: s(b"newComputePipelineStateWithFunction:error:\0")?,
            sel_new_buf: s(b"newBufferWithLength:options:\0")?,
            sel_contents: s(b"contents\0")?,
            sel_cmd: s(b"commandBuffer\0")?,
            sel_enc: s(b"computeCommandEncoder\0")?,
            sel_set_pipe: s(b"setComputePipelineState:\0")?,
            sel_set_buf: s(b"setBuffer:offset:atIndex:\0")?,
            sel_set_bytes: s(b"setBytes:length:atIndex:\0")?,
            sel_dispatch: s(b"dispatchThreadgroups:threadsPerThreadgroup:\0")?,
            sel_end: s(b"endEncoding\0")?,
            sel_commit: s(b"commit\0")?,
            sel_wait: s(b"waitUntilCompleted\0")?,
            sel_utf8: s(b"stringWithUTF8String:\0")?,
        })
    }

    fn c_path(bytes: &[u8]) -> Option<*const i8> {
        CStr::from_bytes_with_nul(bytes).ok().map(|s| s.as_ptr())
    }

    unsafe fn transmute_sym<T>(ptr: *mut c_void) -> Option<T> {
        if ptr.is_null() {
            None
        } else {
            Some(std::mem::transmute_copy(&ptr))
        }
    }

    struct Pipeline {
        queue: *mut c_void,
        pipeline: *mut c_void,
        silu: *mut c_void,
        rms: *mut c_void,
        add: *mut c_void,
        conv: *mut c_void,
        decay: *mut c_void,
        delta: *mut c_void,
        phrms: *mut c_void,
        copy: *mut c_void,
        split: *mut c_void,
        ph256: *mut c_void,
        rope: *mut c_void,
        writekv: *mut c_void,
        attn: *mut c_void,
        sig: *mut c_void,
        argmax: *mut c_void,
        release: unsafe extern "C" fn(*mut c_void),
    }

    unsafe impl Send for Pipeline {}
    unsafe impl Sync for Pipeline {}

    impl Drop for Pipeline {
        fn drop(&mut self) {
            unsafe {
                if !self.pipeline.is_null() {
                    (self.release)(self.pipeline);
                }
                if !self.silu.is_null() {
                    (self.release)(self.silu);
                }
                if !self.rms.is_null() {
                    (self.release)(self.rms);
                }
                if !self.add.is_null() {
                    (self.release)(self.add);
                }
                if !self.conv.is_null() {
                    (self.release)(self.conv);
                }
                if !self.decay.is_null() {
                    (self.release)(self.decay);
                }
                if !self.delta.is_null() {
                    (self.release)(self.delta);
                }
                if !self.phrms.is_null() {
                    (self.release)(self.phrms);
                }
                if !self.copy.is_null() {
                    (self.release)(self.copy);
                }
                if !self.split.is_null() {
                    (self.release)(self.split);
                }
                if !self.ph256.is_null() {
                    (self.release)(self.ph256);
                }
                if !self.rope.is_null() {
                    (self.release)(self.rope);
                }
                if !self.writekv.is_null() {
                    (self.release)(self.writekv);
                }
                if !self.attn.is_null() {
                    (self.release)(self.attn);
                }
                if !self.sig.is_null() {
                    (self.release)(self.sig);
                }
                if !self.argmax.is_null() {
                    (self.release)(self.argmax);
                }
                if !self.queue.is_null() {
                    (self.release)(self.queue);
                }
            }
        }
    }

    fn pipeline_for(device: *mut c_void) -> Option<&'static Pipeline> {
        static PIPE: OnceLock<Option<Pipeline>> = OnceLock::new();
        PIPE.get_or_init(|| unsafe { compile(device) }).as_ref()
    }

    struct Scratch {
        x: *mut c_void,
        y: *mut c_void,
        x_cap: usize,
        y_cap: usize,
    }

    fn scratch_pair(
        device: *mut c_void,
        x_bytes: usize,
        y_bytes: usize,
    ) -> Option<(*mut c_void, *mut c_void)> {
        use std::cell::RefCell;
        thread_local! {
            static SCRATCH: RefCell<Scratch> = const {
                RefCell::new(Scratch {
                    x: ptr::null_mut(),
                    y: ptr::null_mut(),
                    x_cap: 0,
                    y_cap: 0,
                })
            };
        }
        let fns = fns()?;
        SCRATCH.with(|slot| {
            let mut slot = slot.borrow_mut();
            unsafe {
                if slot.x_cap < x_bytes {
                    if !slot.x.is_null() {
                        (fns.release)(slot.x);
                    }
                    slot.x = (fns.msg_buf)(device, fns.sel_new_buf, x_bytes, STORAGE_SHARED);
                    slot.x_cap = if slot.x.is_null() { 0 } else { x_bytes };
                }
                if slot.y_cap < y_bytes {
                    if !slot.y.is_null() {
                        (fns.release)(slot.y);
                    }
                    slot.y = (fns.msg_buf)(device, fns.sel_new_buf, y_bytes, STORAGE_SHARED);
                    slot.y_cap = if slot.y.is_null() { 0 } else { y_bytes };
                }
                if slot.x.is_null() || slot.y.is_null() {
                    None
                } else {
                    Some((slot.x, slot.y))
                }
            }
        })
    }

    unsafe fn compile(device: *mut c_void) -> Option<Pipeline> {
        let fns = fns()?;
        let class = (fns.get_class)(c_path(b"NSString\0")?);
        if class.is_null() {
            return None;
        }
        let source = std::ffi::CString::new(SHADER).ok()?;
        let ns = (fns.msg1)(class, fns.sel_utf8, source.as_ptr() as *mut c_void);
        if ns.is_null() {
            return None;
        }
        let mut err = ptr::null_mut();
        let lib = (fns.msg_lib)(device, fns.sel_new_lib, ns, ptr::null_mut(), &mut err);
        if lib.is_null() {
            eprintln!("metal shader compile failed: {}", ns_error(fns, err));
            return None;
        }
        let func_of = |name: &str| -> Option<*mut c_void> {
            let cname = std::ffi::CString::new(name).ok()?;
            let fname = (fns.msg1)(class, fns.sel_utf8, cname.as_ptr() as *mut c_void);
            let func = (fns.msg1)(lib, fns.sel_new_fn, fname);
            if func.is_null() {
                None
            } else {
                Some(func)
            }
        };
        let names = [
            "q8_matvec",
            "silu_mul",
            "rms_norm",
            "add_vec",
            "conv1d_silu",
            "decay_beta",
            "delta_heads",
            "per_head_rms",
            "copy_vec",
            "split_qg",
            "per_head_rms256",
            "rope_neox",
            "write_kv",
            "attend_decode",
            "sig_mul",
            "argmax_f32",
        ];
        let mut funcs: [*mut c_void; 16] = [ptr::null_mut(); 16];
        for (i, name) in names.iter().enumerate() {
            if let Some(f) = func_of(name) {
                funcs[i] = f;
            } else if i < 2 {
                for f in funcs {
                    if !f.is_null() {
                        (fns.release)(f);
                    }
                }
                (fns.release)(lib);
                return None;
            }
        }
        (fns.release)(lib);
        let mut pipes: [*mut c_void; 16] = [ptr::null_mut(); 16];
        for (i, func) in funcs.into_iter().enumerate() {
            if func.is_null() {
                continue;
            }
            let mut err = ptr::null_mut();
            let p = (fns.msg_pipe)(device, fns.sel_new_pipe, func, &mut err);
            (fns.release)(func);
            if p.is_null() {
                if i < 2 {
                    for q in pipes {
                        if !q.is_null() {
                            (fns.release)(q);
                        }
                    }
                    return None;
                }
                continue;
            }
            pipes[i] = p;
        }
        let queue = (fns.msg0)(device, fns.sel_new_queue);
        if queue.is_null() {
            for p in pipes {
                if !p.is_null() {
                    (fns.release)(p);
                }
            }
            return None;
        }
        Some(Pipeline {
            queue,
            pipeline: pipes[0],
            silu: pipes[1],
            rms: pipes[2],
            add: pipes[3],
            conv: pipes[4],
            decay: pipes[5],
            delta: pipes[6],
            phrms: pipes[7],
            copy: pipes[8],
            split: pipes[9],
            ph256: pipes[10],
            rope: pipes[11],
            writekv: pipes[12],
            attn: pipes[13],
            sig: pipes[14],
            argmax: pipes[15],
            release: fns.release,
        })
    }

    unsafe fn encode_q8(
        fns: &ComputeFns,
        enc: *mut c_void,
        pipe: *mut c_void,
        weights: *mut c_void,
        x_buf: *mut c_void,
        x_off: usize,
        y_buf: *mut c_void,
        y_off: usize,
        job: &Q8Job,
    ) {
        (fns.msg1)(enc, fns.sel_set_pipe, pipe);
        (fns.msg_set_buf)(enc, fns.sel_set_buf, weights, 0, 0);
        (fns.msg_set_buf)(enc, fns.sel_set_buf, x_buf, x_off, 1);
        (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, y_off, 2);
        let params = Params {
            rows: job.rows as u32,
            width: job.width as u32,
            off_lo: job.offset as u32,
            off_hi: (job.offset >> 32) as u32,
        };
        (fns.msg_set_bytes)(
            enc,
            fns.sel_set_bytes,
            (&params as *const Params).cast(),
            std::mem::size_of::<Params>(),
            3,
        );
        oa_metal_dispatch(enc, fns.sel_dispatch, job.rows.div_ceil(Q8_ROWS), THREADS);
    }

    pub(super) fn q8_matvec_many(
        metal: &MetalShared,
        jobs: &[Q8Job],
        x: &[f32],
    ) -> Option<Vec<Vec<f32>>> {
        if jobs.is_empty() {
            return Some(Vec::new());
        }
        if jobs.iter().any(|j| j.width != x.len() || j.rows == 0) {
            return None;
        }
        let fns = fns()?;
        let pipe = pipeline_for(metal.device)?;
        let y_elems: usize = jobs.iter().map(|j| j.rows).sum();
        unsafe {
            let (x_buf, y_buf) = scratch_pair(metal.device, x.len() * 4, y_elems * 4)?;
            let x_ptr = (fns.msg0)(x_buf, fns.sel_contents) as *mut f32;
            if x_ptr.is_null() {
                return None;
            }
            ptr::copy_nonoverlapping(x.as_ptr(), x_ptr, x.len());
            let cmd = (fns.msg0)(pipe.queue, fns.sel_cmd);
            let enc = (fns.msg0)(cmd, fns.sel_enc);
            if cmd.is_null() || enc.is_null() {
                return None;
            }
            let mut y_off = 0usize;
            for job in jobs {
                encode_q8(
                    fns,
                    enc,
                    pipe.pipeline,
                    metal.buffer,
                    x_buf,
                    0,
                    y_buf,
                    y_off * 4,
                    job,
                );
                y_off += job.rows;
            }
            (fns.msg_void)(enc, fns.sel_end);
            (fns.msg_void)(cmd, fns.sel_commit);
            (fns.msg_void)(cmd, fns.sel_wait);
            let y_ptr = (fns.msg0)(y_buf, fns.sel_contents) as *const f32;
            if y_ptr.is_null() {
                return None;
            }
            let mut out = Vec::with_capacity(jobs.len());
            let mut off = 0usize;
            for job in jobs {
                let mut row = vec![0f32; job.rows];
                ptr::copy_nonoverlapping(y_ptr.add(off), row.as_mut_ptr(), job.rows);
                out.push(row);
                off += job.rows;
            }
            Some(out)
        }
    }

    pub(super) fn q8_ffn(
        metal: &MetalShared,
        gate: &Q8Job,
        up: &Q8Job,
        down: &Q8Job,
        x: &[f32],
    ) -> Option<Vec<f32>> {
        if gate.rows != up.rows || gate.width != x.len() || down.width != gate.rows {
            return None;
        }
        let fns = fns()?;
        let pipe = pipeline_for(metal.device)?;
        let hid = gate.rows;
        let y_elems = hid * 3 + down.rows;
        unsafe {
            let (x_buf, y_buf) = scratch_pair(metal.device, x.len() * 4, y_elems * 4)?;
            let x_ptr = (fns.msg0)(x_buf, fns.sel_contents) as *mut f32;
            if x_ptr.is_null() {
                return None;
            }
            ptr::copy_nonoverlapping(x.as_ptr(), x_ptr, x.len());
            let cmd = (fns.msg0)(pipe.queue, fns.sel_cmd);
            let enc = (fns.msg0)(cmd, fns.sel_enc);
            if cmd.is_null() || enc.is_null() {
                return None;
            }
            let gate_off = 0usize;
            let up_off = hid;
            let hid_off = hid * 2;
            let down_off = hid * 3;
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                x_buf,
                0,
                y_buf,
                gate_off * 4,
                gate,
            );
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                x_buf,
                0,
                y_buf,
                up_off * 4,
                up,
            );
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.silu);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, gate_off * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, up_off * 4, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, hid_off * 4, 2);
            let n = hid as u32;
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&n as *const u32).cast(),
                std::mem::size_of::<u32>(),
                3,
            );
            const SILU_THREADS: usize = 256;
            oa_metal_dispatch(
                enc,
                fns.sel_dispatch,
                hid.div_ceil(SILU_THREADS),
                SILU_THREADS,
            );
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                hid_off * 4,
                y_buf,
                down_off * 4,
                down,
            );
            (fns.msg_void)(enc, fns.sel_end);
            (fns.msg_void)(cmd, fns.sel_commit);
            (fns.msg_void)(cmd, fns.sel_wait);
            let y_ptr = (fns.msg0)(y_buf, fns.sel_contents) as *const f32;
            if y_ptr.is_null() {
                return None;
            }
            let mut out = vec![0f32; down.rows];
            ptr::copy_nonoverlapping(y_ptr.add(down_off), out.as_mut_ptr(), down.rows);
            Some(out)
        }
    }

    pub(super) fn q8_matvec(
        metal: &MetalShared,
        offset: usize,
        rows: usize,
        width: usize,
        x: &[f32],
    ) -> Option<Vec<f32>> {
        if rows == 0 || width != x.len() {
            return None;
        }
        let fns = fns()?;
        let pipe = pipeline_for(metal.device)?;
        unsafe {
            let x_bytes = x.len() * 4;
            let y_bytes = rows * 4;
            let (x_buf, y_buf) = scratch_pair(metal.device, x_bytes, y_bytes)?;
            let x_ptr = (fns.msg0)(x_buf, fns.sel_contents) as *mut f32;
            if x_ptr.is_null() {
                return None;
            }
            ptr::copy_nonoverlapping(x.as_ptr(), x_ptr, x.len());
            let cmd = (fns.msg0)(pipe.queue, fns.sel_cmd);
            let enc = (fns.msg0)(cmd, fns.sel_enc);
            if cmd.is_null() || enc.is_null() {
                return None;
            }
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.pipeline);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, metal.buffer, 0, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, x_buf, 0, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, 0, 2);
            let params = Params {
                rows: rows as u32,
                width: width as u32,
                off_lo: offset as u32,
                off_hi: (offset >> 32) as u32,
            };
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&params as *const Params).cast(),
                std::mem::size_of::<Params>(),
                3,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, rows.div_ceil(Q8_ROWS), THREADS);
            (fns.msg_void)(enc, fns.sel_end);
            (fns.msg_void)(cmd, fns.sel_commit);
            (fns.msg_void)(cmd, fns.sel_wait);
            let y_ptr = (fns.msg0)(y_buf, fns.sel_contents) as *const f32;
            let mut out = vec![0f32; rows];
            if !y_ptr.is_null() {
                ptr::copy_nonoverlapping(y_ptr, out.as_mut_ptr(), rows);
            }
            Some(out)
        }
    }

    #[repr(C)]
    struct RmsP {
        n: u32,
        eps: f32,
    }

    #[repr(C)]
    struct ConvP {
        channels: u32,
        ksize: u32,
        taps: u32,
    }

    #[repr(C)]
    struct DeltaP {
        dim: u32,
        rank: u32,
        groups: u32,
        q_size: u32,
        v_off: u32,
        reordered: u32,
    }

    #[repr(C)]
    struct HeadRmsP {
        dim: u32,
        rank: u32,
        eps: f32,
    }

    #[repr(C)]
    struct SplitP {
        heads: u32,
        dim: u32,
    }

    #[repr(C)]
    struct RopeP {
        n_heads: u32,
        head_dim: u32,
        rotary: u32,
        has_mrope: u32,
        pos0: f32,
        theta_scale: f32,
        s0: u32,
        s1: u32,
        s2: u32,
        s3: u32,
    }

    #[repr(C)]
    struct AttnP {
        heads: u32,
        kv_heads: u32,
        dim: u32,
        seq: u32,
        kv_width: u32,
        n_ctx: u32,
        slot: u32,
        scale: f32,
    }

    struct Resident {
        device: *mut c_void,
        conv: *mut c_void,
        delta: *mut c_void,
        n_slots: usize,
        conv_stride: usize,
        delta_stride: usize,
        conv_bytes: usize,
        delta_bytes: usize,
    }

    thread_local! {
        static RESIDENT: std::cell::RefCell<Resident> = const {
            std::cell::RefCell::new(Resident {
                device: ptr::null_mut(),
                conv: ptr::null_mut(),
                delta: ptr::null_mut(),
                n_slots: 0,
                conv_stride: 0,
                delta_stride: 0,
                conv_bytes: 0,
                delta_bytes: 0,
            })
        };
    }

    struct GpuStream {
        device: *mut c_void,
        hidden: *mut c_void,
        hidden_n: usize,
        pending: bool,
        cmd: *mut c_void,
        enc: *mut c_void,
        open: bool,
    }

    thread_local! {
        static STREAM: std::cell::RefCell<GpuStream> = const {
            std::cell::RefCell::new(GpuStream {
                device: ptr::null_mut(),
                hidden: ptr::null_mut(),
                hidden_n: 0,
                pending: false,
                cmd: ptr::null_mut(),
                enc: ptr::null_mut(),
                open: false,
            })
        };
    }

    fn drain_queue(device: *mut c_void) {
        let Some(fns) = fns() else {
            return;
        };
        let Some(pipe) = pipeline_for(device) else {
            return;
        };
        unsafe {
            let cmd = (fns.msg0)(pipe.queue, fns.sel_cmd);
            if cmd.is_null() {
                return;
            }
            (fns.msg_void)(cmd, fns.sel_commit);
            (fns.msg_void)(cmd, fns.sel_wait);
        }
    }

    fn ensure_hidden(device: *mut c_void, n: usize) -> Option<*mut c_void> {
        if n == 0 {
            return None;
        }
        let fns = fns()?;
        STREAM.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.device == device && slot.hidden_n == n && !slot.hidden.is_null() {
                return Some(slot.hidden);
            }
            unsafe {
                close_stream(fns, &mut slot);
                if !slot.hidden.is_null() {
                    (fns.release)(slot.hidden);
                }
                let buf = (fns.msg_buf)(device, fns.sel_new_buf, n * 4, STORAGE_SHARED);
                if buf.is_null() {
                    slot.hidden = ptr::null_mut();
                    slot.hidden_n = 0;
                    slot.device = ptr::null_mut();
                    slot.pending = false;
                    return None;
                }
                slot.hidden = buf;
                slot.hidden_n = n;
                slot.device = device;
                slot.pending = false;
                Some(buf)
            }
        })
    }

    unsafe fn close_stream(fns: &ComputeFns, slot: &mut GpuStream) {
        if slot.open && !slot.enc.is_null() && !slot.cmd.is_null() {
            (fns.msg_void)(slot.enc, fns.sel_end);
            (fns.msg_void)(slot.cmd, fns.sel_commit);
            (fns.msg_void)(slot.cmd, fns.sel_wait);
            (fns.release)(slot.enc);
            (fns.release)(slot.cmd);
        } else if slot.pending && !slot.device.is_null() {
            drain_queue(slot.device);
        }
        slot.enc = ptr::null_mut();
        slot.cmd = ptr::null_mut();
        slot.open = false;
    }

    pub(super) fn flush_hidden(n: usize) -> Option<Vec<f32>> {
        let fns = fns()?;
        STREAM.with(|slot| {
            let mut slot = slot.borrow_mut();
            if !slot.pending || slot.hidden.is_null() || slot.hidden_n != n {
                return None;
            }
            unsafe {
                close_stream(fns, &mut slot);
                let p = (fns.msg0)(slot.hidden, fns.sel_contents) as *const f32;
                slot.pending = false;
                if p.is_null() {
                    return None;
                }
                let mut out = vec![0f32; n];
                ptr::copy_nonoverlapping(p, out.as_mut_ptr(), n);
                Some(out)
            }
        })
    }

    thread_local! {
        static GPU_ATTN: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn gpu_attn_live() -> bool {
        GPU_ATTN.with(|c| c.get())
    }

    pub(super) fn begin_token() {
        let Some(fns) = fns() else {
            return;
        };
        STREAM.with(|slot| {
            let mut slot = slot.borrow_mut();
            if !slot.pending && !slot.open {
                return;
            }
            unsafe {
                close_stream(fns, &mut slot);
            }
            slot.pending = false;
        });
    }

    struct KvResident {
        device: *mut c_void,
        k: *mut c_void,
        v: *mut c_void,
        n_slots: usize,
        n_ctx: usize,
        kv_width: usize,
        bytes: usize,
    }

    thread_local! {
        static KV: std::cell::RefCell<KvResident> = const {
            std::cell::RefCell::new(KvResident {
                device: ptr::null_mut(),
                k: ptr::null_mut(),
                v: ptr::null_mut(),
                n_slots: 0,
                n_ctx: 0,
                kv_width: 0,
                bytes: 0,
            })
        };
    }

    pub(super) fn reset_resident() {
        let Some(fns) = fns() else {
            return;
        };
        STREAM.with(|slot| {
            let mut slot = slot.borrow_mut();
            unsafe {
                close_stream(fns, &mut slot);
            }
            slot.pending = false;
        });
        RESIDENT.with(|slot| {
            let slot = slot.borrow();
            unsafe {
                zero_buf(fns, slot.conv, slot.conv_bytes);
                zero_buf(fns, slot.delta, slot.delta_bytes);
            }
        });
        KV.with(|slot| {
            let slot = slot.borrow();
            unsafe {
                zero_buf(fns, slot.k, slot.bytes);
                zero_buf(fns, slot.v, slot.bytes);
            }
        });
        GPU_ATTN.with(|c| c.set(false));
    }

    fn ensure_kv(
        device: *mut c_void,
        n_slots: usize,
        n_ctx: usize,
        kv_width: usize,
    ) -> Option<(*mut c_void, *mut c_void)> {
        if n_slots == 0 || n_ctx == 0 || kv_width == 0 {
            return None;
        }
        let fns = fns()?;
        KV.with(|slot| {
            let mut slot = slot.borrow_mut();
            let bytes = n_slots
                .checked_mul(n_ctx)?
                .checked_mul(kv_width)?
                .checked_mul(4)?;
            let reuse = slot.device == device
                && slot.n_slots == n_slots
                && slot.n_ctx == n_ctx
                && slot.kv_width == kv_width
                && !slot.k.is_null()
                && !slot.v.is_null();
            if reuse {
                return Some((slot.k, slot.v));
            }
            unsafe {
                if !slot.k.is_null() {
                    (fns.release)(slot.k);
                }
                if !slot.v.is_null() {
                    (fns.release)(slot.v);
                }
                slot.k = (fns.msg_buf)(device, fns.sel_new_buf, bytes, STORAGE_SHARED);
                slot.v = (fns.msg_buf)(device, fns.sel_new_buf, bytes, STORAGE_SHARED);
                if slot.k.is_null() || slot.v.is_null() {
                    if !slot.k.is_null() {
                        (fns.release)(slot.k);
                    }
                    if !slot.v.is_null() {
                        (fns.release)(slot.v);
                    }
                    slot.k = ptr::null_mut();
                    slot.v = ptr::null_mut();
                    slot.device = ptr::null_mut();
                    slot.n_slots = 0;
                    slot.bytes = 0;
                    return None;
                }
                slot.device = device;
                slot.n_slots = n_slots;
                slot.n_ctx = n_ctx;
                slot.kv_width = kv_width;
                slot.bytes = bytes;
                zero_buf(fns, slot.k, bytes);
                zero_buf(fns, slot.v, bytes);
                Some((slot.k, slot.v))
            }
        })
    }

    unsafe fn zero_buf(fns: &ComputeFns, buf: *mut c_void, bytes: usize) {
        if buf.is_null() || bytes == 0 {
            return;
        }
        let p = (fns.msg0)(buf, fns.sel_contents) as *mut u8;
        if !p.is_null() {
            ptr::write_bytes(p, 0, bytes);
        }
    }

    fn ensure_resident(
        device: *mut c_void,
        n_slots: usize,
        conv_stride: usize,
        delta_stride: usize,
    ) -> Option<(*mut c_void, *mut c_void)> {
        if n_slots == 0 || conv_stride == 0 || delta_stride == 0 {
            return None;
        }
        let fns = fns()?;
        RESIDENT.with(|slot| {
            let mut slot = slot.borrow_mut();
            let conv_bytes = n_slots.checked_mul(conv_stride)?.checked_mul(4)?;
            let delta_bytes = n_slots.checked_mul(delta_stride)?.checked_mul(4)?;
            let reuse = slot.device == device
                && slot.n_slots == n_slots
                && slot.conv_stride == conv_stride
                && slot.delta_stride == delta_stride
                && !slot.conv.is_null()
                && !slot.delta.is_null();
            if reuse {
                return Some((slot.conv, slot.delta));
            }
            unsafe {
                if !slot.conv.is_null() {
                    (fns.release)(slot.conv);
                }
                if !slot.delta.is_null() {
                    (fns.release)(slot.delta);
                }
                slot.conv = (fns.msg_buf)(device, fns.sel_new_buf, conv_bytes, STORAGE_SHARED);
                slot.delta = (fns.msg_buf)(device, fns.sel_new_buf, delta_bytes, STORAGE_SHARED);
                if slot.conv.is_null() || slot.delta.is_null() {
                    if !slot.conv.is_null() {
                        (fns.release)(slot.conv);
                    }
                    if !slot.delta.is_null() {
                        (fns.release)(slot.delta);
                    }
                    slot.conv = ptr::null_mut();
                    slot.delta = ptr::null_mut();
                    slot.device = ptr::null_mut();
                    slot.n_slots = 0;
                    slot.conv_bytes = 0;
                    slot.delta_bytes = 0;
                    return None;
                }
                slot.device = device;
                slot.n_slots = n_slots;
                slot.conv_stride = conv_stride;
                slot.delta_stride = delta_stride;
                slot.conv_bytes = conv_bytes;
                slot.delta_bytes = delta_bytes;
                zero_buf(fns, slot.conv, conv_bytes);
                zero_buf(fns, slot.delta, delta_bytes);
                Some((slot.conv, slot.delta))
            }
        })
    }

    fn f32_named(
        mapped: &MappedWeights,
        metal: &MetalShared,
        names: &[&str],
    ) -> Option<(usize, usize)> {
        names
            .iter()
            .find_map(|name| resolve_f32(mapped, metal, name))
    }

    fn ns_error(fns: &ComputeFns, err: *mut c_void) -> String {
        if err.is_null() {
            return String::from("unknown");
        }
        unsafe {
            extern "C" {
                fn dlopen(filename: *const i8, flags: i32) -> *mut c_void;
                fn dlsym(handle: *mut c_void, symbol: *const i8) -> *mut c_void;
            }
            let objc = dlopen(
                c_path(b"/usr/lib/libobjc.A.dylib\0").unwrap_or(ptr::null()),
                1,
            );
            let sel = match transmute_sym::<unsafe extern "C" fn(*const i8) -> *const c_void>(
                dlsym(objc, c_path(b"sel_registerName\0").unwrap_or(ptr::null())),
            ) {
                Some(s) => s,
                None => return String::from("nserror-sel"),
            };
            let desc_sel = sel(c_path(b"localizedDescription\0").unwrap_or(ptr::null()));
            let utf_sel = sel(c_path(b"UTF8String\0").unwrap_or(ptr::null()));
            if desc_sel.is_null() || utf_sel.is_null() {
                return String::from("nserror-sel");
            }
            let desc = (fns.msg0)(err, desc_sel);
            if desc.is_null() {
                return String::from("nserror-empty");
            }
            let cstr = (fns.msg0)(desc, utf_sel) as *const i8;
            if cstr.is_null() {
                return String::from("nserror-utf8");
            }
            CStr::from_ptr(cstr).to_string_lossy().into_owned()
        }
    }

    fn note_hybrid(msg: &str) {
        if std::env::var_os("OPENAGENTS_METAL_DEBUG").is_none() {
            return;
        }
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| eprintln!("hybrid-gpu: {msg}"));
    }

    pub(super) fn run_hybrid_layer(
        mapped: &MappedWeights,
        spec: &HybridSpec,
        index: usize,
        input: &[f32],
        slot: usize,
        n_slots: usize,
    ) -> Option<Vec<f32>> {
        if std::env::var_os("OPENAGENTS_HYBRID_CPU").is_some() {
            return None;
        }
        if spec.state_size != 128
            || spec.time_step_rank == 0
            || spec.group_count == 0
            || spec.inner_size != spec.time_step_rank * spec.state_size
            || spec.ffn == 0
            || slot >= n_slots
        {
            note_hybrid("skip-shape");
            return None;
        }
        let metal = unsafe { &*active_metal()? };
        let fns = match fns() {
            Some(f) => f,
            None => {
                note_hybrid("skip-fns");
                return None;
            }
        };
        let pipe = match pipeline_for(metal.device) {
            Some(p) => p,
            None => {
                note_hybrid("skip-pipeline");
                return None;
            }
        };
        if pipe.rms.is_null()
            || pipe.add.is_null()
            || pipe.conv.is_null()
            || pipe.decay.is_null()
            || pipe.delta.is_null()
            || pipe.phrms.is_null()
        {
            note_hybrid("skip-kernels");
            return None;
        }
        let p = format!("blk.{index}");
        let hidden = spec.hidden;
        let dim = spec.state_size;
        let rank = spec.time_step_rank;
        let groups = spec.group_count;
        let inner = spec.inner_size;
        let q_size = groups * dim;
        let v_off = q_size * 2;
        let qkv_w = v_off + inner;
        let ffn = spec.ffn;
        let kernel = spec.conv_kernel;
        let taps = kernel.saturating_sub(1);
        let conv_stride = qkv_w * taps;
        let delta_stride = rank * dim * dim;
        let (attn_n, attn_n_len) = resolve_f32(mapped, metal, &format!("{p}.attn_norm.weight"))?;
        let (post_n, post_n_len) =
            resolve_f32(mapped, metal, &format!("{p}.post_attention_norm.weight"))?;
        let (conv_w, conv_w_len) = resolve_f32(mapped, metal, &format!("{p}.ssm_conv1d.weight"))?;
        let (ssm_a, ssm_a_len) = f32_named(
            mapped,
            metal,
            &[&format!("{p}.ssm_a"), &format!("{p}.ssm_a.weight")],
        )?;
        let (ssm_dt, ssm_dt_len) = f32_named(
            mapped,
            metal,
            &[&format!("{p}.ssm_dt.bias"), &format!("{p}.ssm_dt")],
        )?;
        let (ssm_norm, ssm_norm_len) = resolve_f32(mapped, metal, &format!("{p}.ssm_norm.weight"))?;
        if attn_n_len < hidden
            || post_n_len < hidden
            || conv_w_len < qkv_w * kernel
            || ssm_a_len < rank
            || ssm_dt_len < rank
            || ssm_norm_len < dim
        {
            return None;
        }
        let qkv_j = resolve_q8(mapped, metal, &format!("{p}.attn_qkv.weight"), hidden)?;
        let z_j = resolve_q8(mapped, metal, &format!("{p}.attn_gate.weight"), hidden)?;
        let alpha_j = resolve_q8(mapped, metal, &format!("{p}.ssm_alpha.weight"), hidden)?;
        let beta_j = resolve_q8(mapped, metal, &format!("{p}.ssm_beta.weight"), hidden)?;
        let out_j = resolve_q8(mapped, metal, &format!("{p}.ssm_out.weight"), inner)?;
        let gate_j = resolve_q8(mapped, metal, &format!("{p}.ffn_gate.weight"), hidden)?;
        let up_j = resolve_q8(mapped, metal, &format!("{p}.ffn_up.weight"), hidden)?;
        let down_j = resolve_q8(mapped, metal, &format!("{p}.ffn_down.weight"), ffn)?;
        if qkv_j.rows != qkv_w
            || z_j.rows != inner
            || alpha_j.rows != rank
            || beta_j.rows != rank
            || out_j.rows != hidden
            || gate_j.rows != ffn
            || up_j.rows != ffn
            || down_j.rows != hidden
        {
            return None;
        }
        let (conv_state, delta_state) =
            ensure_resident(metal.device, n_slots, conv_stride, delta_stride)?;
        let mut o = 0usize;
        let mut take = |n: usize| {
            let start = o;
            o += n;
            start
        };
        let hn = take(hidden);
        let qkv = take(qkv_w);
        let z = take(inner);
        let alpha = take(rank);
        let beta = take(rank);
        let conv = take(qkv_w);
        let decay = take(rank);
        let bsig = take(rank);
        let gated = take(inner);
        let hnorm = take(inner);
        let act = take(inner);
        let proj = take(hidden);
        let resid = take(hidden);
        let post = take(hidden);
        let fg = take(ffn);
        let fu = take(ffn);
        let fh = take(ffn);
        let fd = take(hidden);
        let yout = take(hidden);
        let y_elems = o;
        const VEC: usize = 256;
        const HEAD_T: usize = 128;
        let hidden_gpu = ensure_hidden(metal.device, hidden)?;
        let chained = STREAM.with(|slot| slot.borrow().pending) && !pipe.copy.is_null();
        let y_keep = resolve_q8(mapped, metal, "output.weight", hidden)
            .map(|j| hidden + j.rows + 1)
            .unwrap_or(y_elems)
            .max(y_elems);
        unsafe {
            let (x_buf, y_buf) = scratch_pair(metal.device, hidden * 4, y_keep * 4)?;
            let src_buf = if chained {
                hidden_gpu
            } else {
                let x_ptr = (fns.msg0)(x_buf, fns.sel_contents) as *mut f32;
                if x_ptr.is_null() {
                    return None;
                }
                ptr::copy_nonoverlapping(input.as_ptr(), x_ptr, hidden);
                x_buf
            };
            let (cmd, enc) = STREAM.with(|slot| {
                let mut slot = slot.borrow_mut();
                if slot.open && !slot.enc.is_null() && !slot.cmd.is_null() {
                    return (slot.cmd, slot.enc);
                }
                let cmd = (fns.msg0)(pipe.queue, fns.sel_cmd);
                let enc = (fns.msg0)(cmd, fns.sel_enc);
                if !cmd.is_null() && !enc.is_null() {
                    (fns.retain)(cmd);
                    (fns.retain)(enc);
                    slot.cmd = cmd;
                    slot.enc = enc;
                    slot.open = true;
                }
                (cmd, enc)
            });
            if cmd.is_null() || enc.is_null() {
                return None;
            }
            let set_rms =
                |src_buf: *mut c_void, src_off: usize, w_off: usize, dst: usize, n: usize| {
                    (fns.msg1)(enc, fns.sel_set_pipe, pipe.rms);
                    (fns.msg_set_buf)(enc, fns.sel_set_buf, src_buf, src_off, 0);
                    (fns.msg_set_buf)(enc, fns.sel_set_buf, metal.buffer, w_off, 1);
                    (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, dst * 4, 2);
                    let rp = RmsP {
                        n: n as u32,
                        eps: spec.epsilon,
                    };
                    (fns.msg_set_bytes)(
                        enc,
                        fns.sel_set_bytes,
                        (&rp as *const RmsP).cast(),
                        std::mem::size_of::<RmsP>(),
                        3,
                    );
                    oa_metal_dispatch(enc, fns.sel_dispatch, 1, VEC);
                };
            let set_add = |a_buf: *mut c_void,
                           a_off: usize,
                           b_buf: *mut c_void,
                           b_off: usize,
                           dst: usize,
                           n: usize| {
                (fns.msg1)(enc, fns.sel_set_pipe, pipe.add);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, a_buf, a_off, 0);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, b_buf, b_off, 1);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, dst * 4, 2);
                let n32 = n as u32;
                (fns.msg_set_bytes)(
                    enc,
                    fns.sel_set_bytes,
                    (&n32 as *const u32).cast(),
                    std::mem::size_of::<u32>(),
                    3,
                );
                oa_metal_dispatch(enc, fns.sel_dispatch, n.div_ceil(VEC), VEC);
            };
            set_rms(src_buf, 0, attn_n, hn, hidden);
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                hn * 4,
                y_buf,
                qkv * 4,
                &qkv_j,
            );
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                hn * 4,
                y_buf,
                z * 4,
                &z_j,
            );
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                hn * 4,
                y_buf,
                alpha * 4,
                &alpha_j,
            );
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                hn * 4,
                y_buf,
                beta * 4,
                &beta_j,
            );
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.conv);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, qkv * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, conv_state, slot * conv_stride * 4, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, metal.buffer, conv_w, 2);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, conv * 4, 3);
            let cp = ConvP {
                channels: qkv_w as u32,
                ksize: kernel as u32,
                taps: taps as u32,
            };
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&cp as *const ConvP).cast(),
                std::mem::size_of::<ConvP>(),
                4,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, qkv_w.div_ceil(VEC), VEC);
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.decay);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, alpha * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, beta * 4, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, metal.buffer, ssm_a, 2);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, metal.buffer, ssm_dt, 3);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, decay * 4, 4);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, bsig * 4, 5);
            let rn = rank as u32;
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&rn as *const u32).cast(),
                std::mem::size_of::<u32>(),
                6,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, rank.div_ceil(VEC), VEC);
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.delta);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, conv * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, decay * 4, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, bsig * 4, 2);
            (fns.msg_set_buf)(
                enc,
                fns.sel_set_buf,
                delta_state,
                slot * delta_stride * 4,
                3,
            );
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, gated * 4, 4);
            let dp = DeltaP {
                dim: dim as u32,
                rank: rank as u32,
                groups: groups as u32,
                q_size: q_size as u32,
                v_off: v_off as u32,
                reordered: u32::from(spec.v_head_reordered),
            };
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&dp as *const DeltaP).cast(),
                std::mem::size_of::<DeltaP>(),
                5,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, rank, HEAD_T);
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.phrms);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, gated * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, metal.buffer, ssm_norm, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, hnorm * 4, 2);
            let hp = HeadRmsP {
                dim: dim as u32,
                rank: rank as u32,
                eps: spec.epsilon,
            };
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&hp as *const HeadRmsP).cast(),
                std::mem::size_of::<HeadRmsP>(),
                3,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, rank, HEAD_T);
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.silu);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, z * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, hnorm * 4, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, act * 4, 2);
            let inn = inner as u32;
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&inn as *const u32).cast(),
                std::mem::size_of::<u32>(),
                3,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, inner.div_ceil(VEC), VEC);
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                act * 4,
                y_buf,
                proj * 4,
                &out_j,
            );
            set_add(y_buf, proj * 4, src_buf, 0, resid, hidden);
            set_rms(y_buf, resid * 4, post_n, post, hidden);
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                post * 4,
                y_buf,
                fg * 4,
                &gate_j,
            );
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                post * 4,
                y_buf,
                fu * 4,
                &up_j,
            );
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.silu);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, fg * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, fu * 4, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, fh * 4, 2);
            let fn32 = ffn as u32;
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&fn32 as *const u32).cast(),
                std::mem::size_of::<u32>(),
                3,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, ffn.div_ceil(VEC), VEC);
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                fh * 4,
                y_buf,
                fd * 4,
                &down_j,
            );
            set_add(y_buf, resid * 4, y_buf, fd * 4, yout, hidden);
            if !pipe.copy.is_null() {
                (fns.msg1)(enc, fns.sel_set_pipe, pipe.copy);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, yout * 4, 0);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, hidden_gpu, 0, 1);
                let hn32 = hidden as u32;
                (fns.msg_set_bytes)(
                    enc,
                    fns.sel_set_bytes,
                    (&hn32 as *const u32).cast(),
                    std::mem::size_of::<u32>(),
                    2,
                );
                oa_metal_dispatch(enc, fns.sel_dispatch, hidden.div_ceil(VEC), VEC);
                STREAM.with(|slot| slot.borrow_mut().pending = true);
                note_hybrid("on");
                return Some(Vec::new());
            }
            (fns.msg_void)(enc, fns.sel_end);
            (fns.msg_void)(cmd, fns.sel_commit);
            (fns.msg_void)(cmd, fns.sel_wait);
            let y_ptr = (fns.msg0)(y_buf, fns.sel_contents) as *const f32;
            if y_ptr.is_null() {
                return None;
            }
            let mut out = vec![0f32; hidden];
            ptr::copy_nonoverlapping(y_ptr.add(yout), out.as_mut_ptr(), hidden);
            note_hybrid("on");
            Some(out)
        }
    }

    fn note_attn(msg: &str) {
        if std::env::var_os("OPENAGENTS_METAL_DEBUG").is_none() {
            return;
        }
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| eprintln!("attn-gpu: {msg}"));
    }

    pub(super) fn run_full_layer(
        mapped: &MappedWeights,
        spec: &HybridSpec,
        attn: &AttnSpec,
        index: usize,
        input: &[f32],
        slot: usize,
        n_full: usize,
        position: usize,
    ) -> Option<Vec<f32>> {
        if std::env::var_os("OPENAGENTS_HYBRID_CPU").is_some()
            || std::env::var_os("OPENAGENTS_ATTN_CPU").is_some()
        {
            return None;
        }
        if attn.dim != 256
            || attn.heads == 0
            || attn.kv_heads == 0
            || attn.heads % attn.kv_heads != 0
            || attn.rotary < 2
            || attn.rotary % 2 != 0
            || attn.rotary > attn.dim
            || attn.n_ctx == 0
            || attn.n_ctx > 4096
            || position >= attn.n_ctx
            || spec.ffn == 0
            || slot >= n_full
        {
            note_attn("skip-shape");
            return None;
        }
        let metal = unsafe { &*active_metal()? };
        let fns = match fns() {
            Some(f) => f,
            None => {
                note_attn("skip-fns");
                return None;
            }
        };
        let pipe = match pipeline_for(metal.device) {
            Some(p) => p,
            None => {
                note_attn("skip-pipeline");
                return None;
            }
        };
        if pipe.rms.is_null()
            || pipe.add.is_null()
            || pipe.split.is_null()
            || pipe.ph256.is_null()
            || pipe.rope.is_null()
            || pipe.writekv.is_null()
            || pipe.attn.is_null()
            || pipe.sig.is_null()
            || pipe.copy.is_null()
        {
            note_attn("skip-kernels");
            return None;
        }
        let p = format!("blk.{index}");
        let hidden = spec.hidden;
        let heads = attn.heads;
        let kv_heads = attn.kv_heads;
        let dim = attn.dim;
        let q_w = heads * dim;
        let kv_w = kv_heads * dim;
        let ffn = spec.ffn;
        let n_ctx = attn.n_ctx;
        let (attn_n, attn_n_len) = resolve_f32(mapped, metal, &format!("{p}.attn_norm.weight"))?;
        let (post_n, post_n_len) =
            resolve_f32(mapped, metal, &format!("{p}.post_attention_norm.weight"))?;
        let (q_norm, q_norm_len) = resolve_f32(mapped, metal, &format!("{p}.attn_q_norm.weight"))?;
        let (k_norm, k_norm_len) = resolve_f32(mapped, metal, &format!("{p}.attn_k_norm.weight"))?;
        if attn_n_len < hidden || post_n_len < hidden || q_norm_len < dim || k_norm_len < dim {
            return None;
        }
        let q_j = resolve_q8(mapped, metal, &format!("{p}.attn_q.weight"), hidden)?;
        let k_j = resolve_q8(mapped, metal, &format!("{p}.attn_k.weight"), hidden)?;
        let v_j = resolve_q8(mapped, metal, &format!("{p}.attn_v.weight"), hidden)?;
        let out_j = resolve_q8(mapped, metal, &format!("{p}.attn_output.weight"), q_w)?;
        let gate_j = resolve_q8(mapped, metal, &format!("{p}.ffn_gate.weight"), hidden)?;
        let up_j = resolve_q8(mapped, metal, &format!("{p}.ffn_up.weight"), hidden)?;
        let down_j = resolve_q8(mapped, metal, &format!("{p}.ffn_down.weight"), ffn)?;
        if q_j.rows != q_w * 2
            || k_j.rows != kv_w
            || v_j.rows != kv_w
            || out_j.rows != hidden
            || gate_j.rows != ffn
            || up_j.rows != ffn
            || down_j.rows != hidden
        {
            note_attn("skip-q8-rows");
            return None;
        }
        let (k_cache, v_cache) = ensure_kv(metal.device, n_full, n_ctx, kv_w)?;
        let mut o = 0usize;
        let mut take = |n: usize| {
            let start = o;
            o += n;
            start
        };
        let hn = take(hidden);
        let qg = take(q_w * 2);
        let k_now = take(kv_w);
        let v_now = take(kv_w);
        let q = take(q_w);
        let gate = take(q_w);
        let qn = take(q_w);
        let kn = take(kv_w);
        let attn_o = take(q_w);
        let gated = take(q_w);
        let proj = take(hidden);
        let resid = take(hidden);
        let post = take(hidden);
        let fg = take(ffn);
        let fu = take(ffn);
        let fh = take(ffn);
        let fd = take(hidden);
        let yout = take(hidden);
        let y_elems = o;
        const VEC: usize = 256;
        let hidden_gpu = ensure_hidden(metal.device, hidden)?;
        let chained = STREAM.with(|slot| slot.borrow().pending);
        let seq = (position + 1) as u32;
        let ap = AttnP {
            heads: heads as u32,
            kv_heads: kv_heads as u32,
            dim: dim as u32,
            seq,
            kv_width: kv_w as u32,
            n_ctx: n_ctx as u32,
            slot: slot as u32,
            scale: (dim as f32).sqrt().recip(),
        };
        let has_mrope = attn.mrope.iter().any(|s| *s > 0);
        let rp_q = RopeP {
            n_heads: heads as u32,
            head_dim: dim as u32,
            rotary: attn.rotary as u32,
            has_mrope: u32::from(has_mrope),
            pos0: position as f32,
            theta_scale: attn.rope_theta.powf(-2.0 / attn.rotary as f32),
            s0: attn.mrope[0] as u32,
            s1: attn.mrope[1] as u32,
            s2: attn.mrope[2] as u32,
            s3: attn.mrope[3] as u32,
        };
        let rp_k = RopeP {
            n_heads: kv_heads as u32,
            ..rp_q
        };
        let y_keep = resolve_q8(mapped, metal, "output.weight", hidden)
            .map(|j| hidden + j.rows + 1)
            .unwrap_or(y_elems)
            .max(y_elems);
        unsafe {
            let (x_buf, y_buf) = scratch_pair(metal.device, hidden * 4, y_keep * 4)?;
            let src_buf = if chained {
                hidden_gpu
            } else {
                let x_ptr = (fns.msg0)(x_buf, fns.sel_contents) as *mut f32;
                if x_ptr.is_null() {
                    return None;
                }
                ptr::copy_nonoverlapping(input.as_ptr(), x_ptr, hidden);
                x_buf
            };
            let (cmd, enc) = STREAM.with(|slot| {
                let mut slot = slot.borrow_mut();
                if slot.open && !slot.enc.is_null() && !slot.cmd.is_null() {
                    return (slot.cmd, slot.enc);
                }
                let cmd = (fns.msg0)(pipe.queue, fns.sel_cmd);
                let enc = (fns.msg0)(cmd, fns.sel_enc);
                if !cmd.is_null() && !enc.is_null() {
                    (fns.retain)(cmd);
                    (fns.retain)(enc);
                    slot.cmd = cmd;
                    slot.enc = enc;
                    slot.open = true;
                }
                (cmd, enc)
            });
            if cmd.is_null() || enc.is_null() {
                return None;
            }
            let set_rms =
                |src_buf: *mut c_void, src_off: usize, w_off: usize, dst: usize, n: usize| {
                    (fns.msg1)(enc, fns.sel_set_pipe, pipe.rms);
                    (fns.msg_set_buf)(enc, fns.sel_set_buf, src_buf, src_off, 0);
                    (fns.msg_set_buf)(enc, fns.sel_set_buf, metal.buffer, w_off, 1);
                    (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, dst * 4, 2);
                    let rp = RmsP {
                        n: n as u32,
                        eps: spec.epsilon,
                    };
                    (fns.msg_set_bytes)(
                        enc,
                        fns.sel_set_bytes,
                        (&rp as *const RmsP).cast(),
                        std::mem::size_of::<RmsP>(),
                        3,
                    );
                    oa_metal_dispatch(enc, fns.sel_dispatch, 1, VEC);
                };
            let set_add = |a_buf: *mut c_void,
                           a_off: usize,
                           b_buf: *mut c_void,
                           b_off: usize,
                           dst: usize,
                           n: usize| {
                (fns.msg1)(enc, fns.sel_set_pipe, pipe.add);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, a_buf, a_off, 0);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, b_buf, b_off, 1);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, dst * 4, 2);
                let n32 = n as u32;
                (fns.msg_set_bytes)(
                    enc,
                    fns.sel_set_bytes,
                    (&n32 as *const u32).cast(),
                    std::mem::size_of::<u32>(),
                    3,
                );
                oa_metal_dispatch(enc, fns.sel_dispatch, n.div_ceil(VEC), VEC);
            };
            set_rms(src_buf, 0, attn_n, hn, hidden);
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                hn * 4,
                y_buf,
                qg * 4,
                &q_j,
            );
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                hn * 4,
                y_buf,
                k_now * 4,
                &k_j,
            );
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                hn * 4,
                y_buf,
                v_now * 4,
                &v_j,
            );
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.split);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, qg * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, q * 4, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, gate * 4, 2);
            let sp = SplitP {
                heads: heads as u32,
                dim: dim as u32,
            };
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&sp as *const SplitP).cast(),
                std::mem::size_of::<SplitP>(),
                3,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, heads, dim);
            let set_ph256 = |src: usize, w_off: usize, dst: usize, rank: usize| {
                (fns.msg1)(enc, fns.sel_set_pipe, pipe.ph256);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, src * 4, 0);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, metal.buffer, w_off, 1);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, dst * 4, 2);
                let hp = HeadRmsP {
                    dim: dim as u32,
                    rank: rank as u32,
                    eps: spec.epsilon,
                };
                (fns.msg_set_bytes)(
                    enc,
                    fns.sel_set_bytes,
                    (&hp as *const HeadRmsP).cast(),
                    std::mem::size_of::<HeadRmsP>(),
                    3,
                );
                oa_metal_dispatch(enc, fns.sel_dispatch, rank, dim);
            };
            set_ph256(q, q_norm, qn, heads);
            set_ph256(k_now, k_norm, kn, kv_heads);
            let set_rope = |off: usize, rp: &RopeP| {
                (fns.msg1)(enc, fns.sel_set_pipe, pipe.rope);
                (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, off * 4, 0);
                (fns.msg_set_bytes)(
                    enc,
                    fns.sel_set_bytes,
                    (rp as *const RopeP).cast(),
                    std::mem::size_of::<RopeP>(),
                    1,
                );
                oa_metal_dispatch(enc, fns.sel_dispatch, rp.n_heads as usize, attn.rotary / 2);
            };
            set_rope(qn, &rp_q);
            set_rope(kn, &rp_k);
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.writekv);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, kn * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, v_now * 4, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, k_cache, 0, 2);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, v_cache, 0, 3);
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&ap as *const AttnP).cast(),
                std::mem::size_of::<AttnP>(),
                4,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, kv_w.div_ceil(VEC), VEC);
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.attn);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, qn * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, k_cache, 0, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, v_cache, 0, 2);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, attn_o * 4, 3);
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&ap as *const AttnP).cast(),
                std::mem::size_of::<AttnP>(),
                4,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, heads, dim);
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.sig);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, attn_o * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, gate * 4, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, gated * 4, 2);
            let qn32 = q_w as u32;
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&qn32 as *const u32).cast(),
                std::mem::size_of::<u32>(),
                3,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, q_w.div_ceil(VEC), VEC);
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                gated * 4,
                y_buf,
                proj * 4,
                &out_j,
            );
            set_add(y_buf, proj * 4, src_buf, 0, resid, hidden);
            set_rms(y_buf, resid * 4, post_n, post, hidden);
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                post * 4,
                y_buf,
                fg * 4,
                &gate_j,
            );
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                post * 4,
                y_buf,
                fu * 4,
                &up_j,
            );
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.silu);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, fg * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, fu * 4, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, fh * 4, 2);
            let fn32 = ffn as u32;
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&fn32 as *const u32).cast(),
                std::mem::size_of::<u32>(),
                3,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, ffn.div_ceil(VEC), VEC);
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                fh * 4,
                y_buf,
                fd * 4,
                &down_j,
            );
            set_add(y_buf, resid * 4, y_buf, fd * 4, yout, hidden);
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.copy);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, yout * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, hidden_gpu, 0, 1);
            let hn32 = hidden as u32;
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&hn32 as *const u32).cast(),
                std::mem::size_of::<u32>(),
                2,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, hidden.div_ceil(VEC), VEC);
            STREAM.with(|slot| slot.borrow_mut().pending = true);
            GPU_ATTN.with(|c| c.set(true));
            note_attn("on");
            Some(Vec::new())
        }
    }

    pub(super) fn try_greedy_id(mapped: &MappedWeights, hidden: &[f32]) -> Option<u32> {
        let metal = unsafe { &*active_metal()? };
        let fns = fns()?;
        let pipe = pipeline_for(metal.device)?;
        if pipe.rms.is_null() || pipe.argmax.is_null() {
            return None;
        }
        let n = hidden.len();
        let (norm_off, norm_len) = resolve_f32(mapped, metal, "output_norm.weight")?;
        if norm_len < n {
            return None;
        }
        let out_j = resolve_q8(mapped, metal, "output.weight", n)?;
        if out_j.rows == 0 {
            return None;
        }
        let hidden_gpu = ensure_hidden(metal.device, n)?;
        let chained = STREAM.with(|slot| slot.borrow().pending);
        let y_elems = n + out_j.rows + 1;
        const VEC: usize = 256;
        unsafe {
            let (x_buf, y_buf) = scratch_pair(metal.device, n * 4, y_elems * 4)?;
            let src_buf = if chained {
                hidden_gpu
            } else {
                let x_ptr = (fns.msg0)(x_buf, fns.sel_contents) as *mut f32;
                if x_ptr.is_null() {
                    return None;
                }
                ptr::copy_nonoverlapping(hidden.as_ptr(), x_ptr, n);
                x_buf
            };
            let (cmd, enc) = STREAM.with(|slot| {
                let mut slot = slot.borrow_mut();
                if slot.open && !slot.enc.is_null() && !slot.cmd.is_null() {
                    return (slot.cmd, slot.enc);
                }
                let cmd = (fns.msg0)(pipe.queue, fns.sel_cmd);
                let enc = (fns.msg0)(cmd, fns.sel_enc);
                if !cmd.is_null() && !enc.is_null() {
                    (fns.retain)(cmd);
                    (fns.retain)(enc);
                    slot.cmd = cmd;
                    slot.enc = enc;
                    slot.open = true;
                }
                (cmd, enc)
            });
            if cmd.is_null() || enc.is_null() {
                return None;
            }
            let hn = 0usize;
            let logits = n;
            let id_off = n + out_j.rows;
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.rms);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, src_buf, 0, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, metal.buffer, norm_off, 1);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, hn * 4, 2);
            let rp = RmsP {
                n: n as u32,
                eps: 1e-6,
            };
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&rp as *const RmsP).cast(),
                std::mem::size_of::<RmsP>(),
                3,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, 1, VEC);
            encode_q8(
                fns,
                enc,
                pipe.pipeline,
                metal.buffer,
                y_buf,
                hn * 4,
                y_buf,
                logits * 4,
                &out_j,
            );
            (fns.msg1)(enc, fns.sel_set_pipe, pipe.argmax);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, logits * 4, 0);
            (fns.msg_set_buf)(enc, fns.sel_set_buf, y_buf, id_off * 4, 1);
            let vn = out_j.rows as u32;
            (fns.msg_set_bytes)(
                enc,
                fns.sel_set_bytes,
                (&vn as *const u32).cast(),
                std::mem::size_of::<u32>(),
                2,
            );
            oa_metal_dispatch(enc, fns.sel_dispatch, 1, VEC);
            STREAM.with(|slot| {
                let mut slot = slot.borrow_mut();
                close_stream(fns, &mut slot);
                slot.pending = false;
            });
            let p = (fns.msg0)(y_buf, fns.sel_contents) as *const u32;
            if p.is_null() {
                return None;
            }
            Some(*p.add(id_off))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_q8(rows: usize, width: usize) -> Vec<u8> {
        let blocks = width.div_ceil(Q8_K);
        let mut out = vec![0u8; rows * blocks * Q8_BLOCK];
        let one = 0x3c00u16.to_le_bytes();
        for r in 0..rows {
            for b in 0..blocks {
                let off = (r * blocks + b) * Q8_BLOCK;
                out[off..off + 2].copy_from_slice(&one);
                for j in 0..Q8_K {
                    let i = b * Q8_K + j;
                    if i < width {
                        out[off + 2 + j] = i8::from(i == r % width) as u8;
                    }
                }
            }
        }
        out
    }

    #[test]
    fn metal_q8_matches_cpu_when_available() {
        let rows = 64usize;
        let width = 64usize;
        let weights = encode_q8(rows, width);
        let x = vec![1.0f32; width];
        let cpu = crate::generate::matvec_q8_bytes(&weights, &x).unwrap();
        let Ok(metal) = crate::metal_wrap::wrap_shared(&weights) else {
            return;
        };
        let gpu = q8_matvec(&metal, 0, rows, width, &x)
            .expect("Metal Q8 shader must compile when wrap_shared succeeds");
        assert_eq!(cpu, gpu, "Metal Q8 matvec must match CPU");

        let mut padded = vec![0u8; 128];
        padded.extend_from_slice(&weights);
        let metal = crate::metal_wrap::wrap_shared(&padded).unwrap();
        let gpu_off = q8_matvec(&metal, 128, rows, width, &x).unwrap();
        assert_eq!(cpu, gpu_off, "Metal Q8 matvec must honor a non-zero offset");

        let jobs = [
            Q8Job {
                offset: 128,
                rows,
                width,
            },
            Q8Job {
                offset: 128,
                rows,
                width,
            },
        ];
        let batched = q8_matvec_many(&metal, &jobs, &x).unwrap();
        assert_eq!(batched.len(), 2);
        assert_eq!(cpu, batched[0], "batched Q8 must match sequential");
        assert_eq!(cpu, batched[1], "batched Q8 must match sequential");
    }

    #[test]
    fn metal_q8_ffn_matches_cpu_when_available() {
        let hidden = 32usize;
        let mid = 64usize;
        let gate_w = encode_q8(mid, hidden);
        let up_w = encode_q8(mid, hidden);
        let down_w = encode_q8(hidden, mid);
        let mut blob = gate_w.clone();
        let up_off = blob.len();
        blob.extend_from_slice(&up_w);
        let down_off = blob.len();
        blob.extend_from_slice(&down_w);
        let x = vec![0.5f32; hidden];
        let gate = crate::generate::matvec_q8_bytes(&gate_w, &x).unwrap();
        let up = crate::generate::matvec_q8_bytes(&up_w, &x).unwrap();
        let hid: Vec<f32> = gate
            .iter()
            .zip(up.iter())
            .map(|(g, u)| {
                let s = *g / (1.0 + (-*g).exp());
                s * *u
            })
            .collect();
        let cpu = crate::generate::matvec_q8_bytes(&down_w, &hid).unwrap();
        let Ok(metal) = crate::metal_wrap::wrap_shared(&blob) else {
            return;
        };
        let Some(gpu) = q8_ffn(
            &metal,
            &Q8Job {
                offset: 0,
                rows: mid,
                width: hidden,
            },
            &Q8Job {
                offset: up_off,
                rows: mid,
                width: hidden,
            },
            &Q8Job {
                offset: down_off,
                rows: hidden,
                width: mid,
            },
            &x,
        ) else {
            return;
        };
        assert_eq!(cpu.len(), gpu.len());
        for (a, b) in cpu.iter().zip(gpu.iter()) {
            assert!((a - b).abs() < 1e-4, "FFN mismatch cpu={a} gpu={b}");
        }
    }
}
