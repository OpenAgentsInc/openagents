// Experimental block32 DeltaNet. The default library does not compile this file.
#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>

using namespace metal;
using namespace mpp::tensor_ops;

struct BlockDeltaArgs {
    uint n, key_heads, value_heads, v_head_reordered, conv_width, value_offset;
};

enum : uint {
    kBlock = 32, kDim = 128, kValueTile = 32, kThreads = 128,
    // Keep the existing bounded workspace layout. L and beta fit in the
    // former W slot, including the prepared diagonal; values use the former U0 slot.
    kL = 0, kBeta = 1024, kKq = 1056, kV = 4096, kM = 8192, kG = 9216, kLast = 9248,
    kWorkspace = 9280,
};

inline bool valid_block_delta(constant BlockDeltaArgs &a, uint3 threads) {
    constexpr uint imax = 0x7fffffff;
    return threads.x == kThreads && threads.y == 1 && threads.z == 1 &&
        a.n > 0 && a.n <= imax && a.key_heads > 0 && a.value_heads > 0 &&
        a.key_heads <= imax / kDim && a.value_heads <= imax / kDim &&
        a.value_heads % a.key_heads == 0 && a.conv_width <= imax &&
        a.value_offset <= a.conv_width && a.value_heads * kDim <= a.conv_width - a.value_offset;
}

inline uint delta_key_head(constant BlockDeltaArgs &a, uint head) {
    return a.v_head_reordered != 0 ? head % a.key_heads : head / (a.value_heads / a.key_heads);
}

// Direct products remain defined when any decay is zero or underflows to zero.
// A ratio of cumulative products would produce 0/0 in that case.
inline float interval_decay(device const float *decay, uint first, uint end,
                            uint head, uint heads) {
    float product = 1.0f;
    for (uint t = first; t < end; ++t) product *= decay[ulong(t) * heads + head];
    return product;
}

