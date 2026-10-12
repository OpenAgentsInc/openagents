// Hopper (sm_90a) projection GEMM for the Clef trunk, OpenAgentsInc/openagents
// #11443.
//
// out[t, m] (+)= sum_k x[t, k] W[m, k] with x f16 row-major [n, K] (the
// chunk's activations) and W in its GGUF layout (Q8_0 or Q4_K rows), the
// same contract as fused_linear_kernel in clef_prefill.cu.
//
// - A CTA owns 128 weight rows x NT tokens per tile and walks tiles
//   persistently (grid = min(tiles, SM count)). Tiles are weight-row major, token
//   minor, so CTAs running at the same time share weight rows in L2.
// - One producer warp issues TMA loads: the raw quantized weight bytes of
//   128 rows x 256 k (one Q4_K super-block, or 8 Q8_0 blocks, per row) and
//   the f16 token tile NT x 64 k (128-byte swizzle), each into a ring of
//   shared-memory stages guarded by mbarriers.
// - Two consumer warpgroups each own 64 weight rows. Every 64-k stage they
//   dequantize their rows straight from the raw shared-memory bytes into
//   the wgmma A fragment registers, then issue four m64nNTk16 wgmma with
//   the token tile as the shared-memory B operand, accumulating in f32.
//   The dequantization of stage i + 1 overlaps the tensor work of stage i.
//   Q8_0 gives exactly the f16 weights of the dequant kernels in
//   clef_prefill.cu; Q4_K uses f16x2 arithmetic (within one f16 rounding
//   step of them) unless EXACT, which keeps their f32 arithmetic.
// - NT (64, 128, 192 or 256) is picked per launch from the chunk size and
//   the weight's row count so the persistent grid wastes the least of its
//   last wave; every NT gives bitwise the same result per token.
// - Each output sums its k stages in order inside one CTA (no split k, no
//   atomics), and a wgmma's per-element arithmetic does not depend on the
//   tile's token count, so a token's output does not depend on which other
//   tokens share its chunk: the trunk stays bitwise chunk-invariant.
// - The epilogue adds the f32 accumulators into `out` (the residual) or
//   overwrites it.

#include <cuda.h>
#include <cuda_fp16.h>
#include <cuda_runtime.h>
#include <stdint.h>

