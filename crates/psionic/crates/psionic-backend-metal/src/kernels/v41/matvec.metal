// DeepSeek-V4.1 block-scaled matrix-vector kernels on Metal (#12420), a port
// of psionic-backend-cuda/src/kernels/v41_block_matvec.cu (#12391).
//
// Weights stay packed: FP8 E4M3 with one UE8M0 scale per 32x32 block, or FP4
// E2M1 (two per byte, the low nibble holding element 2i) with one UE8M0 scale
// per 32 values along K. Activations are FP8 E4M3 with one UE8M0 scale per 32
// values. Products of codes are summed per 32-wide K block with fused
// multiply-adds, element 0 first; each block sum is multiplied by the
// activation scale and then the weight scale, and the scaled block sums are
// added in ascending block order, exactly as the CUDA kernel does.

constant constexpr int kMvWarps = 8;
constant constexpr int kMvThreads = kMvWarps * 32;
constant constexpr int kMvBlock = 32;
constant constexpr int kStageBlocks = 128;
constant constexpr int kStageStride = kMvBlock + 1;

// Nearest E4M3 code, ties to the even code, saturating at 448. The caller
// passes a finite value.
inline uint encode_e4m3_fast(float value) {
    const float magnitude = fmin(fabs(value), 448.0f);
    uint code;
    if (magnitude < 0x1p-6f) {
        // Subnormal step 2^-9; the product is exact and rint rounds half to
        // even.
        code = uint(rint(magnitude * 512.0f));
    } else {
        const uint bits = as_type<uint>(magnitude);
        const uint exponent = (bits >> 23) - 120u;
        uint mantissa = (bits >> 20) & 7u;
        const uint rest = bits & 0xfffffu;
        if (rest > 0x80000u || (rest == 0x80000u && (mantissa & 1u))) {
            mantissa += 1u;
        }
        code = (exponent << 3) + mantissa;
    }
    code = code > 126u ? 126u : code;
    return code | (signbit(value) ? 0x80u : 0u);
}

struct ActQuantArgs {
    device const float* input;
    device uchar* codes;
    device uchar* scales;
    int rows;
    int cols;
};

// One SIMD group quantizes one block of 32 values in a row.
kernel void v41_act_quant(
    constant ActQuantArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint lane [[thread_index_in_simdgroup]]) {
    const int blocks = a.cols / kMvBlock;
    const long block = (long(group) * kMvThreads + long(tid)) / 32;
    if (block >= long(a.rows) * blocks) {
        return;  // uniform across the SIMD group
    }
    const long index = block * kMvBlock + long(lane);
    const float value = bf16_round(a.input[index]);
    bool bad = !isfinite(value);
    float amax = fabs(value);
    for (ushort offset = 16; offset > 0; offset >>= 1) {
        amax = fmax(amax, simd_shuffle_xor(amax, offset));
    }
    bad = simd_any(bad);
    uint exponent = 0xffu;
    if (!bad) {
        const uint bits = as_type<uint>(fmax(amax, 1e-4f) * fdiv(1.0f, 448.0f));
        exponent = ((bits >> 23) & 0xffu) + ((bits & 0x7fffffu) != 0u ? 1u : 0u);
        bad = exponent >= 0xffu;
    }
    if (bad) {
        a.codes[index] = 0x7fu;
        if (lane == 0) {
            a.scales[block] = 0xffu;
        }
        return;
    }
    a.codes[index] = uchar(encode_e4m3_fast(fdiv(value, as_type<float>(exponent << 23))));
    if (lane == 0) {
        a.scales[block] = uchar(exponent);
    }
}

struct BlockMatvecArgs {
    device const uchar* weights;
    device const uchar* weight_scales;
    long matrix_bytes;
    long matrix_scale_bytes;
    device const int* ids;
    device const uchar* act;
    device const uchar* act_scales;
    device float* out;
    int matrices;
    int act_shared;
    int rows;
    int cols;
    int round_bf16;
    int fp4;
};

