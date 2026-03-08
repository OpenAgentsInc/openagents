#include <cuda_runtime.h>

#include <stdint.h>

namespace {

constexpr int kBlockSize = 256;

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
