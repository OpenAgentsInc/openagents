// Scratch reimplementation of explicit cooperative inputs. Public MLX source
// inspired the fragment organization; no MLX lane mapping is used here.
#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
using namespace metal;
using namespace mpp::tensor_ops;

constexpr auto register_desc = matmul2d_descriptor(
    16, 32, 16, false, true, false, matmul2d_descriptor::mode::multiply_accumulate);
using RegisterOp = matmul2d<register_desc, execution_simdgroup>;

template <typename C>
inline void load_left(thread C &dst, device const half *x,
                      uint row, uint k0, uint n, uint k) {
    if (row + 16 <= n && k0 + 16 <= k) {
        tensor<device half, dextents<int, 2>, tensor_inline> view(
            (device half *)(x + ulong(row) * k + k0),
            dextents<int, 2>(16, 16), array<int, 2>{1, int(k)});
        auto tile = view.template slice<16, 16>(0, 0);
        dst.load(tile);
    } else {
        #pragma clang loop unroll(full)
        for (uint16_t i = 0; i < dst.get_capacity(); ++i) {
            if (dst.is_valid_element(i)) {
                auto c = dst.get_multidimensional_index(i);
                const uint xk = k0 + uint(c[0]), xr = row + uint(c[1]);
                dst[i] = xr < n && xk < k ? x[ulong(xr) * k + xk] : half(0);
            }
        }
    }
}

template <typename C>
inline void load_right(thread C &dst, device const half *weight,
                       uint col, uint k0, uint m, uint k) {
    if (col + 32 <= m && k0 + 16 <= k) {
        tensor<device half, dextents<int, 2>, tensor_inline> view(
            (device half *)(weight + ulong(col) * k + k0),
            dextents<int, 2>(16, 32), array<int, 2>{1, int(k)});
        auto tile = view.template slice<16, 32>(0, 0);
        dst.load(tile);
    } else {
        #pragma clang loop unroll(full)
        for (uint16_t i = 0; i < dst.get_capacity(); ++i) {
            if (dst.is_valid_element(i)) {
                auto c = dst.get_multidimensional_index(i);
                const uint wk = k0 + uint(c[0]), wc = col + uint(c[1]);
                dst[i] = wc < m && wk < k ? weight[ulong(wc) * k + wk] : half(0);
            }
        }
    }
}

template <bool ACCUMULATE, typename C>
inline void initialize_result(thread C &dst, device const float *out,
                              uint row, uint col, uint n, uint m) {
    #pragma clang loop unroll(full)
    for (uint16_t i = 0; i < dst.get_capacity(); ++i) {
        if (dst.is_valid_element(i)) {
            auto c = dst.get_multidimensional_index(i);
            const uint yr = row + uint(c[1]), yc = col + uint(c[0]);
            dst[i] = ACCUMULATE && yr < n && yc < m ? out[ulong(yr) * m + yc] : 0.0f;
        }
    }
}

template <typename C>
inline void store_result(thread C &src, device float *out,
                         uint row, uint col, uint n, uint m) {
    if (row + 16 <= n && col + 32 <= m) {
        tensor<device float, dextents<int, 2>, tensor_inline> view(
            out + ulong(row) * m + col, dextents<int, 2>(32, 16), array<int, 2>{1, int(m)});
        auto tile = view.template slice<32, 16>(0, 0);
        src.store(tile);
    } else {
        #pragma clang loop unroll(full)
        for (uint16_t i = 0; i < src.get_capacity(); ++i) {
            if (src.is_valid_element(i)) {
                auto c = src.get_multidimensional_index(i);
                const uint yr = row + uint(c[1]), yc = col + uint(c[0]);
                if (yr < n && yc < m) out[ulong(yr) * m + yc] = src[i];
            }
        }
    }
}

template <bool ACCUMULATE>
inline void register_product(device const half *x, device const half *weight,
                             device float *out, uint3 dims, uint3 group,
                             uint simd_group, uint3 threads) {
    if (threads.x != 128 || threads.y != 1 || threads.z != 1 ||
        dims.x == 0 || dims.x > 2048 || dims.y == 0 || dims.y > 12288 ||
        dims.z == 0 || dims.z > 12288 ||
        group.x >= (dims.y + 63) / 64 || group.y >= (dims.x + 63) / 64) return;
    const uint row = group.y * 64 + (simd_group / 2) * 32;
    const uint col = group.x * 64 + (simd_group % 2) * 32;
    const bool active = row < dims.x && col < dims.y;
    RegisterOp op;
    using A = RegisterOp::cooperative_tensor_left_input_t<half, half, float>;
    using B = RegisterOp::cooperative_tensor_right_input_t<half, half, float>;
    auto c0 = op.get_destination_cooperative_tensor<A, B, float>();
    auto c1 = op.get_destination_cooperative_tensor<A, B, float>();
    initialize_result<ACCUMULATE>(c0, out, row, col, dims.x, dims.y);
    initialize_result<ACCUMULATE>(c1, out, row + 16, col, dims.x, dims.y);
    for (uint block = 0; block < dims.z; block += 128) {
        if (active) {
            for (uint k0 = block; k0 < min(block + 128, dims.z); k0 += 32) {
                auto a00 = op.get_left_input_cooperative_tensor<half, half, float>();
                auto a01 = op.get_left_input_cooperative_tensor<half, half, float>();
                auto a10 = op.get_left_input_cooperative_tensor<half, half, float>();
                auto a11 = op.get_left_input_cooperative_tensor<half, half, float>();
                auto b0 = op.get_right_input_cooperative_tensor<half, half, float>();
                auto b1 = op.get_right_input_cooperative_tensor<half, half, float>();
                load_left(a00, x, row, k0, dims.x, dims.z);
                load_left(a01, x, row, k0 + 16, dims.x, dims.z);
                load_left(a10, x, row + 16, k0, dims.x, dims.z);
                load_left(a11, x, row + 16, k0 + 16, dims.x, dims.z);
                load_right(b0, weight, col, k0, dims.y, dims.z);
                load_right(b1, weight, col, k0 + 16, dims.y, dims.z);
                op.run(a00, b0, c0);
                op.run(a10, b0, c1);
                op.run(a01, b1, c0);
                op.run(a11, b1, c1);
            }
        }
        // Every SIMD group participates, including inactive output tails.
        threadgroup_barrier(mem_flags::mem_none);
    }
    if (active) {
        store_result(c0, out, row, col, dims.x, dims.y);
        store_result(c1, out, row + 16, col, dims.x, dims.y);
    }
}

