#include <cuda_runtime.h>
#include <cuda_fp16.h>
#include <cuda_bf16.h>
#include <cccl/cub/block/block_radix_sort.cuh>
#include <cccl/cub/device/device_radix_sort.cuh>

#include <cfloat>
#include <stdint.h>

namespace {

constexpr int kBlockSize = 256;
constexpr int kLogitsTopKBlockSize = 256;
constexpr int kLogitsTopKItemsPerThread = 12;
constexpr int kLogitsTopKTileSize = kLogitsTopKBlockSize * kLogitsTopKItemsPerThread;
constexpr int kAttentionBlockSize = 256;
constexpr int kMatvecBlockSize = 128;
constexpr int kWarpSize = 32;
constexpr int kMmvqWarps = kMatvecBlockSize / kWarpSize;
constexpr int kMaxWarpsPerBlock = 1024 / kWarpSize;
constexpr int kQ81ElementsPerBlock = 32;
constexpr int kQ80BlockBytes = 34;
constexpr int kQ81BlockBytes = 36;
constexpr int kQ3KBlockBytes = 110;
constexpr int kQ4KBlockBytes = 144;
constexpr int kQ5KBlockBytes = 176;
constexpr int kQ6KBlockBytes = 210;
constexpr int kIq3SBlockBytes = 110;
constexpr int kIq4XsBlockBytes = 136;
constexpr int kAttentionMaxPositions = 1024;
constexpr int kAttentionFa3MaxSplits = 8;
constexpr int kAttentionFa3MaxHeadsPerGroup = 8;
constexpr int kAttentionFa3MaxHeadDim = 256;
constexpr int kAttentionFa3MaxDimsPerLane = kAttentionFa3MaxHeadDim / kWarpSize;
constexpr int kMoeMaxExperts = 128;
constexpr int kMoeMaxSelected = 32;
constexpr int kLogitsMaxSelected = 128;
constexpr int kLogitsFastSharedTopK = kLogitsMaxSelected;

#include "qwen38_quant_tables.cuh"

__device__ __constant__ int8_t kIq4NlValues[16] = {
    -127, -104, -83, -65, -49, -35, -22, -10, 1, 13, 25, 38, 53, 69, 89, 113,
};

__device__ __forceinline__ float half_to_float(uint16_t bits) {
    const uint32_t sign = static_cast<uint32_t>(bits & 0x8000u) << 16;
    const uint32_t exponent = (bits >> 10) & 0x1fu;
    const uint32_t mantissa = bits & 0x03ffu;

    uint32_t out_exponent = 0;
    uint32_t out_mantissa = 0;

    if (exponent == 0) {
        if (mantissa != 0) {
            uint32_t normalized = mantissa;
            uint32_t shift = 0;
            while ((normalized & 0x0400u) == 0) {
                normalized <<= 1;
                ++shift;
            }
            normalized &= 0x03ffu;
            out_exponent = 113u - shift;
            out_mantissa = normalized << 13;
        }
    } else if (exponent == 0x1fu) {
        out_exponent = 0xffu;
        out_mantissa = static_cast<uint32_t>(mantissa) << 13;
    } else {
        out_exponent = static_cast<uint32_t>(exponent) + 112u;
        out_mantissa = static_cast<uint32_t>(mantissa) << 13;
    }

    return __uint_as_float(sign | (out_exponent << 23) | out_mantissa);
}

__device__ __forceinline__ float decode_e8m0_to_fp32_half(uint8_t value) {
    const uint32_t bits = value == 0 ? 0x00400000u : (static_cast<uint32_t>(value) << 23);
    return __uint_as_float(bits);
}

__device__ __forceinline__ float mxfp4_value(uint8_t nibble) {
    switch (nibble & 0x0fu) {
        case 0x0: return 0.0f;
        case 0x1: return 1.0f;
        case 0x2: return 2.0f;
        case 0x3: return 3.0f;
        case 0x4: return 4.0f;
        case 0x5: return 6.0f;
        case 0x6: return 8.0f;
        case 0x7: return 12.0f;
        case 0x8: return 0.0f;
        case 0x9: return -1.0f;
        case 0xa: return -2.0f;
        case 0xb: return -3.0f;
        case 0xc: return -4.0f;
        case 0xd: return -6.0f;
        case 0xe: return -8.0f;
        case 0xf: return -12.0f;
        default: return 0.0f;
    }
}

__device__ __forceinline__ int2 decode_q4_k_scale_min(int index, const uint8_t *packed) {
    if (index < 4) {
        return make_int2(
            static_cast<int>(packed[index] & 63),
            static_cast<int>(packed[index + 4] & 63)
        );
    }
    return make_int2(
        static_cast<int>((packed[index + 4] & 0x0f) | ((packed[index - 4] >> 6) << 4)),
        static_cast<int>((packed[index + 4] >> 4) | ((packed[index] >> 6) << 4))
    );
}

__device__ __forceinline__ float swiglu_oai_single(
    float gate,
    float up,
    float alpha = 1.702f,
    float limit = 7.0f
) {
    gate = fminf(gate, limit);
    up = fmaxf(fminf(up, limit), -limit);
    const float out_glu = gate / (1.0f + expf(-gate * alpha));
    return out_glu * (1.0f + up);
}

__device__ __forceinline__ float sigmoid_single(float value) {
    return 1.0f / (1.0f + expf(-value));
}

__device__ __forceinline__ float softplus_single(float value) {
    return value > 20.0f ? value : log1pf(expf(value));
}

__device__ __forceinline__ float silu_single(float value) {
    return value * sigmoid_single(value);
}

struct Q80Block {
    uint8_t bytes[kQ80BlockBytes];
};
static_assert(sizeof(Q80Block) == kQ80BlockBytes, "wrong q8_0 block size");

struct Q81Block {
    uint8_t bytes[kQ81BlockBytes];
};
static_assert(sizeof(Q81Block) == kQ81BlockBytes, "wrong q8_1 block size");

struct Mxfp4Block {
    uint8_t bytes[kQ81ElementsPerBlock / 2 + 1];
};
static_assert(sizeof(Mxfp4Block) == 17, "wrong mxfp4 block size");

__device__ __forceinline__ uint16_t load_u16_le(const uint8_t *bytes) {
    return static_cast<uint16_t>(bytes[0]) | (static_cast<uint16_t>(bytes[1]) << 8);
}

__device__ __forceinline__ int dp4a_i8(int lhs, int rhs, int accumulator) {
#if defined(__CUDA_ARCH__) && __CUDA_ARCH__ >= 610
    return __dp4a(lhs, rhs, accumulator);
#else
    const int8_t *lhs_bytes = reinterpret_cast<const int8_t *>(&lhs);
    const int8_t *rhs_bytes = reinterpret_cast<const int8_t *>(&rhs);
    return accumulator +
        static_cast<int>(lhs_bytes[0]) * static_cast<int>(rhs_bytes[0]) +
        static_cast<int>(lhs_bytes[1]) * static_cast<int>(rhs_bytes[1]) +
        static_cast<int>(lhs_bytes[2]) * static_cast<int>(rhs_bytes[2]) +
        static_cast<int>(lhs_bytes[3]) * static_cast<int>(rhs_bytes[3]);
#endif
}

static __device__ __forceinline__ int get_int_b1(const void *x, const int i32) {
    const uint8_t *x8 = static_cast<const uint8_t *>(x);
    int x32 = x8[4 * i32 + 0] << 0;
    x32 |= x8[4 * i32 + 1] << 8;
    x32 |= x8[4 * i32 + 2] << 16;
    x32 |= x8[4 * i32 + 3] << 24;
    return x32;
}

static __device__ __forceinline__ int get_int_b2(const void *x, const int i32) {
    const uint16_t *x16 = static_cast<const uint16_t *>(x);
    int x32 = x16[2 * i32 + 0] << 0;
    x32 |= x16[2 * i32 + 1] << 16;
    return x32;
}

static __device__ __forceinline__ int get_int_b4(const void *x, const int i32) {
    return static_cast<const int *>(x)[i32];
}

static __device__ __forceinline__ int2 get_int_from_table_16(const int q4, const int8_t *table) {
    const uint32_t *table32 = reinterpret_cast<const uint32_t *>(table);
    uint32_t tmp[2];
    const uint32_t low_high_selection_indices =
        0x32103210u | (static_cast<uint32_t>(q4 & 0x88888888u) >> 1);
#pragma unroll
    for (uint32_t i = 0; i < 2; ++i) {
        const uint32_t shift = 16u * i;
        const uint32_t low = __byte_perm(table32[0], table32[1], static_cast<uint32_t>(q4) >> shift);
        const uint32_t high = __byte_perm(table32[2], table32[3], static_cast<uint32_t>(q4) >> shift);
        tmp[i] = __byte_perm(low, high, low_high_selection_indices >> shift);
    }
    return make_int2(
        static_cast<int>(__byte_perm(tmp[0], tmp[1], 0x6420)),
        static_cast<int>(__byte_perm(tmp[0], tmp[1], 0x7531))
    );
}

__device__ __constant__ int8_t kMxfp4IntTable[16] = {
    0, 1, 2, 3, 4, 6, 8, 12, 0, -1, -2, -3, -4, -6, -8, -12,
};

__device__ __constant__ int8_t kQ4KIntTable[16] = {
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
};

__device__ __forceinline__ float dot_q8_0_q8_1_block(
    const Q80Block *weight_block,
    const Q81Block *input_block
) {
    int sum = 0;
#pragma unroll
    for (int i = 0; i < kQ81ElementsPerBlock / 4; ++i) {
        sum = dp4a_i8(
            get_int_b2(weight_block->bytes + 2, i),
            get_int_b1(input_block->bytes + 4, i),
            sum
        );
    }
    return half_to_float(load_u16_le(weight_block->bytes)) *
        half_to_float(load_u16_le(input_block->bytes)) *
        static_cast<float>(sum);
}

__device__ __forceinline__ float dot_mxfp4_q8_1_block(
    const Mxfp4Block *weight_block,
    const Q81Block *input_block
) {
    int sum = 0;
#pragma unroll
    for (int lane_group = 0; lane_group < 4; ++lane_group) {
        const int packed = get_int_b1(weight_block->bytes + 1, lane_group);
        const int2 dequantized = get_int_from_table_16(packed, kMxfp4IntTable);
        sum = dp4a_i8(dequantized.x, get_int_b1(input_block->bytes + 4, lane_group + 0), sum);
        sum = dp4a_i8(dequantized.y, get_int_b1(input_block->bytes + 4, lane_group + 4), sum);
    }
    return decode_e8m0_to_fp32_half(weight_block->bytes[0]) * 0.5f *
        half_to_float(load_u16_le(input_block->bytes)) *
        static_cast<float>(sum);
}

__device__ __forceinline__ float warp_reduce_sum(float value) {
#pragma unroll
    for (int offset = kWarpSize / 2; offset > 0; offset >>= 1) {
        value += __shfl_xor_sync(0xffffffffu, value, offset, kWarpSize);
    }
    return value;
}

__device__ __forceinline__ float reduce_block_sum(float value, float *scratch) {
    scratch[threadIdx.x] = value;
    __syncthreads();

    for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (threadIdx.x < offset) {
            scratch[threadIdx.x] += scratch[threadIdx.x + offset];
        }
        __syncthreads();
    }

    return scratch[0];
}

__device__ __forceinline__ float reduce_block_max(float value, float *scratch) {
    scratch[threadIdx.x] = value;
    __syncthreads();

    for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (threadIdx.x < offset) {
            scratch[threadIdx.x] = fmaxf(scratch[threadIdx.x], scratch[threadIdx.x + offset]);
        }
        __syncthreads();
    }

    return scratch[0];
}

__device__ __forceinline__ float dot_query_half_key_pairwise(
    const float *query,
    const __half *key,
    int head_dim
) {
    float dot = 0.0f;
    int dim = 0;
    for (; dim + 1 < head_dim; dim += 2) {
        const __half2 key_pair = *reinterpret_cast<const __half2 *>(key + dim);
        const float2 key_pair_f32 = __half22float2(key_pair);
        dot = fmaf(query[dim], key_pair_f32.x, dot);
        dot = fmaf(query[dim + 1], key_pair_f32.y, dot);
    }
    if (dim < head_dim) {
        dot = fmaf(query[dim], __half2float(key[dim]), dot);
    }
    return dot;
}

__device__ __forceinline__ float q81_block_value(const Q81Block *block, int lane) {
    const float scale = half_to_float(load_u16_le(block->bytes));
    const int8_t quantized = reinterpret_cast<const int8_t *>(block->bytes + 4)[lane];
    return static_cast<float>(quantized) * scale;
}

__device__ __forceinline__ float dot_query_q81_key(
    const float *query,
    const Q81Block *key_blocks,
    int head_dim
) {
    float dot = 0.0f;
    const int block_count = head_dim / kQ81ElementsPerBlock;
    for (int block_index = 0; block_index < block_count; ++block_index) {
        const Q81Block *block = key_blocks + block_index;
        const float scale = half_to_float(load_u16_le(block->bytes));
        const int8_t *quantized = reinterpret_cast<const int8_t *>(block->bytes + 4);
#pragma unroll
        for (int lane = 0; lane < kQ81ElementsPerBlock; ++lane) {
            dot = fmaf(
                query[block_index * kQ81ElementsPerBlock + lane],
                static_cast<float>(quantized[lane]) * scale,
                dot
            );
        }
    }
    return dot;
}

constexpr int kQ80Q81MmvqVdr = 2;
constexpr int kQ80Qi = kQ81ElementsPerBlock / 4;
constexpr int kMxfp4Q81MmvqVdr = 2;
constexpr int kMxfp4Qi = kQ81ElementsPerBlock / 8;
constexpr int kQ4KQ81MmvqVdr = 2;
constexpr int kQ4KQi = 32;
constexpr int kQ6KQ81MmvqVdr = 1;
constexpr int kQ6KQi = 32;
template <typename DotFn>
__global__ void quantized_matvec_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int cols,
    const float *input,
    float *output,
    DotFn dot_fn
) {
    const int row = blockIdx.x;
    if (row >= rows) {
        return;
    }

    float sum = 0.0f;
    const uint8_t *row_weights = weights + static_cast<size_t>(row) * static_cast<size_t>(row_stride);
    for (int index = threadIdx.x; index < cols; index += blockDim.x) {
        sum += dot_fn(row_weights, index, input[index]);
    }

    __shared__ float scratch[kBlockSize];
    scratch[threadIdx.x] = sum;
    __syncthreads();

    for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (threadIdx.x < offset) {
            scratch[threadIdx.x] += scratch[threadIdx.x + offset];
        }
        __syncthreads();
    }

    if (threadIdx.x == 0) {
        output[row] = scratch[0];
    }
}

struct Q80Dot {
    __device__ __forceinline__ float operator()(const uint8_t *row_weights, int index, float input) const {
        const int block_index = index >> 5;
        const int lane = index & 31;
        const uint8_t *block = row_weights + block_index * 34;
        const float scale = half_to_float(static_cast<uint16_t>(block[0]) | (static_cast<uint16_t>(block[1]) << 8));
        const int8_t quantized = reinterpret_cast<const int8_t *>(block + 2)[lane];
        return static_cast<float>(quantized) * scale * input;
    }
};

struct Mxfp4Dot {
    __device__ __forceinline__ float operator()(const uint8_t *row_weights, int index, float input) const {
        const int block_index = index >> 5;
        const int lane = index & 31;
        const uint8_t *block = row_weights + block_index * 17;
        const float scale = decode_e8m0_to_fp32_half(block[0]);
        const uint8_t packed = block[1 + (lane & 15)];
        const uint8_t nibble = lane < 16 ? (packed & 0x0f) : ((packed >> 4) & 0x0f);
        return mxfp4_value(nibble) * scale * 0.5f * input;
    }
};

struct Q4KDot {
    __device__ __forceinline__ float operator()(const uint8_t *row_weights, int index, float input) const {
        const int block_index = index >> 8;
        const int lane = index & 255;
        const uint8_t *block = row_weights + block_index * kQ4KBlockBytes;
        const float scale = half_to_float(load_u16_le(block));
        const float minimum = half_to_float(load_u16_le(block + 2));
        const uint8_t *scales = block + 4;
        const uint8_t *quants = block + 16;
        const int quant_chunk = lane >> 6;
        const int chunk_offset = lane & 63;
        const int quant_byte_index = chunk_offset & 31;
        const int scale_index = quant_chunk * 2 + (chunk_offset >> 5);
        const int2 scale_min = decode_q4_k_scale_min(scale_index, scales);
        const uint8_t packed = quants[quant_chunk * 32 + quant_byte_index];
        const float quantized = static_cast<float>(chunk_offset < 32 ? (packed & 0x0f) : ((packed >> 4) & 0x0f));
        return (scale * static_cast<float>(scale_min.x) * quantized -
            minimum * static_cast<float>(scale_min.y)) * input;
    }
};

struct Q6KDot {
    __device__ __forceinline__ float operator()(const uint8_t *row_weights, int index, float input) const {
        const int block_index = index >> 8;
        const int lane = index & 255;
        const uint8_t *block = row_weights + block_index * kQ6KBlockBytes;
        const uint8_t *ql = block;
        const uint8_t *qh = block + 128;
        const int8_t *scales = reinterpret_cast<const int8_t *>(block + 192);
        const float scale = half_to_float(load_u16_le(block + 208));
        const int chunk_index = lane >> 7;
        const int lane_in_chunk = lane & 127;
        const int group = lane_in_chunk >> 5;
        const int l = lane_in_chunk & 31;
        const uint8_t *ql_chunk = ql + chunk_index * 64;
        const uint8_t qh_value = qh[chunk_index * 32 + l];
        const int scale_index_base = chunk_index * 8 + (l / 16);
        int quantized = 0;
        int scale_index = 0;
        switch (group) {
            case 0:
                quantized = static_cast<int>(ql_chunk[l] & 0x0f) |
                    (static_cast<int>((qh_value >> 0) & 0x03) << 4);
                scale_index = scale_index_base;
                break;
            case 1:
                quantized = static_cast<int>(ql_chunk[l + 32] & 0x0f) |
                    (static_cast<int>((qh_value >> 2) & 0x03) << 4);
                scale_index = scale_index_base + 2;
                break;
            case 2:
                quantized = static_cast<int>(ql_chunk[l] >> 4) |
                    (static_cast<int>((qh_value >> 4) & 0x03) << 4);
                scale_index = scale_index_base + 4;
                break;
            default:
                quantized = static_cast<int>(ql_chunk[l + 32] >> 4) |
                    (static_cast<int>((qh_value >> 6) & 0x03) << 4);
                scale_index = scale_index_base + 6;
                break;
        }
        return scale * static_cast<float>(scales[scale_index]) *
            static_cast<float>(quantized - 32) * input;
    }
};

__device__ __forceinline__ int q3_k_scale(const uint8_t *packed, int scale_index) {
    const uint32_t low0 = static_cast<uint32_t>(get_int_b1(packed, 0));
    const uint32_t low1 = static_cast<uint32_t>(get_int_b1(packed, 1));
    const uint32_t upper = static_cast<uint32_t>(get_int_b1(packed, 2));
    constexpr uint32_t mask_two = 0x03030303u;
    constexpr uint32_t mask_four = 0x0f0f0f0fu;
    uint32_t word = 0;
    switch (scale_index >> 2) {
        case 0:
            word = (low0 & mask_four) | ((upper & mask_two) << 4);
            break;
        case 1:
            word = (low1 & mask_four) | (((upper >> 2) & mask_two) << 4);
            break;
        case 2:
            word = ((low0 >> 4) & mask_four) | (((upper >> 4) & mask_two) << 4);
            break;
        default:
            word = ((low1 >> 4) & mask_four) | (((upper >> 6) & mask_two) << 4);
            break;
    }
    return static_cast<int>((word >> (8 * (scale_index & 3))) & 0xffu) - 32;
}

struct Q3KValue {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int block_index,
        int lane
    ) const {
        const uint8_t *block = row_weights + block_index * kQ3KBlockBytes;
        const int half_index = lane >> 7;
        const int lane_in_half = lane & 127;
        const int shift_group = lane_in_half >> 5;
        const int lane_in_group = lane_in_half & 31;
        const int quant_offset = lane_in_group & 15;
        const int scale_index = half_index * 8 + shift_group * 2 + (lane_in_group >> 4);
        const uint8_t low =
            (block[32 + half_index * 32 + quant_offset + (lane_in_group >> 4) * 16] >>
             (2 * shift_group)) & 3;
        const uint8_t high_mask = static_cast<uint8_t>(1u << (half_index * 4 + shift_group));
        const int high = (block[quant_offset + (lane_in_group >> 4) * 16] & high_mask) == 0 ? 4 : 0;
        const float scale = half_to_float(load_u16_le(block + 108));
        return scale * static_cast<float>(q3_k_scale(block + 96, scale_index)) *
            static_cast<float>(static_cast<int>(low) - high);
    }
};

struct Q5KValue {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int block_index,
        int lane
    ) const {
        const uint8_t *block = row_weights + block_index * kQ5KBlockBytes;
        const float scale = half_to_float(load_u16_le(block));
        const float minimum = half_to_float(load_u16_le(block + 2));
        const uint8_t *scales = block + 4;
        const uint8_t *high_bits = block + 16;
        const uint8_t *quants = block + 48;
        const int quant_chunk = lane >> 6;
        const int chunk_offset = lane & 63;
        const int quant_index = quant_chunk * 32 + (chunk_offset & 31);
        const int scale_index = quant_chunk * 2 + (chunk_offset >> 5);
        const int2 scale_min = decode_q4_k_scale_min(scale_index, scales);
        const uint8_t packed = quants[quant_index];
        const uint8_t nibble = chunk_offset < 32 ? (packed & 0x0f) : (packed >> 4);
        const uint8_t mask = static_cast<uint8_t>((chunk_offset < 32 ? 1u : 2u) << (2 * quant_chunk));
        const int quantized = static_cast<int>(nibble) +
            ((high_bits[chunk_offset & 31] & mask) != 0 ? 16 : 0);
        return scale * static_cast<float>(scale_min.x) * static_cast<float>(quantized) -
            minimum * static_cast<float>(scale_min.y);
    }
};

struct Iq3SValue {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int block_index,
        int lane
    ) const {
        const uint8_t *block = row_weights + block_index * kIq3SBlockBytes;
        const int scale_block = lane >> 5;
        const int lane_in_block = lane & 31;
        const int code_pair = lane_in_block >> 3;
        const int lane_in_code_pair = lane_in_block & 7;
        const int code_selector = lane_in_code_pair >> 2;
        const int code_byte = lane_in_code_pair & 3;
        const uint8_t high = block[66 + scale_block];
        const int high_bit = 2 * code_pair + code_selector;
        const int code_index = static_cast<int>(
            block[2 + scale_block * 8 + code_pair * 2 + code_selector]
        ) | (static_cast<int>((high >> high_bit) & 1) << 8);
        const uint32_t grid = kIq3SGrid[code_index];
        const int magnitude = static_cast<int>((grid >> (8 * code_byte)) & 0xffu);
        const uint8_t sign_bits = block[74 + scale_block * 4 + code_pair];
        const int sign = (sign_bits & (1u << lane_in_code_pair)) == 0 ? 1 : -1;
        const uint8_t packed_scale = block[106 + scale_block / 2];
        const int local_scale = 1 + 2 * static_cast<int>(
            scale_block % 2 == 0 ? (packed_scale & 0x0f) : (packed_scale >> 4)
        );
        return half_to_float(load_u16_le(block)) * static_cast<float>(local_scale * magnitude * sign);
    }
};

struct Iq4XsValue {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int block_index,
        int lane
    ) const {
        const uint8_t *block = row_weights + block_index * kIq4XsBlockBytes;
        const int scale_block = lane >> 5;
        const int lane_in_block = lane & 31;
        const uint16_t scale_high = load_u16_le(block + 2);
        const uint8_t scale_low = block[4 + scale_block / 2];
        const int packed_scale = static_cast<int>(
            (scale_low >> (4 * (scale_block % 2))) & 0x0f
        ) | (static_cast<int>((scale_high >> (2 * scale_block)) & 3) << 4);
        const uint8_t packed = block[8 + scale_block * 16 + (lane_in_block & 15)];
        const uint8_t quant = lane_in_block < 16 ? (packed & 0x0f) : (packed >> 4);
        return half_to_float(load_u16_le(block)) * static_cast<float>(packed_scale - 32) *
            static_cast<float>(kIq4NlValues[quant]);
    }
};

template <typename ValueFn>
struct SuperBlockDot {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int index,
        float input
    ) const {
        return ValueFn{}(row_weights, index >> 8, index & 255) * input;
    }
};

template <typename ValueFn>
struct SuperBlockRowDequant {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int block_index,
        int lane
    ) const {
        const int index = block_index * kQ81ElementsPerBlock + lane;
        return ValueFn{}(row_weights, index >> 8, index & 255);
    }
};

template <typename ValueFn>
struct SuperBlockQ81Dot {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int input_block_index,
        const Q81Block *input
    ) const {
        const Q81Block *input_block = input + input_block_index;
        const int8_t *input_quants = reinterpret_cast<const int8_t *>(input_block->bytes + 4);
        const int base_lane = (input_block_index & 7) * kQ81ElementsPerBlock;
        float sum = 0.0f;
#pragma unroll
        for (int lane = 0; lane < kQ81ElementsPerBlock; ++lane) {
            sum += ValueFn{}(row_weights, input_block_index >> 3, base_lane + lane) *
                static_cast<float>(input_quants[lane]);
        }
        return sum * half_to_float(load_u16_le(input_block->bytes));
    }
};

// Specialized integer dots for the super-block quant types in the mmvq kernel
// shape: (weights, input, weight_block_index, input_block_index, quant_index).
// With Vdr = 4 and Qi = 32, eight threads share one super-block and thread
// `quant_index >> 2` owns one 32-element sub-block, so each thread reads
// contiguous weight bytes and accumulates with dp4a int8 dots instead of the
// scalar per-element value functor.

struct Iq4XsQ81Dot {
    __device__ __forceinline__ float operator()(
        const uint8_t *weights,
        const Q81Block *input,
        int weight_block_index,
        int input_block_index,
        int quant_index
    ) const {
        const uint8_t *block =
            weights + static_cast<size_t>(weight_block_index) * kIq4XsBlockBytes;
        const int scale_block = quant_index >> 2;
        const uint16_t scale_high = load_u16_le(block + 2);
        const uint8_t scale_low = block[4 + scale_block / 2];
        const int packed_scale = static_cast<int>(
            (scale_low >> (4 * (scale_block % 2))) & 0x0f
        ) | (static_cast<int>((scale_high >> (2 * scale_block)) & 3) << 4);
        const uint8_t *quants = block + 8 + scale_block * 16;
        const Q81Block *input_block =
            input + static_cast<size_t>(input_block_index) * 8 + scale_block;
        // The block base plus 8 keeps the 16 quant bytes 8-byte aligned.
        const int2 quant_words_lo = *reinterpret_cast<const int2 *>(quants);
        const int2 quant_words_hi = *reinterpret_cast<const int2 *>(quants + 8);
        int sum = 0;
#pragma unroll
        for (int word = 0; word < 4; ++word) {
            const int2 dequantized = get_int_from_table_16(
                word == 0   ? quant_words_lo.x
                : word == 1 ? quant_words_lo.y
                : word == 2 ? quant_words_hi.x
                            : quant_words_hi.y,
                kIq4NlValues
            );
            sum = dp4a_i8(
                dequantized.x,
                get_int_b4(input_block->bytes + 4, word),
                sum
            );
            sum = dp4a_i8(
                dequantized.y,
                get_int_b4(input_block->bytes + 4, word + 4),
                sum
            );
        }
        return half_to_float(load_u16_le(block)) *
            static_cast<float>(packed_scale - 32) * static_cast<float>(sum) *
            half_to_float(load_u16_le(input_block->bytes));
    }
};

struct Iq3SQ81Dot {
    __device__ __forceinline__ float operator()(
        const uint8_t *weights,
        const Q81Block *input,
        int weight_block_index,
        int input_block_index,
        int quant_index
    ) const {
        const uint8_t *block =
            weights + static_cast<size_t>(weight_block_index) * kIq3SBlockBytes;
        const int scale_block = quant_index >> 2;
        const Q81Block *input_block =
            input + static_cast<size_t>(input_block_index) * 8 + scale_block;
        const uint8_t *code_bytes = block + 2 + scale_block * 8;
        const uint8_t high_byte = block[66 + scale_block];
        const uint8_t *sign_bytes = block + 74 + scale_block * 4;
        int sum = 0;
#pragma unroll
        for (int word = 0; word < 8; ++word) {
            const int code_index = static_cast<int>(code_bytes[word]) |
                (static_cast<int>((high_byte >> word) & 1) << 8);
            const uint32_t magnitudes = kIq3SGrid[code_index];
            const uint32_t sign_nibble =
                (sign_bytes[word >> 1] >> (4 * (word & 1))) & 0x0fu;
            const uint32_t sign_bytes_expanded =
                ((sign_nibble * 0x00204081u) & 0x01010101u) * 0xffu;
            const uint32_t weights =
                __vsub4(magnitudes ^ sign_bytes_expanded, sign_bytes_expanded);
            sum = dp4a_i8(
                static_cast<int>(weights),
                get_int_b4(input_block->bytes + 4, word),
                sum
            );
        }
        const uint8_t packed_scale = block[106 + scale_block / 2];
        const int local_scale = 1 + 2 * static_cast<int>(
            scale_block % 2 == 0 ? (packed_scale & 0x0f) : (packed_scale >> 4)
        );
        return half_to_float(load_u16_le(block)) *
            static_cast<float>(local_scale * sum) *
            half_to_float(load_u16_le(input_block->bytes));
    }
};

struct Q5KQ81Dot {
    __device__ __forceinline__ float operator()(
        const uint8_t *weights,
        const Q81Block *input,
        int weight_block_index,
        int input_block_index,
        int quant_index
    ) const {
        const uint8_t *block =
            weights + static_cast<size_t>(weight_block_index) * kQ5KBlockBytes;
        const int scale_block = quant_index >> 2;
        const int quant_chunk = scale_block >> 1;
        const uint8_t *quants = block + 48 + quant_chunk * 32;
        const int low_shift = (scale_block & 1) * 4;
        const Q81Block *input_block =
            input + static_cast<size_t>(input_block_index) * 8 + scale_block;
        // The 32 quant bytes and the 32 high-bit bytes are 4-byte aligned.
        int sum = 0;
        int input_sum = 0;
#pragma unroll
        for (int word = 0; word < 8; ++word) {
            const uint32_t packed =
                static_cast<uint32_t>(get_int_b4(quants, word));
            const uint32_t low = (packed >> low_shift) & 0x0f0f0f0fu;
            const uint32_t high = (static_cast<uint32_t>(
                get_int_b4(block + 16, word)) >> scale_block) & 0x01010101u;
            const uint32_t w = low | (high << 4);
            const int input_word = get_int_b4(input_block->bytes + 4, word);
            sum = dp4a_i8(static_cast<int>(w), input_word, sum);
            input_sum = dp4a_i8(0x01010101, input_word, input_sum);
        }
        const int2 scale_min = decode_q4_k_scale_min(scale_block, block + 4);
        return (half_to_float(load_u16_le(block)) *
                    static_cast<float>(scale_min.x) * static_cast<float>(sum) -
                half_to_float(load_u16_le(block + 2)) *
                    static_cast<float>(scale_min.y) * static_cast<float>(input_sum)) *
            half_to_float(load_u16_le(input_block->bytes));
    }
};

struct Q3KQ81Dot {
    __device__ __forceinline__ float operator()(
        const uint8_t *weights,
        const Q81Block *input,
        int weight_block_index,
        int input_block_index,
        int quant_index
    ) const {
        const uint8_t *block =
            weights + static_cast<size_t>(weight_block_index) * kQ3KBlockBytes;
        const int scale_block = quant_index >> 2;
        const int half_index = scale_block >> 2;
        const int shift_group = scale_block & 3;
        const uint8_t *quants = block + 32 + half_index * 32;
        const int high_shift = half_index * 4 + shift_group;
        const int low_shift = 2 * shift_group;
        const Q81Block *input_block =
            input + static_cast<size_t>(input_block_index) * 8 + scale_block;
        int sum_low = 0;
        int sum_high = 0;
#pragma unroll
        for (int word = 0; word < 8; ++word) {
            const uint32_t packed =
                static_cast<uint32_t>(get_int_b1(quants, word));
            const uint32_t low = (packed >> low_shift) & 0x03030303u;
            const uint32_t high = (static_cast<uint32_t>(
                get_int_b1(block, word)) >> high_shift) & 0x01010101u;
            const uint32_t w = __vsub4(low | (high << 2), 0x04040404u);
            const int input_word = get_int_b4(input_block->bytes + 4, word);
            if (word < 4) {
                sum_low = dp4a_i8(static_cast<int>(w), input_word, sum_low);
            } else {
                sum_high = dp4a_i8(static_cast<int>(w), input_word, sum_high);
            }
        }
        const float d = half_to_float(load_u16_le(block + 108));
        const int scale_low =
            q3_k_scale(block + 96, half_index * 8 + shift_group * 2);
        const int scale_high =
            q3_k_scale(block + 96, half_index * 8 + shift_group * 2 + 1);
        return d *
            (static_cast<float>(scale_low * sum_low) +
             static_cast<float>(scale_high * sum_high)) *
            half_to_float(load_u16_le(input_block->bytes));
    }
};

__global__ void quantize_q8_1_rows_kernel(
    const float *input,
    int rows,
    int cols,
    Q81Block *output
) {
    const int block_index = static_cast<int>(blockIdx.x);
    const int row = static_cast<int>(blockIdx.y);
    const int lane = static_cast<int>(threadIdx.x);
    if (row >= rows || lane >= kQ81ElementsPerBlock) {
        return;
    }

    const int blocks_per_row = cols / kQ81ElementsPerBlock;
    const int input_index = row * cols + block_index * kQ81ElementsPerBlock + lane;
    const float value = input[input_index];

    float amax = fabsf(value);
    float sum = value;
    for (int offset = 16; offset > 0; offset >>= 1) {
        amax = fmaxf(amax, __shfl_xor_sync(0xffffffffu, amax, offset, 32));
        sum += __shfl_xor_sync(0xffffffffu, sum, offset, 32);
    }

    const float scale = amax == 0.0f ? 0.0f : amax / 127.0f;
    const float quantized = scale == 0.0f ? 0.0f : value / scale;
    const float clamped = fminf(fmaxf(roundf(quantized), -127.0f), 127.0f);

    Q81Block *row_output = output + row * blocks_per_row + block_index;
    row_output->bytes[4 + lane] = static_cast<uint8_t>(static_cast<int8_t>(clamped));
    if (lane == 0) {
        const uint16_t scale_bits = __half_as_ushort(__float2half_rn(scale));
        const uint16_t sum_bits = __half_as_ushort(__float2half_rn(sum));
        row_output->bytes[0] = static_cast<uint8_t>(scale_bits & 0xffu);
        row_output->bytes[1] = static_cast<uint8_t>((scale_bits >> 8) & 0xffu);
        row_output->bytes[2] = static_cast<uint8_t>(sum_bits & 0xffu);
        row_output->bytes[3] = static_cast<uint8_t>((sum_bits >> 8) & 0xffu);
    }
}

__device__ __forceinline__ void quantize_q8_1_warp_value(
    float value,
    Q81Block *output,
    int lane
) {
    float amax = fabsf(value);
    float sum = value;
    for (int offset = 16; offset > 0; offset >>= 1) {
        amax = fmaxf(amax, __shfl_xor_sync(0xffffffffu, amax, offset, 32));
        sum += __shfl_xor_sync(0xffffffffu, sum, offset, 32);
    }

    const float scale = amax == 0.0f ? 0.0f : amax / 127.0f;
    const float quantized = scale == 0.0f ? 0.0f : value / scale;
    const float clamped = fminf(fmaxf(roundf(quantized), -127.0f), 127.0f);

    output->bytes[4 + lane] = static_cast<uint8_t>(static_cast<int8_t>(clamped));
    if (lane == 0) {
        const uint16_t scale_bits = __half_as_ushort(__float2half_rn(scale));
        const uint16_t sum_bits = __half_as_ushort(__float2half_rn(sum));
        output->bytes[0] = static_cast<uint8_t>(scale_bits & 0xffu);
        output->bytes[1] = static_cast<uint8_t>((scale_bits >> 8) & 0xffu);
        output->bytes[2] = static_cast<uint8_t>(sum_bits & 0xffu);
        output->bytes[3] = static_cast<uint8_t>((sum_bits >> 8) & 0xffu);
    }
}

__device__ __forceinline__ void quantize_q8_1_shared_block(
    const float *input,
    Q81Block *output,
    int lane
) {
    quantize_q8_1_warp_value(input[lane], output, lane);
}

struct Q80Dequant {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int block_index,
        int lane
    ) const {
        const uint8_t *block = row_weights + block_index * kQ80BlockBytes;
        const float scale = half_to_float(load_u16_le(block));
        const int8_t quantized = reinterpret_cast<const int8_t *>(block + 2)[lane];
        return static_cast<float>(quantized) * scale;
    }
};

struct Mxfp4Dequant {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int block_index,
        int lane
    ) const {
        const uint8_t *block = row_weights + block_index * sizeof(Mxfp4Block);
        const float scale = decode_e8m0_to_fp32_half(block[0]);
        const uint8_t packed = block[1 + (lane & 15)];
        const uint8_t nibble = lane < 16 ? (packed & 0x0f) : ((packed >> 4) & 0x0f);
        return mxfp4_value(nibble) * scale * 0.5f;
    }
};

template <typename DequantFn>
__global__ void dequantize_row_to_f32_kernel(
    const uint8_t *weights,
    int rows,
    int cols,
    int row_stride,
    const int *decode_params,
    float *output,
    DequantFn dequant_fn
) {
    const int block_index = static_cast<int>(blockIdx.x);
    const int lane = static_cast<int>(threadIdx.x);
    const int col = block_index * kQ81ElementsPerBlock + lane;
    const int row_index = decode_params[2];
    if (lane >= kQ81ElementsPerBlock || col >= cols || row_index < 0 || row_index >= rows) {
        return;
    }

    const uint8_t *row_weights =
        weights + static_cast<size_t>(row_index) * static_cast<size_t>(row_stride);
    output[col] = dequant_fn(row_weights, block_index, lane);
}

__global__ void cast_f32_to_f16_kernel(
    const float *input,
    int element_count,
    __half *output
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + static_cast<int>(threadIdx.x);
    if (index >= element_count) {
        return;
    }
    output[index] = __float2half(input[index]);
}

__global__ void cast_f32_to_bf16_kernel(
    const float *input,
    int element_count,
    __nv_bfloat16 *output
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + static_cast<int>(threadIdx.x);
    if (index >= element_count) {
        return;
    }
    output[index] = __float2bfloat16(input[index]);
}

__global__ void cast_bf16_to_f32_kernel(
    const __nv_bfloat16 *input,
    int element_count,
    float *output
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + static_cast<int>(threadIdx.x);
    if (index >= element_count) {
        return;
    }
    output[index] = __bfloat162float(input[index]);
}

__global__ void gather_f16_row_to_f32_kernel(
    const __half *input,
    int rows,
    int cols,
    const int *decode_params,
    float *output
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + static_cast<int>(threadIdx.x);
    const int row_index = decode_params[2];
    if (index >= cols || row_index < 0 || row_index >= rows) {
        return;
    }
    output[index] = __half2float(input[static_cast<size_t>(row_index) * static_cast<size_t>(cols) + index]);
}

__global__ void gather_f32_by_indices_kernel(
    const float *input,
    int input_len,
    const int *indices,
    int index_count,
    float *output
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + static_cast<int>(threadIdx.x);
    if (index >= index_count) {
        return;
    }
    const int source_index = indices[index];
    if (source_index < 0 || source_index >= input_len) {
        output[index] = -INFINITY;
        return;
    }
    output[index] = input[source_index];
}

template <bool UseActiveMask>
struct Q80Q81DotImpl {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int block_index,
        const Q81Block *input
    ) const {
        const Q80Block *weight_block =
            reinterpret_cast<const Q80Block *>(row_weights + static_cast<size_t>(block_index) * sizeof(Q80Block));
        return dot_q8_0_q8_1_block(weight_block, input + block_index);
    }

    __device__ __forceinline__ float operator()(
        const uint8_t *weights,
        const Q81Block *input,
        int weight_block_index,
        int input_block_index,
        int quant_index
    ) const {
        const Q80Block *weight_block =
            reinterpret_cast<const Q80Block *>(weights) + weight_block_index;
        const Q81Block *input_block = input + input_block_index;
        const int *input_quants = reinterpret_cast<const int *>(input_block->bytes + 4) + quant_index;

        int packed_weights[kQ80Q81MmvqVdr];
        int packed_input[kQ80Q81MmvqVdr];
#pragma unroll
        for (int index = 0; index < kQ80Q81MmvqVdr; ++index) {
            packed_weights[index] = get_int_b2(weight_block->bytes + 2, quant_index + index);
            packed_input[index] = input_quants[index];
        }

        int sum = 0;
#pragma unroll
        for (int index = 0; index < kQ80Q81MmvqVdr; ++index) {
            sum = dp4a_i8(packed_weights[index], packed_input[index], sum);
        }

        float weight_scale = 0.0f;
        float input_scale = 0.0f;
        const int subgroup_lane = static_cast<int>(threadIdx.x) & 3;
        const int subgroup_base_lane = static_cast<int>(threadIdx.x) & ~3;
        const unsigned int subgroup_mask = UseActiveMask
            ? (__activemask() & (0x0fu << static_cast<unsigned int>(subgroup_base_lane)))
            : (0x0fu << static_cast<unsigned int>(subgroup_base_lane));
        if (subgroup_lane == 0) {
            weight_scale = half_to_float(load_u16_le(weight_block->bytes));
            input_scale = half_to_float(load_u16_le(input_block->bytes));
        }
        weight_scale = __shfl_sync(subgroup_mask, weight_scale, subgroup_base_lane, kWarpSize);
        input_scale = __shfl_sync(subgroup_mask, input_scale, subgroup_base_lane, kWarpSize);
        return weight_scale * input_scale * static_cast<float>(sum);
    }
};

using Q80Q81Dot = Q80Q81DotImpl<true>;
using Q80Q81DotFixedMask = Q80Q81DotImpl<false>;

struct Mxfp4Q81Dot {
    __device__ __forceinline__ float operator()(
        const uint8_t *row_weights,
        int block_index,
        const Q81Block *input
    ) const {
        const Mxfp4Block *weight_block =
            reinterpret_cast<const Mxfp4Block *>(row_weights + static_cast<size_t>(block_index) * sizeof(Mxfp4Block));
        return dot_mxfp4_q8_1_block(weight_block, input + block_index);
    }

    __device__ __forceinline__ float operator()(
        const uint8_t *weights,
        const Q81Block *input,
        int weight_block_index,
        int input_block_index,
        int quant_index
    ) const {
        const Mxfp4Block *weight_block =
            reinterpret_cast<const Mxfp4Block *>(weights) + weight_block_index;
        const Q81Block *input_block = input + input_block_index;
        const int *input_quants = reinterpret_cast<const int *>(input_block->bytes + 4) + quant_index;

        int sum = 0;
#pragma unroll
        for (int lane_group = 0; lane_group < kMxfp4Q81MmvqVdr; ++lane_group) {
            const int packed = get_int_b1(weight_block->bytes + 1, quant_index + lane_group);
            const int2 dequantized = get_int_from_table_16(packed, kMxfp4IntTable);
            sum = dp4a_i8(dequantized.x, input_quants[lane_group + 0], sum);
            sum = dp4a_i8(dequantized.y, input_quants[lane_group + 4], sum);
        }

        return decode_e8m0_to_fp32_half(weight_block->bytes[0]) * 0.5f *
            half_to_float(load_u16_le(input_block->bytes)) *
            static_cast<float>(sum);
    }
};

struct Q4KQ81Dot {
    __device__ __forceinline__ float operator()(
        const uint8_t *weights,
        const Q81Block *input,
        int weight_block_index,
        int input_block_index,
        int quant_index
    ) const {
        const uint8_t *weight_block =
            weights + static_cast<size_t>(weight_block_index) * kQ4KBlockBytes;
        const int input_block_offset = input_block_index * 8;
        const int bq8_offset = 2 * ((quant_index >> 1) / 4);
        const int q4_offset = 4 * ((quant_index >> 1) % 4);
        const int *q4 = reinterpret_cast<const int *>(weight_block + 16 + 16 * bq8_offset + q4_offset);
        const uint16_t *scale_words = reinterpret_cast<const uint16_t *>(weight_block + 4);
        uint16_t aux[2];
        const int scale_group = bq8_offset / 2;
        if (scale_group < 2) {
            aux[0] = scale_words[scale_group + 0] & 0x3f3f;
            aux[1] = scale_words[scale_group + 2] & 0x3f3f;
        } else {
            aux[0] = ((scale_words[scale_group + 2] >> 0) & 0x0f0f) |
                ((scale_words[scale_group - 2] & 0xc0c0) >> 2);
            aux[1] = ((scale_words[scale_group + 2] >> 4) & 0x0f0f) |
                ((scale_words[scale_group - 0] & 0xc0c0) >> 2);
        }
        const uint8_t *scales = reinterpret_cast<const uint8_t *>(aux);
        const uint8_t *mins = scales + 2;
        const int q8_index = (quant_index >> 1) % 4;

        float sumf_d = 0.0f;
        float sumf_m = 0.0f;
#pragma unroll
        for (int group_index = 0; group_index < 2; ++group_index) {
            const Q81Block *input_block = input + input_block_offset + bq8_offset + group_index;
            const float input_scale = half_to_float(load_u16_le(input_block->bytes));
            const int q8_low = get_int_b4(input_block->bytes + 4, q8_index + 0);
            const int q8_high = get_int_b4(input_block->bytes + 4, q8_index + 4);
            const int q4_low = (q4[0] >> (4 * group_index)) & 0x0f0f0f0f;
            const int q4_high = (q4[4] >> (4 * group_index)) & 0x0f0f0f0f;
            const int dot = dp4a_i8(q4_high, q8_high, dp4a_i8(q4_low, q8_low, 0));
            const int q8_sum = dp4a_i8(0x01010101, q8_high, dp4a_i8(0x01010101, q8_low, 0));
            sumf_d += input_scale * (static_cast<float>(dot) * static_cast<float>(scales[group_index]));
            sumf_m += input_scale * (static_cast<float>(q8_sum) * static_cast<float>(mins[group_index]));
        }
        const float base_scale = half_to_float(load_u16_le(weight_block));
        const float base_min = half_to_float(load_u16_le(weight_block + 2));
        return base_scale * sumf_d - base_min * sumf_m;
    }
};

struct Q6KQ81Dot {
    __device__ __forceinline__ float operator()(
        const uint8_t *weights,
        const Q81Block *input,
        int weight_block_index,
        int input_block_index,
        int quant_index
    ) const {
        const uint8_t *weight_block =
            weights + static_cast<size_t>(weight_block_index) * kQ6KBlockBytes;
        const int input_block_offset = input_block_index * 8;
        const int bq8_offset = 4 * (quant_index / 16) + ((quant_index % 16) / 8);
        const int scale_offset = 8 * (quant_index / 16) + ((quant_index % 16) / 4);
        const int vh_shift = 2 * ((quant_index % 16) / 8);
        const int vl = get_int_b2(weight_block, quant_index);
        const int vh = get_int_b2(weight_block + 128, 8 * (quant_index / 16) + (quant_index % 8)) >> vh_shift;
        const int8_t *scales = reinterpret_cast<const int8_t *>(weight_block + 192) + scale_offset;
        const float base_scale = half_to_float(load_u16_le(weight_block + 208));

        float sumf = 0.0f;
#pragma unroll
        for (int group_index = 0; group_index < 2; ++group_index) {
            const int scale = static_cast<int>(scales[4 * group_index]);
            const int q6_low = (vl >> (4 * group_index)) & 0x0f0f0f0f;
            const int q6_high = ((vh >> (4 * group_index)) << 4) & 0x30303030;
            const int q6 = __vsubss4(q6_low | q6_high, 0x20202020);
            const Q81Block *input_block = input + input_block_offset + bq8_offset + 2 * group_index;
            const int q8 = get_int_b4(input_block->bytes + 4, quant_index % 8);
            const float input_scale = half_to_float(load_u16_le(input_block->bytes));
            sumf += input_scale * static_cast<float>(dp4a_i8(q6, q8, 0) * scale);
        }
        return base_scale * sumf;
    }
};

template <typename DotFn, int Vdr, int Qi>
__launch_bounds__(kMmvqWarps * kWarpSize, 1)
__global__ void quantized_matvec_q8_1_mmvq_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int block_count,
    const Q81Block *input,
    const float *bias,
    float *output,
    DotFn dot_fn
) {
    const int row = static_cast<int>(blockIdx.x);
    if (row >= rows) {
        return;
    }

    constexpr int rows_per_block = 1;
    constexpr int blocks_per_iter = Vdr * kMmvqWarps * kWarpSize / Qi;

    const int tid = kWarpSize * static_cast<int>(threadIdx.y) + static_cast<int>(threadIdx.x);
    const uint8_t *row_weights = weights + static_cast<size_t>(row) * static_cast<size_t>(row_stride);

    float sum = 0.0f;
    for (int block_index = tid / (Qi / Vdr); block_index < block_count; block_index += blocks_per_iter) {
        const int quant_index = Vdr * (tid % (Qi / Vdr));
        sum += dot_fn(row_weights, input, block_index, block_index, quant_index);
    }

    __shared__ float partials[kMmvqWarps - 1 > 0 ? kMmvqWarps - 1 : 1][rows_per_block][kWarpSize];
    if (threadIdx.y > 0) {
        partials[threadIdx.y - 1][0][threadIdx.x] = sum;
    }
    __syncthreads();

    if (threadIdx.y > 0) {
        return;
    }

#pragma unroll
    for (int warp_index = 0; warp_index < kMmvqWarps - 1; ++warp_index) {
        sum += partials[warp_index][0][threadIdx.x];
    }

    sum = warp_reduce_sum(sum);
    if (threadIdx.x == 0) {
        output[row] = sum + (bias != nullptr ? bias[row] : 0.0f);
    }
}

template <typename DotFn, int Vdr, int Qi, int RowsPerBlock, int WarpsPerRow>
__launch_bounds__(RowsPerBlock * WarpsPerRow * kWarpSize, 1)
__global__ void quantized_matvec_q8_1_grouped_mmvq_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int block_count,
    const Q81Block *input,
    const float *bias,
    float *output,
    DotFn dot_fn
) {
    const int warp_index = static_cast<int>(threadIdx.y);
    const int row_in_block = warp_index / WarpsPerRow;
    const int warp_in_row = warp_index % WarpsPerRow;
    const int row = static_cast<int>(blockIdx.x) * RowsPerBlock + row_in_block;
    if (row >= rows) {
        return;
    }

    constexpr int blocks_per_iter = Vdr * WarpsPerRow * kWarpSize / Qi;
    const int tid = kWarpSize * warp_in_row + static_cast<int>(threadIdx.x);
    const uint8_t *row_weights = weights + static_cast<size_t>(row) * static_cast<size_t>(row_stride);

    float sum = 0.0f;
    for (int block_index = tid / (Qi / Vdr); block_index < block_count; block_index += blocks_per_iter) {
        const int quant_index = Vdr * (tid % (Qi / Vdr));
        sum += dot_fn(row_weights, input, block_index, block_index, quant_index);
    }

    __shared__ float partials[RowsPerBlock][WarpsPerRow - 1 > 0 ? WarpsPerRow - 1 : 1][kWarpSize];
    if (warp_in_row > 0) {
        partials[row_in_block][warp_in_row - 1][threadIdx.x] = sum;
    }
    __syncthreads();

    if (warp_in_row > 0) {
        return;
    }

#pragma unroll
    for (int other_warp = 0; other_warp < WarpsPerRow - 1; ++other_warp) {
        sum += partials[row_in_block][other_warp][threadIdx.x];
    }

    sum = warp_reduce_sum(sum);
    if (threadIdx.x == 0) {
        output[row] = sum + (bias != nullptr ? bias[row] : 0.0f);
    }
}

template <typename DotFn, int Vdr, int Qi, int RowsPerBlock, int WarpsPerRow>
__launch_bounds__(RowsPerBlock * WarpsPerRow * kWarpSize, 1)
__global__ void quantized_matvec_q8_1_grouped_mmvq_shared_input_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int weight_block_count,
    int input_block_count,
    const Q81Block *input,
    const float *bias,
    float *output,
    DotFn dot_fn
) {
    extern __shared__ unsigned char shared_storage[];
    Q81Block *shared_input = reinterpret_cast<Q81Block *>(shared_storage);

    const int linear_tid = kWarpSize * static_cast<int>(threadIdx.y) + static_cast<int>(threadIdx.x);
    const int thread_count = RowsPerBlock * WarpsPerRow * kWarpSize;
    for (int input_index = linear_tid; input_index < input_block_count; input_index += thread_count) {
        shared_input[input_index] = input[input_index];
    }
    __syncthreads();

    const int warp_index = static_cast<int>(threadIdx.y);
    const int row_in_block = warp_index / WarpsPerRow;
    const int warp_in_row = warp_index % WarpsPerRow;
    const int row = static_cast<int>(blockIdx.x) * RowsPerBlock + row_in_block;
    if (row >= rows) {
        return;
    }

    constexpr int blocks_per_iter = Vdr * WarpsPerRow * kWarpSize / Qi;
    const int tid = kWarpSize * warp_in_row + static_cast<int>(threadIdx.x);
    const uint8_t *row_weights = weights + static_cast<size_t>(row) * static_cast<size_t>(row_stride);

    float sum = 0.0f;
    for (int weight_block_index = tid / (Qi / Vdr);
         weight_block_index < weight_block_count;
         weight_block_index += blocks_per_iter) {
        const int quant_index = Vdr * (tid % (Qi / Vdr));
        sum += dot_fn(row_weights, shared_input, weight_block_index, weight_block_index, quant_index);
    }

    __shared__ float partials[RowsPerBlock][WarpsPerRow - 1 > 0 ? WarpsPerRow - 1 : 1][kWarpSize];
    if (warp_in_row > 0) {
        partials[row_in_block][warp_in_row - 1][threadIdx.x] = sum;
    }
    __syncthreads();

    if (warp_in_row > 0) {
        return;
    }

#pragma unroll
    for (int other_warp = 0; other_warp < WarpsPerRow - 1; ++other_warp) {
        sum += partials[row_in_block][other_warp][threadIdx.x];
    }

    sum = warp_reduce_sum(sum);
    if (threadIdx.x == 0) {
        output[row] = sum + (bias != nullptr ? bias[row] : 0.0f);
    }
}

__device__ __forceinline__ unsigned long long pack_argmax_pair(float value, int index);
__device__ __forceinline__ void atomic_update_argmax_pair(
    unsigned long long *state,
    float value,
    int index
);

template <typename DotFn, int Vdr, int Qi, int RowsPerBlock, int WarpsPerRow>
__launch_bounds__(RowsPerBlock * WarpsPerRow * kWarpSize, 1)
__global__ void quantized_matvec_q8_1_grouped_mmvq_argmax_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int block_count,
    const Q81Block *input,
    const float *bias,
    unsigned long long *argmax_state,
    DotFn dot_fn
) {
    const int warp_index = static_cast<int>(threadIdx.y);
    const int row_in_block = warp_index / WarpsPerRow;
    const int warp_in_row = warp_index % WarpsPerRow;
    const int row = static_cast<int>(blockIdx.x) * RowsPerBlock + row_in_block;
    const bool valid_row = row < rows;

    constexpr int blocks_per_iter = Vdr * WarpsPerRow * kWarpSize / Qi;
    const int tid = kWarpSize * warp_in_row + static_cast<int>(threadIdx.x);
    const uint8_t *row_weights = valid_row
        ? weights + static_cast<size_t>(row) * static_cast<size_t>(row_stride)
        : weights;

    float sum = 0.0f;
    if (valid_row) {
        for (int block_index = tid / (Qi / Vdr); block_index < block_count; block_index += blocks_per_iter) {
            const int quant_index = Vdr * (tid % (Qi / Vdr));
            sum += dot_fn(row_weights, input, block_index, block_index, quant_index);
        }
    }

    __shared__ float partials[RowsPerBlock][WarpsPerRow - 1 > 0 ? WarpsPerRow - 1 : 1][kWarpSize];
    if (warp_in_row > 0) {
        partials[row_in_block][warp_in_row - 1][threadIdx.x] = sum;
    }
    __shared__ float row_values[RowsPerBlock];
    __shared__ int row_indices[RowsPerBlock];
    __syncthreads();

    if (warp_in_row == 0) {
#pragma unroll
        for (int other_warp = 0; other_warp < WarpsPerRow - 1; ++other_warp) {
            sum += partials[row_in_block][other_warp][threadIdx.x];
        }

        sum = warp_reduce_sum(sum);
        if (threadIdx.x == 0) {
            row_values[row_in_block] = valid_row
                ? sum + (bias != nullptr ? bias[row] : 0.0f)
                : -FLT_MAX;
            row_indices[row_in_block] = valid_row ? row : -1;
        }
    }
    __syncthreads();

    if (threadIdx.y == 0 && threadIdx.x == 0) {
        float best_value = -FLT_MAX;
        int best_index = -1;
#pragma unroll
        for (int row_index = 0; row_index < RowsPerBlock; ++row_index) {
            const float candidate_value = row_values[row_index];
            const int candidate_index = row_indices[row_index];
            if (candidate_index >= 0 &&
                (candidate_value > best_value ||
                 (candidate_value == best_value && candidate_index < best_index))) {
                best_value = candidate_value;
                best_index = candidate_index;
            }
        }
        if (best_index >= 0) {
            atomic_update_argmax_pair(argmax_state, best_value, best_index);
        }
    }
}

__device__ __forceinline__ unsigned long long pack_argmax_pair(float value, int index) {
    const uint32_t bits = __float_as_uint(value);
    const uint32_t ordered = (bits & 0x80000000u) != 0u ? ~bits : (bits | 0x80000000u);
    return (static_cast<unsigned long long>(ordered) << 32) |
        static_cast<unsigned long long>(0xffffffffu - static_cast<uint32_t>(index));
}

__device__ __forceinline__ void atomic_update_argmax_pair(
    unsigned long long *state,
    float value,
    int index
) {
    const unsigned long long packed = pack_argmax_pair(value, index);
    if (packed <= *state) {
        return;
    }
    atomicMax(state, packed);
}

template <typename DotFn, int Vdr, int Qi>
__launch_bounds__(kMmvqWarps * kWarpSize, 1)
__global__ void quantized_matvec_q8_1_mmvq_argmax_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int block_count,
    const Q81Block *input,
    const float *bias,
    unsigned long long *argmax_state,
    DotFn dot_fn
) {
    const int row = static_cast<int>(blockIdx.x);
    if (row >= rows) {
        return;
    }

    constexpr int blocks_per_iter = Vdr * kMmvqWarps * kWarpSize / Qi;
    const int tid = kWarpSize * static_cast<int>(threadIdx.y) + static_cast<int>(threadIdx.x);
    const uint8_t *row_weights = weights + static_cast<size_t>(row) * static_cast<size_t>(row_stride);

    float sum = 0.0f;
    for (int block_index = tid / (Qi / Vdr); block_index < block_count; block_index += blocks_per_iter) {
        const int quant_index = Vdr * (tid % (Qi / Vdr));
        sum += dot_fn(row_weights, input, block_index, block_index, quant_index);
    }

    __shared__ float partials[kMmvqWarps - 1 > 0 ? kMmvqWarps - 1 : 1][kWarpSize];
    if (threadIdx.y > 0) {
        partials[threadIdx.y - 1][threadIdx.x] = sum;
    }
    __syncthreads();

    if (threadIdx.y > 0) {
        return;
    }

#pragma unroll
    for (int warp_index = 0; warp_index < kMmvqWarps - 1; ++warp_index) {
        sum += partials[warp_index][threadIdx.x];
    }

    sum = warp_reduce_sum(sum);
    if (threadIdx.x == 0) {
        const float value = sum + (bias != nullptr ? bias[row] : 0.0f);
        atomic_update_argmax_pair(argmax_state, value, row);
    }
}

template <typename DotFn, int RowsPerBlock>
__launch_bounds__(RowsPerBlock * kWarpSize, 1)
__global__ void quantized_matvec_q8_1_shared_input_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int block_count,
    const Q81Block *input,
    const float *bias,
    float *output,
    DotFn dot_fn
) {
    extern __shared__ unsigned char shared_storage[];
    Q81Block *shared_input = reinterpret_cast<Q81Block *>(shared_storage);

    const int linear_tid = static_cast<int>(threadIdx.y) * kWarpSize + static_cast<int>(threadIdx.x);
    const int thread_count = RowsPerBlock * kWarpSize;
    for (int block_index = linear_tid; block_index < block_count; block_index += thread_count) {
        shared_input[block_index] = input[block_index];
    }
    __syncthreads();

    const int row = static_cast<int>(blockIdx.x) * RowsPerBlock + static_cast<int>(threadIdx.y);
    if (row >= rows) {
        return;
    }

    const uint8_t *row_weights = weights + static_cast<size_t>(row) * static_cast<size_t>(row_stride);
    float sum = 0.0f;
    for (int block_index = static_cast<int>(threadIdx.x); block_index < block_count; block_index += kWarpSize) {
        sum += dot_fn(row_weights, block_index, shared_input);
    }

    sum = warp_reduce_sum(sum);
    if (threadIdx.x == 0) {
        output[row] = sum + (bias != nullptr ? bias[row] : 0.0f);
    }
}

template <typename DotFn, int RowsPerBlock>
__launch_bounds__(RowsPerBlock * kWarpSize, 1)
__global__ void quantized_matvec_q8_1_shared_input_argmax_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int block_count,
    const Q81Block *input,
    const float *bias,
    unsigned long long *argmax_state,
    DotFn dot_fn
) {
    extern __shared__ unsigned char shared_storage[];
    Q81Block *shared_input = reinterpret_cast<Q81Block *>(shared_storage);

    const int linear_tid = static_cast<int>(threadIdx.y) * kWarpSize + static_cast<int>(threadIdx.x);
    const int thread_count = RowsPerBlock * kWarpSize;
    for (int block_index = linear_tid; block_index < block_count; block_index += thread_count) {
        shared_input[block_index] = input[block_index];
    }
    __syncthreads();

    const int row = static_cast<int>(blockIdx.x) * RowsPerBlock + static_cast<int>(threadIdx.y);
    __shared__ float row_values[RowsPerBlock];
    __shared__ int row_indices[RowsPerBlock];
    if (threadIdx.x == 0) {
        row_values[threadIdx.y] = -FLT_MAX;
        row_indices[threadIdx.y] = -1;
    }
    __syncthreads();

    if (row < rows) {
        const uint8_t *row_weights =
            weights + static_cast<size_t>(row) * static_cast<size_t>(row_stride);
        float sum = 0.0f;
        for (int block_index = static_cast<int>(threadIdx.x); block_index < block_count; block_index += kWarpSize) {
            sum += dot_fn(row_weights, block_index, shared_input);
        }

        sum = warp_reduce_sum(sum);
        if (threadIdx.x == 0) {
            row_values[threadIdx.y] = sum + (bias != nullptr ? bias[row] : 0.0f);
            row_indices[threadIdx.y] = row;
        }
    }
    __syncthreads();

    if (threadIdx.x == 0 && threadIdx.y == 0) {
        float best_value = -FLT_MAX;
        int best_index = -1;
#pragma unroll
        for (int row_index = 0; row_index < RowsPerBlock; ++row_index) {
            const float candidate_value = row_values[row_index];
            const int candidate_index = row_indices[row_index];
            if (candidate_index >= 0 &&
                (candidate_value > best_value ||
                 (candidate_value == best_value && candidate_index < best_index))) {
                best_value = candidate_value;
                best_index = candidate_index;
            }
        }
        if (best_index >= 0) {
            atomic_update_argmax_pair(argmax_state, best_value, best_index);
        }
    }
}

template <typename DotFn>
static void launch_quantized_matvec_q8_1_regular(
    const uint8_t *weights,
    int rows,
    int cols,
    int row_stride,
    const Q81Block *input_q8_1,
    const float *bias,
    float *output,
    cudaStream_t stream,
    DotFn dot_fn
) {
    const int block_count = cols / kQ81ElementsPerBlock;
    constexpr int rows_per_block = 8;
    const dim3 grid_dims((rows + rows_per_block - 1) / rows_per_block, 1, 1);
    const dim3 block_dims(kWarpSize, rows_per_block, 1);
    const size_t shared_bytes = static_cast<size_t>(block_count) * sizeof(Q81Block);
    quantized_matvec_q8_1_shared_input_kernel<DotFn, rows_per_block><<<
        grid_dims,
        block_dims,
        shared_bytes,
        stream
    >>>(
        weights,
        row_stride,
        rows,
        block_count,
        input_q8_1,
        bias,
        output,
        dot_fn
    );
}

template <typename DotFn>
static void launch_quantized_matvec_q8_1_argmax_regular(
    const uint8_t *weights,
    int rows,
    int cols,
    int row_stride,
    const Q81Block *input_q8_1,
    const float *bias,
    unsigned long long *argmax_state,
    cudaStream_t stream,
    DotFn dot_fn
) {
    const int block_count = cols / kQ81ElementsPerBlock;
    constexpr int rows_per_block = 8;
    const dim3 grid_dims((rows + rows_per_block - 1) / rows_per_block, 1, 1);
    const dim3 block_dims(kWarpSize, rows_per_block, 1);
    const size_t shared_bytes = static_cast<size_t>(block_count) * sizeof(Q81Block);
    quantized_matvec_q8_1_shared_input_argmax_kernel<DotFn, rows_per_block><<<
        grid_dims,
        block_dims,
        shared_bytes,
        stream
    >>>(
        weights,
        row_stride,
        rows,
        block_count,
        input_q8_1,
        bias,
        argmax_state,
        dot_fn
    );
}

template <typename DotFn, int Vdr, int Qi>
static void launch_quantized_matvec_q8_1_argmax_mmvq(
    const uint8_t *weights,
    int rows,
    int block_count,
    int row_stride,
    const Q81Block *input_q8_1,
    const float *bias,
    unsigned long long *argmax_state,
    cudaStream_t stream,
    DotFn dot_fn
) {
    const dim3 grid_dims(rows, 1, 1);
    const dim3 block_dims(kWarpSize, kMmvqWarps, 1);
    quantized_matvec_q8_1_mmvq_argmax_kernel<DotFn, Vdr, Qi><<<grid_dims, block_dims, 0, stream>>>(
        weights,
        row_stride,
        rows,
        block_count,
        input_q8_1,
        bias,
        argmax_state,
        dot_fn
    );
}

template <typename DotFn, int Vdr, int Qi, int RowsPerBlock, int WarpsPerRow>
static void launch_quantized_matvec_q8_1_grouped_argmax_mmvq(
    const uint8_t *weights,
    int rows,
    int block_count,
    int row_stride,
    const Q81Block *input_q8_1,
    const float *bias,
    unsigned long long *argmax_state,
    cudaStream_t stream,
    DotFn dot_fn
) {
    const dim3 grid_dims((rows + RowsPerBlock - 1) / RowsPerBlock, 1, 1);
    const dim3 block_dims(kWarpSize, RowsPerBlock * WarpsPerRow, 1);
    quantized_matvec_q8_1_grouped_mmvq_argmax_kernel<
        DotFn,
        Vdr,
        Qi,
        RowsPerBlock,
        WarpsPerRow><<<grid_dims, block_dims, 0, stream>>>(
        weights,
        row_stride,
        rows,
        block_count,
        input_q8_1,
        bias,
        argmax_state,
        dot_fn
    );
}

__global__ void argmax_f32_kernel(
    const float *input,
    int32_t *output,
    int row_count,
    int column_count
) {
    const int row = static_cast<int>(blockIdx.x);
    if (row >= row_count) {
        return;
    }

    const float *row_input = input + static_cast<size_t>(row) * static_cast<size_t>(column_count);
    float max_value = -FLT_MAX;
    int max_index = -1;

    for (int column = static_cast<int>(threadIdx.x); column < column_count; column += blockDim.x) {
        const float value = row_input[column];
        if (value > max_value) {
            max_value = value;
            max_index = column;
        }
    }

#pragma unroll
    for (int offset = kWarpSize / 2; offset > 0; offset >>= 1) {
        const float candidate_value = __shfl_xor_sync(0xffffffffu, max_value, offset, kWarpSize);
        const int candidate_index = __shfl_xor_sync(0xffffffffu, max_index, offset, kWarpSize);
        if (candidate_value > max_value) {
            max_value = candidate_value;
            max_index = candidate_index;
        }
    }

    const int warp_id = static_cast<int>(threadIdx.x) / kWarpSize;
    const int lane_id = static_cast<int>(threadIdx.x) % kWarpSize;
    const int warp_count = blockDim.x / kWarpSize;

    if (warp_count > 1) {
        __shared__ float shared_values[kMaxWarpsPerBlock];
        __shared__ int shared_indices[kMaxWarpsPerBlock];
        if (lane_id == 0) {
            shared_values[warp_id] = max_value;
            shared_indices[warp_id] = max_index;
        }
        __syncthreads();

        if (warp_id == 0) {
            if (lane_id < warp_count) {
                max_value = shared_values[lane_id];
                max_index = shared_indices[lane_id];
            }
#pragma unroll
            for (int offset = kWarpSize / 2; offset > 0; offset >>= 1) {
                const float candidate_value = __shfl_xor_sync(0xffffffffu, max_value, offset, kWarpSize);
                const int candidate_index = __shfl_xor_sync(0xffffffffu, max_index, offset, kWarpSize);
                if (candidate_value > max_value) {
                    max_value = candidate_value;
                    max_index = candidate_index;
                }
            }
        }
    }

    if (warp_id == 0 && lane_id == 0) {
        output[row] = max_index;
    }
}

__device__ __forceinline__ bool top_k_candidate_better(
    float candidate_value,
    int candidate_index,
    float current_value,
    int current_index
) {
    return candidate_value > current_value ||
        (candidate_value == current_value &&
            candidate_index >= 0 &&
            (current_index < 0 || candidate_index < current_index));
}

__global__ void top_k_f32_kernel(
    const float *input,
    int32_t *selected_indices,
    float *selected_values,
    int row_count,
    int column_count,
    int top_k
) {
    const int row = static_cast<int>(blockIdx.x);
    if (row >= row_count) {
        return;
    }

    top_k = min(top_k, min(column_count, kLogitsMaxSelected));
    const float *row_input = input + static_cast<size_t>(row) * static_cast<size_t>(column_count);
    __shared__ float shared_values[kMaxWarpsPerBlock];
    __shared__ int shared_indices[kMaxWarpsPerBlock];
    __shared__ float row_top_values[kLogitsMaxSelected];
    __shared__ int row_top_indices[kLogitsMaxSelected];

    if (top_k <= kLogitsFastSharedTopK) {
        using TopKBlockRadixSort =
            cub::BlockRadixSort<float, kLogitsTopKBlockSize, kLogitsTopKItemsPerThread, int>;
        __shared__ typename TopKBlockRadixSort::TempStorage sort_storage;
        __shared__ float tile_top_values[kLogitsFastSharedTopK];
        __shared__ int tile_top_indices[kLogitsFastSharedTopK];
        __shared__ float merged_top_values[kLogitsMaxSelected];
        __shared__ int merged_top_indices[kLogitsMaxSelected];
        if (threadIdx.x == 0) {
            for (int slot = 0; slot < top_k; ++slot) {
                row_top_values[slot] = -INFINITY;
                row_top_indices[slot] = -1;
            }
        }
        __syncthreads();

        for (int tile_base = 0; tile_base < column_count; tile_base += kLogitsTopKTileSize) {
            float thread_keys[kLogitsTopKItemsPerThread];
            int thread_values[kLogitsTopKItemsPerThread];
#pragma unroll
            for (int item = 0; item < kLogitsTopKItemsPerThread; ++item) {
                const int column =
                    tile_base + static_cast<int>(threadIdx.x) * kLogitsTopKItemsPerThread + item;
                if (column < column_count) {
                    thread_keys[item] = row_input[column];
                    thread_values[item] = column;
                } else {
                    thread_keys[item] = -INFINITY;
                    thread_values[item] = column_count;
                }
            }

            TopKBlockRadixSort(sort_storage).SortDescending(thread_keys, thread_values);
            __syncthreads();

#pragma unroll
            for (int item = 0; item < kLogitsTopKItemsPerThread; ++item) {
                const int rank =
                    static_cast<int>(threadIdx.x) * kLogitsTopKItemsPerThread + item;
                if (rank < top_k) {
                    tile_top_values[rank] = thread_keys[item];
                    tile_top_indices[rank] = thread_values[item];
                }
            }
            __syncthreads();

            if (threadIdx.x == 0) {
                int row_slot = 0;
                int tile_slot = 0;
                for (int merged_slot = 0; merged_slot < top_k; ++merged_slot) {
                    float row_value = -INFINITY;
                    int row_index = -1;
                    if (row_slot < top_k) {
                        row_value = row_top_values[row_slot];
                        row_index = row_top_indices[row_slot];
                    }

                    float tile_value = -INFINITY;
                    int tile_index = -1;
                    if (tile_slot < top_k) {
                        const int candidate_index = tile_top_indices[tile_slot];
                        if (candidate_index >= 0 && candidate_index < column_count) {
                            tile_value = tile_top_values[tile_slot];
                            tile_index = candidate_index;
                        }
                    }

                    if (top_k_candidate_better(tile_value, tile_index, row_value, row_index)) {
                        merged_top_values[merged_slot] = tile_value;
                        merged_top_indices[merged_slot] = tile_index;
                        ++tile_slot;
                    } else {
                        merged_top_values[merged_slot] = row_value;
                        merged_top_indices[merged_slot] = row_index;
                        ++row_slot;
                    }
                }

                for (int slot = 0; slot < top_k; ++slot) {
                    row_top_values[slot] = merged_top_values[slot];
                    row_top_indices[slot] = merged_top_indices[slot];
                }
            }
            __syncthreads();
        }

        const size_t row_offset = static_cast<size_t>(row) * static_cast<size_t>(top_k);
        for (int slot = static_cast<int>(threadIdx.x); slot < top_k; slot += blockDim.x) {
            selected_indices[row_offset + static_cast<size_t>(slot)] = row_top_indices[slot];
            selected_values[row_offset + static_cast<size_t>(slot)] = row_top_values[slot];
        }
        return;
    }

    const int lane_id = static_cast<int>(threadIdx.x) % kWarpSize;
    const int warp_id = static_cast<int>(threadIdx.x) / kWarpSize;
    const int warp_count = blockDim.x / kWarpSize;

    for (int slot = 0; slot < top_k; ++slot) {
        float max_value = -FLT_MAX;
        int max_index = -1;

        for (int column = static_cast<int>(threadIdx.x); column < column_count; column += blockDim.x) {
            bool already_selected = false;
#pragma unroll 4
            for (int previous = 0; previous < slot; ++previous) {
                if (column == row_top_indices[previous]) {
                    already_selected = true;
                    break;
                }
            }
            if (already_selected) {
                continue;
            }

            const float value = row_input[column];
            if (top_k_candidate_better(value, column, max_value, max_index)) {
                max_value = value;
                max_index = column;
            }
        }

#pragma unroll
        for (int offset = kWarpSize / 2; offset > 0; offset >>= 1) {
            const float candidate_value =
                __shfl_xor_sync(0xffffffffu, max_value, offset, kWarpSize);
            const int candidate_index =
                __shfl_xor_sync(0xffffffffu, max_index, offset, kWarpSize);
            if (top_k_candidate_better(candidate_value, candidate_index, max_value, max_index)) {
                max_value = candidate_value;
                max_index = candidate_index;
            }
        }

        if (lane_id == 0) {
            shared_values[warp_id] = max_value;
            shared_indices[warp_id] = max_index;
        }
        __syncthreads();

        if (warp_id == 0) {
            if (lane_id < warp_count) {
                max_value = shared_values[lane_id];
                max_index = shared_indices[lane_id];
            } else {
                max_value = -FLT_MAX;
                max_index = -1;
            }
#pragma unroll
            for (int offset = kWarpSize / 2; offset > 0; offset >>= 1) {
                const float candidate_value =
                    __shfl_xor_sync(0xffffffffu, max_value, offset, kWarpSize);
                const int candidate_index =
                    __shfl_xor_sync(0xffffffffu, max_index, offset, kWarpSize);
                if (top_k_candidate_better(candidate_value, candidate_index, max_value, max_index)) {
                    max_value = candidate_value;
                    max_index = candidate_index;
                }
            }
            if (lane_id == 0) {
                row_top_values[slot] = max_value;
                row_top_indices[slot] = max_index;
            }
        }
        __syncthreads();
    }

    const size_t row_offset = static_cast<size_t>(row) * static_cast<size_t>(top_k);
    for (int slot = static_cast<int>(threadIdx.x); slot < top_k; slot += blockDim.x) {
        selected_indices[row_offset + static_cast<size_t>(slot)] = row_top_indices[slot];
        selected_values[row_offset + static_cast<size_t>(slot)] = row_top_values[slot];
    }
}

__global__ void top_k_f32_one_row_partial_kernel(
    const float *input,
    int32_t *partial_indices,
    float *partial_values,
    int column_count,
    int top_k
) {
    top_k = min(top_k, min(column_count, kLogitsMaxSelected));
    if (top_k <= 0) {
        return;
    }

    const int block_count = max(gridDim.x, 1);
    const float *row_input = input;
    __shared__ float row_top_values[kLogitsMaxSelected];
    __shared__ int row_top_indices[kLogitsMaxSelected];

    using TopKBlockRadixSort =
        cub::BlockRadixSort<float, kLogitsTopKBlockSize, kLogitsTopKItemsPerThread, int>;
    __shared__ typename TopKBlockRadixSort::TempStorage sort_storage;
    __shared__ float tile_top_values[kLogitsFastSharedTopK];
    __shared__ int tile_top_indices[kLogitsFastSharedTopK];
    __shared__ float merged_top_values[kLogitsMaxSelected];
    __shared__ int merged_top_indices[kLogitsMaxSelected];

    if (threadIdx.x == 0) {
        for (int slot = 0; slot < top_k; ++slot) {
            row_top_values[slot] = -INFINITY;
            row_top_indices[slot] = -1;
        }
    }
    __syncthreads();

    for (int tile_base = static_cast<int>(blockIdx.x) * kLogitsTopKTileSize;
         tile_base < column_count;
         tile_base += block_count * kLogitsTopKTileSize) {
        float thread_keys[kLogitsTopKItemsPerThread];
        int thread_values[kLogitsTopKItemsPerThread];
#pragma unroll
        for (int item = 0; item < kLogitsTopKItemsPerThread; ++item) {
            const int column =
                tile_base + static_cast<int>(threadIdx.x) * kLogitsTopKItemsPerThread + item;
            if (column < column_count) {
                thread_keys[item] = row_input[column];
                thread_values[item] = column;
            } else {
                thread_keys[item] = -INFINITY;
                thread_values[item] = column_count;
            }
        }

        TopKBlockRadixSort(sort_storage).SortDescending(thread_keys, thread_values);
        __syncthreads();

#pragma unroll
        for (int item = 0; item < kLogitsTopKItemsPerThread; ++item) {
            const int rank = static_cast<int>(threadIdx.x) * kLogitsTopKItemsPerThread + item;
            if (rank < top_k) {
                tile_top_values[rank] = thread_keys[item];
                tile_top_indices[rank] = thread_values[item];
            }
        }
        __syncthreads();

        if (threadIdx.x == 0) {
            int row_slot = 0;
            int tile_slot = 0;
            for (int merged_slot = 0; merged_slot < top_k; ++merged_slot) {
                float row_value = -INFINITY;
                int row_index = -1;
                if (row_slot < top_k) {
                    row_value = row_top_values[row_slot];
                    row_index = row_top_indices[row_slot];
                }

                float tile_value = -INFINITY;
                int tile_index = -1;
                if (tile_slot < top_k) {
                    const int candidate_index = tile_top_indices[tile_slot];
                    if (candidate_index >= 0 && candidate_index < column_count) {
                        tile_value = tile_top_values[tile_slot];
                        tile_index = candidate_index;
                    }
                }

                if (top_k_candidate_better(tile_value, tile_index, row_value, row_index)) {
                    merged_top_values[merged_slot] = tile_value;
                    merged_top_indices[merged_slot] = tile_index;
                    ++tile_slot;
                } else {
                    merged_top_values[merged_slot] = row_value;
                    merged_top_indices[merged_slot] = row_index;
                    ++row_slot;
                }
            }

            for (int slot = 0; slot < top_k; ++slot) {
                row_top_values[slot] = merged_top_values[slot];
                row_top_indices[slot] = merged_top_indices[slot];
            }
        }
        __syncthreads();
    }

    const size_t block_offset = static_cast<size_t>(blockIdx.x) * static_cast<size_t>(top_k);
    for (int slot = static_cast<int>(threadIdx.x); slot < top_k; slot += blockDim.x) {
        partial_indices[block_offset + static_cast<size_t>(slot)] = row_top_indices[slot];
        partial_values[block_offset + static_cast<size_t>(slot)] = row_top_values[slot];
    }
}

__global__ void remap_top_k_indices_kernel(
    const int32_t *source_indices,
    int32_t *selected_indices,
    int top_k
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + static_cast<int>(threadIdx.x);
    if (index >= top_k) {
        return;
    }
    const int32_t partial_index = selected_indices[index];
    if (partial_index < 0) {
        selected_indices[index] = -1;
        return;
    }
    selected_indices[index] = source_indices[partial_index];
}

__global__ void apply_sampling_penalties_f32_sparse_kernel(
    float *logits,
    int vocab_size,
    const int32_t *token_ids,
    const int32_t *token_counts,
    int active_token_count,
    float repeat_penalty,
    float presence_penalty,
    float frequency_penalty
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + static_cast<int>(threadIdx.x);
    if (index >= active_token_count) {
        return;
    }

    const int token_id = token_ids[index];
    if (token_id < 0 || token_id >= vocab_size) {
        return;
    }

    const int count = token_counts[index];
    if (count <= 0) {
        return;
    }

    float logit = logits[token_id];
    if (fabsf(repeat_penalty - 1.0f) > FLT_EPSILON) {
        if (logit < 0.0f) {
            logit *= repeat_penalty;
        } else {
            logit /= repeat_penalty;
        }
    }
    if (fabsf(frequency_penalty) > FLT_EPSILON) {
        logit -= frequency_penalty * static_cast<float>(count);
    }
    if (fabsf(presence_penalty) > FLT_EPSILON) {
        logit -= presence_penalty;
    }
    logits[token_id] = logit;
}

__global__ void copy_top_k_pairs_kernel(
    const int32_t *sorted_indices,
    const float *sorted_values,
    int top_k,
    int32_t *selected_indices,
    float *selected_values
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + static_cast<int>(threadIdx.x);
    if (index >= top_k) {
        return;
    }
    selected_indices[index] = sorted_indices[index];
    selected_values[index] = sorted_values[index];
}

__global__ void add_f32_offset_in_place_kernel(
    float *destination,
    int element_offset,
    const float *rhs,
    int element_count
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index < element_count) {
        destination[element_offset + index] += rhs[index];
    }
}

__global__ void mul_f32_kernel(
    const float *left,
    const float *right,
    int element_count,
    float *output
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index < element_count) {
        output[index] = left[index] * right[index];
    }
}

__global__ void silu_mul_f32_kernel(
    const float *activation_input,
    int activation_offset,
    const float *rhs,
    int rhs_offset,
    int element_count,
    float *output
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index < element_count) {
        output[index] = silu_single(activation_input[activation_offset + index]) *
            rhs[rhs_offset + index];
    }
}

__global__ void silu_mul_q8_1_kernel(
    const float *activation_input,
    int activation_offset,
    const float *rhs,
    int rhs_offset,
    int element_count,
    Q81Block *output
) {
    const int warp_index = static_cast<int>(threadIdx.x) / kWarpSize;
    const int lane = static_cast<int>(threadIdx.x) & (kWarpSize - 1);
    const int blocks_per_cta = kBlockSize / kWarpSize;
    const int output_block_index = static_cast<int>(blockIdx.x) * blocks_per_cta + warp_index;
    const int block_count = element_count / kQ81ElementsPerBlock;
    if (output_block_index >= block_count) {
        return;
    }

    const int input_index = output_block_index * kQ81ElementsPerBlock + lane;
    const float value = silu_single(activation_input[activation_offset + input_index]) *
        rhs[rhs_offset + input_index];

    float amax = fabsf(value);
    float sum = value;
    for (int offset = 16; offset > 0; offset >>= 1) {
        amax = fmaxf(amax, __shfl_xor_sync(0xffffffffu, amax, offset, 32));
        sum += __shfl_xor_sync(0xffffffffu, sum, offset, 32);
    }

    const float scale = amax == 0.0f ? 0.0f : amax / 127.0f;
    const float quantized = scale == 0.0f ? 0.0f : value / scale;
    const float clamped = fminf(fmaxf(roundf(quantized), -127.0f), 127.0f);

    Q81Block *row_output = output + output_block_index;
    row_output->bytes[4 + lane] = static_cast<uint8_t>(static_cast<int8_t>(clamped));
    if (lane == 0) {
        const uint16_t scale_bits = __half_as_ushort(__float2half_rn(scale));
        const uint16_t sum_bits = __half_as_ushort(__float2half_rn(sum));
        row_output->bytes[0] = static_cast<uint8_t>(scale_bits & 0xffu);
        row_output->bytes[1] = static_cast<uint8_t>((scale_bits >> 8) & 0xffu);
        row_output->bytes[2] = static_cast<uint8_t>(sum_bits & 0xffu);
        row_output->bytes[3] = static_cast<uint8_t>((sum_bits >> 8) & 0xffu);
    }
}

__global__ void sigmoid_mul_f32_kernel(
    const float *values,
    int values_offset,
    const float *gate,
    int gate_offset,
    int element_count,
    float *output
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index < element_count) {
        output[index] = values[values_offset + index] *
            sigmoid_single(gate[gate_offset + index]);
    }
}

__global__ void sigmoid_mul_q8_1_kernel(
    const float *values,
    int values_offset,
    const float *gate,
    int gate_offset,
    int element_count,
    Q81Block *output
) {
    const int warp_index = static_cast<int>(threadIdx.x) / kWarpSize;
    const int lane = static_cast<int>(threadIdx.x) & (kWarpSize - 1);
    const int blocks_per_cta = kBlockSize / kWarpSize;
    const int output_block_index = static_cast<int>(blockIdx.x) * blocks_per_cta + warp_index;
    const int block_count = element_count / kQ81ElementsPerBlock;
    if (output_block_index >= block_count) {
        return;
    }

    const int input_index = output_block_index * kQ81ElementsPerBlock + lane;
    const float value = values[values_offset + input_index] *
        sigmoid_single(gate[gate_offset + input_index]);

    float amax = fabsf(value);
    float sum = value;
    for (int offset = 16; offset > 0; offset >>= 1) {
        amax = fmaxf(amax, __shfl_xor_sync(0xffffffffu, amax, offset, 32));
        sum += __shfl_xor_sync(0xffffffffu, sum, offset, 32);
    }

    const float scale = amax == 0.0f ? 0.0f : amax / 127.0f;
    const float quantized = scale == 0.0f ? 0.0f : value / scale;
    const float clamped = fminf(fmaxf(roundf(quantized), -127.0f), 127.0f);

    Q81Block *row_output = output + output_block_index;
    row_output->bytes[4 + lane] = static_cast<uint8_t>(static_cast<int8_t>(clamped));
    if (lane == 0) {
        const uint16_t scale_bits = __half_as_ushort(__float2half_rn(scale));
        const uint16_t sum_bits = __half_as_ushort(__float2half_rn(sum));
        row_output->bytes[0] = static_cast<uint8_t>(scale_bits & 0xffu);
        row_output->bytes[1] = static_cast<uint8_t>((scale_bits >> 8) & 0xffu);
        row_output->bytes[2] = static_cast<uint8_t>(sum_bits & 0xffu);
        row_output->bytes[3] = static_cast<uint8_t>((sum_bits >> 8) & 0xffu);
    }
}

__global__ void split_interleaved_query_gate_f32_kernel(
    const float *input,
    int head_count,
    int head_dim,
    float *query_output,
    float *gate_output
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + threadIdx.x;
    const int element_count = head_count * head_dim;
    if (index >= element_count) {
        return;
    }
    const int head_index = index / head_dim;
    const int dim_index = index % head_dim;
    const int source_base = head_index * head_dim * 2;
    query_output[index] = input[source_base + dim_index];
    gate_output[index] = input[source_base + head_dim + dim_index];
}

__global__ void split_interleaved_query_gate_rms_norm_f32_kernel(
    const float *input,
    int head_count,
    int head_dim,
    const float *weight,
    float epsilon,
    float *query_output,
    float *gate_output
) {
    const int head_index = static_cast<int>(blockIdx.x);
    if (head_index >= head_count) {
        return;
    }
    const int source_base = head_index * head_dim * 2;
    const int output_base = head_index * head_dim;
    __shared__ float scratch[kBlockSize];

    float mean_square_partial = 0.0f;
    for (int dim_index = threadIdx.x; dim_index < head_dim; dim_index += blockDim.x) {
        const float query_value = input[source_base + dim_index];
        mean_square_partial += query_value * query_value;
    }
    const float mean_square_sum = reduce_block_sum(mean_square_partial, scratch);
    const float inv_rms = rsqrtf(mean_square_sum / static_cast<float>(head_dim) + epsilon);

    for (int dim_index = threadIdx.x; dim_index < head_dim; dim_index += blockDim.x) {
        const float query_value = input[source_base + dim_index];
        query_output[output_base + dim_index] = query_value * weight[dim_index] * inv_rms;
        gate_output[output_base + dim_index] = input[source_base + head_dim + dim_index];
    }
}

__global__ void pack_qwen35_key_value_rms_norm_f32_kernel(
    const float *input,
    int key_offset,
    int value_offset,
    int kv_head_count,
    int head_dim,
    const float *weight,
    float epsilon,
    float *output,
    int output_key_offset,
    int output_value_offset
) {
    const int kv_head_index = static_cast<int>(blockIdx.x);
    if (kv_head_index >= kv_head_count) {
        return;
    }
    const int input_key_base = key_offset + kv_head_index * head_dim;
    const int input_value_base = value_offset + kv_head_index * head_dim;
    const int output_key_base = output_key_offset + kv_head_index * head_dim;
    const int output_value_base = output_value_offset + kv_head_index * head_dim;
    __shared__ float scratch[kBlockSize];

    float mean_square_partial = 0.0f;
    for (int dim_index = threadIdx.x; dim_index < head_dim; dim_index += blockDim.x) {
        const float key_value = input[input_key_base + dim_index];
        mean_square_partial += key_value * key_value;
    }
    const float mean_square_sum = reduce_block_sum(mean_square_partial, scratch);
    const float inv_rms = rsqrtf(mean_square_sum / static_cast<float>(head_dim) + epsilon);

    for (int dim_index = threadIdx.x; dim_index < head_dim; dim_index += blockDim.x) {
        output[output_key_base + dim_index] =
            input[input_key_base + dim_index] * weight[dim_index] * inv_rms;
        output[output_value_base + dim_index] = input[input_value_base + dim_index];
    }
}

__global__ void pack_qwen35_hybrid_qkv_rms_norm_f32_kernel(
    const float *input,
    int q_offset,
    int k_offset,
    int v_offset,
    int group_count,
    int state_size,
    int v_size,
    const float *q_weight,
    const float *k_weight,
    float epsilon,
    float *output,
    int output_q_offset,
    int output_k_offset,
    int output_v_offset
) {
    const int block_index = static_cast<int>(blockIdx.x);
    __shared__ float scratch[kBlockSize];

    if (block_index < group_count) {
        const int q_input_base = q_offset + block_index * state_size;
        const int q_output_base = output_q_offset + block_index * state_size;
        float mean_square_partial = 0.0f;
        for (int feature = threadIdx.x; feature < state_size; feature += blockDim.x) {
            const float value = input[q_input_base + feature];
            mean_square_partial += value * value;
        }
        const float mean_square_sum = reduce_block_sum(mean_square_partial, scratch);
        const float inv_rms = rsqrtf(mean_square_sum / static_cast<float>(state_size) + epsilon);
        for (int feature = threadIdx.x; feature < state_size; feature += blockDim.x) {
            output[q_output_base + feature] =
                input[q_input_base + feature] * q_weight[feature] * inv_rms;
        }
    }

    __syncthreads();

    if (block_index < group_count) {
        const int k_input_base = k_offset + block_index * state_size;
        const int k_output_base = output_k_offset + block_index * state_size;
        float mean_square_partial = 0.0f;
        for (int feature = threadIdx.x; feature < state_size; feature += blockDim.x) {
            const float value = input[k_input_base + feature];
            mean_square_partial += value * value;
        }
        const float mean_square_sum = reduce_block_sum(mean_square_partial, scratch);
        const float inv_rms = rsqrtf(mean_square_sum / static_cast<float>(state_size) + epsilon);
        for (int feature = threadIdx.x; feature < state_size; feature += blockDim.x) {
            output[k_output_base + feature] =
                input[k_input_base + feature] * k_weight[feature] * inv_rms;
        }
    }

    const int v_index = block_index * static_cast<int>(blockDim.x) + static_cast<int>(threadIdx.x);
    if (v_index < v_size) {
        output[output_v_offset + v_index] = input[v_offset + v_index];
    }
}

__global__ void depthwise_causal_conv1d_step_f32_kernel(
    const float *input,
    float *state,
    const float *weights,
    int channels,
    int kernel_size,
    float *output
) {
    const int channel = static_cast<int>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (channel >= channels) {
        return;
    }
    const int state_tokens = kernel_size - 1;
    const float *channel_weights = weights + channel * kernel_size;
    float *channel_state = state + channel * state_tokens;

    float sum = input[channel] * channel_weights[state_tokens];
    for (int token = 0; token < state_tokens; ++token) {
        sum += channel_state[token] * channel_weights[token];
    }
    output[channel] = sum;

    for (int token = 0; token + 1 < state_tokens; ++token) {
        channel_state[token] = channel_state[token + 1];
    }
    if (state_tokens > 0) {
        channel_state[state_tokens - 1] = input[channel];
    }
}

__global__ void depthwise_causal_conv1d_step_silu_f32_kernel(
    const float *input,
    float *state,
    const float *weights,
    int channels,
    int kernel_size,
    float *output
) {
    const int channel = static_cast<int>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (channel >= channels) {
        return;
    }
    const int state_tokens = kernel_size - 1;
    const float *channel_weights = weights + channel * kernel_size;
    float *channel_state = state + channel * state_tokens;

    float sum = input[channel] * channel_weights[state_tokens];
    for (int token = 0; token < state_tokens; ++token) {
        sum += channel_state[token] * channel_weights[token];
    }
    output[channel] = silu_single(sum);

    for (int token = 0; token + 1 < state_tokens; ++token) {
        channel_state[token] = channel_state[token + 1];
    }
    if (state_tokens > 0) {
        channel_state[state_tokens - 1] = input[channel];
    }
}

__global__ void qwen35_ssm_decay_beta_f32_kernel(
    const float *input,
    int alpha_offset,
    int beta_offset,
    const float *ssm_a,
    const float *ssm_dt,
    int element_count,
    float *decay_output,
    float *beta_output
) {
    const int index = static_cast<int>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (index >= element_count) {
        return;
    }
    const float alpha = input[alpha_offset + index];
    const float beta = input[beta_offset + index];
    decay_output[index] = expf(softplus_single(alpha + ssm_dt[index]) * ssm_a[index]);
    beta_output[index] = sigmoid_single(beta);
}

__global__ void gated_delta_step_f32_kernel(
    const float *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    const float *decay,
    const float *beta,
    float *state,
    int key_head_count,
    int value_head_count,
    int key_dim,
    int value_dim,
    int v_head_reordered,
    float *output
) {
    const int lane = static_cast<int>(threadIdx.x);
    const int value_dim_index = static_cast<int>(blockIdx.y);
    const int value_head_index = static_cast<int>(blockIdx.z);
    if (value_dim_index >= value_dim || value_head_index >= value_head_count) {
        return;
    }
    const int repeat_factor = value_head_count / key_head_count;
    const int key_head_index = v_head_reordered != 0
        ? (key_head_count > 0 ? value_head_index % key_head_count : 0)
        : (repeat_factor > 0 ? value_head_index / repeat_factor : 0);
    const float *query = qkv + query_offset + key_head_index * key_dim;
    const float *key = qkv + key_offset + key_head_index * key_dim;
    const float *value = qkv + value_offset + value_head_index * value_dim;
    float *state_row = state + (value_head_index * value_dim + value_dim_index) * key_dim;
    const float decay_value = decay[value_head_index];
    float kv_mem = 0.0f;
    for (int key_index = lane; key_index < key_dim; key_index += kWarpSize) {
        state_row[key_index] *= decay_value;
        kv_mem += state_row[key_index] * key[key_index];
    }
    for (int offset = kWarpSize / 2; offset > 0; offset >>= 1) {
        kv_mem += __shfl_down_sync(0xffffffffu, kv_mem, offset);
    }
    kv_mem = __shfl_sync(0xffffffffu, kv_mem, 0);

    const float delta = (value[value_dim_index] - kv_mem) * beta[value_head_index];
    float sum = 0.0f;
    for (int key_index = lane; key_index < key_dim; key_index += kWarpSize) {
        state_row[key_index] += key[key_index] * delta;
        sum += state_row[key_index] * query[key_index];
    }
    for (int offset = kWarpSize / 2; offset > 0; offset >>= 1) {
        sum += __shfl_down_sync(0xffffffffu, sum, offset);
    }
    if (lane == 0) {
        output[value_head_index * value_dim + value_dim_index] = sum;
    }
}

__global__ void rms_norm_kernel(
    const float *input,
    const float *weight,
    int feature_count,
    float epsilon,
    float *output
) {
    const int row = static_cast<int>(blockIdx.x);
    const int row_offset = row * feature_count;
    __shared__ float scratch[kBlockSize];

    float mean_square_partial = 0.0f;
    for (int feature = threadIdx.x; feature < feature_count; feature += blockDim.x) {
        const float value = input[row_offset + feature];
        mean_square_partial += value * value;
    }
    const float mean_square_sum = reduce_block_sum(mean_square_partial, scratch);
    const float inv_rms = rsqrtf(mean_square_sum / static_cast<float>(feature_count) + epsilon);
    for (int feature = threadIdx.x; feature < feature_count; feature += blockDim.x) {
        output[row_offset + feature] = input[row_offset + feature] * weight[feature] * inv_rms;
    }
}

__global__ void rms_norm_region_kernel(
    const float *input,
    int input_offset,
    const float *weight,
    int feature_count,
    float epsilon,
    float *output,
    int output_offset
) {
    const int row = static_cast<int>(blockIdx.x);
    const int row_offset = row * feature_count;
    const int input_base = input_offset + row_offset;
    const int output_base = output_offset + row_offset;
    __shared__ float scratch[kBlockSize];

    float mean_square_partial = 0.0f;
    for (int feature = threadIdx.x; feature < feature_count; feature += blockDim.x) {
        const float value = input[input_base + feature];
        mean_square_partial += value * value;
    }
    const float mean_square_sum = reduce_block_sum(mean_square_partial, scratch);
    const float inv_rms = rsqrtf(mean_square_sum / static_cast<float>(feature_count) + epsilon);
    for (int feature = threadIdx.x; feature < feature_count; feature += blockDim.x) {
        output[output_base + feature] = input[input_base + feature] * weight[feature] * inv_rms;
    }
}

__global__ void rms_norm_input_backward_kernel(
    const float *input,
    const float *weight,
    const float *grad_output,
    int feature_count,
    float epsilon,
    float *output
) {
    const int row = static_cast<int>(blockIdx.x);
    const int row_offset = row * feature_count;
    __shared__ float scratch[kBlockSize];

    float mean_square_partial = 0.0f;
    for (int feature = threadIdx.x; feature < feature_count; feature += blockDim.x) {
        const float value = input[row_offset + feature];
        mean_square_partial += value * value;
    }
    const float mean_square_sum = reduce_block_sum(mean_square_partial, scratch);
    const float inv_rms = rsqrtf(mean_square_sum / static_cast<float>(feature_count) + epsilon);
    const float inv_rms_cubed = inv_rms * inv_rms * inv_rms;

    float weighted_dot_partial = 0.0f;
    for (int feature = threadIdx.x; feature < feature_count; feature += blockDim.x) {
        weighted_dot_partial +=
            input[row_offset + feature] *
            weight[feature] *
            grad_output[row_offset + feature];
    }
    const float weighted_dot = reduce_block_sum(weighted_dot_partial, scratch);
    const float correction = inv_rms_cubed * weighted_dot / static_cast<float>(feature_count);

    for (int feature = threadIdx.x; feature < feature_count; feature += blockDim.x) {
        const float value = input[row_offset + feature];
        const float scale = weight[feature];
        const float grad = grad_output[row_offset + feature];
        output[row_offset + feature] = (grad * scale * inv_rms) - (value * correction);
    }
}

__global__ void rms_norm_weight_backward_kernel(
    const float *input,
    const float *grad_output,
    int row_count,
    int feature_count,
    float epsilon,
    float *output
) {
    __shared__ float scratch[kBlockSize];

    for (int feature = threadIdx.x; feature < feature_count; feature += blockDim.x) {
        output[feature] = 0.0f;
    }
    __syncthreads();

    for (int row = 0; row < row_count; ++row) {
        const int row_offset = row * feature_count;
        float mean_square_partial = 0.0f;
        for (int feature = threadIdx.x; feature < feature_count; feature += blockDim.x) {
            const float value = input[row_offset + feature];
            mean_square_partial += value * value;
        }
        const float mean_square_sum = reduce_block_sum(mean_square_partial, scratch);
        const float inv_rms = rsqrtf(mean_square_sum / static_cast<float>(feature_count) + epsilon);

        for (int feature = threadIdx.x; feature < feature_count; feature += blockDim.x) {
            output[feature] += grad_output[row_offset + feature] *
                input[row_offset + feature] *
                inv_rms;
        }
        __syncthreads();
    }
}

__global__ void parameter_golf_projection_loss_kernel(
    const float *logits,
    const int *target_ids,
    int row_count,
    int vocab_size,
    float logit_softcap,
    float *output
) {
    __shared__ float scratch[kBlockSize];
    const int row = blockIdx.x;
    if (row >= row_count) {
        return;
    }
    const int row_offset = row * vocab_size;
    float local_max = -FLT_MAX;
    for (int column = threadIdx.x; column < vocab_size; column += blockDim.x) {
        const float softcapped =
            logit_softcap * tanhf(logits[row_offset + column] / logit_softcap);
        local_max = fmaxf(local_max, softcapped);
    }
    const float max_value = reduce_block_max(local_max, scratch);
    float local_sum = 0.0f;
    for (int column = threadIdx.x; column < vocab_size; column += blockDim.x) {
        const float softcapped =
            logit_softcap * tanhf(logits[row_offset + column] / logit_softcap);
        local_sum += expf(softcapped - max_value);
    }
    const float denom = fmaxf(reduce_block_sum(local_sum, scratch), 1e-20f);
    if (threadIdx.x == 0) {
        const int target = static_cast<int>(target_ids[row]);
        const float target_softcapped =
            logit_softcap * tanhf(logits[row_offset + target] / logit_softcap);
        atomicAdd(
            output,
            (max_value + logf(denom) - target_softcapped) / static_cast<float>(row_count)
        );
    }
}

__global__ void parameter_golf_projection_token_losses_kernel(
    const float *logits,
    const int *target_ids,
    int row_count,
    int vocab_size,
    float logit_softcap,
    float *output
) {
    __shared__ float scratch[kBlockSize];
    const int row = blockIdx.x;
    if (row >= row_count) {
        return;
    }
    const int row_offset = row * vocab_size;
    float local_max = -FLT_MAX;
    for (int column = threadIdx.x; column < vocab_size; column += blockDim.x) {
        const float softcapped =
            logit_softcap * tanhf(logits[row_offset + column] / logit_softcap);
        local_max = fmaxf(local_max, softcapped);
    }
    const float max_value = reduce_block_max(local_max, scratch);
    float local_sum = 0.0f;
    for (int column = threadIdx.x; column < vocab_size; column += blockDim.x) {
        const float softcapped =
            logit_softcap * tanhf(logits[row_offset + column] / logit_softcap);
        local_sum += expf(softcapped - max_value);
    }
    const float denom = fmaxf(reduce_block_sum(local_sum, scratch), 1e-20f);
    if (threadIdx.x == 0) {
        const int target = static_cast<int>(target_ids[row]);
        const float target_softcapped =
            logit_softcap * tanhf(logits[row_offset + target] / logit_softcap);
        output[row] = max_value + logf(denom) - target_softcapped;
    }
}

__global__ void parameter_golf_projection_loss_backward_kernel(
    const float *logits,
    const int *target_ids,
    const float *grad_output,
    int row_count,
    int vocab_size,
    float logit_softcap,
    float *output
) {
    __shared__ float scratch[kBlockSize];
    const int row = blockIdx.x;
    if (row >= row_count) {
        return;
    }
    const int row_offset = row * vocab_size;
    float local_max = -FLT_MAX;
    for (int column = threadIdx.x; column < vocab_size; column += blockDim.x) {
        const float softcapped =
            logit_softcap * tanhf(logits[row_offset + column] / logit_softcap);
        local_max = fmaxf(local_max, softcapped);
    }
    const float max_value = reduce_block_max(local_max, scratch);
    float local_sum = 0.0f;
    for (int column = threadIdx.x; column < vocab_size; column += blockDim.x) {
        const float softcapped =
            logit_softcap * tanhf(logits[row_offset + column] / logit_softcap);
        local_sum += expf(softcapped - max_value);
    }
    const float denom = fmaxf(reduce_block_sum(local_sum, scratch), 1e-20f);
    const int target = static_cast<int>(target_ids[row]);
    const float scale = grad_output[0] / static_cast<float>(row_count);
    for (int column = threadIdx.x; column < vocab_size; column += blockDim.x) {
        const float softcapped =
            logit_softcap * tanhf(logits[row_offset + column] / logit_softcap);
        const float probability = expf(softcapped - max_value) / denom;
        const float delta = column == target ? 1.0f : 0.0f;
        const float ratio = softcapped / logit_softcap;
        const float softcap_derivative = 1.0f - ratio * ratio;
        output[row_offset + column] =
            scale * (probability - delta) * softcap_derivative;
    }
}

__global__ void parameter_golf_token_embedding_lookup_kernel(
    const int *token_ids,
    const float *token_embedding,
    int row_count,
    int vocab_size,
    int width,
    float *output
) {
    const int row = blockIdx.x;
    if (row >= row_count) {
        return;
    }
    const int token_id = token_ids[row];
    if (token_id < 0 || token_id >= vocab_size) {
        return;
    }
    const int source_offset = token_id * width;
    const int destination_offset = row * width;
    for (int column = threadIdx.x; column < width; column += blockDim.x) {
        output[destination_offset + column] = token_embedding[source_offset + column];
    }
}

__global__ void parameter_golf_token_embedding_lookup_bf16_to_f32_kernel(
    const int *token_ids,
    const __nv_bfloat16 *token_embedding,
    int row_count,
    int vocab_size,
    int width,
    float *output
) {
    const int row = blockIdx.x;
    if (row >= row_count) {
        return;
    }
    const int token_id = token_ids[row];
    if (token_id < 0 || token_id >= vocab_size) {
        return;
    }
    const int source_offset = token_id * width;
    const int destination_offset = row * width;
    for (int column = threadIdx.x; column < width; column += blockDim.x) {
        output[destination_offset + column] = __bfloat162float(
            token_embedding[source_offset + column]
        );
    }
}

__global__ void parameter_golf_token_embedding_lookup_backward_kernel(
    const int *token_ids,
    const float *grad_output,
    int row_count,
    int vocab_size,
    int width,
    float *output
) {
    const int row = blockIdx.x;
    if (row >= row_count) {
        return;
    }
    const int token_id = token_ids[row];
    if (token_id < 0 || token_id >= vocab_size) {
        return;
    }
    const int source_offset = row * width;
    const int destination_offset = token_id * width;
    for (int column = threadIdx.x; column < width; column += blockDim.x) {
        atomicAdd(
            &output[destination_offset + column],
            grad_output[source_offset + column]
        );
    }
}

__global__ void rms_norm_q8_1_kernel(
    const float *input,
    const float *weight,
    int element_count,
    float epsilon,
    Q81Block *output
) {
    __shared__ float scratch[kBlockSize];

    const int vec4_count = element_count >> 2;
    float sum = 0.0f;
    if ((reinterpret_cast<uintptr_t>(input) & 15) == 0) {
        const float4 *input4 = reinterpret_cast<const float4 *>(input);
        for (int index = static_cast<int>(threadIdx.x); index < vec4_count; index += blockDim.x) {
            const float4 value = input4[index];
            sum += value.x * value.x + value.y * value.y + value.z * value.z + value.w * value.w;
        }
        for (int index = (vec4_count << 2) + static_cast<int>(threadIdx.x); index < element_count; index += blockDim.x) {
            const float value = input[index];
            sum += value * value;
        }
    } else {
        for (int index = static_cast<int>(threadIdx.x); index < element_count; index += blockDim.x) {
            const float value = input[index];
            sum += value * value;
        }
    }
    scratch[threadIdx.x] = sum;
    __syncthreads();

    for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (threadIdx.x < offset) {
            scratch[threadIdx.x] += scratch[threadIdx.x + offset];
        }
        __syncthreads();
    }

    const float inv_rms = rsqrtf(scratch[0] / static_cast<float>(element_count) + epsilon);
    const int warp_id = static_cast<int>(threadIdx.x) / kWarpSize;
    const int lane = static_cast<int>(threadIdx.x) % kWarpSize;
    const int warp_count = blockDim.x / kWarpSize;
    const int blocks_per_row = element_count / kQ81ElementsPerBlock;

    for (int block_index = warp_id; block_index < blocks_per_row; block_index += warp_count) {
        const int index = block_index * kQ81ElementsPerBlock + lane;
        const float normalized = input[index] * weight[index] * inv_rms;
        quantize_q8_1_warp_value(normalized, output + block_index, lane);
    }
}

__global__ void add_residual_rms_norm_kernel(
    const float *input,
    const float *residual,
    const float *input_bias,
    const float *weight,
    int element_count,
    float epsilon,
    float *summed_output,
    float *normalized_output
) {
    __shared__ float scratch[kBlockSize];
    float sum = 0.0f;
    for (int index = threadIdx.x; index < element_count; index += blockDim.x) {
        const float value =
            input[index] +
            residual[index] +
            (input_bias != nullptr ? input_bias[index] : 0.0f);
        sum += value * value;
    }
    scratch[threadIdx.x] = sum;
    __syncthreads();

    for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (threadIdx.x < offset) {
            scratch[threadIdx.x] += scratch[threadIdx.x + offset];
        }
        __syncthreads();
    }

    const float inv_rms = rsqrtf(scratch[0] / static_cast<float>(element_count) + epsilon);
    for (int index = threadIdx.x; index < element_count; index += blockDim.x) {
        const float value =
            input[index] +
            residual[index] +
            (input_bias != nullptr ? input_bias[index] : 0.0f);
        summed_output[index] = value;
        normalized_output[index] = value * weight[index] * inv_rms;
    }
}

__global__ void add_residual_rms_norm_q8_1_kernel(
    const float *input,
    const float *residual,
    const float *input_bias,
    const float *weight,
    int element_count,
    float epsilon,
    float *summed_output,
    float *normalized_output,
    Q81Block *quantized_output
) {
    __shared__ float scratch[kBlockSize];

    const int vec4_count = element_count >> 2;
    const bool vectorized =
        (reinterpret_cast<uintptr_t>(input) & 15) == 0 &&
        (reinterpret_cast<uintptr_t>(residual) & 15) == 0 &&
        (input_bias == nullptr || (reinterpret_cast<uintptr_t>(input_bias) & 15) == 0);
    float sum = 0.0f;
    if (vectorized) {
        const float4 *input4 = reinterpret_cast<const float4 *>(input);
        const float4 *residual4 = reinterpret_cast<const float4 *>(residual);
        const float4 *bias4 = reinterpret_cast<const float4 *>(input_bias);
        for (int index = static_cast<int>(threadIdx.x); index < vec4_count; index += blockDim.x) {
            const float4 a = input4[index];
            const float4 b = residual4[index];
            const float4 c = input_bias != nullptr ? bias4[index] : make_float4(0.0f, 0.0f, 0.0f, 0.0f);
            const float v0 = a.x + b.x + c.x;
            const float v1 = a.y + b.y + c.y;
            const float v2 = a.z + b.z + c.z;
            const float v3 = a.w + b.w + c.w;
            sum += v0 * v0 + v1 * v1 + v2 * v2 + v3 * v3;
        }
    }
    for (int index = (vectorized ? (vec4_count << 2) : 0) + static_cast<int>(threadIdx.x); index < element_count; index += blockDim.x) {
        const float value =
            input[index] +
            residual[index] +
            (input_bias != nullptr ? input_bias[index] : 0.0f);
        sum += value * value;
    }
    scratch[threadIdx.x] = sum;
    __syncthreads();

    for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (threadIdx.x < offset) {
            scratch[threadIdx.x] += scratch[threadIdx.x + offset];
        }
        __syncthreads();
    }

    const float inv_rms = rsqrtf(scratch[0] / static_cast<float>(element_count) + epsilon);
    const int warp_id = static_cast<int>(threadIdx.x) / kWarpSize;
    const int lane = static_cast<int>(threadIdx.x) % kWarpSize;
    const int warp_count = blockDim.x / kWarpSize;
    const int blocks_per_row = element_count / kQ81ElementsPerBlock;

    for (int block_index = warp_id; block_index < blocks_per_row; block_index += warp_count) {
        const int index = block_index * kQ81ElementsPerBlock + lane;
        const float value =
            input[index] +
            residual[index] +
            (input_bias != nullptr ? input_bias[index] : 0.0f);
        const float normalized = value * weight[index] * inv_rms;
        summed_output[index] = value;
        normalized_output[index] = normalized;
        quantize_q8_1_warp_value(normalized, quantized_output + block_index, lane);
    }
}

__global__ void add_residual_rms_norm_q8_1_router_topk_kernel(
    const float *input,
    const float *residual,
    const float *input_bias,
    const float *weight,
    int element_count,
    float epsilon,
    float *summed_output,
    float *normalized_output,
    Q81Block *quantized_output,
    const float *router_weights,
    const float *router_bias,
    int expert_count,
    int top_k,
    int32_t *selected_ids,
    float *selected_weights
) {
    __shared__ float scratch[kBlockSize];
    __shared__ float normalized_blocks[kBlockSize];
    __shared__ float logits[kMoeMaxExperts];

    float sum = 0.0f;
    for (int index = threadIdx.x; index < element_count; index += blockDim.x) {
        const float value =
            input[index] +
            residual[index] +
            (input_bias != nullptr ? input_bias[index] : 0.0f);
        sum += value * value;
    }
    scratch[threadIdx.x] = sum;
    __syncthreads();

    for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (threadIdx.x < offset) {
            scratch[threadIdx.x] += scratch[threadIdx.x + offset];
        }
        __syncthreads();
    }

    const float inv_rms = rsqrtf(scratch[0] / static_cast<float>(element_count) + epsilon);
    const int warp_id = static_cast<int>(threadIdx.x) / kWarpSize;
    const int lane = static_cast<int>(threadIdx.x) % kWarpSize;
    const int warp_count = blockDim.x / kWarpSize;
    const int blocks_per_row = element_count / kQ81ElementsPerBlock;

    for (int tile = 0; tile < blocks_per_row; tile += warp_count) {
        const int block_index = tile + warp_id;
        if (block_index < blocks_per_row) {
            const int index = block_index * kQ81ElementsPerBlock + lane;
            const float value =
                input[index] +
                residual[index] +
                (input_bias != nullptr ? input_bias[index] : 0.0f);
            const float normalized = value * weight[index] * inv_rms;
            summed_output[index] = value;
            normalized_output[index] = normalized;
            normalized_blocks[threadIdx.x] = normalized;
        } else {
            normalized_blocks[threadIdx.x] = 0.0f;
        }
        __syncthreads();
        if (block_index < blocks_per_row) {
            quantize_q8_1_shared_block(
                normalized_blocks + warp_id * kQ81ElementsPerBlock,
                quantized_output + block_index,
                lane
            );
        }
        __syncthreads();
    }

    expert_count = min(expert_count, kMoeMaxExperts);
    top_k = min(top_k, min(expert_count, kMoeMaxSelected));
    for (int expert = 0; expert < expert_count; ++expert) {
        const float *row = router_weights + static_cast<size_t>(expert) * static_cast<size_t>(element_count);
        float partial = 0.0f;
        for (int index = threadIdx.x; index < element_count; index += blockDim.x) {
            partial += row[index] * normalized_output[index];
        }
        const float reduced = reduce_block_sum(partial, scratch);
        if (threadIdx.x == 0) {
            logits[expert] = reduced + (router_bias != nullptr ? router_bias[expert] : 0.0f);
        }
        __syncthreads();
    }

    if (threadIdx.x != 0) {
        return;
    }

    float top_values[kMoeMaxSelected];
    int top_indices[kMoeMaxSelected];
    for (int index = 0; index < top_k; ++index) {
        top_values[index] = -INFINITY;
        top_indices[index] = -1;
    }

    for (int expert = 0; expert < expert_count; ++expert) {
        const float value = logits[expert];
        int insert_at = top_k;
        for (int slot = 0; slot < top_k; ++slot) {
            if (value > top_values[slot] ||
                (value == top_values[slot] && (top_indices[slot] < 0 || expert < top_indices[slot]))) {
                insert_at = slot;
                break;
            }
        }
        if (insert_at >= top_k) {
            continue;
        }
        for (int slot = top_k - 1; slot > insert_at; --slot) {
            top_values[slot] = top_values[slot - 1];
            top_indices[slot] = top_indices[slot - 1];
        }
        top_values[insert_at] = value;
        top_indices[insert_at] = expert;
    }

    float max_value = -INFINITY;
    for (int slot = 0; slot < top_k; ++slot) {
        max_value = fmaxf(max_value, top_values[slot]);
    }

    float denom = 0.0f;
    for (int slot = 0; slot < top_k; ++slot) {
        const float selected = expf(top_values[slot] - max_value);
        selected_weights[slot] = selected;
        denom += selected;
    }
    if (denom != 0.0f) {
        for (int slot = 0; slot < top_k; ++slot) {
            selected_weights[slot] /= denom;
        }
    }
    for (int slot = 0; slot < top_k; ++slot) {
        selected_ids[slot] = top_indices[slot];
    }
}

__device__ __forceinline__ float rope_yarn_ramp(const float low, const float high, const int i0) {
    const float y = ((i0 / 2) - low) / fmaxf(high - low, 0.001f);
    return 1.0f - fminf(fmaxf(y, 0.0f), 1.0f);
}

__device__ __forceinline__ void rope_yarn(
    const float theta_extrap,
    const float freq_scale,
    const float corr_low,
    const float corr_high,
    const int i0,
    const float ext_factor,
    const float theta_scale,
    float &cos_theta,
    float &sin_theta
) {
    float theta_interp = freq_scale * theta_extrap;
    float theta = theta_interp;
    float mscale = 1.0f;
    if (ext_factor != 0.0f) {
        const float ramp_mix = rope_yarn_ramp(corr_low, corr_high, i0) * ext_factor;
        theta = theta_interp * (1.0f - ramp_mix) + theta_extrap * ramp_mix;
        mscale *= 1.0f + 0.1f * logf(1.0f / freq_scale);
    }
    cos_theta = cosf(theta) * mscale;
    sin_theta = sinf(theta) * mscale;
}

__global__ void rope_neox_in_place_kernel(
    float *values,
    int element_offset,
    int head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale
) {
    const int head_index = blockIdx.x;
    if (head_index >= head_count) {
        return;
    }
    const int rotary_pairs = rotary_dim / 2;
    for (int pair = threadIdx.x; pair < rotary_pairs; pair += blockDim.x) {
        const int head_base = element_offset + head_index * head_dim;
        const int index0 = head_base + pair;
        const int index1 = head_base + pair + rotary_pairs;
        if (index1 >= head_base + head_dim) {
            continue;
        }
        const float theta_base = static_cast<float>(position) * powf(theta_scale, static_cast<float>(pair));
        float cos_theta = 0.0f;
        float sin_theta = 0.0f;
        rope_yarn(
            theta_base,
            freq_scale,
            corr_low,
            corr_high,
            pair * 2,
            ext_factor,
            theta_scale,
            cos_theta,
            sin_theta
        );
        const float x0 = values[index0];
        const float x1 = values[index1];
        values[index0] = x0 * cos_theta + x1 * sin_theta;
        values[index1] = -x0 * sin_theta + x1 * cos_theta;
    }
}

__global__ void rotary_embedding_backward_kernel(
    const float *grad_output,
    const float *cos,
    const float *sin,
    int batch_size,
    int head_count,
    int sequence_length,
    int head_dim,
    int batched_tables,
    float *grad_input
) {
    const int batch_head_index = blockIdx.x;
    const int position = blockIdx.y;
    const int total_heads = batch_size * head_count;
    if (batch_head_index >= total_heads || position >= sequence_length) {
        return;
    }
    const int half_dim = head_dim / 2;
    const int batch_index = batch_head_index / head_count;
    const int token_base =
        ((batch_head_index * sequence_length) + position) * head_dim;
    const int table_base = batched_tables
        ? ((batch_index * sequence_length) + position) * half_dim
        : position * half_dim;
    for (int pair = threadIdx.x; pair < half_dim; pair += blockDim.x) {
        const int left_index = token_base + pair;
        const int right_index = left_index + half_dim;
        const float cosine = cos[table_base + pair];
        const float sine = sin[table_base + pair];
        const float grad_left = grad_output[left_index];
        const float grad_right = grad_output[right_index];
        grad_input[left_index] = grad_left * cosine - grad_right * sine;
        grad_input[right_index] = grad_left * sine + grad_right * cosine;
    }
}

__global__ void permute_rank2_transpose_f32_kernel(
    const float *input,
    int rows,
    int cols,
    float *output
) {
    const int col = blockIdx.x * blockDim.x + threadIdx.x;
    const int row = blockIdx.y * blockDim.y + threadIdx.y;
    if (row >= rows || col >= cols) {
        return;
    }
    output[col * rows + row] = input[row * cols + col];
}

__global__ void permute_rank4_swap_middle_axes_f32_kernel(
    const float *input,
    int dim0,
    int dim1,
    int dim2,
    int dim3,
    float *output
) {
    const int index = blockIdx.x * blockDim.x + threadIdx.x;
    const int total = dim0 * dim1 * dim2 * dim3;
    if (index >= total) {
        return;
    }
    int remaining = index;
    const int i3 = remaining % dim3;
    remaining /= dim3;
    const int i2 = remaining % dim2;
    remaining /= dim2;
    const int i1 = remaining % dim1;
    const int i0 = remaining / dim1;
    const int output_index = ((i0 * dim2 + i2) * dim1 + i1) * dim3 + i3;
    output[output_index] = input[index];
}

__global__ void reduce_sum_rows_f32_kernel(
    const float *input,
    int row_count,
    int column_count,
    float *output
) {
    const int row = blockIdx.x;
    if (row >= row_count) {
        return;
    }
    float partial = 0.0f;
    const int base = row * column_count;
    for (int column = threadIdx.x; column < column_count; column += blockDim.x) {
        partial += input[base + column];
    }
    __shared__ float scratch[kBlockSize];
    scratch[threadIdx.x] = partial;
    __syncthreads();
    for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (threadIdx.x < offset) {
            scratch[threadIdx.x] += scratch[threadIdx.x + offset];
        }
        __syncthreads();
    }
    if (threadIdx.x == 0) {
        output[row] = scratch[0];
    }
}

__global__ void reduce_sum_axis0_f32_kernel(
    const float *input,
    int axis0_extent,
    int row_width,
    float *output
) {
    const int index = blockIdx.x * blockDim.x + threadIdx.x;
    if (index >= row_width) {
        return;
    }
    float sum = 0.0f;
    for (int axis0_index = 0; axis0_index < axis0_extent; ++axis0_index) {
        sum += input[axis0_index * row_width + index];
    }
    output[index] = sum;
}

__global__ void reduce_sum_axis1_rank3_f32_kernel(
    const float *input,
    int dim0,
    int dim1,
    int dim2,
    float *output
) {
    const int index = blockIdx.x * blockDim.x + threadIdx.x;
    const int total = dim0 * dim2;
    if (index >= total) {
        return;
    }
    const int i2 = index % dim2;
    const int i0 = index / dim2;
    const int base = i0 * dim1 * dim2 + i2;
    float sum = 0.0f;
    for (int i1 = 0; i1 < dim1; ++i1) {
        sum += input[base + i1 * dim2];
    }
    output[index] = sum;
}

__global__ void expand_rank3_f32_kernel(
    const float *input,
    int input_dim0,
    int input_dim1,
    int input_dim2,
    int output_dim0,
    int output_dim1,
    int output_dim2,
    float *output
) {
    const int index = blockIdx.x * blockDim.x + threadIdx.x;
    const int total = output_dim0 * output_dim1 * output_dim2;
    if (index >= total) {
        return;
    }
    int remaining = index;
    const int i2 = remaining % output_dim2;
    remaining /= output_dim2;
    const int i1 = remaining % output_dim1;
    const int i0 = remaining / output_dim1;
    const int input_i0 = input_dim0 == 1 ? 0 : i0;
    const int input_i1 = input_dim1 == 1 ? 0 : i1;
    const int input_i2 = input_dim2 == 1 ? 0 : i2;
    const int input_index = (input_i0 * input_dim1 + input_i1) * input_dim2 + input_i2;
    output[index] = input[input_index];
}

__global__ void expand_rank4_f32_kernel(
    const float *input,
    int input_dim0,
    int input_dim1,
    int input_dim2,
    int input_dim3,
    int output_dim0,
    int output_dim1,
    int output_dim2,
    int output_dim3,
    float *output
) {
    const int index = blockIdx.x * blockDim.x + threadIdx.x;
    const int total = output_dim0 * output_dim1 * output_dim2 * output_dim3;
    if (index >= total) {
        return;
    }
    int remaining = index;
    const int i3 = remaining % output_dim3;
    remaining /= output_dim3;
    const int i2 = remaining % output_dim2;
    remaining /= output_dim2;
    const int i1 = remaining % output_dim1;
    remaining /= output_dim1;
    const int i0 = remaining;
    const int input_i0 = input_dim0 == 1 ? 0 : i0;
    const int input_i1 = input_dim1 == 1 ? 0 : i1;
    const int input_i2 = input_dim2 == 1 ? 0 : i2;
    const int input_i3 = input_dim3 == 1 ? 0 : i3;
    const int input_index =
        ((input_i0 * input_dim1 + input_i1) * input_dim2 + input_i2) * input_dim3 + input_i3;
    output[index] = input[input_index];
}

__device__ __forceinline__ float rope_neox_component(
    const float *values,
    int dim,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale
) {
    if (dim >= head_dim) {
        return 0.0f;
    }
    const int bounded_rotary_dim = max(min(rotary_dim, head_dim), 2);
    if (dim >= bounded_rotary_dim) {
        return values[dim];
    }

    const int rotary_pairs = bounded_rotary_dim / 2;
    const int pair = dim < rotary_pairs ? dim : dim - rotary_pairs;
    const int index0 = pair;
    const int index1 = pair + rotary_pairs;
    const float theta_base =
        static_cast<float>(position) * powf(theta_scale, static_cast<float>(pair));
    float cos_theta = 0.0f;
    float sin_theta = 0.0f;
    rope_yarn(
        theta_base,
        freq_scale,
        corr_low,
        corr_high,
        pair * 2,
        ext_factor,
        theta_scale,
        cos_theta,
        sin_theta
    );
    const float x0 = values[index0];
    const float x1 = values[index1];
    return dim < rotary_pairs ? x0 * cos_theta - x1 * sin_theta
                              : x0 * sin_theta + x1 * cos_theta;
}

__device__ __forceinline__ int mrope_position_for_pair(
    int pair,
    int position_t,
    int position_h,
    int position_w,
    int section_t,
    int section_h,
    int section_w,
    int section_extra,
    int interleaved
) {
    const int section_dims = section_t + section_h + section_w + section_extra;
    if (section_dims <= 0) {
        return position_t;
    }
    const int sector = pair % section_dims;
    if (interleaved != 0) {
        if (sector % 3 == 1 && sector < 3 * section_h) {
            return position_h;
        }
        if (sector % 3 == 2 && sector < 3 * section_w) {
            return position_w;
        }
        if (sector % 3 == 0 && sector < 3 * section_t) {
            return position_t;
        }
        return 0;
    }
    if (sector < section_t) {
        return position_t;
    }
    if (sector < section_t + section_h) {
        return position_h;
    }
    if (sector < section_t + section_h + section_w) {
        return position_w;
    }
    return 0;
}

__device__ __forceinline__ float rope_neox_mrope_component(
    const float *values,
    int dim,
    int head_dim,
    int rotary_dim,
    int position_t,
    int position_h,
    int position_w,
    int section_t,
    int section_h,
    int section_w,
    int section_extra,
    int interleaved,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale
) {
    if (dim >= head_dim) {
        return 0.0f;
    }
    const int bounded_rotary_dim = max(min(rotary_dim, head_dim), 2);
    if (dim >= bounded_rotary_dim) {
        return values[dim];
    }

    const int rotary_pairs = bounded_rotary_dim / 2;
    const int pair = dim < rotary_pairs ? dim : dim - rotary_pairs;
    const int index0 = pair;
    const int index1 = pair + rotary_pairs;
    const int position = mrope_position_for_pair(
        pair,
        position_t,
        position_h,
        position_w,
        section_t,
        section_h,
        section_w,
        section_extra,
        interleaved
    );
    const float theta_base =
        static_cast<float>(position) * powf(theta_scale, static_cast<float>(pair));
    float cos_theta = 0.0f;
    float sin_theta = 0.0f;
    rope_yarn(
        theta_base,
        freq_scale,
        corr_low,
        corr_high,
        pair * 2,
        ext_factor,
        theta_scale,
        cos_theta,
        sin_theta
    );
    const float x0 = values[index0];
    const float x1 = values[index1];
    return dim < rotary_pairs ? x0 * cos_theta - x1 * sin_theta
                              : x0 * sin_theta + x1 * cos_theta;
}

template <typename T>
__device__ __forceinline__ float load_attention_value(const T *values, int index);

template <>
__device__ __forceinline__ float load_attention_value<float>(const float *values, int index) {
    return values[index];
}

template <>
__device__ __forceinline__ float load_attention_value<__nv_bfloat16>(
    const __nv_bfloat16 *values,
    int index
) {
    return __bfloat162float(values[index]);
}

template <typename T>
__device__ __forceinline__ void store_attention_value(T *values, int index, float value);

template <>
__device__ __forceinline__ void store_attention_value<float>(
    float *values,
    int index,
    float value
) {
    values[index] = value;
}

template <>
__device__ __forceinline__ void store_attention_value<__nv_bfloat16>(
    __nv_bfloat16 *values,
    int index,
    float value
) {
    values[index] = __float2bfloat16(value);
}

template <typename QueryT, typename KeyT, typename ValueT, typename OutputT>
__global__ void attention_causal_sequence_kernel(
    const QueryT *query,
    const KeyT *key,
    const ValueT *value,
    int batch_size,
    int head_count,
    int kv_head_count,
    int sequence_length,
    int head_dim,
    OutputT *output
) {
    const int head_index = static_cast<int>(blockIdx.x);
    const int position = static_cast<int>(blockIdx.y);
    const int batch_index = static_cast<int>(blockIdx.z);
    if (batch_index >= batch_size || head_index >= head_count || position >= sequence_length) {
        return;
    }

    __shared__ float logits[kAttentionMaxPositions];
    __shared__ float weights[kAttentionMaxPositions];
    __shared__ float reduction_scratch[kAttentionBlockSize];

    const int group_size = max(head_count / max(kv_head_count, 1), 1);
    const int kv_head = min(head_index / group_size, kv_head_count - 1);
    const float scale = rsqrtf(static_cast<float>(head_dim));
    const int query_head_offset =
        ((batch_index * head_count + head_index) * sequence_length + position) * head_dim;
    const QueryT *query_head = query + query_head_offset;

    for (int token_index = static_cast<int>(threadIdx.x); token_index <= position;
         token_index += blockDim.x) {
        const int key_head_offset =
            ((batch_index * kv_head_count + kv_head) * sequence_length + token_index) * head_dim;
        const KeyT *key_head = key + key_head_offset;
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += load_attention_value(query_head, dim) * load_attention_value(key_head, dim);
        }
        logits[token_index] = dot * scale;
    }
    __syncthreads();

    float local_max = -INFINITY;
    for (int index = static_cast<int>(threadIdx.x); index <= position; index += blockDim.x) {
        local_max = fmaxf(local_max, logits[index]);
    }
    const float max_value = reduce_block_max(local_max, reduction_scratch);

    float local_denom = 0.0f;
    for (int index = static_cast<int>(threadIdx.x); index <= position; index += blockDim.x) {
        const float weight = expf(logits[index] - max_value);
        weights[index] = weight;
        local_denom += weight;
    }
    const float denom = fmaxf(reduce_block_sum(local_denom, reduction_scratch), 1e-20f);
    for (int index = static_cast<int>(threadIdx.x); index <= position; index += blockDim.x) {
        weights[index] /= denom;
    }
    __syncthreads();

    const int output_head_offset =
        ((batch_index * head_count + head_index) * sequence_length + position) * head_dim;
    OutputT *output_head = output + output_head_offset;
    for (int dim = static_cast<int>(threadIdx.x); dim < head_dim; dim += blockDim.x) {
        float sum = 0.0f;
        for (int token_index = 0; token_index <= position; ++token_index) {
            const int value_head_offset =
                ((batch_index * kv_head_count + kv_head) * sequence_length + token_index) *
                head_dim;
            const ValueT *value_head = value + value_head_offset;
            sum += load_attention_value(value_head, dim) * weights[token_index];
        }
        store_attention_value(output_head, dim, sum);
    }
}

__global__ void attention_causal_row_softmax_in_place_f32_kernel(
    float *logits,
    int row_count,
    int sequence_length,
    float scale
) {
    const int row_index = static_cast<int>(blockIdx.x);
    if (row_index >= row_count) {
        return;
    }

    __shared__ float reduction_scratch[kAttentionBlockSize];

    const int position = row_index % sequence_length;
    float *row = logits + static_cast<size_t>(row_index) * static_cast<size_t>(sequence_length);

    float local_max = -INFINITY;
    for (int token_index = static_cast<int>(threadIdx.x); token_index <= position;
         token_index += blockDim.x) {
        local_max = fmaxf(local_max, row[token_index] * scale);
    }
    const float max_value = reduce_block_max(local_max, reduction_scratch);

    float local_denom = 0.0f;
    for (int token_index = static_cast<int>(threadIdx.x); token_index <= position;
         token_index += blockDim.x) {
        local_denom += expf(row[token_index] * scale - max_value);
    }
    const float denom = fmaxf(reduce_block_sum(local_denom, reduction_scratch), 1e-20f);

    for (int token_index = static_cast<int>(threadIdx.x); token_index < sequence_length;
         token_index += blockDim.x) {
        if (token_index <= position) {
            row[token_index] = expf(row[token_index] * scale - max_value) / denom;
        } else {
            row[token_index] = 0.0f;
        }
    }
}

__global__ void attention_causal_row_softmax_backward_in_place_f32_kernel(
    const float *probabilities,
    float *grad_probabilities,
    int row_count,
    int sequence_length,
    float post_scale
) {
    const int row_index = static_cast<int>(blockIdx.x);
    if (row_index >= row_count) {
        return;
    }

    __shared__ float reduction_scratch[kAttentionBlockSize];

    const int position = row_index % sequence_length;
    const float *probability_row =
        probabilities + static_cast<size_t>(row_index) * static_cast<size_t>(sequence_length);
    float *grad_row =
        grad_probabilities + static_cast<size_t>(row_index) * static_cast<size_t>(sequence_length);

    float local_dot = 0.0f;
    for (int token_index = static_cast<int>(threadIdx.x); token_index <= position;
         token_index += blockDim.x) {
        local_dot += probability_row[token_index] * grad_row[token_index];
    }
    const float dot = reduce_block_sum(local_dot, reduction_scratch);

    for (int token_index = static_cast<int>(threadIdx.x); token_index < sequence_length;
         token_index += blockDim.x) {
        if (token_index <= position) {
            grad_row[token_index] =
                post_scale * probability_row[token_index] * (grad_row[token_index] - dot);
        } else {
            grad_row[token_index] = 0.0f;
        }
    }
}

template <typename QueryT, typename KeyT, typename ValueT, typename GradOutputT>
__global__ void attention_causal_sequence_backward_to_f32_kernel(
    const QueryT *query,
    const KeyT *key,
    const ValueT *value,
    const GradOutputT *grad_output,
    int batch_size,
    int head_count,
    int kv_head_count,
    int sequence_length,
    int head_dim,
    float *query_gradient,
    float *key_gradient,
    float *value_gradient
) {
    const int head_index = static_cast<int>(blockIdx.x);
    const int position = static_cast<int>(blockIdx.y);
    const int batch_index = static_cast<int>(blockIdx.z);
    if (batch_index >= batch_size || head_index >= head_count || position >= sequence_length) {
        return;
    }

    __shared__ float logits[kAttentionMaxPositions];
    __shared__ float weights[kAttentionMaxPositions];
    __shared__ float grad_scores[kAttentionMaxPositions];
    __shared__ float reduction_scratch[kAttentionBlockSize];

    const int group_size = max(head_count / max(kv_head_count, 1), 1);
    const int kv_head = min(head_index / group_size, kv_head_count - 1);
    const float scale = rsqrtf(static_cast<float>(head_dim));
    const int query_row_offset =
        ((batch_index * head_count + head_index) * sequence_length + position) * head_dim;
    for (int token_index = static_cast<int>(threadIdx.x); token_index <= position;
         token_index += blockDim.x) {
        const int key_row_offset =
            ((batch_index * kv_head_count + kv_head) * sequence_length + token_index) * head_dim;
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += load_attention_value(query, query_row_offset + dim) *
                   load_attention_value(key, key_row_offset + dim);
        }
        logits[token_index] = dot * scale;
    }
    __syncthreads();

    float local_max = -INFINITY;
    for (int token_index = static_cast<int>(threadIdx.x); token_index <= position;
         token_index += blockDim.x) {
        local_max = fmaxf(local_max, logits[token_index]);
    }
    const float max_value = reduce_block_max(local_max, reduction_scratch);

    float local_denom = 0.0f;
    for (int token_index = static_cast<int>(threadIdx.x); token_index <= position;
         token_index += blockDim.x) {
        const float weight = expf(logits[token_index] - max_value);
        weights[token_index] = weight;
        local_denom += weight;
    }
    const float denom = fmaxf(reduce_block_sum(local_denom, reduction_scratch), 1e-20f);
    for (int token_index = static_cast<int>(threadIdx.x); token_index <= position;
         token_index += blockDim.x) {
        weights[token_index] /= denom;
    }
    __syncthreads();

    float local_weighted_grad_sum = 0.0f;
    for (int token_index = static_cast<int>(threadIdx.x); token_index <= position;
         token_index += blockDim.x) {
        const int value_row_offset =
            ((batch_index * kv_head_count + kv_head) * sequence_length + token_index) * head_dim;
        float grad_weight = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            grad_weight += load_attention_value(grad_output, query_row_offset + dim) *
                           load_attention_value(value, value_row_offset + dim);
        }
        logits[token_index] = grad_weight;
        local_weighted_grad_sum += weights[token_index] * grad_weight;
    }
    const float weighted_grad_sum =
        reduce_block_sum(local_weighted_grad_sum, reduction_scratch);
    for (int token_index = static_cast<int>(threadIdx.x); token_index <= position;
         token_index += blockDim.x) {
        grad_scores[token_index] =
            weights[token_index] * (logits[token_index] - weighted_grad_sum);
    }
    __syncthreads();

    for (int dim = static_cast<int>(threadIdx.x); dim < head_dim; dim += blockDim.x) {
        float query_grad_value = 0.0f;
        for (int token_index = 0; token_index <= position; ++token_index) {
            const int key_row_offset =
                ((batch_index * kv_head_count + kv_head) * sequence_length + token_index) *
                head_dim;
            query_grad_value += grad_scores[token_index] * scale *
                                load_attention_value(key, key_row_offset + dim);
        }
        query_gradient[query_row_offset + dim] = query_grad_value;
    }

    for (int token_index = 0; token_index <= position; ++token_index) {
        const int kv_row_offset =
            ((batch_index * kv_head_count + kv_head) * sequence_length + token_index) * head_dim;
        const float key_scale = grad_scores[token_index] * scale;
        const float value_scale = weights[token_index];
        for (int dim = static_cast<int>(threadIdx.x); dim < head_dim; dim += blockDim.x) {
            atomicAdd(
                &key_gradient[kv_row_offset + dim],
                key_scale * load_attention_value(query, query_row_offset + dim)
            );
            atomicAdd(
                &value_gradient[kv_row_offset + dim],
                value_scale * load_attention_value(grad_output, query_row_offset + dim)
            );
        }
    }
}

__global__ void attention_decode_kernel(
    const float *query,
    int query_offset,
    const float *current_key,
    int key_offset,
    const float *current_value,
    int value_offset,
    const float *cache_keys,
    const float *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    const float *attention_sinks,
    float *output
) {
    const int head_index = blockIdx.x;
    if (head_index >= head_count) {
        return;
    }

    __shared__ float logits[kAttentionMaxPositions];
    __shared__ float weights[kAttentionMaxPositions];
    __shared__ float reduction_scratch[kAttentionBlockSize];

    int window_tokens = past_tokens;
    if (sliding_window > 0 && window_tokens > sliding_window) {
        window_tokens = sliding_window;
    }
    if (window_tokens > kAttentionMaxPositions - 1) {
        window_tokens = kAttentionMaxPositions - 1;
    }
    const int start = past_tokens - window_tokens;
    const int group_size = max(head_count / max(kv_head_count, 1), 1);
    const int kv_head = min(head_index / group_size, kv_head_count - 1);
    const float scale = rsqrtf(static_cast<float>(head_dim));
    const float *query_head = query + query_offset + head_index * head_dim;

    for (int token_index = static_cast<int>(threadIdx.x); token_index < window_tokens;
         token_index += blockDim.x) {
        const float *key_head =
            cache_keys + (start + token_index) * cache_width + layer_offset + kv_head * head_dim;
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_head[dim] * key_head[dim];
        }
        logits[token_index] = dot * scale;
    }
    if (threadIdx.x == 0) {
        float dot = 0.0f;
        const float *current_key_head = current_key + key_offset + kv_head * head_dim;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_head[dim] * current_key_head[dim];
        }
        logits[window_tokens] = dot * scale;
    }
    __syncthreads();

    float local_max = -INFINITY;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        local_max = fmaxf(local_max, logits[index]);
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_max = fmaxf(local_max, attention_sinks[head_index]);
    }
    const float max_value = reduce_block_max(local_max, reduction_scratch);

    float local_denom = 0.0f;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        const float weight = expf(logits[index] - max_value);
        weights[index] = weight;
        local_denom += weight;
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_denom += expf(attention_sinks[head_index] - max_value);
    }
    const float denom = reduce_block_sum(local_denom, reduction_scratch);
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        weights[index] = denom != 0.0f ? weights[index] / denom : 0.0f;
    }
    __syncthreads();

    if (threadIdx.x < head_dim) {
        float sum = 0.0f;
        for (int index = 0; index < window_tokens; ++index) {
            const float *value_head =
                cache_values + (start + index) * cache_width + layer_offset + kv_head * head_dim;
            sum += value_head[threadIdx.x] * weights[index];
        }
        const float *current_value_head = current_value + value_offset + kv_head * head_dim;
        sum += current_value_head[threadIdx.x] * weights[window_tokens];
        output[head_index * head_dim + threadIdx.x] = sum;
    }
}

__global__ void attention_decode_rope_cache_kernel(
    const float *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    float *cache_keys,
    float *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const float *attention_sinks,
    float *output
) {
    const int head_index = blockIdx.x;
    if (head_index >= head_count) {
        return;
    }

    extern __shared__ float head_state[];
    float *query_rotated = head_state;
    float *current_key_rotated = head_state + head_dim;

    __shared__ float logits[kAttentionMaxPositions];
    __shared__ float weights[kAttentionMaxPositions];

    int window_tokens = past_tokens;
    if (sliding_window > 0 && window_tokens > sliding_window) {
        window_tokens = sliding_window;
    }
    if (window_tokens > kAttentionMaxPositions - 1) {
        window_tokens = kAttentionMaxPositions - 1;
    }
    const int start = past_tokens - window_tokens;
    const int group_size = max(head_count / max(kv_head_count, 1), 1);
    const int kv_head = min(head_index / group_size, kv_head_count - 1);
    const bool cache_writer = (head_index % group_size) == 0;
    const int cache_token_offset = past_tokens * cache_width + layer_offset + kv_head * head_dim;
    const float scale = rsqrtf(static_cast<float>(head_dim));

    const float *query_head = qkv + query_offset + head_index * head_dim;
    const float *current_key_head = qkv + key_offset + kv_head * head_dim;
    const float *current_value_head = qkv + value_offset + kv_head * head_dim;

    for (int dim = threadIdx.x; dim < head_dim; dim += blockDim.x) {
        query_rotated[dim] = rope_neox_component(
            query_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        const float rotated_key = rope_neox_component(
            current_key_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        current_key_rotated[dim] = rotated_key;
        if (cache_writer) {
            cache_keys[cache_token_offset + dim] = rotated_key;
            cache_values[cache_token_offset + dim] = current_value_head[dim];
        }
    }
    __syncthreads();

    for (int token_index = static_cast<int>(threadIdx.x); token_index <= window_tokens;
         token_index += blockDim.x) {
        const bool current = token_index == window_tokens;
        const float *key_head = current
            ? current_key_rotated
            : cache_keys + (start + token_index) * cache_width + layer_offset + kv_head * head_dim;
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_rotated[dim] * key_head[dim];
        }
        logits[token_index] = dot * scale;
    }
    __syncthreads();

    if (threadIdx.x == 0) {
        float max_value = logits[0];
        for (int index = 1; index <= window_tokens; ++index) {
            max_value = fmaxf(max_value, logits[index]);
        }
        if (attention_sinks != nullptr) {
            max_value = fmaxf(max_value, attention_sinks[head_index]);
        }

        float denom = 0.0f;
        for (int index = 0; index <= window_tokens; ++index) {
            weights[index] = expf(logits[index] - max_value);
            denom += weights[index];
        }
        if (attention_sinks != nullptr) {
            denom += expf(attention_sinks[head_index] - max_value);
        }
        if (denom != 0.0f) {
            for (int index = 0; index <= window_tokens; ++index) {
                weights[index] /= denom;
            }
        }
    }
    __syncthreads();

    if (threadIdx.x < head_dim) {
        float sum = 0.0f;
        for (int index = 0; index < window_tokens; ++index) {
            const float *value_head =
                cache_values + (start + index) * cache_width + layer_offset + kv_head * head_dim;
            sum += value_head[threadIdx.x] * weights[index];
        }
        sum += current_value_head[threadIdx.x] * weights[window_tokens];
        output[head_index * head_dim + threadIdx.x] = sum;
    }
}

__global__ void attention_decode_rope_cache_f16_kv_kernel(
    const float *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    __half *cache_keys,
    __half *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position_t,
    int position_h,
    int position_w,
    int section_t,
    int section_h,
    int section_w,
    int section_extra,
    int mrope_interleaved,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const float *attention_sinks,
    float *output
) {
    const int head_index = blockIdx.x;
    if (head_index >= head_count) {
        return;
    }

    extern __shared__ float head_state[];
    float *query_rotated = head_state;
    float *current_key_rotated = head_state + head_dim;

    __shared__ float logits[kAttentionMaxPositions];
    __shared__ float weights[kAttentionMaxPositions];
    __shared__ float reduction_scratch[kAttentionBlockSize];

    int window_tokens = past_tokens;
    if (sliding_window > 0 && window_tokens > sliding_window) {
        window_tokens = sliding_window;
    }
    if (window_tokens > kAttentionMaxPositions - 1) {
        window_tokens = kAttentionMaxPositions - 1;
    }
    const int start = past_tokens - window_tokens;
    const int group_size = max(head_count / max(kv_head_count, 1), 1);
    const int kv_head = min(head_index / group_size, kv_head_count - 1);
    const bool cache_writer = (head_index % group_size) == 0;
    const int cache_token_offset = past_tokens * cache_width + layer_offset + kv_head * head_dim;
    const float scale = rsqrtf(static_cast<float>(head_dim));

    const float *query_head = qkv + query_offset + head_index * head_dim;
    const float *current_key_head = qkv + key_offset + kv_head * head_dim;
    const float *current_value_head = qkv + value_offset + kv_head * head_dim;

    for (int dim = threadIdx.x; dim < head_dim; dim += blockDim.x) {
        query_rotated[dim] = rope_neox_mrope_component(
            query_head,
            dim,
            head_dim,
            rotary_dim,
            position_t,
            position_h,
            position_w,
            section_t,
            section_h,
            section_w,
            section_extra,
            mrope_interleaved,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        const float rotated_key = rope_neox_mrope_component(
            current_key_head,
            dim,
            head_dim,
            rotary_dim,
            position_t,
            position_h,
            position_w,
            section_t,
            section_h,
            section_w,
            section_extra,
            mrope_interleaved,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        current_key_rotated[dim] = rotated_key;
        if (cache_writer) {
            cache_keys[cache_token_offset + dim] = __float2half_rn(rotated_key);
            cache_values[cache_token_offset + dim] = __float2half_rn(current_value_head[dim]);
        }
    }
    __syncthreads();

    for (int token_index = static_cast<int>(threadIdx.x); token_index < window_tokens;
         token_index += blockDim.x) {
        const __half *key_head =
            cache_keys + (start + token_index) * cache_width + layer_offset + kv_head * head_dim;
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_rotated[dim] * __half2float(key_head[dim]);
        }
        logits[token_index] = dot * scale;
    }
    if (threadIdx.x == 0) {
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_rotated[dim] * current_key_rotated[dim];
        }
        logits[window_tokens] = dot * scale;
    }
    __syncthreads();

    float local_max = -INFINITY;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        local_max = fmaxf(local_max, logits[index]);
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_max = fmaxf(local_max, attention_sinks[head_index]);
    }
    const float max_value = reduce_block_max(local_max, reduction_scratch);

    float local_denom = 0.0f;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        const float weight = expf(logits[index] - max_value);
        weights[index] = weight;
        local_denom += weight;
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_denom += expf(attention_sinks[head_index] - max_value);
    }
    const float denom = reduce_block_sum(local_denom, reduction_scratch);
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        weights[index] = denom != 0.0f ? weights[index] / denom : 0.0f;
    }
    __syncthreads();

    if (threadIdx.x < head_dim) {
        float sum = 0.0f;
        for (int index = 0; index < window_tokens; ++index) {
            const __half *value_head =
                cache_values + (start + index) * cache_width + layer_offset + kv_head * head_dim;
            sum += __half2float(value_head[threadIdx.x]) * weights[index];
        }
        sum += current_value_head[threadIdx.x] * weights[window_tokens];
        output[head_index * head_dim + threadIdx.x] = sum;
    }
}

__global__ void attention_decode_rope_cache_f16_kv_graph_kernel(
    const float *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    __half *cache_keys,
    __half *cache_values,
    int cache_width,
    int layer_offset,
    const int *decode_params,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const float *attention_sinks,
    float *output
) {
    const int past_tokens = decode_params[0];
    const int position = decode_params[1];
    const int head_index = blockIdx.x;
    if (head_index >= head_count) {
        return;
    }

    extern __shared__ float head_state[];
    float *query_rotated = head_state;
    float *current_key_rotated = head_state + head_dim;

    __shared__ float logits[kAttentionMaxPositions];
    __shared__ float weights[kAttentionMaxPositions];
    __shared__ float reduction_scratch[kAttentionBlockSize];

    int window_tokens = past_tokens;
    if (sliding_window > 0 && window_tokens > sliding_window) {
        window_tokens = sliding_window;
    }
    if (window_tokens > kAttentionMaxPositions - 1) {
        window_tokens = kAttentionMaxPositions - 1;
    }
    const int start = past_tokens - window_tokens;
    const int group_size = max(head_count / max(kv_head_count, 1), 1);
    const int kv_head = min(head_index / group_size, kv_head_count - 1);
    const bool cache_writer = (head_index % group_size) == 0;
    const int cache_token_offset = past_tokens * cache_width + layer_offset + kv_head * head_dim;
    const float scale = rsqrtf(static_cast<float>(head_dim));

    const float *query_head = qkv + query_offset + head_index * head_dim;
    const float *current_key_head = qkv + key_offset + kv_head * head_dim;
    const float *current_value_head = qkv + value_offset + kv_head * head_dim;

    for (int dim = threadIdx.x; dim < head_dim; dim += blockDim.x) {
        query_rotated[dim] = rope_neox_component(
            query_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        const float rotated_key = rope_neox_component(
            current_key_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        current_key_rotated[dim] = rotated_key;
        if (cache_writer) {
            cache_keys[cache_token_offset + dim] = __float2half_rn(rotated_key);
            cache_values[cache_token_offset + dim] = __float2half_rn(current_value_head[dim]);
        }
    }
    __syncthreads();

    for (int token_index = static_cast<int>(threadIdx.x); token_index < window_tokens;
         token_index += blockDim.x) {
        const __half *key_head =
            cache_keys + (start + token_index) * cache_width + layer_offset + kv_head * head_dim;
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_rotated[dim] * __half2float(key_head[dim]);
        }
        logits[token_index] = dot * scale;
    }
    if (threadIdx.x == 0) {
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_rotated[dim] * current_key_rotated[dim];
        }
        logits[window_tokens] = dot * scale;
    }
    __syncthreads();

    float local_max = -INFINITY;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        local_max = fmaxf(local_max, logits[index]);
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_max = fmaxf(local_max, attention_sinks[head_index]);
    }
    const float max_value = reduce_block_max(local_max, reduction_scratch);

    float local_denom = 0.0f;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        const float weight = expf(logits[index] - max_value);
        weights[index] = weight;
        local_denom += weight;
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_denom += expf(attention_sinks[head_index] - max_value);
    }
    const float denom = reduce_block_sum(local_denom, reduction_scratch);
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        weights[index] = denom != 0.0f ? weights[index] / denom : 0.0f;
    }
    __syncthreads();

    if (threadIdx.x < head_dim) {
        float sum = 0.0f;
        for (int index = 0; index < window_tokens; ++index) {
            const __half *value_head =
                cache_values + (start + index) * cache_width + layer_offset + kv_head * head_dim;
            sum += __half2float(value_head[threadIdx.x]) * weights[index];
        }
        sum += current_value_head[threadIdx.x] * weights[window_tokens];
        output[head_index * head_dim + threadIdx.x] = sum;
    }
}

__device__ __forceinline__ int attention_fa3_active_splits(int context_tokens) {
    if (context_tokens <= 512) {
        return 1;
    }
    if (context_tokens <= 2048) {
        return 2;
    }
    if (context_tokens <= 8192) {
        return 4;
    }
    return kAttentionFa3MaxSplits;
}

__global__ void attention_decode_rope_cache_f16_kv_graph_fa3_split_kernel(
    const float *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    __half *cache_keys,
    __half *cache_values,
    int cache_width,
    int layer_offset,
    const int *decode_params,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const float *attention_sinks,
    float *partial_output,
    float *partial_max,
    float *partial_sum
) {
    const int kv_head = blockIdx.x;
    const int split_idx = blockIdx.y;
    const int tid = static_cast<int>(threadIdx.x);
    const int warp_id = tid / kWarpSize;
    const int lane_id = tid % kWarpSize;
    const int past_tokens = decode_params[0];
    const int position = decode_params[1];

    if (kv_head >= kv_head_count || head_count <= 0 || kv_head_count <= 0 || head_dim <= 0 ||
        head_dim > kAttentionFa3MaxHeadDim || head_count % kv_head_count != 0) {
        return;
    }

    const int heads_per_group = head_count / kv_head_count;
    if (heads_per_group <= 0 || heads_per_group > kAttentionFa3MaxHeadsPerGroup) {
        return;
    }

    int window_tokens = past_tokens;
    if (sliding_window > 0 && window_tokens > sliding_window) {
        window_tokens = sliding_window;
    }
    const int context_tokens = window_tokens + 1;
    const int active_splits = attention_fa3_active_splits(context_tokens);

    const int group_head = kv_head * heads_per_group + warp_id;
    const bool active_warp = warp_id < heads_per_group && group_head < head_count;
    const int workspace_index = active_warp ? (split_idx * head_count) + group_head : 0;
    const int output_base = workspace_index * head_dim;
    const int dims_per_lane = (head_dim + kWarpSize - 1) / kWarpSize;
    float output_acc[kAttentionFa3MaxDimsPerLane];
    #pragma unroll
    for (int index = 0; index < kAttentionFa3MaxDimsPerLane; ++index) {
        output_acc[index] = 0.0f;
    }

    if (split_idx >= active_splits) {
        if (active_warp && lane_id == 0) {
            partial_max[workspace_index] = -FLT_MAX;
            partial_sum[workspace_index] = 0.0f;
        }
        if (active_warp) {
            for (int lane_offset = 0; lane_offset < dims_per_lane; ++lane_offset) {
                const int dim = lane_id + lane_offset * kWarpSize;
                if (dim < head_dim) {
                    partial_output[output_base + dim] = 0.0f;
                }
            }
        }
        return;
    }

    extern __shared__ float fa3_shared[];
    float *query_rotated = fa3_shared;
    float *current_key_rotated = query_rotated + heads_per_group * head_dim;

    const float *query_head = qkv + query_offset + group_head * head_dim;
    const float *current_key_head = qkv + key_offset + kv_head * head_dim;
    const float *current_value_head = qkv + value_offset + kv_head * head_dim;
    const int cache_token_offset = past_tokens * cache_width + layer_offset + kv_head * head_dim;

    if (active_warp) {
        for (int dim = lane_id; dim < head_dim; dim += kWarpSize) {
            query_rotated[warp_id * head_dim + dim] = rope_neox_component(
                query_head,
                dim,
                head_dim,
                rotary_dim,
                position,
                freq_scale,
                ext_factor,
                corr_low,
                corr_high,
                theta_scale
            );
        }
    }
    if (warp_id == 0) {
        for (int dim = lane_id; dim < head_dim; dim += kWarpSize) {
            const float rotated_key = rope_neox_component(
                current_key_head,
                dim,
                head_dim,
                rotary_dim,
                position,
                freq_scale,
                ext_factor,
                corr_low,
                corr_high,
                theta_scale
            );
            current_key_rotated[dim] = rotated_key;
            if (split_idx == 0) {
                cache_keys[cache_token_offset + dim] = __float2half_rn(rotated_key);
                cache_values[cache_token_offset + dim] = __float2half_rn(current_value_head[dim]);
            }
        }
    }
    __syncthreads();

    if (!active_warp) {
        return;
    }

    const float *query_rotated_head = query_rotated + warp_id * head_dim;
    const float scale = rsqrtf(static_cast<float>(head_dim));
    const int start = past_tokens - window_tokens;
    const int tokens_per_split = (window_tokens + active_splits - 1) / active_splits;
    const int split_start = start + split_idx * tokens_per_split;
    const int split_end = min(start + window_tokens, split_start + tokens_per_split);
    float row_max = -FLT_MAX;
    float row_sum = 0.0f;

    for (int token = split_start; token < split_end; ++token) {
        const __half *key_head = cache_keys + token * cache_width + layer_offset + kv_head * head_dim;
        float dot = 0.0f;
        for (int dim = lane_id; dim < head_dim; dim += kWarpSize) {
            dot += query_rotated_head[dim] * __half2float(key_head[dim]);
        }
        dot = warp_reduce_sum(dot) * scale;
        dot = __shfl_sync(0xffffffffu, dot, 0, kWarpSize);
        const float next_row_max = fmaxf(row_max, dot);
        const float alpha = row_max == -FLT_MAX ? 0.0f : expf(row_max - next_row_max);
        const float beta = expf(dot - next_row_max);
        #pragma unroll
        for (int lane_offset = 0; lane_offset < kAttentionFa3MaxDimsPerLane; ++lane_offset) {
            const int dim = lane_id + lane_offset * kWarpSize;
            if (dim < head_dim) {
                const __half *value_head =
                    cache_values + token * cache_width + layer_offset + kv_head * head_dim;
                output_acc[lane_offset] =
                    output_acc[lane_offset] * alpha + beta * __half2float(value_head[dim]);
            }
        }
        row_sum = row_sum * alpha + beta;
        row_max = next_row_max;
    }

    if (split_idx == 0) {
        float dot = 0.0f;
        for (int dim = lane_id; dim < head_dim; dim += kWarpSize) {
            dot += query_rotated_head[dim] * current_key_rotated[dim];
        }
        dot = warp_reduce_sum(dot) * scale;
        dot = __shfl_sync(0xffffffffu, dot, 0, kWarpSize);
        float next_row_max = fmaxf(row_max, dot);
        float alpha = row_max == -FLT_MAX ? 0.0f : expf(row_max - next_row_max);
        float beta = expf(dot - next_row_max);
        #pragma unroll
        for (int lane_offset = 0; lane_offset < kAttentionFa3MaxDimsPerLane; ++lane_offset) {
            const int dim = lane_id + lane_offset * kWarpSize;
            if (dim < head_dim) {
                output_acc[lane_offset] =
                    output_acc[lane_offset] * alpha + beta * current_value_head[dim];
            }
        }
        row_sum = row_sum * alpha + beta;
        row_max = next_row_max;
        if (attention_sinks != nullptr) {
            const float sink_value = attention_sinks[group_head];
            next_row_max = fmaxf(row_max, sink_value);
            alpha = row_max == -FLT_MAX ? 0.0f : expf(row_max - next_row_max);
            beta = expf(sink_value - next_row_max);
            #pragma unroll
            for (int lane_offset = 0; lane_offset < kAttentionFa3MaxDimsPerLane; ++lane_offset) {
                const int dim = lane_id + lane_offset * kWarpSize;
                if (dim < head_dim) {
                    output_acc[lane_offset] *= alpha;
                }
            }
            row_sum = row_sum * alpha + beta;
            row_max = next_row_max;
        }
    }

    if (lane_id == 0) {
        partial_max[workspace_index] = row_max;
        partial_sum[workspace_index] = row_sum;
    }
    #pragma unroll
    for (int lane_offset = 0; lane_offset < kAttentionFa3MaxDimsPerLane; ++lane_offset) {
        const int dim = lane_id + lane_offset * kWarpSize;
        if (dim < head_dim) {
            partial_output[output_base + dim] = output_acc[lane_offset];
        }
    }
}

__global__ void attention_decode_rope_cache_f16_kv_graph_fa3_combine_kernel(
    const float *partial_output,
    const float *partial_max,
    const float *partial_sum,
    int head_count,
    int head_dim,
    float *output
) {
    const int head_index = blockIdx.x;
    if (head_index >= head_count) {
        return;
    }

    __shared__ float max_value;
    __shared__ float denom;

    if (threadIdx.x == 0) {
        float local_max = -FLT_MAX;
        for (int split_idx = 0; split_idx < kAttentionFa3MaxSplits; ++split_idx) {
            local_max = fmaxf(local_max, partial_max[split_idx * head_count + head_index]);
        }
        float local_denom = 0.0f;
        for (int split_idx = 0; split_idx < kAttentionFa3MaxSplits; ++split_idx) {
            const int workspace_index = split_idx * head_count + head_index;
            const float split_sum = partial_sum[workspace_index];
            if (split_sum > 0.0f) {
                local_denom += expf(partial_max[workspace_index] - local_max) * split_sum;
            }
        }
        max_value = local_max;
        denom = local_denom;
    }
    __syncthreads();

    for (int dim = static_cast<int>(threadIdx.x); dim < head_dim; dim += blockDim.x) {
        float sum = 0.0f;
        for (int split_idx = 0; split_idx < kAttentionFa3MaxSplits; ++split_idx) {
            const int workspace_index = split_idx * head_count + head_index;
            const float split_sum = partial_sum[workspace_index];
            if (split_sum > 0.0f) {
                sum += expf(partial_max[workspace_index] - max_value) *
                    partial_output[(workspace_index * head_dim) + dim];
            }
        }
        output[head_index * head_dim + dim] = denom > 0.0f ? sum / denom : 0.0f;
    }
}

__global__ void attention_decode_rope_cache_turboquant_kv_kernel(
    const float *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    Q81Block *cache_keys,
    Q81Block *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const float *attention_sinks,
    float *output
) {
    const int head_index = blockIdx.x;
    if (head_index >= head_count || head_dim % kQ81ElementsPerBlock != 0 ||
        cache_width % kQ81ElementsPerBlock != 0 || layer_offset % kQ81ElementsPerBlock != 0) {
        return;
    }

    extern __shared__ float head_state[];
    float *query_rotated = head_state;
    float *current_key_rotated = head_state + head_dim;

    __shared__ float logits[kAttentionMaxPositions];
    __shared__ float weights[kAttentionMaxPositions];
    __shared__ float reduction_scratch[kAttentionBlockSize];

    int window_tokens = past_tokens;
    if (sliding_window > 0 && window_tokens > sliding_window) {
        window_tokens = sliding_window;
    }
    if (window_tokens > kAttentionMaxPositions - 1) {
        window_tokens = kAttentionMaxPositions - 1;
    }
    const int start = past_tokens - window_tokens;
    const int group_size = max(head_count / max(kv_head_count, 1), 1);
    const int kv_head = min(head_index / group_size, kv_head_count - 1);
    const bool cache_writer = (head_index % group_size) == 0;
    const int cache_row_blocks = cache_width / kQ81ElementsPerBlock;
    const int layer_block_offset =
        (layer_offset + kv_head * head_dim) / kQ81ElementsPerBlock;
    const int cache_token_block_offset = past_tokens * cache_row_blocks + layer_block_offset;
    const float scale = rsqrtf(static_cast<float>(head_dim));

    const float *query_head = qkv + query_offset + head_index * head_dim;
    const float *current_key_head = qkv + key_offset + kv_head * head_dim;
    const float *current_value_head = qkv + value_offset + kv_head * head_dim;

    for (int dim = threadIdx.x; dim < head_dim; dim += blockDim.x) {
        query_rotated[dim] = rope_neox_component(
            query_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        current_key_rotated[dim] = rope_neox_component(
            current_key_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
    }
    __syncthreads();

    if (cache_writer && threadIdx.x < head_dim) {
        const int block_offset = static_cast<int>(threadIdx.x) / kQ81ElementsPerBlock;
        const int lane = static_cast<int>(threadIdx.x) % kQ81ElementsPerBlock;
        quantize_q8_1_shared_block(
            current_key_rotated + block_offset * kQ81ElementsPerBlock,
            cache_keys + cache_token_block_offset + block_offset,
            lane
        );
        quantize_q8_1_shared_block(
            current_value_head + block_offset * kQ81ElementsPerBlock,
            cache_values + cache_token_block_offset + block_offset,
            lane
        );
    }
    __syncthreads();

    for (int token_index = static_cast<int>(threadIdx.x); token_index < window_tokens;
         token_index += blockDim.x) {
        const Q81Block *key_blocks =
            cache_keys + (start + token_index) * cache_row_blocks + layer_block_offset;
        logits[token_index] = dot_query_q81_key(query_rotated, key_blocks, head_dim) * scale;
    }
    if (threadIdx.x == 0) {
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_rotated[dim] * current_key_rotated[dim];
        }
        logits[window_tokens] = dot * scale;
    }
    __syncthreads();

    float local_max = -INFINITY;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        local_max = fmaxf(local_max, logits[index]);
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_max = fmaxf(local_max, attention_sinks[head_index]);
    }
    const float max_value = reduce_block_max(local_max, reduction_scratch);

    float local_denom = 0.0f;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        const float weight = expf(logits[index] - max_value);
        weights[index] = weight;
        local_denom += weight;
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_denom += expf(attention_sinks[head_index] - max_value);
    }
    const float denom = reduce_block_sum(local_denom, reduction_scratch);
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        weights[index] = denom != 0.0f ? weights[index] / denom : 0.0f;
    }
    __syncthreads();

    if (threadIdx.x < head_dim) {
        float sum = 0.0f;
        const int block_offset = static_cast<int>(threadIdx.x) / kQ81ElementsPerBlock;
        const int lane = static_cast<int>(threadIdx.x) % kQ81ElementsPerBlock;
        for (int index = 0; index < window_tokens; ++index) {
            const Q81Block *value_blocks =
                cache_values + (start + index) * cache_row_blocks + layer_block_offset;
            sum = fmaf(
                q81_block_value(value_blocks + block_offset, lane),
                weights[index],
                sum
            );
        }
        sum = fmaf(current_value_head[threadIdx.x], weights[window_tokens], sum);
        output[head_index * head_dim + threadIdx.x] = sum;
    }
}

__global__ void attention_decode_rope_cache_turboquant_kv_graph_kernel(
    const float *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    Q81Block *cache_keys,
    Q81Block *cache_values,
    int cache_width,
    int layer_offset,
    const int *decode_params,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const float *attention_sinks,
    float *output
) {
    const int past_tokens = decode_params[0];
    const int position = decode_params[1];
    const int head_index = blockIdx.x;
    if (head_index >= head_count || head_dim % kQ81ElementsPerBlock != 0 ||
        cache_width % kQ81ElementsPerBlock != 0 || layer_offset % kQ81ElementsPerBlock != 0) {
        return;
    }

    extern __shared__ float head_state[];
    float *query_rotated = head_state;
    float *current_key_rotated = head_state + head_dim;

    __shared__ float logits[kAttentionMaxPositions];
    __shared__ float weights[kAttentionMaxPositions];
    __shared__ float reduction_scratch[kAttentionBlockSize];

    int window_tokens = past_tokens;
    if (sliding_window > 0 && window_tokens > sliding_window) {
        window_tokens = sliding_window;
    }
    if (window_tokens > kAttentionMaxPositions - 1) {
        window_tokens = kAttentionMaxPositions - 1;
    }
    const int start = past_tokens - window_tokens;
    const int group_size = max(head_count / max(kv_head_count, 1), 1);
    const int kv_head = min(head_index / group_size, kv_head_count - 1);
    const bool cache_writer = (head_index % group_size) == 0;
    const int cache_row_blocks = cache_width / kQ81ElementsPerBlock;
    const int layer_block_offset =
        (layer_offset + kv_head * head_dim) / kQ81ElementsPerBlock;
    const int cache_token_block_offset = past_tokens * cache_row_blocks + layer_block_offset;
    const float scale = rsqrtf(static_cast<float>(head_dim));

    const float *query_head = qkv + query_offset + head_index * head_dim;
    const float *current_key_head = qkv + key_offset + kv_head * head_dim;
    const float *current_value_head = qkv + value_offset + kv_head * head_dim;

    for (int dim = threadIdx.x; dim < head_dim; dim += blockDim.x) {
        query_rotated[dim] = rope_neox_component(
            query_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        current_key_rotated[dim] = rope_neox_component(
            current_key_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
    }
    __syncthreads();

    if (cache_writer && threadIdx.x < head_dim) {
        const int block_offset = static_cast<int>(threadIdx.x) / kQ81ElementsPerBlock;
        const int lane = static_cast<int>(threadIdx.x) % kQ81ElementsPerBlock;
        quantize_q8_1_shared_block(
            current_key_rotated + block_offset * kQ81ElementsPerBlock,
            cache_keys + cache_token_block_offset + block_offset,
            lane
        );
        quantize_q8_1_shared_block(
            current_value_head + block_offset * kQ81ElementsPerBlock,
            cache_values + cache_token_block_offset + block_offset,
            lane
        );
    }
    __syncthreads();

    for (int token_index = static_cast<int>(threadIdx.x); token_index < window_tokens;
         token_index += blockDim.x) {
        const Q81Block *key_blocks =
            cache_keys + (start + token_index) * cache_row_blocks + layer_block_offset;
        logits[token_index] = dot_query_q81_key(query_rotated, key_blocks, head_dim) * scale;
    }
    if (threadIdx.x == 0) {
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_rotated[dim] * current_key_rotated[dim];
        }
        logits[window_tokens] = dot * scale;
    }
    __syncthreads();

    float local_max = -INFINITY;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        local_max = fmaxf(local_max, logits[index]);
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_max = fmaxf(local_max, attention_sinks[head_index]);
    }
    const float max_value = reduce_block_max(local_max, reduction_scratch);

    float local_denom = 0.0f;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        const float weight = expf(logits[index] - max_value);
        weights[index] = weight;
        local_denom += weight;
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_denom += expf(attention_sinks[head_index] - max_value);
    }
    const float denom = reduce_block_sum(local_denom, reduction_scratch);
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        weights[index] = denom != 0.0f ? weights[index] / denom : 0.0f;
    }
    __syncthreads();

    if (threadIdx.x < head_dim) {
        float sum = 0.0f;
        const int block_offset = static_cast<int>(threadIdx.x) / kQ81ElementsPerBlock;
        const int lane = static_cast<int>(threadIdx.x) % kQ81ElementsPerBlock;
        for (int index = 0; index < window_tokens; ++index) {
            const Q81Block *value_blocks =
                cache_values + (start + index) * cache_row_blocks + layer_block_offset;
            sum = fmaf(
                q81_block_value(value_blocks + block_offset, lane),
                weights[index],
                sum
            );
        }
        sum = fmaf(current_value_head[threadIdx.x], weights[window_tokens], sum);
        output[head_index * head_dim + threadIdx.x] = sum;
    }
}

__global__ void attention_decode_rope_cache_f16_kv_q8_1_kernel(
    const float *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    __half *cache_keys,
    __half *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const float *attention_sinks,
    Q81Block *output_q8_1
) {
    const int head_index = blockIdx.x;
    if (head_index >= head_count || head_dim % kQ81ElementsPerBlock != 0) {
        return;
    }

    extern __shared__ float head_state[];
    float *query_rotated = head_state;
    float *current_key_rotated = head_state + head_dim;

    __shared__ float logits[kAttentionMaxPositions];
    __shared__ float weights[kAttentionMaxPositions];
    __shared__ float reduction_scratch[kAttentionBlockSize];

    int window_tokens = past_tokens;
    if (sliding_window > 0 && window_tokens > sliding_window) {
        window_tokens = sliding_window;
    }
    if (window_tokens > kAttentionMaxPositions - 1) {
        window_tokens = kAttentionMaxPositions - 1;
    }
    const int start = past_tokens - window_tokens;
    const int group_size = max(head_count / max(kv_head_count, 1), 1);
    const int kv_head = min(head_index / group_size, kv_head_count - 1);
    const bool cache_writer = (head_index % group_size) == 0;
    const int cache_token_offset = past_tokens * cache_width + layer_offset + kv_head * head_dim;
    const float scale = rsqrtf(static_cast<float>(head_dim));

    const float *query_head = qkv + query_offset + head_index * head_dim;
    const float *current_key_head = qkv + key_offset + kv_head * head_dim;
    const float *current_value_head = qkv + value_offset + kv_head * head_dim;

    for (int dim = threadIdx.x; dim < head_dim; dim += blockDim.x) {
        query_rotated[dim] = rope_neox_component(
            query_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        const float rotated_key = rope_neox_component(
            current_key_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        current_key_rotated[dim] = rotated_key;
        if (cache_writer) {
            cache_keys[cache_token_offset + dim] = __float2half_rn(rotated_key);
            cache_values[cache_token_offset + dim] = __float2half_rn(current_value_head[dim]);
        }
    }
    __syncthreads();

    for (int token_index = static_cast<int>(threadIdx.x); token_index < window_tokens;
         token_index += blockDim.x) {
        const __half *key_head =
            cache_keys + (start + token_index) * cache_width + layer_offset + kv_head * head_dim;
        logits[token_index] = dot_query_half_key_pairwise(query_rotated, key_head, head_dim) * scale;
    }
    if (threadIdx.x == 0) {
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_rotated[dim] * current_key_rotated[dim];
        }
        logits[window_tokens] = dot * scale;
    }
    __syncthreads();

    float local_max = -INFINITY;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        local_max = fmaxf(local_max, logits[index]);
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_max = fmaxf(local_max, attention_sinks[head_index]);
    }
    const float max_value = reduce_block_max(local_max, reduction_scratch);

    float local_denom = 0.0f;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        const float weight = expf(logits[index] - max_value);
        weights[index] = weight;
        local_denom += weight;
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_denom += expf(attention_sinks[head_index] - max_value);
    }
    const float denom = reduce_block_sum(local_denom, reduction_scratch);
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        weights[index] = denom != 0.0f ? weights[index] / denom : 0.0f;
    }
    __syncthreads();

    const int value_pair_index = static_cast<int>(threadIdx.x);
    if (2 * value_pair_index + 1 < head_dim) {
        float sum0 = 0.0f;
        float sum1 = 0.0f;
        for (int index = 0; index < window_tokens; ++index) {
            const __half *value_head =
                cache_values + (start + index) * cache_width + layer_offset + kv_head * head_dim;
            const __half2 value_pair =
                *reinterpret_cast<const __half2 *>(value_head + 2 * value_pair_index);
            const float2 value_pair_f32 = __half22float2(value_pair);
            sum0 = fmaf(value_pair_f32.x, weights[index], sum0);
            sum1 = fmaf(value_pair_f32.y, weights[index], sum1);
        }
        sum0 = fmaf(current_value_head[2 * value_pair_index], weights[window_tokens], sum0);
        sum1 = fmaf(current_value_head[2 * value_pair_index + 1], weights[window_tokens], sum1);
        current_key_rotated[2 * value_pair_index] = sum0;
        current_key_rotated[2 * value_pair_index + 1] = sum1;
    }
    __syncthreads();

    if (threadIdx.x < head_dim) {
        const int q81_blocks_per_head = head_dim / kQ81ElementsPerBlock;
        const int block_offset = static_cast<int>(threadIdx.x) / kQ81ElementsPerBlock;
        const int lane = static_cast<int>(threadIdx.x) % kQ81ElementsPerBlock;
        quantize_q8_1_shared_block(
            current_key_rotated + block_offset * kQ81ElementsPerBlock,
            output_q8_1 + head_index * q81_blocks_per_head + block_offset,
            lane
        );
    }
}

__global__ void attention_decode_rope_cache_f16_kv_graph_q8_1_kernel(
    const float *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    __half *cache_keys,
    __half *cache_values,
    int cache_width,
    int layer_offset,
    const int *decode_params,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const float *attention_sinks,
    Q81Block *output_q8_1
) {
    const int past_tokens = decode_params[0];
    const int position = decode_params[1];
    const int head_index = blockIdx.x;
    if (head_index >= head_count || head_dim % kQ81ElementsPerBlock != 0) {
        return;
    }

    extern __shared__ float head_state[];
    float *query_rotated = head_state;
    float *current_key_rotated = head_state + head_dim;

    __shared__ float logits[kAttentionMaxPositions];
    __shared__ float weights[kAttentionMaxPositions];
    __shared__ float reduction_scratch[kAttentionBlockSize];

    int window_tokens = past_tokens;
    if (sliding_window > 0 && window_tokens > sliding_window) {
        window_tokens = sliding_window;
    }
    if (window_tokens > kAttentionMaxPositions - 1) {
        window_tokens = kAttentionMaxPositions - 1;
    }
    const int start = past_tokens - window_tokens;
    const int group_size = max(head_count / max(kv_head_count, 1), 1);
    const int kv_head = min(head_index / group_size, kv_head_count - 1);
    const bool cache_writer = (head_index % group_size) == 0;
    const int cache_token_offset = past_tokens * cache_width + layer_offset + kv_head * head_dim;
    const float scale = rsqrtf(static_cast<float>(head_dim));

    const float *query_head = qkv + query_offset + head_index * head_dim;
    const float *current_key_head = qkv + key_offset + kv_head * head_dim;
    const float *current_value_head = qkv + value_offset + kv_head * head_dim;

    for (int dim = threadIdx.x; dim < head_dim; dim += blockDim.x) {
        query_rotated[dim] = rope_neox_component(
            query_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        const float rotated_key = rope_neox_component(
            current_key_head,
            dim,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_low,
            corr_high,
            theta_scale
        );
        current_key_rotated[dim] = rotated_key;
        if (cache_writer) {
            cache_keys[cache_token_offset + dim] = __float2half_rn(rotated_key);
            cache_values[cache_token_offset + dim] = __float2half_rn(current_value_head[dim]);
        }
    }
    __syncthreads();

    for (int token_index = static_cast<int>(threadIdx.x); token_index < window_tokens;
         token_index += blockDim.x) {
        const __half *key_head =
            cache_keys + (start + token_index) * cache_width + layer_offset + kv_head * head_dim;
        logits[token_index] = dot_query_half_key_pairwise(query_rotated, key_head, head_dim) * scale;
    }
    if (threadIdx.x == 0) {
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_rotated[dim] * current_key_rotated[dim];
        }
        logits[window_tokens] = dot * scale;
    }
    __syncthreads();

    float local_max = -INFINITY;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        local_max = fmaxf(local_max, logits[index]);
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_max = fmaxf(local_max, attention_sinks[head_index]);
    }
    const float max_value = reduce_block_max(local_max, reduction_scratch);

    float local_denom = 0.0f;
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        const float weight = expf(logits[index] - max_value);
        weights[index] = weight;
        local_denom += weight;
    }
    if (threadIdx.x == 0 && attention_sinks != nullptr) {
        local_denom += expf(attention_sinks[head_index] - max_value);
    }
    const float denom = reduce_block_sum(local_denom, reduction_scratch);
    for (int index = static_cast<int>(threadIdx.x); index <= window_tokens; index += blockDim.x) {
        weights[index] = denom != 0.0f ? weights[index] / denom : 0.0f;
    }
    __syncthreads();

    const int graph_value_pair_index = static_cast<int>(threadIdx.x);
    if (2 * graph_value_pair_index + 1 < head_dim) {
        float sum0 = 0.0f;
        float sum1 = 0.0f;
        for (int index = 0; index < window_tokens; ++index) {
            const __half *value_head =
                cache_values + (start + index) * cache_width + layer_offset + kv_head * head_dim;
            const __half2 value_pair =
                *reinterpret_cast<const __half2 *>(value_head + 2 * graph_value_pair_index);
            const float2 value_pair_f32 = __half22float2(value_pair);
            sum0 = fmaf(value_pair_f32.x, weights[index], sum0);
            sum1 = fmaf(value_pair_f32.y, weights[index], sum1);
        }
        sum0 = fmaf(current_value_head[2 * graph_value_pair_index], weights[window_tokens], sum0);
        sum1 = fmaf(
            current_value_head[2 * graph_value_pair_index + 1],
            weights[window_tokens],
            sum1
        );
        current_key_rotated[2 * graph_value_pair_index] = sum0;
        current_key_rotated[2 * graph_value_pair_index + 1] = sum1;
    }
    __syncthreads();

    if (threadIdx.x < head_dim) {
        const int q81_blocks_per_head = head_dim / kQ81ElementsPerBlock;
        const int block_offset = static_cast<int>(threadIdx.x) / kQ81ElementsPerBlock;
        const int lane = static_cast<int>(threadIdx.x) % kQ81ElementsPerBlock;
        quantize_q8_1_shared_block(
            current_key_rotated + block_offset * kQ81ElementsPerBlock,
            output_q8_1 + head_index * q81_blocks_per_head + block_offset,
            lane
        );
    }
}

__global__ void router_topk_softmax_kernel(
    const float *weights,
    const float *bias,
    const float *input,
    int expert_count,
    int input_size,
    int top_k,
    int32_t *selected_ids,
    float *selected_weights
) {
    __shared__ float scratch[kBlockSize];
    __shared__ float logits[kMoeMaxExperts];

    expert_count = min(expert_count, kMoeMaxExperts);
    top_k = min(top_k, min(expert_count, kMoeMaxSelected));

    for (int expert = 0; expert < expert_count; ++expert) {
        const float *row = weights + static_cast<size_t>(expert) * static_cast<size_t>(input_size);
        float partial = 0.0f;
        for (int index = threadIdx.x; index < input_size; index += blockDim.x) {
            partial += row[index] * input[index];
        }
        const float reduced = reduce_block_sum(partial, scratch);
        if (threadIdx.x == 0) {
            logits[expert] = reduced + (bias != nullptr ? bias[expert] : 0.0f);
        }
        __syncthreads();
    }

    if (threadIdx.x != 0) {
        return;
    }

    float top_values[kMoeMaxSelected];
    int top_indices[kMoeMaxSelected];
    for (int index = 0; index < top_k; ++index) {
        top_values[index] = -INFINITY;
        top_indices[index] = -1;
    }

    for (int expert = 0; expert < expert_count; ++expert) {
        const float value = logits[expert];
        int insert_at = top_k;
        for (int slot = 0; slot < top_k; ++slot) {
            if (value > top_values[slot] ||
                (value == top_values[slot] && (top_indices[slot] < 0 || expert < top_indices[slot]))) {
                insert_at = slot;
                break;
            }
        }
        if (insert_at >= top_k) {
            continue;
        }
        for (int slot = top_k - 1; slot > insert_at; --slot) {
            top_values[slot] = top_values[slot - 1];
            top_indices[slot] = top_indices[slot - 1];
        }
        top_values[insert_at] = value;
        top_indices[insert_at] = expert;
    }

    float max_value = -INFINITY;
    for (int slot = 0; slot < top_k; ++slot) {
        max_value = fmaxf(max_value, top_values[slot]);
    }

    float denom = 0.0f;
    for (int slot = 0; slot < top_k; ++slot) {
        const float weight = expf(top_values[slot] - max_value);
        selected_weights[slot] = weight;
        denom += weight;
    }
    if (denom != 0.0f) {
        for (int slot = 0; slot < top_k; ++slot) {
            selected_weights[slot] /= denom;
        }
    }
    for (int slot = 0; slot < top_k; ++slot) {
        selected_ids[slot] = top_indices[slot];
    }
}

__global__ void router_logits_topk_delayed_softmax_kernel(
    const float *logits,
    int expert_count,
    int top_k,
    int32_t *selected_ids,
    float *selected_weights
) {
    expert_count = min(expert_count, kMoeMaxExperts);
    top_k = min(top_k, min(expert_count, kMoeMaxSelected));
    if (threadIdx.x != 0) {
        return;
    }

    float top_values[kMoeMaxSelected];
    int top_indices[kMoeMaxSelected];
    for (int index = 0; index < top_k; ++index) {
        top_values[index] = -INFINITY;
        top_indices[index] = -1;
    }

    for (int expert = 0; expert < expert_count; ++expert) {
        const float value = logits[expert];
        int insert_at = top_k;
        for (int slot = 0; slot < top_k; ++slot) {
            if (value > top_values[slot] ||
                (value == top_values[slot] && (top_indices[slot] < 0 || expert < top_indices[slot]))) {
                insert_at = slot;
                break;
            }
        }
        if (insert_at >= top_k) {
            continue;
        }
        for (int slot = top_k - 1; slot > insert_at; --slot) {
            top_values[slot] = top_values[slot - 1];
            top_indices[slot] = top_indices[slot - 1];
        }
        top_values[insert_at] = value;
        top_indices[insert_at] = expert;
    }

    float max_value = -INFINITY;
    for (int slot = 0; slot < top_k; ++slot) {
        max_value = fmaxf(max_value, top_values[slot]);
    }

    float denom = 0.0f;
    for (int slot = 0; slot < top_k; ++slot) {
        const float weight = expf(top_values[slot] - max_value);
        selected_weights[slot] = weight;
        denom += weight;
    }
    if (denom != 0.0f) {
        for (int slot = 0; slot < top_k; ++slot) {
            selected_weights[slot] /= denom;
        }
    }
    for (int slot = 0; slot < top_k; ++slot) {
        selected_ids[slot] = top_indices[slot];
    }
}

template <bool HasBias>
__launch_bounds__(kWarpSize, 1)
__global__ void router_topk_softmax_32_kernel(
    const float *weights,
    const float *bias,
    const float *input,
    int input_size,
    int top_k,
    int32_t *selected_ids,
    float *selected_weights
) {
    const int expert = static_cast<int>(threadIdx.x);
    const float *row = weights + static_cast<size_t>(expert) * static_cast<size_t>(input_size);
    float weight_value = 0.0f;
    for (int index = 0; index < input_size; ++index) {
        weight_value += row[index] * input[index];
    }
    if constexpr (HasBias) {
        weight_value += bias[expert];
    }

    float top_values[kMoeMaxSelected];
    int top_indices[kMoeMaxSelected];
    if (threadIdx.x == 0) {
        for (int index = 0; index < top_k; ++index) {
            top_values[index] = -INFINITY;
            top_indices[index] = -1;
        }
    }

    for (int selection = 0; selection < top_k; ++selection) {
        float max_value = weight_value;
        int max_expert = expert;
#pragma unroll
        for (int mask = kWarpSize / 2; mask > 0; mask >>= 1) {
            const float candidate_value = __shfl_xor_sync(0xffffffffu, max_value, mask, kWarpSize);
            const int candidate_expert = __shfl_xor_sync(0xffffffffu, max_expert, mask, kWarpSize);
            if (candidate_value > max_value ||
                (candidate_value == max_value && candidate_expert < max_expert)) {
                max_value = candidate_value;
                max_expert = candidate_expert;
            }
        }
        if (expert == max_expert) {
            weight_value = -INFINITY;
        }
        if (threadIdx.x == 0) {
            top_values[selection] = max_value;
            top_indices[selection] = max_expert;
        }
    }

    if (threadIdx.x == 0) {
        float max_value = -INFINITY;
        for (int slot = 0; slot < top_k; ++slot) {
            max_value = fmaxf(max_value, top_values[slot]);
        }
        float denom = 0.0f;
        for (int slot = 0; slot < top_k; ++slot) {
            const float selected = expf(top_values[slot] - max_value);
            selected_weights[slot] = selected;
            denom += selected;
        }
        if (denom != 0.0f) {
            for (int slot = 0; slot < top_k; ++slot) {
                selected_weights[slot] /= denom;
            }
        }
        for (int slot = 0; slot < top_k; ++slot) {
            selected_ids[slot] = top_indices[slot];
        }
    }
}

__launch_bounds__(kWarpSize, 1)
__global__ void router_logits_topk_delayed_softmax_32_kernel(
    const float *logits,
    int top_k,
    int32_t *selected_ids,
    float *selected_weights
) {
    const int expert = static_cast<int>(threadIdx.x);
    float weight_value = logits[expert];

    float top_values[kMoeMaxSelected];
    int top_indices[kMoeMaxSelected];
    if (threadIdx.x == 0) {
        for (int index = 0; index < top_k; ++index) {
            top_values[index] = -INFINITY;
            top_indices[index] = -1;
        }
    }

    for (int selection = 0; selection < top_k; ++selection) {
        float max_value = weight_value;
        int max_expert = expert;
#pragma unroll
        for (int mask = kWarpSize / 2; mask > 0; mask >>= 1) {
            const float candidate_value = __shfl_xor_sync(0xffffffffu, max_value, mask, kWarpSize);
            const int candidate_expert = __shfl_xor_sync(0xffffffffu, max_expert, mask, kWarpSize);
            if (candidate_value > max_value ||
                (candidate_value == max_value && candidate_expert < max_expert)) {
                max_value = candidate_value;
                max_expert = candidate_expert;
            }
        }
        if (expert == max_expert) {
            weight_value = -INFINITY;
        }
        if (threadIdx.x == 0) {
            top_values[selection] = max_value;
            top_indices[selection] = max_expert;
        }
    }

    if (threadIdx.x == 0) {
        float max_value = -INFINITY;
        for (int slot = 0; slot < top_k; ++slot) {
            max_value = fmaxf(max_value, top_values[slot]);
        }
        float denom = 0.0f;
        for (int slot = 0; slot < top_k; ++slot) {
            const float selected = expf(top_values[slot] - max_value);
            selected_weights[slot] = selected;
            denom += selected;
        }
        if (denom != 0.0f) {
            for (int slot = 0; slot < top_k; ++slot) {
                selected_weights[slot] /= denom;
            }
        }
        for (int slot = 0; slot < top_k; ++slot) {
            selected_ids[slot] = top_indices[slot];
        }
    }
}

__global__ void moe_gate_up_swiglu_kernel(
    const uint8_t *weights,
    int mode,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const int32_t *selected_ids,
    int selected_count,
    const float *input,
    const float *gate_bias,
    const float *up_bias,
    float *output
) {
    const int row = static_cast<int>(blockIdx.x);
    const int selected_slot = static_cast<int>(blockIdx.y);
    if (selected_slot >= selected_count || row >= gate_rows || row >= up_rows) {
        return;
    }

    const int expert_id = selected_ids[selected_slot];
    const size_t gate_row_offset =
        (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) + static_cast<size_t>(row)) *
        static_cast<size_t>(row_stride);
    const size_t up_row_offset =
        (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) + static_cast<size_t>(gate_rows + row)) *
        static_cast<size_t>(row_stride);
    const uint8_t *gate_row = weights + gate_row_offset;
    const uint8_t *up_row = weights + up_row_offset;

    float gate_partial = 0.0f;
    float up_partial = 0.0f;
    if (mode == 0) {
        const Q80Dot dot{};
        for (int index = threadIdx.x; index < columns; index += blockDim.x) {
            const float in = input[index];
            gate_partial += dot(gate_row, index, in);
            up_partial += dot(up_row, index, in);
        }
    } else {
        const Mxfp4Dot dot{};
        for (int index = threadIdx.x; index < columns; index += blockDim.x) {
            const float in = input[index];
            gate_partial += dot(gate_row, index, in);
            up_partial += dot(up_row, index, in);
        }
    }

    __shared__ float scratch_gate[kBlockSize];
    __shared__ float scratch_up[kBlockSize];
    const float gate_sum = reduce_block_sum(gate_partial, scratch_gate);
    const float up_sum = reduce_block_sum(up_partial, scratch_up);
    if (threadIdx.x != 0) {
        return;
    }

    const float gate = gate_sum + (gate_bias != nullptr ? gate_bias[expert_id * gate_rows + row] : 0.0f);
    const float up = up_sum + (up_bias != nullptr ? up_bias[expert_id * up_rows + row] : 0.0f);
    const float x = fminf(gate, 7.0f);
    const float y = fminf(fmaxf(up, -7.0f), 7.0f);
    const float out_glu = x / (1.0f + expf(1.702f * -x));
    output[selected_slot * gate_rows + row] = out_glu * (y + 1.0f);
}

__global__ void moe_down_aggregate_kernel(
    const uint8_t *weights,
    int mode,
    int row_stride,
    int rows,
    int columns,
    const int32_t *selected_ids,
    const float *selected_weights,
    int selected_count,
    const float *activated,
    const float *bias,
    const float *residual,
    float *output
) {
    const int row = static_cast<int>(blockIdx.x);
    if (row >= rows) {
        return;
    }

    __shared__ float scratch[kBlockSize];
    __shared__ float total;
    if (threadIdx.x == 0) {
        total = 0.0f;
    }
    __syncthreads();

    for (int selected_slot = 0; selected_slot < selected_count; ++selected_slot) {
        const int expert_id = selected_ids[selected_slot];
        const uint8_t *row_weights = weights + (
            static_cast<size_t>(expert_id) * static_cast<size_t>(rows) + static_cast<size_t>(row)
        ) * static_cast<size_t>(row_stride);
        const float *expert_input = activated + static_cast<size_t>(selected_slot) * static_cast<size_t>(columns);

        float partial = 0.0f;
        if (mode == 0) {
            const Q80Dot dot{};
            for (int index = threadIdx.x; index < columns; index += blockDim.x) {
                partial += dot(row_weights, index, expert_input[index]);
            }
        } else {
            const Mxfp4Dot dot{};
            for (int index = threadIdx.x; index < columns; index += blockDim.x) {
                partial += dot(row_weights, index, expert_input[index]);
            }
        }

        const float reduced = reduce_block_sum(partial, scratch);
        if (threadIdx.x == 0) {
            const float expert_value =
                reduced + (bias != nullptr ? bias[expert_id * rows + row] : 0.0f);
            total += expert_value * selected_weights[selected_slot];
        }
        __syncthreads();
    }

    if (threadIdx.x == 0) {
        output[row] = total + (residual != nullptr ? residual[row] : 0.0f);
    }
}

__global__ void moe_gate_up_swiglu_q8_1_kernel(
    const uint8_t *weights,
    int mode,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const int32_t *selected_ids,
    int selected_count,
    const Q81Block *input,
    const float *gate_bias,
    const float *up_bias,
    float *output
) {
    const int row = static_cast<int>(blockIdx.x);
    const int selected_slot = static_cast<int>(blockIdx.y);
    if (selected_slot >= selected_count || row >= gate_rows || row >= up_rows) {
        return;
    }

    const int expert_id = selected_ids[selected_slot];
    const size_t gate_row_offset =
        (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) + static_cast<size_t>(row)) *
        static_cast<size_t>(row_stride);
    const size_t up_row_offset =
        (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) + static_cast<size_t>(gate_rows + row)) *
        static_cast<size_t>(row_stride);
    const uint8_t *gate_row = weights + gate_row_offset;
    const uint8_t *up_row = weights + up_row_offset;
    const int block_count = columns / kQ81ElementsPerBlock;

    float gate_partial = 0.0f;
    float up_partial = 0.0f;
    if (mode == 0) {
        const Q80Q81Dot dot{};
        for (int block_index = static_cast<int>(threadIdx.x); block_index < block_count; block_index += blockDim.x) {
            gate_partial += dot(gate_row, block_index, input);
            up_partial += dot(up_row, block_index, input);
        }
    } else {
        const Mxfp4Q81Dot dot{};
        for (int block_index = static_cast<int>(threadIdx.x); block_index < block_count; block_index += blockDim.x) {
            gate_partial += dot(gate_row, block_index, input);
            up_partial += dot(up_row, block_index, input);
        }
    }

    __shared__ float scratch_gate[kMatvecBlockSize];
    __shared__ float scratch_up[kMatvecBlockSize];
    scratch_gate[threadIdx.x] = gate_partial;
    scratch_up[threadIdx.x] = up_partial;
    __syncthreads();
    for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (threadIdx.x < offset) {
            scratch_gate[threadIdx.x] += scratch_gate[threadIdx.x + offset];
            scratch_up[threadIdx.x] += scratch_up[threadIdx.x + offset];
        }
        __syncthreads();
    }
    if (threadIdx.x != 0) {
        return;
    }

    const float gate = scratch_gate[0] + (gate_bias != nullptr ? gate_bias[expert_id * gate_rows + row] : 0.0f);
    const float up = scratch_up[0] + (up_bias != nullptr ? up_bias[expert_id * up_rows + row] : 0.0f);
    const float x = fminf(gate, 7.0f);
    const float y = fminf(fmaxf(up, -7.0f), 7.0f);
    const float out_glu = x / (1.0f + expf(1.702f * -x));
    output[selected_slot * gate_rows + row] = out_glu * (y + 1.0f);
}

template <int MaxSelected>
__global__ void moe_gate_up_swiglu_q8_1_grouped_selected_kernel(
    const uint8_t *weights,
    int mode,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const int32_t *selected_ids,
    int selected_count,
    const Q81Block *input,
    const float *gate_bias,
    const float *up_bias,
    float *output
) {
    const int row = static_cast<int>(blockIdx.x);
    if (row >= gate_rows || row >= up_rows || selected_count > MaxSelected) {
        return;
    }

    const int block_count = columns / kQ81ElementsPerBlock;
    const uint8_t *gate_rows_ptrs[MaxSelected];
    const uint8_t *up_rows_ptrs[MaxSelected];
    float gate_partial[MaxSelected];
    float up_partial[MaxSelected];

#pragma unroll
    for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
        gate_rows_ptrs[selected_slot] = nullptr;
        up_rows_ptrs[selected_slot] = nullptr;
        gate_partial[selected_slot] = 0.0f;
        up_partial[selected_slot] = 0.0f;
        if (selected_slot < selected_count) {
            const int expert_id = selected_ids[selected_slot];
            const size_t gate_row_offset =
                (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) + static_cast<size_t>(row)) *
                static_cast<size_t>(row_stride);
            const size_t up_row_offset =
                (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) + static_cast<size_t>(gate_rows + row)) *
                static_cast<size_t>(row_stride);
            gate_rows_ptrs[selected_slot] = weights + gate_row_offset;
            up_rows_ptrs[selected_slot] = weights + up_row_offset;
        }
    }

    if (mode == 0) {
        for (int block_index = static_cast<int>(threadIdx.x); block_index < block_count; block_index += blockDim.x) {
            const Q81Block *input_block = input + block_index;
#pragma unroll
            for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
                if (selected_slot >= selected_count) {
                    break;
                }
                const Q80Block *gate_block =
                    reinterpret_cast<const Q80Block *>(gate_rows_ptrs[selected_slot]) + block_index;
                const Q80Block *up_block =
                    reinterpret_cast<const Q80Block *>(up_rows_ptrs[selected_slot]) + block_index;
                gate_partial[selected_slot] += dot_q8_0_q8_1_block(gate_block, input_block);
                up_partial[selected_slot] += dot_q8_0_q8_1_block(up_block, input_block);
            }
        }
    } else {
        for (int block_index = static_cast<int>(threadIdx.x); block_index < block_count; block_index += blockDim.x) {
            const Q81Block *input_block = input + block_index;
#pragma unroll
            for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
                if (selected_slot >= selected_count) {
                    break;
                }
                const Mxfp4Block *gate_block =
                    reinterpret_cast<const Mxfp4Block *>(gate_rows_ptrs[selected_slot]) + block_index;
                const Mxfp4Block *up_block =
                    reinterpret_cast<const Mxfp4Block *>(up_rows_ptrs[selected_slot]) + block_index;
                gate_partial[selected_slot] += dot_mxfp4_q8_1_block(gate_block, input_block);
                up_partial[selected_slot] += dot_mxfp4_q8_1_block(up_block, input_block);
            }
        }
    }

    __shared__ float scratch_gate[MaxSelected][kMatvecBlockSize];
    __shared__ float scratch_up[MaxSelected][kMatvecBlockSize];
#pragma unroll
    for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
        if (selected_slot < selected_count) {
            scratch_gate[selected_slot][threadIdx.x] = gate_partial[selected_slot];
            scratch_up[selected_slot][threadIdx.x] = up_partial[selected_slot];
        }
    }
    __syncthreads();
    for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
        if (threadIdx.x < offset) {
#pragma unroll
            for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
                if (selected_slot >= selected_count) {
                    break;
                }
                scratch_gate[selected_slot][threadIdx.x] += scratch_gate[selected_slot][threadIdx.x + offset];
                scratch_up[selected_slot][threadIdx.x] += scratch_up[selected_slot][threadIdx.x + offset];
            }
        }
        __syncthreads();
    }
    if (threadIdx.x != 0) {
        return;
    }

#pragma unroll
    for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
        if (selected_slot >= selected_count) {
            break;
        }
        const int expert_id = selected_ids[selected_slot];
        const float gate =
            scratch_gate[selected_slot][0] +
            (gate_bias != nullptr ? gate_bias[expert_id * gate_rows + row] : 0.0f);
        const float up =
            scratch_up[selected_slot][0] +
            (up_bias != nullptr ? up_bias[expert_id * up_rows + row] : 0.0f);
        output[selected_slot * gate_rows + row] = swiglu_oai_single(gate, up);
    }
}

template <typename DotFn, int Vdr, int Qi, int MaxSelected, int RowsPerBlock>
__launch_bounds__(kMmvqWarps * kWarpSize, 1)
__global__ void moe_gate_up_swiglu_q8_1_mmvq_grouped_selected_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const int32_t *selected_ids,
    int selected_count,
    const Q81Block *input,
    const float *gate_bias,
    const float *up_bias,
    float *output,
    DotFn dot_fn
) {
    const int row0 = RowsPerBlock * static_cast<int>(blockIdx.x);
    if (selected_count > MaxSelected || row0 >= gate_rows || row0 >= up_rows) {
        return;
    }

    constexpr int blocks_per_iter = Vdr * kMmvqWarps * kWarpSize / Qi;
    const int tid = kWarpSize * static_cast<int>(threadIdx.y) + static_cast<int>(threadIdx.x);

    const uint8_t *gate_rows_ptrs[MaxSelected][RowsPerBlock];
    const uint8_t *up_rows_ptrs[MaxSelected][RowsPerBlock];
    float gate_partial[MaxSelected][RowsPerBlock] = {{0.0f}};
    float up_partial[MaxSelected][RowsPerBlock] = {{0.0f}};
    const int block_count = columns / kQ81ElementsPerBlock;

#pragma unroll
    for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
        const bool selected_valid = selected_slot < selected_count;
        const int expert_id = selected_valid ? selected_ids[selected_slot] : 0;
#pragma unroll
        for (int row_index = 0; row_index < RowsPerBlock; ++row_index) {
            gate_rows_ptrs[selected_slot][row_index] = nullptr;
            up_rows_ptrs[selected_slot][row_index] = nullptr;
            const int row = row0 + row_index;
            if (selected_valid && row < gate_rows && row < up_rows) {
                const size_t gate_row_offset =
                    (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
                     static_cast<size_t>(row)) *
                    static_cast<size_t>(row_stride);
                const size_t up_row_offset =
                    (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
                     static_cast<size_t>(gate_rows + row)) *
                    static_cast<size_t>(row_stride);
                gate_rows_ptrs[selected_slot][row_index] = weights + gate_row_offset;
                up_rows_ptrs[selected_slot][row_index] = weights + up_row_offset;
            }
        }
    }

    for (int block_index = tid / (Qi / Vdr); block_index < block_count; block_index += blocks_per_iter) {
        const int quant_index = Vdr * (tid % (Qi / Vdr));
#pragma unroll
        for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
            if (selected_slot >= selected_count) {
                break;
            }
#pragma unroll
            for (int row_index = 0; row_index < RowsPerBlock; ++row_index) {
                if (gate_rows_ptrs[selected_slot][row_index] != nullptr) {
                    gate_partial[selected_slot][row_index] += dot_fn(
                        gate_rows_ptrs[selected_slot][row_index],
                        input,
                        block_index,
                        block_index,
                        quant_index
                    );
                    up_partial[selected_slot][row_index] += dot_fn(
                        up_rows_ptrs[selected_slot][row_index],
                        input,
                        block_index,
                        block_index,
                        quant_index
                    );
                }
            }
        }
    }

    __shared__ float gate_shared[kMmvqWarps - 1 > 0 ? kMmvqWarps - 1 : 1][MaxSelected][RowsPerBlock][kWarpSize];
    __shared__ float up_shared[kMmvqWarps - 1 > 0 ? kMmvqWarps - 1 : 1][MaxSelected][RowsPerBlock][kWarpSize];
    if (threadIdx.y > 0) {
#pragma unroll
        for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
            if (selected_slot >= selected_count) {
                break;
            }
#pragma unroll
            for (int row_index = 0; row_index < RowsPerBlock; ++row_index) {
                gate_shared[threadIdx.y - 1][selected_slot][row_index][threadIdx.x] =
                    gate_partial[selected_slot][row_index];
                up_shared[threadIdx.y - 1][selected_slot][row_index][threadIdx.x] =
                    up_partial[selected_slot][row_index];
            }
        }
    }
    __syncthreads();

    if (threadIdx.y > 0) {
        return;
    }

#pragma unroll
    for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
        if (selected_slot >= selected_count) {
            break;
        }
#pragma unroll
        for (int row_index = 0; row_index < RowsPerBlock; ++row_index) {
#pragma unroll
            for (int warp_index = 0; warp_index < kMmvqWarps - 1; ++warp_index) {
                gate_partial[selected_slot][row_index] +=
                    gate_shared[warp_index][selected_slot][row_index][threadIdx.x];
                up_partial[selected_slot][row_index] +=
                    up_shared[warp_index][selected_slot][row_index][threadIdx.x];
            }
            gate_partial[selected_slot][row_index] =
                warp_reduce_sum(gate_partial[selected_slot][row_index]);
            up_partial[selected_slot][row_index] =
                warp_reduce_sum(up_partial[selected_slot][row_index]);
        }
    }

    if (threadIdx.x >= RowsPerBlock) {
        return;
    }

    const int row = row0 + static_cast<int>(threadIdx.x);
    if (row >= gate_rows || row >= up_rows) {
        return;
    }

#pragma unroll
    for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
        if (selected_slot >= selected_count) {
            break;
        }
        const int expert_id = selected_ids[selected_slot];
        const float gate =
            gate_partial[selected_slot][threadIdx.x] +
            (gate_bias != nullptr ? gate_bias[expert_id * gate_rows + row] : 0.0f);
        const float up =
            up_partial[selected_slot][threadIdx.x] +
            (up_bias != nullptr ? up_bias[expert_id * up_rows + row] : 0.0f);
        output[selected_slot * gate_rows + row] = swiglu_oai_single(gate, up);
    }
}

template <typename DotFn, int Vdr, int Qi>
__launch_bounds__(4 * kWarpSize, 1)
__global__ void moe_gate_up_swiglu_q8_1_selected4_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const int32_t *selected_ids,
    int selected_count,
    const Q81Block *input,
    const float *gate_bias,
    const float *up_bias,
    float *output,
    DotFn dot_fn
) {
    extern __shared__ unsigned char shared_storage[];
    Q81Block *shared_input = reinterpret_cast<Q81Block *>(shared_storage);

    const int block_count = columns / kQ81ElementsPerBlock;
    const int linear_tid = static_cast<int>(threadIdx.y) * kWarpSize + static_cast<int>(threadIdx.x);
    const int thread_count = 4 * kWarpSize;
    for (int block_index = linear_tid; block_index < block_count; block_index += thread_count) {
        shared_input[block_index] = input[block_index];
    }
    __syncthreads();

    const int row0 = 2 * static_cast<int>(blockIdx.x);
    const int selected_slot = static_cast<int>(threadIdx.y);
    if (row0 >= gate_rows || row0 >= up_rows || selected_slot >= selected_count) {
        return;
    }

    const int expert_id = selected_ids[selected_slot];
    const uint8_t *gate_rows_ptrs[2] = {nullptr, nullptr};
    const uint8_t *up_rows_ptrs[2] = {nullptr, nullptr};
#pragma unroll
    for (int row_offset = 0; row_offset < 2; ++row_offset) {
        const int row = row0 + row_offset;
        if (row < gate_rows && row < up_rows) {
            const size_t gate_row_offset =
                (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
                 static_cast<size_t>(row)) *
                static_cast<size_t>(row_stride);
            const size_t up_row_offset =
                (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
                 static_cast<size_t>(gate_rows + row)) *
                static_cast<size_t>(row_stride);
            gate_rows_ptrs[row_offset] = weights + gate_row_offset;
            up_rows_ptrs[row_offset] = weights + up_row_offset;
        }
    }
    constexpr int warp_blocks_per_iter = Vdr * kWarpSize / Qi;
    const int block_start = static_cast<int>(threadIdx.x) / (Qi / Vdr);
    const int quant_index = Vdr * (static_cast<int>(threadIdx.x) % (Qi / Vdr));

    float gate_sum[2] = {0.0f, 0.0f};
    float up_sum[2] = {0.0f, 0.0f};
    for (int block_index = block_start; block_index < block_count; block_index += warp_blocks_per_iter) {
        if (gate_rows_ptrs[0] != nullptr) {
            gate_sum[0] += dot_fn(gate_rows_ptrs[0], shared_input, block_index, block_index, quant_index);
            up_sum[0] += dot_fn(up_rows_ptrs[0], shared_input, block_index, block_index, quant_index);
        }
        if (gate_rows_ptrs[1] != nullptr) {
            gate_sum[1] += dot_fn(gate_rows_ptrs[1], shared_input, block_index, block_index, quant_index);
            up_sum[1] += dot_fn(up_rows_ptrs[1], shared_input, block_index, block_index, quant_index);
        }
    }

    gate_sum[0] = warp_reduce_sum(gate_sum[0]);
    gate_sum[1] = warp_reduce_sum(gate_sum[1]);
    up_sum[0] = warp_reduce_sum(up_sum[0]);
    up_sum[1] = warp_reduce_sum(up_sum[1]);
    if (threadIdx.x == 0) {
        for (int row_offset = 0; row_offset < 2; ++row_offset) {
            const int row = row0 + row_offset;
            if (row >= gate_rows || row >= up_rows) {
                continue;
            }
            const float gate = gate_sum[row_offset] +
                (gate_bias != nullptr ? gate_bias[expert_id * gate_rows + row] : 0.0f);
            const float up = up_sum[row_offset] +
                (up_bias != nullptr ? up_bias[expert_id * up_rows + row] : 0.0f);
            output[selected_slot * gate_rows + row] = swiglu_oai_single(gate, up);
        }
    }
}

template <typename DotFn, int Vdr, int Qi>
__launch_bounds__(8 * kWarpSize, 1)
__global__ void moe_gate_up_swiglu_q8_1_selected4_quantized_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const int32_t *selected_ids,
    int selected_count,
    const Q81Block *input,
    const float *gate_bias,
    const float *up_bias,
    Q81Block *output_q8_1,
    DotFn dot_fn
) {
    extern __shared__ unsigned char shared_storage[];
    Q81Block *shared_input = reinterpret_cast<Q81Block *>(shared_storage);
    __shared__ float activated_values[kQ81ElementsPerBlock];

    const int block_count = columns / kQ81ElementsPerBlock;
    const int linear_tid = static_cast<int>(threadIdx.y) * kWarpSize + static_cast<int>(threadIdx.x);
    const int thread_count = 8 * kWarpSize;
    for (int block_index = linear_tid; block_index < block_count; block_index += thread_count) {
        shared_input[block_index] = input[block_index];
    }
    __syncthreads();

    const int selected_slot = static_cast<int>(blockIdx.y);
    if (selected_slot >= selected_count || selected_count > 4) {
        return;
    }

    const int output_block_index = static_cast<int>(blockIdx.x);
    const int row_base = output_block_index * kQ81ElementsPerBlock;
    const int row_block_offset = static_cast<int>(threadIdx.y) * 4;

#pragma unroll
    for (int row_offset = 0; row_offset < 4; ++row_offset) {
        const int activated_index = row_block_offset + row_offset;
        const int row = row_base + activated_index;
        float gate_sum = 0.0f;
        float up_sum = 0.0f;
        if (row < gate_rows && row < up_rows) {
            const int expert_id = selected_ids[selected_slot];
            const uint8_t *gate_row =
                weights +
                (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
                 static_cast<size_t>(row)) *
                    static_cast<size_t>(row_stride);
            const uint8_t *up_row =
                weights +
                (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
                 static_cast<size_t>(gate_rows + row)) *
                    static_cast<size_t>(row_stride);
            constexpr int warp_blocks_per_iter = Vdr * kWarpSize / Qi;
            const int block_start = static_cast<int>(threadIdx.x) / (Qi / Vdr);
            const int quant_index = Vdr * (static_cast<int>(threadIdx.x) % (Qi / Vdr));
            for (int block_index = block_start; block_index < block_count; block_index += warp_blocks_per_iter) {
                gate_sum += dot_fn(gate_row, shared_input, block_index, block_index, quant_index);
                up_sum += dot_fn(up_row, shared_input, block_index, block_index, quant_index);
            }
            gate_sum = warp_reduce_sum(gate_sum);
            up_sum = warp_reduce_sum(up_sum);
            if (threadIdx.x == 0) {
                const float gate =
                    gate_sum +
                    (gate_bias != nullptr ? gate_bias[expert_id * gate_rows + row] : 0.0f);
                const float up =
                    up_sum + (up_bias != nullptr ? up_bias[expert_id * up_rows + row] : 0.0f);
                activated_values[activated_index] = swiglu_oai_single(gate, up);
            }
        } else if (threadIdx.x == 0) {
            activated_values[activated_index] = 0.0f;
        }
    }
    __syncthreads();

    if (threadIdx.y == 0) {
        const int blocks_per_row = gate_rows / kQ81ElementsPerBlock;
        quantize_q8_1_shared_block(
            activated_values,
            output_q8_1 + static_cast<size_t>(selected_slot) * static_cast<size_t>(blocks_per_row) +
                static_cast<size_t>(output_block_index),
            static_cast<int>(threadIdx.x)
        );
    }
}

__global__ void moe_down_aggregate_q8_1_kernel(
    const uint8_t *weights,
    int mode,
    int row_stride,
    int rows,
    int columns,
    const int32_t *selected_ids,
    const float *selected_weights,
    int selected_count,
    const Q81Block *activated,
    const float *bias,
    const float *residual,
    float *output
) {
    const int row = static_cast<int>(blockIdx.x);
    if (row >= rows) {
        return;
    }

    const int block_count = columns / kQ81ElementsPerBlock;
    __shared__ float scratch[kMatvecBlockSize];
    __shared__ float total;
    if (threadIdx.x == 0) {
        total = 0.0f;
    }
    __syncthreads();

    for (int selected_slot = 0; selected_slot < selected_count; ++selected_slot) {
        const int expert_id = selected_ids[selected_slot];
        const uint8_t *row_weights = weights + (
            static_cast<size_t>(expert_id) * static_cast<size_t>(rows) + static_cast<size_t>(row)
        ) * static_cast<size_t>(row_stride);
        const Q81Block *expert_input = activated + static_cast<size_t>(selected_slot) * static_cast<size_t>(block_count);

        float partial = 0.0f;
        if (mode == 0) {
            const Q80Q81Dot dot{};
            for (int block_index = static_cast<int>(threadIdx.x); block_index < block_count; block_index += blockDim.x) {
                partial += dot(row_weights, block_index, expert_input);
            }
        } else {
            const Mxfp4Q81Dot dot{};
            for (int block_index = static_cast<int>(threadIdx.x); block_index < block_count; block_index += blockDim.x) {
                partial += dot(row_weights, block_index, expert_input);
            }
        }

        scratch[threadIdx.x] = partial;
        __syncthreads();
        for (int offset = blockDim.x / 2; offset > 0; offset >>= 1) {
            if (threadIdx.x < offset) {
                scratch[threadIdx.x] += scratch[threadIdx.x + offset];
            }
            __syncthreads();
        }
        if (threadIdx.x == 0) {
            const float expert_value =
                scratch[0] + (bias != nullptr ? bias[expert_id * rows + row] : 0.0f);
            total += expert_value * selected_weights[selected_slot];
        }
        __syncthreads();
    }

    if (threadIdx.x == 0) {
        output[row] = total + (residual != nullptr ? residual[row] : 0.0f);
    }
}

template <typename DotFn, int Vdr, int Qi, bool HasGate>
__launch_bounds__(kMmvqWarps * kWarpSize, 1)
__global__ void expert_mul_mat_vec_q8_1_kernel(
    const uint8_t *weights,
    const uint8_t *gate_weights,
    int row_stride,
    int rows,
    int rows_per_expert,
    int block_count,
    const int32_t *selected_ids,
    int selected_count,
    const Q81Block *input,
    int input_block_stride,
    const float *bias,
    const float *gate_bias,
    float *output,
    DotFn dot_fn
) {
    const int row = static_cast<int>(blockIdx.x);
    const int selected_slot = static_cast<int>(blockIdx.y);
    if (row >= rows || selected_slot >= selected_count) {
        return;
    }

    constexpr int blocks_per_iter = Vdr * kMmvqWarps * kWarpSize / Qi;
    const int tid = kWarpSize * static_cast<int>(threadIdx.y) + static_cast<int>(threadIdx.x);
    const int expert_id = selected_ids[selected_slot];

    const uint8_t *row_weights =
        weights +
        (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
         static_cast<size_t>(row)) *
            static_cast<size_t>(row_stride);
    const uint8_t *gate_row_weights = nullptr;
    if constexpr (HasGate) {
        gate_row_weights =
            gate_weights +
            (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
             static_cast<size_t>(row)) *
                static_cast<size_t>(row_stride);
    }
    const Q81Block *input_blocks =
        input + static_cast<size_t>(selected_slot) * static_cast<size_t>(input_block_stride);

    float sum = 0.0f;
    float gate_sum = 0.0f;
    for (int block_index = tid / (Qi / Vdr); block_index < block_count; block_index += blocks_per_iter) {
        const int quant_index = Vdr * (tid % (Qi / Vdr));
        sum += dot_fn(row_weights, input_blocks, block_index, block_index, quant_index);
        if constexpr (HasGate) {
            gate_sum += dot_fn(gate_row_weights, input_blocks, block_index, block_index, quant_index);
        }
    }

    __shared__ float partials[kMmvqWarps - 1 > 0 ? kMmvqWarps - 1 : 1][kWarpSize];
    __shared__ float gate_partials[(HasGate && (kMmvqWarps - 1 > 0)) ? kMmvqWarps - 1 : 1][kWarpSize];

    if (threadIdx.y > 0) {
        partials[threadIdx.y - 1][threadIdx.x] = sum;
        if constexpr (HasGate) {
            gate_partials[threadIdx.y - 1][threadIdx.x] = gate_sum;
        }
    }
    __syncthreads();

    if (threadIdx.y > 0) {
        return;
    }

#pragma unroll
    for (int warp_index = 0; warp_index < kMmvqWarps - 1; ++warp_index) {
        sum += partials[warp_index][threadIdx.x];
        if constexpr (HasGate) {
            gate_sum += gate_partials[warp_index][threadIdx.x];
        }
    }

    sum = warp_reduce_sum(sum);
    if constexpr (HasGate) {
        gate_sum = warp_reduce_sum(gate_sum);
    }

    if (threadIdx.x != 0) {
        return;
    }

    float result = sum;
    if (bias != nullptr) {
        result += bias[expert_id * rows + row];
    }
    if constexpr (HasGate) {
        float gate = gate_sum;
        if (gate_bias != nullptr) {
            gate += gate_bias[expert_id * rows + row];
        }
        result = swiglu_oai_single(gate, result);
    }
    output[selected_slot * rows + row] = result;
}

template <typename DotFn, int Vdr, int Qi, int WarpsPerBlock>
__launch_bounds__(WarpsPerBlock * kWarpSize, 1)
__global__ void expert_mul_mat_vec_q8_1_project_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int rows_per_expert,
    int block_count,
    const int32_t *selected_ids,
    int selected_count,
    const Q81Block *input,
    int input_block_stride,
    const float *bias,
    float *output,
    DotFn dot_fn
) {
    const int row = static_cast<int>(blockIdx.x);
    const int selected_slot = static_cast<int>(blockIdx.y);
    if (row >= rows || selected_slot >= selected_count) {
        return;
    }

    constexpr int blocks_per_iter = Vdr * WarpsPerBlock * kWarpSize / Qi;
    const int tid = kWarpSize * static_cast<int>(threadIdx.y) + static_cast<int>(threadIdx.x);
    const int expert_id = selected_ids[selected_slot];
    const uint8_t *row_weights =
        weights +
        (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
         static_cast<size_t>(row)) *
            static_cast<size_t>(row_stride);
    const Q81Block *input_blocks =
        input + static_cast<size_t>(selected_slot) * static_cast<size_t>(input_block_stride);

    float sum = 0.0f;
    for (int block_index = tid / (Qi / Vdr); block_index < block_count; block_index += blocks_per_iter) {
        const int quant_index = Vdr * (tid % (Qi / Vdr));
        sum += dot_fn(row_weights, input_blocks, block_index, block_index, quant_index);
    }

    __shared__ float partials[WarpsPerBlock - 1 > 0 ? WarpsPerBlock - 1 : 1][kWarpSize];
    if (threadIdx.y > 0) {
        partials[threadIdx.y - 1][threadIdx.x] = sum;
    }
    __syncthreads();

    if (threadIdx.y > 0) {
        return;
    }

#pragma unroll
    for (int warp_index = 0; warp_index < WarpsPerBlock - 1; ++warp_index) {
        sum += partials[warp_index][threadIdx.x];
    }

    sum = warp_reduce_sum(sum);
    if (threadIdx.x != 0) {
        return;
    }

    float result = sum;
    if (bias != nullptr) {
        result += bias[expert_id * rows + row];
    }
    output[selected_slot * rows + row] = result;
}

template <typename DotFn, int Vdr, int Qi, int SelectedCols, int RowsPerBlock, int WarpsPerBlock>
__launch_bounds__(WarpsPerBlock * kWarpSize, 1)
__global__ void expert_mul_mat_vec_q8_1_project_grouped_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int rows_per_expert,
    int block_count,
    const int32_t *selected_ids,
    int selected_count,
    const Q81Block *input,
    int input_block_stride,
    const float *bias,
    float *output,
    DotFn dot_fn
) {
    const int row0 = RowsPerBlock * static_cast<int>(blockIdx.x);
    const int tid = kWarpSize * static_cast<int>(threadIdx.y) + static_cast<int>(threadIdx.x);
    const int block_start = tid / (Qi / Vdr);
    const int quant_index = Vdr * (tid % (Qi / Vdr));
    constexpr int blocks_per_iter = Vdr * WarpsPerBlock * kWarpSize / Qi;

    float sums[SelectedCols][RowsPerBlock] = {{0.0f}};

#pragma unroll
    for (int selected_slot = 0; selected_slot < SelectedCols; ++selected_slot) {
        if (selected_slot >= selected_count) {
            continue;
        }
        const int expert_id = selected_ids[selected_slot];
        const Q81Block *input_blocks =
            input + static_cast<size_t>(selected_slot) * static_cast<size_t>(input_block_stride);

#pragma unroll
        for (int row_offset = 0; row_offset < RowsPerBlock; ++row_offset) {
            const int row = row0 + row_offset;
            if (row >= rows) {
                continue;
            }
            const uint8_t *row_weights =
                weights +
                (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
                 static_cast<size_t>(row)) *
                    static_cast<size_t>(row_stride);
            for (int block_index = block_start; block_index < block_count; block_index += blocks_per_iter) {
                sums[selected_slot][row_offset] += dot_fn(
                    row_weights,
                    input_blocks,
                    block_index,
                    block_index,
                    quant_index
                );
            }
        }
    }

    __shared__ float partials[WarpsPerBlock - 1 > 0 ? WarpsPerBlock - 1 : 1][SelectedCols][RowsPerBlock][kWarpSize];
    if (threadIdx.y > 0) {
#pragma unroll
        for (int selected_slot = 0; selected_slot < SelectedCols; ++selected_slot) {
#pragma unroll
            for (int row_offset = 0; row_offset < RowsPerBlock; ++row_offset) {
                partials[threadIdx.y - 1][selected_slot][row_offset][threadIdx.x] =
                    sums[selected_slot][row_offset];
            }
        }
    }
    __syncthreads();

    if (threadIdx.y > 0) {
        return;
    }

#pragma unroll
    for (int selected_slot = 0; selected_slot < SelectedCols; ++selected_slot) {
#pragma unroll
        for (int row_offset = 0; row_offset < RowsPerBlock; ++row_offset) {
#pragma unroll
            for (int warp_index = 0; warp_index < WarpsPerBlock - 1; ++warp_index) {
                sums[selected_slot][row_offset] +=
                    partials[warp_index][selected_slot][row_offset][threadIdx.x];
            }
            sums[selected_slot][row_offset] = warp_reduce_sum(sums[selected_slot][row_offset]);
        }
    }

    if (threadIdx.x >= RowsPerBlock) {
        return;
    }

    const int row = row0 + static_cast<int>(threadIdx.x);
    if (row >= rows) {
        return;
    }

#pragma unroll
    for (int selected_slot = 0; selected_slot < SelectedCols; ++selected_slot) {
        if (selected_slot >= selected_count) {
            continue;
        }
        const int expert_id = selected_ids[selected_slot];
        float result = sums[selected_slot][threadIdx.x];
        if (bias != nullptr) {
            result += bias[expert_id * rows + row];
        }
        output[selected_slot * rows + row] = result;
    }
}

template <typename DotFn, int Vdr, int Qi>
static void launch_expert_gate_up_swiglu_q8_1_direct(
    const uint8_t *weights,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int selected_count,
    const int32_t *selected_ids,
    const Q81Block *input_q8_1,
    const float *gate_bias,
    const float *up_bias,
    float *output,
    cudaStream_t stream,
    DotFn dot_fn
) {
    const dim3 blocks(static_cast<unsigned int>(gate_rows), static_cast<unsigned int>(selected_count), 1);
    const dim3 block_dims(kWarpSize, kMmvqWarps, 1);
    const uint8_t *gate_weights = weights;
    const uint8_t *up_weights =
        weights + static_cast<size_t>(gate_rows) * static_cast<size_t>(row_stride);
    expert_mul_mat_vec_q8_1_kernel<DotFn, Vdr, Qi, true><<<
        blocks,
        block_dims,
        0,
        stream
    >>>(
        up_weights,
        gate_weights,
        row_stride,
        gate_rows,
        rows_per_expert,
        columns / kQ81ElementsPerBlock,
        selected_ids,
        selected_count,
        input_q8_1,
        0,
        up_bias,
        gate_bias,
        output,
        dot_fn
    );
}

template <typename DotFn, int Vdr, int Qi, int MaxSelected, int RowsPerBlock>
__launch_bounds__(kMmvqWarps * kWarpSize, 1)
__global__ void moe_down_aggregate_q8_1_mmvq_grouped_selected_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int columns,
    const int32_t *selected_ids,
    const float *selected_weights,
    int selected_count,
    const Q81Block *activated,
    const float *bias,
    const float *residual,
    float *output,
    DotFn dot_fn
) {
    const int row0 = RowsPerBlock * static_cast<int>(blockIdx.x);
    if (selected_count > MaxSelected || row0 >= rows) {
        return;
    }

    constexpr int blocks_per_iter = Vdr * kMmvqWarps * kWarpSize / Qi;
    const int tid = kWarpSize * static_cast<int>(threadIdx.y) + static_cast<int>(threadIdx.x);
    const int block_count = columns / kQ81ElementsPerBlock;

    const uint8_t *row_weights_ptrs[MaxSelected][RowsPerBlock];
    float partials[MaxSelected][RowsPerBlock] = {{0.0f}};

#pragma unroll
    for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
        const bool selected_valid = selected_slot < selected_count;
        const int expert_id = selected_valid ? selected_ids[selected_slot] : 0;
#pragma unroll
        for (int row_index = 0; row_index < RowsPerBlock; ++row_index) {
            row_weights_ptrs[selected_slot][row_index] = nullptr;
            const int row = row0 + row_index;
            if (selected_valid && row < rows) {
                const size_t row_offset =
                    (static_cast<size_t>(expert_id) * static_cast<size_t>(rows) +
                     static_cast<size_t>(row)) *
                    static_cast<size_t>(row_stride);
                row_weights_ptrs[selected_slot][row_index] = weights + row_offset;
            }
        }
    }

    for (int block_index = tid / (Qi / Vdr); block_index < block_count; block_index += blocks_per_iter) {
        const int quant_index = Vdr * (tid % (Qi / Vdr));
#pragma unroll
        for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
            if (selected_slot >= selected_count) {
                break;
            }
            const Q81Block *expert_input =
                activated + static_cast<size_t>(selected_slot) * static_cast<size_t>(block_count);
#pragma unroll
            for (int row_index = 0; row_index < RowsPerBlock; ++row_index) {
                if (row_weights_ptrs[selected_slot][row_index] != nullptr) {
                    partials[selected_slot][row_index] += dot_fn(
                        row_weights_ptrs[selected_slot][row_index],
                        expert_input,
                        block_index,
                        block_index,
                        quant_index
                    );
                }
            }
        }
    }

    __shared__ float partials_shared[kMmvqWarps - 1 > 0 ? kMmvqWarps - 1 : 1][MaxSelected][RowsPerBlock][kWarpSize];
    if (threadIdx.y > 0) {
#pragma unroll
        for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
            if (selected_slot >= selected_count) {
                break;
            }
#pragma unroll
            for (int row_index = 0; row_index < RowsPerBlock; ++row_index) {
                partials_shared[threadIdx.y - 1][selected_slot][row_index][threadIdx.x] =
                    partials[selected_slot][row_index];
            }
        }
    }
    __syncthreads();

    if (threadIdx.y > 0) {
        return;
    }

#pragma unroll
    for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
        if (selected_slot >= selected_count) {
            break;
        }
#pragma unroll
        for (int row_index = 0; row_index < RowsPerBlock; ++row_index) {
#pragma unroll
            for (int warp_index = 0; warp_index < kMmvqWarps - 1; ++warp_index) {
                partials[selected_slot][row_index] +=
                    partials_shared[warp_index][selected_slot][row_index][threadIdx.x];
            }
            partials[selected_slot][row_index] =
                warp_reduce_sum(partials[selected_slot][row_index]);
        }
    }

    if (threadIdx.x >= RowsPerBlock) {
        return;
    }

    const int row = row0 + static_cast<int>(threadIdx.x);
    if (row >= rows) {
        return;
    }

    float total = 0.0f;
#pragma unroll
    for (int selected_slot = 0; selected_slot < MaxSelected; ++selected_slot) {
        if (selected_slot >= selected_count) {
            break;
        }
        const int expert_id = selected_ids[selected_slot];
        const float expert_value =
            partials[selected_slot][threadIdx.x] +
            (bias != nullptr ? bias[expert_id * rows + row] : 0.0f);
        total += expert_value * selected_weights[selected_slot];
    }
    output[row] = total + (residual != nullptr ? residual[row] : 0.0f);
}

template <typename DotFn, int Vdr, int Qi>
__launch_bounds__(kMmvqWarps * kWarpSize, 1)
__global__ void expert_down_aggregate_q8_1_atomic_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int rows_per_expert,
    const int32_t *selected_ids,
    const float *selected_weights,
    int selected_count,
    const Q81Block *activated,
    int activated_block_stride,
    const float *bias,
    const float *residual,
    float *output,
    DotFn dot_fn
) {
    const int row = static_cast<int>(blockIdx.x);
    const int selected_slot = static_cast<int>(blockIdx.y);
    if (row >= rows || selected_slot >= selected_count) {
        return;
    }

    constexpr int blocks_per_iter = Vdr * kMmvqWarps * kWarpSize / Qi;
    const int tid = kWarpSize * static_cast<int>(threadIdx.y) + static_cast<int>(threadIdx.x);
    const int block_count = activated_block_stride;
    const int expert_id = selected_ids[selected_slot];
    const uint8_t *row_weights =
        weights +
        (static_cast<size_t>(expert_id) * static_cast<size_t>(rows_per_expert) +
         static_cast<size_t>(row)) *
            static_cast<size_t>(row_stride);
    const Q81Block *input_blocks =
        activated + static_cast<size_t>(selected_slot) * static_cast<size_t>(activated_block_stride);

    float sum = 0.0f;
    for (int block_index = tid / (Qi / Vdr); block_index < block_count; block_index += blocks_per_iter) {
        const int quant_index = Vdr * (tid % (Qi / Vdr));
        sum += dot_fn(row_weights, input_blocks, block_index, block_index, quant_index);
    }

    __shared__ float partials[kMmvqWarps - 1 > 0 ? kMmvqWarps - 1 : 1][kWarpSize];
    if (threadIdx.y > 0) {
        partials[threadIdx.y - 1][threadIdx.x] = sum;
    }
    __syncthreads();

    if (threadIdx.y > 0) {
        return;
    }

#pragma unroll
    for (int warp_index = 0; warp_index < kMmvqWarps - 1; ++warp_index) {
        sum += partials[warp_index][threadIdx.x];
    }

    sum = warp_reduce_sum(sum);
    if (threadIdx.x != 0) {
        return;
    }

    float result = sum;
    if (bias != nullptr) {
        result += bias[expert_id * rows + row];
    }
    result *= selected_weights[selected_slot];
    atomicAdd(output + row, result);
}

template <typename DotFn, int Vdr, int Qi>
__launch_bounds__(4 * kWarpSize, 1)
__global__ void moe_down_aggregate_q8_1_selected4_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    const int32_t *selected_ids,
    const float *selected_weights,
    int selected_count,
    const Q81Block *activated,
    int activated_block_stride,
    const float *bias,
    const float *residual,
    float *output,
    DotFn dot_fn
) {
    extern __shared__ unsigned char shared_storage[];
    Q81Block *shared_inputs = reinterpret_cast<Q81Block *>(shared_storage);

    const int block_count = activated_block_stride;
    const int linear_tid = static_cast<int>(threadIdx.y) * kWarpSize + static_cast<int>(threadIdx.x);
    const int thread_count = 4 * kWarpSize;
    for (int block_index = linear_tid; block_index < selected_count * block_count; block_index += thread_count) {
        shared_inputs[block_index] = activated[block_index];
    }
    __syncthreads();

    const int row0 = 2 * static_cast<int>(blockIdx.x);
    const int selected_slot = static_cast<int>(threadIdx.y);
    if (row0 >= rows) {
        return;
    }

    __shared__ float expert_totals[4];
    __shared__ float expert_totals_row1[4];
    if (threadIdx.x == 0 && selected_slot < 4) {
        expert_totals[selected_slot] = 0.0f;
        expert_totals_row1[selected_slot] = 0.0f;
    }
    __syncthreads();

    if (selected_slot < selected_count) {
        const int expert_id = selected_ids[selected_slot];
        const uint8_t *row_weights0 =
            weights +
            (static_cast<size_t>(expert_id) * static_cast<size_t>(rows) +
             static_cast<size_t>(row0)) *
                static_cast<size_t>(row_stride);
        const uint8_t *row_weights1 = row0 + 1 < rows
            ? weights +
                (static_cast<size_t>(expert_id) * static_cast<size_t>(rows) +
                 static_cast<size_t>(row0 + 1)) *
                    static_cast<size_t>(row_stride)
            : nullptr;
        const Q81Block *input_blocks =
            shared_inputs + static_cast<size_t>(selected_slot) * static_cast<size_t>(activated_block_stride);
        constexpr int warp_blocks_per_iter = Vdr * kWarpSize / Qi;
        const int block_start = static_cast<int>(threadIdx.x) / (Qi / Vdr);
        const int quant_index = Vdr * (static_cast<int>(threadIdx.x) % (Qi / Vdr));

        float sum0 = 0.0f;
        float sum1 = 0.0f;
        for (int block_index = block_start; block_index < block_count; block_index += warp_blocks_per_iter) {
            sum0 += dot_fn(row_weights0, input_blocks, block_index, block_index, quant_index);
            if (row_weights1 != nullptr) {
                sum1 += dot_fn(row_weights1, input_blocks, block_index, block_index, quant_index);
            }
        }
        sum0 = warp_reduce_sum(sum0);
        sum1 = warp_reduce_sum(sum1);
        if (threadIdx.x == 0) {
            expert_totals[selected_slot] =
                sum0 + (bias != nullptr ? bias[expert_id * rows + row0] : 0.0f);
            if (row0 + 1 < rows) {
                expert_totals_row1[selected_slot] =
                    sum1 + (bias != nullptr ? bias[expert_id * rows + row0 + 1] : 0.0f);
            }
        }
    }
    __syncthreads();

    if (selected_slot == 0 && threadIdx.x == 0) {
        float total0 = 0.0f;
        float total1 = 0.0f;
#pragma unroll
        for (int slot = 0; slot < 4; ++slot) {
            if (slot >= selected_count) {
                break;
            }
            total0 += expert_totals[slot] * selected_weights[slot];
            total1 += expert_totals_row1[slot] * selected_weights[slot];
        }
        output[row0] = total0 + (residual != nullptr ? residual[row0] : 0.0f);
        if (row0 + 1 < rows) {
            output[row0 + 1] = total1 + (residual != nullptr ? residual[row0 + 1] : 0.0f);
        }
    }
}

template <typename DotFn, int Vdr, int Qi>
__launch_bounds__(4 * kWarpSize, 1)
__global__ void moe_down_aggregate_q8_1_selected4_f32_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int columns,
    const int32_t *selected_ids,
    const float *selected_weights,
    int selected_count,
    const float *activated,
    const float *bias,
    const float *residual,
    float *output,
    DotFn dot_fn
) {
    extern __shared__ unsigned char shared_storage[];
    Q81Block *shared_inputs = reinterpret_cast<Q81Block *>(shared_storage);

    const int block_count = columns / kQ81ElementsPerBlock;
    const int lane = static_cast<int>(threadIdx.x);
    const int selected_slot = static_cast<int>(threadIdx.y);
    if (selected_slot < selected_count) {
        const float *selected_input =
            activated + static_cast<size_t>(selected_slot) * static_cast<size_t>(columns);
        Q81Block *selected_blocks =
            shared_inputs + static_cast<size_t>(selected_slot) * static_cast<size_t>(block_count);
        for (int block_index = 0; block_index < block_count; ++block_index) {
            quantize_q8_1_shared_block(
                selected_input + static_cast<size_t>(block_index) * static_cast<size_t>(kQ81ElementsPerBlock),
                &selected_blocks[block_index],
                lane
            );
            __syncwarp();
        }
    }
    __syncthreads();

    const int row0 = 2 * static_cast<int>(blockIdx.x);
    if (row0 >= rows) {
        return;
    }

    __shared__ float expert_totals[4];
    __shared__ float expert_totals_row1[4];
    if (lane == 0 && selected_slot < 4) {
        expert_totals[selected_slot] = 0.0f;
        expert_totals_row1[selected_slot] = 0.0f;
    }
    __syncthreads();

    if (selected_slot < selected_count) {
        const int expert_id = selected_ids[selected_slot];
        const uint8_t *row_weights0 =
            weights +
            (static_cast<size_t>(expert_id) * static_cast<size_t>(rows) +
             static_cast<size_t>(row0)) *
                static_cast<size_t>(row_stride);
        const uint8_t *row_weights1 = row0 + 1 < rows
            ? weights +
                (static_cast<size_t>(expert_id) * static_cast<size_t>(rows) +
                 static_cast<size_t>(row0 + 1)) *
                    static_cast<size_t>(row_stride)
            : nullptr;
        const Q81Block *input_blocks =
            shared_inputs + static_cast<size_t>(selected_slot) * static_cast<size_t>(block_count);
        constexpr int warp_blocks_per_iter = Vdr * kWarpSize / Qi;
        const int block_start = lane / (Qi / Vdr);
        const int quant_index = Vdr * (lane % (Qi / Vdr));

        float sum0 = 0.0f;
        float sum1 = 0.0f;
        for (int block_index = block_start; block_index < block_count; block_index += warp_blocks_per_iter) {
            sum0 += dot_fn(row_weights0, input_blocks, block_index, block_index, quant_index);
            if (row_weights1 != nullptr) {
                sum1 += dot_fn(row_weights1, input_blocks, block_index, block_index, quant_index);
            }
        }
        sum0 = warp_reduce_sum(sum0);
        sum1 = warp_reduce_sum(sum1);
        if (lane == 0) {
            expert_totals[selected_slot] =
                sum0 + (bias != nullptr ? bias[expert_id * rows + row0] : 0.0f);
            if (row0 + 1 < rows) {
                expert_totals_row1[selected_slot] =
                    sum1 + (bias != nullptr ? bias[expert_id * rows + row0 + 1] : 0.0f);
            }
        }
    }
    __syncthreads();

    if (selected_slot == 0 && lane == 0) {
        float total0 = 0.0f;
        float total1 = row0 + 1 < rows ? 0.0f : 0.0f;
        for (int index = 0; index < selected_count; ++index) {
            total0 += expert_totals[index] * selected_weights[index];
            if (row0 + 1 < rows) {
                total1 += expert_totals_row1[index] * selected_weights[index];
            }
        }
        output[row0] = total0 + (residual != nullptr ? residual[row0] : 0.0f);
        if (row0 + 1 < rows) {
            output[row0 + 1] = total1 + (residual != nullptr ? residual[row0 + 1] : 0.0f);
        }
    }
}

__global__ void accumulate_selected4_kernel(
    const float *input,
    const float *selected_weights,
    int selected_count,
    int rows,
    const float *residual,
    float *output
) {
    const int row = static_cast<int>(blockIdx.x) * static_cast<int>(blockDim.x) +
        static_cast<int>(threadIdx.x);
    if (row >= rows) {
        return;
    }

    float total = residual != nullptr ? residual[row] : 0.0f;
    for (int selected_slot = 0; selected_slot < selected_count; ++selected_slot) {
        total +=
            input[static_cast<size_t>(selected_slot) * static_cast<size_t>(rows) +
                static_cast<size_t>(row)] * selected_weights[selected_slot];
    }
    output[row] = total;
}

}  // namespace

extern "C" int psionic_cuda_quantized_kernels_compiled(void) {
    return 1;
}

#define PSIONIC_DEFINE_SUPER_BLOCK_KERNELS(NAME, VALUE_FN, MMVQ_DOT)                                      \
extern "C" int psionic_cuda_##NAME##_matvec(                                                   \
    const void *weights, int rows, int cols, int row_stride,                                    \
    const void *input, void *output, void *stream                                                \
) {                                                                                              \
    quantized_matvec_kernel<<<rows, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(         \
        static_cast<const uint8_t *>(weights), row_stride, rows, cols,                           \
        static_cast<const float *>(input), static_cast<float *>(output),                         \
        SuperBlockDot<VALUE_FN>{}                                                                \
    );                                                                                           \
    return static_cast<int>(cudaGetLastError());                                                 \
}                                                                                                \
extern "C" int psionic_cuda_##NAME##_dequantize_row_to_f32(                                    \
    const void *weights, int rows, int cols, int row_stride,                                    \
    const void *decode_params, void *output, void *stream                                        \
) {                                                                                              \
    const int blocks_per_row = cols / kQ81ElementsPerBlock;                                     \
    dequantize_row_to_f32_kernel<<<                                                              \
        blocks_per_row, kQ81ElementsPerBlock, 0, static_cast<cudaStream_t>(stream)               \
    >>>(                                                                                         \
        static_cast<const uint8_t *>(weights), rows, cols, row_stride,                           \
        static_cast<const int *>(decode_params), static_cast<float *>(output),                   \
        SuperBlockRowDequant<VALUE_FN>{}                                                         \
    );                                                                                           \
    return static_cast<int>(cudaGetLastError());                                                 \
}                                                                                                \
extern "C" int psionic_cuda_##NAME##_matvec_q8_1(                                              \
    const void *weights, int rows, int cols, int row_stride,                                    \
    const void *input_q8_1, const void *bias, void *output, void *stream                         \
) {                                                                                              \
    const int block_count = cols / 256;                                                          \
    constexpr int rows_per_block = 4;                                                            \
    const dim3 grid_dims((rows + rows_per_block - 1) / rows_per_block, 1, 1);                    \
    const dim3 block_dims(kWarpSize, rows_per_block, 1);                                         \
    quantized_matvec_q8_1_grouped_mmvq_kernel<MMVQ_DOT, 4, 32, rows_per_block, 1><<<             \
        grid_dims,                                                                               \
        block_dims,                                                                              \
        0,                                                                                       \
        static_cast<cudaStream_t>(stream)                                                        \
    >>>(                                                                                         \
        static_cast<const uint8_t *>(weights), row_stride, rows, block_count,                    \
        static_cast<const Q81Block *>(input_q8_1), static_cast<const float *>(bias),             \
        static_cast<float *>(output),                                                            \
        MMVQ_DOT{}                                                                               \
    );                                                                                           \
    return static_cast<int>(cudaGetLastError());                                                 \
}                                                                                                \
extern "C" int psionic_cuda_##NAME##_matvec_q8_1_argmax(                                       \
    const void *weights, int rows, int cols, int row_stride,                                    \
    const void *input_q8_1, const void *bias, void *output, void *stream                         \
) {                                                                                              \
    launch_quantized_matvec_q8_1_grouped_argmax_mmvq<MMVQ_DOT, 4, 32, 4, 1>(                     \
        static_cast<const uint8_t *>(weights), rows, cols / 256, row_stride,                     \
        static_cast<const Q81Block *>(input_q8_1), static_cast<const float *>(bias),             \
        static_cast<unsigned long long *>(output), static_cast<cudaStream_t>(stream),            \
        MMVQ_DOT{}                                                                               \
    );                                                                                           \
    return static_cast<int>(cudaGetLastError());                                                 \
}

PSIONIC_DEFINE_SUPER_BLOCK_KERNELS(q3_k, Q3KValue, Q3KQ81Dot)
PSIONIC_DEFINE_SUPER_BLOCK_KERNELS(q5_k, Q5KValue, Q5KQ81Dot)
PSIONIC_DEFINE_SUPER_BLOCK_KERNELS(iq3_s, Iq3SValue, Iq3SQ81Dot)
PSIONIC_DEFINE_SUPER_BLOCK_KERNELS(iq4_xs, Iq4XsValue, Iq4XsQ81Dot)

#undef PSIONIC_DEFINE_SUPER_BLOCK_KERNELS

extern "C" int psionic_cuda_q8_0_matvec(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input,
    void *output,
    void *stream
) {
    quantized_matvec_kernel<<<rows, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint8_t *>(weights),
        row_stride,
        rows,
        cols,
        static_cast<const float *>(input),
        static_cast<float *>(output),
        Q80Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_mxfp4_matvec(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input,
    void *output,
    void *stream
) {
    quantized_matvec_kernel<<<rows, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint8_t *>(weights),
        row_stride,
        rows,
        cols,
        static_cast<const float *>(input),
        static_cast<float *>(output),
        Mxfp4Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_q4_k_matvec(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input,
    void *output,
    void *stream
) {
    quantized_matvec_kernel<<<rows, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint8_t *>(weights),
        row_stride,
        rows,
        cols,
        static_cast<const float *>(input),
        static_cast<float *>(output),
        Q4KDot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_q6_k_matvec(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input,
    void *output,
    void *stream
) {
    quantized_matvec_kernel<<<rows, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint8_t *>(weights),
        row_stride,
        rows,
        cols,
        static_cast<const float *>(input),
        static_cast<float *>(output),
        Q6KDot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_quantize_q8_1(
    const void *input,
    int rows,
    int cols,
    void *output,
    void *stream
) {
    const int blocks_per_row = cols / kQ81ElementsPerBlock;
    const dim3 grid(static_cast<unsigned int>(blocks_per_row), static_cast<unsigned int>(rows), 1);
    quantize_q8_1_rows_kernel<<<grid, kQ81ElementsPerBlock, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        rows,
        cols,
        static_cast<Q81Block *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_q8_0_dequantize_row_to_f32(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *decode_params,
    void *output,
    void *stream
) {
    const int blocks_per_row = cols / kQ81ElementsPerBlock;
    dequantize_row_to_f32_kernel<<<blocks_per_row, kQ81ElementsPerBlock, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint8_t *>(weights),
        rows,
        cols,
        row_stride,
        static_cast<const int *>(decode_params),
        static_cast<float *>(output),
        Q80Dequant{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_mxfp4_dequantize_row_to_f32(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *decode_params,
    void *output,
    void *stream
) {
    const int blocks_per_row = cols / kQ81ElementsPerBlock;
    dequantize_row_to_f32_kernel<<<blocks_per_row, kQ81ElementsPerBlock, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint8_t *>(weights),
        rows,
        cols,
        row_stride,
        static_cast<const int *>(decode_params),
        static_cast<float *>(output),
        Mxfp4Dequant{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_cast_f32_to_f16(
    const void *input,
    int element_count,
    void *output,
    void *stream
) {
    const int blocks = (element_count + kBlockSize - 1) / kBlockSize;
    cast_f32_to_f16_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        element_count,
        static_cast<__half *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_cast_f32_to_bf16(
    const void *input,
    int element_count,
    void *output,
    void *stream
) {
    const int blocks = (element_count + kBlockSize - 1) / kBlockSize;
    cast_f32_to_bf16_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        element_count,
        static_cast<__nv_bfloat16 *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_cast_bf16_to_f32(
    const void *input,
    int element_count,
    void *output,
    void *stream
) {
    const int blocks = (element_count + kBlockSize - 1) / kBlockSize;
    cast_bf16_to_f32_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const __nv_bfloat16 *>(input),
        element_count,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_gather_f16_row_to_f32(
    const void *input,
    int rows,
    int cols,
    const void *decode_params,
    void *output,
    void *stream
) {
    const int blocks = (cols + kBlockSize - 1) / kBlockSize;
    gather_f16_row_to_f32_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const __half *>(input),
        rows,
        cols,
        static_cast<const int *>(decode_params),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_gather_f32_by_indices(
    const void *input,
    int input_len,
    const void *indices,
    int index_count,
    void *output,
    void *stream
) {
    const int blocks = (index_count + kBlockSize - 1) / kBlockSize;
    gather_f32_by_indices_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        input_len,
        static_cast<const int *>(indices),
        index_count,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_q8_0_matvec_q8_1(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    const int block_count = cols / kQ81ElementsPerBlock;
    constexpr int rows_per_block = 1;
    constexpr int warps_per_row = 2;
    const dim3 block_dims(kWarpSize, rows_per_block * warps_per_row, 1);
    quantized_matvec_q8_1_grouped_mmvq_kernel<
        Q80Q81DotFixedMask,
        kQ80Q81MmvqVdr,
        kQ80Qi,
        rows_per_block,
        warps_per_row><<<
        (rows + rows_per_block - 1) / rows_per_block,
        block_dims,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const uint8_t *>(weights),
        row_stride,
        rows,
        block_count,
        static_cast<const Q81Block *>(input_q8_1),
        static_cast<const float *>(bias),
        static_cast<float *>(output),
        Q80Q81DotFixedMask{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_mxfp4_matvec_q8_1(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    const int block_count = cols / kQ81ElementsPerBlock;
    const dim3 block_dims(kWarpSize, kMmvqWarps, 1);
    quantized_matvec_q8_1_mmvq_kernel<Mxfp4Q81Dot, kMxfp4Q81MmvqVdr, kMxfp4Qi><<<
        rows,
        block_dims,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const uint8_t *>(weights),
        row_stride,
        rows,
        block_count,
        static_cast<const Q81Block *>(input_q8_1),
        static_cast<const float *>(bias),
        static_cast<float *>(output),
        Mxfp4Q81Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_q4_k_matvec_q8_1(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    const int block_count = cols / 256;
    const dim3 block_dims(kWarpSize, kMmvqWarps, 1);
    quantized_matvec_q8_1_mmvq_kernel<Q4KQ81Dot, kQ4KQ81MmvqVdr, kQ4KQi><<<
        rows,
        block_dims,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const uint8_t *>(weights),
        row_stride,
        rows,
        block_count,
        static_cast<const Q81Block *>(input_q8_1),
        static_cast<const float *>(bias),
        static_cast<float *>(output),
        Q4KQ81Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_q6_k_matvec_q8_1(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    const int block_count = cols / 256;
    const dim3 block_dims(kWarpSize, kMmvqWarps, 1);
    quantized_matvec_q8_1_mmvq_kernel<Q6KQ81Dot, kQ6KQ81MmvqVdr, kQ6KQi><<<
        rows,
        block_dims,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const uint8_t *>(weights),
        row_stride,
        rows,
        block_count,
        static_cast<const Q81Block *>(input_q8_1),
        static_cast<const float *>(bias),
        static_cast<float *>(output),
        Q6KQ81Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_q8_0_matvec_q8_1_argmax(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    constexpr int rows_per_block = 2;
    constexpr int warps_per_row = 2;
    launch_quantized_matvec_q8_1_grouped_argmax_mmvq<
        Q80Q81Dot,
        kQ80Q81MmvqVdr,
        kQ80Qi,
        rows_per_block,
        warps_per_row>(
        static_cast<const uint8_t *>(weights),
        rows,
        cols / kQ81ElementsPerBlock,
        row_stride,
        static_cast<const Q81Block *>(input_q8_1),
        static_cast<const float *>(bias),
        static_cast<unsigned long long *>(output),
        static_cast<cudaStream_t>(stream),
        Q80Q81Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_q4_k_matvec_q8_1_argmax(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    constexpr int rows_per_block = 4;
    constexpr int warps_per_row = 1;
    launch_quantized_matvec_q8_1_grouped_argmax_mmvq<
        Q4KQ81Dot,
        kQ4KQ81MmvqVdr,
        kQ4KQi,
        rows_per_block,
        warps_per_row>(
        static_cast<const uint8_t *>(weights),
        rows,
        cols / 256,
        row_stride,
        static_cast<const Q81Block *>(input_q8_1),
        static_cast<const float *>(bias),
        static_cast<unsigned long long *>(output),
        static_cast<cudaStream_t>(stream),
        Q4KQ81Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_q6_k_matvec_q8_1_argmax(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    constexpr int rows_per_block = 2;
    constexpr int warps_per_row = 2;
    launch_quantized_matvec_q8_1_grouped_argmax_mmvq<
        Q6KQ81Dot,
        kQ6KQ81MmvqVdr,
        kQ6KQi,
        rows_per_block,
        warps_per_row>(
        static_cast<const uint8_t *>(weights),
        rows,
        cols / 256,
        row_stride,
        static_cast<const Q81Block *>(input_q8_1),
        static_cast<const float *>(bias),
        static_cast<unsigned long long *>(output),
        static_cast<cudaStream_t>(stream),
        Q6KQ81Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_mxfp4_matvec_q8_1_argmax(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    launch_quantized_matvec_q8_1_argmax_mmvq<Mxfp4Q81Dot, kMxfp4Q81MmvqVdr, kMxfp4Qi>(
        static_cast<const uint8_t *>(weights),
        rows,
        cols / kQ81ElementsPerBlock,
        row_stride,
        static_cast<const Q81Block *>(input_q8_1),
        static_cast<const float *>(bias),
        static_cast<unsigned long long *>(output),
        static_cast<cudaStream_t>(stream),
        Mxfp4Q81Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_rms_norm(
    const void *input,
    const void *weight,
    int element_count,
    int feature_count,
    float epsilon,
    void *output,
    void *stream
) {
    if (feature_count <= 0 || element_count <= 0 || element_count % feature_count != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    rms_norm_kernel<<<
        element_count / feature_count,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        static_cast<const float *>(weight),
        feature_count,
        epsilon,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_rms_norm_region(
    const void *input,
    int input_offset,
    const void *weight,
    int element_count,
    int feature_count,
    float epsilon,
    void *output,
    int output_offset,
    void *stream
) {
    if (feature_count <= 0 || element_count <= 0 || element_count % feature_count != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    rms_norm_region_kernel<<<
        element_count / feature_count,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        input_offset,
        static_cast<const float *>(weight),
        feature_count,
        epsilon,
        static_cast<float *>(output),
        output_offset
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_parameter_golf_projection_loss(
    const void *logits,
    const void *target_ids,
    int row_count,
    int vocab_size,
    float logit_softcap,
    void *output,
    void *stream
) {
    if (row_count <= 0 || vocab_size <= 0 || !isfinite(logit_softcap) || logit_softcap <= 0.0f) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    cudaError_t status = cudaMemsetAsync(
        output,
        0,
        sizeof(float),
        static_cast<cudaStream_t>(stream)
    );
    if (status != cudaSuccess) {
        return static_cast<int>(status);
    }
    parameter_golf_projection_loss_kernel<<<
        row_count,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(logits),
        static_cast<const int *>(target_ids),
        row_count,
        vocab_size,
        logit_softcap,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_parameter_golf_projection_token_losses(
    const void *logits,
    const void *target_ids,
    int row_count,
    int vocab_size,
    float logit_softcap,
    void *output,
    cudaStream_t stream
) {
    if (row_count <= 0 || vocab_size <= 0 || !isfinite(logit_softcap) || logit_softcap <= 0.0f) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    const dim3 grid(static_cast<unsigned>(row_count));
    const dim3 block(static_cast<unsigned>(kBlockSize));
    parameter_golf_projection_token_losses_kernel<<<
        grid,
        block,
        0,
        stream
    >>>(
        static_cast<const float *>(logits),
        static_cast<const int *>(target_ids),
        row_count,
        vocab_size,
        logit_softcap,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_parameter_golf_projection_loss_backward(
    const void *logits,
    const void *target_ids,
    const void *grad_output,
    int row_count,
    int vocab_size,
    float logit_softcap,
    void *output,
    void *stream
) {
    if (row_count <= 0 || vocab_size <= 0 || !isfinite(logit_softcap) || logit_softcap <= 0.0f) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    parameter_golf_projection_loss_backward_kernel<<<
        row_count,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(logits),
        static_cast<const int *>(target_ids),
        static_cast<const float *>(grad_output),
        row_count,
        vocab_size,
        logit_softcap,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_parameter_golf_token_embedding_lookup(
    const void *token_ids,
    const void *token_embedding,
    int row_count,
    int vocab_size,
    int width,
    void *output,
    void *stream
) {
    if (row_count <= 0 || vocab_size <= 0 || width <= 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    parameter_golf_token_embedding_lookup_kernel<<<
        row_count,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const int *>(token_ids),
        static_cast<const float *>(token_embedding),
        row_count,
        vocab_size,
        width,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_parameter_golf_token_embedding_lookup_bf16_to_f32(
    const void *token_ids,
    const void *token_embedding,
    int row_count,
    int vocab_size,
    int width,
    void *output,
    void *stream
) {
    if (row_count <= 0 || vocab_size <= 0 || width <= 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    parameter_golf_token_embedding_lookup_bf16_to_f32_kernel<<<
        row_count,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const int *>(token_ids),
        static_cast<const __nv_bfloat16 *>(token_embedding),
        row_count,
        vocab_size,
        width,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_parameter_golf_token_embedding_lookup_backward(
    const void *token_ids,
    const void *grad_output,
    int row_count,
    int vocab_size,
    int width,
    void *output,
    void *stream
) {
    if (row_count <= 0 || vocab_size <= 0 || width <= 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    cudaError_t status = cudaMemsetAsync(
        output,
        0,
        static_cast<size_t>(vocab_size) * static_cast<size_t>(width) * sizeof(float),
        static_cast<cudaStream_t>(stream)
    );
    if (status != cudaSuccess) {
        return static_cast<int>(status);
    }
    parameter_golf_token_embedding_lookup_backward_kernel<<<
        row_count,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const int *>(token_ids),
        static_cast<const float *>(grad_output),
        row_count,
        vocab_size,
        width,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_rms_norm_q8_1(
    const void *input,
    const void *weight,
    int element_count,
    float epsilon,
    void *output,
    void *stream
) {
    if (element_count % kQ81ElementsPerBlock != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    rms_norm_q8_1_kernel<<<1, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        static_cast<const float *>(weight),
        element_count,
        epsilon,
        static_cast<Q81Block *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_rms_norm_input_backward(
    const void *input,
    const void *weight,
    const void *grad_output,
    int element_count,
    int feature_count,
    float epsilon,
    void *output,
    void *stream
) {
    if (feature_count <= 0 || element_count <= 0 || element_count % feature_count != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    rms_norm_input_backward_kernel<<<
        element_count / feature_count,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        static_cast<const float *>(weight),
        static_cast<const float *>(grad_output),
        feature_count,
        epsilon,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_rms_norm_weight_backward(
    const void *input,
    const void *grad_output,
    int element_count,
    int feature_count,
    float epsilon,
    void *output,
    void *stream
) {
    if (feature_count <= 0 || element_count <= 0 || element_count % feature_count != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    rms_norm_weight_backward_kernel<<<1, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        static_cast<const float *>(grad_output),
        element_count / feature_count,
        feature_count,
        epsilon,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_add_residual_rms_norm(
    const void *input,
    const void *residual,
    const void *input_bias,
    const void *weight,
    int element_count,
    float epsilon,
    void *summed_output,
    void *normalized_output,
    void *stream
) {
    add_residual_rms_norm_kernel<<<1, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        static_cast<const float *>(residual),
        static_cast<const float *>(input_bias),
        static_cast<const float *>(weight),
        element_count,
        epsilon,
        static_cast<float *>(summed_output),
        static_cast<float *>(normalized_output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_add_residual_rms_norm_q8_1(
    const void *input,
    const void *residual,
    const void *input_bias,
    const void *weight,
    int element_count,
    float epsilon,
    void *summed_output,
    void *normalized_output,
    void *quantized_output,
    void *stream
) {
    if (element_count % kQ81ElementsPerBlock != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    add_residual_rms_norm_q8_1_kernel<<<1, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        static_cast<const float *>(residual),
        static_cast<const float *>(input_bias),
        static_cast<const float *>(weight),
        element_count,
        epsilon,
        static_cast<float *>(summed_output),
        static_cast<float *>(normalized_output),
        static_cast<Q81Block *>(quantized_output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_add_residual_rms_norm_q8_1_router_topk(
    const void *input,
    const void *residual,
    const void *input_bias,
    const void *weight,
    int element_count,
    float epsilon,
    void *summed_output,
    void *normalized_output,
    void *quantized_output,
    const void *router_weights,
    const void *router_bias,
    int expert_count,
    int top_k,
    void *selected_ids,
    void *selected_weights,
    void *stream
) {
    if (element_count % kQ81ElementsPerBlock != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    add_residual_rms_norm_q8_1_router_topk_kernel<<<
        1,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        static_cast<const float *>(residual),
        static_cast<const float *>(input_bias),
        static_cast<const float *>(weight),
        element_count,
        epsilon,
        static_cast<float *>(summed_output),
        static_cast<float *>(normalized_output),
        static_cast<Q81Block *>(quantized_output),
        static_cast<const float *>(router_weights),
        static_cast<const float *>(router_bias),
        expert_count,
        top_k,
        static_cast<int32_t *>(selected_ids),
        static_cast<float *>(selected_weights)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_argmax_f32(
    const void *input,
    int rows,
    int cols,
    void *output,
    void *stream
) {
    const int warp_aligned_columns = ((cols + kWarpSize - 1) / kWarpSize) * kWarpSize;
    const int thread_count = warp_aligned_columns > 1024 ? 1024 : warp_aligned_columns;
    argmax_f32_kernel<<<rows, thread_count, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        static_cast<int32_t *>(output),
        rows,
        cols
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_top_k_f32(
    const void *input,
    int rows,
    int cols,
    int top_k,
    void *selected_indices,
    void *selected_values,
    void *stream
) {
    top_k = min(top_k, min(cols, kLogitsMaxSelected));
    if (top_k <= 0) {
        return 0;
    }
    top_k_f32_kernel<<<rows, kLogitsTopKBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        static_cast<int32_t *>(selected_indices),
        static_cast<float *>(selected_values),
        rows,
        cols,
        top_k
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_top_k_f32_one_row_partitioned(
    const void *input,
    int cols,
    int top_k,
    int partial_block_count,
    void *partial_indices,
    void *partial_values,
    void *selected_indices,
    void *selected_values,
    void *stream
) {
    if (input == nullptr || partial_indices == nullptr || partial_values == nullptr ||
        selected_indices == nullptr || selected_values == nullptr || cols <= 0 || top_k <= 0 ||
        partial_block_count <= 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }

    top_k = min(top_k, min(cols, kLogitsMaxSelected));
    if (top_k <= 0) {
        return 0;
    }

    const int tile_count = max(1, (cols + kLogitsTopKTileSize - 1) / kLogitsTopKTileSize);
    partial_block_count = min(partial_block_count, tile_count);

    const cudaStream_t cuda_stream = static_cast<cudaStream_t>(stream);
    if (partial_block_count <= 1 || cols <= kLogitsTopKTileSize) {
        top_k_f32_kernel<<<1, kLogitsTopKBlockSize, 0, cuda_stream>>>(
            static_cast<const float *>(input),
            static_cast<int32_t *>(selected_indices),
            static_cast<float *>(selected_values),
            1,
            cols,
            top_k
        );
        return static_cast<int>(cudaGetLastError());
    }

    top_k_f32_one_row_partial_kernel<<<partial_block_count, kLogitsTopKBlockSize, 0, cuda_stream>>>(
        static_cast<const float *>(input),
        static_cast<int32_t *>(partial_indices),
        static_cast<float *>(partial_values),
        cols,
        top_k
    );
    cudaError_t status = cudaGetLastError();
    if (status != cudaSuccess) {
        return static_cast<int>(status);
    }

    top_k_f32_kernel<<<1, kLogitsTopKBlockSize, 0, cuda_stream>>>(
        static_cast<const float *>(partial_values),
        static_cast<int32_t *>(selected_indices),
        static_cast<float *>(selected_values),
        1,
        partial_block_count * top_k,
        top_k
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        return static_cast<int>(status);
    }

    const int remap_blocks = (top_k + kBlockSize - 1) / kBlockSize;
    remap_top_k_indices_kernel<<<remap_blocks, kBlockSize, 0, cuda_stream>>>(
        static_cast<const int32_t *>(partial_indices),
        static_cast<int32_t *>(selected_indices),
        top_k
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_apply_sampling_penalties_f32_sparse(
    void *logits,
    int vocab_size,
    const void *token_ids,
    const void *token_counts,
    int active_token_count,
    float repeat_penalty,
    float presence_penalty,
    float frequency_penalty,
    void *stream
) {
    if (logits == nullptr || token_ids == nullptr || token_counts == nullptr || vocab_size <= 0 ||
        active_token_count < 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    if (active_token_count == 0) {
        return 0;
    }

    const int blocks = (active_token_count + kBlockSize - 1) / kBlockSize;
    apply_sampling_penalties_f32_sparse_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<float *>(logits),
        vocab_size,
        static_cast<const int32_t *>(token_ids),
        static_cast<const int32_t *>(token_counts),
        active_token_count,
        repeat_penalty,
        presence_penalty,
        frequency_penalty
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_top_k_f32_one_row_radix_sort_temp_storage_bytes(
    int cols,
    size_t *temp_storage_bytes
) {
    if (cols <= 0 || temp_storage_bytes == nullptr) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    *temp_storage_bytes = 0;
    return static_cast<int>(cub::DeviceRadixSort::SortPairsDescending(
        nullptr,
        *temp_storage_bytes,
        static_cast<const float *>(nullptr),
        static_cast<float *>(nullptr),
        static_cast<const int32_t *>(nullptr),
        static_cast<int32_t *>(nullptr),
        cols,
        0,
        sizeof(float) * 8,
        nullptr
    ));
}

extern "C" int psionic_cuda_top_k_f32_one_row_radix_sort(
    const void *input,
    int cols,
    int top_k,
    const void *input_indices,
    void *sorted_values,
    void *sorted_indices,
    void *temp_storage,
    size_t temp_storage_bytes,
    void *selected_indices,
    void *selected_values,
    void *stream
) {
    if (input == nullptr || input_indices == nullptr || sorted_values == nullptr ||
        sorted_indices == nullptr || temp_storage == nullptr || selected_indices == nullptr ||
        selected_values == nullptr || cols <= 0 || top_k <= 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    top_k = min(top_k, cols);
    cudaError_t status = cub::DeviceRadixSort::SortPairsDescending(
        temp_storage,
        temp_storage_bytes,
        static_cast<const float *>(input),
        static_cast<float *>(sorted_values),
        static_cast<const int32_t *>(input_indices),
        static_cast<int32_t *>(sorted_indices),
        cols,
        0,
        sizeof(float) * 8,
        static_cast<cudaStream_t>(stream)
    );
    if (status != cudaSuccess) {
        return static_cast<int>(status);
    }
    const int block = top_k > kBlockSize ? kBlockSize : top_k;
    const int grid = (top_k + block - 1) / block;
    copy_top_k_pairs_kernel<<<grid, block, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const int32_t *>(sorted_indices),
        static_cast<const float *>(sorted_values),
        top_k,
        static_cast<int32_t *>(selected_indices),
        static_cast<float *>(selected_values)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_add_f32_offset_in_place(
    void *destination,
    int element_offset,
    const void *rhs,
    int element_count,
    void *stream
) {
    const int blocks = (element_count + kBlockSize - 1) / kBlockSize;
    add_f32_offset_in_place_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<float *>(destination),
        element_offset,
        static_cast<const float *>(rhs),
        element_count
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_mul_f32(
    const void *left,
    const void *right,
    int element_count,
    void *output,
    void *stream
) {
    const int blocks = (element_count + kBlockSize - 1) / kBlockSize;
    mul_f32_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(left),
        static_cast<const float *>(right),
        element_count,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_silu_mul_f32(
    const void *activation_input,
    int activation_offset,
    const void *rhs,
    int rhs_offset,
    int element_count,
    void *output,
    void *stream
) {
    const int blocks = (element_count + kBlockSize - 1) / kBlockSize;
    silu_mul_f32_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(activation_input),
        activation_offset,
        static_cast<const float *>(rhs),
        rhs_offset,
        element_count,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_silu_mul_q8_1(
    const void *activation_input,
    int activation_offset,
    const void *rhs,
    int rhs_offset,
    int element_count,
    void *output_q8_1,
    void *stream
) {
    const int block_count = element_count / kQ81ElementsPerBlock;
    const int blocks_per_cta = kBlockSize / kWarpSize;
    const int blocks = (block_count + blocks_per_cta - 1) / blocks_per_cta;
    silu_mul_q8_1_kernel<<<
        blocks,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(activation_input),
        activation_offset,
        static_cast<const float *>(rhs),
        rhs_offset,
        element_count,
        static_cast<Q81Block *>(output_q8_1)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_sigmoid_mul_f32(
    const void *values,
    int values_offset,
    const void *gate,
    int gate_offset,
    int element_count,
    void *output,
    void *stream
) {
    const int blocks = (element_count + kBlockSize - 1) / kBlockSize;
    sigmoid_mul_f32_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(values),
        values_offset,
        static_cast<const float *>(gate),
        gate_offset,
        element_count,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_sigmoid_mul_q8_1(
    const void *values,
    int values_offset,
    const void *gate,
    int gate_offset,
    int element_count,
    void *output_q8_1,
    void *stream
) {
    const int block_count = element_count / kQ81ElementsPerBlock;
    const int blocks_per_cta = kBlockSize / kWarpSize;
    const int blocks = (block_count + blocks_per_cta - 1) / blocks_per_cta;
    sigmoid_mul_q8_1_kernel<<<
        blocks,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(values),
        values_offset,
        static_cast<const float *>(gate),
        gate_offset,
        element_count,
        static_cast<Q81Block *>(output_q8_1)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_split_interleaved_query_gate_f32(
    const void *input,
    int head_count,
    int head_dim,
    void *query_output,
    void *gate_output,
    void *stream
) {
    const int element_count = head_count * head_dim;
    const int blocks = (element_count + kBlockSize - 1) / kBlockSize;
    split_interleaved_query_gate_f32_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        head_count,
        head_dim,
        static_cast<float *>(query_output),
        static_cast<float *>(gate_output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_split_interleaved_query_gate_rms_norm_f32(
    const void *input,
    int head_count,
    int head_dim,
    const void *weight,
    float epsilon,
    void *query_output,
    void *gate_output,
    void *stream
) {
    split_interleaved_query_gate_rms_norm_f32_kernel<<<head_count, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        head_count,
        head_dim,
        static_cast<const float *>(weight),
        epsilon,
        static_cast<float *>(query_output),
        static_cast<float *>(gate_output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_pack_qwen35_key_value_rms_norm_f32(
    const void *input,
    int key_offset,
    int value_offset,
    int kv_head_count,
    int head_dim,
    const void *weight,
    float epsilon,
    void *output,
    int output_key_offset,
    int output_value_offset,
    void *stream
) {
    pack_qwen35_key_value_rms_norm_f32_kernel<<<kv_head_count, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        key_offset,
        value_offset,
        kv_head_count,
        head_dim,
        static_cast<const float *>(weight),
        epsilon,
        static_cast<float *>(output),
        output_key_offset,
        output_value_offset
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_pack_qwen35_hybrid_qkv_rms_norm_f32(
    const void *input,
    int q_offset,
    int k_offset,
    int v_offset,
    int group_count,
    int state_size,
    int v_size,
    const void *q_weight,
    const void *k_weight,
    float epsilon,
    void *output,
    int output_q_offset,
    int output_k_offset,
    int output_v_offset,
    void *stream
) {
    if (group_count <= 0 || state_size <= 0 || v_size < 0 || !isfinite(epsilon) || epsilon <= 0.0f) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    const int v_blocks = (v_size + kBlockSize - 1) / kBlockSize;
    const int block_count = max(group_count, v_blocks);
    pack_qwen35_hybrid_qkv_rms_norm_f32_kernel<<<
        block_count,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        q_offset,
        k_offset,
        v_offset,
        group_count,
        state_size,
        v_size,
        static_cast<const float *>(q_weight),
        static_cast<const float *>(k_weight),
        epsilon,
        static_cast<float *>(output),
        output_q_offset,
        output_k_offset,
        output_v_offset
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_depthwise_causal_conv1d_step_f32(
    const void *input,
    void *state,
    const void *weights,
    int channels,
    int kernel_size,
    void *output,
    void *stream
) {
    const int blocks = (channels + kBlockSize - 1) / kBlockSize;
    depthwise_causal_conv1d_step_f32_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        static_cast<float *>(state),
        static_cast<const float *>(weights),
        channels,
        kernel_size,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_depthwise_causal_conv1d_step_silu_f32(
    const void *input,
    void *state,
    const void *weights,
    int channels,
    int kernel_size,
    void *output,
    void *stream
) {
    const int blocks = (channels + kBlockSize - 1) / kBlockSize;
    depthwise_causal_conv1d_step_silu_f32_kernel<<<
        blocks,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        static_cast<float *>(state),
        static_cast<const float *>(weights),
        channels,
        kernel_size,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_qwen35_ssm_decay_beta_f32(
    const void *input,
    int alpha_offset,
    int beta_offset,
    const void *ssm_a,
    const void *ssm_dt,
    int element_count,
    void *decay_output,
    void *beta_output,
    void *stream
) {
    const int blocks = (element_count + kBlockSize - 1) / kBlockSize;
    qwen35_ssm_decay_beta_f32_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        alpha_offset,
        beta_offset,
        static_cast<const float *>(ssm_a),
        static_cast<const float *>(ssm_dt),
        element_count,
        static_cast<float *>(decay_output),
        static_cast<float *>(beta_output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_gated_delta_step_f32(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    const void *decay,
    const void *beta,
    void *state,
    int key_head_count,
    int value_head_count,
    int key_dim,
    int value_dim,
    int v_head_reordered,
    void *output,
    void *stream
) {
    dim3 block(kWarpSize, 1, 1);
    dim3 grid(1, value_dim, value_head_count);
    gated_delta_step_f32_kernel<<<grid, block, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(qkv),
        query_offset,
        key_offset,
        value_offset,
        static_cast<const float *>(decay),
        static_cast<const float *>(beta),
        static_cast<float *>(state),
        key_head_count,
        value_head_count,
        key_dim,
        value_dim,
        v_head_reordered,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_rope_neox_in_place(
    void *values,
    int element_offset,
    int head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    void *stream
) {
    rope_neox_in_place_kernel<<<head_count, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<float *>(values),
        element_offset,
        head_count,
        head_dim,
        rotary_dim,
        position,
        freq_scale,
        ext_factor,
        corr_low,
        corr_high,
        theta_scale
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_decode_mrope_cache_f16_kv(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position_t,
    int position_h,
    int position_w,
    int section_t,
    int section_h,
    int section_w,
    int section_extra,
    int mrope_interleaved,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    const size_t shared_bytes = static_cast<size_t>(head_dim) * 2 * sizeof(float);
    attention_decode_rope_cache_f16_kv_kernel<<<
        head_count,
        kAttentionBlockSize,
        shared_bytes,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(qkv),
        query_offset,
        key_offset,
        value_offset,
        static_cast<__half *>(cache_keys),
        static_cast<__half *>(cache_values),
        cache_width,
        layer_offset,
        past_tokens,
        sliding_window,
        head_count,
        kv_head_count,
        head_dim,
        rotary_dim,
        position_t,
        position_h,
        position_w,
        section_t,
        section_h,
        section_w,
        section_extra,
        mrope_interleaved,
        freq_scale,
        ext_factor,
        corr_low,
        corr_high,
        theta_scale,
        static_cast<const float *>(attention_sinks),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_rotary_embedding_backward(
    const void *grad_output,
    const void *cos,
    const void *sin,
    int batch_size,
    int head_count,
    int sequence_length,
    int head_dim,
    int batched_tables,
    void *grad_input,
    void *stream
) {
    dim3 grid(
        static_cast<unsigned int>(batch_size * head_count),
        static_cast<unsigned int>(sequence_length),
        1
    );
    rotary_embedding_backward_kernel<<<
        grid,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(grad_output),
        static_cast<const float *>(cos),
        static_cast<const float *>(sin),
        batch_size,
        head_count,
        sequence_length,
        head_dim,
        batched_tables,
        static_cast<float *>(grad_input)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_permute_rank2_transpose_f32(
    const void *input,
    int rows,
    int cols,
    void *output,
    void *stream
) {
    const dim3 block(16, 16, 1);
    const dim3 grid(
        static_cast<unsigned int>((cols + block.x - 1) / block.x),
        static_cast<unsigned int>((rows + block.y - 1) / block.y),
        1
    );
    permute_rank2_transpose_f32_kernel<<<
        grid,
        block,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        rows,
        cols,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_permute_rank4_swap_middle_axes_f32(
    const void *input,
    int dim0,
    int dim1,
    int dim2,
    int dim3,
    void *output,
    void *stream
) {
    const int total = dim0 * dim1 * dim2 * dim3;
    const int blocks = (total + kBlockSize - 1) / kBlockSize;
    permute_rank4_swap_middle_axes_f32_kernel<<<
        blocks,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        dim0,
        dim1,
        dim2,
        dim3,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_reduce_sum_rows_f32(
    const void *input,
    int row_count,
    int column_count,
    void *output,
    void *stream
) {
    reduce_sum_rows_f32_kernel<<<
        row_count,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        row_count,
        column_count,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_reduce_sum_axis0_f32(
    const void *input,
    int axis0_extent,
    int row_width,
    void *output,
    void *stream
) {
    const int blocks = (row_width + kBlockSize - 1) / kBlockSize;
    reduce_sum_axis0_f32_kernel<<<
        blocks,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        axis0_extent,
        row_width,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_reduce_sum_axis1_rank3_f32(
    const void *input,
    int dim0,
    int dim1,
    int dim2,
    void *output,
    void *stream
) {
    if (dim0 <= 0 || dim1 <= 0 || dim2 <= 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    const int total = dim0 * dim2;
    const int blocks = (total + kBlockSize - 1) / kBlockSize;
    reduce_sum_axis1_rank3_f32_kernel<<<
        blocks,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        dim0,
        dim1,
        dim2,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_expand_rank3_f32(
    const void *input,
    int input_dim0,
    int input_dim1,
    int input_dim2,
    int output_dim0,
    int output_dim1,
    int output_dim2,
    void *output,
    void *stream
) {
    const int total = output_dim0 * output_dim1 * output_dim2;
    const int blocks = (total + kBlockSize - 1) / kBlockSize;
    expand_rank3_f32_kernel<<<
        blocks,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        input_dim0,
        input_dim1,
        input_dim2,
        output_dim0,
        output_dim1,
        output_dim2,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_expand_rank4_f32(
    const void *input,
    int input_dim0,
    int input_dim1,
    int input_dim2,
    int input_dim3,
    int output_dim0,
    int output_dim1,
    int output_dim2,
    int output_dim3,
    void *output,
    void *stream
) {
    const int total = output_dim0 * output_dim1 * output_dim2 * output_dim3;
    const int blocks = (total + kBlockSize - 1) / kBlockSize;
    expand_rank4_f32_kernel<<<
        blocks,
        kBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        input_dim0,
        input_dim1,
        input_dim2,
        input_dim3,
        output_dim0,
        output_dim1,
        output_dim2,
        output_dim3,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_decode_rope_cache(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    const size_t shared_bytes = static_cast<size_t>(head_dim) * 2 * sizeof(float);
    attention_decode_rope_cache_kernel<<<
        head_count,
        kAttentionBlockSize,
        shared_bytes,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(qkv),
        query_offset,
        key_offset,
        value_offset,
        static_cast<float *>(cache_keys),
        static_cast<float *>(cache_values),
        cache_width,
        layer_offset,
        past_tokens,
        sliding_window,
        head_count,
        kv_head_count,
        head_dim,
        rotary_dim,
        position,
        freq_scale,
        ext_factor,
        corr_low,
        corr_high,
        theta_scale,
        static_cast<const float *>(attention_sinks),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_decode_rope_cache_f16_kv(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    const size_t shared_bytes = static_cast<size_t>(head_dim) * 2 * sizeof(float);
    attention_decode_rope_cache_f16_kv_kernel<<<
        head_count,
        kAttentionBlockSize,
        shared_bytes,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(qkv),
        query_offset,
        key_offset,
        value_offset,
        static_cast<__half *>(cache_keys),
        static_cast<__half *>(cache_values),
        cache_width,
        layer_offset,
        past_tokens,
        sliding_window,
        head_count,
        kv_head_count,
        head_dim,
        rotary_dim,
        position,
        position,
        position,
        0,
        0,
        0,
        0,
        0,
        freq_scale,
        ext_factor,
        corr_low,
        corr_high,
        theta_scale,
        static_cast<const float *>(attention_sinks),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_decode_rope_cache_turboquant_kv(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    const size_t shared_bytes = static_cast<size_t>(head_dim) * 2 * sizeof(float);
    attention_decode_rope_cache_turboquant_kv_kernel<<<
        head_count,
        kAttentionBlockSize,
        shared_bytes,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(qkv),
        query_offset,
        key_offset,
        value_offset,
        static_cast<Q81Block *>(cache_keys),
        static_cast<Q81Block *>(cache_values),
        cache_width,
        layer_offset,
        past_tokens,
        sliding_window,
        head_count,
        kv_head_count,
        head_dim,
        rotary_dim,
        position,
        freq_scale,
        ext_factor,
        corr_low,
        corr_high,
        theta_scale,
        static_cast<const float *>(attention_sinks),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_decode_rope_cache_f16_kv_q8_1(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output_q8_1,
    void *stream
) {
    const size_t shared_bytes = static_cast<size_t>(head_dim) * 2 * sizeof(float);
    attention_decode_rope_cache_f16_kv_q8_1_kernel<<<
        head_count,
        kAttentionBlockSize,
        shared_bytes,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(qkv),
        query_offset,
        key_offset,
        value_offset,
        static_cast<__half *>(cache_keys),
        static_cast<__half *>(cache_values),
        cache_width,
        layer_offset,
        past_tokens,
        sliding_window,
        head_count,
        kv_head_count,
        head_dim,
        rotary_dim,
        position,
        freq_scale,
        ext_factor,
        corr_low,
        corr_high,
        theta_scale,
        static_cast<const float *>(attention_sinks),
        static_cast<Q81Block *>(output_q8_1)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_decode_rope_cache_f16_kv_graph(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    const void *decode_params,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    const size_t shared_bytes = static_cast<size_t>(head_dim) * 2 * sizeof(float);
    attention_decode_rope_cache_f16_kv_graph_kernel<<<
        head_count,
        kAttentionBlockSize,
        shared_bytes,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(qkv),
        query_offset,
        key_offset,
        value_offset,
        static_cast<__half *>(cache_keys),
        static_cast<__half *>(cache_values),
        cache_width,
        layer_offset,
        static_cast<const int *>(decode_params),
        sliding_window,
        head_count,
        kv_head_count,
        head_dim,
        rotary_dim,
        freq_scale,
        ext_factor,
        corr_low,
        corr_high,
        theta_scale,
        static_cast<const float *>(attention_sinks),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_decode_rope_cache_f16_kv_graph_fa3(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    const void *decode_params,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *partial_output,
    void *partial_max,
    void *partial_sum,
    void *output,
    void *stream
) {
    if (head_count <= 0 || kv_head_count <= 0 || head_dim <= 0 || head_dim > kAttentionFa3MaxHeadDim ||
        head_count % kv_head_count != 0 ||
        head_count / kv_head_count > kAttentionFa3MaxHeadsPerGroup) {
        return 1;
    }
    dim3 split_grid(
        static_cast<unsigned int>(kv_head_count),
        static_cast<unsigned int>(kAttentionFa3MaxSplits),
        1u
    );
    const size_t shared_bytes = static_cast<size_t>(head_dim) *
        static_cast<size_t>((head_count / kv_head_count) + 1) * sizeof(float);
    attention_decode_rope_cache_f16_kv_graph_fa3_split_kernel<<<
        split_grid,
        kAttentionBlockSize,
        shared_bytes,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(qkv),
        query_offset,
        key_offset,
        value_offset,
        static_cast<__half *>(cache_keys),
        static_cast<__half *>(cache_values),
        cache_width,
        layer_offset,
        static_cast<const int *>(decode_params),
        sliding_window,
        head_count,
        kv_head_count,
        head_dim,
        rotary_dim,
        freq_scale,
        ext_factor,
        corr_low,
        corr_high,
        theta_scale,
        static_cast<const float *>(attention_sinks),
        static_cast<float *>(partial_output),
        static_cast<float *>(partial_max),
        static_cast<float *>(partial_sum)
    );
    const cudaError_t split_status = cudaGetLastError();
    if (split_status != cudaSuccess) {
        return static_cast<int>(split_status);
    }
    attention_decode_rope_cache_f16_kv_graph_fa3_combine_kernel<<<
        head_count,
        kAttentionBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(partial_output),
        static_cast<const float *>(partial_max),
        static_cast<const float *>(partial_sum),
        head_count,
        head_dim,
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_decode_rope_cache_turboquant_kv_graph(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    const void *decode_params,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    const size_t shared_bytes = static_cast<size_t>(head_dim) * 2 * sizeof(float);
    attention_decode_rope_cache_turboquant_kv_graph_kernel<<<
        head_count,
        kAttentionBlockSize,
        shared_bytes,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(qkv),
        query_offset,
        key_offset,
        value_offset,
        static_cast<Q81Block *>(cache_keys),
        static_cast<Q81Block *>(cache_values),
        cache_width,
        layer_offset,
        static_cast<const int *>(decode_params),
        sliding_window,
        head_count,
        kv_head_count,
        head_dim,
        rotary_dim,
        freq_scale,
        ext_factor,
        corr_low,
        corr_high,
        theta_scale,
        static_cast<const float *>(attention_sinks),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_decode_rope_cache_f16_kv_graph_q8_1(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    const void *decode_params,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output_q8_1,
    void *stream
) {
    const size_t shared_bytes = static_cast<size_t>(head_dim) * 2 * sizeof(float);
    attention_decode_rope_cache_f16_kv_graph_q8_1_kernel<<<
        head_count,
        kAttentionBlockSize,
        shared_bytes,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(qkv),
        query_offset,
        key_offset,
        value_offset,
        static_cast<__half *>(cache_keys),
        static_cast<__half *>(cache_values),
        cache_width,
        layer_offset,
        static_cast<const int *>(decode_params),
        sliding_window,
        head_count,
        kv_head_count,
        head_dim,
        rotary_dim,
        freq_scale,
        ext_factor,
        corr_low,
        corr_high,
        theta_scale,
        static_cast<const float *>(attention_sinks),
        static_cast<Q81Block *>(output_q8_1)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_decode(
    const void *query,
    int query_offset,
    const void *current_key,
    int key_offset,
    const void *current_value,
    int value_offset,
    const void *cache_keys,
    const void *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    attention_decode_kernel<<<head_count, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(query),
        query_offset,
        static_cast<const float *>(current_key),
        key_offset,
        static_cast<const float *>(current_value),
        value_offset,
        static_cast<const float *>(cache_keys),
        static_cast<const float *>(cache_values),
        cache_width,
        layer_offset,
        past_tokens,
        sliding_window,
        head_count,
        kv_head_count,
        head_dim,
        static_cast<const float *>(attention_sinks),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_causal_sequence_f32(
    const void *query,
    const void *key,
    const void *value,
    int batch_size,
    int head_count,
    int kv_head_count,
    int sequence_length,
    int head_dim,
    void *output,
    void *stream
) {
    const dim3 grid(
        static_cast<unsigned int>(head_count),
        static_cast<unsigned int>(sequence_length),
        static_cast<unsigned int>(batch_size)
    );
    attention_causal_sequence_kernel<float, float, float, float>
        <<<grid, kAttentionBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
            static_cast<const float *>(query),
            static_cast<const float *>(key),
            static_cast<const float *>(value),
            batch_size,
            head_count,
            kv_head_count,
            sequence_length,
            head_dim,
            static_cast<float *>(output)
        );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_causal_sequence_bf16(
    const void *query,
    const void *key,
    const void *value,
    int batch_size,
    int head_count,
    int kv_head_count,
    int sequence_length,
    int head_dim,
    void *output,
    void *stream
) {
    const dim3 grid(
        static_cast<unsigned int>(head_count),
        static_cast<unsigned int>(sequence_length),
        static_cast<unsigned int>(batch_size)
    );
    attention_causal_sequence_kernel<__nv_bfloat16, __nv_bfloat16, __nv_bfloat16, __nv_bfloat16>
        <<<grid, kAttentionBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
            static_cast<const __nv_bfloat16 *>(query),
            static_cast<const __nv_bfloat16 *>(key),
            static_cast<const __nv_bfloat16 *>(value),
            batch_size,
            head_count,
            kv_head_count,
            sequence_length,
            head_dim,
            static_cast<__nv_bfloat16 *>(output)
        );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_router_topk_softmax(
    const void *weights,
    const void *bias,
    const void *input,
    int expert_count,
    int input_size,
    int top_k,
    void *selected_ids,
    void *selected_weights,
    void *stream
) {
    if (expert_count == kWarpSize && top_k > 0 && top_k <= kMoeMaxSelected) {
        if (bias != nullptr) {
            router_topk_softmax_32_kernel<true><<<
                1,
                kWarpSize,
                0,
                static_cast<cudaStream_t>(stream)
            >>>(
                static_cast<const float *>(weights),
                static_cast<const float *>(bias),
                static_cast<const float *>(input),
                input_size,
                top_k,
                static_cast<int32_t *>(selected_ids),
                static_cast<float *>(selected_weights)
            );
        } else {
            router_topk_softmax_32_kernel<false><<<
                1,
                kWarpSize,
                0,
                static_cast<cudaStream_t>(stream)
            >>>(
                static_cast<const float *>(weights),
                static_cast<const float *>(bias),
                static_cast<const float *>(input),
                input_size,
                top_k,
                static_cast<int32_t *>(selected_ids),
                static_cast<float *>(selected_weights)
            );
        }
    } else {
        router_topk_softmax_kernel<<<1, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
            static_cast<const float *>(weights),
            static_cast<const float *>(bias),
            static_cast<const float *>(input),
            expert_count,
            input_size,
            top_k,
            static_cast<int32_t *>(selected_ids),
            static_cast<float *>(selected_weights)
        );
    }
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_router_topk_delayed_softmax(
    const void *logits,
    int expert_count,
    int top_k,
    void *selected_ids,
    void *selected_weights,
    void *stream
) {
    if (expert_count == kWarpSize && top_k > 0 && top_k <= kMoeMaxSelected) {
        router_logits_topk_delayed_softmax_32_kernel<<<1, kWarpSize, 0, static_cast<cudaStream_t>(stream)>>>(
            static_cast<const float *>(logits),
            top_k,
            static_cast<int32_t *>(selected_ids),
            static_cast<float *>(selected_weights)
        );
    } else {
        router_logits_topk_delayed_softmax_kernel<<<1, 1, 0, static_cast<cudaStream_t>(stream)>>>(
            static_cast<const float *>(logits),
            expert_count,
            top_k,
            static_cast<int32_t *>(selected_ids),
            static_cast<float *>(selected_weights)
        );
    }
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_moe_gate_up_swiglu(
    const void *weights,
    int mode,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const void *selected_ids,
    int selected_count,
    const void *input,
    const void *gate_bias,
    const void *up_bias,
    void *output,
    void *stream
) {
    const dim3 blocks(static_cast<unsigned int>(gate_rows), static_cast<unsigned int>(selected_count), 1);
    moe_gate_up_swiglu_kernel<<<blocks, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint8_t *>(weights),
        mode,
        row_stride,
        rows_per_expert,
        columns,
        gate_rows,
        up_rows,
        static_cast<const int32_t *>(selected_ids),
        selected_count,
        static_cast<const float *>(input),
        static_cast<const float *>(gate_bias),
        static_cast<const float *>(up_bias),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_causal_row_softmax_in_place_f32(
    void *logits,
    int row_count,
    int sequence_length,
    float scale,
    void *stream
) {
    if (row_count <= 0 || sequence_length <= 0 || sequence_length > kAttentionMaxPositions) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    attention_causal_row_softmax_in_place_f32_kernel<<<
        row_count,
        kAttentionBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<float *>(logits),
        row_count,
        sequence_length,
        scale
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_causal_row_softmax_backward_in_place_f32(
    const void *probabilities,
    void *grad_probabilities,
    int row_count,
    int sequence_length,
    float post_scale,
    void *stream
) {
    if (row_count <= 0 || sequence_length <= 0 || sequence_length > kAttentionMaxPositions) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    attention_causal_row_softmax_backward_in_place_f32_kernel<<<
        row_count,
        kAttentionBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(probabilities),
        static_cast<float *>(grad_probabilities),
        row_count,
        sequence_length,
        post_scale
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_causal_sequence_backward_f32(
    const void *query,
    const void *key,
    const void *value,
    const void *grad_output,
    int batch_size,
    int head_count,
    int kv_head_count,
    int sequence_length,
    int head_dim,
    void *query_gradient,
    void *key_gradient,
    void *value_gradient,
    void *stream
) {
    const dim3 grid(
        static_cast<unsigned int>(head_count),
        static_cast<unsigned int>(sequence_length),
        static_cast<unsigned int>(batch_size)
    );
    attention_causal_sequence_backward_to_f32_kernel<float, float, float, float><<<
        grid,
        kAttentionBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(query),
        static_cast<const float *>(key),
        static_cast<const float *>(value),
        static_cast<const float *>(grad_output),
        batch_size,
        head_count,
        kv_head_count,
        sequence_length,
        head_dim,
        static_cast<float *>(query_gradient),
        static_cast<float *>(key_gradient),
        static_cast<float *>(value_gradient)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_attention_causal_sequence_backward_bf16_to_f32(
    const void *query,
    const void *key,
    const void *value,
    const void *grad_output,
    int batch_size,
    int head_count,
    int kv_head_count,
    int sequence_length,
    int head_dim,
    void *query_gradient,
    void *key_gradient,
    void *value_gradient,
    void *stream
) {
    const dim3 grid(
        static_cast<unsigned int>(head_count),
        static_cast<unsigned int>(sequence_length),
        static_cast<unsigned int>(batch_size)
    );
    attention_causal_sequence_backward_to_f32_kernel<
        __nv_bfloat16,
        __nv_bfloat16,
        __nv_bfloat16,
        __nv_bfloat16><<<
        grid,
        kAttentionBlockSize,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const __nv_bfloat16 *>(query),
        static_cast<const __nv_bfloat16 *>(key),
        static_cast<const __nv_bfloat16 *>(value),
        static_cast<const __nv_bfloat16 *>(grad_output),
        batch_size,
        head_count,
        kv_head_count,
        sequence_length,
        head_dim,
        static_cast<float *>(query_gradient),
        static_cast<float *>(key_gradient),
        static_cast<float *>(value_gradient)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_moe_gate_up_swiglu_q8_1(
    const void *weights,
    int mode,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const void *selected_ids,
    int selected_count,
    const void *input_q8_1,
    const void *gate_bias,
    const void *up_bias,
    void *output,
    void *stream
) {
    if (selected_count <= 8) {
        if (selected_count <= 4) {
            if (mode == 0) {
                moe_gate_up_swiglu_q8_1_selected4_kernel<Q80Q81Dot, kQ80Q81MmvqVdr, kQ80Qi><<<
                    static_cast<unsigned int>((gate_rows + 1) / 2),
                    dim3(kWarpSize, 4, 1),
                    static_cast<size_t>(columns / kQ81ElementsPerBlock) * sizeof(Q81Block),
                    static_cast<cudaStream_t>(stream)
                >>>(
                    static_cast<const uint8_t *>(weights),
                    row_stride,
                    rows_per_expert,
                    columns,
                    gate_rows,
                    up_rows,
                    static_cast<const int32_t *>(selected_ids),
                    selected_count,
                    static_cast<const Q81Block *>(input_q8_1),
                    static_cast<const float *>(gate_bias),
                    static_cast<const float *>(up_bias),
                    static_cast<float *>(output),
                    Q80Q81Dot{}
                );
            } else {
                moe_gate_up_swiglu_q8_1_selected4_kernel<Mxfp4Q81Dot, kMxfp4Q81MmvqVdr, kMxfp4Qi><<<
                    static_cast<unsigned int>((gate_rows + 1) / 2),
                    dim3(kWarpSize, 4, 1),
                    static_cast<size_t>(columns / kQ81ElementsPerBlock) * sizeof(Q81Block),
                    static_cast<cudaStream_t>(stream)
                >>>(
                    static_cast<const uint8_t *>(weights),
                    row_stride,
                    rows_per_expert,
                    columns,
                    gate_rows,
                    up_rows,
                    static_cast<const int32_t *>(selected_ids),
                    selected_count,
                    static_cast<const Q81Block *>(input_q8_1),
                    static_cast<const float *>(gate_bias),
                    static_cast<const float *>(up_bias),
                    static_cast<float *>(output),
                    Mxfp4Q81Dot{}
                );
            }
        } else if (mode == 0) {
            const dim3 blocks(static_cast<unsigned int>((gate_rows + 1) / 2), 1, 1);
            const dim3 block_dims(kWarpSize, kMmvqWarps, 1);
            moe_gate_up_swiglu_q8_1_mmvq_grouped_selected_kernel<
                Q80Q81Dot,
                kQ80Q81MmvqVdr,
                kQ80Qi,
                8,
                2
            ><<<blocks, block_dims, 0, static_cast<cudaStream_t>(stream)>>>(
                static_cast<const uint8_t *>(weights),
                row_stride,
                rows_per_expert,
                columns,
                gate_rows,
                up_rows,
                static_cast<const int32_t *>(selected_ids),
                selected_count,
                static_cast<const Q81Block *>(input_q8_1),
                static_cast<const float *>(gate_bias),
                static_cast<const float *>(up_bias),
                static_cast<float *>(output),
                Q80Q81Dot{}
            );
        } else {
            const dim3 blocks(static_cast<unsigned int>((gate_rows + 1) / 2), 1, 1);
            const dim3 block_dims(kWarpSize, kMmvqWarps, 1);
            moe_gate_up_swiglu_q8_1_mmvq_grouped_selected_kernel<
                Mxfp4Q81Dot,
                kMxfp4Q81MmvqVdr,
                kMxfp4Qi,
                8,
                2
            ><<<blocks, block_dims, 0, static_cast<cudaStream_t>(stream)>>>(
                static_cast<const uint8_t *>(weights),
                row_stride,
                rows_per_expert,
                columns,
                gate_rows,
                up_rows,
                static_cast<const int32_t *>(selected_ids),
                selected_count,
                static_cast<const Q81Block *>(input_q8_1),
                static_cast<const float *>(gate_bias),
                static_cast<const float *>(up_bias),
                static_cast<float *>(output),
                Mxfp4Q81Dot{}
            );
        }
    } else {
        const dim3 blocks(static_cast<unsigned int>(gate_rows), static_cast<unsigned int>(selected_count), 1);
        moe_gate_up_swiglu_q8_1_kernel<<<blocks, kMatvecBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
            static_cast<const uint8_t *>(weights),
            mode,
            row_stride,
            rows_per_expert,
            columns,
            gate_rows,
            up_rows,
            static_cast<const int32_t *>(selected_ids),
            selected_count,
            static_cast<const Q81Block *>(input_q8_1),
            static_cast<const float *>(gate_bias),
            static_cast<const float *>(up_bias),
            static_cast<float *>(output)
        );
    }
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_moe_gate_up_swiglu_q8_1_selected4_quantized(
    const void *weights,
    int mode,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const void *selected_ids,
    int selected_count,
    const void *input_q8_1,
    const void *gate_bias,
    const void *up_bias,
    void *output_q8_1,
    void *stream
) {
    if (selected_count <= 0 || selected_count > 4 || gate_rows != up_rows ||
        gate_rows % kQ81ElementsPerBlock != 0 || columns % kQ81ElementsPerBlock != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }

    const dim3 blocks(
        static_cast<unsigned int>(gate_rows / kQ81ElementsPerBlock),
        static_cast<unsigned int>(selected_count),
        1
    );
    const dim3 block_dims(kWarpSize, 8, 1);
    const size_t shared_input_bytes =
        static_cast<size_t>(columns / kQ81ElementsPerBlock) * sizeof(Q81Block);
    if (mode == 0) {
        moe_gate_up_swiglu_q8_1_selected4_quantized_kernel<Q80Q81Dot, kQ80Q81MmvqVdr, kQ80Qi><<<
            blocks,
            block_dims,
            shared_input_bytes,
            static_cast<cudaStream_t>(stream)
        >>>(
            static_cast<const uint8_t *>(weights),
            row_stride,
            rows_per_expert,
            columns,
            gate_rows,
            up_rows,
            static_cast<const int32_t *>(selected_ids),
            selected_count,
            static_cast<const Q81Block *>(input_q8_1),
            static_cast<const float *>(gate_bias),
            static_cast<const float *>(up_bias),
            static_cast<Q81Block *>(output_q8_1),
            Q80Q81Dot{}
        );
    } else {
        moe_gate_up_swiglu_q8_1_selected4_quantized_kernel<Mxfp4Q81Dot, kMxfp4Q81MmvqVdr, kMxfp4Qi><<<
            blocks,
            block_dims,
            shared_input_bytes,
            static_cast<cudaStream_t>(stream)
        >>>(
            static_cast<const uint8_t *>(weights),
            row_stride,
            rows_per_expert,
            columns,
            gate_rows,
            up_rows,
            static_cast<const int32_t *>(selected_ids),
            selected_count,
            static_cast<const Q81Block *>(input_q8_1),
            static_cast<const float *>(gate_bias),
            static_cast<const float *>(up_bias),
            static_cast<Q81Block *>(output_q8_1),
            Mxfp4Q81Dot{}
        );
    }
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_moe_down_aggregate(
    const void *weights,
    int mode,
    int row_stride,
    int rows,
    int columns,
    const void *selected_ids,
    const void *selected_weights,
    int selected_count,
    const void *activated,
    const void *bias,
    const void *residual,
    void *output,
    void *stream
) {
    moe_down_aggregate_kernel<<<rows, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint8_t *>(weights),
        mode,
        row_stride,
        rows,
        columns,
        static_cast<const int32_t *>(selected_ids),
        static_cast<const float *>(selected_weights),
        selected_count,
        static_cast<const float *>(activated),
        static_cast<const float *>(bias),
        static_cast<const float *>(residual),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_moe_down_aggregate_q8_1(
    const void *weights,
    int mode,
    int row_stride,
    int rows,
    int columns,
    const void *selected_ids,
    const void *selected_weights,
    int selected_count,
    const void *activated_q8_1,
    const void *bias,
    const void *residual,
    void *output,
    void *stream
) {
    const int activated_block_stride = columns / kQ81ElementsPerBlock;
    if (selected_count <= 8) {
        if (selected_count <= 4) {
            const size_t shared_input_bytes =
                static_cast<size_t>(selected_count) *
                static_cast<size_t>(activated_block_stride) *
                sizeof(Q81Block);
            if (mode == 0) {
                moe_down_aggregate_q8_1_selected4_kernel<Q80Q81Dot, kQ80Q81MmvqVdr, kQ80Qi><<<
                    static_cast<unsigned int>((rows + 1) / 2),
                    dim3(kWarpSize, 4, 1),
                    shared_input_bytes,
                    static_cast<cudaStream_t>(stream)
                >>>(
                    static_cast<const uint8_t *>(weights),
                    row_stride,
                    rows,
                    static_cast<const int32_t *>(selected_ids),
                    static_cast<const float *>(selected_weights),
                    selected_count,
                    static_cast<const Q81Block *>(activated_q8_1),
                    activated_block_stride,
                    static_cast<const float *>(bias),
                    static_cast<const float *>(residual),
                    static_cast<float *>(output),
                    Q80Q81Dot{}
                );
            } else {
                moe_down_aggregate_q8_1_selected4_kernel<Mxfp4Q81Dot, kMxfp4Q81MmvqVdr, kMxfp4Qi><<<
                    static_cast<unsigned int>((rows + 1) / 2),
                    dim3(kWarpSize, 4, 1),
                    shared_input_bytes,
                    static_cast<cudaStream_t>(stream)
                >>>(
                    static_cast<const uint8_t *>(weights),
                    row_stride,
                    rows,
                    static_cast<const int32_t *>(selected_ids),
                    static_cast<const float *>(selected_weights),
                    selected_count,
                    static_cast<const Q81Block *>(activated_q8_1),
                    activated_block_stride,
                    static_cast<const float *>(bias),
                    static_cast<const float *>(residual),
                    static_cast<float *>(output),
                    Mxfp4Q81Dot{}
                );
            }
        } else if (mode == 0) {
            moe_down_aggregate_q8_1_mmvq_grouped_selected_kernel<
                Q80Q81Dot,
                kQ80Q81MmvqVdr,
                kQ80Qi,
                8,
                2
            ><<<
                static_cast<unsigned int>((rows + 1) / 2),
                dim3(kWarpSize, kMmvqWarps, 1),
                0,
                static_cast<cudaStream_t>(stream)
            >>>(
                static_cast<const uint8_t *>(weights),
                row_stride,
                rows,
                columns,
                static_cast<const int32_t *>(selected_ids),
                static_cast<const float *>(selected_weights),
                selected_count,
                static_cast<const Q81Block *>(activated_q8_1),
                static_cast<const float *>(bias),
                static_cast<const float *>(residual),
                static_cast<float *>(output),
                Q80Q81Dot{}
            );
        } else {
            moe_down_aggregate_q8_1_mmvq_grouped_selected_kernel<
                Mxfp4Q81Dot,
                kMxfp4Q81MmvqVdr,
                kMxfp4Qi,
                8,
                2
            ><<<
                static_cast<unsigned int>((rows + 1) / 2),
                dim3(kWarpSize, kMmvqWarps, 1),
                0,
                static_cast<cudaStream_t>(stream)
            >>>(
                static_cast<const uint8_t *>(weights),
                row_stride,
                rows,
                columns,
                static_cast<const int32_t *>(selected_ids),
                static_cast<const float *>(selected_weights),
                selected_count,
                static_cast<const Q81Block *>(activated_q8_1),
                static_cast<const float *>(bias),
                static_cast<const float *>(residual),
                static_cast<float *>(output),
                Mxfp4Q81Dot{}
            );
        }
    } else {
        if (residual != nullptr) {
            cudaMemcpyAsync(
                output,
                residual,
                static_cast<size_t>(rows) * sizeof(float),
                cudaMemcpyDeviceToDevice,
                static_cast<cudaStream_t>(stream)
            );
        } else {
            cudaMemsetAsync(
                output,
                0,
                static_cast<size_t>(rows) * sizeof(float),
                static_cast<cudaStream_t>(stream)
            );
        }
        const dim3 atomic_blocks(static_cast<unsigned int>(rows), static_cast<unsigned int>(selected_count), 1);
        if (mode == 0) {
            expert_down_aggregate_q8_1_atomic_kernel<Q80Q81Dot, kQ80Q81MmvqVdr, kQ80Qi><<<
                atomic_blocks,
                dim3(kWarpSize, kMmvqWarps, 1),
                0,
                static_cast<cudaStream_t>(stream)
            >>>(
                static_cast<const uint8_t *>(weights),
                row_stride,
                rows,
                rows,
                static_cast<const int32_t *>(selected_ids),
                static_cast<const float *>(selected_weights),
                selected_count,
                static_cast<const Q81Block *>(activated_q8_1),
                activated_block_stride,
                static_cast<const float *>(bias),
                static_cast<const float *>(residual),
                static_cast<float *>(output),
                Q80Q81Dot{}
            );
        } else {
            expert_down_aggregate_q8_1_atomic_kernel<Mxfp4Q81Dot, kMxfp4Q81MmvqVdr, kMxfp4Qi><<<
                atomic_blocks,
                dim3(kWarpSize, kMmvqWarps, 1),
                0,
                static_cast<cudaStream_t>(stream)
            >>>(
                static_cast<const uint8_t *>(weights),
                row_stride,
                rows,
                rows,
                static_cast<const int32_t *>(selected_ids),
                static_cast<const float *>(selected_weights),
                selected_count,
                static_cast<const Q81Block *>(activated_q8_1),
                activated_block_stride,
                static_cast<const float *>(bias),
                static_cast<const float *>(residual),
                static_cast<float *>(output),
                Mxfp4Q81Dot{}
            );
        }
    }
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_moe_down_aggregate_q8_1_f32(
    const void *weights,
    int mode,
    int row_stride,
    int rows,
    int columns,
    const void *selected_ids,
    const void *selected_weights,
    int selected_count,
    const void *activated,
    const void *bias,
    const void *residual,
    void *output,
    void *stream
) {
    if (selected_count > 4) {
        return 1;
    }
    const size_t shared_input_bytes =
        static_cast<size_t>(selected_count) *
        static_cast<size_t>(columns / kQ81ElementsPerBlock) *
        sizeof(Q81Block);
    if (mode == 0) {
        moe_down_aggregate_q8_1_selected4_f32_kernel<Q80Q81Dot, kQ80Q81MmvqVdr, kQ80Qi><<<
            static_cast<unsigned int>((rows + 1) / 2),
            dim3(kWarpSize, 4, 1),
            shared_input_bytes,
            static_cast<cudaStream_t>(stream)
        >>>(
            static_cast<const uint8_t *>(weights),
            row_stride,
            rows,
            columns,
            static_cast<const int32_t *>(selected_ids),
            static_cast<const float *>(selected_weights),
            selected_count,
            static_cast<const float *>(activated),
            static_cast<const float *>(bias),
            static_cast<const float *>(residual),
            static_cast<float *>(output),
            Q80Q81Dot{}
        );
    } else {
        moe_down_aggregate_q8_1_selected4_f32_kernel<Mxfp4Q81Dot, kMxfp4Q81MmvqVdr, kMxfp4Qi><<<
            static_cast<unsigned int>((rows + 1) / 2),
            dim3(kWarpSize, 4, 1),
            shared_input_bytes,
            static_cast<cudaStream_t>(stream)
        >>>(
            static_cast<const uint8_t *>(weights),
            row_stride,
            rows,
            columns,
            static_cast<const int32_t *>(selected_ids),
            static_cast<const float *>(selected_weights),
            selected_count,
            static_cast<const float *>(activated),
            static_cast<const float *>(bias),
            static_cast<const float *>(residual),
            static_cast<float *>(output),
            Mxfp4Q81Dot{}
        );
    }
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_moe_down_project_q8_1_selected4(
    const void *weights,
    int mode,
    int row_stride,
    int rows,
    int columns,
    const void *selected_ids,
    int selected_count,
    const void *activated_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    if (selected_count > 4) {
        return 1;
    }
    const dim3 blocks(static_cast<unsigned int>((rows + 1) / 2), 1, 1);
    const dim3 block_dims(kWarpSize, 4, 1);
    if (mode == 0) {
        expert_mul_mat_vec_q8_1_project_grouped_kernel<Q80Q81Dot, kQ80Q81MmvqVdr, kQ80Qi, 4, 2, 4><<<
            blocks,
            block_dims,
            0,
            static_cast<cudaStream_t>(stream)
        >>>(
            static_cast<const uint8_t *>(weights),
            row_stride,
            rows,
            rows,
            columns / kQ81ElementsPerBlock,
            static_cast<const int32_t *>(selected_ids),
            selected_count,
            static_cast<const Q81Block *>(activated_q8_1),
            columns / kQ81ElementsPerBlock,
            static_cast<const float *>(bias),
            static_cast<float *>(output),
            Q80Q81Dot{}
        );
    } else {
        expert_mul_mat_vec_q8_1_project_grouped_kernel<Mxfp4Q81Dot, kMxfp4Q81MmvqVdr, kMxfp4Qi, 4, 2, 4><<<
            blocks,
            block_dims,
            0,
            static_cast<cudaStream_t>(stream)
        >>>(
            static_cast<const uint8_t *>(weights),
            row_stride,
            rows,
            rows,
            columns / kQ81ElementsPerBlock,
            static_cast<const int32_t *>(selected_ids),
            selected_count,
            static_cast<const Q81Block *>(activated_q8_1),
            columns / kQ81ElementsPerBlock,
            static_cast<const float *>(bias),
            static_cast<float *>(output),
            Mxfp4Q81Dot{}
        );
    }
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_accumulate_selected4(
    const void *input,
    const void *selected_weights,
    int selected_count,
    int rows,
    const void *residual,
    void *output,
    void *stream
) {
    if (selected_count > 4) {
        return 1;
    }
    constexpr int kThreadsPerBlock = 128;
    const int block_count = (rows + kThreadsPerBlock - 1) / kThreadsPerBlock;
    accumulate_selected4_kernel<<<
        static_cast<unsigned int>(block_count),
        static_cast<unsigned int>(kThreadsPerBlock),
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const float *>(input),
        static_cast<const float *>(selected_weights),
        selected_count,
        rows,
        static_cast<const float *>(residual),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}
