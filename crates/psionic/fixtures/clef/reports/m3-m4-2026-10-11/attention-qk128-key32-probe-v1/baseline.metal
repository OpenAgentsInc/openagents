// Experimental fused causal attention for Clef, compiled separately from the
// default kernel library. This path requires explicit selection and validation.

#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>

using namespace metal;
using namespace mpp::tensor_ops;

struct FlashArgs {
    uint n;
    uint heads;
    uint kv_heads;
    uint first;
};

// One threadgroup owns 16 queries of one query head and streams 32-key tiles.
// Dispatch (ceil(n / 16), heads, 1) threadgroups of exactly (128, 1, 1) threads.
// Q is [n, heads, 256]; K and V are [first + n, kv_heads, 256]; output is
// [n, heads, 256]. Q already contains the attention scale. Inputs must be finite.
// Arbitrary grouped-query ratios are supported when heads is divisible by
// kv_heads. Dynamic tensor extents bound tail loads; padded inputs are not needed.
//
// Both products use MPP matmul2d with f32 accumulation. Scores, probabilities,
// and the running output occupy 19,456 bytes of threadgroup memory, independent
// of the sequence length. No sequence-sized score or probability matrix exists.
// Online softmax rounds its unnormalized exponentials to f16 before the PV
// product and sums those same values in f32. This differs from the staged path,
// which rounds normalized probabilities; equality requires tolerance checks.
kernel void clef_flash_attention256_tensorops(
    device const half *q16 [[buffer(0)]], device const half *kcache [[buffer(1)]],
    device const half *vcache [[buffer(2)]], device float *out [[buffer(3)]],
    constant FlashArgs &a [[buffer(4)]], uint3 tg [[threadgroup_position_in_grid]],
    uint3 tid_v [[thread_position_in_threadgroup]], uint3 threads [[threads_per_threadgroup]]) {
    constexpr uint kQueries = 16, kKeys = 32, kDim = 256, kThreads = 128;
    constexpr uint kRowLanes = kThreads / kQueries;
    constexpr uint kIntMax = 0x7fffffff;
    const uint tid = tid_v.x;

    // These checks are uniform across the threadgroup and precede all barriers,
    // pointer arithmetic, and divisions. Tensor extents and strides use int32.
    if (threads.x != kThreads || threads.y != 1 || threads.z != 1 || a.n == 0 ||
        a.heads == 0 || a.kv_heads == 0 || a.n > kIntMax || a.heads > kIntMax / kDim ||
        a.kv_heads > kIntMax / kDim) {
        return;
    }
    if (a.heads % a.kv_heads != 0 || a.first > kIntMax - a.n || tg.y >= a.heads ||
        tg.x >= (a.n + kQueries - 1) / kQueries) {
        return;
    }

    threadgroup float scores[kQueries * kKeys];
    threadgroup half probabilities[kQueries * kKeys];
    threadgroup float running[kQueries * kDim];

    const uint head = tg.y;
    const uint kv_head = head / (a.heads / a.kv_heads);
    const uint query_base = tg.x * kQueries;
    const uint query_count = min(kQueries, a.n - query_base);
    const uint key_limit = a.first + query_base + query_count;
    const uint row = tid / kRowLanes;
    const uint lane = tid % kRowLanes;
    const bool active = row < query_count;
    const uint position = a.first + query_base + row;
    const int query_stride = int(a.heads * kDim);
    const int cache_stride = int(a.kv_heads * kDim);

    tensor<device half, dextents<int32_t, 2>, tensor_inline> queries(
        (device half *)(q16 + (ulong(query_base) * a.heads + head) * kDim),
        dextents<int32_t, 2>(int(kDim), int(query_count)), array<int32_t, 2>{1, query_stride});
    tensor<threadgroup float, dextents<int32_t, 2>, tensor_inline> score_tile(
        scores, dextents<int32_t, 2>(int(kKeys), int(query_count)), array<int32_t, 2>{1, int(kKeys)});
    tensor<threadgroup float, dextents<int32_t, 2>, tensor_inline> output_tile(
        running, dextents<int32_t, 2>(int(kDim), int(query_count)));

    constexpr auto score_desc = matmul2d_descriptor(
        kQueries, kKeys, kDim, false, true, false, matmul2d_descriptor::mode::multiply);
    constexpr auto value_desc = matmul2d_descriptor(
        kQueries, kDim, kKeys, false, false, false, matmul2d_descriptor::mode::multiply_accumulate);
    matmul2d<score_desc, execution_simdgroups<4>> score_op;
    matmul2d<value_desc, execution_simdgroups<4>> value_op;

    for (uint d = lane; d < kDim; d += kRowLanes) {
        running[row * kDim + d] = 0.0f;
    }
    float row_max = -INFINITY;
    float row_sum = 0.0f;
    threadgroup_barrier(mem_flags::mem_threadgroup);

    for (uint key_base = 0; key_base < key_limit; key_base += kKeys) {
        const uint key_count = min(kKeys, key_limit - key_base);
        const ulong cache_offset = (ulong(key_base) * a.kv_heads + kv_head) * kDim;
        tensor<device half, dextents<int32_t, 2>, tensor_inline> keys(
            (device half *)(kcache + cache_offset), dextents<int32_t, 2>(int(kDim), int(key_count)),
            array<int32_t, 2>{1, cache_stride});
        tensor<device half, dextents<int32_t, 2>, tensor_inline> values(
            (device half *)(vcache + cache_offset), dextents<int32_t, 2>(int(kDim), int(key_count)),
            array<int32_t, 2>{1, cache_stride});
        tensor<threadgroup half, dextents<int32_t, 2>, tensor_inline> probability_tile(
            probabilities, dextents<int32_t, 2>(int(key_count), int(query_count)),
            array<int32_t, 2>{1, int(kKeys)});

        score_op.run(queries, keys, score_tile);
        threadgroup_barrier(mem_flags::mem_threadgroup);

        // Eight adjacent lanes own a query row. XOR shuffles stay inside each
        // eight-lane group, including all four row groups within a SIMD group.
        float row_scores[kKeys / kRowLanes];
        float tile_max = -INFINITY;
        for (uint c = 0; c < kKeys / kRowLanes; ++c) {
            const uint key = lane * (kKeys / kRowLanes) + c;
            const bool visible = active && key < key_count && key_base + key <= position;
            row_scores[c] = visible ? scores[row * kKeys + key] : -INFINITY;
            tile_max = max(tile_max, row_scores[c]);
        }
        tile_max = max(tile_max, simd_shuffle_xor(tile_max, ushort(1)));
        tile_max = max(tile_max, simd_shuffle_xor(tile_max, ushort(2)));
        tile_max = max(tile_max, simd_shuffle_xor(tile_max, ushort(4)));
        const float new_max = max(row_max, tile_max);
        // A wholly masked tile preserves the preceding state. This also avoids
        // exp(-infinity - -infinity) for inactive tail rows.
        const float alpha = new_max == row_max ? 1.0f : exp(row_max - new_max);
        float tile_sum = 0.0f;
        for (uint c = 0; c < kKeys / kRowLanes; ++c) {
            const uint key = lane * (kKeys / kRowLanes) + c;
            const half p = row_scores[c] == -INFINITY ? half(0) : half(exp(row_scores[c] - new_max));
            probabilities[row * kKeys + key] = p;
            tile_sum += float(p);
        }
        tile_sum += simd_shuffle_xor(tile_sum, ushort(1));
        tile_sum += simd_shuffle_xor(tile_sum, ushort(2));
        tile_sum += simd_shuffle_xor(tile_sum, ushort(4));
        row_sum = row_sum * alpha + tile_sum;
        row_max = new_max;
        for (uint d = lane; d < kDim; d += kRowLanes) {
            running[row * kDim + d] *= alpha;
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);

        value_op.run(probability_tile, values, output_tile);
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }

    if (active) {
        const float inv = 1.0f / row_sum;
        const ulong output_offset = (ulong(query_base + row) * a.heads + head) * kDim;
        for (uint d = lane; d < kDim; d += kRowLanes) {
            out[output_offset + d] = running[row * kDim + d] * inv;
        }
    }
}