// Independent blocks prepare L[t,j]=beta[t] D(t,j) K[t] K[j]^T, j<t,
// and M[t,j]=D(t,j) Q[t] K[j]^T, j<t. The prepared K[t]·Q[t]
// supplies the diagonal separately. The scan solves (I+L) U directly from
// beta(V-G K S^T), avoiding a rounded inverse and separately transformed
// values/state contributions. G[t]=prod(g[0..t]); D(t,j)=prod(g[j+1..t]).
// Every intermediate is f32.
kernel void clef_delta_block32_prepare(
    device const float *q [[buffer(0)]], device const float *k [[buffer(1)]],
    device const float *conv [[buffer(2)]], device const float *decay [[buffer(3)]],
    device const float *beta [[buffer(4)]], device float *workspace [[buffer(5)]],
    device const float *kq [[buffer(6)]], constant BlockDeltaArgs &a [[buffer(7)]],
    uint3 tg [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]], uint3 threads [[threads_per_threadgroup]]) {
    if (!valid_block_delta(a, threads) || tg.y >= a.value_heads ||
        tg.x >= (a.n + kBlock - 1) / kBlock) return;
    const uint first = tg.x * kBlock, count = min(kBlock, a.n - first), head = tg.y;
    const uint kh = delta_key_head(a, head);
    const int qk_stride = int(a.key_heads * kDim);
    const ulong qk_offset = (ulong(first) * a.key_heads + kh) * kDim;
    device float *work = workspace + (ulong(tg.x) * a.value_heads + head) * kWorkspace;
    threadgroup float lower[kBlock * kBlock];

    tensor<device float, dextents<int32_t, 2>, tensor_inline> keys(
        (device float *)(k + qk_offset), dextents<int32_t, 2>(kDim, int(count)),
        array<int32_t, 2>{1, qk_stride});
    tensor<device float, dextents<int32_t, 2>, tensor_inline> queries(
        (device float *)(q + qk_offset), dextents<int32_t, 2>(kDim, int(count)),
        array<int32_t, 2>{1, qk_stride});
    tensor<threadgroup float, dextents<int32_t, 2>, tensor_inline> lmatrix(
        lower, dextents<int32_t, 2>(int(count), int(count)), array<int32_t, 2>{1, kBlock});
    tensor<device float, dextents<int32_t, 2>, tensor_inline> mmatrix(
        work + kM, dextents<int32_t, 2>(int(count), int(count)), array<int32_t, 2>{1, kBlock});
    constexpr auto dot_desc = matmul2d_descriptor(kBlock, kBlock, kDim, false, true, false);
    matmul2d<dot_desc, execution_simdgroups<4>> dot;
    dot.run(keys, keys, lmatrix);
    threadgroup_barrier(mem_flags::mem_threadgroup);
    dot.run(queries, keys, mmatrix);
    threadgroup_barrier(mem_flags::mem_device | mem_flags::mem_threadgroup);

    for (uint i = tid; i < kBlock * kBlock; i += kThreads) {
        const uint row = i / kBlock, col = i % kBlock;
        const bool active = row < count && col < count;
        const float d = active && col <= row
            ? interval_decay(decay, first + col + 1, first + row + 1, head, a.value_heads) : 0.0f;
        // Branches avoid reading inactive TensorOps output, including 0*NaN.
        work[kL + i] = active && col < row ? lower[i] * d * beta[ulong(first + row) * a.value_heads + head] : 0.0f;
        work[kM + i] = active && col < row ? work[kM + i] * d : 0.0f;
    }
    if (tid < kBlock) {
        work[kG + tid] = tid < count ? interval_decay(decay, first, first + tid + 1, head, a.value_heads) : 0.0f;
        work[kLast + tid] = tid < count ? interval_decay(decay, first + tid + 1, first + count, head, a.value_heads) : 0.0f;
        work[kBeta + tid] = tid < count ? beta[ulong(first + tid) * a.value_heads + head] : 0.0f;
        work[kKq + tid] = tid < count ? kq[ulong(first + tid) * a.key_heads + kh] : 0.0f;
    }
    for (uint i = tid; i < count * kDim; i += kThreads) {
        const uint row = i / kDim, col = i % kDim;
        work[kV + i] = conv[ulong(first + row) * a.conv_width + a.value_offset + head * kDim + col];
    }
}

