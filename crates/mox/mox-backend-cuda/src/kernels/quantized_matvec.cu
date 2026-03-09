#include <cuda_runtime.h>

#include <stdint.h>

namespace {

constexpr int kBlockSize = 256;
constexpr int kAttentionMaxPositions = 513;

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

}  // namespace

extern "C" int mox_cuda_quantized_kernels_compiled(void) {
    return 1;
}

extern "C" int mox_cuda_q8_0_matvec(
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

extern "C" int mox_cuda_mxfp4_matvec(
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

extern "C" int mox_cuda_rms_norm(
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

extern "C" int mox_cuda_add_f32_offset_in_place(
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

extern "C" int mox_cuda_rope_neox_in_place(
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

extern "C" int mox_cuda_attention_decode(
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