kernel void register_input(device const half *x [[buffer(0)]],
                           device const half *weight [[buffer(1)]],
                           device float *out [[buffer(2)]], constant uint3 &dims [[buffer(3)]],
                           uint3 group [[threadgroup_position_in_grid]],
                           uint simd_group [[simdgroup_index_in_threadgroup]],
                           uint3 threads [[threads_per_threadgroup]]) {
    register_product<false>(x, weight, out, dims, group, simd_group, threads);
}
kernel void register_input_accumulate(device const half *x [[buffer(0)]],
                                      device const half *weight [[buffer(1)]],
                                      device float *out [[buffer(2)]], constant uint3 &dims [[buffer(3)]],
                                      uint3 group [[threadgroup_position_in_grid]],
                                      uint simd_group [[simdgroup_index_in_threadgroup]],
                                      uint3 threads [[threads_per_threadgroup]]) {
    register_product<true>(x, weight, out, dims, group, simd_group, threads);
}

// Each lane writes its own fixed records. The host proves coordinate coverage
// and exact input bits without assuming a particular cooperative lane layout.
template <typename C>
inline void export_fragment(thread C &src, device uint *records, uint object, uint lane) {
    constexpr uint max_slots = 64, stride = 1 + max_slots * 4;
    device uint *r = records + (object * 32 + lane) * stride;
    r[0] = src.get_capacity();
    for (uint16_t i = 0; i < min(uint(src.get_capacity()), max_slots); ++i) {
        if (src.is_valid_element(i)) {
            auto c = src.get_multidimensional_index(i);
            uint bits;
            if constexpr (is_same_v<typename C::element_type, half>) bits = as_type<ushort>(src[i]);
            else bits = as_type<uint>(src[i]);
            r[1 + i * 4] = uint(c[0]); r[2 + i * 4] = uint(c[1]);
            r[3 + i * 4] = bits; r[4 + i * 4] = 1;
        }
    }
}

kernel void register_input_roundtrip(device const half *x [[buffer(0)]],
                                     device const half *weight [[buffer(1)]],
                                     device const float *seed [[buffer(2)]],
                                     constant uint3 &dims [[buffer(3)]],
                                     device uint *records [[buffer(4)]],
                                     constant uint3 &base [[buffer(5)]],
                                     uint lane [[thread_index_in_simdgroup]],
                                     uint3 threads [[threads_per_threadgroup]]) {
    if (threads.x != 32 || threads.y != 1 || threads.z != 1 ||
        dims.x == 0 || dims.x > 2048 || dims.y == 0 || dims.y > 12288 ||
        dims.z == 0 || dims.z > 12288 || base.x > dims.x || base.y > dims.y || base.z > dims.z) return;
    RegisterOp op;
    auto a00 = op.get_left_input_cooperative_tensor<half, half, float>();
    auto a01 = op.get_left_input_cooperative_tensor<half, half, float>();
    auto a10 = op.get_left_input_cooperative_tensor<half, half, float>();
    auto a11 = op.get_left_input_cooperative_tensor<half, half, float>();
    auto b0 = op.get_right_input_cooperative_tensor<half, half, float>();
    auto b1 = op.get_right_input_cooperative_tensor<half, half, float>();
    auto c0 = op.get_destination_cooperative_tensor<decltype(a00), decltype(b0), float>();
    auto c1 = op.get_destination_cooperative_tensor<decltype(a00), decltype(b0), float>();
    load_left(a00, x, base.x, base.z, dims.x, dims.z);
    load_left(a01, x, base.x, base.z + 16, dims.x, dims.z);
    load_left(a10, x, base.x + 16, base.z, dims.x, dims.z);
    load_left(a11, x, base.x + 16, base.z + 16, dims.x, dims.z);
    load_right(b0, weight, base.y, base.z, dims.y, dims.z);
    load_right(b1, weight, base.y, base.z + 16, dims.y, dims.z);
    initialize_result<true>(c0, seed, base.x, base.y, dims.x, dims.y);
    initialize_result<true>(c1, seed, base.x + 16, base.y, dims.x, dims.y);
    export_fragment(a00, records, 0, lane); export_fragment(a01, records, 1, lane);
    export_fragment(a10, records, 2, lane); export_fragment(a11, records, 3, lane);
    export_fragment(b0, records, 4, lane); export_fragment(b1, records, 5, lane);
    export_fragment(c0, records, 6, lane); export_fragment(c1, records, 7, lane);
}