namespace hopper {

constexpr int kFormatQ8_0 = 0;
constexpr int kFormatQ4K = 1;
constexpr int kRows = 128;       // weight rows per tile: 2 consumer warpgroups x 64
constexpr int kStageK = 64;      // k per token stage
constexpr int kRawK = 256;       // k per raw weight stage
constexpr int kThreads = 384;    // producer warpgroup + 2 consumer warpgroups
constexpr int kRawStages = 2;   // raw weight stages (256 k each)
constexpr int kMaxStages = 8;   // token stages, as many as fit
constexpr int kSmemLimit = 227 * 1024;

// u32 words of one row's raw weight stage (256 k).
template <int FMT>
struct Raw {
    static constexpr int kWords = FMT == kFormatQ8_0 ? 68 : 36;
};

__device__ __forceinline__ uint32_t smem_u32(const void *p) {
    return static_cast<uint32_t>(__cvta_generic_to_shared(p));
}

__device__ __forceinline__ void mbar_init(uint64_t *bar, int count) {
    asm volatile("mbarrier.init.shared::cta.b64 [%0], %1;\n" ::"r"(smem_u32(bar)), "r"(count));
}

__device__ __forceinline__ void mbar_expect_tx(uint64_t *bar, uint32_t bytes) {
    asm volatile("mbarrier.arrive.expect_tx.shared::cta.b64 _, [%0], %1;\n" ::"r"(smem_u32(bar)), "r"(bytes)
                 : "memory");
}

__device__ __forceinline__ void mbar_arrive(uint64_t *bar) {
    asm volatile("mbarrier.arrive.shared::cta.b64 _, [%0];\n" ::"r"(smem_u32(bar)) : "memory");
}

__device__ __forceinline__ void mbar_wait(uint64_t *bar, uint32_t parity) {
    const uint32_t address = smem_u32(bar);
    uint32_t done = 0;
    do {
        asm volatile(
            "{\n.reg .pred p;\n"
            "mbarrier.try_wait.parity.shared::cta.b64 p, [%1], %2;\n"
            "selp.u32 %0, 1, 0, p;\n}\n"
            : "=r"(done)
            : "r"(address), "r"(parity)
            : "memory");
    } while (!done);
}

__device__ __forceinline__ void tma_load_2d(void *dst, const CUtensorMap *map, uint64_t *bar, int c0, int c1) {
    asm volatile(
        "cp.async.bulk.tensor.2d.shared::cluster.global.mbarrier::complete_tx::bytes [%0], [%1, {%3, %4}], [%2];\n" ::"r"(
            smem_u32(dst)),
        "l"(reinterpret_cast<uint64_t>(map)), "r"(smem_u32(bar)), "r"(c0), "r"(c1)
        : "memory");
}

__device__ __forceinline__ void wgmma_fence() { asm volatile("wgmma.fence.sync.aligned;\n" ::: "memory"); }
__device__ __forceinline__ void wgmma_commit() { asm volatile("wgmma.commit_group.sync.aligned;\n" ::: "memory"); }
template <int N>
__device__ __forceinline__ void wgmma_wait() {
    asm volatile("wgmma.wait_group.sync.aligned %0;\n" ::"n"(N) : "memory");
}

// Shared-memory matrix descriptor for a K-major operand stored as rows of
// 128 bytes (64 f16 of k) with the 128-byte swizzle, 8-row atoms 1024 bytes
// apart. Stepping 16 k (32 bytes) adds 2 to the start address field.
__device__ __forceinline__ uint64_t smem_desc(uint32_t address) {
    uint64_t desc = static_cast<uint64_t>((address & 0x3FFFF) >> 4);
    desc |= static_cast<uint64_t>(1) << 16;           // leading byte offset (unused here)
    desc |= static_cast<uint64_t>(1024 >> 4) << 32;   // stride byte offset: 8 rows x 128 bytes
    desc |= static_cast<uint64_t>(1) << 62;           // 128-byte swizzle
    return desc;
}

template <int N>
struct Wgmma;

// m64nNk16, f32 += f16 x f16, A from registers, B K-major from shared
// memory (generated: 64, 128, 192, 256).
template <>
struct Wgmma<64> {
    static __device__ __forceinline__ void mma(float (&d)[32], const uint32_t (&a)[4], uint64_t desc, int scale_d) {
        asm volatile(
            "{\n.reg .pred p;\n"
            "setp.ne.b32 p, %37, 0;\n"
            "wgmma.mma_async.sync.aligned.m64n64k16.f32.f16.f16 "
            "{"
            "%0, %1, %2, %3, %4, %5, %6, %7, %8, %9, %10, %11, %12, %13, %14, %15,"
            "%16, %17, %18, %19, %20, %21, %22, %23, %24, %25, %26, %27, %28, %29, %30, %31"
            "}, {%32, %33, %34, %35}, %36, p, 1, 1, 0;\n}\n"
            : "+f"(d[0]), "+f"(d[1]), "+f"(d[2]), "+f"(d[3]), "+f"(d[4]), "+f"(d[5]), "+f"(d[6]), "+f"(d[7]),
              "+f"(d[8]), "+f"(d[9]), "+f"(d[10]), "+f"(d[11]), "+f"(d[12]), "+f"(d[13]), "+f"(d[14]), "+f"(d[15]),
              "+f"(d[16]), "+f"(d[17]), "+f"(d[18]), "+f"(d[19]), "+f"(d[20]), "+f"(d[21]), "+f"(d[22]), "+f"(d[23]),
              "+f"(d[24]), "+f"(d[25]), "+f"(d[26]), "+f"(d[27]), "+f"(d[28]), "+f"(d[29]), "+f"(d[30]), "+f"(d[31])
            : "r"(a[0]), "r"(a[1]), "r"(a[2]), "r"(a[3]), "l"(desc), "r"(scale_d));
    }
};

template <>
struct Wgmma<128> {
    static __device__ __forceinline__ void mma(float (&d)[64], const uint32_t (&a)[4], uint64_t desc, int scale_d) {
        asm volatile(
            "{\n.reg .pred p;\n"
            "setp.ne.b32 p, %69, 0;\n"
            "wgmma.mma_async.sync.aligned.m64n128k16.f32.f16.f16 "
            "{"
            "%0, %1, %2, %3, %4, %5, %6, %7, %8, %9, %10, %11, %12, %13, %14, %15,"
            "%16, %17, %18, %19, %20, %21, %22, %23, %24, %25, %26, %27, %28, %29, %30, %31,"
            "%32, %33, %34, %35, %36, %37, %38, %39, %40, %41, %42, %43, %44, %45, %46, %47,"
            "%48, %49, %50, %51, %52, %53, %54, %55, %56, %57, %58, %59, %60, %61, %62, %63"
            "}, {%64, %65, %66, %67}, %68, p, 1, 1, 0;\n}\n"
            : "+f"(d[0]), "+f"(d[1]), "+f"(d[2]), "+f"(d[3]), "+f"(d[4]), "+f"(d[5]), "+f"(d[6]), "+f"(d[7]),
              "+f"(d[8]), "+f"(d[9]), "+f"(d[10]), "+f"(d[11]), "+f"(d[12]), "+f"(d[13]), "+f"(d[14]), "+f"(d[15]),
              "+f"(d[16]), "+f"(d[17]), "+f"(d[18]), "+f"(d[19]), "+f"(d[20]), "+f"(d[21]), "+f"(d[22]), "+f"(d[23]),
              "+f"(d[24]), "+f"(d[25]), "+f"(d[26]), "+f"(d[27]), "+f"(d[28]), "+f"(d[29]), "+f"(d[30]), "+f"(d[31]),
              "+f"(d[32]), "+f"(d[33]), "+f"(d[34]), "+f"(d[35]), "+f"(d[36]), "+f"(d[37]), "+f"(d[38]), "+f"(d[39]),
              "+f"(d[40]), "+f"(d[41]), "+f"(d[42]), "+f"(d[43]), "+f"(d[44]), "+f"(d[45]), "+f"(d[46]), "+f"(d[47]),
              "+f"(d[48]), "+f"(d[49]), "+f"(d[50]), "+f"(d[51]), "+f"(d[52]), "+f"(d[53]), "+f"(d[54]), "+f"(d[55]),
              "+f"(d[56]), "+f"(d[57]), "+f"(d[58]), "+f"(d[59]), "+f"(d[60]), "+f"(d[61]), "+f"(d[62]), "+f"(d[63])
            : "r"(a[0]), "r"(a[1]), "r"(a[2]), "r"(a[3]), "l"(desc), "r"(scale_d));
    }
};

template <>
struct Wgmma<192> {
    static __device__ __forceinline__ void mma(float (&d)[96], const uint32_t (&a)[4], uint64_t desc, int scale_d) {
        asm volatile(
            "{\n.reg .pred p;\n"
            "setp.ne.b32 p, %101, 0;\n"
            "wgmma.mma_async.sync.aligned.m64n192k16.f32.f16.f16 "
            "{"
            "%0, %1, %2, %3, %4, %5, %6, %7, %8, %9, %10, %11, %12, %13, %14, %15,"
            "%16, %17, %18, %19, %20, %21, %22, %23, %24, %25, %26, %27, %28, %29, %30, %31,"
            "%32, %33, %34, %35, %36, %37, %38, %39, %40, %41, %42, %43, %44, %45, %46, %47,"
            "%48, %49, %50, %51, %52, %53, %54, %55, %56, %57, %58, %59, %60, %61, %62, %63,"
            "%64, %65, %66, %67, %68, %69, %70, %71, %72, %73, %74, %75, %76, %77, %78, %79,"
            "%80, %81, %82, %83, %84, %85, %86, %87, %88, %89, %90, %91, %92, %93, %94, %95"
            "}, {%96, %97, %98, %99}, %100, p, 1, 1, 0;\n}\n"
            : "+f"(d[0]), "+f"(d[1]), "+f"(d[2]), "+f"(d[3]), "+f"(d[4]), "+f"(d[5]), "+f"(d[6]), "+f"(d[7]),
              "+f"(d[8]), "+f"(d[9]), "+f"(d[10]), "+f"(d[11]), "+f"(d[12]), "+f"(d[13]), "+f"(d[14]), "+f"(d[15]),
              "+f"(d[16]), "+f"(d[17]), "+f"(d[18]), "+f"(d[19]), "+f"(d[20]), "+f"(d[21]), "+f"(d[22]), "+f"(d[23]),
              "+f"(d[24]), "+f"(d[25]), "+f"(d[26]), "+f"(d[27]), "+f"(d[28]), "+f"(d[29]), "+f"(d[30]), "+f"(d[31]),
              "+f"(d[32]), "+f"(d[33]), "+f"(d[34]), "+f"(d[35]), "+f"(d[36]), "+f"(d[37]), "+f"(d[38]), "+f"(d[39]),
              "+f"(d[40]), "+f"(d[41]), "+f"(d[42]), "+f"(d[43]), "+f"(d[44]), "+f"(d[45]), "+f"(d[46]), "+f"(d[47]),
              "+f"(d[48]), "+f"(d[49]), "+f"(d[50]), "+f"(d[51]), "+f"(d[52]), "+f"(d[53]), "+f"(d[54]), "+f"(d[55]),
              "+f"(d[56]), "+f"(d[57]), "+f"(d[58]), "+f"(d[59]), "+f"(d[60]), "+f"(d[61]), "+f"(d[62]), "+f"(d[63]),
              "+f"(d[64]), "+f"(d[65]), "+f"(d[66]), "+f"(d[67]), "+f"(d[68]), "+f"(d[69]), "+f"(d[70]), "+f"(d[71]),
              "+f"(d[72]), "+f"(d[73]), "+f"(d[74]), "+f"(d[75]), "+f"(d[76]), "+f"(d[77]), "+f"(d[78]), "+f"(d[79]),
              "+f"(d[80]), "+f"(d[81]), "+f"(d[82]), "+f"(d[83]), "+f"(d[84]), "+f"(d[85]), "+f"(d[86]), "+f"(d[87]),
              "+f"(d[88]), "+f"(d[89]), "+f"(d[90]), "+f"(d[91]), "+f"(d[92]), "+f"(d[93]), "+f"(d[94]), "+f"(d[95])
            : "r"(a[0]), "r"(a[1]), "r"(a[2]), "r"(a[3]), "l"(desc), "r"(scale_d));
    }
};

template <>
struct Wgmma<256> {
    static __device__ __forceinline__ void mma(float (&d)[128], const uint32_t (&a)[4], uint64_t desc, int scale_d) {
        asm volatile(
            "{\n.reg .pred p;\n"
            "setp.ne.b32 p, %133, 0;\n"
            "wgmma.mma_async.sync.aligned.m64n256k16.f32.f16.f16 "
            "{"
            "%0, %1, %2, %3, %4, %5, %6, %7, %8, %9, %10, %11, %12, %13, %14, %15,"
            "%16, %17, %18, %19, %20, %21, %22, %23, %24, %25, %26, %27, %28, %29, %30, %31,"
            "%32, %33, %34, %35, %36, %37, %38, %39, %40, %41, %42, %43, %44, %45, %46, %47,"
            "%48, %49, %50, %51, %52, %53, %54, %55, %56, %57, %58, %59, %60, %61, %62, %63,"
            "%64, %65, %66, %67, %68, %69, %70, %71, %72, %73, %74, %75, %76, %77, %78, %79,"
            "%80, %81, %82, %83, %84, %85, %86, %87, %88, %89, %90, %91, %92, %93, %94, %95,"
            "%96, %97, %98, %99, %100, %101, %102, %103, %104, %105, %106, %107, %108, %109, %110, %111,"
            "%112, %113, %114, %115, %116, %117, %118, %119, %120, %121, %122, %123, %124, %125, %126, %127"
            "}, {%128, %129, %130, %131}, %132, p, 1, 1, 0;\n}\n"
            : "+f"(d[0]), "+f"(d[1]), "+f"(d[2]), "+f"(d[3]), "+f"(d[4]), "+f"(d[5]), "+f"(d[6]), "+f"(d[7]),
              "+f"(d[8]), "+f"(d[9]), "+f"(d[10]), "+f"(d[11]), "+f"(d[12]), "+f"(d[13]), "+f"(d[14]), "+f"(d[15]),
              "+f"(d[16]), "+f"(d[17]), "+f"(d[18]), "+f"(d[19]), "+f"(d[20]), "+f"(d[21]), "+f"(d[22]), "+f"(d[23]),
              "+f"(d[24]), "+f"(d[25]), "+f"(d[26]), "+f"(d[27]), "+f"(d[28]), "+f"(d[29]), "+f"(d[30]), "+f"(d[31]),
              "+f"(d[32]), "+f"(d[33]), "+f"(d[34]), "+f"(d[35]), "+f"(d[36]), "+f"(d[37]), "+f"(d[38]), "+f"(d[39]),
              "+f"(d[40]), "+f"(d[41]), "+f"(d[42]), "+f"(d[43]), "+f"(d[44]), "+f"(d[45]), "+f"(d[46]), "+f"(d[47]),
              "+f"(d[48]), "+f"(d[49]), "+f"(d[50]), "+f"(d[51]), "+f"(d[52]), "+f"(d[53]), "+f"(d[54]), "+f"(d[55]),
              "+f"(d[56]), "+f"(d[57]), "+f"(d[58]), "+f"(d[59]), "+f"(d[60]), "+f"(d[61]), "+f"(d[62]), "+f"(d[63]),
              "+f"(d[64]), "+f"(d[65]), "+f"(d[66]), "+f"(d[67]), "+f"(d[68]), "+f"(d[69]), "+f"(d[70]), "+f"(d[71]),
              "+f"(d[72]), "+f"(d[73]), "+f"(d[74]), "+f"(d[75]), "+f"(d[76]), "+f"(d[77]), "+f"(d[78]), "+f"(d[79]),
              "+f"(d[80]), "+f"(d[81]), "+f"(d[82]), "+f"(d[83]), "+f"(d[84]), "+f"(d[85]), "+f"(d[86]), "+f"(d[87]),
              "+f"(d[88]), "+f"(d[89]), "+f"(d[90]), "+f"(d[91]), "+f"(d[92]), "+f"(d[93]), "+f"(d[94]), "+f"(d[95]),
              "+f"(d[96]), "+f"(d[97]), "+f"(d[98]), "+f"(d[99]), "+f"(d[100]), "+f"(d[101]), "+f"(d[102]), "+f"(d[103]),
              "+f"(d[104]), "+f"(d[105]), "+f"(d[106]), "+f"(d[107]), "+f"(d[108]), "+f"(d[109]), "+f"(d[110]), "+f"(d[111]),
              "+f"(d[112]), "+f"(d[113]), "+f"(d[114]), "+f"(d[115]), "+f"(d[116]), "+f"(d[117]), "+f"(d[118]), "+f"(d[119]),
              "+f"(d[120]), "+f"(d[121]), "+f"(d[122]), "+f"(d[123]), "+f"(d[124]), "+f"(d[125]), "+f"(d[126]), "+f"(d[127])
            : "r"(a[0]), "r"(a[1]), "r"(a[2]), "r"(a[3]), "l"(desc), "r"(scale_d));
    }
};

__device__ __forceinline__ uint32_t h2_bits(__half2 h) { return *reinterpret_cast<const uint32_t *>(&h); }
__device__ __forceinline__ __half2 bits_h2(uint32_t b) { return *reinterpret_cast<const __half2 *>(&b); }

// Q8_0 byte pair (two int8, low 16 bits of v) times the block scale d, in
// f16x2: each byte goes into an f16 1024 + (q + 128) (exact), the offset
// comes off exactly, and one rounded multiply gives round_f16(d q), the
// value dequant_q8_0_kernel computes (d q is exact in f32 there).
__device__ __forceinline__ uint32_t q8_pair(uint32_t v, __half2 d) {
    const uint32_t t = __byte_perm(v ^ 0x8080u, 0x6464u, 0x5140);
    const __half2 q = __hsub2(bits_h2(t), __float2half2_rn(1152.0f));
    return h2_bits(__hmul2(q, d));
}

// Two Q4_K nibbles (bits `shift` of each byte of v) as scale * q - minimum,
// rounded to f16 exactly as dequant_q4_k_kernel does (f32 arithmetic).
template <int SHIFT>
__device__ __forceinline__ uint32_t q4_pair_exact(uint32_t v, float scale, float minimum) {
    const float q0 = __int_as_float(0x4B000000 | ((v >> SHIFT) & 0xF)) - 8388608.0f;
    const float q1 = __int_as_float(0x4B000000 | ((v >> (8 + SHIFT)) & 0xF)) - 8388608.0f;
    return h2_bits(__floats2half2_rn(__fmaf_rn(scale, q0, -minimum), __fmaf_rn(scale, q1, -minimum)));
}

// Two Q4_K nibbles (low or high nibble of each byte of v's low 16 bits) as
// scale * q - minimum in f16x2, about half the instructions of the f32
// form: the nibble goes into an f16 1024 + q (or 1024 + 16 q for the high
// one, with the scale divided by 16), the offset comes off exactly, then
// one fused multiply-add rounds once. The scale and minimum are f16 here
// (f32 in dequant_q4_k_kernel), so a weight can differ from it by an f16
// rounding step (about 4e-4 relative).
template <int SHIFT>
__device__ __forceinline__ uint32_t q4_pair_fast(uint32_t v, __half2 scale, __half2 neg_minimum) {
    const uint32_t t = __byte_perm(v, 0u, 0x4140);
    const uint32_t mask = SHIFT == 0 ? 0x000F000Fu : 0x00F000F0u;
    const __half2 q = __hsub2(bits_h2((t & mask) | 0x64006400u), __float2half2_rn(1024.0f));
    return h2_bits(__hfma2(q, scale, neg_minimum));
}

__device__ __forceinline__ float half_bits(uint32_t bits) {
    const uint16_t b = static_cast<uint16_t>(bits);
    return __half2float(*reinterpret_cast<const __half *>(&b));
}

// The A fragments of one 64-k stage for this thread's rows `row` and
// `row + 8` (raw rows in shared memory). a[kk] is the m64k16 fragment of k
// step kk: {row k 2c..2c+1, row+8 same, row k 2c+8..2c+9, row+8 same}.
// EXACT selects the f32 Q4_K arithmetic (Q8_0 is always exact).
template <int FMT, bool EXACT>
__device__ __forceinline__ void dequant_stage(const uint32_t *raw, int row, int sub, int c, uint32_t (&a)[4][4]) {
    if constexpr (FMT == kFormatQ4K) {
#pragma unroll
        for (int h = 0; h < 2; ++h) {
            const uint32_t *r = raw + (row + 8 * h) * Raw<kFormatQ4K>::kWords;
            const uint4 header = *reinterpret_cast<const uint4 *>(r);
            const float d = half_bits(header.x);
            const float dmin = half_bits(header.x >> 16);
            uint8_t scales[12];
            *reinterpret_cast<uint32_t *>(scales) = header.y;
            *reinterpret_cast<uint32_t *>(scales + 4) = header.z;
            *reinterpret_cast<uint32_t *>(scales + 8) = header.w;
            // sub-blocks 2 sub (low nibbles) and 2 sub + 1 (high nibbles)
            float scale[2], minimum[2];
#pragma unroll
            for (int part = 0; part < 2; ++part) {
                const int is = 2 * sub + part;
                uint8_t sc, m;
                if (is < 4) {
                    sc = scales[is] & 63;
                    m = scales[is + 4] & 63;
                } else {
                    sc = (scales[is + 4] & 0xF) | ((scales[is - 4] >> 6) << 4);
                    m = (scales[is + 4] >> 4) | ((scales[is] >> 6) << 4);
                }
                scale[part] = d * sc;
                minimum[part] = dmin * m;
            }
            // qs bytes 32 sub + {2c, 2c+8, 2c+16, 2c+24} (+1): k 2c.. of each 8-k group
            const uint16_t *qs = reinterpret_cast<const uint16_t *>(r + 4) + 16 * sub;
            const uint32_t v[4] = {qs[c], qs[c + 4], qs[c + 8], qs[c + 12]};
            if constexpr (EXACT) {
#pragma unroll
                for (int i = 0; i < 4; ++i) {
                    a[i / 2][h + 2 * (i % 2)] = q4_pair_exact<0>(v[i], scale[0], minimum[0]);
                    a[2 + i / 2][h + 2 * (i % 2)] = q4_pair_exact<4>(v[i], scale[1], minimum[1]);
                }
            } else {
                const __half2 s0 = __float2half2_rn(scale[0]), m0 = __float2half2_rn(-minimum[0]);
                const __half2 s1 = __float2half2_rn(scale[1] * 0.0625f), m1 = __float2half2_rn(-minimum[1]);
#pragma unroll
                for (int i = 0; i < 4; ++i) {
                    a[i / 2][h + 2 * (i % 2)] = q4_pair_fast<0>(v[i], s0, m0);
                    a[2 + i / 2][h + 2 * (i % 2)] = q4_pair_fast<4>(v[i], s1, m1);
                }
            }
        }
    } else {
#pragma unroll
        for (int h = 0; h < 2; ++h) {
            const uint8_t *r = reinterpret_cast<const uint8_t *>(raw + (row + 8 * h) * Raw<kFormatQ8_0>::kWords);
#pragma unroll
            for (int b = 0; b < 2; ++b) {
                // block 2 sub + b: f16 scale, then 32 int8 (2-byte aligned)
                const uint16_t *block = reinterpret_cast<const uint16_t *>(r + 34 * (2 * sub + b));
                const __half2 d = __half2half2(*reinterpret_cast<const __half *>(block));
                const uint32_t v0 = block[1 + c], v1 = block[5 + c], v2 = block[9 + c], v3 = block[13 + c];
                a[2 * b][h] = q8_pair(v0, d);
                a[2 * b][h + 2] = q8_pair(v1, d);
                a[2 * b + 1][h] = q8_pair(v2, d);
                a[2 * b + 1][h + 2] = q8_pair(v3, d);
            }
        }
    }
}

struct Layout {
    int b_stages;
    int b_stage_bytes;
    int raw_stage_bytes;
    int smem_bytes;  // including 1024 bytes of alignment slack
};

__host__ __device__ inline Layout layout_for(int nt, int raw_words) {
    Layout l;
    l.b_stage_bytes = nt * 128;
    l.raw_stage_bytes = kRows * raw_words * 4;
    const int barriers = 256;
    const int room = kSmemLimit - 1024 - barriers - kRawStages * l.raw_stage_bytes;
    int stages = room / l.b_stage_bytes;
    l.b_stages = stages > kMaxStages ? kMaxStages : stages;
    l.smem_bytes = 1024 + l.b_stages * l.b_stage_bytes + kRawStages * l.raw_stage_bytes + barriers;
    return l;
}

template <int FMT, int NT, bool EXACT, bool SILU>
__global__ void __launch_bounds__(kThreads, 1)
    hopper_linear_kernel(const __grid_constant__ CUtensorMap xmap, const __grid_constant__ CUtensorMap wmap,
                         float *__restrict__ out, int n, int rows, int k, int token_tiles, int tiles, int accumulate,
                         const float *__restrict__ silu_gate, __half *__restrict__ out16) {
    constexpr int kWords = Raw<FMT>::kWords;
    const Layout lay = layout_for(NT, kWords);
    extern __shared__ uint8_t smem_raw[];
    uint8_t *base = reinterpret_cast<uint8_t *>((reinterpret_cast<uintptr_t>(smem_raw) + 1023) & ~uintptr_t(1023));
    uint8_t *b_smem = base;
    uint32_t *raw_smem = reinterpret_cast<uint32_t *>(base + lay.b_stages * lay.b_stage_bytes);
    uint64_t *bars = reinterpret_cast<uint64_t *>(base + lay.b_stages * lay.b_stage_bytes + kRawStages * lay.raw_stage_bytes);
    uint64_t *full = bars;                        // [kMaxStages]
    uint64_t *empty = bars + kMaxStages;          // [kMaxStages]
    uint64_t *raw_full = bars + 2 * kMaxStages;   // [kRawStages]
    uint64_t *raw_empty = raw_full + kRawStages;  // [kRawStages]

    const int tid = threadIdx.x;
    const int warp = tid / 32;
    const int lane = tid % 32;
    if (tid == 0) {
        for (int s = 0; s < lay.b_stages; ++s) {
            mbar_init(&full[s], 1);
            mbar_init(&empty[s], 8);  // lane 0 of each consumer warp
        }
        for (int r = 0; r < kRawStages; ++r) {
            mbar_init(&raw_full[r], 1);
            mbar_init(&raw_empty[r], 8);
        }
        asm volatile("fence.mbarrier_init.release.cluster;\n" ::: "memory");
        asm volatile("prefetch.tensormap [%0];\n" ::"l"(reinterpret_cast<uint64_t>(&xmap)) : "memory");
        asm volatile("prefetch.tensormap [%0];\n" ::"l"(reinterpret_cast<uint64_t>(&wmap)) : "memory");
    }
    __syncthreads();
    const int k_raw = k / kRawK;

    if (warp < 4) {
        // producer warpgroup: one thread issues every TMA load
        asm volatile("setmaxnreg.dec.sync.aligned.u32 40;\n");
        if (tid != 0) {
            return;
        }
        int stage = 0, phase = 0, raw_stage = 0, raw_phase = 0;
        for (int tile = blockIdx.x; tile < tiles; tile += gridDim.x) {
            const int m0 = (tile / token_tiles) * kRows;
            const int t0 = (tile % token_tiles) * NT;
            for (int kb = 0; kb < k_raw; ++kb) {
                mbar_wait(&raw_empty[raw_stage], raw_phase ^ 1);
                mbar_expect_tx(&raw_full[raw_stage], lay.raw_stage_bytes);
                tma_load_2d(raw_smem + raw_stage * (lay.raw_stage_bytes / 4), &wmap, &raw_full[raw_stage],
                            kb * kWords, m0);
                if (++raw_stage == kRawStages) {
                    raw_stage = 0;
                    raw_phase ^= 1;
                }
#pragma unroll 1
                for (int sub = 0; sub < kRawK / kStageK; ++sub) {
                    mbar_wait(&empty[stage], phase ^ 1);
                    mbar_expect_tx(&full[stage], lay.b_stage_bytes);
                    tma_load_2d(b_smem + stage * lay.b_stage_bytes, &xmap, &full[stage],
                                kb * kRawK + sub * kStageK, t0);
                    if (++stage == lay.b_stages) {
                        stage = 0;
                        phase ^= 1;
                    }
                }
            }
        }
        return;
    }

    // consumer warpgroups
    asm volatile("setmaxnreg.inc.sync.aligned.u32 232;\n");
    const int wg = warp / 4 - 1;
    const int g = lane / 4;
    const int c = lane % 4;
    const int row = wg * 64 + (warp % 4) * 16 + g;  // this thread's rows: row, row + 8 (within the tile)
    float acc[NT / 2];
#pragma unroll
    for (int i = 0; i < NT / 2; ++i) {
        acc[i] = 0.0f;
    }
    uint32_t afrag[2][4][4];
    int stage = 0, phase = 0, raw_stage = 0, raw_phase = 0;
    int prev_stage = 0;
    for (int tile = blockIdx.x; tile < tiles; tile += gridDim.x) {
        const int m0 = (tile / token_tiles) * kRows;
        const int t0 = (tile % token_tiles) * NT;
        bool first = true;
        for (int kb = 0; kb < k_raw; ++kb) {
            mbar_wait(&raw_full[raw_stage], raw_phase);
            const uint32_t *raw = raw_smem + raw_stage * (lay.raw_stage_bytes / 4);
#pragma unroll
            for (int sub = 0; sub < kRawK / kStageK; ++sub) {
                // a tile has 4 x k/256 stages (even), so the fragment
                // buffer alternates with `sub`; buffer sub & 1 was last read
                // by the stage two back, which wait_group<1> retired
                uint32_t (&a)[4][4] = afrag[sub & 1];
                dequant_stage<FMT, EXACT>(raw, row, sub, c, a);
                if (sub == kRawK / kStageK - 1) {
                    // the raw bytes were read through the generic proxy and
                    // the next TMA load into this stage writes through the
                    // async proxy: order the reads before the release
                    asm volatile("fence.proxy.async.shared::cta;\n" ::: "memory");
                    __syncwarp();
                    if (lane == 0) {
                        mbar_arrive(&raw_empty[raw_stage]);
                    }
                }
                mbar_wait(&full[stage], phase);
                const uint64_t desc = smem_desc(smem_u32(b_smem + stage * lay.b_stage_bytes));
                wgmma_fence();
#pragma unroll
                for (int kk = 0; kk < 4; ++kk) {
                    const int scale_d = (first && kk == 0) ? 0 : 1;
                    Wgmma<NT>::mma(acc, a[kk], desc + 2 * kk, scale_d);
                }
                wgmma_commit();
                wgmma_wait<1>();
                if (!first) {
                    // the previous stage's wgmma are done: release its tokens
                    if (lane == 0) {
                        mbar_arrive(&empty[prev_stage]);
                    }
                }
                first = false;
                prev_stage = stage;
                if (++stage == lay.b_stages) {
                    stage = 0;
                    phase ^= 1;
                }
            }
            if (++raw_stage == kRawStages) {
                raw_stage = 0;
                raw_phase ^= 1;
            }
        }
        wgmma_wait<0>();
        if (lane == 0) {
            mbar_arrive(&empty[prev_stage]);
        }
        // D fragment: acc[4j + 2h + e] is (weight row `row + 8h`, token 8j + 2c + e)
#pragma unroll
        for (int h = 0; h < 2; ++h) {
            const int m = m0 + row + 8 * h;
            if (m >= rows) {
                continue;
            }
#pragma unroll
            for (int j = 0; j < NT / 8; ++j) {
#pragma unroll
                for (int e = 0; e < 2; ++e) {
                    const int t = t0 + 8 * j + 2 * c + e;
                    if (t < n) {
                        const long long index = static_cast<long long>(t) * rows + m;
                        const float v = acc[4 * j + 2 * h + e];
                        if constexpr (SILU) {
                            // the up projection fused with silu_mul_to_f16_kernel
                            const float gv = silu_gate[index];
                            out16[index] = __float2half_rn(gv / (1.0f + expf(-gv)) * v);
                        } else {
                            out[index] = accumulate ? out[index] + v : v;
                        }
                    }
                }
            }
        }
    }
}

}  // namespace hopper

