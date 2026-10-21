// DeepSeek-V4.1 attention on Metal (#12420), a port of
// psionic-backend-cuda/src/kernels/v41_attention.cu (#12392): tail RoPE and
// YaRN, the cache quantizers, window and compressed latent attention with the
// sink, the compressor, compressed-row stores, the indexer, and top-k and
// candidate selection.
//
// Every kernel repeats the CUDA kernel's (and so the CPU path's) order:
// sequential dot products, matrix rows, and RMS sums from element 0, the
// 64-slot online softmax, and selection by score with ties to the lower index.
// Two changes keep the threadgroup memory within Apple's 32 KB, neither of
// which changes an operation's order: the dense product stages 256 columns at
// a time instead of 512, and the sparse attention reads value rows straight
// from memory instead of staging each 64-slot block.
//
// RoPE reads cos and sin of each (position, pair) from a host table that
// holds cos((double)(float(p) * f)) rounded to float, which is what the CUDA
// kernels compute on the device. A position at or past the table's rows
// writes NaN, as a position past 2^20 does on CUDA.

constant constexpr int kAttnThreads = 256;
constant constexpr int kSelectThreads = 1024;
constant constexpr int kMaxPosition = 1048576;
constant constexpr int kPoison = int(0x80000000u);
constant constexpr uchar kPoisonFlag = 0xffu;

constant constexpr int kQuantFp8 = 0;
constant constexpr int kQuantIndexFp4 = 1;
constant constexpr int kQuantCompressedFp4 = 2;

constant constexpr int kDenseF32 = 0;
constant constexpr int kDenseBf16 = 1;

inline uint ballot(bool predicate) {
    return uint(static_cast<ulong>(simd_ballot(predicate)));
}

