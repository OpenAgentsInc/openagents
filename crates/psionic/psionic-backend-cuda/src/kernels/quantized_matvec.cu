#include <cuda_runtime.h>
#include <cuda_fp16.h>

#include <cfloat>
#include <stdint.h>

namespace {

constexpr int kBlockSize = 256;
constexpr int kMatvecBlockSize = 128;
constexpr int kWarpSize = 32;
constexpr int kMmvqWarps = kMatvecBlockSize / kWarpSize;
constexpr int kMaxWarpsPerBlock = 1024 / kWarpSize;
constexpr int kQ81ElementsPerBlock = 32;
constexpr int kQ80BlockBytes = 34;
constexpr int kQ81BlockBytes = 36;
constexpr int kAttentionMaxPositions = 513;
constexpr int kMoeMaxExperts = 128;
constexpr int kMoeMaxSelected = 32;

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
    const uint32_t bits = value < 2 ? (0x00200000u << value) : (static_cast<uint32_t>(value - 1) << 23);
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

constexpr int kQ80Q81MmvqVdr = 2;
constexpr int kQ80Qi = kQ81ElementsPerBlock / 4;
constexpr int kMxfp4Q81MmvqVdr = 2;
constexpr int kMxfp4Qi = kQ81ElementsPerBlock / 8;

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
        return mxfp4_value(nibble) * scale * input;
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

struct Q80Q81Dot {
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

        return half_to_float(load_u16_le(weight_block->bytes)) *
            half_to_float(load_u16_le(input_block->bytes)) *
            static_cast<float>(sum);
    }
};

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

template <typename DotFn, int Vdr, int Qi>
__launch_bounds__(kMmvqWarps * kWarpSize, 1)
__global__ void quantized_matvec_q8_1_mmvq_kernel(
    const uint8_t *weights,
    int row_stride,
    int rows,
    int block_count,
    const Q81Block *input,
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
        output[row] = sum;
    }
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

__global__ void rms_norm_kernel(
    const float *input,
    const float *weight,
    int element_count,
    float epsilon,
    float *output
) {
    __shared__ float scratch[kBlockSize];
    float sum = 0.0f;
    for (int index = threadIdx.x; index < element_count; index += blockDim.x) {
        const float value = input[index];
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
        output[index] = input[index] * weight[index] * inv_rms;
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
        values[index0] = x0 * cos_theta - x1 * sin_theta;
        values[index1] = x0 * sin_theta + x1 * cos_theta;
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

    if (threadIdx.x <= window_tokens) {
        const bool current = threadIdx.x == window_tokens;
        const float *key_head = current
            ? current_key + key_offset + kv_head * head_dim
            : cache_keys + (start + threadIdx.x) * cache_width + layer_offset + kv_head * head_dim;
        float dot = 0.0f;
        for (int dim = 0; dim < head_dim; ++dim) {
            dot += query_head[dim] * key_head[dim];
        }
        logits[threadIdx.x] = dot * scale;
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
        const float *current_value_head = current_value + value_offset + kv_head * head_dim;
        sum += current_value_head[threadIdx.x] * weights[window_tokens];
        output[head_index * head_dim + threadIdx.x] = sum;
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
        output[row] = total;
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
        output[row] = total;
    }
}

}  // namespace

extern "C" int psionic_cuda_quantized_kernels_compiled(void) {
    return 1;
}

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

extern "C" int psionic_cuda_q8_0_matvec_q8_1(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    void *output,
    void *stream
) {
    const dim3 block_dims(kWarpSize, kMmvqWarps, 1);
    quantized_matvec_q8_1_mmvq_kernel<Q80Q81Dot, kQ80Q81MmvqVdr, kQ80Qi><<<
        rows,
        block_dims,
        0,
        static_cast<cudaStream_t>(stream)
    >>>(
        static_cast<const uint8_t *>(weights),
        row_stride,
        rows,
        cols / kQ81ElementsPerBlock,
        static_cast<const Q81Block *>(input_q8_1),
        static_cast<float *>(output),
        Q80Q81Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_mxfp4_matvec_q8_1(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    void *output,
    void *stream
) {
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
        cols / kQ81ElementsPerBlock,
        static_cast<const Q81Block *>(input_q8_1),
        static_cast<float *>(output),
        Mxfp4Q81Dot{}
    );
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_cuda_rms_norm(
    const void *input,
    const void *weight,
    int element_count,
    float epsilon,
    void *output,
    void *stream
) {
    rms_norm_kernel<<<1, kBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float *>(input),
        static_cast<const float *>(weight),
        element_count,
        epsilon,
        static_cast<float *>(output)
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
    void *output,
    void *stream
) {
    moe_down_aggregate_q8_1_kernel<<<rows, kMatvecBlockSize, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const uint8_t *>(weights),
        mode,
        row_stride,
        rows,
        columns,
        static_cast<const int32_t *>(selected_ids),
        static_cast<const float *>(selected_weights),
        selected_count,
        static_cast<const Q81Block *>(activated_q8_1),
        static_cast<const float *>(bias),
        static_cast<float *>(output)
    );
    return static_cast<int>(cudaGetLastError());
}