// ---- host side ----

namespace {

using EncodeTiled = CUresult (*)(CUtensorMap *, CUtensorMapDataType, cuuint32_t, void *, const cuuint64_t *,
                                 const cuuint64_t *, const cuuint32_t *, const cuuint32_t *, CUtensorMapInterleave,
                                 CUtensorMapSwizzle, CUtensorMapL2promotion, CUtensorMapFloatOOBfill);

EncodeTiled tensor_map_encoder() {
    static EncodeTiled encode = nullptr;
    static bool tried = false;
    if (!tried) {
        tried = true;
        void *fn = nullptr;
        cudaDriverEntryPointQueryResult query;
        if (cudaGetDriverEntryPointByVersion("cuTensorMapEncodeTiled", &fn, 12000, cudaEnableDefault, &query) ==
                cudaSuccess &&
            query == cudaDriverEntryPointSuccess) {
            encode = reinterpret_cast<EncodeTiled>(fn);
        }
    }
    return encode;
}

int sm_count() {
    static int count = 0;
    if (count == 0) {
        int device = 0;
        cudaGetDevice(&device);
        cudaDeviceGetAttribute(&count, cudaDevAttrMultiProcessorCount, device);
        if (count <= 0) {
            count = 132;
        }
    }
    return count;
}

template <int FMT, int NT, bool EXACT, bool SILU>
int launch_hopper(const void *x, const void *w, void *out, int n, int rows, int k, long long row_bytes,
                  int accumulate, cudaStream_t stream, const void *silu_gate, void *out16) {
    using namespace hopper;
    constexpr int kWords = Raw<FMT>::kWords;
    const Layout lay = layout_for(NT, kWords);
    static bool configured = false;
    if (!configured) {
        const cudaError_t code = cudaFuncSetAttribute(hopper_linear_kernel<FMT, NT, EXACT, SILU>,
                                                      cudaFuncAttributeMaxDynamicSharedMemorySize, lay.smem_bytes);
        if (code != cudaSuccess) {
            return static_cast<int>(code);
        }
        configured = true;
    }
    EncodeTiled encode = tensor_map_encoder();
    if (encode == nullptr) {
        return static_cast<int>(cudaErrorNotSupported);
    }
    CUtensorMap xmap, wmap;
    {
        const cuuint64_t dims[2] = {static_cast<cuuint64_t>(k), static_cast<cuuint64_t>(n)};
        const cuuint64_t strides[1] = {static_cast<cuuint64_t>(k) * 2};
        const cuuint32_t box[2] = {64, static_cast<cuuint32_t>(NT)};
        const cuuint32_t element_strides[2] = {1, 1};
        if (encode(&xmap, CU_TENSOR_MAP_DATA_TYPE_FLOAT16, 2, const_cast<void *>(x), dims, strides, box,
                   element_strides, CU_TENSOR_MAP_INTERLEAVE_NONE, CU_TENSOR_MAP_SWIZZLE_128B,
                   CU_TENSOR_MAP_L2_PROMOTION_L2_256B, CU_TENSOR_MAP_FLOAT_OOB_FILL_NONE) != CUDA_SUCCESS) {
            return static_cast<int>(cudaErrorInvalidValue);
        }
    }
    {
        const cuuint64_t dims[2] = {static_cast<cuuint64_t>(row_bytes / 4), static_cast<cuuint64_t>(rows)};
        const cuuint64_t strides[1] = {static_cast<cuuint64_t>(row_bytes)};
        const cuuint32_t box[2] = {static_cast<cuuint32_t>(kWords), static_cast<cuuint32_t>(kRows)};
        const cuuint32_t element_strides[2] = {1, 1};
        if (encode(&wmap, CU_TENSOR_MAP_DATA_TYPE_UINT32, 2, const_cast<void *>(w), dims, strides, box,
                   element_strides, CU_TENSOR_MAP_INTERLEAVE_NONE, CU_TENSOR_MAP_SWIZZLE_NONE,
                   CU_TENSOR_MAP_L2_PROMOTION_L2_256B, CU_TENSOR_MAP_FLOAT_OOB_FILL_NONE) != CUDA_SUCCESS) {
            return static_cast<int>(cudaErrorInvalidValue);
        }
    }
    const int token_tiles = (n + NT - 1) / NT;
    const int tiles = token_tiles * ((rows + kRows - 1) / kRows);
    const int grid = tiles < sm_count() ? tiles : sm_count();
    hopper_linear_kernel<FMT, NT, EXACT, SILU><<<grid, kThreads, lay.smem_bytes, stream>>>(
        xmap, wmap, static_cast<float *>(out), n, rows, k, token_tiles, tiles, accumulate,
        static_cast<const float *>(silu_gate), static_cast<__half *>(out16));
    return static_cast<int>(cudaGetLastError());
}

// The token tile for an n-token chunk of a `rows`-row weight. A persistent
// grid runs ceil(tiles / SMs) waves, and a wave costs about NT / eff(NT),
// where eff is the measured relative throughput of the tile (H100, 2,048
// rows: 256 -> 1, 192 -> 0.91, 128 -> 0.76, 64 -> 0.51; the small tiles
// pay the same per-stage dequantization and synchronization for less
// tensor work). Pick the cheapest. Every tile gives bitwise the same
// per-token result, so this choice never changes the output.
int auto_token_tile(int n, int rows) {
    const int sms = sm_count();
    const long long row_tiles = (rows + hopper::kRows - 1) / hopper::kRows;
    const int tiles_nt[4] = {256, 192, 128, 64};
    const double eff[4] = {1.0, 0.91, 0.76, 0.51};
    int best = 256;
    double best_cost = 1e300;
    for (int i = 0; i < 4; ++i) {
        const int nt = tiles_nt[i];
        const long long tiles = row_tiles * ((n + nt - 1) / nt);
        const long long waves = (tiles + sms - 1) / sms;
        const double cost = static_cast<double>(waves) * nt / eff[i];
        if (cost < best_cost * 0.999) {
            best = nt;
            best_cost = cost;
        }
    }
    return best;
}

}  // namespace

