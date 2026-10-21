// DeepSeek-V4.1 hyper-connection mixing, Engram gather and gating, and
// expert routing on Metal (#12420), a port of
// psionic-backend-cuda/src/kernels/v41_mix.cu (#12393).
//
// Each kernel keeps the CUDA kernel's grid, thread roles, and order of
// operations: element-wise steps, the Sinkhorn normalization, the four-term
// stream sums, the top-k rule, and the expert sums repeat the CPU path; long
// reductions add every 256th product per thread, a shuffle-down tree adds a
// SIMD group's 32 lanes, and thread 0 adds the eight group totals in order.

constant constexpr int kMixThreads = 256;
constant constexpr int kMixWarps = kMixThreads / 32;
constant constexpr int kHcSplits = 16;
constant constexpr int kRouteThreads = 1024;

// Sums one value per thread of a 256-thread group: a shuffle-down tree in each
// SIMD group, then group 0's total plus groups 1 to 7 in order. The result is
// valid in thread 0 only. Every thread must call it.
inline float mix_block_sum(float value, threadgroup float* scratch, uint tid) {
    for (ushort offset = 16; offset > 0; offset >>= 1) {
        value = value + simd_shuffle_down(value, offset);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if ((tid & 31) == 0) {
        scratch[tid >> 5] = value;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    float total = 0.0f;
    if (tid == 0) {
        total = scratch[0];
        for (int w = 1; w < kMixWarps; ++w) {
            total = total + scratch[w];
        }
    }
    return total;
}

// ---------------------------------------------------------------------------
// Hyper-connection mixing
// ---------------------------------------------------------------------------

struct HcProjectArgs {
    device const float* residual;
    device const float* weights;
    device float* partials;
    int mixes;
    int elements;
};

// Grid (kHcSplits, mixes + 1). Group (s, y) sums one K slice of row y of the
// projection times the residual, or for y == mixes the squares of the
// residual. Thread t adds the slice's elements t, t + 256, ... in order.
kernel void v41_hc_project(
    constant HcProjectArgs& a [[buffer(0)]],
    uint2 group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    threadgroup float scratch[kMixWarps];
    const int split = int(group.x);
    const int row = int(group.y);
    const int chunk = (a.elements + kHcSplits - 1) / kHcSplits;
    const int begin = split * chunk;
    const int end = min(a.elements, begin + chunk);
    device const float* w = a.weights + long(row) * a.elements;
    float acc = 0.0f;
    for (int i = begin + int(tid); i < end; i += kMixThreads) {
        const float x = a.residual[i];
        acc = acc + (row < a.mixes ? w[i] : x) * x;
    }
    const float total = mix_block_sum(acc, scratch, tid);
    if (tid == 0) {
        a.partials[row * kHcSplits + split] = total;
    }
}

// Pre and post coefficients and the Sinkhorn-normalized residual mix, from
// lane i's logit, as HyperConnections::split_sinkhorn computes them. All 32
// lanes must call it. Returns lane i's output value.
inline float split_sinkhorn(float logit, int streams, float mix_epsilon, int iterations, uint lane) {
    const int h = streams;
    const int mixes = (h + 2) * h;
    const bool active = int(lane) < mixes;
    const bool bad = simd_any(active && !isfinite(logit));
    if (bad) {
        return V41_NAN;  // the CPU path refuses nonfinite logits
    }
    float coefficient = 0.0f;
    if (int(lane) < h) {
        coefficient = sigmoid_once(logit) + mix_epsilon;
    } else if (int(lane) < 2 * h) {
        coefficient = 2.0f * sigmoid_once(logit);
    }
    const int c = int(lane) - 2 * h;
    const bool comb = c >= 0 && int(lane) < mixes;
    const int r = comb ? c / h : 0;
    const int k = comb ? c % h : 0;
    const int row_base = 2 * h + r * h;
    const int col_base = 2 * h + k;
    float x = logit;
    float max = -V41_INF;
    for (int j = 0; j < h; ++j) {
        max = fmax(max, simd_shuffle(x, ushort(row_base + j)));
    }
    const float e = comb ? exp_once(x - max) : 1.0f;
    float sum = simd_shuffle(e, ushort(row_base));
    for (int j = 1; j < h; ++j) {
        sum = sum + simd_shuffle(e, ushort(row_base + j));
    }
    x = fdiv(e, sum) + mix_epsilon;
    for (int iteration = 0; iteration < iterations; ++iteration) {
        if (iteration > 0) {
            float s = simd_shuffle(x, ushort(row_base));
            for (int j = 1; j < h; ++j) {
                s = s + simd_shuffle(x, ushort(row_base + j));
            }
            x = fdiv(x, s + mix_epsilon);
        }
        float s = simd_shuffle(x, ushort(col_base));
        for (int j = 1; j < h; ++j) {
            s = s + simd_shuffle(x, ushort(col_base + j * h));
        }
        x = fdiv(x, s + mix_epsilon);
    }
    return int(lane) < 2 * h ? coefficient : x;
}

inline float hc_logit(float projected, device const float* scale, device const float* base, int i, int h) {
    const float s = scale[i < h ? 0 : (i < 2 * h ? 1 : 2)];
    return projected * s + base[i];
}

struct HcFinishArgs {
    device const float* partials;
    device const float* scale;
    device const float* base;
    device float* projected;
    device float* out;
    int streams;
    int elements;
    float norm_epsilon;
    float mix_epsilon;
    int iterations;
};

// One SIMD group: finishes the projection (partials summed in split order),
// the joint RMS, and the mixes.
kernel void v41_hc_finish(constant HcFinishArgs& a [[buffer(0)]], uint lane [[thread_index_in_threadgroup]]) {
    const int h = a.streams;
    const int mixes = (h + 2) * h;
    float squares = a.partials[mixes * kHcSplits];
    for (int s = 1; s < kHcSplits; ++s) {
        squares = squares + a.partials[mixes * kHcSplits + s];
    }
    const float variance = fdiv(squares, float(a.elements));
    const float inv_rms = fdiv(1.0f, fsqrt(variance + a.norm_epsilon));
    float logit = 0.0f;
    if (int(lane) < mixes) {
        float dot = a.partials[lane * kHcSplits];
        for (int s = 1; s < kHcSplits; ++s) {
            dot = dot + a.partials[lane * kHcSplits + s];
        }
        const float p = isfinite(variance) ? dot * inv_rms : V41_NAN;
        a.projected[lane] = p;
        logit = hc_logit(p, a.scale, a.base, int(lane), h);
    }
    const float value = split_sinkhorn(logit, h, a.mix_epsilon, a.iterations, lane);
    if (int(lane) < mixes) {
        a.out[lane] = value;
    }
}

struct HcSplitArgs {
    device const float* projected;
    device const float* scale;
    device const float* base;
    device float* out;
    int streams;
    float mix_epsilon;
    int iterations;
};

kernel void v41_hc_split(constant HcSplitArgs& a [[buffer(0)]], uint lane [[thread_index_in_threadgroup]]) {
    const int h = a.streams;
    const int mixes = (h + 2) * h;
    const float logit = int(lane) < mixes ? hc_logit(a.projected[lane], a.scale, a.base, int(lane), h) : 0.0f;
    const float value = split_sinkhorn(logit, h, a.mix_epsilon, a.iterations, lane);
    if (int(lane) < mixes) {
        a.out[lane] = value;
    }
}

struct HcCollapseArgs {
    device const float* residual;
    device const float* mixes;
    device float* out;
    int streams;
    int width;
    int round_bf16;
};

kernel void v41_hc_collapse(
    constant HcCollapseArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    const int d = int(group) * kMixThreads + int(tid);
    if (d >= a.width) {
        return;
    }
    float acc = a.mixes[0] * a.residual[d];
    for (int j = 1; j < a.streams; ++j) {
        acc = acc + a.mixes[j] * a.residual[long(j) * a.width + d];
    }
    a.out[d] = store_value(acc, a.round_bf16);
}

struct HcExpandArgs {
    device const float* output;
    device const float* residual;
    device const float* mixes;
    device float* out;
    int streams;
    int width;
    int round_bf16;
};

kernel void v41_hc_expand(
    constant HcExpandArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    const long index = long(group) * kMixThreads + long(tid);
    if (index >= long(a.streams) * a.width) {
        return;
    }
    const int k = int(index / a.width);
    const int d = int(index % a.width);
    const int h = a.streams;
    device const float* comb = a.mixes + 2 * h;
    float acc = comb[k] * a.residual[d];
    for (int j = 1; j < h; ++j) {
        acc = acc + comb[j * h + k] * a.residual[long(j) * a.width + d];
    }
    a.out[index] = store_value(a.mixes[h + k] * a.output[d] + acc, a.round_bf16);
}

// ---------------------------------------------------------------------------
// Engram
// ---------------------------------------------------------------------------

struct EngramGatherArgs {
    device const uchar* codes;
    device const uchar* scales;
    long rows;
    device const long* ids;
    device float* out;
    int width;
    int round_bf16;
};

// One SIMD group per requested row. An id outside the table writes NaN.
kernel void v41_engram_gather(
    constant EngramGatherArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint lane [[thread_index_in_threadgroup]]) {
    const int slot = int(group);
    const long id = a.ids[slot];
    device float* row_out = a.out + long(slot) * a.width;
    const bool valid = id >= 0 && id < a.rows;
    device const uchar* row = a.codes + (valid ? id : 0) * a.width;
    device const uchar* row_scales = a.scales + (valid ? id : 0) * (a.width / 32);
    for (int i = int(lane) * 8; i < a.width; i += 32 * 8) {
        if (!valid) {
            for (int j = 0; j < 8; ++j) {
                row_out[i + j] = V41_NAN;
            }
            continue;
        }
        const uint scale = row_scales[i / 32];
        for (int j = 0; j < 8; ++j) {
            const float value = mul_e8m0(e4m3(row[i + j]), scale);
            row_out[i + j] = store_value(value, a.round_bf16);
        }
    }
}

struct EngramGateArgs {
    device const float* residual;
    device const float* projected;
    device const float* q_weight;
    device const float* k_weight;
    device float* out;
    int streams;
    int width;
    float epsilon;
    float inv_sqrt_width;
    int round_bf16;
};

// One group per stream: the per-stream RMS of the residual and the key, the
// gated dot product, then residual + gate * value.
kernel void v41_engram_gate(
    constant EngramGateArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    threadgroup float scratch[kMixWarps];
    threadgroup float gate_shared;
    const int h = int(group);
    const long start = long(h) * a.width;
    device const float* x = a.residual + start;
    device const float* key = a.projected + start;
    device const float* value = a.projected + long(a.streams) * a.width;
    float xs = 0.0f;
    float ks = 0.0f;
    float dot = 0.0f;
    for (int d = int(tid); d < a.width; d += kMixThreads) {
        const float xv = x[d];
        const float kv = key[d];
        xs = xs + xv * xv;
        ks = ks + kv * kv;
        const float qk = a.q_weight[start + d] * a.k_weight[start + d];
        dot = dot + (xv * qk) * kv;
    }
    xs = mix_block_sum(xs, scratch, tid);
    ks = mix_block_sum(ks, scratch, tid);
    dot = mix_block_sum(dot, scratch, tid);
    if (tid == 0) {
        const float xvar = fdiv(xs, float(a.width));
        const float kvar = fdiv(ks, float(a.width));
        const float rstd = fdiv(1.0f, fsqrt(xvar + a.epsilon)) * fdiv(1.0f, fsqrt(kvar + a.epsilon));
        const float scaled = (dot * rstd) * a.inv_sqrt_width;
        float gate = V41_NAN;
        if (isfinite(xvar) && isfinite(kvar) && isfinite(scaled)) {
            const float root = fsqrt(fmax(fabs(scaled), 1e-6f));
            gate = sigmoid_once(copysign(root, scaled));
        }
        gate_shared = gate;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    const float gate = gate_shared;
    for (int d = int(tid); d < a.width; d += kMixThreads) {
        a.out[start + d] = store_value(x[d] + gate * value[d], a.round_bf16);
    }
}

// ---------------------------------------------------------------------------
// Routing and experts
// ---------------------------------------------------------------------------

struct GateLogitsArgs {
    device const ushort* weight;
    device const float* input;
    device float* logits;
    int experts;
    int cols;
};

// One SIMD group per expert row of the BF16 gate: lane l takes 8-value chunks
// l, l + 32, ... and adds their products in order, then a shuffle-down tree
// adds the lanes.
kernel void v41_gate_logits(
    constant GateLogitsArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint warp [[simdgroup_index_in_threadgroup]],
    uint lane [[thread_index_in_simdgroup]]) {
    const int row = int(group) * kMixWarps + int(warp);
    if (row >= a.experts) {
        return;  // uniform across the SIMD group
    }
    device const ushort* w = a.weight + long(row) * a.cols;
    float acc = 0.0f;
    for (int chunk = int(lane); chunk * 8 < a.cols; chunk += 32) {
        const uint4 packed = *(device const uint4*)(w + chunk * 8);
        const uint words[4] = {packed.x, packed.y, packed.z, packed.w};
        for (int q = 0; q < 4; ++q) {
            const float lo = as_type<float>(words[q] << 16);
            const float hi = as_type<float>(words[q] & 0xffff0000u);
            acc = acc + lo * a.input[chunk * 8 + q * 2];
            acc = acc + hi * a.input[chunk * 8 + q * 2 + 1];
        }
    }
    for (ushort offset = 16; offset > 0; offset >>= 1) {
        acc = acc + simd_shuffle_down(acc, offset);
    }
    if (lane == 0) {
        a.logits[row] = acc;
    }
}

struct Candidate {
    float value;
    int id;
};

// Higher value first; equal values pick the lower id.
inline bool better(Candidate a, Candidate b) {
    return a.value > b.value || (a.value == b.value && a.id < b.id);
}

inline Candidate warp_best(Candidate c) {
    for (ushort offset = 16; offset > 0; offset >>= 1) {
        Candidate other;
        other.value = simd_shuffle_down(c.value, offset);
        other.id = simd_shuffle_down(c.id, offset);
        if (better(other, c)) {
            c = other;
        }
    }
    return c;
}

struct RouteArgs {
    device const float* logits;
    device const float* bias;
    device int* ids;
    device float* weights;
    int experts;
    int top_k;
    float temperature;
    int normalize;
    float route_scale;
};

// One group: sqrtsoftplus scores, biased ranking, top-k with ties to the lower
// id, and weights from the unbiased scores, written in ascending id order.
kernel void v41_route(constant RouteArgs& a [[buffer(0)]], uint e_ [[thread_index_in_threadgroup]]) {
    threadgroup float scores[kRouteThreads];
    threadgroup uchar taken[kRouteThreads];
    threadgroup Candidate warp_winners[kRouteThreads / 32];
    threadgroup int rank_ids[32];
    threadgroup atomic_int any_bad;
    const int e = int(e_);
    float ranked = -V41_INF;
    bool bad = false;
    if (e == 0) {
        atomic_store_explicit(&any_bad, 0, memory_order_relaxed);
    }
    if (e < a.experts) {
        const float logit = a.logits[e];
        const float x = fdiv(logit, a.temperature);
        // PyTorch softplus returns its input above the threshold 20.
        const float score = x > 20.0f ? fsqrt(x) : fsqrt(log1p_once(exp_once(x)));
        ranked = score + a.bias[e];
        bad = !isfinite(logit) || !isfinite(x) || !isfinite(ranked);
        scores[e] = score;
    }
    taken[e] = 0;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (bad) {
        atomic_fetch_or_explicit(&any_bad, 1, memory_order_relaxed);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (atomic_load_explicit(&any_bad, memory_order_relaxed) != 0) {
        if (e < a.top_k) {
            a.ids[e] = -1;
            a.weights[e] = V41_NAN;
        }
        return;
    }
    for (int r = 0; r < a.top_k; ++r) {
        Candidate c;
        c.value = (e < a.experts && !taken[e]) ? ranked : -V41_INF;
        c.id = (e < a.experts && !taken[e]) ? e : 0x7fffffff;
        c = warp_best(c);
        if ((e & 31) == 0) {
            warp_winners[e >> 5] = c;
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        if (e == 0) {
            Candidate best = warp_winners[0];
            for (int w = 1; w < kRouteThreads / 32; ++w) {
                if (better(warp_winners[w], best)) {
                    best = warp_winners[w];
                }
            }
            rank_ids[r] = best.id;
            taken[best.id] = 1;
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (e == 0) {
        float w[32];
        int id[32];
        for (int r = 0; r < a.top_k; ++r) {
            id[r] = rank_ids[r];
            w[r] = scores[id[r]];
        }
        if (a.normalize && a.top_k > 1) {
            float sum = w[0];
            for (int r = 1; r < a.top_k; ++r) {
                sum = sum + w[r];
            }
            sum = sum + 1e-20f;
            for (int r = 0; r < a.top_k; ++r) {
                w[r] = fdiv(w[r], sum);
            }
        }
        for (int r = 0; r < a.top_k; ++r) {
            w[r] = w[r] * a.route_scale;
        }
        for (int i = 1; i < a.top_k; ++i) {
            const int key = id[i];
            const float value = w[i];
            int j = i - 1;
            while (j >= 0 && id[j] > key) {
                id[j + 1] = id[j];
                w[j + 1] = w[j];
                --j;
            }
            id[j + 1] = key;
            w[j + 1] = value;
        }
        for (int r = 0; r < a.top_k; ++r) {
            a.ids[r] = id[r];
            a.weights[r] = w[r];
        }
    }
}

struct SwigluArgs {
    device const float* gate;
    device const float* up;
    device const float* route_weights;
    device float* out;
    int slots;
    int width;
    float limit;
    int round_bf16;
};

kernel void v41_swiglu(
    constant SwigluArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    const long index = long(group) * kMixThreads + long(tid);
    if (index >= long(a.slots) * a.width) {
        return;
    }
    float g = a.gate[index];
    float u = a.up[index];
    const float w = a.route_weights == nullptr ? 1.0f : a.route_weights[index / a.width];
    float value;
    if (isnan(g) || isnan(u)) {
        value = V41_NAN;
    } else {
        if (a.limit > 0.0f) {
            g = fmin(g, a.limit);
            u = fmin(fmax(u, -a.limit), a.limit);
        }
        value = ((g * sigmoid_once(g)) * u) * w;
    }
    a.out[index] = store_value(value, a.round_bf16);
}

struct MoeCombineArgs {
    device const float* routed;
    device const float* shared_out;
    device float* out;
    int slots;
    int width;
    int round_bf16;
};

kernel void v41_moe_combine(
    constant MoeCombineArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    const int d = int(group) * kMixThreads + int(tid);
    if (d >= a.width) {
        return;
    }
    float acc = 0.0f;
    for (int s = 0; s < a.slots; ++s) {
        acc = acc + a.routed[long(s) * a.width + d];
    }
    if (a.shared_out != nullptr) {
        acc = acc + a.shared_out[d];
    }
    a.out[d] = store_value(acc, a.round_bf16);
}