// quantization::nearest over E4M3 codes 0 to 126: the first code whose value
// is at least x, then the closer of it and the code below, ties to the even
// code. x is finite and at most 448.
inline uint nearest_e4m3(float x) {
    int lo = 0;
    int hi = 127;
    while (lo < hi) {
        const int mid = (lo + hi) / 2;
        if (e4m3(uint(mid)) < x) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    const int upper = lo < 126 ? lo : 126;
    if (upper == 0) {
        return 0;
    }
    const int lower = upper - 1;
    const float a = x - e4m3(uint(lower));
    const float b = e4m3(uint(upper)) - x;
    return (a < b || (a == b && lower % 2 == 0)) ? uint(lower) : uint(upper);
}

inline uint encode_fp8(float value) {
    const uint sign = signbit(value) ? 0x80u : 0u;
    return nearest_e4m3(fmin(fabs(value), 448.0f)) | sign;
}

inline uint encode_fp4(float value) {
    const float x = fmin(fabs(value), 6.0f);
    int upper = 0;
    while (upper < 7 && kE2m1[upper] < x) {
        ++upper;
    }
    uint code = 0;
    if (upper != 0) {
        const int lower = upper - 1;
        const float a = x - kE2m1[lower];
        const float b = kE2m1[upper] - x;
        code = (a < b || (a == b && lower % 2 == 0)) ? uint(lower) : uint(upper);
    }
    return code | (signbit(value) ? 8u : 0u);
}

// ceil_power_scale: the next power of two at or above value; NaN where the
// CPU refuses (nonfinite, below the smallest normal, or overflow).
inline float ceil_power(float value) {
    if (!isfinite(value) || value < 0x1p-126f) {
        return V41_NAN;
    }
    const uint bits = as_type<uint>(value);
    const uint exponent = ((bits >> 23) & 255u) + ((bits & 0x7fffffu) != 0u ? 1u : 0u);
    if (exponent >= 255u) {
        return V41_NAN;
    }
    return as_type<float>(exponent << 23);
}

// One value of a quantization block held one value per lane (blocks of 32, or
// 16 for the compressed KV, aligned within the SIMD group), as inplace_bf16.
// Every lane calls it.
inline float quant_lane(int kind, float in, int n, uint lane) {
    const uint segment = n == 32 ? 0xffffffffu : (0xffffu << (lane & 16));
    const float x = bf16_round(in);
    const bool finite = (ballot(!isfinite(x)) & segment) == 0u;
    float amax = fabs(x);
    for (int offset = n / 2; offset > 0; offset /= 2) {
        amax = fmax(amax, simd_shuffle_xor(amax, ushort(offset)));
    }
    float scale;
    if (kind == kQuantFp8) {
        scale = ceil_power(fmax(amax, 1e-4f) * fdiv(1.0f, 448.0f));
    } else if (kind == kQuantIndexFp4) {
        scale = ceil_power(fmax(amax, 6.0f * 0x1p-126f) * fdiv(1.0f, 6.0f));
    } else {
        const float value = fdiv(fmax(amax, 6.0f * 0x1p-9f), 6.0f);
        scale = value > 448.0f ? V41_NAN : e4m3(encode_fp8(value));
    }
    const float q = fdiv(x, scale);
    const float decoded = kind == kQuantFp8 ? e4m3(encode_fp8(q)) : e2m1(encode_fp4(q));
    const float y = bf16_round(decoded * scale);
    const bool overflow = (ballot(!isfinite(y)) & segment) != 0u;
    return (!finite || isnan(scale) || overflow) ? V41_NAN : y;
}

inline int quant_block_width(int kind) {
    return kind == kQuantCompressedFp4 ? 16 : 32;
}

// Rotates one head's tail pairs and optionally rounds the whole head. The
// group's threads share the head; `out` may alias `in`. `table` holds
// (cos, sin) per position and pair.
template <typename In, typename Out>
inline void rope_head(
    In in,
    Out out,
    device const float2* table,
    int table_rows,
    int rope_dim,
    int head_width,
    int position,
    bool inverse,
    bool round,
    uint tid,
    uint threads) {
    const int prefix = head_width - rope_dim;
    const bool valid = position >= 0 && position < kMaxPosition && position < table_rows;
    for (int i = int(tid); i < prefix; i += int(threads)) {
        const float v = valid ? in[i] : V41_NAN;
        out[i] = round ? bf16_round(v) : v;
    }
    for (int p = int(tid); p < rope_dim / 2; p += int(threads)) {
        const float a = in[prefix + 2 * p];
        const float b = in[prefix + 2 * p + 1];
        float first = V41_NAN;
        float second = V41_NAN;
        if (valid) {
            const float2 cs = table[long(position) * (rope_dim / 2) + p];
            const float c = cs.x;
            const float s = inverse ? -cs.y : cs.y;
            first = a * c - b * s;
            second = a * s + b * c;
        }
        out[prefix + 2 * p] = round ? bf16_round(first) : first;
        out[prefix + 2 * p + 1] = round ? bf16_round(second) : second;
    }
}

// The CPU's rms_norm in place on a threadgroup vector: a sequential sum of
// squares by thread 0, 1 / sqrt(mean + eps), then bf16(w) * (x * scale),
// rounded to BF16 when asked. Ends with the group synchronized.
inline void rms_norm_shared(
    threadgroup float* x,
    device const float* weight,
    int width,
    float epsilon,
    bool round,
    threadgroup float* scratch,
    uint tid,
    uint threads) {
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (tid == 0) {
        float sum = -0.0f;
        for (int i = 0; i < width; ++i) {
            sum = sum + x[i] * x[i];
        }
        const float variance = fdiv(sum, float(width));
        scratch[0] = isfinite(variance) ? fdiv(1.0f, fsqrt(variance + epsilon)) : V41_NAN;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    const float scale = scratch[0];
    for (int i = int(tid); i < width; i += int(threads)) {
        const float v = bf16_round(weight[i]) * (x[i] * scale);
        x[i] = round ? bf16_round(v) : v;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
}

// ---------------------------------------------------------------------------
// RoPE
// ---------------------------------------------------------------------------

struct RopeArgs {
    device float* values;
    device const float2* table;
    device const int* positions;
    int table_rows;
    int heads;
    int head_width;
    int rope_dim;
    int back;
    int inverse;
    int round_bf16;
};

kernel void v41_rope(
    constant RopeArgs& a [[buffer(0)]],
    uint2 group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint2 threads_v [[threads_per_threadgroup]]) {
    const uint threads = threads_v.x;
    const int token = int(group.y);
    const int head = int(group.x);
    device float* row = a.values + (long(token) * a.heads + head) * a.head_width;
    const int position = a.positions[token] - a.back;
    rope_head(row, row, a.table, a.table_rows, a.rope_dim, a.head_width, position,
              a.inverse != 0, a.round_bf16 != 0, tid, threads);
}

// ---------------------------------------------------------------------------
// Cache quantization and the window ring
// ---------------------------------------------------------------------------

struct CacheQuantArgs {
    device const float* input;
    device float* out;
    long total;
    int kind;
};

kernel void v41_cache_quant(
    constant CacheQuantArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint lane [[thread_index_in_simdgroup]]) {
    const long e = long(group) * kAttnThreads + long(tid);
    const float v = quant_lane(a.kind, e < a.total ? a.input[e] : 0.0f, quant_block_width(a.kind), lane);
    if (e < a.total) {
        a.out[e] = v;
    }
}

struct WindowStoreArgs {
    device const float* input;
    device ushort* ring;
    device const int* positions;
    int ring_rows;
    int width;
};

kernel void v41_window_store(
    constant WindowStoreArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint lane [[thread_index_in_simdgroup]],
    uint threads [[threads_per_threadgroup]]) {
    const int token = int(group);
    const int position = a.positions[token];
    if (position < 0) {
        return;
    }
    const long slot = position % a.ring_rows;
    for (int base = 0; base < a.width; base += int(threads)) {
        const int i = base + int(tid);
        const float v = quant_lane(
            kQuantFp8, i < a.width ? a.input[long(token) * a.width + i] : 0.0f, 32, lane);
        if (i < a.width) {
            a.ring[slot * a.width + i] = bf16_bits(v);
        }
    }
}

// ---------------------------------------------------------------------------
// Dense matrix-vector products and RMS norms in the CPU's order
// ---------------------------------------------------------------------------

inline float dense_weight(
    device const void* weights, device const uchar* scales, int format, long row, int col, int cols) {
    const long at = row * cols + col;
    if (format == kDenseF32) {
        return ((device const float*)weights)[at];
    }
    if (format == kDenseBf16) {
        return bf16_value(((device const ushort*)weights)[at]);
    }
    const float value = mul_e8m0(
        e4m3(((device const uchar*)weights)[at]), scales[(row / 32) * (cols / 32) + col / 32]);
    return bf16_round(value);
}

constant constexpr int kDenseRows = 8;
constant constexpr int kDenseCols = 256;
constant constexpr int kDenseThreads = 256;

struct DenseArgs {
    device const void* weights;
    device const uchar* scales;
    device const float* input;
    device float* out;
    int format;
    int rows;
    int cols;
    int groups;
    int round_input;
    int round_bf16;
};

// Each group computes kDenseRows outputs for one token: 0 + sum_k w[r, k] x[k],
// the sum sequential from -0. Tiles of the rows go through threadgroup memory;
// one thread per row then adds its tile in order.
kernel void v41_dense(
    constant DenseArgs& a [[buffer(0)]],
    uint2 group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    threadgroup float tile[kDenseRows][kDenseCols + 1];
    threadgroup float xs[kDenseRows][kDenseCols + 1];
    const int first = int(group.x) * kDenseRows;
    const int token = int(group.y);
    const int mine = first + int(tid);
    const bool owner = int(tid) < kDenseRows && mine < a.rows;
    const int per_group = a.rows / a.groups;
    constexpr int kPerThread = kDenseRows * kDenseCols / kDenseThreads;
    float sum = -0.0f;
    for (int base = 0; base < a.cols; base += kDenseCols) {
        const int width = a.cols - base < kDenseCols ? a.cols - base : kDenseCols;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (int k = 0; k < kPerThread; ++k) {
            const int e = int(tid) + k * kDenseThreads;
            const int r = e / kDenseCols;
            const int c = e % kDenseCols;
            const bool ok = base + c < a.cols && first + r < a.rows;
            const float wv = ok ? dense_weight(a.weights, a.scales, a.format, first + r, base + c, a.cols) : 0.0f;
            const float xv = ok ? a.input[(long(token) * a.groups + (first + r) / per_group) * a.cols + base + c] : 0.0f;
            tile[r][c] = wv;
            xs[r][c] = a.round_input ? bf16_round(xv) : xv;
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        if (owner) {
            for (int c = 0; c < width; ++c) {
                sum = sum + tile[tid][c] * xs[tid][c];
            }
        }
    }
    if (owner) {
        const float total = 0.0f + sum;
        a.out[long(token) * a.rows + mine] = a.round_bf16 ? bf16_round(total) : total;
    }
}

struct RmsNormArgs {
    device const float* input;
    device const float* weight;
    device float* out;
    int width;
    float epsilon;
    int round_bf16;
};

kernel void v41_rms_norm(
    constant RmsNormArgs& a [[buffer(0)]],
    threadgroup float* shared [[threadgroup(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint threads [[threads_per_threadgroup]]) {
    threadgroup float* x = shared;
    const long base = long(group) * a.width;
    for (int i = int(tid); i < a.width; i += int(threads)) {
        x[i] = a.input[base + i];
    }
    rms_norm_shared(x, a.weight, a.width, a.epsilon, a.round_bf16 != 0, shared + a.width, tid, threads);
    for (int i = int(tid); i < a.width; i += int(threads)) {
        a.out[base + i] = x[i];
    }
}

// ---------------------------------------------------------------------------
// Compression
// ---------------------------------------------------------------------------

struct CompressArgs {
    device const float* values;
    device const float* gates;
    device const float* norm;
    device const float* state;
    device const int* positions;
    device float* latents;
    int ratio;
    int width;
    float epsilon;
};

// The latent of every token whose group completes: the gated softmax pool of
// its `ratio` members, rounded to BF16, then the RMS norm with the BF16 norm
// weight, rounded.
kernel void v41_compress(
    constant CompressArgs& a [[buffer(0)]],
    threadgroup float* shared [[threadgroup(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint threads [[threads_per_threadgroup]]) {
    threadgroup float* pooled = shared;
    const int token = int(group);
    const int position = a.positions[token];
    const int ratio = a.ratio;
    if (position < 0 || (position + 1) % ratio != 0) {
        return;
    }
    for (int d = int(tid); d < a.width; d += int(threads)) {
        float member_values[8];
        float member_gates[8];
        for (int r = 0; r < ratio; ++r) {
            const int source = token - (ratio - 1) + r;
            if (source >= 0) {
                member_values[r] = a.values[long(source) * a.width + d];
                member_gates[r] = ratio > 1 ? a.gates[long(source) * a.width + d] : 0.0f;
            } else {
                const int slot = (position - (ratio - 1) + r) % ratio;
                member_values[r] = a.state[(2 * slot) * a.width + d];
                member_gates[r] = a.state[(2 * slot + 1) * a.width + d];
            }
        }
        float value;
        if (ratio == 1) {
            value = member_values[0];
        } else {
            float max = -V41_INF;
            for (int r = 0; r < ratio; ++r) {
                max = fmax(max, member_gates[r]);
            }
            float denominator = -0.0f;
            for (int r = 0; r < ratio; ++r) {
                denominator = denominator + exp_once(member_gates[r] - max);
            }
            value = -0.0f;
            for (int r = 0; r < ratio; ++r) {
                const float weight = fdiv(exp_once(member_gates[r] - max), denominator);
                value = value + member_values[r] * weight;
            }
        }
        pooled[d] = bf16_round(value);
    }
    rms_norm_shared(pooled, a.norm, a.width, a.epsilon, true, shared + a.width, tid, threads);
    for (int d = int(tid); d < a.width; d += int(threads)) {
        a.latents[long(token) * a.width + d] = pooled[d];
    }
}

struct CompressStateArgs {
    device const float* values;
    device const float* gates;
    device float* state;
    device const int* positions;
    int tokens;
    int ratio;
    int width;
};

// Keeps the chunk's trailing incomplete group in `state` for the next call.
kernel void v41_compress_state(
    constant CompressStateArgs& a [[buffer(0)]],
    uint tid [[thread_index_in_threadgroup]],
    uint threads [[threads_per_threadgroup]]) {
    const int last = a.positions[a.tokens - 1];
    if (last < 0) {
        return;
    }
    const int pending = (last + 1) % a.ratio;
    for (int k = 0; k < pending; ++k) {
        const int token = a.tokens - pending + k;
        if (token < 0) {
            continue;
        }
        const int slot = a.positions[token] % a.ratio;
        for (int d = int(tid); d < a.width; d += int(threads)) {
            a.state[(2 * slot) * a.width + d] = a.values[long(token) * a.width + d];
            a.state[(2 * slot + 1) * a.width + d] =
                a.gates != nullptr ? a.gates[long(token) * a.width + d] : 0.0f;
        }
    }
}

struct CompressedStoreArgs {
    device const float* latents;
    device const ushort* wk;
    device const float* k_norm;
    device const float2* table;
    device const int* positions;
    device ushort* kv;
    device ushort* index_keys;
    int table_rows;
    int ratio;
    int capacity;
    int head_dim;
    int index_dim;
    int rope_dim;
    float epsilon;
};

// For every token whose group completes: the index key and the compressed KV
// row, stored at row (p + 1) / ratio - 1.
kernel void v41_compressed_store(
    constant CompressedStoreArgs& a [[buffer(0)]],
    threadgroup float* shared [[threadgroup(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint lane [[thread_index_in_simdgroup]],
    uint threads [[threads_per_threadgroup]]) {
    threadgroup float* latent = shared;
    threadgroup float* key = latent + a.head_dim;
    threadgroup float* scratch = key + a.index_dim;
    const int token = int(group);
    const int position = a.positions[token];
    if (position < 0 || (position + 1) % a.ratio != 0) {
        return;
    }
    const int row = (position + 1) / a.ratio - 1;
    if (row >= a.capacity) {
        return;
    }
    const int group_position = position + 1 - a.ratio;
    for (int i = int(tid); i < a.head_dim; i += int(threads)) {
        latent[i] = a.latents[long(token) * a.head_dim + i];
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    // wk times the latent, one thread per key row, in column order.
    for (int r = int(tid); r < a.index_dim; r += int(threads)) {
        device const ushort* w = a.wk + long(r) * a.head_dim;
        float sum = -0.0f;
        for (int c = 0; c < a.head_dim; ++c) {
            sum = sum + bf16_value(w[c]) * bf16_round(latent[c]);
        }
        key[r] = bf16_round(0.0f + sum);
    }
    rms_norm_shared(key, a.k_norm, a.index_dim, a.epsilon, true, scratch, tid, threads);
    rope_head(key, key, a.table, a.table_rows, a.rope_dim, a.index_dim, group_position, false, true, tid, threads);
    rope_head(latent, latent, a.table, a.table_rows, a.rope_dim, a.head_dim, group_position, false, true, tid, threads);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (int base = 0; base < a.index_dim; base += int(threads)) {
        const int i = base + int(tid);
        const float v = quant_lane(kQuantIndexFp4, i < a.index_dim ? key[i] : 0.0f, 32, lane);
        if (i < a.index_dim) {
            a.index_keys[long(row) * a.index_dim + i] = bf16_bits(v);
        }
    }
    for (int base = 0; base < a.head_dim; base += int(threads)) {
        const int i = base + int(tid);
        const float v = quant_lane(kQuantCompressedFp4, i < a.head_dim ? latent[i] : 0.0f, 16, lane);
        if (i < a.head_dim) {
            a.kv[long(row) * a.head_dim + i] = bf16_bits(v);
        }
    }
}

// ---------------------------------------------------------------------------
// Indexer scores
// ---------------------------------------------------------------------------

constant constexpr int kScoreWarps = 8;
constant constexpr int kScoreKeys = 32;

struct IndexScoresArgs {
    device const float* query;
    device const float* weights;
    device const ushort* keys;
    device const int* positions;
    device float* scores;
    long score_stride;
    int capacity;
    int ratio;
    int heads;
    int dim;
    float scale;
};

// One SIMD group per key and one lane per index head: each lane takes its
// head's dot product sequentially from -0, then the lanes' terms
// max(dot_h, 0) * bf16(w_h * scale) are added over the heads in order from +0.
kernel void v41_index_scores(
    constant IndexScoresArgs& a [[buffer(0)]],
    threadgroup float* shared [[threadgroup(0)]],
    uint2 group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint warp [[simdgroup_index_in_threadgroup]],
    uint lane [[thread_index_in_simdgroup]],
    uint2 threads_v [[threads_per_threadgroup]]) {
    const uint threads = threads_v.x;
    const int padded = a.dim + 1;
    threadgroup float* q = shared;
    threadgroup float* w = q + a.heads * padded;
    threadgroup ushort* k = (threadgroup ushort*)(w + a.heads);
    const int token = int(group.y);
    for (int i = int(tid); i < a.heads * a.dim; i += int(threads)) {
        q[(i / a.dim) * padded + i % a.dim] = a.query[long(token) * a.heads * a.dim + i];
    }
    for (int h = int(tid); h < a.heads; h += int(threads)) {
        w[h] = bf16_round(a.weights[long(token) * a.heads + h] * a.scale);
    }
    const int position = a.positions[token];
    const long visible = position < 0 ? 0 : (long(position) + 1) / a.ratio;
    const long first = long(group.x) * kScoreKeys;
    for (int i = int(tid); i < kScoreKeys * a.dim; i += int(threads)) {
        const long key = first + i / a.dim;
        k[i] = key < visible && key < a.capacity ? a.keys[key * a.dim + i % a.dim] : 0;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (int local = int(warp); local < kScoreKeys; local += kScoreWarps) {
        const long key = first + local;
        if (key >= visible || key >= a.capacity) {
            break;
        }
        float term = 0.0f;
        bool finite = true;
        if (int(lane) < a.heads) {
            float dot = -0.0f;
            threadgroup const float* qh = q + lane * padded;
            threadgroup const ushort* kk = k + local * a.dim;
            for (int i = 0; i < a.dim; ++i) {
                dot = dot + qh[i] * bf16_value(kk[i]);
            }
            finite = isfinite(dot);
            term = fmax(dot, 0.0f) * w[lane];
        }
        const bool poison = simd_any(!finite);
        float score = 0.0f;
        for (int h = 0; h < 32; ++h) {
            const float t = simd_shuffle(term, ushort(h));
            if (h < a.heads) {
                score = score + t;
            }
        }
        if (lane == 0) {
            a.scores[long(token) * a.score_stride + key] = poison ? V41_NAN : score;
        }
    }
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

// total_cmp order as an unsigned key: larger floats give larger keys.
inline uint order_key(float value) {
    const uint bits = as_type<uint>(value);
    return (bits & 0x80000000u) ? ~bits : (bits | 0x80000000u);
}

// Group-wide exclusive prefix of `flag` over the 1024 threads; returns this
// thread's prefix and writes the total to `total`.
inline int block_prefix(bool flag, threadgroup int* warp_counts, thread int& total, uint tid) {
    const uint lane = tid & 31;
    const uint warp = tid >> 5;
    const uint vote = ballot(flag);
    const int within = int(popcount(vote & ((1u << lane) - 1u)));
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (lane == 0) {
        warp_counts[warp] = int(popcount(vote));
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    int before = 0;
    int sum = 0;
    for (int w = 0; w < kSelectThreads / 32; ++w) {
        if (w < int(warp)) {
            before += warp_counts[w];
        }
        sum += warp_counts[w];
    }
    total = sum;
    return before + within;
}

struct SelectArgs {
    device const float* values;
    long value_stride;
    device const uchar* flags;
    long flag_stride;
    device const int* positions;
    device int* out;
    long out_stride;
    device uchar* flags_out;
    long flags_out_stride;
    int flag_group;
    int ratio;
    int group;
    int top_k;
    int allow_inf;
};

// Top top_k of each token's first n values by (value descending, index
// ascending), written in ascending index order with -1 after the selected
// ones. NaN, +inf (unless allow_inf), or a poisoned flag poison the row.
kernel void v41_select(
    constant SelectArgs& a [[buffer(0)]],
    uint group_id [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    threadgroup atomic_uint histogram[256];
    threadgroup int warp_counts[kSelectThreads / 32];
    threadgroup atomic_int poison_flag;
    threadgroup atomic_int eligible_total;
    threadgroup int shared_state[2];
    const int token = int(group_id);
    device const float* row = a.values + long(token) * a.value_stride;
    device const uchar* flag_row = a.flags != nullptr ? a.flags + long(token) * a.flag_stride : nullptr;
    device int* out_row = a.out + long(token) * a.out_stride;
    device uchar* flags_out_row = a.flags_out != nullptr ? a.flags_out + long(token) * a.flags_out_stride : nullptr;
    const int position = a.positions[token];
    const long visible = position < 0 ? 0 : (long(position) + 1) / a.ratio;
    const long wanted = (visible + a.group - 1) / a.group;
    const int n = int(wanted < a.value_stride ? wanted : a.value_stride);
    const int count = a.top_k < n ? a.top_k : n;

    if (tid == 0) {
        atomic_store_explicit(&poison_flag, 0, memory_order_relaxed);
        atomic_store_explicit(&eligible_total, 0, memory_order_relaxed);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    int eligible_count = 0;
    bool poison = false;
    for (int i = int(tid); i < n; i += kSelectThreads) {
        const float v = row[i];
        const uchar flag = flag_row != nullptr ? flag_row[i / a.flag_group] : uchar(1);
        if (isnan(v) || (v == V41_INF && !a.allow_inf) || flag == kPoisonFlag) {
            poison = true;
        }
        if (v != -V41_INF && flag != 0) {
            ++eligible_count;
        }
    }
    if (poison) {
        atomic_fetch_or_explicit(&poison_flag, 1, memory_order_relaxed);
    }
    atomic_fetch_add_explicit(&eligible_total, eligible_count, memory_order_relaxed);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (atomic_load_explicit(&poison_flag, memory_order_relaxed) != 0) {
        for (int i = int(tid); i < a.top_k; i += kSelectThreads) {
            out_row[i] = kPoison;
        }
        if (flags_out_row != nullptr) {
            for (int i = int(tid); i < n; i += kSelectThreads) {
                flags_out_row[i] = kPoisonFlag;
            }
        }
        return;
    }
    const int eligible = atomic_load_explicit(&eligible_total, memory_order_relaxed);
    const int take = count < eligible ? count : eligible;

    // Radix-select the key of the take-th largest eligible value.
    uint prefix = 0;
    uint mask = 0;
    int remaining = take;
    for (int shift = 24; shift >= 0 && take > 0; shift -= 8) {
        for (int d = int(tid); d < 256; d += kSelectThreads) {
            atomic_store_explicit(&histogram[d], 0u, memory_order_relaxed);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (int i = int(tid); i < n; i += kSelectThreads) {
            const float v = row[i];
            const uchar flag = flag_row != nullptr ? flag_row[i / a.flag_group] : uchar(1);
            if (v == -V41_INF || flag == 0) {
                continue;
            }
            const uint key = order_key(v);
            if ((key & mask) == prefix) {
                atomic_fetch_add_explicit(&histogram[(key >> shift) & 255u], 1u, memory_order_relaxed);
            }
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        if (tid == 0) {
            int above = 0;
            int digit = 255;
            for (; digit > 0; --digit) {
                const int h = int(atomic_load_explicit(&histogram[digit], memory_order_relaxed));
                if (above + h >= remaining) {
                    break;
                }
                above += h;
            }
            shared_state[0] = digit;
            shared_state[1] = remaining - above;
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        prefix |= uint(shared_state[0]) << shift;
        mask |= 255u << shift;
        remaining = shared_state[1];
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    const uint threshold = prefix;
    const int ties_needed = remaining;

    int written = 0;
    int ties_seen = 0;
    for (int base = 0; base < n; base += kSelectThreads) {
        const int i = base + int(tid);
        bool above = false;
        bool tie = false;
        if (i < n && take > 0) {
            const float v = row[i];
            const uchar flag = flag_row != nullptr ? flag_row[i / a.flag_group] : uchar(1);
            if (v != -V41_INF && flag != 0) {
                const uint key = order_key(v);
                above = key > threshold;
                tie = key == threshold;
            }
        }
        int tie_total = 0;
        const int tie_rank = ties_seen + block_prefix(tie, warp_counts, tie_total, tid);
        const bool selected = above || (tie && tie_rank < ties_needed);
        int selected_total = 0;
        const int slot = written + block_prefix(selected, warp_counts, selected_total, tid);
        if (selected) {
            out_row[slot] = i;
        }
        if (flags_out_row != nullptr && i < n) {
            flags_out_row[i] = selected ? 1 : 0;
        }
        written += selected_total;
        ties_seen += tie_total;
    }
    for (int i = take + int(tid); i < a.top_k; i += kSelectThreads) {
        out_row[i] = -1;
    }
}

struct BlockMaxArgs {
    device const float* scores;
    long score_stride;
    device float* block_scores;
    long block_stride;
    device const int* positions;
    int ratio;
    int block_size;
};

// Each block's maximum score, with the newest (possibly partial) block pinned
// to +inf. NaN or +inf in a block poisons it with NaN.
kernel void v41_block_max(
    constant BlockMaxArgs& a [[buffer(0)]],
    uint2 group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    const int token = int(group.y);
    const int position = a.positions[token];
    long visible = position < 0 ? 0 : (long(position) + 1) / a.ratio;
    visible = visible < a.score_stride ? visible : a.score_stride;
    long blocks = (visible + a.block_size - 1) / a.block_size;
    blocks = blocks < a.block_stride ? blocks : a.block_stride;
    const long b = long(group.x) * kAttnThreads + long(tid);
    if (b >= blocks) {
        return;
    }
    device const float* row = a.scores + long(token) * a.score_stride;
    float max = -V41_INF;
    bool poison = false;
    for (long i = b * a.block_size; i < (b + 1) * a.block_size && i < visible; ++i) {
        const float v = row[i];
        poison = poison || isnan(v) || v == V41_INF;
        max = fmax(max, v);
    }
    if (b == (visible - 1) / a.block_size) {
        max = V41_INF;
    }
    a.block_scores[long(token) * a.block_stride + b] = poison ? V41_NAN : max;
}

// ---------------------------------------------------------------------------
// Sparse latent attention
// ---------------------------------------------------------------------------

constant constexpr int kAttentionSplit = 128;
constant constexpr int kAttentionHeads = 4;

// A slot's KV row: the ring row (>= 0), compressed row -(code + 2), or none
// (-1, masked).
inline device const ushort* slot_row(int code, device const ushort* ring, device const ushort* compressed, int head_dim) {
    if (code >= 0) {
        return ring + long(code) * head_dim;
    }
    if (code == -1) {
        return nullptr;
    }
    return compressed + long(-(code + 2)) * head_dim;
}

struct SparseAttentionArgs {
    device const float* query;
    device const float* sinks;
    device const ushort* ring;
    device const ushort* compressed;
    device const int* indices;
    long index_stride;
    device const int* positions;
    device float* out;
    int ring_rows;
    int capacity;
    int heads;
    int head_dim;
    int window;
    int ratio;
    int top_k;
    float scale;
};

// One group per (4 heads, 128 output dimensions, token). Slots are the window
// rows oldest first, then the selected compressed rows (masked -1 slots keep
// their place), in blocks of 64 with the CPU's online softmax: BF16 weights in
// the value product, a float32 running denominator, and the sink added to the
// denominator last.
kernel void v41_sparse_attention(
    constant SparseAttentionArgs& a [[buffer(0)]],
    threadgroup float* shared [[threadgroup(0)]],
    uint3 group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint3 threads_v [[threads_per_threadgroup]]) {
    const uint threads = threads_v.x;
    constexpr int G = kAttentionHeads;
    threadgroup float running[G][3];
    threadgroup atomic_int poisoned[G];
    const int max_slots = a.window + (a.ratio > 0 ? a.top_k : 0);
    threadgroup float* q = shared;
    threadgroup float* scores = q + G * a.head_dim;
    threadgroup float* weights = scores;
    threadgroup int* codes = (threadgroup int*)(scores + G * max_slots);
    const int head0 = int(group.x) * G;
    const int dims = int(group.y) * kAttentionSplit;
    const int token = int(group.z);
    const int position = a.positions[token];
    const long query_base = (long(token) * a.heads + head0) * a.head_dim;
    const int lane = int(tid) % kAttentionSplit;
    const int pair = int(tid) / kAttentionSplit;
    const int d = dims + lane;
    const bool writes = d < a.head_dim;
    for (int i = int(tid); i < G * a.head_dim; i += int(threads)) {
        q[i] = bf16_round(a.query[query_base + i]);
    }
    if (int(tid) < G) {
        atomic_store_explicit(&poisoned[tid], position < 0 ? 1 : 0, memory_order_relaxed);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (position < 0) {
        for (int g = 2 * pair; g < 2 * pair + 2 && writes; ++g) {
            a.out[query_base + g * a.head_dim + d] = V41_NAN;
        }
        return;
    }
    const int window_rows = position + 1 < a.window ? position + 1 : a.window;
    const long visible = a.ratio > 0 ? (long(position) + 1) / a.ratio : 0;
    const int selected = a.ratio > 0 ? int(visible < a.top_k ? visible : a.top_k) : 0;
    const int slots = window_rows + selected;
    const int first = position + 1 - window_rows;
    for (int slot = int(tid); slot < slots; slot += int(threads)) {
        int code;
        if (slot < window_rows) {
            code = (first + slot) % a.ring_rows;
        } else {
            const int id = a.indices[long(token) * a.index_stride + (slot - window_rows)];
            const bool invalid = id < -1 || id >= visible || id >= a.capacity;
            if (invalid) {
                for (int g = 0; g < G; ++g) {
                    atomic_fetch_or_explicit(&poisoned[g], 1, memory_order_relaxed);
                }
            }
            code = id < 0 || invalid ? -1 : -(id + 2);
        }
        codes[slot] = code;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    // Scores: each thread's slots, a sequential dot product per head.
    for (int slot = int(tid); slot < slots; slot += int(threads)) {
        const int code = codes[slot];
        device const ushort* row = slot_row(code, a.ring, a.compressed, a.head_dim);
        float dot[G];
        for (int g = 0; g < G; ++g) {
            dot[g] = -0.0f;
        }
        for (int i = 0; i < a.head_dim; ++i) {
            const float v = row != nullptr ? bf16_value(row[i]) : 0.0f;
            for (int g = 0; g < G; ++g) {
                dot[g] = dot[g] + q[g * a.head_dim + i] * v;
            }
        }
        const bool masked = code == -1;
        for (int g = 0; g < G; ++g) {
            float score = 0.0f;
            if (!masked) {
                score = dot[g] * a.scale;
                if (!isfinite(score)) {
                    atomic_fetch_or_explicit(&poisoned[g], 1, memory_order_relaxed);
                }
            }
            scores[g * max_slots + slot] = score;
        }
    }

    float acc[2] = {0.0f, 0.0f};
    if (int(tid) < G) {
        running[tid][0] = -1e30f;
        running[tid][1] = 0.0f;
    }
    for (int chunk = 0; chunk < slots; chunk += 64) {
        const int end = chunk + 64 < slots ? chunk + 64 : slots;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        if (int(tid) < G) {
            const int g = int(tid);
            threadgroup const float* head_scores = scores + g * max_slots;
            const float previous = running[g][0];
            float max = previous;
            for (int slot = chunk; slot < end; ++slot) {
                if (codes[slot] != -1) {
                    max = fmax(max, head_scores[slot]);
                }
            }
            running[g][0] = max;
            running[g][2] = exp_once(previous - max);
            running[g][1] = running[g][1] * running[g][2];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (int e = int(tid); e < G * 64; e += int(threads)) {
            const int g = e / 64;
            const int slot = chunk + e % 64;
            if (slot < end) {
                weights[g * max_slots + slot] =
                    codes[slot] != -1 ? exp_once(scores[g * max_slots + slot] - running[g][0]) : 0.0f;
            }
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        if (int(tid) < G) {
            const int g = int(tid);
            float sum = running[g][1];
            for (int slot = chunk; slot < end; ++slot) {
                if (codes[slot] != -1) {
                    sum = sum + weights[g * max_slots + slot];
                }
            }
            running[g][1] = sum;
        }
        // A masked slot adds +0 (its weight and row are zero), which leaves
        // the sum unchanged: the sum is never -0.
        float value[2];
        for (int h = 0; h < 2; ++h) {
            value[h] = acc[h] * running[2 * pair + h][2];
        }
        threadgroup const float* w0 = weights + (2 * pair) * max_slots;
        threadgroup const float* w1 = w0 + max_slots;
        for (int slot = chunk; slot < end; ++slot) {
            device const ushort* row = slot_row(codes[slot], a.ring, a.compressed, a.head_dim);
            const float v = row != nullptr && writes ? bf16_value(row[d]) : 0.0f;
            value[0] = value[0] + bf16_round(w0[slot]) * v;
            value[1] = value[1] + bf16_round(w1[slot]) * v;
        }
        acc[0] = value[0];
        acc[1] = value[1];
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (int h = 0; h < 2; ++h) {
        if (!writes) {
            break;
        }
        const int g = 2 * pair + h;
        const long at = query_base + g * a.head_dim + d;
        if (atomic_load_explicit(&poisoned[g], memory_order_relaxed) != 0) {
            a.out[at] = V41_NAN;
            continue;
        }
        const float sum = running[g][1] + exp_once(a.sinks[head0 + g] - running[g][0]);
        a.out[at] = bf16_round(fdiv(acc[h], sum));
    }
}