// 1 when this build carries the Hopper kernels and the current device is
// compute capability 9.0, else 0.
extern "C" int psionic_clef_hopper_supported(void) {
    int device = 0;
    if (cudaGetDevice(&device) != cudaSuccess) {
        return 0;
    }
    int major = 0, minor = 0;
    cudaDeviceGetAttribute(&major, cudaDevAttrComputeCapabilityMajor, device);
    cudaDeviceGetAttribute(&minor, cudaDevAttrComputeCapabilityMinor, device);
    return major == 9 && minor == 0 && tensor_map_encoder() != nullptr ? 1 : 0;
}

// out[n, rows] (+)= x16[n, k] . W[rows, k]^T with W in GGUF layout (format 0
// = Q8_0, 1 = Q4_K, 2 = Q4_K with the exact f32 dequantization arithmetic).
// k must be a multiple of 256. `token_tile` is 0 (by chunk size and weight
// rows) or 64, 128, 192, 256; every choice gives bitwise the same result
// per token.
static int hopper_linear_dispatch(const void *x, const void *w, void *out, int n, int rows, int k, int format,
                                  int accumulate, int token_tile, void *stream, const void *silu_gate, void *out16) {
    const bool exact = format == 2;
    if (exact) {
        format = hopper::kFormatQ4K;
    }
    if (n <= 0) {
        return 0;
    }
    if (k % hopper::kRawK != 0 || rows <= 0 || (format != hopper::kFormatQ8_0 && format != hopper::kFormatQ4K)) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    const long long row_bytes =
        format == hopper::kFormatQ8_0 ? static_cast<long long>(k) / 32 * 34 : static_cast<long long>(k) / 256 * 144;
    if (row_bytes % 16 != 0) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    const int nt = token_tile == 0 ? auto_token_tile(n, rows) : token_tile;
    cudaStream_t s = static_cast<cudaStream_t>(stream);
#define HOPPER(F, T, E)                                                                                  \
    (out16 != nullptr ? launch_hopper<F, T, E, true>(x, w, out, n, rows, k, row_bytes, accumulate, s, silu_gate, out16) \
                      : launch_hopper<F, T, E, false>(x, w, out, n, rows, k, row_bytes, accumulate, s, nullptr, nullptr))
#define HOPPER_TILES(F, E)                \
    switch (nt) {                         \
        case 64: return HOPPER(F, 64, E);   \
        case 128: return HOPPER(F, 128, E); \
        case 192: return HOPPER(F, 192, E); \
        case 256: return HOPPER(F, 256, E); \
        default: break;                   \
    }
    if (format == hopper::kFormatQ8_0) {
        HOPPER_TILES(hopper::kFormatQ8_0, false)
    } else if (exact) {
        HOPPER_TILES(hopper::kFormatQ4K, true)
    } else {
        HOPPER_TILES(hopper::kFormatQ4K, false)
    }
#undef HOPPER_TILES
#undef HOPPER
    return static_cast<int>(cudaErrorInvalidValue);
}