kernel void v41_block_matvec(
    constant BlockMatvecArgs& a [[buffer(0)]],
    uint2 group [[threadgroup_position_in_grid]],
    uint tid [[thread_index_in_threadgroup]],
    uint warp [[simdgroup_index_in_threadgroup]],
    uint lane [[thread_index_in_simdgroup]]) {
    threadgroup float stage[kStageBlocks * kStageStride];
    threadgroup float stage_scale[kStageBlocks];
    const int slot = int(group.y);
    const int row = int(group.x) * kMvWarps + int(warp);
    const int blocks = a.cols / kMvBlock;
    const int matrix = a.ids == nullptr ? 0 : a.ids[slot];
    const bool valid = matrix >= 0 && matrix < a.matrices;
    const long input_row = a.act_shared ? 0 : slot;
    device const uchar* x_codes = a.act + input_row * a.cols;
    device const uchar* x_scales = a.act_scales + input_row * blocks;
    device const uchar* w = a.weights + long(valid ? matrix : 0) * a.matrix_bytes;
    device const uchar* ws = a.weight_scales + long(valid ? matrix : 0) * a.matrix_scale_bytes;
    const bool fp4 = a.fp4 != 0;
    const long row_bytes = fp4 ? a.cols / 2 : a.cols;
    device const uchar* w_row = w + long(row) * row_bytes;
    device const uchar* ws_row = ws + long(fp4 ? row : row / kMvBlock) * blocks;
    float acc = 0.0f;
    for (int base = 0; base < blocks; base += kStageBlocks) {
        const int count = min(kStageBlocks, blocks - base);
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (int i = int(tid); i < count * kMvBlock; i += kMvThreads) {
            stage[(i / kMvBlock) * kStageStride + i % kMvBlock] =
                e4m3(x_codes[long(base) * kMvBlock + i]);
        }
        for (int i = int(tid); i < count; i += kMvThreads) {
            stage_scale[i] = e8m0(x_scales[base + i]);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        if (row >= a.rows || !valid) {
            continue;  // uniform across the SIMD group
        }
        for (int sub = 0; sub < count; sub += 32) {
            const int local = sub + int(lane);
            float scaled = 0.0f;
            if (local < count) {
                const int b = base + local;
                threadgroup const float* x = stage + local * kStageStride;
                float partial = 0.0f;
                if (fp4) {
                    const uint4 packed = *(device const uint4*)(w_row + long(b) * 16);
                    const uint words[4] = {packed.x, packed.y, packed.z, packed.w};
                    for (int q = 0; q < 4; ++q) {
                        for (int t = 0; t < 4; ++t) {
                            const uint byte = (words[q] >> (8 * t)) & 0xffu;
                            const int j = q * 8 + t * 2;
                            partial = fma(x[j], e2m1(byte & 15u), partial);
                            partial = fma(x[j + 1], e2m1(byte >> 4), partial);
                        }
                    }
                } else {
                    device const uint4* p = (device const uint4*)(w_row + long(b) * 32);
                    const uint4 lo = p[0];
                    const uint4 hi = p[1];
                    const uint words[8] = {lo.x, lo.y, lo.z, lo.w, hi.x, hi.y, hi.z, hi.w};
                    for (int q = 0; q < 8; ++q) {
                        for (int t = 0; t < 4; ++t) {
                            partial = fma(x[q * 4 + t], e4m3((words[q] >> (8 * t)) & 0xffu), partial);
                        }
                    }
                }
                scaled = (partial * stage_scale[local]) * e8m0(ws_row[b]);
            }
            const int n = min(32, count - sub);
            for (int t = 0; t < n; ++t) {
                acc = acc + simd_shuffle(scaled, ushort(t));
            }
        }
    }
    if (row < a.rows && lane == 0) {
        float value = valid ? acc : V41_NAN;
        if (a.round_bf16) {
            value = bf16_round(value);
        }
        a.out[long(slot) * a.rows + row] = value;
    }
}
