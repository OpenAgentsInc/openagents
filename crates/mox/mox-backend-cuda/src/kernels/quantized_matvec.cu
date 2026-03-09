#include <cuda_runtime.h>

#include <stdint.h>

namespace {

constexpr int kBlockSize = 256;
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

extern "C" int mox_cuda_router_topk_softmax(
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

extern "C" int mox_cuda_moe_gate_up_swiglu(
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

extern "C" int mox_cuda_moe_down_aggregate(
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