extern "C" int psionic_clef_hopper_linear(const void *x, const void *w, void *out, int n, int rows, int k, int format,
                                          int accumulate, int token_tile, void *stream) {
    return hopper_linear_dispatch(x, w, out, n, rows, k, format, accumulate, token_tile, stream, nullptr, nullptr);
}

// The up projection fused with the SiLU product (#11444): out16[n, rows] =
// f16(silu(gate[n, rows]) * (x16 . W^T)), bitwise what the f32 GEMM followed
// by silu_mul_to_f16_kernel gives, without writing and re-reading the f32
// product.
extern "C" int psionic_clef_hopper_linear_silu(const void *x, const void *w, const void *gate, void *out16, int n,
                                               int rows, int k, int format, int token_tile, void *stream) {
    if (gate == nullptr || out16 == nullptr) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    return hopper_linear_dispatch(x, w, nullptr, n, rows, k, format, 0, token_tile, stream, gate, out16);
}

// ---- Hopper causal flash attention for prefill, OpenAgentsInc/openagents #11444 ----
//
// out[t, h, :] = softmax_j(q[t, h] . k[j]) v[j] over keys j <= first + t, for
// head_dim 256, f16 q (already RMS-normed, rotated and scaled) and f16 K/V
// caches [positions, kv_heads, 256]; the same contract as
// flash_attention256_kernel in clef_prefill.cu, for any even number of query
// heads per KV head (Clef-Flash 4, Clef 27B 6).
//
// - A CTA owns 64 queries of two query heads that share one KV head: one
//   consumer warpgroup per head (its 4 warps x 16 queries are the wgmma
//   M = 64 rows), plus a producer warp that issues every TMA load: both
//   heads' queries once, then K and V tiles of 64 keys into a two-stage
//   ring guarded by mbarriers (128-byte swizzle).
// - S = Q K^T is 16 m64n64k16 wgmma with both operands in shared memory;
//   the online softmax runs on the f32 accumulators in registers; O += P V
//   is 4 m64n256k16 wgmma with P (f16) from registers and V transposed
//   (MN-major) from shared memory, accumulating O in f32 registers.
// - Key tiles sit at absolute positions 0, 64, 128, ... and each query row
//   runs them in order, so its result depends only on its position and the
//   cache, never on the chunk: a tile past its diagonal is fully masked and
//   leaves the row's state bitwise unchanged (alpha = 1, p = 0, and adding
//   zero products to the accumulator is exact). Keys at or beyond
//   `first + n` are zero-filled by TMA, and queries past n are never stored.
// - With `gate` set, the epilogue writes f16(o * sigmoid(gate)) into `out16`
//   (the arithmetic of sigmoid_gate_to_f16_kernel) instead of f32 `out`.
namespace hattn {

constexpr int kDim = 256;
constexpr int kRows = 64;                // queries per consumer warpgroup
constexpr int kKeys = 64;                // keys per tile
constexpr int kStages = 2;
constexpr int kThreads = 384;            // producer warpgroup + 2 consumer warpgroups
constexpr int kBlockBytes = 64 * 128;    // one [64 rows][64 f16] swizzled block
constexpr int kTileBytes = 4 * kBlockBytes;  // [64 rows][256 f16]
constexpr int kSmemBytes = 1024 + 2 * kTileBytes + 2 * kStages * kTileBytes + 256;

using hopper::mbar_arrive;
using hopper::mbar_expect_tx;
using hopper::mbar_init;
using hopper::mbar_wait;
using hopper::smem_u32;
using hopper::wgmma_commit;
using hopper::wgmma_fence;
using hopper::wgmma_wait;

__device__ __forceinline__ void tma_load_3d(void *dst, const CUtensorMap *map, uint64_t *bar, int c0, int c1, int c2) {
    asm volatile(
        "cp.async.bulk.tensor.3d.shared::cluster.global.mbarrier::complete_tx::bytes [%0], [%1, {%3, %4, %5}], [%2];\n" ::"r"(
            smem_u32(dst)),
        "l"(reinterpret_cast<uint64_t>(map)), "r"(smem_u32(bar)), "r"(c0), "r"(c1), "r"(c2)
        : "memory");
}

// K-major operand: rows of 128 bytes (64 f16), 128-byte swizzle, 8-row atoms
// 1024 bytes apart (hopper::smem_desc).
__device__ __forceinline__ uint64_t desc_k_major(uint32_t address) { return hopper::smem_desc(address); }

// MN-major operand (V as the B of P V: N = head dim contiguous): blocks of
// [64 keys][64 dims] (128-byte rows, 128-byte swizzle); the next 64 dims are
// the next block (leading byte offset 8,192), the next 8 keys the next 1,024
// bytes (stride byte offset).
__device__ __forceinline__ uint64_t desc_mn_major(uint32_t address) {
    uint64_t desc = static_cast<uint64_t>((address & 0x3FFFF) >> 4);
    desc |= static_cast<uint64_t>(kBlockBytes >> 4) << 16;
    desc |= static_cast<uint64_t>(1024 >> 4) << 32;
    desc |= static_cast<uint64_t>(1) << 62;
    return desc;
}

// S (64 x 64 keys) += Q K^T over one k16 step, both from shared memory.
__device__ __forceinline__ void wgmma_qk(float (&d)[32], uint64_t desc_a, uint64_t desc_b, int scale_d) {
    asm volatile(
        "{\n.reg .pred p;\n"
        "setp.ne.b32 p, %34, 0;\n"
        "wgmma.mma_async.sync.aligned.m64n64k16.f32.f16.f16 "
        "{"
        "%0, %1, %2, %3, %4, %5, %6, %7, %8, %9, %10, %11, %12, %13, %14, %15,"
        "%16, %17, %18, %19, %20, %21, %22, %23, %24, %25, %26, %27, %28, %29, %30, %31"
        "}, %32, %33, p, 1, 1, 0, 0;\n}\n"
        : "+f"(d[0]), "+f"(d[1]), "+f"(d[2]), "+f"(d[3]), "+f"(d[4]), "+f"(d[5]), "+f"(d[6]), "+f"(d[7]),
          "+f"(d[8]), "+f"(d[9]), "+f"(d[10]), "+f"(d[11]), "+f"(d[12]), "+f"(d[13]), "+f"(d[14]), "+f"(d[15]),
          "+f"(d[16]), "+f"(d[17]), "+f"(d[18]), "+f"(d[19]), "+f"(d[20]), "+f"(d[21]), "+f"(d[22]), "+f"(d[23]),
          "+f"(d[24]), "+f"(d[25]), "+f"(d[26]), "+f"(d[27]), "+f"(d[28]), "+f"(d[29]), "+f"(d[30]), "+f"(d[31])
        : "l"(desc_a), "l"(desc_b), "r"(scale_d));
}

// O (64 x 256) += P V over one k16 step of keys: P from registers, V
// transposed (MN-major) from shared memory.
__device__ __forceinline__ void wgmma_pv(float (&d)[128], const uint32_t (&a)[4], uint64_t desc) {
    asm volatile(
        "{\n.reg .pred p;\n"
        "setp.eq.u32 p, 1, 1;\n"
        "wgmma.mma_async.sync.aligned.m64n256k16.f32.f16.f16 "
        "{"
        "%0, %1, %2, %3, %4, %5, %6, %7, %8, %9, %10, %11, %12, %13, %14, %15,"
        "%16, %17, %18, %19, %20, %21, %22, %23, %24, %25, %26, %27, %28, %29, %30, %31,"
        "%32, %33, %34, %35, %36, %37, %38, %39, %40, %41, %42, %43, %44, %45, %46, %47,"
        "%48, %49, %50, %51, %52, %53, %54, %55, %56, %57, %58, %59, %60, %61, %62, %63,"
        "%64, %65, %66, %67, %68, %69, %70, %71, %72, %73, %74, %75, %76, %77, %78, %79,"
        "%80, %81, %82, %83, %84, %85, %86, %87, %88, %89, %90, %91, %92, %93, %94, %95,"
        "%96, %97, %98, %99, %100, %101, %102, %103, %104, %105, %106, %107, %108, %109, %110, %111,"
        "%112, %113, %114, %115, %116, %117, %118, %119, %120, %121, %122, %123, %124, %125, %126, %127"
        "}, {%128, %129, %130, %131}, %132, p, 1, 1, 1;\n}\n"
        : "+f"(d[0]), "+f"(d[1]), "+f"(d[2]), "+f"(d[3]), "+f"(d[4]), "+f"(d[5]), "+f"(d[6]), "+f"(d[7]),
          "+f"(d[8]), "+f"(d[9]), "+f"(d[10]), "+f"(d[11]), "+f"(d[12]), "+f"(d[13]), "+f"(d[14]), "+f"(d[15]),
          "+f"(d[16]), "+f"(d[17]), "+f"(d[18]), "+f"(d[19]), "+f"(d[20]), "+f"(d[21]), "+f"(d[22]), "+f"(d[23]),
          "+f"(d[24]), "+f"(d[25]), "+f"(d[26]), "+f"(d[27]), "+f"(d[28]), "+f"(d[29]), "+f"(d[30]), "+f"(d[31]),
          "+f"(d[32]), "+f"(d[33]), "+f"(d[34]), "+f"(d[35]), "+f"(d[36]), "+f"(d[37]), "+f"(d[38]), "+f"(d[39]),
          "+f"(d[40]), "+f"(d[41]), "+f"(d[42]), "+f"(d[43]), "+f"(d[44]), "+f"(d[45]), "+f"(d[46]), "+f"(d[47]),
          "+f"(d[48]), "+f"(d[49]), "+f"(d[50]), "+f"(d[51]), "+f"(d[52]), "+f"(d[53]), "+f"(d[54]), "+f"(d[55]),
          "+f"(d[56]), "+f"(d[57]), "+f"(d[58]), "+f"(d[59]), "+f"(d[60]), "+f"(d[61]), "+f"(d[62]), "+f"(d[63]),
          "+f"(d[64]), "+f"(d[65]), "+f"(d[66]), "+f"(d[67]), "+f"(d[68]), "+f"(d[69]), "+f"(d[70]), "+f"(d[71]),
          "+f"(d[72]), "+f"(d[73]), "+f"(d[74]), "+f"(d[75]), "+f"(d[76]), "+f"(d[77]), "+f"(d[78]), "+f"(d[79]),
          "+f"(d[80]), "+f"(d[81]), "+f"(d[82]), "+f"(d[83]), "+f"(d[84]), "+f"(d[85]), "+f"(d[86]), "+f"(d[87]),
          "+f"(d[88]), "+f"(d[89]), "+f"(d[90]), "+f"(d[91]), "+f"(d[92]), "+f"(d[93]), "+f"(d[94]), "+f"(d[95]),
          "+f"(d[96]), "+f"(d[97]), "+f"(d[98]), "+f"(d[99]), "+f"(d[100]), "+f"(d[101]), "+f"(d[102]), "+f"(d[103]),
          "+f"(d[104]), "+f"(d[105]), "+f"(d[106]), "+f"(d[107]), "+f"(d[108]), "+f"(d[109]), "+f"(d[110]), "+f"(d[111]),
          "+f"(d[112]), "+f"(d[113]), "+f"(d[114]), "+f"(d[115]), "+f"(d[116]), "+f"(d[117]), "+f"(d[118]), "+f"(d[119]),
          "+f"(d[120]), "+f"(d[121]), "+f"(d[122]), "+f"(d[123]), "+f"(d[124]), "+f"(d[125]), "+f"(d[126]), "+f"(d[127])
        : "r"(a[0]), "r"(a[1]), "r"(a[2]), "r"(a[3]), "l"(desc));
}

__device__ __forceinline__ uint32_t pack_h2(float lo, float hi) { return hopper::h2_bits(__floats2half2_rn(lo, hi)); }

__device__ __forceinline__ float sigmoid_ref(float x) { return 1.0f / (1.0f + expf(-x)); }

// Pins accumulator registers at a point in the instruction stream (the
// compiler must not move their reads or writes across an asynchronous wgmma
// or its wait).
template <int N>
__device__ __forceinline__ void fence_regs(float (&r)[N]) {
#pragma unroll
    for (int i = 0; i < N; ++i) {
        asm volatile("" : "+f"(r[i])::"memory");
    }
}

// S = Q K^T for one key tile: 16 k16 steps, issued and committed (not waited).
__device__ __forceinline__ void issue_qk(float (&s)[32], uint32_t q_base, uint32_t k_base) {
    // the first k step overwrites the accumulators (scale-d 0)
    wgmma_fence();
#pragma unroll
    for (int kk = 0; kk < kDim / 16; ++kk) {
        const uint32_t offset = (kk / 4) * kBlockBytes + (kk % 4) * 32;
        wgmma_qk(s, desc_k_major(q_base + offset), desc_k_major(k_base + offset), kk > 0 ? 1 : 0);
    }
    wgmma_commit();
}

// The online softmax of one tile's scores (rows row, row + 8 at positions
// pos): masks keys past each row's position, updates the row max and sum,
// and returns P as the A fragments of the P V product and the rescale of O.
__device__ __forceinline__ void tile_softmax(float (&s)[32], int j0, int c, const int (&pos)[2],
                                             float (&row_max)[2], float (&row_sum)[2], float (&alpha)[2],
                                             uint32_t (&pa)[kKeys / 16][4]) {
    float tile_max[2] = {-INFINITY, -INFINITY};
#pragma unroll
    for (int jn = 0; jn < kKeys / 8; ++jn) {
#pragma unroll
        for (int e = 0; e < 4; ++e) {
            const int r = e >> 1;
            const int key = j0 + 8 * jn + 2 * c + (e & 1);
            if (key > pos[r]) {
                s[4 * jn + e] = -INFINITY;
            }
            tile_max[r] = fmaxf(tile_max[r], s[4 * jn + e]);
        }
    }
    float new_max[2];
#pragma unroll
    for (int r = 0; r < 2; ++r) {
        tile_max[r] = fmaxf(tile_max[r], __shfl_xor_sync(0xffffffffu, tile_max[r], 1));
        tile_max[r] = fmaxf(tile_max[r], __shfl_xor_sync(0xffffffffu, tile_max[r], 2));
        new_max[r] = fmaxf(row_max[r], tile_max[r]);
        alpha[r] = new_max[r] == row_max[r] ? 1.0f : expf(row_max[r] - new_max[r]);
        row_max[r] = new_max[r];
    }
    float tile_sum[2] = {0.0f, 0.0f};
#pragma unroll
    for (int jn = 0; jn < kKeys / 8; ++jn) {
        float e4[4];
#pragma unroll
        for (int e = 0; e < 4; ++e) {
            const int r = e >> 1;
            const float x = s[4 * jn + e];
            e4[e] = x == -INFINITY ? 0.0f : expf(x - new_max[r]);
            tile_sum[r] += e4[e];
        }
        // A fragment of k16 step jn / 2: {row k 2c.., row+8 k 2c.., row k 2c+8.., row+8 k 2c+8..}
        pa[jn / 2][2 * (jn % 2)] = pack_h2(e4[0], e4[1]);
        pa[jn / 2][2 * (jn % 2) + 1] = pack_h2(e4[2], e4[3]);
    }
#pragma unroll
    for (int r = 0; r < 2; ++r) {
        tile_sum[r] += __shfl_xor_sync(0xffffffffu, tile_sum[r], 1);
        tile_sum[r] += __shfl_xor_sync(0xffffffffu, tile_sum[r], 2);
        row_sum[r] = row_sum[r] * alpha[r] + tile_sum[r];
    }
}

__device__ __forceinline__ void rescale_o(float (&o)[128], const float (&alpha)[2]) {
    if (alpha[0] != 1.0f || alpha[1] != 1.0f) {
#pragma unroll
        for (int i = 0; i < 32; ++i) {
            o[4 * i] *= alpha[0];
            o[4 * i + 1] *= alpha[0];
            o[4 * i + 2] *= alpha[1];
            o[4 * i + 3] *= alpha[1];
        }
    }
}

// O += P V for one key tile, issued and committed (not waited).
__device__ __forceinline__ void issue_pv(float (&o)[128], const uint32_t (&pa)[kKeys / 16][4], uint32_t v_base) {
    fence_regs(o);
    wgmma_fence();
#pragma unroll
    for (int kk = 0; kk < kKeys / 16; ++kk) {
        wgmma_pv(o, pa[kk], desc_mn_major(v_base + kk * 2048));
    }
    wgmma_commit();
}

// The two consumer warpgroups overlap each other's softmax with tensor
// work. (An intra-warpgroup schedule that issues S_{j+1} before the softmax
// of tile j, as FlashAttention-3 does, measured 20 % slower here: ptxas
// serializes the wgmma because the O rescale writes accumulator registers
// while S_{j+1} is in flight.)
__global__ void __launch_bounds__(kThreads, 1)
    hopper_attention_kernel(const __grid_constant__ CUtensorMap qmap, const __grid_constant__ CUtensorMap kmap,
                            const __grid_constant__ CUtensorMap vmap, float *__restrict__ out,
                            const float *__restrict__ gate, __half *__restrict__ out16, int n, int heads,
                            int group, int first, int query_tiles, int pairs) {
    extern __shared__ uint8_t smem_raw[];
    uint8_t *base = reinterpret_cast<uint8_t *>((reinterpret_cast<uintptr_t>(smem_raw) + 1023) & ~uintptr_t(1023));
    uint8_t *q_smem = base;                                     // [2 heads][4 blocks][64 q][64 d]
    uint8_t *k_smem = base + 2 * kTileBytes;                    // [stage][4 blocks][64 keys][64 d]
    uint8_t *v_smem = k_smem + kStages * kTileBytes;            // [stage][4 blocks][64 keys][64 d]
    uint64_t *bars = reinterpret_cast<uint64_t *>(v_smem + kStages * kTileBytes);
    uint64_t *q_full = bars;
    uint64_t *k_full = bars + 1;              // [kStages]
    uint64_t *v_full = k_full + kStages;      // [kStages]
    uint64_t *k_empty = v_full + kStages;     // [kStages]: the tile's S is done
    uint64_t *v_empty = k_empty + kStages;    // [kStages]: the tile's P V is done

    // heaviest query tiles first
    const int pair = blockIdx.x % pairs;
    const int query_tile = query_tiles - 1 - blockIdx.x / pairs;
    const int head0 = 2 * pair;
    const int kvh = head0 / group;
    const int t0 = query_tile * kRows;
    const int last_query = first + min(t0 + kRows, n) - 1;
    const int tiles = last_query / kKeys + 1;

    const int tid = threadIdx.x;
    const int warp = tid / 32;
    const int lane = tid % 32;
    if (tid == 0) {
        mbar_init(q_full, 1);
        for (int s = 0; s < kStages; ++s) {
            mbar_init(&k_full[s], 1);
            mbar_init(&v_full[s], 1);
            mbar_init(&k_empty[s], 8);  // lane 0 of each consumer warp
            mbar_init(&v_empty[s], 8);
        }
        asm volatile("fence.mbarrier_init.release.cluster;\n" ::: "memory");
        asm volatile("prefetch.tensormap [%0];\n" ::"l"(reinterpret_cast<uint64_t>(&qmap)) : "memory");
        asm volatile("prefetch.tensormap [%0];\n" ::"l"(reinterpret_cast<uint64_t>(&kmap)) : "memory");
        asm volatile("prefetch.tensormap [%0];\n" ::"l"(reinterpret_cast<uint64_t>(&vmap)) : "memory");
    }
    __syncthreads();

    if (warp < 4) {
        asm volatile("setmaxnreg.dec.sync.aligned.u32 24;\n");
        if (tid != 0) {
            return;
        }
        mbar_expect_tx(q_full, 2 * kTileBytes);
#pragma unroll 1
        for (int h = 0; h < 2; ++h) {
#pragma unroll 1
            for (int b = 0; b < 4; ++b) {
                tma_load_3d(q_smem + h * kTileBytes + b * kBlockBytes, &qmap, q_full, 64 * b, head0 + h, t0);
            }
        }
        int stage = 0, phase = 0;
#pragma unroll 1
        for (int tile = 0; tile < tiles; ++tile) {
            mbar_wait(&k_empty[stage], phase ^ 1);
            mbar_expect_tx(&k_full[stage], kTileBytes);
#pragma unroll 1
            for (int b = 0; b < 4; ++b) {
                tma_load_3d(k_smem + stage * kTileBytes + b * kBlockBytes, &kmap, &k_full[stage], 64 * b, kvh,
                            tile * kKeys);
            }
            mbar_wait(&v_empty[stage], phase ^ 1);
            mbar_expect_tx(&v_full[stage], kTileBytes);
#pragma unroll 1
            for (int b = 0; b < 4; ++b) {
                tma_load_3d(v_smem + stage * kTileBytes + b * kBlockBytes, &vmap, &v_full[stage], 64 * b, kvh,
                            tile * kKeys);
            }
            if (++stage == kStages) {
                stage = 0;
                phase ^= 1;
            }
        }
        return;
    }

    asm volatile("setmaxnreg.inc.sync.aligned.u32 240;\n");
    const int wg = warp / 4 - 1;
    const int head = head0 + wg;
    const int g = lane / 4;
    const int c = lane % 4;
    const int row = (warp % 4) * 16 + g;  // this thread's rows: row, row + 8
    const int pos[2] = {first + t0 + row, first + t0 + row + 8};
    const uint32_t q_base = smem_u32(q_smem + wg * kTileBytes);
    auto release = [&](uint64_t *bar) {
        __syncwarp();
        if (lane == 0) {
            mbar_arrive(bar);
        }
    };

    float o[128];
#pragma unroll
    for (int i = 0; i < 128; ++i) {
        o[i] = 0.0f;
    }
    float row_max[2] = {-INFINITY, -INFINITY};
    float row_sum[2] = {0.0f, 0.0f};
    uint32_t pa[kKeys / 16][4];
    float alpha[2];
    mbar_wait(q_full, 0);

    int stage = 0, phase = 0;
#pragma unroll 1
    for (int tile = 0; tile < tiles; ++tile) {
        float s[32];
        mbar_wait(&k_full[stage], phase);
        issue_qk(s, q_base, smem_u32(k_smem + stage * kTileBytes));
        wgmma_wait<0>();
        fence_regs(s);
        release(&k_empty[stage]);
        tile_softmax(s, tile * kKeys, c, pos, row_max, row_sum, alpha, pa);
        rescale_o(o, alpha);
        mbar_wait(&v_full[stage], phase);
        issue_pv(o, pa, smem_u32(v_smem + stage * kTileBytes));
        wgmma_wait<0>();
        fence_regs(o);
        release(&v_empty[stage]);
        if (++stage == kStages) {
            stage = 0;
            phase ^= 1;
        }
    }

    // o[4 jn + 2 r + e] is (row + 8 r, dim 8 jn + 2 c + e)
#pragma unroll
    for (int r = 0; r < 2; ++r) {
        const int t = t0 + row + 8 * r;
        if (t >= n) {
            continue;
        }
        const float inv = 1.0f / row_sum[r];
        const long long base_index = (static_cast<long long>(t) * heads + head) * kDim;
        if (gate != nullptr) {
#pragma unroll
            for (int jn = 0; jn < kDim / 8; ++jn) {
                const int d = 8 * jn + 2 * c;
                const float2 gv = *reinterpret_cast<const float2 *>(gate + base_index + d);
                const float a = o[4 * jn + 2 * r] * inv;
                const float b = o[4 * jn + 2 * r + 1] * inv;
                *reinterpret_cast<__half2 *>(out16 + base_index + d) =
                    __halves2half2(__float2half_rn(a * sigmoid_ref(gv.x)), __float2half_rn(b * sigmoid_ref(gv.y)));
            }
        } else {
#pragma unroll
            for (int jn = 0; jn < kDim / 8; ++jn) {
                *reinterpret_cast<float2 *>(out + base_index + 8 * jn + 2 * c) =
                    make_float2(o[4 * jn + 2 * r] * inv, o[4 * jn + 2 * r + 1] * inv);
            }
        }
    }
}

}  // namespace hattn