// One persistent group owns 32 value coordinates and carries its 32x128 f32
// state in threadgroup memory. Solve (I+L) U=diag(beta)(V-diag(G) K S^T)
// by forward substitution, then Y=diag(G) Q S^T+M U+diag(K·Q) U;
// S'=G[last] S+U^T diag(D(last,j)) K. No recurrent state is converted to half.
// Threadgroup storage is 24 KiB: state (16 KiB), U and Y (4 KiB each).
kernel void clef_delta_block32_scan(
    device const float *q [[buffer(0)]], device const float *k [[buffer(1)]],
    device const float *workspace [[buffer(2)]], device float *state [[buffer(3)]],
    device float *out [[buffer(4)]], constant BlockDeltaArgs &a [[buffer(5)]],
    uint3 tg [[threadgroup_position_in_grid]], uint tid [[thread_index_in_threadgroup]],
    uint3 threads [[threads_per_threadgroup]]) {
    if (!valid_block_delta(a, threads) || tg.x >= kDim / kValueTile || tg.y >= a.value_heads) return;
    const uint head = tg.y, kh = delta_key_head(a, head), value_first = tg.x * kValueTile;
    const ulong state_offset = (ulong(head) * kDim + value_first) * kDim;
    threadgroup float carried[kValueTile * kDim];
    threadgroup float correction[kBlock * kValueTile];
    threadgroup float result[kBlock * kValueTile];
    for (uint i = tid; i < kValueTile * kDim; i += kThreads) carried[i] = state[state_offset + i];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    tensor<threadgroup float, dextents<int32_t, 2>, tensor_inline> smatrix(
        carried, dextents<int32_t, 2>(kDim, kValueTile));
    constexpr auto carry_desc = matmul2d_descriptor(kBlock, kValueTile, kDim, false, true, false);
    constexpr auto output_desc = matmul2d_descriptor(kBlock, kValueTile, kBlock, false, false, false,
        matmul2d_descriptor::mode::multiply_accumulate);
    constexpr auto update_desc = matmul2d_descriptor(kValueTile, kDim, kBlock, true, false, false,
        matmul2d_descriptor::mode::multiply_accumulate);
    matmul2d<carry_desc, execution_simdgroups<4>> carry;
    matmul2d<output_desc, execution_simdgroups<4>> output;
    matmul2d<update_desc, execution_simdgroups<4>> update;

    for (uint first = 0; first < a.n; first += kBlock) {
        const uint count = min(kBlock, a.n - first);
        const ulong qk_offset = (ulong(first) * a.key_heads + kh) * kDim;
        device float *work = (device float *)(workspace + (ulong(first / kBlock) * a.value_heads + head) * kWorkspace);
        tensor<device float, dextents<int32_t, 2>, tensor_inline> queries(
            (device float *)(q + qk_offset), dextents<int32_t, 2>(kDim, int(count)),
            array<int32_t, 2>{1, int(a.key_heads * kDim)});
        tensor<device float, dextents<int32_t, 2>, tensor_inline> keys(
            (device float *)(k + qk_offset), dextents<int32_t, 2>(kDim, int(count)),
            array<int32_t, 2>{1, int(a.key_heads * kDim)});
        tensor<device float, dextents<int32_t, 2>, tensor_inline> m(
            work + kM, dextents<int32_t, 2>(int(count), int(count)), array<int32_t, 2>{1, kBlock});
        tensor<threadgroup float, dextents<int32_t, 2>, tensor_inline> u(
            correction, dextents<int32_t, 2>(kValueTile, int(count)));
        tensor<threadgroup float, dextents<int32_t, 2>, tensor_inline> y(
            result, dextents<int32_t, 2>(kValueTile, int(count)));
        carry.run(keys, smatrix, u);
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint i = tid; i < count * kValueTile; i += kThreads) {
            const uint row = i / kValueTile, col = i % kValueTile;
            correction[i] = work[kBeta + row] * fma(-work[kG + row], correction[i],
                work[kV + row * kDim + value_first + col]);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        // Solve each value column directly. The unit diagonal needs no divide.
        // A barrier publishes the completed pivot before later rows use it.
        for (uint pivot = 0; pivot < count; ++pivot) {
            for (uint i = tid; i < count * kValueTile; i += kThreads) {
                const uint row = i / kValueTile, col = i % kValueTile;
                if (row > pivot) correction[i] = fma(-work[kL + row * kBlock + pivot],
                    correction[pivot * kValueTile + col], correction[i]);
            }
            threadgroup_barrier(mem_flags::mem_threadgroup);
        }
        carry.run(queries, smatrix, y);
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint i = tid; i < count * kValueTile; i += kThreads) result[i] *= work[kG + i / kValueTile];
        threadgroup_barrier(mem_flags::mem_threadgroup);
        output.run(m, u, y);
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint i = tid; i < count * kValueTile; i += kThreads) {
            const uint row = i / kValueTile, col = i % kValueTile;
            // Match the sequential scan's final same-token contribution. U
            // already includes beta; the same-token decay product is one.
            out[(ulong(first + row) * a.value_heads + head) * kDim + value_first + col] =
                result[i] + correction[i] * work[kKq + row];
            correction[i] *= work[kLast + row];
        }
        for (uint i = tid; i < kValueTile * kDim; i += kThreads) carried[i] *= work[kG + count - 1];
        threadgroup_barrier(mem_flags::mem_threadgroup);
        update.run(u, keys, smatrix);
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    for (uint i = tid; i < kValueTile * kDim; i += kThreads) state[state_offset + i] = carried[i];
}