namespace {

int encode_cache_map(CUtensorMap *map, const void *base, int positions, int kv_heads) {
    EncodeTiled encode = tensor_map_encoder();
    if (encode == nullptr) {
        return static_cast<int>(cudaErrorNotSupported);
    }
    const cuuint64_t dims[3] = {256, static_cast<cuuint64_t>(kv_heads), static_cast<cuuint64_t>(positions)};
    const cuuint64_t strides[2] = {512, static_cast<cuuint64_t>(kv_heads) * 512};
    const cuuint32_t box[3] = {64, 1, 64};
    const cuuint32_t element_strides[3] = {1, 1, 1};
    if (encode(map, CU_TENSOR_MAP_DATA_TYPE_FLOAT16, 3, const_cast<void *>(base), dims, strides, box, element_strides,
               CU_TENSOR_MAP_INTERLEAVE_NONE, CU_TENSOR_MAP_SWIZZLE_128B, CU_TENSOR_MAP_L2_PROMOTION_L2_256B,
               CU_TENSOR_MAP_FLOAT_OOB_FILL_NONE) != CUDA_SUCCESS) {
        return static_cast<int>(cudaErrorInvalidValue);
    }
    return 0;
}

}  // namespace

// Causal attention of n queries at positions first.. over the f16 caches
// (head dim 256, an even number of query heads per KV head). With `gate`
// non-null it writes f16(o * sigmoid(gate)) to out16 ([n, heads * 256]),
// else f32 o to out. Returns cudaErrorNotSupported for shapes it does not
// take (the caller keeps clef_prefill.cu's kernel).
extern "C" int psionic_clef_hopper_attention(const void *q16, const void *kcache, const void *vcache, void *out,
                                             const void *gate, void *out16, int n, int heads, int kv_heads, int dim,
                                             int first, void *stream) {
    if (n <= 0) {
        return 0;
    }
    if (dim != hattn::kDim || kv_heads <= 0 || heads % kv_heads != 0 || (heads / kv_heads) % 2 != 0) {
        return static_cast<int>(cudaErrorNotSupported);
    }
    static bool configured = false;
    if (!configured) {
        const cudaError_t code = cudaFuncSetAttribute(hattn::hopper_attention_kernel,
                                                      cudaFuncAttributeMaxDynamicSharedMemorySize, hattn::kSmemBytes);
        if (code != cudaSuccess) {
            return static_cast<int>(code);
        }
        configured = true;
    }
    CUtensorMap qmap, kmap, vmap;
    int code = encode_cache_map(&qmap, q16, n, heads);
    if (code == 0) {
        code = encode_cache_map(&kmap, kcache, first + n, kv_heads);
    }
    if (code == 0) {
        code = encode_cache_map(&vmap, vcache, first + n, kv_heads);
    }
    if (code != 0) {
        return code;
    }
    const int query_tiles = (n + hattn::kRows - 1) / hattn::kRows;
    const int pairs = heads / 2;
    hattn::hopper_attention_kernel<<<query_tiles * pairs, hattn::kThreads, hattn::kSmemBytes, static_cast<cudaStream_t>(stream)>>>(
        qmap, kmap, vmap, static_cast<float *>(out), static_cast<const float *>(gate), static_cast<__half *>(out16), n,
        heads, heads / kv_heads, first, query_tiles, pairs);
    return static_cast<int>(cudaGetLastError());
}
